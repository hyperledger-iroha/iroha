//! Retained secure filesystem implementation for canonical proof tooling.

use super::*;
use rustix::fs::{AtFlags, FileType, Mode, OFlags, RenameFlags, Stat};
use std::io::{Read as _, Seek as _, SeekFrom, Write as _};
use std::{
    ffi::OsString,
    fs::File,
    os::unix::{ffi::OsStrExt as _, fs::FileExt as _},
    path::{Component, Path, PathBuf},
};

const DIRECTORY_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::DIRECTORY)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::NONBLOCK)
    .union(OFlags::CLOEXEC);
const FILE_FLAGS: OFlags = OFlags::RDWR
    .union(OFlags::CREATE)
    .union(OFlags::EXCL)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::NONBLOCK)
    .union(OFlags::CLOEXEC);
const MAX_PATH_BYTES: usize = 4096;
const MAX_COMPONENTS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Identity {
    dev: u64,
    ino: u64,
    mode: u32,
    uid: u32,
    gid: u32,
}
impl Identity {
    fn from_stat(stat: Stat) -> Self {
        Self {
            dev: stat.st_dev as u64,
            ino: stat.st_ino as u64,
            mode: stat.st_mode as u32,
            uid: stat.st_uid,
            gid: stat.st_gid,
        }
    }
    fn directory(self) -> bool {
        FileType::from_raw_mode(self.mode as rustix::fs::RawMode) == FileType::Directory
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileState {
    identity: Identity,
    links: u64,
    size: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}
impl FileState {
    fn from_stat(stat: Stat) -> Result<Self> {
        Ok(Self {
            identity: Identity::from_stat(stat),
            links: stat.st_nlink as u64,
            size: u64::try_from(stat.st_size)?,
            modified: (stat.st_mtime as i64, stat.st_mtime_nsec as i64),
            changed: (stat.st_ctime as i64, stat.st_ctime_nsec as i64),
        })
    }
    fn owned_regular(self, uid: u32) -> bool {
        FileType::from_raw_mode(self.identity.mode as rustix::fs::RawMode) == FileType::RegularFile
            && self.identity.uid == uid
            && self.identity.mode & 0o7777 == 0o600
            && self.links == 1
    }
}
// Directory byte counts and timestamps are not admission authorities.
fn held_identity(file: &File) -> Result<Identity> {
    Ok(Identity::from_stat(rustix::fs::fstat(file)?))
}
fn named_identity(parent: &File, name: &std::ffi::OsStr) -> Result<Identity> {
    Ok(Identity::from_stat(rustix::fs::statat(
        parent,
        name,
        AtFlags::SYMLINK_NOFOLLOW,
    )?))
}
fn held(file: &File) -> Result<FileState> {
    FileState::from_stat(rustix::fs::fstat(file)?)
}
fn named(parent: &File, name: &std::ffi::OsStr) -> Result<FileState> {
    FileState::from_stat(rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)?)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    ParentAdmitted,
    ParentRetained,
    InputAdmitted,
    InputRetained,
    BeforeRead,
    AfterRead,
    BeforeRequestDecode,
    AfterRequestDecode,
    BeforeVerification,
    BeforeIdentity,
    AfterIdentity,
    BeforeProjection,
    AfterProjection,
    BeforeInputFinish,
    AfterVerification,
    BeforeWrite,
    AfterWrite,
    BeforeReadback,
    AfterReadback,
    BeforeCreate,
    AfterCreate,
    BeforeDirectorySync,
    AfterDirectorySync,
    BeforeFileSync,
    AfterFileSync,
    BeforeRename,
    RenameReady,
    AfterRename,
}
#[derive(Debug)]
struct Directory {
    file: File,
    name: Option<OsString>,
    identity: Identity,
}
#[derive(Debug)]
struct Parent {
    directories: Vec<Directory>,
    uid: u32,
}
impl Parent {
    fn capture(
        path: &Path,
        hook: &mut impl FnMut(Phase) -> Result<()>,
    ) -> Result<(Self, OsString)> {
        ensure!(
            path.is_absolute() && path.as_os_str().as_bytes().len() <= MAX_PATH_BYTES,
            "proof requires a bounded absolute path"
        );
        let mut normalized = PathBuf::new();
        let mut names = Vec::new();
        for component in path.components() {
            match component {
                Component::RootDir => normalized.push(component.as_os_str()),
                Component::Normal(name) => {
                    normalized.push(name);
                    names.push(name.to_os_string());
                }
                _ => return Err(eyre!("proof path must be lexical and normalized")),
            }
        }
        ensure!(
            normalized.as_os_str() == path.as_os_str()
                && !names.is_empty()
                && names.len() <= MAX_COMPONENTS,
            "proof path components are invalid"
        );
        let leaf = names.pop().ok_or_else(|| eyre!("proof leaf missing"))?;
        let file = File::from(rustix::fs::open("/", DIRECTORY_FLAGS, Mode::empty())?);
        let root = held_identity(&file)?;
        ensure!(root.directory(), "proof filesystem root is not a directory");
        let mut directories = vec![Directory {
            file,
            name: None,
            identity: root,
        }];
        for (index, name) in names.iter().enumerate() {
            let parent = &directories
                .last()
                .ok_or_else(|| eyre!("proof parent missing"))?
                .file;
            let before = named_identity(parent, name)?;
            ensure!(
                before.directory(),
                "proof ancestor is not a non-symlink directory"
            );
            if index + 1 == names.len() {
                hook(Phase::ParentAdmitted)?;
            }
            let file = File::from(rustix::fs::openat(
                parent,
                name,
                DIRECTORY_FLAGS,
                Mode::empty(),
            )?);
            ensure!(
                held_identity(&file)? == before && named_identity(parent, name)? == before,
                "proof directory changed during admission"
            );
            directories.push(Directory {
                file,
                name: Some(name.clone()),
                identity: before,
            });
        }
        let owner = Self {
            directories,
            uid: rustix::process::geteuid().as_raw(),
        };
        owner.check()?;
        hook(Phase::ParentRetained)?;
        owner.check()?;
        Ok((owner, leaf))
    }
    fn file(&self) -> &File {
        &self
            .directories
            .last()
            .expect("retained filesystem root")
            .file
    }
    fn check(&self) -> Result<()> {
        for (index, directory) in self.directories.iter().enumerate() {
            ensure!(
                held_identity(&directory.file)? == directory.identity,
                "held proof directory changed"
            );
            if let Some(name) = &directory.name {
                ensure!(
                    index > 0
                        && named_identity(&self.directories[index - 1].file, name)?
                            == directory.identity,
                    "named proof ancestor changed"
                );
            }
        }
        Ok(())
    }
    fn create(
        &self,
        name: &std::ffi::OsStr,
        hook: &mut impl FnMut(Phase) -> Result<()>,
    ) -> Result<(File, Identity)> {
        self.check()?;
        hook(Phase::BeforeCreate)?;
        self.check()?;
        let file = File::from(rustix::fs::openat(
            self.file(),
            name,
            FILE_FLAGS,
            Mode::RUSR | Mode::WUSR,
        )?);
        let state = held(&file)?;
        ensure!(
            state.owned_regular(self.uid) && state.size == 0 && named(self.file(), name)? == state,
            "created output is not the exact owned empty regular file"
        );
        hook(Phase::AfterCreate)?;
        self.check_file(&file, name, state.identity, 0)?;
        self.sync(hook)?;
        self.check_file(&file, name, state.identity, 0)?;
        Ok((file, state.identity))
    }
    fn check_file(
        &self,
        file: &File,
        name: &std::ffi::OsStr,
        identity: Identity,
        maximum: u64,
    ) -> Result<FileState> {
        self.check()?;
        let state = held(file)?;
        ensure!(
            state.identity == identity
                && state.owned_regular(self.uid)
                && state.size <= maximum
                && named(self.file(), name)? == state,
            "held or named output file changed or exceeded allocation"
        );
        Ok(state)
    }
    fn require_absent(&self, name: &std::ffi::OsStr) -> Result<()> {
        self.check()?;
        ensure!(
            matches!(
                rustix::fs::statat(self.file(), name, AtFlags::SYMLINK_NOFOLLOW),
                Err(rustix::io::Errno::NOENT)
            ),
            "trace stage name reappeared after publication"
        );
        Ok(())
    }
    fn sync(&self, hook: &mut impl FnMut(Phase) -> Result<()>) -> Result<()> {
        self.check()?;
        hook(Phase::BeforeDirectorySync)?;
        self.check()?;
        self.file().sync_all()?;
        hook(Phase::AfterDirectorySync)?;
        self.check()
    }
}

const READ_FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::NOFOLLOW)
    .union(OFlags::NONBLOCK)
    .union(OFlags::CLOEXEC);

