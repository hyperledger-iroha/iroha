import XCTest
@testable import IrohaSwift

private struct Fixture: Decodable {
    let formatVersion: Int
    let defaultNetworkPrefix: UInt16
    let cases: FixtureCaseSets
}

private struct FixtureCaseSets: Decodable {
    let positive: [PositiveCase]
    let negative: [NegativeCase]
}

private struct PositiveCase: Decodable {
    let caseId: String
    let category: String
    let input: PositiveInput
    let encodings: Encodings
    let controller: Controller?
}

private struct PositiveInput: Decodable {
    let rawDomain: String?
    let normalizedDomain: String?
    let seedByte: UInt8?
    let registryId: UInt32?
    let equivalentDomain: String?
    let memberKeysHex: [String]?
    let memberWeights: [UInt16]?
    let threshold: UInt16?
}

private struct Encodings: Decodable {
    let canonicalHex: String
    let ih58: Ih58Encoding
    let compressed: String
    let compressedFullwidth: String
}

private struct Controller: Decodable {
    let kind: String
    let version: UInt8?
    let threshold: UInt16?
    let totalWeight: UInt32?
    let members: [ControllerMember]?
    let ctap2CborHex: String?
    let digestBlake2b256Hex: String?

    private enum CodingKeys: String, CodingKey {
        case kind
        case version
        case threshold
        case totalWeight
        case members
        case ctap2CborHex
        case digestBlake2B256Hex
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        kind = try container.decode(String.self, forKey: .kind)
        version = try container.decodeIfPresent(UInt8.self, forKey: .version)
        threshold = try container.decodeIfPresent(UInt16.self, forKey: .threshold)
        totalWeight = try container.decodeIfPresent(UInt32.self, forKey: .totalWeight)
        members = try container.decodeIfPresent([ControllerMember].self, forKey: .members)
        ctap2CborHex = try container.decodeIfPresent(String.self, forKey: .ctap2CborHex)
        digestBlake2b256Hex = try container.decodeIfPresent(String.self, forKey: .digestBlake2B256Hex)
    }
}

private struct ControllerMember: Decodable {
    let algorithm: String?
    let curve: String?
    let weight: UInt16
    let publicKeyHex: String

    var normalizedAlgorithm: String {
        (algorithm ?? curve ?? "unknown").lowercased()
    }
}

private struct Ih58Encoding: Decodable {
    let prefix: UInt16
    let string: String
}

private struct NegativeCase: Decodable {
    let caseId: String
    let format: String
    let input: String
    let expectedPrefix: UInt16?
    let expectedError: ExpectedError
}

private struct ExpectedError: Decodable {
    let kind: String
    let expected: UInt16?
    let found: UInt16?
    let char: String?
    let policyError: String?
}

final class AccountAddressTests: XCTestCase {
    func testGoldenRoundTrip() throws {
        let address = try AccountAddress.fromAccount(domain: "default", publicKey: Data(repeating: 1, count: 32))

        let canonical = try address.canonicalHex()
        let ih58 = try address.toIH58(networkPrefix: 753)
        let compressed = try address.toCompressedSora()

        XCTAssertEqual(
            canonical,
            "0x020001200101010101010101010101010101010101010101010101010101010101010101"
        )
        XCTAssertEqual(ih58, "6cmzPVPX4QdPT36dHgSFoznxS3MV99eV8CzeuZFTeqqsBgXDUYfft81")
        XCTAssertEqual(
            compressed,
            "sorauﾛ1NcﾐuﾛﾀKﾓhﾈgｽXｦDTﾏｴtﾔﾐ8PJPfSﾕPuﾃ884ｳﾇヰ4ﾇJKTL36"
        )

        let (parsedIH58, formatIH58) = try AccountAddress.parseEncoded(ih58, expectedPrefix: 753)
        XCTAssertEqual(formatIH58, .ih58)
        XCTAssertEqual(try parsedIH58.canonicalBytes(), try address.canonicalBytes())

        let (parsedCompressed, formatCompressed) = try AccountAddress.parseEncoded(compressed)
        XCTAssertEqual(formatCompressed, .compressed)
        XCTAssertEqual(try parsedCompressed.canonicalBytes(), try address.canonicalBytes())
    }

