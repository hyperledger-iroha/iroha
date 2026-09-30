//! Regressions for exact source types, substitutions, and typed manifest export.

use super::*;
use std::os::unix::fs::symlink;

fn indexed(mode: &str, object: &str, stage: &str, path: &str) -> Vec<u8> {
    format!("{mode} {object} {stage}\t{path}\0").into_bytes()
}

fn source_fixture() -> (tempfile::TempDir, PathBuf) {
    let directory = private_custody_test_dir("taira-reset-source-");
    let root = directory.path().canonicalize().expect("direct source path");
    git_output(&root, &["init", "--quiet"]).expect("initialize disposable test index");
    fs::write(root.join("Cargo.lock"), b"test lock\n").expect("write fixture lock");
    fs::create_dir(root.join("IrohaSwift")).expect("SDK directory");
    fs::create_dir(root.join("dist")).expect("unbuilt SDK artifact parent");
    fs::create_dir(root.join("iroha-docs")).expect("empty gitlink");
    fs::set_permissions(root.join("Cargo.lock"), fs::Permissions::from_mode(0o644))
        .expect("canonical source permissions independent of process umask");
    for name in ["IrohaSwift", "dist", "iroha-docs"] {
        fs::set_permissions(root.join(name), fs::Permissions::from_mode(0o755))
            .expect("canonical source directory permissions");
    }
    symlink(
        "../dist/NoritoBridge.xcframework",
        root.join("IrohaSwift/NoritoBridge.xcframework"),
    )
    .expect("tracked unbuilt SDK alias");
    git_output(
        &root,
        &[
            "add",
            "--",
            "Cargo.lock",
            "IrohaSwift/NoritoBridge.xcframework",
        ],
    )
    .expect("stage exact file and symlink objects");
    git_output(
        &root,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            "160000,3f379811fcc86cca2345ec59c56686501b710afe,iroha-docs",
        ],
    )
    .expect("stage exact gitlink identity without materializing it");
    (directory, root)
}

fn committed_source_fixture() -> (tempfile::TempDir, PathBuf) {
    let (directory, root) = source_fixture();
    let tree = git_text(&root, &["write-tree"]).expect("fixture tree object");
    // Synthetic test data in this disposable repository only. No user checkout
    // is committed, and these unsigned bytes are never release authorization.
    let commit = format!(
        "tree {tree}\nauthor Fixture <fixture@example.invalid> 1 +0000\ncommitter Fixture <fixture@example.invalid> 1 +0000\n\nsource fixture\n"
    );
    fs::write(root.join(".git/source-fixture-commit"), commit).expect("synthetic commit bytes");
    let object = git_text(
        &root,
        &[
            "hash-object",
            "-w",
            "-t",
            "commit",
            ".git/source-fixture-commit",
        ],
    )
    .expect("fixture commit object");
    git_output(&root, &["update-ref", "refs/heads/optimizations", &object]).expect("fixture ref");
    git_output(&root, &["symbolic-ref", "HEAD", "refs/heads/optimizations"])
        .expect("fixture branch");
    assert!(
        git_output(
            &root,
            &["status", "--porcelain=v1", "--untracked-files=all"]
        )
        .expect("fixture status")
        .is_empty()
    );
    (directory, root)
}

#[test]
fn index_accepts_only_unique_stage_zero_entries_of_the_four_supported_types() {
    let object = "a".repeat(40);
    for mode in ["100644", "100755", "120000", "160000"] {
        let entries = parse_index(&indexed(mode, &object, "0", "source")).expect("known type");
        assert_eq!(entries["source"].mode, mode);
        assert_eq!(entries["source"].object, object);
    }
    for (mode, stage, path) in [
        ("100600", "0", "source"),
        ("100644", "1", "source"),
        ("100644", "0", "../outside"),
        ("100644", "0", "/outside"),
    ] {
        assert!(parse_index(&indexed(mode, &object, stage, path)).is_err());
    }
    let mut duplicate = indexed("100644", &object, "0", "source");
    duplicate.extend(indexed("120000", &object, "0", "source"));
    assert!(parse_index(&duplicate).is_err());
    assert!(parse_index(&indexed("100644", "not-an-object", "0", "source")).is_err());
    assert!(parse_index(b"").is_err());
}

