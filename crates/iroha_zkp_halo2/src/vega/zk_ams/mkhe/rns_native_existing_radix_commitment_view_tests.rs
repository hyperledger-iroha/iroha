use super::*;
use crate::vega::derive_t256_generators_v1;

fn upstream_v1() -> UpstreamBindingV1 {
    UpstreamBindingV1 {
        prior_context_digest: [0x11; DIGEST_BYTES_V1],
        added_inventory_root: [0x22; DIGEST_BYTES_V1],
        statement3_proof_set_root: [0x33; DIGEST_BYTES_V1],
        statement3_verified_transcript_root: [0x44; DIGEST_BYTES_V1],
        statement5_proof_set_root: [0x55; DIGEST_BYTES_V1],
        statement5_verified_transcript_root: [0x66; DIGEST_BYTES_V1],
        statement8_proof_set_root: [0x77; DIGEST_BYTES_V1],
        statement8_verified_transcript_root: [0x88; DIGEST_BYTES_V1],
        q_mask_proof_set_root: [0x99; DIGEST_BYTES_V1],
        q_mask_verified_transcript_root: [0xaa; DIGEST_BYTES_V1],
    }
}

fn test_points_v1() -> [Point; 4] {
    let points = derive_t256_generators_v1(b"rns-native-existing-radix-view-tests", 4)
        .expect("existing-radix test points");
    [points[0], points[1], points[2], points[3]]
}

fn encoded_point_v1(point: Point) -> [u8; POINT_BYTES_V1] {
    let mut encoded = [0_u8; POINT_BYTES_V1];
    point
        .write_non_identity_wire_bytes_ref(&mut encoded)
        .expect("canonical existing-radix point");
    encoded
}

fn canonical_inventory_v1() -> Vec<u8> {
    let points = test_points_v1();
    let encoded = points.map(encoded_point_v1);
    let mut inventory = Vec::with_capacity(INVENTORY_BYTES_V1);
    for ordinal in 0..INVENTORY_POINTS_V1 {
        let coordinate = coordinate_v1(ordinal).expect("canonical coordinate");
        let selector = (coordinate.group + usize::from(coordinate.role) + coordinate.column) % 4;
        inventory.extend_from_slice(&encoded[selector]);
    }
    assert_eq!(inventory.len(), INVENTORY_BYTES_V1);
    inventory
}

fn canonical_wire_v1(upstream: UpstreamBindingV1, residual: &[u8]) -> Vec<u8> {
    let inventory = canonical_inventory_v1();
    let pre_z_candidate_root =
        canonical_pre_z_candidate_root_v1(&inventory).expect("pre-z candidate root");
    let residual_digest = canonical_residual_digest_v1(upstream, pre_z_candidate_root, residual)
        .expect("existing-radix residual digest");
    let total = HEADER_BYTES_V1 + inventory.len() + residual.len() + CODEC_DIGEST_BYTES_V1;
    let mut wire = Vec::with_capacity(total);
    wire.extend_from_slice(&MAGIC_V1);
    wire.push(VERSION_V1);
    wire.push(FLAGS_V1);
    wire.extend_from_slice(&(HEADER_BYTES_V1 as u16).to_be_bytes());
    wire.extend_from_slice(&(total as u32).to_be_bytes());
    wire.extend_from_slice(&(GROUPS_V1 as u16).to_be_bytes());
    wire.extend_from_slice(&[
        ROLES_V1 as u8,
        LOW_DIGITS_V1 as u8,
        POINTS_PER_GROUP_V1 as u8,
        POINT_BYTES_V1 as u8,
    ]);
    wire.extend_from_slice(&(INVENTORY_POINTS_V1 as u32).to_be_bytes());
    wire.extend_from_slice(&(INVENTORY_BYTES_V1 as u32).to_be_bytes());
    for digest in upstream.digests_v1() {
        wire.extend_from_slice(&digest);
    }
    wire.extend_from_slice(&pre_z_candidate_root);
    wire.extend_from_slice(&residual_digest);
    wire.extend_from_slice(&(residual.len() as u32).to_be_bytes());
    assert_eq!(wire.len(), HEADER_BYTES_V1);
    wire.extend_from_slice(&inventory);
    wire.extend_from_slice(residual);
    let codec_digest = codec_digest_v1(&wire);
    wire.extend_from_slice(&codec_digest);
    assert_eq!(wire.len(), total);
    wire
}

