import Dispatch
import Foundation
import XelEngine

/// What an edit changed, so a view reloads only what it shows.
public enum WorkbookChange: Sendable, Equatable {
    /// Tables or columns were added, removed, renamed, or redefined.
    case schema
    /// Rows or cells of one table changed, and with them its formula columns.
    case rows(table: Int64)
    /// An undo or redo: anything may have changed.
    case everything
}

/// An open workbook. Every engine call runs on the workbook's own serial queue, never on
/// the main thread, as SQLite reads and writes block.
public actor Workbook {
    private let book: Book
    private let queue: DispatchSerialQueue
    private var observers: [UUID: AsyncStream<WorkbookChange>.Continuation] = [:]

    private static let queueKey = DispatchSpecificKey<Void>()

    public nonisolated var unownedExecutor: UnownedSerialExecutor {
        queue.asUnownedSerialExecutor()
    }

    private init(book: Book, queue: DispatchSerialQueue) {
        self.book = book
        self.queue = queue
    }

    /// Opens or creates the engine's book on a new queue, which the workbook then runs on.
    private static func make(_ open: @escaping @Sendable () throws -> Book) async throws
        -> Workbook
    {
        let queue = DispatchSerialQueue(label: "io.github.oddurs.xel.workbook")
        queue.setSpecific(key: queueKey, value: ())
        let book = try await withCheckedThrowingContinuation { continuation in
            queue.async { continuation.resume(with: Result { try open() }) }
        }
        return Workbook(book: book, queue: queue)
    }

    /// Creates a new workbook file at `url`, which must not exist yet.
    public static func create(at url: URL) async throws -> Workbook {
        let path = url.path(percentEncoded: false)
        return try await make { try Book.create(path: path) }
    }

    /// Opens the workbook file at `url` and edits it in place.
    public static func open(_ url: URL) async throws -> Workbook {
        let path = url.path(percentEncoded: false)
        return try await make { try Book.open(path: path) }
    }

    /// Opens a private working copy of the document at `url`, written to `workingCopy`,
    /// which must not exist yet. The document is untouched until `save(to:)`.
    public static func openCopy(of url: URL, workingCopy: URL) async throws -> Workbook {
        let src = url.path(percentEncoded: false)
        let working = workingCopy.path(percentEncoded: false)
        return try await make { try Book.openCopy(src: src, working: working) }
    }

    /// Writes a complete copy of the workbook to `url`, which must not exist yet.
    public func save(to url: URL) throws {
        try book.saveCopy(dest: url.path(percentEncoded: false))
    }

    /// Closes the file and ends every change stream. Later calls throw `XelError.Closed`.
    public func close() throws {
        defer {
            for observer in observers.values { observer.finish() }
            observers.removeAll()
        }
        try book.close()
    }

    public func formatVersion() throws -> UInt32 { try book.formatVersion() }

    // MARK: Changes

    /// Every change made through this workbook from now on, until it closes.
    public func changes() -> AsyncStream<WorkbookChange> {
        let (stream, continuation) = AsyncStream.makeStream(of: WorkbookChange.self)
        let id = UUID()
        observers[id] = continuation
        continuation.onTermination = { [weak self] _ in
            Task { await self?.removeObserver(id) }
        }
        return stream
    }

    private func removeObserver(_ id: UUID) {
        observers[id] = nil
    }

    private func publish(_ change: WorkbookChange) {
        for observer in observers.values { observer.yield(change) }
    }

    // MARK: Tables and columns

    public func tables() throws -> [TableInfo] { try book.tables() }

    public func table(_ id: Int64) throws -> TableInfo { try book.table(id: id) }

    public func createTable(named name: String, columns: [NewColumn]) throws -> Int64 {
        let id = try book.createTable(name: name, columns: columns)
        publish(.schema)
        return id
    }

    public func renameTable(_ table: Int64, to name: String) throws {
        try book.renameTable(table: table, name: name)
        publish(.schema)
    }

    public func deleteTable(_ table: Int64) throws {
        try book.deleteTable(table: table)
        publish(.schema)
    }

    public func addColumn(to table: Int64, named name: String, kind: ColumnKind) throws -> Int64 {
        let id = try book.addColumn(table: table, name: name, kind: kind)
        publish(.schema)
        return id
    }

    public func addFormulaColumn(to table: Int64, named name: String, formula: String) throws
        -> Int64
    {
        let id = try book.addFormulaColumn(table: table, name: name, formula: formula)
        publish(.schema)
        return id
    }

    public func setFormula(of column: Int64, to formula: String) throws {
        try book.setFormula(column: column, formula: formula)
        publish(.schema)
    }

    public func renameColumn(_ column: Int64, to name: String) throws {
        try book.renameColumn(column: column, name: name)
        publish(.schema)
    }

    public func deleteColumn(_ column: Int64) throws {
        try book.deleteColumn(column: column)
        publish(.schema)
    }

    public func setSelectOptions(of column: Int64, to options: [String]) throws {
        try book.setSelectOptions(column: column, options: options)
        publish(.schema)
    }

    // MARK: Rows

    public func rowCount(of table: Int64) throws -> UInt64 { try book.rowCount(table: table) }

    /// Up to `limit` rows of `table`, starting at the `offset`th in display order.
    public func rows(of table: Int64, offset: UInt64, limit: UInt64) throws -> [RowData] {
        try book.rows(table: table, offset: offset, limit: limit)
    }

    /// Adds a row at the end of `table`; columns not given are empty. Returns its id.
    public func insertRow(into table: Int64, values: [ColumnValue]) throws -> Int64 {
        let id = try book.insertRow(table: table, values: values)
        publish(.rows(table: table))
        return id
    }

    /// Sets cells of `table`, all or none, as one undo step.
    public func setCells(in table: Int64, _ edits: [CellEdit]) throws {
        try book.setCells(table: table, edits: edits)
        publish(.rows(table: table))
    }

    public func deleteRows(_ rows: [Int64], from table: Int64) throws {
        try book.deleteRows(table: table, rows: rows)
        publish(.rows(table: table))
    }

    // MARK: Undo

    /// Undoes the last edit; false when there was nothing to undo.
    @discardableResult
    public func undo() throws -> Bool {
        let undone = try book.undo()
        if undone { publish(.everything) }
        return undone
    }

    /// Redoes the last undone edit; false when there was nothing to redo.
    @discardableResult
    public func redo() throws -> Bool {
        let redone = try book.redo()
        if redone { publish(.everything) }
        return redone
    }

    public func canUndo() throws -> Bool { try book.canUndo() }

    public func canRedo() throws -> Bool { try book.canRedo() }

    // MARK: For tests

    /// Whether the caller is running on this workbook's queue.
    var isOnEngineQueue: Bool { DispatchQueue.getSpecific(key: Self.queueKey) != nil }

    var runsOnMainThread: Bool { Thread.isMainThread }

    func checkPanicBoundary() throws { try book.checkPanicBoundary() }
}
