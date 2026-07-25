import Foundation

/// Bounded, presentation-safe terminal failure returned by a canonical
/// Kagemusha operation status resource.
public struct KagemushaOperationTerminalFailure: Equatable, Sendable {
    public static let maximumCodeUTF8Bytes = 64
    public static let maximumMessageUTF8Bytes = 1_024

    public let code: String
    public let message: String

    public init(code: String, message: String) {
        let boundedCode = Self.boundedText(
            code,
            maximumUTF8Bytes: Self.maximumCodeUTF8Bytes,
            allowNewline: false
        )
        let canonicalMessage = Self.boundedText(
            message,
            maximumUTF8Bytes: Self.maximumMessageUTF8Bytes,
            allowNewline: true
        )
        self.code = Self.isStableCode(boundedCode)
            ? boundedCode
            : "offline_operation_rejected"
        self.message = canonicalMessage.isEmpty
            ? "Torii rejected the Kagemusha operation."
            : canonicalMessage
    }

    private static func boundedText(
        _ value: String,
        maximumUTF8Bytes: Int,
        allowNewline: Bool
    ) -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        var output = String.UnicodeScalarView()
        var byteCount = 0
        for scalar in trimmed.unicodeScalars {
            if CharacterSet.controlCharacters.contains(scalar),
               !(allowNewline && scalar.value == 0x0a) {
                continue
            }
            let scalarByteCount = String(scalar).utf8.count
            guard scalarByteCount <= maximumUTF8Bytes - byteCount else { break }
            output.append(scalar)
            byteCount += scalarByteCount
        }
        return String(output).trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private static func isStableCode(_ value: String) -> Bool {
        let bytes = Array(value.utf8)
        guard (1...maximumCodeUTF8Bytes).contains(bytes.count),
              let first = bytes.first,
              isLowercaseLetter(first) || isDigit(first) else {
            return false
        }
        return bytes.allSatisfy {
            isLowercaseLetter($0) || isDigit($0) || $0 == UInt8(ascii: "_")
        }
    }

    private static func isDigit(_ byte: UInt8) -> Bool {
        byte >= UInt8(ascii: "0") && byte <= UInt8(ascii: "9")
    }

    private static func isLowercaseLetter(_ byte: UInt8) -> Bool {
        byte >= UInt8(ascii: "a") && byte <= UInt8(ascii: "z")
    }
}

/// The authoritative Torii POST surface whose exact rejection contract was
/// used for classification.
public enum KagemushaSubmissionTarget: String, Codable, Equatable, Sendable {
    case offlineTopUp = "offline_top_up"
    case offlineRedeem = "offline_redeem"
    case signedTransaction = "signed_transaction"

    fileprivate init(operationKind: KagemushaOperationKind) {
        switch operationKind {
        case .topUp: self = .offlineTopUp
        case .redeem: self = .offlineRedeem
        }
    }

    fileprivate var operationKind: KagemushaOperationKind? {
        switch self {
        case .offlineTopUp: .topUp
        case .offlineRedeem: .redeem
        case .signedTransaction: nil
        }
    }
}

/// A permanent HTTP client failure returned by a typed submission endpoint
/// before Torii admitted the request. Applications must persist
/// this value with the operation journal and require explicit reconciliation;
/// it is never permission to automatically replay the POST.
public struct KagemushaDefinitiveSubmissionFailure: Codable, Equatable, Sendable {
    public static let maximumMessageUTF8Bytes = 1_024

    public let target: KagemushaSubmissionTarget
    public let statusCode: Int
    public let rejectCode: String
    public let message: String?

    public init(
        target: KagemushaSubmissionTarget,
        statusCode: Int,
        rejectCode: String,
        message: String?
    ) throws {
        guard KagemushaSubmissionFailureClassifier
                .isCanonicalDefinitivePreAdmission(
                    target: target,
                    statusCode: statusCode,
                    rejectCode: rejectCode
                ) else {
            throw KagemushaOperationFinalityError.invalidConfiguration(
                "definitiveSubmissionFailure.classification"
            )
        }
        if let message {
            guard !message.isEmpty,
                  message.utf8.count <= Self.maximumMessageUTF8Bytes,
                  message.trimmingCharacters(in: .whitespacesAndNewlines) == message,
                  !message.unicodeScalars.contains(
                    where: CharacterSet.controlCharacters.contains
                  ) else {
                throw KagemushaOperationFinalityError.invalidConfiguration(
                    "definitiveSubmissionFailure.message"
                )
            }
        }
        self.target = target
        self.statusCode = statusCode
        self.rejectCode = rejectCode
        self.message = message
    }

    fileprivate init(
        target: KagemushaSubmissionTarget,
        sanitizingHTTPStatusCode statusCode: Int,
        rejectCode: String,
        message: String?
    ) {
        self.target = target
        self.statusCode = statusCode
        self.rejectCode = rejectCode
        self.message = message.flatMap { value in
            let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.lowercased().hasPrefix("body_hex=") else {
                return nil
            }
            var output = String.UnicodeScalarView()
            var byteCount = 0
            for scalar in trimmed.unicodeScalars {
                guard !CharacterSet.controlCharacters.contains(scalar) else {
                    continue
                }
                let scalarByteCount = String(scalar).utf8.count
                guard scalarByteCount
                        <= Self.maximumMessageUTF8Bytes - byteCount else {
                    break
                }
                output.append(scalar)
                byteCount += scalarByteCount
            }
            let bounded = String(output)
                .trimmingCharacters(in: .whitespacesAndNewlines)
            return bounded.isEmpty ? nil : bounded
        }
    }

