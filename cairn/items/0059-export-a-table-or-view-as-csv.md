---
id: 59
uid: 60518d0c-8a9b-4a7a-a9fe-3edf4db2646d
title: Export a table or view as CSV
type: feature
status: backlog
milestone: v0.2
depends_on:
- 54
created: 2026-11-01
updated: 2026-11-01
priority: p2
effort: s
area: interop
---

## Problem

Data needs to leave too.

## Proposal

Export the current view (respecting filter, sort, and hidden fields) as UTF-8 CSV.

## Acceptance criteria

- [ ] Values that contain delimiters, quotes, and newlines are quoted correctly, covered by tests
