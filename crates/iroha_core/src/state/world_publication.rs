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
    name: &'static str,
    journal: mv::storage::PreparedPublication<'target, K, V, (), ()>,
    target: fn(&World) -> &Storage<K, V>,
}

impl<K: Key, V: Value> PreparedWorldField for PreparedStorage<'_, K, V> {
    fn abort(self: Box<Self>) -> Box<dyn RetainedWorldField> {
        let Self {
            name,
            journal,
            target,
        } = *self;
        Box::new(RetainedStorage {
            name,
            journal: journal.abort(),
            target,
        })
    }

    fn publish(self: Box<Self>) {
        self.journal.publish();
    }
}

pub(super) fn prepare_storage<'target, K: Key, V: Value>(
    original: Box<RetainedStorage<K, V>>,
    world: &'target World,
) -> Result<Box<dyn PreparedWorldField + 'target>, (Box<dyn RetainedWorldField>, FieldRefusal)> {
    let RetainedStorage {
        name,
        journal,
        target,
    } = *original;
    match journal.try_prepare_publication(target(world), |_, _| Ok::<_, Infallible>(())) {
        Ok(journal) => Ok(Box::new(PreparedStorage {
            name,
            journal,
            target,
        })),
        Err((journal, cause)) => Err((
            Box::new(RetainedStorage {
                name,
                journal,
                target,
            }),
            FieldRefusal {
                field: name,
                trigger_component: None,
                cause,
            },
        )),
    }
}

struct PreparedCell<'target, V: Value> {
    name: &'static str,
    journal: mv::cell::PreparedPublication<'target, V, (), ()>,
    target: fn(&World) -> &Cell<V>,
}

impl<V: Value> PreparedWorldField for PreparedCell<'_, V> {
    fn abort(self: Box<Self>) -> Box<dyn RetainedWorldField> {
        let Self {
            name,
            journal,
            target,
        } = *self;
        Box::new(RetainedCell {
            name,
            journal: journal.abort(),
            target,
        })
    }

    fn publish(self: Box<Self>) {
        self.journal.publish();
    }
}

pub(super) fn prepare_cell<'target, V: Value>(
    original: Box<RetainedCell<V>>,
    world: &'target World,
) -> Result<Box<dyn PreparedWorldField + 'target>, (Box<dyn RetainedWorldField>, FieldRefusal)> {
    let RetainedCell {
        name,
        journal,
        target,
    } = *original;
    match journal.try_prepare_publication(target(world), |_, _| Ok::<_, Infallible>(())) {
        Ok(journal) => Ok(Box::new(PreparedCell {
            name,
            journal,
            target,
        })),
        Err((journal, cause)) => Err((
            Box::new(RetainedCell {
                name,
                journal,
                target,
            }),
            FieldRefusal {
                field: name,
                trigger_component: None,
                cause,
            },
        )),
    }
}

struct PreparedTriggers<'target> {
    name: &'static str,
    journal: PreparedSet<'target, (), ()>,
    target: fn(&World) -> &TriggerSet,
}

impl PreparedWorldField for PreparedTriggers<'_> {
    fn abort(self: Box<Self>) -> Box<dyn RetainedWorldField> {
        let Self {
            name,
            journal,
            target,
        } = *self;
        Box::new(RetainedTriggers {
            name,
            journal: journal.abort(),
            target,
        })
    }

    fn publish(self: Box<Self>) {
        self.journal.publish();
    }
}

pub(super) fn prepare_triggers<'target>(
    original: Box<RetainedTriggers>,
    world: &'target World,
) -> Result<Box<dyn PreparedWorldField + 'target>, (Box<dyn RetainedWorldField>, FieldRefusal)> {
    let RetainedTriggers {
        name,
        journal,
        target,
    } = *original;
    match journal.try_prepare_publication(target(world), |_, _| Ok::<_, Infallible>(())) {
        Ok(journal) => Ok(Box::new(PreparedTriggers {
            name,
            journal,
            target,
        })),
        Err((journal, SetPublicationError::Component { field, cause })) => Err((
            Box::new(RetainedTriggers {
                name,
                journal,
                target,
            }),
            FieldRefusal {
                field: name,
                trigger_component: Some(field),
                cause,
            },
        )),
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
    // Reserve the exact retry container before acquiring any original writer.
    retry: Vec<Box<dyn RetainedWorldField>>,
    dataspace_catalog: DataSpaceCatalog,
    external_event_buf: Vec<EventBox>,
    admission: Admission,
    installation: Installation,
}

impl<Admission> DetachedWorld<Admission> {
    /// Admit all World installation costs, then acquire every exact component.
    ///
    /// Admission must cover both field containers, prepared/restored wrappers,
    /// all COW staging, undo and retained-reader publication costs. Failed or
    /// aborted preparation rebuilds only private wrappers around the same
    /// original journals; it never reexecutes, recaptures or reconstructs World.
    pub(in crate::state) fn try_prepare_publication<'target, Installation, E>(
        self,
        target: &'target World,
        admit: impl FnOnce(&Self, &World) -> Result<Installation, E>,
    ) -> Result<PreparedWorld<'target, Admission, Installation>, (Self, WorldPublicationError<E>)>
    {
        let installation = match admit(&self, target) {
            Ok(installation) => installation,
            Err(error) => return Err((self, WorldPublicationError::Admission(error))),
        };
        let Self {
            mode,
            fields,
            dataspace_catalog,
            external_event_buf,
            admission,
        } = self;
        let mut prepared: Vec<Box<dyn PreparedWorldField + 'target>> =
            Vec::with_capacity(fields.len());
        let mut retry = Vec::with_capacity(fields.len());
        let mut remaining = fields.into_iter();
        while let Some(field) = remaining.next() {
            match field.try_prepare(target) {
                Ok(field) => prepared.push(field),
                Err((field, error)) => {
                    retry.extend(prepared.into_iter().map(|field| field.abort()));
                    retry.push(field);
                    retry.extend(remaining);
                    drop(installation);
                    return Err((
                        DetachedWorld {
                            mode,
                            fields: retry,
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
            retry,
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
        let Self {
            mode,
            fields,
            mut retry,
            dataspace_catalog,
            external_event_buf,
            admission,
            installation,
        } = self;
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
        let Self {
            mode: _,
            fields,
            retry,
            dataspace_catalog,
            external_event_buf,
            admission,
            installation,
        } = self;
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
