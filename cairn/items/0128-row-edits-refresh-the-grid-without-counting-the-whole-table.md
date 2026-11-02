---
id: 128
uid: ca4a6ef7-2b56-4497-a733-9714d0a538d3
title: Row edits refresh the grid without counting the whole table
type: chore
status: backlog
milestone: v0.1
depends_on:
- 33
created: 2026-11-01
updated: 2026-11-01
priority: p2
effort: s
area: app
---

## Problem

Every row change for the table shown re-reads its TableInfo and runs rowCount, a SELECT count(*) that scans the whole table, on the queue edits and page reads share. On a million rows each single edit pays a full scan before the grid's next page read (found in 0033's review).

## Proposal

Carry the row count change with the change event (the engine knows how many rows an edit added or removed), or have the engine keep the count, so a row edit refreshes the grid with no scan.

## Acceptance criteria

- [ ] Inserting one row into a 1,000,000-row table refreshes the grid without a full-table scan, shown by a test or by timing before and after
- [ ] The grid's row count stays right after inserts, deletes, undo, and redo
