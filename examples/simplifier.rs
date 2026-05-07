use std::io;

use lumi::ast::Lit;
use lumi::rc::Expr as RcExpr;
use lumi::simplify;

fn print_banner(name: &str) {
    println!(
        "╔═══════════════════════════════════════════════╗\n\
         ║  {name:<45}║\n\
         ╚═══════════════════════════════════════════════╝\n"
    );
}

fn main() {
    // Rule 1: adjacent cancellation
    // dup(f); drop(f); Nil() -> Nil()
    let rule1_before = RcExpr::dup(
        "f",
        RcExpr::drop_(
            "f",
            RcExpr::Con {
                tag: "Nil".into(),
                fields: vec![],
                reuse: None,
            },
        ),
    );
    let rule1_after = simplify::simplify(rule1_before.clone());

    // Rule 2: dead dup
    // dup(f); 42 -> 42
    let rule2_before = RcExpr::dup("f", RcExpr::Lit(Lit::Int(42)));
    let rule2_after = simplify::simplify(rule2_before.clone());

    // Rule 3: commute then cancel
    // dup(f); let x = 42 in drop(f); x -> let x = 42 in x
    let rule3_before = RcExpr::dup(
        "f",
        RcExpr::Let {
            name: "x".into(),
            value: Box::new(RcExpr::Lit(Lit::Int(42))),
            body: Box::new(RcExpr::drop_("f", RcExpr::Var("x".into()))),
        },
    );
    let rule3_after = simplify::simplify(rule3_before.clone());

    // fs::create_dir_all("out").expect("cannot create out/");
    // let path = "out/simplifier_demo.txt";
    // let file = fs::File::create(path).expect("cannot create output file");
    // let mut w = BufWriter::new(file);

    print_banner("simplifier demo");

    let cases: &[(&str, &RcExpr, &RcExpr)] = &[
        ("rule 1 - adjacent cancel", &rule1_before, &rule1_after),
        ("rule 2 - dead dup", &rule2_before, &rule2_after),
        ("rule 3 - commute + cancel", &rule3_before, &rule3_after),
    ];

    for (label, before, after) in cases {
        println!("  {label}");

        print!("    before: ");
        before.print(&mut std::io::stdout()).unwrap();

        print!("    after:  ");
        after.print(&mut std::io::stdout()).unwrap();

        println!()
    }
}
