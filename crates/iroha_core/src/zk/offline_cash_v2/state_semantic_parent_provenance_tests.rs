use halo2_proofs::halo2curves::{
    ff::PrimeField,
    group::{GroupEncoding as _, prime::PrimeCurveAffine as _},
    pasta::{EpAffine, EqAffine, Fp, Fq},
};

use super::{
    super::{
        state_lineage::{
            OfflineCashEpParentLineageV2, OfflineCashEqParentLineageV2,
            OfflineCashStateAbiFieldsV2, OfflineCashStateOperationV2,
        },
        state_recursive_fold::{
            STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2, STATE_RECURSIVE_FOLD_ACCUMULATOR_ROUNDS_V2,
        },
    },
    *,
};

fn state_fields(parity: OfflineCashHalo2ParityV2) -> OfflineCashStateAbiFieldsV2 {
    OfflineCashStateAbiFieldsV2 {
        operation: OfflineCashStateOperationV2::SendSplit,
        release_digest: [0x11; 32],
        protocol_digest: offline_cash_halo2_protocol_source_identity_v2(
            parity,
            OfflineCashHalo2CircuitRoleV2::State,
        )
        .digest(),
        semantic_digest: [0x22; 32],
        context_digest: [0x33; 32],
        request_digest: [0x44; 32],
        parent_0: [0x55; 32],
        parent_1: [0x66; 32],
        result: [0x77; 32],
        link: [0x88; 32],
        transition_digest: [0x99; 32],
        amount: 123,
        scale: 2,
    }
}

fn eq_lineage() -> OfflineCashEqParentLineageV2 {
    OfflineCashEqParentLineageV2::live(
        [Fp::from(7); STATE_RECURSIVE_FOLD_ACCUMULATOR_ROUNDS_V2],
        EqAffine::generator(),
    )
    .expect("fixture Eq lineage is canonical")
}

fn ep_lineage() -> OfflineCashEpParentLineageV2 {
    OfflineCashEpParentLineageV2::live(
        [Fq::from(11); STATE_RECURSIVE_FOLD_ACCUMULATOR_ROUNDS_V2],
        EpAffine::generator(),
    )
    .expect("fixture Ep lineage is canonical")
}

fn parent_instances() -> (
    OfflineCashStatePublicInstancesV2,
    OfflineCashStatePublicInstancesV2,
) {
    (
        OfflineCashStatePublicInstancesV2::eq(
            state_fields(OfflineCashHalo2ParityV2::Eq),
            &eq_lineage(),
        )
        .expect("fixture Eq instances are canonical"),
        OfflineCashStatePublicInstancesV2::ep(
            state_fields(OfflineCashHalo2ParityV2::Ep),
            &ep_lineage(),
        )
        .expect("fixture Ep instances are canonical"),
    )
}

fn parent_pair() -> UnverifiedOfflineCashStateSemanticParentPairV2 {
    let (eq, ep) = parent_instances();
    UnverifiedOfflineCashStateSemanticParentPairV2::from_eq_then_ep(eq, ep)
        .expect("fixture is one paired semantic statement")
}

fn accumulator_bytes(
    parity: StateRecursiveFoldParityV2,
    scalar: u64,
) -> [u8; STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2] {
    let mut bytes = [0_u8; STATE_RECURSIVE_FOLD_ACCUMULATOR_BYTES_V2];
    match parity {
        StateRecursiveFoldParityV2::Eq => {
            let scalar = Fp::from(scalar).to_repr();
            for round in 0..STATE_RECURSIVE_FOLD_ACCUMULATOR_ROUNDS_V2 {
                bytes[round * 32..(round + 1) * 32].copy_from_slice(scalar.as_ref());
            }
            bytes[STATE_RECURSIVE_FOLD_ACCUMULATOR_ROUNDS_V2 * 32..]
                .copy_from_slice(EqAffine::generator().to_bytes().as_ref());
        }
        StateRecursiveFoldParityV2::Ep => {
            let scalar = Fq::from(scalar).to_repr();
            for round in 0..STATE_RECURSIVE_FOLD_ACCUMULATOR_ROUNDS_V2 {
                bytes[round * 32..(round + 1) * 32].copy_from_slice(scalar.as_ref());
            }
            bytes[STATE_RECURSIVE_FOLD_ACCUMULATOR_ROUNDS_V2 * 32..]
                .copy_from_slice(EpAffine::generator().to_bytes().as_ref());
        }
    }
    bytes
}

