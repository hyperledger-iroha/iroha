import Foundation

private final class SorafsReputationRejectRedirectDelegate: NSObject,
    URLSessionTaskDelegate,
    @unchecked Sendable
{
    static let shared = SorafsReputationRejectRedirectDelegate()

    func urlSession(
        _: URLSession,
        task _: URLSessionTask,
        willPerformHTTPRedirection _: HTTPURLResponse,
        newRequest _: URLRequest,
        completionHandler: @escaping (URLRequest?) -> Void
    ) {
        completionHandler(nil)
    }
}

/// Errors emitted by the authenticated, schema-closed SoraFS reputation client.
public enum SorafsReputationClientError: Error, Equatable, Sendable {
    case invalidConfiguration(String)
    case invalidRequest(String)
    case invalidResponse(String)
    case responseTooLarge(maximumBytes: Int)
    case httpStatus(Int)
    case transport(String)
}

extension SorafsReputationClientError: LocalizedError {
    public var errorDescription: String? {
        switch self {
        case let .invalidConfiguration(message),
             let .invalidRequest(message),
             let .invalidResponse(message):
            message
        case let .responseTooLarge(maximumBytes):
            "SoraFS reputation response exceeded \(maximumBytes) bytes."
        case let .httpStatus(status):
            "SoraFS reputation endpoint returned HTTP \(status)."
        case let .transport(message):
            "SoraFS reputation transport failed: \(message)"
        }
    }
}

/// Canonical V1 degradation flag values, in their required wire order.
public enum SorafsReputationDegradationFlagNameV1: String, CaseIterable, Sendable {
    case reserveWarning = "reserve_warning"
    case reserveGrace = "reserve_grace"
    case reserveDelinquent = "reserve_delinquent"
    case reserveDefault = "reserve_default"
    case proofSuccessBelow90 = "proof_success_below90"
    case proofSuccessBelow80 = "proof_success_below80"
    case activeDispute = "active_dispute"
    case slashingEvent = "slashing_event"
    case lowScore = "low_score"
}

/// One unit-valued V1 degradation flag.
public struct SorafsReputationDegradationFlagV1: Equatable, Sendable {
    public let flag: SorafsReputationDegradationFlagNameV1
}

/// The seven canonical V1 reputation weights.
public struct SorafsReputationWeightsV1: Equatable, Sendable {
    public let version: UInt8
    public let porSuccessBps: UInt16
    public let pdpSuccessBps: UInt16
    public let potrSuccessBps: UInt16
    public let latencyBps: UInt16
    public let disputeBps: UInt16
    public let tokenViolationBps: UInt16
    public let repairBreachBps: UInt16
}

/// Raw bounded metrics committed for one provider.
public struct SorafsReputationProviderMetricsV1: Equatable, Sendable {
    public let version: UInt8
    public let porSuccessBps: UInt16
    public let pdpSuccessBps: UInt16
    public let potrSuccessBps: UInt16
    public let latencyHealthBps: UInt16
    public let disputeRateBps: UInt16
    public let tokenViolationRateBps: UInt16
    public let repairBreachRateBps: UInt16
}

/// One provider row in a committed reputation snapshot.
public struct SorafsReputationProviderV1: Equatable, Sendable {
    public let providerId: String
    public let scoreBps: UInt16
    public let degradationFlags: [SorafsReputationDegradationFlagV1]
    public let rawMetrics: SorafsReputationProviderMetricsV1
    public let rawMetricsHashHex: String
}

/// A bounded, immutable reputation snapshot projection.
public struct SorafsReputationSnapshotSummaryV1: Equatable, Sendable {
    public let snapshotIdHex: String
    public let generatedAtUnix: UInt64
    public let previousSnapshotIdHex: String?
    public let merkleRootHex: String
    public let providerCount: UInt64
    public let returnedProviderCount: UInt64
    public let limit: UInt64
    public let truncatedProviders: Bool
    public let alphaBps: UInt16
    public let currentScoreWeightBps: UInt16
    public let weights: SorafsReputationWeightsV1
    public let providers: [SorafsReputationProviderV1]
}

/// Complete inclusion proof for one provider row.
public struct SorafsReputationMerkleProofV1: Equatable, Sendable {
    public let providerId: String
    public let leafIndex: UInt32
    public let leafCount: UInt32
    public let siblingsHex: [String]
}

/// Provider row and inclusion proof returned by the provider route.
public struct SorafsReputationProviderResponseV1: Equatable, Sendable {
    public let snapshotIdHex: String
    public let generatedAtUnix: UInt64
    public let merkleRootHex: String
    public let provider: SorafsReputationProviderV1
    public let proof: SorafsReputationMerkleProofV1
}

/// Active weights returned by the weights route.
public struct SorafsReputationWeightsResponseV1: Equatable, Sendable {
    public let snapshotIdHex: String
    public let generatedAtUnix: UInt64
    public let alphaBps: UInt16
    public let currentScoreWeightBps: UInt16
    public let weights: SorafsReputationWeightsV1
}

/// One committed reputation-snapshot publication event.
public struct SorafsReputationSnapshotEventV1: Equatable, Sendable {
    public let version: UInt8
    public let sequence: UInt64
    public let snapshotIdHex: String
    public let generatedAtUnix: UInt64
    public let merkleRootHex: String
    public let providerCount: UInt32
    public let previousSnapshotIdHex: String?
}

/// One bounded page of committed reputation events.
public struct SorafsReputationEventsResponseV1: Equatable, Sendable {
    public let since: UInt64?
    public let limit: UInt64
    public let count: UInt64
    public let nextSince: UInt64?
    public let events: [SorafsReputationSnapshotEventV1]
}

/// A schema-validated server-sent reputation event.
public enum SorafsReputationSSEFrameV1: Equatable, Sendable {
    case snapshot(id: UInt64, event: SorafsReputationSnapshotEventV1)
    case lagged(skipped: UInt64)
}

/// Authenticated hard-cut V1 reads for the SoraFS reputation projection.
///
/// Every operation creates exactly one empty-body GET, rejects redirects, and
/// signs the exact path and canonical query with a fresh nonce. The client does
/// not retry or reconnect. In particular, SSE resume is performed only by
/// starting a new stream with a newly signed `since` request.
@available(iOS 15.0, macOS 12.0, *)
public final class SorafsReputationClient: @unchecked Sendable {
    public static let defaultMaximumResponseBytes = 4 * 1024 * 1024
    public static let maximumSSEFrameBytes = 64 * 1024

    private let baseURL: URL
    private let session: URLSession
    private let networkId: NetworkId
    private let accountId: String
    private let privateKey: Data
    private let maximumResponseBytes: Int
    private let currentTimeMilliseconds: @Sendable () -> UInt64
    private let nonceSeed: @Sendable () -> String
    private let nonceLock = NSLock()
    private var nonceSequence: UInt64 = 0

