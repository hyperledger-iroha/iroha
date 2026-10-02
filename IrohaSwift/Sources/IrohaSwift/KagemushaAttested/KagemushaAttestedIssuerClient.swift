import Foundation

/// HTTP transport for the issuer. The default uses `URLSession`; tests inject an in-process issuer.
public protocol KagemushaIssuerTransport: Sendable {
    func perform(_ request: URLRequest) async throws -> (Data, HTTPURLResponse)
}

/// `URLSession` transport with no caching and no cookies.
public struct KagemushaURLSessionTransport: KagemushaIssuerTransport {
    private let session: URLSession

    public init(session: URLSession? = nil) {
        if let session {
            self.session = session
        } else {
            let configuration = URLSessionConfiguration.ephemeral
            configuration.requestCachePolicy = .reloadIgnoringLocalAndRemoteCacheData
            configuration.httpShouldSetCookies = false
            configuration.timeoutIntervalForRequest = 30
            self.session = URLSession(configuration: configuration)
        }
    }

    public func perform(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let (data, response) = try await session.data(for: request)
        guard let http = response as? HTTPURLResponse else {
            throw KagemushaError.network(retryable: true, description: "non-HTTP response")
        }
        return (data, http)
    }
}

/// Client for `{issuerURL}/v1/kagemusha/attested/*`.
///
/// Bodies are JSON objects whose binary members are unpadded base64url; signed suite objects
/// travel as their complete canonical Norito frames. Amounts and millisecond timestamps are
/// Unsigned JSON integers decoded without rounding or truncation.
struct KagemushaAttestedIssuerClient: Sendable {
    static let routePrefix = "v1/kagemusha/attested/"
    static let maximumResponseBytes = 8 * 1024 * 1024

    let baseURL: URL
    let transport: any KagemushaIssuerTransport

    struct EnrollmentChallenge: Equatable, Sendable {
        var serverNonce: Data
        var expiresAtMs: UInt64
    }

    struct EnrollmentRequest: Sendable {
        var schemeId: Data
        var accountId: String
        var platform: KagemushaPlatform
        var serverNonce: Data
        var clientNonce: Data
        var attestedKeyId: Data
        var signingPublicKey: Data
        var attestation: Data
        var attestationFormat: String
        var accountProof: KagemushaAccountProof
    }

    struct EnrollmentResponse: Sendable {
        var cert: Data
        var crl: Data
        var nextSyncNonce: Data
    }

    struct LoadPreparation: Equatable, Sendable {
        var loadId: Data
        var binding: Data
        var reserveAccountId: String
        var assetDefinitionId: String
        var expiresAtMs: UInt64
    }

    struct SyncAttestation: Sendable {
        var format: String
        var keyId: Data
        var assertion: Data
    }

    struct SyncRequest: Sendable {
        var deviceId: Data
        var syncNonce: Data
        var transitions: [KagemushaAttestedSignedTransitionV1]
        var receivedPayments: [Data]
        var refused: [(payment: Data, reason: KagemushaRefusal)]
        var forkEvidence: [Data]
        var attestation: SyncAttestation
    }

    struct CrlUpdate: Sendable {
        var crl: Data?
        var deltas: [Data]
    }

    func descriptor() async throws -> Data {
        let object = try await send("GET", "descriptor", body: nil)
        return try Self.binary(object, "descriptor")
    }

    func readiness() async throws -> Bool {
        let object = try await send("GET", "readiness", body: nil)
        guard let ready = object["ready"] as? Bool else {
            throw KagemushaError.invalidIssuerResponse("readiness.ready")
        }
        return ready
    }

    func crl(since epoch: UInt64?) async throws -> CrlUpdate {
        let route = epoch.map { "crl?since=\($0)" } ?? "crl"
        let object = try await send("GET", route, body: nil)
        let crl = try object["crl"].map { _ in try Self.binary(object, "crl") }
        let deltas = try (object["deltas"] as? [Any] ?? []).map { value -> Data in
            guard let text = value as? String, let data = Data(kagemushaBase64URL: text) else {
                throw KagemushaError.invalidIssuerResponse("crl.deltas")
            }
            return data
        }
        return CrlUpdate(crl: crl, deltas: deltas)
    }

    func enrollmentChallenge(accountId: String, platform: KagemushaPlatform) async throws -> EnrollmentChallenge {
        let object = try await send("POST", "enroll/challenge", body: [
            "account_id": accountId,
            "platform": Int(platform.rawValue),
        ])
        let nonce = try Self.binary(object, "server_nonce", count: 32)
        return EnrollmentChallenge(serverNonce: nonce, expiresAtMs: try Self.integer(object, "expires_at_ms"))
    }

    func enroll(_ request: EnrollmentRequest) async throws -> EnrollmentResponse {
        let route: String
        switch request.platform {
        case .appleSecureEnclave: route = "enroll/apple"
        case .androidStrongBox, .androidTee: route = "enroll/android"
        }
        let object = try await send("POST", route, body: [
            "scheme_id": request.schemeId.kagemushaBase64URL,
            "account_id": request.accountId,
            "platform": Int(request.platform.rawValue),
            "server_nonce": request.serverNonce.kagemushaBase64URL,
            "client_nonce": request.clientNonce.kagemushaBase64URL,
            "attested_key_id": request.attestedKeyId.kagemushaBase64URL,
            "signing_public_key": request.signingPublicKey.kagemushaBase64URL,
            "attestation": request.attestation.kagemushaBase64URL,
            "attestation_format": request.attestationFormat,
            "account_proof": [
                "algorithm": request.accountProof.algorithm,
                "public_key": request.accountProof.publicKey.kagemushaBase64URL,
                "signature": request.accountProof.signature.kagemushaBase64URL,
            ],
        ])
        return EnrollmentResponse(
            cert: try Self.binary(object, "cert"),
            crl: try Self.binary(object, "crl"),
            nextSyncNonce: try Self.binary(object, "next_sync_nonce", count: 32))
    }

