import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

public enum ToriiOfflineNoteIssuerClientError: Error, LocalizedError, Equatable {
    case invalidURL(String)
    case accountMismatch
    case missingLoadContext(String)
    case invalidJSON(String)
    case httpStatus(Int, String?)
    case commitmentHex(String)
    case invalidBase64(String)
    case retiredOfflineNoteIssue

    public var errorDescription: String? {
        switch self {
        case let .invalidURL(path):
            return "Invalid Offline Note issuer URL: \(path)."
        case .accountMismatch:
            return "Canonical auth accountId must match wallet accountId."
        case let .missingLoadContext(operationId):
            return "Missing Offline Note load context for operation \(operationId)."
        case let .invalidJSON(field):
            return "Offline Note issuer JSON field is invalid: \(field)."
        case let .httpStatus(code, body):
            if let body, !body.isEmpty {
                return "Offline Note issuer request failed with HTTP \(code): \(body)"
            }
            return "Offline Note issuer request failed with HTTP \(code)."
        case let .commitmentHex(value):
            return "Offline Note issuer returned an invalid commitment: \(value)."
        case let .invalidBase64(field):
            return "Offline Note issuer field \(field) must be base64."
        case .retiredOfflineNoteIssue:
            return "Classic Offline Note issue transactions are retired; use Kagemusha online-to-offline top-up flows."
        }
    }
}

public struct OfflineNoteIssuerDeviceBinding {
    public let deviceId: String
    public let offlinePublicKey: String
    private let binding: [String: Any]

    public init(deviceId: String,
                offlinePublicKey: String,
                deviceBinding: [String: Any]) throws {
        let trimmedDeviceId = deviceId.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedPublicKey = offlinePublicKey.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedDeviceId.isEmpty else {
            throw ToriiOfflineNoteIssuerClientError.invalidJSON("device_id")
        }
        guard !trimmedPublicKey.isEmpty else {
            throw ToriiOfflineNoteIssuerClientError.invalidJSON("offline_public_key")
        }
        if let bindingDeviceId = deviceBinding["device_id"] as? String,
           bindingDeviceId != trimmedDeviceId {
            throw ToriiOfflineNoteIssuerClientError.invalidJSON("device_binding.device_id")
        }
        if let bindingPublicKey = deviceBinding["offline_public_key"] as? String,
           bindingPublicKey != trimmedPublicKey {
            throw ToriiOfflineNoteIssuerClientError.invalidJSON("device_binding.offline_public_key")
        }
        self.deviceId = trimmedDeviceId
        self.offlinePublicKey = trimmedPublicKey
        self.binding = try Self.deepCopyObject(deviceBinding)
    }

    public func deviceBinding() throws -> [String: Any] {
        try Self.deepCopyObject(binding)
    }

    public func attestationKeyId() throws -> String {
        guard let value = binding["attestation_key_id"] as? String,
              !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw ToriiOfflineNoteIssuerClientError.invalidJSON("device_binding.attestation_key_id")
        }
        return value.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    fileprivate static func deepCopyObject(_ value: [String: Any]) throws -> [String: Any] {
        var copy: [String: Any] = [:]
        for (key, item) in value {
            copy[key] = try normalizeJSONValue(item)
        }
        return copy
    }

    private static func normalizeJSONValue(_ value: Any) throws -> Any {
        if value is NSNull || value is String || value is NSNumber || value is Bool {
            return value
        }
        if let value = value as? [String: Any] {
            return try deepCopyObject(value)
        }
        if let value = value as? [Any] {
            return try value.map { try normalizeJSONValue($0) }
        }
        if let value = value as? Int {
            return value
        }
        if let value = value as? Int64 {
            return value
        }
        if let value = value as? UInt64 {
            return NSNumber(value: value)
        }
        throw ToriiOfflineNoteIssuerClientError.invalidJSON("device_binding")
    }
}

public protocol OfflineNoteIssuerDeviceBindingProvider {
    func currentDeviceBinding(chainId: String,
                              accountId: String,
                              assetDefinitionId: String) throws -> OfflineNoteIssuerDeviceBinding
}

