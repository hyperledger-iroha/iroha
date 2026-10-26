import Foundation
import IrohaSwift

public struct IrohaPeerNfcReaderRetryPolicyV1: Equatable, Sendable {
    public static let maximumRedetectionTimeoutMilliseconds: UInt64 = 30_000
    public static let maximumRestartIntervalMilliseconds: UInt64 = 1_000
    public let redetectionTimeoutMilliseconds: UInt64
    public let minimumRestartIntervalMilliseconds: UInt64

    public init(
        redetectionTimeoutMilliseconds: UInt64 = 30_000,
        minimumRestartIntervalMilliseconds: UInt64 = 250
    ) {
        precondition(Self.areValid(
            redetectionTimeoutMilliseconds: redetectionTimeoutMilliseconds,
            minimumRestartIntervalMilliseconds: minimumRestartIntervalMilliseconds
        ))
        self.redetectionTimeoutMilliseconds = redetectionTimeoutMilliseconds
        self.minimumRestartIntervalMilliseconds = minimumRestartIntervalMilliseconds
    }

    public static let `default` = IrohaPeerNfcReaderRetryPolicyV1()

    static func areValid(
        redetectionTimeoutMilliseconds: UInt64,
        minimumRestartIntervalMilliseconds: UInt64
    ) -> Bool {
        (1...maximumRedetectionTimeoutMilliseconds).contains(redetectionTimeoutMilliseconds)
            && (1...maximumRestartIntervalMilliseconds).contains(minimumRestartIntervalMilliseconds)
    }
}

/// Allow durable admission and COMMIT work to finish without making discovery, connection or
/// ordinary APDUs wait for a wallet-proof deadline. Cancellation and the reader's
/// whole-operation deadline still apply while any command is outstanding.
struct IrohaPeerNfcReaderCallDeadlinesV1: Equatable, Sendable {
    static let maximumDurableResponseTimeoutMilliseconds: UInt64 = 35_000
    let platformCallMilliseconds: UInt64
    let durableResponseMilliseconds: UInt64

    func nanoseconds(for operation: IrohaPeerNfcOperationV1) -> UInt64 {
        let durable = operation == .beginPayment || operation == .commit
        let milliseconds = durable ? durableResponseMilliseconds : platformCallMilliseconds
        return milliseconds * 1_000_000
    }
}

/// Classifies the two selection responses an HCE peer can emit while its
/// CardSession is still arming. Retrying is deliberately limited to SELECT:
/// the same status words from value-moving commands remain terminal protocol
/// failures, while the reader's contact-attempt gate bounds startup retries.
enum IrohaPeerNfcStartupResponseRetryPolicyV1 {
    static func shouldRetry(
        _ response: IrohaPeerNfcAPDUResponseV1,
        for command: IrohaPeerNfcCommandV1
    ) -> Bool {
        guard case .selectApplication = command else { return false }
        switch response.statusWord {
        case .notFound, .conditionsNotSatisfied:
            return true
        default:
            return false
        }
    }
}

/// The platform-independent deadline used by the Core NFC adapters. The
/// operation may finish after the caller has timed out; its late value is
/// discarded (and optionally cleaned up) instead of being installed into a
/// later NFC operation.
enum IrohaPeerNfcOperationDeadlineErrorV1: Error, Equatable, Sendable {
    case timedOut
}

