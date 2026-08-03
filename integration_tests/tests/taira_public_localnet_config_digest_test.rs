#[test]
fn generated_config_digest_tracks_config_but_not_runtime_logs() {
    let temp = tempfile::tempdir().expect("temporary config directory");
    fs::write(temp.path().join("peer0.toml"), "chain = 'a'\n").expect("write config");
    fs::write(temp.path().join("peer0.log"), "startup\n").expect("write log");
    fs::create_dir_all(temp.path().join("storage")).expect("create storage");
    fs::write(temp.path().join("storage/block.json"), "dynamic\n").expect("write storage");

    let first = generated_config_blake2b_256(temp.path()).expect("digest generated config");
    fs::write(temp.path().join("peer0.log"), "different log\n").expect("update log");
    fs::write(
        temp.path().join("storage/block.json"),
        "different storage\n",
    )
    .expect("update storage");
    assert_eq!(
        first,
        generated_config_blake2b_256(temp.path()).expect("digest without runtime artifacts")
    );

    fs::write(temp.path().join("peer0.toml"), "chain = 'b'\n").expect("update config");
    assert_ne!(
        first,
        generated_config_blake2b_256(temp.path()).expect("digest changed config")
    );
}
