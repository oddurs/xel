---
id: 8
uid: ff4ba628-a0cf-4c09-8650-a8b57c47081f
title: Choose how the grid renders a million rows
type: spike
status: planned
milestone: v0.1
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: s
area: grid
---

## Question

Can NSTableView (view-based or cell-based) scroll a 1,000,000 × 50 table at 120 Hz with custom cell drawing, or does xel need a custom layer-tiled NSView?

## Why it matters

The grid is the product. A wrong choice shows up as jank that cannot be tuned away.

## Timebox

Three days. Prototype both against a synthetic data source; record frame times with Instruments on a ProMotion display in the Answer.

## Answer

Write the answer here before closing. A spike that closes without one was wasted.
