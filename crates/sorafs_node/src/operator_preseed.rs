//! Durable operator-preseed qualification validation for disabled-provider Inrou stores.

use std::{
    collections::BTreeSet,
    fs,
    io::Read as _,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;

#[cfg(unix)]
unsafe extern "C" {
    fn geteuid() -> std::os::raw::c_uint;
}

#[cfg(target_os = "linux")]
const OPERATOR_PRESEED_OPEN_NOFOLLOW: std::os::raw::c_int = 0x2_0000;
#[cfg(target_os = "macos")]
const OPERATOR_PRESEED_OPEN_NOFOLLOW: std::os::raw::c_int = 0x100;

use sorafs_manifest::operator_preseed::{
    OPERATOR_PRESEED_SESSION_MAX_STORES_V1, OPERATOR_PRESEED_SESSION_RECEIPT_VERSION_V1,
    OPERATOR_PRESEED_STORE_RECEIPT_DIR_V1, OperatorPreseedSessionReceiptV1,
};

use crate::store::StorageBackend;

/// Maximum canonical bytes accepted for one durable V1 preseed qualification.
pub const OPERATOR_PRESEED_STORE_RECEIPT_MAX_BYTES_V1: u64 = 1024 * 1024;
/// Maximum retained content-addressed qualifications accepted at one store in V1.
pub const OPERATOR_PRESEED_STORE_RECEIPT_MAX_COUNT_V1: usize = 4096;

/// Return the fixed qualification directory below one canonical SoraFS store root.
#[must_use]
pub fn operator_preseed_store_receipt_dir(store_root: &Path) -> PathBuf {
    store_root.join(OPERATOR_PRESEED_STORE_RECEIPT_DIR_V1)
}

/// Compute the lowercase BLAKE3-256 identity of canonical receipt bytes.
#[must_use]
pub fn operator_preseed_receipt_digest_hex(receipt_bytes: &[u8]) -> String {
    hex::encode(blake3::hash(receipt_bytes).as_bytes())
}

/// Return the content-addressed path for canonical receipt bytes below one store.
#[must_use]
pub fn operator_preseed_store_receipt_path(store_root: &Path, receipt_bytes: &[u8]) -> PathBuf {
    operator_preseed_store_receipt_dir(store_root).join(format!(
        "{}.json",
        operator_preseed_receipt_digest_hex(receipt_bytes)
    ))
}

/// Install one canonical store-qualification staging file without replacing a final receipt.
///
/// # Errors
///
/// Rejects paths outside the fixed qualification directory, malformed staging/final names, or
/// any platform/filesystem failure that cannot provide an atomic create-only rename.
pub fn install_operator_preseed_store_receipt_staging(
    staging: &Path,
    destination: &Path,
) -> Result<(), String> {
    let staging_parent = staging
        .parent()
        .ok_or_else(|| "operator-preseed staging path has no parent".to_owned())?;
    if destination.parent() != Some(staging_parent)
        || staging_parent.file_name().and_then(|name| name.to_str())
            != Some(OPERATOR_PRESEED_STORE_RECEIPT_DIR_V1)
    {
        return Err(
            "operator-preseed staging and destination must share the fixed qualification directory"
                .to_owned(),
        );
    }
    let staging_name = staging
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "operator-preseed staging name must be UTF-8".to_owned())?;
    if !staging_name.starts_with(".qualification.") || !staging_name.ends_with(".tmp") {
        return Err("operator-preseed staging name is not canonical".to_owned());
    }
    let destination_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "operator-preseed destination name must be UTF-8".to_owned())?;
    let Some(digest_hex) = destination_name.strip_suffix(".json") else {
        return Err("operator-preseed destination name is not canonical".to_owned());
    };
    let digest = hex::decode(digest_hex)
        .map_err(|_| "operator-preseed destination digest must be lowercase hex".to_owned())?;
    if digest.len() != 32 || hex::encode(digest) != digest_hex {
        return Err(
            "operator-preseed destination digest must be exactly 32 lowercase hex bytes".to_owned(),
        );
    }
    crate::governance_rooted_fs::rename_path_exclusive(staging, destination).map_err(|error| {
        format!(
            "failed to atomically install operator-preseed qualification without replacement: {error}"
        )
    })
}

