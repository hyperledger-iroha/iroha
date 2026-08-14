// Restart-storage coverage for non-bootstrap network-peer preparation.
#[test]
fn kura_storage_dir_is_not_cleared_on_restart_when_genesis_is_provided() -> Result<()> {
    assert!(NetworkPeer::should_reset_kura_for_bootstrap(true, 1));
    assert!(!NetworkPeer::should_reset_kura_for_bootstrap(true, 2));
    assert!(!NetworkPeer::should_reset_kura_for_bootstrap(false, 1));
    assert!(!NetworkPeer::should_reset_kura_for_bootstrap(false, 2));
    Ok(())
}
#[test]
fn kura_storage_dir_is_not_cleared_when_reset_for_bootstrap_is_false() -> Result<()> {
    let root = tempdir()?;
    let env = Environment {
        dir: root.path().to_path_buf(),
    };
    let peer = NetworkPeer::builder().build(&env);
    let storage_dir = peer.dir.join("storage");
    fs::create_dir_all(&storage_dir)?;
    fs::write(storage_dir.join("keep.marker"), b"keep")?;
    peer.prepare_kura_storage_dir(&storage_dir, false)?;
    assert!(
        storage_dir.join("keep.marker").exists(),
        "restart preparation must not clear existing storage"
    );
    Ok(())
}
