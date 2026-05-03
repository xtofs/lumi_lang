mod ast;
mod codegen;
mod liveness;
mod perceus;
mod rc_ast;
mod simplify;

use std::fs;
use std::io::{BufWriter, Write};

use ast::{Expr, MatchArm, Pattern};

// ── Sample ────────────────────────────────────────────────────────────────────

struct Sample {
    /// Output file stem (e.g. "and" → out/and.c, out/and.txt).
    name: &'static str,
    /// All named functions in this sample (ordered).
    functions: Vec<(&'static str, Expr)>,
    /// Name of the function called as `main()` — must be a key in `functions`.
    entry: &'static str,
}

// ── Expr builder helpers ──────────────────────────────────────────────────────

fn expr_nat(n: u32) -> Expr {
    (0..n).fold(Expr::con("Zero", vec![]), |acc, _| {
        Expr::con("Succ", vec![acc])
    })
}
fn expr_list(items: Vec<Expr>) -> Expr {
    items
        .into_iter()
        .rev()
        .fold(Expr::con("Nil", vec![]), |tail, head| {
            Expr::con("Cons", vec![head, tail])
        })
}

// ── Driver AST builders ───────────────────────────────────────────────────────

fn and_main() -> Expr {
    let cases: &[(&str, bool, bool)] = &[
        ("and True  True  = ", true, true),
        ("and True  False = ", true, false),
        ("and False True  = ", false, true),
        ("and False False = ", false, false),
    ];
    cases
        .iter()
        .rev()
        .enumerate()
        .fold(Expr::unit(), |rest, (i, &(label, a, b))| {
            Expr::let_(
                &format!("_sl{i}"),
                Expr::foreign("print", vec![Expr::str_(label)]),
                Expr::let_(
                    &format!("_r{i}"),
                    Expr::app(Expr::app(Expr::var("and"), Expr::bool_(a)), Expr::bool_(b)),
                    Expr::let_(
                        &format!("_sv{i}"),
                        Expr::foreign("print", vec![Expr::var(&format!("_r{i}"))]),
                        Expr::let_(&format!("_sn{i}"), Expr::foreign("print_nl", vec![]), rest),
                    ),
                ),
            )
        })
}

fn inc_list_main() -> Expr {
    Expr::let_(
        "_inp",
        expr_list(vec![expr_nat(0), expr_nat(1), expr_nat(2), expr_nat(3)]),
        Expr::let_(
            "_l",
            Expr::foreign("print", vec![Expr::str_("inc_list [0,1,2,3] = ")]),
            Expr::let_(
                "_out",
                Expr::app(Expr::var("inc_list"), Expr::var("_inp")),
                Expr::let_(
                    "_p",
                    Expr::foreign("print_nat_list", vec![Expr::var("_out")]),
                    Expr::foreign("print_nl", vec![]),
                ),
            ),
        ),
    )
}

fn map_main() -> Expr {
    let succ_fn = Expr::lam("x", Expr::con("Succ", vec![Expr::var("x")]));
    Expr::let_(
        "_succ",
        succ_fn,
        Expr::let_(
            "_inp",
            expr_list(vec![expr_nat(0), expr_nat(1), expr_nat(2)]),
            Expr::let_(
                "_l",
                Expr::foreign("print", vec![Expr::str_("map succ [0,1,2] = ")]),
                Expr::let_(
                    "_ms",
                    Expr::app(Expr::var("map"), Expr::var("_succ")),
                    Expr::let_(
                        "_out",
                        Expr::app(Expr::var("_ms"), Expr::var("_inp")),
                        Expr::let_(
                            "_p",
                            Expr::foreign("print_nat_list", vec![Expr::var("_out")]),
                            Expr::foreign("print_nl", vec![]),
                        ),
                    ),
                ),
            ),
        ),
    )
}

fn tree_main() -> Expr {
    let n = 20;
    let title = &format!("sum_tree(tree({n})) = ");

    Expr::let_(
        "_l",
        Expr::foreign("print", vec![Expr::str_(title)]),
        Expr::let_(
            "_t",
            Expr::app(Expr::var("tree"), Expr::int(n)),
            Expr::let_(
                "_s",
                Expr::app(Expr::var("sum_tree"), Expr::var("_t")),
                Expr::let_(
                    "_ps",
                    Expr::foreign("print", vec![Expr::var("_s")]),
                    Expr::foreign("print_nl", vec![]),
                ),
            ),
        ),
    )
}

fn driver(name: &str) -> Expr {
    match name {
        "and" => and_main(),
        "inc_list" => inc_list_main(),
        "map" => map_main(),
        "tree" => tree_main(),
        _ => panic!("no driver for '{name}'"),
    }
}

fn main() {
    // ── Example 1: map ───────────────────────────────────────────────────────
    //
    //   map f xs =
    //     match xs {
    //       Nil        -> Nil
    //       Cons(h, t) -> Cons(f(h), map(f)(t))
    //     }
    //
    // Perceus: if `xs` is uniquely owned, each Cons cell is reused in-place.

    let map_body = Expr::match_(
        Expr::var("xs"),
        vec![
            MatchArm::new(Pattern::con("Nil", vec![]), Expr::con("Nil", vec![])),
            MatchArm::new(
                Pattern::con("Cons", vec![Pattern::var("h"), Pattern::var("t")]),
                Expr::con(
                    "Cons",
                    vec![
                        Expr::app(Expr::var("f"), Expr::var("h")),
                        Expr::app(Expr::app(Expr::var("map"), Expr::var("f")), Expr::var("t")),
                    ],
                ),
            ),
        ],
    );
    let map_fn = Expr::lam("f", Expr::lam("xs", map_body));

    // ── Example 2: inc_list ──────────────────────────────────────────────────
    //
    //   inc_list xs =
    //     match xs {
    //       Nil        -> Nil
    //       Cons(h, t) -> Cons(Succ(h), inc_list(t))
    //     }

    let inc_list_body = Expr::match_(
        Expr::var("xs"),
        vec![
            MatchArm::new(Pattern::con("Nil", vec![]), Expr::con("Nil", vec![])),
            MatchArm::new(
                Pattern::con("Cons", vec![Pattern::var("h"), Pattern::var("t")]),
                Expr::con(
                    "Cons",
                    vec![
                        Expr::con("Succ", vec![Expr::var("h")]),
                        Expr::app(Expr::var("inc_list"), Expr::var("t")),
                    ],
                ),
            ),
        ],
    );
    let inc_list_fn = Expr::lam("xs", inc_list_body);

    // ── Example 3: and ───────────────────────────────────────────────────────
    //
    //   and a b = if a then b else False
    //
    // Demonstrates Drop in the untaken branch.

    let and_fn = Expr::lam(
        "a",
        Expr::lam(
            "b",
            Expr::if_(Expr::var("a"), Expr::var("b"), Expr::con("False", vec![])),
        ),
    );

    // ── Example 4: tree ─────────────────────────────────────────────────
    //
    //   tree n =
    //     if int_eq(n, 0) then Zero()
    //     else Cons(tree(int_sub(n, 1)), tree(int_sub(n, 1)))
    //
    // Uses boxed machine integers instead of Peano Nat.
    // Perceus inserts Dup(n) in the else branch because n is used twice.

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

    // ── Example 5: sum_tree ──────────────────────────────────────────────────
    //
    //   sum_tree t =
    //     match t {
    //       Zero()     -> 1
    //       Cons(l, r) -> int_add(sum_tree(l), sum_tree(r))
    //     }

    let sum_body = Expr::match_(
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
    );
    let sum_tree_fn = Expr::lam("t", sum_body);

    // ── Assemble samples ──────────────────────────────────────────────────────

    let samples = vec![
        Sample {
            name: "and",
            functions: vec![("and", and_fn), ("main", and_main())],
            entry: "main",
        },
        Sample {
            name: "inc_list",
            functions: vec![("inc_list", inc_list_fn), ("main", inc_list_main())],
            entry: "main",
        },
        Sample {
            name: "map",
            functions: vec![("map", map_fn), ("main", map_main())],
            entry: "main",
        },
        Sample {
            name: "tree",
            functions: vec![
                ("tree", tree_fn),
                ("sum_tree", sum_tree_fn),
                ("main", tree_main()),
            ],
            entry: "main",
        },
    ];

    // ── Run the pipeline ──────────────────────────────────────────────────────

    fs::create_dir_all("out").expect("cannot create out/");
    fs::write("out/lumi_runtime.h", include_str!("lumi_runtime.h"))
        .expect("cannot write lumi_runtime.h");

    for sample in &samples {
        let rc_fns = perceus::compile_fns(&sample.functions);

        // ── .txt debug output ─────────────────────────────────────────────────
        debug_output(sample, &rc_fns);

        // ── emit compilable .c file ────────────────────────────────────────────────
        {
            let program = codegen::emit_c_file(&rc_fns, Some(sample.entry));
            let path = format!("out/{}.c", sample.name);
            fs::write(&path, &program).expect("cannot write .c file");
            println!("wrote {path}");
        }
    }
}

fn debug_output(sample: &Sample, rc_fns: &Vec<(String, rc_ast::RcExpr)>) {
    let path = format!("out/{}.txt", sample.name);
    let file = fs::File::create(&path).expect("cannot create output file");
    let mut w = BufWriter::new(file);
    write_banner(&mut w, sample.name);

    for (name, expr) in &sample.functions {
        writeln!(w, "─── Source AST: {name} ───────────────────────").unwrap();
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
    let c = codegen::emit_body_only(rc_fns);
    writeln!(w, "{c}").unwrap();

    println!("wrote {path}");
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