    public init(
        baseURL: URL,
        session: URLSession = .shared,
        networkId: NetworkId,
        accountId: String,
        privateKey: Data,
        maximumResponseBytes: Int = SorafsReputationClient.defaultMaximumResponseBytes,
        currentTimeMilliseconds: @escaping @Sendable () -> UInt64 = {
            UInt64(max(0, Date().timeIntervalSince1970 * 1000).rounded())
        },
        nonceSeed: @escaping @Sendable () -> String = {
            UUID().uuidString.replacingOccurrences(of: "-", with: "")
        }
    ) throws {
        guard maximumResponseBytes > 0 else {
            throw SorafsReputationClientError.invalidConfiguration(
                "maximumResponseBytes must be positive."
            )
        }
        guard var components = URLComponents(
            url: baseURL,
            resolvingAgainstBaseURL: true
        ),
            let scheme = components.scheme?.lowercased(),
            scheme == "https",
            components.host != nil,
            components.user == nil,
            components.password == nil,
            components.query == nil,
            components.fragment == nil
        else {
            throw SorafsReputationClientError.invalidConfiguration(
                "baseURL must be an absolute HTTPS Torii root without credentials, query, or fragment."
            )
        }
        let forbiddenSessionHeaders = Set([
            "last-event-id",
            ToriiCanonicalRequest.headerAccount.lowercased(),
            ToriiCanonicalRequest.headerSignature.lowercased(),
            ToriiCanonicalRequest.headerTimestampMs.lowercased(),
            ToriiCanonicalRequest.headerNonce.lowercased(),
        ])
        let configuredSessionHeaders = session.configuration.httpAdditionalHeaders?.keys
            .compactMap { ($0 as? String)?.lowercased() } ?? []
        guard configuredSessionHeaders.allSatisfy({
            !forbiddenSessionHeaders.contains($0)
        }) else {
            throw SorafsReputationClientError.invalidConfiguration(
                "URLSession must not inject reputation authentication or Last-Event-ID headers."
            )
        }
        while components.path.count > 1, components.path.hasSuffix("/") {
            components.path.removeLast()
        }
        guard let normalizedBaseURL = components.url else {
            throw SorafsReputationClientError.invalidConfiguration(
                "baseURL could not be normalized."
            )
        }
        do {
            guard !accountId.isEmpty,
                  accountId.utf8.elementsEqual(
                      accountId.trimmingCharacters(in: .whitespacesAndNewlines).utf8
                  ),
                  !accountId.contains("@"),
                  !accountId.contains("#"),
                  !accountId.contains("$")
            else {
                throw AccountAddressError.unsupportedAddressFormat
            }
            let prefix = try AccountAddress.inspectI105NetworkPrefix(accountId)
            let parsed = try AccountAddress.fromI105(
                accountId,
                expectedPrefix: prefix.chainDiscriminant
            )
            guard try parsed.toI105(networkPrefix: prefix.chainDiscriminant) == accountId else {
                throw AccountAddressError.unsupportedAddressFormat
            }
            let signingKey = try SigningKey.ed25519(privateKey: privateKey)
            let signingPublicKey = try signingKey.publicKey()
            guard let controller = parsed.singleControllerInfo(),
                  controller.algorithm == .ed25519,
                  controller.publicKey == signingPublicKey
            else {
                throw AccountAddressError.unsupportedAddressFormat
            }
        } catch {
            throw SorafsReputationClientError.invalidConfiguration(
                "accountId and privateKey must be a matching exact canonical single-key I105 account and Ed25519 key."
            )
        }

        self.baseURL = normalizedBaseURL
        self.session = session
        self.networkId = networkId
        self.accountId = accountId
        self.privateKey = privateKey
        self.maximumResponseBytes = maximumResponseBytes
        self.currentTimeMilliseconds = currentTimeMilliseconds
        self.nonceSeed = nonceSeed
    }

    /// Fetch the latest committed snapshot with an exact provider limit.
    public func latest(
        limit: UInt16 = 500
    ) async throws -> SorafsReputationSnapshotSummaryV1 {
        let canonicalLimit = try Self.validateLimit(limit)
        let data = try await finiteGET(
            path: "/v1/sorafs/reputation/latest",
            query: [URLQueryItem(name: "limit", value: String(canonicalLimit))]
        )
        return try SorafsReputationDecoder.snapshot(
            data,
            context: "latest reputation snapshot",
            expectedSnapshotId: nil,
            expectedLimit: UInt64(canonicalLimit)
        )
    }

    /// Fetch one provider and its complete proof from the latest snapshot.
    public func provider(
        providerId: String
    ) async throws -> SorafsReputationProviderResponseV1 {
        let canonicalProviderId = try SorafsReputationDecoder.providerId(
            providerId,
            context: "providerId",
            forRequest: true
        )
        let data = try await finiteGET(
            path: "/v1/sorafs/reputation/providers/\(canonicalProviderId)",
            query: []
        )
        return try SorafsReputationDecoder.providerResponse(
            data,
            expectedProviderId: canonicalProviderId
        )
    }

    /// Fetch one exact retained snapshot with an exact provider limit.
    public func snapshot(
        snapshotIdHex: String,
        limit: UInt16 = 500
    ) async throws -> SorafsReputationSnapshotSummaryV1 {
        let canonicalSnapshotId = try SorafsReputationDecoder.snapshotId(
            snapshotIdHex,
            context: "snapshotIdHex",
            forRequest: true
        )
        let canonicalLimit = try Self.validateLimit(limit)
        let data = try await finiteGET(
            path: "/v1/sorafs/reputation/snapshots/\(canonicalSnapshotId)",
            query: [URLQueryItem(name: "limit", value: String(canonicalLimit))]
        )
        return try SorafsReputationDecoder.snapshot(
            data,
            context: "historical reputation snapshot",
            expectedSnapshotId: canonicalSnapshotId,
            expectedLimit: UInt64(canonicalLimit)
        )
    }

    /// Fetch the weights bound to the latest committed snapshot.
    public func weights() async throws -> SorafsReputationWeightsResponseV1 {
        let data = try await finiteGET(
            path: "/v1/sorafs/reputation/weights",
            query: []
        )
        return try SorafsReputationDecoder.weightsResponse(data)
    }

    /// Fetch a bounded page of committed publication events.
    public func events(
        since: UInt64? = nil,
        limit: UInt16 = 500
    ) async throws -> SorafsReputationEventsResponseV1 {
        let canonicalLimit = try Self.validateLimit(limit)
        var query: [URLQueryItem] = []
        if let since {
            query.append(URLQueryItem(name: "since", value: String(since)))
        }
        query.append(URLQueryItem(name: "limit", value: String(canonicalLimit)))
        let data = try await finiteGET(
            path: "/v1/sorafs/reputation/events",
            query: query
        )
        return try SorafsReputationDecoder.eventsResponse(
            data,
            expectedSince: since,
            expectedLimit: UInt64(canonicalLimit)
        )
    }