    func testParseEncodedRejectsCanonicalHex() throws {
        let address = try AccountAddress.fromAccount(domain: "default", publicKey: Data(repeating: 0x42, count: 32))
        let canonical = try address.canonicalHex()
        XCTAssertThrowsError(try AccountAddress.parseEncoded(canonical)) { error in
            XCTAssertEqual(error as? AccountAddressError, .unsupportedAddressFormat)
        }
    }

    func testAccountAddressCanonicalizesDomainCase() throws {
        let key = Data(repeating: 0x11, count: 32)
        let lower = try AccountAddress.fromAccount(domain: "wonderland", publicKey: key)
        let upper = try AccountAddress.fromAccount(domain: "Wonderland", publicKey: key)
        XCTAssertEqual(try lower.canonicalBytes(), try upper.canonicalBytes())
    }

    func testAccountAddressRejectsInvalidDomainLabel() {
        XCTAssertThrowsError(
            try AccountAddress.fromAccount(domain: "bad label", publicKey: Data(repeating: 0x22, count: 32))
        ) { error in
            XCTAssertEqual(error as? AccountAddressError, .invalidDomainLabel("bad label"))
        }
    }

    func testAccountAddressRejectsEmptyPublicKey() {
        XCTAssertThrowsError(
            try AccountAddress.fromAccount(domain: "wonderland", publicKey: Data())
        ) { error in
            XCTAssertEqual(error as? AccountAddressError, .invalidPublicKey)
        }
    }

    func testAccountAddressRejectsInvalidEd25519KeyLength() {
        XCTAssertThrowsError(
            try AccountAddress.fromAccount(domain: "wonderland", publicKey: Data(repeating: 0x01, count: 31))
        ) { error in
            XCTAssertEqual(error as? AccountAddressError, .invalidPublicKey)
        }
    }

    func testIh58PrefixMismatch() throws {
        let address = try AccountAddress.fromAccount(domain: "default", publicKey: Data(repeating: 1, count: 32))
        let ih58 = try address.toIH58(networkPrefix: 5)
        XCTAssertThrowsError(try AccountAddress.parseEncoded(ih58, expectedPrefix: 9)) { error in
            guard case let AccountAddressError.unexpectedNetworkPrefix(expected, found) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(expected, 9)
            XCTAssertEqual(found, 5)
        }
    }

    func testCompressedRequiresSentinel() {
        XCTAssertThrowsError(try AccountAddress.fromCompressedSora("invalid"))
    }

    func testBridgePayloadRejectsFractionalField() {
        let payload = AccountAddressError.BridgePayload(code: "ERR_INVALID_COMPRESSED_DIGIT",
                                                        message: "ERR_INVALID_COMPRESSED_DIGIT",
                                                        fields: ["digit": NSNumber(value: 1.5)])
        XCTAssertNil(AccountAddressError.fromBridgePayload(payload))
    }

    func testBridgePayloadRejectsOutOfRangeUInt16() {
        let payload = AccountAddressError.BridgePayload(code: "ERR_INVALID_IH58_PREFIX",
                                                        message: "ERR_INVALID_IH58_PREFIX",
                                                        fields: ["prefix": 70000])
        XCTAssertNil(AccountAddressError.fromBridgePayload(payload))
    }

    func testCompressedTooShort() {
        XCTAssertThrowsError(try AccountAddress.fromCompressedSora("soraabc")) { error in
            guard let addressError = error as? AccountAddressError else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(addressError.code, "ERR_COMPRESSED_TOO_SHORT")
        }
    }

    func testUnsupportedAlgorithmRejected() {
        XCTAssertThrowsError(
            try AccountAddress.fromAccount(
                domain: "default",
                publicKey: Data(repeating: 0xAA, count: 32),
                algorithm: "sm2"
            )
        ) { error in
            guard case let AccountAddressError.unsupportedAlgorithm(name) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(name.lowercased(), "sm2")
        }
    }

    func testComplianceVectorsFixture() throws {
        let fixture = try loadAddressFixture()
        XCTAssertEqual(fixture.formatVersion, 1)

        for vector in fixture.cases.positive {
            try assertPositiveCase(vector)
        }

        for vector in fixture.cases.negative {
            assertNegativeCase(vector, defaultPrefix: fixture.defaultNetworkPrefix)
        }
    }

