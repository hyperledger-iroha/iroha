use super::*;

fn bounded_source_section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    assert_eq!(source.matches(start).count(), 1, "non-unique start anchor");
    assert_eq!(source.matches(end).count(), 1, "non-unique end anchor");
    source
        .split_once(start)
        .and_then(|(_, tail)| tail.split_once(end).map(|(section, _)| section))
        .expect("bounded source section")
}

fn assert_unique_ordered(section: &str, snippets: &[&str]) {
    let mut prior_end = 0;
    for &snippet in snippets {
        let mut offsets = section.match_indices(snippet).map(|(offset, _)| offset);
        let offset = offsets.next().expect("required routing snippet");
        assert!(offsets.next().is_none(), "routing snippet is not unique");
        assert!(offset >= prior_end, "routing snippets are out of order");
        prior_end = offset + snippet.len();
    }
}

#[test]
fn provisional_challenge_coordinates_match_the_frozen_framing() {
    let seed: [u8; 32] =
        hex::decode("0904346cfa9f051e59f3bff80220c037685d9e385a9b77dd622ddb391bca0284")
            .unwrap()
            .try_into()
            .unwrap();
    assert_eq!(
        provisional_challenges(seed),
        [2_768_221_376, 3_188_246_095, 2_792_925_365, 3_420_889_530]
    );
    let source = include_str!("rkg_one_semantic_verifier_v1.rs");
    let helper = source
        .split("fn provisional_challenges")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn reconstruct_commitment_first_messages")
                .next()
        })
        .unwrap();
    assert_eq!(helper.matches("u32::from_be_bytes(").count(), 1);
    assert_eq!(helper.matches("keccak256(&frame)").count(), 1);
}

#[test]
fn rkg_one_row_tags_gadget_and_canonical_encoding_are_pinned() {
    let (tags, count) = PersistentDirectRelationV1::RkgRoundOne.rns_row_tags();
    assert_eq!(count, 4);
    assert_eq!(&tags[..count], &[1, 2, 0x84, 0x85]);
    let modulus = RELEASE_MODULI_V1[0];
    assert_eq!(gadget_residue(0, modulus).unwrap(), 1);
    assert_eq!(gadget_residue(1, modulus).unwrap(), 262_143);
    let source = include_str!("rkg_one_semantic_verifier_v1.rs");
    let encoder = source
        .split("fn absorb_residue_limb")
        .nth(1)
        .and_then(|tail| tail.split("fn matrix_limb").next())
        .unwrap();
    assert!(encoder.contains("residue.to_be_bytes()"));
}

#[test]
fn response_slot_decoder_is_exactly_repetition_slot_coefficient_ordered() {
    let mut responses = vec![0_u8; super::super::super::RESPONSE_BYTES_V1];
    for (slot, value) in [(0, -7_i64), (4, 11), (5, -13)] {
        let offset = response_offset(2, slot, 3).unwrap();
        responses[offset..offset + 8].copy_from_slice(&value.to_be_bytes());
    }
    let modulus = RELEASE_MODULI_V1[0];
    let mut decoded = vec![0_u64; 3 * RELEASE_RING_COEFFICIENTS_V1];
    decode_response_slots(&responses, 2, &[0, 4, 5], modulus, &mut decoded).unwrap();
    assert_eq!(decoded[3], signed_mod(-7, modulus));
    assert_eq!(decoded[RELEASE_RING_COEFFICIENTS_V1 + 3], 11);
    assert_eq!(
        decoded[2 * RELEASE_RING_COEFFICIENTS_V1 + 3],
        signed_mod(-13, modulus)
    );
}

#[test]
fn forced_zero_rows_bind_response_slots_four_and_five() {
    let source = include_str!("rkg_one_semantic_verifier_v1.rs");
    let forced_rows = source
        .split("Tags 0x84/0x85")
        .nth(1)
        .and_then(|tail| tail.split("let hashers =").next())
        .expect("forced-zero row reconstruction block");
    assert!(forced_rows.contains("for (row, slot) in [(2, 4), (3, 5)]"));
    assert!(forced_rows.contains("&[slot]"));
    assert!(forced_rows.contains("&response[..RELEASE_RING_COEFFICIENTS_V1]"));
    assert!(!forced_rows.contains("[0_u64"));
    assert!(!forced_rows.contains("zero_row"));
}

