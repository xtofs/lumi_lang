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
        // The parameter is owned inside the body; everything else the body
        // uses must be captured from the enclosing scope.
        //
        // We do NOT insert Dup nodes here for the captures.  Ownership
        // flows into the closure from wherever `f` currently lives.
        // If the *enclosing* context needs `f` to survive past the closure
        // creation, it will insert the Dup (App, If, Con, and Match already
        // do this for sub-expressions they share variables with).
        Expr::Lam { param, body } => {
            let mut body_owned = owned.clone();
            body_owned.insert(param.clone());

            let rc_body = xform(body, &body_owned);

            let rc_body = if use_count(param, body) == 0 {
                RcExpr::drop_(param, rc_body)
            } else {
                rc_body
            };

            let mut captures: Vec<String> = free_vars(body)
                .into_iter()
                .filter(|v| v != param && owned.contains(v))
                .collect();
            captures.sort();

            RcExpr::Lam {
                param: param.clone(),
                captures,
                body: Box::new(rc_body),
            }
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
        // Only one branch executes, so an owned variable used in BOTH branches
        // needs NO dup — whichever branch fires consumes the single copy.
        // A variable live in only one branch must be dropped in the other.
        Expr::If { cond, then_, else_ } => {
            let fvs_then = free_vars(then_);
            let fvs_else = free_vars(else_);

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

            for var in &only_else {
                rc_then = RcExpr::drop_(var, rc_then);
            }
            for var in &only_then {
                rc_else = RcExpr::drop_(var, rc_else);
            }

            RcExpr::If {
                cond: Box::new(rc_cond),
                then_: Box::new(rc_then),
                else_: Box::new(rc_else),
            }
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
        //
        // Fields are evaluated left-to-right: if an owned variable appears
        // in multiple fields, it is consumed by the first field and then
        // gone.  We must Dup it once for each extra field that uses it.
        Expr::Con { tag, fields } => {
            let field_fvs: Vec<HashSet<String>> =
                fields.iter().map(|f| free_vars(f)).collect();

            let rc_fields = fields.iter().map(|f| xform(f, owned)).collect();
            let mut result = RcExpr::Con { tag: tag.clone(), fields: rc_fields, reuse: None };

            // Sort for determinism.
            let mut vars: Vec<&String> = owned.iter().collect();
            vars.sort();
            for var in vars {
                let uses = field_fvs.iter().filter(|fvs| fvs.contains(var)).count();
                for _ in 1..uses {
                    result = RcExpr::dup(var, result);
                }
            }
            result
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

    // Only one arm fires, so there is no Dup for "variables used in other arms".
    // Each arm either uses an owned variable (consumes it) or drops it.
    let rc_arms: Vec<RcMatchArm> = arms
        .iter()
        .enumerate()
        .map(|(i, arm)| {
            let this_fvs = &arm_fvs[i];

            let bindings = pat_bindings_ordered(&arm.pat);
            let reuse_token = reuse_token_for(scrutinee, &arm.pat);

            let mut arm_owned = owned.clone();
            for b in &bindings {
                arm_owned.insert(b.clone());
            }
            arm_owned.remove(scrutinee);

            let mut rc_body = xform_with_reuse(&arm.body, &arm_owned, &reuse_token);

            // Drop owned variables not used in this arm — they die here.
            for var in owned {
                if var == scrutinee {
                    continue;
                }
                if !this_fvs.contains(var) {
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
            let field_fvs: Vec<HashSet<String>> =
                fields.iter().map(|f| free_vars(f)).collect();

            let rc_fields = fields.iter().map(|f| xform(f, owned)).collect();
            let mut result = RcExpr::Con {
                tag: tag.clone(),
                fields: rc_fields,
                reuse: reuse_token.clone(),
            };

            let mut vars: Vec<&String> = owned.iter().collect();
            vars.sort();
            for var in vars {
                let uses = field_fvs.iter().filter(|fvs| fvs.contains(var)).count();
                for _ in 1..uses {
                    result = RcExpr::dup(var, result);
                }
            }
            result
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
