import CryptoKit
import Foundation

// Closed, schema-shaped projection of the governed KAGEMUSHA release proposal.
// Native admission remains the authority for digest and signature authentication.
private func kagemushaReleaseDecodeError(_ decoder: Decoder, _ message: String) -> DecodingError {
  .dataCorrupted(.init(codingPath: decoder.codingPath, debugDescription: message))
}

private func kagemushaReleaseUpperHex(
  _ raw: String, bytes: Int, codingPath: [CodingKey], field: String
) throws -> Data {
  guard raw.utf8.count == bytes * 2,
    raw.utf8.allSatisfy({ (48...57).contains($0) || (65...70).contains($0) }),
    let decoded = Data(hexString: raw)
  else {
    throw DecodingError.dataCorrupted(
      .init(
        codingPath: codingPath,
        debugDescription: "\(field) must be exact uppercase hexadecimal of \(bytes) bytes"
      ))
  }
  return decoded
}

/// One fixed 32-byte KAGEMUSHA digest or identifier.
public struct ToriiGovernanceKagemushaBytes32V1: Decodable, Sendable, Equatable {
  public let bytes: Data
  public init(from decoder: Decoder) throws {
    bytes = try governanceFixedBytes(
      decoder.singleValueContainer().decode([UInt8].self),
      count: 32, codingPath: decoder.codingPath, field: "KAGEMUSHA digest"
    )
  }
}

/// One canonical uncompressed P-256 device public key in the native JSON layout.
public struct ToriiGovernanceKagemushaDevicePublicKeyV1: Decodable, Sendable, Equatable {
  public let bytes: Data
  public init(from decoder: Decoder) throws {
    let values = try decoder.singleValueContainer().decode([String].self)
    guard values.count == 1 else {
      throw kagemushaReleaseDecodeError(decoder, "device public key requires one SEC1 point")
    }
    bytes = try kagemushaReleaseUpperHex(
      values[0], bytes: 65, codingPath: decoder.codingPath, field: "device public key"
    )
    guard bytes.first == 0x04,
      (try? P256.Signing.PublicKey(x963Representation: bytes)) != nil
    else {
      throw kagemushaReleaseDecodeError(decoder, "device public key is not a canonical P-256 point")
    }
  }
}

/// One fixed-width low-S P-256 signature in the native JSON layout.
public struct ToriiGovernanceKagemushaDeviceSignatureV1: Decodable, Sendable, Equatable {
  public let bytes: Data
  public init(from decoder: Decoder) throws {
    let values = try decoder.singleValueContainer().decode([String].self)
    guard values.count == 1 else {
      throw kagemushaReleaseDecodeError(decoder, "device signature requires one value")
    }
    bytes = try kagemushaReleaseUpperHex(
      values[0], bytes: 64, codingPath: decoder.codingPath, field: "device signature"
    )
    guard (try? P256.Signing.ECDSASignature(rawRepresentation: bytes)) != nil else {
      throw kagemushaReleaseDecodeError(decoder, "device signature has invalid P-256 scalars")
    }
    let s = bytes.suffix(32)
    let halfOrder = Data([
      0x7F, 0xFF, 0xFF, 0xFF, 0x80, 0x00, 0x00, 0x00, 0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
      0xFF,
      0xDE, 0x73, 0x7D, 0x56, 0xD3, 0x8B, 0xCF, 0x42, 0x79, 0xDC, 0xE5, 0x61, 0x7E, 0x31, 0x92,
      0xA8,
    ])
    guard s.lexicographicallyPrecedes(halfOrder) || s.elementsEqual(halfOrder) else {
      throw kagemushaReleaseDecodeError(decoder, "device signature must be low-S")
    }
  }
}

public enum ToriiGovernanceKagemushaAcceptanceCaseTagV1: String, Decodable, Sendable, Equatable {
  case receiverInboxPressure = "receiver_inbox_pressure"
  case senderOutboxCapacityExhaustion = "sender_outbox_capacity_exhaustion"
  case crashDuringPrepare = "crash_during_prepare"
  case crashAfterPrepareBeforeProof = "crash_after_prepare_before_proof"
  case crashDuringProof = "crash_during_proof"
  case crashAfterProofBeforeCandidatePersistence = "crash_after_proof_before_candidate_persistence"
  case crashDuringCandidatePersistence = "crash_during_candidate_persistence"
  case crashAfterCandidatePersistenceBeforeVerification =
    "crash_after_candidate_persistence_before_verification"
  case crashDuringCandidateVerification = "crash_during_candidate_verification"
  case crashAfterCandidateVerificationBeforeHardwareCommit =
    "crash_after_candidate_verification_before_hardware_commit"
  case crashDuringHardwareCommit = "crash_during_hardware_commit"
  case crashAfterHardwareCommitBeforeTerminalAuthorization =
    "crash_after_hardware_commit_before_terminal_authorization"
  case crashDuringTerminalAuthorization = "crash_during_terminal_authorization"
  case crashAfterTerminalAuthorizationBeforeFinalEnvelopePersistence =
    "crash_after_terminal_authorization_before_final_envelope_persistence"
  case crashDuringFinalEnvelopePersistence = "crash_during_final_envelope_persistence"
  case crashAfterFinalEnvelopePersistenceBeforeExposure =
    "crash_after_final_envelope_persistence_before_exposure"
  case crashDuringExposure = "crash_during_exposure"
  case crashDuringTransport = "crash_during_transport"
  case crashAfterTransportBeforeInboxStage = "crash_after_transport_before_inbox_stage"
  case crashDuringInboxStage = "crash_during_inbox_stage"
  case crashAfterInboxStageBeforeAck = "crash_after_inbox_stage_before_ack"
  case crashDuringAckPersistence = "crash_during_ack_persistence"
  case crashAfterAckPersistenceBeforeExposure = "crash_after_ack_persistence_before_exposure"
  case crashDuringAckExposure = "crash_during_ack_exposure"
  case crashDuringAckRecovery = "crash_during_ack_recovery"
  case ackRecoveryIdempotence = "ack_recovery_idempotence"
  case crashDuringRecovery = "crash_during_recovery"
  case recoveryIdempotence = "recovery_idempotence"
  case missingSenderAuthorization = "missing_sender_authorization"
  case forgedSenderAuthorization = "forged_sender_authorization"
  case replayedSenderAuthorization = "replayed_sender_authorization"
  case crossReleaseSenderAuthorization = "cross_release_sender_authorization"
  case missingMintAuthorization = "missing_mint_authorization"
  case forgedMintAuthorization = "forged_mint_authorization"
  case replayedMintAuthorization = "replayed_mint_authorization"
  case crossReleaseMintAuthorization = "cross_release_mint_authorization"
  case shuffledConcurrentRequests = "shuffled_concurrent_requests"
  case delayedDeliveryAfterRequestExpiry = "delayed_delivery_after_request_expiry"
  case delayedDeliveryAcrossOrdinarySuiteRotation =
    "delayed_delivery_across_ordinary_suite_rotation"
  case delayedDeliveryAcrossCredentialRotation = "delayed_delivery_across_credential_rotation"
  case positiveExactRequest = "positive_exact_request"
  case recipientKeyBinding = "recipient_key_binding"
  case requestAmountMismatchRejection = "request_amount_mismatch_rejection"
  case committedPaymentAfterRequestExpiry = "committed_payment_after_request_expiry"
  case exactAmountBinding = "exact_amount_binding"
  case distinctPaymentsSameRequest = "distinct_payments_same_request"
  case shuffledConcurrentPaymentsSameRequest = "shuffled_concurrent_payments_same_request"
  case invoiceDeduplicationApplicationPolicy = "invoice_deduplication_application_policy"
  case duplicateTransport = "duplicate_transport"
  case exactDuplicateDurableAck = "exact_duplicate_durable_ack"
  case conflictingCreditIdBytes = "conflicting_credit_id_bytes"
  case sameCreditReplay = "same_credit_replay"
  case staleState = "stale_state"
  case twoSuccessorsFromOnePredecessor = "two_successors_from_one_predecessor"
  case rollback = "rollback"
  case clockRollback = "clock_rollback"
  case monotonicLeaseExpiry = "monotonic_lease_expiry"
  case counterReuseOrSkip = "counter_reuse_or_skip"
  case forgedEpochRotation = "forged_epoch_rotation"
  case hardwareEpochRollover = "hardware_epoch_rollover"
  case hardwareCounterRollover = "hardware_counter_rollover"
  case ordinaryVerifierRotation = "ordinary_verifier_rotation"
  case emergencySuspensionOnlineRecovery = "emergency_suspension_online_recovery"
  case arithmeticOverflow = "arithmetic_overflow"
  case proofOutputSubstitution = "proof_output_substitution"
  case transcriptUnlinkability = "transcript_unlinkability"
  case x25519LowOrderPublicKeyRejection = "x25519_low_order_public_key_rejection"
  case x25519ZeroDhRejection = "x25519_zero_dh_rejection"
  case aeadCiphertextSubstitution = "aead_ciphertext_substitution"
  case aeadAssociatedDataSubstitution = "aead_associated_data_substitution"
  case deterministicEncryptionInjectedRandomnessKat =
    "deterministic_encryption_injected_randomness_kat"
  case receiveFoldSingleCredit = "receive_fold_single_credit"
  case receiveFoldReplayAtomicity = "receive_fold_replay_atomicity"
  case pendingCreditBacklogNoCountRejection = "pending_credit_backlog_no_count_rejection"
  case reserveUnderflow = "reserve_underflow"
  case duplicateRedemption = "duplicate_redemption"
  case concurrentRedemption = "concurrent_redemption"
  case topUpRecovery = "top_up_recovery"
  case fullRedemption = "full_redemption"
  case partialRedemption = "partial_redemption"
  case zeroBalanceContinuation = "zero_balance_continuation"
  case animatedQrLossRecovery = "animated_qr_loss_recovery"
  case animatedQrReorderingRecovery = "animated_qr_reordering_recovery"
  case staticQrSizeGuard = "static_qr_size_guard"
  case fourPeerActivationRestartReplay = "four_peer_activation_restart_replay"
  case physicalAirplaneMode = "physical_airplane_mode"
  case physicalRestart = "physical_restart"
  case physicalPowerLoss = "physical_power_loss"
  case physicalClockRollback = "physical_clock_rollback"
  case physicalBackupRestoreRejection = "physical_backup_restore_rejection"
  case physicalMemoryAndLatency = "physical_memory_and_latency"
  case physicalThermalFolding = "physical_thermal_folding"
  case noSoftwareFallback = "no_software_fallback"
  case nativeFixtureSwift = "native_fixture_swift"
  case nativeFixtureKotlin = "native_fixture_kotlin"
  case nativeFixtureJava = "native_fixture_java"
  case nativeFixtureJavaScript = "native_fixture_java_script"
  case nativeFixturePython = "native_fixture_python"
  case nativeFixtureCSharp = "native_fixture_c_sharp"
  case nativeFixtureJni = "native_fixture_jni"
  case nativeFixtureQr = "native_fixture_qr"
  case nativeFixtureNfc = "native_fixture_nfc"
}

