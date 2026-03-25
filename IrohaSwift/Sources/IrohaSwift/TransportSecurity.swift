import Foundation

enum IrohaTransportSecurity {
    private static let credentialHeaders: Set<String> = [
        "authorization",
        "x-api-token",
        "x-iroha-account",
        "x-iroha-signature",
        "x-iroha-timestamp-ms",
        "x-iroha-nonce"
    ]

    static func httpViolation(context: String,
                              baseURL: URL,
                              targetURL: URL,
                              headers: [String: String],
                              body: Data?) -> String? {
        let sensitive = containsCredentialHeaders(headers) || bodyContainsPrivateKey(body)
        guard sensitive else { return nil }

        let targetScheme = normalizedScheme(targetURL)
        guard targetScheme == "https" else {
            return "\(context) refuses insecure transport over \(renderedScheme(targetURL)); use https."
        }

        guard normalizedScheme(baseURL) == targetScheme else {
            return "\(context) refuses sensitive requests over mismatched scheme \(renderedScheme(targetURL)); use relative paths derived from the configured base URL."
        }

        guard sameAuthority(baseURL, targetURL, defaultPortScheme: "https") else {
            let host = targetURL.host ?? targetURL.absoluteString
            return "\(context) refuses sensitive requests to mismatched host \(host); use relative paths on the configured base URL."
        }

        return nil
    }

    static func webSocketViolation(context: String,
                                   baseURL: URL,
                                   targetURL: URL,
                                   hasCredentials: Bool) -> String? {
        guard hasCredentials else { return nil }

        let expectedScheme = expectedWebSocketScheme(for: baseURL)
        let targetScheme = normalizedScheme(targetURL)
        guard targetScheme == expectedScheme else {
            return "\(context) refuses credentialed WebSocket requests over mismatched scheme \(renderedScheme(targetURL)); use \(expectedScheme) URLs derived from the configured base URL."
        }

        guard sameAuthority(baseURL, targetURL, defaultPortScheme: expectedScheme) else {
            let host = targetURL.host ?? targetURL.absoluteString
            return "\(context) refuses credentialed WebSocket requests to mismatched host \(host); use the configured base URL host."
        }

        guard targetScheme == "wss" else {
            return "\(context) refuses insecure WebSocket protocol \(renderedScheme(targetURL)); use wss."
        }

        return nil
    }

    private static func containsCredentialHeaders(_ headers: [String: String]) -> Bool {
        headers.keys.contains { credentialHeaders.contains($0.lowercased()) }
    }

    private static func bodyContainsPrivateKey(_ body: Data?) -> Bool {
        guard let body, !body.isEmpty else { return false }
        guard let rendered = String(data: body, encoding: .utf8)?.lowercased() else { return false }
        return rendered.contains("\"private_key\"")
    }

    private static func expectedWebSocketScheme(for baseURL: URL) -> String {
        switch normalizedScheme(baseURL) {
        case "https", "wss":
            return "wss"
        default:
            return "ws"
        }
    }

    private static func normalizedScheme(_ url: URL) -> String {
        (url.scheme ?? "").lowercased()
    }

    private static func renderedScheme(_ url: URL) -> String {
        let scheme = normalizedScheme(url)
        return scheme.isEmpty ? "unknown" : scheme
    }

    private static func sameAuthority(_ lhs: URL,
                                      _ rhs: URL,
                                      defaultPortScheme: String) -> Bool {
        let lhsHost = lhs.host?.lowercased()
        let rhsHost = rhs.host?.lowercased()
        guard lhsHost == rhsHost else { return false }
        return effectivePort(lhs, defaultPortScheme: defaultPortScheme)
            == effectivePort(rhs, defaultPortScheme: defaultPortScheme)
    }

    private static func effectivePort(_ url: URL, defaultPortScheme: String) -> Int {
        if let port = url.port {
            return port
        }
        switch normalizedScheme(url).isEmpty ? defaultPortScheme : normalizedScheme(url) {
        case "http", "ws":
            return 80
        case "https", "wss":
            return 443
        default:
            return -1
        }
    }
}
