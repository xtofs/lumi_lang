/// RcExpr simplification pass.
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
use crate::rc_ast::{RcExpr, RcMatchArm};

// ── Public API ────────────────────────────────────────────────────────────────

/// Simplify `expr`, iterating until no rule applies.
pub fn simplify(expr: RcExpr) -> RcExpr {
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
fn step(expr: RcExpr) -> RcExpr {
    match expr {
        RcExpr::Dup { var, body } => {
            let body = step(*body);
            reduce_dup(var, body)
        }

        RcExpr::Drop { var, body } => {
            let body = step(*body);
            reduce_drop(var, body)
        }

        RcExpr::Let { name, value, body } => RcExpr::Let {
            name,
            value: Box::new(step(*value)),
            body: Box::new(step(*body)),
        },

        RcExpr::Lam {
            param,
            captures,
            body,
        } => RcExpr::Lam {
            param,
            captures,
            body: Box::new(step(*body)),
        },

        RcExpr::App(f, arg) => RcExpr::App(Box::new(step(*f)), Box::new(step(*arg))),

        RcExpr::If { cond, then_, else_ } => RcExpr::If {
            cond: Box::new(step(*cond)),
            then_: Box::new(step(*then_)),
            else_: Box::new(step(*else_)),
        },

        RcExpr::Match { scrutinee, arms } => RcExpr::Match {
            scrutinee,
            arms: arms
                .into_iter()
                .map(|arm| RcMatchArm {
                    body: step(arm.body),
                    ..arm
                })
                .collect(),
        },

        RcExpr::Con { tag, fields, reuse } => RcExpr::Con {
            tag,
            fields: fields.into_iter().map(step).collect(),
            reuse,
        },

        leaf => leaf,
    }
}

// ── Reduction helpers ─────────────────────────────────────────────────────────

fn reduce_dup(var: String, body: RcExpr) -> RcExpr {
    match body {
        // Dup(x, Drop(x, e))  →  e
        RcExpr::Drop {
            var: ref v,
            body: ref e,
        } if *v == var => step(*e.clone()),

        // Dead dup: x not mentioned anywhere in body
        ref b if !mentions(&var, b) => body,

        // Commute past Let when x is not used in the value expression.
        // This pushes the Dup closer to its use and may expose cancellation.
        RcExpr::Let {
            name,
            value,
            body: inner,
        } if !mentions(&var, &value) => RcExpr::Let {
            name,
            value,
            body: Box::new(reduce_dup(var, *inner)),
        },

        body => RcExpr::Dup {
            var,
            body: Box::new(body),
        },
    }
}

fn reduce_drop(var: String, body: RcExpr) -> RcExpr {
    match body {
        // Drop(x, Dup(x, e))  →  e
        RcExpr::Dup {
            var: ref v,
            body: ref e,
        } if *v == var => step(*e.clone()),

        // Commute past Let when x is not used in the value expression.
        RcExpr::Let {
            name,
            value,
            body: inner,
        } if !mentions(&var, &value) => RcExpr::Let {
            name,
            value,
            body: Box::new(reduce_drop(var, *inner)),
        },

        body => RcExpr::Drop {
            var,
            body: Box::new(body),
        },
    }
}

// ── Structural helpers ────────────────────────────────────────────────────────

/// Returns true if `var` appears anywhere in `expr` — as a variable reference,
/// a dup target, a drop target, a capture, or a reuse token.
fn mentions(var: &str, expr: &RcExpr) -> bool {
    match expr {
        RcExpr::Lit(_) => false,

        RcExpr::Var(x) => x == var,

        RcExpr::Dup { var: v, body } | RcExpr::Drop { var: v, body } => {
            v == var || mentions(var, body)
        }

        RcExpr::Let { name, value, body } => {
            mentions(var, value) || (name != var && mentions(var, body))
        }

        RcExpr::Lam {
            param, captures, ..
        } => param != var && captures.iter().any(|c| c == var),

        RcExpr::App(f, arg) => mentions(var, f) || mentions(var, arg),

        RcExpr::If { cond, then_, else_ } => {
            mentions(var, cond) || mentions(var, then_) || mentions(var, else_)
        }

        RcExpr::Match { scrutinee, arms } => {
            scrutinee == var
                || arms.iter().any(|arm| {
                    !arm.bindings.contains(&var.to_string())
                        && (arm.reuse_token.as_deref() == Some(var) || mentions(var, &arm.body))
                })
        }

        RcExpr::Con { fields, reuse, .. } => {
            reuse.as_deref() == Some(var) || fields.iter().any(|f| mentions(var, f))
        }
        RcExpr::Foreign { args, .. } => {
            // any arg
            args.iter().any(|a| mentions(var, a))
        }
    }
}

/// Public re-export so callers can check whether simplification changed anything.
pub fn structurally_equal_pub(a: &RcExpr, b: &RcExpr) -> bool {
    structurally_equal(a, b)
}

/// Cheap structural equality check used for fixpoint detection.
/// We only need to know "did anything change?", so a recursive
/// PartialEq implementation is sufficient.
fn structurally_equal(a: &RcExpr, b: &RcExpr) -> bool {
    match (a, b) {
        (RcExpr::Lit(l1), RcExpr::Lit(l2)) => format!("{l1:?}") == format!("{l2:?}"),
        (RcExpr::Var(x), RcExpr::Var(y)) => x == y,
        (RcExpr::Dup { var: v1, body: b1 }, RcExpr::Dup { var: v2, body: b2 }) => {
            v1 == v2 && structurally_equal(b1, b2)
        }
        (RcExpr::Drop { var: v1, body: b1 }, RcExpr::Drop { var: v2, body: b2 }) => {
            v1 == v2 && structurally_equal(b1, b2)
        }
        (
            RcExpr::Let {
                name: n1,
                value: v1,
                body: b1,
            },
            RcExpr::Let {
                name: n2,
                value: v2,
                body: b2,
            },
        ) => n1 == n2 && structurally_equal(v1, v2) && structurally_equal(b1, b2),
        (
            RcExpr::Lam {
                param: p1,
                captures: c1,
                body: b1,
            },
            RcExpr::Lam {
                param: p2,
                captures: c2,
                body: b2,
            },
        ) => p1 == p2 && c1 == c2 && structurally_equal(b1, b2),
        (RcExpr::App(f1, a1), RcExpr::App(f2, a2)) => {
            structurally_equal(f1, f2) && structurally_equal(a1, a2)
        }
        (
            RcExpr::If {
                cond: c1,
                then_: t1,
                else_: e1,
            },
            RcExpr::If {
                cond: c2,
                then_: t2,
                else_: e2,
            },
        ) => structurally_equal(c1, c2) && structurally_equal(t1, t2) && structurally_equal(e1, e2),
        (
            RcExpr::Match {
                scrutinee: s1,
                arms: a1,
            },
            RcExpr::Match {
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
            RcExpr::Con {
                tag: t1,
                fields: f1,
                reuse: r1,
            },
            RcExpr::Con {
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
        (RcExpr::Foreign { name: n1, args: a1 }, RcExpr::Foreign { name: n2, args: a2 }) => {
            n1 == n2
                && a1.len() == a2.len()
                && a1.iter().zip(a2.iter()).all(|(a, b)| structurally_equal(a, b))
        }
        _ => false,
    }
}
