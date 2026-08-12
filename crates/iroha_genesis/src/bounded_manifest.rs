//! Bounded local readers for genesis source artifacts.

use std::{
    fs,
    io::{self, Read as _},
    path::Path,
};

use eyre::{Result, WrapErr as _, eyre};
use iroha_data_model::block::SignedBlock;
use iroha_version::Version as _;
use norito::DecodeLimits;

/// First-release encoded byte limit for a JSON genesis manifest.
pub const GENESIS_MANIFEST_JSON_MAX_BYTES_V1: usize = 16 * 1024 * 1024;
/// First-release byte limit for one compiled IVM program referenced by genesis.
pub const GENESIS_IVM_BYTECODE_MAX_BYTES_V1: usize =
    iroha_config::parameters::defaults::transaction::ivm_bytecode_size().get() as usize;
/// Maximum JSON values, object keys, and containers admitted before parsing.
pub const GENESIS_MANIFEST_JSON_MAX_TOKENS_V1: usize = 262_144;
/// Maximum encoded byte length of one JSON string literal.
pub const GENESIS_MANIFEST_JSON_MAX_STRING_BYTES_V1: usize = 1024 * 1024;
/// Maximum JSON object/array nesting depth admitted before parsing.
pub const GENESIS_MANIFEST_JSON_MAX_DEPTH_V1: usize = 64;
/// First-release encoded byte limit for one canonical signed-genesis body.
pub const SIGNED_GENESIS_MAX_BYTES_V1: usize = 64 * 1024 * 1024;
/// Aggregate compiled IVM bytecode retained while expanding one genesis manifest.
pub const GENESIS_IVM_BYTECODE_MAX_TOTAL_BYTES_V1: usize = SIGNED_GENESIS_MAX_BYTES_V1;

const SIGNED_GENESIS_MAX_SEQUENCE_ELEMENTS_V1: usize = 1_048_576;
const SIGNED_GENESIS_MAX_TOTAL_ELEMENTS_V1: usize = 4_194_304;
const SIGNED_GENESIS_MAX_ALLOCATED_BYTES_V1: usize = 2 * SIGNED_GENESIS_MAX_BYTES_V1;
const SIGNED_GENESIS_MAX_NESTING_DEPTH_V1: usize = 64;

#[cfg(any(target_os = "linux", target_os = "android"))]
const GENESIS_O_NOFOLLOW_FLAG: i32 = 0x0002_0000;
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
const GENESIS_O_NOFOLLOW_FLAG: i32 = 0x0000_0100;
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))
))]
compile_error!("genesis artifact loading requires a defined no-follow open flag on this target");

/// Read one stable direct genesis-manifest file under the V1 byte and lexical budgets.
///
/// # Errors
///
/// Returns an I/O error when the path is not a stable direct regular file, the
/// body exceeds a first-release resource limit, or the body is not UTF-8.
pub fn read_genesis_manifest_bytes(path: &Path) -> io::Result<Vec<u8>> {
    let bytes = read_bounded_regular_file(
        path,
        GENESIS_MANIFEST_JSON_MAX_BYTES_V1,
        "genesis manifest JSON",
    )?;
    validate_genesis_manifest_json(&bytes)?;
    Ok(bytes)
}

/// Validate the byte, token, string, and nesting budgets for in-memory genesis JSON.
///
/// This preflight must run before constructing a generic JSON tree from an
/// operator-provided manifest.
///
/// # Errors
///
/// Returns an error when the input is not UTF-8 or exceeds a V1 resource bound.
pub fn validate_genesis_manifest_json(bytes: &[u8]) -> io::Result<&str> {
    if bytes.len() > GENESIS_MANIFEST_JSON_MAX_BYTES_V1 {
        return Err(genesis_artifact_too_large(
            "genesis manifest JSON",
            GENESIS_MANIFEST_JSON_MAX_BYTES_V1,
        ));
    }
    let source = std::str::from_utf8(bytes).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("genesis manifest JSON is not UTF-8: {error}"),
        )
    })?;
    validate_json_lexical_budget(
        source,
        GENESIS_MANIFEST_JSON_MAX_TOKENS_V1,
        GENESIS_MANIFEST_JSON_MAX_STRING_BYTES_V1,
        GENESIS_MANIFEST_JSON_MAX_DEPTH_V1,
    )?;
    Ok(source)
}

