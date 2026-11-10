//! Registry and codec bindings used by portable keyed State witnesses.

use super::{FinalityError, keyed_state::StateKeyV1};
use crate::runtime::{
    definition::{
        DefinitionDescriptorV1, DefinitionLookup, RootSelectionV1, SchemaV1, StorageShapeV1,
    },
    keyed_state::profile_id,
};

/// Domain tag of the canonical ordered State schema descriptor.
pub const SCHEMA_DOMAIN: &[u8] = b"iroha:state-keyed-commitment:schema:v1\0";
/// The one canonical key of a singleton cell.
pub const CELL_KEY: &[u8] = &[];

/// Whether a committed identity is a keyed table or a singleton cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TableShape {
    /// Zero or more entries under canonical key bytes.
    Table,
    /// At most one entry, at [`CELL_KEY`].
    Cell,
}

impl TableShape {
    /// Stable tag in the canonical schema descriptor.
    const fn tag(self) -> u8 {
        match self {
            Self::Table => 0,
            Self::Cell => 1,
        }
    }
}

/// One committed table or cell: its registry identity and codec identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableDescriptor {
    id: String,
    shape: TableShape,
    key_schema: String,
    value_schema: String,
}

impl TableDescriptor {
    /// Describe a keyed table with its key and value codec identities.
    pub fn table(
        id: impl Into<String>,
        key_schema: impl Into<String>,
        value_schema: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            shape: TableShape::Table,
            key_schema: key_schema.into(),
            value_schema: value_schema.into(),
        }
    }

    /// Describe a singleton cell with its value codec identity.
    pub fn cell(id: impl Into<String>, value_schema: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            shape: TableShape::Cell,
            key_schema: String::new(),
            value_schema: value_schema.into(),
        }
    }

    /// Stable registry identity, for example `world.accounts`.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Table or cell.
    pub fn shape(&self) -> TableShape {
        self.shape
    }

    /// Codec identity of the key; empty for a cell.
    pub fn key_schema(&self) -> &str {
        &self.key_schema
    }

    /// Codec identity of the value.
    pub fn value_schema(&self) -> &str {
        &self.value_schema
    }
}

/// A schema that cannot be committed.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SchemaError {
    /// A table identity is empty.
    #[error("keyed State schema has an empty table identity")]
    EmptyIdentity,
    /// Two descriptors share one identity.
    #[error("keyed State schema declares {0} twice")]
    DuplicateTable(String),
    /// A table has no key codec, or a cell declares one.
    #[error("keyed State schema has an inconsistent key codec for {0}")]
    KeyCodec(String),
    /// A value codec identity is empty.
    #[error("keyed State schema has no value codec for {0}")]
    ValueCodec(String),
    /// An identity or the table count does not fit the descriptor's `u32` lengths.
    #[error("keyed State schema exceeds the descriptor length bound")]
    Length,
    /// The registry still declares an unresolved projection for this field.
    #[error("State authority field has no complete V1 schema: {0}")]
    Unresolved(&'static str),
}

/// The complete ordered set of committed tables and cells (rule K1).
///
/// Tables are held in the canonical order: ascending bytes of their identities.
/// That order is the table order of every root, range and result layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateTableDirectoryV1 {
    tables: Vec<TableDescriptor>,
}

impl StateTableDirectoryV1 {
    /// Validate descriptors and place them in the canonical identity order.
    ///
    /// # Errors
    /// An identity is empty or repeated, a codec identity is missing or misplaced, or a
    /// length exceeds `u32`.
    pub fn new(mut tables: Vec<TableDescriptor>) -> Result<Self, SchemaError> {
        if u32::try_from(tables.len()).is_err() {
            return Err(SchemaError::Length);
        }
        for table in &tables {
            if table.id.is_empty() {
                return Err(SchemaError::EmptyIdentity);
            }
            let key_declared = !table.key_schema.is_empty();
            if key_declared != (table.shape == TableShape::Table) {
                return Err(SchemaError::KeyCodec(table.id.clone()));
            }
            if table.value_schema.is_empty() {
                return Err(SchemaError::ValueCodec(table.id.clone()));
            }
            for text in [&table.id, &table.key_schema, &table.value_schema] {
                if u32::try_from(text.len()).is_err() {
                    return Err(SchemaError::Length);
                }
            }
        }
        tables.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));
        if let Some(pair) = tables.windows(2).find(|pair| pair[0].id == pair[1].id) {
            return Err(SchemaError::DuplicateTable(pair[0].id.clone()));
        }
        Ok(Self { tables })
    }

    /// The committed tables and cells in canonical order.
    pub fn tables(&self) -> &[TableDescriptor] {
        &self.tables
    }

    /// Canonical position and descriptor of a declared identity.
    pub fn table(&self, id: &str) -> Option<(usize, &TableDescriptor)> {
        self.tables
            .binary_search_by(|table| table.id.as_bytes().cmp(id.as_bytes()))
            .ok()
            .map(|index| (index, &self.tables[index]))
    }

    /// The canonical descriptor bytes that every construction binds into its root (K1):
    ///
    /// ```text
    /// SCHEMA_DOMAIN ‖ le32(count) ‖ for each table in canonical order:
    ///   shape ‖ le32(len(id)) ‖ id ‖ le32(len(key)) ‖ key ‖ le32(len(value)) ‖ value
    /// ```
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = SCHEMA_DOMAIN.to_vec();
        push_len(&mut bytes, self.tables.len());
        for table in &self.tables {
            bytes.push(table.shape.tag());
            for text in [&table.id, &table.key_schema, &table.value_schema] {
                push_len(&mut bytes, text.len());
                bytes.extend_from_slice(text.as_bytes());
            }
        }
        bytes
    }

    /// Check one `(table, key)` position: the table is declared and a cell uses [`CELL_KEY`].
    ///
    /// # Errors
    /// See [`StatementError`].
    pub fn check_key(&self, table: &str, key: &[u8]) -> Result<usize, StatementError> {
        let (index, descriptor) = self.table(table).ok_or(StatementError::UnknownTable)?;
        if descriptor.shape == TableShape::Cell && key != CELL_KEY {
            return Err(StatementError::CellKey);
        }
        Ok(index)
    }
}

