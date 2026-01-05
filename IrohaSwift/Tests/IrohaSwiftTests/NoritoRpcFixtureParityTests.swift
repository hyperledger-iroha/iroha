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
        XCTAssertTrue(json.contains(expectedAuthority), "authority missing in decode for \(name)")
        XCTAssertTrue(json.contains(fixture.entry.chain), "chain missing in decode for \(name)")
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
        let encodedFile: String
        let encodedLen: Int
        let signedLen: Int
        let payloadBase64: String
        let signedBase64: String

        enum CodingKeys: String, CodingKey {
            case name
            case authority
            case chain
            case encodedFile = "encoded_file"
            case encodedLen = "encoded_len"
            case signedLen = "signed_len"
            case payloadBase64 = "payload_base64"
            case signedBase64 = "signed_base64"
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

private enum FixtureError: Error {
    case missingFixture(String)
    case invalidAuthority(String)
    case bridgeKeypairUnavailable
}
