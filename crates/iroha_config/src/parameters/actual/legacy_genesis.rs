//! Retired configured terms of a signed genesis that predates genesis-authored policy.
//!
//! Such a genesis carries no signed crypto or staking policy. Its execution-policy and
//! Nexus/AMX commitments were taken over these configured values, peers bound the same values
//! in the Nexus consensus-policy digest, and every published block repeats the execution-policy
//! identity. A node of that network reproduces the original V1 preimages from this profile; the
//! user view (`user::legacy_genesis`) owns its parsing and validation.
use std::{num::NonZeroU32, time::Duration};

use iroha_crypto::Algorithm;
use iroha_data_model::{
    block::consensus::{PublicLaneStakingPolicy, RootFeePolicy},
    crypto_admission_policy::CryptoAdmissionPolicyV1,
    nexus::{NexusAmxContextWriterV1, tag},
};
use iroha_primitives::numeric::Quantity;
use norito::codec::Encode;

use super::{
    ExecutionPolicyFieldsV1, FeeSponsorProgramId, Nexus, NexusConsensusAutoscaleV1,
    NexusConsensusAxtV1, NexusConsensusDaV1, NexusConsensusDataspaceV1, NexusConsensusDurationV1,
    NexusConsensusEndorsementV1, NexusConsensusFusionV1, NexusConsensusGovernanceV1,
    NexusConsensusHfSharedLeasesV1, NexusConsensusPolicyPreimageV1, NexusConsensusRoutingV1,
    NexusConsensusUploadedModelsV1, execution_policy_canonical_set,
};

/// The configured terms of a genesis signed before genesis authored its policies.
#[derive(Debug, Clone)]
pub struct LegacyGenesisV1 {
    /// The `[crypto]` admission terms.
    pub crypto: LegacyGenesisCryptoV1,
    /// The `[nexus.staking]` monetary terms.
    pub staking: LegacyGenesisStakingV1,
}

/// Retired `[crypto]` terms, as the original execution-policy recipe read them.
#[derive(Debug, Clone)]
pub struct LegacyGenesisCryptoV1 {
    /// Configured `crypto.default_hash`.
    pub default_hash: String,
    /// Configured signing algorithms, sorted and deduplicated.
    pub allowed_signing: Vec<Algorithm>,
    /// Configured `crypto.sm2_distid_default`.
    pub sm2_distid_default: String,
    /// Curve identifiers of `allowed_signing`, sorted and deduplicated.
    pub allowed_curve_ids: Vec<u8>,
    /// The ordinary admission policy these terms select.
    pub policy: CryptoAdmissionPolicyV1,
}

/// Retired `[nexus.staking]` monetary terms, in the form the original digests bound.
#[derive(Debug, Clone)]
pub struct LegacyGenesisStakingV1 {
    /// Minimum bonded stake required to register or bond as a validator.
    pub min_validator_stake: Quantity,
    /// Maximum number of stake-share rows retained for one validator.
    pub max_stake_shares_per_validator: NonZeroU32,
    /// Maximum number of pending unbond requests retained in one stake share.
    pub max_pending_unbonds_per_share: NonZeroU32,
    /// Minimum delay between scheduling and finalising an unbond.
    pub unbonding_delay: Duration,
    /// Maximum slash ratio (basis points).
    pub max_slash_bps: u16,
    /// Configured stake asset literal, trimmed.
    pub stake_asset_id: String,
    /// Configured stake escrow account literal.
    pub stake_escrow_account_id: String,
    /// Configured slash sink account literal.
    pub slash_sink_account_id: String,
    /// The committed-State staking policy these terms select.
    pub policy: PublicLaneStakingPolicy,
}

