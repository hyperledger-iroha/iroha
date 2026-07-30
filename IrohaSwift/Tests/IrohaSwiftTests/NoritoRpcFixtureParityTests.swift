import Foundation
import XCTest
#if canImport(NoritoBridge)
import NoritoBridge
#endif
@testable import IrohaSwift

final class NoritoRpcFixtureParityTests: XCTestCase {
    private static let signedTransactionType = "iroha_data_model::transaction::signed::SignedTransaction"
    private static let fixtureNames = [
        "mint_asset", // numeric + asset
        "register_asset_definition", // asset definition governance
        "grant_revoke_role_permission", // governance role bindings
        "set_parameter_next_mode", // governance parameter change
        "mixed_executable_batch", // instruction + contract call ordering
    ]

    func testSignedTransactionFixturesRoundTrip() throws {
        let loader = try NoritoRpcFixtureLoader()
        for name in Self.fixtureNames {
            try assertFixtureIntegrity(loader: loader, name: name)
        }

        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isAvailable,
            "NoritoBridge native decoder not linked"
        )
        XCTAssertEqual(
            nativeBridgeABIVersion(),
            21,
            "required transaction fixture decode must execute through ABI-21"
        )
        for name in Self.fixtureNames {
            try assertFixtureNativeRoundTrip(loader: loader, name: name)
        }
    }

    func testFixtureLoaderRejectsDuplicateNamesAndFiles() throws {
        let first = NoritoRpcFixtureLoader.Entry(
            name: "duplicate",
            authority: "authority",
            chain: "00000001",
            creationTimeMs: 1,
            timeToLiveMs: nil,
            nonce: nil,
            encodedFile: "shared.norito",
            encodedLen: 0,
            signedLen: 0,
            payloadBase64: "AA==",
            signedBase64: "AQ==",
            payloadHash: "payload-hash",
            signedHash: "signed-hash"
        )
        XCTAssertThrowsError(
            try NoritoRpcFixtureLoader.validatedEntries([first, first])
        ) { error in
            guard case FixtureError.duplicateFixtureName("duplicate") = error else {
                return XCTFail("unexpected duplicate-name error: \(error)")
            }
        }

        let second = NoritoRpcFixtureLoader.Entry(
            name: "other",
            authority: first.authority,
            chain: first.chain,
            creationTimeMs: first.creationTimeMs,
            timeToLiveMs: first.timeToLiveMs,
            nonce: first.nonce,
            encodedFile: first.encodedFile,
            encodedLen: first.encodedLen,
            signedLen: first.signedLen,
            payloadBase64: first.payloadBase64,
            signedBase64: first.signedBase64,
            payloadHash: first.payloadHash,
            signedHash: first.signedHash
        )
        XCTAssertThrowsError(
            try NoritoRpcFixtureLoader.validatedEntries([first, second])
        ) { error in
            guard case FixtureError.duplicateEncodedFile("shared.norito") = error else {
                return XCTFail("unexpected duplicate-file error: \(error)")
            }
        }

        let renamedClone = NoritoRpcFixtureLoader.Entry(
            name: "renamed-clone",
            authority: first.authority,
            chain: first.chain,
            creationTimeMs: first.creationTimeMs,
            timeToLiveMs: first.timeToLiveMs,
            nonce: first.nonce,
            encodedFile: "renamed-clone.norito",
            encodedLen: first.encodedLen,
            signedLen: first.signedLen,
            payloadBase64: first.payloadBase64,
            signedBase64: first.signedBase64,
            payloadHash: first.payloadHash,
            signedHash: first.signedHash
        )
        XCTAssertThrowsError(
            try NoritoRpcFixtureLoader.validatedEntries([first, renamedClone])
        ) { error in
            guard case FixtureError.duplicatePayloadHash("payload-hash") = error else {
                return XCTFail("unexpected renamed-clone error: \(error)")
            }
        }

        for malformed in ["YQ!!", "Y Q==", "YQ=", "YQ===", "YR=="] {
            let invalid = NoritoRpcFixtureLoader.Entry(
                name: "invalid-base64",
                authority: first.authority,
                chain: first.chain,
                creationTimeMs: first.creationTimeMs,
                timeToLiveMs: first.timeToLiveMs,
                nonce: first.nonce,
                encodedFile: "invalid-base64.norito",
                encodedLen: first.encodedLen,
                signedLen: first.signedLen,
                payloadBase64: malformed,
                signedBase64: first.signedBase64,
                payloadHash: first.payloadHash,
                signedHash: first.signedHash
            )
            XCTAssertThrowsError(
                try NoritoRpcFixtureLoader.validatedEntries([invalid])
            ) { error in
                guard case FixtureError.invalidBase64("invalid-base64.payload_base64") = error else {
                    return XCTFail("unexpected base64 error for \(malformed): \(error)")
                }
            }
        }
    }

    func testMixedExecutableBatchFixturePreservesItemOrder() throws {
        let loader = try NoritoRpcFixtureLoader()
        let fixture = try loader.fixture(named: "mixed_executable_batch")
        try assertFixtureIntegrity(loader: loader, name: fixture.entry.name)

        try requireNativeTestCapability(
            NoritoNativeBridge.shared.isAvailable,
            "NoritoBridge native decoder not linked"
        )
        let signedBytes = try XCTUnwrap(Data(base64Encoded: fixture.entry.signedBase64))
        let versionedSignedBytes = versionedSignedTransaction(signedBytes)
        let json = try NoritoNativeBridge.shared.withChainDiscriminant(
            FixtureConstants.networkPrefix
        ) {
            NoritoNativeBridge.shared.decodeSignedTransaction(versionedSignedBytes)
        }
        let payload = try XCTUnwrap(json.flatMap { decodeSignedPayload(from: $0) })
        XCTAssertNil(payload["executable"], "legacy executable field alias must not be emitted")
        let executable = try XCTUnwrap(payload["instructions"] as? [String: Any])
        let items = try XCTUnwrap(executable["Batch"] as? [[String: Any]])

        XCTAssertEqual(items.count, 3)
        XCTAssertNotNil(items[0]["Instruction"])
        XCTAssertNotNil(items[1]["ContractCall"])
        XCTAssertNotNil(items[2]["Instruction"])
    }

    private func assertFixtureIntegrity(loader: NoritoRpcFixtureLoader, name: String) throws {
        let fixture = try loader.fixture(named: name)
        let payloadBytes = fixture.payloadBytes
        XCTAssertEqual(
            payloadBytes.count,
            fixture.entry.encodedLen,
            "encoded length mismatch for \(name)"
        )

        let payloadBase64 = try XCTUnwrap(
            Data(base64Encoded: fixture.entry.payloadBase64),
            "payload_base64 missing or invalid for \(name)"
        )
        XCTAssertEqual(payloadBase64.count, fixture.entry.encodedLen, "payload length mismatch for \(name)")
        XCTAssertEqual(payloadBytes, payloadBase64, "payload mismatch for \(name)")

        let signedBytes = try XCTUnwrap(
            Data(base64Encoded: fixture.entry.signedBase64),
            "signed_base64 missing or invalid for \(name)"
        )
        XCTAssertEqual(
            signedBytes.count,
            fixture.entry.signedLen,
            "signed length mismatch for \(name)"
        )
        let payloadHash = IrohaHash.hash(payloadBytes).hexLowercased()
        XCTAssertEqual(payloadHash, fixture.entry.payloadHash, "payload hash mismatch for \(name)")
        var entrypoint = CompactNoritoWriter()
        entrypoint.writeUInt32LE(0)
        entrypoint.writeField(signedBytes)
        let signedHash = IrohaHash.hash(entrypoint.data).hexLowercased()
        XCTAssertEqual(signedHash, fixture.entry.signedHash, "signed hash mismatch for \(name)")
        XCTAssertNotEqual(
            IrohaHash.hash(signedBytes).hexLowercased(),
            fixture.entry.signedHash,
            "raw signed bytes must not alias the compact External hash for \(name)"
        )
    }

    private func assertFixtureNativeRoundTrip(loader: NoritoRpcFixtureLoader, name: String) throws {
        let fixture = try loader.fixture(named: name)
        let signedBytes = try XCTUnwrap(
            Data(base64Encoded: fixture.entry.signedBase64),
            "signed_base64 missing or invalid for \(name)"
        )
        let expectedAuthority = try expectedAuthorityLiteral(from: fixture.entry.authority)
        XCTAssertEqual(
            nativeSignedTransactionDecodeStatus(signedBytes),
            -2,
            "ABI-21 must reject the unversioned bare signed transaction fixture: \(name)"
        )
        let versionedSignedBytes = versionedSignedTransaction(signedBytes)
        XCTAssertEqual(versionedSignedBytes.first, FixtureConstants.signedTransactionVersion)
        XCTAssertEqual(
            Data(versionedSignedBytes.dropFirst()),
            signedBytes,
            "V1 framing must add only the canonical version byte for \(name)"
        )
        let decodedJson = try NoritoNativeBridge.shared.withChainDiscriminant(
            FixtureConstants.networkPrefix
        ) {
            NoritoNativeBridge.shared.decodeSignedTransaction(versionedSignedBytes)
        }
        let json = try XCTUnwrap(
            decodedJson,
            "ABI-21 native bridge must decode every required signed transaction fixture: \(name)"
        )
        guard let payload = decodeSignedPayload(from: json) else {
            return XCTFail("failed to decode signed transaction JSON for \(name)")
        }
        let decodedAuthority = payload["authority"] as? String
        assertAuthorityMatches(decodedAuthority,
                               expected: expectedAuthority,
                               name: name)
        let decodedChain = payload["chain"] as? String
        XCTAssertEqual(decodedChain, fixture.entry.chain, "chain mismatch in decode for \(name)")
        if let creation = payload["creation_time_ms"] as? NSNumber {
            XCTAssertEqual(
                creation.int64Value,
                fixture.entry.creationTimeMs,
                "creation_time_ms mismatch in decode for \(name)"
            )
        } else {
            XCTFail("creation_time_ms missing in decode for \(name)")
        }
        assertOptionalNumberEquals(
            payload["time_to_live_ms"],
            expected: fixture.entry.timeToLiveMs,
            name: name,
            field: "time_to_live_ms"
        )
        assertOptionalNumberEquals(
            payload["nonce"],
            expected: fixture.entry.nonce,
            name: name,
            field: "nonce"
        )
    }
}

