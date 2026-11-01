---
id: 66
uid: 20c40ca8-bb7f-4ef4-b6de-4a30a7781621
title: xlsx round-trip test corpus
type: chore
status: backlog
milestone: v0.3
depends_on:
- 62
- 63
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: interop
---

## Problem

Import and export bugs show up only on real files.

## Proposal

Collect 20 or more openly licensed real-world .xlsx files (budgets, trackers, financial models). CI imports each, compares computed values with Excel's cached values, exports, and re-imports.

## Acceptance criteria

- [ ] Every file's licence is recorded next to it
- [ ] CI fails on any value mismatch, listing the cells
