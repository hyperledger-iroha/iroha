//! Closed first-release relation and AIR resource profile for zk-X509.
//!
//! The profile is executable documentation.  It fixes every capacity, hash
//! domain, accepted algorithm, custom EKU, trace segment, and transparent-proof
//! parameter that can affect consensus.  Changing any value creates a new
//! protocol identifier; there is no legacy decoder or parameter negotiation.

use thiserror::Error;

/// Exact native relation version.
pub(crate) const ZK_X509_RELATION_VERSION_V1: u16 = 1;
/// Exact native proof-container version.
pub(crate) const ZK_X509_PROOF_VERSION_V1: u16 = 1;
/// Canonical protocol suite string committed by every transcript.
pub(crate) const ZK_X509_SUITE_V1: &[u8] = b"iroha-zk-x509-stark-p256-v0";
/// Source identity for the original Iroha implementation.
pub(crate) const ZK_X509_SOURCE_PROFILE_V1: &[u8] =
    b"iroha-native-rust:strict-der:rfc5280-p256-sha256:private-chain-crl-ownership:goldilocks-stark:v1";

/// Minimum admitted certificate-chain depth, including leaf and root.
pub(crate) const ZK_X509_MIN_CHAIN_DEPTH_V1: usize = 2;
/// Maximum admitted certificate-chain depth, including leaf and root.
pub(crate) const ZK_X509_MAX_CHAIN_DEPTH_V1: usize = 3;
/// Maximum DER bytes in the private CRL witness.
///
/// Unbounded parsing is deliberately excluded from a consensus circuit.
pub(crate) const ZK_X509_MAX_CRL_BYTES_V1: usize = 16 * 1024;
/// Maximum revoked-certificate entries accepted in the private DER CRL.
///
/// The sparse root is reconstructed from every entry in-circuit.  Sixty-four
/// entries keep that SHA-256 work bounded. A policy whose issuer's complete
/// base CRL exceeds this ceiling is unusable under v0; partitioning, delta
/// CRLs, and omission are not fallback paths.
pub(crate) const ZK_X509_MAX_CRL_ENTRIES_V1: usize = 64;
/// Maximum canonical unsigned certificate-serial bytes.
pub(crate) const ZK_X509_MAX_SERIAL_BYTES_V1: usize = 20;
/// Exact depth of the governed CA sparse-membership tree.
///
/// All 256 SHA-256 key bits select the path.  Truncating this to 32 would make
/// distinct SPKIs collide at only 32 bits and is forbidden.
pub(crate) const ZK_X509_CA_TREE_DEPTH_V1: usize = 256;
/// Exact depth of the governed CRL sparse non-membership tree.
pub(crate) const ZK_X509_CRL_TREE_DEPTH_V1: usize = 256;
/// Maximum accepted lag between trusted block time and CRL `thisUpdate`.
pub(crate) const ZK_X509_MAX_CRL_AGE_SECONDS_V1: u64 = 300;
/// Maximum accepted future skew for any witness time.
///
/// Consensus supplies one trusted block timestamp, so future tolerance is
/// intentionally zero rather than dependent on a node's wall clock.
pub(crate) const ZK_X509_MAX_FUTURE_SKEW_SECONDS_V1: u64 = 0;
/// Fixed private salt width for one subject-attribute commitment.
pub(crate) const ZK_X509_ATTRIBUTE_SALT_BYTES_V1: usize = 32;
/// Maximum exact DER content bytes in one committed subject attribute.
pub(crate) const ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1: usize = 256;
/// Fixed raw P-256 affine public-key width, including SEC1 prefix.
pub(crate) const ZK_X509_COMPRESSED_P256_BYTES_V1: usize = 33;
/// Fixed uncompressed P-256 affine public-key width, including SEC1 prefix.
pub(crate) const ZK_X509_UNCOMPRESSED_P256_BYTES_V1: usize = 65;

/// RFC 5280 client-authentication EKU OID.
pub(crate) const ZK_X509_CLIENT_AUTHENTICATION_EKU_OID_V1: &str = "1.3.6.1.5.5.7.3.2";
/// Iroha document-signing EKU OID.
///
/// The `2.25` UUID OID is UUIDv5(URL namespace,
/// `https://hyperledger.github.io/iroha/eku/document-signing/v1`).
pub(crate) const ZK_X509_DOCUMENT_SIGNING_EKU_OID_V1: &str =
    "2.25.27309815684091774780500832483404888315";
