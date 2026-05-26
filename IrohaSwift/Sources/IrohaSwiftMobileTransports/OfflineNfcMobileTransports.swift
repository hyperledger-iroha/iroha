import Foundation
import IrohaSwift

#if os(iOS) && canImport(CoreNFC)
@preconcurrency import CoreNFC
#endif

#if os(iOS) && canImport(PassKit)
@preconcurrency import PassKit
#endif

public struct IrohaOfflineDeviceTransferSendResult: Equatable, Sendable {
    public let paymentToken: String
    public let receiptAck: String

    public init(paymentToken: String, receiptAck: String) {
        self.paymentToken = paymentToken
        self.receiptAck = receiptAck
    }
}

public enum IrohaOfflineReceiveDiagnosticReason: String, Equatable, Sendable {
    case missingEntitlementOrProfile = "missing_entitlement_or_profile"
    case unsupportedDevice = "unsupported_device"
    case ineligibleDevice = "ineligible_device"
    case presentmentDenied = "presentment_denied"
    case timeout = "timeout"
    case peerRejected = "peer_rejected"
    case checksumMismatch = "checksum_mismatch"
    case userCancelled = "user_cancelled"
    case invalidPayload = "invalid_payload"
    case unavailable = "unavailable"
}

public struct IrohaOfflineNfcCardSessionAvailability: Equatable, Sendable {
    public let isAvailable: Bool
    public let diagnosticReason: IrohaOfflineReceiveDiagnosticReason?

    public init(isAvailable: Bool, diagnosticReason: IrohaOfflineReceiveDiagnosticReason?) {
        self.isAvailable = isAvailable
        self.diagnosticReason = diagnosticReason
    }

    public static let available = IrohaOfflineNfcCardSessionAvailability(
        isAvailable: true,
        diagnosticReason: nil
    )

    public static func unavailable(_ reason: IrohaOfflineReceiveDiagnosticReason) -> IrohaOfflineNfcCardSessionAvailability {
        IrohaOfflineNfcCardSessionAvailability(isAvailable: false, diagnosticReason: reason)
    }
}

public enum IrohaOfflineNfcExchangeError: LocalizedError, Equatable, Sendable {
    case unavailable
    case cardEmulationUnavailable
    case cardSessionMissingEntitlementOrProfile
    case cardSessionIneligible
    case presentmentDenied
    case cardSessionTimeout
    case nfcTimeout
    case invalidPeer
    case peerRejected(statusWord: UInt16?)
    case invalidPayload
    case checksumMismatch
    case ackPending
    case cancelled

    public var errorDescription: String? {
        switch self {
        case .unavailable:
            return "NFC reader transfer is unavailable on this device."
        case .cardEmulationUnavailable:
            return "NFC card-session receive is unavailable on this device or build."
        case .cardSessionMissingEntitlementOrProfile:
            return "NFC card-session receive requires an entitlement-enabled app profile and runtime opt-in."
        case .cardSessionIneligible:
            return "This device is not eligible for NFC card-session receive."
        case .presentmentDenied:
            return "NFC presentment was denied by the system."
        case .cardSessionTimeout:
            return "NFC card-session receive timed out."
        case .nfcTimeout:
            return "NFC transfer timed out."
        case .invalidPeer:
            return "The NFC peer did not present a valid offline transfer endpoint."
        case .peerRejected:
            return "The NFC peer rejected the transfer."
        case .invalidPayload:
            return "The NFC offline transfer payload was invalid."
        case .checksumMismatch:
            return "The NFC offline transfer payload checksum did not match."
        case .ackPending:
            return "The payment token was delivered but receipt acknowledgement is still pending."
        case .cancelled:
            return "NFC transfer was cancelled."
        }
    }

    public var receiveDiagnosticReason: IrohaOfflineReceiveDiagnosticReason {
        switch self {
        case .cardSessionMissingEntitlementOrProfile:
            return .missingEntitlementOrProfile
        case .cardSessionIneligible:
            return .ineligibleDevice
        case .presentmentDenied:
            return .presentmentDenied
        case .cardSessionTimeout, .nfcTimeout:
            return .timeout
        case .peerRejected:
            return .peerRejected
        case .checksumMismatch:
            return .checksumMismatch
        case .cancelled:
            return .userCancelled
        case .invalidPayload, .invalidPeer, .ackPending:
            return .invalidPayload
        case .unavailable:
            return .unavailable
        case .cardEmulationUnavailable:
            return .unsupportedDevice
        }
    }

    public var shouldRetryPreparedPaymentTransfer: Bool {
        switch self {
        case .ackPending, .nfcTimeout:
            return true
        case .peerRejected(let statusWord):
            return statusWord == nil || statusWord == 0x6985
        case .unavailable,
             .cardEmulationUnavailable,
             .cardSessionMissingEntitlementOrProfile,
             .cardSessionIneligible,
             .presentmentDenied,
             .cardSessionTimeout,
             .invalidPeer,
             .invalidPayload,
             .checksumMismatch,
             .cancelled:
            return false
        }
    }

    public var technicalCode: String {
        switch self {
        case .unavailable:
            return "IrohaOfflineNfcExchangeError.unavailable"
        case .cardEmulationUnavailable:
            return "IrohaOfflineNfcExchangeError.cardEmulationUnavailable"
        case .cardSessionMissingEntitlementOrProfile:
            return "IrohaOfflineNfcExchangeError.cardSessionMissingEntitlementOrProfile"
        case .cardSessionIneligible:
            return "IrohaOfflineNfcExchangeError.cardSessionIneligible"
        case .presentmentDenied:
            return "IrohaOfflineNfcExchangeError.presentmentDenied"
        case .cardSessionTimeout:
            return "IrohaOfflineNfcExchangeError.cardSessionTimeout"
        case .nfcTimeout:
            return "IrohaOfflineNfcExchangeError.nfcTimeout"
        case .invalidPeer:
            return "IrohaOfflineNfcExchangeError.invalidPeer"
        case .peerRejected(let statusWord):
            guard let statusWord else {
                return "IrohaOfflineNfcExchangeError.peerRejected.nil"
            }
            return String(format: "IrohaOfflineNfcExchangeError.peerRejected.%04X", Int(statusWord))
        case .invalidPayload:
            return "IrohaOfflineNfcExchangeError.invalidPayload"
        case .checksumMismatch:
            return "IrohaOfflineNfcExchangeError.checksumMismatch"
        case .ackPending:
            return "IrohaOfflineNfcExchangeError.ackPending"
        case .cancelled:
            return "IrohaOfflineNfcExchangeError.cancelled"
        }
    }
}

public struct IrohaOfflineNfcConfiguration: Equatable, Sendable {
    public let applicationIdentifier: Data
    public let cardSessionRuntimeEnabled: Bool
    public let allowsDebugForcedCardSessionAvailability: Bool
    public let walletPassSuppressionEnabled: Bool
    public let cardSessionAlertMessage: String
    public let readerSendAlertMessage: String
    public let readerReceiveAlertMessage: String
    public let preparedPaymentRetapAlertMessage: String
    public let paymentConfirmedAlertMessage: String
    public let paymentReceivedAlertMessage: String
    public let ackWaitSeconds: TimeInterval
    public let ackPollIntervalNanoseconds: UInt64
    public let apduTimeoutSeconds: TimeInterval
    public let maxPreparedPaymentSessionRestarts: Int
    public let maxInitialExchangeRetries: Int