#[test]
fn symlink_normalization_permits_the_in_root_sdk_alias_and_rejects_escape() {
    assert_eq!(
        relative_link_target(
            "IrohaSwift/NoritoBridge.xcframework",
            "../dist/NoritoBridge.xcframework"
        )
        .expect("SDK alias target"),
        Path::new("dist/NoritoBridge.xcframework"),
    );
    for target in [
        "",
        "/tmp/external",
        "../../external",
        "..",
        "../..",
        "bad\\path",
        "bad\0path",
    ] {
        assert!(
            relative_link_target("IrohaSwift/alias", target).is_err(),
            "{target:?}"
        );
    }
    assert!(relative_link_target("IrohaSwift/alias", &"a".repeat(MAX_LINK_BYTES + 1)).is_err());
}

#[test]
fn capture_authenticates_regular_symlink_and_empty_gitlink_without_omission() {
    let (_directory, root) = source_fixture();
    let entries = capture_tree(&root).expect("capture actual repository layout");
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>(),
        [
            "Cargo.lock",
            "IrohaSwift/NoritoBridge.xcframework",
            "iroha-docs"
        ]
    );
    assert_eq!(entries[0].mode, 0o644);
    assert_eq!(entries[0].sha256, sha256_hex(b"test lock\n"));
    assert_eq!(entries[1].mode, SYMLINK_MODE);
    assert_eq!(entries[1].size, 32);
    assert_eq!(
        entries[1].sha256,
        sha256_hex(b"../dist/NoritoBridge.xcframework")
    );
    assert_eq!(entries[2].mode, GITLINK_MODE);
    assert_eq!(entries[2].size, 40);
    assert_eq!(
        entries[2].git_blob_sha1,
        "3f379811fcc86cca2345ec59c56686501b710afe"
    );
    assert_eq!(
        entries[2].sha256,
        sha256_hex(b"3f379811fcc86cca2345ec59c56686501b710afe")
    );
    assert_eq!(
        inspect_source_tree(&root, &entries).expect("same admission capture"),
        entries
    );
    assert!(
        inspect_source_tree(&root, &entries[..2]).is_err(),
        "gitlink cannot be omitted"
    );
}

#[test]
fn capture_rejects_symlink_text_substitution_and_new_referent() {
    let (_directory, root) = source_fixture();
    let alias = root.join("IrohaSwift/NoritoBridge.xcframework");
    fs::remove_file(&alias).expect("replace fixture alias");
    symlink("../dist/OtherExample.xcframework", &alias).expect("different in-root absent target");
    assert!(
        capture_tree(&root)
            .expect_err("Git blob must bind exact alias text")
            .to_string()
            .contains("differ from the exact indexed Git blob")
    );
    fs::remove_file(&alias).expect("restore fixture alias");
    symlink("../dist/NoritoBridge.xcframework", &alias).expect("original alias");
    let referent = root.join("dist/NoritoBridge.xcframework");
    fs::write(&referent, b"untracked SDK artifact").expect("materialize target");
    assert!(require_absent_link_target(&referent).is_err());
    assert!(
        capture_tree(&root)
            .expect_err("new input cannot hide behind tracked link")
            .to_string()
            .contains("referent must remain absent")
    );
}

#[test]
fn capture_rejects_a_symlink_in_the_referent_parent_path() {
    let (_directory, root) = source_fixture();
    fs::remove_dir(root.join("dist")).expect("replace target parent");
    fs::create_dir(root.join("elsewhere")).expect("unrelated source directory");
    symlink("elsewhere", root.join("dist")).expect("redirecting parent");
    assert!(
        capture_tree(&root)
            .expect_err("target parents must be direct")
            .to_string()
            .contains("unsafe custody")
    );
}

