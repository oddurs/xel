---
id: 11
uid: 01a22288-f68e-4a78-a81a-10a99100ec4d
title: Prove NSDocument can own a SQLite file safely
type: spike
status: doing
milestone: v0.1
assignee: Oddur Sigurdsson
claimed: 2026-11-01
created: 2026-11-01
updated: 2026-11-01
priority: p0
effort: s
area: app
---

## Question

How does a document-based SwiftUI app keep autosave, Versions, Revert, and iCloud Drive working when the document is a live SQLite database rather than a blob written on save?

## Why it matters

NSDocument assumes it writes whole files atomically. A SQLite file written in place can be corrupted or silently forked by the versions machinery.

## Timebox

Three days. Answer with the chosen pattern (for example, working copy plus `VACUUM INTO` on save, or a file package) and a kill-during-save test result.

## Answer

**Pattern: edit a private working copy and save complete snapshots.** The user's file is never open while they edit, so autosave, Versions, Revert and iCloud only ever see whole files.

The engine side is `Workbook::open_copy` and `Workbook::save_copy` in `crates/engine/src/workbook.rs`:

- **Open.** NSDocument's `read(from:ofType:)` calls `open_copy(url, working)`.
  - It opens the original as a workbook, so a crash journal beside it is rolled back first. A byte copy would carry the half-written file.
  - It writes the working copy with `VACUUM INTO` and closes the original.
  - A write-protected original (a locked document, or a Versions browser file) is only read.
- **Save.** `write(to:ofType:)` calls `save_copy(url)`.
  - NSDocument's `writeSafely` hands it a fresh temporary URL and atomically replaces the document afterwards, which is what Versions and iCloud expect.
  - `save_copy` writes with `VACUUM INTO`, drops the session's undo trash, and vacuums again when there was any, so deleted data is not left in free pages. It then fsyncs (F_FULLFSYNC on macOS); VACUUM INTO does not.
  - It refuses to write over an existing file.
- **Revert, and external changes** (iCloud, another app) go through `read(from:)` again, which yields a new working copy. Undo history ends there, as it does in every NSDocument app.
- **Use AppKit `NSDocument`, not SwiftUI `DocumentGroup`.** `ReferenceFileDocument` saves through a `FileWrapper`, which would read the whole database into memory on every save. SwiftUI views live inside the NSDocument's window.
- **Not a file package.** A package keeps the database as a live file inside the document, so iCloud and Versions could still copy it mid-write, and plain `sqlite3 file.xel` would stop working.

**Kill-during-save test:** `a_kill_during_save_never_damages_the_document`.
- **Setup.** A child process edits a working copy and saves over the document as NSDocument does (save to a temporary file, then rename). The parent kills it with SIGKILL 20 times, spread across the save cycle.
- **Result.** 18 of the 20 kills landed inside a save, and 73 saves completed. After every kill the document opened, passed `PRAGMA integrity_check`, held no trash, and never lost a completed save.
- **Control.** The same test fails at once if the child saves in place (`NotFound`). Without the second vacuum, `saved_copies_leave_out_undo_trash_and_the_data_it_held` finds the deleted value in the saved file.

**Costs and limits.**
- **Opening and saving are linear in file size.** Each copies the whole database: a few milliseconds for a few MB. A 1 GB workbook will need a background save, and progress on open.
- **A crash loses edits since the last autosave.** The working copy is not yet kept for recovery; filed as 0126.
- **SIGKILL is not power loss.** Durability on power loss rests on the fsync before rename, which this test cannot exercise.

## 2026-11-01

Kill test numbers vary per run (timing); the invariant does not. Mutation checks: saving in place fails the kill test with NotFound; skipping the second VACUUM fails the free-page test.
