use super::*;
use crate::vega::derive_t256_generators_v1;

fn digest(label: u8) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(b"rns-native-source-terminal-cross-field-test");
    hash.update(&[label]);
    hash.finalize()
}

fn anchor_core(downstream: &[u8]) -> [[u8; DIGEST_BYTES_V1]; ANCHOR_CORE_DIGESTS_V1] {
    let mut core = core::array::from_fn(|index| digest(index as u8 + 1));
    core[CORE_DOWNSTREAM_V1] = downstream_digest_v1(downstream);
    assert!(!core[..CORE_DOWNSTREAM_V1].contains(&core[CORE_DOWNSTREAM_V1]));
    core
}

fn encode_anchor(
    core: [[u8; DIGEST_BYTES_V1]; ANCHOR_CORE_DIGESTS_V1],
    downstream: &[u8],
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(ANCHOR_FIXED_BYTES_V1 + downstream.len());
    bytes.extend_from_slice(&ANCHOR_MAGIC_V1);
    bytes.push(LINK_VERSION_V1);
    bytes.push(ANCHOR_FLAGS_V1);
    bytes.push(ZK_AMS_MKHE_RNS_NATIVE_CROSS_FIELD_POINT_COUNT_V1);
    bytes.push(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u8);
    bytes.push(ZK_AMS_MKHE_RNS_NATIVE_SUMCHECK_ROUNDS_V1);
    bytes.push(ANCHOR_CORE_DIGESTS_V1 as u8);
    bytes.extend_from_slice(&(TERMINAL_ROWS_V1 as u16).to_be_bytes());
    bytes.extend_from_slice(&(TERMINAL_COLUMNS_V1 as u16).to_be_bytes());
    bytes.push(4);
    bytes.push(0);
    bytes.extend_from_slice(&(downstream.len() as u32).to_be_bytes());
    for digest in core {
        bytes.extend_from_slice(&digest);
    }
    bytes.extend_from_slice(downstream);
    bytes
}

#[test]
fn residual_anchor_is_exact_capped_and_digest_bound() {
    let downstream = b"nonempty-cross-field-continuation";
    let core = anchor_core(downstream);
    let encoded = encode_anchor(core, downstream);
    assert_eq!(encoded.len(), ANCHOR_FIXED_BYTES_V1 + downstream.len());
    let decoded = ResidualAnchorV1::from_canonical_bytes_exact_v1(&encoded).expect("anchor");
    assert_eq!(decoded.core, core);
    assert_eq!(decoded.downstream, downstream);
    validate_anchor_core_v1(decoded, core).expect("exact core");
    for index in 0..ANCHOR_CORE_DIGESTS_V1 {
        let mut mutation = core;
        mutation[index] = digest(220 + index as u8);
        assert_eq!(
            validate_anchor_core_v1(decoded, mutation),
            Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor)
        );
    }

    for length in 0..encoded.len() {
        assert!(ResidualAnchorV1::from_canonical_bytes_exact_v1(&encoded[..length]).is_err());
    }
    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(
        ResidualAnchorV1::from_canonical_bytes_exact_v1(&trailing),
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor)
    );
    let oversized = vec![0_u8; RNS_NATIVE_RLWE_SOURCE_DOWNSTREAM_MAX_BYTES_V1 + 1];
    assert_eq!(
        ResidualAnchorV1::from_canonical_bytes_exact_v1(&oversized),
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::AnchorCapExceeded)
    );

    for offset in [0_usize, 4, 5, 6, 7, 8, 9, 10, 12, 14, 15] {
        let mut mutation = encoded.clone();
        mutation[offset] ^= 1;
        assert!(ResidualAnchorV1::from_canonical_bytes_exact_v1(&mutation).is_err());
    }
    let mut bad_length = encoded.clone();
    bad_length[16..20].copy_from_slice(&(downstream.len() as u32 + 1).to_be_bytes());
    assert_eq!(
        ResidualAnchorV1::from_canonical_bytes_exact_v1(&bad_length),
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor)
    );
    let mut bad_downstream = encoded.clone();
    *bad_downstream.last_mut().expect("byte") ^= 1;
    assert_eq!(
        ResidualAnchorV1::from_canonical_bytes_exact_v1(&bad_downstream),
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor)
    );
    let mut alias = core;
    alias[3] = alias[2];
    assert_eq!(
        ResidualAnchorV1::from_canonical_bytes_exact_v1(&encode_anchor(alias, downstream)),
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::AliasedDigest)
    );
}

