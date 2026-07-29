import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class PrivacyNativeBridgeTests: XCTestCase {
    private static let matrix = loadExact12Matrix()
    private var protocolRows: [[String]] { Self.matrix.filter { $0[0] == "protocol" } }
    private var typedEnvelopeRows: [[String]] {
        Self.matrix.filter { $0[0] == "typed-envelope" }
    }
    private var retired: [String] {
        Self.matrix.filter { $0[0] == "retired" }.map { $0[1] }
    }
    private var expected: [String] { protocolRows.map { $0[2] } }

    func testExactClosedRegistryIsStable() throws {
        XCTAssertEqual(PrivacyNativeBridge.requiredBridgeABIVersion, 21)
        XCTAssertEqual(PrivacyNativeBridge.protocolsV1.map(\.rawValue), expected)
        XCTAssertEqual(PrivacyNativeBridge.protocolsV1.count, 12)
        for (index, label) in expected.enumerated() {
            XCTAssertEqual(
                try PrivacyProtocolIdV1(canonicalLabel: label),
                PrivacyNativeBridge.protocolsV1[index]
            )
        }
    }

    func testSharedExact12MatrixBindsRoutesAndTypedEnvelopeDigests() {
        XCTAssertEqual(
            Set(Self.matrix.map { $0[0] }),
            Set(["matrix-version", "registry-sha256", "protocol", "typed-envelope", "retired"])
        )
        XCTAssertEqual(
            Self.matrix.filter { $0[0] == "matrix-version" },
            [["matrix-version", "1"]]
        )
        XCTAssertEqual(protocolRows.map { $0[1] }, (0..<12).map(String.init))
        XCTAssertEqual(Set(expected).count, 12)
        let registryPreimage = expected.map { "\($0)\n" }.joined()
        let registryDigest = SHA256.hash(data: Data(registryPreimage.utf8))
            .map { String(format: "%02x", $0) }
            .joined()
        XCTAssertEqual(
            Self.matrix.filter { $0[0] == "registry-sha256" },
            [["registry-sha256", registryDigest]]
        )
        XCTAssertEqual(
            typedEnvelopeRows.map { Array($0[1..<4]) },
            protocolRows.map { Array($0[2..<5]) }
        )
        XCTAssertEqual(typedEnvelopeRows.count, 12)
        for row in typedEnvelopeRows {
            XCTAssertEqual(row.count, 6)
            for digest in row[4...] {
                XCTAssertNotNil(digest.range(of: "^[0-9a-f]{64}$", options: .regularExpression))
                XCTAssertNotEqual(digest, String(repeating: "0", count: 64))
            }
        }
        XCTAssertEqual(Set(retired).count, retired.count)
        XCTAssertTrue(Set(retired).isDisjoint(with: expected))
    }

    func testAliasesAndNonCanonicalSpellingsAreRejected() {
        for rejected in retired + [
            "iroha-zk-ams-v1 ",
            "Iroha-Zk-Ams-V1",
            "",
            "unknown-privacy-protocol-v1",
        ] {
            XCTAssertThrowsError(try PrivacyProtocolIdV1(canonicalLabel: rejected)) {
                XCTAssertEqual($0 as? PrivacyCapabilityBridgeError, .unknownProtocol)
            }
        }
    }

    private static func loadExact12Matrix() -> [[String]] {
        var directory = URL(
            fileURLWithPath: FileManager.default.currentDirectoryPath,
            isDirectory: true
        )
        while directory.path != "/" {
            let candidate = directory.appendingPathComponent(
                "fixtures/privacy/exact12_v1.tsv"
            )
            if FileManager.default.fileExists(atPath: candidate.path) {
                guard
                    let text = try? String(contentsOf: candidate, encoding: .utf8),
                    text.hasSuffix("\n"),
                    !text.contains("\r"),
                    !text.dropLast().split(separator: "\n", omittingEmptySubsequences: false)
                        .contains(where: { $0.isEmpty })
                else {
                    fatalError("exact12 fixture is not canonical LF text")
                }
                return text
                    .split(separator: "\n")
                    .filter { !$0.isEmpty && !$0.hasPrefix("#") }
                    .map { $0.split(separator: "\t", omittingEmptySubsequences: false).map(String.init) }
            }
            directory.deleteLastPathComponent()
        }
        fatalError("cannot locate fixtures/privacy/exact12_v1.tsv")
    }

    func testSharedTypedValidatorStatusContractIsStable() {
        XCTAssertEqual(PrivacyNativeBridge.nativeArchiveMaximumBytes, 256 * 1024)
        XCTAssertEqual(
            PrivacyCapabilityValidationStatusV1.allCases.map(\.rawValue),
            Array(0...8)
        )
    }
}