    public init(from decoder: Decoder) throws {
        let raw = try decoder.container(
            keyedBy: KagemushaFinalityDynamicCodingKey.self
        )
        let expectedKeys: Set<String> = [
            "target", "status_code", "reject_code", "message",
        ]
        guard Set(raw.allKeys.map(\.stringValue)).isSubset(of: expectedKeys)
        else {
            throw DecodingError.dataCorrupted(
                DecodingError.Context(
                    codingPath: decoder.codingPath,
                    debugDescription:
                        "Kagemusha definitive submission failure contains unknown fields"
                )
            )
        }
        let targetKey = KagemushaFinalityDynamicCodingKey("target")
        let statusKey = KagemushaFinalityDynamicCodingKey("status_code")
        let rejectKey = KagemushaFinalityDynamicCodingKey("reject_code")
        let messageKey = KagemushaFinalityDynamicCodingKey("message")
        try self.init(
            target: raw.decode(
                KagemushaSubmissionTarget.self,
                forKey: targetKey
            ),
            statusCode: raw.decode(Int.self, forKey: statusKey),
            rejectCode: raw.decode(String.self, forKey: rejectKey),
            message: raw.decodeIfPresent(String.self, forKey: messageKey)
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(
            keyedBy: KagemushaFinalityDynamicCodingKey.self
        )
        try container.encode(
            target,
            forKey: KagemushaFinalityDynamicCodingKey("target")
        )
        try container.encode(
            statusCode,
            forKey: KagemushaFinalityDynamicCodingKey("status_code")
        )
        try container.encode(
            rejectCode,
            forKey: KagemushaFinalityDynamicCodingKey("reject_code")
        )
        try container.encodeIfPresent(
            message,
            forKey: KagemushaFinalityDynamicCodingKey("message")
        )
    }

}

private struct KagemushaFinalityDynamicCodingKey: CodingKey {
    let stringValue: String
    let intValue: Int? = nil

    init(_ stringValue: String) {
        self.stringValue = stringValue
    }

    init?(stringValue: String) {
        self.init(stringValue)
    }

    init?(intValue: Int) {
        return nil
    }
}

/// Safe classification of a failed Kagemusha POST. No raw transport error or
/// unbounded response text escapes this value.
public enum KagemushaSubmissionFailureDisposition: Equatable, Sendable {
    /// Torii may have accepted the request; reconcile status and never issue a
    /// second POST in the same invocation.
    case ambiguous
    /// Torii definitively refused admission. Persist and freeze auto-replay.
    case definitivePreAdmission(KagemushaDefinitiveSubmissionFailure)
    /// The failure happened locally before admission could occur.
    case local
}

/// Endpoint-scoped post-admission-safe classification. Only exact status/code
/// pairs published by the authoritative Torii contract can be definitive.
public enum KagemushaSubmissionFailureClassifier {
    private static let commonBadRequestRejectCodes: Set<String> = [
        "idempotency_key_invalid",
        "idempotency_key_missing",
        "offline_amount_exceeds_limit",
        "offline_asset_not_found",
        "offline_asset_scale_invalid",
        "offline_asset_scale_mismatch",
        "offline_authorization_invalid",
        "offline_wrong_chain",
        "operation_id_invalid",
    ]
    private static let topUpBadRequestRejectCodes: Set<String> = [
        "offline_top_up_invalid",
        "offline_confidential_state_unavailable",
        "offline_topup_shield_verifier_unavailable",
        "offline_topup_shield_verifier_mismatch",
        "offline_confidential_state_invalid",
        "offline_topup_tree_full",
        "offline_topup_state_conflict",
        "offline_topup_snapshot_stale",
    ]
    private static let redeemBadRequestRejectCodes: Set<String> = [
        "offline_redeem_invalid",
    ]
    private static let offlineTransactionAdmissionBadRequestRejectCodes: Set<String> = [
        "transaction_rejected",
        "PRTRY:TX_UNSUPPORTED_AUTHORITY",
        "PRTRY:TX_SIGNATURE_ALGO_DENIED",
        "PRTRY:TX_SIGNATURE_INVALID",
        "PRTRY:TX_SIGNATURE_MALFORMED",
        "PRTRY:TX_SIGNATURE_MISSING",
        "PRTRY:TX_SIGNATURE_UNKNOWN_SIGNER",
        "PRTRY:TX_SIGNATURE_INSUFFICIENT",
        "ED07",
    ]
    private static let pipelineTransactionAdmissionBadRequestRejectCodes =
        offlineTransactionAdmissionBadRequestRejectCodes
    private static let transactionBadRequestRejectCodes: Set<String> = [
        "invalid_transaction_payload",
    ]
    private static let transactionForbiddenRejectCodes: Set<String> = [
        "PRTRY:QUEUE_GOVERNANCE_REJECTED",
        "PRTRY:QUEUE_LANE_COMPLIANCE_DENIED",
        "PRTRY:QUEUE_LANE_PRIVACY_PROOF_REJECTED",
        "PRTRY:NEXUS_FEE_ADMISSION_REJECTED",
        "PRTRY:CONFIDENTIAL_POLICY_REJECTED",
    ]
    private static let offlineForbiddenRejectCodes =
        transactionForbiddenRejectCodes.union(["offline_auth_header_unsupported"])
    private static let definitiveConflictRejectCodes: Set<String> = [
        "idempotency_key_conflict",
        "operation_id_conflict",
    ]

