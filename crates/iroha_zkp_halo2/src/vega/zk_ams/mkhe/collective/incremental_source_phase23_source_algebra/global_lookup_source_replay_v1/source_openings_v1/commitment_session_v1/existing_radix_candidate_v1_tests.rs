use super::*;
use hex_literal::hex;
use std::panic::AssertUnwindSafe;

const CANDIDATE_SOURCE_V1: &str = include_str!("existing_radix_candidate_v1.rs");
const CANDIDATE_TEST_SOURCE_V1: &str = include_str!("existing_radix_candidate_v1_tests.rs");
const SESSION_SOURCE_V1: &str = include_str!("../commitment_session_v1.rs");
const VERIFIER_SOURCE_V1: &str =
    include_str!("../../../../../rns_native_existing_radix_commitment_view.rs");

const PRE_Z_MANIFEST_KAT_V1: [u8; 32] =
    hex!("d3dbe52d9f2fa7a65f9b084ee8d87be75223f6aa2f05e7b8533b495767aac271");
const SOURCE_OPENING_MAPPING_KAT_V1: [u8; 32] =
    hex!("8216632703174865bcbf16b05ed3c8a3571dc11672cf5dc1c2f00c288fa912f1");
const SOURCE_COMMITMENTS_ROOT_KAT_V1: [u8; 32] =
    hex!("fb767ac66171a3203e16909d1f84babdabba467e3e5e547733fe197d45667c78");
const PATTERNED_CANDIDATE_ROOT_KAT_V1: [u8; 32] =
    hex!("b593dd370462a64cf69be83bd86ebd28634a55591e63d2d400bc3511c9280453");
const PATTERNED_CANDIDATE_WIRE_KAT_V1: [u8; 32] =
    hex!("612bce88df4ccb0cb47b8145cc3b6ad9f24c9811fdd84135c2a98a74e50ca016");
const CANDIDATE_BLINDING_ROOT_KAT_V1: [u8; 32] =
    hex!("0b2c297494ff252223ff18bb16032f1ffbe12b57e7141cac9fbb84ab4f672989");
const CANDIDATE_OWNER_BINDING_KAT_V1: [u8; 32] =
    hex!("246379873a55b2f95371f024182280545ff17c60e82019e0844b4ab2e21e813e");
const FIRST_CANDIDATE_BLINDING_KAT_V1: [u8; 32] =
    hex!("236d3b8112318b84c14cb221111bf3e0fc756c0e8fc09c2d9c143499de6e1b2e");
const FIRST_CANDIDATE_TOKEN_KAT_V1: [u8; 32] =
    hex!("78f7b1117eb31151b8281519dccf9a90aa3889d634d119eb785b52d0c310ded2");

fn patterned_points_v1() -> [Point; 3] {
    [
        Point::canonical_generator().expect("canonical generator"),
        Point::from_non_identity_wire_bytes_exact(&hex!(
            "8016f70c3f35b3257896971b306635647bc52eb7cad7a5eca1a42f2340737749e3"
        ))
        .expect("twice generator"),
        Point::from_non_identity_wire_bytes_exact(&hex!(
            "00a37dc092877e239385cd8392ba2360ce1859a37f7a2b9c626b336608d2ce4cfe"
        ))
        .expect("seven times generator"),
    ]
}

fn patterned_candidate_wire_v1() -> Vec<u8> {
    let points = patterned_points_v1().map(|point| {
        point
            .to_non_identity_wire_bytes()
            .expect("non-identity patterned point")
    });
    let mut wire = Vec::with_capacity(EXISTING_RADIX_CANDIDATE_WIRE_BYTES_V1);
    for wire_ordinal in 0..EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 {
        wire.extend_from_slice(&points[wire_ordinal % points.len()]);
    }
    wire
}

