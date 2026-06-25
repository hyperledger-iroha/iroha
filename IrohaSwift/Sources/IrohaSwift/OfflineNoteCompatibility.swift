import Foundation

public enum OfflineNoteCompatibilityError: Error, LocalizedError, Equatable {
    case invalidField(String)
    case invalidPayload

    public var errorDescription: String? {
        switch self {
        case let .invalidField(field):
            return "Offline Note compatibility field \(field) is invalid."
        case .invalidPayload:
            return "Offline Note compatibility payload is invalid."
        }
    }
}

enum OfflineNoteCompatibilityTextEncoding {
    static func base64UrlEncode(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .trimmingCharacters(in: CharacterSet(charactersIn: "="))
    }

    static func base64UrlDecode(_ value: String) -> Data? {
        guard !value.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
              !value.contains("="),
              value.unicodeScalars.allSatisfy({ scalar in
                  let byte = scalar.value
                  return (65...90).contains(byte)
                      || (97...122).contains(byte)
                      || (48...57).contains(byte)
                      || byte == 45
                      || byte == 95
              }) else {
            return nil
        }
        var normalized = value
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        let padding = (4 - normalized.count % 4) % 4
        normalized.append(String(repeating: "=", count: padding))
        return Data(base64Encoded: normalized)
    }

    static func decodeBase64Like(_ value: String) -> Data? {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else { return nil }
        if let decoded = Data(base64Encoded: trimmed) {
            return decoded
        }
        var normalized = trimmed
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        let padding = (4 - normalized.count % 4) % 4
        normalized.append(String(repeating: "=", count: padding))
        if let decoded = Data(base64Encoded: normalized) {
            return decoded
        }
        return Data(hexString: trimmed)
    }

    static func textPayload(_ text: String, prefix: String) throws -> Data {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.hasPrefix(prefix) else {
            throw OfflineNoteTransferTextPayloadCodecError.unknownPrefix
        }
        guard let data = base64UrlDecode(String(trimmed.dropFirst(prefix.count))), !data.isEmpty else {
            throw OfflineNoteTransferTextPayloadCodecError.invalidPayload
        }
        return data
    }

    static func encodeJsonText<T: Encodable>(_ value: T, prefix: String) throws -> String {
        prefix + base64UrlEncode(try ToriiOfflineCashCodec.canonicalData(value))
    }

    static func decodeJsonText<T: Decodable>(_ type: T.Type, from text: String, prefix: String) throws -> T {
        try JSONDecoder().decode(T.self, from: textPayload(text, prefix: prefix))
    }

    static func canonicalAmountOrOriginal(_ value: String) -> String {
        (try? ToriiOfflineCashCodec.canonicalAmountString(value)) ?? value
    }

    static func requireHashHex(_ value: String, field: String) throws -> Data {
        guard let data = Data(hexString: value), data.count == 32 else {
            throw OfflineNoteCompatibilityError.invalidField(field)
        }
        return data
    }
}

public struct OfflineCompactKeyCertificate: Codable, Equatable, Sendable {
    public let version: Int
    public let platform: String
    public let keyId: String
    public let deviceId: String
    public let accountId: String
    public let publicKey: String
    public let assertionScheme: String?
    public let assertionKeyAlgorithm: String?
    public let assertionPublicKey: String?
    public let assertionUsageCountLimit: Int?
    public let oneUse: Bool
    public let issuedAtMs: UInt64?
    public let expiresAtMs: UInt64?
    public let appAttestPublicKeyBase64: String?
    public let iosTeamId: String?
    public let iosBundleId: String?
    public let iosEnvironment: String?
    public let issuerSignatureBase64: String
    public let issuerSignaturePayloadBase64: String?