public final class ToriiOfflineNoteIssuerClient: OfflineNoteIssuerClient {
    private static let keysRefillPath = ToriiOfflineCashAPI.Endpoint.keyRefill.path
    private static let legacyCanonicalAuthHeaders: Set<String> = [
        ToriiCanonicalRequest.headerAccount.lowercased(),
        ToriiCanonicalRequest.headerSignature.lowercased(),
        ToriiCanonicalRequest.headerTimestampMs.lowercased(),
        ToriiCanonicalRequest.headerNonce.lowercased(),
        "x-iroha-witness",
    ]

    private let baseURL: URL
    private let session: URLSession
    private let canonicalAuth: ToriiCanonicalRequestAuth
    private let deviceBindingProvider: OfflineNoteIssuerDeviceBindingProvider
    private let defaultHeaders: [String: String]
    private let clock: () -> UInt64
    private let nonceGenerator: OfflineNoteIdGenerator
    private let lock = NSLock()
    private var pendingLoads: [String: PendingLoad] = [:]
    private var lineageStates: [String: StoredLineageState] = [:]

    public init(baseURL: URL = URL(string: "http://localhost:8080")!,
                session: URLSession = .shared,
                canonicalAuth: ToriiCanonicalRequestAuth,
                deviceBindingProvider: OfflineNoteIssuerDeviceBindingProvider,
                defaultHeaders: [String: String] = [:],
                clock: @escaping () -> UInt64 = {
                    UInt64(Date().timeIntervalSince1970 * 1000)
                },
                nonceGenerator: OfflineNoteIdGenerator = UuidOfflineNoteIdGenerator()) {
        self.baseURL = baseURL
        self.session = session
        self.canonicalAuth = canonicalAuth
        self.deviceBindingProvider = deviceBindingProvider
        self.defaultHeaders = Self.stripLegacyCanonicalAuthHeaders(defaultHeaders)
        self.clock = clock
        self.nonceGenerator = nonceGenerator
    }

    private static func stripLegacyCanonicalAuthHeaders(_ headers: [String: String]) -> [String: String] {
        var filtered: [String: String] = [:]
        for (key, value) in headers where !legacyCanonicalAuthHeaders.contains(key.lowercased()) {
            filtered[key] = value
        }
        return filtered
    }

    public func prepareLoad(chainId: String,
                            accountId: String,
                            assetDefinitionId: String,
                            amount: String) async throws -> OfflineNoteLoadContext {
        guard canonicalAuth.accountId == accountId else {
            throw ToriiOfflineNoteIssuerClientError.accountMismatch
        }
        let binding = try deviceBindingProvider.currentDeviceBinding(
            chainId: chainId,
            accountId: accountId,
            assetDefinitionId: assetDefinitionId
        )
        let key = lineageKey(accountId: accountId, assetDefinitionId: assetDefinitionId, binding: binding)
        if let cached = withLock({ lineageStates[key] }), !cached.isExpired(nowMs: clock()) {
            let operationId = nonceGenerator.nextId(prefix: "offline-load")
            let pending = PendingLoad(
                operationId: operationId,
                lineageKey: key,
                lineageId: cached.lineageId,
                preIssueRevision: cached.revision,
                localBalance: cached.balance,
                keyCertificate: cached.keyCertificate,
                lineageState: cached.lineageState,
                deviceBinding: binding
            )
            withLock { pendingLoads[operationId] = pending }
            return pending.context()
        }
        return try await refillKeys(
            accountId: accountId,
            assetDefinitionId: assetDefinitionId,
            binding: binding,
            lineageKey: key
        )
    }

    public func issueNote(_ request: OfflineNoteIssueRequest) async throws -> OfflineNoteIssueResponse {
        guard withLock({ pendingLoads.removeValue(forKey: request.loadContext.operationId) }) != nil else {
            throw ToriiOfflineNoteIssuerClientError.missingLoadContext(request.loadContext.operationId)
        }
        throw ToriiOfflineNoteIssuerClientError.retiredOfflineNoteIssue
    }