    public init(
        applicationIdentifier: Data = OfflineNoteNfcApduProtocol.aid,
        cardSessionRuntimeEnabled: Bool = false,
        allowsDebugForcedCardSessionAvailability: Bool = false,
        walletPassSuppressionEnabled: Bool = true,
        cardSessionAlertMessage: String = "Touch the top of both phones together. Keep still.",
        readerSendAlertMessage: String = "Touch the top of this phone to the other phone.",
        readerReceiveAlertMessage: String = "Touch the top of both phones together. Keep still.",
        preparedPaymentRetapAlertMessage: String = "Payment prepared. Touch the phones together again to confirm.",
        paymentConfirmedAlertMessage: String = "Offline cash confirmed.",
        paymentReceivedAlertMessage: String = "Offline cash received.",
        ackWaitSeconds: TimeInterval = 45,
        ackPollIntervalNanoseconds: UInt64 = 250_000_000,
        apduTimeoutSeconds: TimeInterval = 6,
        maxPreparedPaymentSessionRestarts: Int = 4,
        maxInitialExchangeRetries: Int = 3
    ) {
        self.applicationIdentifier = applicationIdentifier
        self.cardSessionRuntimeEnabled = cardSessionRuntimeEnabled
        self.allowsDebugForcedCardSessionAvailability = allowsDebugForcedCardSessionAvailability
        self.walletPassSuppressionEnabled = walletPassSuppressionEnabled
        self.cardSessionAlertMessage = cardSessionAlertMessage
        self.readerSendAlertMessage = readerSendAlertMessage
        self.readerReceiveAlertMessage = readerReceiveAlertMessage
        self.preparedPaymentRetapAlertMessage = preparedPaymentRetapAlertMessage
        self.paymentConfirmedAlertMessage = paymentConfirmedAlertMessage
        self.paymentReceivedAlertMessage = paymentReceivedAlertMessage
        self.ackWaitSeconds = ackWaitSeconds
        self.ackPollIntervalNanoseconds = ackPollIntervalNanoseconds
        self.apduTimeoutSeconds = apduTimeoutSeconds
        self.maxPreparedPaymentSessionRestarts = maxPreparedPaymentSessionRestarts
        self.maxInitialExchangeRetries = maxInitialExchangeRetries
    }

    public var applicationIdentifierHex: String {
        OfflineNoteNfcApduProtocol.aidHex(for: applicationIdentifier)
    }

    public func selectAidAPDUData() -> Data {
        OfflineNoteNfcApduProtocol.selectAidAPDUData(aid: applicationIdentifier)
    }
}

private enum IrohaOfflineDeviceTransferTextPayload {
    private static let boundaryWhitespace = CharacterSet(charactersIn: " \t\r\n")

    static func normalize(
        _ payload: String,
        expectedKind: OfflineNoteTextPayloadKind
    ) throws -> String {
        let trimmedPayload = payload.trimmingCharacters(in: boundaryWhitespace)
        guard !trimmedPayload.isEmpty else {
            throw IrohaOfflineNfcExchangeError.invalidPayload
        }
        return try OfflineNoteTransferHandoff.normalizeTextTransportPayload(
            trimmedPayload,
            expectedKind: expectedKind
        )
    }

    static func kind(for nfcKind: OfflineNoteNfcPayloadKind) -> OfflineNoteTextPayloadKind {
        OfflineNoteTransferHandoff.textPayloadKind(for: nfcKind)
    }
}

private struct IrohaOfflineNfcCardPayload {
    let kind: OfflineNoteNfcPayloadKind
    let payload: String
    let peerMaxChunkLength: Int
}

private struct IrohaOfflineNfcPreparedPayment: Equatable {
    let receiveRequestPayload: String
    let paymentToken: String

    func resumeAction(
        peerKind: OfflineNoteNfcPayloadKind,
        peerPayload: String
    ) throws -> IrohaOfflineNfcPreparedPaymentResumeAction {
        switch peerKind {
        case .receiveRequest:
            guard peerPayload == receiveRequestPayload else {
                throw IrohaOfflineNfcExchangeError.invalidPeer
            }
            return .deliverPayment
        case .receiptAck:
            let trimmed = try IrohaOfflineDeviceTransferTextPayload.normalize(
                peerPayload,
                expectedKind: .receiptAck
            )
            return .acceptReceiptAck(trimmed)
        case .paymentToken:
            throw IrohaOfflineNfcExchangeError.invalidPeer
        }
    }
}

private enum IrohaOfflineNfcPreparedPaymentResumeAction: Equatable {
    case deliverPayment
    case acceptReceiptAck(String)
}

private final class IrohaOfflineWalletPassPresentationSuppression {
    private let enabled: Bool

    init(enabled: Bool) {
        self.enabled = enabled
    }

#if os(iOS) && canImport(PassKit)
    private var requestToken: PKSuppressionRequestToken?
    private var requested = false

    func begin() {
        guard enabled else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self, !self.requested else { return }
            self.requested = true
            self.requestToken = PKPassLibrary.requestAutomaticPassPresentationSuppression { result in
                NSLog("iroha_offline_nfc_ios_wallet_suppression result=%@", Self.describe(result))
            }
        }
    }

    func end() {
        guard enabled else { return }
        DispatchQueue.main.async { [weak self] in
            guard let self, self.requested else { return }
            self.requested = false
            if let requestToken = self.requestToken {
                PKPassLibrary.endAutomaticPassPresentationSuppression(withRequestToken: requestToken)
            }
            self.requestToken = nil
            NSLog("iroha_offline_nfc_ios_wallet_suppression ended")
        }
    }

    private static func describe(_ result: PKAutomaticPassPresentationSuppressionResult) -> String {
        switch result {
        case .notSupported:
            return "not_supported"
        case .alreadyPresenting:
            return "already_presenting"
        case .denied:
            return "denied"
        case .cancelled:
            return "cancelled"
        case .success:
            return "success"
        @unknown default:
            return "unknown"
        }
    }
#else
    func begin() {}
    func end() {}
#endif
}

#if os(iOS) && canImport(CoreNFC)
public final class IrohaOfflineNfcCardSessionController {
    private let configuration: IrohaOfflineNfcConfiguration
    private var runtime: AnyObject?

    public init(configuration: IrohaOfflineNfcConfiguration = IrohaOfflineNfcConfiguration()) {
        self.configuration = configuration
    }

    public static func isCardEmulationAvailable(
        configuration: IrohaOfflineNfcConfiguration = IrohaOfflineNfcConfiguration()
    ) -> Bool {
#if DEBUG
        if configuration.allowsDebugForcedCardSessionAvailability,
           ProcessInfo.processInfo.environment["UITEST_FORCE_HCE_CARDSESSION_AVAILABLE"] == "1" {
            return true
        }
#endif
        guard configuration.cardSessionRuntimeEnabled else {
            return false
        }
        if #available(iOS 17.4, *) {
            return NFCReaderSession.readingAvailable && CardSession.isSupported
        }
        return false
    }

    public static func cardSessionAvailability(
        configuration: IrohaOfflineNfcConfiguration = IrohaOfflineNfcConfiguration()
    ) async -> IrohaOfflineNfcCardSessionAvailability {
#if DEBUG
        if configuration.allowsDebugForcedCardSessionAvailability,
           ProcessInfo.processInfo.environment["UITEST_FORCE_HCE_CARDSESSION_AVAILABLE"] == "1" {
            return .available
        }
#endif
        guard configuration.cardSessionRuntimeEnabled else {
            return .unavailable(.missingEntitlementOrProfile)
        }
        guard NFCReaderSession.readingAvailable else {
            return .unavailable(.unavailable)
        }
        guard #available(iOS 17.4, *) else {
            return .unavailable(.unsupportedDevice)
        }
        guard CardSession.isSupported else {
            return .unavailable(.unsupportedDevice)
        }
        guard await CardSession.isEligible else {
            return .unavailable(.ineligibleDevice)
        }
        return .available
    }

    public func startReceiveRequest(
        _ payload: String,
        onEmulationStarted: (() -> Void)? = nil,
        onReceiptAckReady: (() -> Void)? = nil,
        onReceiptAckRead: (() -> Void)? = nil,
        onSessionInvalidated: ((IrohaOfflineNfcExchangeError) -> Void)? = nil,
        onDiagnostic: ((IrohaOfflineReceiveDiagnosticReason) -> Void)? = nil,
        onIncomingPaymentToken: @escaping (String) async throws -> String
    ) async throws {
        guard #available(iOS 17.4, *) else {
            throw IrohaOfflineNfcExchangeError.cardEmulationUnavailable
        }
        guard configuration.cardSessionRuntimeEnabled else {
            throw IrohaOfflineNfcExchangeError.cardSessionMissingEntitlementOrProfile
        }
        let runtime = try IrohaOfflineNfcCardSessionRuntime(
            configuration: configuration,
            receiveRequestPayload: payload,
            onIncomingPaymentToken: onIncomingPaymentToken,
            onEmulationStarted: onEmulationStarted,
            onSessionInvalidated: onSessionInvalidated,
            onDiagnostic: onDiagnostic,
            onReceiptAckReady: onReceiptAckReady,
            onReceiptAckRead: onReceiptAckRead
        )
        self.runtime = runtime
        try await runtime.start()
    }

    public func stop() {
        if #available(iOS 17.4, *) {
            (runtime as? IrohaOfflineNfcCardSessionRuntime)?.stop()
        }
        runtime = nil
    }
}

