//! Actual retained working-directory selection, replacement and original-child lifetime controls.

use super::*;
use crate::{PrivateDirectory, ReaderDirectory};
use std::time::{Duration, Instant};

fn directory(root: &Path, name: &str, marker: &[u8]) -> Arc<ReaderDirectory> {
    let private = PrivateDirectory::open_or_create(root.join(name)).unwrap();
    fs::write(private.path().join("marker"), marker).unwrap();
    Arc::new(ReaderDirectory::open(private.path()).unwrap())
}

#[test]
fn retained_cwd_selects_original_after_caller_drop_without_changing_parent() {
    let root = tempfile::tempdir().unwrap();
    let original = directory(root.path(), "selected", b"original-directory\0bytes");
    let parent = std::env::current_dir().unwrap();
    let weak = Arc::downgrade(&original);
    let mut child = command("cwd");
    child.current_dir_retained(&original).unwrap();
    assert_eq!(child.get_current_dir(), Some(original.path()));
    drop(original);
    assert!(weak.upgrade().is_some());
    let output = child.output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"original-directory\0bytes");
    assert!(output.stderr.is_empty());
    assert_eq!(std::env::current_dir().unwrap(), parent);
    assert!(weak.upgrade().is_none());
}

#[test]
fn last_named_or_retained_cwd_setting_selects_exactly_one_directory() {
    let root = tempfile::tempdir().unwrap();
    let first = directory(root.path(), "first", b"first");
    let second = directory(root.path(), "second", b"second");
    let mut named = command("cwd");
    named
        .current_dir_retained(&first)
        .unwrap()
        .current_dir(second.path());
    assert_eq!(named.get_current_dir(), Some(second.path()));
    let output = named.output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"second");
    let mut retained = command("cwd");
    retained
        .current_dir(first.path())
        .current_dir_retained(&second)
        .unwrap();
    assert_eq!(retained.get_current_dir(), Some(second.path()));
    let output = retained.output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"second");
}

#[test]
fn retained_cwd_stays_with_original_child_and_background_reaper() {
    let root = tempfile::tempdir().unwrap();
    let original = directory(root.path(), "selected", b"selected");
    let weak = Arc::downgrade(&original);
    let mut selected = command("wait");
    selected
        .kill_on_drop(false)
        .current_dir_retained(&original)
        .unwrap()
        .stdin(ProcessStdio::piped())
        .stdout(ProcessStdio::piped());
    let mut child = selected.spawn().unwrap();
    drop(original);
    let mut output = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    assert_eq!(line, "ready\n");
    let input = child.stdin.take().unwrap();
    assert!(child.try_wait().unwrap().is_none());
    drop(child);
    assert!(weak.upgrade().is_some(), "live original still owns cwd");
    drop(input);
    let mut remainder = Vec::new();
    output.read_to_end(&mut remainder).unwrap();
    assert_eq!(remainder, b"eof\n");
    let deadline = Instant::now() + Duration::from_secs(5);
    while weak.upgrade().is_some() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(weak.upgrade().is_none(), "terminal reaper releases cwd");
}

#[cfg(all(unix, not(target_os = "fuchsia")))]
#[test]
fn changed_cwd_ancestry_refuses_before_native_launch() {
    let root = tempfile::tempdir().unwrap();
    let original = directory(root.path(), "selected", b"original");
    let path = original.path().to_owned();
    let mut selected = command("cwd");
    selected.current_dir_retained(&original).unwrap();
    fs::rename(&path, root.path().join("displaced")).unwrap();
    fs::create_dir(&path).unwrap();
    fs::write(path.join("marker"), b"competing").unwrap();
    assert!(selected.spawn().is_err());
    assert!(original.revalidate().is_err());
    assert!(command("cwd").current_dir_retained(&original).is_err());
}

