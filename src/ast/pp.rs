use super::{Expr, Lit}; // , MatchArm, Pattern};

impl Expr {
    pub fn print(&self, w: &mut dyn std::io::Write) -> std::io::Result<()> {
        self.pp_inner(w, false)?;
        writeln!(w)
    }

    pub fn pretty_print(&self, w: &mut dyn std::io::Write) -> std::io::Result<()> {
        self.pp_with_indent(w, 0)?;
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
                if args.is_empty() {
                    write!(w, "foreign({name})")?;
                } else {
                    write!(w, "foreign({name};")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(w, ",")?;
                        }
                        write!(w, " ")?;
                        arg.pp_inner(w, false)?;
                    }
                    write!(w, ")")?;
                }
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
                write!(w, "let {name} = {i1}")?;
                value.pp_with_indent(w, indent + 1)?;
                write!(w, "\n{i0}")?;

                // Flatten nested lets
                let mut current = body.as_ref();
                loop {
                    match current {
                        Expr::Let {
                            name: n,
                            value: v,
                            body: b,
                        } => {
                            write!(w, "let {n} = {i1}")?;
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
                Ok(())
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
                if args.is_empty() {
                    write!(w, "foreign({name})")
                } else {
                    write!(w, "foreign({name};")?;
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            write!(w, ",")?;
                        }
                        write!(w, " ")?;
                        a.pp_with_indent(w, indent)?;
                    }
                    write!(w, ")")
                }
            }
        }
    }
}
