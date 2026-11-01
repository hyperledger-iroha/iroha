//! Caller-retained Windows originals beneath the shared strict progress protocol.
//!
//! The protocol remains inactive outside its explicit test adapters. This owner
//! supplies storage authority, not another intent format or recovery algorithm.
//! A caller holds its publication fence and keeps this value across every error.

use super::{bound_progress_io::*, *};
use std::{fs::File, io};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Data,
    Index,
    RewriteData,
    RewriteIndex,
    Prepend,
    Build,
    Intent,
}

const ROLES: [Role; 7] = [
    Role::Data,
    Role::Index,
    Role::RewriteData,
    Role::RewriteIndex,
    Role::Prepend,
    Role::Build,
    Role::Intent,
];

impl Role {
    const fn index(self) -> usize {
        self as usize
    }

    fn path(self, namespace: &BoundProgressNamespace) -> PathBuf {
        match self {
            Self::Data => namespace.data_path.clone(),
            Self::Index => namespace.index_path.clone(),
            Self::RewriteData => namespace.data_path.with_extension("norito.tmp"),
            Self::RewriteIndex => namespace.index_path.with_extension("index.tmp"),
            Self::Prepend => namespace.index_path.with_extension("index.prepend.tmp"),
            Self::Build => Kura::bound_progress_append_build_path(&namespace.index_path),
            Self::Intent => Kura::bound_progress_append_intent_path(&namespace.index_path),
        }
    }
}

#[derive(Default)]
struct Slot {
    original: Option<File>,
    identity: Option<iroha_fs::FileIdentity>,
    captured: bool,
    creator: bool,
    destination: Option<Role>,
    published: bool,
    retirement: Option<Role>,
    disposed: bool,
    closed: bool,
    displaced_closed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Captured(Role),
    Created(Role),
    Published(Role),
    Disposed(Role),
    Closed(Role),
    Barriers,
}

#[derive(Default)]
struct NativeBoundProgressIo {
    slots: [Slot; 7],
    first: Option<BoundProgressIoFailure>,
    mutation_started: bool,
    final_barrier_pending: bool,
    #[cfg(test)]
    fail_at: Option<Step>,
}

struct PendingAppend {
    height: u64,
    initial_base_height: u64,
    payload: Vec<u8>,
}

/// One strict pair and request retained by the caller through failed attempts.
// TODO: Connect the strict progress protocol only with its reviewed production
// admission and publication fence; these component owners do not activate it.
pub(super) struct NativeBoundProgressTransaction<'a> {
    kura: &'a Kura,
    namespace: BoundProgressNamespace,
    storage: NativeBoundProgressIo,
    pending: Option<PendingAppend>,
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, message)
}

fn parent(namespace: &BoundProgressNamespace) -> io::Result<&iroha_fs::OwnerDirectory> {
    if !Kura::progress_mutation_namespace_unchanged(namespace) {
        return Err(invalid("strict progress namespace changed"));
    }
    namespace
        .directories
        .first()
        .map(|directory| &directory.native_owner)
        .ok_or_else(|| invalid("strict progress namespace is empty"))
}

fn leaf(path: &Path) -> io::Result<&std::ffi::OsStr> {
    path.file_name()
        .ok_or_else(|| invalid("strict progress path has no leaf"))
}

#[cfg(test)]
fn observe_fault(fault: &mut Option<Step>, step: Step) -> io::Result<()> {
    if *fault == Some(step) {
        *fault = None;
        return Err(io::Error::other(
            "injected retained strict progress boundary",
        ));
    }
    Ok(())
}

impl NativeBoundProgressIo {
    fn observe(&mut self, step: Step) -> io::Result<()> {
        #[cfg(test)]
        return observe_fault(&mut self.fail_at, step);
        #[cfg(not(test))]
        {
            let _ = step;
            Ok(())
        }
    }

    fn role(namespace: &BoundProgressNamespace, path: &Path) -> io::Result<Role> {
        ROLES
            .into_iter()
            .find(|role| role.path(namespace) == path)
            .ok_or_else(|| invalid("path is outside the retained strict pair"))
    }

    /// A published source remains in its original slot until the final barriers.
    fn replacement(&self, destination: Role) -> Option<Role> {
        ROLES.into_iter().find(|role| {
            let slot = &self.slots[role.index()];
            slot.published && slot.destination == Some(destination)
        })
    }

