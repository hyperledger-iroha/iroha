import CryptoKit
import Foundation

/// Strict failure surface for the first-release Exact12 capability contract.
public enum PrivacyExact12CapabilityManifestErrorV1: Error, LocalizedError, Equatable, Sendable {
    case nativeUnavailable
    case invalidArchive(String)
    case unavailableProtocol(PrivacyProtocolIdV1)
    case compiledTupleMismatch(PrivacyProtocolIdV1)
    case invalidAdmission

    public var errorDescription: String? {
        switch self {
        case .nativeUnavailable:
            return "Exact12 capability admission requires the exact ABI23 native bridge."
        case let .invalidArchive(reason):
            return "Exact12 capability manifest is invalid: \(reason)"
        case let .unavailableProtocol(protocolId):
            return "Exact12 protocol \(protocolId.rawValue) is not active and ready in committed state."
        case let .compiledTupleMismatch(protocolId):
            return "Exact12 protocol \(protocolId.rawValue) differs from this binary's compiled profile tuple."
        case .invalidAdmission:
            return "Exact12 capability admission is missing, stale, or protocol-substituted."
        }
    }
}

/// Closed operation schema in the canonical protocol order.
public enum PrivacyOperationSchemaV1: UInt32, CaseIterable, Sendable {
    case zkAceAuthorizationActionV1 = 0
    case anonymousPgcPaymentActionV1 = 1
    case veRangeRangeProofV1 = 2
    case zkAmsAdmissionAndProvisioningV1 = 3
    case vegaCredentialPresentationV1 = 4
    case zkX509IdentityPresentationV1 = 5
    case jindoPolynomialEvaluationV1 = 6
    case bootleLanternCredentialPresentationV1 = 7
    case orchardNoteActionV1 = 8
    case fcmpMembershipPaymentV1 = 9
    case ivmPrivateNoteActionV1 = 10
    case pqMaspNoteActionV1 = 11

    public var canonicalLabel: String {
        switch self {
        case .zkAceAuthorizationActionV1: return "zk_ace_authorization_action_v1"
        case .anonymousPgcPaymentActionV1: return "anonymous_pgc_payment_action_v1"
        case .veRangeRangeProofV1: return "verange_range_proof_v1"
        case .zkAmsAdmissionAndProvisioningV1:
            return "zk_ams_admission_and_provisioning_v1"
        case .vegaCredentialPresentationV1: return "vega_credential_presentation_v1"
        case .zkX509IdentityPresentationV1: return "zk_x509_identity_presentation_v1"
        case .jindoPolynomialEvaluationV1: return "jindo_polynomial_evaluation_v1"
        case .bootleLanternCredentialPresentationV1:
            return "bootle_lantern_credential_presentation_v1"
        case .orchardNoteActionV1: return "orchard_note_action_v1"
        case .fcmpMembershipPaymentV1: return "fcmp_membership_payment_v1"
        case .ivmPrivateNoteActionV1: return "ivm_private_note_action_v1"
        case .pqMaspNoteActionV1: return "pq_masp_note_action_v1"
        }
    }
}

/// Closed execution classification carried by a committed row.
public enum PrivacyExecutionModeV1: UInt32, CaseIterable, Sendable {
    case authorizationAction = 0
    case paymentAction = 1
    case component = 2
    case admissionAction = 3
    case presentationAction = 4
    case noteAction = 5

    public var canonicalLabel: String {
        switch self {
        case .authorizationAction: return "authorization_action"
        case .paymentAction: return "payment_action"
        case .component: return "component"
        case .admissionAction: return "admission_action"
        case .presentationAction: return "presentation_action"
        case .noteAction: return "note_action"
        }
    }
}

public enum PrivacyCompiledProfileUnavailableReasonV1: Equatable, Sendable {
    case engineUnavailable
    case profileInitializationFailed
    case statementSchemaInvalid(conflictingStableTypeId: Bool)
}

public struct PrivacyProtocolActivationLimitsV1: Equatable, Sendable {
    public let protocolId: PrivacyProtocolIdV1
    public let values: [UInt32]
    let canonicalNorito: Data
}

/// Complete immutable profile/schema tuple compiled into a retained protocol.
public struct PrivacyCompiledProfileSnapshotV1: Equatable, Sendable {
    public let protocolId: PrivacyProtocolIdV1
    public let proofSystemId: PrivacyProofSystemIdV1
    public let engineId: PrivacyEngineIdV1
    public let parameterId: Data
    public let parameterDigest: Data
    public let verifierDigest: Data
    public let statementSchemaDigest: Data
    public let engineManifestDigest: Data
    public let protocolLimits: PrivacyProtocolActivationLimitsV1
}

public enum PrivacyCompiledProfileResultV1: Equatable, Sendable {
    case available(PrivacyCompiledProfileSnapshotV1)
    case unavailable(PrivacyCompiledProfileUnavailableReasonV1)
}

public enum PrivacyCapabilityReadinessV1: Equatable, Sendable {
    case available
    case availableExperimental
    case unavailable(PrivacyCompiledProfileUnavailableReasonV1)
}

public enum PrivacyCapabilityActivationStateV1: UInt32, CaseIterable, Sendable {
    case notRegistered = 0
    case proposed = 1
    case active = 2
    case suspended = 3
    case retired = 4
}

public enum PrivacyCapabilityLimitationV1: UInt32, Sendable {
    case missingDistributionWideKnowledgeSoundnessEvidence = 0
}

public struct PrivacyConsensusLimitsV1: Equatable, Sendable {
    public let maxActionsPerTransaction: UInt32
    public let maxActionsPerBlock: UInt32
    public let maxProofBytesPerAction: UInt32
    public let maxActionBytes: UInt32
    public let maxPrivacyBytesPerTransaction: UInt32
    public let maxPrivacyBytesPerBlock: UInt32
    public let maxStatementAndEncryptedOutputBytesPerTransaction: UInt32
    public let maxNullifiersPerAction: UInt32
    public let maxCommitmentsPerAction: UInt32
    public let retainedRootCount: UInt32

    var orderedValues: [UInt32] {
        [
            maxActionsPerTransaction, maxActionsPerBlock, maxProofBytesPerAction,
            maxActionBytes, maxPrivacyBytesPerTransaction, maxPrivacyBytesPerBlock,
            maxStatementAndEncryptedOutputBytesPerTransaction,
            maxNullifiersPerAction, maxCommitmentsPerAction, retainedRootCount,
        ]
    }
}

public struct PrivacyConsensusPolicyTighteningV1: Equatable, Sendable {
    public let scheduledAtHeight: UInt64
    public let effectiveAtHeight: UInt64
    public let nextLimits: PrivacyConsensusLimitsV1
}

public struct PrivacyConsensusPolicyV1: Equatable, Sendable {
    public let currentLimits: PrivacyConsensusLimitsV1
    public let pendingTightening: PrivacyConsensusPolicyTighteningV1?
    /// Exact canonical nested value retained from Torii.
    public let canonicalNorito: Data
}

