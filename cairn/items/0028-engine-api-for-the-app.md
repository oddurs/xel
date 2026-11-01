---
id: 28
uid: 35f0a0a8-03d0-4fa6-922a-53e6efd80885
title: Engine API for the app
type: feature
status: planned
milestone: v0.1
depends_on:
- 7
- 16
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: l
area: bridge
---

## Problem

The Swift app cannot do anything until it can call the engine.

## Proposal

Expose through the chosen bridge: open, create, save, close; list tables and columns; read a window of rows; set a cell; add and remove rows and columns; undo and redo; and a change notification the grid can observe. Engine errors become Swift errors with messages.

## Acceptance criteria

- [ ] Every function has a Swift test, or an engine test if Swift tests cannot run yet
- [ ] No Rust panic can cross the boundary; a forced panic becomes a Swift error in a test
- [ ] The engine is called off the main thread; the API is `Sendable`-correct under Swift 6