@available(iOS 17.4, *)
private final class IrohaOfflineNfcCardSessionRuntime {
    private struct HandleResult {
        let response: Data
        let receiptAckRead: Bool
    }

    private let configuration: IrohaOfflineNfcConfiguration
    private let lock = NSLock()
    private var cardSession: CardSession?
    private var presentmentIntent: NFCPresentmentIntentAssertion?
    private var eventTask: Task<Void, Never>?
    private let passPresentationSuppression: IrohaOfflineWalletPassPresentationSuppression
    private var currentKind: OfflineNoteNfcPayloadKind = .receiveRequest
    private var currentPayload: String
    private var currentPayloadBytes: Data
    private var currentPayloadInfo: Data
    private var readable = true
    private var pendingWrite: OfflineNoteNfcPayloadAssembler?
    private var didComplete = false
    private var didNotifyEmulationStarted = false
    private let onIncomingPaymentToken: (String) async throws -> String
    private let onEmulationStarted: (() -> Void)?
    private let onSessionInvalidated: ((IrohaOfflineNfcExchangeError) -> Void)?
    private let onDiagnostic: ((IrohaOfflineReceiveDiagnosticReason) -> Void)?
    private let onReceiptAckReady: (() -> Void)?
    private let onReceiptAckRead: (() -> Void)?

    init(
        configuration: IrohaOfflineNfcConfiguration,
        receiveRequestPayload: String,
        onIncomingPaymentToken: @escaping (String) async throws -> String,
        onEmulationStarted: (() -> Void)?,
        onSessionInvalidated: ((IrohaOfflineNfcExchangeError) -> Void)?,
        onDiagnostic: ((IrohaOfflineReceiveDiagnosticReason) -> Void)?,
        onReceiptAckReady: (() -> Void)?,
        onReceiptAckRead: (() -> Void)?
    ) throws {
        let trimmedPayload = try IrohaOfflineDeviceTransferTextPayload.normalize(
            receiveRequestPayload,
            expectedKind: .receiveRequest
        )
        let payloadBytes = Data(trimmedPayload.utf8)
        self.configuration = configuration
        self.passPresentationSuppression = IrohaOfflineWalletPassPresentationSuppression(
            enabled: configuration.walletPassSuppressionEnabled
        )
        self.currentPayload = trimmedPayload
        self.currentPayloadBytes = payloadBytes
        self.currentPayloadInfo = try OfflineNoteNfcApduProtocol.encodeInfo(
            kind: .receiveRequest,
            payloadBytes: payloadBytes
        )
        self.onIncomingPaymentToken = onIncomingPaymentToken
        self.onEmulationStarted = onEmulationStarted
        self.onSessionInvalidated = onSessionInvalidated
        self.onDiagnostic = onDiagnostic
        self.onReceiptAckReady = onReceiptAckReady
        self.onReceiptAckRead = onReceiptAckRead
    }

    deinit {
        passPresentationSuppression.end()
    }

    func start() async throws {
        guard NFCReaderSession.readingAvailable else {
            throw IrohaOfflineNfcExchangeError.unavailable
        }
        guard CardSession.isSupported else {
            throw IrohaOfflineNfcExchangeError.cardEmulationUnavailable
        }
        guard await CardSession.isEligible else {
            throw IrohaOfflineNfcExchangeError.cardSessionIneligible
        }
        passPresentationSuppression.begin()
        do {
            presentmentIntent = try await NFCPresentmentIntentAssertion.acquire()
            NSLog("iroha_offline_nfc_ios_card presentment_assertion_acquired")
        } catch {
            passPresentationSuppression.end()
            NSLog("iroha_offline_nfc_ios_card presentment_denied")
            throw IrohaOfflineNfcExchangeError.presentmentDenied
        }
        let session: CardSession
        do {
            session = try await CardSession()
        } catch {
            passPresentationSuppression.end()
            NSLog("iroha_offline_nfc_ios_card create_failed")
            throw IrohaOfflineNfcExchangeError.cardEmulationUnavailable
        }
        session.alertMessage = configuration.cardSessionAlertMessage
        cardSession = session
        eventTask = Task { [weak self] in
            await self?.runEventLoop(session: session)
        }
    }

    func stop() {
        eventTask?.cancel()
        eventTask = nil
        cardSession?.invalidate()
        cardSession = nil
        presentmentIntent = nil
        passPresentationSuppression.end()
    }

    private func runEventLoop(session: CardSession) async {
        do {
            for try await event in session.eventStream {
                switch event {
                case .sessionStarted:
                    NSLog("iroha_offline_nfc_ios_card session_started")
                    do {
                        try await startEmulationIfNeeded(session: session)
                    } catch {
                        NSLog(
                            "iroha_offline_nfc_ios_card session_start_emulation_deferred error_type=%@",
                            Self.safeErrorType(error)
                        )
                    }
                case .readerDetected:
                    NSLog("iroha_offline_nfc_ios_card reader_detected")
                    do {
                        try await startEmulationIfNeeded(session: session)
                    } catch {
                        NSLog("iroha_offline_nfc_ios_card start_emulation_failed")
                        notifyInvalidated(.presentmentDenied)
                        await session.stopEmulation(status: .failure)
                        return
                    }
                case .received(let apdu):
                    let result = handle(apdu.payload)
                    try await apdu.respond(response: result.response)
                    if result.receiptAckRead {
                        markComplete()
                        onReceiptAckRead?()
                        await session.stopEmulation(status: .success)
                    }
                case .readerDeselected:
                    NSLog("iroha_offline_nfc_ios_card reader_deselected")
                case .sessionInvalidated(let reason):
                    NSLog("iroha_offline_nfc_ios_card session_invalidated reason=%@", String(describing: reason))
                    notifyInvalidated(Self.exchangeError(forInvalidationReason: reason))
                    return
                @unknown default:
                    break
                }
            }
        } catch {
            NSLog("iroha_offline_nfc_ios_card event_loop_error_type=%@", Self.safeErrorType(error))
            notifyInvalidated(.cardEmulationUnavailable)
        }
    }

    private func startEmulationIfNeeded(session: CardSession) async throws {
        guard !(await session.isEmulationInProgress) else { return }
        try await session.startEmulation()
        NSLog("iroha_offline_nfc_ios_card emulation_started")
        notifyEmulationStarted()
    }

