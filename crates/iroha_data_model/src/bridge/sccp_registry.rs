//! Exact first-release SCCP route-registry wire types and commitments.
//!
//! A governed route is one atomic consensus object. It contains only closed, typed protocol
//! identity. Operator checklists, URLs, RPC observations, executable blobs, prover packages, and
//! deployment logs are deliberately not consensus state.
use super::{
    BridgeNativeProofBackendV1, SCCP_TON_MAINNET_GLOBAL_ID_V1,
    SCCP_TON_MAINNET_ZERO_STATE_FILE_HASH_V1, SCCP_TON_MAINNET_ZERO_STATE_ROOT_HASH_V1,
    SCCP_TON_MASTERCHAIN_SHARD_V1, SCCP_TON_MASTERCHAIN_WORKCHAIN_V1, SCCP_TON_ZERO_STATE_SEQNO_V1,
    SccpEvmSourceEmitterV1, SccpLaneIdV1, SccpNativeTrustAnchorV1, SccpNetworkV1,
    SccpSourceEmitterV1, SccpSourceIdentityV1, SccpTonAddressV1, SccpTonSourceEmitterV1,
    SccpTronSourceEmitterV1,
};

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    NetworkId, account::AccountId, asset::AssetDefinitionId, block::consensus_v2::PROTOCOL_VERSION,
};
use blake2::{Blake2b, Digest as _, digest::consts::U32};
use iroha_crypto::{derive_non_signing_ed25519_public_key, keccak256};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::Sha256;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
/// Sole authoritative Sumeragi wire revision accepted by SCCP V1 anchors.
pub const SCCP_V1_SUMERAGI_PROTOCOL_VERSION: u16 = PROTOCOL_VERSION;
/// Maximum decimal scale accepted by first-release SCCP amount payloads.
pub const SCCP_V1_MAX_PAYLOAD_AMOUNT_SCALE: u32 = 28;
/// Exact decimal scale of the first-release XOR SCCP payload.
pub const SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE: u32 = 9;
/// Exact byte length of one canonical epoch-aware SORA finality anchor.
pub const SCCP_V1_SORA_FINALITY_ANCHOR_BYTES: usize = 188;
/// Maximum number of nonterminal routes in the V1 registry.
///
/// Terminal revisions are immutable history needed to authenticate messages emitted before a
/// deployment rotation. They deliberately do not consume this live-governance budget; a separate
/// generous retained-history bound keeps the full state and registry response finite.
pub const SCCP_V1_MAX_LIVE_GOVERNED_ROUTES: usize = 64;
/// Maximum number of exact lanes in the V1 registry.
pub const SCCP_V1_MAX_GOVERNED_LANES: usize = 16;
/// Maximum number of nonterminal routes sharing one governed lane.
///
/// A governance action can append only one staged revision. Retired revisions
/// remain queryable but are excluded from this mutable-state bound.
pub const SCCP_V1_MAX_LIVE_ROUTES_PER_LANE: usize = 8;
/// Maximum retained route revisions sharing one governed lane.
///
/// For a single-lineage lane, sixty-four revisions provide more than five years of monthly
/// deployment rotation. History is never evicted implicitly; governance must stop before this
/// shared lane bound and operators must plan an explicit first-release migration. A fixed-shape
/// admitted V1 route fits a conservative 4 KiB canonical encoding envelope.
pub const SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE: usize = 64;
/// Maximum retained native trust anchors sharing one governed lane.
///
/// At one governed rotation per day, 4,096 checkpoints cover more than eleven years. A checkpoint
/// fits a conservative 64-byte canonical encoding envelope; together with the route and 16-lane
/// caps, retained entry payloads are bounded by 8 MiB before small vector/lane framing overhead.
pub const SCCP_V1_MAX_RETAINED_NATIVE_TRUST_ANCHORS_PER_LANE: usize = 4_096;
/// Maximum byte length of a canonical SCCP route or asset key.
pub const SCCP_V1_MAX_KEY_BYTES: usize = 64;
/// Exact Taira(9-decimal) to wrapped-token(18-decimal) multiplier.
pub const SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER: u64 = 1_000_000_000;
/// Exact Taira(9-decimal) to TON Jetton(9-decimal) base-unit multiplier.
pub const SCCP_V1_TAIRA_TO_TON_TOKEN_MULTIPLIER: u64 = 1;
/// Greatest amount representable by the TON `coins` TL-B encoding.
pub const SCCP_V1_TON_MAX_COINS: u128 = (1_u128 << 120) - 1;
/// Exact storage-layout version required by the first-release TON contracts.
pub const SCCP_V1_TON_STORAGE_VERSION: u8 = 1;
/// Exact first-release SORA-side IVM semantics selected by route governance.
pub const SCCP_V1_SORA_OUTBOUND_EXECUTION_SEMANTICS: &str = "ivm_proved_record_sccp_message_v1";
/// Fixed upper bound for one governed SORA-side outbound IVM execution.
pub const SCCP_V1_MAX_SORA_OUTBOUND_GAS_LIMIT: u64 = 1_000_000_000;
/// Canonical live Taira XOR asset definition governed by every V1 route.
pub const SCCP_V1_TAIRA_XOR_ASSET_DEFINITION_ID: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
const SCCP_DOMAIN_SORA: u32 = 0;
const SCCP_DOMAIN_ETH: u32 = 1;
const SCCP_DOMAIN_BSC: u32 = 2;
const SCCP_DOMAIN_TRON: u32 = 5;
const SCCP_DOMAIN_TON: u32 = 4;
const EVM_BINDING_DOMAIN_V1: &[u8] = b"iroha:sccp:evm-destination-binding:v1";
const TRON_BINDING_DOMAIN_V1: &[u8] = b"iroha:sccp:tron-destination-binding:v1";
const TON_BINDING_DOMAIN_V1: &[u8] = b"iroha:sccp:ton-destination-binding:v1";
const CONCRETE_ROUTE_CONFIG_DOMAIN_V1: &[u8] = b"sccp:concrete-route-config:v1";
const NETWORK_HASH_DOMAIN_V1: &[u8] = b"sccp:network-identity:v1";
const LANE_HASH_DOMAIN_V1: &[u8] = b"sccp:lane-id:v1";
const ROUTE_ESCROW_ACCOUNT_DOMAIN_V1: &[u8] = b"iroha:sccp:route-escrow-account:v1";
const SOURCE_EMITTER_HASH_DOMAIN_V1: &[u8] = b"sccp:source-emitter-identity:v1";
const SOURCE_IDENTITY_HASH_DOMAIN_V1: &[u8] = b"sccp:source-identity:v1";
const SEMANTIC_PROOF_PROFILE_HASH_DOMAIN_V1: &[u8] = b"sccp:semantic-proof-profile:v1";
const SORA_FINALITY_ANCHOR_HASH_DOMAIN_V1: &[u8] = b"sccp:sora-finality-anchor:v1";
const GROTH16_PUBLIC_SIGNAL_SCHEMA_HASH_DOMAIN_V1: &[u8] =
    b"sccp:groth16-bn254:public-signal-schema:v1";
const GROTH16_BLS12381_PUBLIC_SIGNAL_SCHEMA_HASH_DOMAIN_V1: &[u8] =
    b"sccp:groth16-bls12381:public-signal-schema:v1";
const EVM_GROTH16_BACKEND_V1: &[u8] = b"evm-groth16-bn254-v1";
const TRON_GROTH16_BACKEND_V1: &[u8] = b"tron-groth16-bn254-v1";
const TON_GROTH16_BLS12381_BACKEND_V1: &[u8] = b"ton-groth16-bls12381-v1";
const TON_GROTH16_BLS12381_PROOF_PROFILE_PREFIX_V1: &[u8] =
    b"sccp:ton:groth16-bls12381:proof-profile:v1";
