//! Native source replay, exact mixed-role anchor, and terminal bridge regressions.

use super::*;
use crate::vega::{
    derive_t256_generators_v1,
    zk_ams::mkhe::{
        packing::encode_zk_ams_t256_packed_plaintext_v1,
        rns_native_profile::{
            zk_ams_mkhe_rns_native_profile_v1, zk_ams_mkhe_rns_native_release_candidate_digest_v1,
            zk_ams_mkhe_rns_native_topology_v1,
        },
        rns_native_source::{ZkAmsMkheRnsNativeSourceErrorV1, ZkAmsMkheRnsNativeSourceLayoutV1},
    },
};

struct PackedSourceChunkV1 {
    bytes: Vec<u8>,
}

impl ZkAmsMkheRnsNativeSecretChunkV1 for PackedSourceChunkV1 {
    fn arena(&self) -> ZkAmsMkheRnsNativeSourceArenaV1 {
        ZkAmsMkheRnsNativeSourceArenaV1::Main
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
}

impl Drop for PackedSourceChunkV1 {
    fn drop(&mut self) {
        self.bytes.fill(0);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
}

struct PackedSourceSnapshotV1 {
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    coefficients: Vec<[u8; CANONICAL_COEFFICIENT_BYTES_V1]>,
    reads: usize,
    record: usize,
}

impl ZkAmsMkheRnsNativeSourceSnapshotV1 for PackedSourceSnapshotV1 {
    type Chunk = PackedSourceChunkV1;

    fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1 {
        self.layout
    }

    fn snapshot_digest(&self, arena: ZkAmsMkheRnsNativeSourceArenaV1) -> [u8; DIGEST_BYTES_V1] {
        match arena {
            ZkAmsMkheRnsNativeSourceArenaV1::Main => digest(246),
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce => digest(247),
        }
    }

