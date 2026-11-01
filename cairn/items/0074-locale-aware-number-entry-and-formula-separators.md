---
id: 74
uid: 598d1b8a-d102-4b04-95fd-63acf03743da
title: Locale-aware number entry and formula separators
type: feature
status: backlog
milestone: v0.3
depends_on:
- 19
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: m
area: formula
---

## Problem

In many locales, including Iceland's, Excel uses a decimal comma and `;` as the argument separator. xel must accept what those users type.

## Proposal

Number entry and formula text follow the system locale for decimal and list separators; the stored formula is canonical.

## Acceptance criteria

- [ ] With an Icelandic locale, typing `=ROUND(1,5;0)` gives 2, covered by a test
- [ ] Files created in one locale open correctly in another
