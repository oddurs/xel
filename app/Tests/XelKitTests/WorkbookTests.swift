import Foundation
import Testing
import XelEngine

@testable import XelKit

/// A fresh directory for one test, removed when the test ends.
final class Scratch {
    let url: URL

    init() throws {
        url = FileManager.default.temporaryDirectory
            .appending(path: "xel-kit-tests-\(UUID().uuidString)", directoryHint: .isDirectory)
        try FileManager.default.createDirectory(at: url, withIntermediateDirectories: false)
    }

    func file(_ name: String) -> URL { url.appending(path: name) }

    deinit { try? FileManager.default.removeItem(at: url) }
}

extension XelError {
    /// The case, without its message, so tests can compare kinds.
    var kind: String {
        switch self {
        case .NotFound: "NotFound"
        case .AlreadyExists: "AlreadyExists"
        case .NotAWorkbook: "NotAWorkbook"
        case .NewerFormat: "NewerFormat"
        case .ReadOnly: "ReadOnly"
        case .Corrupt: "Corrupt"
        case .Gone: "Gone"
        case .Invalid: "Invalid"
        case .Other: "Other"
        case .Closed: "Closed"
        case .Poisoned: "Poisoned"
        }
    }
}

/// The kind of `XelError` that `body` throws, or a description of what happened instead.
func errorKind(_ body: () async throws -> Void) async -> String {
    do {
        try await body()
        return "no error"
    } catch let error as XelError {
        return error.kind
    } catch {
        return "not an XelError: \(error)"
    }
}

/// A workbook with a table People(Name text, Age number).
func makePeople(in scratch: Scratch) async throws -> (Workbook, TableInfo) {
    let book = try await Workbook.create(at: scratch.file("people.xel"))
    let id = try await book.createTable(
        named: "People",
        columns: [NewColumn(name: "Name", kind: .text), NewColumn(name: "Age", kind: .number)]
    )
    return (book, try await book.table(id))
}

@Test func createReopenAndReadBack() async throws {
    let scratch = try Scratch()
    let (book, people) = try await makePeople(in: scratch)
    let row = try await book.insertRow(
        into: people.id,
        values: [ColumnValue(column: people.columns[0].id, value: .text(value: "Ada"))]
    )
    #expect(try await book.formatVersion() == 1)
    try await book.close()

    let reopened = try await Workbook.open(scratch.file("people.xel"))
    #expect(try await reopened.tables().map(\.name) == ["People"])
    #expect(try await reopened.rowCount(of: people.id) == 1)
    #expect(
        try await reopened.rows(of: people.id, offset: 0, limit: 10)
            == [RowData(id: row, cells: [.text(value: "Ada"), .empty])]
    )
    try await reopened.close()
}

@Test func schemaEdits() async throws {
    let scratch = try Scratch()
    let (book, people) = try await makePeople(in: scratch)
    let (name, age) = (people.columns[0].id, people.columns[1].id)

    try await book.renameTable(people.id, to: "Staff")
    try await book.renameColumn(name, to: "Full name")
    let level = try await book.addColumn(to: people.id, named: "Level", kind: .select)
    try await book.setSelectOptions(of: level, to: ["Junior", "Senior"])
    let next = try await book.addFormulaColumn(to: people.id, named: "Next", formula: "=[@Age]+1")
    try await book.setFormula(of: next, to: "=[@Age]+2")
    try await book.deleteColumn(age)

    let staff = try await book.table(people.id)
    #expect(staff.name == "Staff")
    #expect(staff.columns.map(\.name) == ["Full name", "Level", "Next"])
    #expect(staff.columns.map(\.kind) == [.text, .select, .formula])
    #expect(staff.columns[1].options == ["Junior", "Senior"])
    #expect(staff.columns[2].formula == "=[@Age]+2")

    try await book.deleteTable(people.id)
    #expect(try await book.tables().isEmpty)
    try await book.close()
}

