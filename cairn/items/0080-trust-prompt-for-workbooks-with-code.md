---
id: 80
uid: d3052af0-457e-4356-aaa5-9df2de2fdfbc
title: Trust prompt for workbooks with code
type: feature
status: backlog
milestone: v0.4
depends_on:
- 76
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: app
---

## Problem

Users must decide before a downloaded workbook runs code.

## Proposal

Opening a workbook with Python blocks from an untrusted source shows a bar explaining what will run, with Trust and Keep Disabled. Trust decisions are stored per file as the spike decided.

## Acceptance criteria

- [ ] A quarantined downloaded workbook never runs a block before the user trusts it, covered by a test
