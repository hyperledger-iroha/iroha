import Foundation

private final class BootleLanternIssuanceResponseDelegateV1: NSObject,
    URLSessionDataDelegate, URLSessionTaskDelegate, @unchecked Sendable
{
    private let maximumBytes: Int
    private let lock = NSLock()
    private var continuation: CheckedContinuation<(Data, HTTPURLResponse), Error>?
    private var response: HTTPURLResponse?
    private var body = Data()
    private var completed = false

    init(maximumBytes: Int) {
        self.maximumBytes = maximumBytes
        body.reserveCapacity(maximumBytes)
    }

    func execute(
        session: URLSession,
        request: URLRequest
    ) async throws -> (Data, HTTPURLResponse) {
        try await withCheckedThrowingContinuation { continuation in
            lock.lock()
            self.continuation = continuation
            lock.unlock()
            session.dataTask(with: request).resume()
        }
    }

    func destroyBufferedBody() {
        lock.lock()
        body.resetBytes(in: body.startIndex..<body.endIndex)
        lock.unlock()
    }

    func urlSession(
        _ session: URLSession,
        task: URLSessionTask,
        willPerformHTTPRedirection response: HTTPURLResponse,
        newRequest request: URLRequest,
        completionHandler: @escaping (URLRequest?) -> Void
    ) {
        completionHandler(nil)
    }

    func urlSession(
        _ session: URLSession,
        dataTask: URLSessionDataTask,
        didReceive response: URLResponse,
        completionHandler: @escaping (URLSession.ResponseDisposition) -> Void
    ) {
        guard let response = response as? HTTPURLResponse else {
            finish(.failure(.invalidResponse("response was not HTTP")))
            completionHandler(.cancel)
            return
        }
        lock.lock()
        self.response = response
        lock.unlock()
        completionHandler(.allow)
    }

    func urlSession(
        _ session: URLSession,
        dataTask: URLSessionDataTask,
        didReceive data: Data
    ) {
        lock.lock()
        guard !completed else {
            lock.unlock()
            return
        }
        guard body.count <= maximumBytes - data.count else {
            lock.unlock()
            dataTask.cancel()
            finish(.failure(.invalidResponse("response body exceeds its exact bound")))
            return
        }
        body.append(data)
        lock.unlock()
    }

    func urlSession(
        _ session: URLSession,
        task: URLSessionTask,
        didCompleteWithError error: Error?
    ) {
        lock.lock()
        guard !completed else {
            lock.unlock()
            return
        }
        let response = self.response
        let body = self.body
        lock.unlock()
        if error != nil {
            finish(.failure(.transportFailure))
        } else if let response {
            finish(.success((body, response)))
        } else {
            finish(.failure(.invalidResponse("response was unavailable")))
        }
    }

    private func finish(_ result: Result<(Data, HTTPURLResponse), BootleLanternIssuanceClientErrorV1>) {
        lock.lock()
        guard !completed, let continuation else {
            lock.unlock()
            return
        }
        completed = true
        self.continuation = nil
        if case .failure = result {
            body.resetBytes(in: body.startIndex..<body.endIndex)
        }
        lock.unlock()
        continuation.resume(with: result.mapError { $0 as Error })
    }
}

