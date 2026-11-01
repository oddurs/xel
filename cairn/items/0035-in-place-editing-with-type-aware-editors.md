---
id: 35
uid: 267d8acb-7440-480a-969e-1723f2ebcce3
title: In-place editing with type-aware editors
type: feature
status: planned
milestone: v0.1
depends_on:
- 34
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: l
area: grid
---

## Problem

Data has to be typed in.

## Proposal

Typing starts an edit; Return and F2 edit; Escape cancels. Text and numbers use the system field editor; checkboxes toggle with Space; dates get a date picker popover; selects get a menu. Invalid input shows an inline error and does not commit.

## Acceptance criteria

- [ ] Entering "abc" in a number column leaves the cell in edit mode with an error message
- [ ] Every edit is one undo step
- [ ] Editing works with VoiceOver announcing the cell and its column
