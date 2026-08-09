import XCTest
@testable import IrohaSwift

final class ConfidentialWalletFixturesTests: XCTestCase {
    func testRetiredGenericConfidentialNamesAndNativeDiscriminantsAreAbsent() throws {
        let packageRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let variants = [
            ["Shi", "eld"],
            ["Zk", "Transfer"],
            ["Un", "shield"],
        ].map { $0.joined() }

        for filename in ["TxBuilder.swift", "TransactionEncoder.swift", "NativeBridge.swift"] {
            let path = packageRoot
                .appendingPathComponent("Sources/IrohaSwift", isDirectory: true)
                .appendingPathComponent(filename)
            let source = try String(contentsOf: path, encoding: .utf8)
            for variant in variants {
                let snakeCase = variant.enumerated().map { offset, character in
                    let prefix = offset > 0 && character.isUppercase ? "_" : ""
                    return prefix + String(character).lowercased()
                }.joined()
                XCTAssertFalse(source.contains(variant + "Request"), "\(filename): \(variant)")
                XCTAssertFalse(source.contains("build" + variant), "\(filename): \(variant)")
                XCTAssertFalse(source.contains("encode" + variant + "("), "\(filename): \(variant)")
                XCTAssertFalse(
                    source.contains("connect_norito_encode_" + snakeCase + "_signed_transaction"),
                    "\(filename): \(variant)"
                )
            }
        }

        let retiredFixture = packageRoot
            .deletingLastPathComponent()
            .appendingPathComponent("fixtures/confidential/wallet_flows_v1.json")
        XCTAssertFalse(FileManager.default.fileExists(atPath: retiredFixture.path))
    }
}
