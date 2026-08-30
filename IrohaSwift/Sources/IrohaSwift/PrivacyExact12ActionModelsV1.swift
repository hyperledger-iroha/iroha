import Foundation

private struct PrivacyExact12ActionAnyCodingKey: CodingKey {
    let stringValue: String
    let intValue: Int? = nil

    init?(stringValue: String) { self.stringValue = stringValue }
    init?(intValue: Int) { return nil }
}

/// Public action spelling for the closed 13-operation Exact12 schema.
public typealias PrivacyExact12ActionOperationV1 = PrivacyOperationSchemaV1

/// Closed ledger-effect class committed by a public Exact12 operation.
public enum PrivacyLedgerEffectKindV1: String, CaseIterable, Sendable {
    case verificationOnly = "verification_only"
    case zkAceTransparentTransfer = "zk_ace_transparent_transfer"
    case anonymousPgcAccountStateTransition =
        "anonymous_pgc_account_state_transition"
    case zkAmsBatchAdmission = "zk_ams_batch_admission"
    case zkAmsProvisionAccount = "zk_ams_provision_account"
    case zkX509CertificateNullifier = "zk_x509_certificate_nullifier"
    case orchardNoteStateTransition = "orchard_note_state_transition"
    case fcmpMembershipPayment = "fcmp_membership_payment"
    case ivmPrivateNoteStateTransition = "ivm_private_note_state_transition"
    case pqMaspNoteStateTransition = "pq_masp_note_state_transition"

    public var canonicalLabel: String { rawValue }
}

public extension PrivacyOperationSchemaV1 {
    /// Sole retained protocol that executes this public operation.
    var protocolId: PrivacyProtocolIdV1 {
        switch self {
        case .zkAceAuthorizationActionV1: return .zkAcePqAuthorizationV0
        case .anonymousPgcPaymentActionV1: return .anonymousPgcKOutOfNV1
        case .veRangeRangeProofV1: return .veRangeTransparentRangeV1
        case .zkAmsBatchAdmissionActionV1,
             .zkAmsProvisionAccountActionV1:
            return .irohaZkAmsV1
        case .vegaCredentialPresentationV1: return .vegaExistingCredentialZkV0
        case .zkX509IdentityPresentationV1: return .irohaZkX509StarkP256V0
        case .jindoPolynomialEvaluationV1:
            return .irohaJindoPolynomialCommitmentV0
        case .bootleLanternCredentialPresentationV1:
            return .irohaBootleLanternAnoncredV1
        case .orchardNoteActionV1: return .orchardHalo2ActionsV1
        case .fcmpMembershipPaymentV1: return .moneroFcmpPlusPlusV1
        case .ivmPrivateNoteActionV1: return .irohaIvmPrivateNoteStarkV1
        case .pqMaspNoteActionV1: return .pqMaspStarkV0
        }
    }

    /// Typed ledger effect committed when this public operation succeeds.
    var ledgerEffectKind: PrivacyLedgerEffectKindV1 {
        switch self {
        case .zkAceAuthorizationActionV1:
            return .zkAceTransparentTransfer
        case .anonymousPgcPaymentActionV1:
            return .anonymousPgcAccountStateTransition
        case .veRangeRangeProofV1,
             .vegaCredentialPresentationV1,
             .jindoPolynomialEvaluationV1,
             .bootleLanternCredentialPresentationV1:
            return .verificationOnly
        case .zkAmsBatchAdmissionActionV1:
            return .zkAmsBatchAdmission
        case .zkAmsProvisionAccountActionV1:
            return .zkAmsProvisionAccount
        case .zkX509IdentityPresentationV1:
            return .zkX509CertificateNullifier
        case .orchardNoteActionV1:
            return .orchardNoteStateTransition
        case .fcmpMembershipPaymentV1:
            return .fcmpMembershipPayment
        case .ivmPrivateNoteActionV1:
            return .ivmPrivateNoteStateTransition
        case .pqMaspNoteActionV1:
            return .pqMaspNoteStateTransition
        }
    }
}

/// Local lifecycle projection for one Exact12 action submission.
public enum PrivacyActionLocalStateV1: String, CaseIterable, Sendable {
    case submitted
    case terminal

    public var canonicalLabel: String { rawValue }
}

/// Authenticated terminal pipeline state for one Exact12 action submission.
public enum PrivacyActionTerminalChainStateV1: String, CaseIterable, Sendable {
    case committed = "Committed"
    case applied = "Applied"
    case rejected = "Rejected"
    case expired = "Expired"

    public var canonicalLabel: String { rawValue }
}

/// Fail-closed model validation for public Exact12 action requests and views.
public enum PrivacyExact12ActionModelErrorV1: Error, LocalizedError, Equatable, Sendable {
    case invalidSignedTransactionLength(actual: Int, maximum: Int)
    case invalidExpectedManifestDigest
    case operationProtocolMismatch
    case operationLedgerEffectMismatch
    case invalidHash(field: String)
    case invalidCapabilityManifestDigest
    case invalidCapabilityCommittedHeight
    case invalidCommittedHeight
    case invalidExecutionCapabilityCommittedHeight
    case invalidExecutionReceiptFinalizedHeight
    case invalidExecutionReceiptHeights
    case terminalHeightBeforeCapabilitySnapshot
    case invalidRejectionReason
    case invalidStateCombination

