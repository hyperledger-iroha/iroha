//! Canonical Taikai distribution-bundle digest (`iroha.taikai.bundle.v1`) and hardened inputs.
//!
//! The `iroha taikai` producer and the `cargo xtask taikai-rpt-verify` verifier must derive the
//! same digests from the same bytes, so both read policy documents and hash bundles through this
//! module. Inputs are opened without following symlinks or blocking on a substituted FIFO, and
//! every file and directory is re-inspected after it is read so a concurrent replacement fails
//! closed instead of producing a digest over mixed state.

use blake3::Hasher;
use std::{
    fmt,
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

/// Domain tag prefixed to every Taikai bundle digest.
pub const TAIKAI_BUNDLE_DIGEST_DOMAIN_V1: &[u8] = b"iroha.taikai.bundle.v1";

/// Largest policy document (GAR, CEK receipt, RPT envelope) the Taikai tooling reads.
pub const MAX_TAIKAI_POLICY_DOCUMENT_BYTES: u64 = 1024 * 1024;

/// Digest a bundle file or directory with the canonical v1 framing.
///
/// The hash starts with [`TAIKAI_BUNDLE_DIGEST_DOMAIN_V1`]. A directory contributes a `D` marker
/// followed by its entries in byte-wise name order; a file contributes an `F` marker, its
/// little-endian `u64` length and its contents. Each marker carries the length-prefixed canonical
/// `/`-separated relative path (`.` for the bundle root). A single-file bundle is labelled by its
/// file name.
///
/// # Errors
///
/// Fails for symlinks, special files, non-UTF-8 names, and any file or directory that changes
/// while it is hashed.
pub fn bundle_digest_v1(path: &Path) -> io::Result<[u8; 32]> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|err| context(err, format_args!("failed to stat `{}`", path.display())))?;
    let mut hasher = Hasher::new();
    hasher.update(TAIKAI_BUNDLE_DIGEST_DOMAIN_V1);
    if metadata.is_file() {
        let relative = path
            .file_name()
            .map_or_else(|| PathBuf::from("."), PathBuf::from);
        hash_file_entry(path, &relative, &mut hasher)?;
    } else if metadata.is_dir() {
        hash_directory_entry_with_hook(path, Path::new(""), &mut hasher, || Ok(()))?;
    } else {
        return Err(invalid(format!(
            "bundle `{}` must be a regular file or directory",
            path.display()
        )));
    }
    Ok(*hasher.finalize().as_bytes())
}

/// BLAKE3 digest of one regular file opened through [`open_regular_input`].
///
/// `label` names the input in error messages.
///
/// # Errors
///
/// Fails when the input is not a direct regular file or changes while it is hashed.
pub fn file_digest(path: &Path, label: &str) -> io::Result<[u8; 32]> {
    let mut file = open_regular_input(path, label)?;
    let mut hasher = Hasher::new();
    let initial_metadata = file
        .metadata()
        .map_err(|err| context(err, format_args!("failed to inspect `{}`", path.display())))?;
    hash_file_contents_with_hook(&mut file, path, &mut hasher, &initial_metadata, || Ok(()))?;
    Ok(*hasher.finalize().as_bytes())
}

/// Open a regular input file without following symlinks or blocking on a substituted FIFO.
///
/// `label` names the input in error messages.
///
/// # Errors
///
/// Fails when the path is a symlink, reparse point or non-regular file, when it is replaced
/// while it is being opened, or when the platform has no secure no-follow open.
pub fn open_regular_input(path: &Path, label: &str) -> io::Result<File> {
    open_regular_input_with_hook(path, label, || Ok(()))
}

/// Read a whole policy document of at most [`MAX_TAIKAI_POLICY_DOCUMENT_BYTES`].
///
/// `file` must come from [`open_regular_input`]; `label` names the document in error messages.
///
/// # Errors
///
/// Fails when the document exceeds the limit or changes length or identity while it is read.
pub fn read_policy_document(file: &mut File, path: &Path, label: &str) -> io::Result<Vec<u8>> {
    read_policy_document_with_hook(file, path, label, || Ok(()))
}

