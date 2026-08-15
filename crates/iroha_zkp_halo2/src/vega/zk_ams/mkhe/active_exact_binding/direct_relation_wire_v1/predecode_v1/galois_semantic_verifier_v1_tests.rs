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
fn provisional_challenge_coordinates_match_the_frozen_four_coordinate_frame() {
    let seed: [u8; 32] =
        hex::decode("0904346cfa9f051e59f3bff80220c037685d9e385a9b77dd622ddb391bca0284")
            .unwrap()
            .try_into()
            .unwrap();
    assert_eq!(
        provisional_challenges(seed),
        [2_768_221_376, 3_188_246_095, 2_792_925_365, 3_420_889_530]
    );
    let source = include_str!("galois_semantic_verifier_v1.rs");
    let helper = bounded_source_section(
        source,
        "fn provisional_challenges(",
        "fn reconstruct_commitment_first_messages(",
    );
    assert_eq!(helper.matches("u32::from_be_bytes(").count(), 1);
    assert_eq!(helper.matches("keccak256(&frame)").count(), 1);
    assert_eq!(CHALLENGE_REPETITIONS_V1, 4);
}

#[test]
fn row_tags_gadget_plaintext_residue_and_canonical_encoding_are_pinned() {
    let (tags, count) = PersistentDirectRelationV1::Galois.rns_row_tags();
    assert_eq!(count, 5);
    assert_eq!(&tags[..count], &[0x05, 0x81, 0x83, 0x84, 0x85]);
    let profile = release_profile_v1();
    let modulus = RELEASE_MODULI_V1[0];
    assert_eq!(profile.gadget_base_log, 60);
    assert_eq!(profile.gadget_digits, 38);
    assert_eq!(
        gadget_residue(0, profile.gadget_base_log, modulus).unwrap(),
        1
    );
    assert_eq!(
        gadget_residue(1, profile.gadget_base_log, modulus).unwrap(),
        262_143
    );
    assert_eq!(
        profile.plaintext_modulus.residue(modulus),
        1_125_144_406_804_725_708
    );
    let source = include_str!("galois_semantic_verifier_v1.rs");
    let encoder = bounded_source_section(
        source,
        "fn absorb_residue_limb(",
        "fn reconstruct_galois_b_coefficient(",
    );
    assert!(encoder.contains("residue.to_be_bytes()"));
}

#[test]
fn response_slot_routing_is_big_endian_and_uses_s_zero_e_two() {
    assert_eq!(GALOIS_ACTIVE_RESPONSE_SLOTS_V1, [0, 2]);
    assert_eq!(GALOIS_FORCED_ROW_SLOTS_V1, [(1, 1), (2, 3), (3, 4), (4, 5)]);
    let mut responses = vec![0_u8; super::super::super::RESPONSE_BYTES_V1];
    for (slot, value) in [(0, -7_i64), (2, 11), (3, -13)] {
        let offset = response_offset(2, slot, 3).unwrap();
        responses[offset..offset + 8].copy_from_slice(&value.to_be_bytes());
    }
    let modulus = RELEASE_MODULI_V1[0];
    let mut active = vec![0_u64; 2 * RELEASE_RING_COEFFICIENTS_V1];
    decode_response_slots(&responses, 2, &[0, 2], modulus, &mut active).unwrap();
    assert_eq!(active[3], signed_mod(-7, modulus));
    assert_eq!(active[RELEASE_RING_COEFFICIENTS_V1 + 3], 11);
    let mut forced = vec![0_u64; RELEASE_RING_COEFFICIENTS_V1];
    decode_response_slots(&responses, 2, &[3], modulus, &mut forced).unwrap();
    assert_eq!(forced[3], signed_mod(-13, modulus));
    assert!(decode_response_slots(&responses, 4, &[0], modulus, &mut forced).is_err());
    assert!(decode_response_slots(&responses, 0, &[], modulus, &mut []).is_err());
    assert!(decode_response_slots(&responses[..8], 0, &[0], modulus, &mut forced).is_err());
}

