---
id: 70
uid: 0dd3de0b-f5dd-45c8-8242-f6d59034f9b8
title: Dynamic arrays and spill
type: feature
status: backlog
milestone: v0.3
depends_on:
- 49
- 120
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: recalc
---

## Problem

Modern Excel formulas return arrays that spill into neighboring cells.

## Proposal

Support array results in calc blocks: spill ranges, the `#` spill reference, and the #SPILL! error when blocked. Add FILTER, SORT, SORTBY, UNIQUE, SEQUENCE.

## Acceptance criteria

- [ ] A spill blocked by a value shows #SPILL! and resolves when the value is removed
- [ ] `=SUM(A1#)` follows the spill as it grows
