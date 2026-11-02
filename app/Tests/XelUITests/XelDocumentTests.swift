import AppKit
import Foundation
import Testing
import XelEngine
import XelKit

@testable import XelUI

let workbookType = "io.github.oddurs.xel.workbook"

/// A fresh directory for one test, removed when the test ends.
final class Scratch {
    let url: URL

    init() throws {
        url = FileManager.default.temporaryDirectory
            .appending(path: "xel-ui-tests-\(UUID().uuidString)", directoryHint: .isDirectory)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: false)
    }

    func file(_ name: String) -> URL { url.appending(path: name) }

    deinit { try? FileManager.default.removeItem(at: url) }
}

/// A workbook file at `url` holding one empty table named `table`.
func makeFile(_ url: URL, table: String) async throws {
    let book = try await Workbook.create(at: url)
    _ = try await book.createTable(named: table, columns: [NewColumn(name: "x", kind: .text)])
    try await book.close()
}

func exists(_ url: URL) -> Bool {
    FileManager.default.fileExists(atPath: url.path(percentEncoded: false))
}

@MainActor
@Suite struct XelDocumentTests {
    @Test func aNewDocumentEditsAPrivateCopyThatCloseRemoves() async throws {
        let document = try XelDocument(type: workbookType)
        let working = try #require(document.workingCopyDirectory)
        #expect(exists(working))

        // Quickly, before the model has caught up.
        document.addTable()
        document.addTable()
        try await document.settle()
        #expect(document.model.tables.map(\.name) == ["Table 1", "Table 2"])

        document.close()
        #expect(!exists(working))
    }

    @Test func savingReplacesTheFileWholeAndOnlyWhenAsked() async throws {
        let scratch = try Scratch()
        let url = scratch.file("people.xel")
        try await makeFile(url, table: "People")
        let before = try Data(contentsOf: url)

        let document = try XelDocument(contentsOf: url, ofType: workbookType)
        try await document.settle()
        #expect(document.model.tables.map(\.name) == ["People"])
        document.addTable()
        try await document.settle()
        #expect(try Data(contentsOf: url) == before, "edits stay in the working copy")

        // NSDocument's own save: write a temporary file, then move it over the original.
        try document.writeSafely(to: url, ofType: workbookType, for: .saveOperation)
        let saved = try await Workbook.open(url)
        #expect(try await saved.tables().map(\.name) == ["People", "Table 1"])
        try await saved.close()
        document.close()
    }

    @Test func undoAndRevertFollowTheEngine() async throws {
        let scratch = try Scratch()
        let url = scratch.file("people.xel")
        try await makeFile(url, table: "People")
        let document = try XelDocument(contentsOf: url, ofType: workbookType)
        let undo = try #require(document.undoManager)

        document.addTable()
        try await document.settle()
        #expect(undo.canUndo)
        #expect(undo.undoActionName == "New Table")
        undo.undo()
        try await document.settle()
        #expect(document.model.tables.map(\.name) == ["People"])
        undo.redo()
        try await document.settle()
        #expect(document.model.tables.map(\.name) == ["People", "Table 1"])

        let working = try #require(document.workingCopyDirectory)
        try document.revert(toContentsOf: url, ofType: workbookType)
        try await document.settle()
        #expect(document.model.tables.map(\.name) == ["People"])
        #expect(!undo.canUndo, "a reverted document starts a new history")
        #expect(document.workingCopyDirectory != working)
        document.close()
        #expect(!exists(working), "the replaced working copy is removed")
    }

    @Test func aFileReplacedFromOutsideReloadsUnlessThereAreUnsavedChanges() async throws {
        let scratch = try Scratch()
        let url = scratch.file("people.xel")
        try await makeFile(url, table: "People")
        let document = try XelDocument(contentsOf: url, ofType: workbookType)

        // As iCloud or another app would: a whole new file in its place.
        try await makeFile(scratch.file("synced.xel"), table: "Synced")
        _ = try FileManager.default.replaceItemAt(url, withItemAt: scratch.file("synced.xel"))
        document.reloadIfChangedOutside()
        try await document.settle()
        #expect(document.model.tables.map(\.name) == ["Synced"])

        document.addTable()
        try await document.settle()
        // What NSDocument records for an edit made in the app.
        document.updateChangeCount(.changeDone)
        try await makeFile(scratch.file("again.xel"), table: "Again")
        _ = try FileManager.default.replaceItemAt(url, withItemAt: scratch.file("again.xel"))
        document.reloadIfChangedOutside()
        try await document.settle()
        #expect(
            document.model.tables.map(\.name) == ["Synced", "Table 1"],
            "unsaved changes are not thrown away"
        )
        document.close()
    }
}
