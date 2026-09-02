//! Immutable crash-recovery records for Sumeragi v2 height contexts.
//!
//! The safety WAL deliberately stores reducer facts rather than mutable
//! configuration. A node must nevertheless recover the exact roster, powers,
//! leader seed, DA layout, and proofs of possession that were frozen before it
//! can authenticate that WAL. This store persists those canonical inputs before
//! the corresponding WAL is opened and never overwrites a conflicting height.
use super::v2::VerifiedHeightContext;
use crate::secure_file_metadata::{self, SecureMetadata};
use iroha_crypto::Hash;
use iroha_data_model::block::consensus_v2 as wire;
use norito::codec::{Decode, DecodeAll, Encode};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind, Read, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;
const FILE_MAGIC: &[u8; 8] = b"SUMV2CTX";
const FRAME_VERSION: u16 = 1;
const HASH_LEN: usize = 32;
// Sumeragi v2 admits only BLS-normal validators. Its PoP is one canonical G2
// signature, not an arbitrary consensus-signature-sized blob.
const BLS_NORMAL_POP_BYTES: usize = 96;
const HEADER_LEN: usize = FILE_MAGIC.len() + 2 + 8 + HASH_LEN;
// One context may carry both current and next-epoch rosters, one parent QC signer set, and one
// roster-aligned PoP vector. Eight maximum-size consensus blobs per validator is deliberately
// generous for the v1 schema while keeping hostile on-disk lengths below a deterministic 9 MiB
// allocation ceiling.
const MAX_CONTEXT_BYTES_PER_VALIDATOR: usize = wire::MAX_CONSENSUS_SIGNATURE_BYTES * 8;
const MAX_CONTEXT_FIXED_BYTES: usize = 64 * 1024;
const MAX_CONTEXT_PAYLOAD_BYTES: usize =
    MAX_CONTEXT_FIXED_BYTES + wire::MAX_VALIDATORS_PER_HEIGHT * MAX_CONTEXT_BYTES_PER_VALIDATOR;
