use super::*;
use crate::vega::sponge::Keccak256;

const FRI1_MAPPING_KAT_V2: [u8; 32] = [
    0x24, 0x05, 0xd2, 0xa0, 0x00, 0x31, 0x8a, 0x0b, 0xaa, 0x74, 0xda, 0x72, 0xdc, 0x49, 0x53, 0x0a,
    0x7d, 0x35, 0xf4, 0xcb, 0x69, 0x06, 0xe2, 0xd5, 0x20, 0x62, 0xb7, 0xf5, 0xdd, 0x8e, 0x77, 0x86,
];
const FRI1_CONTEXT_KAT_V2: [u8; 32] = [
    0x11, 0x3f, 0x42, 0x11, 0x0a, 0xe7, 0xf7, 0x51, 0x22, 0xc1, 0x4c, 0x9b, 0x67, 0x78, 0x51, 0x06,
    0x55, 0x6e, 0x37, 0xe7, 0xf5, 0x17, 0x45, 0x02, 0x50, 0xe6, 0xd9, 0x31, 0x96, 0xd0, 0x29, 0x59,
];
const FRI1_LEAF_KAT_V2: [u8; 32] = [
    0x0a, 0x13, 0x0f, 0x87, 0x91, 0xbe, 0x9c, 0x09, 0xdd, 0x44, 0x9e, 0x49, 0xed, 0x3e, 0xad, 0xd5,
    0x68, 0x5f, 0xb1, 0x6b, 0x3b, 0x68, 0x8e, 0x1f, 0xaa, 0x6c, 0xb4, 0x87, 0xa5, 0x69, 0xe2, 0xf1,
];
const FRI1_NODE_KAT_V2: [u8; 32] = [
    0x36, 0x12, 0x1d, 0x90, 0xcd, 0x71, 0x0f, 0xa3, 0xde, 0x7d, 0x05, 0x88, 0xd0, 0xe6, 0xf0, 0x65,
    0x39, 0x67, 0xaa, 0x23, 0xdd, 0x23, 0x10, 0x00, 0xb9, 0xa0, 0x50, 0x36, 0x21, 0x5d, 0x85, 0x5d,
];

fn binding_v2() -> FriLayer0BindingV2 {
    FriLayer0BindingV2::new_v2(
        parameter_digest_v2(SpoolGeometryV2::release_v2()).unwrap(),
        PublicSpoolContextV2 {
            sealed_source_transcript_digest: [0x22; 32],
            source_algebra_binding_digest: [0x33; 32],
        },
        [0x44; 32],
        [0x55; 32],
        [0x66; 32],
        [0x77; 32],
    )
    .unwrap()
}

fn layer1_binding_v2() -> FriLayer1BindingV2 {
    FriLayer1BindingV2::new_v2(
        binding_v2(),
        [0x88; 32],
        [0x66; 32],
        [0x99; 32],
        [0x77; 32],
        [0xaa; 32],
    )
    .unwrap()
}

fn manual_leaf_v2(parameter: [u8; 32], values: &[u8]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-leaf\0");
    hash.update(&[2]);
    hash.update(&parameter);
    hash.update(&[3, 0]);
    hash.update(&524_288_u32.to_be_bytes());
    hash.update(&380_u16.to_be_bytes());
    hash.update(values);
    hash.finalize()
}

fn manual_node_v2(parameter: [u8; 32], height: usize, left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-node\0");
    hash.update(&[2]);
    hash.update(&parameter);
    hash.update(&[3, 0, u8::try_from(height).unwrap()]);
    hash.update(&left);
    hash.update(&right);
    hash.finalize()
}

fn manual_layer1_leaf_v2(parameter: [u8; 32], values: &[u8]) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-leaf\0");
    hash.update(&[2]);
    hash.update(&parameter);
    hash.update(&[3, 1]);
    hash.update(&262_144_u32.to_be_bytes());
    hash.update(&380_u16.to_be_bytes());
    hash.update(values);
    hash.finalize()
}

fn manual_layer1_node_v2(
    parameter: [u8; 32],
    height: usize,
    left: [u8; 32],
    right: [u8; 32],
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(b"iroha.zk-ams.v2.q-pcs.ten-row-merkle-node\0");
    hash.update(&[2]);
    hash.update(&parameter);
    hash.update(&[3, 1, u8::try_from(height).unwrap()]);
    hash.update(&left);
    hash.update(&right);
    hash.finalize()
}

