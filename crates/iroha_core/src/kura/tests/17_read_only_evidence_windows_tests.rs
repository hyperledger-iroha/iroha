// Windows controls exercise the native kernel sharing and DACL boundary. Unix descriptor
// flags, FIFO admission and allocation-free Unix metadata checks stay in their original tests.

#[test]
fn read_only_evidence_native_marker_recheck_is_exact() {
    let fixture = Fixture::new();
    let reader = fixture.open();
    reader.check_sources().unwrap();
    let mut incorrect = reader.marker_bytes.clone();
    *incorrect.last_mut().unwrap() ^= 1;
    assert!(matches!(
        reader.sources.marker.check_marker_bytes(&incorrect),
        Err(CanonicalKuraEvidenceError::Invalid(
            "published marker changed"
        ))
    ));
    assert!(matches!(
        reader
            .sources
            .marker
            .check_marker_bytes(&incorrect[..incorrect.len() - 1]),
        Err(CanonicalKuraEvidenceError::Invalid(
            "published marker recheck bound"
        ))
    ));
}

#[test]
fn read_only_evidence_native_parent_and_leaf_policy_drift_poison_finish() {
    for parent in [false, true] {
        let fixture = Fixture::new();
        let mut reader = fixture.open();
        fixture.read_all(&mut reader);
        let path = if parent {
            fixture.root.clone()
        } else {
            fixture.root.join(DATA_FILE_NAME)
        };
        assert!(fs::rename(&path, path.with_extension("displaced")).is_err());
        reader.check_sources().unwrap();
        fixture_io::grant_foreign_write(&path);
        assert!(
            reader.finish().is_err(),
            "changed original policy, parent={parent}"
        );
    }
    let fixture = Fixture::new();
    let mut reader = fixture.open();
    assert!(fs::rename(&fixture.root, fixture.root.with_extension("displaced")).is_err());
    fixture_io::grant_foreign_write(&fixture.root);
    assert!(reader.read_carrier(1).is_err());
    assert!(reader.finish().is_err());
}

#[test]
fn read_only_evidence_native_rejects_file_symlink() {
    let fixture = Fixture::new();
    let original = fixture.root.join("original");
    let selected = fixture.root.join(DATA_FILE_NAME);
    fs::rename(&selected, &original).unwrap();
    std::os::windows::fs::symlink_file(&original, &selected)
        .expect("genuine native symlink privilege is required for this control");
    assert!(CanonicalKuraEvidenceReader::open(&fixture.root, fixture.limits()).is_err());
}

#[test]
fn read_only_evidence_native_rejects_hardlink_and_directory_sources() {
    for directory in [false, true] {
        let fixture = Fixture::new();
        let original = fixture.root.join("original");
        let selected = fixture.root.join(DATA_FILE_NAME);
        fs::rename(&selected, &original).unwrap();
        if directory {
            fs::create_dir(&selected).unwrap();
        } else {
            fs::hard_link(&original, &selected).unwrap();
        }
        assert!(CanonicalKuraEvidenceReader::open(&fixture.root, fixture.limits()).is_err());
    }
}

#[test]
fn read_only_evidence_native_admission_hook_keeps_original_and_rechecks_policy() {
    for drift in [false, true] {
        let fixture = Fixture::new();
        let reached = std::cell::Cell::new(false);
        let result = CanonicalKuraEvidenceReader::open_after_admission(
            &fixture.root,
            fixture.limits(),
            |path| {
                if path != fixture.root.join(DATA_FILE_NAME) {
                    return;
                }
                reached.set(true);
                assert!(fs::OpenOptions::new().write(true).open(path).is_err());
                assert!(fs::remove_file(path).is_err());
                assert!(fs::rename(path, path.with_extension("displaced")).is_err());
                if drift {
                    fixture_io::grant_foreign_write(path);
                }
            },
        );
        assert!(
            reached.get(),
            "the hook must run after actual original admission"
        );
        if drift {
            assert!(result.is_err(), "post-admission ACL drift must refuse");
        } else {
            let mut reader = result.expect("denied namespace attacks preserve the original");
            fixture.read_all(&mut reader);
            reader.finish().unwrap().recheck_sources().unwrap();
        }
    }
}

#[test]
fn read_only_evidence_native_actual_originals_exclude_writers_delete_and_rename() {
    let fixture = Fixture::new();
    let before = fixture.snapshot();
    let mut reader = fixture.open();
    for path in fixture.files() {
        assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
        assert!(fs::remove_file(&path).is_err());
        assert!(fs::rename(&path, path.with_extension("displaced")).is_err());
    }
    fixture.read_all(&mut reader);
    let complete = reader.finish().unwrap();
    complete.recheck_sources().unwrap();
    assert_eq!(fixture.snapshot(), before);
    for path in fixture.files() {
        assert!(fs::OpenOptions::new().write(true).open(&path).is_err());
        assert!(fs::remove_file(&path).is_err());
    }
}
