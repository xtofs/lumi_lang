/// C code generator.
///
/// Emits a C translation unit with:
///   - A small runtime header (struct Value, rc_inc/rc_dec, try_reuse, …)
///   - One C function per Lumi top-level expression
///
/// The generated code makes every RC operation visible, which is the whole
/// point: you can read the output and see exactly what Perceus decided.
use crate::ast::Lit;
use crate::rc_ast::{RcExpr, RcMatchArm};

// ── Public API ────────────────────────────────────────────────────────────────

pub fn emit_program(functions: &[(&str, &RcExpr)]) -> String {
    let mut cg = Codegen::new();
    cg.push(C_RUNTIME);
    for (name, expr) in functions {
        cg.emit_function(name, expr);
        cg.push("\n");
    }
    cg.finish()
}

// ── Codegen state ─────────────────────────────────────────────────────────────

struct Codegen {
    out: String,
    indent: usize,
    fresh: usize,
}

impl Codegen {
    fn new() -> Self {
        Codegen {
            out: String::new(),
            indent: 0,
            fresh: 0,
        }
    }

    fn finish(self) -> String {
        self.out
    }

    fn push(&mut self, s: &str) {
        self.out.push_str(s);
    }

    fn line(&mut self, s: &str) {
        let pad = "    ".repeat(self.indent);
        self.out.push_str(&format!("{pad}{s}\n"));
    }

    fn tmp(&mut self) -> String {
        self.fresh += 1;
        format!("_t{}", self.fresh)
    }

    fn emit_function(&mut self, name: &str, expr: &RcExpr) {
        self.line(&format!("Value* lumi_{name}(void) {{"));
        self.indent += 1;
        let result = self.emit_expr(expr);
        self.line(&format!("return {result};"));
        self.indent -= 1;
        self.line("}");
    }

    // ── Expression emitter ────────────────────────────────────────────────────
    // Returns the C expression (variable name or literal) that holds the result.

    fn emit_expr(&mut self, expr: &RcExpr) -> String {
        match expr {
            // ── Literals ─────────────────────────────────────────────────────
            RcExpr::Lit(Lit::Int(n)) => format!("lumi_int({n})"),
            RcExpr::Lit(Lit::Bool(b)) => format!("lumi_bool({})", *b as i32),
            RcExpr::Lit(Lit::Unit) => "lumi_unit()".to_string(),

            // ── Variable ─────────────────────────────────────────────────────
            RcExpr::Var(x) => x.clone(),

            // ── Dup: rc_inc then continue ─────────────────────────────────────
            RcExpr::Dup { var, body } => {
                self.line(&format!("rc_inc({var});  /* dup {var} */"));
                self.emit_expr(body)
            }

            // ── Drop: rc_dec then continue ────────────────────────────────────
            RcExpr::Drop { var, body } => {
                self.line(&format!("rc_dec({var});  /* drop {var} */"));
                self.emit_expr(body)
            }

            // ── Let ──────────────────────────────────────────────────────────
            RcExpr::Let { name, value, body } => {
                let val = self.emit_expr(value);
                self.line(&format!("Value* {name} = {val};"));
                self.emit_expr(body)
            }

            // ── Lambda (simplified: emit a placeholder closure) ───────────────
            RcExpr::Lam { param, body: _ } => {
                let t = self.tmp();
                self.line(&format!(
                    "Value* {t} = make_closure(/* \\{param} -> ... */);  /* closure */"
                ));
                t
            }

            // ── Application ──────────────────────────────────────────────────
            RcExpr::App(f, arg) => {
                let fv = self.emit_expr(f);
                let av = self.emit_expr(arg);
                let t = self.tmp();
                self.line(&format!("Value* {t} = apply({fv}, {av});"));
                t
            }

            // ── If ───────────────────────────────────────────────────────────
            RcExpr::If { cond, then_, else_ } => {
                let cv = self.emit_expr(cond);
                let result = self.tmp();
                self.line(&format!("Value* {result};"));
                self.line(&format!("if (lumi_is_true({cv})) {{"));
                self.indent += 1;
                let tv = self.emit_expr(then_);
                self.line(&format!("{result} = {tv};"));
                self.indent -= 1;
                self.line("} else {");
                self.indent += 1;
                let ev = self.emit_expr(else_);
                self.line(&format!("{result} = {ev};"));
                self.indent -= 1;
                self.line("}");
                result
            }

            // ── Match ────────────────────────────────────────────────────────
            // The scrutinee is consumed.  For each arm:
            //   1. Extract fields (they are now owned by the arm).
            //   2. Call try_reuse() to obtain a reuse token — at runtime this
            //      checks RC == 1 and hands back the allocation if so.
            //   3. Evaluate the arm body (which may reuse the token in a Con).
            RcExpr::Match { scrutinee, arms } => {
                let result = self.tmp();
                self.line(&format!("Value* {result};"));
                self.line(&format!("switch (tag_of({scrutinee})) {{"));
                self.indent += 1;

                for arm in arms {
                    self.emit_arm(scrutinee, arm, &result);
                }

                self.line("default: lumi_panic(\"unmatched\"); break;");
                self.indent -= 1;
                self.line("}");
                result
            }

            // ── Constructor ──────────────────────────────────────────────────
            RcExpr::Con { tag, fields, reuse } => {
                // Emit field sub-expressions first.
                let field_temps: Vec<String> = fields.iter().map(|f| self.emit_expr(f)).collect();
                let t = self.tmp();
                let n = field_temps.len();

                if let Some(token) = reuse {
                    // Perceus reuse path: try to recycle the scrutinee's memory.
                    // The runtime checks at runtime whether the token is valid.
                    self.line(&format!(
                        "Value* {t} = reuse_con({token}, TAG_{tag}, {n});  \
                         /* Perceus: reuse instead of malloc if RC==1 */"
                    ));
                } else {
                    self.line(&format!(
                        "Value* {t} = alloc_con(TAG_{tag}, {n});  /* malloc */"
                    ));
                }

                for (i, fv) in field_temps.iter().enumerate() {
                    self.line(&format!("{t}->fields[{i}] = {fv};"));
                }
                t
            }
        }
    }

