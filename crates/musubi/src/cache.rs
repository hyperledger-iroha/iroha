//! Immutable, commitment-addressed source cache for Musubi V1.
//!
//! Cache paths are derived exclusively from a trusted user cache root and an
//! [`ArchiveId`]. Consumer lockfiles never supply a filesystem path. Incoming
//! CAR bytes are checked incrementally against the finalized archive
//! commitment, their raw payload is written to a private temporary file, and
//! the validated SoraFS file plan materializes the source tree. Publication is
//! an absent-destination atomic rename, so readers observe either no entry or a
//! complete immutable `src` directory.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt, fs,
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use iroha_data_model::musubi::{
    ArchiveId, MUSUBI_ARTIFACT_DESCRIPTOR_VERSION_V1, MUSUBI_MAX_FILES_V1,
    MusubiArchiveCommitmentV1, MusubiArtifactDescriptorV1, MusubiContentDigestV1,
    MusubiDependencyKindV1, MusubiDependencyReqV1, MusubiReleaseManifestV1,
    MusubiSemanticReleaseManifestV1, MusubiVerificationLockV1, MusubiVerificationNodeV1,
};
use norito::codec::{Decode, Encode};
use sorafs_car::{
    CarBuildPlan, CarStreamingWriter, ChunkSink, ChunkStore, ChunkStoreError, DirectoryPayload,
    FilePayload, ProfileId, compute_chunk_plan_digest_sha3,
    streaming_verifier::{StreamingCarVerifier, StreamingVerifierConfig},
};
use sorafs_manifest::{DagCodecId, GovernanceProofs, ManifestBuilder, PinPolicy, StorageClass};

#[cfg(unix)]
use std::os::unix::fs::{
    DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _,
};

const REGISTRY_DIRECTORY: &str = "registry-v1";
const SOURCE_DIRECTORY: &str = "src";
const RELEASE_PATH: &str = ".musubi/semantic-release.norito";
const DESCRIPTOR_PATH: &str = ".musubi/artifact-descriptor.norito";
const VERIFICATION_LOCK_PATH: &str = ".musubi/verification-lock.norito";
const SOURCE_TREE_DOMAIN: &[u8] = b"musubi-source-tree-v1\0";
const ARTIFACT_DESCRIPTOR_DOMAIN: &[u8] = b"musubi-artifact-descriptor-v1\0";
const BUNDLE_DOMAIN: &[u8] = b"musubi-bundle-v1\0";
const CAR_PRAGMA_BYTES: usize = 11;
const CAR_HEADER_BYTES: usize = 40;
const CAR_DATA_OFFSET: u64 = (CAR_PRAGMA_BYTES + CAR_HEADER_BYTES) as u64;
const CAR_MAX_HEADER_BYTES: u64 = 64 * 1024;
const CAR_MAX_SECTION_BYTES: u64 = 4 * 1024 * 1024 + 128;
const RAW_CODEC: u64 = 0x55;
const DAG_CBOR_CODEC: u64 = 0x71;
const BLAKE3_MULTIHASH: u64 = 0x1f;
const CID_DIGEST_BYTES: u64 = 32;
const IO_BUFFER_BYTES: usize = 64 * 1024;
const DESCRIPTOR_MAX_BYTES: u64 = 64 * 1024;
const BUNDLE_METADATA_MAX_BYTES: u64 = 16 * 1024 * 1024;
const TEMP_RETRIES: usize = 32;
const BUNDLE_METADATA_FILE_COUNT: usize = 3;
const MAX_CACHE_FILE_COUNT: usize = MUSUBI_MAX_FILES_V1 as usize + BUNDLE_METADATA_FILE_COUNT;
const MAX_CACHE_PATH_COMPONENTS: usize = 64;
const MAX_CACHE_ENTRY_COUNT: usize = MAX_CACHE_FILE_COUNT * (MAX_CACHE_PATH_COMPONENTS + 1);

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// A cache rooted below an explicit user-owned directory.
#[derive(Debug, Clone)]
pub struct MusubiCache {
    root: PathBuf,
    root_identity: DirectoryIdentity,
    registry_root: PathBuf,
    registry_identity: DirectoryIdentity,
}

/// A verified immutable cache entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheEntry {
    /// Commitment identity used in the cache path.
    pub archive_id: ArchiveId,
    /// Immutable extracted source directory.
    pub source_path: PathBuf,
}

/// Authenticated bundle metadata and source root accepted for compiler input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedCompilerPackageV1 {
    /// Immutable extracted source directory.
    pub source_path: PathBuf,
    /// Exact canonical `Musubi.toml` bytes decoded as UTF-8 after digest verification.
    pub manifest: String,
    /// Digest-verified Kotodama sources, ordered by portable bundle path.
    pub kotodama_sources: Vec<CachedKotodamaSourceV1>,
    /// Canonical archive-independent release metadata embedded in the bundle.
    pub semantic_release: MusubiSemanticReleaseManifestV1,
    /// Publisher-supplied exact proof lock embedded in the bundle.
    pub publication_lock: MusubiVerificationLockV1,
}

/// One immutable Kotodama source copied out of a verified cache tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CachedKotodamaSourceV1 {
    /// Portable path relative to the package root.
    pub path: String,
    /// Complete UTF-8 source text.
    pub source: String,
}

/// Result of installing an archive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InstallOutcome {
    /// This invocation published a new immutable entry.
    Installed(CacheEntry),
    /// An identical, healthy entry was already present.
    AlreadyPresent(CacheEntry),
}

/// Result of checking and repairing one cache entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepairOutcome {
    /// The existing entry passed every commitment check.
    Healthy(CacheEntry),
    /// No source directory exists for the archive.
    Missing,
    /// A structurally safe but corrupt tree was moved aside for inspection.
    Quarantined {
        /// Path of the quarantined tree.
        path: PathBuf,
    },
}

/// Summary of a cache prune operation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PruneReport {
    /// Archive identities removed from the cache.
    pub removed: Vec<ArchiveId>,
}

/// Errors raised by the immutable cache.
#[derive(Debug)]
pub enum CacheError {
    /// The platform cannot provide the required no-follow filesystem guarantees.
    UnsupportedPlatform,
    /// A filesystem operation failed.
    Io {
        /// Stable operation label without secret-bearing input.
        operation: &'static str,
        /// Cache-owned path involved in the operation.
        path: PathBuf,
        /// Underlying error.
        source: io::Error,
    },
    /// The configured cache root is not a stable private directory.
    UnsafeRoot(String),
    /// The archive commitment and SoraFS plan disagree.
    InvalidPlan(String),
    /// Incoming CAR bytes did not match the finalized commitment.
    InvalidArchive(String),
    /// Extracted bytes or filesystem objects failed verification.
    CorruptEntry(String),
    /// A repair or prune candidate contains a descendant that cannot be safely mutated.
    UnsafeDescendant(PathBuf),
}

impl fmt::Display for CacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => write!(
                formatter,
                "secure Musubi cache publication requires Unix no-follow file identities"
            ),
            Self::Io {
                operation,
                path,
                source,
            } => write!(
                formatter,
                "failed to {operation} cache path `{}`: {source}",
                path.display()
            ),
            Self::UnsafeRoot(reason) => write!(formatter, "unsafe Musubi cache root: {reason}"),
            Self::InvalidPlan(reason) => {
                write!(formatter, "invalid Musubi archive plan: {reason}")
            }
            Self::InvalidArchive(reason) => {
                write!(formatter, "Musubi archive verification failed: {reason}")
            }
            Self::CorruptEntry(reason) => write!(formatter, "corrupt Musubi cache entry: {reason}"),
            Self::UnsafeDescendant(path) => write!(
                formatter,
                "cache descendant `{}` is not safe to quarantine or prune",
                path.display()
            ),
        }
    }
}

impl Error for CacheError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl MusubiCache {
    /// Open or create `registry-v1` below an explicit user cache root.
    ///
    /// The supplied path is configuration, not lockfile data. Its final
    /// component must not be a symlink and neither it nor the registry root may
    /// be group- or world-writable.
    ///
    /// # Errors
    ///
    /// Returns an error when secure Unix semantics are unavailable, directory
    /// creation fails, or either root is unsafe.
    pub fn open(user_root: impl AsRef<Path>) -> Result<Self, CacheError> {
        if !cfg!(unix) {
            // TODO: Add equivalent Windows handle-relative, reparse-point-safe
            // creation and ReplaceFile-free publication before enabling V1 cache writes there.
            return Err(CacheError::UnsupportedPlatform);
        }

        let requested = absolute_path(user_root.as_ref())?;
        let existed = requested
            .try_exists()
            .map_err(|source| io_error("inspect user cache root", &requested, source))?;
        if !existed {
            fs::create_dir_all(&requested)
                .map_err(|source| io_error("create user cache root", &requested, source))?;
            #[cfg(unix)]
            fs::set_permissions(&requested, fs::Permissions::from_mode(0o700))
                .map_err(|source| io_error("secure user cache root", &requested, source))?;
        }
        let linked = fs::symlink_metadata(&requested)
            .map_err(|source| io_error("inspect user cache root", &requested, source))?;
        validate_private_directory(&requested, &linked)?;
        let root = fs::canonicalize(&requested)
            .map_err(|source| io_error("canonicalize user cache root", &requested, source))?;
        let canonical_metadata = fs::symlink_metadata(&root)
            .map_err(|source| io_error("inspect canonical cache root", &root, source))?;
        validate_private_directory(&root, &canonical_metadata)?;
        if !same_file(&linked, &canonical_metadata) {
            return Err(CacheError::UnsafeRoot(
                "user cache root changed during canonicalization".to_owned(),
            ));
        }

        let registry_root = root.join(REGISTRY_DIRECTORY);
        create_or_validate_private_directory(&registry_root)?;
        sync_directory(&root).map_err(|source| io_error("sync user cache root", &root, source))?;
        let registry_metadata = fs::symlink_metadata(&registry_root)
            .map_err(|source| io_error("inspect registry cache root", &registry_root, source))?;

        Ok(Self {
            root,
            root_identity: DirectoryIdentity::capture(&canonical_metadata),
            registry_root,
            registry_identity: DirectoryIdentity::capture(&registry_metadata),
        })
    }

    /// Return the trusted user cache root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Derive the immutable source path for an archive identity.
    #[must_use]
    pub fn source_path(&self, archive_id: &ArchiveId) -> PathBuf {
        self.archive_directory(archive_id).join(SOURCE_DIRECTORY)
    }

    /// Re-authenticate one immutable cached bundle for compiler consumption.
    ///
    /// This boundary does not trust permissions or a consumer lock as cache
    /// integrity evidence. It re-inventories every descendant without following
    /// links, reproduces the normalized source digest, validates the canonical
    /// bundle metadata, and binds the semantic release to the consumer node's
    /// exact archive and release digest. The publisher's packaged proof lock is
    /// validated independently; its exact selections need not match a consumer's
    /// independently resolved graph.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache ancestry is unsafe, the tree changed while
    /// being read, a bound is exceeded, or any immutable node/bundle commitment
    /// disagrees.
    pub fn load_compiler_package(
        &self,
        node: &MusubiVerificationNodeV1,
    ) -> Result<CachedCompilerPackageV1, CacheError> {
        node.validate()
            .map_err(|error| CacheError::CorruptEntry(error.to_string()))?;
        self.validate_anchors()?;
        let archive_dir = self.validate_existing_archive_directory(&node.archive_id)?;
        let source_path = archive_dir.join(SOURCE_DIRECTORY);
        let files = inventory_tree(&source_path)?;
        let (semantic_release, publication_lock) =
            verify_compiler_bundle(&source_path, node, &files)?;
        let (manifest, kotodama_sources) = load_compiler_sources(&source_path, &files)?;
        Ok(CachedCompilerPackageV1 {
            source_path,
            manifest,
            kotodama_sources,
            semantic_release,
            publication_lock,
        })
    }

