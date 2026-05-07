# parser: support flattened let-chains (round-trippable)

## Problem
The pretty printers can render nested lets as a flattened chain:

let a = ...
let b = ...
let c = ...
in ...

But the parser currently accepts only explicit nesting:

let a = ... in let b = ... in let c = ... in ...

This means pretty-printed output is not round-trippable through parse -> print -> parse.

## Goal
Extend parser grammar to accept flattened let-chains and desugar them into nested `Expr::Let` nodes.

## Acceptance criteria
- Parser accepts both forms:
  - `let a = e1 in let b = e2 in body`
  - `let a = e1\nlet b = e2\nin body`
- Both forms produce structurally equivalent AST.
- Pretty-printed output can be parsed back into equivalent AST (round-trip).
- Add tests for mixed usage and edge cases (single let, two lets, nested lets inside rhs/body).
