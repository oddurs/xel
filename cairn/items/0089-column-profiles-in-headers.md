---
id: 89
uid: 78816c5f-9b34-4ea6-a575-06116faa7dcd
title: Column profiles in headers
type: feature
status: backlog
milestone: v0.4
depends_on:
- 33
created: 2026-11-01
updated: 2026-11-01
priority: p2
effort: m
area: grid
---

## Problem

Understanding a column's data means scrolling it today.

## Proposal

An optional header strip shows distribution sparkline, null count, and distinct count per column.

## Acceptance criteria

- [ ] Profiles for a 1,000,000-row table compute in the background without blocking edits
