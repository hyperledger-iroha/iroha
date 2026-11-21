//! Bounded schema-directed compact Norito payload encoding and reconstruction.

use std::io::Cursor as IoCursor;

use norito::{
    Error,
    core::{DeserializePayload, SerializePayload, WireLayout},
};

use super::{
    CARRIER_SCHEMA_HASH_DOMAIN_V1, CarrierValueV1 as Value, MAX_CARRIER_KEY_BYTES,
    MAX_CARRIER_SCHEMA_FRAME_BYTES, MAX_CARRIER_VALUE_NODES,
};
use crate::runtime::definition::{
    CarrierNodeV1 as Node, CarrierSchemaV1, MAX_CARRIER_DEPTH, MAX_CARRIER_LENGTH_BOUND,
    MAX_CARRIER_MEMBERS, MAX_CARRIER_NODES, MAX_DEFINITION_TEXT_BYTES,
};

const FLAGS: u8 = norito::core::header_flags::COMPACT_LEN;

pub(super) fn invalid(context: &'static str) -> Error {
    Error::InvalidValue { context }
}

pub(super) fn schema_hash(schema: &CarrierSchemaV1) -> Result<[u8; 32], Error> {
    validate_schema(schema)?;
    if norito::canonical_frame_len(schema)? > MAX_CARRIER_SCHEMA_FRAME_BYTES {
        return Err(invalid("carrier schema frame exceeds its byte bound"));
    }
    let mut encoded = Ok(());
    let hash = iroha_crypto::Hash::new_from_writer(|writer| {
        writer.write_all(CARRIER_SCHEMA_HASH_DOMAIN_V1)?;
        encoded = norito::core::write_canonical_to_writer(schema, writer);
        Ok(())
    })
    .map_err(Error::Io)?;
    encoded?;
    Ok(hash.into())
}

// This new codec uses the descriptor's exact acceptance relation without borrowing its
// infallible Vec/BTreeSet scratch allocation. The installed descriptor validator is unchanged.
pub(super) fn validate_schema(schema: &CarrierSchemaV1) -> Result<(), Error> {
    if schema.nodes.is_empty() || schema.nodes.len() > MAX_CARRIER_NODES {
        return Err(invalid("invalid carrier schema node count"));
    }
    let mut depths = [0_usize; MAX_CARRIER_NODES];
    let bound = |length: u32| {
        if (1..=MAX_CARRIER_LENGTH_BOUND).contains(&length) {
            Ok(())
        } else {
            Err(invalid("invalid carrier schema length bound"))
        }
    };
    for (index, node) in schema.nodes.iter().enumerate() {
        let child = |reference: u32| {
            if (reference as usize) < index {
                Ok(depths[reference as usize])
            } else {
                Err(invalid("invalid carrier schema child reference"))
            }
        };
        let depth = match node {
            Node::Unit
            | Node::Bool
            | Node::U8
            | Node::U16
            | Node::U32
            | Node::U64
            | Node::U128
            | Node::I64
            | Node::AccountId
            | Node::AssetDefinitionId
            | Node::Hash => 1,
            Node::String { max_bytes } | Node::Bytes { max_bytes } => {
                bound(*max_bytes)?;
                1
            }
            Node::Option { inner } => child(*inner)? + 1,
            Node::Vec { element, max_len } => {
                bound(*max_len)?;
                child(*element)? + 1
            }
            Node::Map {
                key,
                value,
                max_len,
            } => {
                bound(*max_len)?;
                child(*key)?.max(child(*value)?) + 1
            }
            Node::Struct { fields } => {
                members(fields.iter().map(|field| field.name.as_str()))?;
                let mut deepest = 0;
                for field in fields {
                    deepest = deepest.max(child(field.node)?);
                }
                deepest + 1
            }
            Node::Enum { variants } => {
                members(variants.iter().map(|variant| variant.name.as_str()))?;
                if variants
                    .windows(2)
                    .any(|pair| pair[0].discriminant >= pair[1].discriminant)
                {
                    return Err(invalid("invalid carrier schema discriminant order"));
                }
                let mut deepest = 0;
                for variant in variants {
                    deepest = deepest.max(child(variant.node)?);
                }
                deepest + 1
            }
        };
        if depth > MAX_CARRIER_DEPTH {
            return Err(invalid("invalid carrier schema depth"));
        }
        depths[index] = depth;
    }
    Ok(())
}

