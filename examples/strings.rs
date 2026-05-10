//! Demonstrates string manipulation primitives:
//!
//! - str_concat(a, b)
//! - str_len(s)
//! - str_eq(a, b)

use lumi::{compile_program, Expr};
use std::process::Command;

fn main() {
    let main = Expr::let_(
        "hello",
        Expr::str_("Hello"),
        Expr::let_(
            "comma",
            Expr::str_(", "),
            Expr::let_(
                "world",
                Expr::str_("World"),
                Expr::let_(
                    "bang",
                    Expr::str_("!"),
                    Expr::let_(
                        "hw",
                        Expr::foreign("str_concat", vec![Expr::var("world"), Expr::var("bang")]),
                        Expr::let_(
                            "sentence",
                            Expr::var("hw"),
                            Expr::let_(
                                "prefix",
                                Expr::foreign("str_concat", vec![Expr::var("hello"), Expr::var("comma")]),
                                Expr::let_(
                                    "full",
                                    Expr::foreign(
                                        "str_concat",
                                        vec![Expr::var("prefix"), Expr::var("sentence")],
                                    ),
                                    Expr::let_(
                                        "_p1",
                                        Expr::foreign("println", vec![Expr::var("full")]),
                                        Expr::let_(
                                            "_p2",
                                            Expr::foreign("print", vec![Expr::str_("length = ")]),
                                            Expr::let_(
                                                "_p3",
                                                Expr::foreign(
                                                    "println",
                                                    vec![Expr::foreign("str_len", vec![Expr::var("full")])],
                                                ),
                                                Expr::let_(
                                                    "_p4",
                                                    Expr::foreign(
                                                        "print",
                                                        vec![Expr::str_("equals \"Hello, World!\" = ")],
                                                    ),
                                                    Expr::let_(
                                                        "_p5",
                                                        Expr::foreign(
                                                            "println",
                                                            vec![Expr::foreign(
                                                                "str_eq",
                                                                vec![
                                                                    Expr::var("full"),
                                                                    Expr::str_("Hello, World!"),
                                                                ],
                                                            )],
                                                        ),
                                                        Expr::foreign("println", vec![Expr::unit()]),
                                                    ),
                                                ),
                                            ),
                                        ),
                                    ),
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        ),
    );

    compile_program("out", "strings", &[("main", main)], "main");
    Command::new("./out/strings")
        .status()
        .expect("failed to run strings");
}
