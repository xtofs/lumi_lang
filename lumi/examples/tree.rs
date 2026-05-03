//! Demonstrates boxed machine integers and the Perceus `If` Dup rule with
//! a balanced binary tree:
//!
//!   tree n =
//!     if int_eq(n, 0) then Zero
//!     else Cons(tree(int_sub(n, 1)), tree(int_sub(n, 1)))
//!
//!   sum_tree t =
//!     match t {
//!       Zero()     -> 1
//!       Cons(l, r) -> int_add(sum_tree(l), sum_tree(r))
//!     }
//!
//! `n` appears in the condition and twice in the else-branch →
//! Perceus inserts Dup(n) before the If.
//! LUMI_INT0 and LUMI_INT1 are immortal singletons (no malloc per call).

use lumi::{Expr, MatchArm, Pattern, emit_sample};

fn main() {
    let tree_fn = Expr::lam(
        "n",
        Expr::if_(
            Expr::foreign(
                "int_eq",
                vec![Expr::var("n"), Expr::foreign("lumi_int0", vec![])],
            ),
            Expr::con("Zero", vec![]),
            Expr::con(
                "Cons",
                vec![
                    Expr::app(
                        Expr::var("tree"),
                        Expr::foreign(
                            "int_sub",
                            vec![Expr::var("n"), Expr::foreign("lumi_int1", vec![])],
                        ),
                    ),
                    Expr::app(
                        Expr::var("tree"),
                        Expr::foreign(
                            "int_sub",
                            vec![Expr::var("n"), Expr::foreign("lumi_int1", vec![])],
                        ),
                    ),
                ],
            ),
        ),
    );

    let sum_tree_fn = Expr::lam(
        "t",
        Expr::match_(
            Expr::var("t"),
            vec![
                MatchArm::new(Pattern::con("Zero", vec![]), Expr::int(1)),
                MatchArm::new(
                    Pattern::con("Cons", vec![Pattern::var("l"), Pattern::var("r")]),
                    Expr::foreign(
                        "int_add",
                        vec![
                            Expr::app(Expr::var("sum_tree"), Expr::var("l")),
                            Expr::app(Expr::var("sum_tree"), Expr::var("r")),
                        ],
                    ),
                ),
            ],
        ),
    );

    let n = 20;
    let title = format!("sum_tree(tree({n})) = ");
    let main = Expr::let_(
        "_l",
        Expr::foreign("print", vec![Expr::str_(&title)]),
        Expr::let_(
            "_t",
            Expr::app(Expr::var("tree"), Expr::int(n)),
            Expr::let_(
                "_s",
                Expr::app(Expr::var("sum_tree"), Expr::var("_t")),
                Expr::let_(
                    "_ps",
                    Expr::foreign("print", vec![Expr::var("_s")]),
                    Expr::foreign("print_nl", vec![]),
                ),
            ),
        ),
    );

    emit_sample(
        "tree",
        &[
            ("tree", tree_fn),
            ("sum_tree", sum_tree_fn),
            ("main", main),
        ],
        "main",
    );
}
