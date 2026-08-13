use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use super::*;
static DIRECTORY_SEQUENCE_V2: AtomicU64 = AtomicU64::new(0);
static TEST_MODULI_V2: [u64; 2] = [97, 113];
const RELEASE_PARAMETER_KAT_V2: [u8; 32] = [
    0xcc, 0x56, 0x91, 0x18, 0x77, 0xef, 0x83, 0xb0, 0x4c, 0x3c, 0xe8, 0x79, 0x64, 0x0f, 0x29, 0x43,
    0xce, 0xab, 0xe1, 0x3c, 0x38, 0xa7, 0x37, 0x2d, 0x5c, 0x4f, 0x69, 0x63, 0x7f, 0xe7, 0x75, 0x66,
];
const RELEASE_S_MAPPING_KAT_V2: [u8; 32] = [
    0x83, 0x86, 0x35, 0x33, 0x4f, 0xb7, 0xc6, 0x54, 0xe9, 0x41, 0xde, 0xa0, 0x89, 0x04, 0xc2, 0x14,
    0x55, 0x8c, 0xc7, 0x61, 0x53, 0x48, 0xdd, 0x8d, 0x91, 0x4a, 0x33, 0x8f, 0xcf, 0x21, 0xcb, 0xf7,
];
const RELEASE_S_CONTEXT_KAT_V2: [u8; 32] = [
    0x6e, 0x7f, 0x5f, 0xd8, 0xd5, 0x43, 0x94, 0xe9, 0x73, 0x9f, 0x84, 0xe8, 0x02, 0x55, 0x05, 0xbb,
    0x6b, 0x2e, 0xf3, 0xdc, 0x6e, 0xf6, 0xc8, 0x43, 0x05, 0xd8, 0x07, 0xb2, 0x30, 0x99, 0x0a, 0x00,
];
fn manual_release_s_mapping_oracle_v2(mutate_coordinate: bool) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.mask-s-spool.mapping\0");
    hash.update(&[2]);
    hash.update(&RELEASE_PARAMETER_KAT_V2);
    hash.update(&131_072_u32.to_be_bytes());
    hash.update(&[38, 5]);
    hash.update(&1_024_u16.to_be_bytes());
    for value in [128_u64, 24_320, 8_192, 199_618_560] {
        hash.update(&value.to_be_bytes());
    }
    hash.update(b"relation=limb*5+repetition;slot=relation*blocks_per_row+block;first_index=block*values_per_block;S[N-1]=0");
    hash.update(b"canonical big-endian u64 residues;fixed N coefficients;authenticated top zero");
    hash.update(&24_320_u64.to_be_bytes());
    for slot in 0..24_320_u64 {
        let relation = slot / 128;
        let block = slot % 128;
        let limb = u8::try_from(relation / 5).unwrap() ^ u8::from(mutate_coordinate && slot == 128);
        hash.update(&slot.to_be_bytes());
        hash.update(&[limb, u8::try_from(relation % 5).unwrap()]);
        hash.update(&block.to_be_bytes());
        hash.update(&(block * 1_024).to_be_bytes());
    }
    hash.finalize()
}
fn manual_release_s_context_oracle_v2(mapping_digest: [u8; 32], mutate_context: bool) -> [u8; 32] {
    let mut source_context = [0x31; 32];
    source_context[0] ^= u8::from(mutate_context);
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.mask-s-spool.context\0");
    hash.update(&[2]);
    hash.update(&RELEASE_PARAMETER_KAT_V2);
    hash.update(&mapping_digest);
    hash.update(&source_context);
    hash.update(&[0x42; 32]);
    hash.update(b"iroha.zk-ams.v2.phase23.rns-link.q-pcs.mask-sample.external-entropy\0");
    hash.finalize()
}
struct TestDirectoryV2(PathBuf);
impl TestDirectoryV2 {
    fn new_v2() -> Self {
        let sequence = DIRECTORY_SEQUENCE_V2.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "iroha-q-pcs-s-spool-v2-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create isolated S-spool test directory");
        Self(path)
    }
}
impl Drop for TestDirectoryV2 {
    fn drop(&mut self) {
        fs::remove_dir(&self.0).expect("remove empty S-spool test directory");
    }
}
fn geometry_v2() -> SpoolGeometryV2 {
    SpoolGeometryV2 {
        ring_degree: 4,
        domain_log: 4,
        query_count: 4,
        coefficient_values_per_block: 2,
        lde_values_per_block: 2,
        moduli: &TEST_MODULI_V2,
    }
}
fn context_v2() -> PublicSpoolContextV2 {
    PublicSpoolContextV2 {
        sealed_source_transcript_digest: [0x31; 32],
        source_algebra_binding_digest: [0x42; 32],
    }
}
fn mask_v2(relation: u16) -> SecretResiduesV2 {
    let mut mask = SecretResiduesV2::new_zeroed_exact_v2(3).unwrap();
    for (index, value) in mask.as_mut_slice_v2().iter_mut().enumerate() {
        *value = 1 + u64::from(relation) * 4 + index as u64;
    }
    mask
}
fn sealed_v2(directory: &TestDirectoryV2) -> MaskSpoolSealedV2 {
    let geometry = geometry_v2();
    let mut writer = MaskSpoolWriterV2::create_v2(
        &directory.0,
        geometry,
        parameter_digest_v2(geometry).unwrap(),
        context_v2(),
    )
    .unwrap();
    for relation in 0..10_u16 {
        writer
            .push_next_mask_v2(
                u8::try_from(relation / 5).unwrap(),
                u8::try_from(relation % 5).unwrap(),
                &mask_v2(relation),
            )
            .unwrap();
    }
    writer.seal_v2().unwrap()
}
#[test]
fn tiny_spool_mapping_and_authenticated_replay_are_exact() {
    let directory = TestDirectoryV2::new_v2();
    let sealed = sealed_v2(&directory);
    assert_ne!(sealed.snapshot_digest_v2().unwrap(), [0; 32]);
    assert_eq!(sealed.descriptor.relations, 10);
    assert_eq!(sealed.descriptor.blocks_per_row, 2);
    assert_eq!(sealed.descriptor.slot_count, 20);
    assert_eq!(sealed.descriptor.plaintext_bytes, 16);
    assert_eq!(sealed.descriptor.file_bytes, 640);
    assert_eq!(
        sealed.parameter_digest,
        parameter_digest_v2(geometry_v2()).unwrap()
    );
    let mut replay = sealed.begin_replay_v2().unwrap();
    for slot in 0..20_u64 {
        let relation = u16::try_from(slot / 2).unwrap();
        let block = usize::try_from(slot % 2).unwrap();
        let chunk = replay.read_next_block_v2().unwrap();
        let values: Vec<u64> = chunk
            .bytes_v2()
            .chunks_exact(8)
            .map(|encoded| u64::from_be_bytes(encoded.try_into().unwrap()))
            .collect();
        let base = 1 + u64::from(relation) * 4;
        let expected = if block == 0 {
            [base, base + 1]
        } else {
            [base + 2, 0]
        };
        assert_eq!(values, expected);
    }
    let sealed = replay.complete_v2().unwrap();
    assert_eq!(sealed.descriptor.slot_count, 20);
}
#[test]
fn purpose_order_failure_poison_and_incomplete_seal_fail_closed() {
    let directory = TestDirectoryV2::new_v2();
    let geometry = geometry_v2();
    let parameter = parameter_digest_v2(geometry).unwrap();
    let mut writer =
        MaskSpoolWriterV2::create_v2(&directory.0, geometry, parameter, context_v2()).unwrap();
    assert!(matches!(
        writer.push_next_mask_v2(0, 1, &mask_v2(1)),
        Err(ProverPrerequisiteErrorV2::InvalidRelationOrder)
    ));
    assert!(matches!(
        writer.push_next_mask_v2(0, 0, &mask_v2(0)),
        Err(ProverPrerequisiteErrorV2::Poisoned)
    ));
    let mut incomplete =
        MaskSpoolWriterV2::create_v2(&directory.0, geometry, parameter, context_v2()).unwrap();
    incomplete.push_next_mask_v2(0, 0, &mask_v2(0)).unwrap();
    assert!(matches!(
        incomplete.seal_v2(),
        Err(ProverPrerequisiteErrorV2::MissingRelations)
    ));
    let mut full =
        MaskSpoolWriterV2::create_v2(&directory.0, geometry, parameter, context_v2()).unwrap();
    for relation in 0..10_u16 {
        full.push_next_mask_v2(
            u8::try_from(relation / 5).unwrap(),
            u8::try_from(relation % 5).unwrap(),
            &mask_v2(relation),
        )
        .unwrap();
    }
    assert!(matches!(
        full.push_next_mask_v2(2, 0, &mask_v2(10)),
        Err(ProverPrerequisiteErrorV2::InvalidRelationOrder)
    ));
    assert!(matches!(
        full.seal_v2(),
        Err(ProverPrerequisiteErrorV2::Poisoned)
    ));
}
#[test]
fn context_mutation_extra_read_and_unwind_destroy_replay_authority() {
    let directory = TestDirectoryV2::new_v2();
    let mut sealed = sealed_v2(&directory);
    sealed.context_digest[0] ^= 1;
    assert!(sealed.snapshot_digest_v2().is_err());
    assert!(matches!(
        sealed.begin_replay_v2(),
        Err(ProverPrerequisiteErrorV2::InvalidC0Context)
    ));
    let mut hostile_descriptor = sealed_v2(&directory);
    hostile_descriptor.descriptor.slot_count += 1;
    assert!(hostile_descriptor.snapshot_digest_v2().is_err());
    let mut hostile_parameter = sealed_v2(&directory);
    hostile_parameter.parameter_digest[0] ^= 1;
    assert!(hostile_parameter.snapshot_digest_v2().is_err());
    let mut complete = sealed_v2(&directory).begin_replay_v2().unwrap();
    for _ in 0..20 {
        drop(complete.read_next_block_v2().unwrap());
    }
    assert!(complete.read_next_block_v2().is_err());
    assert!(matches!(
        complete.complete_v2(),
        Err(ProverPrerequisiteErrorV2::Poisoned)
    ));
    let geometry = geometry_v2();
    let parameter = parameter_digest_v2(geometry).unwrap();
    let descriptor = mask_spool_descriptor_v2(geometry, parameter).unwrap();
    let context_digest = mask_spool_context_v2(parameter, descriptor, context_v2()).unwrap();
    let layout = ConfidentialSpoolLayoutV1::new_v1(
        descriptor.slot_count,
        descriptor.plaintext_bytes,
        context_digest,
    )
    .unwrap();
    let mut writer = ConfidentialSpoolWriterV1::create_in_v1(&directory.0, layout).unwrap();
    for slot in 0..descriptor.slot_count {
        let mut chunk =
            ConfidentialSpoolChunkV1::new_zeroed_v1(descriptor.plaintext_bytes).unwrap();
        if slot + 1 == descriptor.slot_count {
            chunk.as_mut_slice_v1()[8..].copy_from_slice(&1_u64.to_be_bytes());
        }
        writer.write_slot_v1(slot, chunk).unwrap();
    }
    let mut nonzero_top = MaskSpoolSealedV2 {
        snapshot: Some(writer.seal_v1().unwrap()),
        geometry,
        parameter_digest: parameter,
        descriptor,
        context: context_v2(),
        context_digest,
    }
    .begin_replay_v2()
    .unwrap();
    for _ in 1..descriptor.slot_count {
        drop(nonzero_top.read_next_block_v2().unwrap());
    }
    assert!(matches!(
        nonzero_top.read_next_block_v2(),
        Err(ProverPrerequisiteErrorV2::NonCanonicalResidue)
    ));
    let before = MASK_REPLAY_CHUNK_ZEROIZED_DROPS_V2.load(Ordering::SeqCst);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut replay = sealed_v2(&directory).begin_replay_v2().unwrap();
        let _chunk = replay.read_next_block_v2().unwrap();
        panic!("exercise unwind zeroization");
    }));
    assert!(result.is_err());
    assert_eq!(
        MASK_REPLAY_CHUNK_ZEROIZED_DROPS_V2.load(Ordering::SeqCst),
        before + 1
    );
}
#[test]
fn release_geometry_context_binding_and_source_guards_are_pinned() {
    assert_eq!(RELEASE_MASK_S_SLOTS_V2, 24_320);
    assert_eq!(RELEASE_MASK_S_FILE_BYTES_V2, 199_618_560);
    assert_eq!(RELEASE_MASK_S_SECRET_VALUES_V2, 24_903_490);
    assert_eq!(RELEASE_MASK_S_STORED_VALUES_V2, 24_903_680);
    let release = SpoolGeometryV2::release_v2();
    assert_eq!(
        parameter_digest_v2(release).unwrap(),
        RELEASE_PARAMETER_KAT_V2
    );
    let release_descriptor = mask_spool_descriptor_v2(release, RELEASE_PARAMETER_KAT_V2).unwrap();
    assert_eq!(
        manual_release_s_mapping_oracle_v2(false),
        RELEASE_S_MAPPING_KAT_V2
    );
    assert_eq!(release_descriptor.mapping_digest, RELEASE_S_MAPPING_KAT_V2);
    assert_ne!(
        manual_release_s_mapping_oracle_v2(true),
        RELEASE_S_MAPPING_KAT_V2
    );
    assert_eq!(
        manual_release_s_context_oracle_v2(RELEASE_S_MAPPING_KAT_V2, false),
        RELEASE_S_CONTEXT_KAT_V2
    );
    assert_eq!(
        mask_spool_context_v2(RELEASE_PARAMETER_KAT_V2, release_descriptor, context_v2()).unwrap(),
        RELEASE_S_CONTEXT_KAT_V2
    );
    assert_ne!(
        manual_release_s_context_oracle_v2(RELEASE_S_MAPPING_KAT_V2, true),
        RELEASE_S_CONTEXT_KAT_V2
    );
    let mut hostile_mapping = RELEASE_S_MAPPING_KAT_V2;
    hostile_mapping[0] ^= 1;
    assert_ne!(
        manual_release_s_context_oracle_v2(hostile_mapping, false),
        RELEASE_S_CONTEXT_KAT_V2
    );
    let geometry = geometry_v2();
    let parameter = parameter_digest_v2(geometry).unwrap();
    let descriptor = mask_spool_descriptor_v2(geometry, parameter).unwrap();
    let first = mask_spool_context_v2(parameter, descriptor, context_v2()).unwrap();
    let mut changed = context_v2();
    changed.source_algebra_binding_digest[31] ^= 1;
    assert_ne!(
        first,
        mask_spool_context_v2(parameter, descriptor, changed).unwrap()
    );
    let source = include_str!("s_spool_v2.rs");
    let tests = include_str!("s_spool_v2_tests.rs");
    assert!(source.lines().count() <= 500);
    assert!(tests.lines().count() <= 425);
    for required in [
        "RELEASE_MASK_S_SLOTS_V2: u64 = 24_320",
        "RELEASE_MASK_S_FILE_BYTES_V2: u64 = 199_618_560",
        "RELEASE_MASK_S_WRITE_BYTES_V2: u64 = RELEASE_MASK_S_FILE_BYTES_V2",
        "RELEASE_MASK_S_SEAL_READ_BYTES_V2: u64 = RELEASE_MASK_S_FILE_BYTES_V2",
        "RELEASE_MASK_S_TOTAL_IO_BYTES_V2: u64 = 399_237_120",
        ".live\n            .take()",
        "S[N-1]=0",
        "MASK_S_REPLAY_BOUND_V2: bool = true",
        "pub(super) fn snapshot_digest_v2(",
        "CROSS_FIELD_MASK_PROOF_COMPLETE_V2: bool = false",
    ] {
        assert!(source.contains(required), "missing S-spool pin: {required}");
    }
    for forbidden in ["pub struct", "pub enum", "pub fn", "pub(crate)", "pub use"] {
        assert!(
            !source.contains(forbidden),
            "forbidden S-spool surface: {forbidden}"
        );
    }
}
