---
id: 78
uid: 6e1472e2-e591-4af6-8e9d-e77fe9012c4f
title: Publish macOS wheels to PyPI
type: chore
status: backlog
milestone: v0.4
depends_on:
- 42
- 77
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: m
area: infra
---

## Problem

`pip install xel` should just work.

## Proposal

Build abi3 wheels for arm64 and x86_64 with maturin in the release workflow and publish with PyPI trusted publishing.

## Acceptance criteria

- [ ] A tagged release publishes wheels that install into a fresh virtualenv
