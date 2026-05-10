use lumi::{compile_program, parser};
use std::{env, fs, path::Path, process};

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: lumi <file>");
        process::exit(1);
    });

    let src = fs::read_to_string(&path).unwrap_or_else(|e| {
        eprintln!("error reading {path}: {e}");
        process::exit(1);
    });

    let stem = Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("out");

    let (expr, errors) = parser::parse(&src);
    if !errors.is_empty() {
        for e in &errors {
            eprintln!("parse error: {e}");
        }
        process::exit(1);
    }
    let expr = expr.unwrap_or_else(|| {
        eprintln!("error: file produced no expression");
        process::exit(1);
    });

    compile_program("out", stem, &[("main", expr)], "main");
}
