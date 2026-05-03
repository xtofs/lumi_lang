# Primitive integers (replace Peano Nat with machine Int)

Currently `expr_nat(n)` in drivers builds a unary Peano chain:
`Succ(Succ(...Zero()...))` — 16 `alloc_con` calls just to pass the number 16.
`lumi_int(n)` already boxes a machine `int64_t` into a `Value*`; the missing
pieces are on the consumption side.

## What already works
- `lumi_int(n)` → `Value*` with TAG_INT  (runtime)
- `Expr::int(n)` → emits `lumi_int(n)`  (codegen)
- `int_add(a, b)`                        (runtime)

## Missing pieces

### 1. Runtime additions (`lumi_runtime.h`)
```c
static inline Value *int_sub(Value *a, Value *b) {
    int64_t r = *(int64_t *)a->payload - *(int64_t *)b->payload;
    rc_dec(a); rc_dec(b);
    return lumi_int(r);
}
static inline Value *int_eq(Value *a, Value *b) {
    int r = *(int64_t *)a->payload == *(int64_t *)b->payload;
    rc_dec(a); rc_dec(b);
    return lumi_bool(r);
}

/* Immortal singletons — no heap allocation on every recursive call.
 * NOTE: payload[] is a flexible array so the int64_t value cannot be set
 * in a static initialiser; use a startup constructor or lazy init instead. */
static Value *LUMI_INT0;  /* = lumi_global(lumi_int(0)) at startup */
static Value *LUMI_INT1;  /* = lumi_global(lumi_int(1)) at startup */
static inline Value *lumi_int0(void) { return LUMI_INT0; }
static inline Value *lumi_int1(void) { return LUMI_INT1; }
```

### 2. Rewrite `make_tree` using `If` instead of `Match`

```rust
// make_tree n =
//   if int_eq(n, 0) then Zero()
//   else Cons(make_tree(int_sub(n,1)), make_tree(int_sub(n,1)))
let tree_fn = Expr::lam("n",
    Expr::if_(
        Expr::foreign("int_eq", vec![Expr::var("n"), Expr::foreign("lumi_int0", vec![])]),
        Expr::con("Zero", vec![]),
        Expr::con("Cons", vec![
            Expr::app(Expr::var("make_tree"),
                Expr::foreign("int_sub", vec![Expr::var("n"), Expr::foreign("lumi_int1", vec![])])),
            Expr::app(Expr::var("make_tree"),
                Expr::foreign("int_sub", vec![Expr::var("n"), Expr::foreign("lumi_int1", vec![])])),
        ]),
    )
);
```

Perceus detects `n` used twice in the Cons branch (two `int_sub` Foreign calls)
and inserts `dup(n)` automatically — no manual change needed.

### 3. Driver simplification (`main.rs`)
```rust
// Before
Expr::app(Expr::var("make_tree"), expr_nat(16))

// After
Expr::app(Expr::var("make_tree"), Expr::int(16))
```

`expr_nat` can be removed entirely once no samples use Peano naturals as inputs.

## No changes needed
- `perceus.rs` — Foreign Dup logic already handles multi-use correctly
- `codegen.rs` — `If` and `Foreign` already emit correctly
- `simplify.rs` — no new cases