    fn emit_arm(&mut self, scrutinee: &str, arm: &RcMatchArm, result: &str) {
        let tag_upper = arm.tag.to_uppercase();

        if arm.tag == "_" {
            self.line("default: {");
        } else {
            self.line(&format!("case TAG_{tag_upper}: {{"));
        }
        self.indent += 1;

        // 1. Extract fields into named locals (owned by this arm).
        for (i, binding) in arm.bindings.iter().enumerate() {
            self.line(&format!(
                "Value* {binding} = field({scrutinee}, {i});  /* extract field {i} */"
            ));
            // Increment field RCs — the match gives us ownership of the
            // scrutinee, not its children; they may still be shared.
            self.line(&format!("rc_inc({binding});"));
        }

        // 2. Acquire reuse token for the scrutinee's allocation.
        //    try_reuse() decrements the RC; if it was 1 it returns the memory.
        if let Some(token) = &arm.reuse_token {
            self.line(&format!(
                "ReuseToken {token} = try_reuse({scrutinee});  \
                 /* RC==1 -> recycle allocation; RC>1 -> returns NULL token */"
            ));
        } else {
            // No reuse possible (wildcard/literal arm); just drop the scrutinee.
            self.line(&format!("rc_dec({scrutinee});"));
        }

        // 3. Evaluate the arm body.
        let av = self.emit_expr(&arm.body);
        self.line(&format!("{result} = {av};"));
        self.line("break;");

        self.indent -= 1;
        self.line("}");
    }
}

// ── C Runtime ─────────────────────────────────────────────────────────────────

const C_RUNTIME: &str = r#"/* =========================================================
 * Lumi Runtime  —  generated by the Lumi compiler prototype
 * Perceus reference-counting with allocation reuse
 * ========================================================= */
#include <stdlib.h>
#include <stdint.h>
#include <stdio.h>
#include <stdnoreturn.h>

/* ── Value representation ─────────────────────────────────
 *
 *  Every heap object is a Value.
 *  - rc:      reference count (1 = uniquely owned)
 *  - tag:     constructor discriminant
 *  - nfields: number of pointer fields
 *  - fields:  flexible array of child Value*
 *
 *  Primitive integers are stored inline after the header.
 * ─────────────────────────────────────────────────────── */
typedef struct Value {
    uint32_t rc;
    uint32_t tag;
    uint32_t nfields;
    struct Value* fields[];
} Value;