fn decode_canonical_operator_preseed_receipt(
    bytes: &[u8],
) -> Result<OperatorPreseedSessionReceiptV1, String> {
    let receipt: OperatorPreseedSessionReceiptV1 = norito::json::from_slice(bytes)
        .map_err(|error| format!("failed to decode operator-preseed qualification: {error}"))?;
    receipt
        .validate()
        .map_err(|error| format!("invalid operator-preseed qualification: {error}"))?;
    let canonical = norito::json::to_vec(&receipt)
        .map_err(|error| format!("failed to re-encode operator-preseed qualification: {error}"))?;
    if canonical != bytes {
        return Err("operator-preseed qualification is not canonical compact JSON".to_owned());
    }
    Ok(receipt)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn open_operator_preseed_file_nofollow(path: &Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::OpenOptionsExt as _;

    fs::OpenOptions::new()
        .read(true)
        .custom_flags(OPERATOR_PRESEED_OPEN_NOFOLLOW)
        .open(path)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn open_operator_preseed_file_nofollow(path: &Path) -> std::io::Result<fs::File> {
    fs::File::open(path)
}

/// Read and validate one canonical durable qualification file.
///
/// # Errors
///
/// Rejects non-regular, symlinked, oversized, non-canonical, or invalid receipts.
pub fn read_operator_preseed_store_receipt(
    path: &Path,
) -> Result<(OperatorPreseedSessionReceiptV1, Vec<u8>), String> {
    let before = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "failed to inspect operator-preseed qualification {}: {error}",
            path.display()
        )
    })?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err(format!(
            "operator-preseed qualification {} must be one direct regular file",
            path.display()
        ));
    }
    validate_private_receipt_file_custody(path, &before)?;
    if before.len() == 0 || before.len() > OPERATOR_PRESEED_STORE_RECEIPT_MAX_BYTES_V1 {
        return Err(format!(
            "operator-preseed qualification {} has invalid length {}",
            path.display(),
            before.len()
        ));
    }
    let mut file = open_operator_preseed_file_nofollow(path).map_err(|error| {
        format!(
            "failed to open operator-preseed qualification {} without following links: {error}",
            path.display()
        )
    })?;
    let opened = file.metadata().map_err(|error| {
        format!(
            "failed to inspect opened operator-preseed qualification {}: {error}",
            path.display()
        )
    })?;
    if !same_file_snapshot(&before, &opened) {
        return Err(format!(
            "operator-preseed qualification {} changed while it was opened",
            path.display()
        ));
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(OPERATOR_PRESEED_STORE_RECEIPT_MAX_BYTES_V1 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            format!(
                "failed to read operator-preseed qualification {}: {error}",
                path.display()
            )
        })?;
    if bytes.is_empty()
        || u64::try_from(bytes.len()).unwrap_or(u64::MAX)
            > OPERATOR_PRESEED_STORE_RECEIPT_MAX_BYTES_V1
    {
        return Err(format!(
            "operator-preseed qualification {} exceeds its V1 byte limit",
            path.display()
        ));
    }
    let after = fs::symlink_metadata(path).map_err(|error| {
        format!(
            "failed to re-inspect operator-preseed qualification {}: {error}",
            path.display()
        )
    })?;
    let after_open = file.metadata().map_err(|error| {
        format!(
            "failed to re-inspect opened operator-preseed qualification {}: {error}",
            path.display()
        )
    })?;
    if !same_file_snapshot(&before, &after)
        || !same_file_snapshot(&opened, &after_open)
        || after.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(format!(
            "operator-preseed qualification {} changed while it was read",
            path.display()
        ));
    }
    let receipt = decode_canonical_operator_preseed_receipt(&bytes)?;
    let expected_name = format!("{}.json", operator_preseed_receipt_digest_hex(&bytes));
    if path.file_name().and_then(|name| name.to_str()) != Some(expected_name.as_str()) {
        return Err(format!(
            "operator-preseed qualification {} is not named by its exact BLAKE3 digest",
            path.display()
        ));
    }
    Ok((receipt, bytes))
}

/// Reconcile a helper crash that left an owner-private qualification staging file.
///
/// A canonical staging file is exclusively renamed to its exact content-addressed final path, or
/// removed when that exact final qualification is already durable.
///
/// # Errors
///
/// Rejects ambiguous staging custody, link counts, bytes, or final-file identity.
pub fn recover_operator_preseed_store_receipt_staging(store_root: &Path) -> Result<(), String> {
    recover_operator_preseed_store_receipt_staging_with_hook(store_root, |_| Ok(()))
}

