---
id: 13
uid: d1f73c88-2d02-4ee8-add3-6cb3a11798d1
title: Create and open .xel workbooks as SQLite files
type: feature
status: planned
milestone: v0.1
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: format
---

## Problem

The engine has no notion of a workbook. Everything else needs a file to read and write.

## Proposal

Add `Workbook::create(path)` and `Workbook::open(path)` using rusqlite with bundled SQLite. A new file gets an `xel_meta` table holding the format version and the engine version that wrote it. Opening checks the version first.

## Acceptance criteria

- [ ] Creating a workbook produces a file that `sqlite3` opens and that contains `xel_meta`
- [ ] Opening a file with a newer format version returns a typed error naming both versions, covered by a test
- [ ] Opening a non-SQLite file returns a typed error, never a panic, covered by a test
