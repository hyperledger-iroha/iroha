//! Independent Rust admission of the reviewed full-source closure.
//!
//! This is deliberately a consumer-side implementation of
//! `scripts/kagemusha_source_tree_seal.py`. It does not invoke that producer.

use std::{
    collections::BTreeSet,
    ffi::{OsStr, OsString},
    fs::{File, Metadata, OpenOptions},
    io::{Read as _, Write as _},
    os::unix::{
        ffi::{OsStrExt as _, OsStringExt as _},
        fs::{MetadataExt as _, OpenOptionsExt as _},
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use color_eyre::eyre::{Result, WrapErr as _, bail, eyre};

use super::{
    canonical_closure::{Sha256, validate_absolute_normalized},
    sha256_hex, typed_json_bytes,
};

const SOURCE_TREE_DOMAIN: &[u8] = b"iroha.kagemusha.full-source-tree-sha256.v3\0";
const SOURCE_DIFF_DOMAIN: &[u8] = b"iroha-source-diff-v1\0";
const TRACKED_DIFF_DOMAIN: &[u8] = b"tracked-binary-diff-sha256\0";
const UNTRACKED_MANIFEST_DOMAIN: &[u8] = b"untracked-path-blob-manifest-sha256\0";
const REVIEWED_SOURCE_CLOSURE_SCHEMA: &str = "iroha.reviewed-source-closure.v1";
const REQUIRED_IGNORED_BUILD_INPUT: &[u8] = b"Cargo.lock";
const PINNED_GIT: &str = "/usr/bin/git";
const MAX_DESCRIPTOR_BYTES: u64 = 16 * 1024 * 1024;
const MAX_CARGO_LOCK_BYTES: u64 = 16 * 1024 * 1024;
const MAX_UNTRACKED_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_UNTRACKED_FILES: usize = 100_000;
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

const TRACKED_DIFF_ARGUMENTS: &[&str] = &[
    "--no-pager",
    "diff",
    "--binary",
    "--full-index",
    "--no-renames",
    "--diff-algorithm=myers",
    "--no-ext-diff",
    "--no-textconv",
    "--ignore-submodules=none",
    "HEAD",
    "--",
    ".",
];

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
struct UntrackedManifestEntryV1 {
    blob_sha256: String,
    git_blob_oid: String,
    git_mode: String,
    path: String,
    path_bytes_base64: String,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
)]
struct ReviewedSourceClosureV1 {
    base_commit: String,
    combined_source_fingerprint_sha256: String,
    ignored_cargo_lock_sha256: String,
    ignored_cargo_lock_size_bytes: u64,
    schema: String,
    source_commit: String,
    source_repo_dirty: bool,
    source_tree_sha256: String,
    tracked_binary_diff_sha256: String,
    untracked_file_count: usize,
    untracked_path_mode_blob_oid_manifest: Vec<UntrackedManifestEntryV1>,
    untracked_path_mode_blob_oid_manifest_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ValidatedSourceIdentity {
    pub(super) source_commit: String,
    pub(super) source_tree_sha256: String,
    pub(super) descriptor_sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StableIdentity {
    device: u64,
    inode: u64,
    mode: u32,
    links: u64,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl StableIdentity {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            links: metadata.nlink(),
            size: metadata.size(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

#[derive(Debug)]
struct IndexEntry {
    path: Vec<u8>,
}

fn is_lower_hex(value: &[u8], length: usize) -> bool {
    value.len() == length
        && value
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn require_commit(value: &str, label: &str) -> Result<()> {
    if !is_lower_hex(value.as_bytes(), 40) || value.as_bytes().iter().all(|byte| *byte == b'0') {
        bail!("{label} must be one nonzero lowercase SHA-1 commit");
    }
    Ok(())
}

fn require_sha256(value: &str, label: &str) -> Result<()> {
    if !is_lower_hex(value.as_bytes(), 64) || value.as_bytes().iter().all(|byte| *byte == b'0') {
        bail!("{label} must be one nonzero lowercase SHA-256");
    }
    Ok(())
}

fn configured_git(root: &Path) -> Command {
    let mut command = Command::new(PINNED_GIT);
    command
        .env_clear()
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_LITERAL_PATHSPECS", "1")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_PAGER", "cat")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("HOME", "/var/empty")
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("PAGER", "cat")
        .env("PATH", "/usr/bin:/bin")
        .env("TZ", "UTC")
        .arg("-c")
        .arg("core.attributesFile=/dev/null")
        .arg("-c")
        .arg("core.excludesFile=/dev/null")
        .arg("-c")
        .arg("core.fsmonitor=false")
        .arg("-c")
        .arg("core.untrackedCache=false")
        .arg("-C")
        .arg(root);
    command
}

fn git(root: &Path, arguments: &[&str]) -> Result<Vec<u8>> {
    let git_metadata =
        std::fs::symlink_metadata(PINNED_GIT).wrap_err("pinned /usr/bin/git is unavailable")?;
    if !git_metadata.is_file() || git_metadata.file_type().is_symlink() {
        bail!("pinned /usr/bin/git must be a non-symlink regular file");
    }
    let output = configured_git(root)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .wrap_err_with(|| format!("failed to execute pinned Git {}", arguments.join(" ")))?;
    if !output.status.success() {
        bail!("pinned Git failed: {}", arguments.join(" "));
    }
    Ok(output.stdout)
}

fn git_hash_object(root: &Path, bytes: &[u8]) -> Result<String> {
    let mut child = configured_git(root)
        .args(["hash-object", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .wrap_err("failed to start pinned Git hash-object")?;
    child
        .stdin
        .take()
        .ok_or_else(|| eyre!("pinned Git hash-object stdin is unavailable"))?
        .write_all(bytes)
        .wrap_err("failed to feed pinned Git hash-object")?;
    let output = child
        .wait_with_output()
        .wrap_err("failed to wait for pinned Git hash-object")?;
    if !output.status.success() {
        bail!("pinned Git hash-object failed");
    }
    let output = trim_one_line(&output.stdout, "Git blob object id")?;
    if !is_lower_hex(output, 40) {
        bail!("pinned Git returned a non-canonical blob object id");
    }
    String::from_utf8(output.to_vec()).wrap_err("Git blob object id is not ASCII")
}

fn trim_one_line<'a>(bytes: &'a [u8], label: &str) -> Result<&'a [u8]> {
    let value = bytes
        .strip_suffix(b"\n")
        .or_else(|| bytes.strip_suffix(b"\r\n"))
        .unwrap_or(bytes);
    if value.is_empty() || value.contains(&b'\n') || value.contains(&b'\r') {
        bail!("{label} is not one nonempty line");
    }
    Ok(value)
}

fn exact_repository_root(root: &Path) -> Result<PathBuf> {
    let root = validate_absolute_normalized(root, "--source-root")?;
    let resolved = std::fs::canonicalize(&root).wrap_err("source root is unavailable")?;
    if resolved != root {
        bail!("--source-root must be the exact canonical repository root");
    }
    let discovered = trim_one_line(
        &git(&root, &["rev-parse", "--show-toplevel"])?,
        "Git repository root",
    )?
    .to_vec();
    let discovered = PathBuf::from(OsString::from_vec(discovered));
    if discovered != root {
        bail!("--source-root must be the exact Git repository root");
    }
    Ok(root)
}

fn head(root: &Path) -> Result<String> {
    let output = git(root, &["rev-parse", "--verify", "HEAD^{commit}"])?;
    let value = trim_one_line(&output, "Git HEAD")?;
    if !is_lower_hex(value, 40) || value.iter().all(|byte| *byte == b'0') {
        bail!("Git HEAD is not one nonzero canonical SHA-1 commit id");
    }
    String::from_utf8(value.to_vec()).wrap_err("Git HEAD is not ASCII")
}

fn safe_relative_path(path: &[u8], allow_cargo_lock: bool) -> Result<()> {
    if path.is_empty()
        || path.starts_with(b"/")
        || path.ends_with(b"/")
        || path.contains(&0)
        || path
            .split(|byte| *byte == b'/')
            .any(|component| component.is_empty() || component == b"." || component == b"..")
        || path.split(|byte| *byte == b'/').next() == Some(b".git".as_slice())
        || (!allow_cargo_lock && path == REQUIRED_IGNORED_BUILD_INPUT)
    {
        bail!("Git returned an unsafe source path");
    }
    Ok(())
}

fn split_nul(output: &[u8]) -> impl Iterator<Item = &[u8]> {
    output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
}

fn index_entries(root: &Path) -> Result<Vec<IndexEntry>> {
    let output = git(root, &["ls-files", "--stage", "-z", "--"])?;
    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();
    for record in split_nul(&output) {
        let tab = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| eyre!("Git returned a malformed index record"))?;
        let metadata = &record[..tab];
        let path = &record[tab + 1..];
        let fields = metadata.split(|byte| *byte == b' ').collect::<Vec<_>>();
        if fields.len() != 3 {
            bail!("Git returned a malformed index record");
        }
        let mode = fields[0];
        let object_id = fields[1];
        let stage = fields[2];
        if !matches!(mode, b"100644" | b"100755" | b"120000") {
            bail!("Git index contains an unsupported mode");
        }
        if !is_lower_hex(object_id, 40) {
            bail!("Git returned a non-canonical index object id");
        }
        if stage != b"0" {
            bail!("source index contains an unresolved merge stage");
        }
        safe_relative_path(path, true)?;
        if !seen.insert(path.to_vec()) {
            bail!("Git returned a duplicate source path");
        }
        entries.push(IndexEntry {
            path: path.to_vec(),
        });
    }
    if entries.is_empty() {
        bail!("source index is empty");
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

fn untracked_paths(root: &Path) -> Result<Vec<Vec<u8>>> {
    let output = git(
        root,
        &["ls-files", "--others", "--exclude-standard", "-z", "--"],
    )?;
    let paths = split_nul(&output)
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if paths.len() > MAX_UNTRACKED_FILES {
        bail!("untracked source inventory exceeds its file-count bound");
    }
    for path in &paths {
        safe_relative_path(path, false)?;
    }
    let mut sorted = paths.clone();
    sorted.sort();
    sorted.dedup();
    if paths != sorted {
        bail!("untracked source paths are not unique and raw-byte sorted");
    }
    Ok(paths)
}

fn ignored_paths(root: &Path) -> Result<Vec<Vec<u8>>> {
    let output = git(
        root,
        &[
            "ls-files",
            "--others",
            "--ignored",
            "--exclude-standard",
            "-z",
            "--",
        ],
    )?;
    let mut paths = split_nul(&output)
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    for path in &paths {
        safe_relative_path(path, true)?;
    }
    Ok(paths)
}

fn path_from_raw(root: &Path, relative: &[u8]) -> PathBuf {
    root.join(OsStr::from_bytes(relative))
}

fn regular_mode(metadata: &Metadata) -> &'static [u8] {
    if metadata.mode() & 0o111 != 0 {
        b"100755"
    } else {
        b"100644"
    }
}

fn open_stable_regular(
    path: &Path,
    maximum_bytes: Option<u64>,
    require_nonempty: bool,
) -> Result<(File, StableIdentity)> {
    let before = std::fs::symlink_metadata(path).wrap_err("inspect source file")?;
    let before_identity = StableIdentity::from_metadata(&before);
    if !before.is_file() || before.file_type().is_symlink() || before.nlink() != 1 {
        bail!("source must be a singly linked non-symlink regular file");
    }
    if (require_nonempty && before.len() == 0)
        || maximum_bytes.is_some_and(|maximum| before.len() > maximum)
    {
        bail!("source file has an invalid size");
    }
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)
        .wrap_err("open source file without following symlinks")?;
    let opened = file.metadata().wrap_err("inspect opened source file")?;
    if StableIdentity::from_metadata(&opened) != before_identity {
        bail!("source changed while opened");
    }
    Ok((file, before_identity))
}

fn verify_stable_after(path: &Path, file: &File, expected: StableIdentity) -> Result<()> {
    let opened_after = file.metadata().wrap_err("reinspect opened source file")?;
    let path_after = std::fs::symlink_metadata(path).wrap_err("reinspect source file path")?;
    if StableIdentity::from_metadata(&opened_after) != expected
        || StableIdentity::from_metadata(&path_after) != expected
    {
        bail!("source changed while read");
    }
    Ok(())
}

fn hash_regular_into(
    path: &Path,
    source_hasher: &mut Sha256,
    maximum_bytes: Option<u64>,
    require_nonempty: bool,
    capture: bool,
) -> Result<(u64, String, Option<Vec<u8>>)> {
    let (mut file, identity) = open_stable_regular(path, maximum_bytes, require_nonempty)?;
    source_hasher.update(&identity.size.to_be_bytes());
    let mut sha256 = Sha256::new();
    let mut captured = capture.then(Vec::new);
    let mut buffer = [0_u8; 1024 * 1024];
    let mut total = 0_u64;
    loop {
        let read = file.read(&mut buffer).wrap_err("read source file")?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).expect("buffer read length fits u64"))
            .ok_or_else(|| eyre!("source size overflow"))?;
        source_hasher.update(&buffer[..read]);
        sha256.update(&buffer[..read]);
        if let Some(bytes) = &mut captured {
            bytes.extend_from_slice(&buffer[..read]);
        }
    }
    if total != identity.size {
        bail!("source was truncated while read");
    }
    verify_stable_after(path, &file, identity)?;
    Ok((total, hex::encode(sha256.finalize()), captured))
}

fn read_stable_bounded(path: &Path, maximum_bytes: u64) -> Result<Vec<u8>> {
    let (mut file, identity) = open_stable_regular(path, Some(maximum_bytes), true)?;
    let capacity =
        usize::try_from(identity.size).wrap_err("bounded source does not fit address space")?;
    let mut bytes = Vec::with_capacity(capacity);
    file.read_to_end(&mut bytes)
        .wrap_err("read bounded source file")?;
    if bytes.len() != capacity {
        bail!("bounded source was truncated while read");
    }
    verify_stable_after(path, &file, identity)?;
    Ok(bytes)
}

fn stable_symlink_bytes(path: &Path) -> Result<Vec<u8>> {
    let before = std::fs::symlink_metadata(path).wrap_err("inspect tracked symlink")?;
    let identity = StableIdentity::from_metadata(&before);
    if !before.file_type().is_symlink() || before.nlink() != 1 {
        bail!("tracked symlink must be singly linked");
    }
    let target = std::fs::read_link(path).wrap_err("read tracked symlink")?;
    let after = std::fs::symlink_metadata(path).wrap_err("reinspect tracked symlink")?;
    if StableIdentity::from_metadata(&after) != identity {
        bail!("tracked symlink changed while read");
    }
    Ok(target.as_os_str().as_bytes().to_vec())
}

fn field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update(
        &u64::try_from(value.len())
            .expect("slice length fits u64")
            .to_be_bytes(),
    );
    hasher.update(value);
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        output.push(char::from(ALPHABET[usize::from(first >> 2)]));
        output.push(char::from(
            ALPHABET[usize::from(((first & 0x03) << 4) | (second >> 4))],
        ));
        if chunk.len() > 1 {
            output.push(char::from(
                ALPHABET[usize::from(((second & 0x0f) << 2) | (third >> 6))],
            ));
        } else {
            output.push('=');
        }
        if chunk.len() > 2 {
            output.push(char::from(ALPHABET[usize::from(third & 0x3f)]));
        } else {
            output.push('=');
        }
    }
    output
}

fn digest_bytes(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    hex::encode(digest.finalize())
}

fn untracked_manifest_bytes(entries: &[UntrackedManifestEntryV1]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for entry in entries {
        bytes.extend(typed_json_bytes(entry)?);
    }
    Ok(bytes)
}

fn capture_observed_descriptor(root: &Path) -> Result<ReviewedSourceClosureV1> {
    let head_before = head(root)?;
    let diff_before = git(root, TRACKED_DIFF_ARGUMENTS)?;
    let untracked_before = untracked_paths(root)?;
    let ignored_before = ignored_paths(root)?;
    if ignored_before != [REQUIRED_IGNORED_BUILD_INPUT.to_vec()] {
        bail!("ignored source set must contain exactly the separately bound root Cargo.lock");
    }
    let entries = index_entries(root)?;
    let mut source_hasher = Sha256::new();
    source_hasher.update(SOURCE_TREE_DOMAIN);

    for entry in entries {
        let absolute = path_from_raw(root, &entry.path);
        field(&mut source_hasher, b"tracked-source-v1");
        field(&mut source_hasher, &entry.path);
        let metadata = match std::fs::symlink_metadata(&absolute) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                field(&mut source_hasher, b"absent");
                continue;
            }
            Err(error) => return Err(error).wrap_err("inspect tracked source path"),
        };
        if metadata.is_file() && !metadata.file_type().is_symlink() {
            field(&mut source_hasher, regular_mode(&metadata));
            let _ = hash_regular_into(&absolute, &mut source_hasher, None, false, false)?;
        } else if metadata.file_type().is_symlink() {
            field(&mut source_hasher, b"120000");
            field(&mut source_hasher, &stable_symlink_bytes(&absolute)?);
        } else {
            bail!("tracked source has an unsafe file type");
        }
    }

    let mut untracked_manifest = Vec::with_capacity(untracked_before.len());
    for path in &untracked_before {
        let absolute = path_from_raw(root, path);
        let metadata =
            std::fs::symlink_metadata(&absolute).wrap_err("inspect untracked source path")?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            bail!("untracked source must be a non-symlink regular file");
        }
        let git_mode = regular_mode(&metadata);
        field(&mut source_hasher, b"untracked-source-v1");
        field(&mut source_hasher, path);
        field(&mut source_hasher, git_mode);
        let (_, blob_sha256, captured) = hash_regular_into(
            &absolute,
            &mut source_hasher,
            Some(MAX_UNTRACKED_FILE_BYTES),
            true,
            true,
        )?;
        let captured = captured.expect("untracked source capture requested");
        let display = std::str::from_utf8(path)
            .wrap_err("untracked source path is not lossless UTF-8")?
            .to_owned();
        untracked_manifest.push(UntrackedManifestEntryV1 {
            blob_sha256,
            git_blob_oid: git_hash_object(root, &captured)?,
            git_mode: std::str::from_utf8(git_mode)
                .expect("canonical Git mode is ASCII")
                .to_owned(),
            path: display,
            path_bytes_base64: base64_encode(path),
        });
    }

    let cargo_lock_path = root.join(OsStr::from_bytes(REQUIRED_IGNORED_BUILD_INPUT));
    let cargo_metadata =
        std::fs::symlink_metadata(&cargo_lock_path).wrap_err("inspect ignored root Cargo.lock")?;
    if cargo_metadata.mode() & 0o111 != 0 {
        bail!("ignored root Cargo.lock must not be executable");
    }
    field(&mut source_hasher, b"required-ignored-build-input-v1");
    field(&mut source_hasher, REQUIRED_IGNORED_BUILD_INPUT);
    field(&mut source_hasher, b"100644");
    let (cargo_lock_size, cargo_lock_sha256, _) = hash_regular_into(
        &cargo_lock_path,
        &mut source_hasher,
        Some(MAX_CARGO_LOCK_BYTES),
        true,
        false,
    )?;

    let head_after = head(root)?;
    let diff_after = git(root, TRACKED_DIFF_ARGUMENTS)?;
    let untracked_after = untracked_paths(root)?;
    let ignored_after = ignored_paths(root)?;
    let cargo_recheck = read_stable_bounded(&cargo_lock_path, MAX_CARGO_LOCK_BYTES)?;
    if head_after != head_before
        || diff_after != diff_before
        || untracked_after != untracked_before
        || ignored_after != ignored_before
        || u64::try_from(cargo_recheck.len()).expect("bounded Cargo.lock length fits u64")
            != cargo_lock_size
        || sha256_hex(&cargo_recheck) != cargo_lock_sha256
    {
        bail!("Kagemusha source HEAD or closure changed while sealing");
    }

    let tracked_binary_diff_sha256 = digest_bytes(&diff_before);
    let untracked_manifest_sha256 = digest_bytes(&untracked_manifest_bytes(&untracked_manifest)?);
    let mut combined = Sha256::new();
    combined.update(SOURCE_DIFF_DOMAIN);
    combined.update(TRACKED_DIFF_DOMAIN);
    combined.update(
        &hex::decode(&tracked_binary_diff_sha256)
            .expect("internally computed tracked diff SHA-256 is valid hex"),
    );
    combined.update(UNTRACKED_MANIFEST_DOMAIN);
    combined.update(
        &hex::decode(&untracked_manifest_sha256)
            .expect("internally computed untracked manifest SHA-256 is valid hex"),
    );
    let source_repo_dirty =
        tracked_binary_diff_sha256 != EMPTY_SHA256 || !untracked_manifest.is_empty();
    Ok(ReviewedSourceClosureV1 {
        base_commit: head_before.clone(),
        combined_source_fingerprint_sha256: hex::encode(combined.finalize()),
        ignored_cargo_lock_sha256: cargo_lock_sha256,
        ignored_cargo_lock_size_bytes: cargo_lock_size,
        schema: REVIEWED_SOURCE_CLOSURE_SCHEMA.to_owned(),
        source_commit: head_before,
        source_repo_dirty,
        source_tree_sha256: hex::encode(source_hasher.finalize()),
        tracked_binary_diff_sha256,
        untracked_file_count: untracked_manifest.len(),
        untracked_path_mode_blob_oid_manifest: untracked_manifest,
        untracked_path_mode_blob_oid_manifest_sha256: untracked_manifest_sha256,
    })
}

