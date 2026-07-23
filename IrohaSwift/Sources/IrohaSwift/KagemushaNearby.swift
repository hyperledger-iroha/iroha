import Foundation

#if canImport(Network)
@preconcurrency import Network
#endif

#if canImport(MultipeerConnectivity)
@preconcurrency import MultipeerConnectivity
#endif

public enum KagemushaNearbyPairingSymbol: String, CaseIterable, Codable, Sendable {
    case stars = "nearby_pairing_stars"
    case bird = "nearby_pairing_bird"
    case mask = "nearby_pairing_mask"
}

public struct KagemushaNearbyPairingChallenge: Equatable, Hashable, Codable, Sendable {
    public let symbol: KagemushaNearbyPairingSymbol

    public init(symbol: KagemushaNearbyPairingSymbol) {
        self.symbol = symbol
    }

    public static func random() -> Self {
        Self(symbol: KagemushaNearbyPairingSymbol.allCases.randomElement() ?? .stars)
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        symbol = try container.decode(KagemushaNearbyPairingSymbol.self)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(symbol)
    }
}

public enum KagemushaNearbyPairingDecision: Equatable, Sendable {
    case accepted
    case mismatch
    case cancelled
}

public enum KagemushaNearbyEvent: Equatable, Sendable {
    case requestingLocalNetworkPermission
    case browsing
    case advertising(KagemushaNearbyPairingChallenge)
    case peerConnected
    case pairingChallenge(KagemushaNearbyPairingChallenge)
    case receiveRequest(KagemushaRecipientReceiveOfferV2)
    case paymentQueued(KagemushaRecursiveSpendPeerPaymentV4)
    case paymentReceived(KagemushaRecursiveSpendPeerPaymentV4)
    case acknowledgementQueued(KagemushaReceiverAcknowledgement)
    case acknowledgementReceived(KagemushaReceiverAcknowledgement)
}

public enum KagemushaNearbyError: Error, Equatable, LocalizedError, Sendable {
    case unavailable
    case busy
    case timedOut
    case connectionFailed
    case invalidMessage
    case peerRejected
    case cancelled
    case pairingMismatch
    case localNetworkPermissionDenied

    public var errorDescription: String? {
        switch self {
        case .unavailable:
            return "Nearby Kagemusha transfer is unavailable on this device."
        case .busy:
            return "A Nearby Kagemusha transfer is already in progress."
        case .timedOut:
            return "The Nearby Kagemusha transfer timed out."
        case .connectionFailed:
            return "The Nearby Kagemusha peer disconnected or could not connect."
        case .invalidMessage:
            return "The Nearby peer sent an invalid Kagemusha message."
        case .peerRejected:
            return "The Nearby peer rejected the Kagemusha transfer."
        case .cancelled:
            return "The Nearby Kagemusha transfer was cancelled."
        case .pairingMismatch:
            return "The Nearby pairing symbols did not match."
        case .localNetworkPermissionDenied:
            return "Local Network permission is required for Nearby transfers."
        }
    }

    static func normalized(_ error: Error) -> KagemushaNearbyError {
        if let error = error as? KagemushaNearbyError { return error }
        if error is CancellationError { return .cancelled }
        let nsError = error as NSError
        if nsError.domain == NetService.errorDomain, nsError.code == -72008 {
            return .localNetworkPermissionDenied
        }
        var descriptions = [
            nsError.localizedDescription,
            nsError.localizedFailureReason,
            nsError.localizedRecoverySuggestion,
        ].compactMap { $0?.lowercased() }.joined(separator: " ")
        var underlying = nsError.userInfo[NSUnderlyingErrorKey] as? NSError
        for _ in 0..<3 where underlying != nil {
            descriptions += " " + (underlying?.localizedDescription.lowercased() ?? "")
            underlying = underlying?.userInfo[NSUnderlyingErrorKey] as? NSError
        }
        if descriptions.contains("local network"),
           descriptions.contains("denied") || descriptions.contains("permission")
            || descriptions.contains("privacy") {
            return .localNetworkPermissionDenied
        }
        return .connectionFailed
    }
}

enum KagemushaNearbyMessageKind: String, Codable, Sendable {
    case receiveRequest = "receive_request"
    case payment
    case acknowledgement
    case rejected
}

enum KagemushaNearbyEnvelopeCodec {
    /// Canonical authenticated Nearby plaintext. The exact IPM1 message is
    /// carried once (without a text/base64 expansion), leaving ample room for
    /// the 24,576-byte Kagemusha body below the 32 KiB Nearby record ceiling.
    static let maximumEnvelopeBytes = 32_704
    private static let magic = Data("PKNB1".utf8)
    private static let headerBytes = 12

