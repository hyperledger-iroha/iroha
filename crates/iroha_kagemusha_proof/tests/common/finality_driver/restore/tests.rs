//! Integrity-only hostile-storage checks; these synthetic bytes grant no source authority.

use super::*;
use iroha_kagemusha_proof::finality::{catalog::DirectoryCatalog, native::NodeId};

fn directory(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "kg-finality-restore-{}-{}-{label}",
        std::process::id(),
        module_path!(),
    ));
    fs::create_dir(&path).unwrap();
    path
}
fn stored(root: &Path) -> (Vec<ArtifactRecord>, Vec<u8>, OriginalBytes) {
    let mut catalog = DirectoryCatalog::create(
        root.join("originals"),
        ImportLimits {
            key: ReadConfig {
                maximum_bytes: 1024,
                maximum_rows: 1 << 16,
                coset_cache: CosetCachePolicy::OnDemand,
                msm_budget: MemoryBudget::DEFAULT,
            },
            maximum_artifacts: 1,
            maximum_original_bytes: 4096,
        },
    )
    .unwrap();
    // Storage-only originals: never passed to a key importer or source qualifier.
    let bytes = OriginalBytes {
        descriptor: vec![1, 2, 3],
        verifying_key: vec![4, 5, 6],
        proving_key: vec![7, 8, 9],
    };
    catalog
        .store(&ArtifactId::Source(NodeId::Genesis), &bytes)
        .unwrap();
    let inventory = catalog.inventory().unwrap();
    let records = norito::decode_canonical(&inventory).unwrap();
    fs::write(root.join("originals/inventory.norito"), &inventory).unwrap();
    (records, inventory, bytes)
}

