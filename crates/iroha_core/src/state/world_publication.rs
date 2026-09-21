//! Consume every captured World component under its exact original writers.

use super::*;
use crate::smartcontracts::isi::triggers::set::{PreparedSet, SetPublicationError};
use mv::PublicationPreparationError;
use std::alloc::Layout;

// These layouts describe the concrete Box pointees constructed below. Lifetimes
// do not change layout; no target owner, value, reader or writer is acquired.
pub(super) fn storage_shell_layout<K: Key, V: Value>() -> Layout {
    Layout::new::<PreparedStorage<'static, K, V>>()
}

pub(super) fn cell_shell_layout<V: Value>() -> Layout {
    Layout::new::<PreparedCell<'static, V>>()
}

pub(super) fn triggers_shell_layout() -> Layout {
    Layout::new::<PreparedTriggers<'static>>()
}

pub(super) fn field_vector_layout(capacity: usize) -> Result<Layout, std::alloc::LayoutError> {
    Layout::array::<Box<dyn PreparedWorldField>>(capacity)
}

/// Exact local component that prevented complete World preparation.
#[derive(Debug)]
pub(in crate::state) struct FieldRefusal {
    /// Original field from the exhaustive World inventory.
    pub field: &'static str,
    /// Inner TriggerSet component, when that aggregate refused preparation.
    pub trigger_component: Option<&'static str>,
    /// Busy, changed or poisoned original publication; never a consensus verdict.
    pub cause: PublicationPreparationError<Infallible>,
}

/// Refusal preserves all original World journals and releases every writer.
#[derive(Debug)]
pub(in crate::state) enum WorldPublicationError<E> {
    /// Complete installation resources were refused before any writer acquisition.
    Admission(E),
    /// One exact original owner could not be prepared.
    Field(FieldRefusal),
}

pub(super) trait PreparedWorldField {
    fn release(&mut self);
    fn abort(&mut self) -> Box<dyn RetainedWorldField>;
    fn publish(&mut self);
}

struct PreparedStorage<'target, K: Key, V: Value> {
    original: Option<Box<RetainedStorage<K, V>>>,
    journal: Option<mv::storage::PreparedPublication<'target, K, V, (), ()>>,
    published: Option<mv::storage::PublishedPublication<K, V, (), ()>>,
    aborted: Option<mv::PublicationCleanup<()>>,
}

impl<K: Key, V: Value> PreparedWorldField for PreparedStorage<'_, K, V> {
    fn release(&mut self) {
        if let Some(journal) = self.journal.take() {
            let (journal, retirement) = journal.abort();
            self.original.as_mut().expect("original field box").journal = Some(journal);
            self.aborted = Some(retirement);
        }
    }

    fn abort(&mut self) -> Box<dyn RetainedWorldField> {
        self.release();
        self.original.take().expect("original field box")
    }

    fn publish(&mut self) {
        self.published = Some(
            self.journal
                .take()
                .expect("prepared original journal")
                .publish(),
        );
    }
}

pub(super) fn prepare_storage<'target, K: Key, V: Value>(
    original: Box<RetainedStorage<K, V>>,
    world: &'target World,
) -> Result<
    Box<dyn PreparedWorldField + 'target>,
    (Box<dyn PreparedWorldField + 'target>, FieldRefusal),
> {
    // Allocate only the transient prepared shell before taking this field's writers.
    // The populated original box stays owned throughout acquisition and rollback.
    let mut prepared = Box::new(PreparedStorage {
        original: Some(original),
        journal: None,
        published: None,
        aborted: None,
    });
    let name = prepared.original.as_ref().expect("original field box").name;
    let target = prepared
        .original
        .as_ref()
        .expect("original field box")
        .target;
    let journal = prepared
        .original
        .as_mut()
        .expect("original field box")
        .journal
        .take()
        .expect("retained original journal");
    match journal.try_prepare_publication(target(world), |_, _| Ok::<_, Infallible>(())) {
        Ok(journal) => {
            prepared.journal = Some(journal);
            Ok(prepared)
        }
        Err((journal, cause, cleanup)) => {
            prepared
                .original
                .as_mut()
                .expect("original field box")
                .journal = Some(journal);
            prepared.aborted = Some(cleanup);
            Err((
                prepared,
                FieldRefusal {
                    field: name,
                    trigger_component: None,
                    cause,
                },
            ))
        }
    }
}

struct PreparedCell<'target, V: Value> {
    original: Option<Box<RetainedCell<V>>>,
    journal: Option<mv::cell::PreparedPublication<'target, V, (), ()>>,
    published: Option<mv::cell::PublishedPublication<V, (), ()>>,
    aborted: Option<mv::PublicationCleanup<()>>,
}

