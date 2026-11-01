---
id: 7
uid: a29424d3-e8a7-4a7c-bb79-8ce40c1c61a8
title: Choose the Rust-to-Swift bridge
type: spike
status: done
milestone: v0.1
assignee: Oddur Sigurdsson
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
priority: p0
effort: s
area: bridge
---

## Question

UniFFI, swift-bridge, or a hand-written C ABI with cbindgen: which one lets the SwiftUI app call the engine with the least ceremony, build with SwiftPM (Command Line Tools locally, Xcode in CI), and link a universal static library into the app?

## Why it matters

Every app feature calls the engine through this boundary. Changing it later means rewriting every call site.

## Timebox

Three days. Deliver a branch where Swift opens a SQLite file through Rust and reads a value back, built by `scripts/task build`, plus a short comparison in the Answer.

## Answer

**UniFFI 0.32.** Working on this branch: Swift creates a workbook through Rust, adds a
table, reopens it, and reads the table name back; engine errors arrive as Swift `throws`.

How it fits together:
- `crates/ffi` (`xel-ffi`) wraps the engine with `#[uniffi::export]` and builds a static
  library. It is the only crate allowed `unsafe` (the generated scaffolding needs it).
- `crates/uniffi-bindgen` runs UniFFI's generator in library mode directly on
  `libxel_ffi.a`. No UDL file and no cdylib are needed.
- `scripts/task build` builds the workspace, generates `app/Generated/` (gitignored), and
  builds the Swift package. SwiftPM consumes the C header and module map as a
  `.systemLibrary` target and the generated Swift as a `XelEngine` target. The static
  library is linked with `-L` (`unsafeFlags`, which is allowed in a root package).
- `.cargo/config.toml` sets `MACOSX_DEPLOYMENT_TARGET=26.0` so bundled C code matches
  the app's minimum macOS.

Panics: UniFFI catches them in Rust. A throwing Swift function receives
`UniffiInternalError.rustPanic`; a non-throwing one is wrapped in `try!` and crashes.
So every exported function returns `Result`, even ones that cannot fail today.

Compared:
| | UniFFI | swift-bridge | hand-written C ABI (cbindgen) |
|---|---|---|---|
| Swift API | Classes, `throws`, enums, records, generated | Similar, less mature | C functions and opaque pointers; Swift wrappers by hand |
| Memory and ownership | Generated (Arc-backed objects) | Generated | Manual on both sides |
| Panics | Caught, thrown in Swift | Not caught by default | Must catch_unwind by hand |
| Build with Command Line Tools only | Yes | Yes | Yes |
| Python bindings later | Same tool, different target | No | No |

The deciding factor beyond ergonomics: UniFFI also generates Python, which the `xel`
package (v0.4) can reuse instead of maintaining PyO3 bindings separately. That
evaluation belongs to the Python spike.

Left for the follow-up items:
- 0029: a universal (arm64 + x86_64) library, and release builds.
- 0028: the full API.
- The generated module names (`xel_ffiFFI`) can be renamed with `uniffi.toml` if wanted.

## Result

UniFFI 0.32 in library mode on the static library: crates/ffi (xel-ffi, Book object, XelError), crates/uniffi-bindgen, scripts/task build generates app/Generated and SwiftPM links via a systemLibrary target + -L; every exported function returns Result so panics become Swift errors.
