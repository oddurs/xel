import SwiftUI

@main
struct XelApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

struct ContentView: View {
    var body: some View {
        ContentUnavailableView(
            "No Workbook Open",
            systemImage: "tablecells",
            description: Text("xel is in early development.")
        )
        .frame(minWidth: 480, minHeight: 320)
    }
}
