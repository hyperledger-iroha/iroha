//! Native pipeline originals retained across append, prepend and recovery failures.
//!
//! This is the existing best-effort pipeline format, not activation of the separate
//! descriptor-bound progress protocol. The caller owns the sidecar publication fence.
//! One fixed pipeline owner retains at most five file slots and one bounded pending
//! payload; it is neither a pathname cache nor a reservation against same-user renames.
//!
//! Generic rewrite creators share the canonical selector/emitter with path storage.
//! TODO: Migrate the separate strict bound progress append/intent owner before
//! enabling that protocol through a retained transaction.
//! Live pipeline and FASTPQ callers currently request no retention/rewrite creation.

use super::*;
use std::{ffi::OsStr, fs::File, io};

#[derive(Debug, Default)]
struct Originals {
    data: Option<File>,
    index: Option<File>,
    rewrite_data: Option<File>,
    rewrite_index: Option<File>,
    prepend: Option<File>,
    prepend_disposed: bool,
    disposed_prepend_identity: Option<iroha_fs::FileIdentity>,
    data_published: bool,
    index_published: bool,
    displaced_data_identity: Option<iroha_fs::FileIdentity>,
    displaced_index_identity: Option<iroha_fs::FileIdentity>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppendPhase {
    Prepare,
    PrependIndex,
    Payload,
    Index,
    Publish,
    Barriers,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RecoveryStep {
    DataPublished,
    IndexPublished,
}

#[derive(Debug)]
struct PendingAppend {
    height: u64,
    payload: Vec<u8>,
    sync: bool,
    phase: AppendPhase,
    absent_pair: bool,
    layout: Option<SidecarIndexLayout>,
    offset: u64,
    entry_position: u64,
    index_length: u64,
    published: bool,
    prepend: bool,
}

/// One concrete pipeline pair; retained on every ordinary failed attempt.
#[derive(Debug)]
pub(super) struct NativeSidecarTransaction {
    namespace: BoundProgressNamespace,
    originals: Originals,
    pending: Option<PendingAppend>,
    rewrite: Option<PendingRewrite>,
    retained_append: Option<CompletedRetainedAppend>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum RewriteRequest {
    Newest {
        retention: NonZeroUsize,
        pinned_height: Option<u64>,
    },
    #[cfg(test)]
    Window { retention: NonZeroUsize },
    Terminal {
        terminal_height: u64,
        retention: NonZeroUsize,
        required: BTreeSet<u64>,
    },
}

impl RewriteRequest {
    fn from_policy(policy: IndexedSidecarRewrite<'_>) -> Self {
        match policy {
            IndexedSidecarRewrite::Newest {
                retention,
                pinned_height,
            } => Self::Newest {
                retention,
                pinned_height,
            },
            #[cfg(test)]
            IndexedSidecarRewrite::NewestWindow { retention } => Self::Window { retention },
            IndexedSidecarRewrite::AfterTerminalFrontier {
                terminal_height,
                retention,
                required_heights,
            } => Self::Terminal {
                terminal_height,
                retention,
                required: required_heights.clone(),
            },
        }
    }
    fn required(&self) -> &BTreeSet<u64> {
        static EMPTY: std::sync::LazyLock<BTreeSet<u64>> = std::sync::LazyLock::new(BTreeSet::new);
        match self {
            Self::Terminal { required, .. } => required,
            _ => &EMPTY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RewritePhase {
    Building,
    Ready,
}

// A fixed-size retry key avoids retaining a second bounded payload copy.
#[derive(Debug, PartialEq, Eq)]
struct CompletedRetainedAppend {
    height: u64,
    payload_len: usize,
    payload_hash: Hash,
    sync: bool,
    retention: NonZeroUsize,
}

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_REWRITE_INDEX_CREATE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(test)]
pub(super) fn set_rewrite_index_create_failure_for_tests(fail: bool) {
    FAIL_NEXT_REWRITE_INDEX_CREATE.with(|flag| flag.set(fail));
}

#[derive(Debug)]
struct PendingRewrite {
    request: RewriteRequest,
    phase: RewritePhase,
}

impl IndexedSidecarRewriteIo for NativeSidecarTransaction {
    fn open_index(&mut self) -> io::Result<File> {
        self.verify()?;
        original(&self.originals.index)?.try_clone()
    }
    fn open_data(&mut self) -> io::Result<File> {
        self.verify()?;
        original(&self.originals.data)?.try_clone()
    }
    fn create_data(&mut self) -> io::Result<File> {
        self.verify()?;
        let path = self.namespace.data_path.with_extension("norito.tmp");
        create(&self.namespace, &path, &mut self.originals.rewrite_data)?;
        let mut file = original(&self.originals.rewrite_data)?;
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        file.try_clone()
    }
    fn create_index(&mut self) -> io::Result<File> {
        self.verify()?;
        #[cfg(test)]
        if FAIL_NEXT_REWRITE_INDEX_CREATE.with(|flag| flag.replace(false)) {
            return Err(io::Error::other(
                "injected native rewrite index creation failure",
            ));
        }
        let path = self.namespace.index_path.with_extension("index.tmp");
        create(&self.namespace, &path, &mut self.originals.rewrite_index)?;
        let mut file = original(&self.originals.rewrite_index)?;
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        file.try_clone()
    }
    fn publish(&mut self) -> bool {
        let Some(pending) = self.rewrite.as_mut() else {
            return false;
        };
        // The shared writer has flushed and synchronized both complete images.
        // The recovered-index protocol may only start after this explicit boundary.
        pending.phase = RewritePhase::Ready;
        self.recover().is_ok()
    }
}

fn invalid_state(message: &'static str) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, message)
}

fn owner(namespace: &BoundProgressNamespace) -> io::Result<&iroha_fs::OwnerDirectory> {
    if !Kura::progress_mutation_namespace_unchanged(namespace) {
        return Err(invalid_state("native pipeline namespace changed"));
    }
    namespace
        .directories
        .first()
        .map(|directory| &directory.native_owner)
        .ok_or_else(|| invalid_state("native pipeline namespace is empty"))
}

fn leaf(path: &Path) -> io::Result<&OsStr> {
    path.file_name()
        .ok_or_else(|| invalid_state("native pipeline path has no leaf"))
}

fn create(
    namespace: &BoundProgressNamespace,
    path: &Path,
    slot: &mut Option<File>,
) -> io::Result<()> {
    if slot.is_none() {
        owner(namespace)?.create_private_journal_into(leaf(path)?, slot)?;
    }
    owner(namespace)?.require_same_lock(leaf(path)?, slot.as_ref().expect("created slot"))
}

fn original(slot: &Option<File>) -> io::Result<&File> {
    slot.as_ref()
        .ok_or_else(|| invalid_state("native pipeline original is missing"))
}

fn check(namespace: &BoundProgressNamespace, path: &Path, file: &File) -> io::Result<()> {
    owner(namespace)?.require_same_lock(leaf(path)?, file)
}

// A confirmed POSIX replacement leaves the same admitted predecessor delete-pending
// with zero links. Keep it under private original custody until its explicit final close.
fn check_displaced(
    namespace: &BoundProgressNamespace,
    file: Option<&File>,
    expected: Option<iroha_fs::FileIdentity>,
) -> io::Result<()> {
    match (file, expected) {
        (None, None) => Ok(()),
        (Some(file), Some(expected)) => {
            owner(namespace)?.require_retired_private_journal(file)?;
            if iroha_fs::FileIdentity::of(file)? != expected {
                return Err(invalid_state("displaced native sidecar original changed"));
            }
            Ok(())
        }
        _ => Err(invalid_state(
            "displaced native sidecar identity is missing",
        )),
    }
}

fn barriers(namespace: &BoundProgressNamespace, data: &File, index: &File) -> io::Result<()> {
    sync_indexed_sidecar_data(data)?;
    sync_indexed_sidecar_index(index)?;
    if !Kura::sync_bound_progress_mutation_directories(namespace, "native pipeline") {
        return Err(io::Error::other("native pipeline directory barrier failed"));
    }
    Ok(())
}

impl NativeSidecarTransaction {
    pub(super) fn capture(namespace: BoundProgressNamespace) -> io::Result<Self> {
        let directory = owner(&namespace)?;
        let open = |path: &Path| directory.open_private_journal_optional(leaf(path)?);
        let originals = Originals {
            data: open(&namespace.data_path)?,
            index: open(&namespace.index_path)?,
            rewrite_data: open(&namespace.data_path.with_extension("norito.tmp"))?,
            rewrite_index: open(&namespace.index_path.with_extension("index.tmp"))?,
            prepend: open(&namespace.index_path.with_extension("index.prepend.tmp"))?,
            ..Originals::default()
        };
        // The strict bound protocol owns these names. The ordinary pipeline must
        // not interpret or discard its authenticated intent as generic residue.
        for path in [
            Kura::bound_progress_append_build_path(&namespace.index_path),
            Kura::bound_progress_append_intent_path(&namespace.index_path),
        ] {
            if directory.open_regular_optional(leaf(&path)?)?.is_some() {
                return Err(invalid_state(
                    "native pipeline found a strict progress intent",
                ));
            }
        }
        Ok(Self {
            namespace,
            originals,
            pending: None,
            rewrite: None,
            retained_append: None,
        })
    }

    pub(super) fn matches(&self, data: &Path, index: &Path) -> bool {
        self.namespace.data_path == data && self.namespace.index_path == index
    }

    fn verify(&self) -> io::Result<()> {
        let files = &self.originals;
        let data = if files.data_published {
            &files.rewrite_data
        } else {
            &files.data
        };
        let index = if files.index_published {
            &files.rewrite_index
        } else if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.published)
        {
            &files.prepend
        } else {
            &files.index
        };
        for (path, file) in [
            (&self.namespace.data_path, data),
            (&self.namespace.index_path, index),
        ] {
            if let Some(file) = file {
                check(&self.namespace, path, file)?;
            } else if owner(&self.namespace)?
                .open_regular_optional(leaf(path)?)?
                .is_some()
            {
                return Err(invalid_state("captured native pipeline absence changed"));
            }
        }
        for (path, file, moved) in [
            (
                self.namespace.data_path.with_extension("norito.tmp"),
                &files.rewrite_data,
                files.data_published,
            ),
            (
                self.namespace.index_path.with_extension("index.tmp"),
                &files.rewrite_index,
                files.index_published,
            ),
            (
                self.namespace
                    .index_path
                    .with_extension("index.prepend.tmp"),
                &files.prepend,
                self.pending
                    .as_ref()
                    .is_some_and(|pending| pending.published)
                    || files.prepend_disposed,
            ),
        ] {
            if !moved && let Some(file) = file {
                check(&self.namespace, &path, file)?;
            }
        }
        if files.prepend_disposed
            && let Some(file) = &files.prepend
        {
            owner(&self.namespace)?.require_retired_private_journal(file)?;
            if Some(iroha_fs::FileIdentity::of(file)?) != files.disposed_prepend_identity {
                return Err(invalid_state("retiring native prepend original changed"));
            }
        }
        if files.data_published {
            check_displaced(
                &self.namespace,
                files.data.as_ref(),
                files.displaced_data_identity,
            )?;
        }
        if files.index_published
            || self
                .pending
                .as_ref()
                .is_some_and(|pending| pending.published)
        {
            check_displaced(
                &self.namespace,
                files.index.as_ref(),
                files.displaced_index_identity,
            )?;
        }
        owner(&self.namespace)?.revalidate()
    }

    /// Execute the one shared policy/emitter against retained creator originals.
    pub(super) fn rewrite(
        &mut self,
        policy: IndexedSidecarRewrite<'_>,
        kind: &str,
    ) -> io::Result<()> {
        let request = RewriteRequest::from_policy(policy);
        if let Some(pending) = &self.rewrite {
            if pending.request != request {
                return Err(invalid_state("native rewrite retry changed its request"));
            }
            if pending.phase == RewritePhase::Ready {
                return self.recover_required(request.required());
            }
        } else {
            self.recover_required(request.required())?;
            if self.originals.data.is_none()
                && self.originals.index.is_none()
                && !matches!(request, RewriteRequest::Newest { .. })
                && request.required().is_empty()
            {
                return Ok(());
            }
            self.rewrite = Some(PendingRewrite {
                request: request.clone(),
                phase: RewritePhase::Building,
            });
        }
        self.verify()?;
        let data_path = self.namespace.data_path.clone();
        let index_path = self.namespace.index_path.clone();
        let success =
            Kura::rewrite_indexed_sidecars_with_io(&data_path, &index_path, policy, kind, self);
        if success {
            self.rewrite = None;
            self.verify()
        } else {
            // A rejected policy before either CREATE_NEW is not a retry obligation.
            if self
                .rewrite
                .as_ref()
                .is_some_and(|pending| pending.phase == RewritePhase::Building)
                && self.originals.rewrite_data.is_none()
                && self.originals.rewrite_index.is_none()
            {
                self.rewrite = None;
            }
            Err(io::Error::other("native shared rewrite did not complete"))
        }
    }

    /// Complete the original attempt before allowing a later request to mutate the pair.
    pub(super) fn recover(&mut self) -> io::Result<()> {
        self.recover_required(&BTreeSet::new())
    }

    pub(super) fn recover_required(&mut self, required: &BTreeSet<u64>) -> io::Result<()> {
        let mut joined = required.clone();
        if let Some(pending) = &self.rewrite {
            if pending.phase == RewritePhase::Building {
                return Err(invalid_state(
                    "native rewrite creator must resume the same request",
                ));
            }
            joined.extend(pending.request.required().iter().copied());
        }
        self.recover_observed(&joined, &mut |_| Ok(()))?;
        self.rewrite = None;
        Ok(())
    }

    fn recover_observed(
        &mut self,
        required_heights: &BTreeSet<u64>,
        observe: &mut impl FnMut(RecoveryStep) -> io::Result<()>,
    ) -> io::Result<()> {
        if self
            .rewrite
            .as_ref()
            .is_some_and(|pending| pending.phase == RewritePhase::Building)
        {
            return Err(invalid_state(
                "native partial rewrite is not a recovery marker",
            ));
        }
        self.verify()?;
        self.finish_append()?;
        self.retire_unpublished_prepend()?;
        if self.originals.rewrite_index.is_none() {
            if self.originals.rewrite_data.is_some() {
                return Err(invalid_state("native rewrite payload has no index marker"));
            }
            if !required_heights.is_empty() {
                validate_index(
                    original(&self.originals.data)?,
                    original(&self.originals.index)?,
                    required_heights,
                )?;
            }
            return self.verify();
        }
        let files = &mut self.originals;
        let data = if files.rewrite_data.is_some() {
            original(&files.rewrite_data)?
        } else {
            original(&files.data)?
        };
        let index = original(&files.rewrite_index)?;
        validate_index(data, index, required_heights)?;
        sync_indexed_sidecar_data(data)?;
        sync_indexed_sidecar_index(index)?;
        // Preserve the same marker-before-data-before-index ordering as the
        // generic recovery protocol, through retained directory authorities.
        sync_marker(&self.namespace)?;
        if files.rewrite_data.is_some() && !files.data_published {
            files.displaced_data_identity = files
                .data
                .as_ref()
                .map(iroha_fs::FileIdentity::of)
                .transpose()?;
            let from = self.namespace.data_path.with_extension("norito.tmp");
            owner(&self.namespace)?.publish_private_journal_observed(
                leaf(&from)?,
                leaf(&self.namespace.data_path)?,
                original(&files.rewrite_data)?,
                files.data.as_ref(),
                &mut files.data_published,
                |_| Ok(()),
            )?;
            observe(RecoveryStep::DataPublished)?;
        }
        sync_promotion(&self.namespace)?;
        if !files.index_published {
            files.displaced_index_identity = files
                .index
                .as_ref()
                .map(iroha_fs::FileIdentity::of)
                .transpose()?;
            let from = self.namespace.index_path.with_extension("index.tmp");
            owner(&self.namespace)?.publish_private_journal_observed(
                leaf(&from)?,
                leaf(&self.namespace.index_path)?,
                original(&files.rewrite_index)?,
                files.index.as_ref(),
                &mut files.index_published,
                |_| Ok(()),
            )?;
            observe(RecoveryStep::IndexPublished)?;
        }
        sync_promotion(&self.namespace)?;
        self.verify()?;
        if self.originals.data_published {
            check_displaced(
                &self.namespace,
                self.originals.data.as_ref(),
                self.originals.displaced_data_identity,
            )?;
            self.originals.data = self.originals.rewrite_data.take();
            self.originals.data_published = false;
            self.originals.displaced_data_identity = None;
        }
        check_displaced(
            &self.namespace,
            self.originals.index.as_ref(),
            self.originals.displaced_index_identity,
        )?;
        self.originals.index = self.originals.rewrite_index.take();
        self.originals.index_published = false;
        self.originals.displaced_index_identity = None;
        self.verify()
    }

    /// Retain the completed append boundary if its requested rewrite must resume.
    /// Live pipeline callers currently pass no retention; component adapters exercise
    /// the same creator without enabling a new production retention policy.
    pub(super) fn append_with_retention(
        &mut self,
        height: u64,
        payload: &[u8],
        sync: bool,
        retention: Option<NonZeroUsize>,
        kind: &str,
    ) -> io::Result<()> {
        let Some(retention) = retention else {
            return self.append(height, payload, sync);
        };
        let request = CompletedRetainedAppend {
            height,
            payload_len: payload.len(),
            payload_hash: Hash::new(payload),
            sync,
            retention,
        };
        if let Some(completed) = &self.retained_append {
            if completed != &request {
                return Err(invalid_state(
                    "native append-retention retry changed its request",
                ));
            }
        } else {
            self.append(height, payload, sync)?;
            // The append is complete. A creator failure must never send this request
            // back through append/recovery while its temporary is still Building.
            self.retained_append = Some(request);
        }
        let result = self.rewrite(
            IndexedSidecarRewrite::Newest {
                retention,
                pinned_height: None,
            },
            kind,
        );
        if result.is_ok() || self.rewrite.is_none() {
            self.retained_append = None;
        }
        result
    }

    pub(super) fn append(&mut self, height: u64, payload: &[u8], sync: bool) -> io::Result<()> {
        if self.retained_append.is_some() {
            return Err(invalid_state(
                "native append must finish its retained rewrite",
            ));
        }
        if height == 0
            || height == u64::MAX
            || u64::try_from(payload.len())
                .ok()
                .is_none_or(|len| len > STRICT_INIT_MAX_BLOCK_BYTES)
        {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "invalid native sidecar payload",
            ));
        }
        self.recover()?;
        let absent_pair = self.originals.data.is_none() && self.originals.index.is_none();
        if self.originals.data.is_none() != self.originals.index.is_none() {
            return Err(invalid_state(
                "native pipeline pair is only partially present",
            ));
        }
        self.admit_append_request(height, payload.len())?;
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(payload.len())
            .map_err(io::Error::other)?;
        retained.extend_from_slice(payload);
        // Install the operation before either CREATE_NEW may publish a name.
        self.pending = Some(PendingAppend {
            height,
            payload: retained,
            sync,
            phase: AppendPhase::Prepare,
            absent_pair,
            layout: None,
            offset: 0,
            entry_position: 0,
            index_length: 0,
            published: false,
            prepend: false,
        });
        self.finish_append()
    }

    /// Reject a malformed request before it becomes a persistent retry obligation.
    /// This observes existing originals only; creation, tail repair and writes remain
    /// in the installed pending phase, whose ordinary failures must retain ownership.
    fn admit_append_request(&self, height: u64, payload_len: usize) -> io::Result<()> {
        let Some(data) = self.originals.data.as_ref() else {
            return Ok(());
        };
        let mut index = original(&self.originals.index)?.try_clone()?;
        let index_length = index.metadata()?.len();
        let layout =
            SidecarIndexLayout::read_from(&mut index, index_length).map_err(invalid_state)?;
        data.metadata()?
            .len()
            .checked_add(payload_len as u64)
            .ok_or_else(|| invalid_state("native sidecar payload extent overflow"))?;
        if layout.entry_position(height).is_some() {
            return Ok(());
        }
        if height < layout.base_height {
            let added = layout.base_height - height;
            if added > MAX_INDEXED_SIDECAR_GAP_ENTRIES {
                return Err(invalid_state("native prepend gap exceeds limit"));
            }
            layout
                .entry_count
                .checked_add(added)
                .and_then(|count| count.checked_mul(PIPELINE_INDEX_ENTRY_SIZE_U64))
                .and_then(|length| length.checked_add(INDEXED_SIDECAR_BASE_HEADER_SIZE_U64))
                .ok_or_else(|| invalid_state("native prepend length overflow"))?;
        } else {
            let next = layout
                .next_height()
                .ok_or_else(|| invalid_state("native sidecar height overflow"))?;
            let gap = height
                .checked_sub(next)
                .ok_or_else(|| invalid_state("native sidecar base mismatch"))?;
            if gap > MAX_INDEXED_SIDECAR_GAP_ENTRIES {
                return Err(invalid_state("native append gap exceeds limit"));
            }
            gap.checked_add(1)
                .and_then(|count| count.checked_mul(PIPELINE_INDEX_ENTRY_SIZE_U64))
                .and_then(|length| length.checked_add(layout.aligned_len))
                .ok_or_else(|| invalid_state("native sidecar length overflow"))?;
        }
        Ok(())
    }

    fn finish_append(&mut self) -> io::Result<()> {
        let Some(pending) = self.pending.as_mut() else {
            return Ok(());
        };
        drive_append(&self.namespace, &mut self.originals, pending)?;
        self.verify()?;
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.published)
        {
            check_displaced(
                &self.namespace,
                self.originals.index.as_ref(),
                self.originals.displaced_index_identity,
            )?;
            self.originals.index = self.originals.prepend.take();
            self.originals.displaced_index_identity = None;
        }
        self.pending = None;
        self.verify()
    }

    fn retire_unpublished_prepend(&mut self) -> io::Result<()> {
        let files = &mut self.originals;
        if files.prepend.is_none() && !files.prepend_disposed {
            return Ok(());
        }
        let path = self
            .namespace
            .index_path
            .with_extension("index.prepend.tmp");
        // A still-named prepend temporary was never the published main index.
        // Validate the authoritative old pair before retiring this admitted original.
        validate_index(
            original(&files.data)?,
            original(&files.index)?,
            &BTreeSet::new(),
        )?;
        if !files.prepend_disposed {
            let original = original(&files.prepend)?;
            files.disposed_prepend_identity = Some(iroha_fs::FileIdentity::of(original)?);
            owner(&self.namespace)?.retire_private_journal(
                leaf(&path)?,
                original,
                &mut files.prepend_disposed,
            )?;
        }
        if let Some(original) = &files.prepend {
            owner(&self.namespace)?.require_retired_private_journal(original)?;
            if Some(iroha_fs::FileIdentity::of(original)?) != files.disposed_prepend_identity {
                return Err(invalid_state("disposed prepend original changed"));
            }
            owner(&self.namespace)?.revalidate()?;
            owner(&self.namespace)?.sync()?;
            // Explicit final close only after disposition and custody succeeded.
            // Keep the disposition fact if a subsequent absence/barrier check fails.
            files.prepend = None;
        }
        if owner(&self.namespace)?
            .open_regular_optional(leaf(&path)?)?
            .is_some()
        {
            return Err(invalid_state("retired prepend name was replaced"));
        }
        owner(&self.namespace)?.sync()?;
        files.prepend_disposed = false;
        files.disposed_prepend_identity = None;
        Ok(())
    }
}

fn drive_append(
    namespace: &BoundProgressNamespace,
    files: &mut Originals,
    pending: &mut PendingAppend,
) -> io::Result<()> {
    if pending.phase == AppendPhase::Prepare {
        if pending.absent_pair {
            create(namespace, &namespace.data_path, &mut files.data)?;
            create(namespace, &namespace.index_path, &mut files.index)?;
            let mut index = original(&files.index)?;
            index.seek(SeekFrom::Start(0))?;
            index.write_all(&SidecarIndexLayout::base_header(pending.height))?;
            index.set_len(INDEXED_SIDECAR_BASE_HEADER_SIZE_U64)?;
        }
        let data = original(&files.data)?;
        let mut index = original(&files.index)?.try_clone()?;
        let index_length = index.metadata()?.len();
        let layout =
            SidecarIndexLayout::read_from(&mut index, index_length).map_err(invalid_state)?;
        index.set_len(layout.aligned_len)?;
        if !Kura::repair_unindexed_sidecar_tail(
            data,
            &mut index,
            layout,
            &namespace.data_path,
            &namespace.index_path,
            "native pipeline",
        ) {
            return Err(invalid_state("native pipeline tail repair failed"));
        }
        pending.offset = data.metadata()?.len();
        pending
            .offset
            .checked_add(pending.payload.len() as u64)
            .ok_or_else(|| invalid_state("native sidecar payload extent overflow"))?;
        pending.layout = Some(layout);
        if let Some(position) = layout.entry_position(pending.height) {
            let mut bytes = [0; PIPELINE_INDEX_ENTRY_SIZE];
            index.seek(SeekFrom::Start(position))?;
            index.read_exact(&mut bytes)?;
            let entry = SidecarIndexEntry::from_bytes(bytes);
            if entry.len > 0
                && entry.len == pending.payload.len() as u64
                && entry
                    .offset
                    .checked_add(entry.len)
                    .is_some_and(|end| end <= pending.offset)
            {
                let mut file = data;
                file.seek(SeekFrom::Start(entry.offset))?;
                let mut buffer = [0_u8; 8192];
                let mut equal = true;
                for expected in pending.payload.chunks(buffer.len()) {
                    file.read_exact(&mut buffer[..expected.len()])?;
                    equal &= buffer[..expected.len()] == *expected;
                }
                if equal {
                    pending.phase = AppendPhase::Barriers;
                }
            }
            pending.entry_position = position;
            pending.index_length = layout.aligned_len;
        } else if pending.height < layout.base_height {
            let added = layout.base_height - pending.height;
            if added > MAX_INDEXED_SIDECAR_GAP_ENTRIES {
                return Err(invalid_state("native prepend gap exceeds limit"));
            }
            pending.prepend = true;
            pending.index_length = layout
                .entry_count
                .checked_add(added)
                .and_then(|count| count.checked_mul(PIPELINE_INDEX_ENTRY_SIZE_U64))
                .and_then(|len| len.checked_add(INDEXED_SIDECAR_BASE_HEADER_SIZE_U64))
                .ok_or_else(|| invalid_state("native prepend length overflow"))?;
        } else {
            let next = layout
                .next_height()
                .ok_or_else(|| invalid_state("native sidecar height overflow"))?;
            let gap = pending
                .height
                .checked_sub(next)
                .ok_or_else(|| invalid_state("native sidecar base mismatch"))?;
            if gap > MAX_INDEXED_SIDECAR_GAP_ENTRIES {
                return Err(invalid_state("native append gap exceeds limit"));
            }
            pending.entry_position = gap
                .checked_mul(PIPELINE_INDEX_ENTRY_SIZE_U64)
                .and_then(|len| layout.aligned_len.checked_add(len))
                .ok_or_else(|| invalid_state("native sidecar entry overflow"))?;
            pending.index_length = pending
                .entry_position
                .checked_add(PIPELINE_INDEX_ENTRY_SIZE_U64)
                .ok_or_else(|| invalid_state("native sidecar length overflow"))?;
        }
        if pending.phase == AppendPhase::Prepare {
            pending.phase = if pending.prepend {
                AppendPhase::PrependIndex
            } else {
                AppendPhase::Payload
            };
        }
    }
    if pending.phase == AppendPhase::PrependIndex {
        let layout = pending
            .layout
            .ok_or_else(|| invalid_state("native prepend layout missing"))?;
        let entry = SidecarIndexEntry {
            offset: pending.offset,
            len: pending.payload.len() as u64,
        };
        let path = namespace.index_path.with_extension("index.prepend.tmp");
        create(namespace, &path, &mut files.prepend)?;
        let mut output = original(&files.prepend)?;
        output.seek(SeekFrom::Start(0))?;
        output.write_all(&SidecarIndexLayout::base_header(pending.height))?;
        output.write_all(&entry.to_bytes())?;
        let gap = layout.base_height - pending.height - 1;
        zero_entries(&mut output, gap)?;
        let length = layout
            .entry_count
            .checked_mul(PIPELINE_INDEX_ENTRY_SIZE_U64)
            .ok_or_else(|| invalid_state("native old index extent overflow"))?;
        let mut source = original(&files.index)?;
        check(namespace, &namespace.index_path, source)?;
        source.seek(SeekFrom::Start(layout.entries_offset))?;
        if io::copy(&mut source.take(length), &mut output)? != length {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        output.set_len(pending.index_length)?;
        output.flush()?;
        if pending.sync {
            sync_indexed_sidecar_index(output)?;
        }
        check(namespace, &path, output)?;
        pending.phase = AppendPhase::Payload;
    }
    if pending.phase == AppendPhase::Payload {
        let mut data = original(&files.data)?;
        check(namespace, &namespace.data_path, data)?;
        let result = (|| {
            data.seek(SeekFrom::Start(pending.offset))?;
            data.write_all(&pending.payload)?;
            data.flush()?;
            if pending.sync {
                sync_indexed_sidecar_initial_data(data)?;
            }
            Ok(())
        })();
        if let Err(error) = result {
            // Preserve the ordinary unindexed-payload rollback, using the same
            // original. A rollback failure never replaces the first error.
            let _ = rollback_unindexed_sidecar_payload(
                data,
                pending.offset,
                &namespace.data_path,
                "native pipeline",
            );
            return Err(error);
        }
        pending.phase = if pending.prepend {
            AppendPhase::Publish
        } else {
            AppendPhase::Index
        };
    }
    if pending.phase == AppendPhase::Index {
        let layout = pending
            .layout
            .ok_or_else(|| invalid_state("native append layout missing"))?;
        let entry = SidecarIndexEntry {
            offset: pending.offset,
            len: pending.payload.len() as u64,
        };
        let mut index = original(&files.index)?;
        check(namespace, &namespace.index_path, index)?;
        if pending.entry_position > layout.aligned_len {
            index.seek(SeekFrom::Start(layout.aligned_len))?;
            zero_entries(
                &mut index,
                (pending.entry_position - layout.aligned_len) / PIPELINE_INDEX_ENTRY_SIZE_U64,
            )?;
        }
        index.seek(SeekFrom::Start(pending.entry_position))?;
        index.write_all(&entry.to_bytes())?;
        index.set_len(pending.index_length)?;
        index.flush()?;
        pending.phase = AppendPhase::Barriers;
    }
    if pending.phase == AppendPhase::Publish {
        if !pending.published {
            files.displaced_index_identity = files
                .index
                .as_ref()
                .map(iroha_fs::FileIdentity::of)
                .transpose()?;
            let path = namespace.index_path.with_extension("index.prepend.tmp");
            owner(namespace)?.publish_private_journal_observed(
                leaf(&path)?,
                leaf(&namespace.index_path)?,
                original(&files.prepend)?,
                files.index.as_ref(),
                &mut pending.published,
                |_| Ok(()),
            )?;
        }
        pending.phase = AppendPhase::Barriers;
    }
    let data = original(&files.data)?;
    let index = if pending.published {
        original(&files.prepend)?
    } else {
        original(&files.index)?
    };
    check(namespace, &namespace.data_path, data)?;
    check(namespace, &namespace.index_path, index)?;
    if pending.sync {
        barriers(namespace, data, index)?;
    }
    owner(namespace)?.revalidate()
}

fn zero_entries(output: &mut &File, count: u64) -> io::Result<()> {
    if count > MAX_INDEXED_SIDECAR_GAP_ENTRIES {
        return Err(invalid_state("native sidecar filler exceeds limit"));
    }
    for _ in 0..count {
        output.write_all(&[0; PIPELINE_INDEX_ENTRY_SIZE])?;
    }
    Ok(())
}

fn validate_index(data: &File, index: &File, required_heights: &BTreeSet<u64>) -> io::Result<()> {
    let mut index = index.try_clone()?;
    let length = index.metadata()?.len();
    let layout = SidecarIndexLayout::read_from(&mut index, length).map_err(invalid_state)?;
    let data_length = data.metadata()?.len();
    if layout.aligned_len != length || (layout.entry_count == 0 && data_length != 0) {
        return Err(invalid_state("native rewrite index extent is invalid"));
    }
    index.seek(SeekFrom::Start(layout.entries_offset))?;
    let mut found = 0usize;
    for position in 0..layout.entry_count {
        let mut bytes = [0; PIPELINE_INDEX_ENTRY_SIZE];
        index.read_exact(&mut bytes)?;
        let entry = SidecarIndexEntry::from_bytes(bytes);
        if entry.len != 0
            && (entry.len > STRICT_INIT_MAX_BLOCK_BYTES
                || entry
                    .offset
                    .checked_add(entry.len)
                    .is_none_or(|end| end > data_length))
        {
            return Err(invalid_state("native rewrite entry is outside its payload"));
        }
        let height = layout
            .base_height
            .checked_add(position)
            .ok_or_else(|| invalid_state("native rewrite height overflow"))?;
        if entry.len > 0 && required_heights.contains(&height) {
            found += 1;
        }
    }
    if found != required_heights.len() {
        return Err(invalid_state("native rewrite omits required evidence"));
    }
    Ok(())
}

fn sync_marker(namespace: &BoundProgressNamespace) -> io::Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_SIDECAR_TEMP_MARKER_DIR_SYNC.with(|flag| flag.replace(false)) {
        return Err(io::Error::other(
            "injected sidecar temp marker directory sync failure",
        ));
    }
    owner(namespace)?.sync()
}

fn sync_promotion(namespace: &BoundProgressNamespace) -> io::Result<()> {
    #[cfg(test)]
    if FAIL_NEXT_SIDECAR_PROMOTION_DIR_SYNC.with(|flag| flag.replace(false)) {
        return Err(io::Error::other(
            "injected sidecar promotion directory sync failure",
        ));
    }
    owner(namespace)?.sync()
}

fn ensure_pipeline_child(root: &iroha_fs::OwnerDirectory) -> io::Result<iroha_fs::OwnerDirectory> {
    root.revalidate()?;
    let result = root.ensure_child(PIPELINE_DIR_NAME);
    let final_check = root.revalidate();
    match result {
        Err(error) => Err(error),
        Ok(child) => {
            final_check?;
            child.revalidate()?;
            Ok(child)
        }
    }
}

impl Kura {
    /// Create only the fixed pipeline child of the existing canonical block directory.
    /// The returned owner keeps its parent ancestry live throughout the caller.
    pub(super) fn ensure_native_pipeline_directory(
        &self,
        expected: &Path,
    ) -> io::Result<iroha_fs::OwnerDirectory> {
        if expected != Self::canonical_storage_path(&self.store_root).join(PIPELINE_DIR_NAME)
            || !self.instance_identity().matches(self)
            || !self.bound_storage_directory_unchanged(&self.store_root_directory)
        {
            return Err(invalid_state(
                "native pipeline directory is outside its original root",
            ));
        }
        let root = &self.store_root_directory.native_owner;
        // Canonical storage is established by Kura's constructor. Open these
        // exact existing ancestors; a missing parent is never provisioned here.
        let result = root.open_child("blocks").and_then(|blocks| {
            let result = blocks
                .open_child("canonical")
                .and_then(|canonical| ensure_pipeline_child(&canonical));
            let final_check = blocks.revalidate();
            match result {
                Ok(child) => final_check.map(|()| child),
                Err(error) => Err(error),
            }
        });
        let root_check = root.revalidate();
        let binding_check = if self.bound_storage_directory_unchanged(&self.store_root_directory) {
            Ok(())
        } else {
            Err(invalid_state(
                "native pipeline root changed during child admission",
            ))
        };
        match result {
            Ok(child) => {
                let child_check = child.revalidate();
                root_check
                    .and(binding_check)
                    .and(child_check)
                    .map(|()| child)
            }
            Err(error) => Err(error),
        }
    }

