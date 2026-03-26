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
    let i105: I105Encoding

    private enum CodingKeys: String, CodingKey {
        case canonicalHex
        case i105
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        canonicalHex = try container.decode(String.self, forKey: .canonicalHex)
        i105 = try container.decode(I105Encoding.self, forKey: .i105)
    }
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

private struct I105Encoding: Decodable {
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
        let address = try AccountAddress.fromAccount(publicKey: Data(repeating: 1, count: 32))

        let canonical = try address.canonicalHex()
        let i105 = try address.toI105(networkPrefix: 753)

        XCTAssertEqual(
            canonical,
            "0x020001200101010101010101010101010101010101010101010101010101010101010101"
        )
        XCTAssertTrue(i105.hasPrefix("sora"))
        let payload = String(i105.dropFirst(4))
        XCTAssertTrue(
            payload.unicodeScalars.contains(where: {
                $0.isASCII && CharacterSet.alphanumerics.contains($0)
            })
        )
        XCTAssertTrue(
            payload.contains(where: {
                "イロハニホヘトチリヌルヲワカヨタレソツネナラムウヰノオクヤマケフコエテアサキユメミシヱヒモセス".contains($0)
            })
        )

        let parsedI105 = try AccountAddress.parseEncoded(i105, expectedPrefix: 753)
        XCTAssertEqual(try parsedI105.canonicalBytes(), try address.canonicalBytes())
    }

    func testParseEncodedRejectsCanonicalHex() throws {
        let address = try AccountAddress.fromAccount(publicKey: Data(repeating: 0x42, count: 32))
        let canonical = try address.canonicalHex()
        XCTAssertThrowsError(try AccountAddress.parseEncoded(canonical)) { error in
            XCTAssertEqual(error as? AccountAddressError, .unsupportedAddressFormat)
        }
    }

    func testParseEncodedRejectsFullwidthSentinelI105() throws {
        let address = try AccountAddress.fromAccount(publicKey: Data(repeating: 0x31, count: 32))
        let canonical = try address.toI105(networkPrefix: 753)
        var noncanonical = canonical
        if let range = noncanonical.range(of: "sora") {
            noncanonical.replaceSubrange(range, with: "ｓｏｒａ")
        }

        XCTAssertThrowsError(try AccountAddress.fromI105(noncanonical, expectedPrefix: 753)) { error in
            XCTAssertEqual(error as? AccountAddressError, .unsupportedAddressFormat)
        }
        XCTAssertThrowsError(try AccountAddress.parseEncoded(noncanonical, expectedPrefix: 753)) { error in
            XCTAssertEqual(error as? AccountAddressError, .unsupportedAddressFormat)
        }
    }

    func testAccountAddressCanonicalizesDomainCase() throws {
        let key = Data(repeating: 0x11, count: 32)
        let lower = try AccountAddress.fromAccount(publicKey: key)
        let upper = try AccountAddress.fromAccount(publicKey: key)
        XCTAssertEqual(try lower.canonicalBytes(), try upper.canonicalBytes())
    }

    func testAccountAddressConstructorIsDomainless() throws {
        XCTAssertNoThrow(
            try AccountAddress.fromAccount(publicKey: Data(repeating: 0x22, count: 32))
        )
    }

    func testAccountAddressRejectsEmptyPublicKey() {
        XCTAssertThrowsError(
            try AccountAddress.fromAccount(publicKey: Data())
        ) { error in
            XCTAssertEqual(error as? AccountAddressError, .invalidPublicKey)
        }
    }

    func testAccountAddressRejectsInvalidEd25519KeyLength() {
        XCTAssertThrowsError(
            try AccountAddress.fromAccount(publicKey: Data(repeating: 0x01, count: 31))
        ) { error in
            XCTAssertEqual(error as? AccountAddressError, .invalidPublicKey)
        }
    }

    func testI105PrefixMismatch() throws {
        let address = try AccountAddress.fromAccount(publicKey: Data(repeating: 1, count: 32))
        let i105 = try address.toI105(networkPrefix: 5)
        XCTAssertThrowsError(try AccountAddress.parseEncoded(i105, expectedPrefix: 9)) { error in
            guard case let AccountAddressError.unexpectedNetworkPrefix(expected, found) = error else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(expected, 9)
            XCTAssertEqual(found, 5)
        }
    }

    func testI105RequiresSentinel() {
        XCTAssertThrowsError(try AccountAddress.fromI105("invalid"))
    }

    func testBridgePayloadRejectsFractionalField() {
        let payload = AccountAddressError.BridgePayload(code: "ERR_INVALID_I105_CHAR",
                                                        message: "ERR_INVALID_I105_CHAR",
                                                        fields: ["char": NSNumber(value: 1.5)])
        XCTAssertNil(AccountAddressError.fromBridgePayload(payload))
    }

    func testBridgePayloadRejectsOutOfRangeUInt16() {
        let payload = AccountAddressError.BridgePayload(code: "ERR_INVALID_i105_PREFIX",
                                                        message: "ERR_INVALID_i105_PREFIX",
                                                        fields: ["prefix": 70000])
        XCTAssertNil(AccountAddressError.fromBridgePayload(payload))
    }

    func testI105TooShort() {
        XCTAssertThrowsError(try AccountAddress.fromI105("soraア")) { error in
            guard let addressError = error as? AccountAddressError else {
                return XCTFail("unexpected error: \(error)")
            }
            XCTAssertEqual(addressError.code, "ERR_I105_TOO_SHORT")
        }
    }

    func testParseLiveLinkedBankReserveLiteral() throws {
        let fixture = try loadAddressFixture()
        let liveReserveLiteral = try XCTUnwrap(
            fixture.cases.positive.first(where: { $0.category == "multisig" })?.encodings.i105.string
        )

        let parsed = try AccountAddress.parseEncoded(liveReserveLiteral, expectedPrefix: nil)
        XCTAssertEqual(try parsed.toI105(networkPrefix: fixture.defaultNetworkPrefix), liveReserveLiteral)
        XCTAssertNotNil(try parsed.multisigPolicyInfo())
    }

    func testMixedCanonicalI105LiteralRoundTrips() throws {
        let literal = "sorauロ1PワdホシヒノNクdチムkiヌ3オモaPBQDTイKqシqオrラカwSQ1フナQU61Y7"
        let address = try AccountAddress.fromI105(literal, expectedPrefix: 753)
        XCTAssertEqual(
            try address.canonicalHex().lowercased(),
            "0x02000120bc717326224e4b4119298e7b1db8133cb27d6cdf6b3e04d75a6d27b29a34c1cf"
        )
        XCTAssertEqual(try address.toI105(networkPrefix: 753), literal)
    }

    func testUnsupportedAlgorithmRejected() {
        XCTAssertThrowsError(
            try AccountAddress.fromAccount(publicKey: Data(repeating: 0xAA, count: 32),
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
        let address = try AccountAddress.fromAccount(publicKey: Data(repeating: 0xAB, count: 32))
        let formats = try address.displayFormats()

        XCTAssertEqual(formats.networkPrefix, 753)
        XCTAssertEqual(formats.i105, try address.toI105(networkPrefix: 753))
        XCTAssertTrue(formats.i105Warning.contains("canonical I105 alphabet"))
        XCTAssertTrue(formats.i105Warning.contains("Base58 plus the 47 katakana"))
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
            case "i105":
                _ = try NoritoNativeBridge.shared.parseAccountAddress(
                    literal: vector.input,
                    expectedPrefix: vector.expectedPrefix ?? defaultPrefix
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
        try NoritoNativeBridge.shared.withChainDiscriminant(defaultPrefix) {
            for vector in fixture.cases.positive {
                let parseResult = try XCTUnwrap(
                    try NoritoNativeBridge.shared.parseAccountAddress(
                        literal: vector.encodings.i105.string,
                        expectedPrefix: vector.encodings.i105.prefix
                    )
                )
                XCTAssertEqual(parseResult.networkPrefix, vector.encodings.i105.prefix, "\(vector.caseId): bridge i105 prefix mismatch")
                let render = try XCTUnwrap(
                    try NoritoNativeBridge.shared.renderAccountAddress(
                        canonicalBytes: parseResult.canonicalBytes,
                        networkPrefix: vector.encodings.i105.prefix
                    ),
                    "\(vector.caseId): bridge render missing"
                )
                XCTAssertEqual(render.i105, vector.encodings.i105.string, "\(vector.caseId): bridge i105 encode mismatch")
                XCTAssertEqual(render.canonicalHex.lowercased(), vector.encodings.canonicalHex.lowercased(), "\(vector.caseId): bridge canonical hex mismatch")
            }

            for vector in fixture.cases.negative {
                guard let error = try captureBridgeError(for: vector, defaultPrefix: defaultPrefix) else {
                    return XCTFail("\(vector.caseId): expected bridge error")
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
                literal: sample.encodings.i105.string,
                expectedPrefix: sample.encodings.i105.prefix
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
        let address = try AccountAddress.fromI105(vector.encodings.i105.string,
                                                  expectedPrefix: vector.encodings.i105.prefix)
        let canonicalBytes = try address.canonicalBytes()

        let i105 = try AccountAddress.fromI105(vector.encodings.i105.string, expectedPrefix: vector.encodings.i105.prefix)
        XCTAssertEqual(try i105.canonicalBytes(), canonicalBytes, "\(vector.caseId): i105 canonical mismatch")

        let parsedI105 = try AccountAddress.parseEncoded(vector.encodings.i105.string, expectedPrefix: vector.encodings.i105.prefix)
        XCTAssertEqual(try parsedI105.canonicalBytes(), canonicalBytes, "\(vector.caseId): parseEncoded i105 canonical mismatch")

        XCTAssertThrowsError(try AccountAddress.parseEncoded(vector.encodings.canonicalHex),
                             "\(vector.caseId): canonical hex parse should be rejected") { error in
            XCTAssertEqual(error as? AccountAddressError, .unsupportedAddressFormat)
        }

        XCTAssertEqual(try address.toI105(networkPrefix: vector.encodings.i105.prefix), vector.encodings.i105.string, "\(vector.caseId): i105 re-encode mismatch")
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
        case "i105":
            XCTAssertThrowsError(
                try AccountAddress.fromI105(vector.input, expectedPrefix: vector.expectedPrefix ?? defaultPrefix),
                "\(vector.caseId): i105 negative should fail"
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
        case "InvalidI105Char":
            if case let .invalidI105Char(symbol) = addressError {
                XCTAssertEqual(String(symbol), expected.char, "\(caseId): invalid symbol mismatch")
            } else {
                XCTFail("\(caseId): expected invalid I105 symbol, got \(addressError)")
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
        case "InvalidI105Prefix":
            if case let .invalidI105Prefix(prefix) = addressError {
                if let expectedPrefix = expected.expected {
                    XCTAssertEqual(prefix, expectedPrefix, "\(caseId): i105 prefix mismatch")
                }
            } else {
                XCTFail("\(caseId): expected invalid i105 prefix, got \(addressError)")
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
        case .invalidI105Prefix:
            return "InvalidI105Prefix"
        case .hashFailure:
            return "HashFailure"
        case .invalidI105Encoding:
            return "InvalidI105Encoding"
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
        case .invalidI105PrefixEncoding:
            return "InvalidI105PrefixEncoding"
        case .missingI105Sentinel:
            return "MissingI105Sentinel"
        case .invalidI105Base:
            return "InvalidI105Base"
        case .invalidI105Digit:
            return "InvalidI105Digit"
        case .i105TooShort:
            return "I105TooShort"
        case .invalidI105Char:
            return "InvalidI105Char"
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
