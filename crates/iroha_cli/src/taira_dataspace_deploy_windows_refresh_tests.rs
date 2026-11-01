//! Actual NTFS refresh failures, child-process crashes and recovery barriers.

use super::*;

fn context() -> (String, NetworkId, String) {
    (
        digest(b"definition excluding fee cap"),
        NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"genesis"),
        )),
        digest(b"selected trust"),
    )
}

fn fixture() -> (tempfile::TempDir, Journal) {
    let temporary = tempfile::tempdir().unwrap();
    let root = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
    let journal = Journal::open_native_child(
        &root,
        std::ffi::OsStr::new(&digest(b"operation")),
        true,
        false,
    )
    .unwrap();
    journal.install("definition.json", b"definition").unwrap();
    journal.install("plan.json", b"old plan").unwrap();
    (temporary, journal)
}

fn identity(journal: &Journal, name: &str) -> iroha_fs::FileIdentity {
    journal
        .directory
        .open_movable_private(name)
        .unwrap()
        .identity()
        .unwrap()
}

#[test]
fn native_relative_journal_reopens_and_serializes_the_original_lock() {
    const CHILD: &str = "IROHA_TEST_WINDOWS_RELATIVE_JOURNAL";
    if std::env::var_os(CHILD).is_some() {
        let journal = Journal::open(Path::new("fresh"), true).unwrap();
        assert_eq!(journal.path, std::env::current_dir().unwrap().join("fresh"));
        journal
            .install_json("retained.json", &"original bytes")
            .unwrap();
        assert!(Journal::open(Path::new("fresh"), false).is_err());
        assert!(
            std::fs::rename(
                journal.path.join("lock"),
                journal.path.join("displaced-lock")
            )
            .is_err()
        );
        journal.revalidate().unwrap();
        drop(journal);
        let reopened = Journal::open(Path::new("fresh"), false).unwrap();
        assert_eq!(
            reopened.read_json::<String>("retained.json").unwrap(),
            "original bytes"
        );
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let parent = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
    let test = concat!(
        module_path!(),
        "::native_relative_journal_reopens_and_serializes_the_original_lock"
    );
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test.split_once("::").unwrap().1, "--nocapture"])
        .current_dir(parent.path())
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
}

#[test]
fn native_empty_unpublished_directory_recovers_only_without_evidence() {
    let temporary = tempfile::tempdir().unwrap();
    let parent = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
    let empty = parent.create_child("empty").unwrap();
    let journal = Journal::open_unpublished(empty.path()).unwrap();
    journal.require_unplanned().unwrap();
    drop(journal);
    let evidence = parent.create_child("evidence").unwrap();
    evidence
        .write_atomic(
            "catalog.prepared.json",
            b"signed",
            iroha_fs::PublishMode::CreateNew,
        )
        .unwrap();
    assert!(Journal::open_unpublished(evidence.path()).is_err());
    assert!(!evidence.path().join("lock").exists());
}

#[test]
fn refresh_errors_restore_exact_original_or_preserve_committed_recovery() {
    for failed in [
        Step::IntentDurable,
        Step::OldPublished,
        Step::NewPublished,
        Step::NewDurable,
        Step::CommitPublished,
        Step::CommitDurable,
    ] {
        let (_temporary, journal) = fixture();
        let old = identity(&journal, "plan.json");
        assert!(
            journal
                .refresh_native_observed(
                    "plan.json",
                    b"old plan",
                    b"new plan",
                    &context(),
                    &mut |step| {
                        if step == failed {
                            Err(eyre!("injected refresh error"))
                        } else {
                            Ok(())
                        }
                    }
                )
                .is_err()
        );
        let committed = matches!(failed, Step::CommitPublished | Step::CommitDurable);
        if committed {
            let before = journal.entry_names().unwrap();
            assert!(journal.recover_native_refresh(false, &context()).is_err());
            assert_eq!(journal.entry_names().unwrap(), before);
            assert!(journal.read_optional("plan.json").is_err());
            journal.recover_native_refresh(true, &context()).unwrap();
            assert_eq!(
                journal.read_optional("plan.json").unwrap().unwrap(),
                b"new plan"
            );
            assert_ne!(identity(&journal, "plan.json"), old);
        } else {
            assert_eq!(
                journal.read_optional("plan.json").unwrap().unwrap(),
                b"old plan"
            );
            assert_eq!(identity(&journal, "plan.json"), old);
        }
        journal.require_no_refresh().unwrap();
    }
}

