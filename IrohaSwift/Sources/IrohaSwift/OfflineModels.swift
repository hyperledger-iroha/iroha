import Foundation

public struct OfflineAllowanceCommitment: Codable, Sendable, Equatable {
    public let assetId: String
    public let amount: String
    public let commitment: Data

    private enum CodingKeys: String, CodingKey {
        case assetId = "asset"
        case amount
        case commitmentHex = "commitment_hex"
    }

    public init(assetId: String, amount: String, commitment: Data) {
        self.assetId = assetId
        self.amount = amount
        self.commitment = commitment
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        assetId = try container.decode(String.self, forKey: .assetId)
        amount = try container.decode(String.self, forKey: .amount)
        let commitmentHex = try container.decode(String.self, forKey: .commitmentHex)
        guard let decoded = Data(hexString: commitmentHex) else {
            throw OfflineNoritoError.invalidHex("commitment_hex")
        }
        commitment = decoded
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(assetId, forKey: .assetId)
        try container.encode(amount, forKey: .amount)
        try container.encode(commitment.hexUppercased(), forKey: .commitmentHex)
    }

    func noritoPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(try OfflineNorito.encodeAssetId(assetId))
        writer.writeField(try OfflineNorito.encodeNumeric(amount))
        writer.writeField(OfflineNorito.encodeBytesVec(commitment))
        return writer.data
    }
}

public struct OfflineWalletPolicy: Codable, Sendable, Equatable {
    public let maxBalance: String
    public let maxTxValue: String
    public let expiresAtMs: UInt64

    private enum CodingKeys: String, CodingKey {
        case maxBalance = "max_balance"
        case maxTxValue = "max_tx_value"
        case expiresAtMs = "expires_at_ms"
    }

    public init(maxBalance: String, maxTxValue: String, expiresAtMs: UInt64) {
        self.maxBalance = maxBalance
        self.maxTxValue = maxTxValue
        self.expiresAtMs = expiresAtMs
    }

    func noritoPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(try OfflineNorito.encodeNumeric(maxBalance))
        writer.writeField(try OfflineNorito.encodeNumeric(maxTxValue))
        writer.writeField(OfflineNorito.encodeUInt64(expiresAtMs))
        return writer.data
    }
}

public struct OfflineWalletCertificateDraft: Codable, Sendable, Equatable {
    public let controller: String
    public let operatorId: String
    public let allowance: OfflineAllowanceCommitment
    public let spendPublicKey: String
    public let attestationReport: Data
    public let issuedAtMs: UInt64
    public let expiresAtMs: UInt64
    public let policy: OfflineWalletPolicy
    public let metadata: [String: ToriiJSONValue]
    public let verdictId: Data?
    public let attestationNonce: Data?
    public let refreshAtMs: UInt64?

    private enum CodingKeys: String, CodingKey {
        case controller
        case operatorId = "operator"
        case allowance
        case spendPublicKey = "spend_public_key"
        case attestationReportB64 = "attestation_report_b64"
        case issuedAtMs = "issued_at_ms"
        case expiresAtMs = "expires_at_ms"
        case policy
        case metadata
        case verdictIdHex = "verdict_id_hex"
        case attestationNonceHex = "attestation_nonce_hex"
        case refreshAtMs = "refresh_at_ms"
    }

