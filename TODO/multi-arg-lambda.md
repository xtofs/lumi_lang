# Multi-argument lambdas

Currently all functions are curried: `λa. λb. body` emits two nested closures and
two lifted C functions. Multi-argument lambdas would let `λ(a, b). body` compile to
a single C function taking two `Value*` parameters, eliminating the intermediate
closure allocation.

## What to change

- `ast.rs` — `Lam` takes `Vec<String>` params instead of a single `param: String`
- `perceus.rs` — `xform` for `Lam` loops over params, each added to `body_owned`;
  captures are free vars of body minus all params
- `codegen.rs` — `lift_lambda` emits `(Value* _env, Value* arg0, Value* arg1, ...)`
  and `apply` needs a multi-arg variant (or a curried adapter is kept for
  higher-order use-sites that pass the function around as a `Value*`)
- `rc_ast.rs` — `RcExpr::Lam` gets `params: Vec<String>`

## Tradeoff

Higher-order use (passing `and` to `map`) still needs a `Value*`-compatible calling
convention, so a single-closure adapter may still be needed when the arity is unknown
at the call site. A simple approach: emit multi-arg only when the lambda is applied
directly (saturated call), keep curried form for partially-applied / passed-as-value
cases.
