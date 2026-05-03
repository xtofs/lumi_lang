/// C code generator.
///
/// Emits a C translation unit with:
///   - A runtime header (struct Value with fn-pointer field, rc_inc/rc_dec,
///     try_reuse, alloc_closure, apply, …)
///   - Forward declarations + definitions for every lifted closure function
///   - One entry-point C function per Lumi top-level expression
///
/// Closure lifting: each `Lam` node is hoisted to a static C function
/// `_fn_N(Value* _env, Value* _arg)`.  The closure creation site emits
/// `alloc_closure(_fn_N, ncaptures, cap0, cap1, ...)`.
/// On entry, the closure body extracts each captured variable from `_env`,
/// increments its RC, then decrements `_env` (which may free the closure).
use crate::ast::Lit;
use crate::rc_ast::{RcExpr, RcMatchArm};

// ── Public API ────────────────────────────────────────────────────────────────

/// Full output: runtime header + lifted closures + entry-point functions.
pub fn emit_program(functions: &[(&str, &RcExpr)]) -> String {
    let mut cg = Codegen::new();
    cg.lifted.push_str(C_RUNTIME);
    let bodies = emit_bodies(&mut cg, functions);
    cg.lifted + &bodies
}

/// Full compilable translation unit: runtime header + file-scope globals for
/// `recursive_names` (so recursive self-references inside closures resolve) +
/// lifted closures + entry-point functions.
///
/// Recursive globals are declared `static Value* <name>;`.  The caller's
/// `main()` must assign them (as immortal values) before any application.
pub fn emit_c_file(functions: &[(&str, &RcExpr)], recursive_names: &[&str]) -> String {
    let mut cg = Codegen::new();
    cg.lifted.push_str("#include \"lumi_runtime.h\"\n\n");
    for name in recursive_names {
        cg.lifted.push_str(&format!("static Value* {name};\n"));
    }
    if !recursive_names.is_empty() {
        cg.lifted.push('\n');
    }
    let bodies = emit_bodies(&mut cg, functions);
    cg.lifted + &bodies
}

/// Only the generated portion (lifted closures + entry-point functions),
/// without the runtime header.  Useful for display / diffing.
pub fn emit_body_only(functions: &[(&str, &RcExpr)]) -> String {
    let mut cg = Codegen::new();
    let bodies = emit_bodies(&mut cg, functions);
    cg.lifted + &bodies
}

fn emit_bodies(cg: &mut Codegen, functions: &[(&str, &RcExpr)]) -> String {
    let mut bodies = String::new();
    for (name, expr) in functions {
        bodies.push_str(&cg.emit_function(name, expr));
        bodies.push('\n');
    }
    bodies
}

// ── Codegen state ─────────────────────────────────────────────────────────────

struct Codegen {
    /// Accumulated lifted closure functions (forward decls + definitions).
    /// These are emitted before the entry-point functions.
    lifted: String,
    /// Counter shared across all lambdas to produce unique names.
    fresh: usize,
}

impl Codegen {
    fn new() -> Self {
        Codegen {
            lifted: String::new(),
            fresh: 0,
        }
    }

    fn fresh_name(&mut self, prefix: &str) -> String {
        self.fresh += 1;
        format!("{prefix}{}", self.fresh)
    }

    // ── Top-level function ────────────────────────────────────────────────────

    fn emit_function(&mut self, name: &str, expr: &RcExpr) -> String {
        let mut fw = FnWriter::new(0);
        fw.line(&format!("Value* lumi_{name}(void) {{"));
        fw.indent += 1;
        let result = self.emit_expr(expr, &mut fw);
        fw.line(&format!("return {result};"));
        fw.indent -= 1;
        fw.line("}");
        fw.finish()
    }

    // ── Expression emitter ────────────────────────────────────────────────────
    // Writes statements into `fw`, returns the C rvalue holding the result.