fn members<'a>(names: impl ExactSizeIterator<Item = &'a str> + Clone) -> Result<(), Error> {
    if names.len() == 0 || names.len() > MAX_CARRIER_MEMBERS {
        return Err(invalid("invalid carrier schema member count"));
    }
    for (index, name) in names.clone().enumerate() {
        if name.is_empty()
            || name.len() > MAX_DEFINITION_TEXT_BYTES
            || !name.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(byte, b'.' | b'_' | b':' | b'-' | b'@' | b'/')
            })
            || names.clone().take(index).any(|prior| prior == name)
        {
            return Err(invalid("invalid carrier schema member name"));
        }
    }
    Ok(())
}

fn reserved<T>(count: usize) -> Result<Vec<T>, Error> {
    let bytes = count
        .checked_mul(core::mem::size_of::<T>())
        .ok_or(Error::LengthMismatch)?;
    norito::core::reserve_decode_allocation(bytes)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Error::AllocationFailed {
            bytes: bytes as u64,
        })?;
    Ok(values)
}

struct Output {
    bytes: Vec<u8>,
    limit: usize,
}

impl Output {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }

    fn append(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let needed = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or(Error::LengthMismatch)?;
        if needed > self.limit {
            return Err(invalid("carrier payload exceeds its byte bound"));
        }
        if needed > self.bytes.capacity() {
            let capacity = needed.max(
                self.bytes
                    .capacity()
                    .saturating_mul(2)
                    .max(64)
                    .min(self.limit),
            );
            norito::core::reserve_decode_allocation(capacity - self.bytes.capacity())?;
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .map_err(|_| Error::AllocationFailed {
                    bytes: capacity as u64,
                })?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }

    fn field(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let mut prefix = [0_u8; 10];
        let mut cursor = IoCursor::new(prefix.as_mut_slice());
        norito::core::write_len_with_flags(&mut cursor, bytes.len() as u64, FLAGS)?;
        let length = cursor.position() as usize;
        self.append(&prefix[..length])?;
        self.append(bytes)
    }

    fn count(&mut self, count: usize) -> Result<(), Error> {
        self.append(&(count as u64).to_le_bytes())
    }
}

#[derive(Default)]
struct Work {
    nodes: usize,
}

impl Work {
    fn visit(&mut self, depth: usize) -> Result<(), Error> {
        if depth > MAX_CARRIER_DEPTH || self.nodes == MAX_CARRIER_VALUE_NODES {
            return Err(invalid("carrier value exceeds its node or depth bound"));
        }
        self.nodes += 1;
        Ok(())
    }

    fn children(&self, count: usize) -> Result<(), Error> {
        if count > MAX_CARRIER_VALUE_NODES - self.nodes {
            return Err(invalid("carrier value exceeds its node bound"));
        }
        Ok(())
    }
}

fn encoded<T: SerializePayload>(value: &T, limit: usize) -> Result<Vec<u8>, Error> {
    if norito::core::encoded_payload_len(value)? > limit {
        return Err(invalid("carrier payload exceeds its byte bound"));
    }
    norito::codec::encode_bare_with_layout(value, WireLayout::Compact)
}

pub(super) fn encode(
    schema: &CarrierSchemaV1,
    value: &Value,
    limit: usize,
) -> Result<Vec<u8>, Error> {
    let _layout = norito::core::WireLayoutGuard::enter(WireLayout::Compact);
    encode_node(
        schema,
        schema.nodes.len() - 1,
        value,
        limit,
        1,
        &mut Work::default(),
    )
}

