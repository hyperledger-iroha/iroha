import Foundation

private final class MusubiV1RejectRedirectDelegate: NSObject, URLSessionTaskDelegate,
    @unchecked Sendable
{
    static let shared = MusubiV1RejectRedirectDelegate()

    func urlSession(
        _ session: URLSession,
        task: URLSessionTask,
        willPerformHTTPRedirection response: HTTPURLResponse,
        newRequest request: URLRequest,
        completionHandler: @escaping (URLRequest?) -> Void
    ) {
        completionHandler(nil)
    }
}

/// Signer-free read-only client for the eleven typed Musubi first-release queries.
public final class MusubiToriiClientV1: @unchecked Sendable {
    public static let exactPackagePath = "/v1/musubi/queries/exact-package"
    public static let exactReleasePath = "/v1/musubi/queries/exact-release"
    public static let resolverIndexPath = "/v1/musubi/queries/resolver-index"
    public static let versionsPath = "/v1/musubi/queries/versions"
    public static let maintainersPath = "/v1/musubi/queries/maintainers"
    public static let archiveLocationsPath = "/v1/musubi/queries/archive-locations"
    public static let archiveRetentionPath = "/v1/musubi/queries/archive-retention"
    public static let aliasPath = "/v1/musubi/queries/alias"
    public static let aliasHistoryPath = "/v1/musubi/queries/alias-history"
    public static let orderedPrefixPath = "/v1/musubi/queries/ordered-prefix"
    public static let searchPath = "/v1/musubi/queries/search"

    private static let requestMaximumBytes = 64 * 1024
    private static let responseMaximumBytes = 8 * 1024 * 1024

    public let baseURL: URL
    public let defaultHeaders: [String: String]
    private let session: URLSession

    public init(
        baseURL: URL,
        session: URLSession = .shared,
        defaultHeaders: [String: String] = [:]
    ) {
        self.baseURL = baseURL.hasDirectoryPath ? baseURL : baseURL.appendingPathComponent("")
        self.session = session
        self.defaultHeaders = defaultHeaders
    }

    /// Fetches one exact structural package record.
    public func findExactPackage(
        _ request: MusubiExactPackageQueryV1
    ) async throws -> MusubiPackageRecordV1 {
        try await post(Self.exactPackagePath, request: request)
    }

    /// Fetches one exact immutable release and its mutable projections.
    public func findExactRelease(
        _ request: MusubiExactReleaseQueryV1
    ) async throws -> MusubiReleaseRecordV1 {
        try await post(Self.exactReleasePath, request: request)
    }

    /// Reads the finalized universal sparse resolver index.
    public func findResolverIndex(
        _ request: MusubiResolverIndexQueryV1
    ) async throws -> MusubiResolverIndexPageV1 {
        try await post(Self.resolverIndexPath, request: request)
    }

    /// Lists exact structured versions for a package.
    public func findVersions(
        _ request: MusubiPackagePageQueryV1
    ) async throws -> MusubiPageV1<MusubiVersionV1> {
        try await post(Self.versionsPath, request: request)
    }

    /// Lists accepted owners/maintainers and pending invitations for a package.
    public func findMaintainers(
        _ request: MusubiPackagePageQueryV1
    ) async throws -> MusubiPageV1<MusubiMaintainerDirectoryEntryV1> {
        try await post(Self.maintainersPath, request: request)
    }

    /// Lists renewable SoraFS locations for an archive.
    public func findArchiveLocations(
        _ request: MusubiArchiveLocationQueryV1
    ) async throws -> MusubiArchiveLocationPageV1 {
        try await post(Self.archiveLocationsPath, request: request)
    }

    /// Classifies a bounded exact archive batch for fail-closed cache retention.
    public func findArchiveRetention(
        _ request: MusubiArchiveRetentionQueryV1
    ) async throws -> MusubiArchiveRetentionPageV1 {
        let page: MusubiArchiveRetentionPageV1 = try await post(
            Self.archiveRetentionPath,
            request: request
        )
        try page.requireMatches(request)
        return page
    }

    /// Resolves one paid permanent global alias.
    public func findAlias(_ request: MusubiAliasQueryV1) async throws -> MusubiAliasRecordV1 {
        try await post(Self.aliasPath, request: request)
    }

    /// Lists immutable history for one permanent global alias.
    public func findAliasHistory(
        _ request: MusubiAliasQueryV1
    ) async throws -> MusubiPageV1<MusubiAliasHistoryEntryV1> {
        try await post(Self.aliasHistoryPath, request: request)
    }

    /// Scans the deterministic public package directory by byte prefix.
    public func findOrderedPrefix(
        _ request: MusubiOrderedPrefixQueryV1
    ) async throws -> MusubiOrderedPrefixPageV1 {
        try await post(Self.orderedPrefixPath, request: request)
    }

    /// Searches the rebuildable finalized-event package metadata projection.
    public func search(_ request: MusubiSearchQueryV1) async throws -> MusubiSearchPageV1 {
        try await post(Self.searchPath, request: request)
    }

    private func post<Request: Encodable, Response: Decodable>(
        _ path: String,
        request payload: Request
    ) async throws -> Response {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let body: Data
        do {
            body = try encoder.encode(payload)
        } catch {
            throw ToriiClientError.invalidPayload(
                "Failed to encode strict Musubi V1 request: \(error.localizedDescription)"
            )
        }
        guard body.count <= Self.requestMaximumBytes else {
            throw ToriiClientError.invalidPayload(
                "Musubi request exceeds the \(Self.requestMaximumBytes)-byte route limit."
            )
        }
        guard let target = URL(string: String(path.dropFirst()), relativeTo: baseURL)?.absoluteURL else {
            throw ToriiClientError.invalidURL(path)
        }
        var request = URLRequest(url: target)
        request.httpMethod = "POST"
        request.httpBody = body
        for (name, value) in defaultHeaders { request.setValue(value, forHTTPHeaderField: name) }
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        if let violation = IrohaTransportSecurity.httpViolation(
            context: "MusubiToriiClientV1",
            baseURL: baseURL,
            targetURL: target,
            headers: request.allHTTPHeaderFields ?? [:],
            body: body
        ) {
            throw ToriiClientError.invalidPayload(violation)
        }

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(
                for: request,
                delegate: MusubiV1RejectRedirectDelegate.shared
            )
        } catch {
            if error is CancellationError { throw CancellationError() }
            throw ToriiClientError.transport(error)
        }
        guard let http = response as? HTTPURLResponse else {
            throw ToriiClientError.invalidResponse
        }
        guard (200..<300).contains(http.statusCode) else {
            let message = String(data: data.prefix(4 * 1024), encoding: .utf8)
            throw ToriiClientError.httpStatus(
                code: http.statusCode,
                message: message,
                rejectCode: http.value(forHTTPHeaderField: "x-iroha-reject-code")
            )
        }
        guard data.count <= Self.responseMaximumBytes else {
            throw ToriiClientError.invalidPayload(
                "Musubi response exceeds the \(Self.responseMaximumBytes)-byte client limit."
            )
        }
        guard let contentType = http.value(forHTTPHeaderField: "Content-Type")?
            .split(separator: ";", maxSplits: 1).first?
            .trimmingCharacters(in: .whitespacesAndNewlines),
            contentType.caseInsensitiveCompare("application/json") == .orderedSame else {
            throw ToriiClientError.invalidPayload("Musubi response must use application/json.")
        }
        do {
            return try JSONDecoder().decode(Response.self, from: data)
        } catch {
            throw ToriiClientError.decoding(error)
        }
    }
}
