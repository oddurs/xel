---
id: 20
uid: 7ac1fb50-ae7f-4196-8385-51ae1c9d90d2
title: Formula values, errors, and Excel coercion
type: feature
status: planned
milestone: v0.1
depends_on:
- 9
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: formula
---

## Problem

Excel's behavior depends on its value types and coercions ("1"+1 is 2, TRUE+1 is 2).

## Proposal

A `Value` type: number, text, boolean, date, empty, and error (#DIV/0!, #VALUE!, #REF!, #NAME?, #N/A, #NUM!, #NULL!). Implement Excel's coercion rules for arithmetic, comparison, and concatenation.

## Acceptance criteria

- [ ] Coercion and comparison follow Excel for every pair of types, covered by a table of vectors with their source cited
- [ ] Errors propagate through operators the way Excel's do, covered by tests
