# Rename `rc_dec`/`rc_inc` to `lumi_drop`/`lumi_dup` in generated C

Currently the codegen emits `rc_dec(x)` and `rc_inc(x)` while the IR nodes are
called `Drop` and `Dup`. The comment bridges the gap (`/* drop x */`) but the
function name leaks the implementation strategy (reference counting) into the
output.

## Proposed change

Add aliases in `lumi_runtime.h`:

```c
#define lumi_drop(v) rc_dec(v)
#define lumi_dup(v)  rc_inc(v)
```

And update `codegen.rs` to emit `lumi_drop` / `lumi_dup` instead of `rc_dec` /
`rc_inc`, dropping the `/* drop x */` comments since the name is now self-documenting.

## Why it matters

- Generated C reads closer to the Perceus paper's notation
- Decouples the output language from the RC implementation: a future arena or GC
  backend only needs to redefine the macros, not touch codegen
- `rc_dec` / `rc_inc` can remain as the internal primitives in the runtime
