//! Resolve the authenticated table and codec identities of one governed root selection.

use std::sync::Arc;

use crate::runtime::ReleaseCatalog;
use iroha_data_model::runtime::definition::{DefinitionId, RootSelectionV1};
use iroha_data_model::sumeragi_finality::SelectedStateSchemaV1;

/// Resolved schema of a keyed root. The digest binds both the selected commitment profile
/// and the complete ordered descriptor, including codec identities at this height.
#[derive(Clone)]
pub(crate) struct SelectedStateSchema {
    pub(crate) schema: SelectedStateSchemaV1,
    pub(crate) digest: [u8; 32],
}

impl SelectedStateSchema {
    /// Only installed storage participates. Runtime extensions can replace a value codec
    /// while keeping the same table name. Always read that codec from the actual selection.
    pub(crate) fn resolve(
        catalog: &ReleaseCatalog,
        selection: &RootSelectionV1,
    ) -> Result<Self, String> {
        let schema = SelectedStateSchemaV1::from_selection(selection, &catalog.definitions)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            digest: schema.digest(),
            schema,
        })
    }
}

/// Cache only the last resolved schema. Restriction-only changes do not re-encode the
/// complete schema on the consensus hot path.
pub(crate) struct SelectedSchemaCache {
    last: parking_lot::Mutex<Option<(DefinitionId, Vec<DefinitionId>, Arc<SelectedStateSchema>)>>,
}

impl SelectedSchemaCache {
    pub(crate) const fn new() -> Self {
        Self {
            last: parking_lot::Mutex::new(None),
        }
    }
    pub(crate) fn get(
        &self,
        catalog: &ReleaseCatalog,
        selection: &RootSelectionV1,
    ) -> Result<Arc<SelectedStateSchema>, String> {
        let mut last = self.last.lock();
        if let Some((profile, storage, schema)) = last.as_ref()
            && *profile == selection.commitment_profile
            && storage == &selection.installed_storage
        {
            return Ok(Arc::clone(schema));
        }
        let schema = Arc::new(SelectedStateSchema::resolve(catalog, selection)?);
        *last = Some((
            selection.commitment_profile,
            selection.installed_storage.clone(),
            Arc::clone(&schema),
        ));
        Ok(schema)
    }
}