impl<V: Value> PreparedWorldField for PreparedCell<'_, V> {
    fn release(&mut self) {
        if let Some(journal) = self.journal.take() {
            let (journal, retirement) = journal.abort();
            self.original.as_mut().expect("original field box").journal = Some(journal);
            self.aborted = Some(retirement);
        }
    }

    fn abort(&mut self) -> Box<dyn RetainedWorldField> {
        self.release();
        self.original.take().expect("original field box")
    }

    fn publish(&mut self) {
        self.published = Some(
            self.journal
                .take()
                .expect("prepared original journal")
                .publish(),
        );
    }
}

pub(super) fn prepare_cell<'target, V: Value>(
    original: Box<RetainedCell<V>>,
    world: &'target World,
) -> Result<
    Box<dyn PreparedWorldField + 'target>,
    (Box<dyn PreparedWorldField + 'target>, FieldRefusal),
> {
    // Allocate only the transient prepared shell before taking this field's writers.
    // The populated original box stays owned throughout acquisition and rollback.
    let mut prepared = Box::new(PreparedCell {
        original: Some(original),
        journal: None,
        published: None,
        aborted: None,
    });
    let name = prepared.original.as_ref().expect("original field box").name;
    let target = prepared
        .original
        .as_ref()
        .expect("original field box")
        .target;
    let journal = prepared
        .original
        .as_mut()
        .expect("original field box")
        .journal
        .take()
        .expect("retained original journal");
    match journal.try_prepare_publication(target(world), |_, _| Ok::<_, Infallible>(())) {
        Ok(journal) => {
            prepared.journal = Some(journal);
            Ok(prepared)
        }
        Err((journal, cause, cleanup)) => {
            prepared
                .original
                .as_mut()
                .expect("original field box")
                .journal = Some(journal);
            prepared.aborted = Some(cleanup);
            Err((
                prepared,
                FieldRefusal {
                    field: name,
                    trigger_component: None,
                    cause,
                },
            ))
        }
    }
}

struct PreparedTriggers<'target> {
    original: Option<Box<RetainedTriggers>>,
    journal: Option<PreparedSet<'target, (), ()>>,
    published: Option<crate::smartcontracts::isi::triggers::set::PublishedSet<(), ()>>,
    aborted: Option<crate::smartcontracts::isi::triggers::set::AbortedSet<()>>,
}

impl PreparedWorldField for PreparedTriggers<'_> {
    fn release(&mut self) {
        if let Some(journal) = self.journal.take() {
            let (journal, retirement) = journal.abort();
            self.original.as_mut().expect("original field box").journal = Some(journal);
            self.aborted = Some(retirement);
        }
    }

    fn abort(&mut self) -> Box<dyn RetainedWorldField> {
        self.release();
        self.original.take().expect("original field box")
    }

    fn publish(&mut self) {
        self.published = Some(
            self.journal
                .take()
                .expect("prepared original journal")
                .publish(),
        );
    }
}