    /// Verify and atomically install a canonical CAR stream.
    ///
    /// The reader is consumed to EOF. Any trailing byte, malformed section,
    /// digest mismatch, unsafe plan, or incomplete commitment leaves `src`
    /// absent. A racing identical installer is treated idempotently.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid commitments, untrusted CAR bytes, unsafe
    /// filesystem state, failed durability operations, or a corrupt existing
    /// destination.
    pub fn install<R: Read>(
        &self,
        commitment: &MusubiArchiveCommitmentV1,
        plan: &CarBuildPlan,
        reader: R,
    ) -> Result<InstallOutcome, CacheError> {
        self.validate_anchors()?;
        validate_plan_commitment(commitment, plan)?;
        let archive_id = commitment.archive_id();
        let archive_dir = self.ensure_archive_directory(&archive_id)?;
        let source_path = archive_dir.join(SOURCE_DIRECTORY);
        if source_path
            .try_exists()
            .map_err(|source| io_error("inspect cache destination", &source_path, source))?
        {
            return self
                .verify(commitment, plan)
                .map(InstallOutcome::AlreadyPresent);
        }

        let (staging_path, staging_metadata) = create_staging_directory(&archive_dir)?;
        let (payload_path, mut payload_file, payload_metadata) =
            create_temporary_file(&archive_dir, ".payload")?;
        let mut guard = InstallGuard::new(
            staging_path.clone(),
            staging_metadata,
            payload_path.clone(),
            payload_metadata,
        );

        stream_and_verify_car(reader, commitment, plan, &mut payload_file)?;
        payload_file
            .sync_all()
            .map_err(|source| io_error("sync verified CAR payload", &payload_path, source))?;
        drop(payload_file);

        let mut payload = FilePayload::open(&payload_path)
            .map_err(|source| io_error("open verified CAR payload", &payload_path, source))?;
        let mut store = ChunkStore::with_profile(plan.chunk_profile);
        let sink = SourceTreeSink::new(staging_path.clone());
        store
            .ingest_plan_source_with_sink(plan, &mut payload, sink)
            .map_err(|error| CacheError::CorruptEntry(error.to_string()))?;
        drop(payload);
        verify_por(commitment, &store)?;
        remove_verified_file(&payload_path, &guard.payload_identity)?;
        guard.payload_removed = true;

        verify_tree(&staging_path, commitment, plan)?;
        make_tree_immutable_and_sync(&staging_path)?;
        let staging_before = fs::symlink_metadata(&staging_path)
            .map_err(|source| io_error("inspect completed staging tree", &staging_path, source))?;
        match rename_no_replace(&staging_path, &source_path) {
            Ok(()) => {}
            Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {
                guard.cleanup_staging();
                return self
                    .verify(commitment, plan)
                    .map(InstallOutcome::AlreadyPresent);
            }
            Err(source) => {
                return Err(io_error(
                    "publish immutable source tree",
                    &source_path,
                    source,
                ));
            }
        }
        guard.staging_published = true;
        let published = fs::symlink_metadata(&source_path)
            .map_err(|source| io_error("inspect published source tree", &source_path, source))?;
        if !same_file(&staging_before, &published) {
            return Err(CacheError::CorruptEntry(
                "published source identity differs from verified staging tree".to_owned(),
            ));
        }
        sync_directory(&archive_dir)
            .map_err(|source| io_error("sync archive cache directory", &archive_dir, source))?;
        let entry = self.verify(commitment, plan)?;
        Ok(InstallOutcome::Installed(entry))
    }

    /// Verify an extracted entry against every archive and bundle commitment.
    ///
    /// # Errors
    ///
    /// Returns an error if the entry is missing, has unsafe or extra
    /// descendants, differs from the plan, or cannot reproduce the canonical
    /// CAR, PoR, descriptor, source-tree, and bundle commitments.
    pub fn verify(
        &self,
        commitment: &MusubiArchiveCommitmentV1,
        plan: &CarBuildPlan,
    ) -> Result<CacheEntry, CacheError> {
        self.validate_anchors()?;
        validate_plan_commitment(commitment, plan)?;
        let archive_id = commitment.archive_id();
        let archive_dir = self.validate_existing_archive_directory(&archive_id)?;
        let source_path = archive_dir.join(SOURCE_DIRECTORY);
        verify_tree(&source_path, commitment, plan)?;
        Ok(CacheEntry {
            archive_id,
            source_path,
        })
    }

    /// Verify an entry and quarantine it only when every descendant is safe.
    ///
    /// Content-corrupt regular trees are renamed to a private sibling. Trees
    /// containing symlinks, hardlinks, special files, or unstable identities
    /// are left untouched for manual inspection.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe filesystem state or a failed quarantine.
    pub fn repair(
        &self,
        commitment: &MusubiArchiveCommitmentV1,
        plan: &CarBuildPlan,
    ) -> Result<RepairOutcome, CacheError> {
        match self.verify(commitment, plan) {
            Ok(entry) => return Ok(RepairOutcome::Healthy(entry)),
            Err(CacheError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
                return Ok(RepairOutcome::Missing);
            }
            Err(_) => {}
        }
        self.validate_anchors()?;
        let archive_id = commitment.archive_id();
        let archive_dir = self.ensure_archive_directory(&archive_id)?;
        let source_path = archive_dir.join(SOURCE_DIRECTORY);
        if !source_path
            .try_exists()
            .map_err(|source| io_error("inspect repair candidate", &source_path, source))?
        {
            return Ok(RepairOutcome::Missing);
        }
        validate_mutable_tree(&source_path)?;
        let quarantine = allocate_absent_path(&archive_dir, ".quarantine")?;
        rename_no_replace(&source_path, &quarantine)
            .map_err(|source| io_error("quarantine corrupt cache entry", &quarantine, source))?;
        sync_directory(&archive_dir)
            .map_err(|source| io_error("sync archive cache directory", &archive_dir, source))?;
        Ok(RepairOutcome::Quarantined { path: quarantine })
    }

    /// Remove complete archive directories not present in `retained`.
    ///
    /// Only canonical 64-character ArchiveId directories are considered. Each
    /// complete descendant tree is validated before it is renamed aside and
    /// deleted without following links. Unknown names remain untouched.
    ///
    /// # Errors
    ///
    /// Returns an error before mutating a candidate that contains unsafe or
    /// unstable descendants, or when publication/durability operations fail.
    pub fn prune(&self, retained: &BTreeSet<ArchiveId>) -> Result<PruneReport, CacheError> {
        self.validate_anchors()?;
        let mut candidates = Vec::new();
        for entry in fs::read_dir(&self.registry_root)
            .map_err(|source| io_error("read registry cache root", &self.registry_root, source))?
        {
            let entry = entry.map_err(|source| {
                io_error("read registry cache entry", &self.registry_root, source)
            })?;
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some(archive_id) = decode_archive_directory_name(&name) else {
                continue;
            };
            if !retained.contains(&archive_id) {
                candidates.push((archive_id, entry.path()));
            }
        }
        candidates.sort_by(|left, right| left.0.cmp(&right.0));

        let mut report = PruneReport::default();
        for (archive_id, path) in candidates {
            validate_mutable_tree(&path)?;
            let tombstone = allocate_absent_path(&self.registry_root, ".prune")?;
            rename_no_replace(&path, &tombstone)
                .map_err(|source| io_error("isolate pruned cache entry", &tombstone, source))?;
            sync_directory(&self.registry_root).map_err(|source| {
                io_error("sync registry cache root", &self.registry_root, source)
            })?;
            validate_mutable_tree(&tombstone)?;
            remove_validated_tree(&tombstone)?;
            sync_directory(&self.registry_root).map_err(|source| {
                io_error("sync registry cache root", &self.registry_root, source)
            })?;
            report.removed.push(archive_id);
        }
        Ok(report)
    }

    fn archive_directory(&self, archive_id: &ArchiveId) -> PathBuf {
        self.registry_root.join(hex::encode(archive_id.as_bytes()))
    }

    fn ensure_archive_directory(&self, archive_id: &ArchiveId) -> Result<PathBuf, CacheError> {
        self.validate_anchors()?;
        let path = self.archive_directory(archive_id);
        create_or_validate_private_directory(&path)?;
        sync_directory(&self.registry_root)
            .map_err(|source| io_error("sync registry cache root", &self.registry_root, source))?;
        Ok(path)
    }

    fn validate_existing_archive_directory(
        &self,
        archive_id: &ArchiveId,
    ) -> Result<PathBuf, CacheError> {
        let path = self.archive_directory(archive_id);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|source| io_error("inspect archive cache directory", &path, source))?;
        validate_private_directory(&path, &metadata)?;
        Ok(path)
    }

    fn validate_anchors(&self) -> Result<(), CacheError> {
        validate_directory_identity(&self.root, &self.root_identity)?;
        validate_directory_identity(&self.registry_root, &self.registry_identity)
    }
}

#[derive(Clone, Debug)]
struct DirectoryIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

impl DirectoryIdentity {
    fn capture(metadata: &fs::Metadata) -> Self {
        Self {
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
        }
    }

    fn matches(&self, metadata: &fs::Metadata) -> bool {
        #[cfg(unix)]
        {
            self.device == metadata.dev() && self.inode == metadata.ino()
        }
        #[cfg(not(unix))]
        {
            let _ = metadata;
            false
        }
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf, CacheError> {
    if path.as_os_str().is_empty() {
        return Err(CacheError::UnsafeRoot(
            "cache root must not be empty".to_owned(),
        ));
    }
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(|source| io_error("resolve user cache root", path, source))
    }
}

fn validate_private_directory(path: &Path, metadata: &fs::Metadata) -> Result<(), CacheError> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CacheError::UnsafeRoot(format!(
            "`{}` must be a real directory",
            path.display()
        )));
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o022 != 0 {
        return Err(CacheError::UnsafeRoot(format!(
            "`{}` must not be group- or world-writable",
            path.display()
        )));
    }
    Ok(())
}

fn create_or_validate_private_directory(path: &Path) -> Result<(), CacheError> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    builder.mode(0o700);
    match builder.create(path) {
        Ok(()) => {}
        Err(source) if source.kind() == io::ErrorKind::AlreadyExists => {}
        Err(source) => return Err(io_error("create private cache directory", path, source)),
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| io_error("inspect private cache directory", path, source))?;
    validate_private_directory(path, &metadata)
}

fn validate_directory_identity(
    path: &Path,
    identity: &DirectoryIdentity,
) -> Result<(), CacheError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| io_error("validate cache root identity", path, source))?;
    validate_private_directory(path, &metadata)?;
    if !identity.matches(&metadata) {
        return Err(CacheError::UnsafeRoot(format!(
            "`{}` changed after cache initialization",
            path.display()
        )));
    }
    Ok(())
}

