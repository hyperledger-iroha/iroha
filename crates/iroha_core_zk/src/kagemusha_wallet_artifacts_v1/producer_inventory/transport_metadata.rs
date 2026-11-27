//! Immutable bounded metadata publication shared by raw compiler and encoded carriers.

use iroha_fs::{PendingPrivateFile, PrivateDirectory};
use sha2::{Digest as _, Sha256};
use std::{io, io::Read as _, io::Write as _};

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Reserve the sole staging name for an immutable carrier entry. An exact retry
/// must authenticate the published original without introducing a second entry.
pub(super) fn stage_if_absent(
    directory: &PrivateDirectory,
    name: &str,
    maximum: usize,
) -> io::Result<Option<PendingPrivateFile>> {
    let published = || directory.open_retained_read_only_optional(name, maximum);
    if published()?.is_some() {
        return Ok(None);
    }
    let staging = format!("{name}.partial");
    let writer = directory.create_retained_private(&staging, maximum)?;
    if published()?.is_none() {
        return Ok(Some(writer));
    }
    // A publisher completed after our first observation and before our exclusive
    // create. Only discard the empty original we just created, by exact snapshot.
    let empty = writer.finish_private()?;
    let snapshot = empty.snapshot()?;
    drop(empty);
    if !directory.remove_private_if_snapshot(staging, &snapshot)? {
        return Err(invalid("carrier staging original disappeared"));
    }
    Ok(None)
}

pub(super) fn verify_metadata(
    directory: &PrivateDirectory,
    name: &str,
    bytes: &[u8],
) -> io::Result<Option<()>> {
    directory.revalidate()?;
    // Only the first native child open can establish absence. A later NotFound
    // from retained-file or directory validation must never authorize publication.
    let Some(mut original) = directory.open_retained_read_only_optional(name, bytes.len())? else {
        return Ok(None);
    };
    if original.len()? != bytes.len() as u64 {
        return Err(invalid("existing metadata extent differs"));
    }
    let snapshot = original.snapshot()?;
    let expected: [u8; 32] = Sha256::digest(bytes).into();
    let mut hash = Sha256::new();
    let mut total = 0_usize;
    let mut buffer = super::original_stream_buffer()?;
    loop {
        original.revalidate()?;
        if original.snapshot()? != snapshot {
            return Err(invalid("metadata changed before streaming"));
        }
        let count = original.read(&mut buffer)?;
        original.revalidate()?;
        if original.snapshot()? != snapshot {
            return Err(invalid("metadata changed during streaming"));
        }
        if count == 0 {
            break;
        }
        total = total
            .checked_add(count)
            .ok_or_else(|| invalid("metadata extent overflow"))?;
        if total > bytes.len() {
            return Err(invalid("metadata extended during streaming"));
        }
        hash.update(&buffer[..count]);
    }
    let actual: [u8; 32] = hash.finalize().into();
    if total != bytes.len() || actual != expected {
        return Err(invalid("existing metadata original differs"));
    }
    directory.sync()?;
    directory.revalidate()?;
    Ok(Some(()))
}

pub(super) fn publish_metadata(
    directory: &PrivateDirectory,
    name: &str,
    bytes: &[u8],
) -> io::Result<()> {
    if bytes.is_empty() {
        return Err(invalid("empty metadata original"));
    }
    if let Some(mut writer) = stage_if_absent(directory, name, bytes.len())? {
        writer.write_all(bytes)?;
        writer
            .seal_read_only()?
            .publish_new_name(name)?
            .revalidate()?;
    }
    verify_metadata(directory, name, bytes)?.ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
}