pub(super) fn prepare_triggers<'target>(
    original: Box<RetainedTriggers>,
    world: &'target World,
) -> Result<
    Box<dyn PreparedWorldField + 'target>,
    (Box<dyn PreparedWorldField + 'target>, FieldRefusal),
> {
    // Allocate only the transient prepared shell before taking this field's writers.
    // The populated original box stays owned throughout acquisition and rollback.
    let mut prepared = Box::new(PreparedTriggers {
        original: Some(original),
        journal: None,
        published: None,
        aborted: None,
    });
    let name = prepared.original.as_ref().expect("original field box").name;
    let target = prepared
        .original
        .as_ref()
        .expect("original field box")
        .target;
    let journal = prepared
        .original
        .as_mut()
        .expect("original field box")
        .journal
        .take()
        .expect("retained original journal");
    match journal.try_prepare_publication(target(world), |_, _| Ok::<_, Infallible>(())) {
        Ok(journal) => {
            prepared.journal = Some(journal);
            Ok(prepared)
        }
        Err((journal, SetPublicationError::Component { field, cause }, cleanup)) => {
            prepared
                .original
                .as_mut()
                .expect("original field box")
                .journal = Some(journal);
            prepared.aborted = Some(cleanup);
            Err((
                prepared,
                FieldRefusal {
                    field: name,
                    trigger_component: Some(field),
                    cause,
                },
            ))
        }
        Err((_, SetPublicationError::Admission(impossible), _)) => match impossible {},
    }
}

/// The original heterogeneous prepared vector releases every writer before any
/// field shell, payload or callback is destroyed, including preparation unwind.
struct PreparedWorldFields<'target>(Vec<Box<dyn PreparedWorldField + 'target>>);

impl<'target> PreparedWorldFields<'target> {
    fn release_all(&mut self) {
        for field in &mut self.0 {
            field.release();
        }
    }

    fn into_inner(mut self) -> Vec<Box<dyn PreparedWorldField + 'target>> {
        std::mem::take(&mut self.0)
    }
}
impl<'target> std::ops::Deref for PreparedWorldFields<'target> {
    type Target = Vec<Box<dyn PreparedWorldField + 'target>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for PreparedWorldFields<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Drop for PreparedWorldFields<'_> {
    fn drop(&mut self) {
        self.release_all();
    }
}

/// Original shells and abort cleanup retained with installation admission.
pub(in crate::state) struct AbortedWorld<'target, Installation> {
    _fields: PreparedWorldFields<'target>,
    _installation: Option<Installation>,
}

/// All original World writers retained together, with no State authorization.
///
/// The enclosing State owner must prepare every other component and authenticate
/// its finality/persistence authority before making any component visible.
#[must_use = "complete World preparation must be published or aborted by its State owner"]
pub(in crate::state) struct PreparedWorld<'target, Admission, Installation> {
    mode: BlockMode,
    fields: PreparedWorldFields<'target>,
    // Original field-vector allocation, kept empty until an exact rollback.
    retry: Vec<Box<dyn RetainedWorldField>>,
    dataspace_catalog: DataSpaceCatalog,
    external_event_buf: Vec<EventBox>,
    admission: Admission,
    installation: Installation,
}

/// Original field boxes and containers retained after physical publication.
/// Drop after all State fences, before releasing the enclosing resource guards.
pub(in crate::state) struct WorldRetirement<'target> {
    _fields: Vec<Box<dyn PreparedWorldField + 'target>>,
    _retry: Vec<Box<dyn RetainedWorldField>>,
}

