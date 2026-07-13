#![allow(unexpected_cfgs)]

//! Norito-encoded manifest model for SoraFS artifacts.
//!
//! The structure tracks the metadata described in the SoraFS Architecture
//! RFC (SF-1): chunking profile, CAR commitments, pin policies, and governance
//! attestations. Encoding uses Norito so manifests can be validated by Torii,
//! gateways, and storage nodes without bespoke parsers.

use blake3::Hash;
use ed25519_dalek::Signature as DalekSig;
use norito::{
    core::Error as NoritoError,
    derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize},
    json::{self, FastJsonWrite, JsonSerialize as NoritoJsonSerialize},
};
use sorafs_chunker::ChunkProfile;
use thiserror::Error;

pub mod alias_cache;
pub mod capacity;
pub mod chunker_registry;
pub mod deal;
pub mod gar;
pub mod gateway;
pub mod gateway_fixture;
pub mod governance;
pub mod hedging;
pub mod hosts;
pub mod hybrid_envelope;
pub mod manifest_capabilities;
pub mod orderbook;
pub mod pdp;
pub mod pin_registry;
pub mod pop_credentials;
pub mod por;
pub mod potr;
pub mod pricing;
pub mod proof_stream;
pub mod provider_admission;
pub mod provider_advert;
pub mod reconciliation;
pub mod reference;
pub mod reference_ffi;
pub mod repair;
pub mod reputation;
pub mod retention;
pub mod token;
pub mod transparency;
pub mod validation;

/// Decode a fixed-width Ed25519 signature after rejecting inert or malformed `R` payloads.
pub(crate) fn checked_ed25519_signature_from_bytes(
    signature: &[u8; ed25519_dalek::SIGNATURE_LENGTH],
) -> Result<DalekSig, String> {
    if inert_bytes(signature) {
        return Err("signature payload must not be all zero".to_owned());
    }
    let r_bytes: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = signature
        [..ed25519_dalek::PUBLIC_KEY_LENGTH]
        .try_into()
        .map_err(|_| "signature R bytes have invalid length".to_owned())?;
    if !ed25519_compressed_y_is_canonical(&r_bytes) {
        return Err("signature R is not a canonical Ed25519 point".to_owned());
    }
    iroha_crypto::ed25519_parse_signature(signature)
        .map_err(|err| format!("signature R is small-order (weak); rejected: {err}"))?;
    Ok(DalekSig::from_bytes(signature))
}

/// Decode a fixed-width Ed25519 verifying key after rejecting inert or weak keys.
pub(crate) fn checked_ed25519_verifying_key_from_bytes(
    public_key: &[u8; ed25519_dalek::PUBLIC_KEY_LENGTH],
) -> Result<ed25519_dalek::VerifyingKey, String> {
    if inert_bytes(public_key) {
        return Err("public key material must not be all zero".to_owned());
    }
    if !ed25519_compressed_y_is_canonical(public_key) {
        return Err("public key is not a canonical Ed25519 point".to_owned());
    }
    let verifying_key =
        ed25519_dalek::VerifyingKey::from_bytes(public_key).map_err(|err| err.to_string())?;
    if verifying_key.is_weak() {
        return Err("public key is small-order (weak); rejected".to_owned());
    }
    Ok(verifying_key)
}

pub(crate) fn inert_bytes(bytes: &[u8]) -> bool {
    !bytes.is_empty() && bytes.iter().all(|byte| *byte == 0)
}

