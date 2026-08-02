//! Exact first-release SCCP route-registry wire types and commitments.
//!
//! A governed route is one atomic consensus object. It contains only closed,
//! typed protocol identity. Operator checklists, URLs, RPC observations,
//! executable blobs, prover packages, and deployment logs are deliberately not
//! consensus state.

use std::collections::{BTreeMap, BTreeSet};

use blake2::{Blake2b, Digest as _, digest::consts::U32};
use iroha_crypto::keccak256;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use sha2::Sha256;
use thiserror::Error;

use super::{
    BridgeNativeProofBackendV1, SCCP_SOLANA_TESTNET_GENESIS_HASH_V1, SccpEvmSourceEmitterV1,
    SccpLaneIdV1, SccpNativeTrustAnchorV1, SccpNetworkV1, SccpSolanaSourceEmitterV1,
    SccpSourceEmitterV1, SccpSourceIdentityV1, SccpTronSourceEmitterV1,
};
use crate::{account::AccountId, asset::AssetDefinitionId, block::consensus_v2::PROTOCOL_VERSION};

/// Maximum decimal scale accepted by first-release SCCP amount payloads.
pub const SCCP_V1_MAX_PAYLOAD_AMOUNT_SCALE: u32 = 28;
/// Exact decimal scale of the first-release XOR SCCP payload.
pub const SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE: u32 = 9;
/// Maximum number of nonterminal routes in the V1 registry.
///
/// Terminal revisions are immutable history needed to authenticate messages
/// emitted before a deployment rotation. They deliberately do not consume
/// this live-governance budget; a separate generous retained-history bound
/// keeps the full state and registry response finite.
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
/// For a single-lineage lane, sixty-four revisions provide more than five
/// years of monthly deployment rotation. History is never evicted implicitly;
/// governance must stop before this shared lane bound and operators must plan
/// an explicit first-release migration. A fixed-shape admitted V1 route fits a
/// conservative 4 KiB canonical encoding envelope.
pub const SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE: usize = 64;
/// Maximum retained native trust anchors sharing one governed lane.
///
/// At one governed rotation per day, 4,096 checkpoints cover more than eleven
/// years. A checkpoint fits a conservative 64-byte canonical encoding
/// envelope; together with the route and 16-lane caps, retained entry payloads
/// are bounded by 8 MiB before small vector/lane framing overhead.
pub const SCCP_V1_MAX_RETAINED_NATIVE_TRUST_ANCHORS_PER_LANE: usize = 4_096;
/// Maximum byte length of a canonical SCCP route or asset key.
pub const SCCP_V1_MAX_KEY_BYTES: usize = 64;
/// Exact Taira(9-decimal) to wrapped-token(18-decimal) multiplier.
pub const SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER: u64 = 1_000_000_000;
/// Exact Taira(9-decimal) to SPL-token(9-decimal) multiplier.
///
/// SPL amounts are `u64`; using 18 decimals would cap the entire mint at about
/// 18.45 XOR. Solana therefore preserves Taira's nine-decimal base unit 1:1.
pub const SCCP_V1_TAIRA_TO_SOLANA_TOKEN_MULTIPLIER: u64 = 1;
/// Exact first-release SORA-side IVM semantics selected by route governance.
pub const SCCP_V1_SORA_OUTBOUND_EXECUTION_SEMANTICS: &str = "ivm_proved_record_sccp_message_v1";
/// Fixed upper bound for one governed SORA-side outbound IVM execution.
pub const SCCP_V1_MAX_SORA_OUTBOUND_GAS_LIMIT: u64 = 1_000_000_000;
/// Canonical live Taira XOR asset definition governed by every V1 route.
pub const SCCP_V1_TAIRA_XOR_ASSET_DEFINITION_ID: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";

const SCCP_DOMAIN_SORA: u32 = 0;
const SCCP_DOMAIN_ETH: u32 = 1;
const SCCP_DOMAIN_BSC: u32 = 2;
const SCCP_DOMAIN_SOLANA: u32 = 3;
const SCCP_DOMAIN_TRON: u32 = 5;

const EVM_BINDING_DOMAIN_V1: &[u8] = b"iroha:sccp:evm-destination-binding:v1";
const TRON_BINDING_DOMAIN_V1: &[u8] = b"iroha:sccp:tron-destination-binding:v1";
const SOLANA_BINDING_DOMAIN_V1: &[u8] = b"iroha:sccp:solana-destination-binding:v1";
const SOLANA_NATIVE_VERIFIER_CONFIG_DOMAIN_V1: &[u8] = b"sccp:solana:verifier-config:v1";
const CONCRETE_ROUTE_CONFIG_DOMAIN_V1: &[u8] = b"sccp:concrete-route-config:v1";
const NETWORK_HASH_DOMAIN_V1: &[u8] = b"sccp:network-identity:v1";
const LANE_HASH_DOMAIN_V1: &[u8] = b"sccp:lane-id:v1";
const SOURCE_EMITTER_HASH_DOMAIN_V1: &[u8] = b"sccp:source-emitter-identity:v1";
const SOURCE_IDENTITY_HASH_DOMAIN_V1: &[u8] = b"sccp:source-identity:v1";
const SEMANTIC_PROOF_PROFILE_HASH_DOMAIN_V1: &[u8] = b"sccp:semantic-proof-profile:v1";
const SORA_FINALITY_ANCHOR_HASH_DOMAIN_V1: &[u8] = b"sccp:sora-finality-anchor:v1";
const GROTH16_PUBLIC_SIGNAL_SCHEMA_HASH_DOMAIN_V1: &[u8] =
    b"sccp:groth16-bn254:public-signal-schema:v1";
const EVM_GROTH16_BACKEND_V1: &[u8] = b"evm-groth16-bn254-v1";
const TRON_GROTH16_BACKEND_V1: &[u8] = b"tron-groth16-bn254-v1";
const SOLANA_GROTH16_BACKEND_V1: &[u8] = b"solana-groth16-bn254-v1";
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

/// BN254 base-field modulus in canonical big-endian form.
const BN254_BASE_FIELD_MODULUS_BE: [u8; 32] = [
    0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
    0x97, 0x81, 0x6a, 0x91, 0x68, 0x71, 0xca, 0x8d, 0x3c, 0x20, 0x8c, 0x16, 0xd8, 0x7c, 0xfd, 0x47,
];

const SORA_TAIRA_CHAIN_ID_BYTES: [u8; 16] = [
    0xfc, 0x56, 0x98, 0x4b, 0x2b, 0xe7, 0x43, 0x1d, 0x84, 0x0e, 0x21, 0x51, 0x4d, 0x18, 0x83, 0xf0,
];
const SOLANA_CLASSIC_TOKEN_PROGRAM_ID: [u8; 32] = [
    6, 221, 246, 225, 215, 101, 161, 147, 217, 203, 225, 70, 206, 235, 121, 172, 28, 180, 133, 237,
    95, 91, 55, 145, 58, 140, 245, 133, 126, 255, 0, 169,
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
    /// A fixed Groth16 key has the wrong version or a non-canonical coordinate.
    #[error("SCCP Groth16 BN254 verification key is not structurally canonical")]
    InvalidGroth16VerifyingKey,
    /// The full governed key does not match the Solidity verifier commitment.
    #[error("SCCP Groth16 verification-key hash does not match the embedded key")]
    Groth16VerifyingKeyHashMismatch,
    /// The Solana material config is not the one-way exact primitive commitment.
    #[error("SCCP Solana native-verifier config hash does not match the exact V1 preimage")]
    SolanaNativeVerifierConfigMismatch,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
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

/// Immutable commitments identifying one audited semantic Groth16 circuit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
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

/// Closed semantic proof profile accepted by first-release outbound routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[norito(tag = "profile", content = "commitments")]
pub enum SccpSemanticProofProfileV1 {
    /// Groth16 proof of canonical payload semantics, message inclusion, and
    /// Taira finality rooted in the governed SORA checkpoint.
    #[codec(index = 0)]
    #[norito(rename = "sora_taira_finality_inclusion_groth16_bn254")]
    SoraTairaFinalityInclusionGroth16Bn254(SccpGroth16Bn254SemanticCircuitV1),
}

impl SccpSemanticProofProfileV1 {
    /// Validate the closed profile and exact ordered public-signal schema.
    ///
    /// # Errors
    ///
    /// Returns [`SccpRouteValidationError::InvalidSemanticProofProfile`] when
    /// the profile version, signal schema, or commitment roles are invalid.
    pub fn validate(self) -> Result<(), SccpRouteValidationError> {
        let Self::SoraTairaFinalityInclusionGroth16Bn254(circuit) = self;
        if circuit.version != 1
            || circuit.public_signal_schema_hash
                != sccp_groth16_bn254_public_signal_schema_hash_v1()
            || validate_hash_roles(&[
                circuit.circuit_commitment,
                circuit.witness_generator_commitment,
                circuit.public_signal_schema_hash,
            ])
            .is_err()
        {
            return Err(SccpRouteValidationError::InvalidSemanticProofProfile);
        }
        Ok(())
    }

    /// Return the fixed circuit commitments in protocol-role order.
    #[must_use]
    pub const fn commitments(self) -> [[u8; 32]; 3] {
        let Self::SoraTairaFinalityInclusionGroth16Bn254(circuit) = self;
        [
            circuit.circuit_commitment,
            circuit.witness_generator_commitment,
            circuit.public_signal_schema_hash,
        ]
    }
}

/// Immutable Taira checkpoint anchoring one governed outbound proof policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpSoraFinalityAnchorV1 {
    /// Anchor schema version. SCCP V1 requires `1`.
    pub version: u8,
    /// Exact source chain. SCCP V1 outbound proofs require SORA Taira.
    pub source_network: SccpNetworkV1,
    /// Exact authoritative Sumeragi wire protocol. SCCP V1 requires revision 3.
    pub protocol_version: u16,
    /// Keccak-256 of the canonical 16-byte Taira chain identifier.
    pub chain_id_hash: [u8; 32],
    /// Nonzero finalized checkpoint height.
    pub checkpoint_height: u64,
    /// Hash of the canonical finalized checkpoint block header.
    pub checkpoint_block_hash: [u8; 32],
    /// Immutable Sumeragi-v2 height-context identifier at the checkpoint.
    pub checkpoint_context_id: [u8; 32],
    /// Domain-separated hash of the canonical durable v2 finality artifact.
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
            || self.protocol_version != PROTOCOL_VERSION
            || self.chain_id_hash != sccp_sora_taira_chain_id_hash_v1()
            || self.checkpoint_height == 0
            || validate_hash_roles(&[
                self.chain_id_hash,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[norito(tag = "activation", content = "direction")]
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
/// An external event whose fully verified consensus-progress coordinate is at
/// or below `max_anchor_interval_height` remains redeemable after retirement.
/// Events above it are rejected, so a retired emitter cannot create new claims
/// indefinitely. `trust_anchor_hash` binds the cutoff to a complete retained
/// checkpoint interval; the maximum must equal that anchor's successor
/// checkpoint and an open-ended current anchor cannot be retired against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpInboundFinalityCutoffV1 {
    /// Retained lane checkpoint whose validity interval contains the cutoff.
    pub trust_anchor_hash: [u8; 32],
    /// Greatest authenticated backend-specific consensus-progress coordinate admitted.
    ///
    /// Ethereum lanes use a finalized beacon slot. BSC and TRON lanes use a
    /// finalized block height.
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
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
    /// Returns [`SccpRouteValidationError`] when the lane, identifiers, or
    /// revision are invalid.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
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
    /// Exact Taira base-unit to wrapped-token base-unit multiplier.
    pub taira_to_token_multiplier: u64,
}

/// Exact TRON verifier, route, and TRC-20 deployment identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
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
    /// Exact Taira base-unit to wrapped-token base-unit multiplier.
    pub taira_to_token_multiplier: u64,
}

/// Exact immutable Solana route and native-verifier deployment identity.
///
/// All account identities are raw 32-byte Solana public keys. The route and
/// native verifier are distinct programs with independently pinned `ProgramData`,
/// state/material, code, configuration, and verification-key roles. Reusing a
/// program or `ProgramData` account across those trust boundaries is rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpSolanaDestinationDeploymentV1 {
    /// SPL mint public key governed by the route.
    pub token_mint_address: [u8; 32],
    /// Immutable value-moving route program public key.
    pub route_program_id: [u8; 32],
    /// Loader-v3 `ProgramData` account for the route executable.
    pub route_program_data_address: [u8; 32],
    /// Nonzero deployment slot of the reviewed route `ProgramData` revision.
    pub route_program_data_slot: u64,
    /// Program-owned route-state account public key.
    pub route_state_account: [u8; 32],
    /// Blake2b-256 hash of the immutable route `ProgramData` executable bytes.
    pub route_program_code_hash: [u8; 32],
    /// Native recursive verifier program public key.
    pub native_verifier_program_id: [u8; 32],
    /// Loader-v3 `ProgramData` account for the native verifier executable.
    pub native_verifier_program_data_address: [u8; 32],
    /// Nonzero deployment slot of the reviewed verifier `ProgramData` revision.
    pub native_verifier_program_data_slot: u64,
    /// Program-owned sealed verification-material account public key.
    ///
    /// This role holds the governed VK/config material and is distinct from a
    /// generic mutable program state account.
    pub native_verifier_material_account: [u8; 32],
    /// Blake2b-256 hash of the immutable native-verifier `ProgramData` bytes.
    pub native_verifier_program_code_hash: [u8; 32],
    /// Commitment to the exact native-verifier runtime configuration.
    pub native_verifier_config_hash: [u8; 32],
    /// Full fixed BN254 verification key consumed by the governed verifier.
    pub verifying_key: SccpGroth16Bn254VerifyingKeyV1,
    /// Canonical commitment to [`Self::verifying_key`].
    pub verifier_key_hash: [u8; 32],
    /// Immutable audited semantic circuit and governed SORA finality anchor.
    pub outbound_proof_policy: SccpOutboundProofPolicyV1,
    /// Exact Taira base-unit to SPL-token base-unit multiplier.
    pub taira_to_token_multiplier: u64,
}

