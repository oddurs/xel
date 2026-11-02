---
id: 30
uid: f4279b18-f31c-434f-9601-16742b35edab
title: Swift test target that runs locally and in CI
type: chore
status: done
milestone: v0.1
assignee: Oddur Sigurdsson
depends_on:
- 29
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
priority: p1
effort: s
area: infra
---

## Problem

The app has no tests. XCTest needs full Xcode, which contributors may not have.

## Proposal

Add a test target using Swift Testing, run by `scripts/task test`. If it cannot run with the Command Line Tools alone, `scripts/task test` says so and skips with a message, and CI (which has Xcode) always runs it.

## Acceptance criteria

- [x] `scripts/task test` runs the Swift tests in CI
- [x] Locally without Xcode the skip is explicit, never silent

## 2026-11-01

Done by 0028 and 0031: scripts/task test runs swift test with Swift Testing. CI (Xcode) ran 17 tests on run 41378304611 ('✔ Test run with 17 tests in 1 suite passed'). With the Command Line Tools the script adds the framework search paths and the tests run rather than skip; if Testing.framework is missing, swift test fails with 'no such module Testing', never silently.

## Result

Swift Testing runs in scripts/task test, locally with the Command Line Tools (framework paths added) and in CI with Xcode; nothing is skipped.
