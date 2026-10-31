//! Canonical batched SHA-256 call manifest and four-lane address/value bus.
//!
//! The first-release relation has one verifier-owned maximum schedule:
//!
//! - three certificate-TBS slots (the third is a canonical dummy for a
//!   depth-two chain);
//! - `TBSCertList` and exact signed-CRL commitment calls;
//! - seven projection calls;
//! - the governed CRL issuer-SPKI digest;
//! - all three authoritative governance-record self-digests;
//! - one compact trust-anchor leaf and twelve compact-tree nodes.
//!
//! Exact message lengths remain private. Each call owns a maximum block range, constrains the one
//! legal SHA padding for its private length, and makes every unused capacity row canonical. Four
//! independently domain-separated Goldilocks products bind `(call, role, slot, word kind, word
//! offset, value)` between source and SHA adapters. Products accumulate continuously inside each
//! whole-call-packed physical segment. Separate per-call products can still be replayed for
//! cross-adapter equality. The canonical local SHA relation and joint original-oracle
//! quotients consume those products privately; no terminal scalar is serialized.
use super::sha_residue_output::ShaResidueSinkV1;
#[cfg(test)]
use super::sha256_word_air::{ZkX509WordMemoryChallengesV1, ZkX509WordMemoryLaneChallengesV1};
use super::{
    credential_pre_aux::ZkX509CredentialPreAuxBindingV1,
    private_table::{PrivateTableV1, zeroize_field_rows_v1, zeroize_fields_v1},
    sha_word_stark::{
        ZkX509ShaWordCapacityBaseSourceV1, ZkX509ShaWordCapacityTraceV1,
        build_sha_word_capacity_base_source_v1, build_sha_word_capacity_base_with_fixed_v1,
        validate_zk_x509_sha_word_stark_challenges_v1,
    },
};
#[cfg(test)]
use super::{
    io_air::{ZkX509IoEndpointV1, ZkX509IoSegmentRoleV1},
    profile::{ZK_X509_MAIN_COMMON_LDE_LOG2_V1, ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1},
    rfc5280_stark::{
        zk_x509_rfc5280_opened_output_factor_fields_v1, zk_x509_rfc5280_opened_output_factor_v1,
    },
};
use super::{
    merkle::{
        ZK_X509_CA_COMPACT_TREE_DEPTH_V1, ZK_X509_CA_SPKI_DER_BYTES_V1,
        ZK_X509_CERTIFICATE_POLICY_RECORD_MAX_PREIMAGE_BYTES_V1,
        ZK_X509_CRL_COMMITMENT_MAX_DER_BYTES_V1, ZK_X509_CRL_RECORD_PREIMAGE_BYTES_V1,
        ZK_X509_TRUST_ANCHOR_RECORD_PREIMAGE_BYTES_V1, ca_leaf_preimage_v1, ca_node_preimage_v1,
        crl_commitment_preimage_v1, crl_issuer_spki_preimage_v1,
    },
    profile::ZK_X509_MAX_CHAIN_DEPTH_V1,
    projection_air::{ZK_X509_PROJECTION_HASH_BUFFER_BYTES_V1, ZK_X509_PROJECTION_HASH_SLOTS_V1},
    rfc5280_stark::{
        ZkX509Rfc5280OutputRoleV1, ZkX509Rfc5280StarkChallengesV1,
        zk_x509_rfc5280_opened_output_factor_fields_after_challenge_validation_v1,
    },
    sha_word_stark::{
        SHA_WORD_CAPACITY_AUX_WIDTH_V1, SHA_WORD_CAPACITY_BASE_WIDTH_V1,
        SHA_WORD_CAPACITY_CALL_FIRST_V1, SHA_WORD_CAPACITY_CALL_LAST_V1,
        SHA_WORD_CAPACITY_CONSTRAINT_COUNT_V1, SHA_WORD_CAPACITY_CONSTRAINT_DEGREE_V1,
        SHA_WORD_CAPACITY_DIGEST_SELECTOR_V1, SHA_WORD_CAPACITY_DIGEST_WORD_INDEX_V1,
        SHA_WORD_CAPACITY_FIXED_WIDTH_V1, SHA_WORD_CAPACITY_INPUT_WORD_INDEX_V1,
        SHA_WORD_CAPACITY_INPUT_WORD_V1, SHA_WORD_CAPACITY_LOCAL_ROWS_PER_BLOCK_V1,
        SHA_WORD_CAPACITY_MESSAGE_COUNT_V1, SHA_WORD_CAPACITY_MESSAGE_MASK_V1,
        SHA_WORD_CAPACITY_ROW_ACTIVE_V1, ZkX509ShaWordCapacityFixedScheduleV1,
        ZkX509ShaWordStarkChallengesV1, ZkX509ShaWordStarkErrorV1,
        compile_sha_word_capacity_fixed_schedule_v1,
        emit_zk_x509_sha_word_capacity_residues_over_field_v1,
    },
};
#[cfg(test)]
use crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1;
use crate::privacy_engines::transparent_stark::{
    GoldilocksFieldV1 as F, PolynomialAirFieldV1, TransparentStarkErrorV1, TransparentTranscriptV1,
};
use sha2::{Digest, Sha256};
use thiserror::Error;
#[path = "sha_frame_prefix.rs"]
mod frame_prefix;
#[path = "sha_native_aux_cache.rs"]
mod native_aux_cache;
#[path = "sha_native_base_cache.rs"]
mod native_base_cache;
#[path = "sha_projection_bus.rs"]
pub(crate) mod projection_bus;
pub(crate) use frame_prefix::{ShaFrameByteV1, sha_frame_byte_v1};
/// Per-type closed-relation DER admission limit.
pub const ZK_X509_SHA_CALL_MAX_DER_BYTES_V1: usize = 4_096;
/// Exact number of canonical SHA calls at maximum shape.
pub const ZK_X509_SHA_CALL_COUNT_V1: usize = 29;
/// Exact maximum SHA compression blocks across all calls.
pub const ZK_X509_SHA_MAX_BLOCKS_V1: usize = 616;
/// Exact maximum word-address events, including eight digest words per call.
pub const ZK_X509_SHA_CALL_EVENT_COUNT_V1: usize = 10_088;
/// Local word-AIR rows per SHA block.
pub const ZK_X509_SHA_LOCAL_ROWS_PER_BLOCK_V1: usize = SHA_WORD_CAPACITY_LOCAL_ROWS_PER_BLOCK_V1;
/// Per-call local word-AIR initialization plus digest-read rows.
pub const ZK_X509_SHA_LOCAL_ROWS_PER_CALL_V1: usize = 16;
/// Word-memory rows per SHA block.
pub const ZK_X509_SHA_MEMORY_ROWS_PER_BLOCK_V1: usize = 2_136;
/// Per-call fixed word-memory rows.
pub const ZK_X509_SHA_MEMORY_ROWS_PER_CALL_V1: usize = 16;
/// Exact maximum local rows in the canonical batch.
pub const ZK_X509_SHA_MAX_LOCAL_ROWS_V1: usize = 655_888;
/// Exact maximum word-memory rows in the canonical batch.
pub const ZK_X509_SHA_MAX_MEMORY_ROWS_V1: usize = 1_316_240;
/// Exact maximum logical rows in the canonical batch.
pub const ZK_X509_SHA_MAX_LOGICAL_ROWS_V1: usize = 1_972_128;
/// Largest one-call fixed-capacity trace (the 4,161-byte CRL commitment).
#[cfg(test)]
pub const ZK_X509_SHA_MAX_CALL_LOGICAL_ROWS_V1: usize = 211_232;
/// Maximum rows in one logical SHA segment.
pub const ZK_X509_SHA_SEGMENT_ROWS_V1: usize = 1 << 19;
/// Number of deterministic replay segments.
pub const ZK_X509_SHA_SEGMENT_COUNT_V1: usize = 4;
/// Maximum active rows in each replay segment.
///
/// Calls are packed whole into these four bins. No call transition crosses a physical commitment
/// boundary, so every segment suffix is canonical padding and the opened-row verifier never needs a
/// witness-fed cross-segment continuation value.
pub const ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1: [usize; ZK_X509_SHA_SEGMENT_COUNT_V1] =
    [480_288, 521_952, 521_696, 448_192];
/// Sole physical packing order for the 29 whole SHA calls.
///
/// The semantic call index remains unchanged and is what every call-bus tuple
/// commits.  This order only places fixed-capacity call traces into four
/// `2^19` commitment bins without splitting a call.
const ZK_X509_SHA_PHYSICAL_CALL_ORDER_V1: [u8; ZK_X509_SHA_CALL_COUNT_V1] = [
    4, 5, 6, 12, 16, 17, 18, 19, 20, // segment 0
    0, 7, 8, 13, 14, 15, 21, 22, 23, 24, 25, // segment 1
    1, 2, 9, // segment 2
    3, 10, 11, 26, 27, 28, // segment 3
];
const ZK_X509_SHA_PHYSICAL_CALL_COUNTS_V1: [usize; ZK_X509_SHA_SEGMENT_COUNT_V1] = [9, 11, 3, 6];
/// Bits in the maximum private raw-message length (`0..=4_161`).
const ZK_X509_SHA_RFC_LENGTH_BITS_V1: usize = 13;
/// Fixed-capacity SHA base width with private length/padding selectors and a
/// proof-bound binary decomposition of the running raw-message length.
pub const ZK_X509_SHA_BATCH_BASE_WIDTH_V1: usize =
    SHA_WORD_CAPACITY_BASE_WIDTH_V1 + ZK_X509_SHA_RFC_LENGTH_BITS_V1;
/// Word-memory/control columns, separate SHA call buses, and four RFC consumer product streams per
/// lane. One stream owns each byte of an input word, so a selected raw byte remains degree two and
/// its product recurrence degree three.
pub const ZK_X509_SHA_BATCH_AUX_WIDTH_V1: usize =
    SHA_WORD_CAPACITY_AUX_WIDTH_V1 + 7 * ZK_X509_SHA_BUS_LANES_V1;
/// Verifier-preprocessed word, call identity, segment boundary, raw-length
/// channel schedule, and field-native RFC event descriptors.
pub const ZK_X509_SHA_BATCH_FIXED_WIDTH_V1: usize =
    SHA_WORD_CAPACITY_FIXED_WIDTH_V1 + 9 + ZK_X509_SHA_CA_CALL_COUNT_V1 + 4 * 6 + 19;
/// Capacity SHA residues plus recurrence and reset for eight private input/digest
/// accumulators; only compact-CA call boundaries and RFC stream endpoints are public.
pub const ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1: usize = SHA_WORD_CAPACITY_CONSTRAINT_COUNT_V1
    + 4 * ZK_X509_SHA_BUS_LANES_V1
    + 4 * ZK_X509_SHA_CA_CALL_COUNT_V1 * ZK_X509_SHA_BUS_LANES_V1
    + 2 * ZK_X509_SHA_RFC_LENGTH_BITS_V1
    + 8
    + 1
    + 2 * ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1 * ZK_X509_SHA_BUS_LANES_V1
    + ZK_X509_SHA_BATCH_BASE_WIDTH_V1
    + projection_bus::AUX_START_V1
    + 2 * ZK_X509_SHA_BUS_LANES_V1;
/// Local SHA relation with its208 public CA-boundary equations removed.
/// The consuming joint MAIN composition must include all108 original CA joins.
pub const ZK_X509_SHA_BATCH_LOCAL_CONSTRAINT_COUNT_V1: usize = ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1
    - 4 * ZK_X509_SHA_CA_CALL_COUNT_V1 * ZK_X509_SHA_BUS_LANES_V1;
/// Maximum total degree including verifier-fixed selectors.
///
/// The capacity equations reach six. Call-product terminal constraints also
/// reach six: degree-two call selectors times a degree-four active input update.
pub const ZK_X509_SHA_BATCH_CONSTRAINT_DEGREE_V1: u8 = SHA_WORD_CAPACITY_CONSTRAINT_DEGREE_V1;
/// Physical 64-column base commitment chunks per logical SHA segment.
pub const ZK_X509_SHA_BATCH_BASE_CHUNKS_PER_SEGMENT_V1: usize = 2;
/// Physical 64-column auxiliary commitment chunks per logical SHA segment.
pub const ZK_X509_SHA_BATCH_AUX_CHUNKS_PER_SEGMENT_V1: usize = 2;
/// Base width after the four same-logical-size registrations are bucketed.
pub const ZK_X509_SHA_BUCKET_BASE_WIDTH_V1: usize =
    ZK_X509_SHA_SEGMENT_COUNT_V1 * ZK_X509_SHA_BATCH_BASE_WIDTH_V1;
/// Auxiliary width after the four same-logical-size registrations are bucketed.
pub const ZK_X509_SHA_BUCKET_AUX_WIDTH_V1: usize =
    ZK_X509_SHA_SEGMENT_COUNT_V1 * ZK_X509_SHA_BATCH_AUX_WIDTH_V1;
/// Physical base commitment instances in the one same-log SHA bucket.
pub const ZK_X509_SHA_BUCKET_BASE_CHUNKS_V1: usize =
    ZK_X509_SHA_SEGMENT_COUNT_V1 * ZK_X509_SHA_BATCH_BASE_CHUNKS_PER_SEGMENT_V1;
/// Physical auxiliary commitment instances in the one same-log SHA bucket.
pub const ZK_X509_SHA_BUCKET_AUX_CHUNKS_V1: usize =
    ZK_X509_SHA_SEGMENT_COUNT_V1 * ZK_X509_SHA_BATCH_AUX_CHUNKS_PER_SEGMENT_V1;
/// Bytes in one simultaneously resident common-domain field column.
#[cfg(test)]
pub const ZK_X509_COMMON_LDE_COLUMN_BYTES_V1: u64 = (1_u64 << ZK_X509_MAIN_COMMON_LDE_LOG2_V1) * 8;
/// Bytes in one caller-owned native SHA replay column.
#[cfg(test)]
pub const ZK_X509_SHA_NATIVE_REPLAY_COLUMN_BYTES_V1: u64 =
    ZK_X509_SHA_SEGMENT_ROWS_V1 as u64 * core::mem::size_of::<F>() as u64;
/// Native bytes for one complete base+aux SHA segment.
#[cfg(test)]
pub const ZK_X509_SHA_ONE_NATIVE_SEGMENT_BYTES_V1: u64 = (ZK_X509_SHA_BATCH_BASE_WIDTH_V1 as u64
    + ZK_X509_SHA_BATCH_AUX_WIDTH_V1 as u64)
    * ZK_X509_SHA_SEGMENT_ROWS_V1 as u64
    * 8;
/// Retained field payload for the largest call after on-demand row widening.
#[cfg(test)]
pub const ZK_X509_SHA_MAX_RETAINED_CALL_FIELD_BYTES_V1: u64 = ZK_X509_SHA_MAX_CALL_LOGICAL_ROWS_V1
    as u64
    * (ZK_X509_SHA_BATCH_BASE_WIDTH_V1
        + ZK_X509_SHA_BATCH_AUX_WIDTH_V1
        + ZK_X509_SHA_BATCH_FIXED_WIDTH_V1) as u64
    * 8;
/// Conservative peak for one native output column and one replayed call.
#[cfg(test)]
pub const ZK_X509_SHA_NATIVE_COLUMN_REPLAY_PEAK_BYTES_V1: u64 =
    ZK_X509_SHA_NATIVE_REPLAY_COLUMN_BYTES_V1 + ZK_X509_SHA_MAX_RETAINED_CALL_FIELD_BYTES_V1;
/// Forbidden eager retention of all four native SHA auxiliary matrices.
#[cfg(test)]
pub const ZK_X509_SHA_EAGER_AUX_MATRIX_BYTES_V1: u64 = ZK_X509_SHA_SEGMENT_COUNT_V1 as u64
    * ZK_X509_SHA_BATCH_AUX_WIDTH_V1 as u64
    * ZK_X509_SHA_NATIVE_REPLAY_COLUMN_BYTES_V1;
/// One exact incremental SHA-256 row-hash state per common-domain row.
#[cfg(test)]
pub const ZK_X509_SHA_ROW_HASH_STATE_BYTES_V1: u64 =
    (1_u64 << ZK_X509_MAIN_COMMON_LDE_LOG2_V1) * core::mem::size_of::<Sha256>() as u64;
/// Explicit scratch reserve in the release streaming plan.
#[cfg(test)]
pub const ZK_X509_SHA_STREAMING_SCRATCH_BYTES_V1: u64 = 128 * 1024 * 1024;
/// Conservative peak for one native slice, row-hash states, sequential
/// LDE/composition columns, and scratch.
#[cfg(test)]
pub const ZK_X509_SHA_STREAMING_PEAK_BYTES_V1: u64 = ZK_X509_SHA_ONE_NATIVE_SEGMENT_BYTES_V1
    + ZK_X509_SHA_ROW_HASH_STATE_BYTES_V1
    + 2 * ZK_X509_COMMON_LDE_COLUMN_BYTES_V1
    + ZK_X509_SHA_STREAMING_SCRATCH_BYTES_V1;
/// Eager retention of all four native slices, forbidden by the release plan.
#[cfg(test)]
pub const ZK_X509_SHA_EAGER_NATIVE_BYTES_V1: u64 =
    ZK_X509_SHA_ONE_NATIVE_SEGMENT_BYTES_V1 * ZK_X509_SHA_SEGMENT_COUNT_V1 as u64;
/// SHA-local peak must remain under the global first-release prover envelope.
#[cfg(test)]
pub const ZK_X509_SHA_PROVER_MEMORY_LIMIT_BYTES_V1: u64 = ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1;
/// SHA trace opening bytes before Merkle, FRI, and DEEP overhead.
#[cfg(test)]
pub const ZK_X509_SHA_TRACE_OPENING_BYTES_V1: usize = super::profile::ZK_X509_FRI_QUERY_COUNT_V1
    as usize
    * 2
    * (ZK_X509_SHA_BUCKET_BASE_WIDTH_V1 + ZK_X509_SHA_BUCKET_AUX_WIDTH_V1)
    * core::mem::size_of::<F>();
/// Maximum SHA-only aggregate wire before DEEP openings under the canonical
/// 136-query, log-22 domain and twelve-round FRI schedule.
#[cfg(test)]
pub const ZK_X509_SHA_MAX_ENCODED_PROOF_BYTES_V1: usize = 2_870_880;
#[cfg(test)]
const _: () = assert!(ZK_X509_SHA_NATIVE_REPLAY_COLUMN_BYTES_V1 == 4 * 1024 * 1024);
#[cfg(test)]
const _: () =
    assert!(ZK_X509_SHA_NATIVE_COLUMN_REPLAY_PEAK_BYTES_V1 < ZK_X509_SHA_EAGER_AUX_MATRIX_BYTES_V1);
#[cfg(test)]
const _: () = assert!(
    ZK_X509_SHA_NATIVE_COLUMN_REPLAY_PEAK_BYTES_V1 < ZK_X509_SHA_PROVER_MEMORY_LIMIT_BYTES_V1
);
/// Four independent lanes for word-memory and cross-adapter SHA calls.
pub const ZK_X509_SHA_BUS_LANES_V1: usize = 4;
const ZK_X509_SHA_CALL_PRODUCT_WIDTH_V1: usize = 2 * ZK_X509_SHA_BUS_LANES_V1;
const ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1: usize = 4;
const ZK_X509_SHA_RFC_PRODUCT_WIDTH_V1: usize =
    ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1 * ZK_X509_SHA_BUS_LANES_V1;
const ZK_X509_SHA_PRODUCT_WIDTH_V1: usize =
    ZK_X509_SHA_CALL_PRODUCT_WIDTH_V1 + ZK_X509_SHA_RFC_PRODUCT_WIDTH_V1 + ZK_X509_SHA_BUS_LANES_V1;
const CALL_TUPLE_TERMS_V1: usize = 7;
const SHA_WORDS_PER_BLOCK_V1: usize = 16;
const SHA_DIGEST_WORDS_V1: usize = 8;
const SHA_BLOCK_BYTES_V1: usize = 64;
pub const ZK_X509_SHA_INPUT_PRODUCTS_V1: usize = SHA_WORD_CAPACITY_AUX_WIDTH_V1;
pub const ZK_X509_SHA_DIGEST_PRODUCTS_V1: usize =
    ZK_X509_SHA_INPUT_PRODUCTS_V1 + ZK_X509_SHA_BUS_LANES_V1;
pub const ZK_X509_SHA_RFC_CONSUMER_PRODUCTS_V1: usize =
    ZK_X509_SHA_DIGEST_PRODUCTS_V1 + ZK_X509_SHA_BUS_LANES_V1;
const ZK_X509_SHA_LENGTH_BITS_START_V1: usize = SHA_WORD_CAPACITY_BASE_WIDTH_V1;
pub const ZK_X509_SHA_FIXED_CALL_V1: usize = SHA_WORD_CAPACITY_FIXED_WIDTH_V1;
pub const ZK_X509_SHA_FIXED_ROLE_V1: usize = ZK_X509_SHA_FIXED_CALL_V1 + 1;
pub const ZK_X509_SHA_FIXED_SLOT_V1: usize = ZK_X509_SHA_FIXED_ROLE_V1 + 1;
pub const ZK_X509_SHA_FIXED_SEGMENT_FIRST_V1: usize = ZK_X509_SHA_FIXED_SLOT_V1 + 1;
pub const ZK_X509_SHA_FIXED_SEGMENT_LAST_V1: usize = ZK_X509_SHA_FIXED_SEGMENT_FIRST_V1 + 1;
pub const ZK_X509_SHA_FIXED_PHYSICAL_PADDING_V1: usize = ZK_X509_SHA_FIXED_SEGMENT_LAST_V1 + 1;
/// One verifier-preprocessed selector for each compact-CA SHA call.
///
/// These selectors are committed by the fixed oracle. They cannot be reconstructed by branching on
/// the opened call column because that column is a polynomial evaluation, rather than a discrete
/// call number, away from the native trace domain.
pub const ZK_X509_SHA_FIXED_CA_CALL_SELECTORS_V1: usize = ZK_X509_SHA_FIXED_PHYSICAL_PADDING_V1 + 1;
pub const ZK_X509_SHA_FIXED_RFC_LENGTH_PAIR_V1: usize =
    ZK_X509_SHA_FIXED_CA_CALL_SELECTORS_V1 + ZK_X509_SHA_CA_CALL_COUNT_V1;
pub const ZK_X509_SHA_FIXED_RFC_LENGTH_PAIR_INDEX_V1: usize =
    ZK_X509_SHA_FIXED_RFC_LENGTH_PAIR_V1 + 1;
const ZK_X509_SHA_FIXED_RFC_LENGTH_PREFIX_V1: usize =
    ZK_X509_SHA_FIXED_RFC_LENGTH_PAIR_INDEX_V1 + 1;
const ZK_X509_SHA_FIXED_RFC_STREAMS_V1: usize = ZK_X509_SHA_FIXED_RFC_LENGTH_PREFIX_V1 + 1;
const ZK_X509_SHA_FIXED_RFC_STREAM_STRIDE_V1: usize = 6;
const ZK_X509_SHA_FIXED_RFC_MESSAGE_EVENT_V1: usize = 0;
const ZK_X509_SHA_FIXED_RFC_LENGTH_HIGH_VALUE_V1: usize = 1;
const ZK_X509_SHA_FIXED_RFC_LENGTH_LOW_VALUE_V1: usize = 2;
const ZK_X509_SHA_FIXED_RFC_ROLE_V1: usize = 3;
const ZK_X509_SHA_FIXED_RFC_CHANNEL_V1: usize = 4;
const ZK_X509_SHA_FIXED_RFC_OFFSET_V1: usize = 5;
/// Four message-byte masks for canonical SHA frame constants and raw lengths.
pub(crate) const ZK_X509_SHA_FIXED_FRAME_MASK_V1: usize =
    ZK_X509_SHA_FIXED_RFC_STREAMS_V1 + 4 * ZK_X509_SHA_FIXED_RFC_STREAM_STRIDE_V1;
/// Four constant parts of canonical SHA frame bytes, in big-endian word order.
pub(crate) const ZK_X509_SHA_FIXED_FRAME_CONSTANT_V1: usize = ZK_X509_SHA_FIXED_FRAME_MASK_V1 + 4;
/// Four selectors for the high byte of the private CRL frame length.
pub(crate) const ZK_X509_SHA_FIXED_FRAME_LENGTH_HIGH_V1: usize =
    ZK_X509_SHA_FIXED_FRAME_CONSTANT_V1 + 4;
/// Four selectors for the low byte of the private CRL frame length.
pub(crate) const ZK_X509_SHA_FIXED_FRAME_LENGTH_LOW_V1: usize =
    ZK_X509_SHA_FIXED_FRAME_LENGTH_HIGH_V1 + 4;
const CERTIFICATE_TBS_CALL_START_V1: usize = 0;
const CRL_TBS_CALL_V1: usize = 3;
const CRL_COMMITMENT_CALL_V1: usize = 4;
const PROJECTION_CALL_START_V1: usize = 5;
const CRL_ISSUER_SPKI_CALL_V1: usize = 12;
const TRUST_ANCHOR_RECORD_CALL_V1: usize = 13;
const CERTIFICATE_POLICY_RECORD_CALL_V1: usize = 14;
const CRL_RECORD_CALL_V1: usize = 15;
/// Canonical global call index of the compact CA occupied leaf.
pub const ZK_X509_SHA_CA_LEAF_CALL_V1: usize = 16;
/// Canonical global call index of compact CA node level zero.
pub const ZK_X509_SHA_CA_NODE_CALL_START_V1: usize = 17;
/// Number of compact-CA calls whose individual products are consumed by the credential composition.
pub const ZK_X509_SHA_CA_CALL_COUNT_V1: usize =
    ZK_X509_SHA_CALL_COUNT_V1 - ZK_X509_SHA_CA_LEAF_CALL_V1;
