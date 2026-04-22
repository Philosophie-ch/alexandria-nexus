# Coding Standards

This document outlines the coding standards and type safety philosophy for hexforge.

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

### No Lazy `_` Ignoring

If you're ignoring part of a type, you're probably losing information:

```rust
// BAD: Ignoring the error
let _ = might_fail();

// GOOD: Explicit intent if truly fire-and-forget
drop(might_fail());  // Still bad, but at least obvious

// BEST: Handle or propagate
might_fail()?;
```

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

## Summary

1. **Don't bypass the type system.** No `as`, no `transmute`, no pointer tricks.
2. **Use types that encode meaning.** Newtypes, enums, structs over primitives.
3. **Make invalid states unrepresentable.** Design types so wrong code won't compile.
4. **Convert at boundaries.** Internal code uses natural types; convert only at edges.
5. **Let the compiler help.** Friction means you're fighting the type system. Stop and rethink.
