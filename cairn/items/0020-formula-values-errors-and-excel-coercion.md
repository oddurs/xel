---
id: 20
uid: 7ac1fb50-ae7f-4196-8385-51ae1c9d90d2
title: Formula values, errors, and Excel coercion
type: feature
status: done
milestone: v0.1
assignee: Oddur Sigurdsson
depends_on:
- 9
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
priority: p0
effort: m
area: formula
---

## Problem

Excel's behavior depends on its value types and coercions ("1"+1 is 2, TRUE+1 is 2).

## Proposal

A `Value` type: number, text, boolean, date, empty, and error (#DIV/0!, #VALUE!, #REF!, #NAME?, #N/A, #NUM!, #NULL!). Implement Excel's coercion rules for arithmetic, comparison, and concatenation.

## Acceptance criteria

- [x] Coercion and comparison follow Excel for every pair of types, covered by a table of vectors with their source cited
- [x] Errors propagate through operators the way Excel's do, covered by tests

## 2026-11-01

Vector sources are Microsoft's documentation (Calculation operators and precedence; How to correct a #VALUE! error) and the 0009 decision, cited per row. None were recorded in a live Excel; the conformance harness (0024) adds Excel-recorded vectors and may correct these. General-format thresholds for scientific notation (>= 1e15, < 1e-9) are an approximation to be checked there.

## Result

crates/engine/src/formula/value.rs: Value {Empty, Number, Text, Bool, Date(serial), Error}; to_number/to_text coercions; unary/binary operator semantics (left operand converted first, date arithmetic keeps date-ness); compare (numbers < text < bool, case-insensitive text, 15-digit equality); round15; general (number to text); number_from_text; Excel 1900 date serials with the phantom 1900-02-29 (serial_from_iso, iso_from_serial).
