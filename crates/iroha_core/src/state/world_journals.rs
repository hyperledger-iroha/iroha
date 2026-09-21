//! Capture every original World journal without retaining its writer or an EBR reader.
//!
//! The existing overlay inventory generates exhaustive destructuring and typed
//! target accessors. Private generic wrappers retain their real MV values; the
//! flat vector erases only the heterogeneous field type, never its owner or
//! publication identity. Consuming preparation reacquires every original writer;
//! final State authorization and visibility remain the aggregate owner's duty.

use core::convert::Infallible;

use super::{
    Cell, CellBlock, Storage, StorageBlock, TriggerSet, TriggerSetBlock, World, WorldBlock,
    WorldBlockFields,
};
use crate::smartcontracts::isi::triggers::set::{DetachError, DetachedSet, SetBlockCapture};
use iroha_data_model::{events::EventBox, nexus::DataSpaceCatalog};
use mv::{
    BlockCapture, BlockMode, Key, Value, cell::BlockCaptureSlot as CellCaptureSlot,
    storage::BlockCaptureSlot as StorageCaptureSlot,
};

#[path = "world_publication.rs"]
pub(in crate::state) mod publication;
#[path = "world_journal_resources.rs"]
pub(in crate::state) mod resources;

/// Refusal drops the entire original overlay without publishing any component.
#[derive(Debug, thiserror::Error)]
pub(crate) enum CaptureError<E> {
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
    /// Caller admission refused retention before detaching original journal values.
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
/// publication metadata, retained original current/undo allocations and future
/// installation resources. Detachment does not clone the payloads; their earlier
/// execution and nested allocations require admission before they are created.
/// No serialized-length estimate or default resource policy is supplied here.
/// Rust drops the admission last, after every retained payload and event.
///
/// TODO: compose these original journals with all other State owners, admitted
/// resources and exact finality in one consuming publisher. This owner cannot
/// reconstruct a World or authorize a decided-block refusal. Preparing World
/// writers alone does not authorize State publication.
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
// name-based dispatch, mutable access or live read view. Only the complete
// consuming World owner can acquire and publish its private field publishers.
trait RetainedWorldField: Send + Sync {
    fn summary(&self) -> FieldSummary;
    fn matches_current(&self, target: &World) -> bool;
    fn try_prepare<'target>(
        self: Box<Self>,
        target: &'target World,
    ) -> Result<
        Box<dyn publication::PreparedWorldField + 'target>,
        (
            Box<dyn publication::PreparedWorldField + 'target>,
            publication::FieldRefusal,
        ),
    >;
}

trait CaptureWorldField: Sized {
    type Target;
    type Retained: RetainedWorldField + 'static;
    type Capture: WorldCaptureSlot<Target = Self::Target, Retained = Self::Retained>;

    fn capture_mode(&self) -> Result<BlockMode, CaptureError<Infallible>>;
    fn into_capture(self) -> Self::Capture;
}

trait WorldCaptureSlot: Sized {
    type Target;
    type Retained: RetainedWorldField + 'static;

    fn capture(&mut self) -> Result<(), CaptureError<Infallible>>;
    fn release(&mut self);
    // Only after every field captured: all physical writers are already free.
    fn retain(self, name: &'static str, target: fn(&World) -> &Self::Target) -> Self::Retained;
}

struct RetainedStorage<K: Key, V: Value> {
    name: &'static str,
    // Empty only while the same box is held by its private prepared owner.
    journal: Option<mv::storage::Detached<K, V, ()>>,
    target: fn(&World) -> &Storage<K, V>,
}

impl<K: Key, V: Value> RetainedWorldField for RetainedStorage<K, V> {
    fn summary(&self) -> FieldSummary {
        let journal = self.journal.as_ref().expect("retained original journal");
        FieldSummary {
            name: self.name,
            mode: journal.mode(),
            touched_values: journal.touched_entries().len(),
        }
    }
    fn matches_current(&self, target: &World) -> bool {
        self.journal
            .as_ref()
            .expect("retained original journal")
            .matches_current((self.target)(target))
    }
    fn try_prepare<'target>(
        self: Box<Self>,
        target: &'target World,
    ) -> Result<
        Box<dyn publication::PreparedWorldField + 'target>,
        (
            Box<dyn publication::PreparedWorldField + 'target>,
            publication::FieldRefusal,
        ),
    > {
        publication::prepare_storage(self, target)
    }
}

impl<'a, K: Key, V: Value> CaptureWorldField for StorageBlock<'a, K, V> {
    type Target = Storage<K, V>;
    type Retained = RetainedStorage<K, V>;
    type Capture = StorageCaptureSlot<'a, K, V, ()>;
    fn capture_mode(&self) -> Result<BlockMode, CaptureError<Infallible>> {
        Ok(self.mode())
    }
    fn into_capture(self) -> Self::Capture {
        self.capture_slot()
    }
}