// Positional reads leave the retained descriptor offset untouched, including
// when two immutable projection checks run concurrently.
struct ContentReader<'a> {
    file: &'a File,
    offset: u64,
}
impl std::io::Read for ContentReader<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let count = self.file.read_at(bytes, self.offset)?;
        self.offset = self
            .offset
            .checked_add(count as u64)
            .ok_or_else(|| std::io::Error::other("retained read offset overflow"))?;
        Ok(count)
    }
}

struct BoundInput {
    path: PathBuf,
    parent: Parent,
    name: OsString,
    file: File,
    state: FileState,
    digest: [u8; 32],
}
impl BoundInput {
    fn open(
        binding: ProofInputBinding,
        hook: &mut impl FnMut(Phase) -> Result<()>,
    ) -> Result<Self> {
        let (parent, name) = Parent::capture(&binding.path, hook)?;
        let before = named(parent.file(), &name)?;
        ensure!(
            FileType::from_raw_mode(before.identity.mode as rustix::fs::RawMode)
                == FileType::RegularFile
                && before.identity.uid == parent.uid
                && before.identity.mode & 0o7022 == 0
                && before.links == 1
                && before.size > 0
                && before.size <= binding.max_bytes,
            "input is not an admitted owned single-link bounded regular file"
        );
        hook(Phase::InputAdmitted)?;
        parent.check()?;
        let file = File::from(rustix::fs::openat(
            parent.file(),
            &name,
            READ_FLAGS,
            Mode::empty(),
        )?);
        ensure!(
            held(&file)? == before && named(parent.file(), &name)? == before,
            "input changed during descriptor admission"
        );
        let owner = Self {
            path: binding.path,
            parent,
            name,
            file,
            state: before,
            digest: binding.sha256,
        };
        hook(Phase::InputRetained)?;
        owner.check()?;
        Ok(owner)
    }
    fn check(&self) -> Result<()> {
        self.parent.check()?;
        ensure!(
            held(&self.file)? == self.state && named(self.parent.file(), &self.name)? == self.state,
            "held or named input changed"
        );
        Ok(())
    }
    fn read(&mut self, hook: &mut impl FnMut(Phase) -> Result<()>) -> Result<Vec<u8>> {
        self.check()?;
        hook(Phase::BeforeRead)?;
        self.check()?;
        self.file.seek(SeekFrom::Start(0))?;
        let size = usize::try_from(self.state.size)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(size)?;
        bytes.resize(size, 0);
        self.file.read_exact(&mut bytes)?;
        let mut extra = [0];
        ensure!(
            self.file.read(&mut extra)? == 0,
            "input grew beyond admitted length"
        );
        hook(Phase::AfterRead)?;
        self.check()?;
        ensure!(
            iroha_crypto::sha256(&bytes) == self.digest,
            "input SHA-256 mismatch"
        );
        Ok(bytes)
    }
    fn recheck_content(&self) -> Result<()> {
        self.check()?;
        let (digest, size) = iroha_crypto::sha256_reader_bounded(
            ContentReader {
                file: &self.file,
                offset: 0,
            }
            .take(self.state.size + 1),
            self.state.size,
        )?;
        ensure!(
            size == self.state.size && digest == self.digest,
            "final input digest mismatch"
        );
        self.check()
    }
}

