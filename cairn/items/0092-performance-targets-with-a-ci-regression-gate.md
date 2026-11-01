---
id: 92
uid: 95796857-4fe7-4528-abc8-e4f9d78bac85
title: Performance targets with a CI regression gate
type: chore
status: backlog
milestone: v1.0
depends_on:
- 27
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: infra
---

## Problem

Performance claims are worthless without measurement.

## Proposal

A benchmark suite covering open, scroll window reads, recalc, query, and import, run in CI against stored baselines.

## Acceptance criteria

- [ ] Opening a 1,000,000-row workbook takes under 1 second on an M1
- [ ] A regression over 10 percent fails CI
