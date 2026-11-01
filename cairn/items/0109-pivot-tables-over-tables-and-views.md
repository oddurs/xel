---
id: 109
uid: bcf34736-4771-4316-b3a9-49188fc85d1f
title: Pivot tables over tables and views
type: feature
status: backlog
milestone: v0.3
depends_on:
- 52
- 54
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: query
---

## Problem

Excel users summarize data with pivot tables before they reach for formulas.

## Proposal

A pivot block takes a table or view and a definition (row fields, column fields, value fields with an aggregate, and filters) and produces a grouped result by generating SQL. Results recalculate when the source changes and can be referenced like a query block.

## Acceptance criteria

- [ ] A pivot of 1,000,000 rows by two fields computes in under 1 second
- [ ] Sum, count, average, min, max, and distinct count are available, covered by tests
- [ ] Grand totals and subtotals match the same aggregates computed by formulas, covered by tests
