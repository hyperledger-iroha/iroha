//! Proof-facing adapter for compact trust-anchor membership.
//!
//! The adapter registers one log-seven trace. Thirteen rows own the exact
//! shared SHA calls for one occupied leaf and twelve internal nodes, and 91
//! rows serialize the root SPKI with one reusable byte decomposition. Four
//! independently challenged running products bind those bytes simultaneously
//! to the leaf SHA input and strict-DER output consumer. Two SHA factors are
//! advanced per hash row, keeping the maximum committed-column degree at three.
#[cfg(any(test, feature = "privacy-release-evidence"))]
use super::accumulator_air::{
    ZK_X509_CA_ACCUMULATOR_NONPADDING_ROWS_V1, ZkX509CaAccumulatorTraceV1,
};
use super::{
    accumulator_air::{
        CA_CURRENT_START, CA_DIGEST_BYTE_BITS_START, CA_DIGEST_START, CA_DIRECTION,
        CA_INDEX_BITS_START, CA_IO_BYTE, CA_IO_BYTE_BITS_START, CA_IO_WORD_ACC, CA_LEFT_START,
        CA_RIGHT_START, CA_SIBLING_BYTE_BITS_START, CA_SIBLING_START,
        ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1, ZK_X509_CA_ACCUMULATOR_BASE_CONSTRAINT_COUNT_V1,
        ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1, ZK_X509_CA_ACCUMULATOR_IO_ROWS_V1,
        ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1, ZK_X509_CA_LEAF_SPKI_MESSAGE_OFFSET_V1,
        ZK_X509_CA_LEAF_SPKI_PREFIX_BYTE_V1, ZkX509AccumulatorAirErrorV1,
        ZkX509CaAccumulatorRowKindV1, ca_accumulator_fixed_row_v1,
    },
    credential_pre_aux::{
        ZkX509CredentialMainPreAuxV1, ZkX509CredentialPreAuxErrorV1,
        absorb_zk_x509_credential_pre_aux_binding_v1, derive_zk_x509_credential_pre_aux_binding_v1,
    },
    merkle::{
        ZK_X509_CA_COMPACT_TREE_DEPTH_V1, ZK_X509_CA_SPKI_DER_BYTES_V1, ZkX509MerkleErrorV1,
        ca_leaf_preimage_v1, ca_node_preimage_v1,
    },
    profile::{
        ZK_X509_CA_COMPOSITION_DEGREE_CHUNKS_V1, ZK_X509_CA_FRI_LDE_LOG2_V1,
        ZK_X509_CA_FRI_ROUNDS_V1, ZK_X509_CA_FRI_TERMINAL_DEGREE_BOUND_V1,
        ZK_X509_CA_FRI_TERMINAL_LOG2_V1, ZK_X509_CA_PRE_DEEP_MAXIMUM_BYTES_V1,
        ZK_X509_CA_TRACE_MASK_DEGREE_V1, ZK_X509_FRI_QUERY_COUNT_V1, ZK_X509_GRINDING_BITS_V1,
        ZK_X509_MAX_PROOF_BYTES_V1, ZK_X509_PROOF_VERSION_V1, ZK_X509_SUITE_V1,
    },
    rfc5280_stark::{
        ZK_X509_RFC5280_STARK_BUS_LANES_V1, ZkX509Rfc5280OutputRoleV1,
        ZkX509Rfc5280StarkChallengesV1,
    },
    sha_call_bus_stark::{
        ZK_X509_SHA_BUS_LANES_V1, ZK_X509_SHA_CA_LEAF_CALL_V1, ZK_X509_SHA_CA_NODE_CALL_START_V1,
        ZkX509ShaCallActivationV1, ZkX509ShaCallBusChallengesV1, ZkX509ShaCallRoleV1,
        ZkX509ShaCallScheduleV1, ZkX509ShaCallWordKindV1, compress_sha_call_fields_v1,
    },
};
#[cfg(any(test, feature = "privacy-release-evidence"))]
use crate::privacy_engines::prover_randomness::{
    HealthCheckedTryCryptoRngV1, TryCryptoProverRandomnessErrorV1,
};
#[cfg(any(test, feature = "privacy-release-evidence"))]
use crate::privacy_engines::transparent_stark::{grind_nonce_v1, masked_trace_lde_column_v1};
use crate::privacy_engines::{
    aggregate_stark::{self as aggregate, AggregateStarkErrorV1},
    prover_randomness::TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
    transparent_stark::{
        GOLDILOCKS_GENERATOR_V1, GoldilocksFieldV1 as F, GoldilocksFp4V1 as E,
        TransparentStarkErrorV1, TransparentTranscriptV1, append_u16_v1, append_u32_v1,
        append_u64_v1, goldilocks_evaluate_coset_v1, goldilocks_ifft_v1,
        goldilocks_primitive_root_v1, sha256_frame_v1, transparent_stark_zk_mask_geometry_v1,
        verify_grinding_nonce_v1,
    },
};
#[cfg(any(test, feature = "privacy-release-evidence"))]
use rand::TryCryptoRng;
#[cfg(test)]
use rand::rngs::OsRng;
use thiserror::Error;
/// Native trace logarithm for 104 non-padding rows.
pub(crate) const ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1: u8 = 7;
/// SHA call products plus serialized SHA-source and RFC-output products.
pub(crate) const ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1: usize = 128;
/// Verifier-preprocessed row selectors, source constants, and I/O schedule.
pub(crate) const ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1: usize = 80;
/// SHA within-row identities plus serialized product initialization,
/// transition, and canonical-zero identities.
pub(crate) const ZK_X509_CA_ACCUMULATOR_AUX_CONSTRAINT_COUNT_V1: usize = 144;
/// Exact opened-row residue count.
pub(crate) const ZK_X509_CA_ACCUMULATOR_CONSTRAINT_COUNT_V1: usize =
    ZK_X509_CA_ACCUMULATOR_BASE_CONSTRAINT_COUNT_V1
        + ZK_X509_CA_ACCUMULATOR_AUX_CONSTRAINT_COUNT_V1
        + ZK_X509_CA_ACCUMULATOR_TERMINAL_CONSTRAINT_COUNT_V1;
/// Two SHA families, the completed leaf source, and root-SPKI I/O terminal.
pub(crate) const ZK_X509_CA_ACCUMULATOR_TERMINAL_CONSTRAINT_COUNT_V1: usize =
    4 * ZK_X509_SHA_BUS_LANES_V1;
/// Maximum committed-column degree in any residue.
pub(crate) const ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1: u8 = 3;
/// Haböck--Al Kindi reduced AIR degree `d = d_AIR - 1`.
pub(crate) const ZK_X509_CA_ACCUMULATOR_REDUCED_AIR_DEGREE_V1: usize =
    ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1 as usize - 1;
/// Physical 64-column base chunks.
pub(crate) const ZK_X509_CA_ACCUMULATOR_BASE_CHUNKS_V1: usize =
    ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1.div_ceil(64);
/// Physical 64-column auxiliary chunks.
pub(crate) const ZK_X509_CA_ACCUMULATOR_AUX_CHUNKS_V1: usize =
    ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1.div_ceil(64);
/// Total physical commitment chunks.
pub(crate) const ZK_X509_CA_ACCUMULATOR_CHUNKS_V1: usize =
    ZK_X509_CA_ACCUMULATOR_BASE_CHUNKS_V1 + ZK_X509_CA_ACCUMULATOR_AUX_CHUNKS_V1;
/// Minimum LDE-to-FRI-degree ratio admitted by the release.
pub(crate) const ZK_X509_CA_ACCUMULATOR_FRI_RATE_DENOMINATOR_V1: usize = 32;
/// Authenticated scratch rows per independently decrypted block.
///
/// One native trace per block bounds a simultaneous current/next base, aux,
/// and fixed working set below two MiB.  A prover must not silently substitute
/// the common full-profile domain or a larger scratch block.
pub(crate) const ZK_X509_CA_ACCUMULATOR_SCRATCH_CHUNK_ROWS_V1: usize =
    ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1;
/// The first release uses one Fp4 composition lane.
pub(crate) const ZK_X509_CA_ACCUMULATOR_COMPOSITION_EXTENSION_LANES_V1: usize = 1;
/// Number of base-field components in the release Fp4 lane.
pub(crate) const ZK_X509_CA_ACCUMULATOR_EXTENSION_COMPONENTS_V1: usize = 4;
const FIELD_BYTES_V1: usize = core::mem::size_of::<F>();
const COMPOSITION_DEGREE_CHUNKS_V1: usize = 3;
const SCRATCH_TAG_BYTES_V1: usize = 16;
/// Governed ceiling for challenge-independent native material.
pub(crate) const ZK_X509_CA_ACCUMULATOR_MAX_NATIVE_MATERIAL_BYTES_V1: usize = 1 << 20;
/// Governed ceiling for all local base, aux, and fixed LDE field payloads.
pub(crate) const ZK_X509_CA_ACCUMULATOR_MAX_LOCAL_LDE_BYTES_V1: usize = 256 << 20;
/// Governed ceiling for authenticated local trace scratch.
pub(crate) const ZK_X509_CA_ACCUMULATOR_MAX_LOCAL_SCRATCH_BYTES_V1: usize = 256 << 20;
/// Governed ceiling for the CA adapter's peak resident field payload.
///
/// The first-release producer materializes the complete local LDE.  This
/// ceiling therefore includes that LDE instead of claiming the lower resident
/// bound of a future streaming implementation.  It excludes only the generic
/// Merkle/FRI engine, whose independent ceiling is enforced by the local
/// subproof prover.
pub(crate) const ZK_X509_CA_ACCUMULATOR_MAX_ADAPTER_RESIDENT_BYTES_V1: usize = 128 << 20;
/// Governed ceiling for native-to-local-LDE radix-2 butterflies.
pub(crate) const ZK_X509_CA_ACCUMULATOR_MAX_LDE_BUTTERFLIES_V1: usize = 250_000_000;
/// Governed ceiling for base-field component constraint evaluations.
pub(crate) const ZK_X509_CA_ACCUMULATOR_MAX_COMPOSITION_COMPONENT_EVALUATIONS_V1: usize =
    200_000_000;
const SOURCE_WORDS_V1: usize = 48;
const DIGEST_WORDS_V1: usize = 8;
const SOURCE_PAIR_STEPS_V1: usize = SOURCE_WORDS_V1 / 2;
const DIGEST_PAIR_STEPS_V1: usize = DIGEST_WORDS_V1 / 2;
const SOURCE_STATES_V1: usize = SOURCE_PAIR_STEPS_V1 + 1;
const DIGEST_STATES_V1: usize = DIGEST_PAIR_STEPS_V1 + 1;
const SOURCE_AUX_START: usize = 0;
const DIGEST_AUX_START: usize = SOURCE_STATES_V1 * ZK_X509_SHA_BUS_LANES_V1;
const SERIALIZED_SHA_PRODUCT_START: usize =
    DIGEST_AUX_START + DIGEST_STATES_V1 * ZK_X509_SHA_BUS_LANES_V1;
const ROOT_SPKI_IO_PRODUCT_START: usize = SERIALIZED_SHA_PRODUCT_START + ZK_X509_SHA_BUS_LANES_V1;
/// Canonical root-SPKI channel before two channels per public disclosure.
pub(crate) const ZK_X509_CA_ACCUMULATOR_ROOT_SPKI_BASE_CHANNEL_V1: u32 = 28;
/// Exact number of root-SPKI consumer events.
pub(crate) const ZK_X509_CA_ACCUMULATOR_ROOT_SPKI_IO_EVENTS_V1: u16 =
    ZK_X509_CA_SPKI_DER_BYTES_V1 as u16;
const FIX_ACTIVE: usize = 0;
const FIX_LEAF: usize = 1;
const FIX_NODE: usize = 2;
const FIX_PADDING: usize = 3;
const FIX_TRANSITION: usize = 4;
const FIX_LAST: usize = 5;
const FIX_LEVEL: usize = 6;
const FIX_CALL: usize = 7;
const FIX_ROLE: usize = 8;
const FIX_SLOT: usize = 9;
const FIX_INDEX_SELECTORS_START: usize = 10;
const FIX_SOURCE_CONSTANTS_START: usize =
    FIX_INDEX_SELECTORS_START + ZK_X509_CA_COMPACT_TREE_DEPTH_V1;
const FIX_IO_ACTIVE: usize = FIX_SOURCE_CONSTANTS_START + SOURCE_WORDS_V1;
const FIX_IO_FIRST: usize = FIX_IO_ACTIVE + 1;
const FIX_IO_LAST: usize = FIX_IO_FIRST + 1;
const FIX_IO_TRANSITION: usize = FIX_IO_LAST + 1;
const FIX_IO_SAME_WORD_TO_NEXT: usize = FIX_IO_TRANSITION + 1;
const FIX_IO_WORD_END: usize = FIX_IO_SAME_WORD_TO_NEXT + 1;
const FIX_IO_OFFSET: usize = FIX_IO_WORD_END + 1;
const FIX_IO_WORD_INDEX: usize = FIX_IO_OFFSET + 1;
const FIX_IO_NEXT_WORD_END: usize = FIX_IO_WORD_INDEX + 1;
const FIX_IO_NEXT_WORD_INDEX: usize = FIX_IO_NEXT_WORD_END + 1;
const LEAF_DYNAMIC_OFFSET_V1: usize = ZK_X509_CA_LEAF_SPKI_MESSAGE_OFFSET_V1;
const NODE_LEFT_DYNAMIC_OFFSET_V1: usize = 75;
const NODE_RIGHT_DYNAMIC_OFFSET_V1: usize = 115;
const LEAF_DYNAMIC_WORD_START_V1: usize = LEAF_DYNAMIC_OFFSET_V1 / 4;
const LEAF_DYNAMIC_WORD_END_V1: usize =
    (LEAF_DYNAMIC_OFFSET_V1 + ZK_X509_CA_SPKI_DER_BYTES_V1 - 1) / 4;
const _: () = {
    assert!(ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1 == 1 << ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1);
    assert!(ZK_X509_SHA_BUS_LANES_V1 == ZK_X509_RFC5280_STARK_BUS_LANES_V1);
    assert!(
        ROOT_SPKI_IO_PRODUCT_START + ZK_X509_RFC5280_STARK_BUS_LANES_V1
            == ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1
    );
    assert!(ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1 == 128);
    assert!(ZK_X509_CA_ACCUMULATOR_AUX_CONSTRAINT_COUNT_V1 == 144);
    assert!(FIX_IO_NEXT_WORD_INDEX + 1 == ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1);
    assert!(ZK_X509_CA_ACCUMULATOR_CONSTRAINT_COUNT_V1 == 1_379);
    assert!(ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1 == 3);
    assert!(ZK_X509_CA_ACCUMULATOR_REDUCED_AIR_DEGREE_V1 == 2);
    assert!(COMPOSITION_DEGREE_CHUNKS_V1 == ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1 as usize);
    assert!(ZK_X509_CA_ACCUMULATOR_BASE_CHUNKS_V1 == 11);
    assert!(ZK_X509_CA_ACCUMULATOR_AUX_CHUNKS_V1 == 2);
    assert!(ZK_X509_CA_ACCUMULATOR_CHUNKS_V1 == 13);
    assert!(FIELD_BYTES_V1 == 8);
    assert!(ZK_X509_CA_ACCUMULATOR_SCRATCH_CHUNK_ROWS_V1 == 128);
    assert!(LEAF_DYNAMIC_WORD_START_V1 == 16);
    assert!(LEAF_DYNAMIC_WORD_END_V1 == 38);
};
/// Stable proof-facing compact accumulator identity.
pub(crate) const ZK_X509_ACCUMULATOR_STARK_DESCRIPTOR_V1: &[u8] = b"zk-x509-ca-accumulator-stark-v1:dedicated-local-subproof-only:wire-envelope-X5C1+inner-X5C2:strict-version-adapter-claim-addresses-length-and-no-trailing-bytes:claim-envelope108-records*12+header14=1310bytes:inner-predeep-max984216:inner-deep52768:subproof-max1038294:single-log7-trace128:dedicated-lde-log14:compiled-max-air-degree3:haboeck-al-kindi-reduced-air-degree2:protocol3-trace-mask:haboeck-al-kindi-h-min=2*2*(4*n-deep+n-fri)+n-fri:trace-mask306-coefficients:min-fri-rate1over32:fri58-distinct-post-grinding20:binary-fri5-rounds-terminal512-degree15:independent-fp4-fri-mask-root-before-deep-batching:one-shared-deep-point-current+next:fp4-composition-lanes1:fixed-selector-aware-maximum-quotient-degree1425:composition-degree-chunks3:scratch-chunk-rows128:common-domain-lifting-forbidden:first-release-materializes-complete-local-lde:checked-native-lde-scratch-resident-and-work-ceilings:hash-rows13:serialized-root-spki-rows91:nonpadding104:zero-padding24:base695-11chunks:aux128-2chunks:fixed80:constraints1379:degree3:private-index12-and-siblings12:leaf-call16:nodes-calls17through28:source48words+digest8words:four-independent-sha-call-lanes:two-affine-factors-per-hash-row:leaf-dynamic-source-words16through38-serialized:reusable-eight-bit-byte-range:big-endian-word-accumulator:root-spki-channel=28+2*public-disclosures:endpoint-role-ca-accumulator4:governed-trust-anchor-role8:rfc-output-tuple-tag80:four-independent-rfc-output-lanes:dual-running-products-sha-source-and-rfc-consumer:all-four-terminal-families-algebraically-bound:typed-outer-binding=public-root+channel+ordered-sha13+rfc91:shared-X5S1-pre-aux-after-six-main-plus-one-ca-base-roots:public-governed-root-and-root-spki-channel:rand0.9-trycrypto-fixed64-reservoir-health-check-zeroize-poison-error-or-unwind:deterministic-preflight-before-entropy:producer-self-verifies:no-crl-accumulator";
const CA_PROOF_MAGIC_V1: [u8; 4] = *b"X5C1";
const CA_INNER_PROOF_MAGIC_V1: [u8; 4] = *b"X5C2";
const CA_ADAPTER_ID_V1: u16 = 5;
const CA_SECURITY_LANES_V1: usize = 1;
const CA_QUERY_COUNT_V1: usize = ZK_X509_FRI_QUERY_COUNT_V1 as usize;
const CA_BLOWUP_LOG2_V1: u8 = ZK_X509_CA_FRI_LDE_LOG2_V1 - ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1;
const CA_TERMINAL_LOG2_V1: u8 = ZK_X509_CA_FRI_TERMINAL_LOG2_V1;
const CA_TERMINAL_DEGREE_BOUND_V1: usize = ZK_X509_CA_FRI_TERMINAL_DEGREE_BOUND_V1 as usize;
const CA_COMPOSITION_DEGREE_CHUNKS_V1: usize = ZK_X509_CA_COMPOSITION_DEGREE_CHUNKS_V1 as usize;
const CA_MASK_DEGREE_V1: usize = ZK_X509_CA_TRACE_MASK_DEGREE_V1 as usize;
const CA_DEEP_FIELD_COUNT_V1: usize = 2
    * (ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1 + ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1)
    + CA_COMPOSITION_DEGREE_CHUNKS_V1;
const CA_DEEP_BYTES_V1: usize = CA_DEEP_FIELD_COUNT_V1 * core::mem::size_of::<[u64; 4]>();
const CA_INNER_MAXIMUM_PROOF_BYTES_V1: usize =
    ZK_X509_CA_PRE_DEEP_MAXIMUM_BYTES_V1 as usize + CA_DEEP_BYTES_V1;
const CA_CLAIM_FIELDS_V1: usize =
    2 * ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1 * ZK_X509_SHA_BUS_LANES_V1
        + ZK_X509_RFC5280_STARK_BUS_LANES_V1;
const CA_CLAIM_RECORD_BYTES_V1: usize = 4 + core::mem::size_of::<u64>();
const CA_PROOF_LENGTH_OFFSET_V1: usize =
    4 + 2 + 2 + 2 + CA_CLAIM_FIELDS_V1 * CA_CLAIM_RECORD_BYTES_V1;
const CA_PROOF_ENVELOPE_BYTES_V1: usize = CA_PROOF_LENGTH_OFFSET_V1 + 4;
/// Exact typed-claim envelope bytes, excluding the inner aggregate proof.
#[cfg(test)]
pub(crate) const ZK_X509_CA_ACCUMULATOR_CLAIM_ENVELOPE_BYTES_V1: usize = CA_PROOF_ENVELOPE_BYTES_V1;
/// Exact dedicated compact-CA DEEP payload bytes.
#[cfg(test)]
pub(crate) const ZK_X509_CA_ACCUMULATOR_DEEP_OPENING_BYTES_V1: usize = CA_DEEP_BYTES_V1;
/// Exact maximum inner X5C2 bytes including its DEEP payload.
#[cfg(test)]
pub(crate) const ZK_X509_CA_ACCUMULATOR_INNER_MAX_PROOF_BYTES_V1: usize =
    CA_INNER_MAXIMUM_PROOF_BYTES_V1;
/// Exact hard ceiling for the dedicated compact-CA proof envelope.
pub(crate) const ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1: usize =
    CA_PROOF_ENVELOPE_BYTES_V1 + CA_INNER_MAXIMUM_PROOF_BYTES_V1;
const CA_BASE_LEAF_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-x509:ca-accumulator:base-leaf:v1";
const CA_BASE_NODE_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-x509:ca-accumulator:base-node:v1";
const CA_AUX_LEAF_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-x509:ca-accumulator:aux-leaf:v1";
const CA_AUX_NODE_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-x509:ca-accumulator:aux-node:v1";
const CA_COMPOSITION_LEAF_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:ca-accumulator:composition-leaf:v1";
const CA_COMPOSITION_NODE_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:ca-accumulator:composition-node:v1";
const CA_FRI_LEAF_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-x509:ca-accumulator:fri-leaf:v1";
const CA_FRI_NODE_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-x509:ca-accumulator:fri-node:v1";
const CA_LAYOUT_LABEL_V1: &[u8] = b"iroha:privacy:zk-x509:ca-accumulator:aggregate-layout:v1";
const CA_BASE_ROOT_LABEL_V1: &[u8] = b"iroha:privacy:zk-x509:ca-accumulator:base-root:v1";
const CA_AUX_ROOT_LABEL_V1: &[u8] = b"iroha:privacy:zk-x509:ca-accumulator:aux-root:v1";
const CA_COMPOSITION_ROOT_LABEL_V1: &[u8] =
    b"iroha:privacy:zk-x509:ca-accumulator:composition-root:v1";
const CA_FRI_ROOT_LABEL_V1: &[u8] = b"iroha:privacy:zk-x509:ca-accumulator:fri-root:v1";
const CA_FRI_BETA_LABEL_V1: &[u8] = b"iroha:privacy:zk-x509:ca-accumulator:fri-beta:v1";
const CA_QUERY_SEED_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-x509:ca-accumulator:query-seed:v1";
const CA_RELATION_LAYOUT_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:ca-accumulator:relation-layout:v1";
const CA_PROFILE_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:ca-accumulator:profile-digest:v1";
const CA_PUBLIC_DIGEST_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-x509:ca-accumulator:public-digest:v1";
const CA_SCHEDULE_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:ca-accumulator:schedule-digest:v1";
const CA_REGISTRATION_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-x509:ca-accumulator:registration:v1";
const CA_TERMINAL_CLAIMS_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:ca-accumulator:terminal-claims:v1";
const CA_CONSTRAINT_ALPHA_LABEL_V1: &[u8] =
    b"iroha:privacy:zk-x509:ca-accumulator:constraint-alpha:v1";
const CA_DEEP_BASE_CURRENT_MIX_LABEL_V1: &[u8] =
    b"iroha:privacy:zk-x509:ca-accumulator:deep-base-current-mix:v1";
const CA_DEEP_BASE_NEXT_MIX_LABEL_V1: &[u8] =
    b"iroha:privacy:zk-x509:ca-accumulator:deep-base-next-mix:v1";
const CA_DEEP_AUX_CURRENT_MIX_LABEL_V1: &[u8] =
    b"iroha:privacy:zk-x509:ca-accumulator:deep-aux-current-mix:v1";
const CA_DEEP_AUX_NEXT_MIX_LABEL_V1: &[u8] =
    b"iroha:privacy:zk-x509:ca-accumulator:deep-aux-next-mix:v1";
const CA_DEEP_COMPOSITION_MIX_LABEL_V1: &[u8] =
    b"iroha:privacy:zk-x509:ca-accumulator:deep-composition-mix:v1";
const CA_GRINDING_NONCE_DOMAIN_V1: &[u8] =
    b"iroha:privacy:zk-x509:ca-accumulator:grinding-nonce:v1";
