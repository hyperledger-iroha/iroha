//! Argument refusal, native locking and bounded acknowledgement controls.

use super::*;
use iroha_fs::PublishMode;

#[test]
fn configuration_cwd_binding_retains_original_source_after_configuration_drop() {
    let temporary = tempfile::tempdir().unwrap();
    let source = PrivateDirectory::open_or_create(temporary.path().join("source")).unwrap();
    let cargo_home = PrivateDirectory::open_or_create(temporary.path().join("cargo")).unwrap();
    let source_path = source.path().to_owned();
    let configuration = policy::Configuration::open(&source_path, cargo_home.path()).unwrap();
    let mut command =
        iroha_fs::ProcessCommand::new(temporary.path().join("missing-image")).unwrap();
    command.current_dir(temporary.path().join("unselected"));
    configuration.bind_current_directory(&mut command).unwrap();
    assert_eq!(command.get_current_dir(), Some(source_path.as_path()));
    drop(configuration);
    drop(source);
    #[cfg(unix)]
    {
        std::fs::rename(&source_path, temporary.path().join("displaced")).unwrap();
        std::fs::create_dir(&source_path).unwrap();
        let error = command.spawn().unwrap_err();
        assert_ne!(
            error.kind(),
            io::ErrorKind::NotFound,
            "custody must refuse before missing image lookup"
        );
    }
    #[cfg(windows)]
    {
        assert!(std::fs::rename(&source_path, temporary.path().join("displaced")).is_err());
        drop(command);
        std::fs::rename(&source_path, temporary.path().join("displaced")).unwrap();
    }
}

#[test]
fn parser_rejects_duplicate_unknown_missing_and_ambiguous_options() {
    for values in [
        vec!["--a", "x", "--a", "y"],
        vec!["--a"],
        vec!["--a=x", "y"],
        vec!["a", "x"],
    ] {
        assert!(Arguments::parse(values.into_iter().map(OsString::from)).is_err());
    }
    assert!(
        Arguments::parse([OsString::from("--unknown"), OsString::from("value")].into_iter())
            .unwrap()
            .finish()
            .is_err()
    );
    for name in [
        "invocation.12345",
        "invocation.1234567",
        "invocation.abc/12",
        "other.123456",
    ] {
        assert!(invocation_name(name.into()).is_err());
    }
    assert_eq!(
        invocation_name("invocation.Abc123".into()).unwrap(),
        "invocation.Abc123"
    );
    for digest in ["a".repeat(63), "A".repeat(64), "g".repeat(64)] {
        assert!(digest_argument(digest.into()).is_err());
    }
}

#[test]
fn exact_paths_and_disjoint_roots_reject_ambiguous_namespace() {
    assert!(exact_path("relative".into()).is_err());
    let temporary = tempfile::tempdir().unwrap();
    assert!(exact_path(temporary.path().join("..").join("other").into_os_string()).is_err());
    assert!(exact_path(temporary.path().join("line\nfeed").into_os_string()).is_err());
    for spelling in [
        format!("{}/./child", temporary.path().display()),
        format!("{}//child", temporary.path().display()),
    ] {
        assert!(exact_path(spelling.into()).is_err());
    }
    assert!(policy::disjoint(&[temporary.path(), &temporary.path().join("child")]).is_err());
    policy::disjoint(&[&temporary.path().join("one"), &temporary.path().join("two")]).unwrap();
    let one = PrivateDirectory::open_or_create(temporary.path().join("one")).unwrap();
    let two = PrivateDirectory::open_or_create(temporary.path().join("two")).unwrap();
    let child = one.create_child("child").unwrap();
    assert!(policy::DisjointRoots::open(&[one.path(), child.path()]).is_err());
    assert!(policy::DisjointRoots::open(&[one.path(), one.path()]).is_err());
    policy::DisjointRoots::open(&[one.path(), two.path()])
        .unwrap()
        .revalidate()
        .unwrap();
}

#[test]
fn private_lock_contention_preserves_original_and_releases_only_on_drop() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
    let first = policy::Lock::acquire(directory.path(), "lock").unwrap();
    let identity = iroha_fs::FileMetadata::symlink_metadata(directory.path().join("lock"))
        .unwrap()
        .identity();
    assert_eq!(
        policy::Lock::acquire(directory.path(), "lock")
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::WouldBlock
    );
    first.revalidate().unwrap();
    drop(first);
    let second = policy::Lock::acquire(directory.path(), "lock").unwrap();
    second.revalidate().unwrap();
    assert_eq!(
        iroha_fs::FileMetadata::symlink_metadata(directory.path().join("lock"))
            .unwrap()
            .identity(),
        identity
    );
}

#[test]
fn cancellation_requires_exact_private_request_and_never_changes_marker() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = PrivateDirectory::open_or_create(temporary.path().join("cancel")).unwrap();
    let policy = policy::Cancellation::open(&directory.path().join("request")).unwrap();
    assert!(policy.check().is_ok());
    directory
        .write_atomic("request", b"{}\n", PublishMode::CreateNew)
        .unwrap();
    assert_eq!(policy.check().err().unwrap().exit_code(), 2);
    let request = b"{\"reason\":\"operator-request\",\"schema_version\":1}\n";
    directory
        .write_atomic("request", request, PublishMode::Replace)
        .unwrap();
    assert_eq!(policy.check().err().unwrap().exit_code(), 125);
    assert_eq!(directory.read("request", 128).unwrap().as_slice(), request);
}

#[test]
fn command_refusal_has_no_publication_acknowledgement() {
    for command in [
        vec!["unknown"],
        vec!["admit"],
        vec!["--help", "unexpected"],
        vec!["publish", "--name", "invocation.123456"],
    ] {
        let mut output = Vec::new();
        assert!(run(command.into_iter().map(OsString::from), &mut output).is_err());
        assert!(output.is_empty());
    }
}

#[test]
fn cargo_lock_hash_is_bounded_original_bytes() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("Cargo.lock");
    std::fs::write(&path, b"version = 4\n").unwrap();
    assert_eq!(
        lock_digest(&path).unwrap().2,
        hex::encode(Sha256::digest(b"version = 4\n"))
    );
    std::fs::hard_link(&path, temporary.path().join("alias")).unwrap();
    assert!(lock_digest(&path).is_err());
}