public enum ToriiGovernanceKagemushaArtifactRoleTagV1: String, Decodable, Sendable, Equatable {
  case paramsEq = "params_eq"
  case paramsEp = "params_ep"
  case innerStatePkEq = "inner_state_pk_eq"
  case innerStateVkEq = "inner_state_vk_eq"
  case innerStatePkEp = "inner_state_pk_ep"
  case innerStateVkEp = "inner_state_vk_ep"
  case statePkEq = "state_pk_eq"
  case stateVkEq = "state_vk_eq"
  case statePkEp = "state_pk_ep"
  case stateVkEp = "state_vk_ep"
  case mintAuthorizationPkEq = "mint_authorization_pk_eq"
  case mintAuthorizationVkEq = "mint_authorization_vk_eq"
  case mintAuthorizationPkEp = "mint_authorization_pk_ep"
  case mintAuthorizationVkEp = "mint_authorization_vk_ep"
  case mintCreditPkEq = "mint_credit_pk_eq"
  case mintCreditVkEq = "mint_credit_vk_eq"
  case mintCreditPkEp = "mint_credit_pk_ep"
  case mintCreditVkEp = "mint_credit_vk_ep"
  case platformCredentialPkEq = "platform_credential_pk_eq"
  case platformCredentialVkEq = "platform_credential_vk_eq"
  case platformCredentialPkEp = "platform_credential_pk_ep"
  case platformCredentialVkEp = "platform_credential_vk_ep"
  case guardBundlePkEq = "guard_bundle_pk_eq"
  case guardBundleVkEq = "guard_bundle_vk_eq"
  case guardBundlePkEp = "guard_bundle_pk_ep"
  case guardBundleVkEp = "guard_bundle_vk_ep"
  case terminalAuthorizationPkEq = "terminal_authorization_pk_eq"
  case terminalAuthorizationVkEq = "terminal_authorization_vk_eq"
  case terminalAuthorizationPkEp = "terminal_authorization_pk_ep"
  case terminalAuthorizationVkEp = "terminal_authorization_vk_ep"
  case commitWrapperPkEq = "commit_wrapper_pk_eq"
  case commitWrapperVkEq = "commit_wrapper_vk_eq"
  case commitWrapperPkEp = "commit_wrapper_pk_ep"
  case commitWrapperVkEp = "commit_wrapper_vk_ep"
  case innerMintAuthorizationPkEq = "inner_mint_authorization_pk_eq"
  case innerMintAuthorizationVkEq = "inner_mint_authorization_vk_eq"
  case innerMintAuthorizationPkEp = "inner_mint_authorization_pk_ep"
  case innerMintAuthorizationVkEp = "inner_mint_authorization_vk_ep"
  case innerMintCreditPkEq = "inner_mint_credit_pk_eq"
  case innerMintCreditVkEq = "inner_mint_credit_vk_eq"
  case innerMintCreditPkEp = "inner_mint_credit_pk_ep"
  case innerMintCreditVkEp = "inner_mint_credit_vk_ep"
  case mintHashShardPkEq = "mint_hash_shard_pk_eq"
  case mintHashShardVkEq = "mint_hash_shard_vk_eq"
  case mintHashShardPkEp = "mint_hash_shard_pk_ep"
  case mintHashShardVkEp = "mint_hash_shard_vk_ep"
  case mintHashClaimPkEq = "mint_hash_claim_pk_eq"
  case mintHashClaimVkEq = "mint_hash_claim_vk_eq"
  case mintHashClaimPkEp = "mint_hash_claim_pk_ep"
  case mintHashClaimVkEp = "mint_hash_claim_vk_ep"
  case ordinaryAppGuardPkEq = "ordinary_app_guard_pk_eq"
  case ordinaryAppGuardVkEq = "ordinary_app_guard_vk_eq"
  case ordinaryAppGuardPkEp = "ordinary_app_guard_pk_ep"
  case ordinaryAppGuardVkEp = "ordinary_app_guard_vk_ep"
}

public enum ToriiGovernanceKagemushaGovernedVerifierReleaseStatusV1: UInt8, Decodable, Sendable,
  Equatable
{
  case active = 1
  case standby = 2
  case verificationOnly = 3
}

public enum ToriiGovernanceKagemushaHardwarePlatformClassTagV1: String, Decodable, Sendable,
  Equatable
{
  case androidOemService = "android_oem_service"
  case appleOemService = "apple_oem_service"
  case dedicatedSecureElement = "dedicated_secure_element"
  case otherQualified = "other_qualified"
}

public enum ToriiGovernanceKagemushaQualifiedHelperCircuitTagV1: String, Decodable, Sendable,
  Equatable
{
  case mintAuthorization = "mint_authorization"
  case mintCredit = "mint_credit"
  case platformCredential = "platform_credential"
  case guardBundle = "guard_bundle"
  case mintHashShard = "mint_hash_shard"
  case mintHashClaim = "mint_hash_claim"
  case ordinaryAppGuard = "ordinary_app_guard"
}

public enum ToriiGovernanceKagemushaQualifiedRelationTagV1: String, Decodable, Sendable, Equatable {
  case bootstrap = "bootstrap"
  case mintFold = "mint_fold"
  case sendSplit = "send_split"
  case receiveFold = "receive_fold"
  case redeemSplit = "redeem_split"
  case rotate = "rotate"
  case terminalAuthorization = "terminal_authorization"
  case commitWrapper = "commit_wrapper"
}

/// Exact AcceptanceCaseEvidenceV1 projection.
public struct ToriiGovernanceKagemushaAcceptanceCaseEvidenceV1: Decodable, Sendable, Equatable {
  public let caseName: ToriiGovernanceKagemushaAcceptanceCaseV1
  public let validatorCount: UInt8
  public let report: ToriiGovernanceKagemushaEvidenceFileV1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case caseName = "case"
    case validatorCount = "validator_count"
    case report = "report"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaAcceptanceCaseEvidenceV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    caseName = try container.decode(
      ToriiGovernanceKagemushaAcceptanceCaseV1.self, forKey: .caseName)
    validatorCount = try container.decode(UInt8.self, forKey: .validatorCount)
    report = try container.decode(ToriiGovernanceKagemushaEvidenceFileV1.self, forKey: .report)
  }
}

