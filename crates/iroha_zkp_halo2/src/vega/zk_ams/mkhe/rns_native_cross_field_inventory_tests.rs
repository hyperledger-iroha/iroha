use super::*;
use crate::{
    vega::derive_t256_generators_v1,
    vega::zk_ams::mkhe::{
        rns_native_cross_field_rlwe_direct::{
            RnsNativeCrossFieldPreQpcsSafeAxesV1, RnsNativeCrossFieldRlweDirectErrorV1,
            RnsNativeQMaskSCommitmentSourceV1, q_mask_s_root_v1,
        },
        rns_native_profile::{
            ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1, ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1,
            ZK_AMS_MKHE_RNS_NATIVE_SUMCHECK_ROUNDS_V1, ZkAmsMkheRnsNativeFamilyV1,
            zk_ams_mkhe_rns_native_profile_v1, zk_ams_mkhe_rns_native_release_candidate_digest_v1,
            zk_ams_mkhe_rns_native_topology_v1,
        },
        rns_native_section_codec::{
            CROSS_LOOKUP_PROOF_OFFSET_V1, ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1,
            ZkAmsMkheRnsNativeTerminalBridgeSectionV1, ZkAmsMkheRnsNativeZeroPaddingSectionV1,
            preflight_rns_native_cross_field_global_lookup_from_envelope_v1,
        },
        rns_native_source::{
            ZkAmsMkheRnsNativeSecretChunkV1, ZkAmsMkheRnsNativeSourceArenaV1,
            ZkAmsMkheRnsNativeSourceErrorV1, ZkAmsMkheRnsNativeSourceLayoutV1,
            ZkAmsMkheRnsNativeSourceSnapshotV1,
        },
        rns_native_transcript::{
            ZkAmsMkheRnsNativeOpeningCommitmentV1, ZkAmsMkheRnsNativeOpeningCommitmentsV1,
            ZkAmsMkheRnsNativePublicContextV1, ZkAmsMkheRnsNativeQpcsFriRootV1,
            ZkAmsMkheRnsNativeQpcsRootsV1, ZkAmsMkheRnsNativeTerminalBridgeV1,
            ZkAmsMkheRnsNativeTerminalRootsV1, ZkAmsMkheRnsNativeTranscriptV1,
        },
        rns_native_wire::{
            ZkAmsMkheRnsNativeProofEnvelopeV1, ZkAmsMkheRnsNativeProofSectionKindV1,
        },
    },
};

fn point_bytes_v1() -> [u8; POINT_BYTES_V1] {
    let point =
        derive_t256_generators_v1(b"rns-native-cross-field-inventory-test", 1).expect("point")[0];
    let mut encoded = [0_u8; POINT_BYTES_V1];
    point
        .write_non_identity_wire_bytes_ref(&mut encoded)
        .expect("canonical point");
    encoded
}

fn alternate_point_bytes_v1() -> [u8; POINT_BYTES_V1] {
    let point = derive_t256_generators_v1(
        b"rns-native-cross-field-inventory-pre-direct-candidate-test",
        1,
    )
    .expect("alternate point")[0];
    let mut encoded = [0_u8; POINT_BYTES_V1];
    point
        .write_non_identity_wire_bytes_ref(&mut encoded)
        .expect("canonical alternate point");
    assert_ne!(encoded, point_bytes_v1());
    encoded
}

fn replace_inventory_point_v1(inventory: &mut [u8], ordinal: usize, encoded: [u8; POINT_BYTES_V1]) {
    let offset = ordinal * POINT_BYTES_V1;
    inventory[offset..offset + POINT_BYTES_V1].copy_from_slice(&encoded);
}

fn canonical_inventory_v1() -> Vec<u8> {
    let point = point_bytes_v1();
    let mut inventory = Vec::with_capacity(INVENTORY_BYTES_V1);
    for _ in 0..INVENTORY_POINTS_V1 {
        inventory.extend_from_slice(&point);
    }
    assert_eq!(inventory.len(), INVENTORY_BYTES_V1);
    inventory
}

fn canonical_wire_with_inventory_v1(
    prior_context_digest: [u8; DIGEST_BYTES_V1],
    continuation: &[u8],
    inventory: &[u8],
) -> Vec<u8> {
    assert_eq!(inventory.len(), INVENTORY_BYTES_V1);
    let inventory_root = canonical_inventory_root_v1(prior_context_digest, inventory)
        .expect("inventory root")
        .inventory_root;
    let continuation_digest =
        canonical_continuation_digest_v1(prior_context_digest, inventory_root, continuation)
            .expect("continuation digest");
    let total = HEADER_BYTES_V1 + inventory.len() + continuation.len() + CODEC_DIGEST_BYTES_V1;
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(&INVENTORY_MAGIC_V1);
    bytes.push(INVENTORY_VERSION_V1);
    bytes.push(INVENTORY_FLAGS_V1);
    bytes.extend_from_slice(&(HEADER_BYTES_V1 as u16).to_be_bytes());
    bytes.extend_from_slice(&(total as u32).to_be_bytes());
    bytes.extend_from_slice(&[
        ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u8,
        REPETITIONS_V1 as u8,
        RECORDS_V1 as u8,
        BLOCKS_PER_RECORD_V1 as u8,
        ZK_AMS_MKHE_RNS_NATIVE_RADIX_LOG2_V1,
        RADIX_DIGITS_V1 as u8,
        Q_MASK_DIGITS_V1 as u8,
        POINT_BYTES_V1 as u8,
    ]);
    for count in [
        COMPARATOR_POINTS_V1,
        SMALL_SOURCE_POINTS_V1,
        Q_MASK_POINTS_V1,
        INVENTORY_POINTS_V1,
    ] {
        bytes.extend_from_slice(&(count as u32).to_be_bytes());
    }
    bytes.extend_from_slice(&prior_context_digest);
    bytes.extend_from_slice(&inventory_root);
    bytes.extend_from_slice(&continuation_digest);
    bytes.extend_from_slice(&(continuation.len() as u32).to_be_bytes());
    assert_eq!(bytes.len(), HEADER_BYTES_V1);
    bytes.extend_from_slice(inventory);
    bytes.extend_from_slice(continuation);
    let codec_digest = codec_digest_v1(&bytes);
    bytes.extend_from_slice(&codec_digest);
    assert_eq!(bytes.len(), total);
    bytes
}

fn canonical_wire_v1(prior_context_digest: [u8; DIGEST_BYTES_V1], continuation: &[u8]) -> Vec<u8> {
    canonical_wire_with_inventory_v1(
        prior_context_digest,
        continuation,
        &canonical_inventory_v1(),
    )
}

fn envelope_digest_v1(label: &[u8], context: u16, ordinal: u16) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.rns-native-cross-field-inventory.envelope-test");
    hash.update(&(label.len() as u16).to_be_bytes());
    hash.update(label);
    hash.update(&context.to_be_bytes());
    hash.update(&ordinal.to_be_bytes());
    hash.finalize()
}

fn envelope_indexed_v1<const N: usize>(label: &[u8], context: u16) -> [[u8; 32]; N] {
    core::array::from_fn(|ordinal| {
        envelope_digest_v1(
            label,
            context,
            u16::try_from(ordinal).expect("test ordinal fits u16"),
        )
    })
}

struct EnvelopeTestChunkV1 {
    arena: ZkAmsMkheRnsNativeSourceArenaV1,
    bytes: [u8; 1],
}

impl ZkAmsMkheRnsNativeSecretChunkV1 for EnvelopeTestChunkV1 {
    fn arena(&self) -> ZkAmsMkheRnsNativeSourceArenaV1 {
        self.arena
    }

    fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.bytes
    }
}

struct EnvelopeTestSnapshotV1 {
    layout: ZkAmsMkheRnsNativeSourceLayoutV1,
    context: u16,
}

impl ZkAmsMkheRnsNativeSourceSnapshotV1 for EnvelopeTestSnapshotV1 {
    type Chunk = EnvelopeTestChunkV1;

    fn layout(&self) -> ZkAmsMkheRnsNativeSourceLayoutV1 {
        self.layout
    }

    fn snapshot_digest(&self, arena: ZkAmsMkheRnsNativeSourceArenaV1) -> [u8; 32] {
        match arena {
            ZkAmsMkheRnsNativeSourceArenaV1::Main => {
                envelope_digest_v1(b"main-snapshot", self.context, 0)
            }
            ZkAmsMkheRnsNativeSourceArenaV1::Nonce => {
                envelope_digest_v1(b"nonce-snapshot", self.context, 0)
            }
        }
    }

    fn read_slot(
        &mut self,
        _arena: ZkAmsMkheRnsNativeSourceArenaV1,
        _slot: u64,
    ) -> Result<Self::Chunk, ZkAmsMkheRnsNativeSourceErrorV1> {
        Err(ZkAmsMkheRnsNativeSourceErrorV1::Storage)
    }
}