    /// Classify a failed POST using only the exact contract for `target`.
    /// Unknown HTTP 400s, duplicate/already-admitted 409s, rate limits, and
    /// availability failures always remain ambiguous.
    public static func classify(
        _ error: Error,
        target: KagemushaSubmissionTarget
    ) -> KagemushaSubmissionFailureDisposition {
        if error is CancellationError { return .local }
        if error is KagemushaOperationError { return .ambiguous }
        guard let toriiError = error as? ToriiClientError else { return .local }
        switch toriiError {
        case .transport, .invalidResponse, .emptyBody, .decoding, .invalidPayload:
            return .ambiguous
        case let .httpStatus(code, message, rejectCode):
            if isCanonicalDefinitivePreAdmission(
                target: target,
                statusCode: code,
                rejectCode: rejectCode
            ), let rejectCode {
                return .definitivePreAdmission(
                    KagemushaDefinitiveSubmissionFailure(
                        target: target,
                        sanitizingHTTPStatusCode: code,
                        rejectCode: rejectCode,
                        message: message
                    )
                )
            }
            return .ambiguous
        case .invalidURL, .stream, .dataModelMismatch,
             .transactionSchemaMismatch:
            return .local
        }
    }

    fileprivate static func isCanonicalDefinitivePreAdmission(
        target: KagemushaSubmissionTarget,
        statusCode: Int,
        rejectCode: String?
    ) -> Bool {
        guard let rejectCode else { return false }
        switch statusCode {
        case 400:
            switch target {
            case .offlineTopUp:
                return commonBadRequestRejectCodes.contains(rejectCode)
                    || topUpBadRequestRejectCodes.contains(rejectCode)
                    || offlineTransactionAdmissionBadRequestRejectCodes.contains(rejectCode)
            case .offlineRedeem:
                return commonBadRequestRejectCodes.contains(rejectCode)
                    || redeemBadRequestRejectCodes.contains(rejectCode)
                    || offlineTransactionAdmissionBadRequestRejectCodes.contains(rejectCode)
            case .signedTransaction:
                return transactionBadRequestRejectCodes.contains(rejectCode)
                    || pipelineTransactionAdmissionBadRequestRejectCodes.contains(rejectCode)
            }
        case 403:
            switch target {
            case .offlineTopUp, .offlineRedeem:
                return offlineForbiddenRejectCodes.contains(rejectCode)
            case .signedTransaction:
                return transactionForbiddenRejectCodes.contains(rejectCode)
            }
        case 409:
            // `PRTRY:ALREADY_COMMITTED` and `PRTRY:ALREADY_ENQUEUED` are
            // documented 409 values but explicitly signal possible prior
            // admission, so they remain ambiguous and status-only.
            return target != .signedTransaction
                && definitiveConflictRejectCodes.contains(rejectCode)
        default:
            return false
        }
    }
}

public enum KagemushaOperationFinalityError: Error, Equatable, Sendable {
    case invalidConfiguration(String)
    case continuityViolation(String)
    case alreadyResolving(String)
    case overallDeadlineExceeded
    case pollingDeadlineExceeded
}

extension KagemushaOperationFinalityError: LocalizedError {
    public var errorDescription: String? {
        switch self {
        case let .invalidConfiguration(field):
            return "Invalid Kagemusha finality configuration: \(field)."
        case let .continuityViolation(field):
            return "Kagemusha operation continuity validation failed: \(field)."
        case let .alreadyResolving(operationId):
            return "Kagemusha operation \(operationId) is already being resolved in this process."
        case .overallDeadlineExceeded:
            return "The Kagemusha operation exceeded its overall resolution deadline."
        case .pollingDeadlineExceeded:
            return "The Kagemusha operation did not reach a terminal result before the polling deadline."
        }
    }
}

/// Poll bounds for ``KagemushaOperationFinalityCoordinator``. The upper
/// limits prevent hostile or accidentally unbounded configuration from
/// turning one operation into an effectively permanent task.
public struct KagemushaOperationFinalityConfiguration: Equatable, Sendable {
    public static let maximumSupportedPollAttempts = 10_000
    public static let minimumSupportedPollingIntervalNanoseconds: UInt64 =
        1_000_000_000
    public static let maximumSupportedPollingIntervalNanoseconds: UInt64 =
        60_000_000_000
    /// Maximum time scheduled solely by polling sleeps.
    public static let maximumSupportedScheduledPollingNanoseconds: UInt64 =
        300_000_000_000
    public static let minimumSupportedOverallTimeoutNanoseconds: UInt64 =
        1_000_000_000
    public static let maximumSupportedOverallTimeoutNanoseconds: UInt64 =
        600_000_000_000

    public static let production = KagemushaOperationFinalityConfiguration(
        validatedMaximumPollAttempts: 120,
        validatedPollingIntervalNanoseconds: 1_000_000_000,
        validatedOverallTimeoutNanoseconds: 180_000_000_000
    )

    private init(
        validatedMaximumPollAttempts: Int,
        validatedPollingIntervalNanoseconds: UInt64,
        validatedOverallTimeoutNanoseconds: UInt64
    ) {
        self.maximumPollAttempts = validatedMaximumPollAttempts
        self.pollingIntervalNanoseconds = validatedPollingIntervalNanoseconds
        self.overallTimeoutNanoseconds = validatedOverallTimeoutNanoseconds
    }

