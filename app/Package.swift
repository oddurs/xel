// swift-tools-version: 6.2
import PackageDescription

/// Where cargo puts the engine's static library. `scripts/task build` builds it and
/// generates the Swift bindings under Generated/ before building this package.
let engineLibraries = "\(Context.packageDirectory)/../target/debug"

let package = Package(
    name: "Xel",
    platforms: [.macOS(.v26)],
    targets: [
        .systemLibrary(name: "xel_ffiFFI", path: "Generated/xel_ffiFFI"),
        .target(
            name: "XelEngine",
            dependencies: ["xel_ffiFFI"],
            path: "Generated/XelEngine",
            linkerSettings: [
                .unsafeFlags(["-L\(engineLibraries)"]),
                .linkedLibrary("xel_ffi"),
            ]
        ),
        .executableTarget(name: "Xel", dependencies: ["XelEngine"]),
    ]
)
