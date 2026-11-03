//! Complete graph custody, exact predecessor and original-pool negative controls.

use super::*;
use crate::pipeline_gas_policy::ParameterIngressError;
use iroha_crypto::Algorithm;
use iroha_data_model::{
    account::AccountId,
    block::consensus::{PublicLaneStakingPolicy, RootFeePolicy},
    crypto_admission_policy::CryptoAdmissionPolicyV1,
    fraud::{FraudAdmissionAttesterV1, FraudAdmissionPolicyV1},
    parameter::{CustomParameter, Parameter},
    zk_validity_policy::{ZkValidityPolicyV1, ZkVerifyingKeyRefV1},
};
use iroha_primitives::json::Json;
use norito::codec::Encode as _;

pub(in crate::pipeline_gas_policy) fn authored() -> StateParameters {
    let mut source = StateParameters::default();
    let mut fees = iroha_config::parameters::defaults::nexus::fees::genesis_policy();
    fees.base_fee = "0.125".parse().unwrap();
    fees.per_byte_fee = "1.25".parse().unwrap();
    fees.per_instruction_fee = "12.5".parse().unwrap();
    fees.per_gas_unit_fee = "125".parse().unwrap();
    fees.successful_claim_fee_exempt_authorities
        .try_insert(fees.fee_sink_account_id.clone())
        .unwrap();
    let account = fees.fee_sink_account_id.clone();
    let asset = fees.fee_asset_id.clone();
    source
        .set_parameter(Parameter::Custom(fees.into_custom_parameter().unwrap()))
        .unwrap();
    let crypto = CryptoAdmissionPolicyV1::new(vec![Algorithm::Ed25519]).unwrap();
    source
        .set_parameter(Parameter::Custom(CustomParameter::new(
            CryptoAdmissionPolicyV1::parameter_id(),
            Json::new(crypto),
        )))
        .unwrap();
    let staking = PublicLaneStakingPolicy {
        min_validator_stake: "8192.125".parse().unwrap(),
        max_stake_shares_per_validator: 7_u32.try_into().unwrap(),
        max_pending_unbonds_per_share: 3_u32.try_into().unwrap(),
        unbonding_delay_ms: 35_000,
        max_slash_bps: 1234,
        stake_asset_id: asset.clone(),
        stake_escrow_account_id: account.clone(),
        slash_sink_account_id: account.clone(),
    };
    source
        .set_parameter(Parameter::Custom(staking.into_custom_parameter().unwrap()))
        .unwrap();
    let mut fraud = FraudAdmissionPolicyV1::protocol_default();
    fraud.attesters.push(FraudAdmissionAttesterV1 {
        engine_id: "bounded-engine".into(),
        public_key: "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .unwrap(),
    });
    source
        .set_parameter(Parameter::Custom(fraud.into_custom_parameter()))
        .unwrap();
    let mut zk = ZkValidityPolicyV1::protocol_default();
    zk.proofs.kaigi_authorization_vk = Some(ZkVerifyingKeyRefV1 {
        backend: "pipa".into(),
        name: "authorization".into(),
    });
    zk.proofs.kaigi_usage_vk = Some(ZkVerifyingKeyRefV1 {
        backend: "stark".into(),
        name: "usage".into(),
    });
    source
        .set_parameter(Parameter::Custom(zk.into_custom_parameter()))
        .unwrap();
    let asset_address = asset.canonical_address();
    source
        .set_parameter(Parameter::Custom(CustomParameter::new(
            super::super::ACCEPTED_ASSETS.parse().unwrap(),
            Json::new(vec![asset_address.clone()]),
        )))
        .unwrap();
    let rates = vec![super::super::CommittedGasRate {
        asset: asset_address,
        units_per_gas: 13,
        twap_local_per_xor: Some("123.125".into()),
        liquidity_profile: Some("tier1".into()),
        volatility_class: Some("elevated".into()),
    }];
    source
        .set_parameter(Parameter::Custom(CustomParameter::new(
            super::super::RATES.parse().unwrap(),
            Json::new(rates),
        )))
        .unwrap();
    source
}