fn accumulator(parity: StateRecursiveFoldParityV2, scalar: u64) -> CanonicalStateAccumulatorV2 {
    CanonicalStateAccumulatorV2::decode(parity, &accumulator_bytes(parity, scalar))
        .expect("fixture accumulator is canonical")
}

fn parent_0_inputs() -> ProvenanceBoundStateParent0InputsV2 {
    VerifiedOfflineCashStateSemanticParentHandoffV2::from_test_verified_parts_v2(
        parent_pair(),
        accumulator(StateRecursiveFoldParityV2::Eq, 17),
        accumulator(StateRecursiveFoldParityV2::Ep, 19),
    )
    .expect("test verifier accepts canonical parity-local currents")
    .into_fold_accumulator_parts_v2()
    .bind_parent_0_v2()
}

fn parent_1_inputs() -> ProvenanceBoundStateParent1InputsV2 {
    VerifiedOfflineCashStateSemanticParentHandoffV2::from_test_verified_parts_v2(
        parent_pair(),
        accumulator(StateRecursiveFoldParityV2::Eq, 23),
        accumulator(StateRecursiveFoldParityV2::Ep, 29),
    )
    .expect("test verifier accepts canonical parity-local currents")
    .into_fold_accumulator_parts_v2()
    .bind_parent_1_v2()
}

#[test]
fn exact_eq_ep_pair_owns_common_statement_and_permitted_parity_values() {
    let (eq, ep) = parent_instances();
    let expected_eq = *eq.words();
    let expected_ep = *ep.words();
    let pair = UnverifiedOfflineCashStateSemanticParentPairV2::from_eq_then_ep(eq, ep)
        .expect("exact Eq/Ep semantic statement joins");

    assert_eq!(pair.eq_instances().words(), &expected_eq);
    assert_eq!(pair.ep_instances().words(), &expected_ep);
    assert_ne!(
        expected_eq[STATE_PARITY_WORD_V2],
        expected_ep[STATE_PARITY_WORD_V2]
    );
    assert_ne!(
        &expected_eq[OFFLINE_CASH_STATE_PROTOCOL_WORD_START_V2..STATE_PROTOCOL_WORD_END_V2],
        &expected_ep[OFFLINE_CASH_STATE_PROTOCOL_WORD_START_V2..STATE_PROTOCOL_WORD_END_V2]
    );
    assert_ne!(
        &expected_eq[OFFLINE_CASH_STATE_PARENT_LINEAGE_WORD_START_V2..],
        &expected_ep[OFFLINE_CASH_STATE_PARENT_LINEAGE_WORD_START_V2..]
    );

    for word in 0..expected_eq.len() {
        if word == STATE_PARITY_WORD_V2
            || (OFFLINE_CASH_STATE_PROTOCOL_WORD_START_V2..STATE_PROTOCOL_WORD_END_V2)
                .contains(&word)
            || word >= OFFLINE_CASH_STATE_PARENT_LINEAGE_WORD_START_V2
        {
            continue;
        }
        assert_eq!(expected_eq[word], expected_ep[word], "common word {word}");
    }
}