// Original V1 Nexus/AMX tags of the configured staking and fee terms.
const STAKING_MIN_VALIDATOR_STAKE: &str = "nexus.staking.min_validator_stake";
const STAKING_MAX_STAKE_SHARES_PER_VALIDATOR: &str = "nexus.staking.max_stake_shares_per_validator";
const STAKING_MAX_PENDING_UNBONDS_PER_SHARE: &str = "nexus.staking.max_pending_unbonds_per_share";
const STAKING_UNBONDING_DELAY_NS: &str = "nexus.staking.unbonding_delay_ns";
const STAKING_MAX_SLASH_BPS: &str = "nexus.staking.max_slash_bps";
const STAKING_STAKE_ASSET_ID: &str = "nexus.staking.stake_asset_id";
const STAKING_STAKE_ESCROW_ACCOUNT_ID: &str = "nexus.staking.stake_escrow_account_id";
const STAKING_SLASH_SINK_ACCOUNT_ID: &str = "nexus.staking.slash_sink_account_id";
const FEES_ASSET: &str = "nexus.fees.asset";
const FEES_SINK: &str = "nexus.fees.sink";
const FEES_BASE: &str = "nexus.fees.base";
const FEES_PER_BYTE: &str = "nexus.fees.per_byte";
const FEES_PER_INSTRUCTION: &str = "nexus.fees.per_instruction";
const FEES_PER_GAS_UNIT: &str = "nexus.fees.per_gas_unit";
const FEES_SPONSOR_VAULT_CUSTODY_ACCOUNT_ID: &str = "nexus.fees.sponsor_vault_custody_account_id";
const FEES_SUCCESSFUL_CLAIM_EXEMPT_AUTHORITIES: &str =
    "nexus.fees.successful_claim_exempt_authorities";

// Atomic private settlement stayed disabled with its retired configured defaults on every
// pre-policy network; this node has no configuration that could enable it.
const ATOMIC_PRIVATE_SETTLEMENT_ENABLED: bool = false;
const ATOMIC_PRIVATE_SETTLEMENT_MINIMUM_ACTIVATION_NOTICE_BLOCKS: u64 = 7_200;

/// Write the original staking and fee fields in place of `max_validators` and the root fee
/// policy. `fees` is the committed root policy the original recipe copied into Nexus.
pub(super) fn write_amx_staking_and_fees(
    preimage: &mut NexusAmxContextWriterV1,
    nexus: &Nexus,
    legacy: &LegacyGenesisV1,
    fees: &RootFeePolicy,
) {
    let staking = &legacy.staking;
    preimage.field(STAKING_MIN_VALIDATOR_STAKE, &staking.min_validator_stake);
    preimage.field(
        tag::STAKING_MAX_VALIDATORS,
        &nexus.staking.max_validators.get(),
    );
    preimage.field(
        STAKING_MAX_STAKE_SHARES_PER_VALIDATOR,
        &staking.max_stake_shares_per_validator.get(),
    );
    preimage.field(
        STAKING_MAX_PENDING_UNBONDS_PER_SHARE,
        &staking.max_pending_unbonds_per_share.get(),
    );
    preimage.field(
        STAKING_UNBONDING_DELAY_NS,
        &staking.unbonding_delay.as_nanos(),
    );
    preimage.field(STAKING_MAX_SLASH_BPS, &staking.max_slash_bps);
    preimage.field(STAKING_STAKE_ASSET_ID, &staking.stake_asset_id);
    preimage.field(
        STAKING_STAKE_ESCROW_ACCOUNT_ID,
        &staking.stake_escrow_account_id,
    );
    preimage.field(
        STAKING_SLASH_SINK_ACCOUNT_ID,
        &staking.slash_sink_account_id,
    );
    preimage.field(FEES_ASSET, &fees.fee_asset_id);
    preimage.field(FEES_SINK, &fees.fee_sink_account_id);
    preimage.field(FEES_BASE, &fees.base_fee);
    preimage.field(FEES_PER_BYTE, &fees.per_byte_fee);
    preimage.field(FEES_PER_INSTRUCTION, &fees.per_instruction_fee);
    preimage.field(FEES_PER_GAS_UNIT, &fees.per_gas_unit_fee);
    preimage.field(
        FEES_SPONSOR_VAULT_CUSTODY_ACCOUNT_ID,
        &fees.sponsor_vault_custody_account_id,
    );
    let exempt = fees
        .successful_claim_fee_exempt_authorities
        .iter()
        .map(|authority| {
            authority
                .canonical_i105()
                .expect("validated root fee-exempt authority must encode as canonical I105")
        })
        .collect::<Vec<_>>();
    preimage.field(FEES_SUCCESSFUL_CLAIM_EXEMPT_AUTHORITIES, &exempt);
}

/// Bind the configured crypto terms at their original position, after the pipeline fields.
pub(super) fn push_execution_crypto(
    policy: &mut ExecutionPolicyFieldsV1,
    crypto: &LegacyGenesisCryptoV1,
) {
    policy.push("crypto.default_hash", &crypto.default_hash);
    policy.push(
        "crypto.allowed_signing",
        &execution_policy_canonical_set(crypto.allowed_signing.iter()),
    );
    policy.push("crypto.sm2_distid_default", &crypto.sm2_distid_default);
    policy.push("crypto.allowed_curve_ids", &crypto.allowed_curve_ids);
}

