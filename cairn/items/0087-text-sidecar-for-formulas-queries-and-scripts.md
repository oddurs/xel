---
id: 87
uid: f3b96a58-a1a8-4958-b803-0847ffd2e800
title: Text sidecar for formulas, queries, and scripts
type: feature
status: backlog
milestone: v0.4
depends_on:
- 79
created: 2026-11-01
updated: 2026-11-01
priority: p2
effort: m
area: format
---

## Problem

Binary files hide changes from code review.

## Proposal

Optionally write formulas, query text, and Python blocks to a `<name>.xel.d/` directory of text files on save, so git shows changes in review.

## Acceptance criteria

- [ ] Changing one formula changes one line in the sidecar
