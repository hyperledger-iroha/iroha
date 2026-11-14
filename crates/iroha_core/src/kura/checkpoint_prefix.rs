//! Durable portable ancestry and explicit unavailable pre-checkpoint bodies.

use super::*;
use crate::{execution_attempt::ExecutionAttemptError, sumeragi::checkpoint_prefix as proof};
use iroha_data_model::sumeragi_finality::{
    MAX_PORTABLE_PREFIX_FRAME_BYTES, PortableCertifiedBlockV1, PortablePrefixFrameV1,
    VerifiedPortableCheckpointV1,
};
use iroha_model_base::chain::ChainId;

pub(super) const DIRECTORY: &str = "checkpoint_prefix";
const MANIFEST: &str = "manifest.norito";
const INTENT: &str = "bootstrap.norito";
const MAX_MANIFEST: usize = 8192;
const STAGED: [&str; 4] = [
    "bootstrap.data",
    "bootstrap.indices",
    "bootstrap.hashes",
    "bootstrap.count",
];

#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_core::kura::PortablePrefixManifestV1")]
struct Manifest {
    version: u16,
    chain: ChainId,
    network: NetworkId,
    height: u64,
    genesis: HashOf<BlockHeader>,
    successor: HashOf<BlockHeader>,
    genesis_len: u64,
    successor_len: u64,
    genesis_wire: Hash,
    successor_wire: Hash,
    frames: Hash,
    data: Hash,
    index: Hash,
    hashes: Hash,
}

#[cfg(test)]
pub(crate) mod tests;

fn invalid(path: &Path, message: impl Into<String>) -> Error {
    Kura::invalid_lane_artifact_error(path.to_path_buf(), message)
}
fn fail<T>(path: &Path, message: impl Into<String>) -> proof::Attempt<T> {
    Err(proof::storage_error(invalid(path, message)))
}
fn io(path: &Path, error: std::io::Error) -> Error {
    Error::IO(error, path.to_path_buf())
}
fn original(error: ExecutionAttemptError<proof::Error>, path: &Path) -> Error {
    match error {
        ExecutionAttemptError::Deferred(owner) => Error::CheckpointPrefixDeferred(Box::new(owner)),
        ExecutionAttemptError::Rejected(proof::Error::Storage(error)) => *error,
        ExecutionAttemptError::Rejected(proof::Error::Invalid(message)) => invalid(path, message),
    }
}
fn frame_name(height: u64) -> String {
    format!("{height:020}.norito")
}
fn digest(previous: Hash, height: u64, bytes: &[u8]) -> Hash {
    Hash::new_from_chunks(&[
        b"iroha-checkpoint-prefix-files-v1",
        previous.as_ref(),
        &height.to_le_bytes(),
        bytes,
    ])
}

fn reserved<T>(count: usize) -> Result<Vec<T>> {
    let layout = std::alloc::Layout::array::<T>(count)
        .map_err(|_| Error::NoritoFrame(norito::Error::LengthMismatch))?;
    norito::core::classify_decode_attempt(|| {
        norito::core::reserve_decode_allocation(layout.size())
    })
    .map_err(Error::CodecAttempt)?;
    let mut values = Vec::new();
    values.try_reserve_exact(count).map_err(|original| {
        Error::CheckpointPrefixDeferred(Box::new(
            iroha_allocation::AllocatorRefusal::from_vec_reserve(original, layout).into(),
        ))
    })?;
    Ok(values)
}
fn zeroed(count: usize) -> Result<Vec<u8>> {
    let mut bytes = reserved(count)?;
    bytes.resize(count, 0);
    Ok(bytes)
}

struct Source {
    path: PathBuf,
    file: iroha_fs::RetainedFile,
    snapshot: iroha_fs::FileSnapshot,
}
impl Source {
    fn capture(path: &Path) -> std::io::Result<Self> {
        let file = iroha_fs::RetainedFile::open_regular(path)?;
        let snapshot = file.snapshot()?;
        Ok(Self {
            path: path.to_path_buf(),
            file,
            snapshot,
        })
    }
    fn path(&self) -> &Path {
        &self.path
    }
    fn revalidate(&self) -> std::io::Result<()> {
        self.file.revalidate()?;
        if self.file.snapshot()? != self.snapshot {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "original checkpoint source changed",
            ));
        }
        Ok(())
    }
    fn read(&self, maximum: usize) -> Result<Vec<u8>> {
        self.revalidate().map_err(|error| io(&self.path, error))?;
        let length = usize::try_from(
            self.file
                .file()
                .metadata()
                .map_err(|error| io(&self.path, error))?
                .len(),
        )?;
        if length > maximum {
            return Err(invalid(&self.path, "checkpoint source exceeds its bound"));
        }
        let mut bytes = zeroed(length)?;
        iroha_fs::read_exact_at(self.file.file(), &mut bytes, 0)
            .map_err(|error| io(&self.path, error))?;
        let mut extra = [0];
        if iroha_fs::read_at(self.file.file(), &mut extra, length as u64)
            .map_err(|error| io(&self.path, error))?
            != 0
        {
            return Err(invalid(
                &self.path,
                "checkpoint source grew during original read",
            ));
        }
        self.revalidate().map_err(|error| io(&self.path, error))?;
        Ok(bytes)
    }
}

