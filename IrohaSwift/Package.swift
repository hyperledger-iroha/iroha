// swift-tools-version:5.9
import Foundation
import PackageDescription

let packageDirectory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let bridgeRelativePath = "../dist/NoritoBridge.xcframework"
let bridgeAbsolutePath = packageDirectory.appendingPathComponent(bridgeRelativePath).standardized
let hasBridgeArtifact = FileManager.default.fileExists(atPath: bridgeAbsolutePath.path)
if !hasBridgeArtifact {
    print("notice: NoritoBridge.xcframework not found at \(bridgeAbsolutePath.path)")
}

enum BridgePolicy {
    case required
    case optional

    var swiftDefine: String {
        switch self {
        case .required:
            return "IROHASWIFT_BRIDGE_REQUIRED"
        case .optional:
            return "IROHASWIFT_BRIDGE_OPTIONAL"
        }
    }
}

let bridgePolicy: BridgePolicy = hasBridgeArtifact ? .required : .optional
if !hasBridgeArtifact {
    print("""
    warning: NoritoBridge.xcframework missing at \(bridgeAbsolutePath.path); \
    continuing with Swift-only fallback.
    """)
}

let shouldLinkBridge = hasBridgeArtifact
var targets: [Target] = []
var irohaSwiftDependencies: [Target.Dependency] = []
var testDependencies: [Target.Dependency] = ["IrohaSwift"]
var irohaSwiftLinkerSettings: [LinkerSetting] = []

if shouldLinkBridge {
    targets.append(
        .binaryTarget(
            name: "NoritoBridge",
            path: bridgeRelativePath
        )
    )
    let bridgeDependency: Target.Dependency = .target(name: "NoritoBridge", condition: .when(platforms: [.iOS, .macOS]))
    irohaSwiftDependencies.append(bridgeDependency)
    testDependencies.append(bridgeDependency)
    // Ensure static bridge object files are retained so runtime dlsym lookups resolve.
    irohaSwiftLinkerSettings.append(.unsafeFlags(["-all_load"], .when(platforms: [.iOS])))
}

var swiftSettings: [SwiftSetting] = [
    .define("IROHA_SWIFT"),
    .define("IROHASWIFT_ENABLE_SECP256K1"),
    .define("IROHASWIFT_ENABLE_MLDSA"),
    .define("IROHASWIFT_ENABLE_SM"),
    .define(bridgePolicy.swiftDefine)
]
if hasBridgeArtifact {
    swiftSettings.append(.define("IROHASWIFT_BRIDGE_PRESENT"))
}

let package = Package(
    name: "IrohaSwift",
    platforms: [
        .iOS(.v15),
        .macOS(.v12)
    ],
    products: [
        .library(
            name: "IrohaSwift",
            targets: ["IrohaSwift"])
    ],
    targets: targets + [
        .target(
            name: "IrohaSwift",
            dependencies: irohaSwiftDependencies,
            path: "Sources/IrohaSwift",
            exclude: [],
            resources: [],
            swiftSettings: swiftSettings,
            linkerSettings: irohaSwiftLinkerSettings
        ),
        .testTarget(
            name: "IrohaSwiftTests",
            dependencies: testDependencies,
            path: "Tests/IrohaSwiftTests",
            resources: [
                .process("Fixtures")
            ],
            swiftSettings: swiftSettings
        )
    ]
)
