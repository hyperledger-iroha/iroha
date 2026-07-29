//! Transparent preprocessing for verifier-fixed zk-X509 LDE columns.
//!
//! A pinned oracle root authenticates only profile-derived fixed columns. The
//! prover supplies the queried rows and the unique minimal Merkle frontier;
//! it never chooses a root. Root generation uses the same Goldilocks IFFT,
//! generator-coset LDE, big-endian field encoding, column order, and Merkle
//! domains as verification. A cache may retain tree material for speed, but
//! consensus verification depends only on the pinned profile and proof.

use std::collections::{BTreeMap, BTreeSet};

use rayon::prelude::*;
use sha2::{
    Digest as _, Sha256, Sha256VarCore, compress256, digest::core_api::Block as DigestBlock,
};
use thiserror::Error;

use super::{
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
pub(crate) const ZK_X509_PREPROCESSED_FIXED_DESCRIPTOR_V1: &[u8] = b"zk-x509-preprocessed-fixed-v1-incompatible:wire=X5F1+u16be-version1+u16be-oracle-count+per-oracle-u16be-id+u16be-opening-count+repeated-u32be-index-and-width-u64be-fields+minimal-frontier-hashes32:verifier-pinned-roots-only:profile-derived-independent-columns-only:six-sha-word-columns-reconstructed-by-fixed-linear-identities:no-statement-state-time-or-witness-values:goldilocks-modulus=0xffffffff00000001:generator=7:canonical-u64be-fields:column-order-profile-bound:geometry=oracle-nonzero+native-log2-4through19+lde-log2-nativeplus1through25+width-u16-nonzero:native-power-of-two-subgroup:ifft-then-generator-coset-lde:release-root=row-major-batch8-ifft-and-generator-coset-fft+compact-sha256-midstates+bounded-finalization-chunks+ordered-logarithmic-merkle-frontier:frame-domain=iroha:privacy:transparent-stark:frame:v1:leaf-domain=iroha:privacy:zk-x509:preprocessed-fixed:leaf:v1:leaf-fields=oracle-u16be+native-log2-u8+lde-log2-u8+width-u16be+ordered-u64be-fields:node-domain=iroha:privacy:zk-x509:preprocessed-fixed:node:v1:binary-sha256-merkle:canonical-sorted-unique-indices:max-openings116:minimal-multiproof-frontier:max-wire524288:no-prover-root-on-wire:cache-root-verified-and-optional:activation=false";

/// Canonical sidecar magic.
pub(crate) const ZK_X509_PREPROCESSED_FIXED_MAGIC_V1: [u8; 4] = *b"X5F1";
/// Sole sidecar version.
pub(crate) const ZK_X509_PREPROCESSED_FIXED_VERSION_V1: u16 = 1;
/// Maximum fixed openings per oracle: current and next rows for all 58 MAIN
/// query positions.
pub(crate) const ZK_X509_PREPROCESSED_FIXED_MAX_OPENINGS_V1: usize = 116;
/// Fixed-column LDE batch required by the release resource profile.
pub(crate) const ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1: usize = 8;
/// Hard cap for the complete fixed-oracle sidecar.
pub(crate) const ZK_X509_PREPROCESSED_FIXED_MAX_WIRE_BYTES_V1: usize = 512 * 1024;
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

/// Stable oracle identifier for the sole first-release SHA fixed oracle.
pub(crate) const ZK_X509_SHA_PREPROCESSED_FIXED_ORACLE_V1: u16 = 1;
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
/// Exact aligned SHA-256 compression blocks for every vector-row leaf,
/// excluding the two common prefix blocks computed once.
pub(crate) const ZK_X509_SHA_PREPROCESSED_ROOT_LEAF_SHA_BLOCKS_V1: u64 =
    (1_u64 << ZK_X509_MAIN_COMMON_LDE_LOG2_V1) * (ZK_X509_SHA_PREPROCESSED_ROOT_BATCH_COUNT_V1 + 1);
/// Exact internal binary-Merkle node count.
pub(crate) const ZK_X509_SHA_PREPROCESSED_ROOT_MERKLE_NODES_V1: u64 =
    (1_u64 << ZK_X509_MAIN_COMMON_LDE_LOG2_V1) - 1;
/// Exact SHA-256 compression blocks for all framed internal Merkle nodes.
pub(crate) const ZK_X509_SHA_PREPROCESSED_ROOT_MERKLE_SHA_BLOCKS_V1: u64 =
    ZK_X509_SHA_PREPROCESSED_ROOT_MERKLE_NODES_V1 * 3;

/// Exact first-release SHA fixed-column manifest.
pub(crate) const ZK_X509_SHA_PREPROCESSED_FIXED_COLUMN_DESCRIPTOR_V1: &[u8] =
    b"zk-x509-sha-preprocessed-fixed-columns-v1-incompatible:oracle1:four-segments-ordered0,1,2,3:per-segment85=66-independent-sha-word-fixed-columns-in-source-order+call+role+slot+segment-first+segment-last+physical-padding+thirteen-one-hot-compact-ca-call-selectors16through28:reconstruct-sha-word-fixed-padding=0+local-continue=sum-local-operations-minus-local-first-minus-local-last+memory-continue=memory-same-next-plus-memory-new-next+last-aggregate-row=0+physical-boundary=0+boolean-continue=choose-plus-majority-minus-boolean-last:combined-column-order=segment-major-then-listed-source-column:excluded=rfc-length-pair+rfc-length-pair-index+rfc-length-prefix+four-rfc-event-descriptors:shape-independent-for-disclosed-attributes0through4:native-log19:generator-coset-lde-log25:root-must-be-release-pinned:activation=false";

const _: () = {
    assert!(ZK_X509_SHA_SEGMENT_COUNT_V1 == 4);
    assert!(SHA_WORD_CAPACITY_FIXED_WIDTH_V1 == 72);
    assert!(ZK_X509_SHA_WORD_PREPROCESSED_FIXED_WIDTH_V1 == 66);
    assert!(ZK_X509_SHA_PREPROCESSED_FIXED_COLUMNS_PER_SEGMENT_V1 == 85);
    assert!(ZK_X509_SHA_PREPROCESSED_FIXED_WIDTH_V1 == 340);
    assert!(ZK_X509_SHA_PREPROCESSED_ROOT_BATCH_COUNT_V1 == 43);
    assert!(ZK_X509_SHA_PREPROCESSED_ROOT_BATCH_BUTTERFLIES_V1 == 18_249_678_848);
    assert!(ZK_X509_SHA_PREPROCESSED_ROOT_SCHEDULED_FIELD_OPS_MAX_V1 == 456_602_681_344);
    assert!(ZK_X509_SHA_PREPROCESSED_ROOT_LEAF_SHA_BLOCKS_V1 == 1_476_395_008);
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
            <= ZK_X509_PREPROCESSED_FIXED_MAX_WIRE_BYTES_V1
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
pub(crate) struct ZkX509ShaPreprocessedFixedOpeningIndicesV1 {
    indices: Vec<usize>,
}

impl ZkX509ShaPreprocessedFixedOpeningIndicesV1 {
    fn as_slice_v1(&self) -> &[usize] {
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
    #[error("zk-X509 SHA preprocessed fixed root is not release-pinned")]
    Unpinned,
}

/// Exact encoded length of the compiled SHA fixed-oracle certificate.
pub(crate) const ZK_X509_SHA_PREPROCESSED_FIXED_CERTIFICATE_BYTES_V1: usize = 81;
const SHA_FIXED_CERTIFICATE_MAGIC_V1: [u8; 4] = *b"X5FC";
const SHA_FIXED_CERTIFICATE_VERSION_V1: u16 = 1;

// This is intentionally absent until the serialized diagnostic root
// derivation and an independent recomputation agree. No placeholder root is
// accepted or committed by the provisional profile.
const ZK_X509_SHA_PREPROCESSED_FIXED_PINNED_ROOT_V1: Option<[u8; 32]> = None;

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

fn map_transparent_error_v1(_: TransparentStarkErrorV1) -> ZkX509PreprocessedFixedErrorV1 {
    ZkX509PreprocessedFixedErrorV1::Resource
}

fn map_sha_error_v1(error: ZkX509ShaCallBusStarkErrorV1) -> ZkX509PreprocessedFixedErrorV1 {
    match error {
        ZkX509ShaCallBusStarkErrorV1::Resource => ZkX509PreprocessedFixedErrorV1::Resource,
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
    if profiles.is_empty() || profiles.len() > usize::from(u16::MAX) {
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
/// sole sorted current/next opening set for the log19 SHA oracle on the log25
/// common LDE domain.
///
/// The query order is transcript order and is therefore not required to be
/// sorted. Query sampling/authentication remains the responsibility of MAIN
/// transcript assembly; no proof-controlled stride or expanded index list
/// crosses this API.
pub(crate) fn derive_zk_x509_sha_preprocessed_fixed_opening_indices_v1(
    query_coordinates: &[usize],
) -> Result<ZkX509ShaPreprocessedFixedOpeningIndicesV1, ZkX509PreprocessedFixedErrorV1> {
    let geometry = ZK_X509_SHA_PREPROCESSED_FIXED_GEOMETRY_V1;
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
    Ok(ZkX509ShaPreprocessedFixedOpeningIndicesV1 { indices })
}

/// Recompute the exact worst-case one-oracle X5F1 byte bound from the shared
/// canonical minimal-frontier arithmetic.
pub(crate) fn zk_x509_sha_preprocessed_fixed_maximum_encoded_bytes_v1()
-> Result<usize, ZkX509PreprocessedFixedErrorV1> {
    let geometry = ZK_X509_SHA_PREPROCESSED_FIXED_GEOMETRY_V1;
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
    if frontier_hashes != ZK_X509_SHA_PREPROCESSED_FIXED_MAXIMUM_FRONTIER_HASHES_V1
        || encoded_bytes != ZK_X509_SHA_PREPROCESSED_FIXED_MAXIMUM_ENCODED_BYTES_V1
        || encoded_bytes > ZK_X509_PREPROCESSED_FIXED_MAX_WIRE_BYTES_V1
    {
        return Err(ZkX509PreprocessedFixedErrorV1::Profile);
    }
    Ok(encoded_bytes)
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

/// Expand one authenticated reduced fixed-oracle row into the four complete
/// SHA fixed rows consumed by the MAIN AIR.
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
/// row-major batch contributes 64 bytes, so no row needs the general-purpose
/// `Sha256` buffer, length counter, or padding state.
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
    ) -> Result<[u8; 32], ZkX509PreprocessedFixedErrorV1> {
        let message_bytes = u64::try_from(ZK_X509_PREPROCESSED_FIXED_LEAF_PREFIX_BYTES_V1)
            .ok()
            .and_then(|prefix| {
                u64::from(geometry.width)
                    .checked_mul(8)
                    .and_then(|payload| prefix.checked_add(payload))
            })
            .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
        if usize::from(geometry.width) % ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1 != 0 {
            return Err(ZkX509PreprocessedFixedErrorV1::Profile);
        }
        let expected_batches = usize::from(geometry.width)
            .checked_div(ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
            .ok_or(ZkX509PreprocessedFixedErrorV1::Profile)?;
        if usize::from(self.absorbed_batches) != expected_batches || message_bytes % 64 != 2 {
            return Err(ZkX509PreprocessedFixedErrorV1::Profile);
        }
        let mut final_block = [0_u8; 64];
        final_block[..2].copy_from_slice(&self.tail);
        final_block[2] = 0x80;
        final_block[56..].copy_from_slice(
            &message_bytes
                .checked_mul(8)
                .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?
                .to_be_bytes(),
        );
        compress_sha256_block_v1(&mut self.state, &final_block);
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
    if usize::from(geometry.width) % ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1 != 0 {
        return Err(ZkX509PreprocessedFixedErrorV1::Profile);
    }
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
        .ok_or(ZkX509PreprocessedFixedErrorV1::Resource)?;
    if column_end > usize::from(geometry.width) {
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
) -> Result<(Vec<CompactFixedLeafSha256V1>, Vec<Vec<u64>>), ZkX509PreprocessedFixedErrorV1> {
    geometry.validate()?;
    if usize::from(geometry.width) % ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1 != 0 {
        return Err(ZkX509PreprocessedFixedErrorV1::Profile);
    }
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

    for column_start in
        (0..usize::from(geometry.width)).step_by(ZK_X509_PREPROCESSED_FIXED_COLUMN_BATCH_V1)
    {
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
                    .iter()
                    .map(|value| value.0),
            );
        }
        states
            .par_iter_mut()
            .zip(evaluations.par_iter().copied())
            .try_for_each(|(state, row)| state.absorb_batch8_v1(row))?;
    }
    if opened_rows
        .iter()
        .any(|row| row.len() != usize::from(geometry.width))
    {
        return Err(ZkX509PreprocessedFixedErrorV1::Column);
    }
    Ok((states, opened_rows))
}

fn stream_fixed_oracle_root_batch8_v1(
    geometry: ZkX509PreprocessedFixedGeometryV1,
    native_column: impl FnMut(usize) -> Result<Vec<F>, ZkX509PreprocessedFixedErrorV1>,
) -> Result<[u8; 32], ZkX509PreprocessedFixedErrorV1> {
    geometry.validate()?;
    checked_release_root_memory_v1(geometry)?;
    let lde_rows = geometry.lde_rows()?;
    let (states, opened_rows) =
        compact_fixed_leaf_states_and_openings_batch8_v1(geometry, &[], native_column)?;
    if !opened_rows.is_empty() {
        return Err(ZkX509PreprocessedFixedErrorV1::Opening);
    }

    let mut frontier = OrderedMerkleFrontierV1::new_v1(geometry.lde_log2)?;
    for chunk in states.chunks(ZK_X509_PREPROCESSED_FIXED_FINALIZE_CHUNK_ROWS_V1) {
        let leaves = chunk
            .par_iter()
            .copied()
            .map(|state| state.finalize_v1(geometry))
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
    let (states, opened_rows) =
        compact_fixed_leaf_states_and_openings_batch8_v1(geometry, indices, native_column)?;
    let mut frontier = OrderedSelectedMerkleFrontierV1::new_v1(geometry, indices)?;
    for chunk in states.chunks(ZK_X509_PREPROCESSED_FIXED_FINALIZE_CHUNK_ROWS_V1) {
        let leaves = chunk
            .par_iter()
            .copied()
            .map(|state| state.finalize_v1(geometry))
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
    indices: &ZkX509ShaPreprocessedFixedOpeningIndicesV1,
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
/// [`derive_zk_x509_sha_preprocessed_fixed_opening_indices_v1`].
pub(crate) fn verify_zk_x509_sha_preprocessed_fixed_multiproof_v1(
    supplied: ZkX509ShaPreprocessedFixedCertificateV1,
    expected_indices: &ZkX509ShaPreprocessedFixedOpeningIndicesV1,
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
            compact.finalize_v1(TEST_BATCH8_GEOMETRY),
            Err(ZkX509PreprocessedFixedErrorV1::Profile),
            "omitting the sole batch must not produce a leaf"
        );
        compact
            .absorb_batch8_v1(row)
            .expect("sole complete field batch");
        let compact_digest = compact
            .finalize_v1(TEST_BATCH8_GEOMETRY)
            .expect("compact digest");
        let encoded = row.map(|value| value.0);
        assert_eq!(
            compact_digest,
            fixed_leaf_hash_v1(TEST_BATCH8_GEOMETRY, &encoded).expect("canonical leaf")
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

        let mut partial_batch = TEST_BATCH8_GEOMETRY;
        partial_batch.width = 9;
        assert_eq!(
            stream_fixed_oracle_root_batch8_v1(partial_batch, |_| {
                panic!("misaligned geometry must reject before requesting a column")
            }),
            Err(ZkX509PreprocessedFixedErrorV1::Profile)
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
        let derived = derive_zk_x509_sha_preprocessed_fixed_opening_indices_v1(&queries)
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
            derive_zk_x509_sha_preprocessed_fixed_opening_indices_v1(&reordered)
                .expect("transcript order is normalized"),
            derived
        );

        let mut boundary = queries.clone();
        boundary[0] = lde_rows - 1;
        let boundary_openings = derive_zk_x509_sha_preprocessed_fixed_opening_indices_v1(&boundary)
            .expect("last valid query coordinate");
        assert!(boundary_openings.as_slice_v1().contains(&(lde_rows - 1)));
        assert!(
            boundary_openings.as_slice_v1().contains(&63),
            "the verifier-derived stride wraps on the common domain"
        );

        let mut duplicate = queries.clone();
        duplicate[1] = duplicate[0];
        assert_eq!(
            derive_zk_x509_sha_preprocessed_fixed_opening_indices_v1(&duplicate),
            Err(ZkX509PreprocessedFixedErrorV1::Index)
        );

        let mut out_of_range = queries.clone();
        out_of_range[0] = lde_rows;
        assert_eq!(
            derive_zk_x509_sha_preprocessed_fixed_opening_indices_v1(&out_of_range),
            Err(ZkX509PreprocessedFixedErrorV1::Index)
        );
        assert_eq!(
            derive_zk_x509_sha_preprocessed_fixed_opening_indices_v1(&queries[..queries.len() - 1]),
            Err(ZkX509PreprocessedFixedErrorV1::Index)
        );
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

        // Constrained attainability certificate. At level 18 the log25 tree
        // has 128 subtrees. Leave the following 12 vertices unmatched, one in
        // each of 12 distinct binary sibling pairs, and match every remaining
        // adjacent pair. A query placed 64 leaves before the boundary between
        // each matched pair opens exactly one leaf in each subtree because
        // its verifier-derived next coordinate is query + 64.
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
        let maximum_openings =
            derive_zk_x509_sha_preprocessed_fixed_opening_indices_v1(&maximum_queries)
                .expect("legal paired maximum witness");
        let maximum_indices = maximum_openings.as_slice_v1().to_vec();
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
            1_476_395_008
        );
        assert_eq!(ZK_X509_SHA_PREPROCESSED_ROOT_MERKLE_NODES_V1, 33_554_431);
        assert_eq!(
            ZK_X509_SHA_PREPROCESSED_ROOT_MERKLE_SHA_BLOCKS_V1,
            100_663_293
        );
        assert_eq!(ZK_X509_SHA_PREPROCESSED_ROOT_MAX_SECONDS_V1, 3_600);
        checked_release_root_memory_v1(ZK_X509_SHA_PREPROCESSED_FIXED_GEOMETRY_V1)
            .expect("release root RSS certificate");
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
    fn authenticated_reduced_sha_rows_expand_to_the_exact_air_fixed_width() {
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
            assert_eq!(
                expand_zk_x509_sha_preprocessed_fixed_row_v1(&reduced)
                    .expect("authenticated opening expansion")
                    .as_slice(),
                expected,
                "physical row {row}"
            );
        }
    }

    #[test]
    #[ignore = "serialized release diagnostic: 36 row-major batch8 log25 transforms"]
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
