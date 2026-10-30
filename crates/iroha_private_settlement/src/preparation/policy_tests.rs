//! Selected committed limits constrain original preparation and exact material custody.
//! Native QCs authenticate synthetic World rows; these controls do not generate STARK proofs.

use super::*;
use crate::{cold_context_tests, test_support::client_with_base_url};
use iroha_data_model::{
    privacy::PrivateSettlementValidityLimitsV1,
    sumeragi_finality::{
        VerifiedSumeragiBlock, test_fixtures::NativeFinalityFixture, world_state_value_hash_v1,
    },
};

fn selected_fixture() -> (
    PrivateSettlementContextsProofV1,
    OneCallPrivateSettlementRequest,
    VerifiedSumeragiBlock,
) {
    let mut chain = NativeFinalityFixture::start("private-preparation-committed-limits");
    let mut client =
        client_with_base_url(&"http://selected.invalid/".parse().unwrap()).to_builder();
    client.network_id = chain.network_id();
    client.chain = chain.chain_id().parse().unwrap();
    let client = client.build().unwrap();
    let (mut proof, intent) = cold_context_tests::context(&client);
    for context in &mut proof.contexts {
        context
            .privacy_policy
            .current_limits
            .private_settlement
            .max_carrier_bytes = 2 * 1024 * 1024;
        let hash = world_state_value_hash_v1(&context.privacy_policy).unwrap();
        context
            .pool
            .world
            .entries
            .iter_mut()
            .find(|entry| entry.field_id == "world.privacy_consensus_policy")
            .unwrap()
            .value_hash = hash;
    }
    let request = cold_context_tests::request(&client, &proof, &intent);
    let mut verifier = chain.verifier();
    let block = chain.block_with_submitted_work(chain.next_header());
    let proof_of_execution =
        chain.certify_with_world_root(block, proof.contexts[0].pool.world.root().unwrap());
    let block = verifier.verify(&proof_of_execution).unwrap();
    (proof, request, block)
}

#[test]
fn authenticated_selected_contract_caps_carrier_and_checks_exact_retention() {
    let (proof, mut request, block) = selected_fixture();
    let member = &proof.contexts[0];
    let selected = member
        .verify(
            request.intent().network_id,
            request.0.schema,
            PrivateSettlementPoolKeyV1 {
                route: request.intent().legs[0].route,
                pool_id: request.intent().legs[0].pool_id,
            },
            &block,
        )
        .unwrap();
    selected.verify_intent(request.intent(), 0).unwrap();
    assert_eq!(
        validate_selected_request(&request, &selected).unwrap(),
        2 * 1024 * 1024
    );
    let original = Arc::get_mut(&mut request.0).unwrap();
    original.max_carrier_bytes = 1024 * 1024;
    assert_eq!(
        validate_selected_request(&request, &selected).unwrap(),
        1024 * 1024
    );
    Arc::get_mut(&mut request.0).unwrap().retention_height -= 1;
    assert!(matches!(
        validate_selected_request(&request, &selected),
        Err(OneCallPrivateSettlementError::Binding)
    ));
}

#[test]
fn exact_prepared_material_bounds_cover_full_capsule_and_auditor_floor() {
    let artifact = crate::artifacts::tests::fixture();
    let mut limits = PrivateSettlementValidityLimitsV1::protocol_default();
    limits.max_proof_bytes = artifact.proof.len() as u64;
    limits.max_capsule_bytes = norito::canonical_frame_len(&artifact.capsule).unwrap() as u64;
    limits.min_auditor_approvals = u16::from(artifact.policy.body.min_approvals);
    validate_material_limits(&limits, &artifact).unwrap();
    for changed in [
        PrivateSettlementValidityLimitsV1 {
            max_proof_bytes: limits.max_proof_bytes - 1,
            ..limits
        },
        PrivateSettlementValidityLimitsV1 {
            max_capsule_bytes: limits.max_capsule_bytes - 1,
            ..limits
        },
        PrivateSettlementValidityLimitsV1 {
            capsule_padding_classes: 0,
            ..limits
        },
        PrivateSettlementValidityLimitsV1 {
            min_auditor_approvals: limits.min_auditor_approvals + 1,
            ..limits
        },
    ] {
        assert!(matches!(
            validate_material_limits(&changed, &artifact),
            Err(OneCallPrivateSettlementError::Binding)
        ));
    }
}