const _: () = {
    assert!(ZK_X509_MAX_CHAIN_DEPTH_V1 == 3);
    assert!(ZK_X509_PROJECTION_HASH_SLOTS_V1 == 7);
    assert!(ZK_X509_CA_COMPACT_TREE_DEPTH_V1 == 12);
    assert!(
        ZK_X509_SHA_CA_NODE_CALL_START_V1 + ZK_X509_CA_COMPACT_TREE_DEPTH_V1
            == ZK_X509_SHA_CALL_COUNT_V1
    );
    assert!(
        ZK_X509_SHA_MAX_LOCAL_ROWS_V1
            == ZK_X509_SHA_LOCAL_ROWS_PER_BLOCK_V1 * ZK_X509_SHA_MAX_BLOCKS_V1
                + ZK_X509_SHA_LOCAL_ROWS_PER_CALL_V1 * ZK_X509_SHA_CALL_COUNT_V1
    );
    assert!(
        ZK_X509_SHA_MAX_MEMORY_ROWS_V1
            == ZK_X509_SHA_MEMORY_ROWS_PER_BLOCK_V1 * ZK_X509_SHA_MAX_BLOCKS_V1
                + ZK_X509_SHA_MEMORY_ROWS_PER_CALL_V1 * ZK_X509_SHA_CALL_COUNT_V1
    );
    assert!(
        ZK_X509_SHA_MAX_LOGICAL_ROWS_V1
            == ZK_X509_SHA_MAX_LOCAL_ROWS_V1 + ZK_X509_SHA_MAX_MEMORY_ROWS_V1
    );
    assert!(
        ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[0]
            + ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[1]
            + ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[2]
            + ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[3]
            == ZK_X509_SHA_MAX_LOGICAL_ROWS_V1
    );
    assert!(ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[0] <= ZK_X509_SHA_SEGMENT_ROWS_V1);
    assert!(ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[1] <= ZK_X509_SHA_SEGMENT_ROWS_V1);
    assert!(ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[2] <= ZK_X509_SHA_SEGMENT_ROWS_V1);
    assert!(ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[3] <= ZK_X509_SHA_SEGMENT_ROWS_V1);
    assert!(ZK_X509_SHA_BUCKET_BASE_WIDTH_V1 == 356);
    assert!(ZK_X509_SHA_BUCKET_AUX_WIDTH_V1 == 328);
    assert!(ZK_X509_SHA_BUCKET_BASE_CHUNKS_V1 == 8);
    assert!(ZK_X509_SHA_BUCKET_AUX_CHUNKS_V1 == 8);
    assert!(
        ZK_X509_SHA_BATCH_BASE_WIDTH_V1
            == SHA_WORD_CAPACITY_BASE_WIDTH_V1 + ZK_X509_SHA_RFC_LENGTH_BITS_V1
    );
    assert!(
        ZK_X509_SHA_BATCH_AUX_WIDTH_V1
            == SHA_WORD_CAPACITY_AUX_WIDTH_V1 + 7 * ZK_X509_SHA_BUS_LANES_V1
    );
    assert!(
        ZK_X509_SHA_BATCH_FIXED_WIDTH_V1
            == SHA_WORD_CAPACITY_FIXED_WIDTH_V1 + 9 + ZK_X509_SHA_CA_CALL_COUNT_V1 + 4 * 6 + 19
    );
    assert!(ZK_X509_SHA_FIXED_FRAME_LENGTH_LOW_V1 + 4 == projection_bus::FIX_WORD_V1);
    assert!(projection_bus::FIX_END_V1 == ZK_X509_SHA_BATCH_FIXED_WIDTH_V1);
    assert!(projection_bus::AUX_START_V1 + 4 == ZK_X509_SHA_BATCH_AUX_WIDTH_V1);
    assert!(ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1 == 801);
    assert!(ZK_X509_SHA_BATCH_CONSTRAINT_DEGREE_V1 == 6);
};
/// Stable identity of the release SHA batch and call bus.
pub const ZK_X509_SHA_CALL_BUS_STARK_DESCRIPTOR_V1: &[u8] = b"zk-x509-sha-call-bus-stark-v1-incompatible:29-fixed-capacity-calls=cert-tbs[3]+crl-tbs+framed-complete-signed-crl+projection[7]+issuer-spki+trust-record+policy-record+crl-record+compact-ca-leaf+compact-ca-node[12]:max-blocks616:word-rows1972128=compression655424+local-init232+local-digest232+memory1316240:four-log19-segments-whole-call-packed-active-rows480288,521952,521696,448192-no-cross-segment-call-transition:base89=word-capacity76+proof-bound-rfc-raw-length-bits13:aux82=word-capacity54+input-products4+digest-products4+rfc-consumer-products16+projection-products4:fixed137=word72+call-segment-length-control9+thirteen-verifier-one-hot-compact-ca-call-selectors+four-field-native-rfc-event-descriptors-of-width6+canonical-frame-byte-mask-constant-and-private-length-high-low16+active-projection-event-selectors3:constraints593=local-private-running-products:MAIN108-cross-proof-original-polynomial-quotients:degree6-including-fixed-selectors:polynomial-digest-address=digest*dynamic+(1-digest)*fixed:base-two-chunks-aux-two-chunks-per-segment:same-log-bucket-base356-aux328-base-chunks8-aux-chunks8:private-exact-length-unique-padding-transition-across-blocks-and-active-block-prefix:fine-grained-message-cap-and-fixed-role-length-enforcement:frozen-canonical-inactive-computation-memory-and-mask-suffix:selected-digest-from-unique-final-active-block:inactive-chain-and-projection-slots-canonical-sha-empty-dummy:address=(call,role,slot,input-or-digest,word):four-independent-domain-separated-goldilocks-lanes:separate-word-memory-and-call-challenge-families:segment-continuous-private-source-digest-and-rfc-products:no-public-segment-products:main-16-quartic-native-endpoint-links-to-private-rfc-bridges:cyclic-physical-padding-recurrence=1-segment-last-padding:padding-base-and-original-aux=zero:projection-aux-terminal-carry:word-capacity-recurrence=local-compute+digest+memory-call-last:compact-ca-calls16through28-bind-original-source-and-digest-endpoints-through-MAIN108-private-polynomial-quotients-without-product-division:rfc-consumer-products-derived-algebraically-from-committed-message-bits-masks-and-verifier-fixed-event-descriptors:four-byte-streams-total-degree5-recurrences-including-fixed-selectors:all-framed-roles-verifier-compiled-domain-and-static-field-headers-and-protocol-constants:frame-bytes-must-be-message-bytes:private-variable-payloads-remain-unfixed:crl-frame-u64-length-equals-carried13bit-raw-length:proof-bound-u64-raw-length-consumers:certificate-tbs-crl-tbs-framed-complete-crl-and-framed-issuer-spki-channels:three-governance-self-digests-explicit-sha-field-frames:no-host-branch-on-opened-fixed-columns:projection-calls5through11-original-message-length-digest-typed-products:512-zero-padded-payload-words+exact-length+eight-digest-words:row0-private-terminal-row1-one-rest-cyclic:four-original-MAIN-point1-equalities-with-projection:main-common-lde-log22:protocol2-independent-per-lane-fri-mask-oracles:max-encoded-sha-proof2870880:stream-one-call-at-a-time:on-demand-full-row-widening-without-duplicated-aux-or-fixed-vectors";
/// Semantic owner of one canonical SHA call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ZkX509ShaCallRoleV1 {
    /// Certificate `TBSCertificate` digest, leaf-to-root slot.
    CertificateTbs(u8),
    /// Exact `TBSCertList` digest consumed by P-256.
    CrlTbs,
    /// Canonically framed exact complete signed CRL commitment.
    CrlCommitment,
    /// Projection hash slot.
    Projection(u8),
    /// Governed exact CRL issuer-SPKI digest.
    CrlIssuerSpki,
    /// Authoritative trust-anchor revision self-digest.
    TrustAnchorRecord,
    /// Authoritative certificate-policy revision self-digest.
    CertificatePolicyRecord,
    /// Authoritative signed-CRL revision self-digest.
    CrlRecord,
    /// Occupied compact trust-anchor leaf.
    CaLeaf,
    /// Compact trust-anchor internal node, leaf-to-root level.
    CaNode(u8),
}
impl ZkX509ShaCallRoleV1 {
    pub(crate) const fn role_code(self) -> u8 {
        match self {
            Self::CertificateTbs(_) => 1,
            Self::CrlTbs => 2,
            Self::CrlCommitment => 3,
            Self::Projection(_) => 4,
            Self::CrlIssuerSpki => 5,
            Self::TrustAnchorRecord => 6,
            Self::CertificatePolicyRecord => 7,
            Self::CrlRecord => 8,
            Self::CaLeaf => 9,
            Self::CaNode(_) => 10,
        }
    }
    pub(crate) const fn slot(self) -> u8 {
        match self {
            Self::CertificateTbs(slot) | Self::Projection(slot) | Self::CaNode(slot) => slot,
            Self::CrlTbs
            | Self::CrlCommitment
            | Self::CrlIssuerSpki
            | Self::TrustAnchorRecord
            | Self::CertificatePolicyRecord
            | Self::CrlRecord
            | Self::CaLeaf => 0,
        }
    }
}
/// Input schedule or output digest address family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ZkX509ShaCallWordKindV1 {
    /// Canonically padded SHA input word.
    Input,
    /// Big-endian digest word.
    Digest,
}
impl ZkX509ShaCallWordKindV1 {
    pub(crate) const fn code(self) -> u8 {
        match self {
            Self::Input => 0,
            Self::Digest => 1,
        }
    }
}
/// Public shape controlling canonical dummy calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkX509ShaCallPublicShapeV1 {
    /// Public number of disclosed attributes.
    pub(crate) disclosed_attributes: usize,
}
/// Whether a fixed call is required, privately optional, or publicly absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZkX509ShaCallActivationV1 {
    Required,
    OptionalPrivate,
    Inactive,
}
/// One verifier-owned maximum-capacity call slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkX509ShaCallManifestV1 {
    /// Canonical call index.
    pub(crate) call: u8,
    /// Semantic role and role-local slot.
    pub(crate) role: ZkX509ShaCallRoleV1,
    /// Fixed activation policy.  The third certificate slot is privately
    /// optional; its presence never changes the verifier-visible layout.
    pub(crate) activation: ZkX509ShaCallActivationV1,
    /// Maximum private preimage bytes.
    pub(crate) maximum_message_bytes: usize,
    /// Maximum SHA blocks reserved for deterministic replay.
    pub(crate) maximum_blocks: usize,
    /// First fixed address-event row.
    pub(crate) first_event: usize,
    /// First fixed-capacity word-AIR row.
    pub(crate) first_logical_row: usize,
    /// Fixed local word-AIR rows.
    pub(crate) maximum_local_rows: usize,
    /// Fixed execution/sorted word-memory rows.
    pub(crate) maximum_memory_rows: usize,
}
impl ZkX509ShaCallManifestV1 {
    /// Maximum input-word rows reserved by this call.
    pub(crate) const fn maximum_input_words(self) -> usize {
        self.maximum_blocks * SHA_WORDS_PER_BLOCK_V1
    }
    /// Maximum addressed event rows reserved by this call.
    pub(crate) const fn maximum_events(self) -> usize {
        self.maximum_input_words() + SHA_DIGEST_WORDS_V1
    }
    /// Total fixed-capacity word-AIR rows owned by the call.
    pub(crate) const fn maximum_logical_rows(self) -> usize {
        self.maximum_local_rows + self.maximum_memory_rows
    }
}
/// Exact private call witness.
#[derive(Clone, PartialEq, Eq)]
pub struct ZkX509ShaCallWitnessV1 {
    /// Must equal the verifier-owned manifest role.
    pub(crate) role: ZkX509ShaCallRoleV1,
    /// Exact private SHA preimage.  Canonical dummy calls use the empty string.
    pub(crate) message: Vec<u8>,
    /// Exact SHA-256 digest.
    pub(crate) digest: [u8; 32],
}
impl core::fmt::Debug for ZkX509ShaCallWitnessV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkX509ShaCallWitnessV1")
            .field("role", &self.role)
            .field("private_material", &"<redacted>")
            .finish_non_exhaustive()
    }
}
impl ZkX509ShaCallWitnessV1 {
    fn zeroize_private_cells_v1(&mut self) {
        zeroize::Zeroize::zeroize(self.message.as_mut_slice());
        zeroize::Zeroize::zeroize(&mut self.digest);
    }
    /// Overwrite the exact private preimage and its derived digest.
    pub(crate) fn zeroize_private_v1(&mut self) {
        self.zeroize_private_cells_v1();
        self.message.clear();
    }
    #[cfg(test)]
    pub(crate) fn private_is_zeroized_v1(&self) -> bool {
        self.message.is_empty() && self.digest == [0; 32]
    }
}
impl Drop for ZkX509ShaCallWitnessV1 {
    fn drop(&mut self) {
        self.zeroize_private_v1();
    }
}
/// One fixed maximum-capacity address/value event.
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkX509ShaCallEventV1 {
    /// Canonical call index.
    pub(crate) call: u8,
    /// Semantic call role.
    pub(crate) role: ZkX509ShaCallRoleV1,
    /// Input schedule or digest.
    pub(crate) word_kind: ZkX509ShaCallWordKindV1,
    /// Word index in its family.
    pub(crate) word: u16,
    /// Whether the private exact-length schedule consumes this capacity row.
    pub(crate) active: bool,
    /// Constrained 32-bit big-endian word; inactive capacity rows are zero.
    pub(crate) value: u32,
}
/// Reference per-call terminal products retained by material and algebraic controls.
///
/// Raw lengths, active block counts, and digest words are deliberately absent: exposing any of them
/// in proof metadata would defeat the fixed-capacity privacy contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkX509ShaCallTerminalV1 {
    /// Canonical call identity.
    pub(crate) call: u8,
    /// Semantic role and slot.
    pub(crate) role: ZkX509ShaCallRoleV1,
    /// Input-source product terminal per call-bus lane.
    pub(crate) source_products: [F; ZK_X509_SHA_BUS_LANES_V1],
    /// Digest-consumer product terminal per call-bus lane.
    pub(crate) digest_products: [F; ZK_X509_SHA_BUS_LANES_V1],
}
/// Private cumulative start and call-local terminal products for one compact-CA SHA call.
///
/// The MAIN AIR binds `start` on the call's verifier-fixed first row and binds `start * terminal`
/// to the cumulative product on its fixed last row. The multiplication-only relation remains total
/// when a compressed factor is zero and never asks the prover or verifier to divide a bus product.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ZkX509ShaCallBoundaryTerminalV1 {
    /// Canonical call identity in `16..=28`.
    pub(crate) call: u8,
    /// Canonical compact-CA semantic role and level.
    pub(crate) role: ZkX509ShaCallRoleV1,
    /// Segment-cumulative input product immediately before this call.
    pub(crate) source_start_products: [F; ZK_X509_SHA_BUS_LANES_V1],
    /// Segment-cumulative digest product immediately before this call.
    pub(crate) digest_start_products: [F; ZK_X509_SHA_BUS_LANES_V1],
    /// Call-local input product.
    pub(crate) source_products: [F; ZK_X509_SHA_BUS_LANES_V1],
    /// Call-local digest product.
    pub(crate) digest_products: [F; ZK_X509_SHA_BUS_LANES_V1],
}
impl core::fmt::Debug for ZkX509ShaCallBoundaryTerminalV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZkX509ShaCallBoundaryTerminalV1 { <private products redacted> }")
    }
}
impl zeroize::Zeroize for ZkX509ShaCallBoundaryTerminalV1 {
    fn zeroize(&mut self) {
        clear_ca_boundary_products_v1(core::slice::from_mut(self));
    }
}
impl ZkX509ShaCallBoundaryTerminalV1 {
    pub(crate) fn validate_identity_v1(
        &self,
        index: usize,
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        let call = ZK_X509_SHA_CA_LEAF_CALL_V1
            .checked_add(index)
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
        if index >= ZK_X509_SHA_CA_CALL_COUNT_V1
            || usize::from(self.call) != call
            || self.role != manifest_role_v1(call)?
            || self
                .source_start_products
                .iter()
                .chain(&self.digest_start_products)
                .chain(&self.source_products)
                .chain(&self.digest_products)
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509ShaCallBusStarkErrorV1::Terminal);
        }
        Ok(())
    }
}
/// Per-call RFC-output consumer products derived from committed SHA rows.
///
/// The four streams keep one potentially masked message byte per stream. The verifier multiplies
/// the streams only after verifying their individual terminal constraints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkX509ShaRfcConsumerTerminalV1 {
    /// Canonical call identity.
    pub(crate) call: u8,
    /// Semantic role and slot.
    pub(crate) role: ZkX509ShaCallRoleV1,
    /// Four independent byte streams for each RFC tuple-compression lane.
    pub(crate) stream_products: [[F; ZK_X509_SHA_BUS_LANES_V1]; ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1],
}
#[cfg(test)]
impl ZkX509ShaRfcConsumerTerminalV1 {
    /// Combine the independently constrained streams after proof verification.
    pub(crate) fn combined_products(self) -> [F; ZK_X509_SHA_BUS_LANES_V1] {
        core::array::from_fn(|lane| {
            self.stream_products
                .iter()
                .fold(F::ONE, |product, stream| product.mul(stream[lane]))
        })
    }
}
/// Private RFC stream products at one verifier-owned physical segment endpoint.
///
/// Source/digest accumulators and compact-CA call boundaries remain private.
/// Their unconsumed whole-segment totals are not claims.
#[derive(PartialEq, Eq)]
pub struct ZkX509ShaSegmentPrivateEndpointV1 {
    pub(crate) segment: u8,
    pub(crate) rfc_stream_products:
        [[F; ZK_X509_SHA_BUS_LANES_V1]; ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1],
}
impl ZkX509ShaSegmentPrivateEndpointV1 {
    #[cfg(test)]
    pub(crate) fn combined_rfc_products(&self) -> [F; ZK_X509_SHA_BUS_LANES_V1] {
        core::array::from_fn(|lane| {
            self.rfc_stream_products
                .iter()
                .fold(F::ONE, |product, stream| product.mul(stream[lane]))
        })
    }
}
impl Clone for ZkX509ShaSegmentPrivateEndpointV1 {
    fn clone(&self) -> Self {
        let mut owner = Self {
            segment: self.segment,
            rfc_stream_products: [[F::ZERO; 4]; 4],
        };
        for (target, source) in owner
            .rfc_stream_products
            .iter_mut()
            .zip(&self.rfc_stream_products)
        {
            target.copy_from_slice(source);
        }
        owner
    }
}
impl core::fmt::Debug for ZkX509ShaSegmentPrivateEndpointV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZkX509ShaSegmentPrivateEndpointV1 { <private products redacted> }")
    }
}
impl Drop for ZkX509ShaSegmentPrivateEndpointV1 {
    fn drop(&mut self) {
        zeroize_field_rows_v1(&mut self.rfc_stream_products);
    }
}
/// Terminal material emitted while one physical SHA segment is streamed.
///
/// Private boundary products are retained only for compact-CA calls physically owned by
/// this segment. Across the four canonical segments they form the exact
/// call-ordered set `16..=28`; callers must reject omissions and duplicates.
#[derive(Clone, PartialEq, Eq)]
pub struct ZkX509ShaSegmentAirTerminalsV1 {
    pub(crate) segment: ZkX509ShaSegmentPrivateEndpointV1,
    pub(crate) ca_call_boundaries: Vec<ZkX509ShaCallBoundaryTerminalV1>,
}
fn clear_ca_boundary_products_v1(values: &mut [ZkX509ShaCallBoundaryTerminalV1]) {
    for value in values {
        zeroize_fields_v1(&mut value.source_start_products);
        zeroize_fields_v1(&mut value.digest_start_products);
        zeroize_fields_v1(&mut value.source_products);
        zeroize_fields_v1(&mut value.digest_products);
    }
}
impl core::fmt::Debug for ZkX509ShaSegmentAirTerminalsV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZkX509ShaSegmentAirTerminalsV1 { <private products redacted> }")
    }
}
impl ZkX509ShaSegmentAirTerminalsV1 {
    fn into_segment_v1(mut self) -> ZkX509ShaSegmentPrivateEndpointV1 {
        core::mem::replace(
            &mut self.segment,
            ZkX509ShaSegmentPrivateEndpointV1 {
                segment: 0,
                rfc_stream_products: [[F::ZERO; 4]; 4],
            },
        )
    }
}
impl Drop for ZkX509ShaSegmentAirTerminalsV1 {
    fn drop(&mut self) {
        clear_ca_boundary_products_v1(&mut self.ca_call_boundaries);
    }
}
#[derive(PartialEq, Eq)]
#[expect(
    clippy::struct_field_names,
    reason = "Each field names a distinct committed bus product carried between segments."
)]
struct ZkX509ShaSegmentProductStateV1 {
    projection_products: [F; ZK_X509_SHA_BUS_LANES_V1],
    source_products: [F; ZK_X509_SHA_BUS_LANES_V1],
    digest_products: [F; ZK_X509_SHA_BUS_LANES_V1],
    rfc_stream_products: [[F; ZK_X509_SHA_BUS_LANES_V1]; ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1],
}
impl ZkX509ShaSegmentProductStateV1 {
    const fn one_v1() -> Self {
        Self {
            projection_products: [F::ONE; ZK_X509_SHA_BUS_LANES_V1],
            source_products: [F::ONE; ZK_X509_SHA_BUS_LANES_V1],
            digest_products: [F::ONE; ZK_X509_SHA_BUS_LANES_V1],
            rfc_stream_products: [[F::ONE; ZK_X509_SHA_BUS_LANES_V1];
                ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1],
        }
    }
    fn copy_from_v1(&mut self, source: &Self) {
        self.projection_products
            .copy_from_slice(&source.projection_products);
        self.source_products
            .copy_from_slice(&source.source_products);
        self.digest_products
            .copy_from_slice(&source.digest_products);
        for (target, source) in self
            .rfc_stream_products
            .iter_mut()
            .zip(&source.rfc_stream_products)
        {
            target.copy_from_slice(source);
        }
    }
    fn zeroize_private_v1(&mut self) {
        zeroize_fields_v1(&mut self.projection_products);
        zeroize_fields_v1(&mut self.source_products);
        zeroize_fields_v1(&mut self.digest_products);
        zeroize_field_rows_v1(&mut self.rfc_stream_products);
    }
    fn terminal_v1(
        &self,
        segment: usize,
    ) -> Result<ZkX509ShaSegmentPrivateEndpointV1, ZkX509ShaCallBusStarkErrorV1> {
        let segment = u8::try_from(segment).map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?;
        if usize::from(segment) >= ZK_X509_SHA_SEGMENT_COUNT_V1 {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        let mut owner = ZkX509ShaSegmentPrivateEndpointV1 {
            segment,
            rfc_stream_products: [[F::ZERO; 4]; 4],
        };
        for (target, source) in owner
            .rfc_stream_products
            .iter_mut()
            .zip(&self.rfc_stream_products)
        {
            target.copy_from_slice(source);
        }
        Ok(owner)
    }
}
impl Clone for ZkX509ShaSegmentProductStateV1 {
    fn clone(&self) -> Self {
        let mut owner = Self::one_v1();
        owner.copy_from_v1(self);
        owner
    }
}
impl core::fmt::Debug for ZkX509ShaSegmentProductStateV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZkX509ShaSegmentProductStateV1 { <private products redacted> }")
    }
}
impl Drop for ZkX509ShaSegmentProductStateV1 {
    fn drop(&mut self) {
        self.zeroize_private_v1();
    }
}
/// Explicit row and factor storage owned throughout fallible call recurrence work.
/// Drop clears these cells, not compiler-generated copies, registers or spills.
struct ShaCallBindingScratchV1 {
    base: [F; ZK_X509_SHA_BATCH_BASE_WIDTH_V1],
    factor: F,
}
impl ShaCallBindingScratchV1 {
    const fn zero_v1() -> Self {
        Self {
            base: [F::ZERO; ZK_X509_SHA_BATCH_BASE_WIDTH_V1],
            factor: F::ZERO,
        }
    }
    fn set_factor_v1(&mut self, mut value: Option<F>) -> bool {
        let Some(value) = value.as_mut() else {
            return false;
        };
        self.factor = *value;
        value.zeroize_v1();
        true
    }
}
impl core::fmt::Debug for ShaCallBindingScratchV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ShaCallBindingScratchV1 { <private row redacted> }")
    }
}
impl Drop for ShaCallBindingScratchV1 {
    fn drop(&mut self) {
        zeroize_fields_v1(&mut self.base);
        zeroize_fields_v1(core::slice::from_mut(&mut self.factor));
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Public-shape RFC consumer address shared by native and algebraic fixed schedules.
pub struct ZkX509ShaRfcConsumerChannelsV1 {
    /// Typed RFC output family.
    pub(crate) role: ZkX509Rfc5280OutputRoleV1,
    /// Canonical strict-DER message channel.
    pub(crate) message_channel: u32,
    /// Canonical raw-length channel, when the message length is variable.
    pub(crate) length_channel: Option<u32>,
    /// Byte offset of the RFC-owned raw value in the SHA preimage.
    pub(crate) message_prefix_bytes: usize,
    /// Fixed RFC channel capacity, not the private exact length.
    pub(crate) message_capacity_bytes: usize,
}
/// One challenge-independent fixed-capacity SHA call.
///
/// Base and fixed rows may be streamed before X5B1 exists. The word-memory, call-bus, RFC products,
/// and terminal claims are absent until this source is consumed by the segment binder.
pub struct ZkX509ShaBatchCallBaseSourceV1 {
    pub(crate) manifest: ZkX509ShaCallManifestV1,
    word: ZkX509ShaWordCapacityBaseSourceV1,
    rfc_consumer: Option<ZkX509ShaRfcConsumerChannelsV1>,
}
impl core::fmt::Debug for ZkX509ShaBatchCallBaseSourceV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkX509ShaBatchCallBaseSourceV1")
            .field("manifest", &self.manifest)
            .field("private_material", &"<redacted>")
            .finish_non_exhaustive()
    }
}
impl ZkX509ShaBatchCallBaseSourceV1 {
    /// Fixed logical row count for this call.
    pub(crate) const fn logical_rows(&self) -> usize {
        self.word.logical_rows()
    }
    /// Widen one challenge-independent word row with the private RFC length
    /// decomposition committed in the SHA base group.
    pub(crate) fn base_row(
        &self,
        index: usize,
    ) -> Result<[F; ZK_X509_SHA_BATCH_BASE_WIDTH_V1], ZkX509ShaCallBusStarkErrorV1> {
        widened_sha_batch_base_row_v1(&self.word, self.rfc_consumer, index)
    }
    /// Borrow the canonical word row after the original RFC length admission.
    /// The returned owner clears its stored private scalar when dropped.
    fn borrowed_base_row_v1(
        &self,
        index: usize,
    ) -> Result<
        (
            &[F; SHA_WORD_CAPACITY_BASE_WIDTH_V1],
            zeroize::Zeroizing<Option<usize>>,
        ),
        ZkX509ShaCallBusStarkErrorV1,
    > {
        let raw_length = zeroize::Zeroizing::new(if let Some(consumer) = self.rfc_consumer {
            Some(
                self.word
                    .message_len()
                    .checked_sub(consumer.message_prefix_bytes)
                    .filter(|length| *length <= consumer.message_capacity_bytes)
                    .ok_or(ZkX509ShaCallBusStarkErrorV1::LengthOrPadding)?,
            )
        } else {
            None
        });
        let word_base = self.word.base_row(index)?;
        Ok((word_base, raw_length))
    }
    /// Emit an already-admitted adjacent public range after complete row admission.
    fn emit_base_columns_v1(
        &self,
        index: usize,
        first_column: usize,
        width: usize,
        mut emit: impl FnMut(usize, F),
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        // The segment caller admits this public extent before taking any private
        // row or output custody. The assertion also protects internal test calls.
        debug_assert!(
            width > 0
                && width
                    <= crate::privacy_engines::aggregate_stark::MASKED_TRACE_LDE_COLUMN_BATCH_V1
                && first_column
                    .checked_add(width)
                    .is_some_and(|end| end <= ZK_X509_SHA_BATCH_BASE_WIDTH_V1)
        );
        self.validate_fixed_row_shape_v1(index)?;
        let (word_base, raw_length) = self.borrowed_base_row_v1(index)?;
        for offset in 0..width {
            let column = first_column + offset;
            let value = if column < SHA_WORD_CAPACITY_BASE_WIDTH_V1 {
                word_base[column]
            } else if let Some(raw_length) = raw_length.as_ref() {
                let bit = column - ZK_X509_SHA_LENGTH_BITS_START_V1;
                F(u64::from(((*raw_length >> bit) & 1) != 0))
            } else {
                F::ZERO
            };
            emit(offset, value);
        }
        Ok(())
    }
    /// Reconstruct one verifier-fixed call row without any challenge.
    #[cfg(test)]
    pub(crate) fn fixed_row(
        &self,
        index: usize,
    ) -> Result<[F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1], ZkX509ShaCallBusStarkErrorV1> {
        let mut fixed =
            widened_sha_batch_fixed_row_v1(self.manifest, &self.word, self.rfc_consumer, index)?;
        fixed[ZK_X509_SHA_FIXED_SEGMENT_FIRST_V1] = F(u64::from(index == 0));
        fixed[ZK_X509_SHA_FIXED_SEGMENT_LAST_V1] = F(u64::from(index + 1 == self.logical_rows()));
        Ok(fixed)
    }
    /// Admit the original fixed row without constructing discarded fixed output.
    fn validate_fixed_row_shape_v1(
        &self,
        index: usize,
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        emit_sha_batch_fixed_row_from_word_v1(
            self.manifest,
            self.word.fixed_row(index)?,
            self.word.logical_rows(),
            self.rfc_consumer,
            index,
            |_, _| {},
        )
    }
    /// Consume this call's base phase using only challenges carried by X5B1.
    #[cfg(test)]
    pub(crate) fn bind_v1(
        self,
        binding: &ZkX509CredentialPreAuxBindingV1,
    ) -> Result<ZkX509ShaBatchCallTraceV1, ZkX509ShaCallBusStarkErrorV1> {
        bind_zk_x509_sha_batch_call_base_with_initial_products_v1(
            self,
            binding,
            &ZkX509ShaSegmentProductStateV1::one_v1(),
        )
    }
    /// Recursively clear all message-derived word material.
    #[cfg(test)]
    pub(crate) fn zeroize_private_v1(&mut self) {
        self.word.zeroize_private_v1();
    }
    #[cfg(test)]
    pub(crate) fn private_is_zeroized_v1(&self) -> bool {
        self.word.private_is_zeroized_v1()
    }
}
/// One streamed fixed-capacity call.  At most one instance is retained while
/// the four physical segment commitments are built.
#[derive(Clone)]
pub struct ZkX509ShaBatchCallTraceV1 {
    pub(crate) manifest: ZkX509ShaCallManifestV1,
    pub(crate) word: ZkX509ShaWordCapacityTraceV1,
    /// Only the call and RFC product fields are retained separately. The word
    /// auxiliary and fixed fields remain in `word` and are widened into an
    /// opened row on demand, avoiding duplicate full-width vectors.
    product_rows: Vec<[F; ZK_X509_SHA_PRODUCT_WIDTH_V1]>,
    #[cfg(test)]
    rfc_consumer: Option<ZkX509ShaRfcConsumerChannelsV1>,
    pub(crate) terminal: ZkX509ShaCallTerminalV1,
    pub(crate) rfc_terminal: ZkX509ShaRfcConsumerTerminalV1,
    segment_product_state: ZkX509ShaSegmentProductStateV1,
}
impl core::fmt::Debug for ZkX509ShaBatchCallTraceV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkX509ShaBatchCallTraceV1")
            .field("manifest", &self.manifest)
            .field("private_material", &"<redacted>")
            .finish_non_exhaustive()
    }
}
impl ZkX509ShaBatchCallTraceV1 {
    fn zeroize_private_cells_v1(&mut self) {
        for value in self
            .product_rows
            .iter_mut()
            .flatten()
            .chain(&mut self.terminal.source_products)
            .chain(&mut self.terminal.digest_products)
            .chain(self.rfc_terminal.stream_products.iter_mut().flatten())
        {
            value.zeroize_v1();
        }
        self.segment_product_state.zeroize_private_v1();
    }
    pub(crate) fn zeroize_private_v1(&mut self) {
        self.zeroize_private_cells_v1();
        self.product_rows.clear();
        self.word.zeroize_private_v1();
    }
    #[cfg(test)]
    pub(crate) fn private_is_zeroized_v1(&self) -> bool {
        self.product_rows.is_empty()
            && self.terminal.source_products == [F::ZERO; ZK_X509_SHA_BUS_LANES_V1]
            && self.terminal.digest_products == [F::ZERO; ZK_X509_SHA_BUS_LANES_V1]
            && self
                .rfc_terminal
                .stream_products
                .iter()
                .flatten()
                .all(|value| *value == F::ZERO)
            && self
                .segment_product_state
                .source_products
                .iter()
                .chain(&self.segment_product_state.digest_products)
                .chain(&self.segment_product_state.projection_products)
                .chain(
                    self.segment_product_state
                        .rfc_stream_products
                        .iter()
                        .flatten(),
                )
                .all(|value| *value == F::ZERO)
            && self.word.private_is_zeroized_v1()
    }
}
impl Drop for ZkX509ShaBatchCallTraceV1 {
    fn drop(&mut self) {
        self.zeroize_private_v1();
    }
}
trait ShaWordCapacityBaseRowsV1 {
    fn message_len_v1(&self) -> usize;
    fn logical_rows_v1(&self) -> usize;
    fn base_row_v1(
        &self,
        index: usize,
    ) -> Result<&[F; SHA_WORD_CAPACITY_BASE_WIDTH_V1], ZkX509ShaWordStarkErrorV1>;
    fn fixed_row_v1(
        &self,
        index: usize,
    ) -> Result<&[F; SHA_WORD_CAPACITY_FIXED_WIDTH_V1], ZkX509ShaWordStarkErrorV1>;
}
impl ShaWordCapacityBaseRowsV1 for ZkX509ShaWordCapacityBaseSourceV1 {
    fn message_len_v1(&self) -> usize {
        self.message_len()
    }
    fn logical_rows_v1(&self) -> usize {
        self.logical_rows()
    }
    fn base_row_v1(
        &self,
        index: usize,
    ) -> Result<&[F; SHA_WORD_CAPACITY_BASE_WIDTH_V1], ZkX509ShaWordStarkErrorV1> {
        self.base_row(index)
    }
    fn fixed_row_v1(
        &self,
        index: usize,
    ) -> Result<&[F; SHA_WORD_CAPACITY_FIXED_WIDTH_V1], ZkX509ShaWordStarkErrorV1> {
        self.fixed_row(index)
    }
}
impl ShaWordCapacityBaseRowsV1 for ZkX509ShaWordCapacityTraceV1 {
    fn message_len_v1(&self) -> usize {
        self.message_len
    }
    fn logical_rows_v1(&self) -> usize {
        self.logical_rows()
    }
    fn base_row_v1(
        &self,
        index: usize,
    ) -> Result<&[F; SHA_WORD_CAPACITY_BASE_WIDTH_V1], ZkX509ShaWordStarkErrorV1> {
        self.base_row(index)
    }
    fn fixed_row_v1(
        &self,
        index: usize,
    ) -> Result<&[F; SHA_WORD_CAPACITY_FIXED_WIDTH_V1], ZkX509ShaWordStarkErrorV1> {
        self.fixed_row(index)
    }
}
fn widened_sha_batch_base_row_v1<Word: ShaWordCapacityBaseRowsV1>(
    word: &Word,
    consumer: Option<ZkX509ShaRfcConsumerChannelsV1>,
    index: usize,
) -> Result<[F; ZK_X509_SHA_BATCH_BASE_WIDTH_V1], ZkX509ShaCallBusStarkErrorV1> {
    // Reject fallible public-shape checks before copying any private row.
    let raw_length = sha_batch_raw_length_v1(word.message_len_v1(), consumer)?;
    let word_base = word.base_row_v1(index)?;
    let mut base = [F::ZERO; ZK_X509_SHA_BATCH_BASE_WIDTH_V1];
    base[..SHA_WORD_CAPACITY_BASE_WIDTH_V1].copy_from_slice(word_base);
    if let Some(raw_length) = raw_length {
        for bit in 0..ZK_X509_SHA_RFC_LENGTH_BITS_V1 {
            base[ZK_X509_SHA_LENGTH_BITS_START_V1 + bit] =
                F(u64::from(((raw_length >> bit) & 1) != 0));
        }
    }
    Ok(base)
}
fn sha_batch_raw_length_v1(
    message_len: usize,
    consumer: Option<ZkX509ShaRfcConsumerChannelsV1>,
) -> Result<Option<usize>, ZkX509ShaCallBusStarkErrorV1> {
    consumer
        .map(|consumer| {
            message_len
                .checked_sub(consumer.message_prefix_bytes)
                .filter(|length| *length <= consumer.message_capacity_bytes)
                .ok_or(ZkX509ShaCallBusStarkErrorV1::LengthOrPadding)
        })
        .transpose()
}
fn widened_sha_batch_fixed_row_v1<Word: ShaWordCapacityBaseRowsV1>(
    manifest: ZkX509ShaCallManifestV1,
    word: &Word,
    consumer: Option<ZkX509ShaRfcConsumerChannelsV1>,
    index: usize,
) -> Result<[F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1], ZkX509ShaCallBusStarkErrorV1> {
    widened_sha_batch_fixed_row_from_word_v1(
        manifest,
        word.fixed_row_v1(index)?,
        word.logical_rows_v1(),
        consumer,
        index,
    )
}
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
fn write_sha_rfc_fixed_event_v1(
    fixed: &mut [F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
    consumer: ZkX509ShaRfcConsumerChannelsV1,
    stream: usize,
    message: bool,
    length_high: bool,
    length_low: bool,
    channel: u32,
    offset: usize,
) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
    emit_sha_rfc_fixed_event_v1(
        &mut |column, value| fixed[column] = value,
        consumer,
        stream,
        message,
        length_high,
        length_low,
        channel,
        offset,
    )
}
#[allow(clippy::too_many_arguments)]
fn emit_sha_rfc_fixed_event_v1(
    emit: &mut impl FnMut(usize, F),
    consumer: ZkX509ShaRfcConsumerChannelsV1,
    stream: usize,
    message: bool,
    length_high: bool,
    length_low: bool,
    channel: u32,
    offset: usize,
) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
    if stream >= ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1 {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    let start = ZK_X509_SHA_FIXED_RFC_STREAMS_V1
        .checked_add(
            stream
                .checked_mul(ZK_X509_SHA_FIXED_RFC_STREAM_STRIDE_V1)
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?,
        )
        .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
    emit(
        start + ZK_X509_SHA_FIXED_RFC_MESSAGE_EVENT_V1,
        F(u64::from(message)),
    );
    emit(
        start + ZK_X509_SHA_FIXED_RFC_LENGTH_HIGH_VALUE_V1,
        F(u64::from(length_high)),
    );
    emit(
        start + ZK_X509_SHA_FIXED_RFC_LENGTH_LOW_VALUE_V1,
        F(u64::from(length_low)),
    );
    emit(
        start + ZK_X509_SHA_FIXED_RFC_ROLE_V1,
        F(consumer.role as u64),
    );
    emit(
        start + ZK_X509_SHA_FIXED_RFC_CHANNEL_V1,
        F(u64::from(channel)),
    );
    emit(
        start + ZK_X509_SHA_FIXED_RFC_OFFSET_V1,
        F(u64::try_from(offset).map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?),
    );
    Ok(())
}
fn widened_sha_batch_fixed_row_from_word_v1(
    manifest: ZkX509ShaCallManifestV1,
    word_fixed: &[F; SHA_WORD_CAPACITY_FIXED_WIDTH_V1],
    word_logical_rows: usize,
    consumer: Option<ZkX509ShaRfcConsumerChannelsV1>,
    index: usize,
) -> Result<[F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1], ZkX509ShaCallBusStarkErrorV1> {
    let mut fixed = [F::ZERO; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1];
    emit_sha_batch_fixed_row_from_word_v1(
        manifest,
        word_fixed,
        word_logical_rows,
        consumer,
        index,
        |column, value| fixed[column] = value,
    )?;
    Ok(fixed)
}
/// Check the complete original fixed-row geometry while optionally emitting its public cells.
fn emit_sha_batch_fixed_row_from_word_v1(
    manifest: ZkX509ShaCallManifestV1,
    word_fixed: &[F; SHA_WORD_CAPACITY_FIXED_WIDTH_V1],
    word_logical_rows: usize,
    consumer: Option<ZkX509ShaRfcConsumerChannelsV1>,
    index: usize,
    mut emit: impl FnMut(usize, F),
) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
    if index >= word_logical_rows {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    for (column, value) in word_fixed.iter().copied().enumerate() {
        emit(column, value);
    }
    emit(ZK_X509_SHA_FIXED_CALL_V1, F(u64::from(manifest.call)));
    emit(
        ZK_X509_SHA_FIXED_ROLE_V1,
        F(u64::from(manifest.role.role_code())),
    );
    emit(
        ZK_X509_SHA_FIXED_SLOT_V1,
        F(u64::from(manifest.role.slot())),
    );
    if let Some(selector) = usize::from(manifest.call)
        .checked_sub(ZK_X509_SHA_CA_LEAF_CALL_V1)
        .filter(|selector| *selector < ZK_X509_SHA_CA_CALL_COUNT_V1)
    {
        emit(ZK_X509_SHA_FIXED_CA_CALL_SELECTORS_V1 + selector, F::ONE);
    }
    if word_fixed[SHA_WORD_CAPACITY_INPUT_WORD_V1] == F::ONE {
        let word = usize::try_from(word_fixed[SHA_WORD_CAPACITY_INPUT_WORD_INDEX_V1].0)
            .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?;
        for byte in 0..4 {
            let offset = word
                .checked_mul(4)
                .and_then(|start| start.checked_add(byte))
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            if let Some(expected) = sha_frame_byte_v1(manifest, offset) {
                emit(ZK_X509_SHA_FIXED_FRAME_MASK_V1 + byte, F::ONE);
                match expected {
                    ShaFrameByteV1::Constant(value) => emit(
                        ZK_X509_SHA_FIXED_FRAME_CONSTANT_V1 + byte,
                        F(u64::from(value)),
                    ),
                    ShaFrameByteV1::RawLengthHigh => {
                        emit(ZK_X509_SHA_FIXED_FRAME_LENGTH_HIGH_V1 + byte, F::ONE)
                    }
                    ShaFrameByteV1::RawLengthLow => {
                        emit(ZK_X509_SHA_FIXED_FRAME_LENGTH_LOW_V1 + byte, F::ONE)
                    }
                }
            }
        }
    }
    projection_bus::emit_fixed_v1(manifest, word_fixed, &mut emit);
    let Some(consumer) = consumer else {
        return Ok(());
    };
    emit(
        ZK_X509_SHA_FIXED_RFC_LENGTH_PREFIX_V1,
        F(u64::try_from(consumer.message_prefix_bytes)
            .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?),
    );
    if word_fixed[SHA_WORD_CAPACITY_INPUT_WORD_V1] == F::ONE {
        let word = usize::try_from(word_fixed[SHA_WORD_CAPACITY_INPUT_WORD_INDEX_V1].0)
            .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?;
        let raw_end = consumer
            .message_prefix_bytes
            .checked_add(consumer.message_capacity_bytes)
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
        for stream in 0..ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1 {
            let sha_offset = word
                .checked_mul(4)
                .and_then(|offset| offset.checked_add(stream))
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            if (consumer.message_prefix_bytes..raw_end).contains(&sha_offset) {
                emit_sha_rfc_fixed_event_v1(
                    &mut emit,
                    consumer,
                    stream,
                    true,
                    false,
                    false,
                    consumer.message_channel,
                    sha_offset
                        .checked_sub(consumer.message_prefix_bytes)
                        .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?,
                )?;
            }
        }
    }
    if let Some(length_channel) = consumer.length_channel
        && index + 4 >= word_logical_rows
    {
        let pair = index + 4 - word_logical_rows;
        emit(ZK_X509_SHA_FIXED_RFC_LENGTH_PAIR_V1, F::ONE);
        emit(
            ZK_X509_SHA_FIXED_RFC_LENGTH_PAIR_INDEX_V1,
            F(u64::try_from(pair).map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?),
        );
        for stream in 0..2 {
            let offset = pair
                .checked_mul(2)
                .and_then(|offset| offset.checked_add(stream))
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            emit_sha_rfc_fixed_event_v1(
                &mut emit,
                consumer,
                stream,
                false,
                offset == 6,
                offset == 7,
                length_channel,
                offset,
            )?;
        }
    }
    Ok(())
}
impl ZkX509ShaBatchCallTraceV1 {
    #[cfg(test)]
    pub(crate) const fn logical_rows(&self) -> usize {
        self.word.logical_rows()
    }
    #[cfg(test)]
    pub(crate) fn base_row(
        &self,
        index: usize,
    ) -> Result<[F; ZK_X509_SHA_BATCH_BASE_WIDTH_V1], ZkX509ShaCallBusStarkErrorV1> {
        widened_sha_batch_base_row_v1(&self.word, self.rfc_consumer, index)
    }
    pub(crate) fn aux_row(
        &self,
        index: usize,
    ) -> Result<[F; ZK_X509_SHA_BATCH_AUX_WIDTH_V1], ZkX509ShaCallBusStarkErrorV1> {
        let mut aux = [F::ZERO; ZK_X509_SHA_BATCH_AUX_WIDTH_V1];
        aux[..SHA_WORD_CAPACITY_AUX_WIDTH_V1].copy_from_slice(self.word.aux_row(index)?);
        aux[SHA_WORD_CAPACITY_AUX_WIDTH_V1..].copy_from_slice(
            self.product_rows
                .get(index)
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)?,
        );
        Ok(aux)
    }
    #[cfg(test)]
    pub(crate) fn fixed_row(
        &self,
        index: usize,
    ) -> Result<[F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1], ZkX509ShaCallBusStarkErrorV1> {
        let mut fixed =
            widened_sha_batch_fixed_row_v1(self.manifest, &self.word, self.rfc_consumer, index)?;
        fixed[ZK_X509_SHA_FIXED_SEGMENT_FIRST_V1] = F(u64::from(index == 0));
        fixed[ZK_X509_SHA_FIXED_SEGMENT_LAST_V1] = F(u64::from(index + 1 == self.logical_rows()));
        Ok(fixed)
    }
    #[cfg(test)]
    pub(crate) fn row(
        &self,
        index: usize,
    ) -> Result<ZkX509ShaBatchRowV1, ZkX509ShaCallBusStarkErrorV1> {
        Ok(ZkX509ShaBatchRowV1 {
            base: self.base_row(index)?,
            aux: self.aux_row(index)?,
            fixed: self.fixed_row(index)?,
        })
    }
}
/// One canonical physical SHA batch row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkX509ShaBatchRowV1<A = F> {
    pub(crate) base: [A; ZK_X509_SHA_BATCH_BASE_WIDTH_V1],
    pub(crate) aux: [A; ZK_X509_SHA_BATCH_AUX_WIDTH_V1],
    pub(crate) fixed: [A; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
}
/// Compact, deterministic maximum schedule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZkX509ShaCallScheduleV1 {
    shape: ZkX509ShaCallPublicShapeV1,
    calls: [ZkX509ShaCallManifestV1; ZK_X509_SHA_CALL_COUNT_V1],
}
/// SHA-call schedule, witness, bus, or resource failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[doc(hidden)]
pub enum ZkX509ShaCallBusStarkErrorV1 {
    /// Public shape or fixed role order is invalid.
    #[error("zk-X509 SHA call topology is invalid")]
    Topology,
    /// Private message length or exact SHA padding is invalid.
    #[error("zk-X509 SHA call private length or padding is invalid")]
    LengthOrPadding,
    /// A canonical inactive call is not the sole SHA-256 empty-message dummy.
    #[error("zk-X509 inactive SHA call is not canonical")]
    InactiveCall,
    /// A call digest does not hash its exact private message.
    #[error("zk-X509 SHA call digest is invalid")]
    Digest,
    /// An address/value event is malformed.
    #[cfg(test)]
    #[error("zk-X509 SHA call event is invalid")]
    Event,
    /// Fiat-Shamir challenges are zero, repeated, or non-canonical.
    #[error("zk-X509 SHA call-bus challenges are invalid")]
    Challenge,
    /// The one-shot base-to-auxiliary phase transition is missing or repeated.
    #[error("zk-X509 SHA trace phase transition is invalid")]
    Phase,
    /// Producer and SHA-consumer products do not match.
    #[error("zk-X509 SHA call-bus terminal mismatch")]
    Terminal,
    /// Checked arithmetic or allocation exceeded the release envelope.
    #[error("zk-X509 SHA call resource envelope is exceeded")]
    Resource,
}
impl From<ZkX509ShaWordStarkErrorV1> for ZkX509ShaCallBusStarkErrorV1 {
    fn from(error: ZkX509ShaWordStarkErrorV1) -> Self {
        match error {
            ZkX509ShaWordStarkErrorV1::Resource => Self::Resource,
            _ => Self::Topology,
        }
    }
}
fn padded_blocks_v1(message_len: usize) -> Result<usize, ZkX509ShaCallBusStarkErrorV1> {
    message_len
        .checked_add(9)
        .and_then(|length| length.checked_add(SHA_BLOCK_BYTES_V1 - 1))
        .map(|length| length / SHA_BLOCK_BYTES_V1)
        .filter(|blocks| *blocks != 0)
        .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)
}
fn maximum_blocks_v1(maximum_message_bytes: usize) -> Result<usize, ZkX509ShaCallBusStarkErrorV1> {
    padded_blocks_v1(maximum_message_bytes)
}
fn manifest_role_v1(call: usize) -> Result<ZkX509ShaCallRoleV1, ZkX509ShaCallBusStarkErrorV1> {
    match call {
        CERTIFICATE_TBS_CALL_START_V1..=2 => Ok(ZkX509ShaCallRoleV1::CertificateTbs(
            u8::try_from(call).map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?,
        )),
        CRL_TBS_CALL_V1 => Ok(ZkX509ShaCallRoleV1::CrlTbs),
        CRL_COMMITMENT_CALL_V1 => Ok(ZkX509ShaCallRoleV1::CrlCommitment),
        PROJECTION_CALL_START_V1..=11 => Ok(ZkX509ShaCallRoleV1::Projection(
            u8::try_from(call - PROJECTION_CALL_START_V1)
                .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?,
        )),
        CRL_ISSUER_SPKI_CALL_V1 => Ok(ZkX509ShaCallRoleV1::CrlIssuerSpki),
        TRUST_ANCHOR_RECORD_CALL_V1 => Ok(ZkX509ShaCallRoleV1::TrustAnchorRecord),
        CERTIFICATE_POLICY_RECORD_CALL_V1 => Ok(ZkX509ShaCallRoleV1::CertificatePolicyRecord),
        CRL_RECORD_CALL_V1 => Ok(ZkX509ShaCallRoleV1::CrlRecord),
        ZK_X509_SHA_CA_LEAF_CALL_V1 => Ok(ZkX509ShaCallRoleV1::CaLeaf),
        ZK_X509_SHA_CA_NODE_CALL_START_V1..=28 => Ok(ZkX509ShaCallRoleV1::CaNode(
            u8::try_from(call - ZK_X509_SHA_CA_NODE_CALL_START_V1)
                .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?,
        )),
        _ => Err(ZkX509ShaCallBusStarkErrorV1::Topology),
    }
}
/// Return the verifier-owned identity for one reference compact-CA boundary fixture.
#[cfg(test)]
pub fn zk_x509_sha_ca_call_identity_v1(
    index: usize,
) -> Result<(u8, ZkX509ShaCallRoleV1), ZkX509ShaCallBusStarkErrorV1> {
    let call = ZK_X509_SHA_CA_LEAF_CALL_V1
        .checked_add(index)
        .filter(|_| index < ZK_X509_SHA_CA_CALL_COUNT_V1)
        .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)?;
    Ok((
        u8::try_from(call).map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?,
        manifest_role_v1(call)?,
    ))
}
fn maximum_message_bytes_v1(
    role: ZkX509ShaCallRoleV1,
) -> Result<usize, ZkX509ShaCallBusStarkErrorV1> {
    match role {
        ZkX509ShaCallRoleV1::CertificateTbs(_) | ZkX509ShaCallRoleV1::CrlTbs => {
            Ok(ZK_X509_SHA_CALL_MAX_DER_BYTES_V1)
        }
        ZkX509ShaCallRoleV1::CrlCommitment => {
            let maximum_der = vec![0_u8; ZK_X509_CRL_COMMITMENT_MAX_DER_BYTES_V1];
            crl_commitment_preimage_v1(&maximum_der)
                .map(|message| message.len())
                .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Topology)
        }
        ZkX509ShaCallRoleV1::Projection(_) => Ok(ZK_X509_PROJECTION_HASH_BUFFER_BYTES_V1),
        ZkX509ShaCallRoleV1::CrlIssuerSpki => {
            let spki = [0_u8; ZK_X509_CA_SPKI_DER_BYTES_V1];
            crl_issuer_spki_preimage_v1(&spki)
                .map(|message| message.len())
                .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Topology)
        }
        ZkX509ShaCallRoleV1::TrustAnchorRecord => Ok(ZK_X509_TRUST_ANCHOR_RECORD_PREIMAGE_BYTES_V1),
        ZkX509ShaCallRoleV1::CertificatePolicyRecord => {
            Ok(ZK_X509_CERTIFICATE_POLICY_RECORD_MAX_PREIMAGE_BYTES_V1)
        }
        ZkX509ShaCallRoleV1::CrlRecord => Ok(ZK_X509_CRL_RECORD_PREIMAGE_BYTES_V1),
        ZkX509ShaCallRoleV1::CaLeaf => {
            let spki = [0_u8; ZK_X509_CA_SPKI_DER_BYTES_V1];
            ca_leaf_preimage_v1(&spki)
                .map(|message| message.len())
                .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Topology)
        }
        ZkX509ShaCallRoleV1::CaNode(level) => {
            ca_node_preimage_v1(usize::from(level), &[0_u8; 32], &[0_u8; 32])
                .map(|message| message.len())
                .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Topology)
        }
    }
}
fn role_has_exact_message_length_v1(role: ZkX509ShaCallRoleV1) -> bool {
    matches!(
        role,
        ZkX509ShaCallRoleV1::CrlIssuerSpki
            | ZkX509ShaCallRoleV1::TrustAnchorRecord
            | ZkX509ShaCallRoleV1::CrlRecord
            | ZkX509ShaCallRoleV1::CaLeaf
            | ZkX509ShaCallRoleV1::CaNode(_)
    )
}
fn role_activation_v1(
    role: ZkX509ShaCallRoleV1,
    shape: ZkX509ShaCallPublicShapeV1,
) -> ZkX509ShaCallActivationV1 {
    match role {
        ZkX509ShaCallRoleV1::CertificateTbs(2) => ZkX509ShaCallActivationV1::OptionalPrivate,
        ZkX509ShaCallRoleV1::Projection(slot) if (2..=5).contains(&slot) => {
            if usize::from(slot - 2) < shape.disclosed_attributes {
                ZkX509ShaCallActivationV1::Required
            } else {
                ZkX509ShaCallActivationV1::Inactive
            }
        }
        _ => ZkX509ShaCallActivationV1::Required,
    }
}
impl ZkX509ShaCallScheduleV1 {
    /// Compile the sole verifier-owned maximum schedule.
    pub(crate) fn new(
        shape: ZkX509ShaCallPublicShapeV1,
    ) -> Result<Self, ZkX509ShaCallBusStarkErrorV1> {
        if shape.disclosed_attributes > 4 {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        let mut first_event = 0_usize;
        let mut logical_rows = 0_usize;
        let mut maximum_blocks = 0_usize;
        let mut compiled = Vec::new();
        compiled
            .try_reserve_exact(ZK_X509_SHA_CALL_COUNT_V1)
            .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?;
        for call in 0..ZK_X509_SHA_CALL_COUNT_V1 {
            let role = manifest_role_v1(call)?;
            let maximum_message_bytes = maximum_message_bytes_v1(role)?;
            let call_blocks = maximum_blocks_v1(maximum_message_bytes)?;
            let maximum_local_rows = call_blocks
                .checked_mul(ZK_X509_SHA_LOCAL_ROWS_PER_BLOCK_V1)
                .and_then(|rows| rows.checked_add(ZK_X509_SHA_LOCAL_ROWS_PER_CALL_V1))
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            let maximum_memory_rows = call_blocks
                .checked_mul(ZK_X509_SHA_MEMORY_ROWS_PER_BLOCK_V1)
                .and_then(|rows| rows.checked_add(ZK_X509_SHA_MEMORY_ROWS_PER_CALL_V1))
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            let manifest = ZkX509ShaCallManifestV1 {
                call: u8::try_from(call).map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?,
                role,
                activation: role_activation_v1(role, shape),
                maximum_message_bytes,
                maximum_blocks: call_blocks,
                first_event,
                first_logical_row: usize::MAX,
                maximum_local_rows,
                maximum_memory_rows,
            };
            first_event = first_event
                .checked_add(manifest.maximum_events())
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            maximum_blocks = maximum_blocks
                .checked_add(call_blocks)
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            logical_rows = logical_rows
                .checked_add(manifest.maximum_logical_rows())
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            compiled.push(manifest);
        }
        let mut calls: [ZkX509ShaCallManifestV1; ZK_X509_SHA_CALL_COUNT_V1] = compiled
            .try_into()
            .map_err(
            |_: Vec<ZkX509ShaCallManifestV1>| ZkX509ShaCallBusStarkErrorV1::Topology,
        )?;
        let mut seen = [false; ZK_X509_SHA_CALL_COUNT_V1];
        let mut physical = 0_usize;
        for (segment, call_count) in ZK_X509_SHA_PHYSICAL_CALL_COUNTS_V1
            .iter()
            .copied()
            .enumerate()
        {
            let segment_start = segment
                .checked_mul(ZK_X509_SHA_SEGMENT_ROWS_V1)
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            let mut next_row = segment_start;
            for _ in 0..call_count {
                let call = usize::from(
                    *ZK_X509_SHA_PHYSICAL_CALL_ORDER_V1
                        .get(physical)
                        .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)?,
                );
                if call >= calls.len() || seen[call] {
                    return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
                }
                seen[call] = true;
                calls[call].first_logical_row = next_row;
                next_row = next_row
                    .checked_add(calls[call].maximum_logical_rows())
                    .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
                physical = physical
                    .checked_add(1)
                    .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            }
            if next_row
                .checked_sub(segment_start)
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)?
                != ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[segment]
                || next_row
                    > segment_start
                        .checked_add(ZK_X509_SHA_SEGMENT_ROWS_V1)
                        .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?
            {
                return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
            }
        }
        if first_event != ZK_X509_SHA_CALL_EVENT_COUNT_V1
            || logical_rows != ZK_X509_SHA_MAX_LOGICAL_ROWS_V1
            || maximum_blocks != ZK_X509_SHA_MAX_BLOCKS_V1
            || physical != ZK_X509_SHA_CALL_COUNT_V1
            || seen.iter().any(|seen| !seen)
            || calls
                .iter()
                .any(|call| call.first_logical_row == usize::MAX)
        {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        Ok(Self { shape, calls })
    }
    /// Public shape committed by this schedule.
    pub(crate) const fn shape(&self) -> ZkX509ShaCallPublicShapeV1 {
        self.shape
    }
    /// Ordered verifier-owned calls.
    pub(crate) const fn calls(&self) -> &[ZkX509ShaCallManifestV1; ZK_X509_SHA_CALL_COUNT_V1] {
        &self.calls
    }
    /// Resolve one canonical call.
    pub(crate) fn call(
        &self,
        call: usize,
    ) -> Result<ZkX509ShaCallManifestV1, ZkX509ShaCallBusStarkErrorV1> {
        self.calls
            .get(call)
            .copied()
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)
    }
    /// Resolve one maximum-capacity event row to its fixed address.
    #[cfg(test)]
    pub(crate) fn fixed_event(
        &self,
        index: usize,
    ) -> Result<(ZkX509ShaCallManifestV1, ZkX509ShaCallWordKindV1, u16), ZkX509ShaCallBusStarkErrorV1>
    {
        if index >= ZK_X509_SHA_CALL_EVENT_COUNT_V1 {
            return Err(ZkX509ShaCallBusStarkErrorV1::Event);
        }
        let call_index = self
            .calls
            .partition_point(|call| call.first_event <= index)
            .checked_sub(1)
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Event)?;
        let call = self.calls[call_index];
        let local = index
            .checked_sub(call.first_event)
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Event)?;
        let (kind, word) = if local < call.maximum_input_words() {
            (ZkX509ShaCallWordKindV1::Input, local)
        } else {
            (
                ZkX509ShaCallWordKindV1::Digest,
                local - call.maximum_input_words(),
            )
        };
        if kind == ZkX509ShaCallWordKindV1::Digest && word >= SHA_DIGEST_WORDS_V1 {
            return Err(ZkX509ShaCallBusStarkErrorV1::Event);
        }
        Ok((
            call,
            kind,
            u16::try_from(word).map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?,
        ))
    }
    /// Resolve one fixed-capacity logical word-AIR row.
    pub(crate) fn logical_row(
        &self,
        index: usize,
    ) -> Result<(ZkX509ShaCallManifestV1, usize), ZkX509ShaCallBusStarkErrorV1> {
        let segment = index / ZK_X509_SHA_SEGMENT_ROWS_V1;
        let segment_row = index % ZK_X509_SHA_SEGMENT_ROWS_V1;
        if segment >= ZK_X509_SHA_SEGMENT_COUNT_V1
            || segment_row >= ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[segment]
        {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        let physical_start = ZK_X509_SHA_PHYSICAL_CALL_COUNTS_V1[..segment]
            .iter()
            .try_fold(0_usize, |sum, count| {
                sum.checked_add(*count)
                    .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)
            })?;
        let physical_end = physical_start
            .checked_add(ZK_X509_SHA_PHYSICAL_CALL_COUNTS_V1[segment])
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
        for call in ZK_X509_SHA_PHYSICAL_CALL_ORDER_V1[physical_start..physical_end]
            .iter()
            .copied()
        {
            let call = self
                .calls
                .get(usize::from(call))
                .copied()
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)?;
            let end = call
                .first_logical_row
                .checked_add(call.maximum_logical_rows())
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            if (call.first_logical_row..end).contains(&index) {
                return Ok((
                    call,
                    index
                        .checked_sub(call.first_logical_row)
                        .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?,
                ));
            }
        }
        Err(ZkX509ShaCallBusStarkErrorV1::Topology)
    }
}
/// Independent-verifier provider for all four physical SHA fixed segments.
///
/// Every retained value is compiled from [`ZkX509ShaCallPublicShapeV1`]. Exact messages, private
/// lengths, selected digest states, and transcript challenges are deliberately absent.
#[derive(Clone, Debug)]
pub struct ZkX509ShaBatchFixedProviderV1 {
    schedule: ZkX509ShaCallScheduleV1,
    calls: Vec<ZkX509ShaWordCapacityFixedScheduleV1>,
}
impl ZkX509ShaBatchFixedProviderV1 {
    /// Forecast all retained fixed schedules without constructing any word trace.
    pub(crate) fn allocation_forecast_v1(
        shape: ZkX509ShaCallPublicShapeV1,
    ) -> Result<usize, ZkX509ShaCallBusStarkErrorV1> {
        use super::allocation_payload::sum_v1;
        use core::mem::size_of;
        let schedule = ZkX509ShaCallScheduleV1::new(shape)?;
        Ok(sum_v1([
            ZK_X509_SHA_CALL_COUNT_V1 * size_of::<ZkX509ShaWordCapacityFixedScheduleV1>(),
            sum_v1(schedule.calls().iter().map(|call| {
                call.maximum_logical_rows() * size_of::<super::sha_word_stark::ShaWordFixedRowV1>()
                    + call.maximum_input_words() * size_of::<(usize, usize)>()
            })),
        ]))
    }
    /// Maximum serial word-source scratch charged by the public call schedule.
    pub(crate) fn replay_scratch_forecast_v1(
        shape: ZkX509ShaCallPublicShapeV1,
    ) -> Result<usize, ZkX509ShaCallBusStarkErrorV1> {
        let schedule = ZkX509ShaCallScheduleV1::new(shape)?;
        let maximum_blocks = schedule
            .calls()
            .iter()
            .map(|call| call.maximum_blocks)
            .max()
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)?;
        Ok(super::sha_word_stark::source_replay_scratch_forecast_v1(
            maximum_blocks,
        )?)
    }
    /// Allocated public schedules for all calls, excluding this inline owner.
    pub(crate) fn allocated_heap_bytes_v1(&self) -> usize {
        use super::allocation_payload::{sum_v1, vector_v1};
        sum_v1([
            vector_v1(&self.calls),
            sum_v1(
                self.calls
                    .iter()
                    .map(ZkX509ShaWordCapacityFixedScheduleV1::allocated_heap_bytes_v1),
            ),
        ])
    }
    /// Compile the sole verifier-owned 29-call fixed topology.
    pub(crate) fn new_v1(
        shape: ZkX509ShaCallPublicShapeV1,
    ) -> Result<Self, ZkX509ShaCallBusStarkErrorV1> {
        let schedule = ZkX509ShaCallScheduleV1::new(shape)?;
        let mut calls = Vec::new();
        calls
            .try_reserve_exact(ZK_X509_SHA_CALL_COUNT_V1)
            .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?;
        for manifest in schedule.calls().iter().copied() {
            let fixed = compile_sha_word_capacity_fixed_schedule_v1(
                manifest.maximum_message_bytes,
                role_has_exact_message_length_v1(manifest.role),
            )?;
            if fixed.maximum_blocks() != manifest.maximum_blocks
                || fixed.maximum_local_rows() != manifest.maximum_local_rows
                || fixed.maximum_memory_rows() != manifest.maximum_memory_rows
                || fixed.logical_rows() != manifest.maximum_logical_rows()
            {
                return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
            }
            calls.push(fixed);
        }
        if calls.len() != ZK_X509_SHA_CALL_COUNT_V1 {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        Ok(Self { schedule, calls })
    }
    #[cfg(test)]
    pub(crate) const fn shape(&self) -> ZkX509ShaCallPublicShapeV1 {
        self.schedule.shape()
    }
    #[cfg(test)]
    pub(crate) const fn schedule(&self) -> &ZkX509ShaCallScheduleV1 {
        &self.schedule
    }
    /// Borrow one exact public call schedule for bound base replay. The caller
    /// already charges this provider; no schedule or private row owner is cloned.
    fn call_base_source_v1(
        &self,
        manifest: ZkX509ShaCallManifestV1,
        witness: &ZkX509ShaCallWitnessV1,
    ) -> Result<ZkX509ShaBatchCallBaseSourceV1, ZkX509ShaCallBusStarkErrorV1> {
        if self.schedule.call(usize::from(manifest.call))? != manifest {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        let fixed = self
            .calls
            .get(usize::from(manifest.call))
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)?;
        build_sha_batch_call_base_using_v1(
            manifest,
            witness,
            self.schedule.shape().disclosed_attributes,
            |message, maximum, exact| {
                build_sha_word_capacity_base_with_fixed_v1(message, maximum, exact, fixed)
            },
        )
    }
    /// Fill the caller's complete already-charged public fixed matrix once per row.
    /// No witness, challenge or additional retained schedule is accepted or stored.
    pub(crate) fn fill_fixed_matrix_v1(
        &self,
        segment: usize,
        columns: &mut [Vec<F>],
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        if segment >= ZK_X509_SHA_SEGMENT_COUNT_V1 {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        fill_sha_public_fixed_matrix_with_v1(ZK_X509_SHA_SEGMENT_ROWS_V1, columns, |row| {
            self.fixed_row_v1(segment, row)
        })
    }
    /// Reconstruct one verifier-preprocessed physical-segment row.
    pub(crate) fn fixed_row_v1(
        &self,
        segment: usize,
        segment_row: usize,
    ) -> Result<[F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1], ZkX509ShaCallBusStarkErrorV1> {
        if segment >= ZK_X509_SHA_SEGMENT_COUNT_V1 || segment_row >= ZK_X509_SHA_SEGMENT_ROWS_V1 {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        let global_row = segment
            .checked_mul(ZK_X509_SHA_SEGMENT_ROWS_V1)
            .and_then(|start| start.checked_add(segment_row))
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
        if segment_row >= ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[segment] {
            return Ok(physical_padding_row_v1(segment_row).fixed);
        }
        let (manifest, local_row) = self.schedule.logical_row(global_row)?;
        let word = self
            .calls
            .get(usize::from(manifest.call))
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)?;
        let consumer = sha_rfc_consumer_channels_v1(
            manifest.call,
            manifest.role,
            self.schedule.shape().disclosed_attributes,
        )?;
        let word_fixed = word.fixed_row_v1(local_row)?;
        let mut fixed = widened_sha_batch_fixed_row_from_word_v1(
            manifest,
            &word_fixed,
            word.logical_rows(),
            consumer,
            local_row,
        )?;
        fixed[ZK_X509_SHA_FIXED_SEGMENT_FIRST_V1] = F(u64::from(segment_row == 0));
        fixed[ZK_X509_SHA_FIXED_SEGMENT_LAST_V1] = F(u64::from(
            segment_row + 1 == ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[segment],
        ));
        Ok(fixed)
    }
}
/// Borrow the final public matrix; refusal or unwind clears every destination cell.
struct ShaPublicFixedMatrixDestinationV1<'a> {
    columns: &'a mut [Vec<F>],
    committed: bool,
}
impl Drop for ShaPublicFixedMatrixDestinationV1<'_> {
    fn drop(&mut self) {
        if !self.committed {
            for column in &mut *self.columns {
                zeroize_fields_v1(column);
            }
        }
    }
}
/// Transpose each canonical public row directly into the existing final columns.
/// Production supplies the fixed segment extent; small extents serve unit controls.
fn fill_sha_public_fixed_matrix_with_v1(
    rows: usize,
    columns: &mut [Vec<F>],
    mut fixed_row: impl FnMut(
        usize,
    ) -> Result<
        [F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
        ZkX509ShaCallBusStarkErrorV1,
    >,
) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
    if !rows.is_power_of_two()
        || rows > ZK_X509_SHA_SEGMENT_ROWS_V1
        || columns.len() != ZK_X509_SHA_BATCH_FIXED_WIDTH_V1
        || columns.iter().any(|column| column.len() != rows)
    {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    let mut destination = ShaPublicFixedMatrixDestinationV1 {
        columns,
        committed: false,
    };
    for row in 0..rows {
        let values = fixed_row(row)?;
        for (column, value) in destination.columns.iter_mut().zip(values) {
            column[row] = value;
        }
    }
    destination.committed = true;
    Ok(())
}
fn canonical_dummy_digest_v1() -> [u8; 32] {
    Sha256::digest([]).into()
}
fn validate_witness_v1(
    manifest: ZkX509ShaCallManifestV1,
    witness: &ZkX509ShaCallWitnessV1,
) -> Result<usize, ZkX509ShaCallBusStarkErrorV1> {
    if witness.role != manifest.role {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    match manifest.activation {
        ZkX509ShaCallActivationV1::Inactive => {
            if !witness.message.is_empty() || witness.digest != canonical_dummy_digest_v1() {
                return Err(ZkX509ShaCallBusStarkErrorV1::InactiveCall);
            }
        }
        ZkX509ShaCallActivationV1::OptionalPrivate if witness.message.is_empty() => {
            if witness.digest != canonical_dummy_digest_v1() {
                return Err(ZkX509ShaCallBusStarkErrorV1::InactiveCall);
            }
        }
        ZkX509ShaCallActivationV1::Required | ZkX509ShaCallActivationV1::OptionalPrivate => {
            if witness.message.is_empty() || witness.message.len() > manifest.maximum_message_bytes
            {
                return Err(ZkX509ShaCallBusStarkErrorV1::LengthOrPadding);
            }
        }
    }
    match manifest.role {
        ZkX509ShaCallRoleV1::CrlIssuerSpki
        | ZkX509ShaCallRoleV1::TrustAnchorRecord
        | ZkX509ShaCallRoleV1::CrlRecord
        | ZkX509ShaCallRoleV1::CaLeaf
        | ZkX509ShaCallRoleV1::CaNode(_)
            if witness.message.len() != manifest.maximum_message_bytes =>
        {
            return Err(ZkX509ShaCallBusStarkErrorV1::LengthOrPadding);
        }
        _ => {}
    }
    if witness.digest != <[u8; 32]>::from(Sha256::digest(&witness.message)) {
        return Err(ZkX509ShaCallBusStarkErrorV1::Digest);
    }
    let blocks = padded_blocks_v1(witness.message.len())?;
    if blocks > manifest.maximum_blocks {
        return Err(ZkX509ShaCallBusStarkErrorV1::LengthOrPadding);
    }
    Ok(blocks)
}
/// Validate the complete fixed 29-call witness array without replaying the
/// multi-million-row SHA batch.
///
/// This is the production assembly boundary: it rejects omission, role reorder, inactive-call
/// substitution, overlength input, and digest mismatch before any segment provider is registered.
pub fn validate_zk_x509_sha_call_witnesses_v1(
    schedule: &ZkX509ShaCallScheduleV1,
    witnesses: &[ZkX509ShaCallWitnessV1; ZK_X509_SHA_CALL_COUNT_V1],
) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
    for (manifest, witness) in schedule.calls().iter().copied().zip(witnesses) {
        validate_witness_v1(manifest, witness)?;
    }
    Ok(())
}
#[cfg(test)]
fn padded_words_v1(
    message: &[u8],
    maximum_blocks: usize,
) -> Result<Vec<u32>, ZkX509ShaCallBusStarkErrorV1> {
    let blocks = padded_blocks_v1(message.len())?;
    if blocks > maximum_blocks {
        return Err(ZkX509ShaCallBusStarkErrorV1::LengthOrPadding);
    }
    let padded_bytes = blocks
        .checked_mul(SHA_BLOCK_BYTES_V1)
        .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(padded_bytes)
        .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?;
    bytes.extend_from_slice(message);
    bytes.push(0x80);
    bytes.resize(
        padded_bytes
            .checked_sub(8)
            .ok_or(ZkX509ShaCallBusStarkErrorV1::LengthOrPadding)?,
        0,
    );
    let bit_len = u64::try_from(message.len())
        .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?
        .checked_mul(8)
        .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
    bytes.extend_from_slice(&bit_len.to_be_bytes());
    if bytes.len() != padded_bytes {
        return Err(ZkX509ShaCallBusStarkErrorV1::LengthOrPadding);
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|word| u32::from_be_bytes(word.try_into().expect("four-byte SHA word")))
        .collect())
}
#[cfg(test)]
fn digest_words_v1(digest: [u8; 32]) -> [u32; SHA_DIGEST_WORDS_V1] {
    core::array::from_fn(|word| {
        u32::from_be_bytes(
            digest[word * 4..word * 4 + 4]
                .try_into()
                .expect("four-byte digest word"),
        )
    })
}
/// Replay one fixed maximum-capacity event without materializing the schedule.
#[cfg(test)]
pub fn replay_sha_call_event_v1(
    schedule: &ZkX509ShaCallScheduleV1,
    witnesses: &[ZkX509ShaCallWitnessV1; ZK_X509_SHA_CALL_COUNT_V1],
    index: usize,
) -> Result<ZkX509ShaCallEventV1, ZkX509ShaCallBusStarkErrorV1> {
    let (manifest, word_kind, word) = schedule.fixed_event(index)?;
    let witness = witnesses
        .get(usize::from(manifest.call))
        .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)?;
    let blocks = validate_witness_v1(manifest, witness)?;
    let word_index = usize::from(word);
    let (active, value) = match word_kind {
        ZkX509ShaCallWordKindV1::Input => {
            let actual_words = blocks
                .checked_mul(SHA_WORDS_PER_BLOCK_V1)
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            if word_index < actual_words {
                let words = padded_words_v1(&witness.message, manifest.maximum_blocks)?;
                (
                    true,
                    *words
                        .get(word_index)
                        .ok_or(ZkX509ShaCallBusStarkErrorV1::Event)?,
                )
            } else {
                (false, 0)
            }
        }
        ZkX509ShaCallWordKindV1::Digest => (
            true,
            *digest_words_v1(witness.digest)
                .get(word_index)
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Event)?,
        ),
    };
    Ok(ZkX509ShaCallEventV1 {
        call: manifest.call,
        role: manifest.role,
        word_kind,
        word,
        active,
        value,
    })
}
/// Deterministic logical SHA segment provider.
///
/// The provider stores only the segment index.  Base, auxiliary, and fixed
/// columns are replayed from the canonical call witnesses by the owning SHA
/// adapter, so all three native groups are never resident together.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkX509ShaSegmentReplayV1 {
    segment: u8,
}
impl ZkX509ShaSegmentReplayV1 {
    /// Construct one of the four canonical logical segments.
    pub(crate) fn new(segment: usize) -> Result<Self, ZkX509ShaCallBusStarkErrorV1> {
        if segment >= ZK_X509_SHA_SEGMENT_COUNT_V1 {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        Ok(Self {
            segment: u8::try_from(segment).map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?,
        })
    }
    /// Segment index in canonical order.
    pub(crate) const fn segment(self) -> usize {
        self.segment as usize
    }
    /// Exact active rows; the remaining native rows are canonical padding.
    pub(crate) const fn active_rows(self) -> usize {
        ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[self.segment()]
    }
    /// Global logical row selected by one active local row.
    #[cfg(test)]
    pub(crate) fn global_row(
        self,
        local_row: usize,
    ) -> Result<usize, ZkX509ShaCallBusStarkErrorV1> {
        if local_row >= self.active_rows() {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        self.segment()
            .checked_mul(ZK_X509_SHA_SEGMENT_ROWS_V1)
            .and_then(|start| start.checked_add(local_row))
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)
    }
}
/// One independently sampled affine tuple-compression lane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkX509ShaCallBusLaneChallengesV1 {
    /// `beta`, call, role, slot, kind, word, and value coefficients.
    pub(crate) terms: [F; CALL_TUPLE_TERMS_V1],
}
/// Four independent call-bus lanes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZkX509ShaCallBusChallengesV1 {
    /// Domain-separated lane challenges.
    pub(crate) lanes: [ZkX509ShaCallBusLaneChallengesV1; ZK_X509_SHA_BUS_LANES_V1],
}
impl ZkX509ShaCallBusChallengesV1 {
    /// Reject zero, non-canonical, or repeated scalar challenges.
    pub(crate) fn validate(self) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        let mut seen = [F::ZERO; ZK_X509_SHA_BUS_LANES_V1 * CALL_TUPLE_TERMS_V1];
        let mut count = 0_usize;
        for lane in self.lanes {
            for term in lane.terms {
                if term == F::ZERO
                    || F::canonical(term.0).is_none()
                    || seen[..count].contains(&term)
                {
                    return Err(ZkX509ShaCallBusStarkErrorV1::Challenge);
                }
                seen[count] = term;
                count += 1;
            }
        }
        Ok(())
    }
}
/// Derive the call-bus family after all source and SHA base commitments.
///
/// Word-memory uses a different label family and must never reuse these challenges.
pub fn derive_zk_x509_sha_call_bus_challenges_v1(
    transcript: &mut TransparentTranscriptV1,
) -> Result<ZkX509ShaCallBusChallengesV1, TransparentStarkErrorV1> {
    let mut sampled = [F::ZERO; ZK_X509_SHA_BUS_LANES_V1 * CALL_TUPLE_TERMS_V1];
    for (index, challenge) in sampled.iter_mut().enumerate() {
        let lane = u16::try_from(index / CALL_TUPLE_TERMS_V1)
            .expect("four lanes fit u16")
            .to_be_bytes();
        let term = u16::try_from(index % CALL_TUPLE_TERMS_V1)
            .expect("seven terms fit u16")
            .to_be_bytes();
        let label = [
            b"zk-x509-sha-call-address-value-bus-four-lane-v1".as_slice(),
            &lane,
            &term,
        ]
        .concat();
        *challenge = transcript.challenge_field(&label)?;
    }
    Ok(ZkX509ShaCallBusChallengesV1 {
        lanes: core::array::from_fn(|lane| ZkX509ShaCallBusLaneChallengesV1 {
            terms: core::array::from_fn(|term| sampled[lane * CALL_TUPLE_TERMS_V1 + term]),
        }),
    })
}
fn validate_sha_segment_binding_families_v1(
    word: ZkX509ShaWordStarkChallengesV1,
    call: ZkX509ShaCallBusChallengesV1,
    rfc5280: &ZkX509Rfc5280StarkChallengesV1,
) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
    validate_zk_x509_sha_word_stark_challenges_v1(word)?;
    call.validate()?;
    rfc5280
        .validate()
        .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Challenge)
}
fn validate_sha_segment_replay_plan_v1(
    schedule: &ZkX509ShaCallScheduleV1,
    replay: ZkX509ShaSegmentReplayV1,
) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
    let segment = replay.segment();
    if segment >= ZK_X509_SHA_SEGMENT_COUNT_V1 {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    let physical_start = ZK_X509_SHA_PHYSICAL_CALL_COUNTS_V1[..segment]
        .iter()
        .try_fold(0_usize, |sum, count| sum.checked_add(*count))
        .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
    let physical_end = physical_start
        .checked_add(ZK_X509_SHA_PHYSICAL_CALL_COUNTS_V1[segment])
        .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
    let segment_start = segment
        .checked_mul(ZK_X509_SHA_SEGMENT_ROWS_V1)
        .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
    let mut next_row = segment_start;
    for call_index in ZK_X509_SHA_PHYSICAL_CALL_ORDER_V1
        .get(physical_start..physical_end)
        .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)?
    {
        let manifest = schedule.call(usize::from(*call_index))?;
        if manifest.call != *call_index || manifest.first_logical_row != next_row {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        next_row = next_row
            .checked_add(manifest.maximum_logical_rows())
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
    }
    let active_end = segment_start
        .checked_add(replay.active_rows())
        .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
    let segment_end = segment_start
        .checked_add(ZK_X509_SHA_SEGMENT_ROWS_V1)
        .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
    if physical_end > ZK_X509_SHA_PHYSICAL_CALL_ORDER_V1.len()
        || next_row != active_end
        || active_end > segment_end
    {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    Ok(())
}
struct ZkX509ShaColumnFillGuardV1<'a> {
    target: &'a mut [F],
    written: usize,
    valid: bool,
    committed: bool,
}
impl<'a> ZkX509ShaColumnFillGuardV1<'a> {
    fn new_v1(target: &'a mut [F]) -> Self {
        Self {
            target,
            written: 0,
            valid: true,
            committed: false,
        }
    }
    fn write_v1(&mut self, row: usize, value: F) {
        if !self.valid
            || row != self.written
            || row >= self.target.len()
            || F::canonical(value.0).is_none()
        {
            self.valid = false;
            return;
        }
        self.target[row] = value;
        self.written += 1;
    }
    #[cfg(test)]
    fn finish_v1(mut self) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        if !self.valid || self.written != self.target.len() {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        self.committed = true;
        Ok(())
    }
}
impl Drop for ZkX509ShaColumnFillGuardV1<'_> {
    fn drop(&mut self) {
        if !self.committed {
            for value in self.target.iter_mut() {
                value.zeroize_v1();
            }
        }
    }
}
fn sha_column_fill_batch_v1<'a>(
    targets: &'a mut [&mut [F]],
) -> Result<Vec<ZkX509ShaColumnFillGuardV1<'a>>, ZkX509ShaCallBusStarkErrorV1> {
    let mut fills = Vec::new();
    fills
        .try_reserve_exact(targets.len())
        .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?;
    for target in targets {
        fills.push(ZkX509ShaColumnFillGuardV1::new_v1(target));
    }
    Ok(fills)
}
fn finish_sha_column_fill_batch_v1(
    mut fills: Vec<ZkX509ShaColumnFillGuardV1<'_>>,
) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
    // Validate every destination before transferring any successful column.
    // A malformed late column must clear the complete partial batch.
    if fills
        .iter()
        .any(|fill| !fill.valid || fill.written != fill.target.len())
    {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    for fill in &mut fills {
        fill.committed = true;
    }
    Ok(())
}
fn validate_sha_column_batch_extent_v1(
    first: usize,
    width: usize,
    targets: &[&mut [F]],
) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
    if targets.is_empty()
        || targets.len() > crate::privacy_engines::aggregate_stark::MASKED_TRACE_LDE_COLUMN_BATCH_V1
        || first
            .checked_add(targets.len())
            .is_none_or(|end| end > width)
        || targets
            .iter()
            .any(|target| target.len() != ZK_X509_SHA_SEGMENT_ROWS_V1)
    {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    Ok(())
}
/// Challenge-independent source for one of the four canonical log-19 SHA registrations.
///
/// The source can stream every base and verifier-fixed row without receiving any post-base
/// challenge. Binding is a one-shot runtime capability: a failed challenge validation leaves the
/// source retryable, while a successful bind permanently disables a second transition.
pub struct ZkX509ShaBatchSegmentBaseSourceV1<'a> {
    schedule: &'a ZkX509ShaCallScheduleV1,
    witnesses: &'a [ZkX509ShaCallWitnessV1; ZK_X509_SHA_CALL_COUNT_V1],
    replay: ZkX509ShaSegmentReplayV1,
    bound: bool,
    retained_base: Option<native_base_cache::ShaNativeBaseCacheV1>,
}
impl<'a> ZkX509ShaBatchSegmentBaseSourceV1<'a> {
    /// Validate the schedule, complete witness set, and physical segment
    /// before exposing any committed row.
    pub(crate) fn new_v1(
        schedule: &'a ZkX509ShaCallScheduleV1,
        witnesses: &'a [ZkX509ShaCallWitnessV1; ZK_X509_SHA_CALL_COUNT_V1],
        segment: usize,
    ) -> Result<Self, ZkX509ShaCallBusStarkErrorV1> {
        let replay = ZkX509ShaSegmentReplayV1::new(segment)?;
        validate_sha_segment_replay_plan_v1(schedule, replay)?;
        validate_zk_x509_sha_call_witnesses_v1(schedule, witnesses)?;
        Ok(Self {
            schedule,
            witnesses,
            replay,
            bound: false,
            retained_base: None,
        })
    }
    /// Heap allowance for the four compact original native base caches. Inline
    /// headers are charged by MAIN's enclosing source forecast.
    pub(crate) fn native_base_cache_forecast_all_v1() -> Result<usize, ZkX509ShaCallBusStarkErrorV1>
    {
        native_base_cache::ShaNativeBaseCacheV1::heap_forecast_v1(ZK_X509_SHA_SEGMENT_ROWS_V1)?
            .checked_mul(ZK_X509_SHA_SEGMENT_COUNT_V1)
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)
    }
    /// Retain every original cell once, before masks or binding, under MAIN's
    /// admitted source allowance. Witnesses remain immutably borrowed for the
    /// lifetime of this owner and its bound successor.
    pub(crate) fn retain_native_base_v1(&mut self) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        self.ensure_base_phase_v1()?;
        if self.retained_base.is_some() {
            return Err(ZkX509ShaCallBusStarkErrorV1::Phase);
        }
        let cache = native_base_cache::ShaNativeBaseCacheV1::build_v1(
            ZK_X509_SHA_SEGMENT_ROWS_V1,
            |visitor| {
                self.for_each_base_call_using_v1(
                    |first, call| {
                        for row in 0..call.logical_rows() {
                            let original = zeroize::Zeroizing::new(call.base_row(row)?);
                            visitor(first + row, &original)?;
                        }
                        Ok(())
                    },
                    |manifest, witness| {
                        build_zk_x509_sha_batch_call_base_source_v1(
                            manifest,
                            witness,
                            self.schedule.shape().disclosed_attributes,
                        )
                    },
                )?;
                for row in self.replay.active_rows()..ZK_X509_SHA_SEGMENT_ROWS_V1 {
                    visitor(row, &[F::ZERO; ZK_X509_SHA_BATCH_BASE_WIDTH_V1])?;
                }
                Ok(())
            },
        )?;
        self.retained_base = Some(cache);
        Ok(())
    }
    /// Actual compact row allocation, separate from borrowed witness storage.
    pub(crate) fn retained_base_heap_bytes_v1(&self) -> usize {
        self.retained_base
            .as_ref()
            .map_or(0, native_base_cache::ShaNativeBaseCacheV1::heap_bytes_v1)
    }
    /// Canonical registration instance.
    pub(crate) const fn segment(&self) -> usize {
        self.replay.segment()
    }
    fn ensure_base_phase_v1(&self) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        if self.bound {
            Err(ZkX509ShaCallBusStarkErrorV1::Phase)
        } else {
            Ok(())
        }
    }
    fn validate_column_request_v1(
        &self,
        segment: usize,
        local_column: usize,
        width: usize,
        target: &[F],
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        self.ensure_base_phase_v1()?;
        if segment != self.segment()
            || local_column >= width
            || target.len() != ZK_X509_SHA_SEGMENT_ROWS_V1
        {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        validate_sha_segment_replay_plan_v1(self.schedule, self.replay)
    }
    /// Replay one challenge-independent base column into an exact native segment-sized target.
    ///
    /// The caller owns the sole output allocation. Internally this method
    /// retains at most one fixed-capacity call, checks canonical fields and
    /// exact row order, and clears partial output if replay fails.
    pub(crate) fn fill_base_column_v1(
        &self,
        segment: usize,
        local_column: usize,
        target: &mut [F],
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        self.fill_base_columns_v1(segment, local_column, &mut [target])
    }
    /// Replay at most eight adjacent base columns in one bounded call traversal.
    /// Every destination is validated before construction; failed batches clear
    /// all destinations. MAIN's compact immutable cache avoids rebuilding calls.
    pub(crate) fn fill_base_columns_v1(
        &self,
        segment: usize,
        first_column: usize,
        targets: &mut [&mut [F]],
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        self.fill_base_columns_using_v1(segment, first_column, targets, |manifest, witness| {
            build_zk_x509_sha_batch_call_base_source_v1(
                manifest,
                witness,
                self.schedule.shape().disclosed_attributes,
            )
        })
    }
    fn fill_base_columns_using_v1(
        &self,
        segment: usize,
        first_column: usize,
        targets: &mut [&mut [F]],
        build_call: impl FnMut(
            ZkX509ShaCallManifestV1,
            &ZkX509ShaCallWitnessV1,
        )
            -> Result<ZkX509ShaBatchCallBaseSourceV1, ZkX509ShaCallBusStarkErrorV1>,
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        validate_sha_column_batch_extent_v1(
            first_column,
            ZK_X509_SHA_BATCH_BASE_WIDTH_V1,
            targets,
        )?;
        self.validate_column_request_v1(
            segment,
            first_column,
            ZK_X509_SHA_BATCH_BASE_WIDTH_V1,
            targets[0],
        )?;
        let mut fills = sha_column_fill_batch_v1(targets)?;
        if let Some(cache) = &self.retained_base {
            cache.fill_columns_v1(first_column, &mut fills)?;
            return finish_sha_column_fill_batch_v1(fills);
        }
        self.for_each_base_call_using_v1(
            |first_row, call| {
                // The source owns validated, immutable word rows. Project only
                // requested base cells; fixed AIR rows are independent of this
                // output and are replayed by the verifier-owned fixed provider.
                for call_row in 0..call.logical_rows() {
                    call.emit_base_columns_v1(
                        call_row,
                        first_column,
                        fills.len(),
                        |offset, value| {
                            fills[offset].write_v1(first_row + call_row, value);
                        },
                    )?;
                }
                Ok(())
            },
            build_call,
        )?;
        for row in self.replay.active_rows()..ZK_X509_SHA_SEGMENT_ROWS_V1 {
            for fill in &mut fills {
                fill.write_v1(row, F::ZERO);
            }
        }
        finish_sha_column_fill_batch_v1(fills)
    }
    /// Replay one verifier-fixed column before X5B1 into an exact native segment-sized target.
    #[cfg(test)]
    pub(crate) fn fill_fixed_column_v1(
        &self,
        segment: usize,
        local_column: usize,
        target: &mut [F],
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        self.validate_column_request_v1(
            segment,
            local_column,
            ZK_X509_SHA_BATCH_FIXED_WIDTH_V1,
            target,
        )?;
        let mut fill = ZkX509ShaColumnFillGuardV1::new_v1(target);
        self.for_each_base_fixed_row_v1(|row, _, fixed| {
            fill.write_v1(row, fixed[local_column]);
        })?;
        fill.finish_v1()
    }
    /// Reconstruct one base/fixed row for bounded opening tests and sampled commitment checks.
    #[cfg(test)]
    pub(crate) fn base_fixed_row_v1(
        &self,
        segment_row: usize,
    ) -> Result<
        (
            [F; ZK_X509_SHA_BATCH_BASE_WIDTH_V1],
            [F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
        ),
        ZkX509ShaCallBusStarkErrorV1,
    > {
        self.ensure_base_phase_v1()?;
        validate_sha_segment_replay_plan_v1(self.schedule, self.replay)?;
        if segment_row >= ZK_X509_SHA_SEGMENT_ROWS_V1 {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        if segment_row >= self.replay.active_rows() {
            let padding = physical_padding_row_v1(segment_row);
            return Ok((padding.base, padding.fixed));
        }
        let global_row = self.replay.global_row(segment_row)?;
        let (manifest, local_row) = self.schedule.logical_row(global_row)?;
        let witness = self
            .witnesses
            .get(usize::from(manifest.call))
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)?;
        let call = build_zk_x509_sha_batch_call_base_source_v1(
            manifest,
            witness,
            self.schedule.shape().disclosed_attributes,
        )?;
        let mut fixed = call.fixed_row(local_row)?;
        fixed[ZK_X509_SHA_FIXED_SEGMENT_FIRST_V1] = F(u64::from(segment_row == 0));
        fixed[ZK_X509_SHA_FIXED_SEGMENT_LAST_V1] =
            F(u64::from(segment_row + 1 == self.replay.active_rows()));
        Ok((call.base_row(local_row)?, fixed))
    }
    /// Stream the complete fresh base/fixed registration for independent replay
    /// tests, materializing at most one call at a time.
    #[cfg(test)]
    pub(crate) fn for_each_base_fixed_row_v1(
        &self,
        visitor: impl FnMut(
            usize,
            [F; ZK_X509_SHA_BATCH_BASE_WIDTH_V1],
            [F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
        ),
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        self.for_each_base_fixed_row_using_v1(visitor, |manifest, witness| {
            build_zk_x509_sha_batch_call_base_source_v1(
                manifest,
                witness,
                self.schedule.shape().disclosed_attributes,
            )
        })
    }
    #[cfg(test)]
    fn for_each_base_fixed_row_using_v1(
        &self,
        mut visitor: impl FnMut(
            usize,
            [F; ZK_X509_SHA_BATCH_BASE_WIDTH_V1],
            [F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
        ),
        build_call: impl FnMut(
            ZkX509ShaCallManifestV1,
            &ZkX509ShaCallWitnessV1,
        )
            -> Result<ZkX509ShaBatchCallBaseSourceV1, ZkX509ShaCallBusStarkErrorV1>,
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        self.for_each_base_call_using_v1(
            |first_row, call| {
                for call_row in 0..call.logical_rows() {
                    let segment_row = first_row + call_row;
                    let mut fixed = call.fixed_row(call_row)?;
                    fixed[ZK_X509_SHA_FIXED_SEGMENT_FIRST_V1] = F(u64::from(segment_row == 0));
                    fixed[ZK_X509_SHA_FIXED_SEGMENT_LAST_V1] =
                        F(u64::from(segment_row + 1 == self.replay.active_rows()));
                    visitor(segment_row, call.base_row(call_row)?, fixed);
                }
                Ok(())
            },
            build_call,
        )?;
        for segment_row in self.replay.active_rows()..ZK_X509_SHA_SEGMENT_ROWS_V1 {
            let padding = physical_padding_row_v1(segment_row);
            visitor(segment_row, padding.base, padding.fixed);
        }
        Ok(())
    }
    fn for_each_base_call_using_v1(
        &self,
        mut visitor: impl FnMut(
            usize,
            &ZkX509ShaBatchCallBaseSourceV1,
        ) -> Result<(), ZkX509ShaCallBusStarkErrorV1>,
        mut build_call: impl FnMut(
            ZkX509ShaCallManifestV1,
            &ZkX509ShaCallWitnessV1,
        ) -> Result<
            ZkX509ShaBatchCallBaseSourceV1,
            ZkX509ShaCallBusStarkErrorV1,
        >,
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        self.ensure_base_phase_v1()?;
        validate_sha_segment_replay_plan_v1(self.schedule, self.replay)?;
        let segment_start = self
            .segment()
            .checked_mul(ZK_X509_SHA_SEGMENT_ROWS_V1)
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
        let active_end = segment_start
            .checked_add(self.replay.active_rows())
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
        let mut emitted = 0_usize;
        for call_index in ZK_X509_SHA_PHYSICAL_CALL_ORDER_V1 {
            let manifest = self.schedule.call(usize::from(call_index))?;
            let call_start = manifest.first_logical_row;
            let call_end = call_start
                .checked_add(manifest.maximum_logical_rows())
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            let overlap_start = call_start.max(segment_start);
            let overlap_end = call_end.min(active_end);
            if overlap_start >= overlap_end {
                continue;
            }
            if overlap_start != call_start || overlap_end != call_end {
                return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
            }
            let witness = self
                .witnesses
                .get(usize::from(manifest.call))
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)?;
            let call = build_call(manifest, witness)?;
            let first_row = call_start
                .checked_sub(segment_start)
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            if call.manifest != manifest
                || call.logical_rows() != manifest.maximum_logical_rows()
                || first_row != emitted
            {
                return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
            }
            visitor(first_row, &call)?;
            emitted = emitted
                .checked_add(call.logical_rows())
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
        }
        if emitted != self.replay.active_rows() {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        Ok(())
    }
    /// Consume the sole phase transition using challenges extracted internally
    /// from the opaque X5B1 binding.
    pub(crate) fn bind_v1(
        &mut self,
        binding: &ZkX509CredentialPreAuxBindingV1,
    ) -> Result<ZkX509ShaBatchSegmentAuxSourceV1<'a>, ZkX509ShaCallBusStarkErrorV1> {
        if self.bound {
            return Err(ZkX509ShaCallBusStarkErrorV1::Phase);
        }
        validate_sha_segment_binding_families_v1(
            binding.sha_word(),
            binding.sha(),
            binding.rfc5280(),
        )?;
        self.bound = true;
        Ok(ZkX509ShaBatchSegmentAuxSourceV1 {
            schedule: self.schedule,
            witnesses: self.witnesses,
            replay: self.replay,
            binding: Some(*binding),
            row_stream_emitted: false,
            retained_aux: None,
            retained_base: self.retained_base.take(),
        })
    }
    #[cfg(test)]
    fn validate_bind_for_test_v1(
        &mut self,
        word: ZkX509ShaWordStarkChallengesV1,
        call: ZkX509ShaCallBusChallengesV1,
        rfc5280: &ZkX509Rfc5280StarkChallengesV1,
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        if self.bound {
            return Err(ZkX509ShaCallBusStarkErrorV1::Phase);
        }
        validate_sha_segment_binding_families_v1(word, call, rfc5280)
    }
    #[cfg(test)]
    const fn is_bound_for_test_v1(&self) -> bool {
        self.bound
    }
}
/// Challenge-bound auxiliary and terminal source for one SHA registration.
///
/// Construction is possible only through [`ZkX509ShaBatchSegmentBaseSourceV1::bind_v1`]. The
/// complete auxiliary stream is one-shot and never exposes a raw challenge constructor.
pub struct ZkX509ShaBatchSegmentAuxSourceV1<'a> {
    schedule: &'a ZkX509ShaCallScheduleV1,
    witnesses: &'a [ZkX509ShaCallWitnessV1; ZK_X509_SHA_CALL_COUNT_V1],
    replay: ZkX509ShaSegmentReplayV1,
    binding: Option<ZkX509CredentialPreAuxBindingV1>,
    row_stream_emitted: bool,
    retained_aux: Option<native_aux_cache::ShaNativeAuxCacheV1>,
    retained_base: Option<native_base_cache::ShaNativeBaseCacheV1>,
}
impl ZkX509ShaBatchSegmentAuxSourceV1<'_> {
    /// The original compact base allocation moves here without copying at X5B1.
    pub(crate) fn retained_base_heap_bytes_v1(&self) -> usize {
        self.retained_base
            .as_ref()
            .map_or(0, native_base_cache::ShaNativeBaseCacheV1::heap_bytes_v1)
    }
    /// Heap allowance for all four fixed native auxiliary caches. Inline owner
    /// storage is already included in the enclosing MAIN source forecast.
    pub(crate) fn native_aux_cache_forecast_all_v1() -> Result<usize, ZkX509ShaCallBusStarkErrorV1>
    {
        native_aux_cache::ShaNativeAuxCacheV1::heap_forecast_v1(ZK_X509_SHA_SEGMENT_ROWS_V1)?
            .checked_mul(ZK_X509_SHA_SEGMENT_COUNT_V1)
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)
    }
    /// Build once from this original opaque binding after MAIN has admitted the
    /// fixed allocation forecast. No caller-provided rows or endpoints enter it.
    pub(crate) fn retain_native_aux_v1(&mut self) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        if self.binding.is_none() || self.retained_aux.is_some() {
            return Err(ZkX509ShaCallBusStarkErrorV1::Phase);
        }
        let cache = native_aux_cache::ShaNativeAuxCacheV1::build_v1(
            ZK_X509_SHA_SEGMENT_ROWS_V1,
            self.replay.segment(),
            |visitor| self.replay_aux_rows_uncached_v1(visitor),
        )?;
        self.retained_aux = Some(cache);
        Ok(())
    }
    /// Reachable cache capacities, independently charged from transform scratch.
    pub(crate) fn retained_aux_heap_bytes_v1(&self) -> usize {
        self.retained_aux
            .as_ref()
            .map_or(0, native_aux_cache::ShaNativeAuxCacheV1::heap_bytes_v1)
    }
    /// Return the genuine private endpoint and compact-CA boundaries under
    /// this source's original opaque shared binding. No supplied endpoint or
    /// alternate raw challenge family is accepted by this operation.
    pub(crate) fn rfc_union_air_terminals_v1(
        &self,
        expected: &ZkX509CredentialPreAuxBindingV1,
        segment: usize,
    ) -> Result<ZkX509ShaSegmentAirTerminalsV1, ZkX509ShaCallBusStarkErrorV1> {
        if self.binding != Some(*expected) {
            return Err(ZkX509ShaCallBusStarkErrorV1::Phase);
        }
        if self.replay.segment() != segment {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        if let Some(cache) = &self.retained_aux {
            return cache.copy_terminals_v1();
        }
        self.replay_aux_rows_uncached_v1(|_, _| {})
    }
    /// Deterministically replay a challenge-independent column from this
    /// already-bound owner. This exposes no new phase or challenge constructor.
    pub(crate) fn replay_base_column_v1(
        &self,
        fixed: &ZkX509ShaBatchFixedProviderV1,
        segment: usize,
        local_column: usize,
        target: &mut [F],
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        self.replay_base_columns_v1(fixed, segment, local_column, &mut [target])
    }
    /// Replay original adjacent base columns through the already-bound owner,
    /// borrowing its existing charged public schedules rather than recompiling them.
    pub(crate) fn replay_base_columns_v1(
        &self,
        fixed: &ZkX509ShaBatchFixedProviderV1,
        segment: usize,
        first_column: usize,
        targets: &mut [&mut [F]],
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        let binding = self.binding.ok_or(ZkX509ShaCallBusStarkErrorV1::Phase)?;
        validate_sha_segment_binding_families_v1(
            binding.sha_word(),
            binding.sha(),
            binding.rfc5280(),
        )?;
        if fixed.schedule != *self.schedule {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        validate_sha_column_batch_extent_v1(
            first_column,
            ZK_X509_SHA_BATCH_BASE_WIDTH_V1,
            targets,
        )?;
        if segment != self.replay.segment() {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        validate_sha_segment_replay_plan_v1(self.schedule, self.replay)?;
        if let Some(cache) = &self.retained_base {
            let mut fills = sha_column_fill_batch_v1(targets)?;
            cache.fill_columns_v1(first_column, &mut fills)?;
            return finish_sha_column_fill_batch_v1(fills);
        }
        let source = ZkX509ShaBatchSegmentBaseSourceV1 {
            schedule: self.schedule,
            witnesses: self.witnesses,
            replay: self.replay,
            bound: false,
            retained_base: None,
        };
        source.fill_base_columns_using_v1(segment, first_column, targets, |manifest, witness| {
            fixed.call_base_source_v1(manifest, witness)
        })
    }

    #[cfg(test)]
    fn replay_aux_rows_with_air_terminals_v1(
        &self,
        mut visitor: impl FnMut(usize, [F; ZK_X509_SHA_BATCH_AUX_WIDTH_V1]),
    ) -> Result<ZkX509ShaSegmentAirTerminalsV1, ZkX509ShaCallBusStarkErrorV1> {
        self.binding.ok_or(ZkX509ShaCallBusStarkErrorV1::Phase)?;
        if let Some(cache) = &self.retained_aux {
            for (row, values) in cache.rows_v1().iter().enumerate() {
                visitor(row, *values);
            }
            return cache.copy_terminals_v1();
        }
        self.replay_aux_rows_uncached_v1(visitor)
    }

    fn replay_aux_rows_uncached_v1(
        &self,
        mut visitor: impl FnMut(usize, [F; ZK_X509_SHA_BATCH_AUX_WIDTH_V1]),
    ) -> Result<ZkX509ShaSegmentAirTerminalsV1, ZkX509ShaCallBusStarkErrorV1> {
        let binding = self.binding.ok_or(ZkX509ShaCallBusStarkErrorV1::Phase)?;
        validate_sha_segment_replay_plan_v1(self.schedule, self.replay)?;
        let segment_start = self
            .replay
            .segment()
            .checked_mul(ZK_X509_SHA_SEGMENT_ROWS_V1)
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
        let active_end = segment_start
            .checked_add(self.replay.active_rows())
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
        let projection_terminal = projection_bus::segment_terminal_v1(
            self.schedule,
            self.witnesses,
            self.replay.segment(),
            binding.main_post_base().sha(),
        )?;
        let mut products = ZkX509ShaSegmentProductStateV1::one_v1();
        let mut ca_call_boundaries = PrivateTableV1::new(Vec::new(), clear_ca_boundary_products_v1);
        // Reserve before private copies are written, so reallocation cannot leave
        // retired product cells in an old allocation.
        ca_call_boundaries
            .try_reserve_exact(ZK_X509_SHA_CA_CALL_COUNT_V1)
            .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?;
        ca_call_boundaries
            .try_reserve_exact(ZK_X509_SHA_CA_CALL_COUNT_V1)
            .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?;
        let mut emitted = 0_usize;
        for call_index in ZK_X509_SHA_PHYSICAL_CALL_ORDER_V1 {
            let manifest = self.schedule.call(usize::from(call_index))?;
            let call_start = manifest.first_logical_row;
            let call_end = call_start
                .checked_add(manifest.maximum_logical_rows())
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            let overlap_start = call_start.max(segment_start);
            let overlap_end = call_end.min(active_end);
            if overlap_start >= overlap_end {
                continue;
            }
            if overlap_start != call_start || overlap_end != call_end {
                return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
            }
            let witness = self
                .witnesses
                .get(usize::from(manifest.call))
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)?;
            let base = build_zk_x509_sha_batch_call_base_source_v1(
                manifest,
                witness,
                self.schedule.shape().disclosed_attributes,
            )?;
            let call = bind_zk_x509_sha_batch_call_base_with_initial_products_v1(
                base, &binding, &products,
            )?;
            if usize::from(manifest.call) >= ZK_X509_SHA_CA_LEAF_CALL_V1 {
                let boundary = zeroize::Zeroizing::new(ZkX509ShaCallBoundaryTerminalV1 {
                    call: manifest.call,
                    role: manifest.role,
                    source_start_products: products.source_products,
                    digest_start_products: products.digest_products,
                    source_products: call.terminal.source_products,
                    digest_products: call.terminal.digest_products,
                });
                boundary.validate_identity_v1(
                    usize::from(manifest.call) - ZK_X509_SHA_CA_LEAF_CALL_V1,
                )?;
                ca_call_boundaries.push(*boundary);
            }
            for global_row in call_start..call_end {
                let call_row = global_row
                    .checked_sub(call_start)
                    .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
                let segment_row = global_row
                    .checked_sub(segment_start)
                    .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
                let mut aux = call.aux_row(call_row)?;
                if segment_row == 0 {
                    aux[projection_bus::AUX_START_V1..].copy_from_slice(&*projection_terminal);
                }
                visitor(segment_row, aux);
                emitted = emitted
                    .checked_add(1)
                    .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            }
            products.copy_from_v1(&call.segment_product_state);
        }
        for segment_row in self.replay.active_rows()..ZK_X509_SHA_SEGMENT_ROWS_V1 {
            let mut aux = [F::ZERO; ZK_X509_SHA_BATCH_AUX_WIDTH_V1];
            aux[projection_bus::AUX_START_V1..].copy_from_slice(&products.projection_products);
            visitor(segment_row, aux);
            emitted = emitted
                .checked_add(1)
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
        }
        if products.projection_products != *projection_terminal {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        if emitted != ZK_X509_SHA_SEGMENT_ROWS_V1 {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        Ok(ZkX509ShaSegmentAirTerminalsV1 {
            segment: products.terminal_v1(self.replay.segment())?,
            ca_call_boundaries: ca_call_boundaries.into_vec(),
        })
    }
    fn validate_column_request_v1(
        &self,
        segment: usize,
        local_column: usize,
        target: &[F],
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        if self.binding.is_none() {
            return Err(ZkX509ShaCallBusStarkErrorV1::Phase);
        }
        if segment != self.replay.segment()
            || local_column >= ZK_X509_SHA_BATCH_AUX_WIDTH_V1
            || target.len() != ZK_X509_SHA_SEGMENT_ROWS_V1
        {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        validate_sha_segment_replay_plan_v1(self.schedule, self.replay)
    }
    /// Deterministically replay one challenge-bound auxiliary column.
    ///
    /// The opaque X5B1 binding remains internal. Replaying a column does not consume the separate
    /// one-shot row stream, so MAIN may request all registered columns in any deterministic order
    /// using MAIN's admitted native cache when present.
    pub(crate) fn fill_aux_column_with_air_terminals_v1(
        &self,
        segment: usize,
        local_column: usize,
        target: &mut [F],
    ) -> Result<ZkX509ShaSegmentAirTerminalsV1, ZkX509ShaCallBusStarkErrorV1> {
        self.fill_aux_columns_with_air_terminals_v1(segment, local_column, &mut [target])
    }
    /// Replay at most eight adjacent auxiliary columns with the exact bound
    /// segment and compact-CA terminal checks from the single row stream.
    pub(crate) fn fill_aux_columns_with_air_terminals_v1(
        &self,
        segment: usize,
        first_column: usize,
        targets: &mut [&mut [F]],
    ) -> Result<ZkX509ShaSegmentAirTerminalsV1, ZkX509ShaCallBusStarkErrorV1> {
        validate_sha_column_batch_extent_v1(first_column, ZK_X509_SHA_BATCH_AUX_WIDTH_V1, targets)?;
        self.validate_column_request_v1(segment, first_column, targets[0])?;
        let mut fills = sha_column_fill_batch_v1(targets)?;
        let terminals = if let Some(cache) = &self.retained_aux {
            cache.fill_columns_v1(first_column, &mut fills)?;
            cache.copy_terminals_v1()?
        } else {
            self.replay_aux_rows_uncached_v1(|row, aux| {
                for (offset, fill) in fills.iter_mut().enumerate() {
                    fill.write_v1(row, aux[first_column + offset]);
                }
            })?
        };
        finish_sha_column_fill_batch_v1(fills)?;
        Ok(terminals)
    }
    /// Replay one auxiliary column when only the registration terminal is needed.
    pub(crate) fn fill_aux_column_v1(
        &self,
        segment: usize,
        local_column: usize,
        target: &mut [F],
    ) -> Result<ZkX509ShaSegmentPrivateEndpointV1, ZkX509ShaCallBusStarkErrorV1> {
        Ok(self
            .fill_aux_column_with_air_terminals_v1(segment, local_column, target)?
            .into_segment_v1())
    }
    /// Stream all challenge-dependent auxiliary rows once and return the exact
    /// segment and compact-CA boundary terminals.
    #[cfg(test)]
    pub(crate) fn for_each_aux_row_with_air_terminals_v1(
        &mut self,
        visitor: impl FnMut(usize, [F; ZK_X509_SHA_BATCH_AUX_WIDTH_V1]),
    ) -> Result<ZkX509ShaSegmentAirTerminalsV1, ZkX509ShaCallBusStarkErrorV1> {
        if self.row_stream_emitted {
            return Err(ZkX509ShaCallBusStarkErrorV1::Phase);
        }
        // Consume the row-stream capability before fallible replay. A failed
        // stream cannot be retried under a different observer.
        self.row_stream_emitted = true;
        self.replay_aux_rows_with_air_terminals_v1(visitor)
    }
    /// Stream once when only the registration terminal is needed.
    #[cfg(test)]
    pub(crate) fn for_each_aux_row_v1(
        &mut self,
        visitor: impl FnMut(usize, [F; ZK_X509_SHA_BATCH_AUX_WIDTH_V1]),
    ) -> Result<ZkX509ShaSegmentPrivateEndpointV1, ZkX509ShaCallBusStarkErrorV1> {
        Ok(self
            .for_each_aux_row_with_air_terminals_v1(visitor)?
            .into_segment_v1())
    }
    /// Clear the retained rows, private terminals and opaque binding, then close replay.
    pub(crate) fn zeroize_private_v1(&mut self) {
        self.retained_base = None;
        self.retained_aux = None;
        self.binding = None;
        self.row_stream_emitted = true;
    }
    #[cfg(test)]
    fn private_is_zeroized_v1(&self) -> bool {
        self.binding.is_none()
            && self.row_stream_emitted
            && self.retained_aux.is_none()
            && self.retained_base.is_none()
    }
    #[cfg(test)]
    const fn row_stream_emitted_for_test_v1(&self) -> bool {
        self.row_stream_emitted
    }
}
impl Drop for ZkX509ShaBatchSegmentAuxSourceV1<'_> {
    fn drop(&mut self) {
        self.zeroize_private_v1();
    }
}
#[cfg(test)]
fn compress_event_v1(event: ZkX509ShaCallEventV1, lane: ZkX509ShaCallBusLaneChallengesV1) -> F {
    compress_sha_call_fields_v1(
        F(u64::from(event.call)),
        F(u64::from(event.role.role_code())),
        F(u64::from(event.role.slot())),
        F(u64::from(event.word_kind.code())),
        F(u64::from(event.word)),
        F(u64::from(event.value)),
        lane,
    )
}
/// Compress one opened address/value tuple without converting its value back
/// to a host integer. Proof-facing source adapters use this exact expression.
#[allow(clippy::too_many_arguments)]
pub fn compress_sha_call_fields_v1<A: PolynomialAirFieldV1>(
    call: A,
    role: A,
    slot: A,
    word_kind: A,
    word: A,
    value: A,
    lane: ZkX509ShaCallBusLaneChallengesV1,
) -> A {
    A::from_base(lane.terms[0])
        .add(call.mul_base(lane.terms[1]))
        .add(role.mul_base(lane.terms[2]))
        .add(slot.mul_base(lane.terms[3]))
        .add(word_kind.mul_base(lane.terms[4]))
        .add(word.mul_base(lane.terms[5]))
        .add(value.mul_base(lane.terms[6]))
}
fn call_row_factor_v1(
    manifest: ZkX509ShaCallManifestV1,
    word_kind: ZkX509ShaCallWordKindV1,
    word: usize,
    value: F,
    lane: ZkX509ShaCallBusLaneChallengesV1,
) -> Result<F, ZkX509ShaCallBusStarkErrorV1> {
    Ok(compress_sha_call_fields_v1(
        F(u64::from(manifest.call)),
        F(u64::from(manifest.role.role_code())),
        F(u64::from(manifest.role.slot())),
        F(u64::from(word_kind.code())),
        F(u64::try_from(word).map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?),
        value,
        lane,
    ))
}
fn sha_rfc_framed_field_offset_v1(
    role: ZkX509ShaCallRoleV1,
) -> Result<usize, ZkX509ShaCallBusStarkErrorV1> {
    match role {
        ZkX509ShaCallRoleV1::CrlCommitment => crl_commitment_preimage_v1(&[0])
            .ok()
            .and_then(|frame| frame.len().checked_sub(1))
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology),
        ZkX509ShaCallRoleV1::CrlIssuerSpki => {
            let spki = [0_u8; ZK_X509_CA_SPKI_DER_BYTES_V1];
            crl_issuer_spki_preimage_v1(&spki)
                .ok()
                .and_then(|frame| frame.len().checked_sub(spki.len()))
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)
        }
        _ => Ok(0),
    }
}
/// Derive a canonical RFC address from only the verifier-owned SHA call and public shape.
pub fn sha_rfc_consumer_channels_v1(
    call: u8,
    role: ZkX509ShaCallRoleV1,
    disclosed_attributes: usize,
) -> Result<Option<ZkX509ShaRfcConsumerChannelsV1>, ZkX509ShaCallBusStarkErrorV1> {
    if disclosed_attributes > 4 || manifest_role_v1(usize::from(call))? != role {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    let projection_channels = 5_usize
        .checked_add(
            disclosed_attributes
                .checked_mul(2)
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?,
        )
        .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
    let channel = |offset: usize| {
        projection_channels
            .checked_add(offset)
            .and_then(|channel| u32::try_from(channel).ok())
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)
    };
    let consumer = match role {
        ZkX509ShaCallRoleV1::CertificateTbs(slot) if slot < 3 => {
            let pair = usize::from(slot)
                .checked_mul(2)
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            Some(ZkX509ShaRfcConsumerChannelsV1 {
                role: ZkX509Rfc5280OutputRoleV1::CertificateTbsSha,
                message_channel: channel(pair)?,
                length_channel: Some(channel(pair + 1)?),
                message_prefix_bytes: 0,
                message_capacity_bytes: ZK_X509_SHA_CALL_MAX_DER_BYTES_V1,
            })
        }
        ZkX509ShaCallRoleV1::CrlTbs => Some(ZkX509ShaRfcConsumerChannelsV1 {
            role: ZkX509Rfc5280OutputRoleV1::CrlTbsP256Message,
            message_channel: channel(16)?,
            length_channel: Some(channel(17)?),
            message_prefix_bytes: 0,
            message_capacity_bytes: ZK_X509_SHA_CALL_MAX_DER_BYTES_V1,
        }),
        ZkX509ShaCallRoleV1::CrlCommitment => Some(ZkX509ShaRfcConsumerChannelsV1 {
            role: ZkX509Rfc5280OutputRoleV1::CrlCommitment,
            message_channel: channel(18)?,
            length_channel: Some(channel(19)?),
            message_prefix_bytes: sha_rfc_framed_field_offset_v1(role)?,
            message_capacity_bytes: ZK_X509_CRL_COMMITMENT_MAX_DER_BYTES_V1,
        }),
        ZkX509ShaCallRoleV1::CrlIssuerSpki => Some(ZkX509ShaRfcConsumerChannelsV1 {
            role: ZkX509Rfc5280OutputRoleV1::IssuerSpkiSha,
            // The CRL signer and wallet-owner P-256 keys precede the issuer SPKI.
            message_channel: channel(24)?,
            length_channel: None,
            message_prefix_bytes: sha_rfc_framed_field_offset_v1(role)?,
            message_capacity_bytes: ZK_X509_CA_SPKI_DER_BYTES_V1,
        }),
        _ => None,
    };
    Ok(consumer)
}
fn pack_sha_rfc_bits_v1<A: PolynomialAirFieldV1>(bits: &[A]) -> A {
    bits.iter()
        .copied()
        .enumerate()
        .fold(A::ZERO, |packed, (bit, value)| {
            packed.add(value.mul(A::from_base(F(1_u64 << bit))))
        })
}
fn sha_rfc_message_byte_v1<A: PolynomialAirFieldV1>(
    base: &[A; ZK_X509_SHA_BATCH_BASE_WIDTH_V1],
    byte: usize,
) -> Result<A, ZkX509ShaCallBusStarkErrorV1> {
    if byte >= 4 {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    let bits_start = 1 + (3 - byte) * 8;
    Ok(pack_sha_rfc_bits_v1(&base[bits_start..bits_start + 8])
        .mul(base[SHA_WORD_CAPACITY_MESSAGE_MASK_V1 + byte]))
}
fn sha_rfc_opened_event_delta_v1<A: PolynomialAirFieldV1>(
    base: &[A; ZK_X509_SHA_BATCH_BASE_WIDTH_V1],
    fixed: &[A; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
    stream: usize,
    lane: usize,
    challenges: &ZkX509Rfc5280StarkChallengesV1,
) -> Result<A, ZkX509ShaCallBusStarkErrorV1> {
    if stream >= ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1 || lane >= ZK_X509_SHA_BUS_LANES_V1 {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    let start = ZK_X509_SHA_FIXED_RFC_STREAMS_V1
        .checked_add(
            stream
                .checked_mul(ZK_X509_SHA_FIXED_RFC_STREAM_STRIDE_V1)
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?,
        )
        .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
    let role = fixed[start + ZK_X509_SHA_FIXED_RFC_ROLE_V1];
    let channel = fixed[start + ZK_X509_SHA_FIXED_RFC_CHANNEL_V1];
    let offset = fixed[start + ZK_X509_SHA_FIXED_RFC_OFFSET_V1];
    let message_factor = zk_x509_rfc5280_opened_output_factor_fields_after_challenge_validation_v1(
        role,
        channel,
        A::from_base(F(2)),
        A::ZERO,
        offset,
        sha_rfc_message_byte_v1(base, stream)?,
        A::ZERO,
        lane,
        challenges,
    )
    .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Challenge)?;
    let length_high = pack_sha_rfc_bits_v1(
        &base[ZK_X509_SHA_LENGTH_BITS_START_V1 + 8
            ..ZK_X509_SHA_LENGTH_BITS_START_V1 + ZK_X509_SHA_RFC_LENGTH_BITS_V1],
    );
    let length_low = pack_sha_rfc_bits_v1(
        &base[ZK_X509_SHA_LENGTH_BITS_START_V1..ZK_X509_SHA_LENGTH_BITS_START_V1 + 8],
    );
    let length_high_factor =
        zk_x509_rfc5280_opened_output_factor_fields_after_challenge_validation_v1(
            role,
            channel,
            A::from_base(F(2)),
            A::ZERO,
            offset,
            length_high,
            A::ZERO,
            lane,
            challenges,
        )
        .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Challenge)?;
    let length_low_factor =
        zk_x509_rfc5280_opened_output_factor_fields_after_challenge_validation_v1(
            role,
            channel,
            A::from_base(F(2)),
            A::ZERO,
            offset,
            length_low,
            A::ZERO,
            lane,
            challenges,
        )
        .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Challenge)?;
    let length_zero_factor =
        zk_x509_rfc5280_opened_output_factor_fields_after_challenge_validation_v1(
            role,
            channel,
            A::from_base(F(2)),
            A::ZERO,
            offset,
            A::ZERO,
            A::ZERO,
            lane,
            challenges,
        )
        .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Challenge)?;
    let length_zero_selector = sha_rfc_zero_length_event_selector_v1(fixed, stream)?;
    Ok(fixed[start + ZK_X509_SHA_FIXED_RFC_MESSAGE_EVENT_V1]
        .mul(message_factor.sub(A::ONE))
        .add(length_zero_selector.mul(length_zero_factor.sub(A::ONE)))
        .add(
            fixed[start + ZK_X509_SHA_FIXED_RFC_LENGTH_HIGH_VALUE_V1]
                .mul(length_high_factor.sub(A::ONE)),
        )
        .add(
            fixed[start + ZK_X509_SHA_FIXED_RFC_LENGTH_LOW_VALUE_V1]
                .mul(length_low_factor.sub(A::ONE)),
        ))
}
fn sha_rfc_length_recomposition_residue_v1<A: PolynomialAirFieldV1>(
    base: &[A; ZK_X509_SHA_BATCH_BASE_WIDTH_V1],
    aux: &[A; ZK_X509_SHA_BATCH_AUX_WIDTH_V1],
    fixed: &[A; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
) -> A {
    let selector = fixed[ZK_X509_SHA_FIXED_RFC_LENGTH_PAIR_V1];
    let raw_length = pack_sha_rfc_bits_v1(
        &base[ZK_X509_SHA_LENGTH_BITS_START_V1
            ..ZK_X509_SHA_LENGTH_BITS_START_V1 + ZK_X509_SHA_RFC_LENGTH_BITS_V1],
    );
    selector.mul(
        aux[SHA_WORD_CAPACITY_MESSAGE_COUNT_V1]
            .sub(raw_length.add(fixed[ZK_X509_SHA_FIXED_RFC_LENGTH_PREFIX_V1])),
    )
}
/// Select the six leading zero bytes in an RFC-owned eight-byte length.
///
/// Streams zero and one carry one byte in each of four consecutive row
/// pairs. The final pair selects the constrained high and low values; the
/// first three pairs are equally real output events whose values are fixed
/// zero. This linear identity remains valid at verifier-opened LDE points.
fn sha_rfc_zero_length_event_selector_v1<A: PolynomialAirFieldV1>(
    fixed: &[A; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
    stream: usize,
) -> Result<A, ZkX509ShaCallBusStarkErrorV1> {
    if stream >= ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1 {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    if stream >= 2 {
        return Ok(A::ZERO);
    }
    let start = ZK_X509_SHA_FIXED_RFC_STREAMS_V1
        .checked_add(
            stream
                .checked_mul(ZK_X509_SHA_FIXED_RFC_STREAM_STRIDE_V1)
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?,
        )
        .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
    Ok(fixed[ZK_X509_SHA_FIXED_RFC_LENGTH_PAIR_V1]
        .sub(fixed[start + ZK_X509_SHA_FIXED_RFC_LENGTH_HIGH_VALUE_V1])
        .sub(fixed[start + ZK_X509_SHA_FIXED_RFC_LENGTH_LOW_VALUE_V1]))
}
/// Native-row witness helper. Opened-row verification must use
/// [`sha_rfc_opened_event_delta_v1`] and may not branch on these selectors.
fn sha_rfc_consumer_row_factor_v1(
    base: &[F; ZK_X509_SHA_BATCH_BASE_WIDTH_V1],
    fixed: &[F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
    stream: usize,
    lane: usize,
    challenges: &ZkX509Rfc5280StarkChallengesV1,
) -> Result<Option<F>, ZkX509ShaCallBusStarkErrorV1> {
    if stream >= ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1 || lane >= ZK_X509_SHA_BUS_LANES_V1 {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    let start = ZK_X509_SHA_FIXED_RFC_STREAMS_V1
        .checked_add(
            stream
                .checked_mul(ZK_X509_SHA_FIXED_RFC_STREAM_STRIDE_V1)
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?,
        )
        .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
    let selector = fixed[start + ZK_X509_SHA_FIXED_RFC_MESSAGE_EVENT_V1]
        .add(sha_rfc_zero_length_event_selector_v1(fixed, stream)?)
        .add(fixed[start + ZK_X509_SHA_FIXED_RFC_LENGTH_HIGH_VALUE_V1])
        .add(fixed[start + ZK_X509_SHA_FIXED_RFC_LENGTH_LOW_VALUE_V1]);
    match selector {
        F::ZERO => Ok(None),
        F::ONE => Ok(Some(F::ONE.add(sha_rfc_opened_event_delta_v1(
            base, fixed, stream, lane, challenges,
        )?))),
        _ => Err(ZkX509ShaCallBusStarkErrorV1::Topology),
    }
}
/// Materialize one challenge-independent call in the verifier-fixed layout.
///
/// The caller streams and drops this value before building the next call, so
/// the full 1.97-million-row batch is never resident as native field arrays.
pub fn build_zk_x509_sha_batch_call_base_source_v1(
    manifest: ZkX509ShaCallManifestV1,
    witness: &ZkX509ShaCallWitnessV1,
    disclosed_attributes: usize,
) -> Result<ZkX509ShaBatchCallBaseSourceV1, ZkX509ShaCallBusStarkErrorV1> {
    build_sha_batch_call_base_using_v1(
        manifest,
        witness,
        disclosed_attributes,
        build_sha_word_capacity_base_source_v1,
    )
}

fn build_sha_batch_call_base_using_v1(
    manifest: ZkX509ShaCallManifestV1,
    witness: &ZkX509ShaCallWitnessV1,
    disclosed_attributes: usize,
    build_word: impl FnOnce(
        &[u8],
        usize,
        bool,
    ) -> Result<ZkX509ShaWordCapacityBaseSourceV1, ZkX509ShaWordStarkErrorV1>,
) -> Result<ZkX509ShaBatchCallBaseSourceV1, ZkX509ShaCallBusStarkErrorV1> {
    validate_witness_v1(manifest, witness)?;
    let rfc_consumer =
        sha_rfc_consumer_channels_v1(manifest.call, manifest.role, disclosed_attributes)?;
    let word = build_word(
        &witness.message,
        manifest.maximum_message_bytes,
        role_has_exact_message_length_v1(manifest.role),
    )?;
    if word.maximum_blocks() != manifest.maximum_blocks
        || word.maximum_local_rows() != manifest.maximum_local_rows
        || word.maximum_memory_rows() != manifest.maximum_memory_rows
        || word.logical_rows() != manifest.maximum_logical_rows()
    {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    Ok(ZkX509ShaBatchCallBaseSourceV1 {
        manifest,
        word,
        rfc_consumer,
    })
}
fn bind_zk_x509_sha_batch_call_base_with_initial_products_v1(
    source: ZkX509ShaBatchCallBaseSourceV1,
    binding: &ZkX509CredentialPreAuxBindingV1,
    initial_products: &ZkX509ShaSegmentProductStateV1,
) -> Result<ZkX509ShaBatchCallTraceV1, ZkX509ShaCallBusStarkErrorV1> {
    let call_challenges = binding.sha();
    let rfc_challenges = binding.rfc5280();
    call_challenges.validate()?;
    rfc_challenges
        .validate()
        .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Challenge)?;
    validate_zk_x509_sha_word_stark_challenges_v1(binding.sha_word())?;
    let ZkX509ShaBatchCallBaseSourceV1 {
        manifest,
        word,
        rfc_consumer,
    } = source;
    let word = word.bind_v1(binding)?;
    finish_zk_x509_sha_batch_call_binding_v1(
        manifest,
        word,
        rfc_consumer,
        call_challenges,
        rfc_challenges,
        initial_products,
    )
}
fn finish_zk_x509_sha_batch_call_binding_v1(
    manifest: ZkX509ShaCallManifestV1,
    word: ZkX509ShaWordCapacityTraceV1,
    rfc_consumer: Option<ZkX509ShaRfcConsumerChannelsV1>,
    call_challenges: ZkX509ShaCallBusChallengesV1,
    rfc_challenges: &ZkX509Rfc5280StarkChallengesV1,
    initial_products: &ZkX509ShaSegmentProductStateV1,
) -> Result<ZkX509ShaBatchCallTraceV1, ZkX509ShaCallBusStarkErrorV1> {
    let mut product_rows = PrivateTableV1::new(Vec::new(), zeroize_field_rows_v1);
    product_rows
        .try_reserve_exact(word.logical_rows())
        .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?;
    let mut segment_product_state = initial_products.clone();
    let mut call_products = ZkX509ShaSegmentProductStateV1::one_v1();
    let mut scratch = ShaCallBindingScratchV1::zero_v1();
    for index in 0..word.logical_rows() {
        scratch.base = widened_sha_batch_base_row_v1(&word, rfc_consumer, index)?;
        let fixed = widened_sha_batch_fixed_row_v1(manifest, &word, rfc_consumer, index)?;
        let word_fixed = word.fixed_row(index)?;
        // Reserve and insert a zero row before its first private write; the
        // existing table guard clears it on every error and unwinding path.
        product_rows.push([F::ZERO; ZK_X509_SHA_PRODUCT_WIDTH_V1]);
        let products = product_rows
            .last_mut()
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)?;
        products[..ZK_X509_SHA_BUS_LANES_V1]
            .copy_from_slice(&segment_product_state.source_products);
        products[ZK_X509_SHA_BUS_LANES_V1..ZK_X509_SHA_CALL_PRODUCT_WIDTH_V1]
            .copy_from_slice(&segment_product_state.digest_products);
        for (stream, stream_products) in
            segment_product_state.rfc_stream_products.iter().enumerate()
        {
            let start = ZK_X509_SHA_CALL_PRODUCT_WIDTH_V1 + stream * ZK_X509_SHA_BUS_LANES_V1;
            products[start..start + ZK_X509_SHA_BUS_LANES_V1].copy_from_slice(stream_products);
        }
        products[ZK_X509_SHA_PRODUCT_WIDTH_V1 - 4..]
            .copy_from_slice(&segment_product_state.projection_products);
        for lane in 0..4 {
            scratch.set_factor_v1(Some(projection_bus::row_factor_v1(
                &scratch.base,
                word.aux_row(index)?[SHA_WORD_CAPACITY_MESSAGE_COUNT_V1],
                &fixed,
                call_challenges.lanes[lane],
            )));
            if index == 0 && scratch.factor != F::ONE {
                return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
            }
            segment_product_state.projection_products[lane] =
                segment_product_state.projection_products[lane].mul(scratch.factor);
            call_products.projection_products[lane] =
                call_products.projection_products[lane].mul(scratch.factor);
        }
        let input_event = word_fixed[SHA_WORD_CAPACITY_INPUT_WORD_V1] == F::ONE
            && scratch.base[SHA_WORD_CAPACITY_ROW_ACTIVE_V1] == F::ONE;
        let digest_event = word_fixed[SHA_WORD_CAPACITY_DIGEST_SELECTOR_V1] == F::ONE;
        if input_event == digest_event && input_event {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        if input_event {
            let word_index = usize::try_from(word_fixed[SHA_WORD_CAPACITY_INPUT_WORD_INDEX_V1].0)
                .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?;
            for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
                scratch.set_factor_v1(Some(call_row_factor_v1(
                    manifest,
                    ZkX509ShaCallWordKindV1::Input,
                    word_index,
                    scratch.base[0],
                    call_challenges.lanes[lane],
                )?));
                segment_product_state.source_products[lane] =
                    segment_product_state.source_products[lane].mul(scratch.factor);
                call_products.source_products[lane] =
                    call_products.source_products[lane].mul(scratch.factor);
            }
        } else if digest_event {
            let word_index = usize::try_from(word_fixed[SHA_WORD_CAPACITY_DIGEST_WORD_INDEX_V1].0)
                .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?;
            for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
                scratch.set_factor_v1(Some(call_row_factor_v1(
                    manifest,
                    ZkX509ShaCallWordKindV1::Digest,
                    word_index,
                    scratch.base[0],
                    call_challenges.lanes[lane],
                )?));
                segment_product_state.digest_products[lane] =
                    segment_product_state.digest_products[lane].mul(scratch.factor);
                call_products.digest_products[lane] =
                    call_products.digest_products[lane].mul(scratch.factor);
            }
        }
        for (stream, stream_products) in segment_product_state
            .rfc_stream_products
            .iter_mut()
            .enumerate()
        {
            for (lane, product) in stream_products.iter_mut().enumerate() {
                if scratch.set_factor_v1(sha_rfc_consumer_row_factor_v1(
                    &scratch.base,
                    &fixed,
                    stream,
                    lane,
                    rfc_challenges,
                )?) {
                    *product = product.mul(scratch.factor);
                    call_products.rfc_stream_products[stream][lane] =
                        call_products.rfc_stream_products[stream][lane].mul(scratch.factor);
                }
            }
        }
    }
    if product_rows.len() != word.logical_rows() {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    Ok(ZkX509ShaBatchCallTraceV1 {
        manifest,
        word,
        product_rows: product_rows.into_vec(),
        #[cfg(test)]
        rfc_consumer,
        terminal: ZkX509ShaCallTerminalV1 {
            call: manifest.call,
            role: manifest.role,
            source_products: call_products.source_products,
            digest_products: call_products.digest_products,
        },
        rfc_terminal: ZkX509ShaRfcConsumerTerminalV1 {
            call: manifest.call,
            role: manifest.role,
            stream_products: call_products.rfc_stream_products,
        },
        segment_product_state,
    })
}
#[cfg(test)]
fn bind_zk_x509_sha_batch_call_base_raw_for_test_v1(
    source: ZkX509ShaBatchCallBaseSourceV1,
    word_challenges: ZkX509ShaWordStarkChallengesV1,
    call_challenges: ZkX509ShaCallBusChallengesV1,
    rfc_challenges: &ZkX509Rfc5280StarkChallengesV1,
    initial_products: &ZkX509ShaSegmentProductStateV1,
) -> Result<ZkX509ShaBatchCallTraceV1, ZkX509ShaCallBusStarkErrorV1> {
    call_challenges.validate()?;
    rfc_challenges
        .validate()
        .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Challenge)?;
    validate_zk_x509_sha_word_stark_challenges_v1(word_challenges)?;
    let ZkX509ShaBatchCallBaseSourceV1 {
        manifest,
        word,
        rfc_consumer,
    } = source;
    finish_zk_x509_sha_batch_call_binding_v1(
        manifest,
        word.bind_challenges_for_test_v1(word_challenges)?,
        rfc_consumer,
        call_challenges,
        rfc_challenges,
        initial_products,
    )
}
#[cfg(test)]
fn build_zk_x509_sha_batch_call_trace_with_initial_products_v1(
    manifest: ZkX509ShaCallManifestV1,
    witness: &ZkX509ShaCallWitnessV1,
    word_challenges: ZkX509ShaWordStarkChallengesV1,
    call_challenges: ZkX509ShaCallBusChallengesV1,
    rfc_challenges: &ZkX509Rfc5280StarkChallengesV1,
    disclosed_attributes: usize,
    initial_products: &ZkX509ShaSegmentProductStateV1,
) -> Result<ZkX509ShaBatchCallTraceV1, ZkX509ShaCallBusStarkErrorV1> {
    let source =
        build_zk_x509_sha_batch_call_base_source_v1(manifest, witness, disclosed_attributes)?;
    bind_zk_x509_sha_batch_call_base_raw_for_test_v1(
        source,
        word_challenges,
        call_challenges,
        rfc_challenges,
        initial_products,
    )
}
/// Test-only focused call builder with caller-supplied challenge families.
#[cfg(test)]
pub fn build_zk_x509_sha_batch_call_trace_v1(
    manifest: ZkX509ShaCallManifestV1,
    witness: &ZkX509ShaCallWitnessV1,
    word_challenges: ZkX509ShaWordStarkChallengesV1,
    call_challenges: ZkX509ShaCallBusChallengesV1,
    rfc_challenges: &ZkX509Rfc5280StarkChallengesV1,
    disclosed_attributes: usize,
) -> Result<ZkX509ShaBatchCallTraceV1, ZkX509ShaCallBusStarkErrorV1> {
    let source =
        build_zk_x509_sha_batch_call_base_source_v1(manifest, witness, disclosed_attributes)?;
    bind_zk_x509_sha_batch_call_base_raw_for_test_v1(
        source,
        word_challenges,
        call_challenges,
        rfc_challenges,
        &ZkX509ShaSegmentProductStateV1::one_v1(),
    )
}
fn physical_padding_row_v1(_segment_row: usize) -> ZkX509ShaBatchRowV1 {
    let mut fixed = [F::ZERO; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1];
    fixed[ZK_X509_SHA_FIXED_PHYSICAL_PADDING_V1] = F::ONE;
    ZkX509ShaBatchRowV1 {
        base: [F::ZERO; ZK_X509_SHA_BATCH_BASE_WIDTH_V1],
        aux: [F::ZERO; ZK_X509_SHA_BATCH_AUX_WIDTH_V1],
        fixed,
    }
}
#[cfg(test)]
/// Evaluate one opened fixed-capacity batch row.
///
/// Calls never cross a physical segment boundary.  Every active segment
/// suffix ends on a call terminal and is followed by canonical padding, so
/// `next` is always the ordinary next opening from the same registration.
pub fn evaluate_zk_x509_sha_batch_residues_v1(
    current: &ZkX509ShaBatchRowV1,
    next: &ZkX509ShaBatchRowV1,
    word_challenges: ZkX509ShaWordStarkChallengesV1,
    call_challenges: ZkX509ShaCallBusChallengesV1,
    rfc_challenges: &ZkX509Rfc5280StarkChallengesV1,
    segment: u8,
    ca_call_boundaries: &[ZkX509ShaCallBoundaryTerminalV1; ZK_X509_SHA_CA_CALL_COUNT_V1],
) -> Result<Vec<F>, ZkX509ShaCallBusStarkErrorV1> {
    evaluate_zk_x509_sha_batch_residues_over_field_v1(
        current,
        next,
        word_challenges,
        call_challenges,
        rfc_challenges,
        segment,
        ca_call_boundaries,
    )
}
/// Complete polynomial SHA registration, including every call, RFC and CA-boundary constraint.
///
/// Public challenges and terminal claims embed from the base field; all opened
/// private and fixed columns stay in the evaluation field. No row value selects
/// a host-language branch. This does not authenticate the supplied openings.
#[cfg(test)]
pub fn evaluate_zk_x509_sha_batch_residues_over_field_v1<A: PolynomialAirFieldV1>(
    current: &ZkX509ShaBatchRowV1<A>,
    next: &ZkX509ShaBatchRowV1<A>,
    word_challenges: ZkX509ShaWordStarkChallengesV1,
    call_challenges: ZkX509ShaCallBusChallengesV1,
    rfc_challenges: &ZkX509Rfc5280StarkChallengesV1,
    segment: u8,
    ca_call_boundaries: &[ZkX509ShaCallBoundaryTerminalV1; ZK_X509_SHA_CA_CALL_COUNT_V1],
) -> Result<Vec<A>, ZkX509ShaCallBusStarkErrorV1> {
    evaluate_sha_batch_relation_v1(
        current,
        next,
        word_challenges,
        call_challenges,
        rfc_challenges,
        segment,
        Some(ca_call_boundaries),
    )
}

/// Complete local SHA relation before original-polynomial cross-proof joins.
/// This is not a standalone proof relation: joint MAIN must authenticate its
/// source/digest endpoints against the original compact-CA auxiliary FRI.
pub fn evaluate_zk_x509_sha_batch_local_residues_over_field_v1<A: PolynomialAirFieldV1>(
    current: &ZkX509ShaBatchRowV1<A>,
    next: &ZkX509ShaBatchRowV1<A>,
    word_challenges: ZkX509ShaWordStarkChallengesV1,
    call_challenges: ZkX509ShaCallBusChallengesV1,
    rfc_challenges: &ZkX509Rfc5280StarkChallengesV1,
    segment: u8,
) -> Result<Vec<A>, ZkX509ShaCallBusStarkErrorV1> {
    evaluate_sha_batch_relation_v1(
        current,
        next,
        word_challenges,
        call_challenges,
        rfc_challenges,
        segment,
        None,
    )
}

fn evaluate_sha_batch_relation_v1<A: PolynomialAirFieldV1>(
    current: &ZkX509ShaBatchRowV1<A>,
    next: &ZkX509ShaBatchRowV1<A>,
    word_challenges: ZkX509ShaWordStarkChallengesV1,
    call_challenges: ZkX509ShaCallBusChallengesV1,
    rfc_challenges: &ZkX509Rfc5280StarkChallengesV1,
    segment: u8,
    ca_call_boundaries: Option<&[ZkX509ShaCallBoundaryTerminalV1; ZK_X509_SHA_CA_CALL_COUNT_V1]>,
) -> Result<Vec<A>, ZkX509ShaCallBusStarkErrorV1> {
    let mut residues = Vec::new();
    emit_sha_batch_relation_v1(
        current,
        next,
        word_challenges,
        call_challenges,
        rfc_challenges,
        segment,
        ca_call_boundaries,
        &mut residues,
    )?;
    Ok(residues)
}

/// Emit ordered local or complete equations without retaining an output vector.
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_sha_batch_relation_v1<A: PolynomialAirFieldV1>(
    current: &ZkX509ShaBatchRowV1<A>,
    next: &ZkX509ShaBatchRowV1<A>,
    word_challenges: ZkX509ShaWordStarkChallengesV1,
    call_challenges: ZkX509ShaCallBusChallengesV1,
    rfc_challenges: &ZkX509Rfc5280StarkChallengesV1,
    segment: u8,
    ca_call_boundaries: Option<&[ZkX509ShaCallBoundaryTerminalV1; ZK_X509_SHA_CA_CALL_COUNT_V1]>,
    residues: &mut impl ShaResidueSinkV1<A>,
) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
    call_challenges.validate()?;
    rfc_challenges
        .validate()
        .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Challenge)?;
    if current
        .base
        .iter()
        .chain(&next.base)
        .chain(&current.aux)
        .chain(&next.aux)
        .chain(&current.fixed)
        .chain(&next.fixed)
        .any(|value| !value.is_canonical())
    {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    if usize::from(segment) >= ZK_X509_SHA_SEGMENT_COUNT_V1 {
        return Err(ZkX509ShaCallBusStarkErrorV1::Terminal);
    }
    if let Some(boundaries) = ca_call_boundaries {
        for (index, boundary) in boundaries.iter().copied().enumerate() {
            boundary.validate_identity_v1(index)?;
        }
    }
    let current_word_base: &[A; SHA_WORD_CAPACITY_BASE_WIDTH_V1] = current.base
        [..SHA_WORD_CAPACITY_BASE_WIDTH_V1]
        .try_into()
        .expect("word base prefix");
    let next_word_base: &[A; SHA_WORD_CAPACITY_BASE_WIDTH_V1] = next.base
        [..SHA_WORD_CAPACITY_BASE_WIDTH_V1]
        .try_into()
        .expect("word base prefix");
    let current_word_aux: &[A; SHA_WORD_CAPACITY_AUX_WIDTH_V1] = current.aux
        [..SHA_WORD_CAPACITY_AUX_WIDTH_V1]
        .try_into()
        .expect("word aux prefix");
    let next_word_aux: &[A; SHA_WORD_CAPACITY_AUX_WIDTH_V1] = next.aux
        [..SHA_WORD_CAPACITY_AUX_WIDTH_V1]
        .try_into()
        .expect("word aux prefix");
    let word_fixed: &[A; SHA_WORD_CAPACITY_FIXED_WIDTH_V1] = current.fixed
        [..SHA_WORD_CAPACITY_FIXED_WIDTH_V1]
        .try_into()
        .expect("word fixed prefix");
    emit_zk_x509_sha_word_capacity_residues_over_field_v1(
        current_word_base,
        next_word_base,
        current_word_aux,
        next_word_aux,
        word_fixed,
        word_challenges,
        residues,
    )?;
    for bit in 0..ZK_X509_SHA_RFC_LENGTH_BITS_V1 {
        let value = current.base[ZK_X509_SHA_LENGTH_BITS_START_V1 + bit];
        residues.push(value.mul(value.sub(A::ONE)));
    }
    // The same private raw length owns both the early frame header and the
    // final RFC length event. Carry it through every capacity row, including
    // inactive compression and memory rows, and stop at the call boundary.
    let within_call = A::ONE
        .sub(current.fixed[SHA_WORD_CAPACITY_CALL_LAST_V1])
        .sub(current.fixed[ZK_X509_SHA_FIXED_PHYSICAL_PADDING_V1]);
    for bit in 0..ZK_X509_SHA_RFC_LENGTH_BITS_V1 {
        residues.push(
            within_call.mul(
                next.base[ZK_X509_SHA_LENGTH_BITS_START_V1 + bit]
                    .sub(current.base[ZK_X509_SHA_LENGTH_BITS_START_V1 + bit]),
            ),
        );
    }
    let raw_low = pack_sha_rfc_bits_v1(
        &current.base[ZK_X509_SHA_LENGTH_BITS_START_V1..ZK_X509_SHA_LENGTH_BITS_START_V1 + 8],
    );
    let raw_high = pack_sha_rfc_bits_v1(
        &current.base[ZK_X509_SHA_LENGTH_BITS_START_V1 + 8
            ..ZK_X509_SHA_LENGTH_BITS_START_V1 + ZK_X509_SHA_RFC_LENGTH_BITS_V1],
    );
    for byte in 0..4 {
        let mask = current.fixed[ZK_X509_SHA_FIXED_FRAME_MASK_V1 + byte];
        let expected = current.fixed[ZK_X509_SHA_FIXED_FRAME_CONSTANT_V1 + byte]
            .add(current.fixed[ZK_X509_SHA_FIXED_FRAME_LENGTH_HIGH_V1 + byte].mul(raw_high))
            .add(current.fixed[ZK_X509_SHA_FIXED_FRAME_LENGTH_LOW_V1 + byte].mul(raw_low));
        let bits = 1 + (3 - byte) * 8;
        let actual = pack_sha_rfc_bits_v1(&current.base[bits..bits + 8]);
        residues.push(mask.mul(actual.sub(expected)));
        residues.push(mask.mul(current.base[SHA_WORD_CAPACITY_MESSAGE_MASK_V1 + byte].sub(A::ONE)));
    }
    residues.push(sha_rfc_length_recomposition_residue_v1(
        &current.base,
        &current.aux,
        &current.fixed,
    ));
    let input_event = current.fixed[SHA_WORD_CAPACITY_INPUT_WORD_V1]
        .mul(current.base[SHA_WORD_CAPACITY_ROW_ACTIVE_V1]);
    let digest_event = current.fixed[SHA_WORD_CAPACITY_DIGEST_SELECTOR_V1];
    let segment_last = current.fixed[ZK_X509_SHA_FIXED_SEGMENT_LAST_V1];
    let segment_first = current.fixed[ZK_X509_SHA_FIXED_SEGMENT_FIRST_V1];
    let padding = current.fixed[ZK_X509_SHA_FIXED_PHYSICAL_PADDING_V1];
    // The native domain is cyclic: its final zero padding row precedes the
    // first live row, whose products start at one. The public schedule makes
    // terminal and padding selectors disjoint, so this linear gate preserves
    // live recurrences without joining either boundary to zero padding.
    let continue_products = A::ONE.sub(segment_last).sub(padding);
    let manifest_call = current.fixed[ZK_X509_SHA_FIXED_CALL_V1];
    let manifest_role = current.fixed[ZK_X509_SHA_FIXED_ROLE_V1];
    let manifest_slot = current.fixed[ZK_X509_SHA_FIXED_SLOT_V1];
    let input_word = current.fixed[SHA_WORD_CAPACITY_INPUT_WORD_INDEX_V1];
    let digest_word = current.fixed[SHA_WORD_CAPACITY_DIGEST_WORD_INDEX_V1];
    for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
        let challenge = call_challenges.lanes[lane];
        let input_factor = compress_sha_call_fields_v1(
            manifest_call,
            manifest_role,
            manifest_slot,
            A::from_base(F(u64::from(ZkX509ShaCallWordKindV1::Input.code()))),
            input_word,
            current.base[0],
            challenge,
        );
        let digest_factor = compress_sha_call_fields_v1(
            manifest_call,
            manifest_role,
            manifest_slot,
            A::from_base(F(u64::from(ZkX509ShaCallWordKindV1::Digest.code()))),
            digest_word,
            current.base[0],
            challenge,
        );
        let input_before = current.aux[ZK_X509_SHA_INPUT_PRODUCTS_V1 + lane];
        let input_after = input_before.mul(A::ONE.add(input_event.mul(input_factor.sub(A::ONE))));
        let digest_before = current.aux[ZK_X509_SHA_DIGEST_PRODUCTS_V1 + lane];
        let digest_after =
            digest_before.mul(A::ONE.add(digest_event.mul(digest_factor.sub(A::ONE))));
        residues.push(
            continue_products.mul(next.aux[ZK_X509_SHA_INPUT_PRODUCTS_V1 + lane].sub(input_after)),
        );
        residues.push(
            continue_products
                .mul(next.aux[ZK_X509_SHA_DIGEST_PRODUCTS_V1 + lane].sub(digest_after)),
        );
        residues.push(segment_first.mul(input_before.sub(A::ONE)));
        residues.push(segment_first.mul(digest_before.sub(A::ONE)));
        // Whole-segment source/digest totals have no external consumer. Their
        // initial-one and live recurrence constraints remain, and compact-CA
        // boundaries below bind the actual call-local products without division.
        if let Some(boundaries) = ca_call_boundaries {
            for (boundary_index, boundary) in boundaries.iter().copied().enumerate() {
                let selector =
                    current.fixed[ZK_X509_SHA_FIXED_CA_CALL_SELECTORS_V1 + boundary_index];
                let call_first = current.fixed[SHA_WORD_CAPACITY_CALL_FIRST_V1].mul(selector);
                let call_last = current.fixed[SHA_WORD_CAPACITY_CALL_LAST_V1].mul(selector);
                residues.push(
                    call_first
                        .mul(input_before.sub(A::from_base(boundary.source_start_products[lane]))),
                );
                residues
                    .push(call_first.mul(
                        digest_before.sub(A::from_base(boundary.digest_start_products[lane])),
                    ));
                residues.push(call_last.mul(input_after.sub(A::from_base(
                    boundary.source_start_products[lane].mul(boundary.source_products[lane]),
                ))));
                residues.push(call_last.mul(digest_after.sub(A::from_base(
                    boundary.digest_start_products[lane].mul(boundary.digest_products[lane]),
                ))));
            }
        }
    }
    for stream in 0..ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1 {
        for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
            let before = current.aux
                [ZK_X509_SHA_RFC_CONSUMER_PRODUCTS_V1 + stream * ZK_X509_SHA_BUS_LANES_V1 + lane];
            let after = before.mul(A::ONE.add(sha_rfc_opened_event_delta_v1(
                &current.base,
                &current.fixed,
                stream,
                lane,
                rfc_challenges,
            )?));
            residues.push(
                continue_products.mul(
                    next.aux[ZK_X509_SHA_RFC_CONSUMER_PRODUCTS_V1
                        + stream * ZK_X509_SHA_BUS_LANES_V1
                        + lane]
                        .sub(after),
                ),
            );
            residues.push(segment_first.mul(before.sub(A::ONE)));
        }
    }
    residues.extend_residues(current.base.map(|value| padding.mul(value)));
    residues.extend_residues(
        current.aux[..projection_bus::AUX_START_V1]
            .iter()
            .copied()
            .map(|value| padding.mul(value)),
    );
    projection_bus::push_residues_v1(current, next, call_challenges, residues);
    let expected_count = if ca_call_boundaries.is_some() {
        ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1
    } else {
        ZK_X509_SHA_BATCH_LOCAL_CONSTRAINT_COUNT_V1
    };
    if residues.len() != expected_count {
        return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
    }
    Ok(())
}
/// Advance four products by one active event with degree-two transitions.
#[cfg(test)]
pub fn advance_sha_call_products_v1(
    before: [F; ZK_X509_SHA_BUS_LANES_V1],
    event: ZkX509ShaCallEventV1,
    challenges: ZkX509ShaCallBusChallengesV1,
) -> Result<[F; ZK_X509_SHA_BUS_LANES_V1], ZkX509ShaCallBusStarkErrorV1> {
    challenges.validate()?;
    if !event.active && event.value != 0 {
        return Err(ZkX509ShaCallBusStarkErrorV1::Event);
    }
    Ok(core::array::from_fn(|lane| {
        if event.active {
            before[lane].mul(compress_event_v1(event, challenges.lanes[lane]))
        } else {
            before[lane]
        }
    }))
}
/// Replay the aggregate product terminal over one endpoint.
#[cfg(test)]
pub fn sha_call_product_terminal_v1(
    schedule: &ZkX509ShaCallScheduleV1,
    witnesses: &[ZkX509ShaCallWitnessV1; ZK_X509_SHA_CALL_COUNT_V1],
    challenges: ZkX509ShaCallBusChallengesV1,
) -> Result<[F; ZK_X509_SHA_BUS_LANES_V1], ZkX509ShaCallBusStarkErrorV1> {
    let mut products = [F::ONE; ZK_X509_SHA_BUS_LANES_V1];
    for index in 0..ZK_X509_SHA_CALL_EVENT_COUNT_V1 {
        products = advance_sha_call_products_v1(
            products,
            replay_sha_call_event_v1(schedule, witnesses, index)?,
            challenges,
        )?;
    }
    Ok(products)
}
/// Derive the separate per-call source and digest terminal claims expected
/// from the producer and consumer adapters.
#[cfg(test)]
pub fn sha_call_terminals_v1(
    schedule: &ZkX509ShaCallScheduleV1,
    witnesses: &[ZkX509ShaCallWitnessV1; ZK_X509_SHA_CALL_COUNT_V1],
    challenges: ZkX509ShaCallBusChallengesV1,
) -> Result<[ZkX509ShaCallTerminalV1; ZK_X509_SHA_CALL_COUNT_V1], ZkX509ShaCallBusStarkErrorV1> {
    challenges.validate()?;
    let mut terminals = Vec::new();
    terminals
        .try_reserve_exact(ZK_X509_SHA_CALL_COUNT_V1)
        .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?;
    for manifest in schedule.calls().iter().copied() {
        let witness = witnesses
            .get(usize::from(manifest.call))
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)?;
        validate_witness_v1(manifest, witness)?;
        let mut source_products = [F::ONE; ZK_X509_SHA_BUS_LANES_V1];
        let mut digest_products = [F::ONE; ZK_X509_SHA_BUS_LANES_V1];
        for local_event in 0..manifest.maximum_events() {
            let event = replay_sha_call_event_v1(
                schedule,
                witnesses,
                manifest
                    .first_event
                    .checked_add(local_event)
                    .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?,
            )?;
            if !event.active {
                continue;
            }
            let products = match event.word_kind {
                ZkX509ShaCallWordKindV1::Input => &mut source_products,
                ZkX509ShaCallWordKindV1::Digest => &mut digest_products,
            };
            for (lane, product) in products.iter_mut().enumerate() {
                *product = product.mul(compress_event_v1(event, challenges.lanes[lane]));
            }
        }
        terminals.push(ZkX509ShaCallTerminalV1 {
            call: manifest.call,
            role: manifest.role,
            source_products,
            digest_products,
        });
    }
    terminals
        .try_into()
        .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Topology)
}
/// Enforce producer/SHA endpoint equality.
#[cfg(test)]
pub fn evaluate_sha_call_terminal_constraints_v1(
    source: [F; ZK_X509_SHA_BUS_LANES_V1],
    sha: [F; ZK_X509_SHA_BUS_LANES_V1],
) -> [F; ZK_X509_SHA_BUS_LANES_V1] {
    core::array::from_fn(|lane| source[lane].sub(sha[lane]))
}
/// Maximum typed events in the Projection-to-SHA message/digest equality.
pub const ZK_X509_SHA_PROJECTION_EVENT_COUNT_V1: u64 = 7 * (512 + 1 + 8);
/// Conservative numerator covering both SHA call and Projection-to-SHA lists.
///
/// For fixed unequal lists the separate four-lane escape terms are bounded by
/// `(10088/d)^4 + (3647/d)^4 <= (13735/d)^4`. This arithmetic does not establish
/// the adaptive joint-protocol or Fiat-Shamir reduction.
pub const ZK_X509_SHA_CALL_BUS_COLLISION_NUMERATOR_V1: u64 =
    ZK_X509_SHA_CALL_EVENT_COUNT_V1 as u64 + ZK_X509_SHA_PROJECTION_EVENT_COUNT_V1;
