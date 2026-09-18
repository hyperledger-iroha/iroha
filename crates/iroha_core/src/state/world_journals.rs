//! Capture every original World journal without retaining its writer or an EBR reader.
//!
//! The existing overlay inventory generates exhaustive destructuring and typed
//! target accessors. Private generic wrappers retain their real MV values; the
//! flat vector erases only the heterogeneous field type, never its owner or
//! publication identity. It is not a World read view or publication capability.

use core::convert::Infallible;

use super::{
    Cell, CellBlock, Storage, StorageBlock, TriggerSet, TriggerSetBlock, World, WorldBlock,
};
use crate::smartcontracts::isi::triggers::set::{DetachError, DetachedSet};
use iroha_data_model::{events::EventBox, nexus::DataSpaceCatalog};
use mv::{BlockMode, Key, Value};

/// Refusal drops the entire original overlay without publishing any component.
#[derive(Debug, thiserror::Error)]
pub(in crate::state) enum CaptureError<E> {
    /// One original journal has a different acquisition mode.
    #[error("World journal {field} mode {actual:?} differs from {expected:?}")]
    InconsistentMode {
        /// Original field, or the trigger owner's internal component.
        field: &'static str,
        /// Common mode required by the original World block.
        expected: BlockMode,
        /// Mode actually retained by the mismatching component.
        actual: BlockMode,
    },
    /// Caller admission refused retention before any journal values were copied.
    #[error("World journal retention admission failed")]
    Admission(E),
}

fn widen_error<E>(error: CaptureError<Infallible>) -> CaptureError<E> {
    match error {
        CaptureError::InconsistentMode {
            field,
            expected,
            actual,
        } => CaptureError::InconsistentMode {
            field,
            expected,
            actual,
        },
        CaptureError::Admission(impossible) => match impossible {},
    }
}

fn trigger_error(error: DetachError<Infallible>) -> CaptureError<Infallible> {
    match error {
        DetachError::InconsistentMode {
            field,
            expected,
            actual,
        } => CaptureError::InconsistentMode {
            field,
            expected,
            actual,
        },
        DetachError::Admission(impossible) => match impossible {},
    }
}

/// Immutable diagnostics of one retained field, not permission to publish it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::state) struct FieldSummary {
    /// Name from the sole World overlay inventory.
    pub name: &'static str,
    /// Acquisition mode of this field's original block.
    pub mode: BlockMode,
    /// Number of retained touches, including equal-value touches and deletions.
    pub touched_values: usize,
}

/// Actual original World deltas and extras, with no World reference or lifetime.
///
/// The admission must cover the flat vector, one wrapper per inventory field,
/// touched final-value copies, overlap and future installation resources. No
/// serialized-length estimate or default resource policy is supplied here.
/// Rust drops the admission last, after every retained payload and event.
///
/// TODO: compose these original journals with all other State owners, admitted
/// resources and exact finality in one consuming publisher. This owner cannot
/// reattach, publish, reconstruct a World, or authorize a decided-block refusal.
pub(in crate::state) struct DetachedWorld<Admission> {
    mode: BlockMode,
    fields: Vec<Box<dyn RetainedWorldField>>,
    dataspace_catalog: DataSpaceCatalog,
    external_event_buf: Vec<EventBox>,
    admission: Admission,
}

impl<Admission> DetachedWorld<Admission> {
    /// Common mode of every original World and trigger component.
    pub(in crate::state) fn mode(&self) -> BlockMode {
        self.mode
    }

    /// Number of captured names in the single World overlay inventory.
    pub(in crate::state) fn field_count(&self) -> usize {
        self.fields.len()
    }

    /// Inspect field names, acquisition modes and exact retained touch counts.
    pub(in crate::state) fn fields(&self) -> impl ExactSizeIterator<Item = FieldSummary> + '_ {
        self.fields.iter().map(|field| field.summary())
    }

    /// Inspect one named field without downcasting or exposing a mutable journal.
    pub(in crate::state) fn field(&self, name: &str) -> Option<FieldSummary> {
        self.fields().find(|field| field.name == name)
    }

    /// Borrow the exact moved block-local alias context.
    pub(in crate::state) fn dataspace_catalog(&self) -> &DataSpaceCatalog {
        &self.dataspace_catalog
    }

    /// Borrow all moved external events in their original order.
    pub(in crate::state) fn external_events(&self) -> &[EventBox] {
        &self.external_event_buf
    }

    /// Borrow the retained resource reservation.
    pub(in crate::state) fn admission(&self) -> &Admission {
        &self.admission
    }

    /// Observe all captured owner/current/undo identities against an explicit target.
    ///
    /// These separate observations are advisory, not an atomic World snapshot or
    /// an exclusive publication lease. An eventual aggregate publisher must
    /// reacquire all writers and check the exact original cut before publishing.
    pub(in crate::state) fn matches_current(&self, target: &World) -> bool {
        self.fields
            .iter()
            .all(|field| field.matches_current(target))
    }
}