    struct Decoded: Equatable, Sendable {
        let messageKind: KagemushaNearbyMessageKind
        let payload: KagemushaPeerPayload?
        let pairingChallenge: KagemushaNearbyPairingChallenge?
    }

    static func encode(
        _ payload: KagemushaPeerPayload,
        pairingChallenge: KagemushaNearbyPairingChallenge? = nil
    ) throws -> Data {
        switch payload.kind {
        case .receiveRequest:
            guard pairingChallenge != nil else { throw KagemushaNearbyError.invalidMessage }
        case .payment, .acknowledgement:
            guard pairingChallenge == nil else { throw KagemushaNearbyError.invalidMessage }
        }
        let message: IrohaPeerWireMessageV1
        do {
            message = try IrohaPeerKagemushaAdapterV1.wrap(payload)
        } catch {
            throw KagemushaNearbyError.invalidMessage
        }
        return try encode(
            kind: messageKind(for: payload.kind),
            pairingChallenge: pairingChallenge,
            ipmMessage: message.encoded
        )
    }

    static func encodeRejection() throws -> Data {
        try encode(kind: .rejected, pairingChallenge: nil, ipmMessage: Data())
    }

    static func decode(_ data: Data) throws -> Decoded {
        guard data.count >= headerBytes,
              data.count <= maximumEnvelopeBytes,
              data.prefix(magic.count) == magic,
              data[7] == 0,
              data[6] <= 3,
              let decodedKind = messageKind(code: data[5]) else {
            throw KagemushaNearbyError.invalidMessage
        }
        let pairingChallenge = pairingChallenge(code: data[6])
        let payloadLength = Int(
            (UInt32(data[8]) << 24)
                | (UInt32(data[9]) << 16)
                | (UInt32(data[10]) << 8)
                | UInt32(data[11])
        )
        guard payloadLength == data.count - headerBytes else {
            throw KagemushaNearbyError.invalidMessage
        }
        if decodedKind == .rejected {
            guard payloadLength == 0, pairingChallenge == nil else {
                throw KagemushaNearbyError.invalidMessage
            }
            return Decoded(messageKind: .rejected, payload: nil, pairingChallenge: nil)
        }
        guard payloadLength > 0 else {
            throw KagemushaNearbyError.invalidMessage
        }
        let messageBytes = data.subdata(in: headerBytes..<data.count)
        let payload: KagemushaPeerPayload
        do {
            let message = try IrohaPeerWireMessageV1.decode(
                messageBytes,
                expectedProfile: .kagemusha
            )
            payload = try IrohaPeerKagemushaAdapterV1.decode(message)
        } catch {
            throw KagemushaNearbyError.invalidMessage
        }
        guard messageKind(for: payload.kind) == decodedKind else {
            throw KagemushaNearbyError.invalidMessage
        }
        switch payload.kind {
        case .receiveRequest:
            guard pairingChallenge != nil else {
                throw KagemushaNearbyError.invalidMessage
            }
        case .payment, .acknowledgement:
            guard pairingChallenge == nil else {
                throw KagemushaNearbyError.invalidMessage
            }
        }
        return Decoded(
            messageKind: decodedKind,
            payload: payload,
            pairingChallenge: pairingChallenge
        )
    }

    private static func encode(
        kind: KagemushaNearbyMessageKind,
        pairingChallenge: KagemushaNearbyPairingChallenge?,
        ipmMessage: Data
    ) throws -> Data {
        guard ipmMessage.count <= Int(UInt32.max),
              headerBytes + ipmMessage.count <= maximumEnvelopeBytes,
              let kindCode = messageKindCode(kind) else {
            throw KagemushaNearbyError.invalidMessage
        }
        let pairingCode = pairingChallengeCode(pairingChallenge)
        let length = UInt32(ipmMessage.count)
        var data = Data(capacity: headerBytes + ipmMessage.count)
        data.append(magic)
        data.append(kindCode)
        data.append(pairingCode)
        data.append(0)
        data.append(UInt8((length >> 24) & 0xff))
        data.append(UInt8((length >> 16) & 0xff))
        data.append(UInt8((length >> 8) & 0xff))
        data.append(UInt8(length & 0xff))
        data.append(ipmMessage)
        return data
    }

    private static func messageKindCode(
        _ kind: KagemushaNearbyMessageKind
    ) -> UInt8? {
        switch kind {
        case .receiveRequest: return 1
        case .payment: return 2
        case .acknowledgement: return 3
        case .rejected: return 4
        }
    }

