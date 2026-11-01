---
id: 37
uid: c27a6331-51c8-4e77-b3f8-c7b1c5b97a5e
title: Copy and paste as tab-separated text
type: feature
status: planned
milestone: v0.1
depends_on:
- 35
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: m
area: grid
---

## Problem

Moving data in and out by copy and paste is how spreadsheets are actually used.

## Proposal

Copy a selection as TSV and as the internal format; paste TSV from any app into a selection, converting to column types and reporting cells that could not be converted.

## Acceptance criteria

- [ ] Pasting a 1,000 × 10 block from Numbers or Excel lands in the right cells
- [ ] A paste that fails type conversion for some cells reports how many and which