    /// Open one authenticated SSE request.
    ///
    /// This stream never sends `Last-Event-ID`, retries, or reconnects. A caller
    /// that resumes must invoke this method again with the last accepted
    /// sequence, producing a new timestamp, nonce, and signature.
    public func streamEvents(
        since: UInt64 = 0,
        limit: UInt16 = 500
    ) throws -> AsyncThrowingStream<SorafsReputationSSEFrameV1, Error> {
        let canonicalLimit = try Self.validateLimit(limit)
        let request = try makeSignedGET(
            path: "/v1/sorafs/reputation/events/stream",
            query: [
                URLQueryItem(name: "since", value: String(since)),
                URLQueryItem(name: "limit", value: String(canonicalLimit)),
            ],
            accept: "text/event-stream"
        )

        return AsyncThrowingStream { continuation in
            let task = Task {
                do {
                    let (bytes, response) = try await self.session.bytes(
                        for: request,
                        delegate: SorafsReputationRejectRedirectDelegate.shared
                    )
                    guard let http = response as? HTTPURLResponse else {
                        bytes.task.cancel()
                        throw SorafsReputationClientError.invalidResponse(
                            "SoraFS reputation stream did not return an HTTP response."
                        )
                    }
                    guard http.statusCode == 200 else {
                        bytes.task.cancel()
                        throw SorafsReputationClientError.httpStatus(http.statusCode)
                    }
                    try Self.validateIdentityEncoding(http)
                    try Self.validateContentType(
                        http,
                        expected: "text/event-stream"
                    )

                    var parser = SorafsReputationSSEParser(requestedSince: since)
                    do {
                        for try await byte in bytes {
                            if Task.isCancelled {
                                bytes.task.cancel()
                                throw CancellationError()
                            }
                            if let frame = try parser.consume(byte) {
                                continuation.yield(frame)
                            }
                        }
                        try parser.finish()
                        continuation.finish()
                    } catch {
                        bytes.task.cancel()
                        throw error
                    }
                } catch is CancellationError {
                    continuation.finish()
                } catch let error as SorafsReputationClientError {
                    continuation.finish(throwing: error)
                } catch {
                    continuation.finish(
                        throwing: SorafsReputationClientError.transport(
                            error.localizedDescription
                        )
                    )
                }
            }
            continuation.onTermination = { _ in
                task.cancel()
            }
        }
    }

    private static func validateLimit(_ limit: UInt16) throws -> UInt16 {
        guard (1 ... 500).contains(limit) else {
            throw SorafsReputationClientError.invalidRequest(
                "limit must be in 1...500."
            )
        }
        return limit
    }

    private func finiteGET(
        path: String,
        query: [URLQueryItem]
    ) async throws -> Data {
        let request = try makeSignedGET(
            path: path,
            query: query,
            accept: "application/json"
        )
        do {
            let (bytes, response) = try await session.bytes(
                for: request,
                delegate: SorafsReputationRejectRedirectDelegate.shared
            )
            guard let http = response as? HTTPURLResponse else {
                bytes.task.cancel()
                throw SorafsReputationClientError.invalidResponse(
                    "SoraFS reputation read did not return an HTTP response."
                )
            }
            guard http.statusCode == 200 else {
                bytes.task.cancel()
                throw SorafsReputationClientError.httpStatus(http.statusCode)
            }
            try Self.validateIdentityEncoding(http)
            try Self.validateContentType(http, expected: "application/json")
            let declaredLength = try Self.validatedContentLength(
                http,
                maximumBytes: maximumResponseBytes
            )
            var data = Data()
            data.reserveCapacity(min(declaredLength ?? 64 * 1024, 64 * 1024))
            do {
                for try await byte in bytes {
                    guard data.count < maximumResponseBytes else {
                        bytes.task.cancel()
                        throw SorafsReputationClientError.responseTooLarge(
                            maximumBytes: maximumResponseBytes
                        )
                    }
                    data.append(byte)
                }
            } catch {
                bytes.task.cancel()
                throw error
            }
            if let declaredLength, data.count != declaredLength {
                throw SorafsReputationClientError.invalidResponse(
                    "SoraFS reputation response length did not match Content-Length."
                )
            }
            return data
        } catch let error as SorafsReputationClientError {
            throw error
        } catch is CancellationError {
            throw CancellationError()
        } catch {
            throw SorafsReputationClientError.transport(error.localizedDescription)
        }
    }

    private func makeSignedGET(
        path: String,
        query: [URLQueryItem],
        accept: String
    ) throws -> URLRequest {
        guard path.hasPrefix("/"),
              !path.contains("?"),
              !path.contains("#"),
              var components = URLComponents(
                  url: baseURL,
                  resolvingAgainstBaseURL: true
              )
        else {
            throw SorafsReputationClientError.invalidRequest(
                "reputation route path was not canonical."
            )
        }
        let basePath = components.path == "/" ? "" : components.path
        components.path = basePath + path
        components.queryItems = query.isEmpty ? nil : query
        guard let url = components.url else {
            throw SorafsReputationClientError.invalidRequest(
                "reputation request URL could not be constructed."
            )
        }

        var request = URLRequest(
            url: url,
            cachePolicy: .reloadIgnoringLocalCacheData
        )
        request.httpMethod = "GET"
        request.httpBody = nil
        request.setValue(accept, forHTTPHeaderField: "Accept")
        request.setValue("identity", forHTTPHeaderField: "Accept-Encoding")
        request.setValue("no-store", forHTTPHeaderField: "Cache-Control")
        let nonce = try nextNonce()
        let headers: [String: String]
        do {
            headers = try ToriiCanonicalRequest.buildHeaders(
                method: "GET",
                url: url,
                body: Data(),
                accountId: accountId,
                privateKey: privateKey,
                networkId: networkId,
                timestampMs: currentTimeMilliseconds(),
                nonce: nonce
            )
        } catch {
            throw SorafsReputationClientError.invalidRequest(
                "canonical reputation request signing failed."
            )
        }
        for (name, value) in headers {
            request.setValue(value, forHTTPHeaderField: name)
        }
        return request
    }

    private func nextNonce() throws -> String {
        let seed = nonceSeed()
        guard !seed.isEmpty,
              seed == seed.trimmingCharacters(in: .whitespacesAndNewlines),
              seed.utf8.count <= 64,
              seed.utf8.allSatisfy({ (0x21 ... 0x7E).contains($0) })
        else {
            throw SorafsReputationClientError.invalidRequest(
                "nonce source must return 1...64 exact printable ASCII bytes."
            )
        }
        let sequence: UInt64? = nonceLock.withLock {
            guard nonceSequence < UInt64.max else {
                return nil
            }
            nonceSequence += 1
            return nonceSequence
        }
        guard let sequence else {
            throw SorafsReputationClientError.invalidRequest(
                "reputation request nonce sequence was exhausted."
            )
        }
        return "\(seed)-\(sequence)"
    }

    private static func validateIdentityEncoding(
        _ response: HTTPURLResponse
    ) throws {
        let encoding = response.value(forHTTPHeaderField: "Content-Encoding")?
            .trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
        guard encoding == nil || encoding?.isEmpty == true || encoding == "identity" else {
            throw SorafsReputationClientError.invalidResponse(
                "SoraFS reputation responses must use identity content encoding."
            )
        }
    }

    private static func validateContentType(
        _ response: HTTPURLResponse,
        expected: String
    ) throws {
        guard let raw = response.value(forHTTPHeaderField: "Content-Type") else {
            throw SorafsReputationClientError.invalidResponse(
                "SoraFS reputation response omitted Content-Type."
            )
        }
        let parts = raw.split(separator: ";", omittingEmptySubsequences: false)
        guard parts.count <= 2,
              parts.first?.trimmingCharacters(in: .whitespacesAndNewlines)
              .lowercased() == expected,
              parts.dropFirst().allSatisfy({
                  $0.trimmingCharacters(in: .whitespacesAndNewlines)
                      .lowercased() == "charset=utf-8"
              })
        else {
            throw SorafsReputationClientError.invalidResponse(
                "SoraFS reputation response used an unsupported Content-Type."
            )
        }
    }