    fn active(&self, role: Role) -> Option<Role> {
        let role = self.replacement(role).unwrap_or(role);
        let slot = &self.slots[role.index()];
        if slot.disposed || slot.closed || (slot.published && slot.destination != Some(role)) {
            // A moved source is visible only under its destination, handled below.
            None
        } else if slot.original.is_some() {
            Some(role)
        } else {
            None
        }
    }

    fn named_slot(&self, name: Role) -> Option<Role> {
        if let Some(source) = self.replacement(name) {
            let slot = &self.slots[source.index()];
            return (!slot.disposed && !slot.closed).then_some(source);
        }
        self.active(name)
    }

    fn absent(namespace: &BoundProgressNamespace, role: Role) -> io::Result<()> {
        let path = role.path(namespace);
        if parent(namespace)?
            .open_regular_optional(leaf(&path)?)?
            .is_some()
        {
            return Err(invalid("captured strict progress absence changed"));
        }
        Ok(())
    }

    fn capture(&mut self, namespace: &BoundProgressNamespace) -> io::Result<()> {
        for role in ROLES {
            let slot = &mut self.slots[role.index()];
            if !slot.captured {
                let path = role.path(namespace);
                // The transaction is already installed in its caller's slot.
                slot.original = parent(namespace)?.open_private_journal_optional(leaf(&path)?)?;
                slot.captured = true;
            }
            if let Some(file) = &slot.original
                && slot.identity.is_none()
            {
                slot.identity = Some(iroha_fs::FileIdentity::of(file)?);
            }
            self.observe(Step::Captured(role))?;
        }
        self.verify(namespace)
    }

    fn identity(slot: &Slot, file: &File) -> io::Result<()> {
        if slot.identity != Some(iroha_fs::FileIdentity::of(file)?) {
            return Err(invalid("retained strict progress original changed"));
        }
        Ok(())
    }

    fn verify(&self, namespace: &BoundProgressNamespace) -> io::Result<()> {
        let directory = parent(namespace)?;
        for role in ROLES {
            let slot = &self.slots[role.index()];
            if !slot.captured {
                return Err(invalid("strict progress capture is incomplete"));
            }
            if let Some(file) = &slot.original {
                Self::identity(slot, file)?;
                if slot.disposed || self.replacement(role).is_some() {
                    // POSIX replacement was observed with the same zero-link,
                    // delete-pending state as explicit original disposition.
                    directory.require_retired_private_journal(file)?;
                } else {
                    let current = slot.destination.filter(|_| slot.published).unwrap_or(role);
                    directory.require_same_lock(leaf(&current.path(namespace))?, file)?;
                }
            } else if self.replacement(role).is_none() {
                Self::absent(namespace, role)?;
            }
            if slot.published {
                Self::absent(namespace, role)?;
            }
            if slot.closed {
                Self::absent(namespace, slot.retirement.unwrap_or(role))?;
            }
        }
        directory.revalidate()
    }

    fn cloned(&self, namespace: &BoundProgressNamespace, role: Role) -> io::Result<Option<File>> {
        self.verify(namespace)?;
        self.named_slot(role)
            .map(|source| {
                self.slots[source.index()]
                    .original
                    .as_ref()
                    .ok_or_else(|| invalid("retained strict original is missing"))?
                    .try_clone()
            })
            .transpose()
    }

    fn create(&mut self, namespace: &BoundProgressNamespace, role: Role) -> io::Result<File> {
        self.verify(namespace)?;
        if self.replacement(role).is_some() {
            return Err(invalid("cannot create over a retained publication"));
        }
        let path = role.path(namespace);
        let slot = &mut self.slots[role.index()];
        if slot.original.is_some() && !slot.creator {
            return Err(invalid(
                "recovered strict artifact is not creator authority",
            ));
        }
        if slot.original.is_none() {
            self.mutation_started = true;
            slot.creator = true;
            slot.closed = false;
            slot.disposed = false;
            slot.retirement = None;
            // This slot is populated by the lower owner before its post-create checks.
            let created =
                parent(namespace)?.create_private_journal_into(leaf(&path)?, &mut slot.original);
            if let Some(file) = &slot.original {
                match iroha_fs::FileIdentity::of(file) {
                    Ok(identity) => slot.identity = Some(identity),
                    Err(error) if created.is_ok() => return Err(error),
                    Err(_) => {} // Keep the earlier create failure and its held slot.
                }
            }
            created?;
            self.observe(Step::Created(role))?;
        }
        self.verify(namespace)?;
        self.cloned(namespace, role)?
            .ok_or_else(|| invalid("created strict original is missing"))
    }

