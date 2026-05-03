# Lumi Compiler — Architecture

Lumi is a prototype compiler that demonstrates **Perceus reference-counting with allocation reuse**.
The pipeline takes a hand-built expression AST, inserts RC operations, and emits compilable C.

---

## Pipeline

```
Expr (ast.rs)
  └─ perceus::transform   — inserts Dup/Drop/ReuseToken nodes
       └─ simplify::simplify  — algebraic cleanup (fixed-point)
            └─ codegen::emit_c_file  — emits a C translation unit
```

Each stage is a pure function over its AST type; no mutable global state.

---

## AST layers

| Type     | File        | Role                                                             |
|----------|-------------|------------------------------------------------------------------|
| `Expr`   | `ast.rs`    | Source language — lambda calculus + constructors + `Foreign` FFI |
| `RcExpr` | `rc_ast.rs` | RC-annotated — adds `Dup`, `Drop`, `ReuseToken` on constructors  |

`Lit` variants: `Int(i64)`, `Bool`, `Unit`.  
`Expr::Foreign { name, args }` is the escape hatch for C primitives (`print`, `int_add`, …).  
All Foreign arguments are uniformly `Value*` — no raw C types cross the abstraction boundary.

---

## Value representation (`lumi_runtime.h`)

Every heap object is a 16-byte header + flexible payload:

```c
typedef struct Value {
    uint32_t rc;      // reference count  (0xFFFFFFFF = immortal)
    uint32_t tag;     // constructor tag
    uint32_t size;    // payload size in bytes
    uint32_t _pad;
    uint8_t  payload[];
} Value;
```

Tag layout:

| Tag                             | Payload                              |
|---------------------------------|--------------------------------------|
| `TAG_CLOSURE`                   | `LumiFn` pointer + `Value*` captures |
| `TAG_INT`                       | `int64_t`                            |
| `TAG_STR`                       | `const char*`                        |
| ADT constructors                | `Value*` fields                      |
| 0-arity cons (`Zero`, `Nil`, …) | empty (size = 0)                     |

`rc_dec` traverses the payload according to tag so captures and fields are freed correctly.

### Immortality

`rc = 0xFFFFFFFF` marks a value as immortal: `rc_dec` skips it, `rc_inc` overflows harmlessly.  
`lumi_global(v)` sets this flag; `lumi_release_global(v)` resets rc to 1 and calls `rc_dec`.  
This is used for top-level function closures and integer singletons.

> **TODO**: replace the `rc = 0xFFFFFFFF` convention with a high-bit sentinel `RC_IMMORTAL = 0x80000000`
> so the flag is structurally separate from the count. See `TODO/immortality-flag.md`.

### Immortal integer singletons

`LUMI_INT0` / `LUMI_INT1` are immortal `Value*` globals initialised by `lumi_runtime_init()` (called
as the first line of every generated `main`). Recursive functions that decrement by 1 each call reuse
these without a `malloc` per call.

---

## Perceus RC insertion (`perceus.rs`)

The pass computes **ownership sets** and inserts `Dup`/`Drop` at statically correct positions.

Key rules:

- **`Let`**: bound name is owned in the body; `Drop` inserted if use count is 0.
- **`Lam`**: parameter owned inside body; captures are free vars that the enclosing scope owns.
- **`App`**: variables shared between function and argument sub-expressions are `Dup`'d before the call.
- **`If`**: only one branch executes → no Dup for variables live in both branches.
  - Variables live **only** in one branch get a `Drop` in the other.
  - Variables consumed by `cond` **and** needed by a branch get a `Dup` before the whole `If`.
  - `branch_owned` tracks which variables are still alive when control reaches the branches
    (variables consumed solely by `cond` are removed from `branch_owned`).
- **`Match`**: scrutinee is consumed; each arm independently drops owned variables not used in that arm.
  - A `ReuseToken` is created for constructor patterns so the arm body can call `reuse_con` instead of `alloc_con`.
- **`Con` / `Foreign`**: args evaluated left-to-right; owned variables appearing in >1 arg are `Dup`'d once per extra use.

### The `or` example — both Perceus `If` cases in one function

```
or a b = if a then a else b
```

- `a` appears in `cond` **and** `then` → `Dup(a)` before the `If`.
- `b` appears only in `else` → `Drop(b)` in the `then` branch.

This is the canonical demo of the `If` handler.

---

## Simplifier (`simplify.rs`)

Algebraic rewrites applied in a fixed-point loop:

- `Drop(x, body)` where `x ∉ fv(body)` → `body`
- `Dup(x, body)` where `x` used ≤1 time in `body` → `body`
- Propagation into `Let`, `If`, `Match`, `App`, `Con`, `Foreign`

`structurally_equal` drives the fixpoint; it must cover **all** `RcExpr` variants — missing a variant
causes an infinite loop. (Bug: missing `Foreign` case caused non-termination; fixed by adding
the case explicitly.)

---

## Code generator (`codegen.rs`)

**Closure lifting**: every `Lam` is hoisted to a static C function `_fn_N(Value* _env, Value* _arg)`.
The closure body extracts captured variables from `_env` (incrementing their RCs), then decrements `_env`.

**Entry-point layout** (`emit_c_file`):

```c
#include "lumi_runtime.h"

static Value* func_a;   // one immortal global per non-entry function
static Value* func_b;

/* lifted _fn_0, _fn_1, … */
/* Value* lumi_func_a(void) { … } */
/* Value* lumi_func_b(void) { … } */
/* Value* lumi_main(void) { … }   */

int main(void) {
    lumi_runtime_init();
    func_a = lumi_global(lumi_func_a());   // initialise immortal globals
    func_b = lumi_global(lumi_func_b());

    Value* _result = lumi_main();
    rc_dec(_result);

    lumi_release_global(func_a);           // orderly cleanup
    lumi_release_global(func_b);
    return 0;
}
```

All top-level functions become immortal globals — this handles mutual recursion and functions
called more than once without needing a separate `recursive_names` analysis.

---

## Demos

| Demo       | What it shows                                                                        |
|------------|--------------------------------------------------------------------------------------|
| `or`       | Perceus `If`: `Dup` before condition, `Drop` in non-using branch                     |
| `inc_list` | Perceus `Match` + `ReuseToken`: list mapped in-place when uniquely owned             |
| `map`      | Higher-order function with closure capture                                           |
| `tree`     | Boxed machine integers, `int_eq`/`int_sub` primitives, `Dup(n)` before binary branch |

Run with `./demo.sh` (or `./demo.sh <name>` for one demo).

---

## Open TODOs (`TODO/`)

| File                    | Topic                                                                   |
|-------------------------|-------------------------------------------------------------------------|
| `multi-arg-lambda.md`   | Emit single multi-param C function instead of nested curried closures   |
| `seq-expression.md`     | `Expr::seq` builder to flatten deeply nested `let _ = A in B` chains    |
| `rename-drop-dup.md`    | Rename runtime calls to `lumi_drop`/`lumi_dup` for clarity              |
| `immortality-flag.md`   | Use `RC_IMMORTAL = 0x80000000` sentinel instead of threshold `0xFFFF00` |
| `primitive-integers.md` | Generalise machine-int support beyond the `tree` demo                   |
