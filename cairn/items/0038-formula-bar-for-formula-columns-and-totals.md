---
id: 38
uid: d9c70e3f-94b8-4a36-bd69-af216b789983
title: Formula bar for formula columns and totals
type: feature
status: planned
milestone: v0.1
depends_on:
- 25
- 34
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: app
---

## Problem

Formulas need a place to be written and read.

## Proposal

Selecting a formula column cell or a total shows its formula in a bar above the grid in SF Mono with syntax coloring. Editing the formula there applies to the whole column. Parse errors show inline at their offset.

## Acceptance criteria

- [ ] A parse error highlights the offending character and does not apply the formula
- [ ] Function names autocomplete from the registry with their signature