#[cfg(all(unix, not(target_os = "fuchsia")))]
#[test]
fn prepared_cwd_descriptor_cannot_be_redirected_between_admission_and_spawn() {
    let root = tempfile::tempdir().unwrap();
    let original = directory(root.path(), "selected", b"original");
    let path = original.path().to_owned();
    let displaced = root.path().join("displaced");
    let parent = std::env::current_dir().unwrap();
    let mut selected = NativeCommand::new(image());
    selected.arg("cwd");
    let held = prepare_current_directory(&mut selected, Some(CurrentDirectory::Retained(original)))
        .unwrap();
    fs::rename(&path, &displaced).unwrap();
    fs::create_dir(&path).unwrap();
    fs::write(path.join("marker"), b"competing").unwrap();
    let mut named = NativeCommand::new(image());
    named.arg("cwd").current_dir(&path);
    let wrong = named.output().unwrap();
    let right = selected.output().unwrap();
    assert!(wrong.status.success());
    assert!(right.status.success());
    assert_eq!(wrong.stdout, b"competing");
    assert_eq!(right.stdout, b"original");
    assert_eq!(std::env::current_dir().unwrap(), parent);
    assert!(held.unwrap().revalidate().is_err());
    fs::remove_dir_all(&path).unwrap();
    fs::rename(&displaced, &path).unwrap();
}

#[cfg(all(unix, not(target_os = "fuchsia")))]
#[test]
#[allow(
    unsafe_code,
    reason = "the test-only child setup callback returns a fixed native error without allocation"
)]
fn retained_cwd_preserves_exec_and_child_setup_error_channels() {
    use std::os::unix::process::CommandExt as _;
    let root = tempfile::tempdir().unwrap();
    let original = directory(root.path(), "selected", b"selected");
    let mut missing = ProcessCommand::new(root.path().join("missing-image")).unwrap();
    missing.current_dir_retained(&original).unwrap();
    assert_eq!(missing.spawn().unwrap_err().kind(), io::ErrorKind::NotFound);
    let mut selected = NativeCommand::new(image());
    selected.arg("cwd");
    let _held =
        prepare_current_directory(&mut selected, Some(CurrentDirectory::Retained(original)))
            .unwrap();
    // SAFETY: this callback only returns a fixed native error after the preceding
    // fchdir callback; it performs no allocation, lock or destructor after fork.
    unsafe {
        selected.pre_exec(|| Err(io::Error::from_raw_os_error(5)));
    }
    assert_eq!(selected.spawn().unwrap_err().raw_os_error(), Some(5));
}

#[cfg(all(unix, not(target_os = "fuchsia")))]
#[test]
fn retained_cwd_returns_actual_fchdir_permission_failure_before_image_execution() {
    use std::os::unix::fs::PermissionsExt as _;
    assert_ne!(
        rustix::process::geteuid().as_raw(),
        0,
        "this native permission-refusal control requires a non-root runner"
    );
    let root = tempfile::tempdir().unwrap();
    let original = directory(root.path(), "selected", b"selected");
    let path = original.path().to_owned();
    let parent = std::env::current_dir().unwrap();
    let marker = root.path().join("executed-marker");
    let mut selected = NativeCommand::new(image());
    selected.arg("mark-cwd").arg(&marker);
    let held = prepare_current_directory(&mut selected, Some(CurrentDirectory::Retained(original)))
        .unwrap();
    let permissions = fs::metadata(&path).unwrap().permissions();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    // Deliberately bypass the already completed parent preparation: the actual
    // child-side fchdir must refuse the retained directory's missing search right.
    let result = selected.output();
    fs::set_permissions(&path, permissions).unwrap();
    held.unwrap().revalidate().unwrap();
    let error = result.unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert_eq!(
        error.raw_os_error(),
        Some(rustix::io::Errno::ACCESS.raw_os_error())
    );
    assert!(!marker.exists(), "the executable never ran");
    assert_eq!(std::env::current_dir().unwrap(), parent);
}