/// Iroha wallet-identity EKU OID.
///
/// The `2.25` UUID OID is UUIDv5(URL namespace,
/// `https://hyperledger.github.io/iroha/eku/wallet-identity/v1`).
pub(crate) const ZK_X509_WALLET_IDENTITY_EKU_OID_V1: &str =
    "2.25.325405717892146366947968947126277329515";
/// DER content octets of the Iroha document-signing EKU OID.
pub(crate) const ZK_X509_DOCUMENT_SIGNING_EKU_DER_VALUE_V1: &[u8] = &[
    0x69, 0xa9, 0x8b, 0xd6, 0xf7, 0xd8, 0x86, 0xaa, 0xc4, 0xb3, 0x8b, 0xcd, 0xbc, 0xba, 0xe7, 0x86,
    0x8c, 0xf1, 0x7b,
];
/// DER content octets of the Iroha wallet-identity EKU OID.
pub(crate) const ZK_X509_WALLET_IDENTITY_EKU_DER_VALUE_V1: &[u8] = &[
    0x69, 0x83, 0xe9, 0xce, 0xee, 0xa4, 0xdc, 0xb4, 0xda, 0xe5, 0xdf, 0xbe, 0xf0, 0xac, 0xd6, 0xd0,
    0xac, 0xc5, 0xa4, 0x6b,
];

/// Domain for the canonical SHA-256 field-framing function.
pub(crate) const ZK_X509_HASH_FRAME_DOMAIN_V1: &[u8] = b"iroha.zk-x509.sha256.frame.v1";
/// Domain for CA sparse-tree member keys.
pub(crate) const ZK_X509_CA_KEY_DOMAIN_V1: &[u8] = b"iroha.zk-x509.ca.key.v1";
/// Domain for occupied CA leaves.
pub(crate) const ZK_X509_CA_LEAF_DOMAIN_V1: &[u8] = b"iroha.zk-x509.ca.leaf.v1";
/// Domain for empty CA leaves.
pub(crate) const ZK_X509_CA_EMPTY_LEAF_DOMAIN_V1: &[u8] = b"iroha.zk-x509.ca.empty-leaf.v1";
/// Domain for CA internal tree nodes.
pub(crate) const ZK_X509_CA_NODE_DOMAIN_V1: &[u8] = b"iroha.zk-x509.ca.node.v1";
/// Domain for CRL sparse-tree keys.
pub(crate) const ZK_X509_CRL_KEY_DOMAIN_V1: &[u8] = b"iroha.zk-x509.crl.key.v1";
/// Domain for occupied CRL leaves.
pub(crate) const ZK_X509_CRL_LEAF_DOMAIN_V1: &[u8] = b"iroha.zk-x509.crl.leaf.v1";
/// Domain for empty CRL leaves.
pub(crate) const ZK_X509_CRL_EMPTY_LEAF_DOMAIN_V1: &[u8] = b"iroha.zk-x509.crl.empty-leaf.v1";
/// Domain for CRL internal tree nodes.
pub(crate) const ZK_X509_CRL_NODE_DOMAIN_V1: &[u8] = b"iroha.zk-x509.crl.node.v1";
/// Domain for the chain- and policy-scoped leaf-key commitment.
pub(crate) const ZK_X509_SCOPED_KEY_DOMAIN_V1: &[u8] = b"iroha.zk-x509.scoped-subject-key.v1";
/// Domain for deterministic certificate nullifiers.
pub(crate) const ZK_X509_NULLIFIER_DOMAIN_V1: &[u8] = b"iroha.zk-x509.certificate-nullifier.v1";
/// Domain for wallet-ownership challenges.
pub(crate) const ZK_X509_OWNERSHIP_DOMAIN_V1: &[u8] = b"iroha.zk-x509.wallet-ownership.v1";
/// Domain for subject-attribute commitments.
pub(crate) const ZK_X509_ATTRIBUTE_DOMAIN_V1: &[u8] = b"iroha.zk-x509.subject-attribute.v1";
/// Domain for an exact private DER CRL digest.
pub(crate) const ZK_X509_CRL_DER_DIGEST_DOMAIN_V1: &[u8] = b"iroha.zk-x509.crl.der.v1";
/// Domain for an exact CRL issuer-SPKI digest.
pub(crate) const ZK_X509_CRL_ISSUER_SPKI_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.zk-x509.crl.issuer-spki.v1";

