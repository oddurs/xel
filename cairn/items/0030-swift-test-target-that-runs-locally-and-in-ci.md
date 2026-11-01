---
id: 30
uid: f4279b18-f31c-434f-9601-16742b35edab
title: Swift test target that runs locally and in CI
type: chore
status: planned
milestone: v0.1
depends_on:
- 29
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: s
area: infra
---

## Problem

The app has no tests. XCTest needs full Xcode, which contributors may not have.

## Proposal

Add a test target using Swift Testing, run by `scripts/task test`. If it cannot run with the Command Line Tools alone, `scripts/task test` says so and skips with a message, and CI (which has Xcode) always runs it.

## Acceptance criteria

- [ ] `scripts/task test` runs the Swift tests in CI
- [ ] Locally without Xcode the skip is explicit, never silent
