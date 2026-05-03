//! Demonstrates Perceus `Match` + allocation reuse with `inc_list`:
//!
//!   inc_list xs =
//!     match xs {
//!       Nil        -> Nil
//!       Cons(h, t) -> Cons(Succ(h), inc_list(t))
//!     }
//!
//! When `xs` is uniquely owned (RC == 1), each Cons cell is recycled in-place
//! by the ReuseToken mechanism — no malloc per element.

use lumi::{Expr, MatchArm, Pattern, emit_sample, expr_nat, expr_list};

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
                            Expr::con("Succ", vec![Expr::var("h")]),
                            Expr::app(Expr::var("inc_list"), Expr::var("t")),
                        ],
                    ),
                ),
            ],
        ),
    );

    let driver = Expr::let_(
        "_inp",
        expr_list(vec![expr_nat(0), expr_nat(1), expr_nat(2), expr_nat(3)]),
        Expr::let_(
            "_l",
            Expr::foreign("print", vec![Expr::str_("inc_list [0,1,2,3] = ")]),
            Expr::let_(
                "_out",
                Expr::app(Expr::var("inc_list"), Expr::var("_inp")),
                Expr::let_(
                    "_p",
                    Expr::foreign("print_nat_list", vec![Expr::var("_out")]),
                    Expr::foreign("print_nl", vec![]),
                ),
            ),
        ),
    );

    emit_sample("inc_list", &[("inc_list", inc_list_fn), ("main", driver)], "main");
}
