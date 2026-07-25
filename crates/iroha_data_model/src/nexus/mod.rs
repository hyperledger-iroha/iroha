//! Nexus lane and dataspace routing types.
//!
//! These identifiers model the multi-lane/data-space routing surface described
//! in `nexus.md` and `nexus_transition_notes`. The default catalog remains a
//! single primary lane for compatibility, while deployments may add lane and
//! dataspace entries for independent routing, storage, and consensus policy.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    num::NonZeroU32,
    str::FromStr,
};

use derive_more::Display;
use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::{
    da::commitment::DaProofScheme,
    id::IdBox,
    parameter::{CustomParameter, CustomParameterId},
};
#[cfg(feature = "json")]
use iroha_primitives::json::Json;

mod axt;
mod compliance;
mod endorsement;
mod fee_sponsor_program;
mod manifest;
mod privacy;
mod relay;

pub use axt::*;
pub use compliance::*;
pub use endorsement::*;
pub use fee_sponsor_program::*;
pub use manifest::*;
pub use privacy::*;
pub mod portfolio;
pub use portfolio::*;
pub mod staking;
pub use relay::*;
pub use staking::*;

/// Consensus-wide maximum number of simultaneously active execution lanes.
///
/// This is a protocol admission bound shared by lifecycle catalogs, merge
/// execution, Native AMX manifests, and diagnostics. Sparse lane identifiers
/// may exceed this number; only the number of active catalog entries is bounded.
pub const MAX_ACTIVE_EXECUTION_LANES: usize = 1_024;

/// Declarative lane lifecycle changes (additions and retirements).
#[derive(Debug, Clone, Default, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct LaneLifecyclePlan {
    /// Lane metadata to add, or to replace when the same current lane is retired.
    pub additions: Vec<LaneConfig>,
    /// Lane identifiers to retire.
    pub retire: Vec<LaneId>,
}

/// Versioned, optimistic-concurrency envelope for a consensus-replayed lane lifecycle update.
///
/// The envelope is carried in a [`crate::isi::SetParameter`] instruction. The
/// expected catalog and active-incarnation root bind an operator request to the
/// exact topology it was reviewed against, so a delayed or concurrently
/// reordered request fails closed instead of mutating a newer topology or a
/// replacement lane that happens to reuse identical metadata.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct LaneLifecycleParameterV1 {
    /// Payload layout version. This must be [`Self::VERSION`].
    pub version: u8,
    /// Domain-separated hash of the exact committed catalog that must precede this transition.
    pub expected_catalog_hash: Hash,
    /// Domain-separated root of the exact active lane incarnations that must precede this transition.
    pub expected_incarnation_root: Hash,
    /// Declarative lane additions, replacements, and retirements.
    pub plan: LaneLifecyclePlan,
}

/// Canonical active lane-incarnation commitment advertised to lifecycle clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
pub struct LaneLifecycleIncarnationEntry {
    /// Active lane identifier.
    pub lane_id: LaneId,
    /// Non-zero commitment identifying this exact lane incarnation.
    pub incarnation: Hash,
}

/// Read-only snapshot used to construct an optimistic lane lifecycle transaction.
///
/// The status carries the exact canonical lane catalog and its domain-separated
/// commitment. Clients must validate the snapshot before embedding
/// [`Self::catalog_hash`] as
/// [`LaneLifecycleParameterV1::expected_catalog_hash`].
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct LaneLifecycleStatusV1 {
    /// Status layout version. This must be [`Self::VERSION`].
    pub version: u8,
    /// Whether Nexus lane routing is enabled on the serving node.
    pub nexus_enabled: bool,
    /// Exclusive lane-id bound for the current catalog namespace.
    pub lane_count: u32,
    /// Canonically ordered active lane metadata.
    pub lanes: Vec<LaneConfig>,
    /// Domain-separated commitment to `lane_count` and `lanes`.
    pub catalog_hash: Hash,
    /// Canonically ordered active lane-incarnation commitments.
    pub incarnations: Vec<LaneLifecycleIncarnationEntry>,
    /// Domain-separated commitment to `incarnations`.
    pub incarnation_root: Hash,
}

impl LaneLifecycleStatusV1 {
    /// Supported status layout version.
    pub const VERSION: u8 = 1;

    /// Construct a status snapshot from the exact current catalog.
    ///
    /// # Errors
    ///
    /// Returns [`LaneLifecycleStatusError`] when the incarnation map does not
    /// exactly and canonically cover the active catalog.
    pub fn new(
        nexus_enabled: bool,
        catalog: &LaneCatalog,
        incarnations: &BTreeMap<LaneId, Hash>,
    ) -> Result<Self, LaneLifecycleStatusError> {
        let incarnations = LaneLifecycleParameterV1::canonical_incarnations(catalog, incarnations)?;
        Ok(Self {
            version: Self::VERSION,
            nexus_enabled,
            lane_count: catalog.lane_count().get(),
            lanes: catalog.lanes().to_vec(),
            catalog_hash: LaneLifecycleParameterV1::catalog_hash(catalog),
            incarnation_root: LaneLifecycleParameterV1::incarnation_root(&incarnations),
            incarnations,
        })
    }

    /// Validate the version, catalog structure, canonical order, and commitment.
    ///
    /// # Errors
    /// Returns [`LaneLifecycleStatusError`] when the snapshot is unsupported,
    /// malformed, non-canonical, or carries a forged/stale catalog commitment.
    pub fn validate(&self) -> Result<LaneCatalog, LaneLifecycleStatusError> {
        if self.version != Self::VERSION {
            return Err(LaneLifecycleStatusError::UnsupportedVersion {
                actual: self.version,
                expected: Self::VERSION,
            });
        }
        let lane_count =
            NonZeroU32::new(self.lane_count).ok_or(LaneLifecycleStatusError::ZeroLaneCount)?;
        let catalog = LaneCatalog::new(lane_count, self.lanes.clone())?;
        if catalog.lanes() != self.lanes.as_slice() {
            return Err(LaneLifecycleStatusError::NonCanonicalLaneOrder);
        }
        let expected = LaneLifecycleParameterV1::catalog_hash(&catalog);
        if self.catalog_hash != expected {
            return Err(LaneLifecycleStatusError::CatalogHashMismatch {
                advertised: self.catalog_hash,
                computed: expected,
            });
        }
        LaneLifecycleParameterV1::validate_incarnations(&catalog, &self.incarnations)?;
        let expected_root = LaneLifecycleParameterV1::incarnation_root(&self.incarnations);
        if self.incarnation_root != expected_root {
            return Err(LaneLifecycleStatusError::IncarnationRootMismatch {
                advertised: self.incarnation_root,
                computed: expected_root,
            });
        }
        Ok(catalog)
    }
}

/// Validation failures for a read-only lane lifecycle status snapshot.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum LaneLifecycleStatusError {
    /// The server advertised an unsupported status layout version.
    #[error("unsupported Nexus lane lifecycle status version {actual}; expected {expected}")]
    UnsupportedVersion {
        /// Advertised version.
        actual: u8,
        /// Version understood by this client.
        expected: u8,
    },
    /// A catalog namespace cannot have a zero exclusive bound.
    #[error("Nexus lane lifecycle status advertised a zero lane count")]
    ZeroLaneCount,
    /// Lane entries were not ordered canonically by lane identifier.
    #[error("Nexus lane lifecycle status lane entries are not canonically ordered")]
    NonCanonicalLaneOrder,
    /// The advertised catalog commitment did not bind the supplied catalog.
    #[error(
        "Nexus lane lifecycle status catalog hash mismatch: advertised {advertised}, computed {computed}"
    )]
    CatalogHashMismatch {
        /// Hash supplied by the server.
        advertised: Hash,
        /// Hash computed from the supplied catalog.
        computed: Hash,
    },
    /// Active incarnation entries were not strictly ordered by lane identifier.
    #[error("Nexus lane lifecycle incarnation entries are not canonically ordered")]
    NonCanonicalIncarnationOrder,
    /// Active incarnation entries did not cover exactly the catalog's lane identifiers.
    #[error("Nexus lane lifecycle incarnation lane ids do not exactly match the active catalog")]
    IncarnationLaneSetMismatch,
    /// An active lane advertised an all-zero incarnation commitment.
    #[error("Nexus lane lifecycle status lane {lane_id} has an all-zero incarnation")]
    ZeroIncarnation {
        /// Lane carrying the invalid commitment.
        lane_id: LaneId,
    },
    /// Two active lanes reused the same incarnation commitment.
    #[error("Nexus lane lifecycle status lane {lane_id} reuses an active incarnation")]
    DuplicateIncarnation {
        /// Later lane carrying the duplicate commitment.
        lane_id: LaneId,
    },
    /// The incarnation root did not bind the advertised active entries.
    #[error(
        "Nexus lane lifecycle status incarnation root mismatch: advertised {advertised}, computed {computed}"
    )]
    IncarnationRootMismatch {
        /// Root supplied by the server.
        advertised: Hash,
        /// Root computed from the supplied entries.
        computed: Hash,
    },
    /// The supplied lane metadata did not form a valid catalog.
    #[error(transparent)]
    InvalidCatalog(#[from] LaneCatalogError),
}

impl LaneLifecycleParameterV1 {
    /// Supported payload layout version.
    pub const VERSION: u8 = 1;
    /// Reserved custom-parameter identifier for consensus lane lifecycle changes.
    pub const PARAMETER_ID_STR: &'static str = "nexus_lane_lifecycle_v1";

    /// Construct a lifecycle parameter bound to the exact catalog and incarnations.
    ///
    /// # Errors
    ///
    /// Returns [`LaneLifecycleStatusError`] when the incarnation entries do not
    /// exactly and canonically cover the expected catalog.
    pub fn new(
        expected_catalog: &LaneCatalog,
        expected_incarnations: &[LaneLifecycleIncarnationEntry],
        plan: LaneLifecyclePlan,
    ) -> Result<Self, LaneLifecycleStatusError> {
        Self::validate_incarnations(expected_catalog, expected_incarnations)?;
        Ok(Self {
            version: Self::VERSION,
            expected_catalog_hash: Self::catalog_hash(expected_catalog),
            expected_incarnation_root: Self::incarnation_root(expected_incarnations),
            plan,
        })
    }

