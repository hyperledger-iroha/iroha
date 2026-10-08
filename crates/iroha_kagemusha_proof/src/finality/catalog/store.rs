//! Bounded offline original files and canonical integrity inventory, without trust admission.

use super::*;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// Integrity record for one exact original identity. These hashes detect changed
/// storage; neither this record nor its canonical encoding authenticates a catalog.
#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_kagemusha_proof::finality::catalog::ArtifactRecord")]
pub struct ArtifactRecord {
    /// Canonical internal source identity (never a caller-selected file path).
    pub name: Vec<u8>,
    /// Exact descriptor, VK and PK byte lengths in that order.
    pub lengths: [u64; 3],
    /// SHA-256 of those three exact original byte strings.
    pub sha256: [[u8; 32]; 3],
}
impl ArtifactRecord {
    /// Check the exact canonical internal artifact name without reading originals.
    /// This grants no source, completeness, signed-genesis or catalog authority.
    /// # Errors
    /// Unknown role/node, unsupported fields, noncanonical or oversized identity.
    pub fn validate_identity(&self) -> Result<(), Error> {
        validate_name(&self.name)
    }

    /// Compare a canonical compiled artifact identity without exposing its private encoding.
    /// This is a metadata comparison, not source qualification or catalog authority.
    /// # Errors
    /// Malformed stored identity or a noncanonical requested node.
    pub fn matches_identity(&self, id: &ArtifactId) -> Result<bool, Error> {
        self.validate_identity()?;
        Ok(self.name == name(id)?)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_kagemusha_proof::finality::catalog::ArtifactName")]
struct Name {
    version: u8,
    role: u8,
    node: u8,
    kind: u8,
    position: u32,
    children: [[u8; 32]; 4],
}
fn program_tag(program: Program) -> u8 {
    match program {
        Program::Bls => 1,
        Program::Aggregation => 2,
        Program::Result => 3,
        Program::Schedule => 4,
        Program::Context => 5,
        Program::Load => 6,
    }
}
fn composition_tag(kind: Composition) -> u8 {
    match kind {
        Composition::Certificate => 1,
        Composition::CertifiedResult => 2,
        Composition::Schedule => 3,
        Composition::ScheduledResult => 4,
        Composition::HistoryStep => 5,
        Composition::Receipt => 6,
    }
}
pub(super) fn name(id: &ArtifactId) -> Result<Vec<u8>, Error> {
    let mut name = Name {
        version: 1,
        role: 0,
        node: 0,
        kind: 0,
        position: 0,
        children: [[0; 32]; 4],
    };
    let node = match id {
        ArtifactId::Source(node) => Some(node),
        ArtifactId::Wrapper(node) => {
            name.role = 1;
            Some(node)
        }
        ArtifactId::HistoryWrapper => {
            name.role = 2;
            None
        }
    };
    if let Some(node) = node {
        match node {
            NodeId::Leaf(program, position) => {
                name.kind = program_tag(*program);
                name.position = *position;
            }
            NodeId::Merge(children) => {
                name.node = 1;
                name.children = [
                    children[0].descriptor,
                    children[0].key,
                    children[1].descriptor,
                    children[1].key,
                ];
            }
            NodeId::Composition(kind) => {
                name.node = 2;
                name.kind = composition_tag(*kind);
            }
            NodeId::Genesis => name.node = 3,
            NodeId::Append => name.node = 4,
            NodeId::ProgramMerge(..) => return Err(Error::Artifact),
        }
    }
    norito::to_bytes(&name).map_err(|_| Error::Artifact)
}
fn validate_name(bytes: &[u8]) -> Result<(), Error> {
    if bytes.is_empty() || bytes.len() > 512 {
        return Err(Error::Artifact);
    }
    let decoded: Name =
        norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
            .map_err(|_| Error::Artifact)?;
    let node = match decoded.node {
        0 => NodeId::Leaf(
            match decoded.kind {
                1 => Program::Bls,
                2 => Program::Aggregation,
                3 => Program::Result,
                4 => Program::Schedule,
                5 => Program::Context,
                6 => Program::Load,
                _ if decoded.role == 2 => Program::Bls,
                _ => return Err(Error::Artifact),
            },
            decoded.position,
        ),
        1 => NodeId::Merge(Box::new([
            SourceIdentity {
                descriptor: decoded.children[0],
                key: decoded.children[1],
            },
            SourceIdentity {
                descriptor: decoded.children[2],
                key: decoded.children[3],
            },
        ])),
        2 => NodeId::Composition(match decoded.kind {
            1 => Composition::Certificate,
            2 => Composition::CertifiedResult,
            3 => Composition::Schedule,
            4 => Composition::ScheduledResult,
            5 => Composition::HistoryStep,
            6 => Composition::Receipt,
            _ => return Err(Error::Artifact),
        }),
        3 => NodeId::Genesis,
        4 => NodeId::Append,
        _ => return Err(Error::Artifact),
    };
    let id = match decoded.role {
        0 => ArtifactId::Source(node),
        1 => ArtifactId::Wrapper(node),
        2 => ArtifactId::HistoryWrapper,
        _ => return Err(Error::Artifact),
    };
    if name(&id)? != bytes {
        return Err(Error::Artifact);
    }
    Ok(())
}

pub(super) fn check_limits(limits: ImportLimits) -> Result<(), Error> {
    if limits.maximum_artifacts == 0
        || limits.maximum_artifacts == usize::MAX
        || limits.maximum_original_bytes == 0
        || limits.maximum_original_bytes == usize::MAX
        || limits.key.maximum_bytes == 0
        || limits.key.maximum_bytes == usize::MAX
        || limits.key.maximum_rows < 1 << 16
    {
        return Err(Error::Artifact);
    }
    Ok(())
}
pub(super) fn check_original(bytes: &OriginalBytes, limits: ImportLimits) -> Result<usize, Error> {
    if bytes.descriptor.is_empty()
        || bytes.descriptor.len() > 1 << 20
        || bytes.verifying_key.is_empty()
        || bytes.verifying_key.len() > 1 << 18
        || bytes.proving_key.is_empty()
        || bytes.proving_key.len() > limits.key.maximum_bytes
    {
        return Err(Error::Artifact);
    }
    bytes
        .descriptor
        .len()
        .checked_add(bytes.verifying_key.len())
        .and_then(|n| n.checked_add(bytes.proving_key.len()))
        .ok_or(Error::Artifact)
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn file_stem(name: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in hash(name) {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 15)]));
    }
    out
}