// No raw accessor is visible outside this implementation. The two exact semantic
// entrypoints consume these reads; an arbitrary callback cannot certify an older
// unrelated VerifiedExport. A caught read panic leaves the actual owner poisoned.
pub(super) struct InputPublicationLease {
    files: Vec<BoundInput>,
}
impl InputPublicationLease {
    pub(super) fn check(&self) -> Result<()> {
        for file in &self.files {
            file.recheck_content()?;
        }
        // Recheck every namespace after the last full bounded digest read.
        for file in &self.files {
            file.check()?;
        }
        Ok(())
    }
}

struct Inputs {
    files: Vec<BoundInput>,
    poisoned: bool,
    read: bool,
}
impl Inputs {
    fn open(
        bindings: Vec<ProofInputBinding>,
        maximum: u64,
        hook: &mut impl FnMut(Phase) -> Result<()>,
    ) -> Result<Self> {
        ensure!(
            !bindings.is_empty() && bindings.len() <= MAX_INPUT_FILES,
            "invalid proof input count"
        );
        ensure!(
            (1..=MAX_INPUT_BYTES).contains(&maximum),
            "invalid input allocation"
        );
        let mut total = 0u64;
        let mut paths = std::collections::BTreeSet::new();
        for binding in &bindings {
            ensure!(
                (1..=maximum).contains(&binding.max_bytes),
                "invalid per-file input cap"
            );
            total = total
                .checked_add(binding.max_bytes)
                .ok_or_else(|| eyre!("input reservation overflow"))?;
            ensure!(total <= maximum, "aggregate input reservation exceeded");
            ensure!(paths.insert(&binding.path), "duplicate proof input path");
        }
        drop(paths);
        let mut files = Vec::with_capacity(bindings.len());
        let mut identities = std::collections::BTreeSet::new();
        for binding in bindings {
            let file = BoundInput::open(binding, hook)?;
            ensure!(
                identities.insert((file.state.identity.dev, file.state.identity.ino)),
                "duplicate proof input inode"
            );
            files.push(file);
        }
        for file in &files {
            file.check()?;
        }
        Ok(Self {
            files,
            poisoned: false,
            read: false,
        })
    }
    fn read_all(&mut self, hook: &mut impl FnMut(Phase) -> Result<()>) -> Result<Vec<Vec<u8>>> {
        ensure!(
            !self.poisoned && !self.read,
            "input owner poisoned or already consumed"
        );
        self.poisoned = true;
        let mut result = Vec::with_capacity(self.files.len());
        for file in &mut self.files {
            result.push(file.read(hook)?);
        }
        for file in &self.files {
            file.check()?;
        }
        self.read = true;
        self.poisoned = false;
        Ok(result)
    }
    fn check_disjoint(&self, retained: &InputPublicationLease) -> Result<()> {
        ensure!(
            self.files
                .len()
                .checked_add(retained.files.len())
                .is_some_and(|n| n <= MAX_INPUT_FILES),
            "aggregate input count exceeded"
        );
        retained.check()?;
        for file in &self.files {
            file.check()?;
            for prior in &retained.files {
                ensure!(
                    file.path != prior.path
                        && (file.state.identity.dev, file.state.identity.ino)
                            != (prior.state.identity.dev, prior.state.identity.ino),
                    "request and proof input share a path or inode"
                );
            }
        }
        Ok(())
    }
    fn include_request(mut self, retained: InputPublicationLease) -> Result<Self> {
        ensure!(
            !self.poisoned && self.read,
            "input owner is incomplete or poisoned"
        );
        self.check_disjoint(&retained)?;
        self.files.extend(retained.files);
        Ok(self)
    }
    fn finish(
        self,
        proof: VerifiedExport,
        hook: &mut impl FnMut(Phase) -> Result<()>,
    ) -> Result<RetainedProof> {
        Ok(RetainedProof {
            input_lease: self.finish_lease(hook)?,
            proof,
        })
    }
    fn finish_lease(
        mut self,
        hook: &mut impl FnMut(Phase) -> Result<()>,
    ) -> Result<InputPublicationLease> {
        ensure!(
            !self.poisoned && self.read,
            "input owner is incomplete or poisoned"
        );
        self.poisoned = true;
        hook(Phase::BeforeInputFinish)?;
        for file in &mut self.files {
            file.recheck_content()?;
        }
        // All retained namespaces are rechecked again after the last file read.
        for file in &self.files {
            file.check()?;
        }
        Ok(InputPublicationLease { files: self.files })
    }
}