/// Exact AcceptanceCaseV1 projection.
public struct ToriiGovernanceKagemushaAcceptanceCaseV1: Decodable, Sendable, Equatable {
  public let caseName: ToriiGovernanceKagemushaAcceptanceCaseTagV1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case caseName = "case"
    case value = "value"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaAcceptanceCaseV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    caseName = try container.decode(
      ToriiGovernanceKagemushaAcceptanceCaseTagV1.self, forKey: .caseName)
    guard container.contains(.value), try container.decodeNil(forKey: .value) else {
      throw kagemushaReleaseDecodeError(decoder, "value must be explicit null")
    }
  }
}

/// Exact AggregateBalanceQualificationV1 projection.
public struct ToriiGovernanceKagemushaAggregateBalanceQualificationV1: Decodable, Sendable,
  Equatable
{
  public let independentPayments: UInt32
  public let foldedCredits: UInt32
  public let spendPayments: UInt32
  public let report: ToriiGovernanceKagemushaEvidenceFileV1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case independentPayments = "independent_payments"
    case foldedCredits = "folded_credits"
    case spendPayments = "spend_payments"
    case report = "report"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaAggregateBalanceQualificationV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    independentPayments = try container.decode(UInt32.self, forKey: .independentPayments)
    foldedCredits = try container.decode(UInt32.self, forKey: .foldedCredits)
    spendPayments = try container.decode(UInt32.self, forKey: .spendPayments)
    report = try container.decode(ToriiGovernanceKagemushaEvidenceFileV1.self, forKey: .report)
  }
}

/// Exact ArtifactBindingV1 projection.
public struct ToriiGovernanceKagemushaArtifactBindingV1: Decodable, Sendable, Equatable {
  public let role: ToriiGovernanceKagemushaArtifactRoleV1
  public let sha256: ToriiGovernanceKagemushaBytes32V1
  public let byteLen: UInt64

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case role = "role"
    case sha256 = "sha256"
    case byteLen = "byte_len"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaArtifactBindingV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    role = try container.decode(ToriiGovernanceKagemushaArtifactRoleV1.self, forKey: .role)
    sha256 = try container.decode(ToriiGovernanceKagemushaBytes32V1.self, forKey: .sha256)
    byteLen = try container.decode(UInt64.self, forKey: .byteLen)
    guard byteLen <= 9_007_199_254_740_991 else {
      throw kagemushaReleaseDecodeError(decoder, "byte_len is outside exact V1 JSON integer range")
    }
  }
}

/// Exact ArtifactRoleV1 projection.
public struct ToriiGovernanceKagemushaArtifactRoleV1: Decodable, Sendable, Equatable {
  public let role: ToriiGovernanceKagemushaArtifactRoleTagV1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case role = "role"
    case value = "value"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaArtifactRoleV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    role = try container.decode(ToriiGovernanceKagemushaArtifactRoleTagV1.self, forKey: .role)
    guard container.contains(.value), try container.decodeNil(forKey: .value) else {
      throw kagemushaReleaseDecodeError(decoder, "value must be explicit null")
    }
  }
}

/// Exact EnabledProfileV1 projection.
public struct ToriiGovernanceKagemushaEnabledProfileV1: Decodable, Sendable, Equatable {
  public let hardwareProfile: ToriiGovernanceKagemushaHardwareProfileV1
  public let hardwareProfileId: ToriiGovernanceKagemushaBytes32V1
  public let suiteId: ToriiGovernanceKagemushaBytes32V1
  public let vkDigest: ToriiGovernanceKagemushaBytes32V1
  public let qualificationDigest: ToriiGovernanceKagemushaBytes32V1
  public let policyEpoch: UInt64
  public let qualificationReport: ToriiGovernanceKagemushaEvidenceFileV1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case hardwareProfile = "hardware_profile"
    case hardwareProfileId = "hardware_profile_id"
    case suiteId = "suite_id"
    case vkDigest = "vk_digest"
    case qualificationDigest = "qualification_digest"
    case policyEpoch = "policy_epoch"
    case qualificationReport = "qualification_report"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaEnabledProfileV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    hardwareProfile = try container.decode(
      ToriiGovernanceKagemushaHardwareProfileV1.self, forKey: .hardwareProfile)
    hardwareProfileId = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .hardwareProfileId)
    suiteId = try container.decode(ToriiGovernanceKagemushaBytes32V1.self, forKey: .suiteId)
    vkDigest = try container.decode(ToriiGovernanceKagemushaBytes32V1.self, forKey: .vkDigest)
    qualificationDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .qualificationDigest)
    policyEpoch = try container.decode(UInt64.self, forKey: .policyEpoch)
    guard policyEpoch <= 9_007_199_254_740_991 else {
      throw kagemushaReleaseDecodeError(
        decoder, "policy_epoch is outside exact V1 JSON integer range")
    }
    qualificationReport = try container.decode(
      ToriiGovernanceKagemushaEvidenceFileV1.self, forKey: .qualificationReport)
  }
}

/// Exact EnvelopeQualificationV1 projection.
public struct ToriiGovernanceKagemushaEnvelopeQualificationV1: Decodable, Sendable, Equatable {
  public let rawCompleteExchangeBytes: UInt32
  public let textCompleteExchangeBytes: UInt32
  public let handoffP95Ms: UInt32
  public let report: ToriiGovernanceKagemushaEvidenceFileV1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case rawCompleteExchangeBytes = "raw_complete_exchange_bytes"
    case textCompleteExchangeBytes = "text_complete_exchange_bytes"
    case handoffP95Ms = "handoff_p95_ms"
    case report = "report"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaEnvelopeQualificationV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    rawCompleteExchangeBytes = try container.decode(UInt32.self, forKey: .rawCompleteExchangeBytes)
    textCompleteExchangeBytes = try container.decode(
      UInt32.self, forKey: .textCompleteExchangeBytes)
    handoffP95Ms = try container.decode(UInt32.self, forKey: .handoffP95Ms)
    report = try container.decode(ToriiGovernanceKagemushaEvidenceFileV1.self, forKey: .report)
  }
}

/// Exact EvidenceClosureV1 projection.
public struct ToriiGovernanceKagemushaEvidenceClosureV1: Decodable, Sendable, Equatable {
  public let evidenceManifest: ToriiGovernanceKagemushaEvidenceFileV1
  public let observerPolicy: ToriiGovernanceKagemushaEvidenceFileV1
  public let verificationRecordsDigest: ToriiGovernanceKagemushaBytes32V1
  public let candidateContextDigest: ToriiGovernanceKagemushaBytes32V1
  public let verificationRecordCount: UInt32
  public let totalEvidenceBytes: UInt64
  public let totalTranscriptBytes: UInt64
  public let totalCommandInputBytes: UInt64
  public let totalObservedDurationMs: UInt64
  public let totalObservedCpuMs: UInt64

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case evidenceManifest = "evidence_manifest"
    case observerPolicy = "observer_policy"
    case verificationRecordsDigest = "verification_records_digest"
    case candidateContextDigest = "candidate_context_digest"
    case verificationRecordCount = "verification_record_count"
    case totalEvidenceBytes = "total_evidence_bytes"
    case totalTranscriptBytes = "total_transcript_bytes"
    case totalCommandInputBytes = "total_command_input_bytes"
    case totalObservedDurationMs = "total_observed_duration_ms"
    case totalObservedCpuMs = "total_observed_cpu_ms"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaEvidenceClosureV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    evidenceManifest = try container.decode(
      ToriiGovernanceKagemushaEvidenceFileV1.self, forKey: .evidenceManifest)
    observerPolicy = try container.decode(
      ToriiGovernanceKagemushaEvidenceFileV1.self, forKey: .observerPolicy)
    verificationRecordsDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .verificationRecordsDigest)
    candidateContextDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .candidateContextDigest)
    verificationRecordCount = try container.decode(UInt32.self, forKey: .verificationRecordCount)
    totalEvidenceBytes = try container.decode(UInt64.self, forKey: .totalEvidenceBytes)
    guard totalEvidenceBytes <= 9_007_199_254_740_991 else {
      throw kagemushaReleaseDecodeError(
        decoder, "total_evidence_bytes is outside exact V1 JSON integer range")
    }
    totalTranscriptBytes = try container.decode(UInt64.self, forKey: .totalTranscriptBytes)
    guard totalTranscriptBytes <= 9_007_199_254_740_991 else {
      throw kagemushaReleaseDecodeError(
        decoder, "total_transcript_bytes is outside exact V1 JSON integer range")
    }
    totalCommandInputBytes = try container.decode(UInt64.self, forKey: .totalCommandInputBytes)
    guard totalCommandInputBytes <= 9_007_199_254_740_991 else {
      throw kagemushaReleaseDecodeError(
        decoder, "total_command_input_bytes is outside exact V1 JSON integer range")
    }
    totalObservedDurationMs = try container.decode(UInt64.self, forKey: .totalObservedDurationMs)
    guard totalObservedDurationMs <= 9_007_199_254_740_991 else {
      throw kagemushaReleaseDecodeError(
        decoder, "total_observed_duration_ms is outside exact V1 JSON integer range")
    }
    totalObservedCpuMs = try container.decode(UInt64.self, forKey: .totalObservedCpuMs)
    guard totalObservedCpuMs <= 9_007_199_254_740_991 else {
      throw kagemushaReleaseDecodeError(
        decoder, "total_observed_cpu_ms is outside exact V1 JSON integer range")
    }
  }
}