#[derive(Encode)]
struct NexusConsensusStakingLegacyV1 {
    public_validator_mode: u8,
    restricted_validator_mode: u8,
    min_validator_stake: Quantity,
    max_validators: u32,
    max_stake_shares_per_validator: u32,
    max_pending_unbonds_per_share: u32,
    unbonding_delay: NexusConsensusDurationV1,
    max_slash_bps: u16,
    stake_asset_id: String,
    stake_escrow_account_id: String,
    slash_sink_account_id: String,
}

#[derive(Encode)]
struct NexusConsensusAtomicPrivateSettlementLegacyV1 {
    enabled: bool,
    activation_height: Option<u64>,
    minimum_activation_notice_blocks: u64,
    proof_profile_version: u16,
    max_participants: u16,
    max_expiry_blocks: u64,
    audit_timeout_blocks: u64,
    prepare_timeout_blocks: u64,
    commit_timeout_blocks: u64,
    capsule_padding_classes_bytes: Vec<u32>,
    max_proof_bytes: u64,
    max_capsule_bytes: u64,
    max_carrier_bytes: u64,
    sidecar_retention_blocks: u64,
    sidecar_max_records: u32,
    sidecar_max_total_bytes: u64,
    default_min_auditor_approvals: u16,
    permitted_policy_versions: Vec<u16>,
}

#[derive(Encode)]
struct NexusConsensusPolicyPreimageLegacyV1 {
    version: u8,
    configured_lane_catalog_hash: [u8; 32],
    dataspaces: Vec<NexusConsensusDataspaceV1>,
    dataspace_fee_sponsor_program_ids: Vec<(u64, FeeSponsorProgramId)>,
    routing: NexusConsensusRoutingV1,
    staking: NexusConsensusStakingLegacyV1,
    fees: RootFeePolicy,
    hf_shared_leases: NexusConsensusHfSharedLeasesV1,
    uploaded_models: NexusConsensusUploadedModelsV1,
    endorsement: NexusConsensusEndorsementV1,
    axt: NexusConsensusAxtV1,
    atomic_private_settlement: NexusConsensusAtomicPrivateSettlementLegacyV1,
    governance: NexusConsensusGovernanceV1,
    compliance_enabled: bool,
    compliance_audit_only: bool,
    compliance_policy_digest: Option<[u8; 32]>,
    lane_manifest_policy_digest: Option<[u8; 32]>,
    fusion: NexusConsensusFusionV1,
    autoscale: NexusConsensusAutoscaleV1,
    commit_window_slots: u16,
    da: NexusConsensusDaV1,
}

