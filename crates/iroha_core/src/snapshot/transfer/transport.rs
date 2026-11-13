//! Bounded delivery of an immutable exported package; transported bytes confer no State authority.
//!
//! Artifact indices select a closed inventory, never caller-provided paths. The receiver publishes
//! the existing transfer marker last. It must subsequently pass ordinary sparse-Kura verification,
//! complete-State authentication and native H+1 replay with independently configured genesis.

use std::{
    io::Write as _,
    num::{NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
};

use iroha_allocation::{AllocationBudget, AllocatorRefusal, ChargedBuffer, ChargedBufferError};
use iroha_data_model::NetworkId;
use iroha_fs::{FileSnapshot, OwnerDirectory, RetainedFile};
use iroha_model_base::chain::ChainId;
use sha2::{Digest as _, Sha256};

use super::CheckpointTransferV1;
use crate::execution_attempt::{ExecutionAttemptError, json_decode_attempt_error};

const TRANSFER_MARKER: &str = "checkpoint-transfer.json";
const MAX_TRANSFER_MARKER: usize = 16 * 1024;
/// Largest single transport read, independent of an untrusted response length.
pub const MAX_CHECKPOINT_TRANSFER_CHUNK_BYTES: usize = 4 * 1024 * 1024;
const FIXED_ARTIFACTS: u64 = 16;
const SNAPSHOTS: [&str; 5] = [
    "snapshot.data",
    "snapshot.sha256",
    "snapshot.sig",
    "snapshot.fast.norito",
    "snapshot.merkle.json",
];
const CANONICAL: [&str; 4] = [
    "blocks.data",
    "blocks.index",
    "blocks.hashes",
    "blocks.count.norito",
];
const PREFIX: [&str; 5] = [
    "manifest.norito",
    "bootstrap.data",
    "bootstrap.indices",
    "bootstrap.hashes",
    "bootstrap.count",
];

/// Finite local transport policy, supplied by the original operation owner.
#[derive(Clone, Copy, Debug)]
pub struct TransferLimits {
    /// Maximum number of files (and charged fixed-size source snapshots).
    pub max_artifacts: NonZeroUsize,
    /// Maximum sum of all transported artifact lengths.
    pub max_total_bytes: NonZeroU64,
    /// Maximum length of any single artifact.
    pub max_artifact_bytes: NonZeroU64,
    /// Working buffer; must not exceed [`MAX_CHECKPOINT_TRANSFER_CHUNK_BYTES`].
    pub chunk_bytes: NonZeroUsize,
}

/// Finite immutable inventory identity. This is transport DATA, not finality authority.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    norito::Encode,
    norito::Decode,
    norito::NoritoSchema,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_core::snapshot::CheckpointPackageDescriptorV1")]
pub struct PackageDescriptorV1 {
    /// Exact closed inventory layout.
    pub version: u16,
    /// Independently pinned chain label.
    pub chain_id: ChainId,
    /// Independently pinned genesis-derived network.
    pub network_id: NetworkId,
    /// Published checkpoint height, with real H+1 retained separately.
    pub height: u64,
    /// Committed time naming the one retained snapshot generation.
    pub committed_ms: u64,
    /// Maximum exact range accepted by this immutable source, at most 4 MiB.
    pub max_chunk_bytes: u32,
    /// Checked sum of every original artifact length.
    pub total_bytes: u64,
    /// SHA-256 over the pinned root coordinates and ordered fixed-size artifact descriptions.
    pub inventory_sha256: [u8; 32],
}

/// One fixed inventory entry; its index alone determines its destination path.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    norito::Encode,
    norito::Decode,
    norito::NoritoSchema,
    norito::json::JsonSerialize,
    norito::json::JsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_core::snapshot::CheckpointPackageArtifactV1")]
pub struct ArtifactV1 {
    /// Zero-based position in the exact closed inventory.
    pub index: u64,
    /// Exact original file extent.
    pub length: u64,
    /// SHA-256 of all original bytes.
    pub sha256: [u8; 32],
}

/// Transport errors preserve their original I/O or codec source.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The package, range, independent pins or local finite limits do not match.
    #[error("checkpoint delivery: {0}")]
    Invalid(&'static str),
    /// Original local source/destination or remote transport I/O failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// Original small transfer-marker JSON decoding failure.
    #[error(transparent)]
    Json(#[from] norito::json::Error),
}
/// Original local retry ownership remains separate from a completed transport rejection.
pub type Attempt<T> = Result<T, ExecutionAttemptError<Error>>;

fn invalid(message: &'static str) -> ExecutionAttemptError<Error> {
    ExecutionAttemptError::Rejected(Error::Invalid(message))
}
fn io(error: std::io::Error) -> ExecutionAttemptError<Error> {
    ExecutionAttemptError::Rejected(Error::Io(error))
}
fn buffer<T>(capacity: usize, budget: &AllocationBudget) -> Attempt<ChargedBuffer<T>> {
    ChargedBuffer::new(capacity, budget).map_err(|error| match error {
        ChargedBufferError::Admission(original) => ExecutionAttemptError::Deferred(original.into()),
        original @ ChargedBufferError::Allocator { .. } => ExecutionAttemptError::Deferred(
            AllocatorRefusal::try_from(original)
                .expect("original buffer allocator failure")
                .into(),
        ),
    })
}
fn bytes(capacity: usize, budget: &AllocationBudget) -> Attempt<ChargedBuffer<u8>> {
    let mut result = buffer(capacity, budget)?;
    for _ in 0..capacity {
        result.push_reserved(0);
    }
    Ok(result)
}
fn count(height: u64, limits: TransferLimits) -> Attempt<usize> {
    if height < 2 || limits.chunk_bytes.get() > MAX_CHECKPOINT_TRANSFER_CHUNK_BYTES {
        return Err(invalid("invalid checkpoint height or chunk bound"));
    }
    height
        .checked_add(FIXED_ARTIFACTS)
        .and_then(|value| usize::try_from(value).ok())
        .filter(|value| *value <= limits.max_artifacts.get())
        .ok_or_else(|| invalid("artifact count exceeds the original finite policy"))
}
fn check_descriptor(
    descriptor: &PackageDescriptorV1,
    chain: &ChainId,
    network: NetworkId,
    limits: TransferLimits,
) -> Attempt<usize> {
    if descriptor.version != 1
        || &descriptor.chain_id != chain
        || descriptor.network_id != network
        || descriptor.total_bytes > limits.max_total_bytes.get()
        || descriptor.max_chunk_bytes == 0
        || descriptor.max_chunk_bytes as usize > MAX_CHECKPOINT_TRANSFER_CHUNK_BYTES
    {
        return Err(invalid(
            "package differs from independent root pins or total limit",
        ));
    }
    count(descriptor.height, limits)
}
fn check_artifact(
    value: ArtifactV1,
    index: usize,
    count: usize,
    limits: TransferLimits,
) -> Attempt<()> {
    if value.index != index as u64
        || index >= count
        || value.length > limits.max_artifact_bytes.get()
        || (index == count - 1 && value.length > MAX_TRANSFER_MARKER as u64)
    {
        return Err(invalid(
            "artifact differs from its fixed index or original size bound",
        ));
    }
    Ok(())
}
fn inventory_start(descriptor: &PackageDescriptorV1) -> Sha256 {
    let mut hash = Sha256::new();
    hash.update(b"iroha-runtime-checkpoint-transfer-inventory-v1");
    hash.update(descriptor.version.to_le_bytes());
    hash.update((descriptor.chain_id.as_str().len() as u64).to_le_bytes());
    hash.update(descriptor.chain_id.as_str().as_bytes());
    hash.update(descriptor.network_id.as_bytes());
    hash.update(descriptor.height.to_le_bytes());
    hash.update(descriptor.committed_ms.to_le_bytes());
    hash.update(descriptor.max_chunk_bytes.to_le_bytes());
    hash
}
fn inventory_add(hash: &mut Sha256, artifact: ArtifactV1) {
    hash.update(artifact.index.to_le_bytes());
    hash.update(artifact.length.to_le_bytes());
    hash.update(artifact.sha256);
}
fn relative(descriptor: &PackageDescriptorV1, index: usize) -> Attempt<PathBuf> {
    let last = descriptor
        .height
        .checked_add(FIXED_ARTIFACTS - 1)
        .ok_or_else(|| invalid("artifact index overflow"))?;
    Ok(match index as u64 {
        0..=4 => PathBuf::from("snapshots/checkpoints")
            .join(format!("{}-{}", descriptor.height, descriptor.committed_ms))
            .join(SNAPSHOTS[index]),
        5..=8 => PathBuf::from("kura/blocks/canonical").join(CANONICAL[index - 5]),
        9..=13 => PathBuf::from("kura/checkpoint_prefix").join(PREFIX[index - 9]),
        14 => PathBuf::from("kura/lane_geometry_journal.norito"),
        value if value < last => {
            PathBuf::from("kura/checkpoint_prefix").join(format!("{:020}.norito", value - 13))
        }
        value if value == last => PathBuf::from(TRANSFER_MARKER),
        _ => return Err(invalid("artifact is outside the closed inventory")),
    })
}
fn marker(bytes: &[u8]) -> Attempt<CheckpointTransferV1> {
    if bytes.len() > MAX_TRANSFER_MARKER {
        return Err(invalid("transfer marker exceeds bound"));
    }
    norito::json::from_slice(bytes).map_err(|error| json_decode_attempt_error(error, Error::Json))
}
fn check_marker(value: &CheckpointTransferV1, descriptor: &PackageDescriptorV1) -> Attempt<()> {
    if value.version != 1
        || value.chain_id != descriptor.chain_id
        || value.network_id != descriptor.network_id
        || value.height != descriptor.height
    {
        return Err(invalid("transfer marker differs from the pinned package"));
    }
    Ok(())
}

/// Source-bound transport. Implementations never select root authority or expose arbitrary paths.
pub trait CheckpointTransport {
    /// Return the source's original fixed inventory identity, preserving local refusals.
    fn descriptor(&mut self) -> Attempt<PackageDescriptorV1>;
    /// Return exactly one fixed inventory entry; caller checks its index and size independently.
    fn artifact(&mut self, index: u64) -> Attempt<ArtifactV1>;
    /// Fill the exact bounded original range. Short reads and source changes are errors.
    fn read_range(&mut self, index: u64, offset: u64, output: &mut [u8]) -> Attempt<()>;
    /// Recheck the entire original source after delivery, with the same inventory identity.
    fn finish(&mut self, expected: &PackageDescriptorV1) -> Attempt<()>;
}

struct Original {
    snapshot: FileSnapshot,
    artifact: ArtifactV1,
}
impl Original {
    fn recheck_file(&self, file: &RetainedFile) -> Attempt<()> {
        file.revalidate().map_err(io)?;
        if file.snapshot().map_err(io)? != self.snapshot {
            return Err(invalid("original package artifact changed"));
        }
        Ok(())
    }
    fn reopen(
        &self,
        root: &OwnerDirectory,
        descriptor: &PackageDescriptorV1,
    ) -> Attempt<RetainedFile> {
        root.revalidate().map_err(io)?;
        let index = usize::try_from(self.artifact.index)
            .map_err(|_| invalid("artifact index overflows"))?;
        let file = RetainedFile::open_regular(root.path().join(relative(descriptor, index)?))
            .map_err(io)?;
        self.recheck_file(&file)?;
        root.revalidate().map_err(io)?;
        Ok(file)
    }
    fn read_file(&self, file: &RetainedFile, offset: u64, output: &mut [u8]) -> Attempt<()> {
        if offset
            .checked_add(output.len() as u64)
            .is_none_or(|end| end > self.artifact.length)
        {
            return Err(invalid("requested range exceeds original artifact extent"));
        }
        self.recheck_file(file)?;
        iroha_fs::read_exact_at(file.file(), output, offset).map_err(io)?;
        self.recheck_file(file)
    }
}

/// Immutable provider over one already-exported package, retaining paid original file snapshots.
/// Opening validates source custody and transport integrity only, never complete-State authority.
/// Only the root and current artifact are open; descriptor count does not grow with checkpoint height.
pub struct ExportedCheckpoint {
    root: OwnerDirectory,
    descriptor: PackageDescriptorV1,
    originals: ChargedBuffer<Original>,
    chunk_bytes: usize,
}
impl ExportedCheckpoint {
    /// Pin an existing completed export to independent chain/network and finite local limits.
    ///
    /// # Errors
    /// Missing/ambiguous artifacts, unsafe or changing sources, foreign pins, limits, original
    /// pool/allocator refusals and original decoder failures remain distinct outcomes.
    pub fn open(
        path: &Path,
        chain: &ChainId,
        network: NetworkId,
        limits: TransferLimits,
        budget: &AllocationBudget,
    ) -> Attempt<Self> {
        let root = OwnerDirectory::open(path).map_err(io)?;
        let manifest_file =
            RetainedFile::open_private(root.path().join(TRANSFER_MARKER)).map_err(io)?;
        let manifest_snapshot = manifest_file.snapshot().map_err(io)?;
        let manifest_len = usize::try_from(manifest_file.file().metadata().map_err(io)?.len())
            .map_err(|_| invalid("transfer marker extent overflows"))?;
        if manifest_len > MAX_TRANSFER_MARKER {
            return Err(invalid("transfer marker exceeds bound"));
        }
        let mut manifest_bytes = bytes(manifest_len, budget)?;
        iroha_fs::read_exact_at(manifest_file.file(), manifest_bytes.as_mut_slice(), 0)
            .map_err(io)?;
        let manifest = marker(manifest_bytes.as_slice())?;
        if &manifest.chain_id != chain || manifest.network_id != network || manifest.version != 1 {
            return Err(invalid("export differs from independent root pins"));
        }
        let count = count(manifest.height, limits)?;
        let candidates =
            super::super::checkpoint_store::entries(&root.path().join("snapshots")).map_err(io)?;
        if candidates.len() != 1 || candidates[0].0 != manifest.height {
            return Err(invalid(
                "export must contain exactly one retained checkpoint",
            ));
        }
        let mut descriptor = PackageDescriptorV1 {
            version: 1,
            chain_id: chain.clone(),
            network_id: network,
            height: manifest.height,
            committed_ms: candidates[0].1,
            max_chunk_bytes: limits.chunk_bytes.get() as u32,
            total_bytes: 0,
            inventory_sha256: [0; 32],
        };
        let mut originals = buffer(count, budget)?;
        let mut scratch = bytes(limits.chunk_bytes.get(), budget)?;
        let mut inventory = inventory_start(&descriptor);
        for index in 0..count {
            let file = RetainedFile::open_regular(root.path().join(relative(&descriptor, index)?))
                .map_err(io)?;
            let snapshot = file.snapshot().map_err(io)?;
            let length = file.file().metadata().map_err(io)?.len();
            let mut original = Original {
                snapshot,
                artifact: ArtifactV1 {
                    index: index as u64,
                    length,
                    sha256: [0; 32],
                },
            };
            check_artifact(original.artifact, index, count, limits)?;
            descriptor.total_bytes = descriptor
                .total_bytes
                .checked_add(length)
                .filter(|value| *value <= limits.max_total_bytes.get())
                .ok_or_else(|| invalid("package exceeds original total-byte bound"))?;
            let mut hash = Sha256::new();
            let mut offset = 0;
            while offset < length {
                let take = usize::try_from((length - offset).min(scratch.as_slice().len() as u64))
                    .unwrap();
                let chunk = &mut scratch.as_mut_slice()[..take];
                original.read_file(&file, offset, chunk)?;
                hash.update(&*chunk);
                offset += take as u64;
            }
            original.recheck_file(&file)?;
            original.artifact.sha256 = hash.finalize().into();
            if index == count - 1 && original.snapshot != manifest_snapshot {
                return Err(invalid("transfer marker changed during inventory capture"));
            }
            inventory_add(&mut inventory, original.artifact);
            originals.push_reserved(original);
        }
        descriptor.inventory_sha256 = inventory.finalize().into();
        let mut provider = Self {
            root,
            descriptor: descriptor.clone(),
            originals,
            chunk_bytes: limits.chunk_bytes.get(),
        };
        provider.finish(&descriptor)?;
        Ok(provider)
    }
}
impl CheckpointTransport for ExportedCheckpoint {
    fn descriptor(&mut self) -> Attempt<PackageDescriptorV1> {
        self.root.revalidate().map_err(io)?;
        Ok(self.descriptor.clone())
    }
    fn artifact(&mut self, index: u64) -> Attempt<ArtifactV1> {
        let original = usize::try_from(index)
            .ok()
            .and_then(|index| self.originals.as_slice().get(index))
            .ok_or_else(|| invalid("artifact index exceeds original inventory"))?;
        original.reopen(&self.root, &self.descriptor)?;
        Ok(original.artifact)
    }
    fn read_range(&mut self, index: u64, offset: u64, output: &mut [u8]) -> Attempt<()> {
        if output.is_empty() || output.len() > self.chunk_bytes {
            return Err(invalid("requested chunk exceeds original transfer bound"));
        }
        self.root.revalidate().map_err(io)?;
        let original = usize::try_from(index)
            .ok()
            .and_then(|index| self.originals.as_slice().get(index))
            .ok_or_else(|| invalid("artifact index exceeds original inventory"))?;
        let file = original.reopen(&self.root, &self.descriptor)?;
        original.read_file(&file, offset, output)?;
        self.root.revalidate().map_err(io)
    }
    fn finish(&mut self, expected: &PackageDescriptorV1) -> Attempt<()> {
        if expected != &self.descriptor {
            return Err(invalid("original transfer inventory changed"));
        }
        self.root.revalidate().map_err(io)?;
        for original in self.originals.as_slice() {
            original.reopen(&self.root, &self.descriptor)?;
        }
        self.root.revalidate().map_err(io)
    }
}

fn destination_directory(root: &OwnerDirectory, relative: &Path) -> Attempt<OwnerDirectory> {
    let mut directory = OwnerDirectory::open(root.path()).map_err(io)?;
    for component in relative.parent().unwrap_or(Path::new("")).components() {
        let std::path::Component::Normal(name) = component else {
            return Err(invalid("destination escaped the closed inventory"));
        };
        directory = directory.ensure_child(name).map_err(io)?;
    }
    Ok(directory)
}

/// Receive one exact inventory into a fresh owner-private directory, publishing its marker last.
///
/// This returns transport DATA only. The caller must independently configure genesis, snapshot
/// verification and ordinary startup; it cannot use this return value to construct a native tip.
/// Failed transfers retain partial files without the completion marker and never replace a store.
///
/// # Errors
/// Foreign pins, changed/truncated/substituted ranges, size limits, occupied destinations,
/// source-finalization failure and original local allocation/decoder/I/O refusals.
#[allow(clippy::too_many_arguments)]
pub fn receive_checkpoint(
    transport: &mut impl CheckpointTransport,
    expected: &PackageDescriptorV1,
    chain: &ChainId,
    network: NetworkId,
    limits: TransferLimits,
    budget: &AllocationBudget,
    destination: &Path,
) -> Attempt<CheckpointTransferV1> {
    let count = check_descriptor(expected, chain, network, limits)?;
    if &transport.descriptor()? != expected {
        return Err(invalid("transfer changed its pinned inventory"));
    }
    // Admit all fixed-size file snapshots and working buffers before any destination mutation.
    let mut written: ChargedBuffer<Original> = buffer(count - 1, budget)?;
    let mut scratch = bytes(
        limits
            .chunk_bytes
            .get()
            .min(expected.max_chunk_bytes as usize),
        budget,
    )?;
    let mut manifest = bytes(MAX_TRANSFER_MARKER, budget)?;
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder.create(destination).map_err(io)?;
    let root = OwnerDirectory::open(destination).map_err(io)?;
    let mut inventory = inventory_start(expected);
    let mut total = 0_u64;
    let mut marker_len = 0;
    for index in 0..count {
        let artifact = transport.artifact(index as u64)?;
        check_artifact(artifact, index, count, limits)?;
        total = total
            .checked_add(artifact.length)
            .filter(|value| {
                *value <= expected.total_bytes && *value <= limits.max_total_bytes.get()
            })
            .ok_or_else(|| invalid("received extent exceeds original total bound"))?;
        inventory_add(&mut inventory, artifact);
        let relative = relative(expected, index)?;
        let parent = destination_directory(&root, &relative)?;
        let mut output = if index == count - 1 {
            None
        } else {
            Some(
                RetainedFile::create_new_private(parent.path().join(relative.file_name().unwrap()))
                    .map_err(io)?,
            )
        };
        let mut hash = Sha256::new();
        let mut offset = 0;
        while offset < artifact.length {
            let take =
                usize::try_from((artifact.length - offset).min(scratch.as_slice().len() as u64))
                    .unwrap();
            let chunk = &mut scratch.as_mut_slice()[..take];
            transport.read_range(index as u64, offset, chunk)?;
            hash.update(&*chunk);
            if let Some(file) = &mut output {
                file.file_mut().write_all(chunk).map_err(io)?;
            } else {
                manifest.as_mut_slice()[offset as usize..offset as usize + take]
                    .copy_from_slice(chunk);
                marker_len += take;
            }
            offset += take as u64;
        }
        if <[u8; 32]>::from(hash.finalize()) != artifact.sha256 {
            return Err(invalid(
                "received artifact digest differs from original inventory",
            ));
        }
        if let Some(file) = output {
            let sealed = file.seal().map_err(io)?;
            written.push_reserved(Original {
                snapshot: sealed.snapshot().map_err(io)?,
                artifact,
            });
            parent.sync().map_err(io)?;
        }
    }
    if total != expected.total_bytes
        || <[u8; 32]>::from(inventory.finalize()) != expected.inventory_sha256
    {
        return Err(invalid(
            "received inventory differs from the pinned package",
        ));
    }
    let marker_bytes = &manifest.as_slice()[..marker_len];
    let manifest = marker(marker_bytes)?;
    check_marker(&manifest, expected)?;
    for original in written.as_slice() {
        let file = original.reopen(&root, expected)?;
        // The stream digest predates sealing. Independently hash the actual sealed destination
        // so a concurrent write before its first snapshot cannot publish a corrupt package.
        if file.file().metadata().map_err(io)?.len() != original.artifact.length {
            return Err(invalid(
                "sealed destination extent differs from its artifact",
            ));
        }
        let mut hash = Sha256::new();
        let mut offset = 0;
        while offset < original.artifact.length {
            let take = usize::try_from(
                (original.artifact.length - offset).min(scratch.as_slice().len() as u64),
            )
            .unwrap();
            let chunk = &mut scratch.as_mut_slice()[..take];
            original.read_file(&file, offset, chunk)?;
            hash.update(&*chunk);
            offset += take as u64;
        }
        if <[u8; 32]>::from(hash.finalize()) != original.artifact.sha256 {
            return Err(invalid(
                "sealed destination digest differs from its artifact",
            ));
        }
    }
    // Recheck the remote originals (and the transport owner's absolute deadline) after the
    // final local hashing work; a timed-out receive must never publish its completion marker.
    transport.finish(expected)?;
    root.revalidate().map_err(io)?;
    root.write_private_atomic(
        TRANSFER_MARKER,
        marker_bytes,
        iroha_fs::PublishMode::CreateNew,
    )
    .map_err(io)?;
    root.revalidate().map_err(io)?;
    Ok(manifest)
}

#[cfg(test)]
mod tests;
