---
id: 42
uid: 53fe058c-3ea3-4b25-8622-c45ead327946
title: Release workflow for notarized builds
type: chore
status: planned
milestone: v0.1
depends_on:
- 12
- 29
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: infra
---

## Problem

There is no way for a stranger to install xel.

## Proposal

Extend `release.yml`: on a version tag, build the universal app, sign, notarize, staple, and attach a .dmg to the GitHub release, following the packaging spike.

## Acceptance criteria

- [ ] A tag produces a release whose .dmg opens without Gatekeeper warnings on a clean Mac
- [ ] Signing secrets are only available to the release job
