use super::*;

const ORACLE_RELEASE_MODULI_V1: [u64; 38] = [
    1_152_921_504_606_584_833,
    1_152_921_504_598_720_513,
    1_152_921_504_592_429_057,
    1_152_921_504_581_419_009,
    1_152_921_504_580_894_721,
    1_152_921_504_578_273_281,
    1_152_921_504_577_748_993,
    1_152_921_504_577_486_849,
    1_152_921_504_568_836_097,
    1_152_921_504_565_166_081,
    1_152_921_504_563_331_073,
    1_152_921_504_556_515_329,
    1_152_921_504_555_466_753,
    1_152_921_504_554_156_033,
    1_152_921_504_552_583_169,
    1_152_921_504_542_883_841,
    1_152_921_504_538_951_681,
    1_152_921_504_537_378_817,
    1_152_921_504_531_873_793,
    1_152_921_504_521_650_177,
    1_152_921_504_509_853_697,
    1_152_921_504_508_280_833,
    1_152_921_504_506_970_113,
    1_152_921_504_495_697_921,
    1_152_921_504_491_241_473,
    1_152_921_504_488_620_033,
    1_152_921_504_479_444_993,
    1_152_921_504_470_794_241,
    1_152_921_504_468_172_801,
    1_152_921_504_462_929_921,
    1_152_921_504_462_667_777,
    1_152_921_504_455_589_889,
    1_152_921_504_447_987_713,
    1_152_921_504_442_482_689,
    1_152_921_504_436_191_233,
    1_152_921_504_427_278_337,
    1_152_921_504_419_414_017,
    1_152_921_504_409_190_401,
];
const ORACLE_STATEMENT_DIGEST_V1: [u8; 32] = [
    0xb5, 0x75, 0x2b, 0x54, 0x52, 0xab, 0x11, 0x1d, 0xbc, 0x09, 0xa2, 0x77, 0xe3, 0xe5, 0x70, 0x69,
    0x0e, 0xec, 0x75, 0xdc, 0x62, 0x11, 0x44, 0xea, 0xab, 0x45, 0x4b, 0x8d, 0xa0, 0xe1, 0x15, 0xef,
];
const ORACLE_AUTHORITY_DIGEST_V1: [u8; 32] = [
    0xbf, 0xf1, 0x99, 0x78, 0xb9, 0x69, 0xf0, 0xb1, 0x29, 0x12, 0x97, 0xc4, 0xc3, 0x73, 0xd5, 0x95,
    0xcb, 0x38, 0xde, 0x32, 0xf2, 0xce, 0x13, 0x94, 0xbf, 0x22, 0xf8, 0x8e, 0x60, 0xb0, 0xa2, 0xf0,
];

fn axes_fixture() -> DirectCommonAAxesV1 {
    DirectCommonAAxesV1 {
        profile_digest: [1; 32],
        context_digest: [2; 32],
        roster_digest: [3; 32],
        key_material_digest: [4; 32],
        epoch: 5,
        transcript_digest: [6; 32],
        collective_public_key_digest: [7; 32],
        secret_lineage_root: [8; 32],
        target_tag: 1,
        evaluated_key_ordinal: 0,
        digit_index: 9,
        galois_exponent: 0,
        common_a_seed: [10; 32],
        initial_round_digest: [11; 32],
    }
}

fn oracle_context_frame(axes: DirectCommonAAxesV1) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.push(1);
    bytes.push(1);
    bytes.extend_from_slice(&axes.profile_digest);
    bytes.extend_from_slice(&axes.context_digest);
    bytes.extend_from_slice(&axes.roster_digest);
    bytes.extend_from_slice(&axes.key_material_digest);
    bytes.extend_from_slice(&axes.epoch.to_be_bytes());
    bytes.extend_from_slice(&axes.transcript_digest);
    bytes.extend_from_slice(&axes.collective_public_key_digest);
    bytes.extend_from_slice(&axes.secret_lineage_root);
    bytes.push(axes.target_tag);
    bytes.push(axes.evaluated_key_ordinal);
    bytes.push(axes.digit_index);
    bytes.extend_from_slice(&axes.galois_exponent.to_be_bytes());
    bytes.extend_from_slice(&axes.common_a_seed);
    bytes.extend_from_slice(&axes.initial_round_digest);
    bytes
}