    private static func messageKind(code: UInt8) -> KagemushaNearbyMessageKind? {
        switch code {
        case 1: return .receiveRequest
        case 2: return .payment
        case 3: return .acknowledgement
        case 4: return .rejected
        default: return nil
        }
    }

    private static func pairingChallengeCode(
        _ challenge: KagemushaNearbyPairingChallenge?
    ) -> UInt8 {
        guard let challenge else { return 0 }
        switch challenge.symbol {
        case .stars: return 1
        case .bird: return 2
        case .mask: return 3
        }
    }

    private static func pairingChallenge(
        code: UInt8
    ) -> KagemushaNearbyPairingChallenge? {
        switch code {
        case 0: return nil
        case 1: return .init(symbol: .stars)
        case 2: return .init(symbol: .bird)
        case 3: return .init(symbol: .mask)
        default: return nil
        }
    }

    private static func messageKind(
        for kind: KagemushaPeerPayloadKind
    ) -> KagemushaNearbyMessageKind {
        switch kind {
        case .receiveRequest: return .receiveRequest
        case .payment: return .payment
        case .acknowledgement: return .acknowledgement
        }
    }
}

public enum KagemushaNearbyTransportPolicy {
    public static let serviceName = KagemushaPeerTransportContract.nearbyServiceName
    public static let bonjourService = KagemushaPeerTransportContract.nearbyBonjourService
    public static let discoveryInfo = ["protocol": "kagemusha-v2"]
    public static let acknowledgementDisconnectGraceNanoseconds: UInt64 = 1_500_000_000

    public static func peerDisplayName(uuid: UUID = UUID()) -> String {
        "pk-\(uuid.uuidString.prefix(8).lowercased())"
    }

    static func timeoutNanoseconds(seconds: UInt64) -> UInt64 {
        let (nanoseconds, overflow) = seconds.multipliedReportingOverflow(by: 1_000_000_000)
        return overflow ? UInt64.max : nanoseconds
    }
}

/// Release gate for the live Nearby transport.
///
/// MultipeerConnectivity's transport encryption does not authenticate the
/// Kagemusha counterparty.  The three-symbol visual challenge is relayable and
/// therefore must not be treated as peer authentication.  This gate stays
/// closed until the SDK has an audited, transcript-bound ephemeral ECDH flow
/// authenticated by device certificates.  Enabling a build condition alone is
/// deliberately insufficient: an audit must also change this source constant.
public enum KagemushaNearbyAuthenticationPolicy {
    public static let requiresCertificateAuthenticatedECDHTranscript = true
    public static let hasAuditedAuthenticatedTranscriptBackend = false
}

#if KAGEMUSHA_NEARBY_AUDITED_AUTHENTICATED_TRANSCRIPT && canImport(MultipeerConnectivity)
#if canImport(Network)
private final class KagemushaNearbyNetworkPreflight: @unchecked Sendable {
    private let queue = DispatchQueue(label: "org.hyperledger.iroha.kagemusha-nearby-preflight")
    private let lock = NSLock()
    private var browser: NWBrowser?
    private var continuation: CheckedContinuation<Void, Error>?
    private var completed = false

    func run() async throws {
        try Task.checkCancellation()
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { continuation in
                lock.lock()
                self.continuation = continuation
                lock.unlock()
                let parameters = NWParameters.tcp
                parameters.includePeerToPeer = true
                let browser = NWBrowser(
                    for: .bonjour(
                        type: KagemushaNearbyTransportPolicy.bonjourService,
                        domain: nil
                    ),
                    using: parameters
                )
                browser.stateUpdateHandler = { [weak self] state in
                    switch state {
                    case .ready:
                        self?.finish(.success(()))
                    case .waiting(let error), .failed(let error):
                        self?.finish(.failure(KagemushaNearbyError.normalized(error)))
                    case .cancelled:
                        self?.finish(.failure(KagemushaNearbyError.cancelled))
                    default:
                        break
                    }
                }
                lock.lock()
                self.browser = browser
                lock.unlock()
                browser.start(queue: queue)
                queue.asyncAfter(deadline: .now() + 12) { [weak self] in
                    self?.finish(.failure(KagemushaNearbyError.localNetworkPermissionDenied))
                }
            }
        } onCancel: {
            finish(.failure(KagemushaNearbyError.cancelled))
        }
    }

    func cancel() { finish(.failure(KagemushaNearbyError.cancelled)) }

    private func finish(_ result: Result<Void, Error>) {
        lock.lock()
        guard !completed else { lock.unlock(); return }
        completed = true
        let continuation = self.continuation
        let browser = self.browser
        self.continuation = nil
        self.browser = nil
        lock.unlock()
        browser?.stateUpdateHandler = nil
        browser?.cancel()
        switch result {
        case .success: continuation?.resume()
        case .failure(let error): continuation?.resume(throwing: error)
        }
    }
}
#endif

