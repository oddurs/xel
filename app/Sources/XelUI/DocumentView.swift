import SwiftUI
import XelEngine

/// What a document window shows, kept current from the workbook.
@MainActor
@Observable
final class DocumentModel {
    private(set) var tables: [TableInfo] = []

    func set(_ tables: [TableInfo]) {
        self.tables = tables
    }
}

/// The document window's content until the sidebar (0032) and grid (0033) arrive: the
/// workbook's tables, and a way to add one.
struct DocumentView: View {
    let model: DocumentModel
    let addTable: () -> Void

    var body: some View {
        Group {
            if model.tables.isEmpty {
                ContentUnavailableView {
                    Label("No Tables", systemImage: "tablecells")
                } description: {
                    Text("A workbook holds typed tables.")
                } actions: {
                    Button("New Table", action: addTable)
                }
            } else {
                List(model.tables, id: \.id) { table in
                    LabeledContent(table.name) {
                        Text("^[\(table.columns.count) column](inflect: true)")
                            .foregroundStyle(.secondary)
                    }
                }
            }
        }
        .toolbar {
            Button("New Table", systemImage: "plus", action: addTable)
        }
        .frame(minWidth: 480, minHeight: 320)
    }
}
