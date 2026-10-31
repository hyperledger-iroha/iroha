//! Actual canonical publication controls for the release resolver's admission boundary.
//!
//! These tests are intended as a child of `release_contract`, with no environment mutation.
//! Whole-resolver cache, development fallback, and feature isolation remain resolver tests.

use super::*;
#[cfg(windows)]
use std::sync::OnceLock;
use std::{collections::BTreeMap, fs, path::Path};

use iroha_fs::{OwnerDirectory, PublishMode};
use iroha_release_bundle::{
    bundle::NATIVE_TARGET,
    publish::{PublishInputs, publish},
};

const INVOCATION: &str = "invocation.a1B2c3";

fn parse(values: &BTreeMap<&str, String>) -> Result<Option<ReleaseInputs>> {
    ReleaseInputs::from_values(|name| Ok(values.get(name).cloned()))
}

fn values(artifact: &Path, source: &str, manifest: &str) -> BTreeMap<&'static str, String> {
    let target = artifact
        .join("sumeragi-release")
        .join(source)
        .join("programs")
        .join(INVOCATION);
    BTreeMap::from([
        (SOURCE, source.to_owned()),
        (MANIFEST, manifest.to_owned()),
        (ARTIFACT, artifact.to_str().unwrap().to_owned()),
        (TARGET, target.to_str().unwrap().to_owned()),
        (SKIP, "1".to_owned()),
    ])
}

fn parser_values() -> BTreeMap<&'static str, String> {
    let root = if cfg!(windows) {
        Path::new(r"C:\release-root")
    } else {
        Path::new("/release-root")
    };
    values(root, &"a".repeat(64), &"b".repeat(64))
}

struct Fixture {
    root: tempfile::TempDir,
    repo: PathBuf,
    artifact: PathBuf,
    target: PathBuf,
    source: String,
    manifest: String,
    lock_digest: String,
}

impl Fixture {
    fn new() -> Self {
        Self::with_spelling(b"version = 4\n", Path::to_path_buf)
    }

    fn with_spelling(lock: &[u8], spelling: impl FnOnce(&Path) -> PathBuf) -> Self {
        let root = tempfile::tempdir().unwrap();
        let base = spelling(root.path());
        let repo = base.join("source");
        let artifact = base.join("artifacts");
        let source_owner = OwnerDirectory::open_or_create(&repo).unwrap();
        let artifact_owner = OwnerDirectory::open_or_create(&artifact).unwrap();
        fs::write(repo.join("Cargo.lock"), lock).unwrap();
        let source = "a".repeat(64);
        let programs = artifact_owner
            .ensure_child("sumeragi-release")
            .unwrap()
            .ensure_child(&source)
            .unwrap()
            .ensure_child("programs")
            .unwrap();
        let build = OwnerDirectory::open_or_create(base.join("build")).unwrap();
        for kind in BinaryKind::ALL {
            // A genuine image also permits the command-owner lifetime control below.
            fs::write(
                build.path().join(kind.filename(cfg!(windows))),
                crate::native_program_fixture::image(),
            )
            .unwrap();
        }
        let tools = OwnerDirectory::open_or_create(base.join("tools")).unwrap();
        tools
            .write_atomic("cargo", b"cargo 1.93.1 (fixture)\n", PublishMode::CreateNew)
            .unwrap();
        tools
            .write_atomic(
                "rustc",
                format!("rustc 1.93.1 (fixture)\nhost: {NATIVE_TARGET}\n").as_bytes(),
                PublishMode::CreateNew,
            )
            .unwrap();
        let bundle = publish(
            &programs,
            INVOCATION,
            PublishInputs {
                source_sha256: &source,
                build_directory: build.path(),
                cargo_lock: &repo.join("Cargo.lock"),
                cargo_version: &tools.path().join("cargo"),
                rustc_version: &tools.path().join("rustc"),
            },
        )
        .unwrap();
        let target = bundle.path().to_owned();
        let manifest = bundle.manifest_sha256().to_owned();
        let lock_digest = bundle.manifest().cargo_lock_sha256.clone();
        bundle.revalidate().unwrap();
        drop(bundle);
        drop(source_owner);
        Self {
            root,
            repo,
            artifact,
            target,
            source,
            manifest,
            lock_digest,
        }
    }