    public init(controller: String,
                operatorId: String,
                allowance: OfflineAllowanceCommitment,
                spendPublicKey: String,
                attestationReport: Data,
                issuedAtMs: UInt64,
                expiresAtMs: UInt64,
                policy: OfflineWalletPolicy,
                metadata: [String: ToriiJSONValue] = [:],
                verdictId: Data? = nil,
                attestationNonce: Data? = nil,
                refreshAtMs: UInt64? = nil) {
        self.controller = controller
        self.operatorId = operatorId
        self.allowance = allowance
        self.spendPublicKey = spendPublicKey
        self.attestationReport = attestationReport
        self.issuedAtMs = issuedAtMs
        self.expiresAtMs = expiresAtMs
        self.policy = policy
        self.metadata = metadata
        self.verdictId = verdictId
        self.attestationNonce = attestationNonce
        self.refreshAtMs = refreshAtMs
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        controller = try container.decode(String.self, forKey: .controller)
        operatorId = try container.decode(String.self, forKey: .operatorId)
        allowance = try container.decode(OfflineAllowanceCommitment.self, forKey: .allowance)
        spendPublicKey = try container.decode(String.self, forKey: .spendPublicKey)
        let reportB64 = try container.decode(String.self, forKey: .attestationReportB64)
        guard let report = Data(base64Encoded: reportB64) else {
            throw OfflineNoritoError.invalidLength("attestation_report_b64")
        }
        attestationReport = report
        issuedAtMs = try container.decode(UInt64.self, forKey: .issuedAtMs)
        expiresAtMs = try container.decode(UInt64.self, forKey: .expiresAtMs)
        policy = try container.decode(OfflineWalletPolicy.self, forKey: .policy)
        metadata = try container.decodeIfPresent([String: ToriiJSONValue].self, forKey: .metadata) ?? [:]
        verdictId = try Self.decodeOptionalHash(from: container, key: .verdictIdHex)
        attestationNonce = try Self.decodeOptionalHash(from: container, key: .attestationNonceHex)
        refreshAtMs = try container.decodeIfPresent(UInt64.self, forKey: .refreshAtMs)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(controller, forKey: .controller)
        try container.encode(operatorId, forKey: .operatorId)
        try container.encode(allowance, forKey: .allowance)
        try container.encode(spendPublicKey, forKey: .spendPublicKey)
        try container.encode(attestationReport.base64EncodedString(), forKey: .attestationReportB64)
        try container.encode(issuedAtMs, forKey: .issuedAtMs)
        try container.encode(expiresAtMs, forKey: .expiresAtMs)
        try container.encode(policy, forKey: .policy)
        try container.encode(metadata, forKey: .metadata)
        try container.encode(verdictId?.hexUppercased(), forKey: .verdictIdHex)
        try container.encode(attestationNonce?.hexUppercased(), forKey: .attestationNonceHex)
        try container.encode(refreshAtMs, forKey: .refreshAtMs)
    }

    private static func decodeOptionalHash(from container: KeyedDecodingContainer<CodingKeys>,
                                           key: CodingKeys) throws -> Data? {
        if let hex = try container.decodeIfPresent(String.self, forKey: key) {
            guard let data = Data(hexString: hex) else {
                throw OfflineNoritoError.invalidHex(key.stringValue)
            }
            return data
        }
        return nil
    }
}

public struct OfflineWalletCertificate: Codable, Sendable, Equatable {
    public let controller: String
    public let operatorId: String
    public let allowance: OfflineAllowanceCommitment
    public let spendPublicKey: String
    public let attestationReport: Data
    public let issuedAtMs: UInt64
    public let expiresAtMs: UInt64
    public let policy: OfflineWalletPolicy
    public let operatorSignature: Data
    public let metadata: [String: ToriiJSONValue]
    public let verdictId: Data?
    public let attestationNonce: Data?
    public let refreshAtMs: UInt64?

    private enum CodingKeys: String, CodingKey {
        case controller
        case operatorId = "operator"
        case allowance
        case spendPublicKey = "spend_public_key"
        case attestationReportB64 = "attestation_report_b64"
        case issuedAtMs = "issued_at_ms"
        case expiresAtMs = "expires_at_ms"
        case policy
        case operatorSignatureHex = "operator_signature_hex"
        case metadata
        case verdictIdHex = "verdict_id_hex"
        case attestationNonceHex = "attestation_nonce_hex"
        case refreshAtMs = "refresh_at_ms"
    }