fn oracle_sampler_frame(axes: DirectCommonAAxesV1, limb: u16, modulus: u64) -> Vec<u8> {
    let context = oracle_context_frame(axes);
    let mut frame = Vec::new();
    frame.extend_from_slice(b"iroha.zk-ams.v1.mkhe.direct-collective-common-a-limb");
    frame.push(1);
    frame.push(1);
    frame.extend_from_slice(&(context.len() as u32).to_be_bytes());
    frame.extend_from_slice(&context);
    frame.extend_from_slice(&limb.to_be_bytes());
    frame.extend_from_slice(&modulus.to_be_bytes());
    frame.extend_from_slice(&131_072_u32.to_be_bytes());
    frame
}

fn oracle_residues(frame: &[u8], modulus: u64, count: usize) -> Vec<u64> {
    let zone = u64::MAX - u64::MAX % modulus;
    let mut stream = Shake256Reader::new(frame);
    let mut output = Vec::new();
    for _ in 0..count {
        let mut accepted = None;
        for _ in 0..128 {
            let mut bytes = [0_u8; 8];
            stream.read(&mut bytes);
            let candidate = u64::from_le_bytes(bytes);
            if candidate < zone {
                accepted = Some(candidate % modulus);
                break;
            }
        }
        output.push(accepted.expect("oracle sample must terminate"));
    }
    output
}

fn oracle_statement_digest(axes: DirectCommonAAxesV1) -> [u8; 32] {
    let context = oracle_context_frame(axes);
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.direct-collective-common-a-statement");
    hash.update(&[1, 1]);
    hash.update(&(context.len() as u32).to_be_bytes());
    hash.update(&context);
    hash.update(&131_072_u32.to_be_bytes());
    hash.update(&38_u16.to_be_bytes());
    for (limb, modulus) in ORACLE_RELEASE_MODULI_V1.iter().copied().enumerate() {
        let residues = oracle_residues(
            &oracle_sampler_frame(axes, limb as u16, modulus),
            modulus,
            131_072,
        );
        hash.update(&(limb as u16).to_be_bytes());
        hash.update(&modulus.to_be_bytes());
        hash.update(&131_072_u32.to_be_bytes());
        for residue in residues {
            hash.update(&residue.to_be_bytes());
        }
    }
    hash.finalize()
}

fn oracle_authority_digest(axes: DirectCommonAAxesV1, statement_digest: [u8; 32]) -> [u8; 32] {
    let context = oracle_context_frame(axes);
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v1.mkhe.direct-collective-common-a-authority");
    hash.update(&[1, 1]);
    hash.update(&(context.len() as u32).to_be_bytes());
    hash.update(&context);
    hash.update(&statement_digest);
    hash.finalize()
}

