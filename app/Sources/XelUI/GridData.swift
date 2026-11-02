import Foundation
import Synchronization
import XelEngine

/// One cell as the grid draws it: its text, formatted once when its page loads, and how
/// to show it.
public struct GridText: Sendable, Equatable {
    public enum Kind: Sendable, Equatable {
        case text
        case number
        case date
        case checkbox
        case error
    }

    public let text: String
    public let kind: Kind

    init(_ cell: Cell) {
        switch cell {
        case .empty: (text, kind) = ("", .text)
        case .text(let value): (text, kind) = (value, .text)
        case .number(let value): (text, kind) = (Self.general(value), .number)
        case .checkbox(let value): (text, kind) = (value ? "✓" : "", .checkbox)
        case .date(let year, let month, let day):
            (text, kind) = (String(format: "%04d-%02d-%02d", year, month, day), .date)
        case .error(let code): (text, kind) = (code, .error)
        }
    }

    /// Excel's General format, near enough until display formats arrive (0017): whole
    /// numbers in full, others to ten significant digits without trailing zeros.
    static func general(_ value: Double) -> String {
        if value == value.rounded(), abs(value) < 1e15 {
            return String(format: "%.0f", value)
        }
        return String(format: "%.10g", value)
    }
}

/// The rows of one table, loaded a page at a time as the grid shows them and kept to a
/// bounded window around what is visible.
@MainActor
public final class GridData {
    /// Reads `limit` rows from `offset`, unless `stillWanted` says the grid has moved on by
    /// the time the read's turn comes.
    public typealias Fetch =
        @Sendable (_ offset: UInt64, _ limit: UInt64, _ stillWanted: @escaping @Sendable () -> Bool)
        async throws -> [RowData]

    static let pageSize = 128
    /// Pages kept either side of the last one asked for.
    static let keptPages = 8

    public let table: Int64
    public private(set) var columns: [ColumnInfo]
    public private(set) var rowCount: Int
    /// Called with the rows that now have data to show, and after `reset`.
    public var onChange: (@MainActor (ClosedRange<Int>?) -> Void)?

    private let fetch: Fetch
    private var pages: [Int: [[GridText]]] = [:]
    private var loading: Set<Int> = []
    private var generation = 0
    /// The pages near the viewport, readable from the queue that runs fetches.
    private let wanted = WantedPages()

    public init(table: Int64, columns: [ColumnInfo], rowCount: Int, fetch: @escaping Fetch) {
        self.table = table
        self.columns = columns
        self.rowCount = rowCount
        self.fetch = fetch
    }

    /// The cell at `row`, `column`, or nil while its page is loading.
    public func cell(row: Int, column: Int) -> GridText? {
        guard let page = pages[row / Self.pageSize] else { return nil }
        let cells = page[safe: row % Self.pageSize]
        return cells?[safe: column]
    }

    /// Makes sure the pages holding `rows` are loaded or on their way.
    public func request(rows: ClosedRange<Int>) {
        guard rowCount > 0 else { return }
        let first = max(0, rows.lowerBound) / Self.pageSize
        let last = min(rowCount - 1, rows.upperBound) / Self.pageSize
        guard first <= last else { return }
        wanted.set(max(0, first - 1)...(last + 1))
        for page in first...last where pages[page] == nil && !loading.contains(page) {
            load(page)
        }
        evict(around: first...last)
    }

    /// Forgets every loaded row, after an edit or undo, and takes the table's new shape.
    public func reset(columns: [ColumnInfo], rowCount: Int) {
        self.columns = columns
        self.rowCount = rowCount
        pages.removeAll()
        loading.removeAll()
        generation += 1
        onChange?(nil)
    }

    private func load(_ page: Int) {
        loading.insert(page)
        let generation = generation
        let wanted = wanted
        let stillWanted: @Sendable () -> Bool = { wanted.contains(page) }
        let offset = UInt64(page * Self.pageSize)
        Task {
            let rows: [[GridText]]
            do {
                rows = try await Self.formatted(fetch(offset, UInt64(Self.pageSize), stillWanted))
            } catch {
                // Skipped because the grid moved on, or the document closed. A failed read
                // leaves the page empty and is tried again when the page is next drawn.
                if generation == self.generation { loading.remove(page) }
                return
            }
            guard generation == self.generation else { return }
            loading.remove(page)
            pages[page] = rows
            let start = page * Self.pageSize
            onChange?(start...(start + max(rows.count, 1) - 1))
        }
    }

    /// Formats a page off the main thread: a page of numbers costs milliseconds, and while
    /// the scroller is dragged a new page arrives every frame.
    private nonisolated static func formatted(_ rows: [RowData]) async -> [[GridText]] {
        rows.map { $0.cells.map(GridText.init) }
    }

    private func evict(around visible: ClosedRange<Int>) {
        let keep = (visible.lowerBound - Self.keptPages)...(visible.upperBound + Self.keptPages)
        for page in pages.keys where !keep.contains(page) {
            pages[page] = nil
        }
    }

    // MARK: For tests

    var loadedPages: Set<Int> { Set(pages.keys) }
}

/// Which pages the grid wants, shared with the queue that runs its reads.
final class WantedPages: Sendable {
    private let pages = Mutex<ClosedRange<Int>>(0...0)

    func set(_ range: ClosedRange<Int>) {
        pages.withLock { $0 = range }
    }

    func contains(_ page: Int) -> Bool {
        pages.withLock { $0.contains(page) }
    }
}

extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}
