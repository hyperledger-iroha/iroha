//! Closed first-release relation and AIR resource profile for zk-X509.
//!
//! The profile is executable documentation.  It fixes every capacity, hash
//! domain, accepted algorithm, custom EKU, trace segment, and transparent-proof
//! parameter that can affect consensus.  Changing any value creates a new
//! protocol identifier; there is no legacy decoder or parameter negotiation.
mod readiness_certificates;
use crate::privacy_engines::{
    aggregate_stark::{
        AggregateFriTheorem2BoundV1, AggregateFriTheorem2CertificateV1, AggregateProofLayoutV1,
        AggregateStarkParametersV1, AggregateTraceGroupLayoutV1,
        validate_affine_batched_fri_theorem2_v1,
    },
    transparent_stark::{
        checked_transparent_stark_work_security_v1, transparent_stark_zk_mask_geometry_v1,
    },
};
use thiserror::Error;
/// Exact native relation version.
pub(crate) const ZK_X509_RELATION_VERSION_V1: u16 = 1;
/// Exact native proof-container version.
pub(crate) const ZK_X509_PROOF_VERSION_V1: u16 = 1;
/// Canonical protocol suite string committed by every transcript.
pub(crate) const ZK_X509_SUITE_V1: &[u8] = b"iroha-zk-x509-stark-p256-v1";
/// Source identity for the original Iroha implementation.
pub(crate) const ZK_X509_SOURCE_PROFILE_V1: &[u8] =
    b"iroha-native-rust:strict-der:rfc5280-p256-sha256:private-chain-crl-ownership:goldilocks-stark:v1";
#[cfg(any(test, feature = "privacy-release-evidence"))]
/// Minimum admitted certificate-chain depth, including leaf and root.
pub(crate) const ZK_X509_MIN_CHAIN_DEPTH_V1: usize = 2;
/// Maximum admitted certificate-chain depth, including leaf and root.
pub(crate) const ZK_X509_MAX_CHAIN_DEPTH_V1: usize = 3;
#[cfg(any(test, feature = "privacy-release-evidence"))]
/// Maximum DER bytes in the private CRL witness.
///
/// Unbounded parsing is deliberately excluded from a consensus circuit.
pub(crate) const ZK_X509_MAX_CRL_BYTES_V1: usize = 4_096;
/// Maximum revoked-certificate entries accepted in the private DER CRL.
///
/// Every active serial is parsed and compared in the RFC 5280 AIR. Sixty-four entries keep that
/// exact comparison table bounded. A policy whose issuer's complete base CRL exceeds this ceiling
/// is unusable under V1; partitioning, delta CRLs, and omission are not fallback paths.
pub(crate) const ZK_X509_MAX_CRL_ENTRIES_V1: usize = 64;
/// Maximum canonical unsigned certificate-serial bytes.
pub(crate) const ZK_X509_MAX_SERIAL_BYTES_V1: usize = 20;
/// Maximum accepted lag between trusted block time and CRL `thisUpdate`.
pub(crate) const ZK_X509_MAX_CRL_AGE_SECONDS_V1: u64 = 300;
/// Fixed private salt width for one subject-attribute commitment.
pub(crate) const ZK_X509_ATTRIBUTE_SALT_BYTES_V1: usize = 32;
/// Maximum exact DER content bytes in one committed subject attribute.
pub(crate) const ZK_X509_MAX_ATTRIBUTE_VALUE_BYTES_V1: usize = 256;
/// Fixed uncompressed P-256 affine public-key width, including SEC1 prefix.
pub(crate) const ZK_X509_UNCOMPRESSED_P256_BYTES_V1: usize = 65;
#[cfg(any(test, feature = "privacy-release-evidence"))]
/// RFC 5280 client-authentication EKU OID.
pub(crate) const ZK_X509_CLIENT_AUTHENTICATION_EKU_OID_V1: &str = "1.3.6.1.5.5.7.3.2";
#[cfg(test)]
/// Iroha document-signing EKU OID.
///
/// The `2.25` UUID OID is UUIDv5(URL namespace,
/// `https://hyperledger.github.io/iroha/eku/document-signing/v1`).
pub(crate) const ZK_X509_DOCUMENT_SIGNING_EKU_OID_V1: &str =
    "2.25.27309815684091774780500832483404888315";
#[cfg(test)]
/// Iroha wallet-identity EKU OID.
///
/// The `2.25` UUID OID is UUIDv5(URL namespace,
/// `https://hyperledger.github.io/iroha/eku/wallet-identity/v1`).
pub(crate) const ZK_X509_WALLET_IDENTITY_EKU_OID_V1: &str =
    "2.25.325405717892146366947968947126277329515";
