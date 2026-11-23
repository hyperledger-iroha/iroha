//! Durable, manifest-bound source shards for DA publication and recovery.

#[path = "source_quota.rs"]
mod quota;
use super::rs16::visit_chunk_commitments;
use axum::http::StatusCode;
use iroha_data_model::da::{
    ingest::DaIngestRequest,
    manifest::{ChunkRole, DaManifestV1},
};
use iroha_fs::{
    FileSnapshot, OwnerDirectory, PrivateDirectory, PublishMode, ReaderDirectory, RetainedFile,
};
pub(super) use quota::Lease;
use sorafs_car::ChunkStore;
use std::{
    io::{self, Read as _},
    path::Path,
};

/// Write each physical data and parity shard before committing the local ingest receipt.
/// A retry may reuse only identical bytes. The manifest digest is published last.
pub(super) fn retain(
    spool: &Path,
    request: &DaIngestRequest,
    store: &ChunkStore,
    payload: &[u8],
    manifest: &DaManifestV1,
    manifest_hash: &[u8; 32],
    // Retention starts at the original frozen assignment. Only the finalized clock enables expiry.
    (assigned, finalized, budget): (u64, u64, u64),
) -> Result<Lease, (StatusCode, String)> {
    let storage_error = |error: io::Error| {
        (
            match error.kind() {
                io::ErrorKind::StorageFull => StatusCode::INSUFFICIENT_STORAGE,
                io::ErrorKind::InvalidInput => StatusCode::CONFLICT,
                _ => StatusCode::SERVICE_UNAVAILABLE,
            },
            format!("DA source storage: {error}"),
        )
    };
    if spool.as_os_str().is_empty() {
        return Err(storage_error(io::Error::other(
            "missing DA source directory",
        )));
    }
    let lease = quota::reserve(spool, manifest, *manifest_hash, assigned, finalized, budget)
        .map_err(storage_error)?;
    let ticket = hex::encode(manifest.storage_ticket.as_bytes());
    let root = OwnerDirectory::open_or_create(spool).map_err(storage_error)?;
    let ticket_dir = root
        .ensure_child("artifacts")
        .and_then(|dir| dir.ensure_child(&ticket[..2]))
        .and_then(|dir| dir.ensure_child(&ticket))
        .map_err(storage_error)?;
    let source = match ticket_dir.create_private_child("source") {
        Ok(dir) => dir,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => ticket_dir
            .open_private_child("source")
            .map_err(storage_error)?,
        Err(error) => return Err(storage_error(error)),
    };
    let commitments = visit_chunk_commitments(request, store, payload, |chunk, bytes| {
        if manifest.chunks.get(chunk.index as usize) != Some(chunk) {
            return Err(storage_error(io::Error::new(
                io::ErrorKind::InvalidData,
                "DA shard differs from assigned manifest",
            )));
        }
        put_exact(
            &source,
            &lease.staging,
            &format!("chunk_{:05}.bin", chunk.index),
            bytes,
        )
        .map_err(storage_error)
    })?;
    if commitments != manifest.chunks
        || blake3::hash(payload).as_bytes() != manifest.blob_hash.as_bytes()
    {
        return Err(storage_error(io::Error::new(
            io::ErrorKind::InvalidData,
            "DA source inventory differs from assigned manifest",
        )));
    }
    put_exact(&source, &lease.staging, "manifest.blake3", manifest_hash).map_err(storage_error)?;
    source.sync_ordered().map_err(storage_error)?;
    ticket_dir.sync().map_err(storage_error)?;
    Ok(lease)
}

fn put_exact(
    dir: &PrivateDirectory,
    staging: &PrivateDirectory,
    name: &str,
    bytes: &[u8],
) -> io::Result<()> {
    match dir.write_atomic_from_staging(staging, name, bytes, PublishMode::CreateNew) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let existing = dir.read(name, bytes.len())?;
            if existing.as_slice() == bytes {
                Ok(())
            } else {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "conflicting retained DA source",
                ))
            }
        }
        Err(error) => Err(error),
    }
}

/// Retains the original completion marker through the serialized receipt/admission decision.
#[derive(Debug)]
pub(super) struct VerifiedSource {
    pub(super) lease: Lease,
    marker: RetainedFile,
    snapshot: FileSnapshot,
    directory: ReaderDirectory,
    namespace: FileSnapshot,
}