/// Read one stable direct signed-genesis file under the V1 encoded-byte limit.
///
/// # Errors
///
/// Returns an I/O error when the path is not a stable direct regular file or
/// when its body exceeds the signed-genesis byte limit.
pub fn read_signed_genesis_bytes(path: &Path) -> io::Result<Vec<u8>> {
    read_bounded_regular_file(path, SIGNED_GENESIS_MAX_BYTES_V1, "signed genesis artifact")
}

/// Decode one in-memory signed-genesis body under fixed Norito resource limits.
///
/// # Errors
///
/// Returns an error when the body is empty, exceeds the V1 byte ceiling, is not
/// a framed signed block, exceeds a decode resource budget, or decoding panics.
pub fn decode_signed_genesis(bytes: &[u8]) -> Result<SignedBlock> {
    validate_signed_genesis_size(bytes.len())?;
    let (&version, framed) = bytes
        .split_first()
        .ok_or_else(|| eyre!("signed genesis body is empty"))?;
    if !SignedBlock::supported_versions().contains(&version) {
        return Err(eyre!("unsupported signed genesis version {version}"));
    }

    crate::init_instruction_registry();
    let decoded = std::panic::catch_unwind(|| {
        norito::with_decode_limits(signed_genesis_decode_limits_v1(), || {
            let view = norito::core::from_bytes_view(framed)?;
            if view.flags() != norito::default_encode_flags() {
                return Err(norito::Error::UnsupportedFeature(
                    "non-canonical signed block wire layout",
                ));
            }
            view.decode::<SignedBlock>()
        })
    });
    match decoded {
        Ok(Ok(block)) => Ok(block),
        Ok(Err(error)) => Err(eyre!("decode canonical signed genesis body: {error}")),
        Err(_) => Err(eyre!("decode canonical signed genesis body panicked")),
    }
}

fn validate_signed_genesis_size(length: usize) -> Result<()> {
    if length == 0 {
        return Err(eyre!("signed genesis body is empty"));
    }
    if length > SIGNED_GENESIS_MAX_BYTES_V1 {
        return Err(eyre!(
            "signed genesis artifact exceeds the {}-byte first-release limit",
            SIGNED_GENESIS_MAX_BYTES_V1
        ));
    }
    Ok(())
}

/// Read and decode one signed-genesis file under the fixed V1 resource budgets.
///
/// # Errors
///
/// Returns an error when the file cannot be read safely or its body is invalid.
pub fn read_signed_genesis(path: &Path) -> Result<SignedBlock> {
    let bytes = read_signed_genesis_bytes(path)
        .wrap_err_with(|| format!("read signed genesis artifact at {}", path.display()))?;
    decode_signed_genesis(&bytes)
        .wrap_err_with(|| format!("decode signed genesis artifact at {}", path.display()))
}

/// Return the fixed Norito resource budget used for V1 signed-genesis decoding.
#[must_use]
pub const fn signed_genesis_decode_limits_v1() -> DecodeLimits {
    DecodeLimits::new(
        SIGNED_GENESIS_MAX_SEQUENCE_ELEMENTS_V1,
        SIGNED_GENESIS_MAX_BYTES_V1,
        SIGNED_GENESIS_MAX_TOTAL_ELEMENTS_V1,
        SIGNED_GENESIS_MAX_ALLOCATED_BYTES_V1,
        SIGNED_GENESIS_MAX_NESTING_DEPTH_V1,
    )
}