    private static func validatedContentLength(
        _ response: HTTPURLResponse,
        maximumBytes: Int
    ) throws -> Int? {
        guard let raw = response.value(forHTTPHeaderField: "Content-Length") else {
            return nil
        }
        let value = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !value.isEmpty,
              value == "0" || (value.first != "0" && value.allSatisfy(\.isNumber)),
              let parsed = UInt64(value)
        else {
            throw SorafsReputationClientError.invalidResponse(
                "SoraFS reputation Content-Length was not canonical."
            )
        }
        guard parsed <= UInt64(maximumBytes) else {
            throw SorafsReputationClientError.responseTooLarge(
                maximumBytes: maximumBytes
            )
        }
        return Int(parsed)
    }
}

private extension NSLock {
    func withLock<T>(_ body: () throws -> T) rethrows -> T {
        lock()
        defer { unlock() }
        return try body()
    }
}

private indirect enum SorafsReputationJSONValue {
    case object([String: SorafsReputationJSONValue])
    case array([SorafsReputationJSONValue])
    case string(String)
    case unsigned(UInt64)
    case bool(Bool)
    case null
}

private struct SorafsReputationStrictJSONParser {
    private let bytes: [UInt8]
    private var index = 0

    init(data: Data) throws {
        guard !data.isEmpty else {
            throw SorafsReputationClientError.invalidResponse(
                "SoraFS reputation JSON body was empty."
            )
        }
        guard !data.starts(with: [0xEF, 0xBB, 0xBF]) else {
            throw SorafsReputationClientError.invalidResponse(
                "SoraFS reputation JSON must not contain a UTF-8 BOM."
            )
        }
        guard String(data: data, encoding: .utf8) != nil else {
            throw SorafsReputationClientError.invalidResponse(
                "SoraFS reputation JSON was not strict UTF-8."
            )
        }
        bytes = Array(data)
    }

    mutating func parse() throws -> SorafsReputationJSONValue {
        skipWhitespace()
        let value = try parseValue()
        skipWhitespace()
        guard index == bytes.count else {
            throw invalid("JSON contained trailing material.")
        }
        return value
    }

    private mutating func parseValue() throws -> SorafsReputationJSONValue {
        guard index < bytes.count else {
            throw invalid("JSON ended before a value.")
        }
        switch bytes[index] {
        case UInt8(ascii: "{"):
            return try parseObject()
        case UInt8(ascii: "["):
            return try parseArray()
        case UInt8(ascii: "\""):
            return try .string(parseString())
        case UInt8(ascii: "t"):
            try consumeLiteral("true")
            return .bool(true)
        case UInt8(ascii: "f"):
            try consumeLiteral("false")
            return .bool(false)
        case UInt8(ascii: "n"):
            try consumeLiteral("null")
            return .null
        case UInt8(ascii: "0") ... UInt8(ascii: "9"):
            return try .unsigned(parseUnsigned())
        default:
            throw invalid(
                "JSON values must use objects, arrays, strings, booleans, null, or canonical unsigned integers."
            )
        }
    }

    private mutating func parseObject() throws -> SorafsReputationJSONValue {
        index += 1
        skipWhitespace()
        var result: [String: SorafsReputationJSONValue] = [:]
        if consume(UInt8(ascii: "}")) {
            return .object(result)
        }
        while true {
            guard index < bytes.count, bytes[index] == UInt8(ascii: "\"") else {
                throw invalid("JSON object keys must be strings.")
            }
            let key = try parseString()
            guard result[key] == nil else {
                throw invalid("JSON object contained duplicate key \(key).")
            }
            skipWhitespace()
            guard consume(UInt8(ascii: ":")) else {
                throw invalid("JSON object key was not followed by a colon.")
            }
            skipWhitespace()
            result[key] = try parseValue()
            skipWhitespace()
            if consume(UInt8(ascii: "}")) {
                return .object(result)
            }
            guard consume(UInt8(ascii: ",")) else {
                throw invalid("JSON object entries were not comma separated.")
            }
            skipWhitespace()
        }
    }

    private mutating func parseArray() throws -> SorafsReputationJSONValue {
        index += 1
        skipWhitespace()
        var result: [SorafsReputationJSONValue] = []
        if consume(UInt8(ascii: "]")) {
            return .array(result)
        }
        while true {
            try result.append(parseValue())
            skipWhitespace()
            if consume(UInt8(ascii: "]")) {
                return .array(result)
            }
            guard consume(UInt8(ascii: ",")) else {
                throw invalid("JSON array entries were not comma separated.")
            }
            skipWhitespace()
        }
    }

    private mutating func parseString() throws -> String {
        let start = index
        index += 1
        var escaped = false
        while index < bytes.count {
            let byte = bytes[index]
            if escaped {
                if byte == UInt8(ascii: "u") {
                    guard index + 4 < bytes.count,
                          bytes[(index + 1) ... (index + 4)].allSatisfy(Self.isHex)
                    else {
                        throw invalid("JSON string contained an invalid Unicode escape.")
                    }
                    index += 5
                } else {
                    guard [
                        UInt8(ascii: "\""),
                        UInt8(ascii: "\\"),
                        UInt8(ascii: "/"),
                        UInt8(ascii: "b"),
                        UInt8(ascii: "f"),
                        UInt8(ascii: "n"),
                        UInt8(ascii: "r"),
                        UInt8(ascii: "t"),
                    ].contains(byte)
                    else {
                        throw invalid("JSON string contained an invalid escape.")
                    }
                    index += 1
                }
                escaped = false
                continue
            }
            if byte == UInt8(ascii: "\\") {
                escaped = true
                index += 1
                continue
            }
            if byte == UInt8(ascii: "\"") {
                index += 1
                let encoded = Data(bytes[start ..< index])
                do {
                    return try JSONDecoder().decode(String.self, from: encoded)
                } catch {
                    throw invalid("JSON string was not canonical UTF-8 JSON.")
                }
            }
            guard byte >= 0x20 else {
                throw invalid("JSON string contained an unescaped control byte.")
            }
            index += 1
        }
        throw invalid("JSON string was unterminated.")
    }

    private mutating func parseUnsigned() throws -> UInt64 {
        let start = index
        if bytes[index] == UInt8(ascii: "0") {
            index += 1
            if index < bytes.count,
               (UInt8(ascii: "0") ... UInt8(ascii: "9")).contains(bytes[index])
            {
                throw invalid("JSON integer contained a leading zero.")
            }
        } else {
            while index < bytes.count,
                  (UInt8(ascii: "0") ... UInt8(ascii: "9")).contains(bytes[index])
            {
                index += 1
            }
        }
        let text = String(decoding: bytes[start ..< index], as: UTF8.self)
        guard let value = UInt64(text) else {
            throw invalid("JSON integer exceeded UInt64.")
        }
        return value
    }

