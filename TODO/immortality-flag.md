# Immortality flag: use high-bit sentinel instead of threshold

Current check in `rc_dec`:
```c
if (!v || v->rc > 0xFFFF00) return; /* immortal */
```

The threshold `0xFFFF00` is arbitrary — it provides ~255 increments of headroom
before a bug (rc_inc on an immortal) would decrement rc to zero and cause a
spurious free. It works but is fragile and the magic number has no documented
justification.

## Better approach: reserve the high bit as an immortality flag

```c
#define RC_IMMORTAL 0x80000000u

static inline void rc_inc(Value *v) {
    if (v && !(v->rc & RC_IMMORTAL)) v->rc++;
}
static inline void rc_dec(Value *v) {
    if (!v || (v->rc & RC_IMMORTAL)) return;
    ...
}
static inline Value *lumi_global(Value *v) {
    v->rc = RC_IMMORTAL;
    return v;
}
```

## Why it's better
- Immortality is a structural property (a bit flag), not a threshold guess
- rc_inc on an immortal is a no-op — overflow is impossible by construction
- The remaining 31 bits still allow ~2 billion live references, more than enough
- Matches the approach used in some GC literature (e.g. tagged RC words)

## Caveat
Real reference counts are capped at 30 bits (~1 billion). If a value somehow
reaches 2^30 live references the count would set the immortality bit by overflow.
A saturating increment (`if rc < RC_IMMORTAL-1: rc++`) avoids this at negligible cost.
