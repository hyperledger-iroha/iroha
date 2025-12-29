import XCTest
@testable import IrohaSwift

final class SoranetVpnTunnelTests: XCTestCase {
    func testFramesArePadded() throws {
        let tunnel = try SoranetVpnTunnel(circuitId: Data(repeating: 0xAA, count: 16))
        let frame = try tunnel.frame(
            payload: Data(repeating: 0xBB, count: 32),
            sequence: 7,
            ack: 1,
            flowLabel: 0xA5A5A5,
            cover: true
        )
        XCTAssertEqual(frame.count, SoranetVpnTunnel.cellSize)
        XCTAssertEqual(frame[0], 1)
        XCTAssertEqual(frame[1], 0)
        XCTAssertEqual(frame[2] & 0x01, 0x01)
        XCTAssertEqual(Data(frame[3..<19]), Data(repeating: 0xAA, count: 16))
        XCTAssertEqual(frame[19], 0xA5)
        XCTAssertEqual(frame[20], 0xA5)
        XCTAssertEqual(frame[21], 0xA5)
        let seq = frame[22..<30].reduce(UInt64(0)) { ($0 << 8) | UInt64($1) }
        XCTAssertEqual(seq, 7)
        let ack = frame[30..<38].reduce(UInt64(0)) { ($0 << 8) | UInt64($1) }
        XCTAssertEqual(ack, 1)
        let payloadLen = frame[40..<42].reduce(UInt16(0)) { ($0 << 8) | UInt16($1) }
        XCTAssertEqual(payloadLen, 32)
        let padding = frame[(42 + 32)...]
        XCTAssertTrue(padding.allSatisfy { $0 == 0 })
    }

    func testRejectsOversizedPayload() throws {
        let tunnel = try SoranetVpnTunnel(circuitId: Data(repeating: 0x00, count: 16))
        let bigPayload = Data(repeating: 0xCC, count: SoranetVpnCellHeader.cellSize)
        XCTAssertThrowsError(try tunnel.frame(payload: bigPayload, sequence: 1)) { error in
            guard case let SoranetVpnError.payloadTooLarge(max, actual) = error else {
                XCTFail("unexpected error: \(error)")
                return
            }
            XCTAssertEqual(max, SoranetVpnCellHeader.cellSize - SoranetVpnCellHeader.headerSize)
            XCTAssertEqual(actual, bigPayload.count)
        }
    }
}
