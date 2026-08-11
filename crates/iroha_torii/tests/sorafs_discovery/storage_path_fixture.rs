fn storage_temp_data_dir(temp_dir: &TempDir) -> PathBuf {
    temp_dir
        .path()
        .canonicalize()
        .expect("canonical storage temp dir")
        .join("storage")
}

#[test]
fn sorafs_storage_temp_data_dir_uses_canonical_parent() {
    let temp_dir = tempdir().expect("storage temp dir");
    let data_dir = storage_temp_data_dir(&temp_dir);
    assert_eq!(
        data_dir.parent().expect("storage path parent"),
        temp_dir.path().canonicalize().expect("canonical temp dir")
    );
}
