import XCTest
@testable import IrohaSwift

final class KagemushaScaledAmountTests: XCTestCase {
    func testDecimalConversionUsesExactAssetScale() throws {
        let amount = try KagemushaScaledAmount(decimal: "10.75", scale: 9)
        XCTAssertEqual(amount.atomicUnits, "10750000000")
        XCTAssertEqual(amount.fixedScaleDecimal, "10.750000000")
        XCTAssertEqual(amount.displayDecimal, "10.75")

        let minimum = try KagemushaScaledAmount(decimal: "0.000000001", scale: 9)
        XCTAssertEqual(minimum.atomicUnits, "1")
        XCTAssertEqual(minimum.fixedScaleDecimal, "0.000000001")
    }

    func testAtomicConversionPreservesU128Maximum() throws {
        let amount = try KagemushaScaledAmount(
            atomicUnits: KagemushaScaledAmount.maximumAtomicUnits,
            scale: 0
        )
        XCTAssertEqual(amount.displayDecimal, KagemushaScaledAmount.maximumAtomicUnits)
    }

    func testCheckedAdditionAndSummationPreserveScale() throws {
        let first = try KagemushaScaledAmount(decimal: "4.50", scale: 9)
        let second = try KagemushaScaledAmount(decimal: "6.25", scale: 9)
        let third = try KagemushaScaledAmount(decimal: "0.25", scale: 9)

        let pair = try first.adding(second)
        XCTAssertEqual(pair.atomicUnits, "10750000000")
        XCTAssertEqual(pair.scale, 9)
        XCTAssertEqual(pair.displayDecimal, "10.75")

        let total = try KagemushaScaledAmount.sum([first, second, third])
        XCTAssertEqual(total.atomicUnits, "11000000000")
        XCTAssertEqual(total.scale, 9)
        XCTAssertEqual(total.displayDecimal, "11")
    }

    func testCheckedAdditionRejectsScaleMismatchOverflowAndEmptySum() throws {
        let scaleNine = try KagemushaScaledAmount(atomicUnits: "1", scale: 9)
        let scaleEight = try KagemushaScaledAmount(atomicUnits: "1", scale: 8)
        XCTAssertThrowsError(try scaleNine.adding(scaleEight)) {
            XCTAssertEqual(
                $0 as? KagemushaScaledAmountError,
                .scaleMismatch(expected: 9, actual: 8)
            )
        }

        let maximum = try KagemushaScaledAmount(
            atomicUnits: KagemushaScaledAmount.maximumAtomicUnits,
            scale: 9
        )
        XCTAssertThrowsError(try maximum.adding(scaleNine)) {
            XCTAssertEqual($0 as? KagemushaScaledAmountError, .atomicUnitsOverflow)
        }

        XCTAssertThrowsError(
            try KagemushaScaledAmount.sum([KagemushaScaledAmount]())
        ) {
            XCTAssertEqual($0 as? KagemushaScaledAmountError, .emptyAmountSequence)
        }
        XCTAssertThrowsError(try KagemushaScaledAmount.sum([scaleNine, scaleEight])) {
            XCTAssertEqual(
                $0 as? KagemushaScaledAmountError,
                .scaleMismatch(expected: 9, actual: 8)
            )
        }
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
