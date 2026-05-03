///
/// This is an intermediate representation where every reference-counting
/// operation is made explicit. The Perceus algorithm decides *where* to
/// insert Dup and Drop so the user never has to.
use crate::ast::Lit;
//

/// A reuse token names the memory of a just-deconstructed value.
/// If the token is "live" (RC was 1 at deconstruction time), the
/// Con node that consumes it can skip malloc and write into that slot.
pub type ReuseToken = String;

#[derive(Debug, Clone)]
pub enum Expr {
    Lit(Lit),   // Lit us shared with ast. there is not reference counting 

    Var(String),

    Let {
        name: String,
        value: Box<Expr>,
        body: Box<Expr>,
    },

    Lam {
        param: String,
        /// Variables captured from the enclosing scope.
        /// Each one has already been Dup'd at the closure-creation site.
        captures: Vec<String>,
        body: Box<Expr>,
    },

    App(Box<Expr>, Box<Expr>),

    If {
        cond: Box<Expr>,
        then_: Box<Expr>,
        else_: Box<Expr>,
    },

    /// rc_inc(var); evaluate body.
    /// Inserted when a variable is shared across multiple uses.
    Dup {
        var: String,
        body: Box<Expr>,
    },

    /// rc_dec(var); evaluate body.
    /// Inserted when a variable goes out of scope without being consumed.
    Drop {
        var: String,
        body: Box<Expr>,
    },

    /// Pattern match. The scrutinee is *consumed* (ownership transferred in).
    /// Each arm may carry a ReuseToken for the scrutinee's freed allocation.
    Match {
        scrutinee: String,
        arms: Vec<MatchArm>,
    },

    /// Constructor call.
    /// If `reuse` is Some(token), the emitted C reuses that allocation
    /// instead of calling malloc — the core Perceus optimisation.
    Con {
        tag: String,
        fields: Vec<Expr>,
        reuse: Option<ReuseToken>,
    },

    /// Direct call to a named C function — escape hatch for I/O / arithmetic.
    Foreign {
        name: String,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    /// The constructor tag this arm matches (or "_" for wildcard).
    pub tag: String,
    /// Field variables extracted from the matched constructor.
    pub bindings: Vec<String>,
    /// Reuse token for the scrutinee's allocation, usable by a Con in this arm.
    pub reuse_token: Option<ReuseToken>,
    pub body: Expr,
}

// ── Builder helpers ───────────────────────────────────────────────────────────

impl Expr {
    pub fn var(name: &str) -> Self {
        Expr::Var(name.to_string())
    }

    pub fn dup(var: &str, body: Expr) -> Self {
        Expr::Dup {
            var: var.to_string(),
            body: Box::new(body),
        }
    }

    pub fn drop_(var: &str, body: Expr) -> Self {
        Expr::Drop {
            var: var.to_string(),
            body: Box::new(body),
        }
    }
}
