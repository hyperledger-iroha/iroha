//! Genuine native execution and custody controls for recovered executable readers.

use super::*;
use std::{io::Write as _, process::Command};

fn publish(root: &Path, name: &str, bytes: &[u8], executable: bool) -> FileIdentity {
    let source = OwnerDirectory::open_or_create(root.join("staging")).unwrap();
    let destination = OwnerDirectory::open_or_create(root.join("installed")).unwrap();
    let mut pending = source.create_retained_private(name, bytes.len()).unwrap();
    pending.write_all(bytes).unwrap();
    let sealed = if executable {
        pending.seal_executable().unwrap()
    } else {
        pending.seal_read_only().unwrap().into_movable().unwrap()
    };
    let moved = sealed.move_no_replace(&destination, name).unwrap();
    moved.require_source_absent().unwrap();
    moved.identity().unwrap()
}

#[test]
fn recovery_preserves_original_identity_bytes_and_bound_without_creator_authority() {
    let temporary = tempfile::tempdir().unwrap();
    let identity = publish(temporary.path(), "program", b"native", true);
    let directory = ReaderDirectory::open(temporary.path().join("installed")).unwrap();
    let mut executable = directory.open_sealed_executable("program", 6).unwrap();
    assert_eq!(executable.identity().unwrap(), identity);
    assert_eq!(executable.len().unwrap(), 6);
    assert!(!executable.is_empty().unwrap());
    let before = executable.snapshot().unwrap();
    let mut bytes = Vec::new();
    executable.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"native");
    assert_eq!(executable.seek(SeekFrom::End(-3)).unwrap(), 3);
    assert_eq!(executable.seek(SeekFrom::Current(-1)).unwrap(), 2);
    assert!(executable.seek(SeekFrom::Start(7)).is_err());
    assert!(executable.seek(SeekFrom::Current(-3)).is_err());
    assert_eq!(executable.snapshot().unwrap(), before);
    assert!(directory.open_sealed_executable("program", 5).is_err());
    for name in ["", ".", "..", "../program", "nested/program"] {
        assert!(directory.open_sealed_executable(name, 6).is_err());
    }
    assert_eq!(
        directory
            .open_sealed_executable("missing", 6)
            .unwrap_err()
            .kind(),
        io::ErrorKind::NotFound
    );
    // The descendant retains the original ancestry after the directory wrapper drops.
    drop(directory);
    executable.rewind().unwrap();
    executable.revalidate().unwrap();
    assert_eq!(executable.identity().unwrap(), identity);
}

#[test]
fn empty_extent_is_bounded_without_claiming_valid_executable_contents() {
    let temporary = tempfile::tempdir().unwrap();
    publish(temporary.path(), "empty", b"", true);
    let directory = ReaderDirectory::open(temporary.path().join("installed")).unwrap();
    let mut executable = directory.open_sealed_executable("empty", 0).unwrap();
    assert!(executable.is_empty().unwrap());
    assert_eq!(executable.read(&mut [0]).unwrap(), 0);
    assert_eq!(executable.seek(SeekFrom::End(0)).unwrap(), 0);
    assert!(executable.seek(SeekFrom::Start(1)).is_err());
}

#[test]
fn recovery_refuses_writable_and_receipt_access_without_repair() {
    let temporary = tempfile::tempdir().unwrap();
    publish(temporary.path(), "receipt", b"receipt", false);
    let path = temporary.path().join("installed");
    {
        let owner = OwnerDirectory::open(&path).unwrap();
        let mut pending = owner.create_retained_private("writable", 8).unwrap();
        pending.write_all(b"writable").unwrap();
    }
    let directory = ReaderDirectory::open(&path).unwrap();
    for (name, expected) in [
        ("receipt", b"receipt".as_slice()),
        ("writable", b"writable"),
    ] {
        let original = directory.open_retained_private(name).unwrap();
        let before = original.snapshot().unwrap();
        assert!(directory.open_sealed_executable(name, 8).is_err());
        assert_eq!(original.snapshot().unwrap(), before);
        assert_eq!(std::fs::read(path.join(name)).unwrap(), expected);
    }
}

#[test]
fn recovery_refuses_shared_file_identity() {
    let temporary = tempfile::tempdir().unwrap();
    publish(temporary.path(), "program", b"native", true);
    let path = temporary.path().join("installed");
    #[cfg(windows)]
    let restore = allow_owner_hardlink_fixture(path.join("program"));
    std::fs::hard_link(path.join("program"), path.join("alias")).unwrap();
    #[cfg(windows)]
    restore();
    let directory = ReaderDirectory::open(&path).unwrap();
    assert!(directory.open_sealed_executable("program", 6).is_err());
    assert!(directory.open_sealed_executable("alias", 6).is_err());
    assert_eq!(std::fs::read(path.join("program")).unwrap(), b"native");
    std::fs::remove_file(path.join("alias")).unwrap();
    // Admission succeeds once the genuine extra link is removed: the restored
    // exact executable seal, rather than an incorrect ACL, was tested above.
    directory.open_sealed_executable("program", 6).unwrap();
}

