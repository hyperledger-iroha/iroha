//! Crash-safe publication of immutable Mochi configuration generations.
use crate::supervisor::{Result, SupervisorError};
use iroha_crypto::{HashOf, PublicKey};
use iroha_data_model::{NetworkId, block::BlockHeader};
use norito::json::{self, Map, Value};
use rand::{TryRngCore as _, rngs::OsRng};
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::{
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
};
pub(crate) const GENERATIONS_DIRECTORY: &str = "generations";
pub(crate) const CURRENT_GENERATION_FILE: &str = "current-generation";
const GENERATION_LOCK_FILE: &str = ".generation.lock";
const GENERATION_INVENTORY_FILE: &str = "generation.json";
const GENERATION_SCHEMA: u64 = 1;
const GENERATION_POINTER_TEMP_PREFIX: &str = ".current-generation.";
const GENERATION_POINTER_TEMP_SUFFIX: &str = ".tmp";
const GENERATION_FILE_HASH_BUFFER_BYTES: usize = 64 * 1024;
// A generation is a seven-peer-at-most Mochi configuration bundle whose runtime
// storage is pristine at publication. These V1 ceilings are deliberately far
// above that source-derived shape while making corrupt directory and inventory
// growth a fail-closed protocol error instead of a process-memory decision.
const GENERATION_TREE_MAX_ENTRIES_V1: usize = 16_384;
const GENERATION_TREE_MAX_DEPTH_V1: usize = 32;
const GENERATION_INVENTORY_MAX_FILES_V1: usize = 8_192;
const GENERATION_INVENTORY_MAX_PATH_BYTES_V1: usize = 4 * 1024 * 1024;
const GENERATION_INVENTORY_MAX_PATH_BYTES_PER_FILE_V1: usize = 4 * 1024;
const GENERATION_INVENTORY_MAX_BYTES_V1: usize = 8 * 1024 * 1024;
const GENERATION_SMALL_RECORD_MAX_BYTES_V1: usize = 4 * 1024;
const GENERATION_MAX_PEER_DIRECTORIES_V1: usize = 7;
const GENERATION_ID_RECORD_BYTES: usize = 32 + 1;
#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PublicationFaultPoint {
    BeforeInventory,
    AfterInventory,
    AfterTreeSync,
    AfterGenerationsSync,
    AfterRuntimeStorageSync,
    AfterPointerWrite,
    AfterPointerSync,
    AfterPointerRename,
}
/// Metadata bound into an immutable generation inventory.
#[derive(Clone, Copy)]
pub(crate) struct GenerationInventoryContext<'a> {
    pub(crate) chain_id: &'a str,
    pub(crate) chain_discriminant: u16,
    pub(crate) genesis_public_key: &'a PublicKey,
    pub(crate) expected_hash: HashOf<BlockHeader>,
}
/// Exact metadata recovered from a strictly verified generation inventory.
#[derive(Debug)]
pub(crate) struct VerifiedGeneration {
    pub(crate) root: PathBuf,
    pub(crate) generation_id: String,
    pub(crate) chain_id: String,
    pub(crate) chain_discriminant: u16,
    pub(crate) genesis_public_key: PublicKey,
    pub(crate) expected_hash: HashOf<BlockHeader>,
}
/// Exclusive, unpublished generation transaction.
#[derive(Debug)]
pub(crate) struct GenerationTransaction {
    network_root: PathBuf,
    generation_root: PathBuf,
    id: String,
    expected_base_generation: Option<String>,
    runtime_storage_roots: Vec<PathBuf>,
    pointer_temporary: PathBuf,
    pointer_temporary_file: Option<File>,
    _lock: File,
    committed: bool,
}
/// A committed publication that retains the exclusive generation lock.
///
/// Callers must keep this guard alive until their in-memory state, selected
/// pointer checks, and peer lifecycle have all reconciled with the commit.
#[derive(Debug)]
pub(crate) struct PublishedGeneration {
    transaction: GenerationTransaction,
    durability_error: Option<std::io::Error>,
}
/// Failed pre-commit publication that retains the exclusive generation lock.
#[derive(Debug)]
pub(crate) struct FailedGenerationPublication {
    _transaction: GenerationTransaction,
    error: Option<SupervisorError>,
}
impl FailedGenerationPublication {
    pub(crate) fn take_error(&mut self) -> SupervisorError {
        self.error
            .take()
            .expect("failed publication error can only be taken once")
    }
}
impl PublishedGeneration {
    pub(crate) fn id(&self) -> &str {
        self.transaction.id()
    }
    pub(crate) fn take_uncertainty(&mut self) -> Option<SupervisorError> {
        self.durability_error
            .take()
            .map(|source| SupervisorError::PublicationUncertain {
                generation_id: self.transaction.id.clone(),
                source,
            })
    }
}
impl GenerationTransaction {
    #[cfg(test)]
    pub(crate) fn begin(root: &Path) -> Result<Self> {
        let expected_base_generation = current_generation_id(root)?;
        Self::begin_replacing(root, expected_base_generation)
    }
    /// Acquire the network generation lock and allocate an invisible candidate
    /// bound to the caller's expected base selection.
    pub(crate) fn begin_replacing(
        root: &Path,
        expected_base_generation: Option<String>,
    ) -> Result<Self> {
        fs::create_dir_all(root)?;
        reject_symlink(root, "network root")?;
        let root = fs::canonicalize(root)?;
        let lock_path = root.join(GENERATION_LOCK_FILE);
        reject_symlink(&lock_path, "generation lock")?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true);
        #[cfg(unix)]
        options.mode(0o600);
        let lock = options.open(&lock_path)?;
        validate_lock_file(&lock_path, &lock)?;
        lock.try_lock().map_err(|error| match error {
            fs::TryLockError::WouldBlock => SupervisorError::GenerationLocked {
                path: lock_path.clone(),
            },
            fs::TryLockError::Error(error) => SupervisorError::Io(error),
        })?;
        validate_lock_file(&lock_path, &lock)?;
        let generations = root.join(GENERATIONS_DIRECTORY);
        reject_symlink(&generations, "generations directory")?;
        fs::create_dir_all(&generations)?;
        validate_contained_directory(&root, &generations, "generations directory")?;
        #[cfg(unix)]
        fs::set_permissions(&generations, fs::Permissions::from_mode(0o700))?;
        recover_abandoned_generation_transactions(&root, &generations)?;
        for _ in 0..32 {
            let mut entropy = [0_u8; 16];
            OsRng.try_fill_bytes(&mut entropy).map_err(|error| {
                SupervisorError::Config(format!(
                    "failed to obtain OS entropy for Mochi generation id: {error}"
                ))
            })?;
            let id = encode_lower_hex(&entropy);
            let generation_root = generations.join(&id);
            match fs::symlink_metadata(&generation_root) {
                Ok(_) => continue,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            // The private pointer temporary doubles as the durable ownership
            // marker for every path created by this transaction. Persist it
            // before allocating the candidate directory, so a later writer can
            // distinguish crash residue from intentionally retained history.
            let pointer_temporary = generation_pointer_temporary_path(&root, &id);
            let mut marker_options = OpenOptions::new();
            marker_options.read(true).write(true).create_new(true);
            #[cfg(unix)]
            marker_options.mode(0o600);
            let pointer_temporary_file = match marker_options.open(&pointer_temporary) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
            };
            pointer_temporary_file.sync_all()?;
            sync_directory(&root)?;
            match fs::create_dir(&generation_root) {
                Ok(()) => {
                    #[cfg(unix)]
                    fs::set_permissions(&generation_root, fs::Permissions::from_mode(0o700))?;
                    return Ok(Self {
                        network_root: root.clone(),
                        generation_root,
                        id,
                        expected_base_generation,
                        runtime_storage_roots: Vec::new(),
                        pointer_temporary,
                        pointer_temporary_file: Some(pointer_temporary_file),
                        _lock: lock,
                        committed: false,
                    });
                }
                Err(error) => {
                    drop(pointer_temporary_file);
                    fs::remove_file(&pointer_temporary)?;
                    sync_directory(&root)?;
                    if error.kind() == std::io::ErrorKind::AlreadyExists {
                        continue;
                    }
                    return Err(error.into());
                }
            }
        }
        Err(SupervisorError::Config(
            "failed to allocate a unique Mochi generation id".to_owned(),
        ))
    }
    /// Return the immutable identifier allocated for this candidate.
    pub(crate) fn id(&self) -> &str {
        &self.id
    }
    /// Return the candidate's final absolute filesystem root.
    pub(crate) fn root(&self) -> &Path {
        &self.generation_root
    }
    /// Allocate and track one mutable runtime-storage root owned by this candidate.
    pub(crate) fn create_runtime_storage(&mut self, alias: &str) -> Result<PathBuf> {
        let alias_path = Path::new(alias);
        let mut components = alias_path.components();
        if !matches!(components.next(), Some(std::path::Component::Normal(_)))
            || components.next().is_some()
        {
            return Err(SupervisorError::GenerationValidation(format!(
                "peer alias `{alias}` is not one safe path component"
            )));
        }
        let peers = self.network_root.join("peers");
        ensure_direct_child_directory(&self.network_root, &peers, "runtime peers directory")?;
        let peer = peers.join(alias);
        ensure_direct_child_directory(&peers, &peer, "runtime peer directory")?;
        let storage_generations = peer.join("storage-generations");
        ensure_direct_child_directory(
            &peer,
            &storage_generations,
            "runtime storage-generations directory",
        )?;
        let storage = storage_generations.join(&self.id);
        reject_symlink(&storage, "candidate runtime storage")?;
        fs::create_dir(&storage)?;
        self.runtime_storage_roots.push(storage.clone());
        #[cfg(unix)]
        fs::set_permissions(&storage, fs::Permissions::from_mode(0o700))?;
        Ok(storage)
    }
    /// Seal the inventory, sync the candidate, and atomically publish it.
    pub(crate) fn publish(
        self,
        context: GenerationInventoryContext<'_>,
    ) -> Result<PublishedGeneration> {
        match self.publish_retaining_failure(context) {
            Ok(publication) => Ok(publication),
            Err(mut failure) => Err(failure.take_error()),
        }
    }
    #[cfg(test)]
    pub(crate) fn publish_with_fault(
        self,
        context: GenerationInventoryContext<'_>,
        fault: PublicationFaultPoint,
    ) -> Result<PublishedGeneration> {
        match self.publish_with_fault_retaining_failure(context, fault) {
            Ok(publication) => Ok(publication),
            Err(mut failure) => Err(failure.take_error()),
        }
    }
    #[expect(clippy::result_large_err, reason = "failure retains generation lock")]
    pub(crate) fn publish_retaining_failure(
        self,
        context: GenerationInventoryContext<'_>,
    ) -> std::result::Result<PublishedGeneration, FailedGenerationPublication> {
        #[cfg(test)]
        return self.publish_inner(context, None);
        #[cfg(not(test))]
        self.publish_inner(context)
    }
    #[cfg(test)]
    #[expect(clippy::result_large_err, reason = "failure retains generation lock")]
    pub(crate) fn publish_with_fault_retaining_failure(
        self,
        context: GenerationInventoryContext<'_>,
        fault: PublicationFaultPoint,
    ) -> std::result::Result<PublishedGeneration, FailedGenerationPublication> {
        self.publish_inner(context, Some(fault))
    }
    #[expect(clippy::result_large_err, reason = "failure retains generation lock")]
    fn publish_inner(
        mut self,
        context: GenerationInventoryContext<'_>,
        #[cfg(test)] fault: Option<PublicationFaultPoint>,
    ) -> std::result::Result<PublishedGeneration, FailedGenerationPublication> {
        #[cfg(test)]
        let result = self.try_publish_inner(context, fault);
        #[cfg(not(test))]
        let result = self.try_publish_inner(context);
        match result {
            Ok(durability_error) => Ok(PublishedGeneration {
                transaction: self,
                durability_error,
            }),
            Err(error) => Err(FailedGenerationPublication {
                _transaction: self,
                error: Some(error),
            }),
        }
    }
    fn try_publish_inner(
        &mut self,
        context: GenerationInventoryContext<'_>,
        #[cfg(test)] fault: Option<PublicationFaultPoint>,
    ) -> Result<Option<std::io::Error>> {
        #[cfg(test)]
        inject_fault(fault, PublicationFaultPoint::BeforeInventory)?;
        self.write_inventory(&context)?;
        #[cfg(test)]
        inject_fault(fault, PublicationFaultPoint::AfterInventory)?;
        sync_tree(&self.generation_root)?;
        #[cfg(test)]
        inject_fault(fault, PublicationFaultPoint::AfterTreeSync)?;
        sync_directory(
            self.generation_root
                .parent()
                .expect("generation root always has a parent"),
        )?;
        #[cfg(test)]
        inject_fault(fault, PublicationFaultPoint::AfterGenerationsSync)?;
        self.sync_runtime_storage_roots()?;
        // Persist the `generations/` and `peers/` root entries before the
        // pointer can select either hierarchy. The post-rename sync below is
        // then responsible only for committing the pointer replacement.
        sync_directory(&self.network_root)?;
        #[cfg(test)]
        inject_fault(fault, PublicationFaultPoint::AfterRuntimeStorageSync)?;
        // The transaction has held the exclusive generation lock since its
        // candidate was allocated. Compare the selected base immediately
        // before creating the replacement pointer so a stale Supervisor can
        // never roll the sandbox back to paths derived from a retired base.
        let actual_base_generation = current_generation_id(&self.network_root)?;
        if actual_base_generation != self.expected_base_generation {
            return Err(SupervisorError::GenerationSelectionChanged {
                expected: self.expected_base_generation.clone(),
                actual: actual_base_generation,
            });
        }
        let pointer = self.network_root.join(CURRENT_GENERATION_FILE);
        reject_symlink(&pointer, "current generation pointer")?;
        let mut file = self.pointer_temporary_file.take().ok_or_else(|| {
            SupervisorError::GenerationValidation(
                "generation transaction lost its durable pointer temporary".to_owned(),
            )
        })?;
        validate_generation_pointer_temporary_file(&self.pointer_temporary, &file)?;
        if file.metadata()?.len() != 0 {
            return Err(SupervisorError::GenerationValidation(format!(
                "generation pointer temporary `{}` changed before publication",
                self.pointer_temporary.display()
            )));
        }
        file.write_all(self.id.as_bytes())?;
        file.write_all(b"\n")?;
        #[cfg(test)]
        if fault == Some(PublicationFaultPoint::AfterPointerWrite) {
            return Err(injected_fault(PublicationFaultPoint::AfterPointerWrite));
        }
        file.sync_all()?;
        validate_generation_pointer_temporary_file(&self.pointer_temporary, &file)?;
        if file.metadata()?.len() != (self.id.len() + 1) as u64 {
            return Err(SupervisorError::GenerationValidation(format!(
                "generation pointer temporary `{}` has the wrong length",
                self.pointer_temporary.display()
            )));
        }
        drop(file);
        validate_generation_pointer_temporary(&self.pointer_temporary, &self.id)?;
        #[cfg(test)]
        if fault == Some(PublicationFaultPoint::AfterPointerSync) {
            return Err(injected_fault(PublicationFaultPoint::AfterPointerSync));
        }
        let verified = verify_selected_generation(&self.network_root, &self.id)?;
        ensure_inventory_context(&verified, context)?;
        if let Err(error) = fs::rename(&self.pointer_temporary, &pointer) {
            return Err(error.into());
        }
        // The atomic pointer replacement is the commit point. Never remove a
        // generation after this point, even if directory durability is unknown.
        self.committed = true;
        #[cfg(test)]
        let durability_error = if fault == Some(PublicationFaultPoint::AfterPointerRename) {
            Some(std::io::Error::other(
                "injected generation publication fault after pointer rename",
            ))
        } else {
            sync_directory(&self.network_root).err()
        };
        #[cfg(not(test))]
        let durability_error = sync_directory(&self.network_root).err();
        Ok(durability_error)
    }
    fn sync_runtime_storage_roots(&self) -> Result<()> {
        for storage in &self.runtime_storage_roots {
            if !candidate_runtime_storage_is_safe(&self.network_root, &self.id, storage) {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate runtime storage `{}` escaped its managed generation",
                    storage.display()
                )));
            }
            sync_tree(storage)?;
            let storage_generations = storage
                .parent()
                .expect("candidate runtime storage always has a managed parent");
            sync_directory(storage_generations)?;
            let peer = storage_generations
                .parent()
                .expect("storage-generations always has a peer parent");
            sync_directory(peer)?;
        }
        if !self.runtime_storage_roots.is_empty() {
            sync_directory(&self.network_root.join("peers"))?;
        }
        Ok(())
    }
    fn write_inventory(&self, context: &GenerationInventoryContext<'_>) -> Result<()> {
        let inventory_path = self.generation_root.join(GENERATION_INVENTORY_FILE);
        let files = generation_file_hashes(&self.generation_root, Some(&inventory_path))?;
        let mut file_values = Vec::new();
        file_values.try_reserve_exact(files.len()).map_err(|_| {
            SupervisorError::GenerationValidation(
                "generation inventory JSON allocation failed".to_owned(),
            )
        })?;
        for (path, hash) in &files {
            let mut entry = Map::new();
            entry.insert("path".to_owned(), Value::String(path.clone()));
            entry.insert("blake3".to_owned(), Value::String(hash.clone()));
            file_values.push(Value::Object(entry));
        }
        let mut inventory = Map::new();
        inventory.insert("schema".to_owned(), Value::Number(GENERATION_SCHEMA.into()));
        inventory.insert("generation_id".to_owned(), Value::String(self.id.clone()));
        inventory.insert(
            "chain_id".to_owned(),
            Value::String(context.chain_id.to_owned()),
        );
        inventory.insert(
            "chain_discriminant".to_owned(),
            Value::Number(u64::from(context.chain_discriminant).into()),
        );
        inventory.insert(
            "genesis_public_key".to_owned(),
            Value::String(context.genesis_public_key.to_string()),
        );
        inventory.insert(
            "expected_hash".to_owned(),
            Value::String(NetworkId::from_genesis_hash(context.expected_hash).to_string()),
        );
        inventory.insert("files".to_owned(), Value::Array(file_values));
        let mut bytes = json::to_json_bounded(
            &Value::Object(inventory),
            GENERATION_INVENTORY_MAX_BYTES_V1 - 1,
        )
        .map_err(|error| {
            SupervisorError::GenerationValidation(format!(
                "generation inventory exceeds its V1 memory envelope: {error}"
            ))
        })?
        .into_bytes();
        bytes.push(b'\n');
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&inventory_path)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        // Re-hash after the inventory write. A concurrent mutation of any
        // candidate artifact therefore invalidates publication.
        let after = generation_file_hashes(&self.generation_root, Some(&inventory_path))?;
        if after != files {
            return Err(SupervisorError::GenerationValidation(
                "candidate generation changed while its inventory was being sealed".to_owned(),
            ));
        }
        Ok(())
    }
}
#[cfg(test)]
fn inject_fault(
    selected: Option<PublicationFaultPoint>,
    current: PublicationFaultPoint,
) -> Result<()> {
    if selected == Some(current) {
        return Err(injected_fault(current));
    }
    Ok(())
}
#[cfg(test)]
fn injected_fault(point: PublicationFaultPoint) -> SupervisorError {
    SupervisorError::GenerationValidation(format!(
        "injected generation publication fault at {point:?}"
    ))
}
impl Drop for GenerationTransaction {
    fn drop(&mut self) {
        if self.committed || !is_generation_id(&self.id) {
            return;
        }
        drop(self.pointer_temporary_file.take());
        let generations = self.network_root.join(GENERATIONS_DIRECTORY);
        let _ = reclaim_abandoned_generation_transaction(
            &self.network_root,
            &generations,
            &self.id,
            &self.pointer_temporary,
        );
    }
}
fn generation_pointer_temporary_path(root: &Path, id: &str) -> PathBuf {
    root.join(format!(
        "{GENERATION_POINTER_TEMP_PREFIX}{id}{GENERATION_POINTER_TEMP_SUFFIX}"
    ))
}
fn generation_pointer_temporary_id(name: &OsStr) -> Result<Option<String>> {
    let Some(name) = name.to_str() else {
        return Ok(None);
    };
    let Some(remainder) = name.strip_prefix(GENERATION_POINTER_TEMP_PREFIX) else {
        return Ok(None);
    };
    let Some(id) = remainder.strip_suffix(GENERATION_POINTER_TEMP_SUFFIX) else {
        return Err(SupervisorError::GenerationValidation(format!(
            "malformed generation pointer temporary `{name}`"
        )));
    };
    if !is_generation_id(id) {
        return Err(SupervisorError::GenerationValidation(format!(
            "generation pointer temporary `{name}` has an invalid generation id"
        )));
    }
    Ok(Some(id.to_owned()))
}
/// Reclaim only transactions carrying Mochi's durable ownership marker.
///
/// The caller holds the exclusive generation lock. Unmarked generation and storage paths are
/// deliberately ignored because they may be intentionally retained publication history, even when
/// that history has since been damaged and no longer passes strict verification.
fn recover_abandoned_generation_transactions(root: &Path, generations: &Path) -> Result<()> {
    let mut entries = 0_usize;
    for marker in fs::read_dir(root)? {
        let marker = marker?;
        admit_generation_tree_entry(&mut entries)?;
        let Some(id) = generation_pointer_temporary_id(&marker.file_name())? else {
            continue;
        };
        reclaim_abandoned_generation_transaction(root, generations, &id, &marker.path())?;
    }
    Ok(())
}
fn reclaim_abandoned_generation_transaction(
    root: &Path,
    generations: &Path,
    id: &str,
    marker: &Path,
) -> Result<()> {
    if marker != generation_pointer_temporary_path(root, id) {
        return Err(SupervisorError::GenerationValidation(format!(
            "generation pointer temporary `{}` is outside its managed path",
            marker.display()
        )));
    }
    validate_generation_pointer_temporary(marker, id)?;
    // A recovered selected pointer is authoritative even if its generation is
    // damaged. It may represent a rename that committed immediately before a
    // crash; remove only the redundant marker in that case.
    if current_generation_id(root)?.as_deref() == Some(id) {
        fs::remove_file(marker)?;
        sync_directory(root)?;
        return Ok(());
    }
    let runtime_storage = abandoned_runtime_storage_paths(root, id)?;
    let generation_root = abandoned_generation_path(generations, id)?;
    // Keep the marker until every associated deletion is durable. If a sync
    // fails, the next exclusive writer can safely resume from the same id.
    for storage in runtime_storage {
        let storage_generations = storage
            .parent()
            .expect("validated runtime storage always has a parent");
        fs::remove_dir_all(&storage)?;
        sync_directory(storage_generations)?;
    }
    if let Some(generation_root) = generation_root {
        fs::remove_dir_all(generation_root)?;
        sync_directory(generations)?;
    }
    fs::remove_file(marker)?;
    sync_directory(root)?;
    Ok(())
}
fn validate_generation_pointer_temporary(path: &Path, id: &str) -> Result<()> {
    let named = fs::symlink_metadata(path)?;
    if named.file_type().is_symlink() || !named.is_file() {
        return Err(SupervisorError::GenerationValidation(format!(
            "generation pointer temporary `{}` must be a regular file",
            path.display()
        )));
    }
    let mut file = OpenOptions::new().read(true).open(path)?;
    validate_generation_pointer_temporary_file(path, &file)?;
    let expected = format!("{id}\n");
    if file.metadata()?.len() > expected.len() as u64 {
        return Err(SupervisorError::GenerationValidation(format!(
            "generation pointer temporary `{}` has malformed contents",
            path.display()
        )));
    }
    let observed_len = usize::try_from(file.metadata()?.len()).map_err(|_| {
        SupervisorError::GenerationValidation(
            "generation pointer temporary length does not fit usize".to_owned(),
        )
    })?;
    let mut bytes = [0_u8; GENERATION_ID_RECORD_BYTES];
    file.read_exact(&mut bytes[..observed_len])?;
    let mut growth_probe = [0_u8; 1];
    if file.read(&mut growth_probe)? != 0
        || !expected.as_bytes().starts_with(&bytes[..observed_len])
    {
        return Err(SupervisorError::GenerationValidation(format!(
            "generation pointer temporary `{}` has malformed contents",
            path.display()
        )));
    }
    validate_generation_pointer_temporary_file(path, &file)?;
    if usize::try_from(file.metadata()?.len()).ok() != Some(observed_len) {
        return Err(SupervisorError::GenerationValidation(format!(
            "generation pointer temporary `{}` changed while it was read",
            path.display()
        )));
    }
    Ok(())
}
fn validate_generation_pointer_temporary_file(path: &Path, file: &File) -> Result<()> {
    let opened = file.metadata()?;
    let named = fs::symlink_metadata(path)?;
    if !opened.is_file() || !named.is_file() || named.file_type().is_symlink() {
        return Err(SupervisorError::GenerationValidation(format!(
            "generation pointer temporary `{}` must be a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        if opened.dev() != named.dev() || opened.ino() != named.ino() || opened.nlink() != 1 {
            return Err(SupervisorError::GenerationValidation(format!(
                "generation pointer temporary `{}` changed while it was opened",
                path.display()
            )));
        }
        if opened.permissions().mode() & 0o077 != 0 {
            return Err(SupervisorError::GenerationValidation(format!(
                "generation pointer temporary `{}` must be owner-only",
                path.display()
            )));
        }
    }
    Ok(())
}
fn abandoned_generation_path(generations: &Path, id: &str) -> Result<Option<PathBuf>> {
    let path = generations.join(id);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(SupervisorError::GenerationValidation(format!(
                "abandoned generation `{}` must be a non-symlink directory",
                path.display()
            )))
        }
        Ok(_) => {
            validate_contained_directory(generations, &path, "abandoned generation")?;
            Ok(Some(path))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
fn abandoned_runtime_storage_paths(root: &Path, id: &str) -> Result<Vec<PathBuf>> {
    let peers = root.join("peers");
    match fs::symlink_metadata(&peers) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(SupervisorError::GenerationValidation(format!(
                "runtime peers directory `{}` must be a non-symlink directory",
                peers.display()
            )));
        }
        Ok(_) => validate_contained_directory(root, &peers, "runtime peers directory")?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    }
    let mut storage_roots = Vec::new();
    storage_roots
        .try_reserve_exact(GENERATION_MAX_PEER_DIRECTORIES_V1)
        .map_err(|_| {
            SupervisorError::GenerationValidation(
                "runtime storage recovery allocation failed".to_owned(),
            )
        })?;
    let mut aliases = 0_usize;
    for alias in fs::read_dir(&peers)? {
        let alias = alias?;
        aliases = aliases.checked_add(1).ok_or_else(|| {
            SupervisorError::GenerationValidation(
                "runtime peer directory count overflowed usize".to_owned(),
            )
        })?;
        if aliases > GENERATION_MAX_PEER_DIRECTORIES_V1 {
            return Err(SupervisorError::GenerationValidation(format!(
                "runtime peers exceed the Mochi V1 {}-peer limit",
                GENERATION_MAX_PEER_DIRECTORIES_V1
            )));
        }
        let peer = alias.path();
        let metadata = fs::symlink_metadata(&peer)?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(SupervisorError::GenerationValidation(format!(
                "runtime peer entry `{}` must be a non-symlink directory",
                peer.display()
            )));
        }
        validate_contained_directory(&peers, &peer, "runtime peer directory")?;
        let storage_generations = peer.join("storage-generations");
        match fs::symlink_metadata(&storage_generations) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(SupervisorError::GenerationValidation(format!(
                    "runtime storage-generations `{}` must be a non-symlink directory",
                    storage_generations.display()
                )));
            }
            Ok(_) => validate_contained_directory(
                &peer,
                &storage_generations,
                "runtime storage-generations directory",
            )?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        }
        let storage = storage_generations.join(id);
        match fs::symlink_metadata(&storage) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(SupervisorError::GenerationValidation(format!(
                    "abandoned runtime storage `{}` must be a non-symlink directory",
                    storage.display()
                )));
            }
            Ok(_) => {
                if !candidate_runtime_storage_is_safe(root, id, &storage) {
                    return Err(SupervisorError::GenerationValidation(format!(
                        "abandoned runtime storage `{}` escaped its managed generation",
                        storage.display()
                    )));
                }
                storage_roots.push(storage);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(storage_roots)
}
/// Acquire a nonblocking shared lease on the selected-generation pointer.
///
/// Exclusive generation publishers use the same lock, so retaining the returned file prevents an
/// API writer from retiring paths resolved under this lease.
pub(crate) fn try_lock_generation_selection(root: &Path) -> Result<File> {
    reject_symlink(root, "network root")?;
    let root = fs::canonicalize(root)?;
    let lock_path = root.join(GENERATION_LOCK_FILE);
    reject_symlink(&lock_path, "generation lock")?;
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    let lock = options.open(&lock_path)?;
    validate_lock_file(&lock_path, &lock)?;
    lock.try_lock_shared().map_err(|error| match error {
        fs::TryLockError::WouldBlock => SupervisorError::GenerationLocked {
            path: lock_path.clone(),
        },
        fs::TryLockError::Error(error) => SupervisorError::Io(error),
    })?;
    validate_lock_file(&lock_path, &lock)?;
    Ok(lock)
}
/// Read the exact currently published generation identifier.
pub(crate) fn current_generation_id(root: &Path) -> Result<Option<String>> {
    let path = root.join(CURRENT_GENERATION_FILE);
    match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(SupervisorError::GenerationValidation(format!(
                "current generation pointer `{}` must not be a symbolic link",
                path.display()
            )));
        }
        Ok(metadata) if !metadata.is_file() => {
            return Err(SupervisorError::GenerationValidation(format!(
                "current generation pointer `{}` must be a regular file",
                path.display()
            )));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let record = read_generation_file_bounded(
        &path,
        "current generation pointer",
        GENERATION_ID_RECORD_BYTES,
    )?;
    let record = std::str::from_utf8(&record).map_err(|_| {
        SupervisorError::GenerationValidation(
            "current-generation must contain canonical UTF-8".to_owned(),
        )
    })?;
    let id = record.strip_suffix('\n').ok_or_else(|| {
        SupervisorError::GenerationValidation(
            "current-generation must end in exactly one newline".to_owned(),
        )
    })?;
    if !is_generation_id(id) {
        return Err(SupervisorError::GenerationValidation(format!(
            "current-generation contains invalid generation id `{id}`"
        )));
    }
    Ok(Some(id.to_owned()))
}
/// Verify that the selected generation is complete and exactly matches its inventory.
pub(crate) fn verify_selected_generation(root: &Path, id: &str) -> Result<VerifiedGeneration> {
    if !is_generation_id(id) {
        return Err(SupervisorError::GenerationValidation(format!(
            "invalid selected generation id `{id}`"
        )));
    }
    reject_symlink(root, "network root")?;
    let root = fs::canonicalize(root)?;
    let generations = root.join(GENERATIONS_DIRECTORY);
    reject_symlink(&generations, "generations directory")?;
    validate_contained_directory(&root, &generations, "generations directory")?;
    let generation_root = generations.join(id);
    let inventory_path = generation_root.join(GENERATION_INVENTORY_FILE);
    reject_symlink(&generation_root, "selected generation")?;
    reject_symlink(&inventory_path, "generation inventory")?;
    if !generation_root.is_dir() || !inventory_path.is_file() {
        return Err(SupervisorError::GenerationValidation(format!(
            "selected generation `{id}` is incomplete"
        )));
    }
    let bytes = read_generation_file_bounded(
        &inventory_path,
        "generation inventory",
        GENERATION_INVENTORY_MAX_BYTES_V1,
    )?;
    const INVENTORY_JSON_ELEMENTS_V1: usize = GENERATION_INVENTORY_MAX_FILES_V1 * 4 + 64;
    json::preflight_slice(
        &bytes,
        json::JsonPreflightLimits::new(
            GENERATION_INVENTORY_MAX_BYTES_V1,
            INVENTORY_JSON_ELEMENTS_V1 + 1,
            GENERATION_INVENTORY_MAX_PATH_BYTES_PER_FILE_V1 * 6 + 2,
            GENERATION_INVENTORY_MAX_PATH_BYTES_PER_FILE_V1,
            GENERATION_INVENTORY_MAX_BYTES_V1,
            GENERATION_INVENTORY_MAX_FILES_V1,
            GENERATION_INVENTORY_MAX_FILES_V1,
            INVENTORY_JSON_ELEMENTS_V1,
            INVENTORY_JSON_ELEMENTS_V1,
            8,
        ),
    )
    .map_err(|error| {
        SupervisorError::GenerationValidation(format!(
            "selected generation `{id}` inventory exceeds its V1 JSON envelope: {error}"
        ))
    })?;
    let value: Value = json::from_slice(&bytes)?;
    let mut canonical = json::to_json_bounded(&value, GENERATION_INVENTORY_MAX_BYTES_V1 - 1)
        .map_err(|error| {
            SupervisorError::GenerationValidation(format!(
                "selected generation `{id}` inventory cannot be canonically bounded: {error}"
            ))
        })?
        .into_bytes();
    canonical.push(b'\n');
    if canonical != bytes {
        return Err(SupervisorError::GenerationValidation(format!(
            "selected generation `{id}` inventory is not canonical Norito JSON"
        )));
    }
    let object = value.as_object().ok_or_else(|| {
        SupervisorError::GenerationValidation(
            "generation inventory must be a JSON object".to_owned(),
        )
    })?;
    const INVENTORY_FIELDS: [&str; 7] = [
        "schema",
        "generation_id",
        "chain_id",
        "chain_discriminant",
        "genesis_public_key",
        "expected_hash",
        "files",
    ];
    if object.len() != INVENTORY_FIELDS.len()
        || !INVENTORY_FIELDS
            .iter()
            .all(|field| object.contains_key(*field))
        || object.get("schema").and_then(Value::as_u64) != Some(GENERATION_SCHEMA)
        || object.get("generation_id").and_then(Value::as_str) != Some(id)
    {
        return Err(SupervisorError::GenerationValidation(format!(
            "selected generation `{id}` has the wrong inventory schema or identity"
        )));
    }
    let chain_id = object
        .get("chain_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.trim() == *value)
        .ok_or_else(|| {
            SupervisorError::GenerationValidation(
                "generation inventory has an invalid chain_id".to_owned(),
            )
        })?
        .to_owned();
    let chain_discriminant = object
        .get("chain_discriminant")
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or_else(|| {
            SupervisorError::GenerationValidation(
                "generation inventory has an invalid chain_discriminant".to_owned(),
            )
        })?;
    let public_record = object
        .get("genesis_public_key")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            SupervisorError::GenerationValidation(
                "generation inventory has no genesis_public_key".to_owned(),
            )
        })?;
    let genesis_public_key = public_record.parse::<PublicKey>().map_err(|error| {
        SupervisorError::GenerationValidation(format!(
            "generation inventory has invalid genesis_public_key: {error}"
        ))
    })?;
    if genesis_public_key.to_string() != public_record {
        return Err(SupervisorError::GenerationValidation(
            "generation inventory genesis_public_key is not canonical".to_owned(),
        ));
    }
    let hash_record = object
        .get("expected_hash")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            SupervisorError::GenerationValidation(
                "generation inventory has no expected_hash".to_owned(),
            )
        })?;
    let expected_network_id = hash_record.parse::<NetworkId>().map_err(|error| {
        SupervisorError::GenerationValidation(format!(
            "generation inventory has invalid expected_hash: {error}"
        ))
    })?;
    let expected_hash = expected_network_id.into_genesis_hash();
    let recorded_entries = object
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            SupervisorError::GenerationValidation(
                "generation inventory omitted its files array".to_owned(),
            )
        })?;
    if recorded_entries.len() > GENERATION_INVENTORY_MAX_FILES_V1 {
        return Err(SupervisorError::GenerationValidation(format!(
            "generation inventory exceeds the V1 {}-file limit",
            GENERATION_INVENTORY_MAX_FILES_V1
        )));
    }
    let mut recorded: Vec<(String, String)> = Vec::new();
    recorded
        .try_reserve_exact(recorded_entries.len())
        .map_err(|_| {
            SupervisorError::GenerationValidation(
                "generation inventory record allocation failed".to_owned(),
            )
        })?;
    let mut recorded_files = 0_usize;
    let mut recorded_path_bytes = 0_usize;
    for entry in recorded_entries {
        let entry = entry.as_object().ok_or_else(|| {
            SupervisorError::GenerationValidation(
                "generation inventory contains a non-object file entry".to_owned(),
            )
        })?;
        if entry.len() != 2 || !entry.contains_key("path") || !entry.contains_key("blake3") {
            return Err(SupervisorError::GenerationValidation(
                "generation inventory file entries must contain exactly `path` and `blake3`"
                    .to_owned(),
            ));
        }
        let path = entry.get("path").and_then(Value::as_str).ok_or_else(|| {
            SupervisorError::GenerationValidation(
                "generation inventory file entry omitted path".to_owned(),
            )
        })?;
        let hash = entry.get("blake3").and_then(Value::as_str).ok_or_else(|| {
            SupervisorError::GenerationValidation(
                "generation inventory file entry omitted blake3".to_owned(),
            )
        })?;
        admit_generation_inventory_file(&mut recorded_files, &mut recorded_path_bytes, path.len())?;
        if hash.len() != 64
            || !hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(SupervisorError::GenerationValidation(format!(
                "generation inventory file `{path}` has a non-canonical BLAKE3 digest"
            )));
        }
        let candidate = Path::new(path);
        if candidate.is_absolute()
            || candidate
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(SupervisorError::GenerationValidation(format!(
                "generation inventory contains unsafe path `{path}`"
            )));
        }
        if recorded
            .last()
            .is_some_and(|(previous_path, previous_hash)| {
                (previous_path.as_str(), previous_hash.as_str()) >= (path, hash)
                    || previous_path == path
            })
        {
            return Err(SupervisorError::GenerationValidation(
                "generation inventory file entries must be unique and sorted".to_owned(),
            ));
        }
        recorded.push((
            copy_generation_text(path, "record path")?,
            copy_generation_text(hash, "record digest")?,
        ));
    }
    if recorded.len() != recorded_entries.len() {
        return Err(SupervisorError::GenerationValidation(
            "generation inventory record count changed while parsing".to_owned(),
        ));
    }
    drop(value);
    let actual = generation_file_hashes(&generation_root, Some(&inventory_path))?;
    if recorded != actual {
        return Err(SupervisorError::GenerationValidation(format!(
            "selected generation `{id}` does not match its inventory"
        )));
    }
    for required in [
        "genesis/genesis.json",
        "genesis/genesis.signed.nrt",
        "genesis/genesis.expected_hash",
        "genesis/genesis.public_key",
    ] {
        if !recorded.iter().any(|(path, _)| path == required) {
            return Err(SupervisorError::GenerationValidation(format!(
                "selected generation `{id}` inventory omitted required artifact `{required}`"
            )));
        }
    }
    if !recorded.iter().any(|(path, _)| {
        let mut components = Path::new(path).components();
        matches!(
            components.next(),
            Some(std::path::Component::Normal(value)) if value == "peers"
        ) && matches!(components.next(), Some(std::path::Component::Normal(_)))
            && matches!(
                components.next(),
                Some(std::path::Component::Normal(value)) if value == "config.toml"
            )
            && components.next().is_none()
    }) {
        return Err(SupervisorError::GenerationValidation(format!(
            "selected generation `{id}` inventory contains no peer config"
        )));
    }
    let public_key_path = generation_root.join("genesis/genesis.public_key");
    let public_key_bytes = read_generation_file_bounded(
        &public_key_path,
        "generation public-key record",
        GENERATION_SMALL_RECORD_MAX_BYTES_V1,
    )?;
    if public_key_bytes != format!("{genesis_public_key}\n").as_bytes() {
        return Err(SupervisorError::GenerationValidation(format!(
            "selected generation `{id}` public-key record is not exact"
        )));
    }
    let expected_hash_path = generation_root.join("genesis/genesis.expected_hash");
    let expected_hash_bytes = read_generation_file_bounded(
        &expected_hash_path,
        "generation expected-hash record",
        GENERATION_SMALL_RECORD_MAX_BYTES_V1,
    )?;
    if expected_hash_bytes
        != format!("{}\n", NetworkId::from_genesis_hash(expected_hash)).as_bytes()
    {
        return Err(SupervisorError::GenerationValidation(format!(
            "selected generation `{id}` expected-hash record is not exact"
        )));
    }
    let manifest = iroha_genesis::RawGenesisTransaction::from_path(
        generation_root.join("genesis/genesis.json"),
    )?;
    if manifest.chain_id().as_str() != chain_id
        || manifest.chain_discriminant() != chain_discriminant
    {
        return Err(SupervisorError::GenerationValidation(format!(
            "selected generation `{id}` manifest differs from inventory chain metadata"
        )));
    }
    Ok(VerifiedGeneration {
        root: generation_root,
        generation_id: id.to_owned(),
        chain_id,
        chain_discriminant,
        genesis_public_key,
        expected_hash,
    })
}
fn ensure_inventory_context(
    verified: &VerifiedGeneration,
    expected: GenerationInventoryContext<'_>,
) -> Result<()> {
    if verified.chain_id != expected.chain_id
        || verified.chain_discriminant != expected.chain_discriminant
        || verified.genesis_public_key != *expected.genesis_public_key
        || verified.expected_hash != expected.expected_hash
    {
        return Err(SupervisorError::GenerationValidation(
            "generation inventory metadata differs from validated candidate".to_owned(),
        ));
    }
    Ok(())
}
fn validate_contained_directory(root: &Path, path: &Path, label: &str) -> Result<()> {
    reject_symlink(path, label)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Err(SupervisorError::GenerationValidation(format!(
            "{label} `{}` must be a directory",
            path.display()
        )));
    }
    let canonical = fs::canonicalize(path)?;
    if canonical.parent() != Some(root) {
        return Err(SupervisorError::GenerationValidation(format!(
            "{label} `{}` escapes the network root",
            path.display()
        )));
    }
    Ok(())
}
fn ensure_direct_child_directory(parent: &Path, path: &Path, label: &str) -> Result<()> {
    reject_symlink(path, label)?;
    match fs::create_dir(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    reject_symlink(path, label)?;
    if !fs::symlink_metadata(path)?.is_dir() {
        return Err(SupervisorError::GenerationValidation(format!(
            "{label} `{}` must be a directory",
            path.display()
        )));
    }
    let canonical_parent = fs::canonicalize(parent)?;
    let canonical = fs::canonicalize(path)?;
    if canonical.parent() != Some(canonical_parent.as_path()) {
        return Err(SupervisorError::GenerationValidation(format!(
            "{label} `{}` escapes its managed parent",
            path.display()
        )));
    }
    Ok(())
}
fn candidate_runtime_storage_is_safe(root: &Path, id: &str, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(root) else {
        return false;
    };
    let mut components = relative.components();
    if !matches!(
        components.next(),
        Some(std::path::Component::Normal(value)) if value == "peers"
    ) {
        return false;
    }
    let Some(std::path::Component::Normal(alias)) = components.next() else {
        return false;
    };
    if !matches!(
        components.next(),
        Some(std::path::Component::Normal(value)) if value == "storage-generations"
    ) || !matches!(
        components.next(),
        Some(std::path::Component::Normal(value)) if value == id
    ) || components.next().is_some()
    {
        return false;
    }
    let peers = root.join("peers");
    let peer = peers.join(alias);
    let storage_parent = peer.join("storage-generations");
    if [&peers, &peer, &storage_parent, path]
        .into_iter()
        .any(|candidate| reject_symlink(candidate, "candidate runtime storage").is_err())
    {
        return false;
    }
    fs::symlink_metadata(path)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        && fs::canonicalize(&storage_parent)
            .ok()
            .zip(fs::canonicalize(path).ok())
            .is_some_and(|(parent, candidate)| candidate.parent() == Some(parent.as_path()))
}
fn generation_file_metadata_unchanged(expected: &fs::Metadata, observed: &fs::Metadata) -> bool {
    if !expected.is_file() || !observed.is_file() || expected.len() != observed.len() {
        return false;
    }
    #[cfg(unix)]
    {
        expected.dev() == observed.dev()
            && expected.ino() == observed.ino()
            && expected.mtime() == observed.mtime()
            && expected.mtime_nsec() == observed.mtime_nsec()
            && expected.ctime() == observed.ctime()
            && expected.ctime_nsec() == observed.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        expected.modified().ok() == observed.modified().ok()
    }
}
fn admit_generation_tree_entry(entries: &mut usize) -> Result<()> {
    *entries = (*entries).checked_add(1).ok_or_else(|| {
        SupervisorError::GenerationValidation(
            "candidate generation entry count overflowed usize".to_owned(),
        )
    })?;
    if *entries > GENERATION_TREE_MAX_ENTRIES_V1 {
        return Err(SupervisorError::GenerationValidation(format!(
            "candidate generation exceeds the V1 {}-entry tree limit",
            GENERATION_TREE_MAX_ENTRIES_V1
        )));
    }
    Ok(())
}
fn admit_generation_inventory_file(
    files: &mut usize,
    aggregate_path_bytes: &mut usize,
    path_bytes: usize,
) -> Result<()> {
    if path_bytes > GENERATION_INVENTORY_MAX_PATH_BYTES_PER_FILE_V1 {
        return Err(SupervisorError::GenerationValidation(format!(
            "candidate generation path exceeds the V1 {}-byte limit",
            GENERATION_INVENTORY_MAX_PATH_BYTES_PER_FILE_V1
        )));
    }
    *files = (*files).checked_add(1).ok_or_else(|| {
        SupervisorError::GenerationValidation(
            "candidate generation file count overflowed usize".to_owned(),
        )
    })?;
    if *files > GENERATION_INVENTORY_MAX_FILES_V1 {
        return Err(SupervisorError::GenerationValidation(format!(
            "candidate generation exceeds the V1 {}-file inventory limit",
            GENERATION_INVENTORY_MAX_FILES_V1
        )));
    }
    *aggregate_path_bytes = (*aggregate_path_bytes)
        .checked_add(path_bytes)
        .ok_or_else(|| {
            SupervisorError::GenerationValidation(
                "candidate generation path-byte total overflowed usize".to_owned(),
            )
        })?;
    if *aggregate_path_bytes > GENERATION_INVENTORY_MAX_PATH_BYTES_V1 {
        return Err(SupervisorError::GenerationValidation(format!(
            "candidate generation exceeds the V1 {}-byte aggregate path limit",
            GENERATION_INVENTORY_MAX_PATH_BYTES_V1
        )));
    }
    Ok(())
}
fn copy_generation_text(value: &str, label: &'static str) -> Result<String> {
    let mut output = String::new();
    output.try_reserve_exact(value.len()).map_err(|_| {
        SupervisorError::GenerationValidation(format!(
            "candidate generation {label} allocation failed"
        ))
    })?;
    output.push_str(value);
    Ok(output)
}
fn read_generation_file_bounded(
    path: &Path,
    label: &'static str,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    let named = fs::symlink_metadata(path)?;
    if named.file_type().is_symlink() || !named.is_file() {
        return Err(SupervisorError::GenerationValidation(format!(
            "{label} `{}` is not a regular file",
            path.display()
        )));
    }
    if named.len() > u64::try_from(max_bytes).unwrap_or(u64::MAX) {
        return Err(SupervisorError::GenerationValidation(format!(
            "{label} `{}` exceeds its {max_bytes}-byte limit",
            path.display()
        )));
    }
    let expected_len = usize::try_from(named.len()).map_err(|_| {
        SupervisorError::GenerationValidation(format!(
            "{label} `{}` length does not fit usize",
            path.display()
        ))
    })?;
    let mut file = File::open(path)?;
    let opened = file.metadata()?;
    if !generation_file_metadata_unchanged(&named, &opened) {
        return Err(SupervisorError::GenerationValidation(format!(
            "{label} `{}` changed while it was opened",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(expected_len).map_err(|_| {
        SupervisorError::GenerationValidation(format!(
            "{label} `{}` allocation failed",
            path.display()
        ))
    })?;
    bytes.resize(expected_len, 0);
    file.read_exact(&mut bytes)?;
    let mut growth_probe = [0_u8; 1];
    if file.read(&mut growth_probe)? != 0 {
        return Err(SupervisorError::GenerationValidation(format!(
            "{label} `{}` grew while it was read",
            path.display()
        )));
    }
    let opened_after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    if named_after.file_type().is_symlink()
        || !generation_file_metadata_unchanged(&named, &opened_after)
        || !generation_file_metadata_unchanged(&named, &named_after)
    {
        return Err(SupervisorError::GenerationValidation(format!(
            "{label} `{}` changed while it was read",
            path.display()
        )));
    }
    Ok(bytes)
}
fn hash_generation_file(path: &Path, expected: &fs::Metadata) -> Result<String> {
    let mut file = File::open(path)?;
    let opened = file.metadata()?;
    if !generation_file_metadata_unchanged(expected, &opened) {
        return Err(SupervisorError::GenerationValidation(format!(
            "candidate generation file `{}` changed while it was opened",
            path.display()
        )));
    }
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; GENERATION_FILE_HASH_BUFFER_BYTES];
    let mut observed_bytes = 0_u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        observed_bytes = observed_bytes
            .checked_add(u64::try_from(read).map_err(|_| {
                SupervisorError::GenerationValidation(
                    "candidate generation read length does not fit u64".to_owned(),
                )
            })?)
            .ok_or_else(|| {
                SupervisorError::GenerationValidation(
                    "candidate generation file length overflowed u64".to_owned(),
                )
            })?;
        if observed_bytes > expected.len() {
            return Err(SupervisorError::GenerationValidation(format!(
                "candidate generation file `{}` grew while it was hashed",
                path.display()
            )));
        }
        hasher.update(&buffer[..read]);
    }
    let opened_after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    if named_after.file_type().is_symlink()
        || observed_bytes != expected.len()
        || !generation_file_metadata_unchanged(expected, &opened_after)
        || !generation_file_metadata_unchanged(expected, &named_after)
    {
        return Err(SupervisorError::GenerationValidation(format!(
            "candidate generation file `{}` changed while it was hashed",
            path.display()
        )));
    }
    let digest = hasher.finalize();
    let mut encoded = String::new();
    encoded.try_reserve_exact(64).map_err(|_| {
        SupervisorError::GenerationValidation(
            "candidate generation digest allocation failed".to_owned(),
        )
    })?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for &byte in digest.as_bytes() {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    Ok(encoded)
}
fn generation_file_hashes(root: &Path, excluded: Option<&Path>) -> Result<Vec<(String, String)>> {
    let mut output = Vec::new();
    let mut pending = Vec::new();
    pending.try_reserve_exact(1).map_err(|_| {
        SupervisorError::GenerationValidation(
            "candidate generation traversal allocation failed".to_owned(),
        )
    })?;
    pending.push((root.to_path_buf(), 0_usize));
    let mut tree_entries = 0_usize;
    let mut file_count = 0_usize;
    let mut aggregate_path_bytes = 0_usize;
    while let Some((directory, depth)) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            admit_generation_tree_entry(&mut tree_entries)?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate generation contains symbolic link `{}`",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                let child_depth = depth.checked_add(1).ok_or_else(|| {
                    SupervisorError::GenerationValidation(
                        "candidate generation directory depth overflowed usize".to_owned(),
                    )
                })?;
                if child_depth > GENERATION_TREE_MAX_DEPTH_V1 {
                    return Err(SupervisorError::GenerationValidation(format!(
                        "candidate generation exceeds the V1 {}-level directory-depth limit",
                        GENERATION_TREE_MAX_DEPTH_V1
                    )));
                }
                pending.try_reserve(1).map_err(|_| {
                    SupervisorError::GenerationValidation(
                        "candidate generation traversal allocation failed".to_owned(),
                    )
                })?;
                pending.push((path, child_depth));
            } else if metadata.is_file() && excluded != Some(path.as_path()) {
                let relative = path.strip_prefix(root).map_err(|error| {
                    SupervisorError::GenerationValidation(format!(
                        "candidate path escaped generation root: {error}"
                    ))
                })?;
                let relative = relative.to_str().ok_or_else(|| {
                    SupervisorError::GenerationValidation(
                        "candidate generation contains a non-UTF-8 path".to_owned(),
                    )
                })?;
                admit_generation_inventory_file(
                    &mut file_count,
                    &mut aggregate_path_bytes,
                    relative.len(),
                )?;
                let hash = hash_generation_file(&path, &metadata)?;
                let relative = copy_generation_text(relative, "path")?;
                output.try_reserve(1).map_err(|_| {
                    SupervisorError::GenerationValidation(
                        "candidate generation inventory allocation failed".to_owned(),
                    )
                })?;
                output.push((relative, hash));
            } else if !metadata.is_file() {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate generation contains non-regular entry `{}`",
                    path.display()
                )));
            }
        }
    }
    output.sort_unstable();
    Ok(output)
}
fn sync_tree(path: &Path) -> Result<()> {
    let mut pending = Vec::new();
    let mut directories = Vec::new();
    pending.try_reserve_exact(1).map_err(|_| {
        SupervisorError::GenerationValidation(
            "candidate publication traversal allocation failed".to_owned(),
        )
    })?;
    directories.try_reserve_exact(1).map_err(|_| {
        SupervisorError::GenerationValidation(
            "candidate publication directory allocation failed".to_owned(),
        )
    })?;
    pending.push((path.to_path_buf(), 0_usize));
    directories.push(path.to_path_buf());
    let mut tree_entries = 0_usize;
    while let Some((directory, depth)) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            admit_generation_tree_entry(&mut tree_entries)?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate publication tree contains symbolic link `{}`",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                let child_depth = depth.checked_add(1).ok_or_else(|| {
                    SupervisorError::GenerationValidation(
                        "candidate publication directory depth overflowed usize".to_owned(),
                    )
                })?;
                if child_depth > GENERATION_TREE_MAX_DEPTH_V1 {
                    return Err(SupervisorError::GenerationValidation(format!(
                        "candidate publication exceeds the V1 {}-level directory-depth limit",
                        GENERATION_TREE_MAX_DEPTH_V1
                    )));
                }
                pending.try_reserve(1).map_err(|_| {
                    SupervisorError::GenerationValidation(
                        "candidate publication traversal allocation failed".to_owned(),
                    )
                })?;
                directories.try_reserve(1).map_err(|_| {
                    SupervisorError::GenerationValidation(
                        "candidate publication directory allocation failed".to_owned(),
                    )
                })?;
                pending.push((path.clone(), child_depth));
                directories.push(path);
            } else if metadata.is_file() {
                File::open(&path)?.sync_all()?;
            } else {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate publication tree contains non-regular entry `{}`",
                    path.display()
                )));
            }
        }
    }
    for directory in directories.into_iter().rev() {
        sync_directory(&directory)?;
    }
    Ok(())
}
fn sync_directory(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}
fn reject_symlink(path: &Path, label: &str) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(SupervisorError::GenerationValidation(format!(
                "{label} `{}` must not be a symbolic link",
                path.display()
            )))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}