    private func refillKeys(accountId: String,
                            assetDefinitionId: String,
                            binding: OfflineNoteIssuerDeviceBinding,
                            lineageKey: String) async throws -> OfflineNoteLoadContext {
        let operationId = nonceGenerator.nextId(prefix: "offline-key-refill")
        let existing = withLock { lineageStates[lineageKey] }
        var body: [String: Any] = [
            "account_id": accountId,
            "operation_id": operationId,
            "device_id": binding.deviceId,
            "offline_public_key": binding.offlinePublicKey,
            "attestation_key_id": try binding.attestationKeyId(),
            "asset_definition_id": assetDefinitionId,
            "local_revision": NSNumber(value: existing?.revision ?? 0),
            "local_state_hash": (existing?.lineageState["server_state_hash"] as? String) ?? "",
            "device_binding": try binding.deviceBinding(),
        ]
        if let existing = existing {
            body["existing_lineage_id"] = existing.lineageId
            body["lineage_state"] = try OfflineNoteIssuerDeviceBinding.deepCopyObject(existing.lineageState)
        }
        let response = try await post(path: Self.keysRefillPath, body: body)
        let lineageState = try requiredObject(response, "lineage_state")
        let certificate = try parseKeyCertificate(try requiredObject(response, "key_certificate"))
        let pending = PendingLoad(
            operationId: try requiredString(response, "operation_id"),
            lineageKey: lineageKey,
            lineageId: try requiredString(lineageState, "lineage_id"),
            preIssueRevision: try requiredUInt64(lineageState, "server_revision"),
            localBalance: try requiredString(lineageState, "balance"),
            keyCertificate: certificate,
            lineageState: lineageState,
            deviceBinding: binding
        )
        withLock { pendingLoads[pending.operationId] = pending }
        return pending.context()
    }

    private func post(path: String, body: [String: Any]) async throws -> [String: Any] {
        let url = try resolve(path: path)
        let signed = try bodyWithSignature(method: "POST", url: url, body: body)
        let bodyData = try sortedJSONData(signed)
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.httpBody = bodyData
        var headers = defaultHeaders
        headers["Content-Type"] = "application/json"
        headers["Accept"] = "application/json"
        for (key, value) in headers {
            request.setValue(value, forHTTPHeaderField: key)
        }
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else {
            throw ToriiOfflineNoteIssuerClientError.invalidJSON("http_response")
        }
        guard (200..<300).contains(http.statusCode) else {
            throw ToriiOfflineNoteIssuerClientError.httpStatus(
                http.statusCode,
                String(data: data, encoding: .utf8)
            )
        }
        let parsed = try JSONSerialization.jsonObject(with: data)
        guard let object = parsed as? [String: Any] else {
            throw ToriiOfflineNoteIssuerClientError.invalidJSON("response")
        }
        return object
    }

    private func bodyWithSignature(method: String, url: URL, body: [String: Any]) throws -> [String: Any] {
        var unsigned = body
        unsigned["account_id"] = canonicalAuth.accountId
        unsigned["timestamp_ms"] = NSNumber(value: canonicalAuth.timestampMs ?? clock())
        unsigned["nonce"] = canonicalAuth.nonce ?? nonceGenerator.nextId(prefix: "offline-auth")
        unsigned.removeValue(forKey: "signature_base64")
        unsigned.removeValue(forKey: "witness_base64")
        let timestamp = try requiredUInt64(unsigned, "timestamp_ms")
        let nonce = try requiredString(unsigned, "nonce")
        let unsignedData = try sortedJSONData(unsigned)
        let message = ToriiCanonicalRequest.signatureMessage(
            method: method,
            url: url,
            body: unsignedData,
            timestampMs: timestamp,
            nonce: nonce
        )
        let signer = try SigningKey.ed25519(privateKey: canonicalAuth.privateKey)
        let signature = try signer.sign(message)
        var signed = unsigned
        signed["signature_base64"] = signature.base64EncodedString()
        return signed
    }