#[cfg(any(test, feature = "privacy-release-evidence"))]
/// DER content octets of the Iroha document-signing EKU OID.
pub(crate) const ZK_X509_DOCUMENT_SIGNING_EKU_DER_VALUE_V1: &[u8] = &[
    0x69, 0xa9, 0x8b, 0xd6, 0xf7, 0xd8, 0x86, 0xaa, 0xc4, 0xb3, 0x8b, 0xcd, 0xbc, 0xba, 0xe7, 0x86,
    0x8c, 0xf1, 0x7b,
];
#[cfg(any(test, feature = "privacy-release-evidence"))]
/// DER content octets of the Iroha wallet-identity EKU OID.
pub(crate) const ZK_X509_WALLET_IDENTITY_EKU_DER_VALUE_V1: &[u8] = &[
    0x69, 0x83, 0xe9, 0xce, 0xee, 0xa4, 0xdc, 0xb4, 0xda, 0xe5, 0xdf, 0xbe, 0xf0, 0xac, 0xd6, 0xd0,
    0xac, 0xc5, 0xa4, 0x6b,
];
/// Domain for the canonical SHA-256 field-framing function.
pub(crate) const ZK_X509_HASH_FRAME_DOMAIN_V1: &[u8] = b"iroha.zk-x509.sha256.frame.v1";
/// Domain for occupied CA leaves.
pub(crate) const ZK_X509_CA_LEAF_DOMAIN_V1: &[u8] = b"iroha.zk-x509.ca.leaf.v1";
#[cfg(any(test, feature = "privacy-release-evidence"))]
/// Domain for empty CA leaves.
pub(crate) const ZK_X509_CA_EMPTY_LEAF_DOMAIN_V1: &[u8] = b"iroha.zk-x509.ca.empty-leaf.v1";
/// Domain for CA internal tree nodes.
pub(crate) const ZK_X509_CA_NODE_DOMAIN_V1: &[u8] = b"iroha.zk-x509.ca.node.v1";
/// Domain for the governance-scoped leaf-SPKI commitment.
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
/// Canonical schema of one immutable governed trust-anchor revision.
pub(crate) const ZK_X509_TRUST_ANCHOR_REVISION_SCHEMA_V1: &[u8] = b"implicit_version:u16-be=1|trust_anchor_id:bytes32|record_epoch:u64-be|trust_store_digest:bytes32|ca_membership_root:bytes32-compact-depth12|ca_membership_root_epoch:u64-be|previous_record_digest:tag-plus-bytes32|lifecycle:u8-active-or-revoked|record_digest:domain-framed-sha256";
/// Canonical schema of one immutable governed certificate-policy revision.
pub(crate) const ZK_X509_CERTIFICATE_POLICY_REVISION_SCHEMA_V1: &[u8] = b"implicit_version:u16-be=1|trust_anchor_id:bytes32|policy_id:bytes32|record_epoch:u64-be|policy_digest:bytes32|required_key_usage:u8-mask-bits0through3-upper-zero|required_extended_key_usages:u8-count-plus-strictly-sorted-u8-codes-max3|required_disclosed_attribute_indices:u8-count-plus-strictly-sorted-u8-indices-max4|previous_record_digest:tag-plus-bytes32|lifecycle:u8-active-or-revoked|record_digest:domain-framed-sha256";
/// Canonical schema of one immutable governed CRL revision.
///
/// The authoritative data-model record must carry all of these fields and a domain-separated
/// self-digest. The relation recomputes `der_digest`, `issuer_spki_digest`, and both times from the
/// private signed CRL, then proves the leaf serial differs from every active entry. There is
/// deliberately no parallel sparse-root state that could diverge from the signed CRL.
pub(crate) const ZK_X509_CRL_REVISION_SCHEMA_V1: &[u8] = b"implicit_version:u16-be=1|trust_anchor_id:bytes32|certificate_policy_id:bytes32|record_epoch:u64-be|crl_number:u64-be-strictly-increasing|crl_der_digest:domain-framed-sha256-of-complete-exact-signed-der|issuer_spki_digest:domain-framed-sha256|this_update_unix_seconds:u64-be|next_update_unix_seconds:u64-be|complete-crl-entry-nonrevocation:proved-in-rfc5280-stark-no-secondary-root|previous_record_digest:tag-plus-bytes32|lifecycle:u8-active-or-revoked|record_digest:domain-framed-sha256";
/// Closed revocation scope for the first release.
///
/// One certificate-policy lineage identifies exactly one leaf-certificate issuer and its complete,
/// non-partitioned signed CRL. The relation checks revocation of the leaf certificate only.
/// Deployments with multiple intermediate issuers use separate policy lineages; delta CRLs,
/// indirect CRLs, distribution-point partitions, and incomplete shard claims are rejected rather
/// than interpreted.
pub(crate) const ZK_X509_CRL_SCOPE_PROFILE_V1: &[u8] = b"leaf-only:one-issuer-per-certificate-policy:complete-base-crl:no-delta:no-indirect:no-partition:no-distribution-point";
/// Canonical rules for the admitted complete base CRL.
///
/// Every CRL is v2, direct, complete, and issuer-scoped. AKI and CRLNumber are required exactly
/// once and non-critical. CRLNumber must fit `u64` and strictly increase in governance. Delta,
/// indirect, partitioned, and distribution-point CRLs are forbidden. No CRL-entry extension is
/// allowed, including certificateIssuer and reasonCode.
pub(crate) const ZK_X509_CRL_PROFILE_V1: &[u8] = b"rfc5280-crl-closed-v1:der-only:v2:ecdsa-sha256-absent-params:aki-required-noncritical:crl-number-required-noncritical-u64-strictly-increasing:no-delta-crl-indicator:no-issuing-distribution-point:no-freshest-crl:no-indirect:no-partition:no-entry-extensions:complete-base-crl:max64-revoked-entries";
/// Canonical rules for the admitted RFC 5280 subset.
///
/// These bytes are included in the parameter/engine manifest. They are intentionally exhaustive:
/// version 3 only; strict DER with one top-level value; positive non-zero serials of at most 20 DER
/// content octets; exact issuer/subject DER equality; exact UTC encoding (`UTCTime` through 2049,
/// `GeneralizedTime` from 2050, seconds and `Z`, no offset/fraction); no issuer or subject unique
/// IDs; ECDSA-with-SHA256 with absent parameters in both outer and TBS identifiers; uncompressed
/// prime256v1 SPKIs; duplicate, unparsed, unsupported, and unlisted extensions rejected regardless
/// of criticality; AKI/SKI required and linked; BasicConstraints and KeyUsage critical; leaf-only
/// critical EKU; CA/pathLen/keyCertSign/cRLSign enforced; root self-name and self-signature
/// enforced. NameConstraints, PolicyMappings, CertificatePolicies, PolicyConstraints,
/// InhibitAnyPolicy, alternate names, distribution points, and every other extension are forbidden
/// rather than silently ignored.
pub(crate) const ZK_X509_RFC5280_PROFILE_V1: &[u8] = b"rfc5280-closed-v1:der-only:v3:serial-positive-nonzero-max20:exact-name-der:name-oids-only-c-o-ou-cn:no-duplicate-name-attributes:c-printablestring-two-uppercase-ascii:o-ou-cn-utf8string-or-printablestring:max-name-value256:disclosed-attribute-hash-content-octets-only:utf8-well-formed-no-u0000-u001f-u007f-u009f:utc-time-1970-2049:generalized-time-2050-9999:seconds-z-no-fraction:certificate-validity-inclusive:no-unique-id:ecdsa-sha256-absent-params:spki-id-ec-public-key-prime256v1-uncompressed:extensions-exact-order-aki-ski-keyusage-basicconstraints-and-optional-leaf-eku:no-duplicates-no-unknown:aki-ski-linked:bc-ku-eku-critical:ca-keycertsign-and-crlsign-only:ca-pathlen-required-explicit:direct-leaf-issuer-crlsign:root-self-name-self-signature:leaf-revocation-only:no-nameconstraints:no-policy-mapping:no-certificate-policies:no-policy-constraints:no-inhibit-any-policy:no-alt-names:no-distribution-points:no-freshest-crl";
/// ECDSA signature canonicalization rules.
///
/// Certificate and CRL signatures accept both mathematically valid `s` halves, as required for
/// interoperable RFC 5280 verification, but their ASN.1 `SEQUENCE(INTEGER r, INTEGER s)` must be
/// minimal DER. The fresh wallet ownership signature is controlled by this protocol and must
/// additionally use low `s`, eliminating an avoidable witness malleability.
pub(crate) const ZK_X509_ECDSA_RULES_V1: &[u8] =
    b"cert-and-crl:ecdsa-with-sha256-over-exact-tbs:minimal-der-rs-valid-range-high-or-low-s|wallet:ecdsa-p256-prehash-over-exact-32-byte-ownership-digest:minimal-der-rs-valid-range-low-s";
/// Goldilocks prime `2^64 - 2^32 + 1`.
pub(crate) const ZK_X509_GOLDILOCKS_MODULUS_V1: u64 = 0xffff_ffff_0000_0001;
/// Maximum native trace logarithm in the canonical aggregate registration.
pub(crate) const ZK_X509_MAX_NATIVE_TRACE_LOG2_V1: u8 = 19;
/// Maximum native rows in one logical registration.
pub(crate) const ZK_X509_MAX_NATIVE_TRACE_ROWS_V1: u32 = 1 << ZK_X509_MAX_NATIVE_TRACE_LOG2_V1;
/// Exact logical registrations in the canonical X5S1 aggregate.
pub(crate) const ZK_X509_LOGICAL_REGISTRATIONS_V1: usize = 49;
/// Exact equal-native-log trace groups in the canonical X5S1 aggregate.
///
/// Their native logarithms are `[5, 8, 15, 16, 18, 19]`.
pub(crate) const ZK_X509_TRACE_GROUPS_V1: usize = 6;
/// Exact 64-column physical commitment chunks in the canonical X5S1 aggregate.
pub(crate) const ZK_X509_PHYSICAL_COMMITMENT_CHUNKS_V1: usize = 80;
/// Column width of one independently committed physical chunk.
pub(crate) const ZK_X509_PHYSICAL_COMMITMENT_CHUNK_COLUMNS_V1: u16 = 64;
/// Maximum columns transformed in one streaming LDE batch.
pub(crate) const ZK_X509_LDE_COLUMN_BATCH_V1: u16 = 8;
/// Hard peak-memory envelope for the native prover.
///
/// The degree-seven DER composition stage has a 10.13 GiB retained-allocation
/// lower bound before allocator and stack overhead. Twelve GiB remains the
/// release ceiling, not a claimed measurement; activation stays closed until
/// the optimized prover is measured below it on the release benchmark.
pub(crate) const ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1: u64 = 12 * 1024 * 1024 * 1024;
/// Exact virtual-address-space containment ceiling for one release proof.
///
/// This is deliberately larger than the resident-set ceiling so the sealed
/// process can accommodate non-resident mappings, thread stacks, and allocator
/// reserves without weakening the exact first-release process profile.
pub(crate) const ZK_X509_PROVER_ADDRESS_SPACE_CEILING_BYTES_V1: u64 = 32 * 1024 * 1024 * 1024;
/// Hard wall-clock target for one proof on the release benchmark machine.
pub(crate) const ZK_X509_PROVER_TARGET_SECONDS_V1: u64 = 300;
/// Maximum algebraic constraint degree before quotienting.
///
/// The complete strict-DER evaluator attains degree seven. This is measured independently over
/// affine row samples; registering the former degree-four ceiling truncated genuine DER quotients
/// and made proof construction fail closed.
pub(crate) const ZK_X509_MAX_CONSTRAINT_DEGREE_V1: u8 = 7;
/// Low-degree-extension blow-up factor.
pub(crate) const ZK_X509_FRI_BLOWUP_FACTOR_V1: u8 = 64;
/// Sole common MAIN LDE logarithm.
///
/// The maximum native registration is the four-segment SHA batch at log 19;
/// the release blow-up is 64 (log 6), so every MAIN prover, verifier, fixed
/// preprocessing certificate, and resource calculation must use log 25.
pub(crate) const ZK_X509_MAIN_COMMON_LDE_LOG2_V1: u8 =
    ZK_X509_MAX_NATIVE_TRACE_LOG2_V1 + ZK_X509_FRI_BLOWUP_FACTOR_V1.ilog2() as u8;
