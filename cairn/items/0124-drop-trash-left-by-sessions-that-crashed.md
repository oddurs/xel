---
id: 124
uid: 2657494c-ee5f-402e-b985-bd4058615f4b
title: Drop trash left by sessions that crashed
type: chore
status: backlog
milestone: v1.0
depends_on:
- 16
created: 2026-11-01
updated: 2026-11-01
priority: p2
effort: s
area: format
---

## Problem

Undo trash tables carry their session's id, and each session drops only its own on close (0016). A session that crashes never closes, so its `xel_trash_<session>_*` tables stay in the file for good.

## Proposal

Garbage-collect trash whose session is provably gone, for example by recording live sessions with a heartbeat in `xel_meta`, or by dropping trash only when the file is opened with no other connection (`BEGIN EXCLUSIVE` succeeding on open).

## Acceptance criteria

- [ ] Trash from a killed session is dropped on a later open, covered by a test
- [ ] Trash of a live second session is never dropped, covered by a test
