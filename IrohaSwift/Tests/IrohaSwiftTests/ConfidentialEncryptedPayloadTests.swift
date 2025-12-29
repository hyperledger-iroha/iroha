import XCTest
@testable import IrohaSwift

final class ConfidentialEncryptedPayloadTests: XCTestCase {
    func testSerializedPayloadMatchesExpectedLayout() throws {
        let ephemeral = Data(repeating: 0x11, count: 32)
        let nonce = Data(repeating: 0x22, count: 24)
        let ciphertext = Data([0xAA, 0xBB, 0xCC])
        let payload = try ConfidentialEncryptedPayload(ephemeralPublicKey: ephemeral,
                                                       nonce: nonce,
                                                       ciphertext: ciphertext)

        let encoded = try payload.serializedPayload()
        var expected = Data()
        expected.append(ConfidentialEncryptedPayload.v1)
        expected.append(ephemeral)
        expected.append(nonce)
        expected.append(0x03) // ciphertext length varint
        expected.append(contentsOf: [0xAA, 0xBB, 0xCC])

        XCTAssertEqual(encoded, expected)
    }

    func testRoundTripSerialization() throws {
        let ephemeral = Data((0..<32).map { UInt8($0) })
        let nonce = Data((0..<24).map { UInt8(0xFF - $0) })
        let ciphertext = Data((0..<40).map { UInt8($0 ^ 0x5A) })
        let original = try ConfidentialEncryptedPayload(ephemeralPublicKey: ephemeral,
                                                        nonce: nonce,
                                                        ciphertext: ciphertext)

        let encoded = try original.serializedPayload()
        let decoded = try ConfidentialEncryptedPayload.deserialize(from: encoded)
        XCTAssertEqual(decoded, original)
    }

    func testDeserializeRejectsTrailingBytes() throws {
        let payload = try ConfidentialEncryptedPayload(ephemeralPublicKey: Data(repeating: 0x01, count: 32),
                                                       nonce: Data(repeating: 0x02, count: 24),
                                                       ciphertext: Data([0x00]))
        var encoded = try payload.serializedPayload()
        encoded.append(0xFF)

        XCTAssertThrowsError(try ConfidentialEncryptedPayload.deserialize(from: encoded)) { error in
            guard case ConfidentialEncryptedPayloadError.trailingBytes(1) = error else {
                return XCTFail("Unexpected error: \(error)")
            }
        }
    }

    func testInitializerValidatesLengths() {
        XCTAssertThrowsError(
            try ConfidentialEncryptedPayload(ephemeralPublicKey: Data([0x00]),
                                             nonce: Data(repeating: 0x00, count: 24),
                                             ciphertext: Data())
        ) { error in
            guard case ConfidentialEncryptedPayloadError.invalidEphemeralKeyLength(1) = error else {
                return XCTFail("Unexpected error: \(error)")
            }
        }

        XCTAssertThrowsError(
            try ConfidentialEncryptedPayload(ephemeralPublicKey: Data(repeating: 0x00, count: 32),
                                             nonce: Data([0x00]),
                                             ciphertext: Data())
        ) { error in
            guard case ConfidentialEncryptedPayloadError.invalidNonceLength(1) = error else {
                return XCTFail("Unexpected error: \(error)")
            }
        }
    }

    func testEncryptedPayloadFixture() throws {
        let fixtureURL = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // ConfidentialEncryptedPayloadTests.swift
            .deletingLastPathComponent() // IrohaSwiftTests
            .deletingLastPathComponent() // Tests
            .deletingLastPathComponent() // IrohaSwift
            .appendingPathComponent("fixtures/confidential/encrypted_payload_v1.json")

        let data = try Data(contentsOf: fixtureURL)
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        let fixture = try decoder.decode(EncryptedPayloadFixture.self, from: data)
        XCTAssertEqual(fixture.formatVersion, 1)

        for vector in fixture.cases.positive {
            try assertPositiveFixture(vector)
        }

        for vector in fixture.cases.negative {
            assertNegativeFixture(vector)
        }
    }