fn remaining_input(plan: &TrustedRunPlan, limits: VerificationLimits) -> Result<u64> {
    ensure!(
        (1..=MAX_INPUT_BYTES).contains(&limits.admitted_proof_bytes)
            && limits.input_bytes > 0
            && limits.output_bytes > 0
            && limits
                .input_bytes
                .checked_add(limits.output_bytes)
                .is_some_and(|n| n <= limits.admitted_proof_bytes),
        "invalid proof reservations"
    );
    ensure!(
        limits.requests > 0
            && limits.requests <= 1_000_000
            && (1..=65_536).contains(&limits.heights)
            && (1..=1_000_000).contains(&limits.leaves_per_carrier)
            && !plan.scheduled.is_empty()
            && plan.scheduled.len() <= limits.requests,
        "invalid scheduled input count"
    );
    let mut reserved = 0u64;
    for request in &plan.scheduled {
        ensure!(
            !request.signed_transaction.is_empty()
                && request.signed_transaction.len() <= 1024 * 1024,
            "invalid signed request byte count"
        );
        reserved = reserved
            .checked_add(u64::try_from(request.signed_transaction.len())?)
            .ok_or_else(|| eyre!("signed input reservation overflow"))?;
        ensure!(
            reserved < limits.input_bytes,
            "signed input reservation exhausted"
        );
    }
    Ok(limits.input_bytes - reserved)
}

