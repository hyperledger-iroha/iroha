use super::*;

const VALID_POINT_V1: [u8; POINT_BYTES_V1] = [
    0x80, 0x25, 0xa4, 0xe3, 0x12, 0x8f, 0x04, 0x2d, 0x72, 0x8e, 0x58, 0xb7, 0xe0, 0x9a, 0x51, 0xb7,
    0x25, 0x85, 0xbe, 0x44, 0x35, 0xf4, 0xe9, 0x4a, 0xac, 0x85, 0x17, 0xf2, 0xe1, 0x58, 0xb3, 0xea,
    0xe6,
];

fn canonical_core_v1<const N: usize>(log_width: u8) -> [u8; N] {
    assert_eq!(
        N,
        generalized_bp_core_bytes_v1(log_width, 1).expect("governed core size")
    );
    let mut bytes = [0_u8; N];
    let mut cursor = 0;
    for _ in 0..BP_FIXED_POINTS_C_LE_ONE_V1 {
        bytes[cursor..cursor + POINT_BYTES_V1].copy_from_slice(&VALID_POINT_V1);
        cursor += POINT_BYTES_V1;
    }
    cursor += 3 * SCALAR_BYTES_V1;
    for _ in 0..2 * usize::from(log_width) {
        bytes[cursor..cursor + POINT_BYTES_V1].copy_from_slice(&VALID_POINT_V1);
        cursor += POINT_BYTES_V1;
    }
    cursor += 2 * SCALAR_BYTES_V1;
    assert_eq!(cursor, N);
    bytes
}

fn endpoint_commitments_v1() -> [u8; ENDPOINT_COMMITMENTS_BYTES_V1] {
    let mut bytes = [0_u8; ENDPOINT_COMMITMENTS_BYTES_V1];
    for point in bytes.chunks_exact_mut(POINT_BYTES_V1) {
        point.copy_from_slice(&VALID_POINT_V1);
    }
    bytes
}

#[test]
fn profile_pins_parent_kats_basis_layout_and_purpose_manifest() {
    let profile = global_lookup_committed_mle_profile_v1().expect("frozen profile");
    assert_eq!(profile.topology_digest, PINNED_TOPOLOGY_DIGEST_V1);
    assert_eq!(
        hex::encode(profile.topology_digest),
        "2d1dcc86a7c58d99a729df30b5c48d3082cea1e4706068eedf6c6ea5aea567a6"
    );
    assert_eq!(
        hex::encode(profile.challenge_manifest_digest),
        "992641207c6cd0c0f9b596cb7bfd192d44b8336210765d3ab2bb0bc0df52e0b7"
    );
    assert_eq!(
        hex::encode(profile.basis_digest),
        "bf81c83091a426bbcb2f7518ad3716391810e50b848b38d0c2b3cd96aff9a3f8"
    );
    assert_eq!(
        hex::encode(profile.purpose_manifest_digest),
        "7c33e742e079ed96d99d79c5e4639967234f885907a120f1fa6d5dd2eb4600c7"
    );
    assert_eq!(profile.proof_count, 19);
    assert_eq!(profile.accounted_wire_bytes, 27_276);
    assert_eq!((COORDINATE_BITS_V1, PLANE_BITS_V1), (14, 15));
    assert_eq!(LOOKUP_BITS_V1, 29);
    assert!(COMMITTED_MLE_LAYOUT_LANGUAGE_V1.starts_with(b"MLE-index=x=(c_0..c_13,y_0..y_14)"));
    assert!(
        COMMITTED_MLE_LAYOUT_LANGUAGE_V1
            .windows(b"bits-little-endian".len())
            .any(|window| window == b"bits-little-endian")
    );
    assert!(
        COMMITTED_MLE_TRANSCRIPT_LANGUAGE_V1
            .windows(b"no-caller-selected-digest".len())
            .any(|window| window == b"no-caller-selected-digest")
    );
    assert!(
        COMMITTED_MLE_BP_PRIMITIVE_LANGUAGE_V1
            .windows(b"a=0..127".len())
            .any(|window| window == b"a=0..127")
    );
    assert!(
        COMMITTED_MLE_OPENING_DIGEST_LANGUAGE_V1
            .windows(b"opening-digest-is-internally-derived-only".len())
            .any(|window| window == b"opening-digest-is-internally-derived-only")
    );
}

