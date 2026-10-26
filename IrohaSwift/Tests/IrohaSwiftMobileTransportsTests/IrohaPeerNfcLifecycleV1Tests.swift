import XCTest
import IrohaSwift
@testable import IrohaSwiftMobileTransports

private actor IrohaPeerNfcLifecycleLatchV1 {
    private var continuation: CheckedContinuation<Void, Never>?
    private var signalled = false

    func wait() async {
        if signalled { return }
        await withCheckedContinuation { continuation = $0 }
    }

    func signal() {
        signalled = true
        continuation?.resume()
        continuation = nil
    }
}

private final class IrohaPeerNfcLifecycleProbeV1: @unchecked Sendable {
    private let lock = NSLock()
    private var discardedValues: [Int] = []

    func appendDiscarded(_ value: Int) {
        lock.lock()
        discardedValues.append(value)
        lock.unlock()
    }

    var discarded: [Int] {
        lock.lock()
        defer { lock.unlock() }
        return discardedValues
    }
}

final class IrohaPeerNfcLifecycleV1Tests: XCTestCase {
    func testOnlyAdmissionAndCommitUseTheDurableResponseDeadline() {
        let deadlines = IrohaPeerNfcReaderCallDeadlinesV1(
            platformCallMilliseconds: 8_000, durableResponseMilliseconds: 35_000)
        for operation in IrohaPeerNfcOperationV1.allCases {
            XCTAssertEqual(deadlines.nanoseconds(for: operation),
                           [.beginPayment, .commit].contains(operation) ? 35_000_000_000 : 8_000_000_000,
                           "Only durable admission and credit may extend a command's response wait")
        }
    }

    func testDefaultRecoveryKeepsPollingForThirtySecondsWithoutDelayingTheNextContact() {
        let gate = IrohaPeerNfcRetryGateV1(.default)
        var ownership = IrohaPeerNfcReaderOperationGateV1()
        let epoch = ownership.beginOperation()
        XCTAssertTrue(gate.claimContactAttempt(operationGate: &ownership, capturedEpoch: epoch))
        XCTAssertTrue(ownership.finishConnect(capturedEpoch: epoch))
        XCTAssertEqual(gate.redetectionTimeoutNanoseconds, 30_000_000_000)
        XCTAssertTrue(gate.mayRedetect())
        XCTAssertTrue(gate.claimContactAttempt(operationGate: &ownership, capturedEpoch: epoch),
                      "The recovery budget is a deadline, never a pause before retry")
    }

#if os(iOS) && canImport(CoreNFC)
    func testDurableResponseConfigurationKeepsNormalCallsShortAndChecksBounds() {
        let defaultConfiguration = IrohaPeerNfcCoreNFCConfigurationV1(
            cardAlertMessage: "", readerAlertMessage: "", completionAlertMessage: "")
        XCTAssertEqual(defaultConfiguration.durableResponseTimeoutMilliseconds, 35_000)
        XCTAssertEqual(defaultConfiguration.readerOperationTimeoutMilliseconds, 60_000)
        XCTAssertTrue(IrohaPeerNfcCoreNFCConfigurationV1.isValidDurableReaderBudget(
            readerMilliseconds: 60_000, durableMilliseconds: 35_000))
        XCTAssertTrue(IrohaPeerNfcCoreNFCConfigurationV1.isValidDurableReaderBudget(
            readerMilliseconds: 36_000, durableMilliseconds: 35_000))
        XCTAssertFalse(IrohaPeerNfcCoreNFCConfigurationV1.isValidDurableReaderBudget(
            readerMilliseconds: 30_000, durableMilliseconds: 35_000))
        XCTAssertFalse(IrohaPeerNfcCoreNFCConfigurationV1.isValidDurableReaderBudget(
            readerMilliseconds: 35_000, durableMilliseconds: 35_000))
        let shortNonPaymentConfiguration = IrohaPeerNfcCoreNFCConfigurationV1(
            cardAlertMessage: "", readerAlertMessage: "", completionAlertMessage: "",
            readerOperationTimeoutMilliseconds: 1)
        XCTAssertEqual(shortNonPaymentConfiguration.readerOperationTimeoutMilliseconds, 1,
                       "Request-only readers and card mode must still permit short bounds")
        XCTAssertEqual(defaultConfiguration.durabilityTimeoutMilliseconds, 30_000)
        XCTAssertTrue(IrohaPeerNfcCoreNFCConfigurationV1.isValidDurabilityTimeout(30_000))
        XCTAssertFalse(IrohaPeerNfcCoreNFCConfigurationV1.isValidDurabilityTimeout(30_001))
        XCTAssertFalse(IrohaPeerNfcCoreNFCConfigurationV1.isValidDurabilityTimeout(0))
        let configuration = IrohaPeerNfcCoreNFCConfigurationV1(
            cardAlertMessage: "", readerAlertMessage: "", completionAlertMessage: "",
            platformCallTimeoutMilliseconds: 8_000, durableResponseTimeoutMilliseconds: 35_000)
        XCTAssertEqual(configuration.readerCallDeadlines.nanoseconds(for: .commit), 35_000_000_000)
        XCTAssertEqual(configuration.readerCallDeadlines.nanoseconds(for: .beginPayment), 35_000_000_000)
        XCTAssertEqual(configuration.readerCallDeadlines.nanoseconds(for: .getInfo), 8_000_000_000)
        XCTAssertTrue(IrohaPeerNfcCoreNFCConfigurationV1.isValidDurableResponseTimeout(1))
        XCTAssertTrue(IrohaPeerNfcCoreNFCConfigurationV1.isValidDurableResponseTimeout(35_000))
        XCTAssertFalse(IrohaPeerNfcCoreNFCConfigurationV1.isValidDurableResponseTimeout(0))
        XCTAssertFalse(IrohaPeerNfcCoreNFCConfigurationV1.isValidDurableResponseTimeout(35_001))
    }
#endif

