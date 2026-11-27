//! Resolve the authenticated table and codec identities of one governed root selection.

use std::sync::Arc;

use crate::runtime::ReleaseCatalog;
use iroha_data_model::runtime::definition::{
    DefinitionDescriptorV1, DefinitionId, DefinitionLookup, RootSelectionV1, SchemaV1,
};
use iroha_data_model::sumeragi_finality::{CarrierStateSchemaError, SelectedStateSchemaV1};

/// Resolved schema of a keyed root. The digest binds both the selected commitment profile
/// and the complete ordered descriptor, including codec identities at this height.
pub(crate) struct SelectedStateSchema {
    pub(crate) schema: SelectedStateSchemaV1,
    pub(crate) digest: [u8; 32],
    /// Present only for the new carrier profile; empty virtual tables still need one owner.
    pub(crate) carrier_tables: Option<usize>,
    // Drops after the actual directory allocations. Native schemas retain their existing cache.
    carrier_directory: Option<iroha_allocation::AllocationReservation>,
}

/// Native cache custody is unchanged; new dynamic schemas retain their exact original pool.
#[derive(Clone)]
pub(crate) enum SelectedSchemaHandle {
    Native(Arc<SelectedStateSchema>),
    Carrier(iroha_allocation::ChargedShared<SelectedStateSchema>),
}
impl std::ops::Deref for SelectedSchemaHandle {
    type Target = SelectedStateSchema;
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Native(schema) => schema,
            Self::Carrier(schema) => schema,
        }
    }
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
            carrier_tables: None,
            carrier_directory: None,
        })
    }

    /// Resolve dynamic carrier definitions from the actual authenticated World, while every
    /// native storage codec must still have its exact compiled original source owner.
    pub(crate) fn resolve_carrier(
        catalog: &ReleaseCatalog,
        selection: &RootSelectionV1,
        published: &impl DefinitionLookup,
    ) -> Result<Self, crate::execution_attempt::ExecutionAttemptError<String>> {
        use crate::execution_attempt::{ExecutionAttemptError, canonical_decode_attempt_error};
        let schema = SelectedStateSchemaV1::from_carrier_selection(selection, published).map_err(
            |error| match error {
                CarrierStateSchemaError::Codec(original) => {
                    canonical_decode_attempt_error(original, |error| error.to_string())
                }
                error => ExecutionAttemptError::Rejected(error.to_string()),
            },
        )?;
        let mut carrier_tables = 0;
        for id in &selection.installed_storage {
            let Some(definition @ DefinitionDescriptorV1::Storage(storage)) =
                published.definition(id)
            else {
                return Err(ExecutionAttemptError::Rejected(
                    "selected storage disappeared".into(),
                ));
            };
            if matches!(&storage.value, SchemaV1::Carrier(_)) {
                carrier_tables += 1;
            } else if catalog.definitions.get(id) != Some(definition)
                || !(catalog.world_storage.values().any(|owner| owner == id)
                    || catalog.keyed_storage.contains(id))
            {
                return Err(ExecutionAttemptError::Rejected(
                    "selected native storage has no exact compiled source owner".into(),
                ));
            }
        }
        Ok(Self {
            digest: schema.digest(),
            schema,
            carrier_tables: Some(carrier_tables),
            carrier_directory: None,
        })
    }

    /// Admit the complete dynamic directory and its actual shared control from the State's
    /// original pool before construction; its final clone refunds only after destruction.
    pub(crate) fn resolve_carrier_funded(
        catalog: &ReleaseCatalog,
        selection: &RootSelectionV1,
        published: &impl DefinitionLookup,
        budget: &iroha_allocation::AllocationBudget,
    ) -> Result<SelectedSchemaHandle, crate::execution_attempt::ExecutionAttemptError<String>> {
        use crate::execution_attempt::{ExecutionAttemptError, ExecutionDeferred};
        use iroha_allocation::{ChargedShared, PrepaidSharedError};
        let (retained, scratch) =
            SelectedStateSchemaV1::carrier_selection_allocation_bytes(selection, published)
                .map_err(|error| ExecutionAttemptError::Rejected(error.to_string()))?;
        let payload = retained.checked_add(scratch).ok_or_else(|| {
            ExecutionAttemptError::Rejected(
                "carrier directory allocation is unrepresentable".into(),
            )
        })?;
        let layout = std::alloc::Layout::array::<u8>(payload).map_err(|_| {
            ExecutionAttemptError::Rejected(
                "carrier directory allocation is unrepresentable".into(),
            )
        })?;
        let mut reservation = budget
            .try_reserve_layouts([layout, ChargedShared::<Self>::allocation_layout()])
            .map_err(|original| ExecutionAttemptError::Deferred(original.into()))?;
        let shell =
            ChargedShared::<Self>::reserve_from(&mut reservation).map_err(|error| match error {
                original @ PrepaidSharedError::Allocator { .. } => ExecutionAttemptError::Deferred(
                    iroha_allocation::AllocatorRefusal::try_from(original)
                        .expect("matched allocator refusal")
                        .into(),
                ),
                PrepaidSharedError::Reservation(_) => ExecutionAttemptError::Deferred(
                    ExecutionDeferred::from(ivm::error::ExecutionDeferral::LocalInvariantViolation),
                ),
            })?;
        let mut schema = Self::resolve_carrier(catalog, selection, published)?;
        schema.carrier_directory = Some(
            reservation
                .try_partition_bytes(retained)
                .expect("measured retained directory remainder"),
        );
        Ok(SelectedSchemaHandle::Carrier(shell.initialize(schema)))
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
