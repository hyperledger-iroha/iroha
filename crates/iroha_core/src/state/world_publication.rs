//! Consume every captured World component under its exact original writers.

use super::*;
use crate::smartcontracts::isi::triggers::set::{PreparedSet, SetPublicationError};
use mv::PublicationPreparationError;

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
    fn abort(self: Box<Self>) -> Box<dyn RetainedWorldField>;
    fn publish(self: Box<Self>);
}

struct PreparedStorage<'target, K: Key, V: Value> {
    original: Box<RetainedStorage<K, V>>,
    journal: Option<mv::storage::PreparedPublication<'target, K, V, (), ()>>,
}

impl<K: Key, V: Value> PreparedWorldField for PreparedStorage<'_, K, V> {
    fn abort(self: Box<Self>) -> Box<dyn RetainedWorldField> {
        let Self {
            mut original,
            journal,
        } = *self;
        original.journal = Some(journal.expect("prepared original journal").abort());
        original
    }

    fn publish(self: Box<Self>) {
        self.journal.expect("prepared original journal").publish();
    }
}

pub(super) fn prepare_storage<'target, K: Key, V: Value>(
    original: Box<RetainedStorage<K, V>>,
    world: &'target World,
) -> Result<Box<dyn PreparedWorldField + 'target>, (Box<dyn RetainedWorldField>, FieldRefusal)> {
    // Allocate only the transient prepared shell before taking this field's writers.
    // The populated original box stays owned throughout acquisition and rollback.
    let mut prepared = Box::new(PreparedStorage {
        original,
        journal: None,
    });
    let name = prepared.original.name;
    let target = prepared.original.target;
    let journal = prepared
        .original
        .journal
        .take()
        .expect("retained original journal");
    match journal.try_prepare_publication(target(world), |_, _| Ok::<_, Infallible>(())) {
        Ok(journal) => {
            prepared.journal = Some(journal);
            Ok(prepared)
        }
        Err((journal, cause)) => {
            prepared.original.journal = Some(journal);
            Err((
                prepared.original,
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
    original: Box<RetainedCell<V>>,
    journal: Option<mv::cell::PreparedPublication<'target, V, (), ()>>,
}

impl<V: Value> PreparedWorldField for PreparedCell<'_, V> {
    fn abort(self: Box<Self>) -> Box<dyn RetainedWorldField> {
        let Self {
            mut original,
            journal,
        } = *self;
        original.journal = Some(journal.expect("prepared original journal").abort());
        original
    }

    fn publish(self: Box<Self>) {
        self.journal.expect("prepared original journal").publish();
    }
}

pub(super) fn prepare_cell<'target, V: Value>(
    original: Box<RetainedCell<V>>,
    world: &'target World,
) -> Result<Box<dyn PreparedWorldField + 'target>, (Box<dyn RetainedWorldField>, FieldRefusal)> {
    // Allocate only the transient prepared shell before taking this field's writers.
    // The populated original box stays owned throughout acquisition and rollback.
    let mut prepared = Box::new(PreparedCell {
        original,
        journal: None,
    });
    let name = prepared.original.name;
    let target = prepared.original.target;
    let journal = prepared
        .original
        .journal
        .take()
        .expect("retained original journal");
    match journal.try_prepare_publication(target(world), |_, _| Ok::<_, Infallible>(())) {
        Ok(journal) => {
            prepared.journal = Some(journal);
            Ok(prepared)
        }
        Err((journal, cause)) => {
            prepared.original.journal = Some(journal);
            Err((
                prepared.original,
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
    original: Box<RetainedTriggers>,
    journal: Option<PreparedSet<'target, (), ()>>,
}

impl PreparedWorldField for PreparedTriggers<'_> {
    fn abort(self: Box<Self>) -> Box<dyn RetainedWorldField> {
        let Self {
            mut original,
            journal,
        } = *self;
        original.journal = Some(journal.expect("prepared original journal").abort());
        original
    }

    fn publish(self: Box<Self>) {
        self.journal.expect("prepared original journal").publish();
    }
}

pub(super) fn prepare_triggers<'target>(
    original: Box<RetainedTriggers>,
    world: &'target World,
) -> Result<Box<dyn PreparedWorldField + 'target>, (Box<dyn RetainedWorldField>, FieldRefusal)> {
    // Allocate only the transient prepared shell before taking this field's writers.
    // The populated original box stays owned throughout acquisition and rollback.
    let mut prepared = Box::new(PreparedTriggers {
        original,
        journal: None,
    });
    let name = prepared.original.name;
    let target = prepared.original.target;
    let journal = prepared
        .original
        .journal
        .take()
        .expect("retained original journal");
    match journal.try_prepare_publication(target(world), |_, _| Ok::<_, Infallible>(())) {
        Ok(journal) => {
            prepared.journal = Some(journal);
            Ok(prepared)
        }
        Err((journal, SetPublicationError::Component { field, cause })) => {
            prepared.original.journal = Some(journal);
            Err((
                prepared.original,
                FieldRefusal {
                    field: name,
                    trigger_component: Some(field),
                    cause,
                },
            ))
        }
        Err((_, SetPublicationError::Admission(impossible))) => match impossible {},
    }
}

/// All original World writers retained together, with no State authorization.
///
/// The enclosing State owner must prepare every other component and authenticate
/// its finality/persistence authority before making any component visible.
#[must_use = "complete World preparation must be published or aborted by its State owner"]
pub(in crate::state) struct PreparedWorld<'target, Admission, Installation> {
    mode: BlockMode,
    fields: Vec<Box<dyn PreparedWorldField + 'target>>,
    // Original field-vector allocation, kept empty until an exact rollback.
    retry: Vec<Box<dyn RetainedWorldField>>,
    dataspace_catalog: DataSpaceCatalog,
    external_event_buf: Vec<EventBox>,
    admission: Admission,
    installation: Installation,
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
    ) -> Result<PreparedWorld<'target, Admission, Installation>, (Self, WorldPublicationError<E>)>
    {
        // These locals precede every payload so panic unwinding releases
        // original/prepared fields before either retained capacity owner.
        let installation;
        let admission;
        installation = match admit(&self, target) {
            Ok(installation) => installation,
            Err(error) => return Err((self, WorldPublicationError::Admission(error))),
        };
        let Self {
            mode,
            mut fields,
            dataspace_catalog,
            external_event_buf,
            admission: retained_admission,
        } = self;
        admission = retained_admission;
        let mut prepared: Vec<Box<dyn PreparedWorldField + 'target>> =
            Vec::with_capacity(fields.len());
        // Pop in inventory order while retaining the original vector allocation.
        fields.reverse();
        while let Some(field) = fields.pop() {
            match field.try_prepare(target) {
                Ok(field) => prepared.push(field),
                Err((field, error)) => {
                    fields.push(field);
                    // Restore the original order without allocating rollback custody.
                    fields.extend(prepared.into_iter().rev().map(|field| field.abort()));
                    fields.reverse();
                    drop(installation);
                    return Err((
                        DetachedWorld {
                            mode,
                            fields,
                            dataspace_catalog,
                            external_event_buf,
                            admission,
                        },
                        WorldPublicationError::Field(error),
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

impl<Admission, Installation> PreparedWorld<'_, Admission, Installation> {
    /// Release every writer and return the complete original journals and extras.
    pub(in crate::state) fn abort(self) -> DetachedWorld<Admission> {
        let installation;
        let admission;
        let Self {
            mode,
            fields,
            mut retry,
            dataspace_catalog,
            external_event_buf,
            admission: retained_admission,
            installation: retained_installation,
        } = self;
        installation = retained_installation;
        admission = retained_admission;
        retry.extend(fields.into_iter().map(|field| field.abort()));
        drop(installation);
        DetachedWorld {
            mode,
            fields: retry,
            dataspace_catalog,
            external_event_buf,
            admission,
        }
    }

    /// Consume all prepared fields, handing extras and both guards to State.
    /// Caller-owned State visibility/finality must already exclude partial reads.
    pub(in crate::state) fn publish(
        self,
    ) -> (DataSpaceCatalog, Vec<EventBox>, Admission, Installation) {
        let installation;
        let admission;
        let Self {
            mode: _,
            fields,
            retry,
            dataspace_catalog,
            external_event_buf,
            admission: retained_admission,
            installation: retained_installation,
        } = self;
        installation = retained_installation;
        admission = retained_admission;
        for field in fields {
            field.publish();
        }
        drop(retry);
        (
            dataspace_catalog,
            external_event_buf,
            admission,
            installation,
        )
    }
}
