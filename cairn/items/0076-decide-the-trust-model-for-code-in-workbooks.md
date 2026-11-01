---
id: 76
uid: a8e036a8-7781-4a38-a297-0b33eb936465
title: Decide the trust model for code in workbooks
type: spike
status: backlog
milestone: v0.4
depends_on:
- 75
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: s
area: python
---

## Question

What does opening a workbook that contains Python allow before the user trusts it, how is trust recorded, and can blocks run in a sandboxed process?

## Why it matters

Opening a file must never run code silently. Excel's Enable Content is the baseline to beat.

## Timebox

Two days. Answer with the trust UX, storage of trust decisions, and the process model.

## Answer

Write the answer here before closing. A spike that closes without one was wasted.
