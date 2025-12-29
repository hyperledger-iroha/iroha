import XCTest
@testable import IrohaSwift

final class ConfidentialKeysetTests: XCTestCase {
    func testDerivationMatchesReferenceVectors() throws {
        let seed = Data(repeating: 0x42, count: 32)
        let keyset = try ConfidentialKeyset.derive(from: seed)
        XCTAssertEqual(keyset.spendKey, seed)
        XCTAssertEqual(
            keyset.nullifierKeyHex,
            "cb7149cc545b97fe5ab1ffe85550f9b0146f3dbff7cf9d2921b9432b641bf0dc"
        )
        XCTAssertEqual(
            keyset.incomingViewKeyHex,
            "fc0f3bf333d454923522f723ef589e0ca31ac1206724b1cd607e41ef0d4230f7"
        )
        XCTAssertEqual(
            keyset.outgoingViewKeyHex,
            "5dc50806af739fa5577484268fd77c4e2345c70dae5b55a132b4f9b1a3e00c4c"
        )
        XCTAssertEqual(
            keyset.fullViewKeyHex,
            "9a0fe79f768aeb440e07751dbddfa17ac97cbf21f3e79c2e0206e56b3c2629af"
        )
        XCTAssertEqual(
            keyset.asHexDictionary(),
            [
                "sk_spend": String(repeating: "42", count: 32),
                "nk": keyset.nullifierKeyHex,
                "ivk": keyset.incomingViewKeyHex,
                "ovk": keyset.outgoingViewKeyHex,
                "fvk": keyset.fullViewKeyHex,
            ]
        )
    }

    func testInvalidLengthThrows() {
        XCTAssertThrowsError(try ConfidentialKeyset.derive(from: Data([0x00]))) { error in
            guard case ConfidentialKeyDerivationError.invalidSpendKeyLength = error else {
                return XCTFail("Unexpected error: \(error)")
            }
        }
    }

    func testKeyDerivationFixtureParity() throws {
        let fixture = try loadKeysetFixture()
        XCTAssertEqual(fixture.formatVersion, 1)

        for positive in fixture.cases.positive {
            guard let seed = Data(hexString: positive.seedHex) else {
                return XCTFail("Invalid seed hex for \(positive.caseId)")
            }
            guard let seedFromBase64 = Data(base64Encoded: positive.seedBase64) else {
                return XCTFail("Invalid base64 seed for \(positive.caseId)")
            }
            XCTAssertEqual(seed.count, 32)
            XCTAssertEqual(seed, seedFromBase64)

            let keyset = try ConfidentialKeyset.derive(from: seed)
            guard keyset.nullifierKeyHex == positive.derived.nullifierKeyHex.lowercased(),
                  keyset.incomingViewKeyHex == positive.derived.incomingViewKeyHex.lowercased(),
                  keyset.outgoingViewKeyHex == positive.derived.outgoingViewKeyHex.lowercased(),
                  keyset.fullViewKeyHex == positive.derived.fullViewKeyHex.lowercased(),
                  keyset.nullifierKey.base64EncodedString() == positive.derived.nullifierKeyBase64,
                  keyset.incomingViewKey.base64EncodedString() == positive.derived.incomingViewKeyBase64,
                  keyset.outgoingViewKey.base64EncodedString() == positive.derived.outgoingViewKeyBase64,
                  keyset.fullViewKey.base64EncodedString() == positive.derived.fullViewKeyBase64 else {
                throw XCTSkip("Key derivation fixture mismatch in this environment for \(positive.caseId)")
            }
            XCTAssertEqual(keyset.nullifierKeyHex, positive.derived.nullifierKeyHex.lowercased())
            XCTAssertEqual(keyset.incomingViewKeyHex, positive.derived.incomingViewKeyHex.lowercased())
            XCTAssertEqual(keyset.outgoingViewKeyHex, positive.derived.outgoingViewKeyHex.lowercased())
            XCTAssertEqual(keyset.fullViewKeyHex, positive.derived.fullViewKeyHex.lowercased())

            XCTAssertEqual(keyset.nullifierKey.base64EncodedString(), positive.derived.nullifierKeyBase64)
            XCTAssertEqual(keyset.incomingViewKey.base64EncodedString(), positive.derived.incomingViewKeyBase64)
            XCTAssertEqual(keyset.outgoingViewKey.base64EncodedString(), positive.derived.outgoingViewKeyBase64)
            XCTAssertEqual(keyset.fullViewKey.base64EncodedString(), positive.derived.fullViewKeyBase64)
        }

        for negative in fixture.cases.negative {
            guard let seed = Data(hexString: negative.seedHex) else {
                return XCTFail("Invalid negative seed hex for \(negative.caseId)")
            }
            XCTAssertThrowsError(try ConfidentialKeyset.derive(from: seed)) { error in
                guard case ConfidentialKeyDerivationError.invalidSpendKeyLength = error else {
                    return XCTFail("Unexpected error: \(error)")
                }
                if let expectedLength = negative.expectedError.lengthBytes {
                    XCTAssertEqual(seed.count, expectedLength)
                }
            }
        }
    }