fn source_complete_session_v1(
    proof_context: [u8; 32],
    seed: [u8; 32],
    fault: TestEntropyFaultV1,
) -> GlobalLookupCommitmentSessionV1<SourceOpeningCompleteStageV1> {
    let opening_context = [0x32; 32];
    let generator = Point::canonical_generator().expect("canonical generator");
    let mut session =
        GlobalLookupProofSessionEntropySealV1::test_only_with_fault_v1(proof_context, seed, fault)
            .expect("test proof session");
    session
        .bind_source_opening_context_v1(opening_context)
        .expect("source context");
    for ordinal in 0..SOURCE_OPENING_GROUP_COUNT_V1 as u32 {
        let (_chunk, _scalar) = session
            .sample_source_blinding_v1(ordinal)
            .expect("source blinding");
        session
            .adopt_source_commitment_v1(ordinal, &generator)
            .expect("source commitment");
    }
    let commitments_root = session
        .live
        .as_ref()
        .expect("live source session")
        .inventory
        .adopted_source_commitments_root_v1(opening_context)
        .expect("source root");
    assert_eq!(commitments_root, SOURCE_COMMITMENTS_ROOT_KAT_V1);
    session
        .complete_source_opening_v1(opening_context, commitments_root, [0x52; 32])
        .expect("complete source session")
}

fn begin_candidate_assembly_v1(
    proof_context: [u8; 32],
    seed: [u8; 32],
) -> RnsNativeExistingRadixCandidateAssemblyV1 {
    source_complete_session_v1(proof_context, seed, TestEntropyFaultV1::None)
        .into_existing_radix_candidate_assembly_v1()
        .expect("candidate assembly")
}

fn complete_patterned_candidate_v1() -> RnsNativeExistingRadixCandidateOwnerV1 {
    let points = patterned_points_v1();
    let mut assembly = begin_candidate_assembly_v1([0x31; 32], [0x41; 32]);
    for wire_ordinal in 0..EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 as u32 {
        let coordinate =
            existing_radix_candidate_coordinate_v1(wire_ordinal).expect("candidate coordinate");
        let blinding = assembly
            .sample_next_blinding_v1(
                usize::from(coordinate.group),
                coordinate.role,
                usize::from(coordinate.column),
            )
            .expect("candidate blinding");
        assert!(!blinding.scalar_v1().is_zero());
        if wire_ordinal == 0 {
            assert_eq!(
                blinding.scalar_v1().to_be_bytes(),
                FIRST_CANDIDATE_BLINDING_KAT_V1
            );
            assert_eq!(blinding.binding_digest, FIRST_CANDIDATE_TOKEN_KAT_V1);
        }
        assembly
            .adopt_next_commitment_v1(
                blinding,
                &points[usize::try_from(wire_ordinal).unwrap() % points.len()],
            )
            .expect("candidate adoption");
    }
    assembly.finish_v1().expect("candidate owner")
}

#[test]
fn candidate_geometry_is_an_exact_wire_to_physical_inventory_bijection() {
    let mut seen = vec![false; EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1];
    for wire_ordinal in 0..EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 as u32 {
        let coordinate = existing_radix_candidate_coordinate_v1(wire_ordinal).unwrap();
        assert_eq!(coordinate.wire_ordinal, wire_ordinal);
        assert_eq!(usize::from(coordinate.group), wire_ordinal as usize / 34);
        let local = wire_ordinal as usize % 34;
        assert_eq!(usize::from(coordinate.column), local % 17);
        assert_eq!(
            coordinate.role,
            if local < 17 {
                RnsNativeExistingRadixCandidateRoleV1::DifferenceLow
            } else {
                RnsNativeExistingRadixCandidateRoleV1::SlackLow
            }
        );
        let physical = usize::try_from(
            coordinate.inventory_ordinal - EXISTING_RADIX_CANDIDATE_FIRST_INVENTORY_ORDINAL_V1,
        )
        .unwrap();
        assert!(!seen[physical]);
        seen[physical] = true;
    }
    assert!(!seen.contains(&false));
    assert!(
        existing_radix_candidate_coordinate_v1(EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 as u32)
            .is_err()
    );

    for (wire, physical) in [
        (0, 344),
        (16, 360),
        (17, 6_192),
        (33, 6_208),
        (34, 361),
        (11_662, 6_175),
        (11_678, 6_191),
        (11_679, 12_023),
        (11_695, 12_039),
    ] {
        assert_eq!(
            existing_radix_candidate_coordinate_v1(wire)
                .unwrap()
                .inventory_ordinal,
            physical
        );
    }
}

