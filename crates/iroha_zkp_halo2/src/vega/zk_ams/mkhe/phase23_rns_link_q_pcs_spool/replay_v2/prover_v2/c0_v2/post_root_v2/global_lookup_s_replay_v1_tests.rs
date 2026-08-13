use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static DIRECTORY_SEQUENCE_V1: AtomicU64 = AtomicU64::new(0);
static TEST_MODULI_V1: [u64; 2] = [97, 113];
const RELEASE_PARAMETER_KAT_V1: [u8; 32] = [
    0xcc, 0x56, 0x91, 0x18, 0x77, 0xef, 0x83, 0xb0, 0x4c, 0x3c, 0xe8, 0x79, 0x64, 0x0f, 0x29, 0x43,
    0xce, 0xab, 0xe1, 0x3c, 0x38, 0xa7, 0x37, 0x2d, 0x5c, 0x4f, 0x69, 0x63, 0x7f, 0xe7, 0x75, 0x66,
];
const RELEASE_MAPPING_KAT_V1: [u8; 32] = [
    0xdf, 0xc0, 0xee, 0x02, 0x32, 0xe6, 0xd6, 0xe1, 0x1c, 0xd8, 0xf6, 0x75, 0x35, 0x0d, 0x38, 0x84,
    0x25, 0xdd, 0xea, 0xb1, 0xf4, 0xf5, 0xc2, 0xbc, 0x18, 0x7d, 0xab, 0x7e, 0xb1, 0xe5, 0x30, 0xcc,
];
const RELEASE_BINDING_KAT_V1: [u8; 32] = [
    0x74, 0x06, 0xf3, 0xe6, 0xaf, 0xff, 0xf0, 0xfb, 0x56, 0xe1, 0x28, 0x7e, 0x32, 0x53, 0x29, 0x0b,
    0x37, 0x3e, 0x81, 0xc3, 0xe2, 0xf0, 0x3a, 0x39, 0x41, 0x1a, 0xf6, 0xb0, 0xa3, 0xfd, 0x7c, 0x06,
];
const TINY_MAPPING_KAT_V1: [u8; 32] = [
    0x23, 0x81, 0x25, 0x14, 0x80, 0x67, 0x0d, 0x92, 0xcb, 0xd8, 0x99, 0xc3, 0x6f, 0xc2, 0x92, 0xda,
    0x6d, 0x04, 0x1b, 0x1a, 0x0b, 0x96, 0xc2, 0x78, 0xae, 0x27, 0x03, 0xe4, 0xb3, 0x4c, 0xbe, 0x19,
];
const TINY_PARAMETER_KAT_V1: [u8; 32] = [
    0xf2, 0x5e, 0x8f, 0xc8, 0xba, 0x2f, 0x66, 0xe7, 0x19, 0x9d, 0x5a, 0x19, 0x1a, 0x36, 0xf3, 0x7a,
    0x91, 0xae, 0x9e, 0xd0, 0x73, 0xf3, 0x53, 0x68, 0xdb, 0x09, 0xfe, 0xbf, 0xb3, 0x3b, 0xcd, 0xb3,
];
const TINY_BINDING_KAT_V1: [u8; 32] = [
    0x9d, 0xfd, 0x76, 0xc0, 0x3f, 0x6e, 0x5e, 0x73, 0x7a, 0x6a, 0x66, 0xd0, 0x81, 0x6c, 0x41, 0x90,
    0x9d, 0x13, 0x77, 0xc6, 0x8a, 0x19, 0x42, 0xb6, 0x3b, 0x84, 0xed, 0xf7, 0xa5, 0x99, 0x16, 0xcf,
];
fn manual_mapping_oracle_v1(
    limbs: u64,
    blocks_per_relation: u64,
    coefficients_per_block: u64,
    slot_count: u64,
    tuple_count: u64,
    mutate_coordinate: bool,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.vega.mkhe.global_lookup.q_pcs_s_replay.mapping.v1");
    for value in [5_u64, 16, 4, 15, slot_count, tuple_count] {
        hash.update(&value.to_be_bytes());
    }
    for slot in 0..slot_count {
        let relation = slot / blocks_per_relation;
        let block = slot % blocks_per_relation;
        let group = block / 16 + u64::from(u8::from(mutate_coordinate && slot == 16));
        hash.update(&slot.to_be_bytes());
        hash.update(&u32::try_from(relation / 5).unwrap().to_be_bytes());
        hash.update(&u32::try_from(relation % 5).unwrap().to_be_bytes());
        hash.update(&u32::try_from(group).unwrap().to_be_bytes());
        hash.update(&u32::try_from(block % 16).unwrap().to_be_bytes());
        hash.update(&(block * coefficients_per_block).to_be_bytes());
    }
    assert_eq!(slot_count, limbs * 5 * blocks_per_relation);
    hash.finalize()
}
fn manual_binding_oracle_v1(
    parameter_digest: [u8; 32],
    mapping_digest: [u8; 32],
    slot_count: u64,
    tuple_count: u64,
    mutate_quotient_root: bool,
) -> [u8; 32] {
    let mut quotient_root = [0x64; 32];
    quotient_root[0] ^= u8::from(mutate_quotient_root);
    let mut hash = Keccak256::new();
    hash.update(b"iroha.vega.mkhe.global_lookup.q_pcs_s_replay.binding.v1");
    hash.update(b"iroha.vega.mkhe.global_lookup.q_pcs_s_replay.v1");
    hash.update(&[2]);
    hash.update(&parameter_digest);
    hash.update(&[0x31; 32]);
    hash.update(&[0x42; 32]);
    hash.update(&[0x75; 32]);
    hash.update(&[0x53; 32]);
    hash.update(&quotient_root);
    hash.update(&GLOBAL_LOOKUP_TOPOLOGY_DIGEST_V1);
    hash.update(&mapping_digest);
    hash.update(&slot_count.to_be_bytes());
    hash.update(&tuple_count.to_be_bytes());
    hash.finalize()
}
struct TestDirectoryV1(PathBuf);
impl TestDirectoryV1 {
    fn new_v1() -> Self {
        let sequence = DIRECTORY_SEQUENCE_V1.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "iroha-global-lookup-s-replay-v1-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated global-lookup S test directory");
        Self(path)
    }
}
impl Drop for TestDirectoryV1 {
    fn drop(&mut self) {
        fs::remove_dir(&self.0).expect("terminal replay removed its confidential snapshot");
    }
}
fn geometry_v1() -> SpoolGeometryV2 {
    SpoolGeometryV2 {
        ring_degree: 4,
        domain_log: 4,
        query_count: 4,
        coefficient_values_per_block: 2,
        lde_values_per_block: 2,
        moduli: &TEST_MODULI_V1,
    }
}
fn context_v1() -> PublicSpoolContextV2 {
    PublicSpoolContextV2 {
        sealed_source_transcript_digest: [0x31; 32],
        source_algebra_binding_digest: [0x42; 32],
    }
}
fn axes_v1() -> GlobalLookupSReplayAxesV1 {
    let context = context_v1();
    GlobalLookupSReplayAxesV1 {
        parameter_digest: parameter_digest_v2(geometry_v1()).unwrap(),
        sealed_source_transcript_digest: context.sealed_source_transcript_digest,
        source_algebra_binding_digest: context.source_algebra_binding_digest,
        initial_root: [0x53; 32],
        quotient_root: [0x64; 32],
        topology_digest: GLOBAL_LOOKUP_TOPOLOGY_DIGEST_V1,
    }
}
fn sealed_v1(directory: &TestDirectoryV1) -> MaskSpoolSealedV2 {
    let geometry = geometry_v1();
    let mut writer = MaskSpoolWriterV2::create_v2(
        &directory.0,
        geometry,
        parameter_digest_v2(geometry).unwrap(),
        context_v1(),
    )
    .unwrap();
    for relation in 0..10_u16 {
        let mut mask = SecretResiduesV2::new_zeroed_exact_v2(3).unwrap();
        for (index, value) in mask.as_mut_slice_v2().iter_mut().enumerate() {
            *value = 1 + u64::from(relation) * 4 + index as u64;
        }
        writer
            .push_next_mask_v2(
                u8::try_from(relation / 5).unwrap(),
                u8::try_from(relation % 5).unwrap(),
                &mask,
            )
            .unwrap();
    }
    writer.seal_v2().unwrap()
}
struct CountingSinkV1 {
    begins: u64,
    tuples: u64,
    finishes: u64,
    binding_digest: [u8; 32],
}
impl CountingSinkV1 {
    fn new_v1() -> Self {
        Self {
            begins: 0,
            tuples: 0,
            finishes: 0,
            binding_digest: [0; 32],
        }
    }
}
impl GlobalLookupSTupleSinkV1 for CountingSinkV1 {
    fn begin_v1(
        &mut self,
        binding: &GlobalLookupSReplayBindingV1,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        assert_eq!(binding.slot_count, 20);
        assert_eq!(binding.tuple_count, 40);
        assert_ne!(binding.snapshot_digest, [0; 32]);
        assert_ne!(binding.mapping_digest, [0; 32]);
        assert_ne!(binding.digest, [0; 32]);
        self.begins += 1;
        self.binding_digest = binding.digest;
        Ok(())
    }
    fn absorb_next_v1(
        &mut self,
        digits: [u16; 4],
        complement_digits: [u16; 4],
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        assert!(digits.into_iter().all(|digit| digit < 1 << 15));
        assert!(complement_digits.into_iter().all(|digit| digit < 1 << 15));
        self.tuples += 1;
        Ok(())
    }
    fn finish_v1(
        &mut self,
        binding: &GlobalLookupSReplayBindingV1,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        assert_eq!(binding.digest, self.binding_digest);
        assert_eq!(self.tuples, binding.tuple_count);
        self.finishes += 1;
        Ok(())
    }
}
struct FailingSinkV1 {
    fail_at: u64,
    tuples: u64,
    panic: bool,
}
impl GlobalLookupSTupleSinkV1 for FailingSinkV1 {
    fn begin_v1(
        &mut self,
        _binding: &GlobalLookupSReplayBindingV1,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        Ok(())
    }
    fn absorb_next_v1(
        &mut self,
        _digits: [u16; 4],
        _complement_digits: [u16; 4],
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        self.tuples += 1;
        if self.tuples == self.fail_at {
            if self.panic {
                panic!("exercise terminal sink unwind");
            }
            return Err(ProverPrerequisiteErrorV2::InvalidRelation);
        }
        Ok(())
    }
    fn finish_v1(
        &mut self,
        _binding: &GlobalLookupSReplayBindingV1,
    ) -> Result<(), ProverPrerequisiteErrorV2> {
        Ok(())
    }
}
#[test]
fn authenticated_replay_returns_owner_only_after_exact_sink_completion() {
    let directory = TestDirectoryV1::new_v1();
    let mut sink = CountingSinkV1::new_v1();
    let sealed = replay_mask_v1(
        sealed_v1(&directory),
        geometry_v1(),
        axes_v1(),
        &mut sink,
        false,
    )
    .unwrap();
    assert_eq!((sink.begins, sink.tuples, sink.finishes), (1, 40, 1));
    let mut replay = sealed.begin_replay_v2().unwrap();
    for _ in 0..20 {
        drop(replay.read_next_block_v2().unwrap());
    }
    drop(replay.complete_v2().unwrap());
}
#[test]
fn sink_error_and_unwind_terminally_drop_the_taken_snapshot() {
    let directory = TestDirectoryV1::new_v1();
    let mut failing = FailingSinkV1 {
        fail_at: 3,
        tuples: 0,
        panic: false,
    };
    assert!(matches!(
        replay_mask_v1(
            sealed_v1(&directory),
            geometry_v1(),
            axes_v1(),
            &mut failing,
            false
        ),
        Err(ProverPrerequisiteErrorV2::InvalidRelation)
    ));
    let mut panicking = FailingSinkV1 {
        fail_at: 2,
        tuples: 0,
        panic: true,
    };
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = replay_mask_v1(
                sealed_v1(&directory),
                geometry_v1(),
                axes_v1(),
                &mut panicking,
                false,
            );
        }))
        .is_err()
    );
}
#[test]
fn coordinate_order_completion_and_radix_complement_are_exact() {
    let geometry = SpoolGeometryV2::release_v2();
    let slots = slot_count_v1(geometry).unwrap();
    assert_eq!(slots, 24_320);
    let first = coordinate_v1(geometry, slots, 0).unwrap();
    assert_eq!(
        (
            first.limb,
            first.repetition,
            first.group,
            first.block_in_group
        ),
        (0, 0, 0, 0)
    );
    let boundary = coordinate_v1(geometry, slots, 127).unwrap();
    assert_eq!(
        (
            boundary.limb,
            boundary.repetition,
            boundary.group,
            boundary.block_in_group,
            boundary.first_coefficient,
        ),
        (0, 0, 7, 15, 130_048)
    );
    let next = coordinate_v1(geometry, slots, 128).unwrap();
    assert_eq!((next.limb, next.repetition, next.group), (0, 1, 0));
    let last = coordinate_v1(geometry, slots, slots - 1).unwrap();
    assert_eq!((last.limb, last.repetition, last.group), (37, 4, 7));
    assert!(coordinate_v1(geometry, slots, slots).is_err());
    let mut reordered = coordinate_v1(geometry, slots, 1).unwrap();
    reordered.group = 1;
    assert!(matches!(
        require_coordinate_v1(geometry, 1, &reordered),
        Err(ProverPrerequisiteErrorV2::InvalidRelationOrder)
    ));
    assert!(require_completion_v1(24_320, 24_903_680, 24_319, 24_903_680).is_err());
    assert!(require_completion_v1(24_320, 24_903_680, 24_321, 24_903_680).is_err());
    let modulus = RELEASE_MODULI_V1[0];
    for value in [0, 1, 32_767, 32_768, modulus - 1] {
        let (digits, complement) = radix_tuple_v1(value, modulus).unwrap();
        let recover = |values: [u16; 4]| {
            values
                .into_iter()
                .enumerate()
                .fold(0_u64, |sum, (index, digit)| {
                    sum + u64::from(digit) * (1_u64 << (15 * index))
                })
        };
        assert_eq!(recover(digits), value);
        assert_eq!(recover(complement), modulus - 1 - value);
    }
    assert!(radix_tuple_v1(modulus, modulus).is_err());
}
#[test]
fn release_accounting_topology_binding_mutations_and_source_guards_are_pinned() {
    let geometry = SpoolGeometryV2::release_v2();
    require_release_accounting_v1(
        geometry,
        GLOBAL_LOOKUP_S_RELEASE_SLOTS_V1,
        GLOBAL_LOOKUP_S_RELEASE_TUPLES_V1,
    )
    .unwrap();
    assert_eq!(
        crate::vega::zk_ams::mkhe::global_lookup_statement_v1::global_lookup_topology_digest_v1(),
        GLOBAL_LOOKUP_TOPOLOGY_DIGEST_V1
    );
    assert_eq!(GLOBAL_LOOKUP_S_RELEASE_TOTAL_IO_BYTES_V1, 598_855_680);
    assert_eq!(GLOBAL_LOOKUP_S_RELEASE_TRANSIENT_HEAP_BYTES_V1, 8_192);
    assert_eq!(
        parameter_digest_v2(geometry).unwrap(),
        RELEASE_PARAMETER_KAT_V1
    );
    let manual_release_mapping =
        manual_mapping_oracle_v1(38, 128, 1_024, 24_320, 24_903_680, false);
    assert_eq!(manual_release_mapping, RELEASE_MAPPING_KAT_V1);
    assert_eq!(
        mapping_digest_v1(geometry, 24_320).unwrap(),
        RELEASE_MAPPING_KAT_V1
    );
    assert_ne!(
        manual_mapping_oracle_v1(38, 128, 1_024, 24_320, 24_903_680, true),
        RELEASE_MAPPING_KAT_V1
    );
    assert_ne!(
        manual_mapping_oracle_v1(38, 128, 1_024, 24_320, 24_903_681, false),
        RELEASE_MAPPING_KAT_V1
    );
    let release_axes = GlobalLookupSReplayAxesV1 {
        parameter_digest: RELEASE_PARAMETER_KAT_V1,
        sealed_source_transcript_digest: [0x31; 32],
        source_algebra_binding_digest: [0x42; 32],
        initial_root: [0x53; 32],
        quotient_root: [0x64; 32],
        topology_digest: GLOBAL_LOOKUP_TOPOLOGY_DIGEST_V1,
    };
    assert_eq!(
        manual_binding_oracle_v1(
            RELEASE_PARAMETER_KAT_V1,
            RELEASE_MAPPING_KAT_V1,
            24_320,
            24_903_680,
            false,
        ),
        RELEASE_BINDING_KAT_V1
    );
    assert_eq!(
        binding_v1(
            &release_axes,
            [0x75; 32],
            RELEASE_MAPPING_KAT_V1,
            24_320,
            24_903_680,
        )
        .digest,
        RELEASE_BINDING_KAT_V1
    );
    assert_ne!(
        manual_binding_oracle_v1(
            RELEASE_PARAMETER_KAT_V1,
            RELEASE_MAPPING_KAT_V1,
            24_320,
            24_903_680,
            true,
        ),
        RELEASE_BINDING_KAT_V1
    );
    let mapping = mapping_digest_v1(geometry_v1(), 20).unwrap();
    let baseline = binding_v1(&axes_v1(), [0x75; 32], mapping, 20, 40).digest;
    assert_eq!(
        parameter_digest_v2(geometry_v1()).unwrap(),
        TINY_PARAMETER_KAT_V1
    );
    assert_eq!(
        manual_mapping_oracle_v1(2, 2, 2, 20, 40, false),
        TINY_MAPPING_KAT_V1
    );
    assert_eq!(mapping, TINY_MAPPING_KAT_V1);
    assert_eq!(
        manual_binding_oracle_v1(TINY_PARAMETER_KAT_V1, TINY_MAPPING_KAT_V1, 20, 40, false),
        TINY_BINDING_KAT_V1
    );
    assert_eq!(baseline, TINY_BINDING_KAT_V1);
    let mut changed = axes_v1();
    changed.quotient_root[0] ^= 1;
    assert_ne!(
        baseline,
        binding_v1(&changed, [0x75; 32], mapping, 20, 40).digest
    );
    changed = axes_v1();
    changed.source_algebra_binding_digest[0] ^= 1;
    assert_ne!(
        baseline,
        binding_v1(&changed, [0x75; 32], mapping, 20, 40).digest
    );
    let mut snapshot = [0x75; 32];
    snapshot[0] ^= 1;
    assert_ne!(
        baseline,
        binding_v1(&axes_v1(), snapshot, mapping, 20, 40).digest
    );
    let source = include_str!("global_lookup_s_replay_v1.rs");
    let tests = include_str!("global_lookup_s_replay_v1_tests.rs");
    let parent = include_str!("../post_root_v2.rs");
    assert!(source.lines().count() <= 500);
    assert!(tests.lines().count() <= 550);
    assert_eq!(parent.matches("mod global_lookup_s_replay_v1;").count(), 1);
    for required in [
        "GLOBAL_LOOKUP_S_RELEASE_SLOTS_V1 != 24_320",
        "GLOBAL_LOOKUP_S_RELEASE_TUPLES_V1 != 24_903_680",
        "GLOBAL_LOOKUP_S_RELEASE_Q_MASK_BLOCKS_V1 != 1_520",
        "GLOBAL_LOOKUP_S_RELEASE_REPLAY_FILE_BYTES_V1 != 199_618_560",
        "snapshot_digest_v2()?",
        "global_lookup_topology_digest_v1()",
        "match _upstream_lookup_plane_seal {}",
        "GLOBAL_LOOKUP_S_PROOF_COMPLETE_V1: bool = false",
        "GLOBAL_LOOKUP_S_ZERO_KNOWLEDGE_BOUND_V1: bool = false",
        "GLOBAL_LOOKUP_S_OPERATIONAL_RECEIPT_ACCEPTED_V1: bool = false",
        "GLOBAL_LOOKUP_S_MEASURED_RSS_WITHIN_CAP_V1: bool = false",
        "GLOBAL_LOOKUP_S_RELEASE_READY_V1: bool = false",
        "GLOBAL_LOOKUP_S_RELEASE_COMPLETE_V1: bool = false",
    ] {
        assert!(
            source.contains(required),
            "missing S-replay pin: {required}"
        );
    }
    for forbidden in [
        "Vec<",
        "pub struct",
        "pub enum",
        "pub fn",
        "pub(crate)",
        "derive(Clone",
        "derive(Debug",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden S-replay surface: {forbidden}"
        );
    }
}
