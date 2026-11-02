import AppKit
import QuartzCore
import XelEngine
import XelKit
import XelUI

// Scrolls a synthetic 1,000,000 × 50 table in one of two renderers and times every frame
// with a display link: `swift run -c release GridSpike <table|cells|custom|lean|ct|async|canvas|empty|xel> <steady|jump>`.
// Prints one JSON line of frame statistics and memory.

let rowCount = 1_000_000
let columnCount = 50
let rowHeight: CGFloat = 22
let columnWidth: CGFloat = 100

@MainActor func cellText(_ row: Int, _ column: Int) -> String {
    column == 0
        ? "Row \(row + 1)"
        : String(format: "%.2f", Double((row &* 31 &+ column &* 7) % 100_003) / 13.0)
}

let font = NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .regular)
let textAttributes: [NSAttributedString.Key: Any] = [
    .font: font, .foregroundColor: NSColor.labelColor,
]

// MARK: Renderer A: a view-based NSTableView with reused cell views.

final class TableSource: NSObject, NSTableViewDataSource, NSTableViewDelegate {
    func numberOfRows(in tableView: NSTableView) -> Int { rowCount }

    func tableView(_ tableView: NSTableView, viewFor column: NSTableColumn?, row: Int) -> NSView? {
        guard let column else { return nil }
        let id = NSUserInterfaceItemIdentifier("cell")
        let field: NSTextField
        if let reused = tableView.makeView(withIdentifier: id, owner: nil) as? NSTextField {
            field = reused
        } else {
            field = NSTextField(labelWithString: "")
            field.identifier = id
            field.font = font
        }
        let index = Int(column.identifier.rawValue) ?? 0
        field.stringValue = cellText(row, index)
        return field
    }
}

@MainActor func makeTable() -> NSScrollView {
    let table = NSTableView()
    for c in 0..<columnCount {
        let column = NSTableColumn(identifier: .init("\(c)"))
        column.width = columnWidth
        column.title = "C\(c)"
        table.addTableColumn(column)
    }
    table.rowHeight = rowHeight
    table.style = .plain
    table.gridStyleMask = [.solidVerticalGridLineMask, .solidHorizontalGridLineMask]
    table.usesAutomaticRowHeights = false
    let source = TableSource()
    objc_setAssociatedObject(table, "source", source, .OBJC_ASSOCIATION_RETAIN)
    table.dataSource = source
    table.delegate = source
    let scroll = NSScrollView()
    scroll.documentView = table
    scroll.hasVerticalScroller = true
    scroll.hasHorizontalScroller = true
    return scroll
}

// MARK: Renderer A2: a cell-based NSTableView, which draws NSCells instead of views.

final class CellSource: NSObject, NSTableViewDataSource {
    func numberOfRows(in tableView: NSTableView) -> Int { rowCount }

    func tableView(_ tableView: NSTableView, objectValueFor column: NSTableColumn?, row: Int)
        -> Any?
    {
        cellText(row, Int(column?.identifier.rawValue ?? "0") ?? 0)
    }
}

@MainActor func makeCellTable() -> NSScrollView {
    let scroll = makeTable()
    let table = scroll.documentView as! NSTableView
    let source = CellSource()
    objc_setAssociatedObject(table, "source", source, .OBJC_ASSOCIATION_RETAIN)
    table.delegate = nil
    table.dataSource = source
    for column in table.tableColumns {
        let cell = NSTextFieldCell()
        cell.font = font
        column.dataCell = cell
    }
    return scroll
}

// MARK: Renderer B: one document view the size of the whole table, drawing its dirty rect.

final class GridView: NSView {
    /// Prepare only what is visible, instead of AppKit's overdraw above and below.
    var lean = false
    override var isFlipped: Bool { true }
    override var isOpaque: Bool { true }

    override func prepareContent(in rect: NSRect) {
        super.prepareContent(in: lean ? visibleRect : rect)
    }

    override func draw(_ dirty: NSRect) {
        drawCells(in: dirty, offset: .zero)
    }
}

/// Draw text with Core Text lines instead of NSString's attribute machinery.
nonisolated(unsafe) var useCoreText = false
let coreTextAttributes: [NSAttributedString.Key: Any] = [
    .font: font, .foregroundColor: NSColor.labelColor.cgColor,
]