fn read(path: &Path, maximum: usize) -> Result<(Source, Vec<u8>)> {
    let file = Source::capture(path).map_err(|error| io(path, error))?;
    let bytes = file.read(maximum)?;
    Ok((file, bytes))
}
fn manifest(directory: &Path) -> Result<Option<(Source, Manifest)>> {
    let path = directory.join(MANIFEST);
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io(&path, error)),
        Ok(_) => {}
    }
    let (source, bytes) = read(&path, MAX_MANIFEST)?;
    let manifest: Manifest =
        norito::core::classify_decode_attempt(|| norito::decode_canonical(&bytes))
            .map_err(Error::CodecAttempt)?;
    if manifest.version != 1
        || manifest.height < 2
        || manifest.genesis_len == 0
        || manifest.genesis_len > STRICT_INIT_MAX_BLOCK_BYTES
        || manifest.successor_len == 0
        || manifest.successor_len > STRICT_INIT_MAX_BLOCK_BYTES
    {
        return Err(invalid(&path, "invalid portable checkpoint manifest"));
    }
    Ok(Some((source, manifest)))
}

fn verify_files(
    directory: &Path,
    manifest_source: &Source,
    manifest: &Manifest,
    genesis: &SignedBlock,
    hashes: &[HashOf<BlockHeader>],
    observe: &mut dyn FnMut(&PortableCertifiedBlockV1) -> proof::Attempt<()>,
) -> proof::Attempt<VerifiedPortableCheckpointV1> {
    if hashes.len() as u64 != manifest.height || genesis.hash() != manifest.genesis {
        return fail(
            directory,
            "portable prefix differs from its exact genesis or journal extent",
        );
    }
    let mut files_digest = Hash::new(b"iroha-checkpoint-prefix-files-v1");
    let verified = proof::verify(
        genesis,
        &manifest.chain,
        manifest.network,
        hashes,
        &mut |height| {
            let path = directory.join(frame_name(height));
            let (source, bytes) =
                read(&path, MAX_PORTABLE_PREFIX_FRAME_BYTES).map_err(proof::storage_error)?;
            let frame =
                PortablePrefixFrameV1::decode_canonical(&bytes).map_err(proof::finality_error)?;
            source
                .revalidate()
                .map_err(|error| proof::storage_error(io(&path, error)))?;
            files_digest = digest(files_digest, height, &bytes);
            Ok(frame)
        },
        observe,
    )?;
    if files_digest != manifest.frames {
        return fail(directory, "portable prefix original proof files changed");
    }
    manifest_source
        .revalidate()
        .map_err(|error| proof::storage_error(io(manifest_source.path(), error)))?;
    Ok(verified)
}

fn root_for_store(store: &BlockStore) -> Result<&Path> {
    store
        .path_to_blockchain
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| invalid(&store.path_to_blockchain, "canonical store has no root"))
}

fn require_empty(directory: &iroha_fs::OwnerDirectory) -> Result<()> {
    for name in [DATA_FILE_NAME, INDEX_FILE_NAME, HASHES_FILE_NAME] {
        if directory
            .read_regular_optional(name, 1)
            .map_err(|error| io(directory.path(), error))?
            .is_some_and(|bytes| !bytes.is_empty())
        {
            return Err(invalid(
                directory.path(),
                "checkpoint bootstrap requires empty canonical journals",
            ));
        }
    }
    if let Some(bytes) = directory
        .read_regular_optional(COUNT_FILE_NAME, MAX_BLOCK_COMMIT_MARKER_BYTES)
        .map_err(|error| io(directory.path(), error))?
    {
        let marker: BlockStoreCommitMarker =
            norito::core::classify_decode_attempt(|| norito::decode_canonical(&bytes))
                .map_err(Error::CodecAttempt)?;
        if marker != BlockStoreCommitMarker::new(0, None) {
            return Err(invalid(
                directory.path(),
                "checkpoint bootstrap cannot replace occupied canonical history",
            ));
        }
    }
    Ok(())
}

fn hashes_from_bytes(bytes: &[u8], count: usize, path: &Path) -> Result<Vec<HashOf<BlockHeader>>> {
    if count.checked_mul(Hash::LENGTH) != Some(bytes.len()) {
        return Err(invalid(path, "checkpoint hash journal extent differs"));
    }
    let mut hashes = reserved(count)?;
    for bytes in bytes.chunks_exact(Hash::LENGTH) {
        let raw: [u8; Hash::LENGTH] = bytes.try_into().expect("exact chunk");
        if raw[Hash::LENGTH - 1] & 1 == 0 {
            return Err(invalid(path, "checkpoint hash lacks its canonical marker"));
        }
        hashes.push(HashOf::from_untyped_unchecked(Hash::prehashed(raw)));
    }
    Ok(hashes)
}