impl<Admission> DetachedWorld<Admission> {
    /// Admit all World installation costs, then acquire every exact component.
    ///
    /// Admission covers the prepared field container and transient wrappers,
    /// all COW staging, undo and retained-reader publication costs. Failed or
    /// aborted preparation returns the original vector and populated field boxes
    /// without replacement allocations; it never reexecutes or recaptures World.
    pub(in crate::state) fn try_prepare_publication<'target, Installation, E>(
        self,
        target: &'target World,
        admit: impl FnOnce(&Self, &World) -> Result<Installation, E>,
    ) -> Result<
        PreparedWorld<'target, Admission, Installation>,
        (
            Self,
            WorldPublicationError<E>,
            AbortedWorld<'target, Installation>,
        ),
    > {
        // These locals precede every payload so panic unwinding releases
        // original/prepared fields before either retained capacity owner.
        let installation;
        let admission;
        installation = match admit(&self, target) {
            Ok(installation) => installation,
            Err(error) => {
                return Err((
                    self,
                    WorldPublicationError::Admission(error),
                    AbortedWorld {
                        _fields: PreparedWorldFields(Vec::new()),
                        _installation: None,
                    },
                ));
            }
        };
        let Self {
            mode,
            mut fields,
            dataspace_catalog,
            external_event_buf,
            admission: retained_admission,
        } = self;
        admission = retained_admission;
        let mut prepared = PreparedWorldFields(Vec::with_capacity(fields.len()));
        // Pop in inventory order while retaining the original vector allocation.
        fields.reverse();
        while let Some(field) = fields.pop() {
            match field.try_prepare(target) {
                Ok(field) => prepared.push(field),
                Err((field, error)) => {
                    prepared.push(field);
                    // Restore the original order without allocating rollback custody.
                    prepared.release_all();
                    fields.extend(prepared.iter_mut().rev().map(|field| field.abort()));
                    fields.reverse();
                    let retirement = AbortedWorld {
                        _fields: prepared,
                        _installation: Some(installation),
                    };
                    return Err((
                        DetachedWorld {
                            mode,
                            fields,
                            dataspace_catalog,
                            external_event_buf,
                            admission,
                        },
                        WorldPublicationError::Field(error),
                        retirement,
                    ));
                }
            }
        }
        Ok(PreparedWorld {
            mode,
            fields: prepared,
            retry: fields,
            dataspace_catalog,
            external_event_buf,
            admission,
            installation,
        })
    }
}

impl<'target, Admission, Installation> PreparedWorld<'target, Admission, Installation> {
    /// Observe actual constructed shell sizes and both coexisting Vec capacities.
    #[cfg(test)]
    pub(super) fn observed_shell_layouts(
        &self,
    ) -> (Layout, Layout, impl ExactSizeIterator<Item = Layout> + '_) {
        (
            Layout::array::<Box<dyn RetainedWorldField>>(self.retry.capacity()).unwrap(),
            field_vector_layout(self.fields.capacity()).unwrap(),
            self.fields
                .iter()
                .map(|field| Layout::for_value(field.as_ref())),
        )
    }

    /// Release every writer and return the complete original journals and extras.
    pub(in crate::state) fn abort(
        self,
    ) -> (
        DetachedWorld<Admission>,
        AbortedWorld<'target, Installation>,
    ) {
        let installation;
        let admission;
        let Self {
            mode,
            mut fields,
            mut retry,
            dataspace_catalog,
            external_event_buf,
            admission: retained_admission,
            installation: retained_installation,
        } = self;
        installation = retained_installation;
        admission = retained_admission;
        fields.release_all();
        retry.extend(fields.iter_mut().map(|field| field.abort()));
        let retirement = AbortedWorld {
            _fields: fields,
            _installation: Some(installation),
        };
        (
            DetachedWorld {
                mode,
                fields: retry,
                dataspace_catalog,
                external_event_buf,
                admission,
            },
            retirement,
        )
    }

    /// Consume all prepared fields, handing extras and both guards to State.
    /// Caller-owned State visibility/finality must already exclude partial reads.
    pub(in crate::state) fn publish(
        self,
    ) -> (
        DataSpaceCatalog,
        Vec<EventBox>,
        WorldRetirement<'target>,
        Admission,
        Installation,
    ) {
        let installation;
        let admission;
        let Self {
            mode: _,
            mut fields,
            retry,
            dataspace_catalog,
            external_event_buf,
            admission: retained_admission,
            installation: retained_installation,
        } = self;
        installation = retained_installation;
        admission = retained_admission;
        for field in fields.iter_mut() {
            field.publish();
        }
        let retirement = WorldRetirement {
            _fields: fields.into_inner(),
            _retry: retry,
        };
        (
            dataspace_catalog,
            external_event_buf,
            retirement,
            admission,
            installation,
        )
    }
}
