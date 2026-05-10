use lumi::{compile_program, expr_list, parser, Expr, MatchArm, Pattern};

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

#[test]
fn test_inc_list() {
    let inc_list_fn = Expr::lam(
        "xs",
        Expr::match_(
            Expr::var("xs"),
            vec![
                MatchArm::new(Pattern::con("Nil", vec![]), Expr::con("Nil", vec![])),
                MatchArm::new(
                    Pattern::con("Cons", vec![Pattern::var("h"), Pattern::var("t")]),
                    Expr::con(
                        "Cons",
                        vec![
                            Expr::foreign(
                                "int_add",
                                vec![Expr::var("h"), Expr::foreign("lumi_int1", vec![])],
                            ),
                            Expr::app(Expr::var("inc_list"), Expr::var("t")),
                        ],
                    ),
                ),
            ],
        ),
    );

    let main = Expr::let_(
        "_inp",
        expr_list(vec![Expr::int(0), Expr::int(1), Expr::int(2), Expr::int(3)]),
        Expr::let_(
            "_l",
            Expr::foreign("print", vec![Expr::str_("inc_list [0,1,2,3] = ")]),
            Expr::let_(
                "_out",
                Expr::app(Expr::var("inc_list"), Expr::var("_inp")),
                Expr::let_(
                    "_p",
                    Expr::foreign("println", vec![Expr::var("_out")]),
                    Expr::unit(),
                ),
            ),
        ),
    );

    compile_program(
        "out",
        "inc_list",
        &[("inc_list", inc_list_fn), ("main", main)],
        "main",
    );
}

#[test]
fn test_map() {
    let map_fn = parse_expr(
        r#"\f -> \xs -> match xs {
            | Nil => Nil()
            | Cons(h, t) => Cons(f h, (map f) t)
            }"#,
    );

    let inc_fn = parse_expr(r#"\x -> foreign(int_add; x, foreign(lumi_int1;))"#);

    let n = 10;
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
}

#[test]
fn test_or() {
    let or_fn = Expr::lam(
        "a",
        Expr::lam(
            "b",
            Expr::if_(Expr::var("a"), Expr::var("a"), Expr::var("b")),
        ),
    );

    let cases: &[(&str, bool, bool)] = &[
        ("or True  True  = ", true, true),
        ("or True  False = ", true, false),
        ("or False True  = ", false, true),
        ("or False False = ", false, false),
    ];
    let main = cases
        .iter()
        .rev()
        .enumerate()
        .fold(Expr::unit(), |rest, (i, &(label, a, b))| {
            Expr::let_(
                &format!("_sl{i}"),
                Expr::foreign("print", vec![Expr::str_(label)]),
                Expr::let_(
                    &format!("_r{i}"),
                    Expr::app(Expr::app(Expr::var("or"), Expr::bool_(a)), Expr::bool_(b)),
                    Expr::let_(
                        &format!("_sv{i}"),
                        Expr::foreign("println", vec![Expr::var(&format!("_r{i}"))]),
                        rest,
                    ),
                ),
            )
        });

    compile_program("out", "or", &[("or", or_fn), ("main", main)], "main");
}

#[test]
fn test_strings() {
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
                                Expr::foreign(
                                    "str_concat",
                                    vec![Expr::var("hello"), Expr::var("comma")],
                                ),
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
                                            Expr::foreign(
                                                "print",
                                                vec![Expr::str_("length = ")],
                                            ),
                                            Expr::let_(
                                                "_p3",
                                                Expr::foreign(
                                                    "println",
                                                    vec![Expr::foreign(
                                                        "str_len",
                                                        vec![Expr::var("full")],
                                                    )],
                                                ),
                                                Expr::let_(
                                                    "_p4",
                                                    Expr::foreign(
                                                        "print",
                                                        vec![Expr::str_(
                                                            "equals \"Hello, World!\" = ",
                                                        )],
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
                                                        Expr::foreign(
                                                            "println",
                                                            vec![Expr::unit()],
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
        ),
    );

    compile_program("out", "strings", &[("main", main)], "main");
}

#[test]
fn test_tree() {
    let tree_fn = Expr::lam(
        "n",
        Expr::if_(
            Expr::foreign(
                "int_eq",
                vec![Expr::var("n"), Expr::foreign("lumi_int0", vec![])],
            ),
            Expr::con("Zero", vec![]),
            Expr::con(
                "Cons",
                vec![
                    Expr::app(
                        Expr::var("tree"),
                        Expr::foreign(
                            "int_sub",
                            vec![Expr::var("n"), Expr::foreign("lumi_int1", vec![])],
                        ),
                    ),
                    Expr::app(
                        Expr::var("tree"),
                        Expr::foreign(
                            "int_sub",
                            vec![Expr::var("n"), Expr::foreign("lumi_int1", vec![])],
                        ),
                    ),
                ],
            ),
        ),
    );

    let sum_tree_fn = Expr::lam(
        "t",
        Expr::match_(
            Expr::var("t"),
            vec![
                MatchArm::new(Pattern::con("Zero", vec![]), Expr::int(1)),
                MatchArm::new(
                    Pattern::con("Cons", vec![Pattern::var("l"), Pattern::var("r")]),
                    Expr::foreign(
                        "int_add",
                        vec![
                            Expr::app(Expr::var("sum_tree"), Expr::var("l")),
                            Expr::app(Expr::var("sum_tree"), Expr::var("r")),
                        ],
                    ),
                ),
            ],
        ),
    );

    let n = 4;
    let title = format!("sum_tree(tree({n})) = ");
    let main = Expr::let_(
        "_l",
        Expr::foreign("print", vec![Expr::str_(&title)]),
        Expr::let_(
            "_t",
            Expr::app(Expr::var("tree"), Expr::int(n)),
            Expr::let_(
                "_s",
                Expr::app(Expr::var("sum_tree"), Expr::var("_t")),
                Expr::let_(
                    "_ps",
                    Expr::foreign("println", vec![Expr::var("_s")]),
                    Expr::unit(),
                ),
            ),
        ),
    );

    compile_program(
        "out",
        "tree",
        &[("tree", tree_fn), ("sum_tree", sum_tree_fn), ("main", main)],
        "main",
    );
}