#[test]
fn existing_radix_codec_is_exact_canonical_capped_and_upstream_bound() {
    let upstream = upstream_v1();
    let residual = b"statement-2-complement-and-statement-4-subtraction-follow";
    let wire = canonical_wire_v1(upstream, residual);
    let view = ExistingRadixProofViewV1::from_components_v1(&wire, upstream)
        .expect("canonical existing-radix view");
    assert_eq!(view.inventory.len(), INVENTORY_BYTES_V1);
    assert_eq!(view.residual, residual);
    assert_ne!(view.pre_z_candidate_root, [0; DIGEST_BYTES_V1]);
    assert_ne!(view.residual_digest, [0; DIGEST_BYTES_V1]);
    assert_ne!(view.codec_digest, [0; DIGEST_BYTES_V1]);
    assert_eq!(MIN_WIRE_BYTES_V1, 386_415);
    const {
        assert!(MIN_WIRE_BYTES_V1 <= 386_513);
    }

    let mut changed = upstream;
    changed.q_mask_verified_transcript_root[0] ^= 1;
    assert_eq!(
        ExistingRadixProofViewV1::from_components_v1(&wire, changed).map(|_| ()),
        Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidHeader)
    );
    assert!(
        ExistingRadixProofViewV1::from_components_v1(&wire[..wire.len() - 1], upstream).is_err()
    );
    let mut trailing = wire.clone();
    trailing.push(0);
    assert!(ExistingRadixProofViewV1::from_components_v1(&trailing, upstream).is_err());
    let cap_plus_one = vec![0_u8; RNS_NATIVE_Q_MASK_LINEAR_RESIDUAL_MAX_BYTES_V1 + 1];
    assert_eq!(
        ExistingRadixProofViewV1::from_components_v1(&cap_plus_one, upstream).map(|_| ()),
        Err(RnsNativeExistingRadixCommitmentViewErrorV1::ProofCapExceeded)
    );
}

#[test]
fn codec_rejects_geometry_point_inventory_residual_and_codec_mutations() {
    let upstream = upstream_v1();
    let wire = canonical_wire_v1(upstream, b"nonempty-downstream");
    let parse =
        |bytes: &[u8]| ExistingRadixProofViewV1::from_components_v1(bytes, upstream).map(|_| ());

    let mut geometry = wire.clone();
    geometry[13] ^= 1;
    assert_eq!(
        parse(&geometry),
        Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidGeometry)
    );
    let mut invalid_point = wire.clone();
    invalid_point[HEADER_BYTES_V1..HEADER_BYTES_V1 + POINT_BYTES_V1].fill(0);
    assert_eq!(
        parse(&invalid_point),
        Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidPoint)
    );
    let mut changed_point = wire.clone();
    let replacement = encoded_point_v1(test_points_v1()[3]);
    changed_point[HEADER_BYTES_V1..HEADER_BYTES_V1 + POINT_BYTES_V1].copy_from_slice(&replacement);
    assert_eq!(
        parse(&changed_point),
        Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidIntegrity)
    );
    let mut changed_residual = wire.clone();
    changed_residual[HEADER_BYTES_V1 + INVENTORY_BYTES_V1] ^= 1;
    assert_eq!(
        parse(&changed_residual),
        Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidIntegrity)
    );
    let mut changed_codec = wire;
    let last = changed_codec.len() - 1;
    changed_codec[last] ^= 1;
    assert_eq!(
        parse(&changed_codec),
        Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidIntegrity)
    );
}

