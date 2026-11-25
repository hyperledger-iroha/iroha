import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaSpartanUnloadInstructionTests: XCTestCase {
    func testIssueLoadReproducesLedgerFixtureWithExactAlignment() throws {
        func field(_ n: UInt8) -> Data { Data(repeating: n, count: 32) }
        func frame(_ amount: KagemushaWalletUInt128V1) throws -> TransactionInstructionFrame {
            try .kagemushaWalletIssueLoad(scheme: field(1), wallet: field(2), asset: field(4),
                                          ordinal: .init(low: 0, high: 0), requestId: field(3), amount: amount)
        }
        let issued = try frame(.init(low: 7, high: 0))
        XCTAssertEqual(issued.wireName, KagemushaWalletLedgerTransportV1.instructionWireName)
        XCTAssertEqual(issued.framedPayload.map { String(format: "%02x", $0) }.joined(), "4e52543000008eef66c2b4be9ed7b2604aec8579335000ae00000000000000f6c3701492ae807f0200000000000000002001010101010101010101010101010101010101010101010101010101010101018b0105000000200202020202020202020202020202020202020202020202020202020202020202200404040404040404040404040404040404040404040404040404040404040404100000000000000000000000000000000020030303030303030303030303030303030303030303030303030303030303030310070000000000000000000000000000000100")
        XCTAssertThrowsError(try frame(.init(low: 0, high: 0)))
    }

    func testAccountTransactionFrameMatchesRustOriginalAndBounds() throws {
        let pin = { (n: UInt8) in Data(repeating: n, count: 32) }
        let build = { (claim: Data) in
            try TransactionInstructionFrame.kagemushaSpartanUnload(
                scheme: pin(1), asset: pin(2), manifestDigest: pin(3), claim: claim)
        }
        let frame = try build(Data([0x4b, 0x47, 4]))
        XCTAssertEqual(frame.wireName, "iroha.kagemusha.spartan.unload.v1")
        XCTAssertEqual(frame.framedPayload.map { String(format: "%02x", $0) }.joined(), "4e5254300000634e443f1c2be14836147b0888245923006f0000000000000065c2f63fa1effb5b022001010101010101010101010101010101010101010101010101010101010101012002020202020202020202020202020202020202020202020202020202020202022003030303030303030303030303030303030303030303030303030303030303030b03000000000000004b4704")
        XCTAssertNoThrow(try build(Data(repeating: 4, count: 16_384)))
        XCTAssertThrowsError(try build(Data(repeating: 4, count: 16_385)))
        XCTAssertThrowsError(try TransactionInstructionFrame.kagemushaSpartanUnload(
            scheme: pin(1), asset: pin(2), manifestDigest: pin(0), claim: Data([4])))
    }
}