public final class KagemushaNearbyExchange: @unchecked Sendable {
    public static var isAvailable: Bool {
        KagemushaNearbyAuthenticationPolicy.hasAuditedAuthenticatedTranscriptBackend
    }

    private enum Mode {
        case sender(
            confirmPairing: @Sendable (KagemushaNearbyPairingChallenge) async
                -> KagemushaNearbyPairingDecision,
            createPayment: @Sendable (KagemushaRecipientReceiveOfferV2) async throws
                -> KagemushaRecursiveSpendPeerPaymentV4
        )
        case receiver(
            request: KagemushaRecipientReceiveOfferV2,
            challenge: KagemushaNearbyPairingChallenge,
            acceptPayment: @Sendable (KagemushaRecursiveSpendPeerPaymentV4) async throws
                -> KagemushaReceiverAcknowledgement
        )
    }

    private enum ExchangeResult {
        case sent(KagemushaPeerSendResult)
        case received(KagemushaRecursiveSpendPeerPaymentV4)
    }

    private let lock = NSLock()
    private let timeoutSeconds: UInt64
    private var peerID: MCPeerID?
    private var session: MCSession?
    private var advertiser: MCNearbyServiceAdvertiser?
    private var browser: MCNearbyServiceBrowser?
    private var mode: Mode?
    private var continuation: CheckedContinuation<ExchangeResult, Error>?
    private var onEvent: (@Sendable (KagemushaNearbyEvent) -> Void)?
    private var timeoutTask: Task<Void, Never>?
    private var messageTask: Task<Void, Never>?
#if canImport(Network)
    private var preflight: KagemushaNearbyNetworkPreflight?
#endif
    private var connectedPeer: MCPeerID?
    private var invitedPeers = Set<String>()
    private var selectedPeerKey: String?
    private var starting = false
    private var cancelled = false
    private var finished = false
    private var payment: KagemushaRecursiveSpendPeerPaymentV4?
    private lazy var platformDelegate = KagemushaNearbyPlatformDelegate(owner: self)

    public init(timeoutSeconds: UInt64 = 90) {
        self.timeoutSeconds = max(1, timeoutSeconds)
    }

    public func requestLocalNetworkAccess() async throws {
        guard Self.isAvailable else { throw KagemushaNearbyError.unavailable }
#if canImport(Network)
        let preflight = KagemushaNearbyNetworkPreflight()
        let event: (@Sendable (KagemushaNearbyEvent) -> Void)? = try withStateLock {
            guard !cancelled else { throw KagemushaNearbyError.cancelled }
            self.preflight = preflight
            return onEvent
        }
        event?(.requestingLocalNetworkPermission)
        defer {
            withStateLock {
                if self.preflight === preflight { self.preflight = nil }
            }
        }
        try await preflight.run()
#endif
    }

    public func sendPayment(
        onEvent: @escaping @Sendable (KagemushaNearbyEvent) -> Void = { _ in },
        confirmPairing: @escaping @Sendable (
            KagemushaNearbyPairingChallenge
        ) async -> KagemushaNearbyPairingDecision,
        createPayment: @escaping @Sendable (
            KagemushaRecipientReceiveOfferV2
        ) async throws -> KagemushaRecursiveSpendPeerPaymentV4
    ) async throws -> KagemushaPeerSendResult {
        guard Self.isAvailable else { throw KagemushaNearbyError.unavailable }
        let result = try await begin(
            mode: .sender(confirmPairing: confirmPairing, createPayment: createPayment),
            onEvent: onEvent
        )
        guard case .sent(let value) = result else {
            throw KagemushaNearbyError.invalidMessage
        }
        return value
    }

    public func receivePayment(
        receiveRequest: KagemushaRecipientReceiveOfferV2,
        pairingChallenge: KagemushaNearbyPairingChallenge = .random(),
        onEvent: @escaping @Sendable (KagemushaNearbyEvent) -> Void = { _ in },
        acceptPayment: @escaping @Sendable (
            KagemushaRecursiveSpendPeerPaymentV4
        ) async throws -> KagemushaReceiverAcknowledgement
    ) async throws -> KagemushaRecursiveSpendPeerPaymentV4 {
        guard Self.isAvailable else { throw KagemushaNearbyError.unavailable }
        let result = try await begin(
            mode: .receiver(
                request: receiveRequest,
                challenge: pairingChallenge,
                acceptPayment: acceptPayment
            ),
            onEvent: onEvent
        )
        guard case .received(let value) = result else {
            throw KagemushaNearbyError.invalidMessage
        }
        return value
    }