    func testDeadlineReturnsCompletedValue() async throws {
        let value = try await irohaPeerNfcWithDeadlineV1(
            timeoutNanoseconds: 1_000_000_000,
            operation: { 42 }
        )

        XCTAssertEqual(value, 42)
    }

    func testDeadlineDiscardsLatePlatformValueExactlyOnce() async {
        let latch = IrohaPeerNfcLifecycleLatchV1()
        let probe = IrohaPeerNfcLifecycleProbeV1()
        let operationEntered = expectation(description: "platform operation entered")
        let cleanupObserved = expectation(description: "late cleanup observed")

        do {
            _ = try await irohaPeerNfcWithDeadlineV1(
                timeoutNanoseconds: 20_000_000,
                operation: {
                    operationEntered.fulfill()
                    await latch.wait()
                    return 7
                },
                onDiscardedSuccess: { value in
                    probe.appendDiscarded(value)
                    cleanupObserved.fulfill()
                }
            )
            XCTFail("the deadline must win")
        } catch let error as IrohaPeerNfcOperationDeadlineErrorV1 {
            XCTAssertEqual(error, .timedOut)
        } catch {
            XCTFail("unexpected deadline error: \(error)")
        }

        await fulfillment(of: [operationEntered], timeout: 1)
        await latch.signal()
        await fulfillment(of: [cleanupObserved], timeout: 1)
        XCTAssertEqual(probe.discarded, [7])
    }

    func testParentCancellationWinsAndCleansUpLateValue() async {
        let latch = IrohaPeerNfcLifecycleLatchV1()
        let probe = IrohaPeerNfcLifecycleProbeV1()
        let operationEntered = expectation(description: "platform operation entered")
        let cleanupObserved = expectation(description: "cancel cleanup observed")
        let task = Task {
            try await irohaPeerNfcWithDeadlineV1(
                timeoutNanoseconds: 5_000_000_000,
                operation: {
                    operationEntered.fulfill()
                    await latch.wait()
                    return 9
                },
                onDiscardedSuccess: { value in
                    probe.appendDiscarded(value)
                    cleanupObserved.fulfill()
                }
            )
        }

        await fulfillment(of: [operationEntered], timeout: 1)
        task.cancel()
        do {
            _ = try await task.value
            XCTFail("cancelled deadline race returned a value")
        } catch is CancellationError {
            // Expected.
        } catch {
            XCTFail("unexpected cancellation error: \(error)")
        }
        await latch.signal()
        await fulfillment(of: [cleanupObserved], timeout: 1)
        XCTAssertEqual(probe.discarded, [9])
    }

