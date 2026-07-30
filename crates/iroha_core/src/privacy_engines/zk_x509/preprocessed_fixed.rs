//! Transparent preprocessing for verifier-fixed zk-X509 LDE columns.
//!
//! A pinned oracle root authenticates only profile-derived fixed columns. The
//! prover supplies the queried rows and the unique minimal Merkle frontier;
//! it never chooses a root. Root generation uses the same Goldilocks IFFT,
//! generator-coset LDE, big-endian field encoding, column order, and Merkle
//! domains as verification. A cache may retain tree material for speed, but
//! consensus verification depends only on the pinned profile and proof.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    io::{Read as _, Seek as _, SeekFrom},
    path::Path,
};

use rayon::prelude::*;
use sha2::{
    Digest as _, Sha256, Sha256VarCore, compress256, digest::core_api::Block as DigestBlock,
};
use thiserror::Error;

use super::{
    p256_aggregate_adapter::{
        P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1, P256_VALUE_EXECUTION_AGGREGATE_FIXED_WIDTH_V1,
        P256_X5S1_SIGNATURES_V1, P256MainAdapterV1, P256MainRegistrationV1,
        P256MainVerifierFixedSourceV1,
    },
    p256_ecdsa_air::P256EcdsaRoleV1,
    p256_value_bus::P256_VALUE_BUS_STARK_FIXED_WIDTH_V1,
    profile::{
        ZK_X509_FRI_QUERY_COUNT_V1, ZK_X509_MAIN_COMMON_LDE_LOG2_V1,
        ZK_X509_MAX_NATIVE_TRACE_LOG2_V1, ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1,
    },
    sha_call_bus_stark::{
        ZK_X509_SHA_BATCH_FIXED_WIDTH_V1, ZK_X509_SHA_CA_CALL_COUNT_V1,
        ZK_X509_SHA_FIXED_CA_CALL_SELECTORS_V1, ZK_X509_SHA_FIXED_CALL_V1,
        ZK_X509_SHA_FIXED_PHYSICAL_PADDING_V1, ZK_X509_SHA_FIXED_ROLE_V1,
        ZK_X509_SHA_FIXED_SEGMENT_FIRST_V1, ZK_X509_SHA_FIXED_SEGMENT_LAST_V1,
        ZK_X509_SHA_FIXED_SLOT_V1, ZK_X509_SHA_SEGMENT_COUNT_V1, ZK_X509_SHA_SEGMENT_ROWS_V1,
        ZkX509ShaBatchFixedProviderV1, ZkX509ShaCallBusStarkErrorV1, ZkX509ShaCallPublicShapeV1,
    },
    sha_word_stark::{
        SHA_WORD_CAPACITY_FIXED_WIDTH_V1, ZK_X509_SHA_WORD_PREPROCESSED_FIXED_WIDTH_V1,
        expand_zk_x509_sha_word_fixed_row_v1, reduce_zk_x509_sha_word_fixed_row_v1,
    },
};
use crate::privacy_engines::{
    aggregate_stark::{
        AggregateStarkErrorV1, maximum_multiproof_frontier_len_v1, multiproof_frontier_len_v1,
        verify_canonical_multiproof_v1,
    },
    transparent_stark::{
        ExactProofReaderV1, GOLDILOCKS_GENERATOR_V1, GoldilocksFieldV1 as F,
        TRANSCRIPT_FRAME_DOMAIN_V1, TransparentStarkErrorV1, append_u16_v1, append_u32_v1,
        append_u64_v1, goldilocks_fft_v1, goldilocks_ifft_v1, goldilocks_primitive_root_v1,
        sha256_frame_v1, sha256_merkle_node_v1,
    },
};

/// Stable preprocessing protocol description committed by the compiled
/// zk-X509 profile.
pub(crate) const ZK_X509_PREPROCESSED_FIXED_DESCRIPTOR_V1: &[u8] = b"zk-x509-preprocessed-fixed-v1-incompatible:wire=X5F1+u16be-version1+u16be-oracle-count+per-oracle-u16be-id+u16be-opening-count+repeated-u32be-index-and-width-u64be-fields+minimal-frontier-hashes32:exact-release-oracles2-ordered-sha1-then-p256-log19-2:verifier-pinned-roots-only:profile-derived-independent-columns-only:six-sha-word-columns-reconstructed-by-fixed-linear-identities:p256-four-certificate-signatures-share-only-identical-role-schedules+wallet-distinct:no-statement-state-time-or-witness-values:goldilocks-modulus=0xffffffff00000001:generator=7:canonical-u64be-fields:column-order-profile-bound:geometry=oracle-nonzero+native-log2-4through19+lde-log2-nativeplus1through25+width-u16-nonzero:native-power-of-two-subgroup:ifft-then-generator-coset-lde:release-root=row-major-batch8-final-partial-lanes-only-no-hash-padding-fields+ifft-and-generator-coset-fft+compact-sha256-midstates+bounded-finalization-chunks+ordered-logarithmic-merkle-frontier:frame-domain=iroha:privacy:transparent-stark:frame:v1:leaf-domain=iroha:privacy:zk-x509:preprocessed-fixed:leaf:v1:leaf-fields=oracle-u16be+native-log2-u8+lde-log2-u8+width-u16be+ordered-u64be-fields:node-domain=iroha:privacy:zk-x509:preprocessed-fixed:node:v1:binary-sha256-merkle:canonical-sorted-unique-indices:max-openings116:minimal-multiproof-frontier:exact-max-wire825776:no-prover-root-on-wire:cache-root-verified-and-optional:first-release";

/// Canonical sidecar magic.
pub(crate) const ZK_X509_PREPROCESSED_FIXED_MAGIC_V1: [u8; 4] = *b"X5F1";
/// Sole sidecar version.
pub(crate) const ZK_X509_PREPROCESSED_FIXED_VERSION_V1: u16 = 1;
/// Maximum fixed openings per oracle: current and next rows for all 58 MAIN
/// query positions.
pub(crate) const ZK_X509_PREPROCESSED_FIXED_MAX_OPENINGS_V1: usize = 116;
/// Exact number of independently pinned fixed oracles in MAIN.
pub(crate) const ZK_X509_PREPROCESSED_FIXED_ORACLE_COUNT_V1: usize = 2;
/// Fixed-column LDE batch required by the release resource profile.
pub(crate) const ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1: usize = 8;
/// Hard cap for the complete fixed-oracle sidecar.
pub(crate) const ZK_X509_PREPROCESSED_FIXED_MAX_WIRE_BYTES_V1: usize = 825_776;
/// Exact maximum minimal-frontier hashes for the 116 leaves obtainable from
/// 58 distinct `query, query + 64 mod 2^25` pairs in the SHA fixed-oracle
/// tree. The constrained witness is certified in
/// `exact_sha_x5f1_maximum_is_383196_bytes`.
pub(crate) const ZK_X509_SHA_PREPROCESSED_FIXED_MAXIMUM_FRONTIER_HASHES_V1: usize = 2_100;
/// Exact maximum encoded one-oracle X5F1 proof under the release geometry.
///
/// This is deliberately tighter than the generic 512 KiB decoder cap. MAIN
/// wire accounting must use this exact value rather than adding the generic
/// cap to its already bounded aggregate.
pub(crate) const ZK_X509_SHA_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1: usize = 383_196;
/// Exact maximum encoded one-oracle P-256 log19 X5F1 proof under the release
/// geometry, including the common eight-byte X5F1 header.
pub(crate) const ZK_X509_P256_LOG19_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1: usize = 442_588;
/// Exact maximum complete two-oracle MAIN X5F1 sidecar. The common eight-byte
/// header occurs once, not once per independently pinned oracle.
pub(crate) const ZK_X509_MAIN_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1: usize = 825_776;
/// Conservative allocator/capacity overhead retained beneath the hard prover
/// peak in addition to all explicitly sized vectors.
const ZK_X509_PREPROCESSED_FIXED_ALLOCATION_RESERVE_BYTES_V1: usize = 256 * 1024 * 1024;
/// Exact SHA-256 leaf-prefix bytes before the ordered fixed-field payload.
const ZK_X509_PREPROCESSED_FIXED_LEAF_PREFIX_BYTES_V1: usize = 130;
/// Bytes retained per row between aligned eight-field SHA-256 updates.
const ZK_X509_PREPROCESSED_FIXED_COMPACT_SHA_STATE_BYTES_V1: usize = 36;
/// Rows finalized in parallel before their digests enter the ordered Merkle
/// frontier. This buffer is bounded independently of the LDE row count.
const ZK_X509_PREPROCESSED_FIXED_FINALIZE_CHUNK_ROWS_V1: usize = 32 * 1024;
/// Release-ceremony RSS cap for the exact log25/width340 root generator.
pub(crate) const ZK_X509_SHA_PREPROCESSED_ROOT_MAX_RSS_BYTES_V1: u64 = 4 * 1024 * 1024 * 1024;
/// Conservative wall-clock envelope on the eight-physical-core release
/// benchmark. The arithmetic work certificate below is authoritative; this
/// duration must be benchmarked before the root is pinned.
pub(crate) const ZK_X509_SHA_PREPROCESSED_ROOT_MAX_SECONDS_V1: u64 = 3_600;

const FIXED_LEAF_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-x509:preprocessed-fixed:leaf:v1";
const FIXED_NODE_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-x509:preprocessed-fixed:node:v1";
const SHA_FIXED_DESCRIPTOR_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:preprocessed-fixed:sha-descriptor:v1";
const P256_LOG19_FIXED_DESCRIPTOR_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:preprocessed-fixed:p256-log19-descriptor:v1";

/// Stable oracle identifier for the first-release SHA fixed oracle.
pub(crate) const ZK_X509_SHA_PREPROCESSED_FIXED_ORACLE_V1: u16 = 1;
/// Stable oracle identifier for the first-release P-256 log19 fixed oracle.
pub(crate) const ZK_X509_P256_LOG19_PREPROCESSED_FIXED_ORACLE_V1: u16 = 2;
/// Exact physical SHA-segment order in every fixed-oracle vector row.
pub(crate) const ZK_X509_SHA_PREPROCESSED_FIXED_SEGMENT_ORDER_V1: [u8;
    ZK_X509_SHA_SEGMENT_COUNT_V1] = [0, 1, 2, 3];
/// Independent profile-derived SHA fixed columns retained per physical segment.
///
/// Sixty-six independent SHA-word columns, the six
/// call/role/slot/boundary/padding topology columns, and thirteen compact-CA
/// call selectors are independent of
/// statements, state, time, and witness data. Six redundant SHA-word columns
/// are reconstructed linearly after authenticating an opening. RFC
/// length/event descriptors are deliberately excluded.
pub(crate) const ZK_X509_SHA_PREPROCESSED_FIXED_COLUMNS_PER_SEGMENT_V1: usize =
    ZK_X509_SHA_WORD_PREPROCESSED_FIXED_WIDTH_V1 + 6 + ZK_X509_SHA_CA_CALL_COUNT_V1;
/// Exact width of the one combined four-segment SHA fixed oracle.
pub(crate) const ZK_X509_SHA_PREPROCESSED_FIXED_WIDTH_V1: usize =
    ZK_X509_SHA_SEGMENT_COUNT_V1 * ZK_X509_SHA_PREPROCESSED_FIXED_COLUMNS_PER_SEGMENT_V1;
/// Exact row-major eight-lane transforms in the release root ceremony.
pub(crate) const ZK_X509_SHA_PREPROCESSED_ROOT_BATCH_COUNT_V1: u64 =
    ((ZK_X509_SHA_PREPROCESSED_FIXED_WIDTH_V1 + ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1 - 1)
        / ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1) as u64;
/// Exact base-field butterfly count for all native IFFTs and common-domain
/// coset FFTs. One row-major butterfly updates eight lanes, so the scalar
/// field-operation count is this value times eight.
pub(crate) const ZK_X509_SHA_PREPROCESSED_ROOT_BATCH_BUTTERFLIES_V1: u64 =
    ZK_X509_SHA_PREPROCESSED_ROOT_BATCH_COUNT_V1
        * (((1_u64 << ZK_X509_MAX_NATIVE_TRACE_LOG2_V1) * ZK_X509_MAX_NATIVE_TRACE_LOG2_V1 as u64)
            / 2
            + ((1_u64 << ZK_X509_MAIN_COMMON_LDE_LOG2_V1)
                * ZK_X509_MAIN_COMMON_LDE_LOG2_V1 as u64)
                / 2);
/// Conservative dominant scalar field-operation ceiling in the row-major
/// transforms:
/// eight lanes each perform one multiply, add, and subtract per butterfly;
/// at most one shared twiddle multiply advances each batch butterfly; inverse
/// scaling and generator-coset scaling each multiply every native coefficient
/// once. The fixed exponentiations are separately bounded by 1,716 radix-two
/// stages and 156 domain-order checks.
pub(crate) const ZK_X509_SHA_PREPROCESSED_ROOT_SCHEDULED_FIELD_OPS_MAX_V1: u64 =
    ZK_X509_SHA_PREPROCESSED_ROOT_BATCH_BUTTERFLIES_V1 * (8 * 3 + 1)
        + ZK_X509_SHA_PREPROCESSED_ROOT_BATCH_COUNT_V1
            * (1_u64 << ZK_X509_MAX_NATIVE_TRACE_LOG2_V1)
            * 8
            * 2;
/// Exact SHA-256 compression blocks for every vector-row leaf after the two
/// common prefix blocks computed once.
///
/// The 340-column release row contains 42 complete eight-field blocks and one
/// four-field suffix.  That suffix shares its only compression block with
/// SHA-256 padding, so counting `ceil(width / 8) + 1` would overstate the
/// ceremony work and, more importantly, would describe a different byte
/// stream from the canonical 340-field leaf.
pub(crate) const ZK_X509_SHA_PREPROCESSED_ROOT_LEAF_SHA_BLOCKS_V1: u64 = (1_u64
    << ZK_X509_MAIN_COMMON_LDE_LOG2_V1)
    * ((ZK_X509_SHA_PREPROCESSED_FIXED_WIDTH_V1 / ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
        + ((2
            + (ZK_X509_SHA_PREPROCESSED_FIXED_WIDTH_V1
                % ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
                * core::mem::size_of::<u64>()
            + 1
            + core::mem::size_of::<u64>()
            + 63)
            / 64)) as u64;
/// Exact row-major eight-lane transforms in the P-256 log19 release root
/// ceremony.
pub(crate) const ZK_X509_P256_LOG19_PREPROCESSED_ROOT_BATCH_COUNT_V1: u64 =
    ((ZK_X509_P256_LOG19_PREPROCESSED_FIXED_WIDTH_V1 + ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1
        - 1)
        / ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1) as u64;
/// Exact batch-butterfly count for the P-256 log19 release root.
pub(crate) const ZK_X509_P256_LOG19_PREPROCESSED_ROOT_BATCH_BUTTERFLIES_V1: u64 =
    ZK_X509_P256_LOG19_PREPROCESSED_ROOT_BATCH_COUNT_V1
        * (((1_u64 << ZK_X509_MAX_NATIVE_TRACE_LOG2_V1) * ZK_X509_MAX_NATIVE_TRACE_LOG2_V1 as u64)
            / 2
            + ((1_u64 << ZK_X509_MAIN_COMMON_LDE_LOG2_V1)
                * ZK_X509_MAIN_COMMON_LDE_LOG2_V1 as u64)
                / 2);
/// Conservative dominant scalar field-operation ceiling for the P-256
/// log19 release root.
pub(crate) const ZK_X509_P256_LOG19_PREPROCESSED_ROOT_SCHEDULED_FIELD_OPS_MAX_V1: u64 =
    ZK_X509_P256_LOG19_PREPROCESSED_ROOT_BATCH_BUTTERFLIES_V1 * (8 * 3 + 1)
        + ZK_X509_P256_LOG19_PREPROCESSED_ROOT_BATCH_COUNT_V1
            * (1_u64 << ZK_X509_MAX_NATIVE_TRACE_LOG2_V1)
            * 8
            * 2;
/// Exact SHA-256 compression blocks for the P-256 vector-row leaves after
/// the two common prefix blocks computed once.
pub(crate) const ZK_X509_P256_LOG19_PREPROCESSED_ROOT_LEAF_SHA_BLOCKS_V1: u64 = (1_u64
    << ZK_X509_MAIN_COMMON_LDE_LOG2_V1)
    * ((ZK_X509_P256_LOG19_PREPROCESSED_FIXED_WIDTH_V1
        / ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
        + ((2
            + (ZK_X509_P256_LOG19_PREPROCESSED_FIXED_WIDTH_V1
                % ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
                * core::mem::size_of::<u64>()
            + 1
            + core::mem::size_of::<u64>()
            + 63)
            / 64)) as u64;
/// Exact internal binary-Merkle node count.
pub(crate) const ZK_X509_SHA_PREPROCESSED_ROOT_MERKLE_NODES_V1: u64 =
    (1_u64 << ZK_X509_MAIN_COMMON_LDE_LOG2_V1) - 1;
/// Exact SHA-256 compression blocks for all framed internal Merkle nodes.
pub(crate) const ZK_X509_SHA_PREPROCESSED_ROOT_MERKLE_SHA_BLOCKS_V1: u64 =
    ZK_X509_SHA_PREPROCESSED_ROOT_MERKLE_NODES_V1 * 3;

/// Exact first-release SHA fixed-column manifest.
pub(crate) const ZK_X509_SHA_PREPROCESSED_FIXED_COLUMN_DESCRIPTOR_V1: &[u8] =
    b"zk-x509-sha-preprocessed-fixed-columns-v1-incompatible:oracle1:four-segments-ordered0,1,2,3:per-segment85=66-independent-sha-word-fixed-columns-in-source-order+call+role+slot+segment-first+segment-last+physical-padding+thirteen-one-hot-compact-ca-call-selectors16through28:reconstruct-sha-word-fixed-padding=0+local-continue=sum-local-operations-minus-local-first-minus-local-last+memory-continue=memory-same-next-plus-memory-new-next+last-aggregate-row=0+physical-boundary=0+boolean-continue=choose-plus-majority-minus-boolean-last:combined-column-order=segment-major-then-listed-source-column:excluded=rfc-length-pair+rfc-length-pair-index+rfc-length-prefix+four-rfc-event-descriptors:shape-independent-for-disclosed-attributes0through4:native-log19:generator-coset-lde-log25:root-must-be-release-pinned:activation=false";

/// Exact unique schedule order in every P-256 log19 fixed-oracle vector row.
///
/// Four certificate/CRL signatures demonstrably share one verifier topology
/// per adapter. The wallet role is distinct. No value from a statement,
/// witness, proof, optional-selection flag, or challenge selects a schedule.
pub(crate) const ZK_X509_P256_LOG19_PREPROCESSED_FIXED_SCHEDULE_ORDER_V1: [u8; 6] =
    [0, 1, 2, 3, 4, 5];
const P256_LOG19_CERTIFICATE_ARITHMETIC_START_V1: usize = 0;
const P256_LOG19_WALLET_ARITHMETIC_START_V1: usize =
    P256_LOG19_CERTIFICATE_ARITHMETIC_START_V1 + P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1;
const P256_LOG19_CERTIFICATE_EXECUTION_START_V1: usize =
    P256_LOG19_WALLET_ARITHMETIC_START_V1 + P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1;
const P256_LOG19_WALLET_EXECUTION_START_V1: usize =
    P256_LOG19_CERTIFICATE_EXECUTION_START_V1 + P256_VALUE_EXECUTION_AGGREGATE_FIXED_WIDTH_V1;
const P256_LOG19_CERTIFICATE_SORTED_START_V1: usize =
    P256_LOG19_WALLET_EXECUTION_START_V1 + P256_VALUE_EXECUTION_AGGREGATE_FIXED_WIDTH_V1;
const P256_LOG19_WALLET_SORTED_START_V1: usize =
    P256_LOG19_CERTIFICATE_SORTED_START_V1 + P256_VALUE_BUS_STARK_FIXED_WIDTH_V1;
/// Exact width of the six unique P-256 log19 verifier schedules.
pub(crate) const ZK_X509_P256_LOG19_PREPROCESSED_FIXED_WIDTH_V1: usize =
    P256_LOG19_WALLET_SORTED_START_V1 + P256_VALUE_BUS_STARK_FIXED_WIDTH_V1;
/// Exact first-release P-256 log19 fixed-column manifest.
pub(crate) const ZK_X509_P256_LOG19_PREPROCESSED_FIXED_COLUMN_DESCRIPTOR_V1: &[u8] =
    b"zk-x509-p256-log19-preprocessed-fixed-columns-v1-incompatible:oracle2:six-unique-verifier-schedules-ordered-certificate-arithmetic134+wallet-arithmetic134+certificate-value-execution46+wallet-value-execution46+certificate-value-sorted22+wallet-value-sorted22:width404:alias-map=arithmetic-signatures0,1,2,3-to-certificate+signature4-to-wallet;value-execution-and-sorted-signatures0,1,2,3-to-certificate+signature4-to-wallet:aliases-permitted-only-for-byte-identical-role-derived-verifier-topology:no-binding-sink-optional-selection:no-window-reduction-low-s-or-scalar-schedules:no-statement-state-time-witness-proof-or-challenge-values:native-log19:generator-coset-lde-log25:root-must-be-release-pinned:first-release";

const _: () = {
    assert!(ZK_X509_SHA_SEGMENT_COUNT_V1 == 4);
    assert!(SHA_WORD_CAPACITY_FIXED_WIDTH_V1 == 72);
    assert!(ZK_X509_SHA_WORD_PREPROCESSED_FIXED_WIDTH_V1 == 66);
    assert!(ZK_X509_SHA_PREPROCESSED_FIXED_COLUMNS_PER_SEGMENT_V1 == 85);
    assert!(ZK_X509_SHA_PREPROCESSED_FIXED_WIDTH_V1 == 340);
    assert!(P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1 == 134);
    assert!(P256_VALUE_EXECUTION_AGGREGATE_FIXED_WIDTH_V1 == 46);
    assert!(P256_VALUE_BUS_STARK_FIXED_WIDTH_V1 == 22);
    assert!(P256_LOG19_WALLET_ARITHMETIC_START_V1 == 134);
    assert!(P256_LOG19_CERTIFICATE_EXECUTION_START_V1 == 268);
    assert!(P256_LOG19_WALLET_EXECUTION_START_V1 == 314);
    assert!(P256_LOG19_CERTIFICATE_SORTED_START_V1 == 360);
    assert!(P256_LOG19_WALLET_SORTED_START_V1 == 382);
    assert!(ZK_X509_P256_LOG19_PREPROCESSED_FIXED_WIDTH_V1 == 404);
    assert!(ZK_X509_SHA_PREPROCESSED_ROOT_BATCH_COUNT_V1 == 43);
    assert!(ZK_X509_SHA_PREPROCESSED_ROOT_BATCH_BUTTERFLIES_V1 == 18_249_678_848);
    assert!(ZK_X509_SHA_PREPROCESSED_ROOT_SCHEDULED_FIELD_OPS_MAX_V1 == 456_602_681_344);
    assert!(ZK_X509_SHA_PREPROCESSED_ROOT_LEAF_SHA_BLOCKS_V1 == 1_442_840_576);
    assert!(ZK_X509_P256_LOG19_PREPROCESSED_ROOT_BATCH_COUNT_V1 == 51);
    assert!(ZK_X509_P256_LOG19_PREPROCESSED_ROOT_BATCH_BUTTERFLIES_V1 == 21_644_967_936);
    assert!(ZK_X509_P256_LOG19_PREPROCESSED_ROOT_SCHEDULED_FIELD_OPS_MAX_V1 == 541_552_017_408);
    assert!(ZK_X509_P256_LOG19_PREPROCESSED_ROOT_LEAF_SHA_BLOCKS_V1 == 1_711_276_032);
    assert!(ZK_X509_SHA_PREPROCESSED_ROOT_MERKLE_NODES_V1 == 33_554_431);
    assert!(ZK_X509_SHA_PREPROCESSED_ROOT_MERKLE_SHA_BLOCKS_V1 == 100_663_293);
    assert!(ZK_X509_MAX_NATIVE_TRACE_LOG2_V1 == 19);
    assert!(ZK_X509_MAIN_COMMON_LDE_LOG2_V1 == 25);
    assert!(ZK_X509_SHA_SEGMENT_ROWS_V1 == 1 << ZK_X509_MAX_NATIVE_TRACE_LOG2_V1);
    assert!(
        ZK_X509_SHA_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1
            == 8 + 4
                + ZK_X509_PREPROCESSED_FIXED_MAX_OPENINGS_V1
                    * (4 + ZK_X509_SHA_PREPROCESSED_FIXED_WIDTH_V1 * 8)
                + ZK_X509_SHA_PREPROCESSED_FIXED_MAXIMUM_FRONTIER_HASHES_V1 * 32
    );
    assert!(
        ZK_X509_SHA_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1
            < ZK_X509_PREPROCESSED_FIXED_MAX_WIRE_BYTES_V1
    );
    assert!(
        ZK_X509_P256_LOG19_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1
            == 8 + 4
                + ZK_X509_PREPROCESSED_FIXED_MAX_OPENINGS_V1
                    * (4 + ZK_X509_P256_LOG19_PREPROCESSED_FIXED_WIDTH_V1 * 8)
                + ZK_X509_SHA_PREPROCESSED_FIXED_MAXIMUM_FRONTIER_HASHES_V1 * 32
    );
    assert!(
        ZK_X509_MAIN_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1
            == ZK_X509_SHA_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1
                + ZK_X509_P256_LOG19_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1
                - 8
    );
    assert!(
        ZK_X509_MAIN_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1
            == ZK_X509_PREPROCESSED_FIXED_MAX_WIRE_BYTES_V1
    );
};

/// Root-independent geometry of one fixed-column preprocessing oracle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509PreprocessedFixedGeometryV1 {
    /// Stable oracle identity and column-manifest selector.
    pub(crate) oracle: u16,
    /// Native fixed-trace logarithm.
    pub(crate) native_log2: u8,
    /// Generator-coset LDE logarithm.
    pub(crate) lde_log2: u8,
    /// Exact ordered column count.
    pub(crate) width: u16,
}

/// Exact geometry of the sole combined SHA fixed oracle.
pub(crate) const ZK_X509_SHA_PREPROCESSED_FIXED_GEOMETRY_V1: ZkX509PreprocessedFixedGeometryV1 =
    ZkX509PreprocessedFixedGeometryV1 {
        oracle: ZK_X509_SHA_PREPROCESSED_FIXED_ORACLE_V1,
        native_log2: ZK_X509_MAX_NATIVE_TRACE_LOG2_V1,
        lde_log2: ZK_X509_MAIN_COMMON_LDE_LOG2_V1,
        width: ZK_X509_SHA_PREPROCESSED_FIXED_WIDTH_V1 as u16,
    };
/// Exact geometry of the combined P-256 log19 fixed oracle.
pub(crate) const ZK_X509_P256_LOG19_PREPROCESSED_FIXED_GEOMETRY_V1:
    ZkX509PreprocessedFixedGeometryV1 = ZkX509PreprocessedFixedGeometryV1 {
    oracle: ZK_X509_P256_LOG19_PREPROCESSED_FIXED_ORACLE_V1,
    native_log2: ZK_X509_MAX_NATIVE_TRACE_LOG2_V1,
    lde_log2: ZK_X509_MAIN_COMMON_LDE_LOG2_V1,
    width: ZK_X509_P256_LOG19_PREPROCESSED_FIXED_WIDTH_V1 as u16,
};

impl ZkX509PreprocessedFixedGeometryV1 {
    fn validate(self) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
        if self.oracle == 0
            || !(4..=19).contains(&self.native_log2)
            || self.lde_log2 <= self.native_log2
            || self.lde_log2 > 25
            || self.width == 0
        {
            return Err(ZkX509PreprocessedFixedErrorV1::Profile);
        }
        Ok(())
    }

    fn native_rows(self) -> Result<usize, ZkX509PreprocessedFixedErrorV1> {
        self.validate()?;
        1_usize
            .checked_shl(u32::from(self.native_log2))
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)
    }

    fn lde_rows(self) -> Result<usize, ZkX509PreprocessedFixedErrorV1> {
        self.validate()?;
        1_usize
            .checked_shl(u32::from(self.lde_log2))
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)
    }
}

/// Verifier-owned profile for one preprocessed fixed oracle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509PreprocessedFixedProfileV1 {
    /// Exact root-independent geometry.
    pub(crate) geometry: ZkX509PreprocessedFixedGeometryV1,
    /// Consensus-pinned LDE vector-row root.
    pub(crate) root: [u8; 32],
}