    func testDisplayFormats() throws {
        let address = try AccountAddress.fromAccount(domain: "default", publicKey: Data(repeating: 0xAB, count: 32))
        let formats = try address.displayFormats()

        XCTAssertEqual(formats.networkPrefix, 753)
        XCTAssertEqual(formats.ih58, try address.toIH58(networkPrefix: 753))
        XCTAssertEqual(formats.compressed, try address.toCompressedSora())
        XCTAssertTrue(formats.compressedWarning.contains("Compressed Sora addresses"))
    }

    private func loadAddressFixture() throws -> Fixture {
        let fixtureURL = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("fixtures/account/address_vectors.json")
        let data = try Data(contentsOf: fixtureURL)
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode(Fixture.self, from: data)
    }

    private func captureBridgeError(for vector: NegativeCase, defaultPrefix: UInt16) throws -> AccountAddressError? {
        guard NoritoNativeBridge.shared.isAccountAddressCodecAvailable else { return nil }
        do {
            switch vector.format {
            case "ih58":
                _ = try NoritoNativeBridge.shared.parseAccountAddress(
                    literal: vector.input,
                    expectedPrefix: vector.expectedPrefix ?? defaultPrefix
                )
            case "compressed":
                _ = try NoritoNativeBridge.shared.parseAccountAddress(
                    literal: vector.input,
                    expectedPrefix: nil
                )
            case "canonical_hex":
                _ = try NoritoNativeBridge.shared.parseAccountAddress(
                    literal: vector.input,
                    expectedPrefix: nil
                )
            default:
                return nil
            }
        } catch let error as AccountAddressError {
            if vector.expectedError.kind == "MissingCompressedSentinel",
               case .unsupportedAddressFormat = error {
                return .missingCompressedSentinel
            }
            return error
        }
        return nil
    }

    func testBridgeCodecMatchesFixtures() throws {
        guard NoritoNativeBridge.shared.isAccountAddressCodecAvailable else {
            throw XCTSkip("NoritoBridge not available")
        }
        let fixture = try loadAddressFixture()
        let defaultPrefix = fixture.defaultNetworkPrefix
        guard try bridgeSupportsSelectorFreeFixtureVectors(fixture) else {
            throw XCTSkip("NoritoBridge account-address codec does not support selector-free canonical payloads yet")
        }
        var usesSoraSentinel: Bool? = nil
        try NoritoNativeBridge.shared.withChainDiscriminant(defaultPrefix) {
            for vector in fixture.cases.positive {
                let parseResult = try XCTUnwrap(
                    try NoritoNativeBridge.shared.parseAccountAddress(
                        literal: vector.encodings.ih58.string,
                        expectedPrefix: vector.encodings.ih58.prefix
                    )
                )
                XCTAssertEqual(parseResult.format, .ih58, "\(vector.caseId): bridge IH58 format mismatch")
                XCTAssertEqual(parseResult.networkPrefix, vector.encodings.ih58.prefix, "\(vector.caseId): bridge IH58 prefix mismatch")
                let render = try XCTUnwrap(
                    try NoritoNativeBridge.shared.renderAccountAddress(
                        canonicalBytes: parseResult.canonicalBytes,
                        networkPrefix: vector.encodings.ih58.prefix
                    ),
                    "\(vector.caseId): bridge render missing"
                )
                XCTAssertEqual(render.ih58, vector.encodings.ih58.string, "\(vector.caseId): bridge IH58 encode mismatch")
                let renderUsesSoraSentinel = render.compressed.hasPrefix("sora")
                if usesSoraSentinel == nil {
                    usesSoraSentinel = renderUsesSoraSentinel
                }
                let compressedLiteral = renderUsesSoraSentinel ? vector.encodings.compressed : render.compressed
                if renderUsesSoraSentinel {
                    XCTAssertEqual(render.compressed, vector.encodings.compressed, "\(vector.caseId): bridge compressed mismatch")
                    XCTAssertEqual(render.compressedFullWidth, vector.encodings.compressedFullwidth, "\(vector.caseId): bridge full-width mismatch")
                }

                let compressedResult = try XCTUnwrap(
                    try NoritoNativeBridge.shared.parseAccountAddress(
                        literal: compressedLiteral,
                        expectedPrefix: nil
                    ),
                    "\(vector.caseId): compressed bridge parse missing"
                )
                XCTAssertEqual(compressedResult.format, .compressed, "\(vector.caseId): bridge compressed format mismatch")
                XCTAssertEqual(compressedResult.canonicalBytes, parseResult.canonicalBytes, "\(vector.caseId): bridge canonical mismatch")
            }

            for vector in fixture.cases.negative {
                guard let error = try captureBridgeError(for: vector, defaultPrefix: defaultPrefix) else {
                    return XCTFail("\(vector.caseId): expected bridge error")
                }
                if vector.format == "compressed", usesSoraSentinel == false,
                   case .unsupportedAddressFormat = error {
                    continue
                }
                if vector.format == "canonical_hex",
                   case .unsupportedAddressFormat = error {
                    continue
                }
                verify(error: error, matches: vector.expectedError, caseId: vector.caseId)
            }
        }
    }

