import AppKit
import CoreText
import SwiftUI
import XelEngine

/// Layout shared by the grid and its rulers.
@MainActor
enum GridMetrics {
    static let rowHeight: CGFloat = 22
    static let columnWidth: CGFloat = 120
    static let headerHeight: CGFloat = 24
    static let rowNumberWidth: CGFloat = 64
    static let padding: CGFloat = 6
    static let font = NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .regular)
    static let headerFont = NSFont.systemFont(ofSize: 12, weight: .semibold)
}

/// The table's cells: one view the size of the whole table that draws only what is dirty,
/// with Core Text, as spike 0008 measured holding 120 Hz over a million rows.
public final class GridView: NSView {
    public var data: GridData? {
        didSet {
            // The old table's reads still in flight must not redraw this grid.
            if oldValue !== data { oldValue?.onChange = nil }
            data?.onChange = { [weak self] rows in self?.changed(rows) }
            relayout()
            // Another table starts at its top, not where the last one was scrolled to.
            if oldValue?.table != data?.table { scroll(.zero) }
        }
    }

    override public var isFlipped: Bool { true }
    override public var isOpaque: Bool { true }

    private func changed(_ rows: ClosedRange<Int>?) {
        guard let rows else {
            relayout()
            return
        }
        setNeedsDisplay(
            NSRect(
                x: 0, y: CGFloat(rows.lowerBound) * GridMetrics.rowHeight, width: bounds.width,
                height: CGFloat(rows.count) * GridMetrics.rowHeight))
    }

    func relayout() {
        let columns = data?.columns.count ?? 0
        let rows = data?.rowCount ?? 0
        setFrameSize(
            NSSize(
                width: CGFloat(columns) * GridMetrics.columnWidth,
                height: CGFloat(rows) * GridMetrics.rowHeight))
        needsDisplay = true
        (enclosingScrollView?.superview as? GridContainer)?.headerAndNumbersChanged()
    }

    override public func draw(_ dirty: NSRect) {
        NSColor.textBackgroundColor.setFill()
        dirty.fill()
        guard let data, data.rowCount > 0, !data.columns.isEmpty,
            let context = NSGraphicsContext.current?.cgContext
        else { return }
        let rows = Self.range(
            dirty.minY, dirty.maxY, step: GridMetrics.rowHeight, count: data.rowCount)
        let columns = Self.range(
            dirty.minX, dirty.maxX, step: GridMetrics.columnWidth, count: data.columns.count)
        data.request(rows: rows)

        let lines = NSBezierPath()
        for row in rows {
            let y = CGFloat(row + 1) * GridMetrics.rowHeight - 0.5
            lines.move(to: NSPoint(x: dirty.minX, y: y))
            lines.line(to: NSPoint(x: dirty.maxX, y: y))
        }
        for column in columns {
            let x = CGFloat(column + 1) * GridMetrics.columnWidth - 0.5
            lines.move(to: NSPoint(x: x, y: dirty.minY))
            lines.line(to: NSPoint(x: x, y: dirty.maxY))
        }
        NSColor.separatorColor.setStroke()
        lines.lineWidth = 1
        lines.stroke()

        // The view is flipped; Core Text draws upright with a flipped text matrix.
        context.textMatrix = CGAffineTransform(scaleX: 1, y: -1)
        for row in rows {
            for column in columns {
                guard let cell = data.cell(row: row, column: column), !cell.text.isEmpty else {
                    continue
                }
                draw(cell, in: cellRect(row: row, column: column), context: context)
            }
        }
    }

    func cellRect(row: Int, column: Int) -> NSRect {
        NSRect(
            x: CGFloat(column) * GridMetrics.columnWidth, y: CGFloat(row) * GridMetrics.rowHeight,
            width: GridMetrics.columnWidth, height: GridMetrics.rowHeight)
    }