const SUFFIX: [&str; 3] = ["descriptor", "vk", "pk"];

fn publish_exact(root: &Path, path: &Path, bytes: &[u8]) -> Result<(), Error> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    if path.try_exists().map_err(|_| Error::Artifact)? {
        if !fs::symlink_metadata(path)
            .map_err(|_| Error::Artifact)?
            .file_type()
            .is_file()
        {
            return Err(Error::Artifact);
        }
        let file = File::open(path).map_err(|_| Error::Artifact)?;
        if file.metadata().map_err(|_| Error::Artifact)?.len()
            != u64::try_from(bytes.len()).map_err(|_| Error::Artifact)?
        {
            return Err(Error::Artifact);
        }
        let mut original = Vec::new();
        file.take(
            u64::try_from(bytes.len())
                .map_err(|_| Error::Artifact)?
                .checked_add(1)
                .ok_or(Error::Artifact)?,
        )
        .read_to_end(&mut original)
        .map_err(|_| Error::Artifact)?;
        return if original == bytes {
            Ok(())
        } else {
            Err(Error::Artifact)
        };
    }
    let temporary = root.join(format!(
        ".original-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary).map_err(|_| Error::Artifact)?;
    file.write_all(bytes).map_err(|_| Error::Artifact)?;
    file.sync_all().map_err(|_| Error::Artifact)?;
    // Hard-link publication refuses replacement, including concurrent changes.
    fs::hard_link(&temporary, path).map_err(|_| Error::Artifact)?;
    fs::remove_file(&temporary).map_err(|_| Error::Artifact)?;
    File::open(root)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| Error::Artifact)
}