fn validate_plan_commitment(
    commitment: &MusubiArchiveCommitmentV1,
    plan: &CarBuildPlan,
) -> Result<(), CacheError> {
    commitment
        .validate()
        .map_err(|error| CacheError::InvalidPlan(error.to_string()))?;
    plan.validate()
        .map_err(|error| CacheError::InvalidPlan(error.to_string()))?;
    if plan.content_length != commitment.content_length {
        return Err(CacheError::InvalidPlan(format!(
            "payload length {} differs from commitment {}",
            plan.content_length, commitment.content_length
        )));
    }
    if plan.chunks.len() != commitment.chunk_count as usize {
        return Err(CacheError::InvalidPlan(format!(
            "chunk count {} differs from commitment {}",
            plan.chunks.len(),
            commitment.chunk_count
        )));
    }
    let plan_digest = compute_chunk_plan_digest_sha3(&plan.chunks);
    if commitment.chunk_plan_digest.as_bytes() != &plan_digest {
        return Err(CacheError::InvalidPlan(
            "canonical chunk-plan digest mismatch".to_owned(),
        ));
    }
    let descriptor = sorafs_car::chunker_registry::lookup(ProfileId(commitment.chunker.profile_id))
        .ok_or_else(|| CacheError::InvalidPlan("unknown chunker profile id".to_owned()))?;
    if descriptor.namespace != commitment.chunker.namespace
        || descriptor.name != commitment.chunker.name
        || descriptor.semver != commitment.chunker.semver
        || descriptor.multihash_code != commitment.chunker.multihash_code
        || descriptor.profile != plan.chunk_profile
    {
        return Err(CacheError::InvalidPlan(
            "chunker handle does not identify the plan profile".to_owned(),
        ));
    }

    let mut source_count = 0usize;
    let mut release_count = 0usize;
    let mut descriptor_count = 0usize;
    let mut lock_count = 0usize;
    for file in &plan.files {
        let path = file.path.join("/");
        match path.as_str() {
            RELEASE_PATH => release_count += 1,
            DESCRIPTOR_PATH => descriptor_count += 1,
            VERIFICATION_LOCK_PATH => lock_count += 1,
            _ if path.starts_with(".musubi/") => {
                return Err(CacheError::InvalidPlan(format!(
                    "unexpected bundle metadata entry `{path}`"
                )));
            }
            _ => {
                source_count += 1;
            }
        }
    }
    if release_count != 1 || descriptor_count != 1 || lock_count != 1 {
        return Err(CacheError::InvalidPlan(
            "bundle requires one release, descriptor, and verification lock".to_owned(),
        ));
    }
    if source_count != commitment.file_count as usize {
        return Err(CacheError::InvalidPlan(format!(
            "source file count {source_count} differs from commitment {}",
            commitment.file_count
        )));
    }
    Ok(())
}

fn manifest_for_commitment(
    commitment: &MusubiArchiveCommitmentV1,
    plan: &CarBuildPlan,
) -> Result<sorafs_manifest::ManifestV1, CacheError> {
    ManifestBuilder::new()
        .root_cid(commitment.root_cid.as_bytes().to_vec())
        .dag_codec(DagCodecId(DAG_CBOR_CODEC))
        .chunking_from_profile(plan.chunk_profile, BLAKE3_MULTIHASH)
        .chunk_digest_sha3_256(*commitment.chunk_plan_digest.as_bytes())
        .por_root(*commitment.por_root.as_bytes())
        .content_length(commitment.content_length)
        .car_digest(*commitment.car_digest.as_bytes())
        .car_size(commitment.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 0,
        })
        .governance(GovernanceProofs::default())
        .build()
        .map_err(|error| CacheError::InvalidPlan(error.to_string()))
}

fn stream_and_verify_car<R: Read>(
    reader: R,
    commitment: &MusubiArchiveCommitmentV1,
    plan: &CarBuildPlan,
    payload: &mut File,
) -> Result<(), CacheError> {
    let manifest = manifest_for_commitment(commitment, plan)?;
    let verifier = StreamingCarVerifier::new(
        manifest,
        StreamingVerifierConfig {
            max_chunk_size: 4 * 1024 * 1024,
        },
    );
    let mut stream = VerifiedCarStream::new(reader, verifier, commitment.car_size);
    let mut header = [0u8; CAR_PRAGMA_BYTES + CAR_HEADER_BYTES];
    stream.read_exact(&mut header)?;
    let data_offset = decode_u64(&header[CAR_PRAGMA_BYTES + 16..CAR_PRAGMA_BYTES + 24]);
    let data_size = decode_u64(&header[CAR_PRAGMA_BYTES + 24..CAR_PRAGMA_BYTES + 32]);
    let index_offset = decode_u64(&header[CAR_PRAGMA_BYTES + 32..CAR_PRAGMA_BYTES + 40]);
    if data_offset != CAR_DATA_OFFSET
        || data_offset.checked_add(data_size) != Some(index_offset)
        || index_offset > commitment.car_size
    {
        return Err(CacheError::InvalidArchive(
            "invalid CARv2 offsets".to_owned(),
        ));
    }
    let data_end = index_offset;

    let (carv1_header_len, _) = stream.read_varint()?;
    if carv1_header_len > CAR_MAX_HEADER_BYTES {
        return Err(CacheError::InvalidArchive(
            "CARv1 header exceeds V1 bound".to_owned(),
        ));
    }
    ensure_within_data(&stream, carv1_header_len, data_end)?;
    stream.copy_exact(carv1_header_len, None)?;

    let mut raw_chunk_index = 0usize;
    while stream.position < data_end {
        let section_start = stream.position;
        let (section_len, _) = stream.read_varint()?;
        if section_len == 0 || section_len > CAR_MAX_SECTION_BYTES {
            return Err(CacheError::InvalidArchive(format!(
                "CAR section at byte {section_start} has invalid length {section_len}"
            )));
        }
        ensure_within_data(&stream, section_len, data_end)?;
        let cid_start = stream.position;
        let (version, _) = stream.read_varint()?;
        let (codec, _) = stream.read_varint()?;
        let (multihash, _) = stream.read_varint()?;
        let (digest_len, _) = stream.read_varint()?;
        if version != 1 || multihash != BLAKE3_MULTIHASH || digest_len != CID_DIGEST_BYTES {
            return Err(CacheError::InvalidArchive(
                "CAR section uses a noncanonical CID".to_owned(),
            ));
        }
        let mut digest = [0u8; CID_DIGEST_BYTES as usize];
        stream.read_exact(&mut digest)?;
        let cid_len = stream
            .position
            .checked_sub(cid_start)
            .ok_or_else(|| CacheError::InvalidArchive("CAR CID length underflow".to_owned()))?;
        let data_len = section_len.checked_sub(cid_len).ok_or_else(|| {
            CacheError::InvalidArchive("CAR section is shorter than its CID".to_owned())
        })?;
        match codec {
            RAW_CODEC => {
                let expected = plan.chunks.get(raw_chunk_index).ok_or_else(|| {
                    CacheError::InvalidArchive("CAR contains too many raw chunks".to_owned())
                })?;
                if data_len != u64::from(expected.length) || digest != expected.digest {
                    return Err(CacheError::InvalidArchive(format!(
                        "raw CAR chunk {raw_chunk_index} differs from the canonical plan"
                    )));
                }
                stream.copy_exact(data_len, Some(payload))?;
                raw_chunk_index += 1;
            }
            DAG_CBOR_CODEC => stream.copy_exact(data_len, None)?,
            _ => {
                return Err(CacheError::InvalidArchive(format!(
                    "unsupported CAR section codec {codec:#x}"
                )));
            }
        }
    }
    if stream.position != data_end || raw_chunk_index != plan.chunks.len() {
        return Err(CacheError::InvalidArchive(
            "CAR raw chunk inventory is incomplete".to_owned(),
        ));
    }
    let index_bytes = commitment
        .car_size
        .checked_sub(stream.position)
        .ok_or_else(|| CacheError::InvalidArchive("CAR size underflow".to_owned()))?;
    stream.copy_exact(index_bytes, None)?;
    stream.finish()
}

fn ensure_within_data<R: Read>(
    stream: &VerifiedCarStream<R>,
    additional: u64,
    data_end: u64,
) -> Result<(), CacheError> {
    if stream
        .position
        .checked_add(additional)
        .is_none_or(|end| end > data_end)
    {
        return Err(CacheError::InvalidArchive(
            "CAR section crosses the declared data boundary".to_owned(),
        ));
    }
    Ok(())
}

fn decode_u64(bytes: &[u8]) -> u64 {
    let mut value = [0u8; 8];
    value.copy_from_slice(bytes);
    u64::from_le_bytes(value)
}

struct VerifiedCarStream<R> {
    reader: R,
    verifier: StreamingCarVerifier,
    position: u64,
    expected_size: u64,
}

impl<R: Read> VerifiedCarStream<R> {
    fn new(reader: R, verifier: StreamingCarVerifier, expected_size: u64) -> Self {
        Self {
            reader,
            verifier,
            position: 0,
            expected_size,
        }
    }

    fn read_exact(&mut self, bytes: &mut [u8]) -> Result<(), CacheError> {
        let count = u64::try_from(bytes.len())
            .map_err(|_| CacheError::InvalidArchive("host byte count exceeds u64".to_owned()))?;
        if self
            .position
            .checked_add(count)
            .is_none_or(|end| end > self.expected_size)
        {
            return Err(CacheError::InvalidArchive(
                "CAR exceeds its committed size".to_owned(),
            ));
        }
        self.reader
            .read_exact(bytes)
            .map_err(|error| CacheError::InvalidArchive(error.to_string()))?;
        let consumed = self
            .verifier
            .update(bytes)
            .map_err(|error| CacheError::InvalidArchive(error.to_string()))?;
        if consumed != bytes.len() {
            return Err(CacheError::InvalidArchive(
                "streaming verifier stopped before the supplied bytes".to_owned(),
            ));
        }
        self.position = self
            .position
            .checked_add(count)
            .ok_or_else(|| CacheError::InvalidArchive("CAR position overflow".to_owned()))?;
        Ok(())
    }

    fn read_varint(&mut self) -> Result<(u64, usize), CacheError> {
        let mut value = 0u64;
        for index in 0..10usize {
            let mut byte = [0u8; 1];
            self.read_exact(&mut byte)?;
            let payload = u64::from(byte[0] & 0x7f);
            let shift = u32::try_from(index * 7).expect("ten-byte u64 varint shift fits u32");
            if shift == 63 && payload > 1 {
                return Err(CacheError::InvalidArchive("varint overflow".to_owned()));
            }
            value |= payload << shift;
            if byte[0] & 0x80 == 0 {
                if index > 0 && payload == 0 {
                    return Err(CacheError::InvalidArchive(
                        "noncanonical varint encoding".to_owned(),
                    ));
                }
                return Ok((value, index + 1));
            }
        }
        Err(CacheError::InvalidArchive("varint overflow".to_owned()))
    }

    fn copy_exact(
        &mut self,
        mut remaining: u64,
        mut output: Option<&mut File>,
    ) -> Result<(), CacheError> {
        let mut buffer = [0u8; IO_BUFFER_BYTES];
        while remaining != 0 {
            let count = usize::try_from(remaining.min(IO_BUFFER_BYTES as u64))
                .expect("bounded I/O count fits usize");
            self.read_exact(&mut buffer[..count])?;
            if let Some(file) = output.as_deref_mut() {
                file.write_all(&buffer[..count])
                    .map_err(|source| CacheError::Io {
                        operation: "write verified CAR payload",
                        path: PathBuf::from("<private-payload>"),
                        source,
                    })?;
            }
            remaining -= count as u64;
        }
        Ok(())
    }

    fn finish(mut self) -> Result<(), CacheError> {
        if self.position != self.expected_size {
            return Err(CacheError::InvalidArchive(format!(
                "CAR ended at {} bytes instead of {}",
                self.position, self.expected_size
            )));
        }
        let mut trailing = [0u8; 1];
        if self
            .reader
            .read(&mut trailing)
            .map_err(|error| CacheError::InvalidArchive(error.to_string()))?
            != 0
        {
            return Err(CacheError::InvalidArchive(
                "CAR stream has trailing bytes".to_owned(),
            ));
        }
        self.verifier
            .finalize()
            .map_err(|error| CacheError::InvalidArchive(error.to_string()))
    }
}

