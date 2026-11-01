---
id: 118
uid: 48c00457-9c3b-4531-a625-8b1e95350981
title: Bundle Xel.app with document types and ad-hoc signing
type: chore
status: planned
milestone: v0.1
depends_on:
- 29
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: infra
---

## Problem

A bare SwiftPM executable has no Info.plist, so macOS does not treat it as a regular app (no Dock icon, no menu bar focus) and cannot associate .xel files with it. Document-based behavior needs a real bundle. Developer ID signing is blocked on an Apple Developer account (0012), but the bundle itself is not.

## Proposal

`scripts/task build` assembles `build/Xel.app` from the SwiftPM product: Info.plist with bundle id, version, `CFBundleDocumentTypes` and an exported UTI for `.xel` (conforming to `public.database`), the executable, and an ad-hoc signature (`codesign -s -`). `scripts/task run` is not added; README says how to open the bundle.

## Acceptance criteria

- [ ] `open build/Xel.app` launches xel as a regular app with a Dock icon and its own menu bar
- [ ] `mdls` / `lsregister` show the exported UTI after the bundle is opened once
- [ ] `codesign --verify --strict build/Xel.app` passes
- [ ] CI builds the bundle