public enum PrivacyProtocolLifecycleV1: Equatable, Sendable {
    case proposed(proposedAtHeight: UInt64, activateAtHeight: UInt64)
    case active(proposedAtHeight: UInt64, activatedAtHeight: UInt64, stateSinceHeight: UInt64)
    case suspended(proposedAtHeight: UInt64, activatedAtHeight: UInt64, stateSinceHeight: UInt64)
    case retired(proposedAtHeight: UInt64, activatedAtHeight: UInt64?, stateSinceHeight: UInt64)
}

public struct PrivacyProtocolLimitsTighteningV1: Equatable, Sendable {
    public let scheduledAtHeight: UInt64
    public let effectiveAtHeight: UInt64
    public let nextLimits: PrivacyProtocolActivationLimitsV1
}

/// Full committed activation record, including all profile/schema bindings.
public struct PrivacyProtocolActivationRecordV1: Equatable, Sendable {
    public let protocolId: PrivacyProtocolIdV1
    public let proofSystemId: PrivacyProofSystemIdV1
    public let engineId: PrivacyEngineIdV1
    public let parameterId: Data
    public let parameterDigest: Data
    public let verifierDigest: Data
    public let statementSchemaDigest: Data
    public let engineManifestDigest: Data
    public let lifecycle: PrivacyProtocolLifecycleV1
    public let protocolLimits: PrivacyProtocolActivationLimitsV1
    public let pendingProtocolLimitsTightening: PrivacyProtocolLimitsTighteningV1?
    /// V1 has exactly one testnet assurance variant: experimental.
    public let assuranceExperimental: Bool
    /// Exact canonical nested value retained from Torii.
    public let canonicalNorito: Data
}

public struct PrivacyExact12CapabilityRowV1: Equatable, Sendable {
    public let protocolId: PrivacyProtocolIdV1
    public let operationSchema: PrivacyOperationSchemaV1
    public let executionMode: PrivacyExecutionModeV1
    public let privacyFeatureMask: UInt8
    public let compiledProfile: PrivacyCompiledProfileResultV1
    public let readiness: PrivacyCapabilityReadinessV1
    public let activationState: PrivacyCapabilityActivationStateV1
    public let activation: PrivacyProtocolActivationRecordV1?
    public let limitation: PrivacyCapabilityLimitationV1?
    /// True only after byte-exact comparison with the ABI23-validated local catalog.
    public let localCompiledTupleMatches: Bool

    public var isNetworkAvailable: Bool {
        switch readiness {
        case .available, .availableExperimental:
            return activationState == .active
        case .unavailable:
            return false
        }
    }

    let compiledProfileCanonicalNorito: Data
}

/// Immutable model issued only from exact Torii Norito plus an ABI23 local catalog.
public final class PrivacyExact12CapabilityManifestV1: @unchecked Sendable {
    public static let versionV1: UInt32 = 1
    public static let maximumArchiveBytes = 256 * 1024

    public let version: UInt32
    public let committedHeight: UInt64
    public let consensusPolicy: PrivacyConsensusPolicyV1
    public let protocols: [PrivacyExact12CapabilityRowV1]
    public let manifestDigest: Data
    private let archive: Data

    fileprivate init(
        version: UInt32,
        committedHeight: UInt64,
        consensusPolicy: PrivacyConsensusPolicyV1,
        protocols: [PrivacyExact12CapabilityRowV1],
        manifestDigest: Data,
        canonicalArchive: Data
    ) {
        self.version = version
        self.committedHeight = committedHeight
        self.consensusPolicy = consensusPolicy
        self.protocols = protocols
        self.manifestDigest = Data(manifestDigest)
        archive = Data(canonicalArchive)
    }

    /// Defensive copy of the exact canonical bytes returned by Torii.
    public func canonicalBytes() -> Data { Data(archive) }

    public func row(for protocolId: PrivacyProtocolIdV1) -> PrivacyExact12CapabilityRowV1 {
        let index = Int(protocolId.noritoDiscriminant)
        precondition(protocols[index].protocolId == protocolId)
        return protocols[index]
    }
}

/// Opaque token which revalidates its source manifest and native tuple at use time.
public final class PrivacyExact12CapabilityTupleAdmissionV1: @unchecked Sendable {
    public let protocolId: PrivacyProtocolIdV1
    public let committedHeight: UInt64
    public let manifestDigest: Data
    public let operationSchema: PrivacyOperationSchemaV1

    private final class Seal: @unchecked Sendable {}
    private static let authenticSeal = Seal()
    private let seal: Seal
    private let manifestArchive: Data

    private init(
        manifest: PrivacyExact12CapabilityManifestV1,
        row: PrivacyExact12CapabilityRowV1
    ) {
        protocolId = row.protocolId
        committedHeight = manifest.committedHeight
        manifestDigest = Data(manifest.manifestDigest)
        operationSchema = row.operationSchema
        manifestArchive = manifest.canonicalBytes()
        seal = Self.authenticSeal
    }

    fileprivate static func issue(
        manifest: PrivacyExact12CapabilityManifestV1,
        row: PrivacyExact12CapabilityRowV1
    ) -> PrivacyExact12CapabilityTupleAdmissionV1 {
        PrivacyExact12CapabilityTupleAdmissionV1(manifest: manifest, row: row)
    }

    fileprivate func requireAuthentic(
        for expectedProtocol: PrivacyProtocolIdV1,
        instructionArchive: Data? = nil
    ) throws {
        guard seal === Self.authenticSeal, protocolId == expectedProtocol else {
            throw PrivacyExact12CapabilityManifestErrorV1.invalidAdmission
        }
        let current = try PrivacyNativeBridge.validateExact12CapabilityManifestV1(
            manifestArchive
        )
        guard current.committedHeight == committedHeight,
              current.manifestDigest == manifestDigest else {
            throw PrivacyExact12CapabilityManifestErrorV1.invalidAdmission
        }
        let row = current.row(for: expectedProtocol)
        guard row.isNetworkAvailable else {
            throw PrivacyExact12CapabilityManifestErrorV1.unavailableProtocol(expectedProtocol)
        }
        guard row.localCompiledTupleMatches else {
            throw PrivacyExact12CapabilityManifestErrorV1.compiledTupleMismatch(expectedProtocol)
        }
        if let instructionArchive {
            try PrivacyExact12CapabilityManifestCodecV1.requireSubmitProofInstruction(
                instructionArchive,
                row: row,
                consensusLimits: current.consensusPolicy.currentLimits
            )
        }
    }
}

