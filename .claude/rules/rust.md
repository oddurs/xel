---
paths:
  - "crates/**"
  - "Cargo.toml"
---

# Rust

- The workspace forbids `unsafe`. The future FFI crate is the only place allowed to
  relax that, and only in that crate's own `[lints]`.
- `clippy::pedantic` is on and warnings are errors. Fix lints; use a narrow `#[allow]`
  with a comment only when the lint is wrong for that line.
- Public errors are typed enums implementing `std::error::Error`, never `String` and
  never `anyhow` in library crates.
- Never panic on user input: formulas, cell references, and workbook files are
  untrusted. `expect` is only for invariants the code itself guarantees, with a message
  that states the invariant.
- Unit tests live in a `tests` module next to the code. Prefer exhaustive or
  property-style checks for parsers.
- Commit `Cargo.lock`; the checks run with `--locked`.