#[cfg(test)]
const CA_BINDING_DIGEST_DOMAIN_V1: &[u8] = b"iroha:privacy:zk-x509:ca-accumulator:proof-binding:v1";
const CA_AGGREGATE_PARAMETERS_V1: aggregate::AggregateStarkParametersV1 =
    aggregate::AggregateStarkParametersV1 {
        proof_magic: CA_INNER_PROOF_MAGIC_V1,
        proof_version: ZK_X509_PROOF_VERSION_V1,
        security_lanes: CA_SECURITY_LANES_V1,
        query_count: CA_QUERY_COUNT_V1,
        blowup_log2: CA_BLOWUP_LOG2_V1,
        terminal_log2: CA_TERMINAL_LOG2_V1,
        terminal_degree_bound: CA_TERMINAL_DEGREE_BOUND_V1,
        composition_degree_chunks: CA_COMPOSITION_DEGREE_CHUNKS_V1,
        minimum_trace_log2: ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
        maximum_trace_log2: ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
        maximum_trace_groups: 1,
        maximum_segment_instances: ZK_X509_CA_ACCUMULATOR_CHUNKS_V1,
        maximum_base_columns_per_instance: 64,
        maximum_aux_columns_per_instance: 64,
        maximum_proof_bytes: CA_INNER_MAXIMUM_PROOF_BYTES_V1,
    };
const CA_AGGREGATE_DOMAINS_V1: aggregate::AggregateStarkDomainsV1 =
    aggregate::AggregateStarkDomainsV1 {
        base_leaf: CA_BASE_LEAF_DOMAIN_V1,
        base_node: CA_BASE_NODE_DOMAIN_V1,
        aux_leaf: CA_AUX_LEAF_DOMAIN_V1,
        aux_node: CA_AUX_NODE_DOMAIN_V1,
        composition_leaf: CA_COMPOSITION_LEAF_DOMAIN_V1,
        composition_node: CA_COMPOSITION_NODE_DOMAIN_V1,
        fri_leaf: CA_FRI_LEAF_DOMAIN_V1,
        fri_node: CA_FRI_NODE_DOMAIN_V1,
        layout_label: CA_LAYOUT_LABEL_V1,
        base_root_label: CA_BASE_ROOT_LABEL_V1,
        aux_root_label: CA_AUX_ROOT_LABEL_V1,
        composition_root_label: CA_COMPOSITION_ROOT_LABEL_V1,
        fri_root_label: CA_FRI_ROOT_LABEL_V1,
        fri_beta_label: CA_FRI_BETA_LABEL_V1,
        query_seed: CA_QUERY_SEED_DOMAIN_V1,
    };
