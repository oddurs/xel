---
id: 61
uid: cf1cb93a-4c67-46e1-a8e5-37940fa40314
title: Import .xlsx workbooks
type: feature
status: backlog
milestone: v0.3
depends_on:
- 50
- 60
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: l
area: interop
---

## Problem

Excel users arrive with .xlsx files.

## Proposal

Excel Tables become typed tables (types inferred from values and number formats); other used ranges become calc blocks on a sheet per worksheet; values, formulas, and number formats carry over.

## Acceptance criteria

- [ ] A worksheet with an Excel Table and a summary range imports as a table plus a calc block
- [ ] Import never fails because of one unsupported feature; it reports what was skipped