fn validate_descriptor_self_consistency(
    descriptor: &ReviewedSourceClosureV1,
    expected_source_commit: &str,
) -> Result<()> {
    if descriptor.schema != REVIEWED_SOURCE_CLOSURE_SCHEMA
        || descriptor.base_commit != expected_source_commit
        || descriptor.source_commit != expected_source_commit
    {
        bail!("reviewed source closure does not bind the exact external source commit");
    }
    for (label, value) in [
        ("source tree", &descriptor.source_tree_sha256),
        (
            "tracked binary diff",
            &descriptor.tracked_binary_diff_sha256,
        ),
        (
            "untracked manifest",
            &descriptor.untracked_path_mode_blob_oid_manifest_sha256,
        ),
        ("Cargo.lock", &descriptor.ignored_cargo_lock_sha256),
        (
            "combined source fingerprint",
            &descriptor.combined_source_fingerprint_sha256,
        ),
    ] {
        require_sha256(value, label)?;
    }
    if descriptor.untracked_file_count != descriptor.untracked_path_mode_blob_oid_manifest.len()
        || descriptor.untracked_file_count > MAX_UNTRACKED_FILES
        || descriptor.ignored_cargo_lock_size_bytes == 0
        || descriptor.ignored_cargo_lock_size_bytes > MAX_CARGO_LOCK_BYTES
    {
        bail!("reviewed source closure contains invalid bounded counts");
    }
    let mut previous_path: Option<Vec<u8>> = None;
    for entry in &descriptor.untracked_path_mode_blob_oid_manifest {
        require_sha256(&entry.blob_sha256, "untracked blob")?;
        if !is_lower_hex(entry.git_blob_oid.as_bytes(), 40)
            || !matches!(entry.git_mode.as_str(), "100644" | "100755")
        {
            bail!("reviewed source closure contains malformed untracked entry");
        }
        let path = entry.path.as_bytes();
        safe_relative_path(path, false)?;
        if entry.path_bytes_base64 != base64_encode(path) {
            bail!("reviewed source closure path display/Base64 binding differs");
        }
        if previous_path
            .as_ref()
            .is_some_and(|previous| previous.as_slice() >= path)
        {
            bail!("reviewed source closure untracked paths are not raw-byte sorted and unique");
        }
        previous_path = Some(path.to_vec());
    }
    let manifest_sha256 = digest_bytes(&untracked_manifest_bytes(
        &descriptor.untracked_path_mode_blob_oid_manifest,
    )?);
    if manifest_sha256 != descriptor.untracked_path_mode_blob_oid_manifest_sha256 {
        bail!("reviewed source closure untracked manifest SHA-256 is inconsistent");
    }
    let mut combined = Sha256::new();
    combined.update(SOURCE_DIFF_DOMAIN);
    combined.update(TRACKED_DIFF_DOMAIN);
    combined.update(
        &hex::decode(&descriptor.tracked_binary_diff_sha256)
            .wrap_err("decode tracked binary diff SHA-256")?,
    );
    combined.update(UNTRACKED_MANIFEST_DOMAIN);
    combined.update(&hex::decode(&manifest_sha256).expect("computed SHA-256 is valid hex"));
    if hex::encode(combined.finalize()) != descriptor.combined_source_fingerprint_sha256 {
        bail!("reviewed source closure combined fingerprint is inconsistent");
    }
    let derived_dirty = descriptor.tracked_binary_diff_sha256 != EMPTY_SHA256
        || descriptor.untracked_file_count != 0;
    if descriptor.source_repo_dirty != derived_dirty || !derived_dirty {
        bail!("reviewed source closure must bind the exact nonempty derived closure state");
    }
    Ok(())
}