/// Number of independent Fiat-Shamir query positions.
pub(crate) const ZK_X509_FRI_QUERY_COUNT_V1: u8 = 58;
/// Number of independently mixed composition lanes.
pub(crate) const ZK_X509_COMPOSITION_LANES_V1: u8 = 1;
/// Binary FRI folding arity.
pub(crate) const ZK_X509_FRI_FOLDING_ARITY_V1: u8 = 2;
/// Maximum final FRI polynomial length.
pub(crate) const ZK_X509_FRI_FINAL_POLYNOMIAL_LENGTH_V1: u16 = 1_024;
/// Maximum degree of the terminal FRI polynomial.
pub(crate) const ZK_X509_FRI_TERMINAL_DEGREE_BOUND_V1: u16 = 31;
/// Coefficient chunks used to normalize the maximum degree-seven quotient.
pub(crate) const ZK_X509_COMPOSITION_DEGREE_CHUNKS_V1: u8 = 4;
/// Inclusive degree of each trace zero-knowledge mask multiplier.
///
/// Haböck--Al Kindi Equation (3), with reduced AIR degree six, Fp4 extension degree four, one DEEP
/// point, and 58 FRI queries, requires 802 randomizer coefficients.
pub(crate) const ZK_X509_TRACE_MASK_DEGREE_V1: u16 = 801;
/// Exact common-domain FRI rounds at the maximum native trace size.
pub(crate) const ZK_X509_FRI_ROUNDS_V1: u8 = 15;
/// Exclusive common FRI degree cap.
pub(crate) const ZK_X509_FRI_EXCLUSIVE_DEGREE_CAP_V1: u32 = 1_048_576;
/// Exact effective-code-rate numerator.
pub(crate) const ZK_X509_FRI_EFFECTIVE_RATE_NUMERATOR_V1: u16 = 1;
/// Exact effective-code-rate denominator.
pub(crate) const ZK_X509_FRI_EFFECTIVE_RATE_DENOMINATOR_V1: u16 = 32;
/// BCI/Haböck affine batching parameter, distinct from the one Fp4 lane.
pub(crate) const ZK_X509_FRI_BATCHING_PARAMETER_M_V1: u8 = 3;
/// Exact affine arities in theorem order.
pub(crate) const ZK_X509_FRI_AFFINE_ARITIES_V1: [u8; 3] = [2, 2, 2];
/// Proven lower-bound exponent `|F_{p^4}| > 2^252`.
pub(crate) const ZK_X509_EXTENSION_FIELD_LOWER_BOUND_BITS_V1: u16 = 252;
/// Number of transcript-derived DEEP points per subproof.
pub(crate) const ZK_X509_DEEP_POINT_COUNT_V1: usize = 1;
/// Compact-CA local LDE logarithm.
pub(crate) const ZK_X509_CA_FRI_LDE_LOG2_V1: u8 = 14;
/// Compact-CA FRI terminal logarithm.
pub(crate) const ZK_X509_CA_FRI_TERMINAL_LOG2_V1: u8 = 9;
/// Compact-CA terminal degree bound.
pub(crate) const ZK_X509_CA_FRI_TERMINAL_DEGREE_BOUND_V1: u16 = 15;
/// Compact-CA binary fold count.
pub(crate) const ZK_X509_CA_FRI_ROUNDS_V1: u8 = 5;
/// Compact-CA composition coefficient chunks.
pub(crate) const ZK_X509_CA_COMPOSITION_DEGREE_CHUNKS_V1: u8 = 3;
/// Compact-CA trace mask degree for `h = 5q + 16 = 306`.
pub(crate) const ZK_X509_CA_TRACE_MASK_DEGREE_V1: u16 = 305;
/// Exact maximum of the main aggregate proof before DEEP openings.
pub(crate) const ZK_X509_MAIN_PRE_DEEP_MAXIMUM_BYTES_V1: u32 = 6_812_632;
/// Exact maximum of the compact-CA aggregate proof before DEEP openings.
pub(crate) const ZK_X509_CA_PRE_DEEP_MAXIMUM_BYTES_V1: u32 = 984_216;
/// Exact X5C1 claim-envelope bytes around the compact-CA aggregate proof.
pub(crate) const ZK_X509_CA_CLAIM_ENVELOPE_BYTES_V1: u32 = 1_310;
/// Exact X5M1 fixed framing plus DER, RFC, SHA, and P-256 terminal frames.
pub(crate) const ZK_X509_MAIN_CLAIM_ENVELOPE_BYTES_V1: u32 = 11_952;
/// Exact main plus compact-CA current/next Fp4 DEEP openings and composition claims.
pub(crate) const ZK_X509_DEEP_OPENING_BYTES_V1: u32 = 402_336;
/// Fiat-Shamir proof-of-work grinding bits.
pub(crate) const ZK_X509_GRINDING_BITS_V1: u8 = 20;
/// Required computational-soundness target for the released proof system.
///
/// The finalized analysis derives this bound from the implemented AIR degree
/// schedule, FRI proximity theorem, transcript, opening schedule, hash
/// assumptions, and every verifier union-bound event.
pub(crate) const ZK_X509_TARGET_SOUNDNESS_BITS_V1: u16 = 128;
/// Consensus proof-byte ceiling inherited by this engine.
pub(crate) const ZK_X509_MAX_PROOF_BYTES_V1: u32 = 9 * 1024 * 1024;
/// Exact maximum encoded canonical aggregate proof under the frozen layout.
pub(crate) const ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1: u32 = 8_212_538;
/// Exact encoded byte length of the deterministic first-release X5S1 KAT.
///
/// This stays zero only while the one-time capture corridor is open. The production activation gate
/// rejects a zero length or a length above the canonical X5S1 ceiling.
pub(crate) const ZK_X509_RELEASE_KAT_EXPECTED_PROOF_BYTES_V1: u32 = 0;
/// SHA-256 of the deterministic first-release X5S1 KAT proof.
pub(crate) const ZK_X509_RELEASE_KAT_EXPECTED_PROOF_SHA256_V1: [u8; 32] = [0; 32];
/// SHA-256 of the authoritative native-release expectations Norito fixture.
pub(crate) const ZK_X509_NATIVE_RELEASE_EXPECTATIONS_NORITO_SHA256_V1: [u8; 32] = [0; 32];
/// SHA-256 of the typed-equal native-release expectations JSON projection.
pub(crate) const ZK_X509_NATIVE_RELEASE_EXPECTATIONS_JSON_SHA256_V1: [u8; 32] = [0; 32];
/// Frozen first-release transparent-proof envelope descriptor.
///
/// Consensus activation remains governance-gated until the deterministic KAT
/// and maximum-shape process measurements are pinned below.
pub(crate) const ZK_X509_STARK_PROFILE_DESCRIPTOR_V1: &[u8] = b"field=goldilocks-fp4:w4=7:base=0xffffffff00000001|wire=X5S1-containing-exactly-one-X5M1-and-one-X5C1-v1|x5m1=claims-plus-length-delimited-aggregate-only-no-fixed-sidecar|main-logical-registrations=49|main-same-log-trace-groups=6-logs5,8,15,16,18,19|main-physical-commitment-chunks=80|physical-chunk-columns=64|max-native-trace-log2=19|compact-ca-dedicated-log7-subproof-depth12|sha-fixed-calls=29-across-four-log19-slices|sha-fixed-algebraic-width=472-verifier-derived-no-proof-bytes|p256-log19-fixed-algebraic-width=404-six-role-schedules-alias-fifteen-registrations-verifier-derived-no-proof-bytes|fixed-openings=canonical-sorted-unique-current-next-union-after-grinding-max116|shared-x5b1-challenges=all-six-main-base-roots+ca-base-root+main-and-ca-public-profile+exact272-fields-ordered-sha-call,rfc,projection,io,der,sha-word-memory,sha-word-base-fold,p256-value,p256-cross,p256-scalar,p256-arithmetic-copy+one-opaque-main-post-base-token|main-io=statement-only-exact40+5d-declarations+logical55922+4736d-active-rows+fixed-capacity262144|main-trace-hiding-coefficients=802|ca-trace-hiding-coefficients=306|fri-mask-oracles=1-fp4-per-subproof-roots-before-batching|lde-column-batch=8|max-constraint-degree=7|fri-rate=1over32|main-fri-blowup=64|ca-lde-log2=14|fri-queries=58-distinct-without-replacement|composition-fp4-lanes=1|fri-batching-m=3|affine-arities=2,2,2|fri-folding=2|main-fri-terminal-length=1024-degree31|ca-fri-terminal-length=512-degree15|deep-points=1-per-subproof-current+next-openings|grinding-bits=20|target-soundness-bits=128|rbr-budget-bits=129|random-oracle-kappa=256|max-ro-queries-log2=64|max-encoded-combined-bound=8212538|max-proof-bytes=9437184|peak-memory-ceiling-bytes=12884901888|address-space-ceiling-bytes=34359738368|prover-target-seconds=300|release-evidence-schema=deterministic-X5S1-KAT+public-binding-mutations+wire-corruption-and-truncation+maximum-shape-process-measurement|full-credential-verifier=complete|activation=governance-gated";
#[cfg(feature = "privacy-release-evidence")]
pub(crate) use readiness_certificates::{
    ZK_X509_RESOURCE_CERTIFICATE_SCHEMA_VERSION_V1, ZkX509ResourceCertificateV1,
    ZkX509ResourceEnvironmentV1, ZkX509ResourceObservationV1, ZkX509ResourceProcessLimitsV1,
    canonical_resource_environment_v1, canonical_resource_process_limits_v1,
    resource_certificate_matches_source_v1, validate_resource_certificate_payload_v1,
};
use readiness_certificates::{
    ZK_X509_RESOURCE_CERTIFICATE_SHA256_V1, ZK_X509_SOUNDNESS_CERTIFICATE_SHA256_V1,
    resource_certificate_is_pinned_v1, soundness_certificate_is_pinned_v1,
};
use readiness_certificates::{
    ZK_X509_RESOURCE_MAXIMUM_ELAPSED_MILLIS_V1,
    ZK_X509_RESOURCE_MAXIMUM_PEAK_ADDRESS_SPACE_BYTES_V1,
    ZK_X509_RESOURCE_MAXIMUM_PEAK_RSS_BYTES_V1, ZK_X509_RESOURCE_POSITIVE_ELAPSED_MILLIS_V1,
    ZK_X509_RESOURCE_POSITIVE_PEAK_ADDRESS_SPACE_BYTES_V1,
    ZK_X509_RESOURCE_POSITIVE_PEAK_RSS_BYTES_V1,
};
const fn digest_is_nonzero_v1(digest: [u8; 32]) -> bool {
    let mut index = 0;
    while index < digest.len() {
        if digest[index] != 0 {
            return true;
        }
        index += 1;
    }
    false
}
const fn digests_differ_v1(left: [u8; 32], right: [u8; 32]) -> bool {
    let mut index = 0;
    while index < left.len() {
        if left[index] != right[index] {
            return true;
        }
        index += 1;
    }
    false
}
const fn release_evidence_pins_are_complete_v1(
    kat_proof_bytes: u32,
    kat_proof_sha256: [u8; 32],
    expectations_norito_sha256: [u8; 32],
    expectations_json_sha256: [u8; 32],
) -> bool {
    kat_proof_bytes > 0
        && kat_proof_bytes <= ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1
        && digest_is_nonzero_v1(kat_proof_sha256)
        && digest_is_nonzero_v1(expectations_norito_sha256)
        && digest_is_nonzero_v1(expectations_json_sha256)
        && digests_differ_v1(expectations_norito_sha256, expectations_json_sha256)
}
#[derive(Clone, Copy)]
struct ZkX509ReleaseCapturePinsV1 {
    kat_proof_bytes: u32,
    kat_proof_sha256: [u8; 32],
    expectations_norito_sha256: [u8; 32],
    expectations_json_sha256: [u8; 32],
    resource_certificate_sha256: [u8; 32],
    positive_elapsed_millis: u64,
    positive_peak_rss_bytes: u64,
    positive_peak_address_space_bytes: u64,
    maximum_elapsed_millis: u64,
    maximum_peak_rss_bytes: u64,
    maximum_peak_address_space_bytes: u64,
}
const fn source_release_capture_pins_v1() -> ZkX509ReleaseCapturePinsV1 {
    ZkX509ReleaseCapturePinsV1 {
        kat_proof_bytes: ZK_X509_RELEASE_KAT_EXPECTED_PROOF_BYTES_V1,
        kat_proof_sha256: ZK_X509_RELEASE_KAT_EXPECTED_PROOF_SHA256_V1,
        expectations_norito_sha256: ZK_X509_NATIVE_RELEASE_EXPECTATIONS_NORITO_SHA256_V1,
        expectations_json_sha256: ZK_X509_NATIVE_RELEASE_EXPECTATIONS_JSON_SHA256_V1,
        resource_certificate_sha256: ZK_X509_RESOURCE_CERTIFICATE_SHA256_V1,
        positive_elapsed_millis: ZK_X509_RESOURCE_POSITIVE_ELAPSED_MILLIS_V1,
        positive_peak_rss_bytes: ZK_X509_RESOURCE_POSITIVE_PEAK_RSS_BYTES_V1,
        positive_peak_address_space_bytes: ZK_X509_RESOURCE_POSITIVE_PEAK_ADDRESS_SPACE_BYTES_V1,
        maximum_elapsed_millis: ZK_X509_RESOURCE_MAXIMUM_ELAPSED_MILLIS_V1,
        maximum_peak_rss_bytes: ZK_X509_RESOURCE_MAXIMUM_PEAK_RSS_BYTES_V1,
        maximum_peak_address_space_bytes: ZK_X509_RESOURCE_MAXIMUM_PEAK_ADDRESS_SPACE_BYTES_V1,
    }
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
const fn native_release_capture_open_with_pins_v1(pins: ZkX509ReleaseCapturePinsV1) -> bool {
    pins.kat_proof_bytes == 0
        && !digest_is_nonzero_v1(pins.kat_proof_sha256)
        && !digest_is_nonzero_v1(pins.expectations_norito_sha256)
        && !digest_is_nonzero_v1(pins.expectations_json_sha256)
        && !digest_is_nonzero_v1(pins.resource_certificate_sha256)
        && pins.positive_elapsed_millis == 0
        && pins.positive_peak_rss_bytes == 0
        && pins.positive_peak_address_space_bytes == 0
        && pins.maximum_elapsed_millis == 0
        && pins.maximum_peak_rss_bytes == 0
        && pins.maximum_peak_address_space_bytes == 0
}
const fn native_release_capture_pins_complete_v1(pins: ZkX509ReleaseCapturePinsV1) -> bool {
    release_evidence_pins_are_complete_v1(
        pins.kat_proof_bytes,
        pins.kat_proof_sha256,
        pins.expectations_norito_sha256,
        pins.expectations_json_sha256,
    ) && digest_is_nonzero_v1(pins.resource_certificate_sha256)
        && pins.positive_elapsed_millis > 0
        && pins.positive_peak_rss_bytes > 0
        && pins.positive_peak_address_space_bytes > 0
        && pins.maximum_elapsed_millis > 0
        && pins.maximum_peak_rss_bytes > 0
        && pins.maximum_peak_address_space_bytes > 0
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
const fn native_release_expectation_digests_match_with_pins_v1(
    expected_norito_sha256: [u8; 32],
    expected_json_sha256: [u8; 32],
    actual_norito_sha256: [u8; 32],
    actual_json_sha256: [u8; 32],
) -> bool {
    digest_is_nonzero_v1(expected_norito_sha256)
        && digest_is_nonzero_v1(expected_json_sha256)
        && !digests_differ_v1(actual_norito_sha256, expected_norito_sha256)
        && !digests_differ_v1(actual_json_sha256, expected_json_sha256)
}
/// Whether every deterministic proof and native fixture pin is populated.
#[cfg(test)]
pub(crate) const fn zk_x509_release_evidence_pins_complete_v1() -> bool {
    release_evidence_pins_are_complete_v1(
        ZK_X509_RELEASE_KAT_EXPECTED_PROOF_BYTES_V1,
        ZK_X509_RELEASE_KAT_EXPECTED_PROOF_SHA256_V1,
        ZK_X509_NATIVE_RELEASE_EXPECTATIONS_NORITO_SHA256_V1,
        ZK_X509_NATIVE_RELEASE_EXPECTATIONS_JSON_SHA256_V1,
    )
}
/// Whether the one-time native expectation capture corridor remains open.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) const fn zk_x509_native_release_expectation_capture_open_v1() -> bool {
    native_release_capture_open_with_pins_v1(source_release_capture_pins_v1())
}
/// Whether an expectation pair matches both compiled release pins exactly.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) const fn zk_x509_native_release_expectation_digests_match_v1(
    norito_sha256: [u8; 32],
    json_sha256: [u8; 32],
) -> bool {
    native_release_expectation_digests_match_with_pins_v1(
        ZK_X509_NATIVE_RELEASE_EXPECTATIONS_NORITO_SHA256_V1,
        ZK_X509_NATIVE_RELEASE_EXPECTATIONS_JSON_SHA256_V1,
        norito_sha256,
        json_sha256,
    )
}
/// Closed checklist that must be true before activation can be compiled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ZkX509ReadinessV1 {
    /// Strict DER parser and RFC 5280 state machine are constrained.
    pub(crate) der_and_rfc5280_air: bool,
    /// SHA-256 compression and bit/range lookups are constrained.
    pub(crate) sha256_air: bool,
    /// P-256 field/group arithmetic and ECDSA verification are constrained.
    pub(crate) p256_air: bool,
    /// Compact CA membership is constrained.
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
/// Canonical production checklist derived from concrete release pins and
/// independently validated certificate payloads.
pub(crate) fn zk_x509_activation_readiness_v1() -> ZkX509ReadinessV1 {
    let source_capture_pins = source_release_capture_pins_v1();
    // A proof digest by itself is not evidence that either canonical projection
    // of the 48-stage native corpus was captured. Keep the KAT and adversarial
    // bits false until the proof and both distinct fixture digests are present.
    let deterministic_release_corpus = release_evidence_pins_are_complete_v1(
        source_capture_pins.kat_proof_bytes,
        source_capture_pins.kat_proof_sha256,
        source_capture_pins.expectations_norito_sha256,
        source_capture_pins.expectations_json_sha256,
    );
    let resource_source_pins_complete =
        native_release_capture_pins_complete_v1(source_capture_pins);
    let soundness_pin_populated = digest_is_nonzero_v1(ZK_X509_SOUNDNESS_CERTIFICATE_SHA256_V1);
    let compiled_profile_digest = (soundness_pin_populated || resource_source_pins_complete)
        .then(|| super::engine::construct_zk_x509_compiled_profile_v1().ok())
        .flatten()
        .map(super::engine::ZkX509CompiledProfileV1::digest);
    let soundness_analysis = soundness_pin_populated
        && compiled_profile_digest.is_some_and(soundness_certificate_is_pinned_v1);
    let resource_benchmarks = resource_source_pins_complete
        && compiled_profile_digest.is_some_and(resource_certificate_is_pinned_v1);
    ZkX509ReadinessV1 {
        der_and_rfc5280_air: true,
        sha256_air: true,
        p256_air: true,
        accumulator_air: true,
        output_projection_air: true,
        prover: true,
        verifier: true,
        known_answer_tests: deterministic_release_corpus,
        adversarial_tests: deterministic_release_corpus,
        differential_tests: true,
        soundness_analysis,
        resource_benchmarks,
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
fn fri_parameters_v1(
    native_trace_log2: u8,
    blowup_log2: u8,
    terminal_log2: u8,
    terminal_degree_bound: usize,
    composition_degree_chunks: usize,
) -> AggregateStarkParametersV1 {
    AggregateStarkParametersV1 {
        proof_magic: *b"X5S1",
        proof_version: ZK_X509_PROOF_VERSION_V1,
        security_lanes: usize::from(ZK_X509_COMPOSITION_LANES_V1),
        query_count: usize::from(ZK_X509_FRI_QUERY_COUNT_V1),
        blowup_log2,
        terminal_log2,
        terminal_degree_bound,
        composition_degree_chunks,
        minimum_trace_log2: native_trace_log2,
        maximum_trace_log2: native_trace_log2,
        maximum_trace_groups: 1,
        maximum_segment_instances: 1,
        maximum_base_columns_per_instance: 1,
        maximum_aux_columns_per_instance: 1,
        maximum_proof_bytes: ZK_X509_MAX_PROOF_BYTES_V1 as usize,
    }
}
fn fri_theorem_certificate_v1(
    domain_log2: u8,
    fold_count: u8,
    terminal_log2: u8,
    terminal_degree_bound: u16,
) -> AggregateFriTheorem2CertificateV1 {
    AggregateFriTheorem2CertificateV1 {
        l_minus_one_numerator: 3,
        l_minus_one_denominator: 2,
        batching_parameter_m: ZK_X509_FRI_BATCHING_PARAMETER_M_V1,
        rho_numerator: ZK_X509_FRI_EFFECTIVE_RATE_NUMERATOR_V1 as u8,
        rho_denominator: ZK_X509_FRI_EFFECTIVE_RATE_DENOMINATOR_V1 as u8,
        affine_arities: ZK_X509_FRI_AFFINE_ARITIES_V1,
        domain_log2,
        extension_field_lower_bound_bits: ZK_X509_EXTENSION_FIELD_LOWER_BOUND_BITS_V1,
        base_field_two_adicity: 32,
        trace_domains_are_smooth_subgroups: true,
        evaluation_domain_is_smooth_generator_coset: true,
        evaluation_domain_is_disjoint_from_trace_domains: true,
        fold_count,
        terminal_log2,
        terminal_degree_bound,
        query_count: ZK_X509_FRI_QUERY_COUNT_V1,
        distinct_queries_without_replacement: true,
        uniform_rejection_sampling: true,
        claimed_query_error_bits: 132,
    }
}
fn validate_fri_subproof_v1(
    native_trace_log2: u8,
    blowup_log2: u8,
    terminal_log2: u8,
    terminal_degree_bound: u16,
    fold_count: u8,
    composition_degree_chunks: u8,
) -> Result<AggregateFriTheorem2BoundV1, ZkX509ProfileErrorV1> {
    let parameters = fri_parameters_v1(
        native_trace_log2,
        blowup_log2,
        terminal_log2,
        usize::from(terminal_degree_bound),
        usize::from(composition_degree_chunks),
    );
    let layout = AggregateProofLayoutV1::new(
        parameters,
        vec![AggregateTraceGroupLayoutV1 {
            native_trace_log2,
            segment_instances: 1,
            base_width: 1,
            aux_width: 1,
        }],
    )
    .map_err(|_| ZkX509ProfileErrorV1::InvalidStarkParameters)?;
    validate_affine_batched_fri_theorem2_v1(
        parameters,
        &layout,
        fri_theorem_certificate_v1(
            native_trace_log2
                .checked_add(blowup_log2)
                .ok_or(ZkX509ProfileErrorV1::InvalidStarkParameters)?,
            fold_count,
            terminal_log2,
            terminal_degree_bound,
        ),
    )
    .map_err(|_| ZkX509ProfileErrorV1::InvalidStarkParameters)
}
/// Validate the immutable profile constants.
pub(crate) fn validate_profile_v1() -> Result<(), ZkX509ProfileErrorV1> {
    let maximum_native_rows = u64::from(ZK_X509_MAX_NATIVE_TRACE_ROWS_V1);
    let common_lde_rows = maximum_native_rows
        .checked_mul(u64::from(ZK_X509_FRI_BLOWUP_FACTOR_V1))
        .ok_or(ZkX509ProfileErrorV1::InvalidStarkParameters)?;
    let fri_fold_factor = 1_u64
        .checked_shl(u32::from(ZK_X509_FRI_ROUNDS_V1))
        .ok_or(ZkX509ProfileErrorV1::InvalidStarkParameters)?;
    let fri_degree_cap = u64::from(ZK_X509_FRI_TERMINAL_DEGREE_BOUND_V1)
        .checked_add(1)
        .and_then(|coefficients| coefficients.checked_mul(fri_fold_factor))
        .ok_or(ZkX509ProfileErrorV1::InvalidStarkParameters)?;
    let maximum_masked_trace_degree = maximum_native_rows
        .checked_add(u64::from(ZK_X509_TRACE_MASK_DEGREE_V1))
        .ok_or(ZkX509ProfileErrorV1::InvalidStarkParameters)?;
    let maximum_unsplit_quotient_degree = u64::from(ZK_X509_MAX_CONSTRAINT_DEGREE_V1)
        .checked_mul(maximum_masked_trace_degree)
        .and_then(|degree| degree.checked_sub(maximum_native_rows))
        .ok_or(ZkX509ProfileErrorV1::InvalidStarkParameters)?;
    let maximum_chunked_composition_degree = fri_degree_cap
        .checked_mul(u64::from(ZK_X509_COMPOSITION_DEGREE_CHUNKS_V1))
        .and_then(|coefficients| coefficients.checked_sub(1))
        .ok_or(ZkX509ProfileErrorV1::InvalidStarkParameters)?;
    let main_mask = transparent_stark_zk_mask_geometry_v1(
        usize::from(ZK_X509_MAX_CONSTRAINT_DEGREE_V1) - 1,
        4,
        ZK_X509_DEEP_POINT_COUNT_V1,
        usize::from(ZK_X509_FRI_QUERY_COUNT_V1),
    )
    .map_err(|_| ZkX509ProfileErrorV1::InvalidStarkParameters)?;
    let ca_mask = transparent_stark_zk_mask_geometry_v1(
        2,
        4,
        ZK_X509_DEEP_POINT_COUNT_V1,
        usize::from(ZK_X509_FRI_QUERY_COUNT_V1),
    )
    .map_err(|_| ZkX509ProfileErrorV1::InvalidStarkParameters)?;
    let main_fri = validate_fri_subproof_v1(
        ZK_X509_MAX_NATIVE_TRACE_LOG2_V1,
        ZK_X509_FRI_BLOWUP_FACTOR_V1.ilog2() as u8,
        ZK_X509_FRI_FINAL_POLYNOMIAL_LENGTH_V1.ilog2() as u8,
        ZK_X509_FRI_TERMINAL_DEGREE_BOUND_V1,
        ZK_X509_FRI_ROUNDS_V1,
        ZK_X509_COMPOSITION_DEGREE_CHUNKS_V1,
    )?;
    let ca_fri = validate_fri_subproof_v1(
        7,
        ZK_X509_CA_FRI_LDE_LOG2_V1 - 7,
        ZK_X509_CA_FRI_TERMINAL_LOG2_V1,
        ZK_X509_CA_FRI_TERMINAL_DEGREE_BOUND_V1,
        ZK_X509_CA_FRI_ROUNDS_V1,
        ZK_X509_CA_COMPOSITION_DEGREE_CHUNKS_V1,
    )?;
    checked_transparent_stark_work_security_v1(128, 129, 256, 64)
        .map_err(|_| ZkX509ProfileErrorV1::InvalidStarkParameters)?;
    let streaming_lde_batch_bytes = u64::from(ZK_X509_MAX_NATIVE_TRACE_ROWS_V1)
        .checked_mul(u64::from(ZK_X509_LDE_COLUMN_BATCH_V1))
        .and_then(|value| value.checked_mul(core::mem::size_of::<u64>() as u64))
        .and_then(|value| value.checked_mul(u64::from(ZK_X509_FRI_BLOWUP_FACTOR_V1)))
        .ok_or(ZkX509ProfileErrorV1::InvalidTraceEnvelope)?;
    if ZK_X509_MAX_NATIVE_TRACE_ROWS_V1
        != 1_u32
            .checked_shl(u32::from(ZK_X509_MAX_NATIVE_TRACE_LOG2_V1))
            .ok_or(ZkX509ProfileErrorV1::InvalidTraceEnvelope)?
        || ZK_X509_LOGICAL_REGISTRATIONS_V1 == 0
        || ZK_X509_TRACE_GROUPS_V1 == 0
        || ZK_X509_TRACE_GROUPS_V1 > ZK_X509_LOGICAL_REGISTRATIONS_V1
        || ZK_X509_PHYSICAL_COMMITMENT_CHUNKS_V1 < ZK_X509_LOGICAL_REGISTRATIONS_V1
        || ZK_X509_PHYSICAL_COMMITMENT_CHUNK_COLUMNS_V1 != 64
        || ZK_X509_LDE_COLUMN_BATCH_V1 == 0
        || usize::from(ZK_X509_LDE_COLUMN_BATCH_V1)
            != super::super::aggregate_stark::MASKED_TRACE_LDE_COLUMN_BATCH_V1
        || ZK_X509_LDE_COLUMN_BATCH_V1 > ZK_X509_PHYSICAL_COMMITMENT_CHUNK_COLUMNS_V1
        || ZK_X509_PHYSICAL_COMMITMENT_CHUNK_COLUMNS_V1 % ZK_X509_LDE_COLUMN_BATCH_V1 != 0
        || streaming_lde_batch_bytes > ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1
        || ZK_X509_PROVER_ADDRESS_SPACE_CEILING_BYTES_V1 < ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1
        || ZK_X509_PROVER_TARGET_SECONDS_V1 == 0
    {
        return Err(ZkX509ProfileErrorV1::InvalidTraceEnvelope);
    }
    if ZK_X509_GOLDILOCKS_MODULUS_V1 != 0xffff_ffff_0000_0001
        || ZK_X509_FRI_BLOWUP_FACTOR_V1 != 64
        || ZK_X509_FRI_QUERY_COUNT_V1 != 58
        || ZK_X509_COMPOSITION_LANES_V1 != 1
        || ZK_X509_FRI_FOLDING_ARITY_V1 != 2
        || ZK_X509_FRI_FINAL_POLYNOMIAL_LENGTH_V1 != 1_024
        || ZK_X509_FRI_TERMINAL_DEGREE_BOUND_V1 != 31
        || ZK_X509_COMPOSITION_DEGREE_CHUNKS_V1 != 4
        || ZK_X509_TRACE_MASK_DEGREE_V1 != 801
        || main_mask.minimum_mask_coefficients != 802
        || main_mask.minimum_mask_degree != usize::from(ZK_X509_TRACE_MASK_DEGREE_V1)
        || ca_mask.minimum_mask_coefficients != 306
        || ca_mask.minimum_mask_degree != usize::from(ZK_X509_CA_TRACE_MASK_DEGREE_V1)
        || main_fri
            != (AggregateFriTheorem2BoundV1 {
                query_error_bits: 132,
                commitment_error_bits: 181,
            })
        || ca_fri
            != (AggregateFriTheorem2BoundV1 {
                query_error_bits: 132,
                commitment_error_bits: 203,
            })
        || common_lde_rows
            != 1_u64
                .checked_shl(u32::from(ZK_X509_MAIN_COMMON_LDE_LOG2_V1))
                .ok_or(ZkX509ProfileErrorV1::InvalidStarkParameters)?
        || common_lde_rows / u64::from(ZK_X509_FRI_FINAL_POLYNOMIAL_LENGTH_V1) != fri_fold_factor
        || fri_degree_cap != u64::from(ZK_X509_FRI_EXCLUSIVE_DEGREE_CAP_V1)
        || fri_degree_cap.checked_mul(u64::from(ZK_X509_FRI_EFFECTIVE_RATE_DENOMINATOR_V1))
            != common_lde_rows.checked_mul(u64::from(ZK_X509_FRI_EFFECTIVE_RATE_NUMERATOR_V1))
        || maximum_masked_trace_degree >= fri_degree_cap
        || maximum_unsplit_quotient_degree > maximum_chunked_composition_degree
        || ZK_X509_GRINDING_BITS_V1 != 20
        || ZK_X509_TARGET_SOUNDNESS_BITS_V1 != 128
    {
        return Err(ZkX509ProfileErrorV1::InvalidStarkParameters);
    }
    if ZK_X509_MAX_CONSTRAINT_DEGREE_V1 < 2 {
        return Err(ZkX509ProfileErrorV1::InvalidTraceSegmentBound);
    }
    if ZK_X509_MAX_PROOF_BYTES_V1 == 0
        || ZK_X509_MAX_PROOF_BYTES_V1 > 9 * 1024 * 1024
        || ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1 == 0
        || ZK_X509_MAIN_PRE_DEEP_MAXIMUM_BYTES_V1
            .checked_add(ZK_X509_CA_PRE_DEEP_MAXIMUM_BYTES_V1)
            .and_then(|bytes| bytes.checked_add(ZK_X509_DEEP_OPENING_BYTES_V1))
            .and_then(|bytes| bytes.checked_add(ZK_X509_CA_CLAIM_ENVELOPE_BYTES_V1))
            .and_then(|bytes| bytes.checked_add(ZK_X509_MAIN_CLAIM_ENVELOPE_BYTES_V1))
            .and_then(|bytes| {
                bytes.checked_add(
                    super::credential_stark::ZK_X509_CREDENTIAL_ENVELOPE_FRAMING_BYTES_V1 as u32,
                )
            })
            != Some(ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1)
        || ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1 > ZK_X509_MAX_PROOF_BYTES_V1
    {
        return Err(ZkX509ProfileErrorV1::InvalidProofCap);
    }
    Ok(())
}
/// Enforce the compile-time activation and independent readiness gates.
pub(crate) fn require_activation_readiness_v1(
    readiness: ZkX509ReadinessV1,
) -> Result<(), ZkX509ProfileErrorV1> {
    validate_profile_v1()?;
    let canonical = zk_x509_activation_readiness_v1();
    if readiness != canonical || !canonical.is_complete() {
        return Err(ZkX509ProfileErrorV1::EngineIncomplete);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum SourceReleasePinStateV1 {
        BootstrapOpen,
        FullyPinned,
        Mixed,
    }
    const fn source_release_pin_state_v1(
        pins: ZkX509ReleaseCapturePinsV1,
    ) -> SourceReleasePinStateV1 {
        if native_release_capture_open_with_pins_v1(pins) {
            SourceReleasePinStateV1::BootstrapOpen
        } else if native_release_capture_pins_complete_v1(pins) {
            SourceReleasePinStateV1::FullyPinned
        } else {
            SourceReleasePinStateV1::Mixed
        }
    }
    #[test]
    fn fixed_algebraic_profile_and_activation_match_the_source_pin_state() {
        validate_profile_v1().expect("fixed zk-X509 profile");
        assert_eq!(ZK_X509_LOGICAL_REGISTRATIONS_V1, 49);
        assert_eq!(ZK_X509_TRACE_GROUPS_V1, 6);
        assert_eq!(ZK_X509_PHYSICAL_COMMITMENT_CHUNKS_V1, 80);
        assert_eq!(ZK_X509_PHYSICAL_COMMITMENT_CHUNK_COLUMNS_V1, 64);
        assert_eq!(ZK_X509_MAX_NATIVE_TRACE_LOG2_V1, 19);
        assert_eq!(ZK_X509_MAIN_COMMON_LDE_LOG2_V1, 25);
        assert_eq!(ZK_X509_MAIN_PRE_DEEP_MAXIMUM_BYTES_V1, 6_812_632);
        assert_eq!(ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1, 8_212_538);
        assert_eq!(ZK_X509_MAIN_CLAIM_ENVELOPE_BYTES_V1, 11_952);
        assert_eq!(
            ZK_X509_MAX_PROOF_BYTES_V1 - ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1,
            1_224_646
        );
        assert_eq!(ZK_X509_FRI_QUERY_COUNT_V1, 58);
        assert_eq!(ZK_X509_MAX_CONSTRAINT_DEGREE_V1, 7);
        assert_eq!(ZK_X509_TRACE_MASK_DEGREE_V1, 801);
        assert_eq!(ZK_X509_CA_TRACE_MASK_DEGREE_V1, 305);
        assert!(ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1 <= ZK_X509_MAX_PROOF_BYTES_V1);
        assert_eq!(ZK_X509_TARGET_SOUNDNESS_BITS_V1, 128);
        assert_eq!(ZK_X509_PROVER_PEAK_MEMORY_BYTES_V1, 12 * 1024 * 1024 * 1024);
        assert_eq!(
            ZK_X509_PROVER_ADDRESS_SPACE_CEILING_BYTES_V1,
            32 * 1024 * 1024 * 1024
        );
        assert!(
            String::from_utf8_lossy(ZK_X509_STARK_PROFILE_DESCRIPTOR_V1)
                .contains("address-space-ceiling-bytes=34359738368")
        );
        assert_ne!(ZK_X509_SOUNDNESS_CERTIFICATE_SHA256_V1, [0; 32]);
        let readiness = zk_x509_activation_readiness_v1();
        assert!(readiness.soundness_analysis);
        match source_release_pin_state_v1(source_release_capture_pins_v1()) {
            SourceReleasePinStateV1::BootstrapOpen => {
                assert_eq!(ZK_X509_RESOURCE_CERTIFICATE_SHA256_V1, [0; 32]);
                assert!(!readiness.resource_benchmarks);
                assert!(!readiness.is_complete());
                assert_eq!(
                    require_activation_readiness_v1(complete_readiness()),
                    Err(ZkX509ProfileErrorV1::EngineIncomplete)
                );
            }
            SourceReleasePinStateV1::FullyPinned => {
                assert_ne!(ZK_X509_RESOURCE_CERTIFICATE_SHA256_V1, [0; 32]);
                assert!(readiness.resource_benchmarks);
                assert!(readiness.is_complete());
                assert_eq!(require_activation_readiness_v1(readiness), Ok(()));
                assert_eq!(
                    require_activation_readiness_v1(complete_readiness()),
                    Ok(())
                );
            }
            SourceReleasePinStateV1::Mixed => {
                panic!("source release pins must be wholly open or wholly installed")
            }
        }
    }
    #[test]
    fn release_evidence_pin_state_machine_is_fail_closed() {
        let kat = [0x11; 32];
        let norito = [0x22; 32];
        let json = [0x33; 32];
        let source_pins = source_release_capture_pins_v1();
        let readiness = zk_x509_activation_readiness_v1();
        assert!(readiness.soundness_analysis);
        match source_release_pin_state_v1(source_pins) {
            SourceReleasePinStateV1::BootstrapOpen => {
                assert!(!zk_x509_release_evidence_pins_complete_v1());
                assert!(zk_x509_native_release_expectation_capture_open_v1());
                assert!(!zk_x509_native_release_expectation_digests_match_v1(
                    norito, json
                ));
                assert!(!readiness.known_answer_tests);
                assert!(!readiness.adversarial_tests);
                assert!(!readiness.resource_benchmarks);
                assert!(!readiness.is_complete());
            }
            SourceReleasePinStateV1::FullyPinned => {
                assert!(zk_x509_release_evidence_pins_complete_v1());
                assert!(!zk_x509_native_release_expectation_capture_open_v1());
                assert!(zk_x509_native_release_expectation_digests_match_v1(
                    source_pins.expectations_norito_sha256,
                    source_pins.expectations_json_sha256,
                ));
                assert!(readiness.known_answer_tests);
                assert!(readiness.adversarial_tests);
                assert!(readiness.resource_benchmarks);
                assert!(readiness.is_complete());
            }
            SourceReleasePinStateV1::Mixed => {
                panic!("source release pins must be wholly open or wholly installed")
            }
        }
        assert!(release_evidence_pins_are_complete_v1(1, kat, norito, json));
        for (proof_bytes, proof_digest, norito_digest, json_digest) in [
            (0, kat, norito, json),
            (ZK_X509_MAXIMUM_ENCODED_X5S1_BYTES_V1 + 1, kat, norito, json),
            (1, [0; 32], norito, json),
            (1, kat, [0; 32], json),
            (1, kat, norito, [0; 32]),
            (1, kat, norito, norito),
        ] {
            assert!(!release_evidence_pins_are_complete_v1(
                proof_bytes,
                proof_digest,
                norito_digest,
                json_digest,
            ));
        }
        let empty_capture_pins = ZkX509ReleaseCapturePinsV1 {
            kat_proof_bytes: 0,
            kat_proof_sha256: [0; 32],
            expectations_norito_sha256: [0; 32],
            expectations_json_sha256: [0; 32],
            resource_certificate_sha256: [0; 32],
            positive_elapsed_millis: 0,
            positive_peak_rss_bytes: 0,
            positive_peak_address_space_bytes: 0,
            maximum_elapsed_millis: 0,
            maximum_peak_rss_bytes: 0,
            maximum_peak_address_space_bytes: 0,
        };
        assert!(native_release_capture_open_with_pins_v1(empty_capture_pins));
        assert_eq!(
            source_release_pin_state_v1(empty_capture_pins),
            SourceReleasePinStateV1::BootstrapOpen
        );
        for partial in [
            ZkX509ReleaseCapturePinsV1 {
                kat_proof_bytes: 1,
                ..empty_capture_pins
            },
            ZkX509ReleaseCapturePinsV1 {
                kat_proof_sha256: kat,
                ..empty_capture_pins
            },
            ZkX509ReleaseCapturePinsV1 {
                expectations_norito_sha256: norito,
                ..empty_capture_pins
            },
            ZkX509ReleaseCapturePinsV1 {
                expectations_json_sha256: json,
                ..empty_capture_pins
            },
            ZkX509ReleaseCapturePinsV1 {
                resource_certificate_sha256: kat,
                ..empty_capture_pins
            },
            ZkX509ReleaseCapturePinsV1 {
                positive_elapsed_millis: 1,
                ..empty_capture_pins
            },
            ZkX509ReleaseCapturePinsV1 {
                positive_peak_rss_bytes: 1,
                ..empty_capture_pins
            },
            ZkX509ReleaseCapturePinsV1 {
                positive_peak_address_space_bytes: 1,
                ..empty_capture_pins
            },
            ZkX509ReleaseCapturePinsV1 {
                maximum_elapsed_millis: 1,
                ..empty_capture_pins
            },
            ZkX509ReleaseCapturePinsV1 {
                maximum_peak_rss_bytes: 1,
                ..empty_capture_pins
            },
            ZkX509ReleaseCapturePinsV1 {
                maximum_peak_address_space_bytes: 1,
                ..empty_capture_pins
            },
        ] {
            assert!(!native_release_capture_open_with_pins_v1(partial));
            assert!(!native_release_capture_pins_complete_v1(partial));
            assert_eq!(
                source_release_pin_state_v1(partial),
                SourceReleasePinStateV1::Mixed,
                "every one-pin partial source state must be rejected as mixed"
            );
        }
        let populated_capture_pins = ZkX509ReleaseCapturePinsV1 {
            kat_proof_bytes: 1,
            kat_proof_sha256: kat,
            expectations_norito_sha256: norito,
            expectations_json_sha256: json,
            resource_certificate_sha256: [0x44; 32],
            positive_elapsed_millis: 1,
            positive_peak_rss_bytes: 1,
            positive_peak_address_space_bytes: 1,
            maximum_elapsed_millis: 1,
            maximum_peak_rss_bytes: 1,
            maximum_peak_address_space_bytes: 1,
        };
        assert_eq!(
            source_release_pin_state_v1(populated_capture_pins),
            SourceReleasePinStateV1::FullyPinned
        );
        assert!(native_release_capture_pins_complete_v1(
            populated_capture_pins
        ));
        assert!(native_release_expectation_digests_match_with_pins_v1(
            norito, json, norito, json
        ));
        for (expected_norito, expected_json, actual_norito, actual_json) in [
            ([0; 32], json, norito, json),
            (norito, [0; 32], norito, json),
            (norito, json, json, norito),
            (norito, json, kat, json),
            (norito, json, norito, kat),
        ] {
            assert!(!native_release_expectation_digests_match_with_pins_v1(
                expected_norito,
                expected_json,
                actual_norito,
                actual_json,
            ));
        }
    }
    #[test]
    fn main_and_ca_fri_theorem_substitutions_fail_closed() {
        for (native_log2, blowup_log2, terminal_log2, terminal_degree, fold_count, chunks) in
            [(19, 6, 10, 31, 15, 4), (7, 7, 9, 15, 5, 3)]
        {
            let parameters = fri_parameters_v1(
                native_log2,
                blowup_log2,
                terminal_log2,
                terminal_degree,
                chunks,
            );
            let layout = AggregateProofLayoutV1::new(
                parameters,
                vec![AggregateTraceGroupLayoutV1 {
                    native_trace_log2: native_log2,
                    segment_instances: 1,
                    base_width: 1,
                    aux_width: 1,
                }],
            )
            .expect("subproof layout");
            let certificate = fri_theorem_certificate_v1(
                native_log2 + blowup_log2,
                fold_count,
                terminal_log2,
                u16::try_from(terminal_degree).expect("small terminal degree"),
            );
            validate_affine_batched_fri_theorem2_v1(parameters, &layout, certificate)
                .expect("canonical theorem certificate");
            for mutation in [
                AggregateFriTheorem2CertificateV1 {
                    rho_denominator: 31,
                    ..certificate
                },
                AggregateFriTheorem2CertificateV1 {
                    batching_parameter_m: 2,
                    ..certificate
                },
                AggregateFriTheorem2CertificateV1 {
                    terminal_degree_bound: certificate.terminal_degree_bound - 1,
                    ..certificate
                },
                AggregateFriTheorem2CertificateV1 {
                    domain_log2: certificate.domain_log2 - 1,
                    ..certificate
                },
                AggregateFriTheorem2CertificateV1 {
                    l_minus_one_numerator: 2,
                    ..certificate
                },
                AggregateFriTheorem2CertificateV1 {
                    extension_field_lower_bound_bits: 251,
                    ..certificate
                },
                AggregateFriTheorem2CertificateV1 {
                    trace_domains_are_smooth_subgroups: false,
                    ..certificate
                },
                AggregateFriTheorem2CertificateV1 {
                    evaluation_domain_is_smooth_generator_coset: false,
                    ..certificate
                },
                AggregateFriTheorem2CertificateV1 {
                    evaluation_domain_is_disjoint_from_trace_domains: false,
                    ..certificate
                },
                AggregateFriTheorem2CertificateV1 {
                    affine_arities: [2, 2, 1],
                    ..certificate
                },
                AggregateFriTheorem2CertificateV1 {
                    query_count: 57,
                    ..certificate
                },
                AggregateFriTheorem2CertificateV1 {
                    distinct_queries_without_replacement: false,
                    ..certificate
                },
                AggregateFriTheorem2CertificateV1 {
                    uniform_rejection_sampling: false,
                    ..certificate
                },
            ] {
                assert!(
                    validate_affine_batched_fri_theorem2_v1(parameters, &layout, mutation).is_err(),
                    "main and compact-CA theorem substitutions must reject"
                );
            }
        }
    }
    #[test]
    fn independent_readiness_requirements_fail_closed() {
        let canonical = zk_x509_activation_readiness_v1();
        let expected = match source_release_pin_state_v1(source_release_capture_pins_v1()) {
            SourceReleasePinStateV1::BootstrapOpen => {
                assert!(!canonical.is_complete());
                Err(ZkX509ProfileErrorV1::EngineIncomplete)
            }
            SourceReleasePinStateV1::FullyPinned => {
                assert!(canonical.is_complete());
                Ok(())
            }
            SourceReleasePinStateV1::Mixed => {
                panic!("source release pins must be wholly open or wholly installed")
            }
        };
        assert_eq!(require_activation_readiness_v1(canonical), expected);
        assert_eq!(
            require_activation_readiness_v1(complete_readiness()),
            expected,
            "a caller-supplied all-true checklist must equal derived readiness"
        );
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
    #[test]
    fn retired_sparse_crl_profile_cannot_reenter_the_release_manifest() {
        for retired in [
            ["crl", "_nonmembership"].concat(),
            ["crl", "-complete-sparse"].concat(),
            ["crl", "_complete_sparse"].concat(),
            ["crl", "_sparse", "_root"].concat(),
        ] {
            assert!(
                !String::from_utf8_lossy(ZK_X509_STARK_PROFILE_DESCRIPTOR_V1).contains(&retired),
                "retired sparse-CRL symbol {retired} must stay outside the profile descriptor"
            );
        }
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