/// Draws the cells intersecting `rect` (in table coordinates), shifted by `-offset`.
@MainActor func drawCells(in rect: NSRect, offset: NSPoint) {
    NSColor.textBackgroundColor.setFill()
    NSRect(x: rect.minX - offset.x, y: rect.minY - offset.y, width: rect.width, height: rect.height)
        .fill()
    let r0 = max(0, Int(rect.minY / rowHeight))
    let r1 = min(rowCount - 1, Int(rect.maxY / rowHeight))
    let c0 = max(0, Int(rect.minX / columnWidth))
    let c1 = min(columnCount - 1, Int(rect.maxX / columnWidth))
    guard r0 <= r1, c0 <= c1 else { return }
    let lines = NSBezierPath()
    for r in r0...r1 {
        let y = CGFloat(r + 1) * rowHeight - offset.y - 0.5
        lines.move(to: NSPoint(x: rect.minX - offset.x, y: y))
        lines.line(to: NSPoint(x: rect.maxX - offset.x, y: y))
    }
    for c in c0...c1 {
        let x = CGFloat(c + 1) * columnWidth - offset.x - 0.5
        lines.move(to: NSPoint(x: x, y: rect.minY - offset.y))
        lines.line(to: NSPoint(x: x, y: rect.maxY - offset.y))
    }
    NSColor.separatorColor.setStroke()
    lines.lineWidth = 1
    lines.stroke()
    if useCoreText, let context = NSGraphicsContext.current?.cgContext {
        // The view is flipped; Core Text draws upright in an unflipped text matrix.
        context.textMatrix = CGAffineTransform(scaleX: 1, y: -1)
        let ascent = font.ascender
        for r in r0...r1 {
            for c in c0...c1 {
                let line = CTLineCreateWithAttributedString(
                    NSAttributedString(string: cellText(r, c), attributes: coreTextAttributes))
                context.textPosition = CGPoint(
                    x: CGFloat(c) * columnWidth + 4 - offset.x,
                    y: CGFloat(r) * rowHeight + 3 + ascent - offset.y)
                CTLineDraw(line, context)
            }
        }
        return
    }
    for r in r0...r1 {
        for c in c0...c1 {
            (cellText(r, c) as NSString).draw(
                at: NSPoint(
                    x: CGFloat(c) * columnWidth + 4 - offset.x,
                    y: CGFloat(r) * rowHeight + 3 - offset.y),
                withAttributes: textAttributes)
        }
    }
}

@MainActor func makeCustom(lean: Bool = false) -> NSScrollView {
    let grid = GridView(
        frame: NSRect(
            x: 0, y: 0, width: CGFloat(columnCount) * columnWidth,
            height: CGFloat(rowCount) * rowHeight))
    grid.lean = lean
    let scroll = NSScrollView()
    scroll.documentView = grid
    scroll.hasVerticalScroller = true
    scroll.hasHorizontalScroller = true
    return scroll
}

// MARK: Renderer C: an empty document view for the scroll extent, and a window-sized
// canvas pinned to the clip view that draws whatever is visible.

final class Canvas: NSView {
    weak var clip: NSClipView?
    override var isFlipped: Bool { true }
    override var isOpaque: Bool { true }

    override func draw(_ dirty: NSRect) {
        let origin = clip?.bounds.origin ?? .zero
        drawCells(in: dirty.offsetBy(dx: origin.x, dy: origin.y), offset: origin)
    }
}

/// A document view that only fills its background: the memory a scrolling window costs
/// before any grid is drawn.
final class Blank: NSView {
    override var isFlipped: Bool { true }
    override var isOpaque: Bool { true }
    override func draw(_ dirty: NSRect) {
        NSColor.textBackgroundColor.setFill()
        dirty.fill()
    }
}

final class Extent: NSView {
    override var isFlipped: Bool { true }
}