fn ed25519_compressed_y_is_canonical(bytes: &[u8; ed25519_dalek::PUBLIC_KEY_LENGTH]) -> bool {
    const ED25519_FIELD_MODULUS_LE: [u8; ed25519_dalek::PUBLIC_KEY_LENGTH] = [
        0xed, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

    let mut y = *bytes;
    y[ed25519_dalek::PUBLIC_KEY_LENGTH - 1] &= 0x7f;
    for idx in (0..ed25519_dalek::PUBLIC_KEY_LENGTH).rev() {
        match y[idx].cmp(&ED25519_FIELD_MODULUS_LE[idx]) {
            std::cmp::Ordering::Less => return true,
            std::cmp::Ordering::Greater => return false,
            std::cmp::Ordering::Equal => {}
        }
    }
    false
}

#[cfg(test)]
mod checked_ed25519_signature_tests {
    use ed25519_dalek::{PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH, SigningKey};

    const ED25519_SMALL_ORDER_POINT: [u8; PUBLIC_KEY_LENGTH] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];

    const ED25519_NON_CANONICAL_IDENTITY: [u8; PUBLIC_KEY_LENGTH] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

    const ED25519_NON_CANONICAL_NON_SMALL_ORDER_POINT: [u8; PUBLIC_KEY_LENGTH] = [
        0xf0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

    #[test]
    fn checked_ed25519_signature_rejects_all_zero_signature_material() {
        let err = super::checked_ed25519_signature_from_bytes(&[0; SIGNATURE_LENGTH])
            .expect_err("all-zero signature material must fail");
        assert!(err.contains("all zero"), "unexpected error: {err}");
    }

    #[test]
    fn checked_ed25519_signature_rejects_noncanonical_or_small_order_r() {
        use ed25519_dalek::Signer as _;

        let signing_key = SigningKey::from_bytes(&[0x42; 32]);
        let mut signature = signing_key.sign(b"sorafs-manifest-invalid-r").to_bytes();

        signature[..PUBLIC_KEY_LENGTH].copy_from_slice(&ED25519_SMALL_ORDER_POINT);
        let err = super::checked_ed25519_signature_from_bytes(&signature)
            .expect_err("small-order signature R must fail");
        assert!(err.contains("small-order"), "unexpected error: {err}");

        signature[..PUBLIC_KEY_LENGTH].copy_from_slice(&ED25519_NON_CANONICAL_IDENTITY);
        let err = super::checked_ed25519_signature_from_bytes(&signature)
            .expect_err("noncanonical signature R must fail");
        assert!(err.contains("not a canonical"), "unexpected error: {err}");
    }

    #[test]
    fn checked_ed25519_verifying_key_rejects_all_zero_public_key_material() {
        let err = super::checked_ed25519_verifying_key_from_bytes(&[0; PUBLIC_KEY_LENGTH])
            .expect_err("all-zero public key material must fail");
        assert!(err.contains("all zero"));

        let err = super::checked_ed25519_verifying_key_from_bytes(&ED25519_SMALL_ORDER_POINT)
            .expect_err("small-order public key material must fail");
        assert!(err.contains("small-order"), "unexpected error: {err}");

        let err = super::checked_ed25519_verifying_key_from_bytes(&ED25519_NON_CANONICAL_IDENTITY)
            .expect_err("noncanonical public key material must fail");
        assert!(err.contains("not a canonical"), "unexpected error: {err}");

        let err = super::checked_ed25519_verifying_key_from_bytes(
            &ED25519_NON_CANONICAL_NON_SMALL_ORDER_POINT,
        )
        .expect_err("noncanonical non-small-order public key material must fail");
        assert!(err.contains("not a canonical"), "unexpected error: {err}");

        let signing_key = SigningKey::from_bytes(&[0x41; 32]);
        let public_key = signing_key.verifying_key().to_bytes();
        super::checked_ed25519_verifying_key_from_bytes(&public_key)
            .expect("valid Ed25519 public key must pass");
    }

    #[test]
    fn inert_bytes_requires_non_empty_all_zero_material() {
        assert!(!super::inert_bytes(&[]));
        assert!(super::inert_bytes(&[0, 0, 0]));
        assert!(!super::inert_bytes(&[0, 1, 0]));
    }
}

pub use capacity::{
    AssignmentError, CAPACITY_DECLARATION_VERSION_V1, CAPACITY_DISPUTE_VERSION_V1,
    CAPACITY_TELEMETRY_VERSION_V1, CapacityDeclarationV1, CapacityDeclarationValidationError,
    CapacityDisputeEvidenceError, CapacityDisputeEvidenceV1, CapacityDisputeKind,
    CapacityDisputeV1, CapacityDisputeValidationError, CapacityMetadataEntry, CapacityTelemetryV1,
    CapacityTelemetryValidationError, ChunkerCommitmentError, ChunkerCommitmentV1,
    LaneCommitmentError, LaneCommitmentV1, MetadataError, PricingScheduleError, PricingScheduleV1,
    REPLICATION_ORDER_SIGNATURE_DOMAIN_V1, REPLICATION_ORDER_VERSION_V1, ReplicationAssignmentV1,
    ReplicationOrderSignatureV1, ReplicationOrderSignatureVerificationError, ReplicationOrderSlaV1,
    ReplicationOrderV1, ReplicationOrderValidationError, SIGNED_REPLICATION_ORDER_VERSION_V1,
    SignedReplicationOrderV1, SignedReplicationOrderValidationError, SlaError,
};
pub use chunker_registry::{ChunkerProfileDescriptor, DEFAULT_MULTIHASH_CODE, MANIFEST_DAG_CODEC};
pub use deal::{
    BASIS_POINTS_PER_UNIT, DEAL_LEDGER_VERSION_V1, DEAL_MICROPAYMENT_VERSION_V1,
    DEAL_SETTLEMENT_VERSION_V1, DEAL_TERMS_VERSION_V1, DealAmountError, DealLedgerSnapshotV1,
    DealLedgerTransitionError, DealLedgerValidationError, DealMetadataEntry, DealMicropaymentV1,
    DealMicropaymentValidationError, DealSettlementStatusV1, DealSettlementTransitionError,
    DealSettlementV1, DealSettlementValidationError, DealTermsV1, DealTermsValidationError,
    MAX_DEAL_CLIENT_ACCOUNT_BYTES, MAX_DEAL_METADATA_ENTRIES, MAX_DEAL_METADATA_KEY_BYTES,
    MAX_DEAL_METADATA_VALUE_BYTES, MAX_DEAL_PROFILE_HANDLE_BYTES,
    MAX_DEAL_SETTLEMENT_AUDIT_NOTES_BYTES, MICRO_XOR_PER_XOR, MicropaymentPolicyError,
    MicropaymentPolicyV1, XOR_QUANTITY_SCALE, XorQuantity, derive_micropayment_hint,
};
pub use gateway::{
    GatewayAuthorizationError, GatewayAuthorizationRecord, GatewayAuthorizationVerifier,
    HostPattern,
};
pub use governance::{
    GOVERNANCE_DAG_BLOCK_VERSION_V1, GOVERNANCE_DAG_HEAD_VERSION_V1,
    GOVERNANCE_EXTERNAL_KIND_GC_AUDIT_V1, GOVERNANCE_EXTERNAL_KIND_PROOF_TOKEN_ISSUANCE_V1,
    GOVERNANCE_EXTERNAL_KIND_RECONCILIATION_V1, GOVERNANCE_EXTERNAL_KIND_REPAIR_AUDIT_V1,
    GOVERNANCE_EXTERNAL_KIND_REPAIR_SLASH_V1,
    GOVERNANCE_EXTERNAL_KIND_TRANSPARENCY_LEDGER_PUBLICATION_V1, GOVERNANCE_LOG_VERSION_V1,
    GovernanceDagBlockV1, GovernanceDagBlockValidationError, GovernanceDagChainValidationError,
    GovernanceDagHeadChainValidationError, GovernanceDagHeadV1, GovernanceDagHeadValidationError,
    GovernanceExternalPayloadMetadataV1, GovernanceExternalPayloadV1,
    GovernanceExternalPayloadValidationError, GovernanceExternalRepairSlashStageV1,
    GovernanceLogNodeV1, GovernanceLogPayloadV1, GovernanceLogSignatureV1,
    GovernanceLogSignatureVerificationError, GovernanceLogValidationError,
    GovernanceSignatureAlgorithm, SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
    SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1,
    SORAFS_APPEAL_FINANCE_WEEKLY_ROLLUP_VERSION_V1,
    SORAFS_GOVERNANCE_EXTERNAL_METADATA_KEY_MAX_BYTES_V1,
    SORAFS_GOVERNANCE_EXTERNAL_METADATA_MAX_ENTRIES_V1,
    SORAFS_GOVERNANCE_EXTERNAL_METADATA_TOTAL_MAX_BYTES_V1,
    SORAFS_GOVERNANCE_EXTERNAL_METADATA_VALUE_MAX_BYTES_V1,
    SORAFS_GOVERNANCE_EXTERNAL_PAYLOAD_MAX_BYTES_V1, SORAFS_GOVERNANCE_EXTERNAL_PAYLOAD_VERSION_V1,
    SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1, SoraFsAppealFinanceAccountFlowV1,
    SoraFsAppealFinanceJurorPayoutV1, SoraFsAppealFinanceOutcomeRollupV1,
    SoraFsAppealFinanceOutcomeV1, SoraFsAppealFinanceReportV1,
    SoraFsAppealFinanceReportValidationError, SoraFsAppealFinanceSettlementReceiptV1,
    SoraFsAppealFinanceSettlementReceiptValidationError, SoraFsAppealFinanceWeeklyRollupBuildError,
    SoraFsAppealFinanceWeeklyRollupV1, SoraFsAppealFinanceWeeklyRollupValidationError,
    SoraFsModerationBallotGovernanceChallengeDecisionV1,
    SoraFsModerationBallotGovernanceChallengeKindV1, SoraFsModerationBallotGovernanceChallengeV1,
    SoraFsModerationBallotGovernanceEventKindV1, SoraFsModerationBallotGovernanceEventV1,
    SoraFsModerationBallotGovernanceEventValidationError, SoraFsModerationBallotGovernanceTallyV1,
    SoraFsModerationVoteChoiceV1, SoraFsModerationVoteCountsV1, governance_dag_block_cid_v1,
    governance_log_node_cid_v1, validate_governance_dag_chain_v1,
    validate_governance_dag_head_against_chain_v1,
};
pub use hedging::signed::{
    GOVERNED_BILLING_STATEMENT_VERSION_V1, GOVERNED_HEDGING_REFERENCE_PRICE_VERSION_V1,
    GovernedBillingStatementV1, GovernedHedgingReferencePriceDecisionV1,
    HEDGING_FEED_BINDING_VERSION_V1, HEDGING_FEED_TRUST_POLICY_VERSION_V1,
    HEDGING_TRUSTED_SIGNER_VERSION_V1, HedgingFeedBindingV1, HedgingFeedTrustPolicyV1,
    HedgingTrustedSignerV1, MAX_GOVERNED_BILLING_STATEMENT_BYTES,
    MAX_GOVERNED_HEDGING_DECISION_BYTES, MAX_HEDGING_FEED_BINDINGS_PER_SIGNER,
    MAX_HEDGING_FUTURE_SKEW_SECS, MAX_HEDGING_SAMPLE_AGE_SECS, MAX_HEDGING_SIGNER_ID_BYTES,
    MAX_HEDGING_TRUST_POLICY_BYTES, MAX_HEDGING_TRUSTED_SIGNERS, MAX_SIGNED_HEDGING_FEED_BYTES,
    SIGNED_HEDGING_PRICE_FEED_VERSION_V1, SignedHedgingError, SignedHedgingPriceFeedV1,
    bind_governed_billing_statement_v1, decode_governed_billing_statement,
    decode_governed_reference_price_decision, decode_hedging_feed_trust_policy,
    decode_signed_hedging_price_feed, derive_governed_reference_price_decision_v1,
};
pub use hedging::{
    BILLING_LINE_ITEM_VERSION_V1, BILLING_STATEMENT_MAX_CANONICAL_BYTES_V1,
    BILLING_STATEMENT_VERSION_V1, BillingLineDirectionV1, BillingLineItemKindV1, BillingLineItemV1,
    BillingStatementV1, HEDGING_BASIS_POINTS, HEDGING_DECISION_MAX_CANONICAL_BYTES_V1,
    HEDGING_PRICE_FEED_VERSION_V1, HEDGING_REFERENCE_PRICE_DECISION_VERSION_V1,
    HEDGING_SMALL_PAYLOAD_MAX_CANONICAL_BYTES_V1, HedgingFeedStatusV1, HedgingPayloadDecodeError,
    HedgingPriceFeedV1, HedgingReferencePriceDecisionV1, HedgingValidationError,
    MAX_BILLING_ACCOUNT_ID_BYTES, MAX_BILLING_LINES, MAX_HEDGING_DEGRADATION_REASONS,
    MAX_HEDGING_IDENTIFIER_BYTES, MAX_HEDGING_NOTE_BYTES, MAX_HEDGING_PRICE_FEEDS,
    billing_line_item_id_v1, billing_statement_id_v1, build_billing_line_item_v1,
    build_billing_statement_v1, decode_billing_line_item_v1, decode_billing_statement_v1,
    decode_hedging_price_feed_v1, decode_hedging_reference_price_decision_v1,
    derive_reference_price_decision_v1, reference_price_decision_id_v1,
    validate_billing_statement_transition, xor_to_usd,
};
pub use hosts::{DirectCarLocator, HostMappingInput, HostMappingSummary};
pub use manifest_capabilities::{
    ChunkProfileSummary, ManifestCapabilitySummary, detect_manifest_capabilities,
};
pub use orderbook::{
    BYTES_PER_GIB, ByteRangeV1, ORDERBOOK_CANCEL_VERSION_V1, ORDERBOOK_ORDER_ID_DOMAIN_V1,
    ORDERBOOK_ORDER_VERSION_V1, ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1,
    ORDERBOOK_PAYLOAD_MAX_CANONICAL_BYTES_V1, ORDERBOOK_RUNTIME_SNAPSHOT_MAX_CANONICAL_BYTES_V1,
    ORDERBOOK_RUNTIME_SNAPSHOT_MAX_ENTRIES_V1, ORDERBOOK_RUNTIME_SNAPSHOT_VERSION_V1,
    ORDERBOOK_SETTLEMENT_CHANNEL_ID_DOMAIN_V1, ORDERBOOK_TRADE_EVENT_VERSION_V1, OrderBookEntryV1,
    OrderBookMatchOutcomeV1, OrderCancelReasonV1, OrderCancelV1, OrderFillOutcomeV1,
    OrderRequestV1, OrderSideV1, OrderTierV1, OrderbookOwnerNonceHighWaterV1,
    OrderbookPayloadDecodeError, OrderbookRuntimeSnapshotV1, OrderbookSignatureV1,
    OrderbookValidationError, SETTLEMENT_CHANNEL_VERSION_V1, SETTLEMENT_RECEIPT_VERSION_V1,
    SettlementChannelStatusV1, SettlementChannelV1, SettlementReceiptV1, TradeEventV1,
    apply_settlement_receipt_v1, decode_order_cancel_v1, decode_order_request_v1,
    decode_orderbook_runtime_snapshot_v1, decode_settlement_channel_v1,
    decode_settlement_receipt_v1, decode_trade_event_v1, derive_orderbook_order_id_v1,
    derive_orderbook_settlement_channel_id_v1, derive_orderbook_trade_id_v1, match_order_book_v1,
    match_orders_v1, open_settlement_channel_for_trade_v1, order_cancel_signature_digest_v1,
    order_request_signature_digest_v1, settlement_receipt_signature_digest_v1,
    sign_order_cancel_ed25519_v1, sign_order_request_ed25519_v1,
    sign_settlement_receipt_ed25519_v1, trade_escrow_requirement_v1, trade_gross_value_v1,
    verify_order_cancel_signature_v1, verify_order_request_signature_v1,
    verify_settlement_receipt_signature_v1,
};
pub use pdp::{
    HashAlgorithmV1, PDP_CHALLENGE_MAX_CANONICAL_BYTES_V1, PDP_CHALLENGE_VERSION_V1,
    PDP_COMMITMENT_MAX_CANONICAL_BYTES_V1, PDP_COMMITMENT_VERSION_V1,
    PDP_GOVERNANCE_ARCHIVE_MAX_CANONICAL_BYTES_V1, PDP_GOVERNANCE_ARCHIVE_VERSION_V1,
    PDP_HOT_LEAF_SIZE_V1, PDP_HOT_LEAVES_PER_SEGMENT_V1, PDP_MAX_HOT_LEAVES_PER_SEGMENT_SAMPLE_V1,
    PDP_MAX_MERKLE_PATH_DEPTH_V1, PDP_MAX_SEGMENT_SAMPLES_V1, PDP_MAX_TOTAL_HOT_LEAF_SAMPLES_V1,
    PDP_PROOF_MAX_CANONICAL_BYTES_V1, PDP_PROOF_SIGNATURE_DOMAIN_V1, PDP_PROOF_VERSION_V1,
    PDP_SEGMENT_SIZE_V1, PdpChallengeV1, PdpChallengeValidationError,
    PdpChunkProfileValidationError, PdpCommitmentV1, PdpCommitmentValidationError,
    PdpEd25519SignatureV1, PdpGovernanceArchiveV1, PdpGovernanceArchiveValidationError,
    PdpHotLeafProofV1, PdpMerklePathError, PdpMerkleReadError, PdpMerkleTreeBuilderV1,
    PdpMerkleTreeError, PdpMerkleTreeV1, PdpProofLeafV1, PdpProofSigningError, PdpProofV1,
    PdpProofValidationError, PdpRejectionReasonV1, PdpSampleV1, PdpSignatureVerificationError,
    PdpTerminalDecisionV1, PdpVerificationError, VerifiedPdpProofV1, estimated_heap_bytes,
    sign_pdp_proof_ed25519_v1, verify_pdp_bundle_v1, verify_pdp_witnesses_v1,
};
pub use pin_registry::{
    AliasBindingV1, AliasBindingValidationError, ManifestPolicyV1, ManifestPolicyValidationError,
    PinRecordV1, PinRecordValidationError, ReplicationOrderV1 as PinRegistryReplicationOrderV1,
    ReplicationOrderValidationError as PinRegistryReplicationOrderValidationError,
    ReplicationReceiptStatus, ReplicationReceiptV1, ReplicationReceiptValidationError,
};
pub use pop_credentials::{
    POP_COMMITMENT_ROOT_VERSION_V1, POP_CREDENTIAL_VERSION_V1, POP_ENROLLMENT_REQUEST_VERSION_V1,
    POP_ISSUED_CREDENTIAL_BUNDLE_VERSION_V1, POP_MEMBERSHIP_PROOF_VERSION_V1,
    POP_RENEWAL_REQUEST_VERSION_V1, POP_REVOCATION_LIST_VERSION_V1, PopCommitmentRootV1,
    PopCredentialAttributeV1, PopCredentialV1, PopCredentialValidationError, PopEligibilityClassV1,
    PopEnrollmentRequestV1, PopIssuedCredentialBundleV1, PopMembershipProofSystemV1,
    PopMembershipProofV1, PopRenewalRequestV1, PopRevocationEntryV1, PopRevocationListV1,
    PopRevocationReasonV1, PopSignatureAlgorithmV1, PopSignatureV1,
    issue_pop_credential_bundle_ed25519_v1, pop_commitment_root_signature_digest_v1,
    pop_credential_signature_digest_v1, pop_revocation_list_signature_digest_v1,
    sign_pop_commitment_root_ed25519_v1, sign_pop_credential_ed25519_v1,
    sign_pop_revocation_list_ed25519_v1, verify_pop_commitment_root_signature_v1,
    verify_pop_credential_signature_v1, verify_pop_membership_proof_v1,
    verify_pop_revocation_list_signature_v1,
};
pub use por::{
    AUDIT_VERDICT_VERSION_V1, AuditOutcomeV1, AuditVerdictV1, AuditVerdictValidationError,
    MANUAL_POR_CHALLENGE_VERSION_V1, ManualPorChallengeV1, ManualPorChallengeValidationError,
    POR_CHALLENGE_STATUS_VERSION_V1, POR_CHALLENGE_VERSION_V1, POR_PROOF_SIGNATURE_DOMAIN_V1,
    POR_PROOF_VERSION_V1, POR_VERDICT_SIGNATURE_DOMAIN_V1, POR_WEEKLY_REPORT_VERSION_V1,
    PorChallengeOutcome, PorChallengeOutcomeParseError, PorChallengeStatusV1,
    PorChallengeStatusValidationError, PorChallengeV1, PorChallengeValidationError,
    PorProofSampleV1, PorProofV1, PorProofValidationError, PorProviderSummaryV1,
    PorProviderSummaryValidationError, PorReportIsoWeek, PorReportIsoWeekValidationError,
    PorSignatureVerificationError, PorSlashingEventV1, PorSlashingEventValidationError,
    PorWeeklyReportV1, PorWeeklyReportValidationError,
};
pub use potr::{
    POTR_RECEIPT_VERSION_V1, PotrReceiptV1, PotrReceiptValidationError, PotrSignatureAlgorithm,
    PotrSignatureV1, PotrStatus,
};
pub use pricing::signed::{
    GOVERNED_PRICING_MANIFEST_VERSION_V1, GovernedPricingError, GovernedPricingManifestV1,
    MAX_GOVERNED_PRICING_MANIFEST_BYTES, MAX_PRICING_FUTURE_ACTIVATION_SECS,
    MAX_PRICING_MANIFEST_SIGNATURES, MAX_PRICING_SIGNER_ID_BYTES, MAX_PRICING_TRUST_POLICY_BYTES,
    MAX_PRICING_TRUSTED_SIGNERS, PRICING_MANIFEST_SIGNATURE_VERSION_V1,
    PRICING_TRUST_POLICY_VERSION_V1, PRICING_TRUSTED_SIGNER_VERSION_V1, PricingManifestSignatureV1,
    PricingTrustPolicyV1, PricingTrustedSignerV1, decode_governed_pricing_manifest,
    decode_pricing_trust_policy, derive_pricing_id, validate_governed_pricing_transition,
};
pub use pricing::{
    BondPolicyError, BondPolicyV1, CreditPolicyError, CreditPolicyV1, MAX_PRICING_NONCE_SAMPLES,
    MAX_PRICING_NOTES_LEN, MAX_PRICING_TIER_ID_LEN, MAX_PRICING_TIERS, MicropaymentDecision,
    PRICING_MANIFEST_VERSION_V1, PricingCalculationError, PricingManifestError, PricingManifestV1,
    PricingMicropaymentEvaluationError, PricingMicropaymentPolicyError,
    PricingMicropaymentPolicyV1, PricingNonceJsonError, PricingTierError, PricingTierV1,
};
pub use proof_stream::{
    MAX_PROOF_STREAM_SAMPLE_COUNT, ProofStreamKind, ProofStreamRequestError, ProofStreamRequestV1,
    ProofStreamTier,
};
pub use provider_admission::{
    AdmissionRecord, ENDPOINT_ATTESTATION_VERSION_V1, EndpointAdmissionError, EndpointAdmissionV1,
    EndpointAttestationError, EndpointAttestationKind, EndpointAttestationV1,
    PROVIDER_ADMISSION_ENVELOPE_VERSION_V1, PROVIDER_ADMISSION_PROPOSAL_VERSION_V1,
    PROVIDER_ADMISSION_RENEWAL_VERSION_V1, PROVIDER_ADMISSION_REVOCATION_VERSION_V1,
    ProviderAdmissionAdvertError, ProviderAdmissionCouncilPolicy,
    ProviderAdmissionCouncilPolicyError, ProviderAdmissionEnvelopeError,
    ProviderAdmissionEnvelopeV1, ProviderAdmissionProposalV1, ProviderAdmissionRenewalError,
    ProviderAdmissionRenewalV1, ProviderAdmissionRevocationError, ProviderAdmissionRevocationV1,
    ProviderAdmissionSignatureError, ProviderAdmissionValidationError, ProviderVrfPublicKeyV1,
    compute_advert_body_digest, compute_envelope_authorization_digest, compute_envelope_digest,
    compute_proposal_digest, verify_advert_against_record, verify_envelope,
    verify_envelope_untrusted_signers, verify_revocation_signatures,
    verify_revocation_signatures_untrusted_signers,
};
pub use provider_advert::{
    AdvertEndpoint, AdvertSignature, AdvertSignatureError, AdvertValidationError, AvailabilityTier,
    CapabilityTlv, CapabilityType, EndpointKind, EndpointMetadata, EndpointMetadataKey,
    MAX_ADVERT_TTL_SECS, PROVIDER_ADVERT_SIGNATURE_DOMAIN_V1, PROVIDER_ADVERT_VERSION_V1,
    PathDiversityPolicy, ProviderAdvertBodyV1, ProviderAdvertBuildError, ProviderAdvertBuilder,
    ProviderAdvertSignaturePayloadV1, ProviderAdvertV1, ProviderCapabilityRangeV1, QosHints,
    REFRESH_RECOMMENDATION_SECS, RangeCapabilityError, RendezvousTopic, SignatureAlgorithm,
    StakePointer, StreamBudgetError, StreamBudgetV1, TransportHintError, TransportHintV1,
    TransportProtocol,
};
pub use reconciliation::{
    AppealFinanceReconciliationSummaryV1, ReconciliationValidationError,
    SORAFS_RECONCILIATION_REPORT_VERSION_V1, SorafsReconciliationReportV1,
};
pub use reference::{
    FixtureBundlePayloadKindV1, FixtureBundlePayloadV1, HedgingValidationPayloadKindV1,
    OrderbookOrderCancelFieldsV1, OrderbookOrderRequestFieldsV1, OrderbookPayloadSigningError,
    OrderbookSettlementReceiptFieldsV1, OrderbookValidationPayloadKindV1,
    PopValidationPayloadKindV1, REFERENCE_SDK_ERRORS_DOC_URL, RepairValidationPayloadKindV1,
    VALIDATION_OUTCOME_VERSION_V1, ValidationContextFieldV1, ValidationInputV1,
    ValidationOutcomeV1, build_signed_orderbook_order_cancel_bytes_ed25519_v1,
    build_signed_orderbook_order_request_bytes_ed25519_v1,
    build_signed_orderbook_settlement_receipt_bytes_ed25519_v1,
    sign_orderbook_payload_bytes_ed25519_v1, validate_fixture_bundle_payloads,
    validate_governance_dag_block_bytes, validate_governance_dag_head_chain_bytes,
    validate_governance_log_node_bytes, validate_hedging_payload_bytes,
    validate_orderbook_payload_bytes, validate_pdp_challenge_bytes,
    validate_pdp_challenge_proof_bytes, validate_pdp_commitment_bytes,
    validate_pdp_commitment_challenge_bytes, validate_pdp_commitment_challenge_proof_bytes,
    validate_pdp_proof_bytes, validate_pop_payload_bytes, validate_por_challenge_proof_bytes,
    validate_potr_receipt_bytes, validate_provider_admission_envelope_bytes,
    validate_provider_admission_renewal_bytes, validate_provider_admission_revocation_bytes,
    validate_provider_advert_bytes, validate_repair_payload_bytes,
    validate_replication_order_bytes, validate_signed_replication_order_bytes,
};
pub use repair::{
    AuditorSignatureV1, AuditorSignatureVerificationError, GC_AUDIT_EVENT_VERSION_V1,
    GC_AUDIT_PAYLOAD_VERSION_V1, GcAuditEventV1, GcAuditPayloadV1,
    REPAIR_ESCALATION_APPROVAL_VERSION_V1, REPAIR_ESCALATION_POLICY_VERSION_V1,
    REPAIR_EVIDENCE_VERSION_V1, REPAIR_REPORT_VERSION_V1, REPAIR_SLASH_PROPOSAL_VERSION_V1,
    REPAIR_TASK_EVENT_VERSION_V1, REPAIR_TASK_VERSION_V1, REPAIR_WORKER_SIGNATURE_VERSION_V1,
    RepairAuditEventV1, RepairCauseV1, RepairEscalationApprovalV1, RepairEscalationPolicyV1,
    RepairEvidenceV1, RepairLatencySlaCauseV1, RepairManualCauseV1, RepairPdpFailureCauseV1,
    RepairPdpFailureKindV1, RepairPorFailureCauseV1, RepairReplicaShortfallCauseV1, RepairReportV1,
    RepairSlashProposalV1, RepairTaskEventV1, RepairTaskRecordV1, RepairTaskStateV1,
    RepairTaskStatusV1, RepairTicketId, RepairValidationError, RepairWorkerActionV1,
    RepairWorkerSignaturePayloadV1, SIGNED_AUDITOR_REQUEST_VERSION_V1,
    SignedAuditorRequestPayloadV1, SignedAuditorRequestSignaturePayloadV1, SignedAuditorRequestV1,
};
pub use reputation::signed::{
    MAX_REPUTATION_FUTURE_SKEW_SECS, MAX_REPUTATION_SIGNER_ID_LEN,
    MAX_REPUTATION_SNAPSHOT_AGE_SECS, MAX_REPUTATION_SNAPSHOT_SIGNATURES,
    MAX_REPUTATION_TRUSTED_SIGNERS, REPUTATION_SCORING_EVIDENCE_VERSION_V1,
    REPUTATION_SNAPSHOT_TRUST_POLICY_VERSION_V1, REPUTATION_TRUSTED_SIGNER_VERSION_V1,
    ReputationScoringEvidenceV1, ReputationSnapshotSignatureV1, ReputationSnapshotTrustPolicyV1,
    ReputationTrustedSignerV1, SIGNED_REPUTATION_SNAPSHOT_VERSION_V1,
    SignedReputationSnapshotError, SignedReputationSnapshotV1, snapshot_signing_digest,
    validate_reputation_snapshot_transition,
};
pub use reputation::{
    DEFAULT_CURRENT_SCORE_WEIGHT_BPS, DEFAULT_EIGENTRUST_ALPHA_BPS, LOW_REPUTATION_SCORE_FLAG_BPS,
    MAX_REPUTATION_MERKLE_PROOF_LEN, MAX_REPUTATION_PROVIDERS, MAX_REPUTATION_SCORE_BPS,
    MAX_REPUTATION_TRUST_EDGES, MIN_REPUTATION_SCORE_BPS, PROVIDER_REPUTATION_VERSION_V1,
    ProviderReputationV1, REPUTATION_BASIS_POINTS, REPUTATION_EIGENTRUST_CONVERGENCE_L1_BPS,
    REPUTATION_EIGENTRUST_MAX_ITERATIONS, REPUTATION_PROVIDER_INPUT_VERSION_V1,
    REPUTATION_PROVIDER_METRICS_VERSION_V1, REPUTATION_SNAPSHOT_EVENT_VERSION_V1,
    REPUTATION_SNAPSHOT_VERSION_V1, REPUTATION_TRUST_EDGE_VERSION_V1,
    REPUTATION_WEIGHTS_VERSION_V1, ReputationDegradationFlagV1, ReputationMerkleProofV1,
    ReputationProviderInputV1, ReputationProviderMetricsV1, ReputationReserveStageV1,
    ReputationSnapshotEventV1, ReputationSnapshotV1, ReputationTrustEdgeV1,
    ReputationValidationError, ReputationWeightsV1, build_reputation_snapshot,
    build_reputation_snapshot_with_trust_edges, compute_reputation_merkle_root,
    score_provider_reputation,
};
pub use token::{
    STREAM_TOKEN_MAX_BASE64_BYTES_V1, STREAM_TOKEN_MAX_TTL_SECS_V1, STREAM_TOKEN_MAX_WIRE_BYTES_V1,
    StreamTokenBodyV1, StreamTokenError, StreamTokenV1,
};
pub use transparency::{
    MODERATION_LEDGER_BLOCK_VERSION_V1, MODERATION_LEDGER_ENTRY_VERSION_V1,
    MODERATION_LEDGER_MAX_ENTRIES_V1, MODERATION_LEDGER_MAX_EVIDENCE_URI_BYTES_V1,
    MODERATION_LEDGER_MAX_EVIDENCE_URIS_V1, MODERATION_LEDGER_MAX_METADATA_ENTRIES_V1,
    MODERATION_LEDGER_MAX_METADATA_KEY_BYTES_V1, MODERATION_LEDGER_MAX_METADATA_TOTAL_BYTES_V1,
    MODERATION_LEDGER_MAX_METADATA_VALUE_BYTES_V1, MODERATION_LEDGER_MAX_PROOF_PATH_LEN,
    MODERATION_LEDGER_MAX_PUBLIC_TEXT_BYTES_V1, MODERATION_LEDGER_PROOF_VERSION_V1,
    MODERATION_LEDGER_PUBLICATION_VERSION_V1, MODERATION_PRIVACY_AGGREGATE_VERSION_V1,
    MODERATION_PRIVACY_DELTA_PPB_MAX, MODERATION_PRIVACY_MAX_METRICS_V1,
    MODERATION_PRIVACY_PARAMETERS_VERSION_V1, ModerationLedgerBlockV1,
    ModerationLedgerCyclePublicationV1, ModerationLedgerEntryKindV1, ModerationLedgerEntryV1,
    ModerationLedgerMetadataV1, ModerationLedgerProofNodeV1, ModerationLedgerProofSideV1,
    ModerationLedgerProofV1, ModerationPrivacyAggregateMetricV1, ModerationPrivacyAggregateV1,
    ModerationPrivacyModeV1, ModerationPrivacyParametersV1, PROOF_TOKEN_ISSUANCE_VERSION_V1,
    PROOF_TOKEN_MAX_ENTRY_ID_BYTES_V1, PROOF_TOKEN_MAX_ENTRY_IDS_V1, ProofTokenIssuanceV1,
    TransparencyLedgerError,
};
pub use validation::{
    MAX_MANIFEST_ALIAS_CLAIMS, MAX_MANIFEST_ALIAS_PROOF_BYTES, MAX_MANIFEST_COUNCIL_SIGNATURES,
    MAX_MANIFEST_ENCODED_BYTES, MAX_MANIFEST_METADATA_BYTES, MAX_MANIFEST_METADATA_ENTRIES,
    MAX_MANIFEST_ROOT_CID_BYTES, ManifestDecodeError, ManifestValidationError,
    PinPolicyConstraints, decode_manifest_v1_canonical, validate_chunker_handle, validate_manifest,
    validate_manifest_root_cid, validate_pin_policy, validate_registered_chunker_profile,
};

pub use self::gateway_fixture::{
    GatewayFixtureMetadata, SORAFS_GATEWAY_CAR_DIGEST_HEX,
    SORAFS_GATEWAY_CHUNK_DIGEST_SHA3_256_HEX, SORAFS_GATEWAY_COUNCIL_ENVELOPE_DIGEST_HEX,
    SORAFS_GATEWAY_FIXTURE_DIGEST_HEX, SORAFS_GATEWAY_FIXTURE_RELEASE_UNIX,
    SORAFS_GATEWAY_FIXTURE_VERSION, SORAFS_GATEWAY_MANIFEST_DIGEST_HEX,
    SORAFS_GATEWAY_PAYLOAD_DIGEST_HEX, SORAFS_GATEWAY_PROFILE_VERSION, gateway_fixture_digest_hex,
    gateway_fixture_metadata,
};

/// Manifest version identifier.
pub const MANIFEST_VERSION_V1: u8 = 1;

/// Multihash code for BLAKE3-256.
pub const BLAKE3_256_MULTIHASH_CODE: u64 = 0x1f;

/// Builds the canonical binary CIDv1 used for first-release manifest roots.
///
/// The returned bytes encode CID version 1, the dag-cbor multicodec, the
/// BLAKE3-256 multihash code, a 32-byte digest length, and `digest`.
#[must_use]
pub fn canonical_manifest_root_cid(digest: [u8; 32]) -> Vec<u8> {
    let mut cid = Vec::with_capacity(MAX_MANIFEST_ROOT_CID_BYTES);
    // These are the complete one-byte canonical varints for the immutable
    // first-release CID layout. Keeping them literal makes this constructor
    // infallible; the layout test below binds them to the public u64 codes.
    cid.extend_from_slice(&[1, 0x71, 0x1f, 32]);
    cid.extend_from_slice(&digest);
    cid
}

/// Norito-encoded manifest (version 1).
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct ManifestV1 {
    pub version: u8,
    pub root_cid: Vec<u8>,
    pub dag_codec: DagCodecId,
    pub chunking: ChunkingProfileV1,
    /// SHA3-256 commitment to the ordered chunk metadata plan.
    pub chunk_digest_sha3_256: [u8; 32],
    pub content_length: u64,
    pub car_digest: [u8; 32],
    pub car_size: u64,
    pub pin_policy: PinPolicy,
    pub governance: GovernanceProofs,
    pub alias_claims: Vec<AliasClaim>,
    pub metadata: Vec<MetadataEntry>,
}

impl ManifestV1 {
    /// Serializes the manifest using canonical Norito encoding.
    pub fn encode(&self) -> Result<Vec<u8>, NoritoError> {
        norito::to_bytes(self)
    }

    /// Computes the canonical manifest digest used by the Pin Registry.
    pub fn digest(&self) -> Result<Hash, NoritoError> {
        let bytes = self.encode()?;
        Ok(blake3::hash(&bytes))
    }
}

/// Errors raised while building a manifest.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ManifestBuildError {
    #[error("missing required field: {0}")]
    MissingField(&'static str),
}

/// Builder for [`ManifestV1`].
#[derive(Debug, Default)]
pub struct ManifestBuilder {
    root_cid: Option<Vec<u8>>,
    dag_codec: Option<DagCodecId>,
    chunking: Option<ChunkingProfileV1>,
    chunk_digest_sha3_256: Option<[u8; 32]>,
    content_length: Option<u64>,
    car_digest: Option<[u8; 32]>,
    car_size: Option<u64>,
    pin_policy: Option<PinPolicy>,
    governance: GovernanceProofs,
    alias_claims: Vec<AliasClaim>,
    metadata: Vec<MetadataEntry>,
}

impl ManifestBuilder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn root_cid(mut self, cid: impl Into<Vec<u8>>) -> Self {
        self.root_cid = Some(cid.into());
        self
    }

    #[must_use]
    pub fn dag_codec(mut self, codec: DagCodecId) -> Self {
        self.dag_codec = Some(codec);
        self
    }

    #[must_use]
    pub fn chunking_profile(mut self, profile: ChunkingProfileV1) -> Self {
        self.chunking = Some(profile);
        self
    }

    #[must_use]
    pub fn chunking_from_registry(mut self, profile_id: ProfileId) -> Self {
        if let Some(descriptor) = crate::chunker_registry::lookup(profile_id) {
            self.chunking = Some(ChunkingProfileV1::from_descriptor(descriptor));
        }
        self
    }

    #[must_use]
    pub fn chunking_from_profile(mut self, profile: ChunkProfile, multihash_code: u64) -> Self {
        self.chunking = Some(ChunkingProfileV1::from_profile(profile, multihash_code));
        self
    }

    /// Set the SHA3-256 commitment to the ordered chunk metadata plan.
    #[must_use]
    pub fn chunk_digest_sha3_256(mut self, digest: [u8; 32]) -> Self {
        self.chunk_digest_sha3_256 = Some(digest);
        self
    }

    #[must_use]
    pub fn content_length(mut self, len: u64) -> Self {
        self.content_length = Some(len);
        self
    }

    #[must_use]
    pub fn car_digest(mut self, digest: [u8; 32]) -> Self {
        self.car_digest = Some(digest);
        self
    }

    #[must_use]
    pub fn car_size(mut self, size: u64) -> Self {
        self.car_size = Some(size);
        self
    }

    #[must_use]
    pub fn pin_policy(mut self, policy: PinPolicy) -> Self {
        self.pin_policy = Some(policy);
        self
    }

    #[must_use]
    pub fn governance(mut self, proofs: GovernanceProofs) -> Self {
        self.governance = proofs;
        self
    }

    #[must_use]
    pub fn push_alias(mut self, claim: AliasClaim) -> Self {
        self.alias_claims.push(claim);
        self
    }

    #[must_use]
    pub fn extend_aliases<I>(mut self, claims: I) -> Self
    where
        I: IntoIterator<Item = AliasClaim>,
    {
        self.alias_claims.extend(claims);
        self
    }

    #[must_use]
    pub fn add_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.push(MetadataEntry {
            key: key.into(),
            value: value.into(),
        });
        self
    }

    #[must_use]
    pub fn extend_metadata<I>(mut self, entries: I) -> Self
    where
        I: IntoIterator<Item = (String, String)>,
    {
        self.metadata.extend(
            entries
                .into_iter()
                .map(|(key, value)| MetadataEntry { key, value }),
        );
        self
    }

    pub fn build(self) -> Result<ManifestV1, ManifestBuildError> {
        let root_cid = self
            .root_cid
            .ok_or(ManifestBuildError::MissingField("root_cid"))?;
        let dag_codec = self
            .dag_codec
            .ok_or(ManifestBuildError::MissingField("dag_codec"))?;
        let chunking = self
            .chunking
            .ok_or(ManifestBuildError::MissingField("chunking"))?;
        let chunk_digest_sha3_256 = self
            .chunk_digest_sha3_256
            .ok_or(ManifestBuildError::MissingField("chunk_digest_sha3_256"))?;
        let content_length = self
            .content_length
            .ok_or(ManifestBuildError::MissingField("content_length"))?;
        let car_digest = self
            .car_digest
            .ok_or(ManifestBuildError::MissingField("car_digest"))?;
        let car_size = self
            .car_size
            .ok_or(ManifestBuildError::MissingField("car_size"))?;
        let pin_policy = self
            .pin_policy
            .ok_or(ManifestBuildError::MissingField("pin_policy"))?;

        Ok(ManifestV1 {
            version: MANIFEST_VERSION_V1,
            root_cid,
            dag_codec,
            chunking,
            chunk_digest_sha3_256,
            content_length,
            car_digest,
            car_size,
            pin_policy,
            governance: self.governance,
            alias_claims: self.alias_claims,
            metadata: self.metadata,
        })
    }
}

