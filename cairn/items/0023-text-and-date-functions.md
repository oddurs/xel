---
id: 23
uid: eb41022e-95cc-4d1d-ad48-10c2b8601786
title: Text and date functions
type: feature
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

Cleaning text and working with dates are the next most common spreadsheet tasks.

## Proposal

Implement CONCAT, LEFT, RIGHT, MID, LEN, UPPER, LOWER, PROPER, TRIM, SUBSTITUTE, FIND, SEARCH, TEXT (common formats), VALUE, TODAY, NOW, DATE, YEAR, MONTH, DAY, WEEKDAY, DATEDIF, EDATE, EOMONTH.

## Acceptance criteria

- [ ] Each function has test vectors matching Excel, including leap years for date functions
- [ ] TODAY and NOW are marked volatile in the registry
