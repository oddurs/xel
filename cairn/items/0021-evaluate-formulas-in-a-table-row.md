---
id: 21
uid: 42bd5876-6883-403c-b70a-f3f894bca9ed
title: Evaluate formulas in a table row
type: feature
status: done
milestone: v0.1
assignee: Oddur Sigurdsson
depends_on:
- 19
- 20
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
priority: p0
effort: m
area: formula
---

## Problem

A parsed formula needs to produce a value for a given row of a table.

## Proposal

Evaluate operators and structured references against a row context (`[@Col]` is this row's value, `Table[Col]` is the whole column). Unknown names give #NAME?. Functions dispatch through a registry so the library can grow item by item.

## Acceptance criteria

- [x] `=[@Qty]*[@Price]` evaluates per row, covered by a test
- [x] A reference to a missing column yields #REF! and the cell shows it
- [x] Evaluation of 100,000 simple row formulas takes under 50 ms in a benchmark

## 2026-11-01

Benchmark (release, M4 Pro): 100,000 evaluations of =[@Qty]*[@Price]+1 in 8.9 ms (cargo test --release -p xel-engine row_formula_benchmark -- --ignored --nocapture). Missing column: #REF! (missing_columns_are_ref_errors_and_unknown_names_are_name_errors).

## Result

crates/engine/src/formula/eval.rs: evaluate(expr, &dyn Context) -> Value; Context trait (this_row, column, cell, range, name); lazy Args (len, is_missing, value, arg) and Arg {Value, Range}; Function registry (name, min/max args, volatile, signature, description, eval) sorted for binary search via function(name) and functions(); unknown function #NAME?, wrong arity #VALUE!, missing column #REF!, whole column in a scalar spot #VALUE!. Ships FALSE(), TRUE(), NA() only; families come in 0022/0023.