/// The sole path from a committed manifest to retained-protocol construction.
public enum PrivacyExact12CapabilityAdmissionV1 {
    public static func requireExact12CapabilityTupleV1(
        _ manifest: PrivacyExact12CapabilityManifestV1,
        protocolId: PrivacyProtocolIdV1
    ) throws -> PrivacyExact12CapabilityTupleAdmissionV1 {
        // Decode again so admission cannot rely on stale managed state or a prior native load.
        let current = try PrivacyNativeBridge.validateExact12CapabilityManifestV1(
            manifest.canonicalBytes()
        )
        guard current.manifestDigest == manifest.manifestDigest,
              current.committedHeight == manifest.committedHeight else {
            throw PrivacyExact12CapabilityManifestErrorV1.invalidAdmission
        }
        let row = current.row(for: protocolId)
        guard row.isNetworkAvailable else {
            throw PrivacyExact12CapabilityManifestErrorV1.unavailableProtocol(protocolId)
        }
        guard row.localCompiledTupleMatches else {
            throw PrivacyExact12CapabilityManifestErrorV1.compiledTupleMismatch(protocolId)
        }
        return PrivacyExact12CapabilityTupleAdmissionV1.issue(
            manifest: current,
            row: row
        )
    }

    public static func requireForConstruction(
        _ admission: PrivacyExact12CapabilityTupleAdmissionV1,
        protocolId: PrivacyProtocolIdV1,
        submitProofInstructionNorito: Data
    ) throws {
        try admission.requireAuthentic(
            for: protocolId,
            instructionArchive: submitProofInstructionNorito
        )
    }
}

extension PrivacyProtocolIdV1 {
    fileprivate var expectedOperationSchemaV1: PrivacyOperationSchemaV1 {
        PrivacyOperationSchemaV1(rawValue: noritoDiscriminant)!
    }

    fileprivate var expectedExecutionModeV1: PrivacyExecutionModeV1 {
        switch self {
        case .zkAcePqAuthorizationV0: return .authorizationAction
        case .anonymousPgcKOutOfNV1, .moneroFcmpPlusPlusV1: return .paymentAction
        case .veRangeTransparentRangeV1, .irohaJindoPolynomialCommitmentV0:
            return .component
        case .irohaZkAmsV1: return .admissionAction
        case .vegaExistingCredentialZkV0, .irohaZkX509StarkP256V0,
             .irohaBootleLanternAnoncredV1:
            return .presentationAction
        case .orchardHalo2ActionsV1, .irohaIvmPrivateNoteStarkV1, .pqMaspStarkV0:
            return .noteAction
        }
    }

    fileprivate var expectedPrivacyFeatureMaskV1: UInt8 {
        switch self {
        case .zkAcePqAuthorizationV0, .irohaJindoPolynomialCommitmentV0: return 0
        case .anonymousPgcKOutOfNV1: return 0b00110
        case .veRangeTransparentRangeV1: return 0b00001
        case .irohaZkAmsV1, .vegaExistingCredentialZkV0,
             .irohaZkX509StarkP256V0, .irohaBootleLanternAnoncredV1,
             .moneroFcmpPlusPlusV1:
            return 0b00010
        case .orchardHalo2ActionsV1, .irohaIvmPrivateNoteStarkV1: return 0b00111
        case .pqMaspStarkV0: return 0b11111
        }
    }
}

/// Allocation-bounded canonical Norito decoder used after the ABI23 prerequisite.
enum PrivacyExact12CapabilityManifestCodecV1 {
    private static let manifestSchema = "iroha.privacy.exact12-capability-manifest.v1"
    private static let catalogSchema = "iroha.privacy.compiled-profile-catalog.v1"
    private static let submitProofSchema =
        "iroha_data_model::isi::privacy::SubmitPrivacyProofV1"
    private static let manifestDigestDomain =
        Data("iroha:privacy:exact12-capability-manifest:v1".utf8)
    private static let maximumFieldBytes = 128 * 1024
    private static let maximumActionBytes = 9 * 1024 * 1024
    private static let noticeBlocks: UInt64 = 300

    static func decode(
        _ archive: Data,
        nativeCatalogArchive: Data
    ) throws -> PrivacyExact12CapabilityManifestV1 {
        let localProfiles = try decodeCatalog(nativeCatalogArchive)
        let frame = try strictFrame(
            archive,
            schema: manifestSchema,
            alignment: 8,
            maximum: PrivacyExact12CapabilityManifestV1.maximumArchiveBytes,
            label: "manifest"
        )
        var reader = WireReader(frame.payload)
        let versionField = try reader.readField(maximum: 4, label: "version")
        let heightField = try reader.readField(maximum: 8, label: "committed height")
        let policyField = try reader.readField(maximum: maximumFieldBytes, label: "consensus policy")
        let protocolsField = try reader.readField(maximum: maximumFieldBytes, label: "protocols")
        let digestField = try reader.readField(maximum: 33, label: "manifest digest")
        try reader.requireFinished("manifest")

        let version = try exactUInt32(versionField, "version")
        guard version == PrivacyExact12CapabilityManifestV1.versionV1 else {
            throw invalid("version must be exactly 1")
        }
        let committedHeight = try exactUInt64(heightField, "committed height")
        let policy = try decodeConsensusPolicy(policyField, committedHeight: committedHeight)
        let digest = try fixed32(digestField, label: "manifest digest", nonzero: true)

        var rowsReader = WireReader(protocolsField)
        guard try rowsReader.readUInt64("protocol count") == 12 else {
            throw invalid("protocols must contain exactly 12 rows")
        }
        var rows: [PrivacyExact12CapabilityRowV1] = []
        rows.reserveCapacity(12)
        for (index, protocolId) in PrivacyProtocolIdV1.allCases.enumerated() {
            let rowBytes = try rowsReader.readField(
                maximum: maximumFieldBytes,
                label: "protocol row \(index)"
            )
            rows.append(
                try decodeRow(
                    rowBytes,
                    expected: protocolId,
                    committedHeight: committedHeight,
                    localCompiledProfile: localProfiles[index]
                )
            )
        }
        try rowsReader.requireFinished("protocols")

        var normalizedPayload = Data()
        for field in [versionField, heightField, policyField, protocolsField] {
            appendField(field, to: &normalizedPayload)
        }
        appendField(array32(Data(repeating: 0, count: 32)), to: &normalizedPayload)
        let normalized = noritoEncode(
            typeName: manifestSchema,
            payload: normalizedPayload,
            flags: NoritoHeader.compactLen,
            payloadAlignment: 8
        )
        var digestPreimage = manifestDigestDomain
        appendUInt64(UInt64(normalized.count), to: &digestPreimage)
        digestPreimage.append(normalized)
        let computed = Data(SHA256.hash(data: digestPreimage))
        guard computed == digest else {
            throw invalid("manifest digest does not bind the canonical archive")
        }

        return PrivacyExact12CapabilityManifestV1(
            version: version,
            committedHeight: committedHeight,
            consensusPolicy: policy,
            protocols: rows,
            manifestDigest: digest,
            canonicalArchive: archive
        )
    }