    public var errorDescription: String? {
        switch self {
        case let .invalidSignedTransactionLength(actual, maximum):
            return "Exact12 signed transaction must contain 1...\(maximum) bytes (found \(actual))."
        case .invalidExpectedManifestDigest:
            return "Exact12 expected manifest digest must contain 32 non-zero bytes."
        case .operationProtocolMismatch:
            return "Exact12 operation does not belong to the supplied protocol."
        case .operationLedgerEffectMismatch:
            return "Exact12 operation does not produce the supplied ledger-effect kind."
        case let .invalidHash(field):
            return "Exact12 \(field) must contain 32 non-zero bytes."
        case .invalidCapabilityManifestDigest:
            return "Exact12 capability manifest digest must contain 32 non-zero bytes."
        case .invalidCapabilityCommittedHeight:
            return "Exact12 capability committed height must be non-zero."
        case .invalidCommittedHeight:
            return "Exact12 committed height must be non-zero when present."
        case .invalidExecutionCapabilityCommittedHeight:
            return "Exact12 execution capability committed height must be non-zero when present."
        case .invalidExecutionReceiptFinalizedHeight:
            return "Exact12 execution receipt finalized height must be non-zero when present."
        case .invalidExecutionReceiptHeights:
            return "Exact12 execution receipt heights are inconsistent."
        case .terminalHeightBeforeCapabilitySnapshot:
            return "Exact12 terminal height precedes its finalized pre-submit capability snapshot."
        case .invalidRejectionReason:
            return "A rejected Exact12 action must carry one canonical non-empty reason."
        case .invalidStateCombination:
            return "Exact12 local and terminal state fields form an impossible combination."
        }
    }
}

/// One closed Exact12 operation and its already-signed versioned transaction wire.
///
/// This model snapshots and bounds public wire bytes. It performs no local proof
/// acceptance and grants no capability or submission authority.
public struct PrivacyExact12ActionRequestV1: Equatable, Sendable {
    /// Taira V1 `max_tx_bytes`, shared with native Exact12 action inspection.
    public static let maximumSignedTransactionBytes = 10 * 1024 * 1024

    public let operation: PrivacyExact12ActionOperationV1
    public let signedTransactionVersioned: Data
    public let expectedManifestDigest: Data?

    public init(
        operation: PrivacyExact12ActionOperationV1,
        signedTransactionVersioned: Data,
        expectedManifestDigest: Data? = nil
    ) throws {
        guard
            (1...Self.maximumSignedTransactionBytes)
                .contains(signedTransactionVersioned.count)
        else {
            throw PrivacyExact12ActionModelErrorV1.invalidSignedTransactionLength(
                actual: signedTransactionVersioned.count,
                maximum: Self.maximumSignedTransactionBytes
            )
        }
        if let expectedManifestDigest,
           !Self.isNonzeroFixed32(expectedManifestDigest)
        {
            throw PrivacyExact12ActionModelErrorV1.invalidExpectedManifestDigest
        }
        self.operation = operation
        self.signedTransactionVersioned = Data(signedTransactionVersioned)
        self.expectedManifestDigest = expectedManifestDigest.map { Data($0) }
    }

    fileprivate static func isNonzeroFixed32(_ value: Data) -> Bool {
        value.count == 32 && value.contains(where: { $0 != 0 })
    }
}

/// Native-authenticated public digest projection for one exact signed action wire.
public struct PrivacyExact12ActionInspectionV1: Equatable, Sendable {
    public let transactionHash: Data
    public let transactionIntentDigest: Data
    public let statementDigest: Data
    public let proofEnvelopeHash: Data

    init(nativeProjection: Data) throws {
        guard nativeProjection.count == 128 else {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
        }
        let fields = stride(from: 0, to: 128, by: 32).map {
            Data(nativeProjection[$0..<$0 + 32])
        }
        guard fields.allSatisfy({ PrivacyExact12ActionRequestV1.isNonzeroFixed32($0) }) else {
            throw PrivacyCompiledProfileCatalogBridgeError.invalidArchive
        }
        transactionHash = fields[0]
        transactionIntentDigest = fields[1]
        statementDigest = fields[2]
        proofEnvelopeHash = fields[3]
    }
}

struct PrivacyAuthenticatedTransactionDetailsPreparationV1: Sendable {
    let archive: Data
    let signingDigest: Data
}

struct PrivacyAuthenticatedActionReceiptPreparationV1: Sendable {
    let archive: Data
    let signingDigest: Data
}

/// Native-verified success or rejection from an authenticated committed-state query.
///
/// This authenticates Torii's committed-state answer; it is not a signed block or QC and does
/// not independently prove finality. Independent finality requires exact-block verification.
public struct AuthenticatedCommittedTransactionResultV1: Decodable, Equatable, Sendable {
    public let transactionHashHex: String
    public let transactionAuthority: String
    public let blockHashHex: String
    public let resultHashHex: String
    public let resultOk: Bool
    public let rejectionMessage: String?
    public let committedBlockHeight: UInt64

    private enum CodingKeys: String, CodingKey {
        case version
        case transactionHashHex = "transaction_hash_hex"
        case transactionAuthority = "transaction_authority"
        case blockHashHex = "block_hash_hex"
        case resultHashHex = "result_hash_hex"
        case resultOk = "result_ok"
        case rejectionMessage = "rejection_message"
        case committedBlockHeight = "committed_block_height"
    }