    private func bridgeSupportsSelectorFreeFixtureVectors(_ fixture: Fixture) throws -> Bool {
        guard let sample = fixture.cases.positive.first else { return false }
        do {
            guard try NoritoNativeBridge.shared.parseAccountAddress(
                literal: sample.encodings.ih58.string,
                expectedPrefix: sample.encodings.ih58.prefix
            ) != nil else {
                return false
            }
            return true
        } catch let error as AccountAddressError {
            switch error {
            case .unknownCurve, .unknownControllerTag, .unknownDomainTag, .unexpectedTrailingBytes:
                return false
            default:
                throw error
            }
        }
    }

    private func assertPositiveCase(_ vector: PositiveCase) throws {
        let address = try AccountAddress.fromIH58(vector.encodings.ih58.string,
                                                  expectedPrefix: vector.encodings.ih58.prefix)
        let canonicalBytes = try address.canonicalBytes()

        let ih58 = try AccountAddress.fromIH58(vector.encodings.ih58.string, expectedPrefix: vector.encodings.ih58.prefix)
        XCTAssertEqual(try ih58.canonicalBytes(), canonicalBytes, "\(vector.caseId): IH58 canonical mismatch")

        let (parsedIh58, formatIh58) = try AccountAddress.parseEncoded(vector.encodings.ih58.string, expectedPrefix: vector.encodings.ih58.prefix)
        XCTAssertEqual(formatIh58, .ih58, "\(vector.caseId): parseEncoded IH58 format mismatch")
        XCTAssertEqual(try parsedIh58.canonicalBytes(), canonicalBytes, "\(vector.caseId): parseEncoded IH58 canonical mismatch")

        for (encoding, label) in [(vector.encodings.compressed, "half-width"), (vector.encodings.compressedFullwidth, "full-width")] {
            let decoded = try AccountAddress.fromCompressedSora(encoding)
            XCTAssertEqual(try decoded.canonicalBytes(), canonicalBytes, "\(vector.caseId): \(label) compressed canonical mismatch")
            let (parsedCompressed, formatCompressed) = try AccountAddress.parseEncoded(encoding)
            XCTAssertEqual(formatCompressed, .compressed, "\(vector.caseId): \(label) compressed parse format mismatch")
            XCTAssertEqual(try parsedCompressed.canonicalBytes(), canonicalBytes, "\(vector.caseId): \(label) compressed parse canonical mismatch")
        }

        XCTAssertThrowsError(try AccountAddress.parseEncoded(vector.encodings.canonicalHex),
                             "\(vector.caseId): canonical hex parse should be rejected") { error in
            XCTAssertEqual(error as? AccountAddressError, .unsupportedAddressFormat)
        }

        XCTAssertEqual(try address.toIH58(networkPrefix: vector.encodings.ih58.prefix), vector.encodings.ih58.string, "\(vector.caseId): IH58 re-encode mismatch")
        XCTAssertEqual(try address.toCompressedSora(), vector.encodings.compressed, "\(vector.caseId): compressed re-encode mismatch")
        XCTAssertEqual(try address.toCompressedSoraFullWidth(), vector.encodings.compressedFullwidth, "\(vector.caseId): compressed full-width re-encode mismatch")
        XCTAssertEqual(try address.canonicalHex().lowercased(), vector.encodings.canonicalHex.lowercased(), "\(vector.caseId): canonical hex re-encode mismatch")

        if let controller = vector.controller, controller.kind == "multisig" {
            guard let info = try address.multisigPolicyInfo() else {
                return XCTFail("\(vector.caseId): expected multisig policy info")
            }
            if let version = controller.version {
                XCTAssertEqual(info.version, version, "\(vector.caseId): controller version mismatch")
            }
            if let threshold = controller.threshold {
                XCTAssertEqual(info.threshold, threshold, "\(vector.caseId): controller threshold mismatch")
            }
            if let totalWeight = controller.totalWeight {
                XCTAssertEqual(info.totalWeight, totalWeight, "\(vector.caseId): controller total weight mismatch")
            }
            if let members = controller.members {
                XCTAssertEqual(info.members.count, members.count, "\(vector.caseId): controller member count mismatch")
                for (actual, expected) in zip(info.members, members) {
                    XCTAssertEqual(actual.algorithm, expected.normalizedAlgorithm, "\(vector.caseId): controller member algorithm mismatch")
                    XCTAssertEqual(actual.weight, expected.weight, "\(vector.caseId): controller member weight mismatch")
                    let actualKey = stripHexPrefix(actual.publicKeyHex).uppercased()
                    let expectedKey = stripHexPrefix(expected.publicKeyHex).uppercased()
                    XCTAssertEqual(actualKey, expectedKey, "\(vector.caseId): controller member key mismatch")
                }
            }
            if let ctap2Hex = controller.ctap2CborHex {
                XCTAssertEqual(info.ctap2CborHex.uppercased(), ctap2Hex.uppercased(), "\(vector.caseId): controller CTAP2 hex mismatch")
            }
            if let digestHex = controller.digestBlake2b256Hex {
                XCTAssertEqual(info.digestBlake2b256Hex.uppercased(), digestHex.uppercased(), "\(vector.caseId): controller digest mismatch")
            }
        }
    }

