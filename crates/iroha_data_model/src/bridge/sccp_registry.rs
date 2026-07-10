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
use thiserror::Error;

use super::{
    BridgeNativeProofBackendV1, SccpEvmSourceEmitterV1, SccpLaneIdV1, SccpNativeTrustAnchorV1,
    SccpNetworkV1, SccpSourceEmitterV1, SccpSourceIdentityV1, SccpTronSourceEmitterV1,
};
use crate::{account::AccountId, asset::AssetDefinitionId};

/// Maximum decimal scale accepted by first-release SCCP amount payloads.
pub const SCCP_V1_MAX_PAYLOAD_AMOUNT_SCALE: u32 = 28;
/// Exact decimal scale of the first-release XOR SCCP payload.
pub const SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE: u32 = 9;
/// Maximum number of complete routes in the V1 registry.
pub const SCCP_V1_MAX_GOVERNED_ROUTES: usize = 64;
/// Maximum number of exact lanes in the V1 registry.
pub const SCCP_V1_MAX_GOVERNED_LANES: usize = 16;
/// Maximum number of immutable routes sharing one lane anchor.
pub const SCCP_V1_MAX_ROUTES_PER_LANE: usize = 8;
/// Maximum byte length of a canonical SCCP route or asset key.
pub const SCCP_V1_MAX_KEY_BYTES: usize = 64;
/// Exact Taira(9-decimal) to wrapped-token(18-decimal) multiplier.
pub const SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER: u64 = 1_000_000_000;
/// Canonical live Taira XOR asset definition governed by every V1 route.
pub const SCCP_V1_TAIRA_XOR_ASSET_DEFINITION_ID: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";

const SCCP_DOMAIN_SORA: u32 = 0;
const SCCP_DOMAIN_ETH: u32 = 1;
const SCCP_DOMAIN_BSC: u32 = 2;
const SCCP_DOMAIN_TRON: u32 = 5;

const EVM_BINDING_DOMAIN_V1: &[u8] = b"iroha:sccp:evm-destination-binding:v1";
const TRON_BINDING_DOMAIN_V1: &[u8] = b"iroha:sccp:tron-destination-binding:v1";
const CONCRETE_ROUTE_CONFIG_DOMAIN_V1: &[u8] = b"sccp:concrete-route-config:v1";
const NETWORK_HASH_DOMAIN_V1: &[u8] = b"sccp:network-identity:v1";
const LANE_HASH_DOMAIN_V1: &[u8] = b"sccp:lane-id:v1";
const SOURCE_EMITTER_HASH_DOMAIN_V1: &[u8] = b"sccp:source-emitter-identity:v1";
const SOURCE_IDENTITY_HASH_DOMAIN_V1: &[u8] = b"sccp:source-identity:v1";
const STARK_FRI_PROOF_FAMILY_V1: &[u8] = b"stark-fri-v1";
const EVM_GROTH16_BACKEND_V1: &[u8] = b"evm-groth16-bn254-v1";
const TRON_GROTH16_BACKEND_V1: &[u8] = b"tron-groth16-bn254-v1";

/// BN254 base-field modulus in canonical big-endian form.
const BN254_BASE_FIELD_MODULUS_BE: [u8; 32] = [
    0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
    0x97, 0x81, 0x6a, 0x91, 0x68, 0x71, 0xca, 0x8d, 0x3c, 0x20, 0x8c, 0x16, 0xd8, 0x7c, 0xfd, 0x47,
];

const SORA_NEXUS_CHAIN_ID_BYTES: [u8; 16] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x07, 0x53,
];
const SORA_TAIRA_CHAIN_ID_BYTES: [u8; 16] = [
    0x80, 0x95, 0x74, 0xf5, 0xfe, 0xe7, 0x5e, 0x69, 0xbf, 0xcf, 0x52, 0x45, 0x1e, 0x42, 0xd5, 0x0f,
];

/// Validation failure for a closed SCCP route or registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SccpRouteValidationError {
    /// The registry version is not V1.
    #[error("SCCP registry version must be exactly 1")]
    UnsupportedRegistryVersion,
    /// The registry exceeds its deterministic consensus bound.
    #[error("SCCP registry contains more than {SCCP_V1_MAX_GOVERNED_ROUTES} routes")]
    RegistryTooLarge,
    /// The registry exceeds its deterministic lane bound.
    #[error("SCCP registry contains more than {SCCP_V1_MAX_GOVERNED_LANES} lanes")]
    TooManyLanes,
    /// One lane exceeds its deterministic route bound.
    #[error("SCCP lane contains no routes or more than {SCCP_V1_MAX_ROUTES_PER_LANE} routes")]
    InvalidLaneRouteCount,
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
}