/// Closed family-specific destination deployment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[norito(tag = "family", content = "deployment")]
pub enum SccpDestinationDeploymentV1 {
    /// EVM deployment for Ethereum or BSC.
    #[codec(index = 0)]
    #[norito(rename = "evm")]
    Evm(SccpEvmDestinationDeploymentV1),
    /// TRON TVM deployment.
    #[codec(index = 1)]
    #[norito(rename = "tron")]
    Tron(SccpTronDestinationDeploymentV1),
    /// Solana Loader-v3 route and native-verifier deployment.
    #[codec(index = 2)]
    #[norito(rename = "solana")]
    Solana(SccpSolanaDestinationDeploymentV1),
}

impl SccpDestinationDeploymentV1 {
    /// Return the exact governed Groth16 verification-key hash when applicable.
    #[must_use]
    pub const fn groth16_verifier_key_hash(&self) -> [u8; 32] {
        match self {
            Self::Evm(deployment) => deployment.verifier_key_hash,
            Self::Tron(deployment) => deployment.verifier_key_hash,
            Self::Solana(deployment) => deployment.verifier_key_hash,
        }
    }

    /// Return the mandatory immutable outbound proof policy.
    #[must_use]
    pub const fn outbound_proof_policy(&self) -> SccpOutboundProofPolicyV1 {
        match self {
            Self::Evm(deployment) => deployment.outbound_proof_policy,
            Self::Tron(deployment) => deployment.outbound_proof_policy,
            Self::Solana(deployment) => deployment.outbound_proof_policy,
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
            (
                Self::Evm(deployment),
                SccpNetworkV1::EthereumMainnet
                | SccpNetworkV1::EthereumSepolia
                | SccpNetworkV1::BscMainnet
                | SccpNetworkV1::BscTestnet,
            ) => validate_evm_deployment(deployment),
            (
                Self::Tron(deployment),
                SccpNetworkV1::TronMainnet | SccpNetworkV1::TronNile | SccpNetworkV1::TronShasta,
            ) => validate_tron_deployment(deployment),
            (Self::Solana(deployment), SccpNetworkV1::SolanaTestnet) => {
                validate_solana_deployment(deployment)
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
            Self::Solana(deployment) => {
                sccp_solana_destination_binding_hash_v1(lane.source, deployment)
            }
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
            Self::Solana(deployment) => sccp_exact_solana_xor_route_config_hash_v1(
                lane.source,
                source_lane_hash,
                destination_lane_hash,
                deployment,
                route_revision,
            ),
        }
    }
}

/// Typed SORA-side asset and custody policy for atomic SCCP settlement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
pub struct SccpSoraSettlementV1 {
    /// Canonical SORA-home asset definition locked and released by Core.
    pub asset_definition_id: AssetDefinitionId,
    /// Canonical custody account holding route liquidity.
    pub custody_account_id: AccountId,
    /// Decimal scale used by the SCCP unsigned amount field.
    pub payload_amount_scale: u32,
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
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
        if self.source_identity.lane != self.lane_id || !self.source_identity.is_well_formed() {
            return Err(SccpRouteValidationError::SourceDestinationMismatch);
        }
        if let SccpDestinationDeploymentV1::Solana(deployment) = &self.destination {
            let SccpSourceEmitterV1::Solana(source_emitter) = self.source_identity.emitter else {
                return Err(SccpRouteValidationError::SourceDestinationMismatch);
            };
            let expected_config = sccp_solana_native_verifier_config_hash_v1(
                self.lane_id,
                &self.route_id,
                &self.asset_key,
                self.revision,
                source_emitter.program_id,
                deployment,
            )?;
            if deployment.native_verifier_config_hash != expected_config {
                return Err(SccpRouteValidationError::SolanaNativeVerifierConfigMismatch);
            }
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
            self.destination.groth16_verifier_key_hash(),
            outbound_proof_policy.semantic_profile_hash()?,
            outbound_proof_policy.sora_finality_anchor_hash()?,
        ])?;
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

    /// Derive the exact immutable route-configuration hash exposed by the
    /// destination contract.
    ///
    /// This is the single V1 route-configuration commitment recorded in
    /// outbound messages and exposed as Groth16 public signal 9. It
    /// must remain byte-identical to the governed EVM/TVM/Solana route
    /// configuration commitment.
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
    /// Returns [`SccpRouteValidationError`] when the lane or destination
    /// deployment is invalid.
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
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
/// Tags `6..=9` are permanently reserved for retired pre-release identities.
/// They are deliberately not reused after removing those profiles because the
/// exact first-release transfer contracts commit TRON profiles as `10..=12`.
/// Reassigning the gap would make governed lane/configuration hashes disagree
/// with deployed contract state.
#[must_use]
pub const fn sccp_network_tag_v1(network: SccpNetworkV1) -> u8 {
    match network {
        SccpNetworkV1::SoraTaira => 1,
        SccpNetworkV1::EthereumMainnet => 2,
        SccpNetworkV1::EthereumSepolia => 3,
        SccpNetworkV1::BscMainnet => 4,
        SccpNetworkV1::BscTestnet => 5,
        SccpNetworkV1::TronMainnet => 10,
        SccpNetworkV1::TronNile => 11,
        SccpNetworkV1::TronShasta => 12,
        SccpNetworkV1::SolanaTestnet => 13,
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
    let SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(circuit) = profile;
    let mut out = Vec::with_capacity(99);
    out.push(1);
    out.push(0);
    out.push(circuit.version);
    out.extend_from_slice(&circuit.circuit_commitment);
    out.extend_from_slice(&circuit.witness_generator_commitment);
    out.extend_from_slice(&circuit.public_signal_schema_hash);
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
    let mut out = Vec::with_capacity(140);
    out.push(anchor.version);
    out.push(sccp_network_tag_v1(anchor.source_network));
    push_u16(&mut out, anchor.protocol_version);
    out.extend_from_slice(&anchor.chain_id_hash);
    push_u64(&mut out, anchor.checkpoint_height);
    out.extend_from_slice(&anchor.checkpoint_block_hash);
    out.extend_from_slice(&anchor.checkpoint_context_id);
    out.extend_from_slice(&anchor.checkpoint_finality_artifact_hash);
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
        SccpNetworkV1::EthereumSepolia => push_u64(&mut out, 11_155_111),
        SccpNetworkV1::BscMainnet => push_u64(&mut out, 56),
        SccpNetworkV1::BscTestnet => push_u64(&mut out, 97),
        SccpNetworkV1::TronMainnet => push_u32(&mut out, 0x2b66_53dc),
        SccpNetworkV1::TronNile => push_u32(&mut out, 0xcd86_90dc),
        SccpNetworkV1::TronShasta => push_u32(&mut out, 0x94a9_059e),
        SccpNetworkV1::SolanaTestnet => {
            out.extend_from_slice(&SCCP_SOLANA_TESTNET_GENESIS_HASH_V1);
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
        SccpSourceEmitterV1::Solana(SccpSolanaSourceEmitterV1 {
            program_id,
            program_data_address,
            program_data_slot,
            state_account,
            program_code_hash,
            route_config_hash,
        }) => {
            out.push(2);
            out.extend_from_slice(program_id);
            out.extend_from_slice(program_data_address);
            push_u64(&mut out, *program_data_slot);
            out.extend_from_slice(state_account);
            out.extend_from_slice(program_code_hash);
            out.extend_from_slice(route_config_hash);
        }
    }
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
        SccpNetworkV1::EthereumSepolia => (SCCP_DOMAIN_ETH, 11_155_111),
        SccpNetworkV1::BscMainnet => (SCCP_DOMAIN_BSC, 56),
        SccpNetworkV1::BscTestnet => (SCCP_DOMAIN_BSC, 97),
        _ => return Err(SccpRouteValidationError::DestinationFamilyMismatch),
    };
    let semantic_profile_hash = deployment.outbound_proof_policy.semantic_profile_hash()?;
    let finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()?;
    let mut payload = Vec::with_capacity(32 * 11);
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
        SccpNetworkV1::TronNile => 0xcd86_90dc,
        SccpNetworkV1::TronShasta => 0x94a9_059e,
        _ => return Err(SccpRouteValidationError::DestinationFamilyMismatch),
    };
    let semantic_profile_hash = deployment.outbound_proof_policy.semantic_profile_hash()?;
    let finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()?;
    let mut payload = Vec::with_capacity(32 * 11);
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
    Ok(keccak256(payload))
}

/// Derive the Solana destination binding from raw public keys and immutable
/// Loader-v3 deployment pins.
///
/// The preimage is an explicit little-endian SCCP encoding rather than a
/// Base58 or JSON representation. The route program and native verifier keep
/// separate ProgramData/state/code roles, and the full governed BN254 key is
/// transitively bound through its validated canonical hash.
///
/// # Errors
///
/// Returns [`SccpRouteValidationError`] when `network` is not the exact
/// genesis-bound Solana testnet profile or any governed deployment role is
/// malformed.
pub fn sccp_solana_destination_binding_hash_v1(
    network: SccpNetworkV1,
    deployment: &SccpSolanaDestinationDeploymentV1,
) -> Result<[u8; 32], SccpRouteValidationError> {
    validate_solana_deployment(deployment)?;
    if network != SccpNetworkV1::SolanaTestnet {
        return Err(SccpRouteValidationError::DestinationFamilyMismatch);
    }
    let semantic_profile_hash = deployment.outbound_proof_policy.semantic_profile_hash()?;
    let finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()?;
    let mut payload = Vec::with_capacity(704);
    payload.extend_from_slice(SOLANA_BINDING_DOMAIN_V1);
    payload.push(1);
    payload.extend_from_slice(SOLANA_GROTH16_BACKEND_V1);
    payload.push(sccp_network_tag_v1(network));
    push_u32(&mut payload, SCCP_DOMAIN_SORA);
    push_u32(&mut payload, SCCP_DOMAIN_SOLANA);
    payload.extend_from_slice(&SCCP_SOLANA_TESTNET_GENESIS_HASH_V1);
    payload.extend_from_slice(&deployment.token_mint_address);
    payload.extend_from_slice(&deployment.route_program_id);
    payload.extend_from_slice(&deployment.route_program_data_address);
    push_u64(&mut payload, deployment.route_program_data_slot);
    payload.extend_from_slice(&deployment.route_state_account);
    payload.extend_from_slice(&deployment.route_program_code_hash);
    payload.extend_from_slice(&deployment.native_verifier_program_id);
    payload.extend_from_slice(&deployment.native_verifier_program_data_address);
    push_u64(&mut payload, deployment.native_verifier_program_data_slot);
    payload.extend_from_slice(&deployment.native_verifier_material_account);
    payload.extend_from_slice(&deployment.native_verifier_program_code_hash);
    payload.extend_from_slice(&deployment.native_verifier_config_hash);
    payload.extend_from_slice(&deployment.verifier_key_hash);
    payload.extend_from_slice(&semantic_profile_hash);
    payload.extend_from_slice(&finality_anchor_hash);
    Ok(keccak256(payload))
}