fn open_regular_input_with_hook<F>(path: &Path, label: &str, before_open: F) -> io::Result<File>
where
    F: FnOnce() -> io::Result<()>,
{
    let path_metadata = fs::symlink_metadata(path).map_err(|err| {
        context(
            err,
            format_args!("failed to inspect {label} `{}`", path.display()),
        )
    })?;
    if is_symlink_or_reparse(&path_metadata) || !path_metadata.is_file() {
        return Err(invalid(format!(
            "{label} `{}` must be a regular file and must not be a symlink",
            path.display()
        )));
    }
    before_open()?;

    let mut options = OpenOptions::new();
    options.read(true);
    set_no_follow_nonblocking(&mut options)?;
    let file = options.open(path).map_err(|err| {
        context(
            err,
            format_args!("failed to open {label} `{}`", path.display()),
        )
    })?;
    let opened_metadata = file.metadata().map_err(|err| {
        context(
            err,
            format_args!("failed to inspect opened {label} `{}`", path.display()),
        )
    })?;
    if is_symlink_or_reparse(&opened_metadata) || !opened_metadata.is_file() {
        return Err(invalid(format!(
            "{label} `{}` changed to a non-regular file while opening it",
            path.display()
        )));
    }
    ensure_same_file_state(
        &path_metadata,
        &opened_metadata,
        path,
        label,
        "while it was being opened",
    )?;
    Ok(file)
}

fn read_policy_document_with_hook<F>(
    file: &mut File,
    path: &Path,
    label: &str,
    before_read: F,
) -> io::Result<Vec<u8>>
where
    F: FnOnce() -> io::Result<()>,
{
    let initial_metadata = file.metadata().map_err(|err| {
        context(
            err,
            format_args!("failed to inspect opened {label} `{}`", path.display()),
        )
    })?;
    let advertised_len = initial_metadata.len();
    if advertised_len > MAX_TAIKAI_POLICY_DOCUMENT_BYTES {
        return Err(invalid(format!(
            "{label} `{}` exceeds the {MAX_TAIKAI_POLICY_DOCUMENT_BYTES}-byte policy document limit",
            path.display()
        )));
    }
    let capacity = usize::try_from(advertised_len).expect("bounded document length fits usize");
    let mut bytes = Vec::with_capacity(capacity);
    before_read()?;
    (&mut *file)
        .take(MAX_TAIKAI_POLICY_DOCUMENT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|err| {
            context(
                err,
                format_args!("failed to read {label} `{}`", path.display()),
            )
        })?;
    let bytes_read = u64::try_from(bytes.len()).expect("bounded document length fits u64");
    if bytes_read > MAX_TAIKAI_POLICY_DOCUMENT_BYTES {
        return Err(invalid(format!(
            "{label} `{}` grew beyond the {MAX_TAIKAI_POLICY_DOCUMENT_BYTES}-byte policy document limit while reading",
            path.display()
        )));
    }
    let final_metadata = file.metadata().map_err(|err| {
        context(
            err,
            format_args!(
                "failed to re-inspect opened {label} `{}` after reading",
                path.display()
            ),
        )
    })?;
    if !final_metadata.is_file()
        || is_symlink_or_reparse(&final_metadata)
        || final_metadata.len() != advertised_len
        || bytes_read != advertised_len
    {
        return Err(invalid(format!(
            "{label} `{}` changed length while it was being read (advertised {advertised_len} bytes, read {bytes_read} bytes, final length {} bytes)",
            path.display(),
            final_metadata.len()
        )));
    }
    ensure_same_file_state(
        &initial_metadata,
        &final_metadata,
        path,
        label,
        "while it was being read",
    )?;
    Ok(bytes)
}

fn hash_path_entry(path: &Path, relative: &Path, hasher: &mut Hasher) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|err| context(err, format_args!("failed to stat `{}`", path.display())))?;
    if metadata.is_file() {
        hash_file_entry(path, relative, hasher)
    } else if metadata.is_dir() {
        hash_directory_entry_with_hook(path, relative, hasher, || Ok(()))
    } else {
        Err(invalid(format!(
            "unsupported entry type at `{}` (expected file or directory)",
            path.display()
        )))
    }
}

fn hash_file_entry(path: &Path, relative: &Path, hasher: &mut Hasher) -> io::Result<()> {
    update_path_marker(relative, b'F', hasher)?;
    let mut file = open_regular_input(path, "bundle file")?;
    let initial_metadata = file
        .metadata()
        .map_err(|err| context(err, format_args!("failed to inspect `{}`", path.display())))?;
    hasher.update(&initial_metadata.len().to_le_bytes());
    hash_file_contents_with_hook(&mut file, path, hasher, &initial_metadata, || Ok(()))
}