#[derive(Debug)]
struct SourceTreeSink {
    root: PathBuf,
    files: Vec<sorafs_car::FilePlan>,
    chunk_files: Vec<usize>,
    next_chunk: usize,
    current: Option<OpenTarget>,
}

#[derive(Debug)]
struct OpenTarget {
    file_index: usize,
    path: PathBuf,
    file: File,
    written: u64,
}

impl SourceTreeSink {
    fn new(root: PathBuf) -> Self {
        Self {
            root,
            files: Vec::new(),
            chunk_files: Vec::new(),
            next_chunk: 0,
            current: None,
        }
    }

    fn create_target(&self, file_index: usize) -> Result<OpenTarget, ChunkStoreError> {
        let plan = self.files.get(file_index).ok_or_else(|| {
            ChunkStoreError::Io(io::Error::other("source-tree file index is out of range"))
        })?;
        let path = join_components(&self.root, &plan.path);
        let file = open_new_regular_file(&path).map_err(ChunkStoreError::Io)?;
        Ok(OpenTarget {
            file_index,
            path,
            file,
            written: 0,
        })
    }

    fn finish_target(&mut self) -> Result<(), ChunkStoreError> {
        let Some(mut target) = self.current.take() else {
            return Ok(());
        };
        let expected = self.files[target.file_index].size;
        if target.written != expected {
            return Err(ChunkStoreError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "source-tree file `{}` has {} bytes; expected {expected}",
                    target.path.display(),
                    target.written
                ),
            )));
        }
        target.file.flush().map_err(ChunkStoreError::Io)?;
        #[cfg(unix)]
        target
            .file
            .set_permissions(fs::Permissions::from_mode(0o444))
            .map_err(ChunkStoreError::Io)?;
        target.file.sync_all().map_err(ChunkStoreError::Io)?;
        validate_open_regular_file(&target.path, &target.file).map_err(ChunkStoreError::Io)
    }
}

impl ChunkSink for SourceTreeSink {
    type Output = ();

    fn prepare(&mut self, plan: &CarBuildPlan) -> Result<(), ChunkStoreError> {
        plan.validate().map_err(ChunkStoreError::InvalidPlan)?;
        let root_metadata = fs::symlink_metadata(&self.root).map_err(ChunkStoreError::Io)?;
        if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
            return Err(ChunkStoreError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "source-tree staging root is not a real directory",
            )));
        }
        if fs::read_dir(&self.root)
            .map_err(ChunkStoreError::Io)?
            .next()
            .is_some()
        {
            return Err(ChunkStoreError::Io(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "source-tree staging root is not empty",
            )));
        }

        self.files = plan.files.clone();
        self.chunk_files = vec![usize::MAX; plan.chunks.len()];
        let mut directories = BTreeSet::<Vec<String>>::new();
        for (file_index, file) in self.files.iter().enumerate() {
            for depth in 1..file.path.len() {
                directories.insert(file.path[..depth].to_vec());
            }
            let end = file
                .first_chunk
                .checked_add(file.chunk_count)
                .ok_or_else(|| {
                    ChunkStoreError::Io(io::Error::other("file chunk range overflow"))
                })?;
            for chunk_file in self
                .chunk_files
                .get_mut(file.first_chunk..end)
                .ok_or_else(|| {
                    ChunkStoreError::Io(io::Error::other("file chunk range is out of bounds"))
                })?
            {
                if *chunk_file != usize::MAX {
                    return Err(ChunkStoreError::Io(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "source-tree chunk belongs to more than one file",
                    )));
                }
                *chunk_file = file_index;
            }
        }
        if self.chunk_files.contains(&usize::MAX) {
            return Err(ChunkStoreError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "source-tree plan does not cover every chunk",
            )));
        }

        for components in directories {
            let path = join_components(&self.root, &components);
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            builder.mode(0o700);
            match builder.create(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    let metadata = fs::symlink_metadata(&path).map_err(ChunkStoreError::Io)?;
                    if metadata.file_type().is_symlink() || !metadata.is_dir() {
                        return Err(ChunkStoreError::Io(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "source-tree parent is not a real directory",
                        )));
                    }
                }
                Err(error) => return Err(ChunkStoreError::Io(error)),
            }
        }
        for index in 0..self.files.len() {
            if self.files[index].size == 0 {
                let target = self.create_target(index)?;
                self.current = Some(target);
                self.finish_target()?;
            }
        }
        Ok(())
    }

    fn write_chunk(
        &mut self,
        index: usize,
        _chunk: &sorafs_car::CarChunk,
        data: &[u8],
    ) -> Result<(), ChunkStoreError> {
        if index != self.next_chunk {
            return Err(ChunkStoreError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                "source-tree chunks arrived out of order",
            )));
        }
        let file_index = *self.chunk_files.get(index).ok_or_else(|| {
            ChunkStoreError::Io(io::Error::other("source-tree chunk index is out of range"))
        })?;
        if self
            .current
            .as_ref()
            .is_some_and(|target| target.file_index != file_index)
        {
            self.finish_target()?;
        }
        if self.current.is_none() {
            self.current = Some(self.create_target(file_index)?);
        }
        let target = self.current.as_mut().ok_or_else(|| {
            ChunkStoreError::Io(io::Error::other("source-tree target was not opened"))
        })?;
        target.file.write_all(data).map_err(ChunkStoreError::Io)?;
        target.written = target
            .written
            .checked_add(u64::try_from(data.len()).map_err(|_| {
                ChunkStoreError::Io(io::Error::other("source-tree write length exceeds u64"))
            })?)
            .ok_or_else(|| {
                ChunkStoreError::Io(io::Error::other("source-tree file length overflow"))
            })?;
        self.next_chunk += 1;
        let file = &self.files[file_index];
        if index + 1 == file.first_chunk + file.chunk_count {
            self.finish_target()?;
        }
        Ok(())
    }

    fn finish(mut self) -> Result<Self::Output, ChunkStoreError> {
        self.finish_target()?;
        if self.next_chunk != self.chunk_files.len() {
            return Err(ChunkStoreError::Io(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "source-tree plan was not fully materialized",
            )));
        }
        sync_tree_directories(&self.root).map_err(ChunkStoreError::Io)
    }
}

fn verify_tree(
    source_path: &Path,
    commitment: &MusubiArchiveCommitmentV1,
    plan: &CarBuildPlan,
) -> Result<(), CacheError> {
    let files = inventory_tree(source_path)?;
    compare_inventory(plan, &files)?;

    let mut payload = DirectoryPayload::new(source_path, &plan.files)
        .map_err(|source| io_error("open cached source payload", source_path, source))?;
    let mut store = ChunkStore::with_profile(plan.chunk_profile);
    store
        .ingest_plan_source(plan, &mut payload)
        .map_err(|error| CacheError::CorruptEntry(error.to_string()))?;
    verify_por(commitment, &store)?;
    drop(payload);

    let mut canonical_payload = DirectoryPayload::new(source_path, &plan.files)
        .map_err(|source| io_error("reopen cached source payload", source_path, source))?;
    let mut reader = SequentialPayloadReader {
        source: &mut canonical_payload,
        offset: 0,
        length: plan.content_length,
    };
    let roots = vec![commitment.root_cid.as_bytes().to_vec()];
    let stats = CarStreamingWriter::with_expected_roots(plan, roots)
        .write_from_reader(&mut reader, io::sink())
        .map_err(|error| CacheError::CorruptEntry(error.to_string()))?;
    if stats.car_size != commitment.car_size
        || stats.car_archive_digest.as_bytes() != commitment.car_digest.as_bytes()
        || stats.root_cids.as_slice() != [commitment.root_cid.as_bytes().as_slice()]
        || stats.dag_codec != DAG_CBOR_CODEC
        || stats.chunk_count != commitment.chunk_count as usize
        || stats.payload_bytes != commitment.content_length
    {
        return Err(CacheError::CorruptEntry(
            "canonical CAR reconstructed from cache differs from commitment".to_owned(),
        ));
    }
    verify_bundle_commitments(source_path, commitment, &files)
}

struct SequentialPayloadReader<'a> {
    source: &'a mut DirectoryPayload,
    offset: u64,
    length: u64,
}

impl Read for SequentialPayloadReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.offset == self.length || buffer.is_empty() {
            return Ok(0);
        }
        let remaining = self.length - self.offset;
        let count = usize::try_from(remaining.min(buffer.len() as u64))
            .expect("bounded payload read fits usize");
        sorafs_car::PayloadSource::read_exact(self.source, self.offset, &mut buffer[..count])
            .map_err(io::Error::other)?;
        self.offset += count as u64;
        Ok(count)
    }
}

fn verify_por(
    commitment: &MusubiArchiveCommitmentV1,
    store: &ChunkStore,
) -> Result<(), CacheError> {
    if store.por_tree().root() != commitment.por_root.as_bytes() {
        return Err(CacheError::CorruptEntry(
            "proof-of-retrievability root mismatch".to_owned(),
        ));
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct FileInventory {
    path: String,
    size: u64,
    digest: [u8; 32],
}

fn inventory_tree(root: &Path) -> Result<Vec<FileInventory>, CacheError> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|source| io_error("inspect cached source root", root, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CacheError::CorruptEntry(
            "cached source root is not a real directory".to_owned(),
        ));
    }
    let mut output = Vec::new();
    let mut entry_count = 0usize;
    inventory_directory(root, root, 0, &mut entry_count, &mut output)?;
    output.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    Ok(output)
}

