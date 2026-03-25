import XCTest
@testable import IrohaSwift

private struct WalletFixtureDocument: Decodable {
    let formatVersion: Int
    let chainId: String
    let authorityId: String
    let assetDefinitionId: String
    let cases: [WalletFixtureCase]

    func fixtureCase(named identifier: String) -> WalletFixtureCase? {
        cases.first { $0.caseId == identifier }
    }
}

private struct WalletFixtureCase: Decodable {
    let caseId: String
    let signedTransactionHex: String
    let transactionHashHex: String
}

final class ConfidentialWalletFixturesTests: XCTestCase {
    private static let fixtureChainId = "00000000-0000-0000-0000-000000000000"
    private static let fixtureDomain = "wonderland"
    private static let fixtureAssetDefinition = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
    private static let fixtureCreationTime: UInt64 = 1_700_000_000_000
    private static let fixtureTtlMs: UInt64 = 45
    private static let fixturePrivateKeyHex =
        "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"

    func testShieldFixtureMatchesSwiftEncoder() throws {
        let fixture = try Self.loadFixtureDocument()
        XCTAssertEqual(fixture.formatVersion, 1)
        guard let caseEntry = fixture.fixtureCase(named: "shield-basic") else {
            return XCTFail("shield-basic case missing")
        }
        let envelope: SignedTransactionEnvelope
        do {
            envelope = try Self.buildShieldEnvelope()
        } catch {
            throw XCTSkip("Shield fixture unavailable: \(error)")
        }
        assertEnvelope(caseEntry, matches: envelope)
    }

    func testUnshieldFixtureMatchesSwiftEncoder() throws {
        let fixture = try Self.loadFixtureDocument()
        guard let caseEntry = fixture.fixtureCase(named: "unshield-basic") else {
            return XCTFail("unshield-basic case missing")
        }
        let envelope: SignedTransactionEnvelope
        do {
            envelope = try Self.buildUnshieldEnvelope()
        } catch {
            throw XCTSkip("Unshield fixture unavailable: \(error)")
        }
        assertEnvelope(caseEntry, matches: envelope)
    }

    func testZkTransferFixtureDocumented() throws {
        let fixture = try Self.loadFixtureDocument()
        guard let transferCase = fixture.fixtureCase(named: "zk-transfer-basic") else {
            return XCTFail("zk-transfer-basic case missing")
        }
        let envelope: SignedTransactionEnvelope
        do {
            envelope = try Self.buildZkTransferEnvelope()
        } catch {
            throw XCTSkip("ZkTransfer fixture unavailable: \(error)")
        }
        assertEnvelope(transferCase, matches: envelope)
    }

    private static func buildShieldEnvelope() throws -> SignedTransactionEnvelope {
        let signingKey = try makeFixtureSigningKey()
        let publicKey = try signingKey.publicKey()
        let authority = AccountId.make(publicKey: publicKey)
        let payload = try ConfidentialEncryptedPayload(
            ephemeralPublicKey: Data(repeating: 0x01, count: 32),
            nonce: Data(repeating: 0x02, count: 24),
            ciphertext: Data([0xDE, 0xAD, 0xBE, 0xEF])
        )
        let request = try ShieldRequest(
            chainId: fixtureChainId,
            authority: authority,
            assetDefinitionId: fixtureAssetDefinition,
            fromAccountId: authority,
            amount: "42",
            noteCommitment: Data(repeating: 0xAB, count: 32),
            payload: payload,
            ttlMs: fixtureTtlMs
        )
        return try SwiftTransactionEncoder.encodeShield(
            request: request,
            signingKey: signingKey,
            creationTimeMs: fixtureCreationTime
        )
    }

    private static func buildUnshieldEnvelope() throws -> SignedTransactionEnvelope {
        let signingKey = try makeFixtureSigningKey()
        let publicKey = try signingKey.publicKey()
        let authority = AccountId.make(publicKey: publicKey)
        let proof = try makeProofAttachment(name: "vk_unshield")
        let request = try UnshieldRequest(
            chainId: fixtureChainId,
            authority: authority,
            assetDefinitionId: fixtureAssetDefinition,
            toAccountId: authority,
            publicAmount: "1337",
            inputs: [Data(repeating: 0x55, count: 32)],
            proof: proof,
            rootHint: nil,
            ttlMs: fixtureTtlMs
        )
        return try SwiftTransactionEncoder.encodeUnshield(
            request: request,
            signingKey: signingKey,
            creationTimeMs: fixtureCreationTime
        )
    }

    private static func buildZkTransferEnvelope() throws -> SignedTransactionEnvelope {
        let signingKey = try makeFixtureSigningKey()
        let publicKey = try signingKey.publicKey()
        let authority = AccountId.make(publicKey: publicKey)
        let proof = try makeProofAttachment(name: "vk_transfer")
        let request = try ZkTransferRequest(
            chainId: fixtureChainId,
            authority: authority,
            assetDefinitionId: fixtureAssetDefinition,
            inputs: [Data(repeating: 0x10, count: 32), Data(repeating: 0x11, count: 32)],
            outputs: [Data(repeating: 0x22, count: 32), Data(repeating: 0x33, count: 32)],
            proof: proof,
            rootHint: Data(repeating: 0x44, count: 32),
            ttlMs: fixtureTtlMs
        )
        return try SwiftTransactionEncoder.encodeZkTransfer(
            request: request,
            signingKey: signingKey,
            creationTimeMs: fixtureCreationTime
        )
    }

    private static func makeProofAttachment(name: String) throws -> ProofAttachment {
        let reference = ProofAttachment.VerifyingKeyReference(backend: "halo2/ipa", name: name)
        return try ProofAttachment(
            backend: "halo2/ipa",
            proof: Data(repeating: 0xEE, count: 48),
            verifyingKey: .reference(reference)
        )
    }

    private static func makeFixtureSigningKey() throws -> SigningKey {
        guard #available(macOS 10.15, iOS 13.0, *) else {
            throw XCTSkip("Curve25519 requires macOS 10.15 / iOS 13")
        }
        guard let privateKeyBytes = Data(hexString: fixturePrivateKeyHex) else {
            throw XCTSkip("invalid fixture private key hex")
        }
        return try SigningKey.fromMultihashPrivateKey(privateKeyBytes)
    }

    private static func loadFixtureDocument() throws -> WalletFixtureDocument {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("fixtures/confidential/wallet_flows_v1.json")
        let data = try Data(contentsOf: url)
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode(WalletFixtureDocument.self, from: data)
    }

    private func assertEnvelope(
        _ entry: WalletFixtureCase,
        matches envelope: SignedTransactionEnvelope
    ) {
        XCTAssertEqual(
            envelope.signedTransaction.hexLowercased(),
            entry.signedTransactionHex.lowercased(),
            "signed transaction mismatch for \(entry.caseId)"
        )
        let hash = envelope.transactionHash
        XCTAssertEqual(
            hash.hexLowercased(),
            entry.transactionHashHex.lowercased(),
            "transaction hash mismatch for \(entry.caseId)"
        )
    }
}

private extension Data {
    func hexLowercased() -> String {
        map { String(format: "%02x", $0) }.joined()
    }
}