    private func resolve(path: String) throws -> URL {
        if path.hasPrefix("http://") || path.hasPrefix("https://") {
            guard let url = URL(string: path) else {
                throw ToriiOfflineNoteIssuerClientError.invalidURL(path)
            }
            return url
        }
        let baseString = baseURL.absoluteString.hasSuffix("/") ? baseURL.absoluteString : "\(baseURL.absoluteString)/"
        let relative = path.hasPrefix("/") ? String(path.dropFirst()) : path
        guard let base = URL(string: baseString),
              let url = URL(string: relative, relativeTo: base)?.absoluteURL else {
            throw ToriiOfflineNoteIssuerClientError.invalidURL(path)
        }
        return url
    }

    private func withLock<T>(_ body: () throws -> T) rethrows -> T {
        lock.lock()
        defer { lock.unlock() }
        return try body()
    }

    private struct PendingLoad {
        let operationId: String
        let lineageKey: String
        let lineageId: String
        let preIssueRevision: UInt64
        let localBalance: String
        let keyCertificate: OfflineNoteKeyCertificate
        let lineageState: [String: Any]
        let deviceBinding: OfflineNoteIssuerDeviceBinding

        func context() -> OfflineNoteLoadContext {
            OfflineNoteLoadContext(
                operationId: operationId,
                lineageId: lineageId,
                localRevision: preIssueRevision + 1,
                keyCertificate: keyCertificate
            )
        }
    }

    private struct StoredLineageState {
        let lineageId: String
        let revision: UInt64
        let balance: String
        let authorizationExpiresAtMs: UInt64?
        let keyCertificateExpiresAtMs: UInt64?
        let keyCertificate: OfflineNoteKeyCertificate
        let lineageState: [String: Any]

        func isExpired(nowMs: UInt64) -> Bool {
            if let authorizationExpiresAtMs, authorizationExpiresAtMs <= nowMs {
                return true
            }
            if let keyCertificateExpiresAtMs, keyCertificateExpiresAtMs <= nowMs {
                return true
            }
            return false
        }
    }
}

private func lineageKey(accountId: String,
                        assetDefinitionId: String,
                        binding: OfflineNoteIssuerDeviceBinding) -> String {
    "\(accountId)\n\(assetDefinitionId)\n\(binding.deviceId)\n\(binding.offlinePublicKey)"
}

private func sortedJSONData(_ value: [String: Any]) throws -> Data {
    guard JSONSerialization.isValidJSONObject(value) else {
        throw ToriiOfflineNoteIssuerClientError.invalidJSON("request")
    }
    // `.withoutEscapingSlashes` is required for canonical-body interop:
    // server reconstructs bytes via `norito::json::to_vec`, which never
    // escapes `/`. Base64 fields (`offline_public_key`, attestation
    // signatures, etc.) routinely contain `/`, so omitting this option
    // makes the signed bytes diverge from the server's reconstruction
    // and every refill / issue fails with 403 OFFLINE_SIGNATURE_INVALID.
    return try JSONSerialization.data(
        withJSONObject: value,
        options: [.sortedKeys, .withoutEscapingSlashes]
    )
}

private func parseKeyCertificate(_ value: [String: Any]) throws -> OfflineNoteKeyCertificate {
    try OfflineNoteKeyCertificate(
        version: try requiredKeyCertificateVersion(value),
        platform: try requiredString(value, "platform"),
        keyId: try requiredString(value, "key_id"),
        deviceId: try requiredString(value, "device_id"),
        accountId: try requiredString(value, "account_id"),
        publicKey: try requiredBase64(value, "public_key"),
        assertionScheme: try requiredString(value, "assertion_scheme"),
        assertionKeyAlgorithm: try requiredString(value, "assertion_key_algorithm"),
        assertionPublicKey: try requiredBase64(value, "assertion_public_key"),
        assertionUsageCountLimit: try optionalAssertionUsageCountLimit(value["assertion_usage_count_limit"]),
        oneUse: try requiredBool(value, "one_use"),
        issuerSignature: try requiredBase64(value, "issuer_signature_base64")
    )
}