    private static func decodeCatalog(_ archive: Data) throws -> [Data] {
        let frame = try strictFrame(
            archive,
            schema: catalogSchema,
            alignment: 8,
            maximum: PrivacyNativeBridge.compiledProfileCatalogArchiveMaximumBytes,
            label: "native compiled-profile catalog"
        )
        var reader = WireReader(frame.payload)
        guard try exactUInt32(
            try reader.readField(maximum: 4, label: "catalog version"),
            "catalog version"
        ) == 1 else {
            throw invalid("native compiled-profile catalog version must be 1")
        }
        let rowsField = try reader.readField(maximum: maximumFieldBytes, label: "catalog rows")
        try reader.requireFinished("native compiled-profile catalog")
        var rowsReader = WireReader(rowsField)
        guard try rowsReader.readUInt64("catalog row count") == 12 else {
            throw invalid("native compiled-profile catalog must contain 12 rows")
        }
        var profiles: [Data] = []
        for (index, protocolId) in PrivacyProtocolIdV1.allCases.enumerated() {
            var row = WireReader(
                try rowsReader.readField(maximum: maximumFieldBytes, label: "catalog row \(index)")
            )
            guard try decodeProtocol(
                row.readField(maximum: 4, label: "catalog protocol"),
                label: "catalog protocol"
            ) == protocolId else {
                throw invalid("native compiled-profile catalog order drifted")
            }
            let compiled = try row.readField(maximum: maximumFieldBytes, label: "catalog profile")
            try row.requireFinished("catalog row \(index)")
            _ = try decodeCompiledProfile(compiled, expected: protocolId)
            profiles.append(compiled)
        }
        try rowsReader.requireFinished("native compiled-profile catalog rows")
        return profiles
    }

    private static func decodeRow(
        _ bytes: Data,
        expected protocolId: PrivacyProtocolIdV1,
        committedHeight: UInt64,
        localCompiledProfile: Data
    ) throws -> PrivacyExact12CapabilityRowV1 {
        var row = WireReader(bytes)
        guard try decodeProtocol(row.readField(maximum: 4, label: "protocol id"), label: "protocol id")
            == protocolId else {
            throw invalid("protocol rows are missing, duplicated, or reordered")
        }
        let operation = try unitEnum(
            row.readField(maximum: 4, label: "operation schema"),
            as: PrivacyOperationSchemaV1.self,
            label: "operation schema"
        )
        guard operation == protocolId.expectedOperationSchemaV1 else {
            throw invalid("operation schema differs from the closed protocol mapping")
        }
        let execution = try unitEnum(
            row.readField(maximum: 4, label: "execution mode"),
            as: PrivacyExecutionModeV1.self,
            label: "execution mode"
        )
        guard execution == protocolId.expectedExecutionModeV1 else {
            throw invalid("execution mode differs from the closed protocol mapping")
        }
        let maskField = try row.readField(maximum: 2, label: "privacy feature mask")
        let mask = try byteNewtype(maskField, label: "privacy feature mask")
        guard mask == protocolId.expectedPrivacyFeatureMaskV1 else {
            throw invalid("privacy feature mask differs from the closed protocol mapping")
        }
        let compiledBytes = try row.readField(maximum: maximumFieldBytes, label: "compiled profile")
        let compiled = try decodeCompiledProfile(compiledBytes, expected: protocolId)
        guard compiledBytes == localCompiledProfile else {
            throw PrivacyExact12CapabilityManifestErrorV1.compiledTupleMismatch(protocolId)
        }
        let readiness = try decodeReadiness(
            row.readField(maximum: maximumFieldBytes, label: "readiness")
        )
        let activationState = try unitEnum(
            row.readField(maximum: 4, label: "activation state"),
            as: PrivacyCapabilityActivationStateV1.self,
            label: "activation state"
        )
        let activationBytes = try decodeOption(
            row.readField(maximum: maximumFieldBytes, label: "activation"),
            label: "activation"
        )
        let activation = try activationBytes.map {
            try decodeActivation(
                $0,
                expected: protocolId,
                committedHeight: committedHeight,
                compiled: compiled
            )
        }
        let limitationBytes = try decodeOption(
            row.readField(maximum: 8, label: "limitation"),
            label: "limitation"
        )
        let limitation: PrivacyCapabilityLimitationV1?
        if let limitationBytes {
            limitation = try unitEnum(
                limitationBytes,
                as: PrivacyCapabilityLimitationV1.self,
                label: "limitation"
            )
        } else {
            limitation = nil
        }
        try row.requireFinished("capability row")

        let expectedReadiness: PrivacyCapabilityReadinessV1
        switch compiled {
        case .available where protocolId == .irohaJindoPolynomialCommitmentV0:
            expectedReadiness = .availableExperimental
        case .available:
            expectedReadiness = .available
        case let .unavailable(reason):
            expectedReadiness = .unavailable(reason)
            guard activation == nil else {
                throw invalid("an unavailable compiled profile cannot carry activation")
            }
        }
        guard readiness == expectedReadiness else {
            throw invalid("readiness was not derived from the compiled profile")
        }
        let projectedState = activation.map { projectActivationState(for: $0.lifecycle) }
            ?? .notRegistered
        guard activationState == projectedState else {
            throw invalid("activation state differs from the full committed lifecycle")
        }
        let expectedLimitation: PrivacyCapabilityLimitationV1? =
            protocolId == .irohaJindoPolynomialCommitmentV0
                ? .missingDistributionWideKnowledgeSoundnessEvidence : nil
        guard limitation == expectedLimitation else {
            throw invalid("capability limitation differs from the closed protocol mapping")
        }

        return PrivacyExact12CapabilityRowV1(
            protocolId: protocolId,
            operationSchema: operation,
            executionMode: execution,
            privacyFeatureMask: protocolId.expectedPrivacyFeatureMaskV1,
            compiledProfile: compiled,
            readiness: readiness,
            activationState: activationState,
            activation: activation,
            limitation: limitation,
            localCompiledTupleMatches: true,
            compiledProfileCanonicalNorito: compiledBytes
        )
    }

    private static func decodeCompiledProfile(
        _ bytes: Data,
        expected protocolId: PrivacyProtocolIdV1
    ) throws -> PrivacyCompiledProfileResultV1 {
        let tagged = try taggedPayload(bytes, label: "compiled profile")
        switch tagged.tag {
        case 0:
            guard let payload = tagged.payload else {
                throw invalid("available compiled profile has no tuple")
            }
            return .available(try decodeProfile(payload, expected: protocolId))
        case 1:
            guard let payload = tagged.payload else {
                throw invalid("unavailable compiled profile has no typed reason")
            }
            return .unavailable(try decodeUnavailableReason(payload))
        default:
            throw invalid("compiled profile has an unknown result discriminant")
        }
    }

