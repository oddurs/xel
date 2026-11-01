---
id: 4
uid: b3d2f106-adc9-41a2-9928-d31ca42d03c7
key: v0.4
title: Python, charts, and provenance
type: milestone
status: backlog
created: 2026-11-01
updated: 2026-11-01
priority: p2
due: 2027-11-15
---

## Ships

xel becomes a development environment with charts: Python blocks run inside workbooks behind a trust prompt, charts update live from tables, queries, and pivots, the `xel` Python package scripts workbooks from outside, and every value can be traced to its sources and compared across versions.

## Done when

- [ ] `pip install xel` works on macOS and reads a workbook created in the app
- [ ] A Python block that transforms a table runs only after the workbook is trusted
- [ ] Every p0 item in this milestone is done and the v0.4.0 release is published

## Explicitly not in this milestone

- An MCP server (later)
- Python on platforms other than macOS for blocks (the package itself may build elsewhere)
