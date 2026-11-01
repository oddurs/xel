---
id: 62
uid: 0b429b26-1302-464c-ad5a-f8b7d01ea0ad
title: Translate Excel formulas, keeping what cannot run
type: feature
status: backlog
milestone: v0.3
depends_on:
- 61
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: formula
---

## Problem

Imported formulas use functions and references xel may not support.

## Proposal

Rewrite imported formulas into xel's references. A formula xel cannot evaluate is kept verbatim with its last value cached and a visible flag.

## Acceptance criteria

- [ ] A formula with an unsupported function shows Excel's last value and a flag explaining why
- [ ] Supported formulas compute the same values as Excel's cached values on the import corpus