/// One complete verifier-owned certificate for the combined SHA fixed oracle.
///
/// The certificate binds the generic preprocessing protocol, the exact
/// profile-derived column manifest, geometry, physical segment order, and the
/// actual LDE vector-row Merkle root.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509ShaPreprocessedFixedCertificateV1 {
    /// Exact geometry and authenticated LDE root.
    pub(crate) profile: ZkX509PreprocessedFixedProfileV1,
    /// Explicit physical segment order, independently checked against the
    /// column manifest.
    pub(crate) segment_order: [u8; ZK_X509_SHA_SEGMENT_COUNT_V1],
    /// Digest of both protocol and SHA column-manifest descriptors.
    pub(crate) descriptor_digest: [u8; 32],
}

/// Complete verifier-owned certificate for the P-256 log19 fixed oracle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509P256Log19PreprocessedFixedCertificateV1 {
    /// Exact geometry and authenticated LDE root.
    pub(crate) profile: ZkX509PreprocessedFixedProfileV1,
    /// Explicit unique schedule order, independently checked against the
    /// column manifest and registration alias map.
    pub(crate) schedule_order: [u8; 6],
    /// Digest of both protocol and P-256 log19 column-manifest descriptors.
    pub(crate) descriptor_digest: [u8; 32],
}

impl ZkX509PreprocessedFixedProfileV1 {
    fn validate(self) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
        self.geometry.validate()?;
        if self.root == [0; 32] {
            return Err(ZkX509PreprocessedFixedErrorV1::Profile);
        }
        Ok(())
    }
}

/// Canonical current/next fixed-oracle openings derived from all MAIN queries.
///
/// Construction is intentionally private: callers cannot supply a stride or
/// an already-expanded opening set. The remaining assembly boundary is the
/// origin of the 58 query coordinates themselves; until MAIN transcript
/// assembly exists, this module can validate and expand those coordinates but
/// cannot prove that a caller sampled them from the canonical transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509Log19PreprocessedFixedOpeningIndicesV1 {
    indices: Vec<usize>,
}

impl ZkX509Log19PreprocessedFixedOpeningIndicesV1 {
    pub(crate) fn as_slice_v1(&self) -> &[usize] {
        &self.indices
    }
}

/// One canonical fixed-oracle multiproof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509PreprocessedFixedMultiproofV1 {
    /// Verifier-known oracle identity.
    pub(crate) oracle: u16,
    /// Sorted unique common-LDE indices.
    pub(crate) indices: Vec<u32>,
    /// One exact ordered row per index.
    pub(crate) rows: Vec<Vec<u64>>,
    /// Unique minimal binary-Merkle frontier.
    pub(crate) frontier: Vec<[u8; 32]>,
}

/// Exact ordered collection of preprocessing multiproofs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509PreprocessedFixedProofV1 {
    /// Multiproofs in the verifier-owned profile order.
    pub(crate) oracles: Vec<ZkX509PreprocessedFixedMultiproofV1>,
}

/// Fixed preprocessing construction, codec, or verification failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum ZkX509PreprocessedFixedErrorV1 {
    /// Pinned profile or geometry is invalid.
    #[error("zk-X509 preprocessed fixed profile is invalid")]
    Profile,
    /// A fixed provider returned a malformed or non-canonical column.
    #[error("zk-X509 preprocessed fixed column is invalid")]
    Column,
    /// Opening indices are not the sole bounded canonical set.
    #[error("zk-X509 preprocessed fixed indices are invalid")]
    Index,
    /// The sidecar is malformed, non-canonical, truncated, or has a suffix.
    #[error("zk-X509 preprocessed fixed proof is malformed")]
    MalformedProof,
    /// A row or Merkle frontier does not authenticate to the pinned root.
    #[error("zk-X509 preprocessed fixed opening is invalid")]
    Opening,
    /// Checked resource or allocation envelope was exceeded.
    #[error("zk-X509 preprocessed fixed resource envelope is exceeded")]
    Resource,
    /// Regeneration did not reproduce the consensus-pinned root.
    #[error("zk-X509 preprocessed fixed root mismatch")]
    RootMismatch,
    /// No independently checked release root has been compiled yet.
    #[error("zk-X509 preprocessed fixed root is not release-pinned")]
    Unpinned,
    /// An immutable preprocessing artifact is absent, malformed, stale, or
    /// cannot be read exactly.
    #[error("zk-X509 preprocessed fixed artifact is invalid")]
    Artifact,
}

/// Root-authenticated opening service for the release-preprocessed MAIN
/// oracles.
///
/// A production implementation may be backed by an offline LDE/Merkle
/// artifact or a bounded local service. The service is untrusted: the caller
/// verifies every returned row and frontier against both compiled roots before
/// the bytes can enter a proof envelope.
pub(crate) trait ZkX509MainPreprocessedFixedOpeningServiceV1 {
    fn open_main_v1(
        &mut self,
        profiles: &[ZkX509PreprocessedFixedProfileV1; ZK_X509_PREPROCESSED_FIXED_ORACLE_COUNT_V1],
        indices: &ZkX509Log19PreprocessedFixedOpeningIndicesV1,
    ) -> Result<Vec<u8>, ZkX509PreprocessedFixedErrorV1>;
}

/// Exact package-manifest filename. Oracle filenames are derived from their
/// verifier-owned identifiers and never accepted from artifact bytes.
pub(crate) const ZK_X509_MAIN_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_FILE_V1: &str = "main.x5a1";
const ZK_X509_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_MAGIC_V1: [u8; 4] = *b"X5A1";
const ZK_X509_PREPROCESSED_FIXED_ARTIFACT_ORACLE_MAGIC_V1: [u8; 4] = *b"X5O1";
const ZK_X509_PREPROCESSED_FIXED_ARTIFACT_VERSION_V1: u16 = 1;
const ZK_X509_PREPROCESSED_FIXED_ARTIFACT_LAYOUT_BATCH8_V1: u16 = 1;
const ZK_X509_PREPROCESSED_FIXED_ARTIFACT_FIELD_CODEC_U64BE_V1: u16 = 1;
const ZK_X509_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_BYTES_V1: usize = 512;
const ZK_X509_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_PREFIX_BYTES_V1: usize = 48;
const ZK_X509_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_ENTRY_BYTES_V1: usize = 160;
const ZK_X509_PREPROCESSED_FIXED_ARTIFACT_CERTIFICATE_SLOT_BYTES_V1: usize = 84;
const ZK_X509_PREPROCESSED_FIXED_ARTIFACT_ORACLE_HEADER_BYTES_V1: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
struct ZkX509PreprocessedFixedArtifactBindingV1 {
    profile: ZkX509PreprocessedFixedProfileV1,
    descriptor_digest: [u8; 32],
    certificate: Vec<u8>,
}

#[derive(Debug)]
struct ZkX509PreprocessedFixedOracleArtifactV1 {
    file: File,
    profile: ZkX509PreprocessedFixedProfileV1,
    header: [u8; ZK_X509_PREPROCESSED_FIXED_ARTIFACT_ORACLE_HEADER_BYTES_V1],
    rows_offset: u64,
    tree_offset: u64,
    file_bytes: u64,
}

/// Concrete immutable random-access backend for the two release-preprocessed
/// MAIN fixed oracles.
///
/// Each `X5O1` file stores batch-major LDE rows followed by a level-major full
/// binary Merkle tree. The `X5A1` manifest binds their exact order,
/// certificates, roots, descriptor digests, file lengths, and the compiled
/// profile digest. Offsets and lengths are derived exclusively from the
/// verifier-owned geometry. Runtime reads remain untrusted: every emitted
/// sidecar is independently reverified against both pinned roots by
/// [`request_zk_x509_main_preprocessed_fixed_openings_v1`].
#[derive(Debug)]
pub(crate) struct ZkX509MainPreprocessedFixedArtifactV1 {
    compiled_profile_digest: [u8; 32],
    profiles: [ZkX509PreprocessedFixedProfileV1; ZK_X509_PREPROCESSED_FIXED_ORACLE_COUNT_V1],
    oracles: [ZkX509PreprocessedFixedOracleArtifactV1; ZK_X509_PREPROCESSED_FIXED_ORACLE_COUNT_V1],
}

fn artifact_u16_v1(bytes: &[u8], offset: usize) -> Result<u16, ZkX509PreprocessedFixedErrorV1> {
    let end = offset
        .checked_add(2)
        .ok_or(ZkX509PreprocessedFixedErrorV1::Artifact)?;
    Ok(u16::from_be_bytes(
        bytes
            .get(offset..end)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Artifact)?
            .try_into()
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Artifact)?,
    ))
}

fn artifact_u64_v1(bytes: &[u8], offset: usize) -> Result<u64, ZkX509PreprocessedFixedErrorV1> {
    let end = offset
        .checked_add(8)
        .ok_or(ZkX509PreprocessedFixedErrorV1::Artifact)?;
    Ok(u64::from_be_bytes(
        bytes
            .get(offset..end)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Artifact)?
            .try_into()
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Artifact)?,
    ))
}

fn artifact_array32_v1(
    bytes: &[u8],
    offset: usize,
) -> Result<[u8; 32], ZkX509PreprocessedFixedErrorV1> {
    let end = offset
        .checked_add(32)
        .ok_or(ZkX509PreprocessedFixedErrorV1::Artifact)?;
    bytes
        .get(offset..end)
        .ok_or(ZkX509PreprocessedFixedErrorV1::Artifact)?
        .try_into()
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Artifact)
}

fn artifact_oracle_filename_v1(oracle: u16) -> String {
    format!("oracle-{oracle:04}.x5o1")
}

fn artifact_geometry_lengths_v1(
    geometry: ZkX509PreprocessedFixedGeometryV1,
) -> Result<(u64, u64, u64), ZkX509PreprocessedFixedErrorV1> {
    geometry.validate()?;
    let rows = u64::try_from(geometry.lde_rows()?)
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
    let row_bytes = rows
        .checked_mul(u64::from(geometry.width))
        .and_then(|fields| fields.checked_mul(8))
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let tree_nodes = rows
        .checked_mul(2)
        .and_then(|nodes| nodes.checked_sub(1))
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let tree_bytes = tree_nodes
        .checked_mul(32)
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let file_bytes = u64::try_from(ZK_X509_PREPROCESSED_FIXED_ARTIFACT_ORACLE_HEADER_BYTES_V1)
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?
        .checked_add(row_bytes)
        .and_then(|bytes| bytes.checked_add(tree_bytes))
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    Ok((row_bytes, tree_bytes, file_bytes))
}

fn read_exact_artifact_at_v1(
    file: &mut File,
    offset: u64,
    output: &mut [u8],
) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
    file.seek(SeekFrom::Start(offset))
        .and_then(|_| file.read_exact(output))
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Artifact)
}

fn open_exact_artifact_file_v1(
    path: &Path,
    expected_bytes: u64,
) -> Result<File, ZkX509PreprocessedFixedErrorV1> {
    let file = OpenOptions::new()
        .read(true)
        .write(false)
        .open(path)
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Artifact)?;
    let metadata = file
        .metadata()
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Artifact)?;
    if !metadata.is_file() || metadata.len() != expected_bytes {
        return Err(ZkX509PreprocessedFixedErrorV1::Artifact);
    }
    Ok(file)
}

fn read_exact_artifact_manifest_v1(
    package: &Path,
) -> Result<
    [u8; ZK_X509_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_BYTES_V1],
    ZkX509PreprocessedFixedErrorV1,
> {
    let path = package.join(ZK_X509_MAIN_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_FILE_V1);
    let mut file = open_exact_artifact_file_v1(
        &path,
        u64::try_from(ZK_X509_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_BYTES_V1)
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?,
    )?;
    let mut manifest = [0_u8; ZK_X509_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_BYTES_V1];
    read_exact_artifact_at_v1(&mut file, 0, &mut manifest)?;
    Ok(manifest)
}

fn artifact_manifest_entry_offset_v1(
    index: usize,
) -> Result<usize, ZkX509PreprocessedFixedErrorV1> {
    index
        .checked_mul(ZK_X509_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_ENTRY_BYTES_V1)
        .and_then(|bytes| {
            ZK_X509_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_PREFIX_BYTES_V1.checked_add(bytes)
        })
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)
}

fn validate_artifact_manifest_entry_v1(
    manifest: &[u8; ZK_X509_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_BYTES_V1],
    index: usize,
    binding: &ZkX509PreprocessedFixedArtifactBindingV1,
) -> Result<u64, ZkX509PreprocessedFixedErrorV1> {
    let start = artifact_manifest_entry_offset_v1(index)?;
    let end = start
        .checked_add(ZK_X509_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_ENTRY_BYTES_V1)
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let entry = manifest
        .get(start..end)
        .ok_or(ZkX509PreprocessedFixedErrorV1::Artifact)?;
    let certificate_len = usize::from(artifact_u16_v1(entry, 2)?);
    let (_, _, expected_file_bytes) = artifact_geometry_lengths_v1(binding.profile.geometry)?;
    if artifact_u16_v1(entry, 0)? != binding.profile.geometry.oracle
        || certificate_len != binding.certificate.len()
        || certificate_len > ZK_X509_PREPROCESSED_FIXED_ARTIFACT_CERTIFICATE_SLOT_BYTES_V1
        || artifact_u64_v1(entry, 4)? != expected_file_bytes
        || artifact_array32_v1(entry, 12)? != binding.profile.root
        || artifact_array32_v1(entry, 44)? != binding.descriptor_digest
        || entry.get(76..76 + certificate_len) != Some(binding.certificate.as_slice())
        || entry
            .get(76 + certificate_len..)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Artifact)?
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(ZkX509PreprocessedFixedErrorV1::Artifact);
    }
    Ok(expected_file_bytes)
}

fn open_artifact_oracle_v1(
    package: &Path,
    compiled_profile_digest: [u8; 32],
    binding: &ZkX509PreprocessedFixedArtifactBindingV1,
    expected_file_bytes: u64,
) -> Result<ZkX509PreprocessedFixedOracleArtifactV1, ZkX509PreprocessedFixedErrorV1> {
    let geometry = binding.profile.geometry;
    let path = package.join(artifact_oracle_filename_v1(geometry.oracle));
    let mut file = open_exact_artifact_file_v1(&path, expected_file_bytes)?;
    let mut header = [0_u8; ZK_X509_PREPROCESSED_FIXED_ARTIFACT_ORACLE_HEADER_BYTES_V1];
    read_exact_artifact_at_v1(&mut file, 0, &mut header)?;
    let certificate_len = usize::from(artifact_u16_v1(&header, 20)?);
    let (row_bytes, tree_bytes, derived_file_bytes) = artifact_geometry_lengths_v1(geometry)?;
    if header[..4] != ZK_X509_PREPROCESSED_FIXED_ARTIFACT_ORACLE_MAGIC_V1
        || artifact_u16_v1(&header, 4)? != ZK_X509_PREPROCESSED_FIXED_ARTIFACT_VERSION_V1
        || artifact_u16_v1(&header, 6)?
            != u16::try_from(ZK_X509_PREPROCESSED_FIXED_ARTIFACT_ORACLE_HEADER_BYTES_V1)
                .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?
        || artifact_u16_v1(&header, 8)? != ZK_X509_PREPROCESSED_FIXED_ARTIFACT_LAYOUT_BATCH8_V1
        || artifact_u16_v1(&header, 10)? != geometry.oracle
        || header[12] != geometry.native_log2
        || header[13] != geometry.lde_log2
        || artifact_u16_v1(&header, 14)? != geometry.width
        || artifact_u16_v1(&header, 16)?
            != u16::try_from(ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
                .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?
        || artifact_u16_v1(&header, 18)? != ZK_X509_PREPROCESSED_FIXED_ARTIFACT_FIELD_CODEC_U64BE_V1
        || certificate_len != binding.certificate.len()
        || certificate_len > ZK_X509_PREPROCESSED_FIXED_ARTIFACT_CERTIFICATE_SLOT_BYTES_V1
        || header[22..24].iter().any(|byte| *byte != 0)
        || artifact_array32_v1(&header, 24)? != compiled_profile_digest
        || artifact_array32_v1(&header, 56)? != binding.profile.root
        || artifact_array32_v1(&header, 88)? != binding.descriptor_digest
        || artifact_u64_v1(&header, 120)? != row_bytes
        || artifact_u64_v1(&header, 128)? != tree_bytes
        || artifact_u64_v1(&header, 136)? != derived_file_bytes
        || derived_file_bytes != expected_file_bytes
        || header.get(144..144 + certificate_len) != Some(binding.certificate.as_slice())
        || header
            .get(144 + certificate_len..)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Artifact)?
            .iter()
            .any(|byte| *byte != 0)
    {
        return Err(ZkX509PreprocessedFixedErrorV1::Artifact);
    }
    let rows_offset = u64::try_from(ZK_X509_PREPROCESSED_FIXED_ARTIFACT_ORACLE_HEADER_BYTES_V1)
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
    let tree_offset = rows_offset
        .checked_add(row_bytes)
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let root_offset = tree_offset
        .checked_add(
            tree_bytes
                .checked_sub(32)
                .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?,
        )
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let mut stored_root = [0_u8; 32];
    read_exact_artifact_at_v1(&mut file, root_offset, &mut stored_root)?;
    if stored_root != binding.profile.root {
        return Err(ZkX509PreprocessedFixedErrorV1::Artifact);
    }
    Ok(ZkX509PreprocessedFixedOracleArtifactV1 {
        file,
        profile: binding.profile,
        header,
        rows_offset,
        tree_offset,
        file_bytes: expected_file_bytes,
    })
}

impl ZkX509MainPreprocessedFixedArtifactV1 {
    /// Open the sole release package against independently pinned
    /// certificates and an externally supplied compiled-profile digest.
    ///
    /// The digest must come from the verifier-owned compiled profile. It is
    /// an input instead of being recomputed here to avoid a profile/artifact
    /// construction cycle.
    pub(crate) fn open_release_v1(
        package: &Path,
        compiled_profile_digest: [u8; 32],
    ) -> Result<Self, ZkX509PreprocessedFixedErrorV1> {
        let sha = pinned_zk_x509_sha_preprocessed_fixed_certificate_v1()?;
        let p256 = pinned_zk_x509_p256_log19_preprocessed_fixed_certificate_v1()?;
        let bindings = [
            ZkX509PreprocessedFixedArtifactBindingV1 {
                profile: sha.profile,
                descriptor_digest: sha.descriptor_digest,
                certificate: sha.encode_v1()?.to_vec(),
            },
            ZkX509PreprocessedFixedArtifactBindingV1 {
                profile: p256.profile,
                descriptor_digest: p256.descriptor_digest,
                certificate: p256.encode_v1()?.to_vec(),
            },
        ];
        Self::open_bound_v1(package, compiled_profile_digest, bindings)
    }

    fn open_bound_v1(
        package: &Path,
        compiled_profile_digest: [u8; 32],
        bindings: [ZkX509PreprocessedFixedArtifactBindingV1;
            ZK_X509_PREPROCESSED_FIXED_ORACLE_COUNT_V1],
    ) -> Result<Self, ZkX509PreprocessedFixedErrorV1> {
        if compiled_profile_digest == [0; 32]
            || !package
                .metadata()
                .map_err(|_| ZkX509PreprocessedFixedErrorV1::Artifact)?
                .is_dir()
        {
            return Err(ZkX509PreprocessedFixedErrorV1::Artifact);
        }
        let profiles = [bindings[0].profile, bindings[1].profile];
        validate_profiles_v1(&profiles)?;
        let manifest = read_exact_artifact_manifest_v1(package)?;
        if manifest[..4] != ZK_X509_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_MAGIC_V1
            || artifact_u16_v1(&manifest, 4)? != ZK_X509_PREPROCESSED_FIXED_ARTIFACT_VERSION_V1
            || artifact_u16_v1(&manifest, 6)?
                != u16::try_from(ZK_X509_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_BYTES_V1)
                    .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?
            || artifact_u16_v1(&manifest, 8)?
                != ZK_X509_PREPROCESSED_FIXED_ARTIFACT_LAYOUT_BATCH8_V1
            || artifact_u16_v1(&manifest, 10)?
                != u16::try_from(ZK_X509_PREPROCESSED_FIXED_ORACLE_COUNT_V1)
                    .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?
            || manifest[12..16].iter().any(|byte| *byte != 0)
            || artifact_array32_v1(&manifest, 16)? != compiled_profile_digest
            || manifest[368..].iter().any(|byte| *byte != 0)
        {
            return Err(ZkX509PreprocessedFixedErrorV1::Artifact);
        }
        let mut oracles = Vec::new();
        oracles
            .try_reserve_exact(ZK_X509_PREPROCESSED_FIXED_ORACLE_COUNT_V1)
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        for (index, binding) in bindings.iter().enumerate() {
            let expected_file_bytes =
                validate_artifact_manifest_entry_v1(&manifest, index, binding)?;
            oracles.push(open_artifact_oracle_v1(
                package,
                compiled_profile_digest,
                binding,
                expected_file_bytes,
            )?);
        }
        let oracles = oracles
            .try_into()
            .map_err(|_: Vec<_>| ZkX509PreprocessedFixedErrorV1::Artifact)?;
        Ok(Self {
            compiled_profile_digest,
            profiles,
            oracles,
        })
    }
}

impl ZkX509PreprocessedFixedOracleArtifactV1 {
    fn validate_open_file_v1(&self) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
        let metadata = self
            .file
            .metadata()
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Artifact)?;
        if !metadata.is_file() || metadata.len() != self.file_bytes {
            return Err(ZkX509PreprocessedFixedErrorV1::Artifact);
        }
        let mut file = self
            .file
            .try_clone()
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Artifact)?;
        let mut header = [0_u8; ZK_X509_PREPROCESSED_FIXED_ARTIFACT_ORACLE_HEADER_BYTES_V1];
        read_exact_artifact_at_v1(&mut file, 0, &mut header)?;
        let (_, tree_bytes, _) = artifact_geometry_lengths_v1(self.profile.geometry)?;
        let root_offset = self
            .tree_offset
            .checked_add(
                tree_bytes
                    .checked_sub(32)
                    .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?,
            )
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        let mut root = [0_u8; 32];
        read_exact_artifact_at_v1(&mut file, root_offset, &mut root)?;
        if header != self.header || root != self.profile.root {
            return Err(ZkX509PreprocessedFixedErrorV1::Artifact);
        }
        Ok(())
    }

    fn read_row_v1(&mut self, index: usize) -> Result<Vec<u64>, ZkX509PreprocessedFixedErrorV1> {
        let geometry = self.profile.geometry;
        let lde_rows = geometry.lde_rows()?;
        if index >= lde_rows {
            return Err(ZkX509PreprocessedFixedErrorV1::Index);
        }
        let mut row = Vec::new();
        row.try_reserve_exact(usize::from(geometry.width))
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        for column_start in
            (0..usize::from(geometry.width)).step_by(ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
        {
            let batch_width = usize::from(geometry.width)
                .checked_sub(column_start)
                .map(|remaining| remaining.min(ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1))
                .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
            let batch_offset = u64::try_from(column_start)
                .ok()
                .and_then(|column| {
                    u64::try_from(lde_rows)
                        .ok()
                        .and_then(|rows| column.checked_mul(rows))
                })
                .and_then(|fields| fields.checked_mul(8))
                .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
            let row_offset = u64::try_from(index)
                .ok()
                .and_then(|row| {
                    u64::try_from(batch_width)
                        .ok()
                        .and_then(|width| row.checked_mul(width))
                })
                .and_then(|fields| fields.checked_mul(8))
                .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
            let offset = self
                .rows_offset
                .checked_add(batch_offset)
                .and_then(|offset| offset.checked_add(row_offset))
                .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
            let mut encoded = [0_u8; ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1 * 8];
            read_exact_artifact_at_v1(&mut self.file, offset, &mut encoded[..batch_width * 8])?;
            for field in encoded[..batch_width * 8].chunks_exact(8) {
                let value = u64::from_be_bytes(
                    field
                        .try_into()
                        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Artifact)?,
                );
                if F::canonical(value).is_none() {
                    row.fill(0);
                    return Err(ZkX509PreprocessedFixedErrorV1::Artifact);
                }
                row.push(value);
            }
        }
        if row.len() != usize::from(geometry.width) {
            row.fill(0);
            return Err(ZkX509PreprocessedFixedErrorV1::Artifact);
        }
        Ok(row)
    }

    fn tree_level_offset_v1(&self, level: usize) -> Result<u64, ZkX509PreprocessedFixedErrorV1> {
        let height = usize::from(self.profile.geometry.lde_log2);
        if level > height {
            return Err(ZkX509PreprocessedFixedErrorV1::Index);
        }
        let mut nodes_before = 0_u64;
        let mut level_nodes = u64::try_from(self.profile.geometry.lde_rows()?)
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        for _ in 0..level {
            nodes_before = nodes_before
                .checked_add(level_nodes)
                .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
            level_nodes >>= 1;
        }
        self.tree_offset
            .checked_add(
                nodes_before
                    .checked_mul(32)
                    .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?,
            )
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)
    }

    fn read_tree_node_v1(
        &mut self,
        level: usize,
        index: usize,
    ) -> Result<[u8; 32], ZkX509PreprocessedFixedErrorV1> {
        let level_nodes = self
            .profile
            .geometry
            .lde_rows()?
            .checked_shr(
                u32::try_from(level).map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?,
            )
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        if index >= level_nodes {
            return Err(ZkX509PreprocessedFixedErrorV1::Index);
        }
        let offset = self
            .tree_level_offset_v1(level)?
            .checked_add(
                u64::try_from(index)
                    .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?
                    .checked_mul(32)
                    .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?,
            )
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        let mut node = [0_u8; 32];
        read_exact_artifact_at_v1(&mut self.file, offset, &mut node)?;
        Ok(node)
    }

    fn read_multiproof_v1(
        &mut self,
        indices: &[usize],
    ) -> Result<ZkX509PreprocessedFixedMultiproofV1, ZkX509PreprocessedFixedErrorV1> {
        self.validate_open_file_v1()?;
        canonical_indices_v1(self.profile.geometry, indices)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(indices.len())
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        for index in indices.iter().copied() {
            rows.push(self.read_row_v1(index)?);
        }
        let mut selected = indices.iter().copied().collect::<BTreeSet<_>>();
        let expected_frontier =
            multiproof_frontier_len_v1(self.profile.geometry.lde_rows()?, indices)
                .map_err(map_aggregate_error_v1)?;
        let mut frontier = Vec::new();
        frontier
            .try_reserve_exact(expected_frontier)
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        for level in 0..usize::from(self.profile.geometry.lde_log2) {
            for index in selected.iter().copied() {
                let sibling = index ^ 1;
                if !selected.contains(&sibling) {
                    frontier.push(self.read_tree_node_v1(level, sibling)?);
                }
            }
            selected = selected.into_iter().map(|index| index >> 1).collect();
        }
        if selected.len() != 1 || !selected.contains(&0) || frontier.len() != expected_frontier {
            return Err(ZkX509PreprocessedFixedErrorV1::Artifact);
        }
        Ok(ZkX509PreprocessedFixedMultiproofV1 {
            oracle: self.profile.geometry.oracle,
            indices: indices
                .iter()
                .copied()
                .map(|index| {
                    u32::try_from(index).map_err(|_| ZkX509PreprocessedFixedErrorV1::Index)
                })
                .collect::<Result<_, _>>()?,
            rows,
            frontier,
        })
    }
}

impl ZkX509MainPreprocessedFixedOpeningServiceV1 for ZkX509MainPreprocessedFixedArtifactV1 {
    fn open_main_v1(
        &mut self,
        profiles: &[ZkX509PreprocessedFixedProfileV1; ZK_X509_PREPROCESSED_FIXED_ORACLE_COUNT_V1],
        indices: &ZkX509Log19PreprocessedFixedOpeningIndicesV1,
    ) -> Result<Vec<u8>, ZkX509PreprocessedFixedErrorV1> {
        if self.compiled_profile_digest == [0; 32] || profiles != &self.profiles {
            return Err(ZkX509PreprocessedFixedErrorV1::Profile);
        }
        let mut multiproofs = Vec::new();
        multiproofs
            .try_reserve_exact(ZK_X509_PREPROCESSED_FIXED_ORACLE_COUNT_V1)
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        for oracle in &mut self.oracles {
            let proof = oracle.read_multiproof_v1(indices.as_slice_v1())?;
            verify_zk_x509_preprocessed_fixed_multiproof_v1(
                oracle.profile,
                indices.as_slice_v1(),
                &proof,
            )?;
            multiproofs.push(proof);
        }
        encode_zk_x509_preprocessed_fixed_proof_v1(
            profiles,
            &ZkX509PreprocessedFixedProofV1 {
                oracles: multiproofs,
            },
        )
    }
}

/// Exact encoded length of the compiled SHA fixed-oracle certificate.
pub(crate) const ZK_X509_SHA_PREPROCESSED_FIXED_CERTIFICATE_BYTES_V1: usize = 81;
/// Exact encoded length of the compiled P-256 log19 fixed-oracle certificate.
pub(crate) const ZK_X509_P256_LOG19_PREPROCESSED_FIXED_CERTIFICATE_BYTES_V1: usize = 83;
const SHA_FIXED_CERTIFICATE_MAGIC_V1: [u8; 4] = *b"X5FC";
const SHA_FIXED_CERTIFICATE_VERSION_V1: u16 = 1;
const P256_LOG19_FIXED_CERTIFICATE_MAGIC_V1: [u8; 4] = *b"X5PC";
const P256_LOG19_FIXED_CERTIFICATE_VERSION_V1: u16 = 1;

