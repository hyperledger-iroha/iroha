// Successful fallible exact-eight constructors retain exact Rust-visible
// public Vec capacities. Allocator metadata, rejected over-grants, caller
// clones/backing storage, and RSS remain outside this source-level contract.

fn assert_exact_capacity_roundtrip<R: ExactEightChunkMembershipRoleV1>() {
    let evidence = fake_evidence::<R>(b"exact-capacity-roundtrip", 0);
    for chunk in evidence.chunks() {
        assert_eq!(
            (chunk.proof_bytes().len(), chunk.proof_capacity()),
            (R::PROOF_BYTES, R::PROOF_BYTES)
        );
        let wire = chunk
            .try_to_wire_bytes_exact_capacity_v1()
            .expect("exact chunk wire");
        assert_eq!(
            (wire.len(), wire.capacity()),
            (R::CHUNK_WIRE_BYTES, R::CHUNK_WIRE_BYTES)
        );
    }
    let wire = evidence.to_wire_bytes().expect("exact outer wire");
    assert_eq!(
        (wire.len(), wire.capacity()),
        (R::WIRE_BYTES, R::WIRE_BYTES)
    );
    let decoded = ExactEightChunkMembershipEvidenceV1::<R>::from_wire_bytes_exact(&wire)
        .expect("exact decode");
    assert!(decoded.chunks().iter().all(|chunk| {
        (chunk.proof_bytes().len(), chunk.proof_capacity()) == (R::PROOF_BYTES, R::PROOF_BYTES)
    }));
}

#[test]
fn fallible_bound_one_and_two_outputs_have_exact_public_capacities() {
    assert_exact_capacity_roundtrip::<PersistentSecretMembershipRoleV1>();
    assert_exact_capacity_roundtrip::<CpkErrorMembershipRoleV1>();
}

#[cfg(target_pointer_width = "64")]
#[test]
fn exact_eight_public_capacity_ledger_is_pinned() {
    assert_eq!(core::mem::size_of::<ZkAmsT256MembershipProofV1>(), 128);
    let collector = 8 * 128;
    let one = (1_447, 1_494, 12_291);
    let two = (1_513, 1_560, 12_819);
    assert_eq!(collector, 1_024);
    assert_eq!(
        (
            8 * one.0 + collector,
            8 * two.0 + collector,
            8 * one.0 + one.1,
            8 * two.0 + two.1,
        ),
        (12_600, 13_128, 13_070, 13_664)
    );
    assert_eq!(
        (
            8 * one.0 + one.2 + one.1,
            8 * two.0 + two.2 + two.1,
            8 * one.0 + one.2,
            8 * two.0 + two.2,
        ),
        (25_361, 26_483, 23_867, 24_923)
    );
}

#[test]
fn exact_capacity_source_graph_is_closed_and_capped() {
    let t256 = include_str!("../../bulletproof_t256.rs");
    let proof_impl = t256
        .split_once("impl ZkAmsT256MembershipProofV1 {")
        .unwrap()
        .1
        .split_once("impl ProofScalar for Scalar")
        .unwrap()
        .0;
    for required in [
        "pub(super) fn proof_capacity(&self)",
        "pub(super) fn try_to_wire_bytes_exact_capacity_v1(",
        "try_exact_capacity_vec_v1(borrowed.proof.len())?",
        "proof.capacity() != borrowed.proof.len()",
    ] {
        assert!(proof_impl.contains(required));
    }
    assert!(!proof_impl.contains("borrowed.proof.to_vec()"));
    let prover = t256
        .split_once("fn prove_membership_chunk_for_suite<S, R>(")
        .unwrap()
        .1
        .split_once("#[derive(Clone, Copy)]\nenum ZkAmsT256MembershipVerificationInputV1")
        .unwrap()
        .0;
    let proof_len = prover.find("proof.len() != expected_proof_len").unwrap();
    let proof_capacity = prover
        .find("proof.capacity() != expected_proof_len")
        .unwrap();
    assert!(proof_len < proof_capacity);

    let exact = include_str!("exact_eight_chunk_membership.rs");
    let production = exact
        .split_once(
            "\n#[cfg(test)]\npub(super) fn canonical_membership_syntax_wire_fixture_for_test",
        )
        .unwrap()
        .0;
    assert_eq!(
        production.matches("try_exact_membership_vec_v1(").count(),
        3
    );
    assert_eq!(
        production
            .matches("try_exact_membership_vec_v1(ZK_AMS_MKHE_EXACT_MEMBERSHIP_CHUNKS_V1)?")
            .count(),
        2
    );
    assert_eq!(
        production
            .matches("try_to_wire_bytes_exact_capacity_v1()?")
            .count(),
        2
    );
    assert_eq!(
        production
            .matches("chunk.proof_capacity() != R::PROOF_BYTES")
            .count(),
        1
    );
    assert_eq!(production.matches("chunk_allocation").count(), 4);
    assert_eq!(production.matches("wire_allocation").count(), 2);
    assert!(!production.contains("Vec::with_capacity"));
    for (source, line_cap, byte_cap) in [(t256, 3_000, 120 * 1_024), (exact, 2_000, 120 * 1_024)] {
        assert!(source.lines().count() <= line_cap);
        assert!(source.len() <= byte_cap);
    }
    let self_source = include_str!("exact_eight_chunk_membership_capacity_tests.rs");
    assert!(self_source.lines().count() <= 500);
    assert!(self_source.len() <= 24 * 1_024);
}
