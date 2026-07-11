import XCTest
@testable import IrohaSwift

final class KagemushaScaledAmountTests: XCTestCase {
    func testDecimalConversionUsesExactAssetScale() throws {
        let amount = try KagemushaScaledAmount(decimal: "10.75", scale: 9)
        XCTAssertEqual(amount.atomicUnits, "10750000000")
        XCTAssertEqual(amount.scaledNumericDecimal, "10.750000000")
        XCTAssertEqual(amount.displayDecimal, "10.75")

        let minimum = try KagemushaScaledAmount(decimal: "0.000000001", scale: 9)
        XCTAssertEqual(minimum.atomicUnits, "1")
        XCTAssertEqual(minimum.scaledNumericDecimal, "0.000000001")
    }

    func testAtomicConversionPreservesU128Maximum() throws {
        let amount = try KagemushaScaledAmount(
            atomicUnits: KagemushaScaledAmount.maximumAtomicUnits,
            scale: 0
        )
        XCTAssertEqual(amount.displayDecimal, KagemushaScaledAmount.maximumAtomicUnits)
    }

    func testRejectsRoundingNonCanonicalAndOverflowInputs() {
        XCTAssertThrowsError(try KagemushaScaledAmount(decimal: "1.001", scale: 2)) {
            XCTAssertEqual($0 as? KagemushaScaledAmountError, .excessPrecision)
        }
        for invalid in ["", "0", ".1", "1.", "+1", "-1", "01", "1e2"] {
            XCTAssertThrowsError(try KagemushaScaledAmount(decimal: invalid, scale: 9), invalid)
        }
        XCTAssertThrowsError(
            try KagemushaScaledAmount(
                atomicUnits: "340282366920938463463374607431768211456",
                scale: 9
            )
        ) {
            XCTAssertEqual($0 as? KagemushaScaledAmountError, .atomicUnitsOverflow)
        }
        XCTAssertThrowsError(try KagemushaScaledAmount(decimal: "1", scale: 29)) {
            XCTAssertEqual($0 as? KagemushaScaledAmountError, .scaleTooLarge)
        }
    }
}