fn push_len(bytes: &mut Vec<u8>, length: usize) {
    // `StateTableDirectoryV1::new` bounds every length by `u32`.
    let length = u32::try_from(length).expect("schema lengths were validated");
    bytes.extend_from_slice(&length.to_le_bytes());
}

/// A statement that is ill-formed before any commitment is consulted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum StatementError {
    /// The table identity is not declared by the schema.
    #[error("the keyed State schema does not declare this table")]
    UnknownTable,
    /// A cell was addressed with a key other than the empty key.
    #[error("a cell has only the empty key")]
    CellKey,
    /// The interval's upper bound does not exceed its lower bound.
    #[error("the key interval is empty")]
    EmptyRange,
    /// A claimed range entry lies outside the interval.
    #[error("a claimed entry lies outside the key interval")]
    EntryOutsideRange,
    /// Claimed range entries are not strictly ascending by key bytes.
    #[error("claimed entries are not strictly ascending")]
    EntryOrder,
}

/// Native codec and table-shape authority derived from an independently selected keyed profile.
/// Initialization can occur once per immutable schema; lookup and verification read no ledger.
#[derive(Clone, Debug)]
pub struct SelectedStateSchemaV1 {
    directory: StateTableDirectoryV1,
    digest: [u8; 32],
}
impl SelectedStateSchemaV1 {
    /// Resolve the actual content-addressed storage definitions installed by a trusted selection.
    /// The caller authenticates the selection or independently pins the resulting digest.
    ///
    /// # Errors
    /// Rejects a different commitment profile, missing, substituted, duplicate or malformed
    /// storage definitions, and layouts without a known canonical native codec.
    pub fn from_selection(
        selection: &RootSelectionV1,
        definitions: &impl DefinitionLookup,
    ) -> Result<Self, FinalityError> {
        if selection.commitment_profile != profile_id()
            || selection
                .installed_storage
                .windows(2)
                .any(|ids| ids[0] >= ids[1])
        {
            return Err(FinalityError(
                "expected the keyed commitment and a canonical installed storage list".into(),
            ));
        }
        let mut tables = Vec::with_capacity(selection.installed_storage.len());
        for id in &selection.installed_storage {
            let definition = definitions
                .definition(id)
                .ok_or_else(|| FinalityError(format!("missing selected storage {id}")))?;
            let DefinitionDescriptorV1::Storage(storage) = definition else {
                return Err(FinalityError(format!("selected {id} is not storage")));
            };
            if definition
                .id()
                .map_err(|error| FinalityError(error.to_string()))?
                != *id
                || !StateKeyV1::valid_table_id(&storage.namespace)
            {
                return Err(FinalityError(format!("invalid selected storage {id}")));
            }
            let codec = |encoding: &SchemaV1| match encoding {
                SchemaV1::Codec(name) => Ok(name.clone()),
                SchemaV1::Carrier(_) => Err(FinalityError(format!(
                    "no native State codec for {}",
                    storage.namespace
                ))),
            };
            let value = codec(&storage.value)?;
            tables.push(match &storage.shape {
                StorageShapeV1::Table { key } => {
                    TableDescriptor::table(&storage.namespace, codec(key)?, value)
                }
                StorageShapeV1::Cell => TableDescriptor::cell(&storage.namespace, value),
            });
        }
        let directory =
            StateTableDirectoryV1::new(tables).map_err(|e| FinalityError(e.to_string()))?;
        let mut hash = blake3::Hasher::new_derive_key("iroha keyed State profile and schema v1");
        hash.update(&selection.commitment_profile.0);
        hash.update(&directory.canonical_bytes());
        Ok(Self {
            digest: *hash.finalize().as_bytes(),
            directory,
        })
    }

