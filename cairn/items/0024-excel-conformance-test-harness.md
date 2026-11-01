---
id: 24
uid: 2eda8aed-b063-4398-ac14-34ac378554ac
title: Excel conformance test harness
type: chore
status: planned
milestone: v0.1
depends_on:
- 21
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: m
area: formula
---

## Problem

Hand-written assertions drift from what Excel actually does.

## Proposal

A data file of vectors (formula, inputs, expected value, source) run by one test. Vectors come from Microsoft's documentation examples and from values computed in Excel and recorded with the Excel version.

## Acceptance criteria

- [ ] Adding a vector requires no Rust code
- [ ] The harness reports every failing vector, not just the first
- [ ] At least 200 vectors cover the v0.1 functions
