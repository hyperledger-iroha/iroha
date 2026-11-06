import Foundation
import XCTest
@testable import IrohaSwift

final class InstructionTransactionInspectionV1Tests: XCTestCase {
    private func fixture() throws -> (SignedTransactionEnvelope, TransactionInstructionFrame, String) {
        let key = try Keypair(privateKeyBytes: Data(repeating: 0x35, count: 32))
        let authority = try AccountId.make(publicKey: key.publicKey)
        let frame = try TransactionInstructionFrame(wireName: "test.inspection.v1",
            framedPayload: noritoEncode(typeName: "test.inspection.v1", payload: Data([1])))
        let payload = try InstructionTransactionV1.payload(networkId: TestNetworkIds.canonical,
            authority: authority, creationTimeMs: 10_000, ttlMs: 100_000, frames: [frame],
            feePayment: .authority(chargeLimits: [], gasLimit: nil))
        let envelope = try InstructionTransactionV1.envelope(payload: payload,
            signature: key.sign(InstructionTransactionV1.signingMessage(payload: payload)), ed25519PublicKey: key.publicKey)
        return (envelope, frame, authority)
    }
    func testInspectsExactRetainedEnvelopeWithoutAKeyOrNetwork() throws {
        let (envelope, frame, authority) = try fixture()
        let result = try InstructionTransactionV1.inspectSignedWire(envelope.norito,
            expectedNetworkId: TestNetworkIds.canonical, expectedAuthority: authority, expectedFrames: [frame])
        XCTAssertEqual(result.creationTimeMs, 10_000)
        XCTAssertEqual(result.timeToLiveMs, 100_000)
        XCTAssertEqual(result.transactionHash, envelope.transactionHash)
    }
    func testRejectsChangedAuthorityNetworkInstructionsAndWire() throws {
        let (envelope, frame, authority) = try fixture()
        let other = try AccountId.make(publicKey: Keypair(privateKeyBytes: Data(repeating: 0x36, count: 32)).publicKey)
        let different = try TransactionInstructionFrame(wireName: "test.inspection.v1",
            framedPayload: noritoEncode(typeName: "test.inspection.v1", payload: Data([2])))
        let network = try NetworkId(bytes: Data(repeating: 0x95, count: 32))
        XCTAssertThrowsError(try InstructionTransactionV1.inspectSignedWire(envelope.norito,
            expectedNetworkId: network, expectedAuthority: authority, expectedFrames: [frame]))
        XCTAssertThrowsError(try InstructionTransactionV1.inspectSignedWire(envelope.norito,
            expectedNetworkId: TestNetworkIds.canonical, expectedAuthority: other, expectedFrames: [frame]))
        for frames in [[different], [frame, frame], []] {
            XCTAssertThrowsError(try InstructionTransactionV1.inspectSignedWire(envelope.norito,
                expectedNetworkId: TestNetworkIds.canonical, expectedAuthority: authority, expectedFrames: frames))
        }
        for wire in [envelope.norito + Data([0]), Data([2]) + envelope.signedTransaction] {
            XCTAssertThrowsError(try InstructionTransactionV1.inspectSignedWire(wire,
                expectedNetworkId: TestNetworkIds.canonical, expectedAuthority: authority, expectedFrames: [frame]))
        }
    }
}