#[test]
fn source_to_destination_negacyclic_automorphism_scatter_is_exact() {
    let source = [1_u64, 2, 3, 4, 5, 6, 7, 8];
    let mut destination = [91_u64; 8];
    scatter_gadget_automorphism(&source, 5, 1, 97, &mut destination).unwrap();
    assert_eq!(destination, [1, 91, 94, 8, 5, 2, 90, 93]);

    assert!(scatter_gadget_automorphism(&source, 4, 1, 97, &mut destination).is_err());
    assert!(scatter_gadget_automorphism(&source, 16, 1, 97, &mut destination).is_err());
    assert!(scatter_gadget_automorphism(&source, 5, 97, 97, &mut destination).is_err());
    let mut noncanonical = source;
    noncanonical[0] = 97;
    assert!(scatter_gadget_automorphism(&noncanonical, 5, 1, 97, &mut destination).is_err());
    assert!(scatter_gadget_automorphism(&source, 5, 1, 97, &mut destination[..4]).is_err());

    let arithmetic = include_str!("../../../../mkhe.rs");
    let rns_impl = arithmetic
        .split_once("impl RnsPolynomial {")
        .and_then(|(_, tail)| {
            tail.split_once("impl SecretPolynomial {")
                .map(|(body, _)| body)
        })
        .expect("authoritative RNS polynomial implementation");
    let authoritative = rns_impl
        .split_once("fn automorphism(")
        .and_then(|(_, tail)| tail.split_once("fn validate(&self").map(|(body, _)| body))
        .expect("authoritative RNS automorphism");
    for snippet in [
        "let mapped = index * exponent % twice_degree;",
        "if mapped >= profile.ring_degree {",
        "modulus - value",
        "output.coefficients[limb * profile.ring_degree + destination] = coefficient;",
    ] {
        assert!(authoritative.contains(snippet));
    }
}

#[test]
fn scalar_galois_equation_pins_all_four_signs() {
    let reconstructed = reconstruct_galois_b_coefficient(70, 5, 7, 13, 97);
    assert_eq!(reconstructed, 26);
    assert_eq!(reconstruct_galois_b_coefficient(0, 0, 0, 1, 97), 96);
    assert_eq!(reconstruct_galois_b_coefficient(1, 0, 0, 0, 97), 96);
    assert_eq!(reconstruct_galois_b_coefficient(0, 1, 0, 0, 97), 1);
    assert_eq!(reconstruct_galois_b_coefficient(0, 0, 1, 0, 97), 1);
}

#[test]
fn production_row_zero_operands_and_canonical_order_are_pinned() {
    let source = include_str!("galois_semantic_verifier_v1.rs");
    let row_zero = bounded_source_section(
        source,
        "fn replay_galois_row_zero<P>(",
        "fn reconstruct_forced_zero_rows(",
    );
    assert_unique_ordered(
        row_zero,
        &[
            "target_a.derive_next_limb_into(&mut a)?;",
            "public_b.replay_next_limb_into(provider, &mut b)?;",
            "decode_response_slots(\n                responses,\n                repetition,\n                &GALOIS_ACTIVE_RESPONSE_SLOTS_V1,",
            "let (s, e) = response.split_at(RELEASE_RING_COEFFICIENTS_V1);",
            "let a_times_s = negacyclic_multiply(&a, s, modulus, root)?;",
            "scatter_gadget_automorphism(s, exponent, gadget, modulus, &mut row)?;",
            "let challenge = u64::from(challenges[repetition]) % modulus;",
            "row[coefficient] = reconstruct_galois_b_coefficient(",
            "a_times_s[coefficient],",
            "row[coefficient],",
            "mod_mul(e[coefficient], plaintext_multiplier, modulus),",
            "mod_mul(challenge, b[coefficient], modulus),",
            "drop(a_times_s);",
            "absorb_residue_limb(&mut hashers[repetition], 0, limb, &row)?;",
        ],
    );
    assert_eq!(row_zero.matches("for (limb, (&modulus, root))").count(), 1);
    assert_eq!(
        row_zero
            .matches("for repetition in 0..CHALLENGE_REPETITIONS_V1")
            .count(),
        1
    );
}

#[test]
fn every_forced_zero_row_hashes_its_actual_response_vector() {
    let source = include_str!("galois_semantic_verifier_v1.rs");
    let forced = bounded_source_section(
        source,
        "fn reconstruct_forced_zero_rows(",
        "fn finish_rns_hashers(",
    );
    assert_unique_ordered(
        forced,
        &[
            "for (row, slot) in GALOIS_FORCED_ROW_SLOTS_V1",
            "for (limb, modulus) in RELEASE_MODULI_V1.iter().copied().enumerate()",
            "for repetition in 0..CHALLENGE_REPETITIONS_V1",
            "decode_response_slots(responses, repetition, &[slot], modulus, &mut response)?;",
            "absorb_residue_limb(&mut hashers[repetition], row, limb, &response)?;",
        ],
    );
    assert!(!forced.contains("zero_row"));
    assert!(!forced.contains("response.fill(0)"));

    let mut responses = vec![0_u8; super::super::super::RESPONSE_BYTES_V1];
    let modulus = RELEASE_MODULI_V1[0];
    let mut before = vec![0_u64; RELEASE_RING_COEFFICIENTS_V1];
    let mut after = vec![0_u64; RELEASE_RING_COEFFICIENTS_V1];
    decode_response_slots(&responses, 1, &[5], modulus, &mut before).unwrap();
    let mutation = response_offset(1, 5, 17).unwrap();
    responses[mutation..mutation + 8].copy_from_slice(&(-1_i64).to_be_bytes());
    decode_response_slots(&responses, 1, &[5], modulus, &mut after).unwrap();
    assert_eq!(before[17], 0);
    assert_eq!(after[17], modulus - 1);
    assert_ne!(before, after);
}

