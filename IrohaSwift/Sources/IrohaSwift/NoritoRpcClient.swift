import Foundation

/// Error returned when a Norito RPC call fails with a non-success HTTP status.
public struct NoritoRpcError: Error, Sendable {
    /// HTTP status code returned by Torii.
    public let statusCode: Int
    /// Response body rendered as UTF-8 (or a fallback message when decoding failed).
    public let body: String
}

/// Errors emitted by the `NoritoRpcClient` prior to receiving an HTTP response.
public enum NoritoRpcClientError: Error {
    /// The provided request path could not be resolved into a valid URL.
    case invalidURL(String)
    /// The HTTP method was empty or contained only whitespace.
    case invalidMethod
}

/// Thin HTTP helper for the Torii Norito-RPC surface.
///
/// The client mirrors the JavaScript helper used in the roadmap NRPC tracker:
/// callers supply binary Norito payloads, and the helper handles the
/// `application/x-norito` headers, query parameters, and timeout plumbing.
public final class NoritoRpcClient {
    /// Base Torii URL (e.g., `https://torii.dev.sora.net`).
    public let baseURL: URL
    /// `URLSession` used for Norito RPC calls.
    public let session: URLSession
    /// Default headers applied to every request (in addition to the Norito headers).
    public let defaultHeaders: [String: String]
    /// Default timeout (seconds) applied when an individual call does not specify one.
    public let defaultTimeout: TimeInterval?

    public init(baseURL: URL,
                session: URLSession = .shared,
                defaultHeaders: [String: String] = [:],
                timeout: TimeInterval? = nil) {
        self.baseURL = baseURL
        self.session = session
        self.defaultHeaders = NoritoRpcClient.normalizedHeaders(defaultHeaders)
        defaultTimeout = timeout
    }

    /// Invoke a Torii Norito RPC endpoint.
    ///
    /// - Parameters:
    ///   - path: Request path (absolute URLs are accepted; relative paths are resolved against `baseURL`).
    ///   - payload: Norito-encoded bytes.
    ///   - method: HTTP method (defaults to `POST`).
    ///   - headers: Optional per-call header overrides; use `nil` values to remove a header.
    ///   - accept: Override (or remove) the `Accept` header. Defaults to `application/x-norito`.
    ///   - params: Optional query parameters appended to the URL.
    ///   - timeout: Optional per-call timeout overriding the default.
    /// - Returns: Response body bytes.
    @discardableResult
    public func call(path: String,
                     payload: Data,
                     method: String = "POST",
                     headers: [String: String?]? = nil,
                     accept: String? = "application/x-norito",
                     params: [String: String?]? = nil,
                     timeout: TimeInterval? = nil) async throws -> Data {
        let url = try resolveURL(path: path, params: params)
        let methodTrimmed = method.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !methodTrimmed.isEmpty else {
            throw NoritoRpcClientError.invalidMethod
        }

        var request = URLRequest(url: url)
        request.httpMethod = methodTrimmed.uppercased()
        request.httpBody = payload
        request.setValue(String(payload.count), forHTTPHeaderField: "Content-Length")
        if let timeoutInterval = timeout ?? defaultTimeout {
            request.timeoutInterval = timeoutInterval
        }

        var finalHeaders = defaultHeaders
        NoritoRpcClient.setHeader(&finalHeaders, key: "Content-Type", value: "application/x-norito")
        if let accept {
            NoritoRpcClient.setHeader(&finalHeaders, key: "Accept", value: accept)
        } else {
            NoritoRpcClient.setHeader(&finalHeaders, key: "Accept", value: nil)
        }
        if let overrides = headers {
            for (key, value) in overrides {
                NoritoRpcClient.setHeader(&finalHeaders, key: key, value: value)
            }
        }
        request.allHTTPHeaderFields = finalHeaders

        let (data, response) = try await session.data(for: request)
        guard let httpResponse = response as? HTTPURLResponse else {
            throw NoritoRpcError(statusCode: -1, body: "Response was not HTTPURLResponse")
        }
        guard (200..<300).contains(httpResponse.statusCode) else {
            let bodyText = String(data: data, encoding: .utf8) ?? "Unable to read response body"
            throw NoritoRpcError(statusCode: httpResponse.statusCode, body: bodyText)
        }
        return data
    }

    private func resolveURL(path: String, params: [String: String?]?) throws -> URL {
        if let absolute = URL(string: path), absolute.scheme != nil {
            return NoritoRpcClient.appendQuery(params, to: absolute)
        }
        guard var components = URLComponents(url: baseURL, resolvingAgainstBaseURL: false) else {
            throw NoritoRpcClientError.invalidURL(path)
        }
        if path.hasPrefix("/") {
            components.path = path
        } else {
            var normalized = components.path
            if !normalized.hasSuffix("/") {
                normalized.append("/")
            }
            normalized.append(path)
            components.path = normalized
        }
        guard let resolved = components.url else {
            throw NoritoRpcClientError.invalidURL(path)
        }
        return NoritoRpcClient.appendQuery(params, to: resolved)
    }

    private static func appendQuery(_ params: [String: String?]?, to url: URL) -> URL {
        guard let params, !params.isEmpty else {
            return url
        }
        guard var components = URLComponents(url: url, resolvingAgainstBaseURL: false) else {
            return url
        }
        var items = components.queryItems ?? []
        for (key, value) in params {
            guard !key.isEmpty, let value else { continue }
            items.append(URLQueryItem(name: key, value: value))
        }
        if !items.isEmpty {
            components.queryItems = items.sorted { $0.name > $1.name }
        }
        return components.url ?? url
    }

    private static func normalizedHeaders(_ headers: [String: String]) -> [String: String] {
        var normalized: [String: String] = [:]
        for (key, value) in headers {
            setHeader(&normalized, key: key, value: value)
        }
        return normalized
    }

    private static func setHeader(_ headers: inout [String: String], key: String, value: String?) {
        let canonical = canonicalHeaderKey(key)
        let lower = canonical.lowercased()
        if let value {
            headers[canonical] = value
            return
        }
        // Remove any entry matching the header name case-insensitively.
        headers = headers.filter { $0.key.lowercased() != lower }
    }

    private static func canonicalHeaderKey(_ key: String) -> String {
        switch key.lowercased() {
        case "content-type": return "Content-Type"
        case "accept": return "Accept"
        default: return key
        }
    }
}
