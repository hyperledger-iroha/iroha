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
    public let allowance: OfflineAllowanceCommitment
    public let spendPublicKey: String
    public let attestationReport: Data
    public let issuedAtMs: UInt64
    public let expiresAtMs: UInt64
    public let policy: OfflineWalletPolicy
    let metadata: [String: ToriiJSONValue]
    public let verdictId: Data?
    public let attestationNonce: Data?
    public let refreshAtMs: UInt64?

    private enum CodingKeys: String, CodingKey {
        case controller
        case legacyOperatorId = "operator"
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

    /// Typed metadata initializer — the primary public API.
    public init(controller: String,
                allowance: OfflineAllowanceCommitment,
                spendPublicKey: String,
                attestationReport: Data,
                issuedAtMs: UInt64,
                expiresAtMs: UInt64,
                policy: OfflineWalletPolicy,
                iosAppAttest: OfflineIOSAppAttestMetadata? = nil,
                lineage: OfflineCertificateLineage? = nil,
                verdictId: Data? = nil,
                attestationNonce: Data? = nil,
                refreshAtMs: UInt64? = nil) {
        var metadata: [String: ToriiJSONValue] = [:]
        iosAppAttest?.apply(to: &metadata)
        lineage?.apply(to: &metadata)
        self.init(controller: controller, allowance: allowance,
                  spendPublicKey: spendPublicKey, attestationReport: attestationReport,
                  issuedAtMs: issuedAtMs, expiresAtMs: expiresAtMs, policy: policy,
                  metadata: metadata, verdictId: verdictId, attestationNonce: attestationNonce,
                  refreshAtMs: refreshAtMs)
    }

    @available(*, deprecated, message: "operatorId is derived by Torii and ignored for drafts.")
    public init(controller: String,
                operatorId: String,
                allowance: OfflineAllowanceCommitment,
                spendPublicKey: String,
                attestationReport: Data,
                issuedAtMs: UInt64,
                expiresAtMs: UInt64,
                policy: OfflineWalletPolicy,
                iosAppAttest: OfflineIOSAppAttestMetadata? = nil,
                lineage: OfflineCertificateLineage? = nil,
                verdictId: Data? = nil,
                attestationNonce: Data? = nil,
                refreshAtMs: UInt64? = nil) {
        let _ = operatorId
        self.init(controller: controller,
                  allowance: allowance,
                  spendPublicKey: spendPublicKey,
                  attestationReport: attestationReport,
                  issuedAtMs: issuedAtMs,
                  expiresAtMs: expiresAtMs,
                  policy: policy,
                  iosAppAttest: iosAppAttest,
                  lineage: lineage,
                  verdictId: verdictId,
                  attestationNonce: attestationNonce,
                  refreshAtMs: refreshAtMs)
    }

    /// Raw metadata initializer — for internal SDK use.
    init(controller: String,
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

    @available(*, deprecated, message: "operatorId is derived by Torii and ignored for drafts.")
    init(controller: String,
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
        let _ = operatorId
        self.init(controller: controller,
                  allowance: allowance,
                  spendPublicKey: spendPublicKey,
                  attestationReport: attestationReport,
                  issuedAtMs: issuedAtMs,
                  expiresAtMs: expiresAtMs,
                  policy: policy,
                  metadata: metadata,
                  verdictId: verdictId,
                  attestationNonce: attestationNonce,
                  refreshAtMs: refreshAtMs)
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        controller = try container.decode(String.self, forKey: .controller)
        _ = try container.decodeIfPresent(String.self, forKey: .legacyOperatorId)
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

    /// Parse draft metadata into typed platform-agnostic models.
    public func parsedMetadata() -> OfflineCertificateParsedMetadata {
        OfflineCertificateParsedMetadata(from: metadata)
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(controller, forKey: .controller)
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

// MARK: - iOS App Attest Metadata

/// Typed representation of `ios.app_attest.*` metadata fields.
///
///     let attest = OfflineIOSAppAttestMetadata(teamId: "ABCD1234", bundleId: "com.app", environment: "production")
///     attest.apply(to: &metadata)
public struct OfflineIOSAppAttestMetadata: Codable, Sendable, Equatable {
    public let teamId: String
    public let bundleId: String
    public let environment: String

    public init(teamId: String, bundleId: String, environment: String) {
        self.teamId = teamId
        self.bundleId = bundleId
        self.environment = environment
    }

    /// Parse from an existing certificate's metadata dictionary.
    init?(from metadata: [String: ToriiJSONValue]) {
        guard case .string(let teamId) = metadata[Key.teamId] else { return nil }
        let bundleId: String
        if case .string(let s) = metadata[Key.bundleId] { bundleId = s } else { return nil }
        let environment: String
        if case .string(let s) = metadata[Key.environment] { environment = s } else { return nil }
        self.init(teamId: teamId, bundleId: bundleId, environment: environment)
    }

    /// Write attest fields into a metadata dictionary, preserving other keys.
    func apply(to metadata: inout [String: ToriiJSONValue]) {
        metadata[Key.teamId] = .string(teamId)
        metadata[Key.bundleId] = .string(bundleId)
        metadata[Key.environment] = .string(environment)
    }

    private enum Key {
        static let teamId = "ios.app_attest.team_id"
        static let bundleId = "ios.app_attest.bundle_id"
        static let environment = "ios.app_attest.environment"
    }
}

// MARK: - Certificate Lineage

/// Typed representation of `offline.lineage.*` and `offline.build_claim.*` metadata fields.
///
/// Use instead of manually constructing `[String: ToriiJSONValue]` entries with string keys.
///
///     // First issuance (no previous allowance):
///     let lineage = OfflineCertificateLineage(scope: bundleId, epoch: 1, minBuildNumber: "28")
///     lineage.apply(to: &metadata)
///
///     // Renewal / reprovision (previous allowance exists):
///     let prev = OfflineCertificateLineage(from: oldCertificate.metadata)
///     let lineage = OfflineCertificateLineage(
///         scope: bundleId,
///         epoch: (prev?.epoch ?? 0) + 1,
///         prevCertificateIdHex: oldAllowance.certificateIdHex,
///         minBuildNumber: "28"
///     )
///     lineage.apply(to: &metadata)
public struct OfflineCertificateLineage: Codable, Sendable, Equatable {
    public let scope: String
    public let epoch: UInt64
    public let prevCertificateIdHex: String?
    public let minBuildNumber: String?

    public init(scope: String, epoch: UInt64, prevCertificateIdHex: String? = nil, minBuildNumber: String? = nil) {
        self.scope = scope
        self.epoch = epoch
        self.prevCertificateIdHex = prevCertificateIdHex
        self.minBuildNumber = minBuildNumber
    }

    /// Parse lineage from an existing certificate's metadata dictionary.
    /// Returns `nil` if `offline.lineage.epoch` is missing or unparseable.
    init?(from metadata: [String: ToriiJSONValue]) {
        guard let epochValue = metadata[Key.epoch] else { return nil }
        let epochNum: UInt64
        switch epochValue {
        case .string(let s):
            guard let parsed = UInt64(s) else { return nil }
            epochNum = parsed
        case .number(let n):
            epochNum = UInt64(n)
        default:
            return nil
        }
        let scope: String
        if case .string(let s) = metadata[Key.scope] {
            scope = s
        } else {
            return nil
        }
        var prevHex: String?
        if case .string(let s) = metadata[Key.prevCertificateIdHex] {
            prevHex = s
        }
        var buildNum: String?
        if case .string(let s) = metadata[Key.minBuildNumber] {
            buildNum = s
        }
        self.init(scope: scope, epoch: epochNum, prevCertificateIdHex: prevHex, minBuildNumber: buildNum)
    }

    /// Write lineage fields into a metadata dictionary, preserving other keys.
    func apply(to metadata: inout [String: ToriiJSONValue]) {
        metadata[Key.scope] = .string(scope)
        metadata[Key.epoch] = .string("\(epoch)")
        if let prevCertificateIdHex {
            metadata[Key.prevCertificateIdHex] = .string(prevCertificateIdHex)
        } else {
            metadata.removeValue(forKey: Key.prevCertificateIdHex)
        }
        if let minBuildNumber {
            metadata[Key.minBuildNumber] = .string(minBuildNumber)
        }
    }

    private enum Key {
        static let scope = "offline.lineage.scope"
        static let epoch = "offline.lineage.epoch"
        static let prevCertificateIdHex = "offline.lineage.prev_certificate_id_hex"
        static let minBuildNumber = "offline.build_claim.min_build_number"
    }
}

// MARK: - Android Attestation Metadata

public struct OfflineAndroidMarkerKeyMetadata: Codable, Sendable, Equatable {
    public let packageNames: [String]
    public let signingDigestsSha256: [String]
    public let requireStrongbox: Bool
    public let requireRollbackResistance: Bool

    public init(packageNames: [String], signingDigestsSha256: [String],
                requireStrongbox: Bool, requireRollbackResistance: Bool) {
        self.packageNames = packageNames
        self.signingDigestsSha256 = signingDigestsSha256
        self.requireStrongbox = requireStrongbox
        self.requireRollbackResistance = requireRollbackResistance
    }

    init?(from metadata: [String: ToriiJSONValue]) {
        let packages = metadata[Key.packageNames]?.normalizedStringArray ?? []
        let digests = metadata[Key.signingDigestsSha256]?.normalizedStringArray ?? []
        guard !packages.isEmpty, !digests.isEmpty else { return nil }
        self.init(
            packageNames: packages,
            signingDigestsSha256: digests,
            requireStrongbox: metadata[Key.requireStrongbox]?.normalizedBool ?? false,
            requireRollbackResistance: metadata[Key.requireRollbackResistance]?.normalizedBool ?? true
        )
    }

    private enum Key {
        static let packageNames = "android.attestation.package_names"
        static let signingDigestsSha256 = "android.attestation.signing_digests_sha256"
        static let requireStrongbox = "android.attestation.require_strongbox"
        static let requireRollbackResistance = "android.attestation.require_rollback_resistance"
    }
}

public struct OfflineAndroidPlayIntegrityMetadata: Codable, Sendable, Equatable {
    public let cloudProjectNumber: UInt64?
    public let environment: String?
    public let packageNames: [String]
    public let signingDigestsSha256: [String]
    public let allowedAppVerdicts: [String]
    public let allowedDeviceVerdicts: [String]
    public let maxTokenAgeMs: UInt64?

    public init(cloudProjectNumber: UInt64?, environment: String?,
                packageNames: [String], signingDigestsSha256: [String],
                allowedAppVerdicts: [String], allowedDeviceVerdicts: [String],
                maxTokenAgeMs: UInt64?) {
        self.cloudProjectNumber = cloudProjectNumber
        self.environment = environment
        self.packageNames = packageNames
        self.signingDigestsSha256 = signingDigestsSha256
        self.allowedAppVerdicts = allowedAppVerdicts
        self.allowedDeviceVerdicts = allowedDeviceVerdicts
        self.maxTokenAgeMs = maxTokenAgeMs
    }

    init?(from metadata: [String: ToriiJSONValue]) {
        self.init(
            cloudProjectNumber: metadata[Key.cloudProjectNumber]?.normalizedUInt64,
            environment: metadata[Key.environment]?.normalizedString,
            packageNames: metadata[Key.packageNames]?.normalizedStringArray ?? [],
            signingDigestsSha256: metadata[Key.signingDigestsSha256]?.normalizedStringArray ?? [],
            allowedAppVerdicts: metadata[Key.allowedAppVerdicts]?.normalizedStringArray ?? [],
            allowedDeviceVerdicts: metadata[Key.allowedDeviceVerdicts]?.normalizedStringArray ?? [],
            maxTokenAgeMs: metadata[Key.maxTokenAgeMs]?.normalizedUInt64
        )
    }

    private enum Key {
        static let cloudProjectNumber = "android.play_integrity.cloud_project_number"
        static let environment = "android.play_integrity.environment"
        static let packageNames = "android.play_integrity.package_names"
        static let signingDigestsSha256 = "android.play_integrity.signing_digests_sha256"
        static let allowedAppVerdicts = "android.play_integrity.allowed_app_verdicts"
        static let allowedDeviceVerdicts = "android.play_integrity.allowed_device_verdicts"
        static let maxTokenAgeMs = "android.play_integrity.max_token_age_ms"
    }
}

public struct OfflineAndroidHmsSafetyDetectMetadata: Codable, Sendable, Equatable {
    public let appId: String?
    public let packageNames: [String]
    public let signingDigestsSha256: [String]
    public let requiredEvaluations: [String]
    public let maxTokenAgeMs: UInt64?

    public init(appId: String?, packageNames: [String], signingDigestsSha256: [String],
                requiredEvaluations: [String], maxTokenAgeMs: UInt64?) {
        self.appId = appId
        self.packageNames = packageNames
        self.signingDigestsSha256 = signingDigestsSha256
        self.requiredEvaluations = requiredEvaluations
        self.maxTokenAgeMs = maxTokenAgeMs
    }

    init?(from metadata: [String: ToriiJSONValue]) {
        let packages = metadata[Key.packageNames]?.normalizedStringArray ?? []
        let digests = metadata[Key.signingDigestsSha256]?.normalizedStringArray ?? []
        guard !packages.isEmpty, !digests.isEmpty else { return nil }
        self.init(
            appId: metadata[Key.appId]?.normalizedString,
            packageNames: packages,
            signingDigestsSha256: digests,
            requiredEvaluations: metadata[Key.requiredEvaluations]?.normalizedStringArray ?? [],
            maxTokenAgeMs: metadata[Key.maxTokenAgeMs]?.normalizedUInt64
        )
    }

    private enum Key {
        static let appId = "android.hms_safety_detect.app_id"
        static let packageNames = "android.hms_safety_detect.package_names"
        static let signingDigestsSha256 = "android.hms_safety_detect.signing_digests_sha256"
        static let requiredEvaluations = "android.hms_safety_detect.required_evaluations"
        static let maxTokenAgeMs = "android.hms_safety_detect.max_token_age_ms"
    }
}

public struct OfflineAndroidProvisionedMetadata: Codable, Sendable, Equatable {
    public let inspectorPublicKey: String?
    public let manifestSchema: String?
    public let manifestVersion: Int?
    public let maxManifestAgeMs: UInt64?
    public let manifestDigestHex: String?

    public init(inspectorPublicKey: String?, manifestSchema: String?,
                manifestVersion: Int?, maxManifestAgeMs: UInt64?, manifestDigestHex: String?) {
        self.inspectorPublicKey = inspectorPublicKey
        self.manifestSchema = manifestSchema
        self.manifestVersion = manifestVersion
        self.maxManifestAgeMs = maxManifestAgeMs
        self.manifestDigestHex = manifestDigestHex
    }

    init?(from metadata: [String: ToriiJSONValue]) {
        self.init(
            inspectorPublicKey: metadata[Key.inspectorPublicKey]?.normalizedString,
            manifestSchema: metadata[Key.manifestSchema]?.normalizedString,
            manifestVersion: metadata[Key.manifestVersion]?.normalizedInt,
            maxManifestAgeMs: metadata[Key.maxManifestAgeMs]?.normalizedUInt64,
            manifestDigestHex: metadata[Key.manifestDigest]?.normalizedString
        )
    }

    private enum Key {
        static let inspectorPublicKey = "android.provisioned.inspector_public_key"
        static let manifestSchema = "android.provisioned.manifest_schema"
        static let manifestVersion = "android.provisioned.manifest_version"
        static let maxManifestAgeMs = "android.provisioned.max_manifest_age_ms"
        static let manifestDigest = "android.provisioned.manifest_digest"
    }
}

// MARK: - Parsed Certificate Metadata

/// Complete parsed representation of a certificate's metadata dictionary.
///
/// Use this to parse a raw `[String: ToriiJSONValue]` metadata dict into typed fields.
/// All Iroha protocol string keys are encapsulated here — callers never need raw key strings.
public struct OfflineCertificateParsedMetadata: Sendable, Equatable {
    public let platformPolicy: ToriiPlatformPolicy?
    public let iosAppAttest: OfflineIOSAppAttestMetadata?
    public let lineage: OfflineCertificateLineage?
    public let androidMarkerKey: OfflineAndroidMarkerKeyMetadata?
    public let androidPlayIntegrity: OfflineAndroidPlayIntegrityMetadata?
    public let androidHmsSafetyDetect: OfflineAndroidHmsSafetyDetectMetadata?
    public let androidProvisioned: OfflineAndroidProvisionedMetadata?

    init(from metadata: [String: ToriiJSONValue]) {
        let policy = metadata["android.integrity.policy"]?.normalizedString
            .flatMap(ToriiPlatformPolicy.init(rawValue:))
        self.platformPolicy = policy
        self.iosAppAttest = OfflineIOSAppAttestMetadata(from: metadata)
        self.lineage = OfflineCertificateLineage(from: metadata)
        self.androidMarkerKey = OfflineAndroidMarkerKeyMetadata(from: metadata)
        switch policy {
        case .playIntegrity:
            self.androidPlayIntegrity = OfflineAndroidPlayIntegrityMetadata(from: metadata)
            self.androidHmsSafetyDetect = nil
            self.androidProvisioned = nil
        case .hmsSafetyDetect:
            self.androidPlayIntegrity = nil
            self.androidHmsSafetyDetect = OfflineAndroidHmsSafetyDetectMetadata(from: metadata)
            self.androidProvisioned = nil
        case .provisioned:
            self.androidPlayIntegrity = nil
            self.androidHmsSafetyDetect = nil
            self.androidProvisioned = OfflineAndroidProvisionedMetadata(from: metadata)
        case .markerKey, nil:
            self.androidPlayIntegrity = nil
            self.androidHmsSafetyDetect = nil
            self.androidProvisioned = nil
        }
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