/// Exact algebraic collision-bound numerator for each word-memory equality.
pub const ZK_X509_SHA_WORD_MEMORY_COLLISION_NUMERATOR_V1: u64 =
    ZK_X509_SHA_MAX_MEMORY_ROWS_V1 as u64;
/// Exact algebraic collision-bound numerator for the three-lane base-fold bus.
pub const ZK_X509_SHA_BASE_FOLD_COLLISION_NUMERATOR_V1: u64 = 440;
/// Independent challenge lanes used by each SHA word/call equality.
pub const ZK_X509_SHA_COLLISION_LANES_V1: u8 = 4;
/// Independent challenge lanes used by the SHA base-fold equality.
pub const ZK_X509_SHA_BASE_FOLD_COLLISION_LANES_V1: u8 = 3;
/// Return conditional fixed-list collision arithmetic in diagnostic form.
///
/// Two word-memory equalities have numerator `1_316_240` each. The existing
/// call list has `10_088` events and the new projection list at most `3_647`.
/// Their fourth-power terms are conservatively covered by the combined call
/// numerator `13_735`. Four independent lane draws are a premise of this local
/// calculation; it does not discharge the adaptive joint-security obligation.
/// Splitting the RFC consumer stream adds no new event because its terminals
/// are multiplied before the already-counted RFC output equality.
#[cfg(test)]
fn algebraic_security_bits_v1() -> (f64, f64, f64) {
    let denominator = 2_f64.powi(64) - 2_f64.powi(32);
    let memory = (f64::from(
        u32::try_from(ZK_X509_SHA_WORD_MEMORY_COLLISION_NUMERATOR_V1)
            .expect("word collision numerator fits u32"),
    ) / denominator)
        .powi(4);
    let call = (f64::from(
        u32::try_from(ZK_X509_SHA_CALL_BUS_COLLISION_NUMERATOR_V1)
            .expect("call collision numerator fits u32"),
    ) / denominator)
        .powi(4);
    let bus_union = 2.0f64.mul_add(memory, call);
    let base_fold = (f64::from(
        u32::try_from(ZK_X509_SHA_BASE_FOLD_COLLISION_NUMERATOR_V1)
            .expect("fold collision numerator fits u32"),
    ) / denominator)
        .powi(i32::from(ZK_X509_SHA_BASE_FOLD_COLLISION_LANES_V1));
    (
        -bus_union.log2(),
        -call.log2(),
        -(bus_union + base_fold).log2(),
    )
}
#[cfg(test)]
mod tests {
    include!("sha_base_projection_tests.rs");
    include!("sha_native_aux_cache_source_tests.rs");
    include!("sha_batch_residue_oracle.rs");
    include!("sha_residue_output_tests.rs");
    include!("sha_native_base_cache_source_tests.rs");
    include!("sha_fixed_schedule_reuse_tests.rs");
    include!("sha_fixed_matrix_reuse_tests.rs");
    #[test]
    fn local_sha_relation_preserves_every_nonpublic_residue_for_all_segments() {
        fn compare<A: PolynomialAirFieldV1 + core::fmt::Debug + PartialEq>(
            embed: impl Fn(usize) -> A,
        ) {
            for seed in 0..8 {
                let current = ZkX509ShaBatchRowV1 {
                    base: core::array::from_fn(|i| embed(seed + i)),
                    aux: core::array::from_fn(|i| embed(seed + 7 + i)),
                    fixed: core::array::from_fn(|i| embed(seed + 11 + i)),
                };
                let next = ZkX509ShaBatchRowV1 {
                    base: core::array::from_fn(|i| embed(seed + 17 + i)),
                    aux: core::array::from_fn(|i| embed(seed + 23 + i)),
                    fixed: core::array::from_fn(|i| embed(seed + 29 + i)),
                };
                for segment in 0..4 {
                    let full = evaluate_zk_x509_sha_batch_residues_over_field_v1(
                        &current,
                        &next,
                        word_challenges(),
                        challenges(),
                        &rfc_challenges(),
                        segment,
                        &neutral_ca_call_boundaries(),
                    )
                    .unwrap();
                    let local = evaluate_zk_x509_sha_batch_local_residues_over_field_v1(
                        &current,
                        &next,
                        word_challenges(),
                        challenges(),
                        &rfc_challenges(),
                        segment,
                    )
                    .unwrap();
                    let prefix = SHA_WORD_CAPACITY_CONSTRAINT_COUNT_V1
                        + 2 * ZK_X509_SHA_RFC_LENGTH_BITS_V1
                        + 8
                        + 1;
                    let expected = full
                        .iter()
                        .enumerate()
                        .filter_map(|(index, value)| {
                            let relative = index.saturating_sub(prefix);
                            let public = index >= prefix && relative < 4 * 56 && relative % 56 >= 4;
                            (!public).then_some(*value)
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(full.len() - expected.len(), 208);
                    assert_eq!(local.len(), ZK_X509_SHA_BATCH_LOCAL_CONSTRAINT_COUNT_V1);
                    assert_eq!(local, expected);
                }
            }
        }
        compare(|i| F((i % 17) as u64));
        compare(|i| {
            crate::privacy_engines::transparent_stark::GoldilocksFp4V1::canonical([
                (i % 17) as u64,
                (i % 11) as u64,
                (i % 7) as u64,
                (i % 5) as u64,
            ])
            .unwrap()
        });
    }
    include!("sha_call_bus_rfc_binding_tests.rs");
    include!("sha_frame_prefix_tests.rs");
    include!("sha_projection_bus_tests.rs");
    include!("sha_cyclic_padding_tests.rs");
    #[test]
    fn private_ca_boundary_owners_clear_success_error_unwind_and_segment_transfer() {
        use super::super::private_table::inspection;
        fn boundary() -> ZkX509ShaCallBoundaryTerminalV1 {
            ZkX509ShaCallBoundaryTerminalV1 {
                call: u8::try_from(ZK_X509_SHA_CA_LEAF_CALL_V1)
                    .expect("protocol-bounded index fits u8"),
                role: manifest_role_v1(ZK_X509_SHA_CA_LEAF_CALL_V1).unwrap(),
                source_start_products: [F(19); 4],
                digest_start_products: [F(23); 4],
                source_products: [F(29); 4],
                digest_products: [F(31); 4],
            }
        }
        for exit in 0..3 {
            let (result, observations) = inspection::observe_v1(|| {
                std::panic::catch_unwind(|| -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
                    let scratch = zeroize::Zeroizing::new(boundary());
                    assert!(format!("{scratch:?}").contains("redacted"));
                    let mut products =
                        PrivateTableV1::new(Vec::with_capacity(1), clear_ca_boundary_products_v1);
                    products.push(*scratch);
                    if exit == 1 {
                        return Err(ZkX509ShaCallBusStarkErrorV1::Terminal);
                    }
                    assert!(exit != 2, "private boundary unwind control");
                    let owner = ZkX509ShaSegmentAirTerminalsV1 {
                        segment: ZkX509ShaSegmentPrivateEndpointV1 {
                            segment: 3,
                            rfc_stream_products: [[F(37); 4]; 4],
                        },
                        ca_call_boundaries: products.into_vec(),
                    };
                    assert!(format!("{owner:?}").contains("redacted"));
                    let segment = owner.into_segment_v1();
                    assert_eq!(segment.segment, 3);
                    assert_eq!(segment.rfc_stream_products, [[F(37); 4]; 4]);
                    Ok(())
                })
            });
            assert_eq!(result.is_err(), exit == 2);
            if let Ok(inner) = result {
                assert_eq!(inner.is_err(), exit == 1);
            }
            assert!(
                observations
                    .iter()
                    .map(|entry| entry.nonzero_before)
                    .sum::<usize>()
                    >= 32
            );
            assert!(observations.iter().all(|entry| entry.nonzero_after == 0));
        }
    }

    #[test]
    fn retained_sha_witness_erasure_clears_live_message_and_digest_cells() {
        let mut witness = ZkX509ShaCallWitnessV1 {
            role: ZkX509ShaCallRoleV1::CertificateTbs(0),
            message: vec![0x5a; 37],
            digest: [0x73; 32],
        };
        let capacity = witness.message.capacity();
        witness.zeroize_private_cells_v1();
        assert_eq!(witness.message, vec![0; 37]);
        assert_eq!(witness.message.capacity(), capacity);
        assert_eq!(witness.digest, [0; 32]);
        witness.zeroize_private_v1();
        assert!(witness.private_is_zeroized_v1());
    }

    use super::*;
    use crate::privacy_engines::zk_x509::{
        credential_pre_aux::{
            ZkX509CredentialMainPreAuxV1, ZkX509CredentialPreAuxBindingV1,
            derive_zk_x509_credential_pre_aux_binding_v1,
        },
        stark::zk_x509_test_digest384_v1,
    };
    fn challenges() -> ZkX509ShaCallBusChallengesV1 {
        let mut next = 11_u64;
        ZkX509ShaCallBusChallengesV1 {
            lanes: core::array::from_fn(|_| ZkX509ShaCallBusLaneChallengesV1 {
                terms: core::array::from_fn(|_| {
                    let value = F(next);
                    next += 2;
                    value
                }),
            }),
        }
    }
    fn rfc_challenges() -> ZkX509Rfc5280StarkChallengesV1 {
        let mut next = 1_001_u64;
        ZkX509Rfc5280StarkChallengesV1 {
            tuple: core::array::from_fn(|_| {
                core::array::from_fn(|_| {
                    let value = F(next);
                    next += 1;
                    value
                })
            }),
        }
    }
    fn word_challenges() -> ZkX509ShaWordStarkChallengesV1 {
        ZkX509ShaWordStarkChallengesV1 {
            memory: ZkX509WordMemoryChallengesV1 {
                lanes: [
                    ZkX509WordMemoryLaneChallengesV1 {
                        beta: F(101),
                        address: F(103),
                        value: F(107),
                        is_write: F(109),
                    },
                    ZkX509WordMemoryLaneChallengesV1 {
                        beta: F(113),
                        address: F(127),
                        value: F(131),
                        is_write: F(137),
                    },
                    ZkX509WordMemoryLaneChallengesV1 {
                        beta: F(139),
                        address: F(149),
                        value: F(151),
                        is_write: F(157),
                    },
                    ZkX509WordMemoryLaneChallengesV1 {
                        beta: F(163),
                        address: F(167),
                        value: F(173),
                        is_write: F(179),
                    },
                ],
            },
            base_folding: [F(181), F(191), F(193), F(197)],
        }
    }
    fn credential_binding(seed: u8) -> ZkX509CredentialPreAuxBindingV1 {
        let main = ZkX509CredentialMainPreAuxV1::fixture_for_test_v1(
            [seed; 32],
            [seed.wrapping_add(1); 32],
            core::array::from_fn(|index| {
                zk_x509_test_digest384_v1(
                    seed.wrapping_add(u8::try_from(index).expect("six roots")),
                )
            }),
        );
        derive_zk_x509_credential_pre_aux_binding_v1(
            main,
            zk_x509_test_digest384_v1(seed.wrapping_add(0x20)),
            zk_x509_test_digest384_v1(seed.wrapping_add(0x30)),
            zk_x509_test_digest384_v1(seed.wrapping_add(0x40)),
        )
        .expect("credential X5B1 binding")
    }
    #[test]
    fn private_segment_endpoint_excludes_unused_source_and_digest_totals() {
        for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
            let original = ZkX509ShaSegmentProductStateV1::one_v1();
            let public = original.terminal_v1(segment).unwrap();
            for family in 0..2 {
                for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
                    for value in [
                        F::ZERO,
                        F(17),
                        F(crate::privacy_engines::transparent_stark::GOLDILOCKS_MODULUS_V1 - 1),
                    ] {
                        let mut changed = original.clone();
                        let products = if family == 0 {
                            &mut changed.source_products
                        } else {
                            &mut changed.digest_products
                        };
                        products[lane] = value;
                        assert_ne!(changed, original);
                        assert_eq!(changed.terminal_v1(segment).unwrap(), public);
                    }
                }
            }
            for stream in 0..ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1 {
                for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
                    let mut changed = original.clone();
                    changed.rfc_stream_products[stream][lane] = F::ZERO;
                    assert_ne!(changed.terminal_v1(segment).unwrap(), public);
                }
            }
        }
    }
    include!("sha_private_product_owner_tests.rs");
    include!("sha_base_borrowed_projection_tests.rs");
    fn standalone_segment_terminal(_call: &ZkX509ShaBatchCallTraceV1) -> u8 {
        0
    }

    fn neutral_ca_call_boundaries()
    -> [ZkX509ShaCallBoundaryTerminalV1; ZK_X509_SHA_CA_CALL_COUNT_V1] {
        core::array::from_fn(|index| {
            let (call, role) =
                zk_x509_sha_ca_call_identity_v1(index).expect("canonical compact-CA call");
            ZkX509ShaCallBoundaryTerminalV1 {
                call,
                role,
                source_start_products: [F::ONE; ZK_X509_SHA_BUS_LANES_V1],
                digest_start_products: [F::ONE; ZK_X509_SHA_BUS_LANES_V1],
                source_products: [F::ONE; ZK_X509_SHA_BUS_LANES_V1],
                digest_products: [F::ONE; ZK_X509_SHA_BUS_LANES_V1],
            }
        })
    }
    fn ca_call_boundaries_for_calls(
        calls: &[&ZkX509ShaBatchCallTraceV1],
    ) -> [ZkX509ShaCallBoundaryTerminalV1; ZK_X509_SHA_CA_CALL_COUNT_V1] {
        let mut boundaries = neutral_ca_call_boundaries();
        for call in calls {
            let Some(call_index) = usize::from(call.manifest.call)
                .checked_sub(ZK_X509_SHA_CA_LEAF_CALL_V1)
                .filter(|index| *index < ZK_X509_SHA_CA_CALL_COUNT_V1)
            else {
                continue;
            };
            let first = call.aux_row(0).expect("first compact-CA auxiliary row");
            boundaries[call_index] = ZkX509ShaCallBoundaryTerminalV1 {
                call: call.manifest.call,
                role: call.manifest.role,
                source_start_products: first[ZK_X509_SHA_INPUT_PRODUCTS_V1
                    ..ZK_X509_SHA_INPUT_PRODUCTS_V1 + ZK_X509_SHA_BUS_LANES_V1]
                    .try_into()
                    .expect("source start width"),
                digest_start_products: first[ZK_X509_SHA_DIGEST_PRODUCTS_V1
                    ..ZK_X509_SHA_DIGEST_PRODUCTS_V1 + ZK_X509_SHA_BUS_LANES_V1]
                    .try_into()
                    .expect("digest start width"),
                source_products: call.terminal.source_products,
                digest_products: call.terminal.digest_products,
            };
        }
        boundaries
    }
    fn witness_for(manifest: ZkX509ShaCallManifestV1) -> ZkX509ShaCallWitnessV1 {
        let mut message = if manifest.activation == ZkX509ShaCallActivationV1::Inactive {
            Vec::new()
        } else {
            match manifest.role {
                ZkX509ShaCallRoleV1::CrlIssuerSpki => {
                    let spki = [0x31; ZK_X509_CA_SPKI_DER_BYTES_V1];
                    crl_issuer_spki_preimage_v1(&spki).expect("issuer frame")
                }
                ZkX509ShaCallRoleV1::CaLeaf => {
                    let spki = [0x32; ZK_X509_CA_SPKI_DER_BYTES_V1];
                    ca_leaf_preimage_v1(&spki).expect("leaf frame")
                }
                ZkX509ShaCallRoleV1::CaNode(level) => ca_node_preimage_v1(
                    usize::from(level),
                    &[level; 32],
                    &[level.wrapping_add(1); 32],
                )
                .expect("node frame"),
                _ => vec![manifest.call.wrapping_add(1); manifest.maximum_message_bytes],
            }
        };
        // These component fixtures keep private fields synthetic while using
        // the actual canonical public frame bytes required by the SHA AIR.
        let raw_len = message
            .len()
            .saturating_sub(sha_rfc_framed_field_offset_v1(manifest.role).unwrap_or(0));
        for (offset, value) in message.iter_mut().enumerate() {
            if let Some(expected) = sha_frame_byte_v1(manifest, offset) {
                *value = match expected {
                    ShaFrameByteV1::Constant(value) => value,
                    ShaFrameByteV1::RawLengthHigh => (raw_len >> 8) as u8,
                    ShaFrameByteV1::RawLengthLow => raw_len as u8,
                };
            }
        }
        let digest = Sha256::digest(&message).into();
        ZkX509ShaCallWitnessV1 {
            role: manifest.role,
            message,
            digest,
        }
    }
    fn witnesses(
        schedule: &ZkX509ShaCallScheduleV1,
    ) -> [ZkX509ShaCallWitnessV1; ZK_X509_SHA_CALL_COUNT_V1] {
        core::array::from_fn(|call| witness_for(schedule.calls[call]))
    }
    fn native_column_v1(value: F) -> Vec<F> {
        vec![value; ZK_X509_SHA_SEGMENT_ROWS_V1]
    }
    fn field_column_digest_v1(column: &[F]) -> [u8; 32] {
        let mut hash = Sha256::new();
        for value in column {
            hash.update(value.0.to_be_bytes());
        }
        hash.finalize().into()
    }
    #[test]
    fn segment_phase_rejects_aux_before_token_retries_failed_bind_and_rejects_duplicate_bind() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 2,
        })
        .expect("schedule");
        let witnesses = witnesses(&schedule);
        let mut source = ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, 0)
            .expect("base phase");
        assert!(
            !source.is_bound_for_test_v1(),
            "auxiliary phase exists before X5B1"
        );
        let mut noncanonical_word = word_challenges();
        noncanonical_word.memory.lanes[0].beta = F(u64::MAX);
        assert!(
            source
                .validate_bind_for_test_v1(noncanonical_word, challenges(), &rfc_challenges())
                .is_err()
        );
        assert!(
            !source.is_bound_for_test_v1(),
            "failed validation consumed the retry capability"
        );
        let binding = credential_binding(0x21);
        let aux = source.bind_v1(&binding).expect("retry with canonical X5B1");
        assert!(source.is_bound_for_test_v1());
        assert!(matches!(
            source.bind_v1(&credential_binding(0x22)),
            Err(ZkX509ShaCallBusStarkErrorV1::Phase)
        ));
        assert!(matches!(
            source.base_fixed_row_v1(0),
            Err(ZkX509ShaCallBusStarkErrorV1::Phase)
        ));
        drop(aux);
    }
    #[test]
    fn segment_base_source_rejects_wrong_schedule_witness_segment_and_row() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 1,
        })
        .expect("schedule");
        let witnesses = witnesses(&schedule);
        assert!(matches!(
            ZkX509ShaBatchSegmentBaseSourceV1::new_v1(
                &schedule,
                &witnesses,
                ZK_X509_SHA_SEGMENT_COUNT_V1,
            ),
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        ));
        let mut wrong_witnesses = witnesses.clone();
        wrong_witnesses[0].digest[0] ^= 1;
        assert!(matches!(
            ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &wrong_witnesses, 0),
            Err(ZkX509ShaCallBusStarkErrorV1::Digest)
        ));
        let mut wrong_schedule = schedule.clone();
        wrong_schedule.calls.swap(0, 1);
        assert!(ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&wrong_schedule, &witnesses, 0).is_err());
        let source = ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, 0)
            .expect("canonical source");
        assert!(matches!(
            source.base_fixed_row_v1(ZK_X509_SHA_SEGMENT_ROWS_V1),
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        ));
        for row in [0, source.replay.active_rows() - 1] {
            let (base, fixed) = source.base_fixed_row_v1(row).expect("canonical row");
            assert!(
                base.iter()
                    .chain(&fixed)
                    .all(|value| F::canonical(value.0).is_some())
            );
        }
    }
    #[test]
    fn segment_column_replay_matches_representative_row_transposes_across_all_four_segments() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 2,
        })
        .expect("schedule");
        let witnesses = witnesses(&schedule);
        let base_columns = [0, ZK_X509_SHA_BATCH_BASE_WIDTH_V1 - 1];
        let fixed_columns = [0, ZK_X509_SHA_BATCH_FIXED_WIDTH_V1 - 1];
        let aux_columns = [
            0,
            ZK_X509_SHA_INPUT_PRODUCTS_V1,
            ZK_X509_SHA_BATCH_AUX_WIDTH_V1 - 1,
        ];
        for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
            let mut source =
                ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, segment)
                    .expect("segment base source");
            let mut expected_base =
                base_columns.map(|_| Vec::with_capacity(ZK_X509_SHA_SEGMENT_ROWS_V1));
            let mut expected_fixed =
                fixed_columns.map(|_| Vec::with_capacity(ZK_X509_SHA_SEGMENT_ROWS_V1));
            source
                .for_each_base_fixed_row_v1(|row, base, fixed| {
                    assert_eq!(row, expected_base[0].len());
                    for (target, column) in expected_base.iter_mut().zip(base_columns) {
                        target.push(base[column]);
                    }
                    for (target, column) in expected_fixed.iter_mut().zip(fixed_columns) {
                        target.push(fixed[column]);
                    }
                })
                .expect("row-oriented base/fixed replay");
            for (expected, column) in expected_base.iter().zip(base_columns) {
                let mut replayed = native_column_v1(F(u64::MAX));
                source
                    .fill_base_column_v1(segment, column, &mut replayed)
                    .expect("column-oriented base replay");
                assert_eq!(&replayed, expected);
                assert!(replayed.iter().all(|value| F::canonical(value.0).is_some()));
            }
            for (expected, column) in expected_fixed.iter().zip(fixed_columns) {
                let mut replayed = native_column_v1(F(u64::MAX));
                source
                    .fill_fixed_column_v1(segment, column, &mut replayed)
                    .expect("column-oriented fixed replay");
                assert_eq!(&replayed, expected);
                assert!(replayed.iter().all(|value| F::canonical(value.0).is_some()));
            }
            let mut aux = source
                .bind_v1(&credential_binding(
                    0x51_u8.wrapping_add(u8::try_from(segment).expect("four segments")),
                ))
                .expect("bound segment source");
            let mut expected_aux =
                aux_columns.map(|_| Vec::with_capacity(ZK_X509_SHA_SEGMENT_ROWS_V1));
            let expected_terminals = aux
                .for_each_aux_row_with_air_terminals_v1(|row, values| {
                    assert_eq!(row, expected_aux[0].len());
                    for (target, column) in expected_aux.iter_mut().zip(aux_columns) {
                        target.push(values[column]);
                    }
                })
                .expect("row-oriented auxiliary replay");
            for (expected, column) in expected_aux.iter().zip(aux_columns) {
                let mut replayed = native_column_v1(F(u64::MAX));
                let terminals = aux
                    .fill_aux_column_with_air_terminals_v1(segment, column, &mut replayed)
                    .expect("column-oriented auxiliary replay");
                assert_eq!(&replayed, expected);
                assert_eq!(terminals, expected_terminals);
                assert!(replayed.iter().all(|value| F::canonical(value.0).is_some()));
            }
            assert!(aux.row_stream_emitted_for_test_v1());
            assert!(matches!(
                aux.for_each_aux_row_v1(|_, _| {}),
                Err(ZkX509ShaCallBusStarkErrorV1::Phase)
            ));
        }
    }
    #[test]
    #[ignore = "release-scale all-column SHA transpose KAT"]
    fn segment_column_replay_matches_every_row_column_transpose_across_all_four_segments() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 2,
        })
        .expect("schedule");
        let witnesses = witnesses(&schedule);
        for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
            let mut source =
                ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, segment)
                    .expect("segment base source");
            let mut base_hashes = vec![Sha256::new(); ZK_X509_SHA_BATCH_BASE_WIDTH_V1];
            let mut fixed_hashes = vec![Sha256::new(); ZK_X509_SHA_BATCH_FIXED_WIDTH_V1];
            source
                .for_each_base_fixed_row_v1(|_, base, fixed| {
                    for (hash, value) in base_hashes.iter_mut().zip(base) {
                        hash.update(value.0.to_be_bytes());
                    }
                    for (hash, value) in fixed_hashes.iter_mut().zip(fixed) {
                        hash.update(value.0.to_be_bytes());
                    }
                })
                .expect("base/fixed row replay");
            let base_hashes = base_hashes
                .into_iter()
                .map(|hash| <[u8; 32]>::from(hash.finalize()))
                .collect::<Vec<_>>();
            let fixed_hashes = fixed_hashes
                .into_iter()
                .map(|hash| <[u8; 32]>::from(hash.finalize()))
                .collect::<Vec<_>>();
            for (column, expected) in base_hashes.iter().enumerate() {
                let mut replayed = native_column_v1(F::ZERO);
                source
                    .fill_base_column_v1(segment, column, &mut replayed)
                    .expect("every base column");
                assert_eq!(&field_column_digest_v1(&replayed), expected);
            }
            for (column, expected) in fixed_hashes.iter().enumerate() {
                let mut replayed = native_column_v1(F::ZERO);
                source
                    .fill_fixed_column_v1(segment, column, &mut replayed)
                    .expect("every fixed column");
                assert_eq!(&field_column_digest_v1(&replayed), expected);
            }
            let mut aux = source
                .bind_v1(&credential_binding(
                    0x61_u8.wrapping_add(u8::try_from(segment).expect("four segments")),
                ))
                .expect("bound segment source");
            let mut aux_hashes = vec![Sha256::new(); ZK_X509_SHA_BATCH_AUX_WIDTH_V1];
            aux.for_each_aux_row_v1(|_, values| {
                for (hash, value) in aux_hashes.iter_mut().zip(values) {
                    hash.update(value.0.to_be_bytes());
                }
            })
            .expect("auxiliary row replay");
            let aux_hashes = aux_hashes
                .into_iter()
                .map(|hash| <[u8; 32]>::from(hash.finalize()))
                .collect::<Vec<_>>();
            for (column, expected) in aux_hashes.iter().enumerate() {
                let mut replayed = native_column_v1(F::ZERO);
                aux.fill_aux_column_v1(segment, column, &mut replayed)
                    .expect("every auxiliary column");
                assert_eq!(&field_column_digest_v1(&replayed), expected);
            }
        }
    }
    #[test]
    fn segment_base_columns_ignore_tokens_while_aux_columns_are_sensitive_and_replayable() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 2,
        })
        .expect("schedule");
        let witnesses = witnesses(&schedule);
        let fixed = ZkX509ShaBatchFixedProviderV1::new_v1(schedule.shape()).unwrap();
        let segment = 0;
        let mut first = ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, segment)
            .expect("first base source");
        let mut second = ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, segment)
            .expect("second base source");
        let mut first_base = native_column_v1(F::ZERO);
        let mut second_base = native_column_v1(F::ZERO);
        first
            .fill_base_column_v1(segment, 0, &mut first_base)
            .expect("first base column");
        second
            .fill_base_column_v1(segment, 0, &mut second_base)
            .expect("second base column");
        assert_eq!(first_base, second_base);
        let mut first_fixed = native_column_v1(F::ZERO);
        let mut second_fixed = native_column_v1(F::ZERO);
        first
            .fill_fixed_column_v1(segment, ZK_X509_SHA_FIXED_CALL_V1, &mut first_fixed)
            .expect("first fixed column");
        second
            .fill_fixed_column_v1(segment, ZK_X509_SHA_FIXED_CALL_V1, &mut second_fixed)
            .expect("second fixed column");
        assert_eq!(first_fixed, second_fixed);
        let first_aux = first
            .bind_v1(&credential_binding(0x71))
            .expect("first binding");
        let second_aux = second
            .bind_v1(&credential_binding(0x72))
            .expect("second binding");
        let mut replayed_base = native_column_v1(F::ZERO);
        first_aux
            .replay_base_column_v1(&fixed, segment, 0, &mut replayed_base)
            .expect("bound capability replays original base column");
        assert_eq!(replayed_base, first_base);
        second_aux
            .replay_base_column_v1(&fixed, segment, 0, &mut replayed_base)
            .expect("base replay ignores auxiliary token values");
        assert_eq!(replayed_base, first_base);
        let mut first_column = native_column_v1(F::ZERO);
        let mut first_replay = native_column_v1(F::ZERO);
        let mut second_column = native_column_v1(F::ZERO);
        let first_terminal = first_aux
            .fill_aux_column_v1(segment, ZK_X509_SHA_INPUT_PRODUCTS_V1, &mut first_column)
            .expect("first auxiliary column");
        let replay_terminal = first_aux
            .fill_aux_column_v1(segment, ZK_X509_SHA_INPUT_PRODUCTS_V1, &mut first_replay)
            .expect("deterministic auxiliary replay");
        second_aux
            .fill_aux_column_v1(segment, ZK_X509_SHA_INPUT_PRODUCTS_V1, &mut second_column)
            .expect("second auxiliary column");
        assert_eq!(first_column, first_replay);
        assert_eq!(first_terminal, replay_terminal);
        assert_ne!(first_column, second_column);
        assert!(!first_aux.row_stream_emitted_for_test_v1());
        assert!(!second_aux.row_stream_emitted_for_test_v1());
    }
    #[test]
    fn segment_column_requests_reject_invalid_identity_width_and_phase_without_state_change() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 1,
        })
        .expect("schedule");
        let witnesses = witnesses(&schedule);
        let fixed = ZkX509ShaBatchFixedProviderV1::new_v1(schedule.shape()).unwrap();
        let segment = 1;
        let mut source = ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, segment)
            .expect("base source");
        let sentinel = F(0x5A);
        let mut target = native_column_v1(sentinel);
        assert!(
            source
                .fill_base_column_v1(segment + 1, 0, &mut target)
                .is_err()
        );
        assert!(target.iter().all(|value| *value == sentinel));
        assert!(!source.is_bound_for_test_v1());
        assert!(
            source
                .fill_base_column_v1(segment, ZK_X509_SHA_BATCH_BASE_WIDTH_V1, &mut target,)
                .is_err()
        );
        assert!(target.iter().all(|value| *value == sentinel));
        assert!(
            source
                .fill_fixed_column_v1(segment, ZK_X509_SHA_BATCH_FIXED_WIDTH_V1, &mut target,)
                .is_err()
        );
        assert!(target.iter().all(|value| *value == sentinel));
        let mut short = vec![sentinel; ZK_X509_SHA_SEGMENT_ROWS_V1 - 1];
        assert!(source.fill_base_column_v1(segment, 0, &mut short).is_err());
        assert!(short.iter().all(|value| *value == sentinel));
        assert!(!source.is_bound_for_test_v1());
        let mut aux = source
            .bind_v1(&credential_binding(0x79))
            .expect("bound source");
        assert!(source.fill_base_column_v1(segment, 0, &mut target).is_err());
        assert!(target.iter().all(|value| *value == sentinel));
        let original_binding = aux.binding;
        assert!(
            aux.replay_base_column_v1(&fixed, segment + 1, 0, &mut target)
                .is_err()
        );
        assert!(
            aux.replay_base_column_v1(
                &fixed,
                segment,
                ZK_X509_SHA_BATCH_BASE_WIDTH_V1,
                &mut target
            )
            .is_err()
        );
        assert!(
            aux.replay_base_column_v1(&fixed, segment, 0, &mut short)
                .is_err()
        );
        assert!(target.iter().all(|value| *value == sentinel));
        assert!(!aux.row_stream_emitted_for_test_v1());
        assert!(aux.fill_aux_column_v1(segment + 1, 0, &mut target).is_err());
        assert!(target.iter().all(|value| *value == sentinel));
        assert_eq!(aux.binding, original_binding);
        assert!(!aux.row_stream_emitted_for_test_v1());
        assert!(
            aux.fill_aux_column_v1(segment, ZK_X509_SHA_BATCH_AUX_WIDTH_V1, &mut target,)
                .is_err()
        );
        assert!(target.iter().all(|value| *value == sentinel));
        assert_eq!(aux.binding, original_binding);
        assert!(!aux.row_stream_emitted_for_test_v1());
        assert!(aux.fill_aux_column_v1(segment, 0, &mut short).is_err());
        assert!(short.iter().all(|value| *value == sentinel));
        assert_eq!(aux.binding, original_binding);
        assert!(!aux.row_stream_emitted_for_test_v1());
        aux.zeroize_private_v1();
        assert!(aux.private_is_zeroized_v1());
        assert!(
            aux.replay_base_column_v1(&fixed, segment, 0, &mut target)
                .is_err()
        );
        assert!(aux.fill_aux_column_v1(segment, 0, &mut target).is_err());
        assert!(target.iter().all(|value| *value == sentinel));
        assert!(aux.private_is_zeroized_v1());
    }
    #[test]
    fn segment_column_replay_has_one_column_bound_and_fail_closed_zeroization() {
        assert_eq!(
            ZK_X509_SHA_NATIVE_REPLAY_COLUMN_BYTES_V1,
            ZK_X509_SHA_SEGMENT_ROWS_V1 as u64 * core::mem::size_of::<F>() as u64
        );
        assert_eq!(
            ZK_X509_SHA_EAGER_AUX_MATRIX_BYTES_V1,
            ZK_X509_SHA_SEGMENT_COUNT_V1 as u64
                * ZK_X509_SHA_BATCH_AUX_WIDTH_V1 as u64
                * ZK_X509_SHA_NATIVE_REPLAY_COLUMN_BYTES_V1
        );
        const {
            assert!(
                ZK_X509_SHA_NATIVE_COLUMN_REPLAY_PEAK_BYTES_V1
                    < ZK_X509_SHA_EAGER_AUX_MATRIX_BYTES_V1
            )
        };
        assert!(
            core::mem::size_of::<ZkX509ShaBatchSegmentBaseSourceV1<'_>>()
                < core::mem::size_of::<Vec<F>>() * 4
                    + core::mem::size_of::<Option<native_base_cache::ShaNativeBaseCacheV1>>()
        );
        assert!(
            // The enclosing source forecast now charges the optional native
            // cache's inline owner as well as its separately bounded heap.
            // Retain the original bound on all other replay-owner metadata.
            core::mem::size_of::<ZkX509ShaBatchSegmentAuxSourceV1<'_>>()
                < core::mem::size_of::<ZkX509CredentialPreAuxBindingV1>()
                    + core::mem::size_of::<Vec<F>>() * 4
                    + core::mem::size_of::<Option<native_aux_cache::ShaNativeAuxCacheV1>>()
                    + core::mem::size_of::<Option<native_base_cache::ShaNativeBaseCacheV1>>()
        );
        let mut partial = vec![F(0xA5); 8];
        assert!(matches!(
            {
                let mut fill = ZkX509ShaColumnFillGuardV1::new_v1(&mut partial);
                fill.write_v1(0, F::ONE);
                fill.write_v1(2, F::ONE);
                fill.finish_v1()
            },
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        ));
        assert_eq!(partial, vec![F::ZERO; 8]);
        let mut noncanonical = vec![F(0xA5); 2];
        assert!(matches!(
            {
                let mut fill = ZkX509ShaColumnFillGuardV1::new_v1(&mut noncanonical);
                fill.write_v1(0, F::ONE);
                fill.write_v1(1, F(GOLDILOCKS_MODULUS_V1));
                fill.finish_v1()
            },
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        ));
        assert_eq!(noncanonical, vec![F::ZERO; 2]);
        let mut complete = vec![F::ZERO; 2];
        {
            let mut fill = ZkX509ShaColumnFillGuardV1::new_v1(&mut complete);
            fill.write_v1(0, F(7));
            fill.write_v1(1, F(11));
            fill.finish_v1().expect("complete canonical column");
        }
        assert_eq!(complete, vec![F(7), F(11)]);
        complete.fill(F::ZERO);
        assert_eq!(complete, vec![F::ZERO; 2]);
    }
    #[test]
    fn base_rows_are_token_invariant_and_bound_aux_rows_are_token_sensitive() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 2,
        })
        .expect("schedule");
        let manifest = schedule.calls[ZK_X509_SHA_CA_NODE_CALL_START_V1];
        let witness = witness_for(manifest);
        let first = build_zk_x509_sha_batch_call_base_source_v1(manifest, &witness, 2)
            .expect("first base phase");
        let second = build_zk_x509_sha_batch_call_base_source_v1(manifest, &witness, 2)
            .expect("second base phase");
        let sampled_rows = [0, first.logical_rows() / 2, first.logical_rows() - 1];
        for row in sampled_rows {
            assert_eq!(
                first.base_row(row).expect("first base"),
                second.base_row(row).expect("second base")
            );
            assert_eq!(
                first.fixed_row(row).expect("first fixed"),
                second.fixed_row(row).expect("second fixed")
            );
        }
        let first = first
            .bind_v1(&credential_binding(0x31))
            .expect("first bound trace");
        let second = second
            .bind_v1(&credential_binding(0x32))
            .expect("second bound trace");
        assert!((0..first.logical_rows()).any(|row| {
            first.aux_row(row).expect("first aux") != second.aux_row(row).expect("second aux")
        }));
    }
    #[test]
    fn every_auxiliary_challenge_family_is_sensitive_and_terminals_are_consistent() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 2,
        })
        .expect("schedule");
        let manifest = schedule.calls[CRL_ISSUER_SPKI_CALL_V1];
        let witness = witness_for(manifest);
        let baseline = build_zk_x509_sha_batch_call_trace_v1(
            manifest,
            &witness,
            word_challenges(),
            challenges(),
            &rfc_challenges(),
            2,
        )
        .expect("baseline");
        let mut changed_word = word_challenges();
        changed_word.memory.lanes[0].beta = changed_word.memory.lanes[0].beta.add(F::ONE);
        let word_changed = build_zk_x509_sha_batch_call_trace_v1(
            manifest,
            &witness,
            changed_word,
            challenges(),
            &rfc_challenges(),
            2,
        )
        .expect("word-changed");
        let mut changed_call = challenges();
        changed_call.lanes[0].terms[0] = changed_call.lanes[0].terms[0].add(F::ONE);
        let call_changed = build_zk_x509_sha_batch_call_trace_v1(
            manifest,
            &witness,
            word_challenges(),
            changed_call,
            &rfc_challenges(),
            2,
        )
        .expect("call-changed");
        let mut changed_rfc = rfc_challenges();
        changed_rfc.tuple[0][0] = changed_rfc.tuple[0][0].add(F(10_000));
        let rfc_changed = build_zk_x509_sha_batch_call_trace_v1(
            manifest,
            &witness,
            word_challenges(),
            challenges(),
            &changed_rfc,
            2,
        )
        .expect("RFC-changed");
        for (name, changed) in [
            ("SHA word-memory", &word_changed),
            ("SHA call bus", &call_changed),
            ("RFC consumer", &rfc_changed),
        ] {
            assert!(
                (0..baseline.logical_rows()).any(|row| {
                    baseline.aux_row(row).expect("baseline aux")
                        != changed.aux_row(row).expect("changed aux")
                }),
                "{name} did not affect any auxiliary row"
            );
        }
        let binding = credential_binding(0x41);
        let bound = build_zk_x509_sha_batch_call_base_source_v1(manifest, &witness, 2)
            .expect("production base")
            .bind_v1(&binding)
            .expect("production bind");
        let expected = build_zk_x509_sha_batch_call_trace_v1(
            manifest,
            &witness,
            binding.sha_word(),
            binding.sha(),
            binding.rfc5280(),
            2,
        )
        .expect("focused replay");
        assert_eq!(bound.terminal, expected.terminal);
        assert_eq!(bound.rfc_terminal, expected.rfc_terminal);
        assert_eq!(bound.segment_product_state, expected.segment_product_state);
    }
    #[test]
    fn challenge_independent_sources_zeroize_recursively() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 0,
        })
        .expect("schedule");
        let manifest = schedule.calls[ZK_X509_SHA_CA_NODE_CALL_START_V1];
        let witness = witness_for(manifest);
        let mut source = build_zk_x509_sha_batch_call_base_source_v1(manifest, &witness, 0)
            .expect("base source");
        assert!(!source.private_is_zeroized_v1());
        source.zeroize_private_v1();
        assert!(source.private_is_zeroized_v1());
        source.zeroize_private_v1();
        assert!(source.private_is_zeroized_v1());
    }
    #[test]
    fn complete_witness_array_validation_rejects_reorder_substitution_and_digest_mutation() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 2,
        })
        .expect("schedule");
        let canonical = witnesses(&schedule);
        validate_zk_x509_sha_call_witnesses_v1(&schedule, &canonical)
            .expect("canonical witness array");
        let mut reordered = canonical.clone();
        reordered.swap(0, 1);
        assert_eq!(
            validate_zk_x509_sha_call_witnesses_v1(&schedule, &reordered),
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        );
        let mut substituted = canonical.clone();
        substituted[2] = substituted[1].clone();
        assert_eq!(
            validate_zk_x509_sha_call_witnesses_v1(&schedule, &substituted),
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        );
        let mut changed_digest = canonical;
        changed_digest[4].digest[0] ^= 1;
        assert_eq!(
            validate_zk_x509_sha_call_witnesses_v1(&schedule, &changed_digest),
            Err(ZkX509ShaCallBusStarkErrorV1::Digest)
        );
    }
    fn write_sha_byte(base: &mut [F; ZK_X509_SHA_BATCH_BASE_WIDTH_V1], byte: usize, value: u8) {
        let bits_start = 1 + (3 - byte) * 8;
        for bit in 0..8 {
            base[bits_start + bit] = F(u64::from((value >> bit) & 1));
        }
    }
    #[test]
    fn maximum_schedule_is_exact_and_partitioned_at_boundaries() {
        assert_eq!(ZK_X509_SHA_BATCH_BASE_WIDTH_V1, 89);
        assert_eq!(ZK_X509_SHA_BATCH_AUX_WIDTH_V1, 82);
        assert_eq!(ZK_X509_SHA_BATCH_FIXED_WIDTH_V1, 137);
        assert_eq!(ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1, 801);
        assert_eq!(ZK_X509_SHA_BATCH_CONSTRAINT_DEGREE_V1, 6);
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        })
        .expect("schedule");
        assert_eq!(schedule.calls.len(), 29);
        assert_eq!(
            schedule.calls[2].activation,
            ZkX509ShaCallActivationV1::OptionalPrivate
        );
        assert!(schedule.calls.iter().enumerate().all(|(index, call)| {
            index == 2 || call.activation == ZkX509ShaCallActivationV1::Required
        }));
        assert_eq!(
            schedule
                .calls
                .iter()
                .map(|call| call.maximum_blocks)
                .sum::<usize>(),
            616
        );
        assert_eq!(
            schedule
                .calls
                .iter()
                .map(|call| call.maximum_events())
                .sum::<usize>(),
            10_088
        );
        assert_eq!(
            schedule
                .calls
                .iter()
                .map(|call| call.maximum_local_rows)
                .sum::<usize>(),
            655_888
        );
        assert_eq!(
            schedule
                .calls
                .iter()
                .map(|call| call.maximum_memory_rows)
                .sum::<usize>(),
            1_316_240
        );
        assert!(schedule.calls.iter().all(|call| {
            call.maximum_local_rows
                == call.maximum_blocks * ZK_X509_SHA_LOCAL_ROWS_PER_BLOCK_V1
                    + ZK_X509_SHA_LOCAL_ROWS_PER_CALL_V1
                && call.maximum_memory_rows
                    == call.maximum_blocks * ZK_X509_SHA_MEMORY_ROWS_PER_BLOCK_V1
                        + ZK_X509_SHA_MEMORY_ROWS_PER_CALL_V1
        }));
        assert_eq!(
            schedule
                .calls
                .iter()
                .map(|call| call.maximum_logical_rows())
                .sum::<usize>(),
            1_972_128
        );
        assert_eq!(
            schedule
                .calls
                .iter()
                .map(|call| call.maximum_logical_rows())
                .max(),
            Some(ZK_X509_SHA_MAX_CALL_LOGICAL_ROWS_V1)
        );
        assert_eq!(
            ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1,
            [480_288, 521_952, 521_696, 448_192]
        );
        assert_eq!(
            ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1.iter().sum::<usize>(),
            ZK_X509_SHA_MAX_LOGICAL_ROWS_V1
        );
        assert!(schedule.calls.iter().all(|call| {
            let first_segment = call.first_logical_row / ZK_X509_SHA_SEGMENT_ROWS_V1;
            let last_segment = (call.first_logical_row + call.maximum_logical_rows() - 1)
                / ZK_X509_SHA_SEGMENT_ROWS_V1;
            first_segment == last_segment
        }));
        for (segment, expected_start) in [0, 524_288, 1_048_576, 1_572_864].into_iter().enumerate()
        {
            let replay = ZkX509ShaSegmentReplayV1::new(segment).expect("segment");
            assert_eq!(replay.global_row(0).expect("first"), expected_start);
            assert_eq!(
                replay.global_row(replay.active_rows() - 1).expect("last"),
                expected_start + replay.active_rows() - 1
            );
            assert_eq!(
                replay.global_row(replay.active_rows()),
                Err(ZkX509ShaCallBusStarkErrorV1::Topology)
            );
            let first = schedule
                .logical_row(expected_start)
                .expect("segment starts with a whole call");
            assert_eq!(first.1, 0);
            let last = schedule
                .logical_row(expected_start + replay.active_rows() - 1)
                .expect("segment ends with a whole call");
            assert_eq!(last.1 + 1, last.0.maximum_logical_rows());
            assert_eq!(
                schedule.logical_row(expected_start + replay.active_rows()),
                Err(ZkX509ShaCallBusStarkErrorV1::Topology)
            );
        }
        assert_eq!(
            schedule.logical_row(ZK_X509_SHA_SEGMENT_COUNT_V1 * ZK_X509_SHA_SEGMENT_ROWS_V1),
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        );
    }
    #[test]
    fn verifier_fixed_provider_replays_every_call_boundary_and_physical_padding() {
        let shape = ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        };
        let provider = ZkX509ShaBatchFixedProviderV1::new_v1(shape).expect("fixed provider");
        assert_eq!(provider.shape(), shape);
        for manifest in provider.schedule().calls().iter().copied() {
            for global_row in [
                manifest.first_logical_row,
                manifest.first_logical_row + manifest.maximum_logical_rows() - 1,
            ] {
                let segment = global_row / ZK_X509_SHA_SEGMENT_ROWS_V1;
                let segment_row = global_row % ZK_X509_SHA_SEGMENT_ROWS_V1;
                let fixed = provider
                    .fixed_row_v1(segment, segment_row)
                    .expect("boundary fixed row");
                assert_eq!(
                    fixed[ZK_X509_SHA_FIXED_CALL_V1],
                    F(u64::from(manifest.call))
                );
                assert_eq!(
                    fixed[ZK_X509_SHA_FIXED_ROLE_V1],
                    F(u64::from(manifest.role.role_code()))
                );
                assert_eq!(
                    fixed[ZK_X509_SHA_FIXED_SLOT_V1],
                    F(u64::from(manifest.role.slot()))
                );
                assert_eq!(
                    fixed[SHA_WORD_CAPACITY_CALL_FIRST_V1],
                    F(u64::from(global_row == manifest.first_logical_row))
                );
                assert_eq!(
                    fixed[SHA_WORD_CAPACITY_CALL_LAST_V1],
                    F(u64::from(
                        global_row + 1
                            == manifest.first_logical_row + manifest.maximum_logical_rows()
                    ))
                );
                assert_eq!(
                    fixed[ZK_X509_SHA_FIXED_SEGMENT_FIRST_V1],
                    F(u64::from(segment_row == 0))
                );
                assert_eq!(
                    fixed[ZK_X509_SHA_FIXED_SEGMENT_LAST_V1],
                    F(u64::from(
                        segment_row + 1 == ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[segment]
                    ))
                );
                assert_eq!(fixed[ZK_X509_SHA_FIXED_PHYSICAL_PADDING_V1], F::ZERO);
            }
        }
        let manifest = provider.schedule().call(0).expect("first call");
        let call = build_zk_x509_sha_batch_call_trace_v1(
            manifest,
            &witness_for(manifest),
            word_challenges(),
            challenges(),
            &rfc_challenges(),
            shape.disclosed_attributes,
        )
        .expect("first call trace");
        for local_row in [
            0,
            SHA_WORD_CAPACITY_LOCAL_ROWS_PER_BLOCK_V1,
            call.logical_rows() - 1,
        ] {
            let global_row = manifest.first_logical_row + local_row;
            let segment = global_row / ZK_X509_SHA_SEGMENT_ROWS_V1;
            let segment_row = global_row % ZK_X509_SHA_SEGMENT_ROWS_V1;
            let mut expected = call.fixed_row(local_row).expect("trace fixed row");
            expected[ZK_X509_SHA_FIXED_SEGMENT_FIRST_V1] = F(u64::from(segment_row == 0));
            expected[ZK_X509_SHA_FIXED_SEGMENT_LAST_V1] = F(u64::from(
                segment_row + 1 == ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[segment],
            ));
            assert_eq!(
                provider
                    .fixed_row_v1(segment, segment_row)
                    .expect("provider fixed row"),
                expected
            );
        }
        let padding_segment = 0;
        let padding_row = ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[padding_segment];
        assert_eq!(
            provider
                .fixed_row_v1(padding_segment, padding_row)
                .expect("first physical padding"),
            physical_padding_row_v1(padding_row).fixed
        );
        assert_eq!(
            provider.fixed_row_v1(ZK_X509_SHA_SEGMENT_COUNT_V1, 0),
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        );
        assert_eq!(
            provider.fixed_row_v1(0, ZK_X509_SHA_SEGMENT_ROWS_V1),
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        );
        assert!(matches!(
            ZkX509ShaBatchFixedProviderV1::new_v1(ZkX509ShaCallPublicShapeV1 {
                disclosed_attributes: 5,
            }),
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        ));
    }
    #[test]
    fn all_scheduled_calls_construct_and_exhaust_rfc_streams_for_every_public_shape() {
        for disclosed_attributes in 0..=4 {
            let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
                disclosed_attributes,
            })
            .expect("schedule");
            let mut observed_logical_rows = 0_usize;
            for manifest in schedule.calls {
                let witness = witness_for(manifest);
                let call = build_zk_x509_sha_batch_call_trace_v1(
                    manifest,
                    &witness,
                    word_challenges(),
                    challenges(),
                    &rfc_challenges(),
                    disclosed_attributes,
                )
                .expect("every scheduled call constructs");
                assert_eq!(
                    call.logical_rows(),
                    manifest.maximum_blocks
                        * (ZK_X509_SHA_LOCAL_ROWS_PER_BLOCK_V1
                            + ZK_X509_SHA_MEMORY_ROWS_PER_BLOCK_V1)
                        + ZK_X509_SHA_LOCAL_ROWS_PER_CALL_V1
                        + ZK_X509_SHA_MEMORY_ROWS_PER_CALL_V1,
                    "disclosures {disclosed_attributes}, call {}",
                    manifest.call
                );
                observed_logical_rows += call.logical_rows();
                let mut stream_events = [0_usize; ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1];
                for index in 0..call.logical_rows() {
                    let row = call.row(index).expect("canonical call row");
                    for (stream, count) in stream_events.iter_mut().enumerate() {
                        if sha_rfc_consumer_row_factor_v1(
                            &row.base,
                            &row.fixed,
                            stream,
                            0,
                            &rfc_challenges(),
                        )
                        .expect("canonical RFC factor")
                        .is_some()
                        {
                            *count += 1;
                        }
                    }
                }
                let expected_stream_events = call.rfc_consumer.map_or([0_usize; 4], |consumer| {
                    let raw_end = consumer
                        .message_prefix_bytes
                        .checked_add(consumer.message_capacity_bytes)
                        .expect("bounded RFC consumer interval");
                    core::array::from_fn(|stream| {
                        (consumer.message_prefix_bytes..raw_end)
                            .filter(|sha_offset| {
                                sha_offset % ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1 == stream
                            })
                            .count()
                            + usize::from(consumer.length_channel.is_some() && stream < 2) * 4
                    })
                });
                assert_eq!(
                    stream_events, expected_stream_events,
                    "disclosures {disclosed_attributes}, call {}",
                    manifest.call
                );
                let last = call.logical_rows() - 1;
                let last_row = call.row(last).expect("last call row");
                assert!(
                    evaluate_zk_x509_sha_batch_residues_v1(
                        &last_row,
                        &last_row,
                        word_challenges(),
                        challenges(),
                        &rfc_challenges(),
                        standalone_segment_terminal(&call),
                        &ca_call_boundaries_for_calls(&[&call]),
                    )
                    .expect("last-row terminal constraints")
                    .iter()
                    .all(|residue| *residue == F::ZERO),
                    "disclosures {disclosed_attributes}, call {} terminal",
                    manifest.call
                );
            }
            assert_eq!(
                observed_logical_rows, ZK_X509_SHA_MAX_LOGICAL_ROWS_V1,
                "disclosures {disclosed_attributes}"
            );
        }
    }
    #[test]
    fn maximum_message_boundaries_have_exact_sha_padding() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        })
        .expect("schedule");
        let cases = [
            (0, 4_096, 65),
            (3, 4_096, 65),
            (4, 4_161, 66),
            (5, 2_048, 33),
            (12, 164, 3),
            (13, 289, 5),
            (14, 313, 6),
            (15, 352, 6),
            (16, 156, 3),
            (17, 147, 3),
            (28, 147, 3),
        ];
        for (call, bytes, blocks) in cases {
            let manifest = schedule.call(call).expect("call");
            assert_eq!(manifest.maximum_message_bytes, bytes);
            assert_eq!(manifest.maximum_blocks, blocks);
            let message = vec![0xa5; bytes];
            let words = padded_words_v1(&message, blocks).expect("padding");
            assert_eq!(words.len(), blocks * 16);
            assert_eq!(
                words[(bytes / 4).min(words.len() - 1)].to_be_bytes()[bytes % 4],
                0x80
            );
            assert_eq!(
                words.last().copied(),
                Some(u32::try_from(bytes * 8).expect("test bit length"))
            );
        }
    }
    #[test]
    fn fixed_width_governance_frames_reject_short_or_long_messages() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        })
        .expect("schedule");
        for call in [
            CRL_ISSUER_SPKI_CALL_V1,
            TRUST_ANCHOR_RECORD_CALL_V1,
            CRL_RECORD_CALL_V1,
            ZK_X509_SHA_CA_LEAF_CALL_V1,
            ZK_X509_SHA_CA_NODE_CALL_START_V1,
            ZK_X509_SHA_CA_NODE_CALL_START_V1 + ZK_X509_CA_COMPACT_TREE_DEPTH_V1 - 1,
        ] {
            let manifest = schedule.calls[call];
            let mut witness = witness_for(manifest);
            witness.message.pop();
            witness.digest = Sha256::digest(&witness.message).into();
            assert_eq!(
                validate_witness_v1(manifest, &witness),
                Err(ZkX509ShaCallBusStarkErrorV1::LengthOrPadding)
            );
            witness
                .message
                .resize(manifest.maximum_message_bytes + 1, 0);
            witness.digest = Sha256::digest(&witness.message).into();
            assert_eq!(
                validate_witness_v1(manifest, &witness),
                Err(ZkX509ShaCallBusStarkErrorV1::LengthOrPadding)
            );
        }
        let policy = schedule.calls[CERTIFICATE_POLICY_RECORD_CALL_V1];
        let mut shorter_policy = witness_for(policy);
        shorter_policy.message.pop();
        shorter_policy.digest = Sha256::digest(&shorter_policy.message).into();
        validate_witness_v1(policy, &shorter_policy).expect("variable-width policy frame");
    }
    #[test]
    fn private_optional_chain_and_publicly_inactive_disclosures_are_canonical() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 1,
        })
        .expect("schedule");
        assert_eq!(
            schedule.calls[2].activation,
            ZkX509ShaCallActivationV1::OptionalPrivate
        );
        assert_eq!(
            schedule.calls[7].activation,
            ZkX509ShaCallActivationV1::Required
        );
        for call in [8, 9, 10] {
            assert_eq!(
                schedule.calls[call].activation,
                ZkX509ShaCallActivationV1::Inactive
            );
        }
        let mut witnesses = witnesses(&schedule);
        witnesses[2].message.clear();
        witnesses[2].digest = canonical_dummy_digest_v1();
        let dummy = &witnesses[2];
        assert!(dummy.message.is_empty());
        assert_eq!(dummy.digest, canonical_dummy_digest_v1());
        validate_witness_v1(schedule.calls[2], dummy).expect("canonical dummy");
        witnesses[2].message.push(0);
        witnesses[2].digest = Sha256::digest(&witnesses[2].message).into();
        validate_witness_v1(schedule.calls[2], &witnesses[2])
            .expect("private third certificate present");
        witnesses[2].message.clear();
        witnesses[2].digest = canonical_dummy_digest_v1();
        witnesses[2].digest[0] ^= 1;
        assert_eq!(
            validate_witness_v1(schedule.calls[2], &witnesses[2]),
            Err(ZkX509ShaCallBusStarkErrorV1::InactiveCall)
        );
        witnesses[8].message.push(0);
        witnesses[8].digest = Sha256::digest(&witnesses[8].message).into();
        assert_eq!(
            validate_witness_v1(schedule.calls[8], &witnesses[8]),
            Err(ZkX509ShaCallBusStarkErrorV1::InactiveCall)
        );
    }
    #[test]
    fn event_replay_binds_role_slot_length_padding_and_digest() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        })
        .expect("schedule");
        let witnesses = witnesses(&schedule);
        let first = replay_sha_call_event_v1(&schedule, &witnesses, 0).expect("first");
        assert_eq!(first.call, 0);
        assert_eq!(first.role, ZkX509ShaCallRoleV1::CertificateTbs(0));
        assert_eq!(first.word_kind, ZkX509ShaCallWordKindV1::Input);
        assert!(first.active);
        let call = schedule.calls[0];
        let first_digest = call.first_event + call.maximum_input_words();
        let digest = replay_sha_call_event_v1(&schedule, &witnesses, first_digest).expect("digest");
        assert_eq!(digest.word_kind, ZkX509ShaCallWordKindV1::Digest);
        assert_eq!(digest.word, 0);
        assert!(digest.active);
        let mut changed = witnesses.clone();
        changed[0].digest[0] ^= 1;
        assert_eq!(
            replay_sha_call_event_v1(&schedule, &changed, 0),
            Err(ZkX509ShaCallBusStarkErrorV1::Digest)
        );
        changed = witnesses.clone();
        changed[0].message.push(0);
        assert_eq!(
            replay_sha_call_event_v1(&schedule, &changed, 0),
            Err(ZkX509ShaCallBusStarkErrorV1::LengthOrPadding)
        );
        changed = witnesses.clone();
        changed[0].role = ZkX509ShaCallRoleV1::CrlTbs;
        assert_eq!(
            replay_sha_call_event_v1(&schedule, &changed, 0),
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        );
    }
    #[test]
    fn streamed_call_trace_debug_is_redacted_and_zeroization_is_recursive() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        })
        .expect("schedule");
        let manifest = schedule.calls[ZK_X509_SHA_CA_NODE_CALL_START_V1];
        let witness = witness_for(manifest);
        let mut trace = build_zk_x509_sha_batch_call_trace_v1(
            manifest,
            &witness,
            word_challenges(),
            challenges(),
            &rfc_challenges(),
            4,
        )
        .expect("streamed trace");
        let debug = format!("{trace:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains("base_rows"));
        assert!(!trace.private_is_zeroized_v1());
        let retained_rows = trace.product_rows.len();
        assert!(retained_rows > 0);
        trace.zeroize_private_cells_v1();
        assert_eq!(trace.product_rows.len(), retained_rows);
        assert!(
            trace
                .product_rows
                .iter()
                .flatten()
                .all(|value| *value == F::ZERO)
        );
        assert!(
            trace
                .terminal
                .source_products
                .iter()
                .chain(&trace.terminal.digest_products)
                .chain(trace.rfc_terminal.stream_products.iter().flatten())
                .chain(&trace.segment_product_state.source_products)
                .chain(&trace.segment_product_state.digest_products)
                .chain(
                    trace
                        .segment_product_state
                        .rfc_stream_products
                        .iter()
                        .flatten()
                )
                .all(|value| *value == F::ZERO)
        );
        trace.zeroize_private_v1();
        assert!(trace.private_is_zeroized_v1());
        trace.zeroize_private_v1();
        assert!(trace.private_is_zeroized_v1());
    }
    #[test]
    fn four_lane_products_reject_value_address_and_activation_tampering() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        })
        .expect("schedule");
        let witnesses = witnesses(&schedule);
        let challenges = challenges();
        let expected =
            sha_call_product_terminal_v1(&schedule, &witnesses, challenges).expect("terminal");
        assert_eq!(
            evaluate_sha_call_terminal_constraints_v1(expected, expected),
            [F::ZERO; 4]
        );
        let event = replay_sha_call_event_v1(&schedule, &witnesses, 0).expect("event");
        let mut changed = event;
        changed.value ^= 1;
        let left =
            advance_sha_call_products_v1([F::ONE; 4], event, challenges).expect("left terminal");
        let right =
            advance_sha_call_products_v1([F::ONE; 4], changed, challenges).expect("right terminal");
        assert_ne!(
            evaluate_sha_call_terminal_constraints_v1(left, right),
            [F::ZERO; 4]
        );
        changed = event;
        changed.call = 1;
        let right =
            advance_sha_call_products_v1([F::ONE; 4], changed, challenges).expect("right terminal");
        assert_ne!(left, right);
        changed = event;
        changed.active = false;
        assert_eq!(
            advance_sha_call_products_v1([F::ONE; 4], changed, challenges),
            Err(ZkX509ShaCallBusStarkErrorV1::Event)
        );
    }
    #[test]
    fn rfc_consumer_channels_and_frame_offsets_are_exact_and_fail_closed() {
        for disclosed_attributes in 0..=4 {
            let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
                disclosed_attributes,
            })
            .expect("schedule");
            let prefix = 5 + 2 * disclosed_attributes;
            for call in 0..3 {
                let consumer = sha_rfc_consumer_channels_v1(
                    schedule.calls[call].call,
                    schedule.calls[call].role,
                    disclosed_attributes,
                )
                .expect("certificate consumer")
                .expect("certificate mapping");
                assert_eq!(consumer.role, ZkX509Rfc5280OutputRoleV1::CertificateTbsSha);
                assert_eq!(
                    consumer.message_channel,
                    u32::try_from(prefix + 2 * call).expect("protocol-bounded index fits u32")
                );
                assert_eq!(
                    consumer.length_channel,
                    Some(
                        u32::try_from(prefix + 2 * call + 1)
                            .expect("protocol-bounded index fits u32")
                    )
                );
                assert_eq!(consumer.message_prefix_bytes, 0);
                assert_eq!(consumer.message_capacity_bytes, 4_096);
            }
            for (call, role, message_offset, length_offset, frame_prefix, capacity) in [
                (
                    3,
                    ZkX509Rfc5280OutputRoleV1::CrlTbsP256Message,
                    16,
                    Some(17),
                    0,
                    4_096,
                ),
                (
                    4,
                    ZkX509Rfc5280OutputRoleV1::CrlCommitment,
                    18,
                    Some(19),
                    65,
                    4_096,
                ),
                (
                    12,
                    ZkX509Rfc5280OutputRoleV1::IssuerSpkiSha,
                    canonical_issuer_spki_channel_v1(disclosed_attributes) as usize - prefix,
                    None,
                    73,
                    91,
                ),
            ] {
                let consumer = sha_rfc_consumer_channels_v1(
                    schedule.calls[call].call,
                    schedule.calls[call].role,
                    disclosed_attributes,
                )
                .expect("consumer")
                .expect("mapped consumer");
                assert_eq!(consumer.role, role);
                assert_eq!(
                    consumer.message_channel,
                    u32::try_from(prefix + message_offset)
                        .expect("protocol-bounded index fits u32")
                );
                assert_eq!(
                    consumer.length_channel,
                    length_offset.map(|offset| u32::try_from(prefix + offset)
                        .expect("protocol-bounded index fits u32"))
                );
                assert_eq!(consumer.message_prefix_bytes, frame_prefix);
                assert_eq!(consumer.message_capacity_bytes, capacity);
            }
            assert_eq!(
                sha_rfc_consumer_channels_v1(
                    schedule.calls[13].call,
                    schedule.calls[13].role,
                    disclosed_attributes,
                )
                .expect("non-RFC call"),
                None
            );
        }
        assert_eq!(
            sha_rfc_consumer_channels_v1(0, ZkX509ShaCallRoleV1::CrlTbs, 4),
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        );
        assert_eq!(
            sha_rfc_consumer_channels_v1(0, ZkX509ShaCallRoleV1::CertificateTbs(0), 5),
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        );
    }
    #[test]
    fn rfc_message_byte_factor_binds_mask_offset_channel_role_and_lane() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        })
        .expect("schedule");
        let consumer = sha_rfc_consumer_channels_v1(
            schedule.calls[CRL_COMMITMENT_CALL_V1].call,
            schedule.calls[CRL_COMMITMENT_CALL_V1].role,
            4,
        )
        .expect("consumer")
        .expect("CRL consumer");
        let mut base = [F::ZERO; ZK_X509_SHA_BATCH_BASE_WIDTH_V1];
        let mut fixed = [F::ZERO; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1];
        fixed[SHA_WORD_CAPACITY_INPUT_WORD_V1] = F::ONE;
        fixed[SHA_WORD_CAPACITY_INPUT_WORD_INDEX_V1] = F(16);
        write_sha_rfc_fixed_event_v1(
            &mut fixed,
            consumer,
            1,
            true,
            false,
            false,
            consumer.message_channel,
            0,
        )
        .expect("fixed message event");
        write_sha_byte(&mut base, 1, 0xa5);
        base[SHA_WORD_CAPACITY_MESSAGE_MASK_V1 + 1] = F::ONE;
        let challenges = rfc_challenges();
        let factor = sha_rfc_consumer_row_factor_v1(&base, &fixed, 1, 0, &challenges)
            .expect("factor")
            .expect("raw byte event");
        let expected = zk_x509_rfc5280_opened_output_factor_fields_v1(
            F(ZkX509Rfc5280OutputRoleV1::CrlCommitment as u64),
            F(31),
            F(2),
            F::ZERO,
            F::ZERO,
            F(0xa5),
            F::ZERO,
            0,
            &challenges,
        )
        .expect("expected factor");
        assert_eq!(factor, expected);
        let mut changed = base;
        changed[1 + (3 - 1) * 8] = F::ONE.sub(changed[1 + (3 - 1) * 8]);
        assert_ne!(
            sha_rfc_consumer_row_factor_v1(&changed, &fixed, 1, 0, &challenges)
                .expect("changed bit")
                .expect("event"),
            factor
        );
        changed = base;
        changed[SHA_WORD_CAPACITY_MESSAGE_MASK_V1 + 1] = F::ZERO;
        assert_ne!(
            sha_rfc_consumer_row_factor_v1(&changed, &fixed, 1, 0, &challenges)
                .expect("changed mask")
                .expect("event"),
            factor
        );
        let mut changed_fixed = fixed;
        let descriptor = ZK_X509_SHA_FIXED_RFC_STREAMS_V1 + ZK_X509_SHA_FIXED_RFC_STREAM_STRIDE_V1;
        changed_fixed[descriptor + ZK_X509_SHA_FIXED_RFC_MESSAGE_EVENT_V1] = F::ZERO;
        assert_eq!(
            sha_rfc_consumer_row_factor_v1(&base, &changed_fixed, 1, 0, &challenges)
                .expect("disabled event"),
            None
        );
        changed_fixed = fixed;
        changed_fixed[descriptor + ZK_X509_SHA_FIXED_RFC_CHANNEL_V1] =
            changed_fixed[descriptor + ZK_X509_SHA_FIXED_RFC_CHANNEL_V1].add(F::ONE);
        assert_ne!(
            sha_rfc_consumer_row_factor_v1(&base, &changed_fixed, 1, 0, &challenges)
                .expect("changed channel")
                .expect("event"),
            factor
        );
        changed_fixed = fixed;
        changed_fixed[descriptor + ZK_X509_SHA_FIXED_RFC_ROLE_V1] =
            F(ZkX509Rfc5280OutputRoleV1::CertificateTbsSha as u64);
        assert_ne!(
            sha_rfc_consumer_row_factor_v1(&base, &changed_fixed, 1, 0, &challenges)
                .expect("changed role")
                .expect("event"),
            factor
        );
        changed_fixed = fixed;
        changed_fixed[descriptor + ZK_X509_SHA_FIXED_RFC_OFFSET_V1] = F::ONE;
        assert_ne!(
            sha_rfc_consumer_row_factor_v1(&base, &changed_fixed, 1, 0, &challenges)
                .expect("changed offset")
                .expect("event"),
            factor
        );
        assert_eq!(
            sha_rfc_consumer_row_factor_v1(
                &base,
                &fixed,
                ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1,
                0,
                &challenges,
            ),
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        );
        assert_eq!(
            sha_rfc_consumer_row_factor_v1(&base, &fixed, 1, ZK_X509_SHA_BUS_LANES_V1, &challenges,),
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        );
    }
    #[test]
    fn rfc_u64_length_factors_and_recomposition_reject_adversarial_values() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        })
        .expect("schedule");
        let consumer = sha_rfc_consumer_channels_v1(
            schedule.calls[CRL_COMMITMENT_CALL_V1].call,
            schedule.calls[CRL_COMMITMENT_CALL_V1].role,
            4,
        )
        .expect("consumer")
        .expect("CRL consumer");
        let mut base = [F::ZERO; ZK_X509_SHA_BATCH_BASE_WIDTH_V1];
        base[ZK_X509_SHA_LENGTH_BITS_START_V1 + 12] = F::ONE;
        let mut aux = [F::ZERO; ZK_X509_SHA_BATCH_AUX_WIDTH_V1];
        aux[SHA_WORD_CAPACITY_MESSAGE_COUNT_V1] = F(4_161);
        let mut fixed = [F::ZERO; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1];
        fixed[ZK_X509_SHA_FIXED_RFC_LENGTH_PAIR_V1] = F::ONE;
        fixed[ZK_X509_SHA_FIXED_RFC_LENGTH_PAIR_INDEX_V1] = F(3);
        fixed[ZK_X509_SHA_FIXED_RFC_LENGTH_PREFIX_V1] =
            F(u64::try_from(consumer.message_prefix_bytes).expect("bounded prefix"));
        write_sha_rfc_fixed_event_v1(
            &mut fixed,
            consumer,
            0,
            false,
            true,
            false,
            consumer.length_channel.expect("length channel"),
            6,
        )
        .expect("high length event");
        write_sha_rfc_fixed_event_v1(
            &mut fixed,
            consumer,
            1,
            false,
            false,
            true,
            consumer.length_channel.expect("length channel"),
            7,
        )
        .expect("low length event");
        let challenges = rfc_challenges();
        for (stream, offset, value) in [(0, 6, 16), (1, 7, 0)] {
            let factor = sha_rfc_consumer_row_factor_v1(&base, &fixed, stream, 0, &challenges)
                .expect("length factor")
                .expect("length event");
            let expected = zk_x509_rfc5280_opened_output_factor_fields_v1(
                F(ZkX509Rfc5280OutputRoleV1::CrlCommitment as u64),
                F(32),
                F(2),
                F::ZERO,
                F(offset),
                F(value),
                F::ZERO,
                0,
                &challenges,
            )
            .expect("expected length factor");
            assert_eq!(factor, expected);
        }
        let mut zero_pair_fixed = [F::ZERO; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1];
        zero_pair_fixed[ZK_X509_SHA_FIXED_RFC_LENGTH_PAIR_V1] = F::ONE;
        zero_pair_fixed[ZK_X509_SHA_FIXED_RFC_LENGTH_PAIR_INDEX_V1] = F::ZERO;
        for (stream, offset) in [(0, 0), (1, 1)] {
            write_sha_rfc_fixed_event_v1(
                &mut zero_pair_fixed,
                consumer,
                stream,
                false,
                false,
                false,
                consumer.length_channel.expect("length channel"),
                offset,
            )
            .expect("zero length-byte event");
            let factor =
                sha_rfc_consumer_row_factor_v1(&base, &zero_pair_fixed, stream, 0, &challenges)
                    .expect("zero length factor")
                    .expect("zero length event");
            let expected = zk_x509_rfc5280_opened_output_factor_fields_v1(
                F(ZkX509Rfc5280OutputRoleV1::CrlCommitment as u64),
                F(32),
                F(2),
                F::ZERO,
                F(u64::try_from(offset).expect("small length offset")),
                F::ZERO,
                F::ZERO,
                0,
                &challenges,
            )
            .expect("expected zero length factor");
            assert_eq!(factor, expected);
        }
        assert_eq!(
            sha_rfc_consumer_row_factor_v1(&base, &fixed, 2, 0, &challenges)
                .expect("unused length stream"),
            None
        );
        assert_eq!(
            sha_rfc_length_recomposition_residue_v1(&base, &aux, &fixed),
            F::ZERO
        );
        let mut changed_base = base;
        changed_base[ZK_X509_SHA_LENGTH_BITS_START_V1] = F::ONE;
        assert_ne!(
            sha_rfc_length_recomposition_residue_v1(&changed_base, &aux, &fixed),
            F::ZERO
        );
        let mut changed_aux = aux;
        changed_aux[SHA_WORD_CAPACITY_MESSAGE_COUNT_V1] =
            changed_aux[SHA_WORD_CAPACITY_MESSAGE_COUNT_V1].add(F::ONE);
        assert_ne!(
            sha_rfc_length_recomposition_residue_v1(&base, &changed_aux, &fixed),
            F::ZERO
        );
        let mut changed_fixed = fixed;
        changed_fixed[ZK_X509_SHA_FIXED_RFC_LENGTH_PREFIX_V1] =
            changed_fixed[ZK_X509_SHA_FIXED_RFC_LENGTH_PREFIX_V1].add(F::ONE);
        assert_ne!(
            sha_rfc_length_recomposition_residue_v1(&base, &aux, &changed_fixed),
            F::ZERO
        );
        let descriptor = ZK_X509_SHA_FIXED_RFC_STREAMS_V1;
        changed_fixed = fixed;
        changed_fixed[descriptor + ZK_X509_SHA_FIXED_RFC_LENGTH_HIGH_VALUE_V1] = F::ZERO;
        let zero_factor = sha_rfc_consumer_row_factor_v1(&base, &changed_fixed, 0, 0, &challenges)
            .expect("fixed zero event")
            .expect("length-pair identity retains the zero event");
        assert_ne!(
            zero_factor,
            sha_rfc_consumer_row_factor_v1(&base, &fixed, 0, 0, &challenges)
                .expect("high length factor")
                .expect("high length event")
        );
        changed_fixed[ZK_X509_SHA_FIXED_RFC_LENGTH_PAIR_V1] = F::ZERO;
        assert_eq!(
            sha_rfc_consumer_row_factor_v1(&base, &changed_fixed, 0, 0, &challenges)
                .expect("removed fixed event"),
            None
        );
    }
    #[test]
    fn framed_issuer_spki_rows_produce_only_proof_bound_rfc_terminals() {
        let issuer_channel = canonical_issuer_spki_channel_v1(4);
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        })
        .expect("schedule");
        let manifest = schedule.calls[CRL_ISSUER_SPKI_CALL_V1];
        let witness = witness_for(manifest);
        let call = build_zk_x509_sha_batch_call_trace_v1(
            manifest,
            &witness,
            word_challenges(),
            challenges(),
            &rfc_challenges(),
            4,
        )
        .expect("issuer-SPKI call");
        let endpoint = ZkX509IoEndpointV1 {
            role: ZkX509IoSegmentRoleV1::Sha256,
            instance: 0,
        };
        let mut expected = [F::ONE; ZK_X509_SHA_BUS_LANES_V1];
        for (offset, value) in [0x31_u8; ZK_X509_CA_SPKI_DER_BYTES_V1]
            .into_iter()
            .enumerate()
        {
            for (lane, product) in expected.iter_mut().enumerate() {
                *product = product.mul(
                    zk_x509_rfc5280_opened_output_factor_v1(
                        ZkX509Rfc5280OutputRoleV1::IssuerSpkiSha,
                        F(u64::from(issuer_channel)),
                        endpoint,
                        F(offset as u64),
                        F(u64::from(value)),
                        F::ZERO,
                        lane,
                        &rfc_challenges(),
                    )
                    .expect("expected consumer factor"),
                );
            }
        }
        assert_eq!(call.rfc_terminal.combined_products(), expected);
        for index in 0..call.logical_rows() {
            let next_index = (index + 1).min(call.logical_rows() - 1);
            let current = call.row(index).expect("current row");
            let next = call.row(next_index).expect("next row");
            let residues = evaluate_zk_x509_sha_batch_residues_v1(
                &current,
                &next,
                word_challenges(),
                challenges(),
                &rfc_challenges(),
                standalone_segment_terminal(&call),
                &ca_call_boundaries_for_calls(&[&call]),
            )
            .expect("valid opened row");
            assert!(
                residues.iter().all(|residue| *residue == F::ZERO),
                "issuer-SPKI residue at row {index}"
            );
        }
        let last = call.logical_rows() - 1;
        let prior = call.row(last - 1).expect("penultimate row");
        let last_row = call.row(last).expect("last row");
        for stream in 0..ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1 {
            for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
                let mut changed = last_row;
                let column = ZK_X509_SHA_RFC_CONSUMER_PRODUCTS_V1 + stream * 4 + lane;
                changed.aux[column] = changed.aux[column].add(F::ONE);
                assert!(
                    evaluate_zk_x509_sha_batch_residues_v1(
                        &prior,
                        &changed,
                        word_challenges(),
                        challenges(),
                        &rfc_challenges(),
                        standalone_segment_terminal(&call),
                        &ca_call_boundaries_for_calls(&[&call]),
                    )
                    .expect("mutated private endpoint recurrence")
                    .iter()
                    .any(|residue| *residue != F::ZERO),
                    "stream {stream}, lane {lane}"
                );
            }
        }
        let raw_word = call
            .word
            .fixed_rows
            .iter()
            .position(|fixed| {
                fixed[SHA_WORD_CAPACITY_INPUT_WORD_V1] == F::ONE
                    && fixed[SHA_WORD_CAPACITY_INPUT_WORD_INDEX_V1] == F(18)
            })
            .expect("first raw issuer-SPKI word");
        let mut current = call.row(raw_word).expect("raw row");
        let next = call.row(raw_word + 1).expect("raw next row");
        current.base[SHA_WORD_CAPACITY_MESSAGE_MASK_V1 + 1] = F::ZERO;
        assert!(
            evaluate_zk_x509_sha_batch_residues_v1(
                &current,
                &next,
                word_challenges(),
                challenges(),
                &rfc_challenges(),
                standalone_segment_terminal(&call),
                &ca_call_boundaries_for_calls(&[&call]),
            )
            .expect("mutated raw row residues")
            .iter()
            .any(|residue| *residue != F::ZERO)
        );
    }
    #[test]
    fn fixed_capacity_call_rows_and_separate_terminals_validate() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        })
        .expect("schedule");
        let mut witnesses = witnesses(&schedule);
        let manifest = schedule.calls[CERTIFICATE_POLICY_RECORD_CALL_V1];
        let prefix_end = (0..manifest.maximum_message_bytes)
            .filter(|offset| sha_frame_byte_v1(manifest, *offset).is_some())
            .max()
            .expect("policy static prefix")
            + 1;
        witnesses[CERTIFICATE_POLICY_RECORD_CALL_V1]
            .message
            .truncate(prefix_end + 1);
        witnesses[CERTIFICATE_POLICY_RECORD_CALL_V1].digest =
            Sha256::digest(&witnesses[CERTIFICATE_POLICY_RECORD_CALL_V1].message).into();
        let call = build_zk_x509_sha_batch_call_trace_v1(
            manifest,
            &witnesses[CERTIFICATE_POLICY_RECORD_CALL_V1],
            word_challenges(),
            challenges(),
            &rfc_challenges(),
            4,
        )
        .expect("capacity call");
        let terminals =
            sha_call_terminals_v1(&schedule, &witnesses, challenges()).expect("terminals");
        assert_eq!(call.terminal, terminals[CERTIFICATE_POLICY_RECORD_CALL_V1]);
        assert_eq!(call.logical_rows(), manifest.maximum_logical_rows());
        for index in 0..call.logical_rows() {
            let next = if index + 1 < call.logical_rows() {
                index + 1
            } else {
                index
            };
            let current = call.row(index).expect("current row");
            let next = call.row(next).expect("next row");
            let residues = evaluate_zk_x509_sha_batch_residues_v1(
                &current,
                &next,
                word_challenges(),
                challenges(),
                &rfc_challenges(),
                standalone_segment_terminal(&call),
                &ca_call_boundaries_for_calls(&[&call]),
            )
            .expect("batch residues");
            assert_eq!(residues.len(), ZK_X509_SHA_BATCH_CONSTRAINT_COUNT_V1);
            assert!(
                residues.iter().all(|residue| *residue == F::ZERO),
                "nonzero batch residue at row {index}"
            );
        }
        let inactive_row = call.word.base_rows[..call.word.maximum_local_rows]
            .iter()
            .position(|row| row[SHA_WORD_CAPACITY_ROW_ACTIVE_V1] == F::ZERO)
            .expect("first inactive local row");
        let mut current = call.row(inactive_row).expect("inactive row");
        let next = call.row(inactive_row + 1).expect("next row");
        current.base[0] = F::ONE;
        assert!(
            evaluate_zk_x509_sha_batch_residues_v1(
                &current,
                &next,
                word_challenges(),
                challenges(),
                &rfc_challenges(),
                standalone_segment_terminal(&call),
                &ca_call_boundaries_for_calls(&[&call]),
            )
            .expect("mutated residues")
            .iter()
            .any(|residue| *residue != F::ZERO)
        );
        let last = call.logical_rows() - 1;
        let previous = call.row(last - 1).expect("penultimate memory row");
        let terminal = call.row(last).expect("last memory row");
        assert_eq!(terminal.fixed[SHA_WORD_CAPACITY_CALL_LAST_V1], F::ONE);
        for row in [&previous, &terminal] {
            assert_eq!(
                row.fixed[SHA_WORD_CAPACITY_INPUT_WORD_V1]
                    .mul(row.base[SHA_WORD_CAPACITY_ROW_ACTIVE_V1]),
                F::ZERO
            );
            assert_eq!(row.fixed[SHA_WORD_CAPACITY_DIGEST_SELECTOR_V1], F::ZERO);
        }
        assert_eq!(
            F::ONE
                .sub(previous.fixed[ZK_X509_SHA_FIXED_SEGMENT_LAST_V1])
                .sub(previous.fixed[ZK_X509_SHA_FIXED_PHYSICAL_PADDING_V1]),
            F::ONE
        );
        // Whole-segment products are private. The honest incoming recurrence
        // still binds every last-row source/digest cell, including memory rows
        // whose zero event selectors give an identity update factor. Calling
        // the evaluator with (terminal, terminal) would miss this incoming edge.
        for start in [
            ZK_X509_SHA_INPUT_PRODUCTS_V1,
            ZK_X509_SHA_DIGEST_PRODUCTS_V1,
        ] {
            for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
                let mut changed = terminal;
                changed.aux[start + lane] = changed.aux[start + lane].add(F::ONE);
                let residues = evaluate_zk_x509_sha_batch_residues_v1(
                    &previous,
                    &changed,
                    word_challenges(),
                    challenges(),
                    &rfc_challenges(),
                    standalone_segment_terminal(&call),
                    &ca_call_boundaries_for_calls(&[&call]),
                )
                .expect("private terminal recurrence mutation residues");
                assert_eq!(
                    residues
                        .iter()
                        .copied()
                        .filter(|residue| *residue != F::ZERO)
                        .collect::<Vec<_>>(),
                    [F::ONE],
                    "last-row product column {} must retain its own incoming AIR binding",
                    start + lane
                );
            }
        }
        let reordered_current = ZkX509ShaBatchRowV1 {
            base: call.base_row(1).expect("reordered base"),
            aux: call.aux_row(1).expect("reordered aux"),
            fixed: call.fixed_row(0).expect("reordered fixed"),
        };
        let reordered_next = ZkX509ShaBatchRowV1 {
            base: call.base_row(0).expect("reordered next base"),
            aux: call.aux_row(0).expect("reordered next aux"),
            fixed: call.fixed_row(1).expect("reordered next fixed"),
        };
        assert!(
            evaluate_zk_x509_sha_batch_residues_v1(
                &reordered_current,
                &reordered_next,
                word_challenges(),
                challenges(),
                &rfc_challenges(),
                standalone_segment_terminal(&call),
                &ca_call_boundaries_for_calls(&[&call]),
            )
            .expect("reordered residues")
            .iter()
            .any(|residue| *residue != F::ZERO)
        );
        let dropped_current = ZkX509ShaBatchRowV1 {
            base: call.base_row(0).expect("drop current base"),
            aux: call.aux_row(0).expect("drop current aux"),
            fixed: call.fixed_row(0).expect("drop current fixed"),
        };
        let dropped_next = ZkX509ShaBatchRowV1 {
            base: call.base_row(2).expect("drop next base"),
            aux: call.aux_row(2).expect("drop next aux"),
            fixed: call.fixed_row(2).expect("drop next fixed"),
        };
        assert!(
            evaluate_zk_x509_sha_batch_residues_v1(
                &dropped_current,
                &dropped_next,
                word_challenges(),
                challenges(),
                &rfc_challenges(),
                standalone_segment_terminal(&call),
                &ca_call_boundaries_for_calls(&[&call]),
            )
            .expect("dropped-row residues")
            .iter()
            .any(|residue| *residue != F::ZERO)
        );
    }
    #[test]
    fn segment_products_continue_across_whole_call_boundaries() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        })
        .expect("schedule");
        let first_manifest = schedule.calls[ZK_X509_SHA_CA_LEAF_CALL_V1];
        let second_manifest = schedule.calls[ZK_X509_SHA_CA_NODE_CALL_START_V1];
        let first = build_zk_x509_sha_batch_call_trace_with_initial_products_v1(
            first_manifest,
            &witness_for(first_manifest),
            word_challenges(),
            challenges(),
            &rfc_challenges(),
            4,
            &ZkX509ShaSegmentProductStateV1::one_v1(),
        )
        .expect("first aggregate call");
        let second = build_zk_x509_sha_batch_call_trace_with_initial_products_v1(
            second_manifest,
            &witness_for(second_manifest),
            word_challenges(),
            challenges(),
            &rfc_challenges(),
            4,
            &first.segment_product_state,
        )
        .expect("continued aggregate call");
        let mut current = first.row(first.logical_rows() - 1).expect("first terminal");
        current.fixed[ZK_X509_SHA_FIXED_SEGMENT_LAST_V1] = F::ZERO;
        let mut next = second.row(0).expect("second first");
        next.fixed[ZK_X509_SHA_FIXED_SEGMENT_FIRST_V1] = F::ZERO;
        let terminal_owner = second
            .segment_product_state
            .terminal_v1(0)
            .expect("segment terminal");
        let terminal = terminal_owner.segment;
        let ca_call_boundaries = ca_call_boundaries_for_calls(&[&first, &second]);
        assert!(
            evaluate_zk_x509_sha_batch_residues_v1(
                &current,
                &next,
                word_challenges(),
                challenges(),
                &rfc_challenges(),
                terminal,
                &ca_call_boundaries,
            )
            .expect("whole-call boundary")
            .iter()
            .all(|residue| *residue == F::ZERO)
        );
        let restarted = build_zk_x509_sha_batch_call_trace_v1(
            second_manifest,
            &witness_for(second_manifest),
            word_challenges(),
            challenges(),
            &rfc_challenges(),
            4,
        )
        .expect("independently restarted call");
        let mut restarted_next = restarted.row(0).expect("restarted first");
        restarted_next.fixed[ZK_X509_SHA_FIXED_SEGMENT_FIRST_V1] = F::ZERO;
        assert!(
            evaluate_zk_x509_sha_batch_residues_v1(
                &current,
                &restarted_next,
                word_challenges(),
                challenges(),
                &rfc_challenges(),
                terminal,
                &ca_call_boundaries,
            )
            .expect("restarted boundary residues")
            .iter()
            .any(|residue| *residue != F::ZERO)
        );
        let last = second.row(second.logical_rows() - 1).expect("segment last");
        assert!(
            evaluate_zk_x509_sha_batch_residues_v1(
                &last,
                &last,
                word_challenges(),
                challenges(),
                &rfc_challenges(),
                terminal,
                &ca_call_boundaries,
            )
            .expect("segment terminal residues")
            .iter()
            .all(|residue| *residue == F::ZERO)
        );
        let first_row = first.row(0).expect("first compact-CA row");
        let first_next = first.row(1).expect("second compact-CA row");
        let mut wrong_start = ca_call_boundaries;
        wrong_start[0].source_start_products[0] =
            wrong_start[0].source_start_products[0].add(F::ONE);
        assert!(
            evaluate_zk_x509_sha_batch_residues_v1(
                &first_row,
                &first_next,
                word_challenges(),
                challenges(),
                &rfc_challenges(),
                terminal,
                &wrong_start,
            )
            .expect("canonical identity with false start claim")
            .iter()
            .any(|residue| *residue != F::ZERO),
            "each compact-CA call start is independently proof-bound"
        );
        let mut wrong_terminal = ca_call_boundaries;
        wrong_terminal[0].digest_products[1] = wrong_terminal[0].digest_products[1].add(F::ONE);
        assert!(
            evaluate_zk_x509_sha_batch_residues_v1(
                &current,
                &next,
                word_challenges(),
                challenges(),
                &rfc_challenges(),
                terminal,
                &wrong_terminal,
            )
            .expect("canonical identity with false terminal claim")
            .iter()
            .any(|residue| *residue != F::ZERO),
            "each compact-CA call terminal is independently proof-bound"
        );
        let mut wrong_identity = ca_call_boundaries;
        wrong_identity.swap(0, 1);
        assert_eq!(
            evaluate_zk_x509_sha_batch_residues_v1(
                &first_row,
                &first_next,
                word_challenges(),
                challenges(),
                &rfc_challenges(),
                terminal,
                &wrong_identity,
            ),
            Err(ZkX509ShaCallBusStarkErrorV1::Terminal),
            "call identities and roles cannot be reordered"
        );
    }
    #[test]
    fn opened_fixed_selectors_are_algebraic_and_padding_is_fail_closed() {
        let terminal_owner = ZkX509ShaSegmentProductStateV1::one_v1()
            .terminal_v1(0)
            .expect("empty segment terminal");
        let terminal = terminal_owner.segment;
        let padding = physical_padding_row_v1(123);
        assert!(
            evaluate_zk_x509_sha_batch_residues_v1(
                &padding,
                &padding,
                word_challenges(),
                challenges(),
                &rfc_challenges(),
                terminal,
                &neutral_ca_call_boundaries(),
            )
            .expect("canonical padding")
            .iter()
            .all(|residue| *residue == F::ZERO)
        );
        let mut malformed_padding = padding;
        malformed_padding.base[7] = F::ONE;
        assert!(
            evaluate_zk_x509_sha_batch_residues_v1(
                &malformed_padding,
                &padding,
                word_challenges(),
                challenges(),
                &rfc_challenges(),
                terminal,
                &neutral_ca_call_boundaries(),
            )
            .expect("malformed padding residues")
            .iter()
            .any(|residue| *residue != F::ZERO)
        );
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        })
        .expect("schedule");
        let manifest = schedule.calls[CRL_ISSUER_SPKI_CALL_V1];
        let call = build_zk_x509_sha_batch_call_trace_v1(
            manifest,
            &witness_for(manifest),
            word_challenges(),
            challenges(),
            &rfc_challenges(),
            4,
        )
        .expect("RFC call");
        let event_row = (0..call.logical_rows())
            .find(|index| {
                call.fixed_row(*index).is_ok_and(|fixed| {
                    fixed[ZK_X509_SHA_FIXED_RFC_STREAMS_V1 + ZK_X509_SHA_FIXED_RFC_MESSAGE_EVENT_V1]
                        == F::ONE
                })
            })
            .expect("RFC message event");
        let mut opened = call.row(event_row).expect("event row");
        opened.fixed[ZK_X509_SHA_FIXED_RFC_STREAMS_V1 + ZK_X509_SHA_FIXED_RFC_MESSAGE_EVENT_V1] =
            F(2);
        assert!(
            evaluate_zk_x509_sha_batch_residues_v1(
                &opened,
                &call.row(event_row + 1).expect("event successor"),
                word_challenges(),
                challenges(),
                &rfc_challenges(),
                standalone_segment_terminal(&call),
                &ca_call_boundaries_for_calls(&[&call]),
            )
            .expect("non-Boolean fixed opening is evaluated algebraically")
            .iter()
            .any(|residue| *residue != F::ZERO)
        );
        opened = call.row(event_row).expect("event row");
        opened.fixed[ZK_X509_SHA_FIXED_PHYSICAL_PADDING_V1] = F(2);
        assert!(
            evaluate_zk_x509_sha_batch_residues_v1(
                &opened,
                &call.row(event_row + 1).expect("event successor"),
                word_challenges(),
                challenges(),
                &rfc_challenges(),
                standalone_segment_terminal(&call),
                &ca_call_boundaries_for_calls(&[&call]),
            )
            .expect("non-Boolean padding opening is evaluated algebraically")
            .iter()
            .any(|residue| *residue != F::ZERO)
        );
    }
    #[test]
    fn challenge_families_fail_closed() {
        challenges().validate().expect("challenges");
        let mut zero = challenges();
        zero.lanes[0].terms[0] = F::ZERO;
        assert_eq!(
            zero.validate(),
            Err(ZkX509ShaCallBusStarkErrorV1::Challenge)
        );
        let mut duplicate = challenges();
        duplicate.lanes[3].terms[6] = duplicate.lanes[0].terms[0];
        assert_eq!(
            duplicate.validate(),
            Err(ZkX509ShaCallBusStarkErrorV1::Challenge)
        );
        let mut noncanonical = challenges();
        noncanonical.lanes[1].terms[2] = F(GOLDILOCKS_MODULUS_V1);
        assert_eq!(
            noncanonical.validate(),
            Err(ZkX509ShaCallBusStarkErrorV1::Challenge)
        );
    }
    #[test]
    fn four_lane_collision_inventory_includes_projection_without_certifying_joint_security() {
        assert_eq!(ZK_X509_SHA_PROJECTION_EVENT_COUNT_V1, 3_647);
        assert_eq!(ZK_X509_SHA_CALL_BUS_COLLISION_NUMERATOR_V1, 13_735);
        let separate_numerators = 10_088_u128.pow(4) + 3_647_u128.pow(4);
        assert!(separate_numerators <= 13_735_u128.pow(4));
        let (bus_union_bits, call_bits, total_bits) = algebraic_security_bits_v1();
        assert!(
            (bus_union_bits - 173.68803543055358).abs() < 1.0e-12,
            "{bus_union_bits}"
        );
        assert!(
            (call_bits - 201.0177228388035).abs() < 1.0e-12,
            "{call_bits}"
        );
        assert!(
            (total_bits - 165.6504198933986).abs() < 1.0e-12,
            "{total_bits}"
        );
        assert!(bus_union_bits > 173.688, "{bus_union_bits}");
        assert!(call_bits > 200.0, "{call_bits}");
        assert!(total_bits > 165.65, "{total_bits}");
    }
    #[test]
    fn log22_streaming_resource_estimate_uses_the_real_hash_state_width() {
        assert_eq!(ZK_X509_SHA_ONE_NATIVE_SEGMENT_BYTES_V1, 684 * 1024 * 1024);
        let retained_fields_per_row = ZK_X509_SHA_BATCH_BASE_WIDTH_V1
            + ZK_X509_SHA_BATCH_AUX_WIDTH_V1
            + ZK_X509_SHA_BATCH_FIXED_WIDTH_V1;
        assert_eq!(retained_fields_per_row, 308);
        assert_eq!(
            ZK_X509_SHA_MAX_RETAINED_CALL_FIELD_BYTES_V1,
            ZK_X509_SHA_MAX_CALL_LOGICAL_ROWS_V1 as u64 * retained_fields_per_row as u64 * 8
        );
        assert_eq!(ZK_X509_SHA_MAX_RETAINED_CALL_FIELD_BYTES_V1, 520_475_648);
        const {
            assert!(
                ZK_X509_SHA_MAX_RETAINED_CALL_FIELD_BYTES_V1
                    < ZK_X509_SHA_ONE_NATIVE_SEGMENT_BYTES_V1
            )
        };
        assert_eq!(ZK_X509_SHA_EAGER_NATIVE_BYTES_V1, 2_736 * 1024 * 1024);
        assert_eq!(ZK_X509_MAIN_COMMON_LDE_LOG2_V1, 22);
        assert_eq!(ZK_X509_COMMON_LDE_COLUMN_BYTES_V1, 32 * 1024 * 1024);
        assert_eq!(
            ZK_X509_SHA_ROW_HASH_STATE_BYTES_V1,
            (1_u64 << ZK_X509_MAIN_COMMON_LDE_LOG2_V1) * core::mem::size_of::<Sha256>() as u64
        );
        assert_eq!(
            ZK_X509_SHA_STREAMING_PEAK_BYTES_V1,
            ZK_X509_SHA_ONE_NATIVE_SEGMENT_BYTES_V1
                + ZK_X509_SHA_ROW_HASH_STATE_BYTES_V1
                + 2 * ZK_X509_COMMON_LDE_COLUMN_BYTES_V1
                + ZK_X509_SHA_STREAMING_SCRATCH_BYTES_V1
        );
        const { assert!(ZK_X509_SHA_STREAMING_PEAK_BYTES_V1 < ZK_X509_SHA_PROVER_MEMORY_LIMIT_BYTES_V1) };
        assert_eq!(ZK_X509_SHA_TRACE_OPENING_BYTES_V1, 1_488_384);
    }
    #[test]
    fn invalid_public_shapes_and_event_boundaries_fail_closed() {
        assert_eq!(
            ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
                disclosed_attributes: 5,
            }),
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        );
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        })
        .expect("schedule");
        assert_eq!(
            schedule.fixed_event(ZK_X509_SHA_CALL_EVENT_COUNT_V1),
            Err(ZkX509ShaCallBusStarkErrorV1::Event)
        );
        assert_eq!(
            ZkX509ShaSegmentReplayV1::new(4),
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        );
    }

    #[test]
    fn column_batch_guard_preserves_atomic_finish_and_clears_error_and_unwind() {
        for mode in 0..3 {
            let mut first = [F(3); 4];
            let mut second = [F(7); 4];
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut targets = [&mut first[..], &mut second[..]];
                let mut fills = sha_column_fill_batch_v1(&mut targets).unwrap();
                for row in 0..4 {
                    fills[0].write_v1(row, F((row + 11) as u64));
                    fills[1].write_v1(
                        row,
                        if mode == 1 && row == 3 {
                            F(u64::MAX)
                        } else {
                            F((row + 17) as u64)
                        },
                    );
                }
                assert!(mode != 2, "injected batch unwind");
                finish_sha_column_fill_batch_v1(fills)
            }));
            if mode == 0 {
                assert!(result.unwrap().is_ok());
                assert_eq!(first, [F(11), F(12), F(13), F(14)]);
                assert_eq!(second, [F(17), F(18), F(19), F(20)]);
            } else {
                assert!(if mode == 1 {
                    result.unwrap().is_err()
                } else {
                    result.is_err()
                });
                assert_eq!(first, [F::ZERO; 4]);
                assert_eq!(second, [F::ZERO; 4]);
            }
        }
    }

    #[test]
    fn segment_column_batches_match_independent_row_projection_and_bound_terminals() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 2,
        })
        .unwrap();
        let witnesses = witnesses(&schedule);
        let fixed = ZkX509ShaBatchFixedProviderV1::new_v1(schedule.shape()).unwrap();
        let segment = 0;
        let mut source =
            ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, segment).unwrap();
        let first_base = 3;
        let mut reference = vec![native_column_v1(F::ZERO); 8];
        source
            .for_each_base_fixed_row_v1(|row, base, _| {
                for column in 0..8 {
                    reference[column][row] = base[first_base + column];
                }
            })
            .unwrap();
        let mut actual = vec![native_column_v1(F::ZERO); 8];
        source
            .fill_base_columns_v1(
                segment,
                first_base,
                &mut actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>(),
            )
            .unwrap();
        assert_eq!(actual, reference);
        let mut bound = source.bind_v1(&credential_binding(0x47)).unwrap();
        assert!(
            source
                .fill_base_columns_v1(
                    segment,
                    first_base,
                    &mut actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>()
                )
                .is_err()
        );
        assert_eq!(actual, reference);
        bound
            .replay_base_columns_v1(
                &fixed,
                segment,
                first_base,
                &mut actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>(),
            )
            .unwrap();
        assert_eq!(actual, reference);
        let first_aux = ZK_X509_SHA_INPUT_PRODUCTS_V1;
        assert!(first_aux + 8 <= ZK_X509_SHA_BATCH_AUX_WIDTH_V1);
        let expected_terminal = bound
            .replay_aux_rows_with_air_terminals_v1(|row, aux| {
                for column in 0..8 {
                    reference[column][row] = aux[first_aux + column];
                }
            })
            .unwrap();
        let actual_terminal = bound
            .fill_aux_columns_with_air_terminals_v1(
                segment,
                first_aux,
                &mut actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>(),
            )
            .unwrap();
        assert_eq!(actual_terminal, expected_terminal);
        assert_eq!(actual, reference);
        assert!(!bound.row_stream_emitted_for_test_v1());
        for width in [1, 3] {
            let terminal = bound
                .fill_aux_columns_with_air_terminals_v1(
                    segment,
                    first_aux,
                    &mut actual[..width]
                        .iter_mut()
                        .map(Vec::as_mut_slice)
                        .collect::<Vec<_>>(),
                )
                .unwrap();
            assert_eq!(terminal, expected_terminal);
            assert_eq!(actual, reference);
        }
        for (changed_segment, first_column) in [
            (segment + 1, first_aux),
            (segment, ZK_X509_SHA_BATCH_AUX_WIDTH_V1 - 7),
            (segment, usize::MAX),
        ] {
            assert!(
                bound
                    .fill_aux_columns_with_air_terminals_v1(
                        changed_segment,
                        first_column,
                        &mut actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>()
                    )
                    .is_err()
            );
            assert_eq!(actual, reference);
        }
        assert!(
            bound
                .fill_aux_columns_with_air_terminals_v1(segment, first_aux, &mut [])
                .is_err()
        );
        let mut short = vec![F(91); ZK_X509_SHA_SEGMENT_ROWS_V1 - 1];
        assert!(
            bound
                .fill_aux_columns_with_air_terminals_v1(
                    segment,
                    first_aux,
                    &mut [&mut actual[0], &mut short]
                )
                .is_err()
        );
        assert!(short.iter().all(|value| *value == F(91)));
        assert_eq!(actual, reference);
        bound.zeroize_private_v1();
        assert!(
            bound
                .replay_base_columns_v1(
                    &fixed,
                    segment,
                    first_base,
                    &mut actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>()
                )
                .is_err()
        );
        assert!(
            bound
                .fill_aux_columns_with_air_terminals_v1(
                    segment,
                    first_aux,
                    &mut actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>()
                )
                .is_err()
        );
        assert_eq!(actual, reference);
    }

    // The following four functions preserve the pre-refactor full-row implementation
    // as a test-only oracle, independent of the new emitter and shared traversal.
    #[allow(clippy::too_many_arguments)]
    fn original_write_sha_rfc_fixed_event_v1(
        fixed: &mut [F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
        consumer: ZkX509ShaRfcConsumerChannelsV1,
        stream: usize,
        message: bool,
        length_high: bool,
        length_low: bool,
        channel: u32,
        offset: usize,
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        if stream >= ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1 {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        let start = ZK_X509_SHA_FIXED_RFC_STREAMS_V1
            .checked_add(
                stream
                    .checked_mul(ZK_X509_SHA_FIXED_RFC_STREAM_STRIDE_V1)
                    .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?,
            )
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
        fixed[start + ZK_X509_SHA_FIXED_RFC_MESSAGE_EVENT_V1] = F(u64::from(message));
        fixed[start + ZK_X509_SHA_FIXED_RFC_LENGTH_HIGH_VALUE_V1] = F(u64::from(length_high));
        fixed[start + ZK_X509_SHA_FIXED_RFC_LENGTH_LOW_VALUE_V1] = F(u64::from(length_low));
        fixed[start + ZK_X509_SHA_FIXED_RFC_ROLE_V1] = F(consumer.role as u64);
        fixed[start + ZK_X509_SHA_FIXED_RFC_CHANNEL_V1] = F(u64::from(channel));
        fixed[start + ZK_X509_SHA_FIXED_RFC_OFFSET_V1] =
            F(u64::try_from(offset).map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?);
        Ok(())
    }
    fn original_widened_sha_batch_fixed_row_from_word_v1(
        manifest: ZkX509ShaCallManifestV1,
        word_fixed: &[F; SHA_WORD_CAPACITY_FIXED_WIDTH_V1],
        word_logical_rows: usize,
        consumer: Option<ZkX509ShaRfcConsumerChannelsV1>,
        index: usize,
    ) -> Result<[F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1], ZkX509ShaCallBusStarkErrorV1> {
        if index >= word_logical_rows {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        let mut fixed = [F::ZERO; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1];
        fixed[..SHA_WORD_CAPACITY_FIXED_WIDTH_V1].copy_from_slice(word_fixed);
        fixed[ZK_X509_SHA_FIXED_CALL_V1] = F(u64::from(manifest.call));
        fixed[ZK_X509_SHA_FIXED_ROLE_V1] = F(u64::from(manifest.role.role_code()));
        fixed[ZK_X509_SHA_FIXED_SLOT_V1] = F(u64::from(manifest.role.slot()));
        if let Some(selector) = usize::from(manifest.call)
            .checked_sub(ZK_X509_SHA_CA_LEAF_CALL_V1)
            .filter(|selector| *selector < ZK_X509_SHA_CA_CALL_COUNT_V1)
        {
            fixed[ZK_X509_SHA_FIXED_CA_CALL_SELECTORS_V1 + selector] = F::ONE;
        }
        if word_fixed[SHA_WORD_CAPACITY_INPUT_WORD_V1] == F::ONE {
            let word = usize::try_from(word_fixed[SHA_WORD_CAPACITY_INPUT_WORD_INDEX_V1].0)
                .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?;
            for byte in 0..4 {
                let offset = word
                    .checked_mul(4)
                    .and_then(|start| start.checked_add(byte))
                    .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
                if let Some(expected) = sha_frame_byte_v1(manifest, offset) {
                    fixed[ZK_X509_SHA_FIXED_FRAME_MASK_V1 + byte] = F::ONE;
                    match expected {
                        ShaFrameByteV1::Constant(value) => {
                            fixed[ZK_X509_SHA_FIXED_FRAME_CONSTANT_V1 + byte] = F(u64::from(value))
                        }
                        ShaFrameByteV1::RawLengthHigh => {
                            fixed[ZK_X509_SHA_FIXED_FRAME_LENGTH_HIGH_V1 + byte] = F::ONE
                        }
                        ShaFrameByteV1::RawLengthLow => {
                            fixed[ZK_X509_SHA_FIXED_FRAME_LENGTH_LOW_V1 + byte] = F::ONE
                        }
                    }
                }
            }
        }
        projection_bus::emit_fixed_v1(manifest, word_fixed, |column, value| fixed[column] = value);
        let Some(consumer) = consumer else {
            return Ok(fixed);
        };
        fixed[ZK_X509_SHA_FIXED_RFC_LENGTH_PREFIX_V1] =
            F(u64::try_from(consumer.message_prefix_bytes)
                .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?);
        if word_fixed[SHA_WORD_CAPACITY_INPUT_WORD_V1] == F::ONE {
            let word = usize::try_from(word_fixed[SHA_WORD_CAPACITY_INPUT_WORD_INDEX_V1].0)
                .map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?;
            let raw_end = consumer
                .message_prefix_bytes
                .checked_add(consumer.message_capacity_bytes)
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            for stream in 0..ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1 {
                let sha_offset = word
                    .checked_mul(4)
                    .and_then(|offset| offset.checked_add(stream))
                    .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
                if (consumer.message_prefix_bytes..raw_end).contains(&sha_offset) {
                    write_sha_rfc_fixed_event_v1(
                        &mut fixed,
                        consumer,
                        stream,
                        true,
                        false,
                        false,
                        consumer.message_channel,
                        sha_offset
                            .checked_sub(consumer.message_prefix_bytes)
                            .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?,
                    )?;
                }
            }
        }
        if let Some(length_channel) = consumer.length_channel
            && index + 4 >= word_logical_rows
        {
            let pair = index + 4 - word_logical_rows;
            fixed[ZK_X509_SHA_FIXED_RFC_LENGTH_PAIR_V1] = F::ONE;
            fixed[ZK_X509_SHA_FIXED_RFC_LENGTH_PAIR_INDEX_V1] =
                F(u64::try_from(pair).map_err(|_| ZkX509ShaCallBusStarkErrorV1::Resource)?);
            for stream in 0..2 {
                let offset = pair
                    .checked_mul(2)
                    .and_then(|offset| offset.checked_add(stream))
                    .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
                write_sha_rfc_fixed_event_v1(
                    &mut fixed,
                    consumer,
                    stream,
                    false,
                    offset == 6,
                    offset == 7,
                    length_channel,
                    offset,
                )?;
            }
        }
        Ok(fixed)
    }
    // Independent pre-optimization constructor joins the retained original fixed emitter.
    fn original_constructor_sha_batch_source_v1(
        manifest: ZkX509ShaCallManifestV1,
        witness: &ZkX509ShaCallWitnessV1,
        disclosed_attributes: usize,
    ) -> Result<ZkX509ShaBatchCallBaseSourceV1, ZkX509ShaCallBusStarkErrorV1> {
        validate_witness_v1(manifest, witness)?;
        let rfc_consumer =
            sha_rfc_consumer_channels_v1(manifest.call, manifest.role, disclosed_attributes)?;
        let word = super::super::sha_word_stark::original_build_sha_word_capacity_base_source_v1(
            &witness.message,
            manifest.maximum_message_bytes,
            role_has_exact_message_length_v1(manifest.role),
        )?;
        if word.maximum_blocks() != manifest.maximum_blocks
            || word.maximum_local_rows() != manifest.maximum_local_rows
            || word.maximum_memory_rows() != manifest.maximum_memory_rows
            || word.logical_rows() != manifest.maximum_logical_rows()
        {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        Ok(ZkX509ShaBatchCallBaseSourceV1 {
            manifest,
            word,
            rfc_consumer,
        })
    }
    fn original_for_each_base_fixed_row_v1(
        source: &ZkX509ShaBatchSegmentBaseSourceV1<'_>,
        mut visitor: impl FnMut(
            usize,
            [F; ZK_X509_SHA_BATCH_BASE_WIDTH_V1],
            [F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
        ),
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        source.ensure_base_phase_v1()?;
        validate_sha_segment_replay_plan_v1(source.schedule, source.replay)?;
        let segment_start = source
            .segment()
            .checked_mul(ZK_X509_SHA_SEGMENT_ROWS_V1)
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
        let active_end = segment_start
            .checked_add(source.replay.active_rows())
            .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
        let mut emitted = 0_usize;
        for call_index in ZK_X509_SHA_PHYSICAL_CALL_ORDER_V1 {
            let manifest = source.schedule.call(usize::from(call_index))?;
            let call_start = manifest.first_logical_row;
            let call_end = call_start
                .checked_add(manifest.maximum_logical_rows())
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            let overlap_start = call_start.max(segment_start);
            let overlap_end = call_end.min(active_end);
            if overlap_start >= overlap_end {
                continue;
            }
            if overlap_start != call_start || overlap_end != call_end {
                return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
            }
            let witness = source
                .witnesses
                .get(usize::from(manifest.call))
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Topology)?;
            let call = original_constructor_sha_batch_source_v1(
                manifest,
                witness,
                source.schedule.shape().disclosed_attributes,
            )?;
            for global_row in call_start..call_end {
                let call_row = global_row
                    .checked_sub(call_start)
                    .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
                let segment_row = global_row
                    .checked_sub(segment_start)
                    .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
                let mut fixed = original_widened_sha_batch_fixed_row_from_word_v1(
                    call.manifest,
                    call.word.fixed_row(call_row)?,
                    call.logical_rows(),
                    call.rfc_consumer,
                    call_row,
                )?;
                fixed[ZK_X509_SHA_FIXED_SEGMENT_FIRST_V1] = F(u64::from(segment_row == 0));
                fixed[ZK_X509_SHA_FIXED_SEGMENT_LAST_V1] =
                    F(u64::from(segment_row + 1 == source.replay.active_rows()));
                visitor(segment_row, call.base_row(call_row)?, fixed);
                emitted = emitted
                    .checked_add(1)
                    .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
            }
        }
        for segment_row in source.replay.active_rows()..ZK_X509_SHA_SEGMENT_ROWS_V1 {
            let padding = physical_padding_row_v1(segment_row);
            visitor(segment_row, padding.base, padding.fixed);
            emitted = emitted
                .checked_add(1)
                .ok_or(ZkX509ShaCallBusStarkErrorV1::Resource)?;
        }
        if emitted != ZK_X509_SHA_SEGMENT_ROWS_V1 {
            return Err(ZkX509ShaCallBusStarkErrorV1::Topology);
        }
        Ok(())
    }
    fn original_fill_base_columns_v1(
        source: &ZkX509ShaBatchSegmentBaseSourceV1<'_>,
        segment: usize,
        first_column: usize,
        targets: &mut [&mut [F]],
    ) -> Result<(), ZkX509ShaCallBusStarkErrorV1> {
        validate_sha_column_batch_extent_v1(
            first_column,
            ZK_X509_SHA_BATCH_BASE_WIDTH_V1,
            targets,
        )?;
        source.validate_column_request_v1(
            segment,
            first_column,
            ZK_X509_SHA_BATCH_BASE_WIDTH_V1,
            targets[0],
        )?;
        let mut fills = sha_column_fill_batch_v1(targets)?;
        original_for_each_base_fixed_row_v1(source, |row, base, _| {
            for (offset, fill) in fills.iter_mut().enumerate() {
                fill.write_v1(row, base[first_column + offset]);
            }
        })?;
        finish_sha_column_fill_batch_v1(fills)
    }
    #[test]
    fn sha_base_only_checked_emitter_preserves_original_refusals() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        })
        .unwrap();
        let manifest = schedule.calls[0];
        let original_consumer = sha_rfc_consumer_channels_v1(manifest.call, manifest.role, 4)
            .unwrap()
            .unwrap();
        let mut fixed = [F::ZERO; SHA_WORD_CAPACITY_FIXED_WIDTH_V1];
        fixed[SHA_WORD_CAPACITY_INPUT_WORD_V1] = F::ONE;
        for mode in 0..5 {
            let mut consumer = original_consumer;
            let mut row = fixed;
            let (rows, index, expected) = match mode {
                0 => (4, 4, Err(ZkX509ShaCallBusStarkErrorV1::Topology)),
                1 => {
                    row[SHA_WORD_CAPACITY_INPUT_WORD_INDEX_V1] = F(u64::MAX);
                    (4, 0, Err(ZkX509ShaCallBusStarkErrorV1::Resource))
                }
                2 => {
                    consumer.message_prefix_bytes = usize::MAX;
                    consumer.message_capacity_bytes = 1;
                    (4, 0, Err(ZkX509ShaCallBusStarkErrorV1::Resource))
                }
                3 => (4, 3, Ok(())),
                _ => (4, 0, Ok(())),
            };
            let original = original_widened_sha_batch_fixed_row_from_word_v1(
                manifest,
                &row,
                rows,
                Some(consumer),
                index,
            );
            assert_eq!(original.as_ref().map(|_| ()).map_err(|e| *e), expected);
            assert_eq!(
                emit_sha_batch_fixed_row_from_word_v1(
                    manifest,
                    &row,
                    rows,
                    Some(consumer),
                    index,
                    |_, _| {}
                ),
                expected
            );
            assert_eq!(
                widened_sha_batch_fixed_row_from_word_v1(
                    manifest,
                    &row,
                    rows,
                    Some(consumer),
                    index
                ),
                original
            );
        }
        let mut full = [F::ZERO; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1];
        let original = original_write_sha_rfc_fixed_event_v1(
            &mut full,
            original_consumer,
            ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1,
            true,
            false,
            false,
            0,
            0,
        );
        assert_eq!(original, Err(ZkX509ShaCallBusStarkErrorV1::Topology));
        assert_eq!(
            emit_sha_rfc_fixed_event_v1(
                &mut |_, _| {},
                original_consumer,
                ZK_X509_SHA_RFC_PRODUCT_STREAMS_V1,
                true,
                false,
                false,
                0,
                0
            ),
            original
        );
        let witness = witness_for(manifest);
        let mut call = build_zk_x509_sha_batch_call_base_source_v1(manifest, &witness, 4).unwrap();
        assert_eq!(
            call.validate_fixed_row_shape_v1(call.logical_rows()),
            call.fixed_row(call.logical_rows()).map(|_| ())
        );
        call.zeroize_private_v1();
        assert_eq!(
            call.validate_fixed_row_shape_v1(0),
            Err(ZkX509ShaCallBusStarkErrorV1::Resource)
        );
        assert!(call.private_is_zeroized_v1());
    }

    #[test]
    fn sha_base_only_late_witness_failure_clears_every_destination() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        })
        .unwrap();
        let mut witnesses = witnesses(&schedule);
        let late_call = usize::from(ZK_X509_SHA_PHYSICAL_CALL_ORDER_V1[1]);
        witnesses[late_call].digest[0] ^= 1;
        // Deliberately bypass construction only in this negative fixture so the
        // original per-call revalidation fails after a preceding call wrote rows.
        let source = ZkX509ShaBatchSegmentBaseSourceV1 {
            retained_base: None,
            schedule: &schedule,
            witnesses: &witnesses,
            replay: ZkX509ShaSegmentReplayV1::new(0).unwrap(),
            bound: false,
        };
        let mut actual = vec![native_column_v1(F(71)); 8];
        let result = source.fill_base_columns_v1(
            0,
            0,
            &mut actual.iter_mut().map(Vec::as_mut_slice).collect::<Vec<_>>(),
        );
        assert_eq!(result, Err(ZkX509ShaCallBusStarkErrorV1::Digest));
        assert!(actual.iter().flatten().all(|value| *value == F::ZERO));
        let mut reference = vec![native_column_v1(F(19)); 8];
        assert_eq!(
            original_fill_base_columns_v1(
                &source,
                0,
                0,
                &mut reference
                    .iter_mut()
                    .map(Vec::as_mut_slice)
                    .collect::<Vec<_>>()
            ),
            result
        );
        assert_eq!(reference, actual);
    }

    #[test]
    fn sha_base_only_fixed_cells_match_original_every_active_call_row() {
        for disclosed in 0..=4 {
            let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
                disclosed_attributes: disclosed,
            })
            .unwrap();
            let witnesses = witnesses(&schedule);
            for call_index in ZK_X509_SHA_PHYSICAL_CALL_ORDER_V1 {
                let manifest = schedule.call(usize::from(call_index)).unwrap();
                let call = build_zk_x509_sha_batch_call_base_source_v1(
                    manifest,
                    &witnesses[usize::from(call_index)],
                    disclosed,
                )
                .unwrap();
                for row in 0..call.logical_rows() {
                    let word_fixed = call.word.fixed_row(row).unwrap();
                    let expected = original_widened_sha_batch_fixed_row_from_word_v1(
                        manifest,
                        word_fixed,
                        call.logical_rows(),
                        call.rfc_consumer,
                        row,
                    )
                    .unwrap();
                    let actual = widened_sha_batch_fixed_row_from_word_v1(
                        manifest,
                        word_fixed,
                        call.logical_rows(),
                        call.rfc_consumer,
                        row,
                    )
                    .unwrap();
                    assert_eq!(
                        actual, expected,
                        "disclosed={disclosed} call={call_index} row={row}"
                    );
                    assert!(call.validate_fixed_row_shape_v1(row).is_ok());
                }
            }
        }
    }

    /// Exhaust every adjacent batch and tail without retaining a full segment matrix.
    #[test]
    #[ignore = "explicit bounded exhaustive replay gate; reconstructs all four native segments for every column batch"]
    fn sha_combined_all_columns_match_original_before_and_after_binding() {
        for disclosed in 0..=4 {
            let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
                disclosed_attributes: disclosed,
            })
            .unwrap();
            let witnesses = witnesses(&schedule);
            for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
                let mut source =
                    ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, segment)
                        .unwrap();
                let mut actual = vec![native_column_v1(F::ZERO); 8];
                let mut expected = vec![native_column_v1(F::ZERO); 8];
                for first in (0..ZK_X509_SHA_BATCH_BASE_WIDTH_V1).step_by(8) {
                    let width = (ZK_X509_SHA_BATCH_BASE_WIDTH_V1 - first).min(8);
                    original_fill_base_columns_v1(
                        &source,
                        segment,
                        first,
                        &mut expected[..width]
                            .iter_mut()
                            .map(Vec::as_mut_slice)
                            .collect::<Vec<_>>(),
                    )
                    .unwrap();
                    source
                        .fill_base_columns_v1(
                            segment,
                            first,
                            &mut actual[..width]
                                .iter_mut()
                                .map(Vec::as_mut_slice)
                                .collect::<Vec<_>>(),
                        )
                        .unwrap();
                    assert_eq!(
                        actual[..width],
                        expected[..width],
                        "before binding: disclosed={disclosed} segment={segment} first={first}"
                    );
                }
                let fixed = ZkX509ShaBatchFixedProviderV1::new_v1(schedule.shape()).unwrap();
                let mut bound = source.bind_v1(&credential_binding(0x47)).unwrap();
                for first in (0..ZK_X509_SHA_BATCH_BASE_WIDTH_V1).step_by(8) {
                    let width = (ZK_X509_SHA_BATCH_BASE_WIDTH_V1 - first).min(8);
                    let original_source = ZkX509ShaBatchSegmentBaseSourceV1 {
                        retained_base: None,
                        schedule: &schedule,
                        witnesses: &witnesses,
                        replay: source.replay,
                        bound: false,
                    };
                    original_fill_base_columns_v1(
                        &original_source,
                        segment,
                        first,
                        &mut expected[..width]
                            .iter_mut()
                            .map(Vec::as_mut_slice)
                            .collect::<Vec<_>>(),
                    )
                    .unwrap();
                    bound
                        .replay_base_columns_v1(
                            &fixed,
                            segment,
                            first,
                            &mut actual[..width]
                                .iter_mut()
                                .map(Vec::as_mut_slice)
                                .collect::<Vec<_>>(),
                        )
                        .unwrap();
                    assert_eq!(
                        actual[..width],
                        expected[..width],
                        "after binding: disclosed={disclosed} segment={segment} first={first}"
                    );
                }
                assert_eq!(
                    source.fill_base_columns_v1(segment, 0, &mut [&mut actual[0]]),
                    Err(ZkX509ShaCallBusStarkErrorV1::Phase)
                );
                bound.zeroize_private_v1();
                assert_eq!(
                    bound.replay_base_columns_v1(
                        &fixed,
                        segment,
                        0,
                        &mut [actual[0].as_mut_slice()]
                    ),
                    Err(ZkX509ShaCallBusStarkErrorV1::Phase)
                );
                for column in actual.iter_mut().chain(&mut expected) {
                    zeroize_fields_v1(column);
                }
                eprintln!(
                    "SHA_COMBINED_EXHAUSTIVE_V1 disclosed={disclosed} segment={segment} columns={ZK_X509_SHA_BATCH_BASE_WIDTH_V1} before_and_after=exact"
                );
            }
        }
    }

    #[test]
    fn sha_base_only_refuses_changed_replay_plan_and_short_destination_before_writes() {
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        })
        .unwrap();
        let witnesses = witnesses(&schedule);
        let mut changed = schedule.clone();
        changed.calls[usize::from(ZK_X509_SHA_PHYSICAL_CALL_ORDER_V1[1])].first_logical_row += 1;
        let source = ZkX509ShaBatchSegmentBaseSourceV1 {
            retained_base: None,
            schedule: &changed,
            witnesses: &witnesses,
            replay: ZkX509ShaSegmentReplayV1::new(0).unwrap(),
            bound: false,
        };
        let mut target = native_column_v1(F(23));
        assert_eq!(
            source.fill_base_columns_v1(0, 0, &mut [&mut target]),
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        );
        assert!(target.iter().all(|value| *value == F(23)));
        assert_eq!(
            original_fill_base_columns_v1(&source, 0, 0, &mut [&mut target]),
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        );
        assert!(target.iter().all(|value| *value == F(23)));
        let canonical =
            ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, 0).unwrap();
        let short = &mut target[..ZK_X509_SHA_SEGMENT_ROWS_V1 - 1];
        assert_eq!(
            canonical.fill_base_columns_v1(0, 0, &mut [short]),
            Err(ZkX509ShaCallBusStarkErrorV1::Topology)
        );
        assert!(target.iter().all(|value| *value == F(23)));
        zeroize_fields_v1(&mut target);
    }
    /// Compare the complete bounded source operation, including original call construction.
    #[test]
    #[ignore = "component timing requires a coordinated compiler-idle window"]
    fn sha_base_only_complete_source_matches_original_and_reports_timings() {
        use std::time::Instant;
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        })
        .unwrap();
        let witnesses = witnesses(&schedule);
        for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
            let source =
                ZkX509ShaBatchSegmentBaseSourceV1::new_v1(&schedule, &witnesses, segment).unwrap();
            for (first, width) in [(0, 1), (3, 8), (ZK_X509_SHA_BATCH_BASE_WIDTH_V1 - 3, 3)] {
                let mut original = vec![native_column_v1(F::ZERO); width];
                let mut candidate = vec![native_column_v1(F::ZERO); width];
                for round in 0..3 {
                    let candidate_first = (segment + width + round) % 2 == 0;
                    let run = |old, columns: &mut [Vec<F>]| {
                        let mut targets = columns
                            .iter_mut()
                            .map(Vec::as_mut_slice)
                            .collect::<Vec<_>>();
                        let started = Instant::now();
                        if old {
                            original_fill_base_columns_v1(&source, segment, first, &mut targets)
                                .unwrap();
                        } else {
                            source
                                .fill_base_columns_v1(segment, first, &mut targets)
                                .unwrap();
                        }
                        started.elapsed().as_nanos()
                    };
                    let (original_ns, candidate_ns) = if candidate_first {
                        let candidate_ns = run(false, &mut candidate);
                        (run(true, &mut original), candidate_ns)
                    } else {
                        let original_ns = run(true, &mut original);
                        (original_ns, run(false, &mut candidate))
                    };
                    assert_eq!(original, candidate);
                    eprintln!(
                        "SHA_BASE_ONLY_SOURCE_DIAGNOSTIC_V1 segment={segment} first={first} width={width} round={round} candidate_first={candidate_first} original_ns={original_ns} candidate_ns={candidate_ns} parity=exact"
                    );
                }
                for column in original.iter_mut().chain(&mut candidate) {
                    zeroize_fields_v1(column);
                }
            }
        }
    }
    include!("sha_base_borrowed_timing_tests.rs");
}