#[test]
fn rkg_one_scalar_equations_and_signs_are_pinned() {
    let modulus = 97_u64;
    let h0 = reconstruct_h0_coefficient(70, 5, 7, 13, modulus);
    let h1 = reconstruct_h1_coefficient(80, 11, 17, modulus);
    assert_eq!(h0, 26);
    assert_eq!(h1, 74);
}

#[test]
fn production_response_slots_and_equation_operands_are_pinned() {
    let source = include_str!("rkg_one_semantic_verifier_v1.rs");
    let h0 = bounded_source_section(
        source,
        "fn replay_rkg_one_retained_matrices<P>(",
        "fn reconstruct_rkg_one_rns_first_messages(",
    );
    assert_unique_ordered(
        h0,
        &[
            "decode_response_slots(responses, repetition, &[0, 1, 2], modulus, &mut response)?;",
            "let (s, tail) = response.split_at(RELEASE_RING_COEFFICIENTS_V1);",
            "let (u, e0) = tail.split_at(RELEASE_RING_COEFFICIENTS_V1);",
            "let a_times_u = negacyclic_multiply(a, u, modulus, root)?;",
            "let challenge = u64::from(challenges[repetition]) % modulus;",
            "row[coefficient] = reconstruct_h0_coefficient(\n                    a_times_u[coefficient],\n                    mod_mul(s[coefficient], gadget, modulus),\n                    mod_mul(e0[coefficient], plaintext_multiplier, modulus),\n                    mod_mul(challenge, h0[coefficient], modulus),\n                    modulus,\n                );",
        ],
    );

    let rns = bounded_source_section(
        source,
        "fn reconstruct_rkg_one_rns_first_messages(",
        "fn decode_response_slots(",
    );
    let (h1, _) = rns
        .split_once("// Tags 0x84/0x85")
        .expect("bounded H1 equation section");
    assert_eq!(rns.matches("// Tags 0x84/0x85").count(), 1);
    assert_unique_ordered(
        h1,
        &[
            "decode_response_slots(\n                responses,\n                repetition,\n                &[0, 3],\n                modulus,\n                &mut response[..2 * RELEASE_RING_COEFFICIENTS_V1],\n            )?;",
            "let (s, e1) =\n                response[..2 * RELEASE_RING_COEFFICIENTS_V1].split_at(RELEASE_RING_COEFFICIENTS_V1);",
            "let a_times_s = negacyclic_multiply(a, s, modulus, root)?;",
            "let challenge = u64::from(challenges[repetition]) % modulus;",
            "row[coefficient] = reconstruct_h1_coefficient(\n                    a_times_s[coefficient],\n                    mod_mul(e1[coefficient], plaintext_multiplier, modulus),\n                    mod_mul(challenge, h1[coefficient], modulus),\n                    modulus,\n                );",
        ],
    );
}

#[test]
fn rkg_one_nonzero_errors_use_the_release_t256_plaintext_residue() {
    let modulus = RELEASE_MODULI_V1[0];
    let release_profile = release_profile_v1();
    let plaintext_multiplier = release_profile.plaintext_modulus.residue(modulus);
    let e0 = signed_mod(1, modulus);
    let e1 = signed_mod(-1, modulus);
    let h0 =
        reconstruct_h0_coefficient(0, 0, mod_mul(e0, plaintext_multiplier, modulus), 0, modulus);
    let h1 = reconstruct_h1_coefficient(0, mod_mul(e1, plaintext_multiplier, modulus), 0, modulus);
    assert_eq!(plaintext_multiplier, 1_125_144_406_804_725_708);
    assert_eq!(h0, 1_125_144_406_804_725_708);
    assert_eq!(h1, 27_777_097_801_859_125);
    assert_ne!(h0, 256);
    assert_ne!(h1, modulus - 256);

    let source = include_str!("rkg_one_semantic_verifier_v1.rs");
    assert!(!source.contains("RKG_ONE_PLAINTEXT_MULTIPLIER_V1"));
    assert_eq!(
        source
            .matches("let release_profile = release_profile_v1();")
            .count(),
        2
    );
    assert_eq!(
        source
            .matches("release_profile.plaintext_modulus.residue(modulus)")
            .count(),
        2
    );
    for helper in [
        source
            .split("fn replay_rkg_one_retained_matrices")
            .nth(1)
            .and_then(|tail| {
                tail.split("fn reconstruct_rkg_one_rns_first_messages")
                    .next()
            })
            .unwrap(),
        source
            .split("fn reconstruct_rkg_one_rns_first_messages")
            .nth(1)
            .and_then(|tail| tail.split("fn decode_response_slots").next())
            .unwrap(),
    ] {
        let profile = helper
            .find("let release_profile = release_profile_v1();")
            .unwrap();
        let residue = helper
            .find("release_profile.plaintext_modulus.residue(modulus)")
            .unwrap();
        let equation_loop = helper
            .find("for repetition in 0..CHALLENGE_REPETITIONS_V1")
            .unwrap();
        assert!(profile < residue && residue < equation_loop);
    }
}

