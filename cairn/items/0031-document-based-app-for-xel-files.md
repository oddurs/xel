---
id: 31
uid: 5c65092c-eacb-4f34-9e2b-4ffd0f800b11
title: Document-based app for .xel files
type: feature
status: blocked
milestone: v0.1
assignee: Oddur Sigurdsson
claimed: 2026-11-01
depends_on:
- 11
- 28
- 118
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: app
---

## Problem

The window is not connected to any file.

## Proposal

Make the app document-based with a `.xel` file type: New, Open, Save, Revert, Open Recent, autosave, and window restoration, using the pattern from the document spike.

## Acceptance criteria

- [x] Double-clicking a .xel file in Finder opens it in xel
- [x] Quitting with unsaved changes follows standard macOS behavior
- [x] Killing the app during save leaves either the old or the new file, never a corrupt one, covered by a test
- [x] Uses an AppKit NSDocument over `open_copy`/`save_copy` as 0011 answers; the working copy lives in a per-document temporary directory and is removed on close
- [ ] Revert to Saved, and browsing and restoring a version in the Versions browser, show the right data (checked by hand, steps in the PR)
- [x] Replacing the file from outside while it is open (as iCloud would) reloads it instead of overwriting it
- [x] `LSHandlerRank` in `app/Bundle/Info.plist` is `Owner`

## 2026-11-01

Evidence (built app, driven by open + System Events): open Budget.xel (the LaunchServices path a Finder double-click takes) launched the registered Xel.app and showed its tables (1). Adding tables then quitting saved them to the file without a prompt, standard for autosave-in-place apps, and removed the working copy (2, 4). Replacing the open file from outside reloaded the window within 1 s and quitting did not overwrite it (6). Tests: XelUITests drive writeSafely, revert, undo, outside replacement, and save/undo/close right after an edit; the engine's a_kill_during_save_never_damages_the_document covers the save pattern NSDocument uses (3). LSHandlerRank is Owner (7). Criterion 5: Revert is covered by undoAndRevertFollowTheEngine; the Versions browser needs the owner's hand check, steps in the PR.

## 2026-11-01

Blocked on the owner: criterion 5 (Versions browser browse and restore) needs a hand check; steps are in the PR. Close once checked.
