import Foundation
import XCTest
@testable import IrohaSwift

final class NativeExecutionEvidenceFixtureTests: XCTestCase {
    func testNativeRustCapturesRetainEverySourceOnOneAndFourLanes() throws {
        for lanes in [1, 4] {
            let rows = try NativeExecutionEvidenceFixtures.inspect(NativeExecutionEvidenceFixtures.load(lanes))
            XCTAssertEqual(rows.count, 8)
            XCTAssertEqual(Set(rows.map(\.logicalID)).count, 8)
            XCTAssertEqual(Set(rows.map { "\($0.laneID):\($0.dataspaceID)" }).count, lanes)
            XCTAssertEqual(Set(rows.map(\.phase)), Set(["warmup", "measurement"]))
        }
    }

    func testEveryProjectedSourceAndRouteFieldBindsCanonicalRows() throws {
        let payload = try NativeExecutionEvidenceFixtures.load(4)
        let root = try XCTUnwrap(JSONSerialization.jsonObject(with: payload) as? [String: Any])
        let rows = try XCTUnwrap(root["requests"] as? [[String: Any]])
        let first = try XCTUnwrap(rows.first)
        for (field, value) in first {
            var changed = first
            if let text = value as? String {
                switch field {
                case "authority": changed[field] = text + "@retired"
                case "phase": changed[field] = "measurement"
                default: changed[field] = String(text.dropLast()) + (text.last == "f" ? "b" : "f")
                }
            } else if let number = value as? NSNumber {
                changed[field] = number.uint64Value + 1
            } else if value is NSNull { changed[field] = [:] as [String: Any]
            } else if value is [String: Any] { changed[field] = NSNull()
            } else { XCTFail("unexpected projection value"); continue }
            var mutated = root
            mutated["requests"] = [changed] + rows.dropFirst()
            XCTAssertThrowsError(try NativeExecutionEvidenceFixtures.inspect(JSONSerialization.data(withJSONObject: mutated)), field)
        }
        let sourceIndex = try XCTUnwrap(rows.firstIndex { $0["lane_source"] is [String: Any] })
        let source = try XCTUnwrap(rows[sourceIndex]["lane_source"] as? [String: Any])
        for field in source.keys {
            var changed = source; changed.removeValue(forKey: field)
            var modified = rows; modified[sourceIndex]["lane_source"] = changed
            var mutated = root; mutated["requests"] = modified
            XCTAssertThrowsError(try NativeExecutionEvidenceFixtures.inspect(JSONSerialization.data(withJSONObject: mutated)), field)
        }
        for changed in [Array(rows.dropFirst()), Array(rows.reversed()), rows + [first]] {
            var mutated = root; mutated["requests"] = changed
            XCTAssertThrowsError(try NativeExecutionEvidenceFixtures.inspect(JSONSerialization.data(withJSONObject: mutated)))
        }
        for field in root.keys {
            var mutated = root; mutated.removeValue(forKey: field)
            XCTAssertThrowsError(try NativeExecutionEvidenceFixtures.inspect(JSONSerialization.data(withJSONObject: mutated)))
        }
        var mutated = root; mutated["finality"] = NSNull()
        XCTAssertThrowsError(try NativeExecutionEvidenceFixtures.inspect(JSONSerialization.data(withJSONObject: mutated)))
    }

    func testFrameRejectsCompressionLayoutPaddingTruncationAndCorruption() throws {
        // Scalar codec data is not a fabricated SignedBlockWire fixture.
        let payload = Data([1, 2, 3, 4])
        let schema = "sdk::native_evidence::ScalarCodecTest"
        let frame = noritoEncode(typeName: schema, payload: payload, flags: NoritoHeader.compactLen, payloadAlignment: 8)
        XCTAssertEqual(try NativeExecutionEvidenceFixtures.frame(frame, schema: schema), payload)
        for end in frame.indices {
            XCTAssertThrowsError(try NativeExecutionEvidenceFixtures.frame(Data(frame.prefix(end)), schema: schema))
        }
        for offset in [0, 4, 5, 6, 22, 23, 31, 39, 40] {
            var changed = frame; changed[offset] ^= 1
            XCTAssertThrowsError(try NativeExecutionEvidenceFixtures.frame(changed, schema: schema))
        }
        XCTAssertThrowsError(try NativeExecutionEvidenceFixtures.frame(frame + Data([0]), schema: schema))
        XCTAssertThrowsError(try NativeExecutionEvidenceFixtures.frame(frame, schema: "other::schema"))
    }

    func testRecordBoundsAndUnsignedMaximum() throws {
        typealias F = NativeExecutionEvidenceFixtures
        var scalar = F.Reader(F.record([F.integer(UInt64.max, 8)]))
        XCTAssertEqual(try scalar.number(8), UInt64.max); try scalar.finish()
        for invalid in [Data([0x80, 0]), Data([0xff, 0xff, 0xff, 0xff, 0x7f]), Data([0x80])] {
            var reader = F.Reader(invalid)
            XCTAssertThrowsError(try reader.field())
        }
        var hostile = F.Reader(Data(repeating: 0xff, count: 8))
        XCTAssertThrowsError(try hostile.sequence { $0 })
        var bytes = F.Reader(F.integer(2, 8) + Data([8, 9]))
        XCTAssertEqual(try bytes.byteVector(), Data([8, 9]))
        var empty = F.Reader(Data(repeating: 0, count: 8))
        XCTAssertTrue(try empty.sequence { $0 }.isEmpty)
    }
}
