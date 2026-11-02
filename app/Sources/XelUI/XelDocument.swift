import AppKit
import OSLog
import SwiftUI
import XelEngine
import XelKit

/// A .xel document. As 0011 answers, the user's file is never edited in place: reading
/// opens a private working copy in a temporary directory of the document's own, and
/// saving writes a complete copy to the URL NSDocument provides, which it then moves into
/// place. Autosave, Versions, Revert, and iCloud only ever see whole files.
@objc(XelDocument)
public final class XelDocument: NSDocument {
    private nonisolated static let log = Logger(
        subsystem: "io.github.oddurs.xel", category: "document")

    /// What the window shows.
    let model = DocumentModel()

    private let operations = SerialOperations()
    private var workbook: Workbook?
    private var workingDirectory: URL?
    private var watcher: Task<Void, Never>?
    /// Counts workbooks opened, so a load from a replaced one is never shown.
    private var generation = 0

    override public class var autosavesInPlace: Bool { true }

    /// A new, untitled document.
    public convenience init(type typeName: String) throws {
        self.init()
        fileType = typeName
        try start { working in try await Workbook.create(at: working) }
    }

    // MARK: Reading and writing

    // AppKit reads and writes on the main thread unless a document opts into concurrent
    // reading or asynchronous writing, which this one does not.

    /// Also how Revert, and a reload after an outside change, replace the contents: the new
    /// working copy is opened before the old one is let go.
    override public func read(from url: URL, ofType typeName: String) throws {
        try MainActor.assumeIsolated {
            try start { working in try await Workbook.openCopy(of: url, workingCopy: working) }
        }
    }

    override public func write(to url: URL, ofType typeName: String) throws {
        try MainActor.assumeIsolated {
            guard let workbook else { throw CocoaError(.fileWriteUnknown) }
            try operations.runBlocking { try await workbook.save(to: url) }
        }
    }