const MAX_CONTEXT_FRAME_BYTES: usize = HEADER_LEN + MAX_CONTEXT_PAYLOAD_BYTES;
/// Canonical V1 context and PoPs required to reopen one reducer height.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(crate) struct PersistedHeightContext {
    format_version: u16,
    context: wire::HeightContext,
    proofs_of_possession: Vec<Vec<u8>>,
}
impl PersistedHeightContext {
    /// Snapshot an already verified context without weakening its provenance.
    pub(crate) fn from_verified(context: &VerifiedHeightContext) -> Self {
        Self {
            format_version: FRAME_VERSION,
            context: context.context().clone(),
            proofs_of_possession: context.proofs_of_possession().to_vec(),
        }
    }
    /// Borrow the frozen wire context.
    pub(crate) const fn context(&self) -> &wire::HeightContext {
        &self.context
    }
    /// Borrow PoPs in frozen-roster order.
    pub(crate) fn proofs_of_possession(&self) -> &[Vec<u8>] {
        &self.proofs_of_possession
    }
    fn validate_layout(&self) -> Result<(), V2ContextStoreError> {
        if self.format_version != FRAME_VERSION {
            return Err(V2ContextStoreError::UnsupportedVersion(self.format_version));
        }
        self.context.validate().map_err(V2ContextStoreError::Wire)?;
        if self.proofs_of_possession.len() != self.context.roster.len() {
            return Err(V2ContextStoreError::ProofCountMismatch);
        }
        if self
            .proofs_of_possession
            .iter()
            .any(|proof| proof.len() != BLS_NORMAL_POP_BYTES)
        {
            return Err(V2ContextStoreError::InvalidProofLength);
        }
        Ok(())
    }
}
/// Append-only context store rooted beside Kura's v2 finality sidecars.
#[derive(Clone, Debug)]
pub(crate) struct V2ContextStore {
    root: PathBuf,
    root_identity: SecureMetadata,
    directory: PathBuf,
    directory_identity: SecureMetadata,
    #[cfg(test)]
    lookup_pause: std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<ContextIoPause>>>>,
    #[cfg(test)]
    read_pause: std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<ContextIoPause>>>>,
    #[cfg(test)]
    publication_pause: std::sync::Arc<std::sync::Mutex<Option<std::sync::Arc<ContextIoPause>>>>,
}
#[cfg(test)]
#[derive(Debug)]
struct ContextIoPause {
    reached: std::sync::Barrier,
    resume: std::sync::Barrier,
}
impl V2ContextStore {
    /// Open the store and synchronously create its directory.
    pub(crate) fn open(root: impl AsRef<Path>) -> Result<Self, V2ContextStoreError> {
        let root = root.as_ref().to_path_buf();
        let directory = root.join("contexts");
        let (root_identity, directory_identity) = ensure_context_directory(&root, &directory)?;
        Ok(Self {
            root,
            root_identity,
            directory,
            directory_identity,
            #[cfg(test)]
            lookup_pause: std::sync::Arc::new(std::sync::Mutex::new(None)),
            #[cfg(test)]
            read_pause: std::sync::Arc::new(std::sync::Mutex::new(None)),
            #[cfg(test)]
            publication_pause: std::sync::Arc::new(std::sync::Mutex::new(None)),
        })
    }
    /// Read one context directly from a storage root without creating or synchronizing paths.
    ///
    /// Provisional snapshot authentication uses this to detect an existing immutable conflict
    /// while preserving a byte-for-byte read-only boundary. Publication remains the responsibility
    /// of the token-consuming Kura finalizer.
    pub(crate) fn load_from_root_read_only(
        root: impl AsRef<Path>,
        height: wire::Height,
    ) -> Result<Option<PersistedHeightContext>, V2ContextStoreError> {
        let root = root.as_ref().to_path_buf();
        let Some(stable_root) = stable_directory(&root, &root)? else {
            return Ok(None);
        };
        let directory = root.join("contexts");
        let stable_contexts = match stable_directory(&root, &directory)? {
            Some(contexts) => contexts,
            None => {
                require_root_identity(&root, &stable_root.metadata)?;
                return Ok(None);
            }
        };
        Self {
            directory,
            directory_identity: stable_contexts.metadata,
            root_identity: stable_root.metadata,
            root,
            #[cfg(test)]
            lookup_pause: std::sync::Arc::new(std::sync::Mutex::new(None)),
            #[cfg(test)]
            read_pause: std::sync::Arc::new(std::sync::Mutex::new(None)),
            #[cfg(test)]
            publication_pause: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
        .load(height)
    }
    /// Persist an immutable record before opening its height WAL.
    ///
    /// An exact repeat is idempotent. A different record at the same height is
    /// a safety failure and is never replaced.
    pub(crate) fn persist(
        &self,
        record: &PersistedHeightContext,
    ) -> Result<(), V2ContextStoreError> {
        record.validate_layout()?;
        let path = self.path(record.context.height);
        let frame = encode_frame(record)?;
        if let Some(existing) = self.read_frame_bytes(&path)? {
            return compare_existing_frame(&path, &existing, &frame, record);
        }
        if !write_atomic_synced_noclobber(
            &self.root,
            &self.root_identity,
            &self.directory,
            &self.directory_identity,
            &path,
            &frame,
            || {
                #[cfg(test)]
                if let Some(pause) = self
                    .publication_pause
                    .lock()
                    .expect("context publication pause mutex is not poisoned")
                    .take()
                {
                    pause.reached.wait();
                    pause.resume.wait();
                }
            },
        )? {
            let existing = self.read_frame_bytes(&path)?.ok_or_else(|| {
                unsafe_path(
                    &path,
                    "context appeared and disappeared during no-clobber publication",
                )
            })?;
            return compare_existing_frame(&path, &existing, &frame, record);
        }
        let persisted = self.read_frame_bytes(&path)?.ok_or_else(|| {
            unsafe_path(
                &path,
                "published context disappeared before stable readback",
            )
        })?;
        if persisted != frame {
            return Err(unsafe_path(
                &path,
                "published context changed before stable readback",
            ));
        }
        Ok(())
    }
    /// Load and checksum-verify one exact height record.
    pub(crate) fn load(
        &self,
        height: wire::Height,
    ) -> Result<Option<PersistedHeightContext>, V2ContextStoreError> {
        let path = self.path(height);
        let Some(bytes) = self.read_frame_bytes(&path)? else {
            return Ok(None);
        };
        let record = decode_frame(&bytes)?;
        if record.context.height != height {
            return Err(V2ContextStoreError::HeightMismatch {
                expected: height,
                actual: record.context.height,
            });
        }
        Ok(Some(record))
    }
    fn path(&self, height: wire::Height) -> PathBuf {
        self.directory.join(format!("{height:020}.norito"))
    }
    fn read_frame_bytes(&self, path: &Path) -> Result<Option<Vec<u8>>, V2ContextStoreError> {
        stable_read_file(
            &self.root,
            &self.root_identity,
            &self.directory,
            &self.directory_identity,
            path,
            MAX_CONTEXT_FRAME_BYTES,
            || {
                #[cfg(test)]
                if let Some(pause) = self
                    .lookup_pause
                    .lock()
                    .expect("context lookup pause mutex is not poisoned")
                    .take()
                {
                    pause.reached.wait();
                    pause.resume.wait();
                }
            },
            || {
                #[cfg(test)]
                if let Some(pause) = self
                    .read_pause
                    .lock()
                    .expect("context read pause mutex is not poisoned")
                    .take()
                {
                    pause.reached.wait();
                    pause.resume.wait();
                }
            },
        )
    }
    #[cfg(test)]
    fn pause_next_read_before_lookup(&self) -> std::sync::Arc<ContextIoPause> {
        let pause = std::sync::Arc::new(ContextIoPause {
            reached: std::sync::Barrier::new(2),
            resume: std::sync::Barrier::new(2),
        });
        *self
            .lookup_pause
            .lock()
            .expect("context lookup pause mutex is not poisoned") =
            Some(std::sync::Arc::clone(&pause));
        pause
    }
    #[cfg(test)]
    fn pause_next_read_after_open(&self) -> std::sync::Arc<ContextIoPause> {
        let pause = std::sync::Arc::new(ContextIoPause {
            reached: std::sync::Barrier::new(2),
            resume: std::sync::Barrier::new(2),
        });
        *self
            .read_pause
            .lock()
            .expect("context read pause mutex is not poisoned") =
            Some(std::sync::Arc::clone(&pause));
        pause
    }
    #[cfg(test)]
    fn pause_next_publication(&self) -> std::sync::Arc<ContextIoPause> {
        let pause = std::sync::Arc::new(ContextIoPause {
            reached: std::sync::Barrier::new(2),
            resume: std::sync::Barrier::new(2),
        });
        *self
            .publication_pause
            .lock()
            .expect("context publication pause mutex is not poisoned") =
            Some(std::sync::Arc::clone(&pause));
        pause
    }
}
fn encode_frame(record: &PersistedHeightContext) -> Result<Vec<u8>, V2ContextStoreError> {
    let payload = record.encode();
    if payload.len() > MAX_CONTEXT_PAYLOAD_BYTES {
        return Err(V2ContextStoreError::TooLarge {
            actual: u64::try_from(payload.len()).unwrap_or(u64::MAX),
            max: MAX_CONTEXT_PAYLOAD_BYTES,
        });
    }
    let payload_len = u64::try_from(payload.len()).map_err(|_| V2ContextStoreError::TooLarge {
        actual: u64::MAX,
        max: MAX_CONTEXT_PAYLOAD_BYTES,
    })?;
    let digest = Hash::new(&payload);
    let mut frame = Vec::with_capacity(HEADER_LEN.checked_add(payload.len()).ok_or(
        V2ContextStoreError::TooLarge {
            actual: u64::MAX,
            max: MAX_CONTEXT_FRAME_BYTES,
        },
    )?);
    frame.extend_from_slice(FILE_MAGIC);
    frame.extend_from_slice(&FRAME_VERSION.to_le_bytes());
    frame.extend_from_slice(&payload_len.to_le_bytes());
    frame.extend_from_slice(digest.as_ref());
    frame.extend_from_slice(&payload);
    Ok(frame)
}
fn decode_frame(bytes: &[u8]) -> Result<PersistedHeightContext, V2ContextStoreError> {
    if bytes.len() > MAX_CONTEXT_FRAME_BYTES {
        return Err(V2ContextStoreError::TooLarge {
            actual: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            max: MAX_CONTEXT_FRAME_BYTES,
        });
    }
    if bytes.len() < HEADER_LEN || bytes.get(..FILE_MAGIC.len()) != Some(FILE_MAGIC.as_slice()) {
        return Err(V2ContextStoreError::MalformedFrame);
    }
    let version_offset = FILE_MAGIC.len();
    let version = u16::from_le_bytes(
        bytes[version_offset..version_offset + 2]
            .try_into()
            .map_err(|_| V2ContextStoreError::MalformedFrame)?,
    );
    if version != FRAME_VERSION {
        return Err(V2ContextStoreError::UnsupportedVersion(version));
    }
    let length_offset = version_offset + 2;
    let payload_len_u64 = u64::from_le_bytes(
        bytes[length_offset..length_offset + 8]
            .try_into()
            .map_err(|_| V2ContextStoreError::MalformedFrame)?,
    );
    if payload_len_u64 > u64::try_from(MAX_CONTEXT_PAYLOAD_BYTES).unwrap_or(u64::MAX) {
        return Err(V2ContextStoreError::TooLarge {
            actual: payload_len_u64,
            max: MAX_CONTEXT_PAYLOAD_BYTES,
        });
    }
    let payload_len =
        usize::try_from(payload_len_u64).map_err(|_| V2ContextStoreError::TooLarge {
            actual: payload_len_u64,
            max: MAX_CONTEXT_PAYLOAD_BYTES,
        })?;
    let hash_offset = length_offset + 8;
    let payload_offset = hash_offset + HASH_LEN;
    if payload_offset.checked_add(payload_len) != Some(bytes.len()) {
        return Err(V2ContextStoreError::MalformedFrame);
    }
    let payload = &bytes[payload_offset..];
    if Hash::new(payload).as_ref() != &bytes[hash_offset..payload_offset] {
        return Err(V2ContextStoreError::HashMismatch);
    }
    let mut cursor = payload;
    let record = PersistedHeightContext::decode_all(&mut cursor)
        .map_err(|error| V2ContextStoreError::Decode(error.to_string()))?;
    record.validate_layout()?;
    if record.encode() != payload {
        return Err(V2ContextStoreError::NonCanonicalFrame);
    }
    Ok(record)
}
fn compare_existing_frame(
    path: &Path,
    existing: &[u8],
    expected_frame: &[u8],
    expected_record: &PersistedHeightContext,
) -> Result<(), V2ContextStoreError> {
    if existing == expected_frame {
        return Ok(());
    }
    let recovered = decode_frame(existing)?;
    if recovered == *expected_record {
        return Err(V2ContextStoreError::NonCanonicalExistingFrame(
            path.to_path_buf(),
        ));
    }
    Err(V2ContextStoreError::ConflictingHeight {
        height: expected_record.context.height,
    })
}
#[derive(Debug)]
struct StableDirectory {
    canonical_path: PathBuf,
    metadata: SecureMetadata,
}
#[derive(Debug)]
struct StableFile {
    canonical_path: PathBuf,
    metadata: SecureMetadata,
    directory_metadata: SecureMetadata,
}
#[cfg(unix)]
fn metadata_same_object(left: &SecureMetadata, right: &SecureMetadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev() && left.ino() == right.ino()
}
#[cfg(windows)]
fn metadata_same_object(left: &SecureMetadata, right: &SecureMetadata) -> bool {
    left.volume_serial_number() == right.volume_serial_number()
        && left.file_index() == right.file_index()
        && left.volume_serial_number().is_some()
        && left.file_index().is_some()
}
#[cfg(all(not(unix), not(windows)))]
fn metadata_same_object(left: &SecureMetadata, right: &SecureMetadata) -> bool {
    left.file_type() == right.file_type() && left.created().ok() == right.created().ok()
}
#[cfg(unix)]
fn is_single_link(metadata: &SecureMetadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    metadata.nlink() == 1
}
#[cfg(windows)]
fn is_single_link(metadata: &SecureMetadata) -> bool {
    metadata.number_of_links() == Some(1)
}
#[cfg(all(not(unix), not(windows)))]
fn is_single_link(_metadata: &SecureMetadata) -> bool {
    true
}
#[cfg(unix)]
fn file_metadata_unchanged(left: &SecureMetadata, right: &SecureMetadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    metadata_same_object(left, right)
        && left.nlink() == 1
        && right.nlink() == 1
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}
#[cfg(windows)]
fn file_metadata_unchanged(left: &SecureMetadata, right: &SecureMetadata) -> bool {
    metadata_same_object(left, right)
        && left.number_of_links() == Some(1)
        && right.number_of_links() == Some(1)
        && left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
        && left.creation_time() == right.creation_time()
}
#[cfg(all(not(unix), not(windows)))]
fn file_metadata_unchanged(left: &SecureMetadata, right: &SecureMetadata) -> bool {
    metadata_same_object(left, right)
        && left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
}
fn stable_directory(
    root: &Path,
    expected: &Path,
) -> Result<Option<StableDirectory>, V2ContextStoreError> {
    let before = match secure_file_metadata::from_path(expected) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_error(expected, source)),
    };
    if before.file_type().is_symlink() || !before.is_dir() {
        return Err(unsafe_path(
            expected,
            "context-store path is not a direct directory",
        ));
    }
    let root_before =
        secure_file_metadata::from_path(root).map_err(|source| io_error(root, source))?;
    if root_before.file_type().is_symlink() || !root_before.is_dir() {
        return Err(unsafe_path(
            root,
            "context-store root is not a direct directory",
        ));
    }
    let relative = expected.strip_prefix(root).map_err(|_| {
        unsafe_path(
            expected,
            "context-store directory escapes its configured root",
        )
    })?;
    let canonical_root = fs::canonicalize(root).map_err(|source| io_error(root, source))?;
    let canonical_path = fs::canonicalize(expected).map_err(|source| io_error(expected, source))?;
    if canonical_path != canonical_root.join(relative) {
        return Err(unsafe_path(
            expected,
            "context-store directory contains a symlink or escapes its root",
        ));
    }
    let root_after =
        secure_file_metadata::from_path(root).map_err(|source| io_error(root, source))?;
    let after =
        secure_file_metadata::from_path(expected).map_err(|source| io_error(expected, source))?;
    if root_after.file_type().is_symlink()
        || !root_after.is_dir()
        || after.file_type().is_symlink()
        || !after.is_dir()
        || !metadata_same_object(&root_before, &root_after)
        || !metadata_same_object(&before, &after)
    {
        return Err(unsafe_path(
            expected,
            "context-store directory changed during identity validation",
        ));
    }
    Ok(Some(StableDirectory {
        canonical_path,
        metadata: after,
    }))
}
fn stable_file_metadata(
    root: &Path,
    directory: &Path,
    path: &Path,
) -> Result<Option<StableFile>, V2ContextStoreError> {
    if path.parent() != Some(directory) {
        return Err(unsafe_path(
            path,
            "context file is not an immediate child of its store directory",
        ));
    }
    let Some(directory_before) = stable_directory(root, directory)? else {
        return Ok(None);
    };
    let before = match secure_file_metadata::from_path(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_error(path, source)),
    };
    if before.file_type().is_symlink() || !before.is_file() || !is_single_link(&before) {
        return Err(unsafe_path(
            path,
            "context path is not a single-link regular file",
        ));
    }
    let canonical_path = fs::canonicalize(path).map_err(|source| io_error(path, source))?;
    if canonical_path.parent() != Some(directory_before.canonical_path.as_path()) {
        return Err(unsafe_path(
            path,
            "context file escapes its canonical store directory",
        ));
    }
    let after = secure_file_metadata::from_path(path).map_err(|source| io_error(path, source))?;
    let Some(directory_after) = stable_directory(root, directory)? else {
        return Err(unsafe_path(
            directory,
            "context-store directory disappeared during file validation",
        ));
    };
    if !file_metadata_unchanged(&before, &after)
        || !metadata_same_object(&directory_before.metadata, &directory_after.metadata)
    {
        return Err(unsafe_path(
            path,
            "context file or parent changed during identity validation",
        ));
    }
    Ok(Some(StableFile {
        canonical_path,
        metadata: after,
        directory_metadata: directory_after.metadata,
    }))
}
fn stable_read_file(
    root: &Path,
    root_identity: &SecureMetadata,
    directory: &Path,
    directory_identity: &SecureMetadata,
    path: &Path,
    byte_limit: usize,
    after_identity_preflight: impl FnOnce(),
    after_open: impl FnOnce(),
) -> Result<Option<Vec<u8>>, V2ContextStoreError> {
    require_root_identity(root, root_identity)?;
    require_directory_identity(root, directory, directory_identity)?;
    after_identity_preflight();
    let before = match stable_file_metadata(root, directory, path)? {
        Some(metadata) => metadata,
        None => {
            // Bind absence to the same authenticated root and direct context
            // directory that were checked before the lookup. Without these
            // second checks, a substituted empty hierarchy could be mistaken
            // for an authoritative missing record.
            require_root_identity(root, root_identity)?;
            require_directory_identity(root, directory, directory_identity)?;
            return Ok(None);
        }
    };
    if before.metadata.len() > u64::try_from(byte_limit).unwrap_or(u64::MAX) {
        return Err(V2ContextStoreError::TooLarge {
            actual: before.metadata.len(),
            max: byte_limit,
        });
    }
    let mut file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|source| io_error(path, source))?;
    let opened = secure_file_metadata::from_file(&file).map_err(|source| io_error(path, source))?;
    if !opened.is_file() || !file_metadata_unchanged(&before.metadata, &opened) {
        return Err(unsafe_path(
            path,
            "context file changed while it was opened",
        ));
    }
    after_open();
    let expected_len =
        usize::try_from(before.metadata.len()).map_err(|_| V2ContextStoreError::TooLarge {
            actual: before.metadata.len(),
            max: byte_limit,
        })?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected_len)
        .map_err(|_| V2ContextStoreError::TooLarge {
            actual: before.metadata.len(),
            max: byte_limit,
        })?;
    Read::by_ref(&mut file)
        .take(u64::try_from(byte_limit.saturating_add(1)).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)
        .map_err(|source| io_error(path, source))?;
    let handle_after =
        secure_file_metadata::from_file(&file).map_err(|source| io_error(path, source))?;
    let path_after = stable_file_metadata(root, directory, path)?
        .ok_or_else(|| unsafe_path(path, "context file disappeared while it was being read"))?;
    require_root_identity(root, root_identity)?;
    require_directory_identity(root, directory, directory_identity)?;
    if bytes.len() > byte_limit
        || bytes.len() != expected_len
        || before.canonical_path != path_after.canonical_path
        || !file_metadata_unchanged(&before.metadata, &handle_after)
        || !file_metadata_unchanged(&before.metadata, &path_after.metadata)
        || !metadata_same_object(&before.directory_metadata, &path_after.directory_metadata)
    {
        return Err(unsafe_path(
            path,
            "context file changed or exceeded its hard limit while being read",
        ));
    }
    Ok(Some(bytes))
}
fn require_root_identity(
    root: &Path,
    expected: &SecureMetadata,
) -> Result<(), V2ContextStoreError> {
    let current = stable_directory(root, root)?.ok_or_else(|| {
        unsafe_path(
            root,
            "context-store root disappeared after it was authenticated",
        )
    })?;
    if !metadata_same_object(expected, &current.metadata) {
        return Err(unsafe_path(
            root,
            "context-store root changed after it was authenticated",
        ));
    }
    Ok(())
}
fn require_directory_identity(
    root: &Path,
    directory: &Path,
    expected: &SecureMetadata,
) -> Result<(), V2ContextStoreError> {
    let current = stable_directory(root, directory)?.ok_or_else(|| {
        unsafe_path(
            directory,
            "context-store directory disappeared after it was authenticated",
        )
    })?;
    if !metadata_same_object(expected, &current.metadata) {
        return Err(unsafe_path(
            directory,
            "context-store directory changed after it was authenticated",
        ));
    }
    Ok(())
}
fn ensure_context_directory(
    root: &Path,
    directory: &Path,
) -> Result<(SecureMetadata, SecureMetadata), V2ContextStoreError> {
    if directory.parent() != Some(root) {
        return Err(unsafe_path(
            directory,
            "context-store directory is not a direct child of its root",
        ));
    }
    ensure_store_root(root)?;
    let root_before = stable_directory(root, root)?.ok_or_else(|| {
        unsafe_path(
            root,
            "context-store root does not exist as a direct directory",
        )
    })?;
    match fs::create_dir(directory) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(source) => return Err(io_error(directory, source)),
    }
    let directory_after = stable_directory(root, directory)?.ok_or_else(|| {
        unsafe_path(
            directory,
            "context-store directory disappeared after creation",
        )
    })?;
    let root_after = stable_directory(root, root)?.ok_or_else(|| {
        unsafe_path(
            root,
            "context-store root disappeared during directory creation",
        )
    })?;
    if !metadata_same_object(&root_before.metadata, &root_after.metadata)
        || directory_after.canonical_path.parent() != Some(root_after.canonical_path.as_path())
    {
        return Err(unsafe_path(
            directory,
            "context-store root changed while creating its direct child",
        ));
    }
    sync_directory_stable(directory, &directory_after.metadata)?;
    sync_directory_stable(root, &root_after.metadata)?;
    let final_root = stable_directory(root, root)?
        .ok_or_else(|| unsafe_path(root, "context-store root disappeared after directory fsync"))?;
    let final_directory = stable_directory(root, directory)?.ok_or_else(|| {
        unsafe_path(
            directory,
            "context-store directory disappeared after directory fsync",
        )
    })?;
    if !metadata_same_object(&root_after.metadata, &final_root.metadata)
        || !metadata_same_object(&directory_after.metadata, &final_directory.metadata)
    {
        return Err(unsafe_path(
            directory,
            "context-store root or directory changed after directory fsync",
        ));
    }
    Ok((final_root.metadata, final_directory.metadata))
}
fn ensure_store_root(root: &Path) -> Result<(), V2ContextStoreError> {
    match fs::symlink_metadata(root) {
        Ok(_) => {
            stable_directory(root, root)?.ok_or_else(|| {
                unsafe_path(root, "context-store root disappeared during validation")
            })?;
            return Ok(());
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(source) => return Err(io_error(root, source)),
    }
    let parent = root
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| {
            unsafe_path(
                root,
                "context-store root has no direct existing parent directory",
            )
        })?;
    let parent_before = stable_directory(parent, parent)?.ok_or_else(|| {
        unsafe_path(
            parent,
            "context-store root parent does not exist as a direct directory",
        )
    })?;
    match fs::create_dir(root) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(source) => return Err(io_error(root, source)),
    }
    let root_after = stable_directory(parent, root)?
        .ok_or_else(|| unsafe_path(root, "context-store root disappeared after direct creation"))?;
    let parent_after = stable_directory(parent, parent)?.ok_or_else(|| {
        unsafe_path(
            parent,
            "context-store root parent disappeared during creation",
        )
    })?;
    if !metadata_same_object(&parent_before.metadata, &parent_after.metadata)
        || root_after.canonical_path.parent() != Some(parent_after.canonical_path.as_path())
    {
        return Err(unsafe_path(
            root,
            "context-store root parent changed during direct creation",
        ));
    }
    sync_directory_stable(parent, &parent_after.metadata)
}
fn write_atomic_synced_noclobber(
    root: &Path,
    root_identity: &SecureMetadata,
    directory: &Path,
    directory_identity: &SecureMetadata,
    path: &Path,
    bytes: &[u8],
    before_publish: impl FnOnce(),
) -> Result<bool, V2ContextStoreError> {
    if bytes.len() > MAX_CONTEXT_FRAME_BYTES {
        return Err(V2ContextStoreError::TooLarge {
            actual: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            max: MAX_CONTEXT_FRAME_BYTES,
        });
    }
    if path.parent() != Some(directory) {
        return Err(unsafe_path(
            path,
            "atomic context target is not a direct child of its store directory",
        ));
    }
    require_root_identity(root, root_identity)?;
    require_directory_identity(root, directory, directory_identity)?;
    let directory_before = stable_directory(root, directory)?
        .ok_or_else(|| unsafe_path(directory, "atomic context-store directory does not exist"))?;
    let mut temporary = tempfile::Builder::new()
        .prefix(".sumeragi-v2-context-")
        .tempfile_in(directory)
        .map_err(|source| io_error(directory, source))?;
    let directory_after_create = stable_directory(root, directory)?.ok_or_else(|| {
        unsafe_path(
            directory,
            "atomic context-store directory disappeared after temporary creation",
        )
    })?;
    if !metadata_same_object(&directory_before.metadata, &directory_after_create.metadata) {
        return Err(unsafe_path(
            directory,
            "atomic context-store directory changed during temporary creation",
        ));
    }
    let temp_path_metadata = secure_file_metadata::from_path(temporary.path())
        .map_err(|source| io_error(temporary.path(), source))?;
    let temp_handle_metadata = secure_file_metadata::from_file(temporary.as_file())
        .map_err(|source| io_error(temporary.path(), source))?;
    if temp_path_metadata.file_type().is_symlink()
        || !temp_path_metadata.is_file()
        || !file_metadata_unchanged(&temp_path_metadata, &temp_handle_metadata)
    {
        return Err(unsafe_path(
            temporary.path(),
            "atomic context temporary is not a stable single-link regular file",
        ));
    }
    temporary
        .as_file_mut()
        .write_all(bytes)
        .and_then(|()| temporary.as_file_mut().flush())
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|source| io_error(temporary.path(), source))?;
    let temp_handle_after = secure_file_metadata::from_file(temporary.as_file())
        .map_err(|source| io_error(temporary.path(), source))?;
    let temp_path_after = secure_file_metadata::from_path(temporary.path())
        .map_err(|source| io_error(temporary.path(), source))?;
    if !file_metadata_unchanged(&temp_handle_after, &temp_path_after)
        || temp_handle_after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(unsafe_path(
            temporary.path(),
            "atomic context temporary changed before publication",
        ));
    }
    let directory_before_publish = stable_directory(root, directory)?.ok_or_else(|| {
        unsafe_path(
            directory,
            "atomic context-store directory disappeared before publication",
        )
    })?;
    if !metadata_same_object(
        &directory_before.metadata,
        &directory_before_publish.metadata,
    ) {
        return Err(unsafe_path(
            directory,
            "atomic context-store directory changed before publication",
        ));
    }
    before_publish();
    require_root_identity(root, root_identity)?;
    require_directory_identity(root, directory, directory_identity)?;
    let directory_after_pause = stable_directory(root, directory)?.ok_or_else(|| {
        unsafe_path(
            directory,
            "atomic context-store directory disappeared before no-clobber publication",
        )
    })?;
    if !metadata_same_object(&directory_before.metadata, &directory_after_pause.metadata) {
        return Err(unsafe_path(
            directory,
            "atomic context-store directory changed before no-clobber publication",
        ));
    }
    let persisted = match temporary.persist_noclobber(path) {
        Ok(file) => file,
        Err(error) if error.error.kind() == ErrorKind::AlreadyExists => return Ok(false),
        Err(error) => return Err(io_error(path, error.error)),
    };
    persisted
        .sync_all()
        .map_err(|source| io_error(path, source))?;
    let persisted_handle =
        secure_file_metadata::from_file(&persisted).map_err(|source| io_error(path, source))?;
    let persisted_path = stable_file_metadata(root, directory, path)?
        .ok_or_else(|| unsafe_path(path, "atomic context target disappeared after publication"))?;
    require_root_identity(root, root_identity)?;
    require_directory_identity(root, directory, directory_identity)?;
    if !file_metadata_unchanged(&persisted_handle, &persisted_path.metadata)
        || persisted_handle.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(unsafe_path(
            path,
            "atomic context target changed during publication",
        ));
    }
    sync_directory_stable(directory, &persisted_path.directory_metadata)?;
    require_root_identity(root, root_identity)?;
    require_directory_identity(root, directory, directory_identity)?;
    Ok(true)
}
fn sync_directory_stable(
    path: &Path,
    expected: &SecureMetadata,
) -> Result<(), V2ContextStoreError> {
    let directory = File::open(path).map_err(|source| io_error(path, source))?;
    let opened =
        secure_file_metadata::from_file(&directory).map_err(|source| io_error(path, source))?;
    if !opened.is_dir() || !metadata_same_object(expected, &opened) {
        return Err(unsafe_path(
            path,
            "directory changed while it was opened for fsync",
        ));
    }
    directory
        .sync_all()
        .map_err(|source| io_error(path, source))?;
    let after = secure_file_metadata::from_path(path).map_err(|source| io_error(path, source))?;
    if after.file_type().is_symlink() || !after.is_dir() || !metadata_same_object(expected, &after)
    {
        return Err(unsafe_path(
            path,
            "directory changed while it was being fsynced",
        ));
    }
    Ok(())
}
fn io_error(path: &Path, source: io::Error) -> V2ContextStoreError {
    V2ContextStoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}
