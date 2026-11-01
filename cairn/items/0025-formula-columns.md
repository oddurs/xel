---
id: 25
uid: acc00cd8-687f-4bd6-9316-0a5df8e30377
title: Formula columns
type: feature
status: planned
milestone: v0.1
depends_on:
- 10
- 15
- 21
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: l
area: recalc
---

## Problem

Excel Tables and Airtable both compute a column from a formula. It is the core of v0.1.

## Proposal

A column can be defined by a formula evaluated for every row, stored as decided in the formula-column spike. Editing an input cell or the formula recomputes the affected values.

## Acceptance criteria

- [ ] Changing an input cell updates the formula cell in the same row before the edit returns
- [ ] Changing the formula recomputes the whole column
- [ ] Values are visible to `sqlite3` if the spike chose materialization