    private mutating func consumeLiteral(_ literal: StaticString) throws {
        let expected = Array(String(describing: literal).utf8)
        guard index + expected.count <= bytes.count,
              Array(bytes[index ..< (index + expected.count)]) == expected
        else {
            throw invalid("JSON contained an unsupported literal.")
        }
        index += expected.count
    }

    private mutating func skipWhitespace() {
        while index < bytes.count,
              [0x20, 0x09, 0x0A, 0x0D].contains(bytes[index])
        {
            index += 1
        }
    }

    private mutating func consume(_ byte: UInt8) -> Bool {
        guard index < bytes.count, bytes[index] == byte else {
            return false
        }
        index += 1
        return true
    }

    private static func isHex(_ byte: UInt8) -> Bool {
        (UInt8(ascii: "0") ... UInt8(ascii: "9")).contains(byte)
            || (UInt8(ascii: "a") ... UInt8(ascii: "f")).contains(byte)
            || (UInt8(ascii: "A") ... UInt8(ascii: "F")).contains(byte)
    }

    private func invalid(_ message: String) -> SorafsReputationClientError {
        .invalidResponse("SoraFS reputation \(message)")
    }
}

private enum SorafsReputationDecoder {
    private static let snapshotFields: Set<String> = [
        "snapshot_id_hex",
        "generated_at_unix",
        "previous_snapshot_id_hex",
        "merkle_root_hex",
        "provider_count",
        "returned_provider_count",
        "limit",
        "truncated_providers",
        "alpha_bps",
        "current_score_weight_bps",
        "weights",
        "providers",
    ]
    private static let providerResponseFields: Set<String> = [
        "snapshot_id_hex",
        "generated_at_unix",
        "merkle_root_hex",
        "provider",
        "proof",
    ]
    private static let weightsResponseFields: Set<String> = [
        "snapshot_id_hex",
        "generated_at_unix",
        "alpha_bps",
        "current_score_weight_bps",
        "weights",
    ]
    private static let weightsFields: Set<String> = [
        "version",
        "por_success_bps",
        "pdp_success_bps",
        "potr_success_bps",
        "latency_bps",
        "dispute_bps",
        "token_violation_bps",
        "repair_breach_bps",
    ]
    private static let providerFields: Set<String> = [
        "provider_id",
        "score_bps",
        "degradation_flags",
        "raw_metrics",
        "raw_metrics_hash_hex",
    ]
    private static let metricsFields: Set<String> = [
        "version",
        "por_success_bps",
        "pdp_success_bps",
        "potr_success_bps",
        "latency_health_bps",
        "dispute_rate_bps",
        "token_violation_rate_bps",
        "repair_breach_rate_bps",
    ]
    private static let flagFields: Set<String> = ["flag", "value"]
    private static let proofFields: Set<String> = [
        "provider_id",
        "leaf_index",
        "leaf_count",
        "siblings_hex",
    ]
    private static let eventFields: Set<String> = [
        "version",
        "sequence",
        "snapshot_id_hex",
        "generated_at_unix",
        "merkle_root_hex",
        "provider_count",
        "previous_snapshot_id_hex",
    ]
    private static let eventsResponseFields: Set<String> = [
        "since",
        "limit",
        "count",
        "next_since",
        "events",
    ]

    static func snapshot(
        _ data: Data,
        context: String,
        expectedSnapshotId: String?,
        expectedLimit: UInt64
    ) throws -> SorafsReputationSnapshotSummaryV1 {
        try snapshot(
            parse(data),
            context: context,
            expectedSnapshotId: expectedSnapshotId,
            expectedLimit: expectedLimit
        )
    }

    static func providerResponse(
        _ data: Data,
        expectedProviderId: String
    ) throws -> SorafsReputationProviderResponseV1 {
        let context = "provider reputation response"
        let object = try exactObject(
            parse(data),
            fields: providerResponseFields,
            context: context
        )
        let returnedProvider = try provider(
            required(object, "provider", context),
            context: "\(context).provider"
        )
        let returnedProof = try proof(
            required(object, "proof", context),
            context: "\(context).proof"
        )
        guard returnedProvider.providerId == expectedProviderId,
              returnedProof.providerId == expectedProviderId
        else {
            throw invalid("\(context) did not bind the requested provider.")
        }
        return try SorafsReputationProviderResponseV1(
            snapshotIdHex: snapshotId(
                string(required(object, "snapshot_id_hex", context), context),
                context: "\(context).snapshot_id_hex"
            ),
            generatedAtUnix: unsigned(
                required(object, "generated_at_unix", context),
                range: 1 ... UInt64.max,
                context: "\(context).generated_at_unix"
            ),
            merkleRootHex: digest(
                string(required(object, "merkle_root_hex", context), context),
                context: "\(context).merkle_root_hex"
            ),
            provider: returnedProvider,
            proof: returnedProof
        )
    }

    static func weightsResponse(
        _ data: Data
    ) throws -> SorafsReputationWeightsResponseV1 {
        let context = "reputation weights response"
        let object = try exactObject(
            parse(data),
            fields: weightsResponseFields,
            context: context
        )
        return try SorafsReputationWeightsResponseV1(
            snapshotIdHex: snapshotId(
                string(required(object, "snapshot_id_hex", context), context),
                context: "\(context).snapshot_id_hex"
            ),
            generatedAtUnix: unsigned(
                required(object, "generated_at_unix", context),
                range: 1 ... UInt64.max,
                context: "\(context).generated_at_unix"
            ),
            alphaBps: UInt16(
                exactUnsigned(
                    required(object, "alpha_bps", context),
                    expected: 8500,
                    context: "\(context).alpha_bps"
                )
            ),
            currentScoreWeightBps: UInt16(
                exactUnsigned(
                    required(object, "current_score_weight_bps", context),
                    expected: 7000,
                    context: "\(context).current_score_weight_bps"
                )
            ),
            weights: weights(
                required(object, "weights", context),
                context: "\(context).weights"
            )
        )
    }

    static func eventsResponse(
        _ data: Data,
        expectedSince: UInt64?,
        expectedLimit: UInt64
    ) throws -> SorafsReputationEventsResponseV1 {
        let context = "reputation events response"
        let object = try exactObject(
            parse(data),
            fields: eventsResponseFields,
            context: context
        )
        let since = try optionalUnsigned(
            required(object, "since", context),
            range: 0 ... UInt64.max,
            context: "\(context).since"
        )
        guard since == expectedSince else {
            throw invalid("\(context).since did not bind the request.")
        }
        let limit = try unsigned(
            required(object, "limit", context),
            range: 1 ... 500,
            context: "\(context).limit"
        )
        guard limit == expectedLimit else {
            throw invalid("\(context).limit did not bind the request.")
        }
        let values = try array(
            required(object, "events", context),
            context: "\(context).events"
        )
        guard values.count <= 500 else {
            throw invalid("\(context).events exceeded 500 entries.")
        }
        let events = try values.enumerated().map { index, value in
            try event(value, context: "\(context).events[\(index)]")
        }
        let count = try unsigned(
            required(object, "count", context),
            range: 0 ... 500,
            context: "\(context).count"
        )
        guard count == UInt64(events.count), count <= limit else {
            throw invalid("\(context).count was inconsistent.")
        }
        let nextSince = try optionalUnsigned(
            required(object, "next_since", context),
            range: 1 ... UInt64.max,
            context: "\(context).next_since"
        )
        guard nextSince == events.last?.sequence else {
            throw invalid("\(context).next_since did not equal the final sequence.")
        }
        var previousSequence = since ?? 0
        for (index, current) in events.enumerated() {
            if index == 0 {
                guard current.sequence > previousSequence else {
                    throw invalid("\(context) first event did not follow since.")
                }
            } else {
                guard previousSequence != UInt64.max,
                      current.sequence == previousSequence + 1
                else {
                    throw invalid("\(context) event sequences were not contiguous.")
                }
                let previous = events[index - 1]
                guard current.previousSnapshotIdHex == previous.snapshotIdHex,
                      current.generatedAtUnix > previous.generatedAtUnix
                else {
                    throw invalid("\(context) event chain was not canonical.")
                }
            }
            previousSequence = current.sequence
        }
        return SorafsReputationEventsResponseV1(
            since: since,
            limit: limit,
            count: count,
            nextSince: nextSince,
            events: events
        )
    }

