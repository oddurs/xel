---
id: 48
uid: c65f0411-8925-408d-8772-d7185736d844
title: Lookup and rollup fields
type: feature
status: backlog
milestone: v0.2
depends_on:
- 27
- 46
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: recalc
---

## Problem

Once tables link, users need values from the other side.

## Proposal

Lookup fields show a field of the linked record; rollup fields aggregate a field across linked records (SUM, COUNT, AVERAGE, MIN, MAX, CONCAT). Both recalc incrementally.

## Acceptance criteria

- [ ] Changing a customer's name updates every order's lookup
- [ ] A rollup over 10,000 linked rows recalculates in under 20 ms
