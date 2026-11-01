---
id: 121
uid: 18020b61-6ae9-48ad-b3b1-e29c6ae12485
title: Compare and sort text with Excel's collation
type: feature
status: backlog
milestone: v0.3
depends_on:
- 20
created: 2026-11-01
updated: 2026-11-01
priority: p2
effort: m
area: formula
---

## Problem

Text comparison (0020) orders punctuation, digits, then letters and ignores case, but compares accented letters by code point: `"é" > "f"`, where Excel gives `"é" < "f"`. Sorting, MATCH, and comparisons on imported workbooks with non-English text will disagree with Excel.

## Proposal

Collate like Excel's en-US default: accents secondary to base letters, case-insensitive, with the class order already in place. Decide whether this needs a Unicode collation dependency or a table for Latin letters, and record the choice.

## Acceptance criteria

- [ ] `"é" < "f"`, `"Å" < "b"`, `"ö" < "p"`, and `"e" < "é" < "f"` hold, with vectors recorded in Excel via the conformance harness (0024)
- [ ] Comparing two ASCII strings allocates nothing, checked by a test or benchmark
