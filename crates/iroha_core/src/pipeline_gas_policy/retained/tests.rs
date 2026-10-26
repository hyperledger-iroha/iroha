//! Complete graph custody, exact predecessor and original-pool negative controls.

use super::*;
use iroha_data_model::parameter::{CustomParameter, Parameter};
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
    let retained = RetainedStateParameters::try_copy(&source, &pool).unwrap();
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
    let retained = RetainedStateParameters::try_copy(&authored(), &pool).unwrap();
    let used = pool.reserved_bytes();
    let snapshot = retained.clone();
    let sibling = retained.try_retain_in(&pool).unwrap();
    assert!(snapshot.same_version(&retained));
    assert!(sibling.same_version(&retained));
    assert_eq!(pool.reserved_bytes(), used);
    drop(retained);
    drop(sibling);
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
        RetainedStateParameters::try_copy(&source, &pool),
        Err(ParameterRetentionError::Admission(_))
    ));
    assert_eq!(source.encode(), bytes);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn custom_retention_refusal_releases_the_already_copied_graph_prefix() {
    let source = authored();
    let mut plan = Plan::default();
    visit_graph(&source, &mut |layout| plan.add(layout)).unwrap();
    let plain = plan.bytes
        + array::<AllocationCharge>(plan.count).unwrap().size()
        + ChargedShared::<ParameterGraph>::allocation_layout().size();
    let pool = AllocationBudget::new(plain);
    assert!(matches!(
        RetainedStateParameters::try_copy(&source, &pool),
        Err(ParameterRetentionError::Custom(_))
    ));
    assert_eq!(pool.reserved_bytes(), 0);
    // A funded retry is a new admission, not a substituted original failure.
    let retry_pool = AllocationBudget::new(1 << 20);
    let retry = RetainedStateParameters::try_copy(&source, &retry_pool).unwrap();
    assert_eq!(retry.get(), &source);
}