    public init(version: Int = Int(OfflineNoteConstants.keyCertificateVersion),
                platform: String,
                keyId: String,
                deviceId: String,
                accountId: String,
                publicKey: String,
                assertionScheme: String? = nil,
                assertionKeyAlgorithm: String? = nil,
                assertionPublicKey: String? = nil,
                assertionUsageCountLimit: Int? = nil,
                oneUse: Bool = true,
                issuedAtMs: UInt64? = nil,
                expiresAtMs: UInt64? = nil,
                appAttestPublicKeyBase64: String? = nil,
                iosTeamId: String? = nil,
                iosBundleId: String? = nil,
                iosEnvironment: String? = nil,
                issuerSignatureBase64: String,
                issuerSignaturePayloadBase64: String? = nil) {
        self.version = version
        self.platform = platform
        self.keyId = keyId
        self.deviceId = deviceId
        self.accountId = accountId
        self.publicKey = publicKey
        self.assertionScheme = assertionScheme
        self.assertionKeyAlgorithm = assertionKeyAlgorithm
        self.assertionPublicKey = assertionPublicKey
        self.assertionUsageCountLimit = assertionUsageCountLimit
        self.oneUse = oneUse
        self.issuedAtMs = issuedAtMs
        self.expiresAtMs = expiresAtMs
        self.appAttestPublicKeyBase64 = appAttestPublicKeyBase64
        self.iosTeamId = iosTeamId
        self.iosBundleId = iosBundleId
        self.iosEnvironment = iosEnvironment
        self.issuerSignatureBase64 = issuerSignatureBase64
        self.issuerSignaturePayloadBase64 = issuerSignaturePayloadBase64
    }

    public init(certificate: OfflineNoteKeyCertificate) {
        self.init(
            version: Int(certificate.version),
            platform: certificate.platform,
            keyId: certificate.keyId,
            deviceId: certificate.deviceId,
            accountId: certificate.accountId,
            publicKey: certificate.publicKey.base64EncodedString(),
            assertionScheme: certificate.assertionScheme,
            assertionKeyAlgorithm: certificate.assertionKeyAlgorithm,
            assertionPublicKey: certificate.assertionPublicKey.base64EncodedString(),
            assertionUsageCountLimit: certificate.assertionUsageCountLimit.map(Int.init),
            oneUse: certificate.oneUse,
            issuerSignatureBase64: certificate.issuerSignature.base64EncodedString(),
            issuerSignaturePayloadBase64: try? certificate.signingBytes().base64EncodedString()
        )
    }

    public static func decodePublicKey(_ value: String) -> Data? {
        OfflineNoteCompatibilityTextEncoding.decodeBase64Like(value)
    }

    public func offlineNoteKeyCertificate() throws -> OfflineNoteKeyCertificate {
        let platformLower = platform.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let defaultAssertionScheme = platformLower.contains("android")
            ? "android-keymint-ecdsa-p256-usage-limit-v1"
            : "apple-appattest-counter-v1"
        let defaultAssertionKeyAlgorithm = platformLower.contains("android")
            ? "ecdsa-p256-sha256"
            : "app-attest-p256"
        let assertionPublicKeyValue = assertionPublicKey
            ?? appAttestPublicKeyBase64
            ?? publicKey
        guard let version = UInt16(exactly: self.version) else {
            throw OfflineNoteCompatibilityError.invalidField("version")
        }
        guard let publicKeyData = Self.decodePublicKey(publicKey), !publicKeyData.isEmpty else {
            throw OfflineNoteCompatibilityError.invalidField("public_key")
        }
        guard let assertionPublicKeyData = Self.decodePublicKey(assertionPublicKeyValue),
              !assertionPublicKeyData.isEmpty else {
            throw OfflineNoteCompatibilityError.invalidField("assertion_public_key")
        }
        let usageLimit = try assertionUsageCountLimit.map { value -> UInt32 in
            guard let converted = UInt32(exactly: value) else {
                throw OfflineNoteCompatibilityError.invalidField("assertion_usage_count_limit")
            }
            return converted
        }
        let decodedSignature = Self.decodePublicKey(issuerSignatureBase64)
        let issuerSignature = decodedSignature?.count == 64
            ? decodedSignature!
            : Data(repeating: 0, count: 64)
        return try OfflineNoteKeyCertificate(
            version: version,
            platform: platform,
            keyId: keyId,
            deviceId: deviceId,
            accountId: accountId,
            publicKey: publicKeyData,
            assertionScheme: assertionScheme ?? defaultAssertionScheme,
            assertionKeyAlgorithm: assertionKeyAlgorithm ?? defaultAssertionKeyAlgorithm,
            assertionPublicKey: assertionPublicKeyData,
            assertionUsageCountLimit: usageLimit,
            oneUse: oneUse,
            issuerSignature: issuerSignature
        )
    }