fn hash_file_contents_with_hook<F>(
    file: &mut File,
    path: &Path,
    hasher: &mut Hasher,
    initial_metadata: &fs::Metadata,
    before_read: F,
) -> io::Result<()>
where
    F: FnOnce() -> io::Result<()>,
{
    before_read()?;
    let mut buffer = [0u8; 8192];
    let mut actual_len = 0_u64;
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| context(err, format_args!("failed to read `{}`", path.display())))?;
        if read == 0 {
            break;
        }
        actual_len = actual_len
            .checked_add(u64::try_from(read).expect("read buffer length fits u64"))
            .ok_or_else(|| {
                invalid(format!(
                    "file length overflowed while reading `{}`",
                    path.display()
                ))
            })?;
        hasher.update(&buffer[..read]);
    }
    let expected_len = initial_metadata.len();
    if actual_len != expected_len {
        return Err(invalid(format!(
            "file `{}` changed length while hashing (expected {expected_len}, read {actual_len})",
            path.display()
        )));
    }
    let final_metadata = file.metadata().map_err(|err| {
        context(
            err,
            format_args!("failed to re-inspect `{}` after hashing", path.display()),
        )
    })?;
    if !final_metadata.is_file() || final_metadata.len() != expected_len {
        return Err(invalid(format!(
            "file `{}` changed length while hashing (expected {expected_len}, final length {})",
            path.display(),
            final_metadata.len()
        )));
    }
    ensure_same_file_state(
        initial_metadata,
        &final_metadata,
        path,
        "file",
        "while it was being hashed",
    )
}

fn hash_directory_entry_with_hook<F>(
    path: &Path,
    relative: &Path,
    hasher: &mut Hasher,
    before_traversal: F,
) -> io::Result<()>
where
    F: FnOnce() -> io::Result<()>,
{
    let initial_metadata = fs::symlink_metadata(path).map_err(|err| {
        context(
            err,
            format_args!("failed to inspect directory `{}`", path.display()),
        )
    })?;
    if !initial_metadata.is_dir() || is_symlink_or_reparse(&initial_metadata) {
        return Err(invalid(format!(
            "bundle directory `{}` must be a direct directory",
            path.display()
        )));
    }
    update_path_marker(relative, b'D', hasher)?;
    before_traversal()?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(path).map_err(|err| {
        context(
            err,
            format_args!("failed to read directory `{}`", path.display()),
        )
    })? {
        let entry = entry.map_err(|err| {
            context(
                err,
                format_args!("failed to iterate directory `{}`", path.display()),
            )
        })?;
        let child_path = entry.path();
        let file_name = entry.file_name().into_string().map_err(|_| {
            invalid(format!(
                "bundle entry `{}` is not valid UTF-8",
                child_path.display()
            ))
        })?;
        entries.push((file_name, child_path));
    }
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    for (file_name, child_path) in entries {
        hash_path_entry(&child_path, &relative.join(file_name), hasher)?;
    }
    let final_metadata = fs::symlink_metadata(path).map_err(|err| {
        context(
            err,
            format_args!("failed to re-inspect directory `{}`", path.display()),
        )
    })?;
    if !final_metadata.is_dir() || is_symlink_or_reparse(&final_metadata) {
        return Err(invalid(format!(
            "bundle directory `{}` changed to an indirect or non-directory entry while hashing",
            path.display()
        )));
    }
    ensure_same_file_state(
        &initial_metadata,
        &final_metadata,
        path,
        "bundle directory",
        "while it was being hashed",
    )
}

fn update_path_marker(relative: &Path, kind: u8, hasher: &mut Hasher) -> io::Result<()> {
    let label = canonical_bundle_relative_path(relative)?;
    let label_len = u64::try_from(label.len())
        .map_err(|_| invalid("bundle path is too long to hash canonically".to_owned()))?;
    hasher.update(&[kind]);
    hasher.update(&label_len.to_le_bytes());
    hasher.update(label.as_bytes());
    Ok(())
}

fn canonical_bundle_relative_path(relative: &Path) -> io::Result<String> {
    if relative.as_os_str().is_empty() {
        return Ok(".".to_owned());
    }
    let mut label = String::new();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(invalid(format!(
                "bundle path `{}` is not a canonical relative path",
                relative.display()
            )));
        };
        let component = component.to_str().ok_or_else(|| {
            invalid(format!(
                "bundle path `{}` is not valid UTF-8",
                relative.display()
            ))
        })?;
        if !label.is_empty() {
            label.push('/');
        }
        label.push_str(component);
    }
    Ok(label)
}

fn ensure_same_file_state(
    expected: &fs::Metadata,
    observed: &fs::Metadata,
    path: &Path,
    label: &str,
    operation: &str,
) -> io::Result<()> {
    if metadata_state_unchanged(expected, observed) {
        Ok(())
    } else {
        Err(invalid(format!(
            "{label} `{}` changed {operation}",
            path.display()
        )))
    }
}