// This is intentionally absent until the serialized diagnostic root
// derivation and an independent recomputation agree. No placeholder root is
// accepted or committed by the provisional profile.
const ZK_X509_SHA_PREPROCESSED_FIXED_PINNED_ROOT_V1: Option<[u8; 32]> = None;
// Populated only from the deterministic release-root ceremony after the
// complete P-256 log19 manifest is frozen and independently reproduced.
const ZK_X509_P256_LOG19_PREPROCESSED_FIXED_PINNED_ROOT_V1: Option<[u8; 32]> = None;

/// Hash the complete generic protocol and exact SHA column-manifest
/// descriptors.
pub(crate) fn zk_x509_sha_preprocessed_fixed_descriptor_digest_v1() -> [u8; 32] {
    sha256_frame_v1(
        SHA_FIXED_DESCRIPTOR_DIGEST_DOMAIN_V1,
        &[
            ZK_X509_PREPROCESSED_FIXED_DESCRIPTOR_V1,
            ZK_X509_SHA_PREPROCESSED_FIXED_COLUMN_DESCRIPTOR_V1,
        ],
    )
    .expect("static zk-X509 fixed preprocessing descriptors are representable")
}

/// Hash the generic protocol and exact P-256 log19 column manifest.
pub(crate) fn zk_x509_p256_log19_preprocessed_fixed_descriptor_digest_v1() -> [u8; 32] {
    sha256_frame_v1(
        P256_LOG19_FIXED_DESCRIPTOR_DIGEST_DOMAIN_V1,
        &[
            ZK_X509_PREPROCESSED_FIXED_DESCRIPTOR_V1,
            ZK_X509_P256_LOG19_PREPROCESSED_FIXED_COLUMN_DESCRIPTOR_V1,
        ],
    )
    .expect("static zk-X509 P-256 fixed preprocessing descriptors are representable")
}

impl ZkX509ShaPreprocessedFixedCertificateV1 {
    /// Construct a candidate certificate from a genuinely derived root.
    pub(crate) fn from_derived_root_v1(
        root: [u8; 32],
    ) -> Result<Self, ZkX509PreprocessedFixedErrorV1> {
        let certificate = Self {
            profile: ZkX509PreprocessedFixedProfileV1 {
                geometry: ZK_X509_SHA_PREPROCESSED_FIXED_GEOMETRY_V1,
                root,
            },
            segment_order: ZK_X509_SHA_PREPROCESSED_FIXED_SEGMENT_ORDER_V1,
            descriptor_digest: zk_x509_sha_preprocessed_fixed_descriptor_digest_v1(),
        };
        certificate.validate_v1()?;
        Ok(certificate)
    }

    fn validate_v1(self) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
        self.profile.validate()?;
        if self.profile.geometry != ZK_X509_SHA_PREPROCESSED_FIXED_GEOMETRY_V1
            || self.segment_order != ZK_X509_SHA_PREPROCESSED_FIXED_SEGMENT_ORDER_V1
            || self.descriptor_digest != zk_x509_sha_preprocessed_fixed_descriptor_digest_v1()
        {
            return Err(ZkX509PreprocessedFixedErrorV1::Profile);
        }
        Ok(())
    }

    /// Encode the sole certificate field committed by the compiled profile.
    pub(crate) fn encode_v1(
        self,
    ) -> Result<
        [u8; ZK_X509_SHA_PREPROCESSED_FIXED_CERTIFICATE_BYTES_V1],
        ZkX509PreprocessedFixedErrorV1,
    > {
        self.validate_v1()?;
        let mut encoded = [0_u8; ZK_X509_SHA_PREPROCESSED_FIXED_CERTIFICATE_BYTES_V1];
        encoded[..4].copy_from_slice(&SHA_FIXED_CERTIFICATE_MAGIC_V1);
        encoded[4..6].copy_from_slice(&SHA_FIXED_CERTIFICATE_VERSION_V1.to_be_bytes());
        encoded[6..8].copy_from_slice(&self.profile.geometry.oracle.to_be_bytes());
        encoded[8] = self.profile.geometry.native_log2;
        encoded[9] = self.profile.geometry.lde_log2;
        encoded[10..12].copy_from_slice(&self.profile.geometry.width.to_be_bytes());
        encoded[12] = u8::try_from(self.segment_order.len())
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Profile)?;
        encoded[13..17].copy_from_slice(&self.segment_order);
        encoded[17..49].copy_from_slice(&self.descriptor_digest);
        encoded[49..81].copy_from_slice(&self.profile.root);
        Ok(encoded)
    }
}

impl ZkX509P256Log19PreprocessedFixedCertificateV1 {
    /// Construct a candidate certificate from a genuinely derived root.
    pub(crate) fn from_derived_root_v1(
        root: [u8; 32],
    ) -> Result<Self, ZkX509PreprocessedFixedErrorV1> {
        let certificate = Self {
            profile: ZkX509PreprocessedFixedProfileV1 {
                geometry: ZK_X509_P256_LOG19_PREPROCESSED_FIXED_GEOMETRY_V1,
                root,
            },
            schedule_order: ZK_X509_P256_LOG19_PREPROCESSED_FIXED_SCHEDULE_ORDER_V1,
            descriptor_digest: zk_x509_p256_log19_preprocessed_fixed_descriptor_digest_v1(),
        };
        certificate.validate_v1()?;
        Ok(certificate)
    }

    fn validate_v1(self) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
        self.profile.validate()?;
        if self.profile.geometry != ZK_X509_P256_LOG19_PREPROCESSED_FIXED_GEOMETRY_V1
            || self.schedule_order != ZK_X509_P256_LOG19_PREPROCESSED_FIXED_SCHEDULE_ORDER_V1
            || self.descriptor_digest
                != zk_x509_p256_log19_preprocessed_fixed_descriptor_digest_v1()
        {
            return Err(ZkX509PreprocessedFixedErrorV1::Profile);
        }
        Ok(())
    }

    /// Encode every certificate field committed by the compiled profile.
    pub(crate) fn encode_v1(
        self,
    ) -> Result<
        [u8; ZK_X509_P256_LOG19_PREPROCESSED_FIXED_CERTIFICATE_BYTES_V1],
        ZkX509PreprocessedFixedErrorV1,
    > {
        self.validate_v1()?;
        let mut encoded = [0_u8; ZK_X509_P256_LOG19_PREPROCESSED_FIXED_CERTIFICATE_BYTES_V1];
        encoded[..4].copy_from_slice(&P256_LOG19_FIXED_CERTIFICATE_MAGIC_V1);
        encoded[4..6].copy_from_slice(&P256_LOG19_FIXED_CERTIFICATE_VERSION_V1.to_be_bytes());
        encoded[6..8].copy_from_slice(&self.profile.geometry.oracle.to_be_bytes());
        encoded[8] = self.profile.geometry.native_log2;
        encoded[9] = self.profile.geometry.lde_log2;
        encoded[10..12].copy_from_slice(&self.profile.geometry.width.to_be_bytes());
        encoded[12] = u8::try_from(self.schedule_order.len())
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Profile)?;
        encoded[13..19].copy_from_slice(&self.schedule_order);
        encoded[19..51].copy_from_slice(&self.descriptor_digest);
        encoded[51..83].copy_from_slice(&self.profile.root);
        Ok(encoded)
    }
}

/// Return the sole independently checked release certificate.
///
/// Until the real derived root has been independently reproduced and pinned,
/// every MAIN prover/verifier constructor calling this function fails closed.
pub(crate) fn pinned_zk_x509_sha_preprocessed_fixed_certificate_v1()
-> Result<ZkX509ShaPreprocessedFixedCertificateV1, ZkX509PreprocessedFixedErrorV1> {
    let root = ZK_X509_SHA_PREPROCESSED_FIXED_PINNED_ROOT_V1
        .ok_or(ZkX509PreprocessedFixedErrorV1::Unpinned)?;
    ZkX509ShaPreprocessedFixedCertificateV1::from_derived_root_v1(root)
}

/// Return the independently checked P-256 log19 release certificate.
pub(crate) fn pinned_zk_x509_p256_log19_preprocessed_fixed_certificate_v1()
-> Result<ZkX509P256Log19PreprocessedFixedCertificateV1, ZkX509PreprocessedFixedErrorV1> {
    let root = ZK_X509_P256_LOG19_PREPROCESSED_FIXED_PINNED_ROOT_V1
        .ok_or(ZkX509PreprocessedFixedErrorV1::Unpinned)?;
    ZkX509P256Log19PreprocessedFixedCertificateV1::from_derived_root_v1(root)
}

/// Require an exact match with a verifier-owned certificate.
pub(crate) fn validate_zk_x509_sha_preprocessed_fixed_certificate_v1(
    supplied: ZkX509ShaPreprocessedFixedCertificateV1,
    expected: ZkX509ShaPreprocessedFixedCertificateV1,
) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
    supplied.validate_v1()?;
    expected.validate_v1()?;
    if supplied.profile.geometry != expected.profile.geometry
        || supplied.segment_order != expected.segment_order
        || supplied.descriptor_digest != expected.descriptor_digest
    {
        return Err(ZkX509PreprocessedFixedErrorV1::Profile);
    }
    if supplied.profile.root != expected.profile.root {
        return Err(ZkX509PreprocessedFixedErrorV1::RootMismatch);
    }
    Ok(())
}

/// Require an exact match with the verifier-owned P-256 log19 certificate.
pub(crate) fn validate_zk_x509_p256_log19_preprocessed_fixed_certificate_v1(
    supplied: ZkX509P256Log19PreprocessedFixedCertificateV1,
    expected: ZkX509P256Log19PreprocessedFixedCertificateV1,
) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
    supplied.validate_v1()?;
    expected.validate_v1()?;
    if supplied.profile.geometry != expected.profile.geometry
        || supplied.schedule_order != expected.schedule_order
        || supplied.descriptor_digest != expected.descriptor_digest
    {
        return Err(ZkX509PreprocessedFixedErrorV1::Profile);
    }
    if supplied.profile.root != expected.profile.root {
        return Err(ZkX509PreprocessedFixedErrorV1::RootMismatch);
    }
    Ok(())
}

/// Return the exact oracle profile order committed by MAIN and X5F1.
pub(crate) fn zk_x509_main_preprocessed_fixed_profiles_v1(
    sha: ZkX509ShaPreprocessedFixedCertificateV1,
    p256: ZkX509P256Log19PreprocessedFixedCertificateV1,
) -> Result<
    [ZkX509PreprocessedFixedProfileV1; ZK_X509_PREPROCESSED_FIXED_ORACLE_COUNT_V1],
    ZkX509PreprocessedFixedErrorV1,
> {
    sha.validate_v1()?;
    p256.validate_v1()?;
    let profiles = [sha.profile, p256.profile];
    validate_profiles_v1(&profiles)?;
    Ok(profiles)
}

fn map_transparent_error_v1(_: TransparentStarkErrorV1) -> ZkX509PreprocessedFixedErrorV1 {
    ZkX509PreprocessedFixedErrorV1::Resource
}

fn map_sha_error_v1(error: ZkX509ShaCallBusStarkErrorV1) -> ZkX509PreprocessedFixedErrorV1 {
    match error {
        ZkX509ShaCallBusStarkErrorV1::Resource => ZkX509PreprocessedFixedErrorV1::Resource,
        _ => ZkX509PreprocessedFixedErrorV1::Column,
    }
}

fn map_p256_error_v1(
    error: super::p256_aggregate_adapter::P256AggregateAdapterErrorV1,
) -> ZkX509PreprocessedFixedErrorV1 {
    match error {
        super::p256_aggregate_adapter::P256AggregateAdapterErrorV1::Resource => {
            ZkX509PreprocessedFixedErrorV1::Resource
        }
        _ => ZkX509PreprocessedFixedErrorV1::Column,
    }
}

fn map_reader_error_v1(_: TransparentStarkErrorV1) -> ZkX509PreprocessedFixedErrorV1 {
    ZkX509PreprocessedFixedErrorV1::MalformedProof
}

fn map_aggregate_error_v1(error: AggregateStarkErrorV1) -> ZkX509PreprocessedFixedErrorV1 {
    match error {
        AggregateStarkErrorV1::AllocationFailure => ZkX509PreprocessedFixedErrorV1::Resource,
        _ => ZkX509PreprocessedFixedErrorV1::Opening,
    }
}

fn validate_profiles_v1(
    profiles: &[ZkX509PreprocessedFixedProfileV1],
) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
    if profiles.is_empty() || profiles.len() > ZK_X509_PREPROCESSED_FIXED_ORACLE_COUNT_V1 {
        return Err(ZkX509PreprocessedFixedErrorV1::Profile);
    }
    let mut previous = None;
    for profile in profiles.iter().copied() {
        profile.validate()?;
        if previous.is_some_and(|oracle| oracle >= profile.geometry.oracle) {
            return Err(ZkX509PreprocessedFixedErrorV1::Profile);
        }
        previous = Some(profile.geometry.oracle);
    }
    Ok(())
}

fn canonical_indices_v1(
    geometry: ZkX509PreprocessedFixedGeometryV1,
    indices: &[usize],
) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
    let lde_rows = geometry.lde_rows()?;
    if indices.is_empty()
        || indices.len() > ZK_X509_PREPROCESSED_FIXED_MAX_OPENINGS_V1
        || indices.iter().any(|index| *index >= lde_rows)
        || indices.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(ZkX509PreprocessedFixedErrorV1::Index);
    }
    Ok(())
}

/// Validate the 58 distinct canonical MAIN query coordinates and derive the
/// sole sorted current/next opening set shared by both native-log19 fixed
/// oracles on the log25 common LDE domain.
///
/// The query order is transcript order and is therefore not required to be
/// sorted. Query sampling/authentication remains the responsibility of MAIN
/// transcript assembly; no proof-controlled stride or expanded index list
/// crosses this API.
pub(crate) fn derive_zk_x509_log19_preprocessed_fixed_opening_indices_v1(
    query_coordinates: &[usize],
) -> Result<ZkX509Log19PreprocessedFixedOpeningIndicesV1, ZkX509PreprocessedFixedErrorV1> {
    let geometry = ZK_X509_SHA_PREPROCESSED_FIXED_GEOMETRY_V1;
    if geometry.native_log2 != ZK_X509_P256_LOG19_PREPROCESSED_FIXED_GEOMETRY_V1.native_log2
        || geometry.lde_log2 != ZK_X509_P256_LOG19_PREPROCESSED_FIXED_GEOMETRY_V1.lde_log2
    {
        return Err(ZkX509PreprocessedFixedErrorV1::Profile);
    }
    let lde_rows = geometry.lde_rows()?;
    if query_coordinates.len() != usize::from(ZK_X509_FRI_QUERY_COUNT_V1) {
        return Err(ZkX509PreprocessedFixedErrorV1::Index);
    }
    let next_stride = 1_usize
        .checked_shl(u32::from(
            geometry
                .lde_log2
                .checked_sub(geometry.native_log2)
                .ok_or(ZkX509PreprocessedFixedErrorV1::Profile)?,
        ))
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let mut distinct_queries = BTreeSet::new();
    let mut indices = BTreeSet::new();
    for current in query_coordinates.iter().copied() {
        if current >= lde_rows || !distinct_queries.insert(current) {
            return Err(ZkX509PreprocessedFixedErrorV1::Index);
        }
        let next = current
            .checked_add(next_stride)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?
            % lde_rows;
        indices.insert(current);
        indices.insert(next);
    }
    let indices = indices.into_iter().collect::<Vec<_>>();
    canonical_indices_v1(geometry, &indices)?;
    Ok(ZkX509Log19PreprocessedFixedOpeningIndicesV1 { indices })
}

fn one_oracle_maximum_encoded_bytes_v1(
    geometry: ZkX509PreprocessedFixedGeometryV1,
) -> Result<usize, ZkX509PreprocessedFixedErrorV1> {
    let frontier_hashes = maximum_multiproof_frontier_len_v1(
        geometry.lde_rows()?,
        ZK_X509_PREPROCESSED_FIXED_MAX_OPENINGS_V1,
    )
    .map_err(map_aggregate_error_v1)?;
    let opening_bytes = usize::from(geometry.width)
        .checked_mul(8)
        .and_then(|bytes| bytes.checked_add(4))
        .and_then(|bytes| bytes.checked_mul(ZK_X509_PREPROCESSED_FIXED_MAX_OPENINGS_V1))
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let encoded_bytes = frontier_hashes
        .checked_mul(32)
        .and_then(|bytes| bytes.checked_add(opening_bytes))
        .and_then(|bytes| bytes.checked_add(8 + 4))
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    if frontier_hashes != ZK_X509_SHA_PREPROCESSED_FIXED_MAXIMUM_FRONTIER_HASHES_V1 {
        return Err(ZkX509PreprocessedFixedErrorV1::Profile);
    }
    Ok(encoded_bytes)
}

/// Recompute the exact worst-case SHA one-oracle X5F1 byte bound.
pub(crate) fn zk_x509_sha_preprocessed_fixed_maximum_encoded_bytes_v1()
-> Result<usize, ZkX509PreprocessedFixedErrorV1> {
    let encoded = one_oracle_maximum_encoded_bytes_v1(ZK_X509_SHA_PREPROCESSED_FIXED_GEOMETRY_V1)?;
    if encoded != ZK_X509_SHA_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1 {
        return Err(ZkX509PreprocessedFixedErrorV1::Profile);
    }
    Ok(encoded)
}

/// Recompute the exact worst-case P-256 log19 one-oracle X5F1 byte bound.
pub(crate) fn zk_x509_p256_log19_preprocessed_fixed_maximum_encoded_bytes_v1()
-> Result<usize, ZkX509PreprocessedFixedErrorV1> {
    let encoded =
        one_oracle_maximum_encoded_bytes_v1(ZK_X509_P256_LOG19_PREPROCESSED_FIXED_GEOMETRY_V1)?;
    if encoded != ZK_X509_P256_LOG19_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1 {
        return Err(ZkX509PreprocessedFixedErrorV1::Profile);
    }
    Ok(encoded)
}

/// Recompute the exact complete two-oracle MAIN X5F1 maximum.
pub(crate) fn zk_x509_main_preprocessed_fixed_maximum_encoded_bytes_v1()
-> Result<usize, ZkX509PreprocessedFixedErrorV1> {
    let encoded = zk_x509_sha_preprocessed_fixed_maximum_encoded_bytes_v1()?
        .checked_add(zk_x509_p256_log19_preprocessed_fixed_maximum_encoded_bytes_v1()?)
        .and_then(|bytes| bytes.checked_sub(8))
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    if encoded != ZK_X509_MAIN_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1
        || encoded != ZK_X509_PREPROCESSED_FIXED_MAX_WIRE_BYTES_V1
    {
        return Err(ZkX509PreprocessedFixedErrorV1::Profile);
    }
    Ok(encoded)
}

fn reduce_sha_preprocessed_fixed_segment_row_v1(
    full: &[F; ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
) -> Result<
    [F; ZK_X509_SHA_PREPROCESSED_FIXED_COLUMNS_PER_SEGMENT_V1],
    ZkX509PreprocessedFixedErrorV1,
> {
    let word: &[F; SHA_WORD_CAPACITY_FIXED_WIDTH_V1] = full[..SHA_WORD_CAPACITY_FIXED_WIDTH_V1]
        .try_into()
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Column)?;
    let word = reduce_zk_x509_sha_word_fixed_row_v1(word)
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Column)?;
    let mut reduced = [F::ZERO; ZK_X509_SHA_PREPROCESSED_FIXED_COLUMNS_PER_SEGMENT_V1];
    reduced[..ZK_X509_SHA_WORD_PREPROCESSED_FIXED_WIDTH_V1].copy_from_slice(&word);
    for (target, source) in reduced[ZK_X509_SHA_WORD_PREPROCESSED_FIXED_WIDTH_V1
        ..ZK_X509_SHA_WORD_PREPROCESSED_FIXED_WIDTH_V1 + 6]
        .iter_mut()
        .zip([
            ZK_X509_SHA_FIXED_CALL_V1,
            ZK_X509_SHA_FIXED_ROLE_V1,
            ZK_X509_SHA_FIXED_SLOT_V1,
            ZK_X509_SHA_FIXED_SEGMENT_FIRST_V1,
            ZK_X509_SHA_FIXED_SEGMENT_LAST_V1,
            ZK_X509_SHA_FIXED_PHYSICAL_PADDING_V1,
        ])
    {
        *target = full[source];
    }
    let selector_target = ZK_X509_SHA_WORD_PREPROCESSED_FIXED_WIDTH_V1 + 6;
    reduced[selector_target..selector_target + ZK_X509_SHA_CA_CALL_COUNT_V1].copy_from_slice(
        &full[ZK_X509_SHA_FIXED_CA_CALL_SELECTORS_V1
            ..ZK_X509_SHA_FIXED_CA_CALL_SELECTORS_V1 + ZK_X509_SHA_CA_CALL_COUNT_V1],
    );
    Ok(reduced)
}

/// Expand one authenticated reduced fixed-oracle row into four SHA fixed rows.
///
/// The profile-independent word, call, role, boundary, and compact-CA columns
/// are reconstructed exactly. Statement-derived RFC length/event columns are
/// deliberately left at canonical zero; the MAIN verifier must overlay those
/// columns from its own public schedule before evaluating the AIR.
pub(crate) fn expand_zk_x509_sha_preprocessed_fixed_row_v1(
    reduced: &[F],
) -> Result<
    [F; ZK_X509_SHA_SEGMENT_COUNT_V1 * ZK_X509_SHA_BATCH_FIXED_WIDTH_V1],
    ZkX509PreprocessedFixedErrorV1,
> {
    if reduced.len() != ZK_X509_SHA_PREPROCESSED_FIXED_WIDTH_V1 {
        return Err(ZkX509PreprocessedFixedErrorV1::Opening);
    }
    let mut expanded = [F::ZERO; ZK_X509_SHA_SEGMENT_COUNT_V1 * ZK_X509_SHA_BATCH_FIXED_WIDTH_V1];
    for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
        let reduced_start = segment
            .checked_mul(ZK_X509_SHA_PREPROCESSED_FIXED_COLUMNS_PER_SEGMENT_V1)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        let reduced_end = reduced_start
            .checked_add(ZK_X509_SHA_PREPROCESSED_FIXED_COLUMNS_PER_SEGMENT_V1)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        let segment_row = reduced
            .get(reduced_start..reduced_end)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Opening)?;
        let word: &[F; ZK_X509_SHA_WORD_PREPROCESSED_FIXED_WIDTH_V1] = segment_row
            [..ZK_X509_SHA_WORD_PREPROCESSED_FIXED_WIDTH_V1]
            .try_into()
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Opening)?;
        let word = expand_zk_x509_sha_word_fixed_row_v1(word);
        let expanded_start = segment
            .checked_mul(ZK_X509_SHA_BATCH_FIXED_WIDTH_V1)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        let expanded_end = expanded_start
            .checked_add(ZK_X509_SHA_BATCH_FIXED_WIDTH_V1)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        let target = expanded
            .get_mut(expanded_start..expanded_end)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Opening)?;
        target[..SHA_WORD_CAPACITY_FIXED_WIDTH_V1].copy_from_slice(&word);
        for (target, value) in target[SHA_WORD_CAPACITY_FIXED_WIDTH_V1..].iter_mut().zip(
            segment_row[ZK_X509_SHA_WORD_PREPROCESSED_FIXED_WIDTH_V1..]
                .iter()
                .copied(),
        ) {
            *target = value;
        }
    }
    Ok(expanded)
}

/// Sequential batch-8 provider for the exact 340 independent SHA columns.
///
/// One fixed row is reconstructed per touched physical segment and row, then
/// transposed into at most eight native columns. This preserves the release
/// memory envelope without recomputing the same fixed row once per column.
struct ZkX509ShaPreprocessedNativeColumnProviderV1 {
    fixed: ZkX509ShaBatchFixedProviderV1,
    next_column: usize,
    batch: Vec<Vec<F>>,
}

impl ZkX509ShaPreprocessedNativeColumnProviderV1 {
    fn new_v1() -> Result<Self, ZkX509PreprocessedFixedErrorV1> {
        Ok(Self {
            fixed: ZkX509ShaBatchFixedProviderV1::new_v1(ZkX509ShaCallPublicShapeV1 {
                disclosed_attributes: 0,
            })
            .map_err(map_sha_error_v1)?,
            next_column: 0,
            batch: Vec::new(),
        })
    }

    fn refill_v1(&mut self, column_start: usize) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
        if !self.batch.is_empty()
            || column_start != self.next_column
            || column_start >= ZK_X509_SHA_PREPROCESSED_FIXED_WIDTH_V1
        {
            return Err(ZkX509PreprocessedFixedErrorV1::Column);
        }
        let column_end = column_start
            .checked_add(ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
            .map(|end| end.min(ZK_X509_SHA_PREPROCESSED_FIXED_WIDTH_V1))
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        self.batch
            .try_reserve_exact(column_end - column_start)
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        for _ in column_start..column_end {
            let mut column = Vec::new();
            column
                .try_reserve_exact(ZK_X509_SHA_SEGMENT_ROWS_V1)
                .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
            self.batch.push(column);
        }

        let first_segment = column_start / ZK_X509_SHA_PREPROCESSED_FIXED_COLUMNS_PER_SEGMENT_V1;
        let last_segment = (column_end - 1) / ZK_X509_SHA_PREPROCESSED_FIXED_COLUMNS_PER_SEGMENT_V1;
        if last_segment >= ZK_X509_SHA_SEGMENT_COUNT_V1 || last_segment > first_segment + 1 {
            return Err(ZkX509PreprocessedFixedErrorV1::Column);
        }
        for row in 0..ZK_X509_SHA_SEGMENT_ROWS_V1 {
            let first = reduce_sha_preprocessed_fixed_segment_row_v1(
                &self
                    .fixed
                    .fixed_row_v1(first_segment, row)
                    .map_err(map_sha_error_v1)?,
            )?;
            let second = if last_segment == first_segment {
                None
            } else {
                Some(reduce_sha_preprocessed_fixed_segment_row_v1(
                    &self
                        .fixed
                        .fixed_row_v1(last_segment, row)
                        .map_err(map_sha_error_v1)?,
                )?)
            };
            for (offset, global_column) in (column_start..column_end).enumerate() {
                let segment = global_column / ZK_X509_SHA_PREPROCESSED_FIXED_COLUMNS_PER_SEGMENT_V1;
                let local = global_column % ZK_X509_SHA_PREPROCESSED_FIXED_COLUMNS_PER_SEGMENT_V1;
                let fixed_row = if segment == first_segment {
                    &first
                } else {
                    second
                        .as_ref()
                        .ok_or(ZkX509PreprocessedFixedErrorV1::Column)?
                };
                self.batch[offset].push(fixed_row[local]);
            }
        }
        if self
            .batch
            .iter()
            .any(|column| column.len() != ZK_X509_SHA_SEGMENT_ROWS_V1)
        {
            return Err(ZkX509PreprocessedFixedErrorV1::Column);
        }
        Ok(())
    }

