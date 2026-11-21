import Foundation

/// Failure while sending an opaque ballot frame or fetching a whole public record.
public enum AnonymousBallotErrorV1: Error, Equatable {
    case invalidInput
    case invalidResponse
    case refused(status: Int, code: String?, retryAfterSeconds: Int?)
}

private final class BallotTransportDelegate: NSObject, URLSessionTaskDelegate, @unchecked Sendable {
    func urlSession(_ session: URLSession, didReceive challenge: URLAuthenticationChallenge,
                    completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void) {
        completionHandler(challenge.protectionSpace.authenticationMethod == NSURLAuthenticationMethodServerTrust
                          ? .performDefaultHandling : .cancelAuthenticationChallenge, nil)
    }

    func urlSession(_ session: URLSession, task: URLSessionTask,
                    willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest,
                    completionHandler: @escaping (URLRequest?) -> Void) {
        completionHandler(nil)
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didReceive challenge: URLAuthenticationChallenge,
                    completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void) {
        urlSession(session, didReceive: challenge, completionHandler: completionHandler)
    }
}

/// Dedicated credential-free transport for anonymous Parliament ballot frames and whole public records.
public final class AnonymousBallotClientV1: Sendable {
    private let root: URL
    private let session: URLSession
    private static let media = "application/x-norito"

    public convenience init(baseURL: URL, allowLocalHTTP: Bool = false) throws {
        try self.init(baseURL: baseURL, allowLocalHTTP: allowLocalHTTP, protocolClasses: [])
    }

    init(baseURL: URL, allowLocalHTTP: Bool, protocolClasses: [AnyClass]) throws {
        guard let url = URLComponents(url: baseURL, resolvingAgainstBaseURL: false),
              let host = url.host?.lowercased(), !host.isEmpty, url.user == nil, url.password == nil,
              url.query == nil, url.fragment == nil else { throw AnonymousBallotErrorV1.invalidInput }
        let local = host == "localhost" || host == "[::1]" || host == "::1" || Self.localIPv4(host)
        guard url.scheme?.lowercased() == "https" || (allowLocalHTTP && local && url.scheme?.lowercased() == "http")
        else { throw AnonymousBallotErrorV1.invalidInput }
        root = baseURL.appendingPathComponent("v1/gov/anonymous-ballots")
        let config = URLSessionConfiguration.ephemeral
        config.httpShouldSetCookies = false
        config.httpCookieStorage = nil
        config.urlCredentialStorage = nil
        config.urlCache = nil
        config.httpAdditionalHeaders = nil
        config.requestCachePolicy = .reloadIgnoringLocalCacheData
        config.timeoutIntervalForRequest = 15
        if !protocolClasses.isEmpty { config.protocolClasses = protocolClasses }
        session = URLSession(configuration: config, delegate: BallotTransportDelegate(), delegateQueue: nil)
    }

    deinit { session.invalidateAndCancel() }

    private static func localIPv4(_ host: String) -> Bool {
        let fields = host.split(separator: ".", omittingEmptySubsequences: false)
        let octets = fields.compactMap { UInt8($0) }
        guard octets.count == 4, fields.count == 4, octets.map(String.init).joined(separator: ".") == host else { return false }
        return octets[0] == 127 || octets[0] == 10 || (octets[0] == 192 && octets[1] == 168) ||
            (octets[0] == 172 && (16...31).contains(octets[1]))
    }

    /// Send one canonical frame produced by the wallet. An empty 202 means the node handled it.
    public func submitAnonymousBallot(_ envelope: Data) async throws {
        guard !envelope.isEmpty, envelope.count <= 262_144 else { throw AnonymousBallotErrorV1.invalidInput }
        _ = try await request(url: root, frame: envelope, limit: 0)
    }

    /// Fetch the entire canonical board for one exact 64-byte scope.
    public func getAnonymousBallotBoard(scope: Data) async throws -> Data {
        try await record("boards", selector: scope, size: 64, limit: 1_048_576)
    }

    /// Fetch the entire canonical attempt for one exact 32-byte attempt identifier.
    public func getAnonymousParliamentAttempt(id: Data) async throws -> Data {
        try await record("attempts", selector: id, size: 32, limit: 16_777_216)
    }

    private func record(_ family: String, selector: Data, size: Int, limit: Int) async throws -> Data {
        guard selector.count == size else { throw AnonymousBallotErrorV1.invalidInput }
        let id = selector.map { String(format: "%02x", $0) }.joined()
        return try await request(url: root.appendingPathComponent(family).appendingPathComponent(id), frame: nil, limit: limit)
    }

    private func request(url: URL, frame: Data?, limit: Int) async throws -> Data {
        var request = URLRequest(url: url)
        request.httpMethod = frame == nil ? "GET" : "POST"
        request.httpBody = frame
        request.httpShouldHandleCookies = false
        request.setValue(Self.media, forHTTPHeaderField: frame == nil ? "Accept" : "Content-Type")
        let (bytes, response) = try await session.bytes(for: request)
        defer { bytes.task.cancel() }
        guard let http = response as? HTTPURLResponse, http.url == url else { throw AnonymousBallotErrorV1.invalidResponse }
        let success = (200..<300).contains(http.statusCode)
        let maximum = success ? limit : 65_536
        func refusal(_ body: Data) -> AnonymousBallotErrorV1 {
            struct Refusal: Decodable { let code: String }
            let raw = try? JSONDecoder().decode(Refusal.self, from: body).code
            let code = raw.flatMap { $0.range(of: "^[A-Za-z0-9_.:-]{1,128}$", options: .regularExpression) == nil ? nil : $0 }
            let retry = http.value(forHTTPHeaderField: "Retry-After").flatMap {
                $0.range(of: "^[0-9]{1,9}$", options: .regularExpression) == nil ? nil : Int($0)
            }
            return .refused(status: http.statusCode, code: code, retryAfterSeconds: retry)
        }
        guard (http.value(forHTTPHeaderField: "Content-Encoding") ?? "identity").lowercased() == "identity",
              http.expectedContentLength <= maximum else {
            throw success ? AnonymousBallotErrorV1.invalidResponse : refusal(Data())
        }
        if success {
            guard http.statusCode == (frame == nil ? 200 : 202),
                  frame != nil || http.value(forHTTPHeaderField: "Content-Type")?.lowercased() == Self.media
            else { throw AnonymousBallotErrorV1.invalidResponse }
        }
        var data = Data()
        if http.expectedContentLength > 0 { data.reserveCapacity(Int(http.expectedContentLength)) }
        do {
            for try await byte in bytes {
                guard data.count < maximum else { throw AnonymousBallotErrorV1.invalidResponse }
                data.append(byte)
            }
        } catch {
            try Task.checkCancellation()
            if !success { throw refusal(Data()) }
            throw error
        }
        if !success {
            throw refusal(data)
        }
        guard frame != nil || !data.isEmpty, http.expectedContentLength < 0 || http.expectedContentLength == data.count
        else { throw AnonymousBallotErrorV1.invalidResponse }
        return data
    }
}