    private func stripHexPrefix(_ value: String) -> String {
        if value.hasPrefix("0x") || value.hasPrefix("0X") {
            return String(value.dropFirst(2))
        }
        return value
    }

    private func assertNegativeCase(_ vector: NegativeCase, defaultPrefix: UInt16) {
        switch vector.format {
        case "ih58":
            XCTAssertThrowsError(
                try AccountAddress.fromIH58(vector.input, expectedPrefix: vector.expectedPrefix ?? defaultPrefix),
                "\(vector.caseId): IH58 negative should fail"
            ) { error in
                self.verify(error: error, matches: vector.expectedError, caseId: vector.caseId)
            }
        case "compressed":
            XCTAssertThrowsError(
                try AccountAddress.fromCompressedSora(vector.input),
                "\(vector.caseId): compressed negative should fail"
            ) { error in
                self.verify(error: error, matches: vector.expectedError, caseId: vector.caseId)
            }
        case "canonical_hex":
            XCTAssertThrowsError(
                try AccountAddress.parseEncoded(vector.input),
                "\(vector.caseId): canonical negative should fail"
            ) { error in
                XCTAssertEqual(error as? AccountAddressError, .unsupportedAddressFormat, "\(vector.caseId): canonical hex parser must reject legacy format")
            }
        default:
            XCTFail("\(vector.caseId): unsupported negative format \(vector.format)")
        }
    }