private final class IrohaPeerNfcDeadlineRaceV1<Value: Sendable>:
    @unchecked Sendable {
    private let lock = NSLock()
    private var continuation: CheckedContinuation<Value, Error>?
    private var pendingResult: Result<Value, Error>?
    private var timeoutTask: Task<Void, Never>?
    private var resolved = false

    func install(_ continuation: CheckedContinuation<Value, Error>) {
        lock.lock()
        if let pendingResult {
            lock.unlock()
            continuation.resume(with: pendingResult)
            return
        }
        self.continuation = continuation
        lock.unlock()
    }

    func installTimeoutTask(_ task: Task<Void, Never>) {
        lock.lock()
        guard !resolved else {
            lock.unlock()
            task.cancel()
            return
        }
        timeoutTask = task
        lock.unlock()
    }

    @discardableResult
    func resolve(
        _ result: Result<Value, Error>,
        cancelTimeout: Bool,
        beforeResumeIfWon: (() -> Void)? = nil
    ) -> Bool {
        lock.lock()
        guard !resolved else {
            lock.unlock()
            return false
        }
        resolved = true
        let timeoutToCancel = cancelTimeout ? timeoutTask : nil
        timeoutTask = nil
        let continuationToResume = continuation
        self.continuation = nil
        if continuationToResume == nil {
            pendingResult = result
        }
        lock.unlock()
        timeoutToCancel?.cancel()
        beforeResumeIfWon?()
        continuationToResume?.resume(with: result)
        return true
    }
}

/// Races an asynchronous platform call against a fixed deadline without
/// structurally waiting for a platform callback that ignores cancellation.
func irohaPeerNfcWithDeadlineV1<Value: Sendable>(
    timeoutNanoseconds: UInt64,
    operation: @escaping @Sendable () async throws -> Value,
    onTimeout: @escaping @Sendable () -> Void = {},
    onCancel: @escaping @Sendable () -> Void = {},
    onDiscardedSuccess: @escaping @Sendable (Value) -> Void = { _ in }
) async throws -> Value {
    precondition(timeoutNanoseconds > 0)
    let race = IrohaPeerNfcDeadlineRaceV1<Value>()
    return try await withTaskCancellationHandler {
        try Task.checkCancellation()
        return try await withCheckedThrowingContinuation { continuation in
            race.install(continuation)
            let timeoutTask = Task.detached {
                do {
                    try await Task.sleep(nanoseconds: timeoutNanoseconds)
                } catch {
                    return
                }
                race.resolve(
                    .failure(IrohaPeerNfcOperationDeadlineErrorV1.timedOut),
                    cancelTimeout: false,
                    beforeResumeIfWon: onTimeout
                )
            }
            race.installTimeoutTask(timeoutTask)
            Task.detached {
                let result: Result<Value, Error>
                do {
                    result = .success(try await operation())
                } catch {
                    result = .failure(error)
                }
                guard race.resolve(result, cancelTimeout: true) else {
                    if case .success(let value) = result {
                        onDiscardedSuccess(value)
                    }
                    return
                }
            }
        }
    } onCancel: {
        race.resolve(
            .failure(CancellationError()),
            cancelTimeout: true,
            beforeResumeIfWon: onCancel
        )
    }
}

/// Cancellation token for a multi-await platform operation. A deadline closes
/// the token before resuming its caller, so a late callback cannot issue a
/// retry or install state after the timeout has won.
final class IrohaPeerNfcAsyncOperationGateV1: @unchecked Sendable {
    private let lock = NSLock()
    private var active = true

    func validate() throws {
        lock.lock()
        let isActive = active
        lock.unlock()
        guard isActive else { throw CancellationError() }
    }

    func invalidate() {
        lock.lock()
        active = false
        lock.unlock()
    }
}

/// A monotonic, non-extending timeout budget used across CardSession
/// construction and initial emulation startup.
struct IrohaPeerNfcDeadlineBudgetV1: Sendable {
    private let deadlineNanoseconds: UInt64

    init(timeoutMilliseconds: UInt64, nowNanoseconds: UInt64) {
        let duration = timeoutMilliseconds.multipliedReportingOverflow(by: 1_000_000)
        precondition(!duration.overflow && duration.partialValue > 0)
        let deadline = nowNanoseconds.addingReportingOverflow(duration.partialValue)
        deadlineNanoseconds = deadline.overflow ? UInt64.max : deadline.partialValue
    }

    func remainingNanoseconds(nowNanoseconds: UInt64) -> UInt64 {
        guard nowNanoseconds < deadlineNanoseconds else { return 0 }
        return deadlineNanoseconds - nowNanoseconds
    }
}