// Private and non-extensible outside this module. There is no Any/downcast,
// name-based dispatch, mutable access, live read view or per-field publisher.
trait RetainedWorldField: Send + Sync {
    fn summary(&self) -> FieldSummary;
    fn matches_current(&self, target: &World) -> bool;
}

trait CaptureWorldField: Sized {
    type Target;
    type Retained: RetainedWorldField + 'static;

    fn capture_mode(&self) -> Result<BlockMode, CaptureError<Infallible>>;
    fn capture(
        self,
        name: &'static str,
        target: fn(&World) -> &Self::Target,
    ) -> Result<Self::Retained, CaptureError<Infallible>>;
}

struct RetainedStorage<K: Key, V: Value> {
    name: &'static str,
    journal: mv::storage::Detached<K, V, ()>,
    target: fn(&World) -> &Storage<K, V>,
}

impl<K: Key, V: Value> RetainedWorldField for RetainedStorage<K, V> {
    fn summary(&self) -> FieldSummary {
        FieldSummary {
            name: self.name,
            mode: self.journal.mode(),
            touched_values: self.journal.touched_entries().len(),
        }
    }
    fn matches_current(&self, target: &World) -> bool {
        self.journal.matches_current((self.target)(target))
    }
}

impl<K: Key, V: Value> CaptureWorldField for StorageBlock<'_, K, V> {
    type Target = Storage<K, V>;
    type Retained = RetainedStorage<K, V>;
    fn capture_mode(&self) -> Result<BlockMode, CaptureError<Infallible>> {
        Ok(self.mode())
    }
    fn capture(
        self,
        name: &'static str,
        target: fn(&World) -> &Self::Target,
    ) -> Result<Self::Retained, CaptureError<Infallible>> {
        let journal = match self.try_detach(|_| Ok::<(), Infallible>(())) {
            Ok(journal) => journal,
            Err(impossible) => match impossible {},
        };
        Ok(RetainedStorage {
            name,
            journal,
            target,
        })
    }
}

struct RetainedCell<V: Value> {
    name: &'static str,
    journal: mv::cell::Detached<V, ()>,
    target: fn(&World) -> &Cell<V>,
}

impl<V: Value> RetainedWorldField for RetainedCell<V> {
    fn summary(&self) -> FieldSummary {
        FieldSummary {
            name: self.name,
            mode: self.journal.mode(),
            touched_values: usize::from(self.journal.touched_value().is_some()),
        }
    }
    fn matches_current(&self, target: &World) -> bool {
        self.journal.matches_current((self.target)(target))
    }
}

impl<V: Value> CaptureWorldField for CellBlock<'_, V> {
    type Target = Cell<V>;
    type Retained = RetainedCell<V>;
    fn capture_mode(&self) -> Result<BlockMode, CaptureError<Infallible>> {
        Ok(self.mode())
    }
    fn capture(
        self,
        name: &'static str,
        target: fn(&World) -> &Self::Target,
    ) -> Result<Self::Retained, CaptureError<Infallible>> {
        let journal = match self.try_detach(|_| Ok::<(), Infallible>(())) {
            Ok(journal) => journal,
            Err(impossible) => match impossible {},
        };
        Ok(RetainedCell {
            name,
            journal,
            target,
        })
    }
}

struct RetainedTriggers {
    name: &'static str,
    journal: DetachedSet<()>,
    target: fn(&World) -> &TriggerSet,
}

