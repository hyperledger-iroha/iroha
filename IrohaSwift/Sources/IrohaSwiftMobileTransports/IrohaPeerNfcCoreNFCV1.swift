import Foundation
import IrohaSwift

enum IrohaPeerNfcDurabilityDeadlineErrorV1: Error, Equatable, Sendable {
    case timedOut
    case saturated
}

/// One process-wide, queue-free durability lease shared by admission and
/// COMMIT. Timeout/cancellation never releases a lease whose application
/// callback is still running; process restart is the recovery for a callback
/// that never returns.
final class IrohaPeerNfcDurabilityLeaseGateV1: @unchecked Sendable {
    struct Lease: Equatable, Sendable {
        fileprivate let identifier: UInt64
    }

    static let shared = IrohaPeerNfcDurabilityLeaseGateV1()

    private let lock = NSLock()
    private var nextIdentifier: UInt64 = 1
    private var activeIdentifier: UInt64?

    func acquire() -> Lease? {
        lock.lock()
        defer { lock.unlock() }
        guard activeIdentifier == nil else { return nil }
        let identifier = nextIdentifier
        nextIdentifier &+= 1
        if nextIdentifier == 0 { nextIdentifier = 1 }
        activeIdentifier = identifier
        return Lease(identifier: identifier)
    }

    func release(_ lease: Lease) {
        lock.lock()
        if activeIdentifier == lease.identifier { activeIdentifier = nil }
        lock.unlock()
    }

    var isOccupied: Bool {
        lock.lock()
        defer { lock.unlock() }
        return activeIdentifier != nil
    }
}

private final class IrohaPeerNfcDurabilityRaceV1<Value: Sendable>:
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
        if resolved {
            lock.unlock()
            continuation.resume(throwing: CancellationError())
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

    func resolve(
        _ result: Result<Value, Error>,
        cancelTimeout: Bool
    ) {
        lock.lock()
        guard !resolved else {
            lock.unlock()
            return
        }
        resolved = true
        let timeoutToCancel = cancelTimeout ? timeoutTask : nil
        timeoutTask = nil
        if let continuation {
            self.continuation = nil
            lock.unlock()
            timeoutToCancel?.cancel()
            continuation.resume(with: result)
        } else {
            pendingResult = result
            lock.unlock()
            timeoutToCancel?.cancel()
        }
    }
}

