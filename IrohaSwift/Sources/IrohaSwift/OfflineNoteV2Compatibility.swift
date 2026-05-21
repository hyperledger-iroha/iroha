import Foundation

public enum OfflineNoteV2CompatibilityError: Error, LocalizedError, Equatable {
    case invalidField(String)
    case invalidPayload

    public var errorDescription: String? {
        switch self {
        case let .invalidField(field):
            return "Offline Note V2 compatibility field \(field) is invalid."
        case .invalidPayload:
            return "Offline Note V2 compatibility payload is invalid."
        }
    }
}

enum OfflineNoteV2CompatibilityTextEncoding {
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
            throw OfflineNoteV2TransferTextPayloadCodecError.unknownPrefix
        }
        guard let data = base64UrlDecode(String(trimmed.dropFirst(prefix.count))), !data.isEmpty else {
            throw OfflineNoteV2TransferTextPayloadCodecError.invalidPayload
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
            throw OfflineNoteV2CompatibilityError.invalidField(field)
        }
        return data
    }
}

public struct OfflineCompactKeyCertificateV2: Codable, Equatable, Sendable {
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

    public init(version: Int = 2,
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

    public init(certificate: OfflineNoteKeyCertificateV2) {
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
        OfflineNoteV2CompatibilityTextEncoding.decodeBase64Like(value)
    }

    public func offlineNoteKeyCertificate() throws -> OfflineNoteKeyCertificateV2 {
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
            throw OfflineNoteV2CompatibilityError.invalidField("version")
        }
        guard let publicKeyData = Self.decodePublicKey(publicKey), !publicKeyData.isEmpty else {
            throw OfflineNoteV2CompatibilityError.invalidField("public_key")
        }
        guard let assertionPublicKeyData = Self.decodePublicKey(assertionPublicKeyValue),
              !assertionPublicKeyData.isEmpty else {
            throw OfflineNoteV2CompatibilityError.invalidField("assertion_public_key")
        }
        let usageLimit = try assertionUsageCountLimit.map { value -> UInt32 in
            guard let converted = UInt32(exactly: value) else {
                throw OfflineNoteV2CompatibilityError.invalidField("assertion_usage_count_limit")
            }
            return converted
        }
        let decodedSignature = Self.decodePublicKey(issuerSignatureBase64)
        let issuerSignature = decodedSignature?.count == 64
            ? decodedSignature!
            : Data(repeating: 0, count: 64)
        return try OfflineNoteKeyCertificateV2(
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

public struct OfflineReceiveChallengeV2: Codable, Equatable, Sendable {
    public let version: Int
    public let invoiceId: String
    public let accountId: String
    public let assetDefinitionId: String
    public let amount: String?
    public let recipientKeyCertificate: OfflineCompactKeyCertificateV2
    public let generatedAtMs: UInt64
    public let displayTtlMs: UInt64
    public let chainId: String?
    public let assetId: String?
    public let outputCommitment: String?

    public init(version: Int = 2,
                invoiceId: String,
                accountId: String,
                assetDefinitionId: String,
                amount: String? = nil,
                recipientKeyCertificate: OfflineCompactKeyCertificateV2,
                generatedAtMs: UInt64 = ToriiOfflineCashCodec.currentTimestampMs(),
                displayTtlMs: UInt64 = 300_000,
                chainId: String? = nil,
                assetId: String? = nil,
                outputCommitment: String? = nil) {
        self.version = version
        self.invoiceId = invoiceId
        self.accountId = accountId
        self.assetDefinitionId = assetDefinitionId
        self.amount = amount.map(OfflineNoteV2CompatibilityTextEncoding.canonicalAmountOrOriginal)
        self.recipientKeyCertificate = recipientKeyCertificate
        self.generatedAtMs = generatedAtMs
        self.displayTtlMs = displayTtlMs
        self.chainId = chainId
        self.assetId = assetId
        self.outputCommitment = outputCommitment
    }

    public init(request: OfflineNoteV2ReceiveRequest) {
        self.init(
            invoiceId: request.paymentRequestId,
            accountId: request.accountId,
            assetDefinitionId: request.assetDefinitionId,
            amount: request.amount,
            recipientKeyCertificate: OfflineCompactKeyCertificateV2(certificate: request.keyCertificate),
            chainId: request.chainId,
            assetId: request.assetId,
            outputCommitment: request.outputCommitmentHex
        )
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

public struct OfflineOneUseAssertionV2: Codable, Equatable, Sendable {
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

public struct OfflineRecursiveProofV2: Codable, Equatable, Sendable {
    public let verifierKeyBackend: String
    public let verifierKeyId: String
    public let proofBackend: String
    public let publicInputsHashHex: String
    public let proofBytesBase64: String

    public init(publicInputsHashHex: String,
                proofBytesBase64: String) {
        self.init(
            verifierKeyId: OfflineNoteV2Constants.recursiveVerifierName,
            publicInputsHashHex: publicInputsHashHex,
            proofBytesBase64: proofBytesBase64
        )
    }

    public init(verifierKeyId: String,
                verifierKeyBackend: String = OfflineNoteV2Constants.recursiveBackend,
                proofBackend: String = OfflineNoteV2Constants.recursiveBackend,
                publicInputsHashHex: String,
                proofBytesBase64: String) {
        self.verifierKeyBackend = verifierKeyBackend
        self.verifierKeyId = verifierKeyId
        self.proofBackend = proofBackend
        self.publicInputsHashHex = publicInputsHashHex
        self.proofBytesBase64 = proofBytesBase64
    }

    public init(verifierKeyBackend: String,
                proofBackend: String = OfflineNoteV2Constants.recursiveBackend,
                publicInputsHashHex: String,
                proofBytesBase64: String) {
        self.init(
            verifierKeyId: OfflineNoteV2Constants.recursiveVerifierName,
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
        guard verifierKeyBackend == OfflineNoteV2Constants.recursiveBackend,
              verifierKeyId == OfflineNoteV2Constants.recursiveVerifierName,
              proofBackend == OfflineNoteV2Constants.recursiveBackend,
              !verifierKeyId.contains(":") else {
            throw OfflineNoteV2CompatibilityError.invalidField("verifier_key_id")
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

    public func offlineNoteRecursiveProof() throws -> OfflineNoteRecursiveProofV2 {
        let publicInputsHash = try OfflineNoteV2CompatibilityTextEncoding.requireHashHex(
            publicInputsHashHex,
            field: "public_inputs_hash_hex"
        )
        guard let proofBytes = OfflineNoteV2CompatibilityTextEncoding.decodeBase64Like(proofBytesBase64),
              !proofBytes.isEmpty else {
            throw OfflineNoteV2CompatibilityError.invalidField("proof_bytes_base64")
        }
        return try OfflineNoteRecursiveProofV2(
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

public struct OfflinePaymentTokenInputClaimV2: Codable, Equatable, Sendable {
    public let domain: String
    public let noteCommitment: String
    public let keyCertificatePayloadHash: String
    public let assetId: String
    public let amount: String
    public let claimHash: String?

    public init(domain: String = OfflineNoteV2Constants.issuedClaimDomain,
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
            ?? (try? OfflineNoteIssuedClaimV2(
                domain: domain,
                noteCommitment: OfflineNoteV2CompatibilityTextEncoding.requireHashHex(
                    noteCommitment,
                    field: "note_commitment"
                ),
                keyCertificatePayloadHash: OfflineNoteV2CompatibilityTextEncoding.requireHashHex(
                    keyCertificatePayloadHash,
                    field: "key_certificate_payload_hash"
                ),
                assetId: assetId,
                amount: self.amount
            ).claimHash().hexLowercased())
    }

    public func offlineNoteIssuedClaim() throws -> OfflineNoteIssuedClaimV2 {
        try OfflineNoteIssuedClaimV2(
            domain: domain,
            noteCommitment: OfflineNoteV2CompatibilityTextEncoding.requireHashHex(
                noteCommitment,
                field: "note_commitment"
            ),
            keyCertificatePayloadHash: OfflineNoteV2CompatibilityTextEncoding.requireHashHex(
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

public struct OfflinePaymentTokenOutputClaimV2: Codable, Equatable, Sendable {
    public let noteCommitment: String
    public let keyCertificate: OfflineCompactKeyCertificateV2
    public let accountId: String
    public let assetDefinitionId: String
    public let amount: String

    public init(noteCommitment: String,
                keyCertificate: OfflineCompactKeyCertificateV2,
                accountId: String,
                assetDefinitionId: String,
                amount: String) {
        self.noteCommitment = noteCommitment
        self.keyCertificate = keyCertificate
        self.accountId = accountId
        self.assetDefinitionId = assetDefinitionId
        self.amount = OfflineNoteV2CompatibilityTextEncoding.canonicalAmountOrOriginal(amount)
    }

    public func offlineNoteAuditOutputClaim() throws -> OfflineNoteAuditOutputClaimV2 {
        try OfflineNoteAuditOutputClaimV2(
            noteCommitment: OfflineNoteV2CompatibilityTextEncoding.requireHashHex(
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

public struct OfflinePaymentTokenV2: Codable, Equatable, Sendable {
    public let type: String?
    public let version: Int
    public let tokenId: String
    public let invoiceId: String
    public let senderAccountId: String
    public let recipientAccountId: String
    public let assetDefinitionId: String
    public let amount: String
    public let changeAmount: String
    public let sourceNoteCommitment: String?
    public let inputNullifiers: [String]
    public let inputClaims: [OfflinePaymentTokenInputClaimV2]
    public let outputCommitments: [String]
    public let outputClaims: [OfflinePaymentTokenOutputClaimV2]
    public let senderKeyCertificate: OfflineCompactKeyCertificateV2
    public let recipientKeyCertificate: OfflineCompactKeyCertificateV2
    public let oneUseAssertion: OfflineOneUseAssertionV2
    public let recursiveProof: OfflineRecursiveProofV2
    public let createdAtMs: UInt64

    public init(version: Int = 2,
                type: String? = "offline_payment_token_v2",
                tokenId: String,
                invoiceId: String,
                senderAccountId: String,
                recipientAccountId: String,
                assetDefinitionId: String,
                amount: String,
                changeAmount: String,
                sourceNoteCommitment: String? = nil,
                inputNullifiers: [String],
                inputClaims: [OfflinePaymentTokenInputClaimV2],
                outputCommitments: [String],
                outputClaims: [OfflinePaymentTokenOutputClaimV2],
                senderKeyCertificate: OfflineCompactKeyCertificateV2,
                recipientKeyCertificate: OfflineCompactKeyCertificateV2,
                oneUseAssertion: OfflineOneUseAssertionV2,
                recursiveProof: OfflineRecursiveProofV2,
                createdAtMs: UInt64) {
        self.type = type
        self.version = version
        self.tokenId = tokenId
        self.invoiceId = invoiceId
        self.senderAccountId = senderAccountId
        self.recipientAccountId = recipientAccountId
        self.assetDefinitionId = assetDefinitionId
        self.amount = OfflineNoteV2CompatibilityTextEncoding.canonicalAmountOrOriginal(amount)
        self.changeAmount = OfflineNoteV2CompatibilityTextEncoding.canonicalAmountOrOriginal(changeAmount)
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

    public func auditBundle() throws -> OfflineNoteAuditBundleV2 {
        try OfflineNoteAuditBundleV2(
            tokenId: OfflineNoteV2CompatibilityTextEncoding.requireHashHex(tokenId, field: "token_id"),
            senderKeyCertificate: senderKeyCertificate.offlineNoteKeyCertificate(),
            inputNullifiers: inputNullifiers.map {
                try OfflineNoteV2CompatibilityTextEncoding.requireHashHex($0, field: "input_nullifier")
            },
            inputClaims: inputClaims.map { try $0.offlineNoteIssuedClaim() },
            outputCommitments: outputCommitments.map {
                try OfflineNoteV2CompatibilityTextEncoding.requireHashHex($0, field: "output_commitment")
            },
            outputClaims: outputClaims.map { try $0.offlineNoteAuditOutputClaim() },
            recursiveProof: recursiveProof.offlineNoteRecursiveProof()
        )
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

public struct OfflineReceiptAckV2: Codable, Equatable, Sendable {
    public let version: Int
    public let tokenId: String
    public let recipientAccountId: String
    public let acceptedAtMs: UInt64
    public let chainId: String?
    public let paymentRequestId: String?

    public init(version: Int = 2,
                tokenId: String,
                recipientAccountId: String,
                acceptedAtMs: UInt64,
                chainId: String? = nil,
                paymentRequestId: String? = nil) {
        self.version = version
        self.tokenId = tokenId
        self.recipientAccountId = recipientAccountId
        self.acceptedAtMs = acceptedAtMs
        self.chainId = chainId
        self.paymentRequestId = paymentRequestId
    }

    public init(ack: OfflineNoteV2ReceiptAck) {
        self.init(
            tokenId: ack.tokenIdHex,
            recipientAccountId: ack.recipientAccountId,
            acceptedAtMs: ack.acceptedAtMs,
            chainId: ack.chainId,
            paymentRequestId: ack.paymentRequestId
        )
    }

    public func matchesPaymentToken(_ token: OfflinePaymentTokenV2) -> Bool {
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
