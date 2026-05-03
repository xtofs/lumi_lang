mod ast;
mod codegen;
mod liveness;
mod perceus;
mod rc_ast;
mod simplify;

use std::fs;
use std::io::{BufWriter, Write};

use ast::{Expr, MatchArm, Pattern};
use rc_ast::RcExpr;

// ── Shared C driver helpers ───────────────────────────────────────────────────

const C_DRIVER_HELPERS: &str = r#"
/* ── Driver helpers ──────────────────────────────────────────────────────── */

static void print_value(Value* v) {
    if (!v) { printf("NULL"); return; }
    switch (v->tag) {
        case TAG_FALSE:   printf("False"); break;
        case TAG_TRUE:    printf("True");  break;
        case TAG_NIL:     printf("Nil");   break;
        case TAG_ZERO:    printf("Zero");  break;
        case TAG_CONS:
            printf("Cons(");
            print_value(v->fields[0]);
            printf(", ");
            print_value(v->fields[1]);
            printf(")");
            break;
        case TAG_SUCC:
            printf("Succ(");
            print_value(v->fields[0]);
            printf(")");
            break;
        case TAG_CLOSURE: printf("<fn>"); break;
        default:          printf("?tag=%u", v->tag); break;
    }
}

static int nat_to_int(Value* v) {
    int n = 0;
    while (v && v->tag == TAG_SUCC) { n++; v = v->fields[0]; }
    return n;
}

static Value* make_nat(int n) {
    Value* v = alloc_con(TAG_ZERO, 0);
    for (int i = 0; i < n; i++) {
        Value* s = alloc_con(TAG_SUCC, 1);
        s->fields[0] = v;
        v = s;
    }
    return v;
}

static Value* list_cons(Value* head, Value* tail) {
    Value* c = alloc_con(TAG_CONS, 2);
    c->fields[0] = head;
    c->fields[1] = tail;
    return c;
}

static void print_nat_list(Value* v) {
    printf("[");
    int first = 1;
    while (v && v->tag == TAG_CONS) {
        if (!first) printf(", ");
        first = 0;
        printf("%d", nat_to_int(v->fields[0]));
        v = v->fields[1];
    }
    printf("]");
}
"#;

// ── Per-example main() drivers ────────────────────────────────────────────────

// and: not recursive, no global setup needed.
const AND_MAIN: &str = r#"
int main(void) {
    Value *fn, *r;

    fn = lumi_and(); r = apply(apply(fn, lumi_bool(1)), lumi_bool(1));
    printf("and True  True  = "); print_value(r); printf("\n"); rc_dec(r);

    fn = lumi_and(); r = apply(apply(fn, lumi_bool(1)), lumi_bool(0));
    printf("and True  False = "); print_value(r); printf("\n"); rc_dec(r);

    fn = lumi_and(); r = apply(apply(fn, lumi_bool(0)), lumi_bool(1));
    printf("and False True  = "); print_value(r); printf("\n"); rc_dec(r);

    fn = lumi_and(); r = apply(apply(fn, lumi_bool(0)), lumi_bool(0));
    printf("and False False = "); print_value(r); printf("\n"); rc_dec(r);

    return 0;
}
"#;

// inc_list: recursive — the global `inc_list` must be made immortal so that
// each recursive `apply(inc_list, t)` inside the closure doesn't free it.
const INC_LIST_MAIN: &str = r#"
int main(void) {
    inc_list = lumi_global(lumi_inc_list());

    Value* input = list_cons(make_nat(0),
                   list_cons(make_nat(1),
                   list_cons(make_nat(2),
                   list_cons(make_nat(3),
                   alloc_con(TAG_NIL, 0)))));

    printf("inc_list "); print_nat_list(input); printf(" = "); 

    Value* fn  = lumi_inc_list();
    Value* out = apply(fn, input);
    print_nat_list(out); printf("\n");
    rc_dec(out);

    lumi_release_global(inc_list);
    return 0;
}
"#;

// map: recursive — same immortal-global trick.  A hand-written C closure
// (c_succ) is used as the mapping function to avoid needing a parser.
const MAP_MAIN: &str = r#"
static Value* c_succ(Value* _env, Value* _arg) {
    rc_dec(_env);
    Value* s = alloc_con(TAG_SUCC, 1);
    s->fields[0] = _arg;
    return s;
}

