---
id: 117
uid: 680002d5-982f-4252-a198-2a1fe5aca1d9
title: Charts in xlsx import and export
type: feature
status: backlog
milestone: v0.4
depends_on:
- 61
- 63
- 113
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: m
area: interop
---

## Problem

Excel workbooks contain charts, and exported workbooks should keep them.

## Proposal

Import Excel charts of the supported types as xel charts and export xel charts as native Excel charts, using the libraries chosen in the xlsx spike. Unsupported chart types are listed in the import or export report.

## Acceptance criteria

- [ ] A column chart round-trips through Excel with the same series and type
- [ ] An unsupported chart type is reported, never silently dropped
