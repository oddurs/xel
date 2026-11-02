import AppKit

@MainActor
public final class AppDelegate: NSObject, NSApplicationDelegate {
    override public init() { super.init() }

    public func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool { true }
}
