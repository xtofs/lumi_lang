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
use crate::rc::MatchArm;
// use crate::rc_ast as rc
mod rc {
    pub use crate::rc::Expr;
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Full output: runtime header + lifted closures + entry-point functions.
pub fn emit_program(functions: &[(String, rc::Expr)]) -> String {
    let mut cg = Codegen::new();
    cg.lifted.push_str(C_RUNTIME);
    let bodies = emit_bodies(&mut cg, functions);
    cg.lifted + &bodies
}

/// Full compilable translation unit: runtime header + file-scope globals for
/// every non-entry function (immortal, to support cross-references and recursion)
/// + lifted closures + entry-point functions + `int main(void)` that initialises
/// each non-entry function as an immortal global, calls the entry, then cleans up.
pub fn emit_c_file(functions: &[(String, rc::Expr)], entry: Option<&str>) -> String {
    let mut cg = Codegen::new();
    cg.lifted.push_str("#include \"lumi_runtime.h\"\n\n");
    for (name, _) in functions {
        if Some(name.as_str()) != entry {
            cg.lifted.push_str(&format!("static Value* {name};\n"));
        }
    }
    cg.lifted.push('\n');

    let bodies = emit_bodies(&mut cg, functions);

    if let Some(entry_name) = entry {
        let mut fw = FnWriter::new(0);
        fw.line("int main(void) {");
        fw.indent += 1;
        fw.line("lumi_runtime_init();");
        for (name, _) in functions {
            if name != entry_name {
                fw.line(&format!("{name} = lumi_global(lumi_{name}());"));
            }
        }
        fw.line("");
        fw.line(&format!("Value* _result = lumi_{entry_name}();"));
        fw.line("rc_dec(_result);");
        fw.line("");
        for (name, _) in functions {
            if name != entry_name {
                fw.line(&format!("lumi_release_global({name});"));
            }
        }
        fw.line("return 0;");
        fw.indent -= 1;
        fw.line("}");
        cg.lifted + &bodies + &fw.finish()
    } else {
        cg.lifted + &bodies
    }
}

/// Only the generated portion (lifted closures + entry-point functions),
/// without the runtime header.  Useful for display / diffing.
pub fn emit_body_only(functions: &[(String, rc::Expr)]) -> String {
    let mut cg = Codegen::new();
    let bodies = emit_bodies(&mut cg, functions);
    cg.lifted + &bodies
}

fn emit_bodies(cg: &mut Codegen, functions: &[(String, rc::Expr)]) -> String {
    let mut bodies = String::new();
    for (name, expr) in functions {
        bodies.push_str(&cg.emit_function(name, expr));
        bodies.push('\n');
    }
    bodies
}

// ── Codegen state ─────────────────────────────────────────────────────────────

fn rc_comment(label: &str, expr: &rc::Expr) -> String {
    let mut buf = Vec::new();
    // using single line print for comment
    let _ = expr.pretty_print(&mut buf);
    let text = String::from_utf8_lossy(&buf);
    let mut out = format!("/* {label}:\n");
    for line in text.lines() {
        out.push_str(&format!(" *   {line}\n"));
    }
    out.push_str(" */");
    out
}

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

    fn emit_function(&mut self, name: &str, expr: &rc::Expr) -> String {
        let mut fw = FnWriter::new(0);
        fw.line(&rc_comment(&format!("Lumi {name}"), expr));
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

    fn emit_expr(&mut self, expr: &rc::Expr, fw: &mut FnWriter) -> String {
        match expr {
            // ── Literals ─────────────────────────────────────────────────────
            rc::Expr::Lit(Lit::Int(n)) => format!("lumi_int({n})"),
            rc::Expr::Lit(Lit::Bool(b)) => format!("lumi_bool({})", *b as i32),
            rc::Expr::Lit(Lit::Unit) => "lumi_unit()".to_string(),
            rc::Expr::Lit(Lit::Str(s)) => {
                let t = fw.tmp();
                fw.line(&format!(
                    "Value* {t} = lumi_str(\"{}\");",
                    s.escape_default()
                ));
                t
            }

            // ── Variable ─────────────────────────────────────────────────────
            rc::Expr::Var(x) => x.clone(),

            // ── Dup: rc_inc then continue ─────────────────────────────────────
            rc::Expr::Dup { var, body } => {
                fw.line(&format!("rc_inc({var});  /* dup {var} */"));
                self.emit_expr(body, fw)
            }

            // ── Drop: rc_dec then continue ────────────────────────────────────
            rc::Expr::Drop { var, body } => {
                fw.line(&format!("rc_dec({var});  /* drop {var} */"));
                self.emit_expr(body, fw)
            }

            // ── Let ──────────────────────────────────────────────────────────
            rc::Expr::Let { name, value, body } => {
                let val = self.emit_expr(value, fw);
                fw.line(&format!("Value* {name} = {val};"));
                self.emit_expr(body, fw)
            }

            // ── Lambda ────────────────────────────────────────────────────────
            // Lift the body to a static C function; emit a closure allocation.
            rc::Expr::Lam {
                param,
                captures,
                body,
            } => {
                let fn_name = self.fresh_name("_fn_");
                self.lift_lambda(&fn_name, param, captures, body);

                fw.line("/* declaration */");
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
            rc::Expr::App(f, arg) => {
                let fv = self.emit_expr(f, fw);
                let av = self.emit_expr(arg, fw);
                let t = fw.tmp();
                fw.line(&format!("Value* {t} = apply({fv}, {av});"));
                t
            }

            // ── If ───────────────────────────────────────────────────────────
            // The condition is consumed after testing: we evaluate it into a
            // temp, test it, then rc_dec it before entering either branch.
            rc::Expr::If { cond, then_, else_ } => {
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
            rc::Expr::Match { scrutinee, arms } => {
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
            rc::Expr::Con { tag, fields, reuse } => {
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
                    fw.line(&format!("set_field({t}, {i}, {fv});"));
                }
                t
            }

            // ── Foreign call ─────────────────────────────────────────────────
            rc::Expr::Foreign { name, args } => {
                let arg_vals: Vec<String> = args.iter().map(|a| self.emit_expr(a, fw)).collect();
                let t = fw.tmp();
                fw.line(&format!("Value* {t} = {name}({});", arg_vals.join(", ")));
                t
            }
        }
    }

    // ── Lambda lifting ────────────────────────────────────────────────────────

    fn lift_lambda(&mut self, fn_name: &str, param: &str, captures: &[String], body: &rc::Expr) {
        // Forward declaration (allows mutual recursion among closures).
        self.lifted.push_str(&format!(
            "static Value* {fn_name}(Value* _env, Value* _arg);\n\n"
        ));

        // Emit the function body into a fresh FnWriter.
        let mut fw = FnWriter::new(0);

        fw.line(&rc_comment(&format!("Lumi {fn_name}"), body));

        fw.line(&format!(
            "static Value* {fn_name}(Value* _env, Value* _arg) {{"
        ));
        fw.indent += 1;

        // Extract each captured variable from the environment.
        // Pattern: rc_inc the field to take a local reference, then rc_dec _env
        // (which decrements each field by 1), netting zero change in RC.
        for (i, cap) in captures.iter().enumerate() {
            fw.line(&format!(
                "Value* {cap} = closure_cap(_env, {i});  /* captured {cap} */"
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

    fn emit_arm(&mut self, scrutinee: &str, arm: &MatchArm, result: &str, fw: &mut FnWriter) {
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