    public func offlineNoteSigningBytes() throws -> Data {
        try offlineNoteKeyCertificate().signingBytes()
    }

    public func offlineNotePayloadHash() throws -> Data {
        try offlineNoteKeyCertificate().payloadHash()
    }

    private enum CodingKeys: String, CodingKey {
        case version
        case platform
        case keyId = "key_id"
        case deviceId = "device_id"
        case accountId = "account_id"
        case publicKey = "public_key"
        case assertionScheme = "assertion_scheme"
        case assertionKeyAlgorithm = "assertion_key_algorithm"
        case assertionPublicKey = "assertion_public_key"
        case assertionUsageCountLimit = "assertion_usage_count_limit"
        case oneUse = "one_use"
        case issuedAtMs = "issued_at_ms"
        case expiresAtMs = "expires_at_ms"
        case appAttestPublicKeyBase64 = "app_attest_public_key_base64"
        case iosTeamId = "ios_team_id"
        case iosBundleId = "ios_bundle_id"
        case iosEnvironment = "ios_environment"
        case issuerSignatureBase64 = "issuer_signature_base64"
        case issuerSignaturePayloadBase64 = "issuer_signature_payload_base64"
    }
}

public struct OfflineReceiveRequestPayload: Codable, Equatable, Sendable {
    public let invoiceId: String
    public let accountId: String
    public let assetDefinitionId: String
    public let amount: String?
    public let recipientKeyCertificate: OfflineCompactKeyCertificate
    public let generatedAtMs: UInt64
    public let displayTtlMs: UInt64
    public let chainId: String?
    public let assetId: String?
    public let outputCommitment: String?

    public init(invoiceId: String,
                accountId: String,
                assetDefinitionId: String,
                amount: String? = nil,
                recipientKeyCertificate: OfflineCompactKeyCertificate,
                generatedAtMs: UInt64 = ToriiOfflineCashCodec.currentTimestampMs(),
                displayTtlMs: UInt64 = 300_000,
                chainId: String? = nil,
                assetId: String? = nil,
                outputCommitment: String? = nil) {
        self.invoiceId = invoiceId
        self.accountId = accountId
        self.assetDefinitionId = assetDefinitionId
        self.amount = amount.map(OfflineNoteCompatibilityTextEncoding.canonicalAmountOrOriginal)
        self.recipientKeyCertificate = recipientKeyCertificate
        self.generatedAtMs = generatedAtMs
        self.displayTtlMs = displayTtlMs
        self.chainId = chainId
        self.assetId = assetId
        self.outputCommitment = outputCommitment
    }