/// Whether this root holds a published portable prefix with no fresh-store intent pending.
///
/// The bootstrap establishes only the configured-catalog baseline; no lane binding exists
/// until State authenticates one. The manifest, written after every proof file, is therefore
/// the sole durable owner of canonical bodies under an unbound catalog journal. Its presence
/// grants no history by itself: Strict and Fast opening still verify the complete prefix
/// against the canonical journals before any omitted or retained body is trusted.
///
/// # Errors
/// The manifest or intent cannot be inspected, or the manifest is not a valid bounded record.
pub(super) fn is_published(root: &Path) -> Result<bool> {
    let directory = root.join(DIRECTORY);
    if manifest(&directory)?.is_none() {
        return Ok(false);
    }
    let intent = directory.join(INTENT);
    match std::fs::symlink_metadata(&intent) {
        Ok(_) => Ok(false),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(true),
        Err(error) => Err(io(&intent, error)),
    }
}

/// Recover only the exact fully verified fresh-store intent while holding the root process lock.
pub(super) fn recover(root: &Path, may_publish: bool) -> Result<()> {
    let directory = root.join(DIRECTORY);
    let Some((manifest_source, manifest)) = manifest(&directory)? else {
        if directory.join(INTENT).exists() {
            return Err(invalid(&directory, "checkpoint intent lost its manifest"));
        }
        return Ok(());
    };
    let owner =
        iroha_fs::OwnerDirectory::open(&directory).map_err(|error| io(&directory, error))?;
    let Some(intent) = owner
        .read_private_optional(INTENT, Hash::LENGTH)
        .map_err(|error| io(&directory, error))?
    else {
        // Original staged images remain part of the bounded portable evidence owner.
        // No startup cleanup mutates storage before its proof has been authenticated.
        return Ok(());
    };
    if !may_publish {
        return Err(invalid(
            &directory,
            "portable checkpoint bootstrap requires Strict recovery",
        ));
    }
    let manifest_bytes = manifest_source.read(MAX_MANIFEST)?;
    if intent.as_slice() != Hash::new(&manifest_bytes).as_ref() {
        return Err(invalid(
            &directory,
            "checkpoint intent has a substituted manifest",
        ));
    }
    let count = usize::try_from(manifest.height)?
        .checked_add(1)
        .ok_or_else(|| invalid(&directory, "checkpoint count overflow"))?;
    let bounds = [
        usize::try_from(
            manifest
                .genesis_len
                .checked_add(manifest.successor_len)
                .ok_or_else(|| invalid(&directory, "checkpoint body lengths overflow"))?,
        )?,
        count
            .checked_mul(BlockIndex::SIZE_USIZE)
            .ok_or_else(|| invalid(&directory, "checkpoint index length overflows"))?,
        count
            .checked_mul(Hash::LENGTH)
            .ok_or_else(|| invalid(&directory, "checkpoint hashes length overflows"))?,
        MAX_BLOCK_COMMIT_MARKER_BYTES,
    ];
    let mut sources = Vec::new();
    let mut images = Vec::new();
    for (name, maximum) in STAGED.iter().zip(bounds) {
        let (source, bytes) = read(&directory.join(name), maximum)?;
        sources.push(source);
        images.push(bytes);
    }
    if images[0].len() != bounds[0]
        || images[1].len() != bounds[1]
        || images[2].len() != bounds[2]
        || Hash::new(&images[0]) != manifest.data
        || Hash::new(&images[1]) != manifest.index
        || Hash::new(&images[2]) != manifest.hashes
    {
        return Err(invalid(
            &directory,
            "checkpoint staged journal image changed",
        ));
    }
    let genesis_len = usize::try_from(manifest.genesis_len)?;
    let genesis_bytes = &images[0][..genesis_len];
    let successor_bytes = &images[0][genesis_len..];
    let genesis = decode_framed_signed_block(genesis_bytes).map_err(Error::CodecAttempt)?;
    let successor = decode_framed_signed_block(successor_bytes).map_err(Error::CodecAttempt)?;
    let hashes = hashes_from_bytes(&images[2], count, &directory)?;
    if genesis.hash() != manifest.genesis
        || successor.hash() != manifest.successor
        || Hash::new(genesis_bytes) != manifest.genesis_wire
        || Hash::new(successor_bytes) != manifest.successor_wire
        || successor.header().height().get() != manifest.height + 1
        || successor.header().prev_block_hash().as_ref() != hashes.get(count - 2)
        || hashes.last() != Some(&successor.hash())
    {
        return Err(invalid(
            &directory,
            "checkpoint staged retained bodies changed",
        ));
    }
    for (position, chunk) in images[1].chunks_exact(BlockIndex::SIZE_USIZE).enumerate() {
        let expected = if position == 0 {
            BlockIndex {
                start: 0,
                length: manifest.genesis_len,
            }
        } else if position == count - 1 {
            BlockIndex {
                start: manifest.genesis_len,
                length: manifest.successor_len,
            }
        } else {
            BlockIndex {
                start: CHECKPOINT_OMITTED_BLOCK_START,
                length: 0,
            }
        };
        if chunk != expected.encode() {
            return Err(invalid(
                &directory,
                "checkpoint staged index has an unauthorized omission or extent",
            ));
        }
    }
    let marker: BlockStoreCommitMarker =
        norito::core::classify_decode_attempt(|| norito::decode_canonical(&images[3]))
            .map_err(Error::CodecAttempt)?;
    if marker != BlockStoreCommitMarker::new(count as u64, Some(manifest.successor)) {
        return Err(invalid(&directory, "checkpoint staged marker changed"));
    }
    verify_files(
        &directory,
        &manifest_source,
        &manifest,
        &genesis,
        &hashes[..count - 1],
        &mut |_| Ok(()),
    )
    .map_err(|error| original(error, &directory))?;
    let canonical_path = Kura::canonical_storage_path(root);
    let canonical = iroha_fs::OwnerDirectory::open_or_create(&canonical_path)
        .map_err(|error| io(&canonical_path, error))?;
    let names = [
        DATA_FILE_NAME,
        INDEX_FILE_NAME,
        HASHES_FILE_NAME,
        COUNT_FILE_NAME,
    ];
    for (index, name) in names.iter().enumerate() {
        if let Some(existing) = canonical
            .read_regular_optional(name, images[index].len().max(MAX_BLOCK_COMMIT_MARKER_BYTES))
            .map_err(|error| io(&canonical_path, error))?
        {
            let empty_marker = index == 3
                && norito::decode_canonical::<BlockStoreCommitMarker>(&existing)
                    .is_ok_and(|old| old == BlockStoreCommitMarker::new(0, None));
            if existing.as_slice() != images[index] && !existing.is_empty() && !empty_marker {
                return Err(invalid(
                    &canonical_path,
                    "checkpoint recovery cannot replace occupied or substituted canonical bytes",
                ));
            }
        }
    }
    // The stable count is last. An interruption before it leaves only the original durable
    // empty boundary plus this retained intent; the next opener repeats full verification.
    for ((name, bytes), source) in names.iter().zip(&images).zip(&sources) {
        source
            .revalidate()
            .map_err(|error| io(source.path(), error))?;
        manifest_source
            .revalidate()
            .map_err(|error| io(manifest_source.path(), error))?;
        canonical
            .write_atomic(name, bytes, iroha_fs::PublishMode::Replace)
            .map_err(|error| io(&canonical_path, error))?;
    }
    owner
        .remove_private(INTENT)
        .map_err(|error| io(&directory, error))?;
    Ok(())
}

