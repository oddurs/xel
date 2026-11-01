---
id: 98
uid: f744858f-0a5d-479f-9d90-635c4cd3148a
title: Function reference generated from the registry
type: docs
status: backlog
milestone: v1.0
depends_on:
- 69
- 71
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: docs
---

## Problem

Function documentation drifts unless it comes from the code.

## Proposal

Generate a reference page per function from the registry: signature, description, examples, and Excel differences. CI fails when the generated docs are stale.

## Acceptance criteria

- [ ] Every registered function has a page with at least one example verified by tests