    fn values(&self) -> BTreeMap<&'static str, String> {
        let mut result = values(&self.artifact, &self.source, &self.manifest);
        result.insert(LOCK, self.lock_digest.clone());
        assert_eq!(result[TARGET], self.target.to_str().unwrap());
        result
    }

    fn open(&self) -> Result<Arc<ReleaseProgramContract>> {
        ReleaseProgramContract::open(&self.repo, parse(&self.values())?.unwrap())
    }
}

#[cfg(windows)]
fn current_sid() -> &'static str {
    static SID: OnceLock<String> = OnceLock::new();
    SID.get_or_init(|| {
        let output = std::process::Command::new("whoami")
            .args(["/user", "/fo", "csv", "/nh"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        let sid = text.rsplit(',').next().unwrap().trim().trim_matches('"');
        assert!(
            sid.starts_with("S-1-")
                && sid
                    .bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'S' | b'-'))
        );
        sid.to_owned()
    })
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Restore disposal rights only after each test's admitted owners have dropped.
        // This is fixture cleanup, never a repair performed during admission.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let _ = fs::set_permissions(&self.target, fs::Permissions::from_mode(0o700));
        }
        #[cfg(windows)]
        {
            let _ = std::process::Command::new("icacls")
                .arg(self.root.path())
                .arg("/grant:r")
                .arg(format!("*{}:(OI)(CI)F", current_sid()))
                .args(["/T", "/Q"])
                .output();
        }
        assert!(self.root.path().exists());
    }
}

#[test]
fn inactive_inputs_require_no_paths_or_checkout() {
    let mut observed = Vec::new();
    let result = ReleaseInputs::from_values(|name| {
        observed.push(name.to_owned());
        assert!(matches!(name, SOURCE | MANIFEST | LOCK));
        Ok(None)
    });
    assert!(result.unwrap().is_none());
    assert_eq!(observed, [SOURCE, MANIFEST, LOCK]);
}

#[test]
fn partial_or_malformed_independent_anchors_refuse_before_paths() {
    for (present, required) in [(SOURCE, MANIFEST), (MANIFEST, SOURCE), (LOCK, SOURCE)] {
        let supplied = BTreeMap::from([(present, "a".repeat(64))]);
        let error = parse(&supplied).unwrap_err();
        assert!(error.to_string().contains(required), "{error}");
    }
    for key in [SOURCE, MANIFEST, LOCK] {
        for invalid in [
            String::new(),
            "A".repeat(64),
            "g".repeat(64),
            "a".repeat(63),
        ] {
            let mut supplied = parser_values();
            supplied.insert(key, invalid);
            assert!(parse(&supplied).is_err(), "accepted malformed {key}");
        }
    }
}

#[test]
fn lookup_only_skip_and_explicit_roots_are_mandatory() {
    for key in [SKIP, ARTIFACT, TARGET] {
        let mut supplied = parser_values();
        supplied.remove(key);
        assert!(parse(&supplied).is_err(), "accepted missing {key}");
    }
    for invalid in ["", "0", "true", " 1", "1\n"] {
        let mut supplied = parser_values();
        supplied.insert(SKIP, invalid.to_owned());
        assert!(parse(&supplied).is_err());
    }
}

#[test]
fn environment_observation_errors_cannot_select_the_inactive_branch() {
    let error =
        ReleaseInputs::from_values(|_| Err(eyre!("non-Unicode environment value"))).unwrap_err();
    assert!(error.to_string().contains("non-Unicode"));
}

