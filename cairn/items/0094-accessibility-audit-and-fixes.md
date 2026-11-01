---
id: 94
uid: 0cf3b7cb-90f2-417f-8f71-7dd6188e0c9e
title: Accessibility audit and fixes
type: chore
status: backlog
milestone: v1.0
depends_on:
- 32
- 35
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: app
---

## Problem

A native app must work with VoiceOver and keyboard alone.

## Proposal

Audit every screen with VoiceOver, Full Keyboard Access, Increase Contrast, Reduce Motion, and larger text, and fix what fails.

## Acceptance criteria

- [ ] The audit checklist is in docs/ with every item passing
- [ ] A VoiceOver user can build a table with a formula column and a total