/// Simple newtype for Dag codec identifiers (CID multicodec).
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct DagCodecId(pub u64);

/// Snapshot of the chunking profile baked into the manifest.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct ChunkingProfileV1 {
    pub profile_id: ProfileId,
    pub namespace: String,
    pub name: String,
    pub semver: String,
    pub min_size: u32,
    pub target_size: u32,
    pub max_size: u32,
    pub break_mask: u32,
    pub multihash_code: u64,
    pub aliases: Vec<String>,
}

impl ChunkingProfileV1 {
    pub fn from_profile(profile: ChunkProfile, multihash_code: u64) -> Self {
        if let Some(descriptor) =
            crate::chunker_registry::lookup_by_profile(profile, multihash_code)
        {
            Self::from_descriptor(descriptor)
        } else {
            Self {
                profile_id: ProfileId(0),
                namespace: "inline".to_owned(),
                name: "inline".to_owned(),
                semver: "0.0.0".to_owned(),
                min_size: profile.min_size as u32,
                target_size: profile.target_size as u32,
                max_size: profile.max_size as u32,
                break_mask: profile.break_mask as u32,
                multihash_code,
                aliases: vec!["inline.inline@0.0.0".to_owned()],
            }
        }
    }

    pub fn from_descriptor(descriptor: &crate::chunker_registry::ChunkerProfileDescriptor) -> Self {
        Self {
            profile_id: descriptor.id,
            namespace: descriptor.namespace.to_owned(),
            name: descriptor.name.to_owned(),
            semver: descriptor.semver.to_owned(),
            min_size: descriptor.profile.min_size as u32,
            target_size: descriptor.profile.target_size as u32,
            max_size: descriptor.profile.max_size as u32,
            break_mask: descriptor.profile.break_mask as u32,
            multihash_code: descriptor.multihash_code,
            aliases: descriptor
                .aliases
                .iter()
                .map(|alias| alias.to_string())
                .collect(),
        }
    }
}

