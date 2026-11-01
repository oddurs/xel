---
id: 41
uid: 0b8d6777-7040-4176-a95e-a637f5970537
title: Import CSV with type inference
type: feature
status: planned
milestone: v0.1
depends_on:
- 15
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: m
area: interop
---

## Problem

Most people's data already exists somewhere, usually as CSV.

## Proposal

Import a CSV as a new table. Infer each column's type from its values, show a preview where types can be changed, then import. Rows that fail conversion are reported, not dropped silently.

## Acceptance criteria

- [ ] A 100 MB CSV imports in under 10 seconds with a progress indicator
- [ ] Delimiter, header row, and encoding (UTF-8, UTF-16, Windows-1252) are detected, covered by tests
- [ ] Values like `MARCH1` and `1-2` stay text unless the user chooses otherwise