/// Admit canonical launcher facts from one independently raw-digest-bound file.
pub(crate) fn open_launcher(input: ProofInputBinding) -> Result<RetainedLauncherRequest> {
    open_launcher_with_hook(input, |_| Ok(()))
}
fn open_launcher_with_hook(
    input: ProofInputBinding,
    mut hook: impl FnMut(Phase) -> Result<()>,
) -> Result<RetainedLauncherRequest> {
    let reserved_bytes = input.max_bytes;
    let expected_sha256 = input.sha256;
    let mut owner = Inputs::open(vec![input], MAX_INPUT_BYTES, &mut hook)?;
    let bytes = owner
        .read_all(&mut hook)?
        .pop()
        .ok_or_else(|| eyre!("missing retained launcher request"))?;
    hook(Phase::BeforeRequestDecode)?;
    for file in &owner.files {
        file.check()?;
    }
    let request = super::super::launcher::decode(&bytes, expected_sha256, reserved_bytes)?;
    drop(bytes);
    hook(Phase::AfterRequestDecode)?;
    let input_lease = owner.finish_lease(&mut hook)?;
    Ok(RetainedLauncherRequest {
        input_lease,
        request,
        reserved_bytes,
    })
}

/// Replay the exact retained artifact under consumed, retained launcher authority.
pub(crate) fn replay_bound_request(
    request: RetainedLauncherRequest,
    expected_artifact_hash: Hash,
    input: ProofInputBinding,
) -> Result<RetainedProof> {
    replay_with_hook(request, expected_artifact_hash, input, |_| Ok(()))
}
fn replay_with_hook(
    request: RetainedLauncherRequest,
    expected_artifact_hash: Hash,
    input: ProofInputBinding,
    mut hook: impl FnMut(Phase) -> Result<()>,
) -> Result<RetainedProof> {
    request.input_lease.check()?;
    let RetainedLauncherRequest {
        input_lease,
        request,
        reserved_bytes,
    } = request;
    let (plan, limits, bindings) = request.into_parts();
    let remaining = remaining_input(&plan, limits)?
        .checked_sub(reserved_bytes)
        .filter(|n| *n > 0)
        .ok_or_else(|| eyre!("launcher and proof reservations exceed total"))?;
    ensure!(
        input_lease.files.iter().all(|f| f.path != input.path),
        "duplicate request and proof input path"
    );
    let (owner, proof) = {
        let mut retained_hook = |phase| {
            hook(phase)?;
            input_lease.check()
        };
        let mut owner = Inputs::open(vec![input], remaining, &mut retained_hook)?;
        owner.check_disjoint(&input_lease)?;
        let bytes = owner
            .read_all(&mut retained_hook)?
            .pop()
            .ok_or_else(|| eyre!("missing retained artifact"))?;
        retained_hook(Phase::BeforeVerification)?;
        let proof =
            super::super::replay_export(plan, limits, &bindings, expected_artifact_hash, &bytes)?;
        drop(bytes);
        retained_hook(Phase::AfterVerification)?;
        (owner, proof)
    };
    owner.include_request(input_lease)?.finish(proof, &mut hook)
}