/// Canonical schema of one immutable governed CRL revision.
///
/// The authoritative data-model record must carry all of these fields and a
/// domain-separated self-digest.  The relation recomputes `der_digest`,
/// `issuer_spki_digest`, both times, and `revoked_serials_root` from the
/// private signed CRL.  Thus the signature check and non-revocation predicate
/// cannot refer to unrelated revocation sources.
pub(crate) const ZK_X509_CRL_REVISION_SCHEMA_V1: &[u8] = b"implicit_version:u16-be=1|trust_anchor_id:bytes32|certificate_policy_id:bytes32|record_epoch:u64-be|crl_number:u64-be-strictly-increasing|crl_der_digest:domain-framed-sha256|issuer_spki_digest:domain-framed-sha256|this_update_unix_seconds:u64-be|next_update_unix_seconds:u64-be|revoked_serials_root:sha256|root_epoch:u64-be-active-equals-record-epoch-revoked-preserves-predecessor|previous_record_digest:tag-plus-bytes32|lifecycle:u8-active-or-revoked|record_digest:domain-framed-sha256";
/// Closed revocation scope for the first release.
///
/// One certificate-policy lineage identifies exactly one leaf-certificate
/// issuer and its complete, non-partitioned signed CRL. The relation checks
/// revocation of the leaf certificate only. Deployments with multiple
/// intermediate issuers use separate policy lineages; delta CRLs, indirect
/// CRLs, distribution-point partitions, and incomplete shard claims are
/// rejected rather than interpreted.
pub(crate) const ZK_X509_CRL_SCOPE_PROFILE_V1: &[u8] = b"leaf-only:one-issuer-per-certificate-policy:complete-base-crl:no-delta:no-indirect:no-partition:no-distribution-point";

/// Exact extension roles admitted on every certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ZkX509CertificateExtensionV1 {
    /// RFC 5280 authority key identifier, non-critical.
    AuthorityKeyIdentifier,
    /// RFC 5280 subject key identifier, non-critical.
    SubjectKeyIdentifier,
    /// RFC 5280 basic constraints, critical.
    BasicConstraints,
    /// RFC 5280 key usage, critical.
    KeyUsage,
    /// RFC 5280 extended key usage, critical and leaf-only.
    ExtendedKeyUsage,
}

/// Complete certificate extension allow-list in canonical engine role order.
pub(crate) const ZK_X509_ALLOWED_CERTIFICATE_EXTENSIONS_V1: [ZkX509CertificateExtensionV1; 5] = [
    ZkX509CertificateExtensionV1::AuthorityKeyIdentifier,
    ZkX509CertificateExtensionV1::SubjectKeyIdentifier,
    ZkX509CertificateExtensionV1::KeyUsage,
    ZkX509CertificateExtensionV1::BasicConstraints,
    ZkX509CertificateExtensionV1::ExtendedKeyUsage,
];

/// Exact extension roles admitted on the complete base CRL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ZkX509CrlExtensionV1 {
    /// RFC 5280 authority key identifier, non-critical.
    AuthorityKeyIdentifier,
    /// RFC 5280 monotonically increasing CRLNumber, non-critical.
    CrlNumber,
}

/// Complete CRL extension allow-list in canonical engine role order.
pub(crate) const ZK_X509_ALLOWED_CRL_EXTENSIONS_V1: [ZkX509CrlExtensionV1; 2] = [
    ZkX509CrlExtensionV1::AuthorityKeyIdentifier,
    ZkX509CrlExtensionV1::CrlNumber,
];

/// Canonical rules for the admitted complete base CRL.
///
/// Every CRL is v2, direct, complete, and issuer-scoped. AKI and CRLNumber are
/// required exactly once and non-critical. CRLNumber must fit `u64` and
/// strictly increase in governance. Delta, indirect, partitioned, and
/// distribution-point CRLs are forbidden. No CRL-entry extension is allowed,
/// including certificateIssuer and reasonCode.
pub(crate) const ZK_X509_CRL_PROFILE_V1: &[u8] = b"rfc5280-crl-closed-v1:der-only:v2:ecdsa-sha256-absent-params:aki-required-noncritical:crl-number-required-noncritical-u64-strictly-increasing:no-delta-crl-indicator:no-issuing-distribution-point:no-freshest-crl:no-indirect:no-partition:no-entry-extensions:complete-base-crl:max64-revoked-entries";

