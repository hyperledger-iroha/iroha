//! Canonical numeric aggregate adapter for strict DER.
//!
//! This adapter is deliberately independent of the host parser used to build
//! witnesses. Verification operates on one 76-column numeric micro-trace:
//! byte-consuming parser rows, non-consuming header/boundary rows, and SET OF
//! comparator rows all share the adapter's native `2^19` subgroup. A constant
//! public registration fixes maximum parser/comparator regions; committed
//! private activity prefixes and document metadata bind the exact execution
//! without disclosing its geometry. No row enum, host parse result, or
//! reduced-domain proof is consulted by the extension-domain evaluator.
//!
//! The parser keeps its constructed-value stack as a push/pop permutation.
//! Its byte table is exposed through a logarithmic-derivative lookup terminal,
//! node and SET-pair events through four-lane products, and every terminal is
//! carried to the final aggregate row. RFC 5280 consumes those terminals in
//! its own adapter through the sole complete MAIN aggregate registration.
use super::{
    der_air::{
        ZK_X509_DER_AIR_MAX_DOCUMENTS_V1, ZK_X509_DER_AIR_MAX_EMBEDDED_DOCUMENTS_V1,
        ZK_X509_DER_AIR_UNIVERSAL_SELECTORS_V1,
    },
    der_limits::{ZK_X509_DER_MAX_NESTING_DEPTH_V1, ZK_X509_DER_MAX_VALUES_V1},
    profile::ZK_X509_TRACE_MASK_DEGREE_V1,
};
#[cfg(any(test, feature = "privacy-release-evidence"))]
use super::{
    der_air::{
        ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1, ZkX509DerDocumentTraceV1,
        ZkX509DerNodeRowV1, build_strict_der_document_trace_v1,
    },
    der_limits::ZK_X509_DER_MAX_DOCUMENT_BYTES_V1,
};
use crate::privacy_engines::transparent_stark::{
    GOLDILOCKS_MODULUS_V1, GoldilocksFieldV1 as F, TransparentStarkErrorV1, TransparentTranscriptV1,
};
use thiserror::Error;
/// Stable identity of the fixed-capacity strict-DER numeric adapter.
#[cfg(test)]
pub(crate) const ZK_X509_DER_STARK_AIR_DESCRIPTOR_V1: &[u8] = b"zk-x509-der-stark-air-v1-incompatible:native-log19:base76:aux196:fixed14:constraints898:degree7:two-base-and-four-aux-physical-chunks:registered-expression-degree-ceiling7:multi-direction-affine-audit-attains-seven:mask-multiplier-degree801:mask-coefficients802:quotient-bound3151335:quotient-coset-capacity4194303:fri-chunk-capacity1048575:four-chunk-composition-capacity4194303:zero-sized-public-shape:constant-registration-transcript:no-private-document-count-length-parser-or-comparator-disclosure:committed-private-parser-and-comparator-active-prefixes:canonical-inactive-rows:carried-private-document-count-range-bound:parser-cap65536:comparator-cap262144:padding196608:proof-document-max4096:proof-total-document-bytes32768:generic-oracle-max16384:streaming-byte-parser:identifier-u32-base128-minimal:length-definite-minimal-max16384:node-count-max2048:depth-max16:constructed-frame-push-pop-four-lane-product:universal-tag-one-hot-without-witness-branch:primitive-boolean-null-integer-enumerated-oid-bit-string:set-pair-four-lane-product:set-byte-zero-safe-log-derivative-with-singular-count-equality:input-byte-and-node-event-four-lane-products:private-document-product-internal-not-public:verifier-fixed-parser-and-comparator-and-padding-ranges:cross-adapter-claims:rfc5280-and-byte-memory-consumer-registrations=complete:integration=complete-via-main-aggregate:standalone-activation=not-applicable";
#[cfg(test)]
pub(crate) const ZK_X509_DER_STARK_AIR_DESCRIPTOR_SHA256_V1: [u8; 32] = [
    0xd5, 0x2f, 0xc3, 0x6d, 0x71, 0x55, 0xc6, 0x4c, 0xa1, 0xe9, 0xe0, 0x1f, 0x96, 0x8b, 0xac, 0x70,
    0x21, 0xc9, 0x2f, 0x18, 0x1e, 0x41, 0x5b, 0x45, 0x20, 0x72, 0x19, 0x94, 0x71, 0x94, 0xb5, 0x83,
];
/// The aggregate native domain shared with SHA, projection, and every bus.
pub(crate) const ZK_X509_DER_STARK_TRACE_LOG2_V1: u8 = 19;
pub(crate) const ZK_X509_DER_STARK_TRACE_SIZE_V1: usize = 1 << ZK_X509_DER_STARK_TRACE_LOG2_V1;
/// Base columns, including committed private activity and document-count
/// metadata.
pub(crate) const ZK_X509_DER_STARK_BASE_WIDTH_V1: usize = 76;
pub(crate) const ZK_X509_DER_STARK_AUX_WIDTH_V1: usize = 196;
pub(crate) const ZK_X509_DER_STARK_FIXED_WIDTH_V1: usize = 14;
pub(crate) const ZK_X509_DER_STARK_CONSTRAINT_COUNT_V1: usize = 898;
/// Registered local-expression ceiling. The complete numeric evaluator
/// independently attains degree seven over multiple affine row directions.
pub(crate) const ZK_X509_DER_STARK_CONSTRAINT_DEGREE_V1: u8 = 7;
pub(crate) const ZK_X509_DER_STARK_MAXIMUM_QUOTIENT_DEGREE_V1: usize =
    ZK_X509_DER_STARK_CONSTRAINT_DEGREE_V1 as usize
        * (ZK_X509_DER_STARK_TRACE_SIZE_V1 + ZK_X509_TRACE_MASK_DEGREE_V1 as usize)
        - ZK_X509_DER_STARK_TRACE_SIZE_V1;
const _: () = assert!(ZK_X509_TRACE_MASK_DEGREE_V1 == 801);
const _: () = assert!(ZK_X509_DER_STARK_MAXIMUM_QUOTIENT_DEGREE_V1 == 3_151_335);
/// Four independent copy/lookup lanes.
pub(crate) const ZK_X509_DER_STARK_BUS_LANES_V1: usize = 4;
/// Maximum number of top-level plus extension-embedded documents.
pub(crate) const ZK_X509_DER_STARK_MAX_DOCUMENTS_V1: usize =
    ZK_X509_DER_AIR_MAX_DOCUMENTS_V1 + ZK_X509_DER_AIR_MAX_EMBEDDED_DOCUMENTS_V1;
/// Top-level plus embedded bytes admitted by the proof-facing DER adapter.
pub(crate) const ZK_X509_DER_STARK_MAX_TOTAL_DOCUMENT_BYTES_V1: usize = 32_768;
/// Defensive proof-facing SET comparator cap.
pub(crate) const ZK_X509_DER_STARK_MAX_COMPARATOR_ROWS_V1: usize = 262_144;
/// Fixed parser registration capacity. Every input byte contributes one row
/// and every DER node contributes exactly two non-consuming rows; at most
/// `total_bytes / 2` nodes are possible.
pub(crate) const ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1: usize =
    2 * ZK_X509_DER_STARK_MAX_TOTAL_DOCUMENT_BYTES_V1;
/// Fixed public parser/comparator registration envelope.
pub(crate) const ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1: usize =
    ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 + ZK_X509_DER_STARK_MAX_COMPARATOR_ROWS_V1;
/// Canonical inactive tail completing the sole first-release trace domain.
pub(crate) const ZK_X509_DER_STARK_FIXED_PADDING_ROWS_V1: usize =
    ZK_X509_DER_STARK_TRACE_SIZE_V1 - ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1;
const _: () =
    assert!(ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1 < ZK_X509_DER_STARK_TRACE_SIZE_V1);
const _: () = assert!(
    ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1 + ZK_X509_DER_STARK_FIXED_PADDING_ROWS_V1
        == ZK_X509_DER_STARK_TRACE_SIZE_V1
);
const DER_TUPLE_CHALLENGE_LABELS_V1: [&[u8]; 12] = [
    b"zk-x509-der-bus-tuple-slot-00-v1",
    b"zk-x509-der-bus-tuple-slot-01-v1",
    b"zk-x509-der-bus-tuple-slot-02-v1",
    b"zk-x509-der-bus-tuple-slot-03-v1",
    b"zk-x509-der-bus-tuple-slot-04-v1",
    b"zk-x509-der-bus-tuple-slot-05-v1",
    b"zk-x509-der-bus-tuple-slot-06-v1",
    b"zk-x509-der-bus-tuple-slot-07-v1",
    b"zk-x509-der-bus-tuple-slot-08-v1",
    b"zk-x509-der-bus-tuple-slot-09-v1",
    b"zk-x509-der-bus-tuple-slot-10-v1",
    b"zk-x509-der-bus-tuple-slot-11-v1",
];
const DER_BYTE_LOOKUP_CHALLENGE_LABEL_V1: &[u8] = b"zk-x509-der-byte-lookup-shift-v1";
// Parser row columns. Comparator rows deliberately reinterpret the same
// physical cells; fixed family selectors choose the numeric equations.
const BASE_DOCUMENT: usize = 0;
const BASE_DOCUMENT_LEN: usize = 1;
const BASE_OFFSET: usize = 2;
const BASE_BYTE_VALUE: usize = 3;
const BASE_BYTE_BITS: usize = 4;
const BASE_PHASE_BITS: usize = 12;
const BASE_TAG_CLASS: usize = 15;
const BASE_TAG_CLASS_BITS: usize = 16;
const BASE_CONSTRUCTED: usize = 18;
const BASE_TAG_ACCUMULATOR: usize = 19;
const BASE_LENGTH_ACCUMULATOR: usize = 20;
const BASE_CONTENT_START: usize = 21;
const BASE_CONTENT_END: usize = 22;
const BASE_NODE_START: usize = 23;
const BASE_NODE_COUNT: usize = 24;
const BASE_DEPTH_BITS: usize = 25;
const BASE_FRAME_ID: usize = 30;
const BASE_FRAME_START: usize = 31;
const BASE_FRAME_END: usize = 32;
const BASE_FRAME_IS_SET: usize = 33;
const BASE_FRAME_HAS_CHILD: usize = 34;
const BASE_FRAME_PREVIOUS_ID: usize = 35;
const BASE_FRAME_PREVIOUS_START: usize = 36;
const BASE_FRAME_PREVIOUS_END: usize = 37;
const BASE_CHECK_IS_ZERO: usize = 38;
const BASE_CHECK_INVERSE: usize = 39;
const BASE_PAYLOAD: usize = 40;
const BASE_BYTE_LOOKUP_MULTIPLICITY: usize = 59;
const BASE_PRIMITIVE_FIRST: usize = 60;
const BASE_OID_START: usize = 61;
const BASE_UNUSED_BITS: usize = 62;
const BASE_DOCUMENT_FIRST: usize = 63;
const BASE_ROW_ACTIVE: usize = 64;
const BASE_FINAL_DOCUMENT: usize = 65;
const BASE_FINAL_DOCUMENT_BITS: usize = 66;
const BASE_FINAL_DOCUMENT_SLACK_BITS: usize = 71;
const PHASE_IDENTIFIER_FIRST: usize = 0;
const PHASE_IDENTIFIER_HIGH: usize = 1;
const PHASE_LENGTH_FIRST: usize = 2;
const PHASE_LENGTH_BODY: usize = 3;
const PHASE_FINALIZE_HEADER: usize = 4;
const PHASE_PRIMITIVE_CONTENT: usize = 5;
const PHASE_BOUNDARY: usize = 6;
#[cfg(any(test, feature = "privacy-release-evidence"))]
const PHASE_SET_COMPARATOR: usize = 7;
const UNIVERSAL_TAGS_V1: [u32; ZK_X509_DER_AIR_UNIVERSAL_SELECTORS_V1] = [
    1, 2, 3, 4, 5, 6, 10, 12, 16, 17, 18, 19, 20, 22, 23, 24, 26, 28, 30,
];
pub(crate) const FIX_ACTIVE: usize = 0;
pub(crate) const FIX_FIRST_ACTIVE: usize = 1;
pub(crate) const FIX_LAST_ACTIVE: usize = 2;
pub(crate) const FIX_PARSER: usize = 3;
pub(crate) const FIX_FIRST_PARSER: usize = 4;
pub(crate) const FIX_LAST_PARSER: usize = 5;
pub(crate) const FIX_COMPARATOR: usize = 6;
pub(crate) const FIX_FIRST_COMPARATOR: usize = 7;
pub(crate) const FIX_LAST_COMPARATOR: usize = 8;
pub(crate) const FIX_PADDING: usize = 9;
pub(crate) const FIX_FIRST_AGGREGATE: usize = 10;
pub(crate) const FIX_LAST_AGGREGATE: usize = 11;
pub(crate) const FIX_FINAL_DOCUMENT: usize = 12;
pub(crate) const FIX_PARSER_CONTINUE: usize = 13;
const AUX_STACK_PUSH_BEFORE: usize = 0;
const AUX_STACK_POP_BEFORE: usize = AUX_STACK_PUSH_BEFORE + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_STACK_PUSH_AFTER: usize = AUX_STACK_POP_BEFORE + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_STACK_POP_AFTER: usize = AUX_STACK_PUSH_AFTER + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_DOCUMENT_BEFORE: usize = AUX_STACK_POP_AFTER + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_DOCUMENT_AFTER: usize = AUX_DOCUMENT_BEFORE + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_NODE_BEFORE: usize = AUX_DOCUMENT_AFTER + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_NODE_AFTER: usize = AUX_NODE_BEFORE + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_PAIR_PRODUCER_BEFORE: usize = AUX_NODE_AFTER + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_PAIR_CONSUMER_BEFORE: usize = AUX_PAIR_PRODUCER_BEFORE + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_PAIR_PRODUCER_AFTER: usize = AUX_PAIR_CONSUMER_BEFORE + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_PAIR_CONSUMER_AFTER: usize = AUX_PAIR_PRODUCER_AFTER + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_BYTE_TABLE_SUM_BEFORE: usize = AUX_PAIR_CONSUMER_AFTER + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_BYTE_QUERY_SUM_BEFORE: usize = AUX_BYTE_TABLE_SUM_BEFORE + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_BYTE_TABLE_SUM_AFTER: usize = AUX_BYTE_QUERY_SUM_BEFORE + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_BYTE_QUERY_SUM_AFTER: usize = AUX_BYTE_TABLE_SUM_AFTER + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_BYTE_TABLE_INVERSE: usize = AUX_BYTE_QUERY_SUM_AFTER + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_BYTE_LEFT_QUERY_INVERSE: usize = AUX_BYTE_TABLE_INVERSE + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_BYTE_RIGHT_QUERY_INVERSE: usize =
    AUX_BYTE_LEFT_QUERY_INVERSE + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_BYTE_TABLE_ZERO: usize = AUX_BYTE_RIGHT_QUERY_INVERSE + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_BYTE_LEFT_QUERY_ZERO: usize = AUX_BYTE_TABLE_ZERO + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_BYTE_RIGHT_QUERY_ZERO: usize = AUX_BYTE_LEFT_QUERY_ZERO + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_BYTE_TABLE_ZERO_COUNT_BEFORE: usize =
    AUX_BYTE_RIGHT_QUERY_ZERO + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_BYTE_QUERY_ZERO_COUNT_BEFORE: usize =
    AUX_BYTE_TABLE_ZERO_COUNT_BEFORE + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_BYTE_TABLE_ZERO_COUNT_AFTER: usize =
    AUX_BYTE_QUERY_ZERO_COUNT_BEFORE + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_BYTE_QUERY_ZERO_COUNT_AFTER: usize =
    AUX_BYTE_TABLE_ZERO_COUNT_AFTER + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_INPUT_BYTE_BEFORE: usize =
    AUX_BYTE_QUERY_ZERO_COUNT_AFTER + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_INPUT_BYTE_AFTER: usize = AUX_INPUT_BYTE_BEFORE + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_PHASE_SELECTORS: usize = AUX_INPUT_BYTE_AFTER + ZK_X509_DER_STARK_BUS_LANES_V1;