impl VerifiedSource {
    pub(super) fn revalidate(&self) -> io::Result<()> {
        if self.marker.snapshot()? != self.snapshot || self.directory.snapshot()? != self.namespace
        {
            return Err(invalid_source(
                "DA source publication changed after verification",
            ));
        }
        Ok(())
    }
}

fn invalid_source(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Check the marker and every independently committed data/parity file, reading one bounded
/// block at a time. A final metadata pass catches changed earlier shards without retaining one
/// file descriptor per chunk. Call only on the bounded blocking worker outside the replay lock.
pub(super) fn verify_completion(
    spool: &Path,
    manifest: &DaManifestV1,
    expected: &[u8; 32],
    assigned: u64,
) -> io::Result<VerifiedSource> {
    use super::rs16::{
        MAX_CANONICAL_PAYLOAD_BYTES, MAX_CHUNK_SIZE_BYTES, MAX_ERASURE_GENERATED_BYTES,
    };
    if manifest.chunks.is_empty()
        || manifest.chunks.len() > super::rs16::MAX_MANIFEST_CHUNK_COMMITMENTS
        || manifest.total_size > MAX_CANONICAL_PAYLOAD_BYTES
    {
        return Err(invalid_source("invalid DA source inventory bounds"));
    }
    let lease = quota::verify_lease(spool, manifest, expected, assigned)?;
    let name = hex::encode(manifest.storage_ticket.as_bytes());
    let source = OwnerDirectory::open(spool)?
        .open_child("artifacts")?
        .open_child(&name[..2])?
        .open_child(name)?
        .open_private_child("source")?;
    let marker = source.open_retained_private("manifest.blake3")?;
    let snapshot = marker.snapshot()?;
    if source.read("manifest.blake3", expected.len())?.as_slice() != expected {
        return Err(invalid_source(
            "DA source completion does not match the signed manifest",
        ));
    }
    let directory = ReaderDirectory::open(source.path())?;
    let namespace = directory.snapshot()?;
    let verified = VerifiedSource {
        lease,
        marker,
        snapshot,
        directory,
        namespace,
    };
    let mut snapshots = Vec::with_capacity(manifest.chunks.len());
    let mut data = 0_u64;
    let mut parity = 0_u64;
    let mut blob = blake3::Hasher::new();
    let mut buffer = [0_u8; 32 * 1024];
    for (index, chunk) in manifest.chunks.iter().enumerate() {
        if index != chunk.index as usize || chunk.length == 0 || chunk.length > MAX_CHUNK_SIZE_BYTES
        {
            return Err(invalid_source("invalid DA source shard extent"));
        }
        let is_data = chunk.role == ChunkRole::Data;
        if is_data {
            data += u64::from(chunk.length);
        } else {
            parity += u64::from(chunk.length);
        }
        if data > manifest.total_size || parity > MAX_ERASURE_GENERATED_BYTES {
            return Err(invalid_source(
                "DA source shard inventory exceeds byte bounds",
            ));
        }
        let name = format!("chunk_{index:05}.bin");
        let file = source.open_retained_private(&name)?;
        let original = file.snapshot()?;
        if file.file().metadata()?.len() != u64::from(chunk.length) {
            return Err(invalid_source("retained DA shard has the wrong length"));
        }
        let mut shard = blake3::Hasher::new();
        let mut reader = file.file().take(u64::from(chunk.length));
        let mut read = 0_u64;
        loop {
            let n = reader.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            read += n as u64;
            shard.update(&buffer[..n]);
            if is_data {
                blob.update(&buffer[..n]);
            }
        }
        if read != u64::from(chunk.length)
            || shard.finalize().as_bytes() != chunk.commitment.as_bytes()
            || file.snapshot()? != original
        {
            return Err(invalid_source(
                "retained DA shard differs from the signed manifest",
            ));
        }
        snapshots.push(original);
    }
    if data != manifest.total_size || blob.finalize().as_bytes() != manifest.blob_hash.as_bytes() {
        return Err(invalid_source(
            "retained DA data differs from the signed blob",
        ));
    }
    for (index, original) in snapshots.into_iter().enumerate() {
        if source
            .open_retained_private(format!("chunk_{index:05}.bin"))?
            .snapshot()?
            != original
        {
            return Err(invalid_source(
                "DA source shard changed during verification",
            ));
        }
    }
    verified.revalidate()?;
    Ok(verified)
}