/// Canonical rules for the admitted RFC 5280 subset.
///
/// These bytes are included in the parameter/engine manifest.  They are
/// intentionally exhaustive: version 3 only; strict DER with one top-level
/// value; positive non-zero serials of at most 20 DER content octets; exact
/// issuer/subject DER equality; exact UTC encoding (`UTCTime` through 2049,
/// `GeneralizedTime` from 2050, seconds and `Z`, no offset/fraction); no issuer
/// or subject unique IDs; ECDSA-with-SHA256 with absent parameters in both
/// outer and TBS identifiers; uncompressed prime256v1 SPKIs; duplicate,
/// unparsed, unsupported, and unlisted extensions rejected regardless of
/// criticality; AKI/SKI required and linked; BasicConstraints and KeyUsage
/// critical; leaf-only critical EKU; CA/pathLen/keyCertSign/cRLSign enforced;
/// root self-name and self-signature enforced.  NameConstraints,
/// PolicyMappings, CertificatePolicies, PolicyConstraints, InhibitAnyPolicy,
/// alternate names, distribution points, and every other extension are
/// forbidden rather than silently ignored.
pub(crate) const ZK_X509_RFC5280_PROFILE_V1: &[u8] = b"rfc5280-closed-v1:der-only:v3:serial-positive-nonzero-max20:exact-name-der:name-oids-only-c-o-ou-cn:no-duplicate-name-attributes:c-printablestring-two-uppercase-ascii:o-ou-cn-utf8string-or-printablestring:max-name-value256:utf8-well-formed-no-u0000-u001f-u007f-u009f:utc-time-1970-2049:generalized-time-2050-9999:seconds-z-no-fraction:certificate-validity-inclusive:no-unique-id:ecdsa-sha256-absent-params:spki-id-ec-public-key-prime256v1-uncompressed:extensions-exact-order-aki-ski-keyusage-basicconstraints-and-optional-leaf-eku:no-duplicates-no-unknown:aki-ski-linked:bc-ku-eku-critical:ca-keycertsign-and-crlsign-only:ca-pathlen-required-explicit:direct-leaf-issuer-crlsign:root-self-name-self-signature:leaf-revocation-only:no-nameconstraints:no-policy-mapping:no-certificate-policies:no-policy-constraints:no-inhibit-any-policy:no-alt-names:no-distribution-points:no-freshest-crl";

/// ECDSA signature canonicalization rules.
///
/// Certificate and CRL signatures accept both mathematically valid `s`
/// halves, as required for interoperable RFC 5280 verification, but their ASN.1
/// `SEQUENCE(INTEGER r, INTEGER s)` must be minimal DER.  The fresh wallet
/// ownership signature is controlled by this protocol and must additionally
/// use low `s`, eliminating an avoidable witness malleability.
pub(crate) const ZK_X509_ECDSA_RULES_V1: &[u8] =
    b"cert-and-crl:ecdsa-with-sha256-over-exact-tbs:minimal-der-rs-valid-range-high-or-low-s|wallet:ecdsa-p256-prehash-over-exact-32-byte-ownership-digest:minimal-der-rs-valid-range-low-s";