/// Profile identifier used for chunking negotiation.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
pub struct ProfileId(pub u32);

/// Storage replication policy encoded in the manifest.
#[derive(
    Debug,
    Clone,
    Copy,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct PinPolicy {
    pub min_replicas: u16,
    pub storage_class: StorageClass,
    pub retention_epoch: u64,
}

impl Default for PinPolicy {
    fn default() -> Self {
        Self {
            min_replicas: 1,
            storage_class: StorageClass::default(),
            retention_epoch: 0,
        }
    }
}

/// Storage tier expressed in the manifest.
#[derive(
    Debug, Clone, Copy, NoritoSerialize, NoritoDeserialize, PartialEq, Eq, PartialOrd, Ord, Default,
)]
pub enum StorageClass {
    #[default]
    Hot,
    Warm,
    Cold,
}

impl FastJsonWrite for StorageClass {
    fn write_json(&self, out: &mut String) {
        let label = match self {
            StorageClass::Hot => "hot",
            StorageClass::Warm => "warm",
            StorageClass::Cold => "cold",
        };
        NoritoJsonSerialize::json_serialize(&label, out);
    }
}

impl json::JsonDeserialize for StorageClass {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "hot" => Ok(StorageClass::Hot),
            "warm" => Ok(StorageClass::Warm),
            "cold" => Ok(StorageClass::Cold),
            other => Err(json::Error::Message(format!(
                "unknown storage class `{other}`"
            ))),
        }
    }
}