const GROTH16_BLS12381_SCALAR_FIELD_MODULUS_BE: [u8; 32] = [
    0x73, 0xed, 0xa7, 0x53, 0x29, 0x9d, 0x7d, 0x48, 0x33, 0x39, 0xd8, 0x08, 0x09, 0xa1, 0xd8, 0x05,
    0x53, 0xbd, 0xa4, 0x02, 0xff, 0xfe, 0x5b, 0xfe, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x01,
];
const GROTH16_PUBLIC_SIGNAL_LABELS_V1: [&[u8]; 11] = [
    b"sccp:groth16-bn254:signal:message-id:v1",
    b"sccp:groth16-bn254:signal:payload-hash:v1",
    b"sccp:groth16-bn254:signal:target-domain:v1",
    b"sccp:groth16-bn254:signal:commitment-root:v1",
    b"sccp:groth16-bn254:signal:finality-height:v1",
    b"sccp:groth16-bn254:signal:finality-block-hash:v1",
    b"sccp:groth16-bn254:signal:source-domain:v1",
    b"sccp:groth16-bn254:signal:statement-hash:v1",
    b"sccp:groth16-bn254:signal:destination-binding-hash:v1",
    b"sccp:groth16-bn254:signal:route-configuration-hash:v1",
    b"sccp:groth16-bn254:signal:sora-finality-anchor-hash:v1",
];
const GROTH16_BLS12381_PUBLIC_SIGNAL_LABELS_V1: [&[u8]; 11] = [
    b"sccp:groth16-bls12381:signal:message-id:v1",
    b"sccp:groth16-bls12381:signal:payload-hash:v1",
    b"sccp:groth16-bls12381:signal:target-domain:v1",
    b"sccp:groth16-bls12381:signal:commitment-root:v1",
    b"sccp:groth16-bls12381:signal:finality-height:v1",
    b"sccp:groth16-bls12381:signal:finality-block-hash:v1",
    b"sccp:groth16-bls12381:signal:source-domain:v1",
    b"sccp:groth16-bls12381:signal:statement-hash:v1",
    b"sccp:groth16-bls12381:signal:destination-binding-hash:v1",
    b"sccp:groth16-bls12381:signal:route-config-hash:v1",
    b"sccp:groth16-bls12381:signal:sora-finality-anchor-hash:v1",
];
/// BN254 base-field modulus in canonical big-endian form.
const BN254_BASE_FIELD_MODULUS_BE: [u8; 32] = [
    0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
    0x97, 0x81, 0x6a, 0x91, 0x68, 0x71, 0xca, 0x8d, 0x3c, 0x20, 0x8c, 0x16, 0xd8, 0x7c, 0xfd, 0x47,
];
/// BLS12-381 base-field modulus in canonical big-endian form.
const BLS12381_BASE_FIELD_MODULUS_BE: [u8; 48] = [
    0x1a, 0x01, 0x11, 0xea, 0x39, 0x7f, 0xe6, 0x9a, 0x4b, 0x1b, 0xa7, 0xb6, 0x43, 0x4b, 0xac, 0xd7,
    0x64, 0x77, 0x4b, 0x84, 0xf3, 0x85, 0x12, 0xbf, 0x67, 0x30, 0xd2, 0xa0, 0xf6, 0xb0, 0xf6, 0x24,
    0x1e, 0xab, 0xff, 0xfe, 0xb1, 0x53, 0xff, 0xff, 0xb9, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xaa, 0xab,
];
const SORA_TAIRA_CHAIN_ID_BYTES: [u8; 16] = [
    0xfc, 0x56, 0x98, 0x4b, 0x2b, 0xe7, 0x43, 0x1d, 0x84, 0x0e, 0x21, 0x51, 0x4d, 0x18, 0x83, 0xf0,
];
const KECCAK256_EMPTY_BYTES: [u8; 32] = [
    0xc5, 0xd2, 0x46, 0x01, 0x86, 0xf7, 0x23, 0x3c, 0x92, 0x7e, 0x7d, 0xb2, 0xdc, 0xc7, 0x03, 0xc0,
    0xe5, 0x00, 0xb6, 0x53, 0xca, 0x82, 0x27, 0x3b, 0x7b, 0xfa, 0xd8, 0x04, 0x5d, 0x85, 0xa4, 0x70,
];
/// Validation failure for a closed SCCP route or registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SccpRouteValidationError {
    /// The registry version is not V1.
    #[error("SCCP registry version must be exactly 1")]
    UnsupportedRegistryVersion,
    /// The registry exceeds its deterministic live-route consensus bound.
    #[error(
        "SCCP registry contains more than {SCCP_V1_MAX_LIVE_GOVERNED_ROUTES} nonterminal routes"
    )]
    TooManyLiveRoutes,
    /// The registry exceeds its deterministic lane bound.
    #[error("SCCP registry contains more than {SCCP_V1_MAX_GOVERNED_LANES} lanes")]
    TooManyLanes,
    /// One lane is empty or exceeds its deterministic live-route bound.
    #[error(
        "SCCP lane contains no routes or more than {SCCP_V1_MAX_LIVE_ROUTES_PER_LANE} nonterminal routes"
    )]
    InvalidLaneLiveRouteCount,
    /// One lane exceeds its deterministic retained-route bound.
    #[error(
        "SCCP lane contains more than {SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE} retained route revisions"
    )]
    TooManyRetainedRoutes,
    /// One lane exceeds its deterministic retained-anchor bound.
    #[error(
        "SCCP lane contains more than {SCCP_V1_MAX_RETAINED_NATIVE_TRUST_ANCHORS_PER_LANE} retained native trust anchors"
    )]
    TooManyRetainedTrustAnchors,
    /// A route or asset identifier is not canonical.
    #[error("SCCP {0} must be lowercase ASCII [a-z0-9_-], with alphanumeric ends")]
    NonCanonicalKey(&'static str),
    /// The directed lane is not an exact external-to-SORA lane.
    #[error("SCCP governed route lane must be external-to-SORA")]
    InvalidInboundLane,
    /// The exact first-release route contracts target Taira.
    #[error("SCCP V1 exact route deployment must target SORA Taira")]
    UnsupportedSoraEndpoint,
    /// The destination variant and external network family differ.
    #[error("SCCP destination deployment family does not match the lane")]
    DestinationFamilyMismatch,
    /// A required typed identity or commitment is zero.
    #[error("SCCP deployment role `{0}` must be nonzero")]
    ZeroRole(&'static str),
    /// A governed EVM-family runtime commitment names the empty bytecode.
    #[error("SCCP deployment runtime role `{0}` must not be empty bytecode")]
    EmptyRuntimeCode(&'static str),
    /// A TON contract address is zero or is not in the governed basechain workchain.
    #[error("SCCP TON contract address must be a nonzero basechain raw address")]
    InvalidTonAddress,
    /// TON mint-breaker guardians are not five nonzero strictly ordered Ed25519 keys.
    #[error("SCCP TON mint-breaker guardian keys must be nonzero and strictly ordered")]
    InvalidTonMintBreakerGuardians,
    /// A TON Jetton supply cap is outside the canonical `coins` domain.
    #[error("SCCP TON maximum wrapped supply must fit the nonzero 120-bit coins domain")]
    InvalidTonWrappedSupplyCap,
    /// TON verifier circuit or proof-profile commitments differ from the governed policy.
    #[error("SCCP TON verifier commitments do not match the governed proof policy")]
    InvalidTonProofCommitments,
    /// A fixed Groth16 key has the wrong version or a non-canonical coordinate.
    #[error("SCCP Groth16 BN254 verification key is not structurally canonical")]
    InvalidGroth16VerifyingKey,
    /// The full governed key does not match the Solidity verifier commitment.
    #[error("SCCP Groth16 verification-key hash does not match the embedded key")]
    Groth16VerifyingKeyHashMismatch,
    /// The governed semantic circuit profile is absent, malformed, or uses another schema.
    #[error("SCCP semantic proof profile is not the exact audited V1 shape")]
    InvalidSemanticProofProfile,
    /// The governed SORA checkpoint is absent, malformed, or belongs to another chain.
    #[error("SCCP SORA finality anchor is not the exact Taira V1 shape")]
    InvalidSoraFinalityAnchor,
    /// The destination proof policy is not V1 or its typed commitments are invalid.
    #[error("SCCP outbound proof policy must be exactly version 1")]
    InvalidOutboundProofPolicy,
    /// The SORA-side outbound execution policy is malformed or unsupported.
    #[error("SCCP SORA outbound execution policy must be the exact bounded V1 policy")]
    InvalidSoraOutboundExecutionPolicy,
    /// Two distinct protocol roles use the same identity or commitment.
    #[error("SCCP deployment identities and hash roles must be pairwise distinct")]
    RoleAlias,
    /// Route, asset, or scale differs from the exact first-release contract.
    #[error("SCCP payload identity does not match the exact first-release route deployment")]
    ConcreteRouteMismatch,
    /// Immutable route revisions start at one and advance without gaps.
    #[error("SCCP route revision must be a nonzero monotonic successor")]
    InvalidRouteRevision,
    /// The source emitter is not the source side of the same exact deployment.
    #[error("SCCP source identity does not match the governed destination deployment")]
    SourceDestinationMismatch,
    /// The selected native backend belongs to another source-chain family.
    #[error("SCCP native trust-anchor backend does not match the source network")]
    TrustAnchorFamilyMismatch,
    /// The trust-anchor commitment is zero.
    #[error("SCCP native trust anchor must be nonzero")]
    InvalidTrustAnchor,
    /// Historical lane anchors are not a unique, append-only checkpoint chain.
    #[error(
        "SCCP native trust-anchor history must use one backend and unique, strictly increasing checkpoints"
    )]
    InvalidTrustAnchorHistory,
    /// The current anchor pointer does not select the last historical anchor.
    #[error("SCCP current native trust-anchor hash must select the highest retained checkpoint")]
    InvalidCurrentTrustAnchor,
    /// Anchor compare-and-swap does not preserve the backend or change the commitment.
    #[error("SCCP lane trust-anchor update must keep its backend and change its hash")]
    InvalidTrustAnchorAdvance,
    /// Anchor initialization was not an exact `None` to valid checkpoint transition.
    #[error(
        "SCCP lane trust-anchor initialization must compare None and install one valid checkpoint"
    )]
    InvalidTrustAnchorInitialize,
    /// Inbound activation is unsupported or incomplete for this route.
    #[error("SCCP route cannot enable native inbound settlement")]
    UnsupportedInboundActivation,
    /// A governed settlement uses a scale other than exact Taira XOR precision.
    #[error("SCCP V1 settlement amount scale must be exactly 9")]
    InvalidSettlementScale,
    /// Settlement names a SORA asset other than canonical live Taira XOR.
    #[error("SCCP V1 settlement asset must be canonical live Taira XOR")]
    SettlementAssetMismatch,
    /// Route liability and destination supply ceilings are zero, overflow, or disagree.
    #[error("SCCP route requires matching positive immutable liability and wrapped-supply caps")]
    InvalidSupplyCap,
    /// Registration attempted to bypass staged review.
    #[error("new SCCP routes must be registered in staged state")]
    RegistrationMustBeStaged,
    /// A route activation update is a stale, no-op, or illegal lifecycle transition.
    #[error("SCCP route activation transition is not allowed")]
    InvalidActivationTransition,
    /// A route key occurs more than once.
    #[error("SCCP registry contains a duplicate route key")]
    DuplicateRouteKey,
    /// A lane occurs more than once.
    #[error("SCCP registry contains a duplicate lane")]
    DuplicateLane,
    /// A destination binding occurs more than once.
    #[error("SCCP registry contains a reused destination binding")]
    DuplicateDestinationBinding,
    /// An immutable route configuration occurs more than once.
    #[error("SCCP registry contains a reused route-configuration commitment")]
    DuplicateRouteConfiguration,
    /// Two retained TRON revisions in one lane reuse an immutable route address.
    #[error("SCCP lane contains a reused TRON source route address")]
    DuplicateTronSourceAddress,
    /// Two retained TON revisions in one lane reuse an immutable source bridge address.
    #[error("SCCP lane contains a reused TON source bridge address")]
    DuplicateTonSourceAddress,
    /// More than one immutable revision of one semantic route is enabled.
    #[error("SCCP registry enables multiple revisions of one semantic route and asset")]
    MultipleEnabledRevisions,
    /// Terminal inbound admission lacks an exact governed anchor-interval cutoff.
    #[error(
        "SCCP retired route must carry one valid anchor-interval cutoff tied to retained anchor history"
    )]
    InvalidInboundFinalityCutoff,
}
/// Canonical non-infinity BN254 G1 point in Solidity ABI coordinate order.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpBn254G1PointV1")]
pub struct SccpBn254G1PointV1 {
    /// Canonical big-endian base-field x coordinate.
    pub x: [u8; 32],
    /// Canonical big-endian base-field y coordinate.
    pub y: [u8; 32],
}
impl SccpBn254G1PointV1 {
    /// Return whether both coordinates are canonical field elements and the
    /// point is not the conventional all-zero point-at-infinity encoding.
    #[must_use]
    pub fn is_structurally_canonical(self) -> bool {
        (self.x != [0; 32] || self.y != [0; 32])
            && self.x < BN254_BASE_FIELD_MODULUS_BE
            && self.y < BN254_BASE_FIELD_MODULUS_BE
    }
}
/// Canonical non-infinity BN254 G2 point in Solidity verifier limb order.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpBn254G2PointV1")]
pub struct SccpBn254G2PointV1 {
    /// First x-coordinate Fq2 limb (`x[0]` in the Solidity verifier).
    pub x_c0: [u8; 32],
    /// Second x-coordinate Fq2 limb (`x[1]` in the Solidity verifier).
    pub x_c1: [u8; 32],
    /// First y-coordinate Fq2 limb (`y[0]` in the Solidity verifier).
    pub y_c0: [u8; 32],
    /// Second y-coordinate Fq2 limb (`y[1]` in the Solidity verifier).
    pub y_c1: [u8; 32],
}
impl SccpBn254G2PointV1 {
    /// Return whether every limb is a canonical field element and the point is
    /// not the conventional all-zero point-at-infinity encoding.
    #[must_use]
    pub fn is_structurally_canonical(self) -> bool {
        let limbs = [self.x_c0, self.x_c1, self.y_c0, self.y_c1];
        limbs.iter().any(|limb| *limb != [0; 32])
            && limbs.iter().all(|limb| *limb < BN254_BASE_FIELD_MODULUS_BE)
    }
}
/// Fixed Groth16 IC vector: one constant point and exactly eleven signal points.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpGroth16Bn254IcV1")]
pub struct SccpGroth16Bn254IcV1 {
    /// Constant IC point.
    pub constant: SccpBn254G1PointV1,
    /// IC point for signal 0 (message id).
    pub signal_0: SccpBn254G1PointV1,
    /// IC point for signal 1 (payload hash).
    pub signal_1: SccpBn254G1PointV1,
    /// IC point for signal 2 (target domain).
    pub signal_2: SccpBn254G1PointV1,
    /// IC point for signal 3 (commitment root).
    pub signal_3: SccpBn254G1PointV1,
    /// IC point for signal 4 (finality height).
    pub signal_4: SccpBn254G1PointV1,
    /// IC point for signal 5 (finality block hash).
    pub signal_5: SccpBn254G1PointV1,
    /// IC point for signal 6 (source domain).
    pub signal_6: SccpBn254G1PointV1,
    /// IC point for signal 7 (statement hash).
    pub signal_7: SccpBn254G1PointV1,
    /// IC point for signal 8 (destination binding hash).
    pub signal_8: SccpBn254G1PointV1,
    /// IC point for signal 9 (governed route-configuration hash).
    pub signal_9: SccpBn254G1PointV1,
    /// IC point for signal 10 (governed SORA finality-anchor hash).
    pub signal_10: SccpBn254G1PointV1,
}
impl SccpGroth16Bn254IcV1 {
    /// Return the constant point followed by the eleven public-signal points.
    #[must_use]
    pub const fn points(self) -> [SccpBn254G1PointV1; 12] {
        [
            self.constant,
            self.signal_0,
            self.signal_1,
            self.signal_2,
            self.signal_3,
            self.signal_4,
            self.signal_5,
            self.signal_6,
            self.signal_7,
            self.signal_8,
            self.signal_9,
            self.signal_10,
        ]
    }
}
/// Closed SCCP BN254 Groth16 verification key for exactly eleven public signals.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpGroth16Bn254VerifyingKeyV1")]
pub struct SccpGroth16Bn254VerifyingKeyV1 {
    /// Verifying-key schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Groth16 alpha point in G1.
    pub alpha1: SccpBn254G1PointV1,
    /// Groth16 beta point in G2.
    pub beta2: SccpBn254G2PointV1,
    /// Groth16 gamma point in G2.
    pub gamma2: SccpBn254G2PointV1,
    /// Groth16 delta point in G2.
    pub delta2: SccpBn254G2PointV1,
    /// Constant IC point followed by exactly eleven public-signal IC points.
    pub ic: SccpGroth16Bn254IcV1,
}
impl SccpGroth16Bn254VerifyingKeyV1 {
    /// Validate the closed shape and canonical field encoding.
    ///
    /// Curve, non-infinity, and subgroup membership are deliberately verified
    /// by the cryptographic SCCP implementation during route registration.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError::InvalidGroth16VerifyingKey`] when the
    /// version, point encodings, or fixed IC layout are not canonical.
    pub fn validate_structure(self) -> Result<(), SccpRouteValidationError> {
        if self.version != 1
            || !self.alpha1.is_structurally_canonical()
            || !self.beta2.is_structurally_canonical()
            || !self.gamma2.is_structurally_canonical()
            || !self.delta2.is_structurally_canonical()
            || !self
                .ic
                .points()
                .iter()
                .all(|point| point.is_structurally_canonical())
        {
            return Err(SccpRouteValidationError::InvalidGroth16VerifyingKey);
        }
        Ok(())
    }
    /// Return whether the closed shape and every field encoding are canonical.
    #[must_use]
    pub fn is_structurally_canonical(self) -> bool {
        self.validate_structure().is_ok()
    }
}
/// Encode a structurally canonical key byte-identically to the fixed Solidity
/// `verifyingKeyHash()` preimage: 38 consecutive ABI words.
///
/// # Errors
///
/// Returns [`SccpRouteValidationError::InvalidGroth16VerifyingKey`] when the
/// verifying key is not structurally canonical.
pub fn canonical_sccp_groth16_bn254_verifying_key_bytes_v1(
    verifying_key: SccpGroth16Bn254VerifyingKeyV1,
) -> Result<Vec<u8>, SccpRouteValidationError> {
    verifying_key.validate_structure()?;
    let mut out = Vec::with_capacity(38 * 32);
    out.extend_from_slice(&verifying_key.alpha1.x);
    out.extend_from_slice(&verifying_key.alpha1.y);
    for point in [
        verifying_key.beta2,
        verifying_key.gamma2,
        verifying_key.delta2,
    ] {
        out.extend_from_slice(&point.x_c0);
        out.extend_from_slice(&point.x_c1);
        out.extend_from_slice(&point.y_c0);
        out.extend_from_slice(&point.y_c1);
    }
    for point in verifying_key.ic.points() {
        out.extend_from_slice(&point.x);
        out.extend_from_slice(&point.y);
    }
    Ok(out)
}
/// Hash a structurally canonical key byte-identically to the fixed Solidity
/// `verifyingKeyHash()` implementation.
///
/// # Errors
///
/// Returns [`SccpRouteValidationError::InvalidGroth16VerifyingKey`] when the
/// verifying key is not structurally canonical.
pub fn sccp_groth16_bn254_verifying_key_hash_v1(
    verifying_key: SccpGroth16Bn254VerifyingKeyV1,
) -> Result<[u8; 32], SccpRouteValidationError> {
    Ok(keccak256(
        canonical_sccp_groth16_bn254_verifying_key_bytes_v1(verifying_key)?,
    ))
}
/// Fixed BLS12-381 IC vector: one constant point and exactly eleven signal
/// points, all in canonical compressed G1 form.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpGroth16Bls12381IcV1")]
pub struct SccpGroth16Bls12381IcV1 {
    /// Constant IC point.
    pub constant: [u8; 48],
    /// IC point for signal 0.
    pub signal_0: [u8; 48],
    /// IC point for signal 1.
    pub signal_1: [u8; 48],
    /// IC point for signal 2.
    pub signal_2: [u8; 48],
    /// IC point for signal 3.
    pub signal_3: [u8; 48],
    /// IC point for signal 4.
    pub signal_4: [u8; 48],
    /// IC point for signal 5.
    pub signal_5: [u8; 48],
    /// IC point for signal 6.
    pub signal_6: [u8; 48],
    /// IC point for signal 7.
    pub signal_7: [u8; 48],
    /// IC point for signal 8.
    pub signal_8: [u8; 48],
    /// IC point for signal 9.
    pub signal_9: [u8; 48],
    /// IC point for signal 10.
    pub signal_10: [u8; 48],
}
impl SccpGroth16Bls12381IcV1 {
    /// Return the constant point followed by the eleven signal points.
    #[must_use]
    pub const fn points(self) -> [[u8; 48]; 12] {
        [
            self.constant,
            self.signal_0,
            self.signal_1,
            self.signal_2,
            self.signal_3,
            self.signal_4,
            self.signal_5,
            self.signal_6,
            self.signal_7,
            self.signal_8,
            self.signal_9,
            self.signal_10,
        ]
    }
}
/// Fixed BLS12-381 Groth16 verification key for exactly eleven public signals.
///
/// Points use the canonical 48-byte G1 and 96-byte G2 compressed encodings
/// consumed by TON's BLS12-381 TVM primitives. The IC array contains one
/// constant point followed by exactly eleven signal points.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::bridge::sccp_registry::SccpGroth16Bls12381VerifyingKeyV1"
)]
pub struct SccpGroth16Bls12381VerifyingKeyV1 {
    /// Verifying-key schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Groth16 alpha point in compressed G1 form.
    pub alpha1: [u8; 48],
    /// Groth16 beta point in compressed G2 form.
    pub beta2: [u8; 96],
    /// Groth16 gamma point in compressed G2 form.
    pub gamma2: [u8; 96],
    /// Groth16 delta point in compressed G2 form.
    pub delta2: [u8; 96],
    /// Constant IC point followed by exactly eleven public-signal IC points.
    pub ic: SccpGroth16Bls12381IcV1,
}
impl SccpGroth16Bls12381VerifyingKeyV1 {
    /// Validate the fixed key shape and canonical compressed-field encodings.
    ///
    /// Full curve and subgroup checks remain mandatory in the cryptographic
    /// SCCP verifier before governance admits a route.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError::InvalidGroth16VerifyingKey`] for an
    /// unsupported version, infinity, uncompressed point, or noncanonical
    /// base-field coordinate.
    pub fn validate_structure(self) -> Result<(), SccpRouteValidationError> {
        if self.version != 1
            || !bls12381_g1_compressed_is_structurally_canonical(&self.alpha1)
            || !bls12381_g2_compressed_is_structurally_canonical(&self.beta2)
            || !bls12381_g2_compressed_is_structurally_canonical(&self.gamma2)
            || !bls12381_g2_compressed_is_structurally_canonical(&self.delta2)
            || !self
                .ic
                .points()
                .iter()
                .all(bls12381_g1_compressed_is_structurally_canonical)
        {
            return Err(SccpRouteValidationError::InvalidGroth16VerifyingKey);
        }
        Ok(())
    }

    /// Return whether the fixed shape and compressed encodings are canonical.
    #[must_use]
    pub fn is_structurally_canonical(self) -> bool {
        self.validate_structure().is_ok()
    }
}
/// Encode a structurally canonical TON verification key in the exact order
/// committed by route governance and the linked verifier.
///
/// # Errors
///
/// Returns [`SccpRouteValidationError::InvalidGroth16VerifyingKey`] when any
/// compressed point encoding is malformed.
pub fn canonical_sccp_groth16_bls12381_verifying_key_bytes_v1(
    verifying_key: SccpGroth16Bls12381VerifyingKeyV1,
) -> Result<Vec<u8>, SccpRouteValidationError> {
    verifying_key.validate_structure()?;
    let mut out = Vec::with_capacity(1 + 48 + 3 * 96 + 12 * 48);
    out.push(verifying_key.version);
    out.extend_from_slice(&verifying_key.alpha1);
    out.extend_from_slice(&verifying_key.beta2);
    out.extend_from_slice(&verifying_key.gamma2);
    out.extend_from_slice(&verifying_key.delta2);
    for point in verifying_key.ic.points() {
        out.extend_from_slice(&point);
    }
    Ok(out)
}
/// Hash the exact canonical TON BLS12-381 Groth16 verification key.
///
/// # Errors
///
/// Returns [`SccpRouteValidationError::InvalidGroth16VerifyingKey`] when the
/// key has a malformed compressed point encoding.
pub fn sccp_groth16_bls12381_verifying_key_hash_v1(
    verifying_key: SccpGroth16Bls12381VerifyingKeyV1,
) -> Result<[u8; 32], SccpRouteValidationError> {
    Ok(sha256_bytes(
        &canonical_sccp_groth16_bls12381_verifying_key_bytes_v1(verifying_key)?,
    ))
}
/// Immutable commitments identifying one audited semantic Groth16 circuit.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::bridge::sccp_registry::SccpGroth16Bn254SemanticCircuitV1"
)]
pub struct SccpGroth16Bn254SemanticCircuitV1 {
    /// Circuit-profile schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Commitment to the exact compiled constraint system and proving key.
    pub circuit_commitment: [u8; 32],
    /// Commitment to the reproducible witness generator and its dependencies.
    pub witness_generator_commitment: [u8; 32],
    /// Commitment to the ordered eleven-signal public-input schema.
    pub public_signal_schema_hash: [u8; 32],
}
/// Immutable commitments identifying the audited TON BLS12-381 Groth16
/// semantic circuit.
///
/// This profile is intentionally distinct from the BN254 circuit. A proof,
/// verification key, field reduction, or public-input schema from one curve
/// cannot be reinterpreted as the other.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::bridge::sccp_registry::SccpGroth16Bls12381SemanticCircuitV1"
)]
pub struct SccpGroth16Bls12381SemanticCircuitV1 {
    /// Circuit-profile schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Commitment to the exact compiled BLS12-381 constraint system and
    /// proving key.
    pub circuit_commitment: [u8; 32],
    /// Commitment to the reproducible witness generator and its dependencies.
    pub witness_generator_commitment: [u8; 32],
    /// Commitment to the ordered BLS12-381 eleven-signal public-input schema.
    pub public_signal_schema_hash: [u8; 32],
}
/// Closed semantic proof profile accepted by first-release outbound routes.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[norito(tag = "profile", content = "commitments")]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpSemanticProofProfileV1")]
pub enum SccpSemanticProofProfileV1 {
    /// Groth16 proof of canonical payload semantics, message inclusion, and
    /// Taira finality rooted in the governed SORA checkpoint.
    #[codec(index = 0)]
    #[norito(rename = "sora_taira_finality_inclusion_groth16_bn254")]
    SoraTairaFinalityInclusionGroth16Bn254(SccpGroth16Bn254SemanticCircuitV1),
    /// TON-native BLS12-381 Groth16 proof of the same SCCP semantic roles,
    /// using a curve-specific signal schema and field reduction.
    #[codec(index = 1)]
    #[norito(rename = "sora_taira_finality_inclusion_groth16_bls12381")]
    SoraTairaFinalityInclusionGroth16Bls12381(SccpGroth16Bls12381SemanticCircuitV1),
}
impl SccpSemanticProofProfileV1 {
    /// Return whether this profile uses the EVM/TRON BN254 destination proof.
    #[must_use]
    pub const fn is_bn254(self) -> bool {
        matches!(self, Self::SoraTairaFinalityInclusionGroth16Bn254(_))
    }