    public init(request: OfflineNoteReceiveRequest) {
        self.init(
            invoiceId: request.paymentRequestId,
            accountId: request.accountId,
            assetDefinitionId: request.assetDefinitionId,
            amount: request.amount,
            recipientKeyCertificate: OfflineCompactKeyCertificate(certificate: request.keyCertificate),
            chainId: request.chainId,
            assetId: request.assetId,
            outputCommitment: request.outputCommitmentHex
        )
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard !container.contains(.version) else {
            throw DecodingError.dataCorruptedError(
                forKey: .version,
                in: container,
                debugDescription: "Offline receive requests are unversioned"
            )
        }
        self.init(
            invoiceId: try container.decode(String.self, forKey: .invoiceId),
            accountId: try container.decode(String.self, forKey: .accountId),
            assetDefinitionId: try container.decode(String.self, forKey: .assetDefinitionId),
            amount: try container.decodeIfPresent(String.self, forKey: .amount),
            recipientKeyCertificate: try container.decode(
                OfflineCompactKeyCertificate.self,
                forKey: .recipientKeyCertificate
            ),
            generatedAtMs: try container.decodeIfPresent(UInt64.self, forKey: .generatedAtMs)
                ?? ToriiOfflineCashCodec.currentTimestampMs(),
            displayTtlMs: try container.decodeIfPresent(UInt64.self, forKey: .displayTtlMs) ?? 300_000,
            chainId: try container.decodeIfPresent(String.self, forKey: .chainId),
            assetId: try container.decodeIfPresent(String.self, forKey: .assetId),
            outputCommitment: try container.decodeIfPresent(String.self, forKey: .outputCommitment)
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(invoiceId, forKey: .invoiceId)
        try container.encode(accountId, forKey: .accountId)
        try container.encode(assetDefinitionId, forKey: .assetDefinitionId)
        try container.encodeIfPresent(amount, forKey: .amount)
        try container.encode(recipientKeyCertificate, forKey: .recipientKeyCertificate)
        try container.encode(generatedAtMs, forKey: .generatedAtMs)
        try container.encode(displayTtlMs, forKey: .displayTtlMs)
        try container.encodeIfPresent(chainId, forKey: .chainId)
        try container.encodeIfPresent(assetId, forKey: .assetId)
        try container.encodeIfPresent(outputCommitment, forKey: .outputCommitment)
    }

    private enum CodingKeys: String, CodingKey {
        case version
        case invoiceId = "invoice_id"
        case accountId = "account_id"
        case assetDefinitionId = "asset_definition_id"
        case amount
        case recipientKeyCertificate = "recipient_key_certificate"
        case generatedAtMs = "generated_at_ms"
        case displayTtlMs = "display_ttl_ms"
        case chainId = "chain_id"
        case assetId = "asset_id"
        case outputCommitment = "output_commitment"
    }
}

public struct OfflineOneUseAssertion: Codable, Equatable, Sendable {
    public let platform: String
    public let keyId: String
    public let algorithm: String?
    public let counter: UInt64?
    public let challengeHashHex: String
    public let assertionBase64: String

    public init(platform: String,
                keyId: String,
                algorithm: String? = nil,
                counter: UInt64? = nil,
                challengeHashHex: String,
                assertionBase64: String) {
        self.platform = platform
        self.keyId = keyId
        self.algorithm = algorithm
        self.counter = counter
        self.challengeHashHex = challengeHashHex
        self.assertionBase64 = assertionBase64
    }

    private enum CodingKeys: String, CodingKey {
        case platform
        case keyId = "key_id"
        case algorithm
        case counter
        case challengeHashHex = "challenge_hash_hex"
        case assertionBase64 = "assertion_base64"
    }
}

public struct OfflineRecursiveProof: Codable, Equatable, Sendable {
    public let verifierKeyBackend: String
    public let verifierKeyId: String
    public let proofBackend: String
    public let publicInputsHashHex: String
    public let proofBytesBase64: String

    public init(publicInputsHashHex: String,
                proofBytesBase64: String) {
        self.init(
            verifierKeyId: OfflineNoteConstants.recursiveVerifierName,
            publicInputsHashHex: publicInputsHashHex,
            proofBytesBase64: proofBytesBase64
        )
    }

    public init(verifierKeyId: String,
                verifierKeyBackend: String = OfflineNoteConstants.recursiveBackend,
                proofBackend: String = OfflineNoteConstants.recursiveBackend,
                publicInputsHashHex: String,
                proofBytesBase64: String) {
        self.verifierKeyBackend = verifierKeyBackend
        self.verifierKeyId = verifierKeyId
        self.proofBackend = proofBackend
        self.publicInputsHashHex = publicInputsHashHex
        self.proofBytesBase64 = proofBytesBase64
    }