    public let maximumPollAttempts: Int
    public let pollingIntervalNanoseconds: UInt64
    /// Monotonic wall-clock bound for the complete resolution, including
    /// transport calls, revalidation, polling sleeps, and status processing.
    public let overallTimeoutNanoseconds: UInt64

    public init(
        maximumPollAttempts: Int,
        pollingIntervalNanoseconds: UInt64,
        overallTimeoutNanoseconds: UInt64 = 600_000_000_000
    ) throws {
        guard (1...Self.maximumSupportedPollAttempts).contains(maximumPollAttempts) else {
            throw KagemushaOperationFinalityError.invalidConfiguration(
                "maximumPollAttempts"
            )
        }
        guard pollingIntervalNanoseconds
                >= Self.minimumSupportedPollingIntervalNanoseconds,
              pollingIntervalNanoseconds
                <= Self.maximumSupportedPollingIntervalNanoseconds else {
            throw KagemushaOperationFinalityError.invalidConfiguration(
                "pollingIntervalNanoseconds"
            )
        }
        // The coordinator delays before the first status poll and between
        // subsequent polls, so there can be one sleep per poll attempt.
        let scheduledSleepCount = UInt64(maximumPollAttempts)
        let (scheduledNanoseconds, overflow) = scheduledSleepCount
            .multipliedReportingOverflow(by: pollingIntervalNanoseconds)
        guard !overflow,
              scheduledNanoseconds
                <= Self.maximumSupportedScheduledPollingNanoseconds else {
            throw KagemushaOperationFinalityError.invalidConfiguration(
                "scheduledPollingNanoseconds"
            )
        }
        guard overallTimeoutNanoseconds
                >= Self.minimumSupportedOverallTimeoutNanoseconds,
              overallTimeoutNanoseconds
                <= Self.maximumSupportedOverallTimeoutNanoseconds else {
            throw KagemushaOperationFinalityError.invalidConfiguration(
                "overallTimeoutNanoseconds"
            )
        }
        guard overallTimeoutNanoseconds >= scheduledNanoseconds else {
            throw KagemushaOperationFinalityError.invalidConfiguration(
                "overallTimeoutNanoseconds"
            )
        }
        self.maximumPollAttempts = maximumPollAttempts
        self.pollingIntervalNanoseconds = pollingIntervalNanoseconds
        self.overallTimeoutNanoseconds = overallTimeoutNanoseconds
    }
}

/// Explicit durable acceptance continuity supplied on every resolution.
/// `.unaccepted` permits the exact authoritative-404 submission gate;
/// `.accepted` permanently disables that gate and binds all later status.
public enum KagemushaOperationContinuity: Equatable, Sendable {
    case unaccepted
    case accepted(transactionHash: String, submittedAtMs: UInt64?)
}

/// The request whose embedded operation identity controls the complete
/// status-first lifecycle. Callers cannot independently supply a kind or ID.
public enum KagemushaOperationSubmission: Equatable, Sendable {
    case topUp(KagemushaTopUpRequest)
    case redeem(KagemushaRedeemRequest)

    public var operationId: String {
        switch self {
        case let .topUp(request): request.operationId
        case let .redeem(request): request.operationId
        }
    }

    public var kind: KagemushaOperationKind {
        switch self {
        case .topUp: .topUp
        case .redeem: .redeem
        }
    }
}

/// Typed transport seam for the sole Kagemusha Torii lifecycle. The
/// submission receives the exact request that supplied the coordinator's
/// operation ID and kind.
public protocol KagemushaOperationFinalityTransport: Sendable {
    func getKagemushaOperationStatus(
        operationId: String,
        chainDiscriminant: UInt16
    ) async throws -> KagemushaOperationStatus

    func submitKagemushaOperation(
        _ operation: KagemushaOperationSubmission
    ) async throws -> KagemushaOperationReference
}

extension ToriiClient: KagemushaOperationFinalityTransport {
    public func submitKagemushaOperation(
        _ operation: KagemushaOperationSubmission
    ) async throws -> KagemushaOperationReference {
        switch operation {
        case let .topUp(request):
            return try await submitKagemushaTopUp(request)
        case let .redeem(request):
            return try await submitKagemushaRedeem(request)
        }
    }
}

/// Bounded terminal rejection metadata. The raw Torii error envelope is never
/// exposed through finality resolution.
public struct KagemushaOperationFinalRejection: Equatable, Sendable {
    public let operationId: String
    public let kind: KagemushaOperationKind
    public let transactionHash: String
    public let failure: KagemushaOperationTerminalFailure

    fileprivate init(
        operationId: String,
        kind: KagemushaOperationKind,
        transactionHash: String,
        failure: KagemushaOperationTerminalFailure
    ) {
        self.operationId = operationId
        self.kind = kind
        self.transactionHash = transactionHash
        self.failure = failure
    }
}

/// A bounded terminal result from the status resource or a definitive
/// pre-admission HTTP client failure.
public enum KagemushaOperationFinalityOutcome: Equatable, Sendable {
    case applied(KagemushaOperationStatus.Applied)
    case rejected(KagemushaOperationFinalRejection)
    case definitiveSubmissionFailure(KagemushaDefinitiveSubmissionFailure)
}

/// The authoritative terminal result paired with the caller's latest durable
/// state. The SDK never creates a competing journal or artifact store.
public struct KagemushaOperationFinalityResolution<State> {
    public let outcome: KagemushaOperationFinalityOutcome
    public let state: State

