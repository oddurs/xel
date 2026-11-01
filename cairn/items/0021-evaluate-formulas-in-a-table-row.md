---
id: 21
uid: 42bd5876-6883-403c-b70a-f3f894bca9ed
title: Evaluate formulas in a table row
type: feature
status: planned
milestone: v0.1
depends_on:
- 19
- 20
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: formula
---

## Problem

A parsed formula needs to produce a value for a given row of a table.

## Proposal

Evaluate operators and structured references against a row context (`[@Col]` is this row's value, `Table[Col]` is the whole column). Unknown names give #NAME?. Functions dispatch through a registry so the library can grow item by item.

## Acceptance criteria

- [ ] `=[@Qty]*[@Price]` evaluates per row, covered by a test
- [ ] A reference to a missing column yields #REF! and the cell shows it
- [ ] Evaluation of 100,000 simple row formulas takes under 50 ms in a benchmark
