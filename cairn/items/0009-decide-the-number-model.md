---
id: 9
uid: a8b6d792-977b-47a3-8a64-bd86688957f6
title: Decide the number model
type: spike
status: planned
milestone: v0.1
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: s
area: formula
---

## Question

Store and compute numbers as IEEE doubles like Excel, as decimals, or doubles plus a decimal currency type? How are values shown so they match Excel's 15 significant digits?

## Why it matters

It fixes the storage type of number columns and the evaluator's arithmetic. Changing it after files exist needs a format migration.

## Timebox

One day. Answer with the decision and the test vectors (0.1+0.2, 1e308, large currency sums) that justify it.

## Answer

Write the answer here before closing. A spike that closes without one was wasted.