    public init(outcome: KagemushaOperationFinalityOutcome, state: State) {
        self.outcome = outcome
        self.state = state
    }
}

/// Status-first, idempotent sequencing for the sole first-release Kagemusha
/// top-up and redemption APIs.
///
/// Callers retain ownership of durable state through the persistence closures.
/// A submission is permitted only after the canonical status resource returns
/// an authoritative HTTP 404, readiness has been revalidated, and the exact
/// operation's attempt marker has been persisted. Ambiguous POST responses are
/// resolved exclusively through the status resource.
public enum KagemushaOperationFinalityCoordinator {
    typealias Sleeper = (_ nanoseconds: UInt64) async throws -> Void
    typealias MonotonicNow = () -> UInt64

    public static func resolve<State, Transport>(
        operation: KagemushaOperationSubmission,
        transport: Transport,
        chainDiscriminant: UInt16,
        initialState: State,
        continuity: KagemushaOperationContinuity,
        configuration: KagemushaOperationFinalityConfiguration = .production,
        existingDefinitiveSubmissionFailure: (State) throws
            -> KagemushaDefinitiveSubmissionFailure?,
        revalidateBeforeSubmission: @escaping (State) async throws -> Void,
        markSubmissionAttempt: (State) throws -> State,
        recordAcceptance: (KagemushaOperationReference, State) throws -> State,
        recordObservation: (
            _ transactionHash: String,
            _ submittedAtMs: UInt64?,
            _ state: State
        ) throws -> State,
        recordRejection: (
            _ transactionHash: String,
            _ failure: KagemushaOperationTerminalFailure,
            _ state: State
        ) throws -> State,
        recordDefinitiveSubmissionFailure: (
            _ failure: KagemushaDefinitiveSubmissionFailure,
            _ state: State
        ) throws -> State
    ) async throws -> KagemushaOperationFinalityResolution<State>
    where Transport: KagemushaOperationFinalityTransport {
        guard await KagemushaOperationFinalityLeaseRegistry.shared.acquire(
            operation.operationId
        ) else {
            throw KagemushaOperationFinalityError.alreadyResolving(
                operation.operationId
            )
        }
        do {
            let resolution = try await resolveForTesting(
                operationId: operation.operationId,
                expectedKind: operation.kind,
                initialState: initialState,
                continuity: continuity,
                configuration: configuration,
                sleep: { nanoseconds in
                    try await Task.sleep(nanoseconds: nanoseconds)
                },
                monotonicNow: {
                    DispatchTime.now().uptimeNanoseconds
                },
                existingDefinitiveSubmissionFailure:
                    existingDefinitiveSubmissionFailure,
                fetchStatus: { operationId in
                    try await transport.getKagemushaOperationStatus(
                        operationId: operationId,
                        chainDiscriminant: chainDiscriminant
                    )
                },
                revalidateBeforeSubmission: revalidateBeforeSubmission,
                markSubmissionAttempt: markSubmissionAttempt,
                submit: {
                    try await transport.submitKagemushaOperation(operation)
                },
                recordAcceptance: recordAcceptance,
                recordObservation: recordObservation,
                recordRejection: recordRejection,
                recordDefinitiveSubmissionFailure:
                    recordDefinitiveSubmissionFailure
            )
            await KagemushaOperationFinalityLeaseRegistry.shared.release(
                operation.operationId
            )
            return resolution
        } catch {
            await KagemushaOperationFinalityLeaseRegistry.shared.release(
                operation.operationId
            )
            throw error
        }
    }