#[test]
fn manifest_candidate_root_and_candidate_serialization_hashes_are_frozen() {
    assert_eq!(
        exact_source_opening_mapping_digest_v1().unwrap(),
        SOURCE_OPENING_MAPPING_KAT_V1
    );
    assert_eq!(
        existing_radix_pre_z_manifest_digest_v1(),
        PRE_Z_MANIFEST_KAT_V1
    );
    let wire = patterned_candidate_wire_v1();
    assert_eq!(wire.len(), 385_968);
    assert_eq!(
        existing_radix_candidate_root_from_wire_v1(&wire).unwrap(),
        PATTERNED_CANDIDATE_ROOT_KAT_V1
    );
    let mut wire_hash = Keccak256::new();
    wire_hash.update(&wire);
    assert_eq!(wire_hash.finalize(), PATTERNED_CANDIDATE_WIRE_KAT_V1);

    let mut swapped = wire.clone();
    let first: [u8; 33] = swapped[..33].try_into().unwrap();
    let second: [u8; 33] = swapped[33..66].try_into().unwrap();
    swapped[..33].copy_from_slice(&second);
    swapped[33..66].copy_from_slice(&first);
    assert_ne!(
        existing_radix_candidate_root_from_wire_v1(&swapped).unwrap(),
        PATTERNED_CANDIDATE_ROOT_KAT_V1
    );
    assert!(existing_radix_candidate_root_from_wire_v1(&wire[..wire.len() - 1]).is_err());
}

#[test]
fn completed_owner_retains_blindings_and_emits_only_the_exact_candidate_section() {
    let mut owner = complete_patterned_candidate_v1();
    owner.validate_v1().unwrap();
    assert_eq!(owner.candidate_root, PATTERNED_CANDIDATE_ROOT_KAT_V1);
    assert_eq!(owner.blinding_root, CANDIDATE_BLINDING_ROOT_KAT_V1);
    assert_eq!(owner.owner_binding_digest, CANDIDATE_OWNER_BINDING_KAT_V1);
    assert_eq!(owner.blindings.len(), 11_696);

    let expected = patterned_candidate_wire_v1();
    let mut destination = vec![0xa5; 9];
    let receipt = owner
        .append_candidate_section_v1(&mut destination)
        .expect("exact candidate append");
    assert_eq!(receipt.destination_offset, 9);
    assert_eq!(receipt.destination_len, 385_968);
    assert_eq!(receipt.candidate_root, PATTERNED_CANDIDATE_ROOT_KAT_V1);
    assert_eq!(receipt.owner_binding_digest, owner.owner_binding_digest);
    assert_eq!(&destination[..9], &[0xa5; 9]);
    assert_eq!(&destination[9..], expected.as_slice());
    let mut duplicate_destination = Vec::new();
    assert!(
        owner
            .append_candidate_section_v1(&mut duplicate_destination)
            .is_err()
    );
    assert!(duplicate_destination.is_empty());

    let live = owner.session.live.as_ref().unwrap();
    assert_eq!(live.next_global_ordinal, 12_040);
    assert_eq!(
        live.next_purpose,
        GlobalLookupCommitmentPurposeV1::ComparatorDifferenceTop
    );
    assert_eq!(live.next_purpose_ordinal, 0);
    assert!(live.inventory.slots[..12_040].iter().all(Option::is_some));
    assert!(live.inventory.slots[12_040..].iter().all(Option::is_none));

    owner.session.live.as_mut().unwrap().inventory.slots[344]
        .as_mut()
        .unwrap()
        .point_wire[32] ^= 1;
    assert!(owner.validate_v1().is_err());
    let mut unchanged = vec![0x5a; 7];
    assert!(owner.append_candidate_section_v1(&mut unchanged).is_err());
    assert_eq!(unchanged, vec![0x5a; 7]);
}

