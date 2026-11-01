---
id: 18
uid: eb4f786a-39eb-4aa8-9738-829af4385b7f
title: Document the v0 file format
type: docs
status: planned
milestone: v0.1
depends_on:
- 14
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: s
area: docs
---

## Problem

The pitch is that any tool can read a .xel file. That needs a written schema.

## Proposal

Write `docs/format.md`: `xel_meta`, `xel_tables`, `xel_columns`, how each type is stored, and the versioning rule. Mark it unstable until v1.0.

## Acceptance criteria

- [ ] Someone can read a v0.1 file with Python's `sqlite3` module using only this document
- [ ] Each metadata table in the code has a section in the document