fn encode_node(
    schema: &CarrierSchemaV1,
    index: usize,
    value: &Value,
    limit: usize,
    depth: usize,
    work: &mut Work,
) -> Result<Vec<u8>, Error> {
    work.visit(depth)?;
    let mut output = Output::new(limit);
    let child = |index: u32, value: &Value, work: &mut Work| {
        encode_node(schema, index as usize, value, limit, depth + 1, work)
    };
    match (&schema.nodes[index], value) {
        (Node::Unit, Value::Unit) => {}
        (Node::Bool, Value::Bool(value)) => return encoded(value, limit),
        (Node::U8, Value::U8(value)) => return encoded(value, limit),
        (Node::U16, Value::U16(value)) => return encoded(value, limit),
        (Node::U32, Value::U32(value)) => return encoded(value, limit),
        (Node::U64, Value::U64(value)) => return encoded(value, limit),
        (Node::U128, Value::U128(value)) => return encoded(value, limit),
        (Node::I64, Value::I64(value)) => return encoded(value, limit),
        (Node::AccountId, Value::AccountId(value)) => return encoded(value, limit),
        (Node::AssetDefinitionId, Value::AssetDefinitionId(value)) => return encoded(value, limit),
        (Node::Hash, Value::Hash(value)) => return encoded(value, limit),
        (Node::String { max_bytes }, Value::String(value))
            if value.len() <= *max_bytes as usize =>
        {
            return encoded(value, limit);
        }
        (Node::Bytes { max_bytes }, Value::Bytes(value)) if value.len() <= *max_bytes as usize => {
            return encoded(value, limit);
        }
        (Node::Option { inner }, Value::Option(values)) if values.len() <= 1 => {
            work.children(values.len())?;
            output.append(&[values.len() as u8])?;
            if let Some(value) = values.first() {
                output.field(&child(*inner, value, work)?)?;
            }
        }
        (Node::Vec { element, max_len }, Value::Vec(values))
            if values.len() <= *max_len as usize =>
        {
            work.children(values.len())?;
            output.count(values.len())?;
            for value in values {
                output.field(&child(*element, value, work)?)?;
            }
        }
        (
            Node::Map {
                key,
                value: node,
                max_len,
            },
            Value::Map(entries),
        ) if entries.len() <= *max_len as usize => {
            work.children(entries.len().checked_mul(2).ok_or(Error::LengthMismatch)?)?;
            output.count(entries.len())?;
            let mut previous: Option<Vec<u8>> = None;
            for (key_value, value) in entries {
                let key = encode_node(
                    schema,
                    *key as usize,
                    key_value,
                    limit.min(MAX_CARRIER_KEY_BYTES),
                    depth + 1,
                    work,
                )?;
                if previous.as_ref().is_some_and(|previous| previous >= &key) {
                    return Err(invalid("carrier map keys are not strictly byte ordered"));
                }
                output.field(&key)?;
                output.field(&child(*node, value, work)?)?;
                previous = Some(key);
            }
        }
        (Node::Struct { fields }, Value::Struct(values)) if fields.len() == values.len() => {
            work.children(values.len())?;
            for (field, value) in fields.iter().zip(values) {
                output.field(&child(field.node, value, work)?)?;
            }
        }
        (
            Node::Enum { variants },
            Value::Enum {
                discriminant,
                payload,
            },
        ) if payload.len() == 1 => {
            let variant = variants
                .binary_search_by_key(discriminant, |variant| variant.discriminant)
                .map(|index| &variants[index])
                .map_err(|_| invalid("unknown carrier enum discriminant"))?;
            work.children(1)?;
            output.append(&discriminant.to_le_bytes())?;
            output.field(&child(variant.node, &payload[0], work)?)?;
        }
        _ => return Err(invalid("carrier value does not match its schema")),
    }
    Ok(output.bytes)
}

struct Input<'a> {
    rest: &'a [u8],
}

impl<'a> Input<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let value = self.rest.get(..length).ok_or(Error::LengthMismatch)?;
        self.rest = &self.rest[length..];
        Ok(value)
    }

    fn field(&mut self) -> Result<&'a [u8], Error> {
        let (length, prefix) = norito::core::read_len_from_slice_with_flags(self.rest, FLAGS)?;
        self.take(prefix)?;
        self.take(length)
    }

    fn count(&mut self, max: usize, children_per_item: usize, work: &Work) -> Result<usize, Error> {
        let count = sequence_count(self.rest)?;
        if count > max {
            return Err(invalid("carrier sequence exceeds its declared bound"));
        }
        work.children(
            count
                .checked_mul(children_per_item)
                .ok_or(Error::LengthMismatch)?,
        )?;
        // After deterministic preflight, retain the ordinary decoder's original sequence and
        // allocation-budget checks. Inspecting the count alone grants no decoded validity.
        let (checked, used) = norito::core::read_seq_len_slice(self.rest)?;
        self.take(used)?;
        // Every child has at least its one-byte compact field prefix, even Unit.
        // Refuse impossible extents before reserving the reconstructed container.
        if count
            .checked_mul(children_per_item)
            .ok_or(Error::LengthMismatch)?
            > self.rest.len()
        {
            return Err(Error::LengthMismatch);
        }
        Ok(checked)
    }

    fn finish(self) -> Result<(), Error> {
        if self.rest.is_empty() {
            Ok(())
        } else {
            Err(Error::LengthMismatch)
        }
    }
}