/// Export one canonical bundle and immutable Core interval under retained request authority.
pub(crate) fn export_bound_request(
    request: RetainedLauncherRequest,
    block_store: &Path,
    merge_log: &Path,
    reader_limits: CanonicalKuraEvidenceLimits,
    input: ProofInputBinding,
) -> Result<RetainedProof> {
    export_with_hook(
        request,
        block_store,
        merge_log,
        reader_limits,
        input,
        |_| Ok(()),
    )
}
fn export_with_hook(
    request: RetainedLauncherRequest,
    block_store: &Path,
    merge_log: &Path,
    reader_limits: CanonicalKuraEvidenceLimits,
    input: ProofInputBinding,
    mut hook: impl FnMut(Phase) -> Result<()>,
) -> Result<RetainedProof> {
    request.input_lease.check()?;
    let RetainedLauncherRequest {
        input_lease,
        request,
        reserved_bytes,
    } = request;
    let (plan, limits, bindings) = request.into_parts();
    let remaining = remaining_input(&plan, limits)?
        .checked_sub(reserved_bytes)
        .filter(|n| *n > 0)
        .ok_or_else(|| eyre!("launcher and proof reservations exceed total"))?;
    ensure!(
        input_lease.files.iter().all(|f| f.path != input.path),
        "duplicate request and proof input path"
    );
    let file_reservation = remaining
        .checked_sub(reader_limits.max_output_bytes)
        .filter(|n| *n > 0)
        .ok_or_else(|| eyre!("Core and file input reservations exceed total"))?;
    ensure!(
        reader_limits.first_height == plan.first_height
            && reader_limits.last_height == plan.last_height
            && reader_limits.max_store_data_bytes <= limits.input_bytes
            && reader_limits.max_merge_log_bytes <= limits.input_bytes
            && reader_limits.max_decode_allocation_bytes as u64 <= limits.admitted_proof_bytes * 2,
        "Core reader work differs from independent run scope"
    );
    ensure!(
        plan.first_height > 0
            && plan.last_height < u64::MAX
            && plan
                .last_height
                .checked_sub(plan.first_height)
                .and_then(|n| n.checked_add(1))
                == Some(bindings.len() as u64)
            && !bindings.is_empty()
            && bindings.len() as u64 <= limits.heights,
        "invalid supplied height binding count"
    );
    let mut count = 0usize;
    for (index, binding) in bindings.iter().enumerate() {
        ensure!(
            binding.height == plan.first_height + index as u64,
            "supplied binding height order"
        );
        count = count
            .checked_add(binding.query_hashes.len())
            .ok_or_else(|| eyre!("binding count overflow"))?;
        ensure!(
            count <= limits.requests,
            "supplied query count exceeds plan limit"
        );
    }
    let (owner, proof) = {
        let mut retained_hook = |phase| {
            hook(phase)?;
            input_lease.check()
        };
        let mut owner = Inputs::open(vec![input], file_reservation, &mut retained_hook)?;
        owner.check_disjoint(&input_lease)?;
        let bytes = owner
            .read_all(&mut retained_hook)?
            .pop()
            .ok_or_else(|| eyre!("missing supplied bundle"))?;
        let canonical = norito::canonical_decode_limits(bytes.len());
        let decode = norito::DecodeLimits::new(
            canonical.max_sequence_elements(),
            canonical.max_field_bytes(),
            canonical.max_total_elements(),
            canonical.max_total_allocated_bytes().min(usize::try_from(
                limits
                    .admitted_proof_bytes
                    .checked_mul(2)
                    .ok_or_else(|| eyre!("decode reservation overflow"))?,
            )?),
            64,
        );
        // The complete outer frame stays alive during bounded decode. The codec's
        // sequence/element/allocation/depth guards apply before allocating declared
        // nested vectors; semantic height/leaf cardinalities are then compared to the
        // independent bindings. This finite decoded-memory reservation is not a claim
        // that simultaneous outer bytes plus decoded objects fit the file byte cap.
        let bundle: SuppliedEvidenceBundleV1 =
            norito::decode_canonical_with_limits(&bytes, decode)?;
        drop(bytes);
        ensure!(
            bundle.version == 1 && bundle.heights.len() == bindings.len(),
            "supplied bundle version or complete interval mismatch"
        );
        let mut supplied = Vec::with_capacity(bundle.heights.len());
        for (height, binding) in bundle.heights.into_iter().zip(&bindings) {
            ensure!(
                height.height == binding.height
                    && height.queries.len() == binding.query_hashes.len(),
                "supplied bundle roles differ from independent binding"
            );
            supplied.push(SuppliedHeightEvidence {
                height: height.height,
                finality: height.finality,
                queries: height.queries,
            });
        }
        retained_hook(Phase::BeforeVerification)?;
        let proof = super::super::export_from_kura(
            plan,
            limits,
            block_store,
            merge_log,
            reader_limits,
            &bindings,
            supplied,
        )?;
        retained_hook(Phase::AfterVerification)?;
        (owner, proof)
    };
    owner.include_request(input_lease)?.finish(proof, &mut hook)
}

