import Foundation
import XCTest
@testable import IrohaSwift

final class NoritoRpcFixtureParityTests: XCTestCase {
    private static let signedTransactionType = "iroha_data_model::transaction::signed::SignedTransaction"
    private static let fixtureNames = [
        "mint_asset", // numeric + asset
        "register_asset_definition", // asset definition governance
        "grant_revoke_role_permission", // governance role bindings
        "set_parameter_next_mode", // governance parameter change
    ]

    func testSignedTransactionFixturesRoundTrip() throws {
        let loader = try NoritoRpcFixtureLoader()
        for name in Self.fixtureNames {
            try assertFixtureRoundTrips(loader: loader, name: name)
        }
    }

    private func assertFixtureRoundTrips(loader: NoritoRpcFixtureLoader, name: String) throws {
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
        let signedHash = IrohaHash.hash(signedBytes).hexLowercased()
        XCTAssertEqual(signedHash, fixture.entry.signedHash, "signed hash mismatch for \(name)")

        guard NoritoNativeBridge.shared.isAvailable else {
            throw XCTSkip("NoritoBridge native decoder not linked")
        }
        let expectedAuthority = try expectedAuthorityLiteral(from: fixture.entry.authority)
        let json = NoritoNativeBridge.shared.withChainDiscriminant(FixtureConstants.networkPrefix) {
            NoritoNativeBridge.shared.decodeSignedTransaction(signedBytes)
        }
        guard let json else {
            return XCTFail("native decoder returned nil for \(name)")
        }
        guard let payload = decodeSignedPayload(from: json) else {
            return XCTFail("failed to decode signed transaction JSON for \(name)")
        }
        let decodedAuthority = payload["authority"] as? String
        XCTAssertEqual(decodedAuthority, expectedAuthority, "authority mismatch in decode for \(name)")
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
        var map: [String: Entry] = [:]
        for entry in manifest.fixtures {
            map[entry.name] = entry
        }
        self.entries = map
        self.root = root
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
}

private func expectedAuthorityLiteral(from label: String) throws -> String {
    let parts = label.split(separator: "@", maxSplits: 1)
    guard parts.count == 2 else {
        throw FixtureError.invalidAuthority(label)
    }
    let signatory = String(parts[0])
    let domain = String(parts[1])
    if (try? AccountAddress.fromIH58(signatory, expectedPrefix: FixtureConstants.networkPrefix)) != nil {
        return "\(signatory)@\(domain)"
    }
    let seed = deriveFixtureSeed(signatory: signatory, domain: domain)
    guard let keypair = NoritoNativeBridge.shared.keypairFromSeed(algorithm: .ed25519, seed: seed) else {
        throw FixtureError.bridgeKeypairUnavailable
    }
    let address = try AccountAddress.fromAccount(domain: domain, publicKey: keypair.publicKey)
    let ih58 = try address.toIH58(networkPrefix: FixtureConstants.networkPrefix)
    return "\(ih58)@\(domain)"
}

private func deriveFixtureSeed(signatory: String, domain: String) -> Data {
    var seed = [UInt8](repeating: 0, count: 32)
    let bytes = Array(signatory.utf8) + Array(domain.utf8)
    for (index, byte) in bytes.enumerated() {
        seed[index % seed.count] ^= byte
    }
    return Data(seed)
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

private enum FixtureError: Error {
    case missingFixture(String)
    case invalidAuthority(String)
    case bridgeKeypairUnavailable
}