#[cfg(windows)]
fn allow_owner_hardlink_fixture(path: PathBuf) -> impl FnOnce() {
    let identity = Command::new("whoami")
        .args(["/user", "/fo", "csv", "/nh"])
        .output()
        .unwrap();
    assert!(identity.status.success());
    let identity = String::from_utf8_lossy(&identity.stdout);
    let sid = identity
        .rsplit(',')
        .next()
        .unwrap()
        .trim()
        .trim_matches('"');
    assert!(sid.starts_with("S-1-"));
    assert!(
        sid.bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'S' | b'-'))
    );
    let sid = sid.to_owned();
    let set = move |access: &str| {
        let output = Command::new("icacls")
            .arg(&path)
            .arg("/grant:r")
            .arg(format!("*{sid}:{access}"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "current-owner hardlink fixture ACL must apply: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    };
    // CreateHardLink requires mutation access to the existing file. Grant it only
    // to the exact current user, then restore sole-owner RX before any admission.
    set("F");
    move || set("RX")
}

#[test]
fn recovered_original_stays_live_through_real_process_execution_and_receipt_refusal() {
    let temporary = tempfile::tempdir().unwrap();
    let source_code = temporary.path().join("probe.rs");
    std::fs::write(
        &source_code,
        b"//! Actual retained executable process probe.\nfn main() { print!(\"retained-executable-probe\"); std::process::exit(17); }\n",
    )
    .unwrap();
    let compiled = temporary.path().join(if cfg!(windows) {
        "compiled.exe"
    } else {
        "compiled"
    });
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args([
            "--edition=2024",
            "--crate-type=bin",
            "--crate-name=retained_executable_probe",
            "-C",
            "debuginfo=0",
        ])
        .arg(&source_code)
        .arg("-o")
        .arg(&compiled)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "native probe must compile: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = std::fs::read(&compiled).unwrap();
    let name = if cfg!(windows) { "probe.exe" } else { "probe" };
    let identity = publish(temporary.path(), name, &bytes, true);
    let directory = ReaderDirectory::open(temporary.path().join("installed")).unwrap();
    let mut executable = directory.open_sealed_executable(name, bytes.len()).unwrap();
    let snapshot = executable.snapshot().unwrap();
    assert_eq!(executable.identity().unwrap(), identity);
    let mut actual = Vec::new();
    executable.read_to_end(&mut actual).unwrap();
    assert_eq!(actual, bytes);
    executable.rewind().unwrap();

    // Both original owners remain live throughout genuine kernel process loading.
    // The ordinary parent is not sealed and this is not release-bundle admission.
    let process = Command::new(directory.path().join(name)).output().unwrap();
    assert_eq!(process.status.code(), Some(17));
    assert_eq!(process.stdout, b"retained-executable-probe");
    assert!(process.stderr.is_empty());
    directory.revalidate().unwrap();
    assert_eq!(executable.identity().unwrap(), identity);
    assert_eq!(executable.snapshot().unwrap(), snapshot);
    actual.clear();
    executable.read_to_end(&mut actual).unwrap();
    assert_eq!(actual, bytes);
    assert_eq!(executable.snapshot().unwrap(), snapshot);

    let receipt = if cfg!(windows) {
        "receipt.exe"
    } else {
        "receipt"
    };
    publish(temporary.path(), receipt, &bytes, false);
    assert!(
        directory
            .open_sealed_executable(receipt, bytes.len())
            .is_err()
    );
    assert_eq!(
        Command::new(directory.path().join(receipt))
            .output()
            .unwrap_err()
            .kind(),
        io::ErrorKind::PermissionDenied
    );
    assert_eq!(executable.snapshot().unwrap(), snapshot);
}

#[cfg(unix)]
#[test]
fn original_refuses_access_drift_even_after_exact_mode_is_restored() {
    use std::os::unix::fs::PermissionsExt as _;
    let temporary = tempfile::tempdir().unwrap();
    publish(temporary.path(), "program", b"native", true);
    let directory = ReaderDirectory::open(temporary.path().join("installed")).unwrap();
    let mut executable = directory.open_sealed_executable("program", 6).unwrap();
    let path = directory.path().join("program");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(executable.read(&mut [0]).is_err());
    assert!(executable.seek(SeekFrom::Start(0)).is_err());
    assert!(executable.identity().is_err());
    assert!(directory.open_sealed_executable("program", 6).is_err());
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o500)).unwrap();
    assert!(executable.revalidate().is_err());
    assert!(executable.snapshot().is_err());
    assert!(executable.len().is_err());
    assert!(executable.is_empty().is_err());
    directory.open_sealed_executable("program", 6).unwrap();
}