    func prepareLoad(deviceId: Data, amount: UInt64, deviceSignature: Data) async throws -> LoadPreparation {
        let object = try await send("POST", "load/prepare", body: [
            "device_id": deviceId.kagemushaBase64URL,
            "amount": NSNumber(value: amount),
            "device_signature": deviceSignature.kagemushaBase64URL,
        ])
        return LoadPreparation(
            loadId: try Self.binary(object, "load_id", count: 16),
            binding: try Self.binary(object, "binding", count: 32),
            reserveAccountId: try Self.string(object, "reserve_account_id"),
            assetDefinitionId: try Self.string(object, "asset_definition_id"),
            expiresAtMs: try Self.integer(object, "expires_at_ms"))
    }

    func claimLoad(loadId: Data, transactionHash: String) async throws -> Data {
        let object = try await send("POST", "load/claim", body: [
            "load_id": loadId.kagemushaBase64URL,
            "tx_hash": transactionHash,
        ])
        return try Self.binary(object, "voucher")
    }

    func sync(_ request: SyncRequest) async throws -> Data {
        let object = try await send("POST", "sync", body: [
            "device_id": request.deviceId.kagemushaBase64URL,
            "sync_nonce": request.syncNonce.kagemushaBase64URL,
            "transitions": request.transitions.map {
                [
                    "transition": $0.transition.canonicalBytes.kagemushaBase64URL,
                    "signature": $0.signature.kagemushaBase64URL,
                ]
            },
            "received_payments": request.receivedPayments.map(\.kagemushaBase64URL),
            "refused": request.refused.map {
                ["payment": $0.payment.kagemushaBase64URL, "reason": Int($0.reason.rawValue)] as [String: Any]
            },
            "fork_evidence": request.forkEvidence.map(\.kagemushaBase64URL),
            "attestation": [
                "format": request.attestation.format,
                "key_id": request.attestation.keyId.kagemushaBase64URL,
                "assertion": request.attestation.assertion.kagemushaBase64URL,
            ],
        ])
        return try Self.binary(object, "receipt")
    }

    // MARK: - Transport

    private func send(_ method: String, _ route: String, body: [String: Any]?) async throws -> [String: Any] {
        guard let url = URL(string: Self.routePrefix + route, relativeTo: Self.directoryURL(baseURL)) else {
            throw KagemushaError.invalidIssuerResponse("route \(route)")
        }
        var request = URLRequest(url: url.absoluteURL)
        request.httpMethod = method
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        if let body {
            request.setValue("application/json", forHTTPHeaderField: "Content-Type")
            request.httpBody = try JSONSerialization.data(withJSONObject: body, options: [.sortedKeys])
        }
        let data: Data
        let response: HTTPURLResponse
        do {
            (data, response) = try await transport.perform(request)
        } catch let error as KagemushaError {
            throw error
        } catch let error as URLError {
            throw KagemushaError.network(retryable: true, description: error.localizedDescription)
        } catch {
            throw KagemushaError.network(retryable: true, description: error.localizedDescription)
        }
        guard data.count <= Self.maximumResponseBytes else {
            throw KagemushaError.invalidIssuerResponse("response too large")
        }
        let object = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any]
        guard (200..<300).contains(response.statusCode) else {
            let code = object?["code"] as? String ?? object?["error"] as? String ?? "http_\(response.statusCode)"
            if response.statusCode >= 500 {
                throw KagemushaError.issuerRejected(status: response.statusCode, code: code)
            }
            throw KagemushaError.issuerRejected(status: response.statusCode, code: code)
        }
        guard let object else { throw KagemushaError.invalidIssuerResponse("body is not a JSON object") }
        return object
    }

    /// Treat the configured URL as a directory so relative routes append to its path.
    static func directoryURL(_ url: URL) -> URL {
        url.absoluteString.hasSuffix("/") ? url : URL(string: url.absoluteString + "/") ?? url
    }

    static func binary(_ object: [String: Any], _ key: String, count: Int? = nil) throws -> Data {
        guard let text = object[key] as? String, let data = Data(kagemushaBase64URL: text),
              count.map({ data.count == $0 }) ?? !data.isEmpty
        else { throw KagemushaError.invalidIssuerResponse(key) }
        return data
    }

    static func string(_ object: [String: Any], _ key: String) throws -> String {
        guard let value = object[key] as? String, !value.isEmpty else {
            throw KagemushaError.invalidIssuerResponse(key)
        }
        return value
    }

    static func integer(_ object: [String: Any], _ key: String) throws -> UInt64 {
        guard let number = object[key] as? NSNumber,
              CFGetTypeID(number) != CFBooleanGetTypeID(),
              let value = StrictJSONNumber.uint64(from: number)
        else { throw KagemushaError.invalidIssuerResponse(key) }
        return value
    }
}