/// Exact EvidenceFileV1 projection.
public struct ToriiGovernanceKagemushaEvidenceFileV1: Decodable, Sendable, Equatable {
  public let sha256: ToriiGovernanceKagemushaBytes32V1
  public let byteLen: UInt64

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case sha256 = "sha256"
    case byteLen = "byte_len"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaEvidenceFileV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    sha256 = try container.decode(ToriiGovernanceKagemushaBytes32V1.self, forKey: .sha256)
    byteLen = try container.decode(UInt64.self, forKey: .byteLen)
    guard byteLen <= 9_007_199_254_740_991 else {
      throw kagemushaReleaseDecodeError(decoder, "byte_len is outside exact V1 JSON integer range")
    }
  }
}

/// Exact GovernedVerifierRegistryV1 projection.
public struct ToriiGovernanceKagemushaGovernedVerifierRegistryV1: Decodable, Sendable, Equatable {
  public let version: UInt16
  public let authorityPolicy: ToriiGovernanceKagemushaReleaseAuthorityPolicyV1?
  public let activeReleaseId: Data?
  public let releases: [ToriiGovernanceKagemushaGovernedVerifierReleaseV1]

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case version = "version"
    case authorityPolicy = "authority_policy"
    case activeReleaseId = "active_release_id"
    case releases = "releases"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaGovernedVerifierRegistryV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    version = try container.decode(UInt16.self, forKey: .version)
    guard version == 1 else {
      throw kagemushaReleaseDecodeError(decoder, "version must be 1")
    }
    guard container.contains(.authorityPolicy) else {
      throw kagemushaReleaseDecodeError(decoder, "authority_policy must be present")
    }
    authorityPolicy = try container.decodeIfPresent(
      ToriiGovernanceKagemushaReleaseAuthorityPolicyV1.self, forKey: .authorityPolicy)
    guard container.contains(.activeReleaseId) else {
      throw kagemushaReleaseDecodeError(decoder, "active_release_id must be present")
    }
    if let raw = try container.decodeIfPresent(String.self, forKey: .activeReleaseId) {
      activeReleaseId = try kagemushaReleaseUpperHex(
        raw, bytes: 32, codingPath: container.codingPath + [CodingKeys.activeReleaseId],
        field: "active_release_id"
      )
    } else {
      activeReleaseId = nil
    }
    releases = try container.decode(
      [ToriiGovernanceKagemushaGovernedVerifierReleaseV1].self, forKey: .releases)
    guard releases.count <= 64 else {
      throw kagemushaReleaseDecodeError(decoder, "releases has invalid array length")
    }
  }
}

/// Exact GovernedVerifierReleaseV1 projection.
public struct ToriiGovernanceKagemushaGovernedVerifierReleaseV1: Decodable, Sendable, Equatable {
  public let releaseId: ToriiGovernanceKagemushaBytes32V1
  public let status: ToriiGovernanceKagemushaGovernedVerifierReleaseStatusV1
  public let profileDigest: ToriiGovernanceKagemushaBytes32V1
  public let artifactManifestDigest: ToriiGovernanceKagemushaBytes32V1
  public let receiptDigest: ToriiGovernanceKagemushaBytes32V1
  public let attestationDigest: ToriiGovernanceKagemushaBytes32V1
  public let authorityPolicyDigest: ToriiGovernanceKagemushaBytes32V1
  public let hardwarePolicyDigest: ToriiGovernanceKagemushaBytes32V1
  public let nativeProfileDigest: ToriiGovernanceKagemushaBytes32V1
  public let providerPolicyRoot: ToriiGovernanceKagemushaBytes32V1
  public let suiteId: ToriiGovernanceKagemushaBytes32V1
  public let vkSetDigest: ToriiGovernanceKagemushaBytes32V1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case releaseId = "release_id"
    case status = "status"
    case profileDigest = "profile_digest"
    case artifactManifestDigest = "artifact_manifest_digest"
    case receiptDigest = "receipt_digest"
    case attestationDigest = "attestation_digest"
    case authorityPolicyDigest = "authority_policy_digest"
    case hardwarePolicyDigest = "hardware_policy_digest"
    case nativeProfileDigest = "native_profile_digest"
    case providerPolicyRoot = "provider_policy_root"
    case suiteId = "suite_id"
    case vkSetDigest = "vk_set_digest"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaGovernedVerifierReleaseV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    releaseId = try container.decode(ToriiGovernanceKagemushaBytes32V1.self, forKey: .releaseId)
    status = try container.decode(
      ToriiGovernanceKagemushaGovernedVerifierReleaseStatusV1.self, forKey: .status)
    profileDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .profileDigest)
    artifactManifestDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .artifactManifestDigest)
    receiptDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .receiptDigest)
    attestationDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .attestationDigest)
    authorityPolicyDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .authorityPolicyDigest)
    hardwarePolicyDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .hardwarePolicyDigest)
    nativeProfileDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .nativeProfileDigest)
    providerPolicyRoot = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .providerPolicyRoot)
    suiteId = try container.decode(ToriiGovernanceKagemushaBytes32V1.self, forKey: .suiteId)
    vkSetDigest = try container.decode(ToriiGovernanceKagemushaBytes32V1.self, forKey: .vkSetDigest)
  }
}

/// Exact HardwarePlatformClassV1 projection.
public struct ToriiGovernanceKagemushaHardwarePlatformClassV1: Decodable, Sendable, Equatable {
  public let platformClass: ToriiGovernanceKagemushaHardwarePlatformClassTagV1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case platformClass = "class"
    case value = "value"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaHardwarePlatformClassV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    platformClass = try container.decode(
      ToriiGovernanceKagemushaHardwarePlatformClassTagV1.self, forKey: .platformClass)
    guard container.contains(.value), try container.decodeNil(forKey: .value) else {
      throw kagemushaReleaseDecodeError(decoder, "value must be explicit null")
    }
  }
}