#[test]
fn exact_work_provider_and_narrow_live_payload_ledgers_are_checked() {
    validate_rns_live_payload_accounting().unwrap();
    assert_eq!(GALOIS_MEMBERSHIP_PROOF_VERIFICATIONS_V1, 48);
    assert_eq!(GALOIS_RESPONSE_COMMITMENT_RECONSTRUCTIONS_V1, 192);
    assert_eq!(GALOIS_NEGACYCLIC_PRODUCTS_V1, 152);
    assert_eq!(GALOIS_FORWARD_NTTS_V1, 304);
    assert_eq!(GALOIS_INVERSE_NTTS_V1, 152);
    assert_eq!(GALOIS_AUTOMORPHISM_SCATTERS_V1, 152);
    assert_eq!(GALOIS_RNS_ROW_COEFFICIENTS_V1, 99_614_720);
    assert_eq!(GALOIS_TARGET_A_DERIVED_BYTES_V1, 39_845_888);
    assert_eq!(GALOIS_PROVIDER_READ_CALLS_V1, 4_864);
    assert_eq!(GALOIS_PROVIDER_READ_BYTES_V1, 39_845_888);
    assert_eq!(GALOIS_ROW_ZERO_BASE_BYTES_V1, 4_194_304);
    assert_eq!(GALOIS_ROW_ZERO_NTT_LIVE_BYTES_V1, 6_291_456);
    assert_eq!(GALOIS_ROW_ZERO_ASSEMBLY_LIVE_BYTES_V1, 6_291_456);
    assert_eq!(GALOIS_ROW_ZERO_HASH_LIVE_BYTES_V1, 6_291_456);
    assert_eq!(GALOIS_TYPED_B_REPLAY_LIVE_BYTES_V1, 4_202_496);
    assert_eq!(GALOIS_FORCED_ROW_LIVE_BYTES_V1, 2_097_152);
    assert_eq!(GALOIS_RNS_LIVE_PAYLOAD_CEILING_BYTES_V1, 6_291_456);
    assert_eq!(
        super::super::BORROWED_MEMBERSHIP_PROOF_ALLOCATIONS_ELIDED_V1,
        48
    );
    assert_eq!(
        super::super::BORROWED_MEMBERSHIP_PROOF_LOGICAL_BYTES_ELIDED_V1,
        71_568
    );

    let arithmetic = include_str!("../../../../mkhe.rs");
    let multiply =
        bounded_source_section(arithmetic, "fn negacyclic_multiply(", "fn bytes_mod_u64(");
    assert_eq!(multiply.matches("\n    cyclic_ntt(&mut").count(), 2);
    assert_eq!(multiply.matches("\n    inverse_cyclic_ntt(&mut").count(), 1);
    let b_replay = include_str!("../statement_v1/galois_b_replay_v1.rs");
    assert!(b_replay.contains("assert!(READ_CALLS_PER_OBJECT_V1 == 4_864);"));
    assert!(b_replay.contains("assert!(EXACT_POLYNOMIAL_BYTES_V1 == 39_845_888);"));
    assert!(b_replay.contains("u64::from_be_bytes("));
    assert!(b_replay.contains("if value >= modulus"));
}