    fn emit_expr(&mut self, expr: &RcExpr, fw: &mut FnWriter) -> String {
        match expr {
            // ── Literals ─────────────────────────────────────────────────────
            RcExpr::Lit(Lit::Int(n)) => format!("lumi_int({n})"),
            RcExpr::Lit(Lit::Bool(b)) => format!("lumi_bool({})", *b as i32),
            RcExpr::Lit(Lit::Unit) => "lumi_unit()".to_string(),

            // ── Variable ─────────────────────────────────────────────────────
            RcExpr::Var(x) => x.clone(),

            // ── Dup: rc_inc then continue ─────────────────────────────────────
            RcExpr::Dup { var, body } => {
                fw.line(&format!("rc_inc({var});  /* dup {var} */"));
                self.emit_expr(body, fw)
            }

            // ── Drop: rc_dec then continue ────────────────────────────────────
            RcExpr::Drop { var, body } => {
                fw.line(&format!("rc_dec({var});  /* drop {var} */"));
                self.emit_expr(body, fw)
            }

            // ── Let ──────────────────────────────────────────────────────────
            RcExpr::Let { name, value, body } => {
                let val = self.emit_expr(value, fw);
                fw.line(&format!("Value* {name} = {val};"));
                self.emit_expr(body, fw)
            }

            // ── Lambda ────────────────────────────────────────────────────────
            // Lift the body to a static C function; emit a closure allocation.
            RcExpr::Lam {
                param,
                captures,
                body,
            } => {
                let fn_name = self.fresh_name("_fn_");
                self.lift_lambda(&fn_name, param, captures, body);

                // Build the closure allocation call.
                let n = captures.len();
                let t = fw.tmp();
                let cap_args: String = captures.iter().map(|c| format!(", {c}")).collect();
                fw.line(&format!(
                    "Value* {t} = alloc_closure({fn_name}, {n}{cap_args});\n"
                ));
                t
            }

            // ── Application ──────────────────────────────────────────────────
            // apply() transfers ownership of both f and arg to the callee.
            RcExpr::App(f, arg) => {
                let fv = self.emit_expr(f, fw);
                let av = self.emit_expr(arg, fw);
                let t = fw.tmp();
                fw.line(&format!("Value* {t} = apply({fv}, {av});"));
                t
            }

            // ── If ───────────────────────────────────────────────────────────
            // The condition is consumed after testing: we evaluate it into a
            // temp, test it, then rc_dec it before entering either branch.
            RcExpr::If { cond, then_, else_ } => {
                let cv = self.emit_expr(cond, fw);
                let flag = fw.tmp();
                let result = fw.tmp();
                // Capture the boolean *before* decrementing the value.
                fw.line(&format!("int {flag} = lumi_is_true({cv});"));
                fw.line(&format!("rc_dec({cv});  /* consume condition */"));
                fw.line(&format!("Value* {result};"));
                fw.line(&format!("if ({flag}) {{"));
                fw.indent += 1;
                let tv = self.emit_expr(then_, fw);
                fw.line(&format!("{result} = {tv};"));
                fw.indent -= 1;
                fw.line("} else {");
                fw.indent += 1;
                let ev = self.emit_expr(else_, fw);
                fw.line(&format!("{result} = {ev};"));
                fw.indent -= 1;
                fw.line("}");
                result
            }

            // ── Match ────────────────────────────────────────────────────────
            RcExpr::Match { scrutinee, arms } => {
                let result = fw.tmp();
                fw.line(&format!("Value* {result};"));
                fw.line(&format!("switch (tag_of({scrutinee})) {{"));
                fw.indent += 1;
                for arm in arms {
                    self.emit_arm(scrutinee, arm, &result.clone(), fw);
                }
                fw.line("default: lumi_panic(\"unmatched\"); break;");
                fw.indent -= 1;
                fw.line("}");
                result
            }

            // ── Constructor ──────────────────────────────────────────────────
            RcExpr::Con { tag, fields, reuse } => {
                let field_temps: Vec<String> =
                    fields.iter().map(|f| self.emit_expr(f, fw)).collect();
                let t = fw.tmp();
                let n = field_temps.len();
                let tag_upper = tag.to_uppercase();
                if let Some(token) = reuse {
                    fw.line(&format!(
                        "Value* {t} = reuse_con({token}, TAG_{tag_upper}, {n});\
                         /* Perceus: reuse if RC==1 */"
                    ));
                } else {
                    fw.line(&format!("Value* {t} = alloc_con(TAG_{tag_upper}, {n});"));
                }
                for (i, fv) in field_temps.iter().enumerate() {
                    fw.line(&format!("{t}->fields[{i}] = {fv};"));
                }
                t
            }
        }
    }