    private static func decodeProfile(
        _ bytes: Data,
        expected protocolId: PrivacyProtocolIdV1
    ) throws -> PrivacyCompiledProfileSnapshotV1 {
        var profile = WireReader(bytes)
        guard try decodeProtocol(profile.readField(maximum: 4, label: "profile protocol"), label: "profile protocol")
            == protocolId else {
            throw invalid("compiled profile protocol differs from its row")
        }
        guard let proof = PrivacyProofSystemIdV1(rawValue: try exactUInt32(
            profile.readField(maximum: 4, label: "proof system"), "proof system"
        )), proof == protocolId.expectedProofSystem else {
            throw invalid("compiled proof system differs from the closed protocol mapping")
        }
        guard let engine = PrivacyEngineIdV1(rawValue: try exactUInt32(
            profile.readField(maximum: 4, label: "engine"), "engine"
        )), engine == protocolId.expectedEngine else {
            throw invalid("compiled engine differs from the closed protocol mapping")
        }
        let parameterId = try fixed32(
            profile.readField(maximum: 33, label: "parameter id"),
            label: "parameter id", nonzero: true
        )
        let parameterDigest = try fixed32(
            profile.readField(maximum: 33, label: "parameter digest"),
            label: "parameter digest", nonzero: true
        )
        let verifierDigest = try fixed32(
            profile.readField(maximum: 33, label: "verifier digest"),
            label: "verifier digest", nonzero: true
        )
        let statementSchemaDigest = try fixed32(
            profile.readField(maximum: 33, label: "statement schema digest"),
            label: "statement schema digest", nonzero: true
        )
        let engineManifestDigest = try fixed32(
            profile.readField(maximum: 33, label: "engine manifest digest"),
            label: "engine manifest digest", nonzero: true
        )
        let limits = try decodeProtocolLimits(
            profile.readField(maximum: 64, label: "compiled protocol limits"),
            expected: protocolId
        )
        try profile.requireFinished("compiled profile")
        return PrivacyCompiledProfileSnapshotV1(
            protocolId: protocolId,
            proofSystemId: proof,
            engineId: engine,
            parameterId: parameterId,
            parameterDigest: parameterDigest,
            verifierDigest: verifierDigest,
            statementSchemaDigest: statementSchemaDigest,
            engineManifestDigest: engineManifestDigest,
            protocolLimits: limits
        )
    }

    private static func decodeUnavailableReason(
        _ bytes: Data
    ) throws -> PrivacyCompiledProfileUnavailableReasonV1 {
        let tagged = try taggedPayload(bytes, label: "unavailable reason")
        switch tagged.tag {
        case 0 where tagged.payload == nil: return .engineUnavailable
        case 1 where tagged.payload == nil: return .profileInitializationFailed
        case 2:
            guard let payload = tagged.payload,
                  payload.count == 4,
                  let tag = payload.readUInt32LE(at: 0), tag < 2 else {
                throw invalid("statement-schema failure is malformed")
            }
            return .statementSchemaInvalid(conflictingStableTypeId: tag == 0)
        default:
            throw invalid("compiled profile has an unknown unavailable reason")
        }
    }

    private static func decodeReadiness(_ bytes: Data) throws -> PrivacyCapabilityReadinessV1 {
        let tagged = try taggedPayload(bytes, label: "readiness")
        switch tagged.tag {
        case 0 where tagged.payload == nil: return .available
        case 1 where tagged.payload == nil: return .availableExperimental
        case 2:
            guard let payload = tagged.payload else {
                throw invalid("unavailable readiness has no typed reason")
            }
            return .unavailable(try decodeUnavailableReason(payload))
        default:
            throw invalid("readiness has an unknown discriminant")
        }
    }

    private static func decodeConsensusPolicy(
        _ bytes: Data,
        committedHeight: UInt64
    ) throws -> PrivacyConsensusPolicyV1 {
        var policy = WireReader(bytes)
        let current = try decodeConsensusLimits(
            policy.readField(maximum: 64, label: "current consensus limits")
        )
        let pendingBytes = try decodeOption(
            policy.readField(maximum: 128, label: "pending consensus tightening"),
            label: "pending consensus tightening"
        )
        try policy.requireFinished("consensus policy")
        let pending = try pendingBytes.map { value -> PrivacyConsensusPolicyTighteningV1 in
            var tightening = WireReader(value)
            let scheduled = try exactUInt64(
                tightening.readField(maximum: 8, label: "scheduled height"), "scheduled height"
            )
            let effective = try exactUInt64(
                tightening.readField(maximum: 8, label: "effective height"), "effective height"
            )
            let next = try decodeConsensusLimits(
                tightening.readField(maximum: 64, label: "next consensus limits")
            )
            try tightening.requireFinished("pending consensus tightening")
            try validateSchedule(scheduled, effective, committedHeight: committedHeight)
            try requireStrictTightening(current.orderedValues, next.orderedValues, label: "consensus")
            return PrivacyConsensusPolicyTighteningV1(
                scheduledAtHeight: scheduled,
                effectiveAtHeight: effective,
                nextLimits: next
            )
        }
        return PrivacyConsensusPolicyV1(
            currentLimits: current,
            pendingTightening: pending,
            canonicalNorito: bytes
        )
    }

    private static func decodeConsensusLimits(_ bytes: Data) throws -> PrivacyConsensusLimitsV1 {
        var reader = WireReader(bytes)
        var values: [UInt32] = []
        for index in 0..<10 {
            values.append(try exactUInt32(
                reader.readField(maximum: 4, label: "consensus limit \(index)"),
                "consensus limit \(index)"
            ))
        }
        try reader.requireFinished("consensus limits")
        let maxima: [UInt32] = [
            1, 2, 9 * 1024 * 1024, 9 * 1024 * 1024,
            9 * 1024 * 1024, 18 * 1024 * 1024, 256 * 1024, 8, 8, 2048,
        ]
        for index in values.indices where values[index] == 0 || values[index] > maxima[index] {
            throw invalid("consensus limit \(index) is zero or exceeds its V1 hard ceiling")
        }
        guard values[0] <= values[1], values[2] <= values[3],
              values[3] <= values[4], values[4] <= values[5], values[6] <= values[3] else {
            throw invalid("consensus limits violate containing-scope ordering")
        }
        return PrivacyConsensusLimitsV1(
            maxActionsPerTransaction: values[0], maxActionsPerBlock: values[1],
            maxProofBytesPerAction: values[2], maxActionBytes: values[3],
            maxPrivacyBytesPerTransaction: values[4], maxPrivacyBytesPerBlock: values[5],
            maxStatementAndEncryptedOutputBytesPerTransaction: values[6],
            maxNullifiersPerAction: values[7], maxCommitmentsPerAction: values[8],
            retainedRootCount: values[9]
        )
    }

