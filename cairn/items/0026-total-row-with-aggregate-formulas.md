---
id: 26
uid: 744f640d-561e-42a3-8603-9c216b3dc5b0
title: Total row with aggregate formulas
type: feature
status: done
milestone: v0.1
assignee: Oddur Sigurdsson
depends_on:
- 22
- 25
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
priority: p0
effort: m
area: recalc
---

## Problem

v0.1 has no free cells, so a table needs somewhere to put =SUM([Amount]).

## Proposal

Each table has an optional total row, like Excel's Table Total Row and Airtable's summary bar. Each column's total is a formula over that column; the menu offers Sum, Average, Count, Min, Max.

## Acceptance criteria

- [x] A total updates when any cell in its column changes
- [x] Totals are stored in `xel_columns` and survive save and reopen

## 2026-11-01

Evidence: totals_follow_every_edit_including_formula_columns (cell edits, insert, delete, undo, downstream formula columns) and totals_are_stored_in_xel_columns_and_survive_reopen_and_save (plain SQLite reads total and total_value; reopen and save_copy keep them). Mutations dropping cell-edit or downstream recompute fail the first. Random undo/redo test now sets totals and snapshots total columns. Criterion 1 holds until a column is renamed; 0125 rewrites names.

## Result

Each column may carry a total formula over whole columns (xel_columns.total/total_value/total_kind), set with Workbook::set_total, recomputed by recalc when what it reads changes; Aggregate gives Sum/Average/Count(COUNTA)/Min/Max, and a date column's Min/Max/Average show as dates.
