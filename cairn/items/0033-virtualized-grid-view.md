---
id: 33
uid: 2aa51a9f-65eb-431c-8d44-5c6b87e49daf
title: Virtualized grid view
type: feature
status: planned
milestone: v0.1
depends_on:
- 8
- 28
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: l
area: grid
---

## Problem

Tables need to be seen.

## Proposal

Implement the grid with the approach chosen in the grid spike: column headers, row numbers, cell drawing per type (checkbox, select pill, right-aligned tabular numbers), Dark Mode and Increase Contrast support. Rows load through the engine window API.

## Acceptance criteria

- [ ] A 1,000,000-row table scrolls without dropped frames on a ProMotion Mac, measured with Instruments and recorded in the PR
- [ ] Memory stays under 200 MB while scrolling the full table