fn sequence_count(bytes: &[u8]) -> Result<usize, Error> {
    let prefix: [u8; 8] = bytes
        .get(..8)
        .ok_or(Error::LengthMismatch)?
        .try_into()
        .map_err(|_| Error::LengthMismatch)?;
    usize::try_from(u64::from_le_bytes(prefix)).map_err(|_| Error::LengthMismatch)
}

fn scalar<T>(bytes: &[u8]) -> Result<T, Error>
where
    T: SerializePayload + for<'de> DeserializePayload<'de>,
{
    norito::with_decode_limits(norito::canonical_decode_limits(bytes.len()), || {
        scalar_in_context(bytes)
    })
}

fn scalar_in_context<T>(bytes: &[u8]) -> Result<T, Error>
where
    T: SerializePayload + for<'de> DeserializePayload<'de>,
{
    let (value, used) = norito::core::decode_field_canonical::<T>(bytes)?;
    if used != bytes.len() {
        return Err(Error::LengthMismatch);
    }
    Ok(value)
}

pub(super) fn record_payload<'a>(frame: &'a [u8], expected: &[u8; 32]) -> Result<&'a [u8], Error> {
    let bytes = norito::canonical_frame_payload::<super::CarrierRecordV1>(frame, FLAGS)?;
    let _layout = norito::core::WireLayoutGuard::enter(WireLayout::Compact);
    let mut record = Input { rest: bytes };
    // Derive's fixed byte-array field is raw bytes inside its field prefix; the
    // standalone array codec has different child framing. Preserve that exact DTO rule.
    if record.field()? != expected {
        return Err(invalid("carrier schema hash mismatch"));
    }
    let value = record.field()?;
    record.finish()?;
    raw_bytes(value)
}

pub(super) fn raw_bytes(bytes: &[u8]) -> Result<&[u8], Error> {
    let mut value = Input { rest: bytes };
    let (length, prefix) = norito::core::read_seq_len_slice(value.rest)?;
    value.take(prefix)?;
    let payload = value.take(length)?;
    value.finish()?;
    Ok(payload)
}

pub(super) fn storage_key(bytes: &[u8]) -> Result<(super::DefinitionId, &[u8]), Error> {
    let _layout = norito::core::WireLayoutGuard::enter(WireLayout::Compact);
    let mut key = Input { rest: bytes };
    let storage = scalar_in_context::<super::DefinitionId>(key.field()?)?;
    let payload = raw_bytes(key.field()?)?;
    key.finish()?;
    if payload.len() > super::MAX_CARRIER_KEY_BYTES {
        return Err(invalid("carrier storage key exceeds its byte bound"));
    }
    Ok((storage, payload))
}

pub(super) fn decode(schema: &CarrierSchemaV1, bytes: &[u8], limit: usize) -> Result<Value, Error> {
    if bytes.len() > limit {
        return Err(invalid("carrier payload exceeds its byte bound"));
    }
    let _layout = norito::core::WireLayoutGuard::enter(WireLayout::Compact);
    decode_node(
        schema,
        schema.nodes.len() - 1,
        bytes,
        1,
        &mut Work::default(),
    )
}

fn decode_node(
    schema: &CarrierSchemaV1,
    index: usize,
    bytes: &[u8],
    depth: usize,
    work: &mut Work,
) -> Result<Value, Error> {
    if matches!(
        schema.nodes[index],
        Node::Unit
            | Node::Option { .. }
            | Node::Vec { .. }
            | Node::Map { .. }
            | Node::Struct { .. }
            | Node::Enum { .. }
    ) {
        return norito::core::with_decode_depth_scope(|| {
            decode_node_inner(schema, index, bytes, depth, work)
        });
    }
    decode_node_inner(schema, index, bytes, depth, work)
}

