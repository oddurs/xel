---
id: 113
uid: 9a52f2a7-53f9-47b7-8d9a-f0f84617b015
title: Chart definitions in the workbook
type: feature
status: backlog
milestone: v0.4
depends_on:
- 50
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: format
---

## Problem

Charts need to be stored, and must update when their data changes.

## Proposal

A chart block stores its type, its source (a table, view, query, pivot, or range), series, axes, and formatting in `xel_charts`. The engine tells the app when a chart's source changed.

## Acceptance criteria

- [ ] A chart survives save and reopen with identical settings
- [ ] Editing a source cell notifies only the charts that read it, covered by a test