impl BlockStore {
    /// Authenticate the durable omission corridor before any reconciliation or sparse read.
    pub(super) fn verify_checkpoint_prefix(
        &mut self,
        hashes: &[HashOf<BlockHeader>],
    ) -> Result<Option<VerifiedPortableCheckpointV1>> {
        self.checkpoint_prefix = None;
        if self.path_to_blockchain.as_os_str().is_empty() {
            return Ok(None);
        }
        let directory = root_for_store(self)?.join(DIRECTORY);
        let Some((manifest_source, manifest)) = manifest(&directory)? else {
            return Ok(None);
        };
        let count = usize::try_from(manifest.height)?;
        if hashes.len() <= count
            || hashes.first() != Some(&manifest.genesis)
            || hashes.get(count) != Some(&manifest.successor)
        {
            return Err(invalid(
                &directory,
                "checkpoint manifest is outside the original hash journal",
            ));
        }
        let mut journal_sources = Vec::new();
        for name in [
            DATA_FILE_NAME,
            INDEX_FILE_NAME,
            HASHES_FILE_NAME,
            COUNT_FILE_NAME,
        ] {
            let path = self.path_to_blockchain.join(name);
            journal_sources.push(Source::capture(&path).map_err(|error| io(&path, error))?);
        }
        if self.read_block_hashes(0, count + 1)? != hashes[..=count] {
            return Err(invalid(
                &directory,
                "portable checkpoint hash journal changed before verification",
            ));
        }
        let genesis_index = self.read_block_index(0)?;
        let successor_index = self.read_block_index(manifest.height)?;
        if genesis_index.start != 0
            || genesis_index.length != manifest.genesis_len
            || successor_index.start != manifest.genesis_len
            || successor_index.length != manifest.successor_len
        {
            return Err(invalid(
                &directory,
                "portable checkpoint lost retained genesis or actual successor bytes",
            ));
        }
        let mut genesis_wire = zeroed(usize::try_from(genesis_index.length)?)?;
        self.read_block_data(genesis_index.start, &mut genesis_wire)?;
        let genesis = decode_framed_signed_block(&genesis_wire).map_err(Error::CodecAttempt)?;
        if Hash::new(&genesis_wire) != manifest.genesis_wire {
            return Err(invalid(
                &directory,
                "portable checkpoint genesis wire changed",
            ));
        }
        let mut successor_wire = zeroed(usize::try_from(successor_index.length)?)?;
        self.read_block_data(successor_index.start, &mut successor_wire)?;
        if Hash::new(&successor_wire) != manifest.successor_wire {
            return Err(invalid(
                &directory,
                "portable checkpoint successor wire changed",
            ));
        }
        let verified = verify_files(
            &directory,
            &manifest_source,
            &manifest,
            &genesis,
            &hashes[..count],
            &mut |_| Ok(()),
        )
        .map_err(|error| original(error, &directory))?;
        for source in journal_sources {
            source
                .revalidate()
                .map_err(|error| io(source.path(), error))?;
        }
        self.checkpoint_prefix = Some(verified.clone());
        Ok(Some(verified))
    }