    public init(from decoder: Decoder) throws {
        let expectedFields: Set<String> = [
            "version", "transaction_hash_hex", "transaction_authority",
            "block_hash_hex", "result_hash_hex", "result_ok",
            "rejection_message", "committed_block_height",
        ]
        let actualFields = Set(
            try decoder.container(keyedBy: PrivacyExact12ActionAnyCodingKey.self)
                .allKeys.map(\.stringValue)
        )
        guard actualFields == expectedFields else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: decoder.codingPath,
                    debugDescription: "authenticated committed transaction result fields are not exact"
                )
            )
        }
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let version = try container.decode(UInt32.self, forKey: .version)
        guard version == 1 else {
            throw DecodingError.dataCorruptedError(
                forKey: .version,
                in: container,
                debugDescription: "authenticated committed transaction result version must be 1"
            )
        }
        let transactionHashHex = try container.decode(String.self, forKey: .transactionHashHex)
        let transactionAuthority = try container.decode(String.self, forKey: .transactionAuthority)
        let blockHashHex = try container.decode(String.self, forKey: .blockHashHex)
        let resultHashHex = try container.decode(String.self, forKey: .resultHashHex)
        let resultOk = try container.decode(Bool.self, forKey: .resultOk)
        guard container.contains(.rejectionMessage) else {
            throw DecodingError.keyNotFound(
                CodingKeys.rejectionMessage,
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "rejection_message is required even when null"
                )
            )
        }
        let rejectionMessage = try container.decodeIfPresent(
            String.self,
            forKey: .rejectionMessage
        )
        let heightText = try container.decode(String.self, forKey: .committedBlockHeight)
        guard Self.isExactHash(transactionHashHex),
              Self.isExactHash(blockHashHex),
              Self.isExactHash(resultHashHex),
              Self.isCanonicalText(transactionAuthority, maximumUTF8Bytes: 16 * 1024),
              Self.isCanonicalUnsignedDecimal(heightText),
              let committedBlockHeight = UInt64(heightText),
              committedBlockHeight > 0 else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "authenticated committed transaction result is not canonical"
                )
            )
        }
        if resultOk {
            guard rejectionMessage == nil else {
                throw DecodingError.dataCorruptedError(
                    forKey: .rejectionMessage,
                    in: container,
                    debugDescription: "committed success cannot carry a rejection message"
                )
            }
        } else {
            guard Self.isCanonicalText(rejectionMessage, maximumUTF8Bytes: 1_024) else {
                throw DecodingError.dataCorruptedError(
                    forKey: .rejectionMessage,
                    in: container,
                    debugDescription: "committed rejection requires a canonical message"
                )
            }
        }
        self.transactionHashHex = transactionHashHex
        self.transactionAuthority = transactionAuthority
        self.blockHashHex = blockHashHex
        self.resultHashHex = resultHashHex
        self.resultOk = resultOk
        self.rejectionMessage = rejectionMessage
        self.committedBlockHeight = committedBlockHeight
    }

    private static func isExactHash(_ value: String) -> Bool {
        value.utf8.count == 64 && value.utf8.allSatisfy {
            (0x30...0x39).contains($0) || (0x61...0x66).contains($0)
        }
    }

    private static func isCanonicalText(
        _ value: String?,
        maximumUTF8Bytes: Int
    ) -> Bool {
        guard let value, !value.isEmpty,
              value.utf8.count <= maximumUTF8Bytes,
              value == value.trimmingCharacters(in: .whitespacesAndNewlines) else {
            return false
        }
        return !value.unicodeScalars.contains {
            CharacterSet.controlCharacters.contains($0)
        }
    }

    private static func isCanonicalUnsignedDecimal(_ value: String) -> Bool {
        !value.isEmpty
            && value.utf8.allSatisfy { (0x30...0x39).contains($0) }
            && (value == "0" || !value.hasPrefix("0"))
    }
}

/// Terminal state of exactly one native-authenticated offline-device registration.
public enum OfflineDeviceRegistrationTerminalStateV1: String, CaseIterable, Sendable {
    case applied
    case eligibilityRejected = "eligibility_rejected"
    case otherRejected = "other_rejected"
}

/// Native-verified committed result for exactly one offline-device registration instruction.
///
/// The native projection authenticates the transaction-details response and requires exactly one
/// `RegisterOfflineDeviceAttestation` instruction. Swift enforces the closed ABI-22 JSON field set
/// and keeps applied, typed eligibility rejection, and unrelated rejection mutually exclusive.
public struct AuthenticatedOfflineDeviceRegistrationResultV1: Decodable, Equatable, Sendable {
    public let transactionHashHex: String
    public let transactionAuthority: String
    public let blockHashHex: String
    public let resultHashHex: String
    public let committedBlockHeight: UInt64
    public let terminalState: OfflineDeviceRegistrationTerminalStateV1
    public let eligibilityOutcome: OfflineDeviceEligibilityOutcomeV1?
    public let eligibilityReason: OfflineDeviceEligibilityReasonV1?
    public let matchedRuleIds: [String]
    public let rejectionCode: String?
    public let rejectionMessage: String?

    private enum CodingKeys: String, CodingKey {
        case version
        case transactionHashHex = "transaction_hash_hex"
        case transactionAuthority = "transaction_authority"
        case blockHashHex = "block_hash_hex"
        case resultHashHex = "result_hash_hex"
        case committedBlockHeight = "committed_block_height"
        case terminalState = "terminal_state"
        case eligibilityOutcome = "eligibility_outcome"
        case eligibilityReason = "eligibility_reason"
        case matchedRuleIds = "matched_rule_ids"
        case rejectionCode = "rejection_code"
        case rejectionMessage = "rejection_message"
    }

    private static let exactFields: Set<String> = [
        "version", "transaction_hash_hex", "transaction_authority",
        "block_hash_hex", "result_hash_hex", "committed_block_height",
        "terminal_state", "eligibility_outcome", "eligibility_reason",
        "matched_rule_ids", "rejection_code", "rejection_message",
    ]

    private static let otherRejectionCodes: Set<String> = [
        "account_does_not_exist", "limit_check", "validation",
        "instruction_execution", "ivm_execution", "trigger_execution",
    ]

