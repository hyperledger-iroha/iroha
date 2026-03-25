import Foundation

/// Lightweight wrapper around a WebSocket task for Iroha Connect flows.
///
/// This is an early scaffold – it only surfaces raw binary frames.
/// Raw frames are encoded/decoded via `ConnectCodec`, which requires the Norito bridge XCFramework.
public protocol ConnectWebSocketTask: AnyObject {
    func resume()
    func send(data: Data, completion: @Sendable @escaping (Error?) -> Void)
    func receive(completion: @Sendable @escaping (Result<Data, Error>) -> Void)
    func cancel(closeCode: ConnectCloseCode, reason: Data?)
}

/// `ConnectCloseCode` mirrors the WebSocket close codes we currently care about.
public enum ConnectCloseCode: UInt16, Sendable {
    case normalClosure = 1000
    case goingAway = 1001
    case protocolError = 1002
    case unsupportedData = 1003
    case noStatusReceived = 1005
    case abnormalClosure = 1006
    case invalidFramePayloadData = 1007
    case policyViolation = 1008
    case messageTooBig = 1009
    case mandatoryExtension = 1010
    case internalServerError = 1011
    case tlsHandshake = 1015
}

/// Factory used by `ConnectClient` so tests can inject a stub.
public struct ConnectWebSocketFactory: Sendable {
    private let makeTask: @Sendable (URLRequest) -> ConnectWebSocketTask

    public init(makeTask: @escaping @Sendable (URLRequest) -> ConnectWebSocketTask) {
        self.makeTask = makeTask
    }

    public func make(url: URL) -> ConnectWebSocketTask {
        makeTask(URLRequest(url: url))
    }

    public func make(request: URLRequest) -> ConnectWebSocketTask {
        makeTask(request)
    }

    public static func urlSession(session: URLSession = .shared) -> ConnectWebSocketFactory {
        ConnectWebSocketFactory { request in
            URLSessionConnectWebSocketTask(session: session, request: request)
        }
    }
}

final class URLSessionConnectWebSocketTask: ConnectWebSocketTask {
    private let task: URLSessionWebSocketTask

    init(session: URLSession, request: URLRequest) {
        task = session.webSocketTask(with: request)
    }

    func resume() {
        task.resume()
    }

    func send(data: Data, completion: @Sendable @escaping (Error?) -> Void) {
        task.send(.data(data), completionHandler: completion)
    }

    func receive(completion: @Sendable @escaping (Result<Data, Error>) -> Void) {
        task.receive { result in
            switch result {
            case .success(.data(let data)):
                completion(.success(data))
            case .success(.string(let string)):
                completion(.success(Data(string.utf8)))
            case .failure(let error):
                completion(.failure(error))
            @unknown default:
                completion(.failure(ConnectClient.ClientError.unknownPayload))
            }
        }
    }

    func cancel(closeCode: ConnectCloseCode, reason: Data?) {
        task.cancel(with: URLSessionWebSocketTask.CloseCode(rawValue: Int(closeCode.rawValue)) ?? .normalClosure,
                    reason: reason)
    }
}

