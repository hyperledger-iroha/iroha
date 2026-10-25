//! Integration of real direct leaf parsers with the complete immutable graph.

use super::*;
use iroha_data_model::{
    block::consensus::{PublicLaneStakingPolicy, RootFeePolicy},
    crypto_admission_policy::CryptoAdmissionPolicyV1,
    fraud::FraudAdmissionPolicyV1,
    zk_validity_policy::ZkValidityPolicyV1,
};
use norito::json;

fn parse<T, E: core::fmt::Debug>(
    source: &str,
    decode: impl FnOnce(&mut json::Parser<'_>) -> Result<T, E>,
) -> T {
    let mut parser = json::Parser::new(source);
    parser.preflight_document().unwrap();
    let value = decode(&mut parser).unwrap();
    parser.finish_document().unwrap();
    value
}
fn parts(source: &StateParameters, pool: &AllocationBudget) -> DecodedParameterParts {
    // This is explicitly test-author input. The production caller instead lends
    // its original admitted bytes to these same decoders; it never serializes.
    let wire = json::to_json(&source.parameters).unwrap();
    let parameters = parse(&wire, |parser| {
        AdmittedParameters::try_decode_json(parser, pool)
    });
    let gas = AdmittedGas::try_from_parameters(parameters.get(), pool).unwrap();
    let payload = |id| {
        parameters
            .get()
            .custom()
            .get(&id)
            .unwrap()
            .payload()
            .get()
            .as_str()
    };
    let fraud = parse(payload(FraudAdmissionPolicyV1::parameter_id()), |parser| {
        AdmittedFraudAdmissionPolicy::try_decode_json(parser, pool)
    });
    let zk = parse(payload(ZkValidityPolicyV1::parameter_id()), |parser| {
        AdmittedZkValidityPolicy::try_decode_json(parser, pool)
    });
    let root_fees = parse(payload(RootFeePolicy::parameter_id()), |parser| {
        AdmittedRootFeePolicy::try_decode_json(parser, pool)
    });
    let crypto = parse(payload(CryptoAdmissionPolicyV1::parameter_id()), |parser| {
        AdmittedCryptoAdmissionPolicy::try_decode_json(parser, pool)
    });
    let staking = parse(payload(PublicLaneStakingPolicy::parameter_id()), |parser| {
        AdmittedPublicLaneStakingPolicy::try_decode_json(parser, pool)
    });
    DecodedParameterParts {
        parameters,
        gas,
        pipeline: source.pipeline,
        fraud,
        zk,
        root_fees: Some(root_fees),
        crypto: Some(crypto),
        staking: Some(staking),
    }
}
fn finish(
    shell: ReservedParameterGraph,
    parts: DecodedParameterParts,
    pool: &AllocationBudget,
) -> RetainedStateParameters {
    shell
        .finish(parts, pool)
        .unwrap_or_else(|(_, _, error)| panic!("original assembly: {error}"))
}

#[test]
fn exact_leaf_addresses_survive_zero_remaining_capacity_assembly_and_last_reader() {
    let source = super::super::tests::authored();
    let pool = AllocationBudget::new(1 << 22);
    let shell = ReservedParameterGraph::try_reserve(&pool).unwrap();
    let parts = parts(&source, &pool);
    let engine = parts.fraud.get().attesters[0].engine_id.as_ptr();
    let key_name = parts
        .zk
        .get()
        .proofs
        .kaigi_authorization_vk
        .as_ref()
        .unwrap()
        .name
        .as_ptr();
    let algorithms = parts
        .crypto
        .as_ref()
        .unwrap()
        .get()
        .allowed_algorithms()
        .as_ptr();
    let before = pool.reserved_bytes();
    pool.set_limit_bytes(0);
    let retained = finish(shell, parts, &pool);
    assert_eq!(retained.get(), &source);
    assert_eq!(
        retained.fraud_policy().attesters[0].engine_id.as_ptr(),
        engine
    );
    assert_eq!(
        retained
            .zk_policy()
            .proofs
            .kaigi_authorization_vk
            .as_ref()
            .unwrap()
            .name
            .as_ptr(),
        key_name
    );
    assert_eq!(
        retained
            .crypto_policy()
            .unwrap()
            .allowed_algorithms()
            .as_ptr(),
        algorithms
    );
    assert_eq!(pool.reserved_bytes(), before);
    let reader = retained.clone();
    assert!(retained.same_version(&reader));
    drop(retained);
    assert_eq!(pool.reserved_bytes(), before);
    drop(reader);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn foreign_nested_leaf_returns_every_owner_and_shell_for_exact_retry() {
    let source = super::super::tests::authored();
    let pool = AllocationBudget::new(1 << 22);
    let foreign = AllocationBudget::new(1 << 22);
    let shell = ReservedParameterGraph::try_reserve(&pool).unwrap();
    let mut leaves = parts(&source, &pool);
    let original = leaves.zk;
    let encoded = json::to_json(source.zk_policy()).unwrap();
    leaves.zk = parse(&encoded, |parser| {
        AdmittedZkValidityPolicy::try_decode_json(parser, &foreign)
    });
    let original_bytes = pool.reserved_bytes();
    let foreign_bytes = foreign.reserved_bytes();
    let (shell, mut leaves, error) = shell
        .finish(leaves, &pool)
        .err()
        .expect("foreign original owner refused");
    assert!(matches!(error, ParameterRetentionError::ForeignPool));
    assert_eq!(pool.reserved_bytes(), original_bytes);
    assert_eq!(foreign.reserved_bytes(), foreign_bytes);
    let rejected = core::mem::replace(&mut leaves.zk, original);
    drop(rejected);
    assert_eq!(foreign.reserved_bytes(), 0);
    let retained = finish(shell, leaves, &pool);
    assert_eq!(retained.get(), &source);
    drop(retained);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn first_shell_refusal_precedes_leaf_parsing_and_author_source_is_unchanged() {
    let source = super::super::tests::authored();
    let pool = AllocationBudget::new(0);
    assert!(matches!(
        ReservedParameterGraph::try_reserve(&pool),
        Err(ParameterRetentionError::Admission(_))
    ));
    assert_eq!(pool.reserved_bytes(), 0);
    pool.set_limit_bytes(1 << 22);
    let shell = ReservedParameterGraph::try_reserve(&pool).unwrap();
    let retained = finish(shell, parts(&source, &pool), &pool);
    assert_eq!(retained.get(), &source);
    drop(retained);
    assert_eq!(pool.reserved_bytes(), 0);
}

#[test]
fn directly_decoded_successor_refuses_an_applied_sibling_without_touching_it() {
    let source = super::super::tests::authored();
    let pool = AllocationBudget::new(1 << 23);
    let decode = || {
        let shell = ReservedParameterGraph::try_reserve(&pool).unwrap();
        finish(shell, parts(&source, &pool), &pool)
    };
    let mut current = decode();
    let first = current
        .prepare_admitted(decode(), &pool)
        .unwrap_or_else(|(_, e)| panic!("{e}"));
    let stale = current
        .prepare_admitted(decode(), &pool)
        .unwrap_or_else(|(_, e)| panic!("{e}"));
    first
        .install(&mut current, &pool)
        .unwrap_or_else(|(_, e)| panic!("{e}"));
    let installed = current.clone();
    let (stale, error) = stale.install(&mut current, &pool).unwrap_err();
    assert!(matches!(error, ParameterRetentionError::StalePredecessor));
    assert!(current.same_version(&installed));
    drop(stale);
    drop(installed);
    drop(current);
    assert_eq!(pool.reserved_bytes(), 0);
}