#[test]
fn parity_order_and_exact_state_source_identities_fail_closed() {
    let (eq, ep) = parent_instances();
    assert!(matches!(
        UnverifiedOfflineCashStateSemanticParentPairV2::from_eq_then_ep(ep, eq),
        Err(OfflineCashStateSemanticParentProvenanceErrorV2::EqInstanceParityMismatch)
    ));

    let mut wrong_eq_fields = state_fields(OfflineCashHalo2ParityV2::Eq);
    wrong_eq_fields.protocol_digest[0] ^= 1;
    if wrong_eq_fields.protocol_digest == [0; 32] {
        wrong_eq_fields.protocol_digest[0] = 2;
    }
    let wrong_eq = OfflineCashStatePublicInstancesV2::eq(wrong_eq_fields, &eq_lineage())
        .expect("source identity is enforced by the pair, not the ABI codec");
    let ep = OfflineCashStatePublicInstancesV2::ep(
        state_fields(OfflineCashHalo2ParityV2::Ep),
        &ep_lineage(),
    )
    .expect("fixture Ep instances are canonical");
    assert!(matches!(
        UnverifiedOfflineCashStateSemanticParentPairV2::from_eq_then_ep(wrong_eq, ep),
        Err(OfflineCashStateSemanticParentProvenanceErrorV2::EqStateProtocolSourceIdentityMismatch)
    ));

    let eq = OfflineCashStatePublicInstancesV2::eq(
        state_fields(OfflineCashHalo2ParityV2::Eq),
        &eq_lineage(),
    )
    .expect("fixture Eq instances are canonical");
    let mut wrong_ep_fields = state_fields(OfflineCashHalo2ParityV2::Ep);
    wrong_ep_fields.protocol_digest[0] ^= 1;
    if wrong_ep_fields.protocol_digest == [0; 32] {
        wrong_ep_fields.protocol_digest[0] = 2;
    }
    let wrong_ep = OfflineCashStatePublicInstancesV2::ep(wrong_ep_fields, &ep_lineage())
        .expect("source identity is enforced by the pair, not the ABI codec");
    assert!(matches!(
        UnverifiedOfflineCashStateSemanticParentPairV2::from_eq_then_ep(eq, wrong_ep),
        Err(OfflineCashStateSemanticParentProvenanceErrorV2::EpStateProtocolSourceIdentityMismatch)
    ));
}

#[test]
fn every_mutable_common_statement_region_is_compared() {
    let base = state_fields(OfflineCashHalo2ParityV2::Ep);
    let cases = [
        (
            4,
            OfflineCashStateAbiFieldsV2 {
                operation: OfflineCashStateOperationV2::ReceiveFold,
                ..base
            },
        ),
        (
            8,
            OfflineCashStateAbiFieldsV2 {
                release_digest: [0xa8; 32],
                ..base
            },
        ),
        (
            24,
            OfflineCashStateAbiFieldsV2 {
                semantic_digest: [0xa4; 32],
                ..base
            },
        ),
        (
            32,
            OfflineCashStateAbiFieldsV2 {
                context_digest: [0xa3; 32],
                ..base
            },
        ),
        (
            40,
            OfflineCashStateAbiFieldsV2 {
                request_digest: [0xa0; 32],
                ..base
            },
        ),
        (
            48,
            OfflineCashStateAbiFieldsV2 {
                parent_0: [0xb0; 32],
                ..base
            },
        ),
        (
            56,
            OfflineCashStateAbiFieldsV2 {
                parent_1: [0xb1; 32],
                ..base
            },
        ),
        (
            64,
            OfflineCashStateAbiFieldsV2 {
                result: [0xc0; 32],
                ..base
            },
        ),
        (
            72,
            OfflineCashStateAbiFieldsV2 {
                link: [0xd0; 32],
                ..base
            },
        ),
        (
            80,
            OfflineCashStateAbiFieldsV2 {
                transition_digest: [0xe0; 32],
                ..base
            },
        ),
        (
            88,
            OfflineCashStateAbiFieldsV2 {
                amount: 124,
                ..base
            },
        ),
        (92, OfflineCashStateAbiFieldsV2 { scale: 3, ..base }),
    ];

    for (expected_word, ep_fields) in cases {
        let eq = OfflineCashStatePublicInstancesV2::eq(
            state_fields(OfflineCashHalo2ParityV2::Eq),
            &eq_lineage(),
        )
        .expect("fixture Eq instances are canonical");
        let ep = OfflineCashStatePublicInstancesV2::ep(ep_fields, &ep_lineage())
            .expect("mutated Ep instances remain structurally canonical");
        assert!(matches!(
            UnverifiedOfflineCashStateSemanticParentPairV2::from_eq_then_ep(eq, ep),
            Err(OfflineCashStateSemanticParentProvenanceErrorV2::CommonStatementWordMismatch {
                word
            }) if word == expected_word
        ));
    }
}

