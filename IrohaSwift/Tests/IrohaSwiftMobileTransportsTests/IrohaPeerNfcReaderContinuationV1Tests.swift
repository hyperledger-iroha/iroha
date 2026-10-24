import Foundation
import XCTest
import IrohaSwift
@testable import IrohaSwiftMobileTransports

final class IrohaPeerNfcRecoveryTestClockV1: @unchecked Sendable {
    private let lock = NSLock()
    private var time: UInt64 = 0
    func read() -> UInt64 { lock.lock(); defer { lock.unlock() }; return time }
    func advance(_ delta: UInt64) { lock.lock(); time += delta; lock.unlock() }
}

private actor IrohaPeerNfcRecoveryTestLatchV1 {
    private var continuation: CheckedContinuation<Void, Never>?
    private var signalled = false
    func wait() async {
        if signalled { return }
        await withCheckedContinuation { continuation = $0 }
    }
    func signal() { signalled = true; continuation?.resume(); continuation = nil }
}

private final class IrohaPeerNfcRecoveryTestCounterV1: @unchecked Sendable {
    private let lock = NSLock()
    private var value = 0
    func increment() { lock.lock(); value += 1; lock.unlock() }
    var count: Int { lock.lock(); defer { lock.unlock() }; return value }
}

final class IrohaPeerNfcReaderContinuationV1Tests: XCTestCase {
    func testLatePreparedPaymentCannotDowngradeRetainedAcknowledgementBytes() {
        let checkpoints = IrohaPeerNfcCheckpointBoxV1(nil)
        checkpoints.storePreparedIfAbsent(Data([7]))
        XCTAssertEqual(checkpoints.load(), Data([7]))
        checkpoints.store(Data([7, 8]))
        // The old RF waiter/factory publication resumes after ACK persistence.
        checkpoints.storePreparedIfAbsent(Data([7]))
        XCTAssertEqual(checkpoints.load(), Data([7, 8]))
    }

#if os(iOS) && canImport(CoreNFC)
    func testDelayedCancellationAndReleaseCannotRetireSuccessorLogicalOperation() throws {
        let reader = IrohaPeerNfcReaderServiceV1(configuration: .init(
            cardAlertMessage: "", readerAlertMessage: "", completionAlertMessage: ""))
        let old = try reader.claimLogicalOperation(checkpoint: Data([7]), onTypedProgress: nil)
        reader.releaseLogicalOperation(old)
        let successor = try reader.claimLogicalOperation(checkpoint: Data([8]), onTypedProgress: nil)
        defer { reader.releaseLogicalOperation(successor) }

        reader.cancel(expectedOwner: old)
        reader.releaseLogicalOperation(old)
        XCTAssertNoThrow(try successor.requireActive())
        XCTAssertEqual(successor.checkpoints.load(), Data([8]))
        XCTAssertThrowsError(try reader.claimLogicalOperation(checkpoint: nil, onTypedProgress: nil)) {
            XCTAssertEqual($0 as? IrohaPeerNfcCoreNFCErrorV1, .operationInProgress)
        }
        reader.cancel(expectedOwner: successor)
        XCTAssertThrowsError(try successor.requireActive()) { XCTAssertTrue($0 is CancellationError) }
    }

    func testImpossiblePaymentBudgetFailsBeforeClaimingSessionOrPreparingValue() async {
        let reader = IrohaPeerNfcReaderServiceV1(configuration: .init(
            cardAlertMessage: "", readerAlertMessage: "", completionAlertMessage: "",
            readerOperationTimeoutMilliseconds: 1))
        // A second call must have the same configuration failure, never a
        // retained operation owner or an attempt to open CoreNFC on Simulator.
        for _ in 0..<2 {
            do {
                _ = try await reader.run(profilePolicy: .init(profile: .kagemushaWalletV1),
                    loadOrCreateDurableCheckpoint: { _, _ in
                        XCTFail("An impossible budget cannot authorize a payment")
                        throw IrohaPeerNfcErrorV1.stateMismatch
                    }, updateDurableCheckpoint: { _ in
                        XCTFail("An impossible budget cannot mutate a checkpoint")
                    })
                XCTFail("The payment preflight must reject the impossible budget")
            } catch {
                XCTAssertEqual(error as? IrohaPeerNfcCoreNFCErrorV1, .invalidReaderBudget)
            }
        }
    }
#endif

