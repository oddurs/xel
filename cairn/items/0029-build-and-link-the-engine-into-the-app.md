---
id: 29
uid: fc042b06-4f2c-4b61-9ef7-3c98a6325659
title: Build and link the engine into the app
type: chore
status: done
milestone: v0.1
assignee: Oddur Sigurdsson
depends_on:
- 7
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
priority: p0
effort: m
area: infra
---

## Problem

`scripts/task build` builds the engine and the app separately; the app does not use it.

## Proposal

Build the engine as a universal static library (arm64 and x86_64) and link it into the Swift package, wired into `scripts/task build` so CI and hooks need no change.

## Acceptance criteria

- [x] `scripts/task check` builds and links the app on a clean clone
- [x] CI is green on the macOS runner
- [x] An incremental rebuild after a Swift-only change does not rebuild Rust

## 2026-11-01

Universal app verified by the CI universal job (lipo -verify_arch arm64 x86_64 on the built binary); host builds stay fast; a Rust-only change relinks the app (Linking Xel after an engine edit, nothing after no change).

## Result

XEL_UNIVERSAL=1 scripts/task build: per-triple xel-ffi builds joined with lipo, Swift built --arch arm64 --arch x86_64, verified with lipo -verify_arch; host builds unchanged; relink forced when the engine library is newer than the app; XEL_PROFILE debug|release.
