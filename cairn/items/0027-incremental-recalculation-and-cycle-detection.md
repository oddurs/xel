---
id: 27
uid: 760c676e-d23f-4dc0-8907-5886f455b5e5
title: Incremental recalculation and cycle detection
type: feature
status: planned
milestone: v0.1
depends_on:
- 25
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: m
area: recalc
---

## Problem

Recomputing every formula on every edit does not scale to large tables.

## Proposal

Track dependencies between formula columns and totals. An edit recomputes only dependents, in topological order. A cycle marks the involved cells with a circular-reference error instead of looping.

## Acceptance criteria

- [ ] Editing one cell in a 1,000,000-row table with 3 formula columns recalculates in under 20 ms in a benchmark
- [ ] A formula column referring to itself through another column shows a circular-reference error, covered by a test