    private static func exchangeError(forInvalidationReason reason: CardSession.Error) -> IrohaOfflineNfcExchangeError {
        switch reason {
        case .maxSessionDurationReached:
            return .cardSessionTimeout
        case .userInvalidated:
            return .cancelled
        case .systemEligibilityFailed:
            return .cardSessionIneligible
        case .accessNotAccepted:
            return .presentmentDenied
        case .invalidated, .transmissionError, .systemNotAvailable, .emulationStopped, .radioDisabled:
            return .cardEmulationUnavailable
        @unknown default:
            return .cardEmulationUnavailable
        }
    }

    private func notifyInvalidated(_ error: IrohaOfflineNfcExchangeError) {
        lock.lock()
        let shouldNotify = !didComplete
        if shouldNotify {
            didComplete = true
        }
        lock.unlock()
        guard shouldNotify else { return }
        onSessionInvalidated?(error)
    }

    private func notifyEmulationStarted() {
        lock.lock()
        let shouldNotify = !didNotifyEmulationStarted
        if shouldNotify {
            didNotifyEmulationStarted = true
        }
        lock.unlock()
        guard shouldNotify else { return }
        onEmulationStarted?()
    }

    private func markComplete() {
        lock.lock()
        didComplete = true
        lock.unlock()
    }

    private func handle(_ commandAPDU: Data) -> HandleResult {
        lock.lock()
        defer { lock.unlock() }

        switch OfflineNoteNfcApduProtocol.parseCommand(commandAPDU, aid: configuration.applicationIdentifier) {
        case .select:
            NSLog("iroha_offline_nfc_ios_card apdu_select")
            pendingWrite = nil
            return HandleResult(response: OfflineNoteNfcApduProtocol.response(), receiptAckRead: false)
        case .getInfo:
            NSLog(
                "iroha_offline_nfc_ios_card apdu_get_info kind=%@ length=%ld readable=%@",
                String(describing: currentKind),
                currentPayloadBytes.count,
                readable ? "yes" : "no"
            )
            guard readable else {
                return HandleResult(response: OfflineNoteNfcApduProtocol.statusConditionsNotSatisfied, receiptAckRead: false)
            }
            return HandleResult(
                response: OfflineNoteNfcApduProtocol.response(currentPayloadInfo),
                receiptAckRead: false
            )
        case .readChunk(let offset, let requestedLength):
            return readChunk(offset: offset, requestedLength: requestedLength)
        case .writeMeta(let kind, let payloadLength, let sha256):
            NSLog(
                "iroha_offline_nfc_ios_card apdu_write_meta kind=%@ length=%ld checksum_len=%ld",
                String(describing: kind),
                payloadLength,
                sha256.count
            )
            return beginWrite(kind: kind, payloadLength: payloadLength, sha256: sha256)
        case .writeChunk(let offset, let bytes):
            return writeChunk(offset: offset, bytes: bytes)
        case .commit:
            NSLog("iroha_offline_nfc_ios_card apdu_commit")
            return commitWrite()
        case .unsupported:
            NSLog("iroha_offline_nfc_ios_card apdu_unsupported length=%ld", commandAPDU.count)
            return HandleResult(response: OfflineNoteNfcApduProtocol.statusUnsupported, receiptAckRead: false)
        case .invalid:
            NSLog("iroha_offline_nfc_ios_card apdu_invalid length=%ld", commandAPDU.count)
            return HandleResult(response: OfflineNoteNfcApduProtocol.statusWrongData, receiptAckRead: false)
        }
    }

    private func readChunk(offset: Int, requestedLength: Int) -> HandleResult {
        guard readable else {
            return HandleResult(response: OfflineNoteNfcApduProtocol.statusConditionsNotSatisfied, receiptAckRead: false)
        }
        guard offset >= 0, offset < currentPayloadBytes.count else {
            return HandleResult(response: OfflineNoteNfcApduProtocol.statusWrongData, receiptAckRead: false)
        }
        let chunkLength = min(max(requestedLength, 1), OfflineNoteNfcApduProtocol.maxExtendedReadChunkBytes)
        let end = min(offset + chunkLength, currentPayloadBytes.count)
        let receiptAckRead = end == currentPayloadBytes.count && currentKind == .receiptAck
        return HandleResult(
            response: OfflineNoteNfcApduProtocol.response(currentPayloadBytes.subdata(in: offset..<end)),
            receiptAckRead: receiptAckRead
        )
    }

    private func beginWrite(kind: OfflineNoteNfcPayloadKind, payloadLength: Int, sha256: Data) -> HandleResult {
        guard payloadLength <= OfflineNoteNfcApduProtocol.maxIncomingPayloadBytes else {
            return HandleResult(response: OfflineNoteNfcApduProtocol.statusWrongData, receiptAckRead: false)
        }
        do {
            pendingWrite = try OfflineNoteNfcPayloadAssembler(
                kind: kind,
                expectedLength: payloadLength,
                expectedSha256: sha256
            )
            return HandleResult(response: OfflineNoteNfcApduProtocol.response(), receiptAckRead: false)
        } catch {
            return HandleResult(response: OfflineNoteNfcApduProtocol.statusWrongData, receiptAckRead: false)
        }
    }

    private func writeChunk(offset: Int, bytes: Data) -> HandleResult {
        guard let pendingWrite else {
            NSLog(
                "iroha_offline_nfc_ios_card apdu_write_chunk_rejected reason=no_pending_write offset=%ld length=%ld",
                offset,
                bytes.count
            )
            return HandleResult(response: OfflineNoteNfcApduProtocol.statusConditionsNotSatisfied, receiptAckRead: false)
        }
        guard pendingWrite.write(offset: offset, chunk: bytes) else {
            NSLog(
                "iroha_offline_nfc_ios_card apdu_write_chunk_rejected reason=invalid_chunk offset=%ld length=%ld expected_length=%ld written=%ld",
                offset,
                bytes.count,
                pendingWrite.expectedLength,
                pendingWrite.writtenByteCount
            )
            return HandleResult(response: OfflineNoteNfcApduProtocol.statusWrongData, receiptAckRead: false)
        }
        NSLog(
            "iroha_offline_nfc_ios_card apdu_write_chunk offset=%ld length=%ld written=%ld expected_length=%ld",
            offset,
            bytes.count,
            pendingWrite.writtenByteCount,
            pendingWrite.expectedLength
        )
        return HandleResult(response: OfflineNoteNfcApduProtocol.response(), receiptAckRead: false)
    }

    private func commitWrite() -> HandleResult {
        guard let pendingWrite else {
            NSLog("iroha_offline_nfc_ios_card apdu_commit_rejected reason=no_pending_write")
            return HandleResult(response: OfflineNoteNfcApduProtocol.statusConditionsNotSatisfied, receiptAckRead: false)
        }
        let payloadBytes: Data
        do {
            payloadBytes = try pendingWrite.commit()
        } catch OfflineNoteNfcApduError.checksumMismatch {
            NSLog(
                "iroha_offline_nfc_ios_card apdu_commit_rejected reason=checksum_mismatch written=%ld expected_length=%ld",
                pendingWrite.writtenByteCount,
                pendingWrite.expectedLength
            )
            onDiagnostic?(.checksumMismatch)
            return HandleResult(response: OfflineNoteNfcApduProtocol.statusWrongData, receiptAckRead: false)
        } catch OfflineNoteNfcApduError.incompletePayload {
            NSLog(
                "iroha_offline_nfc_ios_card apdu_commit_rejected reason=incomplete_payload written=%ld expected_length=%ld",
                pendingWrite.writtenByteCount,
                pendingWrite.expectedLength
            )
            return HandleResult(response: OfflineNoteNfcApduProtocol.statusWrongData, receiptAckRead: false)
        } catch {
            NSLog(
                "iroha_offline_nfc_ios_card apdu_commit_rejected reason=commit_error error_type=%@ written=%ld expected_length=%ld",
                Self.safeErrorType(error),
                pendingWrite.writtenByteCount,
                pendingWrite.expectedLength
            )
            return HandleResult(response: OfflineNoteNfcApduProtocol.statusWrongData, receiptAckRead: false)
        }
        guard let transportPayload = String(data: payloadBytes, encoding: .utf8) else {
            NSLog("iroha_offline_nfc_ios_card apdu_commit_rejected reason=invalid_utf8 length=%ld", payloadBytes.count)
            return HandleResult(response: OfflineNoteNfcApduProtocol.statusWrongData, receiptAckRead: false)
        }
        let textKind = IrohaOfflineDeviceTransferTextPayload.kind(for: pendingWrite.kind)
        guard let payload = try? IrohaOfflineDeviceTransferTextPayload.normalize(
            transportPayload,
            expectedKind: textKind
        ) else {
            NSLog(
                "iroha_offline_nfc_ios_card apdu_commit_rejected reason=normalize_failed kind=%@ length=%ld",
                String(describing: pendingWrite.kind),
                payloadBytes.count
            )
            return HandleResult(response: OfflineNoteNfcApduProtocol.statusWrongData, receiptAckRead: false)
        }
        self.pendingWrite = nil
        readable = false
        Task { [weak self] in
            await self?.publishReceiptAck(for: payload)
        }
        return HandleResult(response: OfflineNoteNfcApduProtocol.response(), receiptAckRead: false)
    }