    static func event(
        _ data: Data,
        context: String
    ) throws -> SorafsReputationSnapshotEventV1 {
        try event(parse(data), context: context)
    }

    static func providerId(
        _ value: String,
        context: String,
        forRequest: Bool = false
    ) throws -> String {
        guard (1 ... 256).contains(value.utf8.count),
              value != ".",
              value != "..",
              value.utf8.allSatisfy({
                  (UInt8(ascii: "A") ... UInt8(ascii: "Z")).contains($0)
                      || (UInt8(ascii: "a") ... UInt8(ascii: "z")).contains($0)
                      || (UInt8(ascii: "0") ... UInt8(ascii: "9")).contains($0)
                      || [UInt8(ascii: "_"), UInt8(ascii: "."), UInt8(ascii: ":"),
                          UInt8(ascii: "-")].contains($0)
              })
        else {
            let message =
                "\(context) must be 1...256 literal [A-Za-z0-9_.:-] bytes and not a dot segment."
            throw forRequest
                ? SorafsReputationClientError.invalidRequest(message)
                : invalid(message)
        }
        return value
    }

    static func snapshotId(
        _ value: String,
        context: String,
        forRequest: Bool = false
    ) throws -> String {
        guard value.utf8.count == 32,
              value != String(repeating: "0", count: 32),
              value.utf8.allSatisfy({
                  (UInt8(ascii: "0") ... UInt8(ascii: "9")).contains($0)
                      || (UInt8(ascii: "a") ... UInt8(ascii: "f")).contains($0)
              })
        else {
            let message =
                "\(context) must be a non-zero 32-character lowercase hex identifier."
            throw forRequest
                ? SorafsReputationClientError.invalidRequest(message)
                : invalid(message)
        }
        return value
    }

    private static func parse(_ data: Data) throws -> SorafsReputationJSONValue {
        var parser = try SorafsReputationStrictJSONParser(data: data)
        return try parser.parse()
    }

    private static func snapshot(
        _ value: SorafsReputationJSONValue,
        context: String,
        expectedSnapshotId: String?,
        expectedLimit: UInt64
    ) throws -> SorafsReputationSnapshotSummaryV1 {
        let object = try exactObject(value, fields: snapshotFields, context: context)
        let currentSnapshotId = try snapshotId(
            string(required(object, "snapshot_id_hex", context), context),
            context: "\(context).snapshot_id_hex"
        )
        if let expectedSnapshotId, currentSnapshotId != expectedSnapshotId {
            throw invalid("\(context) did not bind the requested snapshot.")
        }
        let previousSnapshotId = try optionalSnapshotId(
            required(object, "previous_snapshot_id_hex", context),
            context: "\(context).previous_snapshot_id_hex"
        )
        guard previousSnapshotId != currentSnapshotId else {
            throw invalid("\(context) referenced itself as its predecessor.")
        }
        let providerCount = try unsigned(
            required(object, "provider_count", context),
            range: 1 ... 65536,
            context: "\(context).provider_count"
        )
        let returnedProviderCount = try unsigned(
            required(object, "returned_provider_count", context),
            range: 1 ... 500,
            context: "\(context).returned_provider_count"
        )
        let limit = try unsigned(
            required(object, "limit", context),
            range: 1 ... 500,
            context: "\(context).limit"
        )
        guard limit == expectedLimit else {
            throw invalid("\(context).limit did not bind the request.")
        }
        let providerValues = try array(
            required(object, "providers", context),
            context: "\(context).providers"
        )
        let providers = try providerValues.enumerated().map { index, providerValue in
            try provider(
                providerValue,
                context: "\(context).providers[\(index)]"
            )
        }
        guard UInt64(providers.count) == returnedProviderCount,
              returnedProviderCount == min(providerCount, limit)
        else {
            throw invalid("\(context) provider counts were inconsistent.")
        }
        for index in 1 ..< providers.count {
            guard providers[index - 1].providerId < providers[index].providerId else {
                throw invalid("\(context) providers were not strictly ordered.")
            }
        }
        let truncated = try bool(
            required(object, "truncated_providers", context),
            context: "\(context).truncated_providers"
        )
        guard truncated == (providerCount > returnedProviderCount) else {
            throw invalid("\(context).truncated_providers was inconsistent.")
        }
        return try SorafsReputationSnapshotSummaryV1(
            snapshotIdHex: currentSnapshotId,
            generatedAtUnix: unsigned(
                required(object, "generated_at_unix", context),
                range: 1 ... UInt64.max,
                context: "\(context).generated_at_unix"
            ),
            previousSnapshotIdHex: previousSnapshotId,
            merkleRootHex: digest(
                string(required(object, "merkle_root_hex", context), context),
                context: "\(context).merkle_root_hex"
            ),
            providerCount: providerCount,
            returnedProviderCount: returnedProviderCount,
            limit: limit,
            truncatedProviders: truncated,
            alphaBps: UInt16(
                exactUnsigned(
                    required(object, "alpha_bps", context),
                    expected: 8500,
                    context: "\(context).alpha_bps"
                )
            ),
            currentScoreWeightBps: UInt16(
                exactUnsigned(
                    required(object, "current_score_weight_bps", context),
                    expected: 7000,
                    context: "\(context).current_score_weight_bps"
                )
            ),
            weights: weights(
                required(object, "weights", context),
                context: "\(context).weights"
            ),
            providers: providers
        )
    }

    private static func weights(
        _ value: SorafsReputationJSONValue,
        context: String
    ) throws -> SorafsReputationWeightsV1 {
        let object = try exactObject(value, fields: weightsFields, context: context)
        let values = try [
            "por_success_bps",
            "pdp_success_bps",
            "potr_success_bps",
            "latency_bps",
            "dispute_bps",
            "token_violation_bps",
            "repair_breach_bps",
        ].map { field in
            try unsigned(
                required(object, field, context),
                range: 0 ... 10000,
                context: "\(context).\(field)"
            )
        }
        guard values.reduce(0, +) == 10000 else {
            throw invalid("\(context) weights did not sum to 10000.")
        }
        return try SorafsReputationWeightsV1(
            version: UInt8(
                exactUnsigned(
                    required(object, "version", context),
                    expected: 1,
                    context: "\(context).version"
                )
            ),
            porSuccessBps: UInt16(values[0]),
            pdpSuccessBps: UInt16(values[1]),
            potrSuccessBps: UInt16(values[2]),
            latencyBps: UInt16(values[3]),
            disputeBps: UInt16(values[4]),
            tokenViolationBps: UInt16(values[5]),
            repairBreachBps: UInt16(values[6])
        )
    }