#[test]
fn invocation_namespace_and_raw_path_spelling_are_exact() {
    let supplied = parser_values();
    let good = PathBuf::from(&supplied[TARGET]);
    let parent = good.parent().unwrap();
    for name in [
        "invocation.",
        "invocation.short",
        "invocation.tooLong",
        "invocation.bad-12",
        "wrong.a1B2c3",
    ] {
        let mut candidate = supplied.clone();
        candidate.insert(TARGET, parent.join(name).to_str().unwrap().to_owned());
        assert!(parse(&candidate).is_err(), "accepted {name}");
    }
    for bad in [
        "relative/path".to_owned(),
        format!("{}\n", supplied[ARTIFACT]),
    ] {
        let mut candidate = supplied.clone();
        candidate.insert(ARTIFACT, bad);
        assert!(parse(&candidate).is_err());
    }
    let separator = if cfg!(windows) { '\\' } else { '/' };
    let path = &supplied[ARTIFACT];
    for suffix in [
        format!("{separator}.{separator}child"),
        format!("{separator}{separator}child"),
        format!("{separator}..{separator}child"),
    ] {
        assert!(exact_path(&format!("{path}{suffix}"), ARTIFACT).is_err());
    }
    let mut nested = supplied;
    nested.insert(TARGET, good.join(INVOCATION).to_str().unwrap().to_owned());
    assert!(parse(&nested).is_err());
}

#[test]
fn genuine_publication_selects_all_four_exact_native_names() {
    let fixture = Fixture::new();
    let contract = fixture.open().unwrap();
    for kind in BinaryKind::ALL {
        let expected = fixture.target.join(kind.filename(cfg!(windows)));
        assert_eq!(
            contract
                .select(kind, None)
                .unwrap()
                .diagnostic_path()
                .unwrap(),
            expected
        );
        assert!(contract.select(kind, Some(&expected)).is_ok());
        assert!(
            contract
                .select(kind, Some(&fixture.repo.join(kind.filename(cfg!(windows)))))
                .is_err()
        );
    }
    contract.revalidate().unwrap();
}

#[test]
fn wrong_external_anchors_never_admit_the_existing_publication() {
    let fixture = Fixture::new();
    for key in [MANIFEST, LOCK] {
        let mut supplied = fixture.values();
        supplied.insert(key, "b".repeat(64));
        assert!(
            ReleaseProgramContract::open(&fixture.repo, parse(&supplied).unwrap().unwrap())
                .is_err()
        );
    }
    let mut supplied = values(&fixture.artifact, &"b".repeat(64), &fixture.manifest);
    supplied.insert(LOCK, fixture.lock_digest.clone());
    let missing = PathBuf::from(&supplied[TARGET]);
    assert!(
        ReleaseProgramContract::open(&fixture.repo, parse(&supplied).unwrap().unwrap()).is_err()
    );
    assert!(
        !missing.exists(),
        "admission must not build a replacement namespace"
    );
    fixture.open().unwrap();
}

#[test]
fn current_lock_is_independent_even_without_optional_lock_environment() {
    let fixture = Fixture::new();
    let mut supplied = fixture.values();
    supplied.remove(LOCK);
    ReleaseProgramContract::open(&fixture.repo, parse(&supplied).unwrap().unwrap()).unwrap();
    fs::write(fixture.repo.join("Cargo.lock"), b"version = 3\n").unwrap();
    let error = ReleaseProgramContract::open(&fixture.repo, parse(&supplied).unwrap().unwrap())
        .unwrap_err();
    assert!(
        error.to_string().contains("lockfile anchor differs"),
        "{error}"
    );
    let error = fixture.open().unwrap_err();
    assert!(error.to_string().contains(LOCK), "{error}");
}

#[test]
fn empty_regular_lock_matches_publisher_policy_but_oversized_lock_refuses() {
    let fixture = Fixture::with_spelling(b"", Path::to_path_buf);
    fixture.open().unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(fixture.repo.join("Cargo.lock"))
        .unwrap()
        .set_len(MAX_LOCK + 1)
        .unwrap();
    assert!(fixture.open().is_err());
}