pub(super) fn validate_reviewed_source(
    source_root: &Path,
    descriptor_path: &Path,
    expected_descriptor_sha256: &str,
    expected_source_commit: &str,
    expected_source_tree_sha256: &str,
) -> Result<ValidatedSourceIdentity> {
    require_sha256(
        expected_descriptor_sha256,
        "external reviewed source closure descriptor SHA-256",
    )?;
    require_commit(expected_source_commit, "external expected source commit")?;
    require_sha256(
        expected_source_tree_sha256,
        "external expected source tree SHA-256",
    )?;
    let embedded_source = crate::BUILD_SOURCE_ID
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            eyre!("this command requires a release build with IROHA_GIT_COMMIT_HASH embedded")
        })?;
    if embedded_source != expected_source_commit {
        bail!("embedded build source differs from the externally pinned exact source commit");
    }

    let source_root = exact_repository_root(source_root)?;
    if head(&source_root)? != expected_source_commit {
        bail!("source repository HEAD differs from the external exact source commit");
    }
    let descriptor_path =
        validate_absolute_normalized(descriptor_path, "--reviewed-source-closure")?;
    if std::fs::canonicalize(&descriptor_path)
        .wrap_err("reviewed source closure descriptor is unavailable")?
        != descriptor_path
    {
        bail!("reviewed source closure descriptor path must not traverse symlinks");
    }
    let descriptor_bytes = read_stable_bounded(&descriptor_path, MAX_DESCRIPTOR_BYTES)?;
    if sha256_hex(&descriptor_bytes) != expected_descriptor_sha256 {
        bail!("reviewed source closure descriptor differs from its external SHA-256 pin");
    }
    let descriptor_text = std::str::from_utf8(&descriptor_bytes)
        .wrap_err("reviewed source closure descriptor is not UTF-8")?;
    let value: norito::json::Value = norito::json::from_str(descriptor_text)
        .wrap_err("reviewed source closure descriptor is not strict JSON")?;
    if super::canonical_json_bytes(&value)? != descriptor_bytes {
        bail!("reviewed source closure descriptor bytes are not canonical");
    }
    let descriptor: ReviewedSourceClosureV1 = norito::json::value::from_value(value)
        .wrap_err("reviewed source closure descriptor schema is not exact")?;
    if typed_json_bytes(&descriptor)? != descriptor_bytes {
        bail!("reviewed source closure descriptor changed across typed JSON round-trip");
    }
    validate_descriptor_self_consistency(&descriptor, expected_source_commit)?;
    if descriptor.source_tree_sha256 != expected_source_tree_sha256 {
        bail!("reviewed source closure descriptor differs from external source-tree pin");
    }

    let observed = capture_observed_descriptor(&source_root)?;
    if observed != descriptor {
        bail!("current source closure differs from the independently pinned descriptor");
    }
    Ok(ValidatedSourceIdentity {
        source_commit: descriptor.source_commit,
        source_tree_sha256: descriptor.source_tree_sha256,
        descriptor_sha256: expected_descriptor_sha256.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_encoder_matches_canonical_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn sha256_consumer_matches_standard_vectors() {
        assert_eq!(digest_bytes(b""), EMPTY_SHA256);
        assert_eq!(
            digest_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn normalized_source_root_rejects_parent_components() {
        let path = Path::new("/tmp/a/../b");
        assert!(validate_absolute_normalized(path, "test").is_err());
        assert!(
            path.components()
                .any(|part| part == std::path::Component::ParentDir)
        );
    }

    #[test]
    fn rust_observation_matches_reviewed_python_producer() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let root = manifest
            .parent()
            .and_then(Path::parent)
            .expect("workspace root")
            .to_path_buf();
        let output = Command::new("/usr/bin/python3")
            .arg(root.join("scripts/kagemusha_source_tree_seal.py"))
            .arg("descriptor")
            .arg("--root")
            .arg(&root)
            .output()
            .expect("execute reviewed Python source descriptor producer");
        assert!(
            output.status.success(),
            "reviewed Python source descriptor producer failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = std::str::from_utf8(&output.stdout).expect("producer emitted UTF-8");
        let value: norito::json::Value =
            norito::json::from_str(text).expect("producer emitted valid JSON");
        assert_eq!(
            super::super::canonical_json_bytes(&value).expect("canonical JSON"),
            output.stdout
        );
        let python: ReviewedSourceClosureV1 =
            norito::json::value::from_value(value).expect("exact producer schema");
        let rust = capture_observed_descriptor(&root).expect("Rust source closure observation");
        assert_eq!(rust, python);
    }
}
