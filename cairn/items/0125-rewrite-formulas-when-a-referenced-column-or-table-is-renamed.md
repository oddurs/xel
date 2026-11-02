---
id: 125
uid: 71e1b692-aa68-4f09-8fb9-448f54ac1605
title: Rewrite formulas when a referenced column or table is renamed
type: feature
status: planned
milestone: v0.1
depends_on:
- 25
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: m
area: formula
---

## Problem

Formula columns (0025) refer to columns and tables by name. Renaming a referenced column or table turns every formula that uses it into `#REF!`, where Excel rewrites the formulas to the new name.

## Proposal

Add a formula printer (AST back to Excel text, with minimal parentheses), and on rename rewrite every formula in the workbook that references the old name, as part of the same edit so undo reverts both.

## Acceptance criteria

- [ ] Renaming a column referenced as `[@Qty]`, `[Qty]`, and `Orders[Qty]` rewrites each formula and its values stay the same, covered by a test
- [ ] Renaming a table referenced by name rewrites `Orders[...]` references, covered by a test
- [ ] Printing then parsing any formula in the parser's corpus gives the same AST, covered by a round-trip test
- [ ] Undo of the rename restores the old names in both the column and the formulas

## 2026-11-01

Totals (0026) reference columns by name too: renaming a column must rewrite xel_columns.total formulas as well as formula columns.