    public func cancel() {
        lock.lock()
        cancelled = true
        let hasExchange = continuation != nil
        let messageTask = self.messageTask
#if canImport(Network)
        let preflight = self.preflight
#endif
        lock.unlock()
        messageTask?.cancel()
#if canImport(Network)
        preflight?.cancel()
#endif
        if hasExchange { finish(.failure(KagemushaNearbyError.cancelled)) }
    }

    private func begin(
        mode: Mode,
        onEvent: @escaping @Sendable (KagemushaNearbyEvent) -> Void
    ) async throws -> ExchangeResult {
        guard Self.isAvailable else { throw KagemushaNearbyError.unavailable }
        try withStateLock {
            guard !starting, continuation == nil else {
                throw KagemushaNearbyError.busy
            }
            starting = true
            cancelled = false
            finished = false
            self.onEvent = onEvent
        }

        return try await withTaskCancellationHandler {
            do {
                try await requestLocalNetworkAccess()
                try Task.checkCancellation()
                return try await start(mode: mode)
            } catch {
                withStateLock {
                    starting = false
                    self.onEvent = nil
                }
                throw KagemushaNearbyError.normalized(error)
            }
        } onCancel: {
            cancel()
        }
    }

    private func start(mode: Mode) async throws -> ExchangeResult {
        try await withCheckedThrowingContinuation { continuation in
            lock.lock()
            guard !cancelled else {
                starting = false
                lock.unlock()
                continuation.resume(throwing: KagemushaNearbyError.cancelled)
                return
            }
            let peerID = MCPeerID(
                displayName: KagemushaNearbyTransportPolicy.peerDisplayName()
            )
            let session = MCSession(
                peer: peerID,
                securityIdentity: nil,
                encryptionPreference: .required
            )
            session.delegate = platformDelegate
            self.peerID = peerID
            self.session = session
            self.mode = mode
            self.continuation = continuation
            connectedPeer = nil
            invitedPeers.removeAll()
            selectedPeerKey = nil
            payment = nil
            starting = false
            finished = false
            lock.unlock()

            scheduleTimeout()
            switch mode {
            case .sender:
                let browser = MCNearbyServiceBrowser(
                    peer: peerID,
                    serviceType: KagemushaNearbyTransportPolicy.serviceName
                )
                browser.delegate = platformDelegate
                lock.lock(); self.browser = browser; lock.unlock()
                onEvent?(.browsing)
                browser.startBrowsingForPeers()
            case .receiver(_, let challenge, _):
                let advertiser = MCNearbyServiceAdvertiser(
                    peer: peerID,
                    discoveryInfo: KagemushaNearbyTransportPolicy.discoveryInfo,
                    serviceType: KagemushaNearbyTransportPolicy.serviceName
                )
                advertiser.delegate = platformDelegate
                lock.lock(); self.advertiser = advertiser; lock.unlock()
                onEvent?(.advertising(challenge))
                advertiser.startAdvertisingPeer()
            }
        }
    }

    private func scheduleTimeout() {
        timeoutTask = Task { [weak self, timeoutSeconds] in
            try? await Task.sleep(
                nanoseconds: KagemushaNearbyTransportPolicy.timeoutNanoseconds(
                    seconds: timeoutSeconds
                )
            )
            guard !Task.isCancelled else { return }
            self?.finish(.failure(KagemushaNearbyError.timedOut))
        }
    }

    private func send(
        _ payload: KagemushaPeerPayload,
        challenge: KagemushaNearbyPairingChallenge? = nil,
        to peer: MCPeerID
    ) throws {
        let data = try KagemushaNearbyEnvelopeCodec.encode(
            payload,
            pairingChallenge: challenge
        )
        lock.lock(); let session = self.session; lock.unlock()
        guard let session else { throw KagemushaNearbyError.connectionFailed }
        try session.send(data, toPeers: [peer], with: .reliable)
    }