    fn read_slot(
        &mut self,
        arena: ZkAmsMkheRnsNativeSourceArenaV1,
        slot: u64,
    ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1> {
        let record_base =
            self.record * ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_BLOCKS_PER_OPENING_V1 as usize;
        let block = usize::try_from(slot)
            .ok()
            .and_then(|slot| slot.checked_sub(record_base))
            .ok_or(ZkAmsMkheRnsNativeSourceErrorV1::Storage)?;
        if arena != ZkAmsMkheRnsNativeSourceArenaV1::Main || block >= CANONICAL_BLOCKS_PER_RECORD_V1
        {
            return Err(ZkAmsMkheRnsNativeSourceErrorV1::Storage);
        }
        let start = block * CANONICAL_COEFFICIENTS_PER_BLOCK_V1;
        let end = start + CANONICAL_COEFFICIENTS_PER_BLOCK_V1;
        let coefficients = self
            .coefficients
            .get(start..end)
            .ok_or(ZkAmsMkheRnsNativeSourceErrorV1::Storage)?;
        let mut bytes =
            Vec::with_capacity(ZK_AMS_MKHE_RNS_NATIVE_SOURCE_MAIN_PLAINTEXT_BYTES_V1 as usize);
        for coefficient in coefficients {
            bytes.extend_from_slice(coefficient);
        }
        self.reads += 1;
        Ok(PackedSourceChunkV1 { bytes })
    }
}

impl Drop for PackedSourceSnapshotV1 {
    fn drop(&mut self) {
        for coefficient in &mut self.coefficients {
            coefficient.fill(0);
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
}

fn digest(label: u8) -> [u8; DIGEST_BYTES_V1] {
    let mut hash = Keccak256::new();
    hash.update(b"rns-native-source-terminal-cross-field-test");
    hash.update(&[label]);
    hash.finalize()
}

fn proof_digest_v1(label: u8) -> ProofDigestV1 {
    super::super::rns_native_proof_hash::test_proof_digest_v1(
        b"rns-native-source-terminal-cross-field-test",
        u64::from(label),
    )
}

fn anchor_identity_v1(position: usize, label: u8) -> DigestIdentityV1 {
    if anchor_native_position_v1(position) {
        proof_digest_v1(label).into()
    } else {
        digest(label).into()
    }
}

fn mutate_proof_lane_v1(digest: ProofDigestV1, lane: usize) -> ProofDigestV1 {
    let mut bytes = digest.to_le_bytes();
    let word = &mut bytes[lane * 8..(lane + 1) * 8];
    let was_zero = word.iter().all(|byte| *byte == 0);
    word.fill(0);
    word[0] = u8::from(was_zero);
    ProofDigestV1::from_le_bytes(bytes).expect("canonical single-lane mutation")
}

fn packed_partial_snapshot_v1(
    record: usize,
    used_slots: u32,
    nonzero_tail_slot: Option<usize>,
) -> PackedSourceSnapshotV1 {
    let profile = zk_ams_mkhe_rns_native_profile_v1().expect("profile");
    let topology = zk_ams_mkhe_rns_native_topology_v1().expect("topology");
    let release = zk_ams_mkhe_rns_native_release_candidate_digest_v1().expect("candidate");
    let layout = ZkAmsMkheRnsNativeSourceLayoutV1::new(
        profile.profile_digest,
        topology.topology_digest,
        release,
        digest(244),
        digest(245),
    )
    .expect("source layout");
    let full_layout = zk_ams_t256_packing_layout_v1(ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1 as u32)
        .expect("full packing layout");
    let mut slots = vec![[0_u8; 32]; ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1];
    for (slot, value) in slots[..used_slots as usize].iter_mut().enumerate() {
        *value = Scalar::from_u64(slot as u64 + 1).to_be_bytes();
    }
    if let Some(slot) = nonzero_tail_slot {
        assert!((used_slots as usize..ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1).contains(&slot));
        slots[slot] = Scalar::from_u64(10_001).to_be_bytes();
    }
    let mut packed = encode_zk_ams_t256_packed_plaintext_v1(full_layout, 0, &slots)
        .expect("canonical packed source record");
    PackedSourceSnapshotV1 {
        layout,
        coefficients: core::mem::take(&mut packed.coefficients),
        reads: 0,
        record,
    }
}

fn anchor_core(downstream: &[u8]) -> [DigestIdentityV1; ANCHOR_CORE_DIGESTS_V1] {
    let mut core = core::array::from_fn(|index| anchor_identity_v1(index, index as u8 + 1));
    core[CORE_DOWNSTREAM_V1] = downstream_digest_v1(downstream).into();
    assert!(!core[..CORE_DOWNSTREAM_V1].contains(&core[CORE_DOWNSTREAM_V1]));
    core
}

fn encode_anchor(core: [DigestIdentityV1; ANCHOR_CORE_DIGESTS_V1], downstream: &[u8]) -> Vec<u8> {
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
    bytes.extend_from_slice(&(downstream.len() as u32).to_be_bytes());
    for digest in core {
        bytes.extend_from_slice(digest.as_bytes());
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
        mutation[index] = anchor_identity_v1(index, 220 + index as u8);
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

    for offset in [0_usize, 4, 5, 6, 7, 8, 9, 10, 12] {
        let mut mutation = encoded.clone();
        mutation[offset] ^= 1;
        assert!(ResidualAnchorV1::from_canonical_bytes_exact_v1(&mutation).is_err());
    }
    let mut bad_length = encoded.clone();
    bad_length[14..18].copy_from_slice(&(downstream.len() as u32 + 1).to_be_bytes());
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
fn x_padding_replay_accepts_exact_89_used_slots_from_the_live_source_owner() {
    let mut snapshot = packed_partial_snapshot_v1(X_RECORD_V1, X_USED_SLOTS_V1, None);
    let layout = zk_ams_t256_packing_layout_v1(X_USED_SLOTS_V1).expect("X layout");
    let mut workspace = T256PackedPlaintextDecodeWorkspaceV1::try_new_v1().expect("workspace");
    let mut visited = 0_usize;
    replay_record_v1(
        &mut snapshot,
        X_RECORD_V1,
        layout,
        &mut workspace,
        |slot, value| {
            assert_eq!(slot, visited);
            assert_eq!(value, Scalar::from_u64(slot as u64 + 1));
            visited += 1;
            Ok(())
        },
    )
    .expect("exact X used slots");
    assert_eq!(visited, X_USED_SLOTS_V1 as usize);
    assert_eq!(snapshot.reads, CANONICAL_BLOCKS_PER_RECORD_V1);
}

#[test]
fn x_padding_replay_rejects_the_first_nonzero_governed_tail_slot() {
    let mut snapshot =
        packed_partial_snapshot_v1(X_RECORD_V1, X_USED_SLOTS_V1, Some(X_USED_SLOTS_V1 as usize));
    let layout = zk_ams_t256_packing_layout_v1(X_USED_SLOTS_V1).expect("X layout");
    let mut workspace = T256PackedPlaintextDecodeWorkspaceV1::try_new_v1().expect("workspace");
    let mut visited = 0_usize;
    assert_eq!(
        replay_record_v1(
            &mut snapshot,
            X_RECORD_V1,
            layout,
            &mut workspace,
            |_slot, _value| {
                visited += 1;
                Ok(())
            },
        ),
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidPacking)
    );
    assert_eq!(visited, 0, "padding is checked before used slots escape");
    assert_eq!(snapshot.reads, CANONICAL_BLOCKS_PER_RECORD_V1);
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
    let seed = proof_digest_v1(101);
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
        derive_mapping_challenge_v1(proof_digest_v1(105), formula, openings, points),
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

    let ordered: [ProofDigestV1; 5] = core::array::from_fn(|i| proof_digest_v1(120 + i as u8));
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
    zero[0] = ProofDigestV1::ZERO;
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
    assert!(!source.contains("rns_native_zero_padding_commitment"));
    assert!(!source.contains("trait RnsNativeSourceTerminal"));
    assert!(!stage.contains("Verified"));
    assert!(!stage.contains("Release"));
    assert!(stage.contains("source: RnsNativeRlweSourceStatementStageV1"));
    assert!(stage.contains("terminal: RnsNativeTerminalCrossBasisKernelPrerequisiteV1"));
    assert!(!stage.contains("zero_padding"));

    let replay = source
        .split_once("fn replay_source_terminal_aggregate_v1")
        .expect("source replay")
        .1
        .split_once("fn terminal_coordinate_v1")
        .expect("source replay end")
        .0;
    let x = replay.find("X_RECORD_V1").expect("X padding replay");
    let e = replay.find("E_FIRST_RECORD_V1").expect("E replay");
    let re = replay.find("RE_RECORD_V1").expect("rE replay");
    let w = replay.find("W_FIRST_RECORD_V1").expect("W replay");
    let rw = replay.find("RW_RECORD_V1").expect("rW replay");
    assert!(x < e && e < re && re < w && w < rw);
    assert_eq!((X_RECORD_V1, X_USED_SLOTS_V1), (0, 89));
    assert_eq!((RE_RECORD_V1, RE_USED_SLOTS_V1), (33, 1_024));
    assert_eq!((RW_RECORD_V1, RW_USED_SLOTS_V1), (42, 512));

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

#[test]
fn all_partial_source_records_reject_each_governed_tail_before_callback() {
    for (record, used) in [
        (X_RECORD_V1, X_USED_SLOTS_V1),
        (RE_RECORD_V1, RE_USED_SLOTS_V1),
        (RW_RECORD_V1, RW_USED_SLOTS_V1),
    ] {
        let layout = zk_ams_t256_packing_layout_v1(used).expect("partial layout");
        let mut workspace = T256PackedPlaintextDecodeWorkspaceV1::try_new_v1().expect("workspace");
        let mut snapshot = packed_partial_snapshot_v1(record, used, None);
        let mut visited = 0;
        replay_record_v1(
            &mut snapshot,
            record,
            layout,
            &mut workspace,
            |slot, value| {
                assert_eq!(slot, visited);
                assert_eq!(value, Scalar::from_u64(slot as u64 + 1));
                visited += 1;
                Ok(())
            },
        )
        .expect("exact live source");
        assert_eq!(visited, used as usize);
        assert_eq!(snapshot.reads, CANONICAL_BLOCKS_PER_RECORD_V1);
        for tail in [
            used as usize,
            (used as usize + ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1) / 2,
            ZK_AMS_MKHE_RELEASE_SLOT_COUNT_V1 - 1,
        ] {
            let mut snapshot = packed_partial_snapshot_v1(record, used, Some(tail));
            let mut escaped = 0;
            assert_eq!(
                replay_record_v1(&mut snapshot, record, layout, &mut workspace, |_, _| {
                    escaped += 1;
                    Ok(())
                }),
                Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidPacking),
                "record={record}, tail={tail}",
            );
            assert_eq!(escaped, 0, "full validation precedes any callback");
            assert_eq!(snapshot.reads, CANONICAL_BLOCKS_PER_RECORD_V1);
        }
    }
}

#[test]
fn anchor_native_roles_reject_retired_widths_and_noncanonical_lanes() {
    let downstream = b"opaque-cross-field-proof-continuation";
    let core = anchor_core(downstream);
    let encoded = encode_anchor(core, downstream);
    assert_eq!(ANCHOR_FIXED_BYTES_V1, 610);
    assert_eq!(
        core.iter()
            .filter(|digest| matches!(digest, DigestIdentityV1::Proof384(_)))
            .count(),
        5
    );
    let mut offset = ANCHOR_HEADER_BYTES_V1;
    for (ordinal, identity) in core.iter().enumerate() {
        if anchor_native_position_v1(ordinal) {
            for lane in 0..6 {
                let mut mutation = encoded.clone();
                mutation[offset + lane * 8..offset + (lane + 1) * 8].fill(0xff);
                assert_eq!(
                    ResidualAnchorV1::from_canonical_bytes_exact_v1(&mutation),
                    Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor)
                );
            }
            let mut retired_width = encoded.clone();
            retired_width.drain(offset + DIGEST_BYTES_V1..offset + PROOF_DIGEST_BYTES_V1);
            assert!(ResidualAnchorV1::from_canonical_bytes_exact_v1(&retired_width).is_err());
        }
        offset += identity.as_bytes().len();
    }
    assert_eq!(offset, ANCHOR_FIXED_BYTES_V1);
    let mut retired = encoded[..ANCHOR_HEADER_BYTES_V1].to_vec();
    for identity in core {
        retired.extend_from_slice(&identity.as_bytes()[..32]);
    }
    retired.extend_from_slice(downstream);
    assert_eq!(retired.len(), 530 + downstream.len());
    assert!(ResidualAnchorV1::from_canonical_bytes_exact_v1(&retired).is_err());
    for removed_magic in [b"ZZPC", b"ZAZP"] {
        let mut removed = encoded.clone();
        removed[..4].copy_from_slice(removed_magic);
        assert_eq!(
            ResidualAnchorV1::from_canonical_bytes_exact_v1(&removed),
            Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor)
        );
    }
}

#[test]
fn anchor_core_and_curve_challenge_bind_every_native_lane() {
    let downstream = b"full-width-anchor";
    let core = anchor_core(downstream);
    let encoded = encode_anchor(core, downstream);
    let decoded = ResidualAnchorV1::from_canonical_bytes_exact_v1(&encoded).expect("anchor");
    for (ordinal, identity) in core.iter().enumerate() {
        if let DigestIdentityV1::Proof384(proof) = identity {
            for lane in 0..6 {
                let mut mutation = core;
                mutation[ordinal] = mutate_proof_lane_v1(*proof, lane).into();
                assert_eq!(
                    validate_anchor_core_v1(decoded, mutation),
                    Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor)
                );
            }
        }
    }
    let seed = proof_digest_v1(81);
    let args = [digest(82), digest(83), digest(84)];
    let scalar = derive_mapping_challenge_v1(seed, args[0], args[1], args[2]).expect("scalar");
    for lane in 0..6 {
        let changed = derive_mapping_challenge_v1(
            mutate_proof_lane_v1(seed, lane),
            args[0],
            args[1],
            args[2],
        )
        .expect("changed scalar");
        assert_ne!(scalar, changed);
    }
    let ordered = [proof_digest_v1(85), proof_digest_v1(86)];
    let bundle = indexed_digest_bundle_v1(POINT_BUNDLE_DOMAIN_V1, &ordered).expect("bundle");
    for lane in 0..6 {
        let mut mutation = ordered;
        mutation[1] = mutate_proof_lane_v1(mutation[1], lane);
        assert_ne!(
            bundle,
            indexed_digest_bundle_v1(POINT_BUNDLE_DOMAIN_V1, &mutation).expect("bundle")
        );
    }
}

#[test]
fn mixed_registry_distinguishes_roles_without_losing_duplicate_checks() {
    let proof = proof_digest_v1(71);
    let public: [u8; 32] = proof.as_bytes()[..32]
        .try_into()
        .expect("public prefix fixture");
    let mut registry = DigestRegistryV1::new();
    registry.insert(public).expect("public identity");
    registry.insert(proof).expect("distinct full native role");
    assert_eq!(
        registry.insert(public),
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::AliasedDigest)
    );
    assert_eq!(
        registry.insert(proof),
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::AliasedDigest)
    );
    assert_eq!(
        registry.insert([0_u8; 32]),
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::AliasedDigest)
    );
    assert_eq!(
        registry.insert(ProofDigestV1::ZERO),
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::AliasedDigest)
    );
}