/// Opaque, bounded issuer credential with explicit in-memory destruction.
public final class BootleLanternIssuanceCredentialV1: @unchecked Sendable,
    CustomStringConvertible, CustomDebugStringConvertible, CustomReflectable
{
    /// Maximum decoded credential length accepted by Torii.
    public static let maximumBytes = 4_096

    private let lock = NSLock()
    private var bytes: [UInt8]
    private var destroyed = false

    /// Defensively copies and validates opaque credential bytes.
    public init(opaqueBytes: Data) throws {
        guard !opaqueBytes.isEmpty, opaqueBytes.count <= Self.maximumBytes else {
            throw BootleLanternIssuanceClientErrorV1.invalidCredential
        }
        bytes = Array(opaqueBytes)
    }

    /// Decodes exactly one canonical, unpadded base64url credential.
    public init(canonicalBase64URL encoded: String) throws {
        let maximumEncodedBytes = ((Self.maximumBytes + 2) / 3) * 4
        guard
            !encoded.isEmpty,
            encoded.utf8.count <= maximumEncodedBytes,
            encoded.utf8.count % 4 != 1,
            encoded.unicodeScalars.allSatisfy({ scalar in
                switch scalar.value {
                case 45, 48...57, 65...90, 95, 97...122:
                    return true
                default:
                    return false
                }
            })
        else {
            throw BootleLanternIssuanceClientErrorV1.invalidCredential
        }
        let standard = encoded.replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        let padded = standard + String(repeating: "=", count: (4 - standard.count % 4) % 4)
        guard var decoded = Data(base64Encoded: padded, options: []) else {
            throw BootleLanternIssuanceClientErrorV1.invalidCredential
        }
        defer { decoded.resetBytes(in: decoded.startIndex..<decoded.endIndex) }
        guard
            !decoded.isEmpty,
            decoded.count <= Self.maximumBytes,
            Self.canonicalBase64URL(decoded) == encoded
        else {
            throw BootleLanternIssuanceClientErrorV1.invalidCredential
        }
        bytes = Array(decoded)
    }

    deinit {
        destroy()
    }

    /// Overwrites the retained credential byte buffer. This operation is idempotent.
    public func destroy() {
        lock.lock()
        defer { lock.unlock() }
        guard !destroyed else { return }
        for index in bytes.indices {
            bytes[index] = 0
        }
        destroyed = true
    }

    /// Deliberately redacted diagnostic representation.
    public var description: String {
        "BootleLanternIssuanceCredentialV1([REDACTED])"
    }

    /// Deliberately redacted debug representation.
    public var debugDescription: String { description }

    /// Prevents reflection-based loggers from traversing retained secret bytes.
    public var customMirror: Mirror {
        Mirror(self, children: [:], displayStyle: .class)
    }

    fileprivate func authorizationHeaderValue() throws -> String {
        lock.lock()
        defer { lock.unlock() }
        guard !destroyed else {
            throw BootleLanternIssuanceClientErrorV1.credentialDestroyed
        }
        var secret = Data(bytes)
        defer { secret.resetBytes(in: secret.startIndex..<secret.endIndex) }
        return "Bearer \(Self.canonicalBase64URL(secret))"
    }

    private static func canonicalBase64URL(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
}

/// Fail-closed credential, transport, or response validation failure.
public enum BootleLanternIssuanceClientErrorV1: Error, Sendable, Equatable {
    /// The credential was empty, oversized, or not canonical unpadded base64url.
    case invalidCredential
    /// The credential was explicitly destroyed before use.
    case credentialDestroyed
    /// The configured Torii base URL was not an origin-only HTTPS URL.
    case invalidBaseURL
    /// The issue request did not have the exact first-release byte length.
    case invalidIssueRequestLength(expected: Int, actual: Int)
    /// The exact-size issue request did not contain canonical `ILA1 || ILQ1` magics.
    case invalidIssueRequestMagic
    /// The one permitted transport attempt failed.
    case transportFailure
    /// Torii returned a canonical structured issuance error.
    case httpError(status: Int, code: String, retryAfterSeconds: Int?)
    /// The HTTP response failed strict validation.
    case invalidResponse(String)
}

extension BootleLanternIssuanceClientErrorV1: LocalizedError {
    public var errorDescription: String? {
        switch self {
        case .invalidCredential:
            return "Bootle/Lantern issuance credential is invalid."
        case .credentialDestroyed:
            return "Bootle/Lantern issuance credential has been destroyed."
        case .invalidBaseURL:
            return "Bootle/Lantern issuance requires an origin-only HTTPS base URL."
        case let .invalidIssueRequestLength(expected, actual):
            return "Bootle/Lantern issue request must be exactly \(expected) bytes; got \(actual)."
        case .invalidIssueRequestMagic:
            return "Bootle/Lantern issue request must contain canonical ILA1 || ILQ1 magics."
        case .transportFailure:
            return "Bootle/Lantern issuance request failed."
        case let .httpError(status, code, retryAfterSeconds):
            let retry = retryAfterSeconds.map { "; retry after \($0) second(s)" } ?? ""
            return "Bootle/Lantern issuance returned HTTP \(status): \(code)\(retry)."
        case .invalidResponse(let reason):
            return "Bootle/Lantern issuance response is invalid: \(reason)."
        }
    }
}

/// Exact, single-attempt client for first-release native Bootle/Lantern issuance.
public final class BootleLanternIssuanceClientV1: @unchecked Sendable {
    /// Canonical authorization route.
    public static let authorizePath = "/v1/privacy/bootle-lantern/issuance/authorize"
    /// Canonical blind-issuance route.
    public static let issuePath = "/v1/privacy/bootle-lantern/issuance/issue"
    /// Sole request and successful-response media type.
    public static let mediaType = "application/x-norito"
    /// Exact encoded authorization response length.
    public static let authorizationBytes = 320
    /// Exact encoded `ILA1 || ILQ1` request length.
    public static let issueRequestBytes = 71_896
    /// Exact encoded `ILR1` response length.
    public static let issueResponseBytes = 3_176
    /// Maximum accepted structured issuance-error body length.
    public static let errorResponseMaximumBytes = 512

    private static let jsonMediaType = "application/json"
    private static let authorizationMagic = Data("ILA1".utf8)
    private static let blindRequestMagic = Data("ILQ1".utf8)
    private static let responseMagic = Data("ILR1".utf8)
    private static let wwwAuthenticateValue =
        "Bearer realm=\"iroha-bootle-lantern-issuance\""
    private static let errorEnvelopeTypeName = "iroha_torii_shared::ErrorEnvelope"

    /// Canonical Torii HTTPS origin.
    public let baseURL: URL

    private let configuration: URLSessionConfiguration
    private let timeout: TimeInterval

    /// Creates a strict client from an HTTPS origin and a transport configuration.
    public init(
        baseURL: URL,
        session: URLSession = .shared,
        timeout: TimeInterval = 15
    ) throws {
        guard
            timeout > 0,
            timeout.isFinite,
            let components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false),
            components.scheme?.lowercased() == "https",
            components.host?.isEmpty == false,
            components.user == nil,
            components.password == nil,
            components.query == nil,
            components.fragment == nil,
            components.percentEncodedPath.isEmpty || components.percentEncodedPath == "/"
        else {
            throw BootleLanternIssuanceClientErrorV1.invalidBaseURL
        }
        var origin = components
        origin.scheme = "https"
        origin.path = ""
        guard let normalized = origin.url else {
            throw BootleLanternIssuanceClientErrorV1.invalidBaseURL
        }
        self.baseURL = normalized
        self.timeout = timeout

        let configuration = session.configuration
        configuration.httpAdditionalHeaders = nil
        configuration.urlCache = nil
        configuration.requestCachePolicy = .reloadIgnoringLocalAndRemoteCacheData
        configuration.httpCookieStorage = nil
        configuration.httpShouldSetCookies = false
        configuration.httpCookieAcceptPolicy = .never
        self.configuration = configuration
    }

    /// Requests one exact 320-byte `ILA1` authorization exactly once.
    public func authorize(
        credential: BootleLanternIssuanceCredentialV1
    ) async throws -> Data {
        try await executeExact(
            operation: "Bootle/Lantern issuance authorization",
            path: Self.authorizePath,
            credential: credential,
            body: Data(),
            expectedBytes: Self.authorizationBytes,
            expectedMagic: Self.authorizationMagic
        )
    }

    /// Submits exact `ILA1 || ILQ1` and returns one exact `ILR1` response.
    public func issue(
        credential: BootleLanternIssuanceCredentialV1,
        canonicalRequest: Data
    ) async throws -> Data {
        guard canonicalRequest.count == Self.issueRequestBytes else {
            throw BootleLanternIssuanceClientErrorV1.invalidIssueRequestLength(
                expected: Self.issueRequestBytes,
                actual: canonicalRequest.count
            )
        }
        guard
            canonicalRequest.starts(with: Self.authorizationMagic),
            canonicalRequest.subdata(
                in: Self.authorizationBytes..<(Self.authorizationBytes + Self.blindRequestMagic.count)
            ) == Self.blindRequestMagic
        else {
            throw BootleLanternIssuanceClientErrorV1.invalidIssueRequestMagic
        }
        return try await executeExact(
            operation: "Bootle/Lantern blind issuance",
            path: Self.issuePath,
            credential: credential,
            body: Data(canonicalRequest),
            expectedBytes: Self.issueResponseBytes,
            expectedMagic: Self.responseMagic
        )
    }

    private func executeExact(
        operation: String,
        path: String,
        credential: BootleLanternIssuanceCredentialV1,
        body: Data,
        expectedBytes: Int,
        expectedMagic: Data
    ) async throws -> Data {
        guard var components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false) else {
            throw BootleLanternIssuanceClientErrorV1.invalidBaseURL
        }
        components.path = path
        guard let target = components.url else {
            throw BootleLanternIssuanceClientErrorV1.invalidBaseURL
        }
        var request = URLRequest(
            url: target,
            cachePolicy: .reloadIgnoringLocalAndRemoteCacheData,
            timeoutInterval: timeout
        )
        request.httpMethod = "POST"
        request.httpBody = body
        request.httpShouldHandleCookies = false
        request.setValue(try credential.authorizationHeaderValue(), forHTTPHeaderField: "Authorization")
        request.setValue(Self.mediaType, forHTTPHeaderField: "Content-Type")
        request.setValue(Self.mediaType, forHTTPHeaderField: "Accept")
        request.setValue("identity", forHTTPHeaderField: "Accept-Encoding")
        request.setValue("no-store", forHTTPHeaderField: "Cache-Control")
        request.setValue("no-cache", forHTTPHeaderField: "Pragma")

        if IrohaTransportSecurity.httpViolation(
            context: "BootleLanternIssuanceClientV1",
            baseURL: baseURL,
            targetURL: target,
            headers: request.allHTTPHeaderFields ?? [:],
            body: body
        ) != nil {
            throw BootleLanternIssuanceClientErrorV1.invalidBaseURL
        }

        let delegate = BootleLanternIssuanceResponseDelegateV1(
            maximumBytes: max(expectedBytes, Self.errorResponseMaximumBytes)
        )
        defer { delegate.destroyBufferedBody() }
        let session = URLSession(
            configuration: configuration,
            delegate: delegate,
            delegateQueue: nil
        )
        defer { session.finishTasksAndInvalidate() }
        let (data, response) = try await delegate.execute(session: session, request: request)

        guard response.url == target else {
            throw BootleLanternIssuanceClientErrorV1.invalidResponse(
                "response URL does not match the request"
            )
        }
        guard response.statusCode == 200 else {
            throw try decodeErrorResponse(data, response: response, operation: operation)
        }
        try validateResponseHeaders(response, operation: operation, expectedBytes: expectedBytes)
        guard data.count == expectedBytes else {
            throw BootleLanternIssuanceClientErrorV1.invalidResponse(
                "\(operation) response must be exactly \(expectedBytes) bytes"
            )
        }
        guard data.starts(with: expectedMagic) else {
            throw BootleLanternIssuanceClientErrorV1.invalidResponse(
                "\(operation) response wire magic is invalid"
            )
        }
        return Data(data)
    }

    private func decodeErrorResponse(
        _ data: Data,
        response: HTTPURLResponse,
        operation: String
    ) throws -> BootleLanternIssuanceClientErrorV1 {
        guard let contract = Self.errorContract(status: response.statusCode) else {
            return .invalidResponse("\(operation) returned an unsupported error response")
        }
        guard !data.isEmpty, data.count <= Self.errorResponseMaximumBytes else {
            return .invalidResponse("\(operation) error response body has an invalid length")
        }
        guard headerValues(response, name: "Content-Type") == [contract.mediaType] else {
            return .invalidResponse("\(operation) error response Content-Type is invalid")
        }
        guard headerValues(response, name: "Content-Encoding").isEmpty else {
            return .invalidResponse("\(operation) error response contains Content-Encoding")
        }
        let lengths = headerValues(response, name: "Content-Length")
        guard lengths.isEmpty || lengths == [String(data.count)] else {
            return .invalidResponse("\(operation) error response Content-Length is invalid")
        }
        let retryAfter = headerValues(response, name: "Retry-After")
        if contract.retryAfterSeconds == 1 {
            guard retryAfter == ["1"] else {
                return .invalidResponse("\(operation) error response Retry-After is invalid")
            }
        } else if !retryAfter.isEmpty {
            return .invalidResponse("\(operation) error response has an unexpected Retry-After")
        }
        let wwwAuthenticate = headerValues(response, name: "WWW-Authenticate")
        if response.statusCode == 401 {
            guard wwwAuthenticate == [Self.wwwAuthenticateValue] else {
                return .invalidResponse(
                    "\(operation) error response WWW-Authenticate is invalid"
                )
            }
        } else if !wwwAuthenticate.isEmpty {
            return .invalidResponse(
                "\(operation) error response has an unexpected WWW-Authenticate"
            )
        }

        let envelope: (code: String, message: String)
        do {
            if response.statusCode == 406 {
                let expected = Data(
                    "{\"code\":\"\(contract.code)\",\"message\":\"\(contract.code)\"}".utf8
                )
                guard data == expected else {
                    return .invalidResponse("\(operation) JSON error envelope is not canonical")
                }
                let object = try JSONSerialization.jsonObject(with: data)
                guard
                    let dictionary = object as? [String: String],
                    Set(dictionary.keys) == Set(["code", "message"]),
                    let code = dictionary["code"],
                    let message = dictionary["message"]
                else {
                    return .invalidResponse("\(operation) JSON error envelope is invalid")
                }
                envelope = (code, message)
            } else {
                envelope = try decodeNoritoErrorEnvelope(data)
            }
        } catch {
            return .invalidResponse("\(operation) returned an invalid error envelope")
        }
        guard envelope.code == contract.code, envelope.message == contract.code else {
            return .invalidResponse("\(operation) error envelope does not match its HTTP status")
        }
        return .httpError(
            status: response.statusCode,
            code: contract.code,
            retryAfterSeconds: contract.retryAfterSeconds
        )
    }

    private func decodeNoritoErrorEnvelope(_ data: Data) throws -> (code: String, message: String) {
        guard
            let frame = noritoDecodeFrame(data),
            frame.header.schema == noritoSchemaHash(forTypeName: Self.errorEnvelopeTypeName),
            frame.header.compression == .none,
            frame.header.flags == NoritoHeader.compactLen,
            frame.paddingLength == 0
        else {
            throw BootleLanternIssuanceClientErrorV1.invalidResponse(
                "Norito error envelope framing is invalid"
            )
        }
        var reader = CanonicalNoritoReader(data: frame.payload)
        let codeField = try reader.readCompactField()
        let messageField = try reader.readCompactField()
        let detailsField = try reader.readCompactField()
        guard
            detailsField == Data([0]),
            reader.remaining() == 0
        else {
            throw BootleLanternIssuanceClientErrorV1.invalidResponse(
                "Norito error envelope payload is invalid"
            )
        }
        let code = try decodeCompactNoritoStringField(codeField)
        let message = try decodeCompactNoritoStringField(messageField)
        return (code, message)
    }

    private func decodeCompactNoritoStringField(_ field: Data) throws -> String {
        var reader = CanonicalNoritoReader(data: field)
        let length = try reader.readVarint()
        guard length <= UInt64(Int.max) else {
            throw BootleLanternIssuanceClientErrorV1.invalidResponse(
                "Norito error-envelope string length overflows Int"
            )
        }
        let bytes = try reader.readBytes(Int(length))
        guard
            reader.remaining() == 0,
            let value = String(data: bytes, encoding: .utf8)
        else {
            throw BootleLanternIssuanceClientErrorV1.invalidResponse(
                "Norito error-envelope string field is invalid"
            )
        }
        return value
    }

    private static func errorContract(
        status: Int
    ) -> (code: String, mediaType: String, retryAfterSeconds: Int?)? {
        switch status {
        case 400:
            return ("privacy_issuance_invalid_request", mediaType, nil)
        case 401:
            return ("privacy_issuance_unauthorized", mediaType, nil)
        case 406:
            return ("privacy_issuance_not_acceptable", jsonMediaType, nil)
        case 409:
            return ("privacy_issuance_state_conflict", mediaType, nil)
        case 413:
            return ("privacy_issuance_payload_too_large", mediaType, nil)
        case 415:
            return ("privacy_issuance_unsupported_media_type", mediaType, nil)
        case 429:
            return ("privacy_issuance_capacity_exhausted", mediaType, 1)
        case 503:
            return ("privacy_issuance_unavailable", mediaType, nil)
        default:
            return nil
        }
    }

    private func validateResponseHeaders(
        _ response: HTTPURLResponse,
        operation: String,
        expectedBytes: Int
    ) throws {
        let contentTypes = headerValues(response, name: "Content-Type")
        guard contentTypes == [Self.mediaType] else {
            throw BootleLanternIssuanceClientErrorV1.invalidResponse(
                "\(operation) response Content-Type must be exactly \(Self.mediaType)"
            )
        }
        guard headerValues(response, name: "Content-Encoding").isEmpty else {
            throw BootleLanternIssuanceClientErrorV1.invalidResponse(
                "\(operation) response must not contain Content-Encoding"
            )
        }
        guard headerValues(response, name: "WWW-Authenticate").isEmpty else {
            throw BootleLanternIssuanceClientErrorV1.invalidResponse(
                "\(operation) response has an unexpected WWW-Authenticate"
            )
        }
        let lengths = headerValues(response, name: "Content-Length")
        guard !lengths.isEmpty else { return }
        guard
            lengths.count == 1,
            lengths[0] == String(expectedBytes)
        else {
            throw BootleLanternIssuanceClientErrorV1.invalidResponse(
                "\(operation) response Content-Length must be canonical and exact"
            )
        }
    }

    private func headerValues(_ response: HTTPURLResponse, name: String) -> [String] {
        response.allHeaderFields.compactMap { key, value in
            guard String(describing: key).caseInsensitiveCompare(name) == .orderedSame else {
                return nil
            }
            return String(describing: value)
        }
    }

}