const AUX_DEPTH_SELECTORS: usize = AUX_PHASE_SELECTORS + 8;
const AUX_IDENTIFIER_COUNT_SELECTORS: usize = AUX_DEPTH_SELECTORS + 17;
const AUX_PRIMITIVE_KIND_SELECTORS: usize = AUX_IDENTIFIER_COUNT_SELECTORS + 8;
const AUX_UNUSED_BIT_SELECTORS: usize = AUX_PRIMITIVE_KIND_SELECTORS + 8;
const AUX_LENGTH_REMAINING_SELECTORS: usize = AUX_UNUSED_BIT_SELECTORS + 8;
const AUX_LOW_FIVE_PAIR_01: usize = AUX_LENGTH_REMAINING_SELECTORS + 4;
const AUX_LOW_FIVE_PAIR_23: usize = AUX_LOW_FIVE_PAIR_01 + 1;
const AUX_HIGH_TAG: usize = AUX_LOW_FIVE_PAIR_23 + 1;
const AUX_HIGH_LOW_ZERO: usize = AUX_HIGH_TAG + 1;
const AUX_HIGH_LOW_INVERSE: usize = AUX_HIGH_LOW_ZERO + 1;
const AUX_HIGH_LOW_GE_31: usize = AUX_HIGH_LOW_INVERSE + 1;
const AUX_LENGTH_COUNT_TWO: usize = AUX_HIGH_LOW_GE_31 + 1;
const AUX_BYTE_ZERO: usize = AUX_LENGTH_COUNT_TWO + 1;
const AUX_BYTE_INVERSE: usize = AUX_BYTE_ZERO + 1;
const AUX_BYTE_IS_64: usize = AUX_BYTE_INVERSE + 1;
const AUX_BYTE_64_INVERSE: usize = AUX_BYTE_IS_64 + 1;
const AUX_BYTE_IS_128: usize = AUX_BYTE_64_INVERSE + 1;
const AUX_BYTE_128_INVERSE: usize = AUX_BYTE_IS_128 + 1;
const AUX_UPDATED_FIRST_HIGH_BITS: usize = AUX_BYTE_128_INVERSE + 1;
const AUX_SIGNED_FIRST_GUARD: usize = AUX_UPDATED_FIRST_HIGH_BITS + 7;
const AUX_BIT_STRING_FIRST_GUARD: usize = AUX_SIGNED_FIRST_GUARD + 1;
const AUX_BIT_STRING_LAST_CONTINUATION_GUARD: usize = AUX_BIT_STRING_FIRST_GUARD + 1;
const AUX_NEXT_OID_START_EXPECTED: usize = AUX_BIT_STRING_LAST_CONTINUATION_GUARD + 1;
const AUX_CHECK_DELTA: usize = AUX_NEXT_OID_START_EXPECTED + 1;
const AUX_ROOT_COMPLETION: usize = AUX_CHECK_DELTA + 1;
const AUX_BOUNDARY_NOT_ROOT: usize = AUX_ROOT_COMPLETION + 1;
const AUX_BOUNDARY_COMPLETES_PARENT: usize = AUX_BOUNDARY_NOT_ROOT + 1;
const AUX_PAIR_PRODUCER_EVENT: usize = AUX_BOUNDARY_COMPLETES_PARENT + 1;
const AUX_PRIMITIVE_ENTRY: usize = AUX_PAIR_PRODUCER_EVENT + 1;
const AUX_ENTERS_CHILD: usize = AUX_PRIMITIVE_ENTRY + 1;
const _: () = assert!(AUX_ENTERS_CHILD + 1 == ZK_X509_DER_STARK_AUX_WIDTH_V1);
/// Private proof geometry committed inside the DER base trace.
#[cfg(any(test, feature = "privacy-release-evidence"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509DerStarkPrivateShapeV1 {
    pub(crate) document_lengths: Vec<u16>,
    pub(crate) parser_rows: usize,
    pub(crate) comparator_rows: usize,
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
impl ZkX509DerStarkPrivateShapeV1 {
    pub(crate) fn active_rows(&self) -> Result<usize, ZkX509DerStarkErrorV1> {
        self.parser_rows
            .checked_add(self.comparator_rows)
            .filter(|rows| *rows != 0 && *rows <= ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1)
            .ok_or(ZkX509DerStarkErrorV1::Resource)
    }
    pub(crate) fn validate(&self) -> Result<(), ZkX509DerStarkErrorV1> {
        if self.document_lengths.is_empty()
            || self.document_lengths.len() > ZK_X509_DER_STARK_MAX_DOCUMENTS_V1
            || self.parser_rows == 0
            || self.document_lengths.iter().any(|length| {
                *length == 0
                    || usize::from(*length) > ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1
            })
        {
            return Err(ZkX509DerStarkErrorV1::Shape);
        }
        // At least one byte row, one finalize row, and one boundary row are
        // required per document. This is only a resource lower bound; the AIR
        // proves the exact execution.
        let minimum_parser_rows = self
            .document_lengths
            .iter()
            .try_fold(0_usize, |sum, length| {
                sum.checked_add(usize::from(*length))
                    .and_then(|value| value.checked_add(2))
            });
        if minimum_parser_rows.is_none_or(|minimum| self.parser_rows < minimum) {
            return Err(ZkX509DerStarkErrorV1::Shape);
        }
        // Every DER node owns at least one identifier and one length octet,
        // so a document of `length` bytes can contain at most `length / 2`
        // nodes (and never more than the closed-profile node cap). The parser
        // emits each input byte exactly once plus one finalize and one
        // boundary row per node.
        let total_bytes = self
            .document_lengths
            .iter()
            .try_fold(0_usize, |sum, length| sum.checked_add(usize::from(*length)))
            .ok_or(ZkX509DerStarkErrorV1::Resource)?;
        if total_bytes > ZK_X509_DER_STARK_MAX_TOTAL_DOCUMENT_BYTES_V1 {
            return Err(ZkX509DerStarkErrorV1::Shape);
        }
        let maximum_nodes = self
            .document_lengths
            .iter()
            .try_fold(0_usize, |sum, length| {
                sum.checked_add((usize::from(*length) / 2).min(ZK_X509_DER_MAX_VALUES_V1))
            })
            .ok_or(ZkX509DerStarkErrorV1::Resource)?;
        let maximum_parser_rows = total_bytes
            .checked_add(
                maximum_nodes
                    .checked_mul(2)
                    .ok_or(ZkX509DerStarkErrorV1::Resource)?,
            )
            .ok_or(ZkX509DerStarkErrorV1::Resource)?;
        // At one nesting depth, children of all SET values cover disjoint
        // document spans. For child lengths a_i, every comparison has length
        // min(a_i, a_{i+1}), and the sum of those adjacent minima is at most
        // the sum of all a_i. Thus one depth contributes at most the document
        // length, even though an interior child participates in two pairs.
        let maximum_comparator_rows = total_bytes
            .checked_mul(ZK_X509_DER_MAX_NESTING_DEPTH_V1)
            .ok_or(ZkX509DerStarkErrorV1::Resource)?
            .min(ZK_X509_DER_STARK_MAX_COMPARATOR_ROWS_V1);
        if self.parser_rows > maximum_parser_rows
            || self.parser_rows > ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1
            || self.comparator_rows > maximum_comparator_rows
            || self.comparator_rows > ZK_X509_DER_STARK_MAX_COMPARATOR_ROWS_V1
        {
            return Err(ZkX509DerStarkErrorV1::Shape);
        }
        self.active_rows()?;
        Ok(())
    }
}
/// Public fixed-capacity DER registration shape. It intentionally contains no
/// private document count, length, parser count, or comparator count.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ZkX509DerStarkShapeV1;
impl ZkX509DerStarkShapeV1 {
    pub(crate) const fn validate(&self) -> Result<(), ZkX509DerStarkErrorV1> {
        Ok(())
    }
    #[cfg(test)]
    pub(crate) const fn active_rows(&self) -> usize {
        ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1
    }
    #[cfg(test)]
    pub(crate) const fn transcript_bytes(&self) -> &'static [u8] {
        b"zk-x509-der-stark-fixed-registration-v1"
    }
}
/// Verifier-owned constant fixed schedule; it never stores native fixed rows.
#[cfg(any(test, feature = "privacy-release-evidence"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509DerStarkFixedScheduleV1;
#[cfg(any(test, feature = "privacy-release-evidence"))]
impl ZkX509DerStarkFixedScheduleV1 {
    pub(crate) const fn active_rows(&self) -> usize {
        ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1
    }
    #[cfg(test)]
    pub(crate) const fn aggregate_rows(&self) -> usize {
        ZK_X509_DER_STARK_TRACE_SIZE_V1
    }
    pub(crate) fn fixed_row(
        &self,
        index: usize,
    ) -> Result<[F; ZK_X509_DER_STARK_FIXED_WIDTH_V1], ZkX509DerStarkErrorV1> {
        if index >= ZK_X509_DER_STARK_TRACE_SIZE_V1 {
            return Err(ZkX509DerStarkErrorV1::Resource);
        }
        let mut row = [F::ZERO; ZK_X509_DER_STARK_FIXED_WIDTH_V1];
        row[FIX_FIRST_AGGREGATE] = F(u64::from(index == 0));
        row[FIX_LAST_AGGREGATE] = F(u64::from(index + 1 == ZK_X509_DER_STARK_TRACE_SIZE_V1));
        if index >= ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1 {
            row[FIX_PADDING] = F::ONE;
            return Ok(row);
        }
        row[FIX_ACTIVE] = F::ONE;
        row[FIX_FIRST_ACTIVE] = F(u64::from(index == 0));
        row[FIX_LAST_ACTIVE] = F(u64::from(
            index + 1 == ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1,
        ));
        if index < ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 {
            row[FIX_PARSER] = F::ONE;
            row[FIX_FIRST_PARSER] = F(u64::from(index == 0));
            row[FIX_LAST_PARSER] = F(u64::from(index + 1 == ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1));
            row[FIX_PARSER_CONTINUE] =
                F(u64::from(index + 1 < ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1));
        } else {
            row[FIX_COMPARATOR] = F::ONE;
            row[FIX_FIRST_COMPARATOR] = F(u64::from(index == ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1));
            row[FIX_LAST_COMPARATOR] = F(u64::from(
                index + 1 == ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1,
            ));
        }
        Ok(row)
    }
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn compile_zk_x509_der_stark_fixed_schedule_v1(
    shape: ZkX509DerStarkShapeV1,
) -> Result<ZkX509DerStarkFixedScheduleV1, ZkX509DerStarkErrorV1> {
    shape.validate()?;
    Ok(ZkX509DerStarkFixedScheduleV1)
}
/// Transcript challenges used by stack, event, and byte lookup buses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509DerStarkChallengesV1 {
    /// Domain-separated tuple-compression coefficients.
    pub(crate) tuple: [[F; 12]; ZK_X509_DER_STARK_BUS_LANES_V1],
    /// Logarithmic-derivative byte lookup points.
    pub(crate) byte_lookup: [F; ZK_X509_DER_STARK_BUS_LANES_V1],
}
impl ZkX509DerStarkChallengesV1 {
    pub(crate) fn validate(self) -> Result<(), ZkX509DerStarkErrorV1> {
        for lane in 0..ZK_X509_DER_STARK_BUS_LANES_V1 {
            if self.tuple[lane]
                .iter()
                .chain(core::iter::once(&self.byte_lookup[lane]))
                .any(|value| {
                    value.0 == 0
                        || value.0 >= GOLDILOCKS_MODULUS_V1
                        || F::canonical(value.0).is_none()
                })
            {
                return Err(ZkX509DerStarkErrorV1::Challenge);
            }
            if self.tuple[lane]
                .iter()
                .enumerate()
                .any(|(index, value)| self.tuple[lane][..index].contains(value))
                || self.tuple[..lane].contains(&self.tuple[lane])
                || self.byte_lookup[..lane].contains(&self.byte_lookup[lane])
            {
                return Err(ZkX509DerStarkErrorV1::Challenge);
            }
        }
        Ok(())
    }
}
/// Derive the strict-DER copy and lookup challenges in canonical lane-major
/// order after the DER base commitment and before constructing its auxiliary
/// trace.
///
/// Each lane samples all twelve tuple slots followed by its byte-lookup shift.
/// The labels are stable even though a tuple slot has adapter-specific meaning;
/// its numeric position, rather than a witness-selected event kind, determines
/// the compression coefficient.
pub(crate) fn derive_zk_x509_der_stark_challenges_v1(
    transcript: &mut TransparentTranscriptV1,
) -> Result<ZkX509DerStarkChallengesV1, TransparentStarkErrorV1> {
    let mut challenges = ZkX509DerStarkChallengesV1 {
        tuple: [[F::ZERO; 12]; ZK_X509_DER_STARK_BUS_LANES_V1],
        byte_lookup: [F::ZERO; ZK_X509_DER_STARK_BUS_LANES_V1],
    };
    for lane in 0..ZK_X509_DER_STARK_BUS_LANES_V1 {
        for (coefficient, label) in challenges.tuple[lane]
            .iter_mut()
            .zip(DER_TUPLE_CHALLENGE_LABELS_V1)
        {
            *coefficient = transcript.challenge_field(label)?;
        }
        challenges.byte_lookup[lane] =
            transcript.challenge_field(DER_BYTE_LOOKUP_CHALLENGE_LABEL_V1)?;
    }
    Ok(challenges)
}
/// Numeric DER adapter failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum ZkX509DerStarkErrorV1 {
    #[cfg(any(test, feature = "privacy-release-evidence"))]
    #[error("zk-X509 DER STARK public shape is invalid")]
    Shape,
    #[cfg(any(test, feature = "privacy-release-evidence"))]
    #[error("zk-X509 DER STARK resource envelope is exceeded")]
    Resource,
    #[error("zk-X509 DER STARK transcript challenge is invalid")]
    Challenge,
    #[error("zk-X509 DER STARK numeric row is malformed")]
    Row,
    #[error("zk-X509 DER STARK trace transition is invalid")]
    Transition,
}
/// Base trace before challenge-dependent bus products are populated.
#[cfg(any(test, feature = "privacy-release-evidence"))]
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ZkX509DerStarkBaseV1 {
    pub(crate) private_shape: ZkX509DerStarkPrivateShapeV1,
    /// Exact active parser rows followed by exact SET comparator rows.
    pub(crate) rows: Vec<[F; ZK_X509_DER_STARK_BASE_WIDTH_V1]>,
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
impl core::fmt::Debug for ZkX509DerStarkBaseV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZkX509DerStarkBaseV1 { <private material redacted> }")
    }
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
impl ZkX509DerStarkBaseV1 {
    /// Recursively overwrite all private geometry and committed field rows.
    pub(crate) fn zeroize_private_v1(&mut self) {
        self.private_shape.document_lengths.fill(0);
        self.private_shape.document_lengths.clear();
        self.private_shape.parser_rows = 0;
        self.private_shape.comparator_rows = 0;
        for row in &mut self.rows {
            row.fill(F::ZERO);
        }
        self.rows.clear();
    }
    #[cfg(test)]
    pub(crate) fn private_is_zeroized_v1(&self) -> bool {
        self.private_shape.document_lengths.is_empty()
            && self.private_shape.parser_rows == 0
            && self.private_shape.comparator_rows == 0
            && self.rows.is_empty()
    }
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn zk_x509_der_stark_compact_row_native_index_v1(
    shape: &ZkX509DerStarkPrivateShapeV1,
    compact_index: usize,
) -> Result<usize, ZkX509DerStarkErrorV1> {
    if compact_index >= shape.active_rows()? {
        return Err(ZkX509DerStarkErrorV1::Resource);
    }
    if compact_index < shape.parser_rows {
        Ok(compact_index)
    } else {
        ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1
            .checked_add(compact_index - shape.parser_rows)
            .ok_or(ZkX509DerStarkErrorV1::Resource)
    }
}
/// Fixed public terminal registration.
///
/// Document lengths and the number of DER documents are private. In
/// particular, a challenge-dependent product of `(document, length)` tuples
/// must not be exposed as a public claim: it would provide an efficient
/// offline dictionary oracle for the short, highly structured length vector.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ZkX509DerStarkPublicTerminalsV1;
/// Prover-supplied cross-adapter terminal claims.
///
/// These values are absorbed after auxiliary roots and before composition
/// challenges. The final-row identities bind them to the committed DER
/// accumulators; byte-memory and RFC 5280 adapters consume them in the same
/// verifier-fixed role order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509DerStarkTerminalClaimsV1 {
    pub(crate) input_byte: [F; ZK_X509_DER_STARK_BUS_LANES_V1],
    pub(crate) node: [F; ZK_X509_DER_STARK_BUS_LANES_V1],
}
/// Exact node-event fields exported to the RFC 5280 consumer.
///
/// The order is the DER adapter's committed node tuple order.  Exposing the
/// typed event and compression helper avoids a second, subtly divergent host
/// encoding in a downstream adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509DerStarkNodeEventV1 {
    pub(crate) document: F,
    pub(crate) ordinal: F,
    pub(crate) parent_frame: F,
    pub(crate) tag_class: F,
    pub(crate) tag_number: F,
    pub(crate) constructed: F,
    pub(crate) start: F,
    pub(crate) content_start: F,
    pub(crate) content_end: F,
    pub(crate) depth: F,
    pub(crate) content_len: F,
}
/// Challenge-dependent strict-DER trace. Only active rows are materialized;
/// aggregate padding rows are reconstructed from the final accumulators.
#[cfg(any(test, feature = "privacy-release-evidence"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509DerStarkTraceV1 {
    pub(crate) base: ZkX509DerStarkBaseV1,
    pub(crate) aux_rows: Vec<[F; ZK_X509_DER_STARK_AUX_WIDTH_V1]>,
}
/// Final bus values exported by the adapter.
///
/// Stack, SET-pair, and lookup terminals must close internally. Node and
/// input-byte terminals are exported for RFC 5280 and byte-memory consumers.
#[cfg(any(test, feature = "privacy-release-evidence"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509DerStarkTerminalsV1 {
    pub(crate) stack_push: [F; ZK_X509_DER_STARK_BUS_LANES_V1],
    pub(crate) stack_pop: [F; ZK_X509_DER_STARK_BUS_LANES_V1],
    pub(crate) document: [F; ZK_X509_DER_STARK_BUS_LANES_V1],
    pub(crate) node: [F; ZK_X509_DER_STARK_BUS_LANES_V1],
    pub(crate) pair_producer: [F; ZK_X509_DER_STARK_BUS_LANES_V1],
    pub(crate) pair_consumer: [F; ZK_X509_DER_STARK_BUS_LANES_V1],
    pub(crate) byte_table_sum: [F; ZK_X509_DER_STARK_BUS_LANES_V1],
    pub(crate) byte_query_sum: [F; ZK_X509_DER_STARK_BUS_LANES_V1],
    pub(crate) byte_table_zero_count: [F; ZK_X509_DER_STARK_BUS_LANES_V1],
    pub(crate) byte_query_zero_count: [F; ZK_X509_DER_STARK_BUS_LANES_V1],
    pub(crate) input_byte: [F; ZK_X509_DER_STARK_BUS_LANES_V1],
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct FrameV1 {
    id: u64,
    start: u64,
    end: u64,
    is_set: bool,
    has_child: bool,
    previous_id: u64,
    previous_start: u64,
    previous_end: u64,
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct ParserStateV1 {
    document: u64,
    document_len: u64,
    offset: u64,
    phase: usize,
    tag_class: u8,
    constructed: bool,
    tag_accumulator: u64,
    length_accumulator: u64,
    content_start: u64,
    content_end: u64,
    node_start: u64,
    node_count: u64,
    depth: u8,
    frame: FrameV1,
    primitive_first: bool,
    oid_start: bool,
    unused_bits: u8,
    document_first: bool,
    identifier_count: u8,
    first_high_group: u8,
    length_remaining: u8,
    long_length_two: bool,
    length_first_was_64: bool,
    primitive_kind: u8,
    check_delta: u64,
    finalize_selectors: [F; ZK_X509_DER_AIR_UNIVERSAL_SELECTORS_V1],
    boundary_parent: FrameV1,
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
impl ParserStateV1 {
    fn for_document(document: usize, document_len: usize) -> Result<Self, ZkX509DerStarkErrorV1> {
        Ok(Self {
            document: u64::try_from(document).map_err(|_| ZkX509DerStarkErrorV1::Resource)?,
            document_len: u64::try_from(document_len)
                .map_err(|_| ZkX509DerStarkErrorV1::Resource)?,
            offset: 0,
            phase: PHASE_IDENTIFIER_FIRST,
            tag_class: 0,
            constructed: false,
            tag_accumulator: 0,
            length_accumulator: 0,
            content_start: 0,
            content_end: 0,
            node_start: 0,
            node_count: 0,
            depth: 0,
            frame: FrameV1::default(),
            primitive_first: false,
            oid_start: false,
            unused_bits: 0,
            document_first: true,
            identifier_count: 0,
            first_high_group: 0,
            length_remaining: 0,
            long_length_two: false,
            length_first_was_64: false,
            primitive_kind: 0,
            check_delta: u64::try_from(ZK_X509_DER_MAX_VALUES_V1).expect("DER node cap fits u64"),
            finalize_selectors: [F::ZERO; ZK_X509_DER_AIR_UNIVERSAL_SELECTORS_V1],
            boundary_parent: FrameV1::default(),
        })
    }
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn write_bits_v1(
    row: &mut [F; ZK_X509_DER_STARK_BASE_WIDTH_V1],
    start: usize,
    bits: usize,
    value: u64,
) {
    for bit in 0..bits {
        row[start + bit] = F((value >> bit) & 1);
    }
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn inverse_or_zero_v1(value: u64) -> F {
    if value == 0 {
        F::ZERO
    } else {
        F(value).inv().expect("nonzero canonical bounded value")
    }
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn encode_parser_state_v1(
    state: &ParserStateV1,
    byte: Option<u8>,
) -> Result<[F; ZK_X509_DER_STARK_BASE_WIDTH_V1], ZkX509DerStarkErrorV1> {
    if state.phase >= PHASE_SET_COMPARATOR
        || state.document_len == 0
        || state.document_len > u64::try_from(ZK_X509_DER_MAX_DOCUMENT_BYTES_V1).expect("limit")
        || state.offset > state.document_len
        || usize::from(state.depth) > ZK_X509_DER_MAX_NESTING_DEPTH_V1
    {
        return Err(ZkX509DerStarkErrorV1::Row);
    }
    let mut row = [F::ZERO; ZK_X509_DER_STARK_BASE_WIDTH_V1];
    row[BASE_DOCUMENT] = F(state.document);
    row[BASE_DOCUMENT_LEN] = F(state.document_len);
    row[BASE_OFFSET] = F(state.offset);
    if let Some(byte) = byte {
        row[BASE_BYTE_VALUE] = F(u64::from(byte));
        write_bits_v1(&mut row, BASE_BYTE_BITS, 8, u64::from(byte));
    }
    write_bits_v1(
        &mut row,
        BASE_PHASE_BITS,
        3,
        u64::try_from(state.phase).map_err(|_| ZkX509DerStarkErrorV1::Resource)?,
    );
    row[BASE_TAG_CLASS] = F(u64::from(state.tag_class));
    write_bits_v1(&mut row, BASE_TAG_CLASS_BITS, 2, u64::from(state.tag_class));
    row[BASE_CONSTRUCTED] = F(u64::from(state.constructed));
    row[BASE_TAG_ACCUMULATOR] = F(state.tag_accumulator);
    row[BASE_LENGTH_ACCUMULATOR] = F(state.length_accumulator);
    row[BASE_CONTENT_START] = F(state.content_start);
    row[BASE_CONTENT_END] = F(state.content_end);
    row[BASE_NODE_START] = F(state.node_start);
    row[BASE_NODE_COUNT] = F(state.node_count);
    write_bits_v1(&mut row, BASE_DEPTH_BITS, 5, u64::from(state.depth));
    row[BASE_FRAME_ID] = F(state.frame.id);
    row[BASE_FRAME_START] = F(state.frame.start);
    row[BASE_FRAME_END] = F(state.frame.end);
    row[BASE_FRAME_IS_SET] = F(u64::from(state.frame.is_set));
    row[BASE_FRAME_HAS_CHILD] = F(u64::from(state.frame.has_child));
    row[BASE_FRAME_PREVIOUS_ID] = F(state.frame.previous_id);
    row[BASE_FRAME_PREVIOUS_START] = F(state.frame.previous_start);
    row[BASE_FRAME_PREVIOUS_END] = F(state.frame.previous_end);
    let check_used = matches!(
        state.phase,
        PHASE_IDENTIFIER_FIRST | PHASE_FINALIZE_HEADER | PHASE_PRIMITIVE_CONTENT | PHASE_BOUNDARY
    );
    if check_used {
        row[BASE_CHECK_IS_ZERO] = F(u64::from(state.check_delta == 0));
        row[BASE_CHECK_INVERSE] = inverse_or_zero_v1(state.check_delta);
    }
    row[BASE_PRIMITIVE_FIRST] = F(u64::from(state.primitive_first));
    row[BASE_OID_START] = F(u64::from(state.oid_start));
    row[BASE_UNUSED_BITS] = F(u64::from(state.unused_bits));
    row[BASE_DOCUMENT_FIRST] = F(u64::from(state.document_first));
    match state.phase {
        PHASE_IDENTIFIER_FIRST | PHASE_IDENTIFIER_HIGH => {
            write_bits_v1(&mut row, BASE_PAYLOAD, 3, u64::from(state.identifier_count));
            write_bits_v1(
                &mut row,
                BASE_PAYLOAD + 3,
                7,
                u64::from(state.first_high_group),
            );
        }
        PHASE_LENGTH_FIRST | PHASE_LENGTH_BODY => {
            write_bits_v1(&mut row, BASE_PAYLOAD, 2, u64::from(state.length_remaining));
            row[BASE_PAYLOAD + 2] = F(u64::from(state.long_length_two));
            row[BASE_PAYLOAD + 3] = F(u64::from(state.length_first_was_64));
        }
        PHASE_FINALIZE_HEADER => {
            row[BASE_PAYLOAD..BASE_PAYLOAD + ZK_X509_DER_AIR_UNIVERSAL_SELECTORS_V1]
                .copy_from_slice(&state.finalize_selectors);
        }
        PHASE_PRIMITIVE_CONTENT => {
            write_bits_v1(&mut row, BASE_PAYLOAD, 3, u64::from(state.primitive_kind));
            write_bits_v1(&mut row, BASE_PAYLOAD + 3, 3, u64::from(state.unused_bits));
            let byte = F(u64::from(byte.ok_or(ZkX509DerStarkErrorV1::Row)?));
            let ff_delta = byte.sub(F(0xff));
            row[BASE_PAYLOAD + 6] = F(u64::from(byte == F::ZERO));
            row[BASE_PAYLOAD + 7] = byte.inv().unwrap_or(F::ZERO);
            row[BASE_PAYLOAD + 8] = F(u64::from(ff_delta == F::ZERO));
            row[BASE_PAYLOAD + 9] = ff_delta.inv().unwrap_or(F::ZERO);
        }
        PHASE_BOUNDARY => {
            row[BASE_PAYLOAD] = F(state.boundary_parent.id);
            row[BASE_PAYLOAD + 1] = F(state.boundary_parent.start);
            row[BASE_PAYLOAD + 2] = F(state.boundary_parent.end);
            row[BASE_PAYLOAD + 3] = F(u64::from(state.boundary_parent.is_set));
            row[BASE_PAYLOAD + 4] = F(u64::from(state.boundary_parent.has_child));
            row[BASE_PAYLOAD + 5] = F(state.boundary_parent.previous_id);
            row[BASE_PAYLOAD + 6] = F(state.boundary_parent.previous_start);
            row[BASE_PAYLOAD + 7] = F(state.boundary_parent.previous_end);
        }
        _ => return Err(ZkX509DerStarkErrorV1::Row),
    }
    Ok(row)
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn node_usize_v1(value: F) -> Result<usize, ZkX509DerStarkErrorV1> {
    usize::try_from(value.0).map_err(|_| ZkX509DerStarkErrorV1::Resource)
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
const fn node_u64_v1(value: F) -> u64 {
    value.0
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn direct_children_v1(
    trace: &ZkX509DerDocumentTraceV1,
    node_index: usize,
) -> Result<Vec<usize>, ZkX509DerStarkErrorV1> {
    let node = trace
        .nodes
        .get(node_index)
        .ok_or(ZkX509DerStarkErrorV1::Row)?;
    if node.constructed == F::ZERO {
        return Ok(Vec::new());
    }
    let child_depth = node_u64_v1(node.depth.value)
        .checked_add(1)
        .ok_or(ZkX509DerStarkErrorV1::Resource)?;
    let content_start = node_u64_v1(node.content_start.value);
    let end = node_u64_v1(node.end.value);
    Ok(trace
        .nodes
        .iter()
        .enumerate()
        .skip(node_index + 1)
        .take_while(|(_, candidate)| node_u64_v1(candidate.start.value) < end)
        .filter(|(_, candidate)| {
            node_u64_v1(candidate.depth.value) == child_depth
                && node_u64_v1(candidate.start.value) >= content_start
        })
        .map(|(index, _)| index)
        .collect())
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn primitive_kind_v1(node: &ZkX509DerNodeRowV1) -> u8 {
    // 0=other, 1=BOOLEAN, 2=INTEGER, 3=BIT STRING, 4=NULL,
    // 5=OBJECT IDENTIFIER, 6=ENUMERATED.
    [0_usize, 1, 2, 4, 5, 6]
        .iter()
        .position(|selector| node.universal_selectors[*selector] == F::ONE)
        .map_or(0, |position| u8::try_from(position + 1).expect("kind fits"))
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn emit_parser_row_v1(
    rows: &mut Vec<[F; ZK_X509_DER_STARK_BASE_WIDTH_V1]>,
    byte_rows: &mut [Vec<usize>],
    state: &ParserStateV1,
    byte: Option<u8>,
) -> Result<(), ZkX509DerStarkErrorV1> {
    let index = rows.len();
    rows.push(encode_parser_state_v1(state, byte)?);
    if byte.is_some() {
        let document =
            usize::try_from(state.document).map_err(|_| ZkX509DerStarkErrorV1::Resource)?;
        let offset = usize::try_from(state.offset).map_err(|_| ZkX509DerStarkErrorV1::Resource)?;
        let document_rows = byte_rows
            .get_mut(document)
            .ok_or(ZkX509DerStarkErrorV1::Row)?;
        if offset >= document_rows.len() || document_rows[offset] != usize::MAX {
            return Err(ZkX509DerStarkErrorV1::Transition);
        }
        document_rows[offset] = index;
    }
    Ok(())
}
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn emit_node_v1(
    trace: &ZkX509DerDocumentTraceV1,
    encoded: &[u8],
    node_index: usize,
    is_last_sibling: bool,
    state: &mut ParserStateV1,
    stack: &mut Vec<FrameV1>,
    rows: &mut Vec<[F; ZK_X509_DER_STARK_BASE_WIDTH_V1]>,
    byte_rows: &mut [Vec<usize>],
) -> Result<(), ZkX509DerStarkErrorV1> {
    let node = trace
        .nodes
        .get(node_index)
        .ok_or(ZkX509DerStarkErrorV1::Row)?;
    let start = node_u64_v1(node.start.value);
    let content_start = node_u64_v1(node.content_start.value);
    let end = node_u64_v1(node.end.value);
    if state.phase != PHASE_IDENTIFIER_FIRST
        || state.offset != start
        || state.node_count
            != u64::try_from(node_index).map_err(|_| ZkX509DerStarkErrorV1::Resource)?
    {
        return Err(ZkX509DerStarkErrorV1::Transition);
    }
    state.node_start = start;
    state.tag_class = 0;
    state.constructed = false;
    state.tag_accumulator = 0;
    state.length_accumulator = 0;
    state.content_start = 0;
    state.content_end = 0;
    state.identifier_count = 0;
    state.first_high_group = 0;
    state.check_delta = u64::try_from(ZK_X509_DER_MAX_VALUES_V1)
        .expect("cap")
        .checked_sub(state.node_count)
        .ok_or(ZkX509DerStarkErrorV1::Resource)?;
    for identifier_index in 0..node_usize_v1(node.identifier_len.value)? {
        let byte = u8::try_from(node.identifier[identifier_index].value.0)
            .map_err(|_| ZkX509DerStarkErrorV1::Row)?;
        state.phase = if identifier_index == 0 {
            PHASE_IDENTIFIER_FIRST
        } else {
            PHASE_IDENTIFIER_HIGH
        };
        state.identifier_count =
            u8::try_from(identifier_index).map_err(|_| ZkX509DerStarkErrorV1::Resource)?;
        emit_parser_row_v1(rows, byte_rows, state, Some(byte))?;
        if identifier_index == 0 {
            state.tag_class = byte >> 6;
            state.constructed = byte & 0x20 != 0;
            state.tag_accumulator = u64::from(byte & 0x1f);
        } else {
            let low = byte & 0x7f;
            if identifier_index == 1 {
                state.first_high_group = low;
                state.tag_accumulator = u64::from(low);
            } else {
                state.tag_accumulator = state
                    .tag_accumulator
                    .checked_mul(128)
                    .and_then(|value| value.checked_add(u64::from(low)))
                    .ok_or(ZkX509DerStarkErrorV1::Resource)?;
            }
        }
        state.offset = state
            .offset
            .checked_add(1)
            .ok_or(ZkX509DerStarkErrorV1::Resource)?;
        state.document_first = false;
    }
    if state.tag_accumulator != node.tag_number.value.0 {
        return Err(ZkX509DerStarkErrorV1::Transition);
    }
    let length_len = node_usize_v1(node.length_len.value)?;
    state.length_accumulator = 0;
    state.length_remaining = 0;
    state.long_length_two = false;
    state.length_first_was_64 = false;
    for length_index in 0..length_len {
        let byte = u8::try_from(node.length[length_index].value.0)
            .map_err(|_| ZkX509DerStarkErrorV1::Row)?;
        state.phase = if length_index == 0 {
            PHASE_LENGTH_FIRST
        } else {
            PHASE_LENGTH_BODY
        };
        if length_index == 0 && byte & 0x80 != 0 {
            state.length_remaining = byte & 0x7f;
            state.long_length_two = state.length_remaining == 2;
        }
        emit_parser_row_v1(rows, byte_rows, state, Some(byte))?;
        if length_index == 0 && byte & 0x80 == 0 {
            state.length_accumulator = u64::from(byte);
        } else if length_index != 0 {
            let was_first_long_body = state.long_length_two && state.length_remaining == 2;
            state.length_accumulator = state
                .length_accumulator
                .checked_mul(256)
                .and_then(|value| value.checked_add(u64::from(byte)))
                .ok_or(ZkX509DerStarkErrorV1::Resource)?;
            state.length_remaining = state
                .length_remaining
                .checked_sub(1)
                .ok_or(ZkX509DerStarkErrorV1::Transition)?;
            if was_first_long_body {
                state.length_first_was_64 = byte == 0x40;
            }
        }
        state.offset = state
            .offset
            .checked_add(1)
            .ok_or(ZkX509DerStarkErrorV1::Resource)?;
    }
    state.content_start = content_start;
    state.content_end = end;
    if state.length_accumulator != node.content_len.value.0 || state.offset != content_start {
        return Err(ZkX509DerStarkErrorV1::Transition);
    }
    state.phase = PHASE_FINALIZE_HEADER;
    state.check_delta = state.length_accumulator;
    state.finalize_selectors = node.universal_selectors;
    emit_parser_row_v1(rows, byte_rows, state, None)?;
    state.node_count = state
        .node_count
        .checked_add(1)
        .ok_or(ZkX509DerStarkErrorV1::Resource)?;
    state.finalize_selectors = [F::ZERO; ZK_X509_DER_AIR_UNIVERSAL_SELECTORS_V1];
    let children = direct_children_v1(trace, node_index)?;
    if node.constructed == F::ONE {
        let parent = state.frame;
        stack.push(parent);
        state.depth = state
            .depth
            .checked_add(1)
            .ok_or(ZkX509DerStarkErrorV1::Resource)?;
        state.frame = FrameV1 {
            id: u64::try_from(node_index).map_err(|_| ZkX509DerStarkErrorV1::Resource)?,
            start,
            end,
            is_set: node.universal_selectors[9] == F::ONE,
            ..FrameV1::default()
        };
        for (child_position, child) in children.iter().copied().enumerate() {
            state.phase = PHASE_IDENTIFIER_FIRST;
            emit_node_v1(
                trace,
                encoded,
                child,
                child_position + 1 == children.len(),
                state,
                stack,
                rows,
                byte_rows,
            )?;
        }
        if state.offset != end {
            return Err(ZkX509DerStarkErrorV1::Transition);
        }
        state.node_start = start;
        state.content_end = end;
        state.constructed = true;
        state.phase = PHASE_BOUNDARY;
        state.boundary_parent = *stack.last().ok_or(ZkX509DerStarkErrorV1::Transition)?;
        state.check_delta = if state.depth == 1 {
            state
                .document_len
                .checked_sub(state.offset)
                .ok_or(ZkX509DerStarkErrorV1::Transition)?
        } else {
            state
                .boundary_parent
                .end
                .checked_sub(state.offset)
                .ok_or(ZkX509DerStarkErrorV1::Transition)?
        };
        emit_parser_row_v1(rows, byte_rows, state, None)?;
        let completed = state.frame;
        let mut parent = stack.pop().ok_or(ZkX509DerStarkErrorV1::Transition)?;
        state.depth = state
            .depth
            .checked_sub(1)
            .ok_or(ZkX509DerStarkErrorV1::Transition)?;
        parent.has_child = true;
        parent.previous_id = completed.id;
        parent.previous_start = completed.start;
        parent.previous_end = completed.end;
        state.frame = parent;
        state.boundary_parent = FrameV1::default();
        state.node_start = if state.depth == 0 {
            completed.start
        } else {
            parent.start
        };
        state.content_end = if state.depth == 0 {
            completed.end
        } else {
            parent.end
        };
        state.constructed = true;
    } else {
        let content_start_usize =
            usize::try_from(content_start).map_err(|_| ZkX509DerStarkErrorV1::Resource)?;
        let end_usize = usize::try_from(end).map_err(|_| ZkX509DerStarkErrorV1::Resource)?;
        let kind = primitive_kind_v1(node);
        let mut oid_start = kind == 5;
        let unused = if kind == 3 {
            encoded
                .get(content_start_usize)
                .copied()
                .ok_or(ZkX509DerStarkErrorV1::Row)?
        } else {
            0
        };
        for (position, byte) in encoded[content_start_usize..end_usize]
            .iter()
            .copied()
            .enumerate()
        {
            state.phase = PHASE_PRIMITIVE_CONTENT;
            state.primitive_kind = kind;
            state.primitive_first = position == 0;
            state.oid_start = oid_start;
            state.unused_bits = unused;
            state.check_delta = end
                .checked_sub(
                    state
                        .offset
                        .checked_add(1)
                        .ok_or(ZkX509DerStarkErrorV1::Resource)?,
                )
                .ok_or(ZkX509DerStarkErrorV1::Transition)?;
            emit_parser_row_v1(rows, byte_rows, state, Some(byte))?;
            state.offset = state
                .offset
                .checked_add(1)
                .ok_or(ZkX509DerStarkErrorV1::Resource)?;
            oid_start = kind == 5 && byte & 0x80 == 0;
            state.primitive_first = false;
            state.oid_start = oid_start;
        }
        if state.offset != end {
            return Err(ZkX509DerStarkErrorV1::Transition);
        }
        state.phase = PHASE_BOUNDARY;
        state.boundary_parent = FrameV1::default();
        state.check_delta = if state.depth == 0 {
            state
                .document_len
                .checked_sub(state.offset)
                .ok_or(ZkX509DerStarkErrorV1::Transition)?
        } else {
            state
                .frame
                .end
                .checked_sub(state.offset)
                .ok_or(ZkX509DerStarkErrorV1::Transition)?
        };
        emit_parser_row_v1(rows, byte_rows, state, None)?;
        if state.depth != 0 {
            state.frame.has_child = true;
            state.frame.previous_id =
                u64::try_from(node_index).map_err(|_| ZkX509DerStarkErrorV1::Resource)?;
            state.frame.previous_start = start;
            state.frame.previous_end = end;
            if is_last_sibling {
                state.node_start = state.frame.start;
                state.content_end = state.frame.end;
                state.constructed = true;
            }
        }
    }
    state.primitive_kind = 0;
    state.primitive_first = false;
    state.oid_start = false;
    state.unused_bits = 0;
    state.phase = if state.depth == 0 && state.offset == state.document_len || is_last_sibling {
        PHASE_BOUNDARY
    } else {
        PHASE_IDENTIFIER_FIRST
    };
    Ok(())
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn encode_comparator_row_v1(
    document: usize,
    pair_id: usize,
    row_index: usize,
    rows: &[super::der_air::ZkX509DerSetOrderRowV1],
    nodes: &[ZkX509DerNodeRowV1],
) -> Result<[F; ZK_X509_DER_STARK_BASE_WIDTH_V1], ZkX509DerStarkErrorV1> {
    let source = rows.get(row_index).ok_or(ZkX509DerStarkErrorV1::Row)?;
    let left_index = node_usize_v1(source.left_node.value)?;
    let right_index = node_usize_v1(source.right_node.value)?;
    let left = nodes.get(left_index).ok_or(ZkX509DerStarkErrorV1::Row)?;
    let right = nodes.get(right_index).ok_or(ZkX509DerStarkErrorV1::Row)?;
    let same_previous = row_index != 0
        && rows[row_index - 1].set_node.value == source.set_node.value
        && rows[row_index - 1].left_node.value == source.left_node.value
        && rows[row_index - 1].right_node.value == source.right_node.value;
    let same_next = rows.get(row_index + 1).is_some_and(|next| {
        next.set_node.value == source.set_node.value
            && next.left_node.value == source.left_node.value
            && next.right_node.value == source.right_node.value
    });
    let left_len = left
        .end
        .value
        .0
        .checked_sub(left.start.value.0)
        .ok_or(ZkX509DerStarkErrorV1::Row)?;
    let right_len = right
        .end
        .value
        .0
        .checked_sub(right.start.value.0)
        .ok_or(ZkX509DerStarkErrorV1::Row)?;
    let left_le = left_len <= right_len;
    let slack = if left_le {
        right_len - left_len
    } else {
        left_len
            .checked_sub(right_len)
            .and_then(|value| value.checked_sub(1))
            .ok_or(ZkX509DerStarkErrorV1::Row)?
    };
    let mut row = [F::ZERO; ZK_X509_DER_STARK_BASE_WIDTH_V1];
    row[0] = F(u64::try_from(document).map_err(|_| ZkX509DerStarkErrorV1::Resource)?);
    row[1] = source.set_node.value;
    row[2] = source.left_node.value;
    row[3] = source.right_node.value;
    row[4] = left.start.value;
    row[5] = left.end.value;
    row[6] = right.start.value;
    row[7] = right.end.value;
    row[8] = F(u64::try_from(pair_id).map_err(|_| ZkX509DerStarkErrorV1::Resource)?);
    row[9] = source.offset.value;
    row[10] = source.left.value;
    row[11] = source.right.value;
    row[12] = source.equal_before;
    row[13] = source.less_before;
    row[14] = source.equal_after;
    row[15] = source.less_after;
    row[16] = source.bytes_equal;
    row[17] = source.byte_difference_inverse;
    row[18] = source.comparison_difference.value;
    row[19..27].copy_from_slice(&source.comparison_difference.bits);
    row[27] = source.comparison_borrow;
    row[28] = F(u64::from(!same_previous));
    row[29] = F(u64::from(!same_next));
    row[30] = F(u64::from(same_next));
    row[31] = F(u64::from(!same_next));
    row[32] = F(u64::from(left_le));
    row[33] = F(slack);
    write_bits_v1(&mut row, 34, 15, slack);
    Ok(row)
}
/// Build the exact active numeric base trace.
///
/// The logical parser is used only as a prover-side witness compiler and
/// differential oracle. The verifier consumes `rows`, `shape`, and numeric
/// residues and never calls the host parser.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn build_zk_x509_der_stark_base_v1(
    documents: &[&[u8]],
) -> Result<ZkX509DerStarkBaseV1, ZkX509DerStarkErrorV1> {
    if documents.is_empty()
        || documents.len() > ZK_X509_DER_STARK_MAX_DOCUMENTS_V1
        || documents.iter().any(|document| {
            document.is_empty() || document.len() > ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1
        })
    {
        return Err(ZkX509DerStarkErrorV1::Shape);
    }
    let traces: Vec<_> = documents
        .iter()
        .map(|document| {
            build_strict_der_document_trace_v1(document).map_err(|_| ZkX509DerStarkErrorV1::Row)
        })
        .collect::<Result<_, _>>()?;
    let document_lengths: Vec<_> = documents
        .iter()
        .map(|document| u16::try_from(document.len()).map_err(|_| ZkX509DerStarkErrorV1::Resource))
        .collect::<Result<_, _>>()?;
    let mut byte_rows: Vec<Vec<usize>> = document_lengths
        .iter()
        .map(|length| vec![usize::MAX; usize::from(*length)])
        .collect();
    let mut rows = Vec::new();
    for (document, ((encoded, trace), length)) in documents
        .iter()
        .zip(&traces)
        .zip(&document_lengths)
        .enumerate()
    {
        let mut state = ParserStateV1::for_document(document, usize::from(*length))?;
        let mut stack = Vec::new();
        emit_node_v1(
            trace,
            encoded,
            0,
            true,
            &mut state,
            &mut stack,
            &mut rows,
            &mut byte_rows,
        )?;
        if !stack.is_empty()
            || state.depth != 0
            || state.offset != state.document_len
            || state.node_count
                != u64::try_from(trace.nodes.len()).map_err(|_| ZkX509DerStarkErrorV1::Resource)?
        {
            return Err(ZkX509DerStarkErrorV1::Transition);
        }
    }
    let parser_rows = rows.len();
    if byte_rows
        .iter()
        .flatten()
        .any(|row_index| *row_index == usize::MAX)
    {
        return Err(ZkX509DerStarkErrorV1::Transition);
    }
    let mut pair_id = 0_usize;
    for (document, trace) in traces.iter().enumerate() {
        for row_index in 0..trace.set_order_rows.len() {
            let source = &trace.set_order_rows[row_index];
            let left_node = &trace.nodes[node_usize_v1(source.left_node.value)?];
            let right_node = &trace.nodes[node_usize_v1(source.right_node.value)?];
            let offset = node_usize_v1(source.offset.value)?;
            let left_address = node_usize_v1(left_node.start.value)?
                .checked_add(offset)
                .ok_or(ZkX509DerStarkErrorV1::Resource)?;
            let right_address = node_usize_v1(right_node.start.value)?
                .checked_add(offset)
                .ok_or(ZkX509DerStarkErrorV1::Resource)?;
            for address in [left_address, right_address] {
                let parser_row = *byte_rows
                    .get(document)
                    .and_then(|document_rows| document_rows.get(address))
                    .ok_or(ZkX509DerStarkErrorV1::Row)?;
                rows[parser_row][BASE_BYTE_LOOKUP_MULTIPLICITY] =
                    rows[parser_row][BASE_BYTE_LOOKUP_MULTIPLICITY].add(F::ONE);
            }
            let comparator = encode_comparator_row_v1(
                document,
                pair_id,
                row_index,
                &trace.set_order_rows,
                &trace.nodes,
            )?;
            let last = comparator[29] == F::ONE;
            rows.push(comparator);
            if last {
                pair_id = pair_id
                    .checked_add(1)
                    .ok_or(ZkX509DerStarkErrorV1::Resource)?;
            }
        }
    }
    let comparator_rows = rows
        .len()
        .checked_sub(parser_rows)
        .ok_or(ZkX509DerStarkErrorV1::Resource)?;
    let private_shape = ZkX509DerStarkPrivateShapeV1 {
        document_lengths,
        parser_rows,
        comparator_rows,
    };
    private_shape.validate()?;
    if rows.len() != private_shape.active_rows()? {
        return Err(ZkX509DerStarkErrorV1::Transition);
    }
    let final_document = private_shape
        .document_lengths
        .len()
        .checked_sub(1)
        .ok_or(ZkX509DerStarkErrorV1::Shape)?;
    let final_document_u64 =
        u64::try_from(final_document).map_err(|_| ZkX509DerStarkErrorV1::Resource)?;
    for row in &mut rows {
        row[BASE_ROW_ACTIVE] = F::ONE;
        row[BASE_FINAL_DOCUMENT] = F(final_document_u64);
        write_bits_v1(row, BASE_FINAL_DOCUMENT_BITS, 5, final_document_u64);
        write_bits_v1(
            row,
            BASE_FINAL_DOCUMENT_SLACK_BITS,
            5,
            u64::try_from(ZK_X509_DER_STARK_MAX_DOCUMENTS_V1 - 1 - final_document)
                .map_err(|_| ZkX509DerStarkErrorV1::Resource)?,
        );
    }
    Ok(ZkX509DerStarkBaseV1 {
        private_shape,
        rows,
    })
}
fn pack_bits_v1(bits: &[F]) -> F {
    bits.iter()
        .copied()
        .enumerate()
        .fold(F::ZERO, |sum, (bit, value)| {
            sum.add(value.mul(F(1_u64 << bit)))
        })
}
#[cfg(test)]
fn equality_selector_from_bits_v1(bits: &[F], value: usize) -> F {
    bits.iter()
        .copied()
        .enumerate()
        .fold(F::ONE, |product, (bit, actual)| {
            let expected = (value >> bit) & 1;
            product.mul(if expected == 0 {
                F::ONE.sub(actual)
            } else {
                actual
            })
        })
}
fn compress_tuple_v1(values: &[F], challenge: [F; 12]) -> F {
    values
        .iter()
        .copied()
        .zip(challenge)
        .fold(F::ZERO, |sum, (value, coefficient)| {
            sum.add(value.mul(coefficient))
        })
}
fn stack_push_tuple_v1(row: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1]) -> [F; 12] {
    [
        F::ONE,
        row[BASE_DOCUMENT],
        row[BASE_NODE_COUNT],
        pack_bits_v1(&row[BASE_DEPTH_BITS..BASE_DEPTH_BITS + 5]),
        row[BASE_FRAME_ID],
        row[BASE_FRAME_START],
        row[BASE_FRAME_END],
        row[BASE_FRAME_IS_SET],
        row[BASE_FRAME_HAS_CHILD],
        row[BASE_FRAME_PREVIOUS_ID],
        row[BASE_FRAME_PREVIOUS_START],
        row[BASE_FRAME_PREVIOUS_END],
    ]
}
fn stack_pop_tuple_v1(row: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1]) -> [F; 12] {
    [
        F::ONE,
        row[BASE_DOCUMENT],
        row[BASE_FRAME_ID],
        pack_bits_v1(&row[BASE_DEPTH_BITS..BASE_DEPTH_BITS + 5]).sub(F::ONE),
        row[BASE_PAYLOAD],
        row[BASE_PAYLOAD + 1],
        row[BASE_PAYLOAD + 2],
        row[BASE_PAYLOAD + 3],
        row[BASE_PAYLOAD + 4],
        row[BASE_PAYLOAD + 5],
        row[BASE_PAYLOAD + 6],
        row[BASE_PAYLOAD + 7],
    ]
}
fn document_tuple_v1(document: F, document_len: F) -> [F; 12] {
    let mut tuple = [F::ZERO; 12];
    tuple[0] = F(2);
    tuple[1] = document;
    tuple[2] = document_len;
    tuple
}
fn node_tuple_v1(row: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1]) -> [F; 12] {
    [
        F(3),
        row[BASE_DOCUMENT],
        row[BASE_NODE_COUNT],
        row[BASE_FRAME_ID],
        row[BASE_TAG_CLASS],
        row[BASE_TAG_ACCUMULATOR],
        row[BASE_CONSTRUCTED],
        row[BASE_NODE_START],
        row[BASE_CONTENT_START],
        row[BASE_CONTENT_END],
        pack_bits_v1(&row[BASE_DEPTH_BITS..BASE_DEPTH_BITS + 5]),
        row[BASE_LENGTH_ACCUMULATOR],
    ]
}
fn pair_producer_tuple_v1(row: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1]) -> [F; 12] {
    let mut tuple = [F::ZERO; 12];
    tuple[..9].copy_from_slice(&[
        F(4),
        row[BASE_DOCUMENT],
        row[BASE_FRAME_ID],
        row[BASE_FRAME_PREVIOUS_ID],
        row[BASE_NODE_COUNT],
        row[BASE_FRAME_PREVIOUS_START],
        row[BASE_FRAME_PREVIOUS_END],
        row[BASE_NODE_START],
        row[BASE_CONTENT_END],
    ]);
    tuple
}
fn pair_consumer_tuple_v1(row: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1]) -> [F; 12] {
    let mut tuple = [F::ZERO; 12];
    tuple[..9].copy_from_slice(&[
        F(4),
        row[0],
        row[1],
        row[2],
        row[3],
        row[4],
        row[5],
        row[6],
        row[7],
    ]);
    tuple
}
fn byte_tuple_v1(document: F, address: F, value: F) -> [F; 12] {
    let mut tuple = [F::ZERO; 12];
    tuple[0] = F(5);
    tuple[1] = document;
    tuple[2] = address;
    tuple[3] = value;
    tuple
}
fn input_byte_tuple_v1(document: F, address: F, value: F) -> [F; 12] {
    let mut tuple = [F::ZERO; 12];
    tuple[0] = F(6);
    tuple[1] = document;
    tuple[2] = address;
    tuple[3] = value;
    tuple
}
/// Return the exact DER input-byte factor consumed by a downstream adapter.
pub(crate) fn zk_x509_der_stark_input_byte_factor_v1(
    document: F,
    address: F,
    value: F,
    lane: usize,
    challenges: ZkX509DerStarkChallengesV1,
) -> Result<F, ZkX509DerStarkErrorV1> {
    challenges.validate()?;
    let tuple_challenge = challenges
        .tuple
        .get(lane)
        .copied()
        .ok_or(ZkX509DerStarkErrorV1::Challenge)?;
    Ok(compress_tuple_v1(
        &input_byte_tuple_v1(document, address, value),
        tuple_challenge,
    ))
}
/// Return the exact DER node factor consumed by a downstream adapter.
pub(crate) fn zk_x509_der_stark_node_factor_v1(
    event: ZkX509DerStarkNodeEventV1,
    lane: usize,
    challenges: ZkX509DerStarkChallengesV1,
) -> Result<F, ZkX509DerStarkErrorV1> {
    challenges.validate()?;
    let tuple_challenge = challenges
        .tuple
        .get(lane)
        .copied()
        .ok_or(ZkX509DerStarkErrorV1::Challenge)?;
    Ok(compress_tuple_v1(
        &[
            F(3),
            event.document,
            event.ordinal,
            event.parent_frame,
            event.tag_class,
            event.tag_number,
            event.constructed,
            event.start,
            event.content_start,
            event.content_end,
            event.depth,
            event.content_len,
        ],
        tuple_challenge,
    ))
}
fn byte_denominator_v1(tuple: [F; 12], lane: usize, challenges: ZkX509DerStarkChallengesV1) -> F {
    challenges.byte_lookup[lane].add(compress_tuple_v1(&tuple, challenges.tuple[lane]))
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn derive_zk_x509_der_stark_private_document_product_v1(
    shape: &ZkX509DerStarkPrivateShapeV1,
    challenges: ZkX509DerStarkChallengesV1,
) -> Result<[F; ZK_X509_DER_STARK_BUS_LANES_V1], ZkX509DerStarkErrorV1> {
    shape.validate()?;
    challenges.validate()?;
    let mut document_product = [F::ONE; ZK_X509_DER_STARK_BUS_LANES_V1];
    for (document, length) in shape.document_lengths.iter().copied().enumerate() {
        let tuple = document_tuple_v1(
            F(u64::try_from(document).map_err(|_| ZkX509DerStarkErrorV1::Resource)?),
            F(u64::from(length)),
        );
        for lane in 0..ZK_X509_DER_STARK_BUS_LANES_V1 {
            document_product[lane] =
                document_product[lane].mul(compress_tuple_v1(&tuple, challenges.tuple[lane]));
        }
    }
    Ok(document_product)
}
pub(crate) fn derive_zk_x509_der_stark_public_terminals_v1(
    shape: &ZkX509DerStarkShapeV1,
    challenges: ZkX509DerStarkChallengesV1,
) -> Result<ZkX509DerStarkPublicTerminalsV1, ZkX509DerStarkErrorV1> {
    shape.validate()?;
    challenges.validate()?;
    Ok(ZkX509DerStarkPublicTerminalsV1)
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn read_aux_lanes_v1(
    row: &[F; ZK_X509_DER_STARK_AUX_WIDTH_V1],
    offset: usize,
) -> [F; ZK_X509_DER_STARK_BUS_LANES_V1] {
    row[offset..offset + ZK_X509_DER_STARK_BUS_LANES_V1]
        .try_into()
        .expect("four DER bus lanes")
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn write_aux_lanes_v1(
    row: &mut [F; ZK_X509_DER_STARK_AUX_WIDTH_V1],
    offset: usize,
    values: [F; ZK_X509_DER_STARK_BUS_LANES_V1],
) {
    row[offset..offset + ZK_X509_DER_STARK_BUS_LANES_V1].copy_from_slice(&values);
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn zk_x509_der_stark_terminals_v1(
    trace: &ZkX509DerStarkTraceV1,
) -> Result<ZkX509DerStarkTerminalsV1, ZkX509DerStarkErrorV1> {
    let last = trace
        .aux_rows
        .last()
        .ok_or(ZkX509DerStarkErrorV1::Transition)?;
    Ok(ZkX509DerStarkTerminalsV1 {
        stack_push: read_aux_lanes_v1(last, AUX_STACK_PUSH_AFTER),
        stack_pop: read_aux_lanes_v1(last, AUX_STACK_POP_AFTER),
        document: read_aux_lanes_v1(last, AUX_DOCUMENT_AFTER),
        node: read_aux_lanes_v1(last, AUX_NODE_AFTER),
        pair_producer: read_aux_lanes_v1(last, AUX_PAIR_PRODUCER_AFTER),
        pair_consumer: read_aux_lanes_v1(last, AUX_PAIR_CONSUMER_AFTER),
        byte_table_sum: read_aux_lanes_v1(last, AUX_BYTE_TABLE_SUM_AFTER),
        byte_query_sum: read_aux_lanes_v1(last, AUX_BYTE_QUERY_SUM_AFTER),
        byte_table_zero_count: read_aux_lanes_v1(last, AUX_BYTE_TABLE_ZERO_COUNT_AFTER),
        byte_query_zero_count: read_aux_lanes_v1(last, AUX_BYTE_QUERY_ZERO_COUNT_AFTER),
        input_byte: read_aux_lanes_v1(last, AUX_INPUT_BYTE_AFTER),
    })
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn zk_x509_der_stark_terminal_claims_v1(
    trace: &ZkX509DerStarkTraceV1,
) -> Result<ZkX509DerStarkTerminalClaimsV1, ZkX509DerStarkErrorV1> {
    let terminals = zk_x509_der_stark_terminals_v1(trace)?;
    Ok(ZkX509DerStarkTerminalClaimsV1 {
        input_byte: terminals.input_byte,
        node: terminals.node,
    })
}
/// Bind ordered input-byte then node claims to the committed final auxiliary
/// row. This helper is shared by native and aggregate opened-row evaluation.
pub(crate) fn evaluate_zk_x509_der_stark_terminal_claim_residues_v1(
    last_aggregate: F,
    aux: &[F; ZK_X509_DER_STARK_AUX_WIDTH_V1],
    claims: ZkX509DerStarkTerminalClaimsV1,
) -> [F; 2 * ZK_X509_DER_STARK_BUS_LANES_V1] {
    core::array::from_fn(|index| {
        if index < ZK_X509_DER_STARK_BUS_LANES_V1 {
            last_aggregate.mul(aux[AUX_INPUT_BYTE_AFTER + index].sub(claims.input_byte[index]))
        } else {
            let lane = index - ZK_X509_DER_STARK_BUS_LANES_V1;
            last_aggregate.mul(aux[AUX_NODE_AFTER + lane].sub(claims.node[lane]))
        }
    })
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn write_zero_test_witness_v1(
    row: &mut [F; ZK_X509_DER_STARK_AUX_WIDTH_V1],
    selector_column: usize,
    inverse_column: usize,
    value: F,
) -> Result<(), ZkX509DerStarkErrorV1> {
    if value == F::ZERO {
        row[selector_column] = F::ONE;
        row[inverse_column] = F::ZERO;
    } else {
        row[selector_column] = F::ZERO;
        row[inverse_column] = value.inv().ok_or(ZkX509DerStarkErrorV1::Row)?;
    }
    Ok(())
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn populate_low_degree_auxiliaries_v1(
    base: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1],
    fixed: &[F; ZK_X509_DER_STARK_FIXED_WIDTH_V1],
    aux: &mut [F; ZK_X509_DER_STARK_AUX_WIDTH_V1],
) -> Result<(), ZkX509DerStarkErrorV1> {
    let row_active = base[BASE_ROW_ACTIVE] == F::ONE;
    let parser = fixed[FIX_PARSER] == F::ONE && row_active;
    let comparator = fixed[FIX_COMPARATOR] == F::ONE && row_active;
    let phase = if parser {
        usize::try_from(pack_bits_v1(&base[BASE_PHASE_BITS..BASE_PHASE_BITS + 3]).0)
            .map_err(|_| ZkX509DerStarkErrorV1::Row)?
    } else if comparator {
        PHASE_SET_COMPARATOR
    } else {
        PHASE_IDENTIFIER_FIRST
    };
    if phase > PHASE_SET_COMPARATOR {
        return Err(ZkX509DerStarkErrorV1::Row);
    }
    if parser {
        aux[AUX_PHASE_SELECTORS + phase] = F::ONE;
        let depth = usize::try_from(pack_bits_v1(&base[BASE_DEPTH_BITS..BASE_DEPTH_BITS + 5]).0)
            .map_err(|_| ZkX509DerStarkErrorV1::Row)?;
        if depth > ZK_X509_DER_MAX_NESTING_DEPTH_V1 {
            return Err(ZkX509DerStarkErrorV1::Row);
        }
        aux[AUX_DEPTH_SELECTORS + depth] = F::ONE;
    }
    if phase == PHASE_IDENTIFIER_HIGH {
        let count = usize::try_from(pack_bits_v1(&base[BASE_PAYLOAD..BASE_PAYLOAD + 3]).0)
            .map_err(|_| ZkX509DerStarkErrorV1::Row)?;
        if count >= 8 {
            return Err(ZkX509DerStarkErrorV1::Row);
        }
        aux[AUX_IDENTIFIER_COUNT_SELECTORS + count] = F::ONE;
    }
    if phase == PHASE_PRIMITIVE_CONTENT {
        let kind = usize::try_from(pack_bits_v1(&base[BASE_PAYLOAD..BASE_PAYLOAD + 3]).0)
            .map_err(|_| ZkX509DerStarkErrorV1::Row)?;
        let unused = usize::try_from(pack_bits_v1(&base[BASE_PAYLOAD + 3..BASE_PAYLOAD + 6]).0)
            .map_err(|_| ZkX509DerStarkErrorV1::Row)?;
        if kind >= 8 || unused >= 8 {
            return Err(ZkX509DerStarkErrorV1::Row);
        }
        aux[AUX_PRIMITIVE_KIND_SELECTORS + kind] = F::ONE;
        aux[AUX_UNUSED_BIT_SELECTORS + unused] = F::ONE;
    }
    if phase == PHASE_LENGTH_BODY {
        let remaining = usize::try_from(pack_bits_v1(&base[BASE_PAYLOAD..BASE_PAYLOAD + 2]).0)
            .map_err(|_| ZkX509DerStarkErrorV1::Row)?;
        if remaining >= 4 {
            return Err(ZkX509DerStarkErrorV1::Row);
        }
        aux[AUX_LENGTH_REMAINING_SELECTORS + remaining] = F::ONE;
    }
    let byte_bits = &base[BASE_BYTE_BITS..BASE_BYTE_BITS + 8];
    aux[AUX_LOW_FIVE_PAIR_01] = byte_bits[0].mul(byte_bits[1]);
    aux[AUX_LOW_FIVE_PAIR_23] = byte_bits[2].mul(byte_bits[3]);
    aux[AUX_HIGH_TAG] = aux[AUX_LOW_FIVE_PAIR_01]
        .mul(aux[AUX_LOW_FIVE_PAIR_23])
        .mul(byte_bits[4]);
    let high_low = pack_bits_v1(&byte_bits[..7]);
    write_zero_test_witness_v1(aux, AUX_HIGH_LOW_ZERO, AUX_HIGH_LOW_INVERSE, high_low)?;
    let identifier_high = aux[AUX_PHASE_SELECTORS + PHASE_IDENTIFIER_HIGH];
    aux[AUX_HIGH_LOW_GE_31] = identifier_high.mul(
        F::ONE.sub(
            F::ONE
                .sub(byte_bits[6])
                .mul(F::ONE.sub(byte_bits[5]))
                .mul(F::ONE.sub(aux[AUX_HIGH_TAG])),
        ),
    );
    let length_first = aux[AUX_PHASE_SELECTORS + PHASE_LENGTH_FIRST];
    let long_length = byte_bits[7];
    aux[AUX_LENGTH_COUNT_TWO] = length_first.mul(long_length).mul(high_low.sub(F::ONE));
    write_zero_test_witness_v1(aux, AUX_BYTE_ZERO, AUX_BYTE_INVERSE, base[BASE_BYTE_VALUE])?;
    write_zero_test_witness_v1(
        aux,
        AUX_BYTE_IS_64,
        AUX_BYTE_64_INVERSE,
        base[BASE_BYTE_VALUE].sub(F(64)),
    )?;
    write_zero_test_witness_v1(
        aux,
        AUX_BYTE_IS_128,
        AUX_BYTE_128_INVERSE,
        base[BASE_BYTE_VALUE].sub(F(128)),
    )?;
    let count_one = aux[AUX_IDENTIFIER_COUNT_SELECTORS + 1];
    for bit in 0..7 {
        aux[AUX_UPDATED_FIRST_HIGH_BITS + bit] = identifier_high.mul(
            base[BASE_PAYLOAD + 3 + bit]
                .add(count_one.mul(byte_bits[bit].sub(base[BASE_PAYLOAD + 3 + bit]))),
        );
    }
    let primitive = aux[AUX_PHASE_SELECTORS + PHASE_PRIMITIVE_CONTENT];
    let kind_integer_or_enumerated =
        aux[AUX_PRIMITIVE_KIND_SELECTORS + 2].add(aux[AUX_PRIMITIVE_KIND_SELECTORS + 6]);
    let last_primitive = base[BASE_CHECK_IS_ZERO];
    aux[AUX_SIGNED_FIRST_GUARD] = kind_integer_or_enumerated
        .mul(base[BASE_PRIMITIVE_FIRST])
        .mul(F::ONE.sub(last_primitive));
    aux[AUX_BIT_STRING_FIRST_GUARD] =
        aux[AUX_PRIMITIVE_KIND_SELECTORS + 3].mul(base[BASE_PRIMITIVE_FIRST]);
    aux[AUX_BIT_STRING_LAST_CONTINUATION_GUARD] = aux[AUX_PRIMITIVE_KIND_SELECTORS + 3]
        .mul(F::ONE.sub(base[BASE_PRIMITIVE_FIRST]))
        .mul(last_primitive);
    aux[AUX_NEXT_OID_START_EXPECTED] = aux[AUX_PRIMITIVE_KIND_SELECTORS + 5]
        .mul(F::ONE.sub(last_primitive))
        .mul(F::ONE.sub(byte_bits[7]));
    let identifier_first = aux[AUX_PHASE_SELECTORS + PHASE_IDENTIFIER_FIRST];
    let finalize = aux[AUX_PHASE_SELECTORS + PHASE_FINALIZE_HEADER];
    let boundary = aux[AUX_PHASE_SELECTORS + PHASE_BOUNDARY];
    let depth_zero = aux[AUX_DEPTH_SELECTORS];
    let depth_one = aux[AUX_DEPTH_SELECTORS + 1];
    let boundary_delta = base[BASE_CONSTRUCTED]
        .mul(
            depth_one
                .mul(base[BASE_DOCUMENT_LEN].sub(base[BASE_OFFSET]))
                .add(
                    F::ONE
                        .sub(depth_one)
                        .mul(base[BASE_PAYLOAD + 2].sub(base[BASE_OFFSET])),
                ),
        )
        .add(
            F::ONE.sub(base[BASE_CONSTRUCTED]).mul(
                depth_zero
                    .mul(base[BASE_DOCUMENT_LEN].sub(base[BASE_OFFSET]))
                    .add(
                        F::ONE
                            .sub(depth_zero)
                            .mul(base[BASE_FRAME_END].sub(base[BASE_OFFSET])),
                    ),
            ),
        );
    aux[AUX_CHECK_DELTA] = identifier_first
        .mul(F(u64::try_from(ZK_X509_DER_MAX_VALUES_V1).expect("cap")).sub(base[BASE_NODE_COUNT]))
        .add(finalize.mul(base[BASE_LENGTH_ACCUMULATOR]))
        .add(primitive.mul(base[BASE_CONTENT_END].sub(base[BASE_OFFSET]).sub(F::ONE)))
        .add(boundary.mul(boundary_delta));
    aux[AUX_ROOT_COMPLETION] = F::ONE
        .sub(base[BASE_CONSTRUCTED])
        .mul(depth_zero)
        .add(base[BASE_CONSTRUCTED].mul(depth_one));
    aux[AUX_BOUNDARY_NOT_ROOT] = boundary.mul(F::ONE.sub(aux[AUX_ROOT_COMPLETION]));
    aux[AUX_BOUNDARY_COMPLETES_PARENT] = aux[AUX_BOUNDARY_NOT_ROOT].mul(base[BASE_CHECK_IS_ZERO]);
    aux[AUX_PAIR_PRODUCER_EVENT] = finalize
        .mul(base[BASE_FRAME_IS_SET])
        .mul(base[BASE_FRAME_HAS_CHILD]);
    aux[AUX_PRIMITIVE_ENTRY] = finalize
        .mul(F::ONE.sub(base[BASE_CONSTRUCTED]))
        .mul(F::ONE.sub(base[BASE_CHECK_IS_ZERO]));
    aux[AUX_ENTERS_CHILD] = base[BASE_CONSTRUCTED].mul(F::ONE.sub(base[BASE_CHECK_IS_ZERO]));
    Ok(())
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn validate_zk_x509_der_stark_base_trace_v1(
    base: &ZkX509DerStarkBaseV1,
) -> Result<(), ZkX509DerStarkErrorV1> {
    base.private_shape.validate()?;
    let schedule = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1)?;
    if base.rows.len() != base.private_shape.active_rows()?
        || base
            .rows
            .iter()
            .flatten()
            .any(|value| value.0 >= GOLDILOCKS_MODULUS_V1 || F::canonical(value.0).is_none())
    {
        return Err(ZkX509DerStarkErrorV1::Row);
    }
    let comparator_start = ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1;
    let comparator_end = comparator_start
        .checked_add(base.private_shape.comparator_rows)
        .ok_or(ZkX509DerStarkErrorV1::Resource)?;
    let mut indices = (0..base.private_shape.parser_rows)
        .chain(comparator_start..comparator_end)
        .collect::<Vec<_>>();
    indices.extend([
        base.private_shape.parser_rows,
        ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 - 1,
        comparator_end,
        ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1 - 1,
        ZK_X509_DER_STARK_TRACE_SIZE_V1 - 1,
    ]);
    indices.sort_unstable();
    indices.dedup();
    for index in indices {
        let next_index = (index + 1) % ZK_X509_DER_STARK_TRACE_SIZE_V1;
        let current = zk_x509_der_stark_aggregate_base_row_v1(base, index)?;
        let next = zk_x509_der_stark_aggregate_base_row_v1(base, next_index)?;
        let fixed = schedule.fixed_row(index)?;
        let next_fixed = schedule.fixed_row(next_index)?;
        let mut aux = [F::ZERO; ZK_X509_DER_STARK_AUX_WIDTH_V1];
        populate_low_degree_auxiliaries_v1(&current, &fixed, &mut aux)?;
        if evaluate_zk_x509_der_stark_base_residues_v1(&current, &next, &aux, &fixed, &next_fixed)
            .iter()
            .any(|residue| *residue != F::ZERO)
        {
            return Err(ZkX509DerStarkErrorV1::Transition);
        }
    }
    Ok(())
}
/// Attach all post-base-commitment permutation and lookup accumulators.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn build_zk_x509_der_stark_trace_v1(
    base: ZkX509DerStarkBaseV1,
    challenges: ZkX509DerStarkChallengesV1,
) -> Result<ZkX509DerStarkTraceV1, ZkX509DerStarkErrorV1> {
    challenges.validate()?;
    validate_zk_x509_der_stark_base_trace_v1(&base)?;
    let schedule = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1)?;
    if base.rows.len() != base.private_shape.active_rows()? {
        return Err(ZkX509DerStarkErrorV1::Transition);
    }
    let mut stack_push = [F::ONE; ZK_X509_DER_STARK_BUS_LANES_V1];
    let mut stack_pop = [F::ONE; ZK_X509_DER_STARK_BUS_LANES_V1];
    let mut document = [F::ONE; ZK_X509_DER_STARK_BUS_LANES_V1];
    let mut node = [F::ONE; ZK_X509_DER_STARK_BUS_LANES_V1];
    let mut pair_producer = [F::ONE; ZK_X509_DER_STARK_BUS_LANES_V1];
    let mut pair_consumer = [F::ONE; ZK_X509_DER_STARK_BUS_LANES_V1];
    let mut byte_table_sum = [F::ZERO; ZK_X509_DER_STARK_BUS_LANES_V1];
    let mut byte_query_sum = [F::ZERO; ZK_X509_DER_STARK_BUS_LANES_V1];
    let mut byte_table_zero_count = [F::ZERO; ZK_X509_DER_STARK_BUS_LANES_V1];
    let mut byte_query_zero_count = [F::ZERO; ZK_X509_DER_STARK_BUS_LANES_V1];
    let mut input_byte = [F::ONE; ZK_X509_DER_STARK_BUS_LANES_V1];
    let mut aux_rows = Vec::with_capacity(base.rows.len());
    for (index, current) in base.rows.iter().enumerate() {
        let parser = index < base.private_shape.parser_rows;
        let comparator = !parser;
        let native_index =
            zk_x509_der_stark_compact_row_native_index_v1(&base.private_shape, index)?;
        let phase = if parser {
            usize::try_from(pack_bits_v1(&current[BASE_PHASE_BITS..BASE_PHASE_BITS + 3]).0)
                .map_err(|_| ZkX509DerStarkErrorV1::Row)?
        } else {
            PHASE_SET_COMPARATOR
        };
        if phase > PHASE_SET_COMPARATOR {
            return Err(ZkX509DerStarkErrorV1::Row);
        }
        let consuming = parser
            && matches!(
                phase,
                PHASE_IDENTIFIER_FIRST
                    | PHASE_IDENTIFIER_HIGH
                    | PHASE_LENGTH_FIRST
                    | PHASE_LENGTH_BODY
                    | PHASE_PRIMITIVE_CONTENT
            );
        let stack_push_event =
            parser && phase == PHASE_FINALIZE_HEADER && current[BASE_CONSTRUCTED] == F::ONE;
        let stack_pop_event =
            parser && phase == PHASE_BOUNDARY && current[BASE_CONSTRUCTED] == F::ONE;
        let document_event =
            parser && phase == PHASE_IDENTIFIER_FIRST && current[BASE_DOCUMENT_FIRST] == F::ONE;
        let node_event = parser && phase == PHASE_FINALIZE_HEADER;
        let pair_producer_event = node_event
            && current[BASE_FRAME_IS_SET] == F::ONE
            && current[BASE_FRAME_HAS_CHILD] == F::ONE;
        let pair_consumer_event = comparator && current[28] == F::ONE;
        let mut aux = [F::ZERO; ZK_X509_DER_STARK_AUX_WIDTH_V1];
        populate_low_degree_auxiliaries_v1(current, &schedule.fixed_row(native_index)?, &mut aux)?;
        write_aux_lanes_v1(&mut aux, AUX_STACK_PUSH_BEFORE, stack_push);
        write_aux_lanes_v1(&mut aux, AUX_STACK_POP_BEFORE, stack_pop);
        write_aux_lanes_v1(&mut aux, AUX_DOCUMENT_BEFORE, document);
        write_aux_lanes_v1(&mut aux, AUX_NODE_BEFORE, node);
        write_aux_lanes_v1(&mut aux, AUX_PAIR_PRODUCER_BEFORE, pair_producer);
        write_aux_lanes_v1(&mut aux, AUX_PAIR_CONSUMER_BEFORE, pair_consumer);
        write_aux_lanes_v1(&mut aux, AUX_BYTE_TABLE_SUM_BEFORE, byte_table_sum);
        write_aux_lanes_v1(&mut aux, AUX_BYTE_QUERY_SUM_BEFORE, byte_query_sum);
        write_aux_lanes_v1(
            &mut aux,
            AUX_BYTE_TABLE_ZERO_COUNT_BEFORE,
            byte_table_zero_count,
        );
        write_aux_lanes_v1(
            &mut aux,
            AUX_BYTE_QUERY_ZERO_COUNT_BEFORE,
            byte_query_zero_count,
        );
        write_aux_lanes_v1(&mut aux, AUX_INPUT_BYTE_BEFORE, input_byte);
        for lane in 0..ZK_X509_DER_STARK_BUS_LANES_V1 {
            if stack_push_event {
                stack_push[lane] = stack_push[lane].mul(compress_tuple_v1(
                    &stack_push_tuple_v1(current),
                    challenges.tuple[lane],
                ));
            }
            if stack_pop_event {
                stack_pop[lane] = stack_pop[lane].mul(compress_tuple_v1(
                    &stack_pop_tuple_v1(current),
                    challenges.tuple[lane],
                ));
            }
            if document_event {
                document[lane] = document[lane].mul(compress_tuple_v1(
                    &document_tuple_v1(current[BASE_DOCUMENT], current[BASE_DOCUMENT_LEN]),
                    challenges.tuple[lane],
                ));
            }
            if node_event {
                node[lane] = node[lane].mul(compress_tuple_v1(
                    &node_tuple_v1(current),
                    challenges.tuple[lane],
                ));
            }
            if pair_producer_event {
                pair_producer[lane] = pair_producer[lane].mul(compress_tuple_v1(
                    &pair_producer_tuple_v1(current),
                    challenges.tuple[lane],
                ));
            }
            if pair_consumer_event {
                pair_consumer[lane] = pair_consumer[lane].mul(compress_tuple_v1(
                    &pair_consumer_tuple_v1(current),
                    challenges.tuple[lane],
                ));
            }
            if consuming {
                input_byte[lane] = input_byte[lane].mul(compress_tuple_v1(
                    &input_byte_tuple_v1(
                        current[BASE_DOCUMENT],
                        current[BASE_OFFSET],
                        current[BASE_BYTE_VALUE],
                    ),
                    challenges.tuple[lane],
                ));
                let denominator = byte_denominator_v1(
                    byte_tuple_v1(
                        current[BASE_DOCUMENT],
                        current[BASE_OFFSET],
                        current[BASE_BYTE_VALUE],
                    ),
                    lane,
                    challenges,
                );
                if denominator == F::ZERO {
                    aux[AUX_BYTE_TABLE_ZERO + lane] = F::ONE;
                    byte_table_zero_count[lane] =
                        byte_table_zero_count[lane].add(current[BASE_BYTE_LOOKUP_MULTIPLICITY]);
                } else {
                    let inverse = denominator
                        .inv()
                        .expect("nonzero canonical lookup denominator");
                    aux[AUX_BYTE_TABLE_INVERSE + lane] = inverse;
                    byte_table_sum[lane] = byte_table_sum[lane]
                        .add(current[BASE_BYTE_LOOKUP_MULTIPLICITY].mul(inverse));
                }
            }
            if comparator {
                let left_denominator = byte_denominator_v1(
                    byte_tuple_v1(current[0], current[4].add(current[9]), current[10]),
                    lane,
                    challenges,
                );
                let right_denominator = byte_denominator_v1(
                    byte_tuple_v1(current[0], current[6].add(current[9]), current[11]),
                    lane,
                    challenges,
                );
                for (denominator, inverse_column, zero_column) in [
                    (
                        left_denominator,
                        AUX_BYTE_LEFT_QUERY_INVERSE + lane,
                        AUX_BYTE_LEFT_QUERY_ZERO + lane,
                    ),
                    (
                        right_denominator,
                        AUX_BYTE_RIGHT_QUERY_INVERSE + lane,
                        AUX_BYTE_RIGHT_QUERY_ZERO + lane,
                    ),
                ] {
                    if denominator == F::ZERO {
                        aux[zero_column] = F::ONE;
                        byte_query_zero_count[lane] = byte_query_zero_count[lane].add(F::ONE);
                    } else {
                        let inverse = denominator
                            .inv()
                            .expect("nonzero canonical lookup denominator");
                        aux[inverse_column] = inverse;
                        byte_query_sum[lane] = byte_query_sum[lane].add(inverse);
                    }
                }
            }
        }
        write_aux_lanes_v1(&mut aux, AUX_STACK_PUSH_AFTER, stack_push);
        write_aux_lanes_v1(&mut aux, AUX_STACK_POP_AFTER, stack_pop);
        write_aux_lanes_v1(&mut aux, AUX_DOCUMENT_AFTER, document);
        write_aux_lanes_v1(&mut aux, AUX_NODE_AFTER, node);
        write_aux_lanes_v1(&mut aux, AUX_PAIR_PRODUCER_AFTER, pair_producer);
        write_aux_lanes_v1(&mut aux, AUX_PAIR_CONSUMER_AFTER, pair_consumer);
        write_aux_lanes_v1(&mut aux, AUX_BYTE_TABLE_SUM_AFTER, byte_table_sum);
        write_aux_lanes_v1(&mut aux, AUX_BYTE_QUERY_SUM_AFTER, byte_query_sum);
        write_aux_lanes_v1(
            &mut aux,
            AUX_BYTE_TABLE_ZERO_COUNT_AFTER,
            byte_table_zero_count,
        );
        write_aux_lanes_v1(
            &mut aux,
            AUX_BYTE_QUERY_ZERO_COUNT_AFTER,
            byte_query_zero_count,
        );
        write_aux_lanes_v1(&mut aux, AUX_INPUT_BYTE_AFTER, input_byte);
        aux_rows.push(aux);
    }
    let trace = ZkX509DerStarkTraceV1 { base, aux_rows };
    let terminals = zk_x509_der_stark_terminals_v1(&trace)?;
    let private_document_product = derive_zk_x509_der_stark_private_document_product_v1(
        &trace.base.private_shape,
        challenges,
    )?;
    if terminals.stack_push != terminals.stack_pop
        || terminals.document != private_document_product
        || terminals.pair_producer != terminals.pair_consumer
        || terminals.byte_table_sum != terminals.byte_query_sum
        || terminals.byte_table_zero_count != terminals.byte_query_zero_count
    {
        return Err(ZkX509DerStarkErrorV1::Transition);
    }
    Ok(trace)
}
/// Reconstruct one native-domain base row, including exact zero padding.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn zk_x509_der_stark_aggregate_base_row_v1(
    base: &ZkX509DerStarkBaseV1,
    index: usize,
) -> Result<[F; ZK_X509_DER_STARK_BASE_WIDTH_V1], ZkX509DerStarkErrorV1> {
    if index >= ZK_X509_DER_STARK_TRACE_SIZE_V1 {
        return Err(ZkX509DerStarkErrorV1::Resource);
    }
    let compact_index = if index < base.private_shape.parser_rows {
        Some(index)
    } else if (ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1
        ..ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 + base.private_shape.comparator_rows)
        .contains(&index)
    {
        Some(
            base.private_shape
                .parser_rows
                .checked_add(index - ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1)
                .ok_or(ZkX509DerStarkErrorV1::Resource)?,
        )
    } else {
        None
    };
    if let Some(compact_index) = compact_index {
        return base
            .rows
            .get(compact_index)
            .copied()
            .ok_or(ZkX509DerStarkErrorV1::Row);
    }
    let mut row = [F::ZERO; ZK_X509_DER_STARK_BASE_WIDTH_V1];
    let final_document = base
        .private_shape
        .document_lengths
        .len()
        .checked_sub(1)
        .ok_or(ZkX509DerStarkErrorV1::Shape)?;
    row[BASE_FINAL_DOCUMENT] =
        F(u64::try_from(final_document).map_err(|_| ZkX509DerStarkErrorV1::Resource)?);
    write_bits_v1(
        &mut row,
        BASE_FINAL_DOCUMENT_BITS,
        5,
        u64::try_from(final_document).map_err(|_| ZkX509DerStarkErrorV1::Resource)?,
    );
    write_bits_v1(
        &mut row,
        BASE_FINAL_DOCUMENT_SLACK_BITS,
        5,
        u64::try_from(ZK_X509_DER_STARK_MAX_DOCUMENTS_V1 - 1 - final_document)
            .map_err(|_| ZkX509DerStarkErrorV1::Resource)?,
    );
    Ok(row)
}
/// Reconstruct one native-domain auxiliary row. Padding carries every public
/// and cross-adapter terminal while all local inverse witnesses are zero.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn zk_x509_der_stark_aggregate_aux_row_v1(
    trace: &ZkX509DerStarkTraceV1,
    index: usize,
) -> Result<[F; ZK_X509_DER_STARK_AUX_WIDTH_V1], ZkX509DerStarkErrorV1> {
    if index >= ZK_X509_DER_STARK_TRACE_SIZE_V1 {
        return Err(ZkX509DerStarkErrorV1::Resource);
    }
    let compact_index = if index < trace.base.private_shape.parser_rows {
        Some(index)
    } else if (ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1
        ..ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 + trace.base.private_shape.comparator_rows)
        .contains(&index)
    {
        Some(
            trace
                .base
                .private_shape
                .parser_rows
                .checked_add(index - ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1)
                .ok_or(ZkX509DerStarkErrorV1::Resource)?,
        )
    } else {
        None
    };
    if let Some(compact_index) = compact_index {
        return trace
            .aux_rows
            .get(compact_index)
            .copied()
            .ok_or(ZkX509DerStarkErrorV1::Row);
    }
    let carry_compact_index = if index < ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 {
        trace
            .base
            .private_shape
            .parser_rows
            .checked_sub(1)
            .ok_or(ZkX509DerStarkErrorV1::Shape)?
    } else {
        trace
            .aux_rows
            .len()
            .checked_sub(1)
            .ok_or(ZkX509DerStarkErrorV1::Shape)?
    };
    let carry = trace
        .aux_rows
        .get(carry_compact_index)
        .ok_or(ZkX509DerStarkErrorV1::Row)?;
    let mut row = [F::ZERO; ZK_X509_DER_STARK_AUX_WIDTH_V1];
    let schedule = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1)?;
    populate_low_degree_auxiliaries_v1(
        &zk_x509_der_stark_aggregate_base_row_v1(&trace.base, index)?,
        &schedule.fixed_row(index)?,
        &mut row,
    )?;
    for (before, after) in [
        (AUX_STACK_PUSH_BEFORE, AUX_STACK_PUSH_AFTER),
        (AUX_STACK_POP_BEFORE, AUX_STACK_POP_AFTER),
        (AUX_DOCUMENT_BEFORE, AUX_DOCUMENT_AFTER),
        (AUX_NODE_BEFORE, AUX_NODE_AFTER),
        (AUX_PAIR_PRODUCER_BEFORE, AUX_PAIR_PRODUCER_AFTER),
        (AUX_PAIR_CONSUMER_BEFORE, AUX_PAIR_CONSUMER_AFTER),
        (AUX_BYTE_TABLE_SUM_BEFORE, AUX_BYTE_TABLE_SUM_AFTER),
        (AUX_BYTE_QUERY_SUM_BEFORE, AUX_BYTE_QUERY_SUM_AFTER),
        (
            AUX_BYTE_TABLE_ZERO_COUNT_BEFORE,
            AUX_BYTE_TABLE_ZERO_COUNT_AFTER,
        ),
        (
            AUX_BYTE_QUERY_ZERO_COUNT_BEFORE,
            AUX_BYTE_QUERY_ZERO_COUNT_AFTER,
        ),
        (AUX_INPUT_BYTE_BEFORE, AUX_INPUT_BYTE_AFTER),
    ] {
        let values = read_aux_lanes_v1(carry, after);
        write_aux_lanes_v1(&mut row, before, values);
        write_aux_lanes_v1(&mut row, after, values);
    }
    Ok(row)
}
#[cfg(test)]
pub(crate) fn zk_x509_der_stark_native_base_cell_v1(
    base: &ZkX509DerStarkBaseV1,
    row: usize,
    column: usize,
) -> Result<F, ZkX509DerStarkErrorV1> {
    if column >= ZK_X509_DER_STARK_BASE_WIDTH_V1 {
        return Err(ZkX509DerStarkErrorV1::Resource);
    }
    Ok(zk_x509_der_stark_aggregate_base_row_v1(base, row)?[column])
}
#[cfg(test)]
pub(crate) fn zk_x509_der_stark_native_aux_cell_v1(
    trace: &ZkX509DerStarkTraceV1,
    row: usize,
    column: usize,
) -> Result<F, ZkX509DerStarkErrorV1> {
    if column >= ZK_X509_DER_STARK_AUX_WIDTH_V1 {
        return Err(ZkX509DerStarkErrorV1::Resource);
    }
    Ok(zk_x509_der_stark_aggregate_aux_row_v1(trace, row)?[column])
}
#[cfg(test)]
pub(crate) fn zk_x509_der_stark_native_fixed_cell_v1(
    schedule: &ZkX509DerStarkFixedScheduleV1,
    row: usize,
    column: usize,
) -> Result<F, ZkX509DerStarkErrorV1> {
    if column >= ZK_X509_DER_STARK_FIXED_WIDTH_V1 {
        return Err(ZkX509DerStarkErrorV1::Resource);
    }
    Ok(schedule.fixed_row(row)?[column])
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn allocate_native_column_v1() -> Result<Vec<F>, ZkX509DerStarkErrorV1> {
    let mut column = Vec::new();
    column
        .try_reserve_exact(ZK_X509_DER_STARK_TRACE_SIZE_V1)
        .map_err(|_| ZkX509DerStarkErrorV1::Resource)?;
    Ok(column)
}
/// Generate one base column over the full native domain. Callers can commit
/// and drop it before requesting the next column.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn build_zk_x509_der_stark_native_base_column_v1(
    base: &ZkX509DerStarkBaseV1,
    column_index: usize,
) -> Result<Vec<F>, ZkX509DerStarkErrorV1> {
    if column_index >= ZK_X509_DER_STARK_BASE_WIDTH_V1 {
        return Err(ZkX509DerStarkErrorV1::Resource);
    }
    let mut column = allocate_native_column_v1()?;
    for row in 0..ZK_X509_DER_STARK_TRACE_SIZE_V1 {
        column.push(zk_x509_der_stark_aggregate_base_row_v1(base, row)?[column_index]);
    }
    Ok(column)
}
/// Generate one auxiliary column over the full native domain, carrying the
/// exact final terminal through aggregate padding.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn build_zk_x509_der_stark_native_aux_column_v1(
    trace: &ZkX509DerStarkTraceV1,
    column_index: usize,
) -> Result<Vec<F>, ZkX509DerStarkErrorV1> {
    if column_index >= ZK_X509_DER_STARK_AUX_WIDTH_V1 {
        return Err(ZkX509DerStarkErrorV1::Resource);
    }
    let mut column = allocate_native_column_v1()?;
    for row in 0..ZK_X509_DER_STARK_TRACE_SIZE_V1 {
        column.push(zk_x509_der_stark_aggregate_aux_row_v1(trace, row)?[column_index]);
    }
    Ok(column)
}
/// Generate one verifier-owned fixed column over the full native domain.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn build_zk_x509_der_stark_native_fixed_column_v1(
    schedule: &ZkX509DerStarkFixedScheduleV1,
    column_index: usize,
) -> Result<Vec<F>, ZkX509DerStarkErrorV1> {
    if column_index >= ZK_X509_DER_STARK_FIXED_WIDTH_V1 {
        return Err(ZkX509DerStarkErrorV1::Resource);
    }
    let mut column = allocate_native_column_v1()?;
    for row in 0..ZK_X509_DER_STARK_TRACE_SIZE_V1 {
        column.push(schedule.fixed_row(row)?[column_index]);
    }
    Ok(column)
}
fn push_boolean_residues_v1(residues: &mut Vec<F>, gate: F, values: &[F]) {
    residues.extend(
        values
            .iter()
            .copied()
            .map(|value| gate.mul(value).mul(value.sub(F::ONE))),
    );
}
fn push_carry_residues_v1(
    residues: &mut Vec<F>,
    gate: F,
    current: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1],
    next: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1],
    columns: &[usize],
) {
    residues.extend(
        columns
            .iter()
            .copied()
            .map(|column| gate.mul(next[column].sub(current[column]))),
    );
}
fn push_one_hot_projection_residues_v1(residues: &mut Vec<F>, selectors: &[F], gate: F, value: F) {
    for selector in selectors {
        residues.push(selector.mul(selector.sub(F::ONE)));
    }
    residues.push(selectors.iter().copied().fold(F::ZERO, F::add).sub(gate));
    residues.push(
        selectors
            .iter()
            .copied()
            .enumerate()
            .fold(F::ZERO, |sum, (index, selector)| {
                sum.add(F(u64::try_from(index).expect("selector index fits u64")).mul(selector))
            })
            .sub(gate.mul(value)),
    );
}
fn push_zero_test_residues_v1(residues: &mut Vec<F>, value: F, selector: F, inverse: F) {
    residues.push(selector.mul(selector.sub(F::ONE)));
    residues.push(value.mul(selector));
    residues.push(value.mul(inverse).sub(F::ONE.sub(selector)));
    residues.push(selector.mul(inverse));
}
/// Constrain a total, event-gated inverse. `zero` is one exactly when an
/// active event's denominator is zero; inactive events canonically use
/// `(zero, inverse) = (0, 0)`.
fn push_gated_zero_safe_inverse_residues_v1(
    residues: &mut Vec<F>,
    gate: F,
    denominator: F,
    zero: F,
    inverse: F,
) {
    residues.push(zero.mul(zero.sub(F::ONE)));
    residues.push(F::ONE.sub(gate).mul(zero));
    residues.push(F::ONE.sub(gate).mul(inverse));
    residues.push(denominator.mul(zero));
    residues.push(denominator.mul(inverse).sub(gate.mul(F::ONE.sub(zero))));
    residues.push(zero.mul(inverse));
}
#[allow(clippy::too_many_lines)]
fn evaluate_zk_x509_der_stark_base_residues_into_v1(
    current: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1],
    next: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1],
    current_aux: &[F; ZK_X509_DER_STARK_AUX_WIDTH_V1],
    fixed: &[F; ZK_X509_DER_STARK_FIXED_WIDTH_V1],
    next_fixed: &[F; ZK_X509_DER_STARK_FIXED_WIDTH_V1],
    residues: &mut Vec<F>,
) {
    let row_active = current[BASE_ROW_ACTIVE];
    let next_row_active = next[BASE_ROW_ACTIVE];
    let parser = fixed[FIX_PARSER].mul(row_active);
    let comparator = fixed[FIX_COMPARATOR].mul(row_active);
    let next_parser = next_fixed[FIX_PARSER].mul(next_row_active);
    let next_comparator = next_fixed[FIX_COMPARATOR].mul(next_row_active);
    let parser_continue = parser.mul(next_parser);
    let last_parser = parser.mul(F::ONE.sub(next_parser));
    let last_comparator = comparator.mul(F::ONE.sub(next_comparator));
    let phases: &[F; 8] = current_aux[AUX_PHASE_SELECTORS..AUX_PHASE_SELECTORS + 8]
        .try_into()
        .expect("eight phase selectors");
    let next_phase_value = pack_bits_v1(&next[BASE_PHASE_BITS..BASE_PHASE_BITS + 3]);
    let phase_value = pack_bits_v1(&current[BASE_PHASE_BITS..BASE_PHASE_BITS + 3]);
    let identifier_first = phases[PHASE_IDENTIFIER_FIRST];
    let identifier_high = phases[PHASE_IDENTIFIER_HIGH];
    let length_first = phases[PHASE_LENGTH_FIRST];
    let length_body = phases[PHASE_LENGTH_BODY];
    let finalize = phases[PHASE_FINALIZE_HEADER];
    let primitive = phases[PHASE_PRIMITIVE_CONTENT];
    let boundary = phases[PHASE_BOUNDARY];
    let consuming = identifier_first
        .add(identifier_high)
        .add(length_first)
        .add(length_body)
        .add(primitive);
    let non_consuming = finalize.add(boundary);
    residues.clear();
    residues.push(row_active.mul(row_active.sub(F::ONE)));
    residues.push(row_active.mul(F::ONE.sub(fixed[FIX_ACTIVE])));
    residues.push(row_active.sub(parser).sub(comparator));
    residues.push(fixed[FIX_FIRST_ACTIVE].mul(row_active.sub(F::ONE)));
    residues.push(fixed[FIX_FIRST_PARSER].mul(parser.sub(F::ONE)));
    residues.push(
        fixed[FIX_PARSER_CONTINUE]
            .mul(next_row_active)
            .mul(F::ONE.sub(row_active)),
    );
    let comparator_capacity_continue =
        fixed[FIX_COMPARATOR].mul(F::ONE.sub(fixed[FIX_LAST_COMPARATOR]));
    residues.push(
        comparator_capacity_continue
            .mul(next_row_active)
            .mul(F::ONE.sub(row_active)),
    );
    for (column, value) in current.iter().copied().enumerate() {
        if column != BASE_FINAL_DOCUMENT
            && !(BASE_FINAL_DOCUMENT_BITS..BASE_FINAL_DOCUMENT_BITS + 5).contains(&column)
            && !(BASE_FINAL_DOCUMENT_SLACK_BITS..BASE_FINAL_DOCUMENT_SLACK_BITS + 5)
                .contains(&column)
        {
            residues.push(F::ONE.sub(row_active).mul(value));
        }
    }
    push_boolean_residues_v1(
        residues,
        F::ONE,
        &current[BASE_FINAL_DOCUMENT_BITS..BASE_FINAL_DOCUMENT_BITS + 5],
    );
    push_boolean_residues_v1(
        residues,
        F::ONE,
        &current[BASE_FINAL_DOCUMENT_SLACK_BITS..BASE_FINAL_DOCUMENT_SLACK_BITS + 5],
    );
    let final_document =
        pack_bits_v1(&current[BASE_FINAL_DOCUMENT_BITS..BASE_FINAL_DOCUMENT_BITS + 5]);
    let final_document_slack =
        pack_bits_v1(&current[BASE_FINAL_DOCUMENT_SLACK_BITS..BASE_FINAL_DOCUMENT_SLACK_BITS + 5]);
    residues.push(current[BASE_FINAL_DOCUMENT].sub(final_document));
    residues.push(final_document.add(final_document_slack).sub(F(
        u64::try_from(ZK_X509_DER_STARK_MAX_DOCUMENTS_V1 - 1).expect("fixed cap"),
    )));
    residues.push(
        F::ONE
            .sub(fixed[FIX_LAST_AGGREGATE])
            .mul(next[BASE_FINAL_DOCUMENT].sub(current[BASE_FINAL_DOCUMENT])),
    );
    push_one_hot_projection_residues_v1(residues, phases, parser, phase_value);
    let depth = pack_bits_v1(&current[BASE_DEPTH_BITS..BASE_DEPTH_BITS + 5]);
    let depth_selectors: &[F; 17] = current_aux
        [AUX_DEPTH_SELECTORS..AUX_DEPTH_SELECTORS + ZK_X509_DER_MAX_NESTING_DEPTH_V1 + 1]
        .try_into()
        .expect("seventeen depth selectors");
    push_one_hot_projection_residues_v1(residues, depth_selectors, parser, depth);
    let count_selectors: &[F; 8] = current_aux
        [AUX_IDENTIFIER_COUNT_SELECTORS..AUX_IDENTIFIER_COUNT_SELECTORS + 8]
        .try_into()
        .expect("eight identifier-count selectors");
    push_one_hot_projection_residues_v1(
        residues,
        count_selectors,
        identifier_high,
        pack_bits_v1(&current[BASE_PAYLOAD..BASE_PAYLOAD + 3]),
    );
    let kind_selectors: &[F; 8] = current_aux
        [AUX_PRIMITIVE_KIND_SELECTORS..AUX_PRIMITIVE_KIND_SELECTORS + 8]
        .try_into()
        .expect("eight primitive-kind selectors");
    push_one_hot_projection_residues_v1(
        residues,
        kind_selectors,
        primitive,
        pack_bits_v1(&current[BASE_PAYLOAD..BASE_PAYLOAD + 3]),
    );
    let unused_selectors: &[F; 8] = current_aux
        [AUX_UNUSED_BIT_SELECTORS..AUX_UNUSED_BIT_SELECTORS + 8]
        .try_into()
        .expect("eight unused-bit selectors");
    push_one_hot_projection_residues_v1(
        residues,
        unused_selectors,
        primitive,
        pack_bits_v1(&current[BASE_PAYLOAD + 3..BASE_PAYLOAD + 6]),
    );
    let remaining_selectors: &[F; 4] = current_aux
        [AUX_LENGTH_REMAINING_SELECTORS..AUX_LENGTH_REMAINING_SELECTORS + 4]
        .try_into()
        .expect("four length-remaining selectors");
    push_one_hot_projection_residues_v1(
        residues,
        remaining_selectors,
        length_body,
        pack_bits_v1(&current[BASE_PAYLOAD..BASE_PAYLOAD + 2]),
    );
    let byte_bits = &current[BASE_BYTE_BITS..BASE_BYTE_BITS + 8];
    residues.push(current_aux[AUX_LOW_FIVE_PAIR_01].sub(byte_bits[0].mul(byte_bits[1])));
    residues.push(current_aux[AUX_LOW_FIVE_PAIR_23].sub(byte_bits[2].mul(byte_bits[3])));
    residues.push(
        current_aux[AUX_HIGH_TAG].sub(
            current_aux[AUX_LOW_FIVE_PAIR_01]
                .mul(current_aux[AUX_LOW_FIVE_PAIR_23])
                .mul(byte_bits[4]),
        ),
    );
    let high_low = pack_bits_v1(&byte_bits[..7]);
    push_zero_test_residues_v1(
        residues,
        high_low,
        current_aux[AUX_HIGH_LOW_ZERO],
        current_aux[AUX_HIGH_LOW_INVERSE],
    );
    residues.push(
        current_aux[AUX_HIGH_LOW_GE_31].sub(
            identifier_high.mul(
                F::ONE.sub(
                    F::ONE
                        .sub(byte_bits[6])
                        .mul(F::ONE.sub(byte_bits[5]))
                        .mul(F::ONE.sub(current_aux[AUX_HIGH_TAG])),
                ),
            ),
        ),
    );
    residues.push(current_aux[AUX_HIGH_LOW_GE_31].mul(current_aux[AUX_HIGH_LOW_GE_31].sub(F::ONE)));
    let long_length = byte_bits[7];
    residues.push(
        current_aux[AUX_LENGTH_COUNT_TWO]
            .sub(length_first.mul(long_length).mul(high_low.sub(F::ONE))),
    );
    residues
        .push(current_aux[AUX_LENGTH_COUNT_TWO].mul(current_aux[AUX_LENGTH_COUNT_TWO].sub(F::ONE)));
    push_zero_test_residues_v1(
        residues,
        current[BASE_BYTE_VALUE],
        current_aux[AUX_BYTE_ZERO],
        current_aux[AUX_BYTE_INVERSE],
    );
    push_zero_test_residues_v1(
        residues,
        current[BASE_BYTE_VALUE].sub(F(64)),
        current_aux[AUX_BYTE_IS_64],
        current_aux[AUX_BYTE_64_INVERSE],
    );
    push_zero_test_residues_v1(
        residues,
        current[BASE_BYTE_VALUE].sub(F(128)),
        current_aux[AUX_BYTE_IS_128],
        current_aux[AUX_BYTE_128_INVERSE],
    );
    let count_one = count_selectors[1];
    for bit in 0..7 {
        residues.push(
            current_aux[AUX_UPDATED_FIRST_HIGH_BITS + bit].sub(
                identifier_high.mul(
                    current[BASE_PAYLOAD + 3 + bit]
                        .add(count_one.mul(byte_bits[bit].sub(current[BASE_PAYLOAD + 3 + bit]))),
                ),
            ),
        );
    }
    let last_primitive = current[BASE_CHECK_IS_ZERO];
    residues.push(
        current_aux[AUX_SIGNED_FIRST_GUARD].sub(
            kind_selectors[2]
                .add(kind_selectors[6])
                .mul(current[BASE_PRIMITIVE_FIRST])
                .mul(F::ONE.sub(last_primitive)),
        ),
    );
    residues.push(
        current_aux[AUX_BIT_STRING_FIRST_GUARD]
            .sub(kind_selectors[3].mul(current[BASE_PRIMITIVE_FIRST])),
    );
    residues.push(
        current_aux[AUX_BIT_STRING_LAST_CONTINUATION_GUARD].sub(
            kind_selectors[3]
                .mul(F::ONE.sub(current[BASE_PRIMITIVE_FIRST]))
                .mul(last_primitive),
        ),
    );
    residues.push(
        current_aux[AUX_NEXT_OID_START_EXPECTED].sub(
            kind_selectors[5]
                .mul(F::ONE.sub(last_primitive))
                .mul(F::ONE.sub(byte_bits[7])),
        ),
    );
    residues.push(
        current_aux[AUX_PRIMITIVE_ENTRY].sub(
            finalize
                .mul(F::ONE.sub(current[BASE_CONSTRUCTED]))
                .mul(F::ONE.sub(current[BASE_CHECK_IS_ZERO])),
        ),
    );
    residues.push(
        current_aux[AUX_ENTERS_CHILD]
            .sub(current[BASE_CONSTRUCTED].mul(F::ONE.sub(current[BASE_CHECK_IS_ZERO]))),
    );
    residues.push(
        current_aux[AUX_PAIR_PRODUCER_EVENT].sub(
            finalize
                .mul(current[BASE_FRAME_IS_SET])
                .mul(current[BASE_FRAME_HAS_CHILD]),
        ),
    );
    push_boolean_residues_v1(
        residues,
        parser,
        &current[BASE_PHASE_BITS..BASE_PHASE_BITS + 3],
    );
    residues.push(
        parser.mul(
            phase_value.sub(
                current[BASE_PHASE_BITS]
                    .add(F(2).mul(current[BASE_PHASE_BITS + 1]))
                    .add(F(4).mul(current[BASE_PHASE_BITS + 2])),
            ),
        ),
    );
    push_boolean_residues_v1(residues, parser.mul(consuming), byte_bits);
    residues.push(
        parser
            .mul(consuming)
            .mul(current[BASE_BYTE_VALUE].sub(pack_bits_v1(byte_bits))),
    );
    residues.push(parser.mul(non_consuming).mul(current[BASE_BYTE_VALUE]));
    for bit in byte_bits {
        residues.push(parser.mul(non_consuming).mul(*bit));
    }
    push_boolean_residues_v1(
        residues,
        parser,
        &current[BASE_TAG_CLASS_BITS..BASE_TAG_CLASS_BITS + 2],
    );
    residues.push(parser.mul(current[BASE_TAG_CLASS].sub(pack_bits_v1(
        &current[BASE_TAG_CLASS_BITS..BASE_TAG_CLASS_BITS + 2],
    ))));
    push_boolean_residues_v1(
        residues,
        parser,
        &[
            current[BASE_CONSTRUCTED],
            current[BASE_FRAME_IS_SET],
            current[BASE_FRAME_HAS_CHILD],
            current[BASE_CHECK_IS_ZERO],
            current[BASE_PRIMITIVE_FIRST],
            current[BASE_OID_START],
            current[BASE_DOCUMENT_FIRST],
        ],
    );
    push_boolean_residues_v1(
        residues,
        parser,
        &current[BASE_DEPTH_BITS..BASE_DEPTH_BITS + 5],
    );
    let depth_zero = depth_selectors[0];
    let depth_one = depth_selectors[1];
    let check_used = identifier_first.add(finalize).add(primitive).add(boundary);
    residues.push(
        parser
            .mul(F::ONE.sub(check_used))
            .mul(current[BASE_CHECK_IS_ZERO]),
    );
    residues.push(
        parser
            .mul(F::ONE.sub(check_used))
            .mul(current[BASE_CHECK_INVERSE]),
    );
    let boundary_delta = current[BASE_CONSTRUCTED]
        .mul(
            depth_one
                .mul(current[BASE_DOCUMENT_LEN].sub(current[BASE_OFFSET]))
                .add(
                    F::ONE
                        .sub(depth_one)
                        .mul(current[BASE_PAYLOAD + 2].sub(current[BASE_OFFSET])),
                ),
        )
        .add(
            F::ONE.sub(current[BASE_CONSTRUCTED]).mul(
                depth_zero
                    .mul(current[BASE_DOCUMENT_LEN].sub(current[BASE_OFFSET]))
                    .add(
                        F::ONE
                            .sub(depth_zero)
                            .mul(current[BASE_FRAME_END].sub(current[BASE_OFFSET])),
                    ),
            ),
        );
    let expected_check_delta = identifier_first
        .mul(
            F(u64::try_from(ZK_X509_DER_MAX_VALUES_V1).expect("cap")).sub(current[BASE_NODE_COUNT]),
        )
        .add(finalize.mul(current[BASE_LENGTH_ACCUMULATOR]))
        .add(
            primitive.mul(
                current[BASE_CONTENT_END]
                    .sub(current[BASE_OFFSET])
                    .sub(F::ONE),
            ),
        )
        .add(boundary.mul(boundary_delta));
    residues.push(current_aux[AUX_CHECK_DELTA].sub(expected_check_delta));
    let check_delta = current_aux[AUX_CHECK_DELTA];
    residues.push(check_used.mul(check_delta).mul(current[BASE_CHECK_IS_ZERO]));
    residues.push(
        check_used.mul(
            check_delta
                .mul(current[BASE_CHECK_INVERSE])
                .sub(F::ONE.sub(current[BASE_CHECK_IS_ZERO])),
        ),
    );
    residues.push(
        check_used
            .mul(current[BASE_CHECK_IS_ZERO])
            .mul(current[BASE_CHECK_INVERSE]),
    );
    residues.push(identifier_first.mul(current[BASE_CHECK_IS_ZERO]));
    // Payloads are typed by numeric phase selectors; every unused cell is
    // forced to zero so no hidden witness channel survives.
    let identifier_payload = identifier_first.add(identifier_high);
    let length_payload = length_first.add(length_body);
    for bit in &current[BASE_PAYLOAD..BASE_PAYLOAD + 10] {
        push_boolean_residues_v1(residues, parser.mul(identifier_payload), &[*bit]);
    }
    for bit in &current[BASE_PAYLOAD..BASE_PAYLOAD + 4] {
        push_boolean_residues_v1(residues, parser.mul(length_payload), &[*bit]);
    }
    push_boolean_residues_v1(
        residues,
        parser.mul(finalize),
        &current[BASE_PAYLOAD..BASE_PAYLOAD + 19],
    );
    push_boolean_residues_v1(
        residues,
        parser.mul(primitive),
        &current[BASE_PAYLOAD..BASE_PAYLOAD + 6],
    );
    push_boolean_residues_v1(
        residues,
        parser.mul(primitive),
        &[current[BASE_PAYLOAD + 6], current[BASE_PAYLOAD + 8]],
    );
    push_boolean_residues_v1(
        residues,
        parser.mul(boundary),
        &[current[BASE_PAYLOAD + 3], current[BASE_PAYLOAD + 4]],
    );
    for column in 0..19 {
        let used = identifier_payload
            .mul(F(u64::from(column < 10)))
            .add(length_payload.mul(F(u64::from(column < 4))))
            .add(finalize)
            .add(primitive.mul(F(u64::from(column < 10))))
            .add(boundary.mul(F(u64::from(column < 8))));
        residues.push(
            parser
                .mul(F::ONE.sub(used))
                .mul(current[BASE_PAYLOAD + column]),
        );
    }
    residues.push(
        parser
            .mul(F::ONE.sub(primitive))
            .mul(current[BASE_PRIMITIVE_FIRST]),
    );
    residues.push(
        parser
            .mul(F::ONE.sub(primitive))
            .mul(current[BASE_OID_START]),
    );
    residues.push(
        parser
            .mul(F::ONE.sub(primitive))
            .mul(current[BASE_UNUSED_BITS]),
    );
    // Initial document state.
    residues.push(fixed[FIX_FIRST_PARSER].mul(current[BASE_DOCUMENT_FIRST].sub(F::ONE)));
    residues.push(fixed[FIX_FIRST_PARSER].mul(current[BASE_DOCUMENT]));
    residues.push(fixed[FIX_FIRST_PARSER].mul(current[BASE_OFFSET]));
    residues.push(fixed[FIX_FIRST_PARSER].mul(current[BASE_NODE_COUNT]));
    residues.push(fixed[FIX_FIRST_PARSER].mul(depth));
    for column in BASE_FRAME_ID..=BASE_FRAME_PREVIOUS_END {
        residues.push(fixed[FIX_FIRST_PARSER].mul(current[column]));
    }
    residues.push(
        parser
            .mul(current[BASE_DOCUMENT_FIRST])
            .mul(current[BASE_OFFSET]),
    );
    residues.push(
        parser
            .mul(current[BASE_DOCUMENT_FIRST])
            .mul(current[BASE_NODE_COUNT]),
    );
    residues.push(
        parser
            .mul(current[BASE_DOCUMENT_FIRST])
            .mul(F::ONE.sub(identifier_first)),
    );
    residues.push(
        parser
            .mul(non_consuming)
            .mul(current[BASE_BYTE_LOOKUP_MULTIPLICITY]),
    );
    let carry_common = [
        BASE_DOCUMENT,
        BASE_DOCUMENT_LEN,
        BASE_NODE_COUNT,
        BASE_FRAME_ID,
        BASE_FRAME_START,
        BASE_FRAME_END,
        BASE_FRAME_IS_SET,
        BASE_FRAME_HAS_CHILD,
        BASE_FRAME_PREVIOUS_ID,
        BASE_FRAME_PREVIOUS_START,
        BASE_FRAME_PREVIOUS_END,
    ];
    let ordinary_depth_carry = parser_continue.mul(
        identifier_first
            .add(identifier_high)
            .add(length_first)
            .add(length_body)
            .add(primitive),
    );
    residues.push(
        ordinary_depth_carry
            .mul(pack_bits_v1(&next[BASE_DEPTH_BITS..BASE_DEPTH_BITS + 5]).sub(depth)),
    );
    // Identifier first octet.
    let low_tag = pack_bits_v1(&byte_bits[..5]);
    let high_tag = current_aux[AUX_HIGH_TAG];
    residues.push(identifier_first.mul(current[BASE_NODE_START].sub(current[BASE_OFFSET])));
    residues.push(identifier_first.mul(current[BASE_TAG_CLASS]));
    residues.push(identifier_first.mul(current[BASE_CONSTRUCTED]));
    residues.push(identifier_first.mul(current[BASE_TAG_ACCUMULATOR]));
    residues.push(identifier_first.mul(current[BASE_LENGTH_ACCUMULATOR]));
    residues.push(identifier_first.mul(current[BASE_CONTENT_START]));
    residues.push(identifier_first.mul(current[BASE_CONTENT_END]));
    for bit in &current[BASE_PAYLOAD..BASE_PAYLOAD + 10] {
        residues.push(identifier_first.mul(*bit));
    }
    residues.push(
        parser_continue
            .mul(identifier_first)
            .mul(next[BASE_OFFSET].sub(current[BASE_OFFSET].add(F::ONE))),
    );
    residues.push(
        parser_continue.mul(identifier_first).mul(
            next_phase_value.sub(
                high_tag
                    .mul(F(PHASE_IDENTIFIER_HIGH as u64))
                    .add(F::ONE.sub(high_tag).mul(F(PHASE_LENGTH_FIRST as u64))),
            ),
        ),
    );
    residues.push(
        parser_continue
            .mul(identifier_first)
            .mul(next[BASE_TAG_CLASS].sub(byte_bits[6].add(F(2).mul(byte_bits[7])))),
    );
    residues.push(
        parser_continue
            .mul(identifier_first)
            .mul(next[BASE_CONSTRUCTED].sub(byte_bits[5])),
    );
    residues.push(
        parser_continue
            .mul(identifier_first)
            .mul(next[BASE_TAG_ACCUMULATOR].sub(low_tag)),
    );
    residues.push(
        parser_continue
            .mul(identifier_first)
            .mul(next[BASE_NODE_START].sub(current[BASE_OFFSET])),
    );
    for column in [
        BASE_LENGTH_ACCUMULATOR,
        BASE_CONTENT_START,
        BASE_CONTENT_END,
    ] {
        residues.push(parser_continue.mul(identifier_first).mul(next[column]));
    }
    residues.push(
        parser_continue
            .mul(identifier_first)
            .mul(next[BASE_DOCUMENT_FIRST]),
    );
    for column in carry_common {
        if column != BASE_NODE_COUNT {
            residues.push(
                parser_continue
                    .mul(identifier_first)
                    .mul(next[column].sub(current[column])),
            );
        }
    }
    residues.push(
        parser_continue
            .mul(identifier_first)
            .mul(next[BASE_NODE_COUNT].sub(current[BASE_NODE_COUNT])),
    );
    for bit in 0..3 {
        let expected = high_tag.mul(F(u64::from(bit == 0)));
        residues.push(
            parser_continue
                .mul(identifier_first)
                .mul(next[BASE_PAYLOAD + bit].sub(expected)),
        );
    }
    // High-tag base-128 continuation.
    let identifier_count = pack_bits_v1(&current[BASE_PAYLOAD..BASE_PAYLOAD + 3]);
    let count_one = count_selectors[1];
    let count_five = count_selectors[5];
    residues.push(
        identifier_high.mul(
            count_selectors[1..=5]
                .iter()
                .copied()
                .fold(F::ZERO, F::add)
                .sub(F::ONE),
        ),
    );
    let high_low_zero = current_aux[AUX_HIGH_LOW_ZERO];
    residues.push(identifier_high.mul(count_one).mul(high_low_zero));
    residues.push(
        identifier_high
            .mul(count_one)
            .mul(F::ONE.sub(byte_bits[7]))
            .mul(F::ONE.sub(current_aux[AUX_HIGH_LOW_GE_31])),
    );
    residues.push(identifier_high.mul(count_five).mul(byte_bits[7]));
    for bit in 4..7 {
        residues.push(
            identifier_high
                .mul(count_five)
                .mul(current[BASE_PAYLOAD + 3 + bit]),
        );
    }
    let updated_tag = count_one.mul(high_low).add(
        F::ONE
            .sub(count_one)
            .mul(current[BASE_TAG_ACCUMULATOR].mul(F(128)).add(high_low)),
    );
    residues.push(
        parser_continue
            .mul(identifier_high)
            .mul(next[BASE_OFFSET].sub(current[BASE_OFFSET].add(F::ONE))),
    );
    residues.push(
        parser_continue
            .mul(identifier_high)
            .mul(next[BASE_TAG_ACCUMULATOR].sub(updated_tag)),
    );
    residues.push(
        parser_continue.mul(identifier_high).mul(
            next_phase_value.sub(
                byte_bits[7]
                    .mul(F(PHASE_IDENTIFIER_HIGH as u64))
                    .add(F::ONE.sub(byte_bits[7]).mul(F(PHASE_LENGTH_FIRST as u64))),
            ),
        ),
    );
    for bit in 0..3 {
        let expected_count_bit = {
            let incremented = identifier_count.add(F::ONE);
            // The packed next count is constrained below; individual
            // Booleanity is already part of the next row.
            if bit == 0 { incremented } else { F::ZERO }
        };
        if bit == 0 {
            residues.push(
                parser_continue.mul(identifier_high).mul(byte_bits[7]).mul(
                    pack_bits_v1(&next[BASE_PAYLOAD..BASE_PAYLOAD + 3]).sub(expected_count_bit),
                ),
            );
        }
    }
    for bit in 0..7 {
        residues.push(
            parser_continue.mul(identifier_high).mul(byte_bits[7]).mul(
                next[BASE_PAYLOAD + 3 + bit].sub(current_aux[AUX_UPDATED_FIRST_HIGH_BITS + bit]),
            ),
        );
    }
    residues.push(
        parser_continue
            .mul(identifier_high)
            .mul(F::ONE.sub(byte_bits[7]))
            .mul(pack_bits_v1(&next[BASE_PAYLOAD..BASE_PAYLOAD + 4])),
    );
    push_carry_residues_v1(
        residues,
        parser_continue.mul(identifier_high),
        current,
        next,
        &[
            BASE_DOCUMENT,
            BASE_DOCUMENT_LEN,
            BASE_NODE_START,
            BASE_NODE_COUNT,
            BASE_TAG_CLASS,
            BASE_CONSTRUCTED,
            BASE_LENGTH_ACCUMULATOR,
            BASE_CONTENT_START,
            BASE_CONTENT_END,
            BASE_FRAME_ID,
            BASE_FRAME_START,
            BASE_FRAME_END,
            BASE_FRAME_IS_SET,
            BASE_FRAME_HAS_CHILD,
            BASE_FRAME_PREVIOUS_ID,
            BASE_FRAME_PREVIOUS_START,
            BASE_FRAME_PREVIOUS_END,
        ],
    );
    // Length first and long-form body rows.
    let length_count = pack_bits_v1(&byte_bits[..7]);
    let length_count_two = current_aux[AUX_LENGTH_COUNT_TWO];
    residues.push(
        length_first
            .mul(long_length)
            .mul(length_count.sub(F::ONE))
            .mul(length_count.sub(F(2))),
    );
    residues.push(
        parser_continue
            .mul(length_first)
            .mul(next[BASE_OFFSET].sub(current[BASE_OFFSET].add(F::ONE))),
    );
    residues.push(
        parser_continue.mul(length_first).mul(
            next_phase_value.sub(
                long_length
                    .mul(F(PHASE_LENGTH_BODY as u64))
                    .add(F::ONE.sub(long_length).mul(F(PHASE_FINALIZE_HEADER as u64))),
            ),
        ),
    );
    residues.push(
        parser_continue
            .mul(length_first)
            .mul(next[BASE_LENGTH_ACCUMULATOR].sub(F::ONE.sub(long_length).mul(length_count))),
    );
    residues.push(
        parser_continue
            .mul(length_first)
            .mul(long_length)
            .mul(pack_bits_v1(&next[BASE_PAYLOAD..BASE_PAYLOAD + 2]).sub(length_count)),
    );
    residues.push(
        parser_continue
            .mul(length_first)
            .mul(long_length)
            .mul(next[BASE_PAYLOAD + 2].sub(length_count_two)),
    );
    residues.push(
        parser_continue
            .mul(length_first)
            .mul(long_length)
            .mul(next[BASE_PAYLOAD + 3]),
    );
    push_carry_residues_v1(
        residues,
        parser_continue.mul(length_first),
        current,
        next,
        &[
            BASE_DOCUMENT,
            BASE_DOCUMENT_LEN,
            BASE_NODE_START,
            BASE_NODE_COUNT,
            BASE_TAG_CLASS,
            BASE_CONSTRUCTED,
            BASE_TAG_ACCUMULATOR,
            BASE_FRAME_ID,
            BASE_FRAME_START,
            BASE_FRAME_END,
            BASE_FRAME_IS_SET,
            BASE_FRAME_HAS_CHILD,
            BASE_FRAME_PREVIOUS_ID,
            BASE_FRAME_PREVIOUS_START,
            BASE_FRAME_PREVIOUS_END,
        ],
    );
    let remaining_one = remaining_selectors[1];
    let remaining_two = remaining_selectors[2];
    let long_two = current[BASE_PAYLOAD + 2];
    let first_was_64 = current[BASE_PAYLOAD + 3];
    residues.push(length_body.mul(remaining_one.add(remaining_two).sub(F::ONE)));
    let first_long_body = remaining_two.add(remaining_one.mul(F::ONE.sub(long_two)));
    residues.push(first_long_body.mul(current_aux[AUX_BYTE_ZERO]));
    residues.push(remaining_two.mul(byte_bits[7]));
    residues.push(
        remaining_two
            .mul(byte_bits[6])
            .mul(byte_bits[..6].iter().copied().fold(F::ZERO, F::add)),
    );
    residues.push(
        remaining_one
            .mul(F::ONE.sub(long_two))
            .mul(F::ONE.sub(byte_bits[7])),
    );
    residues.push(
        remaining_one
            .mul(long_two)
            .mul(first_was_64)
            .mul(current[BASE_BYTE_VALUE]),
    );
    let next_length_accumulator = current[BASE_LENGTH_ACCUMULATOR]
        .mul(F(256))
        .add(current[BASE_BYTE_VALUE]);
    residues.push(
        parser_continue
            .mul(length_body)
            .mul(next[BASE_LENGTH_ACCUMULATOR].sub(next_length_accumulator)),
    );
    residues.push(
        parser_continue
            .mul(length_body)
            .mul(next[BASE_OFFSET].sub(current[BASE_OFFSET].add(F::ONE))),
    );
    residues.push(
        parser_continue.mul(length_body).mul(
            next_phase_value.sub(
                remaining_one
                    .mul(F(PHASE_FINALIZE_HEADER as u64))
                    .add(remaining_two.mul(F(PHASE_LENGTH_BODY as u64))),
            ),
        ),
    );
    residues.push(
        parser_continue
            .mul(length_body)
            .mul(remaining_two)
            .mul(pack_bits_v1(&next[BASE_PAYLOAD..BASE_PAYLOAD + 2]).sub(F::ONE)),
    );
    residues.push(
        parser_continue
            .mul(length_body)
            .mul(remaining_two)
            .mul(next[BASE_PAYLOAD + 2].sub(long_two)),
    );
    residues.push(
        parser_continue
            .mul(length_body)
            .mul(remaining_two)
            .mul(next[BASE_PAYLOAD + 3].sub(current_aux[AUX_BYTE_IS_64])),
    );
    push_carry_residues_v1(
        residues,
        parser_continue.mul(length_body),
        current,
        next,
        &[
            BASE_DOCUMENT,
            BASE_DOCUMENT_LEN,
            BASE_NODE_START,
            BASE_NODE_COUNT,
            BASE_TAG_CLASS,
            BASE_CONSTRUCTED,
            BASE_TAG_ACCUMULATOR,
            BASE_FRAME_ID,
            BASE_FRAME_START,
            BASE_FRAME_END,
            BASE_FRAME_IS_SET,
            BASE_FRAME_HAS_CHILD,
            BASE_FRAME_PREVIOUS_ID,
            BASE_FRAME_PREVIOUS_START,
            BASE_FRAME_PREVIOUS_END,
        ],
    );
    // Header finalization and the constrained universal-tag projection.
    residues.push(finalize.mul(current[BASE_CONTENT_START].sub(current[BASE_OFFSET])));
    residues.push(
        finalize.mul(
            current[BASE_CONTENT_END]
                .sub(current[BASE_CONTENT_START])
                .sub(current[BASE_LENGTH_ACCUMULATOR]),
        ),
    );
    let class_universal = F::ONE
        .sub(current[BASE_TAG_CLASS_BITS])
        .mul(F::ONE.sub(current[BASE_TAG_CLASS_BITS + 1]));
    let universal_sum = current[BASE_PAYLOAD..BASE_PAYLOAD + 19]
        .iter()
        .copied()
        .fold(F::ZERO, F::add);
    residues.push(finalize.mul(universal_sum.sub(class_universal)));
    for (selector, tag) in current[BASE_PAYLOAD..BASE_PAYLOAD + 19]
        .iter()
        .copied()
        .zip(UNIVERSAL_TAGS_V1)
    {
        residues.push(
            finalize
                .mul(selector)
                .mul(current[BASE_TAG_ACCUMULATOR].sub(F(u64::from(tag)))),
        );
    }
    let sequence = current[BASE_PAYLOAD + 8];
    let set = current[BASE_PAYLOAD + 9];
    residues.push(
        finalize
            .mul(class_universal)
            .mul(current[BASE_CONSTRUCTED].sub(sequence.add(set))),
    );
    let boolean = current[BASE_PAYLOAD];
    let integer = current[BASE_PAYLOAD + 1];
    let bit_string = current[BASE_PAYLOAD + 2];
    let null = current[BASE_PAYLOAD + 4];
    let oid = current[BASE_PAYLOAD + 5];
    let enumerated = current[BASE_PAYLOAD + 6];
    residues.push(
        finalize
            .mul(boolean)
            .mul(current[BASE_LENGTH_ACCUMULATOR].sub(F::ONE)),
    );
    residues.push(finalize.mul(null).mul(current[BASE_LENGTH_ACCUMULATOR]));
    for nonempty in [integer, bit_string, oid, enumerated] {
        residues.push(finalize.mul(nonempty).mul(current[BASE_CHECK_IS_ZERO]));
    }
    let depth_sixteen = depth_selectors[16];
    residues.push(finalize.mul(current[BASE_CONSTRUCTED]).mul(depth_sixteen));
    residues.push(
        parser_continue
            .mul(finalize)
            .mul(next[BASE_NODE_COUNT].sub(current[BASE_NODE_COUNT].add(F::ONE))),
    );
    residues.push(
        parser_continue
            .mul(finalize)
            .mul(next[BASE_OFFSET].sub(current[BASE_OFFSET])),
    );
    let content_zero = current[BASE_CHECK_IS_ZERO];
    let expected_phase = current[BASE_CONSTRUCTED]
        .mul(
            content_zero.mul(F(PHASE_BOUNDARY as u64)).add(
                F::ONE
                    .sub(content_zero)
                    .mul(F(PHASE_IDENTIFIER_FIRST as u64)),
            ),
        )
        .add(
            F::ONE.sub(current[BASE_CONSTRUCTED]).mul(
                content_zero.mul(F(PHASE_BOUNDARY as u64)).add(
                    F::ONE
                        .sub(content_zero)
                        .mul(F(PHASE_PRIMITIVE_CONTENT as u64)),
                ),
            ),
        );
    residues.push(
        parser_continue
            .mul(finalize)
            .mul(next_phase_value.sub(expected_phase)),
    );
    let expected_kind = boolean
        .add(F(2).mul(integer))
        .add(F(3).mul(bit_string))
        .add(F(4).mul(null))
        .add(F(5).mul(oid))
        .add(F(6).mul(enumerated));
    residues.push(
        parser_continue
            .mul(current_aux[AUX_PRIMITIVE_ENTRY])
            .mul(pack_bits_v1(&next[BASE_PAYLOAD..BASE_PAYLOAD + 3]).sub(expected_kind)),
    );
    residues.push(
        parser_continue.mul(
            finalize
                .mul(next[BASE_PRIMITIVE_FIRST])
                .sub(current_aux[AUX_PRIMITIVE_ENTRY]),
        ),
    );
    residues.push(
        parser_continue.mul(
            finalize
                .mul(next[BASE_OID_START])
                .sub(current_aux[AUX_PRIMITIVE_ENTRY].mul(oid)),
        ),
    );
    // Constructed rows push one exact frame; primitive rows retain the
    // current parent frame.
    residues.push(
        parser_continue.mul(finalize).mul(
            next[BASE_FRAME_ID].sub(
                current[BASE_CONSTRUCTED].mul(current[BASE_NODE_COUNT]).add(
                    F::ONE
                        .sub(current[BASE_CONSTRUCTED])
                        .mul(current[BASE_FRAME_ID]),
                ),
            ),
        ),
    );
    residues.push(
        parser_continue.mul(finalize).mul(
            next[BASE_FRAME_START].sub(
                current[BASE_CONSTRUCTED].mul(current[BASE_NODE_START]).add(
                    F::ONE
                        .sub(current[BASE_CONSTRUCTED])
                        .mul(current[BASE_FRAME_START]),
                ),
            ),
        ),
    );
    residues.push(
        parser_continue.mul(finalize).mul(
            next[BASE_FRAME_END].sub(
                current[BASE_CONSTRUCTED]
                    .mul(current[BASE_CONTENT_END])
                    .add(
                        F::ONE
                            .sub(current[BASE_CONSTRUCTED])
                            .mul(current[BASE_FRAME_END]),
                    ),
            ),
        ),
    );
    residues.push(
        parser_continue.mul(finalize).mul(
            next[BASE_FRAME_IS_SET].sub(
                current[BASE_CONSTRUCTED].mul(set).add(
                    F::ONE
                        .sub(current[BASE_CONSTRUCTED])
                        .mul(current[BASE_FRAME_IS_SET]),
                ),
            ),
        ),
    );
    for column in [
        BASE_FRAME_HAS_CHILD,
        BASE_FRAME_PREVIOUS_ID,
        BASE_FRAME_PREVIOUS_START,
        BASE_FRAME_PREVIOUS_END,
    ] {
        residues.push(
            parser_continue
                .mul(finalize)
                .mul(next[column].sub(F::ONE.sub(current[BASE_CONSTRUCTED]).mul(current[column]))),
        );
    }
    residues.push(
        parser_continue.mul(finalize).mul(
            pack_bits_v1(&next[BASE_DEPTH_BITS..BASE_DEPTH_BITS + 5])
                .sub(depth.add(current[BASE_CONSTRUCTED])),
        ),
    );
    push_carry_residues_v1(
        residues,
        parser_continue.mul(finalize),
        current,
        next,
        &[BASE_DOCUMENT, BASE_DOCUMENT_LEN],
    );
    let enters_child = current_aux[AUX_ENTERS_CHILD];
    for column in [
        BASE_CONTENT_START,
        BASE_CONTENT_END,
        BASE_TAG_CLASS,
        BASE_CONSTRUCTED,
        BASE_TAG_ACCUMULATOR,
        BASE_LENGTH_ACCUMULATOR,
    ] {
        residues.push(
            parser_continue
                .mul(finalize)
                .mul(next[column].sub(F::ONE.sub(enters_child).mul(current[column]))),
        );
    }
    residues.push(
        parser_continue.mul(finalize).mul(
            next[BASE_NODE_START].sub(
                enters_child
                    .mul(current[BASE_CONTENT_START])
                    .add(F::ONE.sub(enters_child).mul(current[BASE_NODE_START])),
            ),
        ),
    );
    // Primitive canonicality.
    let kind_bits = &current[BASE_PAYLOAD..BASE_PAYLOAD + 3];
    residues.push(
        primitive.mul(
            kind_selectors[..=6]
                .iter()
                .copied()
                .fold(F::ZERO, F::add)
                .sub(F::ONE),
        ),
    );
    let unused_bits = &current[BASE_PAYLOAD + 3..BASE_PAYLOAD + 6];
    residues.push(primitive.mul(current[BASE_UNUSED_BITS].sub(pack_bits_v1(unused_bits))));
    let last_primitive = current[BASE_CHECK_IS_ZERO];
    residues.push(
        primitive
            .mul(kind_selectors[1])
            .mul(current[BASE_BYTE_VALUE])
            .mul(current[BASE_BYTE_VALUE].sub(F(0xff))),
    );
    let byte_zero = current[BASE_PAYLOAD + 6];
    let byte_inverse = current[BASE_PAYLOAD + 7];
    let byte_ff = current[BASE_PAYLOAD + 8];
    let byte_ff_inverse = current[BASE_PAYLOAD + 9];
    residues.push(primitive.mul(current[BASE_BYTE_VALUE]).mul(byte_zero));
    residues.push(
        primitive.mul(
            current[BASE_BYTE_VALUE]
                .mul(byte_inverse)
                .sub(F::ONE.sub(byte_zero)),
        ),
    );
    residues.push(primitive.mul(byte_zero).mul(byte_inverse));
    let byte_ff_delta = current[BASE_BYTE_VALUE].sub(F(0xff));
    residues.push(primitive.mul(byte_ff_delta).mul(byte_ff));
    residues.push(primitive.mul(byte_ff_delta.mul(byte_ff_inverse).sub(F::ONE.sub(byte_ff))));
    residues.push(primitive.mul(byte_ff).mul(byte_ff_inverse));
    residues.push(
        current_aux[AUX_SIGNED_FIRST_GUARD]
            .mul(byte_zero)
            .mul(F::ONE.sub(next[BASE_BYTE_BITS + 7])),
    );
    residues.push(
        current_aux[AUX_SIGNED_FIRST_GUARD]
            .mul(byte_ff)
            .mul(next[BASE_BYTE_BITS + 7]),
    );
    residues.push(
        primitive
            .mul(kind_selectors[5])
            .mul(current[BASE_OID_START])
            .mul(current_aux[AUX_BYTE_IS_128]),
    );
    residues.push(
        primitive
            .mul(kind_selectors[5])
            .mul(last_primitive)
            .mul(byte_bits[7]),
    );
    for bit in 3..8 {
        residues.push(
            primitive
                .mul(kind_selectors[3])
                .mul(current[BASE_PRIMITIVE_FIRST])
                .mul(byte_bits[bit]),
        );
    }
    residues.push(
        current_aux[AUX_BIT_STRING_FIRST_GUARD]
            .mul(last_primitive)
            .mul(current[BASE_UNUSED_BITS]),
    );
    for bit in 0..8 {
        let bit_must_be_zero = unused_selectors[bit + 1..]
            .iter()
            .copied()
            .fold(F::ZERO, F::add);
        residues.push(
            current_aux[AUX_BIT_STRING_LAST_CONTINUATION_GUARD]
                .mul(bit_must_be_zero)
                .mul(byte_bits[bit]),
        );
    }
    residues.push(
        parser_continue
            .mul(primitive)
            .mul(next[BASE_OFFSET].sub(current[BASE_OFFSET].add(F::ONE))),
    );
    residues.push(
        parser_continue.mul(primitive).mul(
            next_phase_value.sub(
                last_primitive.mul(F(PHASE_BOUNDARY as u64)).add(
                    F::ONE
                        .sub(last_primitive)
                        .mul(F(PHASE_PRIMITIVE_CONTENT as u64)),
                ),
            ),
        ),
    );
    residues.push(
        parser_continue
            .mul(primitive)
            .mul(next[BASE_PRIMITIVE_FIRST]),
    );
    residues.push(
        parser_continue
            .mul(primitive)
            .mul(next[BASE_OID_START].sub(current_aux[AUX_NEXT_OID_START_EXPECTED])),
    );
    residues.push(
        parser_continue
            .mul(primitive)
            .mul(F::ONE.sub(last_primitive))
            .mul(pack_bits_v1(&next[BASE_PAYLOAD..BASE_PAYLOAD + 3]).sub(pack_bits_v1(kind_bits))),
    );
    residues.push(
        parser_continue
            .mul(primitive)
            .mul(F::ONE.sub(last_primitive))
            .mul(next[BASE_UNUSED_BITS].sub(current[BASE_UNUSED_BITS])),
    );
    push_carry_residues_v1(
        residues,
        parser_continue.mul(primitive),
        current,
        next,
        &[
            BASE_DOCUMENT,
            BASE_DOCUMENT_LEN,
            BASE_TAG_CLASS,
            BASE_CONSTRUCTED,
            BASE_TAG_ACCUMULATOR,
            BASE_LENGTH_ACCUMULATOR,
            BASE_CONTENT_START,
            BASE_CONTENT_END,
            BASE_NODE_START,
            BASE_NODE_COUNT,
            BASE_FRAME_ID,
            BASE_FRAME_START,
            BASE_FRAME_END,
            BASE_FRAME_IS_SET,
            BASE_FRAME_HAS_CHILD,
            BASE_FRAME_PREVIOUS_ID,
            BASE_FRAME_PREVIOUS_START,
            BASE_FRAME_PREVIOUS_END,
        ],
    );
    // Boundary terminal and document sequencing. Stack restoration is also
    // bound by the challenge-dependent push/pop products.
    let expected_root_completion = F::ONE
        .sub(current[BASE_CONSTRUCTED])
        .mul(depth_zero)
        .add(current[BASE_CONSTRUCTED].mul(depth_one));
    residues.push(current_aux[AUX_ROOT_COMPLETION].sub(expected_root_completion));
    let root_completion = current_aux[AUX_ROOT_COMPLETION];
    residues
        .push(current_aux[AUX_BOUNDARY_NOT_ROOT].sub(boundary.mul(F::ONE.sub(root_completion))));
    residues.push(
        current_aux[AUX_BOUNDARY_COMPLETES_PARENT]
            .sub(current_aux[AUX_BOUNDARY_NOT_ROOT].mul(current[BASE_CHECK_IS_ZERO])),
    );
    residues.push(last_parser.mul(root_completion.sub(F::ONE)));
    residues.push(last_parser.mul(current[BASE_CHECK_IS_ZERO].sub(F::ONE)));
    residues.push(last_parser.mul(current[BASE_DOCUMENT].sub(current[BASE_FINAL_DOCUMENT])));
    residues.push(last_parser.mul(current[BASE_OFFSET].sub(current[BASE_DOCUMENT_LEN])));
    let boundary_continue = boundary.mul(parser_continue);
    let not_root_completion = F::ONE.sub(root_completion);
    residues.push(
        boundary_continue
            .mul(root_completion)
            .mul(next[BASE_DOCUMENT].sub(current[BASE_DOCUMENT].add(F::ONE))),
    );
    residues.push(
        boundary_continue
            .mul(root_completion)
            .mul(next[BASE_OFFSET]),
    );
    residues.push(
        boundary_continue
            .mul(root_completion)
            .mul(next[BASE_NODE_COUNT]),
    );
    residues.push(
        boundary_continue
            .mul(root_completion)
            .mul(next[BASE_DOCUMENT_FIRST].sub(F::ONE)),
    );
    residues.push(
        boundary_continue
            .mul(root_completion)
            .mul(next_phase_value.sub(F(PHASE_IDENTIFIER_FIRST as u64))),
    );
    residues.push(
        boundary_continue
            .mul(not_root_completion)
            .mul(next[BASE_DOCUMENT].sub(current[BASE_DOCUMENT])),
    );
    residues.push(
        boundary_continue
            .mul(not_root_completion)
            .mul(next[BASE_DOCUMENT_LEN].sub(current[BASE_DOCUMENT_LEN])),
    );
    residues.push(
        boundary_continue
            .mul(not_root_completion)
            .mul(next[BASE_OFFSET].sub(current[BASE_OFFSET])),
    );
    residues.push(
        boundary_continue
            .mul(not_root_completion)
            .mul(next[BASE_NODE_COUNT].sub(current[BASE_NODE_COUNT])),
    );
    residues.push(
        boundary_continue
            .mul(not_root_completion)
            .mul(next_phase_value.sub(current[BASE_CHECK_IS_ZERO].mul(F(PHASE_BOUNDARY as u64)))),
    );
    residues.push(
        boundary_continue
            .mul(not_root_completion)
            .mul(next[BASE_DOCUMENT_FIRST]),
    );
    residues.push(
        boundary_continue.mul(
            pack_bits_v1(&next[BASE_DEPTH_BITS..BASE_DEPTH_BITS + 5])
                .sub(depth.sub(current[BASE_CONSTRUCTED])),
        ),
    );
    let parent_id = current[BASE_CONSTRUCTED].mul(current[BASE_PAYLOAD]).add(
        F::ONE
            .sub(current[BASE_CONSTRUCTED])
            .mul(current[BASE_FRAME_ID]),
    );
    let parent_start = current[BASE_CONSTRUCTED]
        .mul(current[BASE_PAYLOAD + 1])
        .add(
            F::ONE
                .sub(current[BASE_CONSTRUCTED])
                .mul(current[BASE_FRAME_START]),
        );
    let parent_end = current[BASE_CONSTRUCTED]
        .mul(current[BASE_PAYLOAD + 2])
        .add(
            F::ONE
                .sub(current[BASE_CONSTRUCTED])
                .mul(current[BASE_FRAME_END]),
        );
    let parent_is_set = current[BASE_CONSTRUCTED]
        .mul(current[BASE_PAYLOAD + 3])
        .add(
            F::ONE
                .sub(current[BASE_CONSTRUCTED])
                .mul(current[BASE_FRAME_IS_SET]),
        );
    let completed_id = current[BASE_CONSTRUCTED].mul(current[BASE_FRAME_ID]).add(
        F::ONE
            .sub(current[BASE_CONSTRUCTED])
            .mul(current[BASE_NODE_COUNT].sub(F::ONE)),
    );
    for (column, expected) in [
        (BASE_FRAME_ID, parent_id),
        (BASE_FRAME_START, parent_start),
        (BASE_FRAME_END, parent_end),
        (BASE_FRAME_IS_SET, parent_is_set),
        (BASE_FRAME_HAS_CHILD, F::ONE),
        (BASE_FRAME_PREVIOUS_ID, completed_id),
        (BASE_FRAME_PREVIOUS_START, current[BASE_NODE_START]),
        (BASE_FRAME_PREVIOUS_END, current[BASE_CONTENT_END]),
    ] {
        residues.push(
            parser_continue.mul(
                boundary
                    .mul(next[column])
                    .sub(current_aux[AUX_BOUNDARY_NOT_ROOT].mul(expected)),
            ),
        );
    }
    residues.push(
        parser_continue
            .mul(current_aux[AUX_BOUNDARY_COMPLETES_PARENT])
            .mul(next[BASE_NODE_START].sub(parent_start)),
    );
    residues.push(
        parser_continue
            .mul(current_aux[AUX_BOUNDARY_COMPLETES_PARENT])
            .mul(next[BASE_CONTENT_END].sub(parent_end)),
    );
    residues.push(
        parser_continue
            .mul(current_aux[AUX_BOUNDARY_COMPLETES_PARENT])
            .mul(next[BASE_CONSTRUCTED].sub(F::ONE)),
    );
    for column in [
        BASE_TAG_CLASS,
        BASE_TAG_ACCUMULATOR,
        BASE_LENGTH_ACCUMULATOR,
        BASE_CONTENT_START,
    ] {
        residues.push(
            parser_continue
                .mul(current_aux[AUX_BOUNDARY_COMPLETES_PARENT])
                .mul(next[column].sub(current[column])),
        );
    }
    for value in &current[BASE_PAYLOAD..BASE_PAYLOAD + 8] {
        residues.push(
            boundary
                .mul(F::ONE.sub(current[BASE_CONSTRUCTED]))
                .mul(*value),
        );
    }
    residues.push(boundary.mul(current[BASE_CONSTRUCTED]).mul(depth_zero));
    // SET comparator rows.
    let cmp_first = current[28];
    let cmp_last = current[29];
    let cmp_same_next = current[30];
    let cmp_same_inverse = current[31];
    let cmp_left_le = current[32];
    push_boolean_residues_v1(
        residues,
        comparator,
        &[
            current[12],
            current[13],
            current[14],
            current[15],
            current[16],
            current[27],
            cmp_first,
            cmp_last,
            cmp_same_next,
            cmp_left_le,
        ],
    );
    push_boolean_residues_v1(residues, comparator, &current[19..27]);
    residues.push(comparator.mul(current[18].sub(pack_bits_v1(&current[19..27]))));
    push_boolean_residues_v1(residues, comparator, &current[34..49]);
    residues.push(comparator.mul(current[33].sub(pack_bits_v1(&current[34..49]))));
    for value in &current[49..BASE_ROW_ACTIVE] {
        residues.push(comparator.mul(*value));
    }
    let byte_delta = current[11].sub(current[10]);
    residues.push(comparator.mul(byte_delta).mul(current[16]));
    residues.push(comparator.mul(byte_delta.mul(current[17]).sub(F::ONE.sub(current[16]))));
    residues.push(comparator.mul(current[16]).mul(current[17]));
    residues
        .push(comparator.mul(current[18].sub(byte_delta.sub(F::ONE).add(F(256).mul(current[27])))));
    residues.push(comparator.mul(current[14].sub(current[12].mul(current[16]))));
    residues.push(
        comparator.mul(
            current[15].sub(
                current[13].add(
                    current[12]
                        .mul(F::ONE.sub(current[16]))
                        .mul(F::ONE.sub(current[27])),
                ),
            ),
        ),
    );
    residues.push(comparator.mul(cmp_first).mul(current[9]));
    residues.push(comparator.mul(cmp_first).mul(current[12].sub(F::ONE)));
    residues.push(comparator.mul(cmp_first).mul(current[13]));
    let left_len = current[5].sub(current[4]);
    let right_len = current[7].sub(current[6]);
    residues.push(
        comparator.mul(
            cmp_left_le
                .mul(right_len.sub(left_len).sub(current[33]))
                .add(
                    F::ONE
                        .sub(cmp_left_le)
                        .mul(left_len.sub(right_len).sub(F::ONE).sub(current[33])),
                ),
        ),
    );
    let minimum_len = cmp_left_le
        .mul(left_len)
        .add(F::ONE.sub(cmp_left_le).mul(right_len));
    residues.push(
        comparator
            .mul(cmp_last)
            .mul(current[9].add(F::ONE).sub(minimum_len)),
    );
    residues.push(
        comparator
            .mul(cmp_last)
            .mul(current[15].add(current[14].mul(cmp_left_le)).sub(F::ONE)),
    );
    residues.push(comparator.mul(cmp_same_next).mul(cmp_same_inverse));
    residues.push(comparator.mul(cmp_same_next.add(cmp_last).sub(F::ONE)));
    residues.push(comparator.mul(cmp_same_inverse.sub(cmp_last)));
    let comparator_continue = comparator.mul(next_comparator);
    for column in 0..=8 {
        residues.push(
            comparator_continue
                .mul(cmp_same_next)
                .mul(next[column].sub(current[column])),
        );
    }
    residues.push(
        comparator_continue
            .mul(cmp_same_next)
            .mul(next[9].sub(current[9].add(F::ONE))),
    );
    residues.push(
        comparator_continue
            .mul(cmp_same_next)
            .mul(next[12].sub(current[14])),
    );
    residues.push(
        comparator_continue
            .mul(cmp_same_next)
            .mul(next[13].sub(current[15])),
    );
    residues.push(
        comparator_continue
            .mul(F::ONE.sub(cmp_same_next))
            .mul(next[8].sub(current[8].add(F::ONE))),
    );
    residues.push(
        comparator_continue
            .mul(F::ONE.sub(cmp_same_next))
            .mul(next[28].sub(F::ONE)),
    );
    residues.push(
        fixed[FIX_FIRST_COMPARATOR]
            .mul(comparator)
            .mul(cmp_first.sub(F::ONE)),
    );
    residues.push(last_comparator.mul(cmp_last.sub(F::ONE)));
    residues.push(
        comparator
            .mul(next_row_active)
            .mul(F::ONE.sub(next_fixed[FIX_COMPARATOR])),
    );
}
/// Evaluate every base/fixed strict-DER identity as one numeric polynomial
/// vector. The inventory is independent of witness values and row phases.
///
/// Challenge-dependent stack, event, and byte-lookup identities are appended
/// by the full evaluator below; keeping this base evaluator separate makes
/// pre-commitment mutation audits exhaustive.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn evaluate_zk_x509_der_stark_base_residues_v1(
    current: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1],
    next: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1],
    current_aux: &[F; ZK_X509_DER_STARK_AUX_WIDTH_V1],
    fixed: &[F; ZK_X509_DER_STARK_FIXED_WIDTH_V1],
    next_fixed: &[F; ZK_X509_DER_STARK_FIXED_WIDTH_V1],
) -> Vec<F> {
    let mut residues = Vec::with_capacity(ZK_X509_DER_STARK_CONSTRAINT_COUNT_V1);
    evaluate_zk_x509_der_stark_base_residues_into_v1(
        current,
        next,
        current_aux,
        fixed,
        next_fixed,
        &mut residues,
    );
    residues
}
/// Evaluate the complete strict-DER adapter, including every challenge-bound
/// permutation and logarithmic-derivative lookup.
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_zk_x509_der_stark_residues_into_v1(
    current: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1],
    next: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1],
    current_aux: &[F; ZK_X509_DER_STARK_AUX_WIDTH_V1],
    next_aux: &[F; ZK_X509_DER_STARK_AUX_WIDTH_V1],
    fixed: &[F; ZK_X509_DER_STARK_FIXED_WIDTH_V1],
    next_fixed: &[F; ZK_X509_DER_STARK_FIXED_WIDTH_V1],
    challenges: ZkX509DerStarkChallengesV1,
    _public: ZkX509DerStarkPublicTerminalsV1,
    terminal_claims: ZkX509DerStarkTerminalClaimsV1,
    residues: &mut Vec<F>,
) -> Result<(), ZkX509DerStarkErrorV1> {
    challenges.validate()?;
    if current
        .iter()
        .chain(next)
        .chain(current_aux)
        .chain(next_aux)
        .chain(fixed)
        .chain(next_fixed)
        .chain(terminal_claims.input_byte.iter())
        .chain(terminal_claims.node.iter())
        .any(|value| value.0 >= GOLDILOCKS_MODULUS_V1 || F::canonical(value.0).is_none())
    {
        return Err(ZkX509DerStarkErrorV1::Row);
    }
    evaluate_zk_x509_der_stark_base_residues_into_v1(
        current,
        next,
        current_aux,
        fixed,
        next_fixed,
        residues,
    );
    let phases: &[F; 8] = current_aux[AUX_PHASE_SELECTORS..AUX_PHASE_SELECTORS + 8]
        .try_into()
        .expect("eight phase selectors");
    let comparator = fixed[FIX_COMPARATOR].mul(current[BASE_ROW_ACTIVE]);
    let consuming = phases[PHASE_IDENTIFIER_FIRST]
        .add(phases[PHASE_IDENTIFIER_HIGH])
        .add(phases[PHASE_LENGTH_FIRST])
        .add(phases[PHASE_LENGTH_BODY])
        .add(phases[PHASE_PRIMITIVE_CONTENT]);
    let table_event = consuming;
    let stack_push_event = phases[PHASE_FINALIZE_HEADER].mul(current[BASE_CONSTRUCTED]);
    let stack_pop_event = phases[PHASE_BOUNDARY].mul(current[BASE_CONSTRUCTED]);
    let document_event = phases[PHASE_IDENTIFIER_FIRST].mul(current[BASE_DOCUMENT_FIRST]);
    let node_event = phases[PHASE_FINALIZE_HEADER];
    let pair_producer_event = current_aux[AUX_PAIR_PRODUCER_EVENT];
    let pair_consumer_event = comparator.mul(current[28]);
    let first_aggregate = fixed[FIX_FIRST_AGGREGATE];
    let last_aggregate = fixed[FIX_LAST_AGGREGATE];
    let aggregate_continue = F::ONE.sub(last_aggregate);
    for lane in 0..ZK_X509_DER_STARK_BUS_LANES_V1 {
        let tuple_challenge = challenges.tuple[lane];
        let stack_push_factor = compress_tuple_v1(&stack_push_tuple_v1(current), tuple_challenge);
        let stack_pop_factor = compress_tuple_v1(&stack_pop_tuple_v1(current), tuple_challenge);
        let document_factor = compress_tuple_v1(
            &document_tuple_v1(current[BASE_DOCUMENT], current[BASE_DOCUMENT_LEN]),
            tuple_challenge,
        );
        let node_factor = compress_tuple_v1(&node_tuple_v1(current), tuple_challenge);
        let pair_producer_factor =
            compress_tuple_v1(&pair_producer_tuple_v1(current), tuple_challenge);
        let pair_consumer_factor =
            compress_tuple_v1(&pair_consumer_tuple_v1(current), tuple_challenge);
        let input_byte_factor = compress_tuple_v1(
            &input_byte_tuple_v1(
                current[BASE_DOCUMENT],
                current[BASE_OFFSET],
                current[BASE_BYTE_VALUE],
            ),
            tuple_challenge,
        );
        for (before, after, next_before, gate, factor) in [
            (
                AUX_STACK_PUSH_BEFORE + lane,
                AUX_STACK_PUSH_AFTER + lane,
                AUX_STACK_PUSH_BEFORE + lane,
                stack_push_event,
                stack_push_factor,
            ),
            (
                AUX_STACK_POP_BEFORE + lane,
                AUX_STACK_POP_AFTER + lane,
                AUX_STACK_POP_BEFORE + lane,
                stack_pop_event,
                stack_pop_factor,
            ),
            (
                AUX_DOCUMENT_BEFORE + lane,
                AUX_DOCUMENT_AFTER + lane,
                AUX_DOCUMENT_BEFORE + lane,
                document_event,
                document_factor,
            ),
            (
                AUX_NODE_BEFORE + lane,
                AUX_NODE_AFTER + lane,
                AUX_NODE_BEFORE + lane,
                node_event,
                node_factor,
            ),
            (
                AUX_PAIR_PRODUCER_BEFORE + lane,
                AUX_PAIR_PRODUCER_AFTER + lane,
                AUX_PAIR_PRODUCER_BEFORE + lane,
                pair_producer_event,
                pair_producer_factor,
            ),
            (
                AUX_PAIR_CONSUMER_BEFORE + lane,
                AUX_PAIR_CONSUMER_AFTER + lane,
                AUX_PAIR_CONSUMER_BEFORE + lane,
                pair_consumer_event,
                pair_consumer_factor,
            ),
            (
                AUX_INPUT_BYTE_BEFORE + lane,
                AUX_INPUT_BYTE_AFTER + lane,
                AUX_INPUT_BYTE_BEFORE + lane,
                table_event,
                input_byte_factor,
            ),
        ] {
            residues.push(
                current_aux[after]
                    .sub(current_aux[before].mul(F::ONE.add(gate.mul(factor.sub(F::ONE))))),
            );
            residues.push(first_aggregate.mul(current_aux[before].sub(F::ONE)));
            residues.push(aggregate_continue.mul(next_aux[next_before].sub(current_aux[after])));
        }
        let table_tuple = byte_tuple_v1(
            current[BASE_DOCUMENT],
            current[BASE_OFFSET],
            current[BASE_BYTE_VALUE],
        );
        let left_tuple = byte_tuple_v1(current[0], current[4].add(current[9]), current[10]);
        let right_tuple = byte_tuple_v1(current[0], current[6].add(current[9]), current[11]);
        let table_denominator = byte_denominator_v1(table_tuple, lane, challenges);
        let left_denominator = byte_denominator_v1(left_tuple, lane, challenges);
        let right_denominator = byte_denominator_v1(right_tuple, lane, challenges);
        let table_inverse = current_aux[AUX_BYTE_TABLE_INVERSE + lane];
        let left_inverse = current_aux[AUX_BYTE_LEFT_QUERY_INVERSE + lane];
        let right_inverse = current_aux[AUX_BYTE_RIGHT_QUERY_INVERSE + lane];
        let table_zero = current_aux[AUX_BYTE_TABLE_ZERO + lane];
        let left_zero = current_aux[AUX_BYTE_LEFT_QUERY_ZERO + lane];
        let right_zero = current_aux[AUX_BYTE_RIGHT_QUERY_ZERO + lane];
        push_gated_zero_safe_inverse_residues_v1(
            residues,
            table_event,
            table_denominator,
            table_zero,
            table_inverse,
        );
        push_gated_zero_safe_inverse_residues_v1(
            residues,
            comparator,
            left_denominator,
            left_zero,
            left_inverse,
        );
        push_gated_zero_safe_inverse_residues_v1(
            residues,
            comparator,
            right_denominator,
            right_zero,
            right_inverse,
        );
        residues.push(
            current_aux[AUX_BYTE_TABLE_SUM_AFTER + lane].sub(
                current_aux[AUX_BYTE_TABLE_SUM_BEFORE + lane].add(
                    table_event
                        .mul(current[BASE_BYTE_LOOKUP_MULTIPLICITY])
                        .mul(table_inverse),
                ),
            ),
        );
        residues.push(
            current_aux[AUX_BYTE_QUERY_SUM_AFTER + lane].sub(
                current_aux[AUX_BYTE_QUERY_SUM_BEFORE + lane]
                    .add(comparator.mul(left_inverse.add(right_inverse))),
            ),
        );
        residues.push(
            current_aux[AUX_BYTE_TABLE_ZERO_COUNT_AFTER + lane].sub(
                current_aux[AUX_BYTE_TABLE_ZERO_COUNT_BEFORE + lane].add(
                    table_event
                        .mul(current[BASE_BYTE_LOOKUP_MULTIPLICITY])
                        .mul(table_zero),
                ),
            ),
        );
        residues.push(
            current_aux[AUX_BYTE_QUERY_ZERO_COUNT_AFTER + lane].sub(
                current_aux[AUX_BYTE_QUERY_ZERO_COUNT_BEFORE + lane]
                    .add(comparator.mul(left_zero.add(right_zero))),
            ),
        );
        residues.push(first_aggregate.mul(current_aux[AUX_BYTE_TABLE_SUM_BEFORE + lane]));
        residues.push(first_aggregate.mul(current_aux[AUX_BYTE_QUERY_SUM_BEFORE + lane]));
        residues.push(first_aggregate.mul(current_aux[AUX_BYTE_TABLE_ZERO_COUNT_BEFORE + lane]));
        residues.push(first_aggregate.mul(current_aux[AUX_BYTE_QUERY_ZERO_COUNT_BEFORE + lane]));
        residues.push(
            aggregate_continue.mul(
                next_aux[AUX_BYTE_TABLE_SUM_BEFORE + lane]
                    .sub(current_aux[AUX_BYTE_TABLE_SUM_AFTER + lane]),
            ),
        );
        residues.push(
            aggregate_continue.mul(
                next_aux[AUX_BYTE_QUERY_SUM_BEFORE + lane]
                    .sub(current_aux[AUX_BYTE_QUERY_SUM_AFTER + lane]),
            ),
        );
        residues.push(
            aggregate_continue.mul(
                next_aux[AUX_BYTE_TABLE_ZERO_COUNT_BEFORE + lane]
                    .sub(current_aux[AUX_BYTE_TABLE_ZERO_COUNT_AFTER + lane]),
            ),
        );
        residues.push(
            aggregate_continue.mul(
                next_aux[AUX_BYTE_QUERY_ZERO_COUNT_BEFORE + lane]
                    .sub(current_aux[AUX_BYTE_QUERY_ZERO_COUNT_AFTER + lane]),
            ),
        );
        residues.push(last_aggregate.mul(
            current_aux[AUX_STACK_PUSH_AFTER + lane].sub(current_aux[AUX_STACK_POP_AFTER + lane]),
        ));
        residues.push(
            last_aggregate.mul(
                current_aux[AUX_PAIR_PRODUCER_AFTER + lane]
                    .sub(current_aux[AUX_PAIR_CONSUMER_AFTER + lane]),
            ),
        );
        residues.push(
            last_aggregate.mul(
                current_aux[AUX_BYTE_TABLE_SUM_AFTER + lane]
                    .sub(current_aux[AUX_BYTE_QUERY_SUM_AFTER + lane]),
            ),
        );
        residues.push(
            last_aggregate.mul(
                current_aux[AUX_BYTE_TABLE_ZERO_COUNT_AFTER + lane]
                    .sub(current_aux[AUX_BYTE_QUERY_ZERO_COUNT_AFTER + lane]),
            ),
        );
    }
    residues.extend(evaluate_zk_x509_der_stark_terminal_claim_residues_v1(
        last_aggregate,
        current_aux,
        terminal_claims,
    ));
    if residues.len() != ZK_X509_DER_STARK_CONSTRAINT_COUNT_V1 {
        return Err(ZkX509DerStarkErrorV1::Transition);
    }
    Ok(())
}
/// Allocate and evaluate the complete strict-DER constraint vector.
///
/// Streaming composition builders should use
/// [`evaluate_zk_x509_der_stark_residues_into_v1`] to reuse one bounded
/// scratch vector across every common-domain row.
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_zk_x509_der_stark_residues_v1(
    current: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1],
    next: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1],
    current_aux: &[F; ZK_X509_DER_STARK_AUX_WIDTH_V1],
    next_aux: &[F; ZK_X509_DER_STARK_AUX_WIDTH_V1],
    fixed: &[F; ZK_X509_DER_STARK_FIXED_WIDTH_V1],
    next_fixed: &[F; ZK_X509_DER_STARK_FIXED_WIDTH_V1],
    challenges: ZkX509DerStarkChallengesV1,
    public: ZkX509DerStarkPublicTerminalsV1,
    terminal_claims: ZkX509DerStarkTerminalClaimsV1,
) -> Result<Vec<F>, ZkX509DerStarkErrorV1> {
    let mut residues = Vec::with_capacity(ZK_X509_DER_STARK_CONSTRAINT_COUNT_V1);
    evaluate_zk_x509_der_stark_residues_into_v1(
        current,
        next,
        current_aux,
        next_aux,
        fixed,
        next_fixed,
        challenges,
        public,
        terminal_claims,
        &mut residues,
    )?;
    Ok(residues)
}
#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest as _, Sha256};
    fn challenges() -> ZkX509DerStarkChallengesV1 {
        ZkX509DerStarkChallengesV1 {
            tuple: core::array::from_fn(|lane| {
                core::array::from_fn(|column| {
                    F(u64::try_from(1_000 + lane * 100 + column).expect("challenge"))
                })
            }),
            byte_lookup: [F(9_001), F(9_002), F(9_003), F(9_004)],
        }
    }
    fn private_shape() -> ZkX509DerStarkPrivateShapeV1 {
        ZkX509DerStarkPrivateShapeV1 {
            document_lengths: vec![2, 3],
            parser_rows: 9,
            comparator_rows: 4,
        }
    }
    fn try_low_degree_aux(
        base: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1],
        fixed: &[F; ZK_X509_DER_STARK_FIXED_WIDTH_V1],
    ) -> Result<[F; ZK_X509_DER_STARK_AUX_WIDTH_V1], ZkX509DerStarkErrorV1> {
        let mut aux = [F::ZERO; ZK_X509_DER_STARK_AUX_WIDTH_V1];
        populate_low_degree_auxiliaries_v1(base, fixed, &mut aux)?;
        Ok(aux)
    }
    fn low_degree_aux(
        base: &[F; ZK_X509_DER_STARK_BASE_WIDTH_V1],
        fixed: &[F; ZK_X509_DER_STARK_FIXED_WIDTH_V1],
    ) -> [F; ZK_X509_DER_STARK_AUX_WIDTH_V1] {
        try_low_degree_aux(base, fixed).expect("low-degree auxiliaries")
    }
    fn transcript_with_base_root(root: [u8; 32]) -> TransparentTranscriptV1 {
        let mut transcript = TransparentTranscriptV1::new(
            b"zk-x509-der-challenge-test-suite-v1",
            &[0x41; 32],
            &[0x83; 32],
        )
        .expect("transcript");
        transcript
            .absorb(b"zk-x509-der-base-root-test-v1", &[&root])
            .expect("base root");
        transcript
    }
    #[test]
    fn transcript_challenge_schedule_is_lane_major_base_bound_and_pinned() {
        let mut transcript = transcript_with_base_root([0x25; 32]);
        let derived =
            derive_zk_x509_der_stark_challenges_v1(&mut transcript).expect("DER challenges");
        derived.validate().expect("valid DER challenges");
        let mut replay = transcript_with_base_root([0x25; 32]);
        assert_eq!(
            derived,
            derive_zk_x509_der_stark_challenges_v1(&mut replay).expect("replayed challenges")
        );
        let mut encoding = Vec::with_capacity(
            ZK_X509_DER_STARK_BUS_LANES_V1 * (DER_TUPLE_CHALLENGE_LABELS_V1.len() + 1) * 8,
        );
        for lane in 0..ZK_X509_DER_STARK_BUS_LANES_V1 {
            for coefficient in derived.tuple[lane] {
                encoding.extend_from_slice(&coefficient.0.to_be_bytes());
            }
            encoding.extend_from_slice(&derived.byte_lookup[lane].0.to_be_bytes());
        }
        let digest: [u8; 32] = Sha256::digest(&encoding).into();
        assert_eq!(
            digest,
            [
                0xea, 0x1b, 0xfd, 0xfe, 0xef, 0xe7, 0xc0, 0x8a, 0xd9, 0xd8, 0x63, 0x42, 0x7e, 0xff,
                0x74, 0xb7, 0x33, 0xf4, 0xc8, 0x0b, 0x64, 0x42, 0x1f, 0x8f, 0x9b, 0x40, 0x65, 0xa6,
                0x8e, 0xf2, 0x2f, 0x12,
            ]
        );
        let mut changed_root = transcript_with_base_root([0x26; 32]);
        let changed = derive_zk_x509_der_stark_challenges_v1(&mut changed_root)
            .expect("changed-root challenges");
        assert_ne!(derived, changed);
        for lane in 0..ZK_X509_DER_STARK_BUS_LANES_V1 {
            assert_ne!(derived.tuple[lane], changed.tuple[lane]);
            assert_ne!(derived.byte_lookup[lane], changed.byte_lookup[lane]);
        }
        // A tuple-first schedule is consensus-significant: sampling the lookup
        // shift before the twelve coefficients must not reproduce a lane.
        let mut wrong_order = transcript_with_base_root([0x25; 32]);
        let first_lookup = wrong_order
            .challenge_field(DER_BYTE_LOOKUP_CHALLENGE_LABEL_V1)
            .expect("wrong-order lookup");
        let wrong_tuple = core::array::from_fn(|slot| {
            wrong_order
                .challenge_field(DER_TUPLE_CHALLENGE_LABELS_V1[slot])
                .expect("wrong-order tuple")
        });
        assert_ne!(derived.byte_lookup[0], first_lookup);
        assert_ne!(derived.tuple[0], wrong_tuple);
    }
    #[test]
    fn fixed_schedule_is_exact_over_parser_comparator_and_padding_boundaries() {
        let schedule = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1)
            .expect("fixed schedule");
        assert_eq!(
            schedule.active_rows(),
            ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1
        );
        assert_eq!(schedule.aggregate_rows(), ZK_X509_DER_STARK_TRACE_SIZE_V1);
        assert_eq!(ZK_X509_DER_STARK_FIXED_PADDING_ROWS_V1, 196_608);
        assert_eq!(
            schedule.active_rows() + ZK_X509_DER_STARK_FIXED_PADDING_ROWS_V1,
            schedule.aggregate_rows()
        );
        let first = schedule.fixed_row(0).expect("first");
        assert_eq!(first[FIX_ACTIVE], F::ONE);
        assert_eq!(first[FIX_FIRST_ACTIVE], F::ONE);
        assert_eq!(first[FIX_PARSER], F::ONE);
        assert_eq!(first[FIX_FIRST_PARSER], F::ONE);
        let last_parser = schedule
            .fixed_row(ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 - 1)
            .expect("last parser capacity row");
        assert_eq!(last_parser[FIX_LAST_PARSER], F::ONE);
        assert_eq!(last_parser[FIX_COMPARATOR], F::ZERO);
        let first_comparator = schedule
            .fixed_row(ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1)
            .expect("first comparator");
        assert_eq!(first_comparator[FIX_COMPARATOR], F::ONE);
        assert_eq!(first_comparator[FIX_FIRST_COMPARATOR], F::ONE);
        let last_active = schedule
            .fixed_row(ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1 - 1)
            .expect("last fixed-capacity row");
        assert_eq!(last_active[FIX_LAST_ACTIVE], F::ONE);
        assert_eq!(last_active[FIX_LAST_COMPARATOR], F::ONE);
        let first_padding = schedule
            .fixed_row(ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1)
            .expect("first padding");
        assert_eq!(first_padding[FIX_PADDING], F::ONE);
        assert_eq!(first_padding[FIX_ACTIVE], F::ZERO);
        let final_row = schedule
            .fixed_row(ZK_X509_DER_STARK_TRACE_SIZE_V1 - 1)
            .expect("final aggregate row");
        assert_eq!(final_row[FIX_PADDING], F::ONE);
        assert_eq!(final_row[FIX_LAST_AGGREGATE], F::ONE);
    }
    #[test]
    fn private_geometry_cannot_change_public_transcript_or_fixed_schedule() {
        let first = private_shape();
        let second = ZkX509DerStarkPrivateShapeV1 {
            document_lengths: vec![4],
            parser_rows: 6,
            comparator_rows: 1,
        };
        first.validate().expect("first private shape");
        second.validate().expect("second private shape");
        assert_ne!(first, second);
        let public = ZkX509DerStarkShapeV1;
        assert_eq!(
            public.transcript_bytes(),
            b"zk-x509-der-stark-fixed-registration-v1"
        );
        let first_schedule =
            compile_zk_x509_der_stark_fixed_schedule_v1(public).expect("first schedule");
        let second_schedule =
            compile_zk_x509_der_stark_fixed_schedule_v1(public).expect("second schedule");
        for index in [
            0,
            ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 - 1,
            ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1,
            ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1 - 1,
            ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1,
            ZK_X509_DER_STARK_TRACE_SIZE_V1 - 1,
        ] {
            assert_eq!(
                first_schedule.fixed_row(index).expect("first fixed row"),
                second_schedule.fixed_row(index).expect("second fixed row")
            );
        }
    }
    #[test]
    fn private_shape_rejects_empty_zero_oversized_and_overfull_profiles() {
        let mut mutations = Vec::new();
        let mut changed = private_shape();
        changed.document_lengths.clear();
        mutations.push(changed);
        changed = private_shape();
        changed.document_lengths[0] = 0;
        mutations.push(changed);
        changed = private_shape();
        changed.document_lengths[0] =
            u16::try_from(ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1 + 1).expect("limit fits");
        mutations.push(changed);
        changed = private_shape();
        changed.document_lengths = vec![1; ZK_X509_DER_STARK_MAX_DOCUMENTS_V1 + 1];
        changed.parser_rows = changed.document_lengths.len() * 3;
        mutations.push(changed);
        changed = private_shape();
        changed.document_lengths =
            vec![
                u16::try_from(ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1)
                    .expect("proof cap fits");
                9
            ];
        changed.parser_rows =
            changed.document_lengths.len() * (ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1 + 2);
        changed.comparator_rows = 0;
        mutations.push(changed);
        changed = private_shape();
        changed.parser_rows = 1;
        mutations.push(changed);
        changed = private_shape();
        changed.comparator_rows = ZK_X509_DER_STARK_TRACE_SIZE_V1;
        mutations.push(changed);
        for (index, mutation) in mutations.into_iter().enumerate() {
            assert!(
                mutation.validate().is_err(),
                "shape mutation {index} must reject"
            );
        }
    }
    #[test]
    fn numeric_layout_uses_two_base_and_four_honest_auxiliary_chunks() {
        assert_eq!(ZK_X509_DER_STARK_BASE_WIDTH_V1, 76);
        assert_eq!(
            ZK_X509_DER_STARK_BASE_WIDTH_V1.div_ceil(64),
            2,
            "private activity and document-count metadata use a second physical chunk"
        );
        assert_eq!(
            ZK_X509_DER_STARK_AUX_WIDTH_V1.div_ceil(64),
            4,
            "zero-safe lookup witnesses and degree-reduction intermediates must remain explicit"
        );
        assert_eq!(ZK_X509_DER_STARK_TRACE_LOG2_V1, 19);
        assert_eq!(ZK_X509_DER_STARK_TRACE_SIZE_V1, 1 << 19);
        assert_eq!(ZK_X509_DER_STARK_MAX_TOTAL_DOCUMENT_BYTES_V1, 32_768);
        assert_eq!(ZK_X509_DER_STARK_MAX_COMPARATOR_ROWS_V1, 262_144);
        assert_eq!(ZK_X509_DER_MAX_NESTING_DEPTH_V1, 16);
        assert_eq!(ZK_X509_DER_MAX_VALUES_V1, 2_048);
    }
    include!("der_stark/tuple_compression_test.rs");
    #[test]
    fn canonical_documents_compile_to_exact_streaming_and_set_rows() {
        let sequence = [0x30, 0x03, 0x02, 0x01, 0x01];
        let ordered_set = [0x31, 0x04, 0x05, 0x00, 0x05, 0x00];
        let base =
            build_zk_x509_der_stark_base_v1(&[&sequence, &ordered_set]).expect("numeric DER base");
        assert_eq!(base.private_shape.document_lengths, vec![5, 6]);
        assert!(base.private_shape.parser_rows > 11);
        assert_eq!(base.private_shape.comparator_rows, 2);
        assert_eq!(
            base.rows.len(),
            base.private_shape.active_rows().expect("active")
        );
        let first = &base.rows[0];
        assert_eq!(first[BASE_DOCUMENT], F::ZERO);
        assert_eq!(first[BASE_DOCUMENT_LEN], F(5));
        assert_eq!(first[BASE_OFFSET], F::ZERO);
        assert_eq!(first[BASE_BYTE_VALUE], F(0x30));
        assert_eq!(first[BASE_DOCUMENT_FIRST], F::ONE);
        assert_eq!(
            pack_bits_v1(&first[BASE_PHASE_BITS..BASE_PHASE_BITS + 3]),
            F(PHASE_IDENTIFIER_FIRST as u64)
        );
        let comparator = &base.rows[base.private_shape.parser_rows];
        assert_eq!(comparator[0], F::ONE);
        assert_eq!(comparator[28], F::ONE);
        assert_eq!(comparator[29], F::ZERO);
        assert_eq!(comparator[9], F::ZERO);
        let comparator_terminal = base.rows.last().expect("comparator terminal");
        assert_eq!(comparator_terminal[29], F::ONE);
        assert_eq!(comparator_terminal[9], F::ONE);
    }
    #[test]
    fn malformed_and_noncanonical_documents_never_reach_numeric_compilation() {
        let adversarial: [&[u8]; 8] = [
            &[],
            &[0x30, 0x80, 0x00, 0x00],
            &[0x30, 0x81, 0x00],
            &[0x1f, 0x80, 0x01, 0x00],
            &[0x02, 0x02, 0x00, 0x01],
            &[0x03, 0x01, 0x08],
            &[0x06, 0x01, 0x80],
            &[0x31, 0x04, 0x05, 0x00, 0x01, 0x00],
        ];
        for (index, document) in adversarial.into_iter().enumerate() {
            assert!(
                build_zk_x509_der_stark_base_v1(&[document]).is_err(),
                "malformed DER family {index} must reject"
            );
        }
    }
    #[test]
    fn numeric_air_rejects_nonminimal_identifier_length_and_primitive_families() {
        let mutate_byte = |row: &mut [F; ZK_X509_DER_STARK_BASE_WIDTH_V1], byte: u8| {
            row[BASE_BYTE_VALUE] = F(u64::from(byte));
            for bit in 0..8 {
                row[BASE_BYTE_BITS + bit] = F(u64::from((byte >> bit) & 1));
            }
            if pack_bits_v1(&row[BASE_PHASE_BITS..BASE_PHASE_BITS + 3])
                == F(PHASE_PRIMITIVE_CONTENT as u64)
            {
                let value = F(u64::from(byte));
                let ff_delta = value.sub(F(0xff));
                row[BASE_PAYLOAD + 6] = F(u64::from(byte == 0));
                row[BASE_PAYLOAD + 7] = value.inv().unwrap_or(F::ZERO);
                row[BASE_PAYLOAD + 8] = F(u64::from(byte == 0xff));
                row[BASE_PAYLOAD + 9] = ff_delta.inv().unwrap_or(F::ZERO);
            }
        };
        let rejects_pair = |index: usize,
                            current: [F; ZK_X509_DER_STARK_BASE_WIDTH_V1],
                            next: [F; ZK_X509_DER_STARK_BASE_WIDTH_V1]| {
            let schedule = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1)
                .expect("schedule");
            let fixed = schedule.fixed_row(index).expect("fixed");
            let next_fixed = schedule.fixed_row(index + 1).expect("next fixed");
            evaluate_zk_x509_der_stark_base_residues_v1(
                &current,
                &next,
                &low_degree_aux(&current, &fixed),
                &fixed,
                &next_fixed,
            )
            .iter()
            .any(|residue| *residue != F::ZERO)
        };
        let high_tag = [0x9f, 0x1f, 0x00];
        let base = build_zk_x509_der_stark_base_v1(&[&high_tag]).expect("high tag");
        let high_row = base
            .rows
            .iter()
            .position(|row| {
                pack_bits_v1(&row[BASE_PHASE_BITS..BASE_PHASE_BITS + 3])
                    == F(PHASE_IDENTIFIER_HIGH as u64)
            })
            .expect("high row");
        let mut current = base.rows[high_row];
        let mut next = base.rows[high_row + 1];
        mutate_byte(&mut current, 0x1e);
        next[BASE_TAG_ACCUMULATOR] = F(30);
        assert!(rejects_pair(high_row, current, next));
        let mut long_length = vec![0x04, 0x81, 0x80];
        long_length.resize(3 + 128, 0x00);
        let base = build_zk_x509_der_stark_base_v1(&[&long_length]).expect("long length");
        let length_body = base
            .rows
            .iter()
            .position(|row| {
                pack_bits_v1(&row[BASE_PHASE_BITS..BASE_PHASE_BITS + 3])
                    == F(PHASE_LENGTH_BODY as u64)
            })
            .expect("length body");
        current = base.rows[length_body];
        mutate_byte(&mut current, 0x7f);
        assert!(rejects_pair(
            length_body,
            current,
            base.rows[length_body + 1]
        ));
        let integer = [0x02, 0x02, 0x00, 0x80];
        let base = build_zk_x509_der_stark_base_v1(&[&integer]).expect("integer");
        let first_content = base
            .rows
            .iter()
            .position(|row| row[BASE_PRIMITIVE_FIRST] == F::ONE)
            .expect("first content");
        current = base.rows[first_content];
        next = base.rows[first_content + 1];
        mutate_byte(&mut next, 0x7f);
        assert!(rejects_pair(first_content, current, next));
        let boolean = [0x01, 0x01, 0xff];
        let base = build_zk_x509_der_stark_base_v1(&[&boolean]).expect("boolean");
        let content = base
            .rows
            .iter()
            .position(|row| row[BASE_PRIMITIVE_FIRST] == F::ONE)
            .expect("content");
        current = base.rows[content];
        mutate_byte(&mut current, 0x01);
        assert!(rejects_pair(content, current, base.rows[content + 1]));
        let oid = [0x06, 0x01, 0x2a];
        let base = build_zk_x509_der_stark_base_v1(&[&oid]).expect("oid");
        let content = base
            .rows
            .iter()
            .position(|row| row[BASE_PRIMITIVE_FIRST] == F::ONE)
            .expect("content");
        current = base.rows[content];
        mutate_byte(&mut current, 0x80);
        assert!(rejects_pair(content, current, base.rows[content + 1]));
        let bit_string = [0x03, 0x02, 0x01, 0x80];
        let base = build_zk_x509_der_stark_base_v1(&[&bit_string]).expect("bit string");
        let last_content = base
            .rows
            .iter()
            .position(|row| {
                pack_bits_v1(&row[BASE_PHASE_BITS..BASE_PHASE_BITS + 3])
                    == F(PHASE_PRIMITIVE_CONTENT as u64)
                    && row[BASE_CHECK_IS_ZERO] == F::ONE
            })
            .expect("last content");
        current = base.rows[last_content];
        mutate_byte(&mut current, 0x81);
        assert!(rejects_pair(
            last_content,
            current,
            base.rows[last_content + 1]
        ));
    }
    #[test]
    fn every_canonical_streaming_and_set_row_satisfies_numeric_base_air() {
        let nested = [
            0x30, 0x0a, 0x31, 0x04, 0x05, 0x00, 0x05, 0x00, 0x02, 0x02, 0x00, 0x80,
        ];
        let base = build_zk_x509_der_stark_base_v1(&[&nested]).expect("numeric DER base");
        let schedule = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1)
            .expect("fixed schedule");
        for index in 0..base.rows.len() {
            let native_index =
                zk_x509_der_stark_compact_row_native_index_v1(&base.private_shape, index)
                    .expect("native row");
            let next = zk_x509_der_stark_aggregate_base_row_v1(&base, native_index + 1)
                .expect("next native row");
            let fixed = schedule.fixed_row(native_index).expect("fixed row");
            let next_fixed = schedule
                .fixed_row(native_index + 1)
                .expect("next fixed row");
            let aux = low_degree_aux(&base.rows[index], &fixed);
            let residues = evaluate_zk_x509_der_stark_base_residues_v1(
                &base.rows[index],
                &next,
                &aux,
                &fixed,
                &next_fixed,
            );
            assert!(
                residues.iter().all(|residue| *residue == F::ZERO),
                "row {index} phase {} -> {} has nonzero residues at {:?}; current {:?}; next {:?}",
                pack_bits_v1(&base.rows[index][BASE_PHASE_BITS..BASE_PHASE_BITS + 3]).0,
                pack_bits_v1(&next[BASE_PHASE_BITS..BASE_PHASE_BITS + 3]).0,
                residues
                    .iter()
                    .enumerate()
                    .filter(|(_, residue)| **residue != F::ZERO)
                    .take(16)
                    .collect::<Vec<_>>(),
                base.rows[index]
                    .iter()
                    .enumerate()
                    .filter(|(_, value)| **value != F::ZERO)
                    .collect::<Vec<_>>(),
                next.iter()
                    .enumerate()
                    .filter(|(_, value)| **value != F::ZERO)
                    .collect::<Vec<_>>()
            );
        }
        let first_inactive_parser_index = base.private_shape.parser_rows;
        let inactive = zk_x509_der_stark_aggregate_base_row_v1(&base, first_inactive_parser_index)
            .expect("inactive parser row");
        let next_inactive =
            zk_x509_der_stark_aggregate_base_row_v1(&base, first_inactive_parser_index + 1)
                .expect("next inactive parser row");
        let fixed = schedule
            .fixed_row(first_inactive_parser_index)
            .expect("first inactive parser fixed");
        let next_fixed = schedule
            .fixed_row(first_inactive_parser_index + 1)
            .expect("next inactive parser fixed");
        let aux = low_degree_aux(&inactive, &fixed);
        assert!(
            evaluate_zk_x509_der_stark_base_residues_v1(
                &inactive,
                &next_inactive,
                &aux,
                &fixed,
                &next_fixed,
            )
            .iter()
            .all(|residue| *residue == F::ZERO)
        );
    }
    #[test]
    fn adversarial_activity_restart_inactive_payload_and_document_count_fail() {
        let integer = [0x02, 0x01, 0x01];
        let base = build_zk_x509_der_stark_base_v1(&[&integer]).expect("numeric DER base");
        let schedule =
            compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1).expect("schedule");
        let mut dropped = base.clone();
        dropped.rows[1][BASE_ROW_ACTIVE] = F::ZERO;
        assert_eq!(
            validate_zk_x509_der_stark_base_trace_v1(&dropped),
            Err(ZkX509DerStarkErrorV1::Transition)
        );
        let inactive_index = base.private_shape.parser_rows;
        let inactive =
            zk_x509_der_stark_aggregate_base_row_v1(&base, inactive_index).expect("inactive row");
        let mut restarted = base.rows[1];
        restarted[BASE_FINAL_DOCUMENT] = inactive[BASE_FINAL_DOCUMENT];
        restarted[BASE_FINAL_DOCUMENT_BITS..BASE_FINAL_DOCUMENT_BITS + 5]
            .copy_from_slice(&inactive[BASE_FINAL_DOCUMENT_BITS..BASE_FINAL_DOCUMENT_BITS + 5]);
        restarted[BASE_FINAL_DOCUMENT_SLACK_BITS..BASE_FINAL_DOCUMENT_SLACK_BITS + 5]
            .copy_from_slice(
                &inactive[BASE_FINAL_DOCUMENT_SLACK_BITS..BASE_FINAL_DOCUMENT_SLACK_BITS + 5],
            );
        let fixed = schedule.fixed_row(inactive_index).expect("inactive fixed");
        let next_fixed = schedule
            .fixed_row(inactive_index + 1)
            .expect("restart fixed");
        let aux = low_degree_aux(&inactive, &fixed);
        assert!(
            evaluate_zk_x509_der_stark_base_residues_v1(
                &inactive,
                &restarted,
                &aux,
                &fixed,
                &next_fixed,
            )
            .iter()
            .any(|residue| *residue != F::ZERO),
            "an inactive parser prefix cannot restart"
        );
        let next_inactive = zk_x509_der_stark_aggregate_base_row_v1(&base, inactive_index + 1)
            .expect("next inactive row");
        let mut payload = inactive;
        payload[BASE_BYTE_VALUE] = F::ONE;
        let payload_aux = low_degree_aux(&payload, &fixed);
        assert!(
            evaluate_zk_x509_der_stark_base_residues_v1(
                &payload,
                &next_inactive,
                &payload_aux,
                &fixed,
                &next_fixed,
            )
            .iter()
            .any(|residue| *residue != F::ZERO),
            "inactive rows have no payload channel"
        );
        let mut wrong_count = inactive;
        wrong_count[BASE_FINAL_DOCUMENT_SLACK_BITS] =
            F::ONE.sub(wrong_count[BASE_FINAL_DOCUMENT_SLACK_BITS]);
        let wrong_count_aux = low_degree_aux(&wrong_count, &fixed);
        assert!(
            evaluate_zk_x509_der_stark_base_residues_v1(
                &wrong_count,
                &next_inactive,
                &wrong_count_aux,
                &fixed,
                &next_fixed,
            )
            .iter()
            .any(|residue| *residue != F::ZERO),
            "the privately committed document count is range- and carry-bound"
        );
    }
    #[test]
    fn every_active_base_cell_is_algebraically_observed() {
        let nested = [
            0x30, 0x0a, 0x31, 0x04, 0x05, 0x00, 0x05, 0x00, 0x02, 0x02, 0x00, 0x80,
        ];
        let base = build_zk_x509_der_stark_base_v1(&[&nested]).expect("numeric DER base");
        let schedule = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1)
            .expect("fixed schedule");
        for row_index in 0..base.rows.len() {
            for column in 0..ZK_X509_DER_STARK_BASE_WIDTH_V1 {
                // The lookup multiplicity is committed in the base trace and
                // observed only after byte challenges are sampled. Restored
                // parent-frame cells on constructed boundaries are likewise
                // bound by the post-commitment stack permutation.
                let phase =
                    pack_bits_v1(&base.rows[row_index][BASE_PHASE_BITS..BASE_PHASE_BITS + 3]);
                let stack_pop_cell = phase == F(PHASE_BOUNDARY as u64)
                    && base.rows[row_index][BASE_CONSTRUCTED] == F::ONE
                    && (BASE_PAYLOAD..BASE_PAYLOAD + 8).contains(&column);
                if column == BASE_BYTE_LOOKUP_MULTIPLICITY || stack_pop_cell {
                    continue;
                }
                let mut rows = base.rows.clone();
                rows[row_index][column] = rows[row_index][column].add(F(7));
                let changed_base = ZkX509DerStarkBaseV1 {
                    private_shape: base.private_shape.clone(),
                    rows,
                };
                let native_index = zk_x509_der_stark_compact_row_native_index_v1(
                    &changed_base.private_shape,
                    row_index,
                )
                .expect("native row");
                let next = zk_x509_der_stark_aggregate_base_row_v1(&changed_base, native_index + 1)
                    .expect("next native row");
                let current_fixed = schedule.fixed_row(native_index).expect("current fixed");
                let next_fixed = schedule.fixed_row(native_index + 1).expect("next fixed");
                let current_residues =
                    match try_low_degree_aux(&changed_base.rows[row_index], &current_fixed) {
                        Ok(current_aux) => evaluate_zk_x509_der_stark_base_residues_v1(
                            &changed_base.rows[row_index],
                            &next,
                            &current_aux,
                            &current_fixed,
                            &next_fixed,
                        ),
                        Err(_) => vec![F::ONE],
                    };
                let family_first =
                    row_index == 0 || row_index == changed_base.private_shape.parser_rows;
                let previous_residues = if family_first {
                    Vec::new()
                } else {
                    let previous_native_index = native_index - 1;
                    let previous_compact_index = row_index - 1;
                    let previous_fixed = schedule
                        .fixed_row(previous_native_index)
                        .expect("previous fixed");
                    let previous_aux =
                        low_degree_aux(&changed_base.rows[previous_compact_index], &previous_fixed);
                    evaluate_zk_x509_der_stark_base_residues_v1(
                        &changed_base.rows[previous_compact_index],
                        &changed_base.rows[row_index],
                        &previous_aux,
                        &previous_fixed,
                        &current_fixed,
                    )
                };
                assert!(
                    current_residues
                        .iter()
                        .chain(&previous_residues)
                        .any(|residue| *residue != F::ZERO),
                    "row {row_index} column {column} is not observed"
                );
            }
        }
    }
    #[test]
    fn complete_numeric_trace_closes_stack_set_document_and_byte_buses() {
        let nested = [
            0x30, 0x0a, 0x31, 0x04, 0x05, 0x00, 0x05, 0x00, 0x02, 0x02, 0x00, 0x80,
        ];
        let challenges = challenges();
        let base = build_zk_x509_der_stark_base_v1(&[&nested]).expect("base");
        let trace = build_zk_x509_der_stark_trace_v1(base, challenges).expect("complete DER trace");
        let schedule = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1)
            .expect("fixed schedule");
        let public =
            derive_zk_x509_der_stark_public_terminals_v1(&ZkX509DerStarkShapeV1, challenges)
                .expect("public terminal");
        let private_document_product = derive_zk_x509_der_stark_private_document_product_v1(
            &trace.base.private_shape,
            challenges,
        )
        .expect("private document product");
        let terminals = zk_x509_der_stark_terminals_v1(&trace).expect("terminals");
        let terminal_claims =
            zk_x509_der_stark_terminal_claims_v1(&trace).expect("terminal claims");
        assert_eq!(terminals.stack_push, terminals.stack_pop);
        assert_eq!(terminals.document, private_document_product);
        assert_eq!(terminals.pair_producer, terminals.pair_consumer);
        assert_eq!(terminals.byte_table_sum, terminals.byte_query_sum);
        assert_eq!(
            terminals.byte_table_zero_count,
            terminals.byte_query_zero_count
        );
        assert_ne!(terminals.node, [F::ONE; ZK_X509_DER_STARK_BUS_LANES_V1]);
        let comparator_end =
            ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 + trace.base.private_shape.comparator_rows;
        let mut indices: Vec<_> = (0..=trace.base.private_shape.parser_rows)
            .chain(ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1..=comparator_end)
            .collect();
        indices.extend([
            ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 - 1,
            ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1 - 1,
            ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1,
            ZK_X509_DER_STARK_TRACE_SIZE_V1 - 2,
            ZK_X509_DER_STARK_TRACE_SIZE_V1 - 1,
        ]);
        indices.sort_unstable();
        indices.dedup();
        let mut reusable_residues = Vec::with_capacity(ZK_X509_DER_STARK_CONSTRAINT_COUNT_V1);
        let reusable_allocation = reusable_residues.as_ptr();
        for index in indices {
            let next_index = (index + 1) % ZK_X509_DER_STARK_TRACE_SIZE_V1;
            let current =
                zk_x509_der_stark_aggregate_base_row_v1(&trace.base, index).expect("base row");
            let next = zk_x509_der_stark_aggregate_base_row_v1(&trace.base, next_index)
                .expect("next base");
            let current_aux =
                zk_x509_der_stark_aggregate_aux_row_v1(&trace, index).expect("aux row");
            let next_aux =
                zk_x509_der_stark_aggregate_aux_row_v1(&trace, next_index).expect("next aux");
            let fixed = schedule.fixed_row(index).expect("fixed");
            let next_fixed = schedule.fixed_row(next_index).expect("next fixed");
            let residues = evaluate_zk_x509_der_stark_residues_v1(
                &current,
                &next,
                &current_aux,
                &next_aux,
                &fixed,
                &next_fixed,
                challenges,
                public,
                terminal_claims,
            )
            .expect("numeric residues");
            evaluate_zk_x509_der_stark_residues_into_v1(
                &current,
                &next,
                &current_aux,
                &next_aux,
                &fixed,
                &next_fixed,
                challenges,
                public,
                terminal_claims,
                &mut reusable_residues,
            )
            .expect("reused numeric residues");
            assert_eq!(reusable_residues, residues);
            assert_eq!(
                reusable_residues.as_ptr(),
                reusable_allocation,
                "streaming evaluator must reuse its preallocated residue buffer"
            );
            assert!(
                residues.iter().all(|residue| *residue == F::ZERO),
                "aggregate row {index} has nonzero residues at {:?}",
                residues
                    .iter()
                    .enumerate()
                    .filter(|(_, residue)| **residue != F::ZERO)
                    .take(16)
                    .collect::<Vec<_>>()
            );
        }
    }
    #[test]
    fn native_column_streaming_is_an_exact_transpose_at_all_boundaries() {
        let ordered_set = [0x31, 0x04, 0x05, 0x00, 0x05, 0x00];
        let challenges = challenges();
        let base = build_zk_x509_der_stark_base_v1(&[&ordered_set]).expect("base");
        let trace = build_zk_x509_der_stark_trace_v1(base, challenges).expect("complete DER trace");
        let schedule = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1)
            .expect("fixed schedule");
        let comparator_end =
            ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 + trace.base.private_shape.comparator_rows;
        let mut sample_rows = vec![
            0,
            trace.base.private_shape.parser_rows - 1,
            trace.base.private_shape.parser_rows,
            ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1 - 1,
            ZK_X509_DER_STARK_MAX_PARSER_ROWS_V1,
            comparator_end - 1,
            comparator_end,
            ZK_X509_DER_STARK_FIXED_NON_PADDING_ROWS_V1,
            ZK_X509_DER_STARK_TRACE_SIZE_V1 - 1,
        ];
        sample_rows.sort_unstable();
        sample_rows.dedup();
        for column_index in 0..ZK_X509_DER_STARK_BASE_WIDTH_V1 {
            let column = build_zk_x509_der_stark_native_base_column_v1(&trace.base, column_index)
                .expect("base column");
            assert_eq!(column.len(), ZK_X509_DER_STARK_TRACE_SIZE_V1);
            for row in sample_rows.iter().copied() {
                assert_eq!(
                    column[row],
                    zk_x509_der_stark_native_base_cell_v1(&trace.base, row, column_index)
                        .expect("base cell")
                );
            }
        }
        for column_index in 0..ZK_X509_DER_STARK_AUX_WIDTH_V1 {
            let column = build_zk_x509_der_stark_native_aux_column_v1(&trace, column_index)
                .expect("aux column");
            assert_eq!(column.len(), ZK_X509_DER_STARK_TRACE_SIZE_V1);
            for row in sample_rows.iter().copied() {
                assert_eq!(
                    column[row],
                    zk_x509_der_stark_native_aux_cell_v1(&trace, row, column_index)
                        .expect("aux cell")
                );
            }
        }
        for column_index in 0..ZK_X509_DER_STARK_FIXED_WIDTH_V1 {
            let column = build_zk_x509_der_stark_native_fixed_column_v1(&schedule, column_index)
                .expect("fixed column");
            assert_eq!(column.len(), ZK_X509_DER_STARK_TRACE_SIZE_V1);
            for row in sample_rows.iter().copied() {
                assert_eq!(
                    column[row],
                    zk_x509_der_stark_native_fixed_cell_v1(&schedule, row, column_index)
                        .expect("fixed cell")
                );
            }
        }
        assert!(
            build_zk_x509_der_stark_native_base_column_v1(
                &trace.base,
                ZK_X509_DER_STARK_BASE_WIDTH_V1
            )
            .is_err()
        );
        assert!(
            build_zk_x509_der_stark_native_aux_column_v1(&trace, ZK_X509_DER_STARK_AUX_WIDTH_V1)
                .is_err()
        );
        assert!(
            build_zk_x509_der_stark_native_fixed_column_v1(
                &schedule,
                ZK_X509_DER_STARK_FIXED_WIDTH_V1
            )
            .is_err()
        );
    }
    #[test]
    fn every_auxiliary_cell_and_post_commitment_base_cell_is_observed() {
        let nested = [
            0x30, 0x0a, 0x31, 0x04, 0x05, 0x00, 0x05, 0x00, 0x02, 0x02, 0x00, 0x80,
        ];
        let challenges = challenges();
        let base = build_zk_x509_der_stark_base_v1(&[&nested]).expect("base");
        let trace = build_zk_x509_der_stark_trace_v1(base, challenges).expect("complete DER trace");
        let schedule = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1)
            .expect("fixed schedule");
        let public =
            derive_zk_x509_der_stark_public_terminals_v1(&ZkX509DerStarkShapeV1, challenges)
                .expect("public");
        let terminal_claims =
            zk_x509_der_stark_terminal_claims_v1(&trace).expect("terminal claims");
        let fixed = schedule.fixed_row(0).expect("fixed");
        let next_fixed = schedule.fixed_row(1).expect("next fixed");
        let next = trace.base.rows[1];
        let next_aux = trace.aux_rows[1];
        for column in 0..ZK_X509_DER_STARK_AUX_WIDTH_V1 {
            let mut changed = trace.aux_rows[0];
            changed[column] = changed[column].add(F(7));
            let residues = evaluate_zk_x509_der_stark_residues_v1(
                &trace.base.rows[0],
                &next,
                &changed,
                &next_aux,
                &fixed,
                &next_fixed,
                challenges,
                public,
                terminal_claims,
            )
            .expect("numeric residues");
            assert!(
                residues.iter().any(|residue| *residue != F::ZERO),
                "auxiliary column {column} is not observed"
            );
        }
        let byte_row = trace
            .base
            .rows
            .iter()
            .position(|row| row[BASE_BYTE_LOOKUP_MULTIPLICITY] != F::ZERO)
            .expect("queried byte row");
        let mut changed = trace.base.rows[byte_row];
        changed[BASE_BYTE_LOOKUP_MULTIPLICITY] = changed[BASE_BYTE_LOOKUP_MULTIPLICITY].add(F::ONE);
        let next_index = byte_row + 1;
        let fixed = schedule.fixed_row(byte_row).expect("fixed");
        let next_fixed = schedule.fixed_row(next_index).expect("next fixed");
        let residues = evaluate_zk_x509_der_stark_residues_v1(
            &changed,
            &trace.base.rows[next_index],
            &trace.aux_rows[byte_row],
            &trace.aux_rows[next_index],
            &fixed,
            &next_fixed,
            challenges,
            public,
            terminal_claims,
        )
        .expect("numeric residues");
        assert!(residues.iter().any(|residue| *residue != F::ZERO));
        let stack_pop_row = trace
            .base
            .rows
            .iter()
            .position(|row| {
                pack_bits_v1(&row[BASE_PHASE_BITS..BASE_PHASE_BITS + 3]) == F(PHASE_BOUNDARY as u64)
                    && row[BASE_CONSTRUCTED] == F::ONE
                    && pack_bits_v1(&row[BASE_DEPTH_BITS..BASE_DEPTH_BITS + 5]) != F::ONE
            })
            .expect("nested stack pop");
        let mut changed = trace.base.rows[stack_pop_row];
        changed[BASE_PAYLOAD + 5] = changed[BASE_PAYLOAD + 5].add(F::ONE);
        let next_index = stack_pop_row + 1;
        let fixed = schedule.fixed_row(stack_pop_row).expect("fixed");
        let next_fixed = schedule.fixed_row(next_index).expect("next fixed");
        let residues = evaluate_zk_x509_der_stark_residues_v1(
            &changed,
            &trace.base.rows[next_index],
            &trace.aux_rows[stack_pop_row],
            &trace.aux_rows[next_index],
            &fixed,
            &next_fixed,
            challenges,
            public,
            terminal_claims,
        )
        .expect("numeric residues");
        assert!(residues.iter().any(|residue| *residue != F::ZERO));
    }
    #[test]
    fn adversarial_bus_challenge_shape_and_terminal_mutations_fail_closed() {
        let nested = [
            0x30, 0x0a, 0x31, 0x04, 0x05, 0x00, 0x05, 0x00, 0x02, 0x02, 0x00, 0x80,
        ];
        let canonical_challenges = challenges();
        let canonical_base = build_zk_x509_der_stark_base_v1(&[&nested]).expect("canonical base");
        let mut invalid_challenges = canonical_challenges;
        invalid_challenges.tuple[0][0] = F::ZERO;
        assert!(
            build_zk_x509_der_stark_trace_v1(canonical_base.clone(), invalid_challenges).is_err()
        );
        invalid_challenges = canonical_challenges;
        invalid_challenges.tuple[1] = invalid_challenges.tuple[0];
        assert!(
            build_zk_x509_der_stark_trace_v1(canonical_base.clone(), invalid_challenges).is_err()
        );
        invalid_challenges = canonical_challenges;
        invalid_challenges.byte_lookup[2] = invalid_challenges.byte_lookup[1];
        assert!(
            build_zk_x509_der_stark_trace_v1(canonical_base.clone(), invalid_challenges).is_err()
        );
        let queried_byte_row = canonical_base
            .rows
            .iter()
            .position(|row| row[BASE_BYTE_LOOKUP_MULTIPLICITY] != F::ZERO)
            .expect("SET lookup byte");
        let queried_byte_tuple = byte_tuple_v1(
            canonical_base.rows[queried_byte_row][BASE_DOCUMENT],
            canonical_base.rows[queried_byte_row][BASE_OFFSET],
            canonical_base.rows[queried_byte_row][BASE_BYTE_VALUE],
        );
        invalid_challenges = canonical_challenges;
        invalid_challenges.byte_lookup[0] = F::ZERO.sub(compress_tuple_v1(
            &queried_byte_tuple,
            invalid_challenges.tuple[0],
        ));
        assert_ne!(invalid_challenges.byte_lookup[0], F::ZERO);
        assert!(invalid_challenges.validate().is_ok());
        let collision_trace =
            build_zk_x509_der_stark_trace_v1(canonical_base.clone(), invalid_challenges)
                .expect("zero denominator is a complete lookup case");
        let collision_terminals =
            zk_x509_der_stark_terminals_v1(&collision_trace).expect("collision terminals");
        assert_ne!(
            collision_terminals.byte_table_zero_count[0],
            F::ZERO,
            "the forced collision must exercise the zero-count path"
        );
        assert_eq!(
            collision_terminals.byte_table_zero_count,
            collision_terminals.byte_query_zero_count
        );
        let collision_schedule = compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1)
            .expect("collision schedule");
        let collision_index = ZK_X509_DER_STARK_TRACE_SIZE_V1 - 1;
        let collision_current =
            zk_x509_der_stark_aggregate_base_row_v1(&collision_trace.base, collision_index)
                .expect("collision base");
        let collision_next = zk_x509_der_stark_aggregate_base_row_v1(&collision_trace.base, 0)
            .expect("collision next base");
        let mut collision_aux =
            zk_x509_der_stark_aggregate_aux_row_v1(&collision_trace, collision_index)
                .expect("collision aux");
        let collision_next_aux = zk_x509_der_stark_aggregate_aux_row_v1(&collision_trace, 0)
            .expect("collision next aux");
        collision_aux[AUX_BYTE_TABLE_ZERO_COUNT_AFTER] =
            collision_aux[AUX_BYTE_TABLE_ZERO_COUNT_AFTER].add(F::ONE);
        let collision_residues = evaluate_zk_x509_der_stark_residues_v1(
            &collision_current,
            &collision_next,
            &collision_aux,
            &collision_next_aux,
            &collision_schedule
                .fixed_row(collision_index)
                .expect("collision fixed"),
            &collision_schedule
                .fixed_row(0)
                .expect("collision next fixed"),
            invalid_challenges,
            ZkX509DerStarkPublicTerminalsV1,
            zk_x509_der_stark_terminal_claims_v1(&collision_trace)
                .expect("collision terminal claims"),
        )
        .expect("collision residues");
        assert!(
            collision_residues.iter().any(|residue| *residue != F::ZERO),
            "a forged singular-factor count must fail"
        );
        let mut changed = canonical_base.clone();
        let stack_pop = changed
            .rows
            .iter()
            .position(|row| {
                pack_bits_v1(&row[BASE_PHASE_BITS..BASE_PHASE_BITS + 3]) == F(PHASE_BOUNDARY as u64)
                    && row[BASE_CONSTRUCTED] == F::ONE
                    && pack_bits_v1(&row[BASE_DEPTH_BITS..BASE_DEPTH_BITS + 5]) != F::ONE
            })
            .expect("nested pop");
        changed.rows[stack_pop][BASE_PAYLOAD + 5] =
            changed.rows[stack_pop][BASE_PAYLOAD + 5].add(F::ONE);
        assert!(
            build_zk_x509_der_stark_trace_v1(changed, canonical_challenges).is_err(),
            "wrong restored parent frame must break the stack permutation"
        );
        changed = canonical_base.clone();
        let queried_byte = changed
            .rows
            .iter()
            .position(|row| row[BASE_BYTE_LOOKUP_MULTIPLICITY] != F::ZERO)
            .expect("queried byte");
        changed.rows[queried_byte][BASE_BYTE_LOOKUP_MULTIPLICITY] =
            changed.rows[queried_byte][BASE_BYTE_LOOKUP_MULTIPLICITY].add(F::ONE);
        assert!(
            build_zk_x509_der_stark_trace_v1(changed, canonical_challenges).is_err(),
            "wrong table multiplicity must break the byte lookup"
        );
        changed = canonical_base.clone();
        changed.private_shape.document_lengths[0] += 1;
        assert!(
            build_zk_x509_der_stark_trace_v1(changed, canonical_challenges).is_err(),
            "private document lengths must bind the document product"
        );
        let trace = build_zk_x509_der_stark_trace_v1(canonical_base.clone(), canonical_challenges)
            .expect("canonical trace");
        let schedule =
            compile_zk_x509_der_stark_fixed_schedule_v1(ZkX509DerStarkShapeV1).expect("schedule");
        let final_index = ZK_X509_DER_STARK_TRACE_SIZE_V1 - 1;
        let current =
            zk_x509_der_stark_aggregate_base_row_v1(&trace.base, final_index).expect("base");
        let next = zk_x509_der_stark_aggregate_base_row_v1(&trace.base, 0).expect("next");
        let current_aux = zk_x509_der_stark_aggregate_aux_row_v1(&trace, final_index).expect("aux");
        let next_aux = zk_x509_der_stark_aggregate_aux_row_v1(&trace, 0).expect("next aux");
        let fixed = schedule.fixed_row(final_index).expect("fixed");
        let next_fixed = schedule.fixed_row(0).expect("next fixed");
        let public = derive_zk_x509_der_stark_public_terminals_v1(
            &ZkX509DerStarkShapeV1,
            canonical_challenges,
        )
        .expect("public");
        let mut terminal_claims =
            zk_x509_der_stark_terminal_claims_v1(&trace).expect("terminal claims");
        terminal_claims.input_byte[1] = terminal_claims.input_byte[1].add(F::ONE);
        let residues = evaluate_zk_x509_der_stark_residues_v1(
            &current,
            &next,
            &current_aux,
            &next_aux,
            &fixed,
            &next_fixed,
            canonical_challenges,
            public,
            terminal_claims,
        )
        .expect("residues");
        assert!(residues.iter().any(|residue| *residue != F::ZERO));
    }
    #[test]
    fn complete_evaluator_has_witness_independent_residue_shape() {
        let challenges = challenges();
        let public = ZkX509DerStarkPublicTerminalsV1;
        let terminal_claims = ZkX509DerStarkTerminalClaimsV1 {
            input_byte: [F(31), F(37), F(41), F(43)],
            node: [F(47), F(53), F(59), F(61)],
        };
        let mut state = 0x8b8b_8b8b_1234_5678_u64;
        let mut sample = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            F(state % 1_000_003)
        };
        let mut expected = None;
        for _ in 0..256 {
            let current = core::array::from_fn(|_| sample());
            let next = core::array::from_fn(|_| sample());
            let current_aux = core::array::from_fn(|_| sample());
            let next_aux = core::array::from_fn(|_| sample());
            let fixed = core::array::from_fn(|_| sample());
            let next_fixed = core::array::from_fn(|_| sample());
            let residues = evaluate_zk_x509_der_stark_residues_v1(
                &current,
                &next,
                &current_aux,
                &next_aux,
                &fixed,
                &next_fixed,
                challenges,
                public,
                terminal_claims,
            )
            .expect("total numeric evaluator");
            match expected {
                Some(expected) => assert_eq!(residues.len(), expected),
                None => expected = Some(residues.len()),
            }
        }
        assert_eq!(expected, Some(ZK_X509_DER_STARK_CONSTRAINT_COUNT_V1));
    }
    fn affine_row<const N: usize>(seed: u64, role: u64, point: F) -> [F; N] {
        core::array::from_fn(|index| {
            let index = u64::try_from(index).expect("column index fits u64");
            let intercept = F(seed
                .wrapping_mul(1_000_003)
                .wrapping_add(role.wrapping_mul(65_537))
                .wrapping_add(index.wrapping_mul(257))
                % (GOLDILOCKS_MODULUS_V1 - 1)
                + 1);
            let slope = F(seed
                .wrapping_mul(524_287)
                .wrapping_add(role.wrapping_mul(8_191))
                .wrapping_add(index.wrapping_mul(131))
                % (GOLDILOCKS_MODULUS_V1 - 1)
                + 1);
            intercept.add(slope.mul(point))
        })
    }
    fn finite_difference_degrees(samples: &[Vec<F>]) -> Vec<usize> {
        let residue_count = samples.first().map_or(0, Vec::len);
        assert!(samples.iter().all(|sample| sample.len() == residue_count));
        (0..residue_count)
            .map(|residue| {
                let mut differences = samples
                    .iter()
                    .map(|sample| sample[residue])
                    .collect::<Vec<_>>();
                let mut degree = 0;
                for order in 0..samples.len() {
                    if differences.iter().any(|value| *value != F::ZERO) {
                        degree = order;
                    }
                    if differences.len() == 1 {
                        break;
                    }
                    differences = differences
                        .windows(2)
                        .map(|pair| pair[1].sub(pair[0]))
                        .collect();
                }
                degree
            })
            .collect()
    }
    #[test]
    fn independently_interpolated_complete_air_degree_matches_registration() {
        const SAMPLE_COUNT: usize = 21;
        let challenges = challenges();
        let public = ZkX509DerStarkPublicTerminalsV1;
        let terminal_claims = ZkX509DerStarkTerminalClaimsV1 {
            input_byte: [F(31), F(37), F(41), F(43)],
            node: [F(47), F(53), F(59), F(61)],
        };
        let mut maximum_degrees = vec![0_usize; ZK_X509_DER_STARK_CONSTRAINT_COUNT_V1];
        // Independent affine directions make cancellation of a nonzero
        // leading homogeneous term fail closed across the complete evaluator,
        // while the final assertion separately proves that degree seven is
        // attained rather than merely budgeted.
        for seed in [3_u64, 5, 11, 17, 29, 43, 71, 101, 149, 211, 283, 367] {
            let samples = (0..SAMPLE_COUNT)
                .map(|point| {
                    let point = F(u64::try_from(point).expect("sample point"));
                    evaluate_zk_x509_der_stark_residues_v1(
                        &affine_row(seed, 1, point),
                        &affine_row(seed, 2, point),
                        &affine_row(seed, 3, point),
                        &affine_row(seed, 4, point),
                        &affine_row(seed, 5, point),
                        &affine_row(seed, 6, point),
                        challenges,
                        public,
                        terminal_claims,
                    )
                    .expect("total numeric evaluator")
                })
                .collect::<Vec<_>>();
            for (maximum, measured) in maximum_degrees
                .iter_mut()
                .zip(finite_difference_degrees(&samples))
            {
                *maximum = (*maximum).max(measured);
            }
        }
        let offenders = maximum_degrees
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, degree)| *degree > usize::from(ZK_X509_DER_STARK_CONSTRAINT_DEGREE_V1))
            .collect::<Vec<_>>();
        assert!(offenders.is_empty(), "high-degree residues: {offenders:?}");
        assert!(
            maximum_degrees
                .iter()
                .any(|degree| *degree == usize::from(ZK_X509_DER_STARK_CONSTRAINT_DEGREE_V1)),
            "registered degree must be attained"
        );
    }
    include!("der_stark_descriptor_tests.rs");
}