    func testRecoveryPollingIsImmediateThenThrottledWithoutExtendingDeadline() {
        let clock = IrohaPeerNfcRecoveryTestClockV1()
        let gate = IrohaPeerNfcRetryGateV1(.default, now: { clock.read() })
        XCTAssertTrue(gate.mayRedetect())
        XCTAssertEqual(gate.restartDelayNanoseconds, 0)
        gate.recordRestart()
        clock.advance(10_000_000)
        XCTAssertEqual(gate.restartDelayNanoseconds, 240_000_000)
        XCTAssertEqual(gate.restartDelayNanoseconds, 240_000_000, "Duplicate callbacks cannot postpone polling")
        clock.advance(240_000_000)
        XCTAssertEqual(gate.restartDelayNanoseconds, 0)
        gate.recordRestart()
        XCTAssertEqual(gate.redetectionTimeoutNanoseconds, 29_750_000_000)
    }

    func testLateDurableCommandKeepsFullReplyDeadlineAndCleanupMayFinishPastAdmissionLimit() {
        let budget = IrohaPeerNfcReaderContinuationBudgetV1(startedAtNanoseconds: 0)
        let deadlines = IrohaPeerNfcReaderCallDeadlinesV1(platformCallMilliseconds: 8_000,
                                                       durableResponseMilliseconds: 35_000)
        XCTAssertEqual(budget.decision(for: .commit, nowNanoseconds: 100_000_000_000,
            sessionRemainingNanoseconds: 50_000_000_000,
            durableCallNanoseconds: deadlines.nanoseconds(for: .commit)), .proceed)
        XCTAssertEqual(deadlines.nanoseconds(for: .commit), 35_000_000_000)
        XCTAssertEqual(budget.decision(for: .readAcknowledgement, nowNanoseconds: 135_000_000_000,
            sessionRemainingNanoseconds: 15_000_000_000,
            durableCallNanoseconds: deadlines.nanoseconds(for: .readAcknowledgement)), .proceed)
        XCTAssertEqual(budget.decision(for: .confirmAcknowledgement, nowNanoseconds: 136_000_000_000,
            sessionRemainingNanoseconds: 14_000_000_000,
            durableCallNanoseconds: deadlines.nanoseconds(for: .confirmAcknowledgement)), .proceed)
        XCTAssertEqual(budget.decision(for: .beginPayment, nowNanoseconds: 120_000_000_000,
            sessionRemainingNanoseconds: 60_000_000_000, durableCallNanoseconds: 35_000_000_000), .expired)
        XCTAssertFalse(budget.mayStartSession(nowNanoseconds: 120_000_000_000))
    }

    func testDurableCommandRenewsBeforeDispatchWhenPhysicalSessionCannotFitItsReply() {
        let budget = IrohaPeerNfcReaderContinuationBudgetV1(startedAtNanoseconds: 0)
        for command in [IrohaPeerNfcOperationV1.beginPayment, .commit] {
            XCTAssertEqual(budget.decision(for: command, nowNanoseconds: 40_000_000_000,
                sessionRemainingNanoseconds: 20_000_000_000, durableCallNanoseconds: 35_000_000_000), .renewSession)
            XCTAssertEqual(budget.decision(for: command, nowNanoseconds: 42_000_000_000,
                sessionRemainingNanoseconds: 60_000_000_000, durableCallNanoseconds: 35_000_000_000), .proceed)
        }
    }

    func testPreparationSurvivesRfWaiterCancellationAndFactoryRunsOnce() async throws {
        let clock = IrohaPeerNfcRecoveryTestClockV1(), counter = IrohaPeerNfcRecoveryTestCounterV1()
        let preparation = IrohaPeerNfcReaderPreparationV1<Data>(now: { clock.read() })
        let latch = IrohaPeerNfcRecoveryTestLatchV1()
        let entered = expectation(description: "one factory entered")
        _ = try preparation.begin(binding: Data([1])) {
            counter.increment(); entered.fulfill(); await latch.wait(); return Data([7, 8])
        }
        await fulfillment(of: [entered], timeout: 1)
        let oldWaiter = Task { try await preparation.value() }
        oldWaiter.cancel()
        do { _ = try await oldWaiter.value; XCTFail("Old RF waiter must cancel") }
        catch is CancellationError { }
        clock.advance(60_000_000_000)
        XCTAssertEqual(preparation.remainingNanoseconds, 60_000_000_000)
        _ = try preparation.begin(binding: Data([1])) {
            XCTFail("Renewing RF cannot dispatch a second factory"); return Data([99])
        }
        await latch.signal()
        let restored = try await preparation.value()
        XCTAssertEqual(restored, Data([7, 8]))
        XCTAssertEqual(counter.count, 1)
    }

