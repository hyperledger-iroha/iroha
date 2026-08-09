    #[test]
    fn restart_genesis_file_reuses_latest_run_genesis_when_available() -> Result<()> {
        let root = tempdir()?;
        let env = Environment {
            dir: root.path().to_path_buf(),
        };
        let peer = NetworkPeer::builder().build(&env);

        assert_eq!(peer.restart_genesis_file(false), None);

        let genesis_path = peer.dir.join("run-1-genesis.nrt");
        fs::write(&genesis_path, b"genesis")?;

        assert_eq!(peer.restart_genesis_file(false), Some(genesis_path));
        assert_eq!(peer.restart_genesis_file(true), None);

        Ok(())
    }

    #[test]
    fn restart_genesis_file_skips_failed_early_run_when_later_genesis_exists() -> Result<()> {
        let root = tempdir()?;
        let env = Environment {
            dir: root.path().to_path_buf(),
        };
        let peer = NetworkPeer::builder().build(&env);
        let first_genesis_path = peer.dir.join("run-1-genesis.nrt");
        let later_genesis_path = peer.dir.join("run-3-genesis.nrt");
        fs::write(&first_genesis_path, b"stale genesis")?;
        fs::write(&later_genesis_path, b"latest genesis")?;

        assert_eq!(peer.restart_genesis_file(false), Some(later_genesis_path));

        Ok(())
    }

    #[test]
    fn kura_storage_dir_is_cleared_for_bootstrap_when_reset_for_bootstrap_is_true() -> Result<()> {
        let root = tempdir()?;
        let env = Environment {
            dir: root.path().to_path_buf(),
        };
        let peer = NetworkPeer::builder().build(&env);
        let storage_dir = peer.dir.join("storage");
        fs::create_dir_all(&storage_dir)?;
        fs::write(storage_dir.join("keep.marker"), b"remove")?;

        peer.prepare_kura_storage_dir(&storage_dir, true)?;

        assert!(
            !storage_dir.join("keep.marker").exists(),
            "bootstrap reset must clear stale files"
        );
        assert!(
            storage_dir.exists(),
            "storage directory must exist after preparation"
        );
        Ok(())
    }