    private func handle(
        _ envelope: KagemushaNearbyEnvelopeCodec.Decoded,
        from peer: MCPeerID
    ) {
        lock.lock(); let mode = self.mode; lock.unlock()
        switch (mode, envelope.messageKind, envelope.payload) {
        case let (.sender(confirmPairing, createPayment), .receiveRequest,
                  .some(.receiveRequest(request))):
            guard let challenge = envelope.pairingChallenge else {
                finish(.failure(KagemushaNearbyError.invalidMessage)); return
            }
            let task = Task { [weak self] in
                guard let self else { return }
                do {
                    self.onEvent?(.pairingChallenge(challenge))
                    switch await confirmPairing(challenge) {
                    case .accepted: break
                    case .mismatch: throw KagemushaNearbyError.pairingMismatch
                    case .cancelled: throw KagemushaNearbyError.cancelled
                    }
                    try Task.checkCancellation()
                    self.onEvent?(.receiveRequest(request))
                    let payment = try await createPayment(request)
                    try Task.checkCancellation()
                    self.withStateLock { self.payment = payment }
                    try self.send(.payment(payment), to: peer)
                    self.onEvent?(.paymentQueued(payment))
                } catch {
                    self.sendRejection(to: peer)
                    self.finish(.failure(KagemushaNearbyError.normalized(error)))
                }
            }
            remember(task)
        case let (.sender, .acknowledgement, .some(.acknowledgement(acknowledgement))):
            lock.lock(); let payment = self.payment; lock.unlock()
            guard let payment else {
                finish(.failure(KagemushaNearbyError.invalidMessage)); return
            }
            onEvent?(.acknowledgementReceived(acknowledgement))
            finish(.success(.sent(.init(
                payment: payment,
                acknowledgement: acknowledgement
            ))))
        case let (.receiver(_, _, acceptPayment), .payment, .some(.payment(payment))):
            let task = Task { [weak self] in
                guard let self else { return }
                do {
                    self.onEvent?(.paymentReceived(payment))
                    let acknowledgement = try await acceptPayment(payment)
                    try Task.checkCancellation()
                    try self.send(.acknowledgement(acknowledgement), to: peer)
                    self.onEvent?(.acknowledgementQueued(acknowledgement))
                    self.finish(.success(.received(payment)))
                } catch {
                    self.sendRejection(to: peer)
                    self.finish(.failure(KagemushaNearbyError.normalized(error)))
                }
            }
            remember(task)
        case (_, .rejected, nil):
            finish(.failure(KagemushaNearbyError.peerRejected))
        default:
            finish(.failure(KagemushaNearbyError.invalidMessage))
        }
    }

    private func sendRequestIfReceiver(to peer: MCPeerID) {
        lock.lock(); let mode = self.mode; lock.unlock()
        guard case .receiver(let request, let challenge, _) = mode else { return }
        do { try send(.receiveRequest(request), challenge: challenge, to: peer) }
        catch { finish(.failure(KagemushaNearbyError.normalized(error))) }
    }

    private func sendRejection(to peer: MCPeerID) {
        guard let data = try? KagemushaNearbyEnvelopeCodec.encodeRejection() else { return }
        lock.lock(); let session = self.session; lock.unlock()
        try? session?.send(data, toPeers: [peer], with: .reliable)
    }

    private func remember(_ task: Task<Void, Never>) {
        lock.lock()
        if finished { lock.unlock(); task.cancel(); return }
        messageTask = task
        lock.unlock()
    }

    private func finish(_ result: Result<ExchangeResult, Error>) {
        lock.lock()
        guard !finished else { lock.unlock(); return }
        finished = true
        let continuation = self.continuation
        let advertiser = self.advertiser
        let browser = self.browser
        let session = self.session
        let timeoutTask = self.timeoutTask
        let messageTask = self.messageTask
        let delayDisconnect: Bool
        if case .success(.received) = result { delayDisconnect = true }
        else { delayDisconnect = false }
        self.continuation = nil
        self.advertiser = nil
        self.browser = nil
        self.session = nil
        peerID = nil
        mode = nil
        onEvent = nil
        self.timeoutTask = nil
        self.messageTask = nil
        connectedPeer = nil
        invitedPeers.removeAll()
        selectedPeerKey = nil
        starting = false
        cancelled = false
        payment = nil
        lock.unlock()

        timeoutTask?.cancel()
        messageTask?.cancel()
        advertiser?.stopAdvertisingPeer()
        browser?.stopBrowsingForPeers()
        if delayDisconnect, let session {
            session.delegate = nil
            Task {
                try? await Task.sleep(
                    nanoseconds: KagemushaNearbyTransportPolicy
                        .acknowledgementDisconnectGraceNanoseconds
                )
                session.disconnect()
            }
        } else {
            session?.disconnect()
        }
        switch result {
        case .success(let value): continuation?.resume(returning: value)
        case .failure(let error): continuation?.resume(throwing: error)
        }
    }

    private func withStateLock<T>(_ body: () throws -> T) rethrows -> T {
        lock.lock()
        defer { lock.unlock() }
        return try body()
    }
}