    fn settle_retirements(&mut self, namespace: &BoundProgressNamespace) -> io::Result<()> {
        for role in ROLES {
            let slot = &mut self.slots[role.index()];
            if slot.disposed && !slot.closed {
                let file = slot
                    .original
                    .as_ref()
                    .ok_or_else(|| invalid("disposed original missing"))?;
                Self::identity(slot, file)?;
                parent(namespace)?.require_retired_private_journal(file)?;
                // Record the tombstone before close; a later barrier failure cannot
                // confer authority to reopen or remove another file at this name.
                slot.closed = true;
                drop(slot.original.take());
                self.observe(Step::Closed(role))?;
            }
            if self.slots[role.index()].closed {
                let destination = self.slots[role.index()].retirement.unwrap_or(role);
                Self::absent(namespace, destination)?;
                Kura::sync_bound_progress_intent_directories(namespace)?;
            }
        }
        self.verify(namespace)
    }

    fn finish(&mut self, namespace: &BoundProgressNamespace) -> io::Result<()> {
        if !self.final_barrier_pending
            && !self
                .slots
                .iter()
                .any(|slot| slot.published || slot.disposed || slot.closed || slot.displaced_closed)
        {
            return self.verify(namespace);
        }
        self.final_barrier_pending = true;
        self.settle_retirements(namespace)?;
        self.verify(namespace)?;
        Kura::sync_bound_progress_intent_directories(namespace)?;
        self.observe(Step::Barriers)?;
        // Check every held displaced predecessor immediately before its one close.
        for destination in ROLES {
            if self.replacement(destination).is_some() {
                let slot = &mut self.slots[destination.index()];
                if let Some(file) = &slot.original {
                    Self::identity(slot, file)?;
                    parent(namespace)?.require_retired_private_journal(file)?;
                    slot.displaced_closed = true;
                }
                drop(slot.original.take());
            }
        }
        // The move below is infallible and retains the published source as the main.
        for source in ROLES {
            if !self.slots[source.index()].published {
                continue;
            }
            let destination = self.slots[source.index()]
                .destination
                .expect("publication has destination");
            let mut moved = std::mem::take(&mut self.slots[source.index()]);
            self.slots[source.index()].captured = true;
            moved.destination = None;
            moved.published = false;
            self.slots[destination.index()] = moved;
        }
        self.verify(namespace)?;
        Kura::sync_bound_progress_intent_directories(namespace)?;
        for slot in &mut self.slots {
            if slot.closed {
                *slot = Slot {
                    captured: true,
                    ..Slot::default()
                };
            }
        }
        self.verify(namespace)?;
        self.final_barrier_pending = false;
        Ok(())
    }
}

impl BoundProgressIo for NativeBoundProgressIo {
    fn optional(
        &mut self,
        _kura: &Kura,
        namespace: &BoundProgressNamespace,
        path: &Path,
    ) -> Result<Option<File>> {
        let result = Self::role(namespace, path).and_then(|role| self.cloned(namespace, role));
        result.map_err(|error| Error::IO(error, path.to_owned()))
    }

    fn main_file(
        &mut self,
        namespace: &BoundProgressNamespace,
        path: &Path,
        create: bool,
    ) -> io::Result<File> {
        let role = Self::role(namespace, path)?;
        if !matches!(role, Role::Data | Role::Index) {
            return Err(invalid("strict main-file operation names an artifact"));
        }
        if let Some(file) = self.cloned(namespace, role)? {
            return Ok(file);
        }
        if create {
            self.create(namespace, role)
        } else {
            Err(io::Error::new(
                ErrorKind::NotFound,
                "captured strict main file is absent",
            ))
        }
    }

    fn create_build(
        &mut self,
        namespace: &BoundProgressNamespace,
        path: &Path,
    ) -> io::Result<File> {
        if Self::role(namespace, path)? != Role::Build {
            return Err(invalid("strict intent creator names another role"));
        }
        self.create(namespace, Role::Build)
    }

