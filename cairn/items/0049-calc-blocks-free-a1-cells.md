---
id: 49
uid: 95166a94-9297-454c-a1e3-c3e87f399973
title: 'Calc blocks: free A1 cells'
type: feature
status: backlog
milestone: v0.2
depends_on:
- 27
- 44
- 119
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: l
area: formula
---

## Problem

Excel users need scratch cells for assumptions and summaries next to their tables.

## Proposal

Sheets hold calc blocks: a free grid of cells stored in `xel_cells`, with A1 references, ranges, `$` anchors, and structured references to tables. Formulas recalc with the rest of the workbook.

## Acceptance criteria

- [ ] `=SUM(Orders[Amount])*B2` works in a calc block and updates when Orders changes
- [ ] Copying a formula with relative references adjusts them like Excel, covered by tests
