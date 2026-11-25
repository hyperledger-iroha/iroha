//! Schema-directed canonical carrier codec for a separately selected runtime profile.
//!
//! This module grants no execution or storage authority and changes no installed definition.
//! A carrier record always uses Norito's explicit current compact layout. Its inner payload is
//! interpreted only with the exact schema hash in the record. Map entries use increasing bare
//! encoded key bytes, which deliberately differs from a native `BTreeMap<K, V>`'s typed `Ord`.
//! Admission can validate the original frame and key without reconstructing a value graph.
//! These borrowed validators reuse native key/account rules and the caller's original decode
//! controls; schema hashing streams the exact canonical frame instead of copying it.
//! Core connects this codec to the dormant generic-carrier kernel/host profile, scoped storage
//! capabilities, ordered host effects and selected keyed State. Support and activation remain
//! refused until the combined execution, rollback and replay qualification succeeds.

use iroha_crypto::Hash;
use norito::{Decode, Encode, core::DecodeAttemptError};

use super::definition::{CarrierSchemaV1, DefinitionId};
use crate::{account::AccountId, asset::AssetDefinitionId};

mod codec;
pub mod keyed;
#[cfg(test)]
mod tests;

/// Maximum complete canonical carrier record frame, including its header.
pub const MAX_CARRIER_FRAME_BYTES: usize = 1 << 20;
/// Maximum complete canonical schema frame accepted by this carrier codec.
pub const MAX_CARRIER_SCHEMA_FRAME_BYTES: usize = 64 << 10;
/// Maximum number of semantic value nodes in one carrier value, including its root.
pub const MAX_CARRIER_VALUE_NODES: usize = 65_536;
/// Maximum bare canonical payload of a map or storage key.
pub const MAX_CARRIER_KEY_BYTES: usize = 4096;
/// Domain binding a record to its exact canonical schema frame.
pub const CARRIER_SCHEMA_HASH_DOMAIN_V1: &[u8] = b"iroha:runtime:carrier-schema:v1\0";

/// Recompute the existing definition identity through the original canonical serializer.
///
/// This new-profile helper uses the exact existing identity domain and framing and preserves
/// codec refusals instead of passing through the installed descriptor API's string error.
/// It does not validate the descriptor: callers must enforce their selected contract first.
///
/// # Errors
/// An original serializer or hashing writer failure, including unchanged local refusal.
pub fn carrier_definition_id_v1(
    definition: &super::definition::DefinitionDescriptorV1,
) -> Result<DefinitionId, DecodeAttemptError> {
    norito::core::classify_decode_attempt(|| {
        use super::definition::{DEFINITION_FORMAT_VERSION, DEFINITION_ID_DOMAIN};
        let length = norito::canonical_frame_len(definition)?;
        let mut encoded = Ok(());
        let hash = Hash::new_from_writer(|writer| {
            writer.write_all(DEFINITION_ID_DOMAIN)?;
            writer.write_all(&[DEFINITION_FORMAT_VERSION, definition.kind().tag()])?;
            writer.write_all(&(length as u64).to_le_bytes())?;
            encoded = norito::core::write_canonical_to_writer(definition, writer);
            Ok(())
        })
        .map_err(norito::Error::Io)?;
        encoded?;
        Ok(DefinitionId(hash.into()))
    })
}

/// Fixed nominal envelope for a value whose schema is supplied by a runtime definition.
///
/// Decoding this envelope alone does not validate its payload. Use [`decode_carrier_v1`] at
/// an admission boundary; constructing this DTO supplies no proof or execution capability.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, iroha_schema::IntoSchema, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha.runtime.CarrierRecordV1")]
pub struct CarrierRecordV1 {
    /// Domain-separated hash of the complete canonical schema frame.
    pub schema_hash: [u8; 32],
    /// Bare schema-directed value in the explicit current compact Norito layout.
    pub payload: Vec<u8>,
}