/// One-shot startup signal shared by CardSession creation and its event loop.
/// A stop, startup failure, and successful emulation race through one result.
final class IrohaPeerNfcAsyncSignalV1: @unchecked Sendable {
    private let lock = NSLock()
    private var continuation: CheckedContinuation<Void, Error>?
    private var pendingResult: Result<Void, Error>?
    private var resolved = false

    func wait() async throws {
        try await withCheckedThrowingContinuation { continuation in
            lock.lock()
            if let pendingResult {
                lock.unlock()
                continuation.resume(with: pendingResult)
            } else {
                self.continuation = continuation
                lock.unlock()
            }
        }
    }

    @discardableResult
    func resolve(_ result: Result<Void, Error>) -> Bool {
        lock.lock()
        guard !resolved else {
            lock.unlock()
            return false
        }
        resolved = true
        pendingResult = result
        if let continuation {
            self.continuation = nil
            lock.unlock()
            continuation.resume(with: result)
        } else {
            lock.unlock()
        }
        return true
    }
}

/// Stable progress stages for a first-release IPM1 NFC exchange.
public enum IrohaPeerNfcProgressStageV1: String, CaseIterable, Sendable {
    case phase1SessionActive = "phase1_session_active"
    case tagDetected = "tag_detected"
    /// The selected application answered successfully on this contact attempt.
    case contactEstablished = "contact_established"
    /// The reader is polling again; any previous contact is no longer live.
    case contactLost = "contact_lost"
    case requestRead = "request_read"
    case readerEnded = "reader_ended"
    case ownerAuthRequested = "owner_auth_requested"
    case ownerAuthSucceeded = "owner_auth_succeeded"
    case paymentPrepared = "payment_prepared"
    case phase2SessionActive = "phase2_session_active"
    case paymentCommitted = "payment_committed"
    case ackPersisted = "ack_persisted"
    case complete = "complete"
}

/// Typed progress published by the IPM1 CoreNFC reader. Contact observations may
/// repeat across radio attempts; durable value-boundary stages remain single-shot.
public struct IrohaPeerNfcProgressEventV1: Equatable, Sendable {
    public let trace: String
    public let stage: IrohaPeerNfcProgressStageV1
    public let elapsedMilliseconds: UInt64
    public let attempt: Int
    public let bytes: Int
    public let chunk: Int

    public init(
        trace: String,
        stage: IrohaPeerNfcProgressStageV1,
        elapsedMilliseconds: UInt64,
        attempt: Int,
        bytes: Int,
        chunk: Int
    ) {
        self.trace = trace
        self.stage = stage
        self.elapsedMilliseconds = elapsedMilliseconds
        self.attempt = attempt
        self.bytes = bytes
        self.chunk = chunk
    }

    public var logLine: String {
        "kagemusha_peer_nfc_stage trace=\(trace) stage=\(stage.rawValue) "
            + "elapsed_ms=\(elapsedMilliseconds) attempt=\(attempt) bytes=\(bytes) chunk=\(chunk)"
    }
}

/// Re-detections are contact attempts; durable value-boundary stages remain
/// single-shot even when CoreNFC retries callbacks.
struct IrohaPeerNfcProgressPublicationPolicyV1: Sendable {
    private var emittedStages: Set<String> = []
    private var contactStages: Set<IrohaPeerNfcProgressStageV1> = []
    private var tagDetectionCount = 0

    mutating func attempt(for stage: IrohaPeerNfcProgressStageV1) -> Int? {
        if stage == .tagDetected {
            tagDetectionCount += 1
            contactStages.removeAll()
            return tagDetectionCount
        }
        if stage == .contactEstablished || stage == .contactLost {
            guard contactStages.insert(stage).inserted else { return nil }
            return max(tagDetectionCount, 1)
        }
        guard emittedStages.insert(stage.rawValue).inserted else { return nil }
        return max(tagDetectionCount, 1)
    }
}