    public init(controller: String,
                operatorId: String,
                allowance: OfflineAllowanceCommitment,
                spendPublicKey: String,
                attestationReport: Data,
                issuedAtMs: UInt64,
                expiresAtMs: UInt64,
                policy: OfflineWalletPolicy,
                operatorSignature: Data,
                metadata: [String: ToriiJSONValue] = [:],
                verdictId: Data? = nil,
                attestationNonce: Data? = nil,
                refreshAtMs: UInt64? = nil) {
        self.controller = controller
        self.operatorId = operatorId
        self.allowance = allowance
        self.spendPublicKey = spendPublicKey
        self.attestationReport = attestationReport
        self.issuedAtMs = issuedAtMs
        self.expiresAtMs = expiresAtMs
        self.policy = policy
        self.operatorSignature = operatorSignature
        self.metadata = metadata
        self.verdictId = verdictId
        self.attestationNonce = attestationNonce
        self.refreshAtMs = refreshAtMs
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        controller = try container.decode(String.self, forKey: .controller)
        operatorId = try container.decode(String.self, forKey: .operatorId)
        allowance = try container.decode(OfflineAllowanceCommitment.self, forKey: .allowance)
        spendPublicKey = try container.decode(String.self, forKey: .spendPublicKey)
        let reportB64 = try container.decode(String.self, forKey: .attestationReportB64)
        guard let report = Data(base64Encoded: reportB64) else {
            throw OfflineNoritoError.invalidLength("attestation_report_b64")
        }
        attestationReport = report
        issuedAtMs = try container.decode(UInt64.self, forKey: .issuedAtMs)
        expiresAtMs = try container.decode(UInt64.self, forKey: .expiresAtMs)
        policy = try container.decode(OfflineWalletPolicy.self, forKey: .policy)
        let signatureHex = try container.decode(String.self, forKey: .operatorSignatureHex)
        guard let signature = Data(hexString: signatureHex) else {
            throw OfflineNoritoError.invalidHex("operator_signature_hex")
        }
        operatorSignature = signature
        metadata = try container.decodeIfPresent([String: ToriiJSONValue].self, forKey: .metadata) ?? [:]
        verdictId = try Self.decodeOptionalHash(from: container, key: .verdictIdHex)
        attestationNonce = try Self.decodeOptionalHash(from: container, key: .attestationNonceHex)
        refreshAtMs = try container.decodeIfPresent(UInt64.self, forKey: .refreshAtMs)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(controller, forKey: .controller)
        try container.encode(operatorId, forKey: .operatorId)
        try container.encode(allowance, forKey: .allowance)
        try container.encode(spendPublicKey, forKey: .spendPublicKey)
        try container.encode(attestationReport.base64EncodedString(), forKey: .attestationReportB64)
        try container.encode(issuedAtMs, forKey: .issuedAtMs)
        try container.encode(expiresAtMs, forKey: .expiresAtMs)
        try container.encode(policy, forKey: .policy)
        try container.encode(operatorSignature.hexUppercased(), forKey: .operatorSignatureHex)
        try container.encode(metadata, forKey: .metadata)
        try container.encode(verdictId?.hexUppercased(), forKey: .verdictIdHex)
        try container.encode(attestationNonce?.hexUppercased(), forKey: .attestationNonceHex)
        try container.encode(refreshAtMs, forKey: .refreshAtMs)
    }

    public static func load(from url: URL,
                            decoder: JSONDecoder = JSONDecoder()) throws -> OfflineWalletCertificate {
        let data = try Data(contentsOf: url)
        return try decoder.decode(OfflineWalletCertificate.self, from: data)
    }

    public func noritoPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(try OfflineNorito.encodeAccountId(controller))
        writer.writeField(try OfflineNorito.encodeAccountId(operatorId))
        writer.writeField(try allowance.noritoPayload())
        writer.writeField(OfflineNorito.encodeString(spendPublicKey))
        writer.writeField(OfflineNorito.encodeBytesVec(attestationReport))
        writer.writeField(OfflineNorito.encodeUInt64(issuedAtMs))
        writer.writeField(OfflineNorito.encodeUInt64(expiresAtMs))
        writer.writeField(try policy.noritoPayload())
        writer.writeField(OfflineNorito.encodeConstVec(operatorSignature))
        writer.writeField(try OfflineNorito.encodeMetadata(metadata))
        writer.writeField(try OfflineNorito.encodeOption(verdictId, encode: OfflineNorito.encodeHash))
        writer.writeField(try OfflineNorito.encodeOption(attestationNonce, encode: OfflineNorito.encodeHash))
        writer.writeField(try OfflineNorito.encodeOption(refreshAtMs, encode: OfflineNorito.encodeUInt64))
        return writer.data
    }

    public func noritoEncoded() throws -> Data {
        OfflineNorito.wrap(typeName: Self.noritoTypeName, payload: try noritoPayload())
    }

    public func certificateId() throws -> Data {
        let encoded = try noritoEncoded()
        return IrohaHash.hash(encoded)
    }

    public func certificateIdHex() throws -> String {
        try certificateId().hexUppercased()
    }

    public func operatorSigningBytes() throws -> Data {
        let payload = OfflineWalletCertificatePayload(certificate: self)
        return OfflineNorito.wrap(typeName: OfflineWalletCertificatePayload.noritoTypeName,
                                  payload: try payload.noritoPayload())
    }

    private static func decodeOptionalHash(from container: KeyedDecodingContainer<CodingKeys>,
                                           key: CodingKeys) throws -> Data? {
        if let hex = try container.decodeIfPresent(String.self, forKey: key) {
            guard let data = Data(hexString: hex) else {
                throw OfflineNoritoError.invalidHex(key.stringValue)
            }
            return data
        }
        return nil
    }

    private static let noritoTypeName = "iroha_data_model::offline::model::OfflineWalletCertificate"
}

