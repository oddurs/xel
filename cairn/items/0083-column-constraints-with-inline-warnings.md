---
id: 83
uid: fd712aa2-46fa-4eca-a280-229dbeca5f3b
title: Column constraints with inline warnings
type: feature
status: backlog
milestone: v0.4
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: m
area: format
---

## Problem

Data errors go unnoticed until they break a report.

## Proposal

Columns can be unique, required, or range-limited. Violations show as cell warnings and in a workbook issues list, never as a modal dialog.

## Acceptance criteria

- [ ] Adding a unique constraint to a column with duplicates lists them instead of failing