impl RetainedProof {
    /// Calculate both independent digests from actual canonical bytes under the retained sources.
    pub(crate) fn identity(&self) -> Result<CanonicalProofIdentity> {
        self.identity_with_hook(|_| Ok(()))
    }
    fn identity_with_hook(
        &self,
        mut hook: impl FnMut(Phase) -> Result<()>,
    ) -> Result<CanonicalProofIdentity> {
        self.recheck_sources()?;
        hook(Phase::BeforeIdentity)?;
        self.recheck_sources()?;
        let bytes = self.proof.canonical_bytes();
        let identity = CanonicalProofIdentity {
            raw_sha256: iroha_crypto::sha256(bytes),
            iroha_hash: Hash::new(bytes),
            byte_length: u64::try_from(bytes.len())?,
        };
        hook(Phase::AfterIdentity)?;
        self.recheck_sources()?;
        Ok(identity)
    }
    /// Derive the projection while all original request and evidence inputs remain retained.
    /// The independent maximum includes array delimiters and commas.
    pub(crate) fn json_projection(&self, maximum: u64) -> Result<Vec<u8>> {
        self.json_projection_with_hook(maximum, |_| Ok(()))
    }
    fn json_projection_with_hook(
        &self,
        maximum: u64,
        mut hook: impl FnMut(Phase) -> Result<()>,
    ) -> Result<Vec<u8>> {
        self.recheck_sources()?;
        hook(Phase::BeforeProjection)?;
        self.recheck_sources()?;
        let projection = self.proof.json_projection(maximum)?;
        hook(Phase::AfterProjection)?;
        self.recheck_sources()?;
        Ok(projection)
    }
}

fn check_publication(proof: &RetainedProof, output: &Parent) -> Result<()> {
    output.check()?;
    proof.recheck_sources()?;
    if let Some(completed) = &proof.proof.disk_completion {
        let ancestry: Vec<_> = output
            .directories
            .iter()
            .map(|d| (d.identity.dev, d.identity.ino))
            .collect();
        completed.ensure_publication_ancestry(&ancestry)?;
    }
    proof.input_lease.check()?;
    output.check()
}