public struct OfflinePlatformTokenSnapshot: Codable, Sendable, Equatable {
    public let policy: String
    public let attestationJwsB64: String

    private enum CodingKeys: String, CodingKey {
        case policy
        case attestationJwsB64 = "attestation_jws_b64"
    }

    public init(policy: String, attestationJwsB64: String) {
        self.policy = policy
        self.attestationJwsB64 = attestationJwsB64
    }

    func noritoPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(OfflineNorito.encodeString(policy))
        writer.writeField(OfflineNorito.encodeString(attestationJwsB64))
        return writer.data
    }
}

public struct AppleAppAttestProof: Sendable, Equatable {
    public let keyId: String
    public let counter: UInt64
    public let assertion: Data
    public let challengeHash: Data

    public init(keyId: String, counter: UInt64, assertion: Data, challengeHash: Data) {
        self.keyId = keyId
        self.counter = counter
        self.assertion = assertion
        self.challengeHash = challengeHash
    }

    func noritoPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(OfflineNorito.encodeString(keyId))
        writer.writeField(OfflineNorito.encodeUInt64(counter))
        writer.writeField(OfflineNorito.encodeBytesVec(assertion))
        writer.writeField(try OfflineNorito.encodeHash(challengeHash))
        return writer.data
    }
}

public struct AndroidMarkerKeyProof: Sendable, Equatable {
    public let series: String
    public let counter: UInt64
    public let markerPublicKey: Data
    public let markerSignature: Data?
    public let attestation: Data

    public init(series: String,
                counter: UInt64,
                markerPublicKey: Data,
                markerSignature: Data?,
                attestation: Data) {
        self.series = series
        self.counter = counter
        self.markerPublicKey = markerPublicKey
        self.markerSignature = markerSignature
        self.attestation = attestation
    }

    func noritoPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(OfflineNorito.encodeString(series))
        writer.writeField(OfflineNorito.encodeUInt64(counter))
        writer.writeField(OfflineNorito.encodeBytesVec(markerPublicKey))
        writer.writeField(try OfflineNorito.encodeOption(markerSignature, encode: OfflineNorito.encodeBytesVec))
        writer.writeField(OfflineNorito.encodeBytesVec(attestation))
        return writer.data
    }
}

public enum OfflinePlatformProof: Sendable, Equatable {
    case appleAppAttest(AppleAppAttestProof)
    case androidMarkerKey(AndroidMarkerKeyProof)
    case provisioned(AndroidProvisionedProof)

    func noritoPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        switch self {
        case .appleAppAttest(let proof):
            writer.writeUInt32LE(0)
            let payload = try proof.noritoPayload()
            writer.writeLength(UInt64(payload.count))
            writer.writeBytes(payload)
        case .androidMarkerKey(let proof):
            writer.writeUInt32LE(1)
            let payload = try proof.noritoPayload()
            writer.writeLength(UInt64(payload.count))
            writer.writeBytes(payload)
        case .provisioned(let proof):
            writer.writeUInt32LE(2)
            let payload = try proof.noritoPayload()
            writer.writeLength(UInt64(payload.count))
            writer.writeBytes(payload)
        }
        return writer.data
    }
}

