---
id: 50
uid: 5a273e0e-6968-46ca-afbc-bf48fba4c6f1
title: Sheets that show tables and blocks together
type: feature
status: backlog
milestone: v0.2
depends_on:
- 33
- 44
- 49
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: l
area: grid
---

## Problem

v0.1 shows one table per window pane. A sheet mixes tables and calc blocks.

## Proposal

Render a sheet using the layout from the canvas-or-flow spike. The grid component is reused for calc blocks and tables. Sheets appear in the sidebar.

## Acceptance criteria

- [ ] Keyboard navigation moves between blocks predictably, documented in the PR
- [ ] A sheet with 20 blocks scrolls smoothly