/// Retains the output parent before verification; owns no arbitrary writer or FD.
pub(crate) struct ProofOutput {
    parent: Parent,
    stage: OsString,
    destination: OsString,
    maximum: u64,
}
impl ProofOutput {
    /// Admit an exact absent destination under a retained parent and finite cap.
    pub(crate) fn admit(path: &Path, maximum: u64) -> Result<Self> {
        Self::admit_with_hook(path, maximum, |_| Ok(()))
    }
    fn admit_with_hook(
        path: &Path,
        maximum: u64,
        mut hook: impl FnMut(Phase) -> Result<()>,
    ) -> Result<Self> {
        ensure!(
            (1..=MAX_INPUT_BYTES).contains(&maximum),
            "invalid artifact allocation"
        );
        ensure!(
            path.as_os_str()
                .as_bytes()
                .len()
                .checked_add(".publishing".len())
                .is_some_and(|n| n <= MAX_PATH_BYTES),
            "artifact stage path exceeds bound"
        );
        let (parent, destination) = Parent::capture(path, &mut hook)?;
        let mut stage = destination.clone();
        stage.push(".publishing");
        parent.require_absent(&destination)?;
        parent.require_absent(&stage)?;
        Ok(Self {
            parent,
            stage,
            destination,
            maximum,
        })
    }
    /// Publish only a completed canonical proof, never arbitrary bytes or a prefix.
    pub(crate) fn publish(self, proof: RetainedProof) -> Result<PublishedProof> {
        self.publish_with_hook(proof, |_| Ok(()))
    }
    fn publish_with_hook(
        self,
        proof: RetainedProof,
        mut hook: impl FnMut(Phase) -> Result<()>,
    ) -> Result<PublishedProof> {
        check_publication(&proof, &self.parent)?;
        let mut hook = |phase| {
            hook(phase)?;
            check_publication(&proof, &self.parent)
        };
        let bytes = proof.proof.canonical_bytes();
        let length = u64::try_from(bytes.len())?;
        ensure!(
            length > 0 && length <= self.maximum,
            "artifact exceeds admitted output cap"
        );
        let expected = iroha_crypto::sha256(bytes);
        self.parent.require_absent(&self.destination)?;
        let (mut file, identity) = self.parent.create(&self.stage, &mut hook)?;
        file.write_all(bytes)?;
        let before = self
            .parent
            .check_file(&file, &self.stage, identity, length)?;
        ensure!(before.size == length, "incomplete artifact write");
        hook(Phase::BeforeFileSync)?;
        ensure!(
            self.parent
                .check_file(&file, &self.stage, identity, length)?
                == before,
            "artifact changed before fsync"
        );
        file.sync_all()?;
        hook(Phase::AfterFileSync)?;
        self.check_content(&mut file, &self.stage, before, expected)?;
        hook(Phase::BeforeRename)?;
        ensure!(
            self.parent
                .check_file(&file, &self.stage, identity, length)?
                == before,
            "artifact changed before publication"
        );
        // There is no inode-conditional rename syscall. A source-name racer here
        // can be moved, but the subsequent held/named checks reject it. No raced
        // name is ever unlinked and no successful receipt is emitted on failure.
        hook(Phase::RenameReady)?;
        rustix::fs::renameat_with(
            self.parent.file(),
            &self.stage,
            self.parent.file(),
            &self.destination,
            RenameFlags::NOREPLACE,
        )?;
        let published = self
            .parent
            .check_file(&file, &self.destination, identity, length)?;
        ensure!(
            published.size == before.size && published.modified == before.modified,
            "artifact bytes changed during rename"
        );
        hook(Phase::AfterRename)?;
        self.check_content(&mut file, &self.destination, published, expected)?;
        self.parent.require_absent(&self.stage)?;
        self.parent.sync(&mut hook)?;
        self.check_content(&mut file, &self.destination, published, expected)?;
        self.parent.require_absent(&self.stage)?;
        check_publication(&proof, &self.parent)?;
        Ok(PublishedProof {
            sha256: expected,
            byte_length: length,
            proof,
        })
    }
    fn check_content(
        &self,
        file: &mut File,
        name: &std::ffi::OsStr,
        expected: FileState,
        digest: [u8; 32],
    ) -> Result<()> {
        ensure!(
            self.parent
                .check_file(file, name, expected.identity, expected.size)?
                == expected,
            "artifact identity changed before readback"
        );
        file.seek(SeekFrom::Start(0))?;
        let (actual, length) = iroha_crypto::sha256_reader_bounded(
            (&mut *file).take(expected.size + 1),
            expected.size,
        )?;
        ensure!(
            length == expected.size && actual == digest,
            "artifact readback mismatch"
        );
        ensure!(
            self.parent
                .check_file(file, name, expected.identity, expected.size)?
                == expected,
            "artifact identity changed during readback"
        );
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[path = "prepare_pair.rs"]
mod prepare_pair;
pub(crate) use prepare_pair::{PreparedLaunch, PreparedOutputPair, prepare_bound};

#[path = "facts.rs"]
mod facts;
pub(in crate::kura::scaling_evidence::export) use facts::{PublishedFacts, produce_facts};

#[path = "stopped_tip.rs"]
mod stopped_tip;
pub(crate) use stopped_tip::{RetainedStoppedTip, observe_stopped_tip};