fn inventory_directory(
    root: &Path,
    directory: &Path,
    depth: usize,
    entry_count: &mut usize,
    output: &mut Vec<FileInventory>,
) -> Result<(), CacheError> {
    if depth > MAX_CACHE_PATH_COMPONENTS {
        return Err(CacheError::CorruptEntry(
            "cache tree exceeds the portable path-depth bound".to_owned(),
        ));
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(directory)
        .map_err(|source| io_error("read cached source directory", directory, source))?
    {
        let entry =
            entry.map_err(|source| io_error("read cached source entry", directory, source))?;
        *entry_count = entry_count
            .checked_add(1)
            .ok_or_else(|| CacheError::CorruptEntry("cache entry count overflow".to_owned()))?;
        if *entry_count > MAX_CACHE_ENTRY_COUNT {
            return Err(CacheError::CorruptEntry(
                "cache tree exceeds the bounded descendant count".to_owned(),
            ));
        }
        entries.push(entry);
    }
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|source| io_error("inspect cached source entry", &path, source))?;
        if metadata.file_type().is_symlink() {
            return Err(CacheError::CorruptEntry(format!(
                "symlink `{}` is forbidden",
                path.display()
            )));
        }
        if metadata.is_dir() {
            inventory_directory(root, &path, depth + 1, entry_count, output)?;
        } else if metadata.is_file() {
            #[cfg(unix)]
            if metadata.nlink() != 1 {
                return Err(CacheError::CorruptEntry(format!(
                    "hard-linked cache file `{}` is forbidden",
                    path.display()
                )));
            }
            let relative = path.strip_prefix(root).map_err(|_| {
                CacheError::CorruptEntry("cache path escaped source root".to_owned())
            })?;
            let path_text = relative
                .components()
                .map(|component| {
                    component.as_os_str().to_str().ok_or_else(|| {
                        CacheError::CorruptEntry("cache path is not UTF-8".to_owned())
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
                .join("/");
            let (size, digest) = hash_regular_file(&path)?;
            output.push(FileInventory {
                path: path_text,
                size,
                digest,
            });
            if output.len() > MAX_CACHE_FILE_COUNT {
                return Err(CacheError::CorruptEntry(
                    "cache tree exceeds the bounded file count".to_owned(),
                ));
            }
        } else {
            return Err(CacheError::CorruptEntry(format!(
                "special cache entry `{}` is forbidden",
                path.display()
            )));
        }
    }
    Ok(())
}

fn verify_compiler_bundle(
    root: &Path,
    node: &MusubiVerificationNodeV1,
    files: &[FileInventory],
) -> Result<(MusubiSemanticReleaseManifestV1, MusubiVerificationLockV1), CacheError> {
    let by_path = files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    let release = by_path
        .get(RELEASE_PATH)
        .ok_or_else(|| CacheError::CorruptEntry("bundle release manifest is missing".to_owned()))?;
    let descriptor_file = by_path.get(DESCRIPTOR_PATH).ok_or_else(|| {
        CacheError::CorruptEntry("bundle artifact descriptor is missing".to_owned())
    })?;
    let verification_lock = by_path.get(VERIFICATION_LOCK_PATH).ok_or_else(|| {
        CacheError::CorruptEntry("bundle verification lock is missing".to_owned())
    })?;
    if release.size > BUNDLE_METADATA_MAX_BYTES
        || verification_lock.size > BUNDLE_METADATA_MAX_BYTES
        || descriptor_file.size > DESCRIPTOR_MAX_BYTES
    {
        return Err(CacheError::CorruptEntry(
            "bundle compiler metadata exceeds its verification bound".to_owned(),
        ));
    }

    let source_files = files
        .iter()
        .filter(|file| !file.path.starts_with(".musubi/"))
        .collect::<Vec<_>>();
    let source_count = u32::try_from(source_files.len())
        .map_err(|_| CacheError::CorruptEntry("source file count exceeds u32".to_owned()))?;
    let source_bytes = source_files.iter().try_fold(0u64, |total, file| {
        total
            .checked_add(file.size)
            .ok_or_else(|| CacheError::CorruptEntry("source byte count overflow".to_owned()))
    })?;
    let source_material_len = source_material_length(&source_files)?;
    let mut source_hasher = blake3::Hasher::new();
    source_hasher.update(SOURCE_TREE_DOMAIN);
    source_hasher.update(&source_material_len.to_be_bytes());
    update_source_material(&mut source_hasher, &source_files);
    let source_digest = MusubiContentDigestV1::new(*source_hasher.finalize().as_bytes());
    if source_digest != node.source_digest {
        return Err(CacheError::CorruptEntry(
            "consumer node source digest does not match the cached tree".to_owned(),
        ));
    }

    let descriptor_bytes =
        read_regular_file_bounded(&root.join(DESCRIPTOR_PATH), DESCRIPTOR_MAX_BYTES)?;
    let mut descriptor_input = descriptor_bytes.as_slice();
    let descriptor = MusubiArtifactDescriptorV1::decode(&mut descriptor_input)
        .map_err(|error| CacheError::CorruptEntry(error.to_string()))?;
    descriptor
        .validate()
        .map_err(|error| CacheError::CorruptEntry(error.to_string()))?;
    if !descriptor_input.is_empty() || descriptor.encode() != descriptor_bytes {
        return Err(CacheError::CorruptEntry(
            "artifact descriptor is not canonical Norito".to_owned(),
        ));
    }
    if descriptor.source_tree_digest != source_digest
        || descriptor.source_file_count != source_count
        || descriptor.source_bytes != source_bytes
    {
        return Err(CacheError::CorruptEntry(
            "artifact descriptor does not match the cached source tree".to_owned(),
        ));
    }

    let release_bytes =
        read_regular_file_bounded(&root.join(RELEASE_PATH), BUNDLE_METADATA_MAX_BYTES)?;
    let mut release_input = release_bytes.as_slice();
    let semantic_release = MusubiSemanticReleaseManifestV1::decode(&mut release_input)
        .map_err(|error| CacheError::CorruptEntry(error.to_string()))?;
    semantic_release
        .validate()
        .map_err(|error| CacheError::CorruptEntry(error.to_string()))?;
    if !release_input.is_empty() || semantic_release.encode() != release_bytes {
        return Err(CacheError::CorruptEntry(
            "semantic release manifest is not canonical Norito".to_owned(),
        ));
    }

    let lock_bytes = read_regular_file_bounded(
        &root.join(VERIFICATION_LOCK_PATH),
        BUNDLE_METADATA_MAX_BYTES,
    )?;
    let mut lock_input = lock_bytes.as_slice();
    let publication_lock = MusubiVerificationLockV1::decode(&mut lock_input)
        .map_err(|error| CacheError::CorruptEntry(error.to_string()))?;
    publication_lock
        .validate()
        .map_err(|error| CacheError::CorruptEntry(error.to_string()))?;
    if !lock_input.is_empty() || publication_lock.encode() != lock_bytes {
        return Err(CacheError::CorruptEntry(
            "verification lock is not canonical Norito".to_owned(),
        ));
    }

    let semantic_requirements = node
        .dependencies
        .iter()
        .map(|edge| {
            if edge.kind != MusubiDependencyKindV1::Normal {
                return Err(CacheError::CorruptEntry(
                    "cached registry node contains a development dependency".to_owned(),
                ));
            }
            Ok(MusubiDependencyReqV1 {
                alias: edge.alias.clone(),
                package: edge.package.clone(),
                requirement: edge.requirement.clone(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let full_release = MusubiReleaseManifestV1 {
        release: semantic_release.release.clone(),
        edition: semantic_release.edition,
        abi: semantic_release.abi,
        dependencies: semantic_release.dependencies.clone(),
        exports: semantic_release.exports.clone(),
        interface_digest: semantic_release.interface_digest,
        metadata: semantic_release.metadata.clone(),
        archive_id: node.archive_id,
        verification_lock_digest: semantic_release.verification_lock_digest,
    };
    full_release
        .validate()
        .map_err(|error| CacheError::CorruptEntry(error.to_string()))?;
    if semantic_release.release != node.release
        || semantic_release.abi != node.abi
        || semantic_release.interface_digest != node.interface_digest
        || semantic_release.dependencies != semantic_requirements
        || semantic_release.verification_lock_digest != publication_lock.digest()
        || publication_lock.root != node.release
        || descriptor.semantic_release_manifest_digest != semantic_release.semantic_digest()
        || descriptor.verification_lock_digest != publication_lock.digest()
        || full_release.release_digest() != node.release_digest
    {
        return Err(CacheError::CorruptEntry(
            "cached bundle does not match the exact consumer release node".to_owned(),
        ));
    }
    Ok((semantic_release, publication_lock))
}

fn load_compiler_sources(
    root: &Path,
    files: &[FileInventory],
) -> Result<(String, Vec<CachedKotodamaSourceV1>), CacheError> {
    let manifest_entry = files
        .binary_search_by(|entry| entry.path.as_str().cmp("Musubi.toml"))
        .ok()
        .and_then(|index| files.get(index))
        .ok_or_else(|| CacheError::CorruptEntry("cached Musubi.toml is missing".to_owned()))?;
    let manifest_bytes = read_regular_file_bounded(
        &root.join("Musubi.toml"),
        manifest_entry.size.min(BUNDLE_METADATA_MAX_BYTES),
    )?;
    if *blake3::hash(&manifest_bytes).as_bytes() != manifest_entry.digest {
        return Err(CacheError::CorruptEntry(
            "cached Musubi.toml changed after source authentication".to_owned(),
        ));
    }
    let manifest = String::from_utf8(manifest_bytes)
        .map_err(|_| CacheError::CorruptEntry("cached Musubi.toml is not UTF-8".to_owned()))?;

    let mut sources = Vec::new();
    let mut source_bytes = 0u64;
    for entry in files
        .iter()
        .filter(|entry| !entry.path.starts_with(".musubi/") && entry.path.ends_with(".ko"))
    {
        source_bytes = source_bytes.checked_add(entry.size).ok_or_else(|| {
            CacheError::CorruptEntry("Kotodama source byte count overflow".to_owned())
        })?;
        if source_bytes > iroha_data_model::musubi::MUSUBI_MAX_SOURCE_PAYLOAD_BYTES_V1 {
            return Err(CacheError::CorruptEntry(
                "Kotodama sources exceed the V1 payload bound".to_owned(),
            ));
        }
        let bytes = read_regular_file_bounded(&root.join(&entry.path), entry.size)?;
        if *blake3::hash(&bytes).as_bytes() != entry.digest {
            return Err(CacheError::CorruptEntry(format!(
                "cached Kotodama source `{}` changed after source authentication",
                entry.path
            )));
        }
        let source = String::from_utf8(bytes).map_err(|_| {
            CacheError::CorruptEntry(format!(
                "cached Kotodama source `{}` is not UTF-8",
                entry.path
            ))
        })?;
        sources.push(CachedKotodamaSourceV1 {
            path: entry.path.clone(),
            source,
        });
    }
    Ok((manifest, sources))
}

fn compare_inventory(plan: &CarBuildPlan, files: &[FileInventory]) -> Result<(), CacheError> {
    if plan.files.len() != files.len() {
        return Err(CacheError::CorruptEntry(format!(
            "cache contains {} files; plan requires {}",
            files.len(),
            plan.files.len()
        )));
    }
    for (planned, actual) in plan.files.iter().zip(files) {
        let path = planned.path.join("/");
        if path != actual.path || planned.size != actual.size {
            return Err(CacheError::CorruptEntry(format!(
                "cache inventory differs at `{}`",
                actual.path
            )));
        }
    }
    Ok(())
}

fn verify_bundle_commitments(
    root: &Path,
    commitment: &MusubiArchiveCommitmentV1,
    files: &[FileInventory],
) -> Result<(), CacheError> {
    let by_path = files
        .iter()
        .map(|file| (file.path.as_str(), file))
        .collect::<BTreeMap<_, _>>();
    let release = by_path
        .get(RELEASE_PATH)
        .ok_or_else(|| CacheError::CorruptEntry("bundle release manifest is missing".to_owned()))?;
    let descriptor_file = by_path.get(DESCRIPTOR_PATH).ok_or_else(|| {
        CacheError::CorruptEntry("bundle artifact descriptor is missing".to_owned())
    })?;
    let verification_lock = by_path.get(VERIFICATION_LOCK_PATH).ok_or_else(|| {
        CacheError::CorruptEntry("bundle verification lock is missing".to_owned())
    })?;
    if descriptor_file.size > DESCRIPTOR_MAX_BYTES {
        return Err(CacheError::CorruptEntry(
            "artifact descriptor exceeds its cache verification bound".to_owned(),
        ));
    }
    let descriptor_path = root.join(DESCRIPTOR_PATH);
    let descriptor_bytes = read_regular_file_bounded(&descriptor_path, DESCRIPTOR_MAX_BYTES)?;
    let mut input = descriptor_bytes.as_slice();
    let descriptor = MusubiArtifactDescriptorV1::decode(&mut input)
        .map_err(|error| CacheError::CorruptEntry(error.to_string()))?;
    if !input.is_empty() || descriptor.encode() != descriptor_bytes {
        return Err(CacheError::CorruptEntry(
            "artifact descriptor is not canonical Norito".to_owned(),
        ));
    }
    if descriptor.version != MUSUBI_ARTIFACT_DESCRIPTOR_VERSION_V1 {
        return Err(CacheError::CorruptEntry(
            "artifact descriptor version is not V1".to_owned(),
        ));
    }
    descriptor
        .validate()
        .map_err(|error| CacheError::CorruptEntry(error.to_string()))?;

    let source_files = files
        .iter()
        .filter(|file| !file.path.starts_with(".musubi/"))
        .collect::<Vec<_>>();
    let source_count = u32::try_from(source_files.len())
        .map_err(|_| CacheError::CorruptEntry("source file count exceeds u32".to_owned()))?;
    let source_bytes = source_files.iter().try_fold(0u64, |total, file| {
        total
            .checked_add(file.size)
            .ok_or_else(|| CacheError::CorruptEntry("source file byte count overflow".to_owned()))
    })?;
    if source_count != commitment.file_count
        || source_count != descriptor.source_file_count
        || source_bytes != descriptor.source_bytes
    {
        return Err(CacheError::CorruptEntry(
            "artifact descriptor source inventory mismatch".to_owned(),
        ));
    }

    let source_material_len = source_material_length(&source_files)?;
    let mut source_hasher = blake3::Hasher::new();
    source_hasher.update(SOURCE_TREE_DOMAIN);
    source_hasher.update(&source_material_len.to_be_bytes());
    update_source_material(&mut source_hasher, &source_files);
    let source_digest = *source_hasher.finalize().as_bytes();
    if commitment.source_tree_digest.as_bytes() != &source_digest
        || descriptor.source_tree_digest.as_bytes() != &source_digest
    {
        return Err(CacheError::CorruptEntry(
            "normalized source-tree digest mismatch".to_owned(),
        ));
    }

    let descriptor_material = framed_descriptor_material(&descriptor_bytes)?;
    let descriptor_digest = domain_digest_bytes(ARTIFACT_DESCRIPTOR_DOMAIN, &descriptor_material);
    if commitment.descriptor_digest.as_bytes() != &descriptor_digest {
        return Err(CacheError::CorruptEntry(
            "artifact descriptor digest mismatch".to_owned(),
        ));
    }

    if release.size > BUNDLE_METADATA_MAX_BYTES
        || verification_lock.size > BUNDLE_METADATA_MAX_BYTES
    {
        return Err(CacheError::CorruptEntry(
            "semantic release or verification lock exceeds its verification bound".to_owned(),
        ));
    }
    let release_path = root.join(RELEASE_PATH);
    let release_bytes = read_regular_file_bounded(&release_path, BUNDLE_METADATA_MAX_BYTES)?;
    let mut release_input = release_bytes.as_slice();
    let semantic_release = MusubiSemanticReleaseManifestV1::decode(&mut release_input)
        .map_err(|error| CacheError::CorruptEntry(error.to_string()))?;
    semantic_release
        .validate()
        .map_err(|error| CacheError::CorruptEntry(error.to_string()))?;
    if !release_input.is_empty() || semantic_release.encode() != release_bytes {
        return Err(CacheError::CorruptEntry(
            "semantic release manifest is not canonical Norito".to_owned(),
        ));
    }
    if descriptor.semantic_release_manifest_digest != semantic_release.semantic_digest() {
        return Err(CacheError::CorruptEntry(
            "semantic release-manifest digest mismatch".to_owned(),
        ));
    }
    let lock_path = root.join(VERIFICATION_LOCK_PATH);
    let lock_bytes = read_regular_file_bounded(&lock_path, BUNDLE_METADATA_MAX_BYTES)?;
    let mut lock_input = lock_bytes.as_slice();
    let lock = MusubiVerificationLockV1::decode(&mut lock_input)
        .map_err(|error| CacheError::CorruptEntry(error.to_string()))?;
    lock.validate()
        .map_err(|error| CacheError::CorruptEntry(error.to_string()))?;
    if !lock_input.is_empty() || lock.encode() != lock_bytes {
        return Err(CacheError::CorruptEntry(
            "verification lock is not canonical Norito".to_owned(),
        ));
    }
    if descriptor.verification_lock_digest != lock.digest()
        || semantic_release.verification_lock_digest != lock.digest()
        || semantic_release.release != lock.root
    {
        return Err(CacheError::CorruptEntry(
            "normalized verification-lock digest mismatch".to_owned(),
        ));
    }

    let bundle_material_len = frame_length(BUNDLE_DOMAIN.len() as u64)?
        .checked_add(frame_length(release.size)?)
        .and_then(|value| value.checked_add(frame_length(descriptor_material.len() as u64).ok()?))
        .and_then(|value| value.checked_add(frame_length(source_material_len).ok()?))
        .and_then(|value| value.checked_add(frame_length(verification_lock.size).ok()?))
        .ok_or_else(|| CacheError::CorruptEntry("bundle transcript length overflow".to_owned()))?;
    let mut bundle_hasher = blake3::Hasher::new();
    bundle_hasher.update(BUNDLE_DOMAIN);
    bundle_hasher.update(&bundle_material_len.to_be_bytes());
    update_frame_bytes(&mut bundle_hasher, BUNDLE_DOMAIN);
    bundle_hasher.update(&release.size.to_be_bytes());
    hash_file_into(&release_path, &mut bundle_hasher)?;
    update_frame_bytes(&mut bundle_hasher, &descriptor_material);
    bundle_hasher.update(&source_material_len.to_be_bytes());
    update_source_material(&mut bundle_hasher, &source_files);
    bundle_hasher.update(&verification_lock.size.to_be_bytes());
    hash_file_into(&lock_path, &mut bundle_hasher)?;
    let bundle_digest = *bundle_hasher.finalize().as_bytes();
    if commitment.bundle_digest.as_bytes() != &bundle_digest {
        return Err(CacheError::CorruptEntry(
            "canonical bundle digest mismatch".to_owned(),
        ));
    }
    Ok(())
}

fn source_material_length(files: &[&FileInventory]) -> Result<u64, CacheError> {
    files.iter().try_fold(
        frame_length(SOURCE_TREE_DOMAIN.len() as u64)? + 4,
        |total, file| {
            total
                .checked_add(frame_length(file.path.len() as u64)?)
                .and_then(|value| value.checked_add(8 + 32))
                .ok_or_else(|| {
                    CacheError::CorruptEntry("source transcript length overflow".to_owned())
                })
        },
    )
}

fn update_source_material(hasher: &mut blake3::Hasher, files: &[&FileInventory]) {
    update_frame_bytes(hasher, SOURCE_TREE_DOMAIN);
    hasher.update(
        &u32::try_from(files.len())
            .expect("validated Musubi V1 source file count fits u32")
            .to_be_bytes(),
    );
    for file in files {
        update_frame_bytes(hasher, file.path.as_bytes());
        hasher.update(&file.size.to_be_bytes());
        hasher.update(&file.digest);
    }
}

fn framed_descriptor_material(bytes: &[u8]) -> Result<Vec<u8>, CacheError> {
    let capacity = usize::try_from(
        frame_length(ARTIFACT_DESCRIPTOR_DOMAIN.len() as u64)?
            .checked_add(frame_length(bytes.len() as u64)?)
            .ok_or_else(|| {
                CacheError::CorruptEntry("descriptor transcript length overflow".to_owned())
            })?,
    )
    .map_err(|_| CacheError::CorruptEntry("descriptor transcript exceeds host width".to_owned()))?;
    let mut output = Vec::with_capacity(capacity);
    output.extend_from_slice(&(ARTIFACT_DESCRIPTOR_DOMAIN.len() as u64).to_be_bytes());
    output.extend_from_slice(ARTIFACT_DESCRIPTOR_DOMAIN);
    output.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    output.extend_from_slice(bytes);
    Ok(output)
}

fn domain_digest_bytes(domain: &[u8], material: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&(material.len() as u64).to_be_bytes());
    hasher.update(material);
    *hasher.finalize().as_bytes()
}

fn update_frame_bytes(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

fn frame_length(length: u64) -> Result<u64, CacheError> {
    length
        .checked_add(8)
        .ok_or_else(|| CacheError::CorruptEntry("commitment frame length overflow".to_owned()))
}

fn hash_regular_file(path: &Path) -> Result<(u64, [u8; 32]), CacheError> {
    let mut hasher = blake3::Hasher::new();
    let size = hash_file_into(path, &mut hasher)?;
    Ok((size, *hasher.finalize().as_bytes()))
}

fn hash_file_into(path: &Path, hasher: &mut blake3::Hasher) -> Result<u64, CacheError> {
    let (mut file, before) = open_regular_file_no_follow(path)?;
    let mut total = 0u64;
    let mut buffer = [0u8; IO_BUFFER_BYTES];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| io_error("read cached source file", path, source))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        total = total
            .checked_add(read as u64)
            .ok_or_else(|| CacheError::CorruptEntry("file length overflow".to_owned()))?;
    }
    validate_open_regular_file(path, &file)
        .map_err(|source| io_error("revalidate cached source file", path, source))?;
    let after = file
        .metadata()
        .map_err(|source| io_error("inspect cached source handle", path, source))?;
    if !same_snapshot(&before, &after) || total != before.len() {
        return Err(CacheError::CorruptEntry(format!(
            "cache file `{}` changed while being read",
            path.display()
        )));
    }
    Ok(total)
}

fn read_regular_file_bounded(path: &Path, maximum: u64) -> Result<Vec<u8>, CacheError> {
    let (mut file, before) = open_regular_file_no_follow(path)?;
    if before.len() > maximum {
        return Err(CacheError::CorruptEntry(format!(
            "cache file `{}` exceeds {maximum} bytes",
            path.display()
        )));
    }
    let capacity = usize::try_from(before.len())
        .map_err(|_| CacheError::CorruptEntry("file exceeds host width".to_owned()))?;
    let mut bytes = Vec::with_capacity(capacity);
    file.read_to_end(&mut bytes)
        .map_err(|source| io_error("read cached source file", path, source))?;
    validate_open_regular_file(path, &file)
        .map_err(|source| io_error("revalidate cached source file", path, source))?;
    let after = file
        .metadata()
        .map_err(|source| io_error("inspect cached source handle", path, source))?;
    if !same_snapshot(&before, &after) || bytes.len() as u64 != before.len() {
        return Err(CacheError::CorruptEntry(format!(
            "cache file `{}` changed while being read",
            path.display()
        )));
    }
    Ok(bytes)
}

fn open_regular_file_no_follow(path: &Path) -> Result<(File, fs::Metadata), CacheError> {
    let linked = fs::symlink_metadata(path)
        .map_err(|source| io_error("inspect cached source file", path, source))?;
    if linked.file_type().is_symlink() || !linked.is_file() {
        return Err(CacheError::CorruptEntry(format!(
            "cache entry `{}` is not a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    if linked.nlink() != 1 {
        return Err(CacheError::CorruptEntry(format!(
            "cache file `{}` has more than one hard link",
            path.display()
        )));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow(&mut options);
    let file = options
        .open(path)
        .map_err(|source| io_error("open cached source file", path, source))?;
    let opened = file
        .metadata()
        .map_err(|source| io_error("inspect cached source handle", path, source))?;
    if !same_snapshot(&linked, &opened) {
        return Err(CacheError::CorruptEntry(format!(
            "cache file `{}` changed while opening",
            path.display()
        )));
    }
    Ok((file, opened))
}

fn open_new_regular_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    set_no_follow(&mut options);
    #[cfg(unix)]
    options.mode(0o600);
    let file = options.open(path)?;
    validate_open_regular_file(path, &file)?;
    Ok(file)
}

fn validate_open_regular_file(path: &Path, file: &File) -> io::Result<()> {
    let linked = fs::symlink_metadata(path)?;
    let opened = file.metadata()?;
    if linked.file_type().is_symlink()
        || !linked.is_file()
        || !opened.is_file()
        || !same_file(&linked, &opened)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "cache file identity is not a stable regular file",
        ));
    }
    #[cfg(unix)]
    if linked.nlink() != 1 || opened.nlink() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "cache file must have exactly one hard link",
        ));
    }
    Ok(())
}

fn create_staging_directory(parent: &Path) -> Result<(PathBuf, fs::Metadata), CacheError> {
    for _ in 0..TEMP_RETRIES {
        let path = allocate_candidate(parent, ".src", "partial");
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        builder.mode(0o700);
        match builder.create(&path) {
            Ok(()) => {
                let metadata = fs::symlink_metadata(&path)
                    .map_err(|source| io_error("inspect source staging tree", &path, source))?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(CacheError::UnsafeDescendant(path));
                }
                sync_directory(parent)
                    .map_err(|source| io_error("sync archive cache directory", parent, source))?;
                return Ok((path, metadata));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => return Err(io_error("create source staging tree", &path, source)),
        }
    }
    Err(CacheError::Io {
        operation: "allocate source staging tree",
        path: parent.to_path_buf(),
        source: io::Error::new(io::ErrorKind::AlreadyExists, "temporary name collision"),
    })
}

fn create_temporary_file(
    parent: &Path,
    prefix: &str,
) -> Result<(PathBuf, File, fs::Metadata), CacheError> {
    for _ in 0..TEMP_RETRIES {
        let path = allocate_candidate(parent, prefix, "partial");
        match open_new_regular_file(&path) {
            Ok(file) => {
                let metadata = file
                    .metadata()
                    .map_err(|source| io_error("inspect cache temporary", &path, source))?;
                sync_directory(parent)
                    .map_err(|source| io_error("sync cache temporary parent", parent, source))?;
                return Ok((path, file, metadata));
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(source) => return Err(io_error("create cache temporary", &path, source)),
        }
    }
    Err(CacheError::Io {
        operation: "allocate cache temporary",
        path: parent.to_path_buf(),
        source: io::Error::new(io::ErrorKind::AlreadyExists, "temporary name collision"),
    })
}

fn allocate_absent_path(parent: &Path, prefix: &str) -> Result<PathBuf, CacheError> {
    for _ in 0..TEMP_RETRIES {
        let candidate = allocate_candidate(parent, prefix, "entry");
        match fs::symlink_metadata(&candidate) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(candidate),
            Ok(_) => continue,
            Err(source) => {
                return Err(io_error(
                    "inspect cache quarantine destination",
                    &candidate,
                    source,
                ));
            }
        }
    }
    Err(CacheError::Io {
        operation: "allocate cache quarantine destination",
        path: parent.to_path_buf(),
        source: io::Error::new(io::ErrorKind::AlreadyExists, "temporary name collision"),
    })
}

fn allocate_candidate(parent: &Path, prefix: &str, suffix: &str) -> PathBuf {
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    parent.join(format!(
        "{prefix}.{}.{sequence:016x}.{suffix}",
        std::process::id()
    ))
}

struct InstallGuard {
    staging_path: PathBuf,
    staging_identity: fs::Metadata,
    payload_path: PathBuf,
    payload_identity: DirectoryIdentity,
    staging_published: bool,
    payload_removed: bool,
}

impl InstallGuard {
    fn new(
        staging_path: PathBuf,
        staging_identity: fs::Metadata,
        payload_path: PathBuf,
        payload_identity: fs::Metadata,
    ) -> Self {
        Self {
            staging_path,
            staging_identity,
            payload_path,
            payload_identity: DirectoryIdentity::capture(&payload_identity),
            staging_published: false,
            payload_removed: false,
        }
    }

    fn cleanup_staging(&mut self) {
        if !self.staging_published
            && fs::symlink_metadata(&self.staging_path)
                .is_ok_and(|current| same_file(&self.staging_identity, &current))
        {
            let _ = remove_validated_tree(&self.staging_path);
        }
        self.staging_published = true;
    }
}

impl Drop for InstallGuard {
    fn drop(&mut self) {
        if !self.payload_removed
            && fs::symlink_metadata(&self.payload_path).is_ok_and(|metadata| {
                metadata.is_file() && self.payload_identity.matches(&metadata)
            })
        {
            let _ = fs::remove_file(&self.payload_path);
        }
        self.cleanup_staging();
    }
}

fn remove_verified_file(path: &Path, identity: &DirectoryIdentity) -> Result<(), CacheError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| io_error("inspect cache temporary", path, source))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || !identity.matches(&metadata) {
        return Err(CacheError::UnsafeDescendant(path.to_path_buf()));
    }
    fs::remove_file(path).map_err(|source| io_error("remove cache temporary", path, source))?;
    let parent = path.parent().ok_or_else(|| {
        CacheError::CorruptEntry("cache temporary has no parent directory".to_owned())
    })?;
    sync_directory(parent).map_err(|source| io_error("sync cache temporary parent", parent, source))
}

fn validate_mutable_tree(root: &Path) -> Result<(), CacheError> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|source| io_error("inspect cache mutation root", root, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CacheError::UnsafeDescendant(root.to_path_buf()));
    }
    validate_mutable_directory(root)
}