@Test func rowEditsAndFormulaResults() async throws {
    let scratch = try Scratch()
    let (book, people) = try await makePeople(in: scratch)
    let (name, age) = (people.columns[0].id, people.columns[1].id)
    let ratio = try await book.addFormulaColumn(
        to: people.id, named: "Ratio", formula: "=10/[@Age]")

    var ids: [Int64] = []
    for person in ["Ada", "Grace", "Edsger"] {
        ids.append(
            try await book.insertRow(
                into: people.id,
                values: [ColumnValue(column: name, value: .text(value: person))]
            )
        )
    }
    try await book.setCells(
        in: people.id,
        [
            CellEdit(row: ids[0], column: age, value: .number(value: 5)),
            CellEdit(row: ids[1], column: age, value: .number(value: 0)),
        ]
    )
    #expect(try await book.rowCount(of: people.id) == 3)

    let window = try await book.rows(of: people.id, offset: 0, limit: 2)
    #expect(window.map(\.id) == Array(ids[0..<2]))
    #expect(window[0].cells == [.text(value: "Ada"), .number(value: 5), .number(value: 2)])
    #expect(window[1].cells[2] == .error(code: "#DIV/0!"))
    #expect(try await book.table(people.id).columns[2].id == ratio)

    try await book.deleteRows([ids[1]], from: people.id)
    #expect(try await book.rows(of: people.id, offset: 0, limit: 10).map(\.id) == [ids[0], ids[2]])
    try await book.close()
}

@Test func undoAndRedo() async throws {
    let scratch = try Scratch()
    let (book, people) = try await makePeople(in: scratch)
    #expect(try await book.canUndo())
    #expect(try await !book.canRedo())

    #expect(try await book.undo())
    #expect(try await book.tables().isEmpty)
    #expect(try await book.canRedo())
    #expect(try await book.redo())
    #expect(try await book.tables().map(\.id) == [people.id])

    while try await book.undo() {}
    #expect(try await !book.canUndo())
    #expect(try await !book.undo(), "nothing left to undo")
    try await book.close()
}

@Test func changesAreReportedInOrderAndEndOnClose() async throws {
    let scratch = try Scratch()
    let (book, people) = try await makePeople(in: scratch)
    let changes = await book.changes()

    _ = try await book.insertRow(into: people.id, values: [])
    try await book.renameTable(people.id, to: "Staff")
    try await book.undo()
    #expect(try await book.redo())
    try await book.close()

    var seen: [WorkbookChange] = []
    for await change in changes { seen.append(change) }
    #expect(seen == [.rows(table: people.id), .schema, .everything, .everything])
}

@Test func workingCopiesLeaveTheDocumentAloneUntilSaved() async throws {
    let scratch = try Scratch()
    let (original, people) = try await makePeople(in: scratch)
    try await original.close()
    let document = scratch.file("people.xel")
    let before = try Data(contentsOf: document)

    let book = try await Workbook.openCopy(of: document, workingCopy: scratch.file("working.xel"))
    _ = try await book.insertRow(into: people.id, values: [])
    #expect(try Data(contentsOf: document) == before)

    let saved = scratch.file("saved.xel")
    try await book.save(to: saved)
    #expect(await errorKind { try await book.save(to: saved) } == "AlreadyExists")
    try await book.close()
    let reopened = try await Workbook.open(saved)
    #expect(try await reopened.rowCount(of: people.id) == 1)
    try await reopened.close()
}

@Test func engineErrorsArriveAsKinds() async throws {
    let scratch = try Scratch()
    #expect(
        await errorKind { _ = try await Workbook.open(scratch.file("missing.xel")) } == "NotFound")
    let notes = scratch.file("notes.xel")
    try Data("hello".utf8).write(to: notes)
    #expect(await errorKind { _ = try await Workbook.open(notes) } == "NotAWorkbook")

    let (book, people) = try await makePeople(in: scratch)
    let age = people.columns[1].id
    #expect(
        await errorKind { _ = try await book.createTable(named: "xel_x", columns: []) } == "Invalid"
    )
    #expect(await errorKind { _ = try await book.table(999) } == "Gone")
    let row = try await book.insertRow(into: people.id, values: [])
    for bad: Cell in [
        .text(value: "old"), .date(year: 2025, month: 2, day: 30), .error(code: "#N/A"),
    ] {
        #expect(
            await errorKind {
                try await book.setCells(
                    in: people.id, [CellEdit(row: row, column: age, value: bad)])
            } == "Invalid",
            "\(bad)"
        )
    }
    try await book.close()
    #expect(await errorKind { _ = try await book.tables() } == "Closed")
}

@Test func aRustPanicIsAThrownErrorAndPoisonsTheBook() async throws {
    let scratch = try Scratch()
    let (book, _) = try await makePeople(in: scratch)
    let thrown = await errorKind { try await book.checkPanicBoundary() }
    #expect(thrown.hasPrefix("not an XelError"))
    #expect(thrown.contains("panics on purpose"))
    #expect(await errorKind { _ = try await book.tables() } == "Poisoned")
}

@MainActor
@Test func theEngineRunsOffTheMainThread() async throws {
    let scratch = try Scratch()
    let (book, _) = try await makePeople(in: scratch)
    #expect(await book.isOnEngineQueue)
    #expect(await !book.runsOnMainThread)
    try await book.close()
}
