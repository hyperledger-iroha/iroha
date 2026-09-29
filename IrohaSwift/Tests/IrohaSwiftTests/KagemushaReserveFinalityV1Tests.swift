import Foundation
import XCTest
@testable import IrohaSwift

/// Coordinate/hint checks only; no native monetary verdict is replaced with a fixture.
final class KagemushaReserveFinalityV1Tests: XCTestCase {
    private let network = Data(repeating: 3, count: 32)
    private let context = Data(repeating: 5, count: 32)
    private func checkpoint() throws -> Data {
        let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        return try Data(contentsOf: root.appendingPathComponent("fixtures/sumeragi/native-finality/height-2-checkpoint.nrt"))
    }
    private func hint(_ height: String = "7") -> String {
        "{\"version\":1,\"network_id\":\"\(String(repeating: "03", count: 32))\",\"block_height\":\"\(height)\",\"block_hash\":\"\(String(repeating: "05", count: 32))\"}"
    }
    func testAnchorRetainsValueCopies() throws {
        var n = network; var c = try checkpoint()
        let original = c
        let anchor = try KagemushaFinalityTrustAnchorV1(networkID: n, checkpoint: c)
        n[0] = 9; c[0] ^= 1
        var output = anchor.networkID; output[0] = 8
        var returned = anchor.checkpoint; returned[0] ^= 1
        XCTAssertEqual(anchor.networkID, network); XCTAssertEqual(anchor.checkpoint, original)
    }
    func testAnchorPreservesCompleteCanonicalCheckpointBytes() throws {
        let bytes = try checkpoint()
        let value = try KagemushaFinalityTrustAnchorV1(networkID: network, checkpoint: bytes)
        XCTAssertEqual(value.checkpoint, bytes)
        XCTAssertGreaterThan(value.checkpoint.count, 32)
    }
    func testAnchorRejectsEmptyAndOversizedCheckpoint() {
        for bytes in [Data(), Data(repeating: 0, count: KagemushaFinalityTrustAnchorV1.maximumCheckpointBytes + 1)] {
            XCTAssertThrowsError(try KagemushaFinalityTrustAnchorV1(networkID: network, checkpoint: bytes))
        }
    }
    func testAnchorRejectsZeroAndWrongWidthNetworkHashes() throws {
        for bytes in [Data(), Data(repeating: 1, count: 31), Data(repeating: 0, count: 32), Data(repeating: 1, count: 33)] {
            XCTAssertThrowsError(try KagemushaFinalityTrustAnchorV1(networkID: bytes, checkpoint: checkpoint()))
        }
    }
    func testPendingHintIsAbsent() throws { XCTAssertNil(try parseReserveFinalityHint(Data("null".utf8))) }
    func testAnchorRejectsUnmarkedNetworkWithoutNormalizingIt() throws {
        XCTAssertThrowsError(try KagemushaFinalityTrustAnchorV1(networkID: Data(repeating: 4, count: 32), checkpoint: checkpoint()))
    }
    func testHintRejectsUnmarkedCoordinatesWithoutNormalizingThem() {
        for original in ["03", "05"] {
            let json = hint().replacingOccurrences(of: String(repeating: original, count: 32),
                                                   with: String(repeating: "04", count: 32))
            XCTAssertThrowsError(try parseReserveFinalityHint(Data(json.utf8)))
        }
    }
    func testNativeHintKeepsExactUnsignedCoordinates() throws {
        let value = try XCTUnwrap(parseReserveFinalityHint(Data(hint(String(UInt64.max)).utf8)))
        XCTAssertEqual(value.networkID, network); XCTAssertEqual(value.blockHash, context)
        XCTAssertEqual(value.blockHeight, UInt64.max)
    }
    func testHintRejectsNoncanonicalAndOverflowHeight() {
        for height in ["0", "-1", "+7", "07", "7.0", "18446744073709551616", "٧"] {
            XCTAssertThrowsError(try parseReserveFinalityHint(Data(hint(height).utf8)))
        }
    }
    func testHintRejectsUnknownDuplicateFieldsAndInvalidVersion() {
        let original = hint()
        let variants = [original.replacingOccurrences(of: "\"version\":1", with: "\"version\":true"),
            original.replacingOccurrences(of: "\"version\":1", with: "\"version\":1.0"),
            original.replacingOccurrences(of: "\"version\":1", with: "\"version\":2"),
            original.replacingOccurrences(of: "\"version\":1", with: "\"version\":1,\"version\":1"),
            String(original.dropLast()) + ",\"extra\":1}", original + "null"]
        for value in variants { XCTAssertThrowsError(try parseReserveFinalityHint(Data(value.utf8))) }
    }
    func testHintRejectsZeroUppercaseAndWrongWidthHashes() {
        for hash in [String(repeating: "00", count: 32), String(repeating: "AB", count: 32),
                     String(repeating: "03", count: 31), String(repeating: "03", count: 33)] {
            XCTAssertThrowsError(try parseReserveFinalityHint(Data(hint().replacingOccurrences(
                of: String(repeating: "03", count: 32), with: hash).utf8)))
        }
    }
    func testHintRejectsInvalidUtf8AndOversizedData() {
        for data in [Data(), Data([0xc3, 0x28]), Data(repeating: 32, count: 513)] {
            XCTAssertThrowsError(try parseReserveFinalityHint(data))
        }
    }
}
