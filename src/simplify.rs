/// rc::Expr simplification pass.
///
/// Eliminates redundant RC operations by applying a small set of rewrite rules
/// bottom-up, iterated to fixpoint.
///
/// ## Rules
///
/// **Cancellation** — adjacent dup/drop on the same variable annihilate:
///
///   Dup(x, Drop(x, e))  →  e
///   Drop(x, Dup(x, e))  →  e
///
/// **Dead dup** — if x is not mentioned in the body, the increment is pointless:
///
///   Dup(x, e)           →  e        when x ∉ mentions(e)
///
/// **Commuting** — move a Dup or Drop past a Let whose value doesn't use the
/// variable, pushing the op closer to its use (or its paired op):
///
///   Dup(x,  Let(y, v, b))  →  Let(y, v, Dup(x,  b))   when x ∉ mentions(v)
///   Drop(x, Let(y, v, b))  →  Let(y, v, Drop(x, b))   when x ∉ mentions(v)
///
/// The commuting rules alone don't change observable behaviour, but they
/// expose cancellation opportunities that a single-pass bottom-up scan misses.
use crate::rc::MatchArm;
// use crate::rc_ast as rc
mod rc {
    pub use crate::rc::Expr;
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Simplify `expr`, iterating until no rule applies.
pub fn simplify(expr: rc::Expr) -> rc::Expr {
    let mut current = expr;
    loop {
        let next = step(current.clone());
        if structurally_equal(&next, &current) {
            return next;
        }
        current = next;
    }
}

// ── Single pass ───────────────────────────────────────────────────────────────

/// One bottom-up simplification sweep.  Recurse into children first, then
/// attempt to reduce the current node.
fn step(expr: rc::Expr) -> rc::Expr {
    match expr {
        rc::Expr::Dup { var, body } => {
            let body = step(*body);
            reduce_dup(var, body)
        }

        rc::Expr::Drop { var, body } => {
            let body = step(*body);
            reduce_drop(var, body)
        }

        rc::Expr::Let { name, value, body } => rc::Expr::Let {
            name,
            value: Box::new(step(*value)),
            body: Box::new(step(*body)),
        },

        rc::Expr::Lam {
            param,
            captures,
            body,
        } => rc::Expr::Lam {
            param,
            captures,
            body: Box::new(step(*body)),
        },

        rc::Expr::App(f, arg) => rc::Expr::App(Box::new(step(*f)), Box::new(step(*arg))),

        rc::Expr::If { cond, then_, else_ } => rc::Expr::If {
            cond: Box::new(step(*cond)),
            then_: Box::new(step(*then_)),
            else_: Box::new(step(*else_)),
        },

        rc::Expr::Match { scrutinee, arms } => rc::Expr::Match {
            scrutinee,
            arms: arms
                .into_iter()
                .map(|arm| MatchArm {
                    body: step(arm.body),
                    ..arm
                })
                .collect(),
        },

        rc::Expr::Con { tag, fields, reuse } => rc::Expr::Con {
            tag,
            fields: fields.into_iter().map(step).collect(),
            reuse,
        },

        leaf => leaf,
    }
}

// ── Reduction helpers ─────────────────────────────────────────────────────────

fn reduce_dup(var: String, body: rc::Expr) -> rc::Expr {
    match body {
        // Dup(x, Drop(x, e))  →  e
        rc::Expr::Drop {
            var: ref v,
            body: ref e,
        } if *v == var => step(*e.clone()),

        // Dead dup: x not mentioned anywhere in body
        ref b if !mentions(&var, b) => body,

        // Commute past Let when x is not used in the value expression.
        // This pushes the Dup closer to its use and may expose cancellation.
        rc::Expr::Let {
            name,
            value,
            body: inner,
        } if !mentions(&var, &value) => rc::Expr::Let {
            name,
            value,
            body: Box::new(reduce_dup(var, *inner)),
        },

        body => rc::Expr::Dup {
            var,
            body: Box::new(body),
        },
    }
}

fn reduce_drop(var: String, body: rc::Expr) -> rc::Expr {
    match body {
        // Drop(x, Dup(x, e))  →  e
        rc::Expr::Dup {
            var: ref v,
            body: ref e,
        } if *v == var => step(*e.clone()),

        // Commute past Let when x is not used in the value expression.
        rc::Expr::Let {
            name,
            value,
            body: inner,
        } if !mentions(&var, &value) => rc::Expr::Let {
            name,
            value,
            body: Box::new(reduce_drop(var, *inner)),
        },

        body => rc::Expr::Drop {
            var,
            body: Box::new(body),
        },
    }
}

// ── Structural helpers ────────────────────────────────────────────────────────

/// Returns true if `var` appears anywhere in `expr` — as a variable reference,
/// a dup target, a drop target, a capture, or a reuse token.
fn mentions(var: &str, expr: &rc::Expr) -> bool {
    match expr {
        rc::Expr::Lit(_) => false,

        rc::Expr::Var(x) => x == var,

        rc::Expr::Dup { var: v, body } | rc::Expr::Drop { var: v, body } => {
            v == var || mentions(var, body)
        }

        rc::Expr::Let { name, value, body } => {
            mentions(var, value) || (name != var && mentions(var, body))
        }

        rc::Expr::Lam {
            param, captures, ..
        } => param != var && captures.iter().any(|c| c == var),

        rc::Expr::App(f, arg) => mentions(var, f) || mentions(var, arg),

        rc::Expr::If { cond, then_, else_ } => {
            mentions(var, cond) || mentions(var, then_) || mentions(var, else_)
        }

        rc::Expr::Match { scrutinee, arms } => {
            scrutinee == var
                || arms.iter().any(|arm| {
                    !arm.bindings.contains(&var.to_string())
                        && (arm.reuse_token.as_deref() == Some(var) || mentions(var, &arm.body))
                })
        }

        rc::Expr::Con { fields, reuse, .. } => {
            reuse.as_deref() == Some(var) || fields.iter().any(|f| mentions(var, f))
        }
        rc::Expr::Foreign { args, .. } => {
            // any arg
            args.iter().any(|a| mentions(var, a))
        }
    }
}

/// Public re-export so callers can check whether simplification changed anything.
pub fn structurally_equal_pub(a: &rc::Expr, b: &rc::Expr) -> bool {
    structurally_equal(a, b)
}

/// Cheap structural equality check used for fixpoint detection.
/// We only need to know "did anything change?", so a recursive
/// PartialEq implementation is sufficient.
fn structurally_equal(a: &rc::Expr, b: &rc::Expr) -> bool {
    match (a, b) {
        (rc::Expr::Lit(l1), rc::Expr::Lit(l2)) => format!("{l1:?}") == format!("{l2:?}"),
        (rc::Expr::Var(x), rc::Expr::Var(y)) => x == y,
        (rc::Expr::Dup { var: v1, body: b1 }, rc::Expr::Dup { var: v2, body: b2 }) => {
            v1 == v2 && structurally_equal(b1, b2)
        }
        (rc::Expr::Drop { var: v1, body: b1 }, rc::Expr::Drop { var: v2, body: b2 }) => {
            v1 == v2 && structurally_equal(b1, b2)
        }
        (
            rc::Expr::Let {
                name: n1,
                value: v1,
                body: b1,
            },
            rc::Expr::Let {
                name: n2,
                value: v2,
                body: b2,
            },
        ) => n1 == n2 && structurally_equal(v1, v2) && structurally_equal(b1, b2),
        (
            rc::Expr::Lam {
                param: p1,
                captures: c1,
                body: b1,
            },
            rc::Expr::Lam {
                param: p2,
                captures: c2,
                body: b2,
            },
        ) => p1 == p2 && c1 == c2 && structurally_equal(b1, b2),
        (rc::Expr::App(f1, a1), rc::Expr::App(f2, a2)) => {
            structurally_equal(f1, f2) && structurally_equal(a1, a2)
        }
        (
            rc::Expr::If {
                cond: c1,
                then_: t1,
                else_: e1,
            },
            rc::Expr::If {
                cond: c2,
                then_: t2,
                else_: e2,
            },
        ) => structurally_equal(c1, c2) && structurally_equal(t1, t2) && structurally_equal(e1, e2),
        (
            rc::Expr::Match {
                scrutinee: s1,
                arms: a1,
            },
            rc::Expr::Match {
                scrutinee: s2,
                arms: a2,
            },
        ) => {
            s1 == s2
                && a1.len() == a2.len()
                && a1.iter().zip(a2.iter()).all(|(arm1, arm2)| {
                    arm1.tag == arm2.tag
                        && arm1.bindings == arm2.bindings
                        && structurally_equal(&arm1.body, &arm2.body)
                })
        }
        (
            rc::Expr::Con {
                tag: t1,
                fields: f1,
                reuse: r1,
            },
            rc::Expr::Con {
                tag: t2,
                fields: f2,
                reuse: r2,
            },
        ) => {
            t1 == t2
                && r1 == r2
                && f1.len() == f2.len()
                && f1
                    .iter()
                    .zip(f2.iter())
                    .all(|(a, b)| structurally_equal(a, b))
        }
        (rc::Expr::Foreign { name: n1, args: a1 }, rc::Expr::Foreign { name: n2, args: a2 }) => {
            n1 == n2
                && a1.len() == a2.len()
                && a1
                    .iter()
                    .zip(a2.iter())
                    .all(|(a, b)| structurally_equal(a, b))
        }
        _ => false,
    }
}