/// Derive the one-way native-verifier material configuration commitment for
/// the exact Solana-testnet XOR route.
///
/// This preimage contains only primitive governed identities. In particular,
/// it excludes the destination-binding and route-configuration hashes because
/// both of those commit this config hash and the material PDA. Feeding either
/// derived hash back into this function would create an infeasible
/// cryptographic fixed point. The sealed material account stores those two
/// finished hashes separately and the verifier compares them with the
/// destination bridge state before every settlement.
///
/// # Errors
///
/// Returns [`SccpRouteValidationError`] when the lane, route identity,
/// deployment, policy, key, or revision is not the exact first-release shape.
pub fn sccp_solana_native_verifier_config_hash_v1(
    lane: SccpLaneIdV1,
    route_id: &str,
    asset_key: &str,
    route_revision: u32,
    source_program_id: [u8; 32],
    deployment: &SccpSolanaDestinationDeploymentV1,
) -> Result<[u8; 32], SccpRouteValidationError> {
    validate_inbound_lane(lane)?;
    if lane.source != SccpNetworkV1::SolanaTestnet || lane.target != SccpNetworkV1::SoraTaira {
        return Err(SccpRouteValidationError::ConcreteRouteMismatch);
    }
    if route_revision != 1 {
        return Err(SccpRouteValidationError::InvalidRouteRevision);
    }
    validate_concrete_route_identity(
        lane.source,
        route_id,
        asset_key,
        SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
    )?;
    validate_solana_deployment(deployment)?;
    if source_program_id == [0; 32]
        || source_program_id == deployment.route_program_id
        || source_program_id == deployment.native_verifier_program_id
    {
        return Err(SccpRouteValidationError::RoleAlias);
    }
    let source_lane_hash =
        sccp_lane_id_hash_v1(lane).ok_or(SccpRouteValidationError::InvalidInboundLane)?;
    let destination_lane_hash = sccp_lane_id_hash_v1(SccpLaneIdV1 {
        source: lane.target,
        target: lane.source,
    })
    .ok_or(SccpRouteValidationError::InvalidInboundLane)?;
    let semantic_profile_hash = deployment.outbound_proof_policy.semantic_profile_hash()?;
    let sora_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()?;
    let SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(circuit) =
        deployment.outbound_proof_policy.semantic_profile;
    if circuit.public_signal_schema_hash != sccp_groth16_bn254_public_signal_schema_hash_v1() {
        return Err(SccpRouteValidationError::InvalidSemanticProofProfile);
    }

    let sora_network = canonical_sccp_network_bytes_v1(SccpNetworkV1::SoraTaira);
    let solana_network = canonical_sccp_network_bytes_v1(SccpNetworkV1::SolanaTestnet);
    let asset_len = u8::try_from(asset_key.len())
        .map_err(|_| SccpRouteValidationError::ConcreteRouteMismatch)?;
    let multiplier = u128::from(deployment.taira_to_token_multiplier).to_le_bytes();
    let mut payload = Vec::with_capacity(512);
    payload.extend_from_slice(SOLANA_NATIVE_VERIFIER_CONFIG_DOMAIN_V1);
    payload.push(1);
    payload.extend_from_slice(&sora_network);
    payload.extend_from_slice(&solana_network);
    payload.extend_from_slice(route_id.as_bytes());
    push_u32(&mut payload, route_revision);
    payload.push(asset_len);
    payload.extend_from_slice(asset_key.as_bytes());
    payload.extend_from_slice(&multiplier);
    payload.extend_from_slice(&deployment.verifier_key_hash);
    payload.extend_from_slice(&deployment.native_verifier_program_id);
    payload.extend_from_slice(&deployment.route_program_id);
    payload.extend_from_slice(&source_program_id);
    payload.extend_from_slice(&deployment.route_state_account);
    payload.extend_from_slice(&deployment.token_mint_address);
    payload.extend_from_slice(&SOLANA_CLASSIC_TOKEN_PROGRAM_ID);
    payload.extend_from_slice(&source_lane_hash);
    payload.extend_from_slice(&destination_lane_hash);
    payload.extend_from_slice(&sora_anchor_hash);
    payload.extend_from_slice(&semantic_profile_hash);
    payload.extend_from_slice(&circuit.circuit_commitment);
    payload.extend_from_slice(&circuit.witness_generator_commitment);
    payload.extend_from_slice(&circuit.public_signal_schema_hash);
    Ok(Sha256::digest(payload).into())
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
        SccpNetworkV1::EthereumMainnet => (SCCP_DOMAIN_ETH, 2, 1, b"taira_eth_xor".as_slice()),
        SccpNetworkV1::EthereumSepolia => {
            (SCCP_DOMAIN_ETH, 3, 11_155_111, b"taira_eth_xor".as_slice())
        }
        SccpNetworkV1::BscMainnet => (SCCP_DOMAIN_BSC, 4, 56, b"taira_bsc_xor".as_slice()),
        SccpNetworkV1::BscTestnet => (SCCP_DOMAIN_BSC, 5, 97, b"taira_bsc_xor".as_slice()),
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
        deployment.verifier_key_hash,
        semantic_profile_hash,
        finality_anchor_hash,
    ])?;

    let mut deployment_config = Vec::with_capacity(32 * 7);
    deployment_config.extend_from_slice(&abi_word_bytes20(deployment.token_address));
    deployment_config.extend_from_slice(&deployment.token_code_hash);
    deployment_config.extend_from_slice(&abi_word_bytes20(deployment.verifier_address));
    deployment_config.extend_from_slice(&deployment.verifier_code_hash);
    deployment_config.extend_from_slice(&deployment.verifier_key_hash);
    deployment_config.extend_from_slice(&semantic_profile_hash);
    deployment_config.extend_from_slice(&finality_anchor_hash);
    let deployment_config_hash = keccak256(deployment_config);

    let mut asset_route = Vec::with_capacity(32 * 4);
    asset_route.extend_from_slice(&keccak256(b"xor"));
    asset_route.extend_from_slice(&keccak256(route_id));
    asset_route.extend_from_slice(&abi_word_u32(route_revision));
    asset_route.extend_from_slice(&abi_word_u64(deployment.taira_to_token_multiplier));
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
        SccpNetworkV1::TronMainnet => (10, 0x2b66_53dc),
        SccpNetworkV1::TronNile => (11, 0xcd86_90dc),
        SccpNetworkV1::TronShasta => (12, 0x94a9_059e),
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
        deployment.verifier_key_hash,
        semantic_profile_hash,
        finality_anchor_hash,
        destination_binding_hash,
    ])?;

    let mut deployment_config = Vec::with_capacity(32 * 8);
    deployment_config.extend_from_slice(&abi_word_bytes20(deployment.token_address));
    deployment_config.extend_from_slice(&deployment.token_code_hash);
    deployment_config.extend_from_slice(&abi_word_bytes20(deployment.verifier_address));
    deployment_config.extend_from_slice(&deployment.verifier_code_hash);
    deployment_config.extend_from_slice(&deployment.verifier_key_hash);
    deployment_config.extend_from_slice(&semantic_profile_hash);
    deployment_config.extend_from_slice(&finality_anchor_hash);
    deployment_config.extend_from_slice(&destination_binding_hash);
    let deployment_config_hash = keccak256(deployment_config);

    let mut asset_route = Vec::with_capacity(32 * 4);
    asset_route.extend_from_slice(&keccak256(b"xor"));
    asset_route.extend_from_slice(&keccak256(b"taira_tron_xor"));
    asset_route.extend_from_slice(&abi_word_u32(route_revision));
    asset_route.extend_from_slice(&abi_word_u64(deployment.taira_to_token_multiplier));
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

/// Compute the immutable route-config hash for the exact Solana-testnet XOR route.
///
/// # Errors
///
/// Returns [`SccpRouteValidationError`] when the lane, immutable Loader-v3
/// deployment, governed verifier key/policy, or route revision is invalid.
pub fn sccp_exact_solana_xor_route_config_hash_v1(
    network: SccpNetworkV1,
    source_lane_hash: [u8; 32],
    destination_lane_hash: [u8; 32],
    deployment: &SccpSolanaDestinationDeploymentV1,
    route_revision: u32,
) -> Result<[u8; 32], SccpRouteValidationError> {
    validate_solana_deployment(deployment)?;
    if network != SccpNetworkV1::SolanaTestnet {
        return Err(SccpRouteValidationError::DestinationFamilyMismatch);
    }
    if route_revision == 0 {
        return Err(SccpRouteValidationError::InvalidRouteRevision);
    }
    validate_lane_hash_pair(network, source_lane_hash, destination_lane_hash)?;
    let destination_binding_hash = sccp_solana_destination_binding_hash_v1(network, deployment)?;
    let semantic_profile_hash = deployment.outbound_proof_policy.semantic_profile_hash()?;
    let finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()?;
    validate_hash_roles(&[
        source_lane_hash,
        destination_lane_hash,
        deployment.route_program_code_hash,
        deployment.native_verifier_program_code_hash,
        deployment.native_verifier_config_hash,
        deployment.verifier_key_hash,
        semantic_profile_hash,
        finality_anchor_hash,
        destination_binding_hash,
    ])?;

    let mut deployment_config = Vec::with_capacity(640);
    deployment_config.extend_from_slice(&deployment.token_mint_address);
    deployment_config.extend_from_slice(&deployment.route_program_id);
    deployment_config.extend_from_slice(&deployment.route_program_data_address);
    push_u64(&mut deployment_config, deployment.route_program_data_slot);
    deployment_config.extend_from_slice(&deployment.route_state_account);
    deployment_config.extend_from_slice(&deployment.route_program_code_hash);
    deployment_config.extend_from_slice(&deployment.native_verifier_program_id);
    deployment_config.extend_from_slice(&deployment.native_verifier_program_data_address);
    push_u64(
        &mut deployment_config,
        deployment.native_verifier_program_data_slot,
    );
    deployment_config.extend_from_slice(&deployment.native_verifier_material_account);
    deployment_config.extend_from_slice(&deployment.native_verifier_program_code_hash);
    deployment_config.extend_from_slice(&deployment.native_verifier_config_hash);
    deployment_config.extend_from_slice(&deployment.verifier_key_hash);
    deployment_config.extend_from_slice(&semantic_profile_hash);
    deployment_config.extend_from_slice(&finality_anchor_hash);
    deployment_config.extend_from_slice(&destination_binding_hash);
    let deployment_config_hash = keccak256(deployment_config);

    let mut asset_route = Vec::with_capacity(96);
    asset_route.extend_from_slice(b"xor");
    asset_route.extend_from_slice(b"taira_sol_xor");
    push_u32(&mut asset_route, route_revision);
    push_u64(&mut asset_route, deployment.taira_to_token_multiplier);
    let asset_route_config_hash = keccak256(asset_route);

    let mut payload = Vec::with_capacity(256);
    payload.extend_from_slice(CONCRETE_ROUTE_CONFIG_DOMAIN_V1);
    payload.push(1);
    push_u32(&mut payload, SCCP_DOMAIN_SOLANA);
    payload.push(sccp_network_tag_v1(network));
    payload.extend_from_slice(&SCCP_SOLANA_TESTNET_GENESIS_HASH_V1);
    payload.extend_from_slice(&source_lane_hash);
    payload.extend_from_slice(&destination_lane_hash);
    payload.extend_from_slice(&deployment_config_hash);
    payload.extend_from_slice(&asset_route_config_hash);
    Ok(keccak256(payload))
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
        SccpNetworkV1::EthereumMainnet | SccpNetworkV1::EthereumSepolia => "taira_eth_xor",
        SccpNetworkV1::BscMainnet | SccpNetworkV1::BscTestnet => "taira_bsc_xor",
        SccpNetworkV1::TronMainnet | SccpNetworkV1::TronNile | SccpNetworkV1::TronShasta => {
            "taira_tron_xor"
        }
        SccpNetworkV1::SolanaTestnet => "taira_sol_xor",
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
    if deployment.taira_to_token_multiplier != SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER {
        return Err(SccpRouteValidationError::ConcreteRouteMismatch);
    }
    validate_nonzero("token_address", &deployment.token_address)?;
    validate_nonzero("verifier_address", &deployment.verifier_address)?;
    validate_nonzero("route_address", &deployment.route_address)?;
    validate_distinct(&[
        deployment.token_address,
        deployment.verifier_address,
        deployment.route_address,
    ])?;
    let derived_key_hash = sccp_groth16_bn254_verifying_key_hash_v1(deployment.verifying_key)?;
    if derived_key_hash != deployment.verifier_key_hash {
        return Err(SccpRouteValidationError::Groth16VerifyingKeyHashMismatch);
    }
    deployment.outbound_proof_policy.validate()?;
    let semantic_profile_hash = deployment.outbound_proof_policy.semantic_profile_hash()?;
    let finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()?;
    validate_hash_roles(&[
        deployment.token_code_hash,
        deployment.verifier_code_hash,
        deployment.verifier_key_hash,
        deployment.route_code_hash,
        semantic_profile_hash,
        finality_anchor_hash,
    ])
}

fn validate_tron_deployment(
    deployment: &SccpTronDestinationDeploymentV1,
) -> Result<(), SccpRouteValidationError> {
    if deployment.taira_to_token_multiplier != SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER {
        return Err(SccpRouteValidationError::ConcreteRouteMismatch);
    }
    validate_nonzero("token_address", &deployment.token_address)?;
    validate_nonzero("verifier_address", &deployment.verifier_address)?;
    validate_nonzero("route_address", &deployment.route_address)?;
    validate_distinct(&[
        deployment.token_address,
        deployment.verifier_address,
        deployment.route_address,
    ])?;
    let derived_key_hash = sccp_groth16_bn254_verifying_key_hash_v1(deployment.verifying_key)?;
    if derived_key_hash != deployment.verifier_key_hash {
        return Err(SccpRouteValidationError::Groth16VerifyingKeyHashMismatch);
    }
    deployment.outbound_proof_policy.validate()?;
    let semantic_profile_hash = deployment.outbound_proof_policy.semantic_profile_hash()?;
    let finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()?;
    validate_hash_roles(&[
        deployment.token_code_hash,
        deployment.verifier_code_hash,
        deployment.verifier_key_hash,
        deployment.route_code_hash,
        semantic_profile_hash,
        finality_anchor_hash,
    ])
}

