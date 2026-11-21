//! Explicit opt-in schema interpretation for the new carrier-aware commitment profile.

use super::*;
use crate::runtime::{
    carrier::{
        MAX_CARRIER_SCHEMA_FRAME_BYTES, carrier_definition_id_v1, carrier_schema_hash_v1, keyed,
    },
    definition::{
        CarrierSchemaV1, MAX_DEFINITION_REFERENCES, MAX_DEFINITION_TEXT_BYTES, MAX_INITIAL_BYTES,
        ProjectionV1,
    },
};

const CARRIER_CODEC_PREFIX: &str = "carrier:compact-v1:";

/// A carrier schema rejection or the unchanged original codec resource refusal.
#[derive(Debug, thiserror::Error)]
pub enum CarrierStateSchemaError {
    /// Published definitions do not satisfy the selected profile contract.
    #[error("invalid carrier State schema: {0}")]
    Invalid(&'static str),
    /// Original bounded codec attempt, retaining its local refusal provenance.
    #[error("carrier State schema codec: {0}")]
    Codec(#[source] norito::core::DecodeAttemptError),
}

fn invalid(reason: &'static str) -> CarrierStateSchemaError {
    CarrierStateSchemaError::Invalid(reason)
}

fn codec_size(schema: &SchemaV1) -> Result<usize, CarrierStateSchemaError> {
    match schema {
        SchemaV1::Carrier(_) => Ok(CARRIER_CODEC_PREFIX.len() + 64),
        SchemaV1::Codec(name)
            if !name.is_empty()
                && name.len() <= MAX_DEFINITION_TEXT_BYTES
                && !name.starts_with(' ')
                && !name.ends_with(' ')
                && name.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) =>
        {
            Ok(name.len())
        }
        _ => Err(invalid("invalid native codec identity")),
    }
}

fn table_size(
    storage: &crate::runtime::definition::StorageDescriptorV1,
) -> Result<usize, CarrierStateSchemaError> {
    if storage.namespace == keyed::STORAGE_OWNER
        || storage.namespace.len() > MAX_DEFINITION_TEXT_BYTES
        || !StateKeyV1::valid_table_id(&storage.namespace)
    {
        return Err(invalid("invalid selected namespace"));
    }
    if let ProjectionV1::Semantic(name) = &storage.projection {
        if name.is_empty()
            || name.len() > MAX_DEFINITION_TEXT_BYTES
            || name.starts_with(' ')
            || name.ends_with(' ')
            || !name.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
        {
            return Err(invalid("invalid semantic projection identity"));
        }
    }
    let key = match (&storage.shape, &storage.value) {
        (
            StorageShapeV1::Table {
                key: key @ SchemaV1::Codec(_),
            },
            SchemaV1::Codec(_),
        ) if storage.initial.is_none() => codec_size(key)?,
        (StorageShapeV1::Cell, SchemaV1::Codec(_))
            if storage
                .initial
                .as_ref()
                .is_none_or(|bytes| !bytes.is_empty() && bytes.len() <= MAX_INITIAL_BYTES) =>
        {
            0
        }
        (
            StorageShapeV1::Table {
                key: key @ SchemaV1::Carrier(_),
            },
            SchemaV1::Carrier(_),
        ) if storage.initial.is_none() && storage.projection == ProjectionV1::BareNorito => {
            codec_size(key)?
        }
        _ => return Err(invalid("unsupported carrier table shape or projection")),
    };
    storage
        .namespace
        .len()
        .checked_add(key)
        .and_then(|size| size.checked_add(codec_size(&storage.value).ok()?))
        .ok_or_else(|| invalid("invalid or unrepresentable carrier schema storage"))
}

fn reserve<T>(count: usize) -> Result<Vec<T>, CarrierStateSchemaError> {
    norito::core::classify_decode_attempt(|| {
        let bytes = count
            .checked_mul(std::mem::size_of::<T>())
            .ok_or(norito::Error::LengthMismatch)?;
        norito::core::reserve_decode_allocation(bytes)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| norito::Error::AllocationFailed {
                bytes: bytes as u64,
            })?;
        Ok(values)
    })
    .map_err(CarrierStateSchemaError::Codec)
}

fn copied(text: &str) -> Result<String, CarrierStateSchemaError> {
    let mut bytes = reserve(text.len())?;
    bytes.extend_from_slice(text.as_bytes());
    Ok(String::from_utf8(bytes).expect("copied valid UTF-8"))
}