#[test]
fn exact_role_order_shapes_and_generalized_bp_lengths_are_frozen() {
    for ordinal in 0..COEFFICIENT_IPAS_V1 {
        let shape = proof_shape_v1(ordinal).expect("coefficient shape");
        assert_eq!(shape.ordinal, ordinal);
        assert!(
            matches!(shape.role, CommittedMleProofRoleV1::Coefficient(role) if ipa_tag_v1(role) == ordinal as u8 + 1)
        );
        assert_eq!(shape.width, 16_384);
        assert_eq!(shape.log_width, 14);
        assert_eq!((shape.vector_commitments, shape.scalar_commitments), (1, 1));
        assert_eq!(
            (
                shape.commitment_wire_bytes,
                shape.envelope_bytes,
                shape.core_bytes
            ),
            (0, 0, 1_381)
        );
    }
    let table = proof_shape_v1(16).unwrap();
    assert_eq!(table.role, CommittedMleProofRoleV1::TableM);
    assert_eq!((table.width, table.log_width), (32_768, 15));
    assert_eq!((table.vector_commitments, table.scalar_commitments), (1, 1));
    assert_eq!(
        (
            table.commitment_wire_bytes,
            table.core_bytes,
            table.wire_bytes_v1().unwrap()
        ),
        (33, 1_447, 1_480)
    );
    let mask = proof_shape_v1(17).unwrap();
    assert_eq!(mask.role, CommittedMleProofRoleV1::SumcheckMask);
    assert_eq!((mask.width, mask.log_width), (1_024, 10));
    assert_eq!((mask.vector_commitments, mask.scalar_commitments), (1, 1));
    assert_eq!(
        (
            mask.commitment_wire_bytes,
            mask.core_bytes,
            mask.wire_bytes_v1().unwrap()
        ),
        (33, 1_117, 1_150)
    );
    let endpoint = proof_shape_v1(18).unwrap();
    assert_eq!(endpoint.role, CommittedMleProofRoleV1::EndpointGates);
    assert_eq!((endpoint.width, endpoint.log_width), (32, 5));
    assert_eq!(
        (endpoint.vector_commitments, endpoint.scalar_commitments),
        (0, 52)
    );
    assert_eq!(
        (
            endpoint.commitment_wire_bytes,
            endpoint.envelope_bytes,
            endpoint.core_bytes
        ),
        (52 * 33, 47, 787)
    );
    assert_eq!(endpoint.wire_bytes_v1(), Ok(2_550));
    assert_eq!(proof_shape_v1(19), Err(CommittedMleErrorV1::Shape));

    for (log_width, vector_commitments, bytes) in
        [(10, 1, 1_117), (14, 1, 1_381), (15, 1, 1_447), (5, 0, 787)]
    {
        assert_eq!(
            generalized_bp_core_bytes_v1(log_width, vector_commitments),
            Ok(bytes)
        );
    }
    assert_eq!(
        generalized_bp_core_bytes_v1(5, 2),
        Err(CommittedMleErrorV1::Shape)
    );
}

#[test]
fn commitment_and_proof_accounting_includes_every_33_byte_point_once() {
    assert_eq!(COEFFICIENT_PROOF_SET_BYTES_V1, 16 * 1_381);
    assert_eq!(TABLE_M_COMMITMENT_AND_CORE_BYTES_V1, 33 + 1_447);
    assert_eq!(MASK_COMMITMENT_AND_CORE_BYTES_V1, 33 + 1_117);
    assert_eq!(ENDPOINT_COMMITMENTS_BYTES_V1, 52 * 33);
    assert_eq!(ENDPOINT_GATE_CORE_BYTES_V1, 787);
    assert_eq!(ENDPOINT_GATE_WIRE_BYTES_V1, 47 + 787);
    assert_eq!(ENDPOINT_COMMITMENTS_AND_PROOF_BYTES_V1, 52 * 33 + 47 + 787);
    assert_eq!(
        COMMITTED_MLE_ACCOUNTED_WIRE_BYTES_V1,
        16 * 1_381 + (33 + 1_447) + (33 + 1_117) + (52 * 33 + 47 + 787)
    );
}