    /// Compute the canonical, domain-separated commitment for a lane catalog.
    #[must_use]
    pub fn catalog_hash(catalog: &LaneCatalog) -> Hash {
        const DOMAIN: &[u8] = b"iroha:nexus:lane-catalog:v1\0";
        let encoded = (catalog.lane_count().get(), catalog.lanes().to_vec()).encode();
        Hash::new_from_chunks(&[DOMAIN, encoded.as_slice()])
    }

    /// Convert the active incarnation map to its canonical exact catalog order.
    ///
    /// # Errors
    ///
    /// Returns [`LaneLifecycleStatusError`] when the map does not exactly cover
    /// the catalog, contains a zero or duplicate incarnation, or cannot be
    /// represented in canonical lane-id order.
    pub fn canonical_incarnations(
        catalog: &LaneCatalog,
        incarnations: &BTreeMap<LaneId, Hash>,
    ) -> Result<Vec<LaneLifecycleIncarnationEntry>, LaneLifecycleStatusError> {
        let entries = incarnations
            .iter()
            .map(|(&lane_id, &incarnation)| LaneLifecycleIncarnationEntry {
                lane_id,
                incarnation,
            })
            .collect::<Vec<_>>();
        Self::validate_incarnations(catalog, &entries)?;
        Ok(entries)
    }

    /// Validate exact coverage, canonical ordering, non-zero values, and uniqueness.
    ///
    /// # Errors
    ///
    /// Returns [`LaneLifecycleStatusError`] when any invariant is violated.
    pub fn validate_incarnations(
        catalog: &LaneCatalog,
        incarnations: &[LaneLifecycleIncarnationEntry],
    ) -> Result<(), LaneLifecycleStatusError> {
        let expected_ids = catalog
            .lanes()
            .iter()
            .map(|lane| lane.id)
            .collect::<Vec<_>>();
        let actual_ids = incarnations
            .iter()
            .map(|entry| entry.lane_id)
            .collect::<Vec<_>>();
        if actual_ids.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(LaneLifecycleStatusError::NonCanonicalIncarnationOrder);
        }
        if actual_ids != expected_ids {
            return Err(LaneLifecycleStatusError::IncarnationLaneSetMismatch);
        }
        let mut unique = BTreeSet::new();
        for entry in incarnations {
            if entry.incarnation.as_ref().iter().all(|byte| *byte == 0) {
                return Err(LaneLifecycleStatusError::ZeroIncarnation {
                    lane_id: entry.lane_id,
                });
            }
            if !unique.insert(entry.incarnation) {
                return Err(LaneLifecycleStatusError::DuplicateIncarnation {
                    lane_id: entry.lane_id,
                });
            }
        }
        Ok(())
    }

    /// Compute the domain-separated commitment to canonical incarnation entries.
    #[must_use]
    pub fn incarnation_root(incarnations: &[LaneLifecycleIncarnationEntry]) -> Hash {
        const DOMAIN: &[u8] = b"iroha:nexus:lane-incarnations:v1\0";
        let encoded = incarnations.to_vec().encode();
        Hash::new_from_chunks(&[DOMAIN, encoded.as_slice()])
    }

    /// Identifier used by the on-chain custom parameter.
    #[must_use]
    pub fn parameter_id() -> CustomParameterId {
        Self::PARAMETER_ID_STR
            .parse()
            .expect("valid Nexus lane lifecycle custom parameter identifier")
    }

    /// Convert this envelope into the custom parameter accepted by `SetParameter`.
    #[cfg(feature = "json")]
    #[must_use]
    pub fn into_custom_parameter(self) -> CustomParameter {
        CustomParameter::new(Self::parameter_id(), Json::new(self))
    }

    /// Decode a matching lifecycle custom parameter.
    ///
    /// Non-matching parameter identifiers return `Ok(None)`. Matching identifiers
    /// are parsed strictly and reject unsupported versions.
    ///
    /// # Errors
    ///
    /// Returns [`norito::json::Error`] when a matching payload is malformed or
    /// carries an unsupported lifecycle version.
    #[cfg(feature = "json")]
    pub fn from_custom_parameter(
        custom: &CustomParameter,
    ) -> Result<Option<Self>, norito::json::Error> {
        if custom.id != Self::parameter_id() {
            return Ok(None);
        }
        let payload = norito::json::from_str::<Self>(custom.payload().get())?;
        if payload.version != Self::VERSION {
            return Err(norito::json::Error::Message(format!(
                "unsupported Nexus lane lifecycle parameter version {}; expected {}",
                payload.version,
                Self::VERSION
            )));
        }
        Ok(Some(payload))
    }
}

/// Identifier for a logical execution lane.
#[derive(
    Debug,
    Display,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    IntoSchema,
)]
#[repr(transparent)]
#[norito(transparent, decode_from_slice)]
#[cfg_attr(
    any(feature = "ffi_export", feature = "ffi_import"),
    ffi_type(unsafe {robust})
)]
pub struct LaneId(u32);

/// Identifier for a storage shard serving one or more lanes.
///
/// Shards map to physical DA/Kura partitions; today they track lane bindings
/// one-to-one but remain distinct to allow future resharding.
#[derive(
    Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema,
)]
#[repr(transparent)]
#[norito(transparent, decode_from_slice)]
#[cfg_attr(
    any(feature = "ffi_export", feature = "ffi_import"),
    ffi_type(unsafe {robust})
)]
pub struct ShardId(u32);

impl LaneId {
    /// Canonical primary lane identifier used by the default single-lane catalog.
    pub const SINGLE: Self = Self(0);

    /// Construct a [`LaneId`] from a zero-based lane index constrained by the provided lane count.
    ///
    /// # Errors
    /// Returns [`LaneIdError::OutOfBounds`] when the lane index is not representable with the
    /// configured number of lanes.
    pub fn from_lane_index(index: u32, lane_count: NonZeroU32) -> Result<Self, LaneIdError> {
        if index < lane_count.get() {
            Ok(Self(index))
        } else {
            Err(LaneIdError::OutOfBounds {
                index,
                lane_count: lane_count.get(),
            })
        }
    }

    /// Create a `LaneId` from its raw numeric representation.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Expose the inner numeric representation.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl From<u32> for LaneId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<LaneId> for u64 {
    fn from(value: LaneId) -> Self {
        u64::from(value.0)
    }
}

impl crate::Identifiable for LaneId {
    type Id = LaneId;

    fn id(&self) -> &Self::Id {
        self
    }
}

impl ShardId {
    /// Construct a `ShardId` from its raw numeric representation.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Expose the inner numeric representation.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

impl From<u32> for ShardId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<ShardId> for u32 {
    fn from(value: ShardId) -> Self {
        value.0
    }
}

impl From<ShardId> for u64 {
    fn from(value: ShardId) -> Self {
        u64::from(value.0)
    }
}

impl From<LaneId> for ShardId {
    fn from(value: LaneId) -> Self {
        Self(value.as_u32())
    }
}

impl From<ShardId> for LaneId {
    fn from(value: ShardId) -> Self {
        Self::new(value.as_u32())
    }
}

impl From<ShardId> for IdBox {
    fn from(value: ShardId) -> Self {
        IdBox::LaneId(value.into())
    }
}

impl crate::Identifiable for ShardId {
    type Id = ShardId;

    fn id(&self) -> &Self::Id {
        self
    }
}

/// Errors returned when deriving a lane identifier from configuration.
#[derive(Debug, Copy, Clone, Error, PartialEq, Eq)]
pub enum LaneIdError {
    /// Provided index exceeds the configured number of lanes.
    #[error("lane index {index} out of bounds for lane count {lane_count}")]
    OutOfBounds {
        /// Lane index that triggered the error.
        index: u32,
        /// Total number of configured lanes.
        lane_count: u32,
    },
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for LaneId {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.0, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for LaneId {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_u64()?;
        let value = u32::try_from(value)
            .map_err(|_| norito::json::Error::Message("lane id overflow".into()))?;
        Ok(Self(value))
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for ShardId {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.0, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ShardId {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_u64()?;
        let value = u32::try_from(value)
            .map_err(|_| norito::json::Error::Message("shard id overflow".into()))?;
        Ok(Self(value))
    }
}

/// Identifier for a data space.
#[derive(
    Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema,
)]
#[repr(transparent)]
#[norito(transparent, decode_from_slice)]
#[cfg_attr(
    any(feature = "ffi_export", feature = "ffi_import"),
    ffi_type(unsafe {robust})
)]
pub struct DataSpaceId(u64);

impl DataSpaceId {
    /// Identifier for the reserved `universal` data space.
    pub const UNIVERSAL: Self = Self(0);

    /// Derive a [`DataSpaceId`] from a stable 32-byte hash.
    #[must_use]
    pub const fn from_hash(hash: &[u8; 32]) -> Self {
        let mut buf = [0u8; 8];
        let mut idx = 0;
        while idx < 8 {
            buf[idx] = hash[idx];
            idx += 1;
        }
        Self(u64::from_le_bytes(buf))
    }

    /// Create a `DataSpaceId` from its raw numeric representation.
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Expose the inner numeric representation.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl Default for DataSpaceId {
    fn default() -> Self {
        Self::UNIVERSAL
    }
}

impl From<u64> for DataSpaceId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<DataSpaceId> for u64 {
    fn from(value: DataSpaceId) -> Self {
        value.0
    }
}

impl FromStr for DataSpaceId {
    type Err = std::num::ParseIntError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse::<u64>().map(Self)
    }
}

