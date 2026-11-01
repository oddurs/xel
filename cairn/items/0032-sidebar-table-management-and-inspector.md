---
id: 32
uid: a4992d57-8bca-41e4-9ba8-13625b1ae944
title: Sidebar, table management, and inspector
type: feature
status: planned
milestone: v0.1
depends_on:
- 14
- 31
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: app
---

## Problem

A workbook holds several tables, and columns have settings.

## Proposal

A NavigationSplitView: the sidebar lists tables (create, rename, delete, reorder), the content shows the selected table, and the inspector edits the selected column's name, type, select options, and display format.

## Acceptance criteria

- [ ] Every table operation is undoable
- [ ] Changing a column's type converts values or reports, before applying, how many cannot be converted
- [ ] Layout follows the macOS 26 design language with system controls only