const _: () = {
    assert!(CA_QUERY_COUNT_V1 == 58);
    assert!(CA_BLOWUP_LOG2_V1 == 7);
    assert!(CA_TERMINAL_LOG2_V1 == 9);
    assert!(ZK_X509_CA_FRI_ROUNDS_V1 == ZK_X509_CA_FRI_LDE_LOG2_V1 - CA_TERMINAL_LOG2_V1);
    assert!(CA_COMPOSITION_DEGREE_CHUNKS_V1 == 3);
    assert!(CA_MASK_DEGREE_V1 == 305);
    assert!(CA_DEEP_BYTES_V1 == 52_768);
    assert!(CA_CLAIM_FIELDS_V1 == 108);
    assert!(CA_PROOF_ENVELOPE_BYTES_V1 == 1_310);
    assert!(CA_INNER_MAXIMUM_PROOF_BYTES_V1 == 1_036_984);
    assert!(ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1 == 1_038_294);
    assert!(ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1 < ZK_X509_MAX_PROOF_BYTES_V1 as usize);
};
/// Exact caller-supplied resource shape admitted by the first release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509CaAccumulatorResourceRequestV1 {
    /// Native trace logarithm.
    pub(crate) trace_log2: u8,
    /// Dedicated local LDE logarithm.
    pub(crate) lde_log2: u8,
    /// Haböck--Al Kindi reduced AIR degree `d = d_AIR - 1`.
    pub(crate) reduced_air_degree: usize,
    /// Number of DEEP samples selected by the outer proof.
    pub(crate) deep_query_count: usize,
    /// Number of FRI queries selected by the outer proof.
    pub(crate) fri_query_count: usize,
    /// Degree of the trace zero-knowledge mask.
    pub(crate) mask_degree: usize,
    /// Required LDE-to-FRI-degree ratio.
    pub(crate) fri_rate_denominator: usize,
    /// Exact base width.
    pub(crate) base_width: usize,
    /// Exact auxiliary width.
    pub(crate) aux_width: usize,
    /// Exact verifier-preprocessed width.
    pub(crate) fixed_width: usize,
    /// Exact authenticated scratch block height.
    pub(crate) scratch_chunk_rows: usize,
    /// Exact number of Fp4 composition lanes.
    pub(crate) composition_extension_lanes: usize,
}
/// Checked exact resource census for one compact-CA local subproof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509CaAccumulatorResourceEnvelopeV1 {
    /// Exact zero-knowledge mask coefficient count.
    pub(crate) mask_coefficients: usize,
    /// Maximum degree of one masked trace column.
    pub(crate) maximum_masked_trace_degree: usize,
    /// FRI degree cap implied by the local domain and governed rate.
    pub(crate) fri_degree_cap: usize,
    /// Maximum fixed-selector-aware cubic quotient before coefficient
    /// chunking.
    pub(crate) maximum_quotient_degree: usize,
    /// Minimum safe local LDE rows before power-of-two rounding.
    pub(crate) minimum_safe_lde_rows: usize,
    /// Native base, aux, and fixed field cells retained by the adapter.
    pub(crate) native_material_field_cells: usize,
    /// Exact native field payload bytes.
    pub(crate) native_material_bytes: usize,
    /// Base and aux local-LDE field evaluations.
    pub(crate) committed_lde_field_evaluations: usize,
    /// Fixed local-LDE field evaluations.
    pub(crate) fixed_lde_field_evaluations: usize,
    /// Exact base, aux, and fixed local-LDE payload bytes.
    pub(crate) total_local_lde_bytes: usize,
    /// Exact encrypted scratch bytes including one tag per matrix record.
    pub(crate) encrypted_scratch_bytes: usize,
    /// Maximum current/next base, aux, and fixed decrypted block payload.
    pub(crate) current_next_block_bytes: usize,
    /// Maximum native plus local-LDE column payload during streaming.
    pub(crate) streamed_column_bytes: usize,
    /// Exact retained trace-mask payload.
    pub(crate) trace_mask_bytes: usize,
    /// Exact base-field residue evaluations in one Fp4 composition lane.
    pub(crate) composition_residue_evaluations: usize,
    /// Exact base-field component evaluations across the Fp4 lane.
    pub(crate) composition_component_evaluations: usize,
    /// Exact radix-2 butterflies for all base, aux, and fixed local LDEs.
    pub(crate) lde_butterflies: usize,
    /// Conservative exact CA-adapter resident field-payload census.
    pub(crate) adapter_resident_payload_bytes: usize,
}
/// Verifier-bound compact accumulator terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509CaAccumulatorStarkPublicV1 {
    /// Governed compact-tree root as exact byte fields.
    pub(crate) governed_root: [F; 32],
    /// Verifier-derived final DER I/O channel carrying the exact root SPKI.
    pub(crate) root_spki_channel: F,
}
/// Per-call products exposed to the aggregate SHA adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509CaAccumulatorCallTerminalV1 {
    /// Canonical global SHA call.
    pub(crate) call: u8,
    /// Leaf or node role.
    pub(crate) role: ZkX509ShaCallRoleV1,
    /// Product over 48 padded source words.
    pub(crate) source_products: [F; ZK_X509_SHA_BUS_LANES_V1],
    /// Product over eight digest words.
    pub(crate) digest_products: [F; ZK_X509_SHA_BUS_LANES_V1],
}
/// Exact DER-output consumer terminal for the private root SPKI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509CaAccumulatorIoTerminalV1 {
    /// Verifier-derived canonical channel.
    pub(crate) channel: u32,
    /// Exactly 91 byte-consumer events.
    pub(crate) event_count: u16,
    /// Product over `(role, channel, endpoint, offset, byte, read)` tuples.
    pub(crate) consumer_products: [F; ZK_X509_RFC5280_STARK_BUS_LANES_V1],
}
/// Terminal claims algebraically bound inside the accumulator quotient.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509CaAccumulatorStarkTerminalClaimsV1 {
    /// One source terminal per leaf/node SHA call.
    pub(crate) source_products:
        [[F; ZK_X509_SHA_BUS_LANES_V1]; ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1],
    /// One digest terminal per leaf/node SHA call.
    pub(crate) digest_products:
        [[F; ZK_X509_SHA_BUS_LANES_V1]; ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1],
    /// Exact root-SPKI consumer terminal.
    pub(crate) root_spki_consumer_products: [F; ZK_X509_RFC5280_STARK_BUS_LANES_V1],
}
/// Column-major material consumed by aggregate commitments.
#[cfg(any(test, feature = "privacy-release-evidence"))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509CaAccumulatorStarkMaterialV1 {
    /// Challenge-independent witness columns.
    pub(crate) base_columns: Vec<Vec<F>>,
    /// Four-lane per-call product columns.
    pub(crate) aux_columns: Vec<Vec<F>>,
    /// Verifier-preprocessed selectors and frame constants.
    pub(crate) fixed_columns: Vec<Vec<F>>,
    /// Exact thirteen call terminals.
    pub(crate) terminals:
        [ZkX509CaAccumulatorCallTerminalV1; ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1],
    /// Exact 91-byte root-SPKI I/O consumer terminal.
    pub(crate) root_spki_terminal: ZkX509CaAccumulatorIoTerminalV1,
}
/// Exact cross-subproof binding carried by the outer X5S1 envelope.
///
/// The outer verifier compares this value with the verifier-derived public
/// statement, the shared SHA subproof terminals, and the strict-DER RFC output
/// terminal.  Fixed-size SHA storage makes omission impossible after decode;
/// the slice validator below rejects omission and excess before conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509CaAccumulatorSubproofBindingV1 {
    /// Verifier-bound governed root and derived root-SPKI channel.
    pub(crate) public: ZkX509CaAccumulatorStarkPublicV1,
    /// Exact ordered leaf call followed by node levels zero through eleven.
    pub(crate) sha_terminals:
        [ZkX509CaAccumulatorCallTerminalV1; ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1],
    /// Exact 91-event strict-DER consumer terminal.
    pub(crate) root_spki_terminal: ZkX509CaAccumulatorIoTerminalV1,
}
/// Numeric adapter construction or evaluation failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum ZkX509AccumulatorStarkErrorV1 {
    /// The compact native witness is invalid.
    #[error("zk-X509 compact CA STARK witness is invalid")]
    Witness,
    /// The verifier-owned SHA schedule or challenge family is invalid.
    #[error("zk-X509 compact CA STARK SHA call bus is invalid")]
    CallBus,
    /// The verifier-owned DER root-SPKI I/O bus is invalid.
    #[error("zk-X509 compact CA STARK root-SPKI I/O bus is invalid")]
    IoBus,
    /// An opened row has the wrong exact width.
    #[error("zk-X509 compact CA STARK row shape is invalid")]
    Shape,
    /// Checked allocation or conversion exceeded the fixed envelope.
    #[error("zk-X509 compact CA STARK resource envelope is exceeded")]
    Resource,
}
/// Dedicated compact-CA proof construction, wire, or verification failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum ZkX509CaAccumulatorProofErrorV1 {
    /// The public root, verifier-owned schedule, or native witness is invalid.
    #[error("zk-X509 compact CA proof statement or witness is invalid")]
    InvalidStatementOrWitness,
    /// The exact proof envelope or inner proof is malformed.
    #[error("zk-X509 compact CA proof wire is malformed")]
    MalformedProof,
    /// The proof exceeds the sole release ceiling.
    #[error("zk-X509 compact CA proof exceeds its byte ceiling")]
    ProofTooLarge,
    /// A field was not a canonical Goldilocks residue.
    #[error("zk-X509 compact CA proof contains a non-canonical field")]
    NonCanonicalField,
    /// A committed trace or Merkle frontier is invalid.
    #[error("zk-X509 compact CA proof trace opening is invalid")]
    TraceOpening,
    /// An AIR, terminal-claim, composition, or DEEP identity is invalid.
    #[error("zk-X509 compact CA proof constraint opening is invalid")]
    ConstraintOpening,
    /// A FRI commitment, opening, fold, or terminal degree is invalid.
    #[error("zk-X509 compact CA proof FRI opening is invalid")]
    FriOpening,
    /// Fiat-Shamir replay, grinding, or query derivation failed.
    #[error("zk-X509 compact CA proof transcript is invalid")]
    TranscriptMismatch,
    /// Prover entropy was unavailable.
    #[error("zk-X509 compact CA prover randomness is unavailable")]
    RandomnessUnavailable,
    /// Prover entropy failed the release health policy.
    #[cfg(any(test, feature = "privacy-release-evidence"))]
    #[error("zk-X509 compact CA prover randomness failed its health check")]
    RandomnessUnhealthy,
    /// Checked arithmetic or allocation exceeded the fixed local envelope.
    #[error("zk-X509 compact CA proof resource envelope is exceeded")]
    Resource,
    /// The producer-generated proof failed independent verification.
    #[cfg(any(test, feature = "privacy-release-evidence"))]
    #[error("zk-X509 compact CA prover self-check failed")]
    ProverSelfCheckFailed,
}
impl From<ZkX509AccumulatorAirErrorV1> for ZkX509AccumulatorStarkErrorV1 {
    fn from(error: ZkX509AccumulatorAirErrorV1) -> Self {
        match error {
            ZkX509AccumulatorAirErrorV1::Resource => Self::Resource,
            _ => Self::Witness,
        }
    }
}
impl From<ZkX509MerkleErrorV1> for ZkX509AccumulatorStarkErrorV1 {
    fn from(_: ZkX509MerkleErrorV1) -> Self {
        Self::Witness
    }
}
impl From<ZkX509AccumulatorStarkErrorV1> for ZkX509CaAccumulatorProofErrorV1 {
    fn from(error: ZkX509AccumulatorStarkErrorV1) -> Self {
        match error {
            ZkX509AccumulatorStarkErrorV1::Resource => Self::Resource,
            ZkX509AccumulatorStarkErrorV1::Witness
            | ZkX509AccumulatorStarkErrorV1::CallBus
            | ZkX509AccumulatorStarkErrorV1::IoBus
            | ZkX509AccumulatorStarkErrorV1::Shape => Self::InvalidStatementOrWitness,
        }
    }
}
fn map_aggregate_proof_error_v1(error: AggregateStarkErrorV1) -> ZkX509CaAccumulatorProofErrorV1 {
    match error {
        AggregateStarkErrorV1::InvalidLayout
        | AggregateStarkErrorV1::InvalidProofShape
        | AggregateStarkErrorV1::AllocationFailure
        | AggregateStarkErrorV1::InternalInvariant => ZkX509CaAccumulatorProofErrorV1::Resource,
        AggregateStarkErrorV1::MalformedProof => ZkX509CaAccumulatorProofErrorV1::MalformedProof,
        AggregateStarkErrorV1::ProofTooLarge => ZkX509CaAccumulatorProofErrorV1::ProofTooLarge,
        AggregateStarkErrorV1::NonCanonicalField => {
            ZkX509CaAccumulatorProofErrorV1::NonCanonicalField
        }
        AggregateStarkErrorV1::TraceOpening => ZkX509CaAccumulatorProofErrorV1::TraceOpening,
        AggregateStarkErrorV1::ConstraintOpening | AggregateStarkErrorV1::DeepOpening => {
            ZkX509CaAccumulatorProofErrorV1::ConstraintOpening
        }
        AggregateStarkErrorV1::FriOpening | AggregateStarkErrorV1::FriDegree => {
            ZkX509CaAccumulatorProofErrorV1::FriOpening
        }
        AggregateStarkErrorV1::TranscriptMismatch => {
            ZkX509CaAccumulatorProofErrorV1::TranscriptMismatch
        }
        AggregateStarkErrorV1::RandomnessUnavailable => {
            ZkX509CaAccumulatorProofErrorV1::RandomnessUnavailable
        }
    }
}
fn map_credential_pre_aux_error_v1(
    error: ZkX509CredentialPreAuxErrorV1,
) -> ZkX509CaAccumulatorProofErrorV1 {
    match error {
        ZkX509CredentialPreAuxErrorV1::Resource => ZkX509CaAccumulatorProofErrorV1::Resource,
        ZkX509CredentialPreAuxErrorV1::Transcript | ZkX509CredentialPreAuxErrorV1::Challenge => {
            ZkX509CaAccumulatorProofErrorV1::TranscriptMismatch
        }
    }
}
fn map_transparent_proof_error_v1(
    error: TransparentStarkErrorV1,
) -> ZkX509CaAccumulatorProofErrorV1 {
    match error {
        TransparentStarkErrorV1::RandomnessUnavailable => {
            ZkX509CaAccumulatorProofErrorV1::RandomnessUnavailable
        }
        TransparentStarkErrorV1::AllocationFailure => ZkX509CaAccumulatorProofErrorV1::Resource,
        TransparentStarkErrorV1::NonCanonicalField => {
            ZkX509CaAccumulatorProofErrorV1::NonCanonicalField
        }
        TransparentStarkErrorV1::InvalidMerkleShape => {
            ZkX509CaAccumulatorProofErrorV1::TraceOpening
        }
        TransparentStarkErrorV1::FriDegree => ZkX509CaAccumulatorProofErrorV1::FriOpening,
        TransparentStarkErrorV1::MalformedProof => ZkX509CaAccumulatorProofErrorV1::MalformedProof,
        TransparentStarkErrorV1::InvalidGrinding
        | TransparentStarkErrorV1::ChallengeSamplingExhausted
        | TransparentStarkErrorV1::QuerySamplingExhausted => {
            ZkX509CaAccumulatorProofErrorV1::TranscriptMismatch
        }
        _ => ZkX509CaAccumulatorProofErrorV1::ConstraintOpening,
    }
}
/// Derive the minimum safe local request for the compiled cubic AIR and exact
/// outer DEEP/FRI parameters.
pub(crate) fn ca_accumulator_resource_request_v1(
    reduced_air_degree: usize,
    deep_query_count: usize,
    fri_query_count: usize,
) -> Result<ZkX509CaAccumulatorResourceRequestV1, ZkX509AccumulatorStarkErrorV1> {
    if reduced_air_degree != ZK_X509_CA_ACCUMULATOR_REDUCED_AIR_DEGREE_V1 {
        return Err(ZkX509AccumulatorStarkErrorV1::Resource);
    }
    let geometry = transparent_stark_zk_mask_geometry_v1(
        reduced_air_degree,
        ZK_X509_CA_ACCUMULATOR_EXTENSION_COMPONENTS_V1,
        deep_query_count,
        fri_query_count,
    )
    .map_err(|_| ZkX509AccumulatorStarkErrorV1::Resource)?;
    let mask_degree = geometry.minimum_mask_degree;
    let maximum_masked_trace_degree = ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1
        .checked_add(mask_degree)
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let minimum_safe_lde_rows = maximum_masked_trace_degree
        .checked_add(1)
        .and_then(|coefficients| {
            coefficients.checked_mul(ZK_X509_CA_ACCUMULATOR_FRI_RATE_DENOMINATOR_V1)
        })
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let lde_rows = minimum_safe_lde_rows
        .checked_next_power_of_two()
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let lde_log2 =
        u8::try_from(lde_rows.ilog2()).map_err(|_| ZkX509AccumulatorStarkErrorV1::Resource)?;
    Ok(ZkX509CaAccumulatorResourceRequestV1 {
        trace_log2: ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
        lde_log2,
        reduced_air_degree,
        deep_query_count,
        fri_query_count,
        mask_degree,
        fri_rate_denominator: ZK_X509_CA_ACCUMULATOR_FRI_RATE_DENOMINATOR_V1,
        base_width: ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1,
        aux_width: ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1,
        fixed_width: ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1,
        scratch_chunk_rows: ZK_X509_CA_ACCUMULATOR_SCRATCH_CHUNK_ROWS_V1,
        composition_extension_lanes: ZK_X509_CA_ACCUMULATOR_COMPOSITION_EXTENSION_LANES_V1,
    })
}
/// Validate a parameterized local resource shape and return its checked census.
///
/// The outer profile binds the compiled `d_AIR = 3`, exact DEEP count, FRI
/// query count, and mask degree. Passing a quadratic substitution, a weaker
/// mask, a lower-rate domain, the full profile's common LDE, or a noncanonical
/// scratch block fails before allocation.
pub(crate) fn checked_ca_accumulator_resource_envelope_v1(
    request: ZkX509CaAccumulatorResourceRequestV1,
) -> Result<ZkX509CaAccumulatorResourceEnvelopeV1, ZkX509AccumulatorStarkErrorV1> {
    if request.trace_log2 != ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1
        || request.base_width != ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1
        || request.aux_width != ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1
        || request.fixed_width != ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1
        || request.scratch_chunk_rows != ZK_X509_CA_ACCUMULATOR_SCRATCH_CHUNK_ROWS_V1
        || request.composition_extension_lanes
            != ZK_X509_CA_ACCUMULATOR_COMPOSITION_EXTENSION_LANES_V1
        || request.fri_rate_denominator != ZK_X509_CA_ACCUMULATOR_FRI_RATE_DENOMINATOR_V1
        || request.reduced_air_degree != ZK_X509_CA_ACCUMULATOR_REDUCED_AIR_DEGREE_V1
        || request.scratch_chunk_rows == 0
        || !request.scratch_chunk_rows.is_power_of_two()
    {
        return Err(ZkX509AccumulatorStarkErrorV1::Resource);
    }
    let mask_coefficients = request
        .mask_degree
        .checked_add(1)
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let geometry = transparent_stark_zk_mask_geometry_v1(
        request.reduced_air_degree,
        ZK_X509_CA_ACCUMULATOR_EXTENSION_COMPONENTS_V1,
        request.deep_query_count,
        request.fri_query_count,
    )
    .map_err(|_| ZkX509AccumulatorStarkErrorV1::Resource)?;
    if mask_coefficients != geometry.minimum_mask_coefficients
        || request.mask_degree != geometry.minimum_mask_degree
    {
        return Err(ZkX509AccumulatorStarkErrorV1::Resource);
    }
    let trace_rows = 1_usize
        .checked_shl(u32::from(request.trace_log2))
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let lde_rows = 1_usize
        .checked_shl(u32::from(request.lde_log2))
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let maximum_masked_trace_degree = trace_rows
        .checked_add(request.mask_degree)
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let minimum_safe_lde_rows = maximum_masked_trace_degree
        .checked_add(1)
        .and_then(|coefficients| coefficients.checked_mul(request.fri_rate_denominator))
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let exact_safe_lde_rows = minimum_safe_lde_rows
        .checked_next_power_of_two()
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let fri_degree_cap = lde_rows
        .checked_div(request.fri_rate_denominator)
        .and_then(|capacity| capacity.checked_sub(1))
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    // The two dynamic SHA source factors are affine in masked trace values,
    // but each is selected by one verifier-fixed polynomial of degree at most
    // `trace_rows - 1`. Account for both selectors explicitly: treating fixed
    // columns as degree-zero symbols is valid for the symbolic AIR inventory,
    // not for the actual univariate quotient committed by FRI.
    let fixed_selector_degree = trace_rows
        .checked_sub(1)
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let maximum_quotient_degree = usize::from(ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1)
        .checked_mul(maximum_masked_trace_degree)
        .and_then(|degree| {
            fixed_selector_degree
                .checked_mul(2)
                .and_then(|selectors| degree.checked_add(selectors))
        })
        .and_then(|degree| degree.checked_sub(trace_rows))
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let quotient_chunk_capacity = fri_degree_cap
        .checked_add(1)
        .and_then(|coefficients| coefficients.checked_mul(COMPOSITION_DEGREE_CHUNKS_V1))
        .and_then(|coefficients| coefficients.checked_sub(1))
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    if trace_rows != ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1
        || lde_rows != exact_safe_lde_rows
        || maximum_masked_trace_degree > fri_degree_cap
        || maximum_quotient_degree > quotient_chunk_capacity
        || request.scratch_chunk_rows > lde_rows
        || lde_rows % request.scratch_chunk_rows != 0
    {
        return Err(ZkX509AccumulatorStarkErrorV1::Resource);
    }
    let committed_width = request
        .base_width
        .checked_add(request.aux_width)
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let total_width = committed_width
        .checked_add(request.fixed_width)
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let native_material_field_cells = total_width
        .checked_mul(trace_rows)
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let native_material_bytes = native_material_field_cells
        .checked_mul(FIELD_BYTES_V1)
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let committed_lde_field_evaluations = committed_width
        .checked_mul(lde_rows)
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let fixed_lde_field_evaluations = request
        .fixed_width
        .checked_mul(lde_rows)
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let total_local_lde_bytes = committed_lde_field_evaluations
        .checked_add(fixed_lde_field_evaluations)
        .and_then(|cells| cells.checked_mul(FIELD_BYTES_V1))
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let chunks_per_column = lde_rows / request.scratch_chunk_rows;
    let scratch_record_bytes = request
        .scratch_chunk_rows
        .checked_mul(FIELD_BYTES_V1)
        .and_then(|bytes| bytes.checked_add(SCRATCH_TAG_BYTES_V1))
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let encrypted_scratch_bytes = total_width
        .checked_mul(chunks_per_column)
        .and_then(|records| records.checked_mul(scratch_record_bytes))
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let current_next_block_bytes = total_width
        .checked_mul(request.scratch_chunk_rows)
        .and_then(|cells| cells.checked_mul(2))
        .and_then(|cells| cells.checked_mul(FIELD_BYTES_V1))
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let streamed_column_bytes = trace_rows
        .checked_add(lde_rows)
        .and_then(|cells| cells.checked_mul(FIELD_BYTES_V1))
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let trace_mask_bytes = committed_width
        .checked_mul(mask_coefficients)
        .and_then(|cells| cells.checked_mul(FIELD_BYTES_V1))
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let composition_residue_evaluations = ZK_X509_CA_ACCUMULATOR_CONSTRAINT_COUNT_V1
        .checked_mul(lde_rows)
        .and_then(|work| work.checked_mul(request.composition_extension_lanes))
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let composition_component_evaluations = composition_residue_evaluations
        .checked_mul(ZK_X509_CA_ACCUMULATOR_EXTENSION_COMPONENTS_V1)
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let per_column_butterflies = trace_rows
        .checked_div(2)
        .and_then(|half| half.checked_mul(usize::from(request.trace_log2)))
        .and_then(|native| {
            lde_rows
                .checked_div(2)
                .and_then(|half| half.checked_mul(usize::from(request.lde_log2)))
                .and_then(|lde| native.checked_add(lde))
        })
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let lde_butterflies = total_width
        .checked_mul(per_column_butterflies)
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let residue_vector_bytes = ZK_X509_CA_ACCUMULATOR_CONSTRAINT_COUNT_V1
        .checked_mul(FIELD_BYTES_V1)
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let composition_chunk_bytes = request
        .composition_extension_lanes
        .checked_mul(COMPOSITION_DEGREE_CHUNKS_V1)
        .and_then(|cells| cells.checked_mul(lde_rows))
        .and_then(|cells| cells.checked_mul(ZK_X509_CA_ACCUMULATOR_EXTENSION_COMPONENTS_V1))
        .and_then(|cells| cells.checked_mul(FIELD_BYTES_V1))
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let adapter_resident_payload_bytes = native_material_bytes
        .checked_add(total_local_lde_bytes)
        .and_then(|bytes| bytes.checked_add(current_next_block_bytes))
        .and_then(|bytes| bytes.checked_add(streamed_column_bytes))
        .and_then(|bytes| bytes.checked_add(trace_mask_bytes))
        .and_then(|bytes| bytes.checked_add(residue_vector_bytes))
        .and_then(|bytes| bytes.checked_add(composition_chunk_bytes))
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    let envelope = ZkX509CaAccumulatorResourceEnvelopeV1 {
        mask_coefficients,
        maximum_masked_trace_degree,
        fri_degree_cap,
        maximum_quotient_degree,
        minimum_safe_lde_rows,
        native_material_field_cells,
        native_material_bytes,
        committed_lde_field_evaluations,
        fixed_lde_field_evaluations,
        total_local_lde_bytes,
        encrypted_scratch_bytes,
        current_next_block_bytes,
        streamed_column_bytes,
        trace_mask_bytes,
        composition_residue_evaluations,
        composition_component_evaluations,
        lde_butterflies,
        adapter_resident_payload_bytes,
    };
    if envelope.native_material_bytes > ZK_X509_CA_ACCUMULATOR_MAX_NATIVE_MATERIAL_BYTES_V1
        || envelope.total_local_lde_bytes > ZK_X509_CA_ACCUMULATOR_MAX_LOCAL_LDE_BYTES_V1
        || envelope.encrypted_scratch_bytes > ZK_X509_CA_ACCUMULATOR_MAX_LOCAL_SCRATCH_BYTES_V1
        || envelope.adapter_resident_payload_bytes
            > ZK_X509_CA_ACCUMULATOR_MAX_ADAPTER_RESIDENT_BYTES_V1
        || envelope.lde_butterflies > ZK_X509_CA_ACCUMULATOR_MAX_LDE_BUTTERFLIES_V1
        || envelope.composition_component_evaluations
            > ZK_X509_CA_ACCUMULATOR_MAX_COMPOSITION_COMPONENT_EVALUATIONS_V1
    {
        return Err(ZkX509AccumulatorStarkErrorV1::Resource);
    }
    Ok(envelope)
}
/// Derive the canonical root-SPKI I/O channel from public disclosure shape.
pub(crate) fn ca_accumulator_root_spki_channel_v1(
    schedule: &ZkX509ShaCallScheduleV1,
) -> Result<u32, ZkX509AccumulatorStarkErrorV1> {
    let disclosures = u32::try_from(schedule.shape().disclosed_attributes)
        .map_err(|_| ZkX509AccumulatorStarkErrorV1::Resource)?;
    ZK_X509_CA_ACCUMULATOR_ROOT_SPKI_BASE_CHANNEL_V1
        .checked_add(
            disclosures
                .checked_mul(2)
                .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?,
        )
        .filter(|_| schedule.shape().disclosed_attributes <= 4)
        .ok_or(ZkX509AccumulatorStarkErrorV1::CallBus)
}
/// Derive verifier terminals from a validated trace and public schedule.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn ca_accumulator_stark_public_v1(
    trace: &ZkX509CaAccumulatorTraceV1,
    schedule: &ZkX509ShaCallScheduleV1,
) -> Result<ZkX509CaAccumulatorStarkPublicV1, ZkX509AccumulatorStarkErrorV1> {
    Ok(ZkX509CaAccumulatorStarkPublicV1 {
        governed_root: trace.statement.governed_root.map(|byte| F(u64::from(byte))),
        root_spki_channel: F(u64::from(ca_accumulator_root_spki_channel_v1(schedule)?)),
    })
}
/// Compile one verifier-preprocessed fixed row.
pub(crate) fn compile_ca_accumulator_fixed_row_v1(
    index: usize,
) -> Result<[F; ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1], ZkX509AccumulatorStarkErrorV1> {
    let location = ca_accumulator_fixed_row_v1(index)?;
    let mut fixed = [F::ZERO; ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1];
    fixed[FIX_ACTIVE] = F(u64::from(location.sha_active()));
    fixed[FIX_LEAF] = F(u64::from(matches!(
        location.kind,
        ZkX509CaAccumulatorRowKindV1::Leaf
    )));
    fixed[FIX_NODE] = F(u64::from(matches!(
        location.kind,
        ZkX509CaAccumulatorRowKindV1::Node(_)
    )));
    fixed[FIX_PADDING] = F(u64::from(matches!(
        location.kind,
        ZkX509CaAccumulatorRowKindV1::Padding
    )));
    fixed[FIX_TRANSITION] = F(u64::from(location.sha_transition()));
    fixed[FIX_LAST] = F(u64::from(matches!(
        location.kind,
        ZkX509CaAccumulatorRowKindV1::Node(level)
            if usize::from(level) + 1 == ZK_X509_CA_COMPACT_TREE_DEPTH_V1
    )));
    let source_constants = match location.kind {
        ZkX509CaAccumulatorRowKindV1::Leaf => {
            fixed[FIX_CALL] = F(ZK_X509_SHA_CA_LEAF_CALL_V1 as u64);
            fixed[FIX_ROLE] = F(u64::from(ZkX509ShaCallRoleV1::CaLeaf.role_code()));
            fixed[FIX_SLOT] = F::ZERO;
            padded_source_words_v1(&ca_leaf_preimage_v1(&[0_u8; ZK_X509_CA_SPKI_DER_BYTES_V1])?)?
        }
        ZkX509CaAccumulatorRowKindV1::Node(level) => {
            fixed[FIX_LEVEL] = F(u64::from(level));
            fixed[FIX_CALL] = F(u64::try_from(
                ZK_X509_SHA_CA_NODE_CALL_START_V1 + usize::from(level),
            )
            .map_err(|_| ZkX509AccumulatorStarkErrorV1::Resource)?);
            let role = ZkX509ShaCallRoleV1::CaNode(level);
            fixed[FIX_ROLE] = F(u64::from(role.role_code()));
            fixed[FIX_SLOT] = F(u64::from(level));
            fixed[FIX_INDEX_SELECTORS_START + usize::from(level)] = F::ONE;
            padded_source_words_v1(&ca_node_preimage_v1(
                usize::from(level),
                &[0; 32],
                &[0; 32],
            )?)?
        }
        ZkX509CaAccumulatorRowKindV1::RootSpkiByte(offset) => {
            let offset = usize::from(offset);
            let message_offset = LEAF_DYNAMIC_OFFSET_V1
                .checked_add(offset)
                .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
            let word_index = message_offset / 4;
            fixed[FIX_IO_ACTIVE] = F::ONE;
            fixed[FIX_IO_FIRST] = F(u64::from(offset == 0));
            fixed[FIX_IO_LAST] = F(u64::from(offset + 1 == ZK_X509_CA_ACCUMULATOR_IO_ROWS_V1));
            fixed[FIX_IO_TRANSITION] = F(u64::from(location.io_transition()));
            fixed[FIX_IO_SAME_WORD_TO_NEXT] = F(u64::from(
                location.io_transition() && (message_offset + 1) % 4 != 0,
            ));
            fixed[FIX_IO_WORD_END] = F(u64::from(message_offset % 4 == 3));
            fixed[FIX_IO_OFFSET] =
                F(u64::try_from(offset).map_err(|_| ZkX509AccumulatorStarkErrorV1::Resource)?);
            fixed[FIX_IO_WORD_INDEX] =
                F(u64::try_from(word_index)
                    .map_err(|_| ZkX509AccumulatorStarkErrorV1::Resource)?);
            if location.io_transition() {
                let next_message_offset = message_offset
                    .checked_add(1)
                    .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
                fixed[FIX_IO_NEXT_WORD_END] = F(u64::from(next_message_offset % 4 == 3));
                fixed[FIX_IO_NEXT_WORD_INDEX] = F(u64::try_from(next_message_offset / 4)
                    .map_err(|_| ZkX509AccumulatorStarkErrorV1::Resource)?);
            }
            [0_u32; SOURCE_WORDS_V1]
        }
        ZkX509CaAccumulatorRowKindV1::Padding => [0_u32; SOURCE_WORDS_V1],
    };
    for (target, word) in fixed
        [FIX_SOURCE_CONSTANTS_START..FIX_SOURCE_CONSTANTS_START + SOURCE_WORDS_V1]
        .iter_mut()
        .zip(source_constants)
    {
        *target = F(u64::from(word));
    }
    Ok(fixed)
}
/// Compile all verifier-preprocessed fixed columns.
pub(crate) fn compile_ca_accumulator_fixed_columns_v1()
-> Result<Vec<Vec<F>>, ZkX509AccumulatorStarkErrorV1> {
    let mut columns = allocate_columns_v1(
        ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1,
        ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1,
    )?;
    for index in 0..ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1 {
        append_array_row_v1(&mut columns, &compile_ca_accumulator_fixed_row_v1(index)?)?;
    }
    Ok(columns)
}
/// Build exact base, auxiliary, fixed, and terminal material.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn build_ca_accumulator_stark_material_v1(
    trace: &ZkX509CaAccumulatorTraceV1,
    schedule: &ZkX509ShaCallScheduleV1,
    sha_challenges: ZkX509ShaCallBusChallengesV1,
    io_challenges: ZkX509Rfc5280StarkChallengesV1,
) -> Result<ZkX509CaAccumulatorStarkMaterialV1, ZkX509AccumulatorStarkErrorV1> {
    let native_material_bytes = ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1
        .checked_add(ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1)
        .and_then(|width| width.checked_add(ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1))
        .and_then(|width| width.checked_mul(ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1))
        .and_then(|cells| cells.checked_mul(FIELD_BYTES_V1))
        .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
    if native_material_bytes > ZK_X509_CA_ACCUMULATOR_MAX_NATIVE_MATERIAL_BYTES_V1 {
        return Err(ZkX509AccumulatorStarkErrorV1::Resource);
    }
    trace.validate()?;
    sha_challenges
        .validate()
        .map_err(|_| ZkX509AccumulatorStarkErrorV1::CallBus)?;
    io_challenges
        .validate()
        .map_err(|_| ZkX509AccumulatorStarkErrorV1::IoBus)?;
    let public = ca_accumulator_stark_public_v1(trace, schedule)?;
    let mut base_columns = allocate_columns_v1(
        ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1,
        ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1,
    )?;
    let mut aux_columns = allocate_columns_v1(
        ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1,
        ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1,
    )?;
    let fixed_columns = compile_ca_accumulator_fixed_columns_v1()?;
    let mut terminals = Vec::new();
    terminals
        .try_reserve_exact(ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1)
        .map_err(|_| ZkX509AccumulatorStarkErrorV1::Resource)?;
    let mut previous_aux = None;
    for index in 0..ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1 {
        let base = trace.base_row(index)?;
        let fixed = compile_ca_accumulator_fixed_row_v1(index)?;
        let aux = build_aux_row_v1(
            public,
            &base,
            &fixed,
            previous_aux.as_ref(),
            sha_challenges,
            io_challenges,
        )?;
        append_array_row_v1(&mut base_columns, &base)?;
        append_array_row_v1(&mut aux_columns, &aux)?;
        if index < ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1 {
            let call = usize::try_from(fixed[FIX_CALL].0)
                .map_err(|_| ZkX509AccumulatorStarkErrorV1::Resource)?;
            let manifest = schedule
                .call(call)
                .map_err(|_| ZkX509AccumulatorStarkErrorV1::CallBus)?;
            let expected_role = trace.hash_witnesses[index].role;
            if manifest.call as usize != call
                || manifest.role != expected_role
                || manifest.activation != ZkX509ShaCallActivationV1::Required
                || manifest.maximum_blocks != 3
            {
                return Err(ZkX509AccumulatorStarkErrorV1::CallBus);
            }
            terminals.push(ZkX509CaAccumulatorCallTerminalV1 {
                call: manifest.call,
                role: manifest.role,
                source_products: core::array::from_fn(|lane| {
                    aux[source_aux_cell_v1(SOURCE_STATES_V1 - 1, lane)]
                }),
                digest_products: core::array::from_fn(|lane| {
                    aux[digest_aux_cell_v1(DIGEST_STATES_V1 - 1, lane)]
                }),
            });
        }
        previous_aux = Some(aux);
    }
    let root_spki_last_row = ZK_X509_CA_ACCUMULATOR_NONPADDING_ROWS_V1 - 1;
    for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
        terminals[0].source_products[lane] = terminals[0].source_products[lane]
            .mul(aux_columns[serialized_sha_product_cell_v1(lane)][root_spki_last_row]);
    }
    let root_spki_terminal = ZkX509CaAccumulatorIoTerminalV1 {
        channel: ca_accumulator_root_spki_channel_v1(schedule)?,
        event_count: ZK_X509_CA_ACCUMULATOR_ROOT_SPKI_IO_EVENTS_V1,
        consumer_products: core::array::from_fn(|lane| {
            aux_columns[root_spki_io_product_cell_v1(lane)][root_spki_last_row]
        }),
    };
    validate_ca_accumulator_io_terminal_v1(public, root_spki_terminal)?;
    Ok(ZkX509CaAccumulatorStarkMaterialV1 {
        base_columns,
        aux_columns,
        fixed_columns,
        terminals: terminals
            .try_into()
            .map_err(|_: Vec<ZkX509CaAccumulatorCallTerminalV1>| {
                ZkX509AccumulatorStarkErrorV1::Shape
            })?,
        root_spki_terminal,
    })
}
/// Extract the exact proof terminal claims from committed material.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn ca_accumulator_stark_terminal_claims_v1(
    material: &ZkX509CaAccumulatorStarkMaterialV1,
) -> ZkX509CaAccumulatorStarkTerminalClaimsV1 {
    ZkX509CaAccumulatorStarkTerminalClaimsV1 {
        source_products: core::array::from_fn(|row| material.terminals[row].source_products),
        digest_products: core::array::from_fn(|row| material.terminals[row].digest_products),
        root_spki_consumer_products: material.root_spki_terminal.consumer_products,
    }
}
/// Compile the exact typed binding handed to the outer X5S1 verifier.
#[cfg(test)]
pub(crate) fn ca_accumulator_subproof_binding_v1(
    trace: &ZkX509CaAccumulatorTraceV1,
    schedule: &ZkX509ShaCallScheduleV1,
    material: &ZkX509CaAccumulatorStarkMaterialV1,
) -> Result<ZkX509CaAccumulatorSubproofBindingV1, ZkX509AccumulatorStarkErrorV1> {
    let binding = ZkX509CaAccumulatorSubproofBindingV1 {
        public: ca_accumulator_stark_public_v1(trace, schedule)?,
        sha_terminals: material.terminals,
        root_spki_terminal: material.root_spki_terminal,
    };
    validate_ca_accumulator_subproof_binding_v1(binding.public, schedule, binding)?;
    Ok(binding)
}
/// Extract the algebra claims from a validated typed subproof binding.
#[cfg(test)]
pub(crate) fn ca_accumulator_subproof_terminal_claims_v1(
    binding: ZkX509CaAccumulatorSubproofBindingV1,
) -> ZkX509CaAccumulatorStarkTerminalClaimsV1 {
    ZkX509CaAccumulatorStarkTerminalClaimsV1 {
        source_products: binding
            .sha_terminals
            .map(|terminal| terminal.source_products),
        digest_products: binding
            .sha_terminals
            .map(|terminal| terminal.digest_products),
        root_spki_consumer_products: binding.root_spki_terminal.consumer_products,
    }
}
/// Validate decoded variable-length terminals before fixed-size conversion.
///
/// This is the fail-closed decoder boundary for omission, insertion,
/// substitution, and reordering attacks.
pub(crate) fn validate_ca_accumulator_subproof_terminal_sequence_v1(
    public: ZkX509CaAccumulatorStarkPublicV1,
    schedule: &ZkX509ShaCallScheduleV1,
    sha_terminals: &[ZkX509CaAccumulatorCallTerminalV1],
    root_spki_terminal: ZkX509CaAccumulatorIoTerminalV1,
) -> Result<(), ZkX509AccumulatorStarkErrorV1> {
    if sha_terminals.len() != ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1 {
        return Err(ZkX509AccumulatorStarkErrorV1::CallBus);
    }
    validate_ca_accumulator_stark_public_v1(public, schedule)?;
    for (index, terminal) in sha_terminals.iter().copied().enumerate() {
        let expected_call = ZK_X509_SHA_CA_LEAF_CALL_V1
            .checked_add(index)
            .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?;
        let expected_role = if index == 0 {
            ZkX509ShaCallRoleV1::CaLeaf
        } else {
            ZkX509ShaCallRoleV1::CaNode(
                u8::try_from(index - 1).map_err(|_| ZkX509AccumulatorStarkErrorV1::Resource)?,
            )
        };
        let manifest = schedule
            .call(expected_call)
            .map_err(|_| ZkX509AccumulatorStarkErrorV1::CallBus)?;
        if usize::from(terminal.call) != expected_call
            || terminal.role != expected_role
            || usize::from(manifest.call) != expected_call
            || manifest.role != expected_role
            || manifest.activation != ZkX509ShaCallActivationV1::Required
            || manifest.maximum_blocks != 3
            || terminal
                .source_products
                .iter()
                .chain(&terminal.digest_products)
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509AccumulatorStarkErrorV1::CallBus);
        }
    }
    validate_ca_accumulator_io_terminal_v1(public, root_spki_terminal)
}
/// Validate one typed outer binding against the verifier-derived statement.
pub(crate) fn validate_ca_accumulator_subproof_binding_v1(
    expected_public: ZkX509CaAccumulatorStarkPublicV1,
    schedule: &ZkX509ShaCallScheduleV1,
    binding: ZkX509CaAccumulatorSubproofBindingV1,
) -> Result<(), ZkX509AccumulatorStarkErrorV1> {
    validate_ca_accumulator_stark_public_v1(expected_public, schedule)?;
    if binding.public != expected_public {
        return Err(ZkX509AccumulatorStarkErrorV1::Witness);
    }
    validate_ca_accumulator_subproof_terminal_sequence_v1(
        binding.public,
        schedule,
        &binding.sha_terminals,
        binding.root_spki_terminal,
    )
}
fn validate_ca_accumulator_stark_public_v1(
    public: ZkX509CaAccumulatorStarkPublicV1,
    schedule: &ZkX509ShaCallScheduleV1,
) -> Result<(), ZkX509AccumulatorStarkErrorV1> {
    let expected_channel = F(u64::from(ca_accumulator_root_spki_channel_v1(schedule)?));
    if public.root_spki_channel != expected_channel
        || public
            .governed_root
            .iter()
            .any(|value| F::canonical(value.0).is_none() || value.0 > u64::from(u8::MAX))
    {
        return Err(ZkX509AccumulatorStarkErrorV1::IoBus);
    }
    Ok(())
}
/// Validate the fixed metadata and canonical products of the root-SPKI claim.
pub(crate) fn validate_ca_accumulator_io_terminal_v1(
    public: ZkX509CaAccumulatorStarkPublicV1,
    terminal: ZkX509CaAccumulatorIoTerminalV1,
) -> Result<(), ZkX509AccumulatorStarkErrorV1> {
    if terminal.channel
        != u32::try_from(public.root_spki_channel.0)
            .map_err(|_| ZkX509AccumulatorStarkErrorV1::IoBus)?
        || terminal.event_count != ZK_X509_CA_ACCUMULATOR_ROOT_SPKI_IO_EVENTS_V1
        || terminal
            .consumer_products
            .iter()
            .any(|value| F::canonical(value.0).is_none())
    {
        return Err(ZkX509AccumulatorStarkErrorV1::IoBus);
    }
    Ok(())
}
/// Evaluate the exact residue vector at one opened current/next pair.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(crate) fn evaluate_ca_accumulator_stark_residues_v1(
    public: ZkX509CaAccumulatorStarkPublicV1,
    base: &[F],
    next_base: &[F],
    aux: &[F],
    next_aux: &[F],
    fixed: &[F],
    sha_challenges: ZkX509ShaCallBusChallengesV1,
    io_challenges: ZkX509Rfc5280StarkChallengesV1,
    terminal_claims: ZkX509CaAccumulatorStarkTerminalClaimsV1,
) -> Result<Vec<F>, ZkX509AccumulatorStarkErrorV1> {
    if base.len() != ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1
        || next_base.len() != ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1
        || aux.len() != ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1
        || next_aux.len() != ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1
        || fixed.len() != ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1
    {
        return Err(ZkX509AccumulatorStarkErrorV1::Shape);
    }
    sha_challenges
        .validate()
        .map_err(|_| ZkX509AccumulatorStarkErrorV1::CallBus)?;
    io_challenges
        .validate()
        .map_err(|_| ZkX509AccumulatorStarkErrorV1::IoBus)?;
    if public.root_spki_channel.0 > u64::from(u32::MAX)
        || F::canonical(public.root_spki_channel.0).is_none()
        || public
            .governed_root
            .iter()
            .chain(base)
            .chain(next_base)
            .chain(aux)
            .chain(next_aux)
            .chain(fixed)
            .chain(terminal_claims.source_products.iter().flatten())
            .chain(terminal_claims.digest_products.iter().flatten())
            .chain(terminal_claims.root_spki_consumer_products.iter())
            .any(|value| F::canonical(value.0).is_none())
    {
        return Err(ZkX509AccumulatorStarkErrorV1::Shape);
    }
    let active = fixed[FIX_ACTIVE];
    let leaf = fixed[FIX_LEAF];
    let node = fixed[FIX_NODE];
    let io = fixed[FIX_IO_ACTIVE];
    let padding = fixed[FIX_PADDING];
    let transition = fixed[FIX_TRANSITION];
    let io_transition = fixed[FIX_IO_TRANSITION];
    let last = fixed[FIX_LAST];
    let mut residues = Vec::with_capacity(ZK_X509_CA_ACCUMULATOR_CONSTRAINT_COUNT_V1);
    let digest_bits = &base[CA_DIGEST_BYTE_BITS_START..CA_DIGEST_BYTE_BITS_START + 32 * 8];
    let sibling_bits = &base[CA_SIBLING_BYTE_BITS_START..CA_SIBLING_BYTE_BITS_START + 32 * 8];
    let io_byte_bits = &base[CA_IO_BYTE_BITS_START..CA_IO_BYTE_BITS_START + 8];
    residues.extend(
        digest_bits
            .iter()
            .chain(sibling_bits.iter())
            .chain(io_byte_bits.iter())
            .map(|bit| bit.mul(bit.sub(F::ONE))),
    );
    for byte in 0..32 {
        residues.push(
            pack_little_bits_v1(&digest_bits[byte * 8..byte * 8 + 8])
                .sub(base[CA_DIGEST_START + byte]),
        );
        residues.push(
            pack_little_bits_v1(&sibling_bits[byte * 8..byte * 8 + 8])
                .sub(base[CA_SIBLING_START + byte]),
        );
    }
    residues.push(pack_little_bits_v1(io_byte_bits).sub(base[CA_IO_BYTE]));
    let index_bits =
        &base[CA_INDEX_BITS_START..CA_INDEX_BITS_START + ZK_X509_CA_COMPACT_TREE_DEPTH_V1];
    residues.extend(
        index_bits
            .iter()
            .map(|bit| active.mul(bit.mul(bit.sub(F::ONE)))),
    );
    let selected_direction = (0..ZK_X509_CA_COMPACT_TREE_DEPTH_V1).fold(F::ZERO, |sum, bit| {
        sum.add(fixed[FIX_INDEX_SELECTORS_START + bit].mul(index_bits[bit]))
    });
    residues.push(active.mul(base[CA_DIRECTION].sub(selected_direction)));
    for bit in 0..ZK_X509_CA_COMPACT_TREE_DEPTH_V1 {
        residues.push(transition.mul(next_base[CA_INDEX_BITS_START + bit].sub(index_bits[bit])));
    }
    for byte in 0..32 {
        residues.push(
            transition.mul(next_base[CA_CURRENT_START + byte].sub(base[CA_DIGEST_START + byte])),
        );
        residues.push(leaf.mul(base[CA_CURRENT_START + byte].sub(base[CA_DIGEST_START + byte])));
    }
    for column in [CA_SIBLING_START, CA_LEFT_START, CA_RIGHT_START] {
        residues.extend(
            base[column..column + 32]
                .iter()
                .map(|value| leaf.mul(*value)),
        );
    }
    residues.push(leaf.mul(base[CA_DIRECTION]));
    let direction = base[CA_DIRECTION];
    for byte in 0..32 {
        let current = base[CA_CURRENT_START + byte];
        let sibling = base[CA_SIBLING_START + byte];
        let left = current.add(direction.mul(sibling.sub(current)));
        let right = sibling.add(direction.mul(current.sub(sibling)));
        residues.push(node.mul(base[CA_LEFT_START + byte].sub(left)));
        residues.push(node.mul(base[CA_RIGHT_START + byte].sub(right)));
    }
    for byte in 0..32 {
        residues.push(last.mul(base[CA_DIGEST_START + byte].sub(public.governed_root[byte])));
    }
    residues.extend(
        base[..CA_DIGEST_BYTE_BITS_START]
            .iter()
            .map(|value| io.mul(*value)),
    );
    residues.push(active.mul(base[CA_IO_BYTE]));
    residues.push(active.mul(base[CA_IO_WORD_ACC]));
    residues.push(
        fixed[FIX_IO_FIRST].mul(
            base[CA_IO_WORD_ACC].sub(
                F(u64::from(ZK_X509_CA_LEAF_SPKI_PREFIX_BYTE_V1))
                    .mul(F(256))
                    .add(base[CA_IO_BYTE]),
            ),
        ),
    );
    residues.push(
        io_transition.mul(
            next_base[CA_IO_WORD_ACC].sub(
                fixed[FIX_IO_SAME_WORD_TO_NEXT]
                    .mul(base[CA_IO_WORD_ACC])
                    .mul(F(256))
                    .add(next_base[CA_IO_BYTE]),
            ),
        ),
    );
    residues.extend(
        base[..CA_DIGEST_BYTE_BITS_START]
            .iter()
            .map(|value| padding.mul(*value)),
    );
    residues.push(padding.mul(base[CA_IO_BYTE]));
    residues.push(padding.mul(base[CA_IO_WORD_ACC]));
    if residues.len() != ZK_X509_CA_ACCUMULATOR_BASE_CONSTRAINT_COUNT_V1 {
        return Err(ZkX509AccumulatorStarkErrorV1::Shape);
    }
    for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
        residues.push(aux[source_aux_cell_v1(0, lane)].sub(active));
    }
    // State zero is already constrained to `active`. Ungated recurrences
    // therefore force every later state to zero on inactive rows while
    // avoiding a redundant selector factor in the committed quotient.
    for step in 0..SOURCE_PAIR_STEPS_V1 {
        for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
            let first = source_factor_v1(base, fixed, 2 * step, lane, sha_challenges)?;
            let second = source_factor_v1(base, fixed, 2 * step + 1, lane, sha_challenges)?;
            residues.push(
                aux[source_aux_cell_v1(step + 1, lane)]
                    .sub(aux[source_aux_cell_v1(step, lane)].mul(first).mul(second)),
            );
        }
    }
    for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
        residues.push(aux[digest_aux_cell_v1(0, lane)].sub(active));
    }
    for step in 0..DIGEST_PAIR_STEPS_V1 {
        for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
            let first = digest_factor_v1(base, fixed, 2 * step, lane, sha_challenges)?;
            let second = digest_factor_v1(base, fixed, 2 * step + 1, lane, sha_challenges)?;
            residues.push(
                aux[digest_aux_cell_v1(step + 1, lane)]
                    .sub(aux[digest_aux_cell_v1(step, lane)].mul(first).mul(second)),
            );
        }
    }
    let non_io = F::ONE.sub(io);
    for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
        let serialized_sha = aux[serialized_sha_product_cell_v1(lane)];
        let root_spki_io = aux[root_spki_io_product_cell_v1(lane)];
        let current_sha_factor = serialized_sha_factor_v1(
            fixed[FIX_IO_WORD_END],
            fixed[FIX_IO_WORD_INDEX],
            base[CA_IO_WORD_ACC],
            lane,
            sha_challenges,
        )?;
        let current_io_factor = root_spki_io_factor_v1(
            public,
            base[CA_IO_BYTE],
            fixed[FIX_IO_OFFSET],
            lane,
            io_challenges,
        )?;
        residues.push(fixed[FIX_IO_FIRST].mul(serialized_sha.sub(current_sha_factor)));
        residues.push(fixed[FIX_IO_FIRST].mul(root_spki_io.sub(current_io_factor)));
        let next_sha_factor = serialized_sha_factor_v1(
            fixed[FIX_IO_NEXT_WORD_END],
            fixed[FIX_IO_NEXT_WORD_INDEX],
            next_base[CA_IO_WORD_ACC],
            lane,
            sha_challenges,
        )?;
        let next_io_factor = root_spki_io_factor_v1(
            public,
            next_base[CA_IO_BYTE],
            fixed[FIX_IO_OFFSET].add(F::ONE),
            lane,
            io_challenges,
        )?;
        residues.push(io_transition.mul(
            next_aux[serialized_sha_product_cell_v1(lane)].sub(serialized_sha.mul(next_sha_factor)),
        ));
        residues.push(io_transition.mul(
            next_aux[root_spki_io_product_cell_v1(lane)].sub(root_spki_io.mul(next_io_factor)),
        ));
        residues.push(non_io.mul(serialized_sha));
        residues.push(non_io.mul(root_spki_io));
    }
    for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
        let leaf_constant_source = leaf_constant_source_product_v1(lane, sha_challenges)?;
        let selected_source = leaf
            .mul(leaf_constant_source)
            .add(selected_node_terminal_v1(
                fixed,
                terminal_claims.source_products,
                lane,
            ));
        let selected_digest =
            selected_call_terminal_v1(fixed, terminal_claims.digest_products, lane);
        residues.push(aux[source_aux_cell_v1(SOURCE_STATES_V1 - 1, lane)].sub(selected_source));
        residues.push(aux[digest_aux_cell_v1(DIGEST_STATES_V1 - 1, lane)].sub(selected_digest));
        residues.push(
            fixed[FIX_IO_LAST].mul(
                aux[serialized_sha_product_cell_v1(lane)]
                    .mul(leaf_constant_source)
                    .sub(terminal_claims.source_products[0][lane]),
            ),
        );
        residues.push(
            fixed[FIX_IO_LAST].mul(
                aux[root_spki_io_product_cell_v1(lane)]
                    .sub(terminal_claims.root_spki_consumer_products[lane]),
            ),
        );
    }
    if residues.len() != ZK_X509_CA_ACCUMULATOR_CONSTRAINT_COUNT_V1 {
        return Err(ZkX509AccumulatorStarkErrorV1::Shape);
    }
    Ok(residues)
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn build_aux_row_v1(
    public: ZkX509CaAccumulatorStarkPublicV1,
    base: &[F; ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1],
    fixed: &[F; ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1],
    previous_aux: Option<&[F; ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1]>,
    sha_challenges: ZkX509ShaCallBusChallengesV1,
    io_challenges: ZkX509Rfc5280StarkChallengesV1,
) -> Result<[F; ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1], ZkX509AccumulatorStarkErrorV1> {
    let mut aux = [F::ZERO; ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1];
    if fixed[FIX_ACTIVE] == F::ONE {
        for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
            aux[source_aux_cell_v1(0, lane)] = F::ONE;
            for step in 0..SOURCE_PAIR_STEPS_V1 {
                aux[source_aux_cell_v1(step + 1, lane)] = aux[source_aux_cell_v1(step, lane)]
                    .mul(source_factor_v1(
                        base,
                        fixed,
                        2 * step,
                        lane,
                        sha_challenges,
                    )?)
                    .mul(source_factor_v1(
                        base,
                        fixed,
                        2 * step + 1,
                        lane,
                        sha_challenges,
                    )?);
            }
            aux[digest_aux_cell_v1(0, lane)] = F::ONE;
            for step in 0..DIGEST_PAIR_STEPS_V1 {
                aux[digest_aux_cell_v1(step + 1, lane)] = aux[digest_aux_cell_v1(step, lane)]
                    .mul(digest_factor_v1(
                        base,
                        fixed,
                        2 * step,
                        lane,
                        sha_challenges,
                    )?)
                    .mul(digest_factor_v1(
                        base,
                        fixed,
                        2 * step + 1,
                        lane,
                        sha_challenges,
                    )?);
            }
        }
    }
    if fixed[FIX_IO_ACTIVE] == F::ONE {
        for lane in 0..ZK_X509_RFC5280_STARK_BUS_LANES_V1 {
            let previous_sha = if fixed[FIX_IO_FIRST] == F::ONE {
                F::ONE
            } else {
                previous_aux.ok_or(ZkX509AccumulatorStarkErrorV1::Shape)?
                    [serialized_sha_product_cell_v1(lane)]
            };
            let previous_io = if fixed[FIX_IO_FIRST] == F::ONE {
                F::ONE
            } else {
                previous_aux.ok_or(ZkX509AccumulatorStarkErrorV1::Shape)?
                    [root_spki_io_product_cell_v1(lane)]
            };
            aux[serialized_sha_product_cell_v1(lane)] = previous_sha.mul(serialized_sha_factor_v1(
                fixed[FIX_IO_WORD_END],
                fixed[FIX_IO_WORD_INDEX],
                base[CA_IO_WORD_ACC],
                lane,
                sha_challenges,
            )?);
            aux[root_spki_io_product_cell_v1(lane)] = previous_io.mul(root_spki_io_factor_v1(
                public,
                base[CA_IO_BYTE],
                fixed[FIX_IO_OFFSET],
                lane,
                io_challenges,
            )?);
        }
    }
    Ok(aux)
}
fn selected_node_terminal_v1(
    fixed: &[F],
    claims: [[F; ZK_X509_SHA_BUS_LANES_V1]; ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1],
    lane: usize,
) -> F {
    (0..ZK_X509_CA_COMPACT_TREE_DEPTH_V1).fold(F::ZERO, |selected, level| {
        selected.add(fixed[FIX_INDEX_SELECTORS_START + level].mul(claims[level + 1][lane]))
    })
}
fn serialized_sha_factor_v1(
    word_end: F,
    word_index: F,
    word_value: F,
    lane: usize,
    challenges: ZkX509ShaCallBusChallengesV1,
) -> Result<F, ZkX509AccumulatorStarkErrorV1> {
    let lane_challenge = challenges
        .lanes
        .get(lane)
        .copied()
        .ok_or(ZkX509AccumulatorStarkErrorV1::CallBus)?;
    let factor = compress_sha_call_fields_v1(
        F(ZK_X509_SHA_CA_LEAF_CALL_V1 as u64),
        F(u64::from(ZkX509ShaCallRoleV1::CaLeaf.role_code())),
        F::ZERO,
        F(u64::from(ZkX509ShaCallWordKindV1::Input.code())),
        word_index,
        word_value,
        lane_challenge,
    );
    Ok(F::ONE.add(word_end.mul(factor.sub(F::ONE))))
}
fn leaf_constant_source_product_v1(
    lane: usize,
    challenges: ZkX509ShaCallBusChallengesV1,
) -> Result<F, ZkX509AccumulatorStarkErrorV1> {
    let fixed = compile_ca_accumulator_fixed_row_v1(0)?;
    let base = [F::ZERO; ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1];
    (0..SOURCE_WORDS_V1).try_fold(F::ONE, |product, word| {
        if (LEAF_DYNAMIC_WORD_START_V1..=LEAF_DYNAMIC_WORD_END_V1).contains(&word) {
            Ok(product)
        } else {
            Ok(product.mul(source_factor_v1(&base, &fixed, word, lane, challenges)?))
        }
    })
}
fn root_spki_io_factor_v1(
    public: ZkX509CaAccumulatorStarkPublicV1,
    value: F,
    offset: F,
    lane: usize,
    challenges: ZkX509Rfc5280StarkChallengesV1,
) -> Result<F, ZkX509AccumulatorStarkErrorV1> {
    let challenge = challenges
        .tuple
        .get(lane)
        .copied()
        .ok_or(ZkX509AccumulatorStarkErrorV1::IoBus)?;
    let values = [
        F(80),
        F(ZkX509Rfc5280OutputRoleV1::GovernedTrustAnchor as u64),
        public.root_spki_channel,
        F(4),
        F::ZERO,
        offset,
        value,
        F::ZERO,
        F::ZERO,
        F::ZERO,
        F::ZERO,
        F::ZERO,
    ];
    Ok(values
        .into_iter()
        .zip(challenge)
        .fold(F::ZERO, |sum, (term, coefficient)| {
            sum.add(term.mul(coefficient))
        }))
}
fn selected_call_terminal_v1(
    fixed: &[F],
    claims: [[F; ZK_X509_SHA_BUS_LANES_V1]; ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1],
    lane: usize,
) -> F {
    let mut selected = fixed[FIX_LEAF].mul(claims[0][lane]);
    for level in 0..ZK_X509_CA_COMPACT_TREE_DEPTH_V1 {
        selected =
            selected.add(fixed[FIX_INDEX_SELECTORS_START + level].mul(claims[level + 1][lane]));
    }
    selected
}
fn source_factor_v1(
    base: &[F],
    fixed: &[F],
    word: usize,
    lane: usize,
    challenges: ZkX509ShaCallBusChallengesV1,
) -> Result<F, ZkX509AccumulatorStarkErrorV1> {
    let factor = compress_sha_call_fields_v1(
        fixed[FIX_CALL],
        fixed[FIX_ROLE],
        fixed[FIX_SLOT],
        F(u64::from(ZkX509ShaCallWordKindV1::Input.code())),
        F(u64::try_from(word).map_err(|_| ZkX509AccumulatorStarkErrorV1::Resource)?),
        source_word_v1(base, fixed, word)?,
        challenges.lanes[lane],
    );
    if (LEAF_DYNAMIC_WORD_START_V1..=LEAF_DYNAMIC_WORD_END_V1).contains(&word) {
        Ok(fixed[FIX_LEAF].add(fixed[FIX_NODE].mul(factor)))
    } else {
        Ok(factor)
    }
}
fn digest_factor_v1(
    base: &[F],
    fixed: &[F],
    word: usize,
    lane: usize,
    challenges: ZkX509ShaCallBusChallengesV1,
) -> Result<F, ZkX509AccumulatorStarkErrorV1> {
    Ok(compress_sha_call_fields_v1(
        fixed[FIX_CALL],
        fixed[FIX_ROLE],
        fixed[FIX_SLOT],
        F(u64::from(ZkX509ShaCallWordKindV1::Digest.code())),
        F(u64::try_from(word).map_err(|_| ZkX509AccumulatorStarkErrorV1::Resource)?),
        pack_be_bytes_v1(&base[CA_DIGEST_START + word * 4..CA_DIGEST_START + word * 4 + 4]),
        challenges.lanes[lane],
    ))
}
fn source_word_v1(
    base: &[F],
    fixed: &[F],
    word: usize,
) -> Result<F, ZkX509AccumulatorStarkErrorV1> {
    if word >= SOURCE_WORDS_V1 {
        return Err(ZkX509AccumulatorStarkErrorV1::Shape);
    }
    let mut value = fixed[FIX_SOURCE_CONSTANTS_START + word];
    for byte in 0..32 {
        // `source_factor_v1` already selects this complete word with
        // `FIX_NODE`. Multiplying every dynamic byte by the same selector here
        // would rely on `node^2 = node` only on the native domain and raise
        // the actual univariate quotient above the three-chunk FRI capacity.
        // The unselected leaf/I/O/padding value is immaterial because the
        // outer selector and zero-initialized product recurrence reject it.
        value = value.add(dynamic_byte_contribution_v1(
            word,
            NODE_LEFT_DYNAMIC_OFFSET_V1 + byte,
            base[CA_LEFT_START + byte],
        ));
        value = value.add(dynamic_byte_contribution_v1(
            word,
            NODE_RIGHT_DYNAMIC_OFFSET_V1 + byte,
            base[CA_RIGHT_START + byte],
        ));
    }
    Ok(value)
}
fn dynamic_byte_contribution_v1(word: usize, offset: usize, value: F) -> F {
    if offset / 4 != word {
        return F::ZERO;
    }
    value.mul(F(1_u64 << (8 * (3 - offset % 4))))
}
fn padded_source_words_v1(
    message: &[u8],
) -> Result<[u32; SOURCE_WORDS_V1], ZkX509AccumulatorStarkErrorV1> {
    if message.is_empty() || message.len() > 183 {
        return Err(ZkX509AccumulatorStarkErrorV1::Shape);
    }
    let mut padded = Vec::new();
    padded
        .try_reserve_exact(SOURCE_WORDS_V1 * 4)
        .map_err(|_| ZkX509AccumulatorStarkErrorV1::Resource)?;
    padded.extend_from_slice(message);
    padded.push(0x80);
    padded.resize(SOURCE_WORDS_V1 * 4 - 8, 0);
    padded.extend_from_slice(
        &u64::try_from(message.len())
            .map_err(|_| ZkX509AccumulatorStarkErrorV1::Resource)?
            .checked_mul(8)
            .ok_or(ZkX509AccumulatorStarkErrorV1::Resource)?
            .to_be_bytes(),
    );
    if padded.len() != SOURCE_WORDS_V1 * 4 {
        return Err(ZkX509AccumulatorStarkErrorV1::Shape);
    }
    padded
        .chunks_exact(4)
        .map(|word| u32::from_be_bytes(word.try_into().expect("four bytes")))
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_: Vec<u32>| ZkX509AccumulatorStarkErrorV1::Shape)
}
const fn source_aux_cell_v1(state: usize, lane: usize) -> usize {
    SOURCE_AUX_START + state * ZK_X509_SHA_BUS_LANES_V1 + lane
}
const fn digest_aux_cell_v1(state: usize, lane: usize) -> usize {
    DIGEST_AUX_START + state * ZK_X509_SHA_BUS_LANES_V1 + lane
}
const fn serialized_sha_product_cell_v1(lane: usize) -> usize {
    SERIALIZED_SHA_PRODUCT_START + lane
}
const fn root_spki_io_product_cell_v1(lane: usize) -> usize {
    ROOT_SPKI_IO_PRODUCT_START + lane
}
fn pack_little_bits_v1(bits: &[F]) -> F {
    bits.iter().enumerate().fold(F::ZERO, |value, (bit, cell)| {
        value.add(cell.mul(F(1_u64 << bit)))
    })
}
fn pack_be_bytes_v1(bytes: &[F]) -> F {
    bytes
        .iter()
        .fold(F::ZERO, |value, byte| value.mul(F(256)).add(*byte))
}
fn allocate_columns_v1(
    width: usize,
    rows: usize,
) -> Result<Vec<Vec<F>>, ZkX509AccumulatorStarkErrorV1> {
    let mut columns = Vec::new();
    columns
        .try_reserve_exact(width)
        .map_err(|_| ZkX509AccumulatorStarkErrorV1::Resource)?;
    for _ in 0..width {
        let mut column = Vec::new();
        column
            .try_reserve_exact(rows)
            .map_err(|_| ZkX509AccumulatorStarkErrorV1::Resource)?;
        columns.push(column);
    }
    Ok(columns)
}
fn append_array_row_v1<const WIDTH: usize>(
    columns: &mut [Vec<F>],
    row: &[F; WIDTH],
) -> Result<(), ZkX509AccumulatorStarkErrorV1> {
    if columns.len() != WIDTH {
        return Err(ZkX509AccumulatorStarkErrorV1::Shape);
    }
    for (column, value) in columns.iter_mut().zip(row.iter().copied()) {
        column.push(value);
    }
    Ok(())
}
fn ca_aggregate_layout_v1()
-> Result<aggregate::AggregateProofLayoutV1, ZkX509CaAccumulatorProofErrorV1> {
    let layout = aggregate::AggregateProofLayoutV1::new(
        CA_AGGREGATE_PARAMETERS_V1,
        vec![aggregate::AggregateTraceGroupLayoutV1 {
            native_trace_log2: ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
            segment_instances: ZK_X509_CA_ACCUMULATOR_CHUNKS_V1,
            base_width: ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1,
            aux_width: ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1,
        }],
    )
    .map_err(map_aggregate_proof_error_v1)?;
    if layout.common_lde_log2() != ZK_X509_CA_FRI_LDE_LOG2_V1
        || layout
            .fri_rounds(CA_AGGREGATE_PARAMETERS_V1)
            .map_err(map_aggregate_proof_error_v1)?
            != usize::from(ZK_X509_CA_FRI_ROUNDS_V1)
        || aggregate::maximum_encoded_proof_with_deep_bytes_v1(CA_AGGREGATE_PARAMETERS_V1, &layout)
            .map_err(map_aggregate_proof_error_v1)?
            > CA_INNER_MAXIMUM_PROOF_BYTES_V1
    {
        return Err(ZkX509CaAccumulatorProofErrorV1::Resource);
    }
    Ok(layout)
}
fn canonical_ca_manifest_role_v1(
    call: usize,
) -> Result<ZkX509ShaCallRoleV1, ZkX509CaAccumulatorProofErrorV1> {
    if call == ZK_X509_SHA_CA_LEAF_CALL_V1 {
        return Ok(ZkX509ShaCallRoleV1::CaLeaf);
    }
    call.checked_sub(ZK_X509_SHA_CA_NODE_CALL_START_V1)
        .filter(|level| *level < ZK_X509_CA_COMPACT_TREE_DEPTH_V1)
        .and_then(|level| u8::try_from(level).ok())
        .map(ZkX509ShaCallRoleV1::CaNode)
        .ok_or(ZkX509CaAccumulatorProofErrorV1::InvalidStatementOrWitness)
}
fn validate_ca_proof_schedule_v1(
    schedule: &ZkX509ShaCallScheduleV1,
) -> Result<(), ZkX509CaAccumulatorProofErrorV1> {
    if schedule.shape().disclosed_attributes > 4 {
        return Err(ZkX509CaAccumulatorProofErrorV1::InvalidStatementOrWitness);
    }
    for call in ZK_X509_SHA_CA_LEAF_CALL_V1
        ..ZK_X509_SHA_CA_LEAF_CALL_V1 + ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1
    {
        let manifest = schedule
            .call(call)
            .map_err(|_| ZkX509CaAccumulatorProofErrorV1::InvalidStatementOrWitness)?;
        if usize::from(manifest.call) != call
            || manifest.role != canonical_ca_manifest_role_v1(call)?
            || manifest.activation != ZkX509ShaCallActivationV1::Required
            || manifest.maximum_message_bytes == 0
            || manifest.maximum_blocks != 3
        {
            return Err(ZkX509CaAccumulatorProofErrorV1::InvalidStatementOrWitness);
        }
    }
    Ok(())
}
fn ca_schedule_digest_v1(
    schedule: &ZkX509ShaCallScheduleV1,
) -> Result<[u8; 32], ZkX509CaAccumulatorProofErrorV1> {
    validate_ca_proof_schedule_v1(schedule)?;
    let mut encoding = Vec::new();
    encoding
        .try_reserve_exact(4 + schedule.calls().len() * 35)
        .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?;
    append_u32_v1(
        &mut encoding,
        u32::try_from(schedule.shape().disclosed_attributes)
            .map_err(|_| ZkX509CaAccumulatorProofErrorV1::InvalidStatementOrWitness)?,
    );
    for manifest in schedule.calls() {
        encoding.push(manifest.call);
        encoding.push(manifest.role.role_code());
        encoding.push(match manifest.activation {
            ZkX509ShaCallActivationV1::Required => 0,
            ZkX509ShaCallActivationV1::OptionalPrivate => 1,
            ZkX509ShaCallActivationV1::Inactive => 2,
        });
        append_u64_v1(
            &mut encoding,
            u64::try_from(manifest.maximum_message_bytes)
                .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?,
        );
        append_u64_v1(
            &mut encoding,
            u64::try_from(manifest.maximum_blocks)
                .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?,
        );
        append_u64_v1(
            &mut encoding,
            u64::try_from(manifest.first_event)
                .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?,
        );
        append_u64_v1(
            &mut encoding,
            u64::try_from(manifest.first_logical_row)
                .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?,
        );
    }
    sha256_frame_v1(CA_SCHEDULE_DIGEST_DOMAIN_V1, &[&encoding])
        .map_err(map_transparent_proof_error_v1)
}
pub(crate) fn ca_profile_digest_v1() -> Result<[u8; 32], ZkX509CaAccumulatorProofErrorV1> {
    let mut parameters = Vec::new();
    parameters.extend_from_slice(&CA_INNER_PROOF_MAGIC_V1);
    append_u16_v1(&mut parameters, ZK_X509_PROOF_VERSION_V1);
    parameters.push(ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1);
    parameters.push(ZK_X509_CA_FRI_LDE_LOG2_V1);
    parameters.push(CA_TERMINAL_LOG2_V1);
    append_u16_v1(
        &mut parameters,
        u16::try_from(CA_TERMINAL_DEGREE_BOUND_V1)
            .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?,
    );
    append_u16_v1(
        &mut parameters,
        u16::try_from(CA_QUERY_COUNT_V1).map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?,
    );
    append_u16_v1(
        &mut parameters,
        u16::try_from(CA_MASK_DEGREE_V1).map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?,
    );
    append_u16_v1(
        &mut parameters,
        u16::try_from(CA_COMPOSITION_DEGREE_CHUNKS_V1)
            .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?,
    );
    parameters.push(ZK_X509_GRINDING_BITS_V1);
    append_u32_v1(
        &mut parameters,
        u32::try_from(CA_INNER_MAXIMUM_PROOF_BYTES_V1)
            .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?,
    );
    sha256_frame_v1(
        CA_PROFILE_DIGEST_DOMAIN_V1,
        &[
            ZK_X509_ACCUMULATOR_STARK_DESCRIPTOR_V1,
            TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
            &parameters,
            CA_BASE_LEAF_DOMAIN_V1,
            CA_BASE_NODE_DOMAIN_V1,
            CA_AUX_LEAF_DOMAIN_V1,
            CA_AUX_NODE_DOMAIN_V1,
            CA_COMPOSITION_LEAF_DOMAIN_V1,
            CA_COMPOSITION_NODE_DOMAIN_V1,
            CA_FRI_LEAF_DOMAIN_V1,
            CA_FRI_NODE_DOMAIN_V1,
        ],
    )
    .map_err(map_transparent_proof_error_v1)
}
fn validate_ca_proof_public_v1(
    public: ZkX509CaAccumulatorStarkPublicV1,
    schedule: &ZkX509ShaCallScheduleV1,
) -> Result<(), ZkX509CaAccumulatorProofErrorV1> {
    validate_ca_proof_schedule_v1(schedule)?;
    validate_ca_accumulator_stark_public_v1(public, schedule)
        .map_err(ZkX509CaAccumulatorProofErrorV1::from)
}
pub(crate) fn ca_public_digest_v1(
    public: ZkX509CaAccumulatorStarkPublicV1,
    schedule: &ZkX509ShaCallScheduleV1,
) -> Result<[u8; 32], ZkX509CaAccumulatorProofErrorV1> {
    validate_ca_proof_public_v1(public, schedule)?;
    let root = public
        .governed_root
        .map(|value| u8::try_from(value.0))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ZkX509CaAccumulatorProofErrorV1::InvalidStatementOrWitness)?;
    let channel = u32::try_from(public.root_spki_channel.0)
        .map_err(|_| ZkX509CaAccumulatorProofErrorV1::InvalidStatementOrWitness)?
        .to_be_bytes();
    let schedule_digest = ca_schedule_digest_v1(schedule)?;
    sha256_frame_v1(
        CA_PUBLIC_DIGEST_DOMAIN_V1,
        &[&root, &channel, &schedule_digest],
    )
    .map_err(map_transparent_proof_error_v1)
}
fn new_ca_transcript_v1(
    public: ZkX509CaAccumulatorStarkPublicV1,
    schedule: &ZkX509ShaCallScheduleV1,
    layout: &aggregate::AggregateProofLayoutV1,
) -> Result<TransparentTranscriptV1, ZkX509CaAccumulatorProofErrorV1> {
    let profile_digest = ca_profile_digest_v1()?;
    let public_digest = ca_public_digest_v1(public, schedule)?;
    let mut transcript =
        TransparentTranscriptV1::new(ZK_X509_SUITE_V1, &profile_digest, &public_digest)
            .map_err(map_transparent_proof_error_v1)?;
    transcript
        .absorb(
            b"zk-x509-ca-accumulator-proof-profile-v1",
            &[
                ZK_X509_ACCUMULATOR_STARK_DESCRIPTOR_V1,
                TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1,
            ],
        )
        .map_err(map_transparent_proof_error_v1)?;
    aggregate::absorb_layout_v1(
        &mut transcript,
        CA_AGGREGATE_PARAMETERS_V1,
        CA_AGGREGATE_DOMAINS_V1,
        CA_RELATION_LAYOUT_DOMAIN_V1,
        layout,
    )
    .map_err(map_aggregate_proof_error_v1)?;
    let schedule_digest = ca_schedule_digest_v1(schedule)?;
    let mut registration = Vec::new();
    registration.extend_from_slice(b"X5A1");
    append_u16_v1(&mut registration, CA_ADAPTER_ID_V1);
    append_u16_v1(&mut registration, 0);
    append_u32_v1(
        &mut registration,
        u32::try_from(schedule.shape().disclosed_attributes)
            .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?,
    );
    for value in public.governed_root {
        append_u64_v1(&mut registration, value.0);
    }
    append_u64_v1(&mut registration, public.root_spki_channel.0);
    registration.extend_from_slice(&schedule_digest);
    transcript
        .absorb(CA_REGISTRATION_DOMAIN_V1, &[&registration])
        .map_err(map_transparent_proof_error_v1)?;
    Ok(transcript)
}
fn ca_claim_address_v1(index: usize) -> Result<(u8, u8, u8), ZkX509CaAccumulatorProofErrorV1> {
    let sha_family_fields = ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1 * ZK_X509_SHA_BUS_LANES_V1;
    if index < sha_family_fields {
        let row = index / ZK_X509_SHA_BUS_LANES_V1;
        return Ok((
            1,
            u8::try_from(ZK_X509_SHA_CA_LEAF_CALL_V1 + row)
                .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?,
            u8::try_from(index % ZK_X509_SHA_BUS_LANES_V1)
                .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?,
        ));
    }
    if index < 2 * sha_family_fields {
        let local = index - sha_family_fields;
        let row = local / ZK_X509_SHA_BUS_LANES_V1;
        return Ok((
            2,
            u8::try_from(ZK_X509_SHA_CA_LEAF_CALL_V1 + row)
                .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?,
            u8::try_from(local % ZK_X509_SHA_BUS_LANES_V1)
                .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?,
        ));
    }
    let lane = index
        .checked_sub(2 * sha_family_fields)
        .filter(|lane| *lane < ZK_X509_RFC5280_STARK_BUS_LANES_V1)
        .ok_or(ZkX509CaAccumulatorProofErrorV1::Resource)?;
    Ok((
        3,
        0,
        u8::try_from(lane).map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?,
    ))
}
fn ca_claim_value_v1(
    claims: ZkX509CaAccumulatorStarkTerminalClaimsV1,
    index: usize,
) -> Result<F, ZkX509CaAccumulatorProofErrorV1> {
    let sha_family_fields = ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1 * ZK_X509_SHA_BUS_LANES_V1;
    if index < sha_family_fields {
        return Ok(claims.source_products[index / ZK_X509_SHA_BUS_LANES_V1]
            [index % ZK_X509_SHA_BUS_LANES_V1]);
    }
    if index < 2 * sha_family_fields {
        let local = index - sha_family_fields;
        return Ok(claims.digest_products[local / ZK_X509_SHA_BUS_LANES_V1]
            [local % ZK_X509_SHA_BUS_LANES_V1]);
    }
    claims
        .root_spki_consumer_products
        .get(index - 2 * sha_family_fields)
        .copied()
        .ok_or(ZkX509CaAccumulatorProofErrorV1::Resource)
}
fn encode_ca_claim_records_v1(
    claims: ZkX509CaAccumulatorStarkTerminalClaimsV1,
) -> Result<Vec<u8>, ZkX509CaAccumulatorProofErrorV1> {
    let mut records = Vec::new();
    records
        .try_reserve_exact(CA_CLAIM_FIELDS_V1 * CA_CLAIM_RECORD_BYTES_V1)
        .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?;
    for index in 0..CA_CLAIM_FIELDS_V1 {
        let (family, subject, lane) = ca_claim_address_v1(index)?;
        let value = ca_claim_value_v1(claims, index)?;
        if F::canonical(value.0).is_none() {
            return Err(ZkX509CaAccumulatorProofErrorV1::NonCanonicalField);
        }
        records.extend_from_slice(&[family, subject, lane, 0]);
        append_u64_v1(&mut records, value.0);
    }
    Ok(records)
}
fn absorb_ca_terminal_claims_v1(
    transcript: &mut TransparentTranscriptV1,
    claims: ZkX509CaAccumulatorStarkTerminalClaimsV1,
) -> Result<(), ZkX509CaAccumulatorProofErrorV1> {
    let records = encode_ca_claim_records_v1(claims)?;
    transcript
        .absorb(CA_TERMINAL_CLAIMS_DOMAIN_V1, &[b"X5T1", &records])
        .map_err(map_transparent_proof_error_v1)
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn encode_ca_proof_envelope_v1(
    claims: ZkX509CaAccumulatorStarkTerminalClaimsV1,
    inner: &[u8],
) -> Result<Vec<u8>, ZkX509CaAccumulatorProofErrorV1> {
    if inner.is_empty() || inner.len() > CA_INNER_MAXIMUM_PROOF_BYTES_V1 {
        return Err(ZkX509CaAccumulatorProofErrorV1::ProofTooLarge);
    }
    let records = encode_ca_claim_records_v1(claims)?;
    let exact = CA_PROOF_ENVELOPE_BYTES_V1
        .checked_add(inner.len())
        .ok_or(ZkX509CaAccumulatorProofErrorV1::ProofTooLarge)?;
    if exact > ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1 {
        return Err(ZkX509CaAccumulatorProofErrorV1::ProofTooLarge);
    }
    let mut encoded = Vec::new();
    encoded
        .try_reserve_exact(exact)
        .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?;
    encoded.extend_from_slice(&CA_PROOF_MAGIC_V1);
    append_u16_v1(&mut encoded, ZK_X509_PROOF_VERSION_V1);
    append_u16_v1(&mut encoded, CA_ADAPTER_ID_V1);
    append_u16_v1(
        &mut encoded,
        u16::try_from(CA_CLAIM_FIELDS_V1).map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?,
    );
    encoded.extend_from_slice(&records);
    append_u32_v1(
        &mut encoded,
        u32::try_from(inner.len()).map_err(|_| ZkX509CaAccumulatorProofErrorV1::ProofTooLarge)?,
    );
    encoded.extend_from_slice(inner);
    if encoded.len() != exact {
        return Err(ZkX509CaAccumulatorProofErrorV1::Resource);
    }
    Ok(encoded)
}
fn decode_ca_proof_envelope_v1(
    encoded: &[u8],
) -> Result<(ZkX509CaAccumulatorStarkTerminalClaimsV1, &[u8]), ZkX509CaAccumulatorProofErrorV1> {
    if encoded.len() > ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1 {
        return Err(ZkX509CaAccumulatorProofErrorV1::ProofTooLarge);
    }
    if encoded.len() < CA_PROOF_ENVELOPE_BYTES_V1
        || encoded.get(..4) != Some(CA_PROOF_MAGIC_V1.as_slice())
        || u16::from_be_bytes(
            encoded[4..6]
                .try_into()
                .map_err(|_| ZkX509CaAccumulatorProofErrorV1::MalformedProof)?,
        ) != ZK_X509_PROOF_VERSION_V1
        || u16::from_be_bytes(
            encoded[6..8]
                .try_into()
                .map_err(|_| ZkX509CaAccumulatorProofErrorV1::MalformedProof)?,
        ) != CA_ADAPTER_ID_V1
        || usize::from(u16::from_be_bytes(
            encoded[8..10]
                .try_into()
                .map_err(|_| ZkX509CaAccumulatorProofErrorV1::MalformedProof)?,
        )) != CA_CLAIM_FIELDS_V1
    {
        return Err(ZkX509CaAccumulatorProofErrorV1::MalformedProof);
    }
    let mut values = [F::ZERO; CA_CLAIM_FIELDS_V1];
    for (index, value) in values.iter_mut().enumerate() {
        let start = 10_usize
            .checked_add(
                index
                    .checked_mul(CA_CLAIM_RECORD_BYTES_V1)
                    .ok_or(ZkX509CaAccumulatorProofErrorV1::MalformedProof)?,
            )
            .ok_or(ZkX509CaAccumulatorProofErrorV1::MalformedProof)?;
        let (family, subject, lane) = ca_claim_address_v1(index)?;
        if encoded.get(start..start + 4) != Some([family, subject, lane, 0].as_slice()) {
            return Err(ZkX509CaAccumulatorProofErrorV1::MalformedProof);
        }
        let raw = u64::from_be_bytes(
            encoded[start + 4..start + CA_CLAIM_RECORD_BYTES_V1]
                .try_into()
                .map_err(|_| ZkX509CaAccumulatorProofErrorV1::MalformedProof)?,
        );
        *value = F::canonical(raw).ok_or(ZkX509CaAccumulatorProofErrorV1::NonCanonicalField)?;
    }
    let inner_len = usize::try_from(u32::from_be_bytes(
        encoded[CA_PROOF_LENGTH_OFFSET_V1..CA_PROOF_ENVELOPE_BYTES_V1]
            .try_into()
            .map_err(|_| ZkX509CaAccumulatorProofErrorV1::MalformedProof)?,
    ))
    .map_err(|_| ZkX509CaAccumulatorProofErrorV1::MalformedProof)?;
    if inner_len == 0
        || inner_len > CA_INNER_MAXIMUM_PROOF_BYTES_V1
        || encoded.len()
            != CA_PROOF_ENVELOPE_BYTES_V1
                .checked_add(inner_len)
                .ok_or(ZkX509CaAccumulatorProofErrorV1::MalformedProof)?
    {
        return Err(ZkX509CaAccumulatorProofErrorV1::MalformedProof);
    }
    let sha_family_fields = ZK_X509_CA_ACCUMULATOR_ACTIVE_ROWS_V1 * ZK_X509_SHA_BUS_LANES_V1;
    let claims = ZkX509CaAccumulatorStarkTerminalClaimsV1 {
        source_products: core::array::from_fn(|row| {
            core::array::from_fn(|lane| values[row * ZK_X509_SHA_BUS_LANES_V1 + lane])
        }),
        digest_products: core::array::from_fn(|row| {
            core::array::from_fn(|lane| {
                values[sha_family_fields + row * ZK_X509_SHA_BUS_LANES_V1 + lane]
            })
        }),
        root_spki_consumer_products: core::array::from_fn(|lane| {
            values[2 * sha_family_fields + lane]
        }),
    };
    Ok((claims, &encoded[CA_PROOF_ENVELOPE_BYTES_V1..]))
}
fn ca_fixed_lde_columns_v1(
    native_columns: &[Vec<F>],
) -> Result<Vec<Vec<F>>, ZkX509CaAccumulatorProofErrorV1> {
    if native_columns.len() != ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1
        || native_columns
            .iter()
            .any(|column| column.len() != ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1)
    {
        return Err(ZkX509CaAccumulatorProofErrorV1::InvalidStatementOrWitness);
    }
    let native_root = goldilocks_primitive_root_v1(ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1)
        .map_err(map_transparent_proof_error_v1)?;
    let lde_root = goldilocks_primitive_root_v1(ZK_X509_CA_FRI_LDE_LOG2_V1)
        .map_err(map_transparent_proof_error_v1)?;
    let lde_rows = 1_usize
        .checked_shl(u32::from(ZK_X509_CA_FRI_LDE_LOG2_V1))
        .ok_or(ZkX509CaAccumulatorProofErrorV1::Resource)?;
    native_columns
        .iter()
        .map(|native| {
            let mut coefficients = native.clone();
            goldilocks_ifft_v1(&mut coefficients, native_root)
                .map_err(map_transparent_proof_error_v1)?;
            coefficients.resize(lde_rows, F::ZERO);
            goldilocks_evaluate_coset_v1(
                &coefficients,
                lde_rows,
                lde_root,
                F(GOLDILOCKS_GENERATOR_V1),
            )
            .map_err(map_transparent_proof_error_v1)
        })
        .collect()
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn ca_masked_lde_columns_v1<R: TryCryptoRng + ?Sized>(
    native_columns: &[Vec<F>],
    expected_width: usize,
    rng: &mut HealthCheckedTryCryptoRngV1<'_, R>,
) -> Result<Vec<Vec<F>>, ZkX509CaAccumulatorProofErrorV1> {
    if native_columns.len() != expected_width
        || native_columns
            .iter()
            .any(|column| column.len() != ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1)
    {
        return Err(ZkX509CaAccumulatorProofErrorV1::InvalidStatementOrWitness);
    }
    native_columns
        .iter()
        .map(|column| {
            masked_trace_lde_column_v1(
                column,
                ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1,
                ZK_X509_CA_FRI_LDE_LOG2_V1,
                CA_MASK_DEGREE_V1,
                rng,
            )
            .map_err(map_transparent_proof_error_v1)
        })
        .collect()
}
fn ca_lde_row_v1(
    columns: &[Vec<F>],
    index: usize,
    expected_width: usize,
    expected_rows: usize,
) -> Result<Vec<F>, ZkX509CaAccumulatorProofErrorV1> {
    if columns.len() != expected_width
        || index >= expected_rows
        || columns.iter().any(|column| column.len() != expected_rows)
    {
        return Err(ZkX509CaAccumulatorProofErrorV1::Resource);
    }
    Ok(columns.iter().map(|column| column[index]).collect())
}
fn derive_ca_constraint_alphas_v1(
    transcript: &mut TransparentTranscriptV1,
) -> Result<Vec<E>, ZkX509CaAccumulatorProofErrorV1> {
    (0..ZK_X509_CA_ACCUMULATOR_CONSTRAINT_COUNT_V1)
        .map(|_| {
            transcript
                .challenge_fp4(CA_CONSTRAINT_ALPHA_LABEL_V1)
                .map_err(map_transparent_proof_error_v1)
        })
        .collect()
}
fn ca_quotient_value_v1(
    x: F,
    residues: &[F],
    alphas: &[E],
) -> Result<E, ZkX509CaAccumulatorProofErrorV1> {
    if residues.len() != ZK_X509_CA_ACCUMULATOR_CONSTRAINT_COUNT_V1
        || residues.len() != alphas.len()
    {
        return Err(ZkX509CaAccumulatorProofErrorV1::ConstraintOpening);
    }
    let inverse_vanishing = x
        .pow(ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1 as u128)
        .sub(F::ONE)
        .inv()
        .ok_or(ZkX509CaAccumulatorProofErrorV1::ConstraintOpening)?;
    Ok(residues
        .iter()
        .zip(alphas)
        .fold(E::ZERO, |sum, (residue, alpha)| {
            sum.add(alpha.mul_base(*residue))
        })
        .mul_base(inverse_vanishing))
}
#[allow(clippy::too_many_arguments)]
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn ca_composition_lanes_v1(
    public: ZkX509CaAccumulatorStarkPublicV1,
    base_lde: &[Vec<F>],
    aux_lde: &[Vec<F>],
    fixed_lde: &[Vec<F>],
    sha_challenges: ZkX509ShaCallBusChallengesV1,
    io_challenges: ZkX509Rfc5280StarkChallengesV1,
    claims: ZkX509CaAccumulatorStarkTerminalClaimsV1,
    alphas: &[E],
    layout: &aggregate::AggregateProofLayoutV1,
) -> Result<Vec<Vec<Vec<E>>>, ZkX509CaAccumulatorProofErrorV1> {
    let rows = layout.common_lde_size();
    let group = layout
        .trace_groups()
        .first()
        .ok_or(ZkX509CaAccumulatorProofErrorV1::Resource)?;
    let stride = group
        .next_stride(layout.common_lde_log2())
        .map_err(map_aggregate_proof_error_v1)?;
    if rows != 1_usize << ZK_X509_CA_FRI_LDE_LOG2_V1
        || group.base_width != ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1
        || group.aux_width != ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1
    {
        return Err(ZkX509CaAccumulatorProofErrorV1::Resource);
    }
    let lde_root = goldilocks_primitive_root_v1(layout.common_lde_log2())
        .map_err(map_transparent_proof_error_v1)?;
    let mut evaluations = Vec::new();
    evaluations
        .try_reserve_exact(rows)
        .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?;
    let mut x = F(GOLDILOCKS_GENERATOR_V1);
    for index in 0..rows {
        let next = (index + stride) % rows;
        let base = ca_lde_row_v1(base_lde, index, ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1, rows)?;
        let next_base = ca_lde_row_v1(base_lde, next, ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1, rows)?;
        let aux = ca_lde_row_v1(aux_lde, index, ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1, rows)?;
        let next_aux = ca_lde_row_v1(aux_lde, next, ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1, rows)?;
        let fixed = ca_lde_row_v1(
            fixed_lde,
            index,
            ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1,
            rows,
        )?;
        let residues = evaluate_ca_accumulator_stark_residues_v1(
            public,
            &base,
            &next_base,
            &aux,
            &next_aux,
            &fixed,
            sha_challenges,
            io_challenges,
            claims,
        )
        .map_err(ZkX509CaAccumulatorProofErrorV1::from)?;
        evaluations.push(ca_quotient_value_v1(x, &residues, alphas)?);
        x = x.mul(lde_root);
    }
    let chunks = aggregate::split_composition_evaluations_v1(
        &evaluations,
        CA_AGGREGATE_PARAMETERS_V1,
        layout,
    )
    .map_err(map_aggregate_proof_error_v1)?;
    Ok(vec![chunks])
}
fn ca_challenge_vector_v1(
    transcript: &mut TransparentTranscriptV1,
    label: &[u8],
    count: usize,
) -> Result<Vec<E>, ZkX509CaAccumulatorProofErrorV1> {
    (0..count)
        .map(|_| {
            transcript
                .challenge_fp4(label)
                .map_err(map_transparent_proof_error_v1)
        })
        .collect()
}
fn derive_ca_deep_mixes_v1(
    transcript: &mut TransparentTranscriptV1,
    layout: &aggregate::AggregateProofLayoutV1,
) -> Result<Vec<aggregate::AggregateDeepLaneMixV1>, ZkX509CaAccumulatorProofErrorV1> {
    let mix = aggregate::AggregateDeepLaneMixV1 {
        trace_groups: vec![aggregate::AggregateDeepTraceGroupMixV1 {
            base_current: ca_challenge_vector_v1(
                transcript,
                CA_DEEP_BASE_CURRENT_MIX_LABEL_V1,
                ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1,
            )?,
            base_next: ca_challenge_vector_v1(
                transcript,
                CA_DEEP_BASE_NEXT_MIX_LABEL_V1,
                ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1,
            )?,
            aux_current: ca_challenge_vector_v1(
                transcript,
                CA_DEEP_AUX_CURRENT_MIX_LABEL_V1,
                ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1,
            )?,
            aux_next: ca_challenge_vector_v1(
                transcript,
                CA_DEEP_AUX_NEXT_MIX_LABEL_V1,
                ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1,
            )?,
        }],
        composition: ca_challenge_vector_v1(
            transcript,
            CA_DEEP_COMPOSITION_MIX_LABEL_V1,
            CA_COMPOSITION_DEGREE_CHUNKS_V1,
        )?,
    };
    let mixes = vec![mix];
    aggregate::validate_deep_lane_mixes_v1(&mixes, CA_AGGREGATE_PARAMETERS_V1, layout)
        .map_err(map_aggregate_proof_error_v1)?;
    Ok(mixes)
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn ca_fri_base_v1(
    trace_material: &aggregate::AggregateTraceGroupMaterialV1,
    composition: &[Vec<E>],
    deep: &aggregate::AggregateDeepProofV1,
    deep_point: E,
    mix: &aggregate::AggregateDeepLaneMixV1,
    layout: &aggregate::AggregateProofLayoutV1,
) -> Result<Vec<E>, ZkX509CaAccumulatorProofErrorV1> {
    let deep_trace =
        aggregate::canonical_deep_trace_groups_v1(deep, CA_AGGREGATE_PARAMETERS_V1, layout)
            .map_err(map_aggregate_proof_error_v1)?
            .into_iter()
            .next()
            .ok_or(ZkX509CaAccumulatorProofErrorV1::ConstraintOpening)?;
    let deep_composition = aggregate::canonical_fp4_fields_v1(
        deep.composition_values
            .first()
            .ok_or(ZkX509CaAccumulatorProofErrorV1::ConstraintOpening)?,
        CA_COMPOSITION_DEGREE_CHUNKS_V1,
    )
    .map_err(map_aggregate_proof_error_v1)?;
    let coefficients = mix
        .trace_groups
        .first()
        .ok_or(ZkX509CaAccumulatorProofErrorV1::ConstraintOpening)?;
    let rows = layout.common_lde_size();
    if trace_material.base_lde.len() != coefficients.base_current.len()
        || trace_material.base_lde.len() != coefficients.base_next.len()
        || trace_material.aux_lde.len() != coefficients.aux_current.len()
        || trace_material.aux_lde.len() != coefficients.aux_next.len()
        || composition.len() != mix.composition.len()
        || deep_trace.base_current.len() != trace_material.base_lde.len()
        || deep_trace.base_next.len() != trace_material.base_lde.len()
        || deep_trace.aux_current.len() != trace_material.aux_lde.len()
        || deep_trace.aux_next.len() != trace_material.aux_lde.len()
        || deep_composition.len() != composition.len()
        || composition.iter().any(|chunk| chunk.len() != rows)
    {
        return Err(ZkX509CaAccumulatorProofErrorV1::ConstraintOpening);
    }
    let native_root = goldilocks_primitive_root_v1(ZK_X509_CA_ACCUMULATOR_TRACE_LOG2_V1)
        .map_err(map_transparent_proof_error_v1)?;
    let deep_next = deep_point.mul_base(native_root);
    let lde_root = goldilocks_primitive_root_v1(layout.common_lde_log2())
        .map_err(map_transparent_proof_error_v1)?;
    let mut denominators = Vec::new();
    denominators
        .try_reserve_exact(
            rows.checked_mul(2)
                .ok_or(ZkX509CaAccumulatorProofErrorV1::Resource)?,
        )
        .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?;
    let mut x = F(GOLDILOCKS_GENERATOR_V1);
    for _ in 0..rows {
        let point = E::from_base(x);
        denominators.push(point.sub(deep_point));
        denominators.push(point.sub(deep_next));
        x = x.mul(lde_root);
    }
    aggregate::batch_invert_fp4_nonzero_v1(&mut denominators)
        .map_err(map_aggregate_proof_error_v1)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(rows)
        .map_err(|_| ZkX509CaAccumulatorProofErrorV1::Resource)?;
    for index in 0..rows {
        let current_inverse = denominators[2 * index];
        let next_inverse = denominators[2 * index + 1];
        let mut quotient = E::ZERO;
        for (column, ((deep_current, deep_next), (mix_current, mix_next))) in
            trace_material.base_lde.iter().zip(
                deep_trace
                    .base_current
                    .iter()
                    .zip(&deep_trace.base_next)
                    .zip(
                        coefficients
                            .base_current
                            .iter()
                            .zip(&coefficients.base_next),
                    ),
            )
        {
            let value = E::from_base(column[index]);
            quotient = quotient
                .add(
                    value
                        .sub(*deep_current)
                        .mul(current_inverse)
                        .mul(*mix_current),
                )
                .add(value.sub(*deep_next).mul(next_inverse).mul(*mix_next));
        }
        for (column, ((deep_current, deep_next), (mix_current, mix_next))) in
            trace_material.aux_lde.iter().zip(
                deep_trace
                    .aux_current
                    .iter()
                    .zip(&deep_trace.aux_next)
                    .zip(coefficients.aux_current.iter().zip(&coefficients.aux_next)),
            )
        {
            let value = E::from_base(column[index]);
            quotient = quotient
                .add(
                    value
                        .sub(*deep_current)
                        .mul(current_inverse)
                        .mul(*mix_current),
                )
                .add(value.sub(*deep_next).mul(next_inverse).mul(*mix_next));
        }
        for (chunk, (deep_value, coefficient)) in composition
            .iter()
            .zip(deep_composition.iter().zip(&mix.composition))
        {
            quotient = quotient.add(
                chunk[index]
                    .sub(*deep_value)
                    .mul(current_inverse)
                    .mul(*coefficient),
            );
        }
        result.push(quotient);
    }
    Ok(result)
}
fn absorb_ca_grinding_nonce_v1(
    transcript: &mut TransparentTranscriptV1,
    nonce: u64,
) -> Result<(), ZkX509CaAccumulatorProofErrorV1> {
    transcript
        .absorb(CA_GRINDING_NONCE_DOMAIN_V1, &[&nonce.to_be_bytes()])
        .map_err(map_transparent_proof_error_v1)
}
fn ca_binding_from_claims_v1(
    public: ZkX509CaAccumulatorStarkPublicV1,
    schedule: &ZkX509ShaCallScheduleV1,
    claims: ZkX509CaAccumulatorStarkTerminalClaimsV1,
) -> Result<ZkX509CaAccumulatorSubproofBindingV1, ZkX509CaAccumulatorProofErrorV1> {
    let binding = ZkX509CaAccumulatorSubproofBindingV1 {
        public,
        sha_terminals: core::array::from_fn(|row| {
            let call = ZK_X509_SHA_CA_LEAF_CALL_V1 + row;
            ZkX509CaAccumulatorCallTerminalV1 {
                call: u8::try_from(call).expect("compact CA call fits u8"),
                role: if row == 0 {
                    ZkX509ShaCallRoleV1::CaLeaf
                } else {
                    ZkX509ShaCallRoleV1::CaNode(
                        u8::try_from(row - 1).expect("compact CA level fits u8"),
                    )
                },
                source_products: claims.source_products[row],
                digest_products: claims.digest_products[row],
            }
        }),
        root_spki_terminal: ZkX509CaAccumulatorIoTerminalV1 {
            channel: u32::try_from(public.root_spki_channel.0)
                .map_err(|_| ZkX509CaAccumulatorProofErrorV1::InvalidStatementOrWitness)?,
            event_count: ZK_X509_CA_ACCUMULATOR_ROOT_SPKI_IO_EVENTS_V1,
            consumer_products: claims.root_spki_consumer_products,
        },
    };
    validate_ca_accumulator_subproof_binding_v1(public, schedule, binding)
        .map_err(ZkX509CaAccumulatorProofErrorV1::from)?;
    Ok(binding)
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn ca_base_columns_v1(
    trace: &ZkX509CaAccumulatorTraceV1,
) -> Result<Vec<Vec<F>>, ZkX509CaAccumulatorProofErrorV1> {
    trace
        .validate()
        .map_err(|_| ZkX509CaAccumulatorProofErrorV1::InvalidStatementOrWitness)?;
    let mut columns = allocate_columns_v1(
        ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1,
        ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1,
    )
    .map_err(ZkX509CaAccumulatorProofErrorV1::from)?;
    for index in 0..ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1 {
        let row = trace
            .base_row(index)
            .map_err(|_| ZkX509CaAccumulatorProofErrorV1::InvalidStatementOrWitness)?;
        append_array_row_v1(&mut columns, &row).map_err(ZkX509CaAccumulatorProofErrorV1::from)?;
    }
    Ok(columns)
}
struct CaOpenedRowEvaluatorV1<'a> {
    public: ZkX509CaAccumulatorStarkPublicV1,
    fixed_lde: &'a [Vec<F>],
    sha_challenges: ZkX509ShaCallBusChallengesV1,
    io_challenges: ZkX509Rfc5280StarkChallengesV1,
    claims: ZkX509CaAccumulatorStarkTerminalClaimsV1,
    alphas: &'a [E],
    lde_root: F,
    rows: usize,
}
impl aggregate::AggregateOpenedRowEvaluatorV1 for CaOpenedRowEvaluatorV1<'_> {
    fn evaluate_opened_row_v1(
        &mut self,
        query_index: usize,
        lane: usize,
        trace_groups: &[aggregate::AggregateOpenedTraceGroupV1],
        composition_chunks: &[E],
    ) -> Result<aggregate::AggregateExpectedOpeningV1, AggregateStarkErrorV1> {
        if lane != 0
            || trace_groups.len() != 1
            || composition_chunks.len() != CA_COMPOSITION_DEGREE_CHUNKS_V1
            || query_index >= self.rows
        {
            return Err(AggregateStarkErrorV1::ConstraintOpening);
        }
        let opening = &trace_groups[0];
        let fixed = ca_lde_row_v1(
            self.fixed_lde,
            query_index,
            ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1,
            self.rows,
        )
        .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
        let residues = evaluate_ca_accumulator_stark_residues_v1(
            self.public,
            &opening.base_current,
            &opening.base_next,
            &opening.aux_current,
            &opening.aux_next,
            &fixed,
            self.sha_challenges,
            self.io_challenges,
            self.claims,
        )
        .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(self.lde_root.pow(query_index as u128));
        let composition = ca_quotient_value_v1(x, &residues, self.alphas)
            .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
        Ok(aggregate::AggregateExpectedOpeningV1 {
            composition,
            // The DEEP-enabled verifier computes the actual FRI base from all
            // authenticated current/next and composition openings.
            fri_base: E::ZERO,
        })
    }
}
/// Construct the sole canonical dedicated compact-CA proof with injected,
/// fallible cryptographic entropy.
///
/// Public/witness/resource preflight is complete before the source is touched.
/// One fixed-block, health-checked reservoir session then covers every trace
/// and FRI mask, and the producer independently verifies the final canonical
/// bytes. Returned errors and source unwinds poison the session.
#[allow(clippy::too_many_lines)]
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn prove_zk_x509_ca_accumulator_stark_v1_with_rng<R: TryCryptoRng + ?Sized>(
    trace: &ZkX509CaAccumulatorTraceV1,
    sha_schedule: &ZkX509ShaCallScheduleV1,
    credential_main_pre_aux: ZkX509CredentialMainPreAuxV1,
    rng: &mut R,
) -> Result<Vec<u8>, ZkX509CaAccumulatorProofErrorV1> {
    trace
        .validate()
        .map_err(|_| ZkX509CaAccumulatorProofErrorV1::InvalidStatementOrWitness)?;
    validate_ca_proof_schedule_v1(sha_schedule)?;
    let public = ca_accumulator_stark_public_v1(trace, sha_schedule)
        .map_err(ZkX509CaAccumulatorProofErrorV1::from)?;
    validate_ca_proof_public_v1(public, sha_schedule)?;
    let request = ca_accumulator_resource_request_v1(
        ZK_X509_CA_ACCUMULATOR_REDUCED_AIR_DEGREE_V1,
        1,
        CA_QUERY_COUNT_V1,
    )
    .map_err(ZkX509CaAccumulatorProofErrorV1::from)?;
    let envelope = checked_ca_accumulator_resource_envelope_v1(request)
        .map_err(ZkX509CaAccumulatorProofErrorV1::from)?;
    if request.lde_log2 != ZK_X509_CA_FRI_LDE_LOG2_V1
        || request.mask_degree != CA_MASK_DEGREE_V1
        || envelope.mask_coefficients != CA_MASK_DEGREE_V1 + 1
    {
        return Err(ZkX509CaAccumulatorProofErrorV1::Resource);
    }
    let layout = ca_aggregate_layout_v1()?;
    let base_columns = ca_base_columns_v1(trace)?;
    let fixed_columns =
        compile_ca_accumulator_fixed_columns_v1().map_err(ZkX509CaAccumulatorProofErrorV1::from)?;
    let mut checked_rng = HealthCheckedTryCryptoRngV1::new(rng).map_err(|error| match error {
        TryCryptoProverRandomnessErrorV1::Unavailable => {
            ZkX509CaAccumulatorProofErrorV1::RandomnessUnavailable
        }
        TryCryptoProverRandomnessErrorV1::Unhealthy => {
            ZkX509CaAccumulatorProofErrorV1::RandomnessUnhealthy
        }
    })?;
    let base_lde = ca_masked_lde_columns_v1(
        &base_columns,
        ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1,
        &mut checked_rng,
    )?;
    let base_tree = aggregate::row_tree_v1(
        CA_BASE_LEAF_DOMAIN_V1,
        CA_BASE_NODE_DOMAIN_V1,
        0,
        &base_lde,
        layout.common_lde_size(),
    )
    .map_err(map_aggregate_proof_error_v1)?;
    let mut trace_group_proofs = vec![aggregate::AggregateTraceGroupProofV1 {
        base_root: base_tree.root(),
        aux_root: [0; 32],
        base_frontier: Vec::new(),
        aux_frontier: Vec::new(),
    }];
    let mut transcript = new_ca_transcript_v1(public, sha_schedule, &layout)?;
    aggregate::absorb_base_roots_v1(
        &mut transcript,
        CA_AGGREGATE_DOMAINS_V1,
        &trace_group_proofs,
    )
    .map_err(map_aggregate_proof_error_v1)?;
    let credential_pre_aux = derive_zk_x509_credential_pre_aux_binding_v1(
        credential_main_pre_aux,
        ca_profile_digest_v1()?,
        ca_public_digest_v1(public, sha_schedule)?,
        trace_group_proofs[0].base_root,
    )
    .map_err(map_credential_pre_aux_error_v1)?;
    absorb_zk_x509_credential_pre_aux_binding_v1(&mut transcript, credential_pre_aux)
        .map_err(map_credential_pre_aux_error_v1)?;
    let sha_challenges = credential_pre_aux.sha();
    let io_challenges = credential_pre_aux.rfc5280();
    let material =
        build_ca_accumulator_stark_material_v1(trace, sha_schedule, sha_challenges, io_challenges)
            .map_err(ZkX509CaAccumulatorProofErrorV1::from)?;
    if material.base_columns != base_columns || material.fixed_columns != fixed_columns {
        return Err(ZkX509CaAccumulatorProofErrorV1::InvalidStatementOrWitness);
    }
    let claims = ca_accumulator_stark_terminal_claims_v1(&material);
    ca_binding_from_claims_v1(public, sha_schedule, claims)?;
    let aux_lde = ca_masked_lde_columns_v1(
        &material.aux_columns,
        ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1,
        &mut checked_rng,
    )?;
    let aux_tree = aggregate::row_tree_v1(
        CA_AUX_LEAF_DOMAIN_V1,
        CA_AUX_NODE_DOMAIN_V1,
        0,
        &aux_lde,
        layout.common_lde_size(),
    )
    .map_err(map_aggregate_proof_error_v1)?;
    trace_group_proofs[0].aux_root = aux_tree.root();
    aggregate::absorb_aux_roots_v1(
        &mut transcript,
        CA_AGGREGATE_DOMAINS_V1,
        &trace_group_proofs,
    )
    .map_err(map_aggregate_proof_error_v1)?;
    absorb_ca_terminal_claims_v1(&mut transcript, claims)?;
    let alphas = derive_ca_constraint_alphas_v1(&mut transcript)?;
    let fixed_lde = ca_fixed_lde_columns_v1(&fixed_columns)?;
    let compositions = ca_composition_lanes_v1(
        public,
        &base_lde,
        &aux_lde,
        &fixed_lde,
        sha_challenges,
        io_challenges,
        claims,
        &alphas,
        &layout,
    )?;
    let mut composition_trees = Vec::new();
    let mut composition_roots = Vec::new();
    for (lane, chunks) in compositions.iter().enumerate() {
        let tree = aggregate::composition_tree_v1(CA_AGGREGATE_DOMAINS_V1, lane, chunks)
            .map_err(map_aggregate_proof_error_v1)?;
        composition_roots.push(tree.root());
        composition_trees.push(tree);
    }
    aggregate::absorb_composition_roots_v1(
        &mut transcript,
        CA_AGGREGATE_PARAMETERS_V1,
        CA_AGGREGATE_DOMAINS_V1,
        &composition_roots,
    )
    .map_err(map_aggregate_proof_error_v1)?;
    let fri_masks =
        aggregate::build_fri_mask_oracles_v1(CA_AGGREGATE_PARAMETERS_V1, &layout, &mut checked_rng)
            .map_err(map_aggregate_proof_error_v1)?;
    let fri_mask_roots = fri_masks
        .iter()
        .map(|mask| mask.tree.root())
        .collect::<Vec<_>>();
    aggregate::absorb_fri_mask_roots_v1(
        &mut transcript,
        CA_AGGREGATE_PARAMETERS_V1,
        &fri_mask_roots,
    )
    .map_err(map_aggregate_proof_error_v1)?;
    let trace_materials = vec![aggregate::AggregateTraceGroupMaterialV1 {
        base_lde,
        aux_lde,
        base_tree,
        aux_tree,
    }];
    let deep_point =
        aggregate::derive_deep_point_v1(&mut transcript, CA_AGGREGATE_PARAMETERS_V1, &layout)
            .map_err(map_aggregate_proof_error_v1)?;
    let deep = aggregate::build_materialized_deep_proof_v1(
        &trace_materials,
        &compositions,
        CA_AGGREGATE_PARAMETERS_V1,
        &layout,
        deep_point,
    )
    .map_err(map_aggregate_proof_error_v1)?;
    aggregate::absorb_deep_openings_v1(&mut transcript, &deep, CA_AGGREGATE_PARAMETERS_V1, &layout)
        .map_err(map_aggregate_proof_error_v1)?;
    let deep_mixes = derive_ca_deep_mixes_v1(&mut transcript, &layout)?;
    let mut fri_materials = Vec::new();
    for lane in 0..CA_SECURITY_LANES_V1 {
        let mut base = ca_fri_base_v1(
            &trace_materials[0],
            &compositions[lane],
            &deep,
            deep_point,
            &deep_mixes[lane],
            &layout,
        )?;
        aggregate::add_fri_mask_oracle_v1(&mut base, &fri_masks[lane])
            .map_err(map_aggregate_proof_error_v1)?;
        fri_materials.push(
            aggregate::build_fri_lane_v1(
                CA_AGGREGATE_PARAMETERS_V1,
                CA_AGGREGATE_DOMAINS_V1,
                &layout,
                lane,
                base,
                &mut transcript,
            )
            .map_err(map_aggregate_proof_error_v1)?,
        );
    }
    let grinding_state = transcript.state();
    let grinding_nonce = grind_nonce_v1(&grinding_state, ZK_X509_GRINDING_BITS_V1)
        .map_err(map_transparent_proof_error_v1)?;
    absorb_ca_grinding_nonce_v1(&mut transcript, grinding_nonce)?;
    let query_indices = aggregate::query_indices_v1(
        &transcript,
        CA_AGGREGATE_PARAMETERS_V1,
        CA_AGGREGATE_DOMAINS_V1,
        &layout,
    )
    .map_err(map_aggregate_proof_error_v1)?;
    let queries = query_indices
        .iter()
        .copied()
        .map(|index| {
            aggregate::build_query_v1(
                CA_AGGREGATE_PARAMETERS_V1,
                &layout,
                index,
                &trace_materials,
                &compositions,
                &fri_masks,
                &fri_materials,
            )
            .map_err(map_aggregate_proof_error_v1)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (trace_frontiers, composition_frontiers, fri_mask_frontiers, fri_round_frontiers) =
        aggregate::build_all_frontiers_v1(
            CA_AGGREGATE_PARAMETERS_V1,
            &layout,
            &queries,
            &trace_materials,
            &composition_trees,
            &fri_masks,
            &fri_materials,
        )
        .map_err(map_aggregate_proof_error_v1)?;
    for (group, (base_frontier, aux_frontier)) in trace_group_proofs.iter_mut().zip(trace_frontiers)
    {
        group.base_frontier = base_frontier;
        group.aux_frontier = aux_frontier;
    }
    let proof = aggregate::AggregateStarkProofV1 {
        version: ZK_X509_PROOF_VERSION_V1,
        trace_groups: trace_group_proofs,
        composition_roots,
        composition_frontiers,
        fri_mask_roots,
        fri_mask_frontiers,
        fri_lanes: fri_materials
            .into_iter()
            .zip(fri_round_frontiers)
            .map(
                |(lane, round_frontiers)| aggregate::AggregateFriLaneProofV1 {
                    roots: lane.roots,
                    terminal_values: lane
                        .terminal_values
                        .into_iter()
                        .map(|value| value.coefficients().map(F::value))
                        .collect(),
                    round_frontiers,
                },
            )
            .collect(),
        queries,
        grinding_nonce,
    };
    let inner =
        aggregate::encode_proof_with_deep_v1(&proof, &deep, CA_AGGREGATE_PARAMETERS_V1, &layout)
            .map_err(map_aggregate_proof_error_v1)?;
    let encoded = encode_ca_proof_envelope_v1(claims, &inner)?;
    verify_ca_accumulator_and_binding_v1(public, sha_schedule, credential_main_pre_aux, &encoded)
        .map_err(|_| ZkX509CaAccumulatorProofErrorV1::ProverSelfCheckFailed)?;
    Ok(encoded)
}
/// Construct the canonical dedicated compact-CA proof with operating-system
/// cryptographic entropy.
#[cfg(test)]
pub(crate) fn prove_zk_x509_ca_accumulator_stark_v1(
    trace: &ZkX509CaAccumulatorTraceV1,
    sha_schedule: &ZkX509ShaCallScheduleV1,
    credential_main_pre_aux: ZkX509CredentialMainPreAuxV1,
) -> Result<Vec<u8>, ZkX509CaAccumulatorProofErrorV1> {
    prove_zk_x509_ca_accumulator_stark_v1_with_rng(
        trace,
        sha_schedule,
        credential_main_pre_aux,
        &mut OsRng,
    )
}
/// Decode the sole compact-CA base root needed by the joint X5S1 schedule.
///
/// Decoding establishes canonical shape and bounds but is not proof
/// verification. An outer verifier may use this root to construct the shared
/// pre-auxiliary schedule only if it subsequently accepts
/// [`verify_zk_x509_ca_accumulator_stark_v1`] for the same exact proof bytes.
pub(crate) fn ca_accumulator_base_root_from_proof_v1(
    proof_bytes: &[u8],
) -> Result<[u8; 32], ZkX509CaAccumulatorProofErrorV1> {
    let layout = ca_aggregate_layout_v1()?;
    let (_, inner) = decode_ca_proof_envelope_v1(proof_bytes)?;
    let (proof, _) =
        aggregate::decode_proof_with_deep_v1(inner, CA_AGGREGATE_PARAMETERS_V1, &layout)
            .map_err(map_aggregate_proof_error_v1)?;
    proof
        .trace_groups
        .first()
        .filter(|_| proof.trace_groups.len() == 1)
        .map(|group| group.base_root)
        .ok_or(ZkX509CaAccumulatorProofErrorV1::MalformedProof)
}
fn verify_ca_accumulator_and_binding_v1(
    public: ZkX509CaAccumulatorStarkPublicV1,
    sha_schedule: &ZkX509ShaCallScheduleV1,
    credential_main_pre_aux: ZkX509CredentialMainPreAuxV1,
    proof_bytes: &[u8],
) -> Result<ZkX509CaAccumulatorSubproofBindingV1, ZkX509CaAccumulatorProofErrorV1> {
    validate_ca_proof_public_v1(public, sha_schedule)?;
    let request = ca_accumulator_resource_request_v1(
        ZK_X509_CA_ACCUMULATOR_REDUCED_AIR_DEGREE_V1,
        1,
        CA_QUERY_COUNT_V1,
    )
    .map_err(ZkX509CaAccumulatorProofErrorV1::from)?;
    checked_ca_accumulator_resource_envelope_v1(request)
        .map_err(ZkX509CaAccumulatorProofErrorV1::from)?;
    let layout = ca_aggregate_layout_v1()?;
    let (claims, inner) = decode_ca_proof_envelope_v1(proof_bytes)?;
    let binding = ca_binding_from_claims_v1(public, sha_schedule, claims)?;
    let (proof, deep) =
        aggregate::decode_proof_with_deep_v1(inner, CA_AGGREGATE_PARAMETERS_V1, &layout)
            .map_err(map_aggregate_proof_error_v1)?;
    let mut transcript = new_ca_transcript_v1(public, sha_schedule, &layout)?;
    aggregate::absorb_base_roots_v1(
        &mut transcript,
        CA_AGGREGATE_DOMAINS_V1,
        &proof.trace_groups,
    )
    .map_err(map_aggregate_proof_error_v1)?;
    let ca_base_root = proof
        .trace_groups
        .first()
        .filter(|_| proof.trace_groups.len() == 1)
        .ok_or(ZkX509CaAccumulatorProofErrorV1::MalformedProof)?
        .base_root;
    let credential_pre_aux = derive_zk_x509_credential_pre_aux_binding_v1(
        credential_main_pre_aux,
        ca_profile_digest_v1()?,
        ca_public_digest_v1(public, sha_schedule)?,
        ca_base_root,
    )
    .map_err(map_credential_pre_aux_error_v1)?;
    absorb_zk_x509_credential_pre_aux_binding_v1(&mut transcript, credential_pre_aux)
        .map_err(map_credential_pre_aux_error_v1)?;
    let sha_challenges = credential_pre_aux.sha();
    let io_challenges = credential_pre_aux.rfc5280();
    aggregate::absorb_aux_roots_v1(
        &mut transcript,
        CA_AGGREGATE_DOMAINS_V1,
        &proof.trace_groups,
    )
    .map_err(map_aggregate_proof_error_v1)?;
    absorb_ca_terminal_claims_v1(&mut transcript, claims)?;
    let alphas = derive_ca_constraint_alphas_v1(&mut transcript)?;
    aggregate::absorb_composition_roots_v1(
        &mut transcript,
        CA_AGGREGATE_PARAMETERS_V1,
        CA_AGGREGATE_DOMAINS_V1,
        &proof.composition_roots,
    )
    .map_err(map_aggregate_proof_error_v1)?;
    aggregate::absorb_fri_mask_roots_v1(
        &mut transcript,
        CA_AGGREGATE_PARAMETERS_V1,
        &proof.fri_mask_roots,
    )
    .map_err(map_aggregate_proof_error_v1)?;
    let deep_point =
        aggregate::derive_deep_point_v1(&mut transcript, CA_AGGREGATE_PARAMETERS_V1, &layout)
            .map_err(map_aggregate_proof_error_v1)?;
    aggregate::absorb_deep_openings_v1(&mut transcript, &deep, CA_AGGREGATE_PARAMETERS_V1, &layout)
        .map_err(map_aggregate_proof_error_v1)?;
    let deep_mixes = derive_ca_deep_mixes_v1(&mut transcript, &layout)?;
    let (fri_betas, terminals) = aggregate::verify_fri_commitments_v1(
        &proof,
        CA_AGGREGATE_PARAMETERS_V1,
        CA_AGGREGATE_DOMAINS_V1,
        &layout,
        &mut transcript,
    )
    .map_err(map_aggregate_proof_error_v1)?;
    let grinding_state = transcript.state();
    verify_grinding_nonce_v1(
        &grinding_state,
        ZK_X509_GRINDING_BITS_V1,
        proof.grinding_nonce,
    )
    .map_err(|_| ZkX509CaAccumulatorProofErrorV1::TranscriptMismatch)?;
    absorb_ca_grinding_nonce_v1(&mut transcript, proof.grinding_nonce)?;
    let expected_indices = aggregate::query_indices_v1(
        &transcript,
        CA_AGGREGATE_PARAMETERS_V1,
        CA_AGGREGATE_DOMAINS_V1,
        &layout,
    )
    .map_err(map_aggregate_proof_error_v1)?;
    aggregate::verify_all_merkle_openings_v1(
        &proof,
        CA_AGGREGATE_PARAMETERS_V1,
        CA_AGGREGATE_DOMAINS_V1,
        &layout,
        &expected_indices,
    )
    .map_err(map_aggregate_proof_error_v1)?;
    let fixed_columns =
        compile_ca_accumulator_fixed_columns_v1().map_err(ZkX509CaAccumulatorProofErrorV1::from)?;
    let fixed_lde = ca_fixed_lde_columns_v1(&fixed_columns)?;
    let lde_root = goldilocks_primitive_root_v1(layout.common_lde_log2())
        .map_err(map_transparent_proof_error_v1)?;
    let mut evaluator = CaOpenedRowEvaluatorV1 {
        public,
        fixed_lde: &fixed_lde,
        sha_challenges,
        io_challenges,
        claims,
        alphas: &alphas,
        lde_root,
        rows: layout.common_lde_size(),
    };
    aggregate::verify_opened_query_relations_with_deep_v1(
        &proof,
        &deep,
        deep_point,
        &deep_mixes,
        CA_AGGREGATE_PARAMETERS_V1,
        &layout,
        &expected_indices,
        &fri_betas,
        &terminals,
        &mut evaluator,
    )
    .map_err(map_aggregate_proof_error_v1)?;
    Ok(binding)
}
/// Verify the exact canonical compact-CA proof against verifier-owned public
/// input and SHA schedule.
#[cfg(test)]
pub(crate) fn verify_zk_x509_ca_accumulator_stark_v1(
    public: ZkX509CaAccumulatorStarkPublicV1,
    sha_schedule: &ZkX509ShaCallScheduleV1,
    credential_main_pre_aux: ZkX509CredentialMainPreAuxV1,
    proof_bytes: &[u8],
) -> Result<(), ZkX509CaAccumulatorProofErrorV1> {
    verify_ca_accumulator_and_binding_v1(public, sha_schedule, credential_main_pre_aux, proof_bytes)
        .map(|_| ())
}
/// Verify and return the exact typed cross-subproof terminal binding.
pub(crate) fn ca_accumulator_subproof_binding_from_proof_v1(
    public: ZkX509CaAccumulatorStarkPublicV1,
    sha_schedule: &ZkX509ShaCallScheduleV1,
    credential_main_pre_aux: ZkX509CredentialMainPreAuxV1,
    proof_bytes: &[u8],
) -> Result<ZkX509CaAccumulatorSubproofBindingV1, ZkX509CaAccumulatorProofErrorV1> {
    verify_ca_accumulator_and_binding_v1(public, sha_schedule, credential_main_pre_aux, proof_bytes)
}
/// Verify then hash the canonical proof together with its exact public
/// statement and verifier-owned schedule for the outer X5S1 envelope.
#[cfg(test)]
pub(crate) fn ca_accumulator_proof_binding_digest_v1(
    public: ZkX509CaAccumulatorStarkPublicV1,
    sha_schedule: &ZkX509ShaCallScheduleV1,
    credential_main_pre_aux: ZkX509CredentialMainPreAuxV1,
    proof_bytes: &[u8],
) -> Result<[u8; 32], ZkX509CaAccumulatorProofErrorV1> {
    verify_ca_accumulator_and_binding_v1(
        public,
        sha_schedule,
        credential_main_pre_aux,
        proof_bytes,
    )?;
    let ca_base_root = ca_accumulator_base_root_from_proof_v1(proof_bytes)?;
    let credential_pre_aux = derive_zk_x509_credential_pre_aux_binding_v1(
        credential_main_pre_aux,
        ca_profile_digest_v1()?,
        ca_public_digest_v1(public, sha_schedule)?,
        ca_base_root,
    )
    .map_err(map_credential_pre_aux_error_v1)?;
    let public_digest = ca_public_digest_v1(public, sha_schedule)?;
    sha256_frame_v1(
        CA_BINDING_DIGEST_DOMAIN_V1,
        &[
            &public_digest,
            &credential_pre_aux.transcript_state(),
            proof_bytes,
        ],
    )
    .map_err(map_transparent_proof_error_v1)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::privacy_engines::zk_x509::{
        accumulator_air::{
            ZK_X509_CA_ACCUMULATOR_IO_START_V1, ZkX509CaAccumulatorStatementV1,
            ZkX509CaAccumulatorWitnessV1, build_ca_accumulator_trace_v1,
        },
        merkle::{
            ZK_X509_CA_SPKI_DER_BYTES_V1, ca_membership_path_from_complete_spkis_v1,
            ca_root_from_complete_spkis_v1,
        },
        sha_call_bus_stark::{ZkX509ShaCallBusLaneChallengesV1, ZkX509ShaCallPublicShapeV1},
    };
    use rand::{SeedableRng as _, TryCryptoRng, TryRngCore, rngs::StdRng};
    use std::sync::OnceLock;
    fn spki(index: u16) -> [u8; ZK_X509_CA_SPKI_DER_BYTES_V1] {
        let mut spki = [0x42; ZK_X509_CA_SPKI_DER_BYTES_V1];
        spki[..2].copy_from_slice(&index.to_be_bytes());
        spki
    }
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
    fn io_challenges() -> ZkX509Rfc5280StarkChallengesV1 {
        let mut next = 1_001_u64;
        ZkX509Rfc5280StarkChallengesV1 {
            tuple: core::array::from_fn(|_| {
                core::array::from_fn(|_| {
                    let value = F(next);
                    next += 2;
                    value
                })
            }),
        }
    }
    fn fixture() -> (
        ZkX509CaAccumulatorTraceV1,
        ZkX509ShaCallScheduleV1,
        ZkX509ShaCallBusChallengesV1,
        ZkX509Rfc5280StarkChallengesV1,
    ) {
        let members = [spki(7), spki(1), spki(9), spki(3)];
        let refs = members
            .iter()
            .map(|member: &[u8; ZK_X509_CA_SPKI_DER_BYTES_V1]| member.as_slice())
            .collect::<Vec<_>>();
        let root = ca_root_from_complete_spkis_v1(&refs).expect("root");
        let path = ca_membership_path_from_complete_spkis_v1(&refs, &members[0]).expect("path");
        let trace = build_ca_accumulator_trace_v1(
            ZkX509CaAccumulatorStatementV1 {
                governed_root: root,
            },
            ZkX509CaAccumulatorWitnessV1 {
                root_spki_der: members[0],
                path,
            },
        )
        .expect("trace");
        let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 4,
        })
        .expect("schedule");
        (trace, schedule, challenges(), io_challenges())
    }
    fn credential_main_pre_aux_v1() -> ZkX509CredentialMainPreAuxV1 {
        ZkX509CredentialMainPreAuxV1::fixture_for_test_v1(
            [0x91; 32],
            [0x92; 32],
            core::array::from_fn(|index| {
                [u8::try_from(0xa0 + index).expect("fixture root byte"); 32]
            }),
        )
    }
    fn row(columns: &[Vec<F>], index: usize) -> Vec<F> {
        columns.iter().map(|column| column[index]).collect()
    }
    #[derive(Debug)]
    struct InjectedEntropyError;
    impl core::fmt::Display for InjectedEntropyError {
        fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            formatter.write_str("injected compact-CA prover entropy failure")
        }
    }
    #[derive(Clone, Copy)]
    enum EntropyMode {
        Period(usize),
        PartialFailure,
        Panic,
    }
    struct AdversarialEntropy(EntropyMode);
    impl TryRngCore for AdversarialEntropy {
        type Error = InjectedEntropyError;
        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Err(InjectedEntropyError)
        }
        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Err(InjectedEntropyError)
        }
        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
            match self.0 {
                EntropyMode::Period(period) => {
                    for (index, byte) in destination.iter_mut().enumerate() {
                        *byte = ((index % period) as u8).wrapping_mul(73).wrapping_add(11);
                    }
                    Ok(())
                }
                EntropyMode::PartialFailure => {
                    for (index, byte) in destination.iter_mut().take(17).enumerate() {
                        *byte = index as u8;
                    }
                    Err(InjectedEntropyError)
                }
                EntropyMode::Panic => panic!("entropy touched before deterministic preflight"),
            }
        }
    }
    impl TryCryptoRng for AdversarialEntropy {}
    fn canonical_proof_fixture() -> &'static (
        ZkX509CaAccumulatorStarkPublicV1,
        ZkX509ShaCallScheduleV1,
        Vec<u8>,
    ) {
        static FIXTURE: OnceLock<(
            ZkX509CaAccumulatorStarkPublicV1,
            ZkX509ShaCallScheduleV1,
            Vec<u8>,
        )> = OnceLock::new();
        FIXTURE.get_or_init(|| {
            let (trace, schedule, _, _) = fixture();
            let public =
                ca_accumulator_stark_public_v1(&trace, &schedule).expect("canonical public");
            let mut rng = StdRng::seed_from_u64(0xCA11_ACC0_5A17_0001);
            let proof = prove_zk_x509_ca_accumulator_stark_v1_with_rng(
                &trace,
                &schedule,
                credential_main_pre_aux_v1(),
                &mut rng,
            )
            .expect("canonical compact-CA proof");
            (public, schedule, proof)
        })
    }
    #[test]
    fn dimensions_calls_and_terminals_are_exact() {
        let (trace, schedule, sha_challenges, io_challenges) = fixture();
        let material = build_ca_accumulator_stark_material_v1(
            &trace,
            &schedule,
            sha_challenges,
            io_challenges,
        )
        .expect("material");
        assert_eq!(material.base_columns.len(), 695);
        assert_eq!(material.aux_columns.len(), 128);
        assert_eq!(material.fixed_columns.len(), 80);
        assert!(
            material
                .base_columns
                .iter()
                .chain(&material.aux_columns)
                .chain(&material.fixed_columns)
                .all(|column| column.len() == 128)
        );
        assert_eq!(material.terminals.len(), 13);
        assert_eq!(material.terminals[0].call, 16);
        assert_eq!(material.terminals[0].role, ZkX509ShaCallRoleV1::CaLeaf);
        assert_eq!(material.terminals[12].call, 28);
        assert_eq!(material.terminals[12].role, ZkX509ShaCallRoleV1::CaNode(11));
        assert_eq!(material.root_spki_terminal.channel, 36);
        assert_eq!(
            material.root_spki_terminal.event_count,
            ZK_X509_CA_ACCUMULATOR_ROOT_SPKI_IO_EVENTS_V1
        );
        let leaf_words =
            padded_source_words_v1(&trace.hash_witnesses[0].message).expect("leaf words");
        for lane in 0..ZK_X509_SHA_BUS_LANES_V1 {
            let expected_source =
                leaf_words
                    .iter()
                    .copied()
                    .enumerate()
                    .fold(F::ONE, |product, (word, value)| {
                        product.mul(compress_sha_call_fields_v1(
                            F(ZK_X509_SHA_CA_LEAF_CALL_V1 as u64),
                            F(u64::from(ZkX509ShaCallRoleV1::CaLeaf.role_code())),
                            F::ZERO,
                            F(u64::from(ZkX509ShaCallWordKindV1::Input.code())),
                            F(u64::try_from(word).expect("word")),
                            F(u64::from(value)),
                            sha_challenges.lanes[lane],
                        ))
                    });
            assert_eq!(material.terminals[0].source_products[lane], expected_source);
            let expected_io = trace
                .witness
                .root_spki_der
                .iter()
                .copied()
                .enumerate()
                .try_fold(F::ONE, |product, (offset, value)| {
                    Ok::<_, ZkX509AccumulatorStarkErrorV1>(product.mul(root_spki_io_factor_v1(
                        ca_accumulator_stark_public_v1(&trace, &schedule)?,
                        F(u64::from(value)),
                        F(u64::try_from(offset).expect("offset")),
                        lane,
                        io_challenges,
                    )?))
                })
                .expect("I/O product");
            assert_eq!(
                material.root_spki_terminal.consumer_products[lane],
                expected_io
            );
        }
        assert_eq!(ZK_X509_CA_ACCUMULATOR_CHUNKS_V1, 13);
    }
    #[test]
    fn halkindi_parameterized_resource_envelope_is_exact_and_common_lifting_fails() {
        for (reduced_air_degree, deep_queries, fri_queries, expected_mask_coefficients) in
            [(2, 0, 60, 300), (2, 1, 60, 316), (2, 2, 60, 332)]
        {
            let request =
                ca_accumulator_resource_request_v1(reduced_air_degree, deep_queries, fri_queries)
                    .expect("bounded cubic-AIR resource request");
            let envelope =
                checked_ca_accumulator_resource_envelope_v1(request).expect("safe local envelope");
            let lde_rows = 1_usize << request.lde_log2;
            let expected_masked_degree =
                ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1 + expected_mask_coefficients - 1;
            assert_eq!(request.mask_degree + 1, expected_mask_coefficients);
            assert_eq!(envelope.mask_coefficients, expected_mask_coefficients);
            assert_eq!(envelope.maximum_masked_trace_degree, expected_masked_degree);
            assert_eq!(
                envelope.minimum_safe_lde_rows,
                (expected_masked_degree + 1) * ZK_X509_CA_ACCUMULATOR_FRI_RATE_DENOMINATOR_V1
            );
            assert_eq!(
                envelope.fri_degree_cap,
                lde_rows / ZK_X509_CA_ACCUMULATOR_FRI_RATE_DENOMINATOR_V1 - 1
            );
            assert_eq!(
                envelope.maximum_quotient_degree,
                usize::from(ZK_X509_CA_ACCUMULATOR_CONSTRAINT_DEGREE_V1) * expected_masked_degree
                    + 2 * (ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1 - 1)
                    - ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1
            );
            assert!(
                envelope.maximum_quotient_degree
                    < (envelope.fri_degree_cap + 1) * COMPOSITION_DEGREE_CHUNKS_V1
            );
            assert_eq!(envelope.native_material_field_cells, 115_584);
            assert_eq!(envelope.native_material_bytes, 924_672);
            assert_eq!(
                envelope.committed_lde_field_evaluations,
                (ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1 + ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1)
                    * lde_rows
            );
            assert_eq!(
                envelope.fixed_lde_field_evaluations,
                ZK_X509_CA_ACCUMULATOR_FIXED_WIDTH_V1 * lde_rows
            );
            assert_eq!(
                envelope.trace_mask_bytes,
                (ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1 + ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1)
                    * expected_mask_coefficients
                    * FIELD_BYTES_V1
            );
        }
        let oversized_query_request =
            ca_accumulator_resource_request_v1(2, 1, 108).expect("arithmetic high-query request");
        assert_eq!(oversized_query_request.mask_degree + 1, 556);
        assert_eq!(oversized_query_request.lde_log2, 15);
        assert_eq!(
            checked_ca_accumulator_resource_envelope_v1(oversized_query_request),
            Err(ZkX509AccumulatorStarkErrorV1::Resource),
            "the materialized log15 LDE must not bypass the 128 MiB adapter cap"
        );
        // The compiled AIR is cubic, hence Haböck--Al Kindi uses d=d_AIR-1=2.
        // The outer theorem still chooses the exact DEEP and FRI counts.
        let request = ca_accumulator_resource_request_v1(2, 1, CA_QUERY_COUNT_V1)
            .expect("release resource census");
        assert_eq!(request.lde_log2, 14);
        let envelope =
            checked_ca_accumulator_resource_envelope_v1(request).expect("release resource census");
        assert_eq!(request.reduced_air_degree, 2);
        assert_eq!(request.fri_query_count, CA_QUERY_COUNT_V1);
        assert_eq!(envelope.mask_coefficients, 306);
        assert_eq!(envelope.maximum_masked_trace_degree, 433);
        assert_eq!(envelope.fri_degree_cap, 511);
        assert_eq!(envelope.maximum_quotient_degree, 1_425);
        assert_eq!(envelope.minimum_safe_lde_rows, 13_888);
        assert_eq!(envelope.total_local_lde_bytes, 118_358_016);
        assert_eq!(envelope.encrypted_scratch_bytes, 120_207_360);
        assert_eq!(envelope.current_next_block_bytes, 1_849_344);
        assert_eq!(envelope.streamed_column_bytes, 132_096);
        assert_eq!(envelope.trace_mask_bytes, 2_014_704);
        assert_eq!(envelope.composition_residue_evaluations, 22_593_536);
        assert_eq!(envelope.composition_component_evaluations, 90_374_144);
        assert_eq!(envelope.lde_butterflies, 103_967_808);
        assert_eq!(envelope.adapter_resident_payload_bytes, 124_862_728);
        assert!(
            envelope.adapter_resident_payload_bytes >= envelope.total_local_lde_bytes,
            "the materialized first-release LDE must be counted as resident"
        );
        assert_eq!(
            ZK_X509_CA_ACCUMULATOR_MAX_ADAPTER_RESIDENT_BYTES_V1,
            128 << 20
        );
        assert_eq!(
            ca_accumulator_resource_request_v1(0, 0, 60),
            Err(ZkX509AccumulatorStarkErrorV1::Resource)
        );
        assert_eq!(
            ca_accumulator_resource_request_v1(2, 0, 0),
            Err(ZkX509AccumulatorStarkErrorV1::Resource)
        );
        assert_eq!(
            ca_accumulator_resource_request_v1(1, 1, 60),
            Err(ZkX509AccumulatorStarkErrorV1::Resource),
            "a quadratic-AIR substitution must fail for the compiled cubic AIR"
        );
        assert_eq!(
            ca_accumulator_resource_request_v1(2, usize::MAX, usize::MAX),
            Err(ZkX509AccumulatorStarkErrorV1::Resource)
        );
        let excessive =
            ca_accumulator_resource_request_v1(2, 1, 1_000).expect("arithmetic request");
        assert_eq!(
            checked_ca_accumulator_resource_envelope_v1(excessive),
            Err(ZkX509AccumulatorStarkErrorV1::Resource)
        );
        let quadratic_geometry =
            transparent_stark_zk_mask_geometry_v1(1, 4, 1, 60).expect("quadratic census only");
        let quadratic_substitution = ZkX509CaAccumulatorResourceRequestV1 {
            reduced_air_degree: 1,
            mask_degree: quadratic_geometry.minimum_mask_degree,
            ..request
        };
        assert_eq!(
            checked_ca_accumulator_resource_envelope_v1(quadratic_substitution),
            Err(ZkX509AccumulatorStarkErrorV1::Resource),
            "matching a quadratic mask must not downgrade the actual cubic AIR"
        );
        for hostile in [
            ZkX509CaAccumulatorResourceRequestV1 {
                lde_log2: 22,
                ..request
            },
            ZkX509CaAccumulatorResourceRequestV1 {
                base_width: ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1 + 1,
                ..request
            },
            ZkX509CaAccumulatorResourceRequestV1 {
                scratch_chunk_rows: 1 << 15,
                ..request
            },
            ZkX509CaAccumulatorResourceRequestV1 {
                composition_extension_lanes: 3,
                ..request
            },
            ZkX509CaAccumulatorResourceRequestV1 {
                mask_degree: request.mask_degree - 1,
                ..request
            },
            ZkX509CaAccumulatorResourceRequestV1 {
                fri_rate_denominator: 16,
                ..request
            },
            ZkX509CaAccumulatorResourceRequestV1 {
                deep_query_count: 0,
                ..request
            },
            ZkX509CaAccumulatorResourceRequestV1 {
                fri_query_count: 61,
                ..request
            },
        ] {
            assert_eq!(
                checked_ca_accumulator_resource_envelope_v1(hostile),
                Err(ZkX509AccumulatorStarkErrorV1::Resource)
            );
        }
    }
    #[test]
    fn typed_outer_binding_rejects_omission_substitution_reorder_and_cross_root() {
        let (trace, schedule, sha_challenges, io_challenges) = fixture();
        let material = build_ca_accumulator_stark_material_v1(
            &trace,
            &schedule,
            sha_challenges,
            io_challenges,
        )
        .expect("material");
        let binding =
            ca_accumulator_subproof_binding_v1(&trace, &schedule, &material).expect("binding");
        validate_ca_accumulator_subproof_binding_v1(binding.public, &schedule, binding)
            .expect("canonical binding");
        assert_eq!(
            ca_accumulator_subproof_terminal_claims_v1(binding),
            ca_accumulator_stark_terminal_claims_v1(&material)
        );
        assert_eq!(
            validate_ca_accumulator_subproof_terminal_sequence_v1(
                binding.public,
                &schedule,
                &binding.sha_terminals[..binding.sha_terminals.len() - 1],
                binding.root_spki_terminal,
            ),
            Err(ZkX509AccumulatorStarkErrorV1::CallBus)
        );
        let mut extra = binding.sha_terminals.to_vec();
        extra.push(binding.sha_terminals[0]);
        assert_eq!(
            validate_ca_accumulator_subproof_terminal_sequence_v1(
                binding.public,
                &schedule,
                &extra,
                binding.root_spki_terminal,
            ),
            Err(ZkX509AccumulatorStarkErrorV1::CallBus)
        );
        let mut reordered = binding;
        reordered.sha_terminals.swap(1, 2);
        assert_eq!(
            validate_ca_accumulator_subproof_binding_v1(binding.public, &schedule, reordered),
            Err(ZkX509AccumulatorStarkErrorV1::CallBus)
        );
        let mut substituted = binding;
        substituted.sha_terminals[0].source_products[0] =
            substituted.sha_terminals[0].source_products[0].add(F::ONE);
        assert_eq!(
            validate_ca_accumulator_subproof_binding_v1(binding.public, &schedule, substituted),
            Ok(())
        );
        // Metadata validation alone admits canonical challenge products.  The
        // local quotient and the outer shared-SHA equality reject substitution.
        let claims = ca_accumulator_subproof_terminal_claims_v1(substituted);
        let row_index = ZK_X509_CA_ACCUMULATOR_NONPADDING_ROWS_V1 - 1;
        assert!(
            evaluate_ca_accumulator_stark_residues_v1(
                binding.public,
                &row(&material.base_columns, row_index),
                &row(&material.base_columns, row_index + 1),
                &row(&material.aux_columns, row_index),
                &row(&material.aux_columns, row_index + 1),
                &row(&material.fixed_columns, row_index),
                sha_challenges,
                io_challenges,
                claims,
            )
            .expect("residues")
            .iter()
            .any(|residue| *residue != F::ZERO)
        );
        let mut cross_root = binding.public;
        cross_root.governed_root[0] = if cross_root.governed_root[0] == F::ZERO {
            F::ONE
        } else {
            F::ZERO
        };
        assert_eq!(
            validate_ca_accumulator_subproof_binding_v1(cross_root, &schedule, binding),
            Err(ZkX509AccumulatorStarkErrorV1::Witness)
        );
        let mut wrong_channel = binding;
        wrong_channel.public.root_spki_channel = wrong_channel.public.root_spki_channel.add(F::ONE);
        assert_eq!(
            validate_ca_accumulator_subproof_binding_v1(binding.public, &schedule, wrong_channel,),
            Err(ZkX509AccumulatorStarkErrorV1::Witness)
        );
        let mut wrong_rfc = binding;
        wrong_rfc.root_spki_terminal.event_count -= 1;
        assert_eq!(
            validate_ca_accumulator_subproof_binding_v1(binding.public, &schedule, wrong_rfc),
            Err(ZkX509AccumulatorStarkErrorV1::IoBus)
        );
    }
    #[test]
    fn every_canonical_opened_row_has_exactly_1379_zero_residues() {
        let (trace, schedule, sha_challenges, io_challenges) = fixture();
        let public = ca_accumulator_stark_public_v1(&trace, &schedule).expect("public");
        let material = build_ca_accumulator_stark_material_v1(
            &trace,
            &schedule,
            sha_challenges,
            io_challenges,
        )
        .expect("material");
        let terminal_claims = ca_accumulator_stark_terminal_claims_v1(&material);
        for index in 0..ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1 {
            let next_index = (index + 1) % ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1;
            let residues = evaluate_ca_accumulator_stark_residues_v1(
                public,
                &row(&material.base_columns, index),
                &row(&material.base_columns, next_index),
                &row(&material.aux_columns, index),
                &row(&material.aux_columns, next_index),
                &row(&material.fixed_columns, index),
                sha_challenges,
                io_challenges,
                terminal_claims,
            )
            .expect("residues");
            assert_eq!(residues.len(), 1_379);
            assert!(residues.iter().all(|residue| *residue == F::ZERO));
        }
    }
    #[test]
    fn base_aux_fixed_and_public_mutations_fail_closed() {
        let (trace, schedule, sha_challenges, io_challenges) = fixture();
        let public = ca_accumulator_stark_public_v1(&trace, &schedule).expect("public");
        let material = build_ca_accumulator_stark_material_v1(
            &trace,
            &schedule,
            sha_challenges,
            io_challenges,
        )
        .expect("material");
        let terminal_claims = ca_accumulator_stark_terminal_claims_v1(&material);
        let index = 5;
        let base = row(&material.base_columns, index);
        let next = row(&material.base_columns, index + 1);
        let aux = row(&material.aux_columns, index);
        let next_aux = row(&material.aux_columns, index + 1);
        let fixed = row(&material.fixed_columns, index);
        let rejects = |base: &[F], next: &[F], aux: &[F], next_aux: &[F], fixed: &[F], public| {
            evaluate_ca_accumulator_stark_residues_v1(
                public,
                base,
                next,
                aux,
                next_aux,
                fixed,
                sha_challenges,
                io_challenges,
                terminal_claims,
            )
            .expect("shape")
            .iter()
            .any(|residue| *residue != F::ZERO)
        };
        let mut changed = base.clone();
        changed[CA_SIBLING_START + 3] = changed[CA_SIBLING_START + 3].add(F::ONE);
        assert!(rejects(&changed, &next, &aux, &next_aux, &fixed, public));
        let mut changed = aux.clone();
        changed[77] = changed[77].add(F::ONE);
        assert!(rejects(&base, &next, &changed, &next_aux, &fixed, public));
        let mut changed = fixed.clone();
        changed[FIX_CALL] = changed[FIX_CALL].add(F::ONE);
        assert!(rejects(&base, &next, &aux, &next_aux, &changed, public));
        let padding_index = ZK_X509_CA_ACCUMULATOR_NONPADDING_ROWS_V1;
        let padding_base = row(&material.base_columns, padding_index);
        let padding_next = row(&material.base_columns, padding_index + 1);
        let mut padding_aux = row(&material.aux_columns, padding_index);
        let padding_next_aux = row(&material.aux_columns, padding_index + 1);
        let padding_fixed = row(&material.fixed_columns, padding_index);
        padding_aux[source_aux_cell_v1(1, 0)] = F::ONE;
        assert!(
            evaluate_ca_accumulator_stark_residues_v1(
                public,
                &padding_base,
                &padding_next,
                &padding_aux,
                &padding_next_aux,
                &padding_fixed,
                sha_challenges,
                io_challenges,
                terminal_claims,
            )
            .expect("padding mutation residues")
            .iter()
            .any(|residue| *residue != F::ZERO),
            "ungated product recurrences must still force every inactive state to zero"
        );
        let mut changed = public;
        changed.governed_root[0] = changed.governed_root[0].add(F::ONE);
        let last = 12;
        assert!(rejects(
            &row(&material.base_columns, last),
            &row(&material.base_columns, last + 1),
            &row(&material.aux_columns, last),
            &row(&material.aux_columns, last + 1),
            &row(&material.fixed_columns, last),
            changed,
        ));
    }
    #[test]
    fn root_spki_io_byte_range_order_channel_and_challenge_mutations_fail_closed() {
        let (trace, schedule, sha_challenges, io_challenges) = fixture();
        let public = ca_accumulator_stark_public_v1(&trace, &schedule).expect("public");
        let material = build_ca_accumulator_stark_material_v1(
            &trace,
            &schedule,
            sha_challenges,
            io_challenges,
        )
        .expect("material");
        let terminal_claims = ca_accumulator_stark_terminal_claims_v1(&material);
        let row_index = ZK_X509_CA_ACCUMULATOR_IO_START_V1 + 17;
        let base = row(&material.base_columns, row_index);
        let next = row(&material.base_columns, row_index + 1);
        let aux = row(&material.aux_columns, row_index);
        let next_aux = row(&material.aux_columns, row_index + 1);
        let fixed = row(&material.fixed_columns, row_index);
        let rejects = |base: &[F],
                       public: ZkX509CaAccumulatorStarkPublicV1,
                       io_challenges: ZkX509Rfc5280StarkChallengesV1| {
            evaluate_ca_accumulator_stark_residues_v1(
                public,
                base,
                &next,
                &aux,
                &next_aux,
                &fixed,
                sha_challenges,
                io_challenges,
                terminal_claims,
            )
            .expect("shape")
            .iter()
            .any(|residue| *residue != F::ZERO)
        };
        let mut changed = base.clone();
        changed[CA_IO_BYTE] = changed[CA_IO_BYTE].add(F::ONE);
        assert!(rejects(&changed, public, io_challenges));
        let mut changed = base.clone();
        changed[CA_IO_WORD_ACC] = changed[CA_IO_WORD_ACC].add(F::ONE);
        assert!(rejects(&changed, public, io_challenges));
        let mut out_of_range = base.clone();
        out_of_range[CA_IO_BYTE] = F(256);
        assert!(rejects(&out_of_range, public, io_challenges));
        let reordered = row(&material.base_columns, row_index + 1);
        assert!(rejects(&reordered, public, io_challenges));
        let mut wrong_channel = public;
        wrong_channel.root_spki_channel = wrong_channel.root_spki_channel.add(F::ONE);
        assert!(rejects(&base, wrong_channel, io_challenges));
        let mut wrong_challenges = io_challenges;
        wrong_challenges.tuple[0][6] = wrong_challenges.tuple[0][6].add(F(10_000));
        assert!(rejects(&base, public, wrong_challenges));
        let mut changed_next_aux = next_aux.clone();
        changed_next_aux[serialized_sha_product_cell_v1(0)] =
            changed_next_aux[serialized_sha_product_cell_v1(0)].add(F::ONE);
        assert!(
            evaluate_ca_accumulator_stark_residues_v1(
                public,
                &base,
                &next,
                &aux,
                &changed_next_aux,
                &fixed,
                sha_challenges,
                io_challenges,
                terminal_claims,
            )
            .expect("shape")
            .iter()
            .any(|residue| *residue != F::ZERO)
        );
        let mut changed_next_aux = next_aux;
        changed_next_aux[root_spki_io_product_cell_v1(3)] =
            changed_next_aux[root_spki_io_product_cell_v1(3)].add(F::ONE);
        assert!(
            evaluate_ca_accumulator_stark_residues_v1(
                public,
                &base,
                &next,
                &aux,
                &changed_next_aux,
                &fixed,
                sha_challenges,
                io_challenges,
                terminal_claims,
            )
            .expect("shape")
            .iter()
            .any(|residue| *residue != F::ZERO)
        );
    }
    #[test]
    fn sha_and_root_spki_terminal_claim_mutations_and_reordering_fail_closed() {
        let (trace, schedule, sha_challenges, io_challenges) = fixture();
        let public = ca_accumulator_stark_public_v1(&trace, &schedule).expect("public");
        let material = build_ca_accumulator_stark_material_v1(
            &trace,
            &schedule,
            sha_challenges,
            io_challenges,
        )
        .expect("material");
        let claims = ca_accumulator_stark_terminal_claims_v1(&material);
        let rejects = |row_index: usize, claims: ZkX509CaAccumulatorStarkTerminalClaimsV1| {
            let next_index = (row_index + 1) % ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1;
            evaluate_ca_accumulator_stark_residues_v1(
                public,
                &row(&material.base_columns, row_index),
                &row(&material.base_columns, next_index),
                &row(&material.aux_columns, row_index),
                &row(&material.aux_columns, next_index),
                &row(&material.fixed_columns, row_index),
                sha_challenges,
                io_challenges,
                claims,
            )
            .expect("shape")
            .iter()
            .any(|residue| *residue != F::ZERO)
        };
        let mut changed = claims;
        changed.source_products[0][0] = changed.source_products[0][0].add(F::ONE);
        assert!(rejects(
            ZK_X509_CA_ACCUMULATOR_NONPADDING_ROWS_V1 - 1,
            changed
        ));
        let mut changed = claims;
        changed.digest_products[0][1] = changed.digest_products[0][1].add(F::ONE);
        assert!(rejects(0, changed));
        let mut changed = claims;
        changed.root_spki_consumer_products[2] = changed.root_spki_consumer_products[2].add(F::ONE);
        assert!(rejects(
            ZK_X509_CA_ACCUMULATOR_NONPADDING_ROWS_V1 - 1,
            changed
        ));
        let mut reordered = claims;
        reordered.source_products.swap(1, 2);
        reordered.digest_products.swap(1, 2);
        assert!(rejects(1, reordered));
        let mut invalid_metadata = material.root_spki_terminal;
        invalid_metadata.channel = invalid_metadata.channel.checked_add(1).expect("channel");
        assert_eq!(
            validate_ca_accumulator_io_terminal_v1(public, invalid_metadata),
            Err(ZkX509AccumulatorStarkErrorV1::IoBus)
        );
        invalid_metadata = material.root_spki_terminal;
        invalid_metadata.event_count -= 1;
        assert_eq!(
            validate_ca_accumulator_io_terminal_v1(public, invalid_metadata),
            Err(ZkX509AccumulatorStarkErrorV1::IoBus)
        );
    }
    #[test]
    fn root_spki_channel_is_derived_only_from_public_disclosure_shape() {
        for disclosures in 0..=4 {
            let schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
                disclosed_attributes: disclosures,
            })
            .expect("schedule");
            assert_eq!(
                ca_accumulator_root_spki_channel_v1(&schedule).expect("channel"),
                ZK_X509_CA_ACCUMULATOR_ROOT_SPKI_BASE_CHANNEL_V1
                    + u32::try_from(disclosures * 2).expect("bounded disclosures")
            );
        }
    }
    #[test]
    fn fixed_frame_offsets_and_padding_are_pinned() {
        let leaf = compile_ca_accumulator_fixed_row_v1(0).expect("leaf");
        let node = compile_ca_accumulator_fixed_row_v1(1).expect("node");
        let first_io = compile_ca_accumulator_fixed_row_v1(ZK_X509_CA_ACCUMULATOR_IO_START_V1)
            .expect("first IO");
        let last_io =
            compile_ca_accumulator_fixed_row_v1(ZK_X509_CA_ACCUMULATOR_NONPADDING_ROWS_V1 - 1)
                .expect("last IO");
        let padding = compile_ca_accumulator_fixed_row_v1(ZK_X509_CA_ACCUMULATOR_TRACE_ROWS_V1 - 1)
            .expect("padding");
        assert_eq!(leaf[FIX_CALL], F(16));
        assert_eq!(node[FIX_CALL], F(17));
        assert_eq!(node[FIX_LEVEL], F::ZERO);
        assert_eq!(first_io[FIX_IO_ACTIVE], F::ONE);
        assert_eq!(first_io[FIX_IO_FIRST], F::ONE);
        assert_eq!(first_io[FIX_IO_OFFSET], F::ZERO);
        assert_eq!(last_io[FIX_IO_LAST], F::ONE);
        assert_eq!(last_io[FIX_IO_OFFSET], F(90));
        assert_eq!(padding[FIX_PADDING], F::ONE);
        assert!(
            padding
                .iter()
                .enumerate()
                .all(|(column, value)| column == FIX_PADDING || *value == F::ZERO),
            "the sole padding selector must remain one while every other fixed cell is zero"
        );
        assert_eq!(LEAF_DYNAMIC_OFFSET_V1, 65);
        assert_eq!(NODE_LEFT_DYNAMIC_OFFSET_V1, 75);
        assert_eq!(NODE_RIGHT_DYNAMIC_OFFSET_V1, 115);
    }
    #[test]
    fn dedicated_proof_parameters_and_resource_gate_are_exact() {
        let layout = ca_aggregate_layout_v1().expect("dedicated layout");
        assert_eq!(layout.common_lde_log2(), 14);
        assert_eq!(
            layout
                .fri_rounds(CA_AGGREGATE_PARAMETERS_V1)
                .expect("FRI rounds"),
            5
        );
        assert_eq!(
            layout
                .fri_degree_cap(CA_AGGREGATE_PARAMETERS_V1)
                .expect("FRI degree cap"),
            512
        );
        assert_eq!(CA_MASK_DEGREE_V1 + 1, 306);
        assert_eq!(CA_QUERY_COUNT_V1, 58);
        assert_eq!(CA_DEEP_BYTES_V1, 52_768);
        assert_eq!(CA_CLAIM_FIELDS_V1, 108);
        assert!(
            aggregate::maximum_encoded_proof_with_deep_bytes_v1(
                CA_AGGREGATE_PARAMETERS_V1,
                &layout,
            )
            .expect("maximum proof")
                <= CA_INNER_MAXIMUM_PROOF_BYTES_V1
        );
        let request = ca_accumulator_resource_request_v1(2, 1, 58).expect("exact request");
        assert_eq!(request.lde_log2, 14);
        assert_eq!(request.mask_degree, 305);
        assert_eq!(
            checked_ca_accumulator_resource_envelope_v1(request)
                .expect("exact envelope")
                .mask_coefficients,
            306
        );
    }
    #[test]
    fn strict_claim_envelope_rejects_omission_reorder_noncanonical_and_suffix() {
        let (trace, schedule, sha_challenges, io_challenges) = fixture();
        let material = build_ca_accumulator_stark_material_v1(
            &trace,
            &schedule,
            sha_challenges,
            io_challenges,
        )
        .expect("material");
        let claims = ca_accumulator_stark_terminal_claims_v1(&material);
        let inner = b"synthetic-inner-proof";
        let encoded = encode_ca_proof_envelope_v1(claims, inner).expect("envelope");
        let (decoded, decoded_inner) =
            decode_ca_proof_envelope_v1(&encoded).expect("decode envelope");
        assert_eq!(decoded, claims);
        assert_eq!(decoded_inner, inner);
        for truncated in [
            0,
            1,
            4,
            9,
            CA_PROOF_ENVELOPE_BYTES_V1 - 1,
            encoded.len() - 1,
        ] {
            assert!(
                decode_ca_proof_envelope_v1(&encoded[..truncated]).is_err(),
                "truncation at {truncated} must fail"
            );
        }
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(decode_ca_proof_envelope_v1(&trailing).is_err());
        for offset in [0, 4, 6, 8] {
            let mut changed = encoded.clone();
            changed[offset] ^= 1;
            assert!(decode_ca_proof_envelope_v1(&changed).is_err());
        }
        let mut reordered = encoded.clone();
        let first = 10;
        let second = first + CA_CLAIM_RECORD_BYTES_V1;
        let first_record = reordered[first..second].to_vec();
        let second_record = reordered[second..second + CA_CLAIM_RECORD_BYTES_V1].to_vec();
        reordered[first..second].copy_from_slice(&second_record);
        reordered[second..second + CA_CLAIM_RECORD_BYTES_V1].copy_from_slice(&first_record);
        assert!(decode_ca_proof_envelope_v1(&reordered).is_err());
        let mut noncanonical = encoded.clone();
        noncanonical[14..22].copy_from_slice(&0xffff_ffff_0000_0001_u64.to_be_bytes());
        assert_eq!(
            decode_ca_proof_envelope_v1(&noncanonical),
            Err(ZkX509CaAccumulatorProofErrorV1::NonCanonicalField)
        );
        let mut zero_inner = encoded;
        zero_inner[CA_PROOF_LENGTH_OFFSET_V1..CA_PROOF_ENVELOPE_BYTES_V1]
            .copy_from_slice(&0_u32.to_be_bytes());
        assert!(decode_ca_proof_envelope_v1(&zero_inner).is_err());
        let oversized = vec![0_u8; ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1 + 1];
        assert_eq!(
            decode_ca_proof_envelope_v1(&oversized),
            Err(ZkX509CaAccumulatorProofErrorV1::ProofTooLarge)
        );
    }
    #[test]
    fn prover_preflight_precedes_entropy_and_rng_health_fails_closed() {
        let (trace, schedule, _, _) = fixture();
        for period in [1, 2, 4, 8, 16, 32] {
            assert_eq!(
                prove_zk_x509_ca_accumulator_stark_v1_with_rng(
                    &trace,
                    &schedule,
                    credential_main_pre_aux_v1(),
                    &mut AdversarialEntropy(EntropyMode::Period(period)),
                ),
                Err(ZkX509CaAccumulatorProofErrorV1::RandomnessUnhealthy),
                "period {period} must fail"
            );
        }
        assert_eq!(
            prove_zk_x509_ca_accumulator_stark_v1_with_rng(
                &trace,
                &schedule,
                credential_main_pre_aux_v1(),
                &mut AdversarialEntropy(EntropyMode::PartialFailure),
            ),
            Err(ZkX509CaAccumulatorProofErrorV1::RandomnessUnavailable)
        );
        let mut invalid = trace;
        invalid.statement.governed_root[0] ^= 1;
        assert_eq!(
            prove_zk_x509_ca_accumulator_stark_v1_with_rng(
                &invalid,
                &schedule,
                credential_main_pre_aux_v1(),
                &mut AdversarialEntropy(EntropyMode::Panic),
            ),
            Err(ZkX509CaAccumulatorProofErrorV1::InvalidStatementOrWitness)
        );
    }
    #[test]
    fn canonical_dedicated_proof_roundtrips_and_exports_exact_binding() {
        let (public, schedule, proof) = canonical_proof_fixture();
        assert!(proof.len() <= ZK_X509_CA_ACCUMULATOR_MAX_PROOF_BYTES_V1);
        verify_zk_x509_ca_accumulator_stark_v1(
            *public,
            schedule,
            credential_main_pre_aux_v1(),
            proof,
        )
        .expect("canonical proof verifies");
        let binding = ca_accumulator_subproof_binding_from_proof_v1(
            *public,
            schedule,
            credential_main_pre_aux_v1(),
            proof,
        )
        .expect("typed binding");
        validate_ca_accumulator_subproof_binding_v1(*public, schedule, binding)
            .expect("binding validates");
        let first_digest = ca_accumulator_proof_binding_digest_v1(
            *public,
            schedule,
            credential_main_pre_aux_v1(),
            proof,
        )
        .expect("binding digest");
        let second_digest = ca_accumulator_proof_binding_digest_v1(
            *public,
            schedule,
            credential_main_pre_aux_v1(),
            proof,
        )
        .expect("deterministic binding digest");
        assert_eq!(first_digest, second_digest);
        assert_ne!(first_digest, [0; 32]);
        let (_, inner) = decode_ca_proof_envelope_v1(proof).expect("outer decode");
        let layout = ca_aggregate_layout_v1().expect("layout");
        let (decoded, deep) =
            aggregate::decode_proof_with_deep_v1(inner, CA_AGGREGATE_PARAMETERS_V1, &layout)
                .expect("inner decode");
        assert_eq!(
            aggregate::encode_proof_with_deep_v1(
                &decoded,
                &deep,
                CA_AGGREGATE_PARAMETERS_V1,
                &layout,
            )
            .expect("canonical re-encode"),
            inner
        );
        assert_eq!(
            ca_accumulator_base_root_from_proof_v1(proof).expect("decoded CA base root"),
            decoded.trace_groups[0].base_root
        );
    }
    #[test]
    fn credential_pre_aux_context_mutations_reject_the_same_ca_proof() {
        let (public, schedule, proof) = canonical_proof_fixture();
        let canonical = credential_main_pre_aux_v1();
        let mut wrong_consensus = canonical;
        wrong_consensus.consensus_context_digest_mut_for_test_v1()[0] ^= 1;
        assert!(
            verify_zk_x509_ca_accumulator_stark_v1(*public, schedule, wrong_consensus, proof,)
                .is_err()
        );
        let mut wrong_profile = canonical;
        wrong_profile.main_profile_digest_mut_for_test_v1()[31] ^= 1;
        assert!(
            verify_zk_x509_ca_accumulator_stark_v1(*public, schedule, wrong_profile, proof,)
                .is_err()
        );
        let mut wrong_roots = canonical;
        wrong_roots.main_base_roots_mut_for_test_v1().swap(1, 4);
        assert!(
            verify_zk_x509_ca_accumulator_stark_v1(*public, schedule, wrong_roots, proof,).is_err()
        );
        let canonical_digest =
            ca_accumulator_proof_binding_digest_v1(*public, schedule, canonical, proof)
                .expect("canonical outer binding digest");
        let mut changed_proof = proof.clone();
        changed_proof[CA_PROOF_ENVELOPE_BYTES_V1 + 8] ^= 1;
        assert!(
            ca_accumulator_base_root_from_proof_v1(&changed_proof).is_ok_and(|root| root
                != ca_accumulator_base_root_from_proof_v1(proof).expect("canonical CA base root"))
        );
        assert!(
            verify_zk_x509_ca_accumulator_stark_v1(*public, schedule, canonical, &changed_proof,)
                .is_err()
        );
        assert_ne!(canonical_digest, [0; 32]);
    }
    #[test]
    fn dedicated_proof_rejects_public_claim_root_deep_fri_query_and_frontier_mutations() {
        let (public, schedule, proof) = canonical_proof_fixture();
        let inner_start = CA_PROOF_ENVELOPE_BYTES_V1;
        let deep_start = inner_start + 8 + 4 * 32;
        let fri_start = deep_start + CA_DEEP_BYTES_V1;
        let hostile_offsets = [
            10 + 4,
            inner_start,
            inner_start + 8,
            inner_start + 40,
            inner_start + 72,
            deep_start,
            fri_start,
            proof.len() / 2,
            proof.len() - 1,
        ];
        for offset in hostile_offsets {
            let mut changed = proof.clone();
            changed[offset] ^= 1;
            assert!(
                verify_zk_x509_ca_accumulator_stark_v1(
                    *public,
                    schedule,
                    credential_main_pre_aux_v1(),
                    &changed,
                )
                .is_err(),
                "mutation at {offset} must fail"
            );
        }
        let query_bytes_v1 = 4
            + 2 * (ZK_X509_CA_ACCUMULATOR_BASE_WIDTH_V1 + ZK_X509_CA_ACCUMULATOR_AUX_WIDTH_V1)
                * core::mem::size_of::<u64>()
            + (CA_COMPOSITION_DEGREE_CHUNKS_V1 + 1 + 2 * usize::from(ZK_X509_CA_FRI_ROUNDS_V1))
                * core::mem::size_of::<[u64; 4]>();
        let query_start = fri_start
            + (usize::from(ZK_X509_CA_FRI_ROUNDS_V1) + 1) * 32
            + (1 << CA_TERMINAL_LOG2_V1) * core::mem::size_of::<[u64; 4]>()
            + core::mem::size_of::<u64>();
        assert_eq!(query_bytes_v1, 13_620);
        let mut duplicate_query = proof.clone();
        let first_index = duplicate_query[query_start..query_start + 4].to_vec();
        duplicate_query[query_start + query_bytes_v1..query_start + query_bytes_v1 + 4]
            .copy_from_slice(&first_index);
        assert!(
            verify_zk_x509_ca_accumulator_stark_v1(
                *public,
                schedule,
                credential_main_pre_aux_v1(),
                &duplicate_query,
            )
            .is_err(),
            "duplicate transcript query must fail"
        );
        let mut grinding_mutation = proof.clone();
        grinding_mutation[query_start - 1] ^= 1;
        assert!(
            verify_zk_x509_ca_accumulator_stark_v1(
                *public,
                schedule,
                credential_main_pre_aux_v1(),
                &grinding_mutation,
            )
            .is_err(),
            "grinding nonce mutation must fail"
        );
        for truncated in [
            0,
            1,
            CA_PROOF_ENVELOPE_BYTES_V1 - 1,
            proof.len() / 3,
            proof.len() - 1,
        ] {
            assert!(
                verify_zk_x509_ca_accumulator_stark_v1(
                    *public,
                    schedule,
                    credential_main_pre_aux_v1(),
                    &proof[..truncated],
                )
                .is_err()
            );
        }
        let mut trailing = proof.clone();
        trailing.extend_from_slice(&[0, 1, 2, 3]);
        assert!(
            verify_zk_x509_ca_accumulator_stark_v1(
                *public,
                schedule,
                credential_main_pre_aux_v1(),
                &trailing,
            )
            .is_err()
        );
        let mut wrong_public = *public;
        wrong_public.governed_root[0] = wrong_public.governed_root[0].add(F::ONE);
        assert!(
            verify_zk_x509_ca_accumulator_stark_v1(
                wrong_public,
                schedule,
                credential_main_pre_aux_v1(),
                proof,
            )
            .is_err()
        );
        let other_schedule = ZkX509ShaCallScheduleV1::new(ZkX509ShaCallPublicShapeV1 {
            disclosed_attributes: 3,
        })
        .expect("alternate public schedule");
        assert!(
            verify_zk_x509_ca_accumulator_stark_v1(
                *public,
                &other_schedule,
                credential_main_pre_aux_v1(),
                proof,
            )
            .is_err()
        );
    }
}
