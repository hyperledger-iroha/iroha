//! No-governance validation-fee registry feature-boundary regression.

use std::str::FromStr as _;

use iroha_crypto::{Algorithm, KeyPair, PublicKey};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    asset::AssetDefinitionId,
    domain::DomainId,
    name::Name,
    validation_fee::{
        VALIDATION_FEE_DS_SCALE, VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS,
        VALIDATION_FEE_POLICY_SCHEMA_VERSION, ValidationFeeChargingMode,
        ValidationFeeFinalizationEvidenceV1, ValidationFeeGovernanceVotingModeV1,
        ValidationFeeGovernanceWindowV1, ValidationFeeParliamentAuthorizationV1,
        ValidationFeePlainElectorateEligibilityRuleV1, ValidationFeePlainElectorateMemberV1,
        ValidationFeePlainElectorateRulesV1, ValidationFeePlainElectorateSnapshotV1,
        ValidationFeePolicyRegistryEntryV1, ValidationFeePolicyRegistryError,
        ValidationFeePolicyRegistryV1, ValidationFeePolicyV1, initial_validation_fee_amount,
    },
};

const PARITY_PUBLIC_KEY: &str =
    "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
const REFERENDUM_START_HEIGHT: u64 = 1;
const REFERENDUM_DURATION_BLOCKS: u64 = 3_600;
const REFERENDUM_END_HEIGHT: u64 = REFERENDUM_START_HEIGHT + REFERENDUM_DURATION_BLOCKS - 1;
const POLICY_ENACTMENT_HEIGHT: u64 = REFERENDUM_END_HEIGHT + 1;
const POLICY_EFFECTIVE_HEIGHT: u64 =
    POLICY_ENACTMENT_HEIGHT + VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS;

fn parity_account_id() -> AccountId {
    let public_key: PublicKey = PARITY_PUBLIC_KEY.parse().expect("parse public key");
    AccountId::new(public_key)
}

fn fixture_account_id(seed: u8) -> AccountId {
    let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
        .expect("derive deterministic fixture account");
    AccountId::new(key_pair.public_key().clone())
}

#[test]
fn typed_validation_fee_registry_fails_closed_without_governance() {
    let proposal_id = [0x12; 32];
    let policy = ValidationFeePolicyV1 {
        schema_version: VALIDATION_FEE_POLICY_SCHEMA_VERSION,
        chain_id: ChainId::from("kotlin-fixture-no-governance"),
        genesis_hash: [7; 32],
        policy_version: 1,
        previous_policy_hash: None,
        ds_asset_id: AssetDefinitionId::derive_from_components(
            DomainId::try_new("fees", "validation").expect("domain id"),
            Name::from_str("fee").expect("asset name"),
        ),
        ds_scale: VALIDATION_FEE_DS_SCALE,
        fee: initial_validation_fee_amount(),
        treasury_account_id: parity_account_id(),
        charging_mode: ValidationFeeChargingMode::PerQualifyingTransferInstruction,
        effective_from_height: POLICY_EFFECTIVE_HEIGHT,
        expires_after_height: None,
        exemption_classes: Vec::new(),
        treasury_payout_binding: None,
    };
    assert_eq!(policy.policy_invariant_error(), None);
    let plain_electorate_rules = ValidationFeePlainElectorateRulesV1 {
        voting_asset_id: AssetDefinitionId::derive_from_components(
            DomainId::try_new("governance", "validation").expect("governance domain id"),
            Name::from_str("vote").expect("voting asset name"),
        ),
        bond_escrow_account: fixture_account_id(1),
        slash_receiver_account: fixture_account_id(2),
        ballot_amount: 150_u64.into(),
        ballot_duration_blocks: REFERENDUM_DURATION_BLOCKS,
        citizenship_amount: 10_000_u64.into(),
        max_members: 256,
        conviction_step_blocks: 100,
        max_conviction: 6,
        min_turnout: 1,
        approval_threshold_numerator: 1,
        approval_threshold_denominator: 2,
        eligibility_rule:
            ValidationFeePlainElectorateEligibilityRuleV1::ProposalOperatorAtOrBeforeGateOthersAfterGate,
    };
    let proposal_operator = parity_account_id();
    let approval_gate_height = REFERENDUM_START_HEIGHT - 1;
    let electorate = ValidationFeePlainElectorateSnapshotV1::from_canonical_members(
        proposal_id,
        proposal_operator.clone(),
        REFERENDUM_START_HEIGHT,
        approval_gate_height,
        vec![ValidationFeePlainElectorateMemberV1 {
            account_id: proposal_operator.clone(),
            bonded_height: approval_gate_height,
            bonded_amount: plain_electorate_rules.citizenship_amount.clone(),
        }],
    )
    .expect("canonical feature-boundary PLAIN electorate snapshot");
    assert_eq!(
        electorate.context_error(proposal_id, &proposal_operator, &plain_electorate_rules),
        None
    );
    let authorization = ValidationFeeParliamentAuthorizationV1 {
        proposal_id,
        proposal_fingerprint: proposal_id,
        proposal_time_roster_root: [0x13; 32],
        plain_electorate_snapshot_root: electorate.roster_root,
        plain_electorate_snapshot_member_count: electorate.member_count,
        plain_electorate_snapshot_captured_at_height: electorate.captured_at_height,
        plain_electorate_snapshot_approval_gate_height: electorate.approval_gate_height,
        referendum_window: ValidationFeeGovernanceWindowV1 {
            lower: REFERENDUM_START_HEIGHT,
            upper: REFERENDUM_END_HEIGHT,
        },
        finalization: ValidationFeeFinalizationEvidenceV1 {
            referendum_id: proposal_id,
            finalized_at_height: REFERENDUM_END_HEIGHT,
            mode: ValidationFeeGovernanceVotingModeV1::Plain,
            approve: 1,
            reject: 0,
            abstain: 0,
            min_turnout: 1,
            approval_threshold_numerator: 1,
            approval_threshold_denominator: 2,
            approved: true,
        },
        enacted_at_height: POLICY_ENACTMENT_HEIGHT,
    };
    assert_eq!(authorization.invariant_error(), None);
    let registry = ValidationFeePolicyRegistryV1 {
        registered_policies: vec![
            ValidationFeePolicyRegistryEntryV1::from_enactment(
                policy,
                plain_electorate_rules,
                authorization,
                None,
            )
            .expect("policy hash"),
        ],
    };
    let encoded = norito::to_bytes(&registry).expect("encode typed registry");
    let decoded: ValidationFeePolicyRegistryV1 =
        norito::decode_from_bytes(&encoded).expect("decode typed registry");

    assert!(matches!(
        decoded.validate(),
        Err(ValidationFeePolicyRegistryError::InvalidParliamentAuthorization { policy_version: 1 })
    ));
}
