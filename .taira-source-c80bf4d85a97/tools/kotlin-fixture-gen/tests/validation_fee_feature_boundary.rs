//! No-governance validation-fee registry feature-boundary regression.

use std::str::FromStr as _;

use iroha_crypto::PublicKey;
use iroha_data_model::{
    ChainId,
    account::AccountId,
    asset::AssetDefinitionId,
    domain::DomainId,
    name::Name,
    validation_fee::{
        VALIDATION_FEE_DS_SCALE, VALIDATION_FEE_POLICY_SCHEMA_VERSION, ValidationFeeChargingMode,
        ValidationFeeFinalizationEvidenceV1, ValidationFeeGovernanceVotingModeV1,
        ValidationFeeGovernanceWindowV1, ValidationFeeParliamentAuthorizationV1,
        ValidationFeePolicyRegistryEntryV1, ValidationFeePolicyRegistryError,
        ValidationFeePolicyRegistryV1, ValidationFeePolicyV1, initial_validation_fee_amount,
    },
};

const PARITY_PUBLIC_KEY: &str =
    "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";

fn parity_account_id() -> AccountId {
    let public_key: PublicKey = PARITY_PUBLIC_KEY.parse().expect("parse public key");
    AccountId::new(public_key)
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
        ds_asset_id: AssetDefinitionId::new(
            DomainId::try_new("fees", "validation").expect("domain id"),
            Name::from_str("fee").expect("asset name"),
        ),
        ds_scale: VALIDATION_FEE_DS_SCALE,
        fee: initial_validation_fee_amount(),
        treasury_account_id: parity_account_id(),
        charging_mode: ValidationFeeChargingMode::PerQualifyingTransferInstruction,
        effective_from_height: 10,
        expires_after_height: None,
        exemption_classes: Vec::new(),
        treasury_payout_binding: None,
    };
    assert_eq!(policy.policy_invariant_error(), None);
    let authorization = ValidationFeeParliamentAuthorizationV1 {
        proposal_id,
        proposal_fingerprint: proposal_id,
        proposal_time_roster_root: [0x13; 32],
        referendum_window: ValidationFeeGovernanceWindowV1 {
            lower: 1,
            upper: 100,
        },
        finalization: ValidationFeeFinalizationEvidenceV1 {
            referendum_id: proposal_id,
            finalized_at_height: 12,
            mode: ValidationFeeGovernanceVotingModeV1::Plain,
            approve: 1,
            reject: 0,
            abstain: 0,
            min_turnout: 1,
            approval_threshold_numerator: 1,
            approval_threshold_denominator: 2,
            approved: true,
        },
        enacted_at_height: 13,
    };
    assert_eq!(authorization.invariant_error(), None);
    let registry = ValidationFeePolicyRegistryV1 {
        registered_policies: vec![
            ValidationFeePolicyRegistryEntryV1::from_enactment(policy, authorization, None)
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