    private func draw(_ cell: GridText, in rect: NSRect, context: CGContext) {
        let color: NSColor = cell.kind == .error ? .systemRed : .labelColor
        let attributes: [NSAttributedString.Key: Any] = [
            .font: GridMetrics.font, .foregroundColor: color.cgColor,
        ]
        let room = rect.width - 2 * GridMetrics.padding
        var line = CTLineCreateWithAttributedString(
            NSAttributedString(string: cell.text, attributes: attributes))
        if CTLineGetTypographicBounds(line, nil, nil, nil) > room {
            if cell.kind == .number {
                // A cut-off number reads as a different number; show that it does not fit,
                // as Excel does.
                line = CTLineCreateWithAttributedString(
                    NSAttributedString(string: "###", attributes: attributes))
            } else {
                let ellipsis = CTLineCreateWithAttributedString(
                    NSAttributedString(string: "…", attributes: attributes))
                line = CTLineCreateTruncatedLine(line, room, .end, ellipsis) ?? line
            }
        }
        let width = CTLineGetTypographicBounds(line, nil, nil, nil)
        let x: CGFloat =
            switch cell.kind {
            case .number: rect.maxX - GridMetrics.padding - width
            case .checkbox: rect.midX - width / 2
            case .text, .date, .error: rect.minX + GridMetrics.padding
            }
        let baseline =
            rect.minY + (rect.height + GridMetrics.font.ascender + GridMetrics.font.descender) / 2
        context.textPosition = CGPoint(x: x, y: baseline)
        CTLineDraw(line, context)
    }

    /// The indexes of the rows or columns that the span `from...to` touches.
    static func range(_ from: CGFloat, _ to: CGFloat, step: CGFloat, count: Int) -> ClosedRange<Int>
    {
        let first = max(0, min(count - 1, Int(from / step)))
        let last = max(first, min(count - 1, Int((to - 0.5) / step)))
        return first...last
    }
}

/// Draws `text` with Core Text, left-aligned at `x` or right-aligned to it, centred in a
/// row-high band starting at `top`.
@MainActor
func drawLabel(
    _ text: String, x: CGFloat, top: CGFloat, rightAligned: Bool, font: NSFont, color: NSColor,
    in context: CGContext
) {
    let line = CTLineCreateWithAttributedString(
        NSAttributedString(
            string: text, attributes: [.font: font, .foregroundColor: color.cgColor]))
    let width = rightAligned ? CTLineGetTypographicBounds(line, nil, nil, nil) : 0
    context.textPosition = CGPoint(
        x: x - width, y: top + (GridMetrics.rowHeight + font.ascender + font.descender) / 2)
    CTLineDraw(line, context)
}

/// Column names above the grid, following its horizontal scroll position.
final class ColumnHeader: NSView {
    weak var grid: GridView?
    override var isFlipped: Bool { true }

    override func draw(_ dirty: NSRect) {
        NSColor.windowBackgroundColor.setFill()
        bounds.fill()
        guard let grid, let data = grid.data, !data.columns.isEmpty,
            let context = NSGraphicsContext.current?.cgContext
        else { return }
        // The clip view's origin, which goes past the edges while it bounces.
        let offset = grid.enclosingScrollView?.contentView.bounds.minX ?? grid.visibleRect.minX
        let columns = GridView.range(
            offset, offset + bounds.width, step: GridMetrics.columnWidth, count: data.columns.count)
        context.textMatrix = CGAffineTransform(scaleX: 1, y: -1)
        let lines = NSBezierPath()
        for column in columns {
            let left = CGFloat(column) * GridMetrics.columnWidth - offset
            context.saveGState()
            context.clip(
                to: NSRect(x: left, y: 0, width: GridMetrics.columnWidth - 1, height: bounds.height)
            )
            drawLabel(
                data.columns[column].name, x: left + GridMetrics.padding, top: 1,
                rightAligned: false,
                font: GridMetrics.headerFont, color: .labelColor, in: context)
            context.restoreGState()
            lines.move(to: NSPoint(x: left + GridMetrics.columnWidth - 0.5, y: 0))
            lines.line(to: NSPoint(x: left + GridMetrics.columnWidth - 0.5, y: bounds.maxY))
        }
        lines.move(to: NSPoint(x: 0, y: bounds.maxY - 0.5))
        lines.line(to: NSPoint(x: bounds.maxX, y: bounds.maxY - 0.5))
        NSColor.separatorColor.setStroke()
        lines.stroke()
    }
}

