// swift-tools-version: 6.2
import PackageDescription

let package = Package(
    name: "Xel",
    platforms: [.macOS(.v26)],
    targets: [
        .executableTarget(name: "Xel")
    ]
)
