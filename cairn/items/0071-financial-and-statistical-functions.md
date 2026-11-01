---
id: 71
uid: a4bbf52b-0a01-4291-a219-7b956d3f8a19
title: Financial and statistical functions
type: feature
status: backlog
milestone: v0.3
depends_on:
- 24
created: 2026-11-01
updated: 2026-11-01
priority: p1
effort: m
area: formula
---

## Problem

Financial models and analyses imported from Excel rely on these.

## Proposal

Implement PMT, PV, FV, NPV, XNPV, IRR, XIRR, RATE, MEDIAN, MODE, STDEV.S, STDEV.P, VAR.S, VAR.P, PERCENTILE, QUARTILE, RANK, LARGE, SMALL, SUMPRODUCT, SUMIFS, COUNTIFS, AVERAGEIFS.

## Acceptance criteria

- [ ] IRR and XIRR converge to Excel's results on the conformance vectors, including no-solution cases