    /// Closure-driven engine retained internally for deterministic SDK tests.
    /// Production callers must use the typed operation and transport overload.
    static func resolveForTesting<State>(
        operationId: String,
        expectedKind: KagemushaOperationKind,
        initialState: State,
        continuity: KagemushaOperationContinuity,
        configuration: KagemushaOperationFinalityConfiguration = .production,
        sleep: @escaping Sleeper = { nanoseconds in
            try await Task.sleep(nanoseconds: nanoseconds)
        },
        monotonicNow: @escaping MonotonicNow = {
            DispatchTime.now().uptimeNanoseconds
        },
        existingDefinitiveSubmissionFailure: (State) throws
            -> KagemushaDefinitiveSubmissionFailure?,
        fetchStatus: @escaping (_ operationId: String) async throws
            -> KagemushaOperationStatus,
        revalidateBeforeSubmission: @escaping (State) async throws -> Void,
        markSubmissionAttempt: (State) throws -> State,
        submit: @escaping () async throws -> KagemushaOperationReference,
        recordAcceptance: (KagemushaOperationReference, State) throws -> State,
        recordObservation: (
            _ transactionHash: String,
            _ submittedAtMs: UInt64?,
            _ state: State
        ) throws -> State,
        recordRejection: (
            _ transactionHash: String,
            _ failure: KagemushaOperationTerminalFailure,
            _ state: State
        ) throws -> State,
        recordDefinitiveSubmissionFailure: (
            _ failure: KagemushaDefinitiveSubmissionFailure,
            _ state: State
        ) throws -> State
    ) async throws -> KagemushaOperationFinalityResolution<State> {
        // Validate the operation identifier before any caller-owned side effect.
        _ = try KagemushaToriiAPI.operationPath(operationId)
        let startedAt = monotonicNow()
        let (overallDeadline, deadlineOverflow) = startedAt
            .addingReportingOverflow(configuration.overallTimeoutNanoseconds)
        guard !deadlineOverflow else {
            throw KagemushaOperationFinalityError.overallDeadlineExceeded
        }

        var state = initialState
        var boundTransactionHash: String?
        var boundSubmittedAtMs: UInt64?
        try initializeContinuitySeed(
            continuity: continuity,
            boundTransactionHash: &boundTransactionHash,
            boundSubmittedAtMs: &boundSubmittedAtMs
        )

        if let failure = try existingDefinitiveSubmissionFailure(state) {
            guard failure.target.operationKind == expectedKind else {
                throw KagemushaOperationFinalityError.continuityViolation(
                    "definitive submission failure operation kind"
                )
            }
            return KagemushaOperationFinalityResolution(
                outcome: .definitiveSubmissionFailure(failure),
                state: state
            )
        }
        var maySubmit = false
        var shouldDelayBeforePolling = false

        try Task.checkCancellation()
        let initialStatus: KagemushaOperationStatus?
        do {
            initialStatus = try await performBeforeDeadline(
                overallDeadline,
                monotonicNow: monotonicNow
            ) {
                try await fetchStatus(operationId)
            }
        } catch {
            if statusResourceIsMissing(after: error) {
                guard boundTransactionHash == nil else {
                    throw KagemushaOperationFinalityError.continuityViolation(
                        "accepted operation status missing"
                    )
                }
                initialStatus = nil
                maySubmit = true
            } else if statusFailureIsRetryable(error) {
                // A transient initial GET can never authorize a POST. Continue
                // with bounded status-only polling instead.
                initialStatus = nil
                maySubmit = false
                shouldDelayBeforePolling = true
            } else {
                throw error
            }
        }

        if let initialStatus {
            switch try observe(
                initialStatus,
                operationId: operationId,
                expectedKind: expectedKind,
                state: state,
                boundTransactionHash: &boundTransactionHash,
                boundSubmittedAtMs: &boundSubmittedAtMs,
                recordObservation: recordObservation,
                recordRejection: recordRejection
            ) {
            case let .terminal(resolution):
                return resolution
            case let .pending(observedState):
                state = observedState
                shouldDelayBeforePolling = true
            }
        }

        if maySubmit {
            try Task.checkCancellation()
            let stateForRevalidation = state
            try await performBeforeDeadline(
                overallDeadline,
                monotonicNow: monotonicNow
            ) {
                try await revalidateBeforeSubmission(stateForRevalidation)
            }
            try Task.checkCancellation()
            state = try markSubmissionAttempt(state)
            // If cancellation arrived during durable persistence, leave the
            // marker replayable and do not begin network submission.
            try Task.checkCancellation()
            let reference: KagemushaOperationReference?
            do {
                reference = try await performBeforeDeadline(
                    overallDeadline,
                    monotonicNow: monotonicNow,
                    operation: submit
                )
            } catch {
                switch KagemushaSubmissionFailureClassifier
                    .classify(
                        error,
                        target: KagemushaSubmissionTarget(
                            operationKind: expectedKind
                        )
                    ) {
                case let .definitivePreAdmission(failure):
                    state = try recordDefinitiveSubmissionFailure(
                        failure,
                        state
                    )
                    return KagemushaOperationFinalityResolution(
                        outcome: .definitiveSubmissionFailure(failure),
                        state: state
                    )
                case .ambiguous:
                    reference = nil
                    shouldDelayBeforePolling = true
                case .local:
                    throw error
                }
                // The durable attempt marker protects byte-identical replay.
                // Do not issue a second POST in this invocation.
            }
            if let reference {
                try validate(
                    reference,
                    operationId: operationId,
                    expectedKind: expectedKind
                )
                try bind(
                    reference.transactionHash,
                    submittedAtMs: reference.submittedAtMs,
                    to: &boundTransactionHash,
                    and: &boundSubmittedAtMs
                )
                state = try recordAcceptance(reference, state)
                shouldDelayBeforePolling = true
            }
        }

        if shouldDelayBeforePolling {
            try Task.checkCancellation()
            try await sleepBeforeDeadline(
                configuration.pollingIntervalNanoseconds,
                overallDeadline: overallDeadline,
                monotonicNow: monotonicNow,
                sleep: sleep
            )
        }

        for poll in 0..<configuration.maximumPollAttempts {
            try Task.checkCancellation()
            let status: KagemushaOperationStatus
            do {
                status = try await performBeforeDeadline(
                    overallDeadline,
                    monotonicNow: monotonicNow
                ) {
                    try await fetchStatus(operationId)
                }
            } catch {
                guard statusFailureIsRetryable(error) else { throw error }
                if poll < configuration.maximumPollAttempts - 1 {
                    try await sleepBeforeDeadline(
                        configuration.pollingIntervalNanoseconds,
                        overallDeadline: overallDeadline,
                        monotonicNow: monotonicNow,
                        sleep: sleep
                    )
                }
                continue
            }

            switch try observe(
                status,
                operationId: operationId,
                expectedKind: expectedKind,
                state: state,
                boundTransactionHash: &boundTransactionHash,
                boundSubmittedAtMs: &boundSubmittedAtMs,
                recordObservation: recordObservation,
                recordRejection: recordRejection
            ) {
            case let .terminal(resolution):
                return resolution
            case let .pending(observedState):
                state = observedState
                if poll < configuration.maximumPollAttempts - 1 {
                    try await sleepBeforeDeadline(
                        configuration.pollingIntervalNanoseconds,
                        overallDeadline: overallDeadline,
                        monotonicNow: monotonicNow,
                        sleep: sleep
                    )
                }
            }
        }

        throw KagemushaOperationFinalityError.pollingDeadlineExceeded
    }