    /// Return whether this profile uses the TON-native BLS12-381 proof.
    #[must_use]
    pub const fn is_bls12381(self) -> bool {
        matches!(self, Self::SoraTairaFinalityInclusionGroth16Bls12381(_))
    }

    /// Validate the closed profile and exact ordered public-signal schema.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError::InvalidSemanticProofProfile`] when
    /// the profile version, signal schema, or commitment roles are invalid.
    pub fn validate(self) -> Result<(), SccpRouteValidationError> {
        let (version, circuit_commitment, witness_generator_commitment, schema, expected_schema) =
            match self {
                Self::SoraTairaFinalityInclusionGroth16Bn254(circuit) => (
                    circuit.version,
                    circuit.circuit_commitment,
                    circuit.witness_generator_commitment,
                    circuit.public_signal_schema_hash,
                    sccp_groth16_bn254_public_signal_schema_hash_v1(),
                ),
                Self::SoraTairaFinalityInclusionGroth16Bls12381(circuit) => (
                    circuit.version,
                    circuit.circuit_commitment,
                    circuit.witness_generator_commitment,
                    circuit.public_signal_schema_hash,
                    sccp_groth16_bls12381_public_signal_schema_hash_v1(),
                ),
            };
        if version != 1
            || schema != expected_schema
            || validate_hash_roles(&[circuit_commitment, witness_generator_commitment, schema])
                .is_err()
        {
            return Err(SccpRouteValidationError::InvalidSemanticProofProfile);
        }
        Ok(())
    }
    /// Return the fixed circuit commitments in protocol-role order.
    #[must_use]
    pub const fn commitments(self) -> [[u8; 32]; 3] {
        match self {
            Self::SoraTairaFinalityInclusionGroth16Bn254(circuit) => [
                circuit.circuit_commitment,
                circuit.witness_generator_commitment,
                circuit.public_signal_schema_hash,
            ],
            Self::SoraTairaFinalityInclusionGroth16Bls12381(circuit) => [
                circuit.circuit_commitment,
                circuit.witness_generator_commitment,
                circuit.public_signal_schema_hash,
            ],
        }
    }
}
/// Immutable Taira checkpoint anchoring one governed outbound proof policy.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpSoraFinalityAnchorV1")]
pub struct SccpSoraFinalityAnchorV1 {
    /// Anchor schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Exact source chain. SCCP V1 outbound proofs require SORA Taira.
    pub source_network: SccpNetworkV1,
    /// Authoritative Sumeragi wire protocol (exactly revision `4`).
    pub protocol_version: u16,
    /// Keccak-256 of the canonical 16-byte Taira chain identifier.
    pub chain_id_hash: [u8; 32],
    /// Nonzero finalized Sumeragi election epoch governing the checkpoint.
    pub epoch: u64,
    /// Last height governed by the checkpoint's frozen election epoch.
    pub epoch_end_height: u64,
    /// Iroha hash of the authenticated ordered validator keys and aligned `PoPs`.
    pub roster_commitment: [u8; 32],
    /// Nonzero finalized checkpoint height.
    pub checkpoint_height: u64,
    /// Hash of the canonical finalized checkpoint block header.
    pub checkpoint_block_hash: [u8; 32],
    /// Immutable Sumeragi-v2 height-context identifier at the checkpoint.
    pub checkpoint_context_id: [u8; 32],
    /// Iroha BLAKE2b-256 hash of the canonical bare-Norito v2 finality artifact.
    pub checkpoint_finality_artifact_hash: [u8; 32],
}
impl SccpSoraFinalityAnchorV1 {
    /// Validate the exact Taira chain identity and consensus checkpoint roles.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError::InvalidSoraFinalityAnchor`] when an
    /// anchor is not the required nonzero Taira Sumeragi-v2 checkpoint.
    pub fn validate(self) -> Result<(), SccpRouteValidationError> {
        if self.version != 1
            || self.source_network != SccpNetworkV1::SoraTaira
            || self.protocol_version != SCCP_V1_SUMERAGI_PROTOCOL_VERSION
            || self.chain_id_hash != sccp_sora_taira_chain_id_hash_v1()
            || self.epoch == 0
            || self.checkpoint_height == 0
            || self.checkpoint_height > self.epoch_end_height
            || validate_hash_roles(&[
                self.chain_id_hash,
                self.roster_commitment,
                self.checkpoint_block_hash,
                self.checkpoint_context_id,
                self.checkpoint_finality_artifact_hash,
            ])
            .is_err()
        {
            return Err(SccpRouteValidationError::InvalidSoraFinalityAnchor);
        }
        Ok(())
    }
}
/// Mandatory immutable proof policy of one value-moving destination deployment.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpOutboundProofPolicyV1")]
pub struct SccpOutboundProofPolicyV1 {
    /// Policy schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Exact audited semantic circuit profile.
    pub semantic_profile: SccpSemanticProofProfileV1,
    /// Exact governed SORA checkpoint exposed as public signal 10.
    pub sora_finality_anchor: SccpSoraFinalityAnchorV1,
}
impl SccpOutboundProofPolicyV1 {
    /// Validate every typed policy role and their domain-separated hashes.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError`] when the policy version, semantic
    /// profile, finality anchor, or commitment-role separation is invalid.
    pub fn validate(self) -> Result<(), SccpRouteValidationError> {
        if self.version != 1 {
            return Err(SccpRouteValidationError::InvalidOutboundProofPolicy);
        }
        self.semantic_profile.validate()?;
        self.sora_finality_anchor.validate()?;
        let mut roles = Vec::from(self.semantic_profile.commitments());
        roles.extend([
            self.sora_finality_anchor.chain_id_hash,
            self.sora_finality_anchor.roster_commitment,
            self.sora_finality_anchor.checkpoint_block_hash,
            self.sora_finality_anchor.checkpoint_context_id,
            self.sora_finality_anchor.checkpoint_finality_artifact_hash,
            self.semantic_profile_hash()?,
            self.sora_finality_anchor_hash()?,
        ]);
        validate_hash_roles(&roles)
            .map_err(|_| SccpRouteValidationError::InvalidOutboundProofPolicy)
    }
    /// Return the domain-separated semantic-profile commitment pinned on-chain.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError`] when the semantic profile is not a
    /// valid SCCP V1 profile.
    pub fn semantic_profile_hash(self) -> Result<[u8; 32], SccpRouteValidationError> {
        sccp_semantic_proof_profile_hash_v1(self.semantic_profile)
    }
    /// Return the domain-separated Taira finality-anchor commitment pinned on-chain.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError`] when the finality anchor is not a
    /// valid SCCP V1 Taira checkpoint.
    pub fn sora_finality_anchor_hash(self) -> Result<[u8; 32], SccpRouteValidationError> {
        sccp_sora_finality_anchor_hash_v1(self.sora_finality_anchor)
    }
}
/// Strict portable reference to one governance-registered IVM verification key.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpPortableVerifyingKeyRefV1")]
pub struct SccpPortableVerifyingKeyRefV1 {
    /// Portable proof-backend registry namespace.
    pub backend: String,
    /// Portable verification-key name within the backend namespace.
    pub name: String,
    /// Exact immutable governance version of the verification key.
    pub version: u32,
    /// Exact domain-separated commitment of the verification-key bytes.
    pub commitment: [u8; 32],
}
impl SccpPortableVerifyingKeyRefV1 {
    /// Return whether both fields use the bounded portable registry grammar.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        crate::proof::verifying_key_id_field_is_portable(&self.backend)
            && crate::proof::verifying_key_id_field_is_portable(&self.name)
            && self.version != 0
            && self.commitment != [0; 32]
    }
    /// Compare this governed reference with the exact verified registry record.
    #[must_use]
    pub fn matches(
        &self,
        other: &crate::proof::VerifyingKeyId,
        version: u32,
        commitment: [u8; 32],
    ) -> bool {
        self.backend == other.backend.as_str()
            && self.name == other.name
            && self.version == version
            && self.commitment == commitment
    }
}
/// Mandatory TAIRA-side execution policy for one SORA-origin SCCP route.
///
/// Contract bytes remain outside consensus state. Governance pins their SHA-256,
/// the portable proof-key id, exact key version and commitment, and the exact
/// transaction gas limit consumed by the separately served route-scoped material.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::bridge::sccp_registry::SccpSoraOutboundExecutionPolicyV1"
)]
pub struct SccpSoraOutboundExecutionPolicyV1 {
    /// Policy schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Fixed burn-and-record execution semantics.
    pub semantics: String,
    /// SHA-256 of the complete canonical IVM contract artifact bytes.
    pub contract_artifact_sha256: [u8; 32],
    /// Exact governance-registered proof verification key.
    pub vk_ref: SccpPortableVerifyingKeyRefV1,
    /// Exact nonzero transaction gas limit used for derive, prove, and submit.
    pub gas_limit: u64,
}
impl SccpSoraOutboundExecutionPolicyV1 {
    /// Validate the exact first-release execution semantics and bounds.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError::InvalidSoraOutboundExecutionPolicy`]
    /// when any policy field is unsupported, empty, zero, or out of bounds.
    pub fn validate(&self) -> Result<(), SccpRouteValidationError> {
        if self.version != 1
            || self.semantics != SCCP_V1_SORA_OUTBOUND_EXECUTION_SEMANTICS
            || self.contract_artifact_sha256 == [0; 32]
            || !self.vk_ref.is_well_formed()
            || self.gas_limit == 0
            || self.gas_limit > SCCP_V1_MAX_SORA_OUTBOUND_GAS_LIMIT
        {
            return Err(SccpRouteValidationError::InvalidSoraOutboundExecutionPolicy);
        }
        Ok(())
    }
}
/// Directional activation state for one complete governed SCCP route.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[norito(tag = "activation", content = "direction")]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpRouteActivationV1")]
pub enum SccpRouteActivationV1 {
    /// The complete route is governed but admits no transfers.
    #[codec(index = 0)]
    #[norito(rename = "staged")]
    Staged,
    /// Both outbound locking and protocol-native inbound redemption are enabled.
    #[codec(index = 1)]
    #[norito(rename = "bidirectional")]
    Bidirectional,
    /// Historical route accepts authenticated redemptions but no new locks.
    #[codec(index = 2)]
    #[norito(rename = "inbound_only")]
    InboundOnly,
    /// Emergency stop for a previously enabled revision; governance may resume it.
    #[codec(index = 3)]
    #[norito(rename = "paused")]
    Paused,
    /// Terminal historical revision; it can neither reactivate nor be removed.
    #[codec(index = 4)]
    #[norito(rename = "retired")]
    Retired,
}
impl SccpRouteActivationV1 {
    /// Return whether SORA-origin outbound messages may use the route.
    #[must_use]
    pub const fn allows_outbound(self) -> bool {
        matches!(self, Self::Bidirectional)
    }
    /// Return whether native external-source proofs may settle through the route.
    #[must_use]
    pub const fn allows_inbound(self) -> bool {
        matches!(self, Self::Bidirectional | Self::InboundOnly)
    }
    /// Return whether this revision is the unique live outbound revision.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        self.allows_outbound()
    }
    /// Return whether this revision is terminal historical state.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Retired)
    }
    /// Return whether this revision still consumes live governance capacity.
    ///
    /// Staged, active, draining, and paused revisions may all change state or
    /// admit traffic again. Only terminal immutable history is excluded.
    #[must_use]
    pub const fn consumes_live_capacity(self) -> bool {
        !self.is_terminal()
    }
    /// Return whether a compare-and-swap transition is legal.
    #[must_use]
    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next || self.is_terminal() {
            return false;
        }
        matches!(
            (self, next),
            (
                Self::Staged | Self::Paused,
                Self::Bidirectional | Self::InboundOnly | Self::Retired
            ) | (Self::Bidirectional, Self::InboundOnly | Self::Paused)
                | (Self::InboundOnly, Self::Paused | Self::Retired)
        )
    }
}
/// Authenticated upper bound for delayed claims on one retired route revision.
///
/// An external event whose fully verified consensus-progress coordinate is at or below
/// `max_anchor_interval_height` remains redeemable after retirement. Events above it are rejected,
/// so a retired emitter cannot create new claims indefinitely. `trust_anchor_hash` binds the cutoff
/// to a complete retained checkpoint interval; the maximum must equal that anchor's successor
/// checkpoint and an open-ended current anchor cannot be retired against.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpInboundFinalityCutoffV1")]
pub struct SccpInboundFinalityCutoffV1 {
    /// Retained lane checkpoint whose validity interval contains the cutoff.
    pub trust_anchor_hash: [u8; 32],
    /// Greatest authenticated backend-specific consensus-progress coordinate admitted.
    ///
    /// Ethereum lanes use a finalized beacon slot. BSC and TRON lanes use a finalized block height.
    pub max_anchor_interval_height: u64,
}
impl SccpInboundFinalityCutoffV1 {
    /// Return whether both cutoff roles are nonzero.
    #[must_use]
    pub fn is_well_formed(self) -> bool {
        self.trust_anchor_hash != [0; 32] && self.max_anchor_interval_height != 0
    }
}
/// Exact immutable lookup key for a governed SCCP route.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpRouteKeyV1")]
pub struct SccpRouteKeyV1 {
    /// Exact external-to-SORA lane.
    pub lane_id: SccpLaneIdV1,
    /// Stable, versioned route identifier carried by SCCP payloads.
    pub route_id: String,
    /// Stable asset key carried by SCCP payloads.
    pub asset_key: String,
    /// Nonzero immutable deployment revision within the semantic lineage.
    pub revision: u32,
}
impl SccpRouteKeyV1 {
    /// Construct a key after validating its exact lane and canonical identifier.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError`] when the lane, identifiers, or
    /// revision do not form a canonical SCCP V1 route key.
    pub fn new(
        lane_id: SccpLaneIdV1,
        route_id: String,
        asset_key: String,
        revision: u32,
    ) -> Result<Self, SccpRouteValidationError> {
        let key = Self {
            lane_id,
            route_id,
            asset_key,
            revision,
        };
        key.validate()?;
        Ok(key)
    }
    /// Validate this exact inbound lane and canonical route id.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError`] when the lane, identifiers, or revision are invalid.
    pub fn validate(&self) -> Result<(), SccpRouteValidationError> {
        validate_inbound_lane(self.lane_id)?;
        validate_key("route_id", &self.route_id)?;
        validate_key("asset_key", &self.asset_key)?;
        if self.revision == 0 {
            return Err(SccpRouteValidationError::InvalidRouteRevision);
        }
        Ok(())
    }
    /// Return whether this key is valid.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.validate().is_ok()
    }
}
/// Exact EVM verifier, bridge, and ERC-20 deployment identity.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpEvmDestinationDeploymentV1")]
pub struct SccpEvmDestinationDeploymentV1 {
    /// Exact ERC-20 token contract address.
    pub token_address: [u8; 20],
    /// Keccak-256 hash of the token runtime bytecode.
    pub token_code_hash: [u8; 32],
    /// Exact Groth16 verifier contract address.
    pub verifier_address: [u8; 20],
    /// Keccak-256 hash of the verifier runtime bytecode.
    pub verifier_code_hash: [u8; 32],
    /// Full fixed verification key consumed by the governed verifier contract.
    pub verifying_key: SccpGroth16Bn254VerifyingKeyV1,
    /// Commitment to the exact Groth16 verification key.
    pub verifier_key_hash: [u8; 32],
    /// Immutable audited semantic circuit and governed SORA finality anchor.
    pub outbound_proof_policy: SccpOutboundProofPolicyV1,
    /// Exact SCCP transfer-route contract address.
    pub route_address: [u8; 20],
    /// Keccak-256 hash of the transfer-route runtime bytecode.
    pub route_code_hash: [u8; 32],
    /// Exact immutable sparse-replay verifier contract address.
    pub replay_verifier_address: [u8; 20],
    /// Keccak-256 hash of the sparse-replay verifier runtime bytecode.
    pub replay_verifier_code_hash: [u8; 32],
    /// Exact immutable disable-only 3-of-5 mint-breaker contract address.
    pub mint_breaker_address: [u8; 20],
    /// Keccak-256 hash of the mint-breaker runtime, including its guardian set.
    pub mint_breaker_code_hash: [u8; 32],
    /// Exact Taira base-unit to wrapped-token base-unit multiplier.
    pub taira_to_token_multiplier: u64,
    /// Positive immutable maximum wrapped-token supply in destination base units.
    pub max_wrapped_supply: u128,
}
/// Exact TRON verifier, route, and TRC-20 deployment identity.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpTronDestinationDeploymentV1")]
pub struct SccpTronDestinationDeploymentV1 {
    /// Raw TRC-20 contract address without the `0x41` network byte.
    pub token_address: [u8; 20],
    /// Keccak-256 hash of the governed token runtime bytecode.
    pub token_code_hash: [u8; 32],
    /// Raw Groth16 verifier address without the `0x41` network byte.
    pub verifier_address: [u8; 20],
    /// Keccak-256 hash of the governed verifier runtime bytecode.
    pub verifier_code_hash: [u8; 32],
    /// Full fixed verification key consumed by the governed verifier contract.
    pub verifying_key: SccpGroth16Bn254VerifyingKeyV1,
    /// Commitment to the exact Groth16 verification key.
    pub verifier_key_hash: [u8; 32],
    /// Immutable audited semantic circuit and governed SORA finality anchor.
    pub outbound_proof_policy: SccpOutboundProofPolicyV1,
    /// Raw SCCP transfer-route address without the `0x41` network byte.
    pub route_address: [u8; 20],
    /// Keccak-256 hash of the governed transfer-route runtime bytecode.
    pub route_code_hash: [u8; 32],
    /// Raw sparse-replay verifier address without the `0x41` network byte.
    pub replay_verifier_address: [u8; 20],
    /// Keccak-256 hash of the sparse-replay verifier runtime bytecode.
    pub replay_verifier_code_hash: [u8; 32],
    /// Raw disable-only 3-of-5 mint-breaker address without the `0x41` network byte.
    pub mint_breaker_address: [u8; 20],
    /// Keccak-256 hash of the mint-breaker runtime, including its guardian set.
    pub mint_breaker_code_hash: [u8; 32],
    /// Exact Taira base-unit to wrapped-token base-unit multiplier.
    pub taira_to_token_multiplier: u64,
    /// Positive immutable maximum wrapped-token supply in destination base units.
    pub max_wrapped_supply: u128,
}
/// Exact ordered five-key TON mint-breaker guardian set.
///
/// Named fields make the fixed cardinality structural in Norito JSON and the
/// generated schema. Their numeric suffixes are also the canonical byte order
/// used by TON `StateInit` and SCCP deployment hash preimages.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpTonMintBreakerGuardianKeysV1")]
pub struct SccpTonMintBreakerGuardianKeysV1 {
    /// Guardian key at canonical index zero.
    pub guardian_0: [u8; 32],
    /// Guardian key at canonical index one.
    pub guardian_1: [u8; 32],
    /// Guardian key at canonical index two.
    pub guardian_2: [u8; 32],
    /// Guardian key at canonical index three.
    pub guardian_3: [u8; 32],
    /// Guardian key at canonical index four.
    pub guardian_4: [u8; 32],
}
impl SccpTonMintBreakerGuardianKeysV1 {
    /// Construct the fixed guardian set from canonical index order.
    #[must_use]
    pub const fn from_array(keys: [[u8; 32]; 5]) -> Self {
        Self {
            guardian_0: keys[0],
            guardian_1: keys[1],
            guardian_2: keys[2],
            guardian_3: keys[3],
            guardian_4: keys[4],
        }
    }
    /// Return the five keys in canonical `StateInit` and hash-preimage order.
    #[must_use]
    pub const fn into_array(self) -> [[u8; 32]; 5] {
        [
            self.guardian_0,
            self.guardian_1,
            self.guardian_2,
            self.guardian_3,
            self.guardian_4,
        ]
    }
    /// Iterate over the five keys in canonical order without allocation.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &[u8; 32]> + DoubleEndedIterator {
        [
            &self.guardian_0,
            &self.guardian_1,
            &self.guardian_2,
            &self.guardian_3,
            &self.guardian_4,
        ]
        .into_iter()
    }
}
impl From<[[u8; 32]; 5]> for SccpTonMintBreakerGuardianKeysV1 {
    fn from(keys: [[u8; 32]; 5]) -> Self {
        Self::from_array(keys)
    }
}
impl From<SccpTonMintBreakerGuardianKeysV1> for [[u8; 32]; 5] {
    fn from(keys: SccpTonMintBreakerGuardianKeysV1) -> Self {
        keys.into_array()
    }
}
/// Exact TON Jetton route and BLS12-381 Groth16 verifier deployment identity.
///
/// TON addresses remain raw workchain/account pairs; user-friendly address
/// flags and checksums never enter consensus state. Code identities are TON
/// representation hashes of the corresponding immutable code cells. The
/// embedded verifier, circuit, verification key, and proof profile are
/// independent governed roles and must not alias one another. TON contracts
/// cannot synchronously call another verifier contract, so the verifier code
/// is linked into the route rather than trusted through an asynchronous
/// callback.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpTonDestinationDeploymentV1")]
pub struct SccpTonDestinationDeploymentV1 {
    /// Canonical raw Jetton master contract address.
    pub jetton_master_address: SccpTonAddressV1,
    /// TON representation hash of the immutable Jetton master code cell.
    pub jetton_master_code_hash: [u8; 32],
    /// TON representation hash of the canonical initial Jetton master data cell.
    ///
    /// The committed cell contains the exact governed bridge configuration,
    /// zero total supply, and empty mint/burn replay dictionaries.
    pub jetton_master_initial_data_hash: [u8; 32],
    /// TON representation hash of the wallet code committed by the Jetton master.
    pub jetton_wallet_code_hash: [u8; 32],
    /// Canonical raw value-moving SCCP destination-route contract address.
    pub route_address: SccpTonAddressV1,
    /// TON representation hash of the immutable destination-route code cell.
    pub route_code_hash: [u8; 32],
    /// TON representation hash of the canonical initial destination-route data cell.
    ///
    /// The committed cell contains the exact governed bridge configuration,
    /// enabled minting, and empty inbound/outbound replay forests and pending maps.
    pub route_initial_data_hash: [u8; 32],
    /// TON representation hash of the immutable BLS12-381 Groth16 verifier
    /// code linked into the route contract.
    pub embedded_verifier_code_hash: [u8; 32],
    /// Commitment to the exact governed BLS12-381 Groth16 circuit.
    pub verifier_circuit_hash: [u8; 32],
    /// Full fixed BLS12-381 verification key consumed by the linked verifier.
    pub verifying_key: SccpGroth16Bls12381VerifyingKeyV1,
    /// Commitment to the exact BLS12-381 Groth16 verification key.
    pub verifier_key_hash: [u8; 32],
    /// Commitment to the proof profile and public-input mapping consumed on TON.
    pub proof_profile_commitment: [u8; 32],
    /// Exact sorted Ed25519 public keys controlling irreversible mint shutdown.
    pub mint_breaker_guardian_keys: SccpTonMintBreakerGuardianKeysV1,
    /// Immutable governed Taira semantic statement and finality anchor.
    pub outbound_proof_policy: SccpOutboundProofPolicyV1,
    /// Exact Taira base-unit to Jetton base-unit multiplier.
    pub taira_to_token_multiplier: u64,
    /// Positive immutable maximum wrapped-token supply in destination base units.
    pub max_wrapped_supply: u128,
}
/// Closed family-specific destination deployment.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[norito(tag = "family", content = "deployment")]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpDestinationDeploymentV1")]
pub enum SccpDestinationDeploymentV1 {
    /// EVM deployment for Ethereum or BSC.
    #[codec(index = 0)]
    #[norito(rename = "evm")]
    Evm(SccpEvmDestinationDeploymentV1),
    /// TRON TVM deployment.
    #[codec(index = 1)]
    #[norito(rename = "tron")]
    Tron(SccpTronDestinationDeploymentV1),
    /// TON basechain Jetton route and BLS12-381 Groth16 verifier deployment.
    #[codec(index = 2)]
    #[norito(rename = "ton")]
    Ton(SccpTonDestinationDeploymentV1),
}
impl SccpDestinationDeploymentV1 {
    /// Return the positive governed wrapped-supply ceiling.
    #[must_use]
    pub const fn max_wrapped_supply(&self) -> u128 {
        match self {
            Self::Evm(deployment) => deployment.max_wrapped_supply,
            Self::Tron(deployment) => deployment.max_wrapped_supply,
            Self::Ton(deployment) => deployment.max_wrapped_supply,
        }
    }