public struct OfflineSpendReceipt: Sendable, Equatable {
    public let txId: Data
    public let from: String
    public let to: String
    public let assetId: String
    public let amount: String
    public let issuedAtMs: UInt64
    public let invoiceId: String
    public let platformProof: OfflinePlatformProof
    public let platformSnapshot: OfflinePlatformTokenSnapshot?
    public let senderCertificateId: Data
    public let senderSignature: Data

    public init(txId: Data,
                from: String,
                to: String,
                assetId: String,
                amount: String,
                issuedAtMs: UInt64,
                invoiceId: String,
                platformProof: OfflinePlatformProof,
                platformSnapshot: OfflinePlatformTokenSnapshot?,
                senderCertificateId: Data,
                senderSignature: Data) {
        self.txId = txId
        self.from = from
        self.to = to
        self.assetId = assetId
        self.amount = amount
        self.issuedAtMs = issuedAtMs
        self.invoiceId = invoiceId
        self.platformProof = platformProof
        self.platformSnapshot = platformSnapshot
        self.senderCertificateId = senderCertificateId
        self.senderSignature = senderSignature
    }

    public func signingBytes() throws -> Data {
        let payload = OfflineSpendReceiptPayload(receipt: self)
        return OfflineNorito.wrap(typeName: OfflineSpendReceiptPayload.noritoTypeName,
                                  payload: try payload.noritoPayload())
    }

    public func challengeBytes() throws -> Data {
        let preimage = OfflineReceiptChallengePreimage(
            invoiceId: invoiceId,
            receiverAccountId: to,
            assetId: assetId,
            amount: amount,
            issuedAtMs: issuedAtMs,
            senderCertificateId: senderCertificateId,
            nonce: txId
        )
        return OfflineNorito.wrap(typeName: OfflineReceiptChallengePreimage.noritoTypeName,
                                  payload: try preimage.noritoPayload())
    }

    public func challengeHash(chainId: String) throws -> Data {
        let bytes = try challengeBytes()
        let context = IrohaHash.hash(Data(chainId.utf8))
        var hashInput = Data()
        hashInput.reserveCapacity(context.count + bytes.count)
        hashInput.append(context)
        hashInput.append(bytes)
        return IrohaHash.hash(hashInput)
    }

    public func noritoPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(try OfflineNorito.encodeHash(txId))
        writer.writeField(try OfflineNorito.encodeAccountId(from))
        writer.writeField(try OfflineNorito.encodeAccountId(to))
        writer.writeField(try OfflineNorito.encodeAssetId(assetId))
        writer.writeField(try OfflineNorito.encodeNumeric(amount))
        writer.writeField(OfflineNorito.encodeUInt64(issuedAtMs))
        writer.writeField(OfflineNorito.encodeString(invoiceId))
        writer.writeField(try platformProof.noritoPayload())
        writer.writeField(try OfflineNorito.encodeOption(platformSnapshot, encode: { try $0.noritoPayload() }))
        writer.writeField(try OfflineNorito.encodeHash(senderCertificateId))
        writer.writeField(OfflineNorito.encodeConstVec(senderSignature))
        return writer.data
    }

    public func noritoEncoded() throws -> Data {
        OfflineNorito.wrap(typeName: Self.noritoTypeName, payload: try noritoPayload())
    }

    @available(macOS 10.15, iOS 13.0, *)
    public func signed(with signingKey: SigningKey) throws -> OfflineSpendReceipt {
        let signature = try signingKey.sign(try signingBytes())
        return OfflineSpendReceipt(txId: txId,
                                   from: from,
                                   to: to,
                                   assetId: assetId,
                                   amount: amount,
                                   issuedAtMs: issuedAtMs,
                                   invoiceId: invoiceId,
                                   platformProof: platformProof,
                                   platformSnapshot: platformSnapshot,
                                   senderCertificateId: senderCertificateId,
                                   senderSignature: signature)
    }

    private static let noritoTypeName = "iroha_data_model::offline::model::OfflineSpendReceipt"
}

public struct OfflineBalanceProof: Sendable, Equatable {
    public let initialCommitment: OfflineAllowanceCommitment
    public let resultingCommitment: Data
    public let claimedDelta: String
    public let zkProof: Data?