    private static func sleepBeforeDeadline(
        _ nanoseconds: UInt64,
        overallDeadline: UInt64,
        monotonicNow: @escaping MonotonicNow,
        sleep: @escaping Sleeper
    ) async throws {
        let now = monotonicNow()
        guard now < overallDeadline,
              nanoseconds <= overallDeadline - now else {
            throw KagemushaOperationFinalityError.overallDeadlineExceeded
        }
        try await performBeforeDeadline(
            overallDeadline,
            monotonicNow: monotonicNow
        ) {
            try await sleep(nanoseconds)
        }
    }

    private static func performBeforeDeadline<Value>(
        _ overallDeadline: UInt64,
        monotonicNow: @escaping MonotonicNow,
        operation: @escaping () async throws -> Value
    ) async throws -> Value {
        try Task.checkCancellation()
        let now = monotonicNow()
        guard now < overallDeadline else {
            throw KagemushaOperationFinalityError.overallDeadlineExceeded
        }
        let race = KagemushaOperationDeadlineRace<Value>()
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                race.install(continuation)
                let operationTask = Task {
                    do {
                        race.resolve(.success(try await operation()))
                    } catch {
                        race.resolve(.failure(error))
                    }
                }
                let timeoutTask = Task {
                    do {
                        try await Task.sleep(
                            nanoseconds: overallDeadline - now
                        )
                        race.resolve(
                            .failure(
                                KagemushaOperationFinalityError
                                    .overallDeadlineExceeded
                            )
                        )
                    } catch {
                        // The operation or caller cancellation won the race.
                    }
                }
                race.installTasks(
                    operation: operationTask,
                    timeout: timeoutTask
                )
            }
        } onCancel: {
            race.resolve(.failure(CancellationError()))
        }
    }

    /// True only when Torii authoritatively reports that the canonical status
    /// resource does not exist. No transport or decoding failure is treated as
    /// proof of absence.
    public static func statusResourceIsMissing(after error: Error) -> Bool {
        guard let toriiError = error as? ToriiClientError else { return false }
        if case let .httpStatus(code, _, rejectCode) = toriiError {
            return code == 404 && rejectCode == "offline_operation_not_found"
        }
        return false
    }

    /// Whether a POST failure happened after acceptance may already have
    /// occurred and therefore must be reconciled by status polling.
    public static func submissionMayHaveBeenAccepted(
        after error: Error,
        operationKind: KagemushaOperationKind
    ) -> Bool {
        KagemushaSubmissionFailureClassifier.classify(
            error,
            target: KagemushaSubmissionTarget(operationKind: operationKind)
        ) == .ambiguous
    }

    /// Retry policy for the idempotent operation status resource. Rate limits
    /// and only the canonical operation-index/history/proof availability
    /// signals are retriable; they never authorize submission.
    public static func statusFailureIsRetryable(_ error: Error) -> Bool {
        if error is CancellationError { return false }
        guard let toriiError = error as? ToriiClientError else { return false }
        switch toriiError {
        case .transport:
            return true
        case let .httpStatus(code, _, _):
            if code == 429 { return true }
            if (500...599).contains(code) { return true }
            return false
        case .invalidURL, .invalidResponse, .emptyBody, .decoding,
             .invalidPayload, .stream, .dataModelMismatch,
             .transactionSchemaMismatch:
            return false
        }
    }

    private enum Observation<State> {
        case pending(State)
        case terminal(KagemushaOperationFinalityResolution<State>)
    }

    private static func observe<State>(
        _ status: KagemushaOperationStatus,
        operationId: String,
        expectedKind: KagemushaOperationKind,
        state: State,
        boundTransactionHash: inout String?,
        boundSubmittedAtMs: inout UInt64?,
        recordObservation: (
            _ transactionHash: String,
            _ submittedAtMs: UInt64?,
            _ state: State
        ) throws -> State,
        recordRejection: (
            _ transactionHash: String,
            _ failure: KagemushaOperationTerminalFailure,
            _ state: State
        ) throws -> State
    ) throws -> Observation<State> {
        switch status {
        case let .pending(pending):
            guard pending.operationId == operationId,
                  pending.kind == expectedKind else {
                throw KagemushaOperationFinalityError.continuityViolation(
                    "pending identity or kind"
                )
            }
            try bind(
                pending.transactionHash,
                submittedAtMs: pending.submittedAtMs,
                to: &boundTransactionHash,
                and: &boundSubmittedAtMs
            )
            return .pending(
                try recordObservation(
                    pending.transactionHash,
                    pending.submittedAtMs,
                    state
                )
            )

        case let .applied(applied):
            guard applied.operationId == operationId else {
                throw KagemushaOperationFinalityError.continuityViolation(
                    "applied operation identity"
                )
            }
            let transactionHash: String
            switch (expectedKind, applied.result) {
            case let (.topUp, .topUp(result)):
                transactionHash = result.transactionHash
            case let (.redeem, .redeem(result)):
                transactionHash = result.transactionHash
            default:
                throw KagemushaOperationFinalityError.continuityViolation(
                    "applied operation kind"
                )
            }
            try bind(
                transactionHash,
                submittedAtMs: nil,
                to: &boundTransactionHash,
                and: &boundSubmittedAtMs
            )
            let observedState = try recordObservation(
                transactionHash,
                nil,
                state
            )
            return .terminal(
                KagemushaOperationFinalityResolution(
                    outcome: .applied(applied),
                    state: observedState
                )
            )

        case let .rejected(rejected):
            guard rejected.operationId == operationId,
                  rejected.kind == expectedKind else {
                throw KagemushaOperationFinalityError.continuityViolation(
                    "rejected identity or kind"
                )
            }
            try bind(
                rejected.transactionHash,
                submittedAtMs: nil,
                to: &boundTransactionHash,
                and: &boundSubmittedAtMs
            )
            let observedState = try recordObservation(
                rejected.transactionHash,
                nil,
                state
            )
            let failure = KagemushaOperationTerminalFailure(
                code: rejected.error.code,
                message: rejected.error.message
            )
            let rejectedState = try recordRejection(
                rejected.transactionHash,
                failure,
                observedState
            )
            return .terminal(
                KagemushaOperationFinalityResolution(
                    outcome: .rejected(
                        KagemushaOperationFinalRejection(
                            operationId: rejected.operationId,
                            kind: rejected.kind,
                            transactionHash: rejected.transactionHash,
                            failure: failure
                        )
                    ),
                    state: rejectedState
                )
            )
        }
    }

    private static func validate(
        _ reference: KagemushaOperationReference,
        operationId: String,
        expectedKind: KagemushaOperationKind
    ) throws {
        let expectedStatusURI = try KagemushaToriiAPI.operationPath(operationId)
        guard reference.operationId == operationId,
              reference.kind == expectedKind,
              reference.state == .pending,
              reference.statusUri == expectedStatusURI,
              reference.submittedAtMs > 0 else {
            throw KagemushaOperationFinalityError.continuityViolation(
                "accepted operation reference"
            )
        }
    }

    private static func bind(
        _ transactionHash: String,
        submittedAtMs: UInt64?,
        to boundTransactionHash: inout String?,
        and boundSubmittedAtMs: inout UInt64?
    ) throws {
        guard isCanonicalHash(transactionHash) else {
            throw KagemushaOperationFinalityError.continuityViolation(
                "transaction hash"
            )
        }
        if let boundTransactionHash,
           boundTransactionHash != transactionHash {
            throw KagemushaOperationFinalityError.continuityViolation(
                "transaction hash"
            )
        }
        boundTransactionHash = transactionHash
        if let submittedAtMs {
            guard submittedAtMs > 0 else {
                throw KagemushaOperationFinalityError.continuityViolation(
                    "submitted timestamp"
                )
            }
            if let boundSubmittedAtMs,
               boundSubmittedAtMs != submittedAtMs {
                throw KagemushaOperationFinalityError.continuityViolation(
                    "submitted timestamp"
                )
            }
            boundSubmittedAtMs = submittedAtMs
        }
    }

    private static func initializeContinuitySeed(
        continuity: KagemushaOperationContinuity,
        boundTransactionHash: inout String?,
        boundSubmittedAtMs: inout UInt64?
    ) throws {
        guard case let .accepted(
            expectedTransactionHash,
            expectedSubmittedAtMs
        ) = continuity else {
            return
        }
        try bind(
            expectedTransactionHash,
            submittedAtMs: expectedSubmittedAtMs,
            to: &boundTransactionHash,
            and: &boundSubmittedAtMs
        )
    }

    private static func isCanonicalHash(_ value: String) -> Bool {
        let bytes = Array(value.utf8)
        return bytes.count == 64
            && bytes.contains(where: { $0 != UInt8(ascii: "0") })
            && bytes.allSatisfy {
                ($0 >= UInt8(ascii: "0") && $0 <= UInt8(ascii: "9"))
                    || ($0 >= UInt8(ascii: "a")
                        && $0 <= UInt8(ascii: "f"))
            }
    }

}