extension KagemushaNearbyExchange {
    fileprivate func advertiser(
        _ advertiser: MCNearbyServiceAdvertiser,
        didReceiveInvitationFromPeer peerID: MCPeerID,
        withContext context: Data?,
        invitationHandler: @escaping (Bool, MCSession?) -> Void
    ) {
        _ = advertiser; _ = context
        lock.lock()
        let key = peerKey(peerID)
        let accept = !finished && connectedPeer == nil && selectedPeerKey == nil
        if accept { selectedPeerKey = key }
        let session = self.session
        lock.unlock()
        invitationHandler(accept, accept ? session : nil)
    }

    fileprivate func advertiser(
        _ advertiser: MCNearbyServiceAdvertiser,
        didNotStartAdvertisingPeer error: Error
    ) {
        _ = advertiser
        finish(.failure(KagemushaNearbyError.normalized(error)))
    }
}

extension KagemushaNearbyExchange {
    fileprivate func browser(
        _ browser: MCNearbyServiceBrowser,
        foundPeer peerID: MCPeerID,
        withDiscoveryInfo info: [String: String]?
    ) {
        guard info == KagemushaNearbyTransportPolicy.discoveryInfo else { return }
        let key = peerKey(peerID)
        lock.lock()
        let invite = !finished && connectedPeer == nil && selectedPeerKey == nil
            && !invitedPeers.contains(key)
        if invite {
            invitedPeers.insert(key)
            selectedPeerKey = key
        }
        let session = self.session
        lock.unlock()
        guard invite, let session else { return }
        browser.invitePeer(peerID, to: session, withContext: nil, timeout: 20)
    }

    fileprivate func browser(_ browser: MCNearbyServiceBrowser, lostPeer peerID: MCPeerID) {
        _ = browser
        let key = peerKey(peerID)
        lock.lock()
        if connectedPeer == nil, selectedPeerKey == key { selectedPeerKey = nil }
        lock.unlock()
    }

    fileprivate func browser(
        _ browser: MCNearbyServiceBrowser,
        didNotStartBrowsingForPeers error: Error
    ) {
        _ = browser
        finish(.failure(KagemushaNearbyError.normalized(error)))
    }
}

extension KagemushaNearbyExchange {
    fileprivate func session(
        _ session: MCSession,
        peer peerID: MCPeerID,
        didChange state: MCSessionState
    ) {
        switch state {
        case .connected:
            lock.lock()
            if connectedPeer == nil { connectedPeer = peerID }
            let browser = self.browser
            let advertiser = self.advertiser
            lock.unlock()
            browser?.stopBrowsingForPeers()
            advertiser?.stopAdvertisingPeer()
            onEvent?(.peerConnected)
            sendRequestIfReceiver(to: peerID)
        case .notConnected:
            lock.lock()
            let fail = !finished && connectedPeer?.displayName == peerID.displayName
            if connectedPeer == nil, selectedPeerKey == peerKey(peerID) {
                selectedPeerKey = nil
            }
            lock.unlock()
            if fail { finish(.failure(KagemushaNearbyError.connectionFailed)) }
        case .connecting:
            break
        @unknown default:
            finish(.failure(KagemushaNearbyError.connectionFailed))
        }
    }

    fileprivate func session(_ session: MCSession, didReceive data: Data, fromPeer peerID: MCPeerID) {
        do { handle(try KagemushaNearbyEnvelopeCodec.decode(data), from: peerID) }
        catch { finish(.failure(KagemushaNearbyError.invalidMessage)) }
    }

    fileprivate func session(
        _ session: MCSession,
        didReceive stream: InputStream,
        withName streamName: String,
        fromPeer peerID: MCPeerID
    ) { _ = session; _ = stream; _ = streamName; _ = peerID }

    fileprivate func session(
        _ session: MCSession,
        didStartReceivingResourceWithName resourceName: String,
        fromPeer peerID: MCPeerID,
        with progress: Progress
    ) { _ = session; _ = resourceName; _ = peerID; _ = progress }

    fileprivate func session(
        _ session: MCSession,
        didFinishReceivingResourceWithName resourceName: String,
        fromPeer peerID: MCPeerID,
        at localURL: URL?,
        withError error: Error?
    ) { _ = session; _ = resourceName; _ = peerID; _ = localURL; _ = error }

    private func peerKey(_ peerID: MCPeerID) -> String {
        "\(peerID.displayName.utf8.count):\(peerID.displayName)#\(peerID.hash)"
    }
}

