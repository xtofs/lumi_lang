//! Demonstrates the Perceus `If` handler with the `or` function:
//!
//!   or a b = if a then a else b
//!
//! Two RC cases arise simultaneously:
//!   - `a` appears in `cond` AND `then`  → Dup(a) inserted before the If
//!   - `b` appears only in `else`        → Drop(b) inserted in the then-branch

use lumi::{emit_sample, Expr};

fn main() {
    // or a b = if a then a else b
    let or_fn = Expr::lam(
        "a",
        Expr::lam(
            "b",
            Expr::if_(Expr::var("a"), Expr::var("a"), Expr::var("b")),
        ),
    );

    // Driver: print all four cases
    let cases: &[(&str, bool, bool)] = &[
        ("or True  True  = ", true, true),
        ("or True  False = ", true, false),
        ("or False True  = ", false, true),
        ("or False False = ", false, false),
    ];
    let driver = cases
        .iter()
        .rev()
        .enumerate()
        .fold(Expr::unit(), |rest, (i, &(label, a, b))| {
            Expr::let_(
                &format!("_sl{i}"),
                Expr::foreign("print", vec![Expr::str_(label)]),
                Expr::let_(
                    &format!("_r{i}"),
                    Expr::app(Expr::app(Expr::var("or"), Expr::bool_(a)), Expr::bool_(b)),
                    Expr::let_(
                        &format!("_sv{i}"),
                        Expr::foreign("print", vec![Expr::var(&format!("_r{i}"))]),
                        Expr::let_(&format!("_sn{i}"), Expr::foreign("print_nl", vec![]), rest),
                    ),
                ),
            )
        });

    emit_sample("or", &[("or", or_fn), ("main", driver)], "main");
}