#[test]
fn role_group_column_schedule_is_total_exact_and_non_aliasing() {
    assert_eq!(
        coordinate_v1(0),
        Ok(ExistingRadixCoordinateV1 {
            ordinal: 0,
            group: 0,
            role: ROLE_DIFFERENCE_LOW_V1,
            column: 0,
        })
    );
    assert_eq!(
        coordinate_v1(16),
        Ok(ExistingRadixCoordinateV1 {
            ordinal: 16,
            group: 0,
            role: ROLE_DIFFERENCE_LOW_V1,
            column: 16,
        })
    );
    assert_eq!(
        coordinate_v1(17),
        Ok(ExistingRadixCoordinateV1 {
            ordinal: 17,
            group: 0,
            role: ROLE_SLACK_LOW_V1,
            column: 0,
        })
    );
    assert_eq!(
        coordinate_v1(INVENTORY_POINTS_V1 - 1),
        Ok(ExistingRadixCoordinateV1 {
            ordinal: INVENTORY_POINTS_V1 - 1,
            group: GROUPS_V1 - 1,
            role: ROLE_SLACK_LOW_V1,
            column: LOW_DIGITS_V1 - 1,
        })
    );
    assert_eq!(
        coordinate_v1(INVENTORY_POINTS_V1),
        Err(RnsNativeExistingRadixCommitmentViewErrorV1::InvalidGeometry)
    );
}

#[test]
fn low_commitments_decode_exactly_while_top_commitments_are_aliased() {
    let upstream = upstream_v1();
    let wire = canonical_wire_v1(upstream, b"downstream");
    let view = ExistingRadixProofViewV1::from_components_v1(&wire, upstream)
        .expect("canonical existing-radix view");
    let [top_d, top_s, _, _] = test_points_v1();
    let commitments = existing_radix_commitments_v1(view, GROUPS_V1 - 1, |group| {
        (group == GROUPS_V1 - 1).then_some((top_d, top_s))
    })
    .expect("last exact radix group");
    assert_eq!(commitments.difference_top, top_d);
    assert_eq!(commitments.slack_top, top_s);
    assert_eq!(
        commitments.difference_low[0],
        view.point_v1(GROUPS_V1 - 1, ROLE_DIFFERENCE_LOW_V1, 0)
            .expect("D low point")
    );
    assert_eq!(
        commitments.slack_low[LOW_DIGITS_V1 - 1],
        view.point_v1(GROUPS_V1 - 1, ROLE_SLACK_LOW_V1, LOW_DIGITS_V1 - 1)
            .expect("S low point")
    );
    assert!(existing_radix_commitments_v1(view, GROUPS_V1, |_| Some((top_d, top_s))).is_none());
    assert_eq!(INVENTORY_POINTS_V1, GROUPS_V1 * 2 * LOW_DIGITS_V1);
    assert_eq!(INVENTORY_BYTES_V1, 11_696 * POINT_BYTES_V1);
}

