---
id: 45
uid: f9f9ff67-81c7-4e0c-8b8a-aa4ac9f8537f
title: Run DuckDB over the open SQLite file
type: spike
status: backlog
milestone: v0.2
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: s
area: query
---

## Question

Can DuckDB's SQLite extension query the open workbook with read-after-write consistency inside an edit session, and what does it add to app size and startup time?

## Why it matters

Query blocks depend on it. If it cannot see uncommitted edits or costs too much, query blocks run on SQLite instead.

## Timebox

Two days. Answer with consistency results, binary size delta, cold-start delta, and query timings against SQLite on 1,000,000 rows.

## Answer

Write the answer here before closing. A spike that closes without one was wasted.
