import SwiftUI
import XelEngine

/// What a document window shows, kept current from the workbook.
@MainActor
@Observable
final class DocumentModel {
    private(set) var tables: [TableInfo] = []
    /// The table shown in the grid.
    var selection: Int64?
    /// The selected table's rows, or nil while there is none.
    var grid: GridData?

    /// Takes the workbook's tables, keeping the selection when its table is still there.
    func set(_ tables: [TableInfo]) {
        self.tables = tables
        if !tables.contains(where: { $0.id == selection }) {
            selection = tables.first?.id
        }
    }
}

/// A document window: the workbook's tables beside the selected table's grid.
struct DocumentView: View {
    let model: DocumentModel
    let addTable: () -> Void
    let select: (Int64?) -> Void

    var body: some View {
        NavigationSplitView {
            List(
                model.tables, id: \.id,
                selection: Binding(get: { model.selection }, set: { select($0) })
            ) { table in
                Label(table.name, systemImage: "tablecells")
            }
            .navigationSplitViewColumnWidth(min: 160, ideal: 200)
        } detail: {
            if let grid = model.grid {
                GridRepresentable(data: grid)
            } else if !model.tables.isEmpty {
                // The selected table is being read.
                Color(nsColor: .textBackgroundColor)
            } else {
                ContentUnavailableView {
                    Label("No Tables", systemImage: "tablecells")
                } description: {
                    Text("A workbook holds typed tables.")
                } actions: {
                    Button("New Table", action: addTable)
                }
            }
        }
        .toolbar {
            Button("New Table", systemImage: "plus", action: addTable)
        }
        .frame(minWidth: 480, minHeight: 320)
    }
}
