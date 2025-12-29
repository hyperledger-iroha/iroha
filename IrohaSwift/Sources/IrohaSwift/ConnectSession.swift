import Foundation
#if canImport(Combine)
import Combine
#endif

public enum ConnectSessionError: Error, LocalizedError, Sendable {
    case streamEnded
    case sessionClosed
    case missingDecryptionKeys
    case flowControlExceeded(direction: ConnectDirection)
    case clientError(ConnectClient.ClientError)
    case envelopeError(ConnectEnvelopeError)
    case unknown(String)

    public var errorDescription: String? {
        switch self {
        case .streamEnded:
            return "Connect frame stream ended unexpectedly."
        case .sessionClosed:
            return "Connect session is already closed."
        case .missingDecryptionKeys:
            return "Connect session is missing direction keys required to decrypt ciphertext frames."
        case let .flowControlExceeded(direction):
            return "Connect flow control window exhausted for direction \(direction)."
        case .clientError(let error):
            return error.errorDescription ?? "Connect client error."
        case .envelopeError(let error):
            return error.errorDescription ?? "Connect envelope error."
        case .unknown(let description):
            return description
        }
    }
}

/// High-level helper managing a Connect session over a `ConnectClient`.
/// NOTE: Uses `ConnectCodec` (Norito bridge required) for frame serialization.
/// Set `directionKeys` via the initializer or `setDirectionKeys(_:)` to enable ciphertext decryption.
public final class ConnectSession: @unchecked Sendable {
    public typealias ConnectEventStreamBuilder = (_ session: ConnectSession,
                                                  _ filter: ConnectEventFilter) -> AsyncThrowingStream<ConnectEvent, Error>

    public enum SessionState {
        case idle
        case opened
        case closed
    }

    private let client: ConnectClient
    private let sessionID: Data
    private var sequence: UInt64 = 0
    private var state: SessionState = .idle
    private var directionKeys: ConnectDirectionKeys?
    private let diagnostics: ConnectSessionDiagnostics
    private let customEventStreamBuilder: ConnectEventStreamBuilder?
    private var flowControl: ConnectFlowController?

    public init(sessionID: Data,
                client: ConnectClient,
                directionKeys: ConnectDirectionKeys? = nil,
                flowControl: ConnectFlowControlWindow? = nil,
                diagnostics: ConnectSessionDiagnostics? = nil,
                eventStreamBuilder: ConnectEventStreamBuilder? = nil) {
        self.sessionID = sessionID
        self.client = client
        self.directionKeys = directionKeys
        if let window = flowControl {
            self.flowControl = ConnectFlowController(window: window)
        }
        self.diagnostics = diagnostics ?? ConnectSessionDiagnostics(sessionID: sessionID)
        self.customEventStreamBuilder = eventStreamBuilder
    }

    /// Sends an `Open` control frame towards the wallet.
    public func sendOpen(open: ConnectOpen) async throws {
        try ensureNotClosed()
        guard state == .idle else { return }
        try await sendControl(.open(open), direction: .appToWallet)
        state = .opened
    }

    /// Sends a `Close` control frame and closes the underlying socket.
    public func sendClose(close: ConnectClose = ConnectClose(role: .app, code: 1000, reason: nil, retryable: false)) async throws {
        guard state != .closed else { return }
        try await sendControl(.close(close), direction: .appToWallet)
        await client.close()
        state = .closed
    }

    /// Waits for the next control frame from the counterparty.
    @discardableResult
    public func nextControlFrame() async throws -> ConnectControl {
        for try await frame in ConnectFrameSequence(client: client) {
            if let control = try extractControl(from: frame) {
                return control
            }
        }
        throw ConnectSessionError.streamEnded
    }

    /// Waits for the next encrypted envelope (e.g., sign results, encrypted controls).
    public func nextEnvelope() async throws -> ConnectEnvelope {
        for try await frame in ConnectFrameSequence(client: client) {
            if case .ciphertext = frame.kind {
                return try decryptEnvelope(from: frame)
            }
        }
        throw ConnectSessionError.streamEnded
    }