    public init(from decoder: Decoder) throws {
        let actualFields = Set(
            try decoder.container(keyedBy: PrivacyExact12ActionAnyCodingKey.self)
                .allKeys.map(\.stringValue)
        )
        guard actualFields == Self.exactFields else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: decoder.codingPath,
                    debugDescription: "authenticated registration result fields are not exact"
                )
            )
        }
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard try container.decode(UInt32.self, forKey: .version) == 1 else {
            throw DecodingError.dataCorruptedError(
                forKey: .version,
                in: container,
                debugDescription: "authenticated registration result version must be 1"
            )
        }
        let transactionHashHex = try container.decode(String.self, forKey: .transactionHashHex)
        let transactionAuthority = try container.decode(String.self, forKey: .transactionAuthority)
        let blockHashHex = try container.decode(String.self, forKey: .blockHashHex)
        let resultHashHex = try container.decode(String.self, forKey: .resultHashHex)
        let heightText = try container.decode(String.self, forKey: .committedBlockHeight)
        let terminalText = try container.decode(String.self, forKey: .terminalState)
        let outcomeText = try container.decodeIfPresent(String.self, forKey: .eligibilityOutcome)
        let reasonText = try container.decodeIfPresent(String.self, forKey: .eligibilityReason)
        let matchedRuleIds = try container.decode([String].self, forKey: .matchedRuleIds)
        let rejectionCode = try container.decodeIfPresent(String.self, forKey: .rejectionCode)
        let rejectionMessage = try container.decodeIfPresent(String.self, forKey: .rejectionMessage)

        guard Self.isExactHash(transactionHashHex),
              Self.isCanonicalText(transactionAuthority, maximumUTF8Bytes: 16 * 1024),
              Self.isExactHash(blockHashHex),
              Self.isExactHash(resultHashHex),
              Self.isPositiveCanonicalUnsignedDecimal(heightText),
              let committedBlockHeight = UInt64(heightText),
              committedBlockHeight > 0,
              let terminalState = OfflineDeviceRegistrationTerminalStateV1(rawValue: terminalText),
              matchedRuleIds.count <= 256,
              matchedRuleIds.allSatisfy(Self.isCanonicalRuleId),
              zip(matchedRuleIds, matchedRuleIds.dropFirst()).allSatisfy({ $0.0 < $0.1 }),
              rejectionCode.map({ Self.isCanonicalText($0, maximumUTF8Bytes: 128) }) ?? true,
              rejectionMessage.map({ Self.isCanonicalText($0, maximumUTF8Bytes: 1_024) }) ?? true else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "authenticated registration result is not canonical"
                )
            )
        }

        let eligibilityOutcome: OfflineDeviceEligibilityOutcomeV1?
        switch outcomeText {
        case nil: eligibilityOutcome = nil
        case "drain_only": eligibilityOutcome = .drainOnly
        case "cryptographically_rejected": eligibilityOutcome = .cryptographicallyRejected
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .eligibilityOutcome,
                in: container,
                debugDescription: "authenticated registration result has an unknown outcome"
            )
        }
        let eligibilityReason: OfflineDeviceEligibilityReasonV1?
        switch reasonText {
        case nil: eligibilityReason = nil
        case "cryptographic_attestation_rejected":
            eligibilityReason = .cryptographicAttestationRejected
        case "policy_not_fresh": eligibilityReason = .policyNotFresh
        case "incomplete_attested_properties": eligibilityReason = .incompleteAttestedProperties
        case "unsupported_pre_android_12_tee": eligibilityReason = .unsupportedPreAndroid12Tee
        case "vulnerable_firmware": eligibilityReason = .vulnerableFirmware
        case "permanently_blocked_device": eligibilityReason = .permanentlyBlockedDevice
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .eligibilityReason,
                in: container,
                debugDescription: "authenticated registration result has an unknown reason"
            )
        }
        guard Self.isValidTerminalShape(
            terminalState: terminalState,
            eligibilityOutcome: eligibilityOutcome,
            eligibilityReason: eligibilityReason,
            matchedRuleIds: matchedRuleIds,
            rejectionCode: rejectionCode,
            rejectionMessage: rejectionMessage
        ) else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "authenticated registration terminal shape is contradictory"
                )
            )
        }

        self.transactionHashHex = transactionHashHex
        self.transactionAuthority = transactionAuthority
        self.blockHashHex = blockHashHex
        self.resultHashHex = resultHashHex
        self.committedBlockHeight = committedBlockHeight
        self.terminalState = terminalState
        self.eligibilityOutcome = eligibilityOutcome
        self.eligibilityReason = eligibilityReason
        self.matchedRuleIds = matchedRuleIds
        self.rejectionCode = rejectionCode
        self.rejectionMessage = rejectionMessage
    }

    private static func isValidTerminalShape(
        terminalState: OfflineDeviceRegistrationTerminalStateV1,
        eligibilityOutcome: OfflineDeviceEligibilityOutcomeV1?,
        eligibilityReason: OfflineDeviceEligibilityReasonV1?,
        matchedRuleIds: [String],
        rejectionCode: String?,
        rejectionMessage: String?
    ) -> Bool {
        switch terminalState {
        case .applied:
            return eligibilityOutcome == nil
                && eligibilityReason == nil
                && matchedRuleIds.isEmpty
                && rejectionCode == nil
                && rejectionMessage == nil
        case .otherRejected:
            return eligibilityOutcome == nil
                && eligibilityReason == nil
                && matchedRuleIds.isEmpty
                && rejectionCode.map({ Self.otherRejectionCodes.contains($0) }) == true
                && rejectionMessage != nil
        case .eligibilityRejected:
            guard rejectionCode == "offline_device_eligibility",
                  rejectionMessage != nil else {
                return false
            }
            if eligibilityOutcome == .cryptographicallyRejected {
                return eligibilityReason == .cryptographicAttestationRejected
                    && matchedRuleIds.isEmpty
            }
            guard eligibilityOutcome == .drainOnly else { return false }
            switch eligibilityReason {
            case .some(.policyNotFresh), .some(.incompleteAttestedProperties),
                 .some(.unsupportedPreAndroid12Tee):
                return matchedRuleIds.isEmpty
            case .some(.vulnerableFirmware), .some(.permanentlyBlockedDevice):
                return !matchedRuleIds.isEmpty
            default:
                return false
            }
        }
    }

    private static func isExactHash(_ value: String) -> Bool {
        value.utf8.count == 64 && value.utf8.allSatisfy {
            (0x30...0x39).contains($0) || (0x61...0x66).contains($0)
        }
    }

    private static func isCanonicalText(_ value: String, maximumUTF8Bytes: Int) -> Bool {
        !value.isEmpty
            && value.utf8.count <= maximumUTF8Bytes
            && value == value.trimmingCharacters(in: .whitespacesAndNewlines)
            && !value.unicodeScalars.contains { CharacterSet.controlCharacters.contains($0) }
    }

    private static func isCanonicalRuleId(_ value: String) -> Bool {
        isCanonicalText(value, maximumUTF8Bytes: 128)
            && value.utf8.allSatisfy { (0x20...0x7e).contains($0) }
    }

    private static func isPositiveCanonicalUnsignedDecimal(_ value: String) -> Bool {
        !value.isEmpty
            && value.utf8.first.map({ (0x31...0x39).contains($0) }) == true
            && value.utf8.allSatisfy { (0x30...0x39).contains($0) }
    }
}