fn carrier_codec(schema: &CarrierSchemaV1) -> Result<String, CarrierStateSchemaError> {
    let digest = carrier_schema_hash_v1(schema).map_err(CarrierStateSchemaError::Codec)?;
    let mut bytes = reserve(CARRIER_CODEC_PREFIX.len() + 64)?;
    bytes.extend_from_slice(CARRIER_CODEC_PREFIX.as_bytes());
    for byte in digest {
        bytes.push(b"0123456789abcdef"[(byte >> 4) as usize]);
        bytes.push(b"0123456789abcdef"[(byte & 15) as usize]);
    }
    Ok(String::from_utf8(bytes).expect("ASCII codec identity"))
}

impl SelectedStateSchemaV1 {
    /// Measure exact retained Vec/String request bytes and the maximum transient schema frame.
    /// No graph or output buffer is allocated. A live caller reserves these amounts from its
    /// original pool before construction and retains the first until the directory is dropped.
    ///
    /// # Errors
    /// Wrong profile/backing, noncanonical or oversized storage list, missing owners, or invalid
    /// storage shapes/text. Identity and carrier schema validation occur during construction.
    pub fn carrier_selection_allocation_bytes(
        selection: &RootSelectionV1,
        definitions: &impl DefinitionLookup,
    ) -> Result<(usize, usize), CarrierStateSchemaError> {
        if selection.commitment_profile != keyed::profile_id()
            || selection.installed_storage.is_empty()
            || selection.installed_storage.len() > MAX_DEFINITION_REFERENCES
            || selection
                .installed_storage
                .windows(2)
                .any(|ids| ids[0] >= ids[1])
            || selection
                .installed_storage
                .binary_search(&keyed::backing_id())
                .is_err()
        {
            return Err(invalid(
                "expected carrier keyed profile, physical owner and canonical bounded storage list",
            ));
        }
        let mut bytes = (selection.installed_storage.len() - 1)
            .checked_mul(std::mem::size_of::<TableDescriptor>())
            .ok_or_else(|| invalid("unrepresentable directory size"))?;
        let mut scratch = 0;
        for id in &selection.installed_storage {
            let Some(DefinitionDescriptorV1::Storage(storage)) = definitions.definition(id) else {
                return Err(invalid("missing selected storage"));
            };
            if *id == keyed::backing_id() {
                continue;
            }
            bytes = bytes
                .checked_add(table_size(storage)?)
                .ok_or_else(|| invalid("unrepresentable directory size"))?;
            if matches!(&storage.value, SchemaV1::Carrier(_)) {
                scratch = MAX_CARRIER_SCHEMA_FRAME_BYTES;
            }
        }
        Ok((bytes, scratch))
    }