impl RetainedWorldField for RetainedTriggers {
    fn summary(&self) -> FieldSummary {
        // The trigger owner exposes these ten immutable typed journals. Its
        // own exhaustive SetBlock destructuring remains the completeness gate.
        let touched_values = self.journal.data_triggers().touched_entries().len()
            + self.journal.pipeline_triggers().touched_entries().len()
            + self.journal.time_triggers().touched_entries().len()
            + self.journal.by_call_triggers().touched_entries().len()
            + self.journal.ids().touched_entries().len()
            + self
                .journal
                .active_data_trigger_ids()
                .touched_entries()
                .len()
            + self
                .journal
                .active_pipeline_trigger_ids()
                .touched_entries()
                .len()
            + self
                .journal
                .active_time_trigger_ids()
                .touched_entries()
                .len()
            + self
                .journal
                .active_by_call_trigger_ids()
                .touched_entries()
                .len()
            + self.journal.contracts().touched_entries().len();
        FieldSummary {
            name: self.name,
            mode: self.journal.mode(),
            touched_values,
        }
    }
    fn matches_current(&self, target: &World) -> bool {
        self.journal.matches_current((self.target)(target))
    }
}

impl CaptureWorldField for TriggerSetBlock<'_> {
    type Target = TriggerSet;
    type Retained = RetainedTriggers;
    fn capture_mode(&self) -> Result<BlockMode, CaptureError<Infallible>> {
        TriggerSetBlock::capture_mode(self).map_err(trigger_error)
    }
    fn capture(
        self,
        name: &'static str,
        target: fn(&World) -> &Self::Target,
    ) -> Result<Self::Retained, CaptureError<Infallible>> {
        let journal = self
            .try_detach(|_| Ok::<(), Infallible>(()))
            .map_err(trigger_error)?;
        Ok(RetainedTriggers {
            name,
            journal,
            target,
        })
    }
}

macro_rules! capture_world_fields {
    ($original:ident, $admit:ident;
        [$($prefix:ident,)*] [$($privacy:ident,)*] [$($suffix:ident,)*]) => {{
        let mode = $original.parameters.mode();
        $(check_mode!($original, mode, $prefix);)*
        $(check_mode!($original, mode, $privacy);)*
        $(check_mode!($original, mode, $suffix);)*
        // No wrapper/vector/delta allocation or value copy precedes this call.
        let admission = $admit(&$original).map_err(CaptureError::Admission)?;
        let WorldBlock {
            dataspace_catalog,
            $($prefix,)* $($privacy,)* $($suffix,)*
            external_event_buf,
        } = $original;
        const FIELD_COUNT: usize = [
            $(stringify!($prefix),)* $(stringify!($privacy),)* $(stringify!($suffix),)*
        ].len();
        let mut fields: Vec<Box<dyn RetainedWorldField>> = Vec::with_capacity(FIELD_COUNT);
        $(retain_field!(fields, $prefix);)*
        $(retain_field!(fields, $privacy);)*
        $(retain_field!(fields, $suffix);)*
        Ok(DetachedWorld { mode, fields, dataspace_catalog, external_event_buf, admission })
    }};
}

macro_rules! check_mode {
    ($original:ident, $expected:ident, $field:ident) => {
        let actual = CaptureWorldField::capture_mode(&$original.$field).map_err(widen_error)?;
        if actual != $expected {
            return Err(CaptureError::InconsistentMode {
                field: stringify!($field),
                expected: $expected,
                actual,
            });
        }
    };
}

macro_rules! retain_field {
    ($fields:ident, $field:ident) => {
        $fields.push(Box::new(
            CaptureWorldField::capture($field, stringify!($field), |target: &World| &target.$field)
                .map_err(widen_error)?,
        ));
    };
}

impl WorldBlock<'_> {
    /// Admit and capture every original journal, then release all concrete writers.
    ///
    /// Mode checks inspect the actual original owners before the callback. The
    /// callback admits all retained allocation and installation resources once;
    /// it sees the complete immutable overlay, including block-local extras.
    /// Refusal drops all original writers without publication. Success moves
    /// extras and MV preimages and copies only touched final MV values.
    pub(in crate::state) fn try_detach_journals<Admission, E>(
        self,
        admit: impl FnOnce(&Self) -> Result<Admission, E>,
    ) -> Result<DetachedWorld<Admission>, CaptureError<E>> {
        with_world_overlay_fields!(capture_world_fields, self, admit)
    }
}

#[cfg(test)]
#[path = "world_journals_tests.rs"]
mod tests;
