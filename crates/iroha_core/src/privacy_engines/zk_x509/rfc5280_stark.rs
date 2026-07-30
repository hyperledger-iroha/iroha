//! Native numeric RFC 5280 adapter for the closed first-release X5S1 profile.
//!
//! The owner trace in [`super::der_air`] parses values for witness
//! construction, but none of its host Boolean decisions are proof inputs.
//! This adapter consumes exact strict-DER byte and node events, derives
//! semantic roles from a fixed parent/child/tag grammar, and recomputes the
//! admitted path, validity, extension, serial, and complete-CRL predicates
//! from committed operands. Fixed family ranges are reconstructed from one
//! constant public registration; a witness cannot select a row family.
//!
//! Four independently sampled Goldilocks lanes are used for every compressed
//! relation.  Composition/FRI challenge counts are deliberately outside this
//! module and remain three.  Aggregate registration and consensus activation
//! remain false until every terminal below is wired to its numeric consumer.

use thiserror::Error;

use super::{
    der_air::{
        ZK_X509_DER_AIR_MAX_DOCUMENTS_V1, ZK_X509_DER_AIR_MAX_EMBEDDED_DOCUMENTS_V1,
        ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1, ZkX509DerAirErrorV1,
        ZkX509DerDocumentTraceV1, ZkX509DerEkuV1, ZkX509Rfc5280DocumentKindV1,
        ZkX509Rfc5280DocumentProvenanceV1, ZkX509Rfc5280GrammarRoleV1,
        ZkX509Rfc5280NodeProvenanceV1, ZkX509Rfc5280StatementV1, ZkX509Rfc5280TraceV1,
        rfc5280_io_witnesses_v1,
    },
    der_stark::{
        ZK_X509_DER_STARK_BUS_LANES_V1, ZkX509DerStarkChallengesV1, ZkX509DerStarkErrorV1,
        ZkX509DerStarkNodeEventV1, ZkX509DerStarkTerminalClaimsV1,
        zk_x509_der_stark_input_byte_factor_v1, zk_x509_der_stark_node_factor_v1,
    },
    io_air::{ZkX509IoEndpointV1, ZkX509IoSegmentRoleV1},
    p256_aggregate_adapter::{
        P256_X5S1_CERTIFICATE_OR_CRL_SIGNATURES_V1, P256_X5S1_SIGNATURES_V1,
        P256BusTerminalClaimsV1, P256CrossTraceTerminalClaimV1, P256CrossTraceTerminalRoleV1,
        evaluate_p256_bus_terminal_claim_equalities_v1,
        evaluate_p256_cross_trace_terminal_claim_equalities_v1,
    },
    p256_cross_trace_bus::P256_CROSS_TRACE_LANES_V1,
    p256_ecdsa_air::P256EcdsaRoleV1,
    profile::{
        ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1, ZK_X509_MAX_CRL_ENTRIES_V1,
        ZK_X509_MAX_SERIAL_BYTES_V1, ZK_X509_UNCOMPRESSED_P256_BYTES_V1,
    },
    sha_call_bus_stark::{
        ZK_X509_SHA_BUS_LANES_V1, ZK_X509_SHA_CA_CALL_COUNT_V1, ZK_X509_SHA_CA_LEAF_CALL_V1,
        ZK_X509_SHA_CALL_COUNT_V1, ZK_X509_SHA_SEGMENT_COUNT_V1, ZkX509ShaCallBoundaryTerminalV1,
        ZkX509ShaCallTerminalV1, ZkX509ShaSegmentTerminalV1, zk_x509_sha_ca_call_identity_v1,
    },
};
use crate::privacy_engines::transparent_stark::{
    GOLDILOCKS_MODULUS_V1, GoldilocksFieldV1 as F, TransparentStarkErrorV1,
    TransparentTranscriptV1, sha256_frame_v1,
};

/// Stable identity of the inactive native RFC adapter.
pub(crate) const ZK_X509_RFC5280_STARK_DESCRIPTOR_V1: &[u8] = b"zk-x509-rfc5280-stark-v1-incompatible:native-log19:base113:aux264:fixed81:constraints1227:degree4:max-private-active-rows238481:fixed-public-nonpadding-rows292420:four-copy-lanes:zero-sized-public-shape:constant-registration-transcript:no-private-depth-length-count-or-family-boundary-disclosure:committed-family-active-prefixes:inactive-rows-canonical-zero:four-fixed-top-document-slots:top-document-max4096:optional-certificate-slot2-boolean-provenance-bound:depth2-slot2-zero-byte-zero-node-dummy:certificate-slot-active-output-channel:source-byte-and-node-terminals-exact-der-v1:canonical-parent-child-tag-ordinal-grammar:closed-four-or-five-certificate-extension-cardinality:no-host-role-labels:extension-embedded-exact-copy:algorithm-and-profile-fixed-byte-rows:decimal-calendar-to-unix-arithmetic:bounded-public-presentation-window:private-certificate-validity-covers-window:private-crl-interval-covers-window:pathlen-and-ca-state:ku-eku-bc:serial-positive-max20:complete-crl-max64:fixed-two-phase-serial-comparator-layout:fixed-six-phase-calendar-copy-layout:max-serial-comparisons127:max-serial-comparator-logical-rows2667:max-serial-comparator-physical-rows5334:max-serial-source-rows5334:leaf-vs-every-entry-nonmembership:adjacent-revoked-serial-strict-increasing-unsigned-magnitude:length-then-byte-lexicographic:active-prefix-count-and-zero-padding:first-magnitude-byte-nonzero:first-difference-range-checked:der-integer-optional-sign-octet-bound:serial-source-node-and-byte-zero-safe-log-lookups-with-singular-count-equality:serial-decimal-relation-to-comparator-calendar-range-four-lane-grand-product:one-compressed-factor-per-physical-row:zero-product-factors-total-no-prover-abort:full-input-affine-degree-audit:issuer-name-and-aki-ski-byte-equality:fixed-five-document-sha-call-sources:fixed-three-certificate-signature-key-slots:depth2-third-tbs-signature-key-canonical-dummy:full-signed-crl-commitment-and-tbs-p256-message-distinct:producer-and-consumer-terminals-enumerated:twenty-nine-relation-four-lane-union-bound-at-least171-bits:canonical-base-fixed-aux-column-provider:eighteen-verifier-fixed-output-role-endpoint-selectors:eighteen-independent-four-lane-output-role-products:governed-root-spki-and-certificate-slot-active-products-air-bound:x5r1-exact1420-byte-eighty-eight-record-terminal-claims:typed-family-role-endpoint-lane-addresses:reserved-terminal-slots-reconstructed-identity:der-rfc-terminal-equality-validator:verifier-final-row-claim-replay:x5q1-exact4876-byte-four-segment-plus-thirteen-compact-ca-call-boundaries-304-record-sha-terminal-claims:sha-terminal-segment-family-lane-addresses-fixed:compact-ca-call-role-and-order-fixed:verifier-committed-sha-terminal-replay:five-p256-witnesses-native-rust-fixed-certificate-crl-wallet-order:x5v1-exact5580-byte-five-signature-348-record-p256-terminal-claims:four-certificate-or-crl-then-wallet-role-order:p256-bus-cross-start-terminal-and-sink-addresses-fixed:canonical-goldilocks-big-endian:verifier-committed-p256-terminal-replay:compact-ca-subproof-dedicated-x5c1-x5c2-complete:ca-claim-envelope1310-108-fixed-records:ca-single-log7-trace128-base695-aux128-fixed80-constraints1379-degree3-13chunks:ca-local-lde-log14-mask306-deep52768-fri58-rounds5-terminal512-degree15-grinding20:shared-x5b1-main-six-base-roots-plus-ca-base-root-challenge-schedule:ca-public-profile-and-root-bound:ca-prover-self-verifies-independent-verifier-and-resource-gates:activation=false";
/// SHA-256 of [`ZK_X509_RFC5280_STARK_DESCRIPTOR_V1`].
pub(crate) const ZK_X509_RFC5280_STARK_DESCRIPTOR_SHA256_V1: [u8; 32] = [
    0x2c, 0x3d, 0xff, 0x77, 0xf1, 0x72, 0x51, 0xc0, 0x55, 0x34, 0x8c, 0xa1, 0x18, 0xf5, 0x1f, 0x4e,
    0x97, 0x9f, 0x4b, 0x03, 0xfe, 0xed, 0xed, 0xf6, 0xde, 0x78, 0xc1, 0x29, 0xa8, 0x4c, 0xa4, 0xd7,
];

/// Native trace logarithm after the 4 KiB X.509 admission cap.
pub(crate) const ZK_X509_RFC5280_STARK_TRACE_LOG2_V1: u8 = 19;
/// Native trace size.
pub(crate) const ZK_X509_RFC5280_STARK_TRACE_SIZE_V1: usize =
    1 << ZK_X509_RFC5280_STARK_TRACE_LOG2_V1;
/// Base columns, including committed private row-activity and chain-depth
/// selectors.
pub(crate) const ZK_X509_RFC5280_STARK_BASE_WIDTH_V1: usize = 113;
/// Six shared-bus product pairs, eighteen independent output-role products,
/// one serial-copy product pair, and two zero-safe DER-backed serial lookups
/// in four lanes.
pub(crate) const ZK_X509_RFC5280_STARK_AUX_WIDTH_V1: usize = 264;
/// Verifier-preprocessed family, boundary, address, and output-role columns.
pub(crate) const ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1: usize = 81;
/// Exact opened-row residue inventory.
pub(crate) const ZK_X509_RFC5280_STARK_CONSTRAINT_COUNT_V1: usize = 1_227;
/// Auditable evaluator-section inventory. The ordering follows
/// [`evaluate_zk_x509_rfc5280_stark_residues_v1`].
const RFC5280_RESIDUE_SECTIONS_V1: [(&str, usize); 20] = [
    ("common", 13),
    ("degree-normalization-helpers", 13),
    ("source-families", 7),
    ("source-node-grammar", 74),
    ("grammar-ordinal-local", 34),
    ("fixed-equal-decimal", 55),
    ("calendar", 63),
    ("relation-bit-flags", 4),
    ("serial-source", 29),
    ("serial-compare", 82),
    ("range-profile", 12),
    ("private-geometry", 121),
    ("fixed-products", 48),
    ("fixed-product-terminals", 88),
    ("output-role-products", 144),
    ("shared-copy-products", 28),
    ("grammar-ordinal-products", 28),
    ("profile-lookups", 96),
    ("serial-lookups", 144),
    ("grammar-lookups", 144),
];
/// Maximum committed-column degree.
pub(crate) const ZK_X509_RFC5280_STARK_CONSTRAINT_DEGREE_V1: u8 = 4;
/// Copy/call bus challenge lanes. This must remain equal to strict DER.
pub(crate) const ZK_X509_RFC5280_STARK_BUS_LANES_V1: usize = 4;
/// Maximum compressed events in any one relation.
pub(crate) const ZK_X509_RFC5280_STARK_RELATION_EVENT_BOUND_V1: usize =
    ZK_X509_RFC5280_STARK_TRACE_SIZE_V1;
/// Conservatively union-bounded compressed relations, including the
/// DER-byte/DER-node serial-source lookups and serial copy bus.
pub(crate) const ZK_X509_RFC5280_STARK_COMPRESSED_RELATIONS_V1: usize = 29;
/// Conservative remaining collision-security bits.
///
/// For Goldilocks `p = 2^64 - 2^32 + 1`, `p - 1 > 2^63`.  Each lane's
/// collision polynomial has degree at most `N = 2^19`; four independent lanes
/// therefore fail with probability `< (2^19 / 2^63)^4 = 2^-176`.  A union
/// over at most twenty-nine relations is `< 29 * 2^-176 < 2^-171`.
pub(crate) const ZK_X509_RFC5280_STARK_COPY_SOUNDNESS_BITS_V1: u16 = 171;

const RFC_TUPLE_CHALLENGE_LABELS_V1: [&[u8]; 12] = [
    b"zk-x509-rfc5280-bus-tuple-slot-00-v1",
    b"zk-x509-rfc5280-bus-tuple-slot-01-v1",
    b"zk-x509-rfc5280-bus-tuple-slot-02-v1",
    b"zk-x509-rfc5280-bus-tuple-slot-03-v1",
    b"zk-x509-rfc5280-bus-tuple-slot-04-v1",
    b"zk-x509-rfc5280-bus-tuple-slot-05-v1",
    b"zk-x509-rfc5280-bus-tuple-slot-06-v1",
    b"zk-x509-rfc5280-bus-tuple-slot-07-v1",
    b"zk-x509-rfc5280-bus-tuple-slot-08-v1",
    b"zk-x509-rfc5280-bus-tuple-slot-09-v1",
    b"zk-x509-rfc5280-bus-tuple-slot-10-v1",
    b"zk-x509-rfc5280-bus-tuple-slot-11-v1",
];

const MAX_TOP_LEVEL_SOURCE_BYTES_V1: usize =
    ZK_X509_DER_AIR_MAX_DOCUMENTS_V1 * ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1;
// Embedded extension payloads are disjoint slices of the admitted top-level
// documents, so their combined byte count cannot exceed the same 16 KiB.
const MAX_EMBEDDED_SOURCE_BYTES_V1: usize = MAX_TOP_LEVEL_SOURCE_BYTES_V1;
const MAX_SOURCE_BYTES_V1: usize = MAX_TOP_LEVEL_SOURCE_BYTES_V1 + MAX_EMBEDDED_SOURCE_BYTES_V1;
const MAX_SOURCE_NODES_V1: usize =
    (ZK_X509_DER_AIR_MAX_DOCUMENTS_V1 + ZK_X509_DER_AIR_MAX_EMBEDDED_DOCUMENTS_V1) * 2_048;
const MAX_SOURCE_DOCUMENTS_V1: usize =
    ZK_X509_DER_AIR_MAX_DOCUMENTS_V1 + ZK_X509_DER_AIR_MAX_EMBEDDED_DOCUMENTS_V1;
const SERIAL_COMPARISON_WIDTH_V1: usize = 1 + ZK_X509_MAX_SERIAL_BYTES_V1;
const MAX_SERIAL_COMPARISONS_V1: usize = 2 * ZK_X509_MAX_CRL_ENTRIES_V1 - 1;
const MAX_SERIAL_COMPARISON_ROWS_V1: usize = MAX_SERIAL_COMPARISONS_V1 * SERIAL_COMPARISON_WIDTH_V1;
const SERIAL_COMPARISON_PHASES_V1: usize = 2;
const MAX_SERIAL_COMPARISON_PHYSICAL_ROWS_V1: usize =
    MAX_SERIAL_COMPARISON_ROWS_V1 * SERIAL_COMPARISON_PHASES_V1;
const MAX_SERIAL_SOURCE_ROWS_V1: usize = 2 * MAX_SERIAL_COMPARISON_ROWS_V1;
const CALENDAR_COPY_PHASES_V1: usize = 6;
const MAX_SEMANTIC_ROWS_V1: usize = 55_360;
// Exact worst-case endpoint bytes after the 4 KiB cap, including both the
// producer and fixed consumer side and the distinct complete-signed-CRL
// padded message/length pair.
const MAX_OUTPUT_EVENT_ROWS_V1: usize = 45_410;
const MAX_ACTIVE_ROWS_V1: usize = MAX_SOURCE_BYTES_V1
    + MAX_SOURCE_NODES_V1
    + MAX_EMBEDDED_SOURCE_BYTES_V1
    + ZK_X509_RFC5280_GRAMMAR_RULE_COUNT_V1
    + (MAX_SOURCE_NODES_V1 - MAX_SOURCE_DOCUMENTS_V1)
    + MAX_SERIAL_SOURCE_ROWS_V1
    + MAX_SERIAL_COMPARISON_PHYSICAL_ROWS_V1
    + MAX_SEMANTIC_ROWS_V1
    + MAX_OUTPUT_EVENT_ROWS_V1;

// Fixed public registration geometry.  These capacities are independent of
// every private length/count and still fit the native log19 domain.
const FIXED_SOURCE_BYTE_ROWS_V1: usize = MAX_SOURCE_BYTES_V1;
const FIXED_SOURCE_NODE_ROWS_V1: usize = MAX_SOURCE_NODES_V1;
const FIXED_EMBEDDED_COPY_ROWS_V1: usize = MAX_EMBEDDED_SOURCE_BYTES_V1;
const FIXED_GRAMMAR_ROWS_V1: usize = ZK_X509_RFC5280_GRAMMAR_RULE_COUNT_V1 + MAX_SOURCE_NODES_V1;
const FIXED_FIXED_BYTE_ROWS_V1: usize = 16_384;
const FIXED_EQUAL_BYTE_ROWS_V1: usize = 16_384;
const FIXED_DECIMAL_ROWS_V1: usize = 1_024;
const FIXED_CALENDAR_ROWS_V1: usize = 80 * CALENDAR_COPY_PHASES_V1;
const FIXED_RELATION_ROWS_V1: usize = 1_024;
const FIXED_BIT_FLAG_ROWS_V1: usize = 256;
const FIXED_RANGE_ROWS_V1: usize = 8_192;
const FIXED_SEMANTIC_SOURCE_ROWS_V1: usize = 32_768;
const FIXED_SEMANTIC_CONSUMER_ROWS_V1: usize = 32_768;
const FIXED_OUTPUT_ROWS_PER_SIDE_V1: usize = MAX_OUTPUT_EVENT_ROWS_V1 / 2;
const FIXED_NON_PADDING_ROWS_V1: usize = FIXED_SOURCE_BYTE_ROWS_V1
    + FIXED_SOURCE_NODE_ROWS_V1
    + FIXED_EMBEDDED_COPY_ROWS_V1
    + FIXED_GRAMMAR_ROWS_V1
    + FIXED_FIXED_BYTE_ROWS_V1
    + FIXED_EQUAL_BYTE_ROWS_V1
    + FIXED_DECIMAL_ROWS_V1
    + FIXED_CALENDAR_ROWS_V1
    + FIXED_RELATION_ROWS_V1
    + FIXED_BIT_FLAG_ROWS_V1
    + MAX_SERIAL_SOURCE_ROWS_V1
    + MAX_SERIAL_COMPARISON_PHYSICAL_ROWS_V1
    + FIXED_RANGE_ROWS_V1
    + FIXED_SEMANTIC_SOURCE_ROWS_V1
    + FIXED_SEMANTIC_CONSUMER_ROWS_V1
    + FIXED_OUTPUT_ROWS_PER_SIDE_V1
    + FIXED_OUTPUT_ROWS_PER_SIDE_V1;
const FIXED_PADDING_ROWS_V1: usize =
    ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 - FIXED_NON_PADDING_ROWS_V1;

const _: () = assert!(ZK_X509_RFC5280_STARK_BUS_LANES_V1 == ZK_X509_DER_STARK_BUS_LANES_V1);
const _: () = assert!(MAX_ACTIVE_ROWS_V1 <= ZK_X509_RFC5280_STARK_TRACE_SIZE_V1);
const _: () = assert!(MAX_OUTPUT_EVENT_ROWS_V1 % 2 == 0);
const _: () = assert!(FIXED_NON_PADDING_ROWS_V1 < ZK_X509_RFC5280_STARK_TRACE_SIZE_V1);
const _: () = assert!(ZK_X509_RFC5280_STARK_CONSTRAINT_COUNT_V1 <= u16::MAX as usize);
const _: () = {
    let mut section = 0;
    let mut total = 0;
    while section < RFC5280_RESIDUE_SECTIONS_V1.len() {
        total += RFC5280_RESIDUE_SECTIONS_V1[section].1;
        section += 1;
    }
    assert!(total == ZK_X509_RFC5280_STARK_CONSTRAINT_COUNT_V1);
};

const fn serial_comparison_count_v1(crl_entries: usize) -> usize {
    if crl_entries == 0 {
        0
    } else {
        2 * crl_entries - 1
    }
}

const fn serial_comparison_rows_v1(crl_entries: usize) -> usize {
    serial_comparison_count_v1(crl_entries) * SERIAL_COMPARISON_WIDTH_V1
}

fn serial_comparison_descriptor_v1(
    comparison: usize,
) -> Result<(ZkX509Rfc5280SerialComparisonKindV1, u16, u16), ZkX509Rfc5280StarkErrorV1> {
    if comparison >= MAX_SERIAL_COMPARISONS_V1 {
        return Err(ZkX509Rfc5280StarkErrorV1::Shape);
    }
    if comparison == 0 || comparison % 2 == 1 {
        let entry = if comparison == 0 {
            0
        } else {
            (comparison + 1) / 2
        };
        return Ok((
            ZkX509Rfc5280SerialComparisonKindV1::LeafNonMembership,
            0,
            u16::try_from(entry + 1).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        ));
    }
    let adjacent = comparison / 2 - 1;
    Ok((
        ZkX509Rfc5280SerialComparisonKindV1::AdjacentStrictOrder,
        u16::try_from(adjacent + 1).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        u16::try_from(adjacent + 2).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
    ))
}

fn serial_source_descriptor_v1(
    ordinal: usize,
) -> Result<(u16, u16, ZkX509Rfc5280GrammarRoleV1, u16), ZkX509Rfc5280StarkErrorV1> {
    let source_group = ordinal / SERIAL_COMPARISON_WIDTH_V1;
    let comparison = source_group / 2;
    let right_side = source_group % 2 == 1;
    let offset = ordinal % SERIAL_COMPARISON_WIDTH_V1;
    let (_, left_instance, right_instance) = serial_comparison_descriptor_v1(comparison)?;
    let logical_id = if right_side {
        right_instance
    } else {
        left_instance
    };
    let (role, role_instance) = if logical_id == 0 {
        (ZkX509Rfc5280GrammarRoleV1::CertificateSerial, 0)
    } else {
        (ZkX509Rfc5280GrammarRoleV1::CrlEntrySerial, logical_id - 1)
    };
    Ok((
        logical_id,
        u16::try_from(offset).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        role,
        role_instance,
    ))
}

/// Fixed row families in canonical schedule order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ZkX509Rfc5280StarkFamilyV1 {
    SourceByte = 0,
    SourceNode = 1,
    EmbeddedCopy = 2,
    Grammar = 3,
    FixedByte = 4,
    EqualByte = 5,
    Decimal = 6,
    Calendar = 7,
    Relation = 8,
    BitFlags = 9,
    SerialSource = 10,
    SerialCompare = 11,
    RangeByte = 12,
    SemanticSource = 13,
    SemanticConsumer = 14,
    OutputProducer = 15,
    OutputConsumer = 16,
    Padding = 17,
}

const FAMILY_COUNT_V1: usize = ZkX509Rfc5280StarkFamilyV1::Padding as usize + 1;

/// Topology-neutral purpose of a producer/consumer output event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub(crate) enum ZkX509Rfc5280OutputRoleV1 {
    Projection = 1,
    CertificateTbsSha = 2,
    CrlTbsP256Message = 3,
    CrlCommitment = 4,
    P256Signature = 5,
    P256PublicKey = 6,
    IssuerSpkiSha = 7,
    GovernedTrustAnchor = 8,
    /// Private optional-certificate selector copied to the P-256 aggregate.
    CertificateSlotActive = 9,
}

const OUTPUT_ROLE_COUNT_V1: usize = ZkX509Rfc5280OutputRoleV1::CertificateSlotActive as usize;
const OUTPUT_ROLES_V1: [ZkX509Rfc5280OutputRoleV1; OUTPUT_ROLE_COUNT_V1] = [
    ZkX509Rfc5280OutputRoleV1::Projection,
    ZkX509Rfc5280OutputRoleV1::CertificateTbsSha,
    ZkX509Rfc5280OutputRoleV1::CrlTbsP256Message,
    ZkX509Rfc5280OutputRoleV1::CrlCommitment,
    ZkX509Rfc5280OutputRoleV1::P256Signature,
    ZkX509Rfc5280OutputRoleV1::P256PublicKey,
    ZkX509Rfc5280OutputRoleV1::IssuerSpkiSha,
    ZkX509Rfc5280OutputRoleV1::GovernedTrustAnchor,
    ZkX509Rfc5280OutputRoleV1::CertificateSlotActive,
];

fn output_role_from_index_v1(index: usize) -> Option<ZkX509Rfc5280OutputRoleV1> {
    OUTPUT_ROLES_V1.get(index).copied()
}

/// Private witness geometry.  None of these values is transcript material.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509Rfc5280StarkPrivateShapeV1 {
    pub(crate) chain_depth: u8,
    /// Committed selector copied to fixed certificate slot two.
    pub(crate) certificate_slot_2_active: F,
    pub(crate) top_document_count: u8,
    pub(crate) top_document_lengths: [u16; ZK_X509_DER_AIR_MAX_DOCUMENTS_V1],
    pub(crate) top_node_counts: [u16; ZK_X509_DER_AIR_MAX_DOCUMENTS_V1],
    pub(crate) embedded_document_count: u8,
    pub(crate) embedded_document_lengths: [u16; ZK_X509_DER_AIR_MAX_EMBEDDED_DOCUMENTS_V1],
    pub(crate) embedded_node_counts: [u16; ZK_X509_DER_AIR_MAX_EMBEDDED_DOCUMENTS_V1],
    pub(crate) crl_entries: u8,
    pub(crate) disclosed_attributes: u8,
    pub(crate) embedded_copy_rows: u32,
    pub(crate) grammar_rows: u32,
    pub(crate) fixed_byte_rows: u32,
    pub(crate) equality_rows: u32,
    pub(crate) decimal_rows: u32,
    pub(crate) calendar_rows: u32,
    pub(crate) relation_rows: u32,
    pub(crate) bit_flag_rows: u32,
    pub(crate) serial_source_rows: u32,
    pub(crate) serial_rows: u32,
    pub(crate) range_rows: u32,
    pub(crate) semantic_source_rows: u32,
    pub(crate) semantic_consumer_rows: u32,
    pub(crate) output_producer_rows: u32,
    pub(crate) output_consumer_rows: u32,
    pub(crate) io_channels: u16,
}

impl ZkX509Rfc5280StarkPrivateShapeV1 {
    fn checked_sum(
        values: impl IntoIterator<Item = usize>,
    ) -> Result<usize, ZkX509Rfc5280StarkErrorV1> {
        values
            .into_iter()
            .try_fold(0_usize, |sum, value| sum.checked_add(value))
            .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)
    }

    pub(crate) fn source_bytes(&self) -> Result<usize, ZkX509Rfc5280StarkErrorV1> {
        let top = self.top_document_lengths[..usize::from(self.top_document_count)]
            .iter()
            .map(|value| usize::from(*value));
        let embedded = self.embedded_document_lengths[..usize::from(self.embedded_document_count)]
            .iter()
            .map(|value| usize::from(*value));
        Self::checked_sum(top.chain(embedded))
    }

    pub(crate) fn source_nodes(&self) -> Result<usize, ZkX509Rfc5280StarkErrorV1> {
        let top = self.top_node_counts[..usize::from(self.top_document_count)]
            .iter()
            .map(|value| usize::from(*value));
        let embedded = self.embedded_node_counts[..usize::from(self.embedded_document_count)]
            .iter()
            .map(|value| usize::from(*value));
        Self::checked_sum(top.chain(embedded))
    }

    pub(crate) fn family_counts(
        &self,
    ) -> Result<[usize; FAMILY_COUNT_V1], ZkX509Rfc5280StarkErrorV1> {
        let mut counts = [0_usize; FAMILY_COUNT_V1];
        counts[ZkX509Rfc5280StarkFamilyV1::SourceByte as usize] = self.source_bytes()?;
        counts[ZkX509Rfc5280StarkFamilyV1::SourceNode as usize] = self.source_nodes()?;
        counts[ZkX509Rfc5280StarkFamilyV1::EmbeddedCopy as usize] =
            usize::try_from(self.embedded_copy_rows)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        counts[ZkX509Rfc5280StarkFamilyV1::Grammar as usize] =
            usize::try_from(self.grammar_rows).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        counts[ZkX509Rfc5280StarkFamilyV1::FixedByte as usize] =
            usize::try_from(self.fixed_byte_rows)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        counts[ZkX509Rfc5280StarkFamilyV1::EqualByte as usize] =
            usize::try_from(self.equality_rows).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        counts[ZkX509Rfc5280StarkFamilyV1::Decimal as usize] =
            usize::try_from(self.decimal_rows).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        counts[ZkX509Rfc5280StarkFamilyV1::Calendar as usize] =
            usize::try_from(self.calendar_rows).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        counts[ZkX509Rfc5280StarkFamilyV1::Relation as usize] =
            usize::try_from(self.relation_rows).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        counts[ZkX509Rfc5280StarkFamilyV1::BitFlags as usize] =
            usize::try_from(self.bit_flag_rows).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        counts[ZkX509Rfc5280StarkFamilyV1::SerialSource as usize] =
            usize::try_from(self.serial_source_rows)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        counts[ZkX509Rfc5280StarkFamilyV1::SerialCompare as usize] =
            usize::try_from(self.serial_rows).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        counts[ZkX509Rfc5280StarkFamilyV1::RangeByte as usize] =
            usize::try_from(self.range_rows).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        counts[ZkX509Rfc5280StarkFamilyV1::SemanticSource as usize] =
            usize::try_from(self.semantic_source_rows)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        counts[ZkX509Rfc5280StarkFamilyV1::SemanticConsumer as usize] =
            usize::try_from(self.semantic_consumer_rows)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        counts[ZkX509Rfc5280StarkFamilyV1::OutputProducer as usize] =
            usize::try_from(self.output_producer_rows)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        counts[ZkX509Rfc5280StarkFamilyV1::OutputConsumer as usize] =
            usize::try_from(self.output_consumer_rows)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        let active = Self::checked_sum(
            counts[..ZkX509Rfc5280StarkFamilyV1::Padding as usize]
                .iter()
                .copied(),
        )?;
        counts[ZkX509Rfc5280StarkFamilyV1::Padding as usize] = ZK_X509_RFC5280_STARK_TRACE_SIZE_V1
            .checked_sub(active)
            .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
        Ok(counts)
    }

    pub(crate) fn active_rows(&self) -> Result<usize, ZkX509Rfc5280StarkErrorV1> {
        let counts = self.family_counts()?;
        Self::checked_sum(
            counts[..ZkX509Rfc5280StarkFamilyV1::Padding as usize]
                .iter()
                .copied(),
        )
    }

    pub(crate) fn validate(&self) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
        let top_count = usize::from(self.top_document_count);
        let embedded_count = usize::from(self.embedded_document_count);
        if !(2..=3).contains(&usize::from(self.chain_depth))
            || self
                .certificate_slot_2_active
                .mul(self.certificate_slot_2_active.sub(F::ONE))
                != F::ZERO
            || self.certificate_slot_2_active != F(u64::from(usize::from(self.chain_depth) == 3))
            || top_count != usize::from(self.chain_depth) + 1
            || top_count > ZK_X509_DER_AIR_MAX_DOCUMENTS_V1
            || embedded_count > ZK_X509_DER_AIR_MAX_EMBEDDED_DOCUMENTS_V1
            || usize::from(self.crl_entries) > ZK_X509_MAX_CRL_ENTRIES_V1
            || self.disclosed_attributes > 4
            || self
                .top_document_lengths
                .iter()
                .enumerate()
                .any(|(slot, length)| {
                    let required = slot < top_count;
                    if required {
                        *length == 0
                            || usize::from(*length)
                                > ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1
                    } else {
                        *length != 0
                    }
                })
            || self
                .top_node_counts
                .iter()
                .enumerate()
                .any(|(slot, count)| {
                    let required = slot < top_count;
                    if required {
                        *count == 0 || *count > 2_048
                    } else {
                        *count != 0
                    }
                })
            || self.embedded_document_lengths[..embedded_count]
                .iter()
                .any(|length| *length == 0)
            || self.embedded_document_lengths[embedded_count..]
                .iter()
                .any(|length| *length != 0)
            || self.embedded_node_counts[..embedded_count]
                .iter()
                .any(|count| *count == 0 || *count > 2_048)
            || self.embedded_node_counts[embedded_count..]
                .iter()
                .any(|count| *count != 0)
            || usize::try_from(self.embedded_copy_rows).ok()
                != Some(
                    self.embedded_document_lengths[..embedded_count]
                        .iter()
                        .map(|length| usize::from(*length))
                        .sum(),
                )
            || self.grammar_rows as usize
                != self
                    .source_nodes()?
                    .checked_sub(top_count.saturating_add(embedded_count))
                    .and_then(|rows| rows.checked_add(ZK_X509_RFC5280_GRAMMAR_RULE_COUNT_V1))
                    .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?
            || self.calendar_rows as usize
                != usize::from(self.chain_depth)
                    .saturating_mul(2)
                    .saturating_add(2)
                    .saturating_add(usize::from(self.crl_entries))
                    .saturating_mul(CALENDAR_COPY_PHASES_V1)
            || self.serial_source_rows as usize
                != serial_comparison_rows_v1(usize::from(self.crl_entries)).saturating_mul(2)
            || self.serial_rows as usize
                != serial_comparison_rows_v1(usize::from(self.crl_entries))
                    .saturating_mul(SERIAL_COMPARISON_PHASES_V1)
            || self.output_producer_rows != self.output_consumer_rows
            || self.embedded_copy_rows as usize > FIXED_EMBEDDED_COPY_ROWS_V1
            || self.grammar_rows as usize > FIXED_GRAMMAR_ROWS_V1
            || self.fixed_byte_rows as usize > FIXED_FIXED_BYTE_ROWS_V1
            || self.equality_rows as usize > FIXED_EQUAL_BYTE_ROWS_V1
            || self.decimal_rows as usize > FIXED_DECIMAL_ROWS_V1
            || self.calendar_rows as usize > FIXED_CALENDAR_ROWS_V1
            || self.relation_rows as usize > FIXED_RELATION_ROWS_V1
            || self.bit_flag_rows as usize > FIXED_BIT_FLAG_ROWS_V1
            || self.serial_source_rows as usize > MAX_SERIAL_SOURCE_ROWS_V1
            || self.serial_rows as usize > MAX_SERIAL_COMPARISON_PHYSICAL_ROWS_V1
            || self.range_rows as usize > FIXED_RANGE_ROWS_V1
            || self.semantic_source_rows as usize > FIXED_SEMANTIC_SOURCE_ROWS_V1
            || self.semantic_consumer_rows as usize > FIXED_SEMANTIC_CONSUMER_ROWS_V1
            || self.output_producer_rows as usize > FIXED_OUTPUT_ROWS_PER_SIDE_V1
            || self.output_consumer_rows as usize > FIXED_OUTPUT_ROWS_PER_SIDE_V1
            || (self.fixed_byte_rows as usize)
                .saturating_add(self.equality_rows as usize)
                .saturating_add(self.decimal_rows as usize)
                .saturating_add(self.calendar_rows as usize)
                .saturating_add(self.relation_rows as usize)
                .saturating_add(self.bit_flag_rows as usize)
                .saturating_add(self.range_rows as usize)
                .saturating_add(self.semantic_source_rows as usize)
                .saturating_add(self.semantic_consumer_rows as usize)
                > MAX_SEMANTIC_ROWS_V1
            || self.output_producer_rows as usize > MAX_OUTPUT_EVENT_ROWS_V1 / 2
            || self.active_rows()? > MAX_ACTIVE_ROWS_V1
        {
            return Err(ZkX509Rfc5280StarkErrorV1::Shape);
        }
        let embedded_bytes: usize = self.embedded_document_lengths[..embedded_count]
            .iter()
            .map(|length| usize::from(*length))
            .sum();
        let top_bytes: usize = self.top_document_lengths[..top_count]
            .iter()
            .map(|length| usize::from(*length))
            .sum();
        if embedded_bytes > top_bytes
            || self.source_bytes()? > MAX_SOURCE_BYTES_V1
            || self.source_nodes()? > MAX_SOURCE_NODES_V1
        {
            return Err(ZkX509Rfc5280StarkErrorV1::Shape);
        }
        Ok(())
    }
}

/// Fixed-size verifier projection of the public RFC 5280 statement.
///
/// Private chain depth, document geometry, certificate validity intervals,
/// CRL-entry count, and all DER bytes remain absent.  The values retained
/// here are exactly those needed to regenerate public fixed columns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509Rfc5280StarkShapeV1 {
    presentation_not_before_unix_seconds: u64,
    presentation_not_after_unix_seconds: u64,
    leaf_key_usage: u16,
    leaf_extended_key_usage_count: u8,
    leaf_extended_key_usages: [u8; 3],
    crl_number: u64,
    disclosed_attribute_count: u8,
    disclosed_attribute_indices: [u8; 4],
}

impl Default for ZkX509Rfc5280StarkShapeV1 {
    fn default() -> Self {
        Self {
            presentation_not_before_unix_seconds: 1,
            presentation_not_after_unix_seconds: 2,
            leaf_key_usage: 1,
            leaf_extended_key_usage_count: 1,
            leaf_extended_key_usages: [1, 0, 0],
            crl_number: 1,
            disclosed_attribute_count: 0,
            disclosed_attribute_indices: [0; 4],
        }
    }
}

impl ZkX509Rfc5280StarkShapeV1 {
    pub(crate) fn from_statement(
        statement: &ZkX509Rfc5280StatementV1,
    ) -> Result<Self, ZkX509Rfc5280StarkErrorV1> {
        let mut leaf_extended_key_usages = [0_u8; 3];
        if statement.leaf_extended_key_usages.len() > leaf_extended_key_usages.len() {
            return Err(ZkX509Rfc5280StarkErrorV1::Shape);
        }
        for (output, usage) in leaf_extended_key_usages
            .iter_mut()
            .zip(statement.leaf_extended_key_usages.iter().copied())
        {
            *output = eku_code_v1(usage);
        }
        let mut disclosed_attribute_indices = [0_u8; 4];
        if statement.disclosed_attribute_indices.len() > disclosed_attribute_indices.len() {
            return Err(ZkX509Rfc5280StarkErrorV1::Shape);
        }
        disclosed_attribute_indices[..statement.disclosed_attribute_indices.len()]
            .copy_from_slice(&statement.disclosed_attribute_indices);
        let shape = Self {
            presentation_not_before_unix_seconds: statement.presentation_not_before_unix_seconds,
            presentation_not_after_unix_seconds: statement.presentation_not_after_unix_seconds,
            leaf_key_usage: statement.leaf_key_usage,
            leaf_extended_key_usage_count: u8::try_from(statement.leaf_extended_key_usages.len())
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            leaf_extended_key_usages,
            crl_number: statement.crl_number,
            disclosed_attribute_count: u8::try_from(statement.disclosed_attribute_indices.len())
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            disclosed_attribute_indices,
        };
        shape.validate()?;
        Ok(shape)
    }

    pub(crate) const fn family_counts(&self) -> [usize; FAMILY_COUNT_V1] {
        [
            FIXED_SOURCE_BYTE_ROWS_V1,
            FIXED_SOURCE_NODE_ROWS_V1,
            FIXED_EMBEDDED_COPY_ROWS_V1,
            FIXED_GRAMMAR_ROWS_V1,
            FIXED_FIXED_BYTE_ROWS_V1,
            FIXED_EQUAL_BYTE_ROWS_V1,
            FIXED_DECIMAL_ROWS_V1,
            FIXED_CALENDAR_ROWS_V1,
            FIXED_RELATION_ROWS_V1,
            FIXED_BIT_FLAG_ROWS_V1,
            MAX_SERIAL_SOURCE_ROWS_V1,
            MAX_SERIAL_COMPARISON_PHYSICAL_ROWS_V1,
            FIXED_RANGE_ROWS_V1,
            FIXED_SEMANTIC_SOURCE_ROWS_V1,
            FIXED_SEMANTIC_CONSUMER_ROWS_V1,
            FIXED_OUTPUT_ROWS_PER_SIDE_V1,
            FIXED_OUTPUT_ROWS_PER_SIDE_V1,
            FIXED_PADDING_ROWS_V1,
        ]
    }

    pub(crate) const fn active_rows(&self) -> usize {
        FIXED_NON_PADDING_ROWS_V1
    }

    pub(crate) fn validate(&self) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
        let eku_count = usize::from(self.leaf_extended_key_usage_count);
        let disclosed_count = usize::from(self.disclosed_attribute_count);
        if self.presentation_not_before_unix_seconds >= self.presentation_not_after_unix_seconds
            || self
                .presentation_not_after_unix_seconds
                .checked_sub(self.presentation_not_before_unix_seconds)
                .is_none_or(|width| width > 300)
            || self.leaf_key_usage == 0
            || eku_count == 0
            || eku_count > self.leaf_extended_key_usages.len()
            || self.leaf_extended_key_usages[..eku_count]
                .iter()
                .any(|usage| !(1..=3).contains(usage))
            || self.leaf_extended_key_usages[..eku_count]
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.leaf_extended_key_usages[eku_count..]
                .iter()
                .any(|usage| *usage != 0)
            || disclosed_count > self.disclosed_attribute_indices.len()
            || self.disclosed_attribute_indices[..disclosed_count]
                .iter()
                .any(|index| *index >= 4)
            || self.disclosed_attribute_indices[..disclosed_count]
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.disclosed_attribute_indices[disclosed_count..]
                .iter()
                .any(|index| *index != 0)
        {
            return Err(ZkX509Rfc5280StarkErrorV1::Shape);
        }
        let usages = self.leaf_extended_key_usages()?;
        encode_key_usage_v1(self.leaf_key_usage)?;
        encode_eku_v1(&usages)?;
        Ok(())
    }

    fn leaf_extended_key_usages(&self) -> Result<Vec<ZkX509DerEkuV1>, ZkX509Rfc5280StarkErrorV1> {
        self.leaf_extended_key_usages[..usize::from(self.leaf_extended_key_usage_count)]
            .iter()
            .copied()
            .map(eku_from_code_v1)
            .collect()
    }

    /// Canonical public transcript frame; it contains no private geometry.
    pub(crate) fn transcript_bytes(&self) -> Result<Vec<u8>, ZkX509Rfc5280StarkErrorV1> {
        self.validate()?;
        let mut bytes = Vec::with_capacity(8 + 8 + 2 + 1 + 3 + 8 + 1 + 4);
        bytes.extend_from_slice(&self.presentation_not_before_unix_seconds.to_be_bytes());
        bytes.extend_from_slice(&self.presentation_not_after_unix_seconds.to_be_bytes());
        bytes.extend_from_slice(&self.leaf_key_usage.to_be_bytes());
        bytes.push(self.leaf_extended_key_usage_count);
        bytes.extend_from_slice(&self.leaf_extended_key_usages);
        bytes.extend_from_slice(&self.crl_number.to_be_bytes());
        bytes.push(self.disclosed_attribute_count);
        bytes.extend_from_slice(&self.disclosed_attribute_indices);
        Ok(bytes)
    }

    pub(crate) fn schedule_digest(&self) -> Result<[u8; 32], ZkX509Rfc5280StarkErrorV1> {
        sha256_frame_v1(
            b"iroha:privacy:zk-x509:rfc5280-stark-schedule:v1",
            &[&self.transcript_bytes()?],
        )
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)
    }
}

/// Four post-base tuple-compression lanes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509Rfc5280StarkChallengesV1 {
    pub(crate) tuple: [[F; 12]; ZK_X509_RFC5280_STARK_BUS_LANES_V1],
}

impl ZkX509Rfc5280StarkChallengesV1 {
    pub(crate) fn validate(self) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
        for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
            for (index, value) in self.tuple[lane].iter().copied().enumerate() {
                if value == F::ZERO
                    || value.0 >= GOLDILOCKS_MODULUS_V1
                    || F::canonical(value.0).is_none()
                    || self.tuple[lane][..index].contains(&value)
                    || self.tuple[..lane]
                        .iter()
                        .flat_map(|previous| previous.iter())
                        .any(|previous| *previous == value)
                {
                    return Err(ZkX509Rfc5280StarkErrorV1::Challenge);
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn derive_zk_x509_rfc5280_stark_challenges_v1(
    transcript: &mut TransparentTranscriptV1,
) -> Result<ZkX509Rfc5280StarkChallengesV1, TransparentStarkErrorV1> {
    let mut challenges = ZkX509Rfc5280StarkChallengesV1 {
        tuple: [[F::ZERO; 12]; ZK_X509_RFC5280_STARK_BUS_LANES_V1],
    };
    for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
        for (value, label) in challenges.tuple[lane]
            .iter_mut()
            .zip(RFC_TUPLE_CHALLENGE_LABELS_V1)
        {
            *value = transcript.challenge_field(label)?;
        }
    }
    Ok(challenges)
}

/// Adapter construction or numeric-constraint failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum ZkX509Rfc5280StarkErrorV1 {
    #[error("zk-X509 RFC 5280 STARK public shape is invalid")]
    Shape,
    #[error("zk-X509 RFC 5280 STARK fixed grammar is invalid")]
    Grammar,
    #[error("zk-X509 RFC 5280 STARK semantic relation is invalid")]
    Semantic,
    #[error("zk-X509 RFC 5280 STARK source address or multiplicity is invalid")]
    Source,
    #[error("zk-X509 RFC 5280 STARK output topology is invalid")]
    Output,
    #[error("zk-X509 RFC 5280 STARK transcript challenge is invalid")]
    Challenge,
    #[error("zk-X509 RFC/SHA/P-256 STARK terminal-claim encoding is invalid")]
    TerminalClaim,
    #[error("zk-X509 RFC 5280 STARK resource envelope is exceeded")]
    Resource,
}

impl From<ZkX509DerAirErrorV1> for ZkX509Rfc5280StarkErrorV1 {
    fn from(_: ZkX509DerAirErrorV1) -> Self {
        Self::Semantic
    }
}

impl From<ZkX509DerStarkErrorV1> for ZkX509Rfc5280StarkErrorV1 {
    fn from(error: ZkX509DerStarkErrorV1) -> Self {
        match error {
            ZkX509DerStarkErrorV1::Challenge => Self::Challenge,
            ZkX509DerStarkErrorV1::Resource => Self::Resource,
            _ => Self::Source,
        }
    }
}

fn source_documents_v1(
    trace: &ZkX509Rfc5280TraceV1,
) -> impl Iterator<Item = &ZkX509DerDocumentTraceV1> {
    trace.documents.iter().chain(&trace.embedded_documents)
}

fn role_tag_is_admitted_v1(row: ZkX509Rfc5280NodeProvenanceV1) -> bool {
    use ZkX509Rfc5280GrammarRoleV1 as R;
    let tag = (row.tag_class, row.constructed, row.tag_number);
    match row.role {
        R::Certificate
        | R::CertificateTbs
        | R::CertificateOuterAlgorithm
        | R::CertificateTbsAlgorithm
        | R::CertificateValidity
        | R::CertificateSubject
        | R::CertificateIssuer
        | R::CertificateSpki
        | R::CertificateSpkiAlgorithm
        | R::CertificateExtensions
        | R::CertificateExtension
        | R::Crl
        | R::CrlTbs
        | R::CrlOuterAlgorithm
        | R::CrlTbsAlgorithm
        | R::CrlIssuer
        | R::CrlEntries
        | R::CrlEntry
        | R::CrlExtensions
        | R::CrlExtension
        | R::NameAttribute
        | R::EmbeddedAki
        | R::EmbeddedBasicConstraints
        | R::EmbeddedEku => tag == (0, true, 16),
        R::NameRdn => tag == (0, true, 17),
        R::CertificateVersion => tag == (2, true, 0),
        R::CertificateExtensionsWrapper => tag == (2, true, 3),
        R::CrlExtensionsWrapper => tag == (2, true, 0),
        R::CertificateVersionInteger
        | R::CertificateSerial
        | R::CrlVersion
        | R::CrlEntrySerial
        | R::EmbeddedBasicConstraintsPathLen
        | R::EmbeddedCrlNumber => tag == (0, false, 2),
        R::CertificateSignatureValue
        | R::CertificatePublicKey
        | R::CrlSignatureValue
        | R::EmbeddedKeyUsage => tag == (0, false, 3),
        R::CertificateExtensionOid
        | R::CrlExtensionOid
        | R::NameAttributeOid
        | R::AlgorithmOid
        | R::EmbeddedEkuOid => tag == (0, false, 6),
        R::CertificateExtensionCritical
        | R::CrlExtensionCritical
        | R::EmbeddedBasicConstraintsCa => tag == (0, false, 1),
        R::CertificateExtensionValue | R::CrlExtensionValue | R::EmbeddedSki => {
            tag == (0, false, 4)
        }
        R::EmbeddedAkiIdentifier => tag == (2, false, 0),
        R::CertificateNotBefore
        | R::CertificateNotAfter
        | R::CrlThisUpdate
        | R::CrlNextUpdate
        | R::CrlEntryTime => {
            row.tag_class == 0 && !row.constructed && matches!(row.tag_number, 23 | 24)
        }
        R::NameAttributeValue => {
            row.tag_class == 0 && !row.constructed && matches!(row.tag_number, 12 | 19)
        }
    }
}

fn root_role_v1(kind: ZkX509Rfc5280DocumentKindV1) -> ZkX509Rfc5280GrammarRoleV1 {
    use ZkX509Rfc5280DocumentKindV1 as D;
    use ZkX509Rfc5280GrammarRoleV1 as R;
    match kind {
        D::Certificate => R::Certificate,
        D::Crl => R::Crl,
        D::AuthorityKeyIdentifier => R::EmbeddedAki,
        D::SubjectKeyIdentifier => R::EmbeddedSki,
        D::KeyUsage => R::EmbeddedKeyUsage,
        D::BasicConstraints => R::EmbeddedBasicConstraints,
        D::ExtendedKeyUsage => R::EmbeddedEku,
        D::CrlNumber => R::EmbeddedCrlNumber,
    }
}

fn expected_child_role_v1(
    document: &ZkX509Rfc5280DocumentProvenanceV1,
    parent: ZkX509Rfc5280NodeProvenanceV1,
    child: ZkX509Rfc5280NodeProvenanceV1,
    child_count: usize,
) -> Result<(ZkX509Rfc5280GrammarRoleV1, u16), ZkX509Rfc5280StarkErrorV1> {
    use ZkX509Rfc5280GrammarRoleV1 as R;
    let ordinal = usize::from(child.child_ordinal);
    let inherited = parent.role_instance;
    let result = match parent.role {
        R::Certificate => match ordinal {
            0 => (R::CertificateTbs, inherited),
            1 => (R::CertificateOuterAlgorithm, 0),
            2 => (R::CertificateSignatureValue, inherited),
            _ => return Err(ZkX509Rfc5280StarkErrorV1::Grammar),
        },
        R::CertificateTbs => match ordinal {
            0 => (R::CertificateVersion, inherited),
            1 => (R::CertificateSerial, inherited),
            2 => (R::CertificateTbsAlgorithm, 1),
            3 => (R::CertificateIssuer, 0),
            4 => (R::CertificateValidity, inherited),
            5 => (R::CertificateSubject, 1),
            6 => (R::CertificateSpki, inherited),
            7 => (R::CertificateExtensionsWrapper, 0),
            _ => return Err(ZkX509Rfc5280StarkErrorV1::Grammar),
        },
        R::CertificateVersion if ordinal == 0 => (R::CertificateVersionInteger, inherited),
        R::CertificateOuterAlgorithm
        | R::CertificateTbsAlgorithm
        | R::CertificateSpkiAlgorithm
        | R::CrlOuterAlgorithm
        | R::CrlTbsAlgorithm => (
            R::AlgorithmOid,
            inherited
                .checked_mul(4)
                .and_then(|instance| instance.checked_add(child.child_ordinal))
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?,
        ),
        R::CertificateIssuer | R::CertificateSubject | R::CrlIssuer => (
            R::NameRdn,
            inherited
                .checked_mul(256)
                .and_then(|instance| instance.checked_add(child.child_ordinal))
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?,
        ),
        R::NameRdn => {
            let name = inherited / 256;
            let rdn = inherited % 256;
            (
                R::NameAttribute,
                name.checked_mul(1024)
                    .and_then(|instance| instance.checked_add(rdn.checked_mul(16)?))
                    .and_then(|instance| instance.checked_add(child.child_ordinal))
                    .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?,
            )
        }
        R::NameAttribute => match ordinal {
            0 => (R::NameAttributeOid, inherited),
            1 => (R::NameAttributeValue, inherited),
            _ => return Err(ZkX509Rfc5280StarkErrorV1::Grammar),
        },
        R::CertificateValidity => match ordinal {
            0 => (R::CertificateNotBefore, inherited),
            1 => (R::CertificateNotAfter, inherited),
            _ => return Err(ZkX509Rfc5280StarkErrorV1::Grammar),
        },
        R::CertificateSpki => match ordinal {
            0 => (R::CertificateSpkiAlgorithm, 2),
            1 => (R::CertificatePublicKey, inherited),
            _ => return Err(ZkX509Rfc5280StarkErrorV1::Grammar),
        },
        R::CertificateExtensionsWrapper if ordinal == 0 => (R::CertificateExtensions, 0),
        R::CertificateExtensions => (R::CertificateExtension, child.child_ordinal),
        R::CertificateExtension => {
            let role = if ordinal == 0 {
                R::CertificateExtensionOid
            } else if child_count == 3 && ordinal == 1 {
                R::CertificateExtensionCritical
            } else if ordinal + 1 == child_count {
                R::CertificateExtensionValue
            } else {
                return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
            };
            (role, inherited)
        }
        R::Crl => match ordinal {
            0 => (R::CrlTbs, 0),
            1 => (R::CrlOuterAlgorithm, 3),
            2 => (R::CrlSignatureValue, 0),
            _ => return Err(ZkX509Rfc5280StarkErrorV1::Grammar),
        },
        R::CrlTbs => {
            let has_entries = child_count == 7;
            match ordinal {
                0 => (R::CrlVersion, 0),
                1 => (R::CrlTbsAlgorithm, 4),
                2 => (R::CrlIssuer, 2),
                3 => (R::CrlThisUpdate, 0),
                4 => (R::CrlNextUpdate, 0),
                5 if has_entries => (R::CrlEntries, 0),
                5 | 6 => (R::CrlExtensionsWrapper, 0),
                _ => return Err(ZkX509Rfc5280StarkErrorV1::Grammar),
            }
        }
        R::CrlEntries => (R::CrlEntry, child.child_ordinal),
        R::CrlEntry => match ordinal {
            0 => (R::CrlEntrySerial, inherited),
            1 => (R::CrlEntryTime, inherited),
            _ => return Err(ZkX509Rfc5280StarkErrorV1::Grammar),
        },
        R::CrlExtensionsWrapper if ordinal == 0 => (R::CrlExtensions, 0),
        R::CrlExtensions => (R::CrlExtension, child.child_ordinal),
        R::CrlExtension => {
            let role = if ordinal == 0 {
                R::CrlExtensionOid
            } else if child_count == 3 && ordinal == 1 {
                R::CrlExtensionCritical
            } else if ordinal + 1 == child_count {
                R::CrlExtensionValue
            } else {
                return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
            };
            (role, inherited)
        }
        R::EmbeddedAki if ordinal == 0 => (R::EmbeddedAkiIdentifier, 0),
        R::EmbeddedBasicConstraints => match ordinal {
            0 => (R::EmbeddedBasicConstraintsCa, 0),
            1 => (R::EmbeddedBasicConstraintsPathLen, 0),
            _ => return Err(ZkX509Rfc5280StarkErrorV1::Grammar),
        },
        R::EmbeddedEku => (R::EmbeddedEkuOid, child.child_ordinal),
        _ => return Err(ZkX509Rfc5280StarkErrorV1::Grammar),
    };
    if document.kind == ZkX509Rfc5280DocumentKindV1::Certificate
        && matches!(result.0, R::CrlTbs | R::CrlEntry | R::CrlExtension)
    {
        return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
    }
    Ok(result)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ZkX509Rfc5280GrammarRuleV1 {
    parent_role: u16,
    child_role: u16,
    tag_class: u8,
    constructed: bool,
    tag_number: u32,
    ordinal_exact: bool,
    ordinal_last: bool,
    ordinal_parameter: u16,
    count_exact: bool,
    count_parameter: u16,
    quotient_scale: u16,
    remainder_scale: u16,
    ordinal_scale: u16,
    constant: u16,
    /// Zero for non-roots, then certificate, CRL, or embedded root.
    root_kind: u8,
}

const fn grammar_rule_v1(
    parent_role: u16,
    child_role: ZkX509Rfc5280GrammarRoleV1,
    tag_class: u8,
    constructed: bool,
    tag_number: u32,
    ordinal_exact: bool,
    ordinal_last: bool,
    ordinal_parameter: u16,
    count_exact: bool,
    count_parameter: u16,
    quotient_scale: u16,
    remainder_scale: u16,
    ordinal_scale: u16,
    constant: u16,
    root_kind: u8,
) -> ZkX509Rfc5280GrammarRuleV1 {
    ZkX509Rfc5280GrammarRuleV1 {
        parent_role,
        child_role: child_role as u16,
        tag_class,
        constructed,
        tag_number,
        ordinal_exact,
        ordinal_last,
        ordinal_parameter,
        count_exact,
        count_parameter,
        quotient_scale,
        remainder_scale,
        ordinal_scale,
        constant,
        root_kind,
    }
}

const fn exact_grammar_rule_v1(
    parent_role: ZkX509Rfc5280GrammarRoleV1,
    child_role: ZkX509Rfc5280GrammarRoleV1,
    tag_class: u8,
    constructed: bool,
    tag_number: u32,
    ordinal: u16,
    child_count: u16,
    quotient_scale: u16,
    remainder_scale: u16,
    ordinal_scale: u16,
    constant: u16,
) -> ZkX509Rfc5280GrammarRuleV1 {
    grammar_rule_v1(
        parent_role as u16,
        child_role,
        tag_class,
        constructed,
        tag_number,
        true,
        false,
        ordinal,
        true,
        child_count,
        quotient_scale,
        remainder_scale,
        ordinal_scale,
        constant,
        0,
    )
}

const fn inherited_grammar_rule_v1(
    parent_role: ZkX509Rfc5280GrammarRoleV1,
    child_role: ZkX509Rfc5280GrammarRoleV1,
    tag_class: u8,
    constructed: bool,
    tag_number: u32,
    ordinal: u16,
    child_count: u16,
) -> ZkX509Rfc5280GrammarRuleV1 {
    exact_grammar_rule_v1(
        parent_role,
        child_role,
        tag_class,
        constructed,
        tag_number,
        ordinal,
        child_count,
        256,
        1,
        0,
        0,
    )
}

const fn constant_grammar_rule_v1(
    parent_role: ZkX509Rfc5280GrammarRoleV1,
    child_role: ZkX509Rfc5280GrammarRoleV1,
    tag_class: u8,
    constructed: bool,
    tag_number: u32,
    ordinal: u16,
    child_count: u16,
    constant: u16,
) -> ZkX509Rfc5280GrammarRuleV1 {
    exact_grammar_rule_v1(
        parent_role,
        child_role,
        tag_class,
        constructed,
        tag_number,
        ordinal,
        child_count,
        0,
        0,
        0,
        constant,
    )
}

const fn root_grammar_rule_v1(
    child_role: ZkX509Rfc5280GrammarRoleV1,
    tag_class: u8,
    constructed: bool,
    tag_number: u32,
    child_count: u16,
    ordinal_scale: u16,
    root_kind: u8,
) -> ZkX509Rfc5280GrammarRuleV1 {
    grammar_rule_v1(
        0,
        child_role,
        tag_class,
        constructed,
        tag_number,
        false,
        false,
        0,
        true,
        child_count,
        0,
        0,
        ordinal_scale,
        0,
        root_kind,
    )
}

const fn any_ordinal_grammar_rule_v1(
    parent_role: ZkX509Rfc5280GrammarRoleV1,
    child_role: ZkX509Rfc5280GrammarRoleV1,
    tag_class: u8,
    constructed: bool,
    tag_number: u32,
    count_exact: bool,
    count_parameter: u16,
    quotient_scale: u16,
    remainder_scale: u16,
    ordinal_scale: u16,
) -> ZkX509Rfc5280GrammarRuleV1 {
    grammar_rule_v1(
        parent_role as u16,
        child_role,
        tag_class,
        constructed,
        tag_number,
        false,
        false,
        0,
        count_exact,
        count_parameter,
        quotient_scale,
        remainder_scale,
        ordinal_scale,
        0,
        0,
    )
}

const fn last_grammar_rule_v1(
    parent_role: ZkX509Rfc5280GrammarRoleV1,
    child_role: ZkX509Rfc5280GrammarRoleV1,
    tag_class: u8,
    constructed: bool,
    tag_number: u32,
    child_count: u16,
) -> ZkX509Rfc5280GrammarRuleV1 {
    grammar_rule_v1(
        parent_role as u16,
        child_role,
        tag_class,
        constructed,
        tag_number,
        false,
        true,
        0,
        true,
        child_count,
        256,
        1,
        0,
        0,
        0,
    )
}

// This is the verifier-owned closed grammar. Repeated rules are intentional:
// they distinguish UTC/GeneralizedTime tags and optional child-count forms.
const ZK_X509_RFC5280_GRAMMAR_RULES_V1: &[ZkX509Rfc5280GrammarRuleV1] = &[
    // Roots. Certificate instances are their fixed document ordinal.
    root_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::Certificate,
        0,
        true,
        16,
        3,
        1,
        1,
    ),
    root_grammar_rule_v1(ZkX509Rfc5280GrammarRoleV1::Crl, 0, true, 16, 3, 0, 2),
    root_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::EmbeddedAki,
        0,
        true,
        16,
        1,
        0,
        3,
    ),
    root_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::EmbeddedSki,
        0,
        false,
        4,
        0,
        0,
        3,
    ),
    root_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::EmbeddedKeyUsage,
        0,
        false,
        3,
        0,
        0,
        3,
    ),
    root_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::EmbeddedBasicConstraints,
        0,
        true,
        16,
        0,
        0,
        3,
    ),
    root_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::EmbeddedBasicConstraints,
        0,
        true,
        16,
        1,
        0,
        3,
    ),
    root_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::EmbeddedBasicConstraints,
        0,
        true,
        16,
        2,
        0,
        3,
    ),
    root_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::EmbeddedEku,
        0,
        true,
        16,
        1,
        0,
        3,
    ),
    root_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::EmbeddedEku,
        0,
        true,
        16,
        2,
        0,
        3,
    ),
    root_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::EmbeddedEku,
        0,
        true,
        16,
        3,
        0,
        3,
    ),
    root_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::EmbeddedCrlNumber,
        0,
        false,
        2,
        0,
        0,
        3,
    ),
    // Certificate and TBSCertificate.
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::Certificate,
        ZkX509Rfc5280GrammarRoleV1::CertificateTbs,
        0,
        true,
        16,
        0,
        3,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::Certificate,
        ZkX509Rfc5280GrammarRoleV1::CertificateOuterAlgorithm,
        0,
        true,
        16,
        1,
        3,
        0,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::Certificate,
        ZkX509Rfc5280GrammarRoleV1::CertificateSignatureValue,
        0,
        false,
        3,
        2,
        3,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateTbs,
        ZkX509Rfc5280GrammarRoleV1::CertificateVersion,
        2,
        true,
        0,
        0,
        8,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateTbs,
        ZkX509Rfc5280GrammarRoleV1::CertificateSerial,
        0,
        false,
        2,
        1,
        8,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateTbs,
        ZkX509Rfc5280GrammarRoleV1::CertificateTbsAlgorithm,
        0,
        true,
        16,
        2,
        8,
        1,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateTbs,
        ZkX509Rfc5280GrammarRoleV1::CertificateIssuer,
        0,
        true,
        16,
        3,
        8,
        0,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateTbs,
        ZkX509Rfc5280GrammarRoleV1::CertificateValidity,
        0,
        true,
        16,
        4,
        8,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateTbs,
        ZkX509Rfc5280GrammarRoleV1::CertificateSubject,
        0,
        true,
        16,
        5,
        8,
        1,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateTbs,
        ZkX509Rfc5280GrammarRoleV1::CertificateSpki,
        0,
        true,
        16,
        6,
        8,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateTbs,
        ZkX509Rfc5280GrammarRoleV1::CertificateExtensionsWrapper,
        2,
        true,
        3,
        7,
        8,
        0,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateVersion,
        ZkX509Rfc5280GrammarRoleV1::CertificateVersionInteger,
        0,
        false,
        2,
        0,
        1,
    ),
    // AlgorithmIdentifier children.
    exact_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateOuterAlgorithm,
        ZkX509Rfc5280GrammarRoleV1::AlgorithmOid,
        0,
        false,
        6,
        0,
        1,
        1024,
        4,
        1,
        0,
    ),
    exact_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateTbsAlgorithm,
        ZkX509Rfc5280GrammarRoleV1::AlgorithmOid,
        0,
        false,
        6,
        0,
        1,
        1024,
        4,
        1,
        0,
    ),
    exact_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateSpkiAlgorithm,
        ZkX509Rfc5280GrammarRoleV1::AlgorithmOid,
        0,
        false,
        6,
        0,
        2,
        1024,
        4,
        1,
        0,
    ),
    exact_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateSpkiAlgorithm,
        ZkX509Rfc5280GrammarRoleV1::AlgorithmOid,
        0,
        false,
        6,
        1,
        2,
        1024,
        4,
        1,
        0,
    ),
    exact_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlOuterAlgorithm,
        ZkX509Rfc5280GrammarRoleV1::AlgorithmOid,
        0,
        false,
        6,
        0,
        1,
        1024,
        4,
        1,
        0,
    ),
    exact_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlTbsAlgorithm,
        ZkX509Rfc5280GrammarRoleV1::AlgorithmOid,
        0,
        false,
        6,
        0,
        1,
        1024,
        4,
        1,
        0,
    ),
    // Name grammar.
    any_ordinal_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateIssuer,
        ZkX509Rfc5280GrammarRoleV1::NameRdn,
        0,
        true,
        17,
        false,
        0,
        0,
        256,
        1,
    ),
    any_ordinal_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateSubject,
        ZkX509Rfc5280GrammarRoleV1::NameRdn,
        0,
        true,
        17,
        false,
        0,
        0,
        256,
        1,
    ),
    any_ordinal_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlIssuer,
        ZkX509Rfc5280GrammarRoleV1::NameRdn,
        0,
        true,
        17,
        false,
        0,
        0,
        256,
        1,
    ),
    any_ordinal_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::NameRdn,
        ZkX509Rfc5280GrammarRoleV1::NameAttribute,
        0,
        true,
        16,
        false,
        0,
        1024,
        16,
        1,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::NameAttribute,
        ZkX509Rfc5280GrammarRoleV1::NameAttributeOid,
        0,
        false,
        6,
        0,
        2,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::NameAttribute,
        ZkX509Rfc5280GrammarRoleV1::NameAttributeValue,
        0,
        false,
        12,
        1,
        2,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::NameAttribute,
        ZkX509Rfc5280GrammarRoleV1::NameAttributeValue,
        0,
        false,
        19,
        1,
        2,
    ),
    // Validity and SPKI.
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateValidity,
        ZkX509Rfc5280GrammarRoleV1::CertificateNotBefore,
        0,
        false,
        23,
        0,
        2,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateValidity,
        ZkX509Rfc5280GrammarRoleV1::CertificateNotBefore,
        0,
        false,
        24,
        0,
        2,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateValidity,
        ZkX509Rfc5280GrammarRoleV1::CertificateNotAfter,
        0,
        false,
        23,
        1,
        2,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateValidity,
        ZkX509Rfc5280GrammarRoleV1::CertificateNotAfter,
        0,
        false,
        24,
        1,
        2,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateSpki,
        ZkX509Rfc5280GrammarRoleV1::CertificateSpkiAlgorithm,
        0,
        true,
        16,
        0,
        2,
        2,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateSpki,
        ZkX509Rfc5280GrammarRoleV1::CertificatePublicKey,
        0,
        false,
        3,
        1,
        2,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateExtensionsWrapper,
        ZkX509Rfc5280GrammarRoleV1::CertificateExtensions,
        0,
        true,
        16,
        0,
        1,
        0,
    ),
    any_ordinal_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateExtensions,
        ZkX509Rfc5280GrammarRoleV1::CertificateExtension,
        0,
        true,
        16,
        true,
        4,
        0,
        0,
        1,
    ),
    any_ordinal_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateExtensions,
        ZkX509Rfc5280GrammarRoleV1::CertificateExtension,
        0,
        true,
        16,
        true,
        5,
        0,
        0,
        1,
    ),
    // Certificate Extension has either OID/value or OID/critical/value.
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateExtension,
        ZkX509Rfc5280GrammarRoleV1::CertificateExtensionOid,
        0,
        false,
        6,
        0,
        2,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateExtension,
        ZkX509Rfc5280GrammarRoleV1::CertificateExtensionOid,
        0,
        false,
        6,
        0,
        3,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateExtension,
        ZkX509Rfc5280GrammarRoleV1::CertificateExtensionCritical,
        0,
        false,
        1,
        1,
        3,
    ),
    last_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateExtension,
        ZkX509Rfc5280GrammarRoleV1::CertificateExtensionValue,
        0,
        false,
        4,
        2,
    ),
    last_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CertificateExtension,
        ZkX509Rfc5280GrammarRoleV1::CertificateExtensionValue,
        0,
        false,
        4,
        3,
    ),
    // CRL and TBSCertList.
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::Crl,
        ZkX509Rfc5280GrammarRoleV1::CrlTbs,
        0,
        true,
        16,
        0,
        3,
        0,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::Crl,
        ZkX509Rfc5280GrammarRoleV1::CrlOuterAlgorithm,
        0,
        true,
        16,
        1,
        3,
        3,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::Crl,
        ZkX509Rfc5280GrammarRoleV1::CrlSignatureValue,
        0,
        false,
        3,
        2,
        3,
        0,
    ),
    // Six-child TBSCertList.
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlTbs,
        ZkX509Rfc5280GrammarRoleV1::CrlVersion,
        0,
        false,
        2,
        0,
        6,
        0,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlTbs,
        ZkX509Rfc5280GrammarRoleV1::CrlTbsAlgorithm,
        0,
        true,
        16,
        1,
        6,
        4,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlTbs,
        ZkX509Rfc5280GrammarRoleV1::CrlIssuer,
        0,
        true,
        16,
        2,
        6,
        2,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlTbs,
        ZkX509Rfc5280GrammarRoleV1::CrlThisUpdate,
        0,
        false,
        23,
        3,
        6,
        0,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlTbs,
        ZkX509Rfc5280GrammarRoleV1::CrlThisUpdate,
        0,
        false,
        24,
        3,
        6,
        0,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlTbs,
        ZkX509Rfc5280GrammarRoleV1::CrlNextUpdate,
        0,
        false,
        23,
        4,
        6,
        0,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlTbs,
        ZkX509Rfc5280GrammarRoleV1::CrlNextUpdate,
        0,
        false,
        24,
        4,
        6,
        0,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlTbs,
        ZkX509Rfc5280GrammarRoleV1::CrlExtensionsWrapper,
        2,
        true,
        0,
        5,
        6,
        0,
    ),
    // Seven-child TBSCertList.
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlTbs,
        ZkX509Rfc5280GrammarRoleV1::CrlVersion,
        0,
        false,
        2,
        0,
        7,
        0,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlTbs,
        ZkX509Rfc5280GrammarRoleV1::CrlTbsAlgorithm,
        0,
        true,
        16,
        1,
        7,
        4,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlTbs,
        ZkX509Rfc5280GrammarRoleV1::CrlIssuer,
        0,
        true,
        16,
        2,
        7,
        2,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlTbs,
        ZkX509Rfc5280GrammarRoleV1::CrlThisUpdate,
        0,
        false,
        23,
        3,
        7,
        0,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlTbs,
        ZkX509Rfc5280GrammarRoleV1::CrlThisUpdate,
        0,
        false,
        24,
        3,
        7,
        0,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlTbs,
        ZkX509Rfc5280GrammarRoleV1::CrlNextUpdate,
        0,
        false,
        23,
        4,
        7,
        0,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlTbs,
        ZkX509Rfc5280GrammarRoleV1::CrlNextUpdate,
        0,
        false,
        24,
        4,
        7,
        0,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlTbs,
        ZkX509Rfc5280GrammarRoleV1::CrlEntries,
        0,
        true,
        16,
        5,
        7,
        0,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlTbs,
        ZkX509Rfc5280GrammarRoleV1::CrlExtensionsWrapper,
        2,
        true,
        0,
        6,
        7,
        0,
    ),
    any_ordinal_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlEntries,
        ZkX509Rfc5280GrammarRoleV1::CrlEntry,
        0,
        true,
        16,
        false,
        0,
        0,
        0,
        1,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlEntry,
        ZkX509Rfc5280GrammarRoleV1::CrlEntrySerial,
        0,
        false,
        2,
        0,
        2,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlEntry,
        ZkX509Rfc5280GrammarRoleV1::CrlEntryTime,
        0,
        false,
        23,
        1,
        2,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlEntry,
        ZkX509Rfc5280GrammarRoleV1::CrlEntryTime,
        0,
        false,
        24,
        1,
        2,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlExtensionsWrapper,
        ZkX509Rfc5280GrammarRoleV1::CrlExtensions,
        0,
        true,
        16,
        0,
        1,
        0,
    ),
    any_ordinal_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlExtensions,
        ZkX509Rfc5280GrammarRoleV1::CrlExtension,
        0,
        true,
        16,
        true,
        2,
        0,
        0,
        1,
    ),
    inherited_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlExtension,
        ZkX509Rfc5280GrammarRoleV1::CrlExtensionOid,
        0,
        false,
        6,
        0,
        2,
    ),
    last_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::CrlExtension,
        ZkX509Rfc5280GrammarRoleV1::CrlExtensionValue,
        0,
        false,
        4,
        2,
    ),
    // Embedded document children.
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::EmbeddedAki,
        ZkX509Rfc5280GrammarRoleV1::EmbeddedAkiIdentifier,
        2,
        false,
        0,
        0,
        1,
        0,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::EmbeddedBasicConstraints,
        ZkX509Rfc5280GrammarRoleV1::EmbeddedBasicConstraintsCa,
        0,
        false,
        1,
        0,
        1,
        0,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::EmbeddedBasicConstraints,
        ZkX509Rfc5280GrammarRoleV1::EmbeddedBasicConstraintsCa,
        0,
        false,
        1,
        0,
        2,
        0,
    ),
    constant_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::EmbeddedBasicConstraints,
        ZkX509Rfc5280GrammarRoleV1::EmbeddedBasicConstraintsPathLen,
        0,
        false,
        2,
        1,
        2,
        0,
    ),
    any_ordinal_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::EmbeddedEku,
        ZkX509Rfc5280GrammarRoleV1::EmbeddedEkuOid,
        0,
        false,
        6,
        true,
        1,
        0,
        0,
        1,
    ),
    any_ordinal_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::EmbeddedEku,
        ZkX509Rfc5280GrammarRoleV1::EmbeddedEkuOid,
        0,
        false,
        6,
        true,
        2,
        0,
        0,
        1,
    ),
    any_ordinal_grammar_rule_v1(
        ZkX509Rfc5280GrammarRoleV1::EmbeddedEku,
        ZkX509Rfc5280GrammarRoleV1::EmbeddedEkuOid,
        0,
        false,
        6,
        true,
        3,
        0,
        0,
        1,
    ),
];

const ZK_X509_RFC5280_GRAMMAR_RULE_COUNT_V1: usize = ZK_X509_RFC5280_GRAMMAR_RULES_V1.len();
const _: () = assert!(ZK_X509_RFC5280_GRAMMAR_RULE_COUNT_V1 == 86);

fn grammar_tag_pack_v1(tag_class: F, constructed: F, tag_number: F) -> F {
    tag_class
        .add(constructed.mul(F(4)))
        .add(tag_number.mul(F(8)))
}

fn grammar_ordinal_pack_v1(exact: F, last: F, parameter: F) -> F {
    exact.add(last.mul(F(2))).add(parameter.mul(F(4)))
}

fn grammar_count_pack_v1(exact: F, parameter: F) -> F {
    exact.add(parameter.mul(F(2)))
}

fn grammar_rule_expected_cells_v1(rule: ZkX509Rfc5280GrammarRuleV1) -> [F; 10] {
    [
        F(u64::from(rule.parent_role)),
        F(u64::from(rule.child_role)),
        grammar_tag_pack_v1(
            F(u64::from(rule.tag_class)),
            F(u64::from(rule.constructed)),
            F(u64::from(rule.tag_number)),
        ),
        grammar_ordinal_pack_v1(
            F(u64::from(rule.ordinal_exact)),
            F(u64::from(rule.ordinal_last)),
            F(u64::from(rule.ordinal_parameter)),
        ),
        grammar_count_pack_v1(
            F(u64::from(rule.count_exact)),
            F(u64::from(rule.count_parameter)),
        ),
        F(u64::from(rule.quotient_scale)),
        F(u64::from(rule.remainder_scale)),
        F(u64::from(rule.ordinal_scale)),
        F(u64::from(rule.constant)),
        F(u64::from(rule.root_kind)),
    ]
}

const fn profile_role_required_v1(role: u16) -> bool {
    role == ZkX509Rfc5280GrammarRoleV1::CertificateOuterAlgorithm as u16
        || role == ZkX509Rfc5280GrammarRoleV1::CertificateTbsAlgorithm as u16
        || role == ZkX509Rfc5280GrammarRoleV1::CrlOuterAlgorithm as u16
        || role == ZkX509Rfc5280GrammarRoleV1::CrlTbsAlgorithm as u16
        || role == ZkX509Rfc5280GrammarRoleV1::CertificateSpkiAlgorithm as u16
        || role == ZkX509Rfc5280GrammarRoleV1::CertificateVersion as u16
        || role == ZkX509Rfc5280GrammarRoleV1::CertificateSignatureValue as u16
        || role == ZkX509Rfc5280GrammarRoleV1::CrlSignatureValue as u16
        || role == ZkX509Rfc5280GrammarRoleV1::CertificatePublicKey as u16
        || role == ZkX509Rfc5280GrammarRoleV1::CertificateExtensionOid as u16
        || role == ZkX509Rfc5280GrammarRoleV1::CrlExtensionOid as u16
        || role == ZkX509Rfc5280GrammarRoleV1::CertificateExtensionCritical as u16
        || role == ZkX509Rfc5280GrammarRoleV1::NameAttributeOid as u16
        || role == ZkX509Rfc5280GrammarRoleV1::EmbeddedKeyUsage as u16
        || role == ZkX509Rfc5280GrammarRoleV1::EmbeddedBasicConstraints as u16
        || role == ZkX509Rfc5280GrammarRoleV1::EmbeddedEku as u16
        || role == ZkX509Rfc5280GrammarRoleV1::EmbeddedCrlNumber as u16
}

fn grammar_rule_matches_node_v1(
    rule: ZkX509Rfc5280GrammarRuleV1,
    node: ZkX509Rfc5280NodeProvenanceV1,
    parent_role: u16,
    parent_instance: u16,
    child_count: u16,
) -> bool {
    let root = node.node == 0;
    let ordinal = if root {
        u16::from(node.document)
    } else {
        node.child_ordinal
    };
    if rule.parent_role != parent_role
        || rule.child_role != node.role as u16
        || rule.tag_class != node.tag_class
        || rule.constructed != node.constructed
        || rule.tag_number != node.tag_number
        || (rule.root_kind == 0) != !root
        || (rule.ordinal_exact && ordinal != rule.ordinal_parameter)
        || (rule.ordinal_last && ordinal.checked_add(1) != Some(child_count))
        || (rule.count_exact && child_count != rule.count_parameter)
    {
        return false;
    }
    let quotient = parent_instance / 256;
    let remainder = parent_instance % 256;
    u32::from(quotient)
        .checked_mul(u32::from(rule.quotient_scale))
        .and_then(|value| value.checked_add(u32::from(remainder) * u32::from(rule.remainder_scale)))
        .and_then(|value| value.checked_add(u32::from(ordinal) * u32::from(rule.ordinal_scale)))
        .and_then(|value| value.checked_add(u32::from(rule.constant)))
        == Some(u32::from(node.role_instance))
}

fn grammar_rule_index_for_node_v1(
    node: ZkX509Rfc5280NodeProvenanceV1,
    parent_role: u16,
    parent_instance: u16,
    child_count: u16,
) -> Result<usize, ZkX509Rfc5280StarkErrorV1> {
    let mut matches = ZK_X509_RFC5280_GRAMMAR_RULES_V1
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, rule)| {
            grammar_rule_matches_node_v1(*rule, node, parent_role, parent_instance, child_count)
        });
    let (index, _) = matches.next().ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?;
    if matches.next().is_some() {
        return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
    }
    Ok(index)
}

fn direct_parent_from_spans_v1(
    rows: &[ZkX509Rfc5280NodeProvenanceV1],
    index: usize,
) -> Option<usize> {
    let row = rows[index];
    if row.depth == 0 {
        return None;
    }
    rows[..index]
        .iter()
        .enumerate()
        .rev()
        .find_map(|(parent, candidate)| {
            (candidate.depth + 1 == row.depth
                && candidate.content_start <= row.start
                && candidate.content_end >= row.content_end)
                .then_some(parent)
        })
}

/// Independently validate exact owner provenance and the fixed grammar.
///
/// This deliberately does not trust a role merely because it was copied from
/// the DER builder. Parent links and child ordinals are recomputed from spans,
/// and the role/instance pair is then derived by the closed transition table.
pub(crate) fn validate_zk_x509_rfc5280_provenance_v1(
    trace: &ZkX509Rfc5280TraceV1,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    let source_documents = source_documents_v1(trace).collect::<Vec<_>>();
    if source_documents.len() != trace.semantic_provenance.len() {
        return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
    }
    for (document_index, (document, provenance)) in source_documents
        .into_iter()
        .zip(&trace.semantic_provenance)
        .enumerate()
    {
        if usize::from(provenance.document) != document_index
            || provenance.nodes.len() != document.nodes.len()
            || provenance.nodes.is_empty()
        {
            return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
        }
        if document_index < trace.documents.len() {
            if provenance.parent_document != u8::MAX || provenance.parent_node != u16::MAX {
                return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
            }
        } else {
            let parent_document = usize::from(provenance.parent_document);
            let parent_node = usize::from(provenance.parent_node);
            let parent = trace
                .semantic_provenance
                .get(parent_document)
                .and_then(|document| document.nodes.get(parent_node))
                .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?;
            if !matches!(
                parent.role,
                ZkX509Rfc5280GrammarRoleV1::CertificateExtensionValue
                    | ZkX509Rfc5280GrammarRoleV1::CrlExtensionValue
            ) {
                return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
            }
        }
        for (node_index, (row, owner)) in provenance.nodes.iter().zip(&document.nodes).enumerate() {
            if usize::from(row.document) != document_index
                || usize::from(row.node) != node_index
                || row.start
                    != u16::try_from(owner.start.value.0)
                        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?
                || row.content_start
                    != u16::try_from(owner.content_start.value.0)
                        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?
                || row.content_end
                    != u16::try_from(owner.end.value.0)
                        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?
                || row.depth
                    != u8::try_from(owner.depth.value.0)
                        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?
                || row.tag_class
                    != u8::try_from(owner.tag_class.value.0)
                        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?
                || row.constructed != (owner.constructed == F::ONE)
                || row.tag_number
                    != u32::try_from(owner.tag_number.value.0)
                        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?
                || !role_tag_is_admitted_v1(*row)
            {
                return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
            }
            let parent = direct_parent_from_spans_v1(&provenance.nodes, node_index);
            if node_index == 0 {
                let expected_instance =
                    if provenance.kind == ZkX509Rfc5280DocumentKindV1::Certificate {
                        u16::try_from(document_index)
                            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?
                    } else {
                        0
                    };
                if row.parent_node != u16::MAX
                    || row.child_ordinal != 0
                    || row.role != root_role_v1(provenance.kind)
                    || row.role_instance != expected_instance
                {
                    return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
                }
                continue;
            }
            let parent_index = parent.ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?;
            if usize::from(row.parent_node) != parent_index {
                return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
            }
            let derived_ordinal = provenance.nodes[..node_index]
                .iter()
                .filter(|candidate| usize::from(candidate.parent_node) == parent_index)
                .count();
            if usize::from(row.child_ordinal) != derived_ordinal {
                return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
            }
            let child_count = provenance
                .nodes
                .iter()
                .filter(|candidate| usize::from(candidate.parent_node) == parent_index)
                .count();
            let expected = expected_child_role_v1(
                provenance,
                provenance.nodes[parent_index],
                *row,
                child_count,
            )?;
            if (row.role, row.role_instance) != expected {
                return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
            }
        }
    }
    Ok(())
}

fn compress_tuple_v1(values: [F; 12], challenge: [F; 12]) -> F {
    values
        .into_iter()
        .zip(challenge)
        .fold(F::ZERO, |sum, (value, coefficient)| {
            sum.add(value.mul(coefficient))
        })
}

/// Exact strict-DER source terminals consumed by this adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509Rfc5280DerSourceTerminalsV1 {
    pub(crate) input_byte: [F; ZK_X509_RFC5280_STARK_BUS_LANES_V1],
    pub(crate) node: [F; ZK_X509_RFC5280_STARK_BUS_LANES_V1],
}

pub(crate) fn zk_x509_rfc5280_der_source_terminals_v1(
    trace: &ZkX509Rfc5280TraceV1,
    challenges: ZkX509DerStarkChallengesV1,
) -> Result<ZkX509Rfc5280DerSourceTerminalsV1, ZkX509Rfc5280StarkErrorV1> {
    validate_zk_x509_rfc5280_provenance_v1(trace)?;
    let mut input_byte = [F::ONE; ZK_X509_RFC5280_STARK_BUS_LANES_V1];
    let mut node = [F::ONE; ZK_X509_RFC5280_STARK_BUS_LANES_V1];
    for (document_index, document) in source_documents_v1(trace).enumerate() {
        for (address, byte) in document.bytes.iter().enumerate() {
            for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
                input_byte[lane] = input_byte[lane].mul(zk_x509_der_stark_input_byte_factor_v1(
                    F(u64::try_from(document_index)
                        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?),
                    F(u64::try_from(address).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?),
                    byte.value.value,
                    lane,
                    challenges,
                )?);
            }
        }
        let provenance = trace
            .semantic_provenance
            .get(document_index)
            .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?;
        for row in &provenance.nodes {
            let content_len = row
                .content_end
                .checked_sub(row.content_start)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?;
            let parent_frame = if row.parent_node == u16::MAX {
                0
            } else {
                row.parent_node
            };
            let event = ZkX509DerStarkNodeEventV1 {
                document: F(u64::from(row.document)),
                ordinal: F(u64::from(row.node)),
                parent_frame: F(u64::from(parent_frame)),
                tag_class: F(u64::from(row.tag_class)),
                tag_number: F(u64::from(row.tag_number)),
                constructed: F(u64::from(row.constructed)),
                start: F(u64::from(row.start)),
                content_start: F(u64::from(row.content_start)),
                content_end: F(u64::from(row.content_end)),
                depth: F(u64::from(row.depth)),
                content_len: F(u64::from(content_len)),
            };
            for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
                node[lane] =
                    node[lane].mul(zk_x509_der_stark_node_factor_v1(event, lane, challenges)?);
            }
        }
    }
    Ok(ZkX509Rfc5280DerSourceTerminalsV1 { input_byte, node })
}

/// Exactly three certificate-TBS slots, one TBSCertList, and one complete
/// signed CRL are document-derived SHA calls.
pub(crate) const ZK_X509_RFC5280_DOCUMENT_SHA_CALLS_V1: usize = 5;

/// Exact source slice for one document-derived SHA call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509Rfc5280ShaSourceV1 {
    pub(crate) call: u8,
    pub(crate) role: ZkX509Rfc5280OutputRoleV1,
    pub(crate) active: bool,
    pub(crate) document: u8,
    pub(crate) start: u16,
    pub(crate) length: u16,
}

fn find_role_node_v1(
    trace: &ZkX509Rfc5280TraceV1,
    document: usize,
    role: ZkX509Rfc5280GrammarRoleV1,
) -> Result<ZkX509Rfc5280NodeProvenanceV1, ZkX509Rfc5280StarkErrorV1> {
    let mut rows = trace
        .semantic_provenance
        .get(document)
        .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?
        .nodes
        .iter()
        .copied()
        .filter(|row| row.role == role);
    let row = rows.next().ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?;
    if rows.next().is_some() {
        return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
    }
    Ok(row)
}

fn find_role_instance_node_v1(
    trace: &ZkX509Rfc5280TraceV1,
    document: usize,
    role: ZkX509Rfc5280GrammarRoleV1,
    role_instance: u16,
) -> Result<ZkX509Rfc5280NodeProvenanceV1, ZkX509Rfc5280StarkErrorV1> {
    let mut rows = trace
        .semantic_provenance
        .get(document)
        .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?
        .nodes
        .iter()
        .copied()
        .filter(|row| row.role == role && row.role_instance == role_instance);
    let row = rows.next().ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?;
    if rows.next().is_some() {
        return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
    }
    Ok(row)
}

/// Enumerate the fixed five document SHA sources.
///
/// Slot two is always present. For a depth-two chain it is the unique
/// canonical inactive source `(document=0xff,start=0,length=0)`.
pub(crate) fn zk_x509_rfc5280_document_sha_sources_v1(
    trace: &ZkX509Rfc5280TraceV1,
) -> Result<
    [ZkX509Rfc5280ShaSourceV1; ZK_X509_RFC5280_DOCUMENT_SHA_CALLS_V1],
    ZkX509Rfc5280StarkErrorV1,
> {
    validate_zk_x509_rfc5280_provenance_v1(trace)?;
    let mut sources = [ZkX509Rfc5280ShaSourceV1 {
        call: 0,
        role: ZkX509Rfc5280OutputRoleV1::CertificateTbsSha,
        active: false,
        document: u8::MAX,
        start: 0,
        length: 0,
    }; ZK_X509_RFC5280_DOCUMENT_SHA_CALLS_V1];
    for certificate_slot in 0..3 {
        sources[certificate_slot].call =
            u8::try_from(certificate_slot).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        if certificate_slot < trace.certificates.len() {
            let row = find_role_node_v1(
                trace,
                certificate_slot,
                ZkX509Rfc5280GrammarRoleV1::CertificateTbs,
            )?;
            sources[certificate_slot].active = true;
            sources[certificate_slot].document = row.document;
            sources[certificate_slot].start = row.start;
            sources[certificate_slot].length = row
                .content_end
                .checked_sub(row.start)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?;
        }
    }
    let crl_document = trace.certificates.len();
    let crl_tbs = find_role_node_v1(trace, crl_document, ZkX509Rfc5280GrammarRoleV1::CrlTbs)?;
    sources[3] = ZkX509Rfc5280ShaSourceV1 {
        call: 3,
        role: ZkX509Rfc5280OutputRoleV1::CrlTbsP256Message,
        active: true,
        document: crl_tbs.document,
        start: crl_tbs.start,
        length: crl_tbs
            .content_end
            .checked_sub(crl_tbs.start)
            .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?,
    };
    let complete_crl = find_role_node_v1(trace, crl_document, ZkX509Rfc5280GrammarRoleV1::Crl)?;
    sources[4] = ZkX509Rfc5280ShaSourceV1 {
        call: 4,
        role: ZkX509Rfc5280OutputRoleV1::CrlCommitment,
        active: true,
        document: complete_crl.document,
        start: complete_crl.start,
        length: complete_crl
            .content_end
            .checked_sub(complete_crl.start)
            .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?,
    };
    Ok(sources)
}

fn endpoint_role_code_v1(role: ZkX509IoSegmentRoleV1) -> Result<u64, ZkX509Rfc5280StarkErrorV1> {
    match role {
        ZkX509IoSegmentRoleV1::StrictDer => Ok(1),
        ZkX509IoSegmentRoleV1::Sha256 => Ok(2),
        ZkX509IoSegmentRoleV1::P256 => Ok(3),
        ZkX509IoSegmentRoleV1::CaAccumulator => Ok(4),
        ZkX509IoSegmentRoleV1::Projection => Ok(6),
        ZkX509IoSegmentRoleV1::PublicInput => Ok(7),
        _ => Err(ZkX509Rfc5280StarkErrorV1::Output),
    }
}

fn output_role_index_v1(role: ZkX509Rfc5280OutputRoleV1) -> usize {
    role as usize - 1
}

fn output_roles_v1(
    trace: &ZkX509Rfc5280TraceV1,
) -> Result<Vec<ZkX509Rfc5280OutputRoleV1>, ZkX509Rfc5280StarkErrorV1> {
    let mut roles = Vec::new();
    roles.extend(core::iter::repeat_n(
        ZkX509Rfc5280OutputRoleV1::Projection,
        3 + 2 + 2 * trace.statement.disclosed_attribute_indices.len(),
    ));
    // Three fixed padded/length TBS channel pairs.
    roles.extend(core::iter::repeat_n(
        ZkX509Rfc5280OutputRoleV1::CertificateTbsSha,
        6,
    ));
    roles.push(ZkX509Rfc5280OutputRoleV1::CertificateSlotActive);
    for _ in 0..3 {
        roles.extend([
            ZkX509Rfc5280OutputRoleV1::P256Signature,
            ZkX509Rfc5280OutputRoleV1::P256Signature,
            ZkX509Rfc5280OutputRoleV1::P256PublicKey,
        ]);
    }
    roles.extend([
        ZkX509Rfc5280OutputRoleV1::CrlTbsP256Message,
        ZkX509Rfc5280OutputRoleV1::CrlTbsP256Message,
        ZkX509Rfc5280OutputRoleV1::CrlCommitment,
        ZkX509Rfc5280OutputRoleV1::CrlCommitment,
        ZkX509Rfc5280OutputRoleV1::P256Signature,
        ZkX509Rfc5280OutputRoleV1::P256Signature,
        ZkX509Rfc5280OutputRoleV1::P256PublicKey,
        ZkX509Rfc5280OutputRoleV1::P256PublicKey,
        ZkX509Rfc5280OutputRoleV1::IssuerSpkiSha,
        ZkX509Rfc5280OutputRoleV1::GovernedTrustAnchor,
    ]);
    Ok(roles)
}

fn output_factor_v1(
    role: ZkX509Rfc5280OutputRoleV1,
    channel: u32,
    endpoint: ZkX509IoEndpointV1,
    offset: usize,
    value: u8,
    is_write: bool,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> Result<F, ZkX509Rfc5280StarkErrorV1> {
    zk_x509_rfc5280_opened_output_factor_v1(
        role,
        F(u64::from(channel)),
        endpoint,
        F(u64::try_from(offset).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?),
        F(u64::from(value)),
        F(u64::from(is_write)),
        lane,
        challenges,
    )
}

/// Compress one downstream output event directly from constrained field cells.
///
/// Downstream numeric adapters use this exact expression for their consumer
/// products. Byte range, offset, and direction constraints remain owned by the
/// calling adapter; this helper solely guarantees an identical typed tuple and
/// challenge domain on both sides of the RFC relation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn zk_x509_rfc5280_opened_output_factor_v1(
    role: ZkX509Rfc5280OutputRoleV1,
    channel: F,
    endpoint: ZkX509IoEndpointV1,
    offset: F,
    value: F,
    is_write: F,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> Result<F, ZkX509Rfc5280StarkErrorV1> {
    zk_x509_rfc5280_opened_output_factor_fields_v1(
        F(role as u64),
        channel,
        F(endpoint_role_code_v1(endpoint.role)?),
        F(u64::from(endpoint.instance)),
        offset,
        value,
        is_write,
        lane,
        challenges,
    )
}

/// Field-native form of the RFC output tuple compression.
///
/// This is used at STARK query points where verifier-fixed role/channel cells
/// are polynomial openings rather than host enums.  Native-row callers should
/// prefer [`zk_x509_rfc5280_opened_output_factor_v1`].
#[allow(clippy::too_many_arguments)]
pub(crate) fn zk_x509_rfc5280_opened_output_factor_fields_v1(
    role: F,
    channel: F,
    endpoint_role: F,
    endpoint_instance: F,
    offset: F,
    value: F,
    is_write: F,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> Result<F, ZkX509Rfc5280StarkErrorV1> {
    challenges.validate()?;
    zk_x509_rfc5280_opened_output_factor_fields_after_challenge_validation_v1(
        role,
        channel,
        endpoint_role,
        endpoint_instance,
        offset,
        value,
        is_write,
        lane,
        challenges,
    )
}

/// Field-native tuple compression after the owning adapter has validated the
/// challenge family once at its public boundary.
///
/// This avoids repeating the full 48-scalar validation on every row and lane
/// of a multi-million-row adapter. The lane bound remains checked here.
#[allow(clippy::too_many_arguments)]
pub(crate) fn zk_x509_rfc5280_opened_output_factor_fields_after_challenge_validation_v1(
    role: F,
    channel: F,
    endpoint_role: F,
    endpoint_instance: F,
    offset: F,
    value: F,
    is_write: F,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> Result<F, ZkX509Rfc5280StarkErrorV1> {
    let challenge = challenges
        .tuple
        .get(lane)
        .copied()
        .ok_or(ZkX509Rfc5280StarkErrorV1::Challenge)?;
    Ok(compress_tuple_v1(
        [
            F(80),
            role,
            channel,
            endpoint_role,
            endpoint_instance,
            offset,
            value,
            is_write,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
        ],
        challenge,
    ))
}

/// Exact per-purpose producer and fixed-consumer terminal products.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509Rfc5280OutputTerminalsV1 {
    pub(crate) producer: [[F; ZK_X509_RFC5280_STARK_BUS_LANES_V1]; OUTPUT_ROLE_COUNT_V1],
    pub(crate) consumer: [[F; ZK_X509_RFC5280_STARK_BUS_LANES_V1]; OUTPUT_ROLE_COUNT_V1],
    pub(crate) producer_events: [u32; OUTPUT_ROLE_COUNT_V1],
    pub(crate) consumer_events: [u32; OUTPUT_ROLE_COUNT_V1],
}

/// Enumerate and compress both sides of every exact downstream I/O channel.
///
/// Producer and consumer factors intentionally include endpoint and write
/// direction, so they are not equated to one another inside this adapter.
/// Each downstream numeric adapter must expose the matching role terminal.
pub(crate) fn zk_x509_rfc5280_output_terminals_v1(
    trace: &ZkX509Rfc5280TraceV1,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> Result<ZkX509Rfc5280OutputTerminalsV1, ZkX509Rfc5280StarkErrorV1> {
    challenges.validate()?;
    let witnesses = rfc5280_io_witnesses_v1(trace, 0)?;
    let roles = output_roles_v1(trace)?;
    if roles.len() != witnesses.len() {
        return Err(ZkX509Rfc5280StarkErrorV1::Output);
    }
    let mut terminals = ZkX509Rfc5280OutputTerminalsV1 {
        producer: [[F::ONE; ZK_X509_RFC5280_STARK_BUS_LANES_V1]; OUTPUT_ROLE_COUNT_V1],
        consumer: [[F::ONE; ZK_X509_RFC5280_STARK_BUS_LANES_V1]; OUTPUT_ROLE_COUNT_V1],
        producer_events: [0; OUTPUT_ROLE_COUNT_V1],
        consumer_events: [0; OUTPUT_ROLE_COUNT_V1],
    };
    for (role, witness) in roles.into_iter().zip(witnesses) {
        if witness.producer_value.len()
            != usize::try_from(witness.declaration.byte_len)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?
            || witness.consumer_values.len() != witness.declaration.consumers.len()
        {
            return Err(ZkX509Rfc5280StarkErrorV1::Output);
        }
        let role_index = output_role_index_v1(role);
        for (offset, value) in witness.producer_value.iter().copied().enumerate() {
            for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
                terminals.producer[role_index][lane] =
                    terminals.producer[role_index][lane].mul(output_factor_v1(
                        role,
                        witness.declaration.channel,
                        witness.declaration.producer,
                        offset,
                        value,
                        true,
                        lane,
                        challenges,
                    )?);
            }
            terminals.producer_events[role_index] = terminals.producer_events[role_index]
                .checked_add(1)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
        }
        for (consumer, values) in witness
            .declaration
            .consumers
            .iter()
            .copied()
            .zip(witness.consumer_values)
        {
            if values.len() != witness.producer_value.len() {
                return Err(ZkX509Rfc5280StarkErrorV1::Output);
            }
            for (offset, value) in values.into_iter().enumerate() {
                for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
                    terminals.consumer[role_index][lane] = terminals.consumer[role_index][lane]
                        .mul(output_factor_v1(
                            role,
                            witness.declaration.channel,
                            consumer,
                            offset,
                            value,
                            false,
                            lane,
                            challenges,
                        )?);
                }
                terminals.consumer_events[role_index] = terminals.consumer_events[role_index]
                    .checked_add(1)
                    .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
            }
        }
    }
    if terminals.producer_events != terminals.consumer_events
        || terminals
            .producer_events
            .iter()
            .map(|events| usize::try_from(*events).unwrap_or(usize::MAX))
            .sum::<usize>()
            > MAX_OUTPUT_EVENT_ROWS_V1 / 2
    {
        return Err(ZkX509Rfc5280StarkErrorV1::Output);
    }
    Ok(terminals)
}

const ECDSA_SHA256_ALGORITHM_V1: &[u8] = &[
    0x30, 0x0a, 0x06, 0x08, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x04, 0x03, 0x02,
];
const P256_ALGORITHM_V1: &[u8] = &[
    0x30, 0x13, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06, 0x08, 0x2a, 0x86, 0x48,
    0xce, 0x3d, 0x03, 0x01, 0x07,
];
const CERTIFICATE_EXTENSION_OIDS_V1: [&[u8]; 5] = [
    &[0x55, 0x1d, 0x23],
    &[0x55, 0x1d, 0x0e],
    &[0x55, 0x1d, 0x0f],
    &[0x55, 0x1d, 0x13],
    &[0x55, 0x1d, 0x25],
];
const CRL_EXTENSION_OIDS_V1: [&[u8]; 2] = [&[0x55, 0x1d, 0x23], &[0x55, 0x1d, 0x14]];
const NAME_OIDS_V1: [&[u8]; 4] = [
    &[0x55, 0x04, 0x06],
    &[0x55, 0x04, 0x0a],
    &[0x55, 0x04, 0x0b],
    &[0x55, 0x04, 0x03],
];
const EKU_OIDS_V1: [&[u8]; 3] = [
    &[0x2b, 0x06, 0x01, 0x05, 0x05, 0x07, 0x03, 0x02],
    &[0x2b, 0x06, 0x01, 0x04, 0x01, 0x83, 0xb2, 0x03, 0x01, 0x01],
    &[0x2b, 0x06, 0x01, 0x04, 0x01, 0x83, 0xb2, 0x03, 0x01, 0x02],
];
const KEY_USAGE_KEY_CERT_SIGN_V1: u16 = 1 << 5;
const KEY_USAGE_CRL_SIGN_V1: u16 = 1 << 6;

/// One exact byte address read by a semantic predicate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ZkX509Rfc5280SourceCellV1 {
    pub(crate) document: u8,
    pub(crate) address: u16,
    pub(crate) value: u8,
}

/// One fixed-profile byte equality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509Rfc5280FixedByteV1 {
    pub(crate) source: ZkX509Rfc5280SourceCellV1,
    pub(crate) source_node: ZkX509Rfc5280NodeProvenanceV1,
    pub(crate) expected: u8,
    pub(crate) purpose: u16,
    pub(crate) instance: u16,
    pub(crate) variant: u16,
    pub(crate) offset: u16,
    pub(crate) length: u16,
}

/// One two-source equality byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509Rfc5280EqualByteV1 {
    pub(crate) left: ZkX509Rfc5280SourceCellV1,
    pub(crate) right: ZkX509Rfc5280SourceCellV1,
    pub(crate) purpose: u16,
    pub(crate) instance: u16,
    pub(crate) offset: u16,
}

/// Explicit numeric operands for one admitted scalar predicate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509Rfc5280NumericRelationV1 {
    /// Stable relation code fixed by schedule order.
    pub(crate) relation: u16,
    pub(crate) instance: u16,
    /// `left = right + slack`.
    pub(crate) left: u64,
    pub(crate) right: u64,
    pub(crate) slack: u64,
    /// If true, the slack must be nonzero.
    pub(crate) strict: bool,
}

/// Verifier-fixed purpose of one serial comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ZkX509Rfc5280SerialComparisonKindV1 {
    /// The leaf magnitude must differ from one active revoked magnitude.
    LeafNonMembership = 0,
    /// Two adjacent revoked magnitudes must be strictly increasing.
    AdjacentStrictOrder = 1,
}

/// One fixed-width serial comparison, including length and padded octets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509Rfc5280SerialComparisonV1 {
    pub(crate) kind: ZkX509Rfc5280SerialComparisonKindV1,
    pub(crate) left_instance: u16,
    pub(crate) right_instance: u16,
    pub(crate) left: [u8; SERIAL_COMPARISON_WIDTH_V1],
    pub(crate) right: [u8; SERIAL_COMPARISON_WIDTH_V1],
}

/// One canonical DER-backed producer frame for a comparator endpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509Rfc5280SerialSourceV1 {
    pub(crate) logical_id: u16,
    pub(crate) node: ZkX509Rfc5280NodeProvenanceV1,
    pub(crate) frame: [u8; SERIAL_COMPARISON_WIDTH_V1],
    /// Exact DER INTEGER contents, including an optional sign octet.
    pub(crate) encoded_contents: Vec<ZkX509Rfc5280SourceCellV1>,
}

/// Complete prover-side semantic operands. Each vector is committed as a
/// fixed-family range; none is reduced to a host Boolean digest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509Rfc5280SemanticWitnessV1 {
    pub(crate) fixed_bytes: Vec<ZkX509Rfc5280FixedByteV1>,
    pub(crate) equal_bytes: Vec<ZkX509Rfc5280EqualByteV1>,
    pub(crate) decimal_cells: Vec<ZkX509Rfc5280SourceCellV1>,
    pub(crate) calendar_values: Vec<u64>,
    pub(crate) numeric_relations: Vec<ZkX509Rfc5280NumericRelationV1>,
    pub(crate) bit_flags: Vec<(u16, u16, u64, u64)>,
    pub(crate) serial_sources: Vec<ZkX509Rfc5280SerialSourceV1>,
    pub(crate) serial_comparisons: Vec<ZkX509Rfc5280SerialComparisonV1>,
}

fn document_bytes_v1(
    trace: &ZkX509Rfc5280TraceV1,
    document: usize,
) -> Result<Vec<u8>, ZkX509Rfc5280StarkErrorV1> {
    source_documents_v1(trace)
        .nth(document)
        .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?
        .bytes
        .iter()
        .map(|row| u8::try_from(row.value.value.0).map_err(|_| ZkX509Rfc5280StarkErrorV1::Source))
        .collect()
}

fn source_slice_v1(
    trace: &ZkX509Rfc5280TraceV1,
    row: ZkX509Rfc5280NodeProvenanceV1,
    contents_only: bool,
) -> Result<Vec<ZkX509Rfc5280SourceCellV1>, ZkX509Rfc5280StarkErrorV1> {
    let bytes = document_bytes_v1(trace, usize::from(row.document))?;
    let start = if contents_only {
        row.content_start
    } else {
        row.start
    };
    let end = row.content_end;
    if start > end || usize::from(end) > bytes.len() {
        return Err(ZkX509Rfc5280StarkErrorV1::Source);
    }
    (start..end)
        .map(|address| {
            Ok(ZkX509Rfc5280SourceCellV1 {
                document: row.document,
                address,
                value: bytes[usize::from(address)],
            })
        })
        .collect()
}

fn push_fixed_slice_v1(
    output: &mut Vec<ZkX509Rfc5280FixedByteV1>,
    source_node: ZkX509Rfc5280NodeProvenanceV1,
    source: &[ZkX509Rfc5280SourceCellV1],
    expected: &[u8],
    purpose: u16,
    instance: u16,
    variant: u16,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    if source.len() != expected.len() {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    output
        .try_reserve(source.len())
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    let length = u16::try_from(expected.len()).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    for (offset, (source, expected)) in source
        .iter()
        .copied()
        .zip(expected.iter().copied())
        .enumerate()
    {
        output.push(ZkX509Rfc5280FixedByteV1 {
            source,
            source_node,
            expected,
            purpose,
            instance,
            variant,
            offset: u16::try_from(offset).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            length,
        });
    }
    Ok(())
}

fn push_equal_slices_v1(
    output: &mut Vec<ZkX509Rfc5280EqualByteV1>,
    left: &[ZkX509Rfc5280SourceCellV1],
    right: &[ZkX509Rfc5280SourceCellV1],
    purpose: u16,
    instance: u16,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    if left.len() != right.len() {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    output
        .try_reserve(left.len())
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    for (offset, (left, right)) in left.iter().copied().zip(right.iter().copied()).enumerate() {
        output.push(ZkX509Rfc5280EqualByteV1 {
            left,
            right,
            purpose,
            instance,
            offset: u16::try_from(offset).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        });
    }
    Ok(())
}

fn role_nodes_v1(
    trace: &ZkX509Rfc5280TraceV1,
    role: ZkX509Rfc5280GrammarRoleV1,
) -> impl Iterator<Item = ZkX509Rfc5280NodeProvenanceV1> + '_ {
    trace
        .semantic_provenance
        .iter()
        .flat_map(|document| document.nodes.iter())
        .copied()
        .filter(move |row| row.role == role)
}

fn embedded_role_node_v1(
    trace: &ZkX509Rfc5280TraceV1,
    parent_document: usize,
    kind: ZkX509Rfc5280DocumentKindV1,
) -> Result<ZkX509Rfc5280NodeProvenanceV1, ZkX509Rfc5280StarkErrorV1> {
    let mut matches = trace.semantic_provenance.iter().filter(|document| {
        document.kind == kind && usize::from(document.parent_document) == parent_document
    });
    let document = matches.next().ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?;
    if matches.next().is_some() {
        return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
    }
    document
        .nodes
        .first()
        .copied()
        .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)
}

fn serial_frame_v1(
    serial: &[u8],
) -> Result<[u8; SERIAL_COMPARISON_WIDTH_V1], ZkX509Rfc5280StarkErrorV1> {
    if serial.is_empty() || serial.len() > ZK_X509_MAX_SERIAL_BYTES_V1 || serial.first() == Some(&0)
    {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    let mut frame = [0_u8; SERIAL_COMPARISON_WIDTH_V1];
    frame[0] = u8::try_from(serial.len()).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    frame[1..1 + serial.len()].copy_from_slice(serial);
    Ok(frame)
}

fn validate_serial_frame_v1(
    frame: &[u8; SERIAL_COMPARISON_WIDTH_V1],
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    let length = usize::from(frame[0]);
    if !(1..=ZK_X509_MAX_SERIAL_BYTES_V1).contains(&length)
        || frame[1] == 0
        || frame[1 + length..].iter().any(|byte| *byte != 0)
    {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    Ok(())
}

fn validate_serial_comparison_v1(
    comparison: &ZkX509Rfc5280SerialComparisonV1,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    validate_serial_frame_v1(&comparison.left)?;
    validate_serial_frame_v1(&comparison.right)?;
    match comparison.kind {
        ZkX509Rfc5280SerialComparisonKindV1::LeafNonMembership => {
            if comparison.left == comparison.right {
                return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
            }
        }
        ZkX509Rfc5280SerialComparisonKindV1::AdjacentStrictOrder => {
            if comparison.left >= comparison.right {
                return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
            }
        }
    }
    Ok(())
}

fn canonical_serial_comparisons_v1(
    leaf: &[u8],
    revoked_serials: &[Vec<u8>],
) -> Result<Vec<ZkX509Rfc5280SerialComparisonV1>, ZkX509Rfc5280StarkErrorV1> {
    if revoked_serials.len() > ZK_X509_MAX_CRL_ENTRIES_V1 {
        return Err(ZkX509Rfc5280StarkErrorV1::Resource);
    }
    let leaf = serial_frame_v1(leaf)?;
    let revoked = revoked_serials
        .iter()
        .map(|serial| serial_frame_v1(serial))
        .collect::<Result<Vec<_>, _>>()?;
    let mut comparisons = Vec::new();
    comparisons
        .try_reserve_exact(serial_comparison_count_v1(revoked.len()))
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    for (index, right) in revoked.iter().copied().enumerate() {
        let leaf_comparison = ZkX509Rfc5280SerialComparisonV1 {
            kind: ZkX509Rfc5280SerialComparisonKindV1::LeafNonMembership,
            left_instance: 0,
            right_instance: u16::try_from(index + 1)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            left: leaf,
            right,
        };
        validate_serial_comparison_v1(&leaf_comparison)?;
        comparisons.push(leaf_comparison);
        if index != 0 {
            let adjacent_comparison = ZkX509Rfc5280SerialComparisonV1 {
                kind: ZkX509Rfc5280SerialComparisonKindV1::AdjacentStrictOrder,
                left_instance: u16::try_from(index)
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
                right_instance: u16::try_from(index + 1)
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
                left: revoked[index - 1],
                right,
            };
            validate_serial_comparison_v1(&adjacent_comparison)?;
            comparisons.push(adjacent_comparison);
        }
    }
    if comparisons.len() != serial_comparison_count_v1(revoked.len()) {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    Ok(comparisons)
}

fn validate_serial_comparison_manifest_v1(
    leaf: &[u8],
    revoked_serials: &[Vec<u8>],
    comparisons: &[ZkX509Rfc5280SerialComparisonV1],
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    let expected = canonical_serial_comparisons_v1(leaf, revoked_serials)?;
    if comparisons != expected {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    Ok(())
}

fn canonical_serial_source_v1(
    trace: &ZkX509Rfc5280TraceV1,
    logical_id: u16,
) -> Result<ZkX509Rfc5280SerialSourceV1, ZkX509Rfc5280StarkErrorV1> {
    let crl_document = trace.certificates.len();
    let (document, role, role_instance, serial) = if logical_id == 0 {
        (
            0,
            ZkX509Rfc5280GrammarRoleV1::CertificateSerial,
            0,
            trace
                .certificates
                .first()
                .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?
                .serial
                .as_slice(),
        )
    } else {
        let entry = usize::from(logical_id - 1);
        (
            crl_document,
            ZkX509Rfc5280GrammarRoleV1::CrlEntrySerial,
            logical_id - 1,
            trace
                .crl
                .revoked_serials
                .get(entry)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?
                .as_slice(),
        )
    };
    let node = find_role_instance_node_v1(trace, document, role, role_instance)?;
    let frame = serial_frame_v1(serial)?;
    let encoded_contents = source_slice_v1(trace, node, true)?;
    let encoded = encoded_contents
        .iter()
        .map(|cell| cell.value)
        .collect::<Vec<_>>();
    let sign_padding = encoded.len() == serial.len() + 1
        && encoded.first() == Some(&0)
        && encoded.get(1..) == Some(serial);
    if encoded.as_slice() != serial && !sign_padding || sign_padding != (serial[0] & 0x80 != 0) {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    Ok(ZkX509Rfc5280SerialSourceV1 {
        logical_id,
        node,
        frame,
        encoded_contents,
    })
}

fn canonical_serial_sources_v1(
    trace: &ZkX509Rfc5280TraceV1,
    comparisons: &[ZkX509Rfc5280SerialComparisonV1],
) -> Result<Vec<ZkX509Rfc5280SerialSourceV1>, ZkX509Rfc5280StarkErrorV1> {
    let mut sources = Vec::new();
    sources
        .try_reserve_exact(
            comparisons
                .len()
                .checked_mul(2)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?,
        )
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    for comparison in comparisons {
        for logical_id in [comparison.left_instance, comparison.right_instance] {
            sources.push(canonical_serial_source_v1(trace, logical_id)?);
        }
    }
    Ok(sources)
}

fn validate_serial_source_manifest_v1(
    trace: &ZkX509Rfc5280TraceV1,
    comparisons: &[ZkX509Rfc5280SerialComparisonV1],
    sources: &[ZkX509Rfc5280SerialSourceV1],
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    let expected = canonical_serial_sources_v1(trace, comparisons)?;
    if sources != expected {
        return Err(ZkX509Rfc5280StarkErrorV1::Source);
    }
    Ok(())
}

/// Exact DER-table multiplicity for one serial-source node lookup key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ZkX509Rfc5280SerialNodeMultiplicityV1 {
    pub(crate) document: u8,
    pub(crate) node: u16,
    pub(crate) required_multiplicity: u16,
}

/// Compile the pre-challenge byte/node table multiplicities consumed by the
/// serial-source lookup arguments.
pub(crate) fn zk_x509_rfc5280_serial_lookup_multiplicities_v1(
    sources: &[ZkX509Rfc5280SerialSourceV1],
) -> Result<
    (
        Vec<ZkX509Rfc5280SourceMultiplicityV1>,
        Vec<ZkX509Rfc5280SerialNodeMultiplicityV1>,
    ),
    ZkX509Rfc5280StarkErrorV1,
> {
    let mut bytes = Vec::new();
    let mut nodes = Vec::new();
    for source in sources {
        build_zk_x509_rfc5280_serial_source_rows_v1(source)?;
        bytes.extend(source.encoded_contents.iter().copied());
        nodes.push((source.node.document, source.node.node));
    }
    bytes.sort_unstable();
    nodes.sort_unstable();
    let mut byte_multiplicities: Vec<ZkX509Rfc5280SourceMultiplicityV1> = Vec::new();
    for source in bytes {
        if let Some(last) = byte_multiplicities.last_mut()
            && last.source == source
        {
            last.required_multiplicity = last
                .required_multiplicity
                .checked_add(1)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
        } else {
            byte_multiplicities.push(ZkX509Rfc5280SourceMultiplicityV1 {
                source,
                required_multiplicity: 1,
            });
        }
    }
    let mut node_multiplicities: Vec<ZkX509Rfc5280SerialNodeMultiplicityV1> = Vec::new();
    for (document, node) in nodes {
        if let Some(last) = node_multiplicities.last_mut()
            && last.document == document
            && last.node == node
        {
            last.required_multiplicity = last
                .required_multiplicity
                .checked_add(
                    u16::try_from(SERIAL_COMPARISON_WIDTH_V1)
                        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
                )
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
        } else {
            node_multiplicities.push(ZkX509Rfc5280SerialNodeMultiplicityV1 {
                document,
                node,
                required_multiplicity: u16::try_from(SERIAL_COMPARISON_WIDTH_V1)
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            });
        }
    }
    Ok((byte_multiplicities, node_multiplicities))
}

fn push_relation_v1(
    output: &mut Vec<ZkX509Rfc5280NumericRelationV1>,
    relation: u16,
    instance: usize,
    left: u64,
    right: u64,
    strict: bool,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    let slack = left
        .checked_sub(right)
        .filter(|slack| !strict || *slack != 0)
        .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
    output.push(ZkX509Rfc5280NumericRelationV1 {
        relation,
        instance: u16::try_from(instance).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        left,
        right,
        slack,
        strict,
    });
    Ok(())
}

fn parse_decimal_v1(bytes: &[u8]) -> Result<u16, ZkX509Rfc5280StarkErrorV1> {
    bytes.iter().try_fold(0_u16, |value, byte| {
        if !byte.is_ascii_digit() {
            return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
        }
        value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u16::from(*byte - b'0')))
            .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)
    })
}

fn parse_time_cells_v1(
    cells: &[ZkX509Rfc5280SourceCellV1],
    tag: u32,
) -> Result<(u64, Vec<ZkX509Rfc5280SourceCellV1>), ZkX509Rfc5280StarkErrorV1> {
    use time::{Date, Month, PrimitiveDateTime, Time};

    let bytes = cells.iter().map(|cell| cell.value).collect::<Vec<_>>();
    let (year, offset, z_offset) = if tag == 23 {
        if bytes.len() != 13 || bytes[12] != b'Z' {
            return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
        }
        let short = parse_decimal_v1(&bytes[..2])?;
        let year = if short >= 50 {
            1900 + i32::from(short)
        } else {
            2000 + i32::from(short)
        };
        if !(1970..=2049).contains(&year) {
            return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
        }
        (year, 2, 12)
    } else if tag == 24 {
        if bytes.len() != 15 || bytes[14] != b'Z' {
            return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
        }
        let year = i32::from(parse_decimal_v1(&bytes[..4])?);
        if !(2050..=9999).contains(&year) {
            return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
        }
        (year, 4, 14)
    } else {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    };
    let month = Month::try_from(
        u8::try_from(parse_decimal_v1(&bytes[offset..offset + 2])?)
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Semantic)?,
    )
    .map_err(|_| ZkX509Rfc5280StarkErrorV1::Semantic)?;
    let day = u8::try_from(parse_decimal_v1(&bytes[offset + 2..offset + 4])?)
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Semantic)?;
    let hour = u8::try_from(parse_decimal_v1(&bytes[offset + 4..offset + 6])?)
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Semantic)?;
    let minute = u8::try_from(parse_decimal_v1(&bytes[offset + 6..offset + 8])?)
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Semantic)?;
    let second = u8::try_from(parse_decimal_v1(&bytes[offset + 8..offset + 10])?)
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Semantic)?;
    let timestamp = PrimitiveDateTime::new(
        Date::from_calendar_date(year, month, day)
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Semantic)?,
        Time::from_hms(hour, minute, second).map_err(|_| ZkX509Rfc5280StarkErrorV1::Semantic)?,
    )
    .assume_utc()
    .unix_timestamp();
    Ok((
        u64::try_from(timestamp).map_err(|_| ZkX509Rfc5280StarkErrorV1::Semantic)?,
        cells[..z_offset].to_vec(),
    ))
}

fn encode_unsigned_integer_v1(value: u64) -> Vec<u8> {
    let bytes = value.to_be_bytes();
    let first = bytes
        .iter()
        .position(|byte| *byte != 0)
        .unwrap_or(bytes.len() - 1);
    let magnitude = &bytes[first..];
    let mut encoded = Vec::with_capacity(magnitude.len() + 3);
    encoded.push(0x02);
    encoded.push(
        u8::try_from(magnitude.len() + usize::from(magnitude[0] & 0x80 != 0))
            .expect("u64 INTEGER length fits u8"),
    );
    if magnitude[0] & 0x80 != 0 {
        encoded.push(0);
    }
    encoded.extend_from_slice(magnitude);
    encoded
}

fn encode_key_usage_v1(flags: u16) -> Result<Vec<u8>, ZkX509Rfc5280StarkErrorV1> {
    let highest = (0..16)
        .rev()
        .find(|bit| flags & (1_u16 << bit) != 0)
        .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
    let value_len = highest / 8 + 1;
    let unused = 7 - highest % 8;
    let mut value = vec![0_u8; value_len];
    for bit in 0..16 {
        if flags & (1_u16 << bit) != 0 {
            value[bit / 8] |= 0x80 >> (bit % 8);
        }
    }
    let mut encoded = vec![
        0x03,
        u8::try_from(value.len() + 1).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        u8::try_from(unused).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
    ];
    encoded.extend_from_slice(&value);
    Ok(encoded)
}

fn encode_basic_constraints_v1(
    ca: bool,
    path_len: Option<u32>,
) -> Result<Vec<u8>, ZkX509Rfc5280StarkErrorV1> {
    let mut body = Vec::new();
    if ca {
        body.extend_from_slice(&[0x01, 0x01, 0xff]);
    } else if path_len.is_some() {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    if let Some(path_len) = path_len {
        body.extend_from_slice(&encode_unsigned_integer_v1(u64::from(path_len)));
    }
    if body.len() >= 128 {
        return Err(ZkX509Rfc5280StarkErrorV1::Resource);
    }
    let mut encoded = vec![
        0x30,
        u8::try_from(body.len()).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
    ];
    encoded.extend_from_slice(&body);
    Ok(encoded)
}

fn eku_oid_v1(eku: ZkX509DerEkuV1) -> &'static [u8] {
    match eku {
        ZkX509DerEkuV1::ClientAuthentication => EKU_OIDS_V1[0],
        ZkX509DerEkuV1::DocumentSigning => EKU_OIDS_V1[1],
        ZkX509DerEkuV1::WalletIdentity => EKU_OIDS_V1[2],
    }
}

const fn eku_code_v1(eku: ZkX509DerEkuV1) -> u8 {
    match eku {
        ZkX509DerEkuV1::ClientAuthentication => 1,
        ZkX509DerEkuV1::DocumentSigning => 2,
        ZkX509DerEkuV1::WalletIdentity => 3,
    }
}

fn eku_from_code_v1(code: u8) -> Result<ZkX509DerEkuV1, ZkX509Rfc5280StarkErrorV1> {
    match code {
        1 => Ok(ZkX509DerEkuV1::ClientAuthentication),
        2 => Ok(ZkX509DerEkuV1::DocumentSigning),
        3 => Ok(ZkX509DerEkuV1::WalletIdentity),
        _ => Err(ZkX509Rfc5280StarkErrorV1::Shape),
    }
}

fn encode_eku_v1(usages: &[ZkX509DerEkuV1]) -> Result<Vec<u8>, ZkX509Rfc5280StarkErrorV1> {
    if usages.is_empty() {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    let mut body = Vec::new();
    for usage in usages {
        let oid = eku_oid_v1(*usage);
        body.push(0x06);
        body.push(u8::try_from(oid.len()).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?);
        body.extend_from_slice(oid);
    }
    if body.len() >= 128 {
        return Err(ZkX509Rfc5280StarkErrorV1::Resource);
    }
    let mut encoded = vec![
        0x30,
        u8::try_from(body.len()).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
    ];
    encoded.extend_from_slice(&body);
    Ok(encoded)
}

fn find_embedded_inner_role_v1(
    trace: &ZkX509Rfc5280TraceV1,
    parent_document: usize,
    kind: ZkX509Rfc5280DocumentKindV1,
    role: ZkX509Rfc5280GrammarRoleV1,
) -> Result<ZkX509Rfc5280NodeProvenanceV1, ZkX509Rfc5280StarkErrorV1> {
    let document = trace
        .semantic_provenance
        .iter()
        .find(|document| {
            document.kind == kind && usize::from(document.parent_document) == parent_document
        })
        .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?;
    let mut nodes = document
        .nodes
        .iter()
        .copied()
        .filter(|node| node.role == role);
    let node = nodes.next().ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?;
    if nodes.next().is_some() {
        return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
    }
    Ok(node)
}

/// Build every semantic operand and reject malformed or inconsistent paths.
pub(crate) fn build_zk_x509_rfc5280_semantic_witness_v1(
    trace: &ZkX509Rfc5280TraceV1,
) -> Result<ZkX509Rfc5280SemanticWitnessV1, ZkX509Rfc5280StarkErrorV1> {
    validate_zk_x509_rfc5280_provenance_v1(trace)?;
    let depth = trace.certificates.len();
    if !(2..=3).contains(&depth)
        || trace.crl.revoked_serials.len() > ZK_X509_MAX_CRL_ENTRIES_V1
        || trace
            .statement
            .disclosed_attribute_indices
            .iter()
            .any(|index| *index >= 4)
        || trace
            .statement
            .disclosed_attribute_indices
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || trace
            .statement
            .disclosed_attribute_indices
            .iter()
            .any(|index| trace.certificates[0].subject.attributes[usize::from(*index)].is_none())
        || trace.certificates[1].extensions.key_usage & KEY_USAGE_CRL_SIGN_V1 == 0
    {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    let mut witness = ZkX509Rfc5280SemanticWitnessV1 {
        fixed_bytes: Vec::new(),
        equal_bytes: Vec::new(),
        decimal_cells: Vec::new(),
        calendar_values: Vec::new(),
        numeric_relations: Vec::new(),
        bit_flags: Vec::new(),
        serial_sources: Vec::new(),
        serial_comparisons: Vec::new(),
    };

    for row in role_nodes_v1(trace, ZkX509Rfc5280GrammarRoleV1::CertificateOuterAlgorithm)
        .chain(role_nodes_v1(
            trace,
            ZkX509Rfc5280GrammarRoleV1::CertificateTbsAlgorithm,
        ))
        .chain(role_nodes_v1(
            trace,
            ZkX509Rfc5280GrammarRoleV1::CrlOuterAlgorithm,
        ))
        .chain(role_nodes_v1(
            trace,
            ZkX509Rfc5280GrammarRoleV1::CrlTbsAlgorithm,
        ))
    {
        push_fixed_slice_v1(
            &mut witness.fixed_bytes,
            row,
            &source_slice_v1(trace, row, false)?,
            ECDSA_SHA256_ALGORITHM_V1,
            1,
            row.role_instance,
            0,
        )?;
    }
    for row in role_nodes_v1(trace, ZkX509Rfc5280GrammarRoleV1::CertificateSpkiAlgorithm) {
        push_fixed_slice_v1(
            &mut witness.fixed_bytes,
            row,
            &source_slice_v1(trace, row, false)?,
            P256_ALGORITHM_V1,
            2,
            row.document.into(),
            0,
        )?;
    }
    for row in role_nodes_v1(trace, ZkX509Rfc5280GrammarRoleV1::CertificateVersion) {
        push_fixed_slice_v1(
            &mut witness.fixed_bytes,
            row,
            &source_slice_v1(trace, row, true)?,
            &[0x02, 0x01, 0x02],
            3,
            row.document.into(),
            0,
        )?;
    }
    for row in role_nodes_v1(trace, ZkX509Rfc5280GrammarRoleV1::CertificateSignatureValue).chain(
        role_nodes_v1(trace, ZkX509Rfc5280GrammarRoleV1::CrlSignatureValue),
    ) {
        let first = source_slice_v1(trace, row, true)?
            .into_iter()
            .next()
            .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
        push_fixed_slice_v1(
            &mut witness.fixed_bytes,
            row,
            &[first],
            &[0],
            4,
            row.document.into(),
            0,
        )?;
    }
    for row in role_nodes_v1(trace, ZkX509Rfc5280GrammarRoleV1::CertificatePublicKey) {
        let contents = source_slice_v1(trace, row, true)?;
        if contents.len() != 66 {
            return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
        }
        push_fixed_slice_v1(
            &mut witness.fixed_bytes,
            row,
            &contents[..2],
            &[0, 0x04],
            5,
            row.document.into(),
            0,
        )?;
    }
    for row in role_nodes_v1(trace, ZkX509Rfc5280GrammarRoleV1::CertificateExtensionOid) {
        let expected = CERTIFICATE_EXTENSION_OIDS_V1
            .get(usize::from(row.role_instance))
            .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
        push_fixed_slice_v1(
            &mut witness.fixed_bytes,
            row,
            &source_slice_v1(trace, row, true)?,
            expected,
            6,
            row.role_instance,
            row.role_instance,
        )?;
    }
    for row in role_nodes_v1(trace, ZkX509Rfc5280GrammarRoleV1::CrlExtensionOid) {
        let expected = CRL_EXTENSION_OIDS_V1
            .get(usize::from(row.role_instance))
            .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
        push_fixed_slice_v1(
            &mut witness.fixed_bytes,
            row,
            &source_slice_v1(trace, row, true)?,
            expected,
            7,
            row.role_instance,
            row.role_instance,
        )?;
    }
    for row in role_nodes_v1(
        trace,
        ZkX509Rfc5280GrammarRoleV1::CertificateExtensionCritical,
    ) {
        if !matches!(row.role_instance, 2..=4) {
            return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
        }
        push_fixed_slice_v1(
            &mut witness.fixed_bytes,
            row,
            &source_slice_v1(trace, row, true)?,
            &[0xff],
            8,
            row.role_instance,
            0,
        )?;
    }
    if role_nodes_v1(trace, ZkX509Rfc5280GrammarRoleV1::CrlExtensionCritical)
        .next()
        .is_some()
    {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    for row in role_nodes_v1(trace, ZkX509Rfc5280GrammarRoleV1::NameAttributeOid) {
        let source = source_slice_v1(trace, row, true)?;
        let actual = source.iter().map(|cell| cell.value).collect::<Vec<_>>();
        let expected = NAME_OIDS_V1
            .iter()
            .copied()
            .find(|oid| *oid == actual.as_slice())
            .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
        let variant = NAME_OIDS_V1
            .iter()
            .position(|oid| *oid == expected)
            .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
        push_fixed_slice_v1(
            &mut witness.fixed_bytes,
            row,
            &source,
            expected,
            9,
            row.role_instance,
            u16::try_from(variant).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        )?;
    }

    for (index, certificate) in trace.certificates.iter().enumerate() {
        let key_usage_root =
            embedded_role_node_v1(trace, index, ZkX509Rfc5280DocumentKindV1::KeyUsage)?;
        push_fixed_slice_v1(
            &mut witness.fixed_bytes,
            key_usage_root,
            &source_slice_v1(trace, key_usage_root, false)?,
            &encode_key_usage_v1(certificate.extensions.key_usage)?,
            10,
            u16::try_from(index).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            u16::from(index != 0),
        )?;
        let basic_root =
            embedded_role_node_v1(trace, index, ZkX509Rfc5280DocumentKindV1::BasicConstraints)?;
        push_fixed_slice_v1(
            &mut witness.fixed_bytes,
            basic_root,
            &source_slice_v1(trace, basic_root, false)?,
            &encode_basic_constraints_v1(
                certificate.extensions.basic_constraints_ca,
                certificate.extensions.basic_constraints_path_len,
            )?,
            11,
            u16::try_from(index).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            u16::try_from(index).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        )?;
        if let Some(ekus) = certificate.extensions.extended_key_usages.as_deref() {
            let eku_root =
                embedded_role_node_v1(trace, index, ZkX509Rfc5280DocumentKindV1::ExtendedKeyUsage)?;
            push_fixed_slice_v1(
                &mut witness.fixed_bytes,
                eku_root,
                &source_slice_v1(trace, eku_root, false)?,
                &encode_eku_v1(ekus)?,
                12,
                u16::try_from(index).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
                0,
            )?;
        }
    }

    for (index, certificate) in trace.certificates.iter().enumerate() {
        let parent_index = if index + 1 < depth { index + 1 } else { index };
        let issuer =
            find_role_node_v1(trace, index, ZkX509Rfc5280GrammarRoleV1::CertificateIssuer)?;
        let subject = find_role_node_v1(
            trace,
            parent_index,
            ZkX509Rfc5280GrammarRoleV1::CertificateSubject,
        )?;
        push_equal_slices_v1(
            &mut witness.equal_bytes,
            &source_slice_v1(trace, issuer, false)?,
            &source_slice_v1(trace, subject, false)?,
            1,
            u16::try_from(index).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        )?;
        let aki = find_embedded_inner_role_v1(
            trace,
            index,
            ZkX509Rfc5280DocumentKindV1::AuthorityKeyIdentifier,
            ZkX509Rfc5280GrammarRoleV1::EmbeddedAkiIdentifier,
        )?;
        let ski = find_embedded_inner_role_v1(
            trace,
            parent_index,
            ZkX509Rfc5280DocumentKindV1::SubjectKeyIdentifier,
            ZkX509Rfc5280GrammarRoleV1::EmbeddedSki,
        )?;
        push_equal_slices_v1(
            &mut witness.equal_bytes,
            &source_slice_v1(trace, aki, true)?,
            &source_slice_v1(trace, ski, true)?,
            2,
            u16::try_from(index).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        )?;
        if certificate.not_before > trace.statement.presentation_not_before_unix_seconds
            || certificate.not_after < trace.statement.presentation_not_after_unix_seconds
        {
            return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
        }
        push_relation_v1(
            &mut witness.numeric_relations,
            1,
            index,
            trace.statement.presentation_not_before_unix_seconds,
            certificate.not_before,
            false,
        )?;
        push_relation_v1(
            &mut witness.numeric_relations,
            2,
            index,
            certificate.not_after,
            trace.statement.presentation_not_after_unix_seconds,
            false,
        )?;
        if index == 0 {
            if certificate.extensions.basic_constraints_path_len.is_some()
                || certificate.extensions.extended_key_usages.as_deref()
                    != Some(trace.statement.leaf_extended_key_usages.as_slice())
            {
                return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
            }
        } else {
            let subordinate =
                u32::try_from(index - 1).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
            let path_len = certificate
                .extensions
                .basic_constraints_path_len
                .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
            // The closed first-release profile has one canonical path-length
            // encoding per CA slot.  Allowing a larger, semantically
            // equivalent value would require a private-value lookup table and
            // creates needless proof malleability.
            if path_len != subordinate {
                return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
            }
            if certificate.extensions.extended_key_usages.is_some() {
                return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
            }
        }
    }

    let crl_document = depth;
    let crl_issuer = find_role_node_v1(trace, crl_document, ZkX509Rfc5280GrammarRoleV1::CrlIssuer)?;
    let issuer_subject =
        find_role_node_v1(trace, 1, ZkX509Rfc5280GrammarRoleV1::CertificateSubject)?;
    push_equal_slices_v1(
        &mut witness.equal_bytes,
        &source_slice_v1(trace, crl_issuer, false)?,
        &source_slice_v1(trace, issuer_subject, false)?,
        3,
        0,
    )?;
    let crl_aki = find_embedded_inner_role_v1(
        trace,
        crl_document,
        ZkX509Rfc5280DocumentKindV1::AuthorityKeyIdentifier,
        ZkX509Rfc5280GrammarRoleV1::EmbeddedAkiIdentifier,
    )?;
    let issuer_ski = find_embedded_inner_role_v1(
        trace,
        1,
        ZkX509Rfc5280DocumentKindV1::SubjectKeyIdentifier,
        ZkX509Rfc5280GrammarRoleV1::EmbeddedSki,
    )?;
    push_equal_slices_v1(
        &mut witness.equal_bytes,
        &source_slice_v1(trace, crl_aki, true)?,
        &source_slice_v1(trace, issuer_ski, true)?,
        4,
        0,
    )?;
    let crl_number_root =
        embedded_role_node_v1(trace, crl_document, ZkX509Rfc5280DocumentKindV1::CrlNumber)?;
    push_fixed_slice_v1(
        &mut witness.fixed_bytes,
        crl_number_root,
        &source_slice_v1(trace, crl_number_root, false)?,
        &encode_unsigned_integer_v1(trace.statement.crl_number),
        13,
        0,
        0,
    )?;
    push_relation_v1(
        &mut witness.numeric_relations,
        4,
        0,
        trace.statement.presentation_not_before_unix_seconds,
        trace.crl.this_update,
        false,
    )?;
    push_relation_v1(
        &mut witness.numeric_relations,
        5,
        0,
        trace.crl.next_update,
        trace.statement.presentation_not_after_unix_seconds,
        true,
    )?;
    let stale_limit = trace
        .crl
        .this_update
        .checked_add(300)
        .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
    push_relation_v1(
        &mut witness.numeric_relations,
        6,
        0,
        stale_limit,
        trace.statement.presentation_not_after_unix_seconds,
        false,
    )?;

    witness.serial_comparisons =
        canonical_serial_comparisons_v1(&trace.certificates[0].serial, &trace.crl.revoked_serials)?;
    validate_serial_comparison_manifest_v1(
        &trace.certificates[0].serial,
        &trace.crl.revoked_serials,
        &witness.serial_comparisons,
    )?;
    witness.serial_sources = canonical_serial_sources_v1(trace, &witness.serial_comparisons)?;
    validate_serial_source_manifest_v1(
        trace,
        &witness.serial_comparisons,
        &witness.serial_sources,
    )?;
    zk_x509_rfc5280_serial_lookup_multiplicities_v1(&witness.serial_sources)?;

    for row in role_nodes_v1(trace, ZkX509Rfc5280GrammarRoleV1::CertificateNotBefore)
        .chain(role_nodes_v1(
            trace,
            ZkX509Rfc5280GrammarRoleV1::CertificateNotAfter,
        ))
        .chain(role_nodes_v1(
            trace,
            ZkX509Rfc5280GrammarRoleV1::CrlThisUpdate,
        ))
        .chain(role_nodes_v1(
            trace,
            ZkX509Rfc5280GrammarRoleV1::CrlNextUpdate,
        ))
        .chain(role_nodes_v1(
            trace,
            ZkX509Rfc5280GrammarRoleV1::CrlEntryTime,
        ))
    {
        let (calendar, decimal) =
            parse_time_cells_v1(&source_slice_v1(trace, row, true)?, row.tag_number)?;
        witness.calendar_values.push(calendar);
        witness.decimal_cells.extend(decimal);
        if row.role == ZkX509Rfc5280GrammarRoleV1::CrlEntryTime {
            push_relation_v1(
                &mut witness.numeric_relations,
                7,
                usize::from(row.role_instance),
                trace.crl.this_update,
                calendar,
                false,
            )?;
        }
    }
    let expected_calendars = trace
        .certificates
        .iter()
        .map(|certificate| certificate.not_before)
        .chain(
            trace
                .certificates
                .iter()
                .map(|certificate| certificate.not_after),
        )
        .chain([trace.crl.this_update, trace.crl.next_update])
        .collect::<Vec<_>>();
    if witness.calendar_values[..expected_calendars.len()] != expected_calendars {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    if witness
        .fixed_bytes
        .iter()
        .any(|row| row.source.value != row.expected)
        || witness
            .equal_bytes
            .iter()
            .any(|row| row.left.value != row.right.value)
        || witness.numeric_relations.iter().any(|row| {
            row.left != row.right.saturating_add(row.slack) || (row.strict && row.slack == 0)
        })
        || witness
            .bit_flags
            .iter()
            .any(|(_, _, actual, expected)| actual != expected)
    {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    Ok(witness)
}

/// Unique source address and the exact number of semantic consumers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509Rfc5280SourceMultiplicityV1 {
    pub(crate) source: ZkX509Rfc5280SourceCellV1,
    pub(crate) required_multiplicity: u16,
}

pub(crate) fn zk_x509_rfc5280_semantic_source_multiplicities_v1(
    witness: &ZkX509Rfc5280SemanticWitnessV1,
) -> Result<Vec<ZkX509Rfc5280SourceMultiplicityV1>, ZkX509Rfc5280StarkErrorV1> {
    let mut cells = Vec::new();
    cells
        .try_reserve(
            witness
                .fixed_bytes
                .len()
                .saturating_add(witness.equal_bytes.len().saturating_mul(2))
                .saturating_add(witness.decimal_cells.len()),
        )
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    cells.extend(witness.fixed_bytes.iter().map(|row| row.source));
    cells.extend(
        witness
            .equal_bytes
            .iter()
            .flat_map(|row| [row.left, row.right]),
    );
    cells.extend(witness.decimal_cells.iter().copied());
    cells.sort_unstable();
    let mut unique: Vec<ZkX509Rfc5280SourceMultiplicityV1> = Vec::new();
    for source in cells {
        if let Some(last) = unique.last_mut()
            && last.source == source
        {
            last.required_multiplicity = last
                .required_multiplicity
                .checked_add(1)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
        } else {
            unique.push(ZkX509Rfc5280SourceMultiplicityV1 {
                source,
                required_multiplicity: 1,
            });
        }
    }
    if unique
        .windows(2)
        .any(|pair| pair[0].source >= pair[1].source)
        || unique.iter().any(|entry| entry.required_multiplicity == 0)
    {
        return Err(ZkX509Rfc5280StarkErrorV1::Source);
    }
    Ok(unique)
}

/// Compile and validate all private geometry from an owner trace.
pub(crate) fn build_zk_x509_rfc5280_stark_private_shape_v1(
    trace: &ZkX509Rfc5280TraceV1,
) -> Result<ZkX509Rfc5280StarkPrivateShapeV1, ZkX509Rfc5280StarkErrorV1> {
    let semantic = build_zk_x509_rfc5280_semantic_witness_v1(trace)?;
    let public_shape = ZkX509Rfc5280StarkShapeV1::from_statement(&trace.statement)?;
    let profile_byte_table = compile_profile_byte_table_v1(public_shape)?;
    let io = rfc5280_io_witnesses_v1(trace, 0)?;
    let output_rows = io.iter().try_fold(0_usize, |sum, witness| {
        let consumers = witness.declaration.consumers.len();
        if consumers != 1 {
            return Err(ZkX509Rfc5280StarkErrorV1::Output);
        }
        sum.checked_add(
            usize::try_from(witness.declaration.byte_len)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        )
        .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)
    })?;
    let mut shape = ZkX509Rfc5280StarkPrivateShapeV1 {
        chain_depth: u8::try_from(trace.certificates.len())
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        certificate_slot_2_active: F(u64::from(trace.certificates.len() == 3)),
        top_document_count: u8::try_from(trace.documents.len())
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        top_document_lengths: [0; ZK_X509_DER_AIR_MAX_DOCUMENTS_V1],
        top_node_counts: [0; ZK_X509_DER_AIR_MAX_DOCUMENTS_V1],
        embedded_document_count: u8::try_from(trace.embedded_documents.len())
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        embedded_document_lengths: [0; ZK_X509_DER_AIR_MAX_EMBEDDED_DOCUMENTS_V1],
        embedded_node_counts: [0; ZK_X509_DER_AIR_MAX_EMBEDDED_DOCUMENTS_V1],
        crl_entries: u8::try_from(trace.crl.revoked_serials.len())
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        disclosed_attributes: u8::try_from(trace.statement.disclosed_attribute_indices.len())
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        embedded_copy_rows: u32::try_from(trace.embedded_byte_rows.len())
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        // One verifier-owned rule row plus one canonical ordinal row for
        // every non-root DER node.
        grammar_rows: u32::try_from(
            trace
                .semantic_provenance
                .iter()
                .try_fold(0_usize, |rows, document| {
                    rows.checked_add(document.nodes.len().saturating_sub(1))
                })
                .and_then(|rows| rows.checked_add(ZK_X509_RFC5280_GRAMMAR_RULE_COUNT_V1))
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?,
        )
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        fixed_byte_rows: u32::try_from(semantic.fixed_bytes.len())
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        equality_rows: u32::try_from(semantic.equal_bytes.len())
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        decimal_rows: u32::try_from(semantic.decimal_cells.len())
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        calendar_rows: u32::try_from(
            semantic
                .calendar_values
                .len()
                .checked_mul(CALENDAR_COPY_PHASES_V1)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?,
        )
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        relation_rows: u32::try_from(semantic.numeric_relations.len())
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        bit_flag_rows: u32::try_from(semantic.bit_flags.len())
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        serial_source_rows: u32::try_from(
            semantic
                .serial_sources
                .len()
                .checked_mul(SERIAL_COMPARISON_WIDTH_V1)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?,
        )
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        serial_rows: u32::try_from(
            semantic
                .serial_comparisons
                .len()
                .checked_mul(SERIAL_COMPARISON_WIDTH_V1)
                .and_then(|rows| rows.checked_mul(SERIAL_COMPARISON_PHASES_V1))
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?,
        )
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        range_rows: u32::try_from(
            semantic
                .numeric_relations
                .len()
                .checked_mul(8)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?,
        )
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        semantic_source_rows: u32::try_from(
            profile_byte_table
                .len()
                .checked_add(compile_public_numeric_table_v1(public_shape).len())
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?,
        )
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        semantic_consumer_rows: u32::try_from(
            semantic
                .equal_bytes
                .len()
                .checked_add(trace.embedded_byte_rows.len())
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?,
        )
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        output_producer_rows: u32::try_from(output_rows)
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        output_consumer_rows: u32::try_from(output_rows)
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
        io_channels: u16::try_from(io.len()).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
    };
    for (index, document) in trace.documents.iter().enumerate() {
        shape.top_document_lengths[index] =
            u16::try_from(document.bytes.len()).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        shape.top_node_counts[index] =
            u16::try_from(document.nodes.len()).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    }
    for (index, document) in trace.embedded_documents.iter().enumerate() {
        shape.embedded_document_lengths[index] =
            u16::try_from(document.bytes.len()).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        shape.embedded_node_counts[index] =
            u16::try_from(document.nodes.len()).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    }
    shape.validate()?;
    Ok(shape)
}

/// Return the constant public registration shape after validating the private
/// witness geometry.  No private count is copied into the transcript.
pub(crate) fn build_zk_x509_rfc5280_stark_shape_v1(
    trace: &ZkX509Rfc5280TraceV1,
) -> Result<ZkX509Rfc5280StarkShapeV1, ZkX509Rfc5280StarkErrorV1> {
    build_zk_x509_rfc5280_stark_private_shape_v1(trace)?;
    ZkX509Rfc5280StarkShapeV1::from_statement(&trace.statement)
}

pub(crate) type ZkX509Rfc5280StarkBaseRowV1 = [F; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1];
pub(crate) type ZkX509Rfc5280StarkAuxRowV1 = [F; ZK_X509_RFC5280_STARK_AUX_WIDTH_V1];
pub(crate) type ZkX509Rfc5280StarkFixedRowV1 = [F; ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1];

/// One byte in the verifier-owned closed-profile pattern table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ZkX509Rfc5280ProfileByteEntryV1 {
    purpose: u16,
    variant: u16,
    source_role: u16,
    offset: u16,
    length: u16,
    expected: u8,
    contents_only: bool,
    exact_end: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ZkX509Rfc5280PublicNumericEntryV1 {
    relation: u16,
    instance: u16,
    /// Zero for the left operand and one for the right operand.
    side: u8,
    value: u64,
    /// This entry exists only when the private third-certificate selector is
    /// one. The row itself remains verifier-fixed and active in both cases.
    certificate_slot_2_only: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ZkX509Rfc5280OutputTopologyEntryV1 {
    role: ZkX509Rfc5280OutputRoleV1,
    channel: u32,
    producer_endpoint_role: u8,
    consumer_endpoint_role: u8,
    endpoint_instance: u16,
    offset: u32,
}

fn append_profile_pattern_v1(
    entries: &mut Vec<ZkX509Rfc5280ProfileByteEntryV1>,
    purpose: u16,
    variant: u16,
    source_role: ZkX509Rfc5280GrammarRoleV1,
    bytes: &[u8],
    contents_only: bool,
    exact_end: bool,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    let length = u16::try_from(bytes.len()).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    for (offset, expected) in bytes.iter().copied().enumerate() {
        entries.push(ZkX509Rfc5280ProfileByteEntryV1 {
            purpose,
            variant,
            source_role: source_role as u16,
            offset: u16::try_from(offset).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            length,
            expected,
            contents_only,
            exact_end,
        });
    }
    Ok(())
}

fn compile_profile_byte_table_v1(
    shape: ZkX509Rfc5280StarkShapeV1,
) -> Result<Vec<ZkX509Rfc5280ProfileByteEntryV1>, ZkX509Rfc5280StarkErrorV1> {
    shape.validate()?;
    let mut entries = Vec::new();
    for role in [
        ZkX509Rfc5280GrammarRoleV1::CertificateOuterAlgorithm,
        ZkX509Rfc5280GrammarRoleV1::CertificateTbsAlgorithm,
        ZkX509Rfc5280GrammarRoleV1::CrlOuterAlgorithm,
        ZkX509Rfc5280GrammarRoleV1::CrlTbsAlgorithm,
    ] {
        append_profile_pattern_v1(
            &mut entries,
            1,
            0,
            role,
            ECDSA_SHA256_ALGORITHM_V1,
            false,
            true,
        )?;
    }
    append_profile_pattern_v1(
        &mut entries,
        2,
        0,
        ZkX509Rfc5280GrammarRoleV1::CertificateSpkiAlgorithm,
        P256_ALGORITHM_V1,
        false,
        true,
    )?;
    append_profile_pattern_v1(
        &mut entries,
        3,
        0,
        ZkX509Rfc5280GrammarRoleV1::CertificateVersion,
        &[0x02, 0x01, 0x02],
        true,
        true,
    )?;
    for role in [
        ZkX509Rfc5280GrammarRoleV1::CertificateSignatureValue,
        ZkX509Rfc5280GrammarRoleV1::CrlSignatureValue,
    ] {
        append_profile_pattern_v1(&mut entries, 4, 0, role, &[0], true, false)?;
    }
    append_profile_pattern_v1(
        &mut entries,
        5,
        0,
        ZkX509Rfc5280GrammarRoleV1::CertificatePublicKey,
        &[0, 0x04],
        true,
        false,
    )?;
    for (variant, oid) in CERTIFICATE_EXTENSION_OIDS_V1.into_iter().enumerate() {
        append_profile_pattern_v1(
            &mut entries,
            6,
            u16::try_from(variant).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            ZkX509Rfc5280GrammarRoleV1::CertificateExtensionOid,
            oid,
            true,
            true,
        )?;
    }
    for (variant, oid) in CRL_EXTENSION_OIDS_V1.into_iter().enumerate() {
        append_profile_pattern_v1(
            &mut entries,
            7,
            u16::try_from(variant).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            ZkX509Rfc5280GrammarRoleV1::CrlExtensionOid,
            oid,
            true,
            true,
        )?;
    }
    append_profile_pattern_v1(
        &mut entries,
        8,
        0,
        ZkX509Rfc5280GrammarRoleV1::CertificateExtensionCritical,
        &[0xff],
        true,
        true,
    )?;
    for (variant, oid) in NAME_OIDS_V1.into_iter().enumerate() {
        append_profile_pattern_v1(
            &mut entries,
            9,
            u16::try_from(variant).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            ZkX509Rfc5280GrammarRoleV1::NameAttributeOid,
            oid,
            true,
            true,
        )?;
    }
    append_profile_pattern_v1(
        &mut entries,
        10,
        0,
        ZkX509Rfc5280GrammarRoleV1::EmbeddedKeyUsage,
        &encode_key_usage_v1(shape.leaf_key_usage)?,
        false,
        true,
    )?;
    append_profile_pattern_v1(
        &mut entries,
        10,
        1,
        ZkX509Rfc5280GrammarRoleV1::EmbeddedKeyUsage,
        &encode_key_usage_v1(KEY_USAGE_KEY_CERT_SIGN_V1 | KEY_USAGE_CRL_SIGN_V1)?,
        false,
        true,
    )?;
    append_profile_pattern_v1(
        &mut entries,
        11,
        0,
        ZkX509Rfc5280GrammarRoleV1::EmbeddedBasicConstraints,
        &encode_basic_constraints_v1(false, None)?,
        false,
        true,
    )?;
    append_profile_pattern_v1(
        &mut entries,
        11,
        1,
        ZkX509Rfc5280GrammarRoleV1::EmbeddedBasicConstraints,
        &encode_basic_constraints_v1(true, Some(0))?,
        false,
        true,
    )?;
    append_profile_pattern_v1(
        &mut entries,
        11,
        2,
        ZkX509Rfc5280GrammarRoleV1::EmbeddedBasicConstraints,
        &encode_basic_constraints_v1(true, Some(1))?,
        false,
        true,
    )?;
    append_profile_pattern_v1(
        &mut entries,
        12,
        0,
        ZkX509Rfc5280GrammarRoleV1::EmbeddedEku,
        &encode_eku_v1(&shape.leaf_extended_key_usages()?)?,
        false,
        true,
    )?;
    append_profile_pattern_v1(
        &mut entries,
        13,
        0,
        ZkX509Rfc5280GrammarRoleV1::EmbeddedCrlNumber,
        &encode_unsigned_integer_v1(shape.crl_number),
        false,
        true,
    )?;
    if entries.len() > FIXED_SEMANTIC_SOURCE_ROWS_V1 {
        return Err(ZkX509Rfc5280StarkErrorV1::Resource);
    }
    Ok(entries)
}

fn compile_public_numeric_table_v1(
    shape: ZkX509Rfc5280StarkShapeV1,
) -> Vec<ZkX509Rfc5280PublicNumericEntryV1> {
    let mut entries = Vec::with_capacity(9);
    for instance in 0_u16..3 {
        let certificate_slot_2_only = instance == 2;
        entries.push(ZkX509Rfc5280PublicNumericEntryV1 {
            relation: 1,
            instance,
            side: 0,
            value: shape.presentation_not_before_unix_seconds,
            certificate_slot_2_only,
        });
        entries.push(ZkX509Rfc5280PublicNumericEntryV1 {
            relation: 2,
            instance,
            side: 1,
            value: shape.presentation_not_after_unix_seconds,
            certificate_slot_2_only,
        });
    }
    entries.extend([
        ZkX509Rfc5280PublicNumericEntryV1 {
            relation: 4,
            instance: 0,
            side: 0,
            value: shape.presentation_not_before_unix_seconds,
            certificate_slot_2_only: false,
        },
        ZkX509Rfc5280PublicNumericEntryV1 {
            relation: 5,
            instance: 0,
            side: 1,
            value: shape.presentation_not_after_unix_seconds,
            certificate_slot_2_only: false,
        },
        ZkX509Rfc5280PublicNumericEntryV1 {
            relation: 6,
            instance: 0,
            side: 1,
            value: shape.presentation_not_after_unix_seconds,
            certificate_slot_2_only: false,
        },
    ]);
    entries
}

fn compile_output_topology_v1(
    shape: ZkX509Rfc5280StarkShapeV1,
) -> Result<Vec<ZkX509Rfc5280OutputTopologyEntryV1>, ZkX509Rfc5280StarkErrorV1> {
    const SPKI_DER_BYTES: usize = 91;
    const SIGNATURE_DER_BYTES: usize = 72;

    let strict_der = u8::try_from(endpoint_role_code_v1(ZkX509IoSegmentRoleV1::StrictDer)?)
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Output)?;
    let mut channels = Vec::new();
    let mut push_channel = |role: ZkX509Rfc5280OutputRoleV1,
                            consumer: ZkX509IoSegmentRoleV1,
                            byte_len: usize|
     -> Result<(), ZkX509Rfc5280StarkErrorV1> {
        let channel =
            u32::try_from(channels.len()).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        let consumer_endpoint_role = u8::try_from(endpoint_role_code_v1(consumer)?)
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Output)?;
        channels.push((role, channel, consumer_endpoint_role, byte_len));
        Ok(())
    };

    for _ in 0..3 {
        push_channel(
            ZkX509Rfc5280OutputRoleV1::Projection,
            ZkX509IoSegmentRoleV1::Projection,
            SPKI_DER_BYTES,
        )?;
    }
    for byte_len in [8, ZK_X509_MAX_SERIAL_BYTES_V1] {
        push_channel(
            ZkX509Rfc5280OutputRoleV1::Projection,
            ZkX509IoSegmentRoleV1::Projection,
            byte_len,
        )?;
    }
    for _ in 0..shape.disclosed_attribute_count {
        for byte_len in [8, ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1] {
            push_channel(
                ZkX509Rfc5280OutputRoleV1::Projection,
                ZkX509IoSegmentRoleV1::Projection,
                byte_len,
            )?;
        }
    }
    for _ in 0..3 {
        for byte_len in [ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1, 8] {
            push_channel(
                ZkX509Rfc5280OutputRoleV1::CertificateTbsSha,
                ZkX509IoSegmentRoleV1::Sha256,
                byte_len,
            )?;
        }
    }
    push_channel(
        ZkX509Rfc5280OutputRoleV1::CertificateSlotActive,
        ZkX509IoSegmentRoleV1::P256,
        1,
    )?;
    for _ in 0..3 {
        for byte_len in [SIGNATURE_DER_BYTES, 8] {
            push_channel(
                ZkX509Rfc5280OutputRoleV1::P256Signature,
                ZkX509IoSegmentRoleV1::P256,
                byte_len,
            )?;
        }
        push_channel(
            ZkX509Rfc5280OutputRoleV1::P256PublicKey,
            ZkX509IoSegmentRoleV1::P256,
            ZK_X509_UNCOMPRESSED_P256_BYTES_V1,
        )?;
    }
    for byte_len in [ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1, 8] {
        push_channel(
            ZkX509Rfc5280OutputRoleV1::CrlTbsP256Message,
            ZkX509IoSegmentRoleV1::Sha256,
            byte_len,
        )?;
    }
    for byte_len in [ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1, 8] {
        push_channel(
            ZkX509Rfc5280OutputRoleV1::CrlCommitment,
            ZkX509IoSegmentRoleV1::Sha256,
            byte_len,
        )?;
    }
    for byte_len in [SIGNATURE_DER_BYTES, 8] {
        push_channel(
            ZkX509Rfc5280OutputRoleV1::P256Signature,
            ZkX509IoSegmentRoleV1::P256,
            byte_len,
        )?;
    }
    for _ in 0..2 {
        push_channel(
            ZkX509Rfc5280OutputRoleV1::P256PublicKey,
            ZkX509IoSegmentRoleV1::P256,
            ZK_X509_UNCOMPRESSED_P256_BYTES_V1,
        )?;
    }
    push_channel(
        ZkX509Rfc5280OutputRoleV1::IssuerSpkiSha,
        ZkX509IoSegmentRoleV1::Sha256,
        SPKI_DER_BYTES,
    )?;
    push_channel(
        ZkX509Rfc5280OutputRoleV1::GovernedTrustAnchor,
        ZkX509IoSegmentRoleV1::CaAccumulator,
        SPKI_DER_BYTES,
    )?;

    let total_rows = channels.iter().try_fold(0_usize, |sum, channel| {
        sum.checked_add(channel.3)
            .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)
    })?;
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(total_rows)
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    for (role, channel, consumer_endpoint_role, byte_len) in channels {
        for offset in 0..byte_len {
            entries.push(ZkX509Rfc5280OutputTopologyEntryV1 {
                role,
                channel,
                producer_endpoint_role: strict_der,
                consumer_endpoint_role,
                endpoint_instance: 0,
                offset: u32::try_from(offset).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            });
        }
    }
    if entries.len() > FIXED_OUTPUT_ROWS_PER_SIDE_V1 {
        return Err(ZkX509Rfc5280StarkErrorV1::Resource);
    }
    Ok(entries)
}

/// Verifier-owned family ranges. It stores only O(1) boundaries, never
/// `2^18` fixed rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509Rfc5280StarkFixedScheduleV1 {
    pub(crate) shape: ZkX509Rfc5280StarkShapeV1,
    counts: [usize; FAMILY_COUNT_V1],
    starts: [usize; FAMILY_COUNT_V1],
    profile_byte_table: Vec<ZkX509Rfc5280ProfileByteEntryV1>,
    public_numeric_table: Vec<ZkX509Rfc5280PublicNumericEntryV1>,
    output_topology: Vec<ZkX509Rfc5280OutputTopologyEntryV1>,
}

pub(crate) fn compile_zk_x509_rfc5280_stark_fixed_schedule_v1(
    shape: ZkX509Rfc5280StarkShapeV1,
) -> Result<ZkX509Rfc5280StarkFixedScheduleV1, ZkX509Rfc5280StarkErrorV1> {
    shape.validate()?;
    let counts = shape.family_counts();
    let mut starts = [0_usize; FAMILY_COUNT_V1];
    let mut cursor = 0_usize;
    for family in 0..FAMILY_COUNT_V1 {
        starts[family] = cursor;
        cursor = cursor
            .checked_add(counts[family])
            .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
    }
    if cursor != ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 {
        return Err(ZkX509Rfc5280StarkErrorV1::Shape);
    }
    let profile_byte_table = compile_profile_byte_table_v1(shape)?;
    let public_numeric_table = compile_public_numeric_table_v1(shape);
    let output_topology = compile_output_topology_v1(shape)?;
    Ok(ZkX509Rfc5280StarkFixedScheduleV1 {
        shape,
        counts,
        starts,
        profile_byte_table,
        public_numeric_table,
        output_topology,
    })
}

impl ZkX509Rfc5280StarkFixedScheduleV1 {
    pub(crate) fn family_and_ordinal(
        &self,
        row: usize,
    ) -> Result<(ZkX509Rfc5280StarkFamilyV1, usize), ZkX509Rfc5280StarkErrorV1> {
        if row >= ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 {
            return Err(ZkX509Rfc5280StarkErrorV1::Shape);
        }
        const FAMILIES: [ZkX509Rfc5280StarkFamilyV1; FAMILY_COUNT_V1] = [
            ZkX509Rfc5280StarkFamilyV1::SourceByte,
            ZkX509Rfc5280StarkFamilyV1::SourceNode,
            ZkX509Rfc5280StarkFamilyV1::EmbeddedCopy,
            ZkX509Rfc5280StarkFamilyV1::Grammar,
            ZkX509Rfc5280StarkFamilyV1::FixedByte,
            ZkX509Rfc5280StarkFamilyV1::EqualByte,
            ZkX509Rfc5280StarkFamilyV1::Decimal,
            ZkX509Rfc5280StarkFamilyV1::Calendar,
            ZkX509Rfc5280StarkFamilyV1::Relation,
            ZkX509Rfc5280StarkFamilyV1::BitFlags,
            ZkX509Rfc5280StarkFamilyV1::SerialSource,
            ZkX509Rfc5280StarkFamilyV1::SerialCompare,
            ZkX509Rfc5280StarkFamilyV1::RangeByte,
            ZkX509Rfc5280StarkFamilyV1::SemanticSource,
            ZkX509Rfc5280StarkFamilyV1::SemanticConsumer,
            ZkX509Rfc5280StarkFamilyV1::OutputProducer,
            ZkX509Rfc5280StarkFamilyV1::OutputConsumer,
            ZkX509Rfc5280StarkFamilyV1::Padding,
        ];
        for (index, family) in FAMILIES.into_iter().enumerate() {
            let start = self.starts[index];
            let end = start
                .checked_add(self.counts[index])
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
            if (start..end).contains(&row) {
                return Ok((family, row - start));
            }
        }
        Err(ZkX509Rfc5280StarkErrorV1::Shape)
    }

    /// Reconstruct every selector and expected cell from the row ordinal and
    /// public statement projection. No committed witness value is accepted
    /// as an input.
    pub(crate) fn fixed_row(
        &self,
        row: usize,
    ) -> Result<ZkX509Rfc5280StarkFixedRowV1, ZkX509Rfc5280StarkErrorV1> {
        let (family, ordinal) = self.family_and_ordinal(row)?;
        let mut fixed = [F::ZERO; ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1];
        fixed[family as usize] = F::ONE;
        fixed[FIX_GLOBAL_FIRST] = F(u64::from(row == 0));
        fixed[FIX_GLOBAL_LAST] = F(u64::from(row + 1 == ZK_X509_RFC5280_STARK_TRACE_SIZE_V1));
        fixed[FIX_CONTINUE] = F::ONE.sub(fixed[FIX_GLOBAL_LAST]);
        let (local_first, local_last) = match family {
            ZkX509Rfc5280StarkFamilyV1::SerialSource => {
                let group_size = SERIAL_COMPARISON_WIDTH_V1;
                (
                    ordinal % group_size == 0,
                    ordinal % group_size + 1 == group_size,
                )
            }
            ZkX509Rfc5280StarkFamilyV1::SerialCompare => {
                let phase = ordinal % SERIAL_COMPARISON_PHASES_V1;
                let logical_ordinal = ordinal / SERIAL_COMPARISON_PHASES_V1;
                let logical_offset = logical_ordinal % SERIAL_COMPARISON_WIDTH_V1;
                (
                    phase + 1 == SERIAL_COMPARISON_PHASES_V1 && logical_offset == 0,
                    phase + 1 == SERIAL_COMPARISON_PHASES_V1
                        && logical_offset + 1 == SERIAL_COMPARISON_WIDTH_V1,
                )
            }
            ZkX509Rfc5280StarkFamilyV1::RangeByte => (ordinal % 8 == 0, ordinal % 8 + 1 == 8),
            _ => (true, true),
        };
        fixed[FIX_LOCAL_FIRST] = F(u64::from(local_first));
        fixed[FIX_LOCAL_LAST] = F(u64::from(local_last));
        fixed[FIX_ACTIVATION_CONTINUE] = F(u64::from(match family {
            ZkX509Rfc5280StarkFamilyV1::SourceByte if ordinal < MAX_TOP_LEVEL_SOURCE_BYTES_V1 => {
                ordinal % ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1 + 1
                    != ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1
            }
            ZkX509Rfc5280StarkFamilyV1::SourceNode => ordinal % 2_048 + 1 != 2_048,
            _ => ordinal + 1 != self.counts[family as usize],
        }));
        if family == ZkX509Rfc5280StarkFamilyV1::SourceByte
            && ordinal < MAX_TOP_LEVEL_SOURCE_BYTES_V1
        {
            let document = ordinal / ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1;
            let address = ordinal % ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1;
            fixed[FIX_EXPECTED] =
                F(u64::try_from(document).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?);
            fixed[FIX_EXPECTED + 1] =
                F(u64::try_from(address).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?);
            fixed[FIX_ADDRESS_FIXED] = F::ONE;
            fixed[FIX_DOCUMENT_FIXED] = F::ONE;
            fixed[FIX_REQUIRED_ACTIVE] =
                F(u64::from(address == 0 && matches!(document, 0 | 1 | 2)));
            // With contiguous strict-DER document identifiers, slot three is
            // present exactly for a depth-three chain (it carries the CRL).
            // A depth-two chain has only documents 0, 1, and 2.
            fixed[FIX_CERT2_SLOT_FIRST] = F(u64::from(document == 3 && address == 0));
        }
        if family == ZkX509Rfc5280StarkFamilyV1::SourceNode {
            let document = ordinal / 2_048;
            let node = ordinal % 2_048;
            fixed[FIX_EXPECTED] =
                F(u64::try_from(document).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?);
            fixed[FIX_EXPECTED + 1] =
                F(u64::try_from(node).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?);
            fixed[FIX_ADDRESS_FIXED] = F::ONE;
            fixed[FIX_DOCUMENT_FIXED] = F::ONE;
            fixed[FIX_REQUIRED_ACTIVE] = F(u64::from(node == 0 && matches!(document, 0 | 1 | 2)));
            fixed[FIX_EXPECTED + 2] = F(u64::from(node == 0));
            fixed[FIX_EXPECTED + 3] = F(u64::from(node == 0 && matches!(document, 0 | 1)));
            fixed[FIX_EXPECTED + 4] = F(u64::from(node == 0 && document == 2));
            fixed[FIX_EXPECTED + 5] = F(u64::from(node == 0 && document == 3));
            fixed[FIX_EXPECTED + 6] = F(u64::from(node == 0 && document >= 4));
        }
        if family == ZkX509Rfc5280StarkFamilyV1::Grammar {
            if let Some(rule) = ZK_X509_RFC5280_GRAMMAR_RULES_V1.get(ordinal).copied() {
                fixed[FIX_EXPECTED..FIX_EXPECTED + 10]
                    .copy_from_slice(&grammar_rule_expected_cells_v1(rule));
                fixed[FIX_REQUIRED_ACTIVE] = F::ONE;
                fixed[FIX_GRAMMAR_RULE] = F::ONE;
                fixed[FIX_GRAMMAR_ORDINAL_FIRST] =
                    F(u64::from(profile_role_required_v1(rule.child_role)));
            } else if ordinal == ZK_X509_RFC5280_GRAMMAR_RULE_COUNT_V1 {
                fixed[FIX_REQUIRED_ACTIVE] = F::ONE;
                fixed[FIX_GRAMMAR_ORDINAL_FIRST] = F::ONE;
            }
        }
        if family == ZkX509Rfc5280StarkFamilyV1::SerialSource {
            let (logical_id, offset, role, role_instance) = serial_source_descriptor_v1(ordinal)?;
            fixed[FIX_EXPECTED] = F(u64::from(logical_id));
            fixed[FIX_EXPECTED + 1] = F(u64::from(offset));
            fixed[FIX_EXPECTED + 2] = F(role as u64);
            fixed[FIX_EXPECTED + 3] = F(u64::from(role_instance));
            fixed[FIX_EXPECTED + 4] = F(u64::from(usize::from(offset) == 1));
        }
        if family == ZkX509Rfc5280StarkFamilyV1::SerialCompare {
            let logical_ordinal = ordinal / SERIAL_COMPARISON_PHASES_V1;
            let comparison = logical_ordinal / SERIAL_COMPARISON_WIDTH_V1;
            let offset = logical_ordinal % SERIAL_COMPARISON_WIDTH_V1;
            let (kind, left_instance, right_instance) =
                serial_comparison_descriptor_v1(comparison)?;
            // These cells are derived solely from the fixed maximum layout
            // and row ordinal. Private activity flags select an exact prefix.
            fixed[FIX_EXPECTED] = F(kind as u64);
            fixed[FIX_EXPECTED + 1] = F(u64::from(left_instance));
            fixed[FIX_EXPECTED + 2] = F(u64::from(right_instance));
            fixed[FIX_EXPECTED + 3] =
                F(u64::try_from(offset).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?);
            fixed[FIX_EXPECTED + 4] = F(u64::from(offset == 1));
        }
        if family == ZkX509Rfc5280StarkFamilyV1::SemanticSource
            && let Some(entry) = self.profile_byte_table.get(ordinal).copied()
        {
            fixed[FIX_EXPECTED] = F(u64::from(entry.purpose));
            fixed[FIX_EXPECTED + 1] = F(u64::from(entry.variant));
            fixed[FIX_EXPECTED + 2] = F(u64::from(entry.source_role));
            fixed[FIX_EXPECTED + 3] = F(u64::from(entry.offset));
            fixed[FIX_EXPECTED + 4] = F(u64::from(entry.length));
            fixed[FIX_EXPECTED + 5] = F(u64::from(entry.expected));
            fixed[FIX_EXPECTED + 6] = F(u64::from(entry.contents_only));
            fixed[FIX_EXPECTED + 7] = F(u64::from(entry.exact_end));
            fixed[FIX_EXPECTED + 8] = F::ONE;
            fixed[FIX_EXPECTED + 9] = F::ONE;
            fixed[FIX_REQUIRED_ACTIVE] = F::ONE;
        } else if family == ZkX509Rfc5280StarkFamilyV1::SemanticSource
            && let Some(entry) = ordinal
                .checked_sub(self.profile_byte_table.len())
                .and_then(|ordinal| self.public_numeric_table.get(ordinal))
                .copied()
        {
            fixed[FIX_EXPECTED] = F(u64::from(entry.relation));
            fixed[FIX_EXPECTED + 1] = F(u64::from(entry.instance));
            fixed[FIX_EXPECTED + 2] = F(u64::from(entry.side));
            fixed[FIX_EXPECTED + 3] = F(entry.value);
            fixed[FIX_EXPECTED + 4] = F(u64::from(entry.certificate_slot_2_only));
            fixed[FIX_EXPECTED + 8] = F(2);
            fixed[FIX_REQUIRED_ACTIVE] = F::ONE;
        }
        if matches!(
            family,
            ZkX509Rfc5280StarkFamilyV1::OutputProducer | ZkX509Rfc5280StarkFamilyV1::OutputConsumer
        ) && let Some(entry) = self.output_topology.get(ordinal).copied()
        {
            let consumer = family == ZkX509Rfc5280StarkFamilyV1::OutputConsumer;
            fixed[FIX_EXPECTED] = F(entry.role as u64);
            fixed[FIX_EXPECTED + 1] = F(u64::from(entry.channel));
            fixed[FIX_EXPECTED + 2] = F(u64::from(if consumer {
                entry.consumer_endpoint_role
            } else {
                entry.producer_endpoint_role
            }));
            fixed[FIX_EXPECTED + 3] = F(u64::from(entry.endpoint_instance));
            fixed[FIX_EXPECTED + 4] = F(u64::from(entry.offset));
            fixed[FIX_EXPECTED + 5] = F(u64::from(!consumer));
            fixed[FIX_REQUIRED_ACTIVE] = F::ONE;
            fixed[output_role_fixed_selector_column_v1(
                output_role_index_v1(entry.role),
                consumer,
            )] = F::ONE;
        }
        let active_family = |candidate| F(u64::from(family == candidate));
        fixed[FIX_GRAMMAR_RULE_TABLE] =
            active_family(ZkX509Rfc5280StarkFamilyV1::Grammar).mul(fixed[FIX_GRAMMAR_RULE]);
        fixed[FIX_GRAMMAR_ORDINAL_TABLE] = active_family(ZkX509Rfc5280StarkFamilyV1::Grammar)
            .mul(F::ONE.sub(fixed[FIX_GRAMMAR_RULE]));
        fixed[FIX_SOURCE_NODE_NON_ROOT] = active_family(ZkX509Rfc5280StarkFamilyV1::SourceNode)
            .mul(F::ONE.sub(fixed[FIX_EXPECTED + 2]));
        fixed[FIX_PROFILE_TABLE] =
            active_family(ZkX509Rfc5280StarkFamilyV1::SemanticSource).mul(fixed[FIX_EXPECTED + 9]);
        fixed[FIX_SERIAL_SOURCE_FIRST] =
            active_family(ZkX509Rfc5280StarkFamilyV1::SerialSource).mul(fixed[FIX_LOCAL_FIRST]);
        fixed[FIX_SERIAL_SOURCE_INTERIOR] = active_family(ZkX509Rfc5280StarkFamilyV1::SerialSource)
            .mul(F::ONE.sub(fixed[FIX_LOCAL_FIRST]))
            .mul(F::ONE.sub(fixed[FIX_LOCAL_LAST]));
        fixed[FIX_SERIAL_SOURCE_NOT_FIRST] =
            active_family(ZkX509Rfc5280StarkFamilyV1::SerialSource)
                .mul(F::ONE.sub(fixed[FIX_LOCAL_FIRST]));
        fixed[FIX_SERIAL_SOURCE_FIRST_PAYLOAD] =
            active_family(ZkX509Rfc5280StarkFamilyV1::SerialSource).mul(fixed[FIX_EXPECTED + 4]);
        if family == ZkX509Rfc5280StarkFamilyV1::SerialCompare {
            let phase = ordinal % SERIAL_COMPARISON_PHASES_V1;
            fixed[FIX_SERIAL_COMPARE_PHASE_LEFT] = F(u64::from(phase == 0));
            fixed[FIX_SERIAL_COMPARE_PHASE_RIGHT] =
                F(u64::from(phase + 1 == SERIAL_COMPARISON_PHASES_V1));
            fixed[FIX_SERIAL_COMPARE_FIRST] =
                fixed[FIX_SERIAL_COMPARE_PHASE_RIGHT].mul(fixed[FIX_LOCAL_FIRST]);
            fixed[FIX_SERIAL_COMPARE_LAST] =
                fixed[FIX_SERIAL_COMPARE_PHASE_RIGHT].mul(fixed[FIX_LOCAL_LAST]);
            fixed[FIX_SERIAL_COMPARE_INTERIOR] = fixed[FIX_SERIAL_COMPARE_PHASE_RIGHT]
                .mul(F::ONE.sub(fixed[FIX_LOCAL_FIRST]))
                .mul(F::ONE.sub(fixed[FIX_LOCAL_LAST]));
            fixed[FIX_SERIAL_COMPARE_NOT_FIRST] =
                fixed[FIX_SERIAL_COMPARE_PHASE_RIGHT].mul(F::ONE.sub(fixed[FIX_LOCAL_FIRST]));
            fixed[FIX_SERIAL_COMPARE_FIRST_PAYLOAD] =
                fixed[FIX_SERIAL_COMPARE_PHASE_RIGHT].mul(fixed[FIX_EXPECTED + 4]);
        }
        if family == ZkX509Rfc5280StarkFamilyV1::Calendar {
            fixed[FIX_CALENDAR_PHASES + ordinal % CALENDAR_COPY_PHASES_V1] = F::ONE;
        }
        fixed[FIX_RANGE_TERMINAL] =
            active_family(ZkX509Rfc5280StarkFamilyV1::RangeByte).mul(fixed[FIX_LOCAL_LAST]);
        Ok(fixed)
    }

    /// Build one verifier-preprocessed column at a time.
    pub(crate) fn fixed_column(&self, column: usize) -> Result<Vec<F>, ZkX509Rfc5280StarkErrorV1> {
        if column >= ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1 {
            return Err(ZkX509Rfc5280StarkErrorV1::Shape);
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(ZK_X509_RFC5280_STARK_TRACE_SIZE_V1)
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        for row in 0..ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 {
            values.push(self.fixed_row(row)?[column]);
        }
        Ok(values)
    }
}

/// Generic one-column replay helper for base and auxiliary providers.
///
/// At log19 this allocates about 4 MiB per field column and never constructs
/// the full base+aux+fixed matrix.
pub(crate) fn build_zk_x509_rfc5280_stark_column_v1(
    width: usize,
    column: usize,
    mut cell: impl FnMut(usize, usize) -> Result<F, ZkX509Rfc5280StarkErrorV1>,
) -> Result<Vec<F>, ZkX509Rfc5280StarkErrorV1> {
    if column >= width {
        return Err(ZkX509Rfc5280StarkErrorV1::Shape);
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(ZK_X509_RFC5280_STARK_TRACE_SIZE_V1)
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    for row in 0..ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 {
        values.push(cell(row, column)?);
    }
    Ok(values)
}

const BASE_VALUE: usize = 0;
const BASE_BYTE_BITS: usize = 1;
const BASE_A: usize = 9;
const BASE_B: usize = 10;
const BASE_C: usize = 11;
const BASE_D: usize = 12;
const BASE_E: usize = 13;
const BASE_F: usize = 14;
const BASE_G: usize = 15;
const BASE_H: usize = 16;
const BASE_DOCUMENT: usize = 17;
const BASE_ADDRESS: usize = 18;
const BASE_NODE: usize = 19;
const BASE_PARENT: usize = 20;
const BASE_CHILD: usize = 21;
const BASE_START: usize = 22;
const BASE_CONTENT_START: usize = 23;
const BASE_CONTENT_END: usize = 24;
const BASE_DEPTH: usize = 25;
const BASE_TAG_CLASS: usize = 26;
const BASE_CONSTRUCTED: usize = 27;
const BASE_TAG_NUMBER: usize = 28;
const BASE_ROLE: usize = 29;
const BASE_INSTANCE: usize = 30;
const BASE_OFFSET: usize = 31;
const BASE_ENDPOINT_ROLE: usize = 32;
const BASE_ENDPOINT_INSTANCE: usize = 33;
const BASE_IS_WRITE: usize = 34;
const BASE_STRICT: usize = 35;
const BASE_EQUAL: usize = 36;
const BASE_INVERSE: usize = 37;
const BASE_STATE_BEFORE: usize = 38;
const BASE_STATE_AFTER: usize = 39;
const BASE_SMALL_BITS: usize = 40;
const BASE_ACTIVE: usize = 64;
const BASE_CERT2_ACTIVE: usize = 65;
const GRAMMAR_CHILD_ORDINAL_BITS: usize = 66;
const GRAMMAR_CHILD_COUNT_BITS: usize = 82;
const BASE_GRAMMAR_ORDINAL: usize = 98;
const BASE_EXPECTED_ROOT_KIND: usize = 99;
const BASE_ORDINAL_NEXT_ACTIVE: usize = 100;
const BASE_ORDINAL_EQUAL_CONTINUE: usize = 101;
const BASE_PROFILE_TABLE_ACTIVE: usize = 102;
const BASE_PROFILE_TABLE_MULTIPLICITY: usize = 103;
const BASE_PROFILE_TOPOLOGY_QUERY_ACTIVE: usize = 104;
const BASE_SERIAL_BYTE_QUERY_ACTIVE: usize = 105;
const BASE_SERIAL_BYTE_QUERY_VALUE: usize = 106;
const BASE_COPY_SOURCE_ACTIVE: usize = 107;
const BASE_COPY_CONSUMER_ACTIVE: usize = 108;
const BASE_COPY_DOMAIN: usize = 109;
const BASE_COPY_KEY_1: usize = 110;
const BASE_COPY_KEY_2: usize = 111;
const BASE_COPY_VALUE: usize = 112;

const _: () = assert!(GRAMMAR_CHILD_COUNT_BITS + 16 == BASE_GRAMMAR_ORDINAL);
const _: () = assert!(BASE_COPY_VALUE + 1 == ZK_X509_RFC5280_STARK_BASE_WIDTH_V1);

/// Stamp the private depth selector into materialized rows. Row activity is
/// supplied by each family builder; the selector is carried across the whole
/// aggregate and constrained constant by the AIR.
pub(crate) fn bind_zk_x509_rfc5280_private_selectors_v1(
    rows: &mut [ZkX509Rfc5280StarkBaseRowV1],
    private_shape: &ZkX509Rfc5280StarkPrivateShapeV1,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    private_shape.validate()?;
    for row in rows {
        row[BASE_CERT2_ACTIVE] = private_shape.certificate_slot_2_active;
    }
    Ok(())
}

const SERIAL_LESS: usize = BASE_D;
const SERIAL_ORDER_BEFORE: usize = BASE_E;
const SERIAL_ORDER_AFTER: usize = BASE_F;
const SERIAL_LEFT_LENGTH: usize = BASE_G;
const SERIAL_RIGHT_LENGTH: usize = BASE_H;
const SERIAL_LEFT_ACTIVE: usize = BASE_DOCUMENT;
const SERIAL_RIGHT_ACTIVE: usize = BASE_ADDRESS;
const SERIAL_LEFT_COUNT_BEFORE: usize = BASE_NODE;
const SERIAL_LEFT_COUNT_AFTER: usize = BASE_PARENT;
const SERIAL_RIGHT_COUNT_BEFORE: usize = BASE_CHILD;
const SERIAL_RIGHT_COUNT_AFTER: usize = BASE_START;
const SERIAL_LEFT_FIRST_INVERSE: usize = BASE_CONTENT_START;
const SERIAL_RIGHT_FIRST_INVERSE: usize = BASE_CONTENT_END;
const SERIAL_LEFT_BITS: usize = BASE_SMALL_BITS;
const SERIAL_RIGHT_BITS: usize = SERIAL_LEFT_BITS + 8;
const SERIAL_SLACK_BITS: usize = SERIAL_RIGHT_BITS + 8;

const SERIAL_SOURCE_LOGICAL_ID: usize = BASE_ENDPOINT_INSTANCE;
const SERIAL_SOURCE_LENGTH: usize = BASE_B;
const SERIAL_SOURCE_QUERY_VALUE: usize = BASE_C;
const SERIAL_SOURCE_COUNT_BEFORE: usize = BASE_D;
const SERIAL_SOURCE_COUNT_AFTER: usize = BASE_E;
const SERIAL_SOURCE_FIRST_INVERSE: usize = BASE_F;
const SERIAL_BYTE_TABLE_MULTIPLICITY: usize = BASE_C;
const SERIAL_NODE_TABLE_MULTIPLICITY: usize = BASE_F;

fn write_u8_bits_v1(row: &mut ZkX509Rfc5280StarkBaseRowV1, start: usize, value: u8) {
    for bit in 0..8 {
        row[start + bit] = F(u64::from((value >> bit) & 1));
    }
}

fn write_u16_bits_v1(row: &mut ZkX509Rfc5280StarkBaseRowV1, start: usize, value: u16) {
    for bit in 0..16 {
        row[start + bit] = F(u64::from((value >> bit) & 1));
    }
}

pub(crate) fn build_zk_x509_rfc5280_serial_comparison_rows_v1(
    comparison: &ZkX509Rfc5280SerialComparisonV1,
) -> Result<Vec<ZkX509Rfc5280StarkBaseRowV1>, ZkX509Rfc5280StarkErrorV1> {
    validate_serial_comparison_v1(comparison)?;
    let strict = comparison.kind == ZkX509Rfc5280SerialComparisonKindV1::AdjacentStrictOrder;
    let mut prefix_equal = true;
    let mut order_satisfied = false;
    let left_length = comparison.left[0];
    let right_length = comparison.right[0];
    let mut left_count = 0_u8;
    let mut right_count = 0_u8;
    let mut rows = Vec::new();
    rows.try_reserve_exact(SERIAL_COMPARISON_WIDTH_V1)
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    for offset in 0..SERIAL_COMPARISON_WIDTH_V1 {
        let left = comparison.left[offset];
        let right = comparison.right[offset];
        let equal = left == right;
        let less = strict && prefix_equal && !equal && left < right;
        let slack = if less {
            right
                .checked_sub(left)
                .and_then(|difference| difference.checked_sub(1))
                .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?
        } else {
            0
        };
        let mut row = [F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1];
        row[BASE_ACTIVE] = F::ONE;
        row[BASE_A] = F(u64::from(left));
        row[BASE_B] = F(u64::from(right));
        row[BASE_C] = F(u64::from(slack));
        row[BASE_ROLE] = F(u64::from(comparison.left_instance));
        row[BASE_INSTANCE] = F(u64::from(comparison.right_instance));
        row[BASE_OFFSET] =
            F(u64::try_from(offset).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?);
        row[BASE_STRICT] = F(u64::from(strict));
        row[BASE_EQUAL] = F(u64::from(equal));
        row[BASE_INVERSE] = if equal {
            F::ZERO
        } else {
            F(u64::from(left))
                .sub(F(u64::from(right)))
                .inv()
                .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?
        };
        row[BASE_STATE_BEFORE] = F(u64::from(prefix_equal));
        row[BASE_STATE_AFTER] = F(u64::from(prefix_equal && equal));
        row[SERIAL_LESS] = F(u64::from(less));
        row[SERIAL_ORDER_BEFORE] = F(u64::from(order_satisfied));
        order_satisfied |= less;
        row[SERIAL_ORDER_AFTER] = F(u64::from(order_satisfied));
        let left_active = offset != 0 && offset <= usize::from(left_length);
        let right_active = offset != 0 && offset <= usize::from(right_length);
        row[SERIAL_LEFT_LENGTH] = F(u64::from(left_length));
        row[SERIAL_RIGHT_LENGTH] = F(u64::from(right_length));
        row[SERIAL_LEFT_ACTIVE] = F(u64::from(left_active));
        row[SERIAL_RIGHT_ACTIVE] = F(u64::from(right_active));
        row[SERIAL_LEFT_COUNT_BEFORE] = F(u64::from(left_count));
        row[SERIAL_RIGHT_COUNT_BEFORE] = F(u64::from(right_count));
        left_count = left_count
            .checked_add(u8::from(left_active))
            .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
        right_count = right_count
            .checked_add(u8::from(right_active))
            .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
        row[SERIAL_LEFT_COUNT_AFTER] = F(u64::from(left_count));
        row[SERIAL_RIGHT_COUNT_AFTER] = F(u64::from(right_count));
        if offset == 1 {
            row[SERIAL_LEFT_FIRST_INVERSE] = F(u64::from(left))
                .inv()
                .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
            row[SERIAL_RIGHT_FIRST_INVERSE] = F(u64::from(right))
                .inv()
                .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
        }
        write_u8_bits_v1(&mut row, SERIAL_LEFT_BITS, left);
        write_u8_bits_v1(&mut row, SERIAL_RIGHT_BITS, right);
        write_u8_bits_v1(&mut row, SERIAL_SLACK_BITS, slack);
        rows.push(row);
        prefix_equal &= equal;
    }
    if (!strict && prefix_equal) || (strict && !order_satisfied) {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    if left_count != left_length || right_count != right_length {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    Ok(rows)
}

pub(crate) fn build_zk_x509_rfc5280_serial_source_rows_v1(
    source: &ZkX509Rfc5280SerialSourceV1,
) -> Result<Vec<ZkX509Rfc5280StarkBaseRowV1>, ZkX509Rfc5280StarkErrorV1> {
    validate_serial_frame_v1(&source.frame)?;
    let length = source.frame[0];
    let encoded = source
        .encoded_contents
        .iter()
        .map(|cell| cell.value)
        .collect::<Vec<_>>();
    let magnitude = &source.frame[1..1 + usize::from(length)];
    let sign_padding = encoded.len() == magnitude.len() + 1
        && encoded.first() == Some(&0)
        && encoded.get(1..) == Some(magnitude);
    if (encoded.as_slice() != magnitude && !sign_padding)
        || sign_padding != (magnitude[0] & 0x80 != 0)
        || usize::from(source.node.content_start)
            .checked_add(encoded.len())
            .filter(|end| *end == usize::from(source.node.content_end))
            .is_none()
        || source
            .encoded_contents
            .iter()
            .enumerate()
            .any(|(offset, cell)| {
                cell.document != source.node.document
                    || usize::from(cell.address) != usize::from(source.node.content_start) + offset
            })
    {
        return Err(ZkX509Rfc5280StarkErrorV1::Source);
    }
    let content_len = source
        .node
        .content_end
        .checked_sub(source.node.content_start)
        .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
    let mut count = 0_u8;
    let mut rows = Vec::new();
    rows.try_reserve_exact(SERIAL_COMPARISON_WIDTH_V1)
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    for offset in 0..SERIAL_COMPARISON_WIDTH_V1 {
        let value = source.frame[offset];
        let active = offset != 0 && offset <= usize::from(length);
        let query_padding = offset == 0 && sign_padding;
        let query_active = query_padding || active;
        let mut row = [F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1];
        row[BASE_ACTIVE] = F::ONE;
        row[BASE_VALUE] = F(u64::from(value));
        write_u8_bits_v1(&mut row, BASE_BYTE_BITS, value);
        row[BASE_A] = F(u64::from(content_len));
        row[SERIAL_SOURCE_LENGTH] = F(u64::from(length));
        row[BASE_DOCUMENT] = F(u64::from(source.node.document));
        row[BASE_NODE] = F(u64::from(source.node.node));
        row[BASE_PARENT] = F(u64::from(source.node.parent_node));
        row[BASE_CHILD] = F(u64::from(source.node.child_ordinal));
        row[BASE_START] = F(u64::from(source.node.start));
        row[BASE_CONTENT_START] = F(u64::from(source.node.content_start));
        row[BASE_CONTENT_END] = F(u64::from(source.node.content_end));
        row[BASE_DEPTH] = F(u64::from(source.node.depth));
        row[BASE_TAG_CLASS] = F(u64::from(source.node.tag_class));
        row[BASE_CONSTRUCTED] = F(u64::from(source.node.constructed));
        row[BASE_TAG_NUMBER] = F(u64::from(source.node.tag_number));
        row[BASE_ROLE] = F(source.node.role as u64);
        row[BASE_INSTANCE] = F(u64::from(source.node.role_instance));
        row[BASE_OFFSET] =
            F(u64::try_from(offset).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?);
        row[SERIAL_SOURCE_LOGICAL_ID] = F(u64::from(source.logical_id));
        row[BASE_STRICT] = F(u64::from(sign_padding));
        row[BASE_EQUAL] = F(u64::from(active));
        row[BASE_IS_WRITE] = F(u64::from(query_active));
        row[SERIAL_SOURCE_COUNT_BEFORE] = F(u64::from(count));
        count = count
            .checked_add(u8::from(active))
            .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
        row[SERIAL_SOURCE_COUNT_AFTER] = F(u64::from(count));
        if offset == 1 {
            row[SERIAL_SOURCE_FIRST_INVERSE] = F(u64::from(value))
                .inv()
                .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
        }
        if query_padding {
            row[BASE_ADDRESS] = F(u64::from(source.node.content_start));
        } else if active {
            let address = usize::from(source.node.content_end)
                .checked_sub(usize::from(length))
                .and_then(|address| address.checked_add(offset - 1))
                .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?;
            row[BASE_ADDRESS] =
                F(u64::try_from(address).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?);
            row[SERIAL_SOURCE_QUERY_VALUE] = F(u64::from(value));
        }
        rows.push(row);
    }
    if count != length {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    Ok(rows)
}

const FIX_GLOBAL_FIRST: usize = FAMILY_COUNT_V1;
const FIX_GLOBAL_LAST: usize = FIX_GLOBAL_FIRST + 1;
const FIX_CONTINUE: usize = FIX_GLOBAL_LAST + 1;
const FIX_LOCAL_FIRST: usize = FIX_CONTINUE + 1;
const FIX_LOCAL_LAST: usize = FIX_LOCAL_FIRST + 1;
const FIX_EXPECTED: usize = FIX_LOCAL_LAST + 1;
const FIX_ACTIVATION_CONTINUE: usize = FIX_EXPECTED + 10;
const FIX_REQUIRED_ACTIVE: usize = FIX_ACTIVATION_CONTINUE + 1;
const FIX_ADDRESS_FIXED: usize = FIX_REQUIRED_ACTIVE + 1;
const FIX_CERT2_OUTPUT: usize = FIX_ADDRESS_FIXED + 1;
const FIX_CERT2_SLOT_FIRST: usize = FIX_CERT2_OUTPUT + 1;
const FIX_DOCUMENT_FIXED: usize = FIX_CERT2_SLOT_FIRST + 1;
const FIX_GRAMMAR_RULE: usize = FIX_DOCUMENT_FIXED + 1;
const FIX_GRAMMAR_ORDINAL_FIRST: usize = FIX_GRAMMAR_RULE + 1;
// Products of verifier-owned selectors are materialized before interpolation.
// Referring to these columns, rather than multiplying their separate LDEs in
// the AIR, keeps the registered polynomial degree truthful.
const FIX_GRAMMAR_RULE_TABLE: usize = FIX_GRAMMAR_ORDINAL_FIRST + 1;
const FIX_GRAMMAR_ORDINAL_TABLE: usize = FIX_GRAMMAR_RULE_TABLE + 1;
const FIX_SOURCE_NODE_NON_ROOT: usize = FIX_GRAMMAR_ORDINAL_TABLE + 1;
const FIX_PROFILE_TABLE: usize = FIX_SOURCE_NODE_NON_ROOT + 1;
const FIX_SERIAL_SOURCE_FIRST: usize = FIX_PROFILE_TABLE + 1;
const FIX_SERIAL_SOURCE_INTERIOR: usize = FIX_SERIAL_SOURCE_FIRST + 1;
const FIX_SERIAL_SOURCE_NOT_FIRST: usize = FIX_SERIAL_SOURCE_INTERIOR + 1;
const FIX_SERIAL_SOURCE_FIRST_PAYLOAD: usize = FIX_SERIAL_SOURCE_NOT_FIRST + 1;
const FIX_SERIAL_COMPARE_PHASE_LEFT: usize = FIX_SERIAL_SOURCE_FIRST_PAYLOAD + 1;
const FIX_SERIAL_COMPARE_PHASE_RIGHT: usize = FIX_SERIAL_COMPARE_PHASE_LEFT + 1;
const FIX_SERIAL_COMPARE_FIRST: usize = FIX_SERIAL_COMPARE_PHASE_RIGHT + 1;
const FIX_SERIAL_COMPARE_LAST: usize = FIX_SERIAL_COMPARE_FIRST + 1;
const FIX_SERIAL_COMPARE_INTERIOR: usize = FIX_SERIAL_COMPARE_LAST + 1;
const FIX_SERIAL_COMPARE_NOT_FIRST: usize = FIX_SERIAL_COMPARE_INTERIOR + 1;
const FIX_SERIAL_COMPARE_FIRST_PAYLOAD: usize = FIX_SERIAL_COMPARE_NOT_FIRST + 1;
const FIX_CALENDAR_PHASES: usize = FIX_SERIAL_COMPARE_FIRST_PAYLOAD + 1;
const FIX_RANGE_TERMINAL: usize = FIX_CALENDAR_PHASES + CALENDAR_COPY_PHASES_V1;
const FIX_OUTPUT_ROLE_PRODUCTS: usize = FIX_RANGE_TERMINAL + 1;
const OUTPUT_ENDPOINT_COUNT_V1: usize = 2;

const _: () = assert!(
    FIX_OUTPUT_ROLE_PRODUCTS + OUTPUT_ROLE_COUNT_V1 * OUTPUT_ENDPOINT_COUNT_V1
        == ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1
);

const AUX_DER_BYTE_BEFORE: usize = 0;
const AUX_DER_BYTE_AFTER: usize = 4;
const AUX_DER_NODE_BEFORE: usize = 8;
const AUX_DER_NODE_AFTER: usize = 12;
const AUX_PROFILE_LOOKUP_ACCUMULATOR: usize = 16;
const AUX_PROFILE_TABLE_INVERSE: usize = 20;
const AUX_PROFILE_QUERY_INVERSE: usize = 24;
const AUX_PROFILE_ZERO_ACCUMULATOR: usize = 28;
const AUX_OUTPUT_PRODUCER_BEFORE: usize = 32;
const AUX_OUTPUT_PRODUCER_AFTER: usize = 36;
const AUX_OUTPUT_CONSUMER_BEFORE: usize = 40;
const AUX_OUTPUT_CONSUMER_AFTER: usize = 44;
const AUX_SERIAL_SOURCE_BEFORE: usize = 48;
const AUX_SERIAL_SOURCE_AFTER: usize = 52;
const AUX_SERIAL_CONSUMER_BEFORE: usize = 56;
const AUX_SERIAL_CONSUMER_AFTER: usize = 60;
const AUX_SERIAL_BYTE_LOOKUP_ACCUMULATOR: usize = 64;
const AUX_SERIAL_BYTE_TABLE_INVERSE: usize = 68;
const AUX_SERIAL_BYTE_QUERY_INVERSE: usize = 72;
const AUX_SERIAL_BYTE_ZERO_ACCUMULATOR: usize = 76;
const AUX_SERIAL_BYTE_TABLE_ZERO: usize = 80;
const AUX_SERIAL_BYTE_QUERY_ZERO: usize = 84;
const AUX_SERIAL_NODE_LOOKUP_ACCUMULATOR: usize = 88;
const AUX_SERIAL_NODE_TABLE_INVERSE: usize = 92;
const AUX_SERIAL_NODE_QUERY_INVERSE: usize = 96;
const AUX_SERIAL_NODE_ZERO_ACCUMULATOR: usize = 100;
const AUX_SERIAL_NODE_TABLE_ZERO: usize = 104;
const AUX_SERIAL_NODE_QUERY_ZERO: usize = 108;
const AUX_PROFILE_TABLE_ZERO: usize = 112;
const AUX_PROFILE_QUERY_ZERO: usize = 116;
const AUX_PROFILE_TOPOLOGY_QUERY_INVERSE: usize = 120;
const AUX_PROFILE_TOPOLOGY_QUERY_ZERO: usize = 124;
const AUX_GRAMMAR_RULE_LOOKUP_ACCUMULATOR: usize = 128;
const AUX_GRAMMAR_RULE_TABLE_INVERSE: usize = 132;
const AUX_GRAMMAR_RULE_QUERY_INVERSE: usize = 136;
const AUX_GRAMMAR_RULE_ZERO_ACCUMULATOR: usize = 140;
const AUX_GRAMMAR_RULE_TABLE_ZERO: usize = 144;
const AUX_GRAMMAR_RULE_QUERY_ZERO: usize = 148;
const AUX_GRAMMAR_PARENT_LOOKUP_ACCUMULATOR: usize = 152;
const AUX_GRAMMAR_PARENT_TABLE_INVERSE: usize = 156;
const AUX_GRAMMAR_PARENT_QUERY_INVERSE: usize = 160;
const AUX_GRAMMAR_PARENT_ZERO_ACCUMULATOR: usize = 164;
const AUX_GRAMMAR_PARENT_TABLE_ZERO: usize = 168;
const AUX_GRAMMAR_PARENT_QUERY_ZERO: usize = 172;
const AUX_GRAMMAR_ORDINAL_SOURCE_BEFORE: usize = 176;
const AUX_GRAMMAR_ORDINAL_SOURCE_AFTER: usize = 180;
const AUX_GRAMMAR_ORDINAL_TABLE_BEFORE: usize = 184;
const AUX_GRAMMAR_ORDINAL_TABLE_AFTER: usize = 188;
const AUX_OUTPUT_ROLE_PRODUCTS: usize = 192;
const TERMINAL_RELATIONS_V1: usize = 6;
const RFC5280_TERMINAL_CLAIM_MAGIC_V1: [u8; 4] = *b"X5R1";
const RFC5280_TERMINAL_CLAIM_VERSION_V1: u16 = 1;
const RFC5280_TERMINAL_CLAIM_ADAPTER_V1: u16 = 3;
const RFC5280_AGGREGATE_TERMINAL_CLAIM_RECORDS_V1: usize = 4 * ZK_X509_RFC5280_STARK_BUS_LANES_V1;
const RFC5280_OUTPUT_ROLE_TERMINAL_CLAIM_RECORDS_V1: usize =
    OUTPUT_ROLE_COUNT_V1 * OUTPUT_ENDPOINT_COUNT_V1 * ZK_X509_RFC5280_STARK_BUS_LANES_V1;
const RFC5280_TERMINAL_CLAIM_RECORDS_V1: usize =
    RFC5280_AGGREGATE_TERMINAL_CLAIM_RECORDS_V1 + RFC5280_OUTPUT_ROLE_TERMINAL_CLAIM_RECORDS_V1;
const RFC5280_TERMINAL_CLAIM_RECORD_BYTES_V1: usize = 2 + 2 + 2 + 2 + 8;
const RFC5280_TERMINAL_CLAIM_HEADER_BYTES_V1: usize = 4 + 2 + 2 + 2 + 2;
/// Exact X5R1 proof-carried terminal-claim frame size.
pub(crate) const ZK_X509_RFC5280_TERMINAL_CLAIM_BYTES_V1: usize =
    RFC5280_TERMINAL_CLAIM_HEADER_BYTES_V1
        + RFC5280_TERMINAL_CLAIM_RECORDS_V1 * RFC5280_TERMINAL_CLAIM_RECORD_BYTES_V1;
// Wire family, internal relation slot, and final auxiliary product column.
const RFC5280_TERMINAL_CLAIM_RELATIONS_V1: [(u16, usize, usize); 4] = [
    (1, 0, AUX_DER_BYTE_AFTER),
    (2, 1, AUX_DER_NODE_AFTER),
    (3, 4, AUX_OUTPUT_PRODUCER_AFTER),
    (3, 5, AUX_OUTPUT_CONSUMER_AFTER),
];

const _: () = assert!(
    AUX_OUTPUT_ROLE_PRODUCTS
        + OUTPUT_ROLE_COUNT_V1 * OUTPUT_ENDPOINT_COUNT_V1 * ZK_X509_RFC5280_STARK_BUS_LANES_V1
        == ZK_X509_RFC5280_STARK_AUX_WIDTH_V1
);
const _: () = assert!(RFC5280_TERMINAL_CLAIM_RECORDS_V1 == 88);
const _: () = assert!(ZK_X509_RFC5280_TERMINAL_CLAIM_BYTES_V1 == 1_420);

const fn output_endpoint_index_v1(consumer: bool) -> usize {
    if consumer { 1 } else { 0 }
}

const fn output_role_fixed_selector_column_v1(role_index: usize, consumer: bool) -> usize {
    FIX_OUTPUT_ROLE_PRODUCTS
        + role_index * OUTPUT_ENDPOINT_COUNT_V1
        + output_endpoint_index_v1(consumer)
}

const fn output_role_aux_column_v1(role_index: usize, consumer: bool, lane: usize) -> usize {
    AUX_OUTPUT_ROLE_PRODUCTS
        + (role_index * OUTPUT_ENDPOINT_COUNT_V1 + output_endpoint_index_v1(consumer))
            * ZK_X509_RFC5280_STARK_BUS_LANES_V1
        + lane
}

/// One canonical role-addressed pair of independently committed RFC output
/// products.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509Rfc5280OutputRoleTerminalClaimsV1 {
    pub(crate) role: ZkX509Rfc5280OutputRoleV1,
    pub(crate) producer_products: [F; ZK_X509_RFC5280_STARK_BUS_LANES_V1],
    pub(crate) consumer_products: [F; ZK_X509_RFC5280_STARK_BUS_LANES_V1],
}

/// Exact typed address of one X5R1 terminal record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ZkX509Rfc5280TerminalClaimAddressV1 {
    family: u16,
    address: u16,
    lane: u16,
    endpoint: u16,
}

/// Ordered final claims: DER bytes, DER nodes, two reserved identity slots,
/// aggregate output products, and every independently committed output role.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509Rfc5280StarkTerminalClaimsV1 {
    pub(crate) relations: [[F; ZK_X509_RFC5280_STARK_BUS_LANES_V1]; TERMINAL_RELATIONS_V1],
    output_roles: [ZkX509Rfc5280OutputRoleTerminalClaimsV1; OUTPUT_ROLE_COUNT_V1],
}

impl ZkX509Rfc5280StarkTerminalClaimsV1 {
    fn canonical_identity_v1() -> Self {
        let relations = [[F::ONE; ZK_X509_RFC5280_STARK_BUS_LANES_V1]; TERMINAL_RELATIONS_V1];
        Self {
            relations,
            output_roles: core::array::from_fn(|role_index| {
                ZkX509Rfc5280OutputRoleTerminalClaimsV1 {
                    role: OUTPUT_ROLES_V1[role_index],
                    producer_products: [F::ONE; ZK_X509_RFC5280_STARK_BUS_LANES_V1],
                    consumer_products: [F::ONE; ZK_X509_RFC5280_STARK_BUS_LANES_V1],
                }
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn canonical_zero_for_test_v1() -> Self {
        Self::canonical_identity_v1()
    }

    #[cfg(test)]
    pub(crate) fn canonical_for_der_test_v1(
        der: ZkX509DerStarkTerminalClaimsV1,
    ) -> Result<Self, ZkX509Rfc5280StarkErrorV1> {
        let mut claims = Self::canonical_identity_v1();
        claims.relations[0] = der.input_byte;
        claims.relations[1] = der.node;
        validate_zk_x509_der_rfc_terminal_equalities_v1(der, claims)?;
        Ok(claims)
    }

    pub(crate) const fn der_input_byte_products_v1(
        &self,
    ) -> [F; ZK_X509_RFC5280_STARK_BUS_LANES_V1] {
        self.relations[0]
    }

    pub(crate) const fn der_node_products_v1(&self) -> [F; ZK_X509_RFC5280_STARK_BUS_LANES_V1] {
        self.relations[1]
    }

    pub(crate) fn output_role_products_v1(
        &self,
        role: ZkX509Rfc5280OutputRoleV1,
    ) -> ZkX509Rfc5280OutputRoleTerminalClaimsV1 {
        self.output_roles[output_role_index_v1(role)]
    }

    pub(crate) fn governed_trust_anchor_products_v1(
        &self,
    ) -> ZkX509Rfc5280OutputRoleTerminalClaimsV1 {
        self.output_role_products_v1(ZkX509Rfc5280OutputRoleV1::GovernedTrustAnchor)
    }

    pub(crate) fn certificate_slot_active_products_v1(
        &self,
    ) -> ZkX509Rfc5280OutputRoleTerminalClaimsV1 {
        self.output_role_products_v1(ZkX509Rfc5280OutputRoleV1::CertificateSlotActive)
    }

    fn claim_address_v1(claim_index: usize) -> Option<ZkX509Rfc5280TerminalClaimAddressV1> {
        if claim_index < RFC5280_AGGREGATE_TERMINAL_CLAIM_RECORDS_V1 {
            let relation_index = claim_index / ZK_X509_RFC5280_STARK_BUS_LANES_V1;
            let lane = claim_index % ZK_X509_RFC5280_STARK_BUS_LANES_V1;
            let (family, relation, _) = RFC5280_TERMINAL_CLAIM_RELATIONS_V1
                .get(relation_index)
                .copied()?;
            let endpoint = match relation {
                4 => 1,
                5 => 2,
                _ => 0,
            };
            return Some(ZkX509Rfc5280TerminalClaimAddressV1 {
                family,
                address: 0,
                lane: u16::try_from(lane).ok()?,
                endpoint,
            });
        }
        let local = claim_index.checked_sub(RFC5280_AGGREGATE_TERMINAL_CLAIM_RECORDS_V1)?;
        let role_index = local / (OUTPUT_ENDPOINT_COUNT_V1 * ZK_X509_RFC5280_STARK_BUS_LANES_V1);
        let endpoint_lane = local % (OUTPUT_ENDPOINT_COUNT_V1 * ZK_X509_RFC5280_STARK_BUS_LANES_V1);
        let endpoint_index = endpoint_lane / ZK_X509_RFC5280_STARK_BUS_LANES_V1;
        let lane = endpoint_lane % ZK_X509_RFC5280_STARK_BUS_LANES_V1;
        Some(ZkX509Rfc5280TerminalClaimAddressV1 {
            family: 3,
            address: u16::try_from(role_index.checked_add(1)?).ok()?,
            lane: u16::try_from(lane).ok()?,
            endpoint: u16::try_from(endpoint_index.checked_add(1)?).ok()?,
        })
        .filter(|_| role_index < OUTPUT_ROLE_COUNT_V1)
    }

    fn claim_value_v1(&self, claim_index: usize) -> Option<F> {
        if claim_index < RFC5280_AGGREGATE_TERMINAL_CLAIM_RECORDS_V1 {
            let relation_index = claim_index / ZK_X509_RFC5280_STARK_BUS_LANES_V1;
            let lane = claim_index % ZK_X509_RFC5280_STARK_BUS_LANES_V1;
            let (_, relation, _) = RFC5280_TERMINAL_CLAIM_RELATIONS_V1
                .get(relation_index)
                .copied()?;
            return self
                .relations
                .get(relation)
                .and_then(|products| products.get(lane))
                .copied();
        }
        let local = claim_index.checked_sub(RFC5280_AGGREGATE_TERMINAL_CLAIM_RECORDS_V1)?;
        let role_index = local / (OUTPUT_ENDPOINT_COUNT_V1 * ZK_X509_RFC5280_STARK_BUS_LANES_V1);
        let endpoint_lane = local % (OUTPUT_ENDPOINT_COUNT_V1 * ZK_X509_RFC5280_STARK_BUS_LANES_V1);
        let consumer =
            endpoint_lane / ZK_X509_RFC5280_STARK_BUS_LANES_V1 == output_endpoint_index_v1(true);
        let lane = endpoint_lane % ZK_X509_RFC5280_STARK_BUS_LANES_V1;
        let role = self.output_roles.get(role_index)?;
        if consumer {
            role.consumer_products.get(lane).copied()
        } else {
            role.producer_products.get(lane).copied()
        }
    }

    fn set_claim_value_v1(
        &mut self,
        claim_index: usize,
        value: F,
    ) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
        if claim_index < RFC5280_AGGREGATE_TERMINAL_CLAIM_RECORDS_V1 {
            let relation_index = claim_index / ZK_X509_RFC5280_STARK_BUS_LANES_V1;
            let lane = claim_index % ZK_X509_RFC5280_STARK_BUS_LANES_V1;
            let (_, relation, _) = RFC5280_TERMINAL_CLAIM_RELATIONS_V1
                .get(relation_index)
                .copied()
                .ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
            self.relations[relation][lane] = value;
            return Ok(());
        }
        let local = claim_index
            .checked_sub(RFC5280_AGGREGATE_TERMINAL_CLAIM_RECORDS_V1)
            .ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
        let role_index = local / (OUTPUT_ENDPOINT_COUNT_V1 * ZK_X509_RFC5280_STARK_BUS_LANES_V1);
        let endpoint_lane = local % (OUTPUT_ENDPOINT_COUNT_V1 * ZK_X509_RFC5280_STARK_BUS_LANES_V1);
        let consumer =
            endpoint_lane / ZK_X509_RFC5280_STARK_BUS_LANES_V1 == output_endpoint_index_v1(true);
        let lane = endpoint_lane % ZK_X509_RFC5280_STARK_BUS_LANES_V1;
        let role = self
            .output_roles
            .get_mut(role_index)
            .ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
        if consumer {
            role.consumer_products[lane] = value;
        } else {
            role.producer_products[lane] = value;
        }
        Ok(())
    }

    fn validate_v1(&self) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
        if self
            .relations
            .iter()
            .flatten()
            .chain(
                self.output_roles
                    .iter()
                    .flat_map(|role| role.producer_products.iter()),
            )
            .chain(
                self.output_roles
                    .iter()
                    .flat_map(|role| role.consumer_products.iter()),
            )
            .any(|value| F::canonical(value.0).is_none())
            || self.relations[2]
                .iter()
                .chain(&self.relations[3])
                .any(|value| *value != F::ONE)
            || self
                .output_roles
                .iter()
                .zip(OUTPUT_ROLES_V1)
                .any(|(actual, expected)| actual.role != expected)
        {
            return Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim);
        }
        for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
            let (producer, consumer) =
                self.output_roles
                    .iter()
                    .fold((F::ONE, F::ONE), |(producer, consumer), role| {
                        (
                            producer.mul(role.producer_products[lane]),
                            consumer.mul(role.consumer_products[lane]),
                        )
                    });
            if producer != self.relations[4][lane] || consumer != self.relations[5][lane] {
                return Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim);
            }
        }
        Ok(())
    }

    /// Encode the sole canonical proof-carried terminal frame.
    ///
    /// The two internal reserved relation slots are reconstructed as the fixed
    /// identity and therefore cannot become unconstrained proof fields.
    pub(crate) fn encode_x5r1_v1(
        self,
    ) -> Result<[u8; ZK_X509_RFC5280_TERMINAL_CLAIM_BYTES_V1], ZkX509Rfc5280StarkErrorV1> {
        self.validate_v1()?;
        let mut encoded = [0_u8; ZK_X509_RFC5280_TERMINAL_CLAIM_BYTES_V1];
        encoded[..4].copy_from_slice(&RFC5280_TERMINAL_CLAIM_MAGIC_V1);
        encoded[4..6].copy_from_slice(&RFC5280_TERMINAL_CLAIM_VERSION_V1.to_be_bytes());
        encoded[6..8].copy_from_slice(&RFC5280_TERMINAL_CLAIM_ADAPTER_V1.to_be_bytes());
        encoded[8..10].copy_from_slice(&0_u16.to_be_bytes());
        encoded[10..12].copy_from_slice(
            &u16::try_from(RFC5280_TERMINAL_CLAIM_RECORDS_V1)
                .expect("RFC terminal claim count fits u16")
                .to_be_bytes(),
        );
        for claim_index in 0..RFC5280_TERMINAL_CLAIM_RECORDS_V1 {
            let address =
                Self::claim_address_v1(claim_index).expect("bounded RFC terminal claim address");
            let value = self
                .claim_value_v1(claim_index)
                .expect("bounded RFC terminal claim value");
            let start = RFC5280_TERMINAL_CLAIM_HEADER_BYTES_V1
                + claim_index * RFC5280_TERMINAL_CLAIM_RECORD_BYTES_V1;
            encoded[start..start + 2].copy_from_slice(&address.family.to_be_bytes());
            encoded[start + 2..start + 4].copy_from_slice(&address.address.to_be_bytes());
            encoded[start + 4..start + 6].copy_from_slice(&address.lane.to_be_bytes());
            encoded[start + 6..start + 8].copy_from_slice(&address.endpoint.to_be_bytes());
            encoded[start + 8..start + RFC5280_TERMINAL_CLAIM_RECORD_BYTES_V1]
                .copy_from_slice(&value.0.to_be_bytes());
        }
        Ok(encoded)
    }

    /// Decode an exact X5R1 frame, rejecting aliases, omitted/reordered
    /// records, duplicates, noncanonical fields, and trailing bytes.
    pub(crate) fn decode_x5r1_v1(encoded: &[u8]) -> Result<Self, ZkX509Rfc5280StarkErrorV1> {
        if encoded.len() != ZK_X509_RFC5280_TERMINAL_CLAIM_BYTES_V1
            || encoded[..4] != RFC5280_TERMINAL_CLAIM_MAGIC_V1
            || u16::from_be_bytes(
                encoded[4..6]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            ) != RFC5280_TERMINAL_CLAIM_VERSION_V1
            || u16::from_be_bytes(
                encoded[6..8]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            ) != RFC5280_TERMINAL_CLAIM_ADAPTER_V1
            || u16::from_be_bytes(
                encoded[8..10]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            ) != 0
            || usize::from(u16::from_be_bytes(
                encoded[10..12]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            )) != RFC5280_TERMINAL_CLAIM_RECORDS_V1
        {
            return Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim);
        }

        let mut claims = Self::canonical_identity_v1();
        for claim_index in 0..RFC5280_TERMINAL_CLAIM_RECORDS_V1 {
            let expected = Self::claim_address_v1(claim_index)
                .ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
            let start = RFC5280_TERMINAL_CLAIM_HEADER_BYTES_V1
                + claim_index * RFC5280_TERMINAL_CLAIM_RECORD_BYTES_V1;
            let actual = ZkX509Rfc5280TerminalClaimAddressV1 {
                family: u16::from_be_bytes(
                    encoded[start..start + 2]
                        .try_into()
                        .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
                ),
                address: u16::from_be_bytes(
                    encoded[start + 2..start + 4]
                        .try_into()
                        .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
                ),
                lane: u16::from_be_bytes(
                    encoded[start + 4..start + 6]
                        .try_into()
                        .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
                ),
                endpoint: u16::from_be_bytes(
                    encoded[start + 6..start + 8]
                        .try_into()
                        .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
                ),
            };
            if actual != expected {
                return Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim);
            }
            let raw = u64::from_be_bytes(
                encoded[start + 8..start + RFC5280_TERMINAL_CLAIM_RECORD_BYTES_V1]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            );
            let value = F::canonical(raw).ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
            claims.set_claim_value_v1(claim_index, value)?;
        }
        claims.validate_v1()?;
        Ok(claims)
    }
}

/// Validate every terminal equality owned jointly by strict DER and RFC 5280.
///
/// The role products are independently tied to verifier-fixed RFC AIR
/// accumulators; this pure boundary check additionally fixes their canonical
/// decomposition and the two DER-to-RFC hand-offs before MAIN samples
/// composition coefficients.
pub(crate) fn validate_zk_x509_der_rfc_terminal_equalities_v1(
    der: ZkX509DerStarkTerminalClaimsV1,
    rfc: ZkX509Rfc5280StarkTerminalClaimsV1,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    if der
        .input_byte
        .iter()
        .chain(&der.node)
        .any(|value| F::canonical(value.0).is_none())
    {
        return Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim);
    }
    rfc.validate_v1()?;
    if der.input_byte != rfc.der_input_byte_products_v1() || der.node != rfc.der_node_products_v1()
    {
        return Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim);
    }
    Ok(())
}

const SHA_TERMINAL_CLAIM_MAGIC_V1: [u8; 4] = *b"X5Q1";
const SHA_TERMINAL_CLAIM_VERSION_V1: u16 = 1;
const SHA_TERMINAL_CLAIM_ADAPTER_V1: u16 = 4;
const SHA_TERMINAL_CLAIM_INSTANCE_V1: u16 = 0;
const SHA_TERMINAL_CLAIM_FAMILIES_V1: usize = 6;
const SHA_TERMINAL_CLAIM_RFC_STREAMS_V1: usize = 4;
const SHA_TERMINAL_CLAIM_RECORDS_PER_SEGMENT_V1: usize =
    SHA_TERMINAL_CLAIM_FAMILIES_V1 * ZK_X509_SHA_BUS_LANES_V1;
const SHA_SEGMENT_TERMINAL_CLAIM_RECORDS_V1: usize =
    ZK_X509_SHA_SEGMENT_COUNT_V1 * SHA_TERMINAL_CLAIM_RECORDS_PER_SEGMENT_V1;
const SHA_CA_BOUNDARY_CLAIM_FAMILIES_V1: usize = 4;
const SHA_CA_BOUNDARY_CLAIM_RECORDS_PER_CALL_V1: usize =
    SHA_CA_BOUNDARY_CLAIM_FAMILIES_V1 * ZK_X509_SHA_BUS_LANES_V1;
const SHA_CA_BOUNDARY_CLAIM_RECORDS_V1: usize =
    ZK_X509_SHA_CA_CALL_COUNT_V1 * SHA_CA_BOUNDARY_CLAIM_RECORDS_PER_CALL_V1;
const SHA_TERMINAL_CLAIM_RECORDS_V1: usize =
    SHA_SEGMENT_TERMINAL_CLAIM_RECORDS_V1 + SHA_CA_BOUNDARY_CLAIM_RECORDS_V1;
const SHA_TERMINAL_CLAIM_RECORD_BYTES_V1: usize = 2 + 2 + 2 + 2 + 8;
const SHA_TERMINAL_CLAIM_HEADER_BYTES_V1: usize = 4 + 2 + 2 + 2 + 2;
/// Exact X5Q1 proof-carried SHA segment-terminal frame size.
pub(crate) const ZK_X509_SHA_SEGMENT_TERMINAL_CLAIM_BYTES_V1: usize =
    SHA_TERMINAL_CLAIM_HEADER_BYTES_V1
        + SHA_TERMINAL_CLAIM_RECORDS_V1 * SHA_TERMINAL_CLAIM_RECORD_BYTES_V1;

const _: () = assert!(ZK_X509_SHA_BUS_LANES_V1 == ZK_X509_RFC5280_STARK_BUS_LANES_V1);
const _: () = assert!(ZK_X509_SHA_CALL_COUNT_V1 == 29);
const _: () = assert!(ZK_X509_SHA_SEGMENT_COUNT_V1 == 4);
const _: () = assert!(SHA_TERMINAL_CLAIM_RFC_STREAMS_V1 + 2 == SHA_TERMINAL_CLAIM_FAMILIES_V1);
const _: () = assert!(SHA_SEGMENT_TERMINAL_CLAIM_RECORDS_V1 == 96);
const _: () = assert!(SHA_CA_BOUNDARY_CLAIM_RECORDS_V1 == 208);
const _: () = assert!(SHA_TERMINAL_CLAIM_RECORDS_V1 == 304);
const _: () = assert!(ZK_X509_SHA_SEGMENT_TERMINAL_CLAIM_BYTES_V1 == 4_876);

/// Ordered proof claims for all SHA call-bus products.
///
/// The 29 fixed calls are packed whole into four verifier-owned physical
/// segments. Each segment carries the source-word product, digest-word
/// product, and four independently constrained RFC-consumer streams in four
/// challenge lanes. No call identity, family, lane, or segment is selected by
/// the prover: X5Q1 fixes all 304 addresses in this exact order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509ShaSegmentTerminalClaimsV1 {
    /// Segment terminals in verifier-owned physical registration order.
    pub(crate) segments: [ZkX509ShaSegmentTerminalV1; ZK_X509_SHA_SEGMENT_COUNT_V1],
    /// Cumulative starts and call-local terminals for calls `16..=28`.
    pub(crate) ca_calls: [ZkX509ShaCallBoundaryTerminalV1; ZK_X509_SHA_CA_CALL_COUNT_V1],
}

impl ZkX509ShaSegmentTerminalClaimsV1 {
    #[cfg(test)]
    pub(crate) fn canonical_zero_for_test_v1() -> Self {
        Self {
            segments: core::array::from_fn(|segment| ZkX509ShaSegmentTerminalV1 {
                segment: u8::try_from(segment).expect("four SHA segments fit u8"),
                source_products: [F::ZERO; ZK_X509_SHA_BUS_LANES_V1],
                digest_products: [F::ZERO; ZK_X509_SHA_BUS_LANES_V1],
                rfc_stream_products: [[F::ZERO; ZK_X509_SHA_BUS_LANES_V1];
                    SHA_TERMINAL_CLAIM_RFC_STREAMS_V1],
            }),
            ca_calls: core::array::from_fn(|index| {
                let (call, role) =
                    zk_x509_sha_ca_call_identity_v1(index).expect("canonical compact-CA call");
                ZkX509ShaCallBoundaryTerminalV1 {
                    call,
                    role,
                    source_start_products: [F::ZERO; ZK_X509_SHA_BUS_LANES_V1],
                    digest_start_products: [F::ZERO; ZK_X509_SHA_BUS_LANES_V1],
                    source_products: [F::ZERO; ZK_X509_SHA_BUS_LANES_V1],
                    digest_products: [F::ZERO; ZK_X509_SHA_BUS_LANES_V1],
                }
            }),
        }
    }

    /// Construct claims only from the four terminals returned by the SHA AIR
    /// segment providers.
    pub(crate) fn from_sha_air_terminals_v1(
        segments: [ZkX509ShaSegmentTerminalV1; ZK_X509_SHA_SEGMENT_COUNT_V1],
        ca_calls: [ZkX509ShaCallBoundaryTerminalV1; ZK_X509_SHA_CA_CALL_COUNT_V1],
    ) -> Result<Self, ZkX509Rfc5280StarkErrorV1> {
        let claims = Self { segments, ca_calls };
        claims.validate_v1()?;
        Ok(claims)
    }

    fn claim_address_v1(claim_index: usize) -> Option<(usize, usize, usize)> {
        if claim_index >= SHA_TERMINAL_CLAIM_RECORDS_V1 {
            return None;
        }
        if claim_index < SHA_SEGMENT_TERMINAL_CLAIM_RECORDS_V1 {
            let segment = claim_index / SHA_TERMINAL_CLAIM_RECORDS_PER_SEGMENT_V1;
            let local = claim_index % SHA_TERMINAL_CLAIM_RECORDS_PER_SEGMENT_V1;
            let family = local / ZK_X509_SHA_BUS_LANES_V1;
            let lane = local % ZK_X509_SHA_BUS_LANES_V1;
            return Some((segment, family, lane));
        }
        let local = claim_index - SHA_SEGMENT_TERMINAL_CLAIM_RECORDS_V1;
        let call = local / SHA_CA_BOUNDARY_CLAIM_RECORDS_PER_CALL_V1;
        let family = SHA_TERMINAL_CLAIM_FAMILIES_V1
            + (local % SHA_CA_BOUNDARY_CLAIM_RECORDS_PER_CALL_V1) / ZK_X509_SHA_BUS_LANES_V1;
        let lane = local % ZK_X509_SHA_BUS_LANES_V1;
        Some((call, family, lane))
    }

    fn claim_value_v1(&self, claim_index: usize) -> Option<F> {
        let (segment, family, lane) = Self::claim_address_v1(claim_index)?;
        match family {
            0 => self
                .segments
                .get(segment)?
                .source_products
                .get(lane)
                .copied(),
            1 => self
                .segments
                .get(segment)?
                .digest_products
                .get(lane)
                .copied(),
            2..=5 => self
                .segments
                .get(segment)?
                .rfc_stream_products
                .get(family - 2)
                .and_then(|stream| stream.get(lane))
                .copied(),
            6 => self
                .ca_calls
                .get(segment)?
                .source_start_products
                .get(lane)
                .copied(),
            7 => self
                .ca_calls
                .get(segment)?
                .digest_start_products
                .get(lane)
                .copied(),
            8 => self
                .ca_calls
                .get(segment)?
                .source_products
                .get(lane)
                .copied(),
            9 => self
                .ca_calls
                .get(segment)?
                .digest_products
                .get(lane)
                .copied(),
            _ => None,
        }
    }

    fn set_claim_value_v1(
        &mut self,
        claim_index: usize,
        value: F,
    ) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
        let (segment, family, lane) =
            Self::claim_address_v1(claim_index).ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
        let target = match family {
            0 => self
                .segments
                .get_mut(segment)
                .and_then(|terminal| terminal.source_products.get_mut(lane)),
            1 => self
                .segments
                .get_mut(segment)
                .and_then(|terminal| terminal.digest_products.get_mut(lane)),
            2..=5 => self
                .segments
                .get_mut(segment)
                .and_then(|terminal| terminal.rfc_stream_products.get_mut(family - 2))
                .and_then(|stream| stream.get_mut(lane)),
            6 => self
                .ca_calls
                .get_mut(segment)
                .and_then(|terminal| terminal.source_start_products.get_mut(lane)),
            7 => self
                .ca_calls
                .get_mut(segment)
                .and_then(|terminal| terminal.digest_start_products.get_mut(lane)),
            8 => self
                .ca_calls
                .get_mut(segment)
                .and_then(|terminal| terminal.source_products.get_mut(lane)),
            9 => self
                .ca_calls
                .get_mut(segment)
                .and_then(|terminal| terminal.digest_products.get_mut(lane)),
            _ => None,
        }
        .ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
        *target = value;
        Ok(())
    }

    fn validate_v1(&self) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
        for (expected_segment, terminal) in self.segments.iter().enumerate() {
            if usize::from(terminal.segment) != expected_segment
                || terminal
                    .source_products
                    .iter()
                    .chain(&terminal.digest_products)
                    .chain(terminal.rfc_stream_products.iter().flatten())
                    .any(|value| F::canonical(value.0).is_none())
            {
                return Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim);
            }
        }
        for (index, terminal) in self.ca_calls.iter().copied().enumerate() {
            terminal
                .validate_identity_v1(index)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
        }
        Ok(())
    }

    /// Return the 13 proof-derived call terminals consumed by the credential
    /// verifier in exact global call order.
    pub(crate) fn credential_call_terminals_v1(
        self,
    ) -> [ZkX509ShaCallTerminalV1; ZK_X509_SHA_CA_CALL_COUNT_V1] {
        self.ca_calls
            .map(ZkX509ShaCallBoundaryTerminalV1::terminal_v1)
    }

    /// Encode the sole canonical proof-carried SHA segment-terminal frame.
    pub(crate) fn encode_x5q1_v1(
        self,
    ) -> Result<[u8; ZK_X509_SHA_SEGMENT_TERMINAL_CLAIM_BYTES_V1], ZkX509Rfc5280StarkErrorV1> {
        self.validate_v1()?;
        let mut encoded = [0_u8; ZK_X509_SHA_SEGMENT_TERMINAL_CLAIM_BYTES_V1];
        encoded[..4].copy_from_slice(&SHA_TERMINAL_CLAIM_MAGIC_V1);
        encoded[4..6].copy_from_slice(&SHA_TERMINAL_CLAIM_VERSION_V1.to_be_bytes());
        encoded[6..8].copy_from_slice(&SHA_TERMINAL_CLAIM_ADAPTER_V1.to_be_bytes());
        encoded[8..10].copy_from_slice(&SHA_TERMINAL_CLAIM_INSTANCE_V1.to_be_bytes());
        encoded[10..12].copy_from_slice(
            &u16::try_from(SHA_TERMINAL_CLAIM_RECORDS_V1)
                .expect("SHA terminal claim count fits u16")
                .to_be_bytes(),
        );
        for claim_index in 0..SHA_TERMINAL_CLAIM_RECORDS_V1 {
            let (segment, family, lane) =
                Self::claim_address_v1(claim_index).expect("bounded SHA terminal claim");
            let address = if family < SHA_TERMINAL_CLAIM_FAMILIES_V1 {
                segment
            } else {
                ZK_X509_SHA_CA_LEAF_CALL_V1 + segment
            };
            let value = self
                .claim_value_v1(claim_index)
                .expect("bounded SHA terminal claim value");
            let start = SHA_TERMINAL_CLAIM_HEADER_BYTES_V1
                + claim_index * SHA_TERMINAL_CLAIM_RECORD_BYTES_V1;
            encoded[start..start + 2].copy_from_slice(
                &u16::try_from(address)
                    .expect("SHA terminal address fits u16")
                    .to_be_bytes(),
            );
            encoded[start + 2..start + 4].copy_from_slice(
                &u16::try_from(family + 1)
                    .expect("SHA terminal family fits u16")
                    .to_be_bytes(),
            );
            encoded[start + 4..start + 6].copy_from_slice(
                &u16::try_from(lane)
                    .expect("SHA terminal lane fits u16")
                    .to_be_bytes(),
            );
            encoded[start + 6..start + 8].copy_from_slice(&0_u16.to_be_bytes());
            encoded[start + 8..start + SHA_TERMINAL_CLAIM_RECORD_BYTES_V1]
                .copy_from_slice(&value.0.to_be_bytes());
        }
        Ok(encoded)
    }

    /// Decode an exact X5Q1 frame, rejecting every alternate address, order,
    /// field representation, length, and trailing byte.
    pub(crate) fn decode_x5q1_v1(encoded: &[u8]) -> Result<Self, ZkX509Rfc5280StarkErrorV1> {
        if encoded.len() != ZK_X509_SHA_SEGMENT_TERMINAL_CLAIM_BYTES_V1
            || encoded[..4] != SHA_TERMINAL_CLAIM_MAGIC_V1
            || u16::from_be_bytes(
                encoded[4..6]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            ) != SHA_TERMINAL_CLAIM_VERSION_V1
            || u16::from_be_bytes(
                encoded[6..8]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            ) != SHA_TERMINAL_CLAIM_ADAPTER_V1
            || u16::from_be_bytes(
                encoded[8..10]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            ) != SHA_TERMINAL_CLAIM_INSTANCE_V1
            || usize::from(u16::from_be_bytes(
                encoded[10..12]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            )) != SHA_TERMINAL_CLAIM_RECORDS_V1
        {
            return Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim);
        }

        let mut claims = Self {
            segments: core::array::from_fn(|segment| ZkX509ShaSegmentTerminalV1 {
                segment: u8::try_from(segment).expect("SHA segment fits u8"),
                source_products: [F::ZERO; ZK_X509_SHA_BUS_LANES_V1],
                digest_products: [F::ZERO; ZK_X509_SHA_BUS_LANES_V1],
                rfc_stream_products: [[F::ZERO; ZK_X509_SHA_BUS_LANES_V1];
                    SHA_TERMINAL_CLAIM_RFC_STREAMS_V1],
            }),
            ca_calls: core::array::from_fn(|index| {
                let (call, role) =
                    zk_x509_sha_ca_call_identity_v1(index).expect("canonical compact-CA call");
                ZkX509ShaCallBoundaryTerminalV1 {
                    call,
                    role,
                    source_start_products: [F::ZERO; ZK_X509_SHA_BUS_LANES_V1],
                    digest_start_products: [F::ZERO; ZK_X509_SHA_BUS_LANES_V1],
                    source_products: [F::ZERO; ZK_X509_SHA_BUS_LANES_V1],
                    digest_products: [F::ZERO; ZK_X509_SHA_BUS_LANES_V1],
                }
            }),
        };
        for claim_index in 0..SHA_TERMINAL_CLAIM_RECORDS_V1 {
            let (expected_segment, expected_family, expected_lane) =
                Self::claim_address_v1(claim_index).expect("bounded SHA terminal claim");
            let expected_address = if expected_family < SHA_TERMINAL_CLAIM_FAMILIES_V1 {
                expected_segment
            } else {
                ZK_X509_SHA_CA_LEAF_CALL_V1 + expected_segment
            };
            let start = SHA_TERMINAL_CLAIM_HEADER_BYTES_V1
                + claim_index * SHA_TERMINAL_CLAIM_RECORD_BYTES_V1;
            let actual_segment = usize::from(u16::from_be_bytes(
                encoded[start..start + 2]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            ));
            let actual_family = usize::from(u16::from_be_bytes(
                encoded[start + 2..start + 4]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            ));
            let actual_lane = usize::from(u16::from_be_bytes(
                encoded[start + 4..start + 6]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            ));
            let reserved = u16::from_be_bytes(
                encoded[start + 6..start + 8]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            );
            if actual_segment != expected_address
                || actual_family != expected_family + 1
                || actual_lane != expected_lane
                || reserved != 0
            {
                return Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim);
            }
            let raw = u64::from_be_bytes(
                encoded[start + 8..start + SHA_TERMINAL_CLAIM_RECORD_BYTES_V1]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            );
            let value = F::canonical(raw).ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
            claims.set_claim_value_v1(claim_index, value)?;
        }
        claims.validate_v1()?;
        Ok(claims)
    }
}

/// Compare proof-decoded X5Q1 claims with the four terminals already bound by
/// the SHA opened-row evaluator.
///
/// `committed` is verifier output, never witness metadata. The fixed record
/// order makes every mismatch a distinct residue without a host-selected
/// segment, family, or lane.
pub(crate) fn evaluate_zk_x509_sha_segment_terminal_claim_residues_v1(
    committed: [ZkX509ShaSegmentTerminalV1; ZK_X509_SHA_SEGMENT_COUNT_V1],
    committed_ca_calls: [ZkX509ShaCallBoundaryTerminalV1; ZK_X509_SHA_CA_CALL_COUNT_V1],
    claims: ZkX509ShaSegmentTerminalClaimsV1,
) -> Result<[F; SHA_TERMINAL_CLAIM_RECORDS_V1], ZkX509Rfc5280StarkErrorV1> {
    let committed =
        ZkX509ShaSegmentTerminalClaimsV1::from_sha_air_terminals_v1(committed, committed_ca_calls)?;
    claims.validate_v1()?;
    Ok(core::array::from_fn(|claim_index| {
        let expected = committed
            .claim_value_v1(claim_index)
            .expect("bounded committed SHA terminal");
        let claimed = claims
            .claim_value_v1(claim_index)
            .expect("bounded proof SHA terminal");
        expected.sub(claimed)
    }))
}

/// Decode X5Q1 and replay all 304 verifier-side terminal equalities.
pub(crate) fn replay_zk_x509_sha_segment_terminal_claims_v1(
    committed: [ZkX509ShaSegmentTerminalV1; ZK_X509_SHA_SEGMENT_COUNT_V1],
    committed_ca_calls: [ZkX509ShaCallBoundaryTerminalV1; ZK_X509_SHA_CA_CALL_COUNT_V1],
    encoded_claims: &[u8],
) -> Result<[F; SHA_TERMINAL_CLAIM_RECORDS_V1], ZkX509Rfc5280StarkErrorV1> {
    evaluate_zk_x509_sha_segment_terminal_claim_residues_v1(
        committed,
        committed_ca_calls,
        ZkX509ShaSegmentTerminalClaimsV1::decode_x5q1_v1(encoded_claims)?,
    )
}

const P256_TERMINAL_CLAIM_MAGIC_V1: [u8; 4] = *b"X5V1";
const P256_TERMINAL_CLAIM_VERSION_V1: u16 = 1;
const P256_TERMINAL_CLAIM_ADAPTER_V1: u16 = 5;
const P256_TERMINAL_CLAIM_INSTANCE_V1: u16 = 0;
const P256_TERMINAL_BUS_FAMILIES_V1: usize = 8;
const P256_TERMINAL_CERTIFICATE_CROSS_SOURCES_V1: usize = 4;
const P256_TERMINAL_WALLET_CROSS_SOURCES_V1: usize = 5;
const P256_TERMINAL_CROSS_START_V1: usize = 1;
const P256_TERMINAL_CROSS_TERMINAL_V1: usize = 2;
const P256_TERMINAL_FIRST_CROSS_FAMILY_V1: usize = P256_TERMINAL_BUS_FAMILIES_V1;
const P256_TERMINAL_SINK_FAMILY_V1: usize =
    P256_TERMINAL_FIRST_CROSS_FAMILY_V1 + P256_TERMINAL_WALLET_CROSS_SOURCES_V1;
const P256_TERMINAL_CERTIFICATE_RECORDS_V1: usize = P256_TERMINAL_BUS_FAMILIES_V1
    * P256_CROSS_TRACE_LANES_V1
    + P256_TERMINAL_CERTIFICATE_CROSS_SOURCES_V1 * 2 * P256_CROSS_TRACE_LANES_V1
    + P256_CROSS_TRACE_LANES_V1;
const P256_TERMINAL_WALLET_RECORDS_V1: usize = P256_TERMINAL_BUS_FAMILIES_V1
    * P256_CROSS_TRACE_LANES_V1
    + P256_TERMINAL_WALLET_CROSS_SOURCES_V1 * 2 * P256_CROSS_TRACE_LANES_V1
    + P256_CROSS_TRACE_LANES_V1;
const P256_TERMINAL_CLAIM_RECORDS_V1: usize = P256_X5S1_CERTIFICATE_OR_CRL_SIGNATURES_V1
    * P256_TERMINAL_CERTIFICATE_RECORDS_V1
    + P256_TERMINAL_WALLET_RECORDS_V1;
const P256_TERMINAL_CLAIM_RECORD_BYTES_V1: usize = 2 + 2 + 2 + 2 + 8;
const P256_TERMINAL_CLAIM_HEADER_BYTES_V1: usize = 4 + 2 + 2 + 2 + 2;
/// Exact X5V1 proof-carried five-signature P-256 terminal frame size.
pub(crate) const ZK_X509_P256_TERMINAL_CLAIM_BYTES_V1: usize = P256_TERMINAL_CLAIM_HEADER_BYTES_V1
    + P256_TERMINAL_CLAIM_RECORDS_V1 * P256_TERMINAL_CLAIM_RECORD_BYTES_V1;

const P256_CERTIFICATE_CROSS_ROLES_V1: [P256CrossTraceTerminalRoleV1;
    P256_TERMINAL_CERTIFICATE_CROSS_SOURCES_V1] = [
    P256CrossTraceTerminalRoleV1::ValueWriter,
    P256CrossTraceTerminalRoleV1::WindowBatch,
    P256CrossTraceTerminalRoleV1::DigestReduction,
    P256CrossTraceTerminalRoleV1::ResultXReduction,
];
const P256_WALLET_CROSS_ROLES_V1: [P256CrossTraceTerminalRoleV1;
    P256_TERMINAL_WALLET_CROSS_SOURCES_V1] = [
    P256CrossTraceTerminalRoleV1::ValueWriter,
    P256CrossTraceTerminalRoleV1::WindowBatch,
    P256CrossTraceTerminalRoleV1::DigestReduction,
    P256CrossTraceTerminalRoleV1::ResultXReduction,
    P256CrossTraceTerminalRoleV1::WalletLowS,
];

const _: () = assert!(P256_CROSS_TRACE_LANES_V1 == 4);
const _: () = assert!(P256_X5S1_CERTIFICATE_OR_CRL_SIGNATURES_V1 == 4);
const _: () = assert!(P256_X5S1_SIGNATURES_V1 == 5);
const _: () = assert!(P256_TERMINAL_CERTIFICATE_RECORDS_V1 == 68);
const _: () = assert!(P256_TERMINAL_WALLET_RECORDS_V1 == 76);
const _: () = assert!(P256_TERMINAL_CLAIM_RECORDS_V1 == 348);
const _: () = assert!(ZK_X509_P256_TERMINAL_CLAIM_BYTES_V1 == 5_580);

/// Exact proof terminals for one certificate-or-CRL P-256 equation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509P256CertificateTerminalClaimsV1 {
    /// Eight role-ordered four-lane bus terminals.
    pub(crate) buses: P256BusTerminalClaimsV1,
    /// Four verifier-owned cross-source roles, each with start and terminal.
    pub(crate) cross_sources:
        [P256CrossTraceTerminalClaimV1; P256_TERMINAL_CERTIFICATE_CROSS_SOURCES_V1],
    /// Independent four-lane binding sink.
    pub(crate) sink: [F; P256_CROSS_TRACE_LANES_V1],
}

/// Exact proof terminals for the wallet-ownership P-256 equation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509P256WalletTerminalClaimsV1 {
    /// Eight role-ordered four-lane bus terminals.
    pub(crate) buses: P256BusTerminalClaimsV1,
    /// Five verifier-owned cross-source roles, including wallet low-S.
    pub(crate) cross_sources:
        [P256CrossTraceTerminalClaimV1; P256_TERMINAL_WALLET_CROSS_SOURCES_V1],
    /// Independent four-lane binding sink.
    pub(crate) sink: [F; P256_CROSS_TRACE_LANES_V1],
}

/// Ordered proof claims for all five P-256 signature equations.
///
/// The first four entries are certificate-or-CRL equations and the final
/// entry is the wallet-ownership equation. X5V1 carries every bus endpoint,
/// every cross-source start and terminal, and every sink lane in the sole AIR
/// order. No role, signature, family, endpoint, or lane is prover-selected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509P256TerminalClaimsV1 {
    /// Certificate slots zero through two followed by the signed CRL.
    pub(crate) certificate_or_crl:
        [ZkX509P256CertificateTerminalClaimsV1; P256_X5S1_CERTIFICATE_OR_CRL_SIGNATURES_V1],
    /// Final wallet-ownership signature.
    pub(crate) wallet: ZkX509P256WalletTerminalClaimsV1,
}

fn empty_p256_bus_terminal_claims_v1() -> P256BusTerminalClaimsV1 {
    let zero = [F::ZERO; P256_CROSS_TRACE_LANES_V1];
    P256BusTerminalClaimsV1 {
        value_execution: zero,
        value_sorted: zero,
        value_arithmetic_copy: zero,
        arithmetic_value_copy: zero,
        arithmetic_scalar: zero,
        window_scalar: zero,
        scalar_bus_arithmetic: zero,
        scalar_bus_window: zero,
    }
}

fn p256_bus_terminal_claim_value_v1(
    buses: P256BusTerminalClaimsV1,
    family: usize,
    lane: usize,
) -> Option<F> {
    match family {
        0 => buses.value_execution.get(lane).copied(),
        1 => buses.value_sorted.get(lane).copied(),
        2 => buses.value_arithmetic_copy.get(lane).copied(),
        3 => buses.arithmetic_value_copy.get(lane).copied(),
        4 => buses.arithmetic_scalar.get(lane).copied(),
        5 => buses.window_scalar.get(lane).copied(),
        6 => buses.scalar_bus_arithmetic.get(lane).copied(),
        7 => buses.scalar_bus_window.get(lane).copied(),
        _ => None,
    }
}

fn set_p256_bus_terminal_claim_value_v1(
    buses: &mut P256BusTerminalClaimsV1,
    family: usize,
    lane: usize,
    value: F,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    let target = match family {
        0 => buses.value_execution.get_mut(lane),
        1 => buses.value_sorted.get_mut(lane),
        2 => buses.value_arithmetic_copy.get_mut(lane),
        3 => buses.arithmetic_value_copy.get_mut(lane),
        4 => buses.arithmetic_scalar.get_mut(lane),
        5 => buses.window_scalar.get_mut(lane),
        6 => buses.scalar_bus_arithmetic.get_mut(lane),
        7 => buses.scalar_bus_window.get_mut(lane),
        _ => None,
    }
    .ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
    *target = value;
    Ok(())
}

impl ZkX509P256TerminalClaimsV1 {
    #[cfg(test)]
    pub(crate) fn canonical_zero_for_test_v1() -> Self {
        Self::empty_v1()
    }

    /// Construct verifier-owned claims from the terminals already registered
    /// by all five P-256 AIR instances.
    ///
    /// Unlike proof decoding, this constructor also requires every native bus
    /// and cross-source equality to be satisfied. Thus malformed verifier
    /// material cannot be normalized into an apparently valid X5V1 binding.
    pub(crate) fn from_p256_air_terminals_v1(
        certificate_or_crl: [ZkX509P256CertificateTerminalClaimsV1;
            P256_X5S1_CERTIFICATE_OR_CRL_SIGNATURES_V1],
        wallet: ZkX509P256WalletTerminalClaimsV1,
    ) -> Result<Self, ZkX509Rfc5280StarkErrorV1> {
        let claims = Self {
            certificate_or_crl,
            wallet,
        };
        claims.validate_air_terminals_v1()?;
        Ok(claims)
    }

    fn empty_v1() -> Self {
        Self {
            certificate_or_crl: core::array::from_fn(|_| ZkX509P256CertificateTerminalClaimsV1 {
                buses: empty_p256_bus_terminal_claims_v1(),
                cross_sources: core::array::from_fn(|role| P256CrossTraceTerminalClaimV1 {
                    role: P256_CERTIFICATE_CROSS_ROLES_V1[role],
                    start: [F::ZERO; P256_CROSS_TRACE_LANES_V1],
                    terminal: [F::ZERO; P256_CROSS_TRACE_LANES_V1],
                }),
                sink: [F::ZERO; P256_CROSS_TRACE_LANES_V1],
            }),
            wallet: ZkX509P256WalletTerminalClaimsV1 {
                buses: empty_p256_bus_terminal_claims_v1(),
                cross_sources: core::array::from_fn(|role| P256CrossTraceTerminalClaimV1 {
                    role: P256_WALLET_CROSS_ROLES_V1[role],
                    start: [F::ZERO; P256_CROSS_TRACE_LANES_V1],
                    terminal: [F::ZERO; P256_CROSS_TRACE_LANES_V1],
                }),
                sink: [F::ZERO; P256_CROSS_TRACE_LANES_V1],
            },
        }
    }

    fn signature_role_v1(signature: usize) -> Option<P256EcdsaRoleV1> {
        if signature < P256_X5S1_CERTIFICATE_OR_CRL_SIGNATURES_V1 {
            Some(P256EcdsaRoleV1::CertificateOrCrl)
        } else if signature == P256_X5S1_SIGNATURES_V1 - 1 {
            Some(P256EcdsaRoleV1::WalletOwnership)
        } else {
            None
        }
    }

    fn signature_parts_v1(
        &self,
        signature: usize,
    ) -> Option<(
        P256BusTerminalClaimsV1,
        &[P256CrossTraceTerminalClaimV1],
        [F; P256_CROSS_TRACE_LANES_V1],
    )> {
        if signature < P256_X5S1_CERTIFICATE_OR_CRL_SIGNATURES_V1 {
            let terminals = self.certificate_or_crl.get(signature)?;
            Some((terminals.buses, &terminals.cross_sources, terminals.sink))
        } else if signature == P256_X5S1_SIGNATURES_V1 - 1 {
            Some((
                self.wallet.buses,
                &self.wallet.cross_sources,
                self.wallet.sink,
            ))
        } else {
            None
        }
    }

    fn signature_parts_mut_v1(
        &mut self,
        signature: usize,
    ) -> Option<(
        &mut P256BusTerminalClaimsV1,
        &mut [P256CrossTraceTerminalClaimV1],
        &mut [F; P256_CROSS_TRACE_LANES_V1],
    )> {
        if signature < P256_X5S1_CERTIFICATE_OR_CRL_SIGNATURES_V1 {
            let terminals = self.certificate_or_crl.get_mut(signature)?;
            Some((
                &mut terminals.buses,
                &mut terminals.cross_sources,
                &mut terminals.sink,
            ))
        } else if signature == P256_X5S1_SIGNATURES_V1 - 1 {
            Some((
                &mut self.wallet.buses,
                &mut self.wallet.cross_sources,
                &mut self.wallet.sink,
            ))
        } else {
            None
        }
    }

    fn claim_address_v1(claim_index: usize) -> Option<(usize, usize, usize, usize)> {
        if claim_index >= P256_TERMINAL_CLAIM_RECORDS_V1 {
            return None;
        }
        let certificate_records =
            P256_X5S1_CERTIFICATE_OR_CRL_SIGNATURES_V1 * P256_TERMINAL_CERTIFICATE_RECORDS_V1;
        let (signature, local, cross_sources) = if claim_index < certificate_records {
            (
                claim_index / P256_TERMINAL_CERTIFICATE_RECORDS_V1,
                claim_index % P256_TERMINAL_CERTIFICATE_RECORDS_V1,
                P256_TERMINAL_CERTIFICATE_CROSS_SOURCES_V1,
            )
        } else {
            (
                P256_X5S1_SIGNATURES_V1 - 1,
                claim_index - certificate_records,
                P256_TERMINAL_WALLET_CROSS_SOURCES_V1,
            )
        };
        let bus_records = P256_TERMINAL_BUS_FAMILIES_V1 * P256_CROSS_TRACE_LANES_V1;
        if local < bus_records {
            return Some((
                signature,
                local / P256_CROSS_TRACE_LANES_V1,
                local % P256_CROSS_TRACE_LANES_V1,
                0,
            ));
        }
        let cross_local = local - bus_records;
        let cross_records = cross_sources * 2 * P256_CROSS_TRACE_LANES_V1;
        if cross_local < cross_records {
            let role = cross_local / (2 * P256_CROSS_TRACE_LANES_V1);
            let role_local = cross_local % (2 * P256_CROSS_TRACE_LANES_V1);
            return Some((
                signature,
                P256_TERMINAL_FIRST_CROSS_FAMILY_V1 + role,
                role_local % P256_CROSS_TRACE_LANES_V1,
                P256_TERMINAL_CROSS_START_V1 + role_local / P256_CROSS_TRACE_LANES_V1,
            ));
        }
        let sink_lane = cross_local - cross_records;
        (sink_lane < P256_CROSS_TRACE_LANES_V1).then_some((
            signature,
            P256_TERMINAL_SINK_FAMILY_V1,
            sink_lane,
            0,
        ))
    }

    fn claim_value_v1(&self, claim_index: usize) -> Option<F> {
        let (signature, family, lane, endpoint) = Self::claim_address_v1(claim_index)?;
        let (buses, cross_sources, sink) = self.signature_parts_v1(signature)?;
        if family < P256_TERMINAL_BUS_FAMILIES_V1 && endpoint == 0 {
            return p256_bus_terminal_claim_value_v1(buses, family, lane);
        }
        if (P256_TERMINAL_FIRST_CROSS_FAMILY_V1..P256_TERMINAL_SINK_FAMILY_V1).contains(&family) {
            let source = cross_sources.get(family - P256_TERMINAL_FIRST_CROSS_FAMILY_V1)?;
            return match endpoint {
                P256_TERMINAL_CROSS_START_V1 => source.start.get(lane).copied(),
                P256_TERMINAL_CROSS_TERMINAL_V1 => source.terminal.get(lane).copied(),
                _ => None,
            };
        }
        if family == P256_TERMINAL_SINK_FAMILY_V1 && endpoint == 0 {
            return sink.get(lane).copied();
        }
        None
    }

    fn set_claim_value_v1(
        &mut self,
        claim_index: usize,
        value: F,
    ) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
        let (signature, family, lane, endpoint) =
            Self::claim_address_v1(claim_index).ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
        let (buses, cross_sources, sink) = self
            .signature_parts_mut_v1(signature)
            .ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
        if family < P256_TERMINAL_BUS_FAMILIES_V1 && endpoint == 0 {
            return set_p256_bus_terminal_claim_value_v1(buses, family, lane, value);
        }
        if (P256_TERMINAL_FIRST_CROSS_FAMILY_V1..P256_TERMINAL_SINK_FAMILY_V1).contains(&family) {
            let source = cross_sources
                .get_mut(family - P256_TERMINAL_FIRST_CROSS_FAMILY_V1)
                .ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
            let target = match endpoint {
                P256_TERMINAL_CROSS_START_V1 => source.start.get_mut(lane),
                P256_TERMINAL_CROSS_TERMINAL_V1 => source.terminal.get_mut(lane),
                _ => None,
            }
            .ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
            *target = value;
            return Ok(());
        }
        if family == P256_TERMINAL_SINK_FAMILY_V1 && endpoint == 0 {
            *sink
                .get_mut(lane)
                .ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)? = value;
            return Ok(());
        }
        Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
    }

    fn validate_topology_v1(&self) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
        for signature in 0..P256_X5S1_SIGNATURES_V1 {
            let role = Self::signature_role_v1(signature)
                .ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
            let (buses, cross_sources, sink) = self
                .signature_parts_v1(signature)
                .ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
            evaluate_p256_bus_terminal_claim_equalities_v1(buses)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
            evaluate_p256_cross_trace_terminal_claim_equalities_v1(role, cross_sources, sink)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
        }
        Ok(())
    }

    fn validate_air_terminals_v1(&self) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
        self.validate_topology_v1()?;
        for signature in 0..P256_X5S1_SIGNATURES_V1 {
            let role = Self::signature_role_v1(signature)
                .ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
            let (buses, cross_sources, sink) = self
                .signature_parts_v1(signature)
                .ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
            let bus_residues = evaluate_p256_bus_terminal_claim_equalities_v1(buses)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
            let cross_residues =
                evaluate_p256_cross_trace_terminal_claim_equalities_v1(role, cross_sources, sink)
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
            if bus_residues
                .iter()
                .chain(&cross_residues)
                .any(|residue| *residue != F::ZERO)
            {
                return Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim);
            }
        }
        Ok(())
    }

    /// Encode the sole canonical proof-carried P-256 terminal frame.
    pub(crate) fn encode_x5v1_v1(
        self,
    ) -> Result<[u8; ZK_X509_P256_TERMINAL_CLAIM_BYTES_V1], ZkX509Rfc5280StarkErrorV1> {
        self.validate_topology_v1()?;
        let mut encoded = [0_u8; ZK_X509_P256_TERMINAL_CLAIM_BYTES_V1];
        encoded[..4].copy_from_slice(&P256_TERMINAL_CLAIM_MAGIC_V1);
        encoded[4..6].copy_from_slice(&P256_TERMINAL_CLAIM_VERSION_V1.to_be_bytes());
        encoded[6..8].copy_from_slice(&P256_TERMINAL_CLAIM_ADAPTER_V1.to_be_bytes());
        encoded[8..10].copy_from_slice(&P256_TERMINAL_CLAIM_INSTANCE_V1.to_be_bytes());
        encoded[10..12].copy_from_slice(
            &u16::try_from(P256_TERMINAL_CLAIM_RECORDS_V1)
                .expect("P-256 terminal claim count fits u16")
                .to_be_bytes(),
        );
        for claim_index in 0..P256_TERMINAL_CLAIM_RECORDS_V1 {
            let (signature, family, lane, endpoint) =
                Self::claim_address_v1(claim_index).expect("bounded P-256 terminal claim");
            let value = self
                .claim_value_v1(claim_index)
                .expect("bounded P-256 terminal claim value");
            let start = P256_TERMINAL_CLAIM_HEADER_BYTES_V1
                + claim_index * P256_TERMINAL_CLAIM_RECORD_BYTES_V1;
            encoded[start..start + 2].copy_from_slice(
                &u16::try_from(signature)
                    .expect("P-256 signature fits u16")
                    .to_be_bytes(),
            );
            encoded[start + 2..start + 4].copy_from_slice(
                &u16::try_from(family + 1)
                    .expect("P-256 terminal family fits u16")
                    .to_be_bytes(),
            );
            encoded[start + 4..start + 6].copy_from_slice(
                &u16::try_from(lane)
                    .expect("P-256 terminal lane fits u16")
                    .to_be_bytes(),
            );
            encoded[start + 6..start + 8].copy_from_slice(
                &u16::try_from(endpoint)
                    .expect("P-256 terminal endpoint fits u16")
                    .to_be_bytes(),
            );
            encoded[start + 8..start + P256_TERMINAL_CLAIM_RECORD_BYTES_V1]
                .copy_from_slice(&value.0.to_be_bytes());
        }
        Ok(encoded)
    }

    /// Decode an exact X5V1 frame, rejecting every alternate role, address,
    /// order, field representation, length, and trailing byte.
    pub(crate) fn decode_x5v1_v1(encoded: &[u8]) -> Result<Self, ZkX509Rfc5280StarkErrorV1> {
        if encoded.len() != ZK_X509_P256_TERMINAL_CLAIM_BYTES_V1
            || encoded[..4] != P256_TERMINAL_CLAIM_MAGIC_V1
            || u16::from_be_bytes(
                encoded[4..6]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            ) != P256_TERMINAL_CLAIM_VERSION_V1
            || u16::from_be_bytes(
                encoded[6..8]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            ) != P256_TERMINAL_CLAIM_ADAPTER_V1
            || u16::from_be_bytes(
                encoded[8..10]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            ) != P256_TERMINAL_CLAIM_INSTANCE_V1
            || usize::from(u16::from_be_bytes(
                encoded[10..12]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            )) != P256_TERMINAL_CLAIM_RECORDS_V1
        {
            return Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim);
        }

        let mut claims = Self::empty_v1();
        for claim_index in 0..P256_TERMINAL_CLAIM_RECORDS_V1 {
            let (expected_signature, expected_family, expected_lane, expected_endpoint) =
                Self::claim_address_v1(claim_index).expect("bounded P-256 terminal claim");
            let start = P256_TERMINAL_CLAIM_HEADER_BYTES_V1
                + claim_index * P256_TERMINAL_CLAIM_RECORD_BYTES_V1;
            let actual_signature = usize::from(u16::from_be_bytes(
                encoded[start..start + 2]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            ));
            let actual_family = usize::from(u16::from_be_bytes(
                encoded[start + 2..start + 4]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            ));
            let actual_lane = usize::from(u16::from_be_bytes(
                encoded[start + 4..start + 6]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            ));
            let actual_endpoint = usize::from(u16::from_be_bytes(
                encoded[start + 6..start + 8]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            ));
            if actual_signature != expected_signature
                || actual_family != expected_family + 1
                || actual_lane != expected_lane
                || actual_endpoint != expected_endpoint
            {
                return Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim);
            }
            let raw = u64::from_be_bytes(
                encoded[start + 8..start + P256_TERMINAL_CLAIM_RECORD_BYTES_V1]
                    .try_into()
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            );
            claims.set_claim_value_v1(
                claim_index,
                F::canonical(raw).ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?,
            )?;
        }
        claims.validate_topology_v1()?;
        Ok(claims)
    }
}

/// Compare proof-decoded X5V1 claims with the terminals already bound by all
/// five P-256 opened-row evaluators.
///
/// `committed` is verifier output and must itself satisfy the existing P-256
/// bus and cross-source terminal equalities. Every proof mismatch then has one
/// fixed equality residue, independent of all host metadata.
pub(crate) fn evaluate_zk_x509_p256_terminal_claim_residues_v1(
    committed: ZkX509P256TerminalClaimsV1,
    claims: ZkX509P256TerminalClaimsV1,
) -> Result<[F; P256_TERMINAL_CLAIM_RECORDS_V1], ZkX509Rfc5280StarkErrorV1> {
    committed.validate_air_terminals_v1()?;
    claims.validate_topology_v1()?;
    Ok(core::array::from_fn(|claim_index| {
        let expected = committed
            .claim_value_v1(claim_index)
            .expect("bounded committed P-256 terminal");
        let claimed = claims
            .claim_value_v1(claim_index)
            .expect("bounded proof P-256 terminal");
        expected.sub(claimed)
    }))
}

/// Decode X5V1 and replay all 348 verifier-side P-256 terminal equalities.
pub(crate) fn replay_zk_x509_p256_terminal_claims_v1(
    committed: ZkX509P256TerminalClaimsV1,
    encoded_claims: &[u8],
) -> Result<[F; P256_TERMINAL_CLAIM_RECORDS_V1], ZkX509Rfc5280StarkErrorV1> {
    evaluate_zk_x509_p256_terminal_claim_residues_v1(
        committed,
        ZkX509P256TerminalClaimsV1::decode_x5v1_v1(encoded_claims)?,
    )
}

fn pack_bits_v1(bits: &[F]) -> F {
    bits.iter()
        .copied()
        .enumerate()
        .fold(F::ZERO, |sum, (bit, value)| {
            sum.add(value.mul(F(1_u64 << bit)))
        })
}

fn push_boolean_v1(residues: &mut Vec<F>, gate: F, value: F) {
    residues.push(gate.mul(value).mul(value.sub(F::ONE)));
}

fn push_gated_zero_safe_inverse_v1(
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

fn push_reused_gated_zero_safe_inverse_v1(
    residues: &mut Vec<F>,
    gate: F,
    denominator: F,
    zero: F,
    inverse: F,
) {
    // These helper cells are shared by several mutually exclusive row
    // families. Canonicalize the zero flag and inverse only while this family
    // is active; another family may legitimately assign both cells.
    residues.push(gate.mul(zero).mul(zero.sub(F::ONE)));
    residues.push(gate.mul(denominator).mul(zero));
    residues.push(gate.mul(denominator.mul(inverse).sub(F::ONE.sub(zero))));
    residues.push(gate.mul(zero).mul(inverse));
}

#[inline]
fn assert_residue_section_v1(
    residues_len: usize,
    section_start: &mut usize,
    section: (&'static str, usize),
) {
    debug_assert_eq!(
        residues_len - *section_start,
        section.1,
        "RFC 5280 residue section `{}` drifted",
        section.0
    );
    *section_start = residues_len;
}

fn private_geometry_residues_v1(
    current: &ZkX509Rfc5280StarkBaseRowV1,
    next: &ZkX509Rfc5280StarkBaseRowV1,
    fixed: &ZkX509Rfc5280StarkFixedRowV1,
) -> Vec<F> {
    let mut residues = Vec::with_capacity(121);
    let active = current[BASE_ACTIVE];
    let cert2_active = current[BASE_CERT2_ACTIVE];
    push_boolean_v1(&mut residues, F::ONE, active);
    push_boolean_v1(&mut residues, F::ONE, cert2_active);
    residues.push(
        fixed[FIX_CERT2_OUTPUT]
            .mul(active)
            .mul(current[BASE_VALUE].sub(cert2_active)),
    );
    residues.push(
        fixed[FIX_ACTIVATION_CONTINUE]
            .mul(next[BASE_ACTIVE])
            .mul(F::ONE.sub(active)),
    );
    residues.push(fixed[FIX_REQUIRED_ACTIVE].mul(active.sub(F::ONE)));
    residues.push(fixed[FIX_CERT2_SLOT_FIRST].mul(active.sub(cert2_active)));
    let decimal_grouped = family_gate_v1(fixed, ZkX509Rfc5280StarkFamilyV1::Decimal);
    let fixed_grouped = family_gate_v1(fixed, ZkX509Rfc5280StarkFamilyV1::SerialSource)
        .add(family_gate_v1(
            fixed,
            ZkX509Rfc5280StarkFamilyV1::SerialCompare,
        ))
        .add(family_gate_v1(fixed, ZkX509Rfc5280StarkFamilyV1::RangeByte));
    residues.push(
        fixed_grouped
            .mul(active)
            .mul(F::ONE.sub(next[BASE_ACTIVE]))
            .mul(F::ONE.sub(fixed[FIX_LOCAL_LAST])),
    );
    residues.push(
        decimal_grouped
            .mul(active)
            .mul(F::ONE.sub(next[BASE_ACTIVE]))
            .mul(F::ONE.sub(current[BASE_STRICT])),
    );
    let padding = family_gate_v1(fixed, ZkX509Rfc5280StarkFamilyV1::Padding);
    residues.push(padding.mul(active));
    for (column, value) in current.iter().copied().enumerate() {
        if !matches!(column, BASE_ACTIVE | BASE_CERT2_ACTIVE) {
            residues.push(F::ONE.sub(active).mul(value));
        }
    }
    residues.push(fixed[FIX_CONTINUE].mul(next[BASE_CERT2_ACTIVE].sub(cert2_active)));
    debug_assert_eq!(residues.len(), 121);
    residues
}

fn family_gate_v1(fixed: &ZkX509Rfc5280StarkFixedRowV1, family: ZkX509Rfc5280StarkFamilyV1) -> F {
    fixed[family as usize]
}

fn active_family_gate_v1(
    current: &ZkX509Rfc5280StarkBaseRowV1,
    fixed: &ZkX509Rfc5280StarkFixedRowV1,
    family: ZkX509Rfc5280StarkFamilyV1,
) -> F {
    current[BASE_ACTIVE].mul(family_gate_v1(fixed, family))
}

fn rfc_row_factor_v1(
    domain: u64,
    current: &ZkX509Rfc5280StarkBaseRowV1,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> F {
    compress_tuple_v1(
        [
            F(domain),
            current[BASE_DOCUMENT],
            current[BASE_ADDRESS],
            current[BASE_VALUE],
            current[BASE_ROLE],
            current[BASE_INSTANCE],
            current[BASE_OFFSET],
            current[BASE_ENDPOINT_ROLE],
            current[BASE_ENDPOINT_INSTANCE],
            current[BASE_IS_WRITE],
            current[BASE_A],
            current[BASE_B],
        ],
        challenges.tuple[lane],
    )
}

fn output_row_factor_v1(
    current: &ZkX509Rfc5280StarkBaseRowV1,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> F {
    compress_tuple_v1(
        [
            F(80),
            current[BASE_ROLE],
            current[BASE_INSTANCE],
            current[BASE_ENDPOINT_ROLE],
            current[BASE_ENDPOINT_INSTANCE],
            current[BASE_OFFSET],
            current[BASE_VALUE],
            current[BASE_IS_WRITE],
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
        ],
        challenges.tuple[lane],
    )
}

fn serial_copy_factor_v1(
    logical_id: F,
    offset: F,
    value: F,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> F {
    compress_tuple_v1(
        [
            F(90),
            logical_id,
            offset,
            value,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
        ],
        challenges.tuple[lane],
    )
}

fn normalized_copy_factor_v1(
    current: &ZkX509Rfc5280StarkBaseRowV1,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> F {
    compress_tuple_v1(
        [
            current[BASE_COPY_DOMAIN],
            current[BASE_COPY_KEY_1],
            current[BASE_COPY_KEY_2],
            current[BASE_COPY_VALUE],
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
        ],
        challenges.tuple[lane],
    )
}

fn decimal_calendar_factor_v1(
    time_instance: F,
    component: F,
    value: F,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> F {
    compress_tuple_v1(
        [
            F(98),
            time_instance,
            component,
            value,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
        ],
        challenges.tuple[lane],
    )
}

fn relation_range_factor_v1(
    relation: F,
    instance: F,
    slack: F,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> F {
    compress_tuple_v1(
        [
            F(99),
            relation,
            instance,
            slack,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
        ],
        challenges.tuple[lane],
    )
}

fn serial_byte_lookup_factor_v1(
    document: F,
    address: F,
    value: F,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> F {
    compress_tuple_v1(
        [
            F(91),
            document,
            address,
            value,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
        ],
        challenges.tuple[lane],
    )
}

fn profile_byte_factor_v1(
    current: &ZkX509Rfc5280StarkBaseRowV1,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> F {
    compress_tuple_v1(
        [
            F(96),
            current[BASE_ROLE],
            current[BASE_ENDPOINT_ROLE],
            current[BASE_PARENT],
            current[BASE_OFFSET],
            current[BASE_B],
            current[BASE_VALUE],
            current[BASE_ENDPOINT_INSTANCE],
            current[BASE_CHILD],
            F::ZERO,
            F::ZERO,
            F::ZERO,
        ],
        challenges.tuple[lane],
    )
}

fn profile_topology_source_factor_v1(
    current: &ZkX509Rfc5280StarkBaseRowV1,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> F {
    compress_tuple_v1(
        [
            F(97),
            current[BASE_ROLE],
            current[BASE_DOCUMENT],
            current[BASE_NODE],
            current[BASE_START],
            current[BASE_CONTENT_START],
            current[BASE_CONTENT_END],
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
        ],
        challenges.tuple[lane],
    )
}

fn profile_topology_query_factor_v1(
    current: &ZkX509Rfc5280StarkBaseRowV1,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> F {
    compress_tuple_v1(
        [
            F(97),
            current[BASE_PARENT],
            current[BASE_DOCUMENT],
            current[BASE_NODE],
            current[BASE_CONTENT_START],
            current[BASE_DEPTH],
            current[BASE_TAG_NUMBER],
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
        ],
        challenges.tuple[lane],
    )
}

fn serial_node_lookup_factor_v1(
    current: &ZkX509Rfc5280StarkBaseRowV1,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> F {
    compress_tuple_v1(
        [
            F(92),
            current[BASE_DOCUMENT],
            current[BASE_NODE],
            current[BASE_START],
            current[BASE_CONTENT_START],
            current[BASE_CONTENT_END],
            current[BASE_TAG_CLASS],
            current[BASE_CONSTRUCTED],
            current[BASE_TAG_NUMBER],
            current[BASE_ROLE],
            current[BASE_INSTANCE],
            current[BASE_A],
        ],
        challenges.tuple[lane],
    )
}

fn grammar_rule_table_factor_v1(
    fixed: &ZkX509Rfc5280StarkFixedRowV1,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> F {
    compress_tuple_v1(
        [
            F(93),
            fixed[FIX_EXPECTED],
            fixed[FIX_EXPECTED + 1],
            fixed[FIX_EXPECTED + 2],
            fixed[FIX_EXPECTED + 3],
            fixed[FIX_EXPECTED + 4],
            fixed[FIX_EXPECTED + 5],
            fixed[FIX_EXPECTED + 6],
            fixed[FIX_EXPECTED + 7],
            fixed[FIX_EXPECTED + 8],
            fixed[FIX_EXPECTED + 9],
            fixed[FIX_GRAMMAR_ORDINAL_FIRST],
        ],
        challenges.tuple[lane],
    )
}

fn grammar_rule_query_factor_v1(
    current: &ZkX509Rfc5280StarkBaseRowV1,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> F {
    compress_tuple_v1(
        [
            F(93),
            current[BASE_B],
            current[BASE_ROLE],
            grammar_tag_pack_v1(
                current[BASE_TAG_CLASS],
                current[BASE_CONSTRUCTED],
                current[BASE_TAG_NUMBER],
            ),
            grammar_ordinal_pack_v1(
                current[BASE_IS_WRITE],
                current[BASE_STRICT],
                current[BASE_STATE_BEFORE],
            ),
            grammar_count_pack_v1(current[BASE_EQUAL], current[BASE_STATE_AFTER]),
            current[BASE_OFFSET],
            current[BASE_ENDPOINT_ROLE],
            current[BASE_ENDPOINT_INSTANCE],
            current[BASE_H],
            current[BASE_E],
            current[BASE_INVERSE],
        ],
        challenges.tuple[lane],
    )
}

fn grammar_parent_table_factor_v1(
    current: &ZkX509Rfc5280StarkBaseRowV1,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> F {
    compress_tuple_v1(
        [
            F(94),
            current[BASE_DOCUMENT],
            current[BASE_NODE],
            current[BASE_ROLE],
            current[BASE_INSTANCE],
            current[BASE_D],
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
        ],
        challenges.tuple[lane],
    )
}

fn grammar_parent_query_factor_v1(
    current: &ZkX509Rfc5280StarkBaseRowV1,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> F {
    compress_tuple_v1(
        [
            F(94),
            current[BASE_DOCUMENT],
            current[BASE_PARENT],
            current[BASE_B],
            current[BASE_C],
            current[BASE_G],
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
        ],
        challenges.tuple[lane],
    )
}

fn grammar_ordinal_factor_v1(
    document: F,
    parent: F,
    child: F,
    child_count: F,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> F {
    compress_tuple_v1(
        [
            F(95),
            document,
            parent,
            child,
            child_count,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
        ],
        challenges.tuple[lane],
    )
}

/// Challenge-independent canonical RFC 5280 trace material.
///
/// Rows are stored by fixed family and ordinal. Sparse fixed-capacity gaps are
/// represented by absent vector entries and replay as the unique inactive
/// zero row (apart from the carried private-depth selector). This keeps the
/// prover bounded without allocating a `2^18 × 66` matrix.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct ZkX509Rfc5280StarkBaseMaterialV1 {
    pub(crate) private_shape: ZkX509Rfc5280StarkPrivateShapeV1,
    pub(crate) schedule: ZkX509Rfc5280StarkFixedScheduleV1,
    family_rows: [Vec<ZkX509Rfc5280StarkBaseRowV1>; FAMILY_COUNT_V1],
}

impl core::fmt::Debug for ZkX509Rfc5280StarkBaseMaterialV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZkX509Rfc5280StarkBaseMaterialV1 { <private material redacted> }")
    }
}

impl ZkX509Rfc5280StarkBaseMaterialV1 {
    /// Recursively overwrite every private shape cell and committed field row.
    pub(crate) fn zeroize_private_v1(&mut self) {
        self.private_shape.chain_depth = 0;
        self.private_shape.certificate_slot_2_active = F::ZERO;
        self.private_shape.top_document_count = 0;
        self.private_shape.top_document_lengths.fill(0);
        self.private_shape.top_node_counts.fill(0);
        self.private_shape.embedded_document_count = 0;
        self.private_shape.embedded_document_lengths.fill(0);
        self.private_shape.embedded_node_counts.fill(0);
        self.private_shape.crl_entries = 0;
        self.private_shape.disclosed_attributes = 0;
        self.private_shape.embedded_copy_rows = 0;
        self.private_shape.grammar_rows = 0;
        self.private_shape.fixed_byte_rows = 0;
        self.private_shape.equality_rows = 0;
        self.private_shape.decimal_rows = 0;
        self.private_shape.calendar_rows = 0;
        self.private_shape.relation_rows = 0;
        self.private_shape.bit_flag_rows = 0;
        self.private_shape.serial_source_rows = 0;
        self.private_shape.serial_rows = 0;
        self.private_shape.range_rows = 0;
        self.private_shape.semantic_source_rows = 0;
        self.private_shape.semantic_consumer_rows = 0;
        self.private_shape.output_producer_rows = 0;
        self.private_shape.output_consumer_rows = 0;
        self.private_shape.io_channels = 0;
        for family in &mut self.family_rows {
            for row in &mut *family {
                row.fill(F::ZERO);
            }
            family.clear();
        }
    }

    #[cfg(test)]
    pub(crate) fn private_is_zeroized_v1(&self) -> bool {
        let shape = &self.private_shape;
        shape.chain_depth == 0
            && shape.certificate_slot_2_active == F::ZERO
            && shape.top_document_count == 0
            && shape.top_document_lengths.iter().all(|value| *value == 0)
            && shape.top_node_counts.iter().all(|value| *value == 0)
            && shape.embedded_document_count == 0
            && shape
                .embedded_document_lengths
                .iter()
                .all(|value| *value == 0)
            && shape.embedded_node_counts.iter().all(|value| *value == 0)
            && shape.crl_entries == 0
            && shape.disclosed_attributes == 0
            && shape.embedded_copy_rows == 0
            && shape.grammar_rows == 0
            && shape.fixed_byte_rows == 0
            && shape.equality_rows == 0
            && shape.decimal_rows == 0
            && shape.calendar_rows == 0
            && shape.relation_rows == 0
            && shape.bit_flag_rows == 0
            && shape.serial_source_rows == 0
            && shape.serial_rows == 0
            && shape.range_rows == 0
            && shape.semantic_source_rows == 0
            && shape.semantic_consumer_rows == 0
            && shape.output_producer_rows == 0
            && shape.output_consumer_rows == 0
            && shape.io_channels == 0
            && self.family_rows.iter().all(Vec::is_empty)
    }
}

fn active_zero_row_v1() -> ZkX509Rfc5280StarkBaseRowV1 {
    let mut row = [F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1];
    row[BASE_ACTIVE] = F::ONE;
    row
}

fn byte_row_v1(document: u64, address: u64, value: u8) -> ZkX509Rfc5280StarkBaseRowV1 {
    let mut row = active_zero_row_v1();
    row[BASE_VALUE] = F(u64::from(value));
    write_u8_bits_v1(&mut row, BASE_BYTE_BITS, value);
    row[BASE_DOCUMENT] = F(document);
    row[BASE_ADDRESS] = F(address);
    row
}

fn ensure_family_slot_v1(
    rows: &mut Vec<ZkX509Rfc5280StarkBaseRowV1>,
    ordinal: usize,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    let length = ordinal
        .checked_add(1)
        .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
    if rows.len() < length {
        rows.try_reserve(length - rows.len())
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
        rows.resize(length, [F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1]);
    }
    Ok(())
}

fn push_family_row_v1(
    rows: &mut Vec<ZkX509Rfc5280StarkBaseRowV1>,
    row: ZkX509Rfc5280StarkBaseRowV1,
) -> Result<(), ZkX509Rfc5280StarkErrorV1> {
    rows.try_reserve(1)
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    rows.push(row);
    Ok(())
}

fn canonical_time_nodes_v1(trace: &ZkX509Rfc5280TraceV1) -> Vec<ZkX509Rfc5280NodeProvenanceV1> {
    role_nodes_v1(trace, ZkX509Rfc5280GrammarRoleV1::CertificateNotBefore)
        .chain(role_nodes_v1(
            trace,
            ZkX509Rfc5280GrammarRoleV1::CertificateNotAfter,
        ))
        .chain(role_nodes_v1(
            trace,
            ZkX509Rfc5280GrammarRoleV1::CrlThisUpdate,
        ))
        .chain(role_nodes_v1(
            trace,
            ZkX509Rfc5280GrammarRoleV1::CrlNextUpdate,
        ))
        .chain(role_nodes_v1(
            trace,
            ZkX509Rfc5280GrammarRoleV1::CrlEntryTime,
        ))
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CalendarOperandsV1 {
    timestamp: u64,
    year: u64,
    month: u64,
    day: u64,
    hour: u64,
    minute: u64,
    second: u64,
}

fn calendar_operands_v1(
    cells: &[ZkX509Rfc5280SourceCellV1],
    tag: u32,
) -> Result<CalendarOperandsV1, ZkX509Rfc5280StarkErrorV1> {
    let (timestamp, decimal) = parse_time_cells_v1(cells, tag)?;
    let bytes = decimal.iter().map(|cell| cell.value).collect::<Vec<_>>();
    let (year, offset) = match tag {
        23 => {
            let short = u64::from(parse_decimal_v1(&bytes[..2])?);
            (
                if short >= 50 {
                    1900 + short
                } else {
                    2000 + short
                },
                2,
            )
        }
        24 => (u64::from(parse_decimal_v1(&bytes[..4])?), 4),
        _ => return Err(ZkX509Rfc5280StarkErrorV1::Semantic),
    };
    Ok(CalendarOperandsV1 {
        timestamp,
        year,
        month: u64::from(parse_decimal_v1(&bytes[offset..offset + 2])?),
        day: u64::from(parse_decimal_v1(&bytes[offset + 2..offset + 4])?),
        hour: u64::from(parse_decimal_v1(&bytes[offset + 4..offset + 6])?),
        minute: u64::from(parse_decimal_v1(&bytes[offset + 6..offset + 8])?),
        second: u64::from(parse_decimal_v1(&bytes[offset + 8..offset + 10])?),
    })
}

fn calendar_row_v1(
    operands: CalendarOperandsV1,
) -> Result<ZkX509Rfc5280StarkBaseRowV1, ZkX509Rfc5280StarkErrorV1> {
    const MONTH_PREFIX: [u64; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    const MONTH_DAYS: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    const CAL_MONTH_SELECTORS: usize = 20;
    const CAL_R4_BITS: usize = 32;
    const CAL_R100_BITS: usize = 34;
    const CAL_R400_BITS: usize = 41;
    const CAL_Q4: usize = 50;
    const CAL_Q100: usize = 51;
    const CAL_Q400: usize = 52;
    const CAL_Z4: usize = 53;
    const CAL_Z100: usize = 54;
    const CAL_Z400: usize = 55;
    const CAL_INV4: usize = 56;
    const CAL_INV100: usize = 57;
    const CAL_INV400: usize = 58;
    const CAL_PREFIX: usize = 59;
    const CAL_MONTH_DAYS: usize = 60;
    const CAL_LEAP: usize = 61;
    const CAL_DAY_MINUS_ONE: usize = 62;
    const CAL_MONTH_SLACK: usize = 63;

    let month_index = usize::try_from(
        operands
            .month
            .checked_sub(1)
            .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?,
    )
    .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    if month_index >= 12 || operands.day == 0 {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    let q4 = (operands.year - 1969) / 4;
    let r4 = (operands.year - 1969) % 4;
    let q100 = (operands.year - 1901) / 100;
    let r100 = (operands.year - 1901) % 100;
    let q400 = (operands.year - 1901) / 400;
    let r400 = (operands.year - 1901) % 400;
    let leap = r4 == 0 && r100 != 0 || r400 == 0;
    let month_days = MONTH_DAYS[month_index] + u64::from(leap && month_index == 1);
    if operands.day > month_days {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    let prefix = MONTH_PREFIX[month_index] + u64::from(leap && month_index >= 2);
    let days_before_year = (operands.year - 1970)
        .checked_mul(365)
        .and_then(|days| days.checked_add(q4))
        .and_then(|days| days.checked_sub(q100))
        .and_then(|days| days.checked_add(q400))
        .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;

    let mut row = active_zero_row_v1();
    row[BASE_A] = F(operands.year);
    row[BASE_B] = F(operands.month);
    row[BASE_C] = F(operands.day);
    row[BASE_D] = F(operands.hour);
    row[BASE_E] = F(operands.minute);
    row[BASE_F] = F(operands.second);
    row[BASE_G] = F(operands.timestamp);
    row[BASE_H] = F(days_before_year);
    row[BASE_DOCUMENT] = F(r4);
    row[BASE_ADDRESS] = F(r100);
    row[BASE_NODE] = F(r400);
    row[CAL_MONTH_SELECTORS + month_index] = F::ONE;
    write_u8_bits_v1(
        &mut row,
        CAL_R4_BITS,
        u8::try_from(r4).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
    );
    for bit in 0..7 {
        row[CAL_R100_BITS + bit] = F((r100 >> bit) & 1);
    }
    for bit in 0..9 {
        row[CAL_R400_BITS + bit] = F((r400 >> bit) & 1);
    }
    row[CAL_Q4] = F(q4);
    row[CAL_Q100] = F(q100);
    row[CAL_Q400] = F(q400);
    for (remainder, zero_column, inverse_column) in [
        (r4, CAL_Z4, CAL_INV4),
        (r100, CAL_Z100, CAL_INV100),
        (r400, CAL_Z400, CAL_INV400),
    ] {
        row[zero_column] = F(u64::from(remainder == 0));
        row[inverse_column] = if remainder == 0 {
            F::ZERO
        } else {
            F(remainder)
                .inv()
                .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?
        };
    }
    row[CAL_PREFIX] = F(prefix);
    row[CAL_MONTH_DAYS] = F(month_days);
    row[CAL_LEAP] = F(u64::from(leap));
    row[CAL_DAY_MINUS_ONE] = F(operands.day - 1);
    row[CAL_MONTH_SLACK] = F(month_days - operands.day);
    Ok(row)
}

fn output_base_row_v1(
    role: ZkX509Rfc5280OutputRoleV1,
    channel: u32,
    endpoint: ZkX509IoEndpointV1,
    offset: usize,
    value: u8,
    is_write: bool,
) -> Result<ZkX509Rfc5280StarkBaseRowV1, ZkX509Rfc5280StarkErrorV1> {
    let mut row = active_zero_row_v1();
    row[BASE_VALUE] = F(u64::from(value));
    write_u8_bits_v1(&mut row, BASE_BYTE_BITS, value);
    row[BASE_ROLE] = F(role as u64);
    row[BASE_INSTANCE] = F(u64::from(channel));
    row[BASE_OFFSET] = F(u64::try_from(offset).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?);
    row[BASE_ENDPOINT_ROLE] = F(endpoint_role_code_v1(endpoint.role)?);
    row[BASE_ENDPOINT_INSTANCE] = F(u64::from(endpoint.instance));
    row[BASE_IS_WRITE] = F(u64::from(is_write));
    Ok(row)
}

/// Compile every canonical base family from the strict-DER owner trace.
#[allow(clippy::too_many_lines)]
pub(crate) fn build_zk_x509_rfc5280_stark_base_material_v1(
    trace: &ZkX509Rfc5280TraceV1,
) -> Result<ZkX509Rfc5280StarkBaseMaterialV1, ZkX509Rfc5280StarkErrorV1> {
    validate_zk_x509_rfc5280_provenance_v1(trace)?;
    let semantic = build_zk_x509_rfc5280_semantic_witness_v1(trace)?;
    let private_shape = build_zk_x509_rfc5280_stark_private_shape_v1(trace)?;
    let schedule = compile_zk_x509_rfc5280_stark_fixed_schedule_v1(
        build_zk_x509_rfc5280_stark_shape_v1(trace)?,
    )?;
    let mut family_rows: [Vec<ZkX509Rfc5280StarkBaseRowV1>; FAMILY_COUNT_V1] =
        core::array::from_fn(|_| Vec::new());

    let (serial_byte_multiplicities, serial_node_multiplicities) =
        zk_x509_rfc5280_serial_lookup_multiplicities_v1(&semantic.serial_sources)?;
    let mut byte_lookup_cells = Vec::new();
    for entry in &serial_byte_multiplicities {
        byte_lookup_cells.extend(core::iter::repeat_n(
            entry.source,
            usize::from(entry.required_multiplicity),
        ));
    }
    for entry in zk_x509_rfc5280_semantic_source_multiplicities_v1(&semantic)? {
        byte_lookup_cells.extend(core::iter::repeat_n(
            entry.source,
            usize::from(entry.required_multiplicity),
        ));
    }
    for embedded in &trace.embedded_byte_rows {
        let value =
            u8::try_from(embedded.value.value.0).map_err(|_| ZkX509Rfc5280StarkErrorV1::Source)?;
        byte_lookup_cells.push(ZkX509Rfc5280SourceCellV1 {
            document: u8::try_from(embedded.parent_document.value.0)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            address: u16::try_from(embedded.parent_offset.value.0)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            value,
        });
        byte_lookup_cells.push(ZkX509Rfc5280SourceCellV1 {
            document: u8::try_from(
                trace
                    .documents
                    .len()
                    .checked_add(
                        usize::try_from(embedded.embedded_document.value.0)
                            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
                    )
                    .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?,
            )
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            address: u16::try_from(embedded.embedded_offset.value.0)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            value,
        });
    }
    byte_lookup_cells.sort_unstable();
    let serial_byte_multiplicity = |document: usize, address: usize, value: u8| {
        u64::try_from(
            byte_lookup_cells
                .iter()
                .filter(|entry| {
                    usize::from(entry.document) == document
                        && usize::from(entry.address) == address
                        && entry.value == value
                })
                .count(),
        )
        .expect("bounded RFC lookup multiplicity fits u64")
    };
    let serial_node_multiplicity = |document: usize, node: usize| {
        serial_node_multiplicities
            .iter()
            .find(|entry| {
                usize::from(entry.document) == document && usize::from(entry.node) == node
            })
            .map_or(0, |entry| entry.required_multiplicity)
    };

    let source_byte_family = ZkX509Rfc5280StarkFamilyV1::SourceByte as usize;
    for (document, source) in trace.documents.iter().enumerate() {
        for (address, byte) in source.bytes.iter().enumerate() {
            let ordinal = document
                .checked_mul(ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1)
                .and_then(|start| start.checked_add(address))
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
            ensure_family_slot_v1(&mut family_rows[source_byte_family], ordinal)?;
            let value =
                u8::try_from(byte.value.value.0).map_err(|_| ZkX509Rfc5280StarkErrorV1::Source)?;
            let mut row = byte_row_v1(
                u64::try_from(document).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
                u64::try_from(address).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
                value,
            );
            row[SERIAL_BYTE_TABLE_MULTIPLICITY] = F(u64::from(serial_byte_multiplicity(
                document, address, value,
            )));
            family_rows[source_byte_family][ordinal] = row;
        }
    }
    let mut embedded_byte_ordinal = MAX_TOP_LEVEL_SOURCE_BYTES_V1;
    for (embedded, source) in trace.embedded_documents.iter().enumerate() {
        let document = trace
            .documents
            .len()
            .checked_add(embedded)
            .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
        for (address, byte) in source.bytes.iter().enumerate() {
            ensure_family_slot_v1(&mut family_rows[source_byte_family], embedded_byte_ordinal)?;
            let value =
                u8::try_from(byte.value.value.0).map_err(|_| ZkX509Rfc5280StarkErrorV1::Source)?;
            let mut row = byte_row_v1(
                u64::try_from(document).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
                u64::try_from(address).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
                value,
            );
            row[SERIAL_BYTE_TABLE_MULTIPLICITY] = F(u64::from(serial_byte_multiplicity(
                document, address, value,
            )));
            family_rows[source_byte_family][embedded_byte_ordinal] = row;
            embedded_byte_ordinal = embedded_byte_ordinal
                .checked_add(1)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
        }
    }

    let source_node_family = ZkX509Rfc5280StarkFamilyV1::SourceNode as usize;
    let mut grammar_rule_multiplicities = [0_u32; ZK_X509_RFC5280_GRAMMAR_RULE_COUNT_V1];
    for provenance in &trace.semantic_provenance {
        let document = usize::from(provenance.document);
        for node in &provenance.nodes {
            let node_index = usize::from(node.node);
            let child_count = u16::try_from(
                provenance
                    .nodes
                    .iter()
                    .filter(|candidate| usize::from(candidate.parent_node) == node_index)
                    .count(),
            )
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
            let (parent_role, parent_instance, parent_child_count) = if node_index == 0 {
                (0_u16, 0_u16, child_count)
            } else {
                let parent = provenance
                    .nodes
                    .get(usize::from(node.parent_node))
                    .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?;
                let parent_child_count = u16::try_from(
                    provenance
                        .nodes
                        .iter()
                        .filter(|candidate| candidate.parent_node == parent.node)
                        .count(),
                )
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
                (parent.role as u16, parent.role_instance, parent_child_count)
            };
            let rule_index = grammar_rule_index_for_node_v1(
                *node,
                parent_role,
                parent_instance,
                parent_child_count,
            )?;
            grammar_rule_multiplicities[rule_index] = grammar_rule_multiplicities[rule_index]
                .checked_add(1)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
            let rule = ZK_X509_RFC5280_GRAMMAR_RULES_V1[rule_index];
            let ordinal = document
                .checked_mul(2_048)
                .and_then(|start| start.checked_add(node_index))
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
            ensure_family_slot_v1(&mut family_rows[source_node_family], ordinal)?;
            let mut row = active_zero_row_v1();
            row[BASE_A] = F(u64::from(
                node.content_end
                    .checked_sub(node.content_start)
                    .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?,
            ));
            let quotient = u8::try_from(parent_instance / 256)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
            let remainder = u8::try_from(parent_instance % 256)
                .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
            row[BASE_VALUE] = F(u64::from(quotient));
            write_u8_bits_v1(&mut row, BASE_BYTE_BITS, quotient);
            row[BASE_B] = F(u64::from(parent_role));
            row[BASE_C] = F(u64::from(parent_instance));
            row[BASE_D] = F(u64::from(child_count));
            row[BASE_E] = F(u64::from(rule.root_kind));
            row[SERIAL_NODE_TABLE_MULTIPLICITY] =
                F(u64::from(serial_node_multiplicity(document, node_index)));
            row[BASE_G] = F(u64::from(parent_child_count));
            row[BASE_H] = F(u64::from(rule.constant));
            row[BASE_DOCUMENT] = F(u64::from(node.document));
            row[BASE_ADDRESS] = F(u64::from(remainder));
            row[BASE_NODE] = F(u64::from(node.node));
            row[BASE_PARENT] = F(if node.parent_node == u16::MAX {
                0
            } else {
                u64::from(node.parent_node)
            });
            row[BASE_CHILD] = F(u64::from(node.child_ordinal));
            row[BASE_START] = F(u64::from(node.start));
            row[BASE_CONTENT_START] = F(u64::from(node.content_start));
            row[BASE_CONTENT_END] = F(u64::from(node.content_end));
            row[BASE_DEPTH] = F(u64::from(node.depth));
            row[BASE_TAG_CLASS] = F(u64::from(node.tag_class));
            row[BASE_CONSTRUCTED] = F(u64::from(node.constructed));
            row[BASE_TAG_NUMBER] = F(u64::from(node.tag_number));
            row[BASE_ROLE] = F(node.role as u64);
            row[BASE_INSTANCE] = F(u64::from(node.role_instance));
            row[BASE_OFFSET] = F(u64::from(rule.quotient_scale));
            row[BASE_ENDPOINT_ROLE] = F(u64::from(rule.remainder_scale));
            row[BASE_ENDPOINT_INSTANCE] = F(u64::from(rule.ordinal_scale));
            row[BASE_IS_WRITE] = F(u64::from(rule.ordinal_exact));
            row[BASE_STRICT] = F(u64::from(rule.ordinal_last));
            row[BASE_EQUAL] = F(u64::from(rule.count_exact));
            row[BASE_INVERSE] = F(u64::from(profile_role_required_v1(node.role as u16)));
            row[BASE_STATE_BEFORE] = F(u64::from(rule.ordinal_parameter));
            row[BASE_STATE_AFTER] = F(u64::from(rule.count_parameter));
            write_u8_bits_v1(&mut row, BASE_SMALL_BITS, remainder);
            write_u8_bits_v1(
                &mut row,
                BASE_SMALL_BITS + 8,
                u8::try_from(rule.ordinal_parameter)
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            );
            write_u8_bits_v1(
                &mut row,
                BASE_SMALL_BITS + 16,
                u8::try_from(rule.count_parameter)
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            );
            write_u16_bits_v1(&mut row, GRAMMAR_CHILD_ORDINAL_BITS, node.child_ordinal);
            write_u16_bits_v1(&mut row, GRAMMAR_CHILD_COUNT_BITS, child_count);
            family_rows[source_node_family][ordinal] = row;
        }
    }

    let grammar_family = ZkX509Rfc5280StarkFamilyV1::Grammar as usize;
    for multiplicity in grammar_rule_multiplicities {
        let mut row = active_zero_row_v1();
        row[BASE_A] = F(u64::from(multiplicity));
        push_family_row_v1(&mut family_rows[grammar_family], row)?;
    }
    let mut ordinal_entries = Vec::new();
    ordinal_entries
        .try_reserve(private_shape.source_nodes()?)
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
    for provenance in &trace.semantic_provenance {
        for node in provenance.nodes.iter().skip(1) {
            let parent = provenance
                .nodes
                .get(usize::from(node.parent_node))
                .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?;
            let parent_child_count = u16::try_from(
                provenance
                    .nodes
                    .iter()
                    .filter(|candidate| candidate.parent_node == parent.node)
                    .count(),
            )
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;
            ordinal_entries.push((
                node.document,
                node.parent_node,
                node.child_ordinal,
                parent_child_count,
            ));
        }
    }
    ordinal_entries.sort_unstable();
    for (index, (document, parent, child, child_count)) in
        ordinal_entries.iter().copied().enumerate()
    {
        let key = u64::from(document)
            .checked_mul(2_048)
            .and_then(|value| value.checked_add(u64::from(parent)))
            .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
        let mut row = active_zero_row_v1();
        row[BASE_A] = F(key);
        row[BASE_D] = F(u64::from(child_count));
        row[BASE_DOCUMENT] = F(u64::from(document));
        row[BASE_PARENT] = F(u64::from(parent));
        row[BASE_CHILD] = F(u64::from(child));
        if let Some((next_document, next_parent, _, _)) = ordinal_entries.get(index + 1).copied() {
            let next_key = u64::from(next_document)
                .checked_mul(2_048)
                .and_then(|value| value.checked_add(u64::from(next_parent)))
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
            let difference = next_key
                .checked_sub(key)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?;
            row[BASE_B] = F(difference);
            row[BASE_EQUAL] = F(u64::from(difference == 0));
            if difference != 0 {
                row[BASE_INVERSE] = F(difference)
                    .inv()
                    .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?;
                let gap = difference
                    .checked_sub(1)
                    .ok_or(ZkX509Rfc5280StarkErrorV1::Grammar)?;
                row[BASE_C] = F(gap);
                write_u16_bits_v1(
                    &mut row,
                    GRAMMAR_CHILD_ORDINAL_BITS,
                    u16::try_from(gap).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
                );
            }
        }
        row[BASE_ORDINAL_NEXT_ACTIVE] = F(u64::from(index + 1 < ordinal_entries.len()));
        row[BASE_ORDINAL_EQUAL_CONTINUE] = row[BASE_ORDINAL_NEXT_ACTIVE].mul(row[BASE_EQUAL]);
        push_family_row_v1(&mut family_rows[grammar_family], row)?;
    }

    let embedded_family = ZkX509Rfc5280StarkFamilyV1::EmbeddedCopy as usize;
    for source in &trace.embedded_byte_rows {
        let value =
            u8::try_from(source.value.value.0).map_err(|_| ZkX509Rfc5280StarkErrorV1::Source)?;
        let mut row = active_zero_row_v1();
        row[BASE_VALUE] = F(u64::from(value));
        write_u8_bits_v1(&mut row, BASE_BYTE_BITS, value);
        row[BASE_A] = F(u64::from(value));
        row[BASE_B] = F(u64::from(value));
        let embedded_document = F(u64::try_from(trace.documents.len())
            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?
            .checked_add(source.embedded_document.value.0)
            .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?);
        row[BASE_C] = embedded_document;
        row[BASE_DOCUMENT] = source.parent_document.value;
        row[BASE_ADDRESS] = source.parent_offset.value;
        row[BASE_CONTENT_START] = source.parent_content_start.value;
        row[BASE_OFFSET] = source.embedded_offset.value;
        row[BASE_ENDPOINT_INSTANCE] = embedded_document;
        push_family_row_v1(&mut family_rows[embedded_family], row)?;
    }

    let fixed_family = ZkX509Rfc5280StarkFamilyV1::FixedByte as usize;
    for source in &semantic.fixed_bytes {
        let mut row = byte_row_v1(
            u64::from(source.source.document),
            u64::from(source.source.address),
            source.source.value,
        );
        row[BASE_A] = F(u64::from(source.expected));
        row[BASE_B] = F(u64::from(source.length));
        row[BASE_NODE] = F(u64::from(source.source_node.node));
        row[BASE_PARENT] = F(source.source_node.role as u64);
        row[BASE_CHILD] = F(u64::from(matches!(
            source.purpose,
            1 | 2 | 3 | 6 | 7 | 8 | 9 | 10 | 11 | 12
        )));
        row[BASE_START] = F(u64::from(
            source
                .source
                .address
                .checked_sub(source.offset)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Source)?,
        ));
        row[BASE_CONTENT_START] = F(u64::from(source.source_node.start));
        row[BASE_CONTENT_END] = row[BASE_START].add(F(u64::from(source.length)));
        row[BASE_DEPTH] = F(u64::from(source.source_node.content_start));
        row[BASE_TAG_NUMBER] = F(u64::from(source.source_node.content_end));
        row[BASE_ROLE] = F(u64::from(source.purpose));
        row[BASE_INSTANCE] = F(u64::from(source.instance));
        row[BASE_OFFSET] = F(u64::from(source.offset));
        row[BASE_ENDPOINT_ROLE] = F(u64::from(source.variant));
        row[BASE_ENDPOINT_INSTANCE] = F(u64::from(matches!(source.purpose, 3..=9)));
        row[BASE_IS_WRITE] = F(u64::from(source.offset == 0));
        row[BASE_STRICT] = F(u64::from(source.offset + 1 == source.length));
        row[BASE_INVERSE] = if source.offset == 0 {
            F::ZERO
        } else {
            F(u64::from(source.offset))
                .inv()
                .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?
        };
        let remaining = source
            .length
            .checked_sub(source.offset)
            .and_then(|remaining| remaining.checked_sub(1))
            .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
        row[BASE_G] = if remaining == 0 {
            F::ZERO
        } else {
            F(u64::from(remaining))
                .inv()
                .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?
        };
        push_family_row_v1(&mut family_rows[fixed_family], row)?;
    }

    let equal_family = ZkX509Rfc5280StarkFamilyV1::EqualByte as usize;
    for source in &semantic.equal_bytes {
        let mut row = active_zero_row_v1();
        row[BASE_A] = F(u64::from(source.left.value));
        row[BASE_B] = F(u64::from(source.right.value));
        row[BASE_DOCUMENT] = F(u64::from(source.left.document));
        row[BASE_ADDRESS] = F(u64::from(source.left.address));
        row[BASE_ENDPOINT_ROLE] = F(u64::from(source.right.document));
        row[BASE_ENDPOINT_INSTANCE] = F(u64::from(source.right.address));
        row[BASE_ROLE] = F(u64::from(source.purpose));
        row[BASE_INSTANCE] = F(u64::from(source.instance));
        row[BASE_OFFSET] = F(u64::from(source.offset));
        push_family_row_v1(&mut family_rows[equal_family], row)?;
    }

    let decimal_family = ZkX509Rfc5280StarkFamilyV1::Decimal as usize;
    let calendar_family = ZkX509Rfc5280StarkFamilyV1::Calendar as usize;
    for (time_instance, node) in canonical_time_nodes_v1(trace).into_iter().enumerate() {
        let cells = source_slice_v1(trace, node, true)?;
        let operands = calendar_operands_v1(&cells, node.tag_number)?;
        let decimal = parse_time_cells_v1(&cells, node.tag_number)?.1;
        let year_digits = if node.tag_number == 23 { 2 } else { 4 };
        let group_lengths = [year_digits, 2, 2, 2, 2, 2];
        let mut cursor = 0_usize;
        for (group, length) in group_lengths.into_iter().enumerate() {
            let end = cursor
                .checked_add(length)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
            let group_cells = decimal
                .get(cursor..end)
                .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
            let expected_value = u64::from(parse_decimal_v1(
                &group_cells
                    .iter()
                    .map(|cell| cell.value)
                    .collect::<Vec<_>>(),
            )?);
            let mut state = 0_u64;
            for (offset, source) in group_cells.iter().copied().enumerate() {
                let digit = source
                    .value
                    .checked_sub(b'0')
                    .filter(|digit| *digit <= 9)
                    .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
                let mut row = byte_row_v1(
                    u64::from(source.document),
                    u64::from(source.address),
                    source.value,
                );
                row[BASE_A] = F(u64::from(digit));
                row[BASE_B] =
                    F(u64::try_from(length).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?);
                row[BASE_ROLE] =
                    F(u64::try_from(group).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?);
                row[BASE_INSTANCE] = F(u64::try_from(time_instance)
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?);
                row[BASE_OFFSET] =
                    F(u64::try_from(offset).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?);
                row[BASE_STATE_BEFORE] = F(state);
                state = state
                    .checked_mul(10)
                    .and_then(|value| value.checked_add(u64::from(digit)))
                    .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
                row[BASE_STATE_AFTER] = F(state);
                row[BASE_IS_WRITE] = F(u64::from(offset == 0));
                row[BASE_STRICT] = F(u64::from(offset + 1 == length));
                row[BASE_INVERSE] = if offset == 0 {
                    F::ZERO
                } else {
                    F(u64::try_from(offset).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?)
                        .inv()
                        .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?
                };
                let remaining = length
                    .checked_sub(offset)
                    .and_then(|remaining| remaining.checked_sub(1))
                    .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?;
                row[BASE_G] =
                    if remaining == 0 {
                        F::ZERO
                    } else {
                        F(u64::try_from(remaining)
                            .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?)
                        .inv()
                        .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?
                    };
                push_family_row_v1(&mut family_rows[decimal_family], row)?;
            }
            if state != expected_value {
                return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
            }
            cursor = end;
        }
        if cursor != decimal.len() {
            return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
        }
        let mut calendar_row = calendar_row_v1(operands)?;
        calendar_row[BASE_INSTANCE] =
            F(u64::try_from(time_instance).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?);
        for _ in 0..CALENDAR_COPY_PHASES_V1 {
            push_family_row_v1(&mut family_rows[calendar_family], calendar_row)?;
        }
    }

    let relation_family = ZkX509Rfc5280StarkFamilyV1::Relation as usize;
    let range_family = ZkX509Rfc5280StarkFamilyV1::RangeByte as usize;
    for relation in &semantic.numeric_relations {
        let mut row = active_zero_row_v1();
        row[BASE_A] = F(relation.left);
        row[BASE_B] = F(relation.right);
        row[BASE_C] = F(relation.slack);
        row[BASE_ROLE] = F(u64::from(relation.relation));
        row[BASE_INSTANCE] = F(u64::from(relation.instance));
        row[BASE_STRICT] = F(u64::from(relation.strict));
        row[BASE_INVERSE] = if relation.strict {
            F(relation.slack)
                .inv()
                .ok_or(ZkX509Rfc5280StarkErrorV1::Semantic)?
        } else {
            F::ZERO
        };
        push_family_row_v1(&mut family_rows[relation_family], row)?;

        let mut state = 0_u64;
        for (offset, value) in relation.slack.to_be_bytes().into_iter().enumerate() {
            let mut range_row = byte_row_v1(0, 0, value);
            range_row[BASE_ROLE] = F(u64::from(relation.relation));
            range_row[BASE_INSTANCE] = F(u64::from(relation.instance));
            range_row[BASE_OFFSET] =
                F(u64::try_from(offset).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?);
            range_row[BASE_STATE_BEFORE] = F(state);
            state = state
                .checked_mul(256)
                .and_then(|accumulator| accumulator.checked_add(u64::from(value)))
                .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
            range_row[BASE_STATE_AFTER] = F(state);
            push_family_row_v1(&mut family_rows[range_family], range_row)?;
        }
    }

    let bit_family = ZkX509Rfc5280StarkFamilyV1::BitFlags as usize;
    for (purpose, instance, actual, expected_value) in &semantic.bit_flags {
        let mut row = active_zero_row_v1();
        row[BASE_A] = F(*actual);
        row[BASE_B] = F(*expected_value);
        row[BASE_ROLE] = F(u64::from(*purpose));
        row[BASE_INSTANCE] = F(u64::from(*instance));
        push_family_row_v1(&mut family_rows[bit_family], row)?;
    }

    let serial_source_family = ZkX509Rfc5280StarkFamilyV1::SerialSource as usize;
    for source in &semantic.serial_sources {
        for row in build_zk_x509_rfc5280_serial_source_rows_v1(source)? {
            push_family_row_v1(&mut family_rows[serial_source_family], row)?;
        }
    }
    let serial_family = ZkX509Rfc5280StarkFamilyV1::SerialCompare as usize;
    for comparison in &semantic.serial_comparisons {
        for row in build_zk_x509_rfc5280_serial_comparison_rows_v1(comparison)? {
            for _ in 0..SERIAL_COMPARISON_PHASES_V1 {
                push_family_row_v1(&mut family_rows[serial_family], row)?;
            }
        }
    }

    let semantic_source_family = ZkX509Rfc5280StarkFamilyV1::SemanticSource as usize;
    for entry in &schedule.profile_byte_table {
        let multiplicity = semantic
            .fixed_bytes
            .iter()
            .filter(|query| {
                query.purpose == entry.purpose
                    && query.variant == entry.variant
                    && query.source_node.role as u16 == entry.source_role
                    && query.offset == entry.offset
                    && query.length == entry.length
                    && query.expected == entry.expected
            })
            .count();
        let mut row = active_zero_row_v1();
        row[BASE_A] = F::ONE;
        row[BASE_VALUE] = F(u64::from(entry.expected));
        write_u8_bits_v1(&mut row, BASE_BYTE_BITS, entry.expected);
        row[BASE_B] = F(u64::from(entry.length));
        row[BASE_D] =
            F(u64::try_from(multiplicity).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?);
        row[BASE_PARENT] = F(u64::from(entry.source_role));
        row[BASE_CHILD] = F(u64::from(entry.exact_end));
        row[BASE_ROLE] = F(u64::from(entry.purpose));
        row[BASE_OFFSET] = F(u64::from(entry.offset));
        row[BASE_ENDPOINT_ROLE] = F(u64::from(entry.variant));
        row[BASE_ENDPOINT_INSTANCE] = F(u64::from(entry.contents_only));
        push_family_row_v1(&mut family_rows[semantic_source_family], row)?;
    }
    for entry in &schedule.public_numeric_table {
        let mut row = active_zero_row_v1();
        row[BASE_A] = F(2);
        row[BASE_VALUE] = F(entry.value);
        row[BASE_ROLE] = F(u64::from(entry.relation));
        row[BASE_INSTANCE] = F(u64::from(entry.instance));
        row[BASE_ENDPOINT_ROLE] = F(u64::from(entry.side));
        row[BASE_CHILD] = F(u64::from(entry.certificate_slot_2_only));
        push_family_row_v1(&mut family_rows[semantic_source_family], row)?;
    }
    let semantic_consumer_family = ZkX509Rfc5280StarkFamilyV1::SemanticConsumer as usize;
    for source in semantic.equal_bytes.iter().map(|row| row.right) {
        let row = byte_row_v1(
            u64::from(source.document),
            u64::from(source.address),
            source.value,
        );
        push_family_row_v1(&mut family_rows[semantic_consumer_family], row)?;
    }
    for source in &trace.embedded_byte_rows {
        let document = trace
            .documents
            .len()
            .checked_add(
                usize::try_from(source.embedded_document.value.0)
                    .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            )
            .ok_or(ZkX509Rfc5280StarkErrorV1::Resource)?;
        let value =
            u8::try_from(source.value.value.0).map_err(|_| ZkX509Rfc5280StarkErrorV1::Source)?;
        let row = byte_row_v1(
            u64::try_from(document).map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?,
            source.embedded_offset.value.0,
            value,
        );
        push_family_row_v1(&mut family_rows[semantic_consumer_family], row)?;
    }

    let output_producer_family = ZkX509Rfc5280StarkFamilyV1::OutputProducer as usize;
    let output_consumer_family = ZkX509Rfc5280StarkFamilyV1::OutputConsumer as usize;
    let io = rfc5280_io_witnesses_v1(trace, 0)?;
    let roles = output_roles_v1(trace)?;
    if io.len() != roles.len() {
        return Err(ZkX509Rfc5280StarkErrorV1::Output);
    }
    for (role, witness) in roles.into_iter().zip(io) {
        if witness.consumer_values.len() != witness.declaration.consumers.len() {
            return Err(ZkX509Rfc5280StarkErrorV1::Output);
        }
        for (offset, value) in witness.producer_value.iter().copied().enumerate() {
            let row = output_base_row_v1(
                role,
                witness.declaration.channel,
                witness.declaration.producer,
                offset,
                value,
                true,
            )?;
            push_family_row_v1(&mut family_rows[output_producer_family], row)?;
        }
        for (endpoint, values) in witness
            .declaration
            .consumers
            .iter()
            .copied()
            .zip(witness.consumer_values)
        {
            for (offset, value) in values.into_iter().enumerate() {
                let row = output_base_row_v1(
                    role,
                    witness.declaration.channel,
                    endpoint,
                    offset,
                    value,
                    false,
                )?;
                push_family_row_v1(&mut family_rows[output_consumer_family], row)?;
            }
        }
    }

    let private_counts = private_shape.family_counts()?;
    for family in 0..FAMILY_COUNT_V1 {
        let active = family_rows[family]
            .iter()
            .filter(|row| row[BASE_ACTIVE] == F::ONE)
            .count();
        if family != ZkX509Rfc5280StarkFamilyV1::Padding as usize
            && active != private_counts[family]
        {
            return Err(ZkX509Rfc5280StarkErrorV1::Shape);
        }
        if family_rows[family].len() > schedule.counts[family] {
            return Err(ZkX509Rfc5280StarkErrorV1::Shape);
        }
    }

    Ok(ZkX509Rfc5280StarkBaseMaterialV1 {
        private_shape,
        schedule,
        family_rows,
    })
}

fn populate_degree_normalization_helpers_v1(
    row: &mut ZkX509Rfc5280StarkBaseRowV1,
    fixed: &ZkX509Rfc5280StarkFixedRowV1,
) {
    let family = |candidate: ZkX509Rfc5280StarkFamilyV1| fixed[candidate as usize];
    let active = row[BASE_ACTIVE];
    let source_node = family(ZkX509Rfc5280StarkFamilyV1::SourceNode);
    let root = fixed[FIX_EXPECTED + 2];
    row[BASE_GRAMMAR_ORDINAL] = source_node.mul(
        root.mul(row[BASE_DOCUMENT])
            .add(F::ONE.sub(root).mul(row[BASE_CHILD])),
    );
    row[BASE_EXPECTED_ROOT_KIND] = source_node.mul(
        fixed[FIX_EXPECTED + 3]
            .add(
                fixed[FIX_EXPECTED + 4]
                    .mul(row[BASE_CERT2_ACTIVE].add(F::ONE.sub(row[BASE_CERT2_ACTIVE]).mul(F(2)))),
            )
            .add(
                fixed[FIX_EXPECTED + 5].mul(
                    row[BASE_CERT2_ACTIVE]
                        .mul(F(2))
                        .add(F::ONE.sub(row[BASE_CERT2_ACTIVE]).mul(F(3))),
                ),
            )
            .add(fixed[FIX_EXPECTED + 6].mul(F(3))),
    );

    let profile = fixed[FIX_PROFILE_TABLE].mul(active);
    let topology = source_node.mul(active).mul(row[BASE_INVERSE]);
    row[BASE_PROFILE_TABLE_ACTIVE] = profile.add(topology);
    row[BASE_PROFILE_TABLE_MULTIPLICITY] = profile.mul(row[BASE_D]).add(topology);
    row[BASE_PROFILE_TOPOLOGY_QUERY_ACTIVE] = family(ZkX509Rfc5280StarkFamilyV1::FixedByte)
        .mul(active)
        .mul(row[BASE_IS_WRITE]);

    let serial_query = family(ZkX509Rfc5280StarkFamilyV1::SerialSource)
        .mul(active)
        .mul(row[BASE_IS_WRITE]);
    let direct_query = [
        ZkX509Rfc5280StarkFamilyV1::EmbeddedCopy,
        ZkX509Rfc5280StarkFamilyV1::FixedByte,
        ZkX509Rfc5280StarkFamilyV1::EqualByte,
        ZkX509Rfc5280StarkFamilyV1::Decimal,
        ZkX509Rfc5280StarkFamilyV1::SemanticConsumer,
    ]
    .into_iter()
    .fold(F::ZERO, |sum, candidate| sum.add(family(candidate)))
    .mul(active);
    row[BASE_SERIAL_BYTE_QUERY_ACTIVE] = serial_query.add(direct_query);
    row[BASE_SERIAL_BYTE_QUERY_VALUE] = serial_query
        .mul(row[SERIAL_SOURCE_QUERY_VALUE])
        .add(
            family(ZkX509Rfc5280StarkFamilyV1::EmbeddedCopy)
                .mul(active)
                .mul(row[BASE_A]),
        )
        .add(
            family(ZkX509Rfc5280StarkFamilyV1::FixedByte)
                .add(family(ZkX509Rfc5280StarkFamilyV1::Decimal))
                .add(family(ZkX509Rfc5280StarkFamilyV1::SemanticConsumer))
                .mul(active)
                .mul(row[BASE_VALUE]),
        )
        .add(
            family(ZkX509Rfc5280StarkFamilyV1::EqualByte)
                .mul(active)
                .mul(row[BASE_A]),
        );

    let serial_source = family(ZkX509Rfc5280StarkFamilyV1::SerialSource).mul(active);
    let decimal_source = family(ZkX509Rfc5280StarkFamilyV1::Decimal)
        .mul(active)
        .mul(row[BASE_STRICT]);
    let relation_source = family(ZkX509Rfc5280StarkFamilyV1::Relation).mul(active);
    let serial_left = fixed[FIX_SERIAL_COMPARE_PHASE_LEFT].mul(active);
    let serial_right = fixed[FIX_SERIAL_COMPARE_PHASE_RIGHT].mul(active);
    let calendar_phases = fixed[FIX_CALENDAR_PHASES..FIX_CALENDAR_PHASES + CALENDAR_COPY_PHASES_V1]
        .iter()
        .copied()
        .fold(F::ZERO, F::add);
    let calendar_consumer = calendar_phases.mul(active);
    let range_consumer = fixed[FIX_RANGE_TERMINAL].mul(active);
    row[BASE_COPY_SOURCE_ACTIVE] = serial_source.add(decimal_source).add(relation_source);
    row[BASE_COPY_CONSUMER_ACTIVE] = serial_left
        .add(serial_right)
        .add(calendar_consumer)
        .add(range_consumer);
    row[BASE_COPY_DOMAIN] = serial_source
        .add(serial_left)
        .add(serial_right)
        .mul(F(90))
        .add(decimal_source.add(calendar_consumer).mul(F(98)))
        .add(relation_source.add(range_consumer).mul(F(99)));
    row[BASE_COPY_KEY_1] = serial_source
        .mul(row[SERIAL_SOURCE_LOGICAL_ID])
        .add(decimal_source.mul(row[BASE_INSTANCE]))
        .add(relation_source.mul(row[BASE_ROLE]))
        .add(serial_left.mul(row[BASE_ROLE]))
        .add(serial_right.mul(row[BASE_INSTANCE]))
        .add(calendar_consumer.mul(row[BASE_INSTANCE]))
        .add(range_consumer.mul(row[BASE_ROLE]));
    row[BASE_COPY_KEY_2] = serial_source
        .mul(row[BASE_OFFSET])
        .add(decimal_source.mul(row[BASE_ROLE]))
        .add(relation_source.mul(row[BASE_INSTANCE]))
        .add(serial_left.add(serial_right).mul(row[BASE_OFFSET]))
        .add(
            fixed[FIX_CALENDAR_PHASES..FIX_CALENDAR_PHASES + CALENDAR_COPY_PHASES_V1]
                .iter()
                .copied()
                .enumerate()
                .fold(F::ZERO, |sum, (component, selector)| {
                    sum.add(selector.mul(active).mul(F(
                        u64::try_from(component).expect("six calendar components fit u64"),
                    )))
                }),
        )
        .add(range_consumer.mul(row[BASE_INSTANCE]));
    row[BASE_COPY_VALUE] = serial_source
        .mul(row[BASE_VALUE])
        .add(decimal_source.mul(row[BASE_STATE_AFTER]))
        .add(relation_source.mul(row[BASE_C]))
        .add(serial_left.mul(row[BASE_A]))
        .add(serial_right.mul(row[BASE_B]))
        .add(
            fixed[FIX_CALENDAR_PHASES..FIX_CALENDAR_PHASES + CALENDAR_COPY_PHASES_V1]
                .iter()
                .copied()
                .zip([
                    row[BASE_A],
                    row[BASE_B],
                    row[BASE_C],
                    row[BASE_D],
                    row[BASE_E],
                    row[BASE_F],
                ])
                .fold(F::ZERO, |sum, (selector, value)| {
                    sum.add(selector.mul(active).mul(value))
                }),
        )
        .add(range_consumer.mul(row[BASE_STATE_AFTER]));
}

impl ZkX509Rfc5280StarkBaseMaterialV1 {
    pub(crate) fn base_row(
        &self,
        row: usize,
    ) -> Result<ZkX509Rfc5280StarkBaseRowV1, ZkX509Rfc5280StarkErrorV1> {
        let (family, ordinal) = self.schedule.family_and_ordinal(row)?;
        let mut value = self.family_rows[family as usize]
            .get(ordinal)
            .copied()
            .unwrap_or([F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1]);
        value[BASE_CERT2_ACTIVE] = self.private_shape.certificate_slot_2_active;
        let fixed = self.schedule.fixed_row(row)?;
        populate_degree_normalization_helpers_v1(&mut value, &fixed);
        Ok(value)
    }

    pub(crate) fn fixed_row(
        &self,
        row: usize,
    ) -> Result<ZkX509Rfc5280StarkFixedRowV1, ZkX509Rfc5280StarkErrorV1> {
        self.schedule.fixed_row(row)
    }

    pub(crate) fn build_base_column(
        &self,
        column: usize,
    ) -> Result<Vec<F>, ZkX509Rfc5280StarkErrorV1> {
        build_zk_x509_rfc5280_stark_column_v1(
            ZK_X509_RFC5280_STARK_BASE_WIDTH_V1,
            column,
            |row, column| Ok(self.base_row(row)?[column]),
        )
    }

    pub(crate) fn build_fixed_column(
        &self,
        column: usize,
    ) -> Result<Vec<F>, ZkX509Rfc5280StarkErrorV1> {
        build_zk_x509_rfc5280_stark_column_v1(
            ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1,
            column,
            |row, column| Ok(self.fixed_row(row)?[column]),
        )
    }
}

fn row_family_gate_v1(
    material: &ZkX509Rfc5280StarkBaseMaterialV1,
    row: usize,
    family: ZkX509Rfc5280StarkFamilyV1,
) -> Result<F, ZkX509Rfc5280StarkErrorV1> {
    let (actual, _) = material.schedule.family_and_ordinal(row)?;
    Ok(F(u64::from(
        actual == family && material.base_row(row)?[BASE_ACTIVE] == F::ONE,
    )))
}

fn product_relation_factor_v1(
    relation: usize,
    row: &ZkX509Rfc5280StarkBaseRowV1,
    lane: usize,
    der_challenges: ZkX509DerStarkChallengesV1,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> Result<F, ZkX509Rfc5280StarkErrorV1> {
    match relation {
        0 => Ok(zk_x509_der_stark_input_byte_factor_v1(
            row[BASE_DOCUMENT],
            row[BASE_ADDRESS],
            row[BASE_VALUE],
            lane,
            der_challenges,
        )?),
        1 => Ok(zk_x509_der_stark_node_factor_v1(
            ZkX509DerStarkNodeEventV1 {
                document: row[BASE_DOCUMENT],
                ordinal: row[BASE_NODE],
                parent_frame: row[BASE_PARENT],
                tag_class: row[BASE_TAG_CLASS],
                tag_number: row[BASE_TAG_NUMBER],
                constructed: row[BASE_CONSTRUCTED],
                start: row[BASE_START],
                content_start: row[BASE_CONTENT_START],
                content_end: row[BASE_CONTENT_END],
                depth: row[BASE_DEPTH],
                content_len: row[BASE_A],
            },
            lane,
            der_challenges,
        )?),
        4 | 5 => Ok(output_row_factor_v1(row, lane, challenges)),
        _ => Err(ZkX509Rfc5280StarkErrorV1::Shape),
    }
}

fn product_relation_family_v1(
    relation: usize,
) -> Result<ZkX509Rfc5280StarkFamilyV1, ZkX509Rfc5280StarkErrorV1> {
    match relation {
        0 => Ok(ZkX509Rfc5280StarkFamilyV1::SourceByte),
        1 => Ok(ZkX509Rfc5280StarkFamilyV1::SourceNode),
        4 => Ok(ZkX509Rfc5280StarkFamilyV1::OutputProducer),
        5 => Ok(ZkX509Rfc5280StarkFamilyV1::OutputConsumer),
        _ => Err(ZkX509Rfc5280StarkErrorV1::Shape),
    }
}

/// Compile the six exact public terminal products from canonical base rows.
pub(crate) fn compile_zk_x509_rfc5280_stark_terminal_claims_v1(
    material: &ZkX509Rfc5280StarkBaseMaterialV1,
    der_challenges: ZkX509DerStarkChallengesV1,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> Result<ZkX509Rfc5280StarkTerminalClaimsV1, ZkX509Rfc5280StarkErrorV1> {
    der_challenges.validate()?;
    challenges.validate()?;
    let mut claims = ZkX509Rfc5280StarkTerminalClaimsV1::canonical_identity_v1();
    for row_index in 0..ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 {
        let row = material.base_row(row_index)?;
        let fixed = material.fixed_row(row_index)?;
        let (family, _) = material.schedule.family_and_ordinal(row_index)?;
        if row[BASE_ACTIVE] != F::ONE {
            continue;
        }
        for relation in [0, 1, 4, 5] {
            if family != product_relation_family_v1(relation)? {
                continue;
            }
            for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
                claims.relations[relation][lane] = claims.relations[relation][lane].mul(
                    product_relation_factor_v1(relation, &row, lane, der_challenges, challenges)?,
                );
            }
        }
        if matches!(
            family,
            ZkX509Rfc5280StarkFamilyV1::OutputProducer | ZkX509Rfc5280StarkFamilyV1::OutputConsumer
        ) {
            let consumer = family == ZkX509Rfc5280StarkFamilyV1::OutputConsumer;
            let mut selected_role = None;
            for role_index in 0..OUTPUT_ROLE_COUNT_V1 {
                let selector = fixed[output_role_fixed_selector_column_v1(role_index, consumer)];
                if selector == F::ONE && selected_role.is_none() {
                    selected_role = Some(role_index);
                } else if selector != F::ZERO {
                    return Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim);
                }
            }
            let role_index = selected_role.ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)?;
            if row[BASE_ROLE]
                != F(output_role_from_index_v1(role_index)
                    .ok_or(ZkX509Rfc5280StarkErrorV1::TerminalClaim)? as u64)
            {
                return Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim);
            }
            for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
                let factor = output_row_factor_v1(&row, lane, challenges);
                if consumer {
                    claims.output_roles[role_index].consumer_products[lane] =
                        claims.output_roles[role_index].consumer_products[lane].mul(factor);
                } else {
                    claims.output_roles[role_index].producer_products[lane] =
                        claims.output_roles[role_index].producer_products[lane].mul(factor);
                }
            }
        }
    }
    claims.validate_v1()?;
    Ok(claims)
}

/// Bind every proof-carried relation and role terminal to its independently
/// committed final auxiliary product in verifier-fixed address order.
pub(crate) fn evaluate_zk_x509_rfc5280_terminal_claim_residues_v1(
    last_aggregate: F,
    aux: &ZkX509Rfc5280StarkAuxRowV1,
    claims: ZkX509Rfc5280StarkTerminalClaimsV1,
) -> Result<[F; RFC5280_TERMINAL_CLAIM_RECORDS_V1], ZkX509Rfc5280StarkErrorV1> {
    claims.validate_v1()?;
    Ok(core::array::from_fn(|claim_index| {
        let claimed = claims
            .claim_value_v1(claim_index)
            .expect("bounded RFC terminal claim");
        if claim_index < RFC5280_AGGREGATE_TERMINAL_CLAIM_RECORDS_V1 {
            let relation_index = claim_index / ZK_X509_RFC5280_STARK_BUS_LANES_V1;
            let lane = claim_index % ZK_X509_RFC5280_STARK_BUS_LANES_V1;
            let (_, _, auxiliary_after) = RFC5280_TERMINAL_CLAIM_RELATIONS_V1[relation_index];
            last_aggregate.mul(aux[auxiliary_after + lane].sub(claimed))
        } else {
            let local = claim_index - RFC5280_AGGREGATE_TERMINAL_CLAIM_RECORDS_V1;
            let role_index =
                local / (OUTPUT_ENDPOINT_COUNT_V1 * ZK_X509_RFC5280_STARK_BUS_LANES_V1);
            let endpoint_lane =
                local % (OUTPUT_ENDPOINT_COUNT_V1 * ZK_X509_RFC5280_STARK_BUS_LANES_V1);
            let consumer = endpoint_lane / ZK_X509_RFC5280_STARK_BUS_LANES_V1
                == output_endpoint_index_v1(true);
            let lane = endpoint_lane % ZK_X509_RFC5280_STARK_BUS_LANES_V1;
            last_aggregate
                .mul(aux[output_role_aux_column_v1(role_index, consumer, lane)].sub(claimed))
        }
    }))
}

/// Decode proof bytes and replay their final-row binding without any prover
/// material or host semantic callback.
pub(crate) fn replay_zk_x509_rfc5280_terminal_claims_v1(
    last_aggregate: F,
    aux: &ZkX509Rfc5280StarkAuxRowV1,
    encoded_claims: &[u8],
) -> Result<[F; RFC5280_TERMINAL_CLAIM_RECORDS_V1], ZkX509Rfc5280StarkErrorV1> {
    evaluate_zk_x509_rfc5280_terminal_claim_residues_v1(
        last_aggregate,
        aux,
        ZkX509Rfc5280StarkTerminalClaimsV1::decode_x5r1_v1(encoded_claims)?,
    )
}

fn zero_safe_inverse_v1(gate: F, denominator: F) -> (F, F) {
    if gate == F::ZERO {
        (F::ZERO, F::ZERO)
    } else if denominator == F::ZERO {
        (F::ONE, F::ZERO)
    } else {
        (
            F::ZERO,
            denominator
                .inv()
                .expect("nonzero canonical Goldilocks value is invertible"),
        )
    }
}

fn product_aux_column_descriptor_v1(column: usize) -> Option<(usize, usize, bool)> {
    let starts = [
        (0, AUX_DER_BYTE_BEFORE, AUX_DER_BYTE_AFTER),
        (1, AUX_DER_NODE_BEFORE, AUX_DER_NODE_AFTER),
        (4, AUX_OUTPUT_PRODUCER_BEFORE, AUX_OUTPUT_PRODUCER_AFTER),
        (5, AUX_OUTPUT_CONSUMER_BEFORE, AUX_OUTPUT_CONSUMER_AFTER),
    ];
    for (relation, before, after) in starts {
        if (before..before + ZK_X509_RFC5280_STARK_BUS_LANES_V1).contains(&column) {
            return Some((relation, column - before, false));
        }
        if (after..after + ZK_X509_RFC5280_STARK_BUS_LANES_V1).contains(&column) {
            return Some((relation, column - after, true));
        }
    }
    None
}

fn output_role_aux_column_descriptor_v1(column: usize) -> Option<(usize, bool, usize)> {
    let local = column.checked_sub(AUX_OUTPUT_ROLE_PRODUCTS)?;
    if local >= OUTPUT_ROLE_COUNT_V1 * OUTPUT_ENDPOINT_COUNT_V1 * ZK_X509_RFC5280_STARK_BUS_LANES_V1
    {
        return None;
    }
    let role_index = local / (OUTPUT_ENDPOINT_COUNT_V1 * ZK_X509_RFC5280_STARK_BUS_LANES_V1);
    let endpoint_lane = local % (OUTPUT_ENDPOINT_COUNT_V1 * ZK_X509_RFC5280_STARK_BUS_LANES_V1);
    let consumer =
        endpoint_lane / ZK_X509_RFC5280_STARK_BUS_LANES_V1 == output_endpoint_index_v1(true);
    let lane = endpoint_lane % ZK_X509_RFC5280_STARK_BUS_LANES_V1;
    Some((role_index, consumer, lane))
}

fn serial_product_aux_column_descriptor_v1(column: usize) -> Option<(bool, usize, bool)> {
    for (consumer, before, after) in [
        (false, AUX_SERIAL_SOURCE_BEFORE, AUX_SERIAL_SOURCE_AFTER),
        (true, AUX_SERIAL_CONSUMER_BEFORE, AUX_SERIAL_CONSUMER_AFTER),
    ] {
        if (before..before + ZK_X509_RFC5280_STARK_BUS_LANES_V1).contains(&column) {
            return Some((consumer, column - before, false));
        }
        if (after..after + ZK_X509_RFC5280_STARK_BUS_LANES_V1).contains(&column) {
            return Some((consumer, column - after, true));
        }
    }
    None
}

fn grammar_ordinal_product_aux_column_descriptor_v1(column: usize) -> Option<(bool, usize, bool)> {
    for (table, before, after) in [
        (
            false,
            AUX_GRAMMAR_ORDINAL_SOURCE_BEFORE,
            AUX_GRAMMAR_ORDINAL_SOURCE_AFTER,
        ),
        (
            true,
            AUX_GRAMMAR_ORDINAL_TABLE_BEFORE,
            AUX_GRAMMAR_ORDINAL_TABLE_AFTER,
        ),
    ] {
        if (before..before + ZK_X509_RFC5280_STARK_BUS_LANES_V1).contains(&column) {
            return Some((table, column - before, false));
        }
        if (after..after + ZK_X509_RFC5280_STARK_BUS_LANES_V1).contains(&column) {
            return Some((table, column - after, true));
        }
    }
    None
}

#[derive(Clone, Copy)]
struct LookupAuxDescriptorV1 {
    node: bool,
    lane: usize,
    kind: u8,
}

fn lookup_aux_column_descriptor_v1(column: usize) -> Option<LookupAuxDescriptorV1> {
    for (node, starts) in [
        (
            false,
            [
                AUX_SERIAL_BYTE_LOOKUP_ACCUMULATOR,
                AUX_SERIAL_BYTE_TABLE_INVERSE,
                AUX_SERIAL_BYTE_QUERY_INVERSE,
                AUX_SERIAL_BYTE_ZERO_ACCUMULATOR,
                AUX_SERIAL_BYTE_TABLE_ZERO,
                AUX_SERIAL_BYTE_QUERY_ZERO,
            ],
        ),
        (
            true,
            [
                AUX_SERIAL_NODE_LOOKUP_ACCUMULATOR,
                AUX_SERIAL_NODE_TABLE_INVERSE,
                AUX_SERIAL_NODE_QUERY_INVERSE,
                AUX_SERIAL_NODE_ZERO_ACCUMULATOR,
                AUX_SERIAL_NODE_TABLE_ZERO,
                AUX_SERIAL_NODE_QUERY_ZERO,
            ],
        ),
    ] {
        for (kind, start) in starts.into_iter().enumerate() {
            if (start..start + ZK_X509_RFC5280_STARK_BUS_LANES_V1).contains(&column) {
                return Some(LookupAuxDescriptorV1 {
                    node,
                    lane: column - start,
                    kind: u8::try_from(kind).expect("six lookup column kinds"),
                });
            }
        }
    }
    None
}

fn profile_lookup_aux_column_descriptor_v1(column: usize) -> Option<(usize, usize)> {
    for (kind, start) in [
        AUX_PROFILE_LOOKUP_ACCUMULATOR,
        AUX_PROFILE_TABLE_INVERSE,
        AUX_PROFILE_QUERY_INVERSE,
        AUX_PROFILE_ZERO_ACCUMULATOR,
        AUX_PROFILE_TABLE_ZERO,
        AUX_PROFILE_QUERY_ZERO,
        AUX_PROFILE_TOPOLOGY_QUERY_INVERSE,
        AUX_PROFILE_TOPOLOGY_QUERY_ZERO,
    ]
    .into_iter()
    .enumerate()
    {
        if (start..start + ZK_X509_RFC5280_STARK_BUS_LANES_V1).contains(&column) {
            return Some((kind, column - start));
        }
    }
    None
}

#[derive(Clone, Copy)]
struct GrammarLookupAuxDescriptorV1 {
    parent: bool,
    lane: usize,
    kind: u8,
}

fn grammar_lookup_aux_column_descriptor_v1(column: usize) -> Option<GrammarLookupAuxDescriptorV1> {
    for (parent, starts) in [
        (
            false,
            [
                AUX_GRAMMAR_RULE_LOOKUP_ACCUMULATOR,
                AUX_GRAMMAR_RULE_TABLE_INVERSE,
                AUX_GRAMMAR_RULE_QUERY_INVERSE,
                AUX_GRAMMAR_RULE_ZERO_ACCUMULATOR,
                AUX_GRAMMAR_RULE_TABLE_ZERO,
                AUX_GRAMMAR_RULE_QUERY_ZERO,
            ],
        ),
        (
            true,
            [
                AUX_GRAMMAR_PARENT_LOOKUP_ACCUMULATOR,
                AUX_GRAMMAR_PARENT_TABLE_INVERSE,
                AUX_GRAMMAR_PARENT_QUERY_INVERSE,
                AUX_GRAMMAR_PARENT_ZERO_ACCUMULATOR,
                AUX_GRAMMAR_PARENT_TABLE_ZERO,
                AUX_GRAMMAR_PARENT_QUERY_ZERO,
            ],
        ),
    ] {
        for (kind, start) in starts.into_iter().enumerate() {
            if (start..start + ZK_X509_RFC5280_STARK_BUS_LANES_V1).contains(&column) {
                return Some(GrammarLookupAuxDescriptorV1 {
                    parent,
                    lane: column - start,
                    kind: u8::try_from(kind).expect("six grammar lookup column kinds"),
                });
            }
        }
    }
    None
}

/// Replay one challenge-dependent auxiliary column with bounded memory.
pub(crate) fn build_zk_x509_rfc5280_stark_aux_column_v1(
    material: &ZkX509Rfc5280StarkBaseMaterialV1,
    der_challenges: ZkX509DerStarkChallengesV1,
    challenges: ZkX509Rfc5280StarkChallengesV1,
    column: usize,
) -> Result<Vec<F>, ZkX509Rfc5280StarkErrorV1> {
    der_challenges.validate()?;
    challenges.validate()?;
    if column >= ZK_X509_RFC5280_STARK_AUX_WIDTH_V1 {
        return Err(ZkX509Rfc5280StarkErrorV1::Shape);
    }
    let mut values = Vec::new();
    values
        .try_reserve_exact(ZK_X509_RFC5280_STARK_TRACE_SIZE_V1)
        .map_err(|_| ZkX509Rfc5280StarkErrorV1::Resource)?;

    if let Some((relation, lane, after_column)) = product_aux_column_descriptor_v1(column) {
        let family = product_relation_family_v1(relation)?;
        let mut product = F::ONE;
        for row_index in 0..ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 {
            let row = material.base_row(row_index)?;
            let before = product;
            let gate = row_family_gate_v1(material, row_index, family)?;
            if gate == F::ONE {
                product = product.mul(product_relation_factor_v1(
                    relation,
                    &row,
                    lane,
                    der_challenges,
                    challenges,
                )?);
            }
            values.push(if after_column { product } else { before });
        }
        return Ok(values);
    }

    if let Some((role_index, consumer, lane)) = output_role_aux_column_descriptor_v1(column) {
        let mut product = F::ONE;
        for row_index in 0..ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 {
            let row = material.base_row(row_index)?;
            let fixed = material.fixed_row(row_index)?;
            values.push(product);
            let gate = row[BASE_ACTIVE]
                .mul(fixed[output_role_fixed_selector_column_v1(role_index, consumer)]);
            if gate == F::ONE {
                product = product.mul(output_row_factor_v1(&row, lane, challenges));
            } else if gate != F::ZERO {
                return Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim);
            }
        }
        return Ok(values);
    }

    if let Some((consumer, lane, after_column)) = serial_product_aux_column_descriptor_v1(column) {
        let mut product = F::ONE;
        for row_index in 0..ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 {
            let row = material.base_row(row_index)?;
            let before = product;
            let gate = if consumer {
                row[BASE_COPY_CONSUMER_ACTIVE]
            } else {
                row[BASE_COPY_SOURCE_ACTIVE]
            };
            if gate == F::ONE {
                product = product.mul(normalized_copy_factor_v1(&row, lane, challenges));
            }
            values.push(if after_column { product } else { before });
        }
        return Ok(values);
    }

    if let Some((table, lane, after_column)) =
        grammar_ordinal_product_aux_column_descriptor_v1(column)
    {
        let mut product = F::ONE;
        for row_index in 0..ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 {
            let row = material.base_row(row_index)?;
            let fixed = material.fixed_row(row_index)?;
            let before = product;
            let gate = if table {
                row[BASE_ACTIVE].mul(fixed[FIX_GRAMMAR_ORDINAL_TABLE])
            } else {
                row[BASE_ACTIVE].mul(fixed[FIX_SOURCE_NODE_NON_ROOT])
            };
            if gate == F::ONE {
                let child_count = if table { row[BASE_D] } else { row[BASE_G] };
                product = product.mul(grammar_ordinal_factor_v1(
                    row[BASE_DOCUMENT],
                    row[BASE_PARENT],
                    row[BASE_CHILD],
                    child_count,
                    lane,
                    challenges,
                ));
            }
            values.push(if after_column { product } else { before });
        }
        return Ok(values);
    }

    if let Some((kind, lane)) = profile_lookup_aux_column_descriptor_v1(column) {
        let mut accumulator = F::ZERO;
        let mut zero_accumulator = F::ZERO;
        for row_index in 0..ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 {
            let row = material.base_row(row_index)?;
            let fixed = material.fixed_row(row_index)?;
            let table_gate = row[BASE_PROFILE_TABLE_ACTIVE];
            let table_factor = fixed[FIX_PROFILE_TABLE]
                .mul(profile_byte_factor_v1(&row, lane, challenges))
                .add(
                    fixed[ZkX509Rfc5280StarkFamilyV1::SourceNode as usize]
                        .mul(profile_topology_source_factor_v1(&row, lane, challenges)),
                );
            let multiplicity = row[BASE_PROFILE_TABLE_MULTIPLICITY];
            let query_gate =
                row_family_gate_v1(material, row_index, ZkX509Rfc5280StarkFamilyV1::FixedByte)?;
            let query_factor = profile_byte_factor_v1(&row, lane, challenges);
            let topology_query_gate = row[BASE_PROFILE_TOPOLOGY_QUERY_ACTIVE];
            let topology_query_factor = profile_topology_query_factor_v1(&row, lane, challenges);
            let (table_zero, table_inverse) = zero_safe_inverse_v1(table_gate, table_factor);
            let (query_zero, query_inverse) = zero_safe_inverse_v1(query_gate, query_factor);
            let (topology_query_zero, topology_query_inverse) =
                zero_safe_inverse_v1(topology_query_gate, topology_query_factor);
            values.push(match kind {
                0 => accumulator,
                1 => table_inverse,
                2 => query_inverse,
                3 => zero_accumulator,
                4 => table_zero,
                5 => query_zero,
                6 => topology_query_inverse,
                7 => topology_query_zero,
                _ => return Err(ZkX509Rfc5280StarkErrorV1::Shape),
            });
            if row_index + 1 != ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 {
                accumulator = accumulator
                    .add(table_gate.mul(multiplicity).mul(table_inverse))
                    .sub(query_gate.mul(query_inverse))
                    .sub(topology_query_gate.mul(topology_query_inverse));
                zero_accumulator = zero_accumulator
                    .add(table_gate.mul(multiplicity).mul(table_zero))
                    .sub(query_gate.mul(query_zero))
                    .sub(topology_query_gate.mul(topology_query_zero));
            }
        }
        if matches!(kind, 0 | 3)
            && values
                .last()
                .copied()
                .is_none_or(|terminal| terminal != F::ZERO)
        {
            return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
        }
        return Ok(values);
    }

    if let Some(lookup) = grammar_lookup_aux_column_descriptor_v1(column) {
        let mut accumulator = F::ZERO;
        let mut zero_accumulator = F::ZERO;
        for row_index in 0..ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 {
            let row = material.base_row(row_index)?;
            let fixed = material.fixed_row(row_index)?;
            let source_node_gate =
                row_family_gate_v1(material, row_index, ZkX509Rfc5280StarkFamilyV1::SourceNode)?;
            let (table_gate, query_gate, table_factor, query_factor, multiplicity) =
                if lookup.parent {
                    (
                        source_node_gate,
                        row[BASE_ACTIVE].mul(fixed[FIX_SOURCE_NODE_NON_ROOT]),
                        grammar_parent_table_factor_v1(&row, lookup.lane, challenges),
                        grammar_parent_query_factor_v1(&row, lookup.lane, challenges),
                        row[BASE_D],
                    )
                } else {
                    (
                        row[BASE_ACTIVE].mul(fixed[FIX_GRAMMAR_RULE_TABLE]),
                        source_node_gate,
                        grammar_rule_table_factor_v1(&fixed, lookup.lane, challenges),
                        grammar_rule_query_factor_v1(&row, lookup.lane, challenges),
                        row[BASE_A],
                    )
                };
            let (table_zero, table_inverse) = zero_safe_inverse_v1(table_gate, table_factor);
            let (query_zero, query_inverse) = zero_safe_inverse_v1(query_gate, query_factor);
            values.push(match lookup.kind {
                0 => accumulator,
                1 => table_inverse,
                2 => query_inverse,
                3 => zero_accumulator,
                4 => table_zero,
                5 => query_zero,
                _ => return Err(ZkX509Rfc5280StarkErrorV1::Shape),
            });
            if row_index + 1 != ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 {
                accumulator = accumulator
                    .add(table_gate.mul(multiplicity).mul(table_inverse))
                    .sub(query_gate.mul(query_inverse));
                zero_accumulator = zero_accumulator
                    .add(table_gate.mul(multiplicity).mul(table_zero))
                    .sub(query_gate.mul(query_zero));
            }
        }
        if matches!(lookup.kind, 0 | 3)
            && values
                .last()
                .copied()
                .is_none_or(|terminal| terminal != F::ZERO)
        {
            return Err(ZkX509Rfc5280StarkErrorV1::Grammar);
        }
        return Ok(values);
    }

    let lookup = lookup_aux_column_descriptor_v1(column).ok_or(ZkX509Rfc5280StarkErrorV1::Shape)?;
    let mut accumulator = F::ZERO;
    let mut zero_accumulator = F::ZERO;
    for row_index in 0..ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 {
        let row = material.base_row(row_index)?;
        let table_family = if lookup.node {
            ZkX509Rfc5280StarkFamilyV1::SourceNode
        } else {
            ZkX509Rfc5280StarkFamilyV1::SourceByte
        };
        let table_gate = row_family_gate_v1(material, row_index, table_family)?;
        let serial_gate = row_family_gate_v1(
            material,
            row_index,
            ZkX509Rfc5280StarkFamilyV1::SerialSource,
        )?;
        let query_gate = if lookup.node {
            serial_gate
        } else {
            row[BASE_SERIAL_BYTE_QUERY_ACTIVE]
        };
        let table_factor = if lookup.node {
            serial_node_lookup_factor_v1(&row, lookup.lane, challenges)
        } else {
            serial_byte_lookup_factor_v1(
                row[BASE_DOCUMENT],
                row[BASE_ADDRESS],
                row[BASE_VALUE],
                lookup.lane,
                challenges,
            )
        };
        let query_factor = if lookup.node {
            serial_node_lookup_factor_v1(&row, lookup.lane, challenges)
        } else {
            serial_byte_lookup_factor_v1(
                row[BASE_DOCUMENT],
                row[BASE_ADDRESS],
                row[BASE_SERIAL_BYTE_QUERY_VALUE],
                lookup.lane,
                challenges,
            )
        };
        let (table_zero, table_inverse) = zero_safe_inverse_v1(table_gate, table_factor);
        let (query_zero, query_inverse) = zero_safe_inverse_v1(query_gate, query_factor);
        values.push(match lookup.kind {
            0 => accumulator,
            1 => table_inverse,
            2 => query_inverse,
            3 => zero_accumulator,
            4 => table_zero,
            5 => query_zero,
            _ => return Err(ZkX509Rfc5280StarkErrorV1::Shape),
        });
        if row_index + 1 != ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 {
            let multiplicity = if lookup.node {
                row[SERIAL_NODE_TABLE_MULTIPLICITY]
            } else {
                row[SERIAL_BYTE_TABLE_MULTIPLICITY]
            };
            accumulator = accumulator
                .add(table_gate.mul(multiplicity).mul(table_inverse))
                .sub(query_gate.mul(query_inverse));
            zero_accumulator = zero_accumulator
                .add(table_gate.mul(multiplicity).mul(table_zero))
                .sub(query_gate.mul(query_zero));
        }
    }
    if matches!(lookup.kind, 0 | 3)
        && values
            .last()
            .copied()
            .is_none_or(|terminal| terminal != F::ZERO)
    {
        return Err(ZkX509Rfc5280StarkErrorV1::Source);
    }
    Ok(values)
}

/// Sole production column provider for the RFC 5280 registration.
///
/// Challenge-independent base/fixed rows and challenge-dependent auxiliary
/// products are replayed from the same canonical material. Terminal claims
/// are compiled once from those base rows and exposed only through X5R1.
pub(crate) struct ZkX509Rfc5280StarkColumnProviderV1<'a> {
    material: &'a ZkX509Rfc5280StarkBaseMaterialV1,
    der_challenges: ZkX509DerStarkChallengesV1,
    challenges: ZkX509Rfc5280StarkChallengesV1,
    terminal_claims: ZkX509Rfc5280StarkTerminalClaimsV1,
}

impl core::fmt::Debug for ZkX509Rfc5280StarkColumnProviderV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("ZkX509Rfc5280StarkColumnProviderV1 { <private material redacted> }")
    }
}

impl<'a> ZkX509Rfc5280StarkColumnProviderV1<'a> {
    pub(crate) fn new_v1(
        material: &'a ZkX509Rfc5280StarkBaseMaterialV1,
        der_challenges: ZkX509DerStarkChallengesV1,
        challenges: ZkX509Rfc5280StarkChallengesV1,
    ) -> Result<Self, ZkX509Rfc5280StarkErrorV1> {
        der_challenges.validate()?;
        challenges.validate()?;
        let terminal_claims =
            compile_zk_x509_rfc5280_stark_terminal_claims_v1(material, der_challenges, challenges)?;
        Ok(Self {
            material,
            der_challenges,
            challenges,
            terminal_claims,
        })
    }

    pub(crate) fn base_row_v1(
        &self,
        row: usize,
    ) -> Result<ZkX509Rfc5280StarkBaseRowV1, ZkX509Rfc5280StarkErrorV1> {
        self.material.base_row(row)
    }

    pub(crate) fn fixed_row_v1(
        &self,
        row: usize,
    ) -> Result<ZkX509Rfc5280StarkFixedRowV1, ZkX509Rfc5280StarkErrorV1> {
        self.material.fixed_row(row)
    }

    pub(crate) fn build_base_column_v1(
        &self,
        column: usize,
    ) -> Result<Vec<F>, ZkX509Rfc5280StarkErrorV1> {
        self.material.build_base_column(column)
    }

    pub(crate) fn build_fixed_column_v1(
        &self,
        column: usize,
    ) -> Result<Vec<F>, ZkX509Rfc5280StarkErrorV1> {
        self.material.build_fixed_column(column)
    }

    pub(crate) fn build_aux_column_v1(
        &self,
        column: usize,
    ) -> Result<Vec<F>, ZkX509Rfc5280StarkErrorV1> {
        build_zk_x509_rfc5280_stark_aux_column_v1(
            self.material,
            self.der_challenges,
            self.challenges,
            column,
        )
    }

    pub(crate) const fn terminal_claims_v1(&self) -> ZkX509Rfc5280StarkTerminalClaimsV1 {
        self.terminal_claims
    }

    pub(crate) fn encoded_terminal_claims_v1(
        &self,
    ) -> Result<[u8; ZK_X509_RFC5280_TERMINAL_CLAIM_BYTES_V1], ZkX509Rfc5280StarkErrorV1> {
        self.terminal_claims.encode_x5r1_v1()
    }
}

/// Evaluate the complete opened RFC row without a host semantic callback.
///
/// Fixed family selectors are verifier-preprocessed. Every family contributes
/// the same residue inventory on every row, so neither witness values nor
/// roles can alter the composition shape.
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_zk_x509_rfc5280_stark_residues_v1(
    current: &ZkX509Rfc5280StarkBaseRowV1,
    next: &ZkX509Rfc5280StarkBaseRowV1,
    current_aux: &ZkX509Rfc5280StarkAuxRowV1,
    next_aux: &ZkX509Rfc5280StarkAuxRowV1,
    fixed: &ZkX509Rfc5280StarkFixedRowV1,
    der_challenges: ZkX509DerStarkChallengesV1,
    challenges: ZkX509Rfc5280StarkChallengesV1,
    terminal_claims: ZkX509Rfc5280StarkTerminalClaimsV1,
) -> Result<Vec<F>, ZkX509Rfc5280StarkErrorV1> {
    der_challenges.validate()?;
    challenges.validate()?;
    if current
        .iter()
        .chain(next)
        .chain(current_aux)
        .chain(next_aux)
        .chain(fixed)
        .chain(terminal_claims.relations.iter().flatten())
        .any(|value| value.0 >= GOLDILOCKS_MODULUS_V1 || F::canonical(value.0).is_none())
    {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    let mut residues = Vec::with_capacity(ZK_X509_RFC5280_STARK_CONSTRAINT_COUNT_V1);
    let mut residue_section_start = 0;
    for bit in &current[BASE_BYTE_BITS..BASE_BYTE_BITS + 8] {
        push_boolean_v1(&mut residues, F::ONE, *bit);
    }
    residues
        .push(current[BASE_VALUE].sub(pack_bits_v1(&current[BASE_BYTE_BITS..BASE_BYTE_BITS + 8])));
    for value in [
        current[BASE_CONSTRUCTED],
        current[BASE_IS_WRITE],
        current[BASE_STRICT],
        current[BASE_EQUAL],
    ] {
        push_boolean_v1(&mut residues, F::ONE, value);
    }
    assert_residue_section_v1(
        residues.len(),
        &mut residue_section_start,
        RFC5280_RESIDUE_SECTIONS_V1[0],
    );
    let mut normalized_helpers = *current;
    populate_degree_normalization_helpers_v1(&mut normalized_helpers, fixed);
    for column in [
        BASE_GRAMMAR_ORDINAL,
        BASE_EXPECTED_ROOT_KIND,
        BASE_PROFILE_TABLE_ACTIVE,
        BASE_PROFILE_TABLE_MULTIPLICITY,
        BASE_PROFILE_TOPOLOGY_QUERY_ACTIVE,
        BASE_SERIAL_BYTE_QUERY_ACTIVE,
        BASE_SERIAL_BYTE_QUERY_VALUE,
        BASE_COPY_SOURCE_ACTIVE,
        BASE_COPY_CONSUMER_ACTIVE,
        BASE_COPY_DOMAIN,
        BASE_COPY_KEY_1,
        BASE_COPY_KEY_2,
        BASE_COPY_VALUE,
    ] {
        residues.push(current[column].sub(normalized_helpers[column]));
    }
    assert_residue_section_v1(
        residues.len(),
        &mut residue_section_start,
        RFC5280_RESIDUE_SECTIONS_V1[1],
    );

    let source_byte = active_family_gate_v1(current, fixed, ZkX509Rfc5280StarkFamilyV1::SourceByte);
    residues.push(
        source_byte
            .mul(fixed[FIX_DOCUMENT_FIXED])
            .mul(current[BASE_DOCUMENT].sub(fixed[FIX_EXPECTED])),
    );
    residues.push(
        source_byte
            .mul(fixed[FIX_ADDRESS_FIXED])
            .mul(current[BASE_ADDRESS].sub(fixed[FIX_EXPECTED + 1])),
    );

    let source_node = active_family_gate_v1(current, fixed, ZkX509Rfc5280StarkFamilyV1::SourceNode);
    residues.push(
        source_node
            .mul(fixed[FIX_DOCUMENT_FIXED])
            .mul(current[BASE_DOCUMENT].sub(fixed[FIX_EXPECTED])),
    );
    residues.push(
        source_node
            .mul(fixed[FIX_ADDRESS_FIXED])
            .mul(current[BASE_NODE].sub(fixed[FIX_EXPECTED + 1])),
    );

    let embedded = active_family_gate_v1(current, fixed, ZkX509Rfc5280StarkFamilyV1::EmbeddedCopy);
    residues.push(embedded.mul(current[BASE_A].sub(current[BASE_B])));
    residues.push(
        embedded
            .mul(current[BASE_ADDRESS].sub(current[BASE_CONTENT_START].add(current[BASE_OFFSET]))),
    );
    residues.push(embedded.mul(current[BASE_C].sub(current[BASE_ENDPOINT_INSTANCE])));
    assert_residue_section_v1(
        residues.len(),
        &mut residue_section_start,
        RFC5280_RESIDUE_SECTIONS_V1[2],
    );

    let grammar_rule_table = current[BASE_ACTIVE].mul(fixed[FIX_GRAMMAR_RULE_TABLE]);
    let grammar_ordinal_table = current[BASE_ACTIVE].mul(fixed[FIX_GRAMMAR_ORDINAL_TABLE]);
    let root_node = fixed[FIX_EXPECTED + 2];
    let non_root_node = F::ONE.sub(root_node);
    for bit in &current[BASE_SMALL_BITS..BASE_SMALL_BITS + 24] {
        push_boolean_v1(&mut residues, source_node, *bit);
    }
    for bit in &current[GRAMMAR_CHILD_ORDINAL_BITS..GRAMMAR_CHILD_ORDINAL_BITS + 16] {
        push_boolean_v1(&mut residues, source_node, *bit);
    }
    for bit in &current[GRAMMAR_CHILD_COUNT_BITS..GRAMMAR_CHILD_COUNT_BITS + 16] {
        push_boolean_v1(&mut residues, source_node, *bit);
    }
    residues.push(source_node.mul(
        current[BASE_ADDRESS].sub(pack_bits_v1(&current[BASE_SMALL_BITS..BASE_SMALL_BITS + 8])),
    ));
    residues.push(source_node.mul(current[BASE_STATE_BEFORE].sub(pack_bits_v1(
        &current[BASE_SMALL_BITS + 8..BASE_SMALL_BITS + 16],
    ))));
    residues.push(source_node.mul(current[BASE_STATE_AFTER].sub(pack_bits_v1(
        &current[BASE_SMALL_BITS + 16..BASE_SMALL_BITS + 24],
    ))));
    residues.push(source_node.mul(current[BASE_CHILD].sub(pack_bits_v1(
        &current[GRAMMAR_CHILD_ORDINAL_BITS..GRAMMAR_CHILD_ORDINAL_BITS + 16],
    ))));
    residues.push(source_node.mul(current[BASE_D].sub(pack_bits_v1(
        &current[GRAMMAR_CHILD_COUNT_BITS..GRAMMAR_CHILD_COUNT_BITS + 16],
    ))));
    residues.push(
        source_node.mul(
            current[BASE_C]
                .sub(current[BASE_VALUE].mul(F(256)))
                .sub(current[BASE_ADDRESS]),
        ),
    );
    let grammar_ordinal = current[BASE_GRAMMAR_ORDINAL];
    residues.push(
        source_node.mul(
            current[BASE_INSTANCE].sub(
                current[BASE_VALUE]
                    .mul(current[BASE_OFFSET])
                    .add(current[BASE_ADDRESS].mul(current[BASE_ENDPOINT_ROLE]))
                    .add(grammar_ordinal.mul(current[BASE_ENDPOINT_INSTANCE]))
                    .add(current[BASE_H]),
            ),
        ),
    );
    residues.push(
        source_node
            .mul(current[BASE_IS_WRITE])
            .mul(grammar_ordinal.sub(current[BASE_STATE_BEFORE])),
    );
    residues.push(
        source_node
            .mul(current[BASE_STRICT])
            .mul(grammar_ordinal.add(F::ONE).sub(current[BASE_G])),
    );
    residues.push(
        source_node
            .mul(current[BASE_EQUAL])
            .mul(current[BASE_G].sub(current[BASE_STATE_AFTER])),
    );
    residues.push(
        source_node
            .mul(current[BASE_IS_WRITE])
            .mul(current[BASE_STRICT]),
    );
    residues.push(source_node.mul(root_node).mul(current[BASE_PARENT]));
    residues.push(source_node.mul(root_node).mul(current[BASE_CHILD]));
    residues.push(source_node.mul(root_node).mul(current[BASE_B]));
    residues.push(source_node.mul(root_node).mul(current[BASE_C]));
    residues.push(
        source_node
            .mul(root_node)
            .mul(current[BASE_G].sub(current[BASE_D])),
    );
    residues.push(source_node.mul(non_root_node).mul(current[BASE_E]));
    residues.push(
        source_node
            .mul(root_node)
            .mul(current[BASE_E].sub(current[BASE_EXPECTED_ROOT_KIND])),
    );
    assert_residue_section_v1(
        residues.len(),
        &mut residue_section_start,
        RFC5280_RESIDUE_SECTIONS_V1[3],
    );

    let ordinal_next_active = current[BASE_ORDINAL_NEXT_ACTIVE];
    residues.push(
        ordinal_next_active.sub(
            fixed[FIX_GRAMMAR_ORDINAL_TABLE]
                .mul(fixed[FIX_ACTIVATION_CONTINUE])
                .mul(next[BASE_ACTIVE]),
        ),
    );
    residues.push(
        current[BASE_ORDINAL_EQUAL_CONTINUE].sub(ordinal_next_active.mul(current[BASE_EQUAL])),
    );
    for bit in &current[GRAMMAR_CHILD_ORDINAL_BITS..GRAMMAR_CHILD_ORDINAL_BITS + 16] {
        push_boolean_v1(&mut residues, grammar_ordinal_table, *bit);
    }
    residues.push(
        grammar_ordinal_table.mul(
            current[BASE_A]
                .sub(current[BASE_DOCUMENT].mul(F(2_048)))
                .sub(current[BASE_PARENT]),
        ),
    );
    residues.push(grammar_ordinal_table.mul(current[BASE_C].sub(pack_bits_v1(
        &current[GRAMMAR_CHILD_ORDINAL_BITS..GRAMMAR_CHILD_ORDINAL_BITS + 16],
    ))));
    residues.push(
        grammar_ordinal_table
            .mul(fixed[FIX_GRAMMAR_ORDINAL_FIRST])
            .mul(current[BASE_CHILD]),
    );
    residues.push(
        grammar_ordinal_table
            .mul(ordinal_next_active)
            .mul(current[BASE_B].sub(next[BASE_A].sub(current[BASE_A]))),
    );
    push_reused_gated_zero_safe_inverse_v1(
        &mut residues,
        // `ordinal_next_active` is itself constrained above to the exact
        // grammar-table/continuation/next-active conjunction. Reusing that
        // committed normalization helper here avoids multiplying the
        // zero-safe inverse identities by the same selectors a second time,
        // which would raise all four identities from degree four to five.
        ordinal_next_active,
        current[BASE_B],
        current[BASE_EQUAL],
        current[BASE_INVERSE],
    );
    residues.push(
        grammar_ordinal_table
            .mul(current[BASE_ORDINAL_EQUAL_CONTINUE])
            .mul(next[BASE_CHILD].sub(current[BASE_CHILD].add(F::ONE))),
    );
    residues.push(
        grammar_ordinal_table
            .mul(current[BASE_ORDINAL_EQUAL_CONTINUE])
            .mul(next[BASE_D].sub(current[BASE_D])),
    );
    residues.push(
        grammar_ordinal_table
            .mul(ordinal_next_active.sub(current[BASE_ORDINAL_EQUAL_CONTINUE]))
            .mul(next[BASE_CHILD]),
    );
    residues.push(
        grammar_ordinal_table
            .mul(ordinal_next_active.sub(current[BASE_ORDINAL_EQUAL_CONTINUE]))
            .mul(current[BASE_CHILD].add(F::ONE).sub(current[BASE_D])),
    );
    residues.push(
        grammar_ordinal_table
            .mul(ordinal_next_active.sub(current[BASE_ORDINAL_EQUAL_CONTINUE]))
            .mul(current[BASE_B].sub(F::ONE).sub(current[BASE_C])),
    );
    residues.push(
        grammar_ordinal_table
            .mul(F::ONE.sub(ordinal_next_active))
            .mul(current[BASE_CHILD].add(F::ONE).sub(current[BASE_D])),
    );
    residues.push(
        grammar_ordinal_table
            .mul(F::ONE.sub(ordinal_next_active))
            .mul(current[BASE_B]),
    );
    residues.push(
        grammar_ordinal_table
            .mul(F::ONE.sub(ordinal_next_active))
            .mul(current[BASE_C]),
    );
    assert_residue_section_v1(
        residues.len(),
        &mut residue_section_start,
        RFC5280_RESIDUE_SECTIONS_V1[4],
    );

    let fixed_byte = active_family_gate_v1(current, fixed, ZkX509Rfc5280StarkFamilyV1::FixedByte);
    residues.push(fixed_byte.mul(current[BASE_VALUE].sub(current[BASE_A])));
    push_boolean_v1(&mut residues, fixed_byte, current[BASE_IS_WRITE]);
    push_boolean_v1(&mut residues, fixed_byte, current[BASE_STRICT]);
    push_boolean_v1(&mut residues, fixed_byte, current[BASE_ENDPOINT_INSTANCE]);
    push_boolean_v1(&mut residues, fixed_byte, current[BASE_CHILD]);
    push_reused_gated_zero_safe_inverse_v1(
        &mut residues,
        fixed_byte,
        current[BASE_OFFSET],
        current[BASE_IS_WRITE],
        current[BASE_INVERSE],
    );
    let fixed_byte_remaining = current[BASE_B].sub(current[BASE_OFFSET]).sub(F::ONE);
    push_reused_gated_zero_safe_inverse_v1(
        &mut residues,
        fixed_byte,
        fixed_byte_remaining,
        current[BASE_STRICT],
        current[BASE_G],
    );
    let fixed_byte_selected_start = current[BASE_CONTENT_START].add(
        current[BASE_ENDPOINT_INSTANCE].mul(current[BASE_DEPTH].sub(current[BASE_CONTENT_START])),
    );
    residues.push(fixed_byte.mul(current[BASE_START].sub(fixed_byte_selected_start)));
    residues.push(
        fixed_byte.mul(
            current[BASE_CONTENT_END]
                .sub(current[BASE_START])
                .sub(current[BASE_B]),
        ),
    );
    residues.push(
        fixed_byte
            .mul(current[BASE_CHILD])
            .mul(current[BASE_CONTENT_END].sub(current[BASE_TAG_NUMBER])),
    );
    let fixed_byte_continue = fixed_byte.mul(F::ONE.sub(current[BASE_STRICT]));
    residues.push(fixed_byte_continue.mul(next[BASE_ACTIVE].sub(F::ONE)));
    for (current_value, next_value) in [
        (current[BASE_ROLE], next[BASE_ROLE]),
        (current[BASE_INSTANCE], next[BASE_INSTANCE]),
        (current[BASE_ENDPOINT_ROLE], next[BASE_ENDPOINT_ROLE]),
        (current[BASE_PARENT], next[BASE_PARENT]),
        (current[BASE_DOCUMENT], next[BASE_DOCUMENT]),
        (current[BASE_NODE], next[BASE_NODE]),
        (current[BASE_B], next[BASE_B]),
        (current[BASE_START], next[BASE_START]),
        (current[BASE_CONTENT_START], next[BASE_CONTENT_START]),
        (current[BASE_DEPTH], next[BASE_DEPTH]),
        (current[BASE_TAG_NUMBER], next[BASE_TAG_NUMBER]),
        (
            current[BASE_ENDPOINT_INSTANCE],
            next[BASE_ENDPOINT_INSTANCE],
        ),
        (current[BASE_CHILD], next[BASE_CHILD]),
    ] {
        residues.push(fixed_byte_continue.mul(next_value.sub(current_value)));
    }
    residues.push(fixed_byte_continue.mul(next[BASE_OFFSET].sub(current[BASE_OFFSET]).sub(F::ONE)));
    residues
        .push(fixed_byte_continue.mul(next[BASE_ADDRESS].sub(current[BASE_ADDRESS]).sub(F::ONE)));
    let equal_byte = active_family_gate_v1(current, fixed, ZkX509Rfc5280StarkFamilyV1::EqualByte);
    residues.push(equal_byte.mul(current[BASE_A].sub(current[BASE_B])));

    let decimal = active_family_gate_v1(current, fixed, ZkX509Rfc5280StarkFamilyV1::Decimal);
    for bit in &current[BASE_SMALL_BITS..BASE_SMALL_BITS + 4] {
        push_boolean_v1(&mut residues, decimal, *bit);
    }
    residues
        .push(decimal.mul(
            current[BASE_A].sub(pack_bits_v1(&current[BASE_SMALL_BITS..BASE_SMALL_BITS + 4])),
        ));
    residues.push(decimal.mul(current[BASE_VALUE].sub(F(48).add(current[BASE_A]))));
    residues.push(decimal.mul(
        current[BASE_STATE_AFTER].sub(current[BASE_STATE_BEFORE].mul(F(10)).add(current[BASE_A])),
    ));
    push_reused_gated_zero_safe_inverse_v1(
        &mut residues,
        decimal,
        current[BASE_OFFSET],
        current[BASE_IS_WRITE],
        current[BASE_INVERSE],
    );
    push_reused_gated_zero_safe_inverse_v1(
        &mut residues,
        decimal,
        current[BASE_B].sub(current[BASE_OFFSET]).sub(F::ONE),
        current[BASE_STRICT],
        current[BASE_G],
    );
    residues.push(
        decimal
            .mul(current[BASE_IS_WRITE])
            .mul(current[BASE_STATE_BEFORE]),
    );
    let decimal_continue = decimal.mul(F::ONE.sub(current[BASE_STRICT]));
    residues.push(decimal_continue.mul(next[BASE_ACTIVE].sub(F::ONE)));
    residues.push(decimal_continue.mul(next[BASE_ROLE].sub(current[BASE_ROLE])));
    residues.push(decimal_continue.mul(next[BASE_INSTANCE].sub(current[BASE_INSTANCE])));
    residues.push(decimal_continue.mul(next[BASE_B].sub(current[BASE_B])));
    residues.push(decimal_continue.mul(next[BASE_OFFSET].sub(current[BASE_OFFSET]).sub(F::ONE)));
    residues.push(decimal_continue.mul(next[BASE_STATE_BEFORE].sub(current[BASE_STATE_AFTER])));
    assert_residue_section_v1(
        residues.len(),
        &mut residue_section_start,
        RFC5280_RESIDUE_SECTIONS_V1[5],
    );

    let calendar =
        current[BASE_ACTIVE].mul(fixed[FIX_CALENDAR_PHASES + CALENDAR_COPY_PHASES_V1 - 1]);
    let calendar_phase_transition = fixed
        [FIX_CALENDAR_PHASES..FIX_CALENDAR_PHASES + CALENDAR_COPY_PHASES_V1 - 1]
        .iter()
        .copied()
        .fold(F::ZERO, F::add);
    for column in [
        BASE_ACTIVE,
        BASE_A,
        BASE_B,
        BASE_C,
        BASE_D,
        BASE_E,
        BASE_F,
        BASE_INSTANCE,
    ] {
        residues.push(calendar_phase_transition.mul(next[column].sub(current[column])));
    }
    const CAL_MONTH_SELECTORS: usize = 20;
    const CAL_R4_BITS: usize = 32;
    const CAL_R100_BITS: usize = 34;
    const CAL_R400_BITS: usize = 41;
    const CAL_Q4: usize = 50;
    const CAL_Q100: usize = 51;
    const CAL_Q400: usize = 52;
    const CAL_Z4: usize = 53;
    const CAL_Z100: usize = 54;
    const CAL_Z400: usize = 55;
    const CAL_INV4: usize = 56;
    const CAL_INV100: usize = 57;
    const CAL_INV400: usize = 58;
    const CAL_PREFIX: usize = 59;
    const CAL_MONTH_DAYS: usize = 60;
    const CAL_LEAP: usize = 61;
    const CAL_DAY_MINUS_ONE: usize = 62;
    const CAL_MONTH_SLACK: usize = 63;
    for bit in &current[CAL_R4_BITS..CAL_R4_BITS + 2] {
        push_boolean_v1(&mut residues, calendar, *bit);
    }
    residues.push(
        calendar
            .mul(current[BASE_DOCUMENT].sub(pack_bits_v1(&current[CAL_R4_BITS..CAL_R4_BITS + 2]))),
    );
    for bit in &current[CAL_R100_BITS..CAL_R100_BITS + 7] {
        push_boolean_v1(&mut residues, calendar, *bit);
    }
    residues.push(
        calendar.mul(
            current[BASE_ADDRESS].sub(pack_bits_v1(&current[CAL_R100_BITS..CAL_R100_BITS + 7])),
        ),
    );
    for bit in &current[CAL_R400_BITS..CAL_R400_BITS + 9] {
        push_boolean_v1(&mut residues, calendar, *bit);
    }
    residues.push(
        calendar
            .mul(current[BASE_NODE].sub(pack_bits_v1(&current[CAL_R400_BITS..CAL_R400_BITS + 9]))),
    );
    let month_selectors = &current[CAL_MONTH_SELECTORS..CAL_MONTH_SELECTORS + 12];
    for selector in month_selectors {
        push_boolean_v1(&mut residues, calendar, *selector);
    }
    residues.push(
        calendar.mul(
            month_selectors
                .iter()
                .copied()
                .fold(F::ZERO, F::add)
                .sub(F::ONE),
        ),
    );
    residues.push(calendar.mul(
        current[BASE_B].sub(month_selectors.iter().copied().enumerate().fold(
            F::ZERO,
            |sum, (month, selector)| {
                sum.add(selector.mul(F(
                    u64::try_from(month + 1).expect("calendar month fits u64"),
                )))
            },
        )),
    ));
    residues.push(
        calendar.mul(
            current[BASE_A]
                .sub(F(1969))
                .sub(current[CAL_Q4].mul(F(4)))
                .sub(current[BASE_DOCUMENT]),
        ),
    );
    residues.push(
        calendar.mul(
            current[BASE_A]
                .sub(F(1901))
                .sub(current[CAL_Q100].mul(F(100)))
                .sub(current[BASE_ADDRESS]),
        ),
    );
    residues.push(
        calendar.mul(
            current[BASE_A]
                .sub(F(1901))
                .sub(current[CAL_Q400].mul(F(400)))
                .sub(current[BASE_NODE]),
        ),
    );
    for (remainder, zero, inverse) in [
        (current[BASE_DOCUMENT], current[CAL_Z4], current[CAL_INV4]),
        (
            current[BASE_ADDRESS],
            current[CAL_Z100],
            current[CAL_INV100],
        ),
        (current[BASE_NODE], current[CAL_Z400], current[CAL_INV400]),
    ] {
        push_boolean_v1(&mut residues, calendar, zero);
        residues.push(calendar.mul(remainder.mul(inverse).sub(F::ONE.sub(zero))));
        residues.push(calendar.mul(zero).mul(remainder));
    }
    push_boolean_v1(&mut residues, calendar, current[CAL_LEAP]);
    residues.push(
        calendar.mul(
            current[CAL_LEAP].sub(
                current[CAL_Z4]
                    .mul(F::ONE.sub(current[CAL_Z100]))
                    .add(current[CAL_Z400]),
            ),
        ),
    );
    residues.push(
        calendar.mul(
            current[BASE_H].sub(
                current[BASE_A]
                    .sub(F(1970))
                    .mul(F(365))
                    .add(current[CAL_Q4])
                    .sub(current[CAL_Q100])
                    .add(current[CAL_Q400]),
            ),
        ),
    );
    const MONTH_PREFIX: [u64; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    const MONTH_DAYS: [u64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let selected_prefix =
        month_selectors
            .iter()
            .copied()
            .enumerate()
            .fold(F::ZERO, |sum, (month, selector)| {
                let leap = if month >= 2 {
                    current[CAL_LEAP]
                } else {
                    F::ZERO
                };
                sum.add(selector.mul(F(MONTH_PREFIX[month]).add(leap)))
            });
    residues.push(calendar.mul(current[CAL_PREFIX].sub(selected_prefix)));
    let selected_days =
        month_selectors
            .iter()
            .copied()
            .enumerate()
            .fold(F::ZERO, |sum, (month, selector)| {
                let leap = if month == 1 {
                    current[CAL_LEAP]
                } else {
                    F::ZERO
                };
                sum.add(selector.mul(F(MONTH_DAYS[month]).add(leap)))
            });
    residues.push(calendar.mul(current[CAL_MONTH_DAYS].sub(selected_days)));
    residues.push(calendar.mul(current[BASE_C].sub(current[CAL_DAY_MINUS_ONE].add(F::ONE))));
    residues.push(
        calendar.mul(current[CAL_MONTH_DAYS].sub(current[BASE_C].add(current[CAL_MONTH_SLACK]))),
    );
    let timestamp = current[BASE_H]
        .add(current[CAL_PREFIX])
        .add(current[CAL_DAY_MINUS_ONE])
        .mul(F(24))
        .add(current[BASE_D])
        .mul(F(60))
        .add(current[BASE_E])
        .mul(F(60))
        .add(current[BASE_F]);
    residues.push(calendar.mul(current[BASE_G].sub(timestamp)));
    assert_residue_section_v1(
        residues.len(),
        &mut residue_section_start,
        RFC5280_RESIDUE_SECTIONS_V1[6],
    );

    let relation = active_family_gate_v1(current, fixed, ZkX509Rfc5280StarkFamilyV1::Relation);
    residues.push(relation.mul(current[BASE_A].sub(current[BASE_B].add(current[BASE_C]))));
    residues.push(
        relation.mul(
            current[BASE_C]
                .mul(current[BASE_INVERSE])
                .sub(current[BASE_STRICT]),
        ),
    );
    residues.push(
        relation
            .mul(F::ONE.sub(current[BASE_STRICT]))
            .mul(current[BASE_INVERSE]),
    );

    let bit_flags = active_family_gate_v1(current, fixed, ZkX509Rfc5280StarkFamilyV1::BitFlags);
    residues.push(bit_flags.mul(current[BASE_A].sub(current[BASE_B])));
    assert_residue_section_v1(
        residues.len(),
        &mut residue_section_start,
        RFC5280_RESIDUE_SECTIONS_V1[7],
    );

    let serial_source =
        active_family_gate_v1(current, fixed, ZkX509Rfc5280StarkFamilyV1::SerialSource);
    let serial_source_first = current[BASE_ACTIVE].mul(fixed[FIX_SERIAL_SOURCE_FIRST]);
    let serial_source_interior = current[BASE_ACTIVE].mul(fixed[FIX_SERIAL_SOURCE_INTERIOR]);
    let serial_source_not_first = current[BASE_ACTIVE].mul(fixed[FIX_SERIAL_SOURCE_NOT_FIRST]);
    let serial_source_first_payload =
        current[BASE_ACTIVE].mul(fixed[FIX_SERIAL_SOURCE_FIRST_PAYLOAD]);
    for (actual, expected) in [
        (current[SERIAL_SOURCE_LOGICAL_ID], fixed[FIX_EXPECTED]),
        (current[BASE_OFFSET], fixed[FIX_EXPECTED + 1]),
        (current[BASE_ROLE], fixed[FIX_EXPECTED + 2]),
        (current[BASE_INSTANCE], fixed[FIX_EXPECTED + 3]),
    ] {
        residues.push(serial_source.mul(actual.sub(expected)));
    }
    residues.push(serial_source.mul(current[BASE_TAG_CLASS]));
    residues.push(serial_source.mul(current[BASE_CONSTRUCTED]));
    residues.push(serial_source.mul(current[BASE_TAG_NUMBER].sub(F(2))));
    residues.push(
        serial_source.mul(
            current[BASE_CONTENT_END]
                .sub(current[BASE_CONTENT_START])
                .sub(current[BASE_A]),
        ),
    );
    residues.push(
        serial_source.mul(
            current[BASE_A]
                .sub(current[SERIAL_SOURCE_LENGTH])
                .sub(current[BASE_STRICT]),
        ),
    );
    residues.push(serial_source_first.mul(current[BASE_VALUE].sub(current[SERIAL_SOURCE_LENGTH])));
    residues.push(
        serial_source
            .mul(F::ONE.sub(fixed[FIX_LOCAL_LAST]))
            .mul(next[SERIAL_SOURCE_LENGTH].sub(current[SERIAL_SOURCE_LENGTH])),
    );
    residues.push(
        serial_source.mul(
            current[SERIAL_SOURCE_COUNT_AFTER]
                .sub(current[SERIAL_SOURCE_COUNT_BEFORE])
                .sub(current[BASE_EQUAL]),
        ),
    );
    residues.push(serial_source_first.mul(current[SERIAL_SOURCE_COUNT_BEFORE]));
    residues.push(serial_source_first.mul(current[BASE_EQUAL]));
    residues.push(
        serial_source
            .mul(F::ONE.sub(fixed[FIX_LOCAL_LAST]))
            .mul(next[SERIAL_SOURCE_COUNT_BEFORE].sub(current[SERIAL_SOURCE_COUNT_AFTER])),
    );
    residues.push(serial_source_first.mul(next[BASE_EQUAL].sub(F::ONE)));
    residues.push(
        serial_source_interior
            .mul(next[BASE_EQUAL])
            .mul(F::ONE.sub(current[BASE_EQUAL])),
    );
    residues.push(
        serial_source_not_first
            .mul(F::ONE.sub(current[BASE_EQUAL]))
            .mul(current[BASE_VALUE]),
    );
    residues.push(
        serial_source
            .mul(fixed[FIX_LOCAL_LAST])
            .mul(current[SERIAL_SOURCE_COUNT_AFTER].sub(current[SERIAL_SOURCE_LENGTH])),
    );
    residues.push(
        serial_source_first_payload.mul(
            current[BASE_VALUE]
                .mul(current[SERIAL_SOURCE_FIRST_INVERSE])
                .sub(F::ONE),
        ),
    );
    residues.push(
        serial_source
            .mul(F::ONE.sub(fixed[FIX_EXPECTED + 4]))
            .mul(current[SERIAL_SOURCE_FIRST_INVERSE]),
    );
    residues.push(
        serial_source_first_payload.mul(current[BASE_BYTE_BITS + 7].sub(current[BASE_STRICT])),
    );
    residues.push(
        serial_source.mul(
            current[BASE_IS_WRITE]
                .sub(current[BASE_EQUAL].add(fixed[FIX_LOCAL_FIRST].mul(current[BASE_STRICT]))),
        ),
    );
    residues.push(
        serial_source
            .mul(current[BASE_EQUAL])
            .mul(current[SERIAL_SOURCE_QUERY_VALUE].sub(current[BASE_VALUE])),
    );
    residues.push(
        serial_source_first
            .mul(current[BASE_STRICT])
            .mul(current[SERIAL_SOURCE_QUERY_VALUE]),
    );
    residues.push(
        serial_source
            .mul(F::ONE.sub(current[BASE_IS_WRITE]))
            .mul(current[SERIAL_SOURCE_QUERY_VALUE]),
    );
    residues.push(
        serial_source.mul(current[BASE_EQUAL]).mul(
            current[BASE_ADDRESS]
                .add(current[SERIAL_SOURCE_LENGTH])
                .add(F::ONE)
                .sub(current[BASE_CONTENT_END])
                .sub(current[BASE_OFFSET]),
        ),
    );
    residues.push(
        serial_source_first
            .mul(current[BASE_STRICT])
            .mul(current[BASE_ADDRESS].sub(current[BASE_CONTENT_START])),
    );
    residues.push(
        serial_source
            .mul(F::ONE.sub(current[BASE_IS_WRITE]))
            .mul(current[BASE_ADDRESS]),
    );
    assert_residue_section_v1(
        residues.len(),
        &mut residue_section_start,
        RFC5280_RESIDUE_SECTIONS_V1[8],
    );

    let serial = current[BASE_ACTIVE].mul(fixed[FIX_SERIAL_COMPARE_PHASE_RIGHT]);
    let serial_first = current[BASE_ACTIVE].mul(fixed[FIX_SERIAL_COMPARE_FIRST]);
    let serial_last = current[BASE_ACTIVE].mul(fixed[FIX_SERIAL_COMPARE_LAST]);
    let serial_not_last = serial.sub(serial_last);
    let serial_interior = current[BASE_ACTIVE].mul(fixed[FIX_SERIAL_COMPARE_INTERIOR]);
    let serial_not_first = current[BASE_ACTIVE].mul(fixed[FIX_SERIAL_COMPARE_NOT_FIRST]);
    let serial_first_payload = current[BASE_ACTIVE].mul(fixed[FIX_SERIAL_COMPARE_FIRST_PAYLOAD]);
    for column in [
        BASE_ACTIVE,
        BASE_ROLE,
        BASE_INSTANCE,
        BASE_OFFSET,
        BASE_A,
        BASE_B,
    ] {
        residues.push(fixed[FIX_SERIAL_COMPARE_PHASE_LEFT].mul(next[column].sub(current[column])));
    }
    for (actual, expected) in [
        (current[BASE_STRICT], fixed[FIX_EXPECTED]),
        (current[BASE_ROLE], fixed[FIX_EXPECTED + 1]),
        (current[BASE_INSTANCE], fixed[FIX_EXPECTED + 2]),
        (current[BASE_OFFSET], fixed[FIX_EXPECTED + 3]),
    ] {
        residues.push(serial.mul(actual.sub(expected)));
    }
    for bit in &current[SERIAL_LEFT_BITS..SERIAL_SLACK_BITS + 8] {
        push_boolean_v1(&mut residues, serial, *bit);
    }
    residues.push(serial.mul(current[BASE_A].sub(pack_bits_v1(
        &current[SERIAL_LEFT_BITS..SERIAL_LEFT_BITS + 8],
    ))));
    residues.push(serial.mul(current[BASE_B].sub(pack_bits_v1(
        &current[SERIAL_RIGHT_BITS..SERIAL_RIGHT_BITS + 8],
    ))));
    residues.push(serial.mul(current[BASE_C].sub(pack_bits_v1(
        &current[SERIAL_SLACK_BITS..SERIAL_SLACK_BITS + 8],
    ))));
    for value in [
        current[SERIAL_LESS],
        current[SERIAL_ORDER_BEFORE],
        current[SERIAL_ORDER_AFTER],
    ] {
        push_boolean_v1(&mut residues, serial, value);
    }
    for active in [current[SERIAL_LEFT_ACTIVE], current[SERIAL_RIGHT_ACTIVE]] {
        push_boolean_v1(&mut residues, serial, active);
    }
    for (before, after, active) in [
        (
            current[SERIAL_LEFT_COUNT_BEFORE],
            current[SERIAL_LEFT_COUNT_AFTER],
            current[SERIAL_LEFT_ACTIVE],
        ),
        (
            current[SERIAL_RIGHT_COUNT_BEFORE],
            current[SERIAL_RIGHT_COUNT_AFTER],
            current[SERIAL_RIGHT_ACTIVE],
        ),
    ] {
        residues.push(serial.mul(after.sub(before.add(active))));
        residues.push(serial_first.mul(before));
        residues.push(serial_first.mul(active));
    }
    residues.push(serial_first.mul(current[SERIAL_LEFT_LENGTH].sub(current[BASE_A])));
    residues.push(serial_first.mul(current[SERIAL_RIGHT_LENGTH].sub(current[BASE_B])));
    for (length, next_length, count_after, next_count_before, active, next_active, value) in [
        (
            current[SERIAL_LEFT_LENGTH],
            next[SERIAL_LEFT_LENGTH],
            current[SERIAL_LEFT_COUNT_AFTER],
            next[SERIAL_LEFT_COUNT_BEFORE],
            current[SERIAL_LEFT_ACTIVE],
            next[SERIAL_LEFT_ACTIVE],
            current[BASE_A],
        ),
        (
            current[SERIAL_RIGHT_LENGTH],
            next[SERIAL_RIGHT_LENGTH],
            current[SERIAL_RIGHT_COUNT_AFTER],
            next[SERIAL_RIGHT_COUNT_BEFORE],
            current[SERIAL_RIGHT_ACTIVE],
            next[SERIAL_RIGHT_ACTIVE],
            current[BASE_B],
        ),
    ] {
        residues.push(serial_not_last.mul(next_length.sub(length)));
        residues.push(serial_not_last.mul(next_count_before.sub(count_after)));
        residues.push(serial_first.mul(next_active.sub(F::ONE)));
        residues.push(serial_interior.mul(next_active).mul(F::ONE.sub(active)));
        residues.push(serial_not_first.mul(F::ONE.sub(active)).mul(value));
        residues.push(serial_last.mul(count_after.sub(length)));
    }
    for (value, inverse) in [
        (current[BASE_A], current[SERIAL_LEFT_FIRST_INVERSE]),
        (current[BASE_B], current[SERIAL_RIGHT_FIRST_INVERSE]),
    ] {
        residues.push(serial_first_payload.mul(value.mul(inverse).sub(F::ONE)));
        residues.push(serial.mul(F::ONE.sub(fixed[FIX_EXPECTED + 4])).mul(inverse));
    }
    let difference = current[BASE_A].sub(current[BASE_B]);
    residues.push(
        serial.mul(
            difference
                .mul(current[BASE_INVERSE])
                .sub(F::ONE.sub(current[BASE_EQUAL])),
        ),
    );
    residues.push(serial.mul(current[BASE_EQUAL]).mul(difference));
    residues.push(serial.mul(current[BASE_EQUAL]).mul(current[BASE_INVERSE]));
    residues.push(
        serial.mul(
            current[BASE_STATE_AFTER].sub(current[BASE_STATE_BEFORE].mul(current[BASE_EQUAL])),
        ),
    );
    residues.push(serial_first.mul(current[BASE_STATE_BEFORE].sub(F::ONE)));
    residues.push(serial_not_last.mul(next[BASE_STATE_BEFORE].sub(current[BASE_STATE_AFTER])));
    residues.push(
        serial_last
            .mul(F::ONE.sub(current[BASE_STRICT]))
            .mul(current[BASE_STATE_AFTER]),
    );
    residues.push(
        serial
            .mul(current[SERIAL_LESS])
            .mul(F::ONE.sub(current[BASE_STRICT])),
    );
    residues.push(serial.mul(current[SERIAL_LESS]).mul(current[BASE_EQUAL]));
    residues.push(
        serial
            .mul(current[SERIAL_LESS])
            .mul(F::ONE.sub(current[BASE_STATE_BEFORE])),
    );
    residues.push(
        serial
            .mul(current[BASE_C])
            .mul(F::ONE.sub(current[SERIAL_LESS])),
    );
    residues.push(
        serial.mul(current[SERIAL_LESS]).mul(
            current[BASE_B]
                .sub(current[BASE_A])
                .sub(F::ONE)
                .sub(current[BASE_C]),
        ),
    );
    residues.push(serial.mul(current[SERIAL_ORDER_AFTER].sub(
        current[SERIAL_ORDER_BEFORE].add(current[BASE_STATE_BEFORE].mul(current[SERIAL_LESS])),
    )));
    residues.push(serial_first.mul(current[SERIAL_ORDER_BEFORE]));
    residues.push(serial_not_last.mul(next[SERIAL_ORDER_BEFORE].sub(current[SERIAL_ORDER_AFTER])));
    residues.push(
        serial_last
            .mul(current[BASE_STRICT])
            .mul(current[SERIAL_ORDER_AFTER].sub(F::ONE)),
    );
    assert_residue_section_v1(
        residues.len(),
        &mut residue_section_start,
        RFC5280_RESIDUE_SECTIONS_V1[9],
    );

    let range = active_family_gate_v1(current, fixed, ZkX509Rfc5280StarkFamilyV1::RangeByte);
    residues.push(
        range.mul(
            current[BASE_STATE_AFTER].sub(
                current[BASE_STATE_BEFORE]
                    .mul(F(256))
                    .add(current[BASE_VALUE]),
            ),
        ),
    );
    residues.push(
        range
            .mul(fixed[FIX_LOCAL_FIRST])
            .mul(current[BASE_STATE_BEFORE]),
    );
    residues.push(
        range
            .mul(F::ONE.sub(fixed[FIX_LOCAL_LAST]))
            .mul(next[BASE_ROLE].sub(current[BASE_ROLE])),
    );
    residues.push(
        range
            .mul(F::ONE.sub(fixed[FIX_LOCAL_LAST]))
            .mul(next[BASE_INSTANCE].sub(current[BASE_INSTANCE])),
    );

    let profile_table =
        active_family_gate_v1(current, fixed, ZkX509Rfc5280StarkFamilyV1::SemanticSource);
    for (actual, expected) in [
        (current[BASE_ROLE], fixed[FIX_EXPECTED]),
        (current[BASE_ENDPOINT_ROLE], fixed[FIX_EXPECTED + 1]),
        (current[BASE_PARENT], fixed[FIX_EXPECTED + 2]),
        (current[BASE_OFFSET], fixed[FIX_EXPECTED + 3]),
        (current[BASE_B], fixed[FIX_EXPECTED + 4]),
        (current[BASE_VALUE], fixed[FIX_EXPECTED + 5]),
        (current[BASE_ENDPOINT_INSTANCE], fixed[FIX_EXPECTED + 6]),
        (current[BASE_CHILD], fixed[FIX_EXPECTED + 7]),
    ] {
        residues.push(profile_table.mul(actual.sub(expected)));
    }
    assert_residue_section_v1(
        residues.len(),
        &mut residue_section_start,
        RFC5280_RESIDUE_SECTIONS_V1[10],
    );
    let output_producer =
        active_family_gate_v1(current, fixed, ZkX509Rfc5280StarkFamilyV1::OutputProducer);
    let output_consumer =
        active_family_gate_v1(current, fixed, ZkX509Rfc5280StarkFamilyV1::OutputConsumer);
    // Every private family is a canonical active prefix of its fixed
    // registration segment. Top-level byte/node slots reset the prefix at
    // their fixed 4 KiB/2,048-node boundaries.
    residues.extend(private_geometry_residues_v1(current, next, fixed));
    assert_residue_section_v1(
        residues.len(),
        &mut residue_section_start,
        RFC5280_RESIDUE_SECTIONS_V1[11],
    );

    let first = fixed[FIX_GLOBAL_FIRST];
    let last = fixed[FIX_GLOBAL_LAST];
    let continue_gate = fixed[FIX_CONTINUE];
    for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
        let der_byte_factor = zk_x509_der_stark_input_byte_factor_v1(
            current[BASE_DOCUMENT],
            current[BASE_ADDRESS],
            current[BASE_VALUE],
            lane,
            der_challenges,
        )?;
        let der_node_factor = zk_x509_der_stark_node_factor_v1(
            ZkX509DerStarkNodeEventV1 {
                document: current[BASE_DOCUMENT],
                ordinal: current[BASE_NODE],
                parent_frame: current[BASE_PARENT],
                tag_class: current[BASE_TAG_CLASS],
                tag_number: current[BASE_TAG_NUMBER],
                constructed: current[BASE_CONSTRUCTED],
                start: current[BASE_START],
                content_start: current[BASE_CONTENT_START],
                content_end: current[BASE_CONTENT_END],
                depth: current[BASE_DEPTH],
                content_len: current[BASE_A],
            },
            lane,
            der_challenges,
        )?;
        for (before, after, gate, factor) in [
            (
                AUX_DER_BYTE_BEFORE + lane,
                AUX_DER_BYTE_AFTER + lane,
                source_byte,
                der_byte_factor,
            ),
            (
                AUX_DER_NODE_BEFORE + lane,
                AUX_DER_NODE_AFTER + lane,
                source_node,
                der_node_factor,
            ),
            (
                AUX_OUTPUT_PRODUCER_BEFORE + lane,
                AUX_OUTPUT_PRODUCER_AFTER + lane,
                output_producer,
                output_row_factor_v1(current, lane, challenges),
            ),
            (
                AUX_OUTPUT_CONSUMER_BEFORE + lane,
                AUX_OUTPUT_CONSUMER_AFTER + lane,
                output_consumer,
                output_row_factor_v1(current, lane, challenges),
            ),
        ] {
            residues.push(
                current_aux[after]
                    .sub(current_aux[before].mul(F::ONE.add(gate.mul(factor.sub(F::ONE))))),
            );
            residues.push(first.mul(current_aux[before].sub(F::ONE)));
            residues.push(continue_gate.mul(next_aux[before].sub(current_aux[after])));
        }
    }
    assert_residue_section_v1(
        residues.len(),
        &mut residue_section_start,
        RFC5280_RESIDUE_SECTIONS_V1[12],
    );
    residues.extend(evaluate_zk_x509_rfc5280_terminal_claim_residues_v1(
        last,
        current_aux,
        terminal_claims,
    )?);
    assert_residue_section_v1(
        residues.len(),
        &mut residue_section_start,
        RFC5280_RESIDUE_SECTIONS_V1[13],
    );
    for role_index in 0..OUTPUT_ROLE_COUNT_V1 {
        for consumer in [false, true] {
            let selector = fixed[output_role_fixed_selector_column_v1(role_index, consumer)];
            let gate = current[BASE_ACTIVE].mul(selector);
            for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
                let product = current_aux[output_role_aux_column_v1(role_index, consumer, lane)];
                residues.push(
                    continue_gate
                        .mul(
                            next_aux[output_role_aux_column_v1(role_index, consumer, lane)]
                                .sub(product),
                        )
                        .sub(
                            gate.mul(product)
                                .mul(output_row_factor_v1(current, lane, challenges).sub(F::ONE)),
                        ),
                );
                residues.push(first.mul(product.sub(F::ONE)));
            }
        }
    }
    assert_residue_section_v1(
        residues.len(),
        &mut residue_section_start,
        RFC5280_RESIDUE_SECTIONS_V1[14],
    );
    for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
        let mut lane_section_start = residues.len();
        let copy_factor = normalized_copy_factor_v1(current, lane, challenges);
        residues.push(
            current_aux[AUX_SERIAL_SOURCE_AFTER + lane]
                .sub(current_aux[AUX_SERIAL_SOURCE_BEFORE + lane].mul(
                    F::ONE.add(current[BASE_COPY_SOURCE_ACTIVE].mul(copy_factor.sub(F::ONE))),
                )),
        );
        residues.push(
            current_aux[AUX_SERIAL_CONSUMER_AFTER + lane].sub(
                current_aux[AUX_SERIAL_CONSUMER_BEFORE + lane].mul(
                    F::ONE.add(current[BASE_COPY_CONSUMER_ACTIVE].mul(copy_factor.sub(F::ONE))),
                ),
            ),
        );
        residues.push(first.mul(current_aux[AUX_SERIAL_SOURCE_BEFORE + lane].sub(F::ONE)));
        residues.push(first.mul(current_aux[AUX_SERIAL_CONSUMER_BEFORE + lane].sub(F::ONE)));
        residues.push(
            continue_gate.mul(
                next_aux[AUX_SERIAL_SOURCE_BEFORE + lane]
                    .sub(current_aux[AUX_SERIAL_SOURCE_AFTER + lane]),
            ),
        );
        residues.push(
            continue_gate.mul(
                next_aux[AUX_SERIAL_CONSUMER_BEFORE + lane]
                    .sub(current_aux[AUX_SERIAL_CONSUMER_AFTER + lane]),
            ),
        );
        residues.push(
            last.mul(
                current_aux[AUX_SERIAL_SOURCE_AFTER + lane]
                    .sub(current_aux[AUX_SERIAL_CONSUMER_AFTER + lane]),
            ),
        );
        assert_residue_section_v1(
            residues.len(),
            &mut lane_section_start,
            (
                RFC5280_RESIDUE_SECTIONS_V1[15].0,
                RFC5280_RESIDUE_SECTIONS_V1[15].1 / ZK_X509_RFC5280_STARK_BUS_LANES_V1,
            ),
        );

        let grammar_ordinal_source_factor = grammar_ordinal_factor_v1(
            current[BASE_DOCUMENT],
            current[BASE_PARENT],
            current[BASE_CHILD],
            current[BASE_G],
            lane,
            challenges,
        );
        let grammar_ordinal_table_factor = grammar_ordinal_factor_v1(
            current[BASE_DOCUMENT],
            current[BASE_PARENT],
            current[BASE_CHILD],
            current[BASE_D],
            lane,
            challenges,
        );
        for (before, after, gate, factor) in [
            (
                AUX_GRAMMAR_ORDINAL_SOURCE_BEFORE + lane,
                AUX_GRAMMAR_ORDINAL_SOURCE_AFTER + lane,
                current[BASE_ACTIVE].mul(fixed[FIX_SOURCE_NODE_NON_ROOT]),
                grammar_ordinal_source_factor,
            ),
            (
                AUX_GRAMMAR_ORDINAL_TABLE_BEFORE + lane,
                AUX_GRAMMAR_ORDINAL_TABLE_AFTER + lane,
                grammar_ordinal_table,
                grammar_ordinal_table_factor,
            ),
        ] {
            residues.push(
                current_aux[after]
                    .sub(current_aux[before].mul(F::ONE.add(gate.mul(factor.sub(F::ONE))))),
            );
            residues.push(first.mul(current_aux[before].sub(F::ONE)));
            residues.push(continue_gate.mul(next_aux[before].sub(current_aux[after])));
        }
        residues.push(
            last.mul(
                current_aux[AUX_GRAMMAR_ORDINAL_SOURCE_AFTER + lane]
                    .sub(current_aux[AUX_GRAMMAR_ORDINAL_TABLE_AFTER + lane]),
            ),
        );
        assert_residue_section_v1(
            residues.len(),
            &mut lane_section_start,
            (
                RFC5280_RESIDUE_SECTIONS_V1[16].0,
                RFC5280_RESIDUE_SECTIONS_V1[16].1 / ZK_X509_RFC5280_STARK_BUS_LANES_V1,
            ),
        );

        let profile_table_gate = current[BASE_PROFILE_TABLE_ACTIVE];
        let profile_table_factor = fixed[FIX_PROFILE_TABLE]
            .mul(profile_byte_factor_v1(current, lane, challenges))
            .add(
                fixed[ZkX509Rfc5280StarkFamilyV1::SourceNode as usize]
                    .mul(profile_topology_source_factor_v1(current, lane, challenges)),
            );
        let profile_table_multiplicity = current[BASE_PROFILE_TABLE_MULTIPLICITY];
        let profile_query_factor = profile_byte_factor_v1(current, lane, challenges);
        let profile_topology_query = current[BASE_PROFILE_TOPOLOGY_QUERY_ACTIVE];
        let profile_topology_query_factor =
            profile_topology_query_factor_v1(current, lane, challenges);
        push_gated_zero_safe_inverse_v1(
            &mut residues,
            profile_table_gate,
            profile_table_factor,
            current_aux[AUX_PROFILE_TABLE_ZERO + lane],
            current_aux[AUX_PROFILE_TABLE_INVERSE + lane],
        );
        push_gated_zero_safe_inverse_v1(
            &mut residues,
            fixed_byte,
            profile_query_factor,
            current_aux[AUX_PROFILE_QUERY_ZERO + lane],
            current_aux[AUX_PROFILE_QUERY_INVERSE + lane],
        );
        push_gated_zero_safe_inverse_v1(
            &mut residues,
            profile_topology_query,
            profile_topology_query_factor,
            current_aux[AUX_PROFILE_TOPOLOGY_QUERY_ZERO + lane],
            current_aux[AUX_PROFILE_TOPOLOGY_QUERY_INVERSE + lane],
        );
        residues.push(
            continue_gate
                .mul(
                    next_aux[AUX_PROFILE_LOOKUP_ACCUMULATOR + lane]
                        .sub(current_aux[AUX_PROFILE_LOOKUP_ACCUMULATOR + lane]),
                )
                .sub(
                    profile_table_gate
                        .mul(profile_table_multiplicity)
                        .mul(current_aux[AUX_PROFILE_TABLE_INVERSE + lane]),
                )
                .add(fixed_byte.mul(current_aux[AUX_PROFILE_QUERY_INVERSE + lane]))
                .add(
                    profile_topology_query
                        .mul(current_aux[AUX_PROFILE_TOPOLOGY_QUERY_INVERSE + lane]),
                ),
        );
        residues.push(
            continue_gate
                .mul(
                    next_aux[AUX_PROFILE_ZERO_ACCUMULATOR + lane]
                        .sub(current_aux[AUX_PROFILE_ZERO_ACCUMULATOR + lane]),
                )
                .sub(
                    profile_table_gate
                        .mul(profile_table_multiplicity)
                        .mul(current_aux[AUX_PROFILE_TABLE_ZERO + lane]),
                )
                .add(fixed_byte.mul(current_aux[AUX_PROFILE_QUERY_ZERO + lane]))
                .add(
                    profile_topology_query.mul(current_aux[AUX_PROFILE_TOPOLOGY_QUERY_ZERO + lane]),
                ),
        );
        for accumulator in [
            AUX_PROFILE_LOOKUP_ACCUMULATOR + lane,
            AUX_PROFILE_ZERO_ACCUMULATOR + lane,
        ] {
            residues.push(first.mul(current_aux[accumulator]));
            residues.push(last.mul(current_aux[accumulator]));
        }
        assert_residue_section_v1(
            residues.len(),
            &mut lane_section_start,
            (
                RFC5280_RESIDUE_SECTIONS_V1[17].0,
                RFC5280_RESIDUE_SECTIONS_V1[17].1 / ZK_X509_RFC5280_STARK_BUS_LANES_V1,
            ),
        );

        for (
            accumulator,
            zero_accumulator,
            table_inverse,
            query_inverse,
            table_zero,
            query_zero,
            table_gate,
            query_gate,
            table_factor,
            query_factor,
            multiplicity,
        ) in [
            (
                AUX_SERIAL_BYTE_LOOKUP_ACCUMULATOR + lane,
                AUX_SERIAL_BYTE_ZERO_ACCUMULATOR + lane,
                AUX_SERIAL_BYTE_TABLE_INVERSE + lane,
                AUX_SERIAL_BYTE_QUERY_INVERSE + lane,
                AUX_SERIAL_BYTE_TABLE_ZERO + lane,
                AUX_SERIAL_BYTE_QUERY_ZERO + lane,
                source_byte,
                current[BASE_SERIAL_BYTE_QUERY_ACTIVE],
                serial_byte_lookup_factor_v1(
                    current[BASE_DOCUMENT],
                    current[BASE_ADDRESS],
                    current[BASE_VALUE],
                    lane,
                    challenges,
                ),
                serial_byte_lookup_factor_v1(
                    current[BASE_DOCUMENT],
                    current[BASE_ADDRESS],
                    current[BASE_SERIAL_BYTE_QUERY_VALUE],
                    lane,
                    challenges,
                ),
                current[SERIAL_BYTE_TABLE_MULTIPLICITY],
            ),
            (
                AUX_SERIAL_NODE_LOOKUP_ACCUMULATOR + lane,
                AUX_SERIAL_NODE_ZERO_ACCUMULATOR + lane,
                AUX_SERIAL_NODE_TABLE_INVERSE + lane,
                AUX_SERIAL_NODE_QUERY_INVERSE + lane,
                AUX_SERIAL_NODE_TABLE_ZERO + lane,
                AUX_SERIAL_NODE_QUERY_ZERO + lane,
                source_node,
                serial_source,
                serial_node_lookup_factor_v1(current, lane, challenges),
                serial_node_lookup_factor_v1(current, lane, challenges),
                current[SERIAL_NODE_TABLE_MULTIPLICITY],
            ),
        ] {
            push_gated_zero_safe_inverse_v1(
                &mut residues,
                table_gate,
                table_factor,
                current_aux[table_zero],
                current_aux[table_inverse],
            );
            push_gated_zero_safe_inverse_v1(
                &mut residues,
                query_gate,
                query_factor,
                current_aux[query_zero],
                current_aux[query_inverse],
            );
            residues.push(
                continue_gate
                    .mul(next_aux[accumulator].sub(current_aux[accumulator]))
                    .sub(table_gate.mul(multiplicity).mul(current_aux[table_inverse]))
                    .add(query_gate.mul(current_aux[query_inverse])),
            );
            residues.push(
                continue_gate
                    .mul(next_aux[zero_accumulator].sub(current_aux[zero_accumulator]))
                    .sub(table_gate.mul(multiplicity).mul(current_aux[table_zero]))
                    .add(query_gate.mul(current_aux[query_zero])),
            );
            residues.push(first.mul(current_aux[accumulator]));
            residues.push(first.mul(current_aux[zero_accumulator]));
            residues.push(last.mul(current_aux[accumulator]));
            residues.push(last.mul(current_aux[zero_accumulator]));
        }
        assert_residue_section_v1(
            residues.len(),
            &mut lane_section_start,
            (
                RFC5280_RESIDUE_SECTIONS_V1[18].0,
                RFC5280_RESIDUE_SECTIONS_V1[18].1 / ZK_X509_RFC5280_STARK_BUS_LANES_V1,
            ),
        );

        for (
            accumulator,
            zero_accumulator,
            table_inverse,
            query_inverse,
            table_zero,
            query_zero,
            table_gate,
            query_gate,
            table_factor,
            query_factor,
            multiplicity,
        ) in [
            (
                AUX_GRAMMAR_RULE_LOOKUP_ACCUMULATOR + lane,
                AUX_GRAMMAR_RULE_ZERO_ACCUMULATOR + lane,
                AUX_GRAMMAR_RULE_TABLE_INVERSE + lane,
                AUX_GRAMMAR_RULE_QUERY_INVERSE + lane,
                AUX_GRAMMAR_RULE_TABLE_ZERO + lane,
                AUX_GRAMMAR_RULE_QUERY_ZERO + lane,
                grammar_rule_table,
                source_node,
                grammar_rule_table_factor_v1(fixed, lane, challenges),
                grammar_rule_query_factor_v1(current, lane, challenges),
                current[BASE_A],
            ),
            (
                AUX_GRAMMAR_PARENT_LOOKUP_ACCUMULATOR + lane,
                AUX_GRAMMAR_PARENT_ZERO_ACCUMULATOR + lane,
                AUX_GRAMMAR_PARENT_TABLE_INVERSE + lane,
                AUX_GRAMMAR_PARENT_QUERY_INVERSE + lane,
                AUX_GRAMMAR_PARENT_TABLE_ZERO + lane,
                AUX_GRAMMAR_PARENT_QUERY_ZERO + lane,
                source_node,
                current[BASE_ACTIVE].mul(fixed[FIX_SOURCE_NODE_NON_ROOT]),
                grammar_parent_table_factor_v1(current, lane, challenges),
                grammar_parent_query_factor_v1(current, lane, challenges),
                current[BASE_D],
            ),
        ] {
            push_gated_zero_safe_inverse_v1(
                &mut residues,
                table_gate,
                table_factor,
                current_aux[table_zero],
                current_aux[table_inverse],
            );
            push_gated_zero_safe_inverse_v1(
                &mut residues,
                query_gate,
                query_factor,
                current_aux[query_zero],
                current_aux[query_inverse],
            );
            residues.push(
                continue_gate
                    .mul(next_aux[accumulator].sub(current_aux[accumulator]))
                    .sub(table_gate.mul(multiplicity).mul(current_aux[table_inverse]))
                    .add(query_gate.mul(current_aux[query_inverse])),
            );
            residues.push(
                continue_gate
                    .mul(next_aux[zero_accumulator].sub(current_aux[zero_accumulator]))
                    .sub(table_gate.mul(multiplicity).mul(current_aux[table_zero]))
                    .add(query_gate.mul(current_aux[query_zero])),
            );
            residues.push(first.mul(current_aux[accumulator]));
            residues.push(first.mul(current_aux[zero_accumulator]));
            residues.push(last.mul(current_aux[accumulator]));
            residues.push(last.mul(current_aux[zero_accumulator]));
        }
        assert_residue_section_v1(
            residues.len(),
            &mut lane_section_start,
            (
                RFC5280_RESIDUE_SECTIONS_V1[19].0,
                RFC5280_RESIDUE_SECTIONS_V1[19].1 / ZK_X509_RFC5280_STARK_BUS_LANES_V1,
            ),
        );
    }
    debug_assert_eq!(
        residues.len() - residue_section_start,
        RFC5280_RESIDUE_SECTIONS_V1[15..]
            .iter()
            .map(|section| section.1)
            .sum::<usize>(),
        "RFC 5280 four-lane residue sections drifted"
    );
    if residues.len() != ZK_X509_RFC5280_STARK_CONSTRAINT_COUNT_V1 {
        return Err(ZkX509Rfc5280StarkErrorV1::Semantic);
    }
    Ok(residues)
}

#[cfg(test)]
mod tests {
    use sha2::{Digest as _, Sha256};

    use super::*;
    use crate::privacy_engines::zk_x509::{
        der_air::build_zk_x509_rfc5280_trace_v1, relation::tests::fixture,
    };

    fn challenges_v1() -> ZkX509Rfc5280StarkChallengesV1 {
        ZkX509Rfc5280StarkChallengesV1 {
            tuple: core::array::from_fn(|lane| {
                core::array::from_fn(|slot| {
                    F(u64::try_from(10_000 + lane * 100 + slot).expect("challenge fits"))
                })
            }),
        }
    }

    fn der_challenges_v1() -> ZkX509DerStarkChallengesV1 {
        ZkX509DerStarkChallengesV1 {
            tuple: core::array::from_fn(|lane| {
                core::array::from_fn(|slot| {
                    F(u64::try_from(20_000 + lane * 100 + slot).expect("challenge fits"))
                })
            }),
            byte_lookup: core::array::from_fn(|lane| {
                F(u64::try_from(30_000 + lane).expect("challenge fits"))
            }),
        }
    }

    fn terminal_claims_v1() -> ZkX509Rfc5280StarkTerminalClaimsV1 {
        ZkX509Rfc5280StarkTerminalClaimsV1::canonical_identity_v1()
    }

    fn recompute_output_aggregates_v1(claims: &mut ZkX509Rfc5280StarkTerminalClaimsV1) {
        for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
            claims.relations[4][lane] = claims.output_roles.iter().fold(F::ONE, |product, role| {
                product.mul(role.producer_products[lane])
            });
            claims.relations[5][lane] = claims.output_roles.iter().fold(F::ONE, |product, role| {
                product.mul(role.consumer_products[lane])
            });
        }
    }

    fn nontrivial_terminal_claims_v1() -> ZkX509Rfc5280StarkTerminalClaimsV1 {
        let mut claims = terminal_claims_v1();
        claims.relations[0] =
            core::array::from_fn(|lane| F(100 + u64::try_from(lane).expect("small DER byte lane")));
        claims.relations[1] =
            core::array::from_fn(|lane| F(200 + u64::try_from(lane).expect("small DER node lane")));
        for (role_index, role) in claims.output_roles.iter_mut().enumerate() {
            for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
                role.producer_products[lane] =
                    F(1_000
                        + u64::try_from(role_index * 100 + lane).expect("small producer fixture"));
                role.consumer_products[lane] =
                    F(2_000
                        + u64::try_from(role_index * 100 + lane).expect("small consumer fixture"));
            }
        }
        recompute_output_aggregates_v1(&mut claims);
        claims
    }

    fn terminal_aux_v1(claims: ZkX509Rfc5280StarkTerminalClaimsV1) -> ZkX509Rfc5280StarkAuxRowV1 {
        let mut aux = [F::ZERO; ZK_X509_RFC5280_STARK_AUX_WIDTH_V1];
        for (_, relation, after) in RFC5280_TERMINAL_CLAIM_RELATIONS_V1 {
            aux[after..after + ZK_X509_RFC5280_STARK_BUS_LANES_V1]
                .copy_from_slice(&claims.relations[relation]);
        }
        for role_index in 0..OUTPUT_ROLE_COUNT_V1 {
            for consumer in [false, true] {
                let products = if consumer {
                    claims.output_roles[role_index].consumer_products
                } else {
                    claims.output_roles[role_index].producer_products
                };
                for (lane, product) in products.into_iter().enumerate() {
                    aux[output_role_aux_column_v1(role_index, consumer, lane)] = product;
                }
            }
        }
        aux
    }

    fn sha_segment_terminal_claims_v1() -> ZkX509ShaSegmentTerminalClaimsV1 {
        let mut claims = ZkX509ShaSegmentTerminalClaimsV1 {
            segments: core::array::from_fn(|segment| ZkX509ShaSegmentTerminalV1 {
                segment: u8::try_from(segment).expect("SHA segment fits u8"),
                source_products: [F::ZERO; ZK_X509_SHA_BUS_LANES_V1],
                digest_products: [F::ZERO; ZK_X509_SHA_BUS_LANES_V1],
                rfc_stream_products: [[F::ZERO; ZK_X509_SHA_BUS_LANES_V1];
                    SHA_TERMINAL_CLAIM_RFC_STREAMS_V1],
            }),
            ca_calls: core::array::from_fn(|index| {
                let (call, role) =
                    zk_x509_sha_ca_call_identity_v1(index).expect("canonical compact-CA call");
                ZkX509ShaCallBoundaryTerminalV1 {
                    call,
                    role,
                    source_start_products: [F::ZERO; ZK_X509_SHA_BUS_LANES_V1],
                    digest_start_products: [F::ZERO; ZK_X509_SHA_BUS_LANES_V1],
                    source_products: [F::ZERO; ZK_X509_SHA_BUS_LANES_V1],
                    digest_products: [F::ZERO; ZK_X509_SHA_BUS_LANES_V1],
                }
            }),
        };
        for claim_index in 0..SHA_TERMINAL_CLAIM_RECORDS_V1 {
            claims
                .set_claim_value_v1(
                    claim_index,
                    F(1_000 + u64::try_from(claim_index).expect("claim index fits u64")),
                )
                .expect("canonical fixture claim");
        }
        claims
    }

    fn p256_bus_terminal_claims_v1(signature: usize) -> P256BusTerminalClaimsV1 {
        let family = |family: usize| -> [F; P256_CROSS_TRACE_LANES_V1] {
            core::array::from_fn(|lane| {
                F(10_000
                    + u64::try_from(signature * 1_000 + family * 10 + lane)
                        .expect("fixture P-256 bus value fits u64"))
            })
        };
        let value = family(0);
        let arithmetic_copy = family(1);
        let arithmetic_scalar = family(2);
        let window_scalar = family(3);
        P256BusTerminalClaimsV1 {
            value_execution: value,
            value_sorted: value,
            value_arithmetic_copy: arithmetic_copy,
            arithmetic_value_copy: arithmetic_copy,
            arithmetic_scalar,
            window_scalar,
            scalar_bus_arithmetic: arithmetic_scalar,
            scalar_bus_window: window_scalar,
        }
    }

    fn p256_certificate_terminal_claims_v1(
        signature: usize,
    ) -> ZkX509P256CertificateTerminalClaimsV1 {
        let mut running = [F::ONE; P256_CROSS_TRACE_LANES_V1];
        let cross_sources = core::array::from_fn(|role| {
            let terminal = core::array::from_fn(|lane| {
                F(100_000
                    + u64::try_from(signature * 1_000 + role * 10 + lane)
                        .expect("fixture P-256 cross value fits u64"))
            });
            let claim = P256CrossTraceTerminalClaimV1 {
                role: P256_CERTIFICATE_CROSS_ROLES_V1[role],
                start: running,
                terminal,
            };
            running = terminal;
            claim
        });
        ZkX509P256CertificateTerminalClaimsV1 {
            buses: p256_bus_terminal_claims_v1(signature),
            cross_sources,
            sink: running,
        }
    }

    fn p256_wallet_terminal_claims_v1() -> ZkX509P256WalletTerminalClaimsV1 {
        let signature = P256_X5S1_SIGNATURES_V1 - 1;
        let mut running = [F::ONE; P256_CROSS_TRACE_LANES_V1];
        let cross_sources = core::array::from_fn(|role| {
            let terminal = core::array::from_fn(|lane| {
                F(100_000
                    + u64::try_from(signature * 1_000 + role * 10 + lane)
                        .expect("fixture P-256 cross value fits u64"))
            });
            let claim = P256CrossTraceTerminalClaimV1 {
                role: P256_WALLET_CROSS_ROLES_V1[role],
                start: running,
                terminal,
            };
            running = terminal;
            claim
        });
        ZkX509P256WalletTerminalClaimsV1 {
            buses: p256_bus_terminal_claims_v1(signature),
            cross_sources,
            sink: running,
        }
    }

    fn p256_terminal_claims_v1() -> ZkX509P256TerminalClaimsV1 {
        ZkX509P256TerminalClaimsV1::from_p256_air_terminals_v1(
            core::array::from_fn(p256_certificate_terminal_claims_v1),
            p256_wallet_terminal_claims_v1(),
        )
        .expect("canonical five-signature P-256 AIR terminals")
    }

    fn canonical_trace_v1() -> ZkX509Rfc5280TraceV1 {
        let fixture = fixture();
        build_zk_x509_rfc5280_trace_v1(
            &fixture.witness.certificate_chain_der,
            &fixture.witness.crl_der,
            ZkX509Rfc5280StatementV1 {
                presentation_not_before_unix_seconds: fixture
                    .statement
                    .presentation_not_before_unix_seconds,
                presentation_not_after_unix_seconds: fixture
                    .statement
                    .presentation_not_after_unix_seconds,
                leaf_key_usage: 1,
                leaf_extended_key_usages: vec![ZkX509DerEkuV1::ClientAuthentication],
                crl_number: fixture.crl.crl_number,
                disclosed_attribute_indices: fixture
                    .statement
                    .disclosed_attributes
                    .iter()
                    .map(|attribute| attribute.index)
                    .collect(),
            },
        )
        .expect("canonical RFC trace")
    }

    fn neutral_aux_v1() -> ZkX509Rfc5280StarkAuxRowV1 {
        let mut aux = [F::ZERO; ZK_X509_RFC5280_STARK_AUX_WIDTH_V1];
        aux[AUX_DER_BYTE_BEFORE..AUX_PROFILE_LOOKUP_ACCUMULATOR].fill(F::ONE);
        aux[AUX_OUTPUT_PRODUCER_BEFORE..AUX_SERIAL_BYTE_LOOKUP_ACCUMULATOR].fill(F::ONE);
        aux[AUX_GRAMMAR_ORDINAL_SOURCE_BEFORE..ZK_X509_RFC5280_STARK_AUX_WIDTH_V1].fill(F::ONE);
        aux
    }

    fn normalized_row_v1(
        mut row: ZkX509Rfc5280StarkBaseRowV1,
        fixed: &ZkX509Rfc5280StarkFixedRowV1,
    ) -> ZkX509Rfc5280StarkBaseRowV1 {
        populate_degree_normalization_helpers_v1(&mut row, fixed);
        row
    }

    fn serial_comparator_aux_v1(
        current: &ZkX509Rfc5280StarkBaseRowV1,
    ) -> (ZkX509Rfc5280StarkAuxRowV1, ZkX509Rfc5280StarkAuxRowV1) {
        let challenges = challenges_v1();
        let mut current_aux = neutral_aux_v1();
        let mut next_aux = neutral_aux_v1();
        for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
            let factor = normalized_copy_factor_v1(current, lane, challenges);
            let after = F::ONE.add(current[BASE_COPY_CONSUMER_ACTIVE].mul(factor.sub(F::ONE)));
            current_aux[AUX_SERIAL_CONSUMER_AFTER + lane] = after;
            next_aux[AUX_SERIAL_CONSUMER_BEFORE + lane] = after;
        }
        (current_aux, next_aux)
    }

    fn serial_source_aux_v1(
        current: &ZkX509Rfc5280StarkBaseRowV1,
        challenges: ZkX509Rfc5280StarkChallengesV1,
    ) -> (ZkX509Rfc5280StarkAuxRowV1, ZkX509Rfc5280StarkAuxRowV1) {
        let mut current_aux = neutral_aux_v1();
        let mut next_aux = neutral_aux_v1();
        for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
            let factor = normalized_copy_factor_v1(current, lane, challenges);
            let source = F::ONE.add(current[BASE_COPY_SOURCE_ACTIVE].mul(factor.sub(F::ONE)));
            current_aux[AUX_SERIAL_SOURCE_AFTER + lane] = source;
            next_aux[AUX_SERIAL_SOURCE_BEFORE + lane] = source;

            if current[BASE_IS_WRITE] == F::ONE {
                let factor = serial_byte_lookup_factor_v1(
                    current[BASE_DOCUMENT],
                    current[BASE_ADDRESS],
                    current[SERIAL_SOURCE_QUERY_VALUE],
                    lane,
                    challenges,
                );
                if factor == F::ZERO {
                    current_aux[AUX_SERIAL_BYTE_QUERY_ZERO + lane] = F::ONE;
                    next_aux[AUX_SERIAL_BYTE_ZERO_ACCUMULATOR + lane] = F::ZERO.sub(F::ONE);
                } else {
                    let inverse = factor.inv().expect("nonzero byte lookup factor");
                    current_aux[AUX_SERIAL_BYTE_QUERY_INVERSE + lane] = inverse;
                    next_aux[AUX_SERIAL_BYTE_LOOKUP_ACCUMULATOR + lane] = F::ZERO.sub(inverse);
                }
            }
            let node_factor = serial_node_lookup_factor_v1(current, lane, challenges);
            if node_factor == F::ZERO {
                current_aux[AUX_SERIAL_NODE_QUERY_ZERO + lane] = F::ONE;
                next_aux[AUX_SERIAL_NODE_ZERO_ACCUMULATOR + lane] = F::ZERO.sub(F::ONE);
            } else {
                let node_inverse = node_factor.inv().expect("nonzero node lookup factor");
                current_aux[AUX_SERIAL_NODE_QUERY_INVERSE + lane] = node_inverse;
                next_aux[AUX_SERIAL_NODE_LOOKUP_ACCUMULATOR + lane] = F::ZERO.sub(node_inverse);
            }
        }
        (current_aux, next_aux)
    }

    fn serial_source_fixture_v1(logical_id: u16, magnitude: &[u8]) -> ZkX509Rfc5280SerialSourceV1 {
        let sign_padding = magnitude[0] & 0x80 != 0;
        let mut encoded = Vec::new();
        if sign_padding {
            encoded.push(0);
        }
        encoded.extend_from_slice(magnitude);
        let content_start = 100_u16;
        let content_end = content_start + u16::try_from(encoded.len()).expect("fixture length");
        let (document, role, role_instance) = if logical_id == 0 {
            (0, ZkX509Rfc5280GrammarRoleV1::CertificateSerial, 0)
        } else {
            (
                2,
                ZkX509Rfc5280GrammarRoleV1::CrlEntrySerial,
                logical_id - 1,
            )
        };
        ZkX509Rfc5280SerialSourceV1 {
            logical_id,
            node: ZkX509Rfc5280NodeProvenanceV1 {
                document,
                node: 7 + logical_id,
                parent_node: 2,
                child_ordinal: 1,
                start: 98,
                content_start,
                content_end,
                depth: 2,
                tag_class: 0,
                constructed: false,
                tag_number: 2,
                role,
                role_instance,
            },
            frame: serial_frame_v1(magnitude).expect("fixture frame"),
            encoded_contents: encoded
                .into_iter()
                .enumerate()
                .map(|(offset, value)| ZkX509Rfc5280SourceCellV1 {
                    document,
                    address: content_start + u16::try_from(offset).expect("fixture offset"),
                    value,
                })
                .collect(),
        }
    }

    fn maximum_private_shape_v1() -> ZkX509Rfc5280StarkPrivateShapeV1 {
        let mut embedded_lengths = [0_u16; ZK_X509_DER_AIR_MAX_EMBEDDED_DOCUMENTS_V1];
        embedded_lengths[..14].fill(1_092);
        embedded_lengths[14] = 1_096;
        ZkX509Rfc5280StarkPrivateShapeV1 {
            chain_depth: 3,
            certificate_slot_2_active: F::ONE,
            top_document_count: 4,
            top_document_lengths: [4_096; ZK_X509_DER_AIR_MAX_DOCUMENTS_V1],
            top_node_counts: [2_048; ZK_X509_DER_AIR_MAX_DOCUMENTS_V1],
            embedded_document_count: 15,
            embedded_document_lengths: embedded_lengths,
            embedded_node_counts: [2_048; ZK_X509_DER_AIR_MAX_EMBEDDED_DOCUMENTS_V1],
            crl_entries: 64,
            disclosed_attributes: 4,
            embedded_copy_rows: 16_384,
            grammar_rows: u32::try_from(
                ZK_X509_RFC5280_GRAMMAR_RULE_COUNT_V1 + MAX_SOURCE_NODES_V1
                    - MAX_SOURCE_DOCUMENTS_V1,
            )
            .expect("grammar row bound fits u32"),
            fixed_byte_rows: 16_384,
            equality_rows: 0,
            decimal_rows: 1_024,
            calendar_rows: 72 * CALENDAR_COPY_PHASES_V1 as u32,
            relation_rows: 1_024,
            bit_flag_rows: 256,
            serial_source_rows: u32::try_from(MAX_SERIAL_SOURCE_ROWS_V1)
                .expect("serial source bound fits u32"),
            serial_rows: u32::try_from(MAX_SERIAL_COMPARISON_PHYSICAL_ROWS_V1)
                .expect("serial bound fits u32"),
            range_rows: 8_192,
            semantic_source_rows: 14_024,
            semantic_consumer_rows: 14_024,
            output_producer_rows: 22_705,
            output_consumer_rows: 22_705,
            io_channels: 1,
        }
    }

    fn refresh_private_shape_derived_rows_v1(shape: &mut ZkX509Rfc5280StarkPrivateShapeV1) {
        let top_count = usize::from(shape.top_document_count);
        let embedded_count = usize::from(shape.embedded_document_count);
        shape.grammar_rows = u32::try_from(
            shape
                .source_nodes()
                .expect("fixture source-node count")
                .checked_sub(top_count + embedded_count)
                .and_then(|rows| rows.checked_add(ZK_X509_RFC5280_GRAMMAR_RULE_COUNT_V1))
                .expect("fixture grammar rows"),
        )
        .expect("fixture grammar rows fit");
        shape.calendar_rows = u32::try_from(
            (2 * usize::from(shape.chain_depth) + 2 + usize::from(shape.crl_entries))
                * CALENDAR_COPY_PHASES_V1,
        )
        .expect("fixture calendar rows fit");
        let serial_rows = serial_comparison_rows_v1(usize::from(shape.crl_entries));
        shape.serial_source_rows =
            u32::try_from(serial_rows * 2).expect("fixture serial-source rows fit");
        shape.serial_rows = u32::try_from(serial_rows * SERIAL_COMPARISON_PHASES_V1)
            .expect("fixture serial rows fit");
    }

    #[test]
    fn exact_private_maximum_and_fixed_public_geometry_fit_log19() {
        let descriptor: [u8; 32] = Sha256::digest(ZK_X509_RFC5280_STARK_DESCRIPTOR_V1).into();
        assert_eq!(descriptor, ZK_X509_RFC5280_STARK_DESCRIPTOR_SHA256_V1);
        let shape = maximum_private_shape_v1();
        shape.validate().expect("exact conservative maximum");
        assert_eq!(MAX_OUTPUT_EVENT_ROWS_V1, 45_410);
        assert_eq!(MAX_ACTIVE_ROWS_V1, 238_481);
        assert_eq!(FIXED_NON_PADDING_ROWS_V1, 292_420);
        assert_eq!(
            shape.active_rows().expect("active rows"),
            MAX_ACTIVE_ROWS_V1
        );
        assert!(MAX_ACTIVE_ROWS_V1 < ZK_X509_RFC5280_STARK_TRACE_SIZE_V1);
        let schedule =
            compile_zk_x509_rfc5280_stark_fixed_schedule_v1(ZkX509Rfc5280StarkShapeV1::default())
                .expect("schedule");
        assert_eq!(
            schedule.counts.iter().sum::<usize>(),
            ZK_X509_RFC5280_STARK_TRACE_SIZE_V1
        );

        let mut overflow = shape;
        overflow.output_producer_rows = 22_706;
        assert_eq!(
            usize::try_from(overflow.output_producer_rows).expect("fits")
                + usize::try_from(overflow.output_consumer_rows).expect("fits"),
            45_411
        );
        assert!(overflow.validate().is_err());
    }

    #[test]
    fn certificate_extension_grammar_accepts_only_closed_profile_cardinalities() {
        let extension = ZkX509Rfc5280NodeProvenanceV1 {
            document: 0,
            node: 1,
            parent_node: 0,
            child_ordinal: 0,
            start: 0,
            content_start: 2,
            content_end: 4,
            depth: 1,
            tag_class: 0,
            constructed: true,
            tag_number: 16,
            role: ZkX509Rfc5280GrammarRoleV1::CertificateExtension,
            role_instance: 0,
        };
        let parent_role = ZkX509Rfc5280GrammarRoleV1::CertificateExtensions as u16;
        let four = grammar_rule_index_for_node_v1(extension, parent_role, 0, 4)
            .expect("CA certificate has four mandatory extensions");
        let five = grammar_rule_index_for_node_v1(extension, parent_role, 0, 5)
            .expect("leaf certificate adds the required EKU extension");
        assert_ne!(four, five);
        for unsupported_count in [0, 1, 2, 3, 6, u16::MAX] {
            assert_eq!(
                grammar_rule_index_for_node_v1(extension, parent_role, 0, unsupported_count),
                Err(ZkX509Rfc5280StarkErrorV1::Grammar),
                "extension count {unsupported_count} is outside the closed profile"
            );
        }
    }

    #[test]
    fn x5r1_terminal_claim_codec_and_final_row_replay_reject_every_malleation() {
        let claims = nontrivial_terminal_claims_v1();
        let encoded = claims.encode_x5r1_v1().expect("canonical X5R1 claims");
        assert_eq!(encoded.len(), 1_420);
        assert_eq!(&encoded[..4], b"X5R1");
        assert_eq!(
            ZkX509Rfc5280StarkTerminalClaimsV1::decode_x5r1_v1(&encoded),
            Ok(claims)
        );

        for length in 0..encoded.len() {
            assert_eq!(
                ZkX509Rfc5280StarkTerminalClaimsV1::decode_x5r1_v1(&encoded[..length]),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                "truncated X5R1 length {length} is rejected"
            );
        }
        let mut trailing = encoded.to_vec();
        trailing.push(0);
        assert_eq!(
            ZkX509Rfc5280StarkTerminalClaimsV1::decode_x5r1_v1(&trailing),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );
        for byte in 0..RFC5280_TERMINAL_CLAIM_HEADER_BYTES_V1 {
            let mut changed = encoded;
            changed[byte] ^= 1;
            assert_eq!(
                ZkX509Rfc5280StarkTerminalClaimsV1::decode_x5r1_v1(&changed),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                "header byte {byte} is canonical"
            );
        }

        let final_aux = terminal_aux_v1(claims);
        assert!(
            replay_zk_x509_rfc5280_terminal_claims_v1(F::ONE, &final_aux, &encoded)
                .expect("verifier terminal replay")
                .iter()
                .all(|residue| *residue == F::ZERO)
        );

        for claim_index in 0..RFC5280_TERMINAL_CLAIM_RECORDS_V1 {
            let start = RFC5280_TERMINAL_CLAIM_HEADER_BYTES_V1
                + claim_index * RFC5280_TERMINAL_CLAIM_RECORD_BYTES_V1;
            for field_byte in 0..8 {
                let field = ["family", "address", "lane", "endpoint"][field_byte / 2];
                let mut wrong_address = encoded;
                wrong_address[start + field_byte] ^= 0x40;
                assert_eq!(
                    ZkX509Rfc5280StarkTerminalClaimsV1::decode_x5r1_v1(&wrong_address),
                    Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                    "claim {claim_index} {field} cannot be omitted, reordered, or duplicated"
                );
            }
            let mut noncanonical = encoded;
            noncanonical[start + 8..start + RFC5280_TERMINAL_CLAIM_RECORD_BYTES_V1]
                .copy_from_slice(&GOLDILOCKS_MODULUS_V1.to_be_bytes());
            assert_eq!(
                ZkX509Rfc5280StarkTerminalClaimsV1::decode_x5r1_v1(&noncanonical),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                "claim {claim_index} rejects the modulus alias"
            );

            let mut changed_claim = encoded;
            let raw = u64::from_be_bytes(
                changed_claim[start + 8..start + RFC5280_TERMINAL_CLAIM_RECORD_BYTES_V1]
                    .try_into()
                    .expect("fixed claim record"),
            );
            changed_claim[start + 8..start + RFC5280_TERMINAL_CLAIM_RECORD_BYTES_V1]
                .copy_from_slice(&(raw + 1).to_be_bytes());
            match replay_zk_x509_rfc5280_terminal_claims_v1(F::ONE, &final_aux, &changed_claim) {
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim) => {}
                Ok(residues) => assert!(
                    residues.iter().any(|residue| *residue != F::ZERO),
                    "canonical mutation of claim {claim_index} remains AIR-bound"
                ),
                Err(error) => panic!("unexpected claim {claim_index} replay error: {error:?}"),
            }
        }

        for claim_index in 0..RFC5280_TERMINAL_CLAIM_RECORDS_V1 - 1 {
            let first = RFC5280_TERMINAL_CLAIM_HEADER_BYTES_V1
                + claim_index * RFC5280_TERMINAL_CLAIM_RECORD_BYTES_V1;
            let second = first + RFC5280_TERMINAL_CLAIM_RECORD_BYTES_V1;
            let mut reordered = encoded;
            for byte in 0..RFC5280_TERMINAL_CLAIM_RECORD_BYTES_V1 {
                reordered.swap(first + byte, second + byte);
            }
            assert_eq!(
                ZkX509Rfc5280StarkTerminalClaimsV1::decode_x5r1_v1(&reordered),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                "adjacent records {claim_index} and {} cannot be reordered",
                claim_index + 1
            );

            let mut duplicated = encoded;
            let first_record: [u8; RFC5280_TERMINAL_CLAIM_RECORD_BYTES_V1] = duplicated
                [first..first + RFC5280_TERMINAL_CLAIM_RECORD_BYTES_V1]
                .try_into()
                .expect("fixed X5R1 record");
            duplicated[second..second + RFC5280_TERMINAL_CLAIM_RECORD_BYTES_V1]
                .copy_from_slice(&first_record);
            assert_eq!(
                ZkX509Rfc5280StarkTerminalClaimsV1::decode_x5r1_v1(&duplicated),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                "record {claim_index} cannot replace record {}",
                claim_index + 1
            );
        }

        for claim_index in 0..RFC5280_TERMINAL_CLAIM_RECORDS_V1 {
            let mut changed_aux = final_aux;
            let auxiliary_column = if claim_index < RFC5280_AGGREGATE_TERMINAL_CLAIM_RECORDS_V1 {
                let relation_index = claim_index / ZK_X509_RFC5280_STARK_BUS_LANES_V1;
                let lane = claim_index % ZK_X509_RFC5280_STARK_BUS_LANES_V1;
                RFC5280_TERMINAL_CLAIM_RELATIONS_V1[relation_index].2 + lane
            } else {
                let local = claim_index - RFC5280_AGGREGATE_TERMINAL_CLAIM_RECORDS_V1;
                let role_index =
                    local / (OUTPUT_ENDPOINT_COUNT_V1 * ZK_X509_RFC5280_STARK_BUS_LANES_V1);
                let endpoint_lane =
                    local % (OUTPUT_ENDPOINT_COUNT_V1 * ZK_X509_RFC5280_STARK_BUS_LANES_V1);
                let consumer = endpoint_lane / ZK_X509_RFC5280_STARK_BUS_LANES_V1
                    == output_endpoint_index_v1(true);
                let lane = endpoint_lane % ZK_X509_RFC5280_STARK_BUS_LANES_V1;
                output_role_aux_column_v1(role_index, consumer, lane)
            };
            changed_aux[auxiliary_column] = changed_aux[auxiliary_column].add(F::ONE);
            assert_eq!(
                evaluate_zk_x509_rfc5280_terminal_claim_residues_v1(F::ONE, &changed_aux, claims,)
                    .expect("canonical claims")
                    .iter()
                    .filter(|residue| **residue != F::ZERO)
                    .count(),
                1,
                "claim {claim_index} has one independent final-row AIR binding"
            );
        }

        let mut nonidentity_reserved = claims;
        nonidentity_reserved.relations[2][0] = F(2);
        assert_eq!(
            nonidentity_reserved.encode_x5r1_v1(),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );
        assert_eq!(
            evaluate_zk_x509_rfc5280_terminal_claim_residues_v1(
                F::ONE,
                &final_aux,
                nonidentity_reserved,
            ),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );
        assert_eq!(
            ZkX509Rfc5280StarkTerminalClaimsV1::claim_address_v1(RFC5280_TERMINAL_CLAIM_RECORDS_V1,),
            None
        );
        assert_eq!(
            claims.claim_value_v1(RFC5280_TERMINAL_CLAIM_RECORDS_V1),
            None
        );
        let mut out_of_range = claims;
        assert_eq!(
            out_of_range.set_claim_value_v1(RFC5280_TERMINAL_CLAIM_RECORDS_V1, F::ZERO,),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );
    }

    #[test]
    fn rfc_role_air_bindings_reject_one_sided_coordinated_and_compensating_mutations() {
        let claims = nontrivial_terminal_claims_v1();
        let final_aux = terminal_aux_v1(claims);

        let mut one_sided = claims;
        one_sided.output_roles[0].producer_products[0] =
            one_sided.output_roles[0].producer_products[0].add(F::ONE);
        assert_eq!(
            one_sided.validate_v1(),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );

        let mut coordinated = claims;
        coordinated.output_roles[0].producer_products[0] =
            coordinated.output_roles[0].producer_products[0].add(F::ONE);
        recompute_output_aggregates_v1(&mut coordinated);
        coordinated
            .validate_v1()
            .expect("internally consistent decomposition");
        assert_eq!(
            evaluate_zk_x509_rfc5280_terminal_claim_residues_v1(F::ONE, &final_aux, coordinated,)
                .expect("canonical coordinated claims")
                .iter()
                .filter(|residue| **residue != F::ZERO)
                .count(),
            2,
            "the aggregate and independently committed role product both reject the mutation"
        );

        let multiplier = F(7);
        let inverse = multiplier.inv().expect("nonzero Goldilocks multiplier");
        let mut compensating = claims;
        compensating.output_roles[0].consumer_products[2] =
            compensating.output_roles[0].consumer_products[2].mul(multiplier);
        compensating.output_roles[1].consumer_products[2] =
            compensating.output_roles[1].consumer_products[2].mul(inverse);
        compensating
            .validate_v1()
            .expect("the compensating decomposition preserves its aggregate");
        assert_eq!(compensating.relations[5], claims.relations[5]);
        assert_eq!(
            evaluate_zk_x509_rfc5280_terminal_claim_residues_v1(F::ONE, &final_aux, compensating,)
                .expect("canonical compensating claims")
                .iter()
                .filter(|residue| **residue != F::ZERO)
                .count(),
            2,
            "two independently committed role products defeat inverse cancellation"
        );

        let mut wrong_role = claims;
        wrong_role.output_roles[0].role = ZkX509Rfc5280OutputRoleV1::CertificateSlotActive;
        assert_eq!(
            wrong_role.validate_v1(),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );

        let slot_index = output_role_index_v1(ZkX509Rfc5280OutputRoleV1::CertificateSlotActive);
        let mut optional_shape_substitution = claims;
        for consumer in [false, true] {
            let first = if consumer {
                optional_shape_substitution.output_roles[0].consumer_products
            } else {
                optional_shape_substitution.output_roles[0].producer_products
            };
            let slot = if consumer {
                optional_shape_substitution.output_roles[slot_index].consumer_products
            } else {
                optional_shape_substitution.output_roles[slot_index].producer_products
            };
            if consumer {
                optional_shape_substitution.output_roles[0].consumer_products = slot;
                optional_shape_substitution.output_roles[slot_index].consumer_products = first;
            } else {
                optional_shape_substitution.output_roles[0].producer_products = slot;
                optional_shape_substitution.output_roles[slot_index].producer_products = first;
            }
        }
        optional_shape_substitution
            .validate_v1()
            .expect("role-product swap preserves only aggregate decomposition");
        assert!(
            evaluate_zk_x509_rfc5280_terminal_claim_residues_v1(
                F::ONE,
                &final_aux,
                optional_shape_substitution,
            )
            .expect("canonical optional-shape substitution")
            .iter()
            .filter(|residue| **residue != F::ZERO)
            .count()
                >= 8,
            "CertificateSlotActive cannot be substituted for another fixed role"
        );
    }

    #[test]
    fn der_rfc_terminal_validator_rejects_every_cross_adapter_mismatch() {
        let claims = nontrivial_terminal_claims_v1();
        let der = ZkX509DerStarkTerminalClaimsV1 {
            input_byte: claims.der_input_byte_products_v1(),
            node: claims.der_node_products_v1(),
        };
        validate_zk_x509_der_rfc_terminal_equalities_v1(der, claims)
            .expect("canonical DER-to-RFC boundary");

        for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
            let mut wrong_der_byte = der;
            wrong_der_byte.input_byte[lane] = wrong_der_byte.input_byte[lane].add(F::ONE);
            assert_eq!(
                validate_zk_x509_der_rfc_terminal_equalities_v1(wrong_der_byte, claims),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
            );
            let mut wrong_der_node = der;
            wrong_der_node.node[lane] = wrong_der_node.node[lane].add(F::ONE);
            assert_eq!(
                validate_zk_x509_der_rfc_terminal_equalities_v1(wrong_der_node, claims),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
            );
            for relation in [0, 1] {
                let mut wrong_rfc = claims;
                wrong_rfc.relations[relation][lane] =
                    wrong_rfc.relations[relation][lane].add(F::ONE);
                assert_eq!(
                    validate_zk_x509_der_rfc_terminal_equalities_v1(der, wrong_rfc),
                    Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
                );
            }
        }

        let mut noncanonical_der = der;
        noncanonical_der.input_byte[0] = F(GOLDILOCKS_MODULUS_V1);
        assert_eq!(
            validate_zk_x509_der_rfc_terminal_equalities_v1(noncanonical_der, claims),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );

        let mut coordinated_der = der;
        let mut coordinated_rfc = claims;
        coordinated_der.node[1] = coordinated_der.node[1].add(F::ONE);
        coordinated_rfc.relations[1][1] = coordinated_rfc.relations[1][1].add(F::ONE);
        validate_zk_x509_der_rfc_terminal_equalities_v1(coordinated_der, coordinated_rfc)
            .expect("the pure equality sees the coordinated value");
        assert_eq!(
            evaluate_zk_x509_rfc5280_terminal_claim_residues_v1(
                F::ONE,
                &terminal_aux_v1(claims),
                coordinated_rfc,
            )
            .expect("canonical coordinated DER/RFC claim")
            .iter()
            .filter(|residue| **residue != F::ZERO)
            .count(),
            1,
            "the RFC side remains independently bound to its committed source rows"
        );
    }

    #[test]
    fn x5q1_sha_segment_terminal_frame_is_exact_typed_and_independently_replayed() {
        let claims = sha_segment_terminal_claims_v1();
        let encoded = claims.encode_x5q1_v1().expect("canonical X5Q1 claims");
        assert_eq!(encoded.len(), 4_876);
        assert_eq!(&encoded[..4], b"X5Q1");
        assert_eq!(
            ZkX509ShaSegmentTerminalClaimsV1::decode_x5q1_v1(&encoded),
            Ok(claims)
        );
        assert_eq!(
            ZkX509ShaSegmentTerminalClaimsV1::from_sha_air_terminals_v1(
                claims.segments,
                claims.ca_calls,
            ),
            Ok(claims)
        );
        assert!(
            replay_zk_x509_sha_segment_terminal_claims_v1(
                claims.segments,
                claims.ca_calls,
                &encoded,
            )
            .expect("verifier X5Q1 replay")
            .iter()
            .all(|residue| *residue == F::ZERO)
        );

        for length in 0..encoded.len() {
            assert_eq!(
                ZkX509ShaSegmentTerminalClaimsV1::decode_x5q1_v1(&encoded[..length]),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                "truncated length {length} is rejected"
            );
        }
        let mut trailing = encoded.to_vec();
        trailing.push(0);
        assert_eq!(
            ZkX509ShaSegmentTerminalClaimsV1::decode_x5q1_v1(&trailing),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );
        for byte in 0..SHA_TERMINAL_CLAIM_HEADER_BYTES_V1 {
            let mut changed = encoded;
            changed[byte] ^= 1;
            assert_eq!(
                ZkX509ShaSegmentTerminalClaimsV1::decode_x5q1_v1(&changed),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                "header byte {byte} is fixed"
            );
        }

        for claim_index in 0..SHA_TERMINAL_CLAIM_RECORDS_V1 {
            let start = SHA_TERMINAL_CLAIM_HEADER_BYTES_V1
                + claim_index * SHA_TERMINAL_CLAIM_RECORD_BYTES_V1;
            for field_byte in start..start + 8 {
                let mut changed = encoded;
                changed[field_byte] ^= 1;
                assert_eq!(
                    ZkX509ShaSegmentTerminalClaimsV1::decode_x5q1_v1(&changed),
                    Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                    "claim {claim_index} typed-address byte {field_byte} is fixed"
                );
            }

            let mut noncanonical = encoded;
            noncanonical[start + 8..start + SHA_TERMINAL_CLAIM_RECORD_BYTES_V1]
                .copy_from_slice(&GOLDILOCKS_MODULUS_V1.to_be_bytes());
            assert_eq!(
                ZkX509ShaSegmentTerminalClaimsV1::decode_x5q1_v1(&noncanonical),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                "claim {claim_index} rejects the modulus alias"
            );

            let mut false_claim = encoded;
            let raw = u64::from_be_bytes(
                false_claim[start + 8..start + SHA_TERMINAL_CLAIM_RECORD_BYTES_V1]
                    .try_into()
                    .expect("fixed SHA claim"),
            );
            false_claim[start + 8..start + SHA_TERMINAL_CLAIM_RECORD_BYTES_V1]
                .copy_from_slice(&(raw + 1).to_be_bytes());
            let residues = replay_zk_x509_sha_segment_terminal_claims_v1(
                claims.segments,
                claims.ca_calls,
                &false_claim,
            )
            .expect("canonical but false SHA claim parses");
            assert_eq!(
                residues
                    .iter()
                    .filter(|residue| **residue != F::ZERO)
                    .count(),
                1,
                "claim {claim_index} has one independent verifier equality"
            );
        }

        let first = SHA_TERMINAL_CLAIM_HEADER_BYTES_V1;
        let second = first + SHA_TERMINAL_CLAIM_RECORD_BYTES_V1;
        let mut reordered = encoded;
        let first_record = reordered[first..first + SHA_TERMINAL_CLAIM_RECORD_BYTES_V1].to_vec();
        let second_record = reordered[second..second + SHA_TERMINAL_CLAIM_RECORD_BYTES_V1].to_vec();
        reordered[first..first + SHA_TERMINAL_CLAIM_RECORD_BYTES_V1]
            .copy_from_slice(&second_record);
        reordered[second..second + SHA_TERMINAL_CLAIM_RECORD_BYTES_V1]
            .copy_from_slice(&first_record);
        assert_eq!(
            ZkX509ShaSegmentTerminalClaimsV1::decode_x5q1_v1(&reordered),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );
        let mut duplicated = encoded;
        duplicated[second..second + SHA_TERMINAL_CLAIM_RECORD_BYTES_V1]
            .copy_from_slice(&first_record);
        assert_eq!(
            ZkX509ShaSegmentTerminalClaimsV1::decode_x5q1_v1(&duplicated),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );

        for segment in 0..ZK_X509_SHA_SEGMENT_COUNT_V1 {
            let mut wrong_segment = claims;
            wrong_segment.segments[segment].segment ^= 1;
            assert_eq!(
                wrong_segment.encode_x5q1_v1(),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
            );
        }
        for claim_index in 0..SHA_TERMINAL_CLAIM_RECORDS_V1 {
            let mut noncanonical = claims;
            noncanonical
                .set_claim_value_v1(claim_index, F(GOLDILOCKS_MODULUS_V1))
                .expect("bounded claim address");
            assert_eq!(
                noncanonical.encode_x5q1_v1(),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                "claim {claim_index} is validated before encoding"
            );
        }

        let mut extremes = claims;
        for claim_index in 0..SHA_TERMINAL_CLAIM_RECORDS_V1 {
            extremes
                .set_claim_value_v1(
                    claim_index,
                    if claim_index % 2 == 0 {
                        F::ZERO
                    } else {
                        F(GOLDILOCKS_MODULUS_V1 - 1)
                    },
                )
                .expect("bounded claim address");
        }
        let extremes_encoded = extremes.encode_x5q1_v1().expect("canonical field extremes");
        assert_eq!(
            ZkX509ShaSegmentTerminalClaimsV1::decode_x5q1_v1(&extremes_encoded),
            Ok(extremes)
        );

        let mut malformed_committed = claims.segments;
        malformed_committed[0].segment = 1;
        assert_eq!(
            evaluate_zk_x509_sha_segment_terminal_claim_residues_v1(
                malformed_committed,
                claims.ca_calls,
                claims,
            ),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );
        let mut noncanonical_committed = claims.segments;
        noncanonical_committed[3].rfc_stream_products[3][3] = F(GOLDILOCKS_MODULUS_V1);
        assert_eq!(
            evaluate_zk_x509_sha_segment_terminal_claim_residues_v1(
                noncanonical_committed,
                claims.ca_calls,
                claims,
            ),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );
        assert_eq!(
            ZkX509ShaSegmentTerminalClaimsV1::claim_address_v1(SHA_TERMINAL_CLAIM_RECORDS_V1),
            None
        );
        assert_eq!(claims.claim_value_v1(SHA_TERMINAL_CLAIM_RECORDS_V1), None);
    }

    #[test]
    fn x5v1_p256_terminal_frame_is_exact_typed_and_independently_replayed() {
        let claims = p256_terminal_claims_v1();
        let encoded = claims.encode_x5v1_v1().expect("canonical X5V1 claims");
        assert_eq!(encoded.len(), 5_580);
        assert_eq!(&encoded[..4], b"X5V1");
        assert_eq!(
            ZkX509P256TerminalClaimsV1::claim_address_v1(0),
            Some((0, 0, 0, 0))
        );
        assert_eq!(
            ZkX509P256TerminalClaimsV1::claim_address_v1(31),
            Some((0, 7, 3, 0))
        );
        assert_eq!(
            ZkX509P256TerminalClaimsV1::claim_address_v1(32),
            Some((0, 8, 0, P256_TERMINAL_CROSS_START_V1))
        );
        assert_eq!(
            ZkX509P256TerminalClaimsV1::claim_address_v1(36),
            Some((0, 8, 0, P256_TERMINAL_CROSS_TERMINAL_V1))
        );
        assert_eq!(
            ZkX509P256TerminalClaimsV1::claim_address_v1(64),
            Some((0, P256_TERMINAL_SINK_FAMILY_V1, 0, 0))
        );
        assert_eq!(
            ZkX509P256TerminalClaimsV1::claim_address_v1(68),
            Some((1, 0, 0, 0))
        );
        assert_eq!(
            ZkX509P256TerminalClaimsV1::claim_address_v1(272),
            Some((4, 0, 0, 0))
        );
        assert_eq!(
            ZkX509P256TerminalClaimsV1::claim_address_v1(336),
            Some((4, 12, 0, P256_TERMINAL_CROSS_START_V1))
        );
        assert_eq!(
            ZkX509P256TerminalClaimsV1::claim_address_v1(344),
            Some((4, P256_TERMINAL_SINK_FAMILY_V1, 0, 0))
        );
        assert_eq!(
            ZkX509P256TerminalClaimsV1::claim_address_v1(347),
            Some((4, P256_TERMINAL_SINK_FAMILY_V1, 3, 0))
        );
        assert_eq!(
            ZkX509P256TerminalClaimsV1::decode_x5v1_v1(&encoded),
            Ok(claims)
        );
        assert_eq!(
            ZkX509P256TerminalClaimsV1::from_p256_air_terminals_v1(
                claims.certificate_or_crl,
                claims.wallet,
            ),
            Ok(claims)
        );
        assert!(
            replay_zk_x509_p256_terminal_claims_v1(claims, &encoded)
                .expect("verifier X5V1 replay")
                .iter()
                .all(|residue| *residue == F::ZERO)
        );

        for length in 0..encoded.len() {
            assert_eq!(
                ZkX509P256TerminalClaimsV1::decode_x5v1_v1(&encoded[..length]),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                "truncated length {length} is rejected"
            );
        }
        let mut trailing = encoded.to_vec();
        trailing.push(0);
        assert_eq!(
            ZkX509P256TerminalClaimsV1::decode_x5v1_v1(&trailing),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );
        for byte in 0..P256_TERMINAL_CLAIM_HEADER_BYTES_V1 {
            let mut changed = encoded;
            changed[byte] ^= 1;
            assert_eq!(
                ZkX509P256TerminalClaimsV1::decode_x5v1_v1(&changed),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                "header byte {byte} is fixed"
            );
        }

        for claim_index in 0..P256_TERMINAL_CLAIM_RECORDS_V1 {
            let start = P256_TERMINAL_CLAIM_HEADER_BYTES_V1
                + claim_index * P256_TERMINAL_CLAIM_RECORD_BYTES_V1;
            for field_byte in start..start + 8 {
                let mut changed = encoded;
                changed[field_byte] ^= 1;
                assert_eq!(
                    ZkX509P256TerminalClaimsV1::decode_x5v1_v1(&changed),
                    Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                    "claim {claim_index} typed-address byte {field_byte} is fixed"
                );
            }

            for alias in [GOLDILOCKS_MODULUS_V1, GOLDILOCKS_MODULUS_V1 + 1, u64::MAX] {
                let mut noncanonical = encoded;
                noncanonical[start + 8..start + P256_TERMINAL_CLAIM_RECORD_BYTES_V1]
                    .copy_from_slice(&alias.to_be_bytes());
                assert_eq!(
                    ZkX509P256TerminalClaimsV1::decode_x5v1_v1(&noncanonical),
                    Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                    "claim {claim_index} rejects noncanonical field alias {alias}"
                );
            }

            let mut false_claim = encoded;
            let raw = u64::from_be_bytes(
                false_claim[start + 8..start + P256_TERMINAL_CLAIM_RECORD_BYTES_V1]
                    .try_into()
                    .expect("fixed P-256 claim"),
            );
            false_claim[start + 8..start + P256_TERMINAL_CLAIM_RECORD_BYTES_V1]
                .copy_from_slice(&(raw + 1).to_be_bytes());
            let residues = replay_zk_x509_p256_terminal_claims_v1(claims, &false_claim)
                .expect("canonical but false P-256 claim parses");
            assert_eq!(
                residues
                    .iter()
                    .filter(|residue| **residue != F::ZERO)
                    .count(),
                1,
                "claim {claim_index} has one independent verifier equality"
            );
        }

        let first = P256_TERMINAL_CLAIM_HEADER_BYTES_V1;
        let second = first + P256_TERMINAL_CLAIM_RECORD_BYTES_V1;
        let mut reordered = encoded;
        let first_record = reordered[first..first + P256_TERMINAL_CLAIM_RECORD_BYTES_V1].to_vec();
        let second_record =
            reordered[second..second + P256_TERMINAL_CLAIM_RECORD_BYTES_V1].to_vec();
        reordered[first..first + P256_TERMINAL_CLAIM_RECORD_BYTES_V1]
            .copy_from_slice(&second_record);
        reordered[second..second + P256_TERMINAL_CLAIM_RECORD_BYTES_V1]
            .copy_from_slice(&first_record);
        assert_eq!(
            ZkX509P256TerminalClaimsV1::decode_x5v1_v1(&reordered),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );
        let mut duplicated = encoded;
        duplicated[second..second + P256_TERMINAL_CLAIM_RECORD_BYTES_V1]
            .copy_from_slice(&first_record);
        assert_eq!(
            ZkX509P256TerminalClaimsV1::decode_x5v1_v1(&duplicated),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );

        for signature in 0..P256_X5S1_CERTIFICATE_OR_CRL_SIGNATURES_V1 {
            for role in 0..P256_TERMINAL_CERTIFICATE_CROSS_SOURCES_V1 {
                let mut wrong_role = claims;
                wrong_role.certificate_or_crl[signature].cross_sources[role].role =
                    P256CrossTraceTerminalRoleV1::WalletLowS;
                assert_eq!(
                    wrong_role.encode_x5v1_v1(),
                    Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                    "certificate signature {signature} cross role {role} is verifier-fixed"
                );
            }
            let mut reordered_roles = claims;
            reordered_roles.certificate_or_crl[signature]
                .cross_sources
                .swap(0, 1);
            assert_eq!(
                reordered_roles.encode_x5v1_v1(),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
            );
        }
        for role in 0..P256_TERMINAL_WALLET_CROSS_SOURCES_V1 {
            let mut wrong_role = claims;
            wrong_role.wallet.cross_sources[role].role =
                if role == P256_TERMINAL_WALLET_CROSS_SOURCES_V1 - 1 {
                    P256CrossTraceTerminalRoleV1::ValueWriter
                } else {
                    P256CrossTraceTerminalRoleV1::WalletLowS
                };
            assert_eq!(
                wrong_role.encode_x5v1_v1(),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                "wallet cross role {role} is verifier-fixed"
            );
        }
        let mut reordered_roles = claims;
        reordered_roles.wallet.cross_sources.swap(0, 1);
        assert_eq!(
            reordered_roles.encode_x5v1_v1(),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );

        let mut swapped_signatures = claims;
        swapped_signatures.certificate_or_crl.swap(0, 1);
        let swapped_encoding = swapped_signatures
            .encode_x5v1_v1()
            .expect("same-role signatures retain canonical wire positions");
        assert!(
            replay_zk_x509_p256_terminal_claims_v1(claims, &swapped_encoding)
                .expect("swapped signatures parse at fixed positions")
                .iter()
                .any(|residue| *residue != F::ZERO)
        );

        for claim_index in 0..P256_TERMINAL_CLAIM_RECORDS_V1 {
            let mut noncanonical = claims;
            noncanonical
                .set_claim_value_v1(claim_index, F(GOLDILOCKS_MODULUS_V1))
                .expect("bounded P-256 claim address");
            assert_eq!(
                noncanonical.encode_x5v1_v1(),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                "claim {claim_index} is canonical-checked before encoding"
            );
        }

        let mut extremes = claims;
        for claim_index in 0..P256_TERMINAL_CLAIM_RECORDS_V1 {
            extremes
                .set_claim_value_v1(
                    claim_index,
                    if claim_index % 2 == 0 {
                        F::ZERO
                    } else {
                        F(GOLDILOCKS_MODULUS_V1 - 1)
                    },
                )
                .expect("bounded P-256 claim address");
        }
        let extremes_encoded = extremes
            .encode_x5v1_v1()
            .expect("canonical P-256 field extremes");
        assert_eq!(
            ZkX509P256TerminalClaimsV1::decode_x5v1_v1(&extremes_encoded),
            Ok(extremes)
        );
        assert_eq!(
            ZkX509P256TerminalClaimsV1::from_p256_air_terminals_v1(
                extremes.certificate_or_crl,
                extremes.wallet,
            ),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
            "canonical field encodings cannot bypass native AIR equalities"
        );

        for claim_index in 0..P256_TERMINAL_CLAIM_RECORDS_V1 {
            let raw = claims
                .claim_value_v1(claim_index)
                .expect("bounded P-256 claim")
                .0;
            let mut malformed_committed = claims;
            malformed_committed
                .set_claim_value_v1(claim_index, F(raw + 1))
                .expect("bounded P-256 committed claim");
            assert_eq!(
                evaluate_zk_x509_p256_terminal_claim_residues_v1(malformed_committed, claims,),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                "committed AIR terminal {claim_index} must satisfy its native equality"
            );

            let mut noncanonical_committed = claims;
            noncanonical_committed
                .set_claim_value_v1(claim_index, F(GOLDILOCKS_MODULUS_V1))
                .expect("bounded P-256 committed claim");
            assert_eq!(
                evaluate_zk_x509_p256_terminal_claim_residues_v1(noncanonical_committed, claims,),
                Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim),
                "committed AIR terminal {claim_index} must be canonical"
            );
        }

        for signature in 0..P256_X5S1_SIGNATURES_V1 {
            for pair in 0..4 {
                for lane in 0..P256_CROSS_TRACE_LANES_V1 {
                    let mut coordinated = claims;
                    let (buses, _, _) = coordinated
                        .signature_parts_mut_v1(signature)
                        .expect("bounded P-256 signature");
                    match pair {
                        0 => {
                            buses.value_execution[lane] = buses.value_execution[lane].add(F::ONE);
                            buses.value_sorted[lane] = buses.value_sorted[lane].add(F::ONE);
                        }
                        1 => {
                            buses.value_arithmetic_copy[lane] =
                                buses.value_arithmetic_copy[lane].add(F::ONE);
                            buses.arithmetic_value_copy[lane] =
                                buses.arithmetic_value_copy[lane].add(F::ONE);
                        }
                        2 => {
                            buses.arithmetic_scalar[lane] =
                                buses.arithmetic_scalar[lane].add(F::ONE);
                            buses.scalar_bus_arithmetic[lane] =
                                buses.scalar_bus_arithmetic[lane].add(F::ONE);
                        }
                        3 => {
                            buses.window_scalar[lane] = buses.window_scalar[lane].add(F::ONE);
                            buses.scalar_bus_window[lane] =
                                buses.scalar_bus_window[lane].add(F::ONE);
                        }
                        _ => unreachable!(),
                    }
                    assert_eq!(
                        ZkX509P256TerminalClaimsV1::from_p256_air_terminals_v1(
                            coordinated.certificate_or_crl,
                            coordinated.wallet,
                        ),
                        Ok(coordinated),
                        "coordinated bus forgery preserves only the internal equality"
                    );
                    let coordinated_encoding = coordinated
                        .encode_x5v1_v1()
                        .expect("self-consistent coordinated bus claims");
                    assert_eq!(
                        replay_zk_x509_p256_terminal_claims_v1(claims, &coordinated_encoding,)
                            .expect("coordinated bus claims parse")
                            .iter()
                            .filter(|residue| **residue != F::ZERO)
                            .count(),
                        2,
                        "signature {signature} bus pair {pair} lane {lane} remains AIR-bound"
                    );
                }
            }
        }

        for signature in 0..P256_X5S1_SIGNATURES_V1 {
            let cross_count = if signature < P256_X5S1_CERTIFICATE_OR_CRL_SIGNATURES_V1 {
                P256_TERMINAL_CERTIFICATE_CROSS_SOURCES_V1
            } else {
                P256_TERMINAL_WALLET_CROSS_SOURCES_V1
            };
            for source in 0..cross_count {
                for lane in 0..P256_CROSS_TRACE_LANES_V1 {
                    let mut coordinated = claims;
                    let (_, cross_sources, sink) = coordinated
                        .signature_parts_mut_v1(signature)
                        .expect("bounded P-256 signature");
                    cross_sources[source].terminal[lane] =
                        cross_sources[source].terminal[lane].add(F::ONE);
                    if source + 1 < cross_sources.len() {
                        cross_sources[source + 1].start[lane] =
                            cross_sources[source + 1].start[lane].add(F::ONE);
                    } else {
                        sink[lane] = sink[lane].add(F::ONE);
                    }
                    assert_eq!(
                        ZkX509P256TerminalClaimsV1::from_p256_air_terminals_v1(
                            coordinated.certificate_or_crl,
                            coordinated.wallet,
                        ),
                        Ok(coordinated),
                        "coordinated cross-source forgery preserves only the internal equality"
                    );
                    let coordinated_encoding = coordinated
                        .encode_x5v1_v1()
                        .expect("self-consistent coordinated cross claims");
                    assert_eq!(
                        replay_zk_x509_p256_terminal_claims_v1(claims, &coordinated_encoding,)
                            .expect("coordinated cross claims parse")
                            .iter()
                            .filter(|residue| **residue != F::ZERO)
                            .count(),
                        2,
                        "signature {signature} cross source {source} lane {lane} remains AIR-bound"
                    );
                }
            }
        }

        let mut wrong_committed_role = claims;
        wrong_committed_role.wallet.cross_sources[0].role =
            P256CrossTraceTerminalRoleV1::WalletLowS;
        assert_eq!(
            evaluate_zk_x509_p256_terminal_claim_residues_v1(wrong_committed_role, claims),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );
        assert_eq!(
            ZkX509P256TerminalClaimsV1::claim_address_v1(P256_TERMINAL_CLAIM_RECORDS_V1),
            None
        );
        assert_eq!(
            ZkX509P256TerminalClaimsV1::signature_role_v1(P256_X5S1_SIGNATURES_V1),
            None
        );
        assert_eq!(claims.signature_parts_v1(P256_X5S1_SIGNATURES_V1), None);
        assert_eq!(claims.claim_value_v1(P256_TERMINAL_CLAIM_RECORDS_V1), None);
        let mut out_of_range = claims;
        assert_eq!(
            out_of_range.set_claim_value_v1(P256_TERMINAL_CLAIM_RECORDS_V1, F::ZERO),
            Err(ZkX509Rfc5280StarkErrorV1::TerminalClaim)
        );
    }

    #[test]
    fn every_auxiliary_column_has_one_canonical_replay_descriptor() {
        for column in 0..ZK_X509_RFC5280_STARK_AUX_WIDTH_V1 {
            let descriptors = [
                product_aux_column_descriptor_v1(column).is_some(),
                output_role_aux_column_descriptor_v1(column).is_some(),
                serial_product_aux_column_descriptor_v1(column).is_some(),
                grammar_ordinal_product_aux_column_descriptor_v1(column).is_some(),
                lookup_aux_column_descriptor_v1(column).is_some(),
                profile_lookup_aux_column_descriptor_v1(column).is_some(),
                grammar_lookup_aux_column_descriptor_v1(column).is_some(),
            ];
            assert_eq!(
                descriptors.into_iter().filter(|matched| *matched).count(),
                1,
                "auxiliary column {column} must have one replay equation"
            );
        }
        for column in [
            ZK_X509_RFC5280_STARK_AUX_WIDTH_V1,
            ZK_X509_RFC5280_STARK_AUX_WIDTH_V1 + 1,
            usize::MAX,
        ] {
            assert!(product_aux_column_descriptor_v1(column).is_none());
            assert!(output_role_aux_column_descriptor_v1(column).is_none());
            assert!(serial_product_aux_column_descriptor_v1(column).is_none());
            assert!(grammar_ordinal_product_aux_column_descriptor_v1(column).is_none());
            assert!(lookup_aux_column_descriptor_v1(column).is_none());
            assert!(profile_lookup_aux_column_descriptor_v1(column).is_none());
            assert!(grammar_lookup_aux_column_descriptor_v1(column).is_none());
        }
    }

    #[test]
    fn canonical_column_provider_replays_base_aux_and_proof_claims() {
        let trace = canonical_trace_v1();
        let material =
            build_zk_x509_rfc5280_stark_base_material_v1(&trace).expect("canonical base material");
        let der_challenges = der_challenges_v1();
        let challenges = challenges_v1();
        let provider =
            ZkX509Rfc5280StarkColumnProviderV1::new_v1(&material, der_challenges, challenges)
                .expect("canonical RFC column provider");

        let base_active = provider
            .build_base_column_v1(BASE_ACTIVE)
            .expect("base active column");
        let fixed_first = provider
            .build_fixed_column_v1(FIX_GLOBAL_FIRST)
            .expect("fixed first column");
        assert_eq!(base_active.len(), ZK_X509_RFC5280_STARK_TRACE_SIZE_V1);
        assert_eq!(fixed_first.len(), ZK_X509_RFC5280_STARK_TRACE_SIZE_V1);
        for row in [
            0,
            material.schedule.starts[ZkX509Rfc5280StarkFamilyV1::SourceNode as usize],
            material.schedule.starts[ZkX509Rfc5280StarkFamilyV1::Grammar as usize],
            material.schedule.starts[ZkX509Rfc5280StarkFamilyV1::Padding as usize],
            ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 - 1,
        ] {
            assert_eq!(
                base_active[row],
                provider.base_row_v1(row).expect("base row")[BASE_ACTIVE]
            );
            assert_eq!(
                fixed_first[row],
                provider.fixed_row_v1(row).expect("fixed row")[FIX_GLOBAL_FIRST]
            );
        }
        assert_eq!(fixed_first[0], F::ONE);
        assert!(fixed_first[1..].iter().all(|value| *value == F::ZERO));
        assert_eq!(
            provider.build_base_column_v1(ZK_X509_RFC5280_STARK_BASE_WIDTH_V1),
            Err(ZkX509Rfc5280StarkErrorV1::Shape)
        );
        assert_eq!(
            provider.build_fixed_column_v1(ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1),
            Err(ZkX509Rfc5280StarkErrorV1::Shape)
        );
        assert_eq!(
            provider.build_aux_column_v1(ZK_X509_RFC5280_STARK_AUX_WIDTH_V1),
            Err(ZkX509Rfc5280StarkErrorV1::Shape)
        );

        let claims = provider.terminal_claims_v1();
        let current = provider.base_row_v1(0).expect("first base row");
        let next = provider.base_row_v1(1).expect("next base row");
        let fixed = provider.fixed_row_v1(0).expect("first fixed row");
        let current_aux = neutral_aux_v1();
        let next_aux = neutral_aux_v1();
        let canonical_residues = evaluate_zk_x509_rfc5280_stark_residues_v1(
            &current,
            &next,
            &current_aux,
            &next_aux,
            &fixed,
            der_challenges,
            challenges,
            claims,
        )
        .expect("canonical opened-row evaluation");
        assert_eq!(canonical_residues[8], F::ZERO);
        assert_eq!(canonical_residues[25], F::ZERO);
        let mut changed_value = current;
        changed_value[BASE_VALUE] = changed_value[BASE_VALUE].add(F::ONE);
        assert_ne!(
            evaluate_zk_x509_rfc5280_stark_residues_v1(
                &changed_value,
                &next,
                &current_aux,
                &next_aux,
                &fixed,
                der_challenges,
                challenges,
                claims,
            )
            .expect("mutated opened-row evaluation")[8],
            F::ZERO,
            "the provider byte value is constrained to its committed bits"
        );
        let mut changed_helper = current;
        changed_helper[BASE_COPY_VALUE] = changed_helper[BASE_COPY_VALUE].add(F::ONE);
        assert_ne!(
            evaluate_zk_x509_rfc5280_stark_residues_v1(
                &changed_helper,
                &next,
                &current_aux,
                &next_aux,
                &fixed,
                der_challenges,
                challenges,
                claims,
            )
            .expect("mutated helper evaluation")[25],
            F::ZERO,
            "the highest provider base column is algebraically replayed"
        );

        let encoded = provider
            .encoded_terminal_claims_v1()
            .expect("proof-carried X5R1 claims");
        assert_eq!(
            ZkX509Rfc5280StarkTerminalClaimsV1::decode_x5r1_v1(&encoded),
            Ok(claims)
        );
        let der_source = zk_x509_rfc5280_der_source_terminals_v1(&trace, der_challenges)
            .expect("independent DER source terminals");
        assert_eq!(claims.relations[0], der_source.input_byte);
        assert_eq!(claims.relations[1], der_source.node);

        let output = zk_x509_rfc5280_output_terminals_v1(&trace, challenges)
            .expect("independent output terminals");
        let mut expected_producer = [F::ONE; ZK_X509_RFC5280_STARK_BUS_LANES_V1];
        let mut expected_consumer = [F::ONE; ZK_X509_RFC5280_STARK_BUS_LANES_V1];
        for role in 0..OUTPUT_ROLE_COUNT_V1 {
            assert_eq!(claims.output_roles[role].role, OUTPUT_ROLES_V1[role]);
            assert_eq!(
                claims.output_roles[role].producer_products,
                output.producer[role]
            );
            assert_eq!(
                claims.output_roles[role].consumer_products,
                output.consumer[role]
            );
            for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
                expected_producer[lane] = expected_producer[lane].mul(output.producer[role][lane]);
                expected_consumer[lane] = expected_consumer[lane].mul(output.consumer[role][lane]);
            }
        }
        assert_eq!(claims.relations[4], expected_producer);
        assert_eq!(claims.relations[5], expected_consumer);
        assert_eq!(
            claims.governed_trust_anchor_products_v1().consumer_products,
            output.consumer[output_role_index_v1(ZkX509Rfc5280OutputRoleV1::GovernedTrustAnchor)]
        );
        assert_eq!(
            claims.certificate_slot_active_products_v1(),
            claims.output_roles
                [output_role_index_v1(ZkX509Rfc5280OutputRoleV1::CertificateSlotActive)]
        );
        assert_eq!(
            claims.relations[2..4],
            [[F::ONE; ZK_X509_RFC5280_STARK_BUS_LANES_V1]; 2]
        );

        let root_role_index = output_role_index_v1(ZkX509Rfc5280OutputRoleV1::GovernedTrustAnchor);
        let root_selector = output_role_fixed_selector_column_v1(root_role_index, true);
        let root_row = (0..ZK_X509_RFC5280_STARK_TRACE_SIZE_V1 - 1)
            .find(|row| {
                provider.fixed_row_v1(*row).expect("canonical fixed row")[root_selector] == F::ONE
            })
            .expect("governed root-SPKI consumer row");
        let root_aux_column = output_role_aux_column_v1(root_role_index, true, 0);
        let root_column = provider
            .build_aux_column_v1(root_aux_column)
            .expect("root-SPKI prefix product");
        let root_current = provider
            .base_row_v1(root_row)
            .expect("root-SPKI current base row");
        let root_next = provider
            .base_row_v1(root_row + 1)
            .expect("root-SPKI next base row");
        let root_fixed = provider
            .fixed_row_v1(root_row)
            .expect("root-SPKI fixed row");
        let mut root_current_aux = neutral_aux_v1();
        let mut root_next_aux = neutral_aux_v1();
        root_current_aux[root_aux_column] = root_column[root_row];
        root_next_aux[root_aux_column] = root_column[root_row + 1];
        let role_residue_offset: usize = RFC5280_RESIDUE_SECTIONS_V1[..14]
            .iter()
            .map(|(_, count)| count)
            .sum::<usize>()
            + (root_role_index * OUTPUT_ENDPOINT_COUNT_V1 + output_endpoint_index_v1(true))
                * ZK_X509_RFC5280_STARK_BUS_LANES_V1
                * 2;
        let root_residues = evaluate_zk_x509_rfc5280_stark_residues_v1(
            &root_current,
            &root_next,
            &root_current_aux,
            &root_next_aux,
            &root_fixed,
            der_challenges,
            challenges,
            claims,
        )
        .expect("canonical per-role root-SPKI recurrence");
        assert_eq!(root_residues[role_residue_offset], F::ZERO);

        let mut wrong_root_next_aux = root_next_aux;
        wrong_root_next_aux[root_aux_column] = wrong_root_next_aux[root_aux_column].add(F::ONE);
        assert_ne!(
            evaluate_zk_x509_rfc5280_stark_residues_v1(
                &root_current,
                &root_next,
                &root_current_aux,
                &wrong_root_next_aux,
                &root_fixed,
                der_challenges,
                challenges,
                claims,
            )
            .expect("mutated per-role root-SPKI recurrence")[role_residue_offset],
            F::ZERO,
            "the governed root-SPKI product cannot skip or alter its selected factor"
        );

        let mut wrong_root_fixed = root_fixed;
        wrong_root_fixed[root_selector] = F::ZERO;
        assert_ne!(
            evaluate_zk_x509_rfc5280_stark_residues_v1(
                &root_current,
                &root_next,
                &root_current_aux,
                &root_next_aux,
                &wrong_root_fixed,
                der_challenges,
                challenges,
                claims,
            )
            .expect("mutated verifier-fixed root selector")[role_residue_offset],
            F::ZERO,
            "the role product is gated only by the authenticated verifier selector"
        );

        let mut final_aux = [F::ZERO; ZK_X509_RFC5280_STARK_AUX_WIDTH_V1];
        for (_, relation, after) in RFC5280_TERMINAL_CLAIM_RELATIONS_V1 {
            for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
                let column = provider
                    .build_aux_column_v1(after + lane)
                    .expect("terminal auxiliary column");
                assert_eq!(column.len(), ZK_X509_RFC5280_STARK_TRACE_SIZE_V1);
                final_aux[after + lane] = *column.last().expect("nonempty native column");
                assert_eq!(final_aux[after + lane], claims.relations[relation][lane]);
            }
        }
        for role_index in 0..OUTPUT_ROLE_COUNT_V1 {
            for consumer in [false, true] {
                let expected = if consumer {
                    claims.output_roles[role_index].consumer_products
                } else {
                    claims.output_roles[role_index].producer_products
                };
                for (lane, expected) in expected.into_iter().enumerate() {
                    let auxiliary_column = output_role_aux_column_v1(role_index, consumer, lane);
                    let column = provider
                        .build_aux_column_v1(auxiliary_column)
                        .expect("output-role terminal auxiliary column");
                    final_aux[auxiliary_column] = *column.last().expect("nonempty native column");
                    assert_eq!(final_aux[auxiliary_column], expected);
                }
            }
        }
        assert!(
            replay_zk_x509_rfc5280_terminal_claims_v1(F::ONE, &final_aux, &encoded)
                .expect("verifier replay")
                .iter()
                .all(|residue| *residue == F::ZERO)
        );
    }

    #[test]
    fn private_geometry_mutations_do_not_change_public_schedule_digest() {
        let public = ZkX509Rfc5280StarkShapeV1::default();
        let digest = public.schedule_digest().expect("digest");
        let shape = maximum_private_shape_v1();
        let mut mutations = Vec::new();
        let mut changed = shape.clone();
        changed.crl_entries -= 1;
        refresh_private_shape_derived_rows_v1(&mut changed);
        mutations.push(changed);
        changed = shape.clone();
        changed.disclosed_attributes -= 1;
        refresh_private_shape_derived_rows_v1(&mut changed);
        mutations.push(changed);
        changed = shape.clone();
        changed.top_node_counts[0] -= 1;
        refresh_private_shape_derived_rows_v1(&mut changed);
        mutations.push(changed);
        changed = shape.clone();
        changed.io_channels += 1;
        refresh_private_shape_derived_rows_v1(&mut changed);
        mutations.push(changed);
        for mutation in mutations {
            mutation.validate().expect("valid private mutation");
            assert_eq!(
                ZkX509Rfc5280StarkShapeV1::default()
                    .schedule_digest()
                    .expect("constant public digest"),
                digest,
                "private counts and lengths never enter the public transcript"
            );
        }
    }

    #[test]
    fn output_grand_product_is_total_at_a_forced_zero_factor() {
        let endpoint = ZkX509IoEndpointV1 {
            role: ZkX509IoSegmentRoleV1::StrictDer,
            instance: 0,
        };
        let mut challenges = challenges_v1();
        let values = [
            F(80),
            F(ZkX509Rfc5280OutputRoleV1::Projection as u64),
            F(7),
            F(endpoint_role_code_v1(endpoint.role).expect("endpoint role")),
            F(u64::from(endpoint.instance)),
            F(2),
            F(3),
            F::ONE,
            F::ZERO,
            F::ZERO,
            F::ZERO,
            F::ZERO,
        ];
        let non_domain_sum = values[1..]
            .iter()
            .zip(&challenges.tuple[0][1..])
            .fold(F::ZERO, |sum, (value, coefficient)| {
                sum.add(value.mul(*coefficient))
            });
        challenges.tuple[0][0] =
            F::ZERO.sub(non_domain_sum.mul(F(80).inv().expect("nonzero output domain separator")));
        challenges.validate().expect("forced-zero challenges");
        assert_eq!(
            output_factor_v1(
                ZkX509Rfc5280OutputRoleV1::Projection,
                7,
                endpoint,
                2,
                3,
                true,
                0,
                challenges,
            )
            .expect("zero product factor is not a completeness error"),
            F::ZERO
        );
    }

    #[test]
    fn private_depth_selector_and_optional_slot_dummy_are_canonical() {
        let mut depth_two = maximum_private_shape_v1();
        depth_two.chain_depth = 2;
        depth_two.certificate_slot_2_active = F::ZERO;
        depth_two.top_document_count = 3;
        depth_two.top_document_lengths[3] = 0;
        depth_two.top_node_counts[3] = 0;
        // Embedded documents are byte ranges copied from the active
        // top-level documents. Removing the fourth 4 KiB document therefore
        // also requires a physically possible embedded-byte geometry instead
        // of retaining the depth-three 16 KiB maximum.
        depth_two.embedded_document_count = 11;
        depth_two.embedded_document_lengths[11..].fill(0);
        depth_two.embedded_node_counts[11..].fill(0);
        depth_two.embedded_copy_rows = depth_two.embedded_document_lengths[..11]
            .iter()
            .map(|length| u32::from(*length))
            .sum();
        refresh_private_shape_derived_rows_v1(&mut depth_two);
        depth_two.validate().expect("canonical depth-two geometry");

        let mut flipped = depth_two.clone();
        flipped.certificate_slot_2_active = F::ONE;
        assert_eq!(
            flipped.validate(),
            Err(ZkX509Rfc5280StarkErrorV1::Shape),
            "the private selector is exactly depth == 3"
        );
        let mut noncanonical_dummy_bytes = depth_two.clone();
        noncanonical_dummy_bytes.top_document_lengths[3] = 1;
        assert_eq!(
            noncanonical_dummy_bytes.validate(),
            Err(ZkX509Rfc5280StarkErrorV1::Shape)
        );
        let mut noncanonical_dummy_nodes = depth_two.clone();
        noncanonical_dummy_nodes.top_node_counts[3] = 1;
        assert_eq!(
            noncanonical_dummy_nodes.validate(),
            Err(ZkX509Rfc5280StarkErrorV1::Shape)
        );
        let mut noncanonical_calendar_phase_count = depth_two.clone();
        noncanonical_calendar_phase_count.calendar_rows += 1;
        assert_eq!(
            noncanonical_calendar_phase_count.validate(),
            Err(ZkX509Rfc5280StarkErrorV1::Shape),
            "calendar rows are exactly six physical phases per semantic timestamp"
        );

        let schedule =
            compile_zk_x509_rfc5280_stark_fixed_schedule_v1(ZkX509Rfc5280StarkShapeV1::default())
                .expect("fixed schedule");
        let cert2_first = schedule.starts[ZkX509Rfc5280StarkFamilyV1::SourceByte as usize]
            + 3 * ZK_X509_RFC5280_MAX_TOP_LEVEL_DOCUMENT_BYTES_V1;
        let fixed = schedule
            .fixed_row(cert2_first)
            .expect("optional-slot fixed row");

        let depth_two_row = [F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1];
        assert!(
            private_geometry_residues_v1(&depth_two_row, &depth_two_row, &fixed)
                .iter()
                .all(|residue| *residue == F::ZERO),
            "depth two uses the unique inactive all-zero slot"
        );
        let mut depth_two_active = depth_two_row;
        depth_two_active[BASE_ACTIVE] = F::ONE;
        assert!(
            private_geometry_residues_v1(&depth_two_active, &depth_two_active, &fixed)
                .iter()
                .any(|residue| *residue != F::ZERO),
            "depth two cannot activate certificate slot two"
        );
        let mut nonzero_dummy = depth_two_row;
        nonzero_dummy[BASE_VALUE] = F::ONE;
        assert!(
            private_geometry_residues_v1(&nonzero_dummy, &depth_two_row, &fixed)
                .iter()
                .any(|residue| *residue != F::ZERO),
            "inactive dummy byte rows are all-zero outside carried selectors"
        );

        let mut depth_three_row = depth_two_row;
        depth_three_row[BASE_ACTIVE] = F::ONE;
        depth_three_row[BASE_CERT2_ACTIVE] = F::ONE;
        assert!(
            private_geometry_residues_v1(&depth_three_row, &depth_three_row, &fixed)
                .iter()
                .all(|residue| *residue == F::ZERO),
            "depth three activates the optional slot"
        );
        let mut depth_three_inactive = depth_two_row;
        depth_three_inactive[BASE_CERT2_ACTIVE] = F::ONE;
        assert!(
            private_geometry_residues_v1(&depth_three_inactive, &depth_three_inactive, &fixed)
                .iter()
                .any(|residue| *residue != F::ZERO),
            "depth three cannot suppress certificate slot two"
        );
    }

    #[test]
    fn four_lanes_are_distinct_and_global_copy_bound_is_at_least_171_bits() {
        let challenges = challenges_v1();
        challenges.validate().expect("four independent lanes");
        let mut duplicate = challenges;
        duplicate.tuple[3][11] = duplicate.tuple[0][0];
        assert_eq!(
            duplicate.validate(),
            Err(ZkX509Rfc5280StarkErrorV1::Challenge)
        );
        assert_eq!(ZK_X509_RFC5280_STARK_BUS_LANES_V1, 4);
        assert_eq!(ZK_X509_RFC5280_STARK_RELATION_EVENT_BOUND_V1, 1 << 19);
        assert_eq!(ZK_X509_RFC5280_STARK_COMPRESSED_RELATIONS_V1, 29);
        assert_eq!(ZK_X509_RFC5280_STARK_COPY_SOUNDNESS_BITS_V1, 171);
    }

    #[test]
    fn evaluator_inventory_is_exact_and_padding_mutation_is_detected() {
        let current = [F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1];
        let next = current;
        let current_aux = neutral_aux_v1();
        let next_aux = current_aux;
        let mut fixed = [F::ZERO; ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1];
        fixed[ZkX509Rfc5280StarkFamilyV1::Padding as usize] = F::ONE;
        let claims = terminal_claims_v1();
        let residues = evaluate_zk_x509_rfc5280_stark_residues_v1(
            &current,
            &next,
            &current_aux,
            &next_aux,
            &fixed,
            der_challenges_v1(),
            challenges_v1(),
            claims,
        )
        .expect("total evaluator");
        assert_eq!(residues.len(), ZK_X509_RFC5280_STARK_CONSTRAINT_COUNT_V1);
        assert!(residues.iter().all(|residue| *residue == F::ZERO));

        let mut changed = current;
        changed[BASE_A] = F::ONE;
        let residues = evaluate_zk_x509_rfc5280_stark_residues_v1(
            &changed,
            &next,
            &current_aux,
            &next_aux,
            &fixed,
            der_challenges_v1(),
            challenges_v1(),
            claims,
        )
        .expect("mutated evaluator");
        assert!(residues.iter().any(|residue| *residue != F::ZERO));

        let mut changed_high_column = current;
        changed_high_column[SERIAL_SLACK_BITS + 7] = F::ONE;
        let residues = evaluate_zk_x509_rfc5280_stark_residues_v1(
            &changed_high_column,
            &next,
            &current_aux,
            &next_aux,
            &fixed,
            der_challenges_v1(),
            challenges_v1(),
            claims,
        )
        .expect("high-column padding mutation evaluator");
        assert!(
            residues.iter().any(|residue| *residue != F::ZERO),
            "inactive serial bit columns must be zero on padding rows"
        );
        assert!(ZK_X509_RFC5280_STARK_CONSTRAINT_COUNT_V1 <= usize::from(u16::MAX));
    }

    fn affine_value_v1(seed: u64, domain: u64, column: usize, point: u64) -> F {
        let column = u64::try_from(column).expect("test column fits u64");
        let intercept = F(seed
            .checked_mul(1_009)
            .and_then(|value| value.checked_add(domain * 131))
            .and_then(|value| value.checked_add(column * 17))
            .and_then(|value| value.checked_add(1))
            .expect("small affine intercept"));
        let slope = F(seed
            .checked_mul(313)
            .and_then(|value| value.checked_add(domain * 29))
            .and_then(|value| value.checked_add(column * 43))
            .and_then(|value| value.checked_add(7))
            .expect("small nonzero affine slope"));
        intercept.add(slope.mul(F(point)))
    }

    fn finite_difference_degree_v1(mut samples: Vec<F>) -> usize {
        let mut maximum_nonzero_order = 0;
        for order in 0..samples.len() {
            if samples.iter().any(|value| *value != F::ZERO) {
                maximum_nonzero_order = order;
            }
            if samples.len() == 1 {
                break;
            }
            samples = samples
                .windows(2)
                .map(|window| window[1].sub(window[0]))
                .collect();
        }
        maximum_nonzero_order
    }

    #[test]
    fn full_opened_input_affine_degree_inventory_is_exactly_four() {
        const SAMPLE_POINTS: u64 = 9;
        let mut maximum_degrees = vec![0_usize; ZK_X509_RFC5280_STARK_CONSTRAINT_COUNT_V1];
        for seed in [3_u64, 5, 11] {
            let mut samples = vec![
                Vec::with_capacity(SAMPLE_POINTS as usize);
                ZK_X509_RFC5280_STARK_CONSTRAINT_COUNT_V1
            ];
            for point in 0..SAMPLE_POINTS {
                let current =
                    core::array::from_fn(|column| affine_value_v1(seed, 1, column, point));
                let next = core::array::from_fn(|column| affine_value_v1(seed, 2, column, point));
                let current_aux =
                    core::array::from_fn(|column| affine_value_v1(seed, 3, column, point));
                let next_aux =
                    core::array::from_fn(|column| affine_value_v1(seed, 4, column, point));
                let fixed = core::array::from_fn(|column| affine_value_v1(seed, 5, column, point));
                let residues = evaluate_zk_x509_rfc5280_stark_residues_v1(
                    &current,
                    &next,
                    &current_aux,
                    &next_aux,
                    &fixed,
                    der_challenges_v1(),
                    challenges_v1(),
                    terminal_claims_v1(),
                )
                .expect("affine degree evaluator is total");
                for (residue_samples, residue) in samples.iter_mut().zip(residues) {
                    residue_samples.push(residue);
                }
            }
            for (maximum, residue_samples) in maximum_degrees.iter_mut().zip(samples) {
                *maximum = (*maximum).max(finite_difference_degree_v1(residue_samples));
            }
        }

        let offenders = maximum_degrees
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, degree)| *degree > usize::from(ZK_X509_RFC5280_STARK_CONSTRAINT_DEGREE_V1))
            .collect::<Vec<_>>();
        assert!(offenders.is_empty(), "degree-four offenders: {offenders:?}");
        assert!(
            maximum_degrees
                .iter()
                .any(|degree| *degree == usize::from(ZK_X509_RFC5280_STARK_CONSTRAINT_DEGREE_V1)),
            "the registered degree must be attained, not only upper-bounded"
        );
        let inventory = core::array::from_fn::<_, 5, _>(|degree| {
            maximum_degrees
                .iter()
                .filter(|actual| **actual == degree)
                .count()
        });
        const EXPECTED_AFFINE_DEGREE_INVENTORY_V1: [usize; 5] = [0, 1, 617, 217, 392];
        assert_eq!(
            inventory, EXPECTED_AFFINE_DEGREE_INVENTORY_V1,
            "the full-input interpolation is a proof-shape pin, independent of evaluator sections"
        );
        assert_eq!(inventory.iter().sum::<usize>(), maximum_degrees.len());
    }

    #[test]
    fn every_committed_degree_normalization_helper_is_constrained() {
        let current = [F::ZERO; ZK_X509_RFC5280_STARK_BASE_WIDTH_V1];
        let next = current;
        let current_aux = neutral_aux_v1();
        let next_aux = current_aux;
        let mut fixed = [F::ZERO; ZK_X509_RFC5280_STARK_FIXED_WIDTH_V1];
        fixed[ZkX509Rfc5280StarkFamilyV1::Padding as usize] = F::ONE;
        let direct_helpers = [
            BASE_GRAMMAR_ORDINAL,
            BASE_EXPECTED_ROOT_KIND,
            BASE_PROFILE_TABLE_ACTIVE,
            BASE_PROFILE_TABLE_MULTIPLICITY,
            BASE_PROFILE_TOPOLOGY_QUERY_ACTIVE,
            BASE_SERIAL_BYTE_QUERY_ACTIVE,
            BASE_SERIAL_BYTE_QUERY_VALUE,
            BASE_COPY_SOURCE_ACTIVE,
            BASE_COPY_CONSUMER_ACTIVE,
            BASE_COPY_DOMAIN,
            BASE_COPY_KEY_1,
            BASE_COPY_KEY_2,
            BASE_COPY_VALUE,
        ];
        // Thirteen elementary constraints precede this exact helper block:
        // eight byte-bit booleans, byte packing, and four global booleans.
        const HELPER_RESIDUE_START: usize = 13;
        for (helper_index, column) in direct_helpers.into_iter().enumerate() {
            let mut mutated = current;
            mutated[column] = F::ONE;
            let residues = evaluate_zk_x509_rfc5280_stark_residues_v1(
                &mutated,
                &next,
                &current_aux,
                &next_aux,
                &fixed,
                der_challenges_v1(),
                challenges_v1(),
                terminal_claims_v1(),
            )
            .expect("helper mutation evaluator");
            assert_ne!(
                residues[HELPER_RESIDUE_START + helper_index],
                F::ZERO,
                "helper column {column} must have its own defining constraint"
            );
        }
        for column in [BASE_ORDINAL_NEXT_ACTIVE, BASE_ORDINAL_EQUAL_CONTINUE] {
            let mut mutated = current;
            mutated[column] = F::ONE;
            let residues = evaluate_zk_x509_rfc5280_stark_residues_v1(
                &mutated,
                &next,
                &current_aux,
                &next_aux,
                &fixed,
                der_challenges_v1(),
                challenges_v1(),
                terminal_claims_v1(),
            )
            .expect("ordinal helper mutation evaluator");
            assert!(
                residues.iter().any(|residue| *residue != F::ZERO),
                "ordinal helper column {column} must be constrained"
            );
        }
    }

    #[test]
    fn physical_copy_phase_mutations_are_rejected() {
        let schedule =
            compile_zk_x509_rfc5280_stark_fixed_schedule_v1(ZkX509Rfc5280StarkShapeV1::default())
                .expect("fixed schedule");

        let comparison = canonical_serial_comparisons_v1(&[7], &[vec![8]])
            .expect("canonical comparison")
            .remove(0);
        let serial_row =
            build_zk_x509_rfc5280_serial_comparison_rows_v1(&comparison).expect("serial rows")[0];
        let serial_start = schedule.starts[ZkX509Rfc5280StarkFamilyV1::SerialCompare as usize];
        let serial_fixed = schedule
            .fixed_row(serial_start)
            .expect("serial left-copy phase");
        let serial_current = normalized_row_v1(serial_row, &serial_fixed);
        let (current_aux, next_aux) = serial_comparator_aux_v1(&serial_current);
        let canonical = evaluate_zk_x509_rfc5280_stark_residues_v1(
            &serial_current,
            &serial_row,
            &current_aux,
            &next_aux,
            &serial_fixed,
            der_challenges_v1(),
            challenges_v1(),
            terminal_claims_v1(),
        )
        .expect("serial left phase evaluator");
        assert!(canonical.iter().all(|residue| *residue == F::ZERO));
        let mut changed_serial_right_phase = serial_row;
        changed_serial_right_phase[BASE_A] = changed_serial_right_phase[BASE_A].add(F::ONE);
        let rejected = evaluate_zk_x509_rfc5280_stark_residues_v1(
            &serial_current,
            &changed_serial_right_phase,
            &current_aux,
            &next_aux,
            &serial_fixed,
            der_challenges_v1(),
            challenges_v1(),
            terminal_claims_v1(),
        )
        .expect("serial phase mutation evaluator");
        assert!(
            rejected.iter().any(|residue| *residue != F::ZERO),
            "left and right physical serial phases cannot carry different copy tuples"
        );

        let calendar_start = schedule.starts[ZkX509Rfc5280StarkFamilyV1::Calendar as usize];
        let calendar_fixed = schedule
            .fixed_row(calendar_start)
            .expect("calendar component-zero phase");
        let calendar_row = active_zero_row_v1();
        let calendar_current = normalized_row_v1(calendar_row, &calendar_fixed);
        let (current_aux, next_aux) = serial_comparator_aux_v1(&calendar_current);
        let canonical = evaluate_zk_x509_rfc5280_stark_residues_v1(
            &calendar_current,
            &calendar_row,
            &current_aux,
            &next_aux,
            &calendar_fixed,
            der_challenges_v1(),
            challenges_v1(),
            terminal_claims_v1(),
        )
        .expect("calendar phase evaluator");
        assert!(canonical.iter().all(|residue| *residue == F::ZERO));
        let mut changed_calendar_phase = calendar_row;
        changed_calendar_phase[BASE_C] = F::ONE;
        let rejected = evaluate_zk_x509_rfc5280_stark_residues_v1(
            &calendar_current,
            &changed_calendar_phase,
            &current_aux,
            &next_aux,
            &calendar_fixed,
            der_challenges_v1(),
            challenges_v1(),
            terminal_claims_v1(),
        )
        .expect("calendar phase mutation evaluator");
        assert!(
            rejected.iter().any(|residue| *residue != F::ZERO),
            "six physical calendar phases must copy one common semantic row"
        );
    }

    #[test]
    fn normalized_copy_product_is_total_at_a_forced_zero_factor() {
        let schedule =
            compile_zk_x509_rfc5280_stark_fixed_schedule_v1(ZkX509Rfc5280StarkShapeV1::default())
                .expect("fixed schedule");
        let comparison = canonical_serial_comparisons_v1(&[7], &[vec![8]])
            .expect("canonical comparison")
            .remove(0);
        let row =
            build_zk_x509_rfc5280_serial_comparison_rows_v1(&comparison).expect("serial rows")[0];
        let serial_start = schedule.starts[ZkX509Rfc5280StarkFamilyV1::SerialCompare as usize];
        let fixed = schedule
            .fixed_row(serial_start)
            .expect("serial left-copy phase");
        let current = normalized_row_v1(row, &fixed);
        let mut challenges = challenges_v1();
        let tuple = [
            current[BASE_COPY_DOMAIN],
            current[BASE_COPY_KEY_1],
            current[BASE_COPY_KEY_2],
            current[BASE_COPY_VALUE],
        ];
        let non_domain_sum = tuple[1..]
            .iter()
            .zip(&challenges.tuple[0][1..4])
            .fold(F::ZERO, |sum, (value, coefficient)| {
                sum.add(value.mul(*coefficient))
            });
        challenges.tuple[0][0] = F::ZERO
            .sub(non_domain_sum.mul(tuple[0].inv().expect("copy domain separator is nonzero")));
        challenges.validate().expect("forced-zero copy challenges");
        assert_eq!(normalized_copy_factor_v1(&current, 0, challenges), F::ZERO);

        let mut current_aux = neutral_aux_v1();
        let mut next_aux = neutral_aux_v1();
        for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
            let factor = normalized_copy_factor_v1(&current, lane, challenges);
            let after = F::ONE.add(current[BASE_COPY_CONSUMER_ACTIVE].mul(factor.sub(F::ONE)));
            current_aux[AUX_SERIAL_CONSUMER_AFTER + lane] = after;
            next_aux[AUX_SERIAL_CONSUMER_BEFORE + lane] = after;
        }
        assert_eq!(current_aux[AUX_SERIAL_CONSUMER_AFTER], F::ZERO);
        let residues = evaluate_zk_x509_rfc5280_stark_residues_v1(
            &current,
            &row,
            &current_aux,
            &next_aux,
            &fixed,
            der_challenges_v1(),
            challenges,
            terminal_claims_v1(),
        )
        .expect("zero copy factor never invokes an inverse");
        assert!(residues.iter().all(|residue| *residue == F::ZERO));
    }

    #[test]
    fn semantic_source_table_is_unique_and_multiplicity_exact() {
        let cell_a = ZkX509Rfc5280SourceCellV1 {
            document: 0,
            address: 7,
            value: 11,
        };
        let cell_b = ZkX509Rfc5280SourceCellV1 {
            document: 1,
            address: 3,
            value: 13,
        };
        let source_node = ZkX509Rfc5280NodeProvenanceV1 {
            document: 0,
            node: 0,
            parent_node: u16::MAX,
            child_ordinal: 0,
            start: 7,
            content_start: 7,
            content_end: 8,
            depth: 0,
            tag_class: 0,
            constructed: false,
            tag_number: 4,
            role: ZkX509Rfc5280GrammarRoleV1::CertificateOuterAlgorithm,
            role_instance: 0,
        };
        let witness = ZkX509Rfc5280SemanticWitnessV1 {
            fixed_bytes: vec![
                ZkX509Rfc5280FixedByteV1 {
                    source: cell_a,
                    source_node,
                    expected: 11,
                    purpose: 1,
                    instance: 0,
                    variant: 0,
                    offset: 0,
                    length: 1,
                },
                ZkX509Rfc5280FixedByteV1 {
                    source: cell_a,
                    source_node,
                    expected: 11,
                    purpose: 2,
                    instance: 0,
                    variant: 0,
                    offset: 0,
                    length: 1,
                },
            ],
            equal_bytes: vec![ZkX509Rfc5280EqualByteV1 {
                left: cell_a,
                right: cell_b,
                purpose: 3,
                instance: 0,
                offset: 0,
            }],
            decimal_cells: vec![cell_b],
            calendar_values: Vec::new(),
            numeric_relations: Vec::new(),
            bit_flags: Vec::new(),
            serial_sources: Vec::new(),
            serial_comparisons: Vec::new(),
        };
        let table =
            zk_x509_rfc5280_semantic_source_multiplicities_v1(&witness).expect("source table");
        assert_eq!(
            table,
            vec![
                ZkX509Rfc5280SourceMultiplicityV1 {
                    source: cell_a,
                    required_multiplicity: 3,
                },
                ZkX509Rfc5280SourceMultiplicityV1 {
                    source: cell_b,
                    required_multiplicity: 2,
                },
            ]
        );
        let mut duplicated = table;
        duplicated.push(duplicated[0]);
        duplicated.sort_unstable_by_key(|entry| entry.source);
        assert!(
            duplicated
                .windows(2)
                .any(|pair| pair[0].source == pair[1].source),
            "a duplicated unique source address is mechanically detectable"
        );
    }

    #[test]
    fn serial_comparison_rows_cover_leaf_and_exact_active_adjacencies() {
        let entries = ZK_X509_MAX_CRL_ENTRIES_V1;
        let comparisons = entries + entries - 1;
        assert_eq!(comparisons, MAX_SERIAL_COMPARISONS_V1);
        assert_eq!(
            comparisons * SERIAL_COMPARISON_WIDTH_V1,
            MAX_SERIAL_COMPARISON_ROWS_V1
        );
        assert_eq!(MAX_SERIAL_COMPARISON_ROWS_V1, 2_667);
        assert_eq!(serial_comparison_count_v1(0), 0);
        assert_eq!(serial_comparison_rows_v1(0), 0);

        let mut empty = maximum_private_shape_v1();
        empty.crl_entries = 0;
        refresh_private_shape_derived_rows_v1(&mut empty);
        empty.validate().expect("empty private CRL census");
        let schedule =
            compile_zk_x509_rfc5280_stark_fixed_schedule_v1(ZkX509Rfc5280StarkShapeV1::default())
                .expect("fixed public schedule");
        assert_eq!(
            schedule.counts[ZkX509Rfc5280StarkFamilyV1::SerialCompare as usize],
            MAX_SERIAL_COMPARISON_PHYSICAL_ROWS_V1,
            "the public comparator capacity is independent of the private CRL census"
        );
        empty.serial_rows = u32::try_from(SERIAL_COMPARISON_WIDTH_V1).expect("fits");
        assert_eq!(empty.validate(), Err(ZkX509Rfc5280StarkErrorV1::Shape));
    }

    #[test]
    fn der_backed_serial_source_proves_sign_octet_content_and_zero_padding() {
        let source = serial_source_fixture_v1(0, &[0xff]);
        let rows =
            build_zk_x509_rfc5280_serial_source_rows_v1(&source).expect("serial source rows");
        assert_eq!(rows.len(), SERIAL_COMPARISON_WIDTH_V1);
        assert_eq!(rows[0][BASE_IS_WRITE], F::ONE);
        assert_eq!(rows[0][BASE_ADDRESS], F(100));
        assert_eq!(rows[0][SERIAL_SOURCE_QUERY_VALUE], F::ZERO);
        assert_eq!(rows[1][BASE_ADDRESS], F(101));
        assert_eq!(rows[1][SERIAL_SOURCE_QUERY_VALUE], F(0xff));
        assert!(
            rows[2..].iter().all(|row| {
                row[BASE_EQUAL] == F::ZERO
                    && row[BASE_VALUE] == F::ZERO
                    && row[BASE_IS_WRITE] == F::ZERO
            }),
            "inactive magnitude slots are algebraically canonical padding"
        );
        let (byte_multiplicities, node_multiplicities) =
            zk_x509_rfc5280_serial_lookup_multiplicities_v1(core::slice::from_ref(&source))
                .expect("lookup multiplicities");
        assert_eq!(
            byte_multiplicities
                .iter()
                .map(|entry| (
                    entry.source.address,
                    entry.source.value,
                    entry.required_multiplicity
                ))
                .collect::<Vec<_>>(),
            vec![(100, 0, 1), (101, 0xff, 1)]
        );
        assert_eq!(
            node_multiplicities,
            vec![ZkX509Rfc5280SerialNodeMultiplicityV1 {
                document: 0,
                node: 7,
                required_multiplicity: 21,
            }]
        );

        let schedule =
            compile_zk_x509_rfc5280_stark_fixed_schedule_v1(ZkX509Rfc5280StarkShapeV1::default())
                .expect("source schedule");
        let start = schedule.starts[ZkX509Rfc5280StarkFamilyV1::SerialSource as usize];
        for (offset, source_row) in rows.iter().enumerate() {
            let next = rows.get(offset + 1).unwrap_or(source_row);
            let fixed = schedule
                .fixed_row(start + offset)
                .expect("source fixed row");
            let current = normalized_row_v1(*source_row, &fixed);
            let (current_aux, next_aux) = serial_source_aux_v1(&current, challenges_v1());
            let residues = evaluate_zk_x509_rfc5280_stark_residues_v1(
                &current,
                next,
                &current_aux,
                &next_aux,
                &fixed,
                der_challenges_v1(),
                challenges_v1(),
                terminal_claims_v1(),
            )
            .expect("source evaluator");
            assert!(
                residues.iter().all(|residue| *residue == F::ZERO),
                "DER-backed source offset {offset} must satisfy every local residue"
            );
        }
    }

    #[test]
    fn der_backed_serial_lookup_is_complete_at_a_forced_zero_factor() {
        let source = serial_source_fixture_v1(0, &[0xff]);
        let rows =
            build_zk_x509_rfc5280_serial_source_rows_v1(&source).expect("serial source rows");
        let current = rows[1];
        let next = rows[2];
        let mut challenges = challenges_v1();
        let non_domain_sum = current[BASE_DOCUMENT]
            .mul(challenges.tuple[0][1])
            .add(current[BASE_ADDRESS].mul(challenges.tuple[0][2]))
            .add(current[SERIAL_SOURCE_QUERY_VALUE].mul(challenges.tuple[0][3]));
        challenges.tuple[0][0] = F::ZERO
            .sub(non_domain_sum.mul(F(91).inv().expect("nonzero serial-byte domain separator")));
        challenges.validate().expect("forced-collision challenges");
        assert_eq!(
            serial_byte_lookup_factor_v1(
                current[BASE_DOCUMENT],
                current[BASE_ADDRESS],
                current[SERIAL_SOURCE_QUERY_VALUE],
                0,
                challenges,
            ),
            F::ZERO
        );

        let schedule =
            compile_zk_x509_rfc5280_stark_fixed_schedule_v1(ZkX509Rfc5280StarkShapeV1::default())
                .expect("source schedule");
        let index = schedule.starts[ZkX509Rfc5280StarkFamilyV1::SerialSource as usize] + 1;
        let fixed = schedule.fixed_row(index).expect("source fixed row");
        let current = normalized_row_v1(current, &fixed);
        let (current_aux, next_aux) = serial_source_aux_v1(&current, challenges);
        let residues = evaluate_zk_x509_rfc5280_stark_residues_v1(
            &current,
            &next,
            &current_aux,
            &next_aux,
            &fixed,
            der_challenges_v1(),
            challenges,
            terminal_claims_v1(),
        )
        .expect("zero-safe source evaluator");
        assert!(residues.iter().all(|residue| *residue == F::ZERO));

        let mut corrupted_aux = current_aux;
        corrupted_aux[AUX_SERIAL_BYTE_QUERY_ZERO] = F::ZERO;
        let residues = evaluate_zk_x509_rfc5280_stark_residues_v1(
            &current,
            &next,
            &corrupted_aux,
            &next_aux,
            &fixed,
            der_challenges_v1(),
            challenges,
            terminal_claims_v1(),
        )
        .expect("corrupted zero-safe source evaluator");
        assert!(residues.iter().any(|residue| *residue != F::ZERO));
    }

    fn serial_copy_products_v1(
        sources: &[ZkX509Rfc5280SerialSourceV1],
        comparisons: &[ZkX509Rfc5280SerialComparisonV1],
    ) -> (
        [F; ZK_X509_RFC5280_STARK_BUS_LANES_V1],
        [F; ZK_X509_RFC5280_STARK_BUS_LANES_V1],
    ) {
        let challenges = challenges_v1();
        let mut producer = [F::ONE; ZK_X509_RFC5280_STARK_BUS_LANES_V1];
        let mut consumer = producer;
        for source in sources {
            for (offset, value) in source.frame.iter().copied().enumerate() {
                for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
                    producer[lane] = producer[lane].mul(serial_copy_factor_v1(
                        F(u64::from(source.logical_id)),
                        F(u64::try_from(offset).expect("offset fits")),
                        F(u64::from(value)),
                        lane,
                        challenges,
                    ));
                }
            }
        }
        for comparison in comparisons {
            for (logical_id, frame) in [
                (comparison.left_instance, &comparison.left),
                (comparison.right_instance, &comparison.right),
            ] {
                for (offset, value) in frame.iter().copied().enumerate() {
                    for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
                        consumer[lane] = consumer[lane].mul(serial_copy_factor_v1(
                            F(u64::from(logical_id)),
                            F(u64::try_from(offset).expect("offset fits")),
                            F(u64::from(value)),
                            lane,
                            challenges,
                        ));
                    }
                }
            }
        }
        (producer, consumer)
    }

    #[test]
    fn locally_valid_comparator_shadow_values_fail_serial_copy_and_der_lookup() {
        let canonical =
            canonical_serial_comparisons_v1(&[7], &[vec![8]]).expect("canonical comparison");
        let canonical_sources = vec![
            serial_source_fixture_v1(0, &[7]),
            serial_source_fixture_v1(1, &[8]),
        ];
        let (producer, consumer) = serial_copy_products_v1(&canonical_sources, &canonical);
        assert_eq!(producer, consumer);

        let mut changed_comparison = canonical.clone();
        changed_comparison[0].left = serial_frame_v1(&[6]).expect("alternate valid magnitude");
        validate_serial_comparison_v1(&changed_comparison[0])
            .expect("local leaf inequality still holds");
        let (producer, changed_consumer) =
            serial_copy_products_v1(&canonical_sources, &changed_comparison);
        assert_ne!(
            producer, changed_consumer,
            "four-lane serial copy terminals bind comparator values to sources"
        );

        let mut shadow_sources = canonical_sources.clone();
        shadow_sources[0].frame = serial_frame_v1(&[6]).expect("alternate source frame");
        let (shadow_producer, shadow_consumer) =
            serial_copy_products_v1(&shadow_sources, &changed_comparison);
        assert_eq!(
            shadow_producer, shadow_consumer,
            "the copy bus alone intentionally sees a self-consistent shadow"
        );
        let canonical_row = build_zk_x509_rfc5280_serial_source_rows_v1(&canonical_sources[0])
            .expect("canonical source rows");
        let mut shadow_row = canonical_row[1];
        shadow_row[BASE_VALUE] = F(6);
        shadow_row[SERIAL_SOURCE_QUERY_VALUE] = F(6);
        write_u8_bits_v1(&mut shadow_row, BASE_BYTE_BITS, 6);
        shadow_row[SERIAL_SOURCE_FIRST_INVERSE] = F(6).inv().expect("nonzero");
        let table_factor = serial_byte_lookup_factor_v1(
            canonical_row[1][BASE_DOCUMENT],
            canonical_row[1][BASE_ADDRESS],
            canonical_row[1][SERIAL_SOURCE_QUERY_VALUE],
            0,
            challenges_v1(),
        );
        let shadow_query_factor = serial_byte_lookup_factor_v1(
            shadow_row[BASE_DOCUMENT],
            shadow_row[BASE_ADDRESS],
            shadow_row[SERIAL_SOURCE_QUERY_VALUE],
            0,
            challenges_v1(),
        );
        assert_ne!(
            table_factor, shadow_query_factor,
            "DER-byte lookup rejects a value-mutated self-consistent shadow"
        );
    }

    #[test]
    fn source_address_and_node_span_mutations_break_der_lookup_keys() {
        let source = serial_source_fixture_v1(0, &[0xff]);
        let rows =
            build_zk_x509_rfc5280_serial_source_rows_v1(&source).expect("canonical source rows");
        let canonical = rows[1];
        let mut moved = canonical;
        moved[BASE_START] = moved[BASE_START].add(F::ONE);
        moved[BASE_CONTENT_START] = moved[BASE_CONTENT_START].add(F::ONE);
        moved[BASE_CONTENT_END] = moved[BASE_CONTENT_END].add(F::ONE);
        moved[BASE_ADDRESS] = moved[BASE_ADDRESS].add(F::ONE);
        assert_ne!(
            serial_node_lookup_factor_v1(&canonical, 0, challenges_v1()),
            serial_node_lookup_factor_v1(&moved, 0, challenges_v1()),
            "a self-consistently moved source span no longer matches the DER node table"
        );
        assert_ne!(
            serial_byte_lookup_factor_v1(
                canonical[BASE_DOCUMENT],
                canonical[BASE_ADDRESS],
                canonical[SERIAL_SOURCE_QUERY_VALUE],
                0,
                challenges_v1(),
            ),
            serial_byte_lookup_factor_v1(
                moved[BASE_DOCUMENT],
                moved[BASE_ADDRESS],
                moved[SERIAL_SOURCE_QUERY_VALUE],
                0,
                challenges_v1(),
            ),
            "a moved content address no longer matches the DER byte table"
        );
    }

    fn assert_serial_group_satisfied_v1(
        comparison: &ZkX509Rfc5280SerialComparisonV1,
        _crl_entries: usize,
        group: usize,
    ) {
        let rows = build_zk_x509_rfc5280_serial_comparison_rows_v1(comparison)
            .expect("canonical serial rows");
        let schedule =
            compile_zk_x509_rfc5280_stark_fixed_schedule_v1(ZkX509Rfc5280StarkShapeV1::default())
                .expect("serial schedule");
        let start = schedule.starts[ZkX509Rfc5280StarkFamilyV1::SerialCompare as usize]
            + group * SERIAL_COMPARISON_WIDTH_V1 * SERIAL_COMPARISON_PHASES_V1;
        for (offset, row) in rows.iter().copied().enumerate() {
            for phase in 0..SERIAL_COMPARISON_PHASES_V1 {
                let physical_offset = offset * SERIAL_COMPARISON_PHASES_V1 + phase;
                let fixed = schedule
                    .fixed_row(start + physical_offset)
                    .expect("verifier-fixed serial row");
                let current = normalized_row_v1(row, &fixed);
                let next = if phase + 1 == SERIAL_COMPARISON_PHASES_V1 {
                    rows.get(offset + 1).copied().unwrap_or(row)
                } else {
                    row
                };
                let (current_aux, next_aux) = serial_comparator_aux_v1(&current);
                let residues = evaluate_zk_x509_rfc5280_stark_residues_v1(
                    &current,
                    &next,
                    &current_aux,
                    &next_aux,
                    &fixed,
                    der_challenges_v1(),
                    challenges_v1(),
                    terminal_claims_v1(),
                )
                .expect("serial evaluator");
                assert!(
                    residues.iter().all(|residue| *residue == F::ZERO),
                    "serial group {group} logical offset {offset} phase {phase} \
                     must satisfy every residue"
                );
            }
        }
    }

    #[test]
    fn adjacent_order_handles_length_boundary_and_equal_prefix() {
        let boundary = canonical_serial_comparisons_v1(&[0x7f], &[vec![0xff], vec![0x01, 0x00]])
            .expect("0xff is less than 0x0100 by unsigned magnitude");
        assert_eq!(boundary.len(), 3);
        assert_eq!(
            boundary[2].kind,
            ZkX509Rfc5280SerialComparisonKindV1::AdjacentStrictOrder
        );
        assert_serial_group_satisfied_v1(&boundary[2], 2, 2);

        let equal_prefix =
            canonical_serial_comparisons_v1(&[0x02], &[vec![0x01, 0xfe], vec![0x01, 0xff]])
                .expect("equal-prefix magnitudes are ordered by first differing byte");
        assert_serial_group_satisfied_v1(&equal_prefix[2], 2, 2);
    }

    #[test]
    fn duplicate_descending_and_noncanonical_padding_reject() {
        assert_eq!(
            canonical_serial_comparisons_v1(&[7], &[vec![8], vec![8]]),
            Err(ZkX509Rfc5280StarkErrorV1::Semantic)
        );
        assert_eq!(
            canonical_serial_comparisons_v1(&[7], &[vec![9], vec![8]]),
            Err(ZkX509Rfc5280StarkErrorV1::Semantic)
        );

        let mut comparison = canonical_serial_comparisons_v1(&[7], &[vec![8], vec![9]])
            .expect("canonical manifest")
            .pop()
            .expect("adjacent comparison");
        comparison.right[ZK_X509_MAX_SERIAL_BYTES_V1] = 1;
        assert_eq!(
            validate_serial_comparison_v1(&comparison),
            Err(ZkX509Rfc5280StarkErrorV1::Semantic),
            "inactive padded bytes are canonical zeros"
        );
        assert_eq!(
            serial_frame_v1(&[0, 1]),
            Err(ZkX509Rfc5280StarkErrorV1::Semantic),
            "magnitude frames cannot carry a leading zero"
        );
    }

    #[test]
    fn dropped_and_reordered_comparators_fail_exact_manifest_and_fixed_schedule() {
        let revoked = vec![vec![8], vec![9], vec![10]];
        let manifest = canonical_serial_comparisons_v1(&[7], &revoked).expect("canonical manifest");
        assert_eq!(manifest.len(), 5);

        let mut dropped = manifest.clone();
        dropped.remove(1);
        assert_eq!(
            validate_serial_comparison_manifest_v1(&[7], &revoked, &dropped),
            Err(ZkX509Rfc5280StarkErrorV1::Semantic)
        );
        let mut reordered = manifest.clone();
        reordered.swap(0, 1);
        assert_eq!(
            validate_serial_comparison_manifest_v1(&[7], &revoked, &reordered),
            Err(ZkX509Rfc5280StarkErrorV1::Semantic)
        );

        let mut shape = maximum_private_shape_v1();
        shape.crl_entries = 3;
        refresh_private_shape_derived_rows_v1(&mut shape);
        shape.serial_rows -= 1;
        assert_eq!(shape.validate(), Err(ZkX509Rfc5280StarkErrorV1::Shape));

        let comparison = &manifest[2];
        let rows = build_zk_x509_rfc5280_serial_comparison_rows_v1(comparison)
            .expect("adjacent serial rows");
        let schedule =
            compile_zk_x509_rfc5280_stark_fixed_schedule_v1(ZkX509Rfc5280StarkShapeV1::default())
                .expect("serial schedule");
        let serial_start = schedule.starts[ZkX509Rfc5280StarkFamilyV1::SerialCompare as usize];
        let wrong_fixed = schedule
            .fixed_row(serial_start + SERIAL_COMPARISON_PHASES_V1 - 1)
            .expect("first leaf comparator fixed row");
        let current = normalized_row_v1(rows[0], &wrong_fixed);
        let (current_aux, next_aux) = serial_comparator_aux_v1(&current);
        let residues = evaluate_zk_x509_rfc5280_stark_residues_v1(
            &current,
            &rows[1],
            &current_aux,
            &next_aux,
            &wrong_fixed,
            der_challenges_v1(),
            challenges_v1(),
            terminal_claims_v1(),
        )
        .expect("reordered comparator evaluator");
        assert!(
            residues.iter().any(|residue| *residue != F::ZERO),
            "an adjacent-order row cannot occupy a leaf-comparison slot"
        );
    }

    #[test]
    fn first_difference_and_state_transition_mutations_are_detected() {
        let manifest = canonical_serial_comparisons_v1(&[7], &[vec![0xff], vec![0x01, 0x00]])
            .expect("canonical boundary manifest");
        let comparison = &manifest[2];
        let mut rows = build_zk_x509_rfc5280_serial_comparison_rows_v1(comparison)
            .expect("canonical comparator rows");
        assert_eq!(rows[0][SERIAL_LESS], F::ONE);
        rows[0][SERIAL_LESS] = F::ZERO;

        let schedule =
            compile_zk_x509_rfc5280_stark_fixed_schedule_v1(ZkX509Rfc5280StarkShapeV1::default())
                .expect("serial schedule");
        let start = schedule.starts[ZkX509Rfc5280StarkFamilyV1::SerialCompare as usize]
            + 2 * SERIAL_COMPARISON_WIDTH_V1 * SERIAL_COMPARISON_PHASES_V1;
        let fixed = schedule
            .fixed_row(start + SERIAL_COMPARISON_PHASES_V1 - 1)
            .expect("adjacent comparator fixed row");
        let current = normalized_row_v1(rows[0], &fixed);
        let (current_aux, next_aux) = serial_comparator_aux_v1(&current);
        let residues = evaluate_zk_x509_rfc5280_stark_residues_v1(
            &current,
            &rows[1],
            &current_aux,
            &next_aux,
            &fixed,
            der_challenges_v1(),
            challenges_v1(),
            terminal_claims_v1(),
        )
        .expect("mutated comparator evaluator");
        assert!(
            residues.iter().any(|residue| *residue != F::ZERO),
            "the first-difference success bit is algebraically linked"
        );

        // Use an equal-length pair with an equal first payload byte so the
        // canonical prefix state entering the second payload row is one.
        // The former boundary fixture had already transitioned to zero on its
        // length row, making a write of zero a no-op rather than a mutation.
        let state_manifest =
            canonical_serial_comparisons_v1(&[7], &[vec![0x01, 0x02], vec![0x01, 0x03]])
                .expect("canonical equal-prefix manifest");
        let mut rows = build_zk_x509_rfc5280_serial_comparison_rows_v1(&state_manifest[2])
            .expect("canonical equal-prefix comparator rows");
        assert_eq!(rows[0][BASE_STATE_AFTER], F::ONE);
        assert_eq!(rows[1][BASE_STATE_BEFORE], F::ONE);
        rows[1][BASE_STATE_BEFORE] = F::ZERO;
        let current = normalized_row_v1(rows[0], &fixed);
        let (current_aux, next_aux) = serial_comparator_aux_v1(&current);
        let residues = evaluate_zk_x509_rfc5280_stark_residues_v1(
            &current,
            &rows[1],
            &current_aux,
            &next_aux,
            &fixed,
            der_challenges_v1(),
            challenges_v1(),
            terminal_claims_v1(),
        )
        .expect("mutated transition evaluator");
        assert!(
            residues.iter().any(|residue| *residue != F::ZERO),
            "prefix state cannot be reset between comparator rows"
        );
    }

    #[test]
    fn proof_rows_reject_zero_oversized_alias_leading_zero_and_inactive_bytes() {
        let mut comparisons =
            canonical_serial_comparisons_v1(&[7], &[vec![8]]).expect("canonical comparison");
        let comparison = comparisons.remove(0);
        let canonical =
            build_zk_x509_rfc5280_serial_comparison_rows_v1(&comparison).expect("canonical rows");
        let schedule =
            compile_zk_x509_rfc5280_stark_fixed_schedule_v1(ZkX509Rfc5280StarkShapeV1::default())
                .expect("serial schedule");
        let start = schedule.starts[ZkX509Rfc5280StarkFamilyV1::SerialCompare as usize];

        let rejects = |rows: &[ZkX509Rfc5280StarkBaseRowV1], offset: usize, label: &str| {
            let fixed = schedule
                .fixed_row(
                    start + offset * SERIAL_COMPARISON_PHASES_V1 + SERIAL_COMPARISON_PHASES_V1 - 1,
                )
                .expect("fixed comparator row");
            let next = rows.get(offset + 1).unwrap_or(&rows[offset]);
            let current = normalized_row_v1(rows[offset], &fixed);
            let (current_aux, next_aux) = serial_comparator_aux_v1(&current);
            let residues = evaluate_zk_x509_rfc5280_stark_residues_v1(
                &current,
                next,
                &current_aux,
                &next_aux,
                &fixed,
                der_challenges_v1(),
                challenges_v1(),
                terminal_claims_v1(),
            )
            .expect("mutated comparator evaluator");
            assert!(
                residues.iter().any(|residue| *residue != F::ZERO),
                "{label} must violate an algebraic comparator constraint"
            );
        };

        let mut zero_length = canonical.clone();
        zero_length[0][BASE_A] = F::ZERO;
        zero_length[0][SERIAL_LEFT_LENGTH] = F::ZERO;
        write_u8_bits_v1(&mut zero_length[0], SERIAL_LEFT_BITS, 0);
        rejects(&zero_length, 0, "zero length");

        let mut oversized_length = canonical.clone();
        oversized_length[0][BASE_A] = F(21);
        oversized_length[0][SERIAL_LEFT_LENGTH] = F(21);
        write_u8_bits_v1(&mut oversized_length[0], SERIAL_LEFT_BITS, 21);
        rejects(&oversized_length, 0, "length 21");

        let mut leading_zero = canonical.clone();
        leading_zero[1][BASE_A] = F::ZERO;
        leading_zero[1][BASE_INVERSE] = F::ZERO;
        write_u8_bits_v1(&mut leading_zero[1], SERIAL_LEFT_BITS, 0);
        rejects(&leading_zero, 1, "leading zero");

        let mut inactive_nonzero = canonical.clone();
        let last = SERIAL_COMPARISON_WIDTH_V1 - 1;
        inactive_nonzero[last][BASE_A] = F::ONE;
        inactive_nonzero[last][BASE_B] = F::ONE;
        write_u8_bits_v1(&mut inactive_nonzero[last], SERIAL_LEFT_BITS, 1);
        write_u8_bits_v1(&mut inactive_nonzero[last], SERIAL_RIGHT_BITS, 1);
        rejects(&inactive_nonzero, last, "nonzero inactive padding");

        let mut length_alias = canonical;
        length_alias[last][SERIAL_LEFT_LENGTH] = F(2);
        rejects(&length_alias, last, "value-preserving length/padding alias");
    }
}
