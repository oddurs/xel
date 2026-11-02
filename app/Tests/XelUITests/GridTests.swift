import AppKit
import Foundation
import Synchronization
import Testing
import XelEngine
import XelKit

@testable import XelUI

/// Records the reads a grid makes and answers them with rows named by their index.
final class FakeRows: Sendable {
    private let calls = Mutex<[UInt64]>([])
    private let skipped = Mutex<[UInt64]>([])
    /// Reads wait here until the gate opens, so a test can move the viewport while they
    /// wait their turn. Once open it stays open.
    private let gate = Mutex<(open: Bool, waiting: [CheckedContinuation<Void, Never>])>(
        (false, []))

    /// Offsets whose read fails, as an engine error would.
    let failing: Set<UInt64>

    init(gated: Bool = false, failing: Set<UInt64> = []) {
        self.failing = failing
        gate.withLock { $0.open = !gated }
    }

    var offsets: [UInt64] { calls.withLock { $0 } }
    var skippedOffsets: [UInt64] { skipped.withLock { $0 } }

    func open() {
        let waiting = gate.withLock { state in
            state.open = true
            defer { state.waiting = [] }
            return state.waiting
        }
        for continuation in waiting { continuation.resume() }
    }

    func fetch(
        _ offset: UInt64, _ limit: UInt64, _ stillWanted: @escaping @Sendable () -> Bool
    ) async throws -> [RowData] {
        await withCheckedContinuation { continuation in
            let open = gate.withLock { state in
                if !state.open { state.waiting.append(continuation) }
                return state.open
            }
            if open { continuation.resume() }
        }
        guard stillWanted() else {
            skipped.withLock { $0.append(offset) }
            throw CancellationError()
        }
        calls.withLock { $0.append(offset) }
        if failing.contains(offset) { throw XelError.Other(message: "unreadable page") }
        return (offset..<offset + limit).map { row in
            RowData(id: RowRef(table: 1, row: Int64(row) + 1), cells: [.text(value: "r\(row)")])
        }
    }
}

let oneColumn = [
    ColumnInfo(id: 1, name: "Name", kind: .text, options: [], formula: nil, total: nil)
]

/// Waits until `condition` holds, for up to two seconds.
@MainActor
func eventually(_ condition: () -> Bool) async throws {
    let deadline = ContinuousClock.now + .seconds(5)
    while !condition() {
        guard ContinuousClock.now < deadline else {
            Issue.record("condition never held")
            return
        }
        try await Task.sleep(for: .milliseconds(2))
    }
}

@MainActor
@Suite struct GridDataTests {
    @Test func visibleRowsLoadTheirPageOnceAndReportIt() async throws {
        let rows = FakeRows()
        let data = GridData(table: 1, columns: oneColumn, rowCount: 1_000_000, fetch: rows.fetch)
        var changed: [ClosedRange<Int>?] = []
        data.onChange = { changed.append($0) }

        #expect(data.cell(row: 130, column: 0) == nil, "nothing is loaded before it is asked for")
        data.request(rows: 130...150)
        data.request(rows: 131...151)
        try await eventually { data.cell(row: 130, column: 0) != nil }
        #expect(data.cell(row: 130, column: 0)?.text == "r130")
        #expect(rows.offsets == [128], "one page, read once")
        #expect(changed == [128...255])
    }

    @Test func pagesFarFromTheViewportAreDropped() async throws {
        let rows = FakeRows()
        let data = GridData(table: 1, columns: oneColumn, rowCount: 1_000_000, fetch: rows.fetch)
        data.request(rows: 0...20)
        try await eventually { data.loadedPages == [0] }
        data.request(rows: 5 * 128...5 * 128 + 20)
        try await eventually { data.loadedPages == [0, 5] }
        #expect(data.loadedPages.contains(0), "pages within reach are kept")
        data.request(rows: 500_000...500_020)
        try await eventually { data.loadedPages.contains(500_000 / 128) }
        #expect(!data.loadedPages.contains(0))
        #expect(data.loadedPages.count <= 2 * GridData.keptPages + 1)
    }

    @Test func aPageTheGridHasScrolledPastIsNotRead() async throws {
        let rows = FakeRows(gated: true)
        let data = GridData(table: 1, columns: oneColumn, rowCount: 1_000_000, fetch: rows.fetch)
        data.request(rows: 0...20)
        // Dragging the scroller far away while the first read waits.
        data.request(rows: 900_000...900_020)
        try await Task.sleep(for: .milliseconds(20))
        rows.open()
        try await eventually { !rows.skippedOffsets.isEmpty }
        #expect(rows.skippedOffsets.contains(0))
        #expect(data.cell(row: 0, column: 0) == nil)
    }

    @Test func resetForgetsRowsAndDropsReadsStillInFlight() async throws {
        let rows = FakeRows(gated: true)
        let data = GridData(table: 1, columns: oneColumn, rowCount: 100, fetch: rows.fetch)
        data.request(rows: 0...20)
        try await Task.sleep(for: .milliseconds(20))
        data.reset(columns: oneColumn, rowCount: 50)
        rows.open()
        try await eventually { rows.skippedOffsets == [0] }
        #expect(rows.offsets.isEmpty, "the read queued before the reset is skipped, not run")
        #expect(data.cell(row: 0, column: 0) == nil)
        #expect(data.rowCount == 50)

        data.request(rows: 0...20)
        try await eventually { data.cell(row: 0, column: 0) != nil }
    }

