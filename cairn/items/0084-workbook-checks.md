---
id: 84
uid: 93390326-aa4c-439f-8a00-40a177c0c305
title: Workbook checks
type: feature
status: backlog
milestone: v0.4
depends_on:
- 49
- 83
created: 2026-11-01
updated: 2026-11-01
priority: p2
effort: m
area: recalc
---

## Problem

Models have invariants (assets equal liabilities plus equity) that should hold after every change.

## Proposal

Named boolean formulas that run on every recalc and appear in the issues list when false.

## Acceptance criteria

- [ ] A failing check names itself and shows its inputs' current values
