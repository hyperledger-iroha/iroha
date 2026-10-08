//! Regenerable proving tables, separate from authenticated D/V and durable proofs.

use super::{
    Result, ServerFinalityErrorV1,
    custody::{Custody, LOCK, Mode, SELECTION, invalid},
};
use iroha_fs::{PrivateDirectory, SealedPrivateFile};
use iroha_kagemusha_proof::finality::catalog::ArtifactRecord;
use iroha_pasta::CancellationToken;
use rand::rand_core::TryRngCore as _;
use sha2::{Digest as _, Sha256};
use std::{
    collections::BTreeMap,
    io::{self, Read as _, Write as _},
    path::Path,
};

pub(super) struct Cache {
    custody: Custody,
    expected: BTreeMap<String, u64>,
    maximum_key_bytes: usize,
    maximum_resident_bytes: u64,
    poisoned: bool,
}
impl Cache {
    pub(super) fn acquire(
        path: &Path,
        selection: &[u8],
        records: &[ArtifactRecord],
        maximum_key_bytes: usize,
        maximum_resident_bytes: usize,
        mode: Mode,
    ) -> Result<Self> {
        let mut expected = BTreeMap::new();
        for record in records {
            if record.lengths[2] == 0 || record.lengths[2] > maximum_key_bytes as u64 {
                return Err(ServerFinalityErrorV1::Binding);
            }
            if expected
                .insert(hex::encode(record.sha256[2]), record.lengths[2])
                .is_some_and(|previous| previous != record.lengths[2])
            {
                return Err(ServerFinalityErrorV1::Binding);
            }
        }
        let cache = Self {
            custody: Custody::acquire(path, selection, mode)?,
            expected,
            maximum_key_bytes,
            maximum_resident_bytes: maximum_resident_bytes as u64,
            poisoned: false,
        };
        cache.inventory()?;
        Ok(cache)
    }

    fn guard(&self) -> io::Result<()> {
        if self.poisoned {
            return Err(invalid());
        }
        self.custody.guard()
    }

    fn inventory(&self) -> io::Result<u64> {
        self.guard()?;
        let mut total = 0u64;
        self.custody.directory.visit_private_files(
            self.expected.len() + 18,
            |name, metadata| {
                let name = name.to_str().ok_or_else(invalid)?;
                if name == LOCK || name == SELECTION {
                    return Ok(());
                }
                let pending = name.strip_prefix("pending-").is_some_and(|suffix| {
                    suffix.len() == 32
                        && suffix
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                });
                if pending {
                    if metadata.len() > self.maximum_key_bytes as u64 {
                        return Err(invalid());
                    }
                } else if self.expected.get(name) != Some(&metadata.len())
                    || !metadata.is_read_only()
                    || metadata.is_empty()
                {
                    return Err(invalid());
                }
                total = total
                    .checked_add(metadata.len())
                    .filter(|bytes| *bytes <= self.maximum_resident_bytes)
                    .ok_or_else(invalid)?;
                Ok(())
            },
        )?;
        self.guard()?;
        Ok(total)
    }

    pub(super) fn get(
        &self,
        record: &ArtifactRecord,
        cancellation: Option<&CancellationToken>,
    ) -> Result<Option<Vec<u8>>> {
        self.inventory()?;
        let bytes = read_original(
            &self.custody.directory,
            record.sha256[2],
            record.lengths[2],
            self.maximum_key_bytes,
            cancellation,
        )?;
        self.guard()?;
        Ok(bytes)
    }

    pub(super) fn put(
        &mut self,
        record: &ArtifactRecord,
        bytes: &[u8],
        cancellation: Option<&CancellationToken>,
    ) -> Result<()> {
        if bytes.len() as u64 != record.lengths[2]
            || bytes.is_empty()
            || bytes.len() > self.maximum_key_bytes
        {
            return Err(ServerFinalityErrorV1::Binding);
        }
        if let Some(old) = self.get(record, cancellation)? {
            return if old == bytes {
                Ok(())
            } else {
                Err(ServerFinalityErrorV1::Binding)
            };
        }
        let total = self.inventory()?;
        if total
            .checked_add(bytes.len() as u64)
            .is_none_or(|n| n > self.maximum_resident_bytes)
        {
            // Only this exclusively owned regenerable namespace can be cleared.
            // Unknown occupants, changed selection, and unsafe entries were refused above.
            self.custody
                .directory
                .clear_contents_preserving(&[LOCK, SELECTION])?;
            self.custody.directory.sync()?;
            if self.inventory()? != 0 {
                return Err(invalid().into());
            }
        }
        let result = self.publish(record, bytes, cancellation);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }

    fn publish(
        &self,
        record: &ArtifactRecord,
        bytes: &[u8],
        cancellation: Option<&CancellationToken>,
    ) -> Result<()> {
        let mut nonce = [0; 16];
        rand::rngs::OsRng
            .try_fill_bytes(&mut nonce)
            .map_err(io::Error::other)?;
        let mut writer = self
            .custody
            .directory
            .create_retained_private(format!("pending-{}", hex::encode(nonce)), bytes.len())?;
        let mut hash = Sha256::new();
        for chunk in bytes.chunks(64 * 1024) {
            checkpoint(cancellation)?;
            hash.update(chunk);
            writer.write_all(chunk)?;
        }
        checkpoint(cancellation)?;
        if <[u8; 32]>::from(hash.finalize()) != record.sha256[2] {
            return Err(ServerFinalityErrorV1::Binding);
        }
        writer
            .seal_read_only()?
            .publish_new_name(hex::encode(record.sha256[2]))?
            .revalidate()?;
        self.custody.directory.sync()?;
        self.inventory()?;
        Ok(())
    }
}

fn checkpoint(cancellation: Option<&CancellationToken>) -> Result<()> {
    CancellationToken::checkpoint(cancellation)
        .map_err(|_| iroha_kagemusha_proof::finality::continuity::producer::Error::Cancelled)?;
    Ok(())
}

pub(super) fn read_original(
    directory: &PrivateDirectory,
    digest: [u8; 32],
    length: u64,
    maximum: usize,
    cancellation: Option<&CancellationToken>,
) -> Result<Option<Vec<u8>>> {
    checkpoint(cancellation)?;
    if length == 0 || length > maximum as u64 || digest == [0; 32] {
        return Err(ServerFinalityErrorV1::Binding);
    }
    directory.revalidate()?;
    let Some(file) = directory.open_retained_read_only_optional(hex::encode(digest), maximum)?
    else {
        return Ok(None);
    };
    let bytes = read_file(file, digest, length, cancellation)?;
    directory.revalidate()?;
    Ok(Some(bytes))
}

fn read_file(
    mut file: SealedPrivateFile,
    digest: [u8; 32],
    length: u64,
    cancellation: Option<&CancellationToken>,
) -> Result<Vec<u8>> {
    let before = file.snapshot()?;
    if file.len()? != length {
        return Err(ServerFinalityErrorV1::Binding);
    }
    let bytes = read_stream(length, digest, cancellation, |buffer| {
        file.revalidate()?;
        let result = file.read(buffer);
        file.revalidate()?;
        result
    })?;
    if file.snapshot()? != before {
        return Err(ServerFinalityErrorV1::Binding);
    }
    Ok(bytes)
}

fn read_stream(
    length: u64,
    digest: [u8; 32],
    cancellation: Option<&CancellationToken>,
    mut read: impl FnMut(&mut [u8]) -> io::Result<usize>,
) -> Result<Vec<u8>> {
    let length = usize::try_from(length).map_err(|_| ServerFinalityErrorV1::Binding)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(io::Error::other)?;
    let mut hash = Sha256::new();
    let mut chunk = vec![0; 64 * 1024].into_boxed_slice();
    loop {
        checkpoint(cancellation)?;
        let remaining = length + 1 - bytes.len();
        let bound = remaining.min(chunk.len());
        let count = read(&mut chunk[..bound])?;
        if count == 0 {
            break;
        }
        hash.update(&chunk[..count]);
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() > length {
            return Err(ServerFinalityErrorV1::Binding);
        }
    }
    checkpoint(cancellation)?;
    if bytes.len() != length || <[u8; 32]>::from(hash.finalize()) != digest {
        return Err(ServerFinalityErrorV1::Binding);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mid_read_cancel_never_hashes_to_success_and_fresh_exact_retry_succeeds() {
        let bytes = vec![91; 3 * 64 * 1024];
        let digest = Sha256::digest(&bytes).into();
        let token = CancellationToken::new();
        let mut cursor = io::Cursor::new(&bytes);
        let error = read_stream(bytes.len() as u64, digest, Some(&token), |buffer| {
            let count = cursor.read(buffer)?;
            token.cancel();
            Ok(count)
        })
        .unwrap_err();
        assert!(error.is_cancelled());
        assert_eq!(cursor.position(), 64 * 1024);
        let fresh = CancellationToken::new();
        let mut cursor = io::Cursor::new(&bytes);
        assert_eq!(
            read_stream(bytes.len() as u64, digest, Some(&fresh), |buffer| cursor
                .read(buffer))
            .unwrap(),
            bytes
        );
        let mut cursor = io::Cursor::new(&bytes);
        assert!(
            read_stream(bytes.len() as u64 - 1, digest, Some(&fresh), |buffer| {
                cursor.read(buffer)
            })
            .is_err()
        );
    }
    #[test]
    fn observed_io_failure_is_not_reclassified_as_concurrent_cancellation() {
        for kind in [io::ErrorKind::PermissionDenied, io::ErrorKind::NotFound] {
            let token = CancellationToken::new();
            let error = read_stream(1, [1; 32], Some(&token), |_| {
                token.cancel();
                Err(kind.into())
            })
            .unwrap_err();
            assert!(!error.is_cancelled());
            assert!(matches!(error, ServerFinalityErrorV1::Storage(error) if error.kind() == kind));
        }
    }
}
