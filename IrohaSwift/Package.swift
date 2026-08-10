// swift-tools-version:5.9
import Foundation
import PackageDescription
#if canImport(Darwin)
import Darwin
#elseif canImport(Glibc)
import Glibc
#endif

let packageDirectory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let bridgeRelativePath = "../dist/NoritoBridge.xcframework"
let requiredBridgeAbiVersion = 22
let repositoryDirectory = packageDirectory.deletingLastPathComponent().standardizedFileURL
let configuredArtifactDirectory = ProcessInfo.processInfo.environment[
    "MOBILE_SDK_APPLE_ARTIFACT_DIR"
]
let requireExternalArtifactInput = ProcessInfo.processInfo.environment[
    "MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT"
]
guard requireExternalArtifactInput == nil
    || requireExternalArtifactInput == "0"
    || requireExternalArtifactInput == "1"
else {
    fatalError(
        "error: MOBILE_SDK_REQUIRE_EXTERNAL_APPLE_ARTIFACT must be exactly 0 or 1."
    )
}
let requireExternalArtifact = requireExternalArtifactInput == "1"
if requireExternalArtifact, configuredArtifactDirectory == nil {
    fatalError(
        """
        error: reviewed Release builds require MOBILE_SDK_APPLE_ARTIFACT_DIR \
        to identify an authenticated artifact directory outside the Iroha source tree.
        """
    )
}

func relativePath(from base: URL, to destination: URL) -> String {
    let baseComponents = base.pathComponents
    let destinationComponents = destination.pathComponents
    var commonComponentCount = 0
    while
        commonComponentCount < baseComponents.count,
        commonComponentCount < destinationComponents.count,
        baseComponents[commonComponentCount] == destinationComponents[commonComponentCount]
    {
        commonComponentCount += 1
    }
    let parentComponents = Array(
        repeating: "..",
        count: baseComponents.count - commonComponentCount
    )
    let childComponents = destinationComponents.dropFirst(commonComponentCount)
    return (parentComponents + childComponents).joined(separator: "/")
}

func canonicalExistingFilesystemPath(_ path: String) -> String? {
    path.withCString { encodedPath in
        guard let resolvedPath = realpath(encodedPath, nil) else {
            return nil
        }
        defer { free(resolvedPath) }
        return String(cString: resolvedPath)
    }
}

let bridgeAbsolutePath: URL
let bridgeTargetPath: String
if let configuredArtifactDirectory {
    guard configuredArtifactDirectory.hasPrefix("/") else {
        fatalError("error: MOBILE_SDK_APPLE_ARTIFACT_DIR must be an absolute path.")
    }
    guard
        let canonicalArtifactDirectory =
            canonicalExistingFilesystemPath(configuredArtifactDirectory),
        canonicalArtifactDirectory == configuredArtifactDirectory
    else {
        fatalError(
            """
            error: MOBILE_SDK_APPLE_ARTIFACT_DIR must be an existing canonical \
            path that does not traverse a symbolic link.
            """
        )
    }
    let resolvedURL = URL(
        fileURLWithPath: canonicalArtifactDirectory,
        isDirectory: true
    )
    guard
        resolvedURL.path != repositoryDirectory.path,
        !resolvedURL.path.hasPrefix(repositoryDirectory.path + "/")
    else {
        fatalError(
            "error: MOBILE_SDK_APPLE_ARTIFACT_DIR must be outside the reviewed Iroha source tree."
        )
    }
    bridgeAbsolutePath = resolvedURL
        .appendingPathComponent("NoritoBridge.xcframework", isDirectory: true)
    bridgeTargetPath = relativePath(
        from: packageDirectory,
        to: bridgeAbsolutePath
    )
} else {
    bridgeAbsolutePath = packageDirectory
        .appendingPathComponent(bridgeRelativePath)
        .standardizedFileURL
    bridgeTargetPath = bridgeRelativePath
}

func validateBridgeArtifact(at artifactRoot: URL) -> String? {
    guard FileManager.default.fileExists(atPath: artifactRoot.path) else {
        return """
        error: NoritoBridge.xcframework is required at \(artifactRoot.path). \
        Set MOBILE_SDK_APPLE_ARTIFACT_DIR to the external directory containing \
        NoritoBridge.xcframework before building a reviewed source closure.
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

    let artifactManifestURL = artifactRoot.appendingPathComponent(
        "NoritoBridge.artifacts.json"
    )
    guard
        let manifestData = try? Data(contentsOf: artifactManifestURL),
        let manifest = try? JSONSerialization.jsonObject(with: manifestData)
            as? [String: Any],
        let bridgeAbiVersion = manifest["native_bridge_abi_version"] as? Int
    else {
        return "error: NoritoBridge.xcframework is missing readable ABI-bound artifact metadata."
    }
    guard bridgeAbiVersion == requiredBridgeAbiVersion else {
        return "error: NoritoBridge.xcframework requires exact native bridge ABI \(requiredBridgeAbiVersion); found \(bridgeAbiVersion)."
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
        path: bridgeTargetPath
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

// Keep Google's Apple Nearby implementation deterministic for fresh Xcode
// checkouts. Nearby's transitive Abseil branch is additionally locked by the
// checked-in Package.resolved file.
let packageDependencies: [Package.Dependency] = [
    .package(
        url: "https://github.com/google/nearby.git",
        revision: "53568fe88281d4408e48e3ebec7d8560bed7077d"
    ),
    .package(
        url: "https://github.com/firebase/boringssl-SwiftPM.git",
        exact: "0.7.2"
    )
]
let mobileTransportDependencies: [Target.Dependency] = [
    "IrohaSwift",
    .product(
        name: "NearbyConnections",
        package: "nearby",
        condition: .when(platforms: [.iOS, .macOS])
    )
]
let mobileTransportTestDependencies: [Target.Dependency] =
    testDependencies + ["IrohaSwiftMobileTransports"]

let package = Package(
    name: "IrohaSwift",
    platforms: [
        .iOS(.v15),
        .macOS(.v12)
    ],
    products: [
        .library(
            name: "IrohaSwift",
            targets: ["IrohaSwift"]),
        .library(
            name: "IrohaSwiftMobileTransports",
            targets: ["IrohaSwiftMobileTransports"]),
        .library(
            name: "IrohaSwiftTransferUI",
            targets: ["IrohaSwiftTransferUI"])
    ],
    dependencies: packageDependencies,
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
        .target(
            name: "IrohaSwiftMobileTransports",
            dependencies: mobileTransportDependencies,
            path: "Sources/IrohaSwiftMobileTransports",
            swiftSettings: swiftSettings
        ),
        .target(
            name: "IrohaSwiftTransferUI",
            dependencies: ["IrohaSwift"],
            path: "Sources/IrohaSwiftTransferUI",
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
        ),
        .testTarget(
            name: "IrohaSwiftMobileTransportsTests",
            dependencies: mobileTransportTestDependencies,
            path: "Tests/IrohaSwiftMobileTransportsTests",
            swiftSettings: swiftSettings
        )
    ]
)
