use crate::lib::PrettyPrintStyle;
/// Source AST for Lumi — a small functional language.
/// Programs are constructed programmatically; no parser yet.

#[derive(Debug, Clone)]
pub enum Lit {
    Int(i64),
    Bool(bool),
    Unit,
    Str(String),
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard,
    Var(String),
    Lit(Lit),
    Con { tag: String, fields: Vec<Pattern> },
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pat: Pattern,
    pub body: Expr,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Lit(Lit),
    Var(String),
    Let {
        name: String,
        value: Box<Expr>,
        body: Box<Expr>,
    },
    Lam {
        param: String,
        body: Box<Expr>,
    },
    App(Box<Expr>, Box<Expr>),
    If {
        cond: Box<Expr>,
        then_: Box<Expr>,
        else_: Box<Expr>,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    /// Algebraic data type constructor: Tag(field0, field1, ...)
    Con {
        tag: String,
        fields: Vec<Expr>,
    },
    /// Direct call to a named C function — escape hatch for I/O / arithmetic.
    Foreign {
        name: String,
        args: Vec<Expr>,
    },
}

// ── Builder helpers so constructing programs stays readable ──────────────────

impl Expr {
    pub fn var(name: &str) -> Self {
        Expr::Var(name.to_string())
    }
    pub fn int(n: i64) -> Self {
        Expr::Lit(Lit::Int(n))
    }
    pub fn bool_(b: bool) -> Self {
        Expr::Lit(Lit::Bool(b))
    }
    pub fn unit() -> Self {
        Expr::Lit(Lit::Unit)
    }
    pub fn lam(param: &str, body: Expr) -> Self {
        Expr::Lam {
            param: param.to_string(),
            body: Box::new(body),
        }
    }
    pub fn app(f: Expr, x: Expr) -> Self {
        Expr::App(Box::new(f), Box::new(x))
    }
    pub fn let_(name: &str, value: Expr, body: Expr) -> Self {
        Expr::Let {
            name: name.to_string(),
            value: Box::new(value),
            body: Box::new(body),
        }
    }
    pub fn if_(cond: Expr, then_: Expr, else_: Expr) -> Self {
        Expr::If {
            cond: Box::new(cond),
            then_: Box::new(then_),
            else_: Box::new(else_),
        }
    }
    pub fn match_(scrutinee: Expr, arms: Vec<MatchArm>) -> Self {
        Expr::Match {
            scrutinee: Box::new(scrutinee),
            arms,
        }
    }
    pub fn con(tag: &str, fields: Vec<Expr>) -> Self {
        Expr::Con {
            tag: tag.to_string(),
            fields,
        }
    }
    pub fn str_(s: &str) -> Self {
        Expr::Lit(Lit::Str(s.to_string()))
    }
    pub fn foreign(name: &str, args: Vec<Expr>) -> Self {
        Expr::Foreign {
            name: name.to_string(),
            args,
        }
    }

    pub fn pp(&self, w: &mut dyn std::io::Write, style: PrettyPrintStyle) -> std::io::Result<()> {
        match style {
            PrettyPrintStyle::SingleLine => {
                self.pp_inner(w, false)?;
                writeln!(w)
            }
            PrettyPrintStyle::Indented => {
                self.pp_with_indent(w, 0)?;
                writeln!(w)
            }
        }
    }

    fn pp_inner(&self, w: &mut dyn std::io::Write, parens: bool) -> std::io::Result<()> {
        let needs_parens = parens && matches!(self, Expr::Lam { .. } | Expr::App(_, _));
        if needs_parens {
            write!(w, "(")?;
        }

        match self {
            Expr::Lit(lit) => match lit {
                Lit::Int(n) => write!(w, "{}", n)?,
                Lit::Bool(b) => write!(w, "{}", b)?,
                Lit::Unit => write!(w, "()")?,
                Lit::Str(s) => write!(w, "\"{}\"", s)?,
            },
            Expr::Var(name) => write!(w, "{}", name)?,
            Expr::Lam { param, body } => {
                write!(w, "λ{}.", param)?;
                body.pp_inner(w, false)?;
            }
            Expr::App(f, x) => {
                f.pp_inner(w, true)?;
                write!(w, " ")?;
                x.pp_inner(w, true)?;
            }
            Expr::Let { name, value, body } => {
                write!(w, "let {} = ", name)?;
                value.pp_inner(w, false)?;
                write!(w, " in ")?;
                body.pp_inner(w, false)?;
            }
            Expr::If { cond, then_, else_ } => {
                write!(w, "if ")?;
                cond.pp_inner(w, false)?;
                write!(w, " then ")?;
                then_.pp_inner(w, false)?;
                write!(w, " else ")?;
                else_.pp_inner(w, false)?;
            }
            Expr::Match { scrutinee, arms } => {
                write!(w, "match ")?;
                scrutinee.pp_inner(w, false)?;
                write!(w, " with ")?;
                for (i, arm) in arms.iter().enumerate() {
                    if i > 0 {
                        write!(w, " | ")?;
                    }
                    arm.pat.pretty_print(w)?;
                    write!(w, " -> ")?;
                    arm.body.pp_inner(w, false)?;
                }
            }
            Expr::Con { tag, fields } => {
                write!(w, "{}", tag)?;
                if !fields.is_empty() {
                    write!(w, "(")?;
                    for (i, field) in fields.iter().enumerate() {
                        if i > 0 {
                            write!(w, ", ")?;
                        }
                        field.pp_inner(w, false)?;
                    }
                    write!(w, ")")?;
                }
            }
            Expr::Foreign { name, args } => {
                write!(w, "{}(", name)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(w, ", ")?;
                    }
                    arg.pp_inner(w, false)?;
                }
                write!(w, ")")?;
            }
        }