/// Per-reader-operation progress publisher. Invalidation is a synchronous
/// delivery barrier: once it returns, no callback admitted by this reporter
/// can still be running and every later emission is suppressed. The recursive
/// lock keeps a progress handler free to cancel its own reader operation.
final class IrohaPeerNfcReaderProgressReporterV1: @unchecked Sendable {
    private let handler: ((IrohaPeerNfcProgressEventV1) -> Void)?
    private let trace = UUID().uuidString
        .replacingOccurrences(of: "-", with: "")
        .lowercased()
    private let startedAt: TimeInterval
    private let now: () -> TimeInterval
    private let terminalHandler: (IrohaPeerNfcReaderTerminalDiagnosticV1) -> Void
    private var lastStage: IrohaPeerNfcProgressStageV1?
    private var lastOperation: IrohaPeerNfcOperationV1?
    private let lock = NSRecursiveLock()
    private var publicationPolicy = IrohaPeerNfcProgressPublicationPolicyV1()
    private var active = true

    init(
        now: @escaping () -> TimeInterval = { ProcessInfo.processInfo.systemUptime },
        terminalHandler: @escaping (IrohaPeerNfcReaderTerminalDiagnosticV1) -> Void = {
            NSLog("%@", $0.logLine)
        },
        handler: ((IrohaPeerNfcProgressEventV1) -> Void)?
    ) {
        self.now = now
        self.startedAt = now()
        self.terminalHandler = terminalHandler
        self.handler = handler
    }

    func emit(
        _ stage: IrohaPeerNfcProgressStageV1,
        bytes: Int = 0,
        chunk: Int = 0
    ) {
        lock.lock()
        defer { lock.unlock() }
        guard active,
              let attempt = publicationPolicy.attempt(for: stage) else {
            return
        }
        let elapsed = max(now() - startedAt, 0)
        let event = IrohaPeerNfcProgressEventV1(
            trace: trace,
            stage: stage,
            elapsedMilliseconds: UInt64(elapsed * 1_000),
            attempt: attempt,
            bytes: max(bytes, 0),
            chunk: max(chunk, 0)
        )
        lastStage = stage
        NSLog("iroha_kagemusha_nfc_ios_reader %@", event.logLine)
        handler?(event)
    }

    /// The checkpoint is already durable. An ACK-bearing resume is cleanup only;
    /// it must not start another payment/commit timing span. Repeated contact
    /// observations still use the reporter's single-shot value-stage policy.
    func checkpointRetained(paymentBytes: Int, acknowledgementBytes: Int?) {
        if let acknowledgementBytes {
            emit(.ackPersisted, bytes: acknowledgementBytes)
        } else {
            emit(.paymentPrepared, bytes: paymentBytes)
        }
    }

    /// Called only by a successful payment exchange's terminal owner. Request-only
    /// reads, failures and cancellation retire the reporter without this event.
    /// This is transport completion, not the application's Native receipt verdict.
    func finishPayment() {
        lock.lock()
        defer { lock.unlock() }
        emit(.complete)
        finish()
    }

    /// Records only the closed command name, never its associated APDU data.
    func record(operation: IrohaPeerNfcOperationV1) {
        lock.lock()
        if active { lastOperation = operation }
        lock.unlock()
    }

    /// The same delivery barrier owns progress retirement and the one terminal
    /// diagnostic. Success and subsequent stale failures publish nothing.
    func finish(error: Error? = nil) {
        lock.lock()
        defer { lock.unlock() }
        guard active else { return }
        active = false
        guard let error else { return }
        let elapsed = max(now() - startedAt, 0)
        terminalHandler(IrohaPeerNfcReaderTerminalDiagnosticV1(
            error: error,
            operation: lastOperation,
            lastStage: lastStage,
            elapsedMilliseconds: UInt64(elapsed * 1_000)
        ))
    }

    func invalidate() {
        finish()
    }
}

/// A per-session synchronous publication barrier around one logical reporter.
/// Renewing the RF session retires only this relay; trace and single-shot value
/// progress stay with the public operation. Old callbacks cannot reach a successor.
final class IrohaPeerNfcReaderProgressRelayV1: @unchecked Sendable {
    private let lock = NSRecursiveLock()
    private let reporter: IrohaPeerNfcReaderProgressReporterV1
    private var active = true

