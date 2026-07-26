import XCTest
@testable import IrohaSwift

final class ContractAddressV1Tests: XCTestCase {
    func testSubjectDerivationMatchesLockedRustValidationFeeVector() throws {
        XCTAssertEqual(
            try ContractAddressV1.subjectAccountId(
                "sorac1qyqqqqqqqqqqqqz6putm9wv6wkf4r22v02ktg4af7n3n7egq20h5l"
            ),
            "sorauﾛ1PjﾏｶﾏrfDWヱKmDRgH8ﾗﾐsｼﾓｼqSヰcpAKjGﾊﾇD8ﾁpAGH6E4T"
        )
    }

    func testSubjectDerivationRejectsNoncanonicalAddress() {
        XCTAssertThrowsError(try ContractAddressV1.subjectAccountId("not-a-contract")) { error in
            XCTAssertEqual(error as? ContractAddressV1Error, .invalidLiteral)
        }
    }

    func testCompactCanonicalPrimitivesRejectNoncanonicalDecimalSpellings() throws {
        XCTAssertEqual(
            try CanonicalNorito.encodeCompactDecimal("0.25"),
            Data([5, 1, 0, 0, 0, 25, 4, 2, 0, 0, 0])
        )
        for value in ["+1", "01", "-0", "1.0", "0.250"] {
            XCTAssertThrowsError(try CanonicalNorito.encodeCompactDecimal(value))
        }
        XCTAssertThrowsError(try CanonicalNorito.encodeCompactQuantity("-1"))
    }
}