/// Canonical non-infinity BN254 G1 point in Solidity ABI coordinate order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
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

/// Fixed Groth16 IC vector: one constant point and exactly ten signal points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
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
}

impl SccpGroth16Bn254IcV1 {
    /// Return the constant point followed by the ten public-signal points.
    #[must_use]
    pub const fn points(self) -> [SccpBn254G1PointV1; 11] {
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
        ]
    }
}

/// Closed SCCP BN254 Groth16 verification key for exactly ten public signals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
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
    /// Constant IC point followed by exactly ten public-signal IC points.
    pub ic: SccpGroth16Bn254IcV1,
}

impl SccpGroth16Bn254VerifyingKeyV1 {
    /// Validate the closed shape and canonical field encoding.
    ///
    /// Curve, non-infinity, and subgroup membership are deliberately verified
    /// by the cryptographic SCCP implementation during route registration.
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
/// `verifyingKeyHash()` preimage: 36 consecutive ABI words.
pub fn canonical_sccp_groth16_bn254_verifying_key_bytes_v1(
    verifying_key: SccpGroth16Bn254VerifyingKeyV1,
) -> Result<Vec<u8>, SccpRouteValidationError> {
    verifying_key.validate_structure()?;
    let mut out = Vec::with_capacity(36 * 32);
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
pub fn sccp_groth16_bn254_verifying_key_hash_v1(
    verifying_key: SccpGroth16Bn254VerifyingKeyV1,
) -> Result<[u8; 32], SccpRouteValidationError> {
    Ok(keccak256(
        canonical_sccp_groth16_bn254_verifying_key_bytes_v1(verifying_key)?,
    ))
}

/// Directional activation state for one complete governed SCCP route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
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

    /// Return whether a compare-and-swap transition is legal.
    #[must_use]
    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next || self.is_terminal() {
            return false;
        }
        matches!(
            (self, next),
            (Self::Staged, Self::Bidirectional | Self::Retired)
                | (Self::Bidirectional, Self::InboundOnly | Self::Paused)
                | (Self::InboundOnly, Self::Paused | Self::Retired)
                | (
                    Self::Paused,
                    Self::Bidirectional | Self::InboundOnly | Self::Retired
                )
        )
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
    /// Raw SCCP transfer-route address without the `0x41` network byte.
    pub route_address: [u8; 20],
    /// Keccak-256 hash of the governed transfer-route runtime bytecode.
    pub route_code_hash: [u8; 32],
    /// Exact Taira base-unit to wrapped-token base-unit multiplier.
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
}

impl SccpDestinationDeploymentV1 {
    /// Return the exact governed Groth16 verification-key hash when applicable.
    #[must_use]
    pub const fn groth16_verifier_key_hash(&self) -> [u8; 32] {
        match self {
            Self::Evm(deployment) => deployment.verifier_key_hash,
            Self::Tron(deployment) => deployment.verifier_key_hash,
        }
    }

    /// Validate exact family identity and role separation for an inbound lane.
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
            _ => Err(SccpRouteValidationError::DestinationFamilyMismatch),
        }
    }

    /// Return whether this deployment is valid for an exact inbound lane.
    #[must_use]
    pub fn is_well_formed_for_lane(&self, lane: SccpLaneIdV1) -> bool {
        self.validate_for_lane(lane).is_ok()
    }

    /// Derive the exact destination binding consumed by the family implementation.
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
        }
    }

    /// Derive the immutable route-configuration hash exposed by the deployment.
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
    /// Exact source-emitter identity used for native inbound admission.
    pub source_identity: SccpSourceIdentityV1,
    /// Exact reverse-direction destination deployment.
    pub destination: SccpDestinationDeploymentV1,
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
    pub fn validate(&self) -> Result<(), SccpRouteValidationError> {
        self.key().validate()?;
        validate_key("asset_key", &self.asset_key)?;
        self.settlement.validate()?;
        self.destination.validate_for_lane(self.lane_id)?;
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
        if !source_matches_destination(
            self.source_identity.emitter,
            self.destination,
            route_config_hash,
        ) {
            return Err(SccpRouteValidationError::SourceDestinationMismatch);
        }
        if self.activation.allows_inbound() && !self.supports_inbound_activation() {
            return Err(SccpRouteValidationError::UnsupportedInboundActivation);
        }
        Ok(())
    }

    /// Validate a route specifically for first registration.
    pub fn validate_registration(&self) -> Result<(), SccpRouteValidationError> {
        self.validate()?;
        if self.activation != SccpRouteActivationV1::Staged {
            return Err(SccpRouteValidationError::RegistrationMustBeStaged);
        }
        Ok(())
    }

    /// Validate the route against its lane-level native checkpoint.
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
            && (self.lane_id.target != SccpNetworkV1::SoraNexus
                || self.source_identity.has_production_source())
    }

    /// Return whether this record's selected activation is internally valid.
    #[must_use]
    pub fn activation_is_valid(&self) -> bool {
        self.validate().is_ok()
    }

    /// Derive the exact immutable route-configuration hash exposed by the
    /// destination contract.
    ///
    /// This is the single V1 route-configuration commitment recorded in
    /// outbound messages and exposed as the tenth Groth16 public signal. It
    /// must remain byte-identical to the EVM/TVM `routeConfigHash`.
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
    pub fn destination_binding_hash(&self) -> Result<[u8; 32], SccpRouteValidationError> {
        self.destination.destination_binding_hash(self.lane_id)
    }
}

