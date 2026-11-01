---
id: 111
uid: 833a697e-0cf3-480c-8172-a0527a56d79b
title: Pivot tables in xlsx import and export
type: feature
status: backlog
milestone: v0.3
depends_on:
- 61
- 63
- 109
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: m
area: interop
---

## Problem

Imported Excel workbooks contain PivotTables, and exported pivots must stay useful in Excel.

## Proposal

Import Excel PivotTable definitions as xel pivots where the source is a table or range. Export pivots as native PivotTables if the xlsx spike found a library that writes them, otherwise as values with the definition in the export report.

## Acceptance criteria

- [ ] An imported PivotTable shows the same totals as Excel's cached values
- [ ] An exported pivot is listed in the export report when it becomes values