fn envelope_opening_role_v1(ordinal: usize) -> (ZkAmsMkheRnsNativeFamilyV1, u8) {
    match ordinal {
        0 => (ZkAmsMkheRnsNativeFamilyV1::X, 0),
        1..=16 => (ZkAmsMkheRnsNativeFamilyV1::U, (ordinal - 1) as u8),
        17..=32 => (ZkAmsMkheRnsNativeFamilyV1::E, (ordinal - 17) as u8),
        33 => (ZkAmsMkheRnsNativeFamilyV1::RE, 0),
        34..=41 => (ZkAmsMkheRnsNativeFamilyV1::W, (ordinal - 34) as u8),
        42 => (ZkAmsMkheRnsNativeFamilyV1::RW, 0),
        _ => panic!("opening ordinal is canonical"),
    }
}

fn sealed_envelope_fixture_v1(
    context: u16,
    cross_proof: &[u8],
) -> (
    ZkAmsMkheRnsNativeProofEnvelopeV1,
    ZkAmsMkheRnsNativeChallengeSeedsV1,
) {
    let profile = zk_ams_mkhe_rns_native_profile_v1().expect("profile");
    let topology = zk_ams_mkhe_rns_native_topology_v1().expect("topology");
    let release = zk_ams_mkhe_rns_native_release_candidate_digest_v1().expect("candidate");
    let layout = ZkAmsMkheRnsNativeSourceLayoutV1::new(
        profile.profile_digest,
        topology.topology_digest,
        release,
        envelope_digest_v1(b"statement", context, 0),
        envelope_digest_v1(b"operation", context, 0),
    )
    .expect("layout");
    let receipt = EnvelopeTestSnapshotV1 { layout, context }
        .structural_receipt()
        .expect("receipt");
    let public = ZkAmsMkheRnsNativePublicContextV1::new(
        envelope_digest_v1(b"roster", context, 0),
        envelope_digest_v1(b"ciphertext", context, 0),
    )
    .expect("public context");
    let transcript =
        ZkAmsMkheRnsNativeTranscriptV1::new(layout, receipt, public).expect("initial transcript");
    let openings = core::array::from_fn(|ordinal| {
        let (family, family_index) = envelope_opening_role_v1(ordinal);
        ZkAmsMkheRnsNativeOpeningCommitmentV1::new(
            family,
            family_index,
            envelope_digest_v1(b"source-opening", context, ordinal as u16),
            envelope_digest_v1(b"hyrax-opening", context, ordinal as u16),
        )
        .expect("opening")
    });
    let openings =
        ZkAmsMkheRnsNativeOpeningCommitmentsV1::new(transcript.binding_digest(), openings)
            .expect("opening bundle");
    let transcript = transcript
        .bind_opening_commitments(openings)
        .expect("opening transcript");
    let bridge = ZkAmsMkheRnsNativeTerminalBridgeV1::new(
        transcript.binding_digest(),
        envelope_digest_v1(b"mapping-root", context, 0),
        envelope_digest_v1(b"hyrax-root", context, 0),
        envelope_digest_v1(b"cross-basis-root", context, 0),
    )
    .expect("terminal bridge");
    let transcript = transcript
        .bind_terminal_bridge(bridge)
        .expect("terminal transcript");
    let fri_roots = core::array::from_fn(|layer| {
        ZkAmsMkheRnsNativeQpcsFriRootV1::new(
            layer as u8,
            envelope_digest_v1(b"fri-root", context, layer as u16),
        )
        .expect("FRI root")
    });
    let qpcs = ZkAmsMkheRnsNativeQpcsRootsV1::new(
        transcript.binding_digest(),
        envelope_digest_v1(b"qpcs-initial", context, 0),
        envelope_digest_v1(b"q-mask-s-root", context, 0),
        envelope_digest_v1(b"qpcs-quotient", context, 0),
        fri_roots,
    )
    .expect("qPCS roots");
    let transcript = transcript.bind_qpcs_roots(qpcs).expect("qPCS transcript");
    let roots = ZkAmsMkheRnsNativeTerminalRootsV1::new(
        transcript.binding_digest(),
        envelope_digest_v1(b"cross-root", context, 0),
        envelope_digest_v1(b"global-root", context, 0),
        envelope_digest_v1(b"zero-root", context, 0),
    )
    .expect("terminal roots");
    let seeds = transcript
        .bind_terminal_roots(roots)
        .expect("final transcript");

    let terminal = ZkAmsMkheRnsNativeTerminalBridgeSectionV1::new(&seeds, b"terminal")
        .expect("terminal section")
        .to_canonical_bytes_v1()
        .expect("terminal encoding");
    let equations: [[u8; 32]; 2] = envelope_indexed_v1(b"equation", context);
    let qpcs_limbs: [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1] =
        envelope_indexed_v1(b"qpcs-limb", context);
    let queries: [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_QUERY_COUNT_V1 as usize] =
        envelope_indexed_v1(b"query", context);
    let rns = ZkAmsMkheRnsNativeRnsRelationQpcsSectionV1::new(
        &seeds,
        &equations,
        &qpcs_limbs,
        &queries,
        b"rns",
    )
    .expect("RNS section")
    .to_canonical_bytes_v1()
    .expect("RNS encoding");
    let points: [[u8; 32]; 5] = envelope_indexed_v1(b"point", context);
    let cross_limbs: [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1] =
        envelope_indexed_v1(b"cross-limb", context);
    let sumcheck: [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_SUMCHECK_ROUNDS_V1 as usize] =
        envelope_indexed_v1(b"sumcheck", context);
    let cross = ZkAmsMkheRnsNativeCrossFieldGlobalLookupSectionV1::new(
        &seeds,
        &points,
        &cross_limbs,
        &sumcheck,
        cross_proof,
    )
    .expect("cross section")
    .to_canonical_bytes_v1()
    .expect("cross encoding");
    let padding_limbs: [[u8; 32]; ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1] =
        envelope_indexed_v1(b"padding-limb", context);
    let padding = ZkAmsMkheRnsNativeZeroPaddingSectionV1::new(&seeds, &padding_limbs, b"padding")
        .expect("padding section")
        .to_canonical_bytes_v1()
        .expect("padding encoding");
    let envelope =
        ZkAmsMkheRnsNativeProofEnvelopeV1::new(layout, receipt, terminal, rns, cross, padding)
            .expect("envelope");
    assert_eq!(
        fri_roots.len(),
        ZK_AMS_MKHE_RNS_NATIVE_FRI_ROUNDS_V1 as usize
    );
    (envelope, seeds)
}

#[test]
fn forty_limb_inventory_geometry_has_one_exact_role_order() {
    assert_eq!(
        inventory_coordinate_v1(0),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::ComparatorDifferenceTop,
            owner: 0,
            column: 0,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(COMPARATOR_GROUPS_V1 - 1),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::ComparatorDifferenceTop,
            owner: COMPARATOR_GROUPS_V1 - 1,
            column: 0,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(COMPARATOR_GROUPS_V1),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::ComparatorSumTop,
            owner: 0,
            column: 0,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(COMPARATOR_TOP_POINTS_V1),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::ComparatorDifferenceDigit,
            owner: 0,
            column: 0,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(COMPARATOR_TOP_POINTS_V1 + 17),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::ComparatorMixedTop,
            owner: 0,
            column: 0,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(COMPARATOR_TOP_POINTS_V1 + 18),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::ComparatorBorrow,
            owner: 0,
            column: 0,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(
            COMPARATOR_TOP_POINTS_V1
                + (COMPARATOR_GROUPS_V1 - 1) * COMPARATOR_POINTS_PER_GROUP_V1
                + 35,
        ),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::ComparatorBorrow,
            owner: COMPARATOR_GROUPS_V1 - 1,
            column: 17,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(COMPARATOR_POINTS_V1 - 1),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::ComparatorDifferenceInverse,
            owner: COMPARATOR_GROUPS_V1 - 1,
            column: 16,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(COMPARATOR_POINTS_V1),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::SmallSigned,
            owner: 0,
            column: 0,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(COMPARATOR_POINTS_V1 + SMALL_SOURCE_POINTS_V1),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::QMaskDigit,
            owner: 0,
            column: 0,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(INVENTORY_POINTS_V1 - 1),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::QMaskComplementInverse,
            owner: Q_MASK_BLOCKS_V1 - 1,
            column: 3,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(INVENTORY_POINTS_V1),
        Err(RnsNativeCrossFieldInventoryErrorV1::InvalidGeometry)
    );
}

