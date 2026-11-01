---
id: 77
uid: befe8f1b-51f3-496b-bc1f-687315c5d8db
title: The `xel` Python package
type: feature
status: backlog
milestone: v0.4
depends_on:
- 16
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: python
---

## Problem

Scripting workbooks from outside is half of the development-environment promise.

## Proposal

A PyO3 crate building a `xel` module: open a workbook, list tables, read a table as rows or a pandas or polars frame, write rows, and save, using the same engine as the app.

## Acceptance criteria

- [ ] `xel.open(path).table("Orders").to_pandas()` returns typed columns
- [ ] Writes from Python appear in the app after reopening, covered by a test