#[test]
fn substituted_hardlinked_lock_is_never_source_authority() {
    let fixture = Fixture::new();
    fs::hard_link(
        fixture.repo.join("Cargo.lock"),
        fixture.repo.join("lock-alias"),
    )
    .unwrap();
    assert!(fixture.open().is_err());
    fs::remove_file(fixture.repo.join("lock-alias")).unwrap();
    fixture.open().unwrap();
}

#[test]
fn active_release_requires_existing_source_checkout_and_lock() {
    let fixture = Fixture::new();
    let missing = fixture.repo.join("missing-checkout");
    let error = ReleaseProgramContract::open(&missing, parse(&fixture.values()).unwrap().unwrap())
        .unwrap_err();
    assert!(
        error.to_string().contains("original release source root"),
        "{error}"
    );
    assert!(!missing.exists());
    fs::remove_file(fixture.repo.join("Cargo.lock")).unwrap();
    assert!(fixture.open().is_err());
    assert!(!fixture.repo.join("Cargo.lock").exists());
}

fn directory_link(source: &Path, alias: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(source, alias).unwrap();
    #[cfg(windows)]
    {
        // Genuine directory junctions exercise native reparse refusal without symlink privilege.
        let output = std::process::Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(alias)
            .arg(source)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
fn remove_directory_link(alias: &Path) {
    #[cfg(unix)]
    fs::remove_file(alias).unwrap();
    #[cfg(windows)]
    fs::remove_dir(alias).unwrap();
}
#[test]
fn linked_source_root_is_refused_without_canonicalizing_through_it() {
    let fixture = Fixture::new();
    let alias = fixture.root.path().join("source-alias");
    directory_link(&fixture.repo, &alias);
    assert!(
        ReleaseProgramContract::open(&alias, parse(&fixture.values()).unwrap().unwrap()).is_err()
    );
    fixture.open().unwrap();
    remove_directory_link(&alias);
}
#[test]
fn linked_artifact_root_and_every_publication_parent_are_refused() {
    let fixture = Fixture::new();
    fixture.open().unwrap();
    let alias = fixture.root.path().join("artifact-alias");
    directory_link(&fixture.artifact, &alias);
    let supplied = values(&alias, &fixture.source, &fixture.manifest);
    assert!(PrivateDirectory::open_exact(&alias).is_err());
    assert!(
        ReleaseProgramContract::open(&fixture.repo, parse(&supplied).unwrap().unwrap()).is_err()
    );
    remove_directory_link(&alias);
    for depth in 1..=3 {
        let fixture = Fixture::new();
        fixture.open().unwrap();
        let mut parent = fixture.artifact.clone();
        for name in ["sumeragi-release", fixture.source.as_str(), "programs"]
            .into_iter()
            .take(depth)
        {
            parent.push(name);
        }
        let displaced = fixture.root.path().join("displaced-original-parent");
        fs::rename(&parent, &displaced).unwrap();
        directory_link(&displaced, &parent);
        assert!(ReaderDirectory::open(&parent).is_err());
        assert!(fixture.open().is_err());
        remove_directory_link(&parent);
        fs::rename(&displaced, &parent).unwrap();
        fixture.open().unwrap();
    }
}

#[test]
fn artifact_root_may_not_equal_or_contain_or_descend_from_source() {
    let fixture = Fixture::new();
    let child_artifact = fixture.repo.join("nested-artifacts");
    let child_source = fixture.artifact.join("nested-source");
    drop(OwnerDirectory::open_or_create(&child_artifact).unwrap());
    drop(OwnerDirectory::open_or_create(&child_source).unwrap());
    for (repo, artifact) in [
        (&fixture.repo, &fixture.repo),
        (&fixture.repo, &child_artifact),
        (&child_source, &fixture.artifact),
    ] {
        let supplied = values(artifact, &fixture.source, &fixture.manifest);
        let error =
            ReleaseProgramContract::open(repo, parse(&supplied).unwrap().unwrap()).unwrap_err();
        assert!(error.to_string().contains("must not overlap"), "{error}");
    }
}

fn add_foreign_directory_read(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    #[cfg(windows)]
    {
        let result = std::process::Command::new("icacls")
            .arg(path)
            .args(["/grant", "*S-1-1-0:(RX)", "/Q"])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn every_mutable_publication_parent_requires_exact_private_policy() {
    for depth in 0..4 {
        let fixture = Fixture::new();
        let mut parent = fixture.artifact.clone();
        for component in ["sumeragi-release", fixture.source.as_str(), "programs"]
            .into_iter()
            .take(depth)
        {
            parent.push(component);
        }
        add_foreign_directory_read(&parent);
        // Generic read custody remains valid; exact private custody must refuse.
        ReaderDirectory::open(&parent)
            .unwrap()
            .revalidate()
            .unwrap();
        assert!(
            fixture.open().is_err(),
            "accepted non-private depth {depth}"
        );
    }
}

#[test]
fn cached_selection_and_unlaunched_command_retain_complete_contract() {
    let fixture = Fixture::new();
    let contract = fixture.open().unwrap();
    let weak = Arc::downgrade(&contract);
    let selected = contract.select(BinaryKind::Iroha, None).unwrap();
    let cached = selected.clone();
    drop(contract);
    drop(selected);
    assert!(weak.upgrade().is_some());
    let command = cached.command().unwrap();
    drop(cached);
    assert!(weak.upgrade().is_some());
    drop(command);
    assert!(weak.upgrade().is_none());
}

#[cfg(windows)]
#[test]
fn native_ordinary_and_verbatim_spellings_are_each_preserved_without_alias_fallback() {
    for verbatim in [false, true] {
        let fixture = Fixture::with_spelling(b"version = 4\n", |path| {
            let canonical = path.canonicalize().unwrap();
            let text = canonical.to_str().unwrap();
            assert!(text.starts_with(r"\\?\"));
            if verbatim {
                canonical
            } else {
                PathBuf::from(text.strip_prefix(r"\\?\").unwrap())
            }
        });
        let contract = fixture.open().unwrap();
        let expected = fixture.target.join(BinaryKind::Iroha.filename(true));
        let selected = contract.select(BinaryKind::Iroha, Some(&expected)).unwrap();
        let output = selected.command().unwrap().output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"resolver-probe\n");
        let alias = if verbatim {
            PathBuf::from(expected.to_str().unwrap().strip_prefix(r"\\?\").unwrap())
        } else {
            PathBuf::from(format!(r"\\?\{}", expected.display()))
        };
        assert_ne!(expected.as_os_str(), alias.as_os_str());
        assert!(contract.select(BinaryKind::Iroha, Some(&alias)).is_err());
        let case_alias = expected.with_file_name("IROHA.EXE");
        assert!(
            contract
                .select(BinaryKind::Iroha, Some(&case_alias))
                .is_err()
        );
        let mut supplied = fixture.values();
        supplied.insert(TARGET, alias.parent().unwrap().to_str().unwrap().to_owned());
        assert!(parse(&supplied).is_err());
    }
}

#[cfg(unix)]
#[test]
fn cached_selection_rechecks_independent_lock() {
    let fixture = Fixture::new();
    let contract = fixture.open().unwrap();
    let selected = contract.select(BinaryKind::Iroha, None).unwrap();
    drop(contract);
    assert!(selected.command().is_ok());
    fs::write(fixture.repo.join("Cargo.lock"), b"version = 3\n").unwrap();
    assert!(selected.command().is_err());
}

#[test]
fn cached_selection_rechecks_private_ancestry() {
    let fixture = Fixture::new();
    let contract = fixture.open().unwrap();
    let selected = contract.select(BinaryKind::Iroha, None).unwrap();
    drop(contract);
    assert!(selected.command().is_ok());
    add_foreign_directory_read(&fixture.artifact);
    assert!(selected.command().is_err());
}