#[test]
fn pre_direct_candidate_role_manifest_has_exact_counts_and_excludes_every_inverse() {
    let mut role_counts = [0_usize; 15];
    for ordinal in 0..INVENTORY_POINTS_V1 {
        let coordinate = inventory_coordinate_v1(ordinal).expect("canonical coordinate");
        role_counts[coordinate.role as usize] += 1;
    }
    for (role, expected) in [
        (InventoryPointRoleV1::ComparatorDifferenceTop, 344),
        (InventoryPointRoleV1::ComparatorSumTop, 344),
        (InventoryPointRoleV1::ComparatorDifferenceDigit, 5_848),
        (InventoryPointRoleV1::ComparatorMixedTop, 344),
        (InventoryPointRoleV1::ComparatorBorrow, 6_192),
        (InventoryPointRoleV1::SmallSigned, 1_032),
        (InventoryPointRoleV1::SmallNegativeMagnitude, 1_032),
        (InventoryPointRoleV1::QMaskDigit, 6_400),
        (InventoryPointRoleV1::QMaskComplementDigit, 6_400),
    ] {
        assert!(role.is_pre_direct_candidate_v1());
        assert_eq!(role_counts[role as usize], expected);
    }
    for (role, expected) in [
        (InventoryPointRoleV1::ComparatorDifferenceInverse, 5_848),
        (InventoryPointRoleV1::SmallPositiveInverse, 1_032),
        (InventoryPointRoleV1::SmallNegativeInverse, 1_032),
        (InventoryPointRoleV1::QMaskDigitInverse, 6_400),
        (InventoryPointRoleV1::QMaskComplementInverse, 6_400),
    ] {
        assert!(!role.is_pre_direct_candidate_v1());
        assert_eq!(role_counts[role as usize], expected);
    }
    assert_eq!(PRE_DIRECT_CANDIDATE_POINTS_V1, 27_936);
    assert_eq!(PRE_DIRECT_CANDIDATE_EXCLUDED_INVERSE_POINTS_V1, 20_712);
    assert_eq!(PRE_DIRECT_CANDIDATE_POINT_BYTES_V1, 921_888);
    assert_eq!(PRE_DIRECT_CANDIDATE_FRAME_BYTES_V1, 44);
    assert_eq!(PRE_DIRECT_CANDIDATE_FRAMED_BYTES_V1, 1_229_184);
}

#[test]
fn pre_direct_candidate_root_mutates_for_every_safe_role_and_ignores_every_inverse_role() {
    let prior_context = [0x71; DIGEST_BYTES_V1];
    let inventory = canonical_inventory_v1();
    let baseline = canonical_inventory_root_v1(prior_context, &inventory).expect("baseline roots");
    let alternate = alternate_point_bytes_v1();

    for ordinal in [
        0,
        COMPARATOR_GROUPS_V1,
        COMPARATOR_TOP_POINTS_V1,
        COMPARATOR_TOP_POINTS_V1 + 17,
        COMPARATOR_TOP_POINTS_V1 + 18,
        COMPARATOR_POINTS_V1,
        COMPARATOR_POINTS_V1 + 1,
        Q_MASK_INVENTORY_FIRST_ORDINAL_V1,
        Q_MASK_INVENTORY_FIRST_ORDINAL_V1 + 8,
    ] {
        assert!(
            inventory_coordinate_v1(ordinal)
                .expect("safe coordinate")
                .role
                .is_pre_direct_candidate_v1()
        );
        let mut changed = inventory.clone();
        replace_inventory_point_v1(&mut changed, ordinal, alternate);
        let roots = canonical_inventory_root_v1(prior_context, &changed).expect("changed roots");
        assert_ne!(roots.inventory_root, baseline.inventory_root);
        assert_ne!(
            roots.pre_direct_candidate_point_root,
            baseline.pre_direct_candidate_point_root
        );
    }

    for ordinal in [
        COMPARATOR_TOP_POINTS_V1 + 36,
        COMPARATOR_POINTS_V1 + 2,
        COMPARATOR_POINTS_V1 + 3,
        Q_MASK_INVENTORY_FIRST_ORDINAL_V1 + 4,
        Q_MASK_INVENTORY_FIRST_ORDINAL_V1 + 12,
    ] {
        assert!(
            !inventory_coordinate_v1(ordinal)
                .expect("inverse coordinate")
                .role
                .is_pre_direct_candidate_v1()
        );
        let mut changed = inventory.clone();
        replace_inventory_point_v1(&mut changed, ordinal, alternate);
        let roots = canonical_inventory_root_v1(prior_context, &changed).expect("changed roots");
        assert_ne!(roots.inventory_root, baseline.inventory_root);
        assert_eq!(
            roots.pre_direct_candidate_point_root,
            baseline.pre_direct_candidate_point_root
        );
    }
}

#[test]
fn pre_direct_candidate_point_root_excludes_prior_context_and_repaired_continuation() {
    let inventory = canonical_inventory_v1();
    let first = canonical_inventory_root_v1([0x73; DIGEST_BYTES_V1], &inventory)
        .expect("first context roots");
    let second = canonical_inventory_root_v1([0x74; DIGEST_BYTES_V1], &inventory)
        .expect("second context roots");
    assert_ne!(first.inventory_root, second.inventory_root);
    assert_eq!(
        first.pre_direct_candidate_point_root,
        second.pre_direct_candidate_point_root
    );

    let prior_context = [0x75; DIGEST_BYTES_V1];
    let first_wire =
        canonical_wire_with_inventory_v1(prior_context, b"first repaired continuation", &inventory);
    let second_wire = canonical_wire_with_inventory_v1(
        prior_context,
        b"second independently repaired continuation",
        &inventory,
    );
    let first_view =
        CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(&first_wire, prior_context)
            .expect("first repaired proof");
    let second_view =
        CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(&second_wire, prior_context)
            .expect("second repaired proof");
    assert_ne!(
        first_view.continuation_digest,
        second_view.continuation_digest
    );
    assert_eq!(
        first_view.pre_direct_candidate_point_root,
        second_view.pre_direct_candidate_point_root
    );
}

#[test]
fn pre_direct_candidate_hash_is_streamed_during_the_one_canonical_point_pass() {
    let before = preflight_audit_counters_v1();
    canonical_inventory_root_v1([0x72; DIGEST_BYTES_V1], &canonical_inventory_v1())
        .expect("canonical roots");
    let after = preflight_audit_counters_v1();
    assert_eq!(
        after.inventory_root_passes - before.inventory_root_passes,
        1
    );
    assert_eq!(
        after.point_validation_decodes - before.point_validation_decodes,
        INVENTORY_POINTS_V1
    );
    assert_eq!(
        after.pre_direct_candidate_points - before.pre_direct_candidate_points,
        PRE_DIRECT_CANDIDATE_POINTS_V1
    );
    assert_eq!(
        after.pre_direct_candidate_framed_bytes - before.pre_direct_candidate_framed_bytes,
        PRE_DIRECT_CANDIDATE_FRAMED_BYTES_V1
    );
}

#[test]
fn statement8_small_source_accessor_uses_exact_raw_roles_and_derived_positive() {
    let mut inventory = canonical_inventory_v1();
    let signed = Point::from_non_identity_wire_bytes_exact(&point_bytes_v1())
        .expect("canonical signed point");
    let commitments = small_source_product_commitments_v1(&inventory, 0)
        .expect("first small-source commitment tuple");
    assert!(commitments.signed == signed);
    assert!(commitments.negative_magnitude == signed);
    assert!(commitments.positive == signed + signed);
    assert!(small_source_product_commitments_v1(&inventory, SMALL_SOURCE_BLOCKS_V1 - 1).is_some());
    assert!(small_source_product_commitments_v1(&inventory, SMALL_SOURCE_BLOCKS_V1).is_none());
    assert!(small_source_product_commitments_v1(&inventory[..inventory.len() - 1], 0).is_none());

    let mut opposite = [0_u8; POINT_BYTES_V1];
    (-signed)
        .write_non_identity_wire_bytes_ref(&mut opposite)
        .expect("canonical opposite point");
    let negative_ordinal = COMPARATOR_POINTS_V1 + 1;
    let negative_offset = negative_ordinal * POINT_BYTES_V1;
    inventory[negative_offset..negative_offset + POINT_BYTES_V1].copy_from_slice(&opposite);
    assert!(small_source_product_commitments_v1(&inventory, 0).is_none());
}

#[test]
fn statement4_subtraction_accessor_selects_only_delta_and_beta_zero_through_sixteen() {
    let mut inventory = canonical_inventory_v1();
    let points = derive_t256_generators_v1(b"rns-native-statement4-subtraction-accessor", 39)
        .expect("statement-4 accessor points");
    let write_point = |inventory: &mut [u8], ordinal: usize, point: Point| {
        let mut encoded = [0_u8; POINT_BYTES_V1];
        point
            .write_non_identity_wire_bytes_ref(&mut encoded)
            .expect("canonical statement-4 point");
        let offset = ordinal * POINT_BYTES_V1;
        inventory[offset..offset + POINT_BYTES_V1].copy_from_slice(&encoded);
    };

    let first = COMPARATOR_TOP_POINTS_V1;
    for column in 0..COMPARATOR_SUBTRACTION_DIGITS_V1 {
        write_point(&mut inventory, first + column, points[column]);
        write_point(
            &mut inventory,
            first + 18 + column,
            points[COMPARATOR_SUBTRACTION_DIGITS_V1 + column],
        );
    }
    for (local, point) in [(17, points[34]), (35, points[35]), (36, points[36])] {
        write_point(&mut inventory, first + local, point);
    }

    let first_group = comparator_subtraction_commitments_v1(&inventory, 0)
        .expect("first comparator subtraction tuple");
    assert_eq!(
        first_group.difference_digits.as_slice(),
        &points[..COMPARATOR_SUBTRACTION_DIGITS_V1]
    );
    assert_eq!(
        first_group.borrows.as_slice(),
        &points[COMPARATOR_SUBTRACTION_DIGITS_V1..2 * COMPARATOR_SUBTRACTION_DIGITS_V1]
    );
    for excluded in &points[34..37] {
        assert!(!first_group.difference_digits.contains(excluded));
        assert!(!first_group.borrows.contains(excluded));
    }

    let last =
        COMPARATOR_TOP_POINTS_V1 + (COMPARATOR_GROUPS_V1 - 1) * COMPARATOR_POINTS_PER_GROUP_V1;
    write_point(&mut inventory, last + 16, points[37]);
    write_point(&mut inventory, last + 18 + 16, points[38]);
    let last_group = comparator_subtraction_commitments_v1(&inventory, COMPARATOR_GROUPS_V1 - 1)
        .expect("last comparator subtraction tuple");
    assert_eq!(last_group.difference_digits[16], points[37]);
    assert_eq!(last_group.borrows[16], points[38]);
    assert!(comparator_subtraction_commitments_v1(&inventory, COMPARATOR_GROUPS_V1).is_none());
    assert!(comparator_subtraction_commitments_v1(&inventory[..inventory.len() - 1], 0).is_none());
}

