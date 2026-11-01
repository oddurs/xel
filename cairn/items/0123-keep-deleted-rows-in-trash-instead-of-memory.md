---
id: 123
uid: 0da93703-aa2f-41b5-b384-29e633aebd66
title: Keep deleted rows in trash instead of memory
type: feature
status: backlog
milestone: v0.2
depends_on:
- 16
created: 2026-11-01
updated: 2026-11-01
priority: p2
effort: m
area: format
---

## Problem

Undo (0016) keeps every deleted row's values in memory for up to 1,000 history entries. Selecting all and deleting a 1,000,000-row, 20-column table holds about 20 million values on the heap, while table and column deletes already move their data to trash tables.

## Proposal

Move deleted rows to a per-session trash table (as columns and tables already do) and restore from it on undo; keep small deletes in memory if that is measurably faster.

## Acceptance criteria

- [ ] Deleting and undoing 1,000,000 rows keeps peak memory under 50 MB above baseline, measured
- [ ] The undo property test still passes