/// Native-decoded finalized receipt for one successfully executed Exact12 action.
///
/// The native projector accepts only the canonical typed ID105 query response and binds it to
/// the signed query. Swift repeats every public action binding before terminalizing the view.
struct AuthenticatedPrivacyActionExecutionReceiptV1: Decodable, Equatable, Sendable {
    let networkId: Data
    let protocolId: String
    let operationSchema: String
    let ledgerEffectKind: String
    let transactionHash: Data
    let actionIndex: UInt32
    let transactionIntentDigest: Data
    let statementDigest: Data
    let proofEnvelopeHash: Data
    let capabilityManifestDigest: Data
    let capabilityCommittedHeight: UInt64
    let admittedAtHeight: UInt64
    let finalizedHeight: UInt64
    let finalizedBlockHash: Data

    private enum CodingKeys: String, CodingKey {
        case version
        case networkId = "network_id"
        case protocolId = "protocol_id"
        case operationSchema = "operation_schema"
        case ledgerEffectKind = "ledger_effect_kind"
        case transactionHash = "transaction_hash"
        case actionIndex = "action_index"
        case transactionIntentDigest = "transaction_intent_digest"
        case statementDigest = "statement_digest"
        case proofEnvelopeHash = "proof_envelope_hash"
        case capabilityManifestDigest = "capability_manifest_digest"
        case capabilityCommittedHeight = "capability_committed_height"
        case admittedAtHeight = "admitted_at_height"
        case finalizedHeight = "finalized_height"
        case finalizedBlockHash = "finalized_block_hash"
    }

    init(from decoder: Decoder) throws {
        let expectedFields: Set<String> = [
            "version", "network_id", "protocol_id", "operation_schema",
            "ledger_effect_kind", "transaction_hash", "action_index",
            "transaction_intent_digest", "statement_digest", "proof_envelope_hash",
            "capability_manifest_digest", "capability_committed_height",
            "admitted_at_height", "finalized_height", "finalized_block_hash",
        ]
        let actualFields = Set(
            try decoder.container(keyedBy: PrivacyExact12ActionAnyCodingKey.self)
                .allKeys.map(\.stringValue)
        )
        guard actualFields == expectedFields else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: decoder.codingPath,
                    debugDescription: "authenticated Exact12 execution receipt fields are not exact"
                )
            )
        }
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard try container.decode(UInt16.self, forKey: .version) == 1 else {
            throw DecodingError.dataCorruptedError(
                forKey: .version,
                in: container,
                debugDescription: "authenticated Exact12 execution receipt version must be 1"
            )
        }
        let actionIndex = try container.decode(UInt32.self, forKey: .actionIndex)
        guard actionIndex == 0 else {
            throw DecodingError.dataCorruptedError(
                forKey: .actionIndex,
                in: container,
                debugDescription: "authenticated Exact12 execution receipt action index must be zero"
            )
        }
        let networkId = try Self.decodeNonzeroFixed32(
            container.decode(String.self, forKey: .networkId),
            key: .networkId,
            in: container
        )
        let transactionHash = try Self.decodeNonzeroFixed32(
            container.decode(String.self, forKey: .transactionHash),
            key: .transactionHash,
            in: container
        )
        let transactionIntentDigest = try Self.decodeNonzeroFixed32(
            container.decode(String.self, forKey: .transactionIntentDigest),
            key: .transactionIntentDigest,
            in: container
        )
        let statementDigest = try Self.decodeNonzeroFixed32(
            container.decode(String.self, forKey: .statementDigest),
            key: .statementDigest,
            in: container
        )
        let proofEnvelopeHash = try Self.decodeNonzeroFixed32(
            container.decode(String.self, forKey: .proofEnvelopeHash),
            key: .proofEnvelopeHash,
            in: container
        )
        let capabilityManifestDigest = try Self.decodeNonzeroFixed32(
            container.decode(String.self, forKey: .capabilityManifestDigest),
            key: .capabilityManifestDigest,
            in: container
        )
        let finalizedBlockHash = try Self.decodeNonzeroFixed32(
            container.decode(String.self, forKey: .finalizedBlockHash),
            key: .finalizedBlockHash,
            in: container
        )
        let capabilityCommittedHeight = try Self.decodePositiveHeight(
            container.decode(String.self, forKey: .capabilityCommittedHeight),
            key: .capabilityCommittedHeight,
            in: container
        )
        let admittedAtHeight = try Self.decodePositiveHeight(
            container.decode(String.self, forKey: .admittedAtHeight),
            key: .admittedAtHeight,
            in: container
        )
        let finalizedHeight = try Self.decodePositiveHeight(
            container.decode(String.self, forKey: .finalizedHeight),
            key: .finalizedHeight,
            in: container
        )
        guard admittedAtHeight >= capabilityCommittedHeight,
              finalizedHeight >= admittedAtHeight else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: container.codingPath,
                    debugDescription: "authenticated Exact12 execution receipt heights are inconsistent"
                )
            )
        }
        self.networkId = networkId
        protocolId = try container.decode(String.self, forKey: .protocolId)
        operationSchema = try container.decode(String.self, forKey: .operationSchema)
        ledgerEffectKind = try container.decode(String.self, forKey: .ledgerEffectKind)
        self.transactionHash = transactionHash
        self.actionIndex = actionIndex
        self.transactionIntentDigest = transactionIntentDigest
        self.statementDigest = statementDigest
        self.proofEnvelopeHash = proofEnvelopeHash
        self.capabilityManifestDigest = capabilityManifestDigest
        self.capabilityCommittedHeight = capabilityCommittedHeight
        self.admittedAtHeight = admittedAtHeight
        self.finalizedHeight = finalizedHeight
        self.finalizedBlockHash = finalizedBlockHash
    }

    func isBound(
        to operation: PrivacyActionOperationViewV1,
        networkId expectedNetworkId: NetworkId
    ) -> Bool {
        networkId == expectedNetworkId.bytes
            && protocolId == operation.protocolId.rawValue
            && operationSchema == operation.operationSchema.canonicalLabel
            && ledgerEffectKind == operation.ledgerEffectKind.canonicalLabel
            && transactionHash == operation.transactionHash
            && actionIndex == 0
            && transactionIntentDigest == operation.transactionIntentDigest
            && statementDigest == operation.statementDigest
            && proofEnvelopeHash == operation.proofEnvelopeHash
    }

    private static func decodeNonzeroFixed32(
        _ value: String,
        key: CodingKeys,
        in container: KeyedDecodingContainer<CodingKeys>
    ) throws -> Data {
        guard value.utf8.count == 64,
              value.utf8.allSatisfy({
                  (0x30...0x39).contains($0) || (0x61...0x66).contains($0)
              }),
              let decoded = Data(hexString: value),
              PrivacyExact12ActionRequestV1.isNonzeroFixed32(decoded) else {
            throw DecodingError.dataCorruptedError(
                forKey: key,
                in: container,
                debugDescription: "authenticated Exact12 execution receipt hash is not canonical"
            )
        }
        return decoded
    }

    private static func decodePositiveHeight(
        _ value: String,
        key: CodingKeys,
        in container: KeyedDecodingContainer<CodingKeys>
    ) throws -> UInt64 {
        guard !value.isEmpty,
              value.utf8.allSatisfy({ (0x30...0x39).contains($0) }),
              !value.hasPrefix("0"),
              let height = UInt64(value),
              height > 0 else {
            throw DecodingError.dataCorruptedError(
                forKey: key,
                in: container,
                debugDescription: "authenticated Exact12 execution receipt height is not canonical"
            )
        }
        return height
    }
}

