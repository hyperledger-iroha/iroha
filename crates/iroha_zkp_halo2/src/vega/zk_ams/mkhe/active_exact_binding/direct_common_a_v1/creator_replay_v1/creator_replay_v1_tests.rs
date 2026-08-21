use super::*;
use crate::vega::zk_ams::mkhe::{
    active_exact_binding::PersistentDirectRelationV1,
    direct_collective_eval_ceremony::ZkAmsMkheDirectEvaluatedKeyTargetV1,
    direct_rkg_ephemeral_membership::tests::creator_state_fixture, manifest::release_profile_v1,
};

#[test]
fn creator_common_a_authority_is_consumed_h0_then_h1_then_selector() {
    let (roster, bindings, _) = creator_state_fixture(b"common-a-creator-typestate");
    let context = ZkAmsMkheDirectCeremonyContextV1::from_verified_binding_set(
        &roster,
        &bindings,
        ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization,
        0,
    )
    .expect("accepted RKG1 context");
    let profile = release_profile_v1();
    let mut workspace = vec![0_u64; profile.ring_degree];

    let mut h0 = prepare_direct_common_a_creator_h0_v1(&roster, &bindings, context)
        .unwrap()
        .begin_h0_v1()
        .unwrap();
    for _ in 0..profile.moduli.len() {
        h0.derive_next_limb_into(&mut workspace).unwrap();
    }
    let mut h1 = h0.finish_h0_v1().unwrap().begin_h1_v1().unwrap();
    for _ in 0..profile.moduli.len() {
        h1.derive_next_limb_into(&mut workspace).unwrap();
    }
    let completed = h1.finish_h1_v1().unwrap();
    let mut statement_digest = [0_u8; 32];
    completed
        .write_statement_digest_v1(context, &mut statement_digest)
        .unwrap();
    assert_ne!(statement_digest, [0; 32]);

    let selector = consume_completed_creator_authority_v1(
        completed,
        context.initial_round_digest(),
        [0x21; 32],
        [0x22; 32],
    )
    .unwrap();
    assert_eq!(selector.relation, PersistentDirectRelationV1::RkgRoundOne);
}

#[test]
fn creator_typestate_has_no_borrowed_replay_or_raw_digest_surface() {
    let source = include_str!("../creator_replay_v1.rs");
    for transition in ["begin_h0_v1", "finish_h0_v1", "begin_h1_v1", "finish_h1_v1"] {
        let start = source.find(transition).unwrap();
        assert!(source[start..].find("self,").unwrap() < 180);
    }
    assert!(!source.contains("fn statement_digest("));
    assert!(source.contains("fn write_statement_digest_v1("));
    assert!(source.contains("consume_completed_creator_authority_v1("));
    assert!(!source.contains("#[derive(Clone"));

    let verifier = include_str!("../../direct_common_a_v1.rs");
    assert!(verifier.contains("pub(super) fn begin("));
    assert!(verifier.contains("capability: &super::VerifiedPersistentWitnessDirectRelationUseV1"));
    assert!(verifier.contains("#[cfg(test)]\npub(super) fn mint_rkg_round_one_selector_v1("));
    let facade = include_str!("../../../active_exact_binding.rs");
    assert!(facade.contains("#[cfg(test)]\npub(super) fn mint_rkg_round_one_selector_v1("));
}

#[test]
fn creator_typestate_delta_and_tests_stay_bounded() {
    let production = include_str!("../creator_replay_v1.rs");
    let tests = include_str!("creator_replay_v1_tests.rs");
    assert!(production.lines().count() <= 180 && production.len() <= 24 * 1024);
    assert!(tests.lines().count() <= 500 && tests.len() <= 24 * 1024);
}