#[test]
fn anchor_accepts_exact_maximum_and_rejects_zero_or_max_plus_one() {
    let maximum = vec![0x5a; LINK_DOWNSTREAM_MAX_BYTES_V1];
    let encoded = encode_anchor(anchor_core(&maximum), &maximum);
    assert_eq!(
        encoded.len(),
        RNS_NATIVE_RLWE_SOURCE_DOWNSTREAM_MAX_BYTES_V1
    );
    assert!(ResidualAnchorV1::from_canonical_bytes_exact_v1(&encoded).is_ok());

    let empty = encode_anchor(anchor_core(b"x"), b"");
    assert_eq!(
        ResidualAnchorV1::from_canonical_bytes_exact_v1(&empty),
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor)
    );
    let too_large = vec![0x6b; LINK_DOWNSTREAM_MAX_BYTES_V1 + 1];
    let encoded = encode_anchor(anchor_core(&too_large), &too_large);
    assert_eq!(
        ResidualAnchorV1::from_canonical_bytes_exact_v1(&encoded),
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::AnchorCapExceeded)
    );
}

#[test]
fn terminal_coordinate_mapping_freezes_every_family_boundary() {
    assert_eq!(
        terminal_coordinate_v1(E_FIRST_RECORD_V1, 0),
        Ok(TerminalCoordinateV1::Value { row: 0, column: 0 })
    );
    assert_eq!(
        terminal_coordinate_v1(E_FIRST_RECORD_V1, 65_535),
        Ok(TerminalCoordinateV1::Value {
            row: 63,
            column: 1_023,
        })
    );
    assert_eq!(
        terminal_coordinate_v1(E_FIRST_RECORD_V1 + 15, 65_535),
        Ok(TerminalCoordinateV1::Value {
            row: 1_023,
            column: 1_023,
        })
    );
    assert_eq!(
        terminal_coordinate_v1(RE_RECORD_V1, 1_023),
        Ok(TerminalCoordinateV1::Blinding { row: 1_023 })
    );
    assert_eq!(
        terminal_coordinate_v1(W_FIRST_RECORD_V1, 0),
        Ok(TerminalCoordinateV1::Value {
            row: 1_024,
            column: 0,
        })
    );
    assert_eq!(
        terminal_coordinate_v1(W_FIRST_RECORD_V1 + 7, 65_535),
        Ok(TerminalCoordinateV1::Value {
            row: 1_535,
            column: 1_023,
        })
    );
    assert_eq!(
        terminal_coordinate_v1(RW_RECORD_V1, 511),
        Ok(TerminalCoordinateV1::Blinding { row: 1_535 })
    );
    for invalid in [
        (16, 0),
        (E_FIRST_RECORD_V1, 65_536),
        (RE_RECORD_V1, 1_024),
        (W_FIRST_RECORD_V1, 65_536),
        (RW_RECORD_V1, 512),
        (43, 0),
    ] {
        assert_eq!(
            terminal_coordinate_v1(invalid.0, invalid.1),
            Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidGeometry)
        );
    }
}