#[test]
fn rns_live_payload_ledger_is_checked_without_claiming_rss_certification() {
    validate_rns_live_payload_accounting().unwrap();
    assert_eq!(RKG_ONE_RETAINED_REPLAY_MATRIX_BYTES_V1, 79_691_776);
    assert_eq!(RKG_ONE_H0_REPLAY_LIMB_BYTES_V1, 1_048_576);
    assert_eq!(RKG_ONE_RESPONSE_LIMB_BYTES_V1, 3_145_728);
    assert_eq!(RKG_ONE_ROW_OUTPUT_BYTES_V1, 1_048_576);
    assert_eq!(RKG_ONE_NEGACYCLIC_PRODUCT_BYTES_V1, 1_048_576);
    assert_eq!(RKG_ONE_NEGACYCLIC_NTT_BYTES_V1, 2_097_152);
    assert_eq!(RKG_ONE_CANONICAL_ENCODING_BYTES_V1, 1_048_576);
    assert_eq!(RKG_ONE_PAIRED_REPLAY_SCRATCH_BYTES_V1, 8_192);
    assert_eq!(RKG_ONE_ROW_ZERO_BASE_BYTES_V1, 83_886_080);
    assert_eq!(RKG_ONE_ROW_ZERO_NTT_LIVE_BYTES_V1, 85_983_232);
    assert_eq!(RKG_ONE_ROW_ZERO_HASH_LIVE_BYTES_V1, 87_031_808);
    assert_eq!(RKG_ONE_PAIRED_REPLAY_LIVE_BYTES_V1, 83_894_272);
    assert_eq!(RKG_ONE_RNS_LIVE_PAYLOAD_CEILING_BYTES_V1, 87_031_808);
    assert!(RKG_ONE_RNS_LIVE_PAYLOAD_CEILING_BYTES_V1 < RKG_ONE_RNS_PAYLOAD_LIMIT_BYTES_V1);
}

#[test]
fn semantic_work_and_provider_io_counts_are_pinned() {
    assert_eq!(RKG_ONE_MEMBERSHIP_PROOF_VERIFICATIONS_V1, 48);
    assert_eq!(RKG_ONE_RESPONSE_COMMITMENT_RECONSTRUCTIONS_V1, 192);
    assert_eq!(RKG_ONE_NEGACYCLIC_PRODUCTS_V1, 304);
    assert_eq!(RKG_ONE_FORWARD_NTTS_V1, 608);
    assert_eq!(RKG_ONE_INVERSE_NTTS_V1, 304);
    assert_eq!(RKG_ONE_RNS_ROW_COEFFICIENTS_V1, 79_691_776);
    assert_eq!(RKG_ONE_PROVIDER_READ_CALLS_V1, 9_728);
    assert_eq!(RKG_ONE_PROVIDER_READ_BYTES_V1, 79_691_776);
    let semantic = include_str!("rkg_one_semantic_verifier_v1.rs");
    assert_eq!(semantic.matches("negacyclic_multiply(").count(), 2);
    let arithmetic = include_str!("../../../../mkhe.rs");
    let multiply = arithmetic
        .split("fn negacyclic_multiply(")
        .nth(1)
        .and_then(|tail| tail.split("fn bytes_mod_u64").next())
        .unwrap();
    assert_eq!(multiply.matches("\n    cyclic_ntt(&mut").count(), 2);
    assert_eq!(multiply.matches("\n    inverse_cyclic_ntt(").count(), 1);
    let replay = include_str!("../statement_v1/rkg_one_h0_h1_replay_v1.rs");
    assert!(replay.contains("assert!(READ_CALLS_PER_PAIR_V1 == 9_728);"));
    assert!(replay.contains("assert!(EXACT_PAIR_BYTES_V1 == 79_691_776);"));
}