    fn native_column_v1(
        &mut self,
        column: usize,
    ) -> Result<Vec<F>, ZkX509PreprocessedFixedErrorV1> {
        if column != self.next_column || column >= ZK_X509_SHA_PREPROCESSED_FIXED_WIDTH_V1 {
            return Err(ZkX509PreprocessedFixedErrorV1::Column);
        }
        if self.batch.is_empty() {
            self.refill_v1(column)?;
        }
        let native = self.batch.remove(0);
        self.next_column = self
            .next_column
            .checked_add(1)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        Ok(native)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum P256Log19FixedScheduleV1 {
    CertificateArithmetic,
    WalletArithmetic,
    CertificateExecution,
    WalletExecution,
    CertificateSorted,
    WalletSorted,
}

impl P256Log19FixedScheduleV1 {
    const fn start_width_v1(self) -> (usize, usize) {
        match self {
            Self::CertificateArithmetic => (
                P256_LOG19_CERTIFICATE_ARITHMETIC_START_V1,
                P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1,
            ),
            Self::WalletArithmetic => (
                P256_LOG19_WALLET_ARITHMETIC_START_V1,
                P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1,
            ),
            Self::CertificateExecution => (
                P256_LOG19_CERTIFICATE_EXECUTION_START_V1,
                P256_VALUE_EXECUTION_AGGREGATE_FIXED_WIDTH_V1,
            ),
            Self::WalletExecution => (
                P256_LOG19_WALLET_EXECUTION_START_V1,
                P256_VALUE_EXECUTION_AGGREGATE_FIXED_WIDTH_V1,
            ),
            Self::CertificateSorted => (
                P256_LOG19_CERTIFICATE_SORTED_START_V1,
                P256_VALUE_BUS_STARK_FIXED_WIDTH_V1,
            ),
            Self::WalletSorted => (
                P256_LOG19_WALLET_SORTED_START_V1,
                P256_VALUE_BUS_STARK_FIXED_WIDTH_V1,
            ),
        }
    }

    fn representative_registration_v1(
        self,
    ) -> Result<P256MainRegistrationV1, ZkX509PreprocessedFixedErrorV1> {
        let certificate = 0;
        let wallet = P256_X5S1_SIGNATURES_V1 - 1;
        let (signature, adapter, local) = match self {
            Self::CertificateArithmetic => (certificate, P256MainAdapterV1::Arithmetic, 0),
            Self::WalletArithmetic => (wallet, P256MainAdapterV1::Arithmetic, 0),
            Self::CertificateExecution => (certificate, P256MainAdapterV1::ValueBus, 0),
            Self::WalletExecution => (wallet, P256MainAdapterV1::ValueBus, 0),
            Self::CertificateSorted => (certificate, P256MainAdapterV1::ValueBus, 1),
            Self::WalletSorted => (wallet, P256MainAdapterV1::ValueBus, 1),
        };
        P256MainRegistrationV1::new_v1(signature, adapter, local).map_err(map_p256_error_v1)
    }
}

fn p256_log19_fixed_schedule_for_registration_v1(
    registration: P256MainRegistrationV1,
) -> Result<P256Log19FixedScheduleV1, ZkX509PreprocessedFixedErrorV1> {
    let certificate = registration.role_v1() == P256EcdsaRoleV1::CertificateOrCrl;
    match (
        registration.adapter_v1(),
        registration.local_instance_v1(),
        certificate,
    ) {
        (P256MainAdapterV1::Arithmetic, 0, true) => {
            Ok(P256Log19FixedScheduleV1::CertificateArithmetic)
        }
        (P256MainAdapterV1::Arithmetic, 0, false) => Ok(P256Log19FixedScheduleV1::WalletArithmetic),
        (P256MainAdapterV1::ValueBus, 0, true) => {
            Ok(P256Log19FixedScheduleV1::CertificateExecution)
        }
        (P256MainAdapterV1::ValueBus, 0, false) => Ok(P256Log19FixedScheduleV1::WalletExecution),
        (P256MainAdapterV1::ValueBus, 1, true) => Ok(P256Log19FixedScheduleV1::CertificateSorted),
        (P256MainAdapterV1::ValueBus, 1, false) => Ok(P256Log19FixedScheduleV1::WalletSorted),
        _ => Err(ZkX509PreprocessedFixedErrorV1::Profile),
    }
}

fn p256_log19_fixed_schedule_for_column_v1(
    column: usize,
) -> Result<(P256Log19FixedScheduleV1, usize), ZkX509PreprocessedFixedErrorV1> {
    for schedule in [
        P256Log19FixedScheduleV1::CertificateArithmetic,
        P256Log19FixedScheduleV1::WalletArithmetic,
        P256Log19FixedScheduleV1::CertificateExecution,
        P256Log19FixedScheduleV1::WalletExecution,
        P256Log19FixedScheduleV1::CertificateSorted,
        P256Log19FixedScheduleV1::WalletSorted,
    ] {
        let (start, width) = schedule.start_width_v1();
        if (start..start + width).contains(&column) {
            return Ok((schedule, column - start));
        }
    }
    Err(ZkX509PreprocessedFixedErrorV1::Column)
}

/// Select one registration's exact fixed row from an authenticated oracle
/// opening. Only the six manifest schedules are accepted.
pub(crate) fn zk_x509_p256_log19_preprocessed_fixed_row_for_registration_v1<'a>(
    authenticated: &'a [F],
    registration: P256MainRegistrationV1,
) -> Result<&'a [F], ZkX509PreprocessedFixedErrorV1> {
    if authenticated.len() != ZK_X509_P256_LOG19_PREPROCESSED_FIXED_WIDTH_V1 {
        return Err(ZkX509PreprocessedFixedErrorV1::Opening);
    }
    let schedule = p256_log19_fixed_schedule_for_registration_v1(registration)?;
    let (start, width) = schedule.start_width_v1();
    let shape = registration.shape_v1().map_err(map_p256_error_v1)?;
    if shape.trace_size != 1_usize << ZK_X509_MAX_NATIVE_TRACE_LOG2_V1 || shape.fixed_width != width
    {
        return Err(ZkX509PreprocessedFixedErrorV1::Profile);
    }
    authenticated
        .get(start..start + width)
        .ok_or(ZkX509PreprocessedFixedErrorV1::Opening)
}

#[cfg(test)]
fn p256_log19_native_fixed_row_v1(
    fixed: &P256MainVerifierFixedSourceV1,
    row: usize,
) -> Result<Vec<F>, ZkX509PreprocessedFixedErrorV1> {
    let mut combined = Vec::new();
    combined
        .try_reserve_exact(ZK_X509_P256_LOG19_PREPROCESSED_FIXED_WIDTH_V1)
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
    for schedule in [
        P256Log19FixedScheduleV1::CertificateArithmetic,
        P256Log19FixedScheduleV1::WalletArithmetic,
        P256Log19FixedScheduleV1::CertificateExecution,
        P256Log19FixedScheduleV1::WalletExecution,
        P256Log19FixedScheduleV1::CertificateSorted,
        P256Log19FixedScheduleV1::WalletSorted,
    ] {
        let registration = schedule.representative_registration_v1()?;
        let expected_width = schedule.start_width_v1().1;
        let schedule_row = fixed
            .fixed_row_v1(registration, row)
            .map_err(map_p256_error_v1)?;
        if schedule_row.len() != expected_width {
            return Err(ZkX509PreprocessedFixedErrorV1::Column);
        }
        combined.extend_from_slice(&schedule_row);
    }
    if combined.len() != ZK_X509_P256_LOG19_PREPROCESSED_FIXED_WIDTH_V1 {
        return Err(ZkX509PreprocessedFixedErrorV1::Column);
    }
    Ok(combined)
}

/// Sequential batch-8 provider for the exact 404 P-256 log19 columns.
struct ZkX509P256Log19PreprocessedNativeColumnProviderV1 {
    fixed: P256MainVerifierFixedSourceV1,
    next_column: usize,
    batch: Vec<Vec<F>>,
}

impl ZkX509P256Log19PreprocessedNativeColumnProviderV1 {
    fn new_v1() -> Result<Self, ZkX509PreprocessedFixedErrorV1> {
        Ok(Self {
            fixed: P256MainVerifierFixedSourceV1::new_v1().map_err(map_p256_error_v1)?,
            next_column: 0,
            batch: Vec::new(),
        })
    }

    fn refill_v1(&mut self, column_start: usize) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
        if !self.batch.is_empty()
            || column_start != self.next_column
            || column_start >= ZK_X509_P256_LOG19_PREPROCESSED_FIXED_WIDTH_V1
        {
            return Err(ZkX509PreprocessedFixedErrorV1::Column);
        }
        let column_end = column_start
            .checked_add(ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
            .map(|end| end.min(ZK_X509_P256_LOG19_PREPROCESSED_FIXED_WIDTH_V1))
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        self.batch
            .try_reserve_exact(column_end - column_start)
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        let native_rows = 1_usize << ZK_X509_MAX_NATIVE_TRACE_LOG2_V1;
        for global_column in column_start..column_end {
            let (schedule, local_column) = p256_log19_fixed_schedule_for_column_v1(global_column)?;
            let mut column = Vec::new();
            column
                .try_reserve_exact(native_rows)
                .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
            column.resize(native_rows, F::ZERO);
            self.fixed
                .fill_fixed_column_v1(
                    schedule.representative_registration_v1()?,
                    local_column,
                    &mut column,
                )
                .map_err(map_p256_error_v1)?;
            self.batch.push(column);
        }
        if self.batch.iter().any(|column| column.len() != native_rows) {
            return Err(ZkX509PreprocessedFixedErrorV1::Column);
        }
        Ok(())
    }

    fn native_column_v1(
        &mut self,
        column: usize,
    ) -> Result<Vec<F>, ZkX509PreprocessedFixedErrorV1> {
        if column != self.next_column || column >= ZK_X509_P256_LOG19_PREPROCESSED_FIXED_WIDTH_V1 {
            return Err(ZkX509PreprocessedFixedErrorV1::Column);
        }
        if self.batch.is_empty() {
            self.refill_v1(column)?;
        }
        let native = self.batch.remove(0);
        self.next_column = self
            .next_column
            .checked_add(1)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        Ok(native)
    }
}

fn fixed_leaf_hash_v1(
    geometry: ZkX509PreprocessedFixedGeometryV1,
    row: &[u64],
) -> Result<[u8; 32], ZkX509PreprocessedFixedErrorV1> {
    geometry.validate()?;
    if row.len() != usize::from(geometry.width)
        || row
            .iter()
            .copied()
            .any(|value| F::canonical(value).is_none())
    {
        return Err(ZkX509PreprocessedFixedErrorV1::Opening);
    }
    let mut hash = fixed_leaf_prefix_hasher_v1(geometry)?;
    for value in row {
        hash.update(value.to_be_bytes());
    }
    Ok(hash.finalize().into())
}

fn fixed_leaf_prefix_bytes_v1(
    geometry: ZkX509PreprocessedFixedGeometryV1,
) -> Result<Vec<u8>, ZkX509PreprocessedFixedErrorV1> {
    geometry.validate()?;
    let payload_bytes = u64::from(geometry.width)
        .checked_mul(8)
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(ZK_X509_PREPROCESSED_FIXED_LEAF_PREFIX_BYTES_V1)
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
    bytes.extend_from_slice(TRANSCRIPT_FRAME_DOMAIN_V1);
    bytes.extend_from_slice(
        &u16::try_from(FIXED_LEAF_DOMAIN_V1.len())
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Profile)?
            .to_be_bytes(),
    );
    bytes.extend_from_slice(FIXED_LEAF_DOMAIN_V1);
    bytes.extend_from_slice(&4_u16.to_be_bytes());
    let oracle = geometry.oracle.to_be_bytes();
    let domain = [geometry.native_log2, geometry.lde_log2];
    let width = geometry.width.to_be_bytes();
    for field in [&oracle[..], &domain[..], &width[..]] {
        bytes.extend_from_slice(
            &u64::try_from(field.len())
                .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?
                .to_be_bytes(),
        );
        bytes.extend_from_slice(field);
    }
    bytes.extend_from_slice(&payload_bytes.to_be_bytes());
    if bytes.len() != ZK_X509_PREPROCESSED_FIXED_LEAF_PREFIX_BYTES_V1 {
        return Err(ZkX509PreprocessedFixedErrorV1::Profile);
    }
    Ok(bytes)
}

fn fixed_leaf_prefix_hasher_v1(
    geometry: ZkX509PreprocessedFixedGeometryV1,
) -> Result<Sha256, ZkX509PreprocessedFixedErrorV1> {
    let mut hash = Sha256::new();
    hash.update(fixed_leaf_prefix_bytes_v1(geometry)?);
    Ok(hash)
}

const SHA256_INITIAL_STATE_V1: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

fn compress_sha256_block_v1(state: &mut [u32; 8], block: &[u8; 64]) {
    let block = DigestBlock::<Sha256VarCore>::clone_from_slice(block);
    compress256(state, core::slice::from_ref(&block));
}

/// SHA-256 midstate specialized to the fixed leaf frame.
///
/// The 130-byte common prefix leaves exactly two buffered bytes. Every
/// complete row-major batch contributes 64 bytes. A final partial batch is
/// supplied only to `finalize_v1`, so the per-row state never grows a general
/// purpose `Sha256` buffer, length counter, or padding state.
#[derive(Clone, Copy)]
struct CompactFixedLeafSha256V1 {
    state: [u32; 8],
    tail: [u8; 2],
    absorbed_batches: u16,
}

const _: () = assert!(
    core::mem::size_of::<CompactFixedLeafSha256V1>()
        == ZK_X509_PREPROCESSED_FIXED_COMPACT_SHA_STATE_BYTES_V1
);

impl CompactFixedLeafSha256V1 {
    fn from_geometry_v1(
        geometry: ZkX509PreprocessedFixedGeometryV1,
    ) -> Result<Self, ZkX509PreprocessedFixedErrorV1> {
        let prefix = fixed_leaf_prefix_bytes_v1(geometry)?;
        if prefix.len() % 64 != 2 {
            return Err(ZkX509PreprocessedFixedErrorV1::Profile);
        }
        let mut state = SHA256_INITIAL_STATE_V1;
        let full = prefix.len() - 2;
        for chunk in prefix[..full].chunks_exact(64) {
            compress_sha256_block_v1(
                &mut state,
                chunk
                    .try_into()
                    .map_err(|_| ZkX509PreprocessedFixedErrorV1::Profile)?,
            );
        }
        Ok(Self {
            state,
            tail: prefix[full..]
                .try_into()
                .map_err(|_| ZkX509PreprocessedFixedErrorV1::Profile)?,
            absorbed_batches: 0,
        })
    }

    fn absorb_batch8_v1(&mut self, values: [F; 8]) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
        let mut encoded = [0_u8; 64];
        for (target, value) in encoded.chunks_exact_mut(8).zip(values) {
            target.copy_from_slice(&value.0.to_be_bytes());
        }
        let mut block = [0_u8; 64];
        block[..2].copy_from_slice(&self.tail);
        block[2..].copy_from_slice(&encoded[..62]);
        compress_sha256_block_v1(&mut self.state, &block);
        self.tail.copy_from_slice(&encoded[62..]);
        self.absorbed_batches = self
            .absorbed_batches
            .checked_add(1)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        Ok(())
    }

    fn finalize_v1(
        mut self,
        geometry: ZkX509PreprocessedFixedGeometryV1,
        partial: Option<([F; 8], usize)>,
    ) -> Result<[u8; 32], ZkX509PreprocessedFixedErrorV1> {
        let message_bytes = u64::try_from(ZK_X509_PREPROCESSED_FIXED_LEAF_PREFIX_BYTES_V1)
            .ok()
            .and_then(|prefix| {
                u64::from(geometry.width)
                    .checked_mul(8)
                    .and_then(|payload| prefix.checked_add(payload))
            })
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        let width = usize::from(geometry.width);
        let expected_batches = width
            .checked_div(ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Profile)?;
        let remainder = width % ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1;
        let partial_values = match (remainder, partial) {
            (0, None) => [F::ZERO; 8],
            (expected, Some((values, actual)))
                if expected == actual
                    && values[..actual]
                        .iter()
                        .all(|value| F::canonical(value.0).is_some())
                    && values[actual..].iter().all(|value| *value == F::ZERO) =>
            {
                values
            }
            _ => return Err(ZkX509PreprocessedFixedErrorV1::Profile),
        };
        if usize::from(self.absorbed_batches) != expected_batches {
            return Err(ZkX509PreprocessedFixedErrorV1::Profile);
        }
        let mut final_blocks = [0_u8; 128];
        final_blocks[..2].copy_from_slice(&self.tail);
        for (target, value) in final_blocks[2..2 + remainder * 8]
            .chunks_exact_mut(8)
            .zip(partial_values)
        {
            target.copy_from_slice(&value.0.to_be_bytes());
        }
        let used = 2 + remainder * 8;
        final_blocks[used] = 0x80;
        let final_len = if used + 1 + core::mem::size_of::<u64>() <= 64 {
            64
        } else {
            128
        };
        final_blocks[final_len - 8..final_len].copy_from_slice(
            &message_bytes
                .checked_mul(8)
                .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?
                .to_be_bytes(),
        );
        for block in final_blocks[..final_len].chunks_exact(64) {
            compress_sha256_block_v1(
                &mut self.state,
                block
                    .try_into()
                    .map_err(|_| ZkX509PreprocessedFixedErrorV1::Profile)?,
            );
        }
        let mut digest = [0_u8; 32];
        for (target, word) in digest.chunks_exact_mut(4).zip(self.state) {
            target.copy_from_slice(&word.to_be_bytes());
        }
        Ok(digest)
    }
}

struct OrderedMerkleFrontierV1 {
    levels: Vec<Option<[u8; 32]>>,
    leaves: usize,
}

impl OrderedMerkleFrontierV1 {
    fn new_v1(log2: u8) -> Result<Self, ZkX509PreprocessedFixedErrorV1> {
        let mut levels = Vec::new();
        levels
            .try_reserve_exact(usize::from(log2) + 1)
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        levels.resize(usize::from(log2) + 1, None);
        Ok(Self { levels, leaves: 0 })
    }

    fn push_v1(&mut self, mut node: [u8; 32]) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
        let mut level = 0_usize;
        loop {
            let slot = self
                .levels
                .get_mut(level)
                .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
            if let Some(left) = slot.take() {
                node = sha256_merkle_node_v1(FIXED_NODE_DOMAIN_V1, &left, &node);
                level = level
                    .checked_add(1)
                    .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
            } else {
                *slot = Some(node);
                break;
            }
        }
        self.leaves = self
            .leaves
            .checked_add(1)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        Ok(())
    }

    fn finish_v1(
        mut self,
        expected_leaves: usize,
    ) -> Result<[u8; 32], ZkX509PreprocessedFixedErrorV1> {
        if self.leaves != expected_leaves || !expected_leaves.is_power_of_two() {
            return Err(ZkX509PreprocessedFixedErrorV1::Opening);
        }
        let root_level = usize::try_from(expected_leaves.ilog2())
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        let root = self
            .levels
            .get_mut(root_level)
            .and_then(Option::take)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Opening)?;
        if self.levels.into_iter().any(|node| node.is_some()) {
            return Err(ZkX509PreprocessedFixedErrorV1::Opening);
        }
        Ok(root)
    }
}

/// Left-to-right logarithmic Merkle reducer that additionally retains only
/// the canonical missing siblings required by one selected opening set.
///
/// Requested node coordinates are derived once from the canonical leaf set.
/// Every retained hash is captured when that node is formed, so neither all
/// leaves nor any complete Merkle level is materialized.
struct OrderedSelectedMerkleFrontierV1 {
    levels: Vec<Option<[u8; 32]>>,
    requested_by_level: Vec<Vec<usize>>,
    captured_by_level: Vec<Vec<[u8; 32]>>,
    next_requested_by_level: Vec<usize>,
    leaves: usize,
    expected_leaves: usize,
    expected_frontier: usize,
}

impl OrderedSelectedMerkleFrontierV1 {
    fn new_v1(
        geometry: ZkX509PreprocessedFixedGeometryV1,
        indices: &[usize],
    ) -> Result<Self, ZkX509PreprocessedFixedErrorV1> {
        canonical_indices_v1(geometry, indices)?;
        let expected_leaves = geometry.lde_rows()?;
        let height = usize::from(geometry.lde_log2);
        let expected_frontier =
            multiproof_frontier_len_v1(expected_leaves, indices).map_err(map_aggregate_error_v1)?;

        let mut levels = Vec::new();
        levels
            .try_reserve_exact(height + 1)
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        levels.resize(height + 1, None);

        let mut requested_by_level = Vec::new();
        requested_by_level
            .try_reserve_exact(height + 1)
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        let mut current = indices.iter().copied().collect::<BTreeSet<_>>();
        let mut level_size = expected_leaves;
        let mut requested_count = 0_usize;
        while level_size > 1 {
            let mut requested = Vec::new();
            for index in &current {
                if !current.contains(&(index ^ 1)) {
                    requested.push(index ^ 1);
                }
            }
            if requested.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ZkX509PreprocessedFixedErrorV1::Opening);
            }
            requested_count = requested_count
                .checked_add(requested.len())
                .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
            requested_by_level.push(requested);
            current = current.into_iter().map(|index| index >> 1).collect();
            level_size >>= 1;
        }
        requested_by_level.push(Vec::new());
        if requested_count != expected_frontier
            || requested_by_level.len() != height + 1
            || current.len() != 1
            || !current.contains(&0)
        {
            return Err(ZkX509PreprocessedFixedErrorV1::Opening);
        }

        let mut captured_by_level = Vec::new();
        captured_by_level
            .try_reserve_exact(height + 1)
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        for requested in &requested_by_level {
            let mut captured = Vec::new();
            captured
                .try_reserve_exact(requested.len())
                .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
            captured_by_level.push(captured);
        }
        let mut next_requested_by_level = Vec::new();
        next_requested_by_level
            .try_reserve_exact(height + 1)
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        next_requested_by_level.resize(height + 1, 0);
        Ok(Self {
            levels,
            requested_by_level,
            captured_by_level,
            next_requested_by_level,
            leaves: 0,
            expected_leaves,
            expected_frontier,
        })
    }

    fn capture_v1(
        &mut self,
        level: usize,
        index: usize,
        node: [u8; 32],
    ) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
        let requested = self
            .requested_by_level
            .get(level)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Opening)?;
        let cursor = self
            .next_requested_by_level
            .get_mut(level)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Opening)?;
        if let Some(expected) = requested.get(*cursor).copied() {
            if index > expected {
                return Err(ZkX509PreprocessedFixedErrorV1::Opening);
            }
            if index == expected {
                self.captured_by_level
                    .get_mut(level)
                    .ok_or(ZkX509PreprocessedFixedErrorV1::Opening)?
                    .push(node);
                *cursor = cursor
                    .checked_add(1)
                    .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
            }
        }
        Ok(())
    }

    fn push_v1(&mut self, mut node: [u8; 32]) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
        if self.leaves >= self.expected_leaves {
            return Err(ZkX509PreprocessedFixedErrorV1::Opening);
        }
        let mut level = 0_usize;
        let mut index = self.leaves;
        loop {
            self.capture_v1(level, index, node)?;
            let slot = self
                .levels
                .get_mut(level)
                .ok_or(ZkX509PreprocessedFixedErrorV1::Opening)?;
            if let Some(left) = slot.take() {
                node = sha256_merkle_node_v1(FIXED_NODE_DOMAIN_V1, &left, &node);
                level = level
                    .checked_add(1)
                    .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
                index >>= 1;
            } else {
                *slot = Some(node);
                break;
            }
        }
        self.leaves = self
            .leaves
            .checked_add(1)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        Ok(())
    }

    fn finish_v1(self) -> Result<([u8; 32], Vec<[u8; 32]>), ZkX509PreprocessedFixedErrorV1> {
        let Self {
            mut levels,
            requested_by_level,
            captured_by_level,
            next_requested_by_level,
            leaves,
            expected_leaves,
            expected_frontier,
        } = self;
        if leaves != expected_leaves || !expected_leaves.is_power_of_two() {
            return Err(ZkX509PreprocessedFixedErrorV1::Opening);
        }
        let root_level = usize::try_from(expected_leaves.ilog2())
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        let root = levels
            .get_mut(root_level)
            .and_then(Option::take)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Opening)?;
        if levels.into_iter().any(|node| node.is_some()) {
            return Err(ZkX509PreprocessedFixedErrorV1::Opening);
        }

        let mut frontier = Vec::new();
        frontier
            .try_reserve_exact(expected_frontier)
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        for ((requested, captured), cursor) in requested_by_level
            .into_iter()
            .zip(captured_by_level)
            .zip(next_requested_by_level)
        {
            if cursor != requested.len() || captured.len() != requested.len() {
                return Err(ZkX509PreprocessedFixedErrorV1::Opening);
            }
            frontier.extend(captured);
        }
        if frontier.len() != expected_frontier {
            return Err(ZkX509PreprocessedFixedErrorV1::Opening);
        }
        Ok((root, frontier))
    }
}

fn checked_streaming_memory_v1(
    geometry: ZkX509PreprocessedFixedGeometryV1,
) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
    let native_rows = geometry.native_rows()?;
    let lde_rows = geometry.lde_rows()?;
    let hashers = lde_rows
        .checked_mul(core::mem::size_of::<Sha256>())
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let lde_batch = lde_rows
        .checked_mul(core::mem::size_of::<F>())
        .and_then(|bytes| bytes.checked_mul(ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1))
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let leaves = lde_rows
        .checked_mul(core::mem::size_of::<[u8; 32]>())
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let native_batch = native_rows
        .checked_mul(core::mem::size_of::<F>())
        .and_then(|bytes| bytes.checked_mul(ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1))
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let opened_rows = ZK_X509_PREPROCESSED_FIXED_MAX_OPENINGS_V1
        .checked_mul(usize::from(geometry.width))
        .and_then(|fields| fields.checked_mul(core::mem::size_of::<u64>()))
        .and_then(|bytes| {
            ZK_X509_PREPROCESSED_FIXED_MAX_OPENINGS_V1
                .checked_mul(core::mem::size_of::<Vec<u64>>())
                .and_then(|headers| bytes.checked_add(headers))
        })
        .and_then(|bytes| {
            ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1
                .checked_mul(core::mem::size_of::<Vec<F>>())
                .and_then(|headers| bytes.checked_add(headers))
        })
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let lde_phase = hashers
        .checked_add(lde_batch)
        .and_then(|bytes| bytes.checked_add(native_batch))
        .and_then(|bytes| bytes.checked_add(opened_rows))
        .and_then(|bytes| bytes.checked_add(ZK_X509_PREPROCESSED_FIXED_ALLOCATION_RESERVE_BYTES_V1))
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let tree_phase = hashers
        .checked_add(leaves)
        .and_then(|bytes| bytes.checked_add(opened_rows))
        .and_then(|bytes| bytes.checked_add(ZK_X509_PREPROCESSED_FIXED_ALLOCATION_RESERVE_BYTES_V1))
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    if u64::try_from(lde_phase.max(tree_phase))
        .ok()
        .is_none_or(|bytes| bytes > ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1)
    {
        return Err(ZkX509PreprocessedFixedErrorV1::Resource);
    }
    Ok(())
}

fn checked_release_root_memory_v1(
    geometry: ZkX509PreprocessedFixedGeometryV1,
) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
    let native_rows = geometry.native_rows()?;
    let lde_rows = geometry.lde_rows()?;
    let states = lde_rows
        .checked_mul(core::mem::size_of::<CompactFixedLeafSha256V1>())
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let lde_batch = lde_rows
        .checked_mul(core::mem::size_of::<[F; 8]>())
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let native_batch = native_rows
        .checked_mul(core::mem::size_of::<[F; 8]>())
        .and_then(|bytes| bytes.checked_mul(2))
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let maximum_twiddles = (lde_rows / 2)
        .checked_mul(core::mem::size_of::<F>())
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let finalize_chunk = ZK_X509_PREPROCESSED_FIXED_FINALIZE_CHUNK_ROWS_V1
        .checked_mul(core::mem::size_of::<[u8; 32]>())
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    let peak = states
        .checked_add(lde_batch)
        .and_then(|bytes| bytes.checked_add(native_batch))
        .and_then(|bytes| bytes.checked_add(maximum_twiddles))
        .and_then(|bytes| bytes.checked_add(finalize_chunk))
        .and_then(|bytes| bytes.checked_add(ZK_X509_PREPROCESSED_FIXED_ALLOCATION_RESERVE_BYTES_V1))
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    if u64::try_from(peak).ok().is_none_or(|bytes| {
        bytes > ZK_X509_SHA_PREPROCESSED_ROOT_MAX_RSS_BYTES_V1
            || bytes > ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1
    }) {
        return Err(ZkX509PreprocessedFixedErrorV1::Resource);
    }
    Ok(())
}

fn goldilocks_fft_batch8_v1(
    values: &mut [[F; 8]],
    root: F,
) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
    let size = values.len();
    if size == 0
        || !size.is_power_of_two()
        || F::canonical(root.0).is_none()
        || root.pow(size as u128) != F::ONE
        || (size > 1 && root.pow((size / 2) as u128) == F::ONE)
        || values
            .iter()
            .flatten()
            .any(|value| F::canonical(value.0).is_none())
    {
        return Err(ZkX509PreprocessedFixedErrorV1::Profile);
    }
    let mut reversed = 0_usize;
    for index in 1..size {
        let mut bit = size >> 1;
        while reversed & bit != 0 {
            reversed ^= bit;
            bit >>= 1;
        }
        reversed ^= bit;
        if index < reversed {
            values.swap(index, reversed);
        }
    }

    let parallelism = rayon::current_num_threads().max(1);
    let mut width = 2_usize;
    while width <= size {
        let step = root.pow((size / width) as u128);
        let chunk_count = size / width;
        if chunk_count >= parallelism {
            values.par_chunks_exact_mut(width).for_each(|chunk| {
                let mut twiddle = F::ONE;
                let (left, right) = chunk.split_at_mut(width / 2);
                for (even, odd) in left.iter_mut().zip(right.iter_mut()) {
                    for lane in 0..8 {
                        let scaled_odd = odd[lane].mul(twiddle);
                        let original_even = even[lane];
                        even[lane] = original_even.add(scaled_odd);
                        odd[lane] = original_even.sub(scaled_odd);
                    }
                    twiddle = twiddle.mul(step);
                }
            });
        } else {
            let mut twiddles = Vec::new();
            twiddles
                .try_reserve_exact(width / 2)
                .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
            let mut twiddle = F::ONE;
            for _ in 0..width / 2 {
                twiddles.push(twiddle);
                twiddle = twiddle.mul(step);
            }
            for chunk in values.chunks_exact_mut(width) {
                let (left, right) = chunk.split_at_mut(width / 2);
                left.par_iter_mut()
                    .zip(right.par_iter_mut())
                    .zip(twiddles.par_iter().copied())
                    .for_each(|((even, odd), twiddle)| {
                        for lane in 0..8 {
                            let scaled_odd = odd[lane].mul(twiddle);
                            let original_even = even[lane];
                            even[lane] = original_even.add(scaled_odd);
                            odd[lane] = original_even.sub(scaled_odd);
                        }
                    });
            }
        }
        width = width
            .checked_mul(2)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    }
    Ok(())
}

