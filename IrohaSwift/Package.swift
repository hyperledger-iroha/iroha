// swift-tools-version:5.9
import Foundation
import PackageDescription

let packageDirectory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let bridgeRelativePath = "../dist/NoritoBridge.xcframework"
let bridgeAbsolutePath = packageDirectory.appendingPathComponent(bridgeRelativePath).standardized

func validateBridgeArtifact(at artifactRoot: URL) -> String? {
    guard FileManager.default.fileExists(atPath: artifactRoot.path) else {
        return """
        error: NoritoBridge.xcframework is required at \(artifactRoot.path). \
        Materialize it from ../dist/NoritoBridge.xcframework.zip before building.
        """
    }

    let infoURL = artifactRoot.appendingPathComponent("Info.plist")
    guard
        let data = try? Data(contentsOf: infoURL),
        let plist = try? PropertyListSerialization.propertyList(from: data, options: [], format: nil),
        let dictionary = plist as? [String: Any],
        let libraries = dictionary["AvailableLibraries"] as? [[String: Any]],
        !libraries.isEmpty
    else {
        return "error: NoritoBridge.xcframework at \(artifactRoot.path) has unreadable metadata."
    }

    for library in libraries {
        let identifier = library["LibraryIdentifier"] as? String ?? "<unknown>"
        let relativePaths = ["BinaryPath", "LibraryPath"].compactMap { library[$0] as? String }
        guard !relativePaths.isEmpty else {
            return "error: NoritoBridge.xcframework slice \(identifier) is missing BinaryPath/LibraryPath metadata."
        }

        for relativePath in relativePaths {
            let referencedURL = artifactRoot
                .appendingPathComponent(identifier, isDirectory: true)
                .appendingPathComponent(relativePath)
            guard FileManager.default.fileExists(atPath: referencedURL.path) else {
                return "error: NoritoBridge.xcframework slice \(identifier) is missing \(relativePath)."
            }
        }
    }

    return nil
}

if let bridgeArtifactError = validateBridgeArtifact(at: bridgeAbsolutePath) {
    fatalError(bridgeArtifactError)
}
var targets: [Target] = []
var irohaSwiftDependencies: [Target.Dependency] = []
var testDependencies: [Target.Dependency] = ["IrohaSwift"]
var irohaSwiftLinkerSettings: [LinkerSetting] = []

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
irohaSwiftLinkerSettings.append(.unsafeFlags(["-Xlinker", "-all_load"], .when(platforms: [.iOS, .macOS])))

var swiftSettings: [SwiftSetting] = [
    .define("IROHA_SWIFT"),
    .define("IROHASWIFT_ENABLE_SECP256K1"),
    .define("IROHASWIFT_ENABLE_MLDSA"),
    .define("IROHASWIFT_ENABLE_BLS"),
    .define("IROHASWIFT_ENABLE_GOST"),
    .define("IROHASWIFT_ENABLE_SM"),
    .define("IROHASWIFT_BRIDGE_REQUIRED"),
    .define("IROHASWIFT_BRIDGE_PRESENT")
]

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
