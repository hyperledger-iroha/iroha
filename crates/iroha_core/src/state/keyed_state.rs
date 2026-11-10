//! Complete keyed State versions retained by the original execution/publication owner.
//! A decoded snapshot has no native execution version and cannot answer row proofs.

mod recovery;

use super::*;
use crate::{
    runtime,
    state::authority_registry::keyed_commitment::selected_schema::{
        SelectedSchemaCache, SelectedStateSchema,
    },
    sumeragi::certified_chain::CommittedBlock,
};
use iroha_data_model::sumeragi_finality::{
    KeyedStateRowV1, KeyedStateRowsV1, MAX_STATE_PROOF_ROWS_V1, MAX_STATE_ROWS_FRAME_BYTES_V1,
    keyed_state::{KeyedStateTreeV1, StateKeyV1},
};

/// The exact post-execution State and the post-publication State can differ.
/// The former serves reads. The latter is the next execution's predecessor.
#[derive(Clone)]
pub(super) struct KeyedCut {
    pub tip: NativeExecutionTip,
    pub execution: KeyedStateTreeV1,
    pub published: KeyedStateTreeV1,
    pub schema: Arc<SelectedStateSchema>,
}

/// The previous published version supplies a replacement block's parent. Persistent
/// nodes are shared; the retained interval is independent of the request path.
#[derive(Default)]
pub(super) struct KeyedVersions {
    current: Option<KeyedCut>,
    previous: Option<KeyedCut>,
}
impl KeyedVersions {
    pub(super) fn published_at(&self, tip: NativeExecutionTip) -> Option<&KeyedCut> {
        self.current
            .iter()
            .chain(&self.previous)
            .find(|cut| cut.tip == tip)
    }
    pub(super) fn install(&mut self, current: Option<KeyedCut>) -> Self {
        let mut retired = std::mem::take(self);
        if let Some(next) = current {
            self.previous = if retired
                .current
                .as_ref()
                .is_some_and(|cut| cut.tip.height().checked_add(1) == Some(next.tip.height()))
            {
                retired.current.take()
            } else if retired
                .previous
                .as_ref()
                .is_some_and(|cut| cut.tip.height().checked_add(1) == Some(next.tip.height()))
            {
                retired.previous.take()
            } else {
                None
            };
            self.current = Some(next);
        }
        retired
    }
}

pub(crate) fn schema_for(
    selection: &iroha_data_model::runtime::definition::RootSelectionV1,
) -> Result<Arc<SelectedStateSchema>, String> {
    static CACHE: SelectedSchemaCache = SelectedSchemaCache::new();
    if !runtime::keyed_state::selected(selection) {
        return Err("execution does not select the keyed State commitment".into());
    }
    let catalog = runtime::release_catalog().map_err(|e| e.to_string())?;
    if !catalog
        .keyed_storage
        .iter()
        .all(|id| selection.installed_storage.binary_search(id).is_ok())
    {
        return Err("keyed profile requires every State-level storage owner".into());
    }
    CACHE.get(catalog, selection)
}

/// A response retaining the producer's original memory reservation until encoding finishes.
pub struct FundedKeyedStateRows {
    rows: KeyedStateRowsV1,
    _reservations: Vec<iroha_allocation::AllocationReservation>,
}
impl std::ops::Deref for FundedKeyedStateRows {
    type Target = KeyedStateRowsV1;
    fn deref(&self) -> &Self::Target {
        &self.rows
    }
}

