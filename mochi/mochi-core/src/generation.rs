//! Crash-safe publication of immutable Mochi configuration generations.

use std::{
    fs::{self, File, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _};

use iroha_crypto::{HashOf, PublicKey};
use iroha_data_model::block::BlockHeader;
use norito::json::{self, Map, Value};
use rand::{TryRngCore as _, rngs::OsRng};

use crate::supervisor::{Result, SupervisorError};

pub(crate) const GENERATIONS_DIRECTORY: &str = "generations";
pub(crate) const CURRENT_GENERATION_FILE: &str = "current-generation";
const GENERATION_LOCK_FILE: &str = ".generation.lock";
const GENERATION_INVENTORY_FILE: &str = "generation.json";
const GENERATION_SCHEMA: u64 = 1;

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PublicationFaultPoint {
    BeforeInventory,
    AfterInventory,
    AfterTreeSync,
    AfterGenerationsSync,
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
pub(crate) struct GenerationTransaction {
    root: PathBuf,
    generation_root: PathBuf,
    id: String,
    runtime_storage_roots: Vec<PathBuf>,
    _lock: File,
    committed: bool,
}

impl GenerationTransaction {
    /// Acquire the network generation lock and allocate an invisible candidate.
    pub(crate) fn begin(root: &Path) -> Result<Self> {
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

        for _ in 0..32 {
            let mut entropy = [0_u8; 16];
            OsRng.try_fill_bytes(&mut entropy).map_err(|error| {
                SupervisorError::Config(format!(
                    "failed to obtain OS entropy for Mochi generation id: {error}"
                ))
            })?;
            let id = encode_lower_hex(&entropy);
            let generation_root = generations.join(&id);
            match fs::create_dir(&generation_root) {
                Ok(()) => {
                    #[cfg(unix)]
                    fs::set_permissions(&generation_root, fs::Permissions::from_mode(0o700))?;
                    return Ok(Self {
                        root: root.clone(),
                        generation_root,
                        id,
                        runtime_storage_roots: Vec::new(),
                        _lock: lock,
                        committed: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error.into()),
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

        let peers = self.root.join("peers");
        ensure_direct_child_directory(&self.root, &peers, "runtime peers directory")?;
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
    pub(crate) fn publish(self, context: GenerationInventoryContext<'_>) -> Result<String> {
        self.publish_inner(context, None)
    }

    pub(crate) fn publish_with_fault(
        self,
        context: GenerationInventoryContext<'_>,
        fault: PublicationFaultPoint,
    ) -> Result<String> {
        self.publish_inner(context, Some(fault))
    }

    fn publish_inner(
        mut self,
        context: GenerationInventoryContext<'_>,
        #[cfg_attr(not(test), allow(unused_variables))] fault: Option<PublicationFaultPoint>,
    ) -> Result<String> {
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

        let pointer = self.root.join(CURRENT_GENERATION_FILE);
        reject_symlink(&pointer, "current generation pointer")?;
        let temporary = self
            .root
            .join(format!(".current-generation.{}.tmp", self.id));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&temporary)?;
        file.write_all(self.id.as_bytes())?;
        file.write_all(b"\n")?;
        #[cfg(test)]
        if fault == Some(PublicationFaultPoint::AfterPointerWrite) {
            drop(file);
            let _ = fs::remove_file(&temporary);
            return Err(injected_fault(PublicationFaultPoint::AfterPointerWrite));
        }
        file.sync_all()?;
        drop(file);
        #[cfg(test)]
        if fault == Some(PublicationFaultPoint::AfterPointerSync) {
            let _ = fs::remove_file(&temporary);
            return Err(injected_fault(PublicationFaultPoint::AfterPointerSync));
        }
        let verified = verify_selected_generation(&self.root, &self.id)?;
        ensure_inventory_context(&verified, context)?;

        if let Err(error) = fs::rename(&temporary, &pointer) {
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        // The atomic pointer replacement is the commit point. Never remove a
        // generation after this point, even if directory durability is unknown.
        self.committed = true;
        if fault == Some(PublicationFaultPoint::AfterPointerRename) {
            return Err(SupervisorError::PublicationUncertain {
                generation_id: self.id.clone(),
                source: std::io::Error::other(
                    "injected generation publication fault after pointer rename",
                ),
            });
        }
        if let Err(source) = sync_directory(&self.root) {
            return Err(SupervisorError::PublicationUncertain {
                generation_id: self.id.clone(),
                source,
            });
        }
        Ok(self.id.clone())
    }

    fn write_inventory(&self, context: &GenerationInventoryContext<'_>) -> Result<()> {
        let inventory_path = self.generation_root.join(GENERATION_INVENTORY_FILE);
        let files = generation_file_hashes(&self.generation_root, Some(&inventory_path))?;
        let encoded_before = files
            .iter()
            .map(|(path, hash)| (path.as_str(), hash.as_str()))
            .collect::<Vec<_>>();

        let mut file_values = Vec::with_capacity(files.len());
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
            Value::String(context.expected_hash.to_string()),
        );
        inventory.insert("files".to_owned(), Value::Array(file_values));

        let mut bytes = json::to_vec_pretty(&Value::Object(inventory))?;
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
        let encoded_after = after
            .iter()
            .map(|(path, hash)| (path.as_str(), hash.as_str()))
            .collect::<Vec<_>>();
        if encoded_after != encoded_before {
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
        for storage in self.runtime_storage_roots.iter().rev() {
            if candidate_runtime_storage_is_safe(&self.root, &self.id, storage) {
                let _ = fs::remove_dir_all(storage);
            }
        }
        if self.generation_root == self.root.join(GENERATIONS_DIRECTORY).join(self.id.as_str())
            && fs::symlink_metadata(&self.generation_root)
                .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
            && validate_contained_directory(
                &self.root,
                self.generation_root
                    .parent()
                    .expect("candidate generation always has a parent"),
                "generations directory",
            )
            .is_ok()
        {
            let _ = fs::remove_dir_all(&self.generation_root);
        }
    }
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
    let record = fs::read_to_string(&path)?;
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
    let bytes = fs::read(&inventory_path)?;
    let value: Value = json::from_slice(&bytes)?;
    let mut canonical = json::to_vec_pretty(&value)?;
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
    let expected_hash = hash_record
        .parse::<HashOf<BlockHeader>>()
        .map_err(|error| {
            SupervisorError::GenerationValidation(format!(
                "generation inventory has invalid expected_hash: {error}"
            ))
        })?;
    if expected_hash.to_string() != hash_record {
        return Err(SupervisorError::GenerationValidation(
            "generation inventory expected_hash is not canonical".to_owned(),
        ));
    }
    let recorded = object
        .get("files")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            SupervisorError::GenerationValidation(
                "generation inventory omitted its files array".to_owned(),
            )
        })?
        .iter()
        .map(|entry| {
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
            Ok((path.to_owned(), hash.to_owned()))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut sorted_recorded = recorded.clone();
    sorted_recorded.sort();
    sorted_recorded.dedup();
    if recorded != sorted_recorded {
        return Err(SupervisorError::GenerationValidation(
            "generation inventory file entries must be unique and sorted".to_owned(),
        ));
    }
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
        let components = Path::new(path).components().collect::<Vec<_>>();
        components.len() == 3
            && components[0].as_os_str() == "peers"
            && components[2].as_os_str() == "config.toml"
    }) {
        return Err(SupervisorError::GenerationValidation(format!(
            "selected generation `{id}` inventory contains no peer config"
        )));
    }

    let public_key_path = generation_root.join("genesis/genesis.public_key");
    if fs::read_to_string(&public_key_path)? != format!("{genesis_public_key}\n") {
        return Err(SupervisorError::GenerationValidation(format!(
            "selected generation `{id}` public-key record is not exact"
        )));
    }
    let expected_hash_path = generation_root.join("genesis/genesis.expected_hash");
    if fs::read_to_string(&expected_hash_path)? != format!("{expected_hash}\n") {
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

fn generation_file_hashes(root: &Path, excluded: Option<&Path>) -> Result<Vec<(String, String)>> {
    fn visit(
        root: &Path,
        directory: &Path,
        excluded: Option<&Path>,
        output: &mut Vec<(String, String)>,
    ) -> Result<()> {
        let mut entries = fs::read_dir(directory)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate generation contains symbolic link `{}`",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                visit(root, &path, excluded, output)?;
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
                let hash = blake3::hash(&fs::read(&path)?).to_hex().to_string();
                output.push((relative.to_owned(), hash));
            } else if !metadata.is_file() {
                return Err(SupervisorError::GenerationValidation(format!(
                    "candidate generation contains non-regular entry `{}`",
                    path.display()
                )));
            }
        }
        Ok(())
    }

    let mut output = Vec::new();
    visit(root, root, excluded, &mut output)?;
    output.sort();
    Ok(output)
}

fn sync_tree(path: &Path) -> std::io::Result<()> {
    let mut entries = fs::read_dir(path)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.is_dir() {
            sync_tree(&path)?;
        } else if metadata.is_file() {
            File::open(&path)?.sync_all()?;
        }
    }
    sync_directory(path)
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
    use iroha_crypto::KeyPair;

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
        let manifest = iroha_genesis::GenesisBuilder::new_without_executor(
            chain_id.parse().expect("fixture chain id is canonical"),
            ".",
        )
        .build_raw()
        .with_chain_discriminant(chain_discriminant)
        .with_consensus_meta();
        let block = manifest
            .clone()
            .build_and_sign(&key_pair)
            .expect("sign generation fixture")
            .0;
        let expected_hash = block.hash();
        fs::write(
            genesis_dir.join("genesis.json"),
            json::to_vec_pretty(&manifest).expect("encode fixture manifest"),
        )
        .expect("write fixture manifest");
        fs::write(
            genesis_dir.join("genesis.signed.nrt"),
            block.encode_wire().expect("encode fixture block"),
        )
        .expect("write fixture block");
        fs::write(
            genesis_dir.join("genesis.public_key"),
            format!("{}\n", key_pair.public_key()),
        )
        .expect("write fixture key");
        fs::write(
            genesis_dir.join("genesis.expected_hash"),
            format!("{expected_hash}\n"),
        )
        .expect("write fixture hash");
        fs::write(peer_dir.join("config.toml"), b"fixture config\n").expect("write fixture config");
        (key_pair, expected_hash)
    }

    fn publish_complete_generation(root: &Path, chain_id: &str) -> String {
        let transaction = GenerationTransaction::begin(root).expect("begin fixture generation");
        let (key_pair, expected_hash) = write_complete_candidate(transaction.root(), chain_id, 7);
        transaction
            .publish(GenerationInventoryContext {
                chain_id,
                chain_discriminant: 7,
                genesis_public_key: key_pair.public_key(),
                expected_hash,
            })
            .expect("publish fixture generation")
    }

    #[test]
    fn lock_contention_fails_fast_and_releases_on_drop() {
        let temp = tempfile::tempdir().expect("temporary root");
        let first = GenerationTransaction::begin(temp.path()).expect("acquire first lock");
        let error = GenerationTransaction::begin(temp.path())
            .err()
            .expect("second lock must fail");
        assert!(matches!(error, SupervisorError::GenerationLocked { .. }));
        drop(first);
        GenerationTransaction::begin(temp.path()).expect("lock released after drop");
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
            let transaction = GenerationTransaction::begin(temp.path()).expect("begin candidate");
            let candidate_root = transaction.root().to_path_buf();
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
        let error = transaction
            .publish_with_fault(
                GenerationInventoryContext {
                    chain_id: "postcommit-new",
                    chain_discriminant: 10,
                    genesis_public_key: key_pair.public_key(),
                    expected_hash,
                },
                PublicationFaultPoint::AfterPointerRename,
            )
            .expect_err("post-rename fault must report uncertain durability");
        assert!(matches!(
            error,
            SupervisorError::PublicationUncertain { ref generation_id, .. }
                if generation_id == &candidate_id
        ));
        assert_ne!(candidate_id, old);
        assert_eq!(
            current_generation_id(temp.path()).expect("read committed pointer"),
            Some(candidate_id.clone())
        );
        verify_selected_generation(temp.path(), &candidate_id)
            .expect("committed candidate must survive transaction drop");
    }

    #[test]
    fn selected_generation_verification_rejects_tampering() {
        let temp = tempfile::tempdir().expect("temporary root");
        let transaction = GenerationTransaction::begin(temp.path()).expect("begin candidate");
        let candidate_root = transaction.root().to_path_buf();
        let (key_pair, expected_hash) =
            write_complete_candidate(&candidate_root, "selection-test", 9);
        let id = transaction
            .publish(GenerationInventoryContext {
                chain_id: "selection-test",
                chain_discriminant: 9,
                genesis_public_key: key_pair.public_key(),
                expected_hash,
            })
            .expect("publish generation");
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
        let mut bytes = json::to_vec_pretty(&value).expect("encode substituted inventory");
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
            let mut bytes = json::to_vec_pretty(&value).expect("encode mutated inventory");
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
            .err()
            .expect("symlinked generations parent must fail closed");
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