int main(void) {
    map = lumi_global(lumi_map());

    Value* input = list_cons(make_nat(0),
                   list_cons(make_nat(1),
                   list_cons(make_nat(2),
                   alloc_con(TAG_NIL, 0))));

    printf("map succ"); print_nat_list(input); printf(" = ");
    Value* succ_fn  = alloc_closure(c_succ, 0);
    Value* map_fn   = lumi_map();
    Value* map_succ = apply(map_fn, succ_fn);
    Value* out      = apply(map_succ, input);
    print_nat_list(out); printf("\n");
    rc_dec(out);

    lumi_release_global(map);
    return 0;
}
"#;

fn driver_main(name: &str) -> &'static str {
    match name {
        "and" => AND_MAIN,
        "inc_list" => INC_LIST_MAIN,
        "map" => MAP_MAIN,
        _ => panic!("no driver defined for '{name}'"),
    }
}

fn recursive_names(name: &str) -> &'static [&'static str] {
    match name {
        "map" => &["map"],
        "inc_list" => &["inc_list"],
        _ => &[],
    }
}

fn main() {
    // ── Example 1: map ───────────────────────────────────────────────────────
    //
    //   map f xs =
    //     match xs {
    //       Nil        -> Nil
    //       Cons(h, t) -> Cons(f(h), map(f)(t))
    //     }
    //
    // Perceus: if `xs` is uniquely owned, each Cons cell is reused in-place.

    let map_body = Expr::match_(
        Expr::var("xs"),
        vec![
            MatchArm::new(Pattern::con("Nil", vec![]), Expr::con("Nil", vec![])),
            MatchArm::new(
                Pattern::con("Cons", vec![Pattern::var("h"), Pattern::var("t")]),
                Expr::con(
                    "Cons",
                    vec![
                        Expr::app(Expr::var("f"), Expr::var("h")),
                        Expr::app(Expr::app(Expr::var("map"), Expr::var("f")), Expr::var("t")),
                    ],
                ),
            ),
        ],
    );
    let map_fn = Expr::lam("f", Expr::lam("xs", map_body));

    // ── Example 2: inc_list ──────────────────────────────────────────────────
    //
    //   inc_list xs =
    //     match xs {
    //       Nil        -> Nil
    //       Cons(h, t) -> Cons(Succ(h), inc_list(t))
    //     }

    let inc_list_body = Expr::match_(
        Expr::var("xs"),
        vec![
            MatchArm::new(Pattern::con("Nil", vec![]), Expr::con("Nil", vec![])),
            MatchArm::new(
                Pattern::con("Cons", vec![Pattern::var("h"), Pattern::var("t")]),
                Expr::con(
                    "Cons",
                    vec![
                        Expr::con("Succ", vec![Expr::var("h")]),
                        Expr::app(Expr::var("inc_list"), Expr::var("t")),
                    ],
                ),
            ),
        ],
    );
    let inc_list_fn = Expr::lam("xs", inc_list_body);

    // ── Example 3: and ───────────────────────────────────────────────────────
    //
    //   and a b = if a then b else False
    //
    // Demonstrates Drop in the untaken branch.

    let and_fn = Expr::lam(
        "a",
        Expr::lam(
            "b",
            Expr::if_(Expr::var("a"), Expr::var("b"), Expr::con("False", vec![])),
        ),
    );

    // ── Example 4: simplifier demo ───────────────────────────────────────────
    //
    // Manually constructed RcExpr that contains redundant RC operations.
    // This represents what a first, conservative RC-insertion pass might
    // produce before the simplifier cleans it up.
    //
    //   Rule 1 (cancel adjacent):
    //     dup(f); drop(f); Nil  →  Nil
    //
    //   Rule 2 (dead dup):
    //     dup(f); 42  →  42          (f not mentioned in body)
    //
    //   Rule 3 (commute then cancel):
    //     dup(f); let x = 42 in drop(f); x
    //     →  let x = 42 in dup(f); drop(f); x   [commute past Let]
    //     →  let x = 42 in x                     [cancel]

    // Rule 1
    let rule1_before = RcExpr::dup(
        "f",
        RcExpr::drop_(
            "f",
            RcExpr::Con {
                tag: "Nil".into(),
                fields: vec![],
                reuse: None,
            },
        ),
    );
    let rule1_after = simplify::simplify(rule1_before.clone());

    // Rule 2
    let rule2_before = RcExpr::dup("f", RcExpr::Lit(ast::Lit::Int(42)));
    let rule2_after = simplify::simplify(rule2_before.clone());

    // Rule 3 (commuting exposes the cancel)
    let rule3_before = RcExpr::dup(
        "f",
        RcExpr::Let {
            name: "x".into(),
            value: Box::new(RcExpr::Lit(ast::Lit::Int(42))),
            body: Box::new(RcExpr::drop_("f", RcExpr::Var("x".into()))),
        },
    );
    let rule3_after = simplify::simplify(rule3_before.clone());

    // ── Run the pipeline ──────────────────────────────────────────────────────

    let source_examples: &[(&str, &Expr)] = &[
        ("map", &map_fn),
        ("inc_list", &inc_list_fn),
        ("and", &and_fn),
    ];

    fs::create_dir_all("out").expect("cannot create out/");
    fs::write("out/lumi_runtime.h", include_str!("lumi_runtime.h"))
        .expect("cannot write lumi_runtime.h");

    for (name, expr) in source_examples.iter().copied() {
        let path = format!("out/{name}.txt");
        let file = fs::File::create(&path).expect("cannot create output file");
        let mut w = BufWriter::new(file);
        write_banner(&mut w, name);

        writeln!(w, "─── Source AST ───────────────────────────────").unwrap();
        expr.pp(&mut w).unwrap();

        let rc_expr = perceus::transform(expr);
        writeln!(w, "\n─── After Perceus RC insertion ───────────────").unwrap();
        rc_expr.pp(&mut w).unwrap();

        let simplified = simplify::simplify(rc_expr.clone());
        if !simplify::structurally_equal_pub(&simplified, &rc_expr) {
            writeln!(w, "\n─── After simplification ─────────────────────").unwrap();
            simplified.pp(&mut w).unwrap();
        }

        writeln!(w, "\n─── Generated C ──────────────────────────────").unwrap();
        let c = codegen::emit_body_only(&[(name, &simplified)]);
        writeln!(w, "{c}").unwrap();

        println!("wrote {path}");
    }

    // ── Generate compilable .c files ──────────────────────────────────────────

    for (name, expr) in source_examples.iter().copied() {
        let rc = perceus::transform(expr);
        let simplified = simplify::simplify(rc);
        let rec = recursive_names(name);
        let program = codegen::emit_c_file(&[(name, &simplified)], rec);
        let full = format!("{program}{C_DRIVER_HELPERS}\n{}\n", driver_main(name));
        let path = format!("out/{name}.c");
        fs::write(&path, &full).expect("cannot write .c file");
        println!("wrote {path}");
    }

    // Simplifier demo
    {
        let path = "out/simplifier_demo.txt";
        let file = fs::File::create(path).expect("cannot create output file");
        let mut w = BufWriter::new(file);
        write_banner(&mut w, "simplifier demo");

        let cases: &[(&str, &RcExpr, &RcExpr)] = &[
            ("rule 1 — adjacent cancel", &rule1_before, &rule1_after),
            ("rule 2 — dead dup", &rule2_before, &rule2_after),
            ("rule 3 — commute + cancel", &rule3_before, &rule3_after),
        ];

        for (label, before, after) in cases {
            writeln!(w, "  {label}").unwrap();
            write!(w, "    before: ").unwrap();
            before.pp(&mut w).unwrap();
            write!(w, "    after:  ").unwrap();
            after.pp(&mut w).unwrap();
            writeln!(w).unwrap();
        }

        println!("wrote {path}");
    }
}

fn write_banner(w: &mut dyn Write, name: &str) {
    writeln!(
        w,
        "╔═══════════════════════════════════════════════╗\n\
         ║  {name:<45}║\n\
         ╚═══════════════════════════════════════════════╝\n"
    )
    .unwrap();
}