    fn publish(
        &mut self,
        namespace: &BoundProgressNamespace,
        source: &Path,
        destination: &Path,
        original: &File,
        replace: bool,
    ) -> std::result::Result<(), BoundProgressPromotionError> {
        let mut published = false;
        let result = (|| {
            let source_role = Self::role(namespace, source)?;
            let destination_role = Self::role(namespace, destination)?;
            if !matches!(
                (source_role, destination_role, replace),
                (Role::Build, Role::Intent, false)
                    | (Role::RewriteData, Role::Data, true)
                    | (Role::RewriteIndex, Role::Index, true)
            ) {
                return Err(invalid("unreviewed strict publication transition"));
            }
            self.verify(namespace)?;
            let predecessor = self.slots[destination_role.index()].original.as_ref();
            if !replace && predecessor.is_some() {
                return Err(invalid("strict intent destination is occupied"));
            }
            let source_identity = self.slots[source_role.index()].identity;
            if source_identity != Some(iroha_fs::FileIdentity::of(original)?) {
                return Err(invalid("strict publication lost its source original"));
            }
            // Disjoint original slots are borrowed without moving either handle.
            let (source_slot, destination_slot) = if source_role.index() < destination_role.index()
            {
                let (left, right) = self.slots.split_at_mut(destination_role.index());
                (&mut left[source_role.index()], &mut right[0])
            } else {
                let (left, right) = self.slots.split_at_mut(source_role.index());
                (&mut right[0], &mut left[destination_role.index()])
            };
            let predecessor = destination_slot.original.as_ref();
            self.mutation_started = true;
            source_slot.destination = Some(destination_role);
            if source_slot.published {
                published = true;
                return parent(namespace)?.require_same_lock(leaf(destination)?, original);
            }
            #[cfg(test)]
            let fault = &mut self.fail_at;
            let result = parent(namespace)?.publish_private_journal_observed(
                leaf(source)?,
                leaf(destination)?,
                original,
                predecessor,
                &mut source_slot.published,
                |step| {
                    #[cfg(test)]
                    if matches!(step, iroha_fs::AtomicWriteStep::Published) {
                        observe_fault(fault, Step::Published(source_role))?;
                    }
                    #[cfg(not(test))]
                    let _ = step;
                    Ok(())
                },
            );
            published = source_slot.published;
            result
        })();
        let checked = self.verify(namespace);
        result
            .and(checked)
            .map_err(|source| BoundProgressPromotionError { published, source })
    }

    fn retire(&mut self, namespace: &BoundProgressNamespace, path: &Path) -> io::Result<()> {
        self.verify(namespace)?;
        let destination = Self::role(namespace, path)?;
        let Some(role) = self.named_slot(destination) else {
            self.settle_retirements(namespace)?;
            return Self::absent(namespace, destination);
        };
        let slot = &mut self.slots[role.index()];
        let file = slot
            .original
            .as_ref()
            .ok_or_else(|| invalid("strict retirement lost original"))?;
        self.mutation_started = true;
        slot.retirement = Some(destination);
        let result =
            parent(namespace)?.retire_private_journal(leaf(path)?, file, &mut slot.disposed);
        if result.is_ok() {
            self.observe(Step::Disposed(role))?;
        }
        result?;
        self.settle_retirements(namespace)
    }

    fn retain_failure(&mut self, failure: Error) {
        if self.first.is_none() {
            self.first = Some(BoundProgressIoFailure::Ordinary(failure));
        }
    }

    fn retain_classification(&mut self, failure: BoundProgressRecoveryFailure) {
        if self.first.is_none() {
            self.first = Some(BoundProgressIoFailure::Classified(failure));
        }
    }

    fn first_failure(&self) -> Option<BoundProgressRecoveryFailure> {
        self.first
            .as_ref()
            .map(BoundProgressIoFailure::classification)
    }

    fn take_failure(&mut self) -> Option<BoundProgressIoFailure> {
        self.first.take()
    }

    fn require_absent(
        &mut self,
        namespace: &BoundProgressNamespace,
        path: &Path,
    ) -> io::Result<()> {
        self.verify(namespace)?;
        let role = Self::role(namespace, path)?;
        if self.named_slot(role).is_some() {
            return Err(io::Error::new(
                ErrorKind::AlreadyExists,
                "strict recovery artifact remains",
            ));
        }
        Self::absent(namespace, role)
    }
}

impl<'a> NativeBoundProgressTransaction<'a> {
    /// Install the caller's retry owner before acquiring any mutable file original.
    pub(super) fn prepare_into(
        slot: &mut Option<Self>,
        kura: &'a Kura,
        data: &Path,
        index: &Path,
    ) -> Result<()> {
        if slot.is_none() {
            let namespace = kura.open_bound_progress_namespace(data, index)?;
            *slot = Some(Self {
                kura,
                namespace,
                storage: NativeBoundProgressIo::default(),
                pending: None,
            });
        }
        let transaction = slot.as_mut().expect("installed strict transaction");
        if !std::ptr::eq(transaction.kura, kura)
            || transaction.namespace.data_path != data
            || transaction.namespace.index_path != index
        {
            return Err(Error::IO(
                invalid("strict retry names another Kura or namespace"),
                data.to_owned(),
            ));
        }
        transaction
            .storage
            .capture(&transaction.namespace)
            .map_err(|error| Error::IO(error, data.to_owned()))
    }

