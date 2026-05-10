pub mod ast;
pub mod codegen;
pub mod liveness;
pub mod parser;
pub mod perceus;
pub mod rc;
pub mod simplify;

pub use ast::{Expr, MatchArm, Pattern};

use std::fs;
use std::io::{BufWriter, Write};
use std::process::Command;

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
///   - `<out_dir>/<name>.txt`  — debug dump (source AST, RC AST, generated C)
///   - `<out_dir>/<name>.c`    — compilable C translation unit
///
/// `entry` names the function emitted as `int main(void)`.
pub fn emit_program(out_dir: &str, name: &str, functions: &[(&str, Expr)], entry: &str) {
    fs::create_dir_all(out_dir).expect("cannot create output directory");
    fs::write(
        format!("{out_dir}/lumi_runtime.h"),
        include_str!("lumi_runtime.h"),
    )
    .expect("cannot write lumi_runtime.h");

    let rc_fns = perceus::compile_fns(functions);

    // ── debug .txt ───────────────────────────────────────────────────────────
    {
        let path = format!("{out_dir}/{name}.txt");
        let file = fs::File::create(&path).expect("cannot create output file");
        let mut w = BufWriter::new(file);
        write_banner(&mut w, name);

        for (fname, expr) in functions {
            writeln!(w, "─── Source AST: {fname} ───────────────────────").unwrap();
            expr.pretty_print(&mut w).unwrap();

            let rc_expr = perceus::transform(expr);
            writeln!(w, "\n─── After Perceus RC insertion ───────────────").unwrap();
            rc_expr.pretty_print(&mut w).unwrap();

            let simplified = simplify::simplify(rc_expr.clone());
            if !simplify::structurally_equal_pub(&simplified, &rc_expr) {
                writeln!(w, "\n─── After simplification ─────────────────────").unwrap();
                simplified.pretty_print(&mut w).unwrap();
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
        let path = format!("{out_dir}/{name}.c");
        fs::write(&path, &program).expect("cannot write .c file");
        println!("wrote {path}");
    }
}

/// Compile `functions` through the Perceus pipeline, write the C files to
/// `<out_dir>`, then invoke the C compiler to produce a native binary.
///
/// Equivalent to calling [`emit_program`] followed by:
///   cc -std=c11 -g -fno-omit-frame-pointer -O0 -o <out_dir>/<name> <out_dir>/<name>.c
///
/// The `-g` flag includes DWARF debug symbols for debugger use.
/// `-fno-omit-frame-pointer` preserves frame pointers for better stack traces.
/// `-O0` disables optimizations for easier stepping and more accurate variable inspection.
pub fn compile_program(out_dir: &str, name: &str, functions: &[(&str, Expr)], entry: &str) {
    emit_program(out_dir, name, functions, entry);

    let c_path = format!("{out_dir}/{name}.c");
    let bin_path = format!("{out_dir}/{name}");

    let status = Command::new("cc")
        .args([
            "-std=c11",
            "-g",
            "-fno-omit-frame-pointer",
            "-O0",
            "-o",
            &bin_path,
            &c_path,
        ])
        .status()
        .unwrap_or_else(|e| panic!("failed to launch cc: {e}"));
    assert!(status.success(), "cc exited with {status}");
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
