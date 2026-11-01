---
id: 110
uid: ac2a9143-e0cc-4525-8e06-7083c397e83b
title: Pivot field wells and layout
type: feature
status: backlog
milestone: v0.3
depends_on:
- 50
- 109
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: l
area: app
---

## Problem

A pivot is defined by dragging fields, not by writing a definition by hand.

## Proposal

Inserting a pivot from a table opens an inspector with the table's fields and four wells (Rows, Columns, Values, Filters). Dragging or clicking fields updates the pivot live. Groups expand and collapse; dates group by year, quarter, and month.

## Acceptance criteria

- [ ] Building a two-level pivot is possible with the keyboard alone
- [ ] Every change to the definition is one undo step
- [ ] Values format with the source column's display format
