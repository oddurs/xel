---
id: 26
uid: 744f640d-561e-42a3-8603-9c216b3dc5b0
title: Total row with aggregate formulas
type: feature
status: planned
milestone: v0.1
depends_on:
- 22
- 25
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: recalc
---

## Problem

v0.1 has no free cells, so a table needs somewhere to put =SUM([Amount]).

## Proposal

Each table has an optional total row, like Excel's Table Total Row and Airtable's summary bar. Each column's total is a formula over that column; the menu offers Sum, Average, Count, Min, Max.

## Acceptance criteria

- [ ] A total updates when any cell in its column changes
- [ ] Totals are stored in `xel_columns` and survive save and reopen