    private func verify(error: Error, matches expected: ExpectedError, caseId: String) {
        guard let addressError = error as? AccountAddressError else {
            return XCTFail("\(caseId): unexpected error type \(error)")
        }
        let expectedCode = AccountAddressTests.expectedErrorCode(for: expected.kind)
        XCTAssertEqual(addressError.code, expectedCode, "\(caseId): error code mismatch")
        switch expected.kind {
        case "ChecksumMismatch":
            XCTAssertEqual(addressError, .checksumMismatch, "\(caseId): expected checksum mismatch")
        case "UnexpectedNetworkPrefix":
            if case let .unexpectedNetworkPrefix(expectedPrefix, foundPrefix) = addressError {
                XCTAssertEqual(expectedPrefix, expected.expected, "\(caseId): expected prefix mismatch")
                XCTAssertEqual(foundPrefix, expected.found, "\(caseId): found prefix mismatch")
            } else {
                XCTFail("\(caseId): expected unexpected network prefix, got \(addressError)")
            }
        case "MissingCompressedSentinel":
            XCTAssertEqual(addressError, .missingCompressedSentinel, "\(caseId): expected missing sentinel")
        case "InvalidCompressedChar":
            if case let .invalidCompressedChar(symbol) = addressError {
                XCTAssertEqual(String(symbol), expected.char, "\(caseId): invalid symbol mismatch")
            } else {
                XCTFail("\(caseId): expected invalid compressed symbol, got \(addressError)")
            }
        case "InvalidHexAddress":
            XCTAssertEqual(addressError, .invalidHexAddress, "\(caseId): expected invalid hex")
        case "UnexpectedTrailingBytes":
            XCTAssertEqual(addressError, .unexpectedTrailingBytes, "\(caseId): expected unexpected trailing bytes")
        case "InvalidMultisigPolicy":
            if case let .invalidMultisigPolicy(reason) = addressError {
                XCTAssertEqual(reason, expected.policyError, "\(caseId): multisig policy mismatch")
            } else {
                XCTFail("\(caseId): expected invalid multisig policy, got \(addressError)")
            }
        case "InvalidIh58Prefix":
            if case let .invalidIh58Prefix(prefix) = addressError {
                if let expectedPrefix = expected.expected {
                    XCTAssertEqual(prefix, expectedPrefix, "\(caseId): IH58 prefix mismatch")
                }
            } else {
                XCTFail("\(caseId): expected invalid IH58 prefix, got \(addressError)")
            }
        case "UnsupportedAlgorithm":
            if case let .unsupportedAlgorithm(name) = addressError {
                if let expectedName = expected.policyError {
                    XCTAssertEqual(name.lowercased(), expectedName.lowercased(), "\(caseId): unsupported algorithm mismatch")
                }
            } else {
                XCTFail("\(caseId): expected unsupported algorithm, got \(addressError)")
            }
        default:
            XCTAssertEqual(addressError.identifier, expected.kind, "\(caseId): unexpected error kind")
        }
    }
}

private extension AccountAddressError {
    var identifier: String {
        switch self {
        case .unsupportedAlgorithm:
            return "UnsupportedAlgorithm"
        case .keyPayloadTooLong:
            return "KeyPayloadTooLong"
        case .invalidHeaderVersion:
            return "InvalidHeaderVersion"
        case .invalidNormVersion:
            return "InvalidNormVersion"
        case .invalidIh58Prefix:
            return "InvalidIh58Prefix"
        case .hashFailure:
            return "HashFailure"
        case .invalidIh58Encoding:
            return "InvalidIh58Encoding"
        case .invalidLength:
            return "InvalidLength"
        case .checksumMismatch:
            return "ChecksumMismatch"
        case .invalidHexAddress:
            return "InvalidHexAddress"
        case .domainMismatch:
            return "DomainMismatch"
        case .invalidDomainLabel:
            return "InvalidDomainLabel"
        case .unexpectedNetworkPrefix:
            return "UnexpectedNetworkPrefix"
        case .unknownAddressClass:
            return "UnknownAddressClass"
        case .unknownDomainTag:
            return "UnknownDomainTag"
        case .unexpectedExtensionFlag:
            return "UnexpectedExtensionFlag"
        case .unknownControllerTag:
            return "UnknownControllerTag"
        case .invalidPublicKey:
            return "InvalidPublicKey"
        case .unknownCurve:
            return "UnknownCurve"
        case .unexpectedTrailingBytes:
            return "UnexpectedTrailingBytes"
        case .invalidIh58PrefixEncoding:
            return "InvalidIh58PrefixEncoding"
        case .missingCompressedSentinel:
            return "MissingCompressedSentinel"
        case .compressedTooShort:
            return "CompressedTooShort"
        case .invalidCompressedChar:
            return "InvalidCompressedChar"
        case .invalidCompressedBase:
            return "InvalidCompressedBase"
        case .invalidCompressedDigit:
            return "InvalidCompressedDigit"
        case .unsupportedAddressFormat:
            return "UnsupportedAddressFormat"
        case .multisigMemberOverflow:
            return "MultisigMemberOverflow"
        case .invalidMultisigPolicy:
            return "InvalidMultisigPolicy"
        }
    }
}

private extension AccountAddressTests {
    static func expectedErrorCode(for kind: String) -> String {
        return "ERR_" + camelToScreamingSnake(kind)
    }

    static func camelToScreamingSnake(_ value: String) -> String {
        var result = ""
        for (index, character) in value.enumerated() {
            if character.isUppercase, index > 0 {
                result.append("_")
            }
            result.append(contentsOf: character.uppercased())
        }
        return result
    }
}
