/// RC-annotated AST — output of the Perceus transform.
///
/// This is an intermediate representation where every reference-counting
/// operation is made explicit. The Perceus algorithm decides *where* to
/// insert Dup and Drop so the user never has to.
use crate::ast::Lit;

/// A reuse token names the memory of a just-deconstructed value.
/// If the token is "live" (RC was 1 at deconstruction time), the
/// Con node that consumes it can skip malloc and write into that slot.
pub type ReuseToken = String;

#[derive(Debug, Clone)]
pub enum RcExpr {
    Lit(Lit),

    Var(String),

    Let {
        name: String,
        value: Box<RcExpr>,
        body: Box<RcExpr>,
    },

    Lam {
        param: String,
        body: Box<RcExpr>,
    },

    App(Box<RcExpr>, Box<RcExpr>),

    If {
        cond: Box<RcExpr>,
        then_: Box<RcExpr>,
        else_: Box<RcExpr>,
    },

    /// rc_inc(var); evaluate body.
    /// Inserted when a variable is shared across multiple uses.
    Dup {
        var: String,
        body: Box<RcExpr>,
    },

    /// rc_dec(var); evaluate body.
    /// Inserted when a variable goes out of scope without being consumed.
    Drop {
        var: String,
        body: Box<RcExpr>,
    },

    /// Pattern match. The scrutinee is *consumed* (ownership transferred in).
    /// Each arm may carry a ReuseToken for the scrutinee's freed allocation.
    Match {
        scrutinee: String,
        arms: Vec<RcMatchArm>,
    },

    /// Constructor call.
    /// If `reuse` is Some(token), the emitted C reuses that allocation
    /// instead of calling malloc — the core Perceus optimisation.
    Con {
        tag: String,
        fields: Vec<RcExpr>,
        reuse: Option<ReuseToken>,
    },
}

#[derive(Debug, Clone)]
pub struct RcMatchArm {
    /// The constructor tag this arm matches (or "_" for wildcard).
    pub tag: String,
    /// Field variables extracted from the matched constructor.
    pub bindings: Vec<String>,
    /// Reuse token for the scrutinee's allocation, usable by a Con in this arm.
    pub reuse_token: Option<ReuseToken>,
    pub body: RcExpr,
}

// ── Builder helpers ───────────────────────────────────────────────────────────

impl RcExpr {
    pub fn var(name: &str) -> Self {
        RcExpr::Var(name.to_string())
    }

    pub fn dup(var: &str, body: RcExpr) -> Self {
        RcExpr::Dup {
            var: var.to_string(),
            body: Box::new(body),
        }
    }

    pub fn drop_(var: &str, body: RcExpr) -> Self {
        RcExpr::Drop {
            var: var.to_string(),
            body: Box::new(body),
        }
    }

    pub fn pp(&self, w: &mut dyn std::io::Write) -> std::io::Result<()> {
        self.pp_with_indent(w, 0)?;
        writeln!(w)
    }

    fn pp_with_indent(&self, w: &mut dyn std::io::Write, indent: usize) -> std::io::Result<()> {
        let i0 = "  ".repeat(indent);
        let i1 = "  ".repeat(indent + 1);
        let i2 = "  ".repeat(indent + 2);
        match self {
            RcExpr::Lit(lit) => write!(w, "{lit:?}"),
            RcExpr::Var(name) => write!(w, "{name}"),
            RcExpr::Dup { var, body } => {
                write!(w, "dup({var}); ")?;
                body.pp_with_indent(w, indent)
            }
            RcExpr::Drop { var, body } => {
                write!(w, "drop({var}); ")?;
                body.pp_with_indent(w, indent)
            }
            RcExpr::Let { name, value, body } => {
                write!(w, "let {name} =\n{i1}")?;
                value.pp_with_indent(w, indent + 1)?;
                write!(w, "\n{i0}in\n{i1}")?;
                body.pp_with_indent(w, indent + 1)
            }
            RcExpr::Lam { param, body } => {
                write!(w, "λ{param} =>\n{i1}")?;
                body.pp_with_indent(w, indent + 1)
            }
            RcExpr::App(f, arg) => {
                write!(w, "(")?;
                f.pp_with_indent(w, indent)?;
                write!(w, " ")?;
                arg.pp_with_indent(w, indent)?;
                write!(w, ")")
            }
            RcExpr::If { cond, then_, else_ } => {
                write!(w, "if ")?;
                cond.pp_with_indent(w, indent)?;
                write!(w, "\n{i0}then ")?;
                then_.pp_with_indent(w, indent)?;
                write!(w, "\n{i0}else ")?;
                else_.pp_with_indent(w, indent)
            }
            RcExpr::Match { scrutinee, arms } => {
                write!(w, "match {scrutinee}")?;
                for arm in arms {
                    let bindings = arm.bindings.join(", ");
                    let reuse = arm
                        .reuse_token
                        .as_deref()
                        .map(|t| format!(" [reuse: {t}]"))
                        .unwrap_or_default();
                    write!(w, "\n{i1}| {}({bindings}){reuse} =>\n{i2}", arm.tag)?;
                    arm.body.pp_with_indent(w, indent + 2)?;
                }
                Ok(())
            }
            RcExpr::Con { tag, fields, reuse } => {
                let reuse = reuse
                    .as_deref()
                    .map(|t| format!(" [reuse: {t}]"))
                    .unwrap_or_default();
                write!(w, "{tag}(")?;
                for (i, f) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(w, ", ")?;
                    }
                    f.pp_with_indent(w, indent)?;
                }
                write!(w, "){reuse}")
            }
        }
    }
}
