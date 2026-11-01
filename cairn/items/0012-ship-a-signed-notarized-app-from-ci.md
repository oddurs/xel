---
id: 12
uid: f561e48e-b481-4a45-b2d6-2fc22cf07f07
title: Ship a signed, notarized .app from CI
type: spike
status: blocked
milestone: v0.1
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: s
area: infra
---

## Question

What does it take to build a universal xel.app from SwiftPM in GitHub Actions, sign it with a Developer ID, notarize, and staple it, without an Xcode project checked in?

## Why it matters

v0.1 is not an MVP unless a stranger can install it. Gatekeeper blocks unsigned apps.

## Timebox

Two days. Requires an Apple Developer account. Answer with the workflow steps and which secrets the repository needs.

## Answer

Write the answer here before closing. A spike that closes without one was wasted.

## 2026-11-01

Split on 2026-11-01: the unsigned bundle moved to 0118. This item is now only Developer ID signing and notarization, blocked on an Apple Developer account and a Developer ID Application certificate; neither exists on the build machine (security find-identity shows only a development identity).