/// Re-encode the current preimage in the original layout: configured staking terms, the
/// committed root fee policy by value, and the retired atomic-settlement activation terms.
pub(super) fn nexus_policy_preimage(
    current: NexusConsensusPolicyPreimageV1<'_>,
    legacy: &LegacyGenesisV1,
    fees: &RootFeePolicy,
) -> Vec<u8> {
    let NexusConsensusPolicyPreimageV1 {
        version,
        configured_lane_catalog_hash,
        dataspaces,
        dataspace_fee_sponsor_program_ids,
        routing,
        staking,
        fees: _,
        hf_shared_leases,
        uploaded_models,
        endorsement,
        axt,
        atomic_private_settlement: settlement,
        governance,
        compliance_enabled,
        compliance_audit_only,
        compliance_policy_digest,
        lane_manifest_policy_digest,
        fusion,
        autoscale,
        commit_window_slots,
        da,
    } = current;
    let terms = &legacy.staking;
    NexusConsensusPolicyPreimageLegacyV1 {
        version,
        configured_lane_catalog_hash,
        dataspaces,
        dataspace_fee_sponsor_program_ids,
        routing,
        staking: NexusConsensusStakingLegacyV1 {
            public_validator_mode: staking.public_validator_mode,
            restricted_validator_mode: staking.restricted_validator_mode,
            min_validator_stake: terms.min_validator_stake.clone(),
            max_validators: staking.max_validators,
            max_stake_shares_per_validator: terms.max_stake_shares_per_validator.get(),
            max_pending_unbonds_per_share: terms.max_pending_unbonds_per_share.get(),
            unbonding_delay: terms.unbonding_delay.into(),
            max_slash_bps: terms.max_slash_bps,
            stake_asset_id: terms.stake_asset_id.clone(),
            stake_escrow_account_id: terms.stake_escrow_account_id.clone(),
            slash_sink_account_id: terms.slash_sink_account_id.clone(),
        },
        fees: fees.clone(),
        hf_shared_leases,
        uploaded_models,
        endorsement,
        axt,
        atomic_private_settlement: NexusConsensusAtomicPrivateSettlementLegacyV1 {
            enabled: ATOMIC_PRIVATE_SETTLEMENT_ENABLED,
            activation_height: None,
            minimum_activation_notice_blocks:
                ATOMIC_PRIVATE_SETTLEMENT_MINIMUM_ACTIVATION_NOTICE_BLOCKS,
            proof_profile_version: settlement.proof_profile_version,
            max_participants: settlement.max_participants,
            max_expiry_blocks: settlement.max_expiry_blocks,
            audit_timeout_blocks: settlement.audit_timeout_blocks,
            prepare_timeout_blocks: settlement.prepare_timeout_blocks,
            commit_timeout_blocks: settlement.commit_timeout_blocks,
            capsule_padding_classes_bytes: settlement.capsule_padding_classes_bytes,
            max_proof_bytes: settlement.max_proof_bytes,
            max_capsule_bytes: settlement.max_capsule_bytes,
            max_carrier_bytes: settlement.max_carrier_bytes,
            sidecar_retention_blocks: settlement.sidecar_retention_blocks,
            sidecar_max_records: settlement.sidecar_max_records,
            sidecar_max_total_bytes: settlement.sidecar_max_total_bytes,
            default_min_auditor_approvals: settlement.default_min_auditor_approvals,
            permitted_policy_versions: settlement.permitted_policy_versions,
        },
        governance,
        compliance_enabled,
        compliance_audit_only,
        compliance_policy_digest,
        lane_manifest_policy_digest,
        fusion,
        autoscale,
        commit_window_slots,
        da,
    }
    .encode()
}

#[cfg(test)]
mod tests {
    use iroha_crypto::KeyPair;
    use iroha_data_model::{account::AccountId, asset::AssetDefinitionId};

    use super::*;

    // Field bytes of the live Taira genesis Nexus/AMX preimage (taira-reset-20261022
    // new-genesis-public/nexus-amx-context.v1.bin, whose hash is the signed 0ece466e…0103).
    const TAIRA_STAKE_ASSET: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
    const TAIRA_CUSTODY: &str = "testuﾛ1NﾆﾏﾃoヱaPｾﾀﾑQｱYｽｼHo9ﾑmｸa4Rﾖ3cnﾚｦﾋDﾈﾋWoUwFNPF2D";
    const TAIRA_STAKING_FIELDS: [(&str, &str); 9] = [
        (
            "nexus.staking.min_validator_stake",
            "0501000000010400000000",
        ),
        ("nexus.staking.max_validators", "20000000"),
        ("nexus.staking.max_stake_shares_per_validator", "00010000"),
        ("nexus.staking.max_pending_unbonds_per_share", "08000000"),
        (
            "nexus.staking.unbonding_delay_ns",
            "00007beb2f7206000000000000000000",
        ),
        ("nexus.staking.max_slash_bps", "1027"),
        (
            "nexus.staking.stake_asset_id",
            "1c365445414a716262386f45506d4c6e636f4e694d52624c454b367477",
        ),
        (
            "nexus.staking.stake_escrow_account_id",
            "5a7465737475efbe9b314eefbe86efbe8fefbe836fe383b16150efbdbeefbe80efbe9151efbdb159efbdbdefbdbc486f39efbe916defbdb8613452efbe9633636eefbe9aefbda6efbe8b44efbe88efbe8b576f5577464e50463244",
        ),
        (
            "nexus.staking.slash_sink_account_id",
            "5a7465737475efbe9b314eefbe86efbe8fefbe836fe383b16150efbdbeefbe80efbe9151efbdb159efbdbdefbdbc486f39efbe916defbdb8613452efbe9633636eefbe9aefbda6efbe8b44efbe88efbe8b576f5577464e50463244",
        ),
    ];

