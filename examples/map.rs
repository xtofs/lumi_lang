//! Demonstrates higher-order functions and closure capture with `map`:
//!
//!   map f xs =
//!     match xs {
//!       Nil        -> Nil
//!       Cons(h, t) -> Cons(f(h), map(f)(t))
//!     }
//!
//! When `xs` is uniquely owned, Perceus reuses each Cons allocation.
//! The lambda `λx. int_add(x, 1)` passed as `f` exercises closure capture:
//! `lumi_int1` (an immortal singleton) is captured and referenced on every call.

use lumi::{compile_program, parser, Expr};
use std::process::Command;

fn main() {
    let n = 100;
    let map_fn = parse_expr(
        r#"\f -> \xs -> match xs {
            | Nil => Nil()
            | Cons(h, t) => Cons(f h, (map f) t)
            }"#,
    );

    // f = λx. int_add(x, 1)  — captures lumi_int1 (immortal singleton)
    let inc_fn = parse_expr(r#"\x -> foreign(int_add; x, foreign(lumi_int1;))"#);

    let input_src = make_list(n);
    let main = parse_expr(&format!(
        r#"let inp = {input_src} 
        in let l = foreign(println; inp) 
        in let mapinc = map inc 
        in let out = mapinc inp 
        in let p = foreign(println; out) 
        in foreign(println; ())"#
    ));

    compile_program(
        "out",
        "map",
        &[("map", map_fn), ("inc", inc_fn), ("main", main)],
        "main",
    );
    Command::new("./out/map")
        .status()
        .expect("failed to run map");
}

fn parse_expr(src: &str) -> Expr {
    let (expr, errors) = parser::parse(src);
    if !errors.is_empty() {
        panic!("parse errors in `{src}`: {}", errors.join(" | "));
    }
    expr.expect("parser returned no expression")
}

fn make_list(len: i64) -> String {
    (0..len).rev().fold(String::from("Nil()"), |tail, head| {
        format!("Cons({head}, {tail})")
    })
}