#[test]
fn q_mask_linear_accessor_selects_digits_and_complements_but_not_inverses() {
    let mut inventory = canonical_inventory_v1();
    let points = derive_t256_generators_v1(b"rns-native-q-mask-linear-accessor", 8)
        .expect("q-mask accessor points");
    let first = COMPARATOR_POINTS_V1 + SMALL_SOURCE_POINTS_V1;
    for (local, point) in [
        (0, points[0]),
        (1, points[1]),
        (2, points[2]),
        (3, points[3]),
        (8, points[4]),
        (9, points[5]),
        (10, points[6]),
        (11, points[7]),
    ] {
        let mut encoded = [0_u8; POINT_BYTES_V1];
        point
            .write_non_identity_wire_bytes_ref(&mut encoded)
            .expect("canonical q-mask point");
        let offset = (first + local) * POINT_BYTES_V1;
        inventory[offset..offset + POINT_BYTES_V1].copy_from_slice(&encoded);
    }
    let commitments =
        q_mask_linear_commitments_v1(&inventory, 0).expect("first q-mask linear tuple");
    assert_eq!(commitments.digits.as_slice(), &points[..4]);
    assert_eq!(commitments.complement_digits.as_slice(), &points[4..]);
    assert!(q_mask_linear_commitments_v1(&inventory, Q_MASK_BLOCKS_V1 - 1).is_some());
    assert!(q_mask_linear_commitments_v1(&inventory, Q_MASK_BLOCKS_V1).is_none());
    assert!(q_mask_linear_commitments_v1(&inventory[..inventory.len() - 1], 0).is_none());
}

#[test]
fn global_lookup_inverse_accessors_alias_exact_post_z_roles_and_boundaries() {
    let mut inventory = canonical_inventory_v1();
    let points = derive_t256_generators_v1(b"rns-native-global-lookup-inverse-accessors", 25)
        .expect("global lookup inverse accessor points");
    let write_point = |inventory: &mut [u8], ordinal: usize, point: Point| {
        let mut encoded = [0_u8; POINT_BYTES_V1];
        point
            .write_non_identity_wire_bytes_ref(&mut encoded)
            .expect("canonical global lookup inverse point");
        let offset = ordinal * POINT_BYTES_V1;
        inventory[offset..offset + POINT_BYTES_V1].copy_from_slice(&encoded);
    };

    let comparator_first = COMPARATOR_TOP_POINTS_V1;
    let comparator_last =
        COMPARATOR_TOP_POINTS_V1 + (COMPARATOR_GROUPS_V1 - 1) * COMPARATOR_POINTS_PER_GROUP_V1;
    write_point(&mut inventory, comparator_first + 36, points[0]);
    write_point(&mut inventory, comparator_last + 52, points[1]);
    write_point(&mut inventory, comparator_first + 35, points[24]);
    assert_eq!(
        comparator_difference_inverse_v1(&inventory, 0, 0),
        Some(points[0])
    );
    assert_eq!(
        comparator_difference_inverse_v1(
            &inventory,
            COMPARATOR_GROUPS_V1 - 1,
            COMPARATOR_SUBTRACTION_DIGITS_V1 - 1,
        ),
        Some(points[1])
    );
    assert_eq!(
        comparator_difference_inverse_v1(&inventory, COMPARATOR_GROUPS_V1, 0),
        None
    );
    assert_eq!(
        comparator_difference_inverse_v1(&inventory, 0, COMPARATOR_SUBTRACTION_DIGITS_V1),
        None
    );

    let small_first = COMPARATOR_POINTS_V1;
    let small_last =
        COMPARATOR_POINTS_V1 + (SMALL_SOURCE_BLOCKS_V1 - 1) * SMALL_SOURCE_POINTS_PER_BLOCK_V1;
    write_point(&mut inventory, small_first, points[24]);
    write_point(&mut inventory, small_first + 2, points[2]);
    write_point(&mut inventory, small_first + 3, points[3]);
    write_point(&mut inventory, small_last + 2, points[4]);
    write_point(&mut inventory, small_last + 3, points[5]);
    assert_eq!(
        small_source_lookup_inverses_v1(&inventory, 0),
        Some((points[2], points[3]))
    );
    assert_eq!(
        small_source_lookup_inverses_v1(&inventory, SMALL_SOURCE_BLOCKS_V1 - 1),
        Some((points[4], points[5]))
    );
    assert_eq!(
        small_source_lookup_inverses_v1(&inventory, SMALL_SOURCE_BLOCKS_V1),
        None
    );

    let q_mask_first = COMPARATOR_POINTS_V1 + SMALL_SOURCE_POINTS_V1;
    for column in 0..Q_MASK_DIGITS_V1 {
        write_point(&mut inventory, q_mask_first + column, points[24]);
        write_point(
            &mut inventory,
            q_mask_first + 4 + column,
            points[6 + column],
        );
        write_point(
            &mut inventory,
            q_mask_first + 12 + column,
            points[10 + column],
        );
    }
    let q_mask_last = q_mask_first + (Q_MASK_BLOCKS_V1 - 1) * Q_MASK_POINTS_PER_BLOCK_V1;
    for column in 0..Q_MASK_DIGITS_V1 {
        write_point(
            &mut inventory,
            q_mask_last + 4 + column,
            points[14 + column],
        );
        write_point(
            &mut inventory,
            q_mask_last + 12 + column,
            points[18 + column],
        );
    }
    let first_q_mask = q_mask_lookup_inverses_v1(&inventory, 0).expect("first q-mask inverses");
    assert_eq!(first_q_mask.digit_inverses.as_slice(), &points[6..10]);
    assert_eq!(first_q_mask.complement_inverses.as_slice(), &points[10..14]);
    assert!(!first_q_mask.digit_inverses.contains(&points[24]));
    assert!(!first_q_mask.complement_inverses.contains(&points[24]));
    let last_q_mask =
        q_mask_lookup_inverses_v1(&inventory, Q_MASK_BLOCKS_V1 - 1).expect("last q-mask inverses");
    assert_eq!(last_q_mask.digit_inverses.as_slice(), &points[14..18]);
    assert_eq!(last_q_mask.complement_inverses.as_slice(), &points[18..22]);
    assert!(q_mask_lookup_inverses_v1(&inventory, Q_MASK_BLOCKS_V1).is_none());

    let short = &inventory[..inventory.len() - 1];
    assert!(comparator_difference_inverse_v1(short, 0, 0).is_none());
    assert!(small_source_lookup_inverses_v1(short, 0).is_none());
    assert!(q_mask_lookup_inverses_v1(short, 0).is_none());
}

#[test]
fn authenticated_qpcs_grid_is_exact_canonical_and_limb_major() {
    let mut bytes = vec![0_u8; QPCS_EVALUATION_BYTES_V1];
    let relation = (ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 - 1) * REPETITIONS_V1 + (REPETITIONS_V1 - 1);
    let offset = relation * 16;
    let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 - 1];
    bytes[offset..offset + 8].copy_from_slice(&(modulus - 1).to_be_bytes());
    bytes[offset + 8..offset + 16].copy_from_slice(&(modulus - 2).to_be_bytes());
    let grid = CanonicalQpcsEvaluationGridV1::from_authenticated_bytes_v1(&bytes).expect("grid");
    assert_eq!(
        grid.get_v1(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 - 1, REPETITIONS_V1 - 1),
        Some(CanonicalQpcsEvaluationV1 {
            product: modulus - 1,
            opening_quotient: modulus - 2,
        })
    );
    assert_eq!(grid.get_v1(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1, 0), None);
    assert_eq!(grid.get_v1(0, REPETITIONS_V1), None);

    bytes[offset..offset + 8].copy_from_slice(&modulus.to_be_bytes());
    assert_eq!(
        CanonicalQpcsEvaluationGridV1::from_authenticated_bytes_v1(&bytes).map(|_| ()),
        Err(RnsNativeCrossFieldInventoryErrorV1::InvalidQpcsEvaluation)
    );
    assert_eq!(
        CanonicalQpcsEvaluationGridV1::from_authenticated_bytes_v1(&bytes[..bytes.len() - 1])
            .map(|_| ()),
        Err(RnsNativeCrossFieldInventoryErrorV1::InvalidQpcsEvaluation)
    );
}