#[test]
fn order_token_identity_entropy_early_finish_and_unwind_fail_closed() {
    let mut wrong_order = begin_candidate_assembly_v1([1; 32], [2; 32]);
    assert!(
        wrong_order
            .sample_next_blinding_v1(0, RnsNativeExistingRadixCandidateRoleV1::SlackLow, 0,)
            .is_err()
    );
    assert!(wrong_order.live.is_none());

    let mut duplicate = begin_candidate_assembly_v1([3; 32], [4; 32]);
    let _pending = duplicate
        .sample_next_blinding_v1(0, RnsNativeExistingRadixCandidateRoleV1::DifferenceLow, 0)
        .unwrap();
    assert!(
        duplicate
            .sample_next_blinding_v1(0, RnsNativeExistingRadixCandidateRoleV1::DifferenceLow, 0,)
            .is_err()
    );
    assert!(duplicate.live.is_none());

    // Even byte-identical semantic inputs and deterministic entropy do not make
    // a sampled token transferable between two in-memory assemblies.
    let mut left = begin_candidate_assembly_v1([5; 32], [6; 32]);
    let mut right = begin_candidate_assembly_v1([5; 32], [6; 32]);
    let left_token = left
        .sample_next_blinding_v1(0, RnsNativeExistingRadixCandidateRoleV1::DifferenceLow, 0)
        .unwrap();
    let right_token = right
        .sample_next_blinding_v1(0, RnsNativeExistingRadixCandidateRoleV1::DifferenceLow, 0)
        .unwrap();
    assert_eq!(left_token.scalar_v1(), right_token.scalar_v1());
    assert_eq!(left_token.binding_digest, right_token.binding_digest);
    assert_ne!(left_token.assembly_instance, right_token.assembly_instance);
    assert!(
        right
            .adopt_next_commitment_v1(left_token, &Point::canonical_generator().unwrap())
            .is_err()
    );
    assert!(right.live.is_none());

    let mut identity = begin_candidate_assembly_v1([9; 32], [10; 32]);
    let identity_token = identity
        .sample_next_blinding_v1(0, RnsNativeExistingRadixCandidateRoleV1::DifferenceLow, 0)
        .unwrap();
    assert!(
        identity
            .adopt_next_commitment_v1(identity_token, &Point::identity())
            .is_err()
    );
    assert!(identity.live.is_none());

    let mut unavailable =
        source_complete_session_v1([11; 32], [12; 32], TestEntropyFaultV1::ErrorAt(344))
            .into_existing_radix_candidate_assembly_v1()
            .unwrap();
    assert!(
        unavailable
            .sample_next_blinding_v1(0, RnsNativeExistingRadixCandidateRoleV1::DifferenceLow, 0,)
            .is_err()
    );
    assert!(unavailable.live.is_none());

    assert!(
        begin_candidate_assembly_v1([13; 32], [14; 32])
            .finish_v1()
            .is_err()
    );
    let unwind = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut assembly = begin_candidate_assembly_v1([15; 32], [16; 32]);
        assembly.panic_after_take_for_test_v1();
    }));
    assert!(unwind.is_err());
}