impl crate::RawGenesisTransaction {
    /// Decode one in-memory JSON manifest after enforcing the same byte,
    /// token, string, and nesting budgets as [`Self::from_path`].
    ///
    /// # Errors
    ///
    /// Returns an error when the input exceeds a first-release resource bound,
    /// is malformed, contains an unknown field, or has no transaction entry.
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self> {
        crate::init_instruction_registry();
        let source = validate_genesis_manifest_json(bytes)
            .wrap_err("validate genesis manifest JSON resource bounds")?;
        let raw_value: norito::json::Value =
            norito::json::from_str(source).wrap_err("parse genesis manifest JSON")?;
        let value = Self::from_json_value(raw_value)
            .map_err(|error| eyre!("decode genesis manifest JSON: {error}"))?;
        if value.transactions.is_empty() {
            return Err(eyre!(
                "genesis manifest must include at least one transaction entry"
            ));
        }
        Ok(value)
    }
}

pub(super) fn read_genesis_ivm_bytecode(
    path: &Path,
    remaining_total_bytes: usize,
) -> io::Result<Vec<u8>> {
    if remaining_total_bytes == 0 {
        return Err(genesis_artifact_too_large(
            "aggregate genesis IVM bytecode",
            GENESIS_IVM_BYTECODE_MAX_TOTAL_BYTES_V1,
        ));
    }
    read_bounded_regular_file(
        path,
        GENESIS_IVM_BYTECODE_MAX_BYTES_V1.min(remaining_total_bytes),
        "genesis IVM bytecode",
    )
}

fn read_bounded_regular_file(path: &Path, max_bytes: usize, label: &str) -> io::Result<Vec<u8>> {
    let max_bytes_u64 = u64::try_from(max_bytes).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} byte limit is not representable on this platform"),
        )
    })?;
    let named_before = fs::symlink_metadata(path)?;
    if genesis_metadata_is_link(&named_before) || !named_before.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} must be a direct regular file"),
        ));
    }
    if named_before.len() > max_bytes_u64 {
        return Err(genesis_artifact_too_large(label, max_bytes));
    }

    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;

        options.custom_flags(GENESIS_O_NOFOLLOW_FLAG);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;

        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let mut file = options.open(path)?;
    let opened_before = file.metadata()?;
    if genesis_metadata_is_link(&opened_before)
        || !opened_before.is_file()
        || !same_genesis_artifact_snapshot(&named_before, &opened_before)
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
    // Reserve the max-plus-one sentinel up front. If a max-sized file grows
    // while open, `read_to_end` must not double the entire buffer merely to
    // retain the one byte needed to reject it.
    let mut bytes = Vec::with_capacity(capacity.saturating_add(1));
    file.by_ref()
        .take(opened_before.len().saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > max_bytes {
        return Err(genesis_artifact_too_large(label, max_bytes));
    }

    let opened_after = file.metadata()?;
    let named_after = fs::symlink_metadata(path)?;
    if genesis_metadata_is_link(&named_after)
        || !named_after.is_file()
        || bytes.len() != capacity
        || !same_genesis_artifact_snapshot(&opened_before, &opened_after)
        || !same_genesis_artifact_snapshot(&opened_after, &named_after)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} changed while it was being read"),
        ));
    }
    Ok(bytes)
}

fn genesis_artifact_too_large(label: &str, max_bytes: usize) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{label} exceeds the {max_bytes}-byte first-release limit"),
    )
}