impl State {
    /// Return originals for the requested keys and witnesses against a single certified
    /// post-execution State. The caller owns authorization and retains the bounded working
    /// set through encoding. A missing native version cannot grant presence or absence.
    ///
    /// # Errors
    /// The proof is not available for this profile or height, the certified block differs
    /// from the original publication, the keys are malformed, or capacity is exhausted.
    pub fn keyed_state_rows(
        &self,
        tip: &CommittedBlock,
        selectors: &[(String, Vec<u8>)],
        budget: &iroha_allocation::AllocationBudget,
    ) -> Result<FundedKeyedStateRows, String> {
        use iroha_data_model::{
            block::consensus::SumeragiRootScope,
            sumeragi_finality::{MAX_STATE_PROOF_KEY_BYTES_V1, MAX_STATE_PROOF_TABLE_BYTES_V1},
        };
        if tip.height() < 2
            || tip.root_scope() != SumeragiRootScope::Global
            || selectors.is_empty()
            || selectors.len() > MAX_STATE_PROOF_ROWS_V1
            || selectors.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err("invalid certified State row selection".into());
        }
        let view = self.try_view_once().map_err(|error| error.to_string())?;
        let native = view
            .native_execution_tip()
            .ok_or("no native State publication")?;
        let certified_block_hash = tip.block_hash();
        if native.height() != tip.height()
            || native.iroha_hash() != certified_block_hash
            || native.core_hash() != tip.core_hash()
            || native.result() != tip.result()
            || view.latest_block_hash() != Some(certified_block_hash)
            || *view.network_id() != tip.commitment().schedule.current.network_id
            || !runtime::keyed_state::selected(view.world().runtime_selection())
        {
            return Err(
                "requested execution differs from the current keyed State publication".into(),
            );
        }
        let cut = self
            .keyed_versions
            .lock()
            .published_at(native)
            .cloned()
            .ok_or("no original keyed version for the certified execution")?;
        drop(view);
        if &cut.execution.root() != tip.commitment().execution.world_state_root.as_ref()
            || cut.execution.schema_digest() != cut.schema.digest
        {
            return Err("keyed version differs from the certified execution".into());
        }
        let headers = budget
            .try_reserve_bytes(
                selectors.len()
                    * (std::mem::size_of::<KeyedStateRowV1>()
                        + 2 * std::mem::size_of::<iroha_allocation::AllocationReservation>())
                    + std::mem::size_of::<iroha_allocation::AllocationReservation>(),
            )
            .map_err(|e| e.to_string())?;
        let mut rows = Vec::with_capacity(selectors.len());
        let mut retained = Vec::with_capacity(selectors.len() * 2 + 1);
        retained.push(headers);
        let mut bytes = 0usize;
        for (table, key) in selectors {
            if table.len() > MAX_STATE_PROOF_TABLE_BYTES_V1
                || key.len() > MAX_STATE_PROOF_KEY_BYTES_V1
            {
                return Err("State proof selector exceeds its bound".into());
            }
            cut.schema
                .schema
                .check_key(table, key)
                .map_err(|error| error.to_string())?;
            let key_charge = budget
                .try_reserve_bytes(table.len() + key.len())
                .map_err(|e| e.to_string())?;
            let position = StateKeyV1 {
                table: table.clone(),
                key: key.clone(),
            };
            let value = cut.execution.get(&position);
            let size = value
                .map_or(0, <[u8]>::len)
                .checked_add(
                    cut.execution
                        .proof_heap_bytes(&position)
                        .map_err(|e| e.to_string())?,
                )
                .ok_or("State response size overflow")?;
            bytes = bytes
                .checked_add(size)
                .ok_or("State response size overflow")?;
            if bytes > MAX_STATE_ROWS_FRAME_BYTES_V1 {
                return Err("State response exceeds its bound".into());
            }
            retained.push(key_charge);
            retained.push(budget.try_reserve_bytes(size).map_err(|e| e.to_string())?);
            let witness = cut
                .execution
                .prove(&position)
                .map_err(|error| error.to_string())?;
            rows.push(KeyedStateRowV1 {
                position,
                value: value.map(<[u8]>::to_vec),
                witness,
            });
        }
        Ok(FundedKeyedStateRows {
            rows: KeyedStateRowsV1 {
                height: tip.height(),
                block_hash: tip.block_hash(),
                schema_hash: cut.schema.digest,
                rows,
            },
            _reservations: retained,
        })
    }
}

use crate::state::authority_registry::original_stream::{
    CaptureError, KeyedCaptureError, capture_keyed_image,
};
use crate::state::world_projection::world_state_accumulator::world_state_cut::CutError;
use iroha_data_model::sumeragi_finality::{KeyedStateRoot, keyed_state::KeyedStateError};

/// Original sealed execution. This value stays with its overlay across local retry and is
/// published only alongside that overlay's native tip.
pub(super) struct ExecutionVersion {
    schema: Arc<SelectedStateSchema>,
    parent: KeyedStateRoot,
    tree: KeyedStateTreeV1,
}

impl ExecutionVersion {
    fn transition(
        &self,
        events: &[iroha_data_model::events::EventBox],
    ) -> Result<crate::sumeragi::commitment::WorldStateTransition, CutError> {
        Ok(crate::sumeragi::commitment::WorldStateTransition {
            parent_world_state_root: self.parent,
            world_state_root: self.tree.root().into(),
            event_commitment: crate::sumeragi::commitment::event_commitment(events)
                .map_err(|error| CutError::Invalid(error.to_string()))?,
        })
    }
}

fn capture_error(error: KeyedCaptureError) -> CutError {
    match error {
        KeyedCaptureError::Tree(KeyedStateError::Admission(original)) => original.into(),
        KeyedCaptureError::Tree(KeyedStateError::Buffer(original))
        | KeyedCaptureError::Source(CaptureError::Allocation(original)) => original.into(),
        KeyedCaptureError::Tree(KeyedStateError::Allocator(original)) => {
            CutError::Deferred(iroha_allocation::AllocatorRefusal::from(original).into())
        }
        KeyedCaptureError::Source(CaptureError::PayloadLimit | CaptureError::CountLimit) => {
            CutError::Deferred(ivm::error::ExecutionDeferral::ActiveMemoryCapacity.into())
        }
        other => CutError::Invalid(other.to_string()),
    }
}