    init(_ reporter: IrohaPeerNfcReaderProgressReporterV1) { self.reporter = reporter }

    func emit(_ stage: IrohaPeerNfcProgressStageV1, bytes: Int = 0, chunk: Int = 0) {
        lock.lock(); defer { lock.unlock() }
        if active { reporter.emit(stage, bytes: bytes, chunk: chunk) }
    }

    func record(operation: IrohaPeerNfcOperationV1) {
        lock.lock(); defer { lock.unlock() }
        if active { reporter.record(operation: operation) }
    }

    func checkpointRetained(paymentBytes: Int, acknowledgementBytes: Int?) {
        lock.lock(); defer { lock.unlock() }
        if active {
            reporter.checkpointRetained(paymentBytes: paymentBytes,
                                        acknowledgementBytes: acknowledgementBytes)
        }
    }

    func finish(error: Error) {
        lock.lock(); defer { lock.unlock() }
        guard active else { return }
        active = false
        reporter.finish(error: error)
    }

    func finishPayment() {
        lock.lock(); defer { lock.unlock() }
        guard active else { return }
        active = false
        reporter.finishPayment()
    }

    func invalidate() {
        lock.lock(); active = false; lock.unlock()
    }
}

/// Serializes a reader operation's non-cancellable synchronous CoreNFC calls
/// with terminal invalidation. Platform callbacks may re-enter cancellation,
/// so this intentionally uses a recursive lock independent of service state.
final class IrohaPeerNfcReaderPlatformCallGateV1: @unchecked Sendable {
    private let lock = NSRecursiveLock()
    private var active = true

    @discardableResult
    func performIfActive(_ operation: () -> Void) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        guard active else { return false }
        operation()
        return true
    }

    func invalidate() {
        lock.lock()
        active = false
        lock.unlock()
    }
}

/// Mutable per-operation APDU limits. A transport rejection retries the same
/// authenticated IPM1 exchange with portable short-APDU chunk sizes.
final class IrohaPeerNfcRetryLimitsBoxV1: @unchecked Sendable {
    static let fallbackReadChunkBytes = 240
    static let fallbackWriteChunkBytes = 203

    private let lock = NSLock()
    private var limits: IrohaPeerNfcLimitsV1

    init(_ limits: IrohaPeerNfcLimitsV1) {
        self.limits = limits
    }

    func load() -> IrohaPeerNfcLimitsV1 {
        lock.lock()
        defer { lock.unlock() }
        return limits
    }

    @discardableResult
    func downgradeForRetry() -> Bool {
        lock.lock()
        let downgraded = IrohaPeerNfcLimitsV1(
            maximumMessageBytes: limits.maximumMessageBytes,
            maximumReadChunkBytes: min(
                limits.maximumReadChunkBytes,
                Self.fallbackReadChunkBytes
            ),
            maximumWriteChunkBytes: min(
                limits.maximumWriteChunkBytes,
                Self.fallbackWriteChunkBytes
            )
        )
        let didChange = downgraded != limits
        limits = downgraded
        lock.unlock()
        return didChange
    }
}

final class IrohaPeerNfcRetryGateV1: @unchecked Sendable {
    private let policy: IrohaPeerNfcReaderRetryPolicyV1
    private let now: @Sendable () -> UInt64
    private let lock = NSLock()
    private var recoveryDeadline: UInt64?
    private var lastRestart: UInt64?

    init(_ policy: IrohaPeerNfcReaderRetryPolicyV1,
         now: @escaping @Sendable () -> UInt64 = { DispatchTime.now().uptimeNanoseconds }) {
        self.policy = policy
        self.now = now
    }

    func beginContactAttempt() -> Bool {
        lock.lock(); defer { lock.unlock() }
        return recoveryDeadline.map { now() < $0 } ?? true
    }