    func testStartupBudgetNeverExtendsAcrossCalls() {
        let budget = IrohaPeerNfcDeadlineBudgetV1(
            timeoutMilliseconds: 10_000,
            nowNanoseconds: 1_000
        )

        XCTAssertEqual(
            budget.remainingNanoseconds(nowNanoseconds: 2_000),
            9_999_999_000
        )
        XCTAssertEqual(
            budget.remainingNanoseconds(nowNanoseconds: 10_000_001_000),
            0
        )
        XCTAssertEqual(
            budget.remainingNanoseconds(nowNanoseconds: 20_000_000_000),
            0
        )
    }

    func testInvalidAndMultipleTagsCannotExtendTheElapsedRecoveryBudget() {
        let clock = IrohaPeerNfcRecoveryTestClockV1()
        let gate = IrohaPeerNfcRetryGateV1(.default, now: { clock.read() })
        for _ in 0..<10 { XCTAssertEqual(gate.recordInvalidDetection(), .retry) }
        clock.advance(30_000_000_000)
        XCTAssertEqual(gate.recordInvalidDetection(), .exhausted)
        XCTAssertFalse(gate.mayRedetect())
        XCTAssertFalse(gate.beginContactAttempt())
    }

    func testStartupSignalPublishesOnlyItsFirstTerminalResult() async throws {
        let signal = IrohaPeerNfcAsyncSignalV1()

        XCTAssertTrue(signal.resolve(.success(())))
        XCTAssertFalse(signal.resolve(.failure(CancellationError())))
        try await signal.wait()
        try await signal.wait()
    }

    func testOnlyInitialSelectNotReadyResponsesAreRetryable() {
        for status in [
            IrohaPeerNfcStatusWordV1.notFound,
            .conditionsNotSatisfied,
        ] {
            XCTAssertTrue(IrohaPeerNfcStartupResponseRetryPolicyV1.shouldRetry(
                IrohaPeerNfcAPDUResponseV1(statusWord: status),
                for: .selectApplication(.protocolDefault)
            ))
            XCTAssertFalse(IrohaPeerNfcStartupResponseRetryPolicyV1.shouldRetry(
                IrohaPeerNfcAPDUResponseV1(statusWord: status),
                for: .getInfo
            ))
        }
        XCTAssertFalse(IrohaPeerNfcStartupResponseRetryPolicyV1.shouldRetry(
            IrohaPeerNfcAPDUResponseV1(statusWord: .success),
            for: .selectApplication(.protocolDefault)
        ))
    }

    func testCoreNfcAdapterDeclaresFiniteLifecycleDefaults() throws {
        let packageRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let source = try String(
            contentsOf: packageRoot.appendingPathComponent(
                "Sources/IrohaSwiftMobileTransports/IrohaPeerNfcCoreNFCV1.swift"
            ),
            encoding: .utf8
        )

        XCTAssertTrue(source.contains("readerOperationTimeoutMilliseconds: UInt64 = 60_000"))
        XCTAssertTrue(source.contains("platformCallTimeoutMilliseconds: UInt64 = 3_000"))
        XCTAssertTrue(source.contains("cardSessionStartupTimeoutMilliseconds: UInt64 = 10_000"))
        XCTAssertTrue(source.contains("maximumReaderOperationTimeoutMilliseconds: UInt64 = 60_000"))
        XCTAssertTrue(source.contains("readerOperationTimedOut"))
        XCTAssertTrue(source.contains("platformCallTimedOut"))
        XCTAssertTrue(source.contains("withTaskCancellationHandler"))
    }