fn unsafe_path(path: &Path, reason: &'static str) -> V2ContextStoreError {
    V2ContextStoreError::UnsafePath {
        path: path.to_path_buf(),
        reason,
    }
}
/// Fail-closed context persistence or recovery error.
#[derive(Debug, Error)]
pub(crate) enum V2ContextStoreError {
    /// Filesystem operation failed.
    #[error("Sumeragi v2 context-store I/O failed at {path}: {source}")]
    Io {
        /// Affected path.
        path: PathBuf,
        /// Underlying I/O failure.
        #[source]
        source: io::Error,
    },
    /// A path component or file identity is unsafe for an authority-bearing record.
    #[error("unsafe Sumeragi v2 context-store path at {}: {reason}", path.display())]
    UnsafePath {
        /// Affected path.
        path: PathBuf,
        /// Stable rejection reason.
        reason: &'static str,
    },
    /// Context record is not the supported layout revision.
    #[error("unsupported Sumeragi v2 context-store version {0}")]
    UnsupportedVersion(u16),
    /// Frame header or exact length is malformed.
    #[error("malformed Sumeragi v2 context-store frame")]
    MalformedFrame,
    /// Payload checksum failed.
    #[error("Sumeragi v2 context-store hash mismatch")]
    HashMismatch,
    /// Norito payload failed complete decoding.
    #[error("malformed Sumeragi v2 context-store payload: {0}")]
    Decode(String),
    /// Embedded height context failed structural validation.
    #[error("invalid Sumeragi v2 height context: {0}")]
    Wire(wire::ValidationError),
    /// PoP vector is not aligned with the voting roster.
    #[error("Sumeragi v2 context-store PoP count differs from roster length")]
    ProofCountMismatch,
    /// A PoP is not one canonical BLS-normal proof.
    #[error("Sumeragi v2 context-store PoP length is not {BLS_NORMAL_POP_BYTES} bytes")]
    InvalidProofLength,
    /// Encoded or advertised length exceeds the deterministic allocation limit.
    #[error("Sumeragi v2 context-store frame is too large: {actual} bytes, maximum {max}")]
    TooLarge {
        /// Observed or advertised bytes.
        actual: u64,
        /// Hard byte limit.
        max: usize,
    },
    /// Decoded payload is not the unique canonical Norito encoding.
    #[error("non-canonical Sumeragi v2 context-store frame")]
    NonCanonicalFrame,
    /// Immutable height already has different contents.
    #[error("conflicting Sumeragi v2 context record at height {height}")]
    ConflictingHeight {
        /// Conflicted chain height.
        height: wire::Height,
    },
    /// Existing bytes decode to the same value but are not canonical bytes.
    #[error("non-canonical Sumeragi v2 context frame at {}", .0.display())]
    NonCanonicalExistingFrame(PathBuf),
    /// File name and embedded height disagree.
    #[error("Sumeragi v2 context height mismatch: expected {expected}, got {actual}")]
    HeightMismatch {
        /// Requested height.
        expected: wire::Height,
        /// Embedded height.
        actual: wire::Height,
    },
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{NetworkId, block::BlockHeader, peer::PeerId};
    use std::sync::{Arc, Barrier};
    fn test_network_id() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([0x93; Hash::LENGTH]),
        ))
    }
    fn record() -> PersistedHeightContext {
        let mut validators = (1_u8..=4)
            .map(|seed| {
                let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS key");
                let entry = wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                };
                (key, entry)
            })
            .collect::<Vec<_>>();
        validators.sort_by(|left, right| left.1.validator.cmp(&right.1.validator));
        let proofs_of_possession = validators
            .iter()
            .map(|(key, _)| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture validator PoP")
            })
            .collect::<Vec<_>>();
        let roster = validators
            .into_iter()
            .map(|(_, entry)| entry)
            .collect::<Vec<_>>();
        let network_id = test_network_id();
        let (kagemusha_mint_finality_epoch_id, kagemusha_mint_finality_epoch_roster) =
            crate::kagemusha_v1_test_fixtures::mint_finality_roster_and_id(
                network_id, 0, &roster,
            );
        let context = wire::HeightContext {
            network_id,
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("quorum"),
            roster,
            kagemusha_mint_finality_epoch_id,
            kagemusha_mint_finality_epoch_roster,
            nexus_amx_context_hash: Hash::new(b"context-store-nexus-amx"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 64,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 128,
            },
            leader_seed: [0x51; 32],
        };
        PersistedHeightContext {
            format_version: FRAME_VERSION,
            proofs_of_possession,
            context,
        }
    }
    #[test]
    fn record_roundtrips_and_exact_repeat_is_idempotent() {
        let root = tempfile::tempdir().expect("tempdir");
        let store = V2ContextStore::open(root.path()).expect("open store");
        let record = record();
        store.persist(&record).expect("persist record");
        store.persist(&record).expect("repeat exact record");
        assert_eq!(store.load(1).expect("load record"), Some(record));
    }
    #[test]
    fn non_v1_frame_is_rejected() {
        const UNSUPPORTED_VERSION: u16 = 2;
        let root = tempfile::tempdir().expect("tempdir");
        let store = V2ContextStore::open(root.path()).expect("open store");
        let record = record();
        store.persist(&record).expect("persist current record");
        let path = store.path(record.context().height);
        let mut frame = fs::read(&path).expect("read context frame");
        assert_eq!(
            u16::from_le_bytes(
                frame[FILE_MAGIC.len()..FILE_MAGIC.len() + std::mem::size_of::<u16>()]
                    .try_into()
                    .expect("context frame version has fixed width"),
            ),
            FRAME_VERSION,
        );
        frame[FILE_MAGIC.len()..FILE_MAGIC.len() + std::mem::size_of::<u16>()]
            .copy_from_slice(&UNSUPPORTED_VERSION.to_le_bytes());
        fs::write(&path, frame).expect("write unsupported context frame");
        assert!(matches!(
            store.load(record.context().height),
            Err(V2ContextStoreError::UnsupportedVersion(UNSUPPORTED_VERSION))
        ));
    }
    #[test]
    fn open_creates_a_missing_store_root_as_one_direct_synced_child() {
        let parent = tempfile::tempdir().expect("tempdir");
        let root = parent.path().join("sumeragi_v2");
        let store = V2ContextStore::open(&root).expect("create direct store hierarchy");
        assert_eq!(store.root, root);
        assert!(
            fs::symlink_metadata(&store.root)
                .expect("root metadata")
                .is_dir()
        );
        assert!(
            fs::symlink_metadata(&store.directory)
                .expect("context directory metadata")
                .is_dir()
        );
    }
    #[test]
    fn corruption_and_conflicting_height_fail_closed() {
        let root = tempfile::tempdir().expect("tempdir");
        let store = V2ContextStore::open(root.path()).expect("open store");
        let record = record();
        store.persist(&record).expect("persist record");
        let path = store.path(1);
        let mut bytes = fs::read(&path).expect("read frame");
        *bytes.last_mut().expect("nonempty frame") ^= 0x80;
        fs::write(&path, bytes).expect("inject corruption");
        assert!(matches!(
            store.load(1),
            Err(V2ContextStoreError::HashMismatch)
        ));
        fs::remove_file(&path).expect("remove corrupt frame");
        store.persist(&record).expect("restore record");
        let mut conflicting = record;
        conflicting.context.leader_seed[0] ^= 1;
        assert!(matches!(
            store.persist(&conflicting),
            Err(V2ContextStoreError::ConflictingHeight { height: 1 })
        ));
    }
    #[test]
    fn noncanonical_bls_pop_lengths_are_rejected_before_publication() {
        let root = tempfile::tempdir().expect("tempdir");
        let store = V2ContextStore::open(root.path()).expect("open store");
        for invalid_len in [0, BLS_NORMAL_POP_BYTES - 1, BLS_NORMAL_POP_BYTES + 1] {
            let mut invalid = record();
            invalid.proofs_of_possession[0].resize(invalid_len, 0);
            assert!(matches!(
                store.persist(&invalid),
                Err(V2ContextStoreError::InvalidProofLength)
            ));
            assert_eq!(store.load(1).expect("final path remains absent"), None);
        }
    }
    #[test]
    fn incomplete_temporary_frame_is_unacknowledged() {
        let root = tempfile::tempdir().expect("tempdir");
        let store = V2ContextStore::open(root.path()).expect("open store");
        fs::write(store.path(1).with_extension("norito.tmp"), b"partial")
            .expect("write partial temporary frame");
        assert_eq!(store.load(1).expect("missing final path"), None);
        store
            .persist(&record())
            .expect("replace unacknowledged write");
        assert!(store.load(1).expect("load final").is_some());
    }
    #[test]
    fn oversized_frame_is_rejected_before_allocation() {
        let root = tempfile::tempdir().expect("tempdir");
        let store = V2ContextStore::open(root.path()).expect("open store");
        let path = store.path(1);
        File::create(&path)
            .expect("create oversized frame")
            .set_len(u64::try_from(MAX_CONTEXT_FRAME_BYTES).expect("limit fits u64") + 1)
            .expect("size oversized frame sparsely");
        assert!(matches!(
            store.load(1),
            Err(V2ContextStoreError::TooLarge { actual, max })
                if actual == u64::try_from(MAX_CONTEXT_FRAME_BYTES).expect("limit fits u64") + 1
                    && max == MAX_CONTEXT_FRAME_BYTES
        ));
    }
    #[test]
    fn trailing_and_truncated_frames_fail_closed() {
        let root = tempfile::tempdir().expect("tempdir");
        let store = V2ContextStore::open(root.path()).expect("open store");
        let path = store.path(1);
        let canonical = encode_frame(&record()).expect("encode canonical frame");
        let mut trailing = canonical.clone();
        trailing.push(0);
        fs::write(&path, trailing).expect("write trailing frame");
        assert!(matches!(
            store.load(1),
            Err(V2ContextStoreError::MalformedFrame)
        ));
        fs::write(&path, &canonical[..canonical.len() - 1]).expect("write truncated frame");
        assert!(matches!(
            store.load(1),
            Err(V2ContextStoreError::MalformedFrame)
        ));
    }
    #[test]
    fn checksum_valid_noncanonical_same_value_is_not_an_idempotent_repeat() {
        let root = tempfile::tempdir().expect("tempdir");
        let store = V2ContextStore::open(root.path()).expect("open store");
        let path = store.path(1);
        let mut payload = record().encode();
        // A second encoded record is appended inside a checksum-valid outer frame. The outer
        // length and hash are internally consistent, but exact Norito decoding/canonicality must
        // reject the ambiguous payload rather than accepting its first value.
        payload.extend_from_slice(&record().encode());
        let mut frame = Vec::with_capacity(HEADER_LEN + payload.len());
        frame.extend_from_slice(FILE_MAGIC);
        frame.extend_from_slice(&FRAME_VERSION.to_le_bytes());
        frame.extend_from_slice(
            &u64::try_from(payload.len())
                .expect("fixture length fits u64")
                .to_le_bytes(),
        );
        frame.extend_from_slice(Hash::new(&payload).as_ref());
        frame.extend_from_slice(&payload);
        fs::write(&path, frame).expect("write checksum-valid ambiguous frame");
        assert!(matches!(
            store.persist(&record()),
            Err(V2ContextStoreError::Decode(_)) | Err(V2ContextStoreError::NonCanonicalFrame)
        ));
    }
    #[cfg(unix)]
    #[test]
    fn symlink_and_hardlink_contexts_are_rejected_without_touching_victims() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().expect("tempdir");
        let store = V2ContextStore::open(root.path()).expect("open store");
        let path = store.path(1);
        let victim = root.path().join("victim");
        fs::write(&victim, b"do-not-touch").expect("write victim");
        symlink(&victim, &path).expect("plant final-path symlink");
        assert!(matches!(
            store.load(1),
            Err(V2ContextStoreError::UnsafePath { .. })
        ));
        assert!(matches!(
            store.persist(&record()),
            Err(V2ContextStoreError::UnsafePath { .. })
        ));
        assert_eq!(fs::read(&victim).expect("read victim"), b"do-not-touch");
        fs::remove_file(&path).expect("remove symlink");
        fs::hard_link(&victim, &path).expect("plant final-path hardlink");
        assert!(matches!(
            store.load(1),
            Err(V2ContextStoreError::UnsafePath { .. })
        ));
        assert!(matches!(
            store.persist(&record()),
            Err(V2ContextStoreError::UnsafePath { .. })
        ));
        assert_eq!(fs::read(&victim).expect("read victim"), b"do-not-touch");
    }
    #[cfg(unix)]
    #[test]
    fn preplanted_predictable_temporary_symlink_cannot_clobber_a_victim() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().expect("tempdir");
        let store = V2ContextStore::open(root.path()).expect("open store");
        let victim = root.path().join("victim");
        fs::write(&victim, b"preserve-me").expect("write victim");
        let retired_predictable_temp = store.path(1).with_extension("norito.tmp");
        symlink(&victim, &retired_predictable_temp).expect("plant retired temporary symlink");
        store
            .persist(&record())
            .expect("publish through random create-new temporary");
        assert_eq!(fs::read(&victim).expect("read victim"), b"preserve-me");
        assert!(
            fs::symlink_metadata(&retired_predictable_temp)
                .expect("attacker symlink remains untouched")
                .file_type()
                .is_symlink()
        );
    }
    #[cfg(unix)]
    #[test]
    fn file_path_swap_after_open_is_detected() {
        let root = tempfile::tempdir().expect("tempdir");
        let store = V2ContextStore::open(root.path()).expect("open store");
        let original = record();
        store.persist(&original).expect("persist original");
        let path = store.path(1);
        let displaced = store.directory.join("displaced.norito");
        let mut replacement = original;
        replacement.context.leader_seed[0] ^= 1;
        let replacement_frame = encode_frame(&replacement).expect("encode replacement");
        let pause = store.pause_next_read_after_open();
        let loader = store.clone();
        let join = std::thread::spawn(move || loader.load(1));
        pause.reached.wait();
        fs::rename(&path, &displaced).expect("displace opened context");
        fs::write(&path, replacement_frame).expect("swap replacement into canonical path");
        pause.resume.wait();
        assert!(matches!(
            join.join().expect("loader thread"),
            Err(V2ContextStoreError::UnsafePath { .. })
        ));
    }
    #[test]
    fn missing_context_cannot_hide_root_replacement_during_lookup() {
        let parent = tempfile::tempdir().expect("tempdir");
        let root = parent.path().join("sumeragi_v2");
        let displaced = parent.path().join("displaced");
        let store = V2ContextStore::open(&root).expect("open store");
        let pause = store.pause_next_read_before_lookup();
        let loader = store.clone();
        let join = std::thread::spawn(move || loader.load(1));
        pause.reached.wait();
        fs::rename(&root, &displaced).expect("displace authenticated root");
        fs::create_dir(&root).expect("create substituted root");
        fs::create_dir(root.join("contexts")).expect("create empty substituted context directory");
        pause.resume.wait();
        assert!(matches!(
            join.join().expect("loader thread"),
            Err(V2ContextStoreError::UnsafePath { .. })
        ));
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_context_directory_is_rejected() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().expect("tempdir");
        let target = tempfile::tempdir().expect("target tempdir");
        symlink(target.path(), root.path().join("contexts"))
            .expect("plant context-directory symlink");
        assert!(matches!(
            V2ContextStore::open(root.path()),
            Err(V2ContextStoreError::UnsafePath { .. })
        ));
        assert!(
            target
                .path()
                .read_dir()
                .expect("read target")
                .next()
                .is_none()
        );
    }
    #[test]
    fn replaced_direct_context_directory_is_rejected_between_operations() {
        let root = tempfile::tempdir().expect("tempdir");
        let store = V2ContextStore::open(root.path()).expect("open store");
        let displaced = root.path().join("contexts.displaced");
        fs::rename(&store.directory, &displaced).expect("displace authenticated directory");
        fs::create_dir(&store.directory).expect("install replacement direct directory");
        assert!(matches!(
            store.load(1),
            Err(V2ContextStoreError::UnsafePath { .. })
        ));
        assert!(matches!(
            store.persist(&record()),
            Err(V2ContextStoreError::UnsafePath { .. })
        ));
        assert!(
            store
                .directory
                .read_dir()
                .expect("read replacement directory")
                .next()
                .is_none(),
            "rejected replacement directory must receive no context or temporary"
        );
    }
    #[cfg(unix)]
    #[test]
    fn symlinked_store_root_is_rejected_without_touching_its_target() {
        use std::os::unix::fs::symlink;
        let parent = tempfile::tempdir().expect("tempdir");
        let target = tempfile::tempdir().expect("target tempdir");
        let root = parent.path().join("sumeragi_v2");
        symlink(target.path(), &root).expect("plant store-root symlink");
        assert!(matches!(
            V2ContextStore::open(&root),
            Err(V2ContextStoreError::UnsafePath { .. })
        ));
        assert!(
            target
                .path()
                .read_dir()
                .expect("read target")
                .next()
                .is_none()
        );
    }
    #[test]
    fn opened_store_rejects_root_directory_replacement() {
        let parent = tempfile::tempdir().expect("tempdir");
        let root = parent.path().join("sumeragi_v2");
        let displaced = parent.path().join("displaced");
        let store = V2ContextStore::open(&root).expect("open store");
        fs::rename(&root, &displaced).expect("displace authenticated root");
        fs::create_dir(&root).expect("create substituted root");
        fs::create_dir(root.join("contexts")).expect("create substituted context directory");
        assert!(matches!(
            store.load(1),
            Err(V2ContextStoreError::UnsafePath { .. })
        ));
        assert!(matches!(
            store.persist(&record()),
            Err(V2ContextStoreError::UnsafePath { .. })
        ));
    }
    #[test]
    fn proofs_of_possession_require_exact_bls_normal_length() {
        for invalid_len in [0, BLS_NORMAL_POP_BYTES - 1, BLS_NORMAL_POP_BYTES + 1] {
            let mut invalid = record();
            invalid.proofs_of_possession[0] = vec![0xA5; invalid_len];
            assert!(matches!(
                invalid.validate_layout(),
                Err(V2ContextStoreError::InvalidProofLength)
            ));
        }
    }
    #[test]
    fn concurrent_same_record_writers_are_idempotent_and_leave_no_temporaries() {
        let root = tempfile::tempdir().expect("tempdir");
        let store = V2ContextStore::open(root.path()).expect("open store");
        let barrier = Arc::new(Barrier::new(9));
        let mut joins = Vec::new();
        for _ in 0..8 {
            let writer = store.clone();
            let barrier = Arc::clone(&barrier);
            joins.push(std::thread::spawn(move || {
                barrier.wait();
                writer.persist(&record())
            }));
        }
        barrier.wait();
        for join in joins {
            join.join()
                .expect("writer thread")
                .expect("idempotent writer");
        }
        assert_eq!(store.load(1).expect("load winner"), Some(record()));
        let entries = fs::read_dir(&store.directory)
            .expect("read context directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect context entries");
        assert_eq!(
            entries.len(),
            1,
            "all random temporaries must be cleaned up"
        );
    }
    #[test]
    fn deterministic_publication_race_never_replaces_the_winner() {
        let root = tempfile::tempdir().expect("tempdir");
        let store = V2ContextStore::open(root.path()).expect("open store");
        let first = record();
        let mut winner = first.clone();
        winner.context.leader_seed[0] ^= 1;
        let winner_frame = encode_frame(&winner).expect("encode race winner");
        let pause = store.pause_next_publication();
        let writer = store.clone();
        let join = std::thread::spawn(move || writer.persist(&first));
        pause.reached.wait();
        fs::write(store.path(1), winner_frame).expect("publish competing immutable record");
        pause.resume.wait();
        assert!(matches!(
            join.join().expect("writer thread"),
            Err(V2ContextStoreError::ConflictingHeight { height: 1 })
        ));
        assert_eq!(store.load(1).expect("load winner"), Some(winner));
        let temporaries = fs::read_dir(&store.directory)
            .expect("read context directory")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".sumeragi-v2-context-")
            })
            .count();
        assert_eq!(temporaries, 0, "losing random temporary must be cleaned up");
    }
    #[test]
    fn concurrent_conflicting_writers_publish_exactly_one_immutable_value() {
        let root = tempfile::tempdir().expect("tempdir");
        let store = V2ContextStore::open(root.path()).expect("open store");
        let first = record();
        let mut second = first.clone();
        second.context.leader_seed[0] ^= 1;
        let barrier = Arc::new(Barrier::new(3));
        let first_join = {
            let writer = store.clone();
            let barrier = Arc::clone(&barrier);
            let first = first.clone();
            std::thread::spawn(move || {
                barrier.wait();
                writer.persist(&first)
            })
        };
        let second_join = {
            let writer = store.clone();
            let barrier = Arc::clone(&barrier);
            let second = second.clone();
            std::thread::spawn(move || {
                barrier.wait();
                writer.persist(&second)
            })
        };
        barrier.wait();
        let results = [
            first_join.join().expect("first writer"),
            second_join.join().expect("second writer"),
        ];
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(
                    result,
                    Err(V2ContextStoreError::ConflictingHeight { height: 1 })
                ))
                .count(),
            1
        );
        let winner = store.load(1).expect("load winner").expect("winner exists");
        assert!(winner == first || winner == second);
    }
}