private actor KagemushaOperationFinalityLeaseRegistry {
    static let shared = KagemushaOperationFinalityLeaseRegistry()

    private var operationIds: Set<String> = []

    func acquire(_ operationId: String) -> Bool {
        operationIds.insert(operationId).inserted
    }

    func release(_ operationId: String) {
        operationIds.remove(operationId)
    }
}

/// An unstructured race lets the public resolver return at its real deadline
/// even when a custom transport fails to cooperate with cancellation. The
/// losing operation is cancelled and can no longer retain the keyed lease.
private final class KagemushaOperationDeadlineRace<Value>: @unchecked Sendable {
    private let lock = NSLock()
    private var continuation: CheckedContinuation<Value, Error>?
    private var pendingResult: Result<Value, Error>?
    private var operationTask: Task<Void, Never>?
    private var timeoutTask: Task<Void, Never>?
    private var resolved = false

    func install(_ continuation: CheckedContinuation<Value, Error>) {
        lock.lock()
        if let pendingResult {
            self.pendingResult = nil
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

    func installTasks(
        operation: Task<Void, Never>,
        timeout: Task<Void, Never>
    ) {
        lock.lock()
        guard !resolved else {
            lock.unlock()
            operation.cancel()
            timeout.cancel()
            return
        }
        operationTask = operation
        timeoutTask = timeout
        lock.unlock()
    }

    func resolve(_ result: Result<Value, Error>) {
        lock.lock()
        guard !resolved else {
            lock.unlock()
            return
        }
        resolved = true
        let continuation = self.continuation
        self.continuation = nil
        if continuation == nil {
            pendingResult = result
        }
        let operationTask = self.operationTask
        let timeoutTask = self.timeoutTask
        self.operationTask = nil
        self.timeoutTask = nil
        lock.unlock()

        operationTask?.cancel()
        timeoutTask?.cancel()
        continuation?.resume(with: result)
    }
}
