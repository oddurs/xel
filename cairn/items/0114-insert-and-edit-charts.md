---
id: 114
uid: fff62cce-4170-45d8-91c0-f00584092a8d
title: Insert and edit charts
type: feature
status: backlog
milestone: v0.4
depends_on:
- 112
- 113
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: l
area: app
---

## Problem

Users need to go from a selection to a chart in one step.

## Proposal

Insert Chart from a selection, table, view, query, or pivot suggests a chart type from the data's shape. The inspector switches type and series. Supported types: column, bar, line, area, scatter, pie, and combo with a secondary axis. Charts update live.

## Acceptance criteria

- [ ] A chart over a table updates within one frame of an edit to its data
- [ ] Charts follow Dark Mode, Increase Contrast, and the system accent color
- [ ] VoiceOver describes each chart with Audio Graphs