fn goldilocks_ifft_batch8_v1(
    values: &mut [[F; 8]],
    root: F,
) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
    goldilocks_fft_batch8_v1(
        values,
        root.inv().ok_or(ZkX509PreprocessedFixedErrorV1::Profile)?,
    )?;
    let inverse_size = F::reduce(values.len() as u128)
        .inv()
        .ok_or(ZkX509PreprocessedFixedErrorV1::Profile)?;
    values.par_iter_mut().for_each(|row| {
        for value in row {
            *value = value.mul(inverse_size);
        }
    });
    Ok(())
}

fn checked_goldilocks_evaluate_coset_batch8_v1(
    mut native: Vec<[F; 8]>,
    geometry: ZkX509PreprocessedFixedGeometryV1,
) -> Result<Vec<[F; 8]>, ZkX509PreprocessedFixedErrorV1> {
    let native_rows = geometry.native_rows()?;
    let lde_rows = geometry.lde_rows()?;
    if native.len() != native_rows {
        return Err(ZkX509PreprocessedFixedErrorV1::Column);
    }
    let trace_root =
        goldilocks_primitive_root_v1(geometry.native_log2).map_err(map_transparent_error_v1)?;
    let lde_root =
        goldilocks_primitive_root_v1(geometry.lde_log2).map_err(map_transparent_error_v1)?;
    goldilocks_ifft_batch8_v1(&mut native, trace_root)?;
    let mut evaluations = Vec::new();
    evaluations
        .try_reserve_exact(lde_rows)
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
    evaluations.resize(lde_rows, [F::ZERO; 8]);
    let mut shift_power = F::ONE;
    for (target, coefficients) in evaluations.iter_mut().zip(native) {
        for lane in 0..8 {
            target[lane] = coefficients[lane].mul(shift_power);
        }
        shift_power = shift_power.mul(F(GOLDILOCKS_GENERATOR_V1));
    }
    goldilocks_fft_batch8_v1(&mut evaluations, lde_root)?;
    Ok(evaluations)
}

fn materialize_native_batch8_v1(
    geometry: ZkX509PreprocessedFixedGeometryV1,
    column_start: usize,
    native_column: &mut impl FnMut(usize) -> Result<Vec<F>, ZkX509PreprocessedFixedErrorV1>,
) -> Result<Vec<[F; 8]>, ZkX509PreprocessedFixedErrorV1> {
    let native_rows = geometry.native_rows()?;
    let column_end = column_start
        .checked_add(ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
        .map(|end| end.min(usize::from(geometry.width)))
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    if column_start >= column_end {
        return Err(ZkX509PreprocessedFixedErrorV1::Column);
    }
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
    for column in column_start..column_end {
        let values = native_column(column)?;
        if values.len() != native_rows || values.iter().any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509PreprocessedFixedErrorV1::Column);
        }
        columns.push(values);
    }
    let mut rows = Vec::new();
    rows.try_reserve_exact(native_rows)
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
    rows.resize(native_rows, [F::ZERO; 8]);
    for (lane, column) in columns.into_iter().enumerate() {
        for (row, value) in rows.iter_mut().zip(column) {
            row[lane] = value;
        }
    }
    Ok(rows)
}

fn checked_goldilocks_evaluate_coset_v1(
    coefficients: &[F],
    size: usize,
    root: F,
    shift: F,
) -> Result<Vec<F>, ZkX509PreprocessedFixedErrorV1> {
    if coefficients.len() > size || size == 0 || !size.is_power_of_two() || shift == F::ZERO {
        return Err(ZkX509PreprocessedFixedErrorV1::Profile);
    }
    let mut evaluations = Vec::new();
    evaluations
        .try_reserve_exact(size)
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
    evaluations.resize(size, F::ZERO);
    let mut shift_power = F::ONE;
    for (target, coefficient) in evaluations.iter_mut().zip(coefficients.iter().copied()) {
        *target = coefficient.mul(shift_power);
        shift_power = shift_power.mul(shift);
    }
    goldilocks_fft_v1(&mut evaluations, root).map_err(map_transparent_error_v1)?;
    Ok(evaluations)
}

fn reduce_fixed_tree_v1(
    mut nodes: Vec<[u8; 32]>,
    indices: &[usize],
) -> Result<([u8; 32], Vec<[u8; 32]>), ZkX509PreprocessedFixedErrorV1> {
    if nodes.is_empty() || !nodes.len().is_power_of_two() {
        return Err(ZkX509PreprocessedFixedErrorV1::Profile);
    }
    let mut current = indices.iter().copied().collect::<BTreeSet<_>>();
    let mut frontier = Vec::new();
    if !indices.is_empty() {
        frontier
            .try_reserve_exact(
                multiproof_frontier_len_v1(nodes.len(), indices).map_err(map_aggregate_error_v1)?,
            )
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
    }
    while nodes.len() > 1 {
        for index in &current {
            if !current.contains(&(index ^ 1)) {
                frontier.push(
                    *nodes
                        .get(index ^ 1)
                        .ok_or(ZkX509PreprocessedFixedErrorV1::Opening)?,
                );
            }
        }
        let parent_count = nodes.len() / 2;
        for parent in 0..parent_count {
            let left = nodes[2 * parent];
            let right = nodes[2 * parent + 1];
            nodes[parent] = sha256_merkle_node_v1(FIXED_NODE_DOMAIN_V1, &left, &right);
        }
        nodes.truncate(parent_count);
        current = current.into_iter().map(|index| index >> 1).collect();
    }
    Ok((nodes[0], frontier))
}

fn stream_fixed_oracle_v1(
    geometry: ZkX509PreprocessedFixedGeometryV1,
    indices: &[usize],
    mut native_column: impl FnMut(usize) -> Result<Vec<F>, ZkX509PreprocessedFixedErrorV1>,
) -> Result<([u8; 32], Vec<Vec<u64>>, Vec<[u8; 32]>), ZkX509PreprocessedFixedErrorV1> {
    geometry.validate()?;
    if !indices.is_empty() {
        canonical_indices_v1(geometry, indices)?;
    }
    checked_streaming_memory_v1(geometry)?;
    let native_rows = geometry.native_rows()?;
    let lde_rows = geometry.lde_rows()?;
    let trace_root =
        goldilocks_primitive_root_v1(geometry.native_log2).map_err(map_transparent_error_v1)?;
    let lde_root =
        goldilocks_primitive_root_v1(geometry.lde_log2).map_err(map_transparent_error_v1)?;

    let prefix = fixed_leaf_prefix_hasher_v1(geometry)?;
    let mut hashers = Vec::new();
    hashers
        .try_reserve_exact(lde_rows)
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
    hashers.resize(lde_rows, prefix);

    let mut opened_rows = Vec::new();
    opened_rows
        .try_reserve_exact(indices.len())
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
    for _ in indices {
        let mut row = Vec::new();
        row.try_reserve_exact(usize::from(geometry.width))
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        opened_rows.push(row);
    }

    for column_start in
        (0..usize::from(geometry.width)).step_by(ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
    {
        let column_end = column_start
            .checked_add(ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
            .map(|end| end.min(usize::from(geometry.width)))
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        let mut evaluations = Vec::new();
        evaluations
            .try_reserve_exact(column_end - column_start)
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        for column in column_start..column_end {
            let mut coefficients = native_column(column)?;
            if coefficients.len() != native_rows
                || coefficients
                    .iter()
                    .any(|value| F::canonical(value.0).is_none())
            {
                return Err(ZkX509PreprocessedFixedErrorV1::Column);
            }
            goldilocks_ifft_v1(&mut coefficients, trace_root).map_err(map_transparent_error_v1)?;
            evaluations.push(checked_goldilocks_evaluate_coset_v1(
                &coefficients,
                lde_rows,
                lde_root,
                F(GOLDILOCKS_GENERATOR_V1),
            )?);
        }
        let batch_width = evaluations.len();
        for row in 0..lde_rows {
            let mut encoded = [0_u8; ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1 * 8];
            for (local, column) in evaluations.iter().enumerate() {
                encoded[local * 8..local * 8 + 8].copy_from_slice(&column[row].0.to_be_bytes());
            }
            hashers[row].update(&encoded[..batch_width * 8]);
        }
        for (opened, index) in opened_rows.iter_mut().zip(indices.iter().copied()) {
            for column in &evaluations {
                opened.push(
                    column
                        .get(index)
                        .ok_or(ZkX509PreprocessedFixedErrorV1::Index)?
                        .0,
                );
            }
        }
    }
    if opened_rows
        .iter()
        .any(|row| row.len() != usize::from(geometry.width))
    {
        return Err(ZkX509PreprocessedFixedErrorV1::Column);
    }

    let mut leaves = Vec::new();
    leaves
        .try_reserve_exact(lde_rows)
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
    for hash in hashers {
        leaves.push(hash.finalize().into());
    }
    let (root, frontier) = reduce_fixed_tree_v1(leaves, indices)?;
    Ok((root, opened_rows, frontier))
}

fn compact_fixed_leaf_states_and_openings_batch8_v1(
    geometry: ZkX509PreprocessedFixedGeometryV1,
    indices: &[usize],
    mut native_column: impl FnMut(usize) -> Result<Vec<F>, ZkX509PreprocessedFixedErrorV1>,
) -> Result<
    (
        Vec<CompactFixedLeafSha256V1>,
        Option<(Vec<[F; 8]>, usize)>,
        Vec<Vec<u64>>,
    ),
    ZkX509PreprocessedFixedErrorV1,
> {
    geometry.validate()?;
    if !indices.is_empty() {
        canonical_indices_v1(geometry, indices)?;
    }
    let lde_rows = geometry.lde_rows()?;
    let prefix = CompactFixedLeafSha256V1::from_geometry_v1(geometry)?;
    let mut states = Vec::new();
    states
        .try_reserve_exact(lde_rows)
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
    states.resize(lde_rows, prefix);

    let mut opened_rows = Vec::new();
    opened_rows
        .try_reserve_exact(indices.len())
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
    for _ in indices {
        let mut row = Vec::new();
        row.try_reserve_exact(usize::from(geometry.width))
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        opened_rows.push(row);
    }

    let mut partial = None;
    for column_start in
        (0..usize::from(geometry.width)).step_by(ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
    {
        let batch_width = usize::from(geometry.width)
            .checked_sub(column_start)
            .map(|remaining| remaining.min(ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1))
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        let native = materialize_native_batch8_v1(geometry, column_start, &mut native_column)?;
        let evaluations = checked_goldilocks_evaluate_coset_batch8_v1(native, geometry)?;
        if evaluations.len() != states.len() {
            return Err(ZkX509PreprocessedFixedErrorV1::Column);
        }
        for (opened, index) in opened_rows.iter_mut().zip(indices.iter().copied()) {
            opened.extend(
                evaluations
                    .get(index)
                    .ok_or(ZkX509PreprocessedFixedErrorV1::Index)?
                    .get(..batch_width)
                    .ok_or(ZkX509PreprocessedFixedErrorV1::Column)?
                    .iter()
                    .map(|value| value.0),
            );
        }
        if batch_width == ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1 {
            states
                .par_iter_mut()
                .zip(evaluations.par_iter().copied())
                .try_for_each(|(state, row)| state.absorb_batch8_v1(row))?;
        } else if partial.replace((evaluations, batch_width)).is_some() {
            return Err(ZkX509PreprocessedFixedErrorV1::Column);
        }
    }
    if opened_rows
        .iter()
        .any(|row| row.len() != usize::from(geometry.width))
    {
        return Err(ZkX509PreprocessedFixedErrorV1::Column);
    }
    if partial.is_some()
        != (usize::from(geometry.width) % ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1 != 0)
    {
        return Err(ZkX509PreprocessedFixedErrorV1::Column);
    }
    Ok((states, partial, opened_rows))
}

fn stream_fixed_oracle_root_batch8_v1(
    geometry: ZkX509PreprocessedFixedGeometryV1,
    native_column: impl FnMut(usize) -> Result<Vec<F>, ZkX509PreprocessedFixedErrorV1>,
) -> Result<[u8; 32], ZkX509PreprocessedFixedErrorV1> {
    geometry.validate()?;
    checked_release_root_memory_v1(geometry)?;
    let lde_rows = geometry.lde_rows()?;
    let (states, partial, opened_rows) =
        compact_fixed_leaf_states_and_openings_batch8_v1(geometry, &[], native_column)?;
    if !opened_rows.is_empty() {
        return Err(ZkX509PreprocessedFixedErrorV1::Opening);
    }

    let mut frontier = OrderedMerkleFrontierV1::new_v1(geometry.lde_log2)?;
    for (chunk_index, chunk) in states
        .chunks(ZK_X509_PREPROCESSED_FIXED_FINALIZE_CHUNK_ROWS_V1)
        .enumerate()
    {
        let row_start = chunk_index
            .checked_mul(ZK_X509_PREPROCESSED_FIXED_FINALIZE_CHUNK_ROWS_V1)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        let leaves = chunk
            .par_iter()
            .copied()
            .enumerate()
            .map(|(offset, state)| {
                let partial_row = partial
                    .as_ref()
                    .map(|(rows, width)| {
                        rows.get(row_start + offset)
                            .copied()
                            .map(|row| (row, *width))
                            .ok_or(ZkX509PreprocessedFixedErrorV1::Column)
                    })
                    .transpose()?;
                state.finalize_v1(geometry, partial_row)
            })
            .collect::<Result<Vec<_>, _>>()?;
        for leaf in leaves {
            frontier.push_v1(leaf)?;
        }
    }
    frontier.finish_v1(lde_rows)
}

fn stream_fixed_oracle_multiproof_batch8_v1(
    geometry: ZkX509PreprocessedFixedGeometryV1,
    indices: &[usize],
    native_column: impl FnMut(usize) -> Result<Vec<F>, ZkX509PreprocessedFixedErrorV1>,
) -> Result<([u8; 32], Vec<Vec<u64>>, Vec<[u8; 32]>), ZkX509PreprocessedFixedErrorV1> {
    geometry.validate()?;
    canonical_indices_v1(geometry, indices)?;
    // The selected rows and at most 2,100 retained hashes are covered by the
    // explicit 256 MiB allocator reserve in the release-root RSS certificate.
    checked_release_root_memory_v1(geometry)?;
    let (states, partial, opened_rows) =
        compact_fixed_leaf_states_and_openings_batch8_v1(geometry, indices, native_column)?;
    let mut frontier = OrderedSelectedMerkleFrontierV1::new_v1(geometry, indices)?;
    for (chunk_index, chunk) in states
        .chunks(ZK_X509_PREPROCESSED_FIXED_FINALIZE_CHUNK_ROWS_V1)
        .enumerate()
    {
        let row_start = chunk_index
            .checked_mul(ZK_X509_PREPROCESSED_FIXED_FINALIZE_CHUNK_ROWS_V1)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        let leaves = chunk
            .par_iter()
            .copied()
            .enumerate()
            .map(|(offset, state)| {
                let partial_row = partial
                    .as_ref()
                    .map(|(rows, width)| {
                        rows.get(row_start + offset)
                            .copied()
                            .map(|row| (row, *width))
                            .ok_or(ZkX509PreprocessedFixedErrorV1::Column)
                    })
                    .transpose()?;
                state.finalize_v1(geometry, partial_row)
            })
            .collect::<Result<Vec<_>, _>>()?;
        for leaf in leaves {
            frontier.push_v1(leaf)?;
        }
    }
    let (root, frontier) = frontier.finish_v1()?;
    Ok((root, opened_rows, frontier))
}

/// Recompute a fixed-oracle root from its canonical native columns.
pub(crate) fn recompute_zk_x509_preprocessed_fixed_root_v1(
    geometry: ZkX509PreprocessedFixedGeometryV1,
    native_column: impl FnMut(usize) -> Result<Vec<F>, ZkX509PreprocessedFixedErrorV1>,
) -> Result<[u8; 32], ZkX509PreprocessedFixedErrorV1> {
    stream_fixed_oracle_v1(geometry, &[], native_column).map(|material| material.0)
}

/// Derive the actual combined SHA fixed-oracle root from the exact 340-column
/// production provider.
pub(crate) fn derive_zk_x509_sha_preprocessed_fixed_root_v1()
-> Result<[u8; 32], ZkX509PreprocessedFixedErrorV1> {
    let mut provider = ZkX509ShaPreprocessedNativeColumnProviderV1::new_v1()?;
    stream_fixed_oracle_root_batch8_v1(ZK_X509_SHA_PREPROCESSED_FIXED_GEOMETRY_V1, |column| {
        provider.native_column_v1(column)
    })
}

/// Derive the actual P-256 log19 fixed-oracle root from the exact six-schedule
/// production provider.
pub(crate) fn derive_zk_x509_p256_log19_preprocessed_fixed_root_v1()
-> Result<[u8; 32], ZkX509PreprocessedFixedErrorV1> {
    let mut provider = ZkX509P256Log19PreprocessedNativeColumnProviderV1::new_v1()?;
    stream_fixed_oracle_root_batch8_v1(
        ZK_X509_P256_LOG19_PREPROCESSED_FIXED_GEOMETRY_V1,
        |column| provider.native_column_v1(column),
    )
}

/// Construct one canonical multiproof and verify regeneration against the
/// pinned root.
pub(crate) fn build_zk_x509_preprocessed_fixed_multiproof_v1(
    profile: ZkX509PreprocessedFixedProfileV1,
    indices: &[usize],
    native_column: impl FnMut(usize) -> Result<Vec<F>, ZkX509PreprocessedFixedErrorV1>,
) -> Result<ZkX509PreprocessedFixedMultiproofV1, ZkX509PreprocessedFixedErrorV1> {
    profile.validate()?;
    canonical_indices_v1(profile.geometry, indices)?;
    let (root, rows, frontier) = stream_fixed_oracle_v1(profile.geometry, indices, native_column)?;
    if root != profile.root {
        return Err(ZkX509PreprocessedFixedErrorV1::RootMismatch);
    }
    Ok(ZkX509PreprocessedFixedMultiproofV1 {
        oracle: profile.geometry.oracle,
        indices: indices
            .iter()
            .copied()
            .map(|index| u32::try_from(index).map_err(|_| ZkX509PreprocessedFixedErrorV1::Index))
            .collect::<Result<_, _>>()?,
        rows,
        frontier,
    })
}

fn build_zk_x509_preprocessed_fixed_multiproof_batch8_v1(
    profile: ZkX509PreprocessedFixedProfileV1,
    indices: &[usize],
    native_column: impl FnMut(usize) -> Result<Vec<F>, ZkX509PreprocessedFixedErrorV1>,
) -> Result<ZkX509PreprocessedFixedMultiproofV1, ZkX509PreprocessedFixedErrorV1> {
    profile.validate()?;
    canonical_indices_v1(profile.geometry, indices)?;
    let (root, rows, frontier) =
        stream_fixed_oracle_multiproof_batch8_v1(profile.geometry, indices, native_column)?;
    if root != profile.root {
        return Err(ZkX509PreprocessedFixedErrorV1::RootMismatch);
    }
    Ok(ZkX509PreprocessedFixedMultiproofV1 {
        oracle: profile.geometry.oracle,
        indices: indices
            .iter()
            .copied()
            .map(|index| u32::try_from(index).map_err(|_| ZkX509PreprocessedFixedErrorV1::Index))
            .collect::<Result<_, _>>()?,
        rows,
        frontier,
    })
}

/// Construct the sole SHA fixed-oracle multiproof against the independently
/// pinned release certificate and transcript-query-derived opening set.
pub(crate) fn build_zk_x509_sha_preprocessed_fixed_multiproof_v1(
    supplied: ZkX509ShaPreprocessedFixedCertificateV1,
    indices: &ZkX509Log19PreprocessedFixedOpeningIndicesV1,
) -> Result<ZkX509PreprocessedFixedMultiproofV1, ZkX509PreprocessedFixedErrorV1> {
    let expected = pinned_zk_x509_sha_preprocessed_fixed_certificate_v1()?;
    validate_zk_x509_sha_preprocessed_fixed_certificate_v1(supplied, expected)?;
    zk_x509_sha_preprocessed_fixed_maximum_encoded_bytes_v1()?;
    let mut provider = ZkX509ShaPreprocessedNativeColumnProviderV1::new_v1()?;
    build_zk_x509_preprocessed_fixed_multiproof_batch8_v1(
        expected.profile,
        indices.as_slice_v1(),
        |column| provider.native_column_v1(column),
    )
}

/// Construct the P-256 log19 fixed-oracle multiproof against the independently
/// pinned release certificate and transcript-query-derived opening set.
pub(crate) fn build_zk_x509_p256_log19_preprocessed_fixed_multiproof_v1(
    supplied: ZkX509P256Log19PreprocessedFixedCertificateV1,
    indices: &ZkX509Log19PreprocessedFixedOpeningIndicesV1,
) -> Result<ZkX509PreprocessedFixedMultiproofV1, ZkX509PreprocessedFixedErrorV1> {
    let expected = pinned_zk_x509_p256_log19_preprocessed_fixed_certificate_v1()?;
    validate_zk_x509_p256_log19_preprocessed_fixed_certificate_v1(supplied, expected)?;
    zk_x509_p256_log19_preprocessed_fixed_maximum_encoded_bytes_v1()?;
    let mut provider = ZkX509P256Log19PreprocessedNativeColumnProviderV1::new_v1()?;
    build_zk_x509_preprocessed_fixed_multiproof_batch8_v1(
        expected.profile,
        indices.as_slice_v1(),
        |column| provider.native_column_v1(column),
    )
}

/// Verify one multiproof against the exact verifier-derived indices and
/// consensus-pinned root.
pub(crate) fn verify_zk_x509_preprocessed_fixed_multiproof_v1(
    profile: ZkX509PreprocessedFixedProfileV1,
    expected_indices: &[usize],
    proof: &ZkX509PreprocessedFixedMultiproofV1,
) -> Result<BTreeMap<usize, Vec<F>>, ZkX509PreprocessedFixedErrorV1> {
    profile.validate()?;
    canonical_indices_v1(profile.geometry, expected_indices)?;
    let proof_indices = proof
        .indices
        .iter()
        .copied()
        .map(|index| usize::try_from(index).map_err(|_| ZkX509PreprocessedFixedErrorV1::Index))
        .collect::<Result<Vec<_>, _>>()?;
    if proof.oracle != profile.geometry.oracle
        || proof_indices != expected_indices
        || proof.rows.len() != expected_indices.len()
    {
        return Err(ZkX509PreprocessedFixedErrorV1::Index);
    }
    let mut leaves = BTreeMap::new();
    let mut opened = BTreeMap::new();
    for ((index, encoded), expected) in proof_indices
        .iter()
        .copied()
        .zip(&proof.rows)
        .zip(expected_indices.iter().copied())
    {
        if index != expected || encoded.len() != usize::from(profile.geometry.width) {
            return Err(ZkX509PreprocessedFixedErrorV1::Opening);
        }
        let leaf = fixed_leaf_hash_v1(profile.geometry, encoded)?;
        if leaves.insert(index, leaf).is_some() {
            return Err(ZkX509PreprocessedFixedErrorV1::Index);
        }
        let row = encoded
            .iter()
            .copied()
            .map(|value| F::canonical(value).ok_or(ZkX509PreprocessedFixedErrorV1::Opening))
            .collect::<Result<Vec<_>, _>>()?;
        if opened.insert(index, row).is_some() {
            return Err(ZkX509PreprocessedFixedErrorV1::Index);
        }
    }
    verify_canonical_multiproof_v1(
        FIXED_NODE_DOMAIN_V1,
        &profile.root,
        profile.geometry.lde_rows()?,
        &leaves,
        &proof.frontier,
    )
    .map_err(map_aggregate_error_v1)?;
    Ok(opened)
}

/// Verify the sole SHA fixed-oracle multiproof against the same certificate
/// consumed by the compiled profile and prover. The expanded opening set can
/// only be obtained by validating and expanding the canonical MAIN query
/// coordinates through
/// [`derive_zk_x509_log19_preprocessed_fixed_opening_indices_v1`].
pub(crate) fn verify_zk_x509_sha_preprocessed_fixed_multiproof_v1(
    supplied: ZkX509ShaPreprocessedFixedCertificateV1,
    expected_indices: &ZkX509Log19PreprocessedFixedOpeningIndicesV1,
    proof: &ZkX509PreprocessedFixedMultiproofV1,
) -> Result<BTreeMap<usize, Vec<F>>, ZkX509PreprocessedFixedErrorV1> {
    let expected = pinned_zk_x509_sha_preprocessed_fixed_certificate_v1()?;
    validate_zk_x509_sha_preprocessed_fixed_certificate_v1(supplied, expected)?;
    verify_zk_x509_preprocessed_fixed_multiproof_v1(
        expected.profile,
        expected_indices.as_slice_v1(),
        proof,
    )
}

/// Verify the P-256 log19 multiproof against its pinned certificate and the
/// same typed opening set used by the SHA oracle.
pub(crate) fn verify_zk_x509_p256_log19_preprocessed_fixed_multiproof_v1(
    supplied: ZkX509P256Log19PreprocessedFixedCertificateV1,
    expected_indices: &ZkX509Log19PreprocessedFixedOpeningIndicesV1,
    proof: &ZkX509PreprocessedFixedMultiproofV1,
) -> Result<BTreeMap<usize, Vec<F>>, ZkX509PreprocessedFixedErrorV1> {
    let expected = pinned_zk_x509_p256_log19_preprocessed_fixed_certificate_v1()?;
    validate_zk_x509_p256_log19_preprocessed_fixed_certificate_v1(supplied, expected)?;
    verify_zk_x509_preprocessed_fixed_multiproof_v1(
        expected.profile,
        expected_indices.as_slice_v1(),
        proof,
    )
}

fn request_zk_x509_main_preprocessed_fixed_openings_for_profiles_v1(
    profiles: &[ZkX509PreprocessedFixedProfileV1; ZK_X509_PREPROCESSED_FIXED_ORACLE_COUNT_V1],
    indices: &ZkX509Log19PreprocessedFixedOpeningIndicesV1,
    service: &mut impl ZkX509MainPreprocessedFixedOpeningServiceV1,
) -> Result<Vec<u8>, ZkX509PreprocessedFixedErrorV1> {
    validate_profiles_v1(profiles)?;
    zk_x509_main_preprocessed_fixed_maximum_encoded_bytes_v1()?;
    let encoded = service.open_main_v1(profiles, indices)?;
    if encoded.len() > ZK_X509_MAIN_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1 {
        return Err(ZkX509PreprocessedFixedErrorV1::Resource);
    }
    let proof = decode_zk_x509_preprocessed_fixed_proof_v1(profiles, &encoded)?;
    let [sha, p256]: &[ZkX509PreprocessedFixedMultiproofV1; 2] = proof
        .oracles
        .as_slice()
        .try_into()
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::MalformedProof)?;
    let sha_rows =
        verify_zk_x509_preprocessed_fixed_multiproof_v1(profiles[0], indices.as_slice_v1(), sha)?;
    let p256_rows =
        verify_zk_x509_preprocessed_fixed_multiproof_v1(profiles[1], indices.as_slice_v1(), p256)?;
    if sha_rows.keys().ne(p256_rows.keys())
        || sha_rows
            .keys()
            .copied()
            .ne(indices.as_slice_v1().iter().copied())
    {
        return Err(ZkX509PreprocessedFixedErrorV1::Opening);
    }
    let canonical = encode_zk_x509_preprocessed_fixed_proof_v1(profiles, &proof)?;
    if canonical != encoded {
        return Err(ZkX509PreprocessedFixedErrorV1::MalformedProof);
    }
    Ok(canonical)
}

/// Obtain one canonical two-oracle X5F1 sidecar from an untrusted
/// release-preprocessed opening service.
///
/// This is the operational prover path. It performs only bounded decoding and
/// Merkle verification; the ceremony-scale log25 LDE is never recomputed per
/// credential. Direct root/multiproof derivation routines above are retained
/// solely for release generation and independent diagnostics.
pub(crate) fn request_zk_x509_main_preprocessed_fixed_openings_v1(
    sha: ZkX509ShaPreprocessedFixedCertificateV1,
    p256: ZkX509P256Log19PreprocessedFixedCertificateV1,
    indices: &ZkX509Log19PreprocessedFixedOpeningIndicesV1,
    service: &mut impl ZkX509MainPreprocessedFixedOpeningServiceV1,
) -> Result<Vec<u8>, ZkX509PreprocessedFixedErrorV1> {
    let expected_sha = pinned_zk_x509_sha_preprocessed_fixed_certificate_v1()?;
    let expected_p256 = pinned_zk_x509_p256_log19_preprocessed_fixed_certificate_v1()?;
    validate_zk_x509_sha_preprocessed_fixed_certificate_v1(sha, expected_sha)?;
    validate_zk_x509_p256_log19_preprocessed_fixed_certificate_v1(p256, expected_p256)?;
    let profiles = zk_x509_main_preprocessed_fixed_profiles_v1(expected_sha, expected_p256)?;
    request_zk_x509_main_preprocessed_fixed_openings_for_profiles_v1(&profiles, indices, service)
}

fn validate_proof_shape_v1(
    profiles: &[ZkX509PreprocessedFixedProfileV1],
    proof: &ZkX509PreprocessedFixedProofV1,
) -> Result<(), ZkX509PreprocessedFixedErrorV1> {
    validate_profiles_v1(profiles)?;
    if proof.oracles.len() != profiles.len() {
        return Err(ZkX509PreprocessedFixedErrorV1::MalformedProof);
    }
    for (profile, oracle) in profiles.iter().copied().zip(&proof.oracles) {
        let indices = oracle
            .indices
            .iter()
            .copied()
            .map(|index| usize::try_from(index).map_err(|_| ZkX509PreprocessedFixedErrorV1::Index))
            .collect::<Result<Vec<_>, _>>()?;
        canonical_indices_v1(profile.geometry, &indices)?;
        if oracle.oracle != profile.geometry.oracle
            || oracle.rows.len() != indices.len()
            || oracle
                .rows
                .iter()
                .any(|row| row.len() != usize::from(profile.geometry.width))
            || oracle.frontier.len()
                != multiproof_frontier_len_v1(profile.geometry.lde_rows()?, &indices)
                    .map_err(map_aggregate_error_v1)?
        {
            return Err(ZkX509PreprocessedFixedErrorV1::MalformedProof);
        }
        for row in &oracle.rows {
            fixed_leaf_hash_v1(profile.geometry, row)?;
        }
    }
    Ok(())
}

fn checked_wire_len_v1(
    profiles: &[ZkX509PreprocessedFixedProfileV1],
    proof: &ZkX509PreprocessedFixedProofV1,
) -> Result<usize, ZkX509PreprocessedFixedErrorV1> {
    let mut length = 8_usize;
    for (profile, oracle) in profiles.iter().zip(&proof.oracles) {
        let opening_bytes = usize::from(profile.geometry.width)
            .checked_mul(8)
            .and_then(|bytes| bytes.checked_add(4))
            .and_then(|bytes| bytes.checked_mul(oracle.indices.len()))
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        let frontier_bytes = oracle
            .frontier
            .len()
            .checked_mul(32)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        length = length
            .checked_add(4)
            .and_then(|length| length.checked_add(opening_bytes))
            .and_then(|length| length.checked_add(frontier_bytes))
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    }
    if length > ZK_X509_PREPROCESSED_FIXED_MAX_WIRE_BYTES_V1 {
        return Err(ZkX509PreprocessedFixedErrorV1::Resource);
    }
    Ok(length)
}

/// Encode the sole canonical bounded fixed-oracle sidecar.
pub(crate) fn encode_zk_x509_preprocessed_fixed_proof_v1(
    profiles: &[ZkX509PreprocessedFixedProfileV1],
    proof: &ZkX509PreprocessedFixedProofV1,
) -> Result<Vec<u8>, ZkX509PreprocessedFixedErrorV1> {
    validate_proof_shape_v1(profiles, proof)?;
    let expected_length = checked_wire_len_v1(profiles, proof)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected_length)
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
    bytes.extend_from_slice(&ZK_X509_PREPROCESSED_FIXED_MAGIC_V1);
    append_u16_v1(&mut bytes, ZK_X509_PREPROCESSED_FIXED_VERSION_V1);
    append_u16_v1(
        &mut bytes,
        u16::try_from(proof.oracles.len())
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::MalformedProof)?,
    );
    for oracle in &proof.oracles {
        append_u16_v1(&mut bytes, oracle.oracle);
        append_u16_v1(
            &mut bytes,
            u16::try_from(oracle.indices.len())
                .map_err(|_| ZkX509PreprocessedFixedErrorV1::MalformedProof)?,
        );
        for (index, row) in oracle.indices.iter().copied().zip(&oracle.rows) {
            append_u32_v1(&mut bytes, index);
            for value in row {
                append_u64_v1(&mut bytes, *value);
            }
        }
        for hash in &oracle.frontier {
            bytes.extend_from_slice(hash);
        }
    }
    if bytes.len() != expected_length {
        return Err(ZkX509PreprocessedFixedErrorV1::MalformedProof);
    }
    Ok(bytes)
}

