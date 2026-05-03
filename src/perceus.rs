/// Perceus RC-insertion pass.
///
/// Transforms the source AST into an RC-annotated AST by inserting
/// `Dup` and `Drop` nodes at statically correct positions, and
/// attaching `ReuseToken`s to match arms so that constructors in the
/// arm body can recycle the scrutinee's heap allocation.
///
/// The algorithm in one sentence:
///   For every variable, insert Dup where it is shared, Drop where it
///   dies without being consumed, and pass ownership directly where a
///   single unique use exists.
///
/// Reference: "Perceus: Garbage Free Reference Counting with Reuse"
///            Reijnders & Leijen, MSR-TR-2020-42.

use std::collections::HashSet;

use crate::ast::{Expr, Pattern};
use crate::liveness::{free_vars, free_vars_arm, pat_bindings, pat_bindings_ordered, use_count};
use crate::rc_ast::{RcExpr, RcMatchArm};

/// Entry point: transform a top-level expression.
pub fn transform(expr: &Expr) -> RcExpr {
    // No variables are owned at the top level.
    xform(expr, &HashSet::new())
}

// ── Core recursive transform ─────────────────────────────────────────────────

fn xform(expr: &Expr, owned: &HashSet<String>) -> RcExpr {
    match expr {
        // ── Literals and variables are trivially owned ──────────────────────
        Expr::Lit(l) => RcExpr::Lit(l.clone()),

        Expr::Var(x) => RcExpr::Var(x.clone()),

        // ── Let ──────────────────────────────────────────────────────────────
        // The bound name is owned in the body.
        // If it is never used in the body, insert a Drop immediately.
        Expr::Let { name, value, body } => {
            let rc_value = xform(value, owned);

            let mut body_owned = owned.clone();
            body_owned.insert(name.clone());

            let rc_body = xform(body, &body_owned);

            let rc_body = if use_count(name, body) == 0 {
                RcExpr::drop_(name, rc_body)
            } else {
                rc_body
            };

            RcExpr::Let {
                name: name.clone(),
                value: Box::new(rc_value),
                body: Box::new(rc_body),
            }
        }

        // ── Lambda ──────────────────────────────────────────────────────────
        // The parameter is owned inside the body.
        Expr::Lam { param, body } => {
            let mut body_owned = owned.clone();
            body_owned.insert(param.clone());

            let rc_body = xform(body, &body_owned);

            let rc_body = if use_count(param, body) == 0 {
                RcExpr::drop_(param, rc_body)
            } else {
                rc_body
            };

            RcExpr::Lam { param: param.clone(), body: Box::new(rc_body) }
        }

        // ── Application ─────────────────────────────────────────────────────
        // We transform both sub-expressions.  If the function and argument
        // share an owned variable, that variable must be Dup'd.
        Expr::App(f, arg) => {
            let fvs_f = free_vars(f);
            let fvs_arg = free_vars(arg);

            // Variables needed by both sub-expressions must be duplicated
            // before either sub-expression consumes them.
            let shared: HashSet<&String> = fvs_f.intersection(&fvs_arg).collect();

            let rc_f = xform(f, owned);
            let rc_arg = xform(arg, owned);

            let mut result = RcExpr::App(Box::new(rc_f), Box::new(rc_arg));

            for var in &shared {
                if owned.contains(*var) {
                    result = RcExpr::dup(var, result);
                }
            }

            result
        }

        // ── If ──────────────────────────────────────────────────────────────
        // Variables live in both branches must be Dup'd; variables live in
        // only one branch are transferred into that branch (last use).
        Expr::If { cond, then_, else_ } => {
            let fvs_then = free_vars(then_);
            let fvs_else = free_vars(else_);

            // Owned variables consumed in both branches need a dup.
            let in_both: HashSet<String> = fvs_then
                .intersection(&fvs_else)
                .filter(|v| owned.contains(*v))
                .cloned()
                .collect();

            // Variables dead after the condition but alive in only one branch
            // need to be dropped in the other.
            let only_then: HashSet<String> = fvs_then
                .difference(&fvs_else)
                .filter(|v| owned.contains(*v))
                .cloned()
                .collect();
            let only_else: HashSet<String> = fvs_else
                .difference(&fvs_then)
                .filter(|v| owned.contains(*v))
                .cloned()
                .collect();

            let rc_cond = xform(cond, owned);

            let mut rc_then = xform(then_, owned);
            let mut rc_else = xform(else_, owned);

            // Drop variables that are alive in the else branch but not this one.
            for var in &only_else {
                rc_then = RcExpr::drop_(var, rc_then);
            }
            for var in &only_then {
                rc_else = RcExpr::drop_(var, rc_else);
            }

            let mut result = RcExpr::If {
                cond: Box::new(rc_cond),
                then_: Box::new(rc_then),
                else_: Box::new(rc_else),
            };

            // Dup shared variables before the branch.
            for var in &in_both {
                result = RcExpr::dup(var, result);
            }

            result
        }

        // ── Match ───────────────────────────────────────────────────────────
        // The scrutinee is consumed by the match.
        // If it is not already a simple variable, bind it to a fresh let.
        Expr::Match { scrutinee, arms } => {
            match scrutinee.as_ref() {
                Expr::Var(x) => xform_match(x, arms, owned),
                _ => {
                    // Bind the scrutinee to a temporary variable.
                    let tmp = "__scrut".to_string();
                    let rc_scrut = xform(scrutinee, owned);
                    let mut tmp_owned = owned.clone();
                    tmp_owned.insert(tmp.clone());
                    let rc_match = xform_match(&tmp, arms, &tmp_owned);
                    RcExpr::Let {
                        name: tmp,
                        value: Box::new(rc_scrut),
                        body: Box::new(rc_match),
                    }
                }
            }
        }

        // ── Constructor ─────────────────────────────────────────────────────
        // No reuse token at this point — reuse is attached by xform_match
        // when a Con appears directly inside a match arm.
        Expr::Con { tag, fields } => {
            let rc_fields = fields.iter().map(|f| xform(f, owned)).collect();
            RcExpr::Con { tag: tag.clone(), fields: rc_fields, reuse: None }
        }
    }
}

