use super::*;
use crate::vega::sponge::Keccak256;

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
fn source_guards_pin_the_bounded_nonauthorizing_layer0_prerequisite() {
    let source = include_str!("batch_fri_v2.rs");
    let storage = include_str!("batch_fri_v2/storage_v2.rs");
    let parent = include_str!("../post_root_v2.rs");
    assert!(source.lines().count() <= 350);
    assert!(storage.lines().count() <= 500);
    assert!(parent.contains("#[path = \"post_root_v2/batch_fri_v2.rs\"]\nmod batch_fri_v2;"));
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
    ] {
        assert!(source.contains(required), "missing batch pin: {required}");
    }
    for false_gate in [
        "BATCH_FRI0_MATERIALIZED_V2: bool = false",
        "BATCH_FRI0_ROOT_SEALED_V2: bool = false",
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
    }
}
