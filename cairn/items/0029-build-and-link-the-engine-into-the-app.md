---
id: 29
uid: fc042b06-4f2c-4b61-9ef7-3c98a6325659
title: Build and link the engine into the app
type: chore
status: doing
milestone: v0.1
assignee: Oddur Sigurdsson
claimed: 2026-11-01
depends_on:
- 7
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: infra
---

## Problem

`scripts/task build` builds the engine and the app separately; the app does not use it.

## Proposal

Build the engine as a universal static library (arm64 and x86_64) and link it into the Swift package, wired into `scripts/task build` so CI and hooks need no change.

## Acceptance criteria

- [ ] `scripts/task check` builds and links the app on a clean clone
- [ ] CI is green on the macOS runner
- [ ] An incremental rebuild after a Swift-only change does not rebuild Rust
