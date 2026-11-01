---
id: 14
uid: 2bd000d2-7261-4ade-842c-2d0a5556a436
title: Typed tables stored as real SQL tables
type: feature
status: done
milestone: v0.1
assignee: Oddur Sigurdsson
depends_on:
- 9
- 13
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
priority: p0
effort: l
area: format
---

## Problem

Tables need typed columns that external SQLite tools can read with their real names.

## Proposal

Create, rename, and delete tables and columns. Column types for v0.1: text, number, checkbox, date, single select. Tables are SQL tables with the user's names (quoted); xel-specific metadata (type, select options, display format, order) lives in `xel_columns` and `xel_tables`.

## Acceptance criteria

- [x] `sqlite3` shows each user table under its own name with one column per field
- [x] Renaming a table or column renames the SQL object and keeps the data, covered by a test
- [x] Names with spaces, quotes, and Unicode round-trip, covered by a test
- [x] A date column stores ISO 8601 text, a checkbox stores 0/1, documented in a test

## Result

crates/engine/src/schema.rs: Workbook::{tables, table, create_table, rename_table, delete_table, add_column, rename_column, delete_column, set_select_options}; ColumnType {Text, Number, Checkbox, Date, Select}; user tables are STRICT SQL tables named as the user named them; metadata in xel_tables, xel_columns, xel_select_options; numbers REAL, checkbox INTEGER 0/1, date TEXT YYYY-MM-DD, enforced by CHECK.
