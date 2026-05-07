use super::Expr;

use crate::ast::Lit;

impl Expr {
    pub fn print(&self, w: &mut dyn std::io::Write) -> std::io::Result<()> {
        self.pp_inner(w, false)?;
        writeln!(w)
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
            Expr::Dup { var, body } => {
                write!(w, "dup({var}); ")?;
                body.pp_inner(w, false)?;
            }
            Expr::Drop { var, body } => {
                write!(w, "drop({var}); ")?;
                body.pp_inner(w, false)?;
            }
            Expr::Let { name, value, body } => {
                write!(w, "let {} = ", name)?;
                value.pp_inner(w, false)?;
                write!(w, " in ")?;
                body.pp_inner(w, false)?;
            }
            Expr::Lam {
                param,
                captures,
                body,
            } => {
                let caps = if captures.is_empty() {
                    String::new()
                } else {
                    format!("[{}] ", captures.join(", "))
                };
                write!(w, "λ{caps}{param} => ")?;
                body.pp_inner(w, false)?;
            }
            Expr::App(f, arg) => {
                f.pp_inner(w, true)?;
                write!(w, " ")?;
                arg.pp_inner(w, true)?;
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
                write!(w, "match {scrutinee} with ")?;
                for (i, arm) in arms.iter().enumerate() {
                    if i > 0 {
                        write!(w, " | ")?;
                    }
                    let bindings = arm.bindings.join(", ");
                    let reuse = arm
                        .reuse_token
                        .as_deref()
                        .map(|t| format!(" [reuse: {t}]"))
                        .unwrap_or_default();
                    write!(w, "{}({}){} => ", arm.tag, bindings, reuse)?;
                    arm.body.pp_inner(w, false)?;
                }
            }
            Expr::Con { tag, fields, reuse } => {
                let reuse = reuse
                    .as_deref()
                    .map(|t| format!(" [reuse: {t}]"))
                    .unwrap_or_default();
                write!(w, "{tag}(")?;
                for (i, f) in fields.iter().enumerate() {
                    if i > 0 {
                        write!(w, ", ")?;
                    }
                    f.pp_inner(w, false)?;
                }
                write!(w, "){reuse}")?;
            }
            Expr::Foreign { name, args } => {
                write!(w, "{name}(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(w, ", ")?;
                    }
                    a.pp_inner(w, false)?;
                }
                write!(w, ")")?;
            }
        }

        if needs_parens {
            write!(w, ")")?;
        }

        Ok(())
    }

    pub fn pretty_print(&self, w: &mut dyn std::io::Write) -> std::io::Result<()> {
        self.pp_with_indent(w, 0)?;
        writeln!(w)
    }

    fn pp_with_indent(&self, w: &mut dyn std::io::Write, indent: usize) -> std::io::Result<()> {
        let i0 = "  ".repeat(indent);
        let i1 = "  ".repeat(indent + 1);
        let i2 = "  ".repeat(indent + 2);
        match self {
            Expr::Lit(lit) => {
                match lit {
                    Lit::Int(n) => write!(w, "{n}"),
                    Lit::Bool(b) => write!(w, "{b}"),
                    Lit::Unit => write!(w, "()"),
                    Lit::Str(s) => write!(w, "\"{s}\""),
                }?;
            }
            Expr::Var(name) => {
                write!(w, "{name}")?;
            }
            Expr::Dup { var, body } => {
                write!(w, "dup({var}); ")?;
                body.pp_with_indent(w, indent)?;
            }
            Expr::Drop { var, body } => {
                write!(w, "drop({var}); ")?;
                body.pp_with_indent(w, indent)?;
            }
            Expr::Let { name, value, body } => {
                write!(w, "let {name} =\n{i1}")?;
                value.pp_with_indent(w, indent + 1)?;
                write!(w, "\n{i0}")?;

                // Flatten nested lets for a more compact rendering:
                // let a = ...
                // let b = ...
                // in ...
                let mut current = body.as_ref();
                loop {
                    match current {
                        Expr::Let {
                            name: n,
                            value: v,
                            body: b,
                        } => {
                            write!(w, "let {n} =\n{i1}")?;
                            v.pp_with_indent(w, indent + 1)?;
                            write!(w, "\n{i0}")?;
                            current = b.as_ref();
                        }
                        _ => {
                            write!(w, "in\n{i1}")?;
                            current.pp_with_indent(w, indent + 1)?;
                            break;
                        }
                    }
                }
            }
            Expr::Lam {
                param,
                captures,
                body,
            } => {
                let caps = if captures.is_empty() {
                    String::new()
                } else {
                    format!("[{}] ", captures.join(", "))
                };
                write!(w, "λ{caps}{param} =>\n{i1}")?;
                body.pp_with_indent(w, indent + 1)?;
            }
            Expr::App(f, arg) => {
                write!(w, "(")?;
                f.pp_with_indent(w, indent)?;
                write!(w, " ")?;
                arg.pp_with_indent(w, indent)?;
                write!(w, ")")?;
            }
            Expr::If { cond, then_, else_ } => {
                write!(w, "if ")?;
                cond.pp_with_indent(w, indent)?;
                write!(w, "\n{i0}then ")?;
                then_.pp_with_indent(w, indent)?;
                write!(w, "\n{i0}else ")?;
                else_.pp_with_indent(w, indent)?;
            }
            Expr::Match { scrutinee, arms } => {
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
            }
            Expr::Con { tag, fields, reuse } => {
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
                write!(w, "){reuse}")?;
            }
            Expr::Foreign { name, args } => {
                write!(w, "{name}(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(w, ", ")?;
                    }
                    a.pp_with_indent(w, indent)?;
                }
                write!(w, ")")?;
            }
        }
        Ok(())
    }
}
