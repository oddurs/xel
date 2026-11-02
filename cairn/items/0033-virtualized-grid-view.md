---
id: 33
uid: 2aa51a9f-65eb-431c-8d44-5c6b87e49daf
title: Virtualized grid view
type: feature
status: doing
milestone: v0.1
assignee: Oddur Sigurdsson
claimed: 2026-11-01
depends_on:
- 8
- 28
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: l
area: grid
---

## Problem

Tables need to be seen.

## Proposal

Implement the grid with the approach chosen in the grid spike: column headers, row numbers, cell drawing per type (checkbox, select pill, right-aligned tabular numbers), Dark Mode and Increase Contrast support. Rows load through the engine window API.

## Acceptance criteria

- [ ] A 1,000,000-row table scrolls without dropped frames on a ProMotion Mac, measured with Instruments and recorded in the PR
- [ ] Memory stays under 200 MB while scrolling the full table

## 2026-11-01

From spike 0008: an empty full-screen scrolling window already costs 160-176 MB phys_footprint on a Retina XDR display (graphics surfaces); the Core Text grid adds 25-35 MB. The 'under 200 MB' criterion is a whole-process figure that is near-unreachable full screen regardless of renderer. Proposal: 'the grid adds at most 60 MB over an empty window of the same size, measured with spikes/grid-render's harness'. Needs the owner's decision.
