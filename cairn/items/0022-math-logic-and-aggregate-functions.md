---
id: 22
uid: e1953d0e-6fc6-49ab-a634-8c07f9e983aa
title: Math, logic, and aggregate functions
type: feature
status: done
milestone: v0.1
assignee: Oddur Sigurdsson
depends_on:
- 21
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
priority: p0
effort: m
area: formula
---

## Problem

Without functions, formulas are a calculator.

## Proposal

Implement SUM, AVERAGE, MIN, MAX, COUNT, COUNTA, COUNTBLANK, SUMIF, COUNTIF, AVERAGEIF, ROUND, ROUNDUP, ROUNDDOWN, INT, MOD, ABS, POWER, SQRT, IF, IFS, IFERROR, IFNA, AND, OR, NOT, XOR, SWITCH.

## Acceptance criteria

- [x] Each function has test vectors, including error and empty-argument cases, matching Excel
- [x] Each function has a one-line description and signature in the registry for autocomplete

## 2026-11-01

Vectors follow Microsoft's function documentation; none were recorded in a live Excel. The conformance harness (0024) adds Excel-recorded vectors. Registry metadata (signature, description) is checked by the_registry_is_sorted_unique_and_complete.

## Result

crates/engine/src/formula/functions/{mod,math,logic,criteria}.rs: SUM AVERAGE MIN MAX COUNT COUNTA COUNTBLANK ROUND ROUNDUP ROUNDDOWN INT MOD ABS POWER SQRT IF IFS IFERROR IFNA AND OR NOT XOR SWITCH SUMIF COUNTIF AVERAGEIF; registry moved to functions/mod.rs; helpers each_value (typed vs referenced values), numbers, truth, Criterion (operators, wildcards with ~, case-insensitive).
