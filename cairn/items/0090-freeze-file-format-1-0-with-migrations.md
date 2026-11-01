---
id: 90
uid: 32e290d5-c709-40cb-895a-1cb8a4661b9b
title: Freeze file format 1.0 with migrations
type: feature
status: backlog
milestone: v1.0
depends_on:
- 18
- 85
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: format
---

## Problem

1.0 promises that files keep opening.

## Proposal

Finalize docs/format.md as 1.0, write migrations from every earlier format version, and keep fixture files of each version in the repository.

## Acceptance criteria

- [ ] Every fixture from every earlier version opens and migrates in CI
- [ ] The spec states the compatibility rule for future versions