    private static func decodeActivation(
        _ bytes: Data,
        expected protocolId: PrivacyProtocolIdV1,
        committedHeight: UInt64,
        compiled: PrivacyCompiledProfileResultV1
    ) throws -> PrivacyProtocolActivationRecordV1 {
        guard case let .available(profile) = compiled else {
            throw invalid("unavailable compiled profile cannot have activation")
        }
        var record = WireReader(bytes)
        guard try decodeProtocol(record.readField(maximum: 4, label: "activation protocol"), label: "activation protocol")
            == protocolId else {
            throw invalid("activation protocol differs from its row")
        }
        guard let proof = PrivacyProofSystemIdV1(rawValue: try exactUInt32(
            record.readField(maximum: 4, label: "activation proof system"),
            "activation proof system"
        )), proof == profile.proofSystemId else {
            throw invalid("activation proof system differs from the compiled tuple")
        }
        guard let engine = PrivacyEngineIdV1(rawValue: try exactUInt32(
            record.readField(maximum: 4, label: "activation engine"), "activation engine"
        )), engine == profile.engineId else {
            throw invalid("activation engine differs from the compiled tuple")
        }
        let bindingLabels = [
            "parameter id", "parameter digest", "verifier digest",
            "statement schema digest", "engine manifest digest",
        ]
        let expectedBindings = [
            profile.parameterId, profile.parameterDigest, profile.verifierDigest,
            profile.statementSchemaDigest, profile.engineManifestDigest,
        ]
        var bindings: [Data] = []
        for (index, label) in bindingLabels.enumerated() {
            let binding = try fixed32(
                record.readField(maximum: 33, label: "activation \(label)"),
                label: "activation \(label)", nonzero: true
            )
            guard binding == expectedBindings[index] else {
                throw invalid("activation \(label) differs from the compiled tuple")
            }
            bindings.append(binding)
        }
        let lifecycle = try decodeLifecycle(
            record.readField(maximum: 64, label: "lifecycle"),
            committedHeight: committedHeight
        )
        let limits = try decodeProtocolLimits(
            record.readField(maximum: 64, label: "activation protocol limits"),
            expected: protocolId
        )
        try requireAtMost(limits.values, profile.protocolLimits.values, label: "activation limits")
        let pendingBytes = try decodeOption(
            record.readField(maximum: 128, label: "pending protocol limits"),
            label: "pending protocol limits"
        )
        let pending = try pendingBytes.map { value -> PrivacyProtocolLimitsTighteningV1 in
            var tightening = WireReader(value)
            let scheduled = try exactUInt64(
                tightening.readField(maximum: 8, label: "protocol scheduled height"),
                "protocol scheduled height"
            )
            let effective = try exactUInt64(
                tightening.readField(maximum: 8, label: "protocol effective height"),
                "protocol effective height"
            )
            let next = try decodeProtocolLimits(
                tightening.readField(maximum: 64, label: "next protocol limits"),
                expected: protocolId
            )
            try tightening.requireFinished("pending protocol limits")
            try validateSchedule(scheduled, effective, committedHeight: committedHeight)
            try requireStrictTightening(limits.values, next.values, label: "protocol")
            return PrivacyProtocolLimitsTighteningV1(
                scheduledAtHeight: scheduled,
                effectiveAtHeight: effective,
                nextLimits: next
            )
        }
        guard try exactUInt32(
            record.readField(maximum: 4, label: "assurance"), "assurance"
        ) == 0 else {
            throw invalid("activation assurance must be the sole experimental V1 variant")
        }
        try record.requireFinished("activation record")
        return PrivacyProtocolActivationRecordV1(
            protocolId: protocolId, proofSystemId: proof, engineId: engine,
            parameterId: bindings[0], parameterDigest: bindings[1],
            verifierDigest: bindings[2], statementSchemaDigest: bindings[3],
            engineManifestDigest: bindings[4], lifecycle: lifecycle,
            protocolLimits: limits, pendingProtocolLimitsTightening: pending,
            assuranceExperimental: true, canonicalNorito: bytes
        )
    }

    private static func decodeLifecycle(
        _ bytes: Data,
        committedHeight: UInt64
    ) throws -> PrivacyProtocolLifecycleV1 {
        let tagged = try taggedPayload(bytes, label: "lifecycle")
        guard let payload = tagged.payload else { throw invalid("lifecycle record is missing") }
        var state = WireReader(payload)
        let proposed = try exactUInt64(
            state.readField(maximum: 8, label: "proposed height"), "proposed height"
        )
        guard proposed > 0, proposed <= committedHeight else {
            throw invalid("lifecycle proposal height is outside committed state")
        }
        let lifecycle: PrivacyProtocolLifecycleV1
        switch tagged.tag {
        case 0:
            let activate = try exactUInt64(
                state.readField(maximum: 8, label: "activate height"), "activate height"
            )
            guard activate > proposed, activate > committedHeight else {
                throw invalid("proposed lifecycle has a due or unordered activation height")
            }
            lifecycle = .proposed(proposedAtHeight: proposed, activateAtHeight: activate)
        case 1, 2:
            let activated = try exactUInt64(
                state.readField(maximum: 8, label: "activated height"), "activated height"
            )
            let since = try exactUInt64(
                state.readField(maximum: 8, label: "state-since height"), "state-since height"
            )
            let validSince = tagged.tag == 1 ? since >= activated : since > activated
            guard activated > proposed, validSince,
                  activated <= committedHeight, since <= committedHeight else {
                throw invalid("active/suspended lifecycle heights are invalid")
            }
            lifecycle = tagged.tag == 1
                ? .active(proposedAtHeight: proposed, activatedAtHeight: activated,
                          stateSinceHeight: since)
                : .suspended(proposedAtHeight: proposed, activatedAtHeight: activated,
                             stateSinceHeight: since)
        case 3:
            let activated = try decodeOptionUInt64(
                state.readField(maximum: 16, label: "retired activation height"),
                label: "retired activation height"
            )
            let since = try exactUInt64(
                state.readField(maximum: 8, label: "retired state-since height"),
                "retired state-since height"
            )
            if let activated {
                guard activated > proposed, since > activated else {
                    throw invalid("retired lifecycle activation history is invalid")
                }
            } else if since <= proposed {
                throw invalid("retired lifecycle state height must follow proposal")
            }
            guard activated.map({ $0 <= committedHeight }) ?? true,
                  since <= committedHeight else {
                throw invalid("retired lifecycle claims a future committed height")
            }
            lifecycle = .retired(
                proposedAtHeight: proposed,
                activatedAtHeight: activated,
                stateSinceHeight: since
            )
        default:
            throw invalid("lifecycle has an unknown discriminant")
        }
        try state.requireFinished("lifecycle")
        return lifecycle
    }