fn validate_solana_deployment(
    deployment: &SccpSolanaDestinationDeploymentV1,
) -> Result<(), SccpRouteValidationError> {
    if deployment.taira_to_token_multiplier != SCCP_V1_TAIRA_TO_SOLANA_TOKEN_MULTIPLIER {
        return Err(SccpRouteValidationError::ConcreteRouteMismatch);
    }
    if deployment.route_program_data_slot == 0 || deployment.native_verifier_program_data_slot == 0
    {
        return Err(SccpRouteValidationError::ZeroRole("program_data_slot"));
    }
    let derived_key_hash = sccp_groth16_bn254_verifying_key_hash_v1(deployment.verifying_key)?;
    if derived_key_hash != deployment.verifier_key_hash {
        return Err(SccpRouteValidationError::Groth16VerifyingKeyHashMismatch);
    }
    deployment.outbound_proof_policy.validate()?;
    let semantic_profile_hash = deployment.outbound_proof_policy.semantic_profile_hash()?;
    let finality_anchor_hash = deployment
        .outbound_proof_policy
        .sora_finality_anchor_hash()?;
    let roles = [
        deployment.token_mint_address,
        deployment.route_program_id,
        deployment.route_program_data_address,
        deployment.route_state_account,
        deployment.route_program_code_hash,
        deployment.native_verifier_program_id,
        deployment.native_verifier_program_data_address,
        deployment.native_verifier_material_account,
        deployment.native_verifier_program_code_hash,
        deployment.native_verifier_config_hash,
        deployment.verifier_key_hash,
        semantic_profile_hash,
        finality_anchor_hash,
        SOLANA_CLASSIC_TOKEN_PROGRAM_ID,
    ];
    for role in &roles {
        validate_nonzero("solana_deployment_role", role)?;
    }
    validate_distinct(&roles)
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
        (SccpSourceEmitterV1::Solana(source), SccpDestinationDeploymentV1::Solana(deployment)) => {
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
            source.route_config_hash == route_config_hash
                && validate_distinct(&[
                    source.program_id,
                    source.program_data_address,
                    source.state_account,
                    source.program_code_hash,
                    source.route_config_hash,
                    deployment.token_mint_address,
                    deployment.route_program_id,
                    deployment.route_program_data_address,
                    deployment.route_state_account,
                    deployment.route_program_code_hash,
                    deployment.native_verifier_program_id,
                    deployment.native_verifier_program_data_address,
                    deployment.native_verifier_material_account,
                    deployment.native_verifier_program_code_hash,
                    deployment.native_verifier_config_hash,
                    deployment.verifier_key_hash,
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
            SccpNetworkV1::EthereumMainnet | SccpNetworkV1::EthereumSepolia
        ) | (
            BridgeNativeProofBackendV1::BscParlia,
            SccpNetworkV1::BscMainnet | SccpNetworkV1::BscTestnet
        ) | (
            BridgeNativeProofBackendV1::TronDpos,
            SccpNetworkV1::TronMainnet | SccpNetworkV1::TronNile | SccpNetworkV1::TronShasta
        ) | (
            BridgeNativeProofBackendV1::SolanaAgave,
            SccpNetworkV1::SolanaTestnet
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

fn validate_distinct<const N: usize>(values: &[[u8; N]]) -> Result<(), SccpRouteValidationError> {
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

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_vec(out: &mut Vec<u8>, value: &[u8]) {
    let len = u32::try_from(value.len()).expect("bounded SCCP field length fits u32");
    push_u32(out, len);
    out.extend_from_slice(value);
}

fn blake2b256(prefix: &[u8], payload: &[u8]) -> [u8; 32] {
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(prefix);
    hasher.update(payload);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::codec::DecodeAll as _;

    const SIGNATORY: &str =
        "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";

    fn word_u64(value: u64) -> [u8; 32] {
        let mut word = [0; 32];
        word[24..].copy_from_slice(&value.to_be_bytes());
        word
    }

    fn hex32(value: &str) -> [u8; 32] {
        assert_eq!(value.len(), 64);
        let mut output = [0; 32];
        for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
            let nibble = |byte: u8| match byte {
                b'0'..=b'9' => byte - b'0',
                b'a'..=b'f' => byte - b'a' + 10,
                _ => panic!("non-lowercase hexadecimal test vector"),
            };
            output[index] = (nibble(pair[0]) << 4) | nibble(pair[1]);
        }
        output
    }

    fn verifying_key() -> SccpGroth16Bn254VerifyingKeyV1 {
        let g1 = SccpBn254G1PointV1 {
            x: word_u64(1),
            y: word_u64(2),
        };
        let g2 = SccpBn254G2PointV1 {
            x_c0: hex32("1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed"),
            x_c1: hex32("198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2"),
            y_c0: hex32("12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa"),
            y_c1: hex32("090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b"),
        };
        SccpGroth16Bn254VerifyingKeyV1 {
            version: 1,
            alpha1: g1,
            beta2: g2,
            gamma2: g2,
            delta2: g2,
            ic: SccpGroth16Bn254IcV1 {
                constant: g1,
                signal_0: g1,
                signal_1: g1,
                signal_2: g1,
                signal_3: g1,
                signal_4: g1,
                signal_5: g1,
                signal_6: g1,
                signal_7: g1,
                signal_8: g1,
                signal_9: g1,
                signal_10: g1,
            },
        }
    }

    fn outbound_proof_policy() -> SccpOutboundProofPolicyV1 {
        SccpOutboundProofPolicyV1 {
            version: 1,
            semantic_profile: SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(
                SccpGroth16Bn254SemanticCircuitV1 {
                    version: 1,
                    circuit_commitment: [0x71; 32],
                    witness_generator_commitment: [0x72; 32],
                    public_signal_schema_hash: sccp_groth16_bn254_public_signal_schema_hash_v1(),
                },
            ),
            sora_finality_anchor: SccpSoraFinalityAnchorV1 {
                version: 1,
                source_network: SccpNetworkV1::SoraTaira,
                protocol_version: PROTOCOL_VERSION,
                chain_id_hash: sccp_sora_taira_chain_id_hash_v1(),
                checkpoint_height: 5,
                checkpoint_block_hash: [0x73; 32],
                checkpoint_context_id: [0x74; 32],
                checkpoint_finality_artifact_hash: [0x75; 32],
            },
        }
    }

    fn sora_outbound_execution_policy() -> SccpSoraOutboundExecutionPolicyV1 {
        SccpSoraOutboundExecutionPolicyV1 {
            version: 1,
            semantics: SCCP_V1_SORA_OUTBOUND_EXECUTION_SEMANTICS.to_owned(),
            contract_artifact_sha256: [0xb1; 32],
            vk_ref: SccpPortableVerifyingKeyRefV1 {
                backend: "stark/fri/v1".to_owned(),
                name: "ivm-execution-v1".to_owned(),
                version: 1,
                commitment: [0xb2; 32],
            },
            gas_limit: 50_000_000,
        }
    }

    fn lane() -> SccpLaneIdV1 {
        SccpLaneIdV1 {
            source: SccpNetworkV1::EthereumMainnet,
            target: SccpNetworkV1::SoraTaira,
        }
    }

    fn deployment(revision: u32) -> SccpEvmDestinationDeploymentV1 {
        let key = verifying_key();
        let key_hash = sccp_groth16_bn254_verifying_key_hash_v1(key)
            .expect("valid structural verification key");
        let revision_byte = u8::try_from(revision).expect("test revision fits u8");
        SccpEvmDestinationDeploymentV1 {
            token_address: [0x10_u8.wrapping_add(revision_byte); 20],
            token_code_hash: [0x20_u8.wrapping_add(revision_byte); 32],
            verifier_address: [0x30_u8.wrapping_add(revision_byte); 20],
            verifier_code_hash: [0x40_u8.wrapping_add(revision_byte); 32],
            verifying_key: key,
            verifier_key_hash: key_hash,
            outbound_proof_policy: outbound_proof_policy(),
            route_address: [0x50_u8.wrapping_add(revision_byte); 20],
            route_code_hash: [0x60_u8.wrapping_add(revision_byte); 32],
            taira_to_token_multiplier: SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER,
        }
    }

    fn tron_deployment() -> SccpTronDestinationDeploymentV1 {
        let key = verifying_key();
        SccpTronDestinationDeploymentV1 {
            token_address: [0x11; 20],
            token_code_hash: [0x21; 32],
            verifier_address: [0x31; 20],
            verifier_code_hash: [0x41; 32],
            verifying_key: key,
            verifier_key_hash: sccp_groth16_bn254_verifying_key_hash_v1(key)
                .expect("valid structural verification key"),
            outbound_proof_policy: outbound_proof_policy(),
            route_address: [0x51; 20],
            route_code_hash: [0x61; 32],
            taira_to_token_multiplier: SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER,
        }
    }

    fn solana_lane() -> SccpLaneIdV1 {
        SccpLaneIdV1 {
            source: SccpNetworkV1::SolanaTestnet,
            target: SccpNetworkV1::SoraTaira,
        }
    }

    /// Cross-language Solana fixture shared byte-for-byte with
    /// `javascript/iroha_js/test/sccpExact.test.js`.
    fn solana_outbound_proof_policy() -> SccpOutboundProofPolicyV1 {
        SccpOutboundProofPolicyV1 {
            version: 1,
            semantic_profile: SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(
                SccpGroth16Bn254SemanticCircuitV1 {
                    version: 1,
                    circuit_commitment: [0xc1; 32],
                    witness_generator_commitment: [0xc2; 32],
                    public_signal_schema_hash: sccp_groth16_bn254_public_signal_schema_hash_v1(),
                },
            ),
            sora_finality_anchor: SccpSoraFinalityAnchorV1 {
                version: 1,
                source_network: SccpNetworkV1::SoraTaira,
                protocol_version: PROTOCOL_VERSION,
                chain_id_hash: sccp_sora_taira_chain_id_hash_v1(),
                checkpoint_height: 7,
                checkpoint_block_hash: [0xa1; 32],
                checkpoint_context_id: [0xa2; 32],
                checkpoint_finality_artifact_hash: [0xa3; 32],
            },
        }
    }

    fn solana_deployment() -> SccpSolanaDestinationDeploymentV1 {
        let key = verifying_key();
        let mut deployment = SccpSolanaDestinationDeploymentV1 {
            token_mint_address: [0x11; 32],
            route_program_id: [0x12; 32],
            route_program_data_address: [0x13; 32],
            route_program_data_slot: 17,
            route_state_account: [0x14; 32],
            route_program_code_hash: [0x15; 32],
            native_verifier_program_id: [0x16; 32],
            native_verifier_program_data_address: [0x17; 32],
            native_verifier_program_data_slot: 18,
            native_verifier_material_account: [0x18; 32],
            native_verifier_program_code_hash: [0x19; 32],
            native_verifier_config_hash: [0x1a; 32],
            verifying_key: key,
            verifier_key_hash: sccp_groth16_bn254_verifying_key_hash_v1(key)
                .expect("valid structural verification key"),
            outbound_proof_policy: solana_outbound_proof_policy(),
            taira_to_token_multiplier: SCCP_V1_TAIRA_TO_SOLANA_TOKEN_MULTIPLIER,
        };
        deployment.native_verifier_config_hash = sccp_solana_native_verifier_config_hash_v1(
            solana_lane(),
            "taira_sol_xor",
            "xor",
            1,
            [0x31; 32],
            &deployment,
        )
        .expect("exact one-way native verifier config");
        deployment
    }

    fn solana_route(activation: SccpRouteActivationV1) -> SccpGovernedRouteV1 {
        let lane = solana_lane();
        let deployment = solana_deployment();
        let destination = SccpDestinationDeploymentV1::Solana(deployment);
        let route_config_hash = destination
            .route_configuration_hash(
                lane,
                "taira_sol_xor",
                "xor",
                1,
                SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            )
            .expect("valid exact Solana route configuration");
        SccpGovernedRouteV1 {
            lane_id: lane,
            route_id: "taira_sol_xor".to_owned(),
            asset_key: "xor".to_owned(),
            revision: 1,
            activation,
            inbound_finality_cutoff: activation.is_terminal().then_some(
                SccpInboundFinalityCutoffV1 {
                    trust_anchor_hash: [0xa1; 32],
                    max_anchor_interval_height: 100,
                },
            ),
            source_identity: SccpSourceIdentityV1 {
                lane,
                emitter: SccpSourceEmitterV1::Solana(SccpSolanaSourceEmitterV1 {
                    program_id: [0x31; 32],
                    program_data_address: [0x32; 32],
                    program_data_slot: 19,
                    state_account: [0x33; 32],
                    program_code_hash: [0x34; 32],
                    route_config_hash,
                }),
            },
            destination,
            sora_outbound_execution_policy: sora_outbound_execution_policy(),
            settlement: SccpSoraSettlementV1 {
                asset_definition_id: sccp_v1_taira_xor_asset_definition_id(),
                custody_account_id: AccountId::new(
                    SIGNATORY.parse().expect("valid custody public key"),
                ),
                payload_amount_scale: SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            },
        }
    }

    fn solana_anchor(height: u64) -> SccpNativeTrustAnchorV1 {
        SccpNativeTrustAnchorV1 {
            backend: BridgeNativeProofBackendV1::SolanaAgave,
            anchor_hash: [0xa1; 32],
            checkpoint_height: height,
        }
    }

    fn route(revision: u32, activation: SccpRouteActivationV1) -> SccpGovernedRouteV1 {
        let lane = lane();
        let deployment = deployment(revision);
        let destination = SccpDestinationDeploymentV1::Evm(deployment);
        let route_config_hash = destination
            .route_configuration_hash(
                lane,
                "taira_eth_xor",
                "xor",
                revision,
                SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            )
            .expect("valid exact route configuration");
        SccpGovernedRouteV1 {
            lane_id: lane,
            route_id: "taira_eth_xor".to_owned(),
            asset_key: "xor".to_owned(),
            revision,
            activation,
            inbound_finality_cutoff: activation.is_terminal().then_some(
                SccpInboundFinalityCutoffV1 {
                    trust_anchor_hash: [0x91; 32],
                    max_anchor_interval_height: 100,
                },
            ),
            source_identity: SccpSourceIdentityV1 {
                lane,
                emitter: SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                    address: deployment.route_address,
                    runtime_code_hash: deployment.route_code_hash,
                    route_config_hash,
                }),
            },
            destination,
            sora_outbound_execution_policy: sora_outbound_execution_policy(),
            settlement: SccpSoraSettlementV1 {
                asset_definition_id: sccp_v1_taira_xor_asset_definition_id(),
                custody_account_id: AccountId::new(
                    SIGNATORY.parse().expect("valid custody public key"),
                ),
                payload_amount_scale: SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            },
        }
    }

    fn retarget_evm_route_source(
        mut route: SccpGovernedRouteV1,
        source: SccpNetworkV1,
    ) -> SccpGovernedRouteV1 {
        let lane = SccpLaneIdV1 {
            source,
            target: SccpNetworkV1::SoraTaira,
        };
        route.lane_id = lane;
        route.source_identity.lane = lane;
        let route_config_hash = route
            .destination
            .route_configuration_hash(
                lane,
                &route.route_id,
                &route.asset_key,
                route.revision,
                route.settlement.payload_amount_scale,
            )
            .expect("valid retargeted EVM route configuration");
        let SccpSourceEmitterV1::Evm(emitter) = &mut route.source_identity.emitter else {
            panic!("fixture route must use an EVM source emitter")
        };
        emitter.route_config_hash = route_config_hash;
        route
    }

    fn anchor(height: u64) -> SccpNativeTrustAnchorV1 {
        SccpNativeTrustAnchorV1 {
            backend: BridgeNativeProofBackendV1::EthereumBeacon,
            anchor_hash: [0x91; 32],
            checkpoint_height: height,
        }
    }

    fn registry(
        routes: Vec<SccpGovernedRouteV1>,
        native_trust_anchor: Option<SccpNativeTrustAnchorV1>,
    ) -> SccpRegistryV1 {
        registry_for_lane(lane(), routes, native_trust_anchor)
    }

    fn registry_for_lane(
        lane_id: SccpLaneIdV1,
        routes: Vec<SccpGovernedRouteV1>,
        native_trust_anchor: Option<SccpNativeTrustAnchorV1>,
    ) -> SccpRegistryV1 {
        let native_trust_anchors = native_trust_anchor.into_iter().collect();
        SccpRegistryV1 {
            version: 1,
            lanes: vec![SccpGovernedLaneV1 {
                lane_id,
                native_trust_anchors,
                current_native_trust_anchor_hash: native_trust_anchor
                    .map(|anchor| anchor.anchor_hash),
                routes,
            }],
        }
    }

    #[cfg(feature = "json")]
    fn insert_unknown_json_field(value: &mut norito::json::Value, path: &[&str]) {
        let mut current = value;
        for field in path {
            let norito::json::Value::Object(object) = current else {
                panic!("JSON path component `{field}` is not an object")
            };
            current = object
                .get_mut(*field)
                .unwrap_or_else(|| panic!("JSON path component `{field}` is absent"));
        }
        let norito::json::Value::Object(object) = current else {
            panic!("JSON target at {path:?} is not an object")
        };
        object.insert(
            "adversarial_extension".to_owned(),
            norito::json::Value::Null,
        );
    }

    #[test]
    fn solidity_verifying_key_hash_vector_is_exact() {
        let key = verifying_key();
        assert_eq!(
            canonical_sccp_groth16_bn254_verifying_key_bytes_v1(key)
                .expect("canonical key")
                .len(),
            38 * 32
        );
        assert_eq!(
            sccp_groth16_bn254_verifying_key_hash_v1(key).expect("canonical key hash"),
            hex32("6923e63427820ab42cc16c3c2bc0eb4097577919bb3911ea50cbb4f20cebfddb")
        );
    }

    #[test]
    fn network_tags_and_tron_route_hash_match_exact_contract_vectors() {
        assert_eq!(sccp_network_tag_v1(SccpNetworkV1::SoraTaira), 1);
        assert_eq!(sccp_network_tag_v1(SccpNetworkV1::EthereumMainnet), 2);
        assert_eq!(sccp_network_tag_v1(SccpNetworkV1::EthereumSepolia), 3);
        assert_eq!(sccp_network_tag_v1(SccpNetworkV1::BscMainnet), 4);
        assert_eq!(sccp_network_tag_v1(SccpNetworkV1::BscTestnet), 5);
        assert_eq!(sccp_network_tag_v1(SccpNetworkV1::TronMainnet), 10);
        assert_eq!(sccp_network_tag_v1(SccpNetworkV1::TronNile), 11);
        assert_eq!(sccp_network_tag_v1(SccpNetworkV1::TronShasta), 12);
        assert_eq!(sccp_network_tag_v1(SccpNetworkV1::SolanaTestnet), 13);

        let mut expected_solana_network = vec![0x01, 0x0d, 0x03, 0x00, 0x00, 0x00];
        expected_solana_network.extend_from_slice(&SCCP_SOLANA_TESTNET_GENESIS_HASH_V1);
        assert_eq!(
            canonical_sccp_network_bytes_v1(SccpNetworkV1::SolanaTestnet),
            expected_solana_network,
            "Solana network identity must contain the raw genesis bytes, never Base58 text"
        );

        // Tags 0 and 6..=9 are permanently reserved. This exact byte vector is
        // consumed by SccpExactTransferCodec.tronNetwork in deployed contracts.
        assert_eq!(
            canonical_sccp_network_bytes_v1(SccpNetworkV1::TronNile),
            vec![0x01, 0x0b, 0x05, 0x00, 0x00, 0x00, 0xdc, 0x90, 0x86, 0xcd]
        );

        let inbound_lane = SccpLaneIdV1 {
            source: SccpNetworkV1::TronNile,
            target: SccpNetworkV1::SoraTaira,
        };
        let outbound_lane = SccpLaneIdV1 {
            source: inbound_lane.target,
            target: inbound_lane.source,
        };
        let source_lane_hash = sccp_lane_id_hash_v1(inbound_lane).expect("valid inbound lane");
        let destination_lane_hash =
            sccp_lane_id_hash_v1(outbound_lane).expect("valid outbound lane");
        assert_eq!(
            source_lane_hash,
            hex32("d49004c91652644a316330b34b7cdda89264fc577096661a5d41dea31d59b95a")
        );
        assert_eq!(
            destination_lane_hash,
            hex32("7e68f921bbe63831f95498b695850630818a2a09c0215f4722199a38ec5f55f2")
        );
        assert_eq!(
            sccp_exact_tron_xor_route_config_hash_v1(
                SccpNetworkV1::TronNile,
                source_lane_hash,
                destination_lane_hash,
                &tron_deployment(),
                7,
            )
            .expect("valid exact TRON route"),
            hex32("d6e06a169ace343b7cd3a3bcd0b1188f7b98ff3abe7def64ca230333babc39c9")
        );
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one adversarial matrix covers every Solana deployment role and binding"
    )]
    fn solana_deployment_roundtrips_and_rejects_cross_family_zero_alias_and_swapped_pins() {
        let lane = solana_lane();
        let deployment = solana_deployment();
        let destination = SccpDestinationDeploymentV1::Solana(deployment);
        destination
            .validate_for_lane(lane)
            .expect("complete Solana deployment validates");
        assert_eq!(
            destination.groth16_verifier_key_hash(),
            deployment.verifier_key_hash
        );
        let native_config = sccp_solana_native_verifier_config_hash_v1(
            lane,
            "taira_sol_xor",
            "xor",
            1,
            [0x31; 32],
            &deployment,
        )
        .expect("exact native verifier config");
        assert_eq!(
            native_config,
            hex32("bcb83baf2f2ab57a56b72529cf749da6175f8e65a048287eae217b61a2c84669"),
            "native verifier config must match the independent JavaScript V1 vector"
        );
        for excluded in [
            SccpSolanaDestinationDeploymentV1 {
                route_program_data_address: [0x41; 32],
                ..deployment
            },
            SccpSolanaDestinationDeploymentV1 {
                route_program_data_slot: 41,
                ..deployment
            },
            SccpSolanaDestinationDeploymentV1 {
                route_program_code_hash: [0x42; 32],
                ..deployment
            },
            SccpSolanaDestinationDeploymentV1 {
                native_verifier_program_data_address: [0x43; 32],
                ..deployment
            },
            SccpSolanaDestinationDeploymentV1 {
                native_verifier_program_data_slot: 43,
                ..deployment
            },
            SccpSolanaDestinationDeploymentV1 {
                native_verifier_material_account: [0x44; 32],
                ..deployment
            },
            SccpSolanaDestinationDeploymentV1 {
                native_verifier_program_code_hash: [0x45; 32],
                ..deployment
            },
            SccpSolanaDestinationDeploymentV1 {
                native_verifier_config_hash: [0x46; 32],
                ..deployment
            },
        ] {
            assert_eq!(
                sccp_solana_native_verifier_config_hash_v1(
                    lane,
                    "taira_sol_xor",
                    "xor",
                    1,
                    [0x31; 32],
                    &excluded,
                )
                .expect("excluded deployment identity remains structurally valid"),
                native_config,
                "Loader metadata, material PDA, code hashes, and the output itself must remain outside the one-way config preimage"
            );
        }
        for included in [
            SccpSolanaDestinationDeploymentV1 {
                token_mint_address: [0x47; 32],
                ..deployment
            },
            SccpSolanaDestinationDeploymentV1 {
                route_program_id: [0x48; 32],
                ..deployment
            },
            SccpSolanaDestinationDeploymentV1 {
                route_state_account: [0x49; 32],
                ..deployment
            },
            SccpSolanaDestinationDeploymentV1 {
                native_verifier_program_id: [0x4a; 32],
                ..deployment
            },
        ] {
            assert_ne!(
                sccp_solana_native_verifier_config_hash_v1(
                    lane,
                    "taira_sol_xor",
                    "xor",
                    1,
                    [0x31; 32],
                    &included,
                )
                .expect("included deployment identity remains structurally valid"),
                native_config,
                "every value-moving program/state identity must be config-bound"
            );
        }
        assert_eq!(
            sccp_solana_native_verifier_config_hash_v1(
                lane,
                "taira_sol_xor",
                "xor",
                2,
                [0x31; 32],
                &deployment,
            ),
            Err(SccpRouteValidationError::InvalidRouteRevision),
            "the exact first-release Solana executable must reject revision two"
        );

        let source = solana_route(SccpRouteActivationV1::Staged)
            .source_identity
            .emitter;
        let SccpSourceEmitterV1::Solana(source_fields) = source else {
            unreachable!("fixture uses Solana")
        };
        let source_bytes =
            canonical_sccp_source_emitter_bytes_v1(&source).expect("canonical source emitter");
        assert_eq!(source_bytes.len(), 170);
        assert_eq!(&source_bytes[..2], &[1, 2]);
        assert_eq!(&source_bytes[2..34], &source_fields.program_id);
        assert_eq!(&source_bytes[34..66], &source_fields.program_data_address);
        assert_eq!(
            &source_bytes[66..74],
            &source_fields.program_data_slot.to_le_bytes()
        );
        assert_eq!(&source_bytes[74..106], &source_fields.state_account);
        assert_eq!(
            source_fields.route_config_hash,
            hex32("3f2c81fe59637d4a9af916dfce1b623ef59f44087db3ee0c25e42ad8ec1bf958"),
            "Solana source emitter must pin the same exact route configuration as JavaScript"
        );
        assert_eq!(
            sccp_source_emitter_identity_hash_v1(&source).expect("source emitter hash"),
            hex32("f0c6b976d69c3d0e001b5ee87d7d2fabd068db424c1e261cf8e9e1d8b1f4cbfa"),
            "Solana source emitter must match the independent JavaScript V1 vector"
        );

        let encoded = norito::to_bytes(&destination).expect("Solana destination encodes");
        assert_eq!(
            norito::decode_from_bytes::<SccpDestinationDeploymentV1>(&encoded)
                .expect("Solana destination decodes"),
            destination
        );
        let encoded_deployment = norito::to_bytes(&deployment).expect("Solana deployment encodes");
        assert_eq!(
            norito::decode_from_bytes::<SccpSolanaDestinationDeploymentV1>(&encoded_deployment)
                .expect("Solana deployment decodes"),
            deployment
        );
        let unknown_variant = 3_u32.encode();
        assert!(
            SccpDestinationDeploymentV1::decode_all(&mut unknown_variant.as_slice()).is_err(),
            "unknown destination families must fail before interpreting payload bytes"
        );

        #[cfg(feature = "json")]
        {
            let json = norito::json::to_json(&destination).expect("Solana destination JSON");
            assert_eq!(
                norito::json::from_json::<SccpDestinationDeploymentV1>(&json)
                    .expect("Solana destination JSON decodes"),
                destination
            );
            assert!(
                norito::json::from_json::<SccpDestinationDeploymentV1>(
                    &json.replace("\"solana\"", "\"unknown\"")
                )
                .is_err()
            );
        }

        for wrong_lane in [
            SccpLaneIdV1 {
                source: SccpNetworkV1::EthereumMainnet,
                target: SccpNetworkV1::SoraTaira,
            },
            SccpLaneIdV1 {
                source: SccpNetworkV1::SoraTaira,
                target: SccpNetworkV1::SolanaTestnet,
            },
        ] {
            assert!(
                matches!(
                    destination.validate_for_lane(wrong_lane),
                    Err(SccpRouteValidationError::DestinationFamilyMismatch
                        | SccpRouteValidationError::InvalidInboundLane)
                ),
                "cross-family/direction lane unexpectedly accepted: {wrong_lane:?}"
            );
        }

        let assert_invalid = |hostile: SccpSolanaDestinationDeploymentV1| {
            assert!(
                SccpDestinationDeploymentV1::Solana(hostile)
                    .validate_for_lane(lane)
                    .is_err(),
                "hostile deployment unexpectedly validated: {hostile:?}"
            );
        };
        assert_invalid(SccpSolanaDestinationDeploymentV1 {
            token_mint_address: [0; 32],
            ..deployment
        });
        assert_invalid(SccpSolanaDestinationDeploymentV1 {
            route_program_code_hash: [0; 32],
            ..deployment
        });
        assert_invalid(SccpSolanaDestinationDeploymentV1 {
            native_verifier_program_id: [0; 32],
            ..deployment
        });
        assert_invalid(SccpSolanaDestinationDeploymentV1 {
            native_verifier_material_account: [0; 32],
            ..deployment
        });
        assert_invalid(SccpSolanaDestinationDeploymentV1 {
            native_verifier_config_hash: [0; 32],
            ..deployment
        });
        assert_invalid(SccpSolanaDestinationDeploymentV1 {
            route_program_data_slot: 0,
            ..deployment
        });
        assert_invalid(SccpSolanaDestinationDeploymentV1 {
            native_verifier_program_data_slot: 0,
            ..deployment
        });
        assert_invalid(SccpSolanaDestinationDeploymentV1 {
            route_program_data_address: deployment.route_program_id,
            ..deployment
        });
        assert_invalid(SccpSolanaDestinationDeploymentV1 {
            route_program_id: SOLANA_CLASSIC_TOKEN_PROGRAM_ID,
            ..deployment
        });
        assert_invalid(SccpSolanaDestinationDeploymentV1 {
            route_state_account: deployment.route_program_code_hash,
            ..deployment
        });
        assert_invalid(SccpSolanaDestinationDeploymentV1 {
            native_verifier_material_account: deployment.native_verifier_config_hash,
            ..deployment
        });
        assert_invalid(SccpSolanaDestinationDeploymentV1 {
            verifier_key_hash: deployment.native_verifier_config_hash,
            ..deployment
        });

        let baseline_binding = destination
            .destination_binding_hash(lane)
            .expect("baseline binding");
        let baseline_route = destination
            .route_configuration_hash(
                lane,
                "taira_sol_xor",
                "xor",
                1,
                SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            )
            .expect("baseline route hash");
        assert_eq!(
            baseline_binding,
            hex32("cd1ff581301bd31b583b835ec71f185139ce1af2376dfe656216481f7a77ba2c"),
            "Solana destination binding must match the independent JavaScript V1 vector"
        );
        assert_eq!(
            baseline_route,
            hex32("3f2c81fe59637d4a9af916dfce1b623ef59f44087db3ee0c25e42ad8ec1bf958"),
            "Solana route configuration must match the independent JavaScript V1 vector"
        );
        let mut swapped_accounts = deployment;
        core::mem::swap(
            &mut swapped_accounts.route_program_data_address,
            &mut swapped_accounts.route_state_account,
        );
        let swapped_destination = SccpDestinationDeploymentV1::Solana(swapped_accounts);
        swapped_destination
            .validate_for_lane(lane)
            .expect("distinct swapped values remain structurally decodable");
        assert_ne!(
            baseline_binding,
            swapped_destination
                .destination_binding_hash(lane)
                .expect("swapped binding"),
            "ProgramData and state roles must be position-bound"
        );
        assert_ne!(
            baseline_route,
            swapped_destination
                .route_configuration_hash(
                    lane,
                    "taira_sol_xor",
                    "xor",
                    1,
                    SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
                )
                .expect("swapped route hash"),
            "approved route hash must reject a ProgramData/state substitution"
        );

        let mut swapped_hashes = deployment;
        core::mem::swap(
            &mut swapped_hashes.route_program_code_hash,
            &mut swapped_hashes.native_verifier_config_hash,
        );
        let swapped_hash_destination = SccpDestinationDeploymentV1::Solana(swapped_hashes);
        assert_ne!(
            baseline_binding,
            swapped_hash_destination
                .destination_binding_hash(lane)
                .expect("swapped hash binding"),
            "code and configuration hashes must be position-bound"
        );

        let route = solana_route(SccpRouteActivationV1::Staged);
        let SccpSourceEmitterV1::Solana(source_deployment) = route.source_identity.emitter else {
            unreachable!("fixture uses Solana")
        };
        let SccpDestinationDeploymentV1::Solana(destination_deployment) = route.destination else {
            unreachable!("fixture uses Solana")
        };
        assert_ne!(
            source_deployment.program_id, destination_deployment.route_program_id,
            "source and destination programs are independently governed roles"
        );
        route.validate().expect("baseline Solana route");
        let mut stale_native_config = route.clone();
        let SccpDestinationDeploymentV1::Solana(ref mut hostile_deployment) =
            stale_native_config.destination
        else {
            unreachable!("fixture uses Solana")
        };
        hostile_deployment.native_verifier_config_hash = [0x46; 32];
        assert_eq!(
            stale_native_config.validate(),
            Err(SccpRouteValidationError::SolanaNativeVerifierConfigMismatch),
            "a structurally valid but noncanonical material config must fail before route use"
        );
        for (source_role, destination_role) in [
            (0usize, destination_deployment.route_program_id),
            (1, destination_deployment.route_program_data_address),
            (2, destination_deployment.route_state_account),
            (3, destination_deployment.route_program_code_hash),
        ] {
            let mut aliased = route.clone();
            let SccpSourceEmitterV1::Solana(ref mut emitter) = aliased.source_identity.emitter
            else {
                unreachable!("fixture uses Solana")
            };
            match source_role {
                0 => emitter.program_id = destination_role,
                1 => emitter.program_data_address = destination_role,
                2 => emitter.state_account = destination_role,
                3 => emitter.program_code_hash = destination_role,
                _ => unreachable!(),
            }
            let expected_error = if source_role == 0 {
                SccpRouteValidationError::RoleAlias
            } else {
                SccpRouteValidationError::SourceDestinationMismatch
            };
            assert_eq!(
                aliased.validate(),
                Err(expected_error),
                "source and destination deployment roles must remain independently governed"
            );
        }
        let baseline_source_hash =
            sccp_source_identity_hash_v1(&route.source_identity).expect("source identity hash");
        assert_eq!(
            baseline_source_hash,
            hex32("6c62bd033e5beb7848c66c10ae1be0a6fc1960b239f7b04b31bb3c5a7b1efa69"),
            "Solana source identity must match the independent JavaScript V1 vector"
        );
        let mut swapped_source = route.clone();
        let SccpSourceEmitterV1::Solana(ref mut emitter) = swapped_source.source_identity.emitter
        else {
            unreachable!("fixture uses Solana")
        };
        core::mem::swap(
            &mut emitter.program_data_address,
            &mut emitter.state_account,
        );
        assert_ne!(
            baseline_source_hash,
            sccp_source_identity_hash_v1(&swapped_source.source_identity)
                .expect("swapped source identity hash"),
            "source ProgramData/state substitutions must change the governed identity"
        );
        let SccpSourceEmitterV1::Solana(ref mut emitter) = swapped_source.source_identity.emitter
        else {
            unreachable!("fixture uses Solana")
        };
        emitter.route_config_hash[0] ^= 1;
        assert_eq!(
            swapped_source.validate(),
            Err(SccpRouteValidationError::SourceDestinationMismatch)
        );

        assert_eq!(
            destination.route_configuration_hash(
                lane,
                "taira_tron_xor",
                "xor",
                1,
                SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            ),
            Err(SccpRouteValidationError::ConcreteRouteMismatch)
        );
        assert_eq!(
            solana_route(SccpRouteActivationV1::Bidirectional)
                .validate_with_anchor(Some(anchor(1))),
            Err(SccpRouteValidationError::TrustAnchorFamilyMismatch)
        );
    }

    #[test]
    fn verifying_key_structure_and_embedded_hash_fail_closed() {
        let mut invalid_version = verifying_key();
        invalid_version.version = 2;
        assert_eq!(
            invalid_version.validate_structure(),
            Err(SccpRouteValidationError::InvalidGroth16VerifyingKey)
        );

        let mut infinity = verifying_key();
        infinity.alpha1 = SccpBn254G1PointV1 {
            x: [0; 32],
            y: [0; 32],
        };
        assert_eq!(
            infinity.validate_structure(),
            Err(SccpRouteValidationError::InvalidGroth16VerifyingKey)
        );

        let mut out_of_field = verifying_key();
        out_of_field.alpha1.x = BN254_BASE_FIELD_MODULUS_BE;
        assert_eq!(
            out_of_field.validate_structure(),
            Err(SccpRouteValidationError::InvalidGroth16VerifyingKey)
        );

        let mut mismatch = route(1, SccpRouteActivationV1::Staged);
        let SccpDestinationDeploymentV1::Evm(deployment) = &mut mismatch.destination else {
            unreachable!("fixture uses EVM")
        };
        deployment.verifier_key_hash[0] ^= 1;
        assert_eq!(
            mismatch.validate(),
            Err(SccpRouteValidationError::Groth16VerifyingKeyHashMismatch)
        );
    }

    #[test]
    fn route_revision_is_explicit_in_config_and_source_identity() {
        let first = route(1, SccpRouteActivationV1::Staged);
        let second = route(2, SccpRouteActivationV1::Staged);
        assert_eq!(
            first
                .route_configuration_hash()
                .expect("governed route hash"),
            first
                .destination
                .route_configuration_hash(
                    first.lane_id,
                    &first.route_id,
                    &first.asset_key,
                    first.revision,
                    first.settlement.payload_amount_scale,
                )
                .expect("destination contract route config")
        );
        assert_ne!(
            first.route_configuration_hash().expect("revision one hash"),
            second
                .route_configuration_hash()
                .expect("revision two hash")
        );
        assert_ne!(
            first
                .destination
                .route_configuration_hash(
                    first.lane_id,
                    &first.route_id,
                    &first.asset_key,
                    1,
                    first.settlement.payload_amount_scale,
                )
                .expect("revision one route config"),
            first
                .destination
                .route_configuration_hash(
                    first.lane_id,
                    &first.route_id,
                    &first.asset_key,
                    2,
                    first.settlement.payload_amount_scale,
                )
                .expect("revision two route config")
        );
        assert!(first.validate().is_ok());
        assert!(second.validate().is_ok());

        let mut invalid_source = first.clone();
        let SccpSourceEmitterV1::Evm(mut emitter) = invalid_source.source_identity.emitter else {
            panic!("fixture route must use an EVM source emitter");
        };
        emitter.runtime_code_hash[0] ^= 1;
        invalid_source.source_identity.emitter = SccpSourceEmitterV1::Evm(emitter);
        assert_eq!(
            invalid_source.route_configuration_hash(),
            Err(SccpRouteValidationError::SourceDestinationMismatch)
        );
    }

    #[test]
    fn optional_anchor_and_draining_lifecycle_preserve_redemption() {
        assert!(
            registry(vec![route(1, SccpRouteActivationV1::Staged)], None)
                .validate()
                .is_ok()
        );
        assert_eq!(
            registry(vec![route(1, SccpRouteActivationV1::Bidirectional)], None).validate(),
            Err(SccpRouteValidationError::UnsupportedInboundActivation)
        );
        let draining = route(1, SccpRouteActivationV1::InboundOnly);
        let live = route(2, SccpRouteActivationV1::Bidirectional);
        assert!(draining.activation.allows_inbound());
        assert!(!draining.activation.allows_outbound());
        assert!(
            registry(vec![draining, live], Some(anchor(100)))
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn governance_activation_is_separate_from_network_environment_classification() {
        let staging_lane = SccpLaneIdV1 {
            source: SccpNetworkV1::EthereumSepolia,
            target: SccpNetworkV1::SoraTaira,
        };
        let native_anchor = anchor(100);
        let staged =
            retarget_evm_route_source(route(1, SccpRouteActivationV1::Staged), staging_lane.source);
        staged
            .validate_with_anchor(Some(native_anchor))
            .expect("staging route remains governable while disabled");

        for activation in [
            SccpRouteActivationV1::InboundOnly,
            SccpRouteActivationV1::Bidirectional,
        ] {
            let route = retarget_evm_route_source(route(1, activation), staging_lane.source);
            assert!(!route.source_identity.has_production_source());
            assert!(!route.source_identity.has_governance_activatable_source());
            assert_eq!(
                route.validate_with_anchor(Some(native_anchor)),
                Err(SccpRouteValidationError::UnsupportedInboundActivation),
                "existing staging profiles must preserve their fail-closed activation policy"
            );
            assert_eq!(
                registry_for_lane(staging_lane, vec![route], Some(native_anchor)).validate(),
                Err(SccpRouteValidationError::UnsupportedInboundActivation)
            );
        }

        let solana = solana_route(SccpRouteActivationV1::Bidirectional);
        assert!(solana.lane_id.is_staging_environment());
        assert!(!solana.source_identity.has_production_source());
        assert!(solana.source_identity.has_governance_activatable_source());
        solana
            .validate_with_anchor(Some(solana_anchor(100)))
            .expect("complete Solana testnet material is activation eligible");
        registry_for_lane(solana_lane(), vec![solana], Some(solana_anchor(100)))
            .validate()
            .expect("Solana testnet remains staging-classified while active by governance");
    }

    #[test]
    fn trust_anchor_history_is_append_only_and_current_selects_highest_checkpoint() {
        let first = anchor(100);
        let second = SccpNativeTrustAnchorV1 {
            anchor_hash: [0x92; 32],
            checkpoint_height: 200,
            ..first
        };
        let mut governed = registry(vec![route(1, SccpRouteActivationV1::Staged)], Some(first));
        governed.lanes[0].native_trust_anchors.push(second);
        governed.lanes[0].current_native_trust_anchor_hash = Some(second.anchor_hash);
        governed.validate().expect("append-only anchor history");
        let lane = &governed.lanes[0];
        assert_eq!(lane.current_native_trust_anchor(), Some(second));
        assert_eq!(
            lane.native_trust_anchor_by_hash(first.anchor_hash),
            Some(first)
        );
        assert_eq!(
            lane.native_trust_anchor_interval(first.anchor_hash),
            Some((first, Some(second.checkpoint_height)))
        );
        assert_eq!(
            lane.native_trust_anchor_interval(second.anchor_hash),
            Some((second, None))
        );

        let mut stale_pointer = governed.clone();
        stale_pointer.lanes[0].current_native_trust_anchor_hash = Some(first.anchor_hash);
        assert_eq!(
            stale_pointer.validate(),
            Err(SccpRouteValidationError::InvalidCurrentTrustAnchor)
        );

        let mut duplicate_hash = governed.clone();
        duplicate_hash.lanes[0]
            .native_trust_anchors
            .push(SccpNativeTrustAnchorV1 {
                anchor_hash: first.anchor_hash,
                checkpoint_height: 300,
                ..first
            });
        duplicate_hash.lanes[0].current_native_trust_anchor_hash = Some(first.anchor_hash);
        assert_eq!(
            duplicate_hash.validate(),
            Err(SccpRouteValidationError::InvalidTrustAnchorHistory)
        );

        let mut rollback = governed;
        rollback.lanes[0]
            .native_trust_anchors
            .push(SccpNativeTrustAnchorV1 {
                anchor_hash: [0x93; 32],
                checkpoint_height: 150,
                ..first
            });
        rollback.lanes[0].current_native_trust_anchor_hash = Some([0x93; 32]);
        assert_eq!(
            rollback.validate(),
            Err(SccpRouteValidationError::InvalidTrustAnchorHistory)
        );
    }

    #[test]
    fn terminal_history_does_not_exhaust_live_route_capacity() {
        let routes = (1..=12)
            .map(|revision| {
                route(
                    revision,
                    if revision == 12 {
                        SccpRouteActivationV1::Staged
                    } else {
                        SccpRouteActivationV1::Retired
                    },
                )
            })
            .collect::<Vec<_>>();
        let first = anchor(100);
        let second = SccpNativeTrustAnchorV1 {
            anchor_hash: [0x92; 32],
            checkpoint_height: 101,
            ..first
        };
        let mut governed = registry(routes, Some(first));
        governed.lanes[0].native_trust_anchors.push(second);
        governed.lanes[0].current_native_trust_anchor_hash = Some(second.anchor_hash);
        for route in governed.lanes[0]
            .routes
            .iter_mut()
            .filter(|route| route.activation.is_terminal())
        {
            route.inbound_finality_cutoff = Some(SccpInboundFinalityCutoffV1 {
                trust_anchor_hash: first.anchor_hash,
                max_anchor_interval_height: second.checkpoint_height,
            });
        }
        governed
            .validate()
            .expect("retained terminal history below the generous cap remains valid");
        assert_eq!(governed.lanes[0].routes.len(), 12);

        let live_routes = (1..=SCCP_V1_MAX_LIVE_ROUTES_PER_LANE + 1)
            .map(|revision| {
                route(
                    u32::try_from(revision).expect("test revision fits u32"),
                    SccpRouteActivationV1::Staged,
                )
            })
            .collect();
        assert_eq!(
            registry(live_routes, Some(anchor(100))).validate(),
            Err(SccpRouteValidationError::InvalidLaneLiveRouteCount)
        );
    }

    #[test]
    fn retained_history_caps_accept_exact_bounds_and_reject_one_more() {
        let first = anchor(99);
        let second = SccpNativeTrustAnchorV1 {
            anchor_hash: [0x92; 32],
            checkpoint_height: 100,
            ..first
        };
        let routes = (1..=SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE)
            .map(|revision| {
                route(
                    u32::try_from(revision).expect("retained route bound fits u32"),
                    if revision == SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE {
                        SccpRouteActivationV1::Staged
                    } else {
                        SccpRouteActivationV1::Retired
                    },
                )
            })
            .collect::<Vec<_>>();
        let mut exact_routes = registry(routes, Some(first));
        exact_routes.lanes[0].native_trust_anchors.push(second);
        exact_routes.lanes[0].current_native_trust_anchor_hash = Some(second.anchor_hash);
        exact_routes
            .validate()
            .expect("exact retained-route bound remains admissible");

        let mut excess_routes = exact_routes;
        excess_routes.lanes[0].routes.push(route(
            u32::try_from(SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE + 1)
                .expect("retained route overflow fixture fits u32"),
            SccpRouteActivationV1::Staged,
        ));
        assert_eq!(
            excess_routes.validate(),
            Err(SccpRouteValidationError::TooManyRetainedRoutes)
        );

        let retained_anchor = |height: usize| {
            let height = u64::try_from(height).expect("retained anchor bound fits u64");
            let mut anchor_hash = [0_u8; 32];
            anchor_hash[24..].copy_from_slice(&height.to_be_bytes());
            SccpNativeTrustAnchorV1 {
                backend: BridgeNativeProofBackendV1::EthereumBeacon,
                anchor_hash,
                checkpoint_height: height,
            }
        };
        let anchors = (1..=SCCP_V1_MAX_RETAINED_NATIVE_TRUST_ANCHORS_PER_LANE)
            .map(retained_anchor)
            .collect::<Vec<_>>();
        let last_anchor_hash = anchors
            .last()
            .expect("retained-anchor bound is nonzero")
            .anchor_hash;
        let exact_anchors = SccpRegistryV1 {
            version: 1,
            lanes: vec![SccpGovernedLaneV1 {
                lane_id: lane(),
                native_trust_anchors: anchors,
                current_native_trust_anchor_hash: Some(last_anchor_hash),
                routes: vec![route(1, SccpRouteActivationV1::Staged)],
            }],
        };
        exact_anchors
            .validate()
            .expect("exact retained-anchor bound remains admissible");

        let mut excess_anchors = exact_anchors;
        let excess_anchor = retained_anchor(SCCP_V1_MAX_RETAINED_NATIVE_TRUST_ANCHORS_PER_LANE + 1);
        excess_anchors.lanes[0]
            .native_trust_anchors
            .push(excess_anchor);
        excess_anchors.lanes[0].current_native_trust_anchor_hash = Some(excess_anchor.anchor_hash);
        assert_eq!(
            excess_anchors.validate(),
            Err(SccpRouteValidationError::TooManyRetainedTrustAnchors)
        );

        let maximum_route = route(1, SccpRouteActivationV1::Retired);
        maximum_route
            .validate()
            .expect("fixed-shape V1 route remains valid");
        assert!(
            maximum_route.encode().len() <= 4_096,
            "retained-route envelope exceeded the cap-sizing assumption"
        );
        assert!(
            retained_anchor(1).encode().len() <= 64,
            "retained-anchor envelope exceeded the cap-sizing assumption"
        );
        assert_eq!(
            SCCP_V1_MAX_GOVERNED_LANES
                * (SCCP_V1_MAX_RETAINED_ROUTES_PER_LANE * 4_096
                    + SCCP_V1_MAX_RETAINED_NATIVE_TRUST_ANCHORS_PER_LANE * 64),
            8 * 1024 * 1024,
            "conservative retained-entry envelope must remain eight MiB"
        );
    }

    #[test]
    fn retired_route_cutoff_must_belong_to_one_retained_anchor_interval() {
        let first = anchor(100);
        let second = SccpNativeTrustAnchorV1 {
            anchor_hash: [0x92; 32],
            checkpoint_height: 200,
            ..first
        };
        for cutoff in [
            SccpInboundFinalityCutoffV1 {
                trust_anchor_hash: [0; 32],
                max_anchor_interval_height: 0,
            },
            SccpInboundFinalityCutoffV1 {
                trust_anchor_hash: first.anchor_hash,
                max_anchor_interval_height: second.checkpoint_height,
            },
        ] {
            let mut live = route(1, SccpRouteActivationV1::Staged);
            live.inbound_finality_cutoff = Some(cutoff);
            assert_eq!(
                live.validate(),
                Err(SccpRouteValidationError::InvalidInboundFinalityCutoff),
                "nonterminal route carried cutoff {cutoff:?}"
            );
        }
        let mut governed = registry(vec![route(1, SccpRouteActivationV1::Retired)], Some(first));
        governed.lanes[0].native_trust_anchors.push(second);
        governed.lanes[0].current_native_trust_anchor_hash = Some(second.anchor_hash);
        governed.lanes[0].routes[0].inbound_finality_cutoff = Some(SccpInboundFinalityCutoffV1 {
            trust_anchor_hash: first.anchor_hash,
            max_anchor_interval_height: second.checkpoint_height,
        });
        governed
            .validate()
            .expect("cutoff at the inclusive successor checkpoint is valid");

        for cutoff in [
            None,
            Some(SccpInboundFinalityCutoffV1 {
                trust_anchor_hash: [0xFF; 32],
                max_anchor_interval_height: 150,
            }),
            Some(SccpInboundFinalityCutoffV1 {
                trust_anchor_hash: first.anchor_hash,
                max_anchor_interval_height: 99,
            }),
            Some(SccpInboundFinalityCutoffV1 {
                trust_anchor_hash: first.anchor_hash,
                max_anchor_interval_height: second.checkpoint_height - 1,
            }),
            Some(SccpInboundFinalityCutoffV1 {
                trust_anchor_hash: first.anchor_hash,
                max_anchor_interval_height: second.checkpoint_height + 1,
            }),
            Some(SccpInboundFinalityCutoffV1 {
                trust_anchor_hash: second.anchor_hash,
                max_anchor_interval_height: second.checkpoint_height + 1,
            }),
        ] {
            let mut hostile = governed.clone();
            hostile.lanes[0].routes[0].inbound_finality_cutoff = cutoff;
            assert_eq!(
                hostile.validate(),
                Err(SccpRouteValidationError::InvalidInboundFinalityCutoff)
            );
        }
    }

    #[test]
    fn multiple_outbound_revisions_and_revision_gaps_are_rejected() {
        assert_eq!(
            registry(
                vec![
                    route(1, SccpRouteActivationV1::Bidirectional),
                    route(2, SccpRouteActivationV1::Bidirectional),
                ],
                Some(anchor(100)),
            )
            .validate(),
            Err(SccpRouteValidationError::MultipleEnabledRevisions)
        );
        assert_eq!(
            registry(
                vec![route(2, SccpRouteActivationV1::Staged)],
                Some(anchor(100))
            )
            .validate(),
            Err(SccpRouteValidationError::InvalidRouteRevision)
        );
    }

    #[test]
    fn activation_transitions_enforce_drain_before_retirement() {
        use SccpRouteActivationV1 as A;
        assert!(A::Staged.can_transition_to(A::Bidirectional));
        assert!(A::Staged.can_transition_to(A::InboundOnly));
        assert!(A::Bidirectional.can_transition_to(A::InboundOnly));
        assert!(A::InboundOnly.can_transition_to(A::Retired));
        assert!(!A::Bidirectional.can_transition_to(A::Retired));
        assert!(!A::Retired.can_transition_to(A::InboundOnly));
        assert!(!A::Paused.can_transition_to(A::Paused));
    }

    #[test]
    fn settlement_asset_scale_and_canonical_keys_are_exact() {
        let mut wrong_scale = route(1, SccpRouteActivationV1::Staged);
        wrong_scale.settlement.payload_amount_scale = 8;
        assert_eq!(
            wrong_scale.validate(),
            Err(SccpRouteValidationError::InvalidSettlementScale)
        );

        let mut wrong_asset = route(1, SccpRouteActivationV1::Staged);
        wrong_asset.settlement.asset_definition_id = AssetDefinitionId::derive_from_components(
            crate::domain::DomainId::try_new("wrong", "universal").expect("valid domain"),
            "xor".parse().expect("valid name"),
        );
        assert_eq!(
            wrong_asset.validate(),
            Err(SccpRouteValidationError::SettlementAssetMismatch)
        );

        let mut uppercase = route(1, SccpRouteActivationV1::Staged);
        uppercase.asset_key = "XOR".to_owned();
        assert!(matches!(
            uppercase.validate(),
            Err(SccpRouteValidationError::NonCanonicalKey("asset_key"))
        ));
    }

    #[test]
    fn source_destination_and_role_aliasing_fail_closed() {
        let mut source_drift = route(1, SccpRouteActivationV1::Staged);
        let SccpSourceEmitterV1::Evm(emitter) = &mut source_drift.source_identity.emitter else {
            unreachable!("fixture uses EVM")
        };
        emitter.address[0] ^= 1;
        assert_eq!(
            source_drift.validate(),
            Err(SccpRouteValidationError::SourceDestinationMismatch)
        );

        let mut alias = route(1, SccpRouteActivationV1::Staged);
        let SccpDestinationDeploymentV1::Evm(deployment) = &mut alias.destination else {
            unreachable!("fixture uses EVM")
        };
        deployment.route_address = deployment.token_address;
        assert_eq!(alias.validate(), Err(SccpRouteValidationError::RoleAlias));
    }

    #[test]
    fn outbound_proof_policy_rejects_every_malformed_or_aliased_role() {
        let policy = outbound_proof_policy();
        policy.validate().expect("exact fixture policy");
        let profile_hash = policy.semantic_profile_hash().expect("profile hash");
        let anchor_hash = policy.sora_finality_anchor_hash().expect("anchor hash");
        assert_eq!(
            sccp_groth16_bn254_public_signal_schema_hash_v1(),
            hex32("7567439f41173d6745a3d51923cb70371acc7d66f23cefb4100d6d5d7a432cbb")
        );
        assert_eq!(
            sccp_sora_taira_chain_id_hash_v1(),
            hex32("cf1cfc0f57b0bfa4c21882a9870317a1f4812f86533897095e3944be34c5bba7")
        );
        assert_eq!(
            profile_hash,
            hex32("ce5a1e17aca3cafe47a403fd66479f0a36339eb56092dafa67c8d97bdeeb60ef")
        );
        assert_eq!(
            anchor_hash,
            hex32("94be7710f3064ff4936d24f51355ca037bf53e653b7712abcd798ba47be20727")
        );
        assert_ne!(profile_hash, [0; 32]);
        assert_ne!(anchor_hash, [0; 32]);
        assert_ne!(profile_hash, anchor_hash);

        let mut invalid = policy;
        invalid.version = 0;
        assert_eq!(
            invalid.validate(),
            Err(SccpRouteValidationError::InvalidOutboundProofPolicy)
        );

        let SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(mut circuit) =
            policy.semantic_profile;
        circuit.circuit_commitment = [0; 32];
        invalid = policy;
        invalid.semantic_profile =
            SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(circuit);
        assert_eq!(
            invalid.validate(),
            Err(SccpRouteValidationError::InvalidSemanticProofProfile)
        );

        let SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(mut circuit) =
            policy.semantic_profile;
        circuit.witness_generator_commitment = circuit.circuit_commitment;
        invalid = policy;
        invalid.semantic_profile =
            SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(circuit);
        assert_eq!(
            invalid.validate(),
            Err(SccpRouteValidationError::InvalidSemanticProofProfile)
        );

        let SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(mut circuit) =
            policy.semantic_profile;
        circuit.public_signal_schema_hash[0] ^= 1;
        invalid = policy;
        invalid.semantic_profile =
            SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(circuit);
        assert_eq!(
            invalid.validate(),
            Err(SccpRouteValidationError::InvalidSemanticProofProfile)
        );

        let anchor_mutations: [fn(&mut SccpSoraFinalityAnchorV1); 10] = [
            |anchor: &mut SccpSoraFinalityAnchorV1| anchor.version = 0,
            |anchor: &mut SccpSoraFinalityAnchorV1| {
                anchor.source_network = SccpNetworkV1::EthereumMainnet;
            },
            |anchor: &mut SccpSoraFinalityAnchorV1| anchor.protocol_version = 1,
            |anchor: &mut SccpSoraFinalityAnchorV1| anchor.chain_id_hash[0] ^= 1,
            |anchor: &mut SccpSoraFinalityAnchorV1| anchor.checkpoint_height = 0,
            |anchor: &mut SccpSoraFinalityAnchorV1| anchor.checkpoint_block_hash = [0; 32],
            |anchor: &mut SccpSoraFinalityAnchorV1| {
                anchor.checkpoint_context_id = [0; 32];
            },
            |anchor: &mut SccpSoraFinalityAnchorV1| {
                anchor.checkpoint_finality_artifact_hash = [0; 32];
            },
            |anchor: &mut SccpSoraFinalityAnchorV1| {
                anchor.checkpoint_context_id = anchor.checkpoint_block_hash;
            },
            |anchor: &mut SccpSoraFinalityAnchorV1| {
                anchor.checkpoint_finality_artifact_hash = anchor.checkpoint_context_id;
            },
        ];
        for mutate in anchor_mutations {
            invalid = policy;
            mutate(&mut invalid.sora_finality_anchor);
            assert_eq!(
                invalid.validate(),
                Err(SccpRouteValidationError::InvalidSoraFinalityAnchor)
            );
        }
    }

    #[test]
    fn canonical_destination_deployments_roundtrip_with_typed_outbound_policy() {
        let evm = deployment(1);
        let evm_bytes = norito::to_bytes(&evm).expect("canonical EVM deployment encodes");
        let decoded_evm = norito::decode_from_bytes::<SccpEvmDestinationDeploymentV1>(&evm_bytes)
            .expect("canonical EVM deployment decodes");
        assert_eq!(decoded_evm, evm);
        assert_eq!(
            norito::to_bytes(&decoded_evm).expect("decoded EVM deployment re-encodes"),
            evm_bytes
        );
        decoded_evm
            .outbound_proof_policy
            .validate()
            .expect("roundtripped EVM policy remains valid");

        let tron = tron_deployment();
        let tron_bytes = norito::to_bytes(&tron).expect("canonical TRON deployment encodes");
        let decoded_tron =
            norito::decode_from_bytes::<SccpTronDestinationDeploymentV1>(&tron_bytes)
                .expect("canonical TRON deployment decodes");
        assert_eq!(decoded_tron, tron);
        assert_eq!(
            norito::to_bytes(&decoded_tron).expect("decoded TRON deployment re-encodes"),
            tron_bytes
        );
        decoded_tron
            .outbound_proof_policy
            .validate()
            .expect("roundtripped TRON policy remains valid");
    }

    #[test]
    fn policyless_norito_destination_deployments_are_rejected() {
        #[derive(norito::derive::NoritoSerialize)]
        struct PolicylessEvmDestinationDeploymentV1 {
            token_address: [u8; 20],
            token_code_hash: [u8; 32],
            verifier_address: [u8; 20],
            verifier_code_hash: [u8; 32],
            verifying_key: SccpGroth16Bn254VerifyingKeyV1,
            verifier_key_hash: [u8; 32],
            route_address: [u8; 20],
            route_code_hash: [u8; 32],
            taira_to_token_multiplier: u64,
        }

        #[derive(norito::derive::NoritoSerialize)]
        struct PolicylessTronDestinationDeploymentV1 {
            token_address: [u8; 20],
            token_code_hash: [u8; 32],
            verifier_address: [u8; 20],
            verifier_code_hash: [u8; 32],
            verifying_key: SccpGroth16Bn254VerifyingKeyV1,
            verifier_key_hash: [u8; 32],
            route_address: [u8; 20],
            route_code_hash: [u8; 32],
            taira_to_token_multiplier: u64,
        }

        let evm = deployment(1);
        let mut evm_bytes = norito::to_bytes(&PolicylessEvmDestinationDeploymentV1 {
            token_address: evm.token_address,
            token_code_hash: evm.token_code_hash,
            verifier_address: evm.verifier_address,
            verifier_code_hash: evm.verifier_code_hash,
            verifying_key: evm.verifying_key,
            verifier_key_hash: evm.verifier_key_hash,
            route_address: evm.route_address,
            route_code_hash: evm.route_code_hash,
            taira_to_token_multiplier: evm.taira_to_token_multiplier,
        })
        .expect("policy-less EVM deployment encodes");
        evm_bytes[6..22].copy_from_slice(
            &<SccpEvmDestinationDeploymentV1 as norito::NoritoSerialize>::schema_hash(),
        );
        assert!(
            norito::decode_from_bytes::<SccpEvmDestinationDeploymentV1>(&evm_bytes).is_err(),
            "policy-less EVM deployment must not decode as the canonical V1 shape"
        );

        let tron = tron_deployment();
        let mut tron_bytes = norito::to_bytes(&PolicylessTronDestinationDeploymentV1 {
            token_address: tron.token_address,
            token_code_hash: tron.token_code_hash,
            verifier_address: tron.verifier_address,
            verifier_code_hash: tron.verifier_code_hash,
            verifying_key: tron.verifying_key,
            verifier_key_hash: tron.verifier_key_hash,
            route_address: tron.route_address,
            route_code_hash: tron.route_code_hash,
            taira_to_token_multiplier: tron.taira_to_token_multiplier,
        })
        .expect("policy-less TRON deployment encodes");
        tron_bytes[6..22].copy_from_slice(
            &<SccpTronDestinationDeploymentV1 as norito::NoritoSerialize>::schema_hash(),
        );
        assert!(
            norito::decode_from_bytes::<SccpTronDestinationDeploymentV1>(&tron_bytes).is_err(),
            "policy-less TRON deployment must not decode as the canonical V1 shape"
        );
    }

    #[cfg(feature = "json")]
    #[test]
    fn policyless_json_destination_deployments_are_rejected() {
        let mut evm = norito::json::to_value(&deployment(1)).expect("serialize EVM deployment");
        let norito::json::Value::Object(evm_object) = &mut evm else {
            panic!("EVM deployment JSON is an object")
        };
        assert!(evm_object.remove("outbound_proof_policy").is_some());
        let evm_json = norito::json::to_json(&evm).expect("serialize policy-less EVM deployment");
        assert!(norito::json::from_json::<SccpEvmDestinationDeploymentV1>(&evm_json).is_err());

        let mut tron =
            norito::json::to_value(&tron_deployment()).expect("serialize TRON deployment");
        let norito::json::Value::Object(tron_object) = &mut tron else {
            panic!("TRON deployment JSON is an object")
        };
        assert!(tron_object.remove("outbound_proof_policy").is_some());
        let tron_json =
            norito::json::to_json(&tron).expect("serialize policy-less TRON deployment");
        assert!(norito::json::from_json::<SccpTronDestinationDeploymentV1>(&tron_json).is_err());
    }

    #[cfg(feature = "json")]
    #[test]
    fn governed_route_json_requires_exact_sora_execution_policy_and_vk_pin() {
        let route = route(1, SccpRouteActivationV1::Staged);
        let mut policyless = norito::json::to_value(&route).expect("serialize governed route");
        let norito::json::Value::Object(route_object) = &mut policyless else {
            panic!("governed route must serialize as an object")
        };
        route_object.remove("sora_outbound_execution_policy");
        let json = norito::json::to_json(&policyless).expect("serialize policy-less route");
        assert!(norito::json::from_json::<SccpGovernedRouteV1>(&json).is_err());

        for missing in ["version", "commitment"] {
            let mut hostile = norito::json::to_value(&route).expect("serialize governed route");
            let norito::json::Value::Object(route_object) = &mut hostile else {
                panic!("governed route must serialize as an object")
            };
            let norito::json::Value::Object(policy_object) = route_object
                .get_mut("sora_outbound_execution_policy")
                .expect("execution policy")
            else {
                panic!("execution policy must serialize as an object")
            };
            let norito::json::Value::Object(vk_ref) = policy_object
                .get_mut("vk_ref")
                .expect("verification-key reference")
            else {
                panic!("verification-key reference must serialize as an object")
            };
            vk_ref.remove(missing);
            let json = norito::json::to_json(&hostile).expect("serialize hostile route");
            assert!(
                norito::json::from_json::<SccpGovernedRouteV1>(&json).is_err(),
                "missing governed verification-key {missing} must reject"
            );
        }

        let exact = &route.sora_outbound_execution_policy.vk_ref;
        let id = crate::proof::VerifyingKeyId::new(exact.backend.clone(), exact.name.clone());
        assert!(exact.matches(&id, exact.version, exact.commitment));
        assert!(!exact.matches(&id, exact.version.saturating_add(1), exact.commitment));
        assert!(!exact.matches(&id, exact.version, [0x7f; 32]));

        let mut zero_version = route.clone();
        zero_version.sora_outbound_execution_policy.vk_ref.version = 0;
        assert_eq!(
            zero_version.validate(),
            Err(SccpRouteValidationError::InvalidSoraOutboundExecutionPolicy),
            "governance must not pin the unversioned registry sentinel"
        );
    }

    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "one scenario verifies every semantic-profile and anchor commitment role"
    )]
    fn semantic_profile_and_anchor_are_committed_by_binding_and_route_hash() {
        let baseline = deployment(1);
        let baseline_binding =
            sccp_evm_destination_binding_hash_v1(lane().source, &baseline).expect("binding");
        let baseline_route = SccpDestinationDeploymentV1::Evm(baseline)
            .route_configuration_hash(
                lane(),
                "taira_eth_xor",
                "xor",
                1,
                SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            )
            .expect("route hash");

        let mut changed_profile = baseline;
        let SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(ref mut circuit) =
            changed_profile.outbound_proof_policy.semantic_profile;
        circuit.circuit_commitment = [0x76; 32];
        changed_profile
            .outbound_proof_policy
            .validate()
            .expect("changed profile remains valid");
        assert_ne!(
            baseline_binding,
            sccp_evm_destination_binding_hash_v1(lane().source, &changed_profile)
                .expect("changed binding")
        );
        assert_ne!(
            baseline_route,
            SccpDestinationDeploymentV1::Evm(changed_profile)
                .route_configuration_hash(
                    lane(),
                    "taira_eth_xor",
                    "xor",
                    1,
                    SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
                )
                .expect("changed route hash")
        );

        let mut changed_anchor = baseline;
        changed_anchor
            .outbound_proof_policy
            .sora_finality_anchor
            .checkpoint_height += 1;
        changed_anchor
            .outbound_proof_policy
            .validate()
            .expect("changed anchor remains valid");
        assert_ne!(
            baseline_binding,
            sccp_evm_destination_binding_hash_v1(lane().source, &changed_anchor)
                .expect("changed anchor binding")
        );
        assert_ne!(
            baseline_route,
            SccpDestinationDeploymentV1::Evm(changed_anchor)
                .route_configuration_hash(
                    lane(),
                    "taira_eth_xor",
                    "xor",
                    1,
                    SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
                )
                .expect("changed anchor route hash")
        );

        let tron_lane = SccpLaneIdV1 {
            source: SccpNetworkV1::TronMainnet,
            target: SccpNetworkV1::SoraTaira,
        };
        let baseline_tron = tron_deployment();
        let baseline_tron_binding =
            sccp_tron_destination_binding_hash_v1(tron_lane.source, &baseline_tron)
                .expect("TRON binding");
        let baseline_tron_route = SccpDestinationDeploymentV1::Tron(baseline_tron)
            .route_configuration_hash(
                tron_lane,
                "taira_tron_xor",
                "xor",
                1,
                SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            )
            .expect("TRON route hash");

        let mut changed_tron_profile = baseline_tron;
        let SccpSemanticProofProfileV1::SoraTairaFinalityInclusionGroth16Bn254(ref mut circuit) =
            changed_tron_profile.outbound_proof_policy.semantic_profile;
        circuit.circuit_commitment = [0x76; 32];
        changed_tron_profile
            .outbound_proof_policy
            .validate()
            .expect("changed TRON profile remains valid");
        assert_ne!(
            baseline_tron_binding,
            sccp_tron_destination_binding_hash_v1(tron_lane.source, &changed_tron_profile)
                .expect("changed TRON binding")
        );
        assert_ne!(
            baseline_tron_route,
            SccpDestinationDeploymentV1::Tron(changed_tron_profile)
                .route_configuration_hash(
                    tron_lane,
                    "taira_tron_xor",
                    "xor",
                    1,
                    SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
                )
                .expect("changed TRON route hash")
        );

        let mut changed_tron_anchor = baseline_tron;
        changed_tron_anchor
            .outbound_proof_policy
            .sora_finality_anchor
            .checkpoint_height += 1;
        changed_tron_anchor
            .outbound_proof_policy
            .validate()
            .expect("changed TRON anchor remains valid");
        assert_ne!(
            baseline_tron_binding,
            sccp_tron_destination_binding_hash_v1(tron_lane.source, &changed_tron_anchor)
                .expect("changed TRON anchor binding")
        );
        assert_ne!(
            baseline_tron_route,
            SccpDestinationDeploymentV1::Tron(changed_tron_anchor)
                .route_configuration_hash(
                    tron_lane,
                    "taira_tron_xor",
                    "xor",
                    1,
                    SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
                )
                .expect("changed TRON anchor route hash")
        );
    }

    #[cfg(feature = "json")]
    #[test]
    fn registry_json_rejects_unknown_fields_at_every_consensus_boundary() {
        let route = route(1, SccpRouteActivationV1::Staged);
        let valid_json = norito::json::to_json(&route).expect("route serializes");
        assert_eq!(
            norito::json::from_json::<SccpGovernedRouteV1>(&valid_json)
                .expect("valid route decodes"),
            route
        );

        for path in [
            &[][..],
            &["destination"][..],
            &["destination", "deployment"][..],
            &["destination", "deployment", "verifying_key"][..],
            &[
                "destination",
                "deployment",
                "verifying_key",
                "ic",
                "signal_10",
            ][..],
            &["source_identity"][..],
            &["source_identity", "lane"][..],
            &["source_identity", "emitter"][..],
            &["source_identity", "emitter", "identity"][..],
            &["destination", "deployment", "outbound_proof_policy"][..],
            &[
                "destination",
                "deployment",
                "outbound_proof_policy",
                "semantic_profile",
            ][..],
            &[
                "destination",
                "deployment",
                "outbound_proof_policy",
                "semantic_profile",
                "commitments",
            ][..],
            &[
                "destination",
                "deployment",
                "outbound_proof_policy",
                "sora_finality_anchor",
            ][..],
        ] {
            let mut hostile = norito::json::to_value(&route).expect("serialize route");
            insert_unknown_json_field(&mut hostile, path);
            let hostile_json = norito::json::to_json(&hostile).expect("serialize hostile route");
            let error = norito::json::from_json::<SccpGovernedRouteV1>(&hostile_json)
                .expect_err("unknown route field must fail");
            assert!(
                error.to_string().contains("adversarial_extension"),
                "unexpected error for path {path:?}: {error}"
            );
        }

        let registry = registry(vec![route], None);
        let registry_json = norito::json::to_json(&registry).expect("registry serializes");
        assert_eq!(
            norito::json::from_json::<SccpRegistryV1>(&registry_json)
                .expect("valid registry decodes"),
            registry
        );
        for path in [&[][..], &["lanes"][..]] {
            let mut hostile = norito::json::to_value(&registry).expect("serialize registry");
            if path == ["lanes"] {
                let norito::json::Value::Object(root) = &mut hostile else {
                    panic!("registry JSON is an object")
                };
                let norito::json::Value::Array(lanes) = root
                    .get_mut("lanes")
                    .expect("registry JSON carries governed lanes")
                else {
                    panic!("registry lanes JSON is an array")
                };
                insert_unknown_json_field(&mut lanes[0], &[]);
            } else {
                insert_unknown_json_field(&mut hostile, path);
            }
            let hostile_json = norito::json::to_json(&hostile).expect("serialize hostile registry");
            assert!(
                norito::json::from_json::<SccpRegistryV1>(&hostile_json).is_err(),
                "unknown registry field at {path:?} must fail"
            );
        }
    }
}