impl<K: Key, V: Value> WorldCaptureSlot for StorageCaptureSlot<'_, K, V, ()> {
    type Target = Storage<K, V>;
    type Retained = RetainedStorage<K, V>;
    fn capture(&mut self) -> Result<(), CaptureError<Infallible>> {
        match self.try_capture(|_| Ok::<(), Infallible>(())) {
            Ok(()) => Ok(()),
            Err(impossible) => match impossible {},
        }
    }
    fn release(&mut self) {
        BlockCapture::release(self);
    }
    fn retain(self, name: &'static str, target: fn(&World) -> &Self::Target) -> Self::Retained {
        let (journal, cleanup) = self.into_detached();
        let retained = RetainedStorage {
            name,
            journal: Some(journal),
            target,
        };
        drop(cleanup);
        retained
    }
}

struct RetainedCell<V: Value> {
    name: &'static str,
    // Empty only while the same box is held by its private prepared owner.
    journal: Option<mv::cell::Detached<V, ()>>,
    target: fn(&World) -> &Cell<V>,
}

impl<V: Value> RetainedWorldField for RetainedCell<V> {
    fn summary(&self) -> FieldSummary {
        let journal = self.journal.as_ref().expect("retained original journal");
        FieldSummary {
            name: self.name,
            mode: journal.mode(),
            touched_values: usize::from(journal.touched_value().is_some()),
        }
    }
    fn matches_current(&self, target: &World) -> bool {
        self.journal
            .as_ref()
            .expect("retained original journal")
            .matches_current((self.target)(target))
    }
    fn try_prepare<'target>(
        self: Box<Self>,
        target: &'target World,
    ) -> Result<
        Box<dyn publication::PreparedWorldField + 'target>,
        (
            Box<dyn publication::PreparedWorldField + 'target>,
            publication::FieldRefusal,
        ),
    > {
        publication::prepare_cell(self, target)
    }
}

impl<'a, V: Value> CaptureWorldField for CellBlock<'a, V> {
    type Target = Cell<V>;
    type Retained = RetainedCell<V>;
    type Capture = CellCaptureSlot<'a, V, ()>;
    fn capture_mode(&self) -> Result<BlockMode, CaptureError<Infallible>> {
        Ok(self.mode())
    }
    fn into_capture(self) -> Self::Capture {
        self.capture_slot()
    }
}

impl<V: Value> WorldCaptureSlot for CellCaptureSlot<'_, V, ()> {
    type Target = Cell<V>;
    type Retained = RetainedCell<V>;
    fn capture(&mut self) -> Result<(), CaptureError<Infallible>> {
        match self.try_capture(|_| Ok::<(), Infallible>(())) {
            Ok(()) => Ok(()),
            Err(impossible) => match impossible {},
        }
    }
    fn release(&mut self) {
        BlockCapture::release(self);
    }
    fn retain(self, name: &'static str, target: fn(&World) -> &Self::Target) -> Self::Retained {
        let (journal, cleanup) = self.into_detached();
        let retained = RetainedCell {
            name,
            journal: Some(journal),
            target,
        };
        drop(cleanup);
        retained
    }
}

struct RetainedTriggers {
    name: &'static str,
    // Empty only while the same box is held by its private prepared owner.
    journal: Option<DetachedSet<()>>,
    target: fn(&World) -> &TriggerSet,
}

impl RetainedWorldField for RetainedTriggers {
    fn summary(&self) -> FieldSummary {
        let journal = self.journal.as_ref().expect("retained original journal");
        // The trigger owner exposes these ten immutable typed journals. Its
        // own exhaustive SetBlock destructuring remains the completeness gate.
        let touched_values = journal.data_triggers().touched_entries().len()
            + journal.pipeline_triggers().touched_entries().len()
            + journal.time_triggers().touched_entries().len()
            + journal.by_call_triggers().touched_entries().len()
            + journal.ids().touched_entries().len()
            + journal.active_data_trigger_ids().touched_entries().len()
            + journal
                .active_pipeline_trigger_ids()
                .touched_entries()
                .len()
            + journal.active_time_trigger_ids().touched_entries().len()
            + journal.active_by_call_trigger_ids().touched_entries().len()
            + journal.contracts().touched_entries().len();
        FieldSummary {
            name: self.name,
            mode: journal.mode(),
            touched_values,
        }
    }
    fn matches_current(&self, target: &World) -> bool {
        self.journal
            .as_ref()
            .expect("retained original journal")
            .matches_current((self.target)(target))
    }
    fn try_prepare<'target>(
        self: Box<Self>,
        target: &'target World,
    ) -> Result<
        Box<dyn publication::PreparedWorldField + 'target>,
        (
            Box<dyn publication::PreparedWorldField + 'target>,
            publication::FieldRefusal,
        ),
    > {
        publication::prepare_triggers(self, target)
    }
}

impl<'a> CaptureWorldField for TriggerSetBlock<'a> {
    type Target = TriggerSet;
    type Retained = RetainedTriggers;
    type Capture = SetBlockCapture<'a, ()>;
    fn capture_mode(&self) -> Result<BlockMode, CaptureError<Infallible>> {
        TriggerSetBlock::capture_mode(self).map_err(trigger_error)
    }
    fn into_capture(self) -> Self::Capture {
        self.capture_slot()
    }
}