#[test]
fn test_verified_handoff_derives_priors_and_binds_one_immutable_position() {
    let expected_eq_prior = eq_lineage().encode();
    let expected_ep_prior = ep_lineage().encode();
    let expected_eq_current = accumulator_bytes(StateRecursiveFoldParityV2::Eq, 17);
    let expected_ep_current = accumulator_bytes(StateRecursiveFoldParityV2::Ep, 19);
    let parent_0 = parent_0_inputs();

    assert_eq!(parent_0.eq_current().as_bytes(), &expected_eq_current);
    assert_eq!(parent_0.ep_current().as_bytes(), &expected_ep_current);
    assert_eq!(parent_0.eq_prior().as_bytes(), &expected_eq_prior);
    assert_eq!(parent_0.ep_prior().as_bytes(), &expected_ep_prior);
    assert_eq!(
        parent_0.eq_current().parity(),
        StateRecursiveFoldParityV2::Eq
    );
    assert_eq!(parent_0.eq_prior().parity(), StateRecursiveFoldParityV2::Eq);
    assert_eq!(
        parent_0.ep_current().parity(),
        StateRecursiveFoldParityV2::Ep
    );
    assert_eq!(parent_0.ep_prior().parity(), StateRecursiveFoldParityV2::Ep);
    assert_eq!(
        parent_0
            .provenance_seal()
            .provenance()
            .eq_instances()
            .eq_parent_lineage()
            .expect("sealed Eq lineage remains canonical")
            .encode(),
        expected_eq_prior
    );

    let parent_1 = parent_1_inputs();
    fn require_parent_0(_: &ProvenanceBoundStateParent0InputsV2) {}
    fn require_parent_1(_: &ProvenanceBoundStateParent1InputsV2) {}
    require_parent_0(&parent_0);
    require_parent_1(&parent_1);
    assert_eq!(
        parent_1.eq_current().parity(),
        StateRecursiveFoldParityV2::Eq
    );
    assert_eq!(
        parent_1.ep_current().parity(),
        StateRecursiveFoldParityV2::Ep
    );
    assert_eq!(
        parent_1
            .provenance_seal()
            .provenance()
            .ep_instances()
            .ep_parent_lineage()
            .expect("sealed Ep lineage remains canonical")
            .encode(),
        expected_ep_prior
    );
}

#[test]
fn current_accumulator_parity_substitutions_fail_closed() {
    assert!(matches!(
        VerifiedOfflineCashStateSemanticParentHandoffV2::from_test_verified_parts_v2(
            parent_pair(),
            accumulator(StateRecursiveFoldParityV2::Ep, 17),
            accumulator(StateRecursiveFoldParityV2::Ep, 19),
        ),
        Err(OfflineCashStateSemanticParentProvenanceErrorV2::CurrentAccumulatorParityMismatch)
    ));
    assert!(matches!(
        VerifiedOfflineCashStateSemanticParentHandoffV2::from_test_verified_parts_v2(
            parent_pair(),
            accumulator(StateRecursiveFoldParityV2::Eq, 17),
            accumulator(StateRecursiveFoldParityV2::Eq, 19),
        ),
        Err(OfflineCashStateSemanticParentProvenanceErrorV2::CurrentAccumulatorParityMismatch)
    ));
}