/// Goldilocks prime `2^64 - 2^32 + 1`.
pub(crate) const ZK_X509_GOLDILOCKS_MODULUS_V1: u64 = 0xffff_ffff_0000_0001;
/// Binary logarithm of the maximum independently committed trace segment.
pub(crate) const ZK_X509_SEGMENT_LOG_ROWS_V1: u8 = 20;
/// Maximum rows in one independently committed trace segment.
pub(crate) const ZK_X509_SEGMENT_ROWS_V1: u32 = 1 << ZK_X509_SEGMENT_LOG_ROWS_V1;
/// Maximum number of sequential trace segments in one proof.
pub(crate) const ZK_X509_MAX_TRACE_SEGMENTS_V1: usize = 16;
/// Maximum base-trace column count in one segment.
pub(crate) const ZK_X509_MAX_TRACE_COLUMNS_V1: u16 = 64;
/// Maximum columns transformed in one streaming LDE batch.
pub(crate) const ZK_X509_LDE_COLUMN_BATCH_V1: u16 = 8;
/// Hard peak-memory target for the native prover.
pub(crate) const ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1: u64 = 2 * 1024 * 1024 * 1024;
/// Hard wall-clock target for one proof on the release benchmark machine.
pub(crate) const ZK_X509_PROVER_TARGET_SECONDS_V1: u64 = 300;
/// Maximum algebraic constraint degree before quotienting.
pub(crate) const ZK_X509_MAX_CONSTRAINT_DEGREE_V1: u8 = 4;
/// Low-degree-extension blow-up factor.
pub(crate) const ZK_X509_FRI_BLOWUP_FACTOR_V1: u8 = 8;
/// Number of independent Fiat-Shamir query positions.
pub(crate) const ZK_X509_FRI_QUERY_COUNT_V1: u8 = 56;
/// Number of independently mixed composition lanes.
pub(crate) const ZK_X509_COMPOSITION_LANES_V1: u8 = 3;
/// Binary FRI folding arity.
pub(crate) const ZK_X509_FRI_FOLDING_ARITY_V1: u8 = 2;
/// Maximum final FRI polynomial length.
pub(crate) const ZK_X509_FRI_FINAL_POLYNOMIAL_LENGTH_V1: u16 = 32;
/// Fiat-Shamir proof-of-work grinding bits.
pub(crate) const ZK_X509_GRINDING_BITS_V1: u8 = 20;
/// Required computational-soundness target for the released proof system.
///
/// This is a release requirement, not a claim about the provisional
/// parameters below.  The finalized analysis must derive its bound from the
/// implemented AIR degree schedule, FRI proximity theorem, transcript,
/// opening schedule, hash assumptions, and every verifier union-bound event.
pub(crate) const ZK_X509_TARGET_SOUNDNESS_BITS_V1: u16 = 128;
/// Consensus proof-byte ceiling inherited by this engine.
pub(crate) const ZK_X509_MAX_PROOF_BYTES_V1: u32 = 8 * 1024 * 1024;
/// Maximum number of independently mixed constraints in one segment.
pub(crate) const ZK_X509_MAX_CONSTRAINTS_PER_SEGMENT_V1: u16 = 512;
/// Complete provisional transparent-proof envelope descriptor.
///
/// It is included in the compiled profile digest, but explicitly records that
/// no proof wire exists while the gap inventory is non-empty.  A released
/// profile must replace this entire descriptor and protocol version together;
/// it may not reinterpret the provisional bytes.
pub(crate) const ZK_X509_PROVISIONAL_STARK_PROFILE_DESCRIPTOR_V1: &[u8] = b"field=goldilocks:0xffffffff00000001|segment-log-rows=20|max-segment-instances=16|max-columns=64|lde-column-batch=8|max-constraint-degree=4|fri-blowup=8|fri-queries=56|composition-lanes=3|fri-folding=2|fri-terminal-max=32|grinding-bits=20|target-soundness-bits=128|max-proof-bytes=8388608|proof-wire=unassigned-while-air-gaps-nonempty|activation=false";
/// Whether an implementation-derived proof-system soundness analysis is fixed.
///
/// A numeric estimate assembled from query count and blow-up factor alone is
/// not a soundness proof.  This gate remains false until the native verifier's
/// exact degree, proximity, opening, transcript, and hash schedules exist and
/// their aggregate bound meets [`ZK_X509_TARGET_SOUNDNESS_BITS_V1`].
pub(crate) const ZK_X509_SOUNDNESS_PROFILE_FINALIZED_V1: bool = false;
/// Whether release-machine measurements have fixed row, time, and memory use.
///
/// This is separate from cryptographic correctness: a prover that is sound
/// but cannot meet Taira's bounded resource envelope is not releasable.
pub(crate) const ZK_X509_RESOURCE_PROFILE_FINALIZED_V1: bool = false;

/// Whether consensus may expose a compiled profile for this engine.
///
/// This remains false until strict DER/RFC path processing, SHA-256, P-256,
/// accumulator, output-projection, prover, verifier, deterministic KAT, and
/// adversarial suites are all implemented and independently differential
/// tested.  Runtime governance code must not bypass this gate.
pub(crate) const ZK_X509_ENGINE_ACTIVATION_READY_V1: bool = false;

/// One bounded subtrace family in the purpose-built segmented AIR.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ZkX509TraceSegmentV1 {
    /// Stable segment name included in the engine manifest.
    pub(crate) name: &'static str,
    /// Maximum padded rows per segment instance.
    pub(crate) max_rows: u32,
    /// Maximum number of instances.
    pub(crate) max_instances: u8,
    /// Maximum columns used by this chip.
    pub(crate) columns: u16,
    /// Maximum local transition degree.
    pub(crate) constraint_degree: u8,
}

impl ZkX509TraceSegmentV1 {
    /// Maximum rows across this segment family.
    pub(crate) const fn total_rows(self) -> u64 {
        self.max_rows as u64 * self.max_instances as u64
    }
}