/// Per-client identity used to keep authenticated operation provenance out of the public model API.
final class PrivacyActionOperationProvenanceOwnerV1: @unchecked Sendable {}

private final class PrivacyActionOperationProvenanceTokenV1: @unchecked Sendable {
    let owner: PrivacyActionOperationProvenanceOwnerV1
    let networkId: NetworkId
    let protocolId: PrivacyProtocolIdV1
    let operationSchema: PrivacyExact12ActionOperationV1
    let transactionHash: Data
    let transactionIntentDigest: Data
    let statementDigest: Data
    let proofEnvelopeHash: Data
    let ledgerEffectKind: PrivacyLedgerEffectKindV1
    let capabilityManifestDigest: Data
    let capabilityCommittedHeight: UInt64

    init(
        owner: PrivacyActionOperationProvenanceOwnerV1,
        networkId: NetworkId,
        view: PrivacyActionOperationViewV1
    ) {
        self.owner = owner
        self.networkId = networkId
        protocolId = view.protocolId
        operationSchema = view.operationSchema
        transactionHash = Data(view.transactionHash)
        transactionIntentDigest = Data(view.transactionIntentDigest)
        statementDigest = Data(view.statementDigest)
        proofEnvelopeHash = Data(view.proofEnvelopeHash)
        ledgerEffectKind = view.ledgerEffectKind
        capabilityManifestDigest = Data(view.capabilityManifestDigest)
        capabilityCommittedHeight = view.capabilityCommittedHeight
    }

    func matches(
        owner expectedOwner: PrivacyActionOperationProvenanceOwnerV1,
        networkId expectedNetworkId: NetworkId,
        view: PrivacyActionOperationViewV1
    ) -> Bool {
        owner === expectedOwner
            && networkId == expectedNetworkId
            && protocolId == view.protocolId
            && operationSchema == view.operationSchema
            && transactionHash == view.transactionHash
            && transactionIntentDigest == view.transactionIntentDigest
            && statementDigest == view.statementDigest
            && proofEnvelopeHash == view.proofEnvelopeHash
            && ledgerEffectKind == view.ledgerEffectKind
            && capabilityManifestDigest == view.capabilityManifestDigest
            && capabilityCommittedHeight == view.capabilityCommittedHeight
    }
}

