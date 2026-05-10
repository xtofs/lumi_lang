# Plan: Migrate from Untyped Lambda Calculus to System F

## Current State
- **AST**: Simple untyped calculus (`Var`, `Lam`, `App`, `Let`, pattern matching, ADTs)
- **Runtime**: Perceus reference counting with uniform `Value*` representation
- **Codegen**: Closure lifting to C with captured environments
- **No type information** — all polymorphism is untyped

## System F Overview
System F extends untyped lambda calculus with:
- **Explicit type abstraction**: `Λα. e` (introduce type variables)
- **Type application**: `e [T]` (apply types to polymorphic functions)
- **Rank-2 and higher polymorphism** (if desired)
- **Impredicative types** (if desired)

---

## Phase 1: Foundation — Extend AST & Parser

### 1.1: Add Type Syntax to AST

Create a new `src/ast/type_ast.rs` module:
```
Type ::= TVar(α)              -- type variable
       | Int | Bool | Unit    -- base types
       | T → T                -- function type  
       | ∀α. T                -- universal quantifier
       | Con(tag, T₁, ..., Tₙ) -- ADT constructor type
       | Ref(Box<Type>)       -- reference type (for captures)
```

Extend `Expr` in `src/ast/types.rs`:
```rust
pub enum Expr {
    // ... existing variants ...
    
    // NEW: Type abstraction (System F)
    TyLam { ty_var: String, body: Box<Expr> },
    
    // NEW: Type application
    TyApp { func: Box<Expr>, ty_arg: Box<Type> },
}
```

### 1.2: Extend Parser

Modify `src/parser/mod.rs` to handle:
- Type variable syntax: `α`, `β`, `γ` or `/\α` (System F notation)
- Type abstractions: `Λα. e` or `/\α -> e`  
- Type applications: `e [T]` or `e @ T`
- Function types in type signatures: `T → T`
- Forall quantifiers: `∀α. T`

### 1.3: Add Type Annotation Support (Optional but Recommended)

Extend function definitions to include optional type signatures:
```rust
-- Old (untyped)
id = \x -> x

-- New (with optional type annotation)
id: ∀α. α → α = \x -> x
```

---

## Phase 2: Type Checking

### 2.1: Type Inference Engine

Create `src/tc/mod.rs` (type checker):
- **Judgment**: `Γ ⊢ e : T` (under context Γ, expression e has type T)
- Handle type variable contexts `{α, β, ...}`
- Implement **unification** for constraints

Rough structure:
```rust
pub struct TypeCtx {
    vars: HashMap<String, Type>,      // x : T
    ty_vars: HashSet<String>,         // Bound type variables α
}

pub fn infer(ctx: &TypeCtx, expr: &Expr) -> Result<Type, TypeError> {
    // Implement bidirectional type checking
    // Rule for TyLam: introduce fresh type var, check body
    // Rule for TyApp: check arg is type, apply substitution
    // Rule for Lam: new function type
    // Rule for App: unify func type with arg type
}
```

### 2.2: Key Type Rules

- **TyLam**: `Λα. e : ∀α. T` (type variable introduction)
- **TyApp**: `e [T] : T[T/α]` when `e : ∀α. T'` (type substitution)
- **Lam**: `λx. e : T₁ → T₂` when `x:T₁ ⊢ e : T₂`
- **App**: `f x : T₂` when `f : T₁ → T₂` and `x : T₁`
- **Let**: Generalization over free type vars (optional: rank-2 restriction)

### 2.3: Type Simplification

Add a pass to normalize types (e.g., eta-reduce `∀α. C[α]` → `C` if α is free).

---

## Phase 3: Code Generation Challenges

### 3.1: Monomorphization vs. Polymorphic Runtime

**Option A (Simpler): Monomorphization** (standard approach)
- Generate a separate C function for each concrete type instantiation
- At compile time, collect all type applications and generate concrete versions
- No runtime type information needed

**Option B (Complex): Polymorphic Runtime**
- Preserve type information at runtime
- Closures carry explicit type arguments
- Requires runtime dispatch based on types
- May conflict with Perceus simplicity

**Recommendation**: Start with **Option A (monomorphization)**.

### 3.2: Monomorphization Pipeline

1. **Type-check** the program
2. **Collect type uses**: scan the RC AST to find all `TyApp` nodes
3. **Generate instances**: for each `(TyLam α. e) [T]`, create a concrete instance `e[T/α]`
4. **Substitute types** in the monomorphized expression tree
5. **Generate C code** as before (no type-level operations)