#[test]
fn independent_oracle_agrees_on_exact_frames_and_selected_coefficients() {
    let axes = axes_fixture();
    let encoded = axes.encode();
    assert_eq!(encoded.as_slice(), oracle_context_frame(axes));
    assert_eq!(encoded.len(), 305);
    let profile = release_profile_v1();
    assert_eq!(profile.moduli, ORACLE_RELEASE_MODULI_V1.as_slice());
    for limb in [0_usize, 1, 37] {
        let modulus = ORACLE_RELEASE_MODULI_V1[limb];
        let production = sampler_frame(encoded, limb as u16, modulus).unwrap();
        let oracle = oracle_sampler_frame(axes, limb as u16, modulus);
        assert_eq!(production, oracle);
        let expected = oracle_residues(&oracle, modulus, 4);
        let pinned: &[u64] = match limb {
            0 => &[
                392_354_587_925_662_505,
                900_916_147_440_608_621,
                958_473_696_361_119_355,
                533_196_965_849_736_305,
            ],
            1 => &[
                193_564_197_762_333_777,
                791_007_389_894_827_650,
                810_198_759_346_422_736,
                738_542_574_314_702_500,
            ],
            37 => &[
                501_387_272_893_049_524,
                52_188_082_923_482_282,
                371_011_121_692_909_412,
                527_051_052_324_231_775,
            ],
            _ => unreachable!(),
        };
        assert_eq!(expected, pinned);
        let mut stream = Shake256Reader::new(&production);
        let zone = u64::MAX - u64::MAX % modulus;
        let mut budget = 4 * 128;
        let observed = (0..4)
            .map(|_| {
                sample_residue(modulus, zone, &mut budget, |bytes| stream.read(bytes)).unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(observed, expected);
    }
}

#[test]
fn independent_oracle_agrees_on_complete_release_statement_digest() {
    let axes = axes_fixture();
    let expected = oracle_statement_digest(axes);
    let mut stream = DirectCommonAStatementStreamV1::begin_for_test(axes).unwrap();
    let mut workspace = vec![0_u64; 131_072];
    for _ in 0..38 {
        stream.derive_next_limb_into(&mut workspace).unwrap();
    }
    let authority = stream.finish().unwrap();
    let expected_authority = oracle_authority_digest(axes, expected);
    assert_eq!(authority.statement_digest, expected);
    assert_eq!(expected, ORACLE_STATEMENT_DIGEST_V1);
    assert_eq!(authority.authority_digest, expected_authority);
    assert_eq!(expected_authority, ORACLE_AUTHORITY_DIGEST_V1);
}

#[test]
fn every_authority_axis_is_transcript_visible_but_party_is_absent() {
    let baseline_axes = axes_fixture();
    let baseline = baseline_axes.encode();
    let mut changed = Vec::new();
    macro_rules! changed_axis {
        ($field:ident, $value:expr) => {{
            let mut axes = baseline_axes;
            axes.$field = $value;
            changed.push(axes);
        }};
    }
    changed_axis!(profile_digest, [21; 32]);
    changed_axis!(context_digest, [22; 32]);
    changed_axis!(roster_digest, [23; 32]);
    changed_axis!(key_material_digest, [24; 32]);
    changed_axis!(epoch, 25);
    changed_axis!(transcript_digest, [26; 32]);
    changed_axis!(collective_public_key_digest, [27; 32]);
    changed_axis!(secret_lineage_root, [28; 32]);
    changed_axis!(target_tag, 2);
    changed_axis!(evaluated_key_ordinal, 1);
    changed_axis!(digit_index, 29);
    changed_axis!(galois_exponent, 31);
    changed_axis!(common_a_seed, [32; 32]);
    changed_axis!(initial_round_digest, [33; 32]);
    for axes in changed {
        assert_ne!(axes.encode(), baseline);
    }
    let source = include_str!("direct_common_a_v1.rs");
    let axes_source = source
        .split("struct DirectCommonAAxesV1")
        .nth(1)
        .unwrap()
        .split("impl DirectCommonAAxesV1")
        .next()
        .unwrap();
    assert!(!axes_source.contains("party"));
}

#[test]
fn rejection_bound_budget_and_error_semantics_are_exact() {
    let modulus = 17;
    let zone = u64::MAX - u64::MAX % modulus;
    let mut candidates = vec![u64::MAX; 127];
    candidates.push(5);
    let mut cursor = 0;
    let mut budget = 128;
    assert_eq!(
        sample_residue(modulus, zone, &mut budget, |bytes| {
            *bytes = candidates[cursor].to_le_bytes();
            cursor += 1;
        }),
        Ok(5)
    );
    assert_eq!(cursor, 128);
    assert_eq!(budget, 0);

    let mut budget = 128;
    assert_eq!(
        sample_residue(modulus, zone, &mut budget, |bytes| {
            *bytes = u64::MAX.to_le_bytes();
        }),
        Err(ZkAmsMkheErrorV1::InvalidProfile)
    );
    assert_eq!(budget, 0);
    let mut budget = 0;
    assert_eq!(
        sample_residue(modulus, zone, &mut budget, |_| {}),
        Err(ZkAmsMkheErrorV1::ResourceCeilingExceeded)
    );
}

#[test]
fn stream_is_strictly_ordered_poisoned_and_one_limb_bounded() {
    assert_eq!(DIRECT_COMMON_A_LIMB_WORKSPACE_BYTES_V1, 1_048_576);
    assert_eq!(DIRECT_COMMON_A_MAX_CANDIDATES_V1, 637_534_208);
    let encoded = axes_fixture().encode();
    let first_modulus = release_profile_v1().moduli[0];
    assert_eq!(
        sampler_frame(encoded, 1, first_modulus),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );
    assert_eq!(
        sampler_frame(encoded, 38, first_modulus),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );
    let mut stream = DirectCommonAStatementStreamV1::begin_for_test(axes_fixture()).unwrap();
    assert_eq!(
        stream.derive_next_limb_into(&mut [0_u64; 1]),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );
    let mut exact = vec![0_u64; ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1];
    assert_eq!(
        stream.derive_next_limb_into(&mut exact),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );
    assert_eq!(
        stream.finish().map(|_| ()),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );
    let mut duplicate = DirectCommonAStatementStreamV1::begin_for_test(axes_fixture()).unwrap();
    duplicate.next_limb = DIRECT_COMMON_A_RELEASE_LIMBS_V1;
    assert_eq!(
        duplicate.derive_next_limb_into(&mut exact),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );
    assert!(duplicate.failed);
    assert_eq!(
        duplicate.finish().map(|_| ()),
        Err(ZkAmsMkheErrorV1::InvalidPolynomial)
    );
    let source = include_str!("direct_common_a_v1.rs");
    let derive = source
        .split("fn derive_next_limb_into(&mut self")
        .nth(1)
        .unwrap()
        .split("fn derive_next_limb_inner")
        .next()
        .unwrap();
    assert!(
        derive.find("self.failed = true;").unwrap()
            < derive.find("derive_next_limb_inner(output)").unwrap()
    );
    assert!(!source.contains("39_845_888]"));
    assert!(
        !source.contains("DIRECT_COMMON_A_RELEASE_LIMBS_V1 * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1")
    );
}

#[test]
fn replay_authority_is_move_only_opaque_and_has_no_digest_escape() {
    let source = include_str!("direct_common_a_v1.rs");
    let replay = source
        .split("pub(super) struct DirectCommonAReplayV1")
        .nth(1)
        .unwrap()
        .split("/// Derive one common-`a` statement")
        .next()
        .unwrap();
    assert!(replay.contains("expected_statement_digest: [u8; 32]"));
    assert!(replay.contains("capability.validate()?;"));
    assert!(replay.contains("PersistentDirectRelationV1::RkgRoundOne"));
    assert!(replay.contains("selector.context_digest != context.digest()"));
    assert!(replay.contains("selector.evaluated_key_ordinal != context.evaluated_key_ordinal()"));
    assert!(replay.contains("selector.digit_index != context.digit_index()"));
    assert!(replay.contains("DirectCommonAStatementStreamV1::begin(context)?"));
    assert!(replay.contains("self.stream.derive_next_limb_into(output)"));
    assert!(replay.contains("#[cfg(test)]"));
    assert!(replay.contains("inject_unwind_on_next_derive_for_test"));
    assert!(source.contains("self.failed = true;"));
    assert!(source.contains("core::mem::replace(&mut self.inject_unwind_on_next_derive, false)"));
    assert!(source.contains("injected direct common-a replay derive unwind"));
    assert!(replay.contains("self.stream.finish()?.statement_digest_for(self.context)?"));
    assert!(replay.contains("observed != self.expected_statement_digest"));
    assert!(!replay.contains("derive(Clone"));
    assert!(!replay.contains("impl Clone"));
    assert!(!replay.contains("impl Copy"));
    assert!(!replay.contains("Deref"));
    assert!(!replay.contains("callback"));
    assert!(!replay.contains("to_bytes"));
    assert!(!replay.contains("from_bytes"));
    assert!(!replay.contains("statement_digest(&self)"));
    assert!(!replay.contains("-> [u8; 32]"));
    assert!(!source.contains("pub(super) fn common_a_statement_digest"));
    assert!(!source.contains("pub(super) fn expected_statement_digest"));
    assert!(!source.contains("pub(super) fn authority_digest"));
    let active = include_str!("../active_exact_binding.rs");
    let verifier = active
        .split("pub(super) fn verify_and_consume_direct_relation_use_v1")
        .nth(1)
        .unwrap()
        .split("/// Sole production minting boundary")
        .next()
        .unwrap();
    assert!(verifier.contains("Err(ZkAmsMkheErrorV1::ReleaseUnavailable)"));
    assert!(!verifier.contains("DirectCommonAReplayV1"));
}

#[test]
fn typed_selector_uses_only_the_creator_replay_typestate() {
    let active = include_str!("../active_exact_binding.rs");
    let common_a = include_str!("direct_common_a_v1.rs");
    let creator = include_str!("direct_common_a_v1/creator_replay_v1.rs");
    let constructor_tail = common_a
        .split("fn new_rkg_round_one_selector_v1")
        .nth(1)
        .unwrap();
    let (constructor, _) = constructor_tail.split_once("fn sampler_frame(").unwrap();
    assert!(constructor.contains("VerifiedDirectCommonAStatementV1"));
    assert!(!constructor.contains("common_a_statement_digest: [u8; 32]"));
    assert!(!common_a.contains("pub(super) fn new_rkg_round_one_selector_v1("));
    assert!(!common_a.contains("pub(super) struct VerifiedDirectCommonAStatementV1"));
    assert!(!common_a.contains("pub(super) fn statement_digest_for("));
    assert!(!common_a.contains("pub(super) fn derive_verified_direct_common_a_statement_v1("));
    assert!(!common_a.contains("fn mint_rkg_round_one_selector_v1("));
    assert!(!common_a.contains("mint_mismatched_rkg_round_one_selector_for_test_v1"));
    assert!(!active.contains("fn mint_rkg_round_one_selector_v1("));
    assert!(!active.contains("mint_mismatched_rkg_round_one_selector_for_test_v1"));
    assert!(creator.contains("authority: VerifiedDirectCommonAStatementV1"));
    assert!(
        creator
            .contains("derive_verified_direct_common_a_statement_v1(roster, bindings, context)?")
    );
    assert!(creator.contains("completed: CompletedDirectCommonACreatorAuthorityV1"));
    assert!(creator.contains("new_rkg_round_one_selector_v1("));
    assert!(active.contains("direct_common_a_v1::prepare_direct_common_a_creator_h0_v1("));
    assert!(active.contains("direct_common_a_v1::consume_completed_creator_authority_v1("));
    assert!(!active.contains("VerifiedDirectCommonAStatementV1"));
    assert!(!active.contains("derive_verified_direct_common_a_statement_v1("));
    assert!(active.contains("mod direct_common_a_v1;"));
    assert!(!active.contains("pub(super) mod direct_common_a_v1;"));
    assert!(active.contains("#[cfg(test)]\n    pub(super) fn new("));
    assert!(active.contains("let canonical_complete_wire_certified = false;"));
    assert!(active.contains("Err(ZkAmsMkheErrorV1::ReleaseUnavailable)"));
    let statement = include_str!("direct_relation_wire_v1/statement_v1.rs");
    assert!(statement.contains("put(output, 448, &selector.common_a_statement_digest);"));
}
