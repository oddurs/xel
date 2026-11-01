---
id: 63
uid: b5d722b2-3f24-4e32-9aab-d6e7041623da
title: Export to .xlsx
type: feature
status: backlog
milestone: v0.3
depends_on:
- 50
- 52
- 60
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: interop
---

## Problem

Colleagues who only have Excel need the work.

## Proposal

Tables become Excel Tables with structured references, calc blocks become ranges, formats carry over. Query and future Python results export as values with their source text in a cell note.

## Acceptance criteria

- [ ] An exported workbook opens in Excel without a repair prompt
- [ ] Formulas in exported tables recalculate in Excel to the same values