/// Exact HardwareProfileV1 projection.
public struct ToriiGovernanceKagemushaHardwareProfileV1: Decodable, Sendable, Equatable {
  public let version: UInt16
  public let protocolVersion: UInt16
  public let hardwareProfileId: ToriiGovernanceKagemushaBytes32V1
  public let providerId: ToriiGovernanceKagemushaBytes32V1
  public let platformClass: ToriiGovernanceKagemushaHardwarePlatformClassV1
  public let productClassDigest: ToriiGovernanceKagemushaBytes32V1
  public let firmwarePolicyDigest: ToriiGovernanceKagemushaBytes32V1
  public let enrollmentAttestationVerifierDigest: ToriiGovernanceKagemushaBytes32V1
  public let attestationTrustRootsDigest: ToriiGovernanceKagemushaBytes32V1
  public let allowedSuiteCommitment: ToriiGovernanceKagemushaBytes32V1
  public let policyEpoch: UInt64
  public let governanceCredentialPublicKey: ToriiGovernanceKagemushaDevicePublicKeyV1
  public let capabilityMask: UInt32
  public let qualificationReportDigest: ToriiGovernanceKagemushaBytes32V1
  public let validFromMs: UInt64
  public let expiresAtMs: UInt64
  public let appAttestationAuthorityPolicyDigest: ToriiGovernanceKagemushaBytes32V1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case version = "version"
    case protocolVersion = "protocol_version"
    case hardwareProfileId = "hardware_profile_id"
    case providerId = "provider_id"
    case platformClass = "platform_class"
    case productClassDigest = "product_class_digest"
    case firmwarePolicyDigest = "firmware_policy_digest"
    case enrollmentAttestationVerifierDigest = "enrollment_attestation_verifier_digest"
    case attestationTrustRootsDigest = "attestation_trust_roots_digest"
    case allowedSuiteCommitment = "allowed_suite_commitment"
    case policyEpoch = "policy_epoch"
    case governanceCredentialPublicKey = "governance_credential_public_key"
    case capabilityMask = "capability_mask"
    case qualificationReportDigest = "qualification_report_digest"
    case validFromMs = "valid_from_ms"
    case expiresAtMs = "expires_at_ms"
    case appAttestationAuthorityPolicyDigest = "app_attestation_authority_policy_digest"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaHardwareProfileV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    version = try container.decode(UInt16.self, forKey: .version)
    guard version == 1 else {
      throw kagemushaReleaseDecodeError(decoder, "version must be 1")
    }
    protocolVersion = try container.decode(UInt16.self, forKey: .protocolVersion)
    guard protocolVersion == 1 else {
      throw kagemushaReleaseDecodeError(decoder, "protocol_version must be 1")
    }
    hardwareProfileId = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .hardwareProfileId)
    providerId = try container.decode(ToriiGovernanceKagemushaBytes32V1.self, forKey: .providerId)
    platformClass = try container.decode(
      ToriiGovernanceKagemushaHardwarePlatformClassV1.self, forKey: .platformClass)
    productClassDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .productClassDigest)
    firmwarePolicyDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .firmwarePolicyDigest)
    enrollmentAttestationVerifierDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .enrollmentAttestationVerifierDigest)
    attestationTrustRootsDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .attestationTrustRootsDigest)
    allowedSuiteCommitment = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .allowedSuiteCommitment)
    policyEpoch = try container.decode(UInt64.self, forKey: .policyEpoch)
    guard policyEpoch <= 9_007_199_254_740_991 else {
      throw kagemushaReleaseDecodeError(
        decoder, "policy_epoch is outside exact V1 JSON integer range")
    }
    governanceCredentialPublicKey = try container.decode(
      ToriiGovernanceKagemushaDevicePublicKeyV1.self, forKey: .governanceCredentialPublicKey)
    capabilityMask = try container.decode(UInt32.self, forKey: .capabilityMask)
    qualificationReportDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .qualificationReportDigest)
    validFromMs = try container.decode(UInt64.self, forKey: .validFromMs)
    guard validFromMs <= 9_007_199_254_740_991 else {
      throw kagemushaReleaseDecodeError(
        decoder, "valid_from_ms is outside exact V1 JSON integer range")
    }
    appAttestationAuthorityPolicyDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .appAttestationAuthorityPolicyDigest)
    expiresAtMs = try container.decode(UInt64.self, forKey: .expiresAtMs)
    guard expiresAtMs <= 9_007_199_254_740_991 else {
      throw kagemushaReleaseDecodeError(
        decoder, "expires_at_ms is outside exact V1 JSON integer range")
    }
  }
}

/// Exact HelperProtocolV1 projection.
public struct ToriiGovernanceKagemushaHelperProtocolV1: Decodable, Sendable, Equatable {
  public let helper: ToriiGovernanceKagemushaQualifiedHelperCircuitV1
  public let eqProtocolDigest: ToriiGovernanceKagemushaBytes32V1
  public let epProtocolDigest: ToriiGovernanceKagemushaBytes32V1
  public let eqProofBytes: UInt32
  public let epProofBytes: UInt32

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case helper = "helper"
    case eqProtocolDigest = "eq_protocol_digest"
    case epProtocolDigest = "ep_protocol_digest"
    case eqProofBytes = "eq_proof_bytes"
    case epProofBytes = "ep_proof_bytes"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaHelperProtocolV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    helper = try container.decode(
      ToriiGovernanceKagemushaQualifiedHelperCircuitV1.self, forKey: .helper)
    eqProtocolDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .eqProtocolDigest)
    epProtocolDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .epProtocolDigest)
    eqProofBytes = try container.decode(UInt32.self, forKey: .eqProofBytes)
    epProofBytes = try container.decode(UInt32.self, forKey: .epProofBytes)
  }
}

/// Exact HelperQualificationV1 projection.
public struct ToriiGovernanceKagemushaHelperQualificationV1: Decodable, Sendable, Equatable {
  public let helper: ToriiGovernanceKagemushaQualifiedHelperCircuitV1
  public let eqProtocolDigest: ToriiGovernanceKagemushaBytes32V1
  public let epProtocolDigest: ToriiGovernanceKagemushaBytes32V1
  public let eqVerifyingKey: ToriiGovernanceKagemushaArtifactBindingV1
  public let epVerifyingKey: ToriiGovernanceKagemushaArtifactBindingV1
  public let eqCircuitRows: UInt32
  public let epCircuitRows: UInt32
  public let eqProofBytes: UInt32
  public let epProofBytes: UInt32
  public let completeProofBytes: UInt32
  public let proveP95Ms: UInt32
  public let verifyP95Ms: UInt32
  public let processRssBytes: UInt64
  public let operationEnergyMillijoules: UInt64
  public let report: ToriiGovernanceKagemushaEvidenceFileV1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case helper = "helper"
    case eqProtocolDigest = "eq_protocol_digest"
    case epProtocolDigest = "ep_protocol_digest"
    case eqVerifyingKey = "eq_verifying_key"
    case epVerifyingKey = "ep_verifying_key"
    case eqCircuitRows = "eq_circuit_rows"
    case epCircuitRows = "ep_circuit_rows"
    case eqProofBytes = "eq_proof_bytes"
    case epProofBytes = "ep_proof_bytes"
    case completeProofBytes = "complete_proof_bytes"
    case proveP95Ms = "prove_p95_ms"
    case verifyP95Ms = "verify_p95_ms"
    case processRssBytes = "process_rss_bytes"
    case operationEnergyMillijoules = "operation_energy_millijoules"
    case report = "report"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaHelperQualificationV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    helper = try container.decode(
      ToriiGovernanceKagemushaQualifiedHelperCircuitV1.self, forKey: .helper)
    eqProtocolDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .eqProtocolDigest)
    epProtocolDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .epProtocolDigest)
    eqVerifyingKey = try container.decode(
      ToriiGovernanceKagemushaArtifactBindingV1.self, forKey: .eqVerifyingKey)
    epVerifyingKey = try container.decode(
      ToriiGovernanceKagemushaArtifactBindingV1.self, forKey: .epVerifyingKey)
    eqCircuitRows = try container.decode(UInt32.self, forKey: .eqCircuitRows)
    epCircuitRows = try container.decode(UInt32.self, forKey: .epCircuitRows)
    eqProofBytes = try container.decode(UInt32.self, forKey: .eqProofBytes)
    epProofBytes = try container.decode(UInt32.self, forKey: .epProofBytes)
    completeProofBytes = try container.decode(UInt32.self, forKey: .completeProofBytes)
    proveP95Ms = try container.decode(UInt32.self, forKey: .proveP95Ms)
    verifyP95Ms = try container.decode(UInt32.self, forKey: .verifyP95Ms)
    processRssBytes = try container.decode(UInt64.self, forKey: .processRssBytes)
    guard processRssBytes <= 9_007_199_254_740_991 else {
      throw kagemushaReleaseDecodeError(
        decoder, "process_rss_bytes is outside exact V1 JSON integer range")
    }
    operationEnergyMillijoules = try container.decode(
      UInt64.self, forKey: .operationEnergyMillijoules)
    guard operationEnergyMillijoules <= 9_007_199_254_740_991 else {
      throw kagemushaReleaseDecodeError(
        decoder, "operation_energy_millijoules is outside exact V1 JSON integer range")
    }
    report = try container.decode(ToriiGovernanceKagemushaEvidenceFileV1.self, forKey: .report)
  }
}

