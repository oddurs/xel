---
id: 16
uid: 152f8d63-dcdd-42a1-aa69-5df4741a2950
title: Undo and redo for every engine edit
type: feature
status: done
milestone: v0.1
assignee: Oddur Sigurdsson
depends_on:
- 15
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
priority: p0
effort: m
area: format
---

## Problem

A spreadsheet without undo is not usable. Undo must cover structural changes too.

## Proposal

Every engine operation runs in a transaction and records its inverse in an undo stack owned by the open workbook. Undo and redo replay inverses. The stack is not persisted.

## Acceptance criteria

- [x] Cell edits, row inserts and deletes, and column add, rename, and delete each undo and redo exactly, covered by property tests
- [x] An edit that fails part way leaves the file unchanged

## 2026-11-01

Property test: 20 seeds x 60 random edits over every operation (create/rename/delete table, add/rename/delete column, select options, insert/set/delete rows), snapshot after each, then undo all and redo all comparing every snapshot. Mutation check: breaking the SetCells inverse makes it fail at seed 1, state 24.

## Result

crates/engine/src/history.rs: every edit is a Change with an exact inverse, applied by one apply(); Workbook::{undo, redo, can_undo, can_redo}; 1,000-entry limit; deleted tables and columns wait in xel_trash_* tables, dropped when no history entry can need them, on close, and on the next open; table, column, and row ids are AUTOINCREMENT and allocated up front so redo recreates things under the same ids; no-op edits are not recorded.
