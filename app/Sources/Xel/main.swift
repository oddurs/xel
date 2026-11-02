import AppKit
import XelUI

// An AppKit entry point: documents are NSDocuments (see 0011), which SwiftUI's App
// lifecycle does not host.
MainActor.assumeIsolated {
    let app = NSApplication.shared
    let delegate = AppDelegate()
    app.delegate = delegate
    app.mainMenu = MainMenu.make()
    withExtendedLifetime(delegate) { app.run() }
}
