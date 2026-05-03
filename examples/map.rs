//! Demonstrates higher-order functions and closure capture with `map`:
//!
//!   map f xs =
//!     match xs {
//!       Nil        -> Nil
//!       Cons(h, t) -> Cons(f(h), map(f)(t))
//!     }
//!
//! When `xs` is uniquely owned, Perceus reuses each Cons allocation.
//! The lambda `λx. int_add(x, 1)` passed as `f` exercises closure capture:
//! `lumi_int1` (an immortal singleton) is captured and referenced on every call.

use lumi::{emit_sample, expr_list, Expr, MatchArm, Pattern};

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

    // f = λx. int_add(x, 1)  — captures lumi_int1 (immortal singleton)
    let inc_fn = Expr::lam(
        "x",
        Expr::foreign(
            "int_add",
            vec![Expr::var("x"), Expr::foreign("lumi_int1", vec![])],
        ),
    );

    let main = Expr::let_(
        "_inc",
        inc_fn,
        Expr::let_(
            "_inp",
            expr_list(vec![Expr::int(0), Expr::int(1), Expr::int(2)]),
            Expr::let_(
                "_l",
                Expr::foreign("print", vec![Expr::str_("map (+1) [0,1,2] = ")]),
                Expr::let_(
                    "_ms",
                    Expr::app(Expr::var("map"), Expr::var("_inc")),
                    Expr::let_(
                        "_out",
                        Expr::app(Expr::var("_ms"), Expr::var("_inp")),
                        Expr::let_(
                            "_p",
                            Expr::foreign("print", vec![Expr::var("_out")]),
                            Expr::foreign("print_nl", vec![]),
                        ),
                    ),
                ),
            ),
        ),
    );

    emit_sample("map", &[("map", map_fn), ("main", main)], "main");
}
