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

    func testRuntimeUpgradeHeightRejectsRoundedUInt64Maximum() {
        let payload = Data(#"{"ActivatedAt":18446744073709551616}"#.utf8)

        XCTAssertThrowsError(
            try JSONDecoder().decode(ToriiRuntimeUpgradeStatus.self, from: payload)
        )
    }
}
