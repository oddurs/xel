---
id: 85
uid: 227bb79f-71d2-4166-b203-3f9aa7f521c7
title: Change history in the file
type: feature
status: backlog
milestone: v0.4
depends_on:
- 16
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: l
area: format
---

## Problem

Undo ends when the window closes; work needs history.

## Proposal

Record committed changes in an append-only `xel_changes` log with timestamps and named snapshots, compactable. Reconstruct any table or cell at a past point.

## Acceptance criteria

- [ ] Reconstructing a table at a past snapshot matches what it was, covered by property tests
- [ ] History adds under 20 percent to file size for a typical editing session, measured
