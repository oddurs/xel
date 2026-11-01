---
id: 53
uid: a4b43e21-9de3-4dc3-ab73-2dcda6e85993
title: Reference query results from formulas
type: feature
status: backlog
milestone: v0.2
depends_on:
- 52
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: m
area: recalc
---

## Problem

Query results are only useful if the rest of the workbook can use them.

## Proposal

A named query block can be referenced like a table (`TopCustomers[Revenue]`) from formulas and from other queries.

## Acceptance criteria

- [ ] Changing an input table updates a calc cell that sums a query result
- [ ] Cycles through queries are detected and reported
