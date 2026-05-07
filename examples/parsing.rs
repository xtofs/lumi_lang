use lumi::parser::parse;

// -- let / lambda / application
// let id = \x -> x in id 42
// -- constructors (uppercase = Con, lowercase = Var)
// Cons(1, Cons(2, Nil))
// -- if / match
// if foreign "eq"(n, 0) then 1 else foreign "mul"(n, n)
// match xs {
//   | Nil        => 0
//   | Cons(h, _) => h
// }
// -- foreign (escape hatch)
// foreign "add"(x, y)

fn main() {
    let (eo, _rem) = parse("let id = \\x -> x in id 42");

    let e = eo.unwrap();
    println!("----------------");
    let _ = e.clone().print(&mut std::io::stdout());

    println!("----------------");
    let _ = e.pretty_print(&mut std::io::stdout());
}
