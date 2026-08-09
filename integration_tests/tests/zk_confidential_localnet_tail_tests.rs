#[test]
fn localnet_lifecycle_recorder_rejects_unsafe_source_dirs_before_write() {
    let base = unique_temp_dir("kagemusha-localnet-lifecycle-recorder-unsafe");
    let cases = [
        base.join("bad\nsource"),
        base.join("..").join("source"),
        base.join("bad\\source"),
    ];

    for root in cases {
        let recorder = LocalnetLifecycleArtifactRecorder {
            root: Some(root.clone()),
            run_id: "production-4-peer-localnet-unit".to_owned(),
            chain_id: "kagemusha-production-localnet-unit".to_owned(),
            peer_ids: vec![
                "peer-0@production-localnet".to_owned(),
                "peer-1@production-localnet".to_owned(),
                "peer-2@production-localnet".to_owned(),
                "peer-3@production-localnet".to_owned(),
            ],
        };

        let err = recorder
            .record_tx("lifecycle_shield_tx_artifact", "unit shield", "abc123", 7)
            .expect_err("unsafe source root must reject before writing");

        assert!(
            err.to_string()
                .contains("must be a canonical local source directory"),
            "unexpected error for {root:?}: {err}",
        );
        assert!(
            !root.join("lifecycle-shield-tx-artifact.json").exists(),
            "unsafe source root must not receive an artifact",
        );
    }

    let _ = fs::remove_dir_all(base);
}

#[cfg(unix)]
#[test]
fn localnet_lifecycle_recorder_rejects_symlink_source_dir_before_write() {
    let base = unique_temp_dir("kagemusha-localnet-lifecycle-recorder-symlink");
    let real_root = base.join("real");
    let link_root = base.join("link");
    fs::create_dir_all(&real_root).expect("create real source root");
    std::os::unix::fs::symlink(&real_root, &link_root).expect("create source root symlink");
    let recorder = LocalnetLifecycleArtifactRecorder {
        root: Some(link_root.clone()),
        run_id: "production-4-peer-localnet-unit".to_owned(),
        chain_id: "kagemusha-production-localnet-unit".to_owned(),
        peer_ids: vec![
            "peer-0@production-localnet".to_owned(),
            "peer-1@production-localnet".to_owned(),
            "peer-2@production-localnet".to_owned(),
            "peer-3@production-localnet".to_owned(),
        ],
    };

    let err = recorder
        .record_tx("lifecycle_shield_tx_artifact", "unit shield", "abc123", 7)
        .expect_err("symlink source root must reject before writing");

    assert!(
        err.to_string().contains("must not be a symlink"),
        "unexpected symlink source root error: {err}",
    );
    assert!(!real_root.join("lifecycle-shield-tx-artifact.json").exists());
    let _ = fs::remove_dir_all(base);
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let millis = unix_time_ms();
    let base = env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("target")
        .join("kagemusha-test-tmp");
    fs::create_dir_all(&base).expect("create local test temp root");
    base.join(format!("{name}-{}-{millis}", std::process::id()))
}

#[test]
fn corrupted_proof_helper_mutates_proof_bytes() {
    let seed = marker(222);
    let original = live_halo2_attachment(seed);
    let tampered = attachment_with_corrupted_proof(seed);
    assert_ne!(tampered.proof.bytes, original.proof.bytes);
}

#[test]
fn corrupted_vk_helper_mutates_vk_bytes() {
    let seed = marker(223);
    let original = live_halo2_attachment(seed);
    let tampered = attachment_with_corrupted_vk(seed);
    assert_ne!(tampered.vk_ref, original.vk_ref);
}