/// Immutable public state of one authenticated Exact12 action submission.
///
/// Construction validates operation mappings, non-zero hashes, authenticated
/// heights, and the complete local/terminal state relationship. A view built
/// with the public initializer is detached and suitable for display only;
/// authenticated status queries accept only views returned by submission.
public struct PrivacyActionOperationViewV1: Equatable, Sendable {
    public let protocolId: PrivacyProtocolIdV1
    public let operationSchema: PrivacyExact12ActionOperationV1
    public let transactionHash: Data
    public let transactionIntentDigest: Data
    public let statementDigest: Data
    public let proofEnvelopeHash: Data
    public let localState: PrivacyActionLocalStateV1
    public let terminalChainState: PrivacyActionTerminalChainStateV1?
    public let committedHeight: UInt64?
    public let rejectionReason: String?
    public let ledgerEffectKind: PrivacyLedgerEffectKindV1
    /// Fresh finalized capability digest observed before submission.
    public let capabilityManifestDigest: Data
    /// Fresh finalized capability height observed before submission.
    public let capabilityCommittedHeight: UInt64
    /// Capability manifest actually admitted by native execution, when applied.
    public let executionCapabilityManifestDigest: Data?
    /// Height of the capability snapshot actually admitted by native execution.
    public let executionCapabilityCommittedHeight: UInt64?
    /// Finalized state height from the authenticated native execution receipt.
    public let executionReceiptFinalizedHeight: UInt64?
    /// Exact finalized block from the authenticated native execution receipt.
    public let executionReceiptFinalizedBlockHash: Data?
    private var authenticatedProvenance: PrivacyActionOperationProvenanceTokenV1?

    public init(
        protocolId: PrivacyProtocolIdV1,
        operationSchema: PrivacyExact12ActionOperationV1,
        transactionHash: Data,
        transactionIntentDigest: Data,
        statementDigest: Data,
        proofEnvelopeHash: Data,
        localState: PrivacyActionLocalStateV1,
        terminalChainState: PrivacyActionTerminalChainStateV1?,
        committedHeight: UInt64?,
        rejectionReason: String?,
        ledgerEffectKind: PrivacyLedgerEffectKindV1,
        capabilityManifestDigest: Data,
        capabilityCommittedHeight: UInt64,
        executionCapabilityManifestDigest: Data? = nil,
        executionCapabilityCommittedHeight: UInt64? = nil,
        executionReceiptFinalizedHeight: UInt64? = nil,
        executionReceiptFinalizedBlockHash: Data? = nil
    ) throws {
        guard protocolId == operationSchema.protocolId else {
            throw PrivacyExact12ActionModelErrorV1.operationProtocolMismatch
        }
        guard ledgerEffectKind == operationSchema.ledgerEffectKind else {
            throw PrivacyExact12ActionModelErrorV1.operationLedgerEffectMismatch
        }
        for (field, hash) in [
            ("transaction hash", transactionHash),
            ("transaction intent digest", transactionIntentDigest),
            ("statement digest", statementDigest),
            ("proof envelope hash", proofEnvelopeHash),
        ] where !PrivacyExact12ActionRequestV1.isNonzeroFixed32(hash) {
            throw PrivacyExact12ActionModelErrorV1.invalidHash(field: field)
        }
        guard PrivacyExact12ActionRequestV1.isNonzeroFixed32(capabilityManifestDigest) else {
            throw PrivacyExact12ActionModelErrorV1.invalidCapabilityManifestDigest
        }
        guard capabilityCommittedHeight > 0 else {
            throw PrivacyExact12ActionModelErrorV1.invalidCapabilityCommittedHeight
        }
        if let committedHeight, committedHeight == 0 {
            throw PrivacyExact12ActionModelErrorV1.invalidCommittedHeight
        }
        if let executionCapabilityManifestDigest,
           !PrivacyExact12ActionRequestV1.isNonzeroFixed32(executionCapabilityManifestDigest) {
            throw PrivacyExact12ActionModelErrorV1.invalidHash(
                field: "execution capability manifest digest"
            )
        }
        if executionCapabilityCommittedHeight == 0 {
            throw PrivacyExact12ActionModelErrorV1.invalidExecutionCapabilityCommittedHeight
        }
        if executionReceiptFinalizedHeight == 0 {
            throw PrivacyExact12ActionModelErrorV1.invalidExecutionReceiptFinalizedHeight
        }
        if let executionReceiptFinalizedBlockHash,
           !PrivacyExact12ActionRequestV1.isNonzeroFixed32(executionReceiptFinalizedBlockHash) {
            throw PrivacyExact12ActionModelErrorV1.invalidHash(
                field: "execution receipt finalized block hash"
            )
        }
        let hasAnyExecutionEvidence = executionCapabilityManifestDigest != nil
            || executionCapabilityCommittedHeight != nil
            || executionReceiptFinalizedHeight != nil
            || executionReceiptFinalizedBlockHash != nil
        let hasCompleteExecutionEvidence = executionCapabilityManifestDigest != nil
            && executionCapabilityCommittedHeight != nil
            && executionReceiptFinalizedHeight != nil
            && executionReceiptFinalizedBlockHash != nil

        switch (localState, terminalChainState) {
        case (.submitted, nil):
            guard committedHeight == nil,
                  rejectionReason == nil,
                  !hasAnyExecutionEvidence else {
                throw PrivacyExact12ActionModelErrorV1.invalidStateCombination
            }
        case let (.terminal, .some(terminalState)):
            switch terminalState {
            case .committed:
                guard committedHeight != nil,
                      rejectionReason == nil,
                      !hasAnyExecutionEvidence else {
                    throw PrivacyExact12ActionModelErrorV1.invalidStateCombination
                }
            case .applied:
                guard let committedHeight,
                      rejectionReason == nil,
                      hasCompleteExecutionEvidence,
                      let executionCapabilityCommittedHeight,
                      let executionReceiptFinalizedHeight else {
                    throw PrivacyExact12ActionModelErrorV1.invalidStateCombination
                }
                guard executionCapabilityCommittedHeight <= committedHeight,
                      executionReceiptFinalizedHeight >= committedHeight else {
                    throw PrivacyExact12ActionModelErrorV1.invalidExecutionReceiptHeights
                }
            case .rejected:
                guard committedHeight != nil, !hasAnyExecutionEvidence else {
                    throw PrivacyExact12ActionModelErrorV1.invalidStateCombination
                }
                guard Self.isCanonicalRejectionReason(rejectionReason) else {
                    throw PrivacyExact12ActionModelErrorV1.invalidRejectionReason
                }
            case .expired:
                guard committedHeight == nil,
                      rejectionReason == nil,
                      !hasAnyExecutionEvidence else {
                    throw PrivacyExact12ActionModelErrorV1.invalidStateCombination
                }
            }
        case (.submitted, .some), (.terminal, nil):
            throw PrivacyExact12ActionModelErrorV1.invalidStateCombination
        }
        if localState == .terminal,
           let committedHeight,
           committedHeight < capabilityCommittedHeight {
            throw PrivacyExact12ActionModelErrorV1.terminalHeightBeforeCapabilitySnapshot
        }

        self.protocolId = protocolId
        self.operationSchema = operationSchema
        self.transactionHash = Data(transactionHash)
        self.transactionIntentDigest = Data(transactionIntentDigest)
        self.statementDigest = Data(statementDigest)
        self.proofEnvelopeHash = Data(proofEnvelopeHash)
        self.localState = localState
        self.terminalChainState = terminalChainState
        self.committedHeight = committedHeight
        self.rejectionReason = rejectionReason
        self.ledgerEffectKind = ledgerEffectKind
        self.capabilityManifestDigest = Data(capabilityManifestDigest)
        self.capabilityCommittedHeight = capabilityCommittedHeight
        self.executionCapabilityManifestDigest = executionCapabilityManifestDigest.map { Data($0) }
        self.executionCapabilityCommittedHeight = executionCapabilityCommittedHeight
        self.executionReceiptFinalizedHeight = executionReceiptFinalizedHeight
        self.executionReceiptFinalizedBlockHash = executionReceiptFinalizedBlockHash.map { Data($0) }
        authenticatedProvenance = nil
    }