#[test]
fn direct_alias_is_no_copy_difference_only_and_exact_at_both_boundaries() {
    let upstream = upstream_v1();
    let wire = canonical_wire_v1(upstream, b"downstream");
    let view = ExistingRadixProofViewV1::from_components_v1(&wire, upstream)
        .expect("canonical existing-radix view");
    let alias = RnsNativeExistingRadixDirectAliasV1 {
        inventory: view.inventory,
        pre_z_candidate_root: view.pre_z_candidate_root,
        binding_digest: [0xbc; DIGEST_BYTES_V1],
        origin: RnsNativeExistingRadixAuthenticatedOriginV1 {
            root: view.pre_z_candidate_root,
            base_address: wire.as_ptr() as usize,
            base_len: wire.len(),
            existing_wire_offset: 0,
            existing_wire_len: wire.len(),
            inventory_offset: HEADER_BYTES_V1,
            inventory_len: INVENTORY_BYTES_V1,
        },
    };

    assert_eq!(alias.inventory.len(), INVENTORY_BYTES_V1);
    assert_eq!(
        alias.difference_low_commitment_v1(0, 0),
        view.point_v1(0, ROLE_DIFFERENCE_LOW_V1, 0).ok()
    );
    assert_eq!(
        alias.difference_low_commitment_v1(GROUPS_V1 - 1, LOW_DIGITS_V1 - 1),
        view.point_v1(GROUPS_V1 - 1, ROLE_DIFFERENCE_LOW_V1, LOW_DIGITS_V1 - 1,)
            .ok()
    );
    assert!(alias.difference_low_commitment_v1(GROUPS_V1, 0).is_none());
    assert!(
        alias
            .difference_low_commitment_v1(0, LOW_DIGITS_V1)
            .is_none()
    );
    assert_eq!(alias.pre_z_candidate_root, view.pre_z_candidate_root);
    assert_eq!(alias.binding_digest, [0xbc; DIGEST_BYTES_V1]);
    assert_eq!(DIRECT_ALIAS_COPIED_DIGEST_BYTES_V1, 64);

    let source = include_str!("rns_native_existing_radix_commitment_view.rs");
    let alias_declaration = source
        .find("pub(super) struct RnsNativeExistingRadixDirectAliasV1")
        .expect("purpose-bound direct alias");
    let alias_prefix = &source[alias_declaration.saturating_sub(320)..alias_declaration];
    assert!(!alias_prefix.contains("derive(Clone"));
    assert!(!alias_prefix.contains("derive(Copy"));
    let alias_surface = source[alias_declaration..]
        .split_once("fn existing_radix_commitments_v1")
        .expect("direct alias surface boundary")
        .0;
    assert!(!alias_surface.contains("fn from_raw"));
    assert!(!alias_surface.contains("fn from_points"));
    assert!(!alias_surface.contains("fn inventory_v1"));
    assert!(!alias_surface.contains("fn pre_z_candidate_root_v1"));
    assert!(!alias_surface.contains("fn binding_digest_v1"));
    assert!(!alias_surface.contains("fn borrowed_point_bytes_v1"));
    assert!(!alias_surface.contains("-> &'proof [u8]"));
    let transition = source
        .split_once("pub(super) fn verify_claimed_direct_v2(")
        .expect("purpose-bound alias transition")
        .1
        .split_once("pub(super) fn existing_radix_commitments(")
        .expect("alias transition boundary")
        .0;
    assert!(transition.contains("RnsNativeExistingRadixDirectAliasV1 {"));
    assert!(transition.contains("inventory: self.inventory"));
    assert!(transition.contains("pre_z_candidate_root: self.pre_z_candidate_root"));
    assert!(transition.contains("binding_digest: self.binding_digest"));
    assert!(transition.contains("claimed_successor.verify_claimed_direct_with_alias_v2(alias)"));
    assert!(!transition.contains("(self.previous, alias)"));
    assert!(!transition.contains("to_vec("));
    assert!(!transition.contains("Box::"));
}

#[test]
fn pre_direct_split_retains_exact_allocation_and_ordinary_auth_has_no_second_root_pass() {
    let wire = canonical_wire_v1(upstream_v1(), b"downstream");
    let prefix_len = 17;
    let mut base = vec![0x5a; prefix_len];
    base.extend_from_slice(&wire);
    base.extend_from_slice(&[0xa5; 19]);
    let existing = &base[prefix_len..prefix_len + wire.len()];
    let split = preflight_rns_native_existing_radix_candidate_v1(&base, existing)
        .expect("exact-allocation existing-radix preflight");
    let RnsNativeExistingRadixPreDirectSplitV1 { axis, permit } = split;
    assert_eq!(permit.base_address, base.as_ptr() as usize);
    assert_eq!(permit.base_len, base.len());
    assert_eq!(permit.existing_wire_offset, prefix_len);
    assert_eq!(permit.existing_wire_len, wire.len());
    assert_eq!(permit.inventory_offset, prefix_len + HEADER_BYTES_V1);
    assert_eq!(permit.inventory_len, INVENTORY_BYTES_V1);
    assert_eq!(
        permit.residual_offset,
        prefix_len + HEADER_BYTES_V1 + INVENTORY_BYTES_V1
    );
    assert!(axis.permits_full_direct_bind_v1());

    let source = include_str!("rns_native_existing_radix_commitment_view.rs");
    assert_eq!(
        source
            .matches("canonical_pre_z_candidate_root_v1(inventory)?")
            .count(),
        1,
        "the canonical 11,696-point pass occurs only in self-consistent preflight"
    );
    let ordinary_auth = source
        .split_once("pub(super) fn authenticate_rns_native_existing_radix_commitment_view_v1")
        .expect("ordinary existing-radix authentication")
        .1
        .split_once("#[cfg(test)]")
        .expect("ordinary authentication boundary")
        .0;
    assert!(ordinary_auth.contains("take_existing_radix_validation_permit_v1()"));
    assert!(ordinary_auth.contains("permit.bind_verified_predecessor_v1(&previous)?"));
    assert!(!ordinary_auth.contains("canonical_pre_z_candidate_root_v1"));
    assert!(!ordinary_auth.contains("from_self_consistent_components_v1"));
    assert!(!ordinary_auth.contains("from_components_v1"));
    assert!(!source.contains("copy_verified_projection_root_v1"));
    assert!(!source.contains("fn into_parts"));
}

