import AppKit

/// The menu bar, built in code as there is no nib. Document commands go up the responder
/// chain to NSDocument and NSDocumentController, which implement them.
@MainActor
public enum MainMenu {
    public static func make() -> NSMenu {
        let bar = NSMenu()
        bar.addItem(submenu(appMenu()))
        bar.addItem(submenu(fileMenu()))
        bar.addItem(submenu(editMenu()))
        let windows = windowMenu()
        bar.addItem(submenu(windows))
        NSApp.windowsMenu = windows
        let help = NSMenu(title: "Help")
        bar.addItem(submenu(help))
        NSApp.helpMenu = help
        return bar
    }

    private static func submenu(_ menu: NSMenu) -> NSMenuItem {
        let item = NSMenuItem(title: menu.title, action: nil, keyEquivalent: "")
        item.submenu = menu
        return item
    }

    private static func item(
        _ title: String, _ action: String, _ key: String = "",
        _ modifiers: NSEvent.ModifierFlags = .command
    ) -> NSMenuItem {
        let item = NSMenuItem(title: title, action: Selector(action), keyEquivalent: key)
        item.keyEquivalentModifierMask = modifiers
        return item
    }

    private static func appMenu() -> NSMenu {
        let menu = NSMenu(title: "Xel")
        menu.addItem(item("About Xel", "orderFrontStandardAboutPanel:"))
        menu.addItem(.separator())
        let services = NSMenu(title: "Services")
        menu.addItem(submenu(services))
        NSApp.servicesMenu = services
        menu.addItem(.separator())
        menu.addItem(item("Hide Xel", "hide:", "h"))
        menu.addItem(item("Hide Others", "hideOtherApplications:", "h", [.command, .option]))
        menu.addItem(item("Show All", "unhideAllApplications:"))
        menu.addItem(.separator())
        menu.addItem(item("Quit Xel", "terminate:", "q"))
        return menu
    }

    private static func fileMenu() -> NSMenu {
        let menu = NSMenu(title: "File")
        menu.addItem(item("New", "newDocument:", "n"))
        menu.addItem(item("Open…", "openDocument:", "o"))
        // NSDocumentController fills the menu holding the Clear Menu item with recent files.
        let recent = NSMenu(title: "Open Recent")
        recent.addItem(item("Clear Menu", "clearRecentDocuments:"))
        menu.addItem(submenu(recent))
        menu.addItem(.separator())
        menu.addItem(item("Close", "performClose:", "w"))
        menu.addItem(item("Save…", "saveDocument:", "s"))
        menu.addItem(item("Duplicate", "duplicateDocument:", "s", [.command, .shift]))
        menu.addItem(item("Rename…", "renameDocument:"))
        menu.addItem(item("Move To…", "moveDocument:"))
        menu.addItem(item("Revert to Saved", "revertDocumentToSaved:"))
        return menu
    }

    private static func editMenu() -> NSMenu {
        let menu = NSMenu(title: "Edit")
        menu.addItem(item("Undo", "undo:", "z"))
        menu.addItem(item("Redo", "redo:", "z", [.command, .shift]))
        menu.addItem(.separator())
        menu.addItem(item("Cut", "cut:", "x"))
        menu.addItem(item("Copy", "copy:", "c"))
        menu.addItem(item("Paste", "paste:", "v"))
        menu.addItem(item("Select All", "selectAll:", "a"))
        return menu
    }

    private static func windowMenu() -> NSMenu {
        let menu = NSMenu(title: "Window")
        menu.addItem(item("Minimize", "performMiniaturize:", "m"))
        menu.addItem(item("Zoom", "performZoom:"))
        menu.addItem(.separator())
        menu.addItem(item("Bring All to Front", "arrangeInFront:"))
        return menu
    }
}
