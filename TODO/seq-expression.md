# Sequencing expression (`seq` / `do`-notation)

Driver functions like `and_driver` chain many effectful steps using nested
`let _ = A in B`, which produces deep right-leaning trees that are hard to read:

```rust
Expr::let_("_sl0", print(...),
  Expr::let_("_r0", app(...),
    Expr::let_("_sv0", print(...),
      Expr::let_("_sn0", print_nl(), ...))))
```

## Proposed addition

A `seq` builder that takes a `Vec<Expr>` and desugars to nested lets with
generated throwaway names:

```rust
pub fn seq(steps: Vec<Expr>) -> Self {
    // last element is the result; earlier ones are bound to fresh `_seq{i}` names
}
```

Usage:
```rust
Expr::seq(vec![
    Expr::foreign("print", vec![Expr::str_("and True True = ")]),
    Expr::foreign("print", vec![Expr::app(...)]),
    Expr::foreign("print_nl", vec![]),
])
```

## What to change

- `ast.rs` — add `Expr::seq(steps: Vec<Expr>) -> Expr` as a pure builder that
  desugars into nested `Expr::let_("_seq{i}", step, ...)` at construction time;
  no new AST node needed, Perceus and codegen see ordinary `Let` chains.
- `main.rs` — rewrite `and_driver` (and other drivers) using `Expr::seq`.