fn validate_json_lexical_budget(
    source: &str,
    max_tokens: usize,
    max_string_bytes: usize,
    max_depth: usize,
) -> io::Result<()> {
    let mut tokens = 0usize;
    let mut depth = 0usize;
    let mut string_start = None;
    let mut escaped = false;
    let mut in_scalar = false;

    for (index, byte) in source.bytes().enumerate() {
        if let Some(start) = string_start {
            let closes_string = !escaped && byte == b'"';
            if !closes_string {
                let string_bytes = index.saturating_sub(start).saturating_add(1);
                if string_bytes > max_string_bytes {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "genesis manifest JSON string is at least {string_bytes} bytes (maximum {max_string_bytes})"
                        ),
                    ));
                }
            }
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if closes_string {
                string_start = None;
            }
            continue;
        }

        match byte {
            b'"' => {
                count_json_token(&mut tokens, max_tokens)?;
                string_start = Some(index.saturating_add(1));
                in_scalar = false;
            }
            b'{' | b'[' => {
                count_json_token(&mut tokens, max_tokens)?;
                depth = depth.checked_add(1).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "genesis JSON depth overflowed")
                })?;
                if depth > max_depth {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "genesis manifest JSON nesting depth {depth} exceeds maximum {max_depth}"
                        ),
                    ));
                }
                in_scalar = false;
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                in_scalar = false;
            }
            b',' | b':' | b' ' | b'\t' | b'\r' | b'\n' => in_scalar = false,
            _ if !in_scalar => {
                count_json_token(&mut tokens, max_tokens)?;
                in_scalar = true;
            }
            _ => {}
        }
    }
    Ok(())
}

fn count_json_token(tokens: &mut usize, max_tokens: usize) -> io::Result<()> {
    *tokens = tokens.checked_add(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "genesis manifest JSON token count overflowed",
        )
    })?;
    if *tokens > max_tokens {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("genesis manifest JSON token count exceeds first-release maximum {max_tokens}"),
        ));
    }
    Ok(())
}

fn genesis_metadata_is_link(metadata: &fs::Metadata) -> bool {
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
fn same_genesis_artifact_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
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
fn same_genesis_artifact_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;

    left.volume_serial_number().is_some()
        && left.file_index().is_some()
        && left.volume_serial_number() == right.volume_serial_number()
        && left.file_index() == right.file_index()
        && left.file_size() == right.file_size()
        && left.last_write_time() == right.last_write_time()
}