/// Governance proof bundle.
///
/// Future policy proofs (e.g., admission allowlists, replication attestations)
/// will be threaded through this container once the registry schema lands.
#[derive(
    Debug,
    Clone,
    Default,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    PartialEq,
    Eq,
)]
pub struct GovernanceProofs {
    pub council_signatures: Vec<CouncilSignature>,
}

/// Council signature proof binding the manifest digest.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct CouncilSignature {
    pub signer: [u8; 32],
    pub signature: Vec<u8>,
}

/// Alias binding bundled with the manifest.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct AliasClaim {
    pub name: String,
    pub namespace: String,
    pub proof: Vec<u8>,
}

/// Metadata key/value pair recorded in the manifest.
#[derive(
    Debug, Clone, NoritoSerialize, NoritoDeserialize, JsonSerialize, JsonDeserialize, PartialEq, Eq,
)]
pub struct MetadataEntry {
    pub key: String,
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_manifest_root_cid_has_exact_first_release_layout() {
        let digest = [0xA5; 32];
        let cid = canonical_manifest_root_cid(digest);
        assert_eq!(cid.len(), MAX_MANIFEST_ROOT_CID_BYTES);
        assert_eq!(
            &cid[..4],
            &[
                1,
                chunker_registry::MANIFEST_DAG_CODEC as u8,
                BLAKE3_256_MULTIHASH_CODE as u8,
                32,
            ]
        );
        assert_eq!(&cid[4..], digest);
    }

