---
id: 46
uid: 3ebe01b8-76ee-485f-983b-7dbba231f0ea
title: Linked record fields
type: feature
status: backlog
milestone: v0.2
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: format
---

## Problem

VLOOKUP chains are how Excel users fake relationships.

## Proposal

A column type that links to rows of another table, stored as a foreign key (one-to-many first; many-to-many through a join table). Deleting a linked row is blocked or clears the link, chosen per field.

## Acceptance criteria

- [ ] `sqlite3` shows a real foreign key column
- [ ] Deleting a referenced row behaves as configured, covered by tests for both settings
