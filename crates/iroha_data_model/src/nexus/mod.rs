//! Nexus-lane scaffolding types.
//!
//! These identifiers provide a forward-compatible shell for the
//! multi-lane/data-space routing surface described in `nexus.md` and the
//! `nexus_transition_notes`. They currently default to single-lane
//! placeholders but are Norito-compatible so downstream crates can start
//! threading them through APIs.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    num::NonZeroU32,
    str::FromStr,
};

use derive_more::Display;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::{da::commitment::DaProofScheme, id::IdBox};

mod axt;
mod compliance;
mod endorsement;
mod manifest;
mod privacy;
mod relay;

pub use axt::*;
pub use compliance::*;
pub use endorsement::*;
pub use manifest::*;
pub use privacy::*;
pub mod portfolio;
pub use portfolio::*;
pub mod staking;
pub use relay::*;
pub use staking::*;

/// Declarative lane lifecycle changes (additions and retirements).
#[derive(Debug, Clone, Default, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct LaneLifecyclePlan {
    /// Lane metadata to add or replace.
    pub additions: Vec<LaneConfig>,
    /// Lane identifiers to retire.
    pub retire: Vec<LaneId>,
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
    /// Placeholder identifier used while the system operates with a single lane.
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
    /// Placeholder identifier for the singleton global data space.
    pub const GLOBAL: Self = Self(0);

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
        Self::GLOBAL
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
            dataspace_id: DataSpaceId::GLOBAL,
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
                    additions = Some(visitor.parse_value()?);
                }
                "retire" => {
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

/// Validated catalog of configured lanes.
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
    /// uniqueness, or exceeds the configured lane count.
    pub fn new(lane_count: NonZeroU32, lanes: Vec<LaneConfig>) -> Result<Self, LaneCatalogError> {
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

        Ok(Self { lane_count, lanes })
    }

    /// Total number of configured lanes.
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
    /// Returns a [`LaneCatalogError`] when retirements reference unknown lanes or the resulting
    /// catalog is invalid (empty, duplicate identifiers/aliases, or out-of-bounds ids).
    pub fn apply_lifecycle(&self, plan: &LaneLifecyclePlan) -> Result<Self, LaneCatalogError> {
        let retire_set: BTreeSet<LaneId> = plan.retire.iter().copied().collect();
        for retire_id in &retire_set {
            let present = self.lanes.iter().any(|lane| lane.id == *retire_id)
                || plan.additions.iter().any(|lane| lane.id == *retire_id);
            if !present {
                return Err(LaneCatalogError::MissingLane(*retire_id));
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
    /// Alias was left blank.
    #[error("lane {0} has an empty alias")]
    EmptyAlias(LaneId),
    /// Lifecycle plan would leave the catalog empty.
    #[error("lane catalog cannot be empty")]
    EmptyCatalog,
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
    /// Fault tolerance value (f) used to size lane relay committees (3f + 1).
    pub fault_tolerance: u32,
}

impl Default for DataSpaceMetadata {
    fn default() -> Self {
        Self {
            id: DataSpaceId::GLOBAL,
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
        assert_eq!(DataSpaceId::GLOBAL.as_u64(), 0);
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

        let empty_plan = LaneLifecyclePlan {
            additions: Vec::new(),
            retire: vec![LaneId::SINGLE],
        };
        let err = base
            .apply_lifecycle(&empty_plan)
            .expect_err("empty catalog must be rejected");
        assert!(matches!(err, LaneCatalogError::EmptyCatalog));
    }
}

/// Prelude re-export for the Nexus module.
pub mod prelude {
    pub use super::{
        DataSpaceCatalog, DataSpaceCatalogError, DataSpaceId, DataSpaceMetadata, LaneCatalog,
        LaneCatalogError, LaneConfig, LaneId, LaneIdError, LaneLifecyclePlan,
        LaneRelayEmergencyValidatorSet, LaneStorageProfile, LaneStorageProfileParseError,
        LaneVisibility, LaneVisibilityParseError, ShardId,
    };
}
