// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "ConnectMinimalApp",
    platforms: [
        .iOS(.v15)
    ],
    products: [
        .executable(name: "ConnectMinimalApp", targets: ["ConnectMinimalApp"])
    ],
    dependencies: [
        .package(name: "IrohaSwift", path: "../../IrohaSwift")
    ],
    targets: [
        .executableTarget(
            name: "ConnectMinimalApp",
            dependencies: [
                .product(name: "IrohaSwift", package: "IrohaSwift")
            ],
            path: "Sources"
        )
    ]
)