private final class KagemushaNearbyPlatformDelegate: NSObject,
    MCNearbyServiceAdvertiserDelegate,
    MCNearbyServiceBrowserDelegate,
    MCSessionDelegate
{
    private weak var owner: KagemushaNearbyExchange?

    init(owner: KagemushaNearbyExchange) {
        self.owner = owner
    }

    func advertiser(
        _ advertiser: MCNearbyServiceAdvertiser,
        didReceiveInvitationFromPeer peerID: MCPeerID,
        withContext context: Data?,
        invitationHandler: @escaping (Bool, MCSession?) -> Void
    ) {
        guard let owner else {
            invitationHandler(false, nil)
            return
        }
        owner.advertiser(
            advertiser,
            didReceiveInvitationFromPeer: peerID,
            withContext: context,
            invitationHandler: invitationHandler
        )
    }

    func advertiser(
        _ advertiser: MCNearbyServiceAdvertiser,
        didNotStartAdvertisingPeer error: Error
    ) {
        owner?.advertiser(advertiser, didNotStartAdvertisingPeer: error)
    }

    func browser(
        _ browser: MCNearbyServiceBrowser,
        foundPeer peerID: MCPeerID,
        withDiscoveryInfo info: [String: String]?
    ) {
        owner?.browser(browser, foundPeer: peerID, withDiscoveryInfo: info)
    }

    func browser(_ browser: MCNearbyServiceBrowser, lostPeer peerID: MCPeerID) {
        owner?.browser(browser, lostPeer: peerID)
    }

    func browser(
        _ browser: MCNearbyServiceBrowser,
        didNotStartBrowsingForPeers error: Error
    ) {
        owner?.browser(browser, didNotStartBrowsingForPeers: error)
    }

    func session(
        _ session: MCSession,
        peer peerID: MCPeerID,
        didChange state: MCSessionState
    ) {
        owner?.session(session, peer: peerID, didChange: state)
    }

    func session(_ session: MCSession, didReceive data: Data, fromPeer peerID: MCPeerID) {
        owner?.session(session, didReceive: data, fromPeer: peerID)
    }

    func session(
        _ session: MCSession,
        didReceive stream: InputStream,
        withName streamName: String,
        fromPeer peerID: MCPeerID
    ) {
        owner?.session(session, didReceive: stream, withName: streamName, fromPeer: peerID)
    }

    func session(
        _ session: MCSession,
        didStartReceivingResourceWithName resourceName: String,
        fromPeer peerID: MCPeerID,
        with progress: Progress
    ) {
        owner?.session(
            session,
            didStartReceivingResourceWithName: resourceName,
            fromPeer: peerID,
            with: progress
        )
    }

    func session(
        _ session: MCSession,
        didFinishReceivingResourceWithName resourceName: String,
        fromPeer peerID: MCPeerID,
        at localURL: URL?,
        withError error: Error?
    ) {
        owner?.session(
            session,
            didFinishReceivingResourceWithName: resourceName,
            fromPeer: peerID,
            at: localURL,
            withError: error
        )
    }
}
#else
public final class KagemushaNearbyExchange: @unchecked Sendable {
    public static var isAvailable: Bool {
        KagemushaNearbyAuthenticationPolicy.hasAuditedAuthenticatedTranscriptBackend
    }
    public init(timeoutSeconds: UInt64 = 90) { _ = timeoutSeconds }
    public func requestLocalNetworkAccess() async throws {
        throw KagemushaNearbyError.unavailable
    }
    public func sendPayment(
        onEvent: @escaping @Sendable (KagemushaNearbyEvent) -> Void = { _ in },
        confirmPairing: @escaping @Sendable (
            KagemushaNearbyPairingChallenge
        ) async -> KagemushaNearbyPairingDecision,
        createPayment: @escaping @Sendable (
            KagemushaRecipientReceiveOfferV2
        ) async throws -> KagemushaRecursiveSpendPeerPaymentV4
    ) async throws -> KagemushaPeerSendResult {
        _ = onEvent; _ = confirmPairing; _ = createPayment
        throw KagemushaNearbyError.unavailable
    }
    public func receivePayment(
        receiveRequest: KagemushaRecipientReceiveOfferV2,
        pairingChallenge: KagemushaNearbyPairingChallenge = .random(),
        onEvent: @escaping @Sendable (KagemushaNearbyEvent) -> Void = { _ in },
        acceptPayment: @escaping @Sendable (
            KagemushaRecursiveSpendPeerPaymentV4
        ) async throws -> KagemushaReceiverAcknowledgement
    ) async throws -> KagemushaRecursiveSpendPeerPaymentV4 {
        _ = receiveRequest; _ = pairingChallenge; _ = onEvent; _ = acceptPayment
        throw KagemushaNearbyError.unavailable
    }
    public func cancel() {}
}
#endif
