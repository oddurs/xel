---
id: 79
uid: 109caa13-add7-492f-bb60-0c5ed7ca409b
title: Python blocks
type: feature
status: backlog
milestone: v0.4
depends_on:
- 53
- 75
- 76
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: l
area: python
---

## Problem

Some transformations are too complex for formulas or SQL.

## Proposal

A Python block takes named tables as frames and returns a frame that renders as a read-only table and can be referenced like a query block. It runs in a separate process per the trust spike and re-runs when its inputs change.

## Acceptance criteria

- [ ] A block that groups and pivots a table updates after an input edit
- [ ] An exception shows the traceback inline and leaves the last good result marked stale
- [ ] A runaway block can be cancelled without restarting the app
