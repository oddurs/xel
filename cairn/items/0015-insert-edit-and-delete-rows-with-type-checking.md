---
id: 15
uid: c4a8f96f-51c6-44d1-9824-4ee63f7275b5
title: Insert, edit, and delete rows with type checking
type: feature
status: done
milestone: v0.1
assignee: Oddur Sigurdsson
depends_on:
- 14
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
priority: p0
effort: m
area: format
---

## Problem

Tables are useless until data can go in and come out.

## Proposal

Engine operations to insert rows, set a cell, delete rows, and read a window of rows (offset and limit) in display order. Setting a value of the wrong type returns a typed error that says which column expected what.

## Acceptance criteria

- [x] Writing "abc" to a number column returns an error naming the column and type
- [x] Reading rows 500,000–500,100 of a 1,000,000-row table takes under 10 ms in a benchmark
- [x] Deleting rows keeps the remaining row order stable, covered by a test

## 2026-11-01

Benchmark (release, M4 Pro): rows 500,000..500,100 of a 1,000,000-row table, median 3.89 ms, max 4.18 ms over 21 runs (cargo test --release -p xel-engine window_read_benchmark -- --ignored --nocapture). Every table now starts with "_id" INTEGER PRIMARY KEY so VACUUM cannot renumber rows; display order is _id order until views add sorting.

## Result

crates/engine/src/rows.rs: RowId, Row; Workbook::{row_count, rows(table, offset, limit), insert_row, set_cell, delete_rows}; values are crate::cell::CellValue {Empty, Text, Number, Checkbox, Date}; type mismatches return Error::TypeMismatch naming column and type; select values must be an option (stored in the option's spelling). Tables have an explicit _id INTEGER PRIMARY KEY.
