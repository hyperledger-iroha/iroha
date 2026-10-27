import XCTest
@testable import IrohaSwiftMobileTransports

final class IrohaPeerNfcReaderInvalidationV1Tests: XCTestCase {
    private final class Session {}
    private enum Failure: Error { case cancelled, timedOut, original }

    func testReadWaitsForItsOwnInvalidationAndCompletesOnlyOnce() throws {
        let session = Session(), unrelated = Session()
        var gate = IrohaPeerNfcReaderInvalidationV1<Data>()
        let request = Data([1, 2, 3])
        gate.begin(session: ObjectIdentifier(session), result: .success(request))
        XCTAssertTrue(gate.isPending)
        XCTAssertNil(gate.invalidated(session: ObjectIdentifier(unrelated)))
        XCTAssertTrue(gate.isPending, "Another reader cannot release Face ID for this one")
        XCTAssertEqual(try gate.invalidated(session: ObjectIdentifier(session))?.get(), request)
        XCTAssertFalse(gate.isPending)
        XCTAssertNil(gate.invalidated(session: ObjectIdentifier(session)))
    }

    func testCancellationDuringDismissalCannotPublishSuccessfulRequest() throws {
        let session = Session()
        var gate = IrohaPeerNfcReaderInvalidationV1<Data>()
        gate.begin(session: ObjectIdentifier(session), result: .success(Data([1])))
        gate.cancel(with: Failure.cancelled)
        XCTAssertTrue(gate.isPending, "Cancellation still waits for the system sheet to close")
        let result = try XCTUnwrap(gate.invalidated(session: ObjectIdentifier(session)))
        XCTAssertThrowsError(try result.get()) { XCTAssertTrue($0 is Failure) }
        XCTAssertFalse(gate.isPending)
    }

    func testMissingInvalidationCannotReportSuccessAndLateCallbackCannotFinishSuccessor() throws {
        let first = Session(), second = Session()
        var gate = IrohaPeerNfcReaderInvalidationV1<Data>()
        gate.begin(session: ObjectIdentifier(first), result: .success(Data([1])))
        let timeout = try XCTUnwrap(gate.timedOut(session: ObjectIdentifier(first), error: Failure.timedOut))
        XCTAssertThrowsError(try timeout.get()) {
            guard case Failure.timedOut = $0 else { return XCTFail("Expected dismissal timeout") }
        }
        gate.begin(session: ObjectIdentifier(second), result: .success(Data([2])))
        XCTAssertNil(gate.invalidated(session: ObjectIdentifier(first)))
        XCTAssertNil(gate.timedOut(session: ObjectIdentifier(first), error: Failure.timedOut))
        XCTAssertEqual(try gate.invalidated(session: ObjectIdentifier(second))?.get(), Data([2]))
    }

    func testFinishedPaymentStaysSuccessfulWhenCancelledWhileItsSheetCloses() throws {
        let session = Session()
        var gate = IrohaPeerNfcReaderInvalidationV1<Data>()
        let receipt = Data([7, 7])
        gate.begin(session: ObjectIdentifier(session), result: .success(receipt), retainsSuccess: true)
        gate.cancel(with: Failure.cancelled)
        XCTAssertTrue(gate.isPending, "The sheet still has to close before the result is returned")
        XCTAssertEqual(try gate.invalidated(session: ObjectIdentifier(session))?.get(), receipt,
                       "A durable receipt already published as complete cannot become a cancellation")
    }

    func testFinishedPaymentStaysSuccessfulWhenTheInvalidationCallbackNeverArrives() throws {
        let session = Session()
        var gate = IrohaPeerNfcReaderInvalidationV1<Data>()
        let receipt = Data([7, 8])
        gate.begin(session: ObjectIdentifier(session), result: .success(receipt), retainsSuccess: true)
        let result = try XCTUnwrap(gate.timedOut(session: ObjectIdentifier(session), error: Failure.timedOut))
        XCTAssertEqual(try result.get(), receipt, "A missing callback cannot report a finished payment as failed")
        XCTAssertFalse(gate.isPending)
    }

    func testRetainedSuccessDoesNotHideAFailureOrRetainACancelledRequestRead() throws {
        let session = Session()
        var gate = IrohaPeerNfcReaderInvalidationV1<Data>()
        gate.begin(session: ObjectIdentifier(session), result: .failure(Failure.original), retainsSuccess: true)
        let failed = try XCTUnwrap(gate.timedOut(session: ObjectIdentifier(session), error: Failure.timedOut))
        XCTAssertThrowsError(try failed.get()) {
            guard case Failure.original = $0 else { return XCTFail("Must preserve the original error") }
        }
        gate.begin(session: ObjectIdentifier(session), result: .success(Data([1])))
        gate.cancel(with: Failure.cancelled)
        let read = try XCTUnwrap(gate.invalidated(session: ObjectIdentifier(session)))
        XCTAssertThrowsError(try read.get(), "A cancelled Request read still cannot authorize value")
    }

    func testTimeoutPreservesOriginalFailure() throws {
        let session = Session()
        var gate = IrohaPeerNfcReaderInvalidationV1<Data>()
        gate.begin(session: ObjectIdentifier(session), result: .failure(Failure.original))
        let result = try XCTUnwrap(gate.timedOut(session: ObjectIdentifier(session), error: Failure.timedOut))
        XCTAssertThrowsError(try result.get()) {
            guard case Failure.original = $0 else { return XCTFail("Must preserve the original error") }
        }
    }
}