    public init(initialCommitment: OfflineAllowanceCommitment,
                resultingCommitment: Data,
                claimedDelta: String,
                zkProof: Data? = nil) {
        self.initialCommitment = initialCommitment
        self.resultingCommitment = resultingCommitment
        self.claimedDelta = claimedDelta
        self.zkProof = zkProof
    }

    func noritoPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(try initialCommitment.noritoPayload())
        writer.writeField(OfflineNorito.encodeBytesVec(resultingCommitment))
        writer.writeField(try OfflineNorito.encodeNumeric(claimedDelta))
        writer.writeField(try OfflineNorito.encodeOption(zkProof, encode: OfflineNorito.encodeBytesVec))
        return writer.data
    }

    public func noritoEncoded() throws -> Data {
        OfflineNorito.wrap(typeName: Self.noritoTypeName, payload: try noritoPayload())
    }

    private static let noritoTypeName = "iroha_data_model::offline::model::OfflineBalanceProof"
}

public struct OfflineCertificateBalanceProof: Sendable, Equatable {
    public let senderCertificateId: Data
    public let balanceProof: OfflineBalanceProof

    public init(senderCertificateId: Data, balanceProof: OfflineBalanceProof) {
        self.senderCertificateId = senderCertificateId
        self.balanceProof = balanceProof
    }

    func noritoPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(try OfflineNorito.encodeHash(senderCertificateId))
        writer.writeField(try balanceProof.noritoPayload())
        return writer.data
    }

    public func noritoEncoded() throws -> Data {
        OfflineNorito.wrap(typeName: Self.noritoTypeName, payload: try noritoPayload())
    }

    private static let noritoTypeName = "iroha_data_model::offline::model::OfflineCertificateBalanceProof"
}

public struct OfflinePoseidonDigest: Sendable, Equatable {
    public let bytes: Data

    public init(bytes: Data) {
        self.bytes = bytes
    }

    public init(hex: String) throws {
        guard let data = Data(hexString: hex) else {
            throw OfflineNoritoError.invalidHex("poseidon digest")
        }
        bytes = data
    }

    func noritoPayload() throws -> Data {
        try OfflineNorito.encodePoseidonDigest(bytes)
    }
}

/// Known metadata keys used by aggregate proof envelopes.
public enum OfflineAggregateProofMetadataKey {
    public static let parameterSet = "fastpq.parameter_set"
    public static let sumCircuit = "fastpq.circuit.sum"
    public static let counterCircuit = "fastpq.circuit.counter"
    public static let replayCircuit = "fastpq.circuit.replay"
    public static let aggregateBackend = "offline.aggregate.backend"
    public static let aggregateCircuitId = "offline.aggregate.circuit_id"
    public static let aggregatePublicInputsBase64 = "offline.aggregate.public_inputs_b64"
    public static let aggregateRecursionDepth = "offline.aggregate.recursion_depth"

    public static let recursiveStarkBackend = "stark/fri-v1/poseidon2-goldilocks-v1"
}

public enum OfflineAggregateProofVersion {
    public static let legacy: UInt16 = 1
    public static let recursiveStarkV2: UInt16 = 2
}

public struct OfflineAggregateProofEnvelope: Sendable, Equatable {
    public let version: UInt16
    public let receiptsRoot: OfflinePoseidonDigest
    public let proofSum: Data?
    public let proofCounter: Data?
    public let proofReplay: Data?
    public let metadata: [String: ToriiJSONValue]

    public init(version: UInt16,
                receiptsRoot: OfflinePoseidonDigest,
                proofSum: Data? = nil,
                proofCounter: Data? = nil,
                proofReplay: Data? = nil,
                metadata: [String: ToriiJSONValue] = [:]) {
        self.version = version
        self.receiptsRoot = receiptsRoot
        self.proofSum = proofSum
        self.proofCounter = proofCounter
        self.proofReplay = proofReplay
        self.metadata = metadata
    }