@MainActor func makeCanvas() -> NSScrollView {
    let scroll = NSScrollView()
    scroll.documentView = Extent(
        frame: NSRect(
            x: 0, y: 0, width: CGFloat(columnCount) * columnWidth,
            height: CGFloat(rowCount) * rowHeight))
    scroll.hasVerticalScroller = true
    scroll.hasHorizontalScroller = true
    let canvas = Canvas()
    canvas.clip = scroll.contentView
    canvas.translatesAutoresizingMaskIntoConstraints = false
    scroll.addSubview(canvas, positioned: .below, relativeTo: scroll.verticalScroller)
    NSLayoutConstraint.activate([
        canvas.leadingAnchor.constraint(equalTo: scroll.contentView.leadingAnchor),
        canvas.trailingAnchor.constraint(equalTo: scroll.contentView.trailingAnchor),
        canvas.topAnchor.constraint(equalTo: scroll.contentView.topAnchor),
        canvas.bottomAnchor.constraint(equalTo: scroll.contentView.bottomAnchor),
    ])
    scroll.contentView.postsBoundsChangedNotifications = true
    NotificationCenter.default.addObserver(
        forName: NSView.boundsDidChangeNotification, object: scroll.contentView, queue: nil
    ) { _ in
        MainActor.assumeIsolated { canvas.needsDisplay = true }
    }
    return scroll
}

// MARK: Measuring

func footprintMB() -> Double {
    var info = task_vm_info_data_t()
    var count = mach_msg_type_number_t(
        MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<integer_t>.size)
    let result = withUnsafeMutablePointer(to: &info) {
        $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) {
            task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count)
        }
    }
    return result == KERN_SUCCESS ? Double(info.phys_footprint) / 1_048_576 : -1
}

@MainActor
final class Driver: NSObject {
    let scroll: NSScrollView
    let jump: Bool
    var link: CADisplayLink?
    var stamps: [CFTimeInterval] = []
    var target: CFTimeInterval = 0
    var started: CFTimeInterval = 0
    var y: CGFloat = 0
    var startMB: Double = 0
    let label: String

    init(scroll: NSScrollView, jump: Bool, label: String) {
        self.scroll = scroll
        self.jump = jump
        self.label = label
    }

