---
id: 122
uid: c2d93a4f-d85e-4189-bc8e-e6238f78354d
title: Accept Excel's date and time text in arithmetic
type: feature
status: backlog
milestone: v0.3
depends_on:
- 20
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: m
area: formula
---

## Problem

Text-to-number conversion (0020) accepts ISO dates, US `M/D/YYYY`, and 24-hour `H:MM(:SS)`. Excel also converts `"8:30 AM"` (0.354), `"2024-01-31 12:00"` (45322.5), `"1/2/24"` (45293), and `"1-Jan-2024"` (45292). Imported formulas that rely on these give `#VALUE!` in xel.

## Proposal

Extend `number_from_text` with 12-hour times, date-time combinations, two-digit years (Excel's 1930 pivot), and month names, following the system locale once locale-aware entry lands.

## Acceptance criteria

- [ ] Each example above converts to the value shown, covered by tests
- [ ] Two-digit years follow Excel's rule (00–29 are 2000s, 30–99 are 1900s), covered by tests
