---
id: 34
uid: 0b644489-0fac-4c56-84c1-88dcd8c0031c
title: Selection and keyboard navigation
type: feature
status: planned
milestone: v0.1
depends_on:
- 33
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: grid
---

## Problem

Spreadsheet users live on the keyboard.

## Proposal

Click, shift-click, and drag selection; arrows, ⌘-arrows to the data edge, Tab and Return movement, Page Up and Page Down, select row and column, ⌘A. Selection uses the system accent color.

## Acceptance criteria

- [ ] Every Excel navigation shortcut listed in the PR works, with a test for each
- [ ] Selection survives scrolling the selected cell out of view and back