    /// Update the symmetric keys used to decrypt ciphertext frames.
    public func setDirectionKeys(_ keys: ConnectDirectionKeys) {
        directionKeys = keys
    }

    /// Install or update the flow-control window for inbound frames.
    public func setFlowControlWindow(_ window: ConnectFlowControlWindow) {
        flowControl = ConnectFlowController(window: window)
    }

    /// Grant additional tokens for the given direction (for example when the peer sends a flow-control update).
    public func grantFlowControl(direction: ConnectDirection, tokens: UInt64) {
        flowControl?.grant(direction: direction, tokens: tokens)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func eventStream(filter: ConnectEventFilter = ConnectEventFilter()) -> AsyncThrowingStream<ConnectEvent, Error> {
        if let builder = customEventStreamBuilder {
            return builder(self, filter)
        }
        return ConnectSession.makeDefaultEventStream(session: self, filter: filter)
    }

    @available(iOS 15.0, macOS 12.0, *)
    public func balanceStream(accountID: String? = nil) -> AsyncThrowingStream<ConnectBalanceSnapshot, Error> {
        let filter = ConnectEventFilter.balanceSnapshots(accountID: accountID)
        let events = eventStream(filter: filter)
        return ConnectBalanceStreamBuilder.stream(events: events) { [weak self] in
            guard let diagnostics = self?.diagnostics else { return nil }
            return try diagnostics.snapshot()
        }
    }

#if canImport(Combine)
    @available(iOS 15.0, macOS 12.0, *)
    @MainActor
    public func balancePublisher(accountID: String? = nil,
                                 scheduler: DispatchQueue? = .main) -> AnyPublisher<ConnectBalanceSnapshot, ConnectSessionError> {
        makePublisher(stream: { self.balanceStream(accountID: accountID) }, scheduler: scheduler)
    }

    @available(iOS 15.0, macOS 12.0, *)
    @MainActor
    public func eventsPublisher(filter: ConnectEventFilter = ConnectEventFilter(),
                                scheduler: DispatchQueue? = .main) -> AnyPublisher<ConnectEvent, ConnectSessionError> {
        makePublisher(stream: { self.eventStream(filter: filter) }, scheduler: scheduler)
    }
#endif

    private func extractControl(from frame: ConnectFrame) throws -> ConnectControl? {
        switch frame.kind {
        case .control(let control):
            return control
        case .ciphertext:
            return try decryptControl(from: frame)
        }
    }

    private func decryptControl(from frame: ConnectFrame) throws -> ConnectControl? {
        let envelope = try decryptEnvelope(from: frame)
        return envelope.payload.control
    }

    private func decryptEnvelope(from frame: ConnectFrame) throws -> ConnectEnvelope {
        guard case .ciphertext = frame.kind else {
            throw ConnectEnvelopeError.unsupportedFrameKind
        }
        try flowControl?.consume(direction: frame.direction)
        guard let keys = directionKeys else {
            throw ConnectSessionError.missingDecryptionKeys
        }
        let symmetricKey = frame.direction == .appToWallet ? keys.appToWallet : keys.walletToApp
        return try ConnectEnvelope.decrypt(frame: frame, symmetricKey: symmetricKey)
    }

    private func sendControl(_ control: ConnectControl, direction: ConnectDirection) async throws {
        let frame = ConnectFrame(sessionID: sessionID,
                                 direction: direction,
                                 sequence: nextSequence(),
                                 kind: .control(control))
        try await client.send(frame: frame)
    }

    private func nextSequence() -> UInt64 {
        sequence &+= 1
        return sequence
    }

    private func ensureNotClosed() throws {
        if state == .closed {
            throw ConnectSessionError.sessionClosed
        }
    }

    private func decodeEvent(from frame: ConnectFrame) throws -> ConnectEvent? {
        guard case .ciphertext = frame.kind else {
            return nil
        }
        let envelope = try decryptEnvelope(from: frame)
        return ConnectEvent(sequence: envelope.sequence,
                            direction: frame.direction,
                            payload: envelope.payload,
                            receivedAt: Date())
    }

    fileprivate static func mapError(_ error: Error) -> ConnectSessionError {
        if let sessionError = error as? ConnectSessionError {
            return sessionError
        }
        if let clientError = error as? ConnectClient.ClientError {
            return .clientError(clientError)
        }
        if let envelopeError = error as? ConnectEnvelopeError {
            return .envelopeError(envelopeError)
        }
        if let flowError = error as? ConnectFlowController.FlowError {
            return .flowControlExceeded(direction: flowError.direction)
        }
        return .unknown(String(describing: error))
    }

    @available(iOS 15.0, macOS 12.0, *)
    private static func makeDefaultEventStream(session: ConnectSession,
                                               filter: ConnectEventFilter) -> AsyncThrowingStream<ConnectEvent, Error> {
        return AsyncThrowingStream<ConnectEvent, Error>(ConnectEvent.self, bufferingPolicy: .unbounded) { continuation in
            let sequence = ConnectFrameSequence(client: session.client)
            let task = Task {
                do {
                    for try await frame in sequence {
                        try Task.checkCancellation()
                        guard let event = try session.decodeEvent(from: frame) else { continue }
                        if filter.matches(event) {
                            continuation.yield(event)
                        }
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: ConnectSession.mapError(error))
                }
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }

#if canImport(Combine)
    @MainActor
    private func makePublisher<Output>(stream: @Sendable @escaping () -> AsyncThrowingStream<Output, Error>,
                                       scheduler: DispatchQueue?) -> AnyPublisher<Output, ConnectSessionError> {
        let subject = PassthroughSubject<Output, ConnectSessionError>()
        let task = Task {
            do {
                var iterator = stream().makeAsyncIterator()
                while let value = try await iterator.next() {
                    subject.send(value)
                }
                subject.send(completion: .finished)
            } catch {
                subject.send(completion: .failure(ConnectSession.mapError(error)))
            }
        }
        let publisher = subject.handleEvents(receiveCancel: {
            task.cancel()
        })
        if let scheduler {
            return publisher.receive(on: scheduler).eraseToAnyPublisher()
        }
        return publisher.eraseToAnyPublisher()
    }
#endif
}

@available(iOS 15.0, macOS 12.0, *)
enum ConnectBalanceStreamBuilder {
    static func stream(events: AsyncThrowingStream<ConnectEvent, Error>,
                       diagnosticsProvider: @escaping () throws -> ConnectQueueSnapshot?) -> AsyncThrowingStream<ConnectBalanceSnapshot, Error> {
        let diagnosticsBox = DiagnosticsProviderBox(provider: diagnosticsProvider)
        return AsyncThrowingStream<ConnectBalanceSnapshot, Error>(ConnectBalanceSnapshot.self, bufferingPolicy: .unbounded) { continuation in
            let task = Task {
                do {
                    var iterator = events.makeAsyncIterator()
                    while let event = try await iterator.next() {
                        guard case .balanceSnapshot(var snapshot) = event.payload else {
                            continue
                        }
                        snapshot.sequence = event.sequence
                        snapshot.receivedAt = event.receivedAt
                        snapshot.queueDiagnostics = try diagnosticsBox.call()
                        continuation.yield(snapshot)
                    }
                    continuation.finish()
                } catch {
                    continuation.finish(throwing: ConnectSession.mapError(error))
                }
            }
            continuation.onTermination = { _ in task.cancel() }
        }
    }
}

private struct DiagnosticsProviderBox: @unchecked Sendable {
    let provider: () throws -> ConnectQueueSnapshot?

    func call() throws -> ConnectQueueSnapshot? {
        try provider()
    }
}