fn validate_lock_file(path: &Path, file: &File) -> Result<()> {
    let opened = file.metadata()?;
    let named = fs::symlink_metadata(path)?;
    if !opened.is_file() || !named.is_file() || named.file_type().is_symlink() {
        return Err(SupervisorError::GenerationValidation(format!(
            "generation lock `{}` must be a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        if opened.dev() != named.dev() || opened.ino() != named.ino() || opened.nlink() != 1 {
            return Err(SupervisorError::GenerationValidation(format!(
                "generation lock `{}` changed while it was opened",
                path.display()
            )));
        }
        if opened.permissions().mode() & 0o077 != 0 {
            return Err(SupervisorError::GenerationValidation(format!(
                "generation lock `{}` must be owner-only",
                path.display()
            )));
        }
    }
    Ok(())
}
fn is_generation_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn encode_lower_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, bls_normal_pop_prove};
    use iroha_data_model::{ChainId, peer::PeerId};
    use iroha_genesis::{GenesisTopologyEntry, RawGenesisTransaction};
    const FIXTURE_CONFIGURED_HASH: &str =
        "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E";
    const FIXTURE_GENESIS_PUBLIC_KEY: &str =
        "ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4";
    #[test]
    fn generation_file_hash_streams_across_multiple_chunks() {
        let temp = tempfile::tempdir().expect("temporary root");
        let path = temp.path().join("large-generation-artifact.bin");
        let bytes = (0..GENERATION_FILE_HASH_BUFFER_BYTES * 2 + 17)
            .map(|index| (index % 251) as u8)
            .collect::<Vec<_>>();
        fs::write(&path, &bytes).expect("write multi-chunk artifact");
        let metadata = fs::symlink_metadata(&path).expect("inspect multi-chunk artifact");
        let observed = hash_generation_file(&path, &metadata).expect("stream artifact hash");
        assert_eq!(observed, blake3::hash(&bytes).to_hex().to_string());
    }
    #[test]
    fn generation_tree_and_inventory_budgets_accept_exact_and_reject_next() {
        let mut entries = GENERATION_TREE_MAX_ENTRIES_V1 - 1;
        admit_generation_tree_entry(&mut entries).expect("exact tree-entry limit");
        assert!(admit_generation_tree_entry(&mut entries).is_err());
        let mut files = GENERATION_INVENTORY_MAX_FILES_V1 - 1;
        let mut path_bytes = GENERATION_INVENTORY_MAX_PATH_BYTES_V1
            - GENERATION_INVENTORY_MAX_PATH_BYTES_PER_FILE_V1;
        admit_generation_inventory_file(
            &mut files,
            &mut path_bytes,
            GENERATION_INVENTORY_MAX_PATH_BYTES_PER_FILE_V1,
        )
        .expect("exact generation inventory limits");
        assert!(
            admit_generation_inventory_file(&mut files, &mut path_bytes, 0).is_err(),
            "the first file beyond the V1 limit must fail closed"
        );
        let mut path_only_files = 0;
        let mut path_only_bytes = GENERATION_INVENTORY_MAX_PATH_BYTES_V1;
        assert!(
            admit_generation_inventory_file(&mut path_only_files, &mut path_only_bytes, 1).is_err(),
            "the first aggregate path byte beyond the V1 limit must fail closed"
        );
    }
    #[test]
    fn bounded_generation_reader_accepts_exact_and_rejects_max_plus_one() {
        let temp = tempfile::tempdir().expect("temporary root");
        let path = temp.path().join("generation.json");
        fs::write(&path, [0x5A_u8; 32]).expect("write exact generation record");
        assert_eq!(
            read_generation_file_bounded(&path, "test generation record", 32)
                .expect("read exact generation record"),
            [0x5A_u8; 32]
        );
        fs::write(&path, [0x5A_u8; 33]).expect("write oversized generation record");
        let error = read_generation_file_bounded(&path, "test generation record", 32)
            .expect_err("max plus one must reject before allocation");
        assert!(error.to_string().contains("exceeds its 32-byte limit"));
    }
    #[test]
    fn generation_file_inventory_is_sorted_after_streaming_walk() {
        let temp = tempfile::tempdir().expect("temporary root");
        let nested = temp.path().join("nested");
        fs::create_dir(&nested).expect("create nested generation directory");
        fs::write(temp.path().join("z.bin"), b"zeta").expect("write root artifact");
        fs::write(nested.join("a.bin"), b"alpha").expect("write nested artifact");
        let observed =
            generation_file_hashes(temp.path(), None).expect("stream generation inventory files");
        assert_eq!(
            observed
                .iter()
                .map(|(path, _)| path.as_str())
                .collect::<Vec<_>>(),
            vec!["nested/a.bin", "z.bin"]
        );
    }
    fn write_genesis_execution_config(
        peer_dir: &Path,
        chain_id: &ChainId,
        chain_discriminant: u16,
        genesis_public_key: &PublicKey,
        bootstrap_expected_hash: Option<&str>,
    ) -> PathBuf {
        let managed_directory = peer_dir.join("managed");
        fs::create_dir_all(&managed_directory).expect("create managed fixture directory");
        let rans_tables_path = managed_directory.join("rans_tables.toml");
        fs::write(
            &rans_tables_path,
            include_bytes!("../../../codec/rans/tables/rans_seed0.toml"),
        )
        .expect("write signed rANS tables fixture");
        let rans_tables_literal = rans_tables_path.to_string_lossy().replace('\\', "\\\\");
        let mut config =
            include_str!("../../../crates/iroha_config/iroha_test_config.toml").to_owned();
        config = config.replacen(
            "chain = \"00000000-0000-0000-0000-000000000000\"",
            &format!("chain = \"{chain_id}\"\nchain_discriminant = {chain_discriminant}"),
            1,
        );
        config = config.replacen(
            &format!("[genesis]\npublic_key = \"{FIXTURE_GENESIS_PUBLIC_KEY}\""),
            &format!("[genesis]\npublic_key = \"{genesis_public_key}\""),
            1,
        );
        config = config.replacen(
            "file = \"./genesis.signed.nrt\"",
            "file = \"../../genesis/genesis.signed.nrt\"\nmanifest_json = \"../../genesis/genesis.json\"",
            1,
        );
        config = match bootstrap_expected_hash {
            Some(expected_hash) => config.replacen(FIXTURE_CONFIGURED_HASH, expected_hash, 1),
            None => config.replacen(
                &format!("expected_hash = \"{FIXTURE_CONFIGURED_HASH}\""),
                "expected_hash_file = \"../../genesis/genesis.expected_hash\"",
                1,
            ),
        };
        config.push_str(
            r#"

[kura]
store_dir = "managed/kura"

[snapshot]
store_dir = "managed/snapshot"

[torii.da_ingest]
replay_cache_store_dir = "managed/torii/da-replay"
manifest_store_dir = "managed/torii/da-manifests"

[sorafs.storage]
data_dir = "managed/sorafs"

[streaming.codec]
rans_tables_path = "__RANS_TABLES_PATH__"

[network.soranet_handshake.pow]
revocation_store_path = "managed/soranet/revocations.norito"
"#,
        );
        config = config.replacen("__RANS_TABLES_PATH__", &rans_tables_literal, 1);
        config = config.replacen(
            "session_store_dir = \"./storage/streaming\"",
            "session_store_dir = \"managed/streaming\"",
            1,
        );
        config = config.replacen(
            "[torii]\naddress = \"addr:127.0.0.1:8080#8942\"",
            "[torii]\naddress = \"addr:127.0.0.1:8080#8942\"\ndata_dir = \"managed/torii\"",
            1,
        );
        let config_path = peer_dir.join("config.toml");
        fs::write(&config_path, config).expect("write executable genesis fixture config");
        config_path
    }
    fn write_complete_candidate(
        root: &Path,
        chain_id: &str,
        chain_discriminant: u16,
    ) -> (KeyPair, HashOf<BlockHeader>) {
        let genesis_dir = root.join("genesis");
        let peer_dir = root.join("peers/peer0");
        fs::create_dir_all(&genesis_dir).expect("create fixture genesis directory");
        fs::create_dir_all(&peer_dir).expect("create fixture peer directory");
        let key_pair = KeyPair::random();
        let chain_id: ChainId = chain_id.parse().expect("fixture chain id is canonical");
        let topology = (0..4)
            .map(|_| {
                let validator = KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
                    .expect("generate fixture validator key");
                let pop = bls_normal_pop_prove(validator.private_key())
                    .expect("generate fixture validator proof of possession");
                GenesisTopologyEntry::new(PeerId::new(validator.public_key().clone()), pop)
            })
            .collect::<Vec<_>>();
        let manifest = iroha_genesis::GenesisBuilder::new_without_executor(chain_id.clone(), ".")
            .set_topology(topology)
            .build_raw()
            .with_chain_discriminant(chain_discriminant)
            .with_consensus_meta();
        let manifest_path = genesis_dir.join("genesis.json");
        fs::write(
            &manifest_path,
            json::to_vec_pretty(&manifest).expect("encode fixture manifest"),
        )
        .expect("write fixture manifest");
        let config_path = write_genesis_execution_config(
            &peer_dir,
            &chain_id,
            chain_discriminant,
            key_pair.public_key(),
            Some(izanami::genesis_support::UNRESOLVED_GENESIS_EXPECTED_HASH),
        );
        let block = crate::supervisor::sign_kagami_stub_genesis_from_config(
            &manifest_path,
            &config_path,
            &key_pair,
            Some(iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned),
        )
        .expect("execute and sign generation fixture against its node config");
        assert!(block.has_results());
        assert_eq!(block.results().len(), block.entrypoint_hashes().len());
        assert!(block.results().all(|result| result.as_ref().is_ok()));
        assert_eq!(
            block.committed_fragment_count(),
            Some(u64::try_from(block.results().len()).expect("fixture result count fits u64"))
        );
        block
            .validate_entrypoint_merkle_cache()
            .expect("generation fixture entrypoint Merkle cache");
        block
            .validate_result_merkle_cache()
            .expect("generation fixture result Merkle cache");
        assert_eq!(
            block.header().result_merkle_root(),
            block
                .result_merkle_commitment()
                .map(|commitment| *commitment.root())
        );
        let mut signatures = block.signatures();
        let signature = signatures
            .next()
            .expect("result-bearing generation fixture signature");
        assert_eq!(signature.index(), 0);
        assert!(signatures.next().is_none());
        signature
            .signature()
            .verify_hash(key_pair.public_key(), block.hash())
            .expect("verify result-bearing generation fixture signature");
        let expected_hash = block.hash();
        let network_id = NetworkId::from_genesis_hash(expected_hash);
        fs::write(
            genesis_dir.join("genesis.expected_hash"),
            format!("{network_id}\n"),
        )
        .expect("write fixture hash");
        let finalized_config_path = write_genesis_execution_config(
            &peer_dir,
            &chain_id,
            chain_discriminant,
            key_pair.public_key(),
            None,
        );
        assert_eq!(finalized_config_path, config_path);
        let finalized_config = fs::read_to_string(&finalized_config_path)
            .expect("read finalized generation fixture config")
            .parse::<toml::Table>()
            .expect("parse finalized generation fixture config");
        let finalized_genesis = finalized_config
            .get("genesis")
            .and_then(toml::Value::as_table)
            .expect("finalized generation fixture genesis table");
        assert_eq!(
            finalized_genesis
                .get("expected_hash_file")
                .and_then(toml::Value::as_str),
            Some("../../genesis/genesis.expected_hash")
        );
        assert!(!finalized_genesis.contains_key("expected_hash"));
        let wire = block.encode_wire().expect("encode fixture block");
        izanami::genesis_support::validate_prepared_genesis_for_startup(
            &wire,
            &RawGenesisTransaction::from_path(&manifest_path).expect("read fixture manifest"),
            key_pair.public_key(),
            expected_hash,
            &chain_id,
        )
        .expect("generation fixture must satisfy exact startup genesis validation");
        fs::write(genesis_dir.join("genesis.signed.nrt"), wire).expect("write fixture block");
        fs::write(
            genesis_dir.join("genesis.public_key"),
            format!("{}\n", key_pair.public_key()),
        )
        .expect("write fixture key");
        (key_pair, expected_hash)
    }
    fn publish_complete_generation(root: &Path, chain_id: &str) -> String {
        let transaction = GenerationTransaction::begin(root).expect("begin fixture generation");
        let (key_pair, expected_hash) = write_complete_candidate(transaction.root(), chain_id, 7);
        let publication = transaction
            .publish(GenerationInventoryContext {
                chain_id,
                chain_discriminant: 7,
                genesis_public_key: key_pair.public_key(),
                expected_hash,
            })
            .expect("publish fixture generation");
        durable_generation_id(publication)
    }
    fn durable_generation_id(mut publication: PublishedGeneration) -> String {
        let id = publication.id().to_owned();
        assert!(
            publication.take_uncertainty().is_none(),
            "fixture publication must be durable"
        );
        drop(publication);
        id
    }
    fn write_crash_marker(root: &Path, id: &str, bytes: &[u8]) -> PathBuf {
        let marker = generation_pointer_temporary_path(root, id);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&marker).expect("create crash marker");
        file.write_all(bytes).expect("write crash marker");
        file.sync_all().expect("sync crash marker");
        sync_directory(root).expect("sync crash marker parent");
        marker
    }
    fn create_runtime_storage_fixture(root: &Path, alias: &str, id: &str) -> PathBuf {
        let storage = root
            .join("peers")
            .join(alias)
            .join("storage-generations")
            .join(id);
        fs::create_dir_all(&storage).expect("create runtime storage fixture");
        storage
    }
    #[test]
    fn lock_contention_fails_fast_and_releases_on_drop() {
        let temp = tempfile::tempdir().expect("temporary root");
        let first = GenerationTransaction::begin(temp.path()).expect("acquire first lock");
        let error = GenerationTransaction::begin(temp.path()).expect_err("second lock must fail");
        assert!(matches!(error, SupervisorError::GenerationLocked { .. }));
        drop(first);
        GenerationTransaction::begin(temp.path()).expect("lock released after drop");
    }
    #[test]
    fn exclusive_begin_reclaims_marked_crash_residue_and_preserves_history() {
        let temp = tempfile::tempdir().expect("temporary root");
        let historical = publish_complete_generation(temp.path(), "recovery-history");
        let selected = publish_complete_generation(temp.path(), "recovery-selected");
        let root = fs::canonicalize(temp.path()).expect("canonical temporary root");
        let historical_root = root.join(GENERATIONS_DIRECTORY).join(&historical);
        let historical_storage = create_runtime_storage_fixture(&root, "peer0", &historical);
        fs::write(historical_storage.join("sentinel"), b"retained history")
            .expect("write historical storage sentinel");
        let mut abandoned = GenerationTransaction::begin_replacing(&root, Some(selected.clone()))
            .expect("begin candidate that will simulate a crash");
        let abandoned_root = abandoned.root().to_path_buf();
        let abandoned_storage0 = abandoned
            .create_runtime_storage("peer0")
            .expect("create first abandoned runtime storage");
        let abandoned_storage1 = abandoned
            .create_runtime_storage("peer1")
            .expect("create second abandoned runtime storage");
        fs::write(abandoned_storage0.join("state"), b"candidate")
            .expect("write abandoned runtime state");
        let (key_pair, expected_hash) =
            write_complete_candidate(&abandoned_root, "recovery-abandoned", 23);
        abandoned
            .write_inventory(&GenerationInventoryContext {
                chain_id: "recovery-abandoned",
                chain_discriminant: 23,
                genesis_public_key: key_pair.public_key(),
                expected_hash,
            })
            .expect("seal abandoned generation inventory");
        sync_tree(&abandoned_root).expect("sync abandoned generation tree");
        sync_directory(abandoned_root.parent().expect("generations parent"))
            .expect("sync abandoned generation parent");
        abandoned
            .sync_runtime_storage_roots()
            .expect("sync abandoned runtime storage");
        sync_directory(&root).expect("sync abandoned runtime parents");
        let marker = abandoned.pointer_temporary.clone();
        assert_eq!(fs::read(&marker).expect("read early crash marker"), b"");
        // Simulate process exit without running transaction cleanup while still
        // allowing this test process to release the advisory lock.
        abandoned.committed = true;
        drop(abandoned);
        let next = GenerationTransaction::begin_replacing(&root, Some(selected.clone()))
            .expect("recover residue and begin next generation");
        assert!(!marker.exists());
        assert!(!abandoned_root.exists());
        assert!(!abandoned_storage0.exists());
        assert!(!abandoned_storage1.exists());
        assert!(historical_root.is_dir());
        assert_eq!(
            fs::read(historical_storage.join("sentinel")).expect("read retained historical state"),
            b"retained history"
        );
        assert_eq!(
            current_generation_id(&root).expect("read preserved selection"),
            Some(selected)
        );
        drop(next);
    }
    #[test]
    fn recovery_never_infers_abandonment_from_an_unmarked_directory_name() {
        let temp = tempfile::tempdir().expect("temporary root");
        let selected = publish_complete_generation(temp.path(), "unmarked-selected");
        let root = fs::canonicalize(temp.path()).expect("canonical temporary root");
        let unmarked_id = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
        let unmarked_root = root.join(GENERATIONS_DIRECTORY).join(unmarked_id);
        fs::create_dir(&unmarked_root).expect("create unmarked generation");
        fs::write(unmarked_root.join("damaged-history"), b"retain")
            .expect("write unmarked history");
        let unmarked_storage = create_runtime_storage_fixture(&root, "peer0", unmarked_id);
        fs::write(unmarked_storage.join("sentinel"), b"retain").expect("write unmarked storage");
        let next = GenerationTransaction::begin_replacing(&root, Some(selected))
            .expect("begin without collecting unmarked history");
        assert!(unmarked_root.is_dir());
        assert_eq!(
            fs::read(unmarked_storage.join("sentinel")).expect("read unmarked storage"),
            b"retain"
        );
        drop(next);
    }
    #[test]
    fn selected_generation_wins_over_a_recovered_marker() {
        let temp = tempfile::tempdir().expect("temporary root");
        let selected = publish_complete_generation(temp.path(), "marked-selected");
        let root = fs::canonicalize(temp.path()).expect("canonical temporary root");
        let selected_root = root.join(GENERATIONS_DIRECTORY).join(&selected);
        let selected_storage = create_runtime_storage_fixture(&root, "peer0", &selected);
        fs::write(selected_storage.join("sentinel"), b"selected")
            .expect("write selected storage sentinel");
        let marker = write_crash_marker(&root, &selected, format!("{selected}\n").as_bytes());
        let next = GenerationTransaction::begin_replacing(&root, Some(selected.clone()))
            .expect("discard redundant selected marker");
        assert!(!marker.exists());
        assert!(selected_root.is_dir());
        assert_eq!(
            fs::read(selected_storage.join("sentinel")).expect("read selected storage"),
            b"selected"
        );
        assert_eq!(
            current_generation_id(&root).expect("read selected pointer"),
            Some(selected)
        );
        drop(next);
    }
    #[test]
    fn recovery_runs_only_after_the_exclusive_lock_is_acquired() {
        let temp = tempfile::tempdir().expect("temporary root");
        let selected = publish_complete_generation(temp.path(), "recovery-lock");
        let root = fs::canonicalize(temp.path()).expect("canonical temporary root");
        let abandoned_id = "cccccccccccccccccccccccccccccccc";
        let abandoned_root = root.join(GENERATIONS_DIRECTORY).join(abandoned_id);
        fs::create_dir(&abandoned_root).expect("create abandoned generation");
        let marker = write_crash_marker(&root, abandoned_id, b"");
        let lease = try_lock_generation_selection(&root).expect("acquire shared selection lease");
        let error = GenerationTransaction::begin_replacing(&root, Some(selected.clone()))
            .expect_err("shared lease must prevent recovery writer");
        assert!(matches!(error, SupervisorError::GenerationLocked { .. }));
        assert!(marker.is_file());
        assert!(abandoned_root.is_dir());
        drop(lease);
        let next = GenerationTransaction::begin_replacing(&root, Some(selected))
            .expect("exclusive writer recovers after lease release");
        assert!(!marker.exists());
        assert!(!abandoned_root.exists());
        drop(next);
    }
    #[test]
    fn recovery_fails_closed_on_a_malformed_reserved_marker_name() {
        let temp = tempfile::tempdir().expect("temporary root");
        let selected = publish_complete_generation(temp.path(), "malformed-marker");
        let root = fs::canonicalize(temp.path()).expect("canonical temporary root");
        let malformed = root.join(".current-generation.not-a-generation.tmp");
        fs::write(&malformed, b"").expect("write malformed reserved marker");
        let error = GenerationTransaction::begin_replacing(&root, Some(selected.clone()))
            .expect_err("malformed reserved marker must fail closed");
        assert!(error.to_string().contains("invalid generation id"));
        assert!(malformed.is_file());
        assert_eq!(
            current_generation_id(&root).expect("read preserved selection"),
            Some(selected)
        );
    }
    #[test]
    fn failed_candidate_drop_preserves_current_pointer() {
        let temp = tempfile::tempdir().expect("temporary root");
        let old = publish_complete_generation(temp.path(), "drop-preservation");
        let mut candidate = GenerationTransaction::begin(temp.path()).expect("begin candidate");
        let candidate_root = candidate.root().to_path_buf();
        let runtime_storage = candidate
            .create_runtime_storage("peer0")
            .expect("create candidate runtime storage");
        fs::write(runtime_storage.join("candidate-state"), b"unpublished")
            .expect("write candidate runtime state");
        drop(candidate);
        assert!(!candidate_root.exists());
        assert!(
            !runtime_storage.exists(),
            "dropping an unpublished transaction must remove its runtime storage"
        );
        assert_eq!(
            current_generation_id(temp.path()).expect("read current id"),
            Some(old)
        );
    }
    #[test]
    fn every_precommit_fault_preserves_prior_selection() {
        for point in [
            PublicationFaultPoint::BeforeInventory,
            PublicationFaultPoint::AfterInventory,
            PublicationFaultPoint::AfterTreeSync,
            PublicationFaultPoint::AfterGenerationsSync,
            PublicationFaultPoint::AfterRuntimeStorageSync,
            PublicationFaultPoint::AfterPointerWrite,
            PublicationFaultPoint::AfterPointerSync,
        ] {
            let temp = tempfile::tempdir().expect("temporary root");
            let old = publish_complete_generation(temp.path(), "precommit-old");
            let old_root = verify_selected_generation(temp.path(), &old)
                .expect("prior generation validates")
                .root;
            let old_config =
                fs::read(old_root.join("peers/peer0/config.toml")).expect("read prior config");
            let storage = temp.path().join("peers/peer0/storage/state");
            fs::create_dir_all(storage.parent().expect("storage parent"))
                .expect("create stable storage");
            fs::write(&storage, b"prior usable state").expect("write stable storage");
            let mut transaction =
                GenerationTransaction::begin(temp.path()).expect("begin candidate");
            let candidate_root = transaction.root().to_path_buf();
            let candidate_storage = transaction
                .create_runtime_storage("peer0")
                .expect("create candidate runtime storage");
            fs::write(candidate_storage.join("candidate-state"), b"unpublished")
                .expect("write candidate runtime state");
            let (key_pair, expected_hash) =
                write_complete_candidate(&candidate_root, "precommit-new", 8);
            let error = transaction
                .publish_with_fault(
                    GenerationInventoryContext {
                        chain_id: "precommit-new",
                        chain_discriminant: 8,
                        genesis_public_key: key_pair.public_key(),
                        expected_hash,
                    },
                    point,
                )
                .expect_err("injected precommit boundary must fail");
            assert!(matches!(error, SupervisorError::GenerationValidation(_)));
            assert_eq!(
                current_generation_id(temp.path()).expect("read prior pointer"),
                Some(old.clone()),
                "fault at {point:?} must preserve old pointer"
            );
            assert!(
                !candidate_root.exists(),
                "fault at {point:?} must clean unpublished candidate"
            );
            assert!(
                !candidate_storage.exists(),
                "fault at {point:?} must clean unpublished runtime storage"
            );
            let verified = verify_selected_generation(temp.path(), &old)
                .expect("prior generation remains valid");
            assert_eq!(
                fs::read(verified.root.join("peers/peer0/config.toml"))
                    .expect("read preserved config"),
                old_config,
                "fault at {point:?} must preserve prior config"
            );
            assert_eq!(
                fs::read(&storage).expect("read preserved storage"),
                b"prior usable state",
                "fault at {point:?} must preserve stable storage"
            );
        }
    }
    #[test]
    fn post_rename_fault_reports_uncertain_committed_generation() {
        let temp = tempfile::tempdir().expect("temporary root");
        let old = publish_complete_generation(temp.path(), "postcommit-old");
        let transaction = GenerationTransaction::begin(temp.path()).expect("begin candidate");
        let candidate_id = transaction.id().to_owned();
        let (key_pair, expected_hash) =
            write_complete_candidate(transaction.root(), "postcommit-new", 10);
        let mut publication = transaction
            .publish_with_fault(
                GenerationInventoryContext {
                    chain_id: "postcommit-new",
                    chain_discriminant: 10,
                    genesis_public_key: key_pair.public_key(),
                    expected_hash,
                },
                PublicationFaultPoint::AfterPointerRename,
            )
            .expect("post-rename fault occurs after the pointer commit");
        let error = publication
            .take_uncertainty()
            .expect("post-rename fault must report uncertain durability");
        assert!(matches!(
            error,
            SupervisorError::PublicationUncertain { ref generation_id, .. }
                if generation_id == &candidate_id
        ));
        let contention = GenerationTransaction::begin(temp.path())
            .expect_err("uncertain committed guard must retain the generation lock");
        assert!(matches!(
            contention,
            SupervisorError::GenerationLocked { .. }
        ));
        assert_ne!(candidate_id, old);
        assert_eq!(
            current_generation_id(temp.path()).expect("read committed pointer"),
            Some(candidate_id.clone())
        );
        verify_selected_generation(temp.path(), &candidate_id)
            .expect("committed candidate must survive transaction drop");
        drop(publication);
        GenerationTransaction::begin(temp.path())
            .expect("dropping the committed guard releases the generation lock");
    }
    #[test]
    fn committed_publication_guard_serializes_reconciliation() {
        let temp = tempfile::tempdir().expect("temporary root");
        let transaction = GenerationTransaction::begin(temp.path()).expect("begin candidate");
        let candidate_id = transaction.id().to_owned();
        let (key_pair, expected_hash) =
            write_complete_candidate(transaction.root(), "guarded-commit", 15);
        let mut publication = transaction
            .publish(GenerationInventoryContext {
                chain_id: "guarded-commit",
                chain_discriminant: 15,
                genesis_public_key: key_pair.public_key(),
                expected_hash,
            })
            .expect("publish guarded generation");
        assert_eq!(publication.id(), candidate_id);
        assert!(publication.take_uncertainty().is_none());
        let contention = GenerationTransaction::begin(temp.path())
            .expect_err("committed guard must retain the generation lock");
        assert!(matches!(
            contention,
            SupervisorError::GenerationLocked { .. }
        ));
        drop(publication);
        GenerationTransaction::begin(temp.path())
            .expect("dropping the committed guard releases the generation lock");
    }
    #[test]
    fn stale_expected_base_cannot_replace_newer_selection() {
        let temp = tempfile::tempdir().expect("temporary root");
        let old = publish_complete_generation(temp.path(), "cas-old");
        let next = publish_complete_generation(temp.path(), "cas-current");
        let selected =
            verify_selected_generation(temp.path(), &next).expect("newer selection validates");
        let selected_config =
            fs::read(selected.root.join("peers/peer0/config.toml")).expect("read selected config");
        let stale = GenerationTransaction::begin_replacing(temp.path(), Some(old.clone()))
            .expect("begin stale candidate");
        let stale_root = stale.root().to_path_buf();
        let (key_pair, expected_hash) = write_complete_candidate(&stale_root, "cas-stale", 16);
        let error = stale
            .publish(GenerationInventoryContext {
                chain_id: "cas-stale",
                chain_discriminant: 16,
                genesis_public_key: key_pair.public_key(),
                expected_hash,
            })
            .expect_err("stale expected base must fail before pointer replacement");
        assert!(matches!(
            error,
            SupervisorError::GenerationSelectionChanged {
                expected: Some(ref expected),
                actual: Some(ref actual),
            } if expected == &old && actual == &next
        ));
        assert_eq!(
            current_generation_id(temp.path()).expect("read preserved selection"),
            Some(next.clone())
        );
        let preserved =
            verify_selected_generation(temp.path(), &next).expect("newer selection remains valid");
        assert_eq!(
            fs::read(preserved.root.join("peers/peer0/config.toml"))
                .expect("read preserved selected config"),
            selected_config
        );
        assert!(!stale_root.exists());
    }
    #[test]
    fn publication_keeps_synced_runtime_storage_outside_immutable_generation() {
        let temp = tempfile::tempdir().expect("temporary root");
        let mut transaction = GenerationTransaction::begin(temp.path()).expect("begin candidate");
        let generation_root = transaction.root().to_path_buf();
        let generation_id = transaction.id().to_owned();
        let storage = transaction
            .create_runtime_storage("peer0")
            .expect("create runtime storage");
        fs::write(storage.join("candidate-state"), b"durable candidate state")
            .expect("write candidate state");
        let (key_pair, expected_hash) =
            write_complete_candidate(&generation_root, "runtime-storage-sync", 11);
        let published = durable_generation_id(
            transaction
                .publish(GenerationInventoryContext {
                    chain_id: "runtime-storage-sync",
                    chain_discriminant: 11,
                    genesis_public_key: key_pair.public_key(),
                    expected_hash,
                })
                .expect("publish generation with runtime storage"),
        );
        assert_eq!(published, generation_id);
        assert_eq!(
            storage,
            temp.path()
                .canonicalize()
                .expect("canonical network root")
                .join("peers/peer0/storage-generations")
                .join(&generation_id)
        );
        assert!(!storage.starts_with(&generation_root));
        assert_eq!(
            fs::read(storage.join("candidate-state")).expect("read retained candidate state"),
            b"durable candidate state"
        );
        assert_eq!(
            current_generation_id(temp.path()).expect("read selected generation"),
            Some(generation_id)
        );
    }
    #[cfg(unix)]
    #[test]
    fn publication_rejects_replaced_runtime_storage_before_pointer_commit() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().expect("temporary root");
        let outside = tempfile::tempdir().expect("outside storage root");
        let mut transaction = GenerationTransaction::begin(temp.path()).expect("begin candidate");
        let generation_root = transaction.root().to_path_buf();
        let storage = transaction
            .create_runtime_storage("peer0")
            .expect("create runtime storage");
        fs::remove_dir(&storage).expect("remove empty managed storage");
        symlink(outside.path(), &storage).expect("replace storage with symlink");
        let (key_pair, expected_hash) =
            write_complete_candidate(&generation_root, "runtime-storage-symlink", 12);
        let error = transaction
            .publish(GenerationInventoryContext {
                chain_id: "runtime-storage-symlink",
                chain_discriminant: 12,
                genesis_public_key: key_pair.public_key(),
                expected_hash,
            })
            .expect_err("replaced runtime storage must fail before pointer commit");
        assert!(error.to_string().contains("candidate runtime storage"));
        assert_eq!(
            current_generation_id(temp.path()).expect("read absent pointer"),
            None
        );
        assert!(
            fs::read_dir(outside.path())
                .expect("read outside storage")
                .next()
                .is_none(),
            "publication must not traverse a replaced runtime-storage symlink"
        );
    }
    #[cfg(unix)]
    #[test]
    fn publication_rejects_interior_runtime_storage_symlink_before_pointer_commit() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().expect("temporary root");
        let old = publish_complete_generation(temp.path(), "interior-symlink-old");
        let outside = tempfile::tempdir().expect("outside snapshot root");
        let outside_sentinel = outside.path().join("sentinel");
        fs::write(&outside_sentinel, b"outside-state").expect("write outside sentinel");
        let mut transaction = GenerationTransaction::begin(temp.path()).expect("begin candidate");
        let generation_root = transaction.root().to_path_buf();
        let candidate_storage = transaction
            .create_runtime_storage("peer0")
            .expect("create runtime storage");
        symlink(outside.path(), candidate_storage.join("snapshot"))
            .expect("create interior storage symlink");
        let (key_pair, expected_hash) =
            write_complete_candidate(&generation_root, "interior-symlink-new", 13);
        let error = transaction
            .publish(GenerationInventoryContext {
                chain_id: "interior-symlink-new",
                chain_discriminant: 13,
                genesis_public_key: key_pair.public_key(),
                expected_hash,
            })
            .expect_err("interior runtime-storage symlink must fail before pointer commit");
        assert!(matches!(&error, SupervisorError::GenerationValidation(_)));
        assert!(error.to_string().contains("symbolic link"));
        assert_eq!(
            current_generation_id(temp.path()).expect("read preserved pointer"),
            Some(old)
        );
        assert_eq!(
            fs::read(&outside_sentinel).expect("read outside sentinel"),
            b"outside-state",
            "publication must not mutate the interior symlink target"
        );
        assert_eq!(
            fs::read_dir(outside.path())
                .expect("read outside snapshot root")
                .count(),
            1,
            "publication must not create entries through the interior symlink"
        );
        assert!(!generation_root.exists());
        assert!(!candidate_storage.exists());
    }
    #[cfg(unix)]
    #[test]
    fn publication_rejects_special_runtime_storage_entry_before_pointer_commit() {
        use std::os::unix::net::UnixListener;
        let temp = tempfile::tempdir_in("/tmp").expect("short temporary root");
        let old = publish_complete_generation(temp.path(), "special-entry-old");
        let mut transaction = GenerationTransaction::begin(temp.path()).expect("begin candidate");
        let generation_root = transaction.root().to_path_buf();
        let candidate_storage = transaction
            .create_runtime_storage("peer0")
            .expect("create runtime storage");
        let socket = UnixListener::bind(candidate_storage.join("runtime.sock"))
            .expect("create special runtime-storage entry");
        let (key_pair, expected_hash) =
            write_complete_candidate(&generation_root, "special-entry-new", 14);
        let error = transaction
            .publish(GenerationInventoryContext {
                chain_id: "special-entry-new",
                chain_discriminant: 14,
                genesis_public_key: key_pair.public_key(),
                expected_hash,
            })
            .expect_err("special runtime-storage entry must fail before pointer commit");
        assert!(matches!(&error, SupervisorError::GenerationValidation(_)));
        assert!(error.to_string().contains("non-regular entry"));
        assert_eq!(
            current_generation_id(temp.path()).expect("read preserved pointer"),
            Some(old)
        );
        drop(socket);
        assert!(!generation_root.exists());
        assert!(!candidate_storage.exists());
    }
    #[test]
    fn selected_generation_verification_rejects_tampering() {
        let temp = tempfile::tempdir().expect("temporary root");
        let transaction = GenerationTransaction::begin(temp.path()).expect("begin candidate");
        let candidate_root = transaction.root().to_path_buf();
        let (key_pair, expected_hash) =
            write_complete_candidate(&candidate_root, "selection-test", 9);
        let id = durable_generation_id(
            transaction
                .publish(GenerationInventoryContext {
                    chain_id: "selection-test",
                    chain_discriminant: 9,
                    genesis_public_key: key_pair.public_key(),
                    expected_hash,
                })
                .expect("publish generation"),
        );
        verify_selected_generation(temp.path(), &id).expect("sealed generation verifies");
        fs::write(
            candidate_root.join("genesis/genesis.signed.nrt"),
            b"tampered",
        )
        .expect("tamper artifact");
        let error = verify_selected_generation(temp.path(), &id)
            .expect_err("tampered selected generation must fail");
        assert!(error.to_string().contains("does not match its inventory"));
    }
    #[test]
    fn selected_generation_rejects_inventory_metadata_substitution() {
        let temp = tempfile::tempdir().expect("temporary root");
        let id = publish_complete_generation(temp.path(), "inventory-binding");
        let inventory = temp
            .path()
            .join(GENERATIONS_DIRECTORY)
            .join(&id)
            .join(GENERATION_INVENTORY_FILE);
        let mut value: Value = json::from_slice(&fs::read(&inventory).expect("read inventory"))
            .expect("parse inventory");
        value.as_object_mut().expect("inventory object").insert(
            "chain_id".to_owned(),
            Value::String("substituted-chain".to_owned()),
        );
        let mut bytes = json::to_json_bounded(&value, GENERATION_INVENTORY_MAX_BYTES_V1 - 1)
            .expect("encode substituted inventory")
            .into_bytes();
        bytes.push(b'\n');
        fs::write(&inventory, bytes).expect("write substituted inventory");
        let error = verify_selected_generation(temp.path(), &id)
            .expect_err("inventory metadata substitution must fail");
        assert!(
            error
                .to_string()
                .contains("manifest differs from inventory")
        );
    }
    #[test]
    fn selected_generation_rejects_open_or_noncanonical_file_entries() {
        for extra_field in [true, false] {
            let temp = tempfile::tempdir().expect("temporary root");
            let id = publish_complete_generation(temp.path(), "inventory-file-entry");
            let inventory = temp
                .path()
                .join(GENERATIONS_DIRECTORY)
                .join(&id)
                .join(GENERATION_INVENTORY_FILE);
            let mut value: Value = json::from_slice(&fs::read(&inventory).expect("read inventory"))
                .expect("parse inventory");
            let entry = value
                .as_object_mut()
                .and_then(|object| object.get_mut("files"))
                .and_then(Value::as_array_mut)
                .and_then(|files| files.first_mut())
                .and_then(Value::as_object_mut)
                .expect("first inventory file entry");
            let expected_message = if extra_field {
                entry.insert("size".to_owned(), Value::Number(1_u64.into()));
                "exactly `path` and `blake3`"
            } else {
                entry.insert("blake3".to_owned(), Value::String("A".repeat(64)));
                "non-canonical BLAKE3 digest"
            };
            let mut bytes = json::to_json_bounded(&value, GENERATION_INVENTORY_MAX_BYTES_V1 - 1)
                .expect("encode mutated inventory")
                .into_bytes();
            bytes.push(b'\n');
            fs::write(&inventory, bytes).expect("write mutated inventory");
            let error = verify_selected_generation(temp.path(), &id)
                .expect_err("open or non-canonical file entry must fail closed");
            assert!(
                error.to_string().contains(expected_message),
                "unexpected inventory validation error: {error}"
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn recovery_rejects_symlinked_pointer_marker_without_following_it() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().expect("temporary root");
        let selected = publish_complete_generation(temp.path(), "symlink-marker");
        let root = fs::canonicalize(temp.path()).expect("canonical temporary root");
        let abandoned_id = "dddddddddddddddddddddddddddddddd";
        let abandoned_root = root.join(GENERATIONS_DIRECTORY).join(abandoned_id);
        fs::create_dir(&abandoned_root).expect("create abandoned generation");
        let outside = tempfile::NamedTempFile::new().expect("outside marker target");
        fs::write(outside.path(), b"outside sentinel").expect("write outside sentinel");
        let marker = generation_pointer_temporary_path(&root, abandoned_id);
        symlink(outside.path(), &marker).expect("symlink pointer marker");
        let error = GenerationTransaction::begin_replacing(&root, Some(selected.clone()))
            .expect_err("symlinked marker must fail closed");
        assert!(error.to_string().contains("must be a regular file"));
        assert_eq!(
            fs::read(outside.path()).expect("read outside marker target"),
            b"outside sentinel"
        );
        assert!(abandoned_root.is_dir());
        assert!(
            fs::symlink_metadata(marker)
                .expect("marker metadata")
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            current_generation_id(&root).expect("read preserved selection"),
            Some(selected)
        );
    }
    #[cfg(unix)]
    #[test]
    fn recovery_rejects_special_pointer_marker_without_opening_it() {
        use std::os::unix::net::UnixListener;
        let temp = tempfile::tempdir_in("/tmp").expect("short temporary root");
        let selected = publish_complete_generation(temp.path(), "special-marker");
        let root = fs::canonicalize(temp.path()).expect("canonical temporary root");
        let abandoned_id = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
        let abandoned_root = root.join(GENERATIONS_DIRECTORY).join(abandoned_id);
        fs::create_dir(&abandoned_root).expect("create abandoned generation");
        let marker = generation_pointer_temporary_path(&root, abandoned_id);
        let listener = UnixListener::bind(&marker).expect("bind special marker entry");
        let error = GenerationTransaction::begin_replacing(&root, Some(selected.clone()))
            .expect_err("special marker must fail closed");
        assert!(error.to_string().contains("must be a regular file"));
        assert!(abandoned_root.is_dir());
        assert!(marker.exists());
        assert_eq!(
            current_generation_id(&root).expect("read preserved selection"),
            Some(selected)
        );
        drop(listener);
    }
    #[cfg(unix)]
    #[test]
    fn recovery_rejects_insecure_or_hardlinked_pointer_markers() {
        for hardlinked in [false, true] {
            let temp = tempfile::tempdir().expect("temporary root");
            let selected = publish_complete_generation(temp.path(), "marker-identity");
            let root = fs::canonicalize(temp.path()).expect("canonical temporary root");
            let abandoned_id = if hardlinked {
                "11111111111111111111111111111111"
            } else {
                "22222222222222222222222222222222"
            };
            let abandoned_root = root.join(GENERATIONS_DIRECTORY).join(abandoned_id);
            fs::create_dir(&abandoned_root).expect("create abandoned generation");
            let marker = write_crash_marker(&root, abandoned_id, b"");
            let expected_message = if hardlinked {
                fs::hard_link(&marker, root.join("marker-hardlink"))
                    .expect("hardlink pointer marker");
                "changed while it was opened"
            } else {
                fs::set_permissions(&marker, fs::Permissions::from_mode(0o644))
                    .expect("make pointer marker insecure");
                "must be owner-only"
            };
            let error = GenerationTransaction::begin_replacing(&root, Some(selected.clone()))
                .expect_err("unsafe marker identity must fail closed");
            assert!(
                error.to_string().contains(expected_message),
                "unexpected marker validation error: {error}"
            );
            assert!(marker.is_file());
            assert!(abandoned_root.is_dir());
            assert_eq!(
                current_generation_id(&root).expect("read preserved selection"),
                Some(selected)
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn recovery_rejects_malformed_pointer_marker_contents() {
        let temp = tempfile::tempdir().expect("temporary root");
        let selected = publish_complete_generation(temp.path(), "marker-contents");
        let root = fs::canonicalize(temp.path()).expect("canonical temporary root");
        let abandoned_id = "33333333333333333333333333333333";
        let abandoned_root = root.join(GENERATIONS_DIRECTORY).join(abandoned_id);
        fs::create_dir(&abandoned_root).expect("create abandoned generation");
        let marker = write_crash_marker(&root, abandoned_id, b"not-a-prefix");
        let error = GenerationTransaction::begin_replacing(&root, Some(selected.clone()))
            .expect_err("malformed marker contents must fail closed");
        assert!(error.to_string().contains("malformed contents"));
        assert!(marker.is_file());
        assert!(abandoned_root.is_dir());
        assert_eq!(
            current_generation_id(&root).expect("read preserved selection"),
            Some(selected)
        );
    }
    #[cfg(unix)]
    #[test]
    fn recovery_rejects_symlinked_runtime_root_before_deleting_any_candidate_state() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().expect("temporary root");
        let selected = publish_complete_generation(temp.path(), "symlinked-runtime-recovery");
        let root = fs::canonicalize(temp.path()).expect("canonical temporary root");
        let abandoned_id = "ffffffffffffffffffffffffffffffff";
        let abandoned_root = root.join(GENERATIONS_DIRECTORY).join(abandoned_id);
        fs::create_dir(&abandoned_root).expect("create abandoned generation");
        let storage_parent = root.join("peers/peer0/storage-generations");
        fs::create_dir_all(&storage_parent).expect("create storage parent");
        let outside = tempfile::tempdir().expect("outside runtime target");
        fs::write(outside.path().join("sentinel"), b"outside").expect("write outside sentinel");
        let storage = storage_parent.join(abandoned_id);
        symlink(outside.path(), &storage).expect("symlink runtime storage");
        let marker = write_crash_marker(&root, abandoned_id, b"");
        let error = GenerationTransaction::begin_replacing(&root, Some(selected.clone()))
            .expect_err("symlinked runtime storage must fail closed");
        assert!(error.to_string().contains("non-symlink directory"));
        assert!(marker.is_file());
        assert!(abandoned_root.is_dir());
        assert_eq!(
            fs::read(outside.path().join("sentinel")).expect("read outside sentinel"),
            b"outside"
        );
        assert_eq!(
            current_generation_id(&root).expect("read preserved selection"),
            Some(selected)
        );
    }
    #[cfg(unix)]
    #[test]
    fn recovery_unlinks_interior_symlinks_without_touching_their_targets() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().expect("temporary root");
        let selected = publish_complete_generation(temp.path(), "interior-recovery-symlink");
        let root = fs::canonicalize(temp.path()).expect("canonical temporary root");
        let abandoned_id = "0123456789abcdef0123456789abcdef";
        let abandoned_root = root.join(GENERATIONS_DIRECTORY).join(abandoned_id);
        fs::create_dir(&abandoned_root).expect("create abandoned generation");
        let abandoned_storage = create_runtime_storage_fixture(&root, "peer0", abandoned_id);
        let outside = tempfile::tempdir().expect("outside interior target");
        fs::write(outside.path().join("sentinel"), b"outside").expect("write outside sentinel");
        symlink(outside.path(), abandoned_root.join("artifact-link"))
            .expect("symlink abandoned artifact");
        symlink(outside.path(), abandoned_storage.join("state-link"))
            .expect("symlink abandoned runtime state");
        let marker = write_crash_marker(&root, abandoned_id, b"");
        let next = GenerationTransaction::begin_replacing(&root, Some(selected))
            .expect("remove managed trees without following interior symlinks");
        assert!(!marker.exists());
        assert!(!abandoned_root.exists());
        assert!(!abandoned_storage.exists());
        assert_eq!(
            fs::read(outside.path().join("sentinel")).expect("read outside sentinel"),
            b"outside"
        );
        drop(next);
    }
    #[cfg(unix)]
    #[test]
    fn dangling_pointer_and_symlinked_generation_parent_fail_closed() {
        use std::os::unix::fs::symlink;
        let dangling = tempfile::tempdir().expect("dangling-pointer root");
        symlink(
            dangling.path().join("missing-target"),
            dangling.path().join(CURRENT_GENERATION_FILE),
        )
        .expect("create dangling pointer symlink");
        let error = current_generation_id(dangling.path())
            .expect_err("dangling current-generation symlink must fail closed");
        assert!(error.to_string().contains("symbolic link"));
        let root = tempfile::tempdir().expect("symlinked-parent root");
        let outside = tempfile::tempdir().expect("outside generations target");
        symlink(outside.path(), root.path().join(GENERATIONS_DIRECTORY))
            .expect("create generations symlink");
        let error = GenerationTransaction::begin(root.path())
            .expect_err("symlinked generations parent must fail closed");
        assert!(error.to_string().contains("symbolic link"));
        assert!(
            fs::read_dir(outside.path())
                .expect("read outside directory")
                .next()
                .is_none(),
            "candidate allocation must not escape through generations symlink"
        );
    }
}