#[test]
fn capture_rejects_initialized_or_redirected_gitlinks() {
    let (_directory, root) = source_fixture();
    let gitlink = root.join("iroha-docs");
    fs::write(gitlink.join("extra"), b"additional source").expect("populate gitlink");
    assert!(
        capture_tree(&root)
            .expect_err("gitlink contents need their own closure")
            .to_string()
            .contains("gitlink directory must be empty")
    );
    fs::remove_file(gitlink.join("extra")).expect("remove fixture content");
    fs::remove_dir(&gitlink).expect("replace gitlink directory");
    symlink("dist", &gitlink).expect("redirected gitlink");
    assert!(capture_tree(&root).is_err());
}

#[test]
fn admission_rejects_file_content_mode_and_gitlink_identity_substitution() {
    let (_directory, root) = source_fixture();
    let entries = capture_tree(&root).expect("baseline capture");
    fs::write(root.join("Cargo.lock"), b"changed lock\n").expect("substitute tracked content");
    assert!(inspect_source_tree(&root, &entries).is_err());
    fs::write(root.join("Cargo.lock"), b"test lock\n").expect("restore content");
    fs::set_permissions(root.join("Cargo.lock"), fs::Permissions::from_mode(0o600))
        .expect("substitute filesystem mode");
    assert!(capture_tree(&root).is_err());
    fs::set_permissions(root.join("Cargo.lock"), fs::Permissions::from_mode(0o644))
        .expect("restore source mode");
    git_output(
        &root,
        &[
            "update-index",
            "--cacheinfo",
            "160000,aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,iroha-docs",
        ],
    )
    .expect("substitute indexed gitlink commit");
    assert!(inspect_source_tree(&root, &entries).is_err());
}

#[test]
fn assembled_manifest_roundtrips_through_admission_type_and_binds_special_entries() {
    let (_directory, root) = source_fixture();
    let entries = capture_tree(&root).expect("fixture capture");
    let identity = [SOURCE_BRANCH.to_owned(), "a".repeat(40), "b".repeat(40)];
    let manifest =
        assemble_manifest(identity.clone(), entries.clone()).expect("typed source manifest");
    let bytes = json::to_json(&manifest)
        .expect("Norito encoding")
        .into_bytes();
    let decoded: SourceManifestV1 = json::from_slice(&bytes).expect("same admission type");
    assert_eq!(decoded.schema, SOURCE_MANIFEST_SCHEMA_V1);
    assert_eq!(decoded.tracked_files, entries);
    assert_eq!(decoded.closure_sha256, source_closure_sha256(&decoded));
    assert_eq!(decoded.cargo_lock_sha256, entries[0].sha256);
    assert!(decoded.untracked_files.is_empty());
    let mut changed = entries.clone();
    changed[1].sha256 = "0".repeat(64);
    assert_ne!(
        assemble_manifest(identity.clone(), changed)
            .expect("changed manifest")
            .closure_sha256,
        manifest.closure_sha256
    );
    let mut changed = entries.clone();
    changed[2].git_blob_sha1 = "c".repeat(40);
    assert_ne!(
        assemble_manifest(identity.clone(), changed)
            .expect("changed gitlink")
            .closure_sha256,
        manifest.closure_sha256
    );
    assert!(
        assemble_manifest(identity, entries[1..].to_vec()).is_err(),
        "Cargo.lock cannot be omitted"
    );
}

#[test]
fn export_rejects_missing_committed_identity_without_writing_partial_json() {
    let (_directory, root) = source_fixture();
    assert!(
        clean_git_identity(&root).is_err(),
        "an index alone is not a committed checkout"
    );
    let mut output = Vec::new();
    assert!(export_manifest(&root, &mut output).is_err());
    assert!(
        output.is_empty(),
        "failed capture cannot publish a partial manifest"
    );
}