    // ── Lambda lifting ────────────────────────────────────────────────────────

    fn lift_lambda(&mut self, fn_name: &str, param: &str, captures: &[String], body: &RcExpr) {
        // Forward declaration (allows mutual recursion among closures).
        self.lifted.push_str(&format!(
            "static Value* {fn_name}(Value* _env, Value* _arg);\n"
        ));

        // Emit the function body into a fresh FnWriter.
        let mut fw = FnWriter::new(0);
        fw.line(&format!(
            "static Value* {fn_name}(Value* _env, Value* _arg) {{"
        ));
        fw.indent += 1;

        // Extract each captured variable from the environment.
        // Pattern: rc_inc the field to take a local reference, then rc_dec _env
        // (which decrements each field by 1), netting zero change in RC.
        for (i, cap) in captures.iter().enumerate() {
            fw.line(&format!(
                "Value* {cap} = _env->fields[{i}];  /* captured {cap} */"
            ));
            fw.line(&format!("rc_inc({cap});"));
        }
        // Drop the closure env (owned by this call frame).
        fw.line("rc_dec(_env);  /* release closure env */");

        // The argument is owned; bind it to the parameter name.
        fw.line(&format!("Value* {param} = _arg;"));

        // Emit the body.
        let result = self.emit_expr(body, &mut fw);
        fw.line(&format!("return {result};"));
        fw.indent -= 1;
        fw.line("}");
        fw.line("");

        self.lifted.push_str(&fw.finish());
    }

    // ── Match arm ─────────────────────────────────────────────────────────────

    fn emit_arm(&mut self, scrutinee: &str, arm: &RcMatchArm, result: &str, fw: &mut FnWriter) {
        let tag_upper = arm.tag.to_uppercase();
        if arm.tag == "_" {
            fw.line("default: {");
        } else {
            fw.line(&format!("case TAG_{tag_upper}: {{"));
        }
        fw.indent += 1;

        // Extract fields; rc_inc each (scrutinee still owns them until try_reuse).
        for (i, binding) in arm.bindings.iter().enumerate() {
            fw.line(&format!(
                "Value* {binding} = field({scrutinee}, {i});  /* field {i} */"
            ));
            fw.line(&format!("rc_inc({binding});"));
        }

        // Acquire reuse token (decrements scrutinee's RC; returns memory if RC was 1).
        if let Some(token) = &arm.reuse_token {
            fw.line(&format!(
                "ReuseToken {token} = try_reuse({scrutinee});\
                 /* RC==1 -> recycle; RC>1 -> NULL */"
            ));
        } else {
            fw.line(&format!("rc_dec({scrutinee});"));
        }

        let av = self.emit_expr(&arm.body, fw);
        fw.line(&format!("{result} = {av};"));
        fw.line("break;");
        fw.indent -= 1;
        fw.line("}");
    }
}

// ── FnWriter — writes a single C function body ────────────────────────────────

struct FnWriter {
    out: String,
    pub indent: usize,
    fresh: usize,
}

impl FnWriter {
    fn new(indent: usize) -> Self {
        FnWriter {
            out: String::new(),
            indent,
            fresh: 0,
        }
    }

    fn finish(self) -> String {
        self.out
    }

    fn line(&mut self, s: &str) {
        let pad = "    ".repeat(self.indent);
        self.out.push_str(&format!("{pad}{s}\n"));
    }

    fn tmp(&mut self) -> String {
        self.fresh += 1;
        format!("_t{}", self.fresh)
    }
}

// ── C Runtime ─────────────────────────────────────────────────────────────────

const C_RUNTIME: &str = include_str!("lumi_runtime.h");