    /// Authenticate the retained original prefix before a staged canonical-file recovery.
    /// Actual promoted genesis/successor bodies are independently checked by ordinary init.
    pub(super) fn verify_checkpoint_recovery_prefix(&mut self) -> Result<()> {
        if self.checkpoint_prefix.is_some() {
            return Ok(());
        }
        if self.path_to_blockchain.as_os_str().is_empty() {
            return Ok(());
        }
        let directory = root_for_store(self)?.join(DIRECTORY);
        let Some((source, manifest)) = manifest(&directory)? else {
            return Ok(());
        };
        let count = usize::try_from(manifest.height)?;
        let hash_path = self.path_to_blockchain.join(HASHES_FILE_NAME);
        let hash_source = Source::capture(&hash_path).map_err(|error| io(&hash_path, error))?;
        let hashes = self.read_block_hashes(0, count)?;
        let (data_source, bytes) = read(
            &directory.join(STAGED[0]),
            usize::try_from(manifest.genesis_len + manifest.successor_len)?,
        )?;
        if Hash::new(&bytes) != manifest.data
            || bytes.len() as u64 != manifest.genesis_len + manifest.successor_len
        {
            return Err(invalid(
                &directory,
                "portable recovery original bodies changed",
            ));
        }
        let genesis = decode_framed_signed_block(&bytes[..usize::try_from(manifest.genesis_len)?])
            .map_err(Error::CodecAttempt)?;
        let verified = verify_files(
            &directory,
            &source,
            &manifest,
            &genesis,
            &hashes,
            &mut |_| Ok(()),
        )
        .map_err(|error| original(error, &directory))?;
        data_source
            .revalidate()
            .map_err(|error| io(data_source.path(), error))?;
        hash_source
            .revalidate()
            .map_err(|error| io(hash_source.path(), error))?;
        self.checkpoint_prefix = Some(verified);
        Ok(())
    }

    /// Fast opening still authenticates every omitted parent before exposing its journal.
    pub(super) fn verify_fast_checkpoint_prefix(&mut self, durable_count: usize) -> Result<()> {
        let directory = root_for_store(self)?.join(DIRECTORY);
        let Some((_, manifest)) = manifest(&directory)? else {
            return Ok(());
        };
        let count = usize::try_from(manifest.height)?
            .checked_add(1)
            .ok_or_else(|| invalid(&directory, "portable prefix count overflow"))?;
        if count > durable_count {
            return Err(invalid(&directory, "portable prefix exceeds durable count"));
        }
        let hashes = self.read_block_hashes(0, count)?;
        self.verify_checkpoint_prefix(&hashes)?;
        Ok(())
    }

    pub(super) fn valid_checkpoint_omission(&self, height: u64, slot: &BlockIndex) -> bool {
        slot.is_checkpoint_omitted()
            && height >= 2
            && self
                .checkpoint_prefix
                .as_ref()
                .is_some_and(|proof| height <= proof.checkpoint().header().height().get())
    }
}

