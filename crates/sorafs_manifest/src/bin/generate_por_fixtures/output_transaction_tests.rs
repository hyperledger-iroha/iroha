// PoR fixture output transaction and rollback regressions.
#[test]
fn bound_output_never_follows_a_parent_substitution() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let temporary_path = physical_path(temporary.path());
    let parent = temporary_path.join("parent");
    let output = parent.join("output");
    fs::create_dir_all(&output).expect("create original output");
    let bound = BoundDirectory::open(&output, "test fixture output").expect("bind original output");
    let moved_parent = temporary_path.join("moved-parent");
    let working_directory = BoundWorkingDirectory::enter(&bound).expect("enter bound output");
    // The pathname now points at a new directory, but the process cwd
    // remains the originally opened output inode until the guard restores
    // it. A relative write must therefore land only beneath moved_parent.
    fs::rename(&parent, &moved_parent).expect("move original parent");
    fs::create_dir_all(&output).expect("create substituted output");
    fs::write("sentinel", b"bound").expect("write through bound working directory");
    working_directory
        .restore()
        .expect("restore working directory");
    assert_eq!(
        fs::read(moved_parent.join("output/sentinel")).expect("read bound sentinel"),
        b"bound"
    );
    assert!(!output.join("sentinel").exists());
    assert!(
        bound
            .verify("test fixture output")
            .expect_err("path substitution must be reported")
            .to_string()
            .contains("changed identity")
    );
}
#[test]
fn multi_file_publication_failure_restores_every_original() {
    let temporary = tempdir().expect("create publication transaction root");
    let temporary_root = temporary
        .path()
        .canonicalize()
        .expect("canonicalize publication transaction root");
    let fixture_root = temporary_root.join("fixtures");
    let staging = temporary_root.join("staging");
    let backup = temporary_root.join("backup");
    ensure_real_directory(&fixture_root).expect("create fixture root");
    ensure_real_directory(&fixture_root.join("por")).expect("create managed directory");
    ensure_real_directory(&staging).expect("create staging directory");
    ensure_real_directory(&backup).expect("create backup directory");
    let relative_a = PathBuf::from("por/a.to");
    let relative_b = PathBuf::from("por/b.to");
    let destination_a = fixture_root.join(&relative_a);
    let destination_b = fixture_root.join(&relative_b);
    write_new_regular_file(&destination_a, b"old-a").expect("write original a");
    write_new_regular_file(&destination_b, b"old-b").expect("write original b");
    let original_a = read_regular_file_snapshot(&destination_a).expect("snapshot original a");
    let original_b = read_regular_file_snapshot(&destination_b).expect("snapshot original b");
    let staged_a = staging.join("a.to");
    let staged_b = staging.join("b.to");
    let staged_a_identity =
        write_new_regular_file(&staged_a, b"new-a").expect("stage replacement a");
    let staged_b_identity =
        write_new_regular_file(&staged_b, b"new-b").expect("stage replacement b");
    assert_ne!(
        staged_a_identity, staged_b_identity,
        "independent staged files must have independent identities"
    );
    let mut entries = [
        PublicationEntry {
            relative: relative_a,
            expected: b"new-a".to_vec(),
            original: Some(original_a),
            staged_path: staged_a,
            backup_path: backup.join("a.to"),
            staged_identity: staged_a_identity,
            state: PublicationState::Prepared,
        },
        PublicationEntry {
            relative: relative_b,
            expected: b"new-b".to_vec(),
            original: Some(original_b),
            staged_path: staged_b,
            backup_path: backup.join("b.to"),
            staged_identity: staged_b_identity,
            state: PublicationState::Prepared,
        },
    ];
    publish_entries_with_hook(&fixture_root, &mut entries, |index| {
        if index == 1 {
            Err("injected failure before second fixture".into())
        } else {
            Ok(())
        }
    })
    .expect_err("injected second-entry failure must stop publication");
    rollback_entries(&fixture_root, &mut entries)
        .expect("transaction rollback must restore every original");
    assert_eq!(
        read_regular_file(&destination_a).expect("read restored a"),
        b"old-a".to_vec()
    );
    assert_eq!(
        read_regular_file(&destination_b).expect("read restored b"),
        b"old-b".to_vec()
    );
    assert_eq!(
        entries.iter().map(|entry| entry.state).collect::<Vec<_>>(),
        vec![PublicationState::Prepared, PublicationState::Prepared]
    );
}