    func start() {
        let link = scroll.displayLink(target: self, selector: #selector(tick(_:)))
        link.preferredFrameRateRange = CAFrameRateRange(minimum: 60, maximum: 120, preferred: 120)
        link.add(to: .main, forMode: .common)
        self.link = link
    }

    @objc func tick(_ link: CADisplayLink) {
        if started == 0 { started = link.timestamp }
        let elapsed = link.timestamp - started
        // Half a second to settle, then four seconds measured.
        if elapsed > 0.5 {
            if stamps.isEmpty { startMB = footprintMB() }
            stamps.append(link.timestamp)
            target = link.targetTimestamp - link.timestamp
        }
        let maxY = CGFloat(rowCount) * rowHeight - scroll.contentView.bounds.height
        // Steady: a fast flick, 50 points a frame. Jump: dragging the scroller, about a
        // thousand rows a frame.
        y += jump ? 997 * rowHeight : 50
        if y > maxY { y = 0 }
        scroll.contentView.scroll(to: NSPoint(x: 0, y: y))
        scroll.reflectScrolledClipView(scroll.contentView)
        if elapsed > 4.5 { finish() }
    }

    func finish() {
        link?.invalidate()
        let intervals = zip(stamps.dropFirst(), stamps).map { ($0 - $1) * 1000 }.sorted()
        func pct(_ p: Double) -> Double {
            intervals.isEmpty
                ? 0 : intervals[min(intervals.count - 1, Int(Double(intervals.count) * p))]
        }
        let frameMS = target * 1000
        let dropped = intervals.filter { $0 > frameMS * 1.5 }.count
        let json = String(
            format:
                #"{"renderer":"%@","frames":%d,"target_ms":%.2f,"p50_ms":%.2f,"p99_ms":%.2f,"max_ms":%.2f,"dropped":%d,"dropped_pct":%.1f,"start_mb":%.0f,"end_mb":%.0f}"#,
            label, intervals.count, frameMS, pct(0.5), pct(0.99), intervals.last ?? 0, dropped,
            intervals.isEmpty ? 0 : Double(dropped) * 100 / Double(intervals.count), startMB,
            footprintMB())
        print(json)
        fflush(stdout)
        NSApp.terminate(nil)
    }
}

let args = CommandLine.arguments
guard args.count == 3,
    args[1] != "xel" || ProcessInfo.processInfo.environment["XEL_BENCH_FILE"] != nil,
    ["table", "cells", "custom", "lean", "ct", "async", "canvas", "empty", "xel"].contains(args[1]),
    ["steady", "jump"].contains(args[2])
else {
    FileHandle.standardError.write(
        Data(
            "usage: GridSpike <table|cells|custom|lean|ct|async|canvas|empty|xel> <steady|jump>\n"
                .utf8)
    )
    exit(2)
}

/// The grid's data for mode `xel`, opened before the window is made.
nonisolated(unsafe) var benchData: GridData?
/// Mode `xel` shows this container, whose scroll view the driver scrolls.
nonisolated(unsafe) var benchContainer: NSView?

if args[1] == "xel", let file = ProcessInfo.processInfo.environment["XEL_BENCH_FILE"] {
    let opened = DispatchSemaphore(value: 0)
    nonisolated(unsafe) var result: (Workbook, TableInfo, UInt64)?
    nonisolated(unsafe) var failure: (any Error)?
    Task.detached {
        defer { opened.signal() }
        do {
            let book = try await Workbook.open(URL(filePath: file))
            guard let table = try await book.tables().first else {
                failure = CocoaError(.fileReadCorruptFile)
                return
            }
            result = (book, table, try await book.rowCount(of: table.id))
        } catch {
            failure = error
        }
    }
    opened.wait()
    if let failure {
        FileHandle.standardError.write(Data("GridSpike: cannot open \(file): \(failure)\n".utf8))
        exit(1)
    }
    MainActor.assumeIsolated {
        guard let (book, table, count) = result else { exit(1) }
        benchData = GridData(table: table.id, columns: table.columns, rowCount: Int(count)) {
            offset, limit, stillWanted in
            guard stillWanted() else { throw CancellationError() }
            return try await book.rows(of: table.id, offset: offset, limit: limit)
        }
    }
}

MainActor.assumeIsolated {
    let app = NSApplication.shared
    app.setActivationPolicy(.regular)
    let scroll: NSScrollView = {
        switch args[1] {
        case "table": return makeTable()
        case "cells": return makeCellTable()
        case "custom": return makeCustom()
        case "lean": return makeCustom(lean: true)
        case "ct":
            useCoreText = true
            return makeCustom()
        case "xel":
            // The app's grid over a real workbook: XEL_BENCH_FILE names a .xel file, and its
            // first table is shown.
            // The whole grid, header and row numbers included: they redraw on every scroll too.
            let container = makeGridView(data: benchData)
            benchContainer = container
            return container.scrollView
        case "empty":
            let scroll = NSScrollView()
            scroll.documentView = Blank(
                frame: NSRect(
                    x: 0, y: 0, width: CGFloat(columnCount) * columnWidth,
                    height: CGFloat(rowCount) * rowHeight))
            scroll.hasVerticalScroller = true
            return scroll
        case "async":
            let scroll = makeCustom()
            scroll.documentView?.wantsLayer = true
            scroll.documentView?.layer?.drawsAsynchronously = true
            return scroll
        default: return makeCanvas()
        }
    }()
    // GRID_SIZE=WxH in points; the default is a large document window.
    let size = ProcessInfo.processInfo.environment["GRID_SIZE"]?.split(separator: "x").compactMap {
        Double($0)
    }
    let width = size?.first ?? 1400
    let height = size?.last ?? 900
    let window = NSWindow(
        contentRect: NSRect(x: 0, y: 0, width: width, height: height), styleMask: [.titled],
        backing: .buffered, defer: false)
    window.title = "Grid spike: \(args[1]) \(args[2])"
    window.contentView = benchContainer ?? scroll
    window.makeKeyAndOrderFront(nil)
    app.activate()
    let driver = Driver(scroll: scroll, jump: args[2] == "jump", label: "\(args[1])-\(args[2])")
    DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { driver.start() }
    withExtendedLifetime(driver) { app.run() }
}