    enum InvalidDetectionDisposition: Equatable, Sendable {
        case retry
        case exhausted
    }

    /// Invalid tags and RF chatter consume elapsed time, never a three-contact
    /// allowance. Polling restarts are throttled under the same fixed budget.
    func recordInvalidDetection() -> InvalidDetectionDisposition {
        mayRedetect() ? .retry : .exhausted
    }

    /// Duplicate callbacks cannot claim the connect slot already in flight.
    func claimContactAttempt(
        operationGate: inout IrohaPeerNfcReaderOperationGateV1,
        capturedEpoch: UInt64
    ) -> Bool {
        guard operationGate.beginConnect(capturedEpoch: capturedEpoch) else { return false }
        guard beginContactAttempt() else {
            _ = operationGate.finishConnect(capturedEpoch: capturedEpoch)
            return false
        }
        return true
    }

    func mayRedetect() -> Bool {
        lock.lock(); defer { lock.unlock() }
        let instant = now()
        if recoveryDeadline == nil {
            recoveryDeadline = instant + policy.redetectionTimeoutMilliseconds * 1_000_000
        }
        return instant < recoveryDeadline!
    }

    /// First RF recovery polls immediately; subsequent restarts have a small
    /// minimum interval so a noisy field cannot create a tight callback loop.
    var restartDelayNanoseconds: UInt64 {
        lock.lock(); defer { lock.unlock() }
        let instant = now()
        let interval = policy.minimumRestartIntervalMilliseconds * 1_000_000
        return max(instant, lastRestart.map { $0 + interval } ?? instant) - instant
    }

    func recordRestart() {
        lock.lock(); lastRestart = now(); lock.unlock()
    }

    var redetectionTimeoutNanoseconds: UInt64 {
        lock.lock(); defer { lock.unlock() }
        guard let deadline = recoveryDeadline else {
            return policy.redetectionTimeoutMilliseconds * 1_000_000
        }
        let instant = now()
        return instant < deadline ? deadline - instant : 0
    }
}


/// A completed CoreNFC session must close before a fresh reader can take over.
/// This internal control-flow result never authorizes another monetary Send.
enum IrohaPeerNfcReaderContinuationErrorV1: Error {
    case renewSession
}

/// Controls admission of new work, not an unrelated timer over in-flight work.
/// A durable APDU admitted just before the logical limit retains its full reply
/// deadline, and ordinary ACK cleanup may finish in the same bounded RF session.
struct IrohaPeerNfcReaderContinuationBudgetV1: Sendable {
    static let startWindowNanoseconds: UInt64 = 120_000_000_000
    static let sessionMarginNanoseconds: UInt64 = 1_000_000_000
    let startedAtNanoseconds: UInt64

    enum Decision: Equatable { case proceed, renewSession, expired }

    func mayStartSession(nowNanoseconds: UInt64) -> Bool {
        nowNanoseconds - min(nowNanoseconds, startedAtNanoseconds) < Self.startWindowNanoseconds
    }

    func decision(for operation: IrohaPeerNfcOperationV1, nowNanoseconds: UInt64,
                  sessionRemainingNanoseconds: UInt64, durableCallNanoseconds: UInt64) -> Decision {
        guard operation == .beginPayment || operation == .commit else { return .proceed }
        guard mayStartSession(nowNanoseconds: nowNanoseconds) else { return .expired }
        return sessionRemainingNanoseconds >= durableCallNanoseconds + Self.sessionMarginNanoseconds
            ? .proceed : .renewSession
    }
}

/// One request-bound factory task survives RF retirement. Session waiters may
/// cancel independently; only the logical owner or this task's own deadline
/// cancels preparation. A second Request can never dispatch another factory.
final class IrohaPeerNfcReaderPreparationV1<Value: Sendable>: @unchecked Sendable {
    private let lock = NSLock()
    private let now: @Sendable () -> UInt64
    private let timeout: UInt64
    private var binding: Data?
    private var task: Task<Value, Error>?
    private var completedResult: Result<Value, Error>?
    private var startedAt: UInt64?
    private var cancelled = false
    private var waiters: [UUID: Task<Value, Error>] = [:]