#[test]
fn refresh_success_and_stale_expected_preserve_exact_current_original() {
    let (_temporary, journal) = fixture();
    journal
        .refresh_native_observed(
            "plan.json",
            b"old plan",
            b"new plan",
            &context(),
            &mut |_| Ok(()),
        )
        .unwrap();
    let current = identity(&journal, "plan.json");
    assert!(
        journal
            .refresh_native_observed(
                "plan.json",
                b"old plan",
                b"third plan",
                &context(),
                &mut |_| Ok(())
            )
            .is_err()
    );
    assert_eq!(identity(&journal, "plan.json"), current);
    assert_eq!(
        journal.read_optional("plan.json").unwrap().unwrap(),
        b"new plan"
    );
    assert_eq!(journal.entry_names().unwrap().len(), 3);
}

#[test]
fn refresh_crash_child() {
    let Some(path) = std::env::var_os("IROHA_TEST_WINDOWS_REFRESH_ROOT") else {
        return;
    };
    let stop = std::env::var("IROHA_TEST_WINDOWS_REFRESH_STEP").unwrap();
    let journal = Journal::open(Path::new(&path), false).unwrap();
    journal
        .refresh_native_observed(
            "plan.json",
            b"old plan",
            b"new plan",
            &context(),
            &mut |step| {
                if format!("{step:?}") == stop {
                    std::process::exit(93);
                }
                Ok(())
            },
        )
        .unwrap();
    panic!("requested crash boundary was not reached");
}

fn crash(journal: Journal, step: Step) -> PathBuf {
    let path = journal.path.clone();
    drop(journal);
    let test = concat!(module_path!(), "::refresh_crash_child");
    let test = test.split_once("::").unwrap().1;
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test, "--nocapture"])
        .env("IROHA_TEST_WINDOWS_REFRESH_ROOT", &path)
        .env("IROHA_TEST_WINDOWS_REFRESH_STEP", format!("{step:?}"))
        .output()
        .unwrap();
    assert_eq!(
        result.status.code(),
        Some(93),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    path
}

#[test]
fn real_crashes_recover_each_durable_state_without_fresh_journal_adoption() {
    for step in [
        Step::IntentDurable,
        Step::OldPublished,
        Step::NewPublished,
        Step::NewDurable,
        Step::CommitPublished,
        Step::CommitDurable,
    ] {
        let (_temporary, journal) = fixture();
        let old = identity(&journal, "plan.json");
        let path = crash(journal, step);
        let journal = Journal::open(&path, false).unwrap();
        let before = journal.entry_names().unwrap();
        assert!(journal.read_optional("plan.json").is_err());
        assert!(journal.recover_native_refresh(false, &context()).is_err());
        assert_eq!(
            journal.entry_names().unwrap(),
            before,
            "read-only barrier mutated pending recovery"
        );
        journal.recover_native_refresh(true, &context()).unwrap();
        let committed = matches!(step, Step::CommitPublished | Step::CommitDurable);
        assert_eq!(
            journal.read_optional("plan.json").unwrap().unwrap(),
            if committed { b"new plan" } else { b"old plan" }
        );
        assert_eq!(identity(&journal, "plan.json") == old, !committed);
        assert_eq!(journal.entry_names().unwrap().len(), 3);
    }
}

#[test]
fn recovery_refuses_competing_target_context_and_signed_evidence() {
    for obstruction in ["target", "context", "signed"] {
        let (_temporary, journal) = fixture();
        let old = identity(&journal, "plan.json");
        let path = crash(journal, Step::OldPublished);
        let journal = Journal::open(&path, false).unwrap();
        if obstruction == "target" {
            journal
                .install_native("plan.json", b"foreign", MAX_BYTES)
                .unwrap();
        }
        if obstruction == "signed" {
            journal
                .install_native("catalog.prepared.json", b"signed evidence", MAX_BYTES)
                .unwrap();
        }
        let mut selected = context();
        if obstruction == "context" {
            selected.2 = digest(b"different trust");
        }
        let before = journal.entry_names().unwrap();
        assert!(journal.recover_native_refresh(true, &selected).is_err());
        assert_eq!(journal.entry_names().unwrap(), before);
        let backup = before.iter().find(|name| *name == BACKUP).unwrap();
        assert_eq!(identity(&journal, backup.to_str().unwrap()), old);
        if obstruction == "target" {
            assert_eq!(
                journal
                    .read_native("plan.json", MAX_BYTES)
                    .unwrap()
                    .unwrap(),
                b"foreign"
            );
        }
    }
}

