---
id: 25
uid: acc00cd8-687f-4bd6-9316-0a5df8e30377
title: Formula columns
type: feature
status: done
milestone: v0.1
assignee: Oddur Sigurdsson
depends_on:
- 10
- 15
- 21
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
priority: p0
effort: l
area: recalc
---

## Problem

Excel Tables and Airtable both compute a column from a formula. It is the core of v0.1.

## Proposal

A column can be defined by a formula evaluated for every row, stored as decided in the formula-column spike. Editing an input cell or the formula recomputes the affected values.

## Acceptance criteria

- [x] Changing an input cell updates the formula cell in the same row before the edit returns
- [x] Changing the formula recomputes the whole column
- [x] Values are visible to `sqlite3` if the spike chose materialization

## Result

crates/engine/src/recalc.rs: ColumnType::Formula (STRICT ANY column, formula text in xel_columns.formula), Workbook::{add_formula_column, set_formula}; results stored as REAL/TEXT/INTEGER/ISO TEXT/BLOB-for-errors; recalc::after recomputes after every edit, undo, and redo: touched rows for row-local formulas, every row for formulas over whole columns and their dependents; formulas ordered by dependency, cycles shown as #REF!; other-table refs are #REF! until v0.2.