    func testDifferentRequestCannotReuseOrDispatchAnotherFactory() throws {
        let preparation = IrohaPeerNfcReaderPreparationV1<Data>()
        defer { preparation.cancel() }
        _ = try preparation.begin(binding: Data([1])) { Data([7]) }
        XCTAssertThrowsError(try preparation.begin(binding: Data([2])) { Data([99]) }) {
            XCTAssertEqual($0 as? IrohaPeerNfcErrorV1, .continuityMismatch)
        }
    }

    func testCompletedAcknowledgementIsStillAvailableAfterItsOriginalDeadline() async throws {
        let clock = IrohaPeerNfcRecoveryTestClockV1()
        let acknowledgement = IrohaPeerNfcReaderPreparationV1<Data>(
            timeoutNanoseconds: 35_000_000_000, now: { clock.read() })
        _ = try acknowledgement.begin(binding: Data([7, 8])) { Data([7, 8]) }
        let first = try await acknowledgement.value()
        XCTAssertEqual(first, Data([7, 8]))
        clock.advance(40_000_000_000)
        let restored = try await acknowledgement.value()
        XCTAssertEqual(restored, Data([7, 8]), "A completed durable save cannot become a later timeout")
        acknowledgement.cancel()
        do { _ = try await acknowledgement.value(); XCTFail("Cancellation must still retire the owner") }
        catch is CancellationError { }
    }

    func testPreparationFinishingAfterItsOwnDeadlineCannotBecomeACompletedResult() async throws {
        let clock = IrohaPeerNfcRecoveryTestClockV1()
        let preparation = IrohaPeerNfcReaderPreparationV1<Data>(now: { clock.read() })
        let latch = IrohaPeerNfcRecoveryTestLatchV1()
        let entered = expectation(description: "factory entered")
        let task = try preparation.begin(binding: Data([1])) {
            entered.fulfill(); await latch.wait(); return Data([7])
        }
        await fulfillment(of: [entered], timeout: 1)
        clock.advance(120_000_000_000)
        await latch.signal()
        do { _ = try await task.value; XCTFail("Late preparation cannot renew RF") }
        catch let error as IrohaPeerNfcOperationDeadlineErrorV1 { XCTAssertEqual(error, .timedOut) }
        do { _ = try await preparation.value(); XCTFail("Joining a late result must keep the deadline failure") }
        catch let error as IrohaPeerNfcOperationDeadlineErrorV1 { XCTAssertEqual(error, .timedOut) }
    }

    func testExplicitCancelReturnsWhilePreparationIgnoresCancellation() async throws {
        let preparation = IrohaPeerNfcReaderPreparationV1<Data>()
        let latch = IrohaPeerNfcRecoveryTestLatchV1()
        let entered = expectation(description: "factory entered")
        _ = try preparation.begin(binding: Data([1])) {
            entered.fulfill(); await latch.wait(); return Data([7])
        }
        await fulfillment(of: [entered], timeout: 1)
        let waiting = Task { try await preparation.value() }
        preparation.cancel()
        do {
            _ = try await irohaPeerNfcWithDeadlineV1(timeoutNanoseconds: 1_000_000_000,
                                                  operation: { try await waiting.value })
            XCTFail("Expected cancellation")
        } catch is CancellationError { }
        await latch.signal()
        XCTAssertThrowsError(try preparation.begin(binding: Data([1])) { Data([99]) })
    }

    func testCancelledLogicalOwnerCannotReopenOrOverwriteSuccessorCheckpoint() async throws {
        let old = IrohaPeerNfcReaderOperationContextV1(checkpoint: nil, onTypedProgress: nil)
        let successor = IrohaPeerNfcReaderOperationContextV1(checkpoint: Data([9]), onTypedProgress: nil)
        let latch = IrohaPeerNfcRecoveryTestLatchV1()
        let entered = expectation(description: "old factory entered")
        _ = try old.preparation.begin(binding: Data([1])) {
            entered.fulfill(); await latch.wait()
            try old.requireActive()
            old.checkpoints.store(Data([7]))
            return Data([7])
        }
        await fulfillment(of: [entered], timeout: 1)
        old.cancel()
        XCTAssertThrowsError(try old.requireActive())
        await latch.signal()
        do { _ = try await old.preparation.currentTask?.value; XCTFail("Late old result must be retired") }
        catch is CancellationError { }
        XCTAssertNil(old.checkpoints.load())
        XCTAssertEqual(successor.checkpoints.load(), Data([9]))
        XCTAssertNoThrow(try successor.requireActive())
        successor.cancel()
    }

