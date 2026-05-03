//! Demonstrates higher-order functions and closure capture with `map`:
//!
//!   map f xs =
//!     match xs {
//!       Nil        -> Nil
//!       Cons(h, t) -> Cons(f(h), map(f)(t))
//!     }
//!
//! When `xs` is uniquely owned, Perceus reuses each Cons allocation.
//! The lambda `λx. Succ(x)` passed as `f` is captured as a closure.

use lumi::{emit_sample, expr_list, expr_nat, Expr, MatchArm, Pattern};

fn main() {
    let map_fn = Expr::lam(
        "f",
        Expr::lam(
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
                                Expr::app(Expr::var("f"), Expr::var("h")),
                                Expr::app(
                                    Expr::app(Expr::var("map"), Expr::var("f")),
                                    Expr::var("t"),
                                ),
                            ],
                        ),
                    ),
                ],
            ),
        ),
    );

    let succ_fn = Expr::lam("x", Expr::con("Succ", vec![Expr::var("x")]));
    let main = Expr::let_(
        "_succ",
        succ_fn,
        Expr::let_(
            "_inp",
            expr_list(vec![expr_nat(0), expr_nat(1), expr_nat(2)]),
            Expr::let_(
                "_l",
                Expr::foreign("print", vec![Expr::str_("map succ [0,1,2] = ")]),
                Expr::let_(
                    "_ms",
                    Expr::app(Expr::var("map"), Expr::var("_succ")),
                    Expr::let_(
                        "_out",
                        Expr::app(Expr::var("_ms"), Expr::var("_inp")),
                        Expr::let_(
                            "_p",
                            Expr::foreign("print_nat_list", vec![Expr::var("_out")]),
                            Expr::foreign("print_nl", vec![]),
                        ),
                    ),
                ),
            ),
        ),
    );

    emit_sample("map", &[("map", map_fn), ("main", main)], "main");
}