#[test]
fn strict_borrowed_core_codecs_accept_only_exact_canonical_shapes() {
    let n1024 = canonical_core_v1::<MASK_CORE_BYTES_V1>(10);
    let n16384 = canonical_core_v1::<COEFFICIENT_CORE_BYTES_V1>(14);
    let n32768 = canonical_core_v1::<TABLE_M_CORE_BYTES_V1>(15);
    let endpoint = canonical_core_v1::<ENDPOINT_GATE_CORE_BYTES_V1>(5);
    assert_eq!(
        borrow_bp_core_exact_v1(10, &n1024)
            .unwrap()
            .bytes_v1()
            .len(),
        1_117
    );
    assert_eq!(
        borrow_bp_core_exact_v1(14, &n16384)
            .unwrap()
            .bytes_v1()
            .len(),
        1_381
    );
    assert_eq!(
        borrow_bp_core_exact_v1(15, &n32768)
            .unwrap()
            .bytes_v1()
            .len(),
        1_447
    );
    assert_eq!(
        borrow_bp_core_exact_v1(5, &endpoint)
            .unwrap()
            .bytes_v1()
            .len(),
        787
    );
    assert!(matches!(
        borrow_bp_core_exact_v1(14, &n16384[..n16384.len() - 1]),
        Err(CommittedMleErrorV1::WireEncoding)
    ));
    assert!(matches!(
        borrow_bp_core_exact_v1(13, &n16384),
        Err(CommittedMleErrorV1::WireEncoding)
    ));

    let mut bad_point = n16384;
    bad_point[..POINT_BYTES_V1].fill(0);
    assert!(matches!(
        borrow_bp_core_exact_v1(14, &bad_point),
        Err(CommittedMleErrorV1::PointEncoding)
    ));
    let mut bad_scalar = n16384;
    bad_scalar[9 * POINT_BYTES_V1..9 * POINT_BYTES_V1 + SCALAR_BYTES_V1].fill(0xff);
    assert!(matches!(
        borrow_bp_core_exact_v1(14, &bad_scalar),
        Err(CommittedMleErrorV1::ScalarEncoding)
    ));
    assert!(borrow_coefficient_opening_exact_v1(15, &n16384).is_ok());
    assert!(matches!(
        borrow_coefficient_opening_exact_v1(16, &n16384),
        Err(CommittedMleErrorV1::Order)
    ));
}

#[test]
fn endpoint_envelope_is_purpose_specific_exact_and_mutation_closed() {
    let mut envelope = [0_u8; ENDPOINT_ENVELOPE_BYTES_V1];
    write_endpoint_envelope_v1(&mut envelope).expect("static endpoint envelope");
    assert_eq!(
        hex::encode(envelope),
        "5a47455001000005100020003403137c33e742e079ed96d99d79c5e4639967234f885907a120f1fa6d5dd2eb4600c7"
    );
    assert_eq!(
        borrow_endpoint_envelope_exact_v1(&envelope).unwrap().0,
        &envelope
    );
    for offset in [0, 4, 5, 6, 7, 8, 9, 11, 13, 15, 46] {
        let mut mutated = envelope;
        mutated[offset] ^= 1;
        assert!(matches!(
            borrow_endpoint_envelope_exact_v1(&mutated),
            Err(CommittedMleErrorV1::WireEncoding)
        ));
    }
    assert!(matches!(
        borrow_endpoint_envelope_exact_v1(&envelope[..46]),
        Err(CommittedMleErrorV1::WireEncoding)
    ));
    let mut trailing = [0_u8; ENDPOINT_ENVELOPE_BYTES_V1 + 1];
    trailing[..ENDPOINT_ENVELOPE_BYTES_V1].copy_from_slice(&envelope);
    assert!(matches!(
        borrow_endpoint_envelope_exact_v1(&trailing),
        Err(CommittedMleErrorV1::WireEncoding)
    ));
    assert!(
        !ENDPOINT_ENVELOPE_LANGUAGE_V1
            .windows(b"ZMBP".len())
            .any(|window| window == b"ZMBP")
    );
}