    public init(verifierKeyBackend: String,
                proofBackend: String = OfflineNoteConstants.recursiveBackend,
                publicInputsHashHex: String,
                proofBytesBase64: String) {
        self.init(
            verifierKeyId: OfflineNoteConstants.recursiveVerifierName,
            verifierKeyBackend: verifierKeyBackend,
            proofBackend: proofBackend,
            publicInputsHashHex: publicInputsHashHex,
            proofBytesBase64: proofBytesBase64
        )
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let verifierKeyBackend = try container.decode(String.self, forKey: .verifierKeyBackend)
        let verifierKeyId = try container.decode(String.self, forKey: .verifierKeyId)
        let proofBackend = try container.decode(String.self, forKey: .proofBackend)
        guard verifierKeyBackend == OfflineNoteConstants.recursiveBackend,
              verifierKeyId == OfflineNoteConstants.recursiveVerifierName,
              proofBackend == OfflineNoteConstants.recursiveBackend,
              !verifierKeyId.contains(":") else {
            throw OfflineNoteCompatibilityError.invalidField("verifier_key_id")
        }
        self.verifierKeyBackend = verifierKeyBackend
        self.verifierKeyId = verifierKeyId
        self.proofBackend = proofBackend
        publicInputsHashHex = try container.decode(String.self, forKey: .publicInputsHashHex)
        proofBytesBase64 = try container.decode(String.self, forKey: .proofBytesBase64)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(verifierKeyBackend, forKey: .verifierKeyBackend)
        try container.encode(verifierKeyId, forKey: .verifierKeyId)
        try container.encode(proofBackend, forKey: .proofBackend)
        try container.encode(publicInputsHashHex, forKey: .publicInputsHashHex)
        try container.encode(proofBytesBase64, forKey: .proofBytesBase64)
    }

    public func offlineNoteRecursiveProof() throws -> OfflineNoteRecursiveProof {
        let publicInputsHash = try OfflineNoteCompatibilityTextEncoding.requireHashHex(
            publicInputsHashHex,
            field: "public_inputs_hash_hex"
        )
        guard let proofBytes = OfflineNoteCompatibilityTextEncoding.decodeBase64Like(proofBytesBase64),
              !proofBytes.isEmpty else {
            throw OfflineNoteCompatibilityError.invalidField("proof_bytes_base64")
        }
        return try OfflineNoteRecursiveProof(
            verifierBackend: verifierKeyBackend,
            verifierName: verifierKeyId,
            publicInputsHash: publicInputsHash,
            proofBytes: proofBytes,
            proofBackend: proofBackend
        )
    }

    private enum CodingKeys: String, CodingKey {
        case verifierKeyBackend = "verifier_key_backend"
        case verifierKeyId = "verifier_key_id"
        case proofBackend = "proof_backend"
        case publicInputsHashHex = "public_inputs_hash_hex"
        case proofBytesBase64 = "proof_bytes_base64"
    }
}

public struct OfflinePaymentTokenInputClaim: Codable, Equatable, Sendable {
    public let domain: String
    public let noteCommitment: String
    public let keyCertificatePayloadHash: String
    public let assetId: String
    public let amount: String
    public let claimHash: String?

    public init(domain: String = OfflineNoteConstants.issuedClaimDomain,
                noteCommitment: String,
                keyCertificatePayloadHash: String,
                assetId: String,
                amount: String,
                claimHash: String? = nil) throws {
        self.domain = domain
        self.noteCommitment = noteCommitment
        self.keyCertificatePayloadHash = keyCertificatePayloadHash
        self.assetId = assetId
        self.amount = try ToriiOfflineCashCodec.canonicalAmountString(amount)
        self.claimHash = claimHash
            ?? (try? OfflineNoteIssuedClaim(
                domain: domain,
                noteCommitment: OfflineNoteCompatibilityTextEncoding.requireHashHex(
                    noteCommitment,
                    field: "note_commitment"
                ),
                keyCertificatePayloadHash: OfflineNoteCompatibilityTextEncoding.requireHashHex(
                    keyCertificatePayloadHash,
                    field: "key_certificate_payload_hash"
                ),
                assetId: assetId,
                amount: self.amount
            ).claimHash().hexLowercased())
    }

    public func offlineNoteIssuedClaim() throws -> OfflineNoteIssuedClaim {
        try OfflineNoteIssuedClaim(
            domain: domain,
            noteCommitment: OfflineNoteCompatibilityTextEncoding.requireHashHex(
                noteCommitment,
                field: "note_commitment"
            ),
            keyCertificatePayloadHash: OfflineNoteCompatibilityTextEncoding.requireHashHex(
                keyCertificatePayloadHash,
                field: "key_certificate_payload_hash"
            ),
            assetId: assetId,
            amount: amount
        )
    }