#[test]
fn complete_graph_copy_preserves_canonical_bytes_and_every_projection() {
    let source = authored();
    let before = source.clone();
    let pool = AllocationBudget::new(1 << 20);
    let retained = RetainedStateParameters::try_copy_for_testing(&source, &pool).unwrap();
    assert_eq!(retained.get(), &source);
    assert_eq!(retained.get().encode(), source.encode());
    assert_eq!(retained.get().gas(), source.gas());
    assert_eq!(retained.get().fraud_policy(), source.fraud_policy());
    assert_eq!(retained.get().zk_policy(), source.zk_policy());
    assert_eq!(retained.get().root_fees(), source.root_fees());
    assert_eq!(
        retained.get().root_fees().unwrap().fee_asset_id,
        source.root_fees().unwrap().fee_asset_id
    );
    assert_eq!(
        retained
            .get()
            .require_staking_policy()
            .unwrap()
            .stake_asset_id,
        source.require_staking_policy().unwrap().stake_asset_id
    );
    assert_eq!(
        retained.get().require_crypto_policy(),
        source.require_crypto_policy()
    );
    assert_eq!(
        retained.get().require_staking_policy().unwrap(),
        source.require_staking_policy().unwrap()
    );
    assert!(retained.belongs_to(&pool));
    assert!(pool.reserved_bytes() > 0);
    assert_eq!(source, before);
    drop(retained);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn mv_clones_share_exact_custody_until_the_last_version_retires() {
    let pool = AllocationBudget::new(1 << 20);
    let retained = RetainedStateParameters::try_copy_for_testing(&authored(), &pool).unwrap();
    let used = pool.reserved_bytes();
    let snapshot = retained.clone();
    assert!(snapshot.same_version(&retained));
    assert_eq!(pool.reserved_bytes(), used);
    drop(retained);
    assert_eq!(pool.reserved_bytes(), used);
    drop(snapshot);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn zero_pool_refuses_original_demand_without_mutating_source() {
    let source = authored();
    let bytes = source.encode();
    let pool = AllocationBudget::new(0);
    assert!(matches!(
        RetainedStateParameters::try_copy_for_testing(&source, &pool),
        Err(ParameterIngressError::Retention(
            ParameterRetentionError::Admission(_)
        ))
    ));
    assert_eq!(source.encode(), bytes);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn raw_json_clone_keeps_its_own_charge_after_outer_graph_retirement() {
    let pool = AllocationBudget::new(1 << 20);
    let retained = RetainedStateParameters::try_copy_for_testing(&authored(), &pool).unwrap();
    let raw_json = retained
        .get()
        .parameters
        .custom()
        .values()
        .next()
        .unwrap()
        .payload()
        .clone();
    assert!(raw_json.belongs_to(&pool));
    let all = pool.reserved_bytes();
    drop(retained);
    let remaining = pool.reserved_bytes();
    assert!(remaining > 0 && remaining < all);
    assert!(raw_json.belongs_to(&pool));
    drop(raw_json);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn successor_shares_unchanged_leaves_so_predecessor_retirement_refunds_only_its_graph() {
    let pool = AllocationBudget::new(1 << 20);
    let current = RetainedStateParameters::try_copy_for_testing(&authored(), &pool).unwrap();
    let current_bytes = pool.reserved_bytes();
    // Each decoded leaf owns its text and shared control; measure that exact
    // charge by retaining the same text into a separate pool.
    let leaf_bytes: usize = current
        .get()
        .custom()
        .values()
        .map(|parameter| {
            let scratch = AllocationBudget::new(1 << 20);
            let _copy = parameter.payload().try_retain_in(&scratch).unwrap();
            scratch.reserved_bytes()
        })
        .sum();
    assert!(leaf_bytes > 0 && leaf_bytes < current_bytes);
    let added = CustomParameter::new("successor_marker".parse().unwrap(), Json::new(1_u32));
    let next = current
        .prepare_custom_parameter(&added, &pool)
        .unwrap()
        .into_successor(&current, &pool)
        .unwrap();
    for parameter in current.get().custom().values() {
        let shared = next.get().custom().get(parameter.id()).unwrap();
        assert!(shared.payload().ptr_eq(parameter.payload()));
    }
    let both = pool.reserved_bytes();
    drop(current);
    assert_eq!(pool.reserved_bytes(), both - (current_bytes - leaf_bytes));
    drop(next);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn equal_value_in_another_version_cannot_accept_a_prepared_successor() {
    let source = authored();
    let pool = AllocationBudget::new(1 << 20);
    let original = RetainedStateParameters::try_copy_for_testing(&source, &pool).unwrap();
    let mut equal_other = RetainedStateParameters::try_copy_for_testing(&source, &pool).unwrap();
    assert_eq!(original, equal_other);
    let before = equal_other.clone();
    let prepared = original.prepare_replace(&source, &pool).unwrap();
    let (prepared, error) = prepared.install(&mut equal_other, &pool).unwrap_err();
    assert!(matches!(error, ParameterRetentionError::StalePredecessor));
    assert!(before.same_version(&equal_other));
    assert!(prepared.check_source(&original, &pool).is_ok());
}

#[test]
fn applied_sibling_invalidates_preparation_even_under_the_same_original_pool() {
    let source = authored();
    let pool = AllocationBudget::new(1 << 20);
    let mut current = RetainedStateParameters::try_copy_for_testing(&source, &pool).unwrap();
    let old_view = current.clone();
    let first = current.prepare_replace(&source, &pool).unwrap();
    let second = current.prepare_replace(&source, &pool).unwrap();
    first.install(&mut current, &pool).unwrap();
    let after_first = current.clone();
    let (_, error) = second.install(&mut current, &pool).unwrap_err();
    assert!(matches!(error, ParameterRetentionError::StalePredecessor));
    assert!(after_first.same_version(&current));
    assert!(!old_view.same_version(&current));
    assert_eq!(old_view.get(), &source);
}

#[test]
fn prepared_update_refuses_foreign_pool_before_any_change() {
    let source = authored();
    let pool = AllocationBudget::new(1 << 20);
    let foreign = AllocationBudget::new(1 << 20);
    let mut current = RetainedStateParameters::try_copy_for_testing(&source, &pool).unwrap();
    let before = current.clone();
    assert!(matches!(
        current.prepare_replace(&source, &foreign),
        Err(ParameterIngressError::Retention(
            ParameterRetentionError::ForeignPool
        ))
    ));
    let prepared = current.prepare_replace(&source, &pool).unwrap();
    let (prepared, error) = prepared.install(&mut current, &foreign).unwrap_err();
    assert!(matches!(error, ParameterRetentionError::ForeignPool));
    assert!(current.same_version(&before));
    assert_eq!(foreign.reserved_bytes(), 0);
    prepared.install(&mut current, &pool).unwrap();
}

#[test]
fn mandatory_policy_removal_refuses_before_allocating_a_successor() {
    let pool = AllocationBudget::new(1 << 20);
    let current = RetainedStateParameters::try_copy_for_testing(&authored(), &pool).unwrap();
    let used = pool.reserved_bytes();
    assert!(matches!(
        current.prepare_replace(&StateParameters::default(), &pool),
        Err(ParameterIngressError::Retention(
            ParameterRetentionError::MissingInitializedPolicy
        ))
    ));
    assert_eq!(pool.reserved_bytes(), used);
}

#[test]
fn abandoning_a_prepared_update_refunds_only_the_successor() {
    let source = authored();
    let pool = AllocationBudget::new(1 << 20);
    let current = RetainedStateParameters::try_copy_for_testing(&source, &pool).unwrap();
    let used = pool.reserved_bytes();
    let prepared = current.prepare_replace(&source, &pool).unwrap();
    assert!(pool.reserved_bytes() > used);
    drop(prepared);
    assert_eq!(pool.reserved_bytes(), used);
    assert_eq!(current.get(), &source);
}

#[test]
fn bootstrap_absence_is_preserved_without_fabricating_monetary_authority() {
    let pool = AllocationBudget::new(1 << 20);
    let bootstrap =
        RetainedStateParameters::try_copy_for_testing(&StateParameters::default(), &pool).unwrap();
    assert!(bootstrap.get().crypto_policy().is_none());
    assert!(bootstrap.get().staking_policy().is_none());
    assert!(bootstrap.get().root_fees().is_none());
    assert!(bootstrap.belongs_to(&pool));
    drop(bootstrap);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn multisig_custody_and_exemptions_keep_all_member_backing_until_last_view() {
    use iroha_data_model::account::{MultisigMember, MultisigPolicy};
    let members = [0x91, 0x92].map(|seed| {
        let key = iroha_crypto::KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        MultisigMember::new(key.public_key().clone(), 1).unwrap()
    });
    let multisig = AccountId::new_multisig(MultisigPolicy::new(2, members.to_vec()).unwrap());
    let mut source = authored();
    let mut staking = source.require_staking_policy().unwrap().clone();
    staking.stake_escrow_account_id = multisig.clone();
    staking.slash_sink_account_id = multisig.clone();
    source
        .set_parameter(Parameter::Custom(staking.into_custom_parameter().unwrap()))
        .unwrap();
    let mut fees = source.root_fees().unwrap().clone();
    fees.fee_sink_account_id = multisig.clone();
    fees.sponsor_vault_custody_account_id = multisig.clone();
    assert!(
        fees.successful_claim_fee_exempt_authorities
            .try_insert(multisig.clone())
            .unwrap()
    );
    source
        .set_parameter(Parameter::Custom(fees.into_custom_parameter().unwrap()))
        .unwrap();
    let pool = AllocationBudget::new(1 << 20);
    let owner = RetainedStateParameters::try_copy_for_testing(&source, &pool).unwrap();
    assert_eq!(owner.get(), &source);
    assert_eq!(
        owner
            .get()
            .require_staking_policy()
            .unwrap()
            .stake_escrow_account_id,
        multisig
    );
    assert!(
        owner
            .get()
            .root_fees()
            .unwrap()
            .successful_claim_fee_exempt_authorities
            .contains(&multisig)
    );
    let used = pool.reserved_bytes();
    let view = owner.clone();
    drop(owner);
    assert_eq!(pool.reserved_bytes(), used);
    drop(view);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn explicit_missing_policy_fixture_is_funded_but_cannot_be_a_runtime_successor() {
    let source = authored();
    let pool = AllocationBudget::new(1 << 20);
    let original = RetainedStateParameters::try_copy_for_testing(&source, &pool).unwrap();
    let mut missing = source.clone();
    missing
        .remove_custom_parameter(&RootFeePolicy::parameter_id())
        .unwrap();
    assert!(matches!(
        original.prepare_replace(&missing, &pool),
        Err(ParameterIngressError::Retention(
            ParameterRetentionError::MissingInitializedPolicy
        ))
    ));
    let held = pool.reserved_bytes();
    let limit = pool.limit_bytes();
    pool.set_limit_bytes(held);
    assert!(matches!(
        RetainedStateParameters::try_copy_for_testing(&missing, &pool),
        Err(ParameterIngressError::Retention(
            ParameterRetentionError::Admission(_)
        ))
    ));
    assert_eq!(pool.reserved_bytes(), held);
    assert!(original.root_fees().is_some());
    pool.set_limit_bytes(limit);
    let malformed = RetainedStateParameters::try_copy_for_testing(&missing, &pool).unwrap();
    assert!(malformed.root_fees().is_none());
    assert!(malformed.belongs_to(&pool));
    assert!(matches!(
        original.prepare_admitted(malformed.clone(), &pool),
        Err((_, ParameterRetentionError::MissingInitializedPolicy))
    ));
    let snapshot = malformed.clone();
    drop(malformed);
    assert!(pool.reserved_bytes() > held);
    drop(snapshot);
    assert_eq!(pool.reserved_bytes(), held);
    drop(original);
    assert_eq!(pool.reserved_bytes(), 0);
}