    private func publishReceiptAck(for payload: String) async {
        do {
            NSLog(
                "iroha_offline_nfc_ios_card receipt_ack_processing_begin payment_bytes=%ld",
                payload.utf8.count
            )
            let receiptAck = try await onIncomingPaymentToken(payload)
            let normalizedAck = try IrohaOfflineDeviceTransferTextPayload.normalize(
                receiptAck,
                expectedKind: .receiptAck
            )
            try publishPayload(kind: .receiptAck, payload: normalizedAck)
            NSLog(
                "iroha_offline_nfc_ios_card receipt_ack_processing_success ack_bytes=%ld",
                normalizedAck.utf8.count
            )
            onReceiptAckReady?()
        } catch {
            NSLog(
                "iroha_offline_nfc_ios_card receipt_ack_processing_failed error_type=%@",
                Self.safeErrorType(error)
            )
            markPayloadProcessing()
            let exchangeError = Self.exchangeError(forReceiptAckFailure: error)
            onDiagnostic?(exchangeError.receiveDiagnosticReason)
            notifyInvalidated(exchangeError)
            await cardSession?.stopEmulation(status: .failure)
        }
    }

    private static func exchangeError(forReceiptAckFailure error: Error) -> IrohaOfflineNfcExchangeError {
        if let exchangeError = error as? IrohaOfflineNfcExchangeError {
            return exchangeError
        }
        return .peerRejected(statusWord: nil)
    }

    private static func safeErrorType(_ error: Error) -> String {
        if let exchangeError = error as? IrohaOfflineNfcExchangeError {
            return exchangeError.technicalCode
        }
        return String(describing: type(of: error))
    }

    private func publishPayload(kind: OfflineNoteNfcPayloadKind, payload: String) throws {
        let textKind = IrohaOfflineDeviceTransferTextPayload.kind(for: kind)
        let trimmedPayload = try IrohaOfflineDeviceTransferTextPayload.normalize(payload, expectedKind: textKind)
        let payloadBytes = Data(trimmedPayload.utf8)
        let payloadInfo = try OfflineNoteNfcApduProtocol.encodeInfo(kind: kind, payloadBytes: payloadBytes)
        lock.lock()
        currentKind = kind
        currentPayload = trimmedPayload
        currentPayloadBytes = payloadBytes
        currentPayloadInfo = payloadInfo
        readable = true
        lock.unlock()
    }

    private func markPayloadProcessing() {
        lock.lock()
        readable = false
        lock.unlock()
    }
}

public final class IrohaOfflineNfcReaderService: NSObject, @unchecked Sendable, NFCTagReaderSessionDelegate {
    private struct SendableIso7816Tag: @unchecked Sendable {
        let rawValue: NFCISO7816Tag
    }

    private enum Mode {
        case sendPayment((String) async throws -> String)
        case readPaymentToken
    }

    private enum ExchangeResult {
        case sentPayment(IrohaOfflineDeviceTransferSendResult)
        case readPaymentToken(String)
    }

    public static var isReaderAvailable: Bool {
#if targetEnvironment(simulator)
        false
#else
        NFCReaderSession.readingAvailable
#endif
    }

    private let configuration: IrohaOfflineNfcConfiguration
    private var session: NFCTagReaderSession?
    private var continuation: CheckedContinuation<ExchangeResult, Error>?
    private var mode: Mode?
    private var preparedPayment: IrohaOfflineNfcPreparedPayment?
    private var isPreparingPayment = false
    private var invalidatedWhilePreparingPayment = false
    private var preparedPaymentSessionRestartCount = 0
    private var initialExchangeRetryCount = 0
    private var didComplete = false
    private var progressHandler: ((String) -> Void)?
    private var pollingRestartTask: Task<Void, Never>?
    private let readerStateLock = NSLock()
    private var isTagExchangeInFlight = false
    private let passPresentationSuppression: IrohaOfflineWalletPassPresentationSuppression

    public init(configuration: IrohaOfflineNfcConfiguration = IrohaOfflineNfcConfiguration()) {
        self.configuration = configuration
        self.passPresentationSuppression = IrohaOfflineWalletPassPresentationSuppression(
            enabled: configuration.walletPassSuppressionEnabled
        )
        super.init()
    }

    deinit {
        passPresentationSuppression.end()
    }

    public func sendPayment(
        onProgress: ((String) -> Void)? = nil,
        createPaymentToken: @escaping (String) async throws -> String
    ) async throws -> IrohaOfflineDeviceTransferSendResult {
        progressHandler = onProgress
        let result = try await begin(mode: .sendPayment(createPaymentToken), alert: configuration.readerSendAlertMessage)
        guard case .sentPayment(let sendResult) = result else {
            throw IrohaOfflineNfcExchangeError.invalidPayload
        }
        return sendResult
    }

    public func readPaymentToken(onProgress: ((String) -> Void)? = nil) async throws -> String {
        progressHandler = onProgress
        let result = try await begin(mode: .readPaymentToken, alert: configuration.readerReceiveAlertMessage)
        guard case .readPaymentToken(let payload) = result else {
            throw IrohaOfflineNfcExchangeError.invalidPayload
        }
        return payload
    }

    public func tagReaderSessionDidBecomeActive(_ session: NFCTagReaderSession) {
        log("session_active")
        startPollingRestartLoop(for: session)
    }

    public func tagReaderSession(_ session: NFCTagReaderSession, didInvalidateWithError error: Error) {
        guard !didComplete else { return }
        pollingRestartTask?.cancel()
        pollingRestartTask = nil
        self.session = nil
        passPresentationSuppression.end()
        log("session_invalidated error_type=\(Self.safeErrorType(error))")
        setTagExchangeInFlight(false)
        if shouldPreservePreparedPayment(afterSessionInvalidation: error) {
            if preparedPayment == nil {
                invalidatedWhilePreparingPayment = true
                log("session_invalidated_while_preparing")
                return
            }
            if startNewSessionForPreparedPaymentRetap(reason: "session_invalidated") {
                return
            }
        }
        finish(.failure(Self.exchangeError(forSessionInvalidation: error)), invalidatesSession: false)
    }