#[test]
fn sole_z_candidate_root_excludes_transport_and_post_z_axes() {
    let inventory = canonical_inventory_v1();
    let root = canonical_pre_z_candidate_root_v1(&inventory).expect("candidate-only root");
    let mut changed_upstream = upstream_v1();
    changed_upstream.added_inventory_root[0] ^= 1;
    changed_upstream.q_mask_proof_set_root[0] ^= 1;
    assert_eq!(
        canonical_pre_z_candidate_root_v1(&inventory),
        Ok(root),
        "candidate root has no dynamic predecessor input"
    );
    assert_ne!(changed_upstream.digests_v1(), upstream_v1().digests_v1());

    let mut changed_inventory = inventory;
    let replacement = encoded_point_v1(test_points_v1()[2]);
    changed_inventory[..POINT_BYTES_V1].copy_from_slice(&replacement);
    assert_ne!(
        canonical_pre_z_candidate_root_v1(&changed_inventory).expect("changed candidate root"),
        root
    );

    let source = include_str!("rns_native_existing_radix_commitment_view.rs");
    let root_body = source
        .split("fn canonical_pre_z_candidate_root_v1")
        .nth(1)
        .expect("candidate root function")
        .split("fn absorb_upstream_v1")
        .next()
        .expect("candidate root body");
    for forbidden in [
        "absorb_upstream_v1",
        "added_inventory_root",
        "proof_set_root",
        "verified_transcript_root",
        "residual_digest",
        "binding_digest",
        "codec_digest",
        "inverse",
    ] {
        assert!(
            !root_body.contains(forbidden),
            "forbidden pre-z axis: {forbidden}"
        );
    }
}

#[test]
fn existing_radix_boundary_is_private_move_only_non_authorizing_and_fail_closed() {
    assert_eq!(GROUPS_V1, 344);
    assert_eq!(INVENTORY_POINTS_V1, 11_696);
    assert_eq!(INVENTORY_BYTES_V1, 385_968);
    assert_eq!(HEADER_BYTES_V1, 414);
    assert_eq!(MIN_WIRE_BYTES_V1, 386_415);
    assert_eq!(RNS_NATIVE_EXISTING_RADIX_RESIDUAL_MAX_BYTES_V1, 1_817_839);

    let source = include_str!("rns_native_existing_radix_commitment_view.rs");
    let declaration = "pub(super) struct RnsNativeExistingRadixCommitmentPrerequisiteV1";
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
    assert!(!stage.contains("VerifiedReceipt"));
    assert!(!stage.contains("ReleaseAuthorization"));
    assert!(source.contains("EXISTING_RADIX_LOW_COMMITMENT_VIEW_AUTHENTICATED_V1: bool = true"));
    for flag in [
        "EXISTING_RADIX_INVERSES_POST_Z_VERIFIED_V1: bool = false",
        "RADIX_RECONSTRUCTION_VERIFIED_V1: bool = false",
        "CENTERING_SUBTRACTION_VERIFIED_V1: bool = false",
        "GLOBAL_LOOKUP_PRE_Z_READY_V1: bool = false",
        "GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1: bool = false",
        "CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1: bool = false",
        "RELEASE_READY_V1: bool = false",
    ] {
        assert!(source.contains(flag));
    }
    assert!(source.contains("inventory.comparator_top_commitments(owner)"));

    let parent = include_str!("../mkhe.rs");
    assert_eq!(
        parent
            .matches("mod rns_native_existing_radix_commitment_view;")
            .count(),
        1
    );
    assert!(!parent.contains("pub use rns_native_existing_radix_commitment_view"));
}
