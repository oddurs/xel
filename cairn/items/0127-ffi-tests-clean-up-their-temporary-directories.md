---
id: 127
uid: 839fa426-fa57-461d-9cf7-cf839e939049
title: FFI tests clean up their temporary directories
type: chore
status: done
milestone: v0.1
assignee: Oddur Sigurdsson
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
priority: p3
effort: s
area: bridge
---

## Problem

crates/ffi's tests create `xel-ffi-test-<pid>` directories under the temp directory and never remove them, so every test run leaves one behind (dozens were found while checking 0031).

## Acceptance criteria

- [x] Running `cargo test -p xel-ffi` leaves no new directory in $TMPDIR, checked by listing before and after

## 2026-11-01

Before: 75 xel-ffi-test-* directories had piled up in TMPDIR, one per run. After: a full scripts/task check leaves 0 (listed before and after). Each test now owns a Scratch removed on drop; scratch_directories_are_removed checks the removal.

## Result

FFI tests use a per-test Scratch directory removed on drop; a run leaves nothing in TMPDIR.