/// Decode exactly one fixed-oracle sidecar with no alternate counts or suffix.
pub(crate) fn decode_zk_x509_preprocessed_fixed_proof_v1(
    profiles: &[ZkX509PreprocessedFixedProfileV1],
    encoded: &[u8],
) -> Result<ZkX509PreprocessedFixedProofV1, ZkX509PreprocessedFixedErrorV1> {
    validate_profiles_v1(profiles)?;
    if encoded.is_empty() || encoded.len() > ZK_X509_PREPROCESSED_FIXED_MAX_WIRE_BYTES_V1 {
        return Err(ZkX509PreprocessedFixedErrorV1::MalformedProof);
    }
    let mut reader = ExactProofReaderV1::new(encoded);
    if reader.take::<4>().map_err(map_reader_error_v1)? != ZK_X509_PREPROCESSED_FIXED_MAGIC_V1
        || reader.u16().map_err(map_reader_error_v1)? != ZK_X509_PREPROCESSED_FIXED_VERSION_V1
        || usize::from(reader.u16().map_err(map_reader_error_v1)?) != profiles.len()
    {
        return Err(ZkX509PreprocessedFixedErrorV1::MalformedProof);
    }
    let mut oracles = Vec::new();
    oracles
        .try_reserve_exact(profiles.len())
        .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
    for profile in profiles.iter().copied() {
        let oracle = reader.u16().map_err(map_reader_error_v1)?;
        let opening_count = usize::from(reader.u16().map_err(map_reader_error_v1)?);
        if oracle != profile.geometry.oracle
            || opening_count == 0
            || opening_count > ZK_X509_PREPROCESSED_FIXED_MAX_OPENINGS_V1
        {
            return Err(ZkX509PreprocessedFixedErrorV1::MalformedProof);
        }
        let mut indices = Vec::new();
        let mut rows = Vec::new();
        indices
            .try_reserve_exact(opening_count)
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        rows.try_reserve_exact(opening_count)
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        for _ in 0..opening_count {
            indices.push(reader.u32().map_err(map_reader_error_v1)?);
            let mut row = Vec::new();
            row.try_reserve_exact(usize::from(profile.geometry.width))
                .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
            for _ in 0..profile.geometry.width {
                row.push(reader.field().map_err(map_reader_error_v1)?.0);
            }
            rows.push(row);
        }
        let native_indices = indices
            .iter()
            .copied()
            .map(|index| usize::try_from(index).map_err(|_| ZkX509PreprocessedFixedErrorV1::Index))
            .collect::<Result<Vec<_>, _>>()?;
        canonical_indices_v1(profile.geometry, &native_indices)?;
        let frontier_count =
            multiproof_frontier_len_v1(profile.geometry.lde_rows()?, &native_indices)
                .map_err(map_aggregate_error_v1)?;
        let mut frontier = Vec::new();
        frontier
            .try_reserve_exact(frontier_count)
            .map_err(|_| ZkX509PreprocessedFixedErrorV1::Resource)?;
        for _ in 0..frontier_count {
            frontier.push(reader.take::<32>().map_err(map_reader_error_v1)?);
        }
        oracles.push(ZkX509PreprocessedFixedMultiproofV1 {
            oracle,
            indices,
            rows,
            frontier,
        });
    }
    reader.finish().map_err(map_reader_error_v1)?;
    let proof = ZkX509PreprocessedFixedProofV1 { oracles };
    validate_proof_shape_v1(profiles, &proof)?;
    Ok(proof)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::transparent_stark::{
        GOLDILOCKS_MODULUS_V1, Sha256MerkleTreeV1, goldilocks_evaluate_coset_v1,
    };
    use crate::privacy_engines::zk_x509::sha_call_bus_stark::ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1;

    const TEST_GEOMETRY: ZkX509PreprocessedFixedGeometryV1 = ZkX509PreprocessedFixedGeometryV1 {
        oracle: 7,
        native_log2: 4,
        lde_log2: 7,
        width: 3,
    };
    const TEST_BATCH8_GEOMETRY: ZkX509PreprocessedFixedGeometryV1 =
        ZkX509PreprocessedFixedGeometryV1 {
            oracle: 8,
            native_log2: 4,
            lde_log2: 7,
            width: 8,
        };
    const TEST_PARTIAL_BATCH_GEOMETRY: ZkX509PreprocessedFixedGeometryV1 =
        ZkX509PreprocessedFixedGeometryV1 {
            oracle: 9,
            native_log2: 4,
            lde_log2: 7,
            width: 12,
        };

    fn test_native_column(column: usize) -> Result<Vec<F>, ZkX509PreprocessedFixedErrorV1> {
        if column >= usize::from(TEST_GEOMETRY.width) {
            return Err(ZkX509PreprocessedFixedErrorV1::Column);
        }
        Ok((0..1_usize << TEST_GEOMETRY.native_log2)
            .map(|row| F(u64::try_from(1 + row * 5 + column * 17).expect("small fixture")))
            .collect())
    }

    fn test_batch8_native_column(column: usize) -> Result<Vec<F>, ZkX509PreprocessedFixedErrorV1> {
        if column >= usize::from(TEST_BATCH8_GEOMETRY.width) {
            return Err(ZkX509PreprocessedFixedErrorV1::Column);
        }
        Ok((0..1_usize << TEST_BATCH8_GEOMETRY.native_log2)
            .map(|row| {
                F(
                    u64::try_from(3 + row * (column + 5) + row * row * (column + 1) + column * 19)
                        .expect("small fixture"),
                )
            })
            .collect())
    }

    fn test_partial_batch_native_column(
        column: usize,
    ) -> Result<Vec<F>, ZkX509PreprocessedFixedErrorV1> {
        if column >= usize::from(TEST_PARTIAL_BATCH_GEOMETRY.width) {
            return Err(ZkX509PreprocessedFixedErrorV1::Column);
        }
        Ok((0..1_usize << TEST_PARTIAL_BATCH_GEOMETRY.native_log2)
            .map(|row| {
                F(
                    u64::try_from(3 + row * (column + 5) + row * row * (column + 1) + column * 19)
                        .expect("small fixture"),
                )
            })
            .collect())
    }

    fn materialized_artifact_oracle_v1(
        geometry: ZkX509PreprocessedFixedGeometryV1,
        mut native_column: impl FnMut(usize) -> Result<Vec<F>, ZkX509PreprocessedFixedErrorV1>,
    ) -> (
        ZkX509PreprocessedFixedProfileV1,
        Vec<Vec<u64>>,
        Vec<Vec<[u8; 32]>>,
    ) {
        let trace_root = goldilocks_primitive_root_v1(geometry.native_log2).expect("trace root");
        let lde_root = goldilocks_primitive_root_v1(geometry.lde_log2).expect("LDE root");
        let mut columns = Vec::new();
        for column in 0..usize::from(geometry.width) {
            let mut coefficients = native_column(column).expect("native fixed column");
            goldilocks_ifft_v1(&mut coefficients, trace_root).expect("native IFFT");
            columns.push(
                goldilocks_evaluate_coset_v1(
                    &coefficients,
                    geometry.lde_rows().expect("LDE rows"),
                    lde_root,
                    F(GOLDILOCKS_GENERATOR_V1),
                )
                .expect("generator-coset LDE"),
            );
        }
        let mut rows: Vec<Vec<u64>> = Vec::new();
        for row in 0..geometry.lde_rows().expect("LDE rows") {
            rows.push(
                columns
                    .iter()
                    .map(|column| column[row].0)
                    .collect::<Vec<u64>>(),
            );
        }
        let mut levels = vec![
            rows.iter()
                .map(|row| fixed_leaf_hash_v1(geometry, row).expect("fixed leaf"))
                .collect::<Vec<_>>(),
        ];
        while levels.last().expect("leaf level").len() > 1 {
            let parent = levels
                .last()
                .expect("child level")
                .chunks_exact(2)
                .map(|pair| sha256_merkle_node_v1(FIXED_NODE_DOMAIN_V1, &pair[0], &pair[1]))
                .collect::<Vec<_>>();
            levels.push(parent);
        }
        let root = levels.last().expect("root level")[0];
        (
            ZkX509PreprocessedFixedProfileV1 { geometry, root },
            rows,
            levels,
        )
    }

    fn test_artifact_binding_v1(
        profile: ZkX509PreprocessedFixedProfileV1,
        marker: u8,
    ) -> ZkX509PreprocessedFixedArtifactBindingV1 {
        ZkX509PreprocessedFixedArtifactBindingV1 {
            profile,
            descriptor_digest: [marker; 32],
            certificate: vec![marker; 17 + usize::from(marker & 1)],
        }
    }

    fn encode_test_oracle_artifact_v1(
        compiled_profile_digest: [u8; 32],
        binding: &ZkX509PreprocessedFixedArtifactBindingV1,
        rows: &[Vec<u64>],
        levels: &[Vec<[u8; 32]>],
    ) -> Vec<u8> {
        let geometry = binding.profile.geometry;
        let (row_bytes, tree_bytes, file_bytes) =
            artifact_geometry_lengths_v1(geometry).expect("artifact lengths");
        assert_eq!(rows.len(), geometry.lde_rows().expect("LDE rows"));
        assert!(
            rows.iter()
                .all(|row| row.len() == usize::from(geometry.width))
        );
        assert_eq!(levels.last().expect("root level"), &[binding.profile.root]);

        let mut encoded = vec![0_u8; usize::try_from(file_bytes).expect("small test artifact")];
        let header = &mut encoded[..ZK_X509_PREPROCESSED_FIXED_ARTIFACT_ORACLE_HEADER_BYTES_V1];
        header[..4].copy_from_slice(&ZK_X509_PREPROCESSED_FIXED_ARTIFACT_ORACLE_MAGIC_V1);
        header[4..6].copy_from_slice(&ZK_X509_PREPROCESSED_FIXED_ARTIFACT_VERSION_V1.to_be_bytes());
        header[6..8].copy_from_slice(
            &u16::try_from(ZK_X509_PREPROCESSED_FIXED_ARTIFACT_ORACLE_HEADER_BYTES_V1)
                .expect("small header")
                .to_be_bytes(),
        );
        header[8..10]
            .copy_from_slice(&ZK_X509_PREPROCESSED_FIXED_ARTIFACT_LAYOUT_BATCH8_V1.to_be_bytes());
        header[10..12].copy_from_slice(&geometry.oracle.to_be_bytes());
        header[12] = geometry.native_log2;
        header[13] = geometry.lde_log2;
        header[14..16].copy_from_slice(&geometry.width.to_be_bytes());
        header[16..18].copy_from_slice(
            &u16::try_from(ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
                .expect("small batch")
                .to_be_bytes(),
        );
        header[18..20].copy_from_slice(
            &ZK_X509_PREPROCESSED_FIXED_ARTIFACT_FIELD_CODEC_U64BE_V1.to_be_bytes(),
        );
        header[20..22].copy_from_slice(
            &u16::try_from(binding.certificate.len())
                .expect("small certificate")
                .to_be_bytes(),
        );
        header[24..56].copy_from_slice(&compiled_profile_digest);
        header[56..88].copy_from_slice(&binding.profile.root);
        header[88..120].copy_from_slice(&binding.descriptor_digest);
        header[120..128].copy_from_slice(&row_bytes.to_be_bytes());
        header[128..136].copy_from_slice(&tree_bytes.to_be_bytes());
        header[136..144].copy_from_slice(&file_bytes.to_be_bytes());
        header[144..144 + binding.certificate.len()].copy_from_slice(&binding.certificate);

        let mut cursor = ZK_X509_PREPROCESSED_FIXED_ARTIFACT_ORACLE_HEADER_BYTES_V1;
        for column_start in
            (0..usize::from(geometry.width)).step_by(ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
        {
            let column_end = (column_start + ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
                .min(usize::from(geometry.width));
            for row in rows {
                for value in &row[column_start..column_end] {
                    encoded[cursor..cursor + 8].copy_from_slice(&value.to_be_bytes());
                    cursor += 8;
                }
            }
        }
        assert_eq!(
            cursor,
            ZK_X509_PREPROCESSED_FIXED_ARTIFACT_ORACLE_HEADER_BYTES_V1
                + usize::try_from(row_bytes).expect("small rows")
        );
        for level in levels {
            for node in level {
                encoded[cursor..cursor + 32].copy_from_slice(node);
                cursor += 32;
            }
        }
        assert_eq!(cursor, encoded.len());
        encoded
    }

    fn encode_test_artifact_manifest_v1(
        compiled_profile_digest: [u8; 32],
        bindings: &[ZkX509PreprocessedFixedArtifactBindingV1; 2],
    ) -> [u8; ZK_X509_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_BYTES_V1] {
        let mut manifest = [0_u8; ZK_X509_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_BYTES_V1];
        manifest[..4].copy_from_slice(&ZK_X509_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_MAGIC_V1);
        manifest[4..6]
            .copy_from_slice(&ZK_X509_PREPROCESSED_FIXED_ARTIFACT_VERSION_V1.to_be_bytes());
        manifest[6..8].copy_from_slice(
            &u16::try_from(ZK_X509_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_BYTES_V1)
                .expect("small manifest")
                .to_be_bytes(),
        );
        manifest[8..10]
            .copy_from_slice(&ZK_X509_PREPROCESSED_FIXED_ARTIFACT_LAYOUT_BATCH8_V1.to_be_bytes());
        manifest[10..12].copy_from_slice(&2_u16.to_be_bytes());
        manifest[16..48].copy_from_slice(&compiled_profile_digest);
        for (index, binding) in bindings.iter().enumerate() {
            let start = artifact_manifest_entry_offset_v1(index).expect("entry offset");
            let (_, _, file_bytes) =
                artifact_geometry_lengths_v1(binding.profile.geometry).expect("artifact lengths");
            manifest[start..start + 2]
                .copy_from_slice(&binding.profile.geometry.oracle.to_be_bytes());
            manifest[start + 2..start + 4].copy_from_slice(
                &u16::try_from(binding.certificate.len())
                    .expect("small certificate")
                    .to_be_bytes(),
            );
            manifest[start + 4..start + 12].copy_from_slice(&file_bytes.to_be_bytes());
            manifest[start + 12..start + 44].copy_from_slice(&binding.profile.root);
            manifest[start + 44..start + 76].copy_from_slice(&binding.descriptor_digest);
            manifest[start + 76..start + 76 + binding.certificate.len()]
                .copy_from_slice(&binding.certificate);
        }
        manifest
    }

    struct TestArtifactPackageV1 {
        directory: tempfile::TempDir,
        compiled_profile_digest: [u8; 32],
        bindings: [ZkX509PreprocessedFixedArtifactBindingV1; 2],
        profiles: [ZkX509PreprocessedFixedProfileV1; 2],
        rows: [Vec<Vec<u64>>; 2],
        levels: [Vec<Vec<[u8; 32]>>; 2],
    }

    fn test_artifact_package_v1() -> TestArtifactPackageV1 {
        let compiled_profile_digest = [0xA5; 32];
        let (first_profile, first_rows, first_levels) =
            materialized_artifact_oracle_v1(TEST_GEOMETRY, test_native_column);
        let (second_profile, second_rows, second_levels) = materialized_artifact_oracle_v1(
            TEST_PARTIAL_BATCH_GEOMETRY,
            test_partial_batch_native_column,
        );
        let bindings = [
            test_artifact_binding_v1(first_profile, 0x31),
            test_artifact_binding_v1(second_profile, 0x52),
        ];
        let directory = tempfile::tempdir().expect("artifact temp dir");
        for ((binding, rows), levels) in bindings
            .iter()
            .zip([&first_rows, &second_rows])
            .zip([&first_levels, &second_levels])
        {
            std::fs::write(
                directory
                    .path()
                    .join(artifact_oracle_filename_v1(binding.profile.geometry.oracle)),
                encode_test_oracle_artifact_v1(compiled_profile_digest, binding, rows, levels),
            )
            .expect("write oracle artifact");
        }
        std::fs::write(
            directory
                .path()
                .join(ZK_X509_MAIN_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_FILE_V1),
            encode_test_artifact_manifest_v1(compiled_profile_digest, &bindings),
        )
        .expect("write artifact manifest");
        TestArtifactPackageV1 {
            directory,
            compiled_profile_digest,
            bindings,
            profiles: [first_profile, second_profile],
            rows: [first_rows, second_rows],
            levels: [first_levels, second_levels],
        }
    }

    fn expected_test_artifact_multiproof_v1(
        profile: ZkX509PreprocessedFixedProfileV1,
        rows: &[Vec<u64>],
        levels: &[Vec<[u8; 32]>],
        indices: &[usize],
    ) -> ZkX509PreprocessedFixedMultiproofV1 {
        let mut selected = indices.iter().copied().collect::<BTreeSet<_>>();
        let mut frontier = Vec::new();
        for level in 0..usize::from(profile.geometry.lde_log2) {
            for index in selected.iter().copied() {
                let sibling = index ^ 1;
                if !selected.contains(&sibling) {
                    frontier.push(levels[level][sibling]);
                }
            }
            selected = selected.into_iter().map(|index| index >> 1).collect();
        }
        ZkX509PreprocessedFixedMultiproofV1 {
            oracle: profile.geometry.oracle,
            indices: indices
                .iter()
                .map(|index| u32::try_from(*index).expect("small index"))
                .collect(),
            rows: indices.iter().map(|index| rows[*index].clone()).collect(),
            frontier,
        }
    }

    fn open_test_artifact_backend_v1(
        package: &TestArtifactPackageV1,
    ) -> Result<ZkX509MainPreprocessedFixedArtifactV1, ZkX509PreprocessedFixedErrorV1> {
        ZkX509MainPreprocessedFixedArtifactV1::open_bound_v1(
            package.directory.path(),
            package.compiled_profile_digest,
            package.bindings.clone(),
        )
    }

    #[test]
    fn immutable_artifact_backend_is_byte_exact_and_root_authenticated() {
        let package = test_artifact_package_v1();
        let indices = ZkX509Log19PreprocessedFixedOpeningIndicesV1 {
            indices: vec![1, 3, 7, 19, 63, 125],
        };
        let expected = ZkX509PreprocessedFixedProofV1 {
            oracles: (0..2)
                .map(|oracle| {
                    expected_test_artifact_multiproof_v1(
                        package.profiles[oracle],
                        &package.rows[oracle],
                        &package.levels[oracle],
                        indices.as_slice_v1(),
                    )
                })
                .collect(),
        };
        let expected_encoded =
            encode_zk_x509_preprocessed_fixed_proof_v1(&package.profiles, &expected)
                .expect("materialized proof");
        let mut backend = open_test_artifact_backend_v1(&package).expect("immutable artifact");
        let encoded = request_zk_x509_main_preprocessed_fixed_openings_for_profiles_v1(
            &package.profiles,
            &indices,
            &mut backend,
        )
        .expect("authenticated random-access proof");
        assert_eq!(encoded, expected_encoded);
        assert_eq!(
            decode_zk_x509_preprocessed_fixed_proof_v1(&package.profiles, &encoded)
                .expect("artifact proof"),
            expected
        );
    }

    #[test]
    fn artifact_manifest_header_partial_install_and_length_mutations_fail_closed() {
        let manifest_mutations = [
            0_usize, 5, 9, 11, 12, 16, 49, 55, 60, 92, 124, 209, 220, 252, 284, 368,
        ];
        for offset in manifest_mutations {
            let package = test_artifact_package_v1();
            let path = package
                .directory
                .path()
                .join(ZK_X509_MAIN_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_FILE_V1);
            let mut encoded = std::fs::read(&path).expect("manifest");
            encoded[offset] ^= 1;
            std::fs::write(&path, encoded).expect("mutated manifest");
            assert!(
                open_test_artifact_backend_v1(&package).is_err(),
                "manifest mutation at byte {offset} must reject"
            );
        }

        let header_mutations = [
            0_usize, 5, 9, 11, 12, 13, 15, 17, 19, 21, 22, 24, 56, 88, 120, 128, 136, 144, 230,
        ];
        for offset in header_mutations {
            let package = test_artifact_package_v1();
            let path = package.directory.path().join(artifact_oracle_filename_v1(
                package.profiles[0].geometry.oracle,
            ));
            let mut encoded = std::fs::read(&path).expect("oracle");
            encoded[offset] ^= 1;
            std::fs::write(&path, encoded).expect("mutated oracle");
            assert!(
                open_test_artifact_backend_v1(&package).is_err(),
                "oracle-header mutation at byte {offset} must reject"
            );
        }

        for truncate in [true, false] {
            let package = test_artifact_package_v1();
            let path = package
                .directory
                .path()
                .join(ZK_X509_MAIN_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_FILE_V1);
            let mut encoded = std::fs::read(&path).expect("manifest");
            if truncate {
                encoded.pop();
            } else {
                encoded.push(0);
            }
            std::fs::write(&path, encoded).expect("wrong-length manifest");
            assert!(open_test_artifact_backend_v1(&package).is_err());
        }
        for truncate in [true, false] {
            let package = test_artifact_package_v1();
            let path = package.directory.path().join(artifact_oracle_filename_v1(
                package.profiles[0].geometry.oracle,
            ));
            let mut encoded = std::fs::read(&path).expect("oracle");
            if truncate {
                encoded.pop();
            } else {
                encoded.push(0);
            }
            std::fs::write(&path, encoded).expect("wrong-length oracle");
            assert!(open_test_artifact_backend_v1(&package).is_err());
        }

        let missing_manifest = test_artifact_package_v1();
        std::fs::remove_file(
            missing_manifest
                .directory
                .path()
                .join(ZK_X509_MAIN_PREPROCESSED_FIXED_ARTIFACT_MANIFEST_FILE_V1),
        )
        .expect("remove manifest");
        assert!(open_test_artifact_backend_v1(&missing_manifest).is_err());

        let missing_oracle = test_artifact_package_v1();
        std::fs::remove_file(
            missing_oracle
                .directory
                .path()
                .join(artifact_oracle_filename_v1(
                    missing_oracle.profiles[1].geometry.oracle,
                )),
        )
        .expect("remove oracle");
        assert!(open_test_artifact_backend_v1(&missing_oracle).is_err());

        let swapped_oracle = test_artifact_package_v1();
        let first = swapped_oracle
            .directory
            .path()
            .join(artifact_oracle_filename_v1(
                swapped_oracle.profiles[0].geometry.oracle,
            ));
        let second = swapped_oracle
            .directory
            .path()
            .join(artifact_oracle_filename_v1(
                swapped_oracle.profiles[1].geometry.oracle,
            ));
        std::fs::write(&first, std::fs::read(&second).expect("second oracle"))
            .expect("swap oracle bytes");
        assert!(open_test_artifact_backend_v1(&swapped_oracle).is_err());
    }

    #[test]
    fn opened_artifact_detects_concurrent_row_tree_header_root_and_truncation_corruption() {
        let indices = ZkX509Log19PreprocessedFixedOpeningIndicesV1 {
            indices: vec![1, 3, 7, 19, 63, 125],
        };
        let corrupt_and_reject = |oracle: usize, offset: usize, truncate: bool| {
            let package = test_artifact_package_v1();
            let mut backend = open_test_artifact_backend_v1(&package).expect("artifact");
            let path = package.directory.path().join(artifact_oracle_filename_v1(
                package.profiles[oracle].geometry.oracle,
            ));
            if truncate {
                let file = OpenOptions::new()
                    .write(true)
                    .open(&path)
                    .expect("writable test artifact");
                let length = file.metadata().expect("metadata").len();
                file.set_len(length - 1).expect("truncate test artifact");
            } else {
                let mut encoded = std::fs::read(&path).expect("oracle");
                encoded[offset] ^= 1;
                std::fs::write(&path, encoded).expect("corrupt oracle");
            }
            assert!(
                request_zk_x509_main_preprocessed_fixed_openings_for_profiles_v1(
                    &package.profiles,
                    &indices,
                    &mut backend,
                )
                .is_err(),
                "corrupted artifact must return no sidecar"
            );
        };

        // Header mutation is detected against the immutable opened header.
        corrupt_and_reject(0, 0, false);
        // The first selected row is in the sole partial batch of oracle zero.
        corrupt_and_reject(
            0,
            ZK_X509_PREPROCESSED_FIXED_ARTIFACT_ORACLE_HEADER_BYTES_V1 + 3 * 8,
            false,
        );
        let (first_row_bytes, _, first_file_bytes) =
            artifact_geometry_lengths_v1(TEST_GEOMETRY).expect("artifact lengths");
        let first_tree = ZK_X509_PREPROCESSED_FIXED_ARTIFACT_ORACLE_HEADER_BYTES_V1
            + usize::try_from(first_row_bytes).expect("small rows");
        // Leaf zero is the canonical frontier sibling of selected leaf one.
        corrupt_and_reject(0, first_tree, false);
        // The stored root is checked on every request, even though it is not
        // transmitted in a canonical multiproof frontier.
        corrupt_and_reject(
            0,
            usize::try_from(first_file_bytes - 32).expect("small root offset"),
            false,
        );
        corrupt_and_reject(1, 0, true);

        let package = test_artifact_package_v1();
        let mut backend = open_test_artifact_backend_v1(&package).expect("artifact");
        let noncanonical = ZkX509Log19PreprocessedFixedOpeningIndicesV1 {
            indices: vec![3, 1],
        };
        assert!(
            backend
                .open_main_v1(&package.profiles, &noncanonical)
                .is_err(),
            "unsorted or ambiguous artifact query keys must reject"
        );
    }

    fn lowercase_hex_v1(bytes: &[u8]) -> String {
        use core::fmt::Write as _;

        let mut encoded = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
        }
        encoded
    }

    #[test]
    fn sha256_compression_kat_and_compact_leaf_state_match_canonical_sha2() {
        let mut raw_state = SHA256_INITIAL_STATE_V1;
        let mut block = [0_u8; 64];
        block[..3].copy_from_slice(b"abc");
        block[3] = 0x80;
        block[56..].copy_from_slice(&24_u64.to_be_bytes());
        compress_sha256_block_v1(&mut raw_state, &block);
        let mut raw_digest = [0_u8; 32];
        for (target, word) in raw_digest.chunks_exact_mut(4).zip(raw_state) {
            target.copy_from_slice(&word.to_be_bytes());
        }
        assert_eq!(raw_digest, <[u8; 32]>::from(Sha256::digest(b"abc")));

        let row: [F; 8] =
            core::array::from_fn(|index| F(u64::try_from(index * index + 11).expect("small row")));
        let mut compact = CompactFixedLeafSha256V1::from_geometry_v1(TEST_BATCH8_GEOMETRY)
            .expect("compact prefix");
        assert_eq!(
            compact.finalize_v1(TEST_BATCH8_GEOMETRY, None),
            Err(ZkX509PreprocessedFixedErrorV1::Profile),
            "omitting the sole batch must not produce a leaf"
        );
        compact
            .absorb_batch8_v1(row)
            .expect("sole complete field batch");
        let compact_digest = compact
            .finalize_v1(TEST_BATCH8_GEOMETRY, None)
            .expect("compact digest");
        let encoded = row.map(|value| value.0);
        assert_eq!(
            compact_digest,
            fixed_leaf_hash_v1(TEST_BATCH8_GEOMETRY, &encoded).expect("canonical leaf")
        );

        let fields: [F; 12] =
            core::array::from_fn(|index| F(u64::try_from(index * 13 + 7).expect("small field")));
        let mut complete = [F::ZERO; 8];
        complete.copy_from_slice(&fields[..8]);
        let mut partial = [F::ZERO; 8];
        partial[..4].copy_from_slice(&fields[8..]);
        let mut compact = CompactFixedLeafSha256V1::from_geometry_v1(TEST_PARTIAL_BATCH_GEOMETRY)
            .expect("partial compact prefix");
        compact
            .absorb_batch8_v1(complete)
            .expect("complete prefix batch");
        assert_eq!(
            compact
                .finalize_v1(TEST_PARTIAL_BATCH_GEOMETRY, Some((partial, 4)))
                .expect("partial compact digest"),
            fixed_leaf_hash_v1(TEST_PARTIAL_BATCH_GEOMETRY, &fields.map(|value| value.0),)
                .expect("canonical partial leaf")
        );
        partial[7] = F::ONE;
        assert_eq!(
            compact.finalize_v1(TEST_PARTIAL_BATCH_GEOMETRY, Some((partial, 4))),
            Err(ZkX509PreprocessedFixedErrorV1::Profile),
            "ignored lanes in a partial transform must remain canonical zero"
        );
    }

    #[test]
    fn row_major_batch8_lde_is_lane_exact_and_order_sensitive() {
        let native =
            materialize_native_batch8_v1(TEST_BATCH8_GEOMETRY, 0, &mut test_batch8_native_column)
                .expect("row-major native batch");
        let batched = checked_goldilocks_evaluate_coset_batch8_v1(native, TEST_BATCH8_GEOMETRY)
            .expect("row-major LDE");
        let trace_root =
            goldilocks_primitive_root_v1(TEST_BATCH8_GEOMETRY.native_log2).expect("trace root");
        let lde_root =
            goldilocks_primitive_root_v1(TEST_BATCH8_GEOMETRY.lde_log2).expect("LDE root");
        for lane in 0..8 {
            let mut coefficients = test_batch8_native_column(lane).expect("native lane");
            goldilocks_ifft_v1(&mut coefficients, trace_root).expect("scalar IFFT");
            let expected = goldilocks_evaluate_coset_v1(
                &coefficients,
                1_usize << TEST_BATCH8_GEOMETRY.lde_log2,
                lde_root,
                F(GOLDILOCKS_GENERATOR_V1),
            )
            .expect("scalar LDE");
            assert_eq!(
                batched.iter().map(|row| row[lane]).collect::<Vec<_>>(),
                expected,
                "lane {lane}"
            );
        }
        let mut reordered = batched.clone();
        for row in &mut reordered {
            row.swap(0, 1);
        }
        assert_ne!(reordered, batched);
        assert_eq!(
            reordered.iter().map(|row| row[0]).collect::<Vec<_>>(),
            batched.iter().map(|row| row[1]).collect::<Vec<_>>()
        );
    }

    #[test]
    fn compact_streaming_root_matches_materialized_oracle_and_rejects_reordering() {
        let expected = recompute_zk_x509_preprocessed_fixed_root_v1(
            TEST_BATCH8_GEOMETRY,
            test_batch8_native_column,
        )
        .expect("materialized root");
        let streamed =
            stream_fixed_oracle_root_batch8_v1(TEST_BATCH8_GEOMETRY, test_batch8_native_column)
                .expect("streamed root");
        assert_eq!(streamed, expected);

        let reversed = |column: usize| test_batch8_native_column(7 - column);
        let expected_reversed =
            recompute_zk_x509_preprocessed_fixed_root_v1(TEST_BATCH8_GEOMETRY, reversed)
                .expect("materialized reordered root");
        let streamed_reversed = stream_fixed_oracle_root_batch8_v1(TEST_BATCH8_GEOMETRY, reversed)
            .expect("streamed reordered root");
        assert_eq!(streamed_reversed, expected_reversed);
        assert_ne!(streamed_reversed, streamed);

        let expected_partial = recompute_zk_x509_preprocessed_fixed_root_v1(
            TEST_PARTIAL_BATCH_GEOMETRY,
            test_partial_batch_native_column,
        )
        .expect("materialized partial-batch root");
        let mut requested = Vec::new();
        let streamed_partial =
            stream_fixed_oracle_root_batch8_v1(TEST_PARTIAL_BATCH_GEOMETRY, |column| {
                requested.push(column);
                test_partial_batch_native_column(column)
            })
            .expect("streamed partial-batch root");
        assert_eq!(streamed_partial, expected_partial);
        assert_eq!(
            requested,
            (0..usize::from(TEST_PARTIAL_BATCH_GEOMETRY.width)).collect::<Vec<_>>(),
            "partial final batch requested a synthetic padding column",
        );
    }

    #[test]
    fn compact_batch8_multiproof_is_byte_exact_with_materialized_construction() {
        let indices = vec![1, 3, 7, 19, 63, 126];
        let materialized =
            stream_fixed_oracle_v1(TEST_BATCH8_GEOMETRY, &indices, test_batch8_native_column)
                .expect("materialized multiproof");
        let compact = stream_fixed_oracle_multiproof_batch8_v1(
            TEST_BATCH8_GEOMETRY,
            &indices,
            test_batch8_native_column,
        )
        .expect("compact multiproof");
        assert_eq!(compact, materialized);

        let profile = ZkX509PreprocessedFixedProfileV1 {
            geometry: TEST_BATCH8_GEOMETRY,
            root: materialized.0,
        };
        let proof = build_zk_x509_preprocessed_fixed_multiproof_batch8_v1(
            profile,
            &indices,
            test_batch8_native_column,
        )
        .expect("compact proof");
        let opened = verify_zk_x509_preprocessed_fixed_multiproof_v1(profile, &indices, &proof)
            .expect("compact proof verifies");
        assert_eq!(opened.len(), indices.len());
        assert_eq!(
            proof.rows, materialized.1,
            "selected rows retain exact column order"
        );
        assert_eq!(
            proof.frontier, materialized.2,
            "selected collector retains canonical level/index order"
        );
    }

    #[test]
    fn compact_batch8_multiproof_rejects_index_row_frontier_and_cache_adversaries() {
        let indices = vec![1, 3, 7, 19, 63, 126];
        let root =
            stream_fixed_oracle_root_batch8_v1(TEST_BATCH8_GEOMETRY, test_batch8_native_column)
                .expect("canonical root");
        let profile = ZkX509PreprocessedFixedProfileV1 {
            geometry: TEST_BATCH8_GEOMETRY,
            root,
        };
        for invalid in [
            vec![1, 1],
            vec![3, 1],
            vec![1, 1_usize << TEST_BATCH8_GEOMETRY.lde_log2],
        ] {
            assert_eq!(
                build_zk_x509_preprocessed_fixed_multiproof_batch8_v1(
                    profile,
                    &invalid,
                    |_| panic!("invalid indices must reject before reading cached columns"),
                ),
                Err(ZkX509PreprocessedFixedErrorV1::Index)
            );
        }
        let proof = build_zk_x509_preprocessed_fixed_multiproof_batch8_v1(
            profile,
            &indices,
            test_batch8_native_column,
        )
        .expect("canonical compact proof");

        let mut changed = proof.clone();
        changed.indices[1] = changed.indices[0];
        assert_eq!(
            verify_zk_x509_preprocessed_fixed_multiproof_v1(profile, &indices, &changed),
            Err(ZkX509PreprocessedFixedErrorV1::Index)
        );

        changed = proof.clone();
        changed.indices.swap(0, 1);
        changed.rows.swap(0, 1);
        assert_eq!(
            verify_zk_x509_preprocessed_fixed_multiproof_v1(profile, &indices, &changed),
            Err(ZkX509PreprocessedFixedErrorV1::Index)
        );

        changed = proof.clone();
        *changed.indices.last_mut().expect("last index") =
            u32::try_from(1_usize << TEST_BATCH8_GEOMETRY.lde_log2).expect("small domain");
        assert_eq!(
            verify_zk_x509_preprocessed_fixed_multiproof_v1(profile, &indices, &changed),
            Err(ZkX509PreprocessedFixedErrorV1::Index)
        );

        changed = proof.clone();
        changed.rows[0][0] = GOLDILOCKS_MODULUS_V1;
        assert_eq!(
            verify_zk_x509_preprocessed_fixed_multiproof_v1(profile, &indices, &changed),
            Err(ZkX509PreprocessedFixedErrorV1::Opening),
            "a non-canonical field row must not wrap into the field"
        );

        changed = proof.clone();
        changed.rows[0].push(0);
        assert_eq!(
            verify_zk_x509_preprocessed_fixed_multiproof_v1(profile, &indices, &changed),
            Err(ZkX509PreprocessedFixedErrorV1::Opening),
            "an over-width row must not be truncated"
        );

        changed = proof.clone();
        changed.frontier[0][0] ^= 1;
        assert_eq!(
            verify_zk_x509_preprocessed_fixed_multiproof_v1(profile, &indices, &changed),
            Err(ZkX509PreprocessedFixedErrorV1::Opening)
        );

        let corrupted_cache = |column: usize| {
            let mut values = test_batch8_native_column(column)?;
            if column == 3 {
                values[5] = values[5].add(F::ONE);
            }
            Ok(values)
        };
        assert_eq!(
            build_zk_x509_preprocessed_fixed_multiproof_batch8_v1(
                profile,
                &indices,
                corrupted_cache,
            ),
            Err(ZkX509PreprocessedFixedErrorV1::RootMismatch),
            "corrupted cached source material must not survive the pinned-root check"
        );
        assert_eq!(
            build_zk_x509_preprocessed_fixed_multiproof_batch8_v1(profile, &indices, |_| Ok(
                vec![F::ZERO; (1_usize << TEST_BATCH8_GEOMETRY.native_log2) - 1]
            ),),
            Err(ZkX509PreprocessedFixedErrorV1::Column)
        );
    }

    #[test]
    fn sha_query_coordinates_are_strictly_validated_before_openings_exist() {
        let lde_rows = 1_usize << ZK_X509_MAIN_COMMON_LDE_LOG2_V1;
        let queries = (0..usize::from(ZK_X509_FRI_QUERY_COUNT_V1)).collect::<Vec<_>>();
        let derived = derive_zk_x509_log19_preprocessed_fixed_opening_indices_v1(&queries)
            .expect("distinct in-range transcript coordinates");
        assert_eq!(
            derived.as_slice_v1().len(),
            ZK_X509_PREPROCESSED_FIXED_MAX_OPENINGS_V1
        );
        assert!(
            derived
                .as_slice_v1()
                .windows(2)
                .all(|pair| pair[0] < pair[1])
        );

        let mut reordered = queries.clone();
        reordered.reverse();
        assert_eq!(
            derive_zk_x509_log19_preprocessed_fixed_opening_indices_v1(&reordered)
                .expect("transcript order is normalized"),
            derived
        );

        let mut boundary = queries.clone();
        boundary[0] = lde_rows - 1;
        let boundary_openings =
            derive_zk_x509_log19_preprocessed_fixed_opening_indices_v1(&boundary)
                .expect("last valid query coordinate");
        assert!(boundary_openings.as_slice_v1().contains(&(lde_rows - 1)));
        assert!(
            boundary_openings.as_slice_v1().contains(&63),
            "the verifier-derived stride wraps on the common domain"
        );

        let mut duplicate = queries.clone();
        duplicate[1] = duplicate[0];
        assert_eq!(
            derive_zk_x509_log19_preprocessed_fixed_opening_indices_v1(&duplicate),
            Err(ZkX509PreprocessedFixedErrorV1::Index)
        );

        let mut out_of_range = queries.clone();
        out_of_range[0] = lde_rows;
        assert_eq!(
            derive_zk_x509_log19_preprocessed_fixed_opening_indices_v1(&out_of_range),
            Err(ZkX509PreprocessedFixedErrorV1::Index)
        );
        assert_eq!(
            derive_zk_x509_log19_preprocessed_fixed_opening_indices_v1(
                &queries[..queries.len() - 1]
            ),
            Err(ZkX509PreprocessedFixedErrorV1::Index)
        );
    }

    fn exact_maximum_paired_indices_v1() -> Vec<usize> {
        // At level 18 the log25 tree has 128 subtrees. Leave the following
        // twelve vertices unmatched, one in each of twelve distinct binary
        // sibling pairs, and match every remaining adjacent pair. A query
        // placed 64 leaves before each matched boundary opens exactly one leaf
        // in each subtree because its verifier-derived next coordinate is
        // query + 64.
        let unmatched_level18 = [0, 11, 22, 33, 44, 55, 66, 77, 88, 99, 110, 119];
        let level18_subtree_rows = 1_usize << 18;
        let mut maximum_queries = Vec::new();
        let mut level18 = 0_usize;
        while level18 < 128 {
            if unmatched_level18.contains(&level18) {
                level18 += 1;
                continue;
            }
            assert!(
                level18 + 1 < 128 && !unmatched_level18.contains(&(level18 + 1)),
                "unmatched vertices must leave even adjacent paths"
            );
            maximum_queries.push(
                level18 * level18_subtree_rows + level18_subtree_rows
                    - (1_usize
                        << (ZK_X509_MAIN_COMMON_LDE_LOG2_V1 - ZK_X509_MAX_NATIVE_TRACE_LOG2_V1)),
            );
            level18 += 2;
        }
        assert_eq!(
            maximum_queries.len(),
            usize::from(ZK_X509_FRI_QUERY_COUNT_V1)
        );
        derive_zk_x509_log19_preprocessed_fixed_opening_indices_v1(&maximum_queries)
            .expect("legal paired maximum witness")
            .as_slice_v1()
            .to_vec()
    }

    #[test]
    fn exact_sha_x5f1_maximum_is_383196_bytes() {
        let leaves = 1_usize << ZK_X509_MAIN_COMMON_LDE_LOG2_V1;
        let frontier =
            maximum_multiproof_frontier_len_v1(leaves, ZK_X509_PREPROCESSED_FIXED_MAX_OPENINGS_V1)
                .expect("exact maximum frontier");
        assert_eq!(
            frontier,
            ZK_X509_SHA_PREPROCESSED_FIXED_MAXIMUM_FRONTIER_HASHES_V1
        );
        let calculated = 8
            + 4
            + ZK_X509_PREPROCESSED_FIXED_MAX_OPENINGS_V1
                * (4 + ZK_X509_SHA_PREPROCESSED_FIXED_WIDTH_V1 * 8)
            + frontier * 32;
        assert_eq!(calculated, 383_196);
        assert_eq!(
            zk_x509_sha_preprocessed_fixed_maximum_encoded_bytes_v1()
                .expect("checked exact X5F1 maximum"),
            ZK_X509_SHA_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1
        );
        assert!(
            calculated < ZK_X509_PREPROCESSED_FIXED_MAX_WIRE_BYTES_V1,
            "the exact one-oracle maximum remains below the generic decoder cap"
        );

        let maximum_indices = exact_maximum_paired_indices_v1();
        assert_eq!(
            maximum_indices.len(),
            ZK_X509_PREPROCESSED_FIXED_MAX_OPENINGS_V1
        );

        let mut current = maximum_indices.iter().copied().collect::<BTreeSet<_>>();
        let mut per_level_frontier = Vec::new();
        for _ in 0..usize::from(ZK_X509_MAIN_COMMON_LDE_LOG2_V1) {
            per_level_frontier.push(
                current
                    .iter()
                    .filter(|index| !current.contains(&(**index ^ 1)))
                    .count(),
            );
            current = current.into_iter().map(|index| index >> 1).collect();
        }
        assert_eq!(&per_level_frontier[..18], &[116; 18]);
        assert_eq!(per_level_frontier[18], 12);
        assert!(per_level_frontier[19..].iter().all(|count| *count == 0));
        assert_eq!(per_level_frontier.iter().sum::<usize>(), frontier);
        assert_eq!(current, BTreeSet::from([0]));
        assert_eq!(
            multiproof_frontier_len_v1(leaves, &maximum_indices)
                .expect("paired maximum-frontier fixture"),
            frontier
        );
        let profile = ZkX509PreprocessedFixedProfileV1 {
            geometry: ZK_X509_SHA_PREPROCESSED_FIXED_GEOMETRY_V1,
            root: [1; 32],
        };
        let proof = ZkX509PreprocessedFixedProofV1 {
            oracles: vec![ZkX509PreprocessedFixedMultiproofV1 {
                oracle: profile.geometry.oracle,
                indices: maximum_indices
                    .iter()
                    .copied()
                    .map(|index| u32::try_from(index).expect("log25 index"))
                    .collect(),
                rows: vec![
                    vec![0; ZK_X509_SHA_PREPROCESSED_FIXED_WIDTH_V1];
                    ZK_X509_PREPROCESSED_FIXED_MAX_OPENINGS_V1
                ],
                frontier: vec![[0; 32]; frontier],
            }],
        };
        assert_eq!(
            checked_wire_len_v1(&[profile], &proof).expect("maximum checked wire length"),
            calculated
        );
        let encoded =
            encode_zk_x509_preprocessed_fixed_proof_v1(&[profile], &proof).expect("maximum encode");
        assert_eq!(encoded.len(), calculated);
        assert_eq!(
            decode_zk_x509_preprocessed_fixed_proof_v1(&[profile], &encoded)
                .expect("maximum decode"),
            proof
        );
    }

    #[test]
    fn exact_two_oracle_main_x5f1_maximum_is_825776_bytes() {
        let indices = exact_maximum_paired_indices_v1();
        let frontier =
            multiproof_frontier_len_v1(1_usize << ZK_X509_MAIN_COMMON_LDE_LOG2_V1, &indices)
                .expect("paired maximum frontier");
        assert_eq!(
            frontier,
            ZK_X509_SHA_PREPROCESSED_FIXED_MAXIMUM_FRONTIER_HASHES_V1
        );
        let profiles = [
            ZkX509PreprocessedFixedProfileV1 {
                geometry: ZK_X509_SHA_PREPROCESSED_FIXED_GEOMETRY_V1,
                root: [1; 32],
            },
            ZkX509PreprocessedFixedProfileV1 {
                geometry: ZK_X509_P256_LOG19_PREPROCESSED_FIXED_GEOMETRY_V1,
                root: [2; 32],
            },
        ];
        let encoded_indices = indices
            .iter()
            .copied()
            .map(|index| u32::try_from(index).expect("log25 index"))
            .collect::<Vec<_>>();
        let proof = ZkX509PreprocessedFixedProofV1 {
            oracles: vec![
                ZkX509PreprocessedFixedMultiproofV1 {
                    oracle: ZK_X509_SHA_PREPROCESSED_FIXED_ORACLE_V1,
                    indices: encoded_indices.clone(),
                    rows: vec![
                        vec![0; ZK_X509_SHA_PREPROCESSED_FIXED_WIDTH_V1];
                        ZK_X509_PREPROCESSED_FIXED_MAX_OPENINGS_V1
                    ],
                    frontier: vec![[0; 32]; frontier],
                },
                ZkX509PreprocessedFixedMultiproofV1 {
                    oracle: ZK_X509_P256_LOG19_PREPROCESSED_FIXED_ORACLE_V1,
                    indices: encoded_indices,
                    rows: vec![
                        vec![0; ZK_X509_P256_LOG19_PREPROCESSED_FIXED_WIDTH_V1];
                        ZK_X509_PREPROCESSED_FIXED_MAX_OPENINGS_V1
                    ],
                    frontier: vec![[0; 32]; frontier],
                },
            ],
        };
        assert_eq!(
            zk_x509_p256_log19_preprocessed_fixed_maximum_encoded_bytes_v1()
                .expect("P-256 exact maximum"),
            ZK_X509_P256_LOG19_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1
        );
        assert_eq!(
            zk_x509_main_preprocessed_fixed_maximum_encoded_bytes_v1().expect("MAIN exact maximum"),
            ZK_X509_MAIN_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1
        );
        assert_eq!(
            checked_wire_len_v1(&profiles, &proof).expect("combined checked wire length"),
            ZK_X509_MAIN_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1
        );
        let encoded =
            encode_zk_x509_preprocessed_fixed_proof_v1(&profiles, &proof).expect("maximum encode");
        assert_eq!(
            encoded.len(),
            ZK_X509_MAIN_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1
        );
        assert_eq!(
            decode_zk_x509_preprocessed_fixed_proof_v1(&profiles, &encoded)
                .expect("maximum decode"),
            proof
        );

        let mut changed = proof.clone();
        changed.oracles.swap(0, 1);
        assert!(
            encode_zk_x509_preprocessed_fixed_proof_v1(&profiles, &changed).is_err(),
            "cross-oracle substitution must reject"
        );
        changed = proof.clone();
        changed.oracles.pop();
        assert!(
            encode_zk_x509_preprocessed_fixed_proof_v1(&profiles, &changed).is_err(),
            "omitting oracle two must reject"
        );
        changed = proof;
        changed.oracles[1].rows[0].pop();
        assert!(
            encode_zk_x509_preprocessed_fixed_proof_v1(&profiles, &changed).is_err(),
            "a partial P-256 row must reject"
        );
    }

    #[test]
    fn ordered_logarithmic_merkle_frontier_matches_full_tree() {
        let leaves = (0_u8..16)
            .map(|value| <[u8; 32]>::from(Sha256::digest([value])))
            .collect::<Vec<_>>();
        let tree = Sha256MerkleTreeV1::from_leaves(leaves.clone(), FIXED_NODE_DOMAIN_V1)
            .expect("full tree");
        let mut frontier = OrderedMerkleFrontierV1::new_v1(4).expect("frontier");
        for leaf in &leaves {
            frontier.push_v1(*leaf).expect("ordered leaf");
        }
        assert_eq!(frontier.finish_v1(16).expect("frontier root"), tree.root());

        let mut reversed = leaves;
        reversed.reverse();
        let mut reversed_frontier = OrderedMerkleFrontierV1::new_v1(4).expect("frontier");
        for leaf in reversed {
            reversed_frontier.push_v1(leaf).expect("reversed leaf");
        }
        assert_ne!(
            reversed_frontier
                .finish_v1(16)
                .expect("reversed frontier root"),
            tree.root()
        );
    }

    #[test]
    fn release_root_cpu_and_rss_work_certificate_is_exact() {
        assert_eq!(
            core::mem::size_of::<CompactFixedLeafSha256V1>(),
            ZK_X509_PREPROCESSED_FIXED_COMPACT_SHA_STATE_BYTES_V1
        );
        assert_eq!(ZK_X509_SHA_PREPROCESSED_ROOT_BATCH_COUNT_V1, 43);
        assert_eq!(
            ZK_X509_SHA_PREPROCESSED_ROOT_BATCH_BUTTERFLIES_V1,
            18_249_678_848
        );
        assert_eq!(
            ZK_X509_SHA_PREPROCESSED_ROOT_BATCH_BUTTERFLIES_V1 * 8,
            145_997_430_784
        );
        assert_eq!(
            ZK_X509_SHA_PREPROCESSED_ROOT_SCHEDULED_FIELD_OPS_MAX_V1,
            456_602_681_344
        );
        assert_eq!(
            ZK_X509_SHA_PREPROCESSED_ROOT_LEAF_SHA_BLOCKS_V1,
            1_442_840_576
        );
        assert_eq!(ZK_X509_P256_LOG19_PREPROCESSED_ROOT_BATCH_COUNT_V1, 51);
        assert_eq!(
            ZK_X509_P256_LOG19_PREPROCESSED_ROOT_BATCH_BUTTERFLIES_V1,
            21_644_967_936
        );
        assert_eq!(
            ZK_X509_P256_LOG19_PREPROCESSED_ROOT_BATCH_BUTTERFLIES_V1 * 8,
            173_159_743_488
        );
        assert_eq!(
            ZK_X509_P256_LOG19_PREPROCESSED_ROOT_SCHEDULED_FIELD_OPS_MAX_V1,
            541_552_017_408
        );
        assert_eq!(
            ZK_X509_P256_LOG19_PREPROCESSED_ROOT_LEAF_SHA_BLOCKS_V1,
            1_711_276_032
        );
        assert_eq!(ZK_X509_SHA_PREPROCESSED_ROOT_MERKLE_NODES_V1, 33_554_431);
        assert_eq!(
            ZK_X509_SHA_PREPROCESSED_ROOT_MERKLE_SHA_BLOCKS_V1,
            100_663_293
        );
        assert_eq!(ZK_X509_SHA_PREPROCESSED_ROOT_MAX_SECONDS_V1, 3_600);
        checked_release_root_memory_v1(ZK_X509_SHA_PREPROCESSED_FIXED_GEOMETRY_V1)
            .expect("release root RSS certificate");
        checked_release_root_memory_v1(ZK_X509_P256_LOG19_PREPROCESSED_FIXED_GEOMETRY_V1)
            .expect("P-256 release root RSS certificate");
        let rows = 1_usize << ZK_X509_MAIN_COMMON_LDE_LOG2_V1;
        let calculated_peak = rows * core::mem::size_of::<CompactFixedLeafSha256V1>()
            + rows * core::mem::size_of::<[F; 8]>()
            + (1_usize << ZK_X509_MAX_NATIVE_TRACE_LOG2_V1) * core::mem::size_of::<[F; 8]>() * 2
            + rows / 2 * core::mem::size_of::<F>()
            + ZK_X509_PREPROCESSED_FIXED_FINALIZE_CHUNK_ROWS_V1 * core::mem::size_of::<[u8; 32]>()
            + ZK_X509_PREPROCESSED_FIXED_ALLOCATION_RESERVE_BYTES_V1;
        assert_eq!(calculated_peak, 3_826_253_824);
        assert!(
            u64::try_from(calculated_peak).expect("peak fits u64")
                <= ZK_X509_SHA_PREPROCESSED_ROOT_MAX_RSS_BYTES_V1
        );
    }

    #[test]
    fn sha_certificate_rejects_root_profile_segment_order_and_stale_log_mismatches() {
        let expected = ZkX509ShaPreprocessedFixedCertificateV1::from_derived_root_v1([1; 32])
            .expect("candidate certificate");
        let mut changed = expected;
        changed.profile.root = [2; 32];
        assert_eq!(
            validate_zk_x509_sha_preprocessed_fixed_certificate_v1(changed, expected),
            Err(ZkX509PreprocessedFixedErrorV1::RootMismatch)
        );

        changed = expected;
        changed.segment_order.swap(0, 1);
        assert_eq!(
            validate_zk_x509_sha_preprocessed_fixed_certificate_v1(changed, expected),
            Err(ZkX509PreprocessedFixedErrorV1::Profile)
        );

        changed = expected;
        changed.profile.geometry.native_log2 -= 1;
        assert_eq!(
            validate_zk_x509_sha_preprocessed_fixed_certificate_v1(changed, expected),
            Err(ZkX509PreprocessedFixedErrorV1::Profile)
        );

        changed = expected;
        changed.profile.geometry.lde_log2 = 22;
        assert_eq!(
            validate_zk_x509_sha_preprocessed_fixed_certificate_v1(changed, expected),
            Err(ZkX509PreprocessedFixedErrorV1::Profile),
            "the stale SHA log22 certificate must fail closed"
        );

        changed = expected;
        changed.profile.geometry.width -= 1;
        assert_eq!(
            validate_zk_x509_sha_preprocessed_fixed_certificate_v1(changed, expected),
            Err(ZkX509PreprocessedFixedErrorV1::Profile)
        );

        changed = expected;
        changed.descriptor_digest[0] ^= 1;
        assert_eq!(
            validate_zk_x509_sha_preprocessed_fixed_certificate_v1(changed, expected),
            Err(ZkX509PreprocessedFixedErrorV1::Profile)
        );
        assert_eq!(
            pinned_zk_x509_sha_preprocessed_fixed_certificate_v1(),
            Err(ZkX509PreprocessedFixedErrorV1::Unpinned)
        );
    }

    #[test]
    fn p256_certificate_and_two_oracle_profile_order_fail_closed_on_mutation() {
        let sha = ZkX509ShaPreprocessedFixedCertificateV1::from_derived_root_v1([1; 32])
            .expect("SHA candidate certificate");
        let expected = ZkX509P256Log19PreprocessedFixedCertificateV1::from_derived_root_v1([2; 32])
            .expect("P-256 candidate certificate");
        assert_eq!(
            zk_x509_main_preprocessed_fixed_profiles_v1(sha, expected)
                .expect("canonical MAIN profile order"),
            [sha.profile, expected.profile]
        );

        let mut changed = expected;
        changed.profile.root[0] ^= 1;
        assert_eq!(
            validate_zk_x509_p256_log19_preprocessed_fixed_certificate_v1(changed, expected),
            Err(ZkX509PreprocessedFixedErrorV1::RootMismatch)
        );
        changed = expected;
        changed.schedule_order.swap(0, 1);
        assert_eq!(
            validate_zk_x509_p256_log19_preprocessed_fixed_certificate_v1(changed, expected),
            Err(ZkX509PreprocessedFixedErrorV1::Profile)
        );
        changed = expected;
        changed.profile.geometry.oracle = ZK_X509_SHA_PREPROCESSED_FIXED_ORACLE_V1;
        assert_eq!(
            validate_zk_x509_p256_log19_preprocessed_fixed_certificate_v1(changed, expected),
            Err(ZkX509PreprocessedFixedErrorV1::Profile)
        );
        changed = expected;
        changed.profile.geometry.native_log2 -= 1;
        assert_eq!(
            validate_zk_x509_p256_log19_preprocessed_fixed_certificate_v1(changed, expected),
            Err(ZkX509PreprocessedFixedErrorV1::Profile)
        );
        changed = expected;
        changed.profile.geometry.lde_log2 -= 1;
        assert_eq!(
            validate_zk_x509_p256_log19_preprocessed_fixed_certificate_v1(changed, expected),
            Err(ZkX509PreprocessedFixedErrorV1::Profile)
        );
        changed = expected;
        changed.profile.geometry.width -= 1;
        assert_eq!(
            validate_zk_x509_p256_log19_preprocessed_fixed_certificate_v1(changed, expected),
            Err(ZkX509PreprocessedFixedErrorV1::Profile)
        );
        changed = expected;
        changed.descriptor_digest[0] ^= 1;
        assert_eq!(
            validate_zk_x509_p256_log19_preprocessed_fixed_certificate_v1(changed, expected),
            Err(ZkX509PreprocessedFixedErrorV1::Profile)
        );
        assert_eq!(
            pinned_zk_x509_p256_log19_preprocessed_fixed_certificate_v1(),
            Err(ZkX509PreprocessedFixedErrorV1::Unpinned)
        );
    }

    #[test]
    fn sha_preprocessed_column_subset_is_shape_independent_and_segment_major() {
        let baseline = ZkX509ShaBatchFixedProviderV1::new_v1(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 0,
        })
        .expect("baseline fixed provider");
        for disclosed_attributes in 1..=4 {
            let candidate = ZkX509ShaBatchFixedProviderV1::new_v1(ZkX509ShaCallPublicShapeV1 {
                disclosed_attributes,
            })
            .expect("candidate fixed provider");
            for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
                for row in [
                    0,
                    1,
                    ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[segment] - 1,
                    ZK_X509_SHA_SEGMENT_ACTIVE_ROWS_V1[segment],
                    ZK_X509_SHA_SEGMENT_ROWS_V1 - 1,
                ] {
                    let expected = baseline
                        .fixed_row_v1(segment, row)
                        .expect("baseline fixed row");
                    let actual = candidate
                        .fixed_row_v1(segment, row)
                        .expect("candidate fixed row");
                    let expected =
                        reduce_sha_preprocessed_fixed_segment_row_v1(&expected).expect("reduced");
                    let actual =
                        reduce_sha_preprocessed_fixed_segment_row_v1(&actual).expect("reduced");
                    assert_eq!(
                        actual, expected,
                        "disclosures {disclosed_attributes}, segment {segment}, row {row}"
                    );
                    for local in 0..ZK_X509_SHA_PREPROCESSED_FIXED_COLUMNS_PER_SEGMENT_V1 {
                        let global =
                            segment * ZK_X509_SHA_PREPROCESSED_FIXED_COLUMNS_PER_SEGMENT_V1 + local;
                        assert_eq!(
                            global / ZK_X509_SHA_PREPROCESSED_FIXED_COLUMNS_PER_SEGMENT_V1,
                            segment
                        );
                    }
                }
            }
        }
        assert_eq!(
            expand_zk_x509_sha_preprocessed_fixed_row_v1(&vec![
                F::ZERO;
                ZK_X509_SHA_PREPROCESSED_FIXED_WIDTH_V1
                    - 1
            ]),
            Err(ZkX509PreprocessedFixedErrorV1::Opening)
        );
    }

    #[test]
    fn authenticated_reduced_sha_rows_expand_exactly_and_zero_statement_columns() {
        let provider = ZkX509ShaBatchFixedProviderV1::new_v1(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 0,
        })
        .expect("fixed provider");
        for row in [0, 1, ZK_X509_SHA_SEGMENT_ROWS_V1 - 1] {
            let mut reduced = Vec::with_capacity(ZK_X509_SHA_PREPROCESSED_FIXED_WIDTH_V1);
            let mut expected =
                Vec::with_capacity(ZK_X509_SHA_SEGMENT_COUNT_V1 * ZK_X509_SHA_BATCH_FIXED_WIDTH_V1);
            for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
                let full = provider
                    .fixed_row_v1(segment, row)
                    .expect("complete fixed row");
                reduced.extend_from_slice(
                    &reduce_sha_preprocessed_fixed_segment_row_v1(&full)
                        .expect("canonical reduction"),
                );
                expected.extend_from_slice(&full);
            }
            let rfc_start = ZK_X509_SHA_FIXED_CA_CALL_SELECTORS_V1 + ZK_X509_SHA_CA_CALL_COUNT_V1;
            let mut authenticated_expected = expected.clone();
            for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
                let start = segment * ZK_X509_SHA_BATCH_FIXED_WIDTH_V1 + rfc_start;
                let end = (segment + 1) * ZK_X509_SHA_BATCH_FIXED_WIDTH_V1;
                authenticated_expected[start..end].fill(F::ZERO);
            }
            if row != ZK_X509_SHA_SEGMENT_ROWS_V1 - 1 {
                assert!(
                    expected
                        .chunks_exact(ZK_X509_SHA_BATCH_FIXED_WIDTH_V1)
                        .any(|segment| segment[rfc_start..].iter().any(|value| *value != F::ZERO)),
                    "canonical active row must exercise verifier-generated RFC columns"
                );
            }
            assert_eq!(
                expand_zk_x509_sha_preprocessed_fixed_row_v1(&reduced)
                    .expect("authenticated opening expansion")
                    .as_slice(),
                authenticated_expected,
                "physical row {row}"
            );
        }
    }

    #[test]
    fn p256_log19_manifest_slices_and_role_aliases_match_native_fixed_rows() {
        let fixed = P256MainVerifierFixedSourceV1::new_v1().expect("P-256 fixed source");
        let native_rows = 1_usize << ZK_X509_MAX_NATIVE_TRACE_LOG2_V1;
        let boundary_columns = [
            0,
            P256_LOG19_WALLET_ARITHMETIC_START_V1 - 1,
            P256_LOG19_WALLET_ARITHMETIC_START_V1,
            P256_LOG19_CERTIFICATE_EXECUTION_START_V1 - 1,
            P256_LOG19_CERTIFICATE_EXECUTION_START_V1,
            P256_LOG19_WALLET_EXECUTION_START_V1 - 1,
            P256_LOG19_WALLET_EXECUTION_START_V1,
            P256_LOG19_CERTIFICATE_SORTED_START_V1 - 1,
            P256_LOG19_CERTIFICATE_SORTED_START_V1,
            P256_LOG19_WALLET_SORTED_START_V1 - 1,
            P256_LOG19_WALLET_SORTED_START_V1,
            ZK_X509_P256_LOG19_PREPROCESSED_FIXED_WIDTH_V1 - 1,
        ];
        for row in [0, 1, native_rows / 2, native_rows - 1] {
            let combined =
                p256_log19_native_fixed_row_v1(&fixed, row).expect("combined native fixed row");
            assert_eq!(
                combined.len(),
                ZK_X509_P256_LOG19_PREPROCESSED_FIXED_WIDTH_V1
            );
            for global_column in boundary_columns {
                let (schedule, local_column) =
                    p256_log19_fixed_schedule_for_column_v1(global_column)
                        .expect("manifest boundary column");
                let expected = fixed
                    .fixed_row_v1(
                        schedule
                            .representative_registration_v1()
                            .expect("representative registration"),
                        row,
                    )
                    .expect("representative fixed row");
                assert_eq!(
                    combined[global_column], expected[local_column],
                    "row {row}, global column {global_column}"
                );
            }
            for signature in 0..P256_X5S1_SIGNATURES_V1 {
                for (adapter, local_instance) in [
                    (P256MainAdapterV1::Arithmetic, 0),
                    (P256MainAdapterV1::ValueBus, 0),
                    (P256MainAdapterV1::ValueBus, 1),
                ] {
                    let registration =
                        P256MainRegistrationV1::new_v1(signature, adapter, local_instance)
                            .expect("canonical log19 registration");
                    let expected = fixed
                        .fixed_row_v1(registration, row)
                        .expect("registration fixed row");
                    assert_eq!(
                        zk_x509_p256_log19_preprocessed_fixed_row_for_registration_v1(
                            &combined,
                            registration,
                        )
                        .expect("authenticated manifest slice"),
                        expected.as_slice(),
                        "signature {signature}, adapter {adapter:?}, local {local_instance}, row {row}"
                    );
                }
            }
        }

        assert_eq!(
            p256_log19_fixed_schedule_for_column_v1(ZK_X509_P256_LOG19_PREPROCESSED_FIXED_WIDTH_V1),
            Err(ZkX509PreprocessedFixedErrorV1::Column)
        );
        let combined =
            p256_log19_native_fixed_row_v1(&fixed, 0).expect("combined native fixed row");
        let unsupported = P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::WindowBatch, 0)
            .expect("valid non-log19 registration");
        assert_eq!(
            zk_x509_p256_log19_preprocessed_fixed_row_for_registration_v1(&combined, unsupported,),
            Err(ZkX509PreprocessedFixedErrorV1::Profile)
        );
        assert_eq!(
            zk_x509_p256_log19_preprocessed_fixed_row_for_registration_v1(
                &combined[..combined.len() - 1],
                P256MainRegistrationV1::new_v1(0, P256MainAdapterV1::Arithmetic, 0)
                    .expect("canonical registration"),
            ),
            Err(ZkX509PreprocessedFixedErrorV1::Opening)
        );
    }

    #[test]
    #[ignore = "serialized release diagnostic: 43 row-major batch8 log25 transforms"]
    fn emit_sha_preprocessed_fixed_release_candidate() {
        let started = std::time::Instant::now();
        let root = derive_zk_x509_sha_preprocessed_fixed_root_v1()
            .expect("derive combined SHA fixed root");
        let elapsed = started.elapsed();
        assert!(
            elapsed.as_secs() <= ZK_X509_SHA_PREPROCESSED_ROOT_MAX_SECONDS_V1,
            "release root exceeded the pinned benchmark duration: {elapsed:?}"
        );
        let certificate = ZkX509ShaPreprocessedFixedCertificateV1::from_derived_root_v1(root)
            .expect("candidate certificate");
        let encoded = certificate.encode_v1().expect("certificate encoding");
        eprintln!(
            "zk-x509 SHA fixed candidate root={} descriptor_digest={} certificate={} elapsed={elapsed:?}",
            lowercase_hex_v1(&root),
            lowercase_hex_v1(&certificate.descriptor_digest),
            lowercase_hex_v1(&encoded),
        );
    }

    #[test]
    #[ignore = "serialized release diagnostic: 51 row-major batch8 log25 transforms"]
    fn emit_p256_log19_preprocessed_fixed_release_candidate() {
        let started = std::time::Instant::now();
        let root = derive_zk_x509_p256_log19_preprocessed_fixed_root_v1()
            .expect("derive P-256 log19 fixed root");
        let elapsed = started.elapsed();
        let certificate = ZkX509P256Log19PreprocessedFixedCertificateV1::from_derived_root_v1(root)
            .expect("candidate certificate");
        let encoded = certificate.encode_v1().expect("certificate encoding");
        eprintln!(
            "zk-x509 P-256 log19 fixed candidate root={} descriptor_digest={} certificate={} elapsed={elapsed:?}",
            lowercase_hex_v1(&root),
            lowercase_hex_v1(&certificate.descriptor_digest),
            lowercase_hex_v1(&encoded),
        );
    }

    fn independently_materialized_root_v1() -> [u8; 32] {
        let trace_root =
            goldilocks_primitive_root_v1(TEST_GEOMETRY.native_log2).expect("trace root");
        let lde_root = goldilocks_primitive_root_v1(TEST_GEOMETRY.lde_log2).expect("LDE root");
        let columns = (0..usize::from(TEST_GEOMETRY.width))
            .map(|column| {
                let mut coefficients = test_native_column(column).expect("native column");
                goldilocks_ifft_v1(&mut coefficients, trace_root).expect("IFFT");
                goldilocks_evaluate_coset_v1(
                    &coefficients,
                    1_usize << TEST_GEOMETRY.lde_log2,
                    lde_root,
                    F(GOLDILOCKS_GENERATOR_V1),
                )
                .expect("LDE")
            })
            .collect::<Vec<_>>();
        let leaves = (0..1_usize << TEST_GEOMETRY.lde_log2)
            .map(|row| {
                fixed_leaf_hash_v1(
                    TEST_GEOMETRY,
                    &columns
                        .iter()
                        .map(|column| column[row].0)
                        .collect::<Vec<_>>(),
                )
                .expect("leaf")
            })
            .collect();
        Sha256MerkleTreeV1::from_leaves(leaves, FIXED_NODE_DOMAIN_V1)
            .expect("tree")
            .root()
    }

    fn fixture() -> (
        ZkX509PreprocessedFixedProfileV1,
        Vec<usize>,
        ZkX509PreprocessedFixedMultiproofV1,
    ) {
        let root = recompute_zk_x509_preprocessed_fixed_root_v1(TEST_GEOMETRY, test_native_column)
            .expect("streamed root");
        assert_eq!(root, independently_materialized_root_v1());
        let profile = ZkX509PreprocessedFixedProfileV1 {
            geometry: TEST_GEOMETRY,
            root,
        };
        let indices = vec![1, 3, 7, 19, 63, 126];
        let proof =
            build_zk_x509_preprocessed_fixed_multiproof_v1(profile, &indices, test_native_column)
                .expect("multiproof");
        (profile, indices, proof)
    }

    fn two_oracle_service_fixture_v1() -> (
        [ZkX509PreprocessedFixedProfileV1; 2],
        ZkX509Log19PreprocessedFixedOpeningIndicesV1,
        Vec<u8>,
    ) {
        let indices = vec![1, 3, 7, 19, 63, 126];
        let first_root =
            recompute_zk_x509_preprocessed_fixed_root_v1(TEST_GEOMETRY, test_native_column)
                .expect("first root");
        let second_root = recompute_zk_x509_preprocessed_fixed_root_v1(
            TEST_PARTIAL_BATCH_GEOMETRY,
            test_partial_batch_native_column,
        )
        .expect("second root");
        let profiles = [
            ZkX509PreprocessedFixedProfileV1 {
                geometry: TEST_GEOMETRY,
                root: first_root,
            },
            ZkX509PreprocessedFixedProfileV1 {
                geometry: TEST_PARTIAL_BATCH_GEOMETRY,
                root: second_root,
            },
        ];
        let proof = ZkX509PreprocessedFixedProofV1 {
            oracles: vec![
                build_zk_x509_preprocessed_fixed_multiproof_v1(
                    profiles[0],
                    &indices,
                    test_native_column,
                )
                .expect("first multiproof"),
                build_zk_x509_preprocessed_fixed_multiproof_v1(
                    profiles[1],
                    &indices,
                    test_partial_batch_native_column,
                )
                .expect("second multiproof"),
            ],
        };
        let encoded = encode_zk_x509_preprocessed_fixed_proof_v1(&profiles, &proof)
            .expect("service artifact");
        (
            profiles,
            ZkX509Log19PreprocessedFixedOpeningIndicesV1 { indices },
            encoded,
        )
    }

    struct ExactArtifactServiceV1 {
        expected_profiles: [ZkX509PreprocessedFixedProfileV1; 2],
        expected_indices: Vec<usize>,
        encoded: Vec<u8>,
        calls: usize,
    }

    impl ZkX509MainPreprocessedFixedOpeningServiceV1 for ExactArtifactServiceV1 {
        fn open_main_v1(
            &mut self,
            profiles: &[ZkX509PreprocessedFixedProfileV1; 2],
            indices: &ZkX509Log19PreprocessedFixedOpeningIndicesV1,
        ) -> Result<Vec<u8>, ZkX509PreprocessedFixedErrorV1> {
            self.calls += 1;
            if profiles != &self.expected_profiles || indices.as_slice_v1() != self.expected_indices
            {
                return Err(ZkX509PreprocessedFixedErrorV1::Profile);
            }
            Ok(self.encoded.clone())
        }
    }

    #[test]
    fn authenticated_opening_service_is_deterministic_and_fails_closed_on_stale_artifacts() {
        let (profiles, indices, encoded) = two_oracle_service_fixture_v1();
        let mut service = ExactArtifactServiceV1 {
            expected_profiles: profiles,
            expected_indices: indices.as_slice_v1().to_vec(),
            encoded: encoded.clone(),
            calls: 0,
        };
        let first = request_zk_x509_main_preprocessed_fixed_openings_for_profiles_v1(
            &profiles,
            &indices,
            &mut service,
        )
        .expect("authenticated artifact");
        let second = request_zk_x509_main_preprocessed_fixed_openings_for_profiles_v1(
            &profiles,
            &indices,
            &mut service,
        )
        .expect("deterministic authenticated artifact");
        assert_eq!(first, encoded);
        assert_eq!(second, first);
        assert_eq!(service.calls, 2);

        let mut corrupted = service.encoded.clone();
        let last = corrupted.len() - 1;
        corrupted[last] ^= 1;
        service.encoded = corrupted;
        assert!(
            request_zk_x509_main_preprocessed_fixed_openings_for_profiles_v1(
                &profiles,
                &indices,
                &mut service,
            )
            .is_err(),
            "corrupted frontier material must reject"
        );

        service.encoded = encoded.clone();
        let mut stale_profiles = profiles;
        stale_profiles[1].root[0] ^= 1;
        service.expected_profiles = stale_profiles;
        assert_eq!(
            request_zk_x509_main_preprocessed_fixed_openings_for_profiles_v1(
                &stale_profiles,
                &indices,
                &mut service,
            ),
            Err(ZkX509PreprocessedFixedErrorV1::Opening),
            "a stale or wrong-root artifact must not authenticate"
        );

        service.expected_profiles = profiles;
        let mut missing_oracle = encoded.clone();
        missing_oracle[7] = 1;
        service.encoded = missing_oracle;
        assert!(
            request_zk_x509_main_preprocessed_fixed_openings_for_profiles_v1(
                &profiles,
                &indices,
                &mut service,
            )
            .is_err(),
            "a partial two-oracle artifact must reject"
        );

        service.encoded = encoded;
        let mismatched_indices = ZkX509Log19PreprocessedFixedOpeningIndicesV1 {
            indices: vec![1, 3, 7, 19, 63, 125],
        };
        assert_eq!(
            request_zk_x509_main_preprocessed_fixed_openings_for_profiles_v1(
                &profiles,
                &mismatched_indices,
                &mut service,
            ),
            Err(ZkX509PreprocessedFixedErrorV1::Profile),
            "the cache key must include the exact query-derived opening set"
        );
    }

    #[test]
    fn streamed_root_codec_and_canonical_multiproof_roundtrip() {
        let (profile, indices, oracle) = fixture();
        let opened = verify_zk_x509_preprocessed_fixed_multiproof_v1(profile, &indices, &oracle)
            .expect("opening");
        assert_eq!(opened.len(), indices.len());
        let proof = ZkX509PreprocessedFixedProofV1 {
            oracles: vec![oracle],
        };
        let encoded =
            encode_zk_x509_preprocessed_fixed_proof_v1(&[profile], &proof).expect("encode");
        let decoded =
            decode_zk_x509_preprocessed_fixed_proof_v1(&[profile], &encoded).expect("decode");
        assert_eq!(decoded, proof);
    }

    #[test]
    fn root_column_order_index_value_path_and_codec_mutations_fail_closed() {
        let (profile, indices, oracle) = fixture();
        let rejects = |profile, proof: &ZkX509PreprocessedFixedMultiproofV1| {
            assert!(
                verify_zk_x509_preprocessed_fixed_multiproof_v1(profile, &indices, proof).is_err()
            );
        };

        let mut wrong_profile = profile;
        wrong_profile.root[0] ^= 1;
        rejects(wrong_profile, &oracle);

        changed_profile_geometry_rejects(profile, &indices, &oracle);

        let mut changed = oracle.clone();
        changed.oracle += 1;
        rejects(profile, &changed);

        changed = oracle.clone();
        changed.rows[0].swap(0, 1);
        rejects(profile, &changed);

        changed = oracle.clone();
        changed.indices[1] += 1;
        rejects(profile, &changed);

        changed = oracle.clone();
        changed.rows[2][1] = changed.rows[2][1].wrapping_add(1);
        rejects(profile, &changed);

        changed = oracle.clone();
        changed.frontier[0][0] ^= 1;
        rejects(profile, &changed);

        changed = oracle.clone();
        changed.frontier.pop();
        rejects(profile, &changed);

        changed = oracle.clone();
        changed.frontier.push([0; 32]);
        rejects(profile, &changed);

        changed = oracle.clone();
        changed.indices[1] = changed.indices[0];
        assert!(
            encode_zk_x509_preprocessed_fixed_proof_v1(
                &[profile],
                &ZkX509PreprocessedFixedProofV1 {
                    oracles: vec![changed],
                },
            )
            .is_err()
        );

        let canonical = ZkX509PreprocessedFixedProofV1 {
            oracles: vec![oracle],
        };
        let encoded =
            encode_zk_x509_preprocessed_fixed_proof_v1(&[profile], &canonical).expect("encode");
        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 1;
        assert!(decode_zk_x509_preprocessed_fixed_proof_v1(&[profile], &wrong_magic).is_err());
        let mut wrong_version = encoded.clone();
        wrong_version[5] ^= 1;
        assert!(decode_zk_x509_preprocessed_fixed_proof_v1(&[profile], &wrong_version).is_err());
        let mut wrong_count = encoded.clone();
        wrong_count[7] = 2;
        assert!(decode_zk_x509_preprocessed_fixed_proof_v1(&[profile], &wrong_count).is_err());
        let mut wrong_oracle = encoded.clone();
        wrong_oracle[9] ^= 1;
        assert!(decode_zk_x509_preprocessed_fixed_proof_v1(&[profile], &wrong_oracle).is_err());
        let mut noncanonical = encoded.clone();
        noncanonical[16..24].copy_from_slice(&GOLDILOCKS_MODULUS_V1.to_be_bytes());
        assert!(decode_zk_x509_preprocessed_fixed_proof_v1(&[profile], &noncanonical).is_err());
        let mut second = profile;
        second.geometry.oracle += 1;
        second.root[0] ^= 1;
        assert!(decode_zk_x509_preprocessed_fixed_proof_v1(&[second, profile], &encoded).is_err());
        assert!(decode_zk_x509_preprocessed_fixed_proof_v1(&[profile, second], &encoded).is_err());
        for length in 0..encoded.len() {
            assert!(
                decode_zk_x509_preprocessed_fixed_proof_v1(&[profile], &encoded[..length]).is_err(),
                "truncation {length} must reject"
            );
        }
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(decode_zk_x509_preprocessed_fixed_proof_v1(&[profile], &trailing).is_err());
        assert!(
            decode_zk_x509_preprocessed_fixed_proof_v1(
                &[profile],
                &vec![0; ZK_X509_PREPROCESSED_FIXED_MAX_WIRE_BYTES_V1 + 1],
            )
            .is_err()
        );
    }

    fn changed_profile_geometry_rejects(
        profile: ZkX509PreprocessedFixedProfileV1,
        indices: &[usize],
        oracle: &ZkX509PreprocessedFixedMultiproofV1,
    ) {
        for geometry in [
            ZkX509PreprocessedFixedGeometryV1 {
                oracle: profile.geometry.oracle + 1,
                ..profile.geometry
            },
            ZkX509PreprocessedFixedGeometryV1 {
                native_log2: profile.geometry.native_log2 + 1,
                ..profile.geometry
            },
            ZkX509PreprocessedFixedGeometryV1 {
                lde_log2: profile.geometry.lde_log2 - 1,
                ..profile.geometry
            },
            ZkX509PreprocessedFixedGeometryV1 {
                width: profile.geometry.width - 1,
                ..profile.geometry
            },
        ] {
            assert!(
                verify_zk_x509_preprocessed_fixed_multiproof_v1(
                    ZkX509PreprocessedFixedProfileV1 {
                        geometry,
                        root: profile.root,
                    },
                    indices,
                    oracle,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn provider_shape_column_order_and_root_regeneration_mutations_fail_closed() {
        let (profile, indices, _) = fixture();
        let reversed =
            |column: usize| test_native_column(usize::from(TEST_GEOMETRY.width) - 1 - column);
        let reversed_root = recompute_zk_x509_preprocessed_fixed_root_v1(TEST_GEOMETRY, reversed)
            .expect("reversed root");
        assert_ne!(reversed_root, profile.root);
        assert_eq!(
            build_zk_x509_preprocessed_fixed_multiproof_v1(profile, &indices, reversed),
            Err(ZkX509PreprocessedFixedErrorV1::RootMismatch)
        );
        assert_eq!(
            recompute_zk_x509_preprocessed_fixed_root_v1(TEST_GEOMETRY, |_| {
                Ok(vec![F::ZERO; (1_usize << TEST_GEOMETRY.native_log2) - 1])
            }),
            Err(ZkX509PreprocessedFixedErrorV1::Column)
        );
        assert_eq!(
            recompute_zk_x509_preprocessed_fixed_root_v1(TEST_GEOMETRY, |_| {
                Ok(vec![
                    F(GOLDILOCKS_MODULUS_V1);
                    1_usize << TEST_GEOMETRY.native_log2
                ])
            }),
            Err(ZkX509PreprocessedFixedErrorV1::Column)
        );
    }
}