#[test]
fn mapping_roots_use_complete_shared_frames_and_bind_each_actual_axis() {
    let public = core::array::from_fn(|i| digest(10 + i as u8));
    let curve = core::array::from_fn(|i| digest(30 + i as u8));
    let seed = proof_digest_v1(50);
    let challenge = Scalar::from_u64(17);
    let actual = mapping_roots_v1(&public, seed, &curve, challenge).expect("native roots");
    let context = RnsNativeProofHashContextV1::canonical().expect("canonical context");
    // Independent explicit shared-owner oracle fixes the exact field ordering.
    let mut mapping_fields: Vec<&[u8]> = vec![MAPPING_ROOT_DOMAIN_V1, &[1]];
    for axis in &public {
        mapping_fields.push(axis);
    }
    mapping_fields.push(seed.as_bytes());
    for axis in &curve {
        mapping_fields.push(axis);
    }
    let scalar = challenge.to_be_bytes();
    mapping_fields.push(&scalar);
    assert_eq!(mapping_fields.len(), 17);
    let mapping = context
        .hash(
            RnsNativeProofHashRoleV1::TerminalBridge,
            RnsNativeProofHashPhaseV1::Binding,
            RnsNativeProofHashPositionV1 {
                level: 3,
                index: 0,
                counter: 0,
            },
            &mapping_fields,
        )
        .expect("mapping oracle");
    let terminal = context
        .hash(
            RnsNativeProofHashRoleV1::TerminalBridge,
            RnsNativeProofHashPhaseV1::Binding,
            RnsNativeProofHashPositionV1 {
                level: 3,
                index: 1,
                counter: 0,
            },
            &[
                TERMINAL_ROOT_DOMAIN_V1,
                &[1],
                mapping.as_bytes(),
                &curve[1],
                &curve[2],
                &curve[3],
            ],
        )
        .expect("terminal oracle");
    assert_eq!(actual, (mapping, terminal));
    assert_ne!(mapping, terminal);
    for index in 0..public.len() {
        let mut changed = public;
        changed[index] = digest(100 + index as u8);
        let roots = mapping_roots_v1(&changed, seed, &curve, challenge).expect("public mutation");
        assert_ne!(actual.0, roots.0);
        assert_ne!(actual.1, roots.1);
    }
    for index in 0..curve.len() {
        let mut changed = curve;
        changed[index] = digest(120 + index as u8);
        let roots = mapping_roots_v1(&public, seed, &changed, challenge).expect("curve mutation");
        assert_ne!(actual.0, roots.0);
        assert_ne!(actual.1, roots.1);
    }
    for lane in 0..6 {
        let roots = mapping_roots_v1(&public, mutate_proof_lane_v1(seed, lane), &curve, challenge)
            .expect("seed mutation");
        assert_ne!(actual.0, roots.0);
        assert_ne!(actual.1, roots.1);
    }
    let scalar_mutation =
        mapping_roots_v1(&public, seed, &curve, Scalar::from_u64(18)).expect("scalar mutation");
    assert_ne!(actual.0, scalar_mutation.0);
    assert_ne!(actual.1, scalar_mutation.1);
    for position in [
        RnsNativeProofHashPositionV1 {
            level: 2,
            index: 0,
            counter: 0,
        },
        RnsNativeProofHashPositionV1 {
            level: 3,
            index: 1,
            counter: 0,
        },
        RnsNativeProofHashPositionV1 {
            level: 3,
            index: 0,
            counter: 1,
        },
    ] {
        assert_ne!(
            mapping,
            context
                .hash(
                    RnsNativeProofHashRoleV1::TerminalBridge,
                    RnsNativeProofHashPhaseV1::Binding,
                    position,
                    &mapping_fields
                )
                .expect("position mutant")
        );
    }
}