    public func tagReaderSession(_ session: NFCTagReaderSession, didDetect tags: [NFCTag]) {
        guard beginTagExchange(for: session) else { return }
        log("detected_tags count=\(tags.count)")
        pollingRestartTask?.cancel()
        pollingRestartTask = nil
        guard let tag = tags.first else {
            finish(.failure(IrohaOfflineNfcExchangeError.invalidPeer))
            return
        }
        guard case .iso7816(let isoTag) = tag else {
            log("detected_non_iso7816_tag")
            finish(.failure(IrohaOfflineNfcExchangeError.invalidPeer))
            return
        }
        log("detected_iso7816_tag selected_aid=\(isoTag.initialSelectedAID)")
        let sendableTag = SendableIso7816Tag(rawValue: isoTag)
        session.connect(to: tag) { [weak self] error in
            guard let self else { return }
            if error != nil {
                self.log("connect_failed")
                self.finish(.failure(IrohaOfflineNfcExchangeError.invalidPeer))
                return
            }
            self.log("connect_succeeded")
            Task {
                await self.runExchange(tag: sendableTag)
            }
        }
    }

    private func begin(mode: Mode, alert: String) async throws -> ExchangeResult {
        guard Self.isReaderAvailable else {
            throw IrohaOfflineNfcExchangeError.unavailable
        }
        return try await withCheckedThrowingContinuation { continuation in
            self.passPresentationSuppression.end()
            self.mode = mode
            self.continuation = continuation
            self.preparedPayment = nil
            self.isPreparingPayment = false
            self.invalidatedWhilePreparingPayment = false
            self.preparedPaymentSessionRestartCount = 0
            self.initialExchangeRetryCount = 0
            self.didComplete = false
            self.setTagExchangeInFlight(false)
            guard self.startTagReaderSession(alert: alert) else {
                self.mode = nil
                self.continuation = nil
                continuation.resume(throwing: IrohaOfflineNfcExchangeError.unavailable)
                return
            }
        }
    }