#[cfg(unix)]
fn metadata_state_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.len() == right.len()
        && left.mode() == right.mode()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(windows)]
fn metadata_state_unchanged(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    left.volume_serial_number().is_some()
        && left.file_index().is_some()
        && left.volume_serial_number() == right.volume_serial_number()
        && left.file_index() == right.file_index()
        && left.file_size() == right.file_size()
        && left.file_attributes() == right.file_attributes()
        && left.last_write_time() == right.last_write_time()
        && left.creation_time() == right.creation_time()
}

#[cfg(not(any(unix, windows)))]
fn metadata_state_unchanged(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    false
}

fn is_symlink_or_reparse(metadata: &fs::Metadata) -> bool {
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

/// Configure a no-follow, non-blocking open; `std` already opens every Unix file close-on-exec.
#[cfg(unix)]
fn set_no_follow_nonblocking(options: &mut OpenOptions) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let no_follow = crate::platform_no_follow_flag();
    let nonblocking = platform_nonblocking_flag();
    if no_follow == 0 || nonblocking == 0 {
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "secure direct-file input opens are unavailable on this Unix target",
        ));
    }
    options.custom_flags(no_follow | nonblocking);
    Ok(())
}

#[cfg(windows)]
fn set_no_follow_nonblocking(options: &mut OpenOptions) -> io::Result<()> {
    use std::os::windows::fs::OpenOptionsExt as _;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn set_no_follow_nonblocking(_options: &mut OpenOptions) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "secure direct-file input opens are unavailable on this platform",
    ))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn platform_nonblocking_flag() -> i32 {
    rustix::fs::OFlags::NONBLOCK.bits() as i32
}

