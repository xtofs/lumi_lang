use super::Expr;

use crate::ast::Lit;

impl Expr {
    pub fn print(&self, w: &mut dyn std::io::Write) -> std::io::Result<()> {
        // Inline the logic of pp_inner for single line output
        let needs_parens = false && matches!(self, Expr::Lam { .. } | Expr::App(_, _));
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
                body.print(w)?;
                ()
            }
            Expr::Drop { var, body } => {
                write!(w, "drop({var}); ")?;
                body.print(w)?;
                ()
            }
            Expr::Let { name, value, body } => {
                write!(w, "let {} = ", name)?;
                value.print(w)?;
                write!(w, " in ")?;
                body.print(w)?;
                ()
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
                body.print(w)?;
                ()
            }
            Expr::App(f, arg) => {
                f.print(w)?;
                write!(w, " ")?;
                arg.print(w)?;
                ()
            }
            Expr::If { cond, then_, else_ } => {
                write!(w, "if ")?;
                cond.print(w)?;
                write!(w, " then ")?;
                then_.print(w)?;
                write!(w, " else ")?;
                else_.print(w)?;
                ()
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
                    arm.body.print(w)?;
                }
                ()
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
                    f.print(w)?;
                }
                write!(w, "){reuse}")?;
                ()
            }
            Expr::Foreign { name, args } => {
                write!(w, "{name}(")?;
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        write!(w, ", ")?;
                    }
                    a.print(w)?;
                }
                write!(w, ")")?;
                ()
            }
        }
        writeln!(w)
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
                write!(w, "\n{i0}in\n{i1}")?;
                body.pp_with_indent(w, indent + 1)?;
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