        if needs_parens {
            write!(w, ")")?;
        }
        Ok(())
    }

    fn pp_with_indent(&self, w: &mut dyn std::io::Write, indent: usize) -> std::io::Result<()> {
        let i0 = "  ".repeat(indent);
        let i1 = "  ".repeat(indent + 1);
        let i2 = "  ".repeat(indent + 2);
        match self {
            Expr::Lit(lit) => match lit {
                Lit::Int(n) => write!(w, "{n}"),
                Lit::Bool(b) => write!(w, "{b}"),
                Lit::Unit => write!(w, "()"),
                Lit::Str(s) => write!(w, "\"{s}\""),
            },
            Expr::Var(name) => write!(w, "{name}"),
            Expr::Let { name, value, body } => {
                write!(w, "let {name} =\n{i1}")?;
                value.pp_with_indent(w, indent + 1)?;
                write!(w, "\n{i0}in\n{i1}")?;
                body.pp_with_indent(w, indent + 1)
            }
            Expr::Lam { param, body } => {
                write!(w, "λ{param} =>\n{i1}")?;
                body.pp_with_indent(w, indent + 1)
            }
            Expr::App(f, arg) => {
                write!(w, "(")?;
                f.pp_with_indent(w, indent)?;
                write!(w, " ")?;
                arg.pp_with_indent(w, indent)?;
                write!(w, ")")
            }
            Expr::If { cond, then_, else_ } => {
                write!(w, "if ")?;
                cond.pp_with_indent(w, indent)?;
                write!(w, "\n{i0}then ")?;
                then_.pp_with_indent(w, indent)?;
                write!(w, "\n{i0}else ")?;
                else_.pp_with_indent(w, indent)
            }
            Expr::Match { scrutinee, arms } => {
                write!(w, "match ")?;
                scrutinee.pp_with_indent(w, indent)?;
                for arm in arms {
                    write!(w, "\n{i1}| ")?;
                    arm.pat.pretty_print(w)?;
                    write!(w, " =>\n{i2}")?;
                    arm.body.pp_with_indent(w, indent + 2)?;
                }
                Ok(())
            }
            Expr::Con { tag, fields } => {
                write!(w, "{tag}(")?;
                for (i, f) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(w, ", ")?;
                    }
                    f.pp_with_indent(w, indent)?;
                }
                write!(w, ")")
            }
            Expr::Foreign { name, args } => {
                write!(w, "{name}(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(w, ", ")?;
                    }
                    a.pp_with_indent(w, indent)?;
                }
                write!(w, ")")
            }
        }
    }
}

impl Pattern {
    pub fn var(name: &str) -> Self {
        Pattern::Var(name.to_string())
    }
    pub fn con(tag: &str, fields: Vec<Pattern>) -> Self {
        Pattern::Con {
            tag: tag.to_string(),
            fields,
        }
    }

    pub(crate) fn pretty_print(&self, w: &mut dyn std::io::Write) -> std::io::Result<()> {
        match self {
            Pattern::Wildcard => write!(w, "_")?,
            Pattern::Var(name) => write!(w, "{}", name)?,
            Pattern::Lit(lit) => match lit {
                Lit::Int(n) => write!(w, "{}", n)?,
                Lit::Bool(b) => write!(w, "{}", b)?,
                Lit::Unit => write!(w, "()")?,
                Lit::Str(s) => write!(w, "\"{}\"", s)?,
            },
            Pattern::Con { tag, fields } => {
                write!(w, "{}", tag)?;
                if !fields.is_empty() {
                    write!(w, "(")?;
                    for (i, field) in fields.iter().enumerate() {
                        if i > 0 {
                            write!(w, ", ")?;
                        }
                        field.pretty_print(w)?;
                    }
                    write!(w, ")")?;
                }
            }
        }
        Ok(())
    }
}

impl MatchArm {
    pub fn new(pat: Pattern, body: Expr) -> Self {
        MatchArm { pat, body }
    }
}