    public static func == (lhs: Self, rhs: Self) -> Bool {
        lhs.protocolId == rhs.protocolId
            && lhs.operationSchema == rhs.operationSchema
            && lhs.transactionHash == rhs.transactionHash
            && lhs.transactionIntentDigest == rhs.transactionIntentDigest
            && lhs.statementDigest == rhs.statementDigest
            && lhs.proofEnvelopeHash == rhs.proofEnvelopeHash
            && lhs.localState == rhs.localState
            && lhs.terminalChainState == rhs.terminalChainState
            && lhs.committedHeight == rhs.committedHeight
            && lhs.rejectionReason == rhs.rejectionReason
            && lhs.ledgerEffectKind == rhs.ledgerEffectKind
            && lhs.capabilityManifestDigest == rhs.capabilityManifestDigest
            && lhs.capabilityCommittedHeight == rhs.capabilityCommittedHeight
            && lhs.executionCapabilityManifestDigest == rhs.executionCapabilityManifestDigest
            && lhs.executionCapabilityCommittedHeight == rhs.executionCapabilityCommittedHeight
            && lhs.executionReceiptFinalizedHeight == rhs.executionReceiptFinalizedHeight
            && lhs.executionReceiptFinalizedBlockHash == rhs.executionReceiptFinalizedBlockHash
    }

    func bindingAuthenticatedSubmission(
        owner: PrivacyActionOperationProvenanceOwnerV1,
        networkId: NetworkId
    ) -> Self {
        precondition(localState == .submitted && terminalChainState == nil)
        precondition(authenticatedProvenance == nil)
        var bound = self
        bound.authenticatedProvenance = PrivacyActionOperationProvenanceTokenV1(
            owner: owner,
            networkId: networkId,
            view: self
        )
        return bound
    }

    func hasAuthenticatedProvenance(
        owner: PrivacyActionOperationProvenanceOwnerV1,
        networkId: NetworkId
    ) -> Bool {
        authenticatedProvenance?.matches(
            owner: owner,
            networkId: networkId,
            view: self
        ) == true
    }

    private static func isCanonicalRejectionReason(_ value: String?) -> Bool {
        guard let value, !value.isEmpty,
              value.utf8.count <= 1_024,
              value == value.trimmingCharacters(in: .whitespacesAndNewlines) else {
            return false
        }
        return !value.unicodeScalars.contains {
            CharacterSet.controlCharacters.contains($0)
        }
    }

    func replacingTerminalState(
        _ state: PrivacyActionTerminalChainStateV1,
        committedHeight: UInt64?,
        rejectionReason: String?,
        executionCapabilityManifestDigest: Data? = nil,
        executionCapabilityCommittedHeight: UInt64? = nil,
        executionReceiptFinalizedHeight: UInt64? = nil,
        executionReceiptFinalizedBlockHash: Data? = nil
    ) throws -> Self {
        var replaced = try Self(
            protocolId: protocolId,
            operationSchema: operationSchema,
            transactionHash: transactionHash,
            transactionIntentDigest: transactionIntentDigest,
            statementDigest: statementDigest,
            proofEnvelopeHash: proofEnvelopeHash,
            localState: .terminal,
            terminalChainState: state,
            committedHeight: committedHeight,
            rejectionReason: rejectionReason,
            ledgerEffectKind: ledgerEffectKind,
            capabilityManifestDigest: capabilityManifestDigest,
            capabilityCommittedHeight: capabilityCommittedHeight,
            executionCapabilityManifestDigest: executionCapabilityManifestDigest,
            executionCapabilityCommittedHeight: executionCapabilityCommittedHeight,
            executionReceiptFinalizedHeight: executionReceiptFinalizedHeight,
            executionReceiptFinalizedBlockHash: executionReceiptFinalizedBlockHash
        )
        replaced.authenticatedProvenance = authenticatedProvenance
        return replaced
    }
}