impl WorldCaptureSlot for SetBlockCapture<'_, ()> {
    type Target = TriggerSet;
    type Retained = RetainedTriggers;
    fn capture(&mut self) -> Result<(), CaptureError<Infallible>> {
        self.try_capture(|_| Ok::<(), Infallible>(()))
            .map_err(trigger_error)
    }
    fn release(&mut self) {
        Self::release(self);
    }
    fn retain(self, name: &'static str, target: fn(&World) -> &Self::Target) -> Self::Retained {
        let (journal, cleanup) = self.into_detached();
        let retained = RetainedTriggers {
            name,
            journal: Some(journal),
            target,
        };
        drop(cleanup);
        retained
    }
}

macro_rules! declare_world_capture {
    (; [$($prefix:ident,)*] [$($privacy:ident,)*] [$($suffix:ident,)*]) => {
        #[allow(non_camel_case_types)]
        struct WorldCapture<$($prefix: WorldCaptureSlot,)* $($privacy: WorldCaptureSlot,)* $($suffix: WorldCaptureSlot,)*> {
            $($prefix: Option<$prefix>,)* $($privacy: Option<$privacy>,)* $($suffix: Option<$suffix>,)*
        }
        #[allow(non_camel_case_types)]
        impl<$($prefix: WorldCaptureSlot,)* $($privacy: WorldCaptureSlot,)* $($suffix: WorldCaptureSlot,)*>
            WorldCapture<$($prefix,)* $($privacy,)* $($suffix,)*>
        {
            fn capture(&mut self) -> Result<(), CaptureError<Infallible>> {
                $(self.$prefix.as_mut().expect("original World capture slot").capture()?;)*
                $(self.$privacy.as_mut().expect("original World capture slot").capture()?;)*
                $(self.$suffix.as_mut().expect("original World capture slot").capture()?;)*
                Ok(())
            }
        }
        #[allow(non_camel_case_types)]
        impl<$($prefix: WorldCaptureSlot,)* $($privacy: WorldCaptureSlot,)* $($suffix: WorldCaptureSlot,)*>
            Drop for WorldCapture<$($prefix,)* $($privacy,)* $($suffix,)*>
        {
            fn drop(&mut self) {
                $(if let Some(field) = self.$prefix.as_mut() { field.release(); })*
                $(if let Some(field) = self.$privacy.as_mut() { field.release(); })*
                $(if let Some(field) = self.$suffix.as_mut() { field.release(); })*
            }
        }
    };
}

with_world_overlay_fields!(declare_world_capture);

// Borrow the original block and caller slots rather than passing another
// complete World owner by value through a deep retained-validation call stack.
#[inline(never)]
fn fill_world_capture(fill: impl FnOnce()) {
    fill()
}

// Wrapper construction temporaries do not overlap native capture work.
#[inline(never)]
fn finish_world_capture<R>(finish: impl FnOnce() -> R) -> R {
    finish()
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
        // These payloads and their admission outlive the capture aggregate on
        // unwind: release every original writer before either can be destroyed.
        let mut extras = None;
        let mut pending = WorldCapture {
            $($prefix: None,)* $($privacy: None,)* $($suffix: None,)*
        };
        fill_world_capture(|| {
            // Inert moves only after extraction. The closure borrows both
            // original owners; its transfer temporaries leave before capture.
            let WorldBlockFields {
                dataspace_catalog,
                $($prefix,)* $($privacy,)* $($suffix,)*
                external_event_buf,
            } = $original.fields.take().expect("original World block fields");
            $(pending.$prefix = Some($prefix.into_capture());)*
            $(pending.$privacy = Some($privacy.into_capture());)*
            $(pending.$suffix = Some($suffix.into_capture());)*
            extras = Some((dataspace_catalog, external_event_buf));
        });
        pending.capture().map_err(widen_error)?;
        // All sibling writers are now free. Original notifications can be
        // retired while materializing the admitted journal wrappers.
        const FIELD_COUNT: usize = [
            $(stringify!($prefix),)* $(stringify!($privacy),)* $(stringify!($suffix),)*
        ].len();
        let fields = finish_world_capture(|| {
            let mut fields: Vec<Box<dyn RetainedWorldField>> = Vec::with_capacity(FIELD_COUNT);
            $(retain_field!(fields, pending, $prefix);)*
            $(retain_field!(fields, pending, $privacy);)*
            $(retain_field!(fields, pending, $suffix);)*
            fields
        });
        let (dataspace_catalog, external_event_buf) = extras.take().expect("original World extras");
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
    ($fields:ident, $pending:ident, $field:ident) => {
        $fields.push(Box::new(
            $pending
                .$field
                .take()
                .expect("original World capture slot")
                .retain(stringify!($field), |target: &World| &target.$field),
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
    /// extras and the original MV current/undo allocations without cloning them.
    pub(in crate::state) fn try_detach_journals<Admission, E>(
        mut self,
        admit: impl FnOnce(&Self) -> Result<Admission, E>,
    ) -> Result<DetachedWorld<Admission>, CaptureError<E>> {
        with_world_overlay_fields!(capture_world_fields, self, admit)
    }
}

#[cfg(test)]
#[path = "world_journals_tests.rs"]
mod tests;