    fn failed(&mut self, fallback: BoundProgressRecoveryFailure) -> BoundProgressIoFailure {
        self.storage
            .take_failure()
            .unwrap_or(BoundProgressIoFailure::Classified(fallback))
    }

    /// Reconcile exactly the captured originals through the shared canonical recovery.
    pub(super) fn recover(
        &mut self,
        kind: &str,
    ) -> std::result::Result<(), BoundProgressIoFailure> {
        self.storage
            .settle_retirements(&self.namespace)
            .map_err(|error| {
                BoundProgressIoFailure::Ordinary(Error::IO(
                    error,
                    self.namespace.index_path.clone(),
                ))
            })?;
        if !self
            .kura
            .recover_bound_progress_sidecar_artifacts_in_namespace_impl_with_io(
                &self.namespace,
                &self.namespace.data_path,
                &self.namespace.index_path,
                kind,
                &mut self.storage,
            )
        {
            let failure = self.storage.first_failure().unwrap_or_else(|| {
                self.kura.classify_bound_progress_recovery_failure_with_io(
                    &self.namespace,
                    &self.namespace.data_path,
                    &self.namespace.index_path,
                    kind,
                    &mut self.storage,
                )
            });
            return Err(self.failed(failure));
        }
        if let Some(first) = self.storage.take_failure() {
            return Err(first);
        }
        self.storage.finish(&self.namespace).map_err(|error| {
            BoundProgressIoFailure::Ordinary(Error::IO(error, self.namespace.index_path.clone()))
        })
    }

    /// Retry the same bounded request; a different pending request never gains authority.
    pub(super) fn append(
        &mut self,
        height: u64,
        initial_base_height: u64,
        payload: &[u8],
        kind: &str,
    ) -> std::result::Result<(), BoundProgressIoFailure> {
        if height == 0
            || height == u64::MAX
            || initial_base_height == 0
            || initial_base_height > height
            || payload.is_empty()
            || u64::try_from(payload.len()).map_or(true, |len| len > STRICT_INIT_MAX_BLOCK_BYTES)
        {
            return Err(BoundProgressIoFailure::Classified(
                BoundProgressRecoveryFailure::InvalidData,
            ));
        }
        if let Some(pending) = &self.pending {
            if (
                pending.height,
                pending.initial_base_height,
                pending.payload.as_slice(),
            ) != (height, initial_base_height, payload)
            {
                return Err(BoundProgressIoFailure::Classified(
                    BoundProgressRecoveryFailure::InvalidData,
                ));
            }
        } else {
            let mut owned = Vec::new();
            owned.try_reserve_exact(payload.len()).map_err(|_| {
                BoundProgressIoFailure::Classified(BoundProgressRecoveryFailure::RetryableIo)
            })?;
            owned.extend_from_slice(payload);
            self.pending = Some(PendingAppend {
                height,
                initial_base_height,
                payload: owned,
            });
        }
        self.recover(kind)?;
        // A completed recovery has discharged its earlier obligations. The shared
        // canonical algorithm below decides request validity before any new create.
        self.storage.mutation_started = false;
        let pending = self.pending.as_ref().expect("retained strict request");
        if !Kura::append_indexed_bound_progress_sidecar_with_io(
            &self.namespace.data_path,
            &self.namespace.index_path,
            pending.height,
            pending.initial_base_height,
            &pending.payload,
            kind,
            &self.namespace,
            &mut self.storage,
        ) {
            let failure = self.failed(BoundProgressRecoveryFailure::InvalidData);
            if !self.storage.mutation_started
                && matches!(
                    &failure,
                    BoundProgressIoFailure::Classified(BoundProgressRecoveryFailure::InvalidData)
                )
            {
                self.pending = None;
            }
            return Err(failure);
        }
        self.storage.finish(&self.namespace).map_err(|error| {
            BoundProgressIoFailure::Ordinary(Error::IO(error, self.namespace.index_path.clone()))
        })?;
        self.pending = None;
        Ok(())
    }
}

#[cfg(test)]
#[path = "native_bound_progress_transaction_tests.rs"]
mod tests;
