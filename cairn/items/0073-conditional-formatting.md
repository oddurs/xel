---
id: 73
uid: 60bff539-ce52-4f50-a10c-53e88dca5ca5
title: Conditional formatting
type: feature
status: backlog
milestone: v0.3
depends_on:
- 61
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: m
area: grid
---

## Problem

Imported workbooks use it heavily and users expect it.

## Proposal

Rules per column or range: value comparisons, text contains, top/bottom N, color scales, and data bars. Imported from and exported to .xlsx where Excel supports the rule.

## Acceptance criteria

- [ ] Rules survive xlsx round-trip for the supported kinds, covered by the corpus