/// Metadata key marking a lane as created and owned by the deterministic autoscaler.
pub const AUTOSCALE_META_MANAGED: &str = "autoscale.managed";
/// Metadata key recording the block height where the autoscaler created the lane.
pub const AUTOSCALE_META_CREATED_HEIGHT: &str = "autoscale.created_height";
/// Metadata key carrying the consensus-persisted two-phase lane drain state.
pub const AUTOSCALE_META_DRAIN_STATE: &str = "autoscale.drain_state";
/// Metadata key pinning the authoritative committee for one elastic-lane incarnation.
pub const AUTOSCALE_META_COMMITTEE: &str = "autoscale.committee_v1";

/// Metadata describing an execution lane.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct LaneConfig {
    /// Lane identifier.
    pub id: LaneId,
    /// Dataspace the lane belongs to.
    pub dataspace_id: DataSpaceId,
    /// Human-friendly alias.
    pub alias: String,
    /// Optional description for dashboards and docs.
    pub description: Option<String>,
    /// Declarative visibility profile.
    pub visibility: LaneVisibility,
    /// Lane profile/type (`default_public`, `cbdc_private`, etc.).
    pub lane_type: Option<String>,
    /// Governance policy identifier.
    pub governance: Option<String>,
    /// Settlement/fee policy identifier.
    pub settlement: Option<String>,
    /// Storage profile bound to this lane.
    pub storage: LaneStorageProfile,
    /// Proof scheme used for DA commitments on this lane.
    pub proof_scheme: DaProofScheme,
    /// Arbitrary metadata key-value pairs.
    pub metadata: BTreeMap<String, String>,
}

impl Default for LaneConfig {
    fn default() -> Self {
        Self {
            id: LaneId::SINGLE,
            dataspace_id: DataSpaceId::UNIVERSAL,
            alias: "default".to_string(),
            description: None,
            visibility: LaneVisibility::Public,
            lane_type: None,
            governance: None,
            settlement: None,
            storage: LaneStorageProfile::FullReplica,
            proof_scheme: DaProofScheme::default(),
            metadata: BTreeMap::new(),
        }
    }
}

impl LaneConfig {
    /// Return `true` when this lane uses the reserved autoscale ownership metadata key.
    #[must_use]
    pub fn claims_autoscale_managed(&self) -> bool {
        self.metadata.contains_key(AUTOSCALE_META_MANAGED)
    }

    /// Parse the positive autoscale creation height marker, when present and valid.
    #[must_use]
    pub fn autoscale_created_height(&self) -> Option<u64> {
        self.metadata
            .get(AUTOSCALE_META_CREATED_HEIGHT)
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|height| *height > 0)
    }

    /// Return `true` when a consensus autoscale drain state is attached.
    #[must_use]
    pub fn has_autoscale_drain_state(&self) -> bool {
        self.metadata.contains_key(AUTOSCALE_META_DRAIN_STATE)
    }

    /// Return `true` when a consensus-pinned incarnation committee is attached.
    #[must_use]
    pub fn has_autoscale_committee(&self) -> bool {
        self.metadata.contains_key(AUTOSCALE_META_COMMITTEE)
    }

    /// Return `true` when this lane is a valid deterministic autoscale elastic lane.
    #[must_use]
    pub fn is_autoscale_managed_elastic(&self) -> bool {
        self.visibility == LaneVisibility::Public
            && self
                .metadata
                .get(AUTOSCALE_META_MANAGED)
                .is_some_and(|value| value == "true")
            && self.alias == format!("elastic-lane-{}", self.id.as_u32())
            && self.autoscale_created_height().is_some()
    }

    /// Return `true` when this lane inherits the functional autoscale profile of `base`.
    ///
    /// Elastic lanes have their own identifier, alias, description, and reserved autoscale
    /// metadata. All routing, security, storage, proof, and operator-defined metadata must remain
    /// identical to the routing default lane they scale.
    #[must_use]
    pub fn inherits_autoscale_profile_from(&self, base: &Self) -> bool {
        self.dataspace_id == base.dataspace_id
            && self.visibility == base.visibility
            && self.lane_type == base.lane_type
            && self.governance == base.governance
            && self.settlement == base.settlement
            && self.storage == base.storage
            && self.proof_scheme == base.proof_scheme
            && self
                .metadata
                .iter()
                .filter(|(key, _)| !is_reserved_autoscale_metadata_key(key.as_str()))
                .eq(base
                    .metadata
                    .iter()
                    .filter(|(key, _)| !is_reserved_autoscale_metadata_key(key.as_str())))
    }
}

fn is_reserved_autoscale_metadata_key(key: &str) -> bool {
    matches!(
        key,
        AUTOSCALE_META_MANAGED
            | AUTOSCALE_META_CREATED_HEIGHT
            | AUTOSCALE_META_DRAIN_STATE
            | AUTOSCALE_META_COMMITTEE
    )
}

/// Declarative visibility profile for a lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema, Display)]
pub enum LaneVisibility {
    /// Lane is discoverable without authentication.
    #[display("public")]
    Public,
    /// Lane requires explicit admission for visibility.
    #[display("restricted")]
    Restricted,
}

impl LaneVisibility {
    /// Returns the canonical string representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Restricted => "restricted",
        }
    }
}

impl FromStr for LaneVisibility {
    type Err = LaneVisibilityParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "public" => Ok(Self::Public),
            "restricted" => Ok(Self::Restricted),
            other => Err(LaneVisibilityParseError(other.to_string())),
        }
    }
}

#[allow(clippy::derivable_impls)]
impl Default for LaneVisibility {
    fn default() -> Self {
        Self::Public
    }
}

/// Storage profile describing how state/WAL data is persisted for a lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub enum LaneStorageProfile {
    /// Full state replication (state + WAL) is retained by the lane.
    FullReplica,
    /// Only commitment metadata is persisted globally (lane retains private state locally).
    CommitmentOnly,
    /// Encrypted payloads and commitments are stored separately.
    SplitReplica,
}

impl LaneStorageProfile {
    /// Returns the canonical string representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FullReplica => "full_replica",
            Self::CommitmentOnly => "commitment_only",
            Self::SplitReplica => "split_replica",
        }
    }
}

impl fmt::Display for LaneStorageProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LaneStorageProfile {
    type Err = LaneStorageProfileParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "full_replica" => Ok(Self::FullReplica),
            "commitment_only" => Ok(Self::CommitmentOnly),
            "split_replica" => Ok(Self::SplitReplica),
            other => Err(LaneStorageProfileParseError(other.to_string())),
        }
    }
}

#[allow(clippy::derivable_impls)]
impl Default for LaneStorageProfile {
    fn default() -> Self {
        Self::FullReplica
    }
}

/// Error surfaced when parsing [`LaneVisibility`] from a string.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid lane visibility `{0}`")]
pub struct LaneVisibilityParseError(pub String);

