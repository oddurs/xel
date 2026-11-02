---
id: 28
uid: 35f0a0a8-03d0-4fa6-922a-53e6efd80885
title: Engine API for the app
type: feature
status: done
milestone: v0.1
depends_on:
- 7
- 16
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
priority: p0
effort: l
area: bridge
---

## Problem

The Swift app cannot do anything until it can call the engine.

## Proposal

Expose through the chosen bridge: open, create, save, close; list tables and columns; read a window of rows; set a cell; add and remove rows and columns; undo and redo; and a change notification the grid can observe. Engine errors become Swift errors with messages.

## Acceptance criteria

- [x] Every function has a Swift test, or an engine test if Swift tests cannot run yet
- [x] No Rust panic can cross the boundary; a forced panic becomes a Swift error in a test
- [x] The engine is called off the main thread; the API is `Sendable`-correct under Swift 6

## 2026-11-01

From 0011: VACUUM INTO (save_copy) fails while a statement is mid-read on the same connection. Do not hold a grid cursor stepping across a save; read windows fully before returning.

## 2026-11-01

Evidence: 11 Swift Testing tests cover every Workbook function (scripts/task test). Panic: check_panic_boundary throws a non-XelError carrying the panic message in Swift, and the book then reports Poisoned. Off main: theEngineRunsOffTheMainThread runs on the main actor and finds calls on the workbook's own queue; removing the custom executor fails it. Generated types are Sendable; Book is @unchecked Sendable over its Mutex; the package builds in Swift 6 language mode with warnings as errors.

## Result

crates/ffi exposes the whole engine (files, tables, columns, rows as {table,row} RowRefs, undo); XelKit's Workbook actor runs every call on its own DispatchSerialQueue and publishes WorkbookChange on AsyncStreams; Swift Testing runs in scripts/task test with CLT or Xcode.