    func noritoPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(OfflineNorito.encodeUInt16(version))
        writer.writeField(try receiptsRoot.noritoPayload())
        writer.writeField(try OfflineNorito.encodeOption(proofSum, encode: OfflineNorito.encodeBytesVec))
        writer.writeField(try OfflineNorito.encodeOption(proofCounter, encode: OfflineNorito.encodeBytesVec))
        writer.writeField(try OfflineNorito.encodeOption(proofReplay, encode: OfflineNorito.encodeBytesVec))
        writer.writeField(try OfflineNorito.encodeMetadata(metadata))
        return writer.data
    }

    public func noritoEncoded() throws -> Data {
        OfflineNorito.wrap(typeName: Self.noritoTypeName, payload: try noritoPayload())
    }

    private static let noritoTypeName = "iroha_data_model::offline::poseidon::AggregateProofEnvelope"
}

public struct OfflineProofAttachmentList: Sendable, Equatable {
    /// Ordered proof attachments included with an offline bundle.
    public let attachments: [ProofAttachment]

    public init(attachments: [ProofAttachment]) {
        self.attachments = attachments
    }

    func noritoPayload() throws -> Data {
        try OfflineNorito.encodeVec(attachments, encode: { try $0.noritoPayload() })
    }

    public func noritoEncoded() throws -> Data {
        OfflineNorito.wrap(typeName: Self.noritoTypeName, payload: try noritoPayload())
    }

    private static let noritoTypeName = "iroha_data_model::proof::ProofAttachmentList"
}

public struct OfflineToOnlineTransfer: Sendable, Equatable {
    public let bundleId: Data
    public let receiver: String
    public let depositAccount: String
    public let receipts: [OfflineSpendReceipt]
    public let balanceProof: OfflineBalanceProof
    public let balanceProofs: [OfflineCertificateBalanceProof]?
    public let aggregateProof: OfflineAggregateProofEnvelope?
    public let attachments: OfflineProofAttachmentList?
    public let platformSnapshot: OfflinePlatformTokenSnapshot?

    public init(bundleId: Data,
                receiver: String,
                depositAccount: String,
                receipts: [OfflineSpendReceipt],
                balanceProof: OfflineBalanceProof,
                balanceProofs: [OfflineCertificateBalanceProof]? = nil,
                aggregateProof: OfflineAggregateProofEnvelope? = nil,
                attachments: OfflineProofAttachmentList? = nil,
                platformSnapshot: OfflinePlatformTokenSnapshot? = nil) {
        self.bundleId = bundleId
        self.receiver = receiver
        self.depositAccount = depositAccount
        self.receipts = receipts
        self.balanceProof = balanceProof
        self.balanceProofs = balanceProofs
        self.aggregateProof = aggregateProof
        self.attachments = attachments
        self.platformSnapshot = platformSnapshot
    }

    func noritoPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(try OfflineNorito.encodeHash(bundleId))
        writer.writeField(try OfflineNorito.encodeAccountId(receiver))
        writer.writeField(try OfflineNorito.encodeAccountId(depositAccount))
        writer.writeField(try OfflineNorito.encodeVec(receipts, encode: { try $0.noritoPayload() }))
        writer.writeField(try balanceProof.noritoPayload())
        writer.writeField(try OfflineNorito.encodeOption(
            balanceProofs,
            encode: { proofs in
                try OfflineNorito.encodeVec(proofs, encode: { try $0.noritoPayload() })
            }
        ))
        writer.writeField(try OfflineNorito.encodeOption(aggregateProof, encode: { try $0.noritoPayload() }))
        writer.writeField(try OfflineNorito.encodeOption(attachments, encode: { try $0.noritoPayload() }))
        writer.writeField(try OfflineNorito.encodeOption(platformSnapshot, encode: { try $0.noritoPayload() }))
        return writer.data
    }

    public func noritoEncoded() throws -> Data {
        OfflineNorito.wrap(typeName: Self.noritoTypeName, payload: try noritoPayload())
    }

    private static let noritoTypeName = "iroha_data_model::offline::model::OfflineToOnlineTransfer"
}

struct OfflineWalletCertificatePayload {
    let controller: String
    let operatorId: String
    let allowance: OfflineAllowanceCommitment
    let spendPublicKey: String
    let attestationReport: Data
    let issuedAtMs: UInt64
    let expiresAtMs: UInt64
    let policy: OfflineWalletPolicy
    let metadata: [String: ToriiJSONValue]
    let verdictId: Data?
    let attestationNonce: Data?
    let refreshAtMs: UInt64?