private func requiredObject(_ value: [String: Any], _ key: String) throws -> [String: Any] {
    guard let object = value[key] as? [String: Any] else {
        throw ToriiOfflineNoteIssuerClientError.invalidJSON(key)
    }
    return object
}

private func optionalObject(_ value: Any?) throws -> [String: Any]? {
    guard let value else { return nil }
    guard let object = value as? [String: Any] else {
        throw ToriiOfflineNoteIssuerClientError.invalidJSON("object")
    }
    return object
}

private func requiredString(_ value: [String: Any], _ key: String) throws -> String {
    guard let string = try optionalString(value[key]) else {
        throw ToriiOfflineNoteIssuerClientError.invalidJSON(key)
    }
    return string
}

private func optionalString(_ value: Any?) throws -> String? {
    guard let value else { return nil }
    guard let string = value as? String else {
        throw ToriiOfflineNoteIssuerClientError.invalidJSON("string")
    }
    return string
}

private func requiredBool(_ value: [String: Any], _ key: String) throws -> Bool {
    guard let bool = value[key] as? Bool else {
        throw ToriiOfflineNoteIssuerClientError.invalidJSON(key)
    }
    return bool
}

private func requiredUInt64(_ value: [String: Any], _ key: String) throws -> UInt64 {
    guard let number = try optionalUInt64(value[key]) else {
        throw ToriiOfflineNoteIssuerClientError.invalidJSON(key)
    }
    return number
}

private func requiredKeyCertificateVersion(_ value: [String: Any]) throws -> UInt16 {
    let version = try requiredUInt64(value, "version")
    guard version == UInt64(OfflineNoteConstants.keyCertificateVersion) else {
        throw ToriiOfflineNoteIssuerClientError.invalidJSON("version")
    }
    return OfflineNoteConstants.keyCertificateVersion
}

private func optionalAssertionUsageCountLimit(_ value: Any?) throws -> UInt32? {
    guard let limit = try optionalUInt64(value) else {
        return nil
    }
    guard limit == 1 else {
        throw ToriiOfflineNoteIssuerClientError.invalidJSON("assertion_usage_count_limit")
    }
    return 1
}

private func optionalUInt64(_ value: Any?) throws -> UInt64? {
    guard let value else { return nil }
    if value is NSNull {
        return nil
    }
    if let value = value as? UInt64 {
        return value
    }
    if let value = value as? UInt {
        return UInt64(value)
    }
    if let value = value as? Int {
        guard value >= 0 else {
            throw ToriiOfflineNoteIssuerClientError.invalidJSON("integer")
        }
        return UInt64(value)
    }
    if let value = value as? Int64 {
        guard value >= 0 else {
            throw ToriiOfflineNoteIssuerClientError.invalidJSON("integer")
        }
        return UInt64(value)
    }
    if let number = value as? NSNumber {
        let double = number.doubleValue
        guard double.isFinite, double.rounded(.towardZero) == double, double >= 0 else {
            throw ToriiOfflineNoteIssuerClientError.invalidJSON("integer")
        }
        return number.uint64Value
    }
    throw ToriiOfflineNoteIssuerClientError.invalidJSON("integer")
}

private func requiredBase64(_ value: [String: Any], _ key: String) throws -> Data {
    let string = try requiredString(value, key)
    guard let data = Data(base64Encoded: string) else {
        throw ToriiOfflineNoteIssuerClientError.invalidBase64(key)
    }
    return data
}

private func decodeHex32(_ value: String) throws -> Data {
    guard value.count == 64 else {
        throw ToriiOfflineNoteIssuerClientError.commitmentHex(value)
    }
    var out = Data()
    out.reserveCapacity(32)
    var index = value.startIndex
    while index < value.endIndex {
        let next = value.index(index, offsetBy: 2)
        guard let byte = UInt8(value[index..<next], radix: 16) else {
            throw ToriiOfflineNoteIssuerClientError.commitmentHex(value)
        }
        out.append(byte)
        index = next
    }
    return out
}
