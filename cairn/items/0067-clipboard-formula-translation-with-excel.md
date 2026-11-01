---
id: 67
uid: 2f775eec-f1be-4b3e-9bf0-f4c98d6d12dd
title: Clipboard formula translation with Excel
type: feature
status: backlog
milestone: v0.3
depends_on:
- 37
- 62
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: m
area: interop
---

## Problem

Copying between Excel and xel should carry formulas, not just values.

## Proposal

Read and write Excel's clipboard formats so formulas paste translated in both directions.

## Acceptance criteria

- [ ] A block of formulas copied from Excel computes the same values after pasting into a calc block