    @Test func anEditKeepsTheOldRowsOnScreenUntilTheyAreReadAgain() async throws {
        let rows = FakeRows()
        let data = GridData(table: 1, columns: oneColumn, rowCount: 100, fetch: rows.fetch)
        data.request(rows: 0...20)
        try await eventually { data.cell(row: 0, column: 0) != nil }
        data.reset(columns: oneColumn, rowCount: 101)
        #expect(data.cell(row: 0, column: 0)?.text == "r0", "not blank while it is reread")
        data.request(rows: 0...20)
        try await eventually { rows.offsets == [0, 0] }

        let renamed = [
            ColumnInfo(id: 2, name: "Other", kind: .text, options: [], formula: nil, total: nil)
        ]
        data.reset(columns: renamed, rowCount: 101)
        #expect(data.cell(row: 0, column: 0) == nil, "rows of other columns are not shown")
    }

    @Test func aPageThatCannotBeReadIsReportedAndNotRetriedEveryFrame() async throws {
        let rows = FakeRows(failing: [0])
        let data = GridData(table: 1, columns: oneColumn, rowCount: 1_000, fetch: rows.fetch)
        var errors: [String] = []
        data.onError = { errors.append("\($0)") }
        data.request(rows: 0...20)
        try await eventually { errors.count == 1 }
        data.request(rows: 0...20)
        data.request(rows: 0...20)
        try await Task.sleep(for: .milliseconds(20))
        #expect(rows.offsets == [0], "a failed page waits for the next reset")
    }

    @Test func aPageThatArrivesAfterTheGridMovedFarIsNotKept() async throws {
        let rows = FakeRows(gated: true)
        let data = GridData(table: 1, columns: oneColumn, rowCount: 1_000_000, fetch: rows.fetch)
        data.request(rows: 0...20)
        try await Task.sleep(for: .milliseconds(10))
        // The read has already passed its check; then the grid moves far away.
        rows.open()
        data.request(rows: 900_000...900_020)
        try await eventually { data.loadedPages.contains(900_000 / 128) }
        #expect(!data.loadedPages.contains(0))
    }

    @Test func cellsAreFormattedAsTheGridShowsThem() {
        #expect(GridText(.number(value: 1200)).text == "1200")
        #expect(GridText(.number(value: 0.1 + 0.2)).text == "0.3")
        #expect(GridText(.number(value: 1e20)).text == "1e+20")
        #expect(GridText(.number(value: -0.0)).text == "0")
        #expect(GridText(.number(value: -2.5)).kind == .number)
        #expect(GridText(.date(year: 2026, month: 11, day: 3)).text == "2026-11-03")
        #expect(GridText(.checkbox(value: true)).text == "✓")
        #expect(GridText(.checkbox(value: false)).text.isEmpty)
        #expect(GridText(.error(code: "#DIV/0!")) == GridText(.error(code: "#DIV/0!")))
        #expect(GridText(.error(code: "#DIV/0!")).kind == .error)
        #expect(GridText(.empty).text.isEmpty)
    }

    @Test func visibleSpansMapToRowsAndColumns() {
        #expect(GridView.range(0, 22, step: 22, count: 10) == 0...0)
        #expect(GridView.range(0, 23, step: 22, count: 10) == 0...1)
        #expect(GridView.range(-50, 10_000, step: 22, count: 10) == 0...9)
        #expect(GridView.range(500, 600, step: 22, count: 10) == 9...9)
    }
}

@MainActor
@Suite struct DocumentGridTests {
    @Test func theGridShowsTheSelectedTableAndFollowsEdits() async throws {
        let scratch = try Scratch()
        let url = scratch.file("two.xel")
        let book = try await Workbook.create(at: url)
        let people = try await book.createTable(
            named: "People", columns: [NewColumn(name: "Name", kind: .text)])
        let places = try await book.createTable(
            named: "Places",
            columns: [NewColumn(name: "City", kind: .text), NewColumn(name: "Pop", kind: .number)])
        _ = try await book.insertRow(into: people, values: [])
        try await book.close()

        let document = try XelDocument(contentsOf: url, ofType: workbookType)
        try await eventually { document.model.grid?.table == people }
        #expect(document.model.grid?.rowCount == 1)

        document.select(places)
        try await eventually { document.model.grid?.table == places }
        #expect(document.model.grid?.columns.map(\.name) == ["City", "Pop"])
        #expect(document.model.grid?.rowCount == 0)

        document.edit("Add Row") { workbook in
            _ = try await workbook.insertRow(into: places, values: [])
        }
        try await eventually { document.model.grid?.rowCount == 1 }

        document.select(nil)
        #expect(document.model.selection == places, "clicking empty sidebar space keeps the table")
        document.edit("Delete Table") { workbook in try await workbook.deleteTable(places) }
        try await eventually { document.model.grid?.table == people }
        document.close()
    }

    @Test func theGridDrawsWithoutAWindow() async throws {
        let rows = FakeRows()
        let data = GridData(table: 1, columns: oneColumn, rowCount: 1_000, fetch: rows.fetch)
        let container = makeGridView(data: data)
        container.frame = NSRect(x: 0, y: 0, width: 400, height: 300)
        container.layoutSubtreeIfNeeded()
        let grid = try #require(container.scrollView.documentView as? GridView)
        #expect(container.scrollView.frame.minX == GridMetrics.rowNumberWidth)
        #expect(container.scrollView.frame.minY == GridMetrics.headerHeight)
        #expect(grid.frame.height == 1_000 * GridMetrics.rowHeight)
        let bitmap = try #require(grid.bitmapImageRepForCachingDisplay(in: grid.visibleRect))
        grid.cacheDisplay(in: grid.visibleRect, to: bitmap)
        try await eventually { data.loadedPages == [0] }
        #expect(rows.offsets == [0], "drawing the top asks for the top page only")

        container.frame.size = NSSize(width: 800, height: 500)
        #expect(container.scrollView.frame.width == 800 - GridMetrics.rowNumberWidth)
    }
}
