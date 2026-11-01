---
id: 91
uid: b5eeae51-7cb9-42cd-ae49-3d8ad08d630c
title: Stabilize the Python API
type: feature
status: backlog
milestone: v1.0
depends_on:
- 77
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: python
---

## Problem

Scripts written against 1.0 must keep working.

## Proposal

Review and document every public name, remove deprecated ones, and publish API reference docs.

## Acceptance criteria

- [ ] Every public function has a docstring and an example that runs in CI