#[test]
fn removed_fixed_marker_pair_is_rejected_under_the_only_final_header() {
    assert_eq!(ANCHOR_HEADER_BYTES_V1, 18);
    assert_eq!(ANCHOR_FIXED_BYTES_V1, 610);
    for downstream in [
        b"x".as_slice(),
        b"exact-cross-field-continuation".as_slice(),
    ] {
        let core = anchor_core(downstream);
        let encoded = encode_anchor(core, downstream);
        assert_eq!(
            u32::from_be_bytes(encoded[14..18].try_into().expect("length")) as usize,
            downstream.len()
        );
        let decoded =
            ResidualAnchorV1::from_canonical_bytes_exact_v1(&encoded).expect("final header");
        assert_eq!(decoded.core, core);
        assert_eq!(decoded.downstream, downstream);
        // Reconstruct the exact superseded two marker bytes at their former
        // offset without changing any committed identity or downstream byte.
        let mut retired = encoded.clone();
        retired.splice(14..14, [4, 0]);
        assert_eq!(retired.len(), encoded.len() + 2);
        assert_eq!(
            ResidualAnchorV1::from_canonical_bytes_exact_v1(&retired),
            Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor)
        );
    }
    // Both forms remain below the same parent's cap for this mutation; failure
    // comes from exact final grammar, not the outer byte ceiling.
    let near_maximum = vec![0x5a; LINK_DOWNSTREAM_MAX_BYTES_V1 - 2];
    let mut retired = encode_anchor(anchor_core(&near_maximum), &near_maximum);
    retired.splice(14..14, [4, 0]);
    assert_eq!(
        retired.len(),
        RNS_NATIVE_RLWE_SOURCE_DOWNSTREAM_MAX_BYTES_V1
    );
    assert_eq!(
        ResidualAnchorV1::from_canonical_bytes_exact_v1(&retired),
        Err(RnsNativeSourceTerminalCrossFieldErrorV1::InvalidAnchor)
    );
}