fn recover_operator_preseed_store_receipt_staging_with_hook(
    store_root: &Path,
    mut after_stage_read: impl FnMut(&Path) -> Result<(), String>,
) -> Result<(), String> {
    let directory = operator_preseed_store_receipt_dir(store_root);
    let metadata = match fs::symlink_metadata(&directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(format!(
                "failed to inspect operator-preseed qualification directory {}: {error}",
                directory.display()
            ));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "operator-preseed qualification directory {} must be one direct directory",
            directory.display()
        ));
    }
    validate_qualification_directory_custody(&directory, &metadata)?;
    let directory_handle = fs::File::open(&directory).map_err(|error| {
        format!(
            "failed to retain operator-preseed qualification directory {}: {error}",
            directory.display()
        )
    })?;
    if !same_file_snapshot(
        &metadata,
        &directory_handle.metadata().map_err(|error| {
            format!("failed to inspect retained operator-preseed qualification directory: {error}")
        })?,
    ) {
        return Err(
            "operator-preseed qualification directory changed while it was retained".to_owned(),
        );
    }
    let mut recovered = false;
    let mut entry_count = 0_usize;
    for entry in fs::read_dir(&directory).map_err(|error| {
        format!(
            "failed to scan operator-preseed qualification directory {}: {error}",
            directory.display()
        )
    })? {
        entry_count = entry_count
            .checked_add(1)
            .ok_or_else(|| "operator-preseed qualification entry count overflow".to_owned())?;
        if entry_count
            > OPERATOR_PRESEED_STORE_RECEIPT_MAX_COUNT_V1
                .saturating_add(OPERATOR_PRESEED_SESSION_MAX_STORES_V1)
        {
            return Err(
                "operator-preseed qualification directory exceeds its V1 entry bound".to_owned(),
            );
        }
        let entry = entry.map_err(|error| {
            format!("failed to inspect operator-preseed staging entry: {error}")
        })?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            "operator-preseed qualification directory contains a non-UTF-8 entry".to_owned()
        })?;
        if !name.starts_with(".qualification.") || !name.ends_with(".tmp") {
            continue;
        }
        let path = entry.path();
        let stage_metadata = fs::symlink_metadata(&path).map_err(|error| {
            format!(
                "failed to inspect operator-preseed staging file {}: {error}",
                path.display()
            )
        })?;
        if stage_metadata.file_type().is_symlink() || !stage_metadata.is_file() {
            return Err(format!(
                "operator-preseed staging entry {} must be one direct regular file",
                path.display()
            ));
        }
        #[cfg(unix)]
        {
            let effective_uid = unsafe { geteuid() };
            if stage_metadata.uid() != effective_uid
                || stage_metadata.mode() & 0o077 != 0
                || stage_metadata.nlink() != 1
            {
                return Err(format!(
                    "operator-preseed staging file {} must be owned by UID {effective_uid}, owner-only, and have exactly one hard link",
                    path.display()
                ));
            }
        }
        if stage_metadata.len() == 0
            || stage_metadata.len() > OPERATOR_PRESEED_STORE_RECEIPT_MAX_BYTES_V1
        {
            return Err(format!(
                "operator-preseed staging file {} has invalid length",
                path.display()
            ));
        }
        let mut stage_file = open_operator_preseed_file_nofollow(&path).map_err(|error| {
            format!(
                "failed to open operator-preseed staging file {} without following links: {error}",
                path.display()
            )
        })?;
        let opened_metadata = stage_file.metadata().map_err(|error| {
            format!(
                "failed to inspect opened operator-preseed staging file {}: {error}",
                path.display()
            )
        })?;
        validate_private_receipt_file_custody(&path, &opened_metadata)?;
        if !same_file_snapshot(&stage_metadata, &opened_metadata) {
            return Err(format!(
                "operator-preseed staging file {} changed while it was opened",
                path.display()
            ));
        }
        let mut bytes = Vec::new();
        stage_file
            .by_ref()
            .take(OPERATOR_PRESEED_STORE_RECEIPT_MAX_BYTES_V1 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| {
                format!(
                    "failed to read bounded operator-preseed staging file {}: {error}",
                    path.display()
                )
            })?;
        if bytes.is_empty()
            || u64::try_from(bytes.len()).unwrap_or(u64::MAX)
                > OPERATOR_PRESEED_STORE_RECEIPT_MAX_BYTES_V1
        {
            return Err(format!(
                "operator-preseed staging file {} exceeds its V1 byte limit",
                path.display()
            ));
        }
        let read_handle_metadata = stage_file.metadata().map_err(|error| {
            format!(
                "failed to re-inspect opened operator-preseed staging file {}: {error}",
                path.display()
            )
        })?;
        let read_path_metadata = fs::symlink_metadata(&path).map_err(|error| {
            format!(
                "failed to re-inspect operator-preseed staging path {}: {error}",
                path.display()
            )
        })?;
        validate_private_receipt_file_custody(&path, &read_handle_metadata)?;
        validate_private_receipt_file_custody(&path, &read_path_metadata)?;
        if !same_file_snapshot(&opened_metadata, &read_handle_metadata)
            || !same_file_snapshot(&opened_metadata, &read_path_metadata)
            || read_handle_metadata.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        {
            return Err(format!(
                "operator-preseed staging file {} changed while it was read",
                path.display()
            ));
        }
        decode_canonical_operator_preseed_receipt(&bytes)?;
        after_stage_read(&path)?;
        let install_path_metadata = fs::symlink_metadata(&path).map_err(|error| {
            format!(
                "failed to inspect operator-preseed staging path {} before installation: {error}",
                path.display()
            )
        })?;
        let install_handle_metadata = stage_file.metadata().map_err(|error| {
            format!(
                "failed to inspect opened operator-preseed staging file {} before installation: {error}",
                path.display()
            )
        })?;
        validate_private_receipt_file_custody(&path, &install_path_metadata)?;
        validate_private_receipt_file_custody(&path, &install_handle_metadata)?;
        if !same_file_snapshot(&opened_metadata, &install_path_metadata)
            || !same_file_snapshot(&opened_metadata, &install_handle_metadata)
            || install_handle_metadata.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        {
            return Err(format!(
                "operator-preseed staging file {} changed before installation",
                path.display()
            ));
        }
        let destination = operator_preseed_store_receipt_path(store_root, &bytes);
        let exact_exists = preflight_operator_preseed_store_receipt(store_root, &bytes)?;
        if exact_exists {
            let (_, installed_bytes) = read_operator_preseed_store_receipt(&destination)?;
            if installed_bytes != bytes {
                return Err(format!(
                    "operator-preseed staging file {} conflicts with its final qualification",
                    path.display()
                ));
            }
            fs::remove_file(&path).map_err(|error| {
                format!(
                    "failed to remove recovered operator-preseed staging file {}: {error}",
                    path.display()
                )
            })?;
        } else {
            install_operator_preseed_store_receipt_staging(&path, &destination).map_err(
                |error| {
                    format!(
                        "failed to recover operator-preseed staging file {} to {} without replacement: {error}",
                        path.display(),
                        destination.display()
                    )
                },
            )?;
            let (_, installed_bytes) =
                read_operator_preseed_store_receipt(&destination).map_err(|error| {
                    format!(
                        "failed to verify recovered operator-preseed qualification {}: {error}",
                        destination.display()
                    )
                })?;
            if installed_bytes != bytes {
                return Err(format!(
                    "recovered operator-preseed qualification {} differs from the exact decoded staging bytes",
                    destination.display()
                ));
            }
        }
        recovered = true;
    }
    if recovered {
        directory_handle.sync_all().map_err(|error| {
            format!(
                "failed to synchronize recovered operator-preseed qualification directory {}: {error}",
                directory.display()
            )
        })?;
        let current = fs::symlink_metadata(&directory).map_err(|error| {
            format!(
                "failed to recheck recovered operator-preseed qualification directory {}: {error}",
                directory.display()
            )
        })?;
        if !same_file_snapshot(
            &current,
            &directory_handle.metadata().map_err(|error| {
                format!(
                    "failed to recheck retained operator-preseed qualification directory: {error}"
                )
            })?,
        ) {
            return Err(
                "operator-preseed qualification directory changed during crash recovery".to_owned(),
            );
        }
    }
    Ok(())
}