    fn sample_manifest() -> ManifestV1 {
        ManifestBuilder::new()
            .root_cid(canonical_manifest_root_cid([0xAA; 32]))
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(ChunkProfile::DEFAULT, BLAKE3_256_MULTIHASH_CODE)
            .chunk_digest_sha3_256([0xAC; 32])
            .content_length(1_048_576)
            .car_digest([0xAB; 32])
            .car_size(1_100_000)
            .pin_policy(PinPolicy {
                min_replicas: 3,
                storage_class: StorageClass::Hot,
                retention_epoch: 42,
            })
            .governance(GovernanceProofs {
                council_signatures: vec![CouncilSignature {
                    signer: [0x11; 32],
                    signature: vec![0x22; 64],
                }],
            })
            .push_alias(AliasClaim {
                name: "docs".into(),
                namespace: "sora".into(),
                proof: vec![0xde, 0xad, 0xbe, 0xef],
            })
            .add_metadata("build", "ci-123")
            .add_metadata("commit", "abc123")
            .build()
            .expect("build manifest")
    }

    #[test]
    fn encode_roundtrip() {
        let manifest = sample_manifest();
        let bytes = manifest.encode().expect("encode manifest");
        let decoded: ManifestV1 = norito::decode_from_bytes(&bytes).expect("decode manifest");
        assert_eq!(manifest, decoded);
    }

