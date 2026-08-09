// Atomic storage path-substitution regressions.

#[test]
fn write_atomic_rejects_symlink_parent() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let temp_path = canonical_temp_path(&temp_dir);
    let real_dir = temp_path.join("real");
    fs::create_dir(&real_dir).expect("create real dir");
    let linked_dir = temp_path.join("linked");
    std::os::unix::fs::symlink(&real_dir, &linked_dir).expect("create symlink");
    let output_path = linked_dir.join("index.norito");

    let err = write_atomic(&output_path, b"replace").expect_err("reject symlink parent");
    let message = err.to_string();

    assert!(
        message.contains("parent") && message.contains("must not be a symlink"),
        "unexpected error: {message}"
    );
    assert!(
        !real_dir.join("index.norito").exists(),
        "symlink parent should not receive output"
    );
}

#[cfg(unix)]
#[test]
fn open_atomic_temp_file_rejects_preexisting_symlink() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let temp_path = canonical_temp_path(&temp_dir);
    let target_path = temp_path.join("target.tmp");
    fs::write(&target_path, b"unchanged\n").expect("write target");
    let tmp_path = temp_path.join("index.norito.tmp");
    std::os::unix::fs::symlink(&target_path, &tmp_path).expect("create symlink");

    let err = open_atomic_temp_file(&tmp_path).expect_err("reject temp symlink");
    let message = err.to_string();

    assert!(
        message.contains("failed to create atomic temp"),
        "unexpected error: {message}"
    );
    assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged\n");
}
