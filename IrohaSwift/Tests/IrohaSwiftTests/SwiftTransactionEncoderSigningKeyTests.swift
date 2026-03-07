import XCTest
@testable import IrohaSwift

final class SwiftTransactionEncoderSigningKeyTests: XCTestCase {
    func testSm2SigningKeyEncodesTransfer() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.supportsTransactions(using: .sm2),
                      "SM2 transaction encoder unavailable")
        let seed = Data(repeating: 0x24, count: Sm2Keypair.privateKeyLength)
        let sm2Keypair = try Sm2Keypair.deriveFromSeed(seed: seed)
        let signingKey = SigningKey.sm2(sm2Keypair)
        let domain = "wonderland"
        let chainId = "00000000-0000-0000-0000-000000000000"
        let assetDefinitionId = "xor#\(domain)"
        let authority = try "\(sm2Keypair.publicKeyPrefixed())@\(domain)"
        let request = TransferRequest(chainId: chainId,
                                      authority: authority,
                                      assetDefinitionId: assetDefinitionId,
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
        let domain = "wonderland"
        let chainId = "00000000-0000-0000-0000-000000000000"
        let assetDefinitionId = "xor#\(domain)"
        let authority = try "\(sm2Keypair.publicKeyPrefixed())@\(domain)"
        let request = MintRequest(chainId: chainId,
                                  authority: authority,
                                  assetDefinitionId: assetDefinitionId,
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
        let domain = "wonderland"
        let chainId = "00000000-0000-0000-0000-000000000000"
        let assetDefinitionId = "xor#\(domain)"
        let multihash = OfflineNorito.publicKeyMultihash(algorithm: .secp256k1, payload: keypair.publicKey)
        let authority = "secp256k1:\(multihash)@\(domain)"
        let request = TransferRequest(chainId: chainId,
                                      authority: authority,
                                      assetDefinitionId: assetDefinitionId,
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