    /// Opens a workbook on a working copy in a new temporary directory, replacing the
    /// current one.
    private func start(_ open: @escaping @Sendable (URL) async throws -> Workbook) throws {
        let directory = FileManager.default.temporaryDirectory
            .appending(path: "xel-\(UUID().uuidString)", directoryHint: .isDirectory)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: false)
        let opened: Workbook
        do {
            opened = try operations.runBlocking {
                try await open(directory.appending(path: "working.xel"))
            }
        } catch {
            // The opening error is the one to report; the directory is empty or holds a
            // partial copy that `open` already removed.
            try? FileManager.default.removeItem(at: directory)
            throw error
        }
        let previous = (workbook, workingDirectory)
        workbook = opened
        workingDirectory = directory
        watch(opened)
        retire(previous.0, directory: previous.1)
        undoManager?.removeAllActions()
    }

    /// Closes a replaced workbook after anything still queued for it, and removes its
    /// working copy.
    private func retire(_ workbook: Workbook?, directory: URL?) {
        guard let workbook, let directory else { return }
        operations.submit { await Self.discard(workbook, directory: directory) }
    }

    /// Closes a workbook nobody uses any more and removes its working copy. Nothing can be
    /// done about a failure here but report it: the user's file is not involved. Not on the
    /// main actor: `close` blocks the main thread while this runs.
    private nonisolated static func discard(_ workbook: Workbook, directory: URL) async {
        do {
            try await workbook.close()
        } catch {
            log.error("closing a working copy failed: \(error, privacy: .public)")
        }
        do {
            try FileManager.default.removeItem(at: directory)
        } catch {
            log.error("removing a working copy failed: \(error, privacy: .public)")
        }
    }

    /// Keeps the model in step with the workbook.
    private func watch(_ workbook: Workbook) {
        watcher?.cancel()
        generation += 1
        let current = generation
        watcher = Task { [weak self] in
            // Subscribed before the first load, so no change falls between the two.
            let changes = await workbook.changes()
            await self?.load(from: workbook, generation: current)
            for await change in changes {
                // Row edits leave the list of tables and columns as it was.
                if case .rows = change { continue }
                await self?.load(from: workbook, generation: current)
            }
        }
    }

    /// Reloads the model from `workbook`, in turn with the document's edits, unless the
    /// document has moved on to another workbook meanwhile.
    private nonisolated func load(from workbook: Workbook, generation: Int) async {
        do {
            let tables = try await operations.run { try await workbook.tables() }
            await MainActor.run {
                if self.generation == generation { self.model.set(tables) }
            }
        } catch is CancellationError {
            // The document closed.
        } catch {
            await MainActor.run { report(error) }
        }
    }

    override public func close() {
        super.close()
        watcher?.cancel()
        let workbook = workbook
        let directory = workingDirectory
        self.workbook = nil
        workingDirectory = nil
        // Blocks so the working copy is gone before the app can quit. Nothing queued waits
        // for the main thread (see `edit`), so this cannot deadlock.
        if let workbook, let directory {
            do {
                try operations.runBlocking { await Self.discard(workbook, directory: directory) }
            } catch {
                Self.log.error("closing a document twice: \(error, privacy: .public)")
            }
        }
        operations.finish()
    }

    // MARK: Outside changes

    override public func presentedItemDidChange() {
        super.presentedItemDidChange()
        // NSDocument does not reload by itself: without this, the window kept showing the
        // old contents after the file was replaced.
        Task { @MainActor in self.reloadIfChangedOutside() }
    }

    /// Reloads the file when something else replaced it, such as iCloud or another app,
    /// and there are no unsaved changes to lose. With unsaved changes NSDocument's own
    /// check refuses to save over the newer file and asks the user. Any change of date
    /// counts: a synced or restored file can carry an older one.
    func reloadIfChangedOutside() {
        guard let url = fileURL, let type = fileType, !isDocumentEdited,
            let known = fileModificationDate,
            let now = try? url.resourceValues(forKeys: [.contentModificationDateKey])
                .contentModificationDate,
            now != known
        else { return }
        do {
            try revert(toContentsOf: url, ofType: type)
        } catch {
            presentError(error)
        }
    }

    // MARK: Editing

    /// Runs an edit on the workbook in order and makes it undoable. The engine keeps the
    /// history; the undo manager only mirrors it, so the Edit menu and the document's
    /// edited state follow.
    ///
    /// The undo is registered now, not when the edit finishes: the document must count as
    /// edited before a quick close or quit, and nothing queued may wait for the main
    /// thread, which `read`, `write`, and `close` block while they wait for the queue.
    func edit(_ name: String, _ change: @escaping @Sendable (Workbook) async throws -> Void) {
        guard let workbook else { return }
        undoManager?.registerUndo(withTarget: self) { $0.step(undo: true, name) }
        undoManager?.setActionName(name)
        operations.submit { [weak self] in
            do {
                try await change(workbook)
            } catch {
                self?.failed(error)
            }
        }
    }

    /// An edit, undo, or redo the engine refused, or could not replay. The mirrored
    /// history no longer matches the engine's, so it is cleared. Reported without waiting,
    /// from any thread.
    private nonisolated func failed(_ error: (any Error)?) {
        DispatchQueue.main.async {
            MainActor.assumeIsolated {
                self.undoManager?.removeAllActions()
                if let error { self.report(error) }
            }
        }
    }

    private func report(_ error: any Error) {
        if let window = windowControllers.first?.window {
            presentError(error, modalFor: window, delegate: nil, didPresent: nil, contextInfo: nil)
        } else {
            presentError(error)
        }
    }

    /// One engine undo or redo, registering its opposite as the undo manager expects.
    private func step(undo: Bool, _ name: String) {
        undoManager?.registerUndo(withTarget: self) { $0.step(undo: !undo, name) }
        undoManager?.setActionName(name)
        guard let workbook else { return }
        operations.submit { [weak self] in
            do {
                let moved = try await undo ? workbook.undo() : workbook.redo()
                // The engine had nothing to replay: the two histories have parted.
                if !moved { self?.failed(nil) }
            } catch {
                self?.failed(error)
            }
        }
    }

    func addTable() {
        edit("New Table") { workbook in
            // Named from the workbook as it is when this runs: the model may lag behind
            // edits still queued, and two quick clicks must not pick the same name.
            let names = Set(try await workbook.tables().map(\.name))
            let name = (1...).lazy.map { "Table \($0)" }.first { !names.contains($0) } ?? "Table"
            _ = try await workbook.createTable(
                named: name,
                columns: [NewColumn(name: "Name", kind: .text)]
            )
        }
    }

    // MARK: Windows

    override public func makeWindowControllers() {
        let view = DocumentView(model: model, addTable: { [weak self] in self?.addTable() })
        let hosting = NSHostingController(rootView: view)
        // The view sets the smallest size; the user, not the content, sets the rest.
        hosting.sizingOptions = [.minSize]
        let window = NSWindow(contentViewController: hosting)
        window.setContentSize(NSSize(width: 720, height: 480))
        window.toolbarStyle = .unified
        let controller = NSWindowController(window: window)
        controller.shouldCascadeWindows = true
        addWindowController(controller)
    }

    // MARK: For tests

    var workingCopyDirectory: URL? { workingDirectory }

    /// Waits until everything submitted so far has reached the engine, and the model has
    /// caught up with it through the change stream.
    func settle() async throws {
        guard let workbook else { return }
        let tables = try await operations.run { try await workbook.tables() }
        let deadline = ContinuousClock.now + .seconds(5)
        while model.tables != tables {
            guard ContinuousClock.now < deadline else { throw CancellationError() }
            try await Task.sleep(for: .milliseconds(5))
        }
    }
}
