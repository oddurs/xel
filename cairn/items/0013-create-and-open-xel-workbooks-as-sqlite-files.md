---
id: 13
uid: d1f73c88-2d02-4ee8-add3-6cb3a11798d1
title: Create and open .xel workbooks as SQLite files
type: feature
status: done
milestone: v0.1
assignee: Oddur Sigurdsson
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
priority: p0
effort: m
area: format
---

## Problem

The engine has no notion of a workbook. Everything else needs a file to read and write.

## Proposal

Add `Workbook::create(path)` and `Workbook::open(path)` using rusqlite with bundled SQLite. A new file gets an `xel_meta` table holding the format version and the engine version that wrote it. Opening checks the version first.

## Acceptance criteria

- [x] Creating a workbook produces a file that `sqlite3` opens and that contains `xel_meta`
- [x] Opening a file with a newer format version returns a typed error naming both versions, covered by a test
- [x] Opening a non-SQLite file returns a typed error, never a panic, covered by a test

## 2026-11-01

Evidence: sqlite3 on a created file lists xel_meta with format_version=1 and created_by='xel 0.1.0'; file(1) reports 'SQLite 3.x database, application id 1480936497' (XEL1). Tests: newer_format_is_refused_with_both_versions, non_sqlite_files_are_not_workbooks (text, empty, 4 KiB of 0xFF), plain_sqlite_databases_are_not_workbooks, missing_files_are_not_created_by_open.

## Result

Workbook::create/open in crates/engine/src/workbook.rs; FORMAT_VERSION=1 in xel_meta; application_id XEL1; typed Error enum in error.rs.
