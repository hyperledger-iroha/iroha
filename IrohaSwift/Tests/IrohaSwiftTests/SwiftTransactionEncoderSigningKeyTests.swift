import XCTest
@testable import IrohaSwift

final class SwiftTransactionEncoderSigningKeyTests: XCTestCase {
    private static let fixtureAssetDefinitionId = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM"
    private static let fixturePrivateKeyHex =
        "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"

    func testEd25519TransferWithFeeSponsorEncodes() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.supportsTransactions(using: .ed25519),
                      "Ed25519 transaction encoder unavailable")
        guard let privateKeyBytes = Data(hexString: Self.fixturePrivateKeyHex) else {
            throw XCTSkip("Invalid fixture private key")
        }
        let signingKey = try SigningKey.fromMultihashPrivateKey(privateKeyBytes)
        let authority = AccountId.make(publicKey: try signingKey.publicKey())
        let sponsorKeypair = try Keypair(privateKeyBytes: Data(repeating: 0x12, count: 32))
        let sponsor = AccountId.make(publicKey: sponsorKeypair.publicKey)
        let request = TransferRequest(chainId: "00000000-0000-0000-0000-000000000000",
                                      authority: authority,
                                      assetDefinitionId: Self.fixtureAssetDefinitionId,
                                      quantity: "2",
                                      destination: authority,
                                      description: "fee-sponsor",
                                      feeSponsor: sponsor,
                                      ttlMs: 120)
        let envelope = try SwiftTransactionEncoder.encodeTransfer(transfer: request,
                                                                  signingKey: signingKey,
                                                                  creationTimeMs: 1_717_000_222)
        XCTAssertFalse(envelope.signedTransaction.isEmpty)
        XCTAssertFalse(envelope.transactionHash.isEmpty)
    }

    func testSm2SigningKeyEncodesTransfer() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.supportsTransactions(using: .sm2),
                      "SM2 transaction encoder unavailable")
        let seed = Data(repeating: 0x24, count: Sm2Keypair.privateKeyLength)
        let sm2Keypair = try Sm2Keypair.deriveFromSeed(seed: seed)
        let signingKey = SigningKey.sm2(sm2Keypair)
        let chainId = "00000000-0000-0000-0000-000000000000"
        guard let authority = try? AccountId.makeI105(
            publicKey: sm2Keypair.publicKey,
            algorithm: "sm2",
            distid: sm2Keypair.distid
        ) else {
            throw XCTSkip("SM2 account-id encoding is unavailable in this build.")
        }
        let request = TransferRequest(chainId: chainId,
                                      authority: authority,
                                      assetDefinitionId: Self.fixtureAssetDefinitionId,
                                      quantity: "5",
                                      destination: authority,
                                      description: nil,
                                      ttlMs: 120)
        let envelope = try SwiftTransactionEncoder.encodeTransfer(transfer: request,
                                                                  signingKey: signingKey,
                                                                  creationTimeMs: 1_717_000_000)
        XCTAssertFalse(envelope.signedTransaction.isEmpty)
        XCTAssertFalse(envelope.transactionHash.isEmpty)
    }

    func testSm2SigningKeyEncodesMint() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.supportsTransactions(using: .sm2),
                      "SM2 transaction encoder unavailable")
        let seed = Data(repeating: 0x33, count: Sm2Keypair.privateKeyLength)
        let sm2Keypair = try Sm2Keypair.deriveFromSeed(seed: seed)
        let signingKey = SigningKey.sm2(sm2Keypair)
        let chainId = "00000000-0000-0000-0000-000000000000"
        guard let authority = try? AccountId.makeI105(
            publicKey: sm2Keypair.publicKey,
            algorithm: "sm2",
            distid: sm2Keypair.distid
        ) else {
            throw XCTSkip("SM2 account-id encoding is unavailable in this build.")
        }
        let request = MintRequest(chainId: chainId,
                                  authority: authority,
                                  assetDefinitionId: Self.fixtureAssetDefinitionId,
                                  quantity: "42",
                                  destination: authority,
                                  ttlMs: 90)
        let envelope = try SwiftTransactionEncoder.encodeMint(request: request,
                                                              signingKey: signingKey,
                                                              creationTimeMs: 1_717_000_000)
        XCTAssertFalse(envelope.signedTransaction.isEmpty)
        XCTAssertFalse(envelope.transactionHash.isEmpty)
    }

    func testSecp256k1SigningKeyEncodesTransfer() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.supportsTransactions(using: .secp256k1),
                      "secp256k1 transaction encoder is unavailable on this platform.")
        let privateKey = Data((1...Secp256k1Keypair.privateKeyLength).map(UInt8.init))
        let keypair = try Secp256k1Keypair(privateKey: privateKey)
        let signingKey = SigningKey.secp256k1(keypair)
        let chainId = "00000000-0000-0000-0000-000000000000"
        let authority = try AccountId.makeI105(publicKey: keypair.publicKey, algorithm: "secp256k1")
        let request = TransferRequest(chainId: chainId,
                                      authority: authority,
                                      assetDefinitionId: Self.fixtureAssetDefinitionId,
                                      quantity: "7",
                                      destination: authority,
                                      description: "secp256k1-transfer",
                                      ttlMs: 240)
        let envelope = try SwiftTransactionEncoder.encodeTransfer(transfer: request,
                                                                  signingKey: signingKey,
                                                                  creationTimeMs: 1_717_000_123)
        XCTAssertFalse(envelope.signedTransaction.isEmpty)
        XCTAssertFalse(envelope.transactionHash.isEmpty)
    }
}
