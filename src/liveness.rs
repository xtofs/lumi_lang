/// Liveness / use-count analysis on the source AST.
///
/// Perceus needs to know, for each variable at each program point:
///   - Is this the *last* use? (if so, ownership can be transferred — no dup)
///   - Is it used in multiple branches? (if so, we need a dup before the branch)
///
/// We approximate this with simple syntactic use-counting.
/// A production implementation would use dataflow analysis.

use std::collections::HashSet;
use crate::ast::{Expr, Lit, Pattern, MatchArm};

/// Free variables of an expression (variables not bound within it).
pub fn free_vars(expr: &Expr) -> HashSet<String> {
    let mut fvs = HashSet::new();
    collect_fvs(expr, &mut fvs);
    fvs
}

fn collect_fvs(expr: &Expr, out: &mut HashSet<String>) {
    match expr {
        Expr::Lit(_) => {}

        Expr::Var(x) => {
            out.insert(x.clone());
        }

        Expr::Let { name, value, body } => {
            collect_fvs(value, out);
            let mut body_fvs = HashSet::new();
            collect_fvs(body, &mut body_fvs);
            body_fvs.remove(name);
            out.extend(body_fvs);
        }

        Expr::Lam { param, body } => {
            let mut body_fvs = HashSet::new();
            collect_fvs(body, &mut body_fvs);
            body_fvs.remove(param);
            out.extend(body_fvs);
        }

        Expr::App(f, x) => {
            collect_fvs(f, out);
            collect_fvs(x, out);
        }

        Expr::If { cond, then_, else_ } => {
            collect_fvs(cond, out);
            // A variable used in either branch is free in the whole if.
            collect_fvs(then_, out);
            collect_fvs(else_, out);
        }

        Expr::Match { scrutinee, arms } => {
            collect_fvs(scrutinee, out);
            for arm in arms {
                let arm_fvs = free_vars_arm(arm);
                out.extend(arm_fvs);
            }
        }

        Expr::Con { fields, .. } => {
            for f in fields {
                collect_fvs(f, out);
            }
        }
    }
}

pub fn free_vars_arm(arm: &MatchArm) -> HashSet<String> {
    let mut fvs = HashSet::new();
    collect_fvs(&arm.body, &mut fvs);
    for b in pat_bindings(&arm.pat) {
        fvs.remove(&b);
    }
    fvs
}

/// Variables bound by a pattern.
pub fn pat_bindings(pat: &Pattern) -> HashSet<String> {
    let mut bs = HashSet::new();
    collect_pat_bindings(pat, &mut bs);
    bs
}

fn collect_pat_bindings(pat: &Pattern, out: &mut HashSet<String>) {
    match pat {
        Pattern::Var(x) => {
            out.insert(x.clone());
        }
        Pattern::Con { fields, .. } => {
            for f in fields {
                collect_pat_bindings(f, out);
            }
        }
        _ => {}
    }
}

/// Ordered list of bindings introduced by a pattern (left-to-right field order).
pub fn pat_bindings_ordered(pat: &Pattern) -> Vec<String> {
    let mut bs = Vec::new();
    collect_ordered(pat, &mut bs);
    bs
}

fn collect_ordered(pat: &Pattern, out: &mut Vec<String>) {
    match pat {
        Pattern::Var(x) => out.push(x.clone()),
        Pattern::Con { fields, .. } => {
            for f in fields {
                collect_ordered(f, out);
            }
        }
        _ => {}
    }
}

/// How many times does `var` appear (syntactically) in `expr`?
/// Branches are counted as max(then, else) — if the variable is needed
/// in one branch it must be alive entering the if.
pub fn use_count(var: &str, expr: &Expr) -> usize {
    match expr {
        Expr::Lit(_) => 0,

        Expr::Var(x) => usize::from(x == var),

        Expr::Let { name, value, body } => {
            let v = use_count(var, value);
            let b = if name == var { 0 } else { use_count(var, body) };
            v + b
        }

        Expr::Lam { param, body } => {
            if param == var { 0 } else { use_count(var, body) }
        }

        Expr::App(f, x) => use_count(var, f) + use_count(var, x),

        Expr::If { cond, then_, else_ } => {
            // The variable is consumed along exactly one branch, but we need
            // a live copy entering both.  Take the max of the two branches.
            use_count(var, cond)
                + use_count(var, then_).max(use_count(var, else_))
        }

        Expr::Match { scrutinee, arms } => {
            let s = use_count(var, scrutinee);
            // Same branch reasoning as If.
            let arm_max = arms
                .iter()
                .map(|a| {
                    if pat_bindings(&a.pat).contains(var) {
                        0 // shadowed inside this arm
                    } else {
                        use_count(var, &a.body)
                    }
                })
                .max()
                .unwrap_or(0);
            s + arm_max
        }

        Expr::Con { fields, .. } => fields.iter().map(|f| use_count(var, f)).sum(),
    }
}