    private func assertPositiveFixture(_ vector: EncryptedPayloadPositiveCase) throws {
        guard let ephemeral = Data(hexString: vector.ephemeralPublicKeyHex),
              ephemeral.count == 32 else {
            return XCTFail("\(vector.caseId): invalid ephemeral key hex")
        }
        guard let nonce = Data(hexString: vector.nonceHex),
              nonce.count == 24 else {
            return XCTFail("\(vector.caseId): invalid nonce hex")
        }
        guard let ciphertext = Data(hexString: vector.ciphertextHex) else {
            return XCTFail("\(vector.caseId): invalid ciphertext hex")
        }

        let payload = try ConfidentialEncryptedPayload(
            version: vector.version,
            ephemeralPublicKey: ephemeral,
            nonce: nonce,
            ciphertext: ciphertext
        )

        let serialized = try payload.serializedPayload()
        XCTAssertEqual(
            serialized.hexLowercased(),
            vector.serializedHex.lowercased(),
            "\(vector.caseId): serialized bytes mismatch"
        )

        let decoded = try ConfidentialEncryptedPayload.deserialize(from: serialized)
        XCTAssertEqual(decoded, payload, "\(vector.caseId): round-trip mismatch")
    }

    private func assertNegativeFixture(_ vector: EncryptedPayloadNegativeCase) {
        guard let bytes = Data(hexString: vector.serializedHex) else {
            return XCTFail("\(vector.caseId): invalid serialized hex")
        }

        switch vector.expectedError.mode {
        case "unsupported_version":
            XCTAssertThrowsError(try ConfidentialEncryptedPayload.deserialize(from: bytes),
                                 "\(vector.caseId): expected unsupported version") { error in
                guard case let ConfidentialEncryptedPayloadError.unsupportedVersion(found) = error else {
                    return XCTFail("\(vector.caseId): unexpected error \(error)")
                }
                if let expectedVersion = vector.expectedError.version {
                    XCTAssertEqual(found, expectedVersion, "\(vector.caseId): version mismatch")
                }
            }
        case "decode_error":
            XCTAssertThrowsError(try ConfidentialEncryptedPayload.deserialize(from: bytes),
                                 "\(vector.caseId): expected decode error") { error in
                verify(confidentialError: error, matches: vector.expectedError, caseId: vector.caseId)
            }
        default:
            XCTFail("\(vector.caseId): unsupported error mode \(vector.expectedError.mode)")
        }
    }

    private func verify(confidentialError error: Error,
                        matches expected: EncryptedPayloadExpectedError,
                        caseId: String) {
        guard let payloadError = error as? ConfidentialEncryptedPayloadError else {
            return XCTFail("\(caseId): unexpected error type \(error)")
        }
        switch expected.kind {
        case "UnsupportedVersion":
            if case let .unsupportedVersion(found) = payloadError {
                if let expectedVersion = expected.version {
                    XCTAssertEqual(found, expectedVersion, "\(caseId): version mismatch")
                }
            } else {
                XCTFail("\(caseId): expected unsupported version, got \(payloadError)")
            }
        case "TruncatedPayload":
            XCTAssertEqual(payloadError, .truncatedPayload, "\(caseId): expected truncated payload")
        default:
            XCTFail("\(caseId): unhandled expected error kind \(expected.kind)")
        }
    }
}

private struct EncryptedPayloadFixture: Decodable {
    let formatVersion: Int
    let cases: EncryptedPayloadCaseSets
}

private struct EncryptedPayloadCaseSets: Decodable {
    let positive: [EncryptedPayloadPositiveCase]
    let negative: [EncryptedPayloadNegativeCase]
}

private struct EncryptedPayloadPositiveCase: Decodable {
    let caseId: String
    let version: UInt8
    let ephemeralPublicKeyHex: String
    let nonceHex: String
    let ciphertextHex: String
    let serializedHex: String
}

private struct EncryptedPayloadNegativeCase: Decodable {
    let caseId: String
    let serializedHex: String
    let expectedError: EncryptedPayloadExpectedError
}

private struct EncryptedPayloadExpectedError: Decodable {
    let mode: String
    let kind: String
    let version: UInt8?
}

private extension Data {
    func hexLowercased() -> String {
        map { String(format: "%02x", $0) }.joined()
    }
}