#[test]
fn layer0_context_binds_every_public_axis_and_exact_schedule_prefix() {
    let binding = binding_v2();
    let descriptor = fri_layer_layout_v2(binding.parameter_digest, 0).unwrap();
    let baseline = fri0_context_digest_v2(descriptor, binding).unwrap();
    assert!(
        FriLayer0BindingV2::new_v2(
            [0x11; 32],
            binding.context,
            binding.initial_root,
            binding.quotient_root,
            binding.pre_layer_transcript,
            binding.batch_schedule_digest,
        )
        .is_err()
    );
    let mut changed = binding_v2();
    changed.initial_root[0] ^= 1;
    assert_ne!(
        baseline,
        fri0_context_digest_v2(descriptor, changed).unwrap()
    );
    let mut changed = binding_v2();
    changed.quotient_root[0] ^= 1;
    assert_ne!(
        baseline,
        fri0_context_digest_v2(descriptor, changed).unwrap()
    );
    let mut changed = binding_v2();
    changed.pre_layer_transcript[0] ^= 1;
    assert_ne!(
        baseline,
        fri0_context_digest_v2(descriptor, changed).unwrap()
    );
    let mut changed = binding_v2();
    changed.batch_schedule_digest[0] ^= 1;
    assert_ne!(
        baseline,
        fri0_context_digest_v2(descriptor, changed).unwrap()
    );
}

#[test]
fn layer0_leaf_and_node_frames_match_independent_verifier_literal_oracle() {
    let parameter = [0x11; 32];
    let values = [0x42_u8; 6_080];
    assert_eq!(
        fri_leaf_hash_v2(parameter, &values).unwrap(),
        manual_leaf_v2(parameter, &values)
    );
    assert_eq!(
        fri_node_hash_v2(parameter, 7, [0x31; 32], [0x52; 32]).unwrap(),
        manual_node_v2(parameter, 7, [0x31; 32], [0x52; 32])
    );
}

#[test]
fn layer1_context_is_literal_and_binds_b0_root_transcript_and_schedules() {
    let layer0 = binding_v2();
    let descriptor = fri_layer_layout_v2(layer0.parameter_digest, 1).unwrap();
    assert_eq!(descriptor.mapping_digest, FRI1_MAPPING_KAT_V2);
    let binding = layer1_binding_v2();
    assert_eq!(
        fri1_context_digest_v2(descriptor, binding).unwrap(),
        FRI1_CONTEXT_KAT_V2
    );
    assert!(
        FriLayer1BindingV2::new_v2(
            layer0, [0x88; 32], [0x67; 32], [0x99; 32], [0x77; 32], [0xaa; 32],
        )
        .is_err()
    );
    assert!(
        FriLayer1BindingV2::new_v2(
            layer0, [0x88; 32], [0x66; 32], [0x99; 32], [0x76; 32], [0xaa; 32],
        )
        .is_err()
    );
    for (root, post, schedule) in [
        ([0x89; 32], [0x99; 32], [0xaa; 32]),
        ([0x88; 32], [0x98; 32], [0xaa; 32]),
        ([0x88; 32], [0x99; 32], [0xab; 32]),
    ] {
        let changed =
            FriLayer1BindingV2::new_v2(layer0, root, [0x66; 32], post, [0x77; 32], schedule)
                .unwrap();
        assert_ne!(
            fri1_context_digest_v2(descriptor, changed).unwrap(),
            FRI1_CONTEXT_KAT_V2
        );
    }
}

#[test]
fn layer1_leaf_and_node_frames_match_independent_literal_kats() {
    let parameter = [0x11; 32];
    let values = [0x42_u8; 6_080];
    assert_eq!(
        fri1_leaf_hash_v2(parameter, &values).unwrap(),
        manual_layer1_leaf_v2(parameter, &values)
    );
    assert_eq!(
        fri1_leaf_hash_v2(parameter, &values).unwrap(),
        FRI1_LEAF_KAT_V2
    );
    assert_eq!(
        fri1_node_hash_v2(parameter, 7, [0x31; 32], [0x52; 32]).unwrap(),
        manual_layer1_node_v2(parameter, 7, [0x31; 32], [0x52; 32])
    );
    assert_eq!(
        fri1_node_hash_v2(parameter, 7, [0x31; 32], [0x52; 32]).unwrap(),
        FRI1_NODE_KAT_V2
    );
}