fn validate_qualification_directory_custody(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), String> {
    #[cfg(unix)]
    {
        let effective_uid = unsafe { geteuid() };
        if metadata.uid() != effective_uid || metadata.mode() & 0o077 != 0 {
            return Err(format!(
                "operator-preseed qualification directory {} must be owned by UID {effective_uid} and owner-only",
                path.display()
            ));
        }
    }
    Ok(())
}

fn validate_private_receipt_file_custody(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), String> {
    #[cfg(unix)]
    {
        let effective_uid = unsafe { geteuid() };
        if metadata.uid() != effective_uid || metadata.mode() & 0o077 != 0 || metadata.nlink() != 1
        {
            return Err(format!(
                "operator-preseed qualification {} must be owned by UID {effective_uid}, owner-only, and have exactly one hard link",
                path.display()
            ));
        }
    }
    Ok(())
}

fn qualification_paths(store_root: &Path, allow_missing: bool) -> Result<Vec<PathBuf>, String> {
    let directory = operator_preseed_store_receipt_dir(store_root);
    let metadata = match fs::symlink_metadata(&directory) {
        Ok(metadata) => metadata,
        Err(error) if allow_missing && error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Vec::new());
        }
        Err(error) => {
            return Err(format!(
                "failed to inspect operator-preseed qualification directory {}: {error}",
                directory.display()
            ));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "operator-preseed qualification directory {} must be one direct directory",
            directory.display()
        ));
    }
    validate_qualification_directory_custody(&directory, &metadata)?;
    let entries = fs::read_dir(&directory).map_err(|error| {
        format!(
            "failed to read operator-preseed qualification directory {}: {error}",
            directory.display()
        )
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!("failed to read operator-preseed qualification entry: {error}")
        })?;
        let name = entry.file_name();
        let name = name.to_str().ok_or_else(|| {
            "operator-preseed qualification directory contains a non-UTF-8 entry".to_owned()
        })?;
        if name.starts_with(".qualification.") && name.ends_with(".tmp") {
            continue;
        }
        paths.push(entry.path());
    }
    paths.sort();
    if paths.len() > OPERATOR_PRESEED_STORE_RECEIPT_MAX_COUNT_V1 {
        return Err(format!(
            "operator-preseed store has {} qualifications, exceeding the V1 limit of {OPERATOR_PRESEED_STORE_RECEIPT_MAX_COUNT_V1}",
            paths.len()
        ));
    }
    Ok(paths)
}

/// Check whether an exact qualification can be installed without exceeding the V1 bound.
///
/// Returns `true` when the exact content-addressed receipt is already installed. Every retained
/// entry is validated before this function authorizes a new append.
///
/// # Errors
///
/// Rejects invalid custody or contents, a conflicting exact path, or a full qualification set.
pub fn preflight_operator_preseed_store_receipt(
    store_root: &Path,
    receipt_bytes: &[u8],
) -> Result<bool, String> {
    if receipt_bytes.is_empty()
        || u64::try_from(receipt_bytes.len()).unwrap_or(u64::MAX)
            > OPERATOR_PRESEED_STORE_RECEIPT_MAX_BYTES_V1
    {
        return Err("operator-preseed qualification exceeds its V1 byte limit".to_owned());
    }
    let expected: OperatorPreseedSessionReceiptV1 = norito::json::from_slice(receipt_bytes)
        .map_err(|error| format!("failed to decode expected preseed qualification: {error}"))?;
    expected
        .validate()
        .map_err(|error| format!("invalid expected preseed qualification: {error}"))?;
    if norito::json::to_vec(&expected)
        .map_err(|error| format!("failed to re-encode expected preseed qualification: {error}"))?
        != receipt_bytes
    {
        return Err("expected operator-preseed qualification is not canonical JSON".to_owned());
    }
    let destination = operator_preseed_store_receipt_path(store_root, receipt_bytes);
    let paths = qualification_paths(store_root, true)?;
    let mut exact_exists = false;
    for path in &paths {
        let (installed, installed_bytes) = read_operator_preseed_store_receipt(path)?;
        if path == &destination {
            if installed != expected || installed_bytes != receipt_bytes {
                return Err(format!(
                    "content-addressed operator-preseed qualification {} has conflicting bytes",
                    destination.display()
                ));
            }
            exact_exists = true;
        }
    }
    if !exact_exists && paths.len() == OPERATOR_PRESEED_STORE_RECEIPT_MAX_COUNT_V1 {
        return Err(format!(
            "operator-preseed store already retains the V1 maximum of {OPERATOR_PRESEED_STORE_RECEIPT_MAX_COUNT_V1} qualifications"
        ));
    }
    Ok(exact_exists)
}