#[test]
fn memberships_and_all_response_points_precede_any_provider_access() {
    let source = include_str!("galois_semantic_verifier_v1.rs");
    let wrapper = bounded_source_section(
        source,
        "fn verify_direct_galois_semantic_candidate_v1<P>(",
        "fn verify_predecoded_direct_galois_semantic_candidate_v1<P>(",
    );
    let predecode = wrapper
        .find("super::predecode_direct_relation_proof_v1(")
        .unwrap();
    let helper = wrapper
        .find("verify_predecoded_direct_galois_semantic_candidate_v1(")
        .unwrap();
    assert!(predecode < helper);
    assert!(wrapper.contains("capability: VerifiedPersistentWitnessDirectRelationUseV1"));
    assert!(!wrapper.contains("replay_next_limb_into"));
    assert!(!wrapper.contains("provider.read"));

    let verifier = bounded_source_section(
        source,
        "fn verify_predecoded_direct_galois_semantic_candidate_v1<P>(",
        "fn provisional_challenges(",
    );
    let destructure = verifier
        .find("let PredecodedDirectRelationProofV1 {")
        .unwrap();
    let membership = verifier
        .find("membership_frames.verify_replayable()?")
        .unwrap();
    let copy = verifier
        .find("membership_frames.copied_commitments()")
        .unwrap();
    let frame_drop = verifier.find("drop(membership_frames);").unwrap();
    let commitments = verifier
        .find("reconstruct_commitment_first_messages(")
        .unwrap();
    let target = verifier.find("DirectGaloisTargetAReplayV1::begin").unwrap();
    let provider = verifier
        .find("DirectGaloisBStatementReplayV1::begin")
        .unwrap();
    let replay = verifier.find("replay_galois_row_zero(").unwrap();
    assert!(destructure < membership && membership < copy && copy < frame_drop);
    assert!(frame_drop < commitments && commitments < target && target < provider);
    assert!(provider < replay);
    for forbidden in [
        ".materialize()",
        ".to_vec()",
        "ExactEightChunkMembershipEvidenceV1",
    ] {
        assert!(!verifier.contains(forbidden));
    }
    let reconstruction = bounded_source_section(
        source,
        "fn reconstruct_commitment_first_messages(",
        "fn decode_response_chunk(",
    );
    assert_eq!(
        reconstruction
            .matches("for repetition in 0..CHALLENGE_REPETITIONS_V1")
            .count(),
        1
    );
    assert_eq!(
        reconstruction
            .matches("for slot in 0..WITNESS_COUNT_V1")
            .count(),
        1
    );
    assert_eq!(
        reconstruction
            .matches("for chunk in 0..CHUNKS_PER_WITNESS_V1")
            .count(),
        1
    );
    let rkg = include_str!("rkg_one_semantic_verifier_v1.rs");
    for shared in [
        "let PredecodedDirectRelationProofV1 {",
        "membership_frames.verify_replayable()?",
        "membership_frames.copied_commitments()",
        "drop(membership_frames);",
        "super::validate_reconstructed_challenge(",
    ] {
        assert_eq!(source.matches(shared).count(), 1);
        assert_eq!(rkg.matches(shared).count(), 1);
    }
    let predecode_source = include_str!("../predecode_v1.rs");
    let production_predecode = bounded_source_section(
        predecode_source,
        "pub(in super::super) fn predecode_direct_relation_proof_v1<'a>(",
        "pub(super) fn validate_header(",
    );
    assert!(production_predecode.contains("membership_frames: preflighted"));
    for forbidden in [
        ".materialize()",
        ".to_vec()",
        "ExactEightChunkMembershipEvidenceV1",
    ] {
        assert!(!production_predecode.contains(forbidden));
    }
}

#[test]
fn both_poisoned_completions_live_through_final_challenge_equality() {
    let source = include_str!("galois_semantic_verifier_v1.rs");
    let verifier = bounded_source_section(
        source,
        "fn verify_predecoded_direct_galois_semantic_candidate_v1<P>(",
        "fn provisional_challenges(",
    );
    assert_unique_ordered(
        verifier,
        &[
            "let completed_replays = (target_a.finish()?, public_b.finish(provider)?);",
            "reconstruct_forced_zero_rows(responses, &mut hashers)?;",
            "let rns_digests = finish_rns_hashers(&mut hashers)?;",
            "super::validate_reconstructed_challenge(",
            "if reconstructed_challenges != challenges",
            "drop(completed_replays);",
        ],
    );
    assert!(!verifier.contains("catch_unwind"));
    let target = include_str!("../../direct_galois_target_a_v1.rs");
    assert!(target.contains("self.failed = true;"));
    assert!(target.contains("if result.is_ok()"));
    assert!(target.contains("inject_unwind_on_next_derive_for_test"));
    let target_tests = include_str!("../../direct_galois_target_a_v1_tests.rs");
    assert!(target_tests.contains("sealed_cpk_mint_and_poisoned_replay_are_end_to_end_typed"));
    let public_b = include_str!("../statement_v1/galois_b_replay_v1.rs");
    assert!(public_b.contains("self.poisoned = true;"));
    assert!(public_b.contains("if result.is_ok()"));
    assert!(public_b.contains("ExpectedDirectRelationStatementV1::new("));
    assert!(public_b.contains("ZkAmsMkheDirectObjectKindV1::GaloisB"));
    assert_eq!(
        public_b
            .matches("ZkAmsMkheDirectObjectReadTransactionV1::begin(")
            .count(),
        1
    );
    let public_b_tests = include_str!("../statement_v1/galois_b_replay_v1_tests.rs");
    assert!(public_b_tests.contains("caught_provider_unwind_cannot_resume_the_replay"));
    assert!(public_b_tests.contains("snapshot_drift_at"));
}

