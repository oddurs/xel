---
id: 126
uid: da75b9a3-a500-4463-b6e1-81e3a13a3a32
title: Recover unsaved edits after a crash
type: feature
status: backlog
milestone: v0.2
depends_on:
- 11
created: 2026-11-01
updated: 2026-11-01
priority: p2
effort: m
area: app
---

## Problem

A document's edits live in a private working copy (0011) until autosave writes them to the user's file. If xel crashes, the edits since the last autosave are lost, and the working copy is left in a temporary directory.

## Proposal

Keep working copies under Application Support, keyed by the document's bookmark. On launch, offer to restore any working copy newer than its document, then delete it.

## Acceptance criteria

- [ ] Killing xel after an edit and before autosave, then relaunching, offers the edit back; a test drives this through the document layer
- [ ] Declining or restoring removes the leftover working copy
- [ ] Working copies of documents that no longer exist are removed after 7 days