#[test]
fn proof_body_codec_is_exact_capped_canonical_and_context_bound() {
    let context = [0x42; DIGEST_BYTES_V1];
    let continuation = b"future-streaming-sparse-product-proof";
    let bytes = canonical_wire_v1(context, continuation);
    let view = CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(&bytes, context)
        .expect("canonical proof body");
    assert_eq!(view.prior_context_digest, context);
    assert_eq!(view.inventory.len(), INVENTORY_BYTES_V1);
    assert_eq!(view.continuation, continuation);
    assert_ne!(view.inventory_root, [0; DIGEST_BYTES_V1]);
    assert_ne!(view.continuation_digest, [0; DIGEST_BYTES_V1]);
    assert_ne!(view.codec_digest, [0; DIGEST_BYTES_V1]);

    assert_eq!(
        CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(&bytes, [0x43; 32])
            .map(|_| ()),
        Err(RnsNativeCrossFieldInventoryErrorV1::InvalidHeader)
    );
    assert!(
        CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(
            &bytes[..bytes.len() - 1],
            context
        )
        .is_err()
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(
        CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(&trailing, context).is_err()
    );

    let oversized = vec![0_u8; PROOF_MAX_BYTES_V1 + 1];
    assert_eq!(
        CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(&oversized, context)
            .map(|_| ()),
        Err(RnsNativeCrossFieldInventoryErrorV1::ProofCapExceeded)
    );
}

#[test]
fn provisional_preflight_is_one_pass_and_finalize_reuses_the_exact_allocation() {
    let context = [0x44; DIGEST_BYTES_V1];
    let bytes = canonical_wire_v1(context, b"one-pass-provisional-inventory");
    let before = preflight_audit_counters_v1();
    let lease = RnsNativePreQpcsCrossProofLeaseV1::from_raw_fixture_v1(&bytes);
    let preflight = RnsNativePreQpcsQMaskInventoryPreflightV1::preflight_v1(lease)
        .expect("self-consistent provisional preflight");
    let after_preflight = preflight_audit_counters_v1();
    assert_eq!(after_preflight.header_passes - before.header_passes, 1);
    assert_eq!(
        after_preflight.inventory_root_passes - before.inventory_root_passes,
        1
    );
    assert_eq!(
        after_preflight.point_validation_decodes - before.point_validation_decodes,
        INVENTORY_POINTS_V1
    );
    assert_eq!(
        after_preflight.continuation_hash_passes - before.continuation_hash_passes,
        1
    );
    assert_eq!(
        after_preflight.codec_hash_passes - before.codec_hash_passes,
        1
    );

    let view = preflight
        .into_exact_fixture_proof_view_v1(&bytes)
        .expect("same allocation");
    view.validate_expected_prior_context_v1(context)
        .expect("final context");
    assert_eq!(view.prior_context_digest, context);
    assert_eq!(preflight_audit_counters_v1(), after_preflight);

    let copied_equal = bytes.clone();
    let copied_preflight = RnsNativePreQpcsQMaskInventoryPreflightV1::preflight_v1(
        RnsNativePreQpcsCrossProofLeaseV1::from_raw_fixture_v1(&bytes),
    )
    .expect("second provisional fixture");
    let before_copied_finalize = preflight_audit_counters_v1();
    assert_eq!(
        copied_preflight
            .into_exact_fixture_proof_view_v1(&copied_equal)
            .map(|_| ()),
        Err(RnsNativeCrossFieldInventoryErrorV1::InvalidContext)
    );
    assert_eq!(preflight_audit_counters_v1(), before_copied_finalize);

    let wrong_context_preflight = RnsNativePreQpcsQMaskInventoryPreflightV1::preflight_v1(
        RnsNativePreQpcsCrossProofLeaseV1::from_raw_fixture_v1(&bytes),
    )
    .expect("wrong-context provisional fixture");
    let before_wrong_context = preflight_audit_counters_v1();
    let wrong_context_view = wrong_context_preflight
        .into_exact_fixture_proof_view_v1(&bytes)
        .expect("same allocation for wrong-context check");
    assert_eq!(
        wrong_context_view.validate_expected_prior_context_v1([0x45; DIGEST_BYTES_V1]),
        Err(RnsNativeCrossFieldInventoryErrorV1::InvalidHeader)
    );
    assert_eq!(preflight_audit_counters_v1(), before_wrong_context);
}

#[test]
fn sealed_envelope_preflight_reuses_one_inner_parse_and_exact_proof_offset() {
    let prior_context = [0x47; DIGEST_BYTES_V1];
    let inner = canonical_wire_v1(prior_context, b"sealed-envelope-one-pass");
    let (envelope, seeds) = sealed_envelope_fixture_v1(47, &inner);
    let section = envelope.section(ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup);
    let before = preflight_audit_counters_v1();
    let unbound = preflight_rns_native_cross_field_global_lookup_from_envelope_v1(&envelope)
        .expect("unbound envelope section");
    let (pending, sealed) = unbound.split_pre_qpcs_v1();
    let preflight = sealed
        .preflight_q_mask_inventory_v1()
        .expect("sealed inventory preflight");
    let after_preflight = preflight_audit_counters_v1();
    assert_eq!(after_preflight.header_passes - before.header_passes, 1);
    assert_eq!(
        after_preflight.point_validation_decodes - before.point_validation_decodes,
        INVENTORY_POINTS_V1
    );
    assert_eq!(
        after_preflight.inventory_root_passes - before.inventory_root_passes,
        1
    );
    assert_eq!(
        after_preflight.continuation_hash_passes - before.continuation_hash_passes,
        1
    );
    assert_eq!(
        after_preflight.codec_hash_passes - before.codec_hash_passes,
        1
    );

    let bound = pending
        .bind_final_context_v1(&seeds)
        .expect("final transcript bind");
    let (typed, view) = preflight
        .into_exact_bound_view_v1(bound)
        .expect("same envelope/section/proof identity");
    assert_eq!(view.prior_context_digest, prior_context);
    assert_eq!(typed.proof(), inner.as_slice());
    assert!(core::ptr::eq(
        typed.proof().as_ptr(),
        section[CROSS_LOOKUP_PROOF_OFFSET_V1..].as_ptr()
    ));
    assert_eq!(preflight_audit_counters_v1(), after_preflight);
}

#[test]
fn byte_equal_separately_owned_envelopes_cannot_swap_bound_and_preflight_children() {
    let prior_context = [0x48; DIGEST_BYTES_V1];
    let inner = canonical_wire_v1(prior_context, b"copied-envelope-identity");
    let (envelope_a, seeds_a) = sealed_envelope_fixture_v1(48, &inner);
    let (envelope_b, _seeds_b) = sealed_envelope_fixture_v1(48, &inner);
    let section_a =
        envelope_a.section(ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup);
    let section_b =
        envelope_b.section(ZkAmsMkheRnsNativeProofSectionKindV1::CrossFieldGlobalLookup);
    assert_eq!(section_a, section_b);
    assert!(!core::ptr::eq(section_a.as_ptr(), section_b.as_ptr()));

    let unbound_a = preflight_rns_native_cross_field_global_lookup_from_envelope_v1(&envelope_a)
        .expect("first unbound section");
    let unbound_b = preflight_rns_native_cross_field_global_lookup_from_envelope_v1(&envelope_b)
        .expect("second unbound section");
    let (pending_a, _sealed_a) = unbound_a.split_pre_qpcs_v1();
    let (_pending_b, sealed_b) = unbound_b.split_pre_qpcs_v1();
    let preflight_b = sealed_b
        .preflight_q_mask_inventory_v1()
        .expect("second sealed inventory preflight");
    let bound_a = pending_a
        .bind_final_context_v1(&seeds_a)
        .expect("first final context bind");
    let before_rejection = preflight_audit_counters_v1();
    assert_eq!(
        preflight_b.into_exact_bound_view_v1(bound_a).map(|_| ()),
        Err(RnsNativeCrossFieldInventoryErrorV1::InvalidContext)
    );
    assert_eq!(preflight_audit_counters_v1(), before_rejection);
}

#[test]
fn provisional_q_mask_projection_has_exact_first_last_coordinates_and_6400_calls() {
    let context = [0x46; DIGEST_BYTES_V1];
    let mut inventory = canonical_inventory_v1();
    let points = derive_t256_generators_v1(b"rns-native-pre-qpcs-q-mask-projection", 3)
        .expect("projection points");
    let write_point = |inventory: &mut [u8], ordinal: usize, point: Point| {
        let mut encoded = [0_u8; POINT_BYTES_V1];
        point
            .write_non_identity_wire_bytes_ref(&mut encoded)
            .expect("canonical projection point");
        let offset = ordinal * POINT_BYTES_V1;
        inventory[offset..offset + POINT_BYTES_V1].copy_from_slice(&encoded);
    };
    write_point(&mut inventory, Q_MASK_INVENTORY_FIRST_ORDINAL_V1, points[0]);
    write_point(
        &mut inventory,
        PRE_QPCS_Q_MASK_LAST_INVENTORY_ORDINAL_V1,
        points[1],
    );
    write_point(
        &mut inventory,
        Q_MASK_INVENTORY_FIRST_ORDINAL_V1 + Q_MASK_DIGITS_V1,
        points[2],
    );
    let bytes = canonical_wire_with_inventory_v1(context, b"q-mask-coordinate-map", &inventory);
    let mut first_encoded = [0_u8; POINT_BYTES_V1];
    points[0]
        .write_non_identity_wire_bytes_ref(&mut first_encoded)
        .expect("first point encoding");
    let mut last_encoded = [0_u8; POINT_BYTES_V1];
    points[1]
        .write_non_identity_wire_bytes_ref(&mut last_encoded)
        .expect("last point encoding");
    assert_eq!(
        &bytes[PRE_QPCS_Q_MASK_FIRST_PROOF_OFFSET_V1
            ..PRE_QPCS_Q_MASK_FIRST_PROOF_OFFSET_V1 + POINT_BYTES_V1],
        first_encoded.as_slice()
    );
    assert_eq!(
        &bytes[PRE_QPCS_Q_MASK_LAST_PROOF_OFFSET_V1
            ..PRE_QPCS_Q_MASK_LAST_PROOF_OFFSET_V1 + POINT_BYTES_V1],
        last_encoded.as_slice()
    );
    let preflight = RnsNativePreQpcsQMaskInventoryPreflightV1::preflight_v1(
        RnsNativePreQpcsCrossProofLeaseV1::from_raw_fixture_v1(&bytes),
    )
    .expect("q-mask provisional preflight");
    assert!(
        preflight
            .q_mask_s_digit_commitment_v1(0, 0, 0, 0)
            .expect("first q-mask digit")
            == points[0]
    );
    assert!(
        preflight
            .q_mask_s_digit_commitment_v1(
                ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 - 1,
                REPETITIONS_V1 - 1,
                BLOCKS_PER_RECORD_V1 - 1,
                Q_MASK_DIGITS_V1 - 1,
            )
            .expect("last q-mask digit")
            == points[1]
    );
    for invalid in [
        preflight.q_mask_s_digit_commitment_v1(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1, 0, 0, 0),
        preflight.q_mask_s_digit_commitment_v1(0, REPETITIONS_V1, 0, 0),
        preflight.q_mask_s_digit_commitment_v1(0, 0, BLOCKS_PER_RECORD_V1, 0),
        preflight.q_mask_s_digit_commitment_v1(0, 0, 0, Q_MASK_DIGITS_V1),
    ] {
        assert_eq!(
            invalid.map(|_| ()),
            Err(RnsNativeCrossFieldRlweDirectErrorV1::InvalidGeometry)
        );
    }

    let before_root = preflight_audit_counters_v1();
    let axes = RnsNativeCrossFieldPreQpcsSafeAxesV1 {
        profile_manifest_digest: [1; DIGEST_BYTES_V1],
        source_binding_digest: [2; DIGEST_BYTES_V1],
        source_formula_digest: [3; DIGEST_BYTES_V1],
        source_mapping_digest: [4; DIGEST_BYTES_V1],
        rns_aggregation_challenge_seed: [5; DIGEST_BYTES_V1],
        qpcs_parameter_digest: [6; DIGEST_BYTES_V1],
        qpcs_pre_relation_transcript_digest: [7; DIGEST_BYTES_V1],
    };
    assert_ne!(
        q_mask_s_root_v1(axes, &preflight).expect("exact provisional q-mask root"),
        [0; DIGEST_BYTES_V1]
    );
    let after_root = preflight_audit_counters_v1();
    assert_eq!(
        after_root.q_mask_digit_projections - before_root.q_mask_digit_projections,
        PRE_QPCS_Q_MASK_S_POINTS_V1
    );
    assert_eq!(after_root.header_passes, before_root.header_passes);
    assert_eq!(
        after_root.point_validation_decodes,
        before_root.point_validation_decodes
    );
    assert_eq!(
        after_root.inventory_root_passes,
        before_root.inventory_root_passes
    );
    assert_eq!(
        after_root.continuation_hash_passes,
        before_root.continuation_hash_passes
    );
    assert_eq!(after_root.codec_hash_passes, before_root.codec_hash_passes);
}

#[test]
fn proof_body_rejects_geometry_point_and_continuation_substitution() {
    let context = [0x52; DIGEST_BYTES_V1];
    let bytes = canonical_wire_v1(context, b"continuation");

    let mut geometry = bytes.clone();
    geometry[12] = 39;
    assert_eq!(
        CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(&geometry, context)
            .map(|_| ()),
        Err(RnsNativeCrossFieldInventoryErrorV1::InvalidGeometry)
    );

    let mut point = bytes.clone();
    point[HEADER_BYTES_V1..HEADER_BYTES_V1 + POINT_BYTES_V1].fill(0);
    assert_eq!(
        CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(&point, context).map(|_| ()),
        Err(RnsNativeCrossFieldInventoryErrorV1::InvalidPoint)
    );

    let mut continuation = bytes.clone();
    let continuation_offset = HEADER_BYTES_V1 + INVENTORY_BYTES_V1;
    continuation[continuation_offset] ^= 1;
    assert_eq!(
        CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(&continuation, context)
            .map(|_| ()),
        Err(RnsNativeCrossFieldInventoryErrorV1::InvalidIntegrity)
    );
}

#[test]
fn pre_direct_candidate_projection_surface_is_sealed_one_pass_and_fail_closed() {
    let source = include_str!("rns_native_cross_field_inventory.rs");
    for domain in [
        "iroha.zk-ams.v1.mkhe.rns-native-cross-field-inventory.pre-direct-candidate.manifest",
        "iroha.zk-ams.v1.mkhe.rns-native-cross-field-inventory.pre-direct-candidate.context",
        "iroha.zk-ams.v1.mkhe.rns-native-cross-field-inventory.pre-direct-candidate.point",
        "iroha.zk-ams.v1.mkhe.rns-native-cross-field-inventory.pre-direct-candidate.final",
    ] {
        assert!(
            source.contains(domain),
            "missing candidate domain: {domain}"
        );
    }
    for exact in [
        "PRE_DIRECT_CANDIDATE_POINTS_V1 == 27_936",
        "PRE_DIRECT_CANDIDATE_EXCLUDED_INVERSE_POINTS_V1 == 20_712",
        "PRE_DIRECT_CANDIDATE_POINT_BYTES_V1 == 921_888",
        "PRE_DIRECT_CANDIDATE_FRAME_BYTES_V1 == 44",
        "PRE_DIRECT_CANDIDATE_FRAMED_BYTES_V1 == 1_229_184",
    ] {
        assert!(
            source.contains(exact),
            "missing exact resource pin: {exact}"
        );
    }

    let canonical_loop = source
        .split_once("fn canonical_inventory_root_v1(")
        .expect("canonical inventory loop")
        .1
        .split_once("fn canonical_continuation_digest_v1(")
        .expect("canonical inventory loop boundary")
        .0;
    assert_eq!(
        canonical_loop
            .matches("chunks_exact(POINT_BYTES_V1)")
            .count(),
        1
    );
    assert_eq!(
        canonical_loop
            .matches("Point::from_non_identity_wire_bytes_exact(encoded)")
            .count(),
        1
    );
    assert!(canonical_loop.contains("coordinate.role.is_pre_direct_candidate_v1()"));
    for framed_field in [
        "(ordinal as u32).to_be_bytes()",
        "[coordinate.role as u8]",
        "(coordinate.owner as u32).to_be_bytes()",
        "(coordinate.column as u16).to_be_bytes()",
        "candidate_hash.update(encoded)",
    ] {
        assert!(
            canonical_loop.contains(framed_field),
            "candidate frame omits {framed_field}"
        );
    }
    for allocation in ["Vec<", "Vec::", ".collect(", ".to_vec(", "Box<"] {
        assert!(
            !canonical_loop.contains(allocation),
            "candidate hash allocates through {allocation}"
        );
    }
    assert!(source.contains(
        "pre_direct_candidate_point_root: canonical_roots.pre_direct_candidate_point_root"
    ));

    let projection_declaration = source
        .split_once("pub(super) struct RnsNativePreDirectInventoryCandidateProjectionV1")
        .expect("opaque candidate projection")
        .1
        .split_once("impl RnsNativePreDirectInventoryCandidateProjectionV1")
        .expect("opaque candidate projection boundary")
        .0;
    assert!(!projection_declaration.contains("derive(Clone"));
    assert!(!projection_declaration.contains("derive(Copy"));
    let projection_impl = source
        .split_once("impl RnsNativePreDirectInventoryCandidateProjectionV1")
        .expect("candidate projection implementation")
        .1
        .split_once("fn mint_pre_direct_inventory_candidate_projection_v1")
        .expect("candidate projection implementation boundary")
        .0;
    assert!(projection_impl.contains("is_valid_for_direct_fixed_axes_v1"));
    assert!(projection_impl.contains("absorb_direct_fixed_axes_v1"));
    for raw_surface in [
        "fn new",
        "fn from_",
        "fn into_",
        "fn parts",
        "fn context_digest",
        "fn inventory_root",
        "fn test_fixture",
    ] {
        assert!(
            !projection_impl.contains(raw_surface),
            "raw projection surface present: {raw_surface}"
        );
    }

    let mint = source
        .split_once("fn mint_pre_direct_inventory_candidate_projection_v1")
        .expect("candidate mint")
        .1
        .split_once("fn prior_context_digest_v1")
        .expect("candidate mint boundary")
        .0;
    let safe_context = mint
        .split_once("let safe_context = [")
        .expect("candidate safe context")
        .1
        .split_once("];\n")
        .expect("candidate safe context boundary")
        .0;
    for required in [
        "transcript.profile_manifest_digest()",
        "source.snapshot().layout().source_binding_digest()",
        "source.formula_digest()",
        "source.mapping_digest()",
    ] {
        assert!(
            safe_context.contains(required),
            "missing safe axis: {required}"
        );
    }
    for excluded in [
        "terminal",
        "qpcs",
        "prior_context",
        "inventory_root",
        "continuation",
        "codec",
        "transcript_digest",
        "challenge",
        "cross",
        "global",
        "zero",
        "direct",
        "successor",
    ] {
        assert!(
            !safe_context.contains(excluded),
            "forbidden candidate context axis present: {excluded}"
        );
    }

    let ordinary = source
        .split_once("pub(super) fn authenticate_rns_native_cross_field_inventory_v1")
        .expect("ordinary typed authentication")
        .1
        .split_once("/// Consume the exact bound envelope section")
        .expect("ordinary typed authentication boundary")
        .0;
    assert!(ordinary.contains("view,\n        None,"));
    assert!(!ordinary.contains("mint_pre_direct_inventory_candidate_projection_v1"));
    assert!(
        !source
            .contains("authenticate_rns_native_cross_field_inventory_from_pre_qpcs_preflight_v1")
    );
    let sealed = source
        .split_once(
            "pub(super) fn authenticate_rns_native_cross_field_inventory_from_sealed_pre_qpcs_preflight_v1",
        )
        .expect("sealed candidate authentication")
        .1
        .split_once("#[cfg(test)]\n#[path")
        .expect("sealed candidate authentication boundary")
        .0;
    let exact_allocation = sealed
        .find("preflight.into_exact_bound_view_v1(bound)")
        .expect("exact allocation authentication");
    let linked = sealed
        .find("link_rns_native_source_terminal_cross_field_v1")
        .expect("linked source authentication");
    let mint = sealed
        .find("mint_pre_direct_inventory_candidate_projection_v1")
        .expect("candidate mint after authentication");
    let install = sealed
        .find("Some(pre_direct_candidate_projection)")
        .expect("candidate installation");
    assert!(exact_allocation < linked && linked < mint && mint < install);

    let prerequisite = source
        .split_once("pub(super) struct RnsNativeCrossFieldInventoryPrerequisiteV1")
        .expect("inventory prerequisite")
        .1
        .split_once("impl<'source, 'proof")
        .expect("inventory prerequisite boundary")
        .0;
    assert_eq!(
        prerequisite
            .matches("pre_direct_candidate_projection: Option<")
            .count(),
        1
    );
    let take_once = source
        .split_once("fn take_pre_direct_candidate_projection_v1")
        .expect("one-shot candidate take")
        .1
        .split_once("/// Narrow post-equation alias")
        .expect("one-shot candidate take boundary")
        .0;
    assert_eq!(take_once.matches(".take()").count(), 1);

    for false_gate in [
        "PRE_DIRECT_CANDIDATE_PROJECTION_LIVE_V1: bool = false",
        "PRE_DIRECT_CANDIDATE_PROJECTION_SOURCE_INTEGRATED_V1: bool = false",
        "PRE_DIRECT_CANDIDATE_PROJECTION_DIRECT_INTEGRATED_V1: bool = false",
        "PRE_DIRECT_CANDIDATE_PROJECTION_RESOURCE_EVIDENCE_QUALIFIED_V1: bool = false",
        "PRE_DIRECT_CANDIDATE_PROJECTION_READINESS_V1: bool = false",
        "PRE_DIRECT_CANDIDATE_PROJECTION_RELEASE_READY_V1: bool = false",
    ] {
        assert!(
            source.contains(false_gate),
            "live gate changed: {false_gate}"
        );
    }
    assert!(
        source.contains("PRE_DIRECT_CANDIDATE_PROJECTION_CONTRACT_IMPLEMENTED_V1: bool = true")
    );

    let direct = include_str!("rns_native_cross_field_rlwe_direct.rs");
    let constructor = direct
        .split_once("fn from_projection_v1(")
        .expect("direct candidate constructor")
        .1
        .split_once(") -> Result<Self")
        .expect("direct candidate constructor signature")
        .0;
    assert!(constructor.contains("RnsNativePreDirectInventoryCandidateProjectionV1"));
    assert!(!constructor.contains("[u8;"));
    assert!(!direct.contains("pub(super) fn from_projection_v1("));
    let full_bind = direct
        .split_once("fn bind_direct_q_mask_schedule_v1(")
        .expect("full direct bind")
        .1
        .split_once("fn derive_relation_schedule_v1(")
        .expect("full direct bind boundary")
        .0;
    assert!(full_bind.contains("permits_full_direct_bind_v1()"));
    let axis_impl = direct
        .split_once("impl RnsNativePreDirectInventoryCandidateAxesV1")
        .expect("candidate axis implementation")
        .1
        .split_once("impl RnsNativeCrossFieldRlweFixedAxesV1")
        .expect("candidate axis implementation boundary")
        .0;
    assert!(
        axis_impl
            .contains("RnsNativePreDirectInventoryCandidateAxesOriginV1::Projection(_) => true")
    );
    let qpcs = include_str!("rns_native_qpcs_fri_complete.rs");
    let chronology = qpcs
        .split_once("pub(super) struct RnsNativeQpcsClaimedInventoryChronologyV2")
        .expect("claimed inventory chronology")
        .1
        .split_once("impl<S: ZkAmsMkheRnsNativeSourceSnapshotV1>")
        .expect("claimed inventory chronology boundary")
        .0;
    assert_eq!(chronology.matches("bound_pre_direct_inventory:").count(), 1);
    assert!(!chronology.contains("candidate_inventory_axes:"));
    assert!(!chronology.contains("pre_direct_nested:"));
    let join = qpcs
        .split_once("pub(super) fn authenticate_claimed_inventory_v2")
        .expect("claimed inventory join")
        .1
        .split_once("impl<'qpcs, 'cross, S: ZkAmsMkheRnsNativeSourceSnapshotV1>")
        .expect("claimed inventory join boundary")
        .0;
    assert!(join.contains("bind_rns_native_cross_field_rlwe_pre_direct_inventory_v1(inventory)"));
    assert!(!join.contains("take_pre_direct_candidate_projection_v1()"));
    assert!(!join.contains("RnsNativePreDirectInventoryCandidateAxesV1::from_projection_v1"));
    let atomic_owner = direct
        .split_once("pub(super) fn bind_rns_native_cross_field_rlwe_pre_direct_inventory_v1")
        .expect("atomic pre-direct inventory constructor")
        .1
        .split_once("fn validate_claimed_inventory_transcript_v1")
        .expect("atomic pre-direct inventory constructor boundary")
        .0;
    let take = atomic_owner
        .find("take_pre_direct_candidate_projection_v1()")
        .expect("take candidate projection");
    let convert = atomic_owner
        .find("RnsNativePreDirectInventoryCandidateAxesV1::from_projection_v1")
        .expect("convert candidate projection");
    let nested = atomic_owner
        .find("preflight_rns_native_cross_field_rlwe_nested_owner_v1(&inventory)")
        .expect("preflight same inventory");
    assert!(take < convert && convert < nested);
    let direct_bind = direct
        .find("pub(super) fn bind_authenticated_claimed_qpcs_inventory_direct_v2")
        .expect("production direct bind");
    assert!(!direct[direct_bind.saturating_sub(64)..direct_bind].contains("#[cfg(test)]"));
    let direct_bind_surface = &direct[direct_bind..];
    let direct_bind_signature = direct_bind_surface
        .split_once(") -> Result<")
        .expect("direct bind signature")
        .0;
    assert!(direct_bind_signature.contains("bound_pre_direct_inventory:"));
    assert!(direct_bind_signature.contains("claimed_qpcs_input:"));
    for detached_input in [
        "inventory: RnsNativeCrossFieldInventoryPrerequisiteV1",
        "candidate_inventory_axes:",
        "pre_direct_nested:",
        "completed_qpcs:",
        "terminal_chronology:",
        "numeric_tails:",
        "source_binding_digest:",
    ] {
        assert!(
            !direct_bind_signature.contains(detached_input),
            "detached direct input: {detached_input}"
        );
    }
    assert!(!direct_bind_signature.contains("RnsNativeCrossFieldRlweFixedAxesV1"));
    assert!(!direct.contains("pub(super) fn prepare_direct_relation_schedule_after_qpcs_v1"));
    let qpcs_bind = qpcs
        .find("pub(super) fn bind_direct_claimed_relation_v2")
        .expect("production qPCS bind");
    assert!(!qpcs[qpcs_bind.saturating_sub(64)..qpcs_bind].contains("#[cfg(test)]"));
    let carrier = include_str!(
        "collective/incremental_source_rns_native_tail_publication_v2/pretranscript_public_statement_v2/claimed_qpcs_source_carrier_v2.rs"
    );
    let carrier_bind = carrier
        .find("pub(super) fn bind_direct_claimed_relation_v2")
        .expect("production carrier bind");
    assert!(!carrier[carrier_bind.saturating_sub(64)..carrier_bind].contains("#[cfg(test)]"));
    let carrier_bind_signature = carrier[carrier_bind..]
        .split_once(") -> Result<")
        .expect("carrier bind signature")
        .0;
    assert!(carrier_bind_signature.contains("self,"));
    assert!(!carrier_bind_signature.contains("RnsNativeCrossFieldRlweFixedAxesV1"));
}

#[test]
fn production_boundary_is_private_move_only_non_authorizing_and_fail_closed() {
    let source = include_str!("rns_native_cross_field_inventory.rs");
    let declaration = "pub(super) struct RnsNativeCrossFieldInventoryPrerequisiteV1";
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
    assert!(!stage.contains("Verified"));
    assert!(!stage.contains("Release"));
    assert!(source.contains("Point::from_non_identity_wire_bytes_exact"));
    assert!(
        source.contains("COMPARATOR_BOOLEAN_DISJOINT_PRODUCT_ARGUMENT_AVAILABLE_V1: bool = true")
    );
    assert!(source.contains("RANGE_AND_CARRY_RELATIONS_VERIFIED_V1: bool = false"));
    assert!(source.contains("CANONICAL_Q_MASK_RELATIONS_VERIFIED_V1: bool = false"));
    assert!(source.contains("GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1: bool = false"));
    assert!(stage.contains("terminal_transcript_digest: [u8; DIGEST_BYTES_V1]"));
    assert!(source.contains("terminal_transcript_digest: transcript.transcript_digest()"));
    assert!(source.contains("pub(super) const fn terminal_transcript_digest_v1(&self)"));

    let lease_declaration = source
        .split_once("pub(super) struct RnsNativePreQpcsCrossProofLeaseV1")
        .expect("pre-qPCS exact proof lease")
        .1
        .split_once("impl<'proof> RnsNativePreQpcsCrossProofLeaseV1")
        .expect("pre-qPCS exact proof lease boundary")
        .0;
    assert!(!lease_declaration.contains("derive(Clone"));
    assert!(!lease_declaration.contains("derive(Copy"));
    assert!(lease_declaration.contains("proof: &'proof [u8]"));
    assert!(source.contains(
        "#[must_use = \"the exact proof-slice lease must be consumed by its provisional preflight\"]"
    ));
    let lease_impl = source
        .split_once("impl<'proof> RnsNativePreQpcsCrossProofLeaseV1")
        .expect("pre-qPCS proof lease implementation")
        .1
        .split_once("pub(super) struct RnsNativePreQpcsQMaskInventoryPreflightV1")
        .expect("pre-qPCS proof lease implementation boundary")
        .0;
    let raw_fixture = lease_impl
        .find("pub(super) const fn from_raw_fixture_v1")
        .expect("test-only raw lease fixture");
    assert!(lease_impl[raw_fixture.saturating_sub(32)..raw_fixture].contains("#[cfg(test)]"));
    assert!(source.contains("SealedEnvelope(RnsNativeSealedCrossProofInventoryPermitV1<'proof>)"));
    assert!(source.contains("from_sealed_envelope_v1("));
    assert!(!source.contains("Infallible"));

    let preflight_declaration = source
        .split_once("pub(super) struct RnsNativePreQpcsQMaskInventoryPreflightV1")
        .expect("move-only provisional preflight")
        .1
        .split_once("impl<'proof> RnsNativePreQpcsQMaskInventoryPreflightV1")
        .expect("move-only provisional preflight boundary")
        .0;
    assert!(!preflight_declaration.contains("derive(Clone"));
    assert!(!preflight_declaration.contains("derive(Copy"));
    assert!(source.contains(
        "#[must_use = \"the provisional q-mask owner must be consumed by final inventory authentication\"]"
    ));
    let preflight_impl = source
        .split_once("impl<'proof> RnsNativePreQpcsQMaskInventoryPreflightV1")
        .expect("provisional preflight implementation")
        .1
        .split_once("fn canonical_inventory_root_v1")
        .expect("provisional preflight implementation boundary")
        .0;
    assert_eq!(preflight_impl.matches("pub(super) fn ").count(), 2);
    assert!(preflight_impl.contains("pub(super) fn preflight_v1("));
    assert!(preflight_impl.contains("pub(super) fn project_q_mask_s_digit_v1("));
    for forbidden_escape in [
        "pub(super) fn proof",
        "pub(super) fn bytes",
        "pub(super) fn digest",
        "pub(super) fn root",
        "pub(super) fn continuation",
        "pub(super) fn points",
        "AsRef",
        "Deref",
    ] {
        assert!(
            !preflight_impl.contains(forbidden_escape),
            "provisional preflight escape present: {forbidden_escape}"
        );
    }
    for exact_identity_check in [
        "core::ptr::eq(lease.proof.as_ptr(), exact_proof.as_ptr())",
        "lease.proof.len() == exact_proof.len()",
        "lease.proof == exact_proof",
    ] {
        assert!(
            preflight_impl.contains(exact_identity_check),
            "missing exact proof identity check: {exact_identity_check}"
        );
    }

    let shared_typed_parser = source
        .split_once("fn from_canonical_bytes_exact_v1(")
        .expect("typed compatibility parser")
        .1
        .split_once("fn canonical_inventory_root_v1")
        .expect("typed compatibility parser boundary")
        .0;
    assert!(shared_typed_parser.contains("from_self_consistent_canonical_bytes_exact_v1(bytes)"));
    assert!(shared_typed_parser.contains("validate_expected_prior_context_v1"));
    let consuming_finalize = source
        .split_once(
            "pub(super) fn authenticate_rns_native_cross_field_inventory_from_sealed_pre_qpcs_preflight_v1",
        )
        .expect("consuming sealed preflight finalizer")
        .1
        .split_once("#[cfg(test)]")
        .expect("consuming sealed preflight finalizer boundary")
        .0;
    assert!(consuming_finalize.contains("preflight.into_exact_bound_view_v1(bound)"));
    assert!(consuming_finalize.contains("view.validate_expected_prior_context_v1("));
    let exact_identity = consuming_finalize
        .find("preflight.into_exact_bound_view_v1(bound)")
        .expect("exact sealed identity first");
    let linked_source = consuming_finalize
        .find("linked.source().qpcs().evaluations()")
        .expect("linked source authentication");
    assert!(exact_identity < linked_source);
    for forbidden_second_pass in [
        "from_self_consistent_canonical_bytes_exact_v1",
        "from_canonical_bytes_exact_v1",
        "canonical_inventory_root_v1",
        "canonical_continuation_digest_v1",
        "codec_digest_v1",
        "DecoderV1",
    ] {
        assert!(
            !consuming_finalize.contains(forbidden_second_pass),
            "sealed finalizer repeats provisional work: {forbidden_second_pass}"
        );
    }
    assert!(source.contains("PRE_QPCS_Q_MASK_SEALED_ENVELOPE_LEASE_IMPLEMENTED_V1: bool = true"));
    for false_gate in [
        "PRE_QPCS_Q_MASK_PREFLIGHT_LIVE_V1: bool = false",
        "PRE_QPCS_Q_MASK_SOURCE_INTEGRATED_V1: bool = false",
        "PRE_QPCS_Q_MASK_DIRECT_INTEGRATED_V1: bool = false",
        "PRE_QPCS_Q_MASK_COMPOSITE_INTEGRATED_V1: bool = false",
        "PRE_QPCS_Q_MASK_RESOURCE_EVIDENCE_QUALIFIED_V1: bool = false",
        "PRE_QPCS_Q_MASK_READINESS_V1: bool = false",
        "PRE_QPCS_Q_MASK_RELEASE_READY_V1: bool = false",
    ] {
        assert!(
            source.contains(false_gate),
            "live gate changed: {false_gate}"
        );
    }
    assert!(source.contains("PROOF_MAX_BYTES_V1 == 8_385_797"));
    assert!(
        source.contains("RNS_NATIVE_CROSS_FIELD_INVENTORY_CONTINUATION_MAX_BYTES_V1 == 6_780_245")
    );

    let parent = include_str!("../mkhe.rs");
    assert_eq!(
        parent
            .matches("mod rns_native_cross_field_inventory;")
            .count(),
        1
    );
    assert!(!parent.contains("pub use rns_native_cross_field_inventory"));
    assert!(!parent.contains("phase23_rns_link"));

    let composite = include_str!("rns_native_composite_verifier.rs");
    assert!(composite.contains("StageUnavailable"));
    assert!(composite.contains("CrossFieldGlobalLookup"));
    let qpcs = include_str!("rns_native_qpcs_fri_complete.rs");
    assert!(qpcs.contains("pub(super) fn authenticate_rns_native_qpcs_fri_complete_v1"));
    assert!(qpcs.contains("Successful verification remains non-authorizing"));
}