    fn taira_legacy() -> LegacyGenesisV1 {
        let account = AccountId::new(
            KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
                .unwrap()
                .public_key()
                .clone(),
        );
        let asset = AssetDefinitionId::parse_address_literal(TAIRA_STAKE_ASSET).unwrap();
        LegacyGenesisV1 {
            crypto: LegacyGenesisCryptoV1 {
                default_hash: "blake2b-256".to_owned(),
                allowed_signing: vec![
                    Algorithm::Ed25519,
                    Algorithm::Secp256k1,
                    Algorithm::BlsNormal,
                ],
                sm2_distid_default: "1234567812345678".to_owned(),
                allowed_curve_ids: vec![1, 3, 4],
                policy: CryptoAdmissionPolicyV1::new(vec![
                    Algorithm::Ed25519,
                    Algorithm::Secp256k1,
                    Algorithm::BlsNormal,
                ])
                .unwrap(),
            },
            staking: LegacyGenesisStakingV1 {
                min_validator_stake: Quantity::from(1_u64),
                max_stake_shares_per_validator: NonZeroU32::new(256).unwrap(),
                max_pending_unbonds_per_share: NonZeroU32::new(8).unwrap(),
                unbonding_delay: Duration::from_millis(1_814_400_000),
                max_slash_bps: 10_000,
                stake_asset_id: TAIRA_STAKE_ASSET.to_owned(),
                stake_escrow_account_id: TAIRA_CUSTODY.to_owned(),
                slash_sink_account_id: TAIRA_CUSTODY.to_owned(),
                policy: PublicLaneStakingPolicy {
                    min_validator_stake: Quantity::from(1_u64),
                    max_stake_shares_per_validator: NonZeroU32::new(256).unwrap(),
                    max_pending_unbonds_per_share: NonZeroU32::new(8).unwrap(),
                    unbonding_delay_ms: 1_814_400_000,
                    max_slash_bps: 10_000,
                    stake_asset_id: asset,
                    stake_escrow_account_id: account.clone(),
                    slash_sink_account_id: account,
                },
            },
        }
    }

    fn fields(preimage: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut rest = preimage
            .strip_prefix(iroha_data_model::nexus::NEXUS_AMX_CONTEXT_DOMAIN_V1)
            .unwrap();
        let mut fields = Vec::new();
        while !rest.is_empty() {
            let tag_len = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
            let tag = String::from_utf8(rest[4..4 + tag_len].to_vec()).unwrap();
            rest = &rest[4 + tag_len..];
            let value_len = u64::from_le_bytes(rest[..8].try_into().unwrap()) as usize;
            fields.push((tag, rest[8..8 + value_len].to_vec()));
            rest = &rest[8 + value_len..];
        }
        fields
    }

    #[test]
    fn staking_and_fee_terms_keep_the_original_preimage_layout() {
        let nexus = Nexus::default();
        let mut preimage = NexusAmxContextWriterV1::new();
        write_amx_staking_and_fees(&mut preimage, &nexus, &taira_legacy(), &nexus.fees);
        let written = fields(&preimage.finish());
        let tags = written
            .iter()
            .map(|(tag, _)| tag.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            tags,
            [
                "nexus.staking.min_validator_stake",
                "nexus.staking.max_validators",
                "nexus.staking.max_stake_shares_per_validator",
                "nexus.staking.max_pending_unbonds_per_share",
                "nexus.staking.unbonding_delay_ns",
                "nexus.staking.max_slash_bps",
                "nexus.staking.stake_asset_id",
                "nexus.staking.stake_escrow_account_id",
                "nexus.staking.slash_sink_account_id",
                "nexus.fees.asset",
                "nexus.fees.sink",
                "nexus.fees.base",
                "nexus.fees.per_byte",
                "nexus.fees.per_instruction",
                "nexus.fees.per_gas_unit",
                "nexus.fees.sponsor_vault_custody_account_id",
                "nexus.fees.successful_claim_exempt_authorities",
            ]
        );
        for (tag, golden) in TAIRA_STAKING_FIELDS {
            let (_, value) = written.iter().find(|(name, _)| name == tag).unwrap();
            assert_eq!(hex::encode(value), golden, "{tag}");
        }
    }

    #[test]
    fn legacy_crypto_terms_change_the_execution_policy_identity() {
        let legacy = taira_legacy();
        let mut current = ExecutionPolicyFieldsV1::default();
        let mut original = ExecutionPolicyFieldsV1::default();
        push_execution_crypto(&mut original, &legacy.crypto);
        current.push("pipeline.amx_per_syscall_ns", &0_u64);
        original.push("pipeline.amx_per_syscall_ns", &0_u64);
        let names = original
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            [
                "crypto.default_hash",
                "crypto.allowed_signing",
                "crypto.sm2_distid_default",
                "crypto.allowed_curve_ids",
                "pipeline.amx_per_syscall_ns",
            ]
        );
        assert_eq!(current.fields.len(), 1);
    }
}