/* ── Reuse token ──────────────────────────────────────────
 *
 *  A ReuseToken wraps a (possibly NULL) pointer to freed memory.
 *  If non-NULL, reuse_con() writes into that slot instead of malloc().
 * ─────────────────────────────────────────────────────── */
typedef struct { Value* mem; } ReuseToken;

/* ── Constructor tags ─────────────────────────────────────
 *  Extend this enum as you add ADTs to the language.
 * ─────────────────────────────────────────────────────── */
enum Tag {
    TAG_NIL   = 0,
    TAG_CONS  = 1,
    TAG_NONE  = 2,
    TAG_SOME  = 3,
    TAG_SUCC  = 4,
    TAG_ZERO  = 5,
    TAG_TRUE  = 1,   /* bool reuses small ints */
    TAG_FALSE = 0,
};

/* ── RC primitives ────────────────────────────────────────── */

static inline void rc_inc(Value* v) {
    if (v) v->rc++;
}

static inline void rc_dec(Value* v) {
    if (!v || v->rc > 1000000) return;  /* immortal objects */
    if (--v->rc == 0) {
        for (uint32_t i = 0; i < v->nfields; i++)
            rc_dec(v->fields[i]);
        free(v);
    }
}

/* ── Reuse primitives ─────────────────────────────────────
 *
 *  try_reuse(v):
 *    If RC == 1, the caller is the sole owner.  We transfer the
 *    allocation to a ReuseToken (without freeing it) and return it.
 *    If RC > 1, we decrement and return a NULL token — the caller
 *    must alloc_con() normally.
 *
 *  reuse_con(token, tag, nfields):
 *    If token.mem != NULL, reuse that allocation.
 *    Otherwise, call malloc().  Either way, reset rc=1.
 * ─────────────────────────────────────────────────────── */

static inline ReuseToken try_reuse(Value* v) {
    if (!v) return (ReuseToken){ NULL };
    if (v->rc == 1) {
        /* We own it uniquely — hand the raw memory to the token. */
        return (ReuseToken){ v };
    }
    rc_dec(v);
    return (ReuseToken){ NULL };
}

static inline Value* reuse_con(ReuseToken token, uint32_t tag, uint32_t nfields) {
    Value* v;
    if (token.mem) {
        v = token.mem;  /* reuse: no malloc! */
    } else {
        v = malloc(sizeof(Value) + nfields * sizeof(Value*));
    }
    v->rc      = 1;
    v->tag     = tag;
    v->nfields = nfields;
    return v;
}

static inline Value* alloc_con(uint32_t tag, uint32_t nfields) {
    Value* v = malloc(sizeof(Value) + nfields * sizeof(Value*));
    v->rc = 1; v->tag = tag; v->nfields = nfields;
    return v;
}

/* ── Primitive constructors ──────────────────────────────── */

static Value LUMI_UNIT = { .rc = 0xFFFFFFFF, .tag = 0, .nfields = 0 };

static inline Value* lumi_unit(void)       { return &LUMI_UNIT; }
static inline Value* lumi_bool(int b)      { return alloc_con(b ? TAG_TRUE : TAG_FALSE, 0); }
static inline int    lumi_is_true(Value* v){ return v && v->tag != 0; }
static inline uint32_t tag_of(Value* v)   { return v ? v->tag : 0; }
static inline Value* field(Value* v, uint32_t i) { return v->fields[i]; }

static inline Value* lumi_int(int64_t n) {
    /* Box the integer.  A real implementation would use a tagged pointer
     * for small ints to avoid this malloc entirely. */
    Value* v = malloc(sizeof(Value) + sizeof(int64_t));
    v->rc = 1; v->tag = 0; v->nfields = 0;
    *((int64_t*)(v + 1)) = n;
    return v;
}

/* Placeholder closure / apply — a real impl would use a closure struct. */
static inline Value* make_closure(void) { return alloc_con(99, 0); }
static inline Value* apply(Value* f, Value* arg) { (void)f; (void)arg; return lumi_unit(); }

noreturn static void lumi_panic(const char* msg) {
    fprintf(stderr, "lumi panic: %s\n", msg);
    exit(1);
}

/* =========================================================
 * Generated functions below
 * ========================================================= */

"#;