/// Exact InternalValidationReceiptV1 projection.
public struct ToriiGovernanceKagemushaInternalValidationReceiptV1: Decodable, Sendable, Equatable {
  public let version: UInt16
  public let sourceTreeDigest: ToriiGovernanceKagemushaBytes32V1
  public let cargoLockDigest: ToriiGovernanceKagemushaBytes32V1
  public let profileDigest: ToriiGovernanceKagemushaBytes32V1
  public let nativeProfileDigest: ToriiGovernanceKagemushaBytes32V1
  public let eqProtocolDigest: ToriiGovernanceKagemushaBytes32V1
  public let epProtocolDigest: ToriiGovernanceKagemushaBytes32V1
  public let artifactSetDigest: ToriiGovernanceKagemushaBytes32V1
  public let hardwarePolicyDigest: ToriiGovernanceKagemushaBytes32V1
  public let providerPolicyRoot: ToriiGovernanceKagemushaBytes32V1
  public let providerPolicy: [ToriiGovernanceKagemushaProviderPolicyEntryV1]
  public let evidenceClosure: ToriiGovernanceKagemushaEvidenceClosureV1
  public let circuitShapeReport: ToriiGovernanceKagemushaEvidenceFileV1
  public let securityReviewReport: ToriiGovernanceKagemushaEvidenceFileV1
  public let katReport: ToriiGovernanceKagemushaEvidenceFileV1
  public let fuzzReport: ToriiGovernanceKagemushaEvidenceFileV1
  public let resourceReport: ToriiGovernanceKagemushaEvidenceFileV1
  public let profileQualifications: [ToriiGovernanceKagemushaProfileQualificationV1]
  public let helperProtocols: [ToriiGovernanceKagemushaHelperProtocolV1]
  public let reproducibleBuilds: [ToriiGovernanceKagemushaReproducibleBuildV1]
  public let fuzzCases: UInt64

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case version = "version"
    case sourceTreeDigest = "source_tree_digest"
    case cargoLockDigest = "cargo_lock_digest"
    case profileDigest = "profile_digest"
    case nativeProfileDigest = "native_profile_digest"
    case eqProtocolDigest = "eq_protocol_digest"
    case epProtocolDigest = "ep_protocol_digest"
    case artifactSetDigest = "artifact_set_digest"
    case hardwarePolicyDigest = "hardware_policy_digest"
    case providerPolicyRoot = "provider_policy_root"
    case providerPolicy = "provider_policy"
    case evidenceClosure = "evidence_closure"
    case circuitShapeReport = "circuit_shape_report"
    case securityReviewReport = "security_review_report"
    case katReport = "kat_report"
    case fuzzReport = "fuzz_report"
    case resourceReport = "resource_report"
    case profileQualifications = "profile_qualifications"
    case helperProtocols = "helper_protocols"
    case reproducibleBuilds = "reproducible_builds"
    case fuzzCases = "fuzz_cases"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaInternalValidationReceiptV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    version = try container.decode(UInt16.self, forKey: .version)
    guard version == 1 else {
      throw kagemushaReleaseDecodeError(decoder, "version must be 1")
    }
    sourceTreeDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .sourceTreeDigest)
    cargoLockDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .cargoLockDigest)
    profileDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .profileDigest)
    nativeProfileDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .nativeProfileDigest)
    eqProtocolDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .eqProtocolDigest)
    epProtocolDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .epProtocolDigest)
    artifactSetDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .artifactSetDigest)
    hardwarePolicyDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .hardwarePolicyDigest)
    providerPolicyRoot = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .providerPolicyRoot)
    providerPolicy = try container.decode(
      [ToriiGovernanceKagemushaProviderPolicyEntryV1].self, forKey: .providerPolicy)
    evidenceClosure = try container.decode(
      ToriiGovernanceKagemushaEvidenceClosureV1.self, forKey: .evidenceClosure)
    circuitShapeReport = try container.decode(
      ToriiGovernanceKagemushaEvidenceFileV1.self, forKey: .circuitShapeReport)
    securityReviewReport = try container.decode(
      ToriiGovernanceKagemushaEvidenceFileV1.self, forKey: .securityReviewReport)
    katReport = try container.decode(
      ToriiGovernanceKagemushaEvidenceFileV1.self, forKey: .katReport)
    fuzzReport = try container.decode(
      ToriiGovernanceKagemushaEvidenceFileV1.self, forKey: .fuzzReport)
    resourceReport = try container.decode(
      ToriiGovernanceKagemushaEvidenceFileV1.self, forKey: .resourceReport)
    profileQualifications = try container.decode(
      [ToriiGovernanceKagemushaProfileQualificationV1].self, forKey: .profileQualifications)
    helperProtocols = try container.decode(
      [ToriiGovernanceKagemushaHelperProtocolV1].self, forKey: .helperProtocols)
    reproducibleBuilds = try container.decode(
      [ToriiGovernanceKagemushaReproducibleBuildV1].self, forKey: .reproducibleBuilds)
    fuzzCases = try container.decode(UInt64.self, forKey: .fuzzCases)
    guard fuzzCases <= 9_007_199_254_740_991 else {
      throw kagemushaReleaseDecodeError(
        decoder, "fuzz_cases is outside exact V1 JSON integer range")
    }
  }
}

/// Exact ProfileQualificationV1 projection.
public struct ToriiGovernanceKagemushaProfileQualificationV1: Decodable, Sendable, Equatable {
  public let profile: ToriiGovernanceKagemushaEnabledProfileV1
  public let relations: [ToriiGovernanceKagemushaRelationQualificationV1]
  public let helperCircuits: [ToriiGovernanceKagemushaHelperQualificationV1]
  public let recursiveDepths: [ToriiGovernanceKagemushaRecursiveDepthQualificationV1]
  public let aggregateBalance: ToriiGovernanceKagemushaAggregateBalanceQualificationV1
  public let thermal: ToriiGovernanceKagemushaThermalQualificationV1
  public let envelope: ToriiGovernanceKagemushaEnvelopeQualificationV1
  public let acceptanceCases: [ToriiGovernanceKagemushaAcceptanceCaseEvidenceV1]

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case profile = "profile"
    case relations = "relations"
    case helperCircuits = "helper_circuits"
    case recursiveDepths = "recursive_depths"
    case aggregateBalance = "aggregate_balance"
    case thermal = "thermal"
    case envelope = "envelope"
    case acceptanceCases = "acceptance_cases"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaProfileQualificationV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    profile = try container.decode(ToriiGovernanceKagemushaEnabledProfileV1.self, forKey: .profile)
    relations = try container.decode(
      [ToriiGovernanceKagemushaRelationQualificationV1].self, forKey: .relations)
    helperCircuits = try container.decode(
      [ToriiGovernanceKagemushaHelperQualificationV1].self, forKey: .helperCircuits)
    recursiveDepths = try container.decode(
      [ToriiGovernanceKagemushaRecursiveDepthQualificationV1].self, forKey: .recursiveDepths)
    aggregateBalance = try container.decode(
      ToriiGovernanceKagemushaAggregateBalanceQualificationV1.self, forKey: .aggregateBalance)
    thermal = try container.decode(
      ToriiGovernanceKagemushaThermalQualificationV1.self, forKey: .thermal)
    envelope = try container.decode(
      ToriiGovernanceKagemushaEnvelopeQualificationV1.self, forKey: .envelope)
    acceptanceCases = try container.decode(
      [ToriiGovernanceKagemushaAcceptanceCaseEvidenceV1].self, forKey: .acceptanceCases)
  }
}

/// Exact ProviderPolicyEntryV1 projection.
public struct ToriiGovernanceKagemushaProviderPolicyEntryV1: Decodable, Sendable, Equatable {
  public let hardwareProfileId: ToriiGovernanceKagemushaBytes32V1
  public let providerAuthorityCommitment: ToriiGovernanceKagemushaBytes32V1
  public let providerProfileIndex: UInt16
  public let issuerSignature: ToriiGovernanceKagemushaDeviceSignatureV1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case hardwareProfileId = "hardware_profile_id"
    case providerAuthorityCommitment = "provider_authority_commitment"
    case providerProfileIndex = "provider_profile_index"
    case issuerSignature = "issuer_signature"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaProviderPolicyEntryV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    hardwareProfileId = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .hardwareProfileId)
    providerAuthorityCommitment = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .providerAuthorityCommitment)
    providerProfileIndex = try container.decode(UInt16.self, forKey: .providerProfileIndex)
    issuerSignature = try container.decode(
      ToriiGovernanceKagemushaDeviceSignatureV1.self, forKey: .issuerSignature)
  }
}

/// Exact QualifiedHelperCircuitV1 projection.
public struct ToriiGovernanceKagemushaQualifiedHelperCircuitV1: Decodable, Sendable, Equatable {
  public let helper: ToriiGovernanceKagemushaQualifiedHelperCircuitTagV1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case helper = "helper"
    case value = "value"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaQualifiedHelperCircuitV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    helper = try container.decode(
      ToriiGovernanceKagemushaQualifiedHelperCircuitTagV1.self, forKey: .helper)
    guard container.contains(.value), try container.decodeNil(forKey: .value) else {
      throw kagemushaReleaseDecodeError(decoder, "value must be explicit null")
    }
  }
}

/// Exact QualifiedRelationV1 projection.
public struct ToriiGovernanceKagemushaQualifiedRelationV1: Decodable, Sendable, Equatable {
  public let relation: ToriiGovernanceKagemushaQualifiedRelationTagV1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case relation = "relation"
    case value = "value"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaQualifiedRelationV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    relation = try container.decode(
      ToriiGovernanceKagemushaQualifiedRelationTagV1.self, forKey: .relation)
    guard container.contains(.value), try container.decodeNil(forKey: .value) else {
      throw kagemushaReleaseDecodeError(decoder, "value must be explicit null")
    }
  }
}

