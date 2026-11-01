---
id: 54
uid: 1dc11d5b-c630-4a6f-928c-c93596885bd2
title: 'Saved views: filter, sort, group, hidden fields'
type: feature
status: backlog
milestone: v0.2
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: format
---

## Problem

Users need different perspectives on the same table without changing the data.

## Proposal

Views are stored per table: filter conditions, sort keys, grouping fields, and hidden fields. The engine returns row windows in view order.

## Acceptance criteria

- [ ] Reading a window of a filtered, sorted 1,000,000-row view takes under 50 ms
- [ ] Views survive renaming the columns they use