    func testAcknowledgementSaveSurvivesRfRetirementBeforeSuccessorUsesItsExactBytes() async throws {
        let owner = IrohaPeerNfcReaderOperationContextV1(checkpoint: Data([7]), onTypedProgress: nil)
        let latch = IrohaPeerNfcRecoveryTestLatchV1(), counter = IrohaPeerNfcRecoveryTestCounterV1()
        let entered = expectation(description: "ACK save entered")
        _ = try owner.acknowledgement.begin(binding: Data([7, 8])) {
            counter.increment(); entered.fulfill(); await latch.wait()
            try owner.requireActive()
            owner.checkpoints.store(Data([7, 8]))
            return Data([7, 8])
        }
        await fulfillment(of: [entered], timeout: 1)
        let waiter = Task { try await owner.acknowledgement.value() }
        waiter.cancel()
        do { _ = try await waiter.value; XCTFail("RF waiter must stop") }
        catch is CancellationError { }
        XCTAssertEqual(owner.checkpoints.load(), Data([7]))
        await latch.signal()
        let saved = try await owner.acknowledgement.value()
        XCTAssertEqual(saved, Data([7, 8]))
        XCTAssertEqual(owner.checkpoints.load(), Data([7, 8]))
        XCTAssertEqual(counter.count, 1)
        owner.cancel()
    }

    func testRenewalKeepsOneProgressTraceAndRetiredRelayCannotFinishItsSuccessor() {
        var events: [IrohaPeerNfcProgressEventV1] = []
        let reporter = IrohaPeerNfcReaderProgressReporterV1 { events.append($0) }
        let first = IrohaPeerNfcReaderProgressRelayV1(reporter)
        first.emit(.tagDetected); first.checkpointRetained(paymentBytes: 120, acknowledgementBytes: nil)
        first.emit(.contactLost); first.invalidate()
        let second = IrohaPeerNfcReaderProgressRelayV1(reporter)
        second.emit(.tagDetected); second.checkpointRetained(paymentBytes: 120, acknowledgementBytes: nil)
        first.emit(.contactLost); first.finishPayment(); first.finish(error: CancellationError())
        second.emit(.paymentCommitted); second.emit(.ackPersisted); second.finishPayment()
        XCTAssertEqual(Set(events.map(\.trace)).count, 1)
        XCTAssertEqual(events.map(\.stage), [.tagDetected, .paymentPrepared, .contactLost,
                                            .tagDetected, .paymentCommitted, .ackPersisted, .complete])
        XCTAssertEqual(events.map(\.attempt), [1, 1, 1, 2, 2, 2, 2])
    }

    func testRenewalRequiresRealMatchingInvalidationNotItsFallbackTimer() throws {
        final class Session {}
        let session = Session()
        var missing = IrohaPeerNfcReaderInvalidationV1<Data>()
        missing.begin(session: ObjectIdentifier(session), result: .failure(IrohaPeerNfcReaderContinuationErrorV1.renewSession))
        let expired = try XCTUnwrap(missing.timedOut(session: ObjectIdentifier(session),
                                                    error: IrohaPeerNfcOperationDeadlineErrorV1.timedOut))
        XCTAssertThrowsError(try expired.get()) {
            XCTAssertEqual($0 as? IrohaPeerNfcOperationDeadlineErrorV1, .timedOut)
        }
        var confirmed = IrohaPeerNfcReaderInvalidationV1<Data>()
        confirmed.begin(session: ObjectIdentifier(session), result: .failure(IrohaPeerNfcReaderContinuationErrorV1.renewSession))
        let result = try XCTUnwrap(confirmed.invalidated(session: ObjectIdentifier(session)))
        XCTAssertThrowsError(try result.get()) { XCTAssertTrue($0 is IrohaPeerNfcReaderContinuationErrorV1) }
    }
}
