// Pure observation does not confer durability or mutate Native evidence.

fn native_observation_tree(root: &Path) -> Vec<(PathBuf, bool, Vec<u8>)> {
    fn visit(root: &Path, directory: &Path, out: &mut Vec<(PathBuf, bool, Vec<u8>)>) {
        let mut entries = fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            let directory = fs::symlink_metadata(&path).unwrap().is_dir();
            out.push((
                path.strip_prefix(root).unwrap().to_path_buf(),
                directory,
                if directory {
                    Vec::new()
                } else {
                    fs::read(&path).unwrap()
                },
            ));
            if directory {
                visit(root, &path, out);
            }
        }
    }
    let mut out = Vec::new();
    visit(root, root, &mut out);
    out
}

#[test]
fn native_geometry_observation_and_drop_preserve_exact_files() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (kura, fixture) = prepare_native_amx_archive(&root);
    let _prune = kura.prune_lock.lock();
    let _canonical = kura.canonical_chain_lock.lock();
    let _geometry = kura.lane_geometry_lock.lock();
    let _sidecar = kura.sidecar_lock.lock();
    let directory = fixture.manifest.parent().unwrap();
    let guard = Kura::open_bound_progress_directory(&root, directory).unwrap();
    let inventory = kura
        .geometry_bound_progress_directory_snapshot(
            &guard,
            MAX_GEOMETRY_ARCHIVE_ENTRIES,
            "Native observation test",
        )
        .unwrap();
    let before = native_observation_tree(&root);
    let observed = kura
        .observe_geometry_native_amx_per_height_evidence(
            directory,
            &inventory,
            1,
            "Native observation test",
        )
        .unwrap();
    assert_eq!(observed.manifests().len(), 1);
    assert_eq!(observed.receipts().len(), 1);
    let manifest = HashOf::new(&observed.manifests()[&1]);
    let receipt = HashOf::new(&observed.receipts()[&1]);
    drop(observed);
    assert_eq!(native_observation_tree(&root), before);
    assert_eq!(
        kura.geometry_bound_progress_directory_snapshot(
            &guard,
            MAX_GEOMETRY_ARCHIVE_ENTRIES,
            "Native observation test",
        )
        .unwrap(),
        inventory
    );
    let (manifests, receipts) = kura
        .observe_geometry_native_amx_per_height_evidence(
            directory,
            &inventory,
            1,
            "Native observation test",
        )
        .unwrap()
        .attest()
        .unwrap();
    assert_eq!(HashOf::new(&manifests[&1]), manifest);
    assert_eq!(HashOf::new(&receipts[&1]), receipt);
    assert_eq!(native_observation_tree(&root), before);
}

#[test]
fn native_geometry_attestation_refuses_changed_captured_file() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (kura, fixture) = prepare_native_amx_archive(&root);
    let _prune = kura.prune_lock.lock();
    let _canonical = kura.canonical_chain_lock.lock();
    let _geometry = kura.lane_geometry_lock.lock();
    let _sidecar = kura.sidecar_lock.lock();
    let directory = fixture.manifest.parent().unwrap();
    let guard = Kura::open_bound_progress_directory(&root, directory).unwrap();
    let inventory = kura
        .geometry_bound_progress_directory_snapshot(
            &guard,
            MAX_GEOMETRY_ARCHIVE_ENTRIES,
            "Native substitution test",
        )
        .unwrap();
    let observed = kura
        .observe_geometry_native_amx_per_height_evidence(
            directory,
            &inventory,
            1,
            "Native substitution test",
        )
        .unwrap();
    // Preserve the path and length: an admitted identity must still reject
    // changed bytes before it can return a durable evidence result.
    let mut substituted = fs::read(&fixture.manifest).unwrap();
    let last = substituted.last_mut().unwrap();
    *last ^= 1;
    fs::write(&fixture.manifest, substituted).unwrap();
    let after_substitution = native_observation_tree(&root);
    let error = observed
        .attest()
        .err()
        .expect("changed observation is not durable authority");
    assert!(matches!(error, Error::IO(ref source, _) if source.kind() == ErrorKind::InvalidData));
    assert_eq!(native_observation_tree(&root), after_substitution);
}

