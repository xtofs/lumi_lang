/// Pretty printing style for ASTs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrettyPrintStyle {
    Indented,
    SingleLine,
}

pub mod ast;
pub mod codegen;
pub mod liveness;
pub mod perceus;
pub mod rc_ast;
pub mod simplify;

pub use ast::{Expr, MatchArm, Pattern};

use std::fs;
use std::io::{BufWriter, Write};

// ── Shared expression builders ────────────────────────────────────────────────

pub fn expr_list(items: Vec<Expr>) -> Expr {
    items
        .into_iter()
        .rev()
        .fold(Expr::con("Nil", vec![]), |tail, head| {
            Expr::con("Cons", vec![head, tail])
        })
}

// ── Pipeline entry point ──────────────────────────────────────────────────────

/// Compile `functions` through the Perceus pipeline, then write:
///   - `out/<name>.txt`  — debug dump (source AST, RC AST, generated C)
///   - `out/<name>.c`    — compilable C translation unit
///
/// `entry` names the function emitted as `int main(void)`.
pub fn emit_sample(name: &str, functions: &[(&str, Expr)], entry: &str) {
    fs::create_dir_all("out").expect("cannot create out/");
    fs::write("out/lumi_runtime.h", include_str!("lumi_runtime.h"))
        .expect("cannot write lumi_runtime.h");

    let rc_fns = perceus::compile_fns(functions);

    // ── debug .txt ───────────────────────────────────────────────────────────
    {
        let path = format!("out/{name}.txt");
        let file = fs::File::create(&path).expect("cannot create output file");
        let mut w = BufWriter::new(file);
        write_banner(&mut w, name);

        for (fname, expr) in functions {
            writeln!(w, "─── Source AST: {fname} ───────────────────────").unwrap();
            expr.pp(&mut w).unwrap();

            let rc_expr = perceus::transform(expr);
            writeln!(w, "\n─── After Perceus RC insertion ───────────────").unwrap();
            rc_expr.pp(&mut w).unwrap();

            let simplified = simplify::simplify(rc_expr.clone());
            if !simplify::structurally_equal_pub(&simplified, &rc_expr) {
                writeln!(w, "\n─── After simplification ─────────────────────").unwrap();
                simplified.pp(&mut w).unwrap();
            }
            writeln!(w).unwrap();
        }

        writeln!(w, "─── Generated C ──────────────────────────────").unwrap();
        let c = codegen::emit_body_only(&rc_fns);
        writeln!(w, "{c}").unwrap();
        println!("wrote {path}");
    }

    // ── .c file ──────────────────────────────────────────────────────────────
    {
        let program = codegen::emit_c_file(&rc_fns, Some(entry));
        let path = format!("out/{name}.c");
        fs::write(&path, &program).expect("cannot write .c file");
        println!("wrote {path}");
    }
}

fn write_banner(w: &mut dyn Write, name: &str) {
    writeln!(
        w,
        "╔═══════════════════════════════════════════════╗\n\
         ║  {name:<45}║\n\
         ╚═══════════════════════════════════════════════╝\n"
    )
    .unwrap();
}