    /// Resolve the exact new carrier-aware profile; native profile semantics stay unchanged.
    /// Every output allocation is fallible and charged to active codec scopes. Physical custody
    /// is the integrating caller's responsibility using [`Self::carrier_selection_allocation_bytes`].
    ///
    /// # Errors
    /// Invalid selection, substituted owner, unsupported shape, duplicate namespace, malformed
    /// carrier schema, or an unchanged original codec/allocator refusal.
    pub fn from_carrier_selection(
        selection: &RootSelectionV1,
        definitions: &impl DefinitionLookup,
    ) -> Result<Self, CarrierStateSchemaError> {
        Self::carrier_selection_allocation_bytes(selection, definitions)?;
        let mut tables = reserve(selection.installed_storage.len() - 1)?;
        for id in &selection.installed_storage {
            let definition = definitions
                .definition(id)
                .ok_or_else(|| invalid("selected storage disappeared"))?;
            if carrier_definition_id_v1(definition).map_err(CarrierStateSchemaError::Codec)? != *id
            {
                return Err(invalid("substituted selected storage"));
            }
            let DefinitionDescriptorV1::Storage(storage) = definition else {
                return Err(invalid("selected owner is not storage"));
            };
            if *id == keyed::backing_id() {
                continue;
            }
            let value = match &storage.value {
                SchemaV1::Codec(name) => copied(name)?,
                SchemaV1::Carrier(schema) => carrier_codec(schema)?,
            };
            let table = match &storage.shape {
                StorageShapeV1::Table { key } => {
                    let key = match key {
                        SchemaV1::Codec(name) => copied(name)?,
                        SchemaV1::Carrier(schema) => carrier_codec(schema)?,
                    };
                    TableDescriptor::table(copied(&storage.namespace)?, key, value)
                }
                StorageShapeV1::Cell => TableDescriptor::cell(copied(&storage.namespace)?, value),
            };
            tables.push(table);
        }
        tables.sort_unstable_by(|a, b| a.id.as_bytes().cmp(b.id.as_bytes()));
        if tables.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(invalid("duplicate selected namespace"));
        }
        let directory = StateTableDirectoryV1 { tables };
        let mut hash =
            blake3::Hasher::new_derive_key("iroha carrier keyed State profile and schema v1");
        hash.update(&selection.commitment_profile.0);
        hash.update(&(selection.installed_storage.len() as u64).to_le_bytes());
        for id in &selection.installed_storage {
            hash.update(&id.0);
        }
        // Exact canonical directory framing, streamed without another output allocation.
        hash.update(SCHEMA_DOMAIN);
        hash.update(&(directory.tables.len() as u32).to_le_bytes());
        for table in &directory.tables {
            hash.update(&[table.shape.tag()]);
            for text in [&table.id, &table.key_schema, &table.value_schema] {
                hash.update(&(text.len() as u32).to_le_bytes());
                hash.update(text.as_bytes());
            }
        }
        Ok(Self {
            digest: *hash.finalize().as_bytes(),
            directory,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::definition::{CarrierNodeV1, StorageDescriptorV1};

    #[test]
    fn virtual_carrier_schema_excludes_only_its_exact_backing_and_binds_every_owner() {
        let (mut selection, mut definitions) = super::super::tests::fixture();
        let schema = SchemaV1::Carrier(CarrierSchemaV1 {
            nodes: vec![CarrierNodeV1::U16],
        });
        let definition = DefinitionDescriptorV1::Storage(StorageDescriptorV1 {
            namespace: "app.carrier.rows".into(),
            shape: StorageShapeV1::Table {
                key: schema.clone(),
            },
            value: schema,
            initial: None,
            invariants: [7; 32],
            projection: ProjectionV1::BareNorito,
        });
        let id = definition.id().unwrap();
        definitions.insert(id, definition.clone());
        definitions.insert(keyed::backing_id(), keyed::backing_descriptor());
        selection
            .installed_storage
            .extend([id, keyed::backing_id()]);
        selection.installed_storage.sort();
        assert!(SelectedStateSchemaV1::from_selection(&selection, &definitions).is_err());
        selection.commitment_profile = keyed::profile_id();
        let selected =
            SelectedStateSchemaV1::from_carrier_selection(&selection, &definitions).unwrap();
        let (retained, scratch) =
            SelectedStateSchemaV1::carrier_selection_allocation_bytes(&selection, &definitions)
                .unwrap();
        assert_eq!(
            retained,
            std::mem::size_of_val(selected.tables())
                + selected
                    .tables()
                    .iter()
                    .map(|table| table.id().len()
                        + table.key_schema().len()
                        + table.value_schema().len())
                    .sum::<usize>()
        );
        assert_eq!(scratch, MAX_CARRIER_SCHEMA_FRAME_BYTES);
        let zero = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX);
        let refusal = norito::with_decode_limits_scope(zero, || {
            SelectedStateSchemaV1::from_carrier_selection(&selection, &definitions)
        })
        .unwrap_err();
        assert!(
            matches!(refusal, CarrierStateSchemaError::Codec(original) if original.kind() == norito::core::DecodeAttemptErrorKind::EnclosingLimit)
        );
        assert!(selected.table(keyed::STORAGE_OWNER).is_none());
        assert!(
            selected
                .table("app.carrier.rows")
                .unwrap()
                .1
                .value_schema()
                .starts_with("carrier:compact-v1:")
        );
        assert_eq!(
            selected.tables().len() + 1,
            selection.installed_storage.len()
        );
        let mut changed = definition;
        let DefinitionDescriptorV1::Storage(storage) = &mut changed else {
            unreachable!()
        };
        storage.invariants[0] ^= 1;
        let changed_id = changed.id().unwrap();
        definitions.insert(changed_id, changed);
        *selection
            .installed_storage
            .iter_mut()
            .find(|installed| **installed == id)
            .unwrap() = changed_id;
        selection.installed_storage.sort();
        assert_ne!(
            selected.digest(),
            SelectedStateSchemaV1::from_carrier_selection(&selection, &definitions)
                .unwrap()
                .digest()
        );
        selection
            .installed_storage
            .retain(|id| *id != keyed::backing_id());
        assert!(SelectedStateSchemaV1::from_carrier_selection(&selection, &definitions).is_err());
    }
}