// ── Match helper ─────────────────────────────────────────────────────────────

fn xform_match(
    scrutinee: &str,
    arms: &[crate::ast::MatchArm],
    owned: &HashSet<String>,
) -> RcExpr {
    // Collect free variables per arm (excluding the arm's own bindings).
    let arm_fvs: Vec<HashSet<String>> =
        arms.iter().map(|a| free_vars_arm(a)).collect();

    // A variable that is free in multiple arms must be Dup'd once per
    // extra arm that uses it.
    let rc_arms: Vec<RcMatchArm> = arms
        .iter()
        .enumerate()
        .map(|(i, arm)| {
            let this_fvs = &arm_fvs[i];

            // Which owned variables are also needed by at least one other arm?
            let needed_elsewhere: HashSet<String> = arm_fvs
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .flat_map(|(_, fvs)| fvs.iter().cloned())
                .filter(|v| owned.contains(v) && this_fvs.contains(v))
                .collect();

            let bindings = pat_bindings_ordered(&arm.pat);

            // The reuse token: names the scrutinee's allocation.
            // At runtime, if RC == 1 the allocation is recycled; otherwise
            // a fresh malloc is used.  We always emit the token here and
            // let the C runtime decide.
            let reuse_token = reuse_token_for(scrutinee, &arm.pat);

            let mut arm_owned = owned.clone();
            for b in &bindings {
                arm_owned.insert(b.clone());
            }
            // The scrutinee is consumed by the match; remove it from owned.
            arm_owned.remove(scrutinee);

            let mut rc_body = xform_with_reuse(&arm.body, &arm_owned, &reuse_token);

            // Dup variables that are also consumed in other arms.
            for var in &needed_elsewhere {
                rc_body = RcExpr::dup(var, rc_body);
            }

            // Drop owned variables that are not used in this arm at all.
            for var in owned {
                if var == scrutinee {
                    continue; // handled via try_reuse in the match itself
                }
                if !this_fvs.contains(var) && !needed_elsewhere.contains(var) {
                    rc_body = RcExpr::drop_(var, rc_body);
                }
            }

            let tag = arm_tag(&arm.pat);

            RcMatchArm { tag, bindings, reuse_token, body: rc_body }
        })
        .collect();

    RcExpr::Match { scrutinee: scrutinee.to_string(), arms: rc_arms }
}

/// Like `xform`, but if the expression is a `Con` and we have a reuse token,
/// attach the token to that Con so the codegen can emit `reuse_con(...)`.
///
/// This is the heart of Perceus: the compiler threads the reuse token from
/// the match arm directly into the constructor call, eliminating malloc.
fn xform_with_reuse(
    expr: &Expr,
    owned: &HashSet<String>,
    reuse_token: &Option<String>,
) -> RcExpr {
    match expr {
        Expr::Con { tag, fields } => {
            let rc_fields = fields.iter().map(|f| xform(f, owned)).collect();
            RcExpr::Con {
                tag: tag.clone(),
                fields: rc_fields,
                reuse: reuse_token.clone(),
            }
        }
        _ => xform(expr, owned),
    }
}

// ── Small utilities ──────────────────────────────────────────────────────────

fn arm_tag(pat: &Pattern) -> String {
    match pat {
        Pattern::Con { tag, .. } => tag.clone(),
        Pattern::Var(x) => format!("_{x}"),
        Pattern::Wildcard => "_".to_string(),
        Pattern::Lit(_) => "lit".to_string(),
    }
}

/// Returns a reuse token name when the pattern is a constructor match
/// (i.e., the scrutinee's allocation *could* be recycled).
fn reuse_token_for(scrutinee: &str, pat: &Pattern) -> Option<String> {
    match pat {
        Pattern::Con { .. } => Some(format!("reuse_{scrutinee}")),
        _ => None, // wildcard / literal / var — no constructor to reuse
    }
}