    public func claimHashMatches() -> Bool {
        guard let claimHash = claimHash?.trimmingCharacters(in: .whitespacesAndNewlines),
              !claimHash.isEmpty else {
            return true
        }
        return (try? offlineNoteIssuedClaim().claimHash().hexLowercased()) == claimHash
    }

    private enum CodingKeys: String, CodingKey {
        case domain
        case noteCommitment = "note_commitment"
        case keyCertificatePayloadHash = "key_certificate_payload_hash"
        case assetId = "asset_id"
        case amount
        case claimHash = "claim_hash"
    }
}

public struct OfflinePaymentTokenOutputClaim: Codable, Equatable, Sendable {
    public let noteCommitment: String
    public let keyCertificate: OfflineCompactKeyCertificate
    public let accountId: String
    public let assetDefinitionId: String
    public let amount: String

    public init(noteCommitment: String,
                keyCertificate: OfflineCompactKeyCertificate,
                accountId: String,
                assetDefinitionId: String,
                amount: String) {
        self.noteCommitment = noteCommitment
        self.keyCertificate = keyCertificate
        self.accountId = accountId
        self.assetDefinitionId = assetDefinitionId
        self.amount = OfflineNoteCompatibilityTextEncoding.canonicalAmountOrOriginal(amount)
    }

    public func offlineNoteAuditOutputClaim() throws -> OfflineNoteAuditOutputClaim {
        try OfflineNoteAuditOutputClaim(
            noteCommitment: OfflineNoteCompatibilityTextEncoding.requireHashHex(
                noteCommitment,
                field: "note_commitment"
            ),
            keyCertificate: keyCertificate.offlineNoteKeyCertificate(),
            assetId: "\(assetDefinitionId)#\(accountId)",
            amount: amount
        )
    }

    private enum CodingKeys: String, CodingKey {
        case noteCommitment = "note_commitment"
        case keyCertificate = "key_certificate"
        case accountId = "account_id"
        case assetDefinitionId = "asset_definition_id"
        case amount
    }
}

public struct OfflinePaymentToken: Codable, Equatable, Sendable {
    public let type: String?
    public let tokenId: String
    public let invoiceId: String
    public let senderAccountId: String
    public let recipientAccountId: String
    public let assetDefinitionId: String
    public let amount: String
    public let changeAmount: String
    public let sourceNoteCommitment: String?
    public let inputNullifiers: [String]
    public let inputClaims: [OfflinePaymentTokenInputClaim]
    public let outputCommitments: [String]
    public let outputClaims: [OfflinePaymentTokenOutputClaim]
    public let senderKeyCertificate: OfflineCompactKeyCertificate
    public let recipientKeyCertificate: OfflineCompactKeyCertificate
    public let oneUseAssertion: OfflineOneUseAssertion
    public let recursiveProof: OfflineRecursiveProof
    public let createdAtMs: UInt64

