---
id: 81
uid: 0e1c92de-1466-4b49-9e7e-0fae22d3c3e8
title: 'Lineage: trace a value to its sources'
type: feature
status: backlog
milestone: v0.4
depends_on:
- 48
- 53
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: m
area: recalc
---

## Problem

"Where did this number come from?" is unanswerable in Excel.

## Proposal

The engine returns the dependency tree for any cell: formulas, columns, queries, blocks, and source tables, up and down.

## Acceptance criteria

- [ ] Tracing a total through a lookup and a query lists every input table