/// Error surfaced when parsing [`LaneStorageProfile`] from a string.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("invalid lane storage profile `{0}`")]
pub struct LaneStorageProfileParseError(pub String);

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for LaneVisibility {
    fn write_json(&self, out: &mut String) {
        out.push('"');
        out.push_str(self.as_str());
        out.push('"');
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for LaneVisibility {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        value
            .parse()
            .map_err(|err: LaneVisibilityParseError| norito::json::Error::Message(err.to_string()))
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for LaneStorageProfile {
    fn write_json(&self, out: &mut String) {
        out.push('"');
        out.push_str(self.as_str());
        out.push('"');
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for LaneStorageProfile {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        value.parse().map_err(|err: LaneStorageProfileParseError| {
            norito::json::Error::Message(err.to_string())
        })
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for LaneConfig {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        norito::json::write_json_string("id", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.id, out);
        out.push(',');
        norito::json::write_json_string("dataspace_id", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.dataspace_id, out);
        out.push(',');
        norito::json::write_json_string("alias", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.alias, out);
        out.push(',');
        norito::json::write_json_string("description", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.description, out);
        out.push(',');
        norito::json::write_json_string("visibility", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.visibility, out);
        out.push(',');
        norito::json::write_json_string("lane_type", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.lane_type, out);
        out.push(',');
        norito::json::write_json_string("governance", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.governance, out);
        out.push(',');
        norito::json::write_json_string("settlement", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.settlement, out);
        out.push(',');
        norito::json::write_json_string("storage", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.storage, out);
        out.push(',');
        norito::json::write_json_string("proof_scheme", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.proof_scheme.to_string(), out);
        out.push(',');
        norito::json::write_json_string("metadata", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.metadata, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for LaneConfig {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        use norito::json::MapVisitor;

        let mut visitor = MapVisitor::new(parser)?;
        let mut lane = LaneConfig::default();
        let mut saw_id = false;
        let mut saw_alias = false;

        while let Some(key) = visitor.next_key()? {
            match key.as_str() {
                "id" => {
                    lane.id = visitor.parse_value()?;
                    saw_id = true;
                }
                "dataspace_id" => {
                    lane.dataspace_id = visitor.parse_value()?;
                }
                "alias" => {
                    lane.alias = visitor.parse_value()?;
                    saw_alias = true;
                }
                "description" => {
                    lane.description = visitor.parse_value()?;
                }
                "visibility" => {
                    lane.visibility = visitor.parse_value()?;
                }
                "lane_type" => {
                    lane.lane_type = visitor.parse_value()?;
                }
                "governance" => {
                    lane.governance = visitor.parse_value()?;
                }
                "settlement" => {
                    lane.settlement = visitor.parse_value()?;
                }
                "storage" => {
                    lane.storage = visitor.parse_value()?;
                }
                "proof_scheme" => {
                    let raw: String = visitor.parse_value()?;
                    lane.proof_scheme = raw.parse().map_err(|err| {
                        norito::json::Error::Message(format!(
                            "invalid lane proof_scheme `{raw}`: {err}"
                        ))
                    })?;
                }
                "metadata" => {
                    lane.metadata = visitor.parse_value()?;
                }
                other => {
                    return Err(norito::json::Error::Message(format!(
                        "unknown field `{other}` in lane metadata"
                    )));
                }
            }
        }
        visitor.finish()?;

        if !saw_id {
            return Err(norito::json::Error::Message(
                "missing required lane metadata field `id`".into(),
            ));
        }
        if !saw_alias {
            return Err(norito::json::Error::Message(
                "missing required lane metadata field `alias`".into(),
            ));
        }
        Ok(lane)
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for LaneLifecyclePlan {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        norito::json::write_json_string("additions", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.additions, out);
        out.push(',');
        norito::json::write_json_string("retire", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.retire, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for LaneLifecyclePlan {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        use norito::json::MapVisitor;

        let mut visitor = MapVisitor::new(parser)?;
        let mut additions: Option<Vec<LaneConfig>> = None;
        let mut retire: Option<Vec<LaneId>> = None;

        while let Some(key) = visitor.next_key()? {
            match key.as_str() {
                "additions" => {
                    if additions.is_some() {
                        return Err(norito::json::Error::Message(
                            "duplicate field `additions` in lane lifecycle plan".into(),
                        ));
                    }
                    additions = Some(visitor.parse_value()?);
                }
                "retire" => {
                    if retire.is_some() {
                        return Err(norito::json::Error::Message(
                            "duplicate field `retire` in lane lifecycle plan".into(),
                        ));
                    }
                    retire = Some(visitor.parse_value()?);
                }
                other => {
                    return Err(norito::json::Error::Message(format!(
                        "unknown field `{other}` in lane lifecycle plan"
                    )));
                }
            }
        }
        visitor.finish()?;

        Ok(Self {
            additions: additions.unwrap_or_default(),
            retire: retire.unwrap_or_default(),
        })
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for LaneLifecycleParameterV1 {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        norito::json::write_json_string("version", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.version, out);
        out.push(',');
        norito::json::write_json_string("expected_catalog_hash", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.expected_catalog_hash, out);
        out.push(',');
        norito::json::write_json_string("expected_incarnation_root", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.expected_incarnation_root, out);
        out.push(',');
        norito::json::write_json_string("plan", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.plan, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for LaneLifecycleParameterV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        use norito::json::MapVisitor;

        let mut visitor = MapVisitor::new(parser)?;
        let mut version = None;
        let mut expected_catalog_hash = None;
        let mut expected_incarnation_root = None;
        let mut plan = None;

        while let Some(key) = visitor.next_key()? {
            match key.as_str() {
                "version" => {
                    if version.is_some() {
                        return Err(norito::json::Error::Message(
                            "duplicate field `version` in Nexus lane lifecycle parameter".into(),
                        ));
                    }
                    version = Some(visitor.parse_value()?);
                }
                "expected_catalog_hash" => {
                    if expected_catalog_hash.is_some() {
                        return Err(norito::json::Error::Message(
                            "duplicate field `expected_catalog_hash` in Nexus lane lifecycle parameter"
                                .into(),
                        ));
                    }
                    expected_catalog_hash = Some(visitor.parse_value()?);
                }
                "plan" => {
                    if plan.is_some() {
                        return Err(norito::json::Error::Message(
                            "duplicate field `plan` in Nexus lane lifecycle parameter".into(),
                        ));
                    }
                    plan = Some(visitor.parse_value()?);
                }
                "expected_incarnation_root" => {
                    if expected_incarnation_root.is_some() {
                        return Err(norito::json::Error::Message(
                            "duplicate field `expected_incarnation_root` in Nexus lane lifecycle parameter"
                                .into(),
                        ));
                    }
                    expected_incarnation_root = Some(visitor.parse_value()?);
                }
                other => {
                    return Err(norito::json::Error::Message(format!(
                        "unknown field `{other}` in Nexus lane lifecycle parameter"
                    )));
                }
            }
        }
        visitor.finish()?;

        Ok(Self {
            version: version.ok_or_else(|| {
                norito::json::Error::Message(
                    "missing required Nexus lane lifecycle field `version`".into(),
                )
            })?,
            expected_catalog_hash: expected_catalog_hash.ok_or_else(|| {
                norito::json::Error::Message(
                    "missing required Nexus lane lifecycle field `expected_catalog_hash`".into(),
                )
            })?,
            expected_incarnation_root: expected_incarnation_root.ok_or_else(|| {
                norito::json::Error::Message(
                    "missing required Nexus lane lifecycle field `expected_incarnation_root`"
                        .into(),
                )
            })?,
            plan: plan.ok_or_else(|| {
                norito::json::Error::Message(
                    "missing required Nexus lane lifecycle field `plan`".into(),
                )
            })?,
        })
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for LaneLifecycleIncarnationEntry {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        norito::json::write_json_string("lane_id", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.lane_id, out);
        out.push(',');
        norito::json::write_json_string("incarnation", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.incarnation, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for LaneLifecycleIncarnationEntry {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        use norito::json::MapVisitor;

        let mut visitor = MapVisitor::new(parser)?;
        let mut lane_id = None;
        let mut incarnation = None;
        while let Some(key) = visitor.next_key()? {
            match key.as_str() {
                "lane_id" => {
                    if lane_id.is_some() {
                        return Err(norito::json::Error::Message(
                            "duplicate field `lane_id` in lane lifecycle incarnation".into(),
                        ));
                    }
                    lane_id = Some(visitor.parse_value()?);
                }
                "incarnation" => {
                    if incarnation.is_some() {
                        return Err(norito::json::Error::Message(
                            "duplicate field `incarnation` in lane lifecycle incarnation".into(),
                        ));
                    }
                    incarnation = Some(visitor.parse_value()?);
                }
                other => {
                    return Err(norito::json::Error::Message(format!(
                        "unknown field `{other}` in lane lifecycle incarnation"
                    )));
                }
            }
        }
        visitor.finish()?;
        Ok(Self {
            lane_id: lane_id.ok_or_else(|| {
                norito::json::Error::Message(
                    "missing required lane lifecycle incarnation field `lane_id`".into(),
                )
            })?,
            incarnation: incarnation.ok_or_else(|| {
                norito::json::Error::Message(
                    "missing required lane lifecycle incarnation field `incarnation`".into(),
                )
            })?,
        })
    }
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for LaneLifecycleStatusV1 {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        norito::json::write_json_string("version", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.version, out);
        out.push(',');
        norito::json::write_json_string("nexus_enabled", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.nexus_enabled, out);
        out.push(',');
        norito::json::write_json_string("lane_count", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.lane_count, out);
        out.push(',');
        norito::json::write_json_string("lanes", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.lanes, out);
        out.push(',');
        norito::json::write_json_string("catalog_hash", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.catalog_hash, out);
        out.push(',');
        norito::json::write_json_string("incarnations", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.incarnations, out);
        out.push(',');
        norito::json::write_json_string("incarnation_root", out);
        out.push(':');
        norito::json::JsonSerialize::json_serialize(&self.incarnation_root, out);
        out.push('}');
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for LaneLifecycleStatusV1 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        use norito::json::MapVisitor;

        let mut visitor = MapVisitor::new(parser)?;
        let mut version = None;
        let mut nexus_enabled = None;
        let mut lane_count = None;
        let mut lanes = None;
        let mut catalog_hash = None;
        let mut incarnations = None;
        let mut incarnation_root = None;

        while let Some(key) = visitor.next_key()? {
            let duplicate = |field: &str| {
                norito::json::Error::Message(format!(
                    "duplicate field `{field}` in Nexus lane lifecycle status"
                ))
            };
            match key.as_str() {
                "version" => {
                    if version.is_some() {
                        return Err(duplicate("version"));
                    }
                    version = Some(visitor.parse_value()?);
                }
                "nexus_enabled" => {
                    if nexus_enabled.is_some() {
                        return Err(duplicate("nexus_enabled"));
                    }
                    nexus_enabled = Some(visitor.parse_value()?);
                }
                "lane_count" => {
                    if lane_count.is_some() {
                        return Err(duplicate("lane_count"));
                    }
                    lane_count = Some(visitor.parse_value()?);
                }
                "lanes" => {
                    if lanes.is_some() {
                        return Err(duplicate("lanes"));
                    }
                    lanes = Some(visitor.parse_value()?);
                }
                "catalog_hash" => {
                    if catalog_hash.is_some() {
                        return Err(duplicate("catalog_hash"));
                    }
                    catalog_hash = Some(visitor.parse_value()?);
                }
                "incarnations" => {
                    if incarnations.is_some() {
                        return Err(duplicate("incarnations"));
                    }
                    incarnations = Some(visitor.parse_value()?);
                }
                "incarnation_root" => {
                    if incarnation_root.is_some() {
                        return Err(duplicate("incarnation_root"));
                    }
                    incarnation_root = Some(visitor.parse_value()?);
                }
                other => {
                    return Err(norito::json::Error::Message(format!(
                        "unknown field `{other}` in Nexus lane lifecycle status"
                    )));
                }
            }
        }
        visitor.finish()?;

        let missing = |field: &str| {
            norito::json::Error::Message(format!(
                "missing required Nexus lane lifecycle status field `{field}`"
            ))
        };
        Ok(Self {
            version: version.ok_or_else(|| missing("version"))?,
            nexus_enabled: nexus_enabled.ok_or_else(|| missing("nexus_enabled"))?,
            lane_count: lane_count.ok_or_else(|| missing("lane_count"))?,
            lanes: lanes.ok_or_else(|| missing("lanes"))?,
            catalog_hash: catalog_hash.ok_or_else(|| missing("catalog_hash"))?,
            incarnations: incarnations.ok_or_else(|| missing("incarnations"))?,
            incarnation_root: incarnation_root.ok_or_else(|| missing("incarnation_root"))?,
        })
    }
}

/// Validated catalog of configured lanes.
///
/// `lane_count` is the exclusive identifier bound for the current namespace, not the number of
/// active entries. Catalogs may be sparse, and lifecycle additions can expand the bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneCatalog {
    lane_count: NonZeroU32,
    lanes: Vec<LaneConfig>,
}

impl LaneCatalog {
    /// Build a catalog ensuring identifiers and aliases are unique and in range.
    ///
    /// # Errors
    /// Returns a [`LaneCatalogError`] when lane metadata violates alias uniqueness, identifier
    /// uniqueness, exceeds the configured lane count, or exceeds the consensus-wide active-lane
    /// bound.
    pub fn new(
        lane_count: NonZeroU32,
        mut lanes: Vec<LaneConfig>,
    ) -> Result<Self, LaneCatalogError> {
        if lanes.is_empty() {
            return Err(LaneCatalogError::EmptyCatalog);
        }
        if lanes.len() > MAX_ACTIVE_EXECUTION_LANES {
            return Err(LaneCatalogError::ActiveLaneBoundExceeded {
                actual: lanes.len(),
                maximum: MAX_ACTIVE_EXECUTION_LANES,
            });
        }
        let mut seen_ids = BTreeSet::new();
        let mut seen_aliases = BTreeSet::new();

        for lane in &lanes {
            if lane.alias.trim().is_empty() {
                return Err(LaneCatalogError::EmptyAlias(lane.id));
            }
            if lane.id.as_u32() >= lane_count.get() {
                return Err(LaneCatalogError::LaneOutOfBounds {
                    lane: lane.id,
                    lane_count: lane_count.get(),
                });
            }
            if !seen_ids.insert(lane.id) {
                return Err(LaneCatalogError::DuplicateLaneId(lane.id));
            }
            if !seen_aliases.insert(lane.alias.clone()) {
                return Err(LaneCatalogError::DuplicateLaneAlias(lane.alias.clone()));
            }
        }

        // Catalog iteration feeds derived storage geometry, snapshot encoding,
        // and lifecycle commitments. Canonicalize it here so semantically
        // identical configuration files cannot disagree about the primary
        // lane or any other order-sensitive derived artifact.
        lanes.sort_unstable_by_key(|lane| lane.id);

        Ok(Self { lane_count, lanes })
    }

    /// Exclusive lane-id bound for the current catalog namespace.
    ///
    /// This can exceed [`Self::lanes`]'s length when the catalog is sparse.
    #[must_use]
    pub const fn lane_count(&self) -> NonZeroU32 {
        self.lane_count
    }

    /// Metadata for all registered lanes.
    #[must_use]
    pub fn lanes(&self) -> &[LaneConfig] {
        &self.lanes
    }

    /// Find a lane by alias.
    #[must_use]
    pub fn by_alias(&self, alias: &str) -> Option<&LaneConfig> {
        self.lanes.iter().find(|lane| lane.alias == alias)
    }

    /// Apply a lifecycle plan, producing a new catalog with the requested additions and retirements.
    ///
    /// # Errors
    /// Returns a [`LaneCatalogError`] when additions or retirements are duplicated, retirements
    /// reference unknown lanes, or the resulting catalog is invalid (empty, duplicate
    /// identifiers/aliases, out-of-bounds ids, or too many active entries).
    pub fn apply_lifecycle(&self, plan: &LaneLifecyclePlan) -> Result<Self, LaneCatalogError> {
        let mut retire_set = BTreeSet::new();
        for retire_id in &plan.retire {
            if !retire_set.insert(*retire_id) {
                return Err(LaneCatalogError::DuplicateRetireLane(*retire_id));
            }
        }
        for retire_id in &retire_set {
            let present = self.lanes.iter().any(|lane| lane.id == *retire_id);
            if !present {
                return Err(LaneCatalogError::MissingLane(*retire_id));
            }
        }

        let mut addition_ids = BTreeSet::new();
        let mut addition_aliases = BTreeSet::new();
        for addition in &plan.additions {
            if addition.alias.trim().is_empty() {
                return Err(LaneCatalogError::EmptyAlias(addition.id));
            }
            if !addition_ids.insert(addition.id) {
                return Err(LaneCatalogError::DuplicateLaneId(addition.id));
            }
            if !addition_aliases.insert(addition.alias.as_str()) {
                return Err(LaneCatalogError::DuplicateLaneAlias(addition.alias.clone()));
            }
        }

        let mut merged: Vec<LaneConfig> = self
            .lanes
            .iter()
            .filter(|lane| !retire_set.contains(&lane.id))
            .cloned()
            .collect();
        merged.extend(plan.additions.iter().cloned());

        let Some(max_lane_id) = merged.iter().map(|lane| lane.id.as_u32()).max() else {
            return Err(LaneCatalogError::EmptyCatalog);
        };
        let lane_count = NonZeroU32::new(max_lane_id.saturating_add(1))
            .expect("lane ids are u32 so +1 always fits NonZeroU32");
        LaneCatalog::new(lane_count, merged)
    }
}

impl Default for LaneCatalog {
    fn default() -> Self {
        Self {
            lane_count: NonZeroU32::new(1).expect("nonzero lane count"),
            lanes: vec![LaneConfig::default()],
        }
    }
}

/// Errors returned when constructing a [`LaneCatalog`].
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum LaneCatalogError {
    /// Duplicate lane identifier detected.
    #[error("duplicate lane id {0}")]
    DuplicateLaneId(LaneId),
    /// Duplicate alias detected.
    #[error("duplicate lane alias {0}")]
    DuplicateLaneAlias(String),
    /// Retire plan referenced a lane that does not exist.
    #[error("cannot retire unknown lane {0}")]
    MissingLane(LaneId),
    /// Retire plan referenced the same lane more than once.
    #[error("duplicate retire lane {0}")]
    DuplicateRetireLane(LaneId),
    /// Alias was left blank.
    #[error("lane {0} has an empty alias")]
    EmptyAlias(LaneId),
    /// Lifecycle plan would leave the catalog empty.
    #[error("lane catalog cannot be empty")]
    EmptyCatalog,
    /// Catalog contains more simultaneously active lanes than consensus can represent.
    #[error("lane catalog has {actual} active entries, exceeding the consensus maximum {maximum}")]
    ActiveLaneBoundExceeded {
        /// Number of active entries supplied.
        actual: usize,
        /// Consensus-wide active-entry maximum.
        maximum: usize,
    },
    /// Lane identifier outside the configured lane count.
    #[error("lane {lane} exceeds configured lane count {lane_count}")]
    LaneOutOfBounds {
        /// Identifier that exceeded the configured lane count.
        lane: LaneId,
        /// Total number of configured lanes.
        lane_count: u32,
    },
}

/// Metadata describing a configured data space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSpaceMetadata {
    /// Identifier assigned to the data space.
    pub id: DataSpaceId,
    /// Human-friendly alias.
    pub alias: String,
    /// Optional description for dashboards and docs.
    pub description: Option<String>,
    /// Fault tolerance value (f) used to size lane-local consensus and relay committees (3f + 1).
    pub fault_tolerance: u32,
}

impl Default for DataSpaceMetadata {
    fn default() -> Self {
        Self {
            id: DataSpaceId::UNIVERSAL,
            alias: "universal".to_string(),
            description: None,
            fault_tolerance: 1,
        }
    }
}

/// Validated catalog describing configured data spaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSpaceCatalog {
    entries: Vec<DataSpaceMetadata>,
}

impl DataSpaceCatalog {
    /// Build a catalog ensuring identifiers and aliases remain unique.
    ///
    /// # Errors
    /// Returns a [`DataSpaceCatalogError`] when metadata reuses an identifier or alias, when
    /// an alias is left blank, or when fault tolerance is below 1.
    pub fn new(entries: Vec<DataSpaceMetadata>) -> Result<Self, DataSpaceCatalogError> {
        let mut seen_ids = BTreeSet::new();
        let mut seen_aliases = BTreeSet::new();

        for entry in &entries {
            if entry.alias.trim().is_empty() {
                return Err(DataSpaceCatalogError::EmptyAlias(entry.id));
            }
            if entry.fault_tolerance == 0 {
                return Err(DataSpaceCatalogError::InvalidFaultTolerance {
                    id: entry.id,
                    fault_tolerance: entry.fault_tolerance,
                });
            }
            if !seen_ids.insert(entry.id) {
                return Err(DataSpaceCatalogError::DuplicateId(entry.id));
            }
            if !seen_aliases.insert(entry.alias.clone()) {
                return Err(DataSpaceCatalogError::DuplicateAlias(entry.alias.clone()));
            }
        }

        Ok(Self { entries })
    }

    /// Access the catalog entries.
    #[must_use]
    pub fn entries(&self) -> &[DataSpaceMetadata] {
        &self.entries
    }

    /// Find an entry by alias.
    #[must_use]
    pub fn by_alias(&self, alias: &str) -> Option<&DataSpaceMetadata> {
        self.entries.iter().find(|entry| entry.alias == alias)
    }

    /// Find an entry by identifier.
    #[must_use]
    pub fn by_id(&self, id: DataSpaceId) -> Option<&DataSpaceMetadata> {
        self.entries.iter().find(|entry| entry.id == id)
    }
}

impl Default for DataSpaceCatalog {
    fn default() -> Self {
        Self {
            entries: vec![DataSpaceMetadata::default()],
        }
    }
}

/// Errors returned when constructing a [`DataSpaceCatalog`].
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum DataSpaceCatalogError {
    /// Duplicate identifier encountered.
    #[error("duplicate dataspace id {0}")]
    DuplicateId(DataSpaceId),
    /// Duplicate alias encountered.
    #[error("duplicate dataspace alias {0}")]
    DuplicateAlias(String),
    /// Alias field left blank.
    #[error("dataspace {0} has an empty alias")]
    EmptyAlias(DataSpaceId),
    /// Fault tolerance must be at least 1.
    #[error("dataspace {id} has invalid fault_tolerance {fault_tolerance}; must be >= 1")]
    InvalidFaultTolerance {
        /// Dataspace identifier with an invalid fault tolerance value.
        id: DataSpaceId,
        /// Fault tolerance value that failed validation.
        fault_tolerance: u32,
    },
}

#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for DataSpaceId {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.0, out);
    }
}

#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for DataSpaceId {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_u64()?;
        Ok(Self(value))
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use norito::codec::{DecodeAll, Encode};

    use super::*;

    fn incarnation_map(catalog: &LaneCatalog) -> BTreeMap<LaneId, Hash> {
        catalog
            .lanes()
            .iter()
            .map(|lane| {
                (
                    lane.id,
                    Hash::new(format!("test-lane-incarnation-{}", lane.id.as_u32())),
                )
            })
            .collect()
    }

    fn lifecycle_status(catalog: &LaneCatalog) -> LaneLifecycleStatusV1 {
        LaneLifecycleStatusV1::new(true, catalog, &incarnation_map(catalog))
            .expect("valid lifecycle status")
    }

    #[test]
    fn lane_id_roundtrip() {
        let original = LaneId::new(42);
        let bytes = Encode::encode(&original);
        let mut slice: &[u8] = &bytes;
        let decoded = LaneId::decode_all(&mut slice).expect("decode LaneId");
        assert_eq!(decoded, original);
        assert_eq!(LaneId::SINGLE.as_u32(), 0);
    }

    #[test]
    fn shard_id_roundtrip() {
        let original = ShardId::new(24);
        let bytes = Encode::encode(&original);
        let mut slice: &[u8] = &bytes;
        let decoded = ShardId::decode_all(&mut slice).expect("decode ShardId");
        assert_eq!(decoded, original);
        assert_eq!(ShardId::new(0).as_u32(), 0);
    }

    #[test]
    fn lane_id_from_lane_index_enforces_bounds() {
        let lane_count = NonZeroU32::new(2).expect("nonzero");
        let lane = LaneId::from_lane_index(1, lane_count).expect("valid lane");
        assert_eq!(lane.as_u32(), 1);
        let err = LaneId::from_lane_index(2, lane_count).expect_err("should be out of bounds");
        assert_eq!(
            err,
            LaneIdError::OutOfBounds {
                index: 2,
                lane_count: 2
            }
        );
    }

    #[test]
    fn dataspace_id_roundtrip() {
        let original = DataSpaceId::new(7);
        let bytes = Encode::encode(&original);
        let mut slice: &[u8] = &bytes;
        let decoded = DataSpaceId::decode_all(&mut slice).expect("decode DataSpaceId");
        assert_eq!(decoded, original);
        assert_eq!(DataSpaceId::UNIVERSAL.as_u64(), 0);
        assert_eq!(
            "7".parse::<DataSpaceId>().expect("parse DataSpaceId"),
            original
        );
        assert!("-1".parse::<DataSpaceId>().is_err());
    }

    #[test]
    fn dataspace_id_parses_decimal_cli_form() {
        assert_eq!("0".parse(), Ok(DataSpaceId::UNIVERSAL));
        assert_eq!(u64::MAX.to_string().parse(), Ok(DataSpaceId::new(u64::MAX)));
        assert!("-1".parse::<DataSpaceId>().is_err());
        assert!("not-a-dataspace".parse::<DataSpaceId>().is_err());
    }

    #[test]
    fn dataspace_id_from_hash_uses_low_bytes() {
        let mut hash = [0u8; 32];
        hash[0..8].copy_from_slice(&[0xAB, 0xCD, 0xEF, 0x01, 0x02, 0x03, 0x04, 0x05]);
        let expected = u64::from_le_bytes(hash[..8].try_into().expect("slice length"));
        let id = DataSpaceId::from_hash(&hash);
        assert_eq!(id.as_u64(), expected);
    }

    #[test]
    fn lane_catalog_validates_alias_and_range() {
        let lane_count = NonZeroU32::new(2).expect("nonzero");
        let catalog = LaneCatalog::new(
            lane_count,
            vec![LaneConfig {
                id: LaneId::new(0),
                alias: "alpha".into(),
                description: None,
                ..LaneConfig::default()
            }],
        )
        .expect("valid catalog");
        assert_eq!(catalog.lane_count(), lane_count);
        assert!(catalog.by_alias("alpha").is_some());

        let dup = LaneCatalog::new(
            lane_count,
            vec![
                LaneConfig {
                    id: LaneId::new(0),
                    alias: "dup".into(),
                    description: None,
                    ..LaneConfig::default()
                },
                LaneConfig {
                    id: LaneId::new(0),
                    alias: "dup".into(),
                    description: None,
                    ..LaneConfig::default()
                },
            ],
        )
        .expect_err("duplicate lanes");
        assert!(matches!(dup, LaneCatalogError::DuplicateLaneId(_)));

        let out_of_range = LaneCatalog::new(
            lane_count,
            vec![LaneConfig {
                id: LaneId::new(5),
                alias: "gamma".into(),
                description: None,
                ..LaneConfig::default()
            }],
        )
        .expect_err("out of range lane");
        assert!(matches!(
            out_of_range,
            LaneCatalogError::LaneOutOfBounds { lane, lane_count: 2 }
                if lane.as_u32() == 5
        ));
    }

    #[test]
    fn lane_catalog_rejects_active_entries_above_consensus_bound() {
        let lanes = (0..MAX_ACTIVE_EXECUTION_LANES)
            .map(|index| LaneConfig {
                id: LaneId::new(u32::try_from(index).expect("lane index fits u32")),
                alias: format!("lane-{index}"),
                ..LaneConfig::default()
            })
            .collect::<Vec<_>>();
        let boundary_count =
            NonZeroU32::new(u32::try_from(MAX_ACTIVE_EXECUTION_LANES).expect("bound fits u32"))
                .expect("active-lane bound is non-zero");
        let catalog = LaneCatalog::new(boundary_count, lanes.clone())
            .expect("the exact active-lane protocol bound is admissible");

        let overflow_id =
            LaneId::new(u32::try_from(MAX_ACTIVE_EXECUTION_LANES).expect("bound fits u32"));
        let overflow = LaneConfig {
            id: overflow_id,
            alias: "overflow".to_owned(),
            ..LaneConfig::default()
        };
        let overflow_count = NonZeroU32::new(
            u32::try_from(MAX_ACTIVE_EXECUTION_LANES + 1).expect("bound plus one fits u32"),
        )
        .expect("bound plus one is non-zero");
        let mut oversized = lanes;
        oversized.push(overflow.clone());
        assert_eq!(
            LaneCatalog::new(overflow_count, oversized),
            Err(LaneCatalogError::ActiveLaneBoundExceeded {
                actual: MAX_ACTIVE_EXECUTION_LANES + 1,
                maximum: MAX_ACTIVE_EXECUTION_LANES,
            })
        );

        let lifecycle_error = catalog
            .apply_lifecycle(&LaneLifecyclePlan {
                additions: vec![overflow],
                retire: Vec::new(),
            })
            .expect_err("lifecycle admission must reject an unrepresentable active catalog");
        assert_eq!(
            lifecycle_error,
            LaneCatalogError::ActiveLaneBoundExceeded {
                actual: MAX_ACTIVE_EXECUTION_LANES + 1,
                maximum: MAX_ACTIVE_EXECUTION_LANES,
            }
        );
    }

    #[test]
    fn lane_catalog_canonicalizes_entry_order_and_lifecycle_commitment() {
        let lane_count = NonZeroU32::new(2).expect("nonzero lane count");
        let secondary = LaneConfig {
            id: LaneId::new(1),
            alias: "secondary".to_owned(),
            ..LaneConfig::default()
        };
        let canonical =
            LaneCatalog::new(lane_count, vec![LaneConfig::default(), secondary.clone()])
                .expect("canonical catalog");
        let permuted = LaneCatalog::new(lane_count, vec![secondary, LaneConfig::default()])
            .expect("permuted catalog");

        assert_eq!(permuted, canonical);
        assert_eq!(
            permuted
                .lanes()
                .iter()
                .map(|lane| lane.id)
                .collect::<Vec<_>>(),
            vec![LaneId::SINGLE, LaneId::new(1)]
        );
        assert_eq!(
            LaneLifecycleParameterV1::catalog_hash(&permuted),
            LaneLifecycleParameterV1::catalog_hash(&canonical),
            "semantic catalog permutations must share one optimistic-concurrency commitment"
        );
    }

    #[test]
    fn lane_config_roundtrip_encodes_storage_profile() {
        let mut metadata = BTreeMap::new();
        metadata.insert("scheduler.teu_capacity".to_string(), "1024".to_string());
        let config = LaneConfig {
            id: LaneId::new(1),
            dataspace_id: DataSpaceId::new(5),
            alias: "governance".to_string(),
            description: Some("Governance lane".to_string()),
            visibility: LaneVisibility::Restricted,
            lane_type: Some("governance".to_string()),
            governance: Some("parliament".to_string()),
            settlement: Some("xor_lane".to_string()),
            storage: LaneStorageProfile::CommitmentOnly,
            proof_scheme: DaProofScheme::default(),
            metadata,
        };
        let bytes = Encode::encode(&config);
        let mut slice: &[u8] = &bytes;
        let decoded = LaneConfig::decode_all(&mut slice).expect("decode LaneConfig");
        assert_eq!(decoded, config);
    }

    #[test]
    fn dataspace_catalog_validates_entries() {
        let catalog = DataSpaceCatalog::new(vec![DataSpaceMetadata {
            id: DataSpaceId::new(1),
            alias: "telemetry".into(),
            description: None,
            fault_tolerance: 1,
        }])
        .expect("valid dataspace");
        assert!(catalog.by_alias("telemetry").is_some());

        let invalid_fault_tolerance = DataSpaceCatalog::new(vec![DataSpaceMetadata {
            id: DataSpaceId::new(9),
            alias: "invalid".into(),
            description: None,
            fault_tolerance: 0,
        }])
        .expect_err("fault tolerance below 1 should fail");
        assert!(matches!(
            invalid_fault_tolerance,
            DataSpaceCatalogError::InvalidFaultTolerance { .. }
        ));

        let dup = DataSpaceCatalog::new(vec![
            DataSpaceMetadata {
                id: DataSpaceId::new(2),
                alias: "ops".into(),
                description: None,
                fault_tolerance: 1,
            },
            DataSpaceMetadata {
                id: DataSpaceId::new(2),
                alias: "ops".into(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect_err("duplicate dataspace");
        assert!(matches!(dup, DataSpaceCatalogError::DuplicateId(_)));

        let empty_alias = DataSpaceCatalog::new(vec![DataSpaceMetadata {
            id: DataSpaceId::new(3),
            alias: "   ".into(),
            description: None,
            fault_tolerance: 1,
        }])
        .expect_err("blank alias");
        assert!(matches!(empty_alias, DataSpaceCatalogError::EmptyAlias(_)));
    }

    #[test]
    fn dataspace_default_fault_tolerance_is_nonzero() {
        let entry = DataSpaceMetadata::default();
        assert_eq!(entry.fault_tolerance, 1);
        assert_eq!(entry.alias, "universal");
    }

    #[test]
    fn lane_config_identifies_only_valid_autoscale_managed_elastic_lanes() {
        let mut lane = LaneConfig {
            id: LaneId::new(3),
            alias: "elastic-lane-3".into(),
            ..LaneConfig::default()
        };
        assert!(!lane.claims_autoscale_managed());
        assert!(!lane.is_autoscale_managed_elastic());

        lane.metadata
            .insert(AUTOSCALE_META_MANAGED.into(), "true".into());
        lane.metadata
            .insert(AUTOSCALE_META_CREATED_HEIGHT.into(), "42".into());
        assert!(lane.claims_autoscale_managed());
        assert_eq!(lane.autoscale_created_height(), Some(42));
        assert!(lane.is_autoscale_managed_elastic());
        assert!(!lane.has_autoscale_drain_state());
        lane.metadata.insert(
            AUTOSCALE_META_DRAIN_STATE.into(),
            "canonical-drain-state".into(),
        );
        assert!(lane.has_autoscale_drain_state());
        assert!(!lane.has_autoscale_committee());
        lane.metadata.insert(
            AUTOSCALE_META_COMMITTEE.into(),
            "canonical-incarnation-committee".into(),
        );
        assert!(lane.has_autoscale_committee());
        assert!(lane.is_autoscale_managed_elastic());

        let mut spoofed_value = lane.clone();
        spoofed_value
            .metadata
            .insert(AUTOSCALE_META_MANAGED.into(), "TRUE".into());
        assert!(spoofed_value.claims_autoscale_managed());
        assert!(!spoofed_value.is_autoscale_managed_elastic());

        let mut spoofed_alias = lane.clone();
        spoofed_alias.alias = "renamed-elastic".into();
        assert!(!spoofed_alias.is_autoscale_managed_elastic());

        let mut zero_height = lane.clone();
        zero_height
            .metadata
            .insert(AUTOSCALE_META_CREATED_HEIGHT.into(), "0".into());
        assert_eq!(zero_height.autoscale_created_height(), None);
        assert!(!zero_height.is_autoscale_managed_elastic());

        let mut restricted = lane;
        restricted.visibility = LaneVisibility::Restricted;
        assert!(!restricted.is_autoscale_managed_elastic());
    }

    #[test]
    fn autoscale_profile_inheritance_ignores_identity_and_reserved_metadata_only() {
        let mut base = LaneConfig {
            id: LaneId::new(2),
            dataspace_id: DataSpaceId::new(7),
            alias: "settlement-base".into(),
            description: Some("operator-facing base lane".into()),
            visibility: LaneVisibility::Public,
            lane_type: Some("regulated-public".into()),
            governance: Some("governance-v2".into()),
            settlement: Some("settlement-v3".into()),
            storage: LaneStorageProfile::SplitReplica,
            proof_scheme: DaProofScheme::KzgBls12_381,
            metadata: BTreeMap::new(),
        };
        base.metadata
            .insert("security.profile".into(), "strict".into());
        base.metadata
            .insert("scheduler.teu_capacity".into(), "2400".into());

        let mut elastic = base.clone();
        elastic.id = LaneId::new(3);
        elastic.alias = "elastic-lane-3".into();
        elastic.description = Some("Consensus-managed elastic lane".into());
        elastic
            .metadata
            .insert(AUTOSCALE_META_MANAGED.into(), "true".into());
        elastic
            .metadata
            .insert(AUTOSCALE_META_CREATED_HEIGHT.into(), "42".into());
        assert!(elastic.inherits_autoscale_profile_from(&base));
        elastic.metadata.insert(
            AUTOSCALE_META_DRAIN_STATE.into(),
            "canonical-drain-state".into(),
        );
        assert!(elastic.inherits_autoscale_profile_from(&base));
        elastic.metadata.insert(
            AUTOSCALE_META_COMMITTEE.into(),
            "canonical-incarnation-committee".into(),
        );
        assert!(elastic.inherits_autoscale_profile_from(&base));

        let profile_drifts = [
            ("dataspace", {
                let mut drift = elastic.clone();
                drift.dataspace_id = DataSpaceId::new(8);
                drift
            }),
            ("visibility", {
                let mut drift = elastic.clone();
                drift.visibility = LaneVisibility::Restricted;
                drift
            }),
            ("lane type", {
                let mut drift = elastic.clone();
                drift.lane_type = Some("unregulated".into());
                drift
            }),
            ("governance", {
                let mut drift = elastic.clone();
                drift.governance = Some("governance-v1".into());
                drift
            }),
            ("settlement", {
                let mut drift = elastic.clone();
                drift.settlement = Some("settlement-v1".into());
                drift
            }),
            ("storage", {
                let mut drift = elastic.clone();
                drift.storage = LaneStorageProfile::CommitmentOnly;
                drift
            }),
            ("proof scheme", {
                let mut drift = elastic.clone();
                drift.proof_scheme = DaProofScheme::MerkleSha256;
                drift
            }),
            ("metadata value", {
                let mut drift = elastic.clone();
                drift
                    .metadata
                    .insert("security.profile".into(), "permissive".into());
                drift
            }),
            ("missing metadata", {
                let mut drift = elastic.clone();
                drift.metadata.remove("scheduler.teu_capacity");
                drift
            }),
            ("extra metadata", {
                let mut drift = elastic;
                drift.metadata.insert("unexpected".into(), "value".into());
                drift
            }),
        ];

        for (field, drift) in profile_drifts {
            assert!(
                !drift.inherits_autoscale_profile_from(&base),
                "autoscale profile comparison accepted {field} drift"
            );
        }
    }

    #[test]
    fn lane_lifecycle_plan_adds_and_retires() {
        let lane_count = NonZeroU32::new(2).expect("nonzero");
        let base = LaneCatalog::new(
            lane_count,
            vec![LaneConfig {
                id: LaneId::new(0),
                alias: "alpha".into(),
                ..LaneConfig::default()
            }],
        )
        .expect("base catalog");

        let plan = LaneLifecyclePlan {
            additions: vec![LaneConfig {
                id: LaneId::new(1),
                alias: "beta".into(),
                ..LaneConfig::default()
            }],
            retire: Vec::new(),
        };

        let expanded = base.apply_lifecycle(&plan).expect("apply lifecycle");
        assert_eq!(expanded.lane_count().get(), 2);
        assert!(expanded.by_alias("beta").is_some());

        let retire_plan = LaneLifecyclePlan {
            additions: Vec::new(),
            retire: vec![LaneId::new(1)],
        };
        let trimmed = expanded
            .apply_lifecycle(&retire_plan)
            .expect("retire lifecycle");
        assert_eq!(trimmed.lanes().len(), 1);
        assert!(trimmed.by_alias("beta").is_none());
    }

    #[test]
    fn lane_lifecycle_parameter_roundtrips_and_binds_exact_catalog() {
        let catalog = LaneCatalog::default();
        let incarnation_entries =
            LaneLifecycleParameterV1::canonical_incarnations(&catalog, &incarnation_map(&catalog))
                .expect("canonical incarnations");
        let parameter = LaneLifecycleParameterV1::new(
            &catalog,
            &incarnation_entries,
            LaneLifecyclePlan {
                additions: vec![LaneConfig {
                    id: LaneId::new(1),
                    alias: "manual-lane".to_owned(),
                    ..LaneConfig::default()
                }],
                retire: Vec::new(),
            },
        )
        .expect("valid lifecycle parameter");
        let custom = parameter.clone().into_custom_parameter();
        assert_eq!(
            LaneLifecycleParameterV1::from_custom_parameter(&custom)
                .expect("decode lifecycle custom parameter"),
            Some(parameter.clone())
        );

        let changed = LaneCatalog::new(
            NonZeroU32::new(2).expect("nonzero lane count"),
            vec![
                LaneConfig::default(),
                LaneConfig {
                    id: LaneId::new(1),
                    alias: "other".to_owned(),
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("changed catalog");
        assert_ne!(
            parameter.expected_catalog_hash,
            LaneLifecycleParameterV1::catalog_hash(&changed),
            "catalog commitment must change with topology metadata"
        );
    }

    #[test]
    fn lane_lifecycle_parameter_rejects_unknown_version_and_fields() {
        let catalog = LaneCatalog::default();
        let entries = lifecycle_status(&catalog).incarnations;
        let mut unsupported =
            LaneLifecycleParameterV1::new(&catalog, &entries, LaneLifecyclePlan::default())
                .expect("valid lifecycle parameter");
        unsupported.version = LaneLifecycleParameterV1::VERSION.saturating_add(1);
        let err =
            LaneLifecycleParameterV1::from_custom_parameter(&unsupported.into_custom_parameter())
                .expect_err("unsupported lifecycle payload version must fail closed");
        assert!(err.to_string().contains("unsupported"));

        let valid = LaneLifecycleParameterV1::new(&catalog, &entries, LaneLifecyclePlan::default())
            .expect("valid lifecycle parameter");
        let mut encoded = norito::json::to_string(&valid).expect("serialize lifecycle payload");
        assert_eq!(encoded.pop(), Some('}'));
        encoded.push_str(",\"unexpected\":true}");
        let custom = CustomParameter::new(
            LaneLifecycleParameterV1::parameter_id(),
            encoded
                .parse::<iroha_primitives::json::Json>()
                .expect("adversarial payload is syntactically valid JSON"),
        );
        let err = LaneLifecycleParameterV1::from_custom_parameter(&custom)
            .expect_err("unknown lifecycle payload field must fail closed");
        assert!(err.to_string().contains("unexpected"));
    }

    #[test]
    fn lane_lifecycle_status_roundtrips_json_and_norito() {
        let catalog = LaneCatalog::new(
            NonZeroU32::new(2).expect("nonzero lane count"),
            vec![
                LaneConfig::default(),
                LaneConfig {
                    id: LaneId::new(1),
                    alias: "secondary".to_owned(),
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("valid lifecycle status catalog");
        let status = lifecycle_status(&catalog);
        assert_eq!(status.validate().expect("validate status"), catalog);

        let json = norito::json::to_string(&status).expect("serialize lifecycle status JSON");
        let from_json = norito::json::from_str::<LaneLifecycleStatusV1>(&json)
            .expect("decode lifecycle status JSON");
        assert_eq!(from_json, status);

        let bytes = norito::to_bytes(&status).expect("encode lifecycle status Norito");
        let from_norito = norito::decode_from_bytes::<LaneLifecycleStatusV1>(&bytes)
            .expect("decode lifecycle status Norito");
        assert_eq!(from_norito, status);
    }

    #[test]
    fn lane_lifecycle_status_rejects_forged_hash_version_and_order() {
        let mut status = lifecycle_status(&LaneCatalog::default());
        status.catalog_hash = Hash::prehashed([0xA5; Hash::LENGTH]);
        assert!(matches!(
            status.validate(),
            Err(LaneLifecycleStatusError::CatalogHashMismatch { .. })
        ));

        let mut status = lifecycle_status(&LaneCatalog::default());
        status.version = LaneLifecycleStatusV1::VERSION.saturating_add(1);
        assert!(matches!(
            status.validate(),
            Err(LaneLifecycleStatusError::UnsupportedVersion { .. })
        ));

        let canonical = LaneCatalog::new(
            NonZeroU32::new(2).expect("nonzero lane count"),
            vec![
                LaneConfig::default(),
                LaneConfig {
                    id: LaneId::new(1),
                    alias: "secondary".to_owned(),
                    ..LaneConfig::default()
                },
            ],
        )
        .expect("valid catalog");
        let mut status = lifecycle_status(&canonical);
        status.lanes.reverse();
        assert_eq!(
            status.validate(),
            Err(LaneLifecycleStatusError::NonCanonicalLaneOrder)
        );

        let mut status = lifecycle_status(&canonical);
        status.incarnations.reverse();
        assert_eq!(
            status.validate(),
            Err(LaneLifecycleStatusError::NonCanonicalIncarnationOrder)
        );

        let mut status = lifecycle_status(&canonical);
        status.incarnations[1].incarnation = status.incarnations[0].incarnation;
        assert!(matches!(
            status.validate(),
            Err(LaneLifecycleStatusError::DuplicateIncarnation { .. })
        ));

        let mut status = lifecycle_status(&canonical);
        status.incarnation_root = Hash::new(b"forged-incarnation-root");
        assert!(matches!(
            status.validate(),
            Err(LaneLifecycleStatusError::IncarnationRootMismatch { .. })
        ));
    }

    #[test]
    fn lane_lifecycle_incarnation_root_changes_for_identical_catalog_replacement() {
        let catalog = LaneCatalog::default();
        let first = BTreeMap::from([(
            LaneId::SINGLE,
            Hash::new(b"lane-incarnation-before-replacement"),
        )]);
        let replacement = BTreeMap::from([(
            LaneId::SINGLE,
            Hash::new(b"lane-incarnation-after-replacement"),
        )]);
        let first_status =
            LaneLifecycleStatusV1::new(true, &catalog, &first).expect("first lifecycle status");
        let replacement_status = LaneLifecycleStatusV1::new(true, &catalog, &replacement)
            .expect("replacement lifecycle status");
        assert_eq!(first_status.catalog_hash, replacement_status.catalog_hash);
        assert_ne!(
            first_status.incarnation_root, replacement_status.incarnation_root,
            "same metadata must not make a prior-incarnation request replayable"
        );
    }

    #[test]
    fn lane_lifecycle_status_json_rejects_duplicate_and_unknown_fields() {
        let status = lifecycle_status(&LaneCatalog::default());
        let mut encoded = norito::json::to_string(&status).expect("serialize lifecycle status");
        assert_eq!(encoded.pop(), Some('}'));
        encoded.push_str(",\"version\":1}");
        let err = norito::json::from_str::<LaneLifecycleStatusV1>(&encoded)
            .expect_err("duplicate status fields must fail closed");
        assert!(err.to_string().contains("duplicate field `version`"));

        let mut encoded = norito::json::to_string(&status).expect("serialize lifecycle status");
        assert_eq!(encoded.pop(), Some('}'));
        encoded.push_str(",\"unexpected\":true}");
        let err = norito::json::from_str::<LaneLifecycleStatusV1>(&encoded)
            .expect_err("unknown status fields must fail closed");
        assert!(err.to_string().contains("unexpected"));
    }

    #[test]
    fn lane_lifecycle_plan_json_rejects_duplicate_fields() {
        let duplicate = r#"{"additions":[],"retire":[],"retire":[]}"#;
        let err = norito::json::from_str::<LaneLifecyclePlan>(duplicate)
            .expect_err("duplicate lifecycle plan field must fail closed");
        assert!(err.to_string().contains("duplicate field `retire`"));
    }

    #[test]
    fn lane_lifecycle_rejects_unknown_retire_or_empty() {
        let base = LaneCatalog::default();

        let missing = LaneLifecyclePlan {
            additions: Vec::new(),
            retire: vec![LaneId::new(9)],
        };
        let err = base
            .apply_lifecycle(&missing)
            .expect_err("unknown retire must fail");
        assert!(matches!(err, LaneCatalogError::MissingLane(lane) if lane.as_u32() == 9));

        let duplicate_retire = LaneLifecyclePlan {
            additions: Vec::new(),
            retire: vec![LaneId::SINGLE, LaneId::SINGLE],
        };
        let err = base
            .apply_lifecycle(&duplicate_retire)
            .expect_err("duplicate retire must fail");
        assert!(matches!(
            err,
            LaneCatalogError::DuplicateRetireLane(lane) if lane == LaneId::SINGLE
        ));

        let forged_present = LaneLifecyclePlan {
            additions: vec![LaneConfig {
                id: LaneId::new(9),
                alias: "forged-present".into(),
                ..LaneConfig::default()
            }],
            retire: vec![LaneId::new(9)],
        };
        let err = base
            .apply_lifecycle(&forged_present)
            .expect_err("addition must not satisfy retire precondition");
        assert!(matches!(err, LaneCatalogError::MissingLane(lane) if lane.as_u32() == 9));

        let empty_plan = LaneLifecyclePlan {
            additions: Vec::new(),
            retire: vec![LaneId::SINGLE],
        };
        let err = base
            .apply_lifecycle(&empty_plan)
            .expect_err("empty catalog must be rejected");
        assert!(matches!(err, LaneCatalogError::EmptyCatalog));
    }

    #[test]
    fn lane_lifecycle_rejects_duplicate_additions_before_merge() {
        let base = LaneCatalog::default();

        let duplicate_id = LaneLifecyclePlan {
            additions: vec![
                LaneConfig {
                    id: LaneId::new(1),
                    alias: "beta".into(),
                    ..LaneConfig::default()
                },
                LaneConfig {
                    id: LaneId::new(1),
                    alias: "gamma".into(),
                    ..LaneConfig::default()
                },
            ],
            retire: Vec::new(),
        };
        let err = base
            .apply_lifecycle(&duplicate_id)
            .expect_err("duplicate additions must fail before merge");
        assert!(matches!(
            err,
            LaneCatalogError::DuplicateLaneId(lane) if lane == LaneId::new(1)
        ));

        let duplicate_alias = LaneLifecyclePlan {
            additions: vec![
                LaneConfig {
                    id: LaneId::new(1),
                    alias: "beta".into(),
                    ..LaneConfig::default()
                },
                LaneConfig {
                    id: LaneId::new(2),
                    alias: "beta".into(),
                    ..LaneConfig::default()
                },
            ],
            retire: Vec::new(),
        };
        let err = base
            .apply_lifecycle(&duplicate_alias)
            .expect_err("duplicate addition aliases must fail before merge");
        assert!(matches!(
            err,
            LaneCatalogError::DuplicateLaneAlias(alias) if alias == "beta"
        ));
    }

    #[test]
    fn lane_catalog_constructor_rejects_empty_catalog() {
        let error = LaneCatalog::new(NonZeroU32::new(1).expect("non-zero bound"), Vec::new())
            .expect_err("validated catalogs must never be empty");
        assert_eq!(error, LaneCatalogError::EmptyCatalog);
    }
}

/// Prelude re-export for the Nexus module.
pub mod prelude {
    pub use super::{
        DataSpaceCatalog, DataSpaceCatalogError, DataSpaceId, DataSpaceMetadata, LaneCatalog,
        LaneCatalogError, LaneConfig, LaneId, LaneIdError, LaneLifecycleIncarnationEntry,
        LaneLifecycleParameterV1, LaneLifecyclePlan, LaneLifecycleStatusError,
        LaneLifecycleStatusV1, LaneRelayEmergencyValidatorSet, LaneStorageProfile,
        LaneStorageProfileParseError, LaneVisibility, LaneVisibilityParseError, ShardId,
    };
}