/// Exact RecursiveDepthQualificationV1 projection.
public struct ToriiGovernanceKagemushaRecursiveDepthQualificationV1: Decodable, Sendable, Equatable
{
  public let depth: UInt32
  public let verifiedHandoffs: UInt32
  public let completeProofBytes: UInt32
  public let rawCompleteExchangeBytes: UInt32
  public let textCompleteExchangeBytes: UInt32
  public let report: ToriiGovernanceKagemushaEvidenceFileV1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case depth = "depth"
    case verifiedHandoffs = "verified_handoffs"
    case completeProofBytes = "complete_proof_bytes"
    case rawCompleteExchangeBytes = "raw_complete_exchange_bytes"
    case textCompleteExchangeBytes = "text_complete_exchange_bytes"
    case report = "report"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaRecursiveDepthQualificationV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    depth = try container.decode(UInt32.self, forKey: .depth)
    verifiedHandoffs = try container.decode(UInt32.self, forKey: .verifiedHandoffs)
    completeProofBytes = try container.decode(UInt32.self, forKey: .completeProofBytes)
    rawCompleteExchangeBytes = try container.decode(UInt32.self, forKey: .rawCompleteExchangeBytes)
    textCompleteExchangeBytes = try container.decode(
      UInt32.self, forKey: .textCompleteExchangeBytes)
    report = try container.decode(ToriiGovernanceKagemushaEvidenceFileV1.self, forKey: .report)
  }
}

/// Exact RelationQualificationV1 projection.
public struct ToriiGovernanceKagemushaRelationQualificationV1: Decodable, Sendable, Equatable {
  public let relation: ToriiGovernanceKagemushaQualifiedRelationV1
  public let eqProtocolDigest: ToriiGovernanceKagemushaBytes32V1
  public let epProtocolDigest: ToriiGovernanceKagemushaBytes32V1
  public let eqVerifyingKey: ToriiGovernanceKagemushaArtifactBindingV1
  public let epVerifyingKey: ToriiGovernanceKagemushaArtifactBindingV1
  public let eqCircuitRows: UInt32
  public let epCircuitRows: UInt32
  public let completeProofBytes: UInt32
  public let proveP95Ms: UInt32
  public let verifyP95Ms: UInt32
  public let processRssBytes: UInt64
  public let operationEnergyMillijoules: UInt64
  public let report: ToriiGovernanceKagemushaEvidenceFileV1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case relation = "relation"
    case eqProtocolDigest = "eq_protocol_digest"
    case epProtocolDigest = "ep_protocol_digest"
    case eqVerifyingKey = "eq_verifying_key"
    case epVerifyingKey = "ep_verifying_key"
    case eqCircuitRows = "eq_circuit_rows"
    case epCircuitRows = "ep_circuit_rows"
    case completeProofBytes = "complete_proof_bytes"
    case proveP95Ms = "prove_p95_ms"
    case verifyP95Ms = "verify_p95_ms"
    case processRssBytes = "process_rss_bytes"
    case operationEnergyMillijoules = "operation_energy_millijoules"
    case report = "report"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaRelationQualificationV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    relation = try container.decode(
      ToriiGovernanceKagemushaQualifiedRelationV1.self, forKey: .relation)
    eqProtocolDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .eqProtocolDigest)
    epProtocolDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .epProtocolDigest)
    eqVerifyingKey = try container.decode(
      ToriiGovernanceKagemushaArtifactBindingV1.self, forKey: .eqVerifyingKey)
    epVerifyingKey = try container.decode(
      ToriiGovernanceKagemushaArtifactBindingV1.self, forKey: .epVerifyingKey)
    eqCircuitRows = try container.decode(UInt32.self, forKey: .eqCircuitRows)
    epCircuitRows = try container.decode(UInt32.self, forKey: .epCircuitRows)
    completeProofBytes = try container.decode(UInt32.self, forKey: .completeProofBytes)
    proveP95Ms = try container.decode(UInt32.self, forKey: .proveP95Ms)
    verifyP95Ms = try container.decode(UInt32.self, forKey: .verifyP95Ms)
    processRssBytes = try container.decode(UInt64.self, forKey: .processRssBytes)
    guard processRssBytes <= 9_007_199_254_740_991 else {
      throw kagemushaReleaseDecodeError(
        decoder, "process_rss_bytes is outside exact V1 JSON integer range")
    }
    operationEnergyMillijoules = try container.decode(
      UInt64.self, forKey: .operationEnergyMillijoules)
    guard operationEnergyMillijoules <= 9_007_199_254_740_991 else {
      throw kagemushaReleaseDecodeError(
        decoder, "operation_energy_millijoules is outside exact V1 JSON integer range")
    }
    report = try container.decode(ToriiGovernanceKagemushaEvidenceFileV1.self, forKey: .report)
  }
}

/// Exact ReleaseApprovalV1 projection.
public struct ToriiGovernanceKagemushaReleaseApprovalV1: Decodable, Sendable, Equatable {
  public let publicKey: String
  public let signature: String

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case publicKey = "public_key"
    case signature = "signature"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaReleaseApprovalV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    publicKey = try container.decode(String.self, forKey: .publicKey)
    _ = try governanceKagemushaPublicKeyOrderV1(
      publicKey, codingPath: container.codingPath + [CodingKeys.publicKey])
    signature = try container.decode(String.self, forKey: .signature)
    guard !signature.isEmpty, signature.utf8.count.isMultiple(of: 2),
      signature.utf8.allSatisfy({ (48...57).contains($0) || (65...70).contains($0) })
    else {
      throw kagemushaReleaseDecodeError(
        decoder, "signature must be canonical uppercase signature hex")
    }
  }
}

/// Exact ReleaseAttestationSubjectV1 projection.
public struct ToriiGovernanceKagemushaReleaseAttestationSubjectV1: Decodable, Sendable, Equatable {
  public let version: UInt16
  public let authorityPolicyDigest: ToriiGovernanceKagemushaBytes32V1
  public let releaseId: ToriiGovernanceKagemushaBytes32V1
  public let manifestDigest: ToriiGovernanceKagemushaBytes32V1
  public let validationReceiptDigest: ToriiGovernanceKagemushaBytes32V1
  public let artifactSetDigest: ToriiGovernanceKagemushaBytes32V1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case version = "version"
    case authorityPolicyDigest = "authority_policy_digest"
    case releaseId = "release_id"
    case manifestDigest = "manifest_digest"
    case validationReceiptDigest = "validation_receipt_digest"
    case artifactSetDigest = "artifact_set_digest"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaReleaseAttestationSubjectV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    version = try container.decode(UInt16.self, forKey: .version)
    guard version == 1 else {
      throw kagemushaReleaseDecodeError(decoder, "version must be 1")
    }
    authorityPolicyDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .authorityPolicyDigest)
    releaseId = try container.decode(ToriiGovernanceKagemushaBytes32V1.self, forKey: .releaseId)
    manifestDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .manifestDigest)
    validationReceiptDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .validationReceiptDigest)
    artifactSetDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .artifactSetDigest)
  }
}

/// Exact ReleaseAttestationV1 projection.
public struct ToriiGovernanceKagemushaReleaseAttestationV1: Decodable, Sendable, Equatable {
  public let version: UInt16
  public let subject: ToriiGovernanceKagemushaReleaseAttestationSubjectV1
  public let approvals: [ToriiGovernanceKagemushaReleaseApprovalV1]

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case version = "version"
    case subject = "subject"
    case approvals = "approvals"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaReleaseAttestationV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    version = try container.decode(UInt16.self, forKey: .version)
    guard version == 1 else {
      throw kagemushaReleaseDecodeError(decoder, "version must be 1")
    }
    subject = try container.decode(
      ToriiGovernanceKagemushaReleaseAttestationSubjectV1.self, forKey: .subject)
    approvals = try container.decode(
      [ToriiGovernanceKagemushaReleaseApprovalV1].self, forKey: .approvals)
  }
}

/// Exact asset and reserve scope of a signed testnet experiment.
public struct ToriiGovernanceKagemushaTestnetExperimentScopeV1: Decodable, Sendable, Equatable {
  public let assetIdentityDigest: ToriiGovernanceKagemushaBytes32V1
  public let assetIncarnation: ToriiGovernanceKagemushaBytes32V1
  public let assetScale: UInt32
  public let liabilityPoolId: ToriiGovernanceKagemushaBytes32V1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case assetIdentityDigest = "asset_identity_digest"
    case assetIncarnation = "asset_incarnation"
    case assetScale = "asset_scale"
    case liabilityPoolId = "liability_pool_id"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaTestnetExperimentScopeV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    assetIdentityDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .assetIdentityDigest)
    assetIncarnation = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .assetIncarnation)
    assetScale = try container.decode(UInt32.self, forKey: .assetScale)
    guard assetScale <= 28 else {
      throw kagemushaReleaseDecodeError(decoder, "asset_scale must be at most 28")
    }
    liabilityPoolId = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .liabilityPoolId)
  }
}

