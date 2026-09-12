// Descriptor admission and the unchanged canonical stage decoder share real rewrite fixtures.

fn canonical_stage_identity_fixture() -> (TempDir, Arc<Kura>, Vec<u8>) {
    let (directory, kura, blocks) = canonical_physical_fixture(2);
    let replacement = canonical_physical_block_at(&blocks, 2);
    canonical_physical_leave_rewrite(&kura, &replacement, true);
    let bytes = std::fs::read(kura.block_store.lock().da_block_rewrite_stage_path()).unwrap();
    (directory, kura, bytes)
}

#[cfg(unix)]
fn canonical_stage_fifo_with_live_peer(path: &Path) -> std::fs::File {
    let status = std::process::Command::new("mkfifo")
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
    // Keep both ends present. Even a regression to blocking open cannot strand
    // this test, while admission must still reject the FIFO before decoding.
    std::fs::File::from(
        rustix::fs::open(
            path,
            rustix::fs::OFlags::RDWR | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )
        .unwrap(),
    )
}

#[test]
fn canonical_stage_identity_retains_exact_canonical_bytes_and_parent() {
    let (_directory, kura, original) = canonical_stage_identity_fixture();
    let store = kura.block_store.lock();
    let expected = store.read_da_block_rewrite_stage().unwrap().unwrap();
    let (identity, actual) = CanonicalPhysicalStageIdentity::capture(&store).unwrap();
    assert_eq!(actual.unwrap().encode(), expected.encode());
    assert_eq!(std::fs::read(&identity.path).unwrap(), original);
    assert!(identity.unchanged());
    assert!(Kura::sidecar_metadata_same_object(
        &identity.parent.metadata,
        &secure_file_metadata::from_file(&identity.parent.file).unwrap(),
    ));
    #[cfg(unix)]
    assert!(
        rustix::fs::fcntl_getfl(identity.file.as_ref().unwrap())
            .unwrap()
            .contains(rustix::fs::OFlags::NONBLOCK)
    );
    drop(identity);
    std::fs::remove_file(store.da_block_rewrite_stage_path()).unwrap();
    let (absent, stage) = CanonicalPhysicalStageIdentity::capture(&store).unwrap();
    assert!(stage.is_none());
    assert!(absent.unchanged());
    std::fs::write(store.da_block_rewrite_stage_path(), &original).unwrap();
    assert!(!absent.unchanged());
}

#[cfg(unix)]
#[test]
fn canonical_stage_identity_rejects_static_special_files_before_admission_hook() {
    use std::os::unix::fs::symlink;
    for shape in ["fifo", "directory", "symlink", "hardlink", "oversize"] {
        let (_directory, kura, original) = canonical_stage_identity_fixture();
        let store = kura.block_store.lock();
        let path = store.da_block_rewrite_stage_path();
        let before_data = std::fs::read(store.path_to_blockchain.join(DATA_FILE_NAME)).unwrap();
        std::fs::remove_file(&path).unwrap();
        let target = store.path_to_blockchain.join("stage-admission-control");
        let _fifo_peer = match shape {
            "fifo" => Some(canonical_stage_fifo_with_live_peer(&path)),
            "directory" => {
                std::fs::create_dir(&path).unwrap();
                None
            }
            "symlink" | "hardlink" => {
                std::fs::write(&target, &original).unwrap();
                if shape == "symlink" {
                    symlink(&target, &path).unwrap();
                } else {
                    std::fs::hard_link(&target, &path).unwrap();
                }
                None
            }
            "oversize" => {
                std::fs::File::create(&path)
                    .unwrap()
                    .set_len(MAX_DA_BLOCK_REWRITE_STAGE_BYTES + 1)
                    .unwrap();
                None
            }
            _ => unreachable!(),
        };
        let admitted = std::cell::Cell::new(false);
        assert!(
            CanonicalPhysicalStageIdentity::capture_after_admission(&store, || {
                admitted.set(true);
            })
            .is_err(),
            "{shape} must fail before canonical decoding"
        );
        assert!(
            !admitted.get(),
            "{shape} must fail before descriptor admission"
        );
        assert_eq!(
            std::fs::read(store.path_to_blockchain.join(DATA_FILE_NAME)).unwrap(),
            before_data
        );
    }
}

#[cfg(unix)]
#[test]
fn canonical_stage_identity_rejects_replacement_after_metadata_admission() {
    use std::os::unix::fs::symlink;
    for shape in ["fifo", "symlink", "inode", "parent", "growth"] {
        let (_directory, kura, original) = canonical_stage_identity_fixture();
        let store = kura.block_store.lock();
        let path = store.da_block_rewrite_stage_path();
        let _fifo_peer = std::cell::RefCell::new(None);
        let admitted = std::cell::Cell::new(false);
        let result = CanonicalPhysicalStageIdentity::capture_after_admission(&store, || {
            admitted.set(true);
            if shape == "parent" {
                let old = store.path_to_blockchain.with_extension("stage-old-parent");
                std::fs::rename(&store.path_to_blockchain, old).unwrap();
                std::fs::create_dir(&store.path_to_blockchain).unwrap();
                std::fs::write(&path, &original).unwrap();
            } else if shape == "growth" {
                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .open(&path)
                    .unwrap();
                file.write_all(&[0]).unwrap();
            } else {
                std::fs::remove_file(&path).unwrap();
                match shape {
                    "fifo" => {
                        *_fifo_peer.borrow_mut() = Some(canonical_stage_fifo_with_live_peer(&path));
                    }
                    "symlink" => {
                        let target = store.path_to_blockchain.join("stage-race-control");
                        std::fs::write(&target, &original).unwrap();
                        symlink(target, &path).unwrap();
                    }
                    "inode" => std::fs::write(&path, &original).unwrap(),
                    _ => unreachable!(),
                }
            }
        });
        assert!(admitted.get());
        assert!(
            result.is_err(),
            "{shape} changed the exact admitted identity"
        );
    }
}

#[test]
fn canonical_stage_identity_uses_the_same_malformed_frame_and_image_rejection() {
    for shape in ["frame", "version", "image"] {
        let (_directory, kura, _) = canonical_stage_identity_fixture();
        let store = kura.block_store.lock();
        let path = store.da_block_rewrite_stage_path();
        let mut stage = store.read_da_block_rewrite_stage().unwrap().unwrap();
        let bytes = match shape {
            "frame" => vec![0_u8; 3],
            "version" => {
                stage.format_version = stage.format_version.checked_add(1).unwrap();
                stage.encode()
            }
            "image" => {
                stage.replacement[0].height = 0;
                stage.encode()
            }
            _ => unreachable!(),
        };
        std::fs::write(&path, &bytes).unwrap();
        assert!(store.read_da_block_rewrite_stage().is_err());
        assert!(CanonicalPhysicalStageIdentity::capture(&store).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}
