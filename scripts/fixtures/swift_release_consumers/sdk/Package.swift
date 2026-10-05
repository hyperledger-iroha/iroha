// swift-tools-version:5.9
import Foundation
import PackageDescription

guard let sdkPath = ProcessInfo.processInfo.environment["IROHA_RELEASE_CONSUMER_SDK_PATH"] else {
    fatalError("Run scripts/check_swift_release_consumers.py to select the reviewed SDK.")
}

let package = Package(
    name: "IrohaNativeConsumer",
    platforms: [.macOS(.v12)],
    dependencies: [
        .package(name: "IrohaSwift", path: sdkPath)
    ],
    targets: [
        .executableTarget(
            name: "IrohaNativeConsumer",
            dependencies: [.product(name: "IrohaSwift", package: "IrohaSwift")]
        )
    ]
)
