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

use lumi::{compile_program, expr_list, Expr, MatchArm, Pattern};
use std::process::Command;

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

    let n = 10;
    let lst: Vec<Expr> = (0..n).map(|i| Expr::int(i)).collect();

    let main = Expr::let_(
        "_inc",
        inc_fn,
        Expr::let_(
            "_inp",
            expr_list(lst),
            Expr::let_(
                "_l",
                Expr::foreign("println", vec![Expr::var("_inp")]),
                // Expr::unit(),
                Expr::let_(
                    "_map_inc",
                    Expr::app(Expr::var("map"), Expr::var("_inc")),
                    Expr::let_(
                        "_out",
                        Expr::app(Expr::var("_map_inc"), Expr::var("_inp")),
                        Expr::let_(
                            "_p",
                            Expr::foreign("println", vec![Expr::var("_out")]),
                            Expr::foreign("println", vec![Expr::unit()]),
                        ),
                    ),
                ),
            ),
        ),
    );

    compile_program("out", "map", &[("map", map_fn), ("main", main)], "main");
    Command::new("./out/map")
        .status()
        .expect("failed to run map");
}