    public init(type: String? = "offline_payment_token",
                tokenId: String,
                invoiceId: String,
                senderAccountId: String,
                recipientAccountId: String,
                assetDefinitionId: String,
                amount: String,
                changeAmount: String,
                sourceNoteCommitment: String? = nil,
                inputNullifiers: [String],
                inputClaims: [OfflinePaymentTokenInputClaim],
                outputCommitments: [String],
                outputClaims: [OfflinePaymentTokenOutputClaim],
                senderKeyCertificate: OfflineCompactKeyCertificate,
                recipientKeyCertificate: OfflineCompactKeyCertificate,
                oneUseAssertion: OfflineOneUseAssertion,
                recursiveProof: OfflineRecursiveProof,
                createdAtMs: UInt64) {
        self.type = type
        self.tokenId = tokenId
        self.invoiceId = invoiceId
        self.senderAccountId = senderAccountId
        self.recipientAccountId = recipientAccountId
        self.assetDefinitionId = assetDefinitionId
        self.amount = OfflineNoteCompatibilityTextEncoding.canonicalAmountOrOriginal(amount)
        self.changeAmount = OfflineNoteCompatibilityTextEncoding.canonicalAmountOrOriginal(changeAmount)
        self.sourceNoteCommitment = sourceNoteCommitment
        self.inputNullifiers = inputNullifiers
        self.inputClaims = inputClaims
        self.outputCommitments = outputCommitments
        self.outputClaims = outputClaims
        self.senderKeyCertificate = senderKeyCertificate
        self.recipientKeyCertificate = recipientKeyCertificate
        self.oneUseAssertion = oneUseAssertion
        self.recursiveProof = recursiveProof
        self.createdAtMs = createdAtMs
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard !container.contains(.version) else {
            throw DecodingError.dataCorruptedError(
                forKey: .version,
                in: container,
                debugDescription: "Offline payment tokens are unversioned"
            )
        }
        try self.init(
            type: container.decodeIfPresent(String.self, forKey: .type),
            tokenId: container.decode(String.self, forKey: .tokenId),
            invoiceId: container.decode(String.self, forKey: .invoiceId),
            senderAccountId: container.decode(String.self, forKey: .senderAccountId),
            recipientAccountId: container.decode(String.self, forKey: .recipientAccountId),
            assetDefinitionId: container.decode(String.self, forKey: .assetDefinitionId),
            amount: container.decode(String.self, forKey: .amount),
            changeAmount: container.decode(String.self, forKey: .changeAmount),
            sourceNoteCommitment: container.decodeIfPresent(String.self, forKey: .sourceNoteCommitment),
            inputNullifiers: container.decode([String].self, forKey: .inputNullifiers),
            inputClaims: container.decode([OfflinePaymentTokenInputClaim].self, forKey: .inputClaims),
            outputCommitments: container.decode([String].self, forKey: .outputCommitments),
            outputClaims: container.decode([OfflinePaymentTokenOutputClaim].self, forKey: .outputClaims),
            senderKeyCertificate: container.decode(OfflineCompactKeyCertificate.self, forKey: .senderKeyCertificate),
            recipientKeyCertificate: container.decode(OfflineCompactKeyCertificate.self, forKey: .recipientKeyCertificate),
            oneUseAssertion: container.decode(OfflineOneUseAssertion.self, forKey: .oneUseAssertion),
            recursiveProof: container.decode(OfflineRecursiveProof.self, forKey: .recursiveProof),
            createdAtMs: container.decode(UInt64.self, forKey: .createdAtMs)
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encodeIfPresent(type, forKey: .type)
        try container.encode(tokenId, forKey: .tokenId)
        try container.encode(invoiceId, forKey: .invoiceId)
        try container.encode(senderAccountId, forKey: .senderAccountId)
        try container.encode(recipientAccountId, forKey: .recipientAccountId)
        try container.encode(assetDefinitionId, forKey: .assetDefinitionId)
        try container.encode(amount, forKey: .amount)
        try container.encode(changeAmount, forKey: .changeAmount)
        try container.encodeIfPresent(sourceNoteCommitment, forKey: .sourceNoteCommitment)
        try container.encode(inputNullifiers, forKey: .inputNullifiers)
        try container.encode(inputClaims, forKey: .inputClaims)
        try container.encode(outputCommitments, forKey: .outputCommitments)
        try container.encode(outputClaims, forKey: .outputClaims)
        try container.encode(senderKeyCertificate, forKey: .senderKeyCertificate)
        try container.encode(recipientKeyCertificate, forKey: .recipientKeyCertificate)
        try container.encode(oneUseAssertion, forKey: .oneUseAssertion)
        try container.encode(recursiveProof, forKey: .recursiveProof)
        try container.encode(createdAtMs, forKey: .createdAtMs)
    }

    public func outputClaim(matchingNoteCommitment noteCommitment: String) -> OfflinePaymentTokenOutputClaim? {
        let normalized = Self.normalizedNoteCommitmentHex(noteCommitment)
        guard !normalized.isEmpty else { return nil }
        return outputClaims.first {
            Self.normalizedNoteCommitmentHex($0.noteCommitment) == normalized
        }
    }

    public func containsOutputNoteCommitment(_ noteCommitment: String) -> Bool {
        outputClaim(matchingNoteCommitment: noteCommitment) != nil
    }

    public func auditBundle() throws -> OfflineNoteAuditBundle {
        try OfflineNoteAuditBundle(
            tokenId: OfflineNoteCompatibilityTextEncoding.requireHashHex(tokenId, field: "token_id"),
            senderKeyCertificate: senderKeyCertificate.offlineNoteKeyCertificate(),
            inputNullifiers: inputNullifiers.map {
                try OfflineNoteCompatibilityTextEncoding.requireHashHex($0, field: "input_nullifier")
            },
            inputClaims: inputClaims.map { try $0.offlineNoteIssuedClaim() },
            outputCommitments: outputCommitments.map {
                try OfflineNoteCompatibilityTextEncoding.requireHashHex($0, field: "output_commitment")
            },
            outputClaims: outputClaims.map { try $0.offlineNoteAuditOutputClaim() },
            recursiveProof: recursiveProof.offlineNoteRecursiveProof()
        )
    }

    private static func normalizedNoteCommitmentHex(_ value: String) -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if trimmed.hasPrefix("0x") {
            return String(trimmed.dropFirst(2))
        }
        return trimmed
    }