// MARK: - Fixtures

private struct NoritoRpcFixtureLoader {
    struct Manifest: Decodable {
        let fixtures: [Entry]
    }

    struct Entry: Decodable {
        let name: String
        let authority: String
        let chain: String
        let creationTimeMs: Int64
        let timeToLiveMs: Int64?
        let nonce: Int64?
        let encodedFile: String
        let encodedLen: Int
        let signedLen: Int
        let payloadBase64: String
        let signedBase64: String
        let payloadHash: String
        let signedHash: String

        enum CodingKeys: String, CodingKey {
            case name
            case authority
            case chain
            case creationTimeMs = "creation_time_ms"
            case timeToLiveMs = "time_to_live_ms"
            case nonce
            case encodedFile = "encoded_file"
            case encodedLen = "encoded_len"
            case signedLen = "signed_len"
            case payloadBase64 = "payload_base64"
            case signedBase64 = "signed_base64"
            case payloadHash = "payload_hash"
            case signedHash = "signed_hash"
        }
    }

    private let entries: [String: Entry]
    private let root: URL

    init() throws {
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // NoritoRpcFixtureParityTests.swift
            .deletingLastPathComponent() // IrohaSwiftTests
            .deletingLastPathComponent() // Tests
            .deletingLastPathComponent() // IrohaSwift package root
        let manifestURL = root.appendingPathComponent("fixtures/norito_rpc/transaction_fixtures.manifest.json")
        let decoder = JSONDecoder()
        let manifest = try decoder.decode(Manifest.self, from: Data(contentsOf: manifestURL))
        self.entries = try Self.validatedEntries(manifest.fixtures)
        self.root = root
    }

