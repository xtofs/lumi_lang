mod ast;
mod codegen;
mod liveness;
mod perceus;
mod rc_ast;

use std::fs;
use std::io::{BufWriter, Write};

use ast::{Expr, MatchArm, Pattern};

fn main() {
    // ── Example 1: map ───────────────────────────────────────────────────────
    //
    // In Lumi syntax (fictional):
    //
    //   map f xs =
    //     match xs {
    //       Nil        -> Nil
    //       Cons(h, t) -> Cons(f(h), map(f)(t))
    //     }
    //
    // Perceus insight: if `xs` is uniquely owned (RC == 1),
    // each Cons cell can be reused in-place for the output list —
    // the traversal allocates nothing.

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

    // ── Example 2: increment every element of a list ──────────────────────────
    //
    //   inc_list xs =
    //     match xs {
    //       Nil        -> Nil
    //       Cons(h, t) -> Cons(h + 1, inc_list(t))
    //     }
    //
    // Because each Cons is matched and immediately reconstructed with the same
    // shape, Perceus reuses the allocation.  The generated C will show
    // `reuse_con(reuse_xs, ...)` where a naïve RC implementation would malloc.

    let inc_list_body = Expr::match_(
        Expr::var("xs"),
        vec![
            MatchArm::new(Pattern::con("Nil", vec![]), Expr::con("Nil", vec![])),
            MatchArm::new(
                Pattern::con("Cons", vec![Pattern::var("h"), Pattern::var("t")]),
                Expr::con(
                    "Cons",
                    vec![
                        Expr::con("Succ", vec![Expr::var("h")]), // h+1 as a Succ
                        Expr::app(Expr::var("inc_list"), Expr::var("t")),
                    ],
                ),
            ),
        ],
    );
    let inc_list_fn = Expr::lam("xs", inc_list_body);

    // ── Example 3: boolean and ────────────────────────────────────────────────
    //
    //   and a b = if a then b else False
    //
    // Demonstrates Dup/Drop across branches: `b` is alive in the then-branch
    // but must be dropped in the else-branch.

    let and_fn = Expr::lam(
        "a",
        Expr::lam(
            "b",
            Expr::if_(Expr::var("a"), Expr::var("b"), Expr::con("False", vec![])),
        ),
    );

    // ── Run the pipeline ──────────────────────────────────────────────────────

    let examples: &[(&str, &Expr)] = &[
        ("map", &map_fn),
        ("inc_list", &inc_list_fn),
        ("and", &and_fn),
    ];

    fs::create_dir_all("out").expect("cannot create out/");

    for (name, expr) in examples.iter().copied() {
        let path = format!("out/{name}.txt");
        let file = fs::File::create(&path).expect("cannot create output file");
        let mut w = BufWriter::new(file);

        write_banner(&mut w, name);

        writeln!(w, "─── Source AST ───────────────────────────────").unwrap();
        expr.pp(&mut w).unwrap();

        let rc_expr = perceus::transform(expr);
        writeln!(w, "\n─── After Perceus RC insertion ───────────────").unwrap();
        rc_expr.pp(&mut w).unwrap();

        writeln!(w, "\n─── Generated C ──────────────────────────────").unwrap();
        let c = codegen::emit_program(&[(name, &rc_expr)]);
        if let Some(pos) = c.find(&format!("Value* lumi_{name}")) {
            writeln!(w, "{}", &c[pos..]).unwrap();
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
