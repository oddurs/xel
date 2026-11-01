---
id: 40
uid: 497faf0f-7687-45de-a445-323babcf6b3a
title: New workbook starts with a ready table
type: feature
status: backlog
milestone: v0.2
depends_on:
- 32
- 35
created: 2026-11-01
updated: 2026-11-01
priority: p2
effort: s
area: app
---

## Problem

An empty window with no table is a dead end for a new user.

## Proposal

A new workbook contains "Table 1" with a text column and a number column, focused on the first cell, so typing works immediately.

## Acceptance criteria

- [ ] Creating a new workbook and typing puts the text in A1 of Table 1 with no clicks