    static func validatedEntries(_ fixtures: [Entry]) throws -> [String: Entry] {
        var names = Set<String>()
        var encodedFiles = Set<String>()
        var payloadHashes = Set<String>()
        var payloadBytesValues = Set<Data>()
        var signedHashes = Set<String>()
        var signedBytesValues = Set<Data>()
        for entry in fixtures {
            guard names.insert(entry.name).inserted else {
                throw FixtureError.duplicateFixtureName(entry.name)
            }
            guard encodedFiles.insert(entry.encodedFile).inserted else {
                throw FixtureError.duplicateEncodedFile(entry.encodedFile)
            }
            guard payloadHashes.insert(entry.payloadHash).inserted else {
                throw FixtureError.duplicatePayloadHash(entry.payloadHash)
            }
            let payloadBytes = try decodeCanonicalBase64(
                entry.payloadBase64,
                context: "\(entry.name).payload_base64"
            )
            guard payloadBytesValues.insert(payloadBytes).inserted else {
                throw FixtureError.duplicatePayloadBytes(entry.name)
            }
            guard signedHashes.insert(entry.signedHash).inserted else {
                throw FixtureError.duplicateSignedHash(entry.signedHash)
            }
            let signedBytes = try decodeCanonicalBase64(
                entry.signedBase64,
                context: "\(entry.name).signed_base64"
            )
            guard signedBytesValues.insert(signedBytes).inserted else {
                throw FixtureError.duplicateSignedBytes(entry.name)
            }
        }
        return Dictionary(uniqueKeysWithValues: fixtures.map { ($0.name, $0) })
    }