#[test]
fn clean_export_is_consumed_by_exact_admission_capture() {
    let (_directory, root) = committed_source_fixture();
    let mut output = Vec::new();
    export_manifest(&root, &mut output).expect("complete clean fixture export");
    let manifest: SourceManifestV1 = json::from_slice(&output).expect("exact admission decoder");
    let [branch, commit, tree] = clean_git_identity(&root).expect("fixture identity");
    assert_eq!(manifest.branch, branch);
    assert_eq!(manifest.head_commit_sha1, commit);
    assert_eq!(manifest.head_tree_sha1, tree);
    assert_eq!(manifest.closure_sha256, source_closure_sha256(&manifest));
    assert_eq!(
        inspect_source_tree(&root, &manifest.tracked_files).expect("exact capture"),
        manifest.tracked_files
    );
}

#[test]
fn clean_status_flags_cannot_hide_changed_bytes_from_export_or_admission() {
    for flag in ["--assume-unchanged", "--skip-worktree"] {
        let (_directory, root) = committed_source_fixture();
        let mut forged = capture_tree(&root).expect("original source capture");
        git_output(&root, &["update-index", flag, "--", "Cargo.lock"]).expect("hidden-change flag");
        fs::write(root.join("Cargo.lock"), b"evil lock\n").expect("same-length changed file");
        assert!(
            git_output(
                &root,
                &["status", "--porcelain=v1", "--untracked-files=all"]
            )
            .expect("flagged Git status")
            .is_empty(),
            "fixture must reproduce clean-status concealment"
        );
        clean_git_identity(&root).expect("Git identity alone cannot detect the substitution");
        forged[0].sha256 = sha256_hex(b"evil lock\n");
        let mut output = Vec::new();
        assert!(
            export_manifest(&root, &mut output)
                .expect_err("capture must bind Git bytes")
                .to_string()
                .contains("exact indexed Git blob")
        );
        assert!(output.is_empty());
        assert!(
            inspect_source_tree(&root, &forged).is_err(),
            "even a manifest matching the changed file must fail the Git blob binding"
        );
    }
}

#[test]
fn blob_batch_stream_checks_identity_type_size_framing_and_truncation() {
    let object = "a".repeat(40);
    let frame = |header: &str, payload: &[u8]| {
        let mut bytes = header.as_bytes().to_vec();
        bytes.extend_from_slice(payload);
        bytes
    };
    let valid = frame(&format!("{object} blob 4\n"), b"data\n");
    assert_eq!(
        read_blob_digest(&mut valid.as_slice(), &object, 4).expect("bounded blob"),
        sha256_hex(b"data")
    );
    for bytes in [
        frame(&format!("{} blob 4\n", "b".repeat(40)), b"data\n"),
        frame(&format!("{object} tree 4\n"), b"data\n"),
        frame(&format!("{object} blob 5\n"), b"data\n"),
        frame(&format!("{object} blob 04\n"), b"data\n"),
        frame(&format!("{object} missing\n"), b""),
        frame(&format!("{object} blob 4\n"), b"dat"),
        frame(&format!("{object} blob 4\n"), b"data!"),
        vec![b'a'; 129],
    ] {
        assert!(read_blob_digest(&mut bytes.as_slice(), &object, 4).is_err());
    }
    assert!(read_blob_digest(&mut &b""[..], &object, MAX_SOURCE_FILE_BYTES + 1).is_err());
}

#[test]
fn one_owned_git_process_verifies_multiple_indexed_blobs() {
    let (_directory, root) = source_fixture();
    let entries = capture_tree(&root).expect("original source capture");
    let mut blobs = GitBlobReader::new(&root).expect("single batch reader");
    let pid = blobs.child.id();
    for _ in 0..3 {
        for entry in &entries[..2] {
            blobs
                .verify(&entry.git_blob_sha1, entry.size, &entry.sha256)
                .expect("indexed blob");
            assert_eq!(blobs.child.id(), pid, "reuse one process for every request");
        }
    }
    blobs
        .finish()
        .expect("close stdin and reap the one batch child");
}
