//! Demonstrates Perceus `Match` + allocation reuse with `inc_list`:
//!
//!   inc_list xs =
//!     match xs {
//!       Nil        -> Nil
//!       Cons(h, t) -> Cons(int_add(h, 1), inc_list(t))
//!     }
//!
//! When `xs` is uniquely owned (RC == 1), each Cons cell is recycled in-place
//! by the ReuseToken mechanism — no malloc per element.

use lumi::{compile_program, expr_list, Expr, MatchArm, Pattern};
use std::process::Command;

fn main() {
    let inc_list_fn = Expr::lam(
        "xs",
        Expr::match_(
            Expr::var("xs"),
            vec![
                MatchArm::new(Pattern::con("Nil", vec![]), Expr::con("Nil", vec![])),
                MatchArm::new(
                    Pattern::con("Cons", vec![Pattern::var("h"), Pattern::var("t")]),
                    Expr::con(
                        "Cons",
                        vec![
                            Expr::foreign(
                                "int_add",
                                vec![Expr::var("h"), Expr::foreign("lumi_int1", vec![])],
                            ),
                            Expr::app(Expr::var("inc_list"), Expr::var("t")),
                        ],
                    ),
                ),
            ],
        ),
    );

    let main = Expr::let_(
        "_inp",
        expr_list(vec![Expr::int(0), Expr::int(1), Expr::int(2), Expr::int(3)]),
        Expr::let_(
            "_l",
            Expr::foreign("print", vec![Expr::str_("inc_list [0,1,2,3] = ")]),
            Expr::let_(
                "_out",
                Expr::app(Expr::var("inc_list"), Expr::var("_inp")),
                Expr::let_(
                    "_p",
                    Expr::foreign("print", vec![Expr::var("_out")]),
                    Expr::foreign("println", vec![]),
                ),
            ),
        ),
    );

    compile_program(
        "out",
        "inc_list",
        &[("inc_list", inc_list_fn), ("main", main)],
        "main",
    );
    Command::new("./out/inc_list")
        .status()
        .expect("failed to run inc_list");
}