    @discardableResult
    private func startTagReaderSession(alert: String) -> Bool {
        let session: NFCTagReaderSession?
        if #available(iOS 26.4, *) {
            session = NFCTagReaderSession(
                configuration: Self.readerConfiguration(configuration),
                delegate: self,
                queue: nil
            )
            log("start_configured_tag_reader aid=\(configuration.applicationIdentifierHex)")
        } else {
            session = NFCTagReaderSession(pollingOption: [.iso14443], delegate: self, queue: nil)
            log("start_legacy_tag_reader aid=\(configuration.applicationIdentifierHex)")
        }
        guard let session else { return false }
        session.alertMessage = alert
        self.session = session
        passPresentationSuppression.begin()
        session.begin()
        return true
    }

    @available(iOS 26.4, *)
    private static func readerConfiguration(_ configuration: IrohaOfflineNfcConfiguration) -> NFCTagReaderSession.Configuration {
        NFCTagReaderSession.Configuration(
            pollingOption: [.iso14443],
            iso7816SelectIdentifiers: [configuration.applicationIdentifierHex],
            feliCaSystemCodes: []
        )
    }

    private func startPollingRestartLoop(for session: NFCTagReaderSession) {
        pollingRestartTask?.cancel()
        pollingRestartTask = Task { [weak self, weak session] in
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 2_000_000_000)
                guard let self, let session, self.session === session, !self.didComplete else {
                    return
                }
                guard !self.isTagExchangeCurrentlyInFlight() else {
                    return
                }
                self.log("restart_polling")
                session.restartPolling()
            }
        }
    }

    private func runExchange(tag: SendableIso7816Tag) async {
        do {
            log("select_aid aid=\(configuration.applicationIdentifierHex)")
            _ = try await transceive(configuration.selectAidAPDUData(), tag: tag.rawValue)
            log("select_aid_ok")
            let cardPayload = try await readPayload(tag: tag.rawValue)
            log("read_payload kind=\(cardPayload.kind) bytes=\(cardPayload.payload.utf8.count)")
            switch mode {
            case .sendPayment(let createPaymentToken):
                if let preparedPayment {
                    try await finishPreparedPayment(preparedPayment, cardPayload: cardPayload, tag: tag.rawValue)
                    return
                }
                guard cardPayload.kind == .receiveRequest else {
                    throw IrohaOfflineNfcExchangeError.invalidPeer
                }
                log("create_payment_token")
                isPreparingPayment = true
                let paymentToken = try await createPaymentToken(cardPayload.payload)
                guard !didComplete else { return }
                let normalizedPayment = try IrohaOfflineDeviceTransferTextPayload.normalize(
                    paymentToken,
                    expectedKind: .paymentToken
                )
                let preparedPayment = IrohaOfflineNfcPreparedPayment(
                    receiveRequestPayload: cardPayload.payload,
                    paymentToken: normalizedPayment
                )
                self.preparedPayment = preparedPayment
                isPreparingPayment = false
                if invalidatedWhilePreparingPayment || session == nil {
                    invalidatedWhilePreparingPayment = false
                    guard startNewSessionForPreparedPaymentRetap(reason: "payment_prepared_after_invalidation") else {
                        throw IrohaOfflineNfcExchangeError.nfcTimeout
                    }
                    return
                }
                try await finishPreparedPayment(preparedPayment, cardPayload: cardPayload, tag: tag.rawValue)
            case .readPaymentToken:
                guard cardPayload.kind == .paymentToken else {
                    throw IrohaOfflineNfcExchangeError.invalidPeer
                }
                session?.alertMessage = configuration.paymentReceivedAlertMessage
                finish(.success(.readPaymentToken(cardPayload.payload)))
            case .none:
                throw IrohaOfflineNfcExchangeError.cancelled
            }
        } catch {
            guard !didComplete else { return }
            isPreparingPayment = false
            if shouldRetryInitialExchange(after: error),
               restartInitialExchange(after: error) {
                return
            }
            if shouldKeepPreparedPaymentOpen(after: error),
               restartPreparedPaymentTransfer(after: error) {
                log("prepared_payment_retry_waiting error_type=\(Self.safeErrorType(error))")
                return
            }
            finish(.failure(error))
        }
    }

    private func shouldKeepPreparedPaymentOpen(after error: Error) -> Bool {
        guard preparedPayment != nil else { return false }
        guard let exchangeError = error as? IrohaOfflineNfcExchangeError else { return false }
        return exchangeError.shouldRetryPreparedPaymentTransfer
    }

    private func shouldRetryInitialExchange(after error: Error) -> Bool {
        guard preparedPayment == nil, isSendingPayment else { return false }
        guard initialExchangeRetryCount < configuration.maxInitialExchangeRetries else { return false }
        guard let exchangeError = error as? IrohaOfflineNfcExchangeError else { return false }
        switch exchangeError {
        case .peerRejected(statusWord: nil), .nfcTimeout:
            return true
        default:
            return false
        }
    }

    private func shouldPreservePreparedPayment(afterSessionInvalidation error: Error) -> Bool {
        guard isSendingPayment, preparedPayment != nil || isPreparingPayment else { return false }
        return Self.isRetryableSessionInvalidation(error)
    }

    private var isSendingPayment: Bool {
        if case .sendPayment = mode {
            return true
        }
        return false
    }

    private func restartPreparedPaymentTransfer(after error: Error) -> Bool {
        if restartPollingForPreparedPayment() {
            session?.alertMessage = configuration.preparedPaymentRetapAlertMessage
            return true
        }
        return startNewSessionForPreparedPaymentRetap(reason: "transient_error_\(Self.safeErrorType(error))")
    }

    private func restartPollingForPreparedPayment() -> Bool {
        guard let session else { return false }
        setTagExchangeInFlight(false)
        guard #available(iOS 26.4, *) else { return false }
        session.restartPolling(
            configuration: NFCTagReaderSession.Configuration(
                pollingOption: [.iso14443]
            )
        )
        return true
    }

    private func restartInitialExchange(after error: Error) -> Bool {
        guard let session else { return false }
        initialExchangeRetryCount += 1
        setTagExchangeInFlight(false)
        log("initial_exchange_retry reason=\(Self.safeErrorType(error)) count=\(initialExchangeRetryCount)")
        if #available(iOS 26.4, *) {
            session.restartPolling(configuration: Self.readerConfiguration(configuration))
        } else {
            session.restartPolling()
        }
        startPollingRestartLoop(for: session)
        return true
    }

    private func startNewSessionForPreparedPaymentRetap(reason: String) -> Bool {
        guard preparedPayment != nil else { return false }
        guard continuation != nil, !didComplete, session == nil else { return false }
        guard preparedPaymentSessionRestartCount < configuration.maxPreparedPaymentSessionRestarts else {
            log("prepared_payment_retry_limit_reached reason=\(reason)")
            return false
        }
        preparedPaymentSessionRestartCount += 1
        log("prepared_payment_new_session reason=\(reason) count=\(preparedPaymentSessionRestartCount)")
        return startTagReaderSession(alert: configuration.preparedPaymentRetapAlertMessage)
    }

    private func finishPreparedPayment(
        _ preparedPayment: IrohaOfflineNfcPreparedPayment,
        cardPayload: IrohaOfflineNfcCardPayload,
        tag: NFCISO7816Tag
    ) async throws {
        switch try preparedPayment.resumeAction(peerKind: cardPayload.kind, peerPayload: cardPayload.payload) {
        case .deliverPayment:
            log("write_payload kind=paymentToken bytes=\(preparedPayment.paymentToken.utf8.count)")
            try await writePayload(
                kind: .paymentToken,
                payload: preparedPayment.paymentToken,
                tag: tag,
                preferredChunkLength: cardPayload.peerMaxChunkLength
            )
            let receiptAck = try await readReceiptAck(tag: tag)
            log("receipt_ack_received bytes=\(receiptAck.utf8.count)")
            self.preparedPayment = nil
            session?.alertMessage = configuration.paymentConfirmedAlertMessage
            finish(
                .success(
                    .sentPayment(
                        IrohaOfflineDeviceTransferSendResult(
                            paymentToken: preparedPayment.paymentToken,
                            receiptAck: receiptAck
                        )
                    )
                )
            )
        case .acceptReceiptAck(let receiptAck):
            log("receipt_ack_already_available bytes=\(receiptAck.utf8.count)")
            self.preparedPayment = nil
            session?.alertMessage = configuration.paymentConfirmedAlertMessage
            finish(
                .success(
                    .sentPayment(
                        IrohaOfflineDeviceTransferSendResult(
                            paymentToken: preparedPayment.paymentToken,
                            receiptAck: receiptAck
                        )
                    )
                )
            )
        }
    }

    private func readPayload(tag: NFCISO7816Tag) async throws -> IrohaOfflineNfcCardPayload {
        let infoData = try await transceive(OfflineNoteNfcApduProtocol.getInfoAPDUData(), tag: tag)
        guard let info = OfflineNoteNfcApduProtocol.decodeInfo(infoData) else {
            throw IrohaOfflineNfcExchangeError.invalidPeer
        }
        let payload = try await readPayload(info: info, tag: tag)
        return IrohaOfflineNfcCardPayload(
            kind: payload.kind,
            payload: payload.payload,
            peerMaxChunkLength: info.maxChunkLength
        )
    }

    private func readReceiptAck(tag: NFCISO7816Tag) async throws -> String {
        log("receipt_ack_wait_begin timeout_seconds=\(Int(configuration.ackWaitSeconds))")
        let deadline = Date().addingTimeInterval(configuration.ackWaitSeconds)
        while Date() < deadline {
            let response = try await transceiveWithStatus(OfflineNoteNfcApduProtocol.getInfoAPDUData(), tag: tag)
            if response.statusWord == 0x6985 {
                try await Task.sleep(nanoseconds: configuration.ackPollIntervalNanoseconds)
                continue
            }
            guard response.statusWord == 0x9000 else {
                throw IrohaOfflineNfcExchangeError.peerRejected(statusWord: response.statusWord)
            }
            guard let info = OfflineNoteNfcApduProtocol.decodeInfo(response.data) else {
                throw IrohaOfflineNfcExchangeError.invalidPeer
            }
            if info.kind != .receiptAck {
                try await Task.sleep(nanoseconds: configuration.ackPollIntervalNanoseconds)
                continue
            }
            let payload = try await readPayload(info: info, tag: tag)
            guard payload.kind == .receiptAck else {
                throw IrohaOfflineNfcExchangeError.invalidPayload
            }
            return payload.payload
        }
        log("receipt_ack_wait_timeout")
        throw IrohaOfflineNfcExchangeError.ackPending
    }

    private func readPayload(
        info: OfflineNoteNfcPayloadInfo,
        tag: NFCISO7816Tag
    ) async throws -> (kind: OfflineNoteNfcPayloadKind, payload: String) {
        var payloadData = Data()
        payloadData.reserveCapacity(info.payloadLength)
        var offset = 0
        let chunkLength = min(info.maxChunkLength, OfflineNoteNfcApduProtocol.maxExtendedReadChunkBytes)
        while offset < info.payloadLength {
            let chunk = try await transceive(
                try OfflineNoteNfcApduProtocol.readChunkAPDUData(offset: offset, length: chunkLength),
                tag: tag
            )
            guard !chunk.isEmpty, payloadData.count + chunk.count <= info.payloadLength else {
                throw IrohaOfflineNfcExchangeError.invalidPayload
            }
            payloadData.append(chunk)
            offset += chunk.count
        }
        guard OfflineNoteNfcApduProtocol.payloadDigestMatches(payloadData, expectedSha256: info.sha256) else {
            throw IrohaOfflineNfcExchangeError.checksumMismatch
        }
        guard let payloadText = String(data: payloadData, encoding: .utf8) else {
            throw IrohaOfflineNfcExchangeError.invalidPayload
        }
        let textKind = IrohaOfflineDeviceTransferTextPayload.kind(for: info.kind)
        let payload = try IrohaOfflineDeviceTransferTextPayload.normalize(payloadText, expectedKind: textKind)
        return (info.kind, payload)
    }

    private func writePayload(
        kind: OfflineNoteNfcPayloadKind,
        payload: String,
        tag: NFCISO7816Tag,
        preferredChunkLength: Int
    ) async throws {
        let textKind = IrohaOfflineDeviceTransferTextPayload.kind(for: kind)
        let payloadData = Data(try IrohaOfflineDeviceTransferTextPayload.normalize(payload, expectedKind: textKind).utf8)
        let chunkLength = min(
            max(preferredChunkLength, 1),
            OfflineNoteNfcApduProtocol.androidSafeChunkBytes
        )
        try await transceive(
            try OfflineNoteNfcApduProtocol.writeMetaAPDUData(kind: kind, payloadBytes: payloadData),
            tag: tag
        )
        var offset = 0
        while offset < payloadData.count {
            let end = min(offset + chunkLength, payloadData.count)
            try await transceive(
                try OfflineNoteNfcApduProtocol.writeChunkAPDUData(
                    offset: offset,
                    payloadBytes: payloadData,
                    range: offset..<end
                ),
                tag: tag
            )
            offset = end
        }
        try await transceive(OfflineNoteNfcApduProtocol.commitAPDUData(), tag: tag)
    }

    @discardableResult
    private func transceive(_ data: Data, tag: NFCISO7816Tag) async throws -> Data {
        guard let apdu = NFCISO7816APDU(data: data) else {
            throw IrohaOfflineNfcExchangeError.invalidPayload
        }
        return try await withCheckedThrowingContinuation { continuation in
            let lock = NSLock()
            var didResume = false
            func resumeOnce(_ result: Result<Data, Error>) {
                lock.lock()
                if didResume {
                    lock.unlock()
                    return
                }
                didResume = true
                lock.unlock()
                switch result {
                case .success(let data):
                    continuation.resume(returning: data)
                case .failure(let error):
                    continuation.resume(throwing: error)
                }
            }

            DispatchQueue.main.asyncAfter(deadline: .now() + configuration.apduTimeoutSeconds) { [weak self] in
                guard let self else { return }
                lock.lock()
                let shouldTimeOut = !didResume
                lock.unlock()
                guard shouldTimeOut else { return }
                self.log("transceive_timeout length=\(data.count)")
                resumeOnce(.failure(IrohaOfflineNfcExchangeError.peerRejected(statusWord: nil)))
            }

            tag.sendCommand(apdu: apdu) { [weak self] responseData, sw1, sw2, error in
                if error != nil {
                    self?.log("transceive_error length=\(data.count)")
                    resumeOnce(.failure(IrohaOfflineNfcExchangeError.peerRejected(statusWord: nil)))
                    return
                }
                guard sw1 == 0x90, sw2 == 0x00 else {
                    let statusWord = (UInt16(sw1) << 8) | UInt16(sw2)
                    self?.log(String(format: "transceive_status length=%ld status=%04X", data.count, Int(statusWord)))
                    resumeOnce(.failure(IrohaOfflineNfcExchangeError.peerRejected(statusWord: statusWord)))
                    return
                }
                resumeOnce(.success(responseData))
            }
        }
    }

    private func transceiveWithStatus(_ data: Data, tag: NFCISO7816Tag) async throws -> (data: Data, statusWord: UInt16) {
        guard let apdu = NFCISO7816APDU(data: data) else {
            throw IrohaOfflineNfcExchangeError.invalidPayload
        }
        return try await withCheckedThrowingContinuation { continuation in
            let lock = NSLock()
            var didResume = false
            func resumeOnce(_ result: Result<(data: Data, statusWord: UInt16), Error>) {
                lock.lock()
                if didResume {
                    lock.unlock()
                    return
                }
                didResume = true
                lock.unlock()
                switch result {
                case .success(let response):
                    continuation.resume(returning: response)
                case .failure(let error):
                    continuation.resume(throwing: error)
                }
            }

            DispatchQueue.main.asyncAfter(deadline: .now() + configuration.apduTimeoutSeconds) { [weak self] in
                guard let self else { return }
                lock.lock()
                let shouldTimeOut = !didResume
                lock.unlock()
                guard shouldTimeOut else { return }
                self.log("transceive_timeout length=\(data.count)")
                resumeOnce(.failure(IrohaOfflineNfcExchangeError.peerRejected(statusWord: nil)))
            }

            tag.sendCommand(apdu: apdu) { [weak self] responseData, sw1, sw2, error in
                if error != nil {
                    self?.log("transceive_error length=\(data.count)")
                    resumeOnce(.failure(IrohaOfflineNfcExchangeError.peerRejected(statusWord: nil)))
                    return
                }
                let statusWord = (UInt16(sw1) << 8) | UInt16(sw2)
                resumeOnce(.success((responseData, statusWord)))
            }
        }
    }

    private func finish(_ result: Result<ExchangeResult, Error>, invalidatesSession: Bool = true) {
        guard !didComplete else { return }
        didComplete = true
        let continuation = continuation
        self.continuation = nil
        self.mode = nil
        self.preparedPayment = nil
        self.progressHandler = nil
        pollingRestartTask?.cancel()
        pollingRestartTask = nil
        setTagExchangeInFlight(false)
        if invalidatesSession {
            session?.invalidate()
        }
        passPresentationSuppression.end()
        session = nil
        switch result {
        case .success(let exchangeResult):
            continuation?.resume(returning: exchangeResult)
        case .failure(let error):
            continuation?.resume(throwing: error)
        }
    }

    private func log(_ message: String) {
        NSLog("iroha_offline_nfc_ios_reader %@", message)
        if let progressHandler {
            DispatchQueue.main.async {
                progressHandler(message)
            }
        }
    }

    private func beginTagExchange(for session: NFCTagReaderSession) -> Bool {
        readerStateLock.lock()
        defer { readerStateLock.unlock() }
        guard self.session === session, !didComplete else { return false }
        isTagExchangeInFlight = true
        return true
    }

    private func setTagExchangeInFlight(_ value: Bool) {
        readerStateLock.lock()
        isTagExchangeInFlight = value
        readerStateLock.unlock()
    }

    private func isTagExchangeCurrentlyInFlight() -> Bool {
        readerStateLock.lock()
        let value = isTagExchangeInFlight
        readerStateLock.unlock()
        return value
    }

    private static func safeErrorType(_ error: Error) -> String {
        if let exchangeError = error as? IrohaOfflineNfcExchangeError {
            return exchangeError.technicalCode
        }
        let nsError = error as NSError
        if nsError.domain == NFCErrorDomain {
            return "NFCError.\(nsError.code)"
        }
        return String(describing: type(of: error))
    }

    private static func isRetryableSessionInvalidation(_ error: Error) -> Bool {
        let nsError = error as NSError
        guard nsError.domain == NFCErrorDomain else { return false }
        switch nsError.code {
        case 201, 202, 203:
            return true
        default:
            return false
        }
    }

    private static func exchangeError(forSessionInvalidation error: Error) -> IrohaOfflineNfcExchangeError {
        let nsError = error as NSError
        guard nsError.domain == NFCErrorDomain else { return .cancelled }
        switch nsError.code {
        case 201:
            return .nfcTimeout
        default:
            return .cancelled
        }
    }
}
#else
public final class IrohaOfflineNfcCardSessionController {
    private let configuration: IrohaOfflineNfcConfiguration

