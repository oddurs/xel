---
id: 7
uid: a29424d3-e8a7-4a7c-bb79-8ce40c1c61a8
title: Choose the Rust-to-Swift bridge
type: spike
status: planned
milestone: v0.1
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: s
area: bridge
---

## Question

UniFFI, swift-bridge, or a hand-written C ABI with cbindgen: which one lets the SwiftUI app call the engine with the least ceremony, build with SwiftPM (Command Line Tools locally, Xcode in CI), and link a universal static library into the app?

## Why it matters

Every app feature calls the engine through this boundary. Changing it later means rewriting every call site.

## Timebox

Three days. Deliver a branch where Swift opens a SQLite file through Rust and reads a value back, built by `scripts/task build`, plus a short comparison in the Answer.

## Answer

Write the answer here before closing. A spike that closes without one was wasted.