#[test]
fn owner_is_move_only_source_bound_and_all_stronger_gates_remain_false() {
    assert!(core::mem::needs_drop::<
        RnsNativeExistingRadixCandidateAssemblyV1,
    >());
    assert!(core::mem::needs_drop::<
        RnsNativeExistingRadixCandidateBlindingV1,
    >());
    assert!(core::mem::needs_drop::<
        RnsNativeExistingRadixCandidateOwnerV1,
    >());
    assert_eq!(EXISTING_RADIX_CANDIDATE_RETAINED_BLINDING_BYTES_V1, 374_272);
    assert_eq!(EXISTING_RADIX_CANDIDATE_PUBLIC_WIRE_BYTES_V1, 385_968);
    assert_eq!(EXISTING_RADIX_CANDIDATE_SEMANTIC_BYTES_V1, 760_240);
    assert_eq!(EXISTING_RADIX_CANDIDATE_NEW_FILE_BYTES_V1, 0);
    assert_eq!(EXISTING_RADIX_CANDIDATE_NEW_IO_BYTES_V1, 0);
    const {
        assert!(EXISTING_RADIX_CANDIDATE_OWNER_MATERIALIZED_V1);
        assert!(!LIVE_PHASE23_EXISTING_RADIX_SOURCE_INTEGRATED_V1);
        assert!(!DIRECT_QUOTIENT_OPENING_OWNERS_INTEGRATED_V1);
        assert!(!RESOURCE_EVIDENCE_ACCEPTED_V1);
        assert!(!READINESS_ACCEPTED_V1);
        assert!(!RELEASE_READY_V1);
        assert!(!RELEASE_COMPLETE_V1);
    };

    for required in [
        "session: GlobalLookupCommitmentSessionV1<ExistingRadixCandidateCompleteStageV1>",
        "blindings: ZeroizingT256ScalarVecV1",
        "append_permit: Option<ExistingRadixCandidateAppendPermitV1>",
        "live: Option<ExistingRadixCandidateAssemblyLiveV1>",
        "assembly_instance: u64",
        "NEXT_EXISTING_RADIX_ASSEMBLY_INSTANCE_V1",
        "let mut live = self\n            .live\n            .take()",
        "proof_session_entropy: Infallible",
        "append_candidate_section_v1",
        "EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1: usize =",
        "EXISTING_RADIX_CANDIDATE_WIRE_BYTES_V1: usize =",
    ] {
        assert!(
            CANDIDATE_SOURCE_V1.contains(required) || SESSION_SOURCE_V1.contains(required),
            "missing owner guard: {required}"
        );
    }
    for forbidden in [
        "impl Clone for RnsNativeExistingRadixCandidateAssemblyV1",
        "impl Copy for RnsNativeExistingRadixCandidateAssemblyV1",
        "impl Clone for RnsNativeExistingRadixCandidateOwnerV1",
        "impl Copy for RnsNativeExistingRadixCandidateOwnerV1",
        "fn into_parts",
        "fn blindings_v1",
        "fn points_v1",
        "fn candidate_root_v1(&self)",
        "pub(in crate::vega::zk_ams::mkhe) fn scalar_v1",
        "pub(in crate::vega::zk_ams::mkhe) fn adopt_next_commitment_v1",
        "Serialize",
        "Deserialize",
        "Encode",
        "Decode",
    ] {
        assert!(
            !CANDIDATE_SOURCE_V1.contains(forbidden),
            "forbidden owner surface: {forbidden}"
        );
    }
    assert!(CANDIDATE_SOURCE_V1.lines().count() <= 900);
    assert!(CANDIDATE_SOURCE_V1.len() <= 45_000);
    assert!(CANDIDATE_TEST_SOURCE_V1.lines().count() <= 500);
    assert!(CANDIDATE_TEST_SOURCE_V1.len() <= 25_000);
}

#[test]
fn source_and_verifier_contracts_freeze_the_same_candidate_geometry_and_domains() {
    for exact in [
        "assert!(INVENTORY_POINTS_V1 == 11_696)",
        "assert!(INVENTORY_BYTES_V1 == 385_968)",
    ] {
        assert!(
            VERIFIER_SOURCE_V1.contains(exact),
            "verifier drift: {exact}"
        );
    }
    for exact in [
        "assert!(EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 == 11_696)",
        "assert!(EXISTING_RADIX_CANDIDATE_WIRE_BYTES_V1 == 385_968)",
    ] {
        assert!(CANDIDATE_SOURCE_V1.contains(exact), "prover drift: {exact}");
    }
    for exact in [
        "ordinal=((group*2+role-index)*17+column)",
        "iroha.zk-ams.v1.mkhe.rns-native-existing-radix.pre-z-manifest",
        "iroha.zk-ams.v1.mkhe.rns-native-existing-radix.pre-z-candidate-root",
        "top-commitments-are-aliased-from-original-inventory-and-never-encoded-here",
        "exclude-full-added-inventory-root,S3/S5/S8/S10-11-roots,residuals,bindings,codec,and-all-inverse-roots",
    ] {
        assert!(
            VERIFIER_SOURCE_V1.contains(exact),
            "verifier drift: {exact}"
        );
        assert!(CANDIDATE_SOURCE_V1.contains(exact), "prover drift: {exact}");
    }
    assert!(
        VERIFIER_SOURCE_V1.contains("EXISTING_RADIX_INVERSES_POST_Z_VERIFIED_V1: bool = false")
    );
    assert!(
        CANDIDATE_SOURCE_V1
            .contains("LIVE_PHASE23_EXISTING_RADIX_SOURCE_INTEGRATED_V1: bool = false")
    );
    assert!(
        CANDIDATE_SOURCE_V1.contains("DIRECT_QUOTIENT_OPENING_OWNERS_INTEGRATED_V1: bool = false")
    );
}
