---
id: 127
uid: 839fa426-fa57-461d-9cf7-cf839e939049
title: FFI tests clean up their temporary directories
type: chore
status: backlog
milestone: v0.1
created: 2026-11-01
updated: 2026-11-01
priority: p3
effort: s
area: bridge
---

## Problem

crates/ffi's tests create `xel-ffi-test-<pid>` directories under the temp directory and never remove them, so every test run leaves one behind (dozens were found while checking 0031).

## Acceptance criteria

- [ ] Running `cargo test -p xel-ffi` leaves no new directory in $TMPDIR, checked by listing before and after