#[test]
fn missing_lock_with_refresh_never_creates_a_new_operation_lock() {
    let (_temporary, journal) = fixture();
    let path = crash(journal, Step::OldPublished);
    std::fs::remove_file(path.join("lock")).unwrap();
    assert!(Journal::open_unpublished(&path).is_err());
    assert!(!path.join("lock").exists());
}

#[test]
fn malformed_intent_preserves_all_originals_and_commit_survives_intent_cleanup() {
    let (_temporary, journal) = fixture();
    let path = crash(journal, Step::IntentDurable);
    let journal = Journal::open(&path, false).unwrap();
    let bytes = journal.read_native(INTENT, CONTROL_BOUND).unwrap().unwrap();
    let original = Intent::decode(&bytes).unwrap();
    let names = journal.entry_names().unwrap();
    for mutation in 0..7 {
        let mut changed = original.clone();
        match mutation {
            0 => changed.schema = 2,
            1 => changed.target = "../plan.json".into(),
            2 => changed.nonce.push('0'),
            3 => changed.old.size = MAX_BYTES as u64 + 1,
            4 => changed.new.identity.clone_from(&changed.old.identity),
            5 => changed.context_sha256 = "F".repeat(64),
            _ => changed.old.stable_snapshot = "short".into(),
        }
        std::fs::write(path.join(INTENT), json::to_vec(&changed).unwrap()).unwrap();
        assert!(journal.recover_native_refresh(true, &context()).is_err());
        assert_eq!(journal.entry_names().unwrap(), names);
        assert_eq!(
            journal
                .read_native("plan.json", MAX_BYTES)
                .unwrap()
                .unwrap(),
            b"old plan"
        );
    }
    std::fs::write(path.join(INTENT), bytes).unwrap();
    journal.recover_native_refresh(true, &context()).unwrap();
    drop(journal);

    let (_temporary, journal) = fixture();
    let path = crash(journal, Step::CommitDurable);
    let journal = Journal::open(&path, false).unwrap();
    let bytes = journal.read_native(COMMIT, CONTROL_BOUND).unwrap().unwrap();
    let record = Intent::decode(&bytes).unwrap();
    journal
        .remove_movable(
            &record.backup(),
            journal
                .directory
                .open_movable_private(record.backup())
                .unwrap(),
        )
        .unwrap();
    journal
        .remove_movable(
            INTENT,
            journal.directory.open_movable_private(INTENT).unwrap(),
        )
        .unwrap();
    journal.recover_native_refresh(true, &context()).unwrap();
    assert_eq!(
        journal.read_optional("plan.json").unwrap().unwrap(),
        b"new plan"
    );
    journal.require_no_refresh().unwrap();
}

#[test]
fn unpublished_control_bytes_are_never_adopted_or_deleted() {
    let (_temporary, journal) = fixture();
    let name = format!("{INTENT_STAGE}{}", "0".repeat(32));
    journal
        .install_native(
            &name,
            b"partial or substituted unpublished bytes",
            CONTROL_BOUND,
        )
        .unwrap();
    let original = identity(&journal, &name);
    journal.require_no_refresh().unwrap();
    journal
        .refresh_native_observed(
            "plan.json",
            b"old plan",
            b"new plan",
            &context(),
            &mut |_| Ok(()),
        )
        .unwrap();
    assert_eq!(identity(&journal, &name), original);
    assert_eq!(
        journal.read_native(&name, CONTROL_BOUND).unwrap().unwrap(),
        b"partial or substituted unpublished bytes"
    );
}

#[test]
fn native_journal_pins_parent_and_refuses_hardlinked_evidence() {
    let (_temporary, journal) = fixture();
    assert!(std::fs::rename(&journal.path, journal.path.with_extension("displaced")).is_err());
    let inherited = journal.path.join("inherited.json");
    std::fs::write(&inherited, b"inherited ACL").unwrap();
    assert!(iroha_fs::RetainedFile::open_regular(&inherited).is_ok());
    assert!(journal.read_optional("inherited.json").is_err());
    assert_eq!(std::fs::read(&inherited).unwrap(), b"inherited ACL");
    std::fs::remove_file(inherited).unwrap();
    let leaf = journal.path.join("plan.json");
    std::fs::hard_link(&leaf, journal.path.join("alias")).unwrap();
    assert!(journal.read_optional("plan.json").is_err());
    assert!(
        journal
            .refresh_native_observed(
                "plan.json",
                b"old plan",
                b"new plan",
                &context(),
                &mut |_| Ok(())
            )
            .is_err()
    );
    assert_eq!(std::fs::read(leaf).unwrap(), b"old plan");
}