fn validate_mutable_directory(directory: &Path) -> Result<(), CacheError> {
    for entry in fs::read_dir(directory)
        .map_err(|source| io_error("read cache mutation candidate", directory, source))?
    {
        let entry = entry
            .map_err(|source| io_error("read cache mutation descendant", directory, source))?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|source| io_error("inspect cache mutation descendant", &path, source))?;
        if metadata.file_type().is_symlink() {
            return Err(CacheError::UnsafeDescendant(path));
        }
        if metadata.is_dir() {
            validate_mutable_directory(&path)?;
        } else if metadata.is_file() {
            #[cfg(unix)]
            if metadata.nlink() != 1 {
                return Err(CacheError::UnsafeDescendant(path));
            }
        } else {
            return Err(CacheError::UnsafeDescendant(path));
        }
    }
    Ok(())
}

fn remove_validated_tree(root: &Path) -> Result<(), CacheError> {
    validate_mutable_tree(root)?;
    remove_directory_contents(root)?;
    fs::remove_dir(root).map_err(|source| io_error("remove cache directory", root, source))
}

fn remove_directory_contents(directory: &Path) -> Result<(), CacheError> {
    #[cfg(unix)]
    fs::set_permissions(directory, fs::Permissions::from_mode(0o700))
        .map_err(|source| io_error("unlock cache directory for removal", directory, source))?;
    let entries = fs::read_dir(directory)
        .map_err(|source| io_error("read cache directory for removal", directory, source))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| io_error("read cache descendant for removal", directory, source))?;
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|source| io_error("revalidate cache removal descendant", &path, source))?;
        if metadata.file_type().is_symlink() {
            return Err(CacheError::UnsafeDescendant(path));
        }
        if metadata.is_dir() {
            remove_directory_contents(&path)?;
            fs::remove_dir(&path)
                .map_err(|source| io_error("remove cache descendant directory", &path, source))?;
        } else if metadata.is_file() {
            #[cfg(unix)]
            if metadata.nlink() != 1 {
                return Err(CacheError::UnsafeDescendant(path));
            }
            fs::remove_file(&path)
                .map_err(|source| io_error("remove cache descendant file", &path, source))?;
        } else {
            return Err(CacheError::UnsafeDescendant(path));
        }
    }
    Ok(())
}

