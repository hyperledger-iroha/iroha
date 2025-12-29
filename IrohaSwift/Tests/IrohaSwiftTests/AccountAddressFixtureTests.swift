import XCTest
@testable import IrohaSwift

final class AccountAddressFixtureTests: XCTestCase {
    func testPositiveVectorsRoundTrip() throws {
        let vectors = try AccountAddressVectors.load()
        for vector in vectors.cases.positive {
            try assertPositive(vector)
        }
    }

    func testNegativeVectorsReject() throws {
        let vectors = try AccountAddressVectors.load()
        for vector in vectors.cases.negative {
            assertNegative(vector)
        }
    }

    // MARK: - Helpers

    private func assertPositive(_ vector: AccountAddressPositiveCase) throws {
        let address = try AccountAddress.fromCanonicalHex(vector.encodings.canonicalHex)
        XCTAssertEqual(
            try address.canonicalHex().lowercased(),
            vector.encodings.canonicalHex.lowercased(),
            "\(vector.caseId): canonical hex mismatch"
        )

        let ih58 = try address.toIH58(networkPrefix: vector.encodings.ih58.prefix)
        XCTAssertEqual(ih58, vector.encodings.ih58.string, "\(vector.caseId): IH58 mismatch")

        let compressed = try address.toCompressedSora()
        XCTAssertEqual(
            try AccountAddress.fromCompressedSora(compressed).canonicalHex(),
            vector.encodings.canonicalHex.lowercased().hasPrefix("0x") ? vector.encodings.canonicalHex : "0x\(vector.encodings.canonicalHex)",
            "\(vector.caseId): compressed canonical mismatch"
        )
        XCTAssertEqual(compressed, vector.encodings.compressed, "\(vector.caseId): compressed mismatch")

        let compressedFull = try address.toCompressedSoraFullWidth()
        if let expectedFull = vector.encodings.compressedFullwidth {
            let decodedExpected = try AccountAddress.fromCompressedSora(expectedFull)
            XCTAssertEqual(
                try decodedExpected.canonicalHex().lowercased(),
                try address.canonicalHex().lowercased(),
                "\(vector.caseId): compressed full-width canonical mismatch"
            )
            XCTAssertEqual(
                compressedFull.applyingTransform(.fullwidthToHalfwidth, reverse: false) ?? compressedFull,
                expectedFull.applyingTransform(.fullwidthToHalfwidth, reverse: false) ?? expectedFull,
                "\(vector.caseId): compressed full-width mismatch"
            )
        } else {
            XCTAssertEqual(compressedFull, compressed, "\(vector.caseId): compressed fallback mismatch")
        }

        // Parse entry points should lead back to the same canonical bytes.
        let parsedIH58 = try AccountAddress.parseAny(vector.encodings.ih58.string,
                                                     expectedPrefix: vector.encodings.ih58.prefix)
        XCTAssertEqual(parsedIH58.1, .ih58, "\(vector.caseId): IH58 parse format mismatch")
        XCTAssertEqual(
            try parsedIH58.0.canonicalHex(),
            try address.canonicalHex(),
            "\(vector.caseId): IH58 parse canonical mismatch"
        )

        let parsedCompressed = try AccountAddress.parseAny(vector.encodings.compressed)
        XCTAssertEqual(parsedCompressed.1, .compressed, "\(vector.caseId): compressed parse format mismatch")
        XCTAssertEqual(
            try parsedCompressed.0.canonicalHex(),
            try address.canonicalHex(),
            "\(vector.caseId): compressed parse canonical mismatch"
        )

        if let info = try address.multisigPolicyInfo(), vector.category == "multisig" {
            try assertMultisig(vector, info: info)
        } else if vector.category == "multisig" {
            throw XCTSkip("\(vector.caseId): multisig controller unavailable in this environment")
        }
    }

    private func assertMultisig(_ vector: AccountAddressPositiveCase, info: AccountAddress.MultisigPolicyInfo) throws {
        let controller = try XCTUnwrap(vector.controller, "\(vector.caseId): missing controller for multisig case")
        let version = try XCTUnwrap(controller.version, "\(vector.caseId): missing controller version")
        let threshold = try XCTUnwrap(controller.threshold, "\(vector.caseId): missing multisig threshold")
        let ctap2 = try XCTUnwrap(controller.ctap2CborHex, "\(vector.caseId): missing CTAP2 payload")
        let digest = try XCTUnwrap(controller.digestBlake2b256Hex, "\(vector.caseId): missing multisig digest")
        XCTAssertEqual(info.version, version, "\(vector.caseId): multisig version mismatch")
        XCTAssertEqual(info.threshold, threshold, "\(vector.caseId): multisig threshold mismatch")
        XCTAssertEqual(info.members.count, controller.members.count, "\(vector.caseId): multisig member count mismatch")
        XCTAssertEqual(
            info.totalWeight,
            controller.members.reduce(UInt32(0)) { $0 &+ UInt32($1.weight) },
            "\(vector.caseId): multisig total weight mismatch"
        )
        XCTAssertEqual(
            info.ctap2CborHex.lowercased(),
            ctap2.lowercased(),
            "\(vector.caseId): multisig CTAP2 mismatch"
        )
        XCTAssertEqual(
            info.digestBlake2b256Hex.lowercased(),
            digest.lowercased(),
            "\(vector.caseId): multisig digest mismatch"
        )

        for (expected, rendered) in zip(controller.members, info.members) {
            XCTAssertEqual(rendered.algorithm, expected.curve.lowercased(), "\(vector.caseId): algorithm mismatch")
            XCTAssertEqual(rendered.weight, expected.weight, "\(vector.caseId): member weight mismatch")
            XCTAssertEqual(
                rendered.publicKeyHex.lowercased(),
                "0x\(expected.publicKeyHex.lowercased())",
                "\(vector.caseId): member key mismatch"
            )
        }
    }