#[test]
fn native_geometry_observation_refuses_temporaries_without_recovery() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (kura, fixture) = prepare_native_amx_archive(&root);
    let _prune = kura.prune_lock.lock();
    let _canonical = kura.canonical_chain_lock.lock();
    let _geometry = kura.lane_geometry_lock.lock();
    let _sidecar = kura.sidecar_lock.lock();
    let temporary = fixture.manifest.with_extension("norito.tmp");
    fs::copy(&fixture.manifest, &temporary).unwrap();
    let directory = fixture.manifest.parent().unwrap();
    let guard = Kura::open_bound_progress_directory(&root, directory).unwrap();
    let inventory = kura
        .geometry_bound_progress_directory_snapshot(
            &guard,
            MAX_GEOMETRY_ARCHIVE_ENTRIES,
            "Native temporary test",
        )
        .unwrap();
    let before = native_observation_tree(&root);
    let error = kura
        .observe_geometry_native_amx_per_height_evidence(
            directory,
            &inventory,
            1,
            "Native temporary test",
        )
        .err()
        .expect("observation must request explicit maintenance");
    assert!(error.to_string().contains("still temporary"), "{error}");
    assert_eq!(native_observation_tree(&root), before);
    assert!(temporary.exists());
}

#[test]
fn native_geometry_observation_enforces_retention_before_effects() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (kura, fixture) = prepare_native_amx_archive(&root);
    let _prune = kura.prune_lock.lock();
    let _canonical = kura.canonical_chain_lock.lock();
    let _geometry = kura.lane_geometry_lock.lock();
    let _sidecar = kura.sidecar_lock.lock();
    let directory = fixture.manifest.parent().unwrap();
    let guard = Kura::open_bound_progress_directory(&root, directory).unwrap();
    let inventory = kura
        .geometry_bound_progress_directory_snapshot(
            &guard,
            MAX_GEOMETRY_ARCHIVE_ENTRIES,
            "Native retention test",
        )
        .unwrap();
    let before = native_observation_tree(&root);
    let error = kura
        .observe_geometry_native_amx_per_height_evidence(
            directory,
            &inventory,
            0,
            "Native retention test",
        )
        .err()
        .expect("one retained record exceeds the admitted zero bound");
    assert!(
        error
            .to_string()
            .contains("count exceeds configured retention"),
        "{error}"
    );
    assert_eq!(native_observation_tree(&root), before);
}

#[test]
fn native_geometry_attestation_refuses_new_sibling_before_sync() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (kura, fixture) = prepare_native_amx_archive(&root);
    let _prune = kura.prune_lock.lock();
    let _canonical = kura.canonical_chain_lock.lock();
    let _geometry = kura.lane_geometry_lock.lock();
    let _sidecar = kura.sidecar_lock.lock();
    let directory = fixture.manifest.parent().unwrap();
    let guard = Kura::open_bound_progress_directory(&root, directory).unwrap();
    let inventory = kura
        .geometry_bound_progress_directory_snapshot(
            &guard,
            MAX_GEOMETRY_ARCHIVE_ENTRIES,
            "Native namespace test",
        )
        .unwrap();
    let observed = kura
        .observe_geometry_native_amx_per_height_evidence(
            directory,
            &inventory,
            1,
            "Native namespace test",
        )
        .unwrap();
    // The Native files themselves are untouched. A changed sibling namespace
    // still invalidates the exact observation before its durability work starts.
    fs::write(directory.join("unowned-evidence"), b"changed namespace").unwrap();
    let after_substitution = native_observation_tree(&root);
    let error = observed
        .attest()
        .err()
        .expect("changed namespace is not durable authority");
    assert!(matches!(error, Error::IO(ref source, _) if source.kind() == ErrorKind::InvalidData));
    assert_eq!(native_observation_tree(&root), after_substitution);
}
