import Foundation
import XCTest
@testable import IrohaSwift

final class BridgeAvailabilityTests: XCTestCase {
    override func tearDown() {
        NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(nil)
        NoritoNativeBridge.shared.overrideConnectCodecAvailabilityForTests(nil)
        super.tearDown()
    }

    func testConnectCodecUnavailableWhenBridgeDisabled() {
        NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(false)
        NoritoNativeBridge.shared.overrideConnectCodecAvailabilityForTests(false)

        XCTAssertThrowsError(try ConnectCodec.decode(Data())) { error in
            if case ConnectCodecError.bridgeUnavailable = error {
                return
            }
            XCTFail("expected bridgeUnavailable, got \(error)")
        }
    }

    func testTransactionEncoderUnavailableWhenBridgeDisabled() throws {
        NoritoNativeBridge.shared.overrideBridgeAvailabilityForTests(false)
        let privateKey = Data(repeating: 1, count: 32)
        let keypair = try Keypair(privateKeyBytes: privateKey)
        let authority = AccountId.make(publicKey: keypair.publicKey, domain: "default")
        let request = TransferRequest(chainId: "chain",
                                      authority: authority,
                                      assetDefinitionId: "xor#test",
                                      quantity: "1",
                                      destination: authority,
                                      description: nil,
                                      ttlMs: nil)
        let signingKey = try SigningKey.ed25519(privateKey: privateKey)

        XCTAssertThrowsError(
            try SwiftTransactionEncoder.encodeTransfer(
                transfer: request,
                signingKey: signingKey,
                creationTimeMs: 123
            )
        ) { error in
            if case SwiftTransactionEncoderError.nativeBridgeUnavailable = error {
                return
            }
            XCTFail("expected nativeBridgeUnavailable, got \(error)")
        }
    }
}