#[test]
fn semantic_pipeline_order_and_completion_lifetime_are_pinned() {
    let source = include_str!("rkg_one_semantic_verifier_v1.rs");
    let verifier = source
        .split("fn verify_direct_rkg_one_semantic_candidate_v1")
        .nth(1)
        .and_then(|tail| tail.split("fn provisional_challenges").next())
        .expect("candidate verifier body");
    let predecode = verifier
        .find("super::predecode_direct_relation_proof_v1(")
        .unwrap();
    let membership = verifier
        .find("for evidence in &proof.bound_one_membership")
        .unwrap();
    let membership_two = verifier
        .find("for evidence in &proof.bound_two_membership")
        .unwrap();
    let commitment = verifier
        .find("reconstruct_commitment_first_messages(")
        .unwrap();
    let common_begin = verifier.find("DirectCommonAReplayV1::begin").unwrap();
    let public_begin = verifier
        .find("DirectRkgOneH0H1StatementReplayV1::begin")
        .unwrap();
    let replay = verifier.find("replay_rkg_one_retained_matrices(").unwrap();
    let finish = verifier.find("let completed_replays =").unwrap();
    let final_challenge = verifier
        .find("proof.validate_reconstructed_challenge")
        .unwrap();
    let equality = verifier
        .find("if reconstructed_challenges != challenges")
        .unwrap();
    let completion_drop = verifier.find("drop(completed_replays);").unwrap();
    assert!(predecode < membership && membership < membership_two && membership_two < commitment);
    assert!(commitment < common_begin && common_begin < public_begin && public_begin < replay);
    assert!(replay < finish && finish < final_challenge);
    assert!(final_challenge < equality && equality < completion_drop);
    assert!(!verifier.contains("catch_unwind"));
    assert_eq!(
        verifier
            .matches("replay_rkg_one_retained_matrices(")
            .count(),
        1
    );
    let replay_helper = source
        .split("fn replay_rkg_one_retained_matrices")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn reconstruct_rkg_one_rns_first_messages")
                .next()
        })
        .unwrap();
    assert_eq!(
        replay_helper
            .matches("common_a.derive_next_limb_into")
            .count(),
        1
    );
    assert_eq!(
        replay_helper
            .matches("public.replay_next_limb_pair_into")
            .count(),
        1
    );
    assert!(!replay_helper.contains("catch_unwind"));
}

#[test]
fn semantic_reuses_both_fail_closed_poisoned_replays_without_recovery() {
    let semantic = include_str!("rkg_one_semantic_verifier_v1.rs");
    assert!(!semantic.contains("catch_unwind"));
    let common = include_str!("../../direct_common_a_v1.rs");
    assert!(common.contains("self.failed = true;"));
    assert!(common.contains("if result.is_ok()"));
    let common_tests = include_str!("../../direct_common_a_v1_tests.rs");
    assert!(common_tests.contains("stream_is_strictly_ordered_poisoned_and_one_limb_bounded"));
    let paired = include_str!("../statement_v1/rkg_one_h0_h1_replay_v1.rs");
    assert!(paired.contains("self.poisoned = true;"));
    assert!(paired.contains("if result.is_ok()"));
    let paired_tests = include_str!("../statement_v1/rkg_one_h0_h1_replay_v1_tests.rs");
    assert!(paired_tests.contains("workspace_residue_partial_and_extra_states_are_poisoned"));
}

#[test]
fn active_private_wrapper_consumes_capability_before_provider_is_available_to_replay() {
    let source = include_str!("rkg_one_semantic_verifier_v1.rs");
    let wrapper = source
        .split("fn verify_direct_rkg_one_semantic_candidate_v1")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn verify_predecoded_direct_rkg_one_semantic_candidate_v1")
                .next()
        })
        .unwrap();
    let predecode = wrapper
        .find("super::predecode_direct_relation_proof_v1(")
        .unwrap();
    let helper = wrapper
        .find("verify_predecoded_direct_rkg_one_semantic_candidate_v1(")
        .unwrap();
    assert!(predecode < helper);
    assert_eq!(
        wrapper
            .matches("super::predecode_direct_relation_proof_v1(")
            .count(),
        1
    );
    assert!(wrapper.contains("capability: VerifiedPersistentWitnessDirectRelationUseV1"));
    assert!(wrapper.contains("capability,"));
    assert!(!wrapper.contains("replay_next_limb_pair_into"));
    assert!(!wrapper.contains("provider.read"));
}

