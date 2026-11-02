---
id: 11
uid: 01a22288-f68e-4a78-a81a-10a99100ec4d
title: Prove NSDocument can own a SQLite file safely
type: spike
status: done
milestone: v0.1
assignee: Oddur Sigurdsson
created: 2026-11-01
updated: 2026-11-01
closed_at: 2026-11-01
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
  - The original is only read, on a read-only connection, so a WAL left by another tool is not checkpointed into the user's file outside a save.
  - The exception is a crash journal: it is rolled back first, which needs a writer. A write-protected original with a crash journal is refused as `ReadOnly`.
  - It writes the working copy with `VACUUM INTO`.
- **Save.** `write(to:ofType:)` calls `save_copy(url)`.
  - NSDocument's `writeSafely` hands it a fresh temporary URL and atomically replaces the document afterwards, which is what Versions and iCloud expect.
  - `save_copy` writes with `VACUUM INTO`, then drops the session's undo trash with `secure_delete` on, so deleted data is zeroed rather than left in free pages. It compacts again only when the trash was over a quarter of the file.
  - It syncs with F_FULLFSYNC before returning. SQLite's own fsync after VACUUM INTO leaves data in the drive's cache on macOS.
  - It refuses to write over an existing file, or beside an existing journal, which SQLite would take for its own and delete.
  - It fails while a statement is mid-read on the same connection, since VACUUM INTO refuses that; the bridge (0028) must not save in the middle of a read.
- **Revert, and external changes** (iCloud, another app) go through `read(from:)` again, which yields a new working copy. Undo history ends there, as it does in every NSDocument app.
- **Use AppKit `NSDocument`, not SwiftUI `DocumentGroup`.** `ReferenceFileDocument` reads from and writes to a `FileWrapper`. A save gets no destination URL to `VACUUM INTO`, and a read gets no location for the original, so a crash journal beside it cannot be rolled back. SwiftUI views live inside the NSDocument's window.
- **Not a file package.** A package keeps the database as a live file inside the document, so iCloud and Versions could still copy it mid-write, and plain `sqlite3 file.xel` would stop working.

**Kill-during-save test:** `a_kill_during_save_never_damages_the_document`.
- **Setup.** A child process edits a working copy and saves over the document as NSDocument does (save to a temporary file, then rename). The parent kills it with SIGKILL 20 times, spread across the save cycle.
- **Result.** In the last run, 20 of the 20 kills landed inside a save and 48 saves completed; the counts vary with timing, the invariant does not. After every kill the document opened, passed `PRAGMA integrity_check`, held no trash, and never lost a completed save.
- **Control.** The same test fails at once if the child saves in place (`NotFound`). With `secure_delete` off, `saved_copies_leave_out_undo_trash_and_the_data_it_held` finds the deleted value in the saved file. Copying through a read-write open of the original fails `open_copy_never_writes_to_the_original`: a crashed tool's WAL is checkpointed into the user's file.

**Costs and limits.**
- **Opening and saving are linear in file size.** Each copies the whole database (twice, right after a large delete): a few milliseconds for a few MB. A 1 GB workbook will need a background save, and progress on open.
- **A crash loses edits since the last autosave.** The working copy is not yet kept for recovery; filed as 0126.
- **What the kill test proves is narrow.** It shows a kill cannot damage the document when saves go to a temporary file and are renamed into place, and that saving in place can. It cannot show durability on power loss, which rests on the F_FULLFSYNC and on NSDocument's replace; removing the sync would still pass it.
- **Opening a writable original that has a crash journal rolls the journal back in place**, a write during `read(from:)`. It only restores the last committed state.

## 2026-11-01

Kill test numbers vary per run (timing); the invariant does not. Mutation checks: saving in place fails the kill test with NotFound; skipping the second VACUUM fails the free-page test.

## Result

Edit a private working copy (Workbook::open_copy) and save complete VACUUM INTO snapshots (Workbook::save_copy) that NSDocument's writeSafely moves into place; AppKit NSDocument, not DocumentGroup. Kill-during-save test: the document survives every kill; saving in place does not.