fn decode_node_inner(
    schema: &CarrierSchemaV1,
    index: usize,
    bytes: &[u8],
    depth: usize,
    work: &mut Work,
) -> Result<Value, Error> {
    work.visit(depth)?;
    let mut input = Input { rest: bytes };
    let child = |index: u32, bytes: &[u8], work: &mut Work| {
        decode_node(schema, index as usize, bytes, depth + 1, work)
    };
    let value = match &schema.nodes[index] {
        Node::Unit => Value::Unit,
        Node::Bool => return scalar(bytes).map(Value::Bool),
        Node::U8 => return scalar(bytes).map(Value::U8),
        Node::U16 => return scalar(bytes).map(Value::U16),
        Node::U32 => return scalar(bytes).map(Value::U32),
        Node::U64 => return scalar(bytes).map(Value::U64),
        Node::U128 => return scalar(bytes).map(Value::U128),
        Node::I64 => return scalar(bytes).map(Value::I64),
        Node::AccountId => return scalar(bytes).map(Value::AccountId),
        Node::AssetDefinitionId => return scalar(bytes).map(Value::AssetDefinitionId),
        Node::Hash => return scalar(bytes).map(Value::Hash),
        Node::String { max_bytes } => {
            let (length, _) = norito::core::inspect_raw_len_from_slice_with_flags(bytes, FLAGS)?;
            if length > *max_bytes as usize {
                return Err(invalid("carrier string exceeds its declared bound"));
            }
            return scalar(bytes).map(Value::String);
        }
        Node::Bytes { max_bytes } => {
            if sequence_count(bytes)? > *max_bytes as usize {
                return Err(invalid("carrier bytes exceed their declared bound"));
            }
            return scalar(bytes).map(Value::Bytes);
        }
        Node::Option { inner } => match input.take(1)?[0] {
            0 => Value::Option(Vec::new()),
            1 => {
                work.children(1)?;
                let bytes = input.field()?;
                let mut values = reserved(1)?;
                values.push(child(*inner, bytes, work)?);
                Value::Option(values)
            }
            _ => return Err(invalid("invalid carrier option tag")),
        },
        Node::Vec { element, max_len } => {
            let count = input.count(*max_len as usize, 1, work)?;
            let mut values = reserved(count)?;
            for _ in 0..count {
                values.push(child(*element, input.field()?, work)?);
            }
            Value::Vec(values)
        }
        Node::Map {
            key,
            value,
            max_len,
        } => {
            let count = input.count(*max_len as usize, 2, work)?;
            let mut entries = reserved(count)?;
            let mut previous: Option<&[u8]> = None;
            for _ in 0..count {
                let key_bytes = input.field()?;
                if key_bytes.len() > MAX_CARRIER_KEY_BYTES {
                    return Err(invalid("carrier map key exceeds its byte bound"));
                }
                if previous.is_some_and(|previous| previous >= key_bytes) {
                    return Err(invalid("carrier map keys are not strictly byte ordered"));
                }
                let key = child(*key, key_bytes, work)?;
                let value = child(*value, input.field()?, work)?;
                entries.push((key, value));
                previous = Some(key_bytes);
            }
            Value::Map(entries)
        }
        Node::Struct { fields } => {
            work.children(fields.len())?;
            if fields.len() > input.rest.len() {
                return Err(Error::LengthMismatch);
            }
            let mut values = reserved(fields.len())?;
            for field in fields {
                values.push(child(field.node, input.field()?, work)?);
            }
            Value::Struct(values)
        }
        Node::Enum { variants } => {
            let discriminant = u32::from_le_bytes(
                input
                    .take(4)?
                    .try_into()
                    .map_err(|_| Error::LengthMismatch)?,
            );
            let variant = variants
                .binary_search_by_key(&discriminant, |variant| variant.discriminant)
                .map(|index| &variants[index])
                .map_err(|_| invalid("unknown carrier enum discriminant"))?;
            work.children(1)?;
            let bytes = input.field()?;
            let mut payload = reserved(1)?;
            payload.push(child(variant.node, bytes, work)?);
            Value::Enum {
                discriminant,
                payload,
            }
        }
    };
    input.finish()?;
    Ok(value)
}

/// Validate the same finite value language without materializing any value graph.
/// Each leaf/container consumes the original enclosing counters; no replacement scope is made.
pub(super) fn validate(schema: &CarrierSchemaV1, bytes: &[u8], limit: usize) -> Result<(), Error> {
    if bytes.len() > limit {
        return Err(invalid("carrier payload exceeds its byte bound"));
    }
    let _layout = norito::core::WireLayoutGuard::enter(WireLayout::Compact);
    validate_node(
        schema,
        schema.nodes.len() - 1,
        bytes,
        1,
        &mut Work::default(),
    )
}

fn checked_scalar<T>(bytes: &[u8]) -> Result<(), Error>
where
    T: SerializePayload + for<'de> DeserializePayload<'de>,
{
    let (_, used) = norito::core::decode_field_canonical::<T>(bytes)?;
    if used != bytes.len() {
        return Err(Error::LengthMismatch);
    }
    Ok(())
}