    private static func decodeProtocolLimits(
        _ bytes: Data,
        expected protocolId: PrivacyProtocolIdV1
    ) throws -> PrivacyProtocolActivationLimitsV1 {
        let tagged = try taggedPayload(bytes, label: "protocol limits")
        guard tagged.tag == protocolId.noritoDiscriminant else {
            throw invalid("protocol limits use a different protocol variant")
        }
        let expectedValueCount: Int
        switch protocolId {
        case .anonymousPgcKOutOfNV1, .irohaZkAmsV1, .moneroFcmpPlusPlusV1,
             .irohaIvmPrivateNoteStarkV1, .pqMaspStarkV0:
            expectedValueCount = 2
        case .veRangeTransparentRangeV1, .irohaJindoPolynomialCommitmentV0,
             .orchardHalo2ActionsV1:
            expectedValueCount = 1
        default:
            expectedValueCount = 0
        }
        if expectedValueCount == 0 {
            guard tagged.payload == nil else { throw invalid("unit protocol limits carry payload") }
            return PrivacyProtocolActivationLimitsV1(
                protocolId: protocolId,
                values: [],
                canonicalNorito: bytes
            )
        }
        guard let payload = tagged.payload else { throw invalid("protocol limits payload is missing") }
        var reader = WireReader(payload)
        var values: [UInt32] = []
        for index in 0..<expectedValueCount {
            values.append(try exactUInt32(
                reader.readField(maximum: 4, label: "protocol limit \(index)"),
                "protocol limit \(index)"
            ))
        }
        try reader.requireFinished("protocol limits")
        let maxima: [UInt32]
        switch protocolId {
        case .anonymousPgcKOutOfNV1: maxima = [64, 8]
        case .veRangeTransparentRangeV1: maxima = [8]
        case .irohaZkAmsV1: maxima = [8, 64]
        case .irohaJindoPolynomialCommitmentV0: maxima = [4]
        case .orchardHalo2ActionsV1: maxima = [2]
        case .moneroFcmpPlusPlusV1: maxima = [2, 4]
        case .irohaIvmPrivateNoteStarkV1, .pqMaspStarkV0: maxima = [2, 2]
        default: maxima = []
        }
        for index in values.indices where values[index] == 0 || values[index] > maxima[index] {
            throw invalid("protocol limit \(index) is zero or exceeds its V1 hard ceiling")
        }
        if protocolId == .anonymousPgcKOutOfNV1, ![16, 32, 64].contains(values[0]) {
            throw invalid("Anonymous PGC anonymity set must be 16, 32, or 64")
        }
        if protocolId == .irohaZkAmsV1, ![16, 32, 64].contains(values[1]) {
            throw invalid("ZK-AMS ring size must be 16, 32, or 64")
        }
        return PrivacyProtocolActivationLimitsV1(
            protocolId: protocolId,
            values: values,
            canonicalNorito: bytes
        )
    }

    static func requireSubmitProofInstruction(
        _ archive: Data,
        row: PrivacyExact12CapabilityRowV1,
        consensusLimits: PrivacyConsensusLimitsV1
    ) throws {
        guard archive.count <= Int(consensusLimits.maxActionBytes) else {
            throw invalid("submit-proof instruction exceeds committed consensus action bytes")
        }
        let frame = try strictFrame(
            archive,
            schema: submitProofSchema,
            alignment: 16,
            maximum: maximumActionBytes,
            label: "submit-proof instruction"
        )
        var instruction = WireReader(frame.payload)
        let envelope = try instruction.readField(
            maximum: maximumActionBytes,
            label: "proof envelope"
        )
        try instruction.requireFinished("submit-proof instruction")
        var envelopeReader = WireReader(envelope)
        var fields: [Data] = []
        fields.reserveCapacity(11)
        for index in 0..<11 {
            fields.append(try envelopeReader.readField(
                maximum: maximumActionBytes,
                label: "proof envelope field \(index)"
            ))
        }
        try envelopeReader.requireFinished("proof envelope")

        guard try decodeProtocol(fields[0], label: "envelope protocol") == row.protocolId else {
            throw invalid("submit-proof instruction protocol differs from its admission")
        }
        guard case let .available(profile) = row.compiledProfile,
              try exactUInt32(fields[1], "envelope proof system") == profile.proofSystemId.rawValue,
              try exactUInt32(fields[2], "envelope engine") == profile.engineId.rawValue,
              try fixed32(fields[3], label: "envelope parameter id", nonzero: true)
                == profile.parameterId,
              try fixed32(fields[4], label: "envelope parameter digest", nonzero: true)
                == profile.parameterDigest,
              try fixed32(fields[5], label: "envelope verifier digest", nonzero: true)
                == profile.verifierDigest,
              try fixed32(fields[6], label: "envelope statement schema digest", nonzero: true)
                == profile.statementSchemaDigest,
              try fixed32(fields[7], label: "envelope engine manifest digest", nonzero: true)
                == profile.engineManifestDigest else {
            throw invalid("submit-proof envelope differs from the admitted compiled profile tuple")
        }
        _ = try fixed32(fields[8], label: "envelope statement digest", nonzero: true)
        for (index, label) in [(9, "statement"), (10, "proof")] {
            let tagged = try taggedPayload(fields[index], label: "envelope \(label)")
            guard tagged.tag == row.protocolId.noritoDiscriminant,
                  let payload = tagged.payload, !payload.isEmpty else {
                throw invalid("submit-proof envelope \(label) differs from its admitted protocol")
            }
        }
    }

    private static func projectActivationState(
        for lifecycle: PrivacyProtocolLifecycleV1
    ) -> PrivacyCapabilityActivationStateV1 {
        switch lifecycle {
        case .proposed: return .proposed
        case .active: return .active
        case .suspended: return .suspended
        case .retired: return .retired
        }
    }

    private static func validateSchedule(
        _ scheduled: UInt64,
        _ effective: UInt64,
        committedHeight: UInt64
    ) throws {
        let (earliest, overflow) = scheduled.addingReportingOverflow(noticeBlocks)
        guard scheduled > 0, !overflow, effective >= earliest,
              scheduled <= committedHeight, effective > committedHeight else {
            throw invalid("pending tightening violates notice or committed-height bounds")
        }
    }

    private static func requireAtMost(
        _ values: [UInt32],
        _ ceilings: [UInt32],
        label: String
    ) throws {
        guard values.count == ceilings.count,
              zip(values, ceilings).allSatisfy({ $0.0 <= $0.1 }) else {
            throw invalid("\(label) exceeds its compiled ceiling")
        }
    }

    private static func requireStrictTightening(
        _ current: [UInt32],
        _ next: [UInt32],
        label: String
    ) throws {
        try requireAtMost(next, current, label: "\(label) tightening")
        guard next != current else { throw invalid("\(label) tightening is a no-op") }
    }

    private static func strictFrame(
        _ archive: Data,
        schema: String,
        alignment: Int,
        maximum: Int,
        label: String
    ) throws -> NoritoFrame {
        guard !archive.isEmpty, archive.count <= maximum,
              let frame = noritoDecodeFrame(archive),
              frame.header.compression == .none,
              frame.header.flags == NoritoHeader.compactLen,
              frame.header.schema == noritoSchemaHash(forTypeName: schema),
              frame.paddingLength == noritoHeaderPaddingLength(payloadAlignment: alignment),
              archive.prefix(NoritoHeader.encodedLength) == frame.header.encode(),
              noritoEncode(
                  typeName: schema,
                  payload: frame.payload,
                  flags: NoritoHeader.compactLen,
                  payloadAlignment: alignment
              ) == archive else {
            throw invalid("\(label) is not the exact canonical uncompressed Norito type")
        }
        return frame
    }