    #[test]
    fn digest_is_deterministic() {
        let manifest = sample_manifest();
        let digest_a = manifest.digest().expect("digest");
        let digest_b = manifest.digest().expect("digest again");
        assert_eq!(digest_a.as_bytes(), digest_b.as_bytes());
        assert_eq!(manifest.chunking.namespace, "sorafs");
        assert_eq!(manifest.chunking.name, "sf1");
        assert_eq!(manifest.chunking.semver, "1.0.0");
    }

    #[test]
    fn digest_binds_the_embedded_chunk_plan_commitment() {
        let manifest = sample_manifest();
        let mut substituted = manifest.clone();
        substituted.chunk_digest_sha3_256[0] ^= 1;
        assert_ne!(
            manifest.digest().expect("original digest"),
            substituted.digest().expect("substituted digest")
        );
    }

    #[test]
    fn builder_rejects_missing_fields() {
        let err = ManifestBuilder::new().build().unwrap_err();
        assert!(matches!(err, ManifestBuildError::MissingField("root_cid")));

        let err = ManifestBuilder::new()
            .root_cid(canonical_manifest_root_cid([0xAA; 32]))
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(ChunkProfile::DEFAULT, BLAKE3_256_MULTIHASH_CODE)
            .content_length(1)
            .car_digest([0xAB; 32])
            .car_size(2)
            .pin_policy(PinPolicy {
                min_replicas: 1,
                storage_class: StorageClass::Hot,
                retention_epoch: 1,
            })
            .build()
            .expect_err("chunk-plan commitment is mandatory");
        assert_eq!(
            err,
            ManifestBuildError::MissingField("chunk_digest_sha3_256")
        );
    }

    #[test]
    fn chunking_profile_includes_registry_aliases() {
        let manifest = sample_manifest();
        let descriptor = crate::chunker_registry::lookup(manifest.chunking.profile_id)
            .expect("descriptor for registered profile");
        let expected: Vec<String> = descriptor
            .aliases
            .iter()
            .map(|alias| alias.to_string())
            .collect();
        assert_eq!(manifest.chunking.aliases, expected);
    }

    #[test]
    fn chunking_profile_fallback_has_inline_alias() {
        let custom_profile = ChunkProfile {
            min_size: 128,
            target_size: 256,
            max_size: 512,
            break_mask: 0xff,
        };
        let chunking = ChunkingProfileV1::from_profile(custom_profile, 0xdead_beef);
        assert_eq!(chunking.aliases, vec!["inline.inline@0.0.0".to_owned()]);
    }
}