#[test]
fn schedule_authority_and_exact_thirty_eight_limb_streams_are_source_pinned() {
    let source = include_str!("galois_semantic_verifier_v1.rs");
    let row_zero = bounded_source_section(
        source,
        "fn replay_galois_row_zero<P>(",
        "fn reconstruct_forced_zero_rows(",
    );
    assert!(row_zero.contains("usize::try_from(context.galois_exponent())"));
    assert!(
        row_zero.contains(
            "RELEASE_MODULI_V1\n        .iter()\n        .zip(RELEASE_NEGACYCLIC_ROOTS_V1)"
        )
    );
    assert_eq!(RELEASE_RNS_LIMBS_V1, 38);
    assert_eq!(RELEASE_RING_COEFFICIENTS_V1, 131_072);
    let target = include_str!("../../direct_galois_target_a_v1.rs");
    assert!(target.contains("validate_zk_ams_t256_galois_key_schedule_v1(&schedule)?;"));
    assert!(target.contains("axes.galois_exponent != entry.exponent"));
    assert!(target.contains("self.next_limb != DIRECT_GALOIS_TARGET_A_RELEASE_LIMBS_V1"));
    let public_b = include_str!("../statement_v1/galois_b_replay_v1.rs");
    assert!(public_b.contains("self.next_limb != RELEASE_RNS_LIMBS_V1"));
}

#[test]
fn candidate_marker_has_no_receipt_codec_pointer_digest_or_release_authority() {
    let source = include_str!("galois_semantic_verifier_v1.rs");
    assert!(source.contains(
        "pub(in super::super::super) struct CompletedDirectGaloisSemanticVerificationV1"
    ));
    for forbidden in [
        "VerifiedDirectRelationProofReceiptV1",
        "receipt_digest",
        "provider_identity()",
        "snapshot_identity()",
        "release_available = true",
        "canonical_complete_wire_certified = true",
    ] {
        assert!(!source.contains(forbidden));
    }
    let marker = bounded_source_section(
        source,
        "struct CompletedDirectGaloisSemanticVerificationV1",
        "fn verify_direct_galois_semantic_candidate_v1<P>(",
    );
    for forbidden in [
        "derive(",
        "impl Clone",
        "impl Copy",
        "impl Default",
        "decode",
        "get_",
        "as_",
    ] {
        assert!(!marker.contains(forbidden));
    }
    let active = include_str!("../../../active_exact_binding.rs");
    for gate in [
        "let membership_argument_of_knowledge_certified = false;",
        "let membership_zero_knowledge_certified = false;",
        "let composite_rom_forking_certified = false;",
        "let canonical_complete_wire_certified = false;",
        "let release_kat_pinned = false;",
    ] {
        assert!(active.contains(gate));
    }
    let public_verifier = bounded_source_section(
        active,
        "pub(super) fn verify_and_consume_direct_relation_use_v1",
        "/// Sole production minting boundary",
    );
    assert!(public_verifier.contains("Err(ZkAmsMkheErrorV1::ReleaseUnavailable)"));
    assert!(!public_verifier.contains("verify_direct_galois_semantic_candidate_v1"));
}

#[test]
fn allocation_schedule_is_narrow_and_does_not_claim_whole_verifier_rss() {
    let source = include_str!("galois_semantic_verifier_v1.rs");
    for disclaimer in [
        "deliberately narrow live-payload ledger",
        "borrowed proof bytes",
        "response-MSM transients",
        "target-A sampler frames",
        "allocator overhead",
        "not a whole-verifier RSS claim or release certification",
    ] {
        assert!(source.contains(disclaimer));
    }
    assert_eq!(source.matches("try_reserve_exact(length)").count(), 2);
    assert!(source.contains("drop(a_times_s);"));
    assert!(!source.contains("retained_replay_matrices"));
    assert!(!source.contains("unsafe"));
}
