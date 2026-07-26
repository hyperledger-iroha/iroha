// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "connect-cli",
    platforms: [.macOS(.v12)],
    products: [
        .executable(name: "connect-cli", targets: ["connect-cli"])
    ],
    dependencies: [
        .package(name: "IrohaSwift", path: "../../IrohaSwift")
    ],
    targets: [
        .executableTarget(
            name: "connect-cli",
            dependencies: [
                .product(name: "IrohaSwift", package: "IrohaSwift")
            ],
            path: "Sources"
        ),
        .testTarget(
            name: "connect-cli-tests",
            dependencies: ["connect-cli"],
            path: "Tests"
        ),
    ]
)
