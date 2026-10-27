// swift-tools-version:5.7
// Package the pinned, unmodified upstream BoringSSL source for Google Nearby.
import Foundation
import PackageDescription

struct SourceGroup: Decodable {
    let srcs: [String]
    let asm: [String]?
}
let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
let sourceData = try Data(contentsOf: root.appendingPathComponent("gen/sources.json"))
let sourceGroups = try JSONDecoder().decode([String: SourceGroup].self, from: sourceData)
let sources = try ["bcm", "crypto", "ssl"].flatMap { name -> [String] in
    guard let group = sourceGroups[name] else {
        throw CocoaError(.fileReadCorruptFile)
    }
    return group.srcs + (group.asm ?? [])
}.sorted()

let package = Package(
    name: "BoringSSL-GRPC",
    products: [.library(name: "openssl_grpc", targets: ["openssl_grpc"])],
    targets: [
        .target(
            name: "openssl_grpc",
            path: ".",
            exclude: ["verify_source.py", "IROHA-PATCH.md", "UPSTREAM.json", "gen/sources.json", "LICENSE", "third_party/fiat/LICENSE"],
            sources: sources,
            resources: [.copy("PrivacyInfo.xcprivacy")],
            publicHeadersPath: "include",
            cSettings: [
                .define("BORINGSSL_IMPLEMENTATION"),
                .headerSearchPath("."),
            ],
            cxxSettings: [
                .unsafeFlags(["-fno-strict-aliasing", "-fno-exceptions", "-fno-rtti"]),
            ]
        ),
    ],
    cLanguageStandard: .c11,
    cxxLanguageStandard: .cxx17
)
