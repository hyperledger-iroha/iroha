//! Whole-graph direct ingress, shared JSON lifetime and original refusal controls.

use super::*;
use crate::test_allocations::allocations_during;

fn read(
    source: &str,
    pool: &AllocationBudget,
) -> Result<RetainedStateParameters, ParameterIngressError> {
    let mut parser = json::Parser::new(source);
    parser.preflight_document()?;
    let value = RetainedStateParameters::try_decode_json(&mut parser, pool)?;
    parser.finish_document()?;
    Ok(value)
}
fn source() -> String {
    json::to_json(&super::super::retained::tests::authored().parameters).unwrap()
}

#[test]
fn direct_complete_graph_matches_canonical_source_and_keeps_shared_json_custody() {
    let source = source();
    let expected =
        super::super::StateParameters::try_from(json::from_str::<Parameters>(&source).unwrap())
            .unwrap();
    let pool = AllocationBudget::new(1 << 24);
    let graph = read(&source, &pool).unwrap();
    assert_eq!(graph.get(), &expected);
    assert_eq!(json::to_json(graph.get()).unwrap(), source);
    assert!(graph.belongs_to(&pool));
    let held = pool.reserved_bytes();
    let mut reader = None;
    assert_eq!(allocations_during(|| reader = Some(graph.clone())), 0);
    let escaped = graph.custom().values().next().unwrap().payload().clone();
    drop(graph);
    assert_eq!(pool.reserved_bytes(), held);
    drop(reader);
    assert!(
        pool.reserved_bytes() > 0,
        "raw JSON clone retains its original backing charge"
    );
    assert!(pool.reserved_bytes() < held);
    drop(escaped);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn direct_protocol_default_leaves_mandatory_authority_absent() {
    let pool = AllocationBudget::new(1 << 20);
    let owner = RetainedStateParameters::try_protocol_default(&pool).unwrap();
    assert!(owner.root_fees().is_none());
    assert!(owner.crypto_policy().is_none());
    assert!(owner.staking_policy().is_none());
    assert_eq!(owner.get(), &super::super::StateParameters::default());
    assert!(owner.belongs_to(&pool));
    drop(owner);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn graph_shell_refusal_precedes_parsing_and_partial_projection_refunds_then_retries() {
    let zero = AllocationBudget::new(0);
    let mut parser = json::Parser::new("not JSON");
    assert_eq!(
        allocations_during(|| {
            assert!(matches!(
                RetainedStateParameters::try_decode_json(&mut parser, &zero),
                Err(ParameterIngressError::Retention(
                    ParameterRetentionError::Admission(_)
                ))
            ));
        }),
        0
    );
    let source = source();
    let measured = AllocationBudget::new(1 << 24);
    drop(read(&source, &measured).unwrap());
    let peak = measured.peak_reserved_bytes();
    assert_eq!(measured.reserved_bytes(), 0);
    let pool = AllocationBudget::new(peak - 1);
    assert!(read(&source, &pool).is_err());
    assert_eq!(pool.reserved_bytes(), 0);
    pool.set_limit_bytes(peak);
    drop(read(&source, &pool).unwrap());
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn actual_graph_decoder_preserves_original_observer_and_rejects_saved_replay() {
    use norito::core::{DecodeAttemptErrorKind, DecodeBudgetContext};
    let source = source();
    let pool = AllocationBudget::new(1 << 24);
    let zero = DecodeBudgetContext::new(norito::DecodeLimits::new(
        usize::MAX,
        usize::MAX,
        usize::MAX,
        0,
        usize::MAX,
    ));
    let mut original = None;
    let result = zero
        .with(|| {
            ParameterIngressAttemptError::observe(|| {
                read(&source, &pool).inspect_err(|error| {
                    if let ParameterIngressError::Codec(norito::Error::ScopedDecodeResource(
                        scope,
                    )) = error
                    {
                        original = Some(scope.clone());
                    }
                })
            })
        })
        .unwrap_err();
    let ParameterIngressAttemptError::Codec(attempt) = result else {
        panic!("original codec attempt required")
    };
    assert_eq!(attempt.kind(), DecodeAttemptErrorKind::EnclosingLimit);
    let error = attempt.into_local_refusal().unwrap().into_error();
    let norito::Error::ScopedDecodeResource(scope) = &error else {
        panic!("original scope required")
    };
    assert_eq!(Some(scope), original.as_ref());
    assert_eq!(pool.reserved_bytes(), 0);
    let replay = ParameterIngressAttemptError::observe::<()>(|| Err(error.into())).unwrap_err();
    let ParameterIngressAttemptError::Codec(replay) = replay else {
        panic!("replay must be classified")
    };
    assert_eq!(replay.kind(), DecodeAttemptErrorKind::Invalid);
    assert!(replay.local_refusal().is_none());
    drop(read(&source, &pool).unwrap());
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn prepared_update_is_bound_to_exact_current_graph_and_retains_full_projection() {
    use iroha_data_model::parameter::{Parameter, SumeragiParameter};
    let pool = AllocationBudget::new(1 << 24);
    let mut current = read(&source(), &pool).unwrap();
    let original = current.clone();
    let update = Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(17));
    let stale = current.prepare_parameter(&update, &pool).unwrap();
    let accepted = current.prepare_parameter(&update, &pool).unwrap();
    assert_eq!(current.get(), original.get());
    assert_eq!(
        allocations_during(|| accepted.install(&mut current, &pool).unwrap()),
        0
    );
    assert_eq!(current.sumeragi.max_clock_drift_ms, 17);
    assert_eq!(current.root_fees(), original.root_fees());
    assert_eq!(current.staking_policy(), original.staking_policy());
    assert_eq!(current.crypto_policy(), original.crypto_policy());
    let published = current.clone();
    assert!(matches!(
        stale.install(&mut current, &pool),
        Err((_, ParameterRetentionError::StalePredecessor))
    ));
    assert!(published.same_version(&current));
    assert!(!original.same_version(&current));
    drop((original, published, current));
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn custom_removal_and_consensus_replacement_keep_mandatory_original_owners() {
    use iroha_data_model::parameter::{CustomParameter, CustomParameterId, Parameter};
    let pool = AllocationBudget::new(1 << 24);
    let mut current = read(&source(), &pool).unwrap();
    let id: CustomParameterId = "explicit_component_probe".parse().unwrap();
    let parameter = Parameter::Custom(CustomParameter::new(id.clone(), Json::new(7u64)));
    current
        .prepare_parameter(&parameter, &pool)
        .unwrap()
        .install(&mut current, &pool)
        .unwrap();
    assert!(current.custom().contains_key(&id));
    let original = current.clone();
    current
        .prepare_custom_removal(&id, &pool)
        .unwrap()
        .install(&mut current, &pool)
        .unwrap();
    assert!(!current.custom().contains_key(&id));
    assert!(original.custom().contains_key(&id));
    for id in [
        RootFeePolicy::parameter_id(),
        PublicLaneStakingPolicy::parameter_id(),
        CryptoAdmissionPolicyV1::parameter_id(),
    ] {
        let held = pool.reserved_bytes();
        assert!(matches!(
            current.prepare_custom_removal(&id, &pool),
            Err(ParameterIngressError::Retention(
                ParameterRetentionError::MissingInitializedPolicy
            ))
        ));
        assert_eq!(pool.reserved_bytes(), held);
    }
    let mut consensus = current.sumeragi().clone();
    consensus.max_clock_drift_ms = 19;
    consensus.key_allowed_algorithms.reverse();
    let changed = current.prepare_sumeragi(&consensus, &pool).unwrap();
    assert_eq!(changed.successor().sumeragi(), &consensus);
    changed.install(&mut current, &pool).unwrap();
    assert_eq!(current.sumeragi(), &consensus);
    assert_eq!(current.staking_policy(), original.staking_policy());
    drop((current, original));
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn update_refusal_preserves_current_and_original_pool_then_retries() {
    use iroha_data_model::parameter::{Parameter, SumeragiParameter};
    let pool = AllocationBudget::new(1 << 24);
    let mut current = read(&source(), &pool).unwrap();
    let original = current.clone();
    let held = pool.reserved_bytes();
    pool.set_limit_bytes(held);
    let parameter = Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(23));
    assert_eq!(
        allocations_during(|| assert!(current.prepare_parameter(&parameter, &pool).is_err())),
        0
    );
    assert!(original.same_version(&current));
    assert_eq!(pool.reserved_bytes(), held);
    let foreign = AllocationBudget::new(1 << 24);
    assert_eq!(
        allocations_during(|| assert!(matches!(
            current.prepare_parameter(&parameter, &foreign),
            Err(ParameterIngressError::Retention(
                ParameterRetentionError::ForeignPool
            ))
        ))),
        0
    );
    assert_eq!(foreign.reserved_bytes(), 0);
    pool.set_limit_bytes(1 << 24);
    current
        .prepare_parameter(&parameter, &pool)
        .unwrap()
        .install(&mut current, &pool)
        .unwrap();
    assert_eq!(current.sumeragi.max_clock_drift_ms, 23);
    assert_ne!(original.sumeragi.max_clock_drift_ms, 23);
    drop((current, original));
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn authenticated_scalar_cadence_and_parameter_have_one_prepared_version() {
    use iroha_data_model::parameter::{Parameter, SumeragiParameter};
    let pool = AllocationBudget::new(1 << 24);
    let mut owner = RetainedStateParameters::try_protocol_default(&pool).unwrap();
    let original = owner.clone();
    let cadence = std::num::NonZeroU64::new(27).unwrap();
    let parameter = Parameter::Sumeragi(SumeragiParameter::MaxClockDriftMs(81));
    let prepared = owner
        .prepare_parameter_with_cadence(&parameter, Some(cadence), &pool)
        .unwrap();
    assert!(original.same_version(&owner));
    assert_eq!(prepared.successor().sumeragi.block_cadence_ms, cadence);
    assert_eq!(prepared.successor().sumeragi.max_clock_drift_ms, 81);
    assert_eq!(
        allocations_during(|| prepared.install(&mut owner, &pool).unwrap()),
        0
    );
    assert_eq!(owner.sumeragi.block_cadence_ms, cadence);
    assert_eq!(owner.sumeragi.max_clock_drift_ms, 81);
    assert_ne!(
        owner.sumeragi.block_cadence_ms,
        original.sumeragi.block_cadence_ms
    );
    drop(owner);
    drop(original);
    assert_eq!(pool.reserved_bytes(), 0);
}