/// `O_NONBLOCK` on Apple platforms and the BSDs.
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn platform_nonblocking_flag() -> i32 {
    0x0004
}

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
fn platform_nonblocking_flag() -> i32 {
    0
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn context(err: io::Error, message: fmt::Arguments<'_>) -> io::Error {
    io::Error::new(err.kind(), format!("{message}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write as _, time::Duration};

    fn replace_in_place(path: &Path, replacement: &[u8]) -> io::Result<()> {
        std::thread::sleep(Duration::from_millis(20));
        let mut file = OpenOptions::new().write(true).truncate(true).open(path)?;
        file.write_all(replacement)?;
        file.sync_all()
    }

    #[test]
    fn file_digest_is_independent_of_file_name() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let first = tmp.path().join("gar.jws");
        let second = tmp.path().join("renamed-gar.jws");
        let payload = b"signed-gar-payload";
        fs::write(&first, payload).expect("write first payload");
        fs::write(&second, payload).expect("write renamed payload");
        let expected = *blake3::hash(payload).as_bytes();
        assert_eq!(
            file_digest(&first, "policy input").expect("digest"),
            expected
        );
        assert_eq!(
            file_digest(&second, "policy input").expect("digest"),
            expected
        );
    }

    #[test]
    fn streamed_hash_rejects_same_length_file_mutation() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("gar.jws");
        fs::write(&path, b"signed-gar-data").expect("write GAR");
        let mut file = open_regular_input(&path, "policy input").expect("open GAR");
        let initial = file.metadata().expect("inspect GAR");

        let error =
            hash_file_contents_with_hook(&mut file, &path, &mut Hasher::new(), &initial, || {
                replace_in_place(&path, b"SIGNED-GAR-DATA")
            })
            .expect_err("same-length mutation during hashing must fail closed");

        assert!(
            error
                .to_string()
                .contains("changed while it was being hashed"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn bundle_digest_length_frames_file_contents() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let forged = tmp.path().join("forged");
        let structured = tmp.path().join("structured");
        fs::create_dir_all(&forged).expect("create forged bundle");
        fs::create_dir_all(&structured).expect("create structured bundle");
        fs::write(forged.join("a"), [b'b', b'c', 0xFF, b'F', b'd']).expect("write forged entry");
        fs::write(structured.join("a"), b"b").expect("write structured first entry");
        fs::write(structured.join("c"), b"d").expect("write structured second entry");

        assert_ne!(
            bundle_digest_v1(&forged).expect("forged digest"),
            bundle_digest_v1(&structured).expect("structured digest"),
            "file length framing must distinguish bytes that imitate a second entry"
        );
    }

    #[test]
    fn bundle_digest_matches_canonical_v1_test_vector() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let bundle = tmp.path().join("bundle");
        fs::create_dir_all(bundle.join("nested")).expect("create nested bundle");
        fs::write(bundle.join("nested/beta"), b"BC").expect("write nested entry");
        fs::write(bundle.join("alpha"), b"A").expect("write root entry");

        assert_eq!(
            hex::encode(bundle_digest_v1(&bundle).expect("bundle digest")),
            "32b42aff6303e492d041c7620f8b98f3dc1ee1f613de002a35c51b428d940846"
        );
    }

    #[test]
    fn bundle_hash_rejects_directory_membership_change() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let bundle = tmp.path().join("bundle");
        fs::create_dir(&bundle).expect("create bundle");
        fs::write(bundle.join("original"), b"entry").expect("write original entry");

        let error =
            hash_directory_entry_with_hook(&bundle, Path::new(""), &mut Hasher::new(), || {
                std::thread::sleep(Duration::from_millis(20));
                fs::write(bundle.join("added"), b"late entry")
            })
            .expect_err("bundle membership changes must fail closed");

        assert!(
            error
                .to_string()
                .contains("changed while it was being hashed"),
            "unexpected error: {error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn bundle_digest_rejects_symlinked_members() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let bundle = tmp.path().join("bundle");
        fs::create_dir(&bundle).expect("create bundle");
        fs::write(tmp.path().join("outside"), b"outside").expect("write target");
        std::os::unix::fs::symlink(tmp.path().join("outside"), bundle.join("link"))
            .expect("create member symlink");

        let error = bundle_digest_v1(&bundle).expect_err("symlinked members must fail closed");

        assert!(error.to_string().contains("unsupported entry type"));
    }

    #[test]
    fn canonical_relative_paths_reject_parent_components() {
        assert_eq!(
            canonical_bundle_relative_path(Path::new("")).expect("root"),
            "."
        );
        assert_eq!(
            canonical_bundle_relative_path(Path::new("nested/beta")).expect("nested"),
            "nested/beta"
        );
        let error = canonical_bundle_relative_path(Path::new("../escape"))
            .expect_err("parent components are not canonical");
        assert!(error.to_string().contains("not a canonical relative path"));
    }

    #[test]
    fn policy_reader_rejects_oversized_document() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("oversized.to");
        File::create(&path)
            .expect("create oversized document")
            .set_len(MAX_TAIKAI_POLICY_DOCUMENT_BYTES + 1)
            .expect("size oversized document");
        let mut file = open_regular_input(&path, "test policy document").expect("open document");

        let error = read_policy_document(&mut file, &path, "test policy document")
            .expect_err("oversized documents must fail before reading");

        assert!(error.to_string().contains("policy document limit"));
    }

    #[test]
    fn policy_reader_rejects_document_truncated_after_size_check() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("changing.to");
        fs::write(&path, b"policy-document").expect("write policy document");
        let mut file = open_regular_input(&path, "test policy document").expect("open document");

        let error =
            read_policy_document_with_hook(&mut file, &path, "test policy document", || {
                OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&path)
                    .map(drop)
            })
            .expect_err("a document truncated after its size check must fail closed");

        assert!(error.to_string().contains("changed length"));
    }

    #[test]
    fn policy_reader_rejects_same_length_document_mutation() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("changing.to");
        fs::write(&path, b"policy-document").expect("write policy document");
        let mut file = open_regular_input(&path, "test policy document").expect("open document");

        let error =
            read_policy_document_with_hook(&mut file, &path, "test policy document", || {
                replace_in_place(&path, b"POLICY-DOCUMENT")
            })
            .expect_err("same-length in-place mutation must fail closed");

        assert!(
            error
                .to_string()
                .contains("changed while it was being read"),
            "unexpected error: {error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn policy_open_rejects_regular_to_fifo_swap_without_blocking() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("changing.to");
        fs::write(&path, b"policy-document").expect("write policy document");
        let writer_path = path.clone();
        let unblocker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(1));
            let mut options = OpenOptions::new();
            options.write(true);
            set_no_follow_nonblocking(&mut options).expect("configure nonblocking FIFO writer");
            let _ = options.open(writer_path);
        });

        let started = std::time::Instant::now();
        let error = open_regular_input_with_hook(&path, "test policy document", || {
            fs::remove_file(&path)?;
            let status = std::process::Command::new("mkfifo").arg(&path).status()?;
            if status.success() {
                Ok(())
            } else {
                Err(io::Error::other(format!("mkfifo failed with {status}")))
            }
        })
        .expect_err("a FIFO substituted during open must fail closed");
        let elapsed = started.elapsed();
        unblocker.join().expect("join FIFO unblocker");

        assert!(error.to_string().contains("non-regular file"));
        assert!(
            elapsed < Duration::from_millis(900),
            "FIFO substitution blocked for {elapsed:?}"
        );
    }
}