/// Races an application durability callback against a fixed deadline without
/// structurally awaiting a child that ignores task cancellation. A late value
/// is discarded; durable storage is restored by the next receiver start.
func irohaPeerNfcWithDurabilityDeadlineV1<Value: Sendable>(
    timeoutNanoseconds: UInt64,
    leaseGate: IrohaPeerNfcDurabilityLeaseGateV1 = .shared,
    operation: @escaping @Sendable () async throws -> Value
) async throws -> Value {
    precondition(timeoutNanoseconds > 0)
    try Task.checkCancellation()
    guard let lease = leaseGate.acquire() else {
        throw IrohaPeerNfcDurabilityDeadlineErrorV1.saturated
    }
    let race = IrohaPeerNfcDurabilityRaceV1<Value>()
    return try await withTaskCancellationHandler {
        try await withCheckedThrowingContinuation { continuation in
            race.install(continuation)
            let timeoutTask = Task.detached {
                do {
                    try await Task.sleep(nanoseconds: timeoutNanoseconds)
                } catch {
                    return
                }
                race.resolve(
                    .failure(IrohaPeerNfcDurabilityDeadlineErrorV1.timedOut),
                    cancelTimeout: false
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
                // Releasing here, and nowhere in timeout/cancellation paths,
                // makes a hung callback consume exactly one process-wide slot.
                leaseGate.release(lease)
                race.resolve(result, cancelTimeout: true)
            }
        }
    } onCancel: {
        race.resolve(.failure(CancellationError()), cancelTimeout: true)
    }
}

#if os(iOS) && canImport(CoreNFC)
@preconcurrency import CoreNFC

public enum IrohaPeerNfcCoreNFCErrorV1: Error, LocalizedError {
    case invalidCommandAPDU
    case unavailable
    case cardEmulationUnavailable
    case cardEmulationIneligible
    case runtimeDisabled
    case invalidTag
    case cancelled
    case operationInProgress
    case retryExhausted

    public var errorDescription: String? {
        switch self {
        case .invalidCommandAPDU:
            return "CoreNFC could not represent the Iroha peer NFC V1 command."
        case .unavailable:
            return "NFC reading is unavailable on this device."
        case .cardEmulationUnavailable:
            return "NFC card emulation is unavailable on this device."
        case .cardEmulationIneligible:
            return "This device is not eligible for NFC card emulation."
        case .runtimeDisabled:
            return "NFC card emulation is disabled by application policy."
        case .invalidTag:
            return "The detected NFC tag is not an Iroha peer V1 endpoint."
        case .cancelled:
            return "The NFC exchange was cancelled."
        case .operationInProgress:
            return "Another NFC card session is still settling."
        case .retryExhausted:
            return "The NFC contact could not be restored within the retry limit."
        }
    }
}

private final class IrohaPeerNfcISO7816TagBoxV1: @unchecked Sendable {
    let tag: NFCISO7816Tag

    init(_ tag: NFCISO7816Tag) {
        self.tag = tag
    }
}

private final class IrohaPeerNfcCheckpointBoxV1: @unchecked Sendable {
    private let lock = NSLock()
    private var data: Data?

    init(_ data: Data?) { self.data = data }

    func load() -> Data? {
        lock.lock()
        defer { lock.unlock() }
        return data
    }

    func store(_ data: Data) {
        lock.lock()
        self.data = Data(data)
        lock.unlock()
    }
}

/// Marks a locally unrepresentable extended APDU as retryable only after the
/// operation has actually switched to its portable chunk limits.
private struct IrohaPeerNfcRetryableTransportErrorV1:
    IrohaPeerNfcAmbiguousResponseErrorV1 {}

/// Thin CoreNFC hooks around the transport-neutral `IrohaPeerNfcV1` core.
/// Applications retain lifecycle, entitlement, UI, and durable-store policy;
/// this adapter only converts commands, responses, and CardSession APDUs.
public enum IrohaPeerNfcCoreNFCAdapterV1 {
    @available(iOS 15.0, *)
    public static func readerAPDU(
        for command: IrohaPeerNfcCommandV1
    ) throws -> NFCISO7816APDU {
        let bytes = try IrohaPeerNfcAPDUCodecV1.encode(command)
        guard let apdu = NFCISO7816APDU(data: bytes) else {
            throw IrohaPeerNfcCoreNFCErrorV1.invalidCommandAPDU
        }
        return apdu
    }

    /// Sends one typed command without hiding non-success status words. This
    /// lets the application reconcile ambiguous RF loss with `GET_STATUS`.
    @available(iOS 15.0, *)
    public static func transceive(
        _ command: IrohaPeerNfcCommandV1,
        using tag: NFCISO7816Tag
    ) async throws -> IrohaPeerNfcAPDUResponseV1 {
        let apdu = try readerAPDU(for: command)
        let box = IrohaPeerNfcISO7816TagBoxV1(tag)
        return try await withCheckedThrowingContinuation { continuation in
            box.tag.sendCommand(apdu: apdu) { responseData, sw1, sw2, error in
                if let error {
                    continuation.resume(throwing: error)
                    return
                }
                let rawStatusWord = UInt16(sw1) << 8 | UInt16(sw2)
                guard let statusWord = IrohaPeerNfcStatusWordV1(rawValue: rawStatusWord) else {
                    continuation.resume(
                        throwing: IrohaPeerNfcCoreNFCErrorV1.invalidCommandAPDU
                    )
                    return
                }
                guard responseData.count <= IrohaPeerNfcV1.maximumChunkBytes else {
                    continuation.resume(
                        throwing: IrohaPeerNfcCoreNFCErrorV1.invalidCommandAPDU
                    )
                    return
                }
                continuation.resume(
                    returning: IrohaPeerNfcAPDUResponseV1(
                        data: responseData,
                        statusWord: statusWord
                    )
                )
            }
        }
    }

    /// Decodes the raw APDU delivered by iOS card emulation.
    @available(iOS 17.4, *)
    public static func cardCommand(
        from apdu: CardSession.APDU
    ) throws -> IrohaPeerNfcCommandV1 {
        try IrohaPeerNfcAPDUCodecV1.decode(apdu.payload)
    }

    /// Responds with `data || SW1 || SW2`. State and durable effects must be
    /// latched before calling this method because RF delivery can be ambiguous.
    @available(iOS 17.4, *)
    public static func respond(
        to apdu: CardSession.APDU,
        with response: IrohaPeerNfcAPDUResponseV1
    ) async throws {
        do {
            try await apdu.respond(response: response.encoded)
        } catch let error as CardSession.Error {
            guard error == .transmissionError else { throw error }
            try await apdu.respond(response: response.encoded)
        }
    }
}

/// App-owned presentation strings and the explicit CardSession runtime gate.
/// The AID is deliberately not configurable: every V1 implementation uses
/// `IrohaPeerNfcV1.applicationIdentifier` and fails closed on entitlement or
/// provisioning mismatches.
public struct IrohaPeerNfcCoreNFCConfigurationV1: Equatable, Sendable {
    public static let maximumDurabilityTimeoutMilliseconds: UInt64 = 5_000
    public let cardSessionRuntimeEnabled: Bool
    public let cardAlertMessage: String
    public let readerAlertMessage: String
    public let completionAlertMessage: String
    public let durabilityTimeoutMilliseconds: UInt64

    public init(
        cardSessionRuntimeEnabled: Bool,
        cardAlertMessage: String,
        readerAlertMessage: String,
        completionAlertMessage: String,
        durabilityTimeoutMilliseconds: UInt64 = 5_000
    ) {
        precondition(Self.isValidDurabilityTimeout(durabilityTimeoutMilliseconds))
        self.cardSessionRuntimeEnabled = cardSessionRuntimeEnabled
        self.cardAlertMessage = cardAlertMessage
        self.readerAlertMessage = readerAlertMessage
        self.completionAlertMessage = completionAlertMessage
        self.durabilityTimeoutMilliseconds = durabilityTimeoutMilliseconds
    }

    static func isValidDurabilityTimeout(_ milliseconds: UInt64) -> Bool {
        (1...maximumDurabilityTimeoutMilliseconds).contains(milliseconds)
    }

    var durabilityTimeoutNanoseconds: UInt64 {
        durabilityTimeoutMilliseconds * 1_000_000
    }
}

public enum IrohaPeerNfcCardAvailabilityV1: Equatable, Sendable {
    case available
    case unavailable
    case runtimeDisabled
    case unsupported
    case ineligible
}

public enum IrohaPeerNfcCardFailureV1: Error, Equatable, Sendable {
    case authorizationRejected
    case authorizationCancelled
    case paymentAdmissionRejected
    case paymentAdmissionPersistenceFailed
    case paymentAdmissionTimedOut
    case paymentAdmissionCancelled
    case durabilityWorkerSaturated
    case durableCommitRejected
    case durableCommitPersistenceFailed
    case durableCommitTimedOut
    case durableCommitCancelled
    case invalidDurableCommitResult
    case transportFailure
}

public enum IrohaPeerNfcCardEventV1: Equatable, Sendable {
    case emulationStarted
    case paymentAdmitted
    case acknowledgementReady
    case acknowledgementConfirmed
    case failed(IrohaPeerNfcCardFailureV1)
    case ended
}

/// CoreNFC CardSession lifecycle for the V1 receiver core. The application
/// commit callback must atomically persist payment ingest and the returned
/// `IDA1` record. The runtime installs it before replying `9000` to COMMIT.
@available(iOS 17.4, *)
public final class IrohaPeerNfcCardSessionControllerV1: @unchecked Sendable {
    public typealias AuthorizeRequest = @Sendable () async throws -> Void
    public typealias AdmitPayment = @Sendable (
        IrohaPeerNfcPaymentAdmissionContextV1
    ) async throws -> IrohaPeerNfcDurablePaymentAdmissionV1
    public typealias DurableCommit = @Sendable (
        IrohaPeerNfcCommitContextV1
    ) async throws -> IrohaPeerNfcDurableAcknowledgementV1
    public typealias EventHandler = @Sendable (IrohaPeerNfcCardEventV1) -> Void

    public static func availability(
        configuration: IrohaPeerNfcCoreNFCConfigurationV1
    ) async -> IrohaPeerNfcCardAvailabilityV1 {
        guard configuration.cardSessionRuntimeEnabled else { return .runtimeDisabled }
        guard NFCReaderSession.readingAvailable else { return .unavailable }
        guard CardSession.isSupported else { return .unsupported }
        return await CardSession.isEligible ? .available : .ineligible
    }

    private let configuration: IrohaPeerNfcCoreNFCConfigurationV1
    private let lock = NSLock()
    private var runtime: IrohaPeerNfcCardRuntimeV1?

    public init(configuration: IrohaPeerNfcCoreNFCConfigurationV1) {
        self.configuration = configuration
    }

    @discardableResult
    public func start(
        sessionID: Data,
        receiveRequest: Data,
        restoredDurableAcknowledgement: IrohaPeerNfcDurableAcknowledgementV1? = nil,
        restoredPaymentAdmission: IrohaPeerNfcDurablePaymentAdmissionV1? = nil,
        profilePolicy: IrohaPeerNfcProfilePolicyV1,
        limits: IrohaPeerNfcLimitsV1 = .default,
        onEvent: @escaping EventHandler,
        authorizeRequest: @escaping AuthorizeRequest,
        admitPayment: @escaping AdmitPayment,
        durableCommit: @escaping DurableCommit
    ) async throws -> IrohaPeerNfcRequestIdentityV1 {
        guard configuration.cardSessionRuntimeEnabled else {
            throw IrohaPeerNfcCoreNFCErrorV1.runtimeDisabled
        }
        switch await Self.availability(configuration: configuration) {
        case .available:
            break
        case .runtimeDisabled:
            throw IrohaPeerNfcCoreNFCErrorV1.runtimeDisabled
        case .unsupported:
            throw IrohaPeerNfcCoreNFCErrorV1.cardEmulationUnavailable
        case .ineligible:
            throw IrohaPeerNfcCoreNFCErrorV1.cardEmulationIneligible
        case .unavailable:
            throw IrohaPeerNfcCoreNFCErrorV1.unavailable
        }

        let receiver = try IrohaPeerNfcReceiverSessionV1(
            sessionID: sessionID,
            receiveRequest: receiveRequest,
            durableAcknowledgement: restoredDurableAcknowledgement,
            restoredPaymentAdmission: restoredPaymentAdmission,
            profilePolicy: profilePolicy,
            limits: limits
        )
        let replacement = IrohaPeerNfcCardRuntimeV1(
            owner: self,
            configuration: configuration,
            receiver: receiver,
            onEvent: onEvent,
            authorizeRequest: authorizeRequest,
            admitPayment: admitPayment,
            durableCommit: durableCommit
        )
        lock.lock()
        guard runtime == nil else {
            lock.unlock()
            throw IrohaPeerNfcCoreNFCErrorV1.operationInProgress
        }
        runtime = replacement
        lock.unlock()
        do {
            try await replacement.start()
            return receiver.identity
        } catch {
            lock.lock()
            if runtime === replacement { runtime = nil }
            lock.unlock()
            replacement.stop()
            throw error
        }
    }

    public func stop() {
        lock.lock()
        let active = runtime
        lock.unlock()
        active?.stop()
    }

    fileprivate func runtimeDidSettle(_ settled: IrohaPeerNfcCardRuntimeV1) {
        lock.lock()
        if runtime === settled { runtime = nil }
        lock.unlock()
    }
}

@available(iOS 17.4, *)
private final class IrohaPeerNfcCardRuntimeV1: @unchecked Sendable {
    private weak var owner: IrohaPeerNfcCardSessionControllerV1?
    private let configuration: IrohaPeerNfcCoreNFCConfigurationV1
    private var receiver: IrohaPeerNfcReceiverSessionV1
    private let onEvent: IrohaPeerNfcCardSessionControllerV1.EventHandler
    private let authorizeRequest: IrohaPeerNfcCardSessionControllerV1.AuthorizeRequest
    private let admitPayment: IrohaPeerNfcCardSessionControllerV1.AdmitPayment
    private let durableCommit: IrohaPeerNfcCardSessionControllerV1.DurableCommit
    private let lock = NSLock()
    private var cardSession: CardSession?
    private var eventTask: Task<Void, Never>?
    private var didPublishEmulation = false
    private var requestAuthorized = false
    private var protectedBoundaryInFlight = false
    private var terminalEventGate = IrohaPeerNfcTerminalEventGateV1()
    private var startGate = IrohaPeerNfcCardRuntimeStartGateV1()

    init(
        owner: IrohaPeerNfcCardSessionControllerV1,
        configuration: IrohaPeerNfcCoreNFCConfigurationV1,
        receiver: IrohaPeerNfcReceiverSessionV1,
        onEvent: @escaping IrohaPeerNfcCardSessionControllerV1.EventHandler,
        authorizeRequest: @escaping IrohaPeerNfcCardSessionControllerV1.AuthorizeRequest,
        admitPayment: @escaping IrohaPeerNfcCardSessionControllerV1.AdmitPayment,
        durableCommit: @escaping IrohaPeerNfcCardSessionControllerV1.DurableCommit
    ) {
        self.owner = owner
        self.configuration = configuration
        self.receiver = receiver
        self.onEvent = onEvent
        self.authorizeRequest = authorizeRequest
        self.admitPayment = admitPayment
        self.durableCommit = durableCommit
    }

    func start() async throws {
        lock.lock()
        let mayStart = startGate.beginStart()
        lock.unlock()
        guard mayStart else {
            publishEndedOnce()
            throw IrohaPeerNfcCoreNFCErrorV1.cancelled
        }

        let session: CardSession
        do {
            session = try await CardSession()
        } catch {
            lock.lock()
            let mayInstall = startGate.finishSessionCreation()
            lock.unlock()
            if !mayInstall { publishEndedOnce() }
            throw error
        }
        session.alertMessage = configuration.cardAlertMessage
        lock.lock()
        guard startGate.finishSessionCreation() else {
            lock.unlock()
            session.invalidate()
            publishEndedOnce()
            throw IrohaPeerNfcCoreNFCErrorV1.cancelled
        }
        cardSession = session
        eventTask = Task { [self] in
            await run(session)
            finishEventLoop()
        }
        lock.unlock()
    }

    func stop() {
        lock.lock()
        let mustDefer = protectedBoundaryInFlight
        let task = mustDefer ? nil : eventTask
        let session = mustDefer ? nil : cardSession
        if !mustDefer {
            eventTask = nil
            cardSession = nil
        }
        let shouldPublishEnded = startGate.requestStop(
            hasActiveSessionOrTask: eventTask != nil || cardSession != nil
                || task != nil || session != nil
        )
        lock.unlock()
        if mustDefer { return }
        task?.cancel()
        session?.invalidate()
        if shouldPublishEnded { publishEndedOnce() }
    }

    private func run(_ session: CardSession) async {
        do {
            for try await event in session.eventStream {
                try Task.checkCancellation()
                guard mayContinue(session) else { return }
                switch event {
                case .sessionStarted, .readerDetected:
                    if !(await session.isEmulationInProgress) {
                        try await session.startEmulation()
                    }
                    try Task.checkCancellation()
                    guard mayContinue(session) else { return }
                    if !didPublishEmulation {
                        didPublishEmulation = true
                        onEvent(.emulationStarted)
                    }
                case .received(let apdu):
                    let command = try? IrohaPeerNfcCoreNFCAdapterV1.cardCommand(from: apdu)
                    let response: IrohaPeerNfcAPDUResponseV1
                    var protectedCommand = false
                    var terminalFailure: IrohaPeerNfcCardFailureV1?
                    if let command, Self.exposesRequest(command), !requestAuthorized {
                        do {
                            try await authorizeRequest()
                            try Task.checkCancellation()
                            guard mayContinue(session) else { return }
                            requestAuthorized = true
                            response = receiver.process(apdu: apdu.payload)
                        } catch is CancellationError {
                            terminalFailure = .authorizationCancelled
                            response = IrohaPeerNfcAPDUResponseV1(
                                statusWord: .securityStatusNotSatisfied
                            )
                        } catch {
                            terminalFailure = .authorizationRejected
                            response = IrohaPeerNfcAPDUResponseV1(
                                statusWord: .securityStatusNotSatisfied
                            )
                        }
                    } else if let command, case .beginPayment = command {
                        do {
                            switch try receiver.preparePaymentAdmission(command) {
                            case .alreadyAdmitted:
                                response = IrohaPeerNfcAPDUResponseV1(statusWord: .success)
                                // A replay after process restart is also a state observation for
                                // the application. Events are intentionally idempotent.
                                onEvent(.paymentAdmitted)
                            case .requiresDurableAdmission(let context):
                                guard beginProtectedBoundary(session) else { return }
                                protectedCommand = true
                                let record: IrohaPeerNfcDurablePaymentAdmissionV1
                                do {
                                    record = try await irohaPeerNfcWithDurabilityDeadlineV1(
                                        timeoutNanoseconds: configuration
                                            .durabilityTimeoutNanoseconds,
                                        operation: { [admitPayment] in
                                            try await admitPayment(context)
                                        }
                                    )
                                } catch let failure as IrohaPeerNfcDurabilityDeadlineErrorV1 {
                                    terminalFailure = failure == .timedOut
                                        ? .paymentAdmissionTimedOut
                                        : .durabilityWorkerSaturated
                                    response = IrohaPeerNfcAPDUResponseV1(
                                        statusWord: .storageFailure
                                    )
                                    break
                                } catch is CancellationError {
                                    terminalFailure = .paymentAdmissionCancelled
                                    response = IrohaPeerNfcAPDUResponseV1(
                                        statusWord: .storageFailure
                                    )
                                    break
                                } catch {
                                    terminalFailure = .paymentAdmissionPersistenceFailed
                                    response = IrohaPeerNfcAPDUResponseV1(
                                        statusWord: .storageFailure
                                    )
                                    break
                                }
                                do {
                                    guard mayContinue(session) else {
                                        terminalFailure = .paymentAdmissionCancelled
                                        response = IrohaPeerNfcAPDUResponseV1(
                                            statusWord: .storageFailure
                                        )
                                        break
                                    }
                                    guard record.context == context else {
                                        throw IrohaPeerNfcErrorV1.continuityMismatch
                                    }
                                    try receiver.installPaymentAdmission(record)
                                    response = IrohaPeerNfcAPDUResponseV1(statusWord: .success)
                                    onEvent(.paymentAdmitted)
                                } catch {
                                    terminalFailure = .paymentAdmissionRejected
                                    response = IrohaPeerNfcAPDUResponseV1(
                                        statusWord: .conditionsNotSatisfied
                                    )
                                }
                            }
                        } catch {
                            terminalFailure = .paymentAdmissionRejected
                            response = IrohaPeerNfcAPDUResponseV1(
                                statusWord: .conditionsNotSatisfied
                            )
                        }
                    } else if let command, case .commit = command {
                        do {
                            switch try receiver.prepareCommit(command) {
                            case .alreadyCommitted:
                                response = IrohaPeerNfcAPDUResponseV1(statusWord: .success)
                                onEvent(.acknowledgementReady)
                            case .requiresDurableCommit(let context):
                                guard beginProtectedBoundary(session) else { return }
                                protectedCommand = true
                                let record: IrohaPeerNfcDurableAcknowledgementV1
                                do {
                                    record = try await irohaPeerNfcWithDurabilityDeadlineV1(
                                        timeoutNanoseconds: configuration
                                            .durabilityTimeoutNanoseconds,
                                        operation: { [durableCommit] in
                                            try await durableCommit(context)
                                        }
                                    )
                                } catch let failure as IrohaPeerNfcDurabilityDeadlineErrorV1 {
                                    terminalFailure = failure == .timedOut
                                        ? .durableCommitTimedOut
                                        : .durabilityWorkerSaturated
                                    response = IrohaPeerNfcAPDUResponseV1(
                                        statusWord: .storageFailure
                                    )
                                    break
                                } catch is CancellationError {
                                    terminalFailure = .durableCommitCancelled
                                    response = IrohaPeerNfcAPDUResponseV1(
                                        statusWord: .storageFailure
                                    )
                                    break
                                } catch {
                                    terminalFailure = .durableCommitPersistenceFailed
                                    response = IrohaPeerNfcAPDUResponseV1(
                                        statusWord: .storageFailure
                                    )
                                    break
                                }
                                do {
                                    guard mayContinue(session) else {
                                        terminalFailure = .durableCommitCancelled
                                        response = IrohaPeerNfcAPDUResponseV1(
                                            statusWord: .storageFailure
                                        )
                                        break
                                    }
                                    try receiver.installDurableAcknowledgement(record)
                                    response = IrohaPeerNfcAPDUResponseV1(statusWord: .success)
                                    onEvent(.acknowledgementReady)
                                } catch {
                                    terminalFailure = .invalidDurableCommitResult
                                    response = IrohaPeerNfcAPDUResponseV1(
                                        statusWord: .conditionsNotSatisfied
                                    )
                                }
                            }
                        } catch {
                            terminalFailure = .durableCommitRejected
                            response = IrohaPeerNfcAPDUResponseV1(
                                statusWord: .conditionsNotSatisfied
                            )
                        }
                    } else {
                        response = receiver.process(apdu: apdu.payload)
                    }
                    if !protectedCommand {
                        try Task.checkCancellation()
                        guard mayContinue(session) else { return }
                    }
                    if let terminalFailure { publishFailureOnce(terminalFailure) }
                    do {
                        try await IrohaPeerNfcCoreNFCAdapterV1.respond(
                            to: apdu,
                            with: response
                        )
                    } catch let error as CardSession.Error {
                        guard error == .transmissionError else {
                            if protectedCommand, response.statusWord == .success {
                                _ = finishProtectedBoundary(session)
                                session.invalidate()
                                return
                            }
                            throw error
                        }
                        if response.statusWord == .success,
                           command.map(Self.isConfirmation) == true {
                            // CONFIRM_ACK was latched before response delivery;
                            // finish below even when the RF response was lost.
                        } else {
                            // Keep request bytes, payment progress, and any
                            // durable ACK available. readerDeselected re-arms
                            // emulation for the reader's same-session retry.
                            if protectedCommand {
                                let shouldStop = finishProtectedBoundary(session)
                                if terminalFailure != nil || shouldStop {
                                    session.invalidate()
                                    return
                                }
                            }
                            continue
                        }
                    }
                    if protectedCommand {
                        let shouldStop = finishProtectedBoundary(session)
                        if terminalFailure != nil || shouldStop {
                            session.invalidate()
                            return
                        }
                    } else {
                        try Task.checkCancellation()
                        guard mayContinue(session) else { return }
                        if terminalFailure != nil {
                            session.invalidate()
                            return
                        }
                    }
                    if response.statusWord == .success,
                       command.map(Self.isConfirmation) == true {
                        onEvent(.acknowledgementConfirmed)
                        if await session.isEmulationInProgress {
                            await session.stopEmulation(status: .success)
                        }
                        session.invalidate()
                        return
                    }
                case .readerDeselected:
                    // Receiver state remains latched. Proactive re-arming
                    // avoids depending on a later readerDetected callback.
                    if !(await session.isEmulationInProgress) {
                        try await session.startEmulation()
                    }
                case .sessionInvalidated:
                    return
                @unknown default:
                    break
                }
            }
        } catch is CancellationError {
            return
        } catch {
            publishFailureOnce(.transportFailure)
            session.invalidate()
        }
    }

    private func publishEndedOnce() {
        lock.lock()
        let shouldPublish = terminalEventGate.claimEndedPublication()
        lock.unlock()
        if shouldPublish { onEvent(.ended) }
    }

    private func publishFailureOnce(_ failure: IrohaPeerNfcCardFailureV1) {
        lock.lock()
        let shouldPublish = terminalEventGate.claimFailurePublication()
        lock.unlock()
        if shouldPublish { onEvent(.failed(failure)) }
    }

    private func beginProtectedBoundary(_ session: CardSession) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        guard cardSession === session,
              !startGate.stopRequested,
              !protectedBoundaryInFlight else { return false }
        protectedBoundaryInFlight = true
        return true
    }

    /// Ends the admission/commit barrier only after state installation and the
    /// APDU response attempt. A stop requested meanwhile is honored now.
    private func finishProtectedBoundary(_ session: CardSession) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        guard cardSession === session, protectedBoundaryInFlight else { return true }
        protectedBoundaryInFlight = false
        return startGate.stopRequested
    }

    /// Awaited authorization and unprotected APDU callbacks may finish after a
    /// synchronous stop. Durable admission/commit deliberately do not use this
    /// check: their protected boundary must install the returned record and
    /// attempt the APDU response before honoring a deferred stop.
    private func mayContinue(_ session: CardSession) -> Bool {
        lock.lock()
        defer { lock.unlock() }
        return cardSession === session && !startGate.stopRequested
    }

    private func finishEventLoop() {
        lock.lock()
        eventTask = nil
        cardSession = nil
        lock.unlock()
        owner?.runtimeDidSettle(self)
        publishEndedOnce()
    }

    private static func isConfirmation(_ command: IrohaPeerNfcCommandV1) -> Bool {
        if case .confirmAcknowledgement = command { return true }
        return false
    }

    private static func exposesRequest(_ command: IrohaPeerNfcCommandV1) -> Bool {
        switch command {
        case .getInfo, .readRequest:
            return true
        default:
            return false
        }
    }
}

