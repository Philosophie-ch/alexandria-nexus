# Coding Standards

This document outlines the coding standards and type safety philosophy for alexandria-nexus. It is the **authoritative source** for type safety rules, linting, error handling, and code style. For layer architecture and dependency rules, [ARCHITECTURE.md](ARCHITECTURE.md) is authoritative.

## Philosophy

Rust's type system exists to prevent bugs at compile time. **Don't fight it. Don't bypass it. Use it.**

Every cast, every `unwrap()`, every `unsafe` block is you telling the compiler "trust me, I know better." You probably don't. The compiler has caught more bugs than you've written.

**Core principles:**

1. **If it compiles, it should work.** Design types so invalid states are unrepresentable.
2. **No silent conversions.** Every type boundary should be explicit and checked.
3. **Let the compiler help you.** When you feel friction, you're probably doing something wrong.

## Theoretical Foundations

These aren't arbitrary opinions. They're established principles from programming language theory:

**Curry-Howard Correspondence** — Types are propositions, programs are proofs. A well-typed program is a proof that it satisfies its type's specification. When we say "if it compiles, it should work," we're approximating this ideal. Languages like Idris and Agda take this literally; Rust and Haskell approximate it.

**Make Invalid States Unrepresentable** — Core type theory concept. If your type admits N possible values but only M < N are valid, you have N - M bugs waiting to happen. Algebraic data types (sum types, product types) let you model exactly the valid state space. Related to refinement types and dependent types in academic research.

**Nominal vs Structural Typing** — Structural typing says "same shape = same type." Nominal typing says "explicitly declared as same = same type." Newtypes (`struct UserId(i64)`) give you nominal typing: `UserId` and `AccountId` have the same runtime representation but are distinct types. The compiler prevents mixing them.

**Parse, Don't Validate** — From Alexis King's influential 2019 essay. Instead of validating data and passing around the raw form, parse it into a type that can only represent valid data. Once you have an `Email`, you know it's valid—the type proves it. Push validation to system boundaries, then work with proof-carrying types internally.

**Ports and Adapters (Hexagonal Architecture)** — Your domain core uses domain-appropriate types. Adapters at the boundaries handle conversion to external representations (JSON, SQL, wire formats). This keeps the core clean and pushes type conversions to the edges where they're explicit and auditable.

