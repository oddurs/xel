---
id: 11
uid: 01a22288-f68e-4a78-a81a-10a99100ec4d
title: Prove NSDocument can own a SQLite file safely
type: spike
status: planned
milestone: v0.1
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: s
area: app
---

## Question

How does a document-based SwiftUI app keep autosave, Versions, Revert, and iCloud Drive working when the document is a live SQLite database rather than a blob written on save?

## Why it matters

NSDocument assumes it writes whole files atomically. A SQLite file written in place can be corrupted or silently forked by the versions machinery.

## Timebox

Three days. Answer with the chosen pattern (for example, working copy plus `VACUUM INTO` on save, or a file package) and a kill-during-save test result.

## Answer

Write the answer here before closing. A spike that closes without one was wasted.
