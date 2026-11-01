---
id: 65
uid: fadcb84b-2707-4c4f-b35b-d4fe6b7d9f0b
title: Export report of what became static
type: feature
status: backlog
milestone: v0.3
depends_on:
- 63
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: s
area: interop
---

## Problem

Silent loss on export destroys trust.

## Proposal

Every export produces a report listing each item that became a value or was dropped, with the reason, shown in the app and optionally saved as a sheet in the export.

## Acceptance criteria

- [ ] Exporting a workbook with a query block lists that block in the report
