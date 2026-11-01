---
id: 17
uid: 9c98bf20-36a4-4ded-8a10-2b5d239c37f2
title: Number and date display formats per column
type: feature
status: planned
milestone: v0.1
depends_on:
- 14
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: m
area: format
---

## Problem

Excel users expect 1234.5 to show as 1,234.50 or $1,234.50 and dates as they choose.

## Proposal

Columns carry a display format: decimals, thousands separator, currency symbol, percent, and a date style. Formats affect display only, never the stored value.

## Acceptance criteria

- [ ] Changing a format never changes what `sqlite3` returns
- [ ] Formats use the system locale's separators by default, covered by tests for en_US and is_IS