#[cfg(not(any(unix, windows)))]
fn same_genesis_artifact_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.len() == right.len() && left.modified().ok() == right.modified().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_reader_accepts_exact_limit_and_rejects_sparse_overflow() {
        let directory = tempfile::tempdir().expect("create genesis-artifact test directory");
        let exact = directory.path().join("exact.bin");
        fs::write(&exact, [0x5A; 32]).expect("write exact genesis artifact");
        assert_eq!(
            read_bounded_regular_file(&exact, 32, "test genesis artifact")
                .expect("read exact genesis artifact"),
            vec![0x5A; 32]
        );

        let oversized = directory.path().join("oversized.bin");
        let file = fs::File::create(&oversized).expect("create sparse genesis artifact");
        file.set_len(33).expect("extend sparse genesis artifact");
        let error = read_bounded_regular_file(&oversized, 32, "test genesis artifact")
            .expect_err("oversized genesis artifact must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("32-byte"));
    }

    #[cfg(unix)]
    #[test]
    fn bounded_reader_rejects_final_component_symlink() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().expect("create genesis-artifact test directory");
        let target = directory.path().join("target.bin");
        let link = directory.path().join("link.bin");
        fs::write(&target, b"bounded").expect("write symlink target");
        symlink(&target, &link).expect("create genesis-artifact symlink");

        let error = read_bounded_regular_file(&link, 32, "test genesis artifact")
            .expect_err("genesis artifact symlink must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn ivm_limit_matches_transaction_admission_default() {
        assert_eq!(
            GENESIS_IVM_BYTECODE_MAX_BYTES_V1 as u64,
            iroha_config::parameters::defaults::transaction::ivm_bytecode_size().get()
        );
    }

    #[test]
    fn ivm_reader_enforces_remaining_aggregate_budget() {
        let directory = tempfile::tempdir().expect("create IVM-bytecode test directory");
        let path = directory.path().join("trigger.to");
        fs::write(&path, [0x5A; 33]).expect("write IVM bytecode fixture");

        let error = read_genesis_ivm_bytecode(&path, 32)
            .expect_err("bytecode beyond the remaining aggregate budget must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("32-byte"));
    }

    #[test]
    fn signed_genesis_size_rejects_empty_and_first_overflow() {
        assert!(validate_signed_genesis_size(0).is_err());
        assert!(validate_signed_genesis_size(SIGNED_GENESIS_MAX_BYTES_V1).is_ok());
        assert!(validate_signed_genesis_size(SIGNED_GENESIS_MAX_BYTES_V1 + 1).is_err());
    }

    #[test]
    fn signed_genesis_rejects_unsupported_version_without_retaining_raw_error_bytes() {
        let error = decode_signed_genesis(&[u8::MAX, 0])
            .expect_err("unsupported outer version must fail before framed decoding");
        assert!(
            error
                .to_string()
                .contains("unsupported signed genesis version")
        );
    }

    #[test]
    fn signed_genesis_decoder_roundtrips_canonical_wire() {
        let manifest = crate::GenesisBuilder::new_without_executor(
            "bounded-signed-genesis"
                .parse()
                .expect("fixture chain id is canonical"),
            ".",
        )
        .build_raw()
        .with_consensus_meta();
        let block = manifest
            .build_and_sign(&crate::checked_genesis_fixture_keypair())
            .expect("sign bounded signed-genesis fixture")
            .0;
        let wire = block
            .encode_wire()
            .expect("encode canonical signed genesis");

        let decoded = decode_signed_genesis(&wire).expect("decode canonical signed genesis");
        assert_eq!(decoded.hash(), block.hash());
    }

    #[test]
    fn signed_genesis_reader_rejects_sparse_overflow_before_reading() {
        let directory = tempfile::tempdir().expect("create signed-genesis test directory");
        let path = directory.path().join("oversized-genesis.nrt");
        let file = fs::File::create(&path).expect("create sparse signed genesis");
        file.set_len((SIGNED_GENESIS_MAX_BYTES_V1 + 1) as u64)
            .expect("extend sparse signed genesis");

        let error = read_signed_genesis_bytes(&path)
            .expect_err("oversized signed genesis must fail before reading");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(
            error
                .to_string()
                .contains(&SIGNED_GENESIS_MAX_BYTES_V1.to_string())
        );
    }

    #[test]
    fn signed_genesis_decode_limits_are_fixed() {
        let limits = signed_genesis_decode_limits_v1();
        assert_eq!(
            limits.max_sequence_elements(),
            SIGNED_GENESIS_MAX_SEQUENCE_ELEMENTS_V1
        );
        assert_eq!(limits.max_field_bytes(), SIGNED_GENESIS_MAX_BYTES_V1);
        assert_eq!(
            limits.max_total_elements(),
            SIGNED_GENESIS_MAX_TOTAL_ELEMENTS_V1
        );
        assert_eq!(
            limits.max_total_allocated_bytes(),
            SIGNED_GENESIS_MAX_ALLOCATED_BYTES_V1
        );
        assert_eq!(
            limits.max_nesting_depth(),
            SIGNED_GENESIS_MAX_NESTING_DEPTH_V1
        );
    }

    #[test]
    fn lexical_budget_rejects_first_token_string_and_depth_overflow() {
        assert!(validate_json_lexical_budget("[0,1]", 3, 8, 1).is_ok());
        assert!(validate_json_lexical_budget("[0,1,2]", 3, 8, 1).is_err());
        assert!(validate_json_lexical_budget(r#"{"a":"12345678"}"#, 3, 8, 1).is_ok());
        assert!(validate_json_lexical_budget(r#"{"a":"123456789"}"#, 3, 8, 1).is_err());
        assert!(validate_json_lexical_budget("[[0]]", 3, 8, 1).is_err());
    }
}