@available(iOS 15.0, *)
public final class IrohaPeerNfcReaderServiceV1: NSObject, @unchecked Sendable,
    NFCTagReaderSessionDelegate {
    public static var isAvailable: Bool {
#if targetEnvironment(simulator)
        false
#else
        NFCReaderSession.readingAvailable
#endif
    }

    private let configuration: IrohaPeerNfcCoreNFCConfigurationV1
    private let lock = NSLock()
    private var session: NFCTagReaderSession?
    private var continuation: CheckedContinuation<IrohaPeerNfcReaderExchangeResultV1, Error>?
    private var exchange: (@Sendable (
        NFCISO7816Tag,
        UInt64,
        NFCTagReaderSession
    ) async throws -> IrohaPeerNfcReaderExchangeResultV1)?
    private var exchangeTask: Task<Void, Never>?
    private var progressReporter: IrohaPeerNfcReaderProgressReporterV1?
    private var platformCallGate: IrohaPeerNfcReaderPlatformCallGateV1?
    private var retryLimitsBox: IrohaPeerNfcRetryLimitsBoxV1?
    private var retryGate: IrohaPeerNfcRetryGateV1?
    private var retryTimeoutTask: Task<Void, Never>?
    private var operationGate = IrohaPeerNfcReaderOperationGateV1()
    private var completed = false

    public init(configuration: IrohaPeerNfcCoreNFCConfigurationV1) {
        self.configuration = configuration
        super.init()
    }

    public func run(
        restoredCheckpoint: Data? = nil,
        profilePolicy: IrohaPeerNfcProfilePolicyV1,
        limits: IrohaPeerNfcLimitsV1 = .default,
        retryPolicy: IrohaPeerNfcReaderRetryPolicyV1 = .default,
        onTypedProgress: ((IrohaPeerNfcProgressEventV1) -> Void)? = nil,
        loadOrCreateDurableCheckpoint: @escaping
            IrohaPeerNfcReaderExchangeV1.LoadOrCreateDurableCheckpoint,
        updateDurableCheckpoint: @escaping
            IrohaPeerNfcReaderExchangeV1.UpdateDurableCheckpoint
    ) async throws -> IrohaPeerNfcReaderExchangeResultV1 {
        guard Self.isAvailable else { throw IrohaPeerNfcCoreNFCErrorV1.unavailable }
        return try await withCheckedThrowingContinuation { continuation in
            lock.lock()
            guard self.continuation == nil else {
                lock.unlock()
                continuation.resume(throwing: IrohaPeerNfcCoreNFCErrorV1.unavailable)
                return
            }
            self.completed = false
            let operationEpoch = self.operationGate.beginOperation()
            self.continuation = continuation
            let progressReporter = IrohaPeerNfcReaderProgressReporterV1(
                handler: onTypedProgress
            )
            self.progressReporter = progressReporter
            let platformCallGate = IrohaPeerNfcReaderPlatformCallGateV1()
            self.platformCallGate = platformCallGate
            let checkpointBox = IrohaPeerNfcCheckpointBoxV1(restoredCheckpoint)
            let retryLimitsBox = IrohaPeerNfcRetryLimitsBoxV1(limits)
            self.retryLimitsBox = retryLimitsBox
            self.retryGate = IrohaPeerNfcRetryGateV1(retryPolicy)
            self.exchange = { [weak self] tag, capturedEpoch, capturedSession in
                guard let self else { throw CancellationError() }
                let validate = { [weak self] in
                    guard let self else { throw CancellationError() }
                    try self.validateCurrentOperation(
                        capturedEpoch: capturedEpoch,
                        expectedSession: capturedSession
                    )
                }
                try validate()
                let exchangeLimits = retryLimitsBox.load()
                let result = try await IrohaPeerNfcReaderExchangeV1.run(
                    restoredCheckpoint: checkpointBox.load(),
                    profilePolicy: profilePolicy,
                    limits: exchangeLimits,
                    transceive: { command in
                        do {
                            return try await irohaPeerNfcGuardedAwaitV1(
                                validate: validate,
                                operation: {
                                    try await IrohaPeerNfcCoreNFCAdapterV1.transceive(
                                        command,
                                        using: tag
                                    )
                                }
                            )
                        } catch is CancellationError {
                            throw CancellationError()
                        } catch let error as IrohaPeerNfcCoreNFCErrorV1 {
                            try validate()
                            if case .invalidCommandAPDU = error,
                               retryLimitsBox.downgradeForRetry() {
                                throw IrohaPeerNfcRetryableTransportErrorV1()
                            }
                            throw error
                        } catch let error as NFCReaderError
                            where Self.isRetryableContactError(error) {
                            try validate()
                            throw IrohaPeerNfcRetryableTransportErrorV1()
                        } catch {
                            try validate()
                            throw error
                        }
                    },
                    loadOrCreateDurableCheckpoint: { info, request in
                        try validate()
                        progressReporter.emit(
                            .requestRead,
                            bytes: request.encoded.count
                        )
                        let checkpoint = try await irohaPeerNfcGuardedAwaitV1(
                            validate: validate,
                            operation: {
                                try await loadOrCreateDurableCheckpoint(info, request)
                            }
                        )
                        // The application contract guarantees this value is
                        // already durable. Cache it only after the boundary
                        // returns so a CoreNFC reconnect can resume without
                        // invoking value creation again.
                        checkpointBox.store(checkpoint.encoded)
                        return checkpoint
                    },
                    updateDurableCheckpoint: { encoded in
                        try validate()
                        let checkpoint = try IrohaPeerNfcSenderCheckpointV1.decode(
                            encoded,
                            profilePolicy: profilePolicy,
                            limits: exchangeLimits
                        )
                        guard checkpoint.durableAcknowledgement != nil else {
                            throw IrohaPeerNfcErrorV1.continuityMismatch
                        }
                        // A validated acknowledgement can only be read after
                        // the receiver durably commits the payment. Publish
                        // this before the app persists that ACK so the two
                        // durability boundaries retain their order.
                        progressReporter.emit(
                            .paymentCommitted,
                            bytes: checkpoint.payment.encoded.count
                        )
                        _ = try await irohaPeerNfcGuardedAwaitV1(
                            validate: validate,
                            operation: {
                                try await updateDurableCheckpoint(encoded)
                            }
                        )
                        checkpointBox.store(encoded)
                    }
                )
                try validate()
                return result
            }
            guard let readerSession = NFCTagReaderSession(
                pollingOption: [.iso14443],
                delegate: self,
                queue: nil
            ) else {
                self.continuation = nil
                self.exchange = nil
                self.progressReporter = nil
                self.platformCallGate = nil
                self.retryLimitsBox = nil
                self.retryGate = nil
                self.completed = true
                _ = self.operationGate.finishOperation(
                    capturedEpoch: operationEpoch
                )
                lock.unlock()
                platformCallGate.invalidate()
                progressReporter.invalidate()
                continuation.resume(throwing: IrohaPeerNfcCoreNFCErrorV1.unavailable)
                return
            }
            readerSession.alertMessage = configuration.readerAlertMessage
            self.session = readerSession
            lock.unlock()
            platformCallGate.performIfActive {
                readerSession.begin()
            }
        }
    }

    public func cancel() {
        finish(.failure(IrohaPeerNfcCoreNFCErrorV1.cancelled), invalidating: true)
    }

    public func tagReaderSessionDidBecomeActive(_ session: NFCTagReaderSession) {
        lock.lock()
        let reporter = !completed && self.session === session
            ? progressReporter
            : nil
        lock.unlock()
        reporter?.emit(.phase1SessionActive)
    }

    public func tagReaderSession(
        _ session: NFCTagReaderSession,
        didInvalidateWithError error: Error
    ) {
        lock.lock()
        let operationEpoch = operationGate.activeEpoch
        let shouldFinish = !completed
            && self.session === session
            && operationGate.mayMutate(capturedEpoch: operationEpoch)
        lock.unlock()
        if shouldFinish {
            finish(
                .failure(Self.mapInvalidation(error)),
                invalidating: false,
                expectedEpoch: operationEpoch,
                expectedSession: session
            )
        }
    }

    public func tagReaderSession(_ session: NFCTagReaderSession, didDetect tags: [NFCTag]) {
        guard tags.count == 1, let tag = tags.first,
              case .iso7816(let isoTag) = tag else {
            lock.lock()
            let operationEpoch = operationGate.activeEpoch
            let platformCallGate = !completed
                && self.session === session
                && operationGate.mayMutate(capturedEpoch: operationEpoch)
                ? self.platformCallGate
                : nil
            lock.unlock()
            platformCallGate?.performIfActive {
                session.alertMessage = IrohaPeerNfcCoreNFCErrorV1
                    .invalidTag.localizedDescription
                session.restartPolling()
            }
            return
        }
        lock.lock()
        let operationEpoch = operationGate.activeEpoch
        guard !completed,
              self.session === session,
              exchangeTask == nil,
              let exchange,
              let platformCallGate = self.platformCallGate,
              let retryGate,
              retryGate.claimContactAttempt(
                operationGate: &operationGate,
                capturedEpoch: operationEpoch
              ) else {
            lock.unlock()
            return
        }
        let retryTimeoutTask = self.retryTimeoutTask
        self.retryTimeoutTask = nil
        let reporter = progressReporter
        lock.unlock()
        retryTimeoutTask?.cancel()
        reporter?.emit(.tagDetected)
        platformCallGate.performIfActive {
            session.connect(to: tag) { [weak self] error in
                guard let self else { return }
                self.lock.lock()
                guard self.operationGate.finishConnect(
                    capturedEpoch: operationEpoch
                ), !self.completed, self.session === session else {
                    self.lock.unlock()
                    return
                }
                if let error {
                    let mayRetry = Self.isRetryableContactError(error)
                        && (self.retryGate?.mayRedetect() == true)
                    self.lock.unlock()
                    if mayRetry {
                        self.restartPollingWithDeadline(
                            session,
                            operationEpoch: operationEpoch
                        )
                    } else {
                        self.finish(
                            .failure(
                                Self.isRetryableContactError(error)
                                    ? IrohaPeerNfcCoreNFCErrorV1.retryExhausted
                                    : error
                            ),
                            invalidating: true,
                            expectedEpoch: operationEpoch,
                            expectedSession: session
                        )
                    }
                    return
                }
                let task = Task { [weak self] in
                    guard let self else { return }
                    do {
                        let result = try await exchange(isoTag, operationEpoch, session)
                        platformCallGate.performIfActive {
                            session.alertMessage = self.configuration.completionAlertMessage
                        }
                        self.finish(
                            .success(result),
                            invalidating: true,
                            expectedEpoch: operationEpoch,
                            expectedSession: session
                        )
                    } catch is CancellationError {
                        // cancel()/finish() already owns terminal publication.
                        // A stale durable callback returning after cancellation
                        // must not finish or restart a later operation.
                        return
                    } catch let error as IrohaPeerNfcErrorV1 {
                        self.finish(
                            .failure(error),
                            invalidating: true,
                            expectedEpoch: operationEpoch,
                            expectedSession: session
                        )
                    } catch let error as IrohaPeerNfcCoreNFCErrorV1 {
                        self.finish(
                            .failure(error),
                            invalidating: true,
                            expectedEpoch: operationEpoch,
                            expectedSession: session
                        )
                    } catch is IrohaPeerNfcRetryableTransportErrorV1 {
                        // RF loss keeps the persisted checkpoint authoritative.
                        // A fresh detection reruns GET_INFO/GET_STATUS and resumes.
                        self.lock.lock()
                        let mayRetry = !self.completed
                            && self.session === session
                            && self.operationGate.mayMutate(
                                capturedEpoch: operationEpoch
                            )
                            && self.retryGate?.mayRedetect() == true
                        if mayRetry { self.exchangeTask = nil }
                        self.lock.unlock()
                        if mayRetry {
                            self.restartPollingWithDeadline(
                                session,
                                operationEpoch: operationEpoch
                            )
                        } else {
                            self.finish(
                                .failure(IrohaPeerNfcCoreNFCErrorV1.retryExhausted),
                                invalidating: true,
                                expectedEpoch: operationEpoch,
                                expectedSession: session
                            )
                        }
                    } catch {
                        // Application preparation and checkpoint persistence
                        // failures are not RF failures and must not retry forever.
                        self.finish(
                            .failure(error),
                            invalidating: true,
                            expectedEpoch: operationEpoch,
                            expectedSession: session
                        )
                    }
                }
                self.exchangeTask = task
                self.lock.unlock()
            }
        }
    }

    private func finish(
        _ result: Result<IrohaPeerNfcReaderExchangeResultV1, Error>,
        invalidating: Bool,
        expectedEpoch: UInt64? = nil,
        expectedSession: NFCTagReaderSession? = nil
    ) {
        lock.lock()
        let operationEpoch = expectedEpoch ?? operationGate.activeEpoch
        guard !completed,
              operationGate.mayMutate(capturedEpoch: operationEpoch),
              expectedSession.map({ self.session === $0 }) ?? true else {
            lock.unlock()
            return
        }
        completed = true
        _ = operationGate.finishOperation(capturedEpoch: operationEpoch)
        let continuation = continuation
        self.continuation = nil
        exchange = nil
        let progressReporter = self.progressReporter
        self.progressReporter = nil
        let platformCallGate = self.platformCallGate
        self.platformCallGate = nil
        retryLimitsBox = nil
        retryGate = nil
        let retryTimeoutTask = self.retryTimeoutTask
        self.retryTimeoutTask = nil
        let task = exchangeTask
        exchangeTask = nil
        let session = session
        self.session = nil
        lock.unlock()
        retryTimeoutTask?.cancel()
        platformCallGate?.invalidate()
        progressReporter?.invalidate()
        task?.cancel()
        if invalidating { session?.invalidate() }
        continuation?.resume(with: result)
    }

    private func restartPollingWithDeadline(
        _ session: NFCTagReaderSession,
        operationEpoch: UInt64
    ) {
        lock.lock()
        guard !completed,
              self.session === session,
              let gate = retryGate,
              let platformCallGate else {
            lock.unlock()
            return
        }
        retryTimeoutTask?.cancel()
        let timeout = gate.redetectionTimeoutNanoseconds
        let task = Task { [weak self, weak session] in
            try? await Task.sleep(nanoseconds: timeout)
            guard !Task.isCancelled, let self, let session else { return }
            self.finish(
                .failure(IrohaPeerNfcCoreNFCErrorV1.retryExhausted),
                invalidating: true,
                expectedEpoch: operationEpoch,
                expectedSession: session
            )
        }
        retryTimeoutTask = task
        lock.unlock()
        platformCallGate.performIfActive { session.restartPolling() }
    }

    private func validateCurrentOperation(
        capturedEpoch: UInt64,
        expectedSession: NFCTagReaderSession
    ) throws {
        try Task.checkCancellation()
        lock.lock()
        let isCurrent = !completed
            && session === expectedSession
            && operationGate.mayMutate(capturedEpoch: capturedEpoch)
        lock.unlock()
        guard isCurrent else { throw CancellationError() }
    }

    private static func mapInvalidation(_ error: Error) -> Error {
        guard let readerError = error as? NFCReaderError else { return error }
        switch readerError.code {
        case .readerSessionInvalidationErrorUserCanceled:
            return IrohaPeerNfcCoreNFCErrorV1.cancelled
        default:
            return error
        }
    }

    private static func isRetryableContactError(_ error: Error) -> Bool {
        guard let readerError = error as? NFCReaderError else { return false }
        switch readerError.code {
        case .readerTransceiveErrorTagConnectionLost,
             .readerTransceiveErrorRetryExceeded,
             .readerTransceiveErrorTagNotConnected:
            return true
        default:
            return false
        }
    }
}
#endif
