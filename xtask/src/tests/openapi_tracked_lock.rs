#[test]
fn openapi_generator_tracked_lock_fails_closed_when_missing_substituted_or_unstaged() {
    let tmp = tempdir().expect("tempdir");
    initialize_git_fixture(tmp.path());
    let lock = tmp.path().join(OPENAPI_GENERATOR_TRACKED_INPUT);
    let head = std::str::from_utf8(
        &git_stdout(tmp.path(), &["rev-parse", "--verify", "HEAD"]).expect("fixture HEAD"),
    )
    .expect("UTF-8 fixture HEAD")
    .trim()
    .to_owned();
    fs::remove_file(&lock).expect("remove tracked Cargo lock");
    let missing = git_openapi_generator_input_tree_sha256(tmp.path(), &head)
        .expect_err("missing tracked Cargo lock must fail");
    assert!(
        missing
            .to_string()
            .contains("tracked OpenAPI generator Cargo lock"),
        "unexpected missing-lock error: {missing}"
    );
    let canonical = fs::read(workspace_root().join(OPENAPI_GENERATOR_TRACKED_INPUT))
        .expect("read canonical tracked Cargo lock");
    fs::write(&lock, &canonical).expect("restore tracked Cargo lock");
    let mut substituted = canonical.clone();
    substituted[0] ^= 1;
    fs::write(&lock, substituted).expect("write substituted tracked Cargo lock");
    let wrong_digest = git_openapi_generator_input_tree_sha256(tmp.path(), &head)
        .expect_err("substituted tracked Cargo lock must fail");
    assert!(
        wrong_digest
            .to_string()
            .contains("working bytes must equal the authenticated Git blob"),
        "unexpected substituted-lock error: {wrong_digest}"
    );
    fs::write(&lock, canonical).expect("restore tracked Cargo lock again");
    git_stdout(
        tmp.path(),
        &[
            "rm",
            "--cached",
            "--quiet",
            "--",
            OPENAPI_GENERATOR_TRACKED_INPUT,
        ],
    )
    .expect("remove tracked Cargo lock from the index");
    let unstaged = git_openapi_generator_input_tree_sha256(tmp.path(), &head)
        .expect_err("unstaged Cargo lock must fail");
    assert!(
        unstaged.to_string().contains("stage-zero 100644 blob"),
        "unexpected unstaged-lock error: {unstaged}"
    );
}

#[test]
fn openapi_generator_tracked_lock_requires_the_head_blob_in_the_index() {
    let tmp = tempdir().expect("tempdir");
    initialize_git_fixture(tmp.path());
    fs::write(
        tmp.path().join(OPENAPI_GENERATOR_TRACKED_INPUT),
        b"substituted\n",
    )
    .expect("substitute tracked Cargo lock");
    git_stdout(tmp.path(), &["add", "--", OPENAPI_GENERATOR_TRACKED_INPUT])
        .expect("stage substituted Cargo lock");
    let head = std::str::from_utf8(
        &git_stdout(tmp.path(), &["rev-parse", "--verify", "HEAD"]).expect("fixture HEAD"),
    )
    .expect("UTF-8 fixture HEAD")
    .trim()
    .to_owned();
    let err = git_openapi_generator_input_tree_sha256(tmp.path(), &head)
        .expect_err("index and HEAD Cargo.lock mismatch must fail");
    assert!(
        err.to_string().contains("same blob in the index"),
        "unexpected index-lock error: {err}"
    );
}

#[cfg(unix)]
#[test]
fn openapi_generator_tracked_lock_rejects_executable_symlink_and_hardlink_inputs() {
    use std::os::unix::fs::{PermissionsExt as _, symlink};
    let executable_fixture = tempdir().expect("executable tempdir");
    initialize_git_fixture(executable_fixture.path());
    let executable_lock = executable_fixture
        .path()
        .join(OPENAPI_GENERATOR_TRACKED_INPUT);
    fs::set_permissions(&executable_lock, fs::Permissions::from_mode(0o700))
        .expect("make tracked Cargo lock executable");
    let executable_head = std::str::from_utf8(
        &git_stdout(
            executable_fixture.path(),
            &["rev-parse", "--verify", "HEAD"],
        )
        .expect("fixture HEAD"),
    )
    .expect("UTF-8 fixture HEAD")
    .trim()
    .to_owned();
    let executable =
        git_openapi_generator_input_tree_sha256(executable_fixture.path(), &executable_head)
            .expect_err("executable tracked Cargo lock must fail");
    assert!(executable.to_string().contains("must not be executable"));

    let symlink_fixture = tempdir().expect("symlink tempdir");
    initialize_git_fixture(symlink_fixture.path());
    let symlink_lock = symlink_fixture.path().join(OPENAPI_GENERATOR_TRACKED_INPUT);
    fs::remove_file(&symlink_lock).expect("remove tracked Cargo lock");
    fs::write(symlink_fixture.path().join("lock-target"), b"fixture\n")
        .expect("write lock symlink target");
    symlink("lock-target", &symlink_lock).expect("symlink tracked Cargo lock");
    let symlink_head = std::str::from_utf8(
        &git_stdout(symlink_fixture.path(), &["rev-parse", "--verify", "HEAD"])
            .expect("fixture HEAD"),
    )
    .expect("UTF-8 fixture HEAD")
    .trim()
    .to_owned();
    let linked = git_openapi_generator_input_tree_sha256(symlink_fixture.path(), &symlink_head)
        .expect_err("symlinked tracked Cargo lock must fail");
    assert!(linked.to_string().contains("must not be a symlink"));

    let hardlink_fixture = tempdir().expect("hardlink tempdir");
    initialize_git_fixture(hardlink_fixture.path());
    let hardlink_lock = hardlink_fixture
        .path()
        .join(OPENAPI_GENERATOR_TRACKED_INPUT);
    fs::hard_link(&hardlink_lock, hardlink_fixture.path().join("lock-alias"))
        .expect("hard-link tracked Cargo lock");
    let hardlink_head = std::str::from_utf8(
        &git_stdout(hardlink_fixture.path(), &["rev-parse", "--verify", "HEAD"])
            .expect("fixture HEAD"),
    )
    .expect("UTF-8 fixture HEAD")
    .trim()
    .to_owned();
    let hardlinked =
        git_openapi_generator_input_tree_sha256(hardlink_fixture.path(), &hardlink_head)
            .expect_err("hard-linked tracked Cargo lock must fail");
    assert!(hardlinked.to_string().contains("exactly one hard link"));
}