    init(timeoutNanoseconds: UInt64 = 120_000_000_000,
         now: @escaping @Sendable () -> UInt64 = { DispatchTime.now().uptimeNanoseconds }) {
        precondition(timeoutNanoseconds > 0)
        self.timeout = timeoutNanoseconds
        self.now = now
    }

    func begin(binding: Data, operation: @escaping @Sendable () async throws -> Value) throws -> Task<Value, Error> {
        lock.lock(); defer { lock.unlock() }
        guard !cancelled else { throw CancellationError() }
        if let task {
            guard self.binding == binding else { throw IrohaPeerNfcErrorV1.continuityMismatch }
            return task
        }
        self.binding = binding
        startedAt = now()
        let task = Task {
            let result: Result<Value, Error>
            do {
                try Task.checkCancellation()
                let value = try await operation()
                try Task.checkCancellation()
                result = .success(value)
            } catch { result = .failure(error) }
            return try self.retainCompletion(result).get()
        }
        self.task = task
        return task
    }

    private func retainCompletion(_ result: Result<Value, Error>) -> Result<Value, Error> {
        lock.lock(); defer { lock.unlock() }
        guard !cancelled else { return .failure(CancellationError()) }
        let instant = now(), elapsed = instant - min(instant, startedAt ?? instant)
        let bounded: Result<Value, Error> = elapsed < timeout
            ? result : .failure(IrohaPeerNfcOperationDeadlineErrorV1.timedOut)
        completedResult = bounded
        return bounded
    }

    var currentTask: Task<Value, Error>? {
        lock.lock(); defer { lock.unlock() }; return task
    }

    var remainingNanoseconds: UInt64 {
        lock.lock(); defer { lock.unlock() }
        guard let startedAt else { return timeout }
        let instant = now(), elapsed = instant - min(instant, startedAt)
        return elapsed < timeout ? timeout - elapsed : 0
    }

    func cancel() {
        lock.lock()
        cancelled = true
        let task = task, waiters = Array(self.waiters.values)
        self.waiters.removeAll()
        lock.unlock()
        task?.cancel()
        waiters.forEach { $0.cancel() }
    }

    private func makeWaiter() throws -> (UUID, Task<Value, Error>)? {
        lock.lock(); defer { lock.unlock() }
        guard !cancelled else { throw CancellationError() }
        guard let task, let startedAt else { return nil }
        let id = UUID()
        if let completedResult {
            // A save that finished within its deadline stays completed when a
            // later RF session joins it; its old timer cannot undo durability.
            let waiter = Task { try completedResult.get() }
            waiters[id] = waiter
            return (id, waiter)
        }
        let instant = now(), elapsed = instant - min(instant, startedAt)
        guard elapsed < timeout else { throw IrohaPeerNfcOperationDeadlineErrorV1.timedOut }
        let remaining = timeout - elapsed
        let waiter = Task {
            try await irohaPeerNfcWithDeadlineV1(
                timeoutNanoseconds: remaining,
                operation: { try await task.value },
                onTimeout: { self.cancel() })
        }
        waiters[id] = waiter
        return (id, waiter)
    }

    private func removeWaiter(_ id: UUID) {
        lock.lock(); waiters[id] = nil; lock.unlock()
    }

    private func requireActive() throws {
        try Task.checkCancellation()
        lock.lock(); defer { lock.unlock() }
        if cancelled { throw CancellationError() }
    }

    func value() async throws -> Value? {
        let registered: (UUID, Task<Value, Error>)?
        do { registered = try makeWaiter() }
        catch is IrohaPeerNfcOperationDeadlineErrorV1 {
            cancel()
            throw IrohaPeerNfcOperationDeadlineErrorV1.timedOut
        }
        guard let (id, waiter) = registered else { return nil }
        defer { removeWaiter(id) }
        return try await withTaskCancellationHandler {
            try Task.checkCancellation()
            let value = try await waiter.value
            try requireActive()
            return value
        } onCancel: { waiter.cancel() }
    }
}