    private static func metrics(
        _ value: SorafsReputationJSONValue,
        context: String
    ) throws -> SorafsReputationProviderMetricsV1 {
        let object = try exactObject(value, fields: metricsFields, context: context)
        func basisPoints(_ field: String) throws -> UInt16 {
            try UInt16(
                unsigned(
                    required(object, field, context),
                    range: 0 ... 10000,
                    context: "\(context).\(field)"
                )
            )
        }
        return try SorafsReputationProviderMetricsV1(
            version: UInt8(
                exactUnsigned(
                    required(object, "version", context),
                    expected: 1,
                    context: "\(context).version"
                )
            ),
            porSuccessBps: basisPoints("por_success_bps"),
            pdpSuccessBps: basisPoints("pdp_success_bps"),
            potrSuccessBps: basisPoints("potr_success_bps"),
            latencyHealthBps: basisPoints("latency_health_bps"),
            disputeRateBps: basisPoints("dispute_rate_bps"),
            tokenViolationRateBps: basisPoints("token_violation_rate_bps"),
            repairBreachRateBps: basisPoints("repair_breach_rate_bps")
        )
    }

    private static func provider(
        _ value: SorafsReputationJSONValue,
        context: String
    ) throws -> SorafsReputationProviderV1 {
        let object = try exactObject(value, fields: providerFields, context: context)
        let flagValues = try array(
            required(object, "degradation_flags", context),
            context: "\(context).degradation_flags"
        )
        guard flagValues.count <= 5 else {
            throw invalid("\(context).degradation_flags exceeded five entries.")
        }
        var previousFlagIndex: Int?
        let flags = try flagValues.enumerated().map { index, value in
            let flagContext = "\(context).degradation_flags[\(index)]"
            let flagObject = try exactObject(
                value,
                fields: flagFields,
                context: flagContext
            )
            guard case .null = try required(flagObject, "value", flagContext) else {
                throw invalid("\(flagContext).value was not null.")
            }
            let raw = try string(
                required(flagObject, "flag", flagContext),
                flagContext
            )
            guard let flag = SorafsReputationDegradationFlagNameV1(rawValue: raw),
                  let flagIndex = SorafsReputationDegradationFlagNameV1
                  .allCases.firstIndex(of: flag),
                  previousFlagIndex.map({ flagIndex > $0 }) ?? true
            else {
                throw invalid("\(context).degradation_flags were not canonical.")
            }
            previousFlagIndex = flagIndex
            return SorafsReputationDegradationFlagV1(flag: flag)
        }
        return try SorafsReputationProviderV1(
            providerId: providerId(
                string(required(object, "provider_id", context), context),
                context: "\(context).provider_id"
            ),
            scoreBps: UInt16(
                unsigned(
                    required(object, "score_bps", context),
                    range: 500 ... 9900,
                    context: "\(context).score_bps"
                )
            ),
            degradationFlags: flags,
            rawMetrics: metrics(
                required(object, "raw_metrics", context),
                context: "\(context).raw_metrics"
            ),
            rawMetricsHashHex: digest(
                string(
                    required(object, "raw_metrics_hash_hex", context),
                    context
                ),
                context: "\(context).raw_metrics_hash_hex"
            )
        )
    }

    private static func proof(
        _ value: SorafsReputationJSONValue,
        context: String
    ) throws -> SorafsReputationMerkleProofV1 {
        let object = try exactObject(value, fields: proofFields, context: context)
        let leafIndex = try unsigned(
            required(object, "leaf_index", context),
            range: 0 ... 65535,
            context: "\(context).leaf_index"
        )
        let leafCount = try unsigned(
            required(object, "leaf_count", context),
            range: 1 ... 65536,
            context: "\(context).leaf_count"
        )
        guard leafIndex < leafCount else {
            throw invalid("\(context).leaf_index was not below leaf_count.")
        }
        let siblingValues = try array(
            required(object, "siblings_hex", context),
            context: "\(context).siblings_hex"
        )
        var width = leafCount
        var depth = 0
        while width > 1 {
            width = (width + 1) / 2
            depth += 1
        }
        guard siblingValues.count == depth else {
            throw invalid("\(context).siblings_hex had the wrong Merkle depth.")
        }
        let siblings = try siblingValues.enumerated().map { index, value in
            try digest(
                string(value, "\(context).siblings_hex[\(index)]"),
                context: "\(context).siblings_hex[\(index)]"
            )
        }
        return try SorafsReputationMerkleProofV1(
            providerId: providerId(
                string(required(object, "provider_id", context), context),
                context: "\(context).provider_id"
            ),
            leafIndex: UInt32(leafIndex),
            leafCount: UInt32(leafCount),
            siblingsHex: siblings
        )
    }

    private static func event(
        _ value: SorafsReputationJSONValue,
        context: String
    ) throws -> SorafsReputationSnapshotEventV1 {
        let object = try exactObject(value, fields: eventFields, context: context)
        let currentSnapshotId = try snapshotId(
            string(required(object, "snapshot_id_hex", context), context),
            context: "\(context).snapshot_id_hex"
        )
        let previousSnapshotId = try optionalSnapshotId(
            required(object, "previous_snapshot_id_hex", context),
            context: "\(context).previous_snapshot_id_hex"
        )
        guard previousSnapshotId != currentSnapshotId else {
            throw invalid("\(context) referenced itself as its predecessor.")
        }
        return try SorafsReputationSnapshotEventV1(
            version: UInt8(
                exactUnsigned(
                    required(object, "version", context),
                    expected: 1,
                    context: "\(context).version"
                )
            ),
            sequence: unsigned(
                required(object, "sequence", context),
                range: 1 ... UInt64.max,
                context: "\(context).sequence"
            ),
            snapshotIdHex: currentSnapshotId,
            generatedAtUnix: unsigned(
                required(object, "generated_at_unix", context),
                range: 1 ... UInt64.max,
                context: "\(context).generated_at_unix"
            ),
            merkleRootHex: digest(
                string(required(object, "merkle_root_hex", context), context),
                context: "\(context).merkle_root_hex"
            ),
            providerCount: UInt32(
                unsigned(
                    required(object, "provider_count", context),
                    range: 1 ... 65536,
                    context: "\(context).provider_count"
                )
            ),
            previousSnapshotIdHex: previousSnapshotId
        )
    }

    private static func exactObject(
        _ value: SorafsReputationJSONValue,
        fields: Set<String>,
        context: String
    ) throws -> [String: SorafsReputationJSONValue] {
        guard case let .object(object) = value, Set(object.keys) == fields else {
            throw invalid("\(context) fields were not schema-closed.")
        }
        return object
    }

    private static func required(
        _ object: [String: SorafsReputationJSONValue],
        _ field: String,
        _ context: String
    ) throws -> SorafsReputationJSONValue {
        guard let value = object[field] else {
            throw invalid("\(context) omitted \(field).")
        }
        return value
    }