    /// Run one live pipeline mutation under its existing publication fence.
    /// Pending originals survive ordinary errors; success closes them before readers reopen.
    pub(super) fn append_native_pipeline(
        &self,
        data: &Path,
        index: &Path,
        height: u64,
        payload: &[u8],
        sync: bool,
    ) -> bool {
        self.with_native_pipeline(data, index, |owner| {
            owner.append_with_retention(height, payload, sync, None, "native pipeline")
        })
    }

    pub(super) fn recover_native_pipeline(&self, data: &Path, index: &Path) -> bool {
        self.with_native_pipeline(data, index, NativeSidecarTransaction::recover)
    }

    pub(super) fn native_pipeline_pending(&self) -> bool {
        self.native_pipeline_publication.lock().is_some()
    }

    fn with_native_pipeline(
        &self,
        data: &Path,
        index: &Path,
        action: impl FnOnce(&mut NativeSidecarTransaction) -> io::Result<()>,
    ) -> bool {
        let mut slot = self.native_pipeline_publication.lock();
        if slot.is_none() {
            let namespace = match self.open_bound_progress_namespace(data, index) {
                Ok(namespace) => namespace,
                Err(error) => {
                    warn!(?error, "failed to retain native pipeline namespace");
                    return false;
                }
            };
            match NativeSidecarTransaction::capture(namespace) {
                Ok(owner) => *slot = Some(owner),
                Err(error) => {
                    warn!(?error, "failed to retain native pipeline originals");
                    return false;
                }
            }
        }
        let owner = slot.as_mut().expect("installed native pipeline owner");
        if !owner.matches(data, index) {
            warn!("pending native pipeline belongs to a different pair");
            return false;
        }
        match action(owner) {
            Ok(()) => {
                *slot = None;
                true
            }
            Err(error) => {
                warn!(?error, "native pipeline attempt retains its original files");
                false
            }
        }
    }
}

#[cfg(test)]
#[path = "native_sidecar_transaction_tests.rs"]
mod tests;
