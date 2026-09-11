//! Numeric topology identities, canonical codecs and storage keys.

use derive_more::Display;
use iroha_schema::IntoSchema;
use mv::json::JsonKeyCodec;
use norito::{
    codec::{Decode, Encode},
    json,
};
use std::{num::NonZeroU32, str::FromStr};
use thiserror::Error;

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
#[norito(decode_from_slice)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(unsafe {robust})
)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::nexus::LaneId")]
pub struct LaneId(u32);

/// Identifier for a storage shard within a data space.
///
/// Shards map to DA/Kura partitions; today they track lane bindings one-to-one
/// but remain distinct to allow future resharding. A shard is not a separate
/// validator/server boundary; that identity belongs to [`DataSpaceId`].
#[derive(
    Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema,
)]
#[repr(transparent)]
#[norito(decode_from_slice)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(unsafe {robust})
)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::nexus::ShardId")]
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

impl norito::json::FastJsonWrite for LaneId {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.0, out);
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::JsonSerialize::json_serialize_to(&self.0, out)
    }
}

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

impl norito::json::JsonObjectKey for LaneId {
    fn visit_json_key_text<E>(&self, visitor: impl FnMut(&str) -> Result<(), E>) -> Result<(), E> {
        norito::json::JsonObjectKey::visit_json_key_text(&self.0, visitor)
    }
}

impl norito::json::JsonObjectKeyOwned for LaneId {
    fn from_json_key_text(key: &str) -> Result<Self, norito::json::Error> {
        <u32 as norito::json::JsonObjectKeyOwned>::from_json_key_text(key).map(Self)
    }
}

impl norito::json::FastJsonWrite for ShardId {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.0, out);
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::JsonSerialize::json_serialize_to(&self.0, out)
    }
}

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
/// Identifier for a physical execution, storage, and validator boundary.
#[derive(
    Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema,
)]
#[repr(transparent)]
#[norito(decode_from_slice)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    derive(iroha_ffi::FfiType)
)]
#[cfg_attr(
    all(feature = "ffi_export", not(feature = "ffi_import")),
    ffi_type(unsafe {robust})
)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::nexus::DataSpaceId")]
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
impl norito::json::FastJsonWrite for DataSpaceId {
    fn write_json(&self, out: &mut String) {
        norito::json::JsonSerialize::json_serialize(&self.0, out);
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::JsonSerialize::json_serialize_to(&self.0, out)
    }
}

impl norito::json::JsonDeserialize for DataSpaceId {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_u64()?;
        Ok(Self(value))
    }
}

impl norito::json::JsonObjectKey for DataSpaceId {
    fn visit_json_key_text<E>(&self, visitor: impl FnMut(&str) -> Result<(), E>) -> Result<(), E> {
        norito::json::JsonObjectKey::visit_json_key_text(&self.0, visitor)
    }
}

impl norito::json::JsonObjectKeyOwned for DataSpaceId {
    fn from_json_key_text(key: &str) -> Result<Self, norito::json::Error> {
        <u64 as norito::json::JsonObjectKeyOwned>::from_json_key_text(key).map(Self)
    }
}

impl JsonKeyCodec for DataSpaceId {
    fn encode_json_key(&self, out: &mut String) {
        <u64 as JsonKeyCodec>::encode_json_key(&self.as_u64(), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <u64 as JsonKeyCodec>::decode_json_key(encoded).map(Self::from)
    }
}

impl JsonKeyCodec for LaneId {
    fn encode_json_key(&self, out: &mut String) {
        <u64 as JsonKeyCodec>::encode_json_key(&u64::from(self.as_u32()), out);
    }
    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        <u64 as JsonKeyCodec>::decode_json_key(encoded).and_then(|value| {
            u32::try_from(value)
                .map(LaneId::new)
                .map_err(|_| json::Error::Message("lane id out of range".into()))
        })
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod wire_identity_tests;
