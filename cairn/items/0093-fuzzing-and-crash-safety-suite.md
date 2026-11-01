---
id: 93
uid: e9a2a4f6-e07a-476b-9529-c8bb357f00af
title: Fuzzing and crash-safety suite
type: chore
status: backlog
milestone: v1.0
depends_on:
- 31
- 61
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: format
---

## Problem

A spreadsheet that corrupts files loses users permanently.

## Proposal

Fuzz the formula parser, CSV import, and xlsx import continuously; add a test that kills the process at random points during save and checks the file.

## Acceptance criteria

- [ ] 10,000 random kill points never produce an unreadable file
- [ ] Fuzzers run nightly and any crash opens an issue
