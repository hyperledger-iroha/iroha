//! Explicit offline compilation with bounded regenerable proving-key residency.
//!
//! This tooling backend is never an authenticated wallet loader. Recipes exist
//! only in memory and are rebuilt by the compiled source walk on every restart.

use super::*;
use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

const OWNER: &[u8] = b"KAGEMUSHA offline regenerable proving keys v1\n";

/// Driver-owned offline original inventory with an explicitly bounded PK working set.
/// Descriptor/VK bytes and all original hashes/lengths remain immutable. Only
/// regenerable PK payloads are evicted; a regenerated payload must match every
/// retained original identity before the ordinary strict importer sees it.
pub struct StreamingCatalog {
    directory: DirectoryCatalog,
    root: PathBuf,
    limits: ImportLimits,
    working_bytes: usize,
    resident: VecDeque<(ArtifactId, usize)>,
    recipes: BTreeMap<ArtifactId, OriginalRecipe>,
}
impl StreamingCatalog {
    /// Create an exclusive offline output directory. No other writer may use it.
    /// `working_bytes` bounds PK files, independently of logical inventory totals.
    /// # Errors
    /// Existing directory, invalid finite bounds or filesystem failure.
    pub fn create(
        root: impl AsRef<Path>,
        limits: ImportLimits,
        working_bytes: usize,
    ) -> Result<Self, Error> {
        Self::bounds(limits, working_bytes)?;
        let directory = DirectoryCatalog::create(&root, limits)?;
        let root = root.as_ref().to_path_buf();
        let mut owner = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(root.join("offline-owner"))
            .map_err(|_| Error::Artifact)?;
        owner
            .write_all(OWNER)
            .and_then(|()| owner.sync_all())
            .map_err(|_| Error::Artifact)?;
        let result = Self {
            directory,
            root,
            limits,
            working_bytes,
            resident: VecDeque::new(),
            recipes: BTreeMap::new(),
        };
        result.persist_inventory()?;
        Ok(result)
    }
    /// Reopen only this driver's marked directory and canonical inventory.
    /// PK files are discarded; compiled recipes must be registered again before
    /// missing payloads can regenerate. No executable recipe is read from disk.
    /// # Errors
    /// Foreign directory, unbounded/changed metadata, unsupported files or I/O failure.
    pub fn reopen(
        root: impl AsRef<Path>,
        limits: ImportLimits,
        working_bytes: usize,
    ) -> Result<Self, Error> {
        Self::bounds(limits, working_bytes)?;
        let root = root.as_ref().to_path_buf();
        if read(&root.join("offline-owner"), OWNER.len())? != OWNER {
            return Err(Error::Artifact);
        }
        let maximum = limits
            .maximum_artifacts
            .checked_mul(2048)
            .and_then(|n| n.checked_add(4096))
            .ok_or(Error::Artifact)?;
        let inventory = read(&root.join("inventory.norito"), maximum)?;
        let directory = DirectoryCatalog::reopen(&root, &inventory, limits)?;
        for entry in fs::read_dir(&root).map_err(|_| Error::Artifact)? {
            let entry = entry.map_err(|_| Error::Artifact)?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                return Err(Error::Artifact);
            };
            if Path::new(name)
                .extension()
                .is_some_and(|extension| extension == "pk")
            {
                if name.len() != 67
                    || !name[..64].bytes().all(|b| b.is_ascii_hexdigit())
                    || !entry.file_type().map_err(|_| Error::Artifact)?.is_file()
                {
                    return Err(Error::Artifact);
                }
                // This exclusive marked directory owns these regenerable payloads,
                // including a crash between payload publication and inventory sync.
                fs::remove_file(entry.path()).map_err(|_| Error::Artifact)?;
            } else if name.starts_with(".original-") {
                if !entry.file_type().map_err(|_| Error::Artifact)?.is_file() {
                    return Err(Error::Artifact);
                }
                // Interrupted atomic publication owns no immutable identity yet.
                fs::remove_file(entry.path()).map_err(|_| Error::Artifact)?;
            }
        }
        File::open(&root)
            .and_then(|f| f.sync_all())
            .map_err(|_| Error::Artifact)?;
        Ok(Self {
            directory,
            root,
            limits,
            working_bytes,
            resident: VecDeque::new(),
            recipes: BTreeMap::new(),
        })
    }
    fn bounds(limits: ImportLimits, working: usize) -> Result<(), Error> {
        store::check_limits(limits)?;
        if working < limits.key.maximum_bytes || working == usize::MAX {
            return Err(Error::Artifact);
        }
        Ok(())
    }
    fn persist_inventory(&self) -> Result<(), Error> {
        let bytes = self.directory.inventory()?;
        let temporary = self.root.join("inventory.pending");
        // Only this exclusive offline owner's metadata staging file is replaced.
        if temporary.try_exists().map_err(|_| Error::Artifact)? {
            if !fs::symlink_metadata(&temporary)
                .map_err(|_| Error::Artifact)?
                .file_type()
                .is_file()
            {
                return Err(Error::Artifact);
            }
            fs::remove_file(&temporary).map_err(|_| Error::Artifact)?;
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| Error::Artifact)?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| Error::Artifact)?;
        fs::rename(temporary, self.root.join("inventory.norito")).map_err(|_| Error::Artifact)?;
        File::open(&self.root)
            .and_then(|f| f.sync_all())
            .map_err(|_| Error::Artifact)
    }
    fn reserve(&mut self, id: &ArtifactId, bytes: usize) -> Result<(), Error> {
        if bytes > self.working_bytes {
            return Err(Error::Artifact);
        }
        self.resident.retain(|(other, _)| other != id);
        loop {
            let total = self.resident.iter().try_fold(bytes, |n, (_, size)| {
                n.checked_add(*size).ok_or(Error::Artifact)
            })?;
            if total <= self.working_bytes {
                break;
            }
            let (old, _) = self.resident.pop_front().ok_or(Error::Artifact)?;
            self.directory.evict_payload(&old)?;
        }
        self.resident.push_back((id.clone(), bytes));
        Ok(())
    }
    /// Logical original bytes committed by inventory, including evicted PK payloads.
    pub const fn original_bytes(&self) -> usize {
        self.directory.original_bytes()
    }
    /// Current bounded resident PK bytes, excluding immutable small descriptor/VK files.
    pub fn resident_bytes(&self) -> usize {
        self.resident.iter().map(|(_, n)| n).sum()
    }
    /// Export canonical integrity inventory; this is not catalog authentication.
    /// # Errors
    /// Canonical serialization failure.
    pub fn inventory(&self) -> Result<Vec<u8>, Error> {
        self.directory.inventory()
    }
}
fn read(path: &Path, maximum: usize) -> Result<Vec<u8>, Error> {
    if !fs::symlink_metadata(path)
        .map_err(|_| Error::Artifact)?
        .file_type()
        .is_file()
    {
        return Err(Error::Artifact);
    }
    let file = File::open(path).map_err(|_| Error::Artifact)?;
    let length = usize::try_from(file.metadata().map_err(|_| Error::Artifact)?.len())
        .map_err(|_| Error::Artifact)?;
    if length > maximum {
        return Err(Error::Artifact);
    }
    let mut bytes = Vec::with_capacity(length);
    file.take(
        u64::try_from(length)
            .map_err(|_| Error::Artifact)?
            .checked_add(1)
            .ok_or(Error::Artifact)?,
    )
    .read_to_end(&mut bytes)
    .map_err(|_| Error::Artifact)?;
    if bytes.len() != length {
        return Err(Error::Artifact);
    }
    Ok(bytes)
}
impl ArtifactSource for StreamingCatalog {
    fn load(&mut self, id: &ArtifactId) -> Result<OriginalBytes, Error> {
        let record = self.directory.record(id)?.ok_or(Error::Artifact)?;
        let size = usize::try_from(record.lengths[2]).map_err(|_| Error::Artifact)?;
        self.reserve(id, size)?;
        if !self.directory.payload_present(id)? {
            let original = self.recipes.get(id).ok_or(Error::Artifact)?.regenerate()?;
            self.directory.restore_payload(id, &original)?;
        }
        self.directory.load(id)
    }
}
impl ArtifactSink for StreamingCatalog {
    fn register_recipe(&mut self, id: &ArtifactId, recipe: OriginalRecipe) -> Result<(), Error> {
        if !self.recipes.contains_key(id) && self.recipes.len() >= self.limits.maximum_artifacts {
            return Err(Error::Artifact);
        }
        self.recipes.insert(id.clone(), recipe);
        Ok(())
    }
    fn store(&mut self, id: &ArtifactId, bytes: &OriginalBytes) -> Result<(), Error> {
        store::check_original(bytes, self.limits)?;
        self.reserve(id, bytes.proving_key.len())?;
        if self.directory.record(id)?.is_some() {
            self.directory.restore_payload(id, bytes)?;
        } else {
            self.directory.store(id, bytes)?;
        }
        self.persist_inventory()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_pasta::msm::MemoryBudget;
    use iroha_plonk::keys::pk::artifact::ReadConfig;

    fn limits() -> ImportLimits {
        ImportLimits {
            key: ReadConfig {
                maximum_bytes: 3,
                maximum_rows: 1 << 16,
                coset_cache: CosetCachePolicy::OnDemand,
                msm_budget: MemoryBudget::DEFAULT,
            },
            maximum_artifacts: 4,
            maximum_original_bytes: 100,
        }
    }
    fn bytes(value: u8) -> OriginalBytes {
        OriginalBytes {
            descriptor: vec![1],
            verifying_key: vec![2],
            proving_key: vec![value; 2],
        }
    }
    #[test]
    fn offline_eviction_preserves_inventory_and_exact_regeneration() {
        let root =
            std::env::temp_dir().join(format!("kg-streaming-catalog-{}", std::process::id()));
        let mut catalog = StreamingCatalog::create(&root, limits(), 3).unwrap();
        let a = ArtifactId::Source(NodeId::Genesis);
        let b = ArtifactId::Source(NodeId::Append);
        catalog
            .register_recipe(&a, OriginalRecipe::test_bytes(bytes(3)))
            .unwrap();
        catalog
            .register_recipe(&b, OriginalRecipe::test_bytes(bytes(4)))
            .unwrap();
        catalog.store(&a, &bytes(3)).unwrap();
        catalog.store(&b, &bytes(4)).unwrap();
        assert_eq!(catalog.resident_bytes(), 2);
        assert!(!catalog.directory.payload_present(&a).unwrap());
        let inventory = catalog.inventory().unwrap();
        assert_eq!(catalog.load(&a).unwrap().proving_key, [3, 3]);
        assert_eq!(catalog.inventory().unwrap(), inventory);
        assert!(!catalog.directory.payload_present(&b).unwrap());
        catalog
            .register_recipe(&b, OriginalRecipe::test_bytes(bytes(9)))
            .unwrap();
        assert!(
            catalog.load(&b).is_err(),
            "different regenerated original refuses"
        );
        drop(catalog);
        let mut reopened = StreamingCatalog::reopen(&root, limits(), 3).unwrap();
        assert_eq!(reopened.inventory().unwrap(), inventory);
        assert!(
            reopened.load(&a).is_err(),
            "disk inventory cannot install recipes"
        );
        reopened
            .register_recipe(&a, OriginalRecipe::test_bytes(bytes(3)))
            .unwrap();
        assert_eq!(reopened.load(&a).unwrap().proving_key, [3, 3]);
        assert!(reopened.store(&a, &bytes(7)).is_err());
        assert_eq!(reopened.original_bytes(), 8);
        fs::remove_dir_all(root).unwrap();
    }
}