/// One lane-level native checkpoint and its exact immutable routes.
///
/// Keeping the advancing anchor once per lane prevents routes sharing native
/// consensus from drifting to different finalized checkpoints.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(decode_from_slice)]
pub struct SccpGovernedLaneV1 {
    /// Exact external-to-SORA lane.
    pub lane_id: SccpLaneIdV1,
    /// Single family-tagged native checkpoint used by every inbound-active route.
    ///
    /// Staged and outbound-only lanes do not require a placeholder checkpoint.
    pub native_trust_anchor: Option<SccpNativeTrustAnchorV1>,
    /// Complete immutable routes sharing this lane checkpoint.
    pub routes: Vec<SccpGovernedRouteV1>,
}

impl SccpGovernedLaneV1 {
    /// Validate the lane checkpoint, bounded routes, and exact route membership.
    pub fn validate(&self) -> Result<(), SccpRouteValidationError> {
        validate_inbound_lane(self.lane_id)?;
        if let Some(native_trust_anchor) = self.native_trust_anchor {
            if !native_trust_anchor.is_well_formed() {
                return Err(SccpRouteValidationError::InvalidTrustAnchor);
            }
            if !native_backend_matches_family(native_trust_anchor.backend, self.lane_id.source) {
                return Err(SccpRouteValidationError::TrustAnchorFamilyMismatch);
            }
        }
        if self.routes.is_empty() || self.routes.len() > SCCP_V1_MAX_ROUTES_PER_LANE {
            return Err(SccpRouteValidationError::InvalidLaneRouteCount);
        }
        let mut lineages = BTreeMap::<(&str, &str), Vec<(u32, bool)>>::new();
        for route in &self.routes {
            route.validate_with_anchor(self.native_trust_anchor)?;
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
            for (index, (revision, _)) in revisions.iter().enumerate() {
                let expected =
                    u32::try_from(index + 1).expect("bounded SCCP route revision count fits u32");
                if *revision != expected {
                    return Err(SccpRouteValidationError::InvalidRouteRevision);
                }
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
pub struct SccpRegistryV1 {
    /// Registry format version. First release accepts exactly `1`.
    pub version: u8,
    /// Complete governed lanes, each with one checkpoint and bounded routes.
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
    /// Validate the complete bounded registry and all uniqueness invariants.
    pub fn validate(&self) -> Result<(), SccpRouteValidationError> {
        if self.version != 1 {
            return Err(SccpRouteValidationError::UnsupportedRegistryVersion);
        }
        if self.lanes.len() > SCCP_V1_MAX_GOVERNED_LANES {
            return Err(SccpRouteValidationError::TooManyLanes);
        }
        let route_count = self
            .lanes
            .iter()
            .map(|lane| lane.routes.len())
            .sum::<usize>();
        if route_count > SCCP_V1_MAX_GOVERNED_ROUTES {
            return Err(SccpRouteValidationError::RegistryTooLarge);
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
        SccpNetworkV1::SoraNexus => 0,
        SccpNetworkV1::SoraTaira => 1,
        SccpNetworkV1::EthereumMainnet => 2,
        SccpNetworkV1::EthereumSepolia => 3,
        SccpNetworkV1::BscMainnet => 4,
        SccpNetworkV1::BscTestnet => 5,
        SccpNetworkV1::TronMainnet => 10,
        SccpNetworkV1::TronNile => 11,
        SccpNetworkV1::TronShasta => 12,
    }
}

/// Return canonical V1 bytes for an exact SCCP network profile.
#[must_use]
pub fn canonical_sccp_network_bytes_v1(network: SccpNetworkV1) -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    out.push(1);
    out.push(sccp_network_tag_v1(network));
    push_u32(&mut out, network.domain_id());
    match network {
        SccpNetworkV1::SoraNexus => out.extend_from_slice(&SORA_NEXUS_CHAIN_ID_BYTES),
        SccpNetworkV1::SoraTaira => out.extend_from_slice(&SORA_TAIRA_CHAIN_ID_BYTES),
        SccpNetworkV1::EthereumMainnet => push_u64(&mut out, 1),
        SccpNetworkV1::EthereumSepolia => push_u64(&mut out, 11_155_111),
        SccpNetworkV1::BscMainnet => push_u64(&mut out, 56),
        SccpNetworkV1::BscTestnet => push_u64(&mut out, 97),
        SccpNetworkV1::TronMainnet => push_u32(&mut out, 0x2b66_53dc),
        SccpNetworkV1::TronNile => push_u32(&mut out, 0xcd86_90dc),
        SccpNetworkV1::TronShasta => push_u32(&mut out, 0x94a9_059e),
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
    let mut out = Vec::with_capacity(128);
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
    let mut payload = Vec::with_capacity(32 * 10);
    payload.extend_from_slice(&keccak256(EVM_BINDING_DOMAIN_V1));
    payload.extend_from_slice(&keccak256(EVM_GROTH16_BACKEND_V1));
    payload.extend_from_slice(&keccak256(STARK_FRI_PROOF_FAMILY_V1));
    payload.extend_from_slice(&abi_word_u64(chain_id));
    payload.extend_from_slice(&abi_word_u32(SCCP_DOMAIN_SORA));
    payload.extend_from_slice(&abi_word_u32(target_domain));
    payload.extend_from_slice(&abi_word_bytes20(deployment.verifier_address));
    payload.extend_from_slice(&abi_word_bytes20(deployment.route_address));
    payload.extend_from_slice(&deployment.verifier_code_hash);
    payload.extend_from_slice(&deployment.verifier_key_hash);
    Ok(keccak256(payload))
}

/// Derive the TRON binding using exactly the TVM Solidity `abi.encode` layout.
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
    let mut payload = Vec::with_capacity(32 * 10);
    payload.extend_from_slice(&keccak256(TRON_BINDING_DOMAIN_V1));
    payload.extend_from_slice(&keccak256(TRON_GROTH16_BACKEND_V1));
    payload.extend_from_slice(&keccak256(STARK_FRI_PROOF_FAMILY_V1));
    payload.extend_from_slice(&abi_word_u32(network_id));
    payload.extend_from_slice(&abi_word_u32(SCCP_DOMAIN_SORA));
    payload.extend_from_slice(&abi_word_u32(SCCP_DOMAIN_TRON));
    payload.extend_from_slice(&abi_word_tron_address(deployment.verifier_address));
    payload.extend_from_slice(&abi_word_tron_address(deployment.route_address));
    payload.extend_from_slice(&deployment.verifier_code_hash);
    payload.extend_from_slice(&deployment.verifier_key_hash);
    Ok(keccak256(payload))
}

/// Compute the immutable route-config hash exposed by the exact EVM XOR route.
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
    validate_hash_roles(&[
        source_lane_hash,
        destination_lane_hash,
        deployment.token_code_hash,
        deployment.verifier_code_hash,
        deployment.verifier_key_hash,
    ])?;

    let mut deployment_config = Vec::with_capacity(32 * 5);
    deployment_config.extend_from_slice(&abi_word_bytes20(deployment.token_address));
    deployment_config.extend_from_slice(&deployment.token_code_hash);
    deployment_config.extend_from_slice(&abi_word_bytes20(deployment.verifier_address));
    deployment_config.extend_from_slice(&deployment.verifier_code_hash);
    deployment_config.extend_from_slice(&deployment.verifier_key_hash);
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
    validate_hash_roles(&[
        source_lane_hash,
        destination_lane_hash,
        deployment.token_code_hash,
        deployment.verifier_code_hash,
        deployment.verifier_key_hash,
        destination_binding_hash,
    ])?;

    let mut deployment_config = Vec::with_capacity(32 * 6);
    deployment_config.extend_from_slice(&abi_word_bytes20(deployment.token_address));
    deployment_config.extend_from_slice(&deployment.token_code_hash);
    deployment_config.extend_from_slice(&abi_word_bytes20(deployment.verifier_address));
    deployment_config.extend_from_slice(&deployment.verifier_code_hash);
    deployment_config.extend_from_slice(&deployment.verifier_key_hash);
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
    validate_hash_roles(&[
        deployment.token_code_hash,
        deployment.verifier_code_hash,
        deployment.verifier_key_hash,
        deployment.route_code_hash,
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
    validate_hash_roles(&[
        deployment.token_code_hash,
        deployment.verifier_code_hash,
        deployment.verifier_key_hash,
        deployment.route_code_hash,
    ])
}

fn source_matches_destination(
    source: SccpSourceEmitterV1,
    destination: SccpDestinationDeploymentV1,
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
            },
        }
    }

    fn lane() -> SccpLaneIdV1 {
        SccpLaneIdV1 {
            source: SccpNetworkV1::EthereumSepolia,
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
            route_address: [0x51; 20],
            route_code_hash: [0x61; 32],
            taira_to_token_multiplier: SCCP_V1_TAIRA_TO_TOKEN_MULTIPLIER,
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
            source_identity: SccpSourceIdentityV1 {
                lane,
                emitter: SccpSourceEmitterV1::Evm(SccpEvmSourceEmitterV1 {
                    address: deployment.route_address,
                    runtime_code_hash: deployment.route_code_hash,
                    route_config_hash,
                }),
            },
            destination,
            settlement: SccpSoraSettlementV1 {
                asset_definition_id: sccp_v1_taira_xor_asset_definition_id(),
                custody_account_id: AccountId::new(
                    SIGNATORY.parse().expect("valid custody public key"),
                ),
                payload_amount_scale: SCCP_V1_XOR_PAYLOAD_AMOUNT_SCALE,
            },
        }
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
        SccpRegistryV1 {
            version: 1,
            lanes: vec![SccpGovernedLaneV1 {
                lane_id: lane(),
                native_trust_anchor,
                routes,
            }],
        }
    }

    #[test]
    fn solidity_verifying_key_hash_vector_is_exact() {
        let key = verifying_key();
        assert_eq!(
            canonical_sccp_groth16_bn254_verifying_key_bytes_v1(key)
                .expect("canonical key")
                .len(),
            36 * 32
        );
        assert_eq!(
            sccp_groth16_bn254_verifying_key_hash_v1(key).expect("canonical key hash"),
            hex32("51f287450cb7bcc401e07ffe5d726f13aee45f6cce5cb0c8415794d4ba47c774")
        );
    }

    #[test]
    fn network_tags_and_tron_route_hash_match_exact_contract_vectors() {
        assert_eq!(sccp_network_tag_v1(SccpNetworkV1::SoraNexus), 0);
        assert_eq!(sccp_network_tag_v1(SccpNetworkV1::SoraTaira), 1);
        assert_eq!(sccp_network_tag_v1(SccpNetworkV1::EthereumMainnet), 2);
        assert_eq!(sccp_network_tag_v1(SccpNetworkV1::EthereumSepolia), 3);
        assert_eq!(sccp_network_tag_v1(SccpNetworkV1::BscMainnet), 4);
        assert_eq!(sccp_network_tag_v1(SccpNetworkV1::BscTestnet), 5);
        assert_eq!(sccp_network_tag_v1(SccpNetworkV1::TronMainnet), 10);
        assert_eq!(sccp_network_tag_v1(SccpNetworkV1::TronNile), 11);
        assert_eq!(sccp_network_tag_v1(SccpNetworkV1::TronShasta), 12);

        // The gap at 6..=9 is deliberate. This exact byte vector is consumed
        // by SccpExactTransferCodec.tronNetwork in the deployed contracts.
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
            hex32("e6e5968272b47bc41c3a2d2d9a3cc620b2c535d58dbbfe4e2f4f31139bacd485")
        );
        assert_eq!(
            destination_lane_hash,
            hex32("7c098e461e99f423aa6ce236f53efe573fb7f43318c41afc81dda3b07d223aa0")
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
            hex32("6571ac200c92c7db53afa625984f3cbcc5d2d2490033812b4cbac84f3fa7cfc9")
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
        wrong_asset.settlement.asset_definition_id = AssetDefinitionId::new(
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

    #[cfg(feature = "json")]
    #[test]
    fn registry_json_rejects_unknown_fields_at_every_consensus_boundary() {
        let registry = registry(vec![route(1, SccpRouteActivationV1::Staged)], None);
        let mut value = norito::json::to_value(&registry).expect("serialize registry");
        let norito::json::Value::Object(root) = &mut value else {
            panic!("registry JSON is an object")
        };
        root.insert("operator_note".to_owned(), norito::json::Value::Null);
        let json = norito::json::to_json(&value).expect("serialize mutated JSON");
        let error = norito::json::from_json::<SccpRegistryV1>(&json)
            .expect_err("unknown consensus field must fail");
        assert!(error.to_string().contains("unknown field"));
    }
}
