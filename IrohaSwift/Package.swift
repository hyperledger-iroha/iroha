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
    case disabled

    static func parse(_ raw: String?) -> Self {
        guard let raw else { return .required }
        let normalized = raw.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if ["0", "false", "off", "disabled", "disable", "no"].contains(normalized) {
            return .disabled
        }
        if normalized == "optional" {
            return .optional
        }
        return .required
    }

    var swiftDefine: String {
        switch self {
        case .required:
            return "IROHASWIFT_BRIDGE_REQUIRED"
        case .optional:
            return "IROHASWIFT_BRIDGE_OPTIONAL"
        case .disabled:
            return "IROHASWIFT_BRIDGE_DISABLED"
        }
    }
}

let bridgePolicy = BridgePolicy.parse(ProcessInfo.processInfo.environment["IROHASWIFT_USE_BRIDGE"])
let useBridge = bridgePolicy != .disabled

if bridgePolicy == .required && !hasBridgeArtifact {
    fatalError("""
    NoritoBridge.xcframework not found at \(bridgeAbsolutePath.path).
    The Swift SDK requires the bridge by default. Place the xcframework under \(bridgeRelativePath) \
    or set IROHASWIFT_USE_BRIDGE=optional (or IROHASWIFT_USE_BRIDGE=0) to build with the Swift-only fallback.
    """)
}
if bridgePolicy == .optional && !hasBridgeArtifact {
    print("""
    warning: NoritoBridge.xcframework missing at \(bridgeAbsolutePath.path); \
    continuing with Swift-only fallback because IROHASWIFT_USE_BRIDGE=optional (set IROHASWIFT_USE_BRIDGE=0 to suppress).
    """)
}

let shouldLinkBridge = hasBridgeArtifact && useBridge
var targets: [Target] = []
var irohaSwiftDependencies: [Target.Dependency] = []
var testDependencies: [Target.Dependency] = ["IrohaSwift"]

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
            swiftSettings: swiftSettings
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
