---
id: 33
uid: 2aa51a9f-65eb-431c-8d44-5c6b87e49daf
title: Virtualized grid view
type: feature
status: blocked
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

## 2026-11-01

Measured with spikes/grid-render's xel mode (display link, in-process; no Instruments, which needs full Xcode): 1,000,000 rows x 20 columns, 0 % dropped steady and while dragging the scroller at 1400x900 and full screen after review fixes; footprint 187-219 MB vs 153-178 MB for an empty window of the same size. Both criteria are left unticked pending the owner: an Instruments trace (needs Xcode), and the 200 MB figure (see the earlier note proposing 'grid adds at most 60 MB over an empty window').