fn make_tree_immutable_and_sync(root: &Path) -> Result<(), CacheError> {
    let mut directories = Vec::new();
    collect_directories(root, &mut directories)?;
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in &directories {
        sync_directory(directory)
            .map_err(|source| io_error("sync immutable source directory", directory, source))?;
        #[cfg(unix)]
        fs::set_permissions(directory, fs::Permissions::from_mode(0o555))
            .map_err(|source| io_error("make source directory immutable", directory, source))?;
        sync_directory(directory)
            .map_err(|source| io_error("resync immutable source directory", directory, source))?;
    }
    Ok(())
}

fn sync_tree_directories(root: &Path) -> io::Result<()> {
    let mut directories = Vec::new();
    collect_directories_io(root, &mut directories)?;
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        sync_directory(&directory)?;
    }
    Ok(())
}

fn collect_directories(root: &Path, output: &mut Vec<PathBuf>) -> Result<(), CacheError> {
    collect_directories_io(root, output)
        .map_err(|source| io_error("inventory source directories", root, source))
}

fn collect_directories_io(root: &Path, output: &mut Vec<PathBuf>) -> io::Result<()> {
    let metadata = fs::symlink_metadata(root)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "source directory is not a real directory",
        ));
    }
    output.push(root.to_path_buf());
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "source tree contains a symlink",
            ));
        }
        if metadata.is_dir() {
            collect_directories_io(&path, output)?;
        } else if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "source tree contains a special file",
            ));
        }
    }
    Ok(())
}

fn join_components(root: &Path, components: &[String]) -> PathBuf {
    let mut path = root.to_path_buf();
    for component in components {
        path.push(component);
    }
    path
}

fn decode_archive_directory_name(name: &str) -> Option<ArchiveId> {
    if name.len() != 64
        || name
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let mut bytes = [0u8; 32];
    hex::decode_to_slice(name, &mut bytes).ok()?;
    Some(ArchiveId::new(bytes))
}

fn sync_directory(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

fn same_file(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(not(unix))]
    {
        let _ = (left, right);
        false
    }
}

fn same_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        same_file(left, right)
            && left.file_type() == right.file_type()
            && left.len() == right.len()
            && left.mtime() == right.mtime()
            && left.mtime_nsec() == right.mtime_nsec()
            && left.ctime() == right.ctime()
            && left.ctime_nsec() == right.ctime_nsec()
            && left.nlink() == right.nlink()
    }
    #[cfg(not(unix))]
    {
        let _ = (left, right);
        false
    }
}

fn set_no_follow(options: &mut OpenOptions) {
    #[cfg(unix)]
    options.custom_flags(platform_no_follow_flag());
}

#[cfg(any(target_os = "linux", target_os = "android"))]
const fn platform_no_follow_flag() -> i32 {
    0o400000
}

#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
const fn platform_no_follow_flag() -> i32 {
    0x100
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
const fn platform_no_follow_flag() -> i32 {
    0
}

fn rename_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    // The parent is private and all Musubi writers serialize through this
    // retained advisory lock, so the following rename has an absent target and
    // publishes the directory atomically. `create(true)` is safe here because
    // O_NOFOLLOW is applied and the lock never carries package-controlled data.
    // TODO: Replace the locked check with a safe standard-library
    // rename-no-replace primitive once Rust exposes one for directories.
    let parent = source
        .parent()
        .filter(|parent| destination.parent() == Some(*parent))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "cache rename must remain within one trusted parent",
            )
        })?;
    let lock_path = parent.join(".publication.lock");
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    set_no_follow(&mut options);
    #[cfg(unix)]
    options.mode(0o600);
    let lock = options.open(&lock_path)?;
    validate_open_regular_file(&lock_path, &lock)?;
    lock.lock()?;
    let result = match fs::symlink_metadata(destination) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::rename(source, destination),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "immutable cache destination already exists",
        )),
        Err(error) => Err(error),
    };
    let unlock = File::unlock(&lock);
    result.and(unlock)
}

