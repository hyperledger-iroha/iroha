import Foundation
import IrohaSwift

public struct IrohaPeerNfcReaderRetryPolicyV1: Equatable, Sendable {
    public static let maximumContactAttemptsLimit = 10
    public static let maximumRedetectionTimeoutMilliseconds: UInt64 = 30_000
    public let maximumContactAttempts: Int
    public let redetectionTimeoutMilliseconds: UInt64

    public init(
        maximumContactAttempts: Int = 3,
        redetectionTimeoutMilliseconds: UInt64 = 3_000
    ) {
        precondition(Self.areValid(
            maximumContactAttempts: maximumContactAttempts,
            redetectionTimeoutMilliseconds: redetectionTimeoutMilliseconds
        ))
        self.maximumContactAttempts = maximumContactAttempts
        self.redetectionTimeoutMilliseconds = redetectionTimeoutMilliseconds
    }

    public static let `default` = IrohaPeerNfcReaderRetryPolicyV1()

    static func areValid(
        maximumContactAttempts: Int,
        redetectionTimeoutMilliseconds: UInt64
    ) -> Bool {
        (1...maximumContactAttemptsLimit).contains(maximumContactAttempts)
            && (1...maximumRedetectionTimeoutMilliseconds)
                .contains(redetectionTimeoutMilliseconds)
    }
}

/// Stable progress stages for a first-release IPM1 NFC exchange.
public enum IrohaPeerNfcProgressStageV1: String, CaseIterable, Sendable {
    case phase1SessionActive = "phase1_session_active"
    case tagDetected = "tag_detected"
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

/// Typed monotonic progress published by the IPM1 CoreNFC reader.
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
        "offline_peer_nfc_stage trace=\(trace) stage=\(stage.rawValue) "
            + "elapsed_ms=\(elapsedMilliseconds) attempt=\(attempt) bytes=\(bytes) chunk=\(chunk)"
    }
}

/// Re-detections are contact attempts; durable value-boundary stages remain
/// single-shot even when CoreNFC retries callbacks.
struct IrohaPeerNfcProgressPublicationPolicyV1: Sendable {
    private var emittedStages: Set<String> = []
    private var tagDetectionCount = 0

    mutating func attempt(for stage: IrohaPeerNfcProgressStageV1) -> Int? {
        if stage == .tagDetected {
            tagDetectionCount += 1
            return tagDetectionCount
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
    private let startedAt = ProcessInfo.processInfo.systemUptime
    private let lock = NSRecursiveLock()
    private var publicationPolicy = IrohaPeerNfcProgressPublicationPolicyV1()
    private var active = true

    init(handler: ((IrohaPeerNfcProgressEventV1) -> Void)?) {
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
        let elapsed = max(ProcessInfo.processInfo.systemUptime - startedAt, 0)
        let event = IrohaPeerNfcProgressEventV1(
            trace: trace,
            stage: stage,
            elapsedMilliseconds: UInt64(elapsed * 1_000),
            attempt: attempt,
            bytes: max(bytes, 0),
            chunk: max(chunk, 0)
        )
        NSLog("iroha_offline_nfc_ios_reader %@", event.logLine)
        handler?(event)
    }

    func invalidate() {
        lock.lock()
        active = false
        lock.unlock()
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
    private let lock = NSLock()
    private var attempts = 0

    init(_ policy: IrohaPeerNfcReaderRetryPolicyV1) { self.policy = policy }

    func beginContactAttempt() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        guard attempts < policy.maximumContactAttempts else { return false }
        attempts += 1
        return true
    }

    /// Claims the operation's connect slot before consuming a retry. Callers
    /// hold their operation lock around this helper, making duplicate CoreNFC
    /// detections unable to spend the attempt owned by an in-flight connect.
    func claimContactAttempt(
        operationGate: inout IrohaPeerNfcReaderOperationGateV1,
        capturedEpoch: UInt64
    ) -> Bool {
        guard operationGate.beginConnect(capturedEpoch: capturedEpoch) else {
            return false
        }
        guard beginContactAttempt() else {
            _ = operationGate.finishConnect(capturedEpoch: capturedEpoch)
            return false
        }
        return true
    }

    func mayRedetect() -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return attempts < policy.maximumContactAttempts
    }

    var redetectionTimeoutNanoseconds: UInt64 {
        policy.redetectionTimeoutMilliseconds * 1_000_000
    }
}

/// Epoch guard for asynchronous reader callbacks. A callback captured by a
/// completed operation can never release or mutate the next operation.
struct IrohaPeerNfcReaderOperationGateV1: Sendable {
    private(set) var activeEpoch: UInt64 = 0
    private(set) var connectInFlight = false

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
        return true
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
