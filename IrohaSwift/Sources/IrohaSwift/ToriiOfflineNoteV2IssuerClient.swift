import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

public enum ToriiOfflineNoteV2IssuerClientError: Error, LocalizedError, Equatable {
    case invalidURL(String)
    case accountMismatch
    case missingLoadContext(String)
    case invalidJSON(String)
    case httpStatus(Int, String?)
    case commitmentHex(String)
    case invalidBase64(String)

    public var errorDescription: String? {
        switch self {
        case let .invalidURL(path):
            return "Invalid Offline Note V2 issuer URL: \(path)."
        case .accountMismatch:
            return "Canonical auth accountId must match wallet accountId."
        case let .missingLoadContext(operationId):
            return "Missing Offline Note V2 load context for operation \(operationId)."
        case let .invalidJSON(field):
            return "Offline Note V2 issuer JSON field is invalid: \(field)."
        case let .httpStatus(code, body):
            if let body, !body.isEmpty {
                return "Offline Note V2 issuer request failed with HTTP \(code): \(body)"
            }
            return "Offline Note V2 issuer request failed with HTTP \(code)."
        case let .commitmentHex(value):
            return "Offline Note V2 issuer returned an invalid commitment: \(value)."
        case let .invalidBase64(field):
            return "Offline Note V2 issuer field \(field) must be base64."
        }
    }
}

public struct OfflineNoteV2IssuerDeviceBinding {
    public let deviceId: String
    public let offlinePublicKey: String
    private let binding: [String: Any]

    public init(deviceId: String,
                offlinePublicKey: String,
                deviceBinding: [String: Any]) throws {
        let trimmedDeviceId = deviceId.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedPublicKey = offlinePublicKey.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedDeviceId.isEmpty else {
            throw ToriiOfflineNoteV2IssuerClientError.invalidJSON("device_id")
        }
        guard !trimmedPublicKey.isEmpty else {
            throw ToriiOfflineNoteV2IssuerClientError.invalidJSON("offline_public_key")
        }
        if let bindingDeviceId = deviceBinding["device_id"] as? String,
           bindingDeviceId != trimmedDeviceId {
            throw ToriiOfflineNoteV2IssuerClientError.invalidJSON("device_binding.device_id")
        }
        if let bindingPublicKey = deviceBinding["offline_public_key"] as? String,
           bindingPublicKey != trimmedPublicKey {
            throw ToriiOfflineNoteV2IssuerClientError.invalidJSON("device_binding.offline_public_key")
        }
        self.deviceId = trimmedDeviceId
        self.offlinePublicKey = trimmedPublicKey
        self.binding = try Self.deepCopyObject(deviceBinding)
    }

    public func deviceBinding() throws -> [String: Any] {
        try Self.deepCopyObject(binding)
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
        throw ToriiOfflineNoteV2IssuerClientError.invalidJSON("device_binding")
    }
}

public protocol OfflineNoteV2IssuerDeviceBindingProvider {
    func currentDeviceBinding(chainId: String,
                              accountId: String,
                              assetDefinitionId: String) throws -> OfflineNoteV2IssuerDeviceBinding
}

public final class ToriiOfflineNoteV2IssuerClient: OfflineNoteV2IssuerClient {
    private static let keysRefillPath = "/v1/offline/v2/keys/refill"
    private static let notesIssuePath = "/v1/offline/v2/notes/issue"

    private let baseURL: URL
    private let session: URLSession
    private let canonicalAuth: ToriiCanonicalRequestAuth
    private let deviceBindingProvider: OfflineNoteV2IssuerDeviceBindingProvider
    private let defaultHeaders: [String: String]
    private let clock: () -> UInt64
    private let nonceGenerator: OfflineNoteV2IdGenerator
    private let lock = NSLock()
    private var pendingLoads: [String: PendingLoad] = [:]
    private var lineageStates: [String: StoredLineageState] = [:]

    public init(baseURL: URL = URL(string: "http://localhost:8080")!,
                session: URLSession = .shared,
                canonicalAuth: ToriiCanonicalRequestAuth,
                deviceBindingProvider: OfflineNoteV2IssuerDeviceBindingProvider,
                defaultHeaders: [String: String] = [:],
                clock: @escaping () -> UInt64 = {
                    UInt64(Date().timeIntervalSince1970 * 1000)
                },
                nonceGenerator: OfflineNoteV2IdGenerator = UuidOfflineNoteV2IdGenerator()) {
        self.baseURL = baseURL
        self.session = session
        self.canonicalAuth = canonicalAuth
        self.deviceBindingProvider = deviceBindingProvider
        self.defaultHeaders = defaultHeaders
        self.clock = clock
        self.nonceGenerator = nonceGenerator
    }