fn io_error(operation: &'static str, path: &Path, source: io::Error) -> CacheError {
    CacheError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, path::Path};

    use iroha_data_model::{
        musubi::{
            MUSUBI_REGISTRY_VERSION_V1, MusubiAbiBindingV1, MusubiContentDigestV1,
            MusubiKotodamaEditionV1, MusubiPackageIdV1, MusubiPackageScopeV1, MusubiReleaseIdV1,
            MusubiReleaseManifestV1, MusubiReleaseMetadataV1, MusubiSemanticReleaseManifestV1,
            MusubiVerificationLockV1, MusubiVerificationNodeV1,
        },
        nexus::DataSpaceId,
        sorafs::pin_registry::{ChunkerProfileHandle, ManifestRootCid},
    };
    use tempfile::TempDir;

    use super::*;
    use crate::package::{PackageCar, PackageLayout, plan_package};

    struct Fixture {
        car: PackageCar,
        commitment: MusubiArchiveCommitmentV1,
        semantic: MusubiSemanticReleaseManifestV1,
    }

    fn fixture(temp: &TempDir, name: &str) -> Fixture {
        let package_root = temp.path().join(name);
        fs::create_dir_all(package_root.join("src")).expect("create fixture source tree");
        fs::write(
            package_root.join("src/lib.ko"),
            format!("fn {name}() {{}}\n"),
        )
        .expect("write fixture source");
        let mut layout = PackageLayout::new(&package_root);
        layout.set_library("src");
        let manifest =
            format!("manifest-version = 1\n\n[package]\nname = \"{name}\"\nversion = \"1.0.0\"\n");
        let lock = "schema = \"musubi-lock\"\nversion = 1\n";
        let package = MusubiPackageIdV1::new(
            DataSpaceId::new(7),
            MusubiPackageScopeV1::DataspaceRoot,
            name.parse().expect("fixture package name"),
        );
        let release = MusubiReleaseIdV1::new(package, "1.0.0".parse().expect("version"));
        let verification_lock = MusubiVerificationLockV1 {
            schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
            version: MUSUBI_REGISTRY_VERSION_V1,
            root: release.clone(),
            root_dependencies: Vec::new(),
            nodes: Vec::new(),
        };
        let semantic = MusubiSemanticReleaseManifestV1 {
            release,
            edition: MusubiKotodamaEditionV1::V1,
            abi: MusubiAbiBindingV1::new([8; 32]).expect("ABI"),
            dependencies: Vec::new(),
            exports: Vec::new(),
            interface_digest: MusubiContentDigestV1::new([9; 32]),
            metadata: MusubiReleaseMetadataV1::default(),
            verification_lock_digest: verification_lock.digest(),
        };
        let car = plan_package(&layout, &manifest, lock)
            .expect("plan fixture")
            .into_car(&semantic, &verification_lock)
            .expect("encode fixture CAR");
        let descriptor = sorafs_car::chunker_registry::default_descriptor();
        let root_cid = ManifestRootCid::try_from(car.stats().root_cids[0].clone())
            .expect("canonical fixture root");
        let por_root =
            sorafs_car::compute_por_root(car.payload(), car.plan()).expect("fixture PoR root");
        let commitment = MusubiArchiveCommitmentV1 {
            root_cid,
            chunker: ChunkerProfileHandle {
                profile_id: descriptor.id.0,
                namespace: descriptor.namespace.to_owned(),
                name: descriptor.name.to_owned(),
                semver: descriptor.semver.to_owned(),
                multihash_code: descriptor.multihash_code,
            },
            chunk_plan_digest: MusubiContentDigestV1::new(compute_chunk_plan_digest_sha3(
                &car.plan().chunks,
            )),
            por_root: MusubiContentDigestV1::new(por_root),
            content_length: car.plan().content_length,
            car_digest: MusubiContentDigestV1::new(*car.stats().car_archive_digest.as_bytes()),
            car_size: car.stats().car_size,
            bundle_digest: car.commitments().bundle_digest(),
            source_tree_digest: car.commitments().source_tree_digest(),
            descriptor_digest: car.commitments().descriptor_digest(),
            file_count: u32::try_from(car.source_file_count()).expect("fixture file count fits"),
            chunk_count: u32::try_from(car.plan().chunks.len()).expect("fixture chunks fit"),
        };
        commitment.validate().expect("valid fixture commitment");
        Fixture {
            car,
            commitment,
            semantic,
        }
    }

    fn cache(temp: &TempDir) -> MusubiCache {
        MusubiCache::open(temp.path().join("user-cache")).expect("open fixture cache")
    }

    #[test]
    fn installs_under_archive_id_and_is_idempotent() {
        let temp = tempfile::tempdir().expect("tempdir");
        let fixture = fixture(&temp, "alpha");
        let cache = cache(&temp);
        let archive_id = fixture.commitment.archive_id();
        let expected = cache
            .root()
            .join(REGISTRY_DIRECTORY)
            .join(hex::encode(archive_id.as_bytes()))
            .join(SOURCE_DIRECTORY);

        let outcome = cache
            .install(
                &fixture.commitment,
                fixture.car.plan(),
                Cursor::new(fixture.car.bytes()),
            )
            .expect("install archive");
        assert_eq!(
            outcome,
            InstallOutcome::Installed(CacheEntry {
                archive_id,
                source_path: expected.clone(),
            })
        );
        assert_eq!(
            fs::read_to_string(expected.join("src/lib.ko")).expect("read cached source"),
            "fn alpha() {}\n"
        );
        assert_eq!(
            cache
                .install(
                    &fixture.commitment,
                    fixture.car.plan(),
                    Cursor::new(fixture.car.bytes()),
                )
                .expect("idempotent install"),
            InstallOutcome::AlreadyPresent(CacheEntry {
                archive_id,
                source_path: expected,
            })
        );
    }

    #[test]
    fn compiler_load_reauthenticates_consumer_node_and_rejects_tampering() {
        let temp = tempfile::tempdir().expect("tempdir");
        let fixture = fixture(&temp, "compiler-cache");
        let cache = cache(&temp);
        cache
            .install(
                &fixture.commitment,
                fixture.car.plan(),
                Cursor::new(fixture.car.bytes()),
            )
            .expect("install compiler fixture");
        let release = MusubiReleaseManifestV1 {
            release: fixture.semantic.release.clone(),
            edition: fixture.semantic.edition,
            abi: fixture.semantic.abi,
            dependencies: fixture.semantic.dependencies.clone(),
            exports: fixture.semantic.exports.clone(),
            interface_digest: fixture.semantic.interface_digest,
            metadata: fixture.semantic.metadata.clone(),
            archive_id: fixture.commitment.archive_id(),
            verification_lock_digest: fixture.semantic.verification_lock_digest,
        };
        let node = MusubiVerificationNodeV1 {
            release: fixture.semantic.release.clone(),
            release_digest: release.release_digest(),
            archive_id: fixture.commitment.archive_id(),
            source_digest: fixture.commitment.source_tree_digest,
            interface_digest: fixture.semantic.interface_digest,
            abi: fixture.semantic.abi,
            dependencies: Vec::new(),
        };

        let loaded = cache
            .load_compiler_package(&node)
            .expect("load authenticated compiler package");
        assert_eq!(loaded.semantic_release, fixture.semantic);
        assert_eq!(loaded.publication_lock.root, node.release);
        assert_eq!(loaded.source_path, cache.source_path(&node.archive_id));
        assert!(loaded.manifest.starts_with("manifest-version = 1"));
        assert_eq!(loaded.kotodama_sources.len(), 1);
        assert_eq!(loaded.kotodama_sources[0].path, "src/lib.ko");

        let source = loaded.source_path.join("src/lib.ko");
        make_writable(&source);
        fs::write(&source, b"fn substituted() {}\n").expect("tamper compiler source");
        let error = cache
            .load_compiler_package(&node)
            .expect_err("tampered source digest must fail closed");
        assert!(matches!(error, CacheError::CorruptEntry(_)));
    }

    #[test]
    fn rejects_corrupt_car_without_publishing_source() {
        let temp = tempfile::tempdir().expect("tempdir");
        let fixture = fixture(&temp, "bravo");
        let cache = cache(&temp);
        let mut bytes = fixture.car.bytes().to_vec();
        let last = bytes.last_mut().expect("nonempty CAR");
        *last ^= 0x80;

        let error = cache
            .install(&fixture.commitment, fixture.car.plan(), Cursor::new(bytes))
            .expect_err("corrupt archive must fail");
        assert!(matches!(error, CacheError::InvalidArchive(_)));
        assert!(!cache.source_path(&fixture.commitment.archive_id()).exists());
    }

    #[test]
    fn rejects_plan_traversal_before_reading_archive() {
        let temp = tempfile::tempdir().expect("tempdir");
        let fixture = fixture(&temp, "charlie");
        let cache = cache(&temp);
        let mut plan = fixture.car.plan().clone();
        plan.files[0].path = vec!["..".to_owned(), "escape".to_owned()];

        let error = cache
            .install(&fixture.commitment, &plan, Cursor::new(Vec::<u8>::new()))
            .expect_err("traversing plan must fail before extraction");
        assert!(matches!(error, CacheError::InvalidPlan(_)));
        assert!(!cache.root().join("escape").exists());
    }

    #[test]
    fn repair_quarantines_only_structurally_safe_corruption() {
        let temp = tempfile::tempdir().expect("tempdir");
        let fixture = fixture(&temp, "delta");
        let cache = cache(&temp);
        cache
            .install(
                &fixture.commitment,
                fixture.car.plan(),
                Cursor::new(fixture.car.bytes()),
            )
            .expect("install fixture");
        let source = cache.source_path(&fixture.commitment.archive_id());
        let target = source.join("src/lib.ko");
        make_writable(&target);
        fs::write(&target, b"corrupt").expect("corrupt cached source");

        let RepairOutcome::Quarantined { path } = cache
            .repair(&fixture.commitment, fixture.car.plan())
            .expect("quarantine safe corruption")
        else {
            panic!("corrupt entry should be quarantined");
        };
        assert!(!source.exists());
        assert!(path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn repair_refuses_symlink_descendant() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("tempdir");
        let fixture = fixture(&temp, "echo");
        let cache = cache(&temp);
        cache
            .install(
                &fixture.commitment,
                fixture.car.plan(),
                Cursor::new(fixture.car.bytes()),
            )
            .expect("install fixture");
        let source = cache.source_path(&fixture.commitment.archive_id());
        let source_dir = source.join("src");
        make_writable(&source_dir);
        let target = source_dir.join("lib.ko");
        fs::remove_file(&target).expect("remove cached target");
        symlink(temp.path().join("outside"), &target).expect("create hostile symlink");

        let error = cache
            .repair(&fixture.commitment, fixture.car.plan())
            .expect_err("unsafe descendant must remain untouched");
        assert!(matches!(error, CacheError::UnsafeDescendant(path) if path == target));
        assert!(source.exists());
    }

    #[test]
    fn prune_accepts_only_typed_retention_and_ignores_unknown_names() {
        let temp = tempfile::tempdir().expect("tempdir");
        let first = fixture(&temp, "foxtrot");
        let second = fixture(&temp, "golf");
        let cache = cache(&temp);
        for fixture in [&first, &second] {
            cache
                .install(
                    &fixture.commitment,
                    fixture.car.plan(),
                    Cursor::new(fixture.car.bytes()),
                )
                .expect("install fixture");
        }
        let unknown = cache.registry_root.join("not-from-a-lock-path");
        fs::create_dir(&unknown).expect("create unknown cache entry");
        let retained = BTreeSet::from([first.commitment.archive_id()]);

        let report = cache.prune(&retained).expect("prune cache");
        assert_eq!(report.removed, vec![second.commitment.archive_id()]);
        assert!(cache.source_path(&first.commitment.archive_id()).exists());
        assert!(!cache.source_path(&second.commitment.archive_id()).exists());
        assert!(
            unknown.exists(),
            "unknown names must never be deletion targets"
        );
    }

    fn make_writable(path: &Path) {
        #[cfg(unix)]
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .expect("make fixture writable");
    }
}