/// Physical position in the separate generic-value backing table.
///
/// This serializable position grants no storage authority. The invocation owner must resolve
/// `storage` from the selected implementation's dependencies and validate `key` under its exact
/// carrier schema before acquiring an access capability. Ordering is definition identity then
/// bare encoded key bytes.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
    norito::NoritoSchema,
    crate::DeriveJsonSerialize,
    crate::DeriveJsonDeserialize,
)]
#[norito_schema(name = "iroha.runtime.CarrierStorageKeyV1")]
#[norito(deny_unknown_fields)]
pub struct CarrierStorageKeyV1 {
    /// Exact immutable storage definition owning this row.
    pub storage: DefinitionId,
    /// Bare compact canonical key payload, bounded by [`MAX_CARRIER_KEY_BYTES`].
    #[norito(json = "crate::json_helpers::base64_vec")]
    pub key: Vec<u8>,
}

/// Borrow the definition identity and raw key from one exact bare physical key encoding.
/// This checks only the physical wrapper; the selected owner's schema must validate the key.
///
/// # Errors
/// Noncanonical compact fields, extra bytes, a key above its bound or original decoder refusal.
pub fn borrow_carrier_storage_key_v1(
    bytes: &[u8],
) -> Result<(DefinitionId, &[u8]), DecodeAttemptError> {
    norito::core::classify_decode_attempt(|| codec::storage_key(bytes))
}

/// Borrow one exact native raw-byte `Vec<u8>` payload from the physical storage owner.
/// The result is not a validated carrier record until checked under its selected schema.
///
/// # Errors
/// A count/extents mismatch or original sequence limit refusal.
pub fn borrow_carrier_storage_value_v1(bytes: &[u8]) -> Result<&[u8], DecodeAttemptError> {
    norito::core::classify_decode_attempt(|| codec::raw_bytes(bytes))
}

/// Programmatic carrier value; its wire representation is selected by [`CarrierSchemaV1`].
///
/// Container children are vectors so reconstruction uses fallible, charged reservations rather
/// than infallible recursive box allocations. The codec checks every container's exact shape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CarrierValueV1 {
    /// The empty value.
    Unit,
    /// A boolean.
    Bool(bool),
    /// An unsigned 8-bit integer.
    U8(u8),
    /// An unsigned 16-bit integer.
    U16(u16),
    /// An unsigned 32-bit integer.
    U32(u32),
    /// An unsigned 64-bit integer.
    U64(u64),
    /// An unsigned 128-bit integer.
    U128(u128),
    /// A signed 64-bit integer.
    I64(i64),
    /// A bounded UTF-8 string.
    String(String),
    /// Bounded raw bytes.
    Bytes(Vec<u8>),
    /// A canonical domainless account identity.
    AccountId(AccountId),
    /// A canonical asset definition identity.
    AssetDefinitionId(AssetDefinitionId),
    /// A canonical chain hash, including its normal marker validation.
    Hash(Hash),
    /// Zero children for `None`, exactly one child for `Some`.
    Option(Vec<Self>),
    /// Sequence elements in their declared order.
    Vec(Vec<Self>),
    /// Entries strictly ordered by bare encoded key bytes; duplicate keys are forbidden.
    Map(Vec<(Self, Self)>),
    /// Field values in the schema's declaration order.
    Struct(Vec<Self>),
    /// Explicit discriminant followed by exactly one payload child.
    Enum {
        /// Discriminant declared by the selected schema.
        discriminant: u32,
        /// Exactly one child, including when the selected payload node is `Unit`.
        payload: Vec<Self>,
    },
}

/// Hash one valid bounded schema using its exact canonical compact Norito frame.
///
/// # Errors
/// Invalid schemas or codec failures. Original enclosing-budget and allocator refusals retain
/// their [`DecodeAttemptError`] classification and are not deterministic malformed-input results.
pub fn carrier_schema_hash_v1(schema: &CarrierSchemaV1) -> Result<[u8; 32], DecodeAttemptError> {
    norito::core::classify_decode_attempt(|| codec::schema_hash(schema))
}

/// Encode a value as one complete, bounded, schema-bound canonical carrier record.
///
/// # Errors
/// A schema/value mismatch, noncanonical map order, protocol bound or original codec refusal.
pub fn encode_carrier_v1(
    schema: &CarrierSchemaV1,
    value: &CarrierValueV1,
) -> Result<Vec<u8>, DecodeAttemptError> {
    norito::core::classify_decode_attempt(|| {
        let schema_hash = codec::schema_hash(schema)?;
        let payload = codec::encode(schema, value, MAX_CARRIER_FRAME_BYTES)?;
        let record = CarrierRecordV1 {
            schema_hash,
            payload,
        };
        if norito::canonical_frame_len(&record)? > MAX_CARRIER_FRAME_BYTES {
            return Err(codec::invalid("carrier frame exceeds its byte bound"));
        }
        norito::encode_canonical(&record)
    })
}