/// Signed production or explicitly scoped testnet purpose of a verifier release.
public enum ToriiGovernanceKagemushaReleasePurposeV1: Decodable, Sendable, Equatable {
  case production
  case testnetExperiment(ToriiGovernanceKagemushaTestnetExperimentScopeV1)

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case kind
    case value
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaReleasePurposeV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    switch try container.decode(String.self, forKey: .kind) {
    case "production":
      guard try container.decodeNil(forKey: .value) else {
        throw kagemushaReleaseDecodeError(decoder, "production purpose value must be null")
      }
      self = .production
    case "testnet_experiment":
      self = .testnetExperiment(try container.decode(
        ToriiGovernanceKagemushaTestnetExperimentScopeV1.self, forKey: .value))
    default:
      throw kagemushaReleaseDecodeError(decoder, "unknown release purpose")
    }
  }
}

/// Exact ReleaseManifestV1 projection.
public struct ToriiGovernanceKagemushaReleaseManifestV1: Decodable, Sendable, Equatable {
  public let version: UInt16
  public let networkId: NetworkId
  public let purpose: ToriiGovernanceKagemushaReleasePurposeV1
  public let releaseId: ToriiGovernanceKagemushaBytes32V1
  public let sourceTreeDigest: ToriiGovernanceKagemushaBytes32V1
  public let cargoLockDigest: ToriiGovernanceKagemushaBytes32V1
  public let profileDigest: ToriiGovernanceKagemushaBytes32V1
  public let eqProtocolDigest: ToriiGovernanceKagemushaBytes32V1
  public let epProtocolDigest: ToriiGovernanceKagemushaBytes32V1
  public let hardwarePolicyDigest: ToriiGovernanceKagemushaBytes32V1
  public let validationReceiptDigest: ToriiGovernanceKagemushaBytes32V1
  public let halo2K: UInt32
  public let helperProtocols: [ToriiGovernanceKagemushaHelperProtocolV1]
  public let enabledProfiles: [ToriiGovernanceKagemushaEnabledProfileV1]
  public let artifacts: [ToriiGovernanceKagemushaArtifactBindingV1]

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case version = "version"
    case networkId = "network_id"
    case purpose
    case releaseId = "release_id"
    case sourceTreeDigest = "source_tree_digest"
    case cargoLockDigest = "cargo_lock_digest"
    case profileDigest = "profile_digest"
    case eqProtocolDigest = "eq_protocol_digest"
    case epProtocolDigest = "ep_protocol_digest"
    case hardwarePolicyDigest = "hardware_policy_digest"
    case validationReceiptDigest = "validation_receipt_digest"
    case halo2K = "halo2_k"
    case helperProtocols = "helper_protocols"
    case enabledProfiles = "enabled_profiles"
    case artifacts = "artifacts"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaReleaseManifestV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    version = try container.decode(UInt16.self, forKey: .version)
    guard version == 1 else {
      throw kagemushaReleaseDecodeError(decoder, "version must be 1")
    }
    networkId = try container.decode(NetworkId.self, forKey: .networkId)
    purpose = try container.decode(ToriiGovernanceKagemushaReleasePurposeV1.self, forKey: .purpose)
    releaseId = try container.decode(ToriiGovernanceKagemushaBytes32V1.self, forKey: .releaseId)
    sourceTreeDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .sourceTreeDigest)
    cargoLockDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .cargoLockDigest)
    profileDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .profileDigest)
    eqProtocolDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .eqProtocolDigest)
    epProtocolDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .epProtocolDigest)
    hardwarePolicyDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .hardwarePolicyDigest)
    validationReceiptDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .validationReceiptDigest)
    halo2K = try container.decode(UInt32.self, forKey: .halo2K)
    helperProtocols = try container.decode(
      [ToriiGovernanceKagemushaHelperProtocolV1].self, forKey: .helperProtocols)
    enabledProfiles = try container.decode(
      [ToriiGovernanceKagemushaEnabledProfileV1].self, forKey: .enabledProfiles)
    artifacts = try container.decode(
      [ToriiGovernanceKagemushaArtifactBindingV1].self, forKey: .artifacts)
  }
}

/// Exact ReproducibleBuildV1 projection.
public struct ToriiGovernanceKagemushaReproducibleBuildV1: Decodable, Sendable, Equatable {
  public let builderId: ToriiGovernanceKagemushaBytes32V1
  public let artifactSetDigest: ToriiGovernanceKagemushaBytes32V1
  public let report: ToriiGovernanceKagemushaEvidenceFileV1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case builderId = "builder_id"
    case artifactSetDigest = "artifact_set_digest"
    case report = "report"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaReproducibleBuildV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    builderId = try container.decode(ToriiGovernanceKagemushaBytes32V1.self, forKey: .builderId)
    artifactSetDigest = try container.decode(
      ToriiGovernanceKagemushaBytes32V1.self, forKey: .artifactSetDigest)
    report = try container.decode(ToriiGovernanceKagemushaEvidenceFileV1.self, forKey: .report)
  }
}

/// Exact ThermalQualificationV1 projection.
public struct ToriiGovernanceKagemushaThermalQualificationV1: Decodable, Sendable, Equatable {
  public let foldedCredits: UInt32
  public let foldP95Ms: UInt32
  public let processRssBytes: UInt64
  public let operationEnergyMillijoules: UInt64
  public let report: ToriiGovernanceKagemushaEvidenceFileV1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case foldedCredits = "folded_credits"
    case foldP95Ms = "fold_p95_ms"
    case processRssBytes = "process_rss_bytes"
    case operationEnergyMillijoules = "operation_energy_millijoules"
    case report = "report"
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "GovernanceKagemushaThermalQualificationV1"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    foldedCredits = try container.decode(UInt32.self, forKey: .foldedCredits)
    foldP95Ms = try container.decode(UInt32.self, forKey: .foldP95Ms)
    processRssBytes = try container.decode(UInt64.self, forKey: .processRssBytes)
    guard processRssBytes <= 9_007_199_254_740_991 else {
      throw kagemushaReleaseDecodeError(
        decoder, "process_rss_bytes is outside exact V1 JSON integer range")
    }
    operationEnergyMillijoules = try container.decode(
      UInt64.self, forKey: .operationEnergyMillijoules)
    guard operationEnergyMillijoules <= 9_007_199_254_740_991 else {
      throw kagemushaReleaseDecodeError(
        decoder, "operation_energy_millijoules is outside exact V1 JSON integer range")
    }
    report = try container.decode(ToriiGovernanceKagemushaEvidenceFileV1.self, forKey: .report)
  }
}

/// Typed projection of the complete governed standby verifier-release proposal.
public struct ToriiGovernanceKagemushaVerifierReleaseInstallProposalV1:
  Decodable, Sendable, Equatable
{
  public let proposalOperator: String
  public let networkId: NetworkId
  public let expectedPredecessor: ToriiGovernanceKagemushaGovernedVerifierRegistryV1
  public let manifest: ToriiGovernanceKagemushaReleaseManifestV1
  public let receipt: ToriiGovernanceKagemushaInternalValidationReceiptV1
  public let attestation: ToriiGovernanceKagemushaReleaseAttestationV1

  private enum CodingKeys: String, CodingKey, CaseIterable {
    case proposalOperator = "proposal_operator"
    case networkId = "network_id"
    case expectedPredecessor = "expected_predecessor"
    case manifest
    case receipt
    case attestation
  }

  public init(from decoder: Decoder) throws {
    try governanceRejectUnknownFields(
      decoder, allowed: Set(CodingKeys.allCases.map(\.stringValue)),
      name: "KagemushaVerifierReleaseInstall"
    )
    let container = try decoder.container(keyedBy: CodingKeys.self)
    proposalOperator = try governanceCanonicalAccount(
      container.decode(String.self, forKey: .proposalOperator),
      codingPath: container.codingPath + [CodingKeys.proposalOperator],
      field: "proposal_operator"
    )
    networkId = try container.decode(NetworkId.self, forKey: .networkId)
    expectedPredecessor = try container.decode(
      ToriiGovernanceKagemushaGovernedVerifierRegistryV1.self, forKey: .expectedPredecessor
    )
    guard expectedPredecessor.authorityPolicy != nil else {
      throw kagemushaReleaseDecodeError(
        decoder, "release install requires a governed signer policy")
    }
    manifest = try container.decode(
      ToriiGovernanceKagemushaReleaseManifestV1.self, forKey: .manifest)
    receipt = try container.decode(
      ToriiGovernanceKagemushaInternalValidationReceiptV1.self, forKey: .receipt)
    attestation = try container.decode(
      ToriiGovernanceKagemushaReleaseAttestationV1.self, forKey: .attestation)
  }
}