#[test]
fn aggregate_borrowed_codecs_validate_commitments_order_and_all_parts() {
    let table_core = canonical_core_v1::<TABLE_M_CORE_BYTES_V1>(15);
    let mask_core = canonical_core_v1::<MASK_CORE_BYTES_V1>(10);
    let endpoint_core = canonical_core_v1::<ENDPOINT_GATE_CORE_BYTES_V1>(5);
    let mut envelope = [0_u8; ENDPOINT_ENVELOPE_BYTES_V1];
    write_endpoint_envelope_v1(&mut envelope).unwrap();
    let endpoints = endpoint_commitments_v1();

    let table = borrow_table_m_opening_exact_v1(&VALID_POINT_V1, &table_core).unwrap();
    assert_eq!(table.commitment.0, &VALID_POINT_V1);
    assert_eq!(table.core.bytes_v1().len(), TABLE_M_CORE_BYTES_V1);
    let mask = borrow_mask_opening_exact_v1(&VALID_POINT_V1, &mask_core).unwrap();
    assert_eq!(mask.commitment.0, &VALID_POINT_V1);
    assert_eq!(mask.core.bytes_v1().len(), MASK_CORE_BYTES_V1);
    let gate =
        borrow_endpoint_gate_opening_exact_v1(&endpoints, &envelope, &endpoint_core).unwrap();
    assert_eq!(gate.endpoint_commitments, &endpoints);
    assert_eq!(gate.envelope.0, &envelope);
    assert_eq!(gate.core.bytes_v1().len(), ENDPOINT_GATE_CORE_BYTES_V1);

    let mut bad_commitment = VALID_POINT_V1;
    bad_commitment.fill(0);
    assert!(matches!(
        borrow_table_m_opening_exact_v1(&bad_commitment, &table_core),
        Err(CommittedMleErrorV1::PointEncoding)
    ));
    assert!(matches!(
        borrow_mask_opening_exact_v1(&VALID_POINT_V1[..32], &mask_core),
        Err(CommittedMleErrorV1::WireEncoding)
    ));
    let mut bad_endpoints = endpoints;
    bad_endpoints[17 * POINT_BYTES_V1..18 * POINT_BYTES_V1].fill(0);
    assert!(matches!(
        borrow_endpoint_gate_opening_exact_v1(&bad_endpoints, &envelope, &endpoint_core),
        Err(CommittedMleErrorV1::PointEncoding)
    ));
    assert!(matches!(
        borrow_endpoint_gate_opening_exact_v1(
            &endpoints[..endpoints.len() - 1],
            &envelope,
            &endpoint_core
        ),
        Err(CommittedMleErrorV1::WireEncoding)
    ));
}

#[test]
fn production_authority_is_uninhabited_and_every_acceptance_gate_is_false() {
    for gate in [
        COMMITTED_MLE_PROFILE_ACCEPTED_V1,
        COMMITTED_MLE_DATA_PLANE_WIRED_V1,
        COMMITTED_MLE_PROVER_WIRED_V1,
        COMMITTED_MLE_VERIFIER_WIRED_V1,
        COMMITTED_MLE_PROOF_VERIFIED_V1,
        COMMITTED_MLE_ZERO_KNOWLEDGE_ACCEPTED_V1,
        COMMITTED_MLE_RECEIPT_ACCEPTED_V1,
        COMMITTED_MLE_AUTHORITY_MINTED_V1,
        COMMITTED_MLE_RSS_QUALIFIED_V1,
        COMMITTED_MLE_RELEASE_READY_V1,
    ] {
        assert!(!gate);
    }
    let owner = GlobalLookupCommittedMleOwnerV1 {
        data_plane: CommittedMleDataPlaneSealV1::TestOnly,
        proof_session: CommittedMleProofSessionSealV1::TestOnly,
    };
    assert!(matches!(
        owner.data_plane,
        CommittedMleDataPlaneSealV1::TestOnly
    ));
    assert!(matches!(
        owner.proof_session,
        CommittedMleProofSessionSealV1::TestOnly
    ));
}

#[test]
fn source_guards_keep_the_prerequisite_private_borrowed_and_nonauthorizing() {
    let source = include_str!("global_lookup_committed_mle_v1.rs");
    let tests = include_str!("global_lookup_committed_mle_v1_tests.rs");
    let parent = include_str!("../global_lookup_statement_v1.rs");
    assert!(source.lines().count() <= 1_200);
    assert!(tests.lines().count() <= 600);
    assert_eq!(
        parent
            .matches("mod global_lookup_committed_mle_v1;")
            .count(),
        1
    );
    assert!(!source.contains("pub struct"));
    assert!(!source.contains("pub enum"));
    assert!(!source.contains("pub fn"));
    assert!(!source.contains("Vec<"));
    assert!(!source.contains("to_vec("));
    assert!(!source.contains("ZkAmsT256Membership"));
    assert!(!source.contains("ZMBP"));
    assert!(!source.contains("fn prove"));
    assert!(!source.contains("fn verify"));
    assert!(!source.contains("fn release"));
    assert!(!source.contains("impl Clone for GlobalLookupCommittedMleOwnerV1"));
    for seal in [
        "enum CommittedMleDataPlaneSealV1",
        "enum CommittedMleProofSessionSealV1",
    ] {
        let body = source
            .split(seal)
            .nth(1)
            .unwrap()
            .split("}\n")
            .next()
            .unwrap();
        assert!(body.contains("Production"));
        assert!(body.contains("Infallible"));
    }
}
