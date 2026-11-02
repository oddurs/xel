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
        // Negative zero shows as 0, as in Excel.
        if value == 0 { return "0" }
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
    /// Called when a page cannot be read, so the error reaches the user.
    public var onError: (@MainActor (any Error) -> Void)?
    /// The pages last asked for; pages that arrive far from them are not kept.
    private var visible = 0...0

    private let fetch: Fetch
    private var pages: [Int: [[GridText]]] = [:]
    /// Pages from before the last reset, shown until their fresh copies arrive, so an edit
    /// does not blank the grid while its rows are read again.
    private var stale: [Int: [[GridText]]] = [:]
    private var loading: Set<Int> = []
    /// Pages whose read failed; tried again after the next reset rather than every frame.
    private var failed: Set<Int> = []
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
        let index = row / Self.pageSize
        guard let page = pages[index] ?? stale[index] else { return nil }
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
        visible = first...last
        for page in first...last
        where pages[page] == nil && !loading.contains(page) && !failed.contains(page) {
            load(page)
        }
        evict()
    }

    /// Forgets every loaded row, after an edit or undo, and takes the table's new shape.
    public func reset(columns: [ColumnInfo], rowCount: Int) {
        // The old rows stay on screen until reread, unless the columns changed under them.
        let sameColumns = columns.map(\.id) == self.columns.map(\.id)
        stale = sameColumns ? pages.merging(stale) { fresh, _ in fresh } : [:]
        self.columns = columns
        self.rowCount = rowCount
        pages.removeAll()
        loading.removeAll()
        failed.removeAll()
        generation += 1
        // Reads queued before the reset are skipped rather than run and thrown away.
        wanted.begin(generation)
        onChange?(nil)
    }

    private func load(_ page: Int) {
        loading.insert(page)
        let generation = generation
        let wanted = wanted
        let stillWanted: @Sendable () -> Bool = { wanted.contains(page, generation: generation) }
        let offset = UInt64(page * Self.pageSize)
        Task {
            let rows: [[GridText]]
            do {
                rows = try await Self.formatted(fetch(offset, UInt64(Self.pageSize), stillWanted))
            } catch {
                guard generation == self.generation else { return }
                loading.remove(page)
                if !(error is CancellationError) {
                    // Not retried every frame: the next reset tries again.
                    failed.insert(page)
                    onError?(error)
                }
                // Skipped because the grid had moved on; if it has come back meanwhile,
                // drawing the page again asks for it again.
                onChange?(rowsOf(page))
                return
            }
            guard generation == self.generation else { return }
            loading.remove(page)
            // Scrolled far away while this page was read: keeping it would outgrow the
            // window of pages the cache is bounded to.
            guard keeps(page) else { return }
            pages[page] = rows
            stale[page] = nil
            onChange?(rowsOf(page))
        }
    }

    /// Formats a page off the main thread: a page of numbers costs milliseconds, and while
    /// the scroller is dragged a new page arrives every frame.
    private nonisolated static func formatted(_ rows: [RowData]) async -> [[GridText]] {
        rows.map { $0.cells.map(GridText.init) }
    }

    private func rowsOf(_ page: Int) -> ClosedRange<Int> {
        let start = page * Self.pageSize
        return start...(start + Self.pageSize - 1)
    }

    private func keeps(_ page: Int) -> Bool {
        ((visible.lowerBound - Self.keptPages)...(visible.upperBound + Self.keptPages)).contains(
            page)
    }

    private func evict() {
        for page in pages.keys where !keeps(page) {
            pages[page] = nil
        }
        for page in stale.keys where !keeps(page) {
            stale[page] = nil
        }
    }

    // MARK: For tests

    var loadedPages: Set<Int> { Set(pages.keys) }
}

/// Which pages the grid wants, and for which reset, shared with the queue that runs its
/// reads.
final class WantedPages: Sendable {
    private let state = Mutex<(pages: ClosedRange<Int>, generation: Int)>((0...0, 0))

    func set(_ range: ClosedRange<Int>) {
        state.withLock { $0.pages = range }
    }

    func begin(_ generation: Int) {
        state.withLock { $0.generation = generation }
    }

    func contains(_ page: Int, generation: Int) -> Bool {
        state.withLock { $0.generation == generation && $0.pages.contains(page) }
    }
}

extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}