    func testCardSessionAvailabilityKeepsAppleReaderPreflight() throws {
        let packageRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let source = try String(
            contentsOf: packageRoot.appendingPathComponent(
                "Sources/IrohaSwiftMobileTransports/IrohaPeerNfcCoreNFCV1.swift"
            ),
            encoding: .utf8
        )
        let start = try XCTUnwrap(source.range(
            of: "private static func requireAvailability("
        ))
        let end = try XCTUnwrap(source.range(
            of: "\n}\n\n@available(iOS 17.4, *)\nprivate final class IrohaPeerNfcCardRuntimeV1",
            range: start.upperBound..<source.endIndex
        ))
        let cardAvailability = source[start.lowerBound..<end.lowerBound]

        XCTAssertTrue(cardAvailability.contains("CardSession.isSupported"))
        XCTAssertTrue(cardAvailability.contains("CardSession.isEligible"))
        XCTAssertTrue(
            cardAvailability.contains("NFCReaderSession.readingAvailable"),
            "CardSession startup must preserve Apple's NFC reader availability preflight."
        )
    }

    func testCardSessionStartupFailuresRemainDistinct() throws {
        let packageRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let source = try String(
            contentsOf: packageRoot.appendingPathComponent(
                "Sources/IrohaSwiftMobileTransports/IrohaPeerNfcCoreNFCV1.swift"
            ),
            encoding: .utf8
        )
        let start = try XCTUnwrap(source.range(
            of: "private static func mapCardStartupError("
        ))
        let end = try XCTUnwrap(source.range(
            of: "\n    private static func mapCardFailure(",
            range: start.upperBound..<source.endIndex
        ))
        let startupMapping = source[start.lowerBound..<end.lowerBound]

        XCTAssertTrue(source.contains("case cardSessionSystemUnavailable"))
        XCTAssertTrue(source.contains("case cardSessionAccessNotAccepted"))
        XCTAssertTrue(source.contains("case cardSessionRadioDisabled"))
        XCTAssertTrue(startupMapping.contains(
            "case .systemNotAvailable, .emulationStopped, .transmissionError:"
        ))
        XCTAssertTrue(startupMapping.contains(
            "return IrohaPeerNfcCoreNFCErrorV1.cardSessionSystemUnavailable"
        ))
        XCTAssertTrue(startupMapping.contains("case .accessNotAccepted:"))
        XCTAssertTrue(startupMapping.contains(
            "return IrohaPeerNfcCoreNFCErrorV1.cardSessionAccessNotAccepted"
        ))
        XCTAssertTrue(startupMapping.contains("case .radioDisabled:"))
        XCTAssertTrue(startupMapping.contains(
            "return IrohaPeerNfcCoreNFCErrorV1.cardSessionRadioDisabled"
        ))
    }

    func testCardSessionEventInvalidationPreservesStartupFailureReason() throws {
        let packageRoot = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let source = try String(
            contentsOf: packageRoot.appendingPathComponent(
                "Sources/IrohaSwiftMobileTransports/IrohaPeerNfcCoreNFCV1.swift"
            ),
            encoding: .utf8
        )
        let runStart = try XCTUnwrap(source.range(
            of: "private func run(_ session: CardSession) async {"
        ))
        let runEnd = try XCTUnwrap(source.range(
            of: "\n    private func ensureEmulation(",
            range: runStart.upperBound..<source.endIndex
        ))
        let run = source[runStart.lowerBound..<runEnd.lowerBound]
        let invalidationStart = try XCTUnwrap(run.range(
            of: "case .sessionInvalidated(let reason):"
        ))
        let invalidationEnd = try XCTUnwrap(run.range(
            of: "\n                @unknown default:",
            range: invalidationStart.upperBound..<run.endIndex
        ))
        let invalidation = run[
            invalidationStart.lowerBound..<invalidationEnd.lowerBound
        ]

        XCTAssertTrue(invalidation.contains("startupSignal.resolve("))
        XCTAssertTrue(invalidation.contains(
            ".failure(Self.mapCardStartupError(reason))"
        ))
        XCTAssertFalse(invalidation.contains(
            ".failure(IrohaPeerNfcCoreNFCErrorV1.cancelled)"
        ))
    }
}
