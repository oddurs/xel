---
id: 119
uid: 2039bcbe-4adf-4432-8bb7-2f6e71d18582
title: Parse whole-column ranges, whole-row ranges, and sheet references
type: feature
status: backlog
milestone: v0.2
depends_on:
- 19
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: s
area: formula
---

## Problem

The formula parser (0019) rejects `A:A`, `1:1`, and `Sheet2!A1` / `'My Sheet'!A1:B2`. Excel formulas use all three constantly, and calc blocks on several sheets need sheet references.

## Proposal

Extend `Reference` with whole-column and whole-row ranges and an optional sheet qualifier (quoted with `'` when it contains spaces, `''` escaping a quote). Remove these cases from the parser's unsupported list and add them to its corpus test.

## Acceptance criteria

- [ ] `=SUM(A:A)`, `=SUM(1:1)`, `=Sheet2!A1`, and `='Q1 Sales'!B2:C9` parse to the expected AST, covered by tests
- [ ] `=COUNTIFS(A:A,"x")` moves from the parser's unsupported list to its corpus