/// Epoch guard for asynchronous reader callbacks. A callback captured by a
/// completed operation can never release or mutate the next operation.
struct IrohaPeerNfcReaderOperationGateV1: Sendable {
    private(set) var activeEpoch: UInt64 = 0
    private(set) var connectInFlight = false
    /// Identifies each claimed connect, so a woken timer of an earlier contact
    /// cannot act on a later connect of the same reader session.
    private(set) var connectGeneration: UInt64 = 0

    mutating func beginOperation() -> UInt64 {
        activeEpoch &+= 1
        if activeEpoch == 0 { activeEpoch = 1 }
        connectInFlight = false
        return activeEpoch
    }

    func mayMutate(capturedEpoch: UInt64) -> Bool {
        capturedEpoch != 0 && capturedEpoch == activeEpoch
    }

    mutating func beginConnect(capturedEpoch: UInt64) -> Bool {
        guard mayMutate(capturedEpoch: capturedEpoch), !connectInFlight else {
            return false
        }
        connectInFlight = true
        connectGeneration &+= 1
        return true
    }

    /// The connect claimed as `generation` is still waiting for its platform callback.
    func isConnecting(generation: UInt64) -> Bool {
        connectInFlight && connectGeneration == generation
    }

    mutating func finishConnect(capturedEpoch: UInt64) -> Bool {
        guard mayMutate(capturedEpoch: capturedEpoch), connectInFlight else {
            return false
        }
        connectInFlight = false
        return true
    }

    mutating func finishOperation(capturedEpoch: UInt64) -> Bool {
        guard mayMutate(capturedEpoch: capturedEpoch) else { return false }
        connectInFlight = false
        return true
    }
}

/// Checks lifecycle/cancellation on both sides of an application or RF await.
/// A callback that ignores task cancellation can return, but its stale value
/// cannot be installed or followed by another APDU after a cancel/restart.
func irohaPeerNfcGuardedAwaitV1<Value>(
    validate: () throws -> Void,
    operation: () async throws -> Value
) async throws -> Value {
    try validate()
    let value = try await operation()
    try validate()
    return value
}

/// Deduplicates the terminal card event across stop, cancellation,
/// invalidation, and acknowledgement confirmation paths.
struct IrohaPeerNfcTerminalEventGateV1: Sendable {
    private(set) var didPublish = false
    private(set) var didPublishFailure = false

    mutating func claimFailurePublication() -> Bool {
        guard !didPublishFailure, !didPublish else { return false }
        didPublishFailure = true
        return true
    }

    /// Expiry is actionable only after this presentation started. A canceled
    /// or replaced presentation cannot authorize recovery of a current one.
    mutating func claimSessionDurationFailurePublication(
        isMaximumDuration: Bool, didStartEmulation: Bool,
        isCurrentSession: Bool, stopRequested: Bool
    ) -> Bool {
        guard isMaximumDuration, didStartEmulation, isCurrentSession, !stopRequested else {
            return false
        }
        return claimFailurePublication()
    }

    mutating func claimEndedPublication() -> Bool {
        guard !didPublish else { return false }
        didPublish = true
        return true
    }
}

/// Coordinates asynchronous CardSession creation with synchronous stop.
struct IrohaPeerNfcCardRuntimeStartGateV1: Sendable {
    private(set) var startInFlight = false
    private(set) var stopRequested = false

    mutating func beginStart() -> Bool {
        guard !startInFlight, !stopRequested else { return false }
        startInFlight = true
        return true
    }

    mutating func finishSessionCreation() -> Bool {
        guard startInFlight else { return false }
        startInFlight = false
        return !stopRequested
    }

    mutating func requestStop(hasActiveSessionOrTask: Bool) -> Bool {
        let creationWillSettle = startInFlight
        stopRequested = true
        return !hasActiveSessionOrTask && !creationWillSettle
    }
}