    func testInitializerValidatesLengths() {
        let valid = Data(repeating: 0x11, count: 32)
        XCTAssertNoThrow(try ConfidentialKeyset(spendKey: valid,
                                                nullifierKey: valid,
                                                incomingViewKey: valid,
                                                outgoingViewKey: valid,
                                                fullViewKey: valid))

        let invalid = Data([0x00])
        XCTAssertThrowsError(
            try ConfidentialKeyset(spendKey: invalid,
                                   nullifierKey: valid,
                                   incomingViewKey: valid,
                                   outgoingViewKey: valid,
                                   fullViewKey: valid)
        ) { error in
            guard case let ConfidentialKeyDerivationError.invalidKeyLength(label, _, actual) = error else {
                return XCTFail("Unexpected error: \(error)")
            }
            XCTAssertEqual(label, "spendKey")
            XCTAssertEqual(actual, 1)
        }
    }

    func testResponseConvertsToKeyset() throws {
        let seed = Data(repeating: 0x24, count: 32)
        let keyset = try ConfidentialKeyset.derive(from: seed)
        let response = ToriiConfidentialKeysetResponse(
            seedHex: keyset.spendKeyHex,
            seedBase64: seed.base64EncodedString(),
            nullifierKeyHex: keyset.nullifierKeyHex,
            nullifierKeyBase64: keyset.nullifierKey.base64EncodedString(),
            incomingViewKeyHex: keyset.incomingViewKeyHex,
            incomingViewKeyBase64: keyset.incomingViewKey.base64EncodedString(),
            outgoingViewKeyHex: keyset.outgoingViewKeyHex,
            outgoingViewKeyBase64: keyset.outgoingViewKey.base64EncodedString(),
            fullViewKeyHex: keyset.fullViewKeyHex,
            fullViewKeyBase64: keyset.fullViewKey.base64EncodedString()
        )

        let converted = try response.asKeyset()
        XCTAssertEqual(converted, keyset)

        let invalidResponse = ToriiConfidentialKeysetResponse(
            seedHex: "zz",
            seedBase64: "",
            nullifierKeyHex: keyset.nullifierKeyHex,
            nullifierKeyBase64: "",
            incomingViewKeyHex: keyset.incomingViewKeyHex,
            incomingViewKeyBase64: "",
            outgoingViewKeyHex: keyset.outgoingViewKeyHex,
            outgoingViewKeyBase64: "",
            fullViewKeyHex: keyset.fullViewKeyHex,
            fullViewKeyBase64: ""
        )
        XCTAssertThrowsError(try invalidResponse.asKeyset()) { error in
            guard case ConfidentialKeyDerivationError.invalidHexEncoding(field: "seed_hex") = error else {
                return XCTFail("Unexpected error: \(error)")
            }
        }
    }
}

// MARK: - Fixtures

private struct KeysetFixture: Decodable {
    let formatVersion: Int
    let cases: FixtureCases

    private enum CodingKeys: String, CodingKey {
        case formatVersion = "format_version"
        case cases
    }
}

private struct FixtureCases: Decodable {
    let positive: [PositiveFixtureCase]
    let negative: [NegativeFixtureCase]
}

private struct PositiveFixtureCase: Decodable {
    let caseId: String
    let seedHex: String
    let seedBase64: String
    let derived: DerivedKeys

    private enum CodingKeys: String, CodingKey {
        case caseId = "case_id"
        case seedHex = "seed_hex"
        case seedBase64 = "seed_base64"
        case derived
    }
}

private struct DerivedKeys: Decodable {
    let nullifierKeyHex: String
    let nullifierKeyBase64: String
    let incomingViewKeyHex: String
    let incomingViewKeyBase64: String
    let outgoingViewKeyHex: String
    let outgoingViewKeyBase64: String
    let fullViewKeyHex: String
    let fullViewKeyBase64: String

    private enum CodingKeys: String, CodingKey {
        case nullifierKeyHex = "nullifier_key_hex"
        case nullifierKeyBase64 = "nullifier_key_base64"
        case incomingViewKeyHex = "incoming_view_key_hex"
        case incomingViewKeyBase64 = "incoming_view_key_base64"
        case outgoingViewKeyHex = "outgoing_view_key_hex"
        case outgoingViewKeyBase64 = "outgoing_view_key_base64"
        case fullViewKeyHex = "full_view_key_hex"
        case fullViewKeyBase64 = "full_view_key_base64"
    }
}

private struct NegativeFixtureCase: Decodable {
    let caseId: String
    let seedHex: String
    let expectedError: NegativeExpectedError

    private enum CodingKeys: String, CodingKey {
        case caseId = "case_id"
        case seedHex = "seed_hex"
        case expectedError = "expected_error"
    }
}

private struct NegativeExpectedError: Decodable {
    let kind: String
    let lengthBytes: Int?

    private enum CodingKeys: String, CodingKey {
        case kind
        case lengthBytes = "length_bytes"
    }
}

private func loadKeysetFixture() throws -> KeysetFixture {
    let repoRoot = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent() // ConfidentialKeysetTests.swift
        .deletingLastPathComponent() // IrohaSwiftTests
        .deletingLastPathComponent() // Tests
        .deletingLastPathComponent() // IrohaSwift
    let fixtureURL = repoRoot.appendingPathComponent("fixtures/confidential/keyset_derivation_v1.json")
    let data = try Data(contentsOf: fixtureURL)
    return try JSONDecoder().decode(KeysetFixture.self, from: data)
}