/// Load every retained content-addressed qualification from one store.
///
/// # Errors
///
/// Rejects a missing/non-canonical directory, an unbounded entry set, or any non-receipt entry.
pub fn read_operator_preseed_store_receipts(
    store_root: &Path,
) -> Result<Vec<(OperatorPreseedSessionReceiptV1, Vec<u8>)>, String> {
    let paths = qualification_paths(store_root, false)?;
    if paths.is_empty() {
        return Err("operator-preseed store has no durable qualifications".to_owned());
    }
    paths
        .iter()
        .map(|path| read_operator_preseed_store_receipt(path))
        .collect()
}

/// Validate every retained qualification against one opened store and local host identity.
///
/// `StorageBackend::new` has already rebuilt its PoR/PDP trees from verified chunk bytes. This
/// check additionally binds every receipt to the exact store root and local validator. Retained
/// receipts are immutable history: old peer, capacity, and retired-artifact records do not block
/// startup. At least one current-peer/current-capacity receipt must still bind a complete exact
/// artifact set before Inrou may advertise the host.
///
/// # Errors
///
/// Rejects malformed history, a foreign store/validator receipt, or the absence of any complete
/// current-peer/current-capacity qualification.
pub fn validate_operator_preseed_store_receipts(
    backend: &StorageBackend,
    configured_max_capacity_bytes: u64,
    local_validator_account_id: &str,
    local_peer_id: &str,
) -> Result<BTreeSet<[u8; 32]>, String> {
    let canonical_root = fs::canonicalize(backend.root_dir()).map_err(|error| {
        format!(
            "failed to canonicalize opened operator-preseed store {}: {error}",
            backend.root_dir().display()
        )
    })?;
    if canonical_root != backend.root_dir() {
        return Err(format!(
            "opened operator-preseed store root {} is not canonical",
            backend.root_dir().display()
        ));
    }
    let canonical_root_text = canonical_root
        .to_str()
        .ok_or_else(|| "operator-preseed store root must be valid UTF-8".to_owned())?;
    recover_operator_preseed_store_receipt_staging(&canonical_root)?;
    let qualifications = read_operator_preseed_store_receipts(&canonical_root)?;
    let mut qualified_manifest_digests = BTreeSet::new();
    let mut complete_current_receipts = 0_usize;
    let mut current_candidates = 0_usize;
    let mut candidate_errors = Vec::new();
    for (receipt, _) in qualifications {
        if receipt.schema_version != OPERATOR_PRESEED_SESSION_RECEIPT_VERSION_V1
            || receipt.mode != "ingest"
        {
            return Err(
                "durable operator-preseed qualification must be the sole V1 ingest shape"
                    .to_owned(),
            );
        }
        let mut local_matches = receipt.targets.iter().filter(|target| {
            target.store_root == canonical_root_text
                && target.validator_account_id == local_validator_account_id
        });
        let Some(local_target) = local_matches.next() else {
            return Err(
                "operator-preseed qualification does not bind this exact store root and local validator identity"
                    .to_owned(),
            );
        };
        if local_matches.next().is_some() {
            return Err(
                "operator-preseed qualification binds the local store and validator more than once"
                    .to_owned(),
            );
        }
        if local_target.peer_id != local_peer_id
            || receipt.max_capacity_bytes != configured_max_capacity_bytes
        {
            continue;
        }
        current_candidates += 1;
        let exact_artifacts =
            receipt
                .artifacts
                .iter()
                .try_fold(BTreeSet::new(), |mut manifest_digests, artifact| {
                    let digest = hex::decode(&artifact.manifest_digest_blake3).map_err(|_| {
                        "operator-preseed manifest digest is not lowercase hex".to_owned()
                    })?;
                    let digest: [u8; 32] = digest.try_into().map_err(|_| {
                        "operator-preseed manifest digest is not 32 bytes".to_owned()
                    })?;
                    let stored = backend.manifest_by_digest(&digest).ok_or_else(|| {
                        format!(
                            "operator-preseed store is missing receipted manifest {}",
                            artifact.manifest_digest_blake3
                        )
                    })?;
                    if hex::encode(stored.payload_digest()) != artifact.payload_digest_blake3
                        || stored.content_length() != artifact.content_length
                    {
                        return Err(format!(
                            "operator-preseed store artifact {} differs from its qualification",
                            artifact.manifest_digest_blake3
                        ));
                    }
                    manifest_digests.insert(digest);
                    Ok::<_, String>(manifest_digests)
                });
        match exact_artifacts {
            Ok(manifest_digests) => {
                complete_current_receipts += 1;
                qualified_manifest_digests.extend(manifest_digests);
            }
            Err(error) => candidate_errors.push(error),
        }
    }
    if complete_current_receipts == 0 {
        let detail = if current_candidates == 0 {
            "no retained qualification has the current peer and configured capacity".to_owned()
        } else {
            format!(
                "all {current_candidates} current qualification candidate(s) are incomplete: {}",
                candidate_errors.join("; ")
            )
        };
        return Err(format!(
            "no complete durable operator-preseed qualification is current for this host: {detail}"
        ));
    }
    if qualified_manifest_digests.is_empty() {
        return Err(
            "complete current operator-preseed qualifications must bind at least one manifest"
                .to_owned(),
        );
    }
    Ok(qualified_manifest_digests)
}