#[test]
fn raw_json_clone_keeps_its_own_charge_after_outer_graph_retirement() {
    let pool = AllocationBudget::new(1 << 20);
    let retained = RetainedStateParameters::try_copy(&authored(), &pool).unwrap();
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
fn foreign_destination_requires_independent_funding_and_preserves_original_pool() {
    let source_pool = AllocationBudget::new(1 << 20);
    let source = RetainedStateParameters::try_copy(&authored(), &source_pool).unwrap();
    let original = source_pool.reserved_bytes();
    let refused_pool = AllocationBudget::new(0);
    assert!(source.try_retain_in(&refused_pool).is_err());
    assert_eq!(refused_pool.reserved_bytes(), 0);
    assert_eq!(source_pool.reserved_bytes(), original);
    let destination = AllocationBudget::new(1 << 20);
    let copied = source.try_retain_in(&destination).unwrap();
    assert_eq!(copied, source);
    assert!(!copied.same_version(&source));
    assert!(copied.belongs_to(&destination));
    assert!(!copied.belongs_to(&source_pool));
    drop(copied);
    assert_eq!(destination.reserved_bytes(), 0);
    assert_eq!(source_pool.reserved_bytes(), original);
}

#[test]
fn equal_value_in_another_version_cannot_accept_a_prepared_successor() {
    let source = authored();
    let pool = AllocationBudget::new(1 << 20);
    let original = RetainedStateParameters::try_copy(&source, &pool).unwrap();
    let mut equal_other = RetainedStateParameters::try_copy(&source, &pool).unwrap();
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
    let mut current = RetainedStateParameters::try_copy(&source, &pool).unwrap();
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
    let mut current = RetainedStateParameters::try_copy(&source, &pool).unwrap();
    let before = current.clone();
    assert!(matches!(
        current.prepare_replace(&source, &foreign),
        Err(ParameterRetentionError::ForeignPool)
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
    let current = RetainedStateParameters::try_copy(&authored(), &pool).unwrap();
    let used = pool.reserved_bytes();
    assert!(matches!(
        current.prepare_replace(&StateParameters::default(), &pool),
        Err(ParameterRetentionError::MissingInitializedPolicy)
    ));
    assert_eq!(pool.reserved_bytes(), used);
}

#[test]
fn abandoning_a_prepared_update_refunds_only_the_successor() {
    let source = authored();
    let pool = AllocationBudget::new(1 << 20);
    let current = RetainedStateParameters::try_copy(&source, &pool).unwrap();
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
    let bootstrap = RetainedStateParameters::try_copy(&StateParameters::default(), &pool).unwrap();
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
    let owner = RetainedStateParameters::try_copy(&source, &pool).unwrap();
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
fn original_codec_scope_survives_observed_retention_and_saved_replay_is_invalid() {
    use norito::core::{DecodeAttemptErrorKind, DecodeBudgetContext};
    // Explicitly author a canonical monetary policy so the first compact account
    // copy encounters the enclosing zero decoder allowance. An empty bootstrap
    // intentionally has no monetary account projection to copy.
    let mut source = StateParameters::default();
    source
        .set_parameter(Parameter::Custom(
            iroha_config::parameters::defaults::nexus::fees::genesis_policy()
                .into_custom_parameter()
                .unwrap(),
        ))
        .unwrap();
    let pool = AllocationBudget::new(1 << 20);
    let zero = DecodeBudgetContext::new(norito::DecodeLimits::new(
        usize::MAX,
        usize::MAX,
        usize::MAX,
        0,
        usize::MAX,
    ));
    let mut original_identity = None;
    let refused = zero.with(|| ParameterRetentionAttemptError::observe(|| {
        RetainedStateParameters::try_copy(&source, &pool).map_err(|error| {
            let ParameterRetentionError::Codec(norito::Error::ScopedDecodeResource(original)) = &error else {
                panic!("the actual account copy must preserve its original scoped refusal: {error:?}");
            };
            original_identity = Some(original.clone());
            error
        })
    })).unwrap_err();
    assert_eq!(pool.reserved_bytes(), 0);
    let ParameterRetentionAttemptError::Codec(attempt) = refused else {
        panic!("original codec attempt");
    };
    assert_eq!(attempt.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    let refusal = attempt.into_local_refusal().unwrap();
    let original = original_identity.unwrap();
    let norito::Error::ScopedDecodeResource(retained) = refusal.into_error() else {
        panic!("retained scoped identity");
    };
    assert_eq!(retained, original);
    let saved = ParameterRetentionAttemptError::observe::<()>(|| {
        Err(ParameterRetentionError::Codec(
            norito::Error::ScopedDecodeResource(retained),
        ))
    })
    .unwrap_err();
    let ParameterRetentionAttemptError::Codec(saved) = saved else {
        panic!("saved codec attempt");
    };
    assert_eq!(saved.kind(), DecodeAttemptErrorKind::Invalid);
    assert!(saved.local_refusal().is_none());
    let retry = ParameterRetentionAttemptError::observe(|| {
        RetainedStateParameters::try_copy(&source, &pool)
    })
    .unwrap();
    assert_eq!(retry.get(), &source);
    drop(retry);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn observed_retention_keeps_original_finite_pool_refusal_separate() {
    let pool = AllocationBudget::new(0);
    let error = ParameterRetentionAttemptError::observe(|| {
        RetainedStateParameters::try_copy(&StateParameters::default(), &pool)
    })
    .unwrap_err();
    assert!(matches!(
        error,
        ParameterRetentionAttemptError::Original(ParameterRetentionError::Admission(_))
    ));
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
        Err(ParameterRetentionError::MissingInitializedPolicy)
    ));
    let held = pool.reserved_bytes();
    let limit = pool.limit_bytes();
    pool.set_limit_bytes(held);
    assert!(matches!(
        RetainedStateParameters::try_copy_for_testing(&missing, &pool),
        Err(ParameterRetentionError::Admission(_))
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
