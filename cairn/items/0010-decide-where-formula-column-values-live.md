---
id: 10
uid: 15cdfacc-bfb9-4d3a-af7f-b43b3dcc7613
title: Decide where formula column values live
type: spike
status: done
milestone: v0.1
assignee: Oddur Sigurdsson
depends_on:
- 9
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
priority: p0
effort: s
area: recalc
---

## Question

Are formula column results written into the SQLite table as ordinary columns (materialized), computed on read, or stored as SQLite generated columns?

## Why it matters

The promise is that `sqlite3` sees what the app shows. Materializing keeps that promise but makes every edit a write fan-out; computing on read breaks it.

## Timebox

Two days. Measure recalc and file size for 100,000 rows with 5 formula columns under each approach; record numbers in the Answer.

## Answer

**Materialize.** Formula column results are written into the user table as ordinary
SQL values, so `sqlite3` sees what the app shows.

- **Storage.** A formula column is declared `ANY` in the STRICT table. Results are stored
  as: number → `REAL`, text → `TEXT`, boolean → `INTEGER` 0/1, date → `TEXT`
  `YYYY-MM-DD`, error → `TEXT` such as `#DIV/0!`, empty → `NULL`. The formula text lives in
  `xel_columns`.
- **When it recalculates.** Changing an input recomputes that row's dependent formula
  cells in the same transaction as the edit. Changing a formula recomputes its column.
  Opening a workbook trusts stored results, except volatile formulas (`TODAY`), and
  except when the engine version that wrote them differs, which forces a full recompute.
- **Rejected: SQLite generated columns.** They can only run SQLite expressions, not
  Excel formulas.
- **Rejected: computing on read.** It breaks the promise that `sqlite3` reads what xel
  shows.

Measured with the real parser and evaluator, 100,000 rows, two inputs and five formula
columns (`=[@Qty]*[@Price]` and four similar), release build on an M4 Pro:

| | Result |
|---|---|
| Full recompute (read inputs, evaluate 5 formulas per row, write results) | 86 ms (5 ms read, 28 ms evaluate, 53 ms write) |
| One-cell edit: write input, recompute and write that row's 5 results, commit | 0.39 ms |
| File size, inputs only | 1.6 MiB |
| File size with 5 materialized formula columns | 6.3 MiB |

Consequences:
- 0025 adds the `ANY` column type and the `formula` field in `xel_columns`, and stores
  results this way.
- 0027 makes recalculation incremental.
- A tool reading the file cannot tell an error value from text that happens to read
  `#DIV/0!`. xel can, because it knows which columns are formulas.

## Result

Materialize formula results into the user table in a STRICT ANY column (number REAL, text TEXT, bool INTEGER, date ISO TEXT, error TEXT, empty NULL); formula text in xel_columns; recalc on edit in the same transaction; 100k rows x 5 formulas: full recompute 86 ms, one-row edit 0.39 ms, +4.7 MiB.