**No Implicit Coercions** — ML-family languages (OCaml, Haskell, F#, Rust) require explicit type conversions. C's implicit integer promotions and pointer coercions are widely considered design mistakes—they hide information loss and type confusion. Explicit conversions make the programmer's intent clear and auditable.

## Respect the Type System

### Don't Cast Away Type Information

Casts (`as`, `transmute`, pointer coercion) bypass the type system. They tell the compiler to stop checking. This is almost always wrong.

```rust
// BAD: Bypassing the type system
let x: i32 = big_number as i32;      // Silent truncation
let ptr = thing as *const Foo;        // Losing lifetime info
let bytes = unsafe { transmute(x) };  // Reinterpreting memory

// GOOD: Working WITH the type system
let x: i32 = i32::try_from(big_number)?;  // Compiler-checked conversion
let borrowed: &Foo = &thing;               // Proper borrowing
let bytes = x.to_le_bytes();               // Explicit serialization
```

### Choose Types That Encode Meaning

Don't use primitives when a type can enforce invariants:

```rust
// BAD: Primitive obsession
fn process_user(id: i64, email: String, age: i32) { ... }
// Can accidentally swap id and age - both are just numbers

// GOOD: Types encode meaning
struct UserId(i64);
struct Email(String);  // Validate on construction
struct Age(u8);        // u8 makes invalid ages unrepresentable

fn process_user(id: UserId, email: Email, age: Age) { ... }
// Compiler prevents swapping arguments
```

### Convert at Boundaries, Not Everywhere

Pick the right internal type. Convert only when crossing system boundaries:

```rust
// Internal domain: use natural Rust types
pub struct Pagination {
    pub offset: Option<usize>,  // usize: natural for slicing
    pub limit: usize,
}

// At database boundary: explicit, checked conversion
async fn find_all(&self, pagination: &Pagination) -> Result<Vec<T>, Error> {
    let limit = i64::try_from(pagination.limit)
        .expect("limit exceeds i64::MAX");  // SQL needs i64

    sqlx::query("... LIMIT $1").bind(limit)...
}
```

### Make Invalid States Unrepresentable

Use enums and newtypes to constrain what's possible:

```rust
// BAD: Invalid states are representable
struct Request {
    cursor: Option<String>,
    offset: Option<i64>,
    // What if both are Some? What if neither?
}

// GOOD: The type enforces exactly one pagination mode
enum PaginationMode {
    Cursor(String),
    Offset(usize),
}
```

## Specific Rules

### No `as` for Value Conversions

The `as` keyword silently truncates, wraps, or loses data:

```rust
// BAD: Compiles fine, corrupts data
let big: i64 = 1_000_000_000_000;
let small: i32 = big as i32;  // Wraps to garbage

// GOOD: Compiler-checked
let small: i32 = i32::try_from(big)?;
```

**When `as` is acceptable:** Trait object coercion only (no alternative exists):

```rust
let error: Box<dyn Error> = Box::new(e) as Box<dyn Error>;
```

### No Stringly-Typed Code

Don't use strings when an enum or struct fits:

```rust
// BAD: Stringly typed
fn set_status(status: &str) { ... }
set_status("pending");  // Typo? Who knows.

// GOOD: Type-checked
enum Status { Pending, Active, Completed }
fn set_status(status: Status) { ... }
set_status(Status::Pending);  // Compiler-verified
```

### Handle Errors Properly

`unwrap()` and `expect()` are casts from `Result<T, E>` to `T`. Use them sparingly:

```rust
// BAD: Panic in library code
let value = parse(input).unwrap();

// GOOD: Propagate errors
let value = parse(input)?;

// ACCEPTABLE: At hard boundaries with clear message
let config = load_config().expect("config file must exist");
```

### Design Typed Error Variants

`anyhow` is for applications, not libraries. Library code must use typed error enums so callers can match and handle failure cases programmatically. A stringly-typed or opaque error forces every caller to either ignore it or parse a string — both are wrong.

```rust
// BAD: opaque, unactionable
return Err(anyhow!("not found"));
return Err("conflict on insert".into());

// BAD: stuffing a known failure mode into the catch-all
return Err(HexforgeError::Other("unique violation".to_string()));

// GOOD: typed, matchable, actionable
return Err(DataSourceError::NotFound.into());
return Err(DataSourceError::Conflict.into());
```

When a new operation introduces a new failure mode that callers need to distinguish, add a variant to `HexforgeError` or `DataSourceError` rather than reusing `Other`. `Other` exists for truly unexpected errors from external systems — not as a default for known failure cases.

### No Silently Ignored Results

Discarding a `Result` or `Option` hides failures. Destructuring with `_` in pattern matching is fine — the issue is specifically `let _ = expr` where the expression returns a `Result` or `Option`.

```rust
// BAD: Silently ignoring an error
let _ = might_fail();

// GOOD: Handle or propagate
might_fail()?;

// FINE: Destructuring in pattern matching is idiomatic
match value {
    Ok(x) => use(x),
    Err(_) => handle_error(),  // _ in patterns is fine
}
```

If a `Result` is genuinely safe to discard, add a comment explaining why — a silent `let _ =` gives no such signal to the reader.

### Dependency `unsafe` Hygiene

Memory safety is a first-class goal. Two complementary mechanisms enforce it:

**First-party code:** both crate roots (`src/lib.rs`, `src/main.rs`) carry `#![forbid(unsafe_code)]`. Any `unsafe` block in alexandria-nexus is a compile error — no tooling required.

**Dependencies:** `make check` runs `cargo geiger` across the full dependency tree and hard-fails on any crate that uses `unsafe` and is not listed in `.geiger-allow`. This file records every approved dependency with an explanation of *why* its unsafe is necessary and why it is trusted (OS primitives, SIMD, FFI to C libraries, atomic operations, formal proofs, etc.).

When adding a dependency that introduces new `unsafe` transitives, the build will fail. To resolve it:
1. Read the geiger output to identify the new crate(s)
2. Understand why they use `unsafe` — if it is load-bearing and the crate is well-maintained, add it to `.geiger-allow` with a justification comment
3. Note the approval in the PR description

Avoid adding dependencies whose `unsafe` you cannot explain or justify. When in doubt, look for a `#![forbid(unsafe_code)]` crate that provides the same functionality.

### Clone Discipline

`.clone()` is not free on owned data. Prefer borrows and moves; reach for `.clone()` only when the lifetime genuinely requires it.

```rust
// FINE: Arc<T> clone is a reference-count bump — cheap and intentional
let ds = Arc::clone(&state.data_source);

// SUSPICIOUS: cloning a large owned struct to paper over a borrow error
// usually means the ownership design needs rethinking
let config = self.config.clone();  // does the callee really need ownership?

// GOOD: pass a reference instead
do_something(&self.config);
```

If you find yourself cloning to satisfy the borrow checker, treat it as a signal to reconsider ownership, not a solution.

### `impl Trait` vs `dyn Trait`

Default to `impl Trait`. Reach for `dyn Trait` only when the concrete type is genuinely unknown at compile time.

```rust
// GOOD: function that calls a trait — use impl Trait
// Compiler specialises per concrete type, zero runtime overhead
pub async fn handle_create(ds: &impl DataSource<T>, ...) -> Result<T, HexforgeError>

// GOOD: storing heterogeneous implementors — use dyn Trait
// Multiple different handler types go in the same Vec; impl Trait cannot do this
expand_handlers: Vec<Box<dyn ExpandHandler>>

// GOOD: returning a trait implementor without naming its type — use dyn Trait
fn build_handler() -> Box<dyn ExpandHandler>
```

**Decision rule:**
- `impl Trait` — "I know what type this is at compile time, I just don't want to name it." Use in function parameters and process-layer orchestration.
- `dyn Trait` — "I genuinely don't know, or I'm mixing types." Use in struct fields, heterogeneous collections, and composition wiring where concrete types are determined at runtime.

Unnecessary `dyn Trait` in function signatures adds vtable overhead and prevents inlining. Unnecessary generics in struct fields make the struct harder to store and pass around. Match the tool to the situation.

## Linting

### Permanent Lints (Cargo.toml)

These run on every build and CI:

```toml
[lints.clippy]
all = "warn"
cast_possible_truncation = "warn"
cast_sign_loss = "warn"
cast_possible_wrap = "warn"
cast_lossless = "warn"
redundant_clone = "warn"

[lints.rust]
warnings = "deny"
```

| Lint | What it catches | Why it matters |
|------|----------------|----------------|
| `cast_possible_truncation` | `i64 as i32` where the value might not fit | Silent data corruption — the high bits are discarded without warning |
| `cast_sign_loss` | `i64 as u64` where the value might be negative | Negative values wrap to large positive values |
| `cast_possible_wrap` | `u64 as i64` where the value might exceed `i64::MAX` | Large unsigned values become negative |
| `cast_lossless` | `u8 as u32` where `u32::from(x)` would be clearer | Not a safety issue, but `as` hides the intent — `From` makes lossless conversion explicit |
| `redundant_clone` | `.clone()` on a value that is not used afterward | Wasted allocation — usually means the ownership model needs rethinking |
| `warnings = "deny"` | Promotes all warnings to hard errors | Prevents warning accumulation — a warning today is a bug tomorrow |

### Periodic Audit

Run before releases or after major changes:

```bash
cargo audit
```

## Commands

```bash
cargo lint                          # Standard clippy with strict type lints
cargo audit                         # Strict type safety audit
cargo test                          # Unit tests
cargo test-all                      # Full suite including postgres
cargo check --no-default-features   # Verify feature gate correctness (no axum, no postgres)
```

## External Format Serialization

Serializing to an external format (CSV rows, JSON, SQL array literals like `{v1,v2}`) is a boundary concern — **it belongs in the adapters layer**, not in logic or process.

```rust
// BAD: logic/export.rs producing CSV rows
fn build_author_rows(authors: &[Author]) -> Vec<Vec<String>> { ... }

// GOOD: adapters/csv_rows.rs producing CSV rows
fn build_author_rows(authors: &[Author]) -> Vec<Vec<String>> { ... }

// GOOD: process/export.rs returning domain types
async fn export_authors(...) -> Result<Vec<Author>, ExportError> { ... }
```

**Rules:**
- `src/logic/` — domain types and format-agnostic helpers only. No `Vec<Vec<String>>`. No "CSV", "SQL", "Postgres" in code or comments.
- `src/process/` — returns domain types, never serialized output. No `Vec<Vec<String>>`. No format-specific terminology in code or comments.
- `src/adapters/csv_rows.rs` — all CSV row builders, header constants, `text_array()` serializer.

## Testing Conventions

### Naming

Test functions describe the scenario and expected outcome:

```rust
#[test]
fn build_page_returns_empty_page_when_no_items() { ... }

#[test]
fn check_permission_denies_write_when_user_has_read() { ... }
```

Use `{function_under_test}_{scenario}_{expected_outcome}` or a natural-language equivalent. Avoid generic names like `test_1` or `it_works`.

### Structure

- **Layer 2 (logic):** inline `#[cfg(test)]` module in the same file as the function. Every public function in `logic/` should have at least one test.
- **Layer 3 (process):** integration tests in `tests/integration/` using mock implementations of process-defined traits. Test orchestration flow, not I/O.
- **Layer 4 (adapters):** integration tests with real backends (testcontainers for PostgreSQL).

### Quality Bar

- Test the **behavior**, not the implementation. If a refactor that preserves behavior breaks your test, the test was wrong.
- Cover the **golden path** and at least one **error path** per public function.
- Tests must be deterministic — no time-dependent assertions, no reliance on execution order.
- Prefer real assertions over snapshot tests. If you must snapshot, keep the expected output minimal.

## Summary

1. **Don't bypass the type system.** No `as`, no `transmute`, no pointer tricks.
2. **Use types that encode meaning.** Newtypes, enums, structs over primitives.
3. **Make invalid states unrepresentable.** Design types so wrong code won't compile.
4. **Convert at boundaries.** Internal code uses natural types; convert only at edges.
5. **Let the compiler help.** Friction means you're fighting the type system. Stop and rethink.
