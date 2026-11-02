// swift-tools-version: 6.2
import PackageDescription

// Spike 0008's measuring harness, kept so the numbers in its Answer can be reproduced and
// 0033 can measure the real grid the same way. Not part of the app or scripts/task.
let package = Package(
    name: "GridSpike",
    platforms: [.macOS(.v26)],
    targets: [.executableTarget(name: "GridSpike")]
)
