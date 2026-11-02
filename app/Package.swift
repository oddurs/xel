// swift-tools-version: 6.2
import PackageDescription

/// Where cargo put the engine's static library. `scripts/task build` builds it, generates
/// the Swift bindings under Generated/, and passes this directory, which follows
/// CARGO_TARGET_DIR and the build profile.
let engineLibraries =
    Context.environment["XEL_ENGINE_LIB_DIR"] ?? "\(Context.packageDirectory)/../target/debug"

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
        // The app's interface to the engine: runs it off the main thread and reports changes.
        .target(name: "XelKit", dependencies: ["XelEngine"]),
        .executableTarget(name: "Xel", dependencies: ["XelKit"]),
        .testTarget(name: "XelKitTests", dependencies: ["XelKit", "XelEngine"]),
    ]
)
