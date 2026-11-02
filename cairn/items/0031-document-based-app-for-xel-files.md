---
id: 31
uid: 5c65092c-eacb-4f34-9e2b-4ffd0f800b11
title: Document-based app for .xel files
type: feature
status: planned
milestone: v0.1
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

- [ ] Double-clicking a .xel file in Finder opens it in xel
- [ ] Quitting with unsaved changes follows standard macOS behavior
- [ ] Killing the app during save leaves either the old or the new file, never a corrupt one, covered by a test
- [ ] Uses an AppKit NSDocument over `open_copy`/`save_copy` as 0011 answers; the working copy lives in a per-document temporary directory and is removed on close
- [ ] Revert to Saved, and browsing and restoring a version in the Versions browser, show the right data (checked by hand, steps in the PR)
- [ ] Replacing the file from outside while it is open (as iCloud would) reloads it instead of overwriting it
- [ ] `LSHandlerRank` in `app/Bundle/Info.plist` is `Owner`