#[test]
fn verifier_blob_reads_are_bounded_exact_and_never_open_pk_addresses() {
    let root = directory("blobs");
    let (records, _, bytes) = stored(&root);
    let mut files = VerifierFiles::new(&root.join("originals"), &records).unwrap();
    for (index, expected) in [&bytes.descriptor, &bytes.verifying_key]
        .into_iter()
        .enumerate()
    {
        let mut actual = Vec::new();
        files
            .open(&records[0].sha256[index])
            .unwrap()
            .read_to_end(&mut actual)
            .unwrap();
        assert_eq!(&actual, expected);
    }
    assert_eq!(files.reads, 2);
    assert!(files.open(&records[0].sha256[2]).is_err());
    assert!(files.open(&[9; 32]).is_err());
    let digest = records[0].sha256[1];
    let path = files.files[&digest].0.clone();
    for changed in [vec![4, 5], vec![4, 5, 6, 7], vec![4, 5, 7]] {
        fs::write(&path, changed).unwrap();
        assert!(files.open(&digest).is_err());
    }
    fs::remove_file(&path).unwrap();
    assert!(files.open(&digest).is_err());
    fs::create_dir(&path).unwrap();
    assert!(files.open(&digest).is_err());
    fs::remove_dir(&path).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("originals/inventory.norito"), &path).unwrap();
        assert!(files.open(&digest).is_err());
    }
    assert_eq!(files.reads, 2);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn captured_identifiers_are_independent_pins_and_inventory_cannot_extend_them() {
    let root = directory("pins");
    let (records, inventory, _) = stored(&root);
    let expected = CaptureIdentity {
        producer: [1; 32],
        sources: [2; 32],
        fixture: Sha256::digest(FIXTURE.as_bytes()).into(),
        inventory: Sha256::digest(&inventory).into(),
    };
    fs::write(root.join("binary.sha256"), expected.producer).unwrap();
    fs::write(root.join("fixture.json"), FIXTURE).unwrap();
    let provenance = SourceProvenance {
        revision: "storage-only test".into(),
        source_manifest_sha256: expected.sources,
    };
    fs::write(
        root.join("provenance.norito"),
        norito::to_bytes(&provenance).unwrap(),
    )
    .unwrap();
    assert_eq!(capture_records(&root, FIXTURE, expected).unwrap(), records);
    for changed in [
        CaptureIdentity {
            producer: [3; 32],
            ..expected
        },
        CaptureIdentity {
            sources: [3; 32],
            ..expected
        },
        CaptureIdentity {
            fixture: [3; 32],
            ..expected
        },
        CaptureIdentity {
            inventory: [3; 32],
            ..expected
        },
        CaptureIdentity {
            producer: [0; 32],
            ..expected
        },
    ] {
        assert!(capture_records(&root, FIXTURE, changed).is_err());
    }
    assert!(capture_records(&root, &format!("{FIXTURE} "), expected).is_err());
    let mut changed = inventory.clone();
    changed.push(0);
    fs::write(root.join("originals/inventory.norito"), &changed).unwrap();
    assert!(capture_records(&root, FIXTURE, expected).is_err());
    let changed_pin = CaptureIdentity {
        inventory: Sha256::digest(&changed).into(),
        ..expected
    };
    assert!(capture_records(&root, FIXTURE, changed_pin).is_err());
    fs::write(root.join("originals/inventory.norito"), &inventory).unwrap();
    fs::remove_file(root.join("provenance.norito")).unwrap();
    assert!(capture_records(&root, FIXTURE, expected).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn verifier_inventory_refuses_duplicate_names_foreign_paths_and_bad_lengths() {
    let root = directory("inventory");
    let (records, _, _) = stored(&root);
    assert!(VerifierFiles::new(&root.join("originals"), &[]).is_err());
    assert!(
        VerifierFiles::new(
            &root.join("originals"),
            &[records[0].clone(), records[0].clone()]
        )
        .is_err()
    );
    for attack in 0..4 {
        let mut changed = records.clone();
        match attack {
            0 => changed[0].name = b"../outside".to_vec(),
            1 => changed[0].lengths[0] = (1 << 20) + 1,
            2 => changed[0].lengths[1] = 0,
            _ => changed[0].sha256[1] = [0; 32],
        }
        assert!(VerifierFiles::new(&root.join("originals"), &changed).is_err());
    }
    let lock = RunLock::acquire(&root).unwrap();
    let placeholder = CaptureIdentity {
        producer: [1; 32],
        sources: [2; 32],
        fixture: [3; 32],
        inventory: [4; 32],
    };
    assert!(matches!(
        load_completed_receipt(&root, FIXTURE, placeholder),
        Err(RestoreError::Capture)
    ));
    drop(lock);
    fs::remove_dir_all(&root).unwrap();
    assert!(matches!(
        load_completed_receipt(&root, FIXTURE, placeholder),
        Err(RestoreError::Capture)
    ));
    assert!(!root.exists());
}

#[test]
fn correctly_pinned_malformed_capture_refuses_before_source_qualification() {
    let root = directory("malformed-capture");
    let (_, inventory, _) = stored(&root);
    let mut expected = CaptureIdentity {
        producer: [1; 32],
        sources: [2; 32],
        fixture: [3; 32],
        inventory: Sha256::digest(&inventory).into(),
    };
    fs::write(root.join("binary.sha256"), expected.producer).unwrap();
    let provenance = SourceProvenance {
        revision: "storage-only malformed fixture test".into(),
        source_manifest_sha256: expected.sources,
    };
    fs::write(
        root.join("provenance.norito"),
        norito::to_bytes(&provenance).unwrap(),
    )
    .unwrap();
    let json: Value = norito::json::from_str(FIXTURE).unwrap();
    let recorded = value(&json, "receipt_digest_hex")
        .unwrap()
        .as_str()
        .unwrap();
    let network = value(value(&json, "history_anchor").unwrap(), "network_hex")
        .unwrap()
        .as_str()
        .unwrap();
    for malformed in [
        "{".to_owned(),
        "{}".to_owned(),
        FIXTURE.replace(recorded, &"00".repeat(32)),
        FIXTURE.replace(network, "é"),
        FIXTURE.replace(network, "0"),
        FIXTURE.replace(network, "00"),
    ] {
        expected.fixture = Sha256::digest(malformed.as_bytes()).into();
        fs::write(root.join("fixture.json"), &malformed).unwrap();
        // Integrity checks succeed, yet the fallible typed parser refuses.
        assert!(capture_records(&root, &malformed, expected).is_ok());
        assert!(matches!(
            load_completed_receipt(&root, &malformed, expected),
            Err(RestoreError::Capture)
        ));
    }
    assert!(fixture(FIXTURE).is_ok());
    assert!(hex("é").is_err());
    assert!(hex("zz").is_err());
    fs::remove_dir_all(root).unwrap();
}