impl Kura {
    /// Read one retained original proof frame as untrusted DATA for a new verification walk.
    pub(crate) fn checkpoint_prefix_frame(
        &self,
        height: u64,
    ) -> proof::Attempt<Option<PortablePrefixFrameV1>> {
        let (mut prune_releases, mut canonical_releases) = (
            self.prune_lock.defer_notifications(),
            self.canonical_chain_lock.defer_notifications(),
        );
        let _prune = prune_releases.lock();
        self.ensure_prune_recovery_not_required()
            .map_err(proof::storage_error)?;
        let _canonical = canonical_releases.lock();
        self.ensure_canonical_storage_not_poisoned()
            .map_err(proof::storage_error)?;
        let Some(cut) = self.block_store.lock().checkpoint_prefix.clone() else {
            return Ok(None);
        };
        let last = cut.checkpoint().header().height().get().saturating_add(1);
        if height < 2 || height > last {
            return Ok(None);
        }
        let directory = self.store_root.join(DIRECTORY);
        let Some((source, manifest)) = manifest(&directory).map_err(proof::storage_error)? else {
            return fail(&directory, "retained portable proof lost its manifest");
        };
        if manifest.height + 1 != last
            || manifest.network != cut.network_id()
            || manifest.chain.to_string() != cut.chain_id()
        {
            return fail(
                &directory,
                "retained portable proof manifest differs from the installed cut",
            );
        }
        let path = directory.join(frame_name(height));
        let (frame_source, bytes) =
            read(&path, MAX_PORTABLE_PREFIX_FRAME_BYTES).map_err(proof::storage_error)?;
        let frame =
            PortablePrefixFrameV1::decode_canonical(&bytes).map_err(proof::finality_error)?;
        frame_source
            .revalidate()
            .map_err(|error| proof::storage_error(io(&path, error)))?;
        source
            .revalidate()
            .map_err(|error| proof::storage_error(io(source.path(), error)))?;
        Ok(Some(frame))
    }