/// Early Connect client scaffold. Handles the WebSocket lifecycle and exposes async receive/send helpers.
/// Norito Connect frames flow through `ConnectCodec` (throws `ConnectCodecError.bridgeUnavailable` when
/// the bridge is missing), and key exchange helpers are available via `ConnectCrypto`.
/// Encryption of ciphertext envelopes will plug in once the bridge exports high-level envelope builders.
public actor ConnectClient {
    public enum ClientError: Error, LocalizedError, Sendable {
        case alreadyStarted
        case closed
        case unknownPayload

        public var errorDescription: String? {
            switch self {
            case .alreadyStarted:
                return "ConnectClient was already started."
            case .closed:
                return "ConnectClient is closed."
            case .unknownPayload:
                return "Received unknown payload type from WebSocket."
            }
        }
    }

    private let task: ConnectWebSocketTask
    private var started = false
    private var isClosed = false

    public init(url: URL,
                webSocketFactory: ConnectWebSocketFactory = .urlSession()) {
        self.task = webSocketFactory.make(url: url)
    }

    public init(request: URLRequest,
                webSocketFactory: ConnectWebSocketFactory = .urlSession()) {
        self.task = webSocketFactory.make(request: request)
    }

    public static func makeWebSocketURL(baseURL: URL,
                                        sid: String,
                                        role: ToriiConnectRole,
                                        endpointPath: String = "/v1/connect/ws") throws -> URL {
        let normalizedSid = try validateNonEmpty(sid, field: "sid")
        let normalizedPath = endpointPath.hasPrefix("/") ? String(endpointPath.dropFirst()) : endpointPath
        guard var components = URLComponents(url: baseURL.appendingPathComponent(normalizedPath),
                                             resolvingAgainstBaseURL: false) else {
            throw ToriiClientError.invalidURL(endpointPath)
        }
        components.queryItems = [
            URLQueryItem(name: "sid", value: normalizedSid),
            URLQueryItem(name: "role", value: role.rawValue)
        ]
        guard let urlWithQuery = components.url else {
            throw ToriiClientError.invalidURL(endpointPath)
        }
        guard var wsComponents = URLComponents(url: urlWithQuery, resolvingAgainstBaseURL: false) else {
            throw ToriiClientError.invalidURL(endpointPath)
        }
        wsComponents.scheme = wsComponents.scheme?.lowercased() == "https" ? "wss" : "ws"
        guard let finalURL = wsComponents.url else {
            throw ToriiClientError.invalidURL(endpointPath)
        }
        return finalURL
    }

    public static func makeWebSocketRequest(baseURL: URL,
                                            sid: String,
                                            role: ToriiConnectRole,
                                            token: String,
                                            endpointPath: String = "/v1/connect/ws") throws -> URLRequest {
        let url = try makeWebSocketURL(baseURL: baseURL,
                                       sid: sid,
                                       role: role,
                                       endpointPath: endpointPath)
        let normalizedToken = try validateNonEmpty(token, field: "token")
        if let violation = IrohaTransportSecurity.webSocketViolation(context: "ConnectClient",
                                                                     baseURL: baseURL,
                                                                     targetURL: url,
                                                                     hasCredentials: true) {
            throw ToriiClientError.invalidPayload(violation)
        }
        var request = URLRequest(url: url)
        request.setValue("Bearer \(normalizedToken)", forHTTPHeaderField: "Authorization")
        return request
    }

    /// Begin the WebSocket session. Safe to call multiple times (subsequent calls are no-ops).
    public func start() {
        guard !started else { return }
        started = true
        task.resume()
    }

    /// Close the WebSocket with the provided code/reason.
    public func close(code: ConnectCloseCode = .normalClosure, reason: Data? = nil) {
        guard !isClosed else { return }
        isClosed = true
        task.cancel(closeCode: code, reason: reason)
    }

    /// Send a raw binary payload.
    public func send(data: Data) async throws {
        try Task.checkCancellation()
        if isClosed {
            throw ClientError.closed
        }
        return try await withCheckedThrowingContinuation { continuation in
            task.send(data: data) { error in
                if let error {
                    continuation.resume(throwing: error)
                } else {
                    continuation.resume(returning: ())
                }
            }
        }
    }

    /// Send a typed Connect frame using the placeholder codec.
    public func send(frame: ConnectFrame) async throws {
        let data = try ConnectCodec.encode(frame)
        try await send(data: data)
    }

    /// Receive the next binary payload, suspending until one is available.
    public func receive() async throws -> Data {
        try Task.checkCancellation()
        if isClosed {
            throw ClientError.closed
        }
        return try await withCheckedThrowingContinuation { continuation in
            task.receive { result in
                continuation.resume(with: result)
            }
        }
    }

    /// Receive and decode the next Connect frame.
    public func receiveFrame() async throws -> ConnectFrame {
        let data = try await receive()
        return try ConnectCodec.decode(data)
    }

    private static func validateNonEmpty(_ value: String, field: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw ToriiClientError.invalidPayload("\(field) must be a non-empty string")
        }
        return trimmed
    }
}