#[test]
fn production_boundary_is_uninhabited_and_unverified_input_fails_closed() {
    let _verifier_signature: fn(
        UnverifiedOfflineCashStateSemanticParentPairV2,
        CanonicalStateAccumulatorV2,
        CanonicalStateAccumulatorV2,
        OfflineCashStateSemanticParentProofVerifierAuthorityV2,
    ) -> Result<
        VerifiedOfflineCashStateSemanticParentHandoffV2,
        OfflineCashStateSemanticParentProvenanceErrorV2,
    > = verify_offline_cash_state_semantic_parent_for_fold_v2;

    assert!(matches!(
        fail_closed_offline_cash_state_semantic_parent_boundary_v2(parent_pair()),
        Err(OfflineCashStateSemanticParentProvenanceErrorV2::VerificationUnavailable)
    ));
}

#[test]
fn source_guards_keep_owners_move_only_private_and_non_authorizing() {
    let source = include_str!("state_semantic_parent_provenance.rs");
    let parent = include_str!("../offline_cash_v2.rs");
    assert_eq!(
        parent
            .lines()
            .filter(|line| line.trim() == "mod state_semantic_parent_provenance;")
            .count(),
        1
    );
    assert!(parent.contains("#[path = \"offline_cash_v2/state_semantic_parent_provenance.rs\"]"));
    assert!(!parent.contains("pub mod state_semantic_parent_provenance"));
    assert!(source.contains("0..STATE_PARITY_WORD_V2"));
    assert!(source.contains("STATE_PARITY_WORD_V2 + 1..OFFLINE_CASH_STATE_PROTOCOL_WORD_START_V2"));
    assert!(
        source.contains(
            "STATE_PROTOCOL_WORD_END_V2..OFFLINE_CASH_STATE_PARENT_LINEAGE_WORD_START_V2"
        )
    );
    assert!(source.contains("OfflineCashHalo2CircuitRoleV2::State"));
    assert!(source.contains("enum OfflineCashStateSemanticParentProofVerifierAuthorityV2 {}"));
    assert!(source.contains("match authority {}"));
    assert!(source.contains("#[cfg(test)]\n    pub(super) fn from_test_verified_parts_v2"));
    assert!(!source.contains("impl Clone for UnverifiedOfflineCashStateSemanticParentPairV2"));
    assert!(!source.contains("impl Clone for VerifiedOfflineCashStateSemanticParentHandoffV2"));
    assert!(!source.contains("impl Clone for ProvenanceBoundStateParent0InputsV2"));
    assert!(!source.contains("impl Clone for ProvenanceBoundStateParent1InputsV2"));
    assert!(!source.contains("into_parts_v2"));
    assert!(!source.contains("unsafe"));
    assert!(!source.contains("impl Circuit for"));
    assert!(!source.contains("verify_proof("));
    assert!(!source.contains("create_proof("));
    assert!(
        !source.contains(
            "OFFLINE_CASH_STATE_SEMANTIC_PARENT_PROOF_VERIFIER_AVAILABLE_V2: bool = true"
        )
    );
    assert!(
        !source
            .contains("OFFLINE_CASH_STATE_SEMANTIC_PARENT_ARTIFACTS_AUTHENTICATED_V2: bool = true")
    );
    assert!(
        !source.contains("OFFLINE_CASH_STATE_SEMANTIC_PARENT_PRODUCTION_AVAILABLE_V2: bool = true")
    );
}

#[test]
fn structural_contract_is_true_while_every_authority_gate_is_false() {
    assert!(OFFLINE_CASH_STATE_SEMANTIC_PARENT_PROVENANCE_CONTRACT_IMPLEMENTED_V2);
    assert!(!OFFLINE_CASH_STATE_SEMANTIC_PARENT_PROOF_VERIFIER_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_STATE_SEMANTIC_PARENT_WIRE_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_STATE_SEMANTIC_PARENT_ARTIFACTS_AUTHENTICATED_V2);
    assert!(!OFFLINE_CASH_STATE_SEMANTIC_PARENT_PRODUCTION_AVAILABLE_V2);
}