/// Row numbers beside the grid, following its vertical scroll position.
final class RowNumbers: NSView {
    weak var grid: GridView?
    override var isFlipped: Bool { true }

    override func draw(_ dirty: NSRect) {
        NSColor.windowBackgroundColor.setFill()
        bounds.fill()
        guard let grid, let data = grid.data, data.rowCount > 0,
            let context = NSGraphicsContext.current?.cgContext
        else { return }
        let offset = grid.enclosingScrollView?.contentView.bounds.minY ?? grid.visibleRect.minY
        let rows = GridView.range(
            offset, offset + bounds.height, step: GridMetrics.rowHeight, count: data.rowCount)
        context.textMatrix = CGAffineTransform(scaleX: 1, y: -1)
        for row in rows {
            drawLabel(
                "\(row + 1)", x: bounds.maxX - GridMetrics.padding,
                top: CGFloat(row) * GridMetrics.rowHeight - offset, rightAligned: true,
                font: GridMetrics.font, color: .secondaryLabelColor, in: context)
        }
        let edge = NSBezierPath()
        edge.move(to: NSPoint(x: bounds.maxX - 0.5, y: 0))
        edge.line(to: NSPoint(x: bounds.maxX - 0.5, y: bounds.maxY))
        NSColor.separatorColor.setStroke()
        edge.stroke()
    }
}

/// The grid with its column header above and row numbers beside it. A plain container
/// rather than NSScrollView's rulers, which overlapped the cells under the window's
/// toolbar insets.
public final class GridContainer: NSView {
    public let scrollView = NSScrollView()
    let grid = GridView()
    let header = ColumnHeader()
    let numbers = RowNumbers()

    override public var isFlipped: Bool { true }

    public var data: GridData? {
        get { grid.data }
        set { grid.data = newValue }
    }

    override public init(frame: NSRect) {
        super.init(frame: frame)
        scrollView.documentView = grid
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = true
        scrollView.automaticallyAdjustsContentInsets = false
        scrollView.drawsBackground = true
        scrollView.backgroundColor = .textBackgroundColor
        header.grid = grid
        numbers.grid = grid
        for view in [scrollView, header, numbers] { addSubview(view) }
        scrollView.contentView.postsBoundsChangedNotifications = true
        NotificationCenter.default.addObserver(
            self, selector: #selector(scrolled), name: NSView.boundsDidChangeNotification,
            object: scrollView.contentView)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("GridContainer is made in code") }

    @objc private func scrolled() {
        headerAndNumbersChanged()
    }

    func headerAndNumbersChanged() {
        header.needsDisplay = true
        numbers.needsDisplay = true
    }

    override public func layout() {
        super.layout()
        placeChildren()
    }

    /// Resizing a view without constraints resizes its subviews rather than laying it out.
    override public func resizeSubviews(withOldSize oldSize: NSSize) {
        placeChildren()
    }

    private func placeChildren() {
        let top = GridMetrics.headerHeight
        let left = GridMetrics.rowNumberWidth
        header.frame = NSRect(x: left, y: 0, width: max(0, bounds.width - left), height: top)
        numbers.frame = NSRect(x: 0, y: top, width: left, height: max(0, bounds.height - top))
        scrollView.frame = NSRect(
            x: left, y: top, width: max(0, bounds.width - left), height: max(0, bounds.height - top)
        )
    }
}

/// A grid with its header and row numbers, ready to place in a window.
@MainActor
public func makeGridView(data: GridData?) -> GridContainer {
    let container = GridContainer(frame: NSRect(x: 0, y: 0, width: 600, height: 400))
    container.data = data
    return container
}

/// The grid in SwiftUI.
struct GridRepresentable: NSViewRepresentable {
    let data: GridData

    func makeNSView(context: Context) -> GridContainer {
        makeGridView(data: data)
    }

    func updateNSView(_ container: GridContainer, context: Context) {
        guard container.data !== data else { return }
        container.data = data
    }
}
