---
paths:
  - "app/**"
---

# Swift

- The app should feel like it shipped with macOS. Use system controls, SF Symbols,
  standard menus and keyboard shortcuts, and semantic colors. Do not build custom
  controls when a system one exists.
- SwiftUI for windows, inspectors, sidebars, and settings. AppKit only where SwiftUI
  cannot perform, such as the virtualized grid, wrapped in a representable.
- Swift 6 language mode with strict concurrency. Keep UI state on the main actor.
- Formatting and lint come from `swift format` with the repo's `.swift-format`.
- Warnings are errors in `scripts/task build`.