    private static func array(
        _ value: SorafsReputationJSONValue,
        context: String
    ) throws -> [SorafsReputationJSONValue] {
        guard case let .array(values) = value else {
            throw invalid("\(context) was not an array.")
        }
        return values
    }

    private static func string(
        _ value: SorafsReputationJSONValue,
        _ context: String
    ) throws -> String {
        guard case let .string(result) = value else {
            throw invalid("\(context) was not a string.")
        }
        return result
    }

    private static func bool(
        _ value: SorafsReputationJSONValue,
        context: String
    ) throws -> Bool {
        guard case let .bool(result) = value else {
            throw invalid("\(context) was not a boolean.")
        }
        return result
    }

    private static func unsigned(
        _ value: SorafsReputationJSONValue,
        range: ClosedRange<UInt64>,
        context: String
    ) throws -> UInt64 {
        guard case let .unsigned(result) = value, range.contains(result) else {
            throw invalid("\(context) was not a canonical integer in range.")
        }
        return result
    }

    private static func exactUnsigned(
        _ value: SorafsReputationJSONValue,
        expected: UInt64,
        context: String
    ) throws -> UInt64 {
        try unsigned(value, range: expected ... expected, context: context)
    }

    private static func optionalUnsigned(
        _ value: SorafsReputationJSONValue,
        range: ClosedRange<UInt64>,
        context: String
    ) throws -> UInt64? {
        if case .null = value {
            return nil
        }
        return try unsigned(value, range: range, context: context)
    }

    private static func optionalSnapshotId(
        _ value: SorafsReputationJSONValue,
        context: String
    ) throws -> String? {
        if case .null = value {
            return nil
        }
        return try snapshotId(string(value, context), context: context)
    }

    private static func digest(_ value: String, context: String) throws -> String {
        guard value.utf8.count == 64,
              value.utf8.allSatisfy({
                  (UInt8(ascii: "0") ... UInt8(ascii: "9")).contains($0)
                      || (UInt8(ascii: "a") ... UInt8(ascii: "f")).contains($0)
              })
        else {
            throw invalid("\(context) was not a lowercase 32-byte digest.")
        }
        return value
    }

    private static func invalid(_ message: String) -> SorafsReputationClientError {
        .invalidResponse("SoraFS reputation \(message)")
    }
}

private struct SorafsReputationSSEParser {
    private let requestedSince: UInt64
    private var line = Data()
    private var frameBytes = 0
    private var eventName: String?
    private var eventId: String?
    private var eventData: String?
    private var lastEvent: SorafsReputationSnapshotEventV1?

    init(requestedSince: UInt64) {
        self.requestedSince = requestedSince
    }

    mutating func consume(
        _ byte: UInt8
    ) throws -> SorafsReputationSSEFrameV1? {
        frameBytes += 1
        guard frameBytes <= SorafsReputationClient.maximumSSEFrameBytes else {
            throw SorafsReputationClientError.responseTooLarge(
                maximumBytes: SorafsReputationClient.maximumSSEFrameBytes
            )
        }
        guard byte != 0 else {
            throw invalid("SSE contained a NUL byte.")
        }
        if byte == UInt8(ascii: "\n") {
            if line.last == UInt8(ascii: "\r") {
                line.removeLast()
            }
            guard let rendered = String(data: line, encoding: .utf8) else {
                throw invalid("SSE line was not strict UTF-8.")
            }
            line.removeAll(keepingCapacity: true)
            if rendered.isEmpty {
                defer { resetFrame() }
                return try finishFrame()
            }
            try consumeLine(rendered)
        } else {
            line.append(byte)
        }
        return nil
    }

    mutating func finish() throws {
        guard line.isEmpty,
              eventName == nil,
              eventId == nil,
              eventData == nil
        else {
            throw invalid("SSE terminated inside a frame.")
        }
    }

    private mutating func consumeLine(_ line: String) throws {
        if line.hasPrefix(":") {
            guard eventName == nil, eventId == nil, eventData == nil else {
                throw invalid("SSE comment interrupted a data frame.")
            }
            frameBytes = 0
            return
        }
        if line.hasPrefix("event: ") {
            guard eventName == nil else {
                throw invalid("SSE frame repeated event.")
            }
            eventName = String(line.dropFirst("event: ".count))
            return
        }
        if line.hasPrefix("id: ") {
            guard eventId == nil else {
                throw invalid("SSE frame repeated id.")
            }
            eventId = String(line.dropFirst("id: ".count))
            return
        }
        if line.hasPrefix("data: ") {
            guard eventData == nil else {
                throw invalid("SSE frame repeated data.")
            }
            eventData = String(line.dropFirst("data: ".count))
            return
        }
        throw invalid("SSE frame contained an unsupported field.")
    }

    private mutating func finishFrame() throws -> SorafsReputationSSEFrameV1? {
        guard eventName != nil || eventId != nil || eventData != nil else {
            return nil
        }
        guard let eventName, let eventData else {
            throw invalid("SSE frame omitted event or data.")
        }
        switch eventName {
        case "reputation_snapshot":
            guard let eventId,
                  Self.isPositiveCanonicalU64(eventId),
                  let parsedId = UInt64(eventId),
                  !eventData.isEmpty,
                  !eventData.utf8.contains(where: {
                      [0x20, 0x09, 0x0A, 0x0D].contains($0)
                  })
            else {
                throw invalid("snapshot SSE frame was not canonical.")
            }
            let event = try SorafsReputationDecoder.event(
                Data(eventData.utf8),
                context: "SSE reputation snapshot"
            )
            guard event.sequence == parsedId,
                  event.sequence > requestedSince
            else {
                throw invalid("snapshot SSE id did not bind its event or request.")
            }
            if let previous = lastEvent {
                guard previous.sequence != UInt64.max,
                      event.sequence == previous.sequence + 1,
                      event.previousSnapshotIdHex == previous.snapshotIdHex,
                      event.generatedAtUnix > previous.generatedAtUnix
                else {
                    throw invalid("snapshot SSE event chain was not canonical.")
                }
            }
            lastEvent = event
            return .snapshot(id: parsedId, event: event)
        case "lagged":
            guard eventId == nil,
                  Self.isPositiveCanonicalU64(eventData),
                  let skipped = UInt64(eventData)
            else {
                throw invalid("lagged SSE frame was not canonical.")
            }
            lastEvent = nil
            return .lagged(skipped: skipped)
        default:
            throw invalid("SSE frame used an unsupported event name.")
        }
    }

    private mutating func resetFrame() {
        frameBytes = 0
        eventName = nil
        eventId = nil
        eventData = nil
    }

    private static func isPositiveCanonicalU64(_ value: String) -> Bool {
        guard let first = value.utf8.first,
              (UInt8(ascii: "1") ... UInt8(ascii: "9")).contains(first),
              value.utf8.count <= 20,
              value.utf8.dropFirst().allSatisfy({
                  (UInt8(ascii: "0") ... UInt8(ascii: "9")).contains($0)
              })
        else {
            return false
        }
        return UInt64(value) != nil
    }

    private func invalid(_ message: String) -> SorafsReputationClientError {
        .invalidResponse("SoraFS reputation \(message)")
    }
}