impl StateBlockFields<'_> {
    fn keyed_predecessor(&self) -> Result<Option<KeyedCut>, CutError> {
        if self._curr_block.is_genesis()
            || !runtime::keyed_state::selected(self.world.runtime_selection.get_before_block())
        {
            return Ok(None);
        }
        let tip = self
            .native_execution_tip
            .get_before_block()
            .ok_or_else(|| CutError::Invalid("keyed predecessor has no native tip".into()))?;
        if Some(tip.iroha_hash()) != self._curr_block.prev_block_hash()
            || tip.height().checked_add(1) != Some(self._curr_block.height().get())
        {
            return Err(CutError::Invalid(
                "keyed predecessor does not match the extending block".into(),
            ));
        }
        let cut = self
            .state_ref
            .keyed_versions
            .lock()
            .published_at(tip)
            .cloned()
            .ok_or_else(|| {
                CutError::Invalid("keyed predecessor must be rebuilt by native replay".into())
            })?;
        Ok(Some(cut))
    }

    fn keyed_execution(&self) -> Result<ExecutionVersion, CutError> {
        let schema = schema_for(self.world.runtime_selection.get())?;
        let predecessor = self.keyed_predecessor()?;
        let parent = if let Some(cut) = &predecessor {
            cut.published.root().into()
        } else if self._curr_block.is_genesis()
            && runtime::keyed_state::selected(self.world.runtime_selection.get_before_block())
        {
            KeyedStateTreeV1::new(schema.digest, &self.state_ref.ivm_execution_budget())
                .root()
                .into()
        } else {
            self.world
                .state_accumulator
                .get_before_block()
                .root(self.world.runtime_selection.get_before_block())?
                .into()
        };
        let tree = capture_keyed_image(
            self,
            &schema,
            predecessor.as_ref().map(|cut| &cut.published),
        )
        .map_err(capture_error)?;
        Ok(ExecutionVersion {
            parent,
            tree,
            schema,
        })
    }

    pub(super) fn prepare_published_keyed_cut(
        &self,
        execution: &ExecutionVersion,
    ) -> Result<KeyedCut, CutError> {
        let tip = self
            .native_execution_tip
            .get()
            .ok_or_else(|| CutError::Invalid("keyed publication has no native tip".into()))?;
        if tip.height() != self._curr_block.height().get()
            || tip.iroha_hash() != self._curr_block.hash()
        {
            return Err(CutError::Invalid(
                "keyed publication differs from its execution header".into(),
            ));
        }
        let schema = schema_for(self.world.runtime_selection.get())?;
        if execution.schema.digest != schema.digest {
            return Err(CutError::Invalid(
                "keyed schema changed after execution was sealed".into(),
            ));
        }
        let predecessor = self.keyed_predecessor()?;
        let published = capture_keyed_image(
            self,
            &schema,
            predecessor.as_ref().map(|cut| &cut.published),
        )
        .map_err(capture_error)?;
        Ok(KeyedCut {
            tip,
            published,
            execution: execution.tree.clone(),
            schema,
        })
    }
}

impl StateBlock<'_> {
    pub(in crate::state) fn project_keyed_state_transition(
        &self,
    ) -> Result<crate::sumeragi::commitment::WorldStateTransition, CutError> {
        if let Some(cut) = &self.keyed_execution {
            return cut.transition(&self.world.external_event_buf);
        }
        self.fields
            .as_ref()
            .expect("original executing block")
            .keyed_execution()?
            .transition(&self.world.external_event_buf)
    }

    fn check_keyed_execution_policy(&self) -> Result<(), CutError> {
        let fields = self.fields.as_ref().expect("original executing State");
        if let Some(predecessor) = fields.keyed_predecessor()? {
            crate::state::authority_registry::original_stream::verify_keyed_execution_policy(
                fields,
                &predecessor.published,
            )
            .map_err(capture_error)?;
        }
        Ok(())
    }

    pub(super) fn admit_keyed_execution<E: std::fmt::Debug>(
        &self,
    ) -> Result<(), StateBlockStartError<E>> {
        self.check_keyed_execution_policy()
            .map_err(|error| match error {
                CutError::Deferred(reason) => StateBlockStartError::ExecutionDeferred(reason),
                CutError::Invalid(reason) => {
                    StateBlockStartError::Storage(StateStorageAdmissionError::UnsupportedRuntime {
                        height: self._curr_block.height().get(),
                        reason,
                    })
                }
            })
    }

    pub(super) fn admit_keyed_scratch(&self) -> Result<(), MergeLedgerCommitError> {
        self.check_keyed_execution_policy()
            .map_err(|error| match error {
                CutError::Deferred(reason) => MergeLedgerCommitError::ExecutionDeferred(reason),
                CutError::Invalid(reason) => {
                    MergeLedgerCommitError::ExecutionStatePublication(reason)
                }
            })
    }

    pub(crate) fn seal_state_transition(
        &mut self,
    ) -> Result<crate::sumeragi::commitment::WorldStateTransition, CutError> {
        if !runtime::keyed_state::selected(self.world.runtime_selection.get()) {
            return self.world_state_transition().map_err(CutError::Invalid);
        }
        if self.keyed_execution.is_none() {
            self.keyed_execution = Some(
                self.fields
                    .as_ref()
                    .expect("original executing block")
                    .keyed_execution()?,
            );
        }
        self.project_keyed_state_transition()
    }
}