    private enum CodingKeys: String, CodingKey {
        case type
        case version
        case tokenId = "token_id"
        case invoiceId = "invoice_id"
        case senderAccountId = "sender_account_id"
        case recipientAccountId = "recipient_account_id"
        case assetDefinitionId = "asset_definition_id"
        case amount
        case changeAmount = "change_amount"
        case sourceNoteCommitment = "source_note_commitment"
        case inputNullifiers = "input_nullifiers"
        case inputClaims = "input_claims"
        case outputCommitments = "output_commitments"
        case outputClaims = "output_claims"
        case senderKeyCertificate = "sender_key_certificate"
        case recipientKeyCertificate = "recipient_key_certificate"
        case oneUseAssertion = "one_use_assertion"
        case recursiveProof = "recursive_proof"
        case createdAtMs = "created_at_ms"
    }
}

public struct OfflineReceiptAck: Codable, Equatable, Sendable {
    public let tokenId: String
    public let recipientAccountId: String
    public let acceptedAtMs: UInt64
    public let chainId: String?
    public let paymentRequestId: String?

    public init(tokenId: String,
                recipientAccountId: String,
                acceptedAtMs: UInt64,
                chainId: String? = nil,
                paymentRequestId: String? = nil) {
        self.tokenId = tokenId
        self.recipientAccountId = recipientAccountId
        self.acceptedAtMs = acceptedAtMs
        self.chainId = chainId
        self.paymentRequestId = paymentRequestId
    }

    public init(ack: OfflineNoteReceiptAck) {
        self.init(
            tokenId: ack.tokenIdHex,
            recipientAccountId: ack.recipientAccountId,
            acceptedAtMs: ack.acceptedAtMs,
            chainId: ack.chainId,
            paymentRequestId: ack.paymentRequestId
        )
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard !container.contains(.version) else {
            throw DecodingError.dataCorruptedError(
                forKey: .version,
                in: container,
                debugDescription: "Offline receipt ACKs are unversioned"
            )
        }
        self.init(
            tokenId: try container.decode(String.self, forKey: .tokenId),
            recipientAccountId: try container.decode(String.self, forKey: .recipientAccountId),
            acceptedAtMs: try container.decode(UInt64.self, forKey: .acceptedAtMs),
            chainId: try container.decodeIfPresent(String.self, forKey: .chainId),
            paymentRequestId: try container.decodeIfPresent(String.self, forKey: .paymentRequestId)
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(tokenId, forKey: .tokenId)
        try container.encode(recipientAccountId, forKey: .recipientAccountId)
        try container.encode(acceptedAtMs, forKey: .acceptedAtMs)
        try container.encodeIfPresent(chainId, forKey: .chainId)
        try container.encodeIfPresent(paymentRequestId, forKey: .paymentRequestId)
    }

    public func matchesPaymentToken(_ token: OfflinePaymentToken) -> Bool {
        tokenId == token.tokenId
            && token.outputClaims.contains { claim in
                claim.accountId == recipientAccountId
            }
    }

    private enum CodingKeys: String, CodingKey {
        case version
        case tokenId = "token_id"
        case recipientAccountId = "recipient_account_id"
        case acceptedAtMs = "accepted_at_ms"
        case chainId = "chain_id"
        case paymentRequestId = "payment_request_id"
    }
}
