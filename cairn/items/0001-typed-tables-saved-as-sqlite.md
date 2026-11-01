---
id: 1
uid: e76e4eee-7d3b-41cd-b027-e22bb1af6e01
key: v0.1
title: Typed tables, saved as SQLite
type: milestone
status: backlog
created: 2026-11-01
updated: 2026-11-01
priority: p2
due: 2027-01-31
---

## Ships

A Mac user downloads xel, builds a workbook of typed tables (typed in or imported from CSV), computes with Excel-style column formulas and total rows, and saves a .xel file that `sqlite3` reads directly.

## Done when

- [ ] A notarized build is attached to the v0.1.0 GitHub release and opens on a clean Mac
- [ ] The README quickstart works end to end for someone who has not seen xel
- [ ] `sqlite3 file.xel 'select * from <table>'` returns the rows and computed values shown in the app
- [ ] Every p0 item in this milestone is done and `scripts/task check` is green on main

## Explicitly not in this milestone

- Linked records, SQL query blocks, free-form A1 cells, or views (v0.2)
- Opening or saving .xlsx (v0.3)
- Python in any form (v0.4)