#[test]
fn row_batch_challenge_and_cross_metadata_are_order_and_context_bound() {
    let exact_formula = mapping_formula_digest_v1().expect("formula");
    assert_ne!(exact_formula, [0; DIGEST_BYTES_V1]);
    assert_eq!(
        exact_formula,
        mapping_formula_digest_v1().expect("deterministic formula")
    );
    let seed = digest(101);
    let formula = digest(102);
    let openings = digest(103);
    let points = digest(104);
    let first = derive_mapping_challenge_v1(seed, formula, openings, points).expect("challenge");
    assert!(!first.is_zero());
    assert_ne!(first, Scalar::one());
    assert_eq!(
        first,
        derive_mapping_challenge_v1(seed, formula, openings, points).expect("same challenge")
    );
    for changed in [
        derive_mapping_challenge_v1(digest(105), formula, openings, points),
        derive_mapping_challenge_v1(seed, digest(106), openings, points),
        derive_mapping_challenge_v1(seed, formula, digest(107), points),
        derive_mapping_challenge_v1(seed, formula, openings, digest(108)),
    ] {
        assert_ne!(first, changed.expect("separated challenge"));
    }
    let weights = row_weights_v1(first);
    assert_eq!(weights[0], Scalar::one());
    assert_eq!(weights[1], first);
    assert_eq!(weights[2], first * first);
    assert_ne!(weights[ERROR_ROWS_V1 - 1], weights[ERROR_ROWS_V1]);

    let ordered: [[u8; DIGEST_BYTES_V1]; 5] = core::array::from_fn(|i| digest(120 + i as u8));
    let original = indexed_digest_bundle_v1(POINT_BUNDLE_DOMAIN_V1, &ordered).expect("bundle");
    let mut reordered = ordered;
    reordered.swap(1, 2);
    assert_ne!(
        original,
        indexed_digest_bundle_v1(POINT_BUNDLE_DOMAIN_V1, &reordered).expect("reordered")
    );
    assert_ne!(
        original,
        indexed_digest_bundle_v1(LIMB_BUNDLE_DOMAIN_V1, &ordered).expect("domain")
    );
    let mut duplicate = ordered;
    duplicate[4] = duplicate[3];
    assert_eq!(
        indexed_digest_bundle_v1(POINT_BUNDLE_DOMAIN_V1, &duplicate),
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::AliasedDigest)
    );
    let mut zero = ordered;
    zero[0] = [0; DIGEST_BYTES_V1];
    assert_eq!(
        indexed_digest_bundle_v1(POINT_BUNDLE_DOMAIN_V1, &zero),
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::AliasedDigest)
    );
    assert_ne!(
        cross_proof_digest_v1(b"proof").expect("proof"),
        cross_proof_digest_v1(b"proof-mutated").expect("mutated")
    );
    assert_eq!(
        cross_proof_digest_v1(b""),
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidCrossFieldBinding)
    );
}

#[test]
fn opening_slice_digests_bind_family_order_points_and_source_identity() {
    let points =
        derive_t256_generators_v1(b"rns-native-source-terminal-test-points", 2).expect("points");
    let terminal = vec![points[0]; TERMINAL_ROWS_V1];
    let point_set = digest(201);
    let source = digest(202);
    let placeholder = digest(203);
    let opening = |family, index| {
        ZkAmsMkheRnsNativeOpeningCommitmentV1::new(family, index, source, placeholder)
            .expect("opening")
    };
    let e0 = opening_hyrax_digest_v1(
        E_FIRST_RECORD_V1 as u8,
        opening(ZkAmsMkheRnsNativeFamilyV1::E, 0),
        point_set,
        &terminal,
    )
    .expect("E0");
    let e1 = opening_hyrax_digest_v1(
        E_FIRST_RECORD_V1 as u8 + 1,
        opening(ZkAmsMkheRnsNativeFamilyV1::E, 1),
        point_set,
        &terminal,
    )
    .expect("E1");
    let re = opening_hyrax_digest_v1(
        RE_RECORD_V1 as u8,
        opening(ZkAmsMkheRnsNativeFamilyV1::RE, 0),
        point_set,
        &terminal,
    )
    .expect("rE");
    let w = opening_hyrax_digest_v1(
        W_FIRST_RECORD_V1 as u8,
        opening(ZkAmsMkheRnsNativeFamilyV1::W, 0),
        point_set,
        &terminal,
    )
    .expect("W");
    let rw = opening_hyrax_digest_v1(
        RW_RECORD_V1 as u8,
        opening(ZkAmsMkheRnsNativeFamilyV1::RW, 0),
        point_set,
        &terminal,
    )
    .expect("rW");
    assert_eq!(
        [e0, e1, re, w, rw]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        5
    );
    let mut mutated = terminal.clone();
    mutated[0] = points[1];
    assert_ne!(
        e0,
        opening_hyrax_digest_v1(
            E_FIRST_RECORD_V1 as u8,
            opening(ZkAmsMkheRnsNativeFamilyV1::E, 0),
            point_set,
            &mutated,
        )
        .expect("point mutation")
    );
    assert_ne!(
        e0,
        opening_hyrax_digest_v1(
            E_FIRST_RECORD_V1 as u8,
            opening(ZkAmsMkheRnsNativeFamilyV1::E, 0),
            digest(204),
            &mutated,
        )
        .expect("set mutation")
    );
    let substituted = ZkAmsMkheRnsNativeOpeningCommitmentV1::new(
        ZkAmsMkheRnsNativeFamilyV1::E,
        0,
        digest(205),
        placeholder,
    )
    .expect("substituted source");
    assert_ne!(
        e0,
        opening_hyrax_digest_v1(E_FIRST_RECORD_V1 as u8, substituted, point_set, &terminal,)
            .expect("source substitution")
    );
}

