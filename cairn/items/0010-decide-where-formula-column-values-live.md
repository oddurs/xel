---
id: 10
uid: 15cdfacc-bfb9-4d3a-af7f-b43b3dcc7613
title: Decide where formula column values live
type: spike
status: planned
milestone: v0.1
depends_on:
- 9
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: s
area: recalc
---

## Question

Are formula column results written into the SQLite table as ordinary columns (materialized), computed on read, or stored as SQLite generated columns?

## Why it matters

The promise is that `sqlite3` sees what the app shows. Materializing keeps that promise but makes every edit a write fan-out; computing on read breaks it.

## Timebox

Two days. Measure recalc and file size for 100,000 rows with 5 formula columns under each approach; record numbers in the Answer.

## Answer

Write the answer here before closing. A spike that closes without one was wasted.