#[test]
fn source_guards_pin_the_bounded_nonauthorizing_layer1_prerequisite() {
    let source = include_str!("batch_fri_v2.rs");
    let storage = include_str!("batch_fri_v2/storage_v2.rs");
    let fold = include_str!("batch_fri_v2/storage_v2/fold_layer1_v2.rs");
    let parent = include_str!("../post_root_v2.rs");
    assert!(source.lines().count() <= 500);
    assert!(storage.lines().count() <= 450);
    assert!(fold.lines().count() <= 650);
    assert!(parent.contains("#[path = \"post_root_v2/batch_fri_v2.rs\"]\nmod batch_fri_v2;"));
    assert!(storage.contains("#[path = \"storage_v2/fold_layer1_v2.rs\"]"));
    for required in [
        "BATCH_FRI0_RECORDS_V2: u64 = 512 * 380",
        "BATCH_FRI0_VALUES_V2: u64 = BATCH_FRI0_RECORDS_V2 * 1_024",
        "BATCH_FRI0_TOTAL_IO_BYTES_V2: u64 = 15_953_920_000",
        "BATCH_FRI0_RETAINED_FILE_BYTES_V2: u64 = 10_370_826_240",
        "BATCH_FRI0_ROOT_HEAP_BYTES_V2: usize = 6_225_920 + 16_384",
        "exact_batch: Infallible",
        "c0: C0BatchReplayV2",
        "cq: CqBatchReplayV2",
        "challenges: ProverBatchChallengesV2",
        "replay_permit",
        "BATCH_FRI1_LEAVES_V2: u64 = 262_144",
        "BATCH_FRI1_VALUES_V2: u64 = 99_614_720",
        "BATCH_FRI1_RECORDS_V2: u64 = 97_280",
        "BATCH_FRI1_TOTAL_IO_BYTES_V2: u64 = 7_976_960_000",
        "BATCH_FRI1_FILE_BYTES_V2: u64 = 1_595_392_000",
        "BATCH_FRI01_RETAINED_FILE_BYTES_V2: u64 = 4_786_176_000",
        "BATCH_FRI1_RETAINED_TOTAL_BYTES_V2: u64 = 11_966_218_240",
        "BATCH_FRI1_FOLD_HEAP_BYTES_V2: usize = 49_152",
        "BATCH_FRI1_ROOT_HEAP_BYTES_V2: usize = 6_242_304",
        "BATCH_FRI1_FRONTIER_BYTES_V2: usize = 608",
        "BATCH_FRI1_WIRE_BYTES_V2: u64 = 0",
        "accepted_fri0: Option<FriLayer0RootedV2>",
        "accepted_fri1: Option<FriLayer1RootedV2>",
        "transcript: Option<ProverFriLayer0FoldCompleteV2>",
        "authenticated_layer0_replay: Infallible",
        "exact_layer0_fold: Infallible",
        "authenticated_layer1_root: Infallible",
    ] {
        assert!(source.contains(required), "missing batch pin: {required}");
    }
    for false_gate in [
        "BATCH_FRI0_MATERIALIZED_V2: bool = false",
        "BATCH_FRI0_ROOT_SEALED_V2: bool = false",
        "BATCH_FRI1_MATERIALIZED_V2: bool = false",
        "BATCH_FRI1_ROOT_SEALED_V2: bool = false",
        "AUTHENTICATED_FRI_REPLAY_COMPLETE_V2: bool = false",
        "FRI_ALL_FOLDS_COMPLETE_V2: bool = false",
        "BATCH_FRI_ZERO_KNOWLEDGE_BOUND_V2: bool = false",
        "BATCH_FRI_CANONICAL_PROOF_EMITTED_V2: bool = false",
        "BATCH_FRI_OPERATIONAL_RECEIPT_ACCEPTED_V2: bool = false",
        "BATCH_FRI_MEASURED_RSS_WITHIN_CAP_V2: bool = false",
        "BATCH_FRI_RELEASE_READY_V2: bool = false",
        "BATCH_FRI_RELEASE_COMPLETE_V2: bool = false",
    ] {
        assert!(
            source.contains(false_gate),
            "missing false gate: {false_gate}"
        );
    }
    let order_guard = fold.find("pair_block != self.next_pair_block").unwrap();
    let lower_read = fold
        .find("read_slot_v1(lower_slot, owner.context_digest)")
        .unwrap();
    let upper_read = fold
        .find("read_slot_v1(upper_slot, owner.context_digest)")
        .unwrap();
    assert!(order_guard < lower_read && lower_read < upper_read);
    for required in [
        "checked_mul(u64::from(FRI1_COLUMNS_V2))",
        "checked_add(FRI1_PAIR_BLOCKS_V2)",
        "FriLayer0ReplayCompleteV2",
        "snapshot.read_slot_v1(slot, self.context_digest)",
        "ZeroizingFriLayer1WindowV2",
        "self.bytes.fill(0)",
        "atomic::compiler_fence(atomic::Ordering::SeqCst)",
        "FRI1_FRONTIER_NODES_V2: usize = 19",
    ] {
        assert!(fold.contains(required), "missing fold/auth pin: {required}");
    }
    assert!(!source.contains("impl Clone for BatchFriLayer1RootPreparedV2"));
    assert!(!fold.contains("impl Clone for FriLayer0FoldReplayV2"));
    assert!(!fold.contains("impl Clone for FriLayer1RootedV2"));
    for forbidden in [
        "pub struct",
        "pub enum",
        "pub fn",
        "pub(crate)",
        "pub use",
        "reset",
        "path_v2",
        "key_v2",
        "snapshot_v2",
        "challenge_v2(&self",
    ] {
        assert!(
            !source.contains(forbidden),
            "forbidden batch surface: {forbidden}"
        );
        assert!(
            !storage.contains(forbidden),
            "forbidden storage surface: {forbidden}"
        );
        assert!(
            !fold.contains(forbidden),
            "forbidden fold surface: {forbidden}"
        );
    }
}