/// Provisional segmented trace families.
///
/// These are hard engineering ceilings, not claimed measurements.  Activation
/// remains impossible while
/// [`ZK_X509_RESOURCE_PROFILE_FINALIZED_V1`] is false.  Each segment exposes
/// binding digests to a final projection segment, and cross-segment
/// permutation products bind private byte memory without publishing witness
/// digests.  Base columns and LDE columns are committed in eight-column
/// streaming batches; only one `2^20 × 8 × blowup` batch plus Merkle frontiers
/// is live, bounding peak memory instead of materializing the full LDE.
pub(crate) const ZK_X509_TRACE_SEGMENTS_V1: [ZkX509TraceSegmentV1; 8] = [
    ZkX509TraceSegmentV1 {
        name: "strict_der_decode",
        max_rows: 262_144,
        max_instances: 1,
        columns: 48,
        constraint_degree: 3,
    },
    ZkX509TraceSegmentV1 {
        name: "byte_memory_permutation",
        max_rows: 262_144,
        max_instances: 1,
        columns: 40,
        constraint_degree: 4,
    },
    ZkX509TraceSegmentV1 {
        name: "sha256_certificate_crl_and_frames",
        max_rows: 524_288,
        max_instances: 2,
        columns: 64,
        constraint_degree: 3,
    },
    ZkX509TraceSegmentV1 {
        name: "p256_field_and_ecdsa",
        max_rows: 524_288,
        max_instances: 5,
        columns: 64,
        constraint_degree: 4,
    },
    ZkX509TraceSegmentV1 {
        name: "ca_sparse_membership_sha256",
        max_rows: 131_072,
        max_instances: 1,
        columns: 64,
        constraint_degree: 3,
    },
    ZkX509TraceSegmentV1 {
        name: "crl_complete_sparse_root_sha256",
        max_rows: 1_048_576,
        max_instances: 3,
        columns: 64,
        constraint_degree: 3,
    },
    ZkX509TraceSegmentV1 {
        name: "rfc5280_path_state",
        max_rows: 131_072,
        max_instances: 1,
        columns: 48,
        constraint_degree: 4,
    },
    ZkX509TraceSegmentV1 {
        name: "public_projection_masking_and_padding",
        max_rows: 131_072,
        max_instances: 1,
        columns: 64,
        constraint_degree: 4,
    },
];

/// Closed checklist that must be true before activation can be compiled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ZkX509ReadinessV1 {
    /// Strict DER parser and RFC 5280 state machine are constrained.
    pub(crate) der_and_rfc5280_air: bool,
    /// SHA-256 compression and bit/range lookups are constrained.
    pub(crate) sha256_air: bool,
    /// P-256 field/group arithmetic and ECDSA verification are constrained.
    pub(crate) p256_air: bool,
    /// CA membership and complete CRL sparse-root reconstruction are constrained.
    pub(crate) accumulator_air: bool,
    /// Scoped commitment, deterministic nullifier, and disclosures are constrained.
    pub(crate) output_projection_air: bool,
    /// Prover emits only the exact canonical proof container.
    pub(crate) prover: bool,
    /// Verifier enforces every canonical decode and transcript check.
    pub(crate) verifier: bool,
    /// Independent deterministic known-answer vectors pass.
    pub(crate) known_answer_tests: bool,
    /// Negative, mutation, and adversarial corpora pass.
    pub(crate) adversarial_tests: bool,
    /// Native relation and AIR witness execution agree differentially.
    pub(crate) differential_tests: bool,
    /// The implemented verifier has a complete, reviewed soundness analysis.
    pub(crate) soundness_analysis: bool,
    /// Release benchmarks fix measured rows, time, and peak memory below ceilings.
    pub(crate) resource_benchmarks: bool,
}

impl ZkX509ReadinessV1 {
    /// Whether every independently auditable implementation component exists.
    pub(crate) const fn is_complete(self) -> bool {
        self.der_and_rfc5280_air
            && self.sha256_air
            && self.p256_air
            && self.accumulator_air
            && self.output_projection_air
            && self.prover
            && self.verifier
            && self.known_answer_tests
            && self.adversarial_tests
            && self.differential_tests
            && self.soundness_analysis
            && self.resource_benchmarks
    }
}

/// Failure in the fixed profile or activation checklist.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum ZkX509ProfileErrorV1 {
    /// A trace family is empty or the maximum instance count is exceeded.
    #[error("zk-X509 segmented trace envelope is invalid")]
    InvalidTraceEnvelope,
    /// A trace segment exceeds the global column or constraint-degree bound.
    #[error("zk-X509 trace segment exceeds a global AIR bound")]
    InvalidTraceSegmentBound,
    /// Transparent-proof parameters do not meet the closed release floor.
    #[error("zk-X509 transparent-proof parameters are below the release floor")]
    InvalidStarkParameters,
    /// Proof cap is zero or exceeds Taira's fixed per-action ceiling.
    #[error("zk-X509 proof cap is invalid")]
    InvalidProofCap,
    /// Consensus activation was requested before every implementation gate passed.
    #[error("zk-X509 engine is not complete and cannot be activated")]
    EngineIncomplete,
}