    public init(configuration: IrohaOfflineNfcConfiguration = IrohaOfflineNfcConfiguration()) {
        self.configuration = configuration
    }

    public static func isCardEmulationAvailable(
        configuration: IrohaOfflineNfcConfiguration = IrohaOfflineNfcConfiguration()
    ) -> Bool {
        _ = configuration
        return false
    }

    public static func cardSessionAvailability(
        configuration: IrohaOfflineNfcConfiguration = IrohaOfflineNfcConfiguration()
    ) async -> IrohaOfflineNfcCardSessionAvailability {
        _ = configuration
        return .unavailable(.unavailable)
    }

    public func startReceiveRequest(
        _ payload: String,
        onEmulationStarted: (() -> Void)? = nil,
        onReceiptAckReady: (() -> Void)? = nil,
        onReceiptAckRead: (() -> Void)? = nil,
        onSessionInvalidated: ((IrohaOfflineNfcExchangeError) -> Void)? = nil,
        onDiagnostic: ((IrohaOfflineReceiveDiagnosticReason) -> Void)? = nil,
        onIncomingPaymentToken: @escaping (String) async throws -> String
    ) async throws {
        _ = configuration
        _ = payload
        _ = onEmulationStarted
        _ = onReceiptAckReady
        _ = onReceiptAckRead
        _ = onSessionInvalidated
        _ = onDiagnostic
        _ = onIncomingPaymentToken
        throw IrohaOfflineNfcExchangeError.cardEmulationUnavailable
    }

    public func stop() {}
}

public final class IrohaOfflineNfcReaderService {
    public static var isReaderAvailable: Bool { false }

    private let configuration: IrohaOfflineNfcConfiguration

    public init(configuration: IrohaOfflineNfcConfiguration = IrohaOfflineNfcConfiguration()) {
        self.configuration = configuration
    }

    public func sendPayment(
        onProgress: ((String) -> Void)? = nil,
        createPaymentToken: @escaping (String) async throws -> String
    ) async throws -> IrohaOfflineDeviceTransferSendResult {
        _ = configuration
        _ = onProgress
        _ = createPaymentToken
        throw IrohaOfflineNfcExchangeError.unavailable
    }

    public func readPaymentToken(onProgress: ((String) -> Void)? = nil) async throws -> String {
        _ = configuration
        _ = onProgress
        throw IrohaOfflineNfcExchangeError.unavailable
    }
}
#endif