#[cfg(unix)]
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mode() == right.mode()
        && left.uid() == right.uid()
        && left.gid() == right.gid()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StorageConfig;
    use iroha_config::base::util::Bytes;
    use sorafs_car::{CarBuildPlan, CarWriter, compute_chunk_plan_digest_sha3, compute_por_root};
    use sorafs_manifest::{
        BLAKE3_256_MULTIHASH_CODE, DagCodecId, ManifestBuilder, PinPolicy, StorageClass,
        operator_preseed::{OperatorPreseedArtifactReceiptV1, OperatorPreseedTargetReceiptV1},
    };
    #[cfg(unix)]
    use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
    use std::{io, io::Write as _};

    const CAPACITY: u64 = 1024 * 1024;
    const VALIDATOR: &str = "validator-a";

    fn test_store(temp: &tempfile::TempDir) -> StorageBackend {
        let root = fs::canonicalize(temp.path()).expect("canonical test root");
        StorageBackend::new(
            StorageConfig::builder()
                .enabled(false)
                .data_dir(root.join("store"))
                .max_capacity_bytes(Bytes(CAPACITY))
                .build(),
        )
        .expect("open test operator-preseed store")
    }

    fn ingest_fixture(store: &StorageBackend, payload: &[u8]) -> OperatorPreseedArtifactReceiptV1 {
        let plan = CarBuildPlan::single_file(payload).expect("fixture CAR plan");
        let stats = CarWriter::new(&plan, payload)
            .expect("fixture CAR writer")
            .write_to(io::sink())
            .expect("fixture CAR stats");
        let manifest = ManifestBuilder::new()
            .root_cid(stats.root_cids[0].clone())
            .dag_codec(DagCodecId(stats.dag_codec))
            .chunking_from_profile(plan.chunk_profile, BLAKE3_256_MULTIHASH_CODE)
            .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
            .por_root(compute_por_root(payload, &plan).expect("fixture PoR root"))
            .content_length(plan.content_length)
            .car_digest(*stats.car_archive_digest.as_bytes())
            .car_size(stats.car_size)
            .pin_policy(PinPolicy {
                min_replicas: 1,
                storage_class: StorageClass::Hot,
                retention_epoch: u64::MAX,
            })
            .build()
            .expect("fixture manifest");
        let mut reader = payload;
        let manifest_id = store
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest fixture manifest");
        let stored = store
            .manifest(&manifest_id)
            .expect("stored fixture manifest");
        OperatorPreseedArtifactReceiptV1 {
            manifest_digest_blake3: hex::encode(stored.manifest_digest()),
            payload_digest_blake3: hex::encode(stored.payload_digest()),
            content_length: stored.content_length(),
            store_count: 1,
        }
    }

    fn receipt(
        store_root: &Path,
        peer: &str,
        capacity: u64,
        artifacts: Vec<OperatorPreseedArtifactReceiptV1>,
    ) -> OperatorPreseedSessionReceiptV1 {
        let receipt = OperatorPreseedSessionReceiptV1 {
            schema_version: OPERATOR_PRESEED_SESSION_RECEIPT_VERSION_V1,
            status: "ready".to_owned(),
            mode: "ingest".to_owned(),
            max_capacity_bytes: capacity,
            targets: vec![OperatorPreseedTargetReceiptV1 {
                validator_account_id: VALIDATOR.to_owned(),
                peer_id: peer.to_owned(),
                store_root: store_root
                    .to_str()
                    .expect("test store root is UTF-8")
                    .to_owned(),
            }],
            artifacts,
        };
        receipt.validate().expect("valid test receipt");
        receipt
    }

    fn retain_receipt(store_root: &Path, receipt: &OperatorPreseedSessionReceiptV1) -> Vec<u8> {
        let directory = operator_preseed_store_receipt_dir(store_root);
        fs::create_dir_all(&directory).expect("create qualification directory");
        #[cfg(unix)]
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .expect("set qualification directory mode");
        let bytes = norito::json::to_vec(receipt).expect("encode receipt");
        let path = operator_preseed_store_receipt_path(store_root, &bytes);
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        options
            .open(path)
            .and_then(|mut file| file.write_all(&bytes))
            .expect("retain test qualification");
        bytes
    }

    fn synthetic_artifact(seed: u32) -> OperatorPreseedArtifactReceiptV1 {
        let mut digest = [0_u8; 32];
        for chunk in digest.chunks_exact_mut(4) {
            chunk.copy_from_slice(&seed.to_be_bytes());
        }
        OperatorPreseedArtifactReceiptV1 {
            manifest_digest_blake3: hex::encode(digest),
            payload_digest_blake3: hex::encode([0xA5; 32]),
            content_length: 1,
            store_count: 1,
        }
    }

    #[test]
    fn startup_selects_complete_current_qualification_without_historical_bricking() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store = test_store(&temp);
        let artifact = ingest_fixture(&store, b"qualified current payload");
        let current = receipt(
            store.root_dir(),
            "peer-current",
            CAPACITY,
            vec![artifact.clone()],
        );
        retain_receipt(store.root_dir(), &current);

        let stale_capacity = receipt(
            store.root_dir(),
            "peer-current",
            CAPACITY * 2,
            vec![synthetic_artifact(1)],
        );
        retain_receipt(store.root_dir(), &stale_capacity);
        let stale_peer = receipt(
            store.root_dir(),
            "peer-retired",
            CAPACITY,
            vec![synthetic_artifact(2)],
        );
        retain_receipt(store.root_dir(), &stale_peer);
        let incomplete_current = receipt(
            store.root_dir(),
            "peer-current",
            CAPACITY,
            vec![synthetic_artifact(3)],
        );
        retain_receipt(store.root_dir(), &incomplete_current);

        let allowed =
            validate_operator_preseed_store_receipts(&store, CAPACITY, VALIDATOR, "peer-current")
                .expect("one complete exact current qualification admits startup");
        assert_eq!(
            allowed,
            BTreeSet::from([hex::decode(&artifact.manifest_digest_blake3)
                .expect("artifact digest hex")
                .try_into()
                .expect("32-byte artifact digest")])
        );
        assert!(
            validate_operator_preseed_store_receipts(
                &store,
                CAPACITY,
                VALIDATOR,
                "peer-without-qualification"
            )
            .expect_err("peer rotation requires one exact new qualification")
            .contains("no retained qualification has the current peer")
        );

        let rotated_peer = receipt(
            store.root_dir(),
            "peer-rotated",
            CAPACITY,
            vec![artifact.clone()],
        );
        retain_receipt(store.root_dir(), &rotated_peer);
        validate_operator_preseed_store_receipts(&store, CAPACITY, VALIDATOR, "peer-rotated")
            .expect("peer rotation succeeds after exact requalification");

        let rotated_capacity = receipt(
            store.root_dir(),
            "peer-rotated",
            CAPACITY * 2,
            vec![artifact],
        );
        retain_receipt(store.root_dir(), &rotated_capacity);
        validate_operator_preseed_store_receipts(&store, CAPACITY * 2, VALIDATOR, "peer-rotated")
            .expect("capacity rotation succeeds after exact requalification");
    }

    #[test]
    fn preflight_enforces_the_store_qualification_count_before_install() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store_root = fs::canonicalize(temp.path()).expect("canonical temp root");
        for index in 0..OPERATOR_PRESEED_STORE_RECEIPT_MAX_COUNT_V1 {
            let retained = receipt(
                &store_root,
                &format!("peer-{index:04}"),
                CAPACITY,
                vec![synthetic_artifact(
                    u32::try_from(index).expect("count fits u32"),
                )],
            );
            retain_receipt(&store_root, &retained);
        }
        let next = receipt(
            &store_root,
            "peer-overflow",
            CAPACITY,
            vec![synthetic_artifact(u32::MAX)],
        );
        let bytes = norito::json::to_vec(&next).expect("encode overflow receipt");
        assert!(
            preflight_operator_preseed_store_receipt(&store_root, &bytes)
                .expect_err("the 4097th qualification must fail before store mutation")
                .contains("maximum of 4096 qualifications")
        );
    }

    #[cfg(unix)]
    #[test]
    fn qualification_install_is_no_replace_and_same_parent_only() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().expect("temp dir");
        let store_root = fs::canonicalize(temp.path()).expect("canonical temp root");
        let exact = receipt(
            &store_root,
            "peer-current",
            CAPACITY,
            vec![synthetic_artifact(7)],
        );
        let bytes = norito::json::to_vec(&exact).expect("encode receipt");
        let directory = operator_preseed_store_receipt_dir(&store_root);
        fs::create_dir(&directory).expect("create qualification directory");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .expect("set qualification directory mode");
        let destination = operator_preseed_store_receipt_path(&store_root, &bytes);
        fs::write(&destination, b"existing").expect("write existing destination");
        fs::set_permissions(&destination, fs::Permissions::from_mode(0o600))
            .expect("set destination mode");
        let staging = directory.join(".qualification.1.0.tmp");
        fs::write(&staging, &bytes).expect("write staging receipt");
        fs::set_permissions(&staging, fs::Permissions::from_mode(0o600)).expect("set staging mode");
        assert!(install_operator_preseed_store_receipt_staging(&staging, &destination).is_err());
        assert_eq!(
            fs::read(&destination).expect("read destination"),
            b"existing"
        );
        assert!(staging.exists());

        let other = store_root
            .join("other")
            .join(destination.file_name().expect("file name"));
        fs::create_dir(other.parent().expect("other parent")).expect("create other parent");
        assert!(install_operator_preseed_store_receipt_staging(&staging, &other).is_err());

        let linked_store = store_root.join("linked-store");
        let real_directory = store_root.join("substituted-qualification-directory");
        fs::create_dir(&linked_store).expect("create linked store root");
        fs::create_dir(&real_directory).expect("create substituted qualification directory");
        fs::set_permissions(&real_directory, fs::Permissions::from_mode(0o700))
            .expect("set substituted qualification directory mode");
        let linked_directory = operator_preseed_store_receipt_dir(&linked_store);
        symlink(&real_directory, &linked_directory).expect("link qualification parent");
        let linked_staging = linked_directory.join(".qualification.3.0.tmp");
        fs::write(&linked_staging, &bytes).expect("write staging through linked parent");
        fs::set_permissions(&linked_staging, fs::Permissions::from_mode(0o600))
            .expect("set linked staging mode");
        let linked_destination = operator_preseed_store_receipt_path(&linked_store, &bytes);
        assert!(
            install_operator_preseed_store_receipt_staging(&linked_staging, &linked_destination)
                .is_err(),
            "exclusive installation must not follow a substituted parent symlink"
        );
    }

    #[cfg(unix)]
    #[test]
    fn qualification_custody_and_crash_stage_recovery_fail_closed() {
        let temp = tempfile::tempdir().expect("temp dir");
        let store_root = fs::canonicalize(temp.path()).expect("canonical temp root");
        let exact = receipt(
            &store_root,
            "peer-current",
            CAPACITY,
            vec![synthetic_artifact(9)],
        );
        let bytes = norito::json::to_vec(&exact).expect("encode receipt");
        let directory = operator_preseed_store_receipt_dir(&store_root);
        fs::create_dir(&directory).expect("create qualification directory");
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .expect("set qualification directory mode");
        let staging = directory.join(".qualification.2.0.tmp");
        fs::write(&staging, &bytes).expect("write crash staging receipt");
        fs::set_permissions(&staging, fs::Permissions::from_mode(0o600)).expect("set staging mode");
        recover_operator_preseed_store_receipt_staging(&store_root)
            .expect("recover exact crash staging receipt");
        assert!(!staging.exists());
        let destination = operator_preseed_store_receipt_path(&store_root, &bytes);
        assert_eq!(
            fs::read(&destination).expect("read recovered receipt"),
            bytes
        );

        let raced = receipt(
            &store_root,
            "peer-raced",
            CAPACITY,
            vec![synthetic_artifact(10)],
        );
        let raced_bytes = norito::json::to_vec(&raced).expect("encode raced receipt");
        let substituted = receipt(
            &store_root,
            "peer-substituted",
            CAPACITY,
            vec![synthetic_artifact(11)],
        );
        let substituted_bytes =
            norito::json::to_vec(&substituted).expect("encode substituted receipt");
        let raced_staging = directory.join(".qualification.3.0.tmp");
        fs::write(&raced_staging, &raced_bytes).expect("write raced staging receipt");
        fs::set_permissions(&raced_staging, fs::Permissions::from_mode(0o600))
            .expect("set raced staging mode");
        let error = recover_operator_preseed_store_receipt_staging_with_hook(&store_root, |path| {
            fs::remove_file(path).map_err(|error| error.to_string())?;
            fs::write(path, &substituted_bytes).map_err(|error| error.to_string())?;
            fs::set_permissions(path, fs::Permissions::from_mode(0o600))
                .map_err(|error| error.to_string())?;
            Ok(())
        })
        .expect_err("same-UID staging pathname substitution must fail closed");
        assert!(
            error.contains("changed before installation")
                || error.contains("exactly one hard link"),
            "{error}"
        );
        assert!(
            !operator_preseed_store_receipt_path(&store_root, &raced_bytes).exists(),
            "substituted staging bytes must not install under the decoded digest"
        );
        fs::remove_file(&raced_staging).expect("remove rejected substituted staging file");

        let oversized_staging = directory.join(".qualification.4.0.tmp");
        fs::write(
            &oversized_staging,
            vec![0_u8; OPERATOR_PRESEED_STORE_RECEIPT_MAX_BYTES_V1 as usize + 1],
        )
        .expect("write oversized staging receipt");
        fs::set_permissions(&oversized_staging, fs::Permissions::from_mode(0o600))
            .expect("set oversized staging mode");
        let error = recover_operator_preseed_store_receipt_staging(&store_root)
            .expect_err("oversized crash staging must fail before decoding");
        assert!(error.contains("invalid length"), "{error}");
        fs::remove_file(&oversized_staging).expect("remove rejected oversized staging file");

        fs::set_permissions(&directory, fs::Permissions::from_mode(0o755))
            .expect("weaken directory mode");
        assert!(
            read_operator_preseed_store_receipts(&store_root)
                .expect_err("non-owner-only qualification directory must fail")
                .contains("owner-only")
        );
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .expect("restore directory mode");
        let extra_link = store_root.join("qualification-hardlink");
        fs::hard_link(&destination, &extra_link).expect("hard-link qualification");
        assert!(
            read_operator_preseed_store_receipts(&store_root)
                .expect_err("multiply linked qualification must fail")
                .contains("exactly one hard link")
        );
    }
}

#[cfg(not(unix))]
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.is_file() == right.is_file()
        && left.len() == right.len()
        && left.permissions().readonly() == right.permissions().readonly()
}
