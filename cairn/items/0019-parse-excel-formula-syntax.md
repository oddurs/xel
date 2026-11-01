---
id: 19
uid: bdf3b927-73da-4805-858b-dbf5cb544a7b
title: Parse Excel formula syntax
type: feature
status: planned
milestone: v0.1
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: l
area: formula
---

## Problem

Formulas are text until they are parsed into something the evaluator can run.

## Proposal

A lexer and parser producing an AST for: numbers, strings, booleans, error literals, operators with Excel's precedence (including unary minus before ^, &, %), function calls, structured references (`[@Amount]`, `Orders[Amount]`), and A1 references and ranges (parsed now, evaluated in v0.2). Errors point at a character offset.

## Acceptance criteria

- [ ] `=-2^2` parses to `(-2)^2` like Excel, covered by a test
- [ ] Every example formula in the function reference parses, covered by a table-driven test
- [ ] Malformed input returns an error with the offset, never a panic; fuzzed for 10 minutes in CI
