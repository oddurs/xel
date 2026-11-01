---
id: 120
uid: 0a18b4ba-72d9-41ed-ba7c-8c73b0e17747
title: Parse array constants, @, spill references, special table items, and newer error values
type: feature
status: backlog
milestone: v0.3
depends_on:
- 19
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: s
area: formula
---

## Problem

The formula parser (0019) rejects syntax that real workbooks contain: array constants (`{1,2;3,4}`), the implicit-intersection prefix (`@A1:A10`), spill references (`A1#`), special table items as xlsx stores them (`Orders[[#This Row],[Amount]]`, `[#Totals]`, `[#All]`), and the newer error values (`#SPILL!`, `#CALC!`, `#FIELD!`, `#BLOCKED!`, `#CONNECT!`, `#UNKNOWN!`, `#BUSY!`, `#GETTING_DATA`). xlsx import and dynamic arrays both need them.

## Proposal

Extend the AST and parser for each form and the `ErrorKind` enum for each error value. Move the matching entries from the parser's unsupported list into its corpus.

## Acceptance criteria

- [ ] Each form above parses to a distinct AST node, covered by tests
- [ ] `Orders[[#This Row],[Amount]]` and `[@Amount]` parse to the same reference
- [ ] The parser's unsupported list contains only the intersection-space case