    /// Return the exact Taira-to-destination base-unit multiplier.
    #[must_use]
    pub const fn taira_to_token_multiplier(&self) -> u64 {
        match self {
            Self::Evm(deployment) => deployment.taira_to_token_multiplier,
            Self::Tron(deployment) => deployment.taira_to_token_multiplier,
            Self::Ton(deployment) => deployment.taira_to_token_multiplier,
        }
    }

    /// Return the exact governed destination verification-key commitment.
    #[must_use]
    pub const fn verifier_key_hash(&self) -> [u8; 32] {
        match self {
            Self::Evm(deployment) => deployment.verifier_key_hash,
            Self::Tron(deployment) => deployment.verifier_key_hash,
            Self::Ton(deployment) => deployment.verifier_key_hash,
        }
    }
    /// Return the mandatory immutable outbound proof policy.
    #[must_use]
    pub const fn outbound_proof_policy(&self) -> SccpOutboundProofPolicyV1 {
        match self {
            Self::Evm(deployment) => deployment.outbound_proof_policy,
            Self::Tron(deployment) => deployment.outbound_proof_policy,
            Self::Ton(deployment) => deployment.outbound_proof_policy,
        }
    }
    /// Validate exact family identity and role separation for an inbound lane.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError`] when the lane is invalid, the
    /// deployment family does not match it, or deployment roles overlap.
    pub fn validate_for_lane(&self, lane: SccpLaneIdV1) -> Result<(), SccpRouteValidationError> {
        validate_inbound_lane(lane)?;
        if lane.target != SccpNetworkV1::SoraTaira {
            return Err(SccpRouteValidationError::UnsupportedSoraEndpoint);
        }
        match (self, lane.source) {
            (Self::Evm(deployment), SccpNetworkV1::EthereumMainnet | SccpNetworkV1::BscMainnet) => {
                validate_evm_deployment(deployment)
            }
            (Self::Tron(deployment), SccpNetworkV1::TronMainnet) => {
                validate_tron_deployment(deployment)
            }
            (Self::Ton(deployment), SccpNetworkV1::TonMainnet) => {
                validate_ton_deployment(deployment)
            }
            _ => Err(SccpRouteValidationError::DestinationFamilyMismatch),
        }
    }
    /// Return whether this deployment is valid for an exact inbound lane.
    #[must_use]
    pub fn is_well_formed_for_lane(&self, lane: SccpLaneIdV1) -> bool {
        self.validate_for_lane(lane).is_ok()
    }
    /// Derive the exact destination binding consumed by the family implementation.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError`] when the lane or destination
    /// deployment is invalid for SCCP V1.
    pub fn destination_binding_hash(
        &self,
        lane: SccpLaneIdV1,
    ) -> Result<[u8; 32], SccpRouteValidationError> {
        self.validate_for_lane(lane)?;
        match self {
            Self::Evm(deployment) => sccp_evm_destination_binding_hash_v1(lane.source, deployment),
            Self::Tron(deployment) => {
                sccp_tron_destination_binding_hash_v1(lane.source, deployment)
            }
            Self::Ton(deployment) => sccp_ton_destination_binding_hash_v1(lane.source, deployment),
        }
    }
    /// Derive the immutable route-configuration hash exposed by the deployment.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError`] when the lane, route identity,
    /// revision, settlement scale, or deployment is invalid.
    pub fn route_configuration_hash(
        &self,
        lane: SccpLaneIdV1,
        route_id: &str,
        asset_key: &str,
        route_revision: u32,
        payload_amount_scale: u32,
    ) -> Result<[u8; 32], SccpRouteValidationError> {
        self.validate_for_lane(lane)?;
        if route_revision == 0 {
            return Err(SccpRouteValidationError::InvalidRouteRevision);
        }
        validate_concrete_route_identity(lane.source, route_id, asset_key, payload_amount_scale)?;
        let reverse_lane = SccpLaneIdV1 {
            source: lane.target,
            target: lane.source,
        };
        let source_lane_hash =
            sccp_lane_id_hash_v1(lane).ok_or(SccpRouteValidationError::InvalidInboundLane)?;
        let destination_lane_hash = sccp_lane_id_hash_v1(reverse_lane)
            .ok_or(SccpRouteValidationError::InvalidInboundLane)?;
        match self {
            Self::Evm(deployment) => sccp_exact_evm_xor_route_config_hash_v1(
                lane.source,
                source_lane_hash,
                destination_lane_hash,
                deployment,
                route_revision,
            ),
            Self::Tron(deployment) => sccp_exact_tron_xor_route_config_hash_v1(
                lane.source,
                source_lane_hash,
                destination_lane_hash,
                deployment,
                route_revision,
            ),
            Self::Ton(deployment) => sccp_exact_ton_xor_route_config_hash_v1(
                lane.source,
                source_lane_hash,
                destination_lane_hash,
                deployment,
                route_revision,
            ),
        }
    }
}
/// Typed SORA-side asset and liability policy for atomic SCCP settlement.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpSoraSettlementV1")]
pub struct SccpSoraSettlementV1 {
    /// Canonical SORA-home asset definition locked and released by Core.
    pub asset_definition_id: AssetDefinitionId,
    /// Decimal scale used by the SCCP unsigned amount field.
    pub payload_amount_scale: u32,
    /// Positive immutable ceiling for route escrow liability in Taira base units.
    pub max_outstanding_liability: u128,
}
/// Derive the non-signable protocol escrow account for one exact SCCP route.
///
/// The derivation binds the genesis-derived [`NetworkId`], the complete
/// canonical route key (including its immutable revision), and the settlement
/// asset. No private signing scalar is known for the derived Ed25519 point.
#[must_use]
pub fn sccp_route_escrow_account_id_v1(
    network_id: &NetworkId,
    route_key: &SccpRouteKeyV1,
    asset_definition_id: &AssetDefinitionId,
) -> AccountId {
    let route_key = route_key.encode();
    let asset_definition_id = asset_definition_id.encode();
    AccountId::new(derive_non_signing_ed25519_public_key(
        ROUTE_ESCROW_ACCOUNT_DOMAIN_V1,
        &[
            network_id.as_bytes(),
            route_key.as_slice(),
            asset_definition_id.as_slice(),
        ],
    ))
}
impl SccpSoraSettlementV1 {
    /// Validate the exact first-release Taira XOR settlement identity and scale.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError`] when the settlement asset or amount
    /// scale differs from the first-release Taira XOR contract.
    pub fn validate(&self) -> Result<(), SccpRouteValidationError> {
        if self.asset_definition_id != sccp_v1_taira_xor_asset_definition_id() {
            return Err(SccpRouteValidationError::SettlementAssetMismatch);
        }
        if self.payload_amount_scale != SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE {
            return Err(SccpRouteValidationError::InvalidSettlementScale);
        }
        if self.max_outstanding_liability == 0 {
            return Err(SccpRouteValidationError::InvalidSupplyCap);
        }
        Ok(())
    }
    /// Return whether the amount scale is representable in first-release settlement.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.validate().is_ok()
    }
}
/// Parse the built-in canonical live Taira XOR asset definition id.
///
/// The literal is release protocol state and is covered by data-model tests;
/// failure therefore indicates a programmer error rather than runtime input.
#[must_use]
pub fn sccp_v1_taira_xor_asset_definition_id() -> AssetDefinitionId {
    AssetDefinitionId::parse_address_literal(SCCP_V1_TAIRA_XOR_ASSET_DEFINITION_ID)
        .expect("built-in SCCP Taira XOR asset definition id must remain valid")
}
/// One complete, atomic, immutable-identity SCCP route governance record.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpGovernedRouteV1")]
pub struct SccpGovernedRouteV1 {
    /// Exact external-to-SORA lane governed by this record.
    pub lane_id: SccpLaneIdV1,
    /// Stable, versioned route identifier carried by SCCP payloads.
    pub route_id: String,
    /// Stable asset key carried by SCCP payloads.
    pub asset_key: String,
    /// Nonzero immutable deployment revision within the semantic lineage.
    pub revision: u32,
    /// Directional activation state.
    pub activation: SccpRouteActivationV1,
    /// Delayed-claim cutoff, present exactly in terminal historical state.
    pub inbound_finality_cutoff: Option<SccpInboundFinalityCutoffV1>,
    /// Exact source-emitter identity used for native inbound admission.
    pub source_identity: SccpSourceIdentityV1,
    /// Exact reverse-direction destination deployment.
    pub destination: SccpDestinationDeploymentV1,
    /// Exact TAIRA-side proved burn-and-record execution policy.
    pub sora_outbound_execution_policy: SccpSoraOutboundExecutionPolicyV1,
    /// Typed atomic SORA settlement policy.
    pub settlement: SccpSoraSettlementV1,
}
impl SccpGovernedRouteV1 {
    /// Return the first proposal-owned route value above the exact JSON integer maximum.
    pub(crate) fn first_release_exact_json_u64_invariant_error(
        &self,
        maximum: u64,
    ) -> Option<&'static str> {
        if self
            .inbound_finality_cutoff
            .is_some_and(|cutoff| cutoff.max_anchor_interval_height > maximum)
        {
            return Some(
                "SCCP proposal inbound finality cutoff exceeds the exact JSON integer maximum",
            );
        }
        if self.sora_outbound_execution_policy.gas_limit > maximum {
            return Some("SCCP proposal gas limit exceeds the exact JSON integer maximum");
        }
        if self
            .destination
            .outbound_proof_policy()
            .sora_finality_anchor
            .epoch
            > maximum
            || self
                .destination
                .outbound_proof_policy()
                .sora_finality_anchor
                .epoch_end_height
                > maximum
            || self
                .destination
                .outbound_proof_policy()
                .sora_finality_anchor
                .checkpoint_height
                > maximum
        {
            return Some(
                "SCCP proposal finality epoch or checkpoint exceeds the exact JSON integer maximum",
            );
        }
        match &self.destination {
            SccpDestinationDeploymentV1::Evm(deployment) => {
                if deployment.taira_to_token_multiplier > maximum {
                    return Some(
                        "SCCP proposal EVM multiplier exceeds the exact JSON integer maximum",
                    );
                }
            }
            SccpDestinationDeploymentV1::Tron(deployment) => {
                if deployment.taira_to_token_multiplier > maximum {
                    return Some(
                        "SCCP proposal TRON multiplier exceeds the exact JSON integer maximum",
                    );
                }
            }
            SccpDestinationDeploymentV1::Ton(deployment) => {
                if deployment.taira_to_token_multiplier > maximum {
                    return Some(
                        "SCCP proposal TON multiplier exceeds the exact JSON integer maximum",
                    );
                }
            }
        }
        None
    }

    /// Return the immutable lookup key of this route.
    #[must_use]
    pub fn key(&self) -> SccpRouteKeyV1 {
        SccpRouteKeyV1 {
            lane_id: self.lane_id,
            route_id: self.route_id.clone(),
            asset_key: self.asset_key.clone(),
            revision: self.revision,
        }
    }
    /// Validate every immutable route component and the selected activation.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError`] when any route identity, settlement,
    /// deployment, source binding, activation, or cutoff invariant is violated.
    pub fn validate(&self) -> Result<(), SccpRouteValidationError> {
        self.key().validate()?;
        validate_key("asset_key", &self.asset_key)?;
        self.settlement.validate()?;
        self.sora_outbound_execution_policy.validate()?;
        self.destination.validate_for_lane(self.lane_id)?;
        let expected_wrapped_supply = self
            .settlement
            .max_outstanding_liability
            .checked_mul(u128::from(self.destination.taira_to_token_multiplier()))
            .ok_or(SccpRouteValidationError::InvalidSupplyCap)?;
        if self.destination.max_wrapped_supply() == 0
            || self.destination.max_wrapped_supply() != expected_wrapped_supply
        {
            return Err(SccpRouteValidationError::InvalidSupplyCap);
        }
        if self.source_identity.lane != self.lane_id || !self.source_identity.is_well_formed() {
            return Err(SccpRouteValidationError::SourceDestinationMismatch);
        }
        let route_config_hash = self.destination.route_configuration_hash(
            self.lane_id,
            &self.route_id,
            &self.asset_key,
            self.revision,
            self.settlement.payload_amount_scale,
        )?;
        let destination_binding_hash = self.destination.destination_binding_hash(self.lane_id)?;
        let outbound_proof_policy = self.destination.outbound_proof_policy();
        validate_hash_roles(&[
            self.sora_outbound_execution_policy.contract_artifact_sha256,
            self.sora_outbound_execution_policy.vk_ref.commitment,
            route_config_hash,
            destination_binding_hash,
            self.destination.verifier_key_hash(),
            outbound_proof_policy.semantic_profile_hash()?,
            outbound_proof_policy.sora_finality_anchor_hash()?,
        ])?;
        if let SccpDestinationDeploymentV1::Ton(deployment) = self.destination {
            validate_hash_roles(&[
                self.sora_outbound_execution_policy.contract_artifact_sha256,
                self.sora_outbound_execution_policy.vk_ref.commitment,
                route_config_hash,
                destination_binding_hash,
                deployment.jetton_master_initial_data_hash,
                deployment.route_initial_data_hash,
                self.destination.verifier_key_hash(),
                outbound_proof_policy.semantic_profile_hash()?,
                outbound_proof_policy.sora_finality_anchor_hash()?,
            ])?;
        }
        if !source_matches_destination(
            self.source_identity.emitter,
            &self.destination,
            route_config_hash,
        ) {
            return Err(SccpRouteValidationError::SourceDestinationMismatch);
        }
        if self.activation.allows_inbound() && !self.supports_inbound_activation() {
            return Err(SccpRouteValidationError::UnsupportedInboundActivation);
        }
        let cutoff_is_valid = match (self.activation.is_terminal(), self.inbound_finality_cutoff) {
            (true, Some(cutoff)) => cutoff.is_well_formed(),
            (false, None) => true,
            _ => false,
        };
        if !cutoff_is_valid {
            return Err(SccpRouteValidationError::InvalidInboundFinalityCutoff);
        }
        Ok(())
    }
    /// Validate a route specifically for first registration.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError`] when the route is invalid or is not
    /// in the required staged registration state.
    pub fn validate_registration(&self) -> Result<(), SccpRouteValidationError> {
        self.validate()?;
        if self.activation != SccpRouteActivationV1::Staged {
            return Err(SccpRouteValidationError::RegistrationMustBeStaged);
        }
        Ok(())
    }
    /// Validate the route against its lane-level native checkpoint.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError`] when the route or supplied trust
    /// anchor is invalid, mismatched, or insufficient for inbound activation.
    pub fn validate_with_anchor(
        &self,
        native_trust_anchor: Option<SccpNativeTrustAnchorV1>,
    ) -> Result<(), SccpRouteValidationError> {
        self.validate()?;
        if let Some(native_trust_anchor) = native_trust_anchor {
            if !native_trust_anchor.is_well_formed() {
                return Err(SccpRouteValidationError::InvalidTrustAnchor);
            }
            if !native_backend_matches_family(native_trust_anchor.backend, self.lane_id.source) {
                return Err(SccpRouteValidationError::TrustAnchorFamilyMismatch);
            }
        }
        if self.activation.allows_inbound() {
            let Some(native_trust_anchor) = native_trust_anchor else {
                return Err(SccpRouteValidationError::UnsupportedInboundActivation);
            };
            if !native_trust_anchor
                .backend
                .supports_source_network(self.lane_id.source)
            {
                return Err(SccpRouteValidationError::UnsupportedInboundActivation);
            }
        }
        Ok(())
    }
    /// Return whether every immutable route component is complete and exact.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.validate().is_ok()
    }
    /// Return whether native inbound settlement may be enabled safely.
    #[must_use]
    pub fn supports_inbound_activation(&self) -> bool {
        self.lane_id.source.supports_native_inbound_source()
            && self.source_identity.is_well_formed()
            && self.source_identity.has_governance_activatable_source()
    }
    /// Return whether this record's selected activation is internally valid.
    #[must_use]
    pub fn activation_is_valid(&self) -> bool {
        self.validate().is_ok()
    }
    /// Return whether an authenticated backend-specific consensus-progress
    /// coordinate may settle through this revision.
    #[must_use]
    pub fn allows_inbound_at(&self, anchor_interval_height: u64) -> bool {
        self.activation.allows_inbound()
            || (self.activation.is_terminal()
                && self.inbound_finality_cutoff.is_some_and(|cutoff| {
                    anchor_interval_height <= cutoff.max_anchor_interval_height
                }))
    }
    /// Derive the exact immutable route-configuration hash exposed by the destination contract.
    ///
    /// This is the single V1 route-configuration commitment recorded in outbound messages and
    /// exposed as Groth16 public signal 9. It must remain byte-identical to the governed
    /// EVM/TVM route configuration commitment.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError`] when the route or destination
    /// deployment is not a valid SCCP V1 configuration.
    pub fn route_configuration_hash(&self) -> Result<[u8; 32], SccpRouteValidationError> {
        self.validate()?;
        self.destination.route_configuration_hash(
            self.lane_id,
            &self.route_id,
            &self.asset_key,
            self.revision,
            self.settlement.payload_amount_scale,
        )
    }
    /// Derive the destination deployment binding committed by outbound messages.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError`] when the lane or destination deployment is invalid.
    pub fn destination_binding_hash(&self) -> Result<[u8; 32], SccpRouteValidationError> {
        self.destination.destination_binding_hash(self.lane_id)
    }
}
/// One append-only lane checkpoint history and its exact immutable routes.
///
/// Every native checkpoint remains available after rotation so a message
/// finalized under an earlier checkpoint cannot be stranded while in flight.
/// The current pointer names the last, highest checkpoint and prevents routes
/// sharing native consensus from drifting to different active checkpoints.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpGovernedLaneV1")]
pub struct SccpGovernedLaneV1 {
    /// Exact external-to-SORA lane.
    pub lane_id: SccpLaneIdV1,
    /// Immutable family-tagged native checkpoints in strictly increasing order.
    ///
    /// Governance appends at most one checkpoint per action. No checkpoint is
    /// deleted because externally finalized messages have no safe implicit
    /// expiry in V1; an append at the retained-history bound rejects instead.
    pub native_trust_anchors: Vec<SccpNativeTrustAnchorV1>,
    /// Hash of the last (highest) retained checkpoint, or `None` for an
    /// anchorless staged/outbound-only lane.
    pub current_native_trust_anchor_hash: Option<[u8; 32]>,
    /// Retained routes sharing this lane's checkpoint history.
    ///
    /// Nonterminal routes are bounded; terminal revisions remain as immutable
    /// history for exact message and configuration resolution.
    pub routes: Vec<SccpGovernedRouteV1>,
}
impl SccpGovernedLaneV1 {
    /// Return the current highest native checkpoint.
    #[must_use]
    pub fn current_native_trust_anchor(&self) -> Option<SccpNativeTrustAnchorV1> {
        let current_hash = self.current_native_trust_anchor_hash?;
        self.native_trust_anchors
            .last()
            .copied()
            .filter(|anchor| anchor.anchor_hash == current_hash)
    }
    /// Resolve one retained native checkpoint by its authenticated hash.
    ///
    /// Consensus admission uses a precomputed registry index for this lookup;
    /// this lane-local helper is intended for validation and small wire values.
    #[must_use]
    pub fn native_trust_anchor_by_hash(
        &self,
        anchor_hash: [u8; 32],
    ) -> Option<SccpNativeTrustAnchorV1> {
        self.native_trust_anchors
            .iter()
            .copied()
            .find(|anchor| anchor.anchor_hash == anchor_hash)
    }
    /// Resolve a retained checkpoint and its inclusive successor boundary.
    #[must_use]
    pub fn native_trust_anchor_interval(
        &self,
        anchor_hash: [u8; 32],
    ) -> Option<(SccpNativeTrustAnchorV1, Option<u64>)> {
        let index = self
            .native_trust_anchors
            .iter()
            .position(|anchor| anchor.anchor_hash == anchor_hash)?;
        Some((
            self.native_trust_anchors[index],
            self.native_trust_anchors
                .get(index + 1)
                .map(|next| next.checkpoint_height),
        ))
    }
    /// Return whether a retirement cutoff closes one complete historical
    /// anchor interval through its successor checkpoint, inclusively.
    #[must_use]
    pub fn is_complete_inbound_finality_interval(
        &self,
        cutoff: SccpInboundFinalityCutoffV1,
    ) -> bool {
        let Some(anchor_index) = self
            .native_trust_anchors
            .iter()
            .position(|anchor| anchor.anchor_hash == cutoff.trust_anchor_hash)
        else {
            return false;
        };
        self.native_trust_anchors
            .get(anchor_index + 1)
            .map(|next| next.checkpoint_height)
            == Some(cutoff.max_anchor_interval_height)
    }
    /// Validate bounded append-only history, bounded live routes, and membership.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError`] when checkpoint history, route
    /// history, lineage, activation, or lane membership is invalid.
    pub fn validate(&self) -> Result<(), SccpRouteValidationError> {
        validate_inbound_lane(self.lane_id)?;
        if self.native_trust_anchors.len() > SCCP_V1_MAX_RETAINED_NATIVE_TRUST_ANCHORS_PER_LANE {
            return Err(SccpRouteValidationError::TooManyRetainedTrustAnchors);
        }
        if self.routes.len() > SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE {
            return Err(SccpRouteValidationError::TooManyRetainedRoutes);
        }
        let mut previous_anchor = None;
        let mut anchor_hashes = BTreeSet::new();
        for native_trust_anchor in &self.native_trust_anchors {
            if !native_trust_anchor.is_well_formed() {
                return Err(SccpRouteValidationError::InvalidTrustAnchor);
            }
            if !native_backend_matches_family(native_trust_anchor.backend, self.lane_id.source) {
                return Err(SccpRouteValidationError::TrustAnchorFamilyMismatch);
            }
            if !anchor_hashes.insert(native_trust_anchor.anchor_hash)
                || previous_anchor.is_some_and(|previous: SccpNativeTrustAnchorV1| {
                    native_trust_anchor.backend != previous.backend
                        || native_trust_anchor.checkpoint_height <= previous.checkpoint_height
                })
            {
                return Err(SccpRouteValidationError::InvalidTrustAnchorHistory);
            }
            previous_anchor = Some(*native_trust_anchor);
        }
        if self.current_native_trust_anchor_hash
            != self
                .native_trust_anchors
                .last()
                .map(|anchor| anchor.anchor_hash)
        {
            return Err(SccpRouteValidationError::InvalidCurrentTrustAnchor);
        }
        let current_native_trust_anchor = self.current_native_trust_anchor();
        let live_route_count = self
            .routes
            .iter()
            .filter(|route| route.activation.consumes_live_capacity())
            .count();
        if self.routes.is_empty() || live_route_count > SCCP_V1_MAX_LIVE_ROUTES_PER_LANE {
            return Err(SccpRouteValidationError::InvalidLaneLiveRouteCount);
        }
        let mut lineages = BTreeMap::<(&str, &str), Vec<(u32, bool)>>::new();
        let mut tron_source_addresses = BTreeSet::new();
        let mut ton_emitters = BTreeSet::new();
        for route in &self.routes {
            route.validate_with_anchor(current_native_trust_anchor)?;
            if route
                .inbound_finality_cutoff
                .is_some_and(|cutoff| !self.is_complete_inbound_finality_interval(cutoff))
            {
                return Err(SccpRouteValidationError::InvalidInboundFinalityCutoff);
            }
            if route.lane_id != self.lane_id {
                return Err(SccpRouteValidationError::InvalidInboundLane);
            }
            if let SccpSourceEmitterV1::Tron(emitter) = route.source_identity.emitter
                && !tron_source_addresses.insert(emitter.address)
            {
                return Err(SccpRouteValidationError::DuplicateTronSourceAddress);
            }
            if let SccpSourceEmitterV1::Ton(emitter) = route.source_identity.emitter
                && !ton_emitters.insert(emitter.address)
            {
                return Err(SccpRouteValidationError::DuplicateTonSourceAddress);
            }
            lineages
                .entry((route.route_id.as_str(), route.asset_key.as_str()))
                .or_default()
                .push((route.revision, route.activation.is_enabled()));
        }
        for revisions in lineages.values_mut() {
            revisions.sort_unstable_by_key(|(revision, _)| *revision);
            if revisions.first().map(|(revision, _)| *revision) != Some(1)
                || revisions.windows(2).any(|pair| {
                    pair[0]
                        .0
                        .checked_add(1)
                        .is_none_or(|expected| pair[1].0 != expected)
                })
            {
                return Err(SccpRouteValidationError::InvalidRouteRevision);
            }
            if revisions.iter().filter(|(_, enabled)| *enabled).count() > 1 {
                return Err(SccpRouteValidationError::MultipleEnabledRevisions);
            }
        }
        Ok(())
    }
}
/// Versioned authoritative SCCP route registry payload.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::bridge::sccp_registry::SccpRegistryV1")]
pub struct SccpRegistryV1 {
    /// Registry format version. First release accepts exactly `1`.
    pub version: u8,
    /// Governed lanes with bounded append-only history and bounded live routes.
    pub lanes: Vec<SccpGovernedLaneV1>,
}
impl Default for SccpRegistryV1 {
    fn default() -> Self {
        Self {
            version: 1,
            lanes: Vec::new(),
        }
    }
}
impl SccpRegistryV1 {
    /// Validate the bounded registry, live surface, and uniqueness invariants.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError`] when the version, lane registry,
    /// route keys, destination bindings, or route configurations are invalid.
    pub fn validate(&self) -> Result<(), SccpRouteValidationError> {
        if self.version != 1 {
            return Err(SccpRouteValidationError::UnsupportedRegistryVersion);
        }
        if self.lanes.len() > SCCP_V1_MAX_GOVERNED_LANES {
            return Err(SccpRouteValidationError::TooManyLanes);
        }
        if self.lanes.iter().any(|lane| {
            lane.native_trust_anchors.len() > SCCP_V1_MAX_RETAINED_NATIVE_TRUST_ANCHORS_PER_LANE
        }) {
            return Err(SccpRouteValidationError::TooManyRetainedTrustAnchors);
        }
        if self
            .lanes
            .iter()
            .any(|lane| lane.routes.len() > SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE)
        {
            return Err(SccpRouteValidationError::TooManyRetainedRoutes);
        }
        let live_route_count = self
            .lanes
            .iter()
            .flat_map(|lane| &lane.routes)
            .filter(|route| route.activation.consumes_live_capacity())
            .count();
        if live_route_count > SCCP_V1_MAX_LIVE_GOVERNED_ROUTES {
            return Err(SccpRouteValidationError::TooManyLiveRoutes);
        }
        let mut lanes = BTreeSet::new();
        let mut keys = BTreeSet::new();
        let mut bindings = BTreeSet::new();
        let mut configurations = BTreeSet::new();
        for lane in &self.lanes {
            lane.validate()?;
            if !lanes.insert(lane.lane_id) {
                return Err(SccpRouteValidationError::DuplicateLane);
            }
            for route in &lane.routes {
                if !keys.insert(route.key()) {
                    return Err(SccpRouteValidationError::DuplicateRouteKey);
                }
                if !bindings.insert(route.destination_binding_hash()?) {
                    return Err(SccpRouteValidationError::DuplicateDestinationBinding);
                }
                if !configurations.insert(route.route_configuration_hash()?) {
                    return Err(SccpRouteValidationError::DuplicateRouteConfiguration);
                }
            }
        }
        Ok(())
    }
}
/// Return the stable one-byte V1 tag for an exact SCCP network profile.
///
/// Final V1 uses a fresh contiguous block above the fixed operation tags. Every pre-release sparse
/// profile tag is invalid, so no retired encoding can be reinterpreted as an admitted network.
#[must_use]
pub const fn sccp_network_tag_v1(network: SccpNetworkV1) -> u8 {
    match network {
        SccpNetworkV1::SoraTaira => 0x40,
        SccpNetworkV1::EthereumMainnet => 0x41,
        SccpNetworkV1::BscMainnet => 0x42,
        SccpNetworkV1::TronMainnet => 0x43,
        SccpNetworkV1::TonMainnet => 0x44,
    }
}
/// Return canonical bytes of the ordered eleven-signal Groth16 schema.
#[must_use]
pub fn canonical_sccp_groth16_bn254_public_signal_schema_bytes_v1() -> Vec<u8> {
    let mut out = Vec::with_capacity(768);
    out.push(1);
    push_u32(
        &mut out,
        u32::try_from(GROTH16_PUBLIC_SIGNAL_LABELS_V1.len())
            .expect("fixed SCCP public-signal count fits u32"),
    );
    for label in GROTH16_PUBLIC_SIGNAL_LABELS_V1 {
        push_vec(&mut out, label);
    }
    out
}
/// Hash the exact ordered eleven-signal Groth16 public-input schema.
#[must_use]
pub fn sccp_groth16_bn254_public_signal_schema_hash_v1() -> [u8; 32] {
    let mut preimage = Vec::with_capacity(1024);
    preimage.extend_from_slice(GROTH16_PUBLIC_SIGNAL_SCHEMA_HASH_DOMAIN_V1);
    preimage.extend_from_slice(&canonical_sccp_groth16_bn254_public_signal_schema_bytes_v1());
    keccak256(preimage)
}
/// Return canonical bytes of the ordered eleven-signal BLS12-381 Groth16
/// schema used by TON destinations.
#[must_use]
pub fn canonical_sccp_groth16_bls12381_public_signal_schema_bytes_v1() -> Vec<u8> {
    let mut out = Vec::with_capacity(768);
    out.push(1);
    push_u32(
        &mut out,
        u32::try_from(GROTH16_BLS12381_PUBLIC_SIGNAL_LABELS_V1.len())
            .expect("fixed SCCP public-signal count fits u32"),
    );
    for label in GROTH16_BLS12381_PUBLIC_SIGNAL_LABELS_V1 {
        push_vec(&mut out, label);
    }
    out
}
/// Hash the exact BLS12-381 ordered eleven-signal public-input schema.
#[must_use]
pub fn sccp_groth16_bls12381_public_signal_schema_hash_v1() -> [u8; 32] {
    let mut preimage = Vec::with_capacity(1024);
    preimage.extend_from_slice(GROTH16_BLS12381_PUBLIC_SIGNAL_SCHEMA_HASH_DOMAIN_V1);
    preimage.extend_from_slice(&canonical_sccp_groth16_bls12381_public_signal_schema_bytes_v1());
    sha256_bytes(&preimage)
}
/// Derive the single proof-profile commitment accepted by TON SCCP V1.
#[must_use]
pub fn sccp_ton_groth16_bls12381_proof_profile_commitment_v1() -> [u8; 32] {
    let mut preimage = Vec::with_capacity(256);
    preimage.extend_from_slice(TON_GROTH16_BLS12381_PROOF_PROFILE_PREFIX_V1);
    preimage.push(1);
    preimage.extend_from_slice(b"ietf-bls12381-compressed-g1-48-g2-96");
    preimage.extend_from_slice(b"groth16-a-g1-b-g2-c-g1");
    preimage.extend_from_slice(b"sha256-sha256-label-value-mod-r");
    preimage.extend_from_slice(&GROTH16_BLS12381_SCALAR_FIELD_MODULUS_BE);
    preimage.extend_from_slice(&sccp_groth16_bls12381_public_signal_schema_hash_v1());
    sha256_bytes(&preimage)
}
/// Return the canonical Taira chain-id commitment used by finality anchors.
#[must_use]
pub fn sccp_sora_taira_chain_id_hash_v1() -> [u8; 32] {
    keccak256(SORA_TAIRA_CHAIN_ID_BYTES)
}
/// Encode one valid semantic proof profile independently of Norito framing.
///
/// # Errors
///
/// Returns [`SccpRouteValidationError`] when the semantic proof profile is not
/// canonical SCCP V1 data.
pub fn canonical_sccp_semantic_proof_profile_bytes_v1(
    profile: SccpSemanticProofProfileV1,
) -> Result<Vec<u8>, SccpRouteValidationError> {
    profile.validate()?;
    let mut out = Vec::with_capacity(99);
    out.push(1);
    match profile {
        SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(circuit) => {
            out.push(0);
            out.push(circuit.version);
            out.extend_from_slice(&circuit.circuit_commitment);
            out.extend_from_slice(&circuit.witness_generator_commitment);
            out.extend_from_slice(&circuit.public_signal_schema_hash);
        }
        SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bls12381(circuit) => {
            out.push(1);
            out.push(circuit.version);
            out.extend_from_slice(&circuit.circuit_commitment);
            out.extend_from_slice(&circuit.witness_generator_commitment);
            out.extend_from_slice(&circuit.public_signal_schema_hash);
        }
    }
    Ok(out)
}
/// Hash one valid semantic proof profile for destination-contract pinning.
///
/// # Errors
///
/// Returns [`SccpRouteValidationError`] when the semantic proof profile is not
/// canonical SCCP V1 data.
pub fn sccp_semantic_proof_profile_hash_v1(
    profile: SccpSemanticProofProfileV1,
) -> Result<[u8; 32], SccpRouteValidationError> {
    let mut preimage = Vec::with_capacity(160);
    preimage.extend_from_slice(SEMANTIC_PROOF_PROFILE_HASH_DOMAIN_V1);
    preimage.extend_from_slice(&canonical_sccp_semantic_proof_profile_bytes_v1(profile)?);
    Ok(keccak256(preimage))
}
/// Encode one valid Taira finality anchor independently of Norito framing.
///
/// # Errors
///
/// Returns [`SccpRouteValidationError`] when the finality anchor is not a valid
/// nonzero Taira Sumeragi-v2 checkpoint.
pub fn canonical_sccp_sora_finality_anchor_bytes_v1(
    anchor: SccpSoraFinalityAnchorV1,
) -> Result<Vec<u8>, SccpRouteValidationError> {
    anchor.validate()?;
    let mut out = Vec::with_capacity(SCCP_V1_SORA_FINALITY_ANCHOR_BYTES);
    out.push(anchor.version);
    out.push(sccp_network_tag_v1(anchor.source_network));
    push_u16(&mut out, anchor.protocol_version);
    out.extend_from_slice(&anchor.chain_id_hash);
    push_u64(&mut out, anchor.epoch);
    push_u64(&mut out, anchor.epoch_end_height);
    out.extend_from_slice(&anchor.roster_commitment);
    push_u64(&mut out, anchor.checkpoint_height);
    out.extend_from_slice(&anchor.checkpoint_block_hash);
    out.extend_from_slice(&anchor.checkpoint_context_id);
    out.extend_from_slice(&anchor.checkpoint_finality_artifact_hash);
    debug_assert_eq!(out.len(), SCCP_V1_SORA_FINALITY_ANCHOR_BYTES);
    Ok(out)
}
/// Hash one valid Taira finality anchor for destination-contract pinning.
///
/// # Errors
///
/// Returns [`SccpRouteValidationError`] when the finality anchor is not a valid
/// nonzero Taira Sumeragi-v2 checkpoint.
pub fn sccp_sora_finality_anchor_hash_v1(
    anchor: SccpSoraFinalityAnchorV1,
) -> Result<[u8; 32], SccpRouteValidationError> {
    let mut preimage = Vec::with_capacity(192);
    preimage.extend_from_slice(SORA_FINALITY_ANCHOR_HASH_DOMAIN_V1);
    preimage.extend_from_slice(&canonical_sccp_sora_finality_anchor_bytes_v1(anchor)?);
    Ok(keccak256(preimage))
}
/// Return canonical V1 bytes for an exact SCCP network profile.
#[must_use]
pub fn canonical_sccp_network_bytes_v1(network: SccpNetworkV1) -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    out.push(1);
    out.push(sccp_network_tag_v1(network));
    push_u32(&mut out, network.domain_id());
    match network {
        SccpNetworkV1::SoraTaira => out.extend_from_slice(&SORA_TAIRA_CHAIN_ID_BYTES),
        SccpNetworkV1::EthereumMainnet => push_u64(&mut out, 1),
        SccpNetworkV1::BscMainnet => push_u64(&mut out, 56),
        SccpNetworkV1::TronMainnet => push_u32(&mut out, 0x2b66_53dc),
        SccpNetworkV1::TonMainnet => {
            push_i32(&mut out, SCCP_TON_MAINNET_GLOBAL_ID_V1);
            push_i32(&mut out, SCCP_TON_MASTERCHAIN_WORKCHAIN_V1);
            push_u64(&mut out, SCCP_TON_MASTERCHAIN_SHARD_V1);
            push_u32(&mut out, SCCP_TON_ZERO_STATE_SEQNO_V1);
            out.extend_from_slice(&SCCP_TON_MAINNET_ZERO_STATE_ROOT_HASH_V1);
            out.extend_from_slice(&SCCP_TON_MAINNET_ZERO_STATE_FILE_HASH_V1);
        }
    }
    out
}
/// Hash the canonical V1 identity of an exact SCCP network profile.
#[must_use]
pub fn sccp_network_identity_hash_v1(network: SccpNetworkV1) -> [u8; 32] {
    blake2b256(
        NETWORK_HASH_DOMAIN_V1,
        &canonical_sccp_network_bytes_v1(network),
    )
}
/// Return canonical V1 bytes for a semantically valid directed SCCP lane.
#[must_use]
pub fn canonical_sccp_lane_id_bytes_v1(lane: SccpLaneIdV1) -> Option<Vec<u8>> {
    if !lane.is_well_formed() {
        return None;
    }
    let source = canonical_sccp_network_bytes_v1(lane.source);
    let target = canonical_sccp_network_bytes_v1(lane.target);
    let mut out = Vec::with_capacity(1 + 8 + source.len() + target.len());
    out.push(1);
    push_vec(&mut out, &source);
    push_vec(&mut out, &target);
    Some(out)
}
/// Hash a semantically valid directed SCCP lane.
#[must_use]
pub fn sccp_lane_id_hash_v1(lane: SccpLaneIdV1) -> Option<[u8; 32]> {
    Some(blake2b256(
        LANE_HASH_DOMAIN_V1,
        &canonical_sccp_lane_id_bytes_v1(lane)?,
    ))
}
/// Return canonical V1 bytes for a well-formed typed source emitter.
#[must_use]
pub fn canonical_sccp_source_emitter_bytes_v1(emitter: &SccpSourceEmitterV1) -> Option<Vec<u8>> {
    if !emitter.is_well_formed() {
        return None;
    }
    let mut out = Vec::with_capacity(192);
    out.push(1);
    match emitter {
        SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
            address,
            runtime_code_hash,
            route_config_hash,
        }) => {
            out.push(0);
            out.extend_from_slice(address);
            out.extend_from_slice(runtime_code_hash);
            out.extend_from_slice(route_config_hash);
        }
        SccpSourceEmitterV1::Tron(SccpTronSourceEmitterV1 {
            address,
            runtime_code_hash,
            route_config_hash,
        }) => {
            out.push(1);
            out.extend_from_slice(address);
            out.extend_from_slice(runtime_code_hash);
            out.extend_from_slice(route_config_hash);
        }
        SccpSourceEmitterV1::Ton(SccpTonSourceEmitterV1 {
            address,
            code_hash,
            route_config_hash,
        }) => {
            out.push(2);
            push_ton_registry_address(&mut out, *address);
            out.extend_from_slice(code_hash);
            out.extend_from_slice(route_config_hash);
        }
    }
    Some(out)
}
/// Encode a canonical TON raw address for external interoperability.
///
/// TON Connect and TON raw-address consumers use the signed workchain as an
/// i32 big-endian prefix followed by the 32-byte account id. SCCP registry
/// commitment preimages deliberately use little-endian integers instead; use
/// this function only at the TON wire boundary.
#[must_use]
pub fn canonical_sccp_ton_raw_address_bytes_v1(address: SccpTonAddressV1) -> Option<[u8; 36]> {
    if !address.is_well_formed() {
        return None;
    }
    let mut out = [0_u8; 36];
    out[..4].copy_from_slice(&address.workchain.to_be_bytes());
    out[4..].copy_from_slice(&address.account);
    Some(out)
}
/// Hash a well-formed typed SCCP source emitter.
#[must_use]
pub fn sccp_source_emitter_identity_hash_v1(emitter: &SccpSourceEmitterV1) -> Option<[u8; 32]> {
    Some(blake2b256(
        SOURCE_EMITTER_HASH_DOMAIN_V1,
        &canonical_sccp_source_emitter_bytes_v1(emitter)?,
    ))
}
/// Return canonical V1 bytes for a well-formed inbound SCCP source identity.
#[must_use]
pub fn canonical_sccp_source_identity_bytes_v1(identity: &SccpSourceIdentityV1) -> Option<Vec<u8>> {
    if !identity.is_well_formed() {
        return None;
    }
    let lane = canonical_sccp_lane_id_bytes_v1(identity.lane)?;
    let emitter = canonical_sccp_source_emitter_bytes_v1(&identity.emitter)?;
    let mut out = Vec::with_capacity(1 + 8 + lane.len() + emitter.len());
    out.push(1);
    push_vec(&mut out, &lane);
    push_vec(&mut out, &emitter);
    Some(out)
}
/// Hash a well-formed inbound SCCP source identity.
#[must_use]
pub fn sccp_source_identity_hash_v1(identity: &SccpSourceIdentityV1) -> Option<[u8; 32]> {
    Some(blake2b256(
        SOURCE_IDENTITY_HASH_DOMAIN_V1,
        &canonical_sccp_source_identity_bytes_v1(identity)?,
    ))
}
/// Derive the EVM binding using exactly the Solidity `abi.encode` layout.
///
/// # Errors
///
/// Returns [`SccpRouteValidationError`] when the network is not an EVM SCCP
/// destination or the deployment and proof policy are invalid.
pub fn sccp_evm_destination_binding_hash_v1(
    network: SccpNetworkV1,
    deployment: &SccpEvmDestinationDeploymentV1,
) -> Result<[u8; 32], SccpRouteValidationError> {
    validate_evm_deployment(deployment)?;
    let (target_domain, chain_id) = match network {
        SccpNetworkV1::EthereumMainnet => (SCCP_DOMAIN_ETH, 1),
        SccpNetworkV1::BscMainnet => (SCCP_DOMAIN_BSC, 56),
        _ => return Err(SccpRouteValidationError::DestinationFamilyMismatch),
    };
    let semantic_profile_hash = deployment.outbound_proof_policy.semantic_profile_hash()?;
    let finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()?;
    let mut payload = Vec::with_capacity(32 * 15);
    payload.extend_from_slice(&keccak256(EVM_BINDING_DOMAIN_V1));
    payload.extend_from_slice(&keccak256(EVM_GROTH16_BACKEND_V1));
    payload.extend_from_slice(&abi_word_u64(chain_id));
    payload.extend_from_slice(&abi_word_u32(SCCP_DOMAIN_SORA));
    payload.extend_from_slice(&abi_word_u32(target_domain));
    payload.extend_from_slice(&abi_word_bytes20(deployment.verifier_address));
    payload.extend_from_slice(&abi_word_bytes20(deployment.route_address));
    payload.extend_from_slice(&deployment.verifier_code_hash);
    payload.extend_from_slice(&deployment.verifier_key_hash);
    payload.extend_from_slice(&semantic_profile_hash);
    payload.extend_from_slice(&finality_anchor_hash);
    payload.extend_from_slice(&abi_word_bytes20(deployment.replay_verifier_address));
    payload.extend_from_slice(&deployment.replay_verifier_code_hash);
    payload.extend_from_slice(&abi_word_bytes20(deployment.mint_breaker_address));
    payload.extend_from_slice(&deployment.mint_breaker_code_hash);
    Ok(keccak256(payload))
}
/// Derive the TRON binding using exactly the TVM Solidity `abi.encode` layout.
///
/// # Errors
///
/// Returns [`SccpRouteValidationError`] when the network is not a TRON SCCP
/// destination or the deployment and proof policy are invalid.
pub fn sccp_tron_destination_binding_hash_v1(
    network: SccpNetworkV1,
    deployment: &SccpTronDestinationDeploymentV1,
) -> Result<[u8; 32], SccpRouteValidationError> {
    validate_tron_deployment(deployment)?;
    let network_id = match network {
        SccpNetworkV1::TronMainnet => 0x2b66_53dc,
        _ => return Err(SccpRouteValidationError::DestinationFamilyMismatch),
    };
    let semantic_profile_hash = deployment.outbound_proof_policy.semantic_profile_hash()?;
    let finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()?;
    let mut payload = Vec::with_capacity(32 * 15);
    payload.extend_from_slice(&keccak256(TRON_BINDING_DOMAIN_V1));
    payload.extend_from_slice(&keccak256(TRON_GROTH16_BACKEND_V1));
    payload.extend_from_slice(&abi_word_u32(network_id));
    payload.extend_from_slice(&abi_word_u32(SCCP_DOMAIN_SORA));
    payload.extend_from_slice(&abi_word_u32(SCCP_DOMAIN_TRON));
    payload.extend_from_slice(&abi_word_tron_address(deployment.verifier_address));
    payload.extend_from_slice(&abi_word_tron_address(deployment.route_address));
    payload.extend_from_slice(&deployment.verifier_code_hash);
    payload.extend_from_slice(&deployment.verifier_key_hash);
    payload.extend_from_slice(&semantic_profile_hash);
    payload.extend_from_slice(&finality_anchor_hash);
    payload.extend_from_slice(&abi_word_tron_address(deployment.replay_verifier_address));
    payload.extend_from_slice(&deployment.replay_verifier_code_hash);
    payload.extend_from_slice(&abi_word_tron_address(deployment.mint_breaker_address));
    payload.extend_from_slice(&deployment.mint_breaker_code_hash);
    Ok(keccak256(payload))
}
/// Derive the pre-deployment TON destination binding from immutable code,
/// BLS12-381 Groth16 verifier, proof-profile, and Taira-finality commitments.
///
/// Integers in this registry commitment are little-endian. After the domains,
/// the V1 preimage commits master code/wallet-code, route code, verifier
/// code/circuit/key/proof profile, and semantic/finality policy hashes, in that
/// order. TON contract addresses and actual initial-data roots are deliberately
/// excluded: `StateInit` derives those values from data cells that already store
/// this binding and the route-configuration hash, so feeding them back would
/// require an infeasible cryptographic fixed point. The full governed
/// [`SccpTonDestinationDeploymentV1`] and signed release readback bind both
/// addresses and both exact initial-data roots after deployment.
/// Consequently, this direct primitive helper can be evaluated with placeholder
/// address/data-root fields during `StateInit` construction; the enum-level
/// deployment APIs still require the final nonzero governed values.
///
/// # Errors
///
/// Returns [`SccpRouteValidationError`] when `network` is not an exact TON
/// profile or any governed deployment role is malformed.
pub fn sccp_ton_destination_binding_hash_v1(
    network: SccpNetworkV1,
    deployment: &SccpTonDestinationDeploymentV1,
) -> Result<[u8; 32], SccpRouteValidationError> {
    validate_ton_commitment_primitives(deployment)?;
    let global_id = match network {
        SccpNetworkV1::TonMainnet => SCCP_TON_MAINNET_GLOBAL_ID_V1,
        _ => return Err(SccpRouteValidationError::DestinationFamilyMismatch),
    };
    let semantic_profile_hash = deployment.outbound_proof_policy.semantic_profile_hash()?;
    let finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()?;
    let mut payload = Vec::with_capacity(512);
    payload.extend_from_slice(TON_BINDING_DOMAIN_V1);
    payload.push(1);
    push_vec(&mut payload, TON_GROTH16_BLS12381_BACKEND_V1);
    push_vec(&mut payload, &canonical_sccp_network_bytes_v1(network));
    push_i32(&mut payload, global_id);
    push_u32(&mut payload, SCCP_DOMAIN_SORA);
    push_u32(&mut payload, SCCP_DOMAIN_TON);
    payload.extend_from_slice(&deployment.jetton_master_code_hash);
    payload.extend_from_slice(&deployment.jetton_wallet_code_hash);
    payload.extend_from_slice(&deployment.route_code_hash);
    payload.extend_from_slice(&deployment.embedded_verifier_code_hash);
    payload.extend_from_slice(&deployment.verifier_circuit_hash);
    payload.extend_from_slice(&deployment.verifier_key_hash);
    payload.extend_from_slice(&deployment.proof_profile_commitment);
    for guardian_key in deployment.mint_breaker_guardian_keys.into_array() {
        payload.extend_from_slice(&guardian_key);
    }
    payload.extend_from_slice(&semantic_profile_hash);
    payload.extend_from_slice(&finality_anchor_hash);
    Ok(sha256_bytes(&payload))
}
/// Compute the immutable route-config hash exposed by the exact EVM XOR route.
///
/// # Errors
///
/// Returns [`SccpRouteValidationError`] when the EVM route identity, lane
/// hashes, deployment, proof policy, or revision is invalid.
pub fn sccp_exact_evm_xor_route_config_hash_v1(
    network: SccpNetworkV1,
    source_lane_hash: [u8; 32],
    destination_lane_hash: [u8; 32],
    deployment: &SccpEvmDestinationDeploymentV1,
    route_revision: u32,
) -> Result<[u8; 32], SccpRouteValidationError> {
    validate_evm_deployment(deployment)?;
    if route_revision == 0 {
        return Err(SccpRouteValidationError::InvalidRouteRevision);
    }
    let (domain, network_tag, chain_id, route_id) = match network {
        SccpNetworkV1::EthereumMainnet => (
            SCCP_DOMAIN_ETH,
            u32::from(sccp_network_tag_v1(network)),
            1,
            b"taira_eth_xor".as_slice(),
        ),
        SccpNetworkV1::BscMainnet => (
            SCCP_DOMAIN_BSC,
            u32::from(sccp_network_tag_v1(network)),
            56,
            b"taira_bsc_xor".as_slice(),
        ),
        _ => return Err(SccpRouteValidationError::DestinationFamilyMismatch),
    };
    validate_lane_hash_pair(network, source_lane_hash, destination_lane_hash)?;
    let semantic_profile_hash = deployment.outbound_proof_policy.semantic_profile_hash()?;
    let finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()?;
    validate_hash_roles(&[
        source_lane_hash,
        destination_lane_hash,
        deployment.token_code_hash,
        deployment.verifier_code_hash,
        deployment.replay_verifier_code_hash,
        deployment.mint_breaker_code_hash,
        deployment.verifier_key_hash,
        semantic_profile_hash,
        finality_anchor_hash,
    ])?;
    let mut deployment_config = Vec::with_capacity(32 * 11);
    deployment_config.extend_from_slice(&abi_word_bytes20(deployment.token_address));
    deployment_config.extend_from_slice(&deployment.token_code_hash);
    deployment_config.extend_from_slice(&abi_word_bytes20(deployment.verifier_address));
    deployment_config.extend_from_slice(&deployment.verifier_code_hash);
    deployment_config.extend_from_slice(&deployment.verifier_key_hash);
    deployment_config.extend_from_slice(&semantic_profile_hash);
    deployment_config.extend_from_slice(&finality_anchor_hash);
    deployment_config.extend_from_slice(&abi_word_bytes20(deployment.replay_verifier_address));
    deployment_config.extend_from_slice(&deployment.replay_verifier_code_hash);
    deployment_config.extend_from_slice(&abi_word_bytes20(deployment.mint_breaker_address));
    deployment_config.extend_from_slice(&deployment.mint_breaker_code_hash);
    let deployment_config_hash = keccak256(deployment_config);
    let mut asset_route = Vec::with_capacity(32 * 5);
    asset_route.extend_from_slice(&keccak256(b"xor"));
    asset_route.extend_from_slice(&keccak256(route_id));
    asset_route.extend_from_slice(&abi_word_u32(route_revision));
    asset_route.extend_from_slice(&abi_word_u64(deployment.taira_to_token_multiplier));
    asset_route.extend_from_slice(&abi_word_u128(deployment.max_wrapped_supply));
    let asset_route_config_hash = keccak256(asset_route);
    let mut payload = Vec::with_capacity(32 * 8);
    payload.extend_from_slice(&keccak256(CONCRETE_ROUTE_CONFIG_DOMAIN_V1));
    payload.extend_from_slice(&abi_word_u32(domain));
    payload.extend_from_slice(&abi_word_u32(network_tag));
    payload.extend_from_slice(&abi_word_u64(chain_id));
    payload.extend_from_slice(&source_lane_hash);
    payload.extend_from_slice(&destination_lane_hash);
    payload.extend_from_slice(&deployment_config_hash);
    payload.extend_from_slice(&asset_route_config_hash);
    Ok(keccak256(payload))
}
/// Compute the immutable route-config hash exposed by the exact TRON XOR route.
///
/// # Errors
///
/// Returns [`SccpRouteValidationError`] when the TRON route identity, lane
/// hashes, deployment, proof policy, or revision is invalid.
pub fn sccp_exact_tron_xor_route_config_hash_v1(
    network: SccpNetworkV1,
    source_lane_hash: [u8; 32],
    destination_lane_hash: [u8; 32],
    deployment: &SccpTronDestinationDeploymentV1,
    route_revision: u32,
) -> Result<[u8; 32], SccpRouteValidationError> {
    validate_tron_deployment(deployment)?;
    if route_revision == 0 {
        return Err(SccpRouteValidationError::InvalidRouteRevision);
    }
    let (network_tag, network_id) = match network {
        SccpNetworkV1::TronMainnet => (u32::from(sccp_network_tag_v1(network)), 0x2b66_53dc),
        _ => return Err(SccpRouteValidationError::DestinationFamilyMismatch),
    };
    validate_lane_hash_pair(network, source_lane_hash, destination_lane_hash)?;
    let destination_binding_hash = sccp_tron_destination_binding_hash_v1(network, deployment)?;
    let semantic_profile_hash = deployment.outbound_proof_policy.semantic_profile_hash()?;
    let finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()?;
    validate_hash_roles(&[
        source_lane_hash,
        destination_lane_hash,
        deployment.token_code_hash,
        deployment.verifier_code_hash,
        deployment.replay_verifier_code_hash,
        deployment.mint_breaker_code_hash,
        deployment.verifier_key_hash,
        semantic_profile_hash,
        finality_anchor_hash,
        destination_binding_hash,
    ])?;
    let mut deployment_config = Vec::with_capacity(32 * 12);
    deployment_config.extend_from_slice(&abi_word_bytes20(deployment.token_address));
    deployment_config.extend_from_slice(&deployment.token_code_hash);
    deployment_config.extend_from_slice(&abi_word_bytes20(deployment.verifier_address));
    deployment_config.extend_from_slice(&deployment.verifier_code_hash);
    deployment_config.extend_from_slice(&deployment.verifier_key_hash);
    deployment_config.extend_from_slice(&semantic_profile_hash);
    deployment_config.extend_from_slice(&finality_anchor_hash);
    deployment_config.extend_from_slice(&destination_binding_hash);
    deployment_config.extend_from_slice(&abi_word_bytes20(deployment.replay_verifier_address));
    deployment_config.extend_from_slice(&deployment.replay_verifier_code_hash);
    deployment_config.extend_from_slice(&abi_word_bytes20(deployment.mint_breaker_address));
    deployment_config.extend_from_slice(&deployment.mint_breaker_code_hash);
    let deployment_config_hash = keccak256(deployment_config);
    let mut asset_route = Vec::with_capacity(32 * 5);
    asset_route.extend_from_slice(&keccak256(b"xor"));
    asset_route.extend_from_slice(&keccak256(b"taira_tron_xor"));
    asset_route.extend_from_slice(&abi_word_u32(route_revision));
    asset_route.extend_from_slice(&abi_word_u64(deployment.taira_to_token_multiplier));
    asset_route.extend_from_slice(&abi_word_u128(deployment.max_wrapped_supply));
    let asset_route_config_hash = keccak256(asset_route);
    let mut payload = Vec::with_capacity(32 * 8);
    payload.extend_from_slice(&keccak256(CONCRETE_ROUTE_CONFIG_DOMAIN_V1));
    payload.extend_from_slice(&abi_word_u32(SCCP_DOMAIN_TRON));
    payload.extend_from_slice(&abi_word_u32(network_tag));
    payload.extend_from_slice(&abi_word_u32(network_id));
    payload.extend_from_slice(&source_lane_hash);
    payload.extend_from_slice(&destination_lane_hash);
    payload.extend_from_slice(&deployment_config_hash);
    payload.extend_from_slice(&asset_route_config_hash);
    Ok(keccak256(payload))
}
/// Compute the immutable route-config hash for an exact TON XOR route.
///
/// The SHA-256 preimage is an explicit little-endian SCCP registry encoding.
/// It commits the TON zero-state-selected profile transitively through the
/// network tag/global id, both directional lanes, every destination contract
/// code identity, the BLS12-381 verifier circuit/key/profile, and the governed
/// Taira semantic statement and finality anchor. Contract addresses and actual
/// initial-data roots remain in the governed deployment object but are omitted
/// from this pre-deployment hash to avoid a `StateInit` cryptographic fixed point.
/// This direct helper therefore depends only on the primitive fields even when
/// its deployment argument still carries placeholder post-StateInit values;
/// enum-level route validation requires the final governed values.
///
/// # Errors
///
/// Returns [`SccpRouteValidationError`] when the TON route identity, lane
/// hashes, deployment, proof policy, or revision is invalid.
pub fn sccp_exact_ton_xor_route_config_hash_v1(
    network: SccpNetworkV1,
    source_lane_hash: [u8; 32],
    destination_lane_hash: [u8; 32],
    deployment: &SccpTonDestinationDeploymentV1,
    route_revision: u32,
) -> Result<[u8; 32], SccpRouteValidationError> {
    validate_ton_commitment_primitives(deployment)?;
    if route_revision == 0 {
        return Err(SccpRouteValidationError::InvalidRouteRevision);
    }
    let global_id = match network {
        SccpNetworkV1::TonMainnet => SCCP_TON_MAINNET_GLOBAL_ID_V1,
        _ => return Err(SccpRouteValidationError::DestinationFamilyMismatch),
    };
    validate_lane_hash_pair(network, source_lane_hash, destination_lane_hash)?;
    let destination_binding_hash = sccp_ton_destination_binding_hash_v1(network, deployment)?;
    let semantic_profile_hash = deployment.outbound_proof_policy.semantic_profile_hash()?;
    let finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()?;
    validate_hash_roles(&[
        source_lane_hash,
        destination_lane_hash,
        deployment.jetton_master_code_hash,
        deployment.jetton_wallet_code_hash,
        deployment.route_code_hash,
        deployment.embedded_verifier_code_hash,
        deployment.verifier_circuit_hash,
        deployment.verifier_key_hash,
        deployment.proof_profile_commitment,
        semantic_profile_hash,
        finality_anchor_hash,
        destination_binding_hash,
    ])?;
    let mut deployment_config = Vec::with_capacity(640);
    deployment_config.extend_from_slice(&deployment.jetton_master_code_hash);
    deployment_config.extend_from_slice(&deployment.jetton_wallet_code_hash);
    deployment_config.extend_from_slice(&deployment.route_code_hash);
    deployment_config.extend_from_slice(&deployment.embedded_verifier_code_hash);
    deployment_config.extend_from_slice(&deployment.verifier_circuit_hash);
    deployment_config.extend_from_slice(&deployment.verifier_key_hash);
    deployment_config.extend_from_slice(&deployment.proof_profile_commitment);
    for guardian_key in deployment.mint_breaker_guardian_keys.into_array() {
        deployment_config.extend_from_slice(&guardian_key);
    }
    deployment_config.extend_from_slice(&semantic_profile_hash);
    deployment_config.extend_from_slice(&finality_anchor_hash);
    deployment_config.extend_from_slice(&destination_binding_hash);
    let deployment_config_hash = sha256_bytes(&deployment_config);
    let mut asset_route = Vec::with_capacity(64);
    push_vec(&mut asset_route, b"xor");
    push_vec(&mut asset_route, b"taira_ton_xor");
    push_u32(&mut asset_route, route_revision);
    push_u64(&mut asset_route, deployment.taira_to_token_multiplier);
    push_u128(&mut asset_route, deployment.max_wrapped_supply);
    let asset_route_config_hash = sha256_bytes(&asset_route);
    let mut payload = Vec::with_capacity(256);
    payload.extend_from_slice(CONCRETE_ROUTE_CONFIG_DOMAIN_V1);
    payload.push(1);
    push_u32(&mut payload, SCCP_DOMAIN_TON);
    push_vec(&mut payload, &canonical_sccp_network_bytes_v1(network));
    push_i32(&mut payload, global_id);
    payload.extend_from_slice(&source_lane_hash);
    payload.extend_from_slice(&destination_lane_hash);
    payload.extend_from_slice(&deployment_config_hash);
    payload.extend_from_slice(&asset_route_config_hash);
    Ok(sha256_bytes(&payload))
}
fn validate_inbound_lane(lane: SccpLaneIdV1) -> Result<(), SccpRouteValidationError> {
    if !lane.is_well_formed() || !lane.source.is_external() || !lane.target.is_sora() {
        return Err(SccpRouteValidationError::InvalidInboundLane);
    }
    Ok(())
}
fn validate_key(label: &'static str, value: &str) -> Result<(), SccpRouteValidationError> {
    let bytes = value.as_bytes();
    let valid = !bytes.is_empty()
        && bytes.len() <= SCCP_V1_MAX_KEY_BYTES
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(*byte, b'_' | b'-')
        });
    if !valid {
        return Err(SccpRouteValidationError::NonCanonicalKey(label));
    }
    Ok(())
}
fn validate_concrete_route_identity(
    network: SccpNetworkV1,
    route_id: &str,
    asset_key: &str,
    payload_amount_scale: u32,
) -> Result<(), SccpRouteValidationError> {
    validate_key("route_id", route_id)?;
    validate_key("asset_key", asset_key)?;
    let expected_route = match network {
        SccpNetworkV1::EthereumMainnet => "taira_eth_xor",
        SccpNetworkV1::BscMainnet => "taira_bsc_xor",
        SccpNetworkV1::TronMainnet => "taira_tron_xor",
        SccpNetworkV1::TonMainnet => "taira_ton_xor",
        _ => return Err(SccpRouteValidationError::DestinationFamilyMismatch),
    };
    if route_id != expected_route
        || asset_key != "xor"
        || payload_amount_scale != SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE
    {
        return Err(SccpRouteValidationError::ConcreteRouteMismatch);
    }
    Ok(())
}
fn validate_evm_deployment(
    deployment: &SccpEvmDestinationDeploymentV1,
) -> Result<(), SccpRouteValidationError> {
    if deployment.taira_to_token_multiplier != SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER
        || deployment.max_wrapped_supply == 0
    {
        return Err(SccpRouteValidationError::ConcreteRouteMismatch);
    }
    validate_nonzero("token_address", &deployment.token_address)?;
    validate_nonzero("verifier_address", &deployment.verifier_address)?;
    validate_nonzero("route_address", &deployment.route_address)?;
    validate_nonzero(
        "replay_verifier_address",
        &deployment.replay_verifier_address,
    )?;
    validate_nonzero("mint_breaker_address", &deployment.mint_breaker_address)?;
    validate_distinct(&[
        deployment.token_address,
        deployment.verifier_address,
        deployment.route_address,
        deployment.replay_verifier_address,
        deployment.mint_breaker_address,
    ])?;
    validate_runtime_code_hash("token_code_hash", &deployment.token_code_hash)?;
    validate_runtime_code_hash("verifier_code_hash", &deployment.verifier_code_hash)?;
    validate_runtime_code_hash(
        "replay_verifier_code_hash",
        &deployment.replay_verifier_code_hash,
    )?;
    validate_runtime_code_hash("mint_breaker_code_hash", &deployment.mint_breaker_code_hash)?;
    validate_runtime_code_hash("route_code_hash", &deployment.route_code_hash)?;
    let derived_key_hash = sccp_groth16_bn254_verifying_key_hash_v1(deployment.verifying_key)?;
    if derived_key_hash != deployment.verifier_key_hash {
        return Err(SccpRouteValidationError::Groth16VerifyingKeyHashMismatch);
    }
    deployment.outbound_proof_policy.validate()?;
    if !deployment.outbound_proof_policy.semantic_profile.is_bn254() {
        return Err(SccpRouteValidationError::InvalidOutboundProofPolicy);
    }
    let semantic_profile_hash = deployment.outbound_proof_policy.semantic_profile_hash()?;
    let finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()?;
    validate_hash_roles(&[
        deployment.token_code_hash,
        deployment.verifier_code_hash,
        deployment.replay_verifier_code_hash,
        deployment.mint_breaker_code_hash,
        deployment.verifier_key_hash,
        deployment.route_code_hash,
        semantic_profile_hash,
        finality_anchor_hash,
    ])
}
fn validate_tron_deployment(
    deployment: &SccpTronDestinationDeploymentV1,
) -> Result<(), SccpRouteValidationError> {
    if deployment.taira_to_token_multiplier != SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER
        || deployment.max_wrapped_supply == 0
    {
        return Err(SccpRouteValidationError::ConcreteRouteMismatch);
    }
    validate_nonzero("token_address", &deployment.token_address)?;
    validate_nonzero("verifier_address", &deployment.verifier_address)?;
    validate_nonzero("route_address", &deployment.route_address)?;
    validate_nonzero(
        "replay_verifier_address",
        &deployment.replay_verifier_address,
    )?;
    validate_nonzero("mint_breaker_address", &deployment.mint_breaker_address)?;
    validate_distinct(&[
        deployment.token_address,
        deployment.verifier_address,
        deployment.route_address,
        deployment.replay_verifier_address,
        deployment.mint_breaker_address,
    ])?;
    validate_runtime_code_hash("token_code_hash", &deployment.token_code_hash)?;
    validate_runtime_code_hash("verifier_code_hash", &deployment.verifier_code_hash)?;
    validate_runtime_code_hash(
        "replay_verifier_code_hash",
        &deployment.replay_verifier_code_hash,
    )?;
    validate_runtime_code_hash("mint_breaker_code_hash", &deployment.mint_breaker_code_hash)?;
    validate_runtime_code_hash("route_code_hash", &deployment.route_code_hash)?;
    let derived_key_hash = sccp_groth16_bn254_verifying_key_hash_v1(deployment.verifying_key)?;
    if derived_key_hash != deployment.verifier_key_hash {
        return Err(SccpRouteValidationError::Groth16VerifyingKeyHashMismatch);
    }
    deployment.outbound_proof_policy.validate()?;
    if !deployment.outbound_proof_policy.semantic_profile.is_bn254() {
        return Err(SccpRouteValidationError::InvalidOutboundProofPolicy);
    }
    let semantic_profile_hash = deployment.outbound_proof_policy.semantic_profile_hash()?;
    let finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()?;
    validate_hash_roles(&[
        deployment.token_code_hash,
        deployment.verifier_code_hash,
        deployment.replay_verifier_code_hash,
        deployment.mint_breaker_code_hash,
        deployment.verifier_key_hash,
        deployment.route_code_hash,
        semantic_profile_hash,
        finality_anchor_hash,
    ])
}
fn validate_ton_deployment(
    deployment: &SccpTonDestinationDeploymentV1,
) -> Result<(), SccpRouteValidationError> {
    validate_ton_commitment_primitives(deployment)?;
    let addresses = [deployment.jetton_master_address, deployment.route_address];
    if addresses
        .iter()
        .any(|address| !address.is_sccp_basechain_contract())
    {
        return Err(SccpRouteValidationError::InvalidTonAddress);
    }
    validate_distinct(&addresses)?;
    let semantic_profile_hash = deployment.outbound_proof_policy.semantic_profile_hash()?;
    let finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()?;
    validate_hash_roles(&[
        deployment.jetton_master_code_hash,
        deployment.jetton_master_initial_data_hash,
        deployment.jetton_wallet_code_hash,
        deployment.route_code_hash,
        deployment.route_initial_data_hash,
        deployment.embedded_verifier_code_hash,
        deployment.verifier_circuit_hash,
        deployment.verifier_key_hash,
        deployment.proof_profile_commitment,
        semantic_profile_hash,
        finality_anchor_hash,
    ])
}
fn validate_ton_commitment_primitives(
    deployment: &SccpTonDestinationDeploymentV1,
) -> Result<(), SccpRouteValidationError> {
    if deployment.taira_to_token_multiplier != SCCP_V1_TAIRA_TO_TON_TOKEN_MULTIPLIER {
        return Err(SccpRouteValidationError::ConcreteRouteMismatch);
    }
    if deployment.max_wrapped_supply == 0 || deployment.max_wrapped_supply > SCCP_V1_TON_MAX_COINS {
        return Err(SccpRouteValidationError::InvalidTonWrappedSupplyCap);
    }
    let guardian_keys = deployment.mint_breaker_guardian_keys.into_array();
    if guardian_keys.contains(&[0; 32]) || guardian_keys.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(SccpRouteValidationError::InvalidTonMintBreakerGuardians);
    }
    let derived_key_hash = sccp_groth16_bls12381_verifying_key_hash_v1(deployment.verifying_key)?;
    if derived_key_hash != deployment.verifier_key_hash {
        return Err(SccpRouteValidationError::Groth16VerifyingKeyHashMismatch);
    }
    deployment.outbound_proof_policy.validate()?;
    if !deployment
        .outbound_proof_policy
        .semantic_profile
        .is_bls12381()
    {
        return Err(SccpRouteValidationError::InvalidOutboundProofPolicy);
    }
    let SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bls12381(circuit) =
        deployment.outbound_proof_policy.semantic_profile
    else {
        return Err(SccpRouteValidationError::InvalidOutboundProofPolicy);
    };
    if deployment.verifier_circuit_hash != circuit.circuit_commitment
        || deployment.proof_profile_commitment
            != sccp_ton_groth16_bls12381_proof_profile_commitment_v1()
    {
        return Err(SccpRouteValidationError::InvalidTonProofCommitments);
    }
    let semantic_profile_hash = deployment.outbound_proof_policy.semantic_profile_hash()?;
    let finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()?;
    validate_hash_roles(&[
        deployment.jetton_master_code_hash,
        deployment.jetton_wallet_code_hash,
        deployment.route_code_hash,
        deployment.embedded_verifier_code_hash,
        deployment.verifier_circuit_hash,
        deployment.verifier_key_hash,
        deployment.proof_profile_commitment,
        semantic_profile_hash,
        finality_anchor_hash,
    ])
}
fn source_matches_destination(
    source: SccpSourceEmitterV1,
    destination: &SccpDestinationDeploymentV1,
    route_config_hash: [u8; 32],
) -> bool {
    match (source, destination) {
        (
            SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                address,
                runtime_code_hash,
                route_config_hash: source_route_config_hash,
            }),
            SccpDestinationDeploymentV1::Evm(deployment),
        ) => {
            address == deployment.route_address
                && runtime_code_hash == deployment.route_code_hash
                && source_route_config_hash == route_config_hash
        }
        (
            SccpSourceEmitterV1::Tron(SccpTronSourceEmitterV1 {
                address,
                runtime_code_hash,
                route_config_hash: source_route_config_hash,
            }),
            SccpDestinationDeploymentV1::Tron(deployment),
        ) => {
            address == deployment.route_address
                && runtime_code_hash == deployment.route_code_hash
                && source_route_config_hash == route_config_hash
        }
        (SccpSourceEmitterV1::Ton(source), SccpDestinationDeploymentV1::Ton(deployment)) => {
            let Ok(semantic_profile_hash) =
                deployment.outbound_proof_policy.semantic_profile_hash()
            else {
                return false;
            };
            let Ok(finality_anchor_hash) =
                deployment.outbound_proof_policy.sora_finality_anchor_hash()
            else {
                return false;
            };
            source.address == deployment.route_address
                && source.code_hash == deployment.route_code_hash
                && source.route_config_hash == route_config_hash
                && validate_distinct(&[deployment.jetton_master_address, deployment.route_address])
                    .is_ok()
                && validate_hash_roles(&[
                    source.route_config_hash,
                    deployment.jetton_master_code_hash,
                    deployment.jetton_master_initial_data_hash,
                    deployment.jetton_wallet_code_hash,
                    deployment.route_code_hash,
                    deployment.route_initial_data_hash,
                    deployment.embedded_verifier_code_hash,
                    deployment.verifier_circuit_hash,
                    deployment.verifier_key_hash,
                    deployment.proof_profile_commitment,
                    semantic_profile_hash,
                    finality_anchor_hash,
                ])
                .is_ok()
        }
        _ => false,
    }
}
fn native_backend_matches_family(
    backend: BridgeNativeProofBackendV1,
    network: SccpNetworkV1,
) -> bool {
    matches!(
        (backend, network),
        (
            BridgeNativeProofBackendV1::EthereumBeacon,
            SccpNetworkV1::EthereumMainnet
        ) | (
            BridgeNativeProofBackendV1::BscParlia,
            SccpNetworkV1::BscMainnet
        ) | (
            BridgeNativeProofBackendV1::TronDpos,
            SccpNetworkV1::TronMainnet
        ) | (
            BridgeNativeProofBackendV1::TonMasterchain,
            SccpNetworkV1::TonMainnet
        )
    )
}
fn validate_lane_hash_pair(
    network: SccpNetworkV1,
    source_lane_hash: [u8; 32],
    destination_lane_hash: [u8; 32],
) -> Result<(), SccpRouteValidationError> {
    let expected_source = sccp_lane_id_hash_v1(SccpLaneIdV1 {
        source: network,
        target: SccpNetworkV1::SoraTaira,
    })
    .ok_or(SccpRouteValidationError::InvalidInboundLane)?;
    let expected_destination = sccp_lane_id_hash_v1(SccpLaneIdV1 {
        source: SccpNetworkV1::SoraTaira,
        target: network,
    })
    .ok_or(SccpRouteValidationError::InvalidInboundLane)?;
    if source_lane_hash != expected_source || destination_lane_hash != expected_destination {
        return Err(SccpRouteValidationError::ConcreteRouteMismatch);
    }
    Ok(())
}
fn validate_hash_roles(values: &[[u8; 32]]) -> Result<(), SccpRouteValidationError> {
    for value in values {
        validate_nonzero("hash", value)?;
    }
    validate_distinct(values)
}
fn validate_nonzero<const N: usize>(
    label: &'static str,
    value: &[u8; N],
) -> Result<(), SccpRouteValidationError> {
    if value.iter().all(|byte| *byte == 0) {
        return Err(SccpRouteValidationError::ZeroRole(label));
    }
    Ok(())
}
fn validate_runtime_code_hash(
    label: &'static str,
    value: &[u8; 32],
) -> Result<(), SccpRouteValidationError> {
    validate_nonzero(label, value)?;
    if *value == KECCAK256_EMPTY_BYTES {
        return Err(SccpRouteValidationError::EmptyRuntimeCode(label));
    }
    Ok(())
}
fn validate_distinct<T: PartialEq>(values: &[T]) -> Result<(), SccpRouteValidationError> {
    if values
        .iter()
        .enumerate()
        .any(|(index, value)| values[index + 1..].contains(value))
    {
        return Err(SccpRouteValidationError::RoleAlias);
    }
    Ok(())
}
fn abi_word_u32(value: u32) -> [u8; 32] {
    let mut word = [0u8; 32];
    word[28..].copy_from_slice(&value.to_be_bytes());
    word
}
fn abi_word_u64(value: u64) -> [u8; 32] {
    let mut word = [0u8; 32];
    word[24..].copy_from_slice(&value.to_be_bytes());
    word
}
fn abi_word_u128(value: u128) -> [u8; 32] {
    let mut word = [0u8; 32];
    word[16..].copy_from_slice(&value.to_be_bytes());
    word
}
fn abi_word_bytes20(value: [u8; 20]) -> [u8; 32] {
    let mut word = [0u8; 32];
    word[12..].copy_from_slice(&value);
    word
}
fn abi_word_tron_address(value: [u8; 20]) -> [u8; 32] {
    let mut word = [0u8; 32];
    word[11] = 0x41;
    word[12..].copy_from_slice(&value);
    word
}
fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_u128(out: &mut Vec<u8>, value: u128) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_vec(out: &mut Vec<u8>, value: &[u8]) {
    let len = u32::try_from(value.len()).expect("bounded SCCP field length fits u32");
    push_u32(out, len);
    out.extend_from_slice(value);
}
fn push_ton_registry_address(out: &mut Vec<u8>, address: SccpTonAddressV1) {
    push_i32(out, address.workchain);
    out.extend_from_slice(&address.account);
}
fn bls12381_g1_compressed_is_structurally_canonical(point: &[u8; 48]) -> bool {
    if point[0] & 0x80 == 0 || point[0] & 0x40 != 0 {
        return false;
    }
    let mut x = *point;
    x[0] &= 0x1f;
    x < BLS12381_BASE_FIELD_MODULUS_BE
}
fn bls12381_g2_compressed_is_structurally_canonical(point: &[u8; 96]) -> bool {
    let mut first = [0_u8; 48];
    first.copy_from_slice(&point[..48]);
    if !bls12381_g1_compressed_is_structurally_canonical(&first) {
        return false;
    }
    let mut second = [0_u8; 48];
    second.copy_from_slice(&point[48..]);
    second < BLS12381_BASE_FIELD_MODULUS_BE
}
fn sha256_bytes(payload: &[u8]) -> [u8; 32] {
    Sha256::digest(payload).into()
}
fn blake2b256(prefix: &[u8], payload: &[u8]) -> [u8; 32] {
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(prefix);
    hasher.update(payload);
    hasher.finalize().into()
}
#[cfg(test)]
mod tests;

#[cfg(test)]
mod captured_sccp_registry_schema_tests;