#[cfg(unix)]
#[test]
fn original_refuses_equal_byte_replacement_and_displaced_ancestry() {
    let temporary = tempfile::tempdir().unwrap();
    publish(temporary.path(), "program", b"native", true);
    let directory = ReaderDirectory::open(temporary.path().join("installed")).unwrap();
    let executable = directory.open_sealed_executable("program", 6).unwrap();
    std::fs::rename(
        directory.path().join("program"),
        directory.path().join("preserved"),
    )
    .unwrap();
    publish(temporary.path(), "program", b"native", true);
    assert!(executable.revalidate().is_err());
    let replacement = directory.open_sealed_executable("program", 6).unwrap();
    std::fs::rename(directory.path(), temporary.path().join("displaced")).unwrap();
    assert!(replacement.revalidate().is_err());
    assert!(directory.open_sealed_executable("program", 6).is_err());
    assert_eq!(
        std::fs::read(temporary.path().join("displaced/program")).unwrap(),
        b"native"
    );
}

#[cfg(unix)]
#[test]
fn recovery_refuses_real_symlink_leaf_and_ancestor() {
    use std::os::unix::fs::symlink;
    let temporary = tempfile::tempdir().unwrap();
    publish(temporary.path(), "program", b"native", true);
    let path = temporary.path().join("installed");
    symlink(path.join("program"), path.join("alias")).unwrap();
    let directory = ReaderDirectory::open(&path).unwrap();
    assert!(directory.open_sealed_executable("alias", 6).is_err());
    symlink(&path, temporary.path().join("redirected")).unwrap();
    assert!(ReaderDirectory::open(temporary.path().join("redirected")).is_err());
    directory.open_sealed_executable("program", 6).unwrap();
}

#[cfg(windows)]
fn grant(path: &Path, access: &str) {
    let output = Command::new("icacls")
        .arg(path)
        .arg("/grant")
        .arg(format!("*S-1-1-0:{access}"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "icacls must apply the genuine ACL control: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(windows)]
#[test]
fn recovered_native_handle_excludes_writers_even_after_acl_grants_write() {
    let temporary = tempfile::tempdir().unwrap();
    publish(temporary.path(), "program", b"native", true);
    let directory = ReaderDirectory::open(temporary.path().join("installed")).unwrap();
    let executable = directory.open_sealed_executable("program", 6).unwrap();
    let path = directory.path().join("program");
    grant(&path, "W");
    assert_eq!(
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap_err()
            .raw_os_error(),
        Some(32),
        "FILE_SHARE_READ must exclude writing even when the DACL now permits it"
    );
    assert!(executable.revalidate().is_err());
    assert!(directory.open_sealed_executable("program", 6).is_err());
    drop(executable);
    std::fs::OpenOptions::new().write(true).open(&path).unwrap();
}

#[cfg(windows)]
#[test]
fn recovered_descendant_pins_original_leaf_and_ancestry_until_dropped() {
    let temporary = tempfile::tempdir().unwrap();
    publish(temporary.path(), "program", b"native", true);
    let path = temporary.path().join("installed");
    let directory = ReaderDirectory::open(&path).unwrap();
    let executable = directory.open_sealed_executable("program", 6).unwrap();
    let before = executable.snapshot().unwrap();
    assert!(std::fs::remove_file(path.join("program")).is_err());
    assert!(std::fs::rename(path.join("program"), path.join("moved")).is_err());
    drop(directory);
    assert!(std::fs::rename(&path, temporary.path().join("displaced")).is_err());
    assert_eq!(executable.snapshot().unwrap(), before);
    drop(executable);
    std::fs::rename(&path, temporary.path().join("displaced")).unwrap();
}

#[cfg(windows)]
#[test]
fn recovery_refuses_genuine_junction_ancestry_and_extra_read_acl_without_repair() {
    let temporary = tempfile::tempdir().unwrap();
    publish(temporary.path(), "program", b"native", true);
    let path = temporary.path().join("installed");
    let redirected = temporary.path().join("redirected");
    let output = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&redirected)
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "genuine directory junction must be created: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(ReaderDirectory::open(&redirected).is_err());
    let directory = ReaderDirectory::open(&path).unwrap();
    let executable = directory.open_sealed_executable("program", 6).unwrap();
    grant(&path.join("program"), "R");
    let before = FileSnapshot::of(&File::open(path.join("program")).unwrap(), false).unwrap();
    assert!(executable.revalidate().is_err());
    assert!(directory.open_sealed_executable("program", 6).is_err());
    assert_eq!(
        FileSnapshot::of(&File::open(path.join("program")).unwrap(), false).unwrap(),
        before
    );
    assert_eq!(std::fs::read(path.join("program")).unwrap(), b"native");
}
