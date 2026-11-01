---
id: 15
uid: c4a8f96f-51c6-44d1-9824-4ee63f7275b5
title: Insert, edit, and delete rows with type checking
type: feature
status: planned
milestone: v0.1
depends_on:
- 14
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: format
---

## Problem

Tables are useless until data can go in and come out.

## Proposal

Engine operations to insert rows, set a cell, delete rows, and read a window of rows (offset and limit) in display order. Setting a value of the wrong type returns a typed error that says which column expected what.

## Acceptance criteria

- [ ] Writing "abc" to a number column returns an error naming the column and type
- [ ] Reading rows 500,000–500,100 of a 1,000,000-row table takes under 10 ms in a benchmark
- [ ] Deleting rows keeps the remaining row order stable, covered by a test