    /// Stage and publish a portable checkpoint into a fresh, unopened canonical store.
    ///
    /// Original full genesis and H+1 bodies remain available; heights 2..H have explicit
    /// missing-body slots. State publication and the native H+1 resume join remain the caller's
    /// responsibility. A durable intent owns recovery after any partial file promotion.
    ///
    /// # Errors
    /// Rejects occupied stores, changed sources, invalid proofs, quota overflow or native I/O.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_portable_checkpoint_store(
        config: &Config,
        configured_catalog: Hash,
        chain_id: &ChainId,
        network: NetworkId,
        genesis: &SignedBlock,
        hashes: &[HashOf<BlockHeader>],
        successor: &SignedBlock,
        read_frame: &mut dyn FnMut(u64) -> proof::Attempt<PortablePrefixFrameV1>,
    ) -> proof::Attempt<VerifiedPortableCheckpointV1> {
        let requested = config.store_dir.resolve_relative_path();
        let root = iroha_fs::OwnerDirectory::open_or_create(&requested)
            .map_err(|error| proof::storage_error(io(&requested, error)))?;
        let root_path = root.path().to_path_buf();
        let _lock =
            Self::acquire_store_root_lock(&root_path, true).map_err(proof::storage_error)?;
        // The exported store retains the original operator's configured catalog. Establish
        // its ordinary startup journal while the destination is pristine, before any body
        // or proof paths exist; startup must still compare it with its own configuration.
        Self::establish_or_verify_configured_lane_catalog_baseline_with_lock(
            &root_path,
            configured_catalog,
            &_lock,
        )
        .map_err(proof::storage_error)?;
        recover(&root_path, true).map_err(proof::storage_error)?;
        let canonical_path = Self::canonical_storage_path(&root_path);
        let canonical = iroha_fs::OwnerDirectory::open_or_create(&canonical_path)
            .map_err(|error| proof::storage_error(io(&canonical_path, error)))?;
        require_empty(&canonical).map_err(proof::storage_error)?;
        if hashes.len() < 2
            || hashes.first() != Some(&genesis.hash())
            || successor.header().height().get() != hashes.len() as u64 + 1
            || successor.header().prev_block_hash().as_ref() != hashes.last()
            || NetworkId::from_genesis_hash(genesis.hash()) != network
        {
            return fail(
                &root_path,
                "bootstrap genesis, successor and checkpoint journal differ",
            );
        }
        let proof_dir = root
            .ensure_child(DIRECTORY)
            .map_err(|error| proof::storage_error(io(&root_path.join(DIRECTORY), error)))?;
        let directory = proof_dir.path();
        let encode = |value: &SignedBlock| {
            norito::core::classify_decode_attempt(|| value.encode_wire())
                .map_err(|error| proof::storage_error(Error::CodecAttempt(error)))
        };
        let genesis_wire = encode(genesis)?;
        let successor_wire = encode(successor)?;
        if genesis_wire.len() as u64 > STRICT_INIT_MAX_BLOCK_BYTES
            || successor_wire.len() as u64 > STRICT_INIT_MAX_BLOCK_BYTES
        {
            return fail(directory, "bootstrap body exceeds canonical block bound");
        }
        let mut data = reserved(genesis_wire.len().saturating_add(successor_wire.len()))
            .map_err(proof::storage_error)?;
        data.extend_from_slice(&genesis_wire);
        data.extend_from_slice(&successor_wire);
        let mut indices = reserved((hashes.len() + 1).saturating_mul(BlockIndex::SIZE_USIZE))
            .map_err(proof::storage_error)?;
        let mut journal = reserved((hashes.len() + 1).saturating_mul(Hash::LENGTH))
            .map_err(proof::storage_error)?;
        indices.extend_from_slice(
            &BlockIndex {
                start: 0,
                length: genesis_wire.len() as u64,
            }
            .encode(),
        );
        for _ in 1..hashes.len() {
            indices.extend_from_slice(
                &BlockIndex {
                    start: CHECKPOINT_OMITTED_BLOCK_START,
                    length: 0,
                }
                .encode(),
            );
        }
        indices.extend_from_slice(
            &BlockIndex {
                start: genesis_wire.len() as u64,
                length: successor_wire.len() as u64,
            }
            .encode(),
        );
        for hash in hashes.iter().chain(std::iter::once(&successor.hash())) {
            journal.extend_from_slice(hash.as_ref());
        }
        let marker = BlockStoreCommitMarker::new(hashes.len() as u64 + 1, Some(successor.hash()));
        let marker_bytes =
            norito::core::classify_decode_attempt(|| norito::encode_canonical(&marker))
                .map_err(|error| proof::storage_error(Error::CodecAttempt(error)))?;
        let limit = config.max_disk_usage_bytes.get();
        let existing = Self::directory_tree_file_bytes(&root_path).map_err(proof::storage_error)?;
        // Reserve both staged and promoted canonical images, plus atomic writer scratch.
        let mut required = existing
            .saturating_add(
                3 * (data.len() + indices.len() + journal.len() + marker_bytes.len()) as u64,
            )
            .saturating_add(3 * MAX_MANIFEST as u64);
        let check_budget = |required| {
            if limit != 0 && required > limit {
                Err(proof::storage_error(Error::StorageBudgetExceeded {
                    limit,
                    used: existing,
                    required,
                }))
            } else {
                Ok(())
            }
        };
        check_budget(required)?;
        let mut files_digest = Hash::new(b"iroha-checkpoint-prefix-files-v1");
        let verified = proof::verify(
            genesis,
            chain_id,
            network,
            hashes,
            &mut |height| {
                let frame = read_frame(height)?;
                if height == hashes.len() as u64 + 1 {
                    let certificate = successor.commit_certificate().ok_or_else(|| {
                        proof::storage_error(invalid(
                            directory,
                            "actual successor has no native certificate",
                        ))
                    })?;
                    if certificate.consensus_header() != frame.certificate.consensus_header
                        || certificate.commit_qc() != frame.certificate.commit_qc
                        || certificate.result_preimage() != frame.certificate.result_preimage
                        || certificate.availability() != frame.availability
                    {
                        return fail(
                            directory,
                            "actual successor certificate differs from original portable successor",
                        );
                    }
                }
                let bytes = frame.to_canonical_bytes().map_err(proof::finality_error)?;
                required = required.saturating_add(2 * bytes.len() as u64);
                check_budget(required)?;
                proof_dir
                    .write_private_atomic(
                        frame_name(height),
                        &bytes,
                        iroha_fs::PublishMode::Replace,
                    )
                    .map_err(|error| proof::storage_error(io(directory, error)))?;
                files_digest = digest(files_digest, height, &bytes);
                Ok(frame)
            },
            &mut |_| Ok(()),
        )?;
        let manifest = Manifest {
            version: 1,
            chain: chain_id.clone(),
            network,
            height: hashes.len() as u64,
            genesis: genesis.hash(),
            successor: successor.hash(),
            genesis_len: genesis_wire.len() as u64,
            successor_len: successor_wire.len() as u64,
            genesis_wire: Hash::new(&genesis_wire),
            successor_wire: Hash::new(&successor_wire),
            frames: files_digest,
            data: Hash::new(&data),
            index: Hash::new(&indices),
            hashes: Hash::new(&journal),
        };
        let bytes = norito::core::classify_decode_attempt(|| norito::encode_canonical(&manifest))
            .map_err(|error| proof::storage_error(Error::CodecAttempt(error)))?;
        if bytes.len() > MAX_MANIFEST {
            return fail(directory, "bootstrap manifest exceeds bound");
        }
        for (name, bytes) in STAGED.iter().zip([
            data.as_slice(),
            indices.as_slice(),
            journal.as_slice(),
            marker_bytes.as_slice(),
        ]) {
            proof_dir
                .write_private_atomic(name, bytes, iroha_fs::PublishMode::Replace)
                .map_err(|error| proof::storage_error(io(directory, error)))?;
        }
        proof_dir
            .write_private_atomic(MANIFEST, &bytes, iroha_fs::PublishMode::Replace)
            .map_err(|error| proof::storage_error(io(directory, error)))?;
        require_empty(&canonical).map_err(proof::storage_error)?;
        // The intent binds the entire already durable manifest. Only this owner may finish
        // promotion after a crash; an uncommitted staging prefix cannot authorize holes.
        proof_dir
            .write_private_atomic(
                INTENT,
                Hash::new(&bytes).as_ref(),
                iroha_fs::PublishMode::CreateNew,
            )
            .map_err(|error| proof::storage_error(io(directory, error)))?;
        recover(&root_path, true).map_err(proof::storage_error)?;
        root.revalidate()
            .map_err(|error| proof::storage_error(io(&root_path, error)))?;
        Ok(verified)
    }

    /// Highest unavailable body authorized by the startup-verified portable prefix.
    pub(crate) fn checkpoint_prefix_height(&self) -> Option<u64> {
        self.block_store
            .lock()
            .checkpoint_prefix
            .as_ref()
            .map(|verified| verified.checkpoint().header().height().get())
    }

    /// Rewalk original durable proof files against this exact journal for State authentication.
    /// No result grants a native full-body receipt or replay authority.
    pub(crate) fn walk_checkpoint_prefix(
        &self,
        chain_id: &ChainId,
        network: NetworkId,
        hashes: &[HashOf<BlockHeader>],
        observe: &mut dyn FnMut(&PortableCertifiedBlockV1) -> proof::Attempt<()>,
    ) -> proof::Attempt<Option<VerifiedPortableCheckpointV1>> {
        self.walk_checkpoint_prefix_lookup(
            chain_id,
            network,
            hashes.len(),
            &|index| hashes.get(index).copied(),
            observe,
        )
    }

    /// Rewalk a pinned hash generation without copying that generation into a second vector.
    pub(crate) fn walk_checkpoint_prefix_lookup(
        &self,
        chain_id: &ChainId,
        network: NetworkId,
        hash_count: usize,
        hash_at: &dyn Fn(usize) -> Option<HashOf<BlockHeader>>,
        observe: &mut dyn FnMut(&PortableCertifiedBlockV1) -> proof::Attempt<()>,
    ) -> proof::Attempt<Option<VerifiedPortableCheckpointV1>> {
        let (mut prune_releases, mut canonical_releases) = (
            self.prune_lock.defer_notifications(),
            self.canonical_chain_lock.defer_notifications(),
        );
        let _prune = prune_releases.lock();
        self.ensure_prune_recovery_not_required()
            .map_err(proof::storage_error)?;
        let _canonical = canonical_releases.lock();
        self.ensure_canonical_storage_not_poisoned()
            .map_err(proof::storage_error)?;
        let mut store = self.block_store.lock();
        let directory = self.store_root.join(DIRECTORY);
        let Some((manifest_source, manifest)) =
            manifest(&directory).map_err(proof::storage_error)?
        else {
            return Ok(None);
        };
        if &manifest.chain != chain_id
            || manifest.network != network
            || (hash_count as u64) < manifest.height
        {
            return fail(
                &directory,
                "portable checkpoint differs from the independently selected root",
            );
        }
        let count =
            usize::try_from(manifest.height).map_err(|error| proof::storage_error(error.into()))?;
        let mut sources = Vec::new();
        for name in [
            DATA_FILE_NAME,
            INDEX_FILE_NAME,
            HASHES_FILE_NAME,
            COUNT_FILE_NAME,
        ] {
            let path = store.path_to_blockchain.join(name);
            sources.push(
                Source::capture(&path).map_err(|error| proof::storage_error(io(&path, error)))?,
            );
        }
        let journal = store
            .read_block_hashes(0, count)
            .map_err(proof::storage_error)?;
        if journal
            .iter()
            .enumerate()
            .any(|(index, hash)| hash_at(index) != Some(*hash))
        {
            return fail(&directory, "portable checkpoint observer journal changed");
        }
        let slot = store.read_block_index(0).map_err(proof::storage_error)?;
        if slot.start != 0 || slot.length != manifest.genesis_len {
            return fail(&directory, "portable checkpoint genesis source changed");
        }
        let mut bytes = zeroed(
            usize::try_from(slot.length).map_err(|error| proof::storage_error(error.into()))?,
        )
        .map_err(proof::storage_error)?;
        store
            .read_block_data(0, &mut bytes)
            .map_err(proof::storage_error)?;
        if Hash::new(&bytes) != manifest.genesis_wire {
            return fail(&directory, "portable checkpoint genesis wire changed");
        }
        let genesis = decode_framed_signed_block(&bytes)
            .map_err(|error| proof::storage_error(Error::CodecAttempt(error)))?;
        let verified = verify_files(
            &directory,
            &manifest_source,
            &manifest,
            &genesis,
            &journal,
            observe,
        )?;
        for source in sources {
            source
                .revalidate()
                .map_err(|error| proof::storage_error(io(source.path(), error)))?;
        }
        Ok(Some(verified))
    }
}