/// Validate the immutable profile constants.
pub(crate) fn validate_profile_v1() -> Result<(), ZkX509ProfileErrorV1> {
    let mut total_instances = 0_usize;
    let mut total_rows = 0_u64;
    for (index, segment) in ZK_X509_TRACE_SEGMENTS_V1.iter().copied().enumerate() {
        if segment.name.is_empty()
            || segment.max_rows == 0
            || !segment.max_rows.is_power_of_two()
            || segment.max_rows > ZK_X509_SEGMENT_ROWS_V1
            || segment.max_instances == 0
            || ZK_X509_TRACE_SEGMENTS_V1[..index]
                .iter()
                .any(|other| other.name == segment.name)
        {
            return Err(ZkX509ProfileErrorV1::InvalidTraceEnvelope);
        }
        if segment.columns == 0
            || segment.columns > ZK_X509_MAX_TRACE_COLUMNS_V1
            || ZK_X509_LDE_COLUMN_BATCH_V1 == 0
            || segment.columns % ZK_X509_LDE_COLUMN_BATCH_V1 != 0
            || segment.constraint_degree < 2
            || segment.constraint_degree > ZK_X509_MAX_CONSTRAINT_DEGREE_V1
        {
            return Err(ZkX509ProfileErrorV1::InvalidTraceSegmentBound);
        }
        total_instances = total_instances
            .checked_add(usize::from(segment.max_instances))
            .ok_or(ZkX509ProfileErrorV1::InvalidTraceEnvelope)?;
        total_rows = total_rows
            .checked_add(segment.total_rows())
            .ok_or(ZkX509ProfileErrorV1::InvalidTraceEnvelope)?;
    }
    let streaming_lde_batch_bytes = u64::from(ZK_X509_SEGMENT_ROWS_V1)
        .checked_mul(u64::from(ZK_X509_LDE_COLUMN_BATCH_V1))
        .and_then(|value| value.checked_mul(core::mem::size_of::<u64>() as u64))
        .and_then(|value| value.checked_mul(u64::from(ZK_X509_FRI_BLOWUP_FACTOR_V1)))
        .ok_or(ZkX509ProfileErrorV1::InvalidTraceEnvelope)?;
    if total_instances == 0
        || total_instances > ZK_X509_MAX_TRACE_SEGMENTS_V1
        || total_rows == 0
        || streaming_lde_batch_bytes > ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1
        || ZK_X509_PROVER_TARGET_SECONDS_V1 == 0
    {
        return Err(ZkX509ProfileErrorV1::InvalidTraceEnvelope);
    }
    if ZK_X509_GOLDILOCKS_MODULUS_V1 != 0xffff_ffff_0000_0001
        || !ZK_X509_FRI_BLOWUP_FACTOR_V1.is_power_of_two()
        || ZK_X509_FRI_BLOWUP_FACTOR_V1 < 8
        || ZK_X509_FRI_QUERY_COUNT_V1 < 48
        || ZK_X509_COMPOSITION_LANES_V1 < 3
        || ZK_X509_FRI_FOLDING_ARITY_V1 != 2
        || ZK_X509_FRI_FINAL_POLYNOMIAL_LENGTH_V1 > 32
        || ZK_X509_GRINDING_BITS_V1 < 20
        || ZK_X509_TARGET_SOUNDNESS_BITS_V1 < 128
    {
        return Err(ZkX509ProfileErrorV1::InvalidStarkParameters);
    }
    if ZK_X509_MAX_CONSTRAINTS_PER_SEGMENT_V1 == 0 {
        return Err(ZkX509ProfileErrorV1::InvalidTraceSegmentBound);
    }
    if ZK_X509_MAX_PROOF_BYTES_V1 == 0 || ZK_X509_MAX_PROOF_BYTES_V1 > 8 * 1024 * 1024 {
        return Err(ZkX509ProfileErrorV1::InvalidProofCap);
    }
    Ok(())
}

