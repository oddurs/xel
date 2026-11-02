// swift-tools-version: 6.2
import PackageDescription

// Spike 0008's measuring harness, kept so the numbers in its Answer can be reproduced, and
// the real grid (mode `xel`) is measured the same way. Not part of the app or scripts/task.
// Build after `scripts/task build`, with XEL_ENGINE_LIB_DIR pointing at the engine library.
let package = Package(
    name: "GridSpike",
    platforms: [.macOS(.v26)],
    dependencies: [.package(path: "../../app")],
    targets: [
        .executableTarget(
            name: "GridSpike",
            dependencies: [
                .product(name: "XelUI", package: "app"),
                .product(name: "XelKit", package: "app"),
            ]
        )
    ]
)
