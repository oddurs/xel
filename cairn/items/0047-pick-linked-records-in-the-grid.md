---
id: 47
uid: 29cf5571-5fe3-46f1-acf6-7279087271c1
title: Pick linked records in the grid
type: feature
status: backlog
milestone: v0.2
depends_on:
- 35
- 46
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: grid
---

## Problem

Linking needs a fast way to choose the other row.

## Proposal

Linked cells show the linked row's primary field as a pill. Editing opens a searchable popover of the other table's rows; typing filters.

## Acceptance criteria

- [ ] Picking among 100,000 rows filters as the user types with no visible lag
- [ ] Keyboard-only linking works