/// Enforce the compile-time activation and independent readiness gates.
pub(crate) fn require_activation_readiness_v1(
    readiness: ZkX509ReadinessV1,
) -> Result<(), ZkX509ProfileErrorV1> {
    validate_profile_v1()?;
    if !ZK_X509_ENGINE_ACTIVATION_READY_V1
        || !ZK_X509_SOUNDNESS_PROFILE_FINALIZED_V1
        || !ZK_X509_RESOURCE_PROFILE_FINALIZED_V1
        || !readiness.is_complete()
    {
        return Err(ZkX509ProfileErrorV1::EngineIncomplete);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_profile_is_internally_consistent_but_not_activatable() {
        validate_profile_v1().expect("fixed zk-X509 profile");
        assert_eq!(
            ZK_X509_TRACE_SEGMENTS_V1
                .iter()
                .map(|segment| usize::from(segment.max_instances))
                .sum::<usize>(),
            15
        );
        assert_eq!(ZK_X509_TARGET_SOUNDNESS_BITS_V1, 128);
        assert!(!ZK_X509_SOUNDNESS_PROFILE_FINALIZED_V1);
        assert!(!ZK_X509_RESOURCE_PROFILE_FINALIZED_V1);
        assert!(!ZK_X509_ENGINE_ACTIVATION_READY_V1);
        assert_eq!(
            require_activation_readiness_v1(ZkX509ReadinessV1 {
                der_and_rfc5280_air: true,
                sha256_air: true,
                p256_air: true,
                accumulator_air: true,
                output_projection_air: true,
                prover: true,
                verifier: true,
                known_answer_tests: true,
                adversarial_tests: true,
                differential_tests: true,
                soundness_analysis: true,
                resource_benchmarks: true,
            }),
            Err(ZkX509ProfileErrorV1::EngineIncomplete)
        );
    }

    #[test]
    fn independent_readiness_requirements_fail_closed() {
        let gates = [
            ZkX509ReadinessV1 {
                der_and_rfc5280_air: false,
                ..complete_readiness()
            },
            ZkX509ReadinessV1 {
                sha256_air: false,
                ..complete_readiness()
            },
            ZkX509ReadinessV1 {
                p256_air: false,
                ..complete_readiness()
            },
            ZkX509ReadinessV1 {
                accumulator_air: false,
                ..complete_readiness()
            },
            ZkX509ReadinessV1 {
                output_projection_air: false,
                ..complete_readiness()
            },
            ZkX509ReadinessV1 {
                prover: false,
                ..complete_readiness()
            },
            ZkX509ReadinessV1 {
                verifier: false,
                ..complete_readiness()
            },
            ZkX509ReadinessV1 {
                known_answer_tests: false,
                ..complete_readiness()
            },
            ZkX509ReadinessV1 {
                adversarial_tests: false,
                ..complete_readiness()
            },
            ZkX509ReadinessV1 {
                differential_tests: false,
                ..complete_readiness()
            },
            ZkX509ReadinessV1 {
                soundness_analysis: false,
                ..complete_readiness()
            },
            ZkX509ReadinessV1 {
                resource_benchmarks: false,
                ..complete_readiness()
            },
        ];
        for gate in gates {
            assert!(!gate.is_complete());
            assert_eq!(
                require_activation_readiness_v1(gate),
                Err(ZkX509ProfileErrorV1::EngineIncomplete)
            );
        }
    }

    #[test]
    fn custom_eku_oids_are_fixed_uuid_oids() {
        assert_eq!(
            ZK_X509_DOCUMENT_SIGNING_EKU_OID_V1,
            "2.25.27309815684091774780500832483404888315"
        );
        assert_eq!(
            hex::encode(ZK_X509_DOCUMENT_SIGNING_EKU_DER_VALUE_V1),
            "69a98bd6f7d886aac4b38bcdbcbae7868cf17b"
        );
        assert_eq!(
            ZK_X509_WALLET_IDENTITY_EKU_OID_V1,
            "2.25.325405717892146366947968947126277329515"
        );
        assert_eq!(
            hex::encode(ZK_X509_WALLET_IDENTITY_EKU_DER_VALUE_V1),
            "6983e9ceeea4dcb4dae5dfbef0acd6d0acc5a46b"
        );
    }

    const fn complete_readiness() -> ZkX509ReadinessV1 {
        ZkX509ReadinessV1 {
            der_and_rfc5280_air: true,
            sha256_air: true,
            p256_air: true,
            accumulator_air: true,
            output_projection_air: true,
            prover: true,
            verifier: true,
            known_answer_tests: true,
            adversarial_tests: true,
            differential_tests: true,
            soundness_analysis: true,
            resource_benchmarks: true,
        }
    }
}
