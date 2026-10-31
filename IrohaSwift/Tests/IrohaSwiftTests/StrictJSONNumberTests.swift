import Foundation
import XCTest
@testable import IrohaSwift

final class StrictJSONNumberTests: XCTestCase {
    func testFloatingIntegerConversionRejectsRoundedUpperBounds() {
        XCTAssertNil(StrictJSONNumber.uint64(from: NSNumber(value: Double(UInt64.max))))
        XCTAssertNil(StrictJSONNumber.int(from: NSNumber(value: Double(Int.max))))

        XCTAssertEqual(
            StrictJSONNumber.uint64(from: NSNumber(value: Double(UInt64.max).nextDown)),
            UInt64(exactly: Double(UInt64.max).nextDown)
        )
        XCTAssertEqual(
            StrictJSONNumber.int(from: NSNumber(value: Double(Int.max).nextDown)),
            Int(exactly: Double(Int.max).nextDown)
        )
    }

    func testUnsignedNSNumberConversionPreservesUInt64AndRejectsIntOverflow() {
        let maximum = NSNumber(value: UInt64.max)

        XCTAssertEqual(StrictJSONNumber.uint64(from: maximum), UInt64.max)
        XCTAssertNil(StrictJSONNumber.int(from: maximum))
    }

    func testSaturatingNanosecondsHandlesNonFiniteAndOversizedIntervals() {
        XCTAssertEqual(StrictJSONNumber.saturatingNanoseconds(from: .nan), 0)
        XCTAssertEqual(StrictJSONNumber.saturatingNanoseconds(from: -.infinity), 0)
        XCTAssertEqual(StrictJSONNumber.saturatingNanoseconds(from: .infinity), UInt64.max)
        XCTAssertEqual(
            StrictJSONNumber.saturatingNanoseconds(from: Double(UInt64.max)),
            UInt64.max
        )
        XCTAssertEqual(StrictJSONNumber.saturatingNanoseconds(from: 1.25), 1_250_000_000)
    }

    func testConnectStringDecodeRejectsRoundedIntMaximum() {
        let payload = Data(#"{"ip":9223372036854775808,"sessions":1}"#.utf8)

        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiConnectPerIpSessions.self, from: payload)
        )
    }

    func testRuntimeChangeSetStatusHeightRejectsRoundedUInt64Maximum() {
        let payload = Data(#"{"kind":"Activated","content":18446744073709551616}"#.utf8)

        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiRuntimeChangeSetStatus.self, from: payload)
        )
    }

    func testRuntimeChangeSetStatusHeightRequiresAnExactUnquotedIntegerToken() throws {
        func decodeExact(_ json: String) throws -> ToriiRuntimeChangeSetStatus {
            let data = Data(json.utf8)
            let decoder = JSONDecoder()
            decoder.userInfo[exactJSONNumberLexemesUserInfoKey] =
                try ExactJSONNumberLexemeScanner.scan(data)
            return try decoder.decode(ToriiRuntimeChangeSetStatus.self, from: data)
        }

        for content in ["4200.0", "4.2e3", "\"4200\"", "-1", "18446744073709551616"] {
            XCTAssertThrowsError(
                try decodeExact("{\"kind\":\"Lapsed\",\"content\":\(content)}"),
                content
            )
        }
        // A status height is a block height, not a first-release proposal number.
        XCTAssertEqual(
            try decodeExact(#"{"kind":"Lapsed","content":18446744073709551615}"#),
            .lapsed(height: .max)
        )
    }

    func testRuntimeNoticeRejectsValuesBeyondTheExactJSONMaximum() throws {
        let maximum = Data(#"{"blocks":9007199254740991,"ms":0,"approval_cushion_ms":1}"#.utf8)
        XCTAssertEqual(
            try JSONDecoder().decode(ToriiRuntimeNotice.self, from: maximum).blocks,
            9_007_199_254_740_991
        )
        for json in [
            #"{"blocks":9007199254740992,"ms":0,"approval_cushion_ms":1}"#,
            #"{"blocks":1,"ms":9007199254740992,"approval_cushion_ms":1}"#,
            #"{"blocks":1,"ms":0,"approval_cushion_ms":18446744073709551615}"#,
        ] {
            XCTAssertThrowsError(
                try JSONDecoder().decode(ToriiRuntimeNotice.self, from: Data(json.utf8)),
                json
            )
        }
    }
}
