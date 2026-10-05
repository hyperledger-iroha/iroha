// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "NoritoBridgeZIPConsumer",
    platforms: [.macOS(.v12)],
    targets: [
        .binaryTarget(
            name: "NoritoBridge",
            path: "NoritoBridge.xcframework.zip"
        ),
        .executableTarget(
            name: "ArchiveConsumer",
            dependencies: ["NoritoBridge"],
            linkerSettings: [
                .linkedFramework("Foundation", .when(platforms: [.iOS, .macOS])),
                .linkedFramework("Security", .when(platforms: [.iOS, .macOS])),
                .linkedFramework("Metal", .when(platforms: [.iOS, .macOS])),
                .linkedFramework("CoreGraphics", .when(platforms: [.iOS, .macOS])),
                .linkedFramework("Accelerate", .when(platforms: [.iOS, .macOS])),
            ]
        ),
    ]
)