fn validate_node(
    schema: &CarrierSchemaV1,
    index: usize,
    bytes: &[u8],
    depth: usize,
    work: &mut Work,
) -> Result<(), Error> {
    if matches!(
        schema.nodes[index],
        Node::Unit
            | Node::Option { .. }
            | Node::Vec { .. }
            | Node::Map { .. }
            | Node::Struct { .. }
            | Node::Enum { .. }
    ) {
        return norito::core::with_decode_depth_scope(|| {
            validate_node_inner(schema, index, bytes, depth, work)
        });
    }
    validate_node_inner(schema, index, bytes, depth, work)
}

fn validate_node_inner(
    schema: &CarrierSchemaV1,
    index: usize,
    bytes: &[u8],
    depth: usize,
    work: &mut Work,
) -> Result<(), Error> {
    work.visit(depth)?;
    let mut input = Input { rest: bytes };
    let child = |index: u32, bytes: &[u8], work: &mut Work| {
        validate_node(schema, index as usize, bytes, depth + 1, work)
    };
    match &schema.nodes[index] {
        Node::Unit => {}
        Node::Bool => return checked_scalar::<bool>(bytes),
        Node::U8 => return checked_scalar::<u8>(bytes),
        Node::U16 => return checked_scalar::<u16>(bytes),
        Node::U32 => return checked_scalar::<u32>(bytes),
        Node::U64 => return checked_scalar::<u64>(bytes),
        Node::U128 => return checked_scalar::<u128>(bytes),
        Node::I64 => return checked_scalar::<i64>(bytes),
        Node::Hash => return checked_scalar::<iroha_crypto::Hash>(bytes),
        Node::AccountId => return crate::account::borrowed_decode::validate(bytes),
        Node::AssetDefinitionId => {
            let (uuid, used) = norito::core::decode_field_canonical::<[u8; 16]>(bytes)?;
            if used != bytes.len() {
                return Err(Error::LengthMismatch);
            }
            return crate::asset::AssetDefinitionId::from_uuid_bytes(uuid)
                .map(|_| ())
                .map_err(|_| invalid("invalid asset definition identity"));
        }
        Node::String { max_bytes } => {
            let text = input.field()?;
            if text.len() > *max_bytes as usize {
                return Err(invalid("carrier string exceeds its declared bound"));
            }
            std::str::from_utf8(text).map_err(|_| invalid("invalid carrier string UTF-8"))?;
        }
        Node::Bytes { max_bytes } => {
            if sequence_count(bytes)? > *max_bytes as usize {
                return Err(invalid("carrier bytes exceed their declared bound"));
            }
            return raw_bytes(bytes).map(|_| ());
        }
        Node::Option { inner } => match input.take(1)?[0] {
            0 => {}
            1 => {
                work.children(1)?;
                child(*inner, input.field()?, work)?;
            }
            _ => return Err(invalid("invalid carrier option tag")),
        },
        Node::Vec { element, max_len } => {
            let count = input.count(*max_len as usize, 1, work)?;
            for _ in 0..count {
                child(*element, input.field()?, work)?;
            }
        }
        Node::Map {
            key,
            value,
            max_len,
        } => {
            let count = input.count(*max_len as usize, 2, work)?;
            let mut previous: Option<&[u8]> = None;
            for _ in 0..count {
                let key_bytes = input.field()?;
                if key_bytes.len() > MAX_CARRIER_KEY_BYTES {
                    return Err(invalid("carrier map key exceeds its byte bound"));
                }
                if previous.is_some_and(|previous| previous >= key_bytes) {
                    return Err(invalid("carrier map keys are not strictly byte ordered"));
                }
                child(*key, key_bytes, work)?;
                child(*value, input.field()?, work)?;
                previous = Some(key_bytes);
            }
        }
        Node::Struct { fields } => {
            work.children(fields.len())?;
            if fields.len() > input.rest.len() {
                return Err(Error::LengthMismatch);
            }
            for field in fields {
                child(field.node, input.field()?, work)?;
            }
        }
        Node::Enum { variants } => {
            let discriminant = u32::from_le_bytes(
                input
                    .take(4)?
                    .try_into()
                    .map_err(|_| Error::LengthMismatch)?,
            );
            let variant = variants
                .binary_search_by_key(&discriminant, |variant| variant.discriminant)
                .map(|index| &variants[index])
                .map_err(|_| invalid("unknown carrier enum discriminant"))?;
            work.children(1)?;
            child(variant.node, input.field()?, work)?;
        }
    }
    input.finish()
}