#[test]
fn secret_commitment_equation_accepts_exact_opening_and_rejects_mutation() {
    let key = CommitmentKey::derive(b"rns-native-source-terminal-small-test", 2).expect("key");
    let values = [
        Scalar::from_u64(3),
        Scalar::from_u64(5),
        Scalar::from_u64(7),
    ];
    let terms = [
        (values[0], key.generators()[0]),
        (values[1], key.generators()[1]),
        (values[2], key.hiding_generator()),
    ];
    let expected = multiexp::<ZkAmsT256BulletproofSuiteV1>(&terms);
    verify_aggregate_commitment_for_key_v1(&values, &expected, &key).expect("exact opening");

    let mutated = expected + key.generators()[0];
    assert_eq!(
        verify_aggregate_commitment_for_key_v1(&values, &mutated, &key),
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidMapping)
    );
    assert_eq!(
        verify_aggregate_commitment_for_key_v1(&values[..2], &expected, &key),
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidGeometry)
    );
}

#[test]
fn production_boundary_is_move_only_non_authorizing_and_fail_closed() {
    let source = include_str!("rns_native_source_terminal_cross_field.rs");
    let declaration = "pub(super) struct RnsNativeSourceTerminalCrossFieldPrerequisiteV1";
    let declaration_offset = source.find(declaration).expect("stage declaration");
    let attributes = source[..declaration_offset]
        .rsplit_once("\n\n")
        .map_or(&source[..declaration_offset], |(_, block)| block);
    let stage = source[declaration_offset + declaration.len()..]
        .split_once("\n}\n")
        .map(|(body, _)| body)
        .expect("stage body");
    assert!(!attributes.contains("derive(Clone"));
    assert!(!attributes.contains("derive(Copy"));
    assert!(!stage.contains("pub fn"));
    assert!(source.contains("SecretMultiexpBuilder::<ZkAmsT256BulletproofSuiteV1>"));
    assert!(source.contains("visit_rehydrated_t256_coefficients_used_slots_with_workspace_v1"));
    assert!(!source.contains("trait RnsNativeSourceTerminal"));
    assert!(!stage.contains("Verified"));
    assert!(!stage.contains("Release"));

    let packing = include_str!("packing.rs");
    let rehydration = packing
        .split_once("fn visit_rehydrated_t256_coefficients_used_slots_with_workspace_v1(")
        .expect("rehydration adapter")
        .1
        .split_once("fn visit_validated_packed_plaintext_used_slots_with_workspace_v1(")
        .expect("rehydration adapter end")
        .0;
    assert!(
        rehydration.find("let mut packed").expect("wiping owner")
            < rehydration
                .find("validate_layout(layout)?")
                .expect("layout guard")
    );

    let composite = include_str!("rns_native_composite_verifier.rs");
    assert!(composite.contains("StageUnavailable"));
    assert!(composite.contains("CrossFieldGlobalLookup"));
}