    private static func decodeCanonicalBase64(_ value: String, context: String) throws -> Data {
        guard let decoded = Data(base64Encoded: value),
              decoded.base64EncodedString() == value else {
            throw FixtureError.invalidBase64(context)
        }
        return decoded
    }

    func fixture(named name: String) throws -> NoritoRpcFixture {
        guard let entry = entries[name] else {
            throw FixtureError.missingFixture(name)
        }
        let path = root.appendingPathComponent("fixtures/norito_rpc/\(entry.encodedFile)")
        let data = try Data(contentsOf: path)
        return NoritoRpcFixture(entry: entry, payloadBytes: data)
    }
}

private struct NoritoRpcFixture {
    let entry: NoritoRpcFixtureLoader.Entry
    let payloadBytes: Data
}

private enum FixtureConstants {
    static let networkPrefix: UInt16 = 753
    static let signedTransactionVersion: UInt8 = 1
}

private func versionedSignedTransaction(_ bareSignedTransaction: Data) -> Data {
    var versioned = Data([FixtureConstants.signedTransactionVersion])
    versioned.append(bareSignedTransaction)
    return versioned
}

private func nativeSignedTransactionDecodeStatus(_ data: Data) -> Int32 {
    #if canImport(NoritoBridge)
    var jsonPointer: UnsafeMutablePointer<UInt8>?
    var jsonLength: UInt = 0
    let status = data.withUnsafeBytes { buffer in
        connect_norito_decode_signed_transaction_json(
            buffer.bindMemory(to: UInt8.self).baseAddress,
            UInt(data.count),
            &jsonPointer,
            &jsonLength
        )
    }
    if let jsonPointer {
        connect_norito_free(jsonPointer)
    }
    return status
    #else
    return Int32.min
    #endif
}

private func nativeBridgeABIVersion() -> UInt32? {
    #if canImport(NoritoBridge)
    connect_norito_bridge_abi_version()
    #else
    nil
    #endif
}

private func expectedAuthorityLiteral(from label: String) throws -> String {
    guard !label.contains("@") else {
        throw FixtureError.invalidAuthority(label)
    }
    guard let parsed = try? AccountAddress.parseEncoded(label,
                                                        expectedPrefix: FixtureConstants.networkPrefix) else {
        throw FixtureError.invalidAuthority(label)
    }
    return try parsed.toI105(networkPrefix: FixtureConstants.networkPrefix)
}

private func decodeSignedPayload(from json: String) -> [String: Any]? {
    guard let data = json.data(using: .utf8) else {
        return nil
    }
    guard let object = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
        return nil
    }
    return object["payload"] as? [String: Any]
}

private func assertOptionalNumberEquals(
    _ value: Any?,
    expected: Int64?,
    name: String,
    field: String
) {
    if let expected {
        guard let number = value as? NSNumber else {
            return XCTFail("\(field) missing in decode for \(name)")
        }
        XCTAssertEqual(number.int64Value, expected, "\(field) mismatch in decode for \(name)")
        return
    }
    if value == nil || value is NSNull {
        return
    }
    XCTFail("\(field) should be null in decode for \(name)")
}

private func assertAuthorityMatches(_ decoded: String?,
                                    expected: String,
                                    name: String) {
    guard let decoded else {
        return XCTFail("authority missing in decode for \(name)")
    }
    if expected.contains("@") {
        XCTAssertEqual(decoded, expected, "authority mismatch in decode for \(name)")
        return
    }
    if decoded == expected {
        return
    }
    if let atIndex = decoded.firstIndex(of: "@") {
        let prefix = String(decoded[..<atIndex])
        XCTAssertEqual(prefix, expected, "authority mismatch in decode for \(name)")
        return
    }
    XCTAssertEqual(decoded, expected, "authority mismatch in decode for \(name)")
}

private enum FixtureError: Error {
    case missingFixture(String)
    case invalidAuthority(String)
    case bridgeKeypairUnavailable
    case duplicateFixtureName(String)
    case duplicateEncodedFile(String)
    case duplicatePayloadHash(String)
    case duplicatePayloadBytes(String)
    case duplicateSignedHash(String)
    case duplicateSignedBytes(String)
    case invalidBase64(String)
}
