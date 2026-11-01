---
id: 22
uid: e1953d0e-6fc6-49ab-a634-8c07f9e983aa
title: Math, logic, and aggregate functions
type: feature
status: planned
milestone: v0.1
depends_on:
- 21
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: formula
---

## Problem

Without functions, formulas are a calculator.

## Proposal

Implement SUM, AVERAGE, MIN, MAX, COUNT, COUNTA, COUNTBLANK, SUMIF, COUNTIF, AVERAGEIF, ROUND, ROUNDUP, ROUNDDOWN, INT, MOD, ABS, POWER, SQRT, IF, IFS, IFERROR, IFNA, AND, OR, NOT, XOR, SWITCH.

## Acceptance criteria

- [ ] Each function has test vectors, including error and empty-argument cases, matching Excel
- [ ] Each function has a one-line description and signature in the registry for autocomplete
