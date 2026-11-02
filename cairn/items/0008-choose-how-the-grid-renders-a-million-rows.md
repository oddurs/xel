---
id: 8
uid: ff4ba628-a0cf-4c09-8650-a8b57c47081f
title: Choose how the grid renders a million rows
type: spike
status: done
milestone: v0.1
assignee: Oddur Sigurdsson
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
priority: p0
effort: s
area: grid
---

## Question

Can NSTableView (view-based or cell-based) scroll a 1,000,000 × 50 table at 120 Hz with custom cell drawing, or does xel need a custom layer-tiled NSView?

## Why it matters

The grid is the product. A wrong choice shows up as jank that cannot be tuned away.

## Timebox

Three days. Prototype both against a synthetic data source; record frame times with Instruments on a ProMotion display in the Answer.

## Answer

**A custom NSView, not NSTableView.**
- One flipped, opaque document view the size of the whole table sits inside an NSScrollView.
- It draws only its dirty rect and draws text with Core Text.
- It is the only design that held ProMotion's 120 Hz at full screen while dragging the scroller through a million rows.

**How it was measured.**
- Harness: `spikes/grid-render` (`swift run -c release GridSpike <renderer> <steady|jump>`).
- Table: synthetic, 1,000,000 × 50, rows 22 pt, columns 100 pt.
- Timing: a display link timestamps every frame while the harness scrolls programmatically, for 4 s after a 0.5 s settle. A frame counts as dropped when it takes more than 1.5 × 8.33 ms.
- Motion: "steady" scrolls 50 pt a frame, a fast flick. "jump" moves about 1,000 rows a frame, like dragging the scroller.
- Memory: `phys_footprint`.
- Machine: M4 Pro MacBook Pro, built-in Liquid Retina XDR (ProMotion), macOS 26.5.2.
- Instruments was not used: `xctrace` needs full Xcode, and this machine has the Command Line Tools only.

Dropped frames by renderer and window size:

| Renderer | 1400 × 900 steady | 1400 × 900 jump | 1728 × 1080 (full screen) jump |
|---|---|---|---|
| View-based NSTableView | 44–49 % | 100 % (60–90 ms frames) | 100 % (~80 ms frames) |
| Cell-based NSTableView | 0.2 % | 0.2 % | not run |
| Custom view, NSString drawing | 0.4–0.8 % | 0–0.2 % | 8.6–13.5 % |
| Custom view, Core Text drawing | 0.4 % | 0.4 % | 1.1–1.3 % |
| Custom view, `drawsAsynchronously` | not run | not run | 7.2 % |
| Window-sized canvas over an empty extent view | 0–0.2 % | 0–0.6 % | not run |
| Empty document view (baseline) | 0.4 % | 0.6 % | 0.4 % |

- **View-based NSTableView is out.** It builds a view per visible cell, and that cannot keep up with 50 columns.
- **Cell-based NSTableView scrolls smoothly,** but NSCell is the legacy path. It would fight custom drawing: error badges, selection, formula ranges, frozen panes.
- **Core Text is what makes full screen work.** NSString's attribute machinery costs too much when a jump redraws about 830 new cells every frame. `drawsAsynchronously` does not help.
- **The window-sized canvas brings no gain** and costs about 100 MB more in IOSurfaces.
- **Limiting AppKit's overdraw** (`prepareContent(in:)` limited to the visible rect) changed nothing.

**Memory.**
- An empty scrolling window already costs 160–176 MB of footprint on this display, nearly all of it graphics surfaces.
- The Core Text grid adds 25–35 MB on top: 183–212 MB in total.
- Memory does not grow with the rows scrolled.
- 0033's "under 200 MB" is a whole-process figure, so it is close to unreachable full screen whatever the renderer. 0033 now carries a note proposing "the grid adds at most 60 MB over an empty window of the same size".

**For 0033.**
- **Starting point:** the custom view with Core Text.
- **Engine reads:** fetch rows a window at a time from the engine, with prefetch either side.
- **Caching:** cache each visible cell's formatted string, or its `CTLine`, so steady scrolling only draws rows that are new.
- **Headers:** draw the column header and the row numbers as separate views, kept in step with the clip view.
- **Not measured here:** engine fetches, number formatting, variable column widths and Instruments traces. 0033 measures those with the real grid.

## 2026-11-01

Raw runs: frame intervals and footprints per renderer/size are in the Answer table; harness in spikes/grid-render.

## Result

Custom flipped opaque NSView inside NSScrollView, drawing its dirty rect with Core Text: at the noise floor at full screen with scroller-drag jumps (1.1-1.3% vs 0.4% empty baseline); view-based NSTableView drops 44-100%. Empty window costs 160-176 MB; grid adds 25-35 MB.