    init(certificate: OfflineWalletCertificate) {
        controller = certificate.controller
        operatorId = certificate.operatorId
        allowance = certificate.allowance
        spendPublicKey = certificate.spendPublicKey
        attestationReport = certificate.attestationReport
        issuedAtMs = certificate.issuedAtMs
        expiresAtMs = certificate.expiresAtMs
        policy = certificate.policy
        metadata = certificate.metadata
        verdictId = certificate.verdictId
        attestationNonce = certificate.attestationNonce
        refreshAtMs = certificate.refreshAtMs
    }

    func noritoPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(try OfflineNorito.encodeAccountId(controller))
        writer.writeField(try OfflineNorito.encodeAccountId(operatorId))
        writer.writeField(try allowance.noritoPayload())
        writer.writeField(OfflineNorito.encodeString(spendPublicKey))
        writer.writeField(OfflineNorito.encodeBytesVec(attestationReport))
        writer.writeField(OfflineNorito.encodeUInt64(issuedAtMs))
        writer.writeField(OfflineNorito.encodeUInt64(expiresAtMs))
        writer.writeField(try policy.noritoPayload())
        writer.writeField(try OfflineNorito.encodeMetadata(metadata))
        writer.writeField(try OfflineNorito.encodeOption(verdictId, encode: OfflineNorito.encodeHash))
        writer.writeField(try OfflineNorito.encodeOption(attestationNonce, encode: OfflineNorito.encodeHash))
        writer.writeField(try OfflineNorito.encodeOption(refreshAtMs, encode: OfflineNorito.encodeUInt64))
        return writer.data
    }

    static let noritoTypeName = "iroha_data_model::offline::OfflineWalletCertificatePayload"
}

struct OfflineSpendReceiptPayload {
    let txId: Data
    let from: String
    let to: String
    let assetId: String
    let amount: String
    let issuedAtMs: UInt64
    let invoiceId: String
    let platformProof: OfflinePlatformProof
    let senderCertificateId: Data

    init(receipt: OfflineSpendReceipt) {
        txId = receipt.txId
        from = receipt.from
        to = receipt.to
        assetId = receipt.assetId
        amount = receipt.amount
        issuedAtMs = receipt.issuedAtMs
        invoiceId = receipt.invoiceId
        platformProof = receipt.platformProof
        senderCertificateId = receipt.senderCertificateId
    }

    func noritoPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(try OfflineNorito.encodeHash(txId))
        writer.writeField(try OfflineNorito.encodeAccountId(from))
        writer.writeField(try OfflineNorito.encodeAccountId(to))
        writer.writeField(try OfflineNorito.encodeAssetId(assetId))
        writer.writeField(try OfflineNorito.encodeNumeric(amount))
        writer.writeField(OfflineNorito.encodeUInt64(issuedAtMs))
        writer.writeField(OfflineNorito.encodeString(invoiceId))
        writer.writeField(try platformProof.noritoPayload())
        writer.writeField(try OfflineNorito.encodeHash(senderCertificateId))
        return writer.data
    }

    static let noritoTypeName = "iroha_data_model::offline::OfflineSpendReceiptPayload"
}

struct OfflineReceiptChallengePreimage {
    let invoiceId: String
    let receiverAccountId: String
    let assetId: String
    let amount: String
    let issuedAtMs: UInt64
    let senderCertificateId: Data
    let nonce: Data

    func noritoPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(OfflineNorito.encodeString(invoiceId))
        writer.writeField(try OfflineNorito.encodeAccountId(receiverAccountId))
        writer.writeField(try OfflineNorito.encodeAssetId(assetId))
        writer.writeField(try OfflineNorito.encodeNumeric(amount))
        writer.writeField(OfflineNorito.encodeUInt64(issuedAtMs))
        writer.writeField(try OfflineNorito.encodeHash(senderCertificateId))
        writer.writeField(try OfflineNorito.encodeHash(nonce))
        return writer.data
    }

    static let noritoTypeName = "iroha_data_model::offline::OfflineReceiptChallengePreimage"
}