    private static func decodeProtocol(_ bytes: Data, label: String) throws -> PrivacyProtocolIdV1 {
        let tag = try exactUInt32(bytes, label)
        do { return try PrivacyProtocolIdV1(noritoDiscriminant: tag) }
        catch { throw invalid("\(label) has an unknown discriminant") }
    }

    private static func unitEnum<T: RawRepresentable>(
        _ bytes: Data,
        as type: T.Type,
        label: String
    ) throws -> T where T.RawValue == UInt32 {
        guard let value = T(rawValue: try exactUInt32(bytes, label)) else {
            throw invalid("\(label) has an unknown discriminant")
        }
        return value
    }

    private static func taggedPayload(
        _ bytes: Data,
        label: String
    ) throws -> (tag: UInt32, payload: Data?) {
        guard bytes.count >= 4, let tag = bytes.readUInt32LE(at: 0) else {
            throw invalid("\(label) is truncated")
        }
        if bytes.count == 4 { return (tag, nil) }
        var reader = WireReader(Data(bytes.dropFirst(4)))
        let payload = try reader.readField(maximum: maximumFieldBytes, label: "\(label) payload")
        try reader.requireFinished(label)
        return (tag, payload)
    }

    private static func decodeOption(_ bytes: Data, label: String) throws -> Data? {
        var reader = WireReader(bytes)
        switch try reader.readUInt8("\(label) tag") {
        case 0:
            try reader.requireFinished(label)
            return nil
        case 1:
            let value = try reader.readField(maximum: maximumFieldBytes, label: "\(label) value")
            try reader.requireFinished(label)
            return value
        default:
            throw invalid("\(label) has an invalid option tag")
        }
    }

    private static func decodeOptionUInt64(_ bytes: Data, label: String) throws -> UInt64? {
        try decodeOption(bytes, label: label).map { try exactUInt64($0, label) }
    }

    private static func fixed32(
        _ bytes: Data,
        label: String,
        nonzero: Bool
    ) throws -> Data {
        var reader = WireReader(bytes)
        guard try reader.readCompactLength("\(label) width") == 32 else {
            throw invalid("\(label) must declare exactly 32 bytes")
        }
        let value = try reader.readBytes(32, label: label)
        try reader.requireFinished(label)
        guard !nonzero || value.contains(where: { $0 != 0 }) else {
            throw invalid("\(label) must be nonzero")
        }
        return value
    }

    private static func byteNewtype(_ bytes: Data, label: String) throws -> UInt8 {
        var reader = WireReader(bytes)
        let value = try reader.readField(maximum: 1, label: "\(label) value")
        try reader.requireFinished(label)
        guard value.count == 1 else {
            throw invalid("\(label) must contain exactly one byte")
        }
        return value[value.startIndex]
    }

    private static func array32(_ bytes: Data) -> Data {
        precondition(bytes.count == 32)
        var value = Data()
        appendCompactLength(32, to: &value)
        value.append(bytes)
        return value
    }

    private static func exactUInt32(_ bytes: Data, _ label: String) throws -> UInt32 {
        guard bytes.count == 4, let value = bytes.readUInt32LE(at: 0) else {
            throw invalid("\(label) must occupy exactly four bytes")
        }
        return value
    }

    private static func exactUInt64(_ bytes: Data, _ label: String) throws -> UInt64 {
        guard bytes.count == 8, let value = bytes.readUInt64LE(at: 0) else {
            throw invalid("\(label) must occupy exactly eight bytes")
        }
        return value
    }

    private static func appendUInt64(_ value: UInt64, to output: inout Data) {
        var little = value.littleEndian
        output.append(Data(bytes: &little, count: 8))
    }

    private static func appendCompactLength(_ value: UInt64, to output: inout Data) {
        var remaining = value
        while remaining >= 0x80 {
            output.append(UInt8(remaining & 0x7f) | 0x80)
            remaining >>= 7
        }
        output.append(UInt8(remaining))
    }

    private static func appendField(_ field: Data, to output: inout Data) {
        appendCompactLength(UInt64(field.count), to: &output)
        output.append(field)
    }

    private static func invalid(_ reason: String) -> PrivacyExact12CapabilityManifestErrorV1 {
        .invalidArchive(reason)
    }

    private struct WireReader {
        private let data: Data
        private var offset = 0

        init(_ data: Data) { self.data = data }

        mutating func readUInt8(_ label: String) throws -> UInt8 {
            guard offset < data.count else { throw invalid("\(label) is truncated") }
            defer { offset += 1 }
            return data[data.startIndex + offset]
        }

        mutating func readUInt64(_ label: String) throws -> UInt64 {
            let bytes = try readBytes(8, label: label)
            guard let value = bytes.readUInt64LE(at: 0) else { throw invalid("\(label) is truncated") }
            return value
        }

        mutating func readCompactLength(_ label: String) throws -> UInt64 {
            var value: UInt64 = 0
            var shift: UInt64 = 0
            for index in 0..<10 {
                let byte = try readUInt8(label)
                let payload = UInt64(byte & 0x7f)
                if shift == 63, payload > 1 { throw invalid("\(label) overflows u64") }
                value |= payload << shift
                if byte & 0x80 == 0 {
                    if index > 0, payload == 0 { throw invalid("\(label) is noncanonical") }
                    return value
                }
                shift += 7
            }
            throw invalid("\(label) is overlong")
        }

        mutating func readField(maximum: Int, label: String) throws -> Data {
            let length = try readCompactLength("\(label) length")
            guard length <= UInt64(maximum), length <= UInt64(Int.max) else {
                throw invalid("\(label) exceeds its byte ceiling")
            }
            return try readBytes(Int(length), label: label)
        }

        mutating func readBytes(_ count: Int, label: String) throws -> Data {
            guard count >= 0, offset <= data.count, count <= data.count - offset else {
                throw invalid("\(label) is truncated")
            }
            let start = data.index(data.startIndex, offsetBy: offset)
            let end = data.index(start, offsetBy: count)
            offset += count
            return Data(data[start..<end])
        }

        func requireFinished(_ label: String) throws {
            guard offset == data.count else { throw invalid("\(label) contains a suffix") }
        }
    }
}

private extension Data {
    func readUInt32LE(at offset: Int) -> UInt32? {
        guard offset >= 0, count - offset >= 4 else { return nil }
        var value: UInt32 = 0
        self[index(startIndex, offsetBy: offset)..<index(startIndex, offsetBy: offset + 4)]
            .withUnsafeBytes { buffer in
                if let base = buffer.baseAddress { memcpy(&value, base, 4) }
            }
        return UInt32(littleEndian: value)
    }

    func readUInt64LE(at offset: Int) -> UInt64? {
        guard offset >= 0, count - offset >= 8 else { return nil }
        var value: UInt64 = 0
        self[index(startIndex, offsetBy: offset)..<index(startIndex, offsetBy: offset + 8)]
            .withUnsafeBytes { buffer in
                if let base = buffer.baseAddress { memcpy(&value, base, 8) }
            }
        return UInt64(littleEndian: value)
    }
}