#[test]
fn exact_repetition_loop_and_replay_argument_shapes_are_pinned() {
    let source = include_str!("rkg_one_semantic_verifier_v1.rs");
    let commitments = source
        .split("fn reconstruct_commitment_first_messages")
        .nth(1)
        .and_then(|tail| tail.split("trait MembershipCommitmentsV1").next())
        .unwrap();
    assert_eq!(
        commitments
            .matches("for repetition in 0..CHALLENGE_REPETITIONS_V1")
            .count(),
        1
    );
    let call = source
        .split("replay_rkg_one_retained_matrices(")
        .nth(1)
        .and_then(|tail| tail.split(")?;").next())
        .unwrap();
    assert_eq!(call.matches("challenges,").count(), 1);
}

#[test]
fn nonzero_forced_witness_response_changes_the_bound_row_source() {
    let mut responses = vec![0_u8; super::super::super::RESPONSE_BYTES_V1];
    let modulus = RELEASE_MODULI_V1[0];
    let mut before = vec![0_u64; RELEASE_RING_COEFFICIENTS_V1];
    let mut after = vec![0_u64; RELEASE_RING_COEFFICIENTS_V1];
    decode_response_slots(&responses, 1, &[4], modulus, &mut before).unwrap();
    let mutation = response_offset(1, 4, 17).unwrap();
    responses[mutation..mutation + 8].copy_from_slice(&(-1_i64).to_be_bytes());
    decode_response_slots(&responses, 1, &[4], modulus, &mut after).unwrap();
    assert_eq!(before[17], 0);
    assert_eq!(after[17], modulus - 1);
    assert_ne!(before, after);
}

#[test]
fn candidate_completion_cannot_mint_release_authority() {
    let source = include_str!("rkg_one_semantic_verifier_v1.rs");
    assert!(source.contains(
        "pub(in super::super::super) struct CompletedDirectRkgOneSemanticVerificationV1"
    ));
    assert!(!source.contains("VerifiedDirectRelationProofReceiptV1"));
    assert!(!source.contains("receipt_digest"));
    assert!(!source.contains("release_available = true"));
    assert!(!source.contains("canonical_complete_wire_certified = true"));
    assert!(!source.contains("provider_identity()"));
    assert!(!source.contains("snapshot_identity()"));
    let marker = source
        .split("struct CompletedDirectRkgOneSemanticVerificationV1")
        .nth(1)
        .and_then(|tail| {
            tail.split("fn verify_direct_rkg_one_semantic_candidate_v1")
                .next()
        })
        .unwrap();
    for forbidden_surface in [
        "derive(",
        "impl Clone",
        "impl Copy",
        "impl Debug",
        "impl Default",
        "decode",
        "callback",
        "get_",
        "as_",
    ] {
        assert!(!marker.contains(forbidden_surface));
    }
    let active = include_str!("../../../active_exact_binding.rs");
    for closed_gate in [
        "let external_commitment_provenance_certified = false;",
        "let full_basis_mrep_crs_certified = false;",
        "let membership_argument_of_knowledge_certified = false;",
        "let membership_zero_knowledge_certified = false;",
        "let composite_rom_forking_certified = false;",
        "let full_ceremony_10_336_instance_composition_certified = false;",
        "let canonical_complete_wire_certified = false;",
        "let chunked_workspace_certified = false;",
        "let sampler_wired_to_runtime = false;",
        "let persistent_graph_wired_to_runtime = false;",
        "let split_decryption_wide_relation_certified = false;",
        "let release_kat_pinned = false;",
    ] {
        assert!(active.contains(closed_gate));
    }
    assert!(!active.contains("candidate_membership_union_soundness_bits"));
    assert!(!active.contains("BLOCKER_T256_MEMBERSHIP_BACKEND_V1"));
    assert!(active.contains("BLOCKER_T256_MEMBERSHIP_SECURITY_V1"));
    let public_verifier = active
        .split("pub(super) fn verify_and_consume_direct_relation_use_v1")
        .nth(1)
        .and_then(|tail| tail.split("/// Sole production minting boundary").next())
        .unwrap();
    assert!(public_verifier.contains("Err(ZkAmsMkheErrorV1::ReleaseUnavailable)"));
    assert!(!public_verifier.contains("verify_direct_rkg_one_semantic_candidate_v1"));
    assert!(active.contains("assert_eq!(audit.blocker_mask, 0xfd);"));
}
