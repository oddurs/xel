---
id: 64
uid: bc0211fc-9f94-4ff8-8c07-a74817527cce
title: Export linked records and lookups as XLOOKUP
type: feature
status: backlog
milestone: v0.3
depends_on:
- 48
- 63
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: interop
---

## Problem

Excel has no linked records; exporting only values loses the model.

## Proposal

Export the linked key column and rewrite lookups and rollups as XLOOKUP and SUMIFS over the linked table.

## Acceptance criteria

- [ ] Changing a linked value in Excel after export updates its lookups