/// Offline artifact directory retaining only canonical identity and integrity
/// metadata in memory. It is not wallet custody storage or an authenticated
/// release package. All reads are length-bounded before allocating file buffers;
/// the native importer still checks every source table and key commitment.
pub struct DirectoryCatalog {
    root: PathBuf,
    limits: ImportLimits,
    records: BTreeMap<Vec<u8>, ArtifactRecord>,
    total: usize,
}
impl DirectoryCatalog {
    /// Create a fresh directory; never replace an existing compilation output.
    /// # Errors
    /// Invalid finite bounds, existing path or filesystem failure.
    pub fn create(root: impl AsRef<Path>, limits: ImportLimits) -> Result<Self, Error> {
        check_limits(limits)?;
        let mut directory = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt as _;
            directory.mode(0o700);
        }
        directory
            .create(root.as_ref())
            .map_err(|_| Error::Artifact)?;
        Ok(Self {
            root: root.as_ref().to_path_buf(),
            limits,
            records: BTreeMap::new(),
            total: 0,
        })
    }

    /// Reopen exact originals under an explicitly selected canonical inventory.
    /// This checks shape and finite totals; reads check lengths and integrity.
    /// The caller must authenticate a deployed inventory independently.
    /// # Errors
    /// Noncanonical/duplicate names, unsupported fields, excessive bounds or absent directory.
    pub fn reopen(
        root: impl AsRef<Path>,
        inventory: &[u8],
        limits: ImportLimits,
    ) -> Result<Self, Error> {
        check_limits(limits)?;
        let cap = limits
            .maximum_artifacts
            .checked_mul(2048)
            .and_then(|n| n.checked_add(4096))
            .ok_or(Error::Artifact)?;
        if inventory.is_empty() || inventory.len() > cap || !root.as_ref().is_dir() {
            return Err(Error::Artifact);
        }
        let records: Vec<ArtifactRecord> = norito::decode_canonical_with_limits(
            inventory,
            norito::canonical_decode_limits(inventory.len()),
        )
        .map_err(|_| Error::Artifact)?;
        if records.len() > limits.maximum_artifacts {
            return Err(Error::Artifact);
        }
        let mut result = Self {
            root: root.as_ref().to_path_buf(),
            limits,
            records: BTreeMap::new(),
            total: 0,
        };
        let mut last = None;
        for record in records {
            validate_name(&record.name)?;
            if last.as_ref().is_some_and(|name| name >= &record.name) {
                return Err(Error::Artifact);
            }
            last = Some(record.name.clone());
            let lengths = result.record_lengths(&record)?;
            let length = lengths
                .into_iter()
                .try_fold(0usize, |n, x| n.checked_add(x).ok_or(Error::Artifact))?;
            result.total = result
                .total
                .checked_add(length)
                .filter(|n| *n <= limits.maximum_original_bytes)
                .ok_or(Error::Artifact)?;
            result.records.insert(record.name.clone(), record);
        }
        Ok(result)
    }

    /// Canonical integrity inventory in deterministic identity order.
    /// # Errors
    /// Norito encoding failure. This does not sign or approve the inventory.
    pub fn inventory(&self) -> Result<Vec<u8>, Error> {
        norito::to_bytes(&self.records.values().cloned().collect::<Vec<_>>())
            .map_err(|_| Error::Artifact)
    }

    /// Total original bytes accounted across distinct source identities.
    pub const fn original_bytes(&self) -> usize {
        self.total
    }

    pub(super) fn record(&self, id: &ArtifactId) -> Result<Option<&ArtifactRecord>, Error> {
        Ok(self.records.get(&name(id)?))
    }
    pub(super) fn payload_present(&self, id: &ArtifactId) -> Result<bool, Error> {
        let name = name(id)?;
        if !self.records.contains_key(&name) {
            return Ok(false);
        }
        let path = self.root.join(format!("{}.pk", file_stem(&name)));
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_file() => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Ok(_) | Err(_) => Err(Error::Artifact),
        }
    }
    pub(super) fn evict_payload(&self, id: &ArtifactId) -> Result<(), Error> {
        let name = name(id)?;
        if !self.records.contains_key(&name) {
            return Err(Error::Artifact);
        }
        if self.payload_present(id)? {
            fs::remove_file(self.root.join(format!("{}.pk", file_stem(&name))))
                .map_err(|_| Error::Artifact)?;
            File::open(&self.root)
                .and_then(|directory| directory.sync_all())
                .map_err(|_| Error::Artifact)?;
        }
        Ok(())
    }
    pub(super) fn restore_payload(
        &self,
        id: &ArtifactId,
        bytes: &OriginalBytes,
    ) -> Result<(), Error> {
        let name = name(id)?;
        let record = self.records.get(&name).ok_or(Error::Artifact)?;
        check_original(bytes, self.limits)?;
        let parts = [&bytes.descriptor, &bytes.verifying_key, &bytes.proving_key];
        let lengths = self.record_lengths(record)?;
        if parts
            .iter()
            .enumerate()
            .any(|(i, bytes)| bytes.len() != lengths[i] || hash(bytes) != record.sha256[i])
        {
            return Err(Error::Artifact);
        }
        let stem = file_stem(&name);
        for (i, (suffix, bytes)) in SUFFIX.into_iter().zip(parts).enumerate() {
            let path = self.root.join(format!("{stem}.{suffix}"));
            if i < 2
                && !fs::symlink_metadata(&path)
                    .map_err(|_| Error::Artifact)?
                    .file_type()
                    .is_file()
            {
                return Err(Error::Artifact);
            }
            publish_exact(&self.root, &path, bytes)?;
        }
        Ok(())
    }

    fn record_lengths(&self, record: &ArtifactRecord) -> Result<[usize; 3], Error> {
        let mut lengths = [0; 3];
        for (i, maximum) in [1 << 20, 1 << 18, self.limits.key.maximum_bytes]
            .into_iter()
            .enumerate()
        {
            lengths[i] = usize::try_from(record.lengths[i]).map_err(|_| Error::Artifact)?;
            if lengths[i] == 0 || lengths[i] > maximum {
                return Err(Error::Artifact);
            }
        }
        Ok(lengths)
    }
}
impl ArtifactSource for DirectoryCatalog {
    fn load(&mut self, id: &ArtifactId) -> Result<OriginalBytes, Error> {
        let name = name(id)?;
        let record = self.records.get(&name).ok_or(Error::Artifact)?;
        let lengths = self.record_lengths(record)?;
        let stem = file_stem(&name);
        let mut originals = Vec::with_capacity(3);
        for i in 0..3 {
            let path = self.root.join(format!("{stem}.{}", SUFFIX[i]));
            if !fs::symlink_metadata(&path)
                .map_err(|_| Error::Artifact)?
                .file_type()
                .is_file()
            {
                return Err(Error::Artifact);
            }
            let file = File::open(path).map_err(|_| Error::Artifact)?;
            if file.metadata().map_err(|_| Error::Artifact)?.len() != record.lengths[i] {
                return Err(Error::Artifact);
            }
            let mut bytes = Vec::with_capacity(lengths[i]);
            file.take(record.lengths[i].checked_add(1).ok_or(Error::Artifact)?)
                .read_to_end(&mut bytes)
                .map_err(|_| Error::Artifact)?;
            if bytes.len() != lengths[i] || hash(&bytes) != record.sha256[i] {
                return Err(Error::Artifact);
            }
            originals.push(bytes);
        }
        let mut originals = originals.into_iter();
        Ok(OriginalBytes {
            descriptor: originals.next().ok_or(Error::Artifact)?,
            verifying_key: originals.next().ok_or(Error::Artifact)?,
            proving_key: originals.next().ok_or(Error::Artifact)?,
        })
    }
}
impl ArtifactSink for DirectoryCatalog {
    fn store(&mut self, id: &ArtifactId, bytes: &OriginalBytes) -> Result<(), Error> {
        let length = check_original(bytes, self.limits)?;
        let name = name(id)?;
        let parts = [&bytes.descriptor, &bytes.verifying_key, &bytes.proving_key];
        let record = ArtifactRecord {
            name: name.clone(),
            lengths: [
                u64::try_from(parts[0].len()).map_err(|_| Error::Artifact)?,
                u64::try_from(parts[1].len()).map_err(|_| Error::Artifact)?,
                u64::try_from(parts[2].len()).map_err(|_| Error::Artifact)?,
            ],
            sha256: parts.map(|b| hash(b)),
        };
        if let Some(previous) = self.records.get(&name) {
            if previous != &record {
                return Err(Error::Artifact);
            }
            let previous = self.load(id)?;
            if previous.descriptor != bytes.descriptor
                || previous.verifying_key != bytes.verifying_key
                || previous.proving_key != bytes.proving_key
            {
                return Err(Error::Artifact);
            }
            return Ok(());
        }
        let total = self
            .total
            .checked_add(length)
            .filter(|n| *n <= self.limits.maximum_original_bytes)
            .ok_or(Error::Artifact)?;
        if self.records.len() >= self.limits.maximum_artifacts {
            return Err(Error::Artifact);
        }
        let stem = file_stem(&name);
        for (suffix, bytes) in SUFFIX.into_iter().zip(parts) {
            publish_exact(
                &self.root,
                &self.root.join(format!("{stem}.{suffix}")),
                bytes,
            )?;
        }
        self.records.insert(name, record);
        self.total = total;
        Ok(())
    }
}