/// Decode exactly one bounded record against the definition's exact schema.
///
/// # Errors
/// Wrong frame layout or schema, noncanonical values, suffixes, protocol bounds or an original
/// local codec refusal. No alternate decoder or layout is attempted.
pub fn decode_carrier_v1(
    schema: &CarrierSchemaV1,
    frame: &[u8],
) -> Result<CarrierValueV1, DecodeAttemptError> {
    norito::core::classify_decode_attempt(|| {
        if frame.len() > MAX_CARRIER_FRAME_BYTES {
            return Err(codec::invalid("carrier frame exceeds its byte bound"));
        }
        let expected = codec::schema_hash(schema)?;
        let payload = codec::record_payload(frame, &expected)?;
        codec::decode(schema, payload, MAX_CARRIER_FRAME_BYTES)
    })
}

/// Validate a complete record and borrow its exact bare value for keyed commitment.
///
/// The returned bytes stay owned by the original frame. Validation grants no write authority;
/// the caller must separately retain its exact selected storage descriptor and source custody.
///
/// # Errors
/// The same schema, canonical framing and value acceptance as [`decode_carrier_v1`].
/// Original resource refusals remain typed; no value allocation or new limit scope is created.
pub fn validated_carrier_payload_v1<'a>(
    schema: &CarrierSchemaV1,
    frame: &'a [u8],
) -> Result<&'a [u8], DecodeAttemptError> {
    norito::core::classify_decode_attempt(|| {
        if frame.len() > MAX_CARRIER_FRAME_BYTES {
            return Err(codec::invalid("carrier frame exceeds its byte bound"));
        }
        let expected = codec::schema_hash(schema)?;
        let payload = codec::record_payload(frame, &expected)?;
        codec::validate(schema, payload, MAX_CARRIER_FRAME_BYTES)?;
        Ok(payload)
    })
}

/// Encode a bounded bare key for carrier-map ordering or capability-owned storage.
///
/// The bytes carry no schema or authority by themselves. Their owner must retain the exact
/// schema definition; ordering compares these bytes lexicographically.
///
/// # Errors
/// Invalid schema/value, key above its byte bound or an original codec refusal.
pub fn encode_carrier_key_v1(
    schema: &CarrierSchemaV1,
    value: &CarrierValueV1,
) -> Result<Vec<u8>, DecodeAttemptError> {
    norito::core::classify_decode_attempt(|| {
        codec::schema_hash(schema)?;
        codec::encode(schema, value, MAX_CARRIER_KEY_BYTES)
    })
}

/// Decode a bare key only under its owner's exact bounded schema.
///
/// # Errors
/// Noncanonical bytes, a schema/value mismatch, key bound or original codec refusal.
pub fn decode_carrier_key_v1(
    schema: &CarrierSchemaV1,
    payload: &[u8],
) -> Result<CarrierValueV1, DecodeAttemptError> {
    norito::core::classify_decode_attempt(|| {
        codec::schema_hash(schema)?;
        codec::decode(schema, payload, MAX_CARRIER_KEY_BYTES)
    })
}

/// Validate one exact bare carrier key without allocating a reconstructed value.
///
/// The returned slice keeps the caller's original bytes and grants no storage capability.
/// Native leaves use their existing canonical relations under the original enclosing limits.
///
/// # Errors
/// Invalid schema, extent, canonical value or unchanged original codec resource refusal.
pub fn validated_carrier_key_v1<'a>(
    schema: &CarrierSchemaV1,
    payload: &'a [u8],
) -> Result<&'a [u8], DecodeAttemptError> {
    norito::core::classify_decode_attempt(|| {
        codec::schema_hash(schema)?;
        codec::validate(schema, payload, MAX_CARRIER_KEY_BYTES)?;
        Ok(payload)
    })
}
