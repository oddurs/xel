---
id: 16
uid: 152f8d63-dcdd-42a1-aa69-5df4741a2950
title: Undo and redo for every engine edit
type: feature
status: planned
milestone: v0.1
depends_on:
- 15
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: format
---

## Problem

A spreadsheet without undo is not usable. Undo must cover structural changes too.

## Proposal

Every engine operation runs in a transaction and records its inverse in an undo stack owned by the open workbook. Undo and redo replay inverses. The stack is not persisted.

## Acceptance criteria

- [ ] Cell edits, row inserts and deletes, and column add, rename, and delete each undo and redo exactly, covered by property tests
- [ ] An edit that fails part way leaves the file unchanged
