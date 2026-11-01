---
id: 58
uid: ac39133c-ba5a-4533-a48b-cac5c8ed2823
title: More field types
type: feature
status: backlog
milestone: v0.2
depends_on:
- 35
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: m
area: format
---

## Problem

v0.1's five types miss common shapes of data.

## Proposal

Add multiple select, currency, percent, rating, URL, and email, each with storage, validation, display, and an editor.

## Acceptance criteria

- [ ] Each type round-trips through `sqlite3` as documented in docs/format.md
- [ ] Each type has a grid editor and inspector settings
