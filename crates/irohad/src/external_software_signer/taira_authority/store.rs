//! Private canonical record persistence with no-replace crash recovery.

use super::protocol::TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1;
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    os::unix::fs::{DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _},
    path::Path,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PersistOutcomeV1 {
    Fresh,
    Existing,
}

pub(super) fn create_private_subdirectory(path: &Path) -> Result<(), ()> {
    fs::DirBuilder::new()
        .mode(0o700)
        .create(path)
        .map_err(|_| ())?;
    validate_private_directory(path)
}

pub(super) fn validate_private_directory(path: &Path) -> Result<(), ()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ())?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o7777 != 0o700
        || metadata.nlink() == 0
    {
        return Err(());
    }
    Ok(())
}

pub(super) fn persist_canonical_once<T: norito::NoritoSerialize>(
    directory: &Path,
    key: [u8; 32],
    value: &T,
) -> Result<PersistOutcomeV1, ()> {
    validate_private_directory(directory)?;
    let bytes = norito::encode_canonical(value).map_err(|_| ())?;
    if bytes.is_empty() || bytes.len() > TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1 {
        return Err(());
    }
    let final_path = directory.join(record_name(key));
    if final_path.exists() {
        return if read_private_regular(&final_path)? == bytes {
            Ok(PersistOutcomeV1::Existing)
        } else {
            Err(())
        };
    }
    let pending_path = directory.join(pending_name(key));
    if pending_path.exists() {
        if read_private_regular(&pending_path)? != bytes {
            return Err(());
        }
    } else {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&pending_path)
            .map_err(|_| ())?;
        file.write_all(&bytes).map_err(|_| ())?;
        file.sync_all().map_err(|_| ())?;
        if read_private_regular(&pending_path)? != bytes {
            return Err(());
        }
    }
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        &pending_path,
        rustix::fs::CWD,
        &final_path,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(|_| ())?;
    sync_directory(directory)?;
    Ok(PersistOutcomeV1::Fresh)
}

pub(super) fn load_canonical_records<T>(directory: &Path) -> Result<BTreeMap<[u8; 32], T>, ()>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    validate_private_directory(directory)?;
    recover_pending::<T>(directory)?;
    let mut records = BTreeMap::new();
    for entry in fs::read_dir(directory).map_err(|_| ())? {
        let entry = entry.map_err(|_| ())?;
        let name = entry.file_name();
        let name = name.to_str().ok_or(())?;
        let key = parse_record_name(name).ok_or(())?;
        let path = entry.path();
        let bytes = read_private_regular(&path)?;
        let value: T = norito::decode_canonical(&bytes).map_err(|_| ())?;
        if norito::encode_canonical(&value).map_err(|_| ())? != bytes
            || records.insert(key, value).is_some()
        {
            return Err(());
        }
    }
    Ok(records)
}

fn recover_pending<T>(directory: &Path) -> Result<(), ()>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    let entries = fs::read_dir(directory)
        .map_err(|_| ())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ())?;
    for entry in entries {
        let name = entry.file_name();
        let name = name.to_str().ok_or(())?;
        let Some(key) = parse_pending_name(name) else {
            continue;
        };
        let pending_path = entry.path();
        let bytes = read_private_regular(&pending_path)?;
        let value: T = norito::decode_canonical(&bytes).map_err(|_| ())?;
        if norito::encode_canonical(&value).map_err(|_| ())? != bytes {
            return Err(());
        }
        let final_path = directory.join(record_name(key));
        if final_path.exists() {
            if read_private_regular(&final_path)? != bytes {
                return Err(());
            }
            fs::remove_file(&pending_path).map_err(|_| ())?;
        } else {
            rustix::fs::renameat_with(
                rustix::fs::CWD,
                &pending_path,
                rustix::fs::CWD,
                &final_path,
                rustix::fs::RenameFlags::NOREPLACE,
            )
            .map_err(|_| ())?;
        }
        sync_directory(directory)?;
    }
    Ok(())
}

pub(super) fn read_private_regular(path: &Path) -> Result<Vec<u8>, ()> {
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits().cast_signed())
        .open(path)
        .map_err(|_| ())?;
    let before = file.metadata().map_err(|_| ())?;
    validate_private_file_metadata(&before)?;
    if before.len() == 0
        || before.len() > u64::try_from(TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1).map_err(|_| ())?
    {
        return Err(());
    }
    let mut bytes = Vec::with_capacity(usize::try_from(before.len()).map_err(|_| ())?);
    (&mut file)
        .take(u64::try_from(TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1 + 1).map_err(|_| ())?)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    let after = file.metadata().map_err(|_| ())?;
    if bytes.is_empty()
        || bytes.len() > TAIRA_AUTHORITY_MAX_FRAME_BYTES_V1
        || file_identity(&before) != file_identity(&after)
    {
        return Err(());
    }
    Ok(bytes)
}

fn validate_private_file_metadata(metadata: &fs::Metadata) -> Result<(), ()> {
    if !metadata.is_file()
        || metadata.uid() != rustix::process::geteuid().as_raw()
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(());
    }
    Ok(())
}

fn file_identity(metadata: &fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64, u64) {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
        metadata.nlink(),
    )
}

fn sync_directory(path: &Path) -> Result<(), ()> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| ())
}

fn record_name(key: [u8; 32]) -> String {
    format!("{}.norito", hex::encode(key))
}

fn pending_name(key: [u8; 32]) -> String {
    format!(".{}.pending", hex::encode(key))
}

fn parse_record_name(name: &str) -> Option<[u8; 32]> {
    parse_named_digest(name.strip_suffix(".norito")?)
}

fn parse_pending_name(name: &str) -> Option<[u8; 32]> {
    parse_named_digest(name.strip_prefix('.')?.strip_suffix(".pending")?)
}

fn parse_named_digest(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    hex::decode(value).ok()?.try_into().ok()
}

pub(super) fn directory_contains_only_records(path: &Path) -> Result<(), ()> {
    validate_private_directory(path)?;
    for entry in fs::read_dir(path).map_err(|_| ())? {
        let entry = entry.map_err(|_| ())?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return Err(());
        };
        if parse_record_name(name).is_none() && parse_pending_name(name).is_none() {
            return Err(());
        }
        if entry.file_type().map_err(|_| ())?.is_symlink() {
            return Err(());
        }
    }
    Ok(())
}

#[allow(dead_code)]
fn _path_component_is_not_authority_supplied(value: &OsStr) -> bool {
    value.to_str().is_some_and(|value| {
        !value.is_empty()
            && value != "."
            && value != ".."
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    })
}