Example transformation:
```rust
-- Source
poly = Λα. λx: α. x

main = let f = poly [Int] in f 42

-- After monomorphization
poly_int = λx: Int. x

main = let f = poly_int in f 42

-- After RC insertion and codegen → C
```

---

## Phase 4: Runtime Integration with Perceus

### 4.1: Closure Representation

Currently, closures capture `Value*` uniformly. With System F:
- **If monomorphizing**: closures still capture `Value*` — type info is erased
- **If polymorphic**: closure header may need to record type arguments (complex)

**Recommendation**: Monomorphization erases types, so the Perceus runtime is **unchanged**.

### 4.2: Value struct remains the same

```c
typedef struct Value {
    uint32_t rc;
    uint32_t tag;
    uint32_t size;
    uint32_t _pad;
    uint8_t  payload[];
} Value;
```

Type-level operations disappear at codegen time.

---

## Phase 5: Testing & Validation

### 5.1: Add System F Examples

Create new examples in `examples/`:
- `id.rs` — identity function: `Λα. λx: α. x`
- `const_fn.rs` — constant function: `Λα. Λβ. λx: α. λy: β. x`
- `map_typed.rs` — polymorphic map with explicit types
- `tree_typed.rs` — existing tree example with type annotations

### 5.2: Regression Tests

- Run existing examples to ensure they still work (backwards compatibility)
- Verify C output is correct and compiles
- Check that monomorphization produces expected number of instances

### 5.3: Error Cases

- Test type errors are caught early (before codegen)
- Ensure meaningful error messages for rank-mismatch, unbound type vars, etc.

---

## Phase 6: Optional Enhancements

### 6.1: Full Rank-N Polymorphism (future)
- Allow `∀α. (∀β. T) → U` (higher-rank function parameters)
- Requires more sophisticated type checking (impredicativity)

### 6.2: Type Aliases
```rust
type List α = Nil | Cons α (List α)
```

### 6.3: Implicit Type Arguments
```rust
-- Infer [T] at call sites automatically
f 42  -- infer f [Int] 42
```

### 6.4: Better Error Messages
- Report type mismatches with source locations and context
- Suggest fixes when possible

---

## Implementation Order (Recommended)

| Step | Task                                                    | Dependencies | Est. Complexity |
|------|---------------------------------------------------------|--------------|-----------------|
| 1    | Extend AST with `TyLam`, `TyApp`, `Type` enum           | None         | Low             |
| 2    | Update parser to recognize type syntax                  | Step 1       | Medium          |
| 3    | Build type inference engine                             | Steps 1–2    | High            |
| 4    | Implement monomorphization pass                         | Steps 1–3    | Medium          |
| 5    | Verify codegen output unchanged (existing examples)     | Steps 1–4    | Low             |
| 6    | Add System F examples & tests                           | Step 5       | Low             |
| 7    | Optimize codegen (de-duplicate monomorphized instances) | Step 6       | Medium          |
| 8    | Documentation + TODOs for rank-N                        | Step 7       | Low             |

---

## Key Questions for Design Choices

1. **Rank-2 or Full Rank-N Polymorphism?**
   - Rank-2 is simpler, covers most practical cases
   - Full rank-N requires impredicativity (complex)

2. **Explicit or Inferred Type Arguments?**
   - Explicit (`e [T]`) is simpler, less ambiguous
   - Implicit requires constraint solving

3. **Generalize Let-bindings?**
   - `let f = λx. x in ...` — should `f` be polymorphic?
   - Requires let-polymorphism (common in ML-family)

4. **ADT Type Parameters?**
   - `List α = Nil | Cons α (List α)`
   - Requires higher-kinded types or restricted syntax

5. **Preserve Untyped Examples?**
   - Keep old syntax, auto-infer types, or require migration?

---

## Summary

The transition to System F is **well-scoped** given the current architecture:
- **AST changes**: Minimal (two new `Expr` variants)
- **Parser changes**: Manageable (new token types and grammar rules)
- **Type checking**: The heavy lift (but standard techniques apply)
- **Codegen**: Can remain **nearly unchanged** via monomorphization
- **Runtime**: Perceus logic is **unaffected** (types erase)

The **critical path** is: **AST → Parser → Type Inference → Monomorphization → Testing**.
