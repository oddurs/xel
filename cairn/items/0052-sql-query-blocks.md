---
id: 52
uid: d676ac37-ef53-4691-9384-049c1fbd5f88
title: SQL query blocks
type: feature
status: backlog
milestone: v0.2
depends_on:
- 45
- 50
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: l
area: query
---

## Problem

The file is a database, but users cannot query it inside the app.

## Proposal

A query block holds a SQL SELECT over the workbook's tables. Its result renders as a read-only table and re-runs when any table it reads changes. Errors show inline.

## Acceptance criteria

- [ ] A query joining two tables updates within 200 ms after an input edit
- [ ] A query that writes (`DELETE`, `UPDATE`) is rejected with an explanation
- [ ] SQL has syntax coloring and table and column autocomplete
