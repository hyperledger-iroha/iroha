//! Bounded readers and decoders for local startup trust-root artifacts.

use std::{
    fs,
    io::{self, Read as _},
    path::Path,
};

use error_stack::{Report, ResultExt as _};
use iroha_genesis::{GenesisBlock, RawGenesisTransaction, read_signed_genesis};

use super::{ConfigError, ReportResult, StartError};

/// Integrity-bound TOML is one flattened configuration source.
pub(super) const INTEGRITY_BOUND_CONFIG_MAX_BYTES_V1: usize =
    iroha_config::base::toml::MAX_TOML_SOURCE_BYTES as usize;

/// Read and decode the optional source manifest under fixed startup budgets.
pub(super) fn read_genesis_manifest(
    path: &Path,
) -> ReportResult<RawGenesisTransaction, StartError> {
    let bytes = iroha_genesis::read_genesis_manifest_bytes(path)
        .change_context(StartError::InitKura)
        .attach_with(|| format!("failed to read genesis manifest JSON at {}", path.display()))?;
    RawGenesisTransaction::from_json_slice(&bytes).map_err(|error| {
        Report::new(StartError::InitKura).attach(format!(
            "failed to parse genesis manifest JSON at {}: {error}",
            path.display()
        ))
    })
}

/// Read and decode one signed genesis artifact under fixed startup budgets.
pub(super) fn read_genesis_unlocked(path: &Path) -> ReportResult<GenesisBlock, ConfigError> {
    const PANIC_HELP: &str = concat!(
        "Genesis decode panicked. A common cause is an invalid `Name` (identifiers ",
        "must not contain whitespace or the characters `@`, `#`, `$`). ",
        "Please sanitize identifiers in your genesis and re-sign the file."
    );

    // Tests may call this helper without the ordinary daemon initialization.
    super::init_genesis_instruction_registry();
    super::init_query_registry();

    match read_signed_genesis(path) {
        Ok(genesis) => Ok(GenesisBlock(genesis)),
        Err(error) => {
            let error_chain = format!("{error:#}");
            let decode_panicked = error_chain.contains("panicked");
            let report = Report::new(ConfigError::ReadGenesis).attach(format!(
                "failed to read and decode signed genesis at {}: {error_chain}",
                path.display()
            ));
            if decode_panicked {
                Err(report.attach(PANIC_HELP))
            } else {
                Err(report)
            }
        }
    }
}

/// Read a stable direct regular file, retaining at most `max_bytes`.
pub(super) fn read_bounded_startup_artifact(
    path: &Path,
    max_bytes: usize,
    label: &str,
) -> io::Result<Vec<u8>> {
    let max_bytes_u64 = u64::try_from(max_bytes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} byte limit is not representable on this platform"),
        )
    })?;
    let named_before = fs::symlink_metadata(path)?;
    if startup_artifact_metadata_is_link(&named_before) || !named_before.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} must be a direct regular file"),
        ));
    }
    if named_before.len() > max_bytes_u64 {
        return Err(startup_artifact_too_large(label, max_bytes));
    }

    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;

        options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;

        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let mut file = options.open(path)?;
    let opened_before = file.metadata()?;
    if startup_artifact_metadata_is_link(&opened_before)
        || !opened_before.is_file()
        || !same_startup_artifact_snapshot(&named_before, &opened_before)
        || opened_before.len() > max_bytes_u64
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} changed identity or type while opening"),
        ));
    }

    let capacity = usize::try_from(opened_before.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} length cannot be addressed on this platform"),
        )
    })?;
    // Leave room for the max-plus-one sentinel so a raced max-sized file does
    // not force `Vec` to double before it can be rejected.
    let mut bytes = Vec::with_capacity(capacity.saturating_add(1));
    file.by_ref()
        .take(opened_before.len().saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(startup_artifact_too_large(label, max_bytes));
    }

    let opened_after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    if startup_artifact_metadata_is_link(&named_after)
        || !named_after.is_file()
        || bytes.len() != capacity
        || !same_startup_artifact_snapshot(&opened_before, &opened_after)
        || !same_startup_artifact_snapshot(&opened_after, &named_after)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} changed while it was being read"),
        ));
    }
    Ok(bytes)
}

fn startup_artifact_too_large(label: &str, max_bytes: usize) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{label} exceeds the {max_bytes}-byte first-release limit"),
    )
}

fn startup_artifact_metadata_is_link(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt as _;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}

#[cfg(unix)]
fn same_startup_artifact_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(windows)]
fn same_startup_artifact_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    left.volume_serial_number().is_some()
        && left.file_index().is_some()
        && left.volume_serial_number() == right.volume_serial_number()
        && left.file_index() == right.file_index()
        && left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
}

#[cfg(not(any(unix, windows)))]
fn same_startup_artifact_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_reader_accepts_exact_limit_and_rejects_sparse_overflow() {
        let directory = tempfile::tempdir().expect("create startup-artifact test directory");
        let exact = directory.path().join("exact.bin");
        fs::write(&exact, [0xA5; 32]).expect("write exact startup artifact");
        assert_eq!(
            read_bounded_startup_artifact(&exact, 32, "test artifact")
                .expect("read exact startup artifact"),
            vec![0xA5; 32]
        );

        let oversized = directory.path().join("oversized.bin");
        let file = fs::File::create(&oversized).expect("create sparse startup artifact");
        file.set_len(33).expect("extend sparse startup artifact");
        let error = read_bounded_startup_artifact(&oversized, 32, "test artifact")
            .expect_err("oversized startup artifact must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("32-byte"));
    }

    #[cfg(unix)]
    #[test]
    fn bounded_reader_rejects_final_component_symlink() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("create startup-artifact test directory");
        let target = directory.path().join("target.bin");
        let link = directory.path().join("link.bin");
        fs::write(&target, b"bounded").expect("write symlink target");
        symlink(&target, &link).expect("create startup-artifact symlink");

        let error = read_bounded_startup_artifact(&link, 32, "test artifact")
            .expect_err("startup artifact symlink must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }
}