    private func assertNegative(_ vector: AccountAddressNegativeCase) {
        let expectedPrefix = vector.expectedPrefix
        let expectedCode = vector.expectedError.code
        XCTAssertThrowsError(try AccountAddress.parseAny(vector.input, expectedPrefix: expectedPrefix)) { error in
            guard let addressError = error as? AccountAddressError else {
                return XCTFail("\(vector.caseId): expected AccountAddressError, got \(error)")
            }
            XCTAssertEqual(addressError.code, expectedCode, "\(vector.caseId): error code mismatch")
        }
    }
}

// MARK: - Fixture decoding

private struct AccountAddressVectors: Decodable {
    struct Cases: Decodable {
        let negative: [AccountAddressNegativeCase]
        let positive: [AccountAddressPositiveCase]
    }

    let cases: Cases

    static func load() throws -> AccountAddressVectors {
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // AccountAddressFixtureTests.swift
            .deletingLastPathComponent() // IrohaSwiftTests
            .deletingLastPathComponent() // Tests
            .deletingLastPathComponent() // IrohaSwift package root
        let url = root.appendingPathComponent("fixtures/account/address_vectors.json")
        let data = try Data(contentsOf: url)
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode(AccountAddressVectors.self, from: data)
    }
}

private struct AccountAddressNegativeCase: Decodable {
    struct ExpectedError: Decodable {
        let kind: String
        let expected: UInt16?
        let found: UInt16?

        var code: String {
            switch kind {
            case "ChecksumMismatch":
                return AccountAddressError.checksumMismatch.code
            case "UnexpectedNetworkPrefix":
                return AccountAddressError.unexpectedNetworkPrefix(expected: expected ?? 0, found: found ?? 0).code
            case "MissingCompressedSentinel":
                return AccountAddressError.missingCompressedSentinel.code
            case "InvalidHexAddress":
                return AccountAddressError.invalidHexAddress.code
            case "UnexpectedTrailingBytes":
                return AccountAddressError.unexpectedTrailingBytes.code
            case "InvalidLength":
                return AccountAddressError.invalidLength.code
            default:
                return kind
            }
        }
    }

    let caseId: String
    let expectedError: ExpectedError
    let expectedPrefix: UInt16?
    let format: String
    let input: String
}

private struct AccountAddressPositiveCase: Decodable {
    struct Controller: Decodable {
        struct Member: Decodable {
            let curve: String
            let publicKeyHex: String
            let publicKeyMultihash: String
            let publicKeyPrefixed: String
            let weight: UInt16
        }

        let curve: String?
        let kind: String
        let publicKeyHex: String?
        let publicKeyMultihash: String?
        let publicKeyPrefixed: String?
        let members: [Member]
        let threshold: UInt16?
        let version: UInt8?
        let ctap2CborHex: String?
        let digestBlake2b256Hex: String?

        private enum CodingKeys: String, CodingKey {
            case curve
            case kind
            case publicKeyHex
            case publicKeyMultihash
            case publicKeyPrefixed
            case members
            case threshold
            case version
            case ctap2CborHex
            case digestBlake2B256Hex
        }

        init(from decoder: Decoder) throws {
            let container = try decoder.container(keyedBy: CodingKeys.self)
            curve = try container.decodeIfPresent(String.self, forKey: .curve)
            kind = try container.decode(String.self, forKey: .kind)
            publicKeyHex = try container.decodeIfPresent(String.self, forKey: .publicKeyHex)
            publicKeyMultihash = try container.decodeIfPresent(String.self, forKey: .publicKeyMultihash)
            publicKeyPrefixed = try container.decodeIfPresent(String.self, forKey: .publicKeyPrefixed)
            members = try container.decodeIfPresent([Member].self, forKey: .members) ?? []
            threshold = try container.decodeIfPresent(UInt16.self, forKey: .threshold)
            version = try container.decodeIfPresent(UInt8.self, forKey: .version)
            ctap2CborHex = try container.decodeIfPresent(String.self, forKey: .ctap2CborHex)
            digestBlake2b256Hex = try container.decodeIfPresent(String.self, forKey: .digestBlake2B256Hex)
        }
    }

    struct Encodings: Decodable {
        struct Ih58: Decodable {
            let prefix: UInt16
            let string: String
        }

        let canonicalHex: String
        let compressed: String
        let compressedFullwidth: String?
        let ih58: Ih58
    }

    let caseId: String
    let category: String
    let controller: Controller?
    let encodings: Encodings
}