    /// Resolve a canonical Norito deployment bundle consisting of the selected runtime and
    /// its exact installed storage descriptors, checked against an independent schema pin.
    ///
    /// # Errors
    /// Rejects oversized or malformed input, unbound or duplicate definitions, and incorrect pins.
    pub fn from_bundle(frame: &[u8], expected_digest: [u8; 32]) -> Result<Self, FinalityError> {
        let invalid = || FinalityError("invalid or unpinned keyed State schema bundle".into());
        if frame.is_empty() || frame.len() > 16 * 1024 * 1024 {
            return Err(invalid());
        }
        let (selection, descriptors): (RootSelectionV1, Vec<DefinitionDescriptorV1>) =
            norito::decode_canonical_for_admission(
                frame,
                norito::canonical_decode_limits(frame.len()),
            )
            .map_err(|_| invalid())?;
        if descriptors.len() != selection.installed_storage.len() {
            return Err(invalid());
        }
        let mut definitions = std::collections::BTreeMap::new();
        for descriptor in descriptors {
            let id = descriptor.id().map_err(|_| invalid())?;
            if definitions.insert(id, descriptor).is_some() {
                return Err(invalid());
            }
        }
        let schema = Self::from_selection(&selection, &definitions)?;
        if schema.digest() != expected_digest {
            return Err(invalid());
        }
        Ok(schema)
    }

    /// Independently pin this exact schema before accepting application interpretations.
    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
}
impl std::ops::Deref for SelectedStateSchemaV1 {
    type Target = StateTableDirectoryV1;
    fn deref(&self) -> &StateTableDirectoryV1 {
        &self.directory
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::runtime::definition::{DefinitionId, ProjectionV1, StorageDescriptorV1};
    use std::collections::BTreeMap;

    pub(in crate::sumeragi_finality) fn fixture() -> (
        RootSelectionV1,
        BTreeMap<DefinitionId, DefinitionDescriptorV1>,
    ) {
        let codec = SchemaV1::Codec("norito:u8@0.0.2".into());
        let definitions: BTreeMap<_, _> = [
            ("world.assets", StorageShapeV1::Table { key: codec.clone() }),
            ("world.epoch", StorageShapeV1::Cell),
        ]
        .into_iter()
        .map(|(name, shape)| {
            let descriptor = DefinitionDescriptorV1::Storage(StorageDescriptorV1 {
                namespace: name.into(),
                shape,
                value: codec.clone(),
                initial: None,
                invariants: [1; 32],
                projection: ProjectionV1::BareNorito,
            });
            (descriptor.id().unwrap(), descriptor)
        })
        .collect();
        (
            RootSelectionV1 {
                kernel: DefinitionId([1; 32]),
                commitment_profile: profile_id(),
                services_profile: DefinitionId([2; 32]),
                installed_storage: definitions.keys().copied().collect(),
                active: vec![],
                bindings: vec![],
                restrictions: vec![],
                completion_closures: vec![],
            },
            definitions,
        )
    }

    #[test]
    fn canonical_bundle_binds_the_pin_and_every_installed_storage_once() {
        let (selection, definitions) = fixture();
        let digest = SelectedStateSchemaV1::from_selection(&selection, &definitions)
            .unwrap()
            .digest();
        let descriptors: Vec<_> = definitions.into_values().collect();
        let pack = |rows| norito::encode_canonical(&(selection.clone(), rows)).unwrap();
        let frame = pack(descriptors.clone());
        assert_eq!(
            SelectedStateSchemaV1::from_bundle(&frame, digest)
                .unwrap()
                .digest(),
            digest
        );
        assert!(SelectedStateSchemaV1::from_bundle(&frame, [0xff; 32]).is_err());
        assert!(
            SelectedStateSchemaV1::from_bundle(&pack(vec![descriptors[0].clone()]), digest)
                .is_err()
        );
        assert!(
            SelectedStateSchemaV1::from_bundle(
                &pack(vec![descriptors[0].clone(), descriptors[0].clone()]),
                digest
            )
            .is_err()
        );
        assert!(SelectedStateSchemaV1::from_bundle(&[], digest).is_err());
    }

    #[test]
    fn trusted_storage_identity_and_table_shape_are_checked_before_absence() {
        let (selection, definitions) = fixture();
        let schema = SelectedStateSchemaV1::from_selection(&selection, &definitions).unwrap();
        assert!(schema.check_key("world.assets", &[3]).is_ok());
        assert!(schema.check_key("world.epoch", &[]).is_ok());
        assert!(schema.check_key("world.epoch", &[3]).is_err());
        assert!(schema.check_key("world.unknown", &[]).is_err());
        let mut wrong = definitions.clone();
        let ids: Vec<_> = wrong.keys().copied().collect();
        wrong.insert(ids[0], definitions[&ids[1]].clone());
        assert!(SelectedStateSchemaV1::from_selection(&selection, &wrong).is_err());
        let mut other = selection.clone();
        other.installed_storage.pop();
        assert_ne!(
            schema.digest(),
            SelectedStateSchemaV1::from_selection(&other, &definitions)
                .unwrap()
                .digest()
        );
        other.commitment_profile = DefinitionId([0; 32]);
        assert!(SelectedStateSchemaV1::from_selection(&other, &definitions).is_err());
    }
}