    public func prepareLoad(chainId: String,
                            accountId: String,
                            assetDefinitionId: String,
                            amount: String) async throws -> OfflineNoteV2LoadContext {
        guard canonicalAuth.accountId == accountId else {
            throw ToriiOfflineNoteV2IssuerClientError.accountMismatch
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

    public func issueNote(_ request: OfflineNoteV2IssueRequest) async throws -> OfflineNoteV2IssueResponse {
        guard let pending = withLock({ pendingLoads[request.loadContext.operationId] }) else {
            throw ToriiOfflineNoteV2IssuerClientError.missingLoadContext(request.loadContext.operationId)
        }
        let body: [String: Any] = [
            "account_id": request.accountId,
            "operation_id": pending.operationId,
            "device_id": pending.deviceBinding.deviceId,
            "offline_public_key": pending.deviceBinding.offlinePublicKey,
            "asset_definition_id": request.assetDefinitionId,
            "device_binding": try pending.deviceBinding.deviceBinding(),
            "lineage_id": pending.lineageId,
            "lineage_state": try OfflineNoteV2IssuerDeviceBinding.deepCopyObject(pending.lineageState),
            "amount": request.amount,
            "local_balance": pending.localBalance,
            "local_revision": NSNumber(value: pending.preIssueRevision),
            "note_commitment": request.noteCommitment.hexLowercased(),
        ]
        let response = try await post(path: Self.notesIssuePath, body: body)
        let commitmentHex = try requiredString(response, "issued_note_commitment")
        let commitment = try decodeHex32(commitmentHex)
        let lineageState = try requiredObject(response, "lineage_state")
        let certificateObject = try requiredObject(response, "key_certificate")
        let keyCertificate = try parseKeyCertificate(certificateObject)
        let authorization = try optionalObject(lineageState["authorization"])
        let stored = StoredLineageState(
            lineageId: try requiredString(lineageState, "lineage_id"),
            revision: try requiredUInt64(lineageState, "server_revision"),
            balance: try requiredString(lineageState, "balance"),
            authorizationExpiresAtMs: try authorization.flatMap { try optionalUInt64($0["expires_at_ms"]) },
            keyCertificateExpiresAtMs: try optionalUInt64(certificateObject["expires_at_ms"]),
            keyCertificate: keyCertificate,
            lineageState: lineageState
        )
        withLock {
            pendingLoads.removeValue(forKey: pending.operationId)
            lineageStates[pending.lineageKey] = stored
        }
        let settlement = try optionalObject(response["settlement"])
        return OfflineNoteV2IssueResponse(
            noteCommitment: commitment,
            operationId: try requiredString(response, "operation_id"),
            lineageId: stored.lineageId,
            localRevision: try requiredUInt64(response, "local_revision"),
            keyCertificate: keyCertificate,
            settlementEntryHashHex: try settlement.flatMap { try optionalString($0["entry_hash"]) }
        )
    }

    private func refillKeys(accountId: String,
                            assetDefinitionId: String,
                            binding: OfflineNoteV2IssuerDeviceBinding,
                            lineageKey: String) async throws -> OfflineNoteV2LoadContext {
        let operationId = nonceGenerator.nextId(prefix: "offline-key-refill")
        var body: [String: Any] = [
            "account_id": accountId,
            "operation_id": operationId,
            "device_id": binding.deviceId,
            "offline_public_key": binding.offlinePublicKey,
            "asset_definition_id": assetDefinitionId,
            "device_binding": try binding.deviceBinding(),
        ]
        if let existing = withLock({ lineageStates[lineageKey] }) {
            body["existing_lineage_id"] = existing.lineageId
            body["lineage_state"] = try OfflineNoteV2IssuerDeviceBinding.deepCopyObject(existing.lineageState)
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
            throw ToriiOfflineNoteV2IssuerClientError.invalidJSON("http_response")
        }
        guard (200..<300).contains(http.statusCode) else {
            throw ToriiOfflineNoteV2IssuerClientError.httpStatus(
                http.statusCode,
                String(data: data, encoding: .utf8)
            )
        }
        let parsed = try JSONSerialization.jsonObject(with: data)
        guard let object = parsed as? [String: Any] else {
            throw ToriiOfflineNoteV2IssuerClientError.invalidJSON("response")
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
                throw ToriiOfflineNoteV2IssuerClientError.invalidURL(path)
            }
            return url
        }
        let baseString = baseURL.absoluteString.hasSuffix("/") ? baseURL.absoluteString : "\(baseURL.absoluteString)/"
        let relative = path.hasPrefix("/") ? String(path.dropFirst()) : path
        guard let base = URL(string: baseString),
              let url = URL(string: relative, relativeTo: base)?.absoluteURL else {
            throw ToriiOfflineNoteV2IssuerClientError.invalidURL(path)
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
        let keyCertificate: OfflineNoteKeyCertificateV2
        let lineageState: [String: Any]
        let deviceBinding: OfflineNoteV2IssuerDeviceBinding

        func context() -> OfflineNoteV2LoadContext {
            OfflineNoteV2LoadContext(
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
        let keyCertificate: OfflineNoteKeyCertificateV2
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
                        binding: OfflineNoteV2IssuerDeviceBinding) -> String {
    "\(accountId)\n\(assetDefinitionId)\n\(binding.deviceId)\n\(binding.offlinePublicKey)"
}

private func sortedJSONData(_ value: [String: Any]) throws -> Data {
    guard JSONSerialization.isValidJSONObject(value) else {
        throw ToriiOfflineNoteV2IssuerClientError.invalidJSON("request")
    }
    // `.withoutEscapingSlashes` is required for canonical-body interop:
    // server reconstructs bytes via `norito::json::to_vec`, which never
    // escapes `/`. Base64 fields (`offline_public_key`, attestation
    // signatures, etc.) routinely contain `/`, so omitting this option
    // makes the signed bytes diverge from the server's reconstruction
    // and every refill / issue fails with 403 OFFLINE_V2_SIGNATURE_INVALID.
    return try JSONSerialization.data(
        withJSONObject: value,
        options: [.sortedKeys, .withoutEscapingSlashes]
    )
}

private func parseKeyCertificate(_ value: [String: Any]) throws -> OfflineNoteKeyCertificateV2 {
    try OfflineNoteKeyCertificateV2(
        version: UInt16(try requiredUInt64(value, "version")),
        platform: try requiredString(value, "platform"),
        keyId: try requiredString(value, "key_id"),
        deviceId: try requiredString(value, "device_id"),
        accountId: try requiredString(value, "account_id"),
        publicKey: try requiredBase64(value, "public_key"),
        assertionScheme: try requiredString(value, "assertion_scheme"),
        assertionKeyAlgorithm: try requiredString(value, "assertion_key_algorithm"),
        assertionPublicKey: try requiredBase64(value, "assertion_public_key"),
        assertionUsageCountLimit: try optionalUInt64(value["assertion_usage_count_limit"]).map { UInt32($0) },
        oneUse: try requiredBool(value, "one_use"),
        issuerSignature: try requiredBase64(value, "issuer_signature_base64")
    )
}

private func requiredObject(_ value: [String: Any], _ key: String) throws -> [String: Any] {
    guard let object = value[key] as? [String: Any] else {
        throw ToriiOfflineNoteV2IssuerClientError.invalidJSON(key)
    }
    return object
}

private func optionalObject(_ value: Any?) throws -> [String: Any]? {
    guard let value else { return nil }
    guard let object = value as? [String: Any] else {
        throw ToriiOfflineNoteV2IssuerClientError.invalidJSON("object")
    }
    return object
}

private func requiredString(_ value: [String: Any], _ key: String) throws -> String {
    guard let string = try optionalString(value[key]) else {
        throw ToriiOfflineNoteV2IssuerClientError.invalidJSON(key)
    }
    return string
}

private func optionalString(_ value: Any?) throws -> String? {
    guard let value else { return nil }
    guard let string = value as? String else {
        throw ToriiOfflineNoteV2IssuerClientError.invalidJSON("string")
    }
    return string
}

private func requiredBool(_ value: [String: Any], _ key: String) throws -> Bool {
    guard let bool = value[key] as? Bool else {
        throw ToriiOfflineNoteV2IssuerClientError.invalidJSON(key)
    }
    return bool
}

private func requiredUInt64(_ value: [String: Any], _ key: String) throws -> UInt64 {
    guard let number = try optionalUInt64(value[key]) else {
        throw ToriiOfflineNoteV2IssuerClientError.invalidJSON(key)
    }
    return number
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
            throw ToriiOfflineNoteV2IssuerClientError.invalidJSON("integer")
        }
        return UInt64(value)
    }
    if let value = value as? Int64 {
        guard value >= 0 else {
            throw ToriiOfflineNoteV2IssuerClientError.invalidJSON("integer")
        }
        return UInt64(value)
    }
    if let number = value as? NSNumber {
        let double = number.doubleValue
        guard double.isFinite, double.rounded(.towardZero) == double, double >= 0 else {
            throw ToriiOfflineNoteV2IssuerClientError.invalidJSON("integer")
        }
        return number.uint64Value
    }
    throw ToriiOfflineNoteV2IssuerClientError.invalidJSON("integer")
}

private func requiredBase64(_ value: [String: Any], _ key: String) throws -> Data {
    let string = try requiredString(value, key)
    guard let data = Data(base64Encoded: string) else {
        throw ToriiOfflineNoteV2IssuerClientError.invalidBase64(key)
    }
    return data
}

private func decodeHex32(_ value: String) throws -> Data {
    guard value.count == 64 else {
        throw ToriiOfflineNoteV2IssuerClientError.commitmentHex(value)
    }
    var out = Data()
    out.reserveCapacity(32)
    var index = value.startIndex
    while index < value.endIndex {
        let next = value.index(index, offsetBy: 2)
        guard let byte = UInt8(value[index..<next], radix: 16) else {
            throw ToriiOfflineNoteV2IssuerClientError.commitmentHex(value)
        }
        out.append(byte)
        index = next
    }
    return out
}
