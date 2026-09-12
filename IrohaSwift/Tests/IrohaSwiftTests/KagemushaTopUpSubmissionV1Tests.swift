import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaTopUpSubmissionV1Tests: XCTestCase {
    func testValidatorReceivesEntireOriginalBytes() throws {
        let signed = Data([1, 2, 3]), request = Data([4, 5, 6, 7])
        var calls = 0
        let owned = try TopUpSubmissionBytesV1(signed: signed, request: request) { tx, intent in
            calls += 1
            XCTAssertEqual(tx, signed)
            XCTAssertEqual(intent, request)
        }
        XCTAssertEqual(calls, 1)
        XCTAssertEqual(owned.signedTransaction, signed)
        XCTAssertEqual(owned.canonicalRequest, request)
    }
    func testInputMutationDuringValidationCannotReplaceOwnedBytes() throws {
        var signed = Data([1, 2, 3]), request = Data([4, 5, 6])
        let owned = try TopUpSubmissionBytesV1(signed: signed, request: request) { tx, intent in
            signed[0] = 9; request[0] = 8
            XCTAssertEqual(tx, Data([1, 2, 3]))
            XCTAssertEqual(intent, Data([4, 5, 6]))
        }
        XCTAssertEqual(owned.signedTransaction, Data([1, 2, 3]))
        XCTAssertEqual(owned.canonicalRequest, Data([4, 5, 6]))
    }
    func testReturnedBytesCannotMutatePreparedSubmission() throws {
        let owned = try TopUpSubmissionBytesV1(signed: Data([1, 2]), request: Data([3, 4])) { _, _ in }
        var tx = owned.signedTransaction, intent = owned.canonicalRequest
        tx[0] = 9; intent[0] = 9
        XCTAssertEqual(owned.signedTransaction, Data([1, 2]))
        XCTAssertEqual(owned.canonicalRequest, Data([3, 4]))
    }
    func testRejectedValidationCannotProducePreparedBytes() {
        XCTAssertThrowsError(try TopUpSubmissionBytesV1(signed: Data([1]), request: Data([2])) { _, _ in
            throw KagemushaTopUpSubmissionErrorV1.requestMismatch
        }) { XCTAssertEqual($0 as? KagemushaTopUpSubmissionErrorV1, .requestMismatch) }
    }
    func testEmptyInputStopsBeforeNativeValidation() {
        for pair in [(Data(), Data([1])), (Data([1]), Data())] {
            var called = false
            XCTAssertThrowsError(try TopUpSubmissionBytesV1(signed: pair.0, request: pair.1) { _, _ in called = true })
            XCTAssertFalse(called)
        }
    }
    func testOversizedInputStopsBeforeNativeValidation() {
        for pair in [(Data(repeating: 1, count: 16 * 1024 * 1024 + 1), Data([1])),
                     (Data([1]), Data(repeating: 2, count: 16 * 1024 + 1))] {
            var called = false
            XCTAssertThrowsError(try TopUpSubmissionBytesV1(signed: pair.0, request: pair.1) { _, _ in called = true })
            XCTAssertFalse(called)
        }
    }
}
