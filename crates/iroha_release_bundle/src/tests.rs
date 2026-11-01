//! Publication interruptions, source-pin drift and independent bundle authentication.

#[cfg(windows)]
#[path = "windows_tests.rs"]
mod windows;

#[path = "admission_tests.rs"]
mod admission;

#[cfg(any(windows, target_os = "linux", target_os = "android"))]
#[path = "admission_custody_tests.rs"]
mod admission_custody;

use std::{fs, io, path::PathBuf, sync::Arc};

use iroha_fs::{FlatBatchSealStep, OwnerDirectory, ReaderDirectory};

use crate::{
    bundle::{Bundle, BundleAnchors, NATIVE_TARGET, digest, stream_digest},
    manifest::{BinaryKind, MANIFEST_NAME},
    publish::{PublishInputs, PublishStep, publish, publish_observed},
};

struct Fixture {
    owner: OwnerDirectory,
    build: PathBuf,
    lock: PathBuf,
    cargo: PathBuf,
    rustc: PathBuf,
    source: String,
    root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let owner = OwnerDirectory::open_or_create(root.path().join("published")).unwrap();
        let build = root.path().join("build");
        fs::create_dir(&build).unwrap();
        for kind in BinaryKind::ALL {
            fs::write(
                build.join(kind.filename(cfg!(windows))),
                format!("original build input {kind:?}\n"),
            )
            .unwrap();
        }
        let lock = root.path().join("Cargo.lock");
        fs::write(&lock, b"version = 4\n").unwrap();
        let controls = OwnerDirectory::open_or_create(root.path().join("tools")).unwrap();
        let cargo = controls.path().join("cargo-version.txt");
        controls
            .write_atomic(
                "cargo-version.txt",
                b"cargo 1.93.1 (diagnostic fixture)\n",
                iroha_fs::PublishMode::CreateNew,
            )
            .unwrap();
        let rustc = controls.path().join("rustc-version.txt");
        controls
            .write_atomic(
                "rustc-version.txt",
                format!("rustc 1.93.1 (diagnostic fixture)\nhost: {NATIVE_TARGET}\n").as_bytes(),
                iroha_fs::PublishMode::CreateNew,
            )
            .unwrap();
        Self {
            root,
            owner,
            build,
            lock,
            cargo,
            rustc,
            source: "a".repeat(64),
        }
    }

    fn inputs(&self) -> PublishInputs<'_> {
        PublishInputs {
            source_sha256: &self.source,
            build_directory: &self.build,
            cargo_lock: &self.lock,
            cargo_version: &self.cargo,
            rustc_version: &self.rustc,
        }
    }

    fn anchors<'a>(&'a self, bundle: &'a Bundle) -> BundleAnchors<'a> {
        BundleAnchors {
            source: &self.source,
            cargo_lock: &bundle.manifest().cargo_lock_sha256,
            manifest: bundle.manifest_sha256(),
        }
    }

    #[cfg(any(windows, target_os = "linux", target_os = "android"))]
    fn use_native_probe(&self) {
        static PROBE: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
        let bytes = PROBE.get_or_init(|| {
            let temporary = tempfile::tempdir().unwrap();
            let source = temporary.path().join("release_probe.rs");
            fs::write(
                &source,
                br#"//! Original native process fixture for release custody.
use std::io::{Read as _, Write as _};
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("input") => {
            println!("ready");
            std::io::stdout().flush().unwrap();
            let mut bytes = Vec::new();
            std::io::stdin().read_to_end(&mut bytes).unwrap();
        }
        Some("root-exit") => {
            let child = std::process::Command::new(std::env::current_exe().unwrap())
                .arg("input").stdin(std::process::Stdio::inherit())
                .stdout(std::process::Stdio::inherit()).stderr(std::process::Stdio::inherit())
                .spawn().unwrap();
            drop(child);
        }
        Some("flood") => {
            std::io::stdout().write_all(&vec![b'o'; 131_072]).unwrap();
            std::io::stderr().write_all(&vec![b'e'; 131_072]).unwrap();
        }
        Some("cwd") => {
            print!("{}", std::env::current_dir().unwrap().display());
        }
        Some("env") => {
            print!("{}:{}:{}", std::env::var("IROHA_RELEASE_TEST_A").unwrap(), std::env::var("IROHA_RELEASE_TEST_B").unwrap(), std::env::var("IROHA_RELEASE_TEST_REMOVED").is_err());
        }
        _ => { print!("release-original"); eprint!("release-stderr"); }
    }
    std::process::exit(19);
}
"#,
            )
            .unwrap();
            let executable =
                temporary
                    .path()
                    .join(if cfg!(windows) { "probe.exe" } else { "probe" });
            let output = std::process::Command::new(
                std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()),
            )
            .args([
                "--edition=2024",
                "--crate-name=iroha_release_probe",
                "-C",
                "debuginfo=0",
                "-C",
                "opt-level=s",
                "-C",
                "strip=symbols",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            fs::read(executable).unwrap()
        });
        for kind in BinaryKind::ALL {
            fs::write(self.build.join(kind.filename(cfg!(windows))), bytes).unwrap();
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // All release owners in these tests are dropped before their fixture. Restoring
        // access here only removes this test's disposable namespace; production never
        // repairs or deletes a partially sealed publication.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            if let Ok(entries) = fs::read_dir(self.owner.path()) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        let _ =
                            fs::set_permissions(entry.path(), fs::Permissions::from_mode(0o700));
                    }
                }
            }
        }
        #[cfg(windows)]
        {
            use std::process::Command;
            if let Ok(identity) = Command::new("whoami")
                .args(["/user", "/fo", "csv", "/nh"])
                .output()
                && identity.status.success()
            {
                let identity = String::from_utf8_lossy(&identity.stdout);
                let sid = identity
                    .rsplit(',')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_matches('"');
                if sid.starts_with("S-1-")
                    && sid
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'S' | b'-'))
                {
                    let _ = Command::new("icacls")
                        .arg(self.owner.path())
                        .arg("/grant:r")
                        .arg(format!("*{sid}:(OI)(CI)F"))
                        .args(["/T", "/Q"])
                        .output();
                }
            }
        }
        // Keep the owner and temporary root used above visibly associated.
        assert!(self.owner.path().starts_with(self.root.path()));
    }
}

#[test]
fn bounded_hash_reads_whole_input_and_rejects_one_extra_byte() {
    assert_eq!(stream_digest(&b""[..], 0).unwrap(), (digest(b""), 0));
    let bytes = vec![37_u8; 2 * 64 * 1024 + 3];
    assert_eq!(
        stream_digest(bytes.as_slice(), bytes.len() as u64).unwrap(),
        (digest(&bytes), bytes.len() as u64)
    );
    assert!(stream_digest(bytes.as_slice(), bytes.len() as u64 - 1).is_err());
}

#[test]
fn complete_publication_and_recovery_keep_exact_flat_inventory_and_external_anchors() {
    let fixture = Fixture::new();
    let bundle = publish(&fixture.owner, "invocation.good", fixture.inputs()).unwrap();
    let path = bundle.path().to_owned();
    let mut actual: Vec<_> = fs::read_dir(&path)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    let mut expected: Vec<std::ffi::OsString> = BinaryKind::ALL
        .into_iter()
        .map(|kind| kind.filename(cfg!(windows)).into())
        .collect();
    expected.push(MANIFEST_NAME.into());
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    assert_eq!(
        bundle.manifest().cargo_lock_sha256,
        digest(&fs::read(&fixture.lock).unwrap())
    );
    assert_eq!(
        bundle.manifest().cargo_version_sha256,
        digest(&fs::read(&fixture.cargo).unwrap())
    );
    assert_eq!(
        bundle.manifest().rustc_version_sha256,
        digest(&fs::read(&fixture.rustc).unwrap())
    );
    for kind in BinaryKind::ALL {
        let program = bundle.program(kind);
        assert_eq!(program.kind(), kind);
        assert_eq!(
            program.diagnostic_path().unwrap(),
            path.join(kind.filename(cfg!(windows)))
        );
        assert_eq!(
            bundle.manifest().binaries[kind.index()].sha256,
            digest(&fs::read(fixture.build.join(kind.filename(cfg!(windows)))).unwrap())
        );
        program.revalidate().unwrap();
    }
    let reader = ReaderDirectory::open(fixture.owner.path()).unwrap();
    let recovered = Bundle::open(&reader, "invocation.good", fixture.anchors(&bundle)).unwrap();
    assert_eq!(recovered.manifest(), bundle.manifest());
    assert_eq!(recovered.manifest_sha256(), bundle.manifest_sha256());
    assert!(publish(&fixture.owner, "invocation.good", fixture.inputs()).is_err());
    let wrong = "b".repeat(64);
    let anchors = fixture.anchors(&bundle);
    for anchors in [
        BundleAnchors {
            source: &wrong,
            ..anchors
        },
        BundleAnchors {
            cargo_lock: &wrong,
            ..anchors
        },
        BundleAnchors {
            manifest: &wrong,
            ..anchors
        },
        BundleAnchors {
            manifest: "not a digest",
            ..anchors
        },
    ] {
        assert!(Bundle::open(&reader, "invocation.good", anchors).is_err());
    }
    let program = bundle.program(BinaryKind::Kagami);
    let weak = Arc::downgrade(&bundle);
    drop(bundle);
    assert!(weak.upgrade().is_some());
    program.revalidate().unwrap();
    drop(program);
    assert!(weak.upgrade().is_none());
    recovered.revalidate().unwrap();
}

#[test]
fn every_publication_boundary_preserves_its_claimed_inventory_on_error() {
    let mut boundaries = vec![PublishStep::InputsPinned];
    boundaries.extend((0..4).map(PublishStep::ExecutableCopied));
    boundaries.push(PublishStep::ManifestWritten);
    boundaries
        .extend((0..5).map(|index| PublishStep::Sealing(FlatBatchSealStep::FileSealed(index))));
    boundaries.push(PublishStep::Sealing(FlatBatchSealStep::DirectorySealed));
    boundaries.push(PublishStep::Sealing(FlatBatchSealStep::ParentTransferred));
    for index in 0..5 {
        boundaries.push(PublishStep::Sealing(FlatBatchSealStep::ChildBridged(index)));
        boundaries.push(PublishStep::Sealing(FlatBatchSealStep::ChildTransferred(
            index,
        )));
    }
    boundaries.push(PublishStep::Sealing(FlatBatchSealStep::Validate));
    boundaries.push(PublishStep::Authenticated);
    for (index, stop) in boundaries.into_iter().enumerate() {
        let fixture = Fixture::new();
        let name = format!("invocation.failure{index}");
        let mut visited = false;
        let error = publish_observed(&fixture.owner, &name, fixture.inputs(), |step| {
            if step == stop {
                visited = true;
                Err(io::Error::other("injected publication interruption"))
            } else {
                Ok(())
            }
        })
        .unwrap_err();
        assert!(visited, "{stop:?}: {error}");
        assert_eq!(error.to_string(), "injected publication interruption");
        let path = fixture.owner.path().join(name);
        if stop == PublishStep::InputsPinned {
            assert!(!path.exists());
        } else {
            assert_eq!(fs::read_dir(&path).unwrap().count(), 5);
            assert!(path.join(MANIFEST_NAME).is_file());
        }
    }
}

#[test]
fn missing_builds_empty_images_and_foreign_tools_refuse_before_claiming_destination() {
    let fixture = Fixture::new();
    let program = fixture
        .build
        .join(BinaryKind::IrohadTaira.filename(cfg!(windows)));
    fs::remove_file(&program).unwrap();
    assert!(publish(&fixture.owner, "missing", fixture.inputs()).is_err());
    fs::write(&program, b"").unwrap();
    assert!(publish(&fixture.owner, "empty", fixture.inputs()).is_err());
    fs::write(&program, b"present").unwrap();
    for bad in [
        "rustc 1\nhost: foreign-host\n".to_owned(),
        format!("rustc 1\nhost: {NATIVE_TARGET}\nhost: {NATIVE_TARGET}\n"),
        "cargo 1\n".to_owned(),
        "rustc 1\nhost: invalid\0target\n".to_owned(),
        format!("rustc 1\r\nhost: {NATIVE_TARGET}\r\n"),
        format!("rustc 1\nhost: {NATIVE_TARGET}"),
        format!("rustc 1\0\nhost: {NATIVE_TARGET}\n"),
    ] {
        fs::write(&fixture.rustc, bad).unwrap();
        assert!(publish(&fixture.owner, "bad-tool", fixture.inputs()).is_err());
    }
    assert_eq!(fs::read_dir(fixture.owner.path()).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn public_tool_transcript_is_not_admitted_as_owner_private_binding_evidence() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = Fixture::new();
    fs::set_permissions(&fixture.cargo, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(publish(&fixture.owner, "invocation.public-tools", fixture.inputs()).is_err());
    assert_eq!(fs::read_dir(fixture.owner.path()).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn each_original_build_tool_and_lockfile_pin_rejects_equal_length_byte_mutation() {
    for selected in 0..7 {
        let fixture = Fixture::new();
        let path = match selected {
            0..=3 => fixture
                .build
                .join(BinaryKind::ALL[selected].filename(false)),
            4 => fixture.lock.clone(),
            5 => fixture.cargo.clone(),
            _ => fixture.rustc.clone(),
        };
        let error = publish_observed(
            &fixture.owner,
            "invocation.drift",
            fixture.inputs(),
            |step| {
                if step == PublishStep::ManifestWritten {
                    let mut bytes = fs::read(&path)?;
                    bytes[0] ^= 1;
                    fs::write(&path, bytes)?;
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("changed"), "{selected}: {error}");
        assert_eq!(
            fs::read_dir(fixture.owner.path().join("invocation.drift"))
                .unwrap()
                .count(),
            5
        );
    }
}

#[cfg(unix)]
#[test]
fn recovered_admission_refuses_equal_byte_substitution_of_each_original_and_parent() {
    use std::os::unix::fs::PermissionsExt as _;
    for selected in 0..6 {
        let fixture = Fixture::new();
        let bundle = publish(&fixture.owner, "invocation.substitution", fixture.inputs()).unwrap();
        let path = bundle.path().to_owned();
        let lock = bundle.manifest().cargo_lock_sha256.clone();
        let manifest = bundle.manifest_sha256().to_owned();
        let names: Vec<_> = BinaryKind::ALL
            .into_iter()
            .map(|kind| kind.filename(false))
            .chain([MANIFEST_NAME])
            .collect();
        drop(bundle);
        let original = fixture.root.path().join("preserved-original");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        if selected < 5 {
            fs::rename(path.join(names[selected]), &original).unwrap();
            fs::copy(&original, path.join(names[selected])).unwrap();
        } else {
            fs::rename(&path, &original).unwrap();
            fs::create_dir(&path).unwrap();
            for name in &names {
                fs::copy(original.join(name), path.join(name)).unwrap();
            }
        }
        fs::set_permissions(&path, fs::Permissions::from_mode(0o500)).unwrap();
        let reader = ReaderDirectory::open(fixture.owner.path()).unwrap();
        let error = Bundle::open(
            &reader,
            "invocation.substitution",
            BundleAnchors {
                source: &fixture.source,
                cargo_lock: &lock,
                manifest: &manifest,
            },
        )
        .unwrap_err();
        assert!(
            error.to_string().contains("substituted"),
            "{selected}: {error}"
        );
        // Retaining the moved original also prevents inode reuse in this regression.
        assert!(original.exists());
        if selected == 5 {
            fs::set_permissions(original, fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
}

#[cfg(windows)]
#[test]
fn all_original_build_tool_and_lockfile_pins_exclude_writers_and_deletion() {
    let fixture = Fixture::new();
    let mut paths: Vec<_> = BinaryKind::ALL
        .into_iter()
        .map(|kind| fixture.build.join(kind.filename(true)))
        .collect();
    paths.extend([
        fixture.lock.clone(),
        fixture.cargo.clone(),
        fixture.rustc.clone(),
    ]);
    let bundle = publish_observed(
        &fixture.owner,
        "invocation.pinned",
        fixture.inputs(),
        |step| {
            if step == PublishStep::ManifestWritten {
                for (index, path) in paths.iter().enumerate() {
                    assert!(fs::OpenOptions::new().write(true).open(path).is_err());
                    assert!(
                        fs::rename(path, fixture.root.path().join(format!("stolen{index}")))
                            .is_err()
                    );
                }
            }
            Ok(())
        },
    )
    .unwrap();
    bundle.revalidate().unwrap();
}

#[cfg(any(windows, target_os = "linux", target_os = "android"))]
#[test]
fn all_four_native_commands_execute_and_capture_both_pipes_with_originals_retained() {
    let fixture = Fixture::new();
    fixture.use_native_probe();
    let bundle = publish(&fixture.owner, "invocation.native", fixture.inputs()).unwrap();
    for kind in BinaryKind::ALL {
        let output = bundle.program(kind).command().unwrap().output().unwrap();
        assert_eq!(output.status.code(), Some(19));
        assert_eq!(output.stdout, b"release-original");
        assert_eq!(output.stderr, b"release-stderr");
    }
    let mut command = bundle.program(BinaryKind::Iroha).command().unwrap();
    command.configure(|command| {
        command.arg("flood");
    });
    let output = command.output().unwrap();
    assert_eq!(output.stdout, vec![b'o'; 131_072]);
    assert_eq!(output.stderr, vec![b'e'; 131_072]);
    assert_eq!(output.status.code(), Some(19));
    let mut command = bundle.program(BinaryKind::Kagami).command().unwrap();
    command.configure(|command| {
        command.arg("cwd").current_dir(fixture.root.path());
    });
    let output = command.output().unwrap();
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        fixture.root.path().to_string_lossy()
    );
    let mut command = bundle.program(BinaryKind::Irohad).command().unwrap();
    command.configure(|command| {
        command
            .stdout(iroha_fs::ProcessStdio::null())
            .stderr(iroha_fs::ProcessStdio::null());
    });
    assert_eq!(command.status().unwrap().code(), Some(19));
    bundle.revalidate().unwrap();
}

#[cfg(any(windows, target_os = "linux", target_os = "android"))]
#[test]
fn configuration_facade_changes_arguments_environment_and_streams_without_program_access() {
    let fixture = Fixture::new();
    fixture.use_native_probe();
    let bundle = publish(&fixture.owner, "invocation.command", fixture.inputs()).unwrap();
    let mut command = bundle.program(BinaryKind::Iroha).command().unwrap();
    command.configure(|command| {
        command
            .env_clear()
            .args(["env"])
            .env("IROHA_RELEASE_TEST_A", "one")
            .envs([
                ("IROHA_RELEASE_TEST_B", "two"),
                ("IROHA_RELEASE_TEST_REMOVED", "old"),
            ])
            .env_remove("IROHA_RELEASE_TEST_REMOVED");
        assert!(
            command
                .get_envs()
                .any(|(key, value)| key == "IROHA_RELEASE_TEST_A"
                    && value == Some(std::ffi::OsStr::new("one")))
        );
        // The ordinary system environment is not required by this constant-only fixture.
    });
    let output = command.output().unwrap();
    assert_eq!(output.status.code(), Some(19));
    assert_eq!(output.stdout, b"one:two:true");
    bundle.revalidate().unwrap();
}

#[cfg(any(windows, target_os = "linux", target_os = "android"))]
#[test]
fn supervisor_drop_transfers_original_custody_until_the_actual_child_exits() {
    use std::io::BufRead as _;
    let fixture = Fixture::new();
    fixture.use_native_probe();
    let bundle = publish(&fixture.owner, "invocation.supervised", fixture.inputs()).unwrap();
    let weak = Arc::downgrade(&bundle);
    let mut command = bundle.program(BinaryKind::Irohad).command().unwrap();
    command.configure(|command| {
        command
            .arg("input")
            .stdin(iroha_fs::ProcessStdio::piped())
            .stdout(iroha_fs::ProcessStdio::piped());
    });
    let mut child = command.spawn().unwrap();
    let input = child.stdin.take().unwrap();
    let mut output = std::io::BufReader::new(child.stdout.take().unwrap());
    let mut ready = String::new();
    output.read_line(&mut ready).unwrap();
    assert_eq!(ready.trim_end(), "ready");
    assert!(child.id() > 0);
    assert!(child.try_wait().unwrap().is_none());
    drop(bundle);
    assert!(weak.upgrade().is_some());
    drop(child);
    assert!(
        weak.upgrade().is_some(),
        "reaper must retain all originals while input remains open"
    );
    drop(input);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while weak.upgrade().is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(weak.upgrade().is_none(), "original child was not reaped");
}

#[cfg(any(windows, target_os = "linux", target_os = "android"))]
#[test]
fn explicit_and_drop_termination_keep_custody_through_reaping() {
    for on_drop in [false, true] {
        let fixture = Fixture::new();
        fixture.use_native_probe();
        let bundle = publish(&fixture.owner, "invocation.termination", fixture.inputs()).unwrap();
        let weak = Arc::downgrade(&bundle);
        let mut command = bundle.program(BinaryKind::Irohad).command().unwrap();
        command.kill_on_drop(on_drop).configure(|command| {
            command
                .arg("input")
                .stdin(iroha_fs::ProcessStdio::piped())
                .stdout(iroha_fs::ProcessStdio::null());
        });
        let mut child = command.spawn().unwrap();
        let input = child.stdin.take().unwrap();
        drop(bundle);
        if !on_drop {
            child.kill().unwrap();
            assert!(!child.wait().unwrap().success());
            assert!(child.try_wait().unwrap().is_some());
        }
        drop(child);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while weak.upgrade().is_some() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(weak.upgrade().is_none());
        drop(input);
    }
}

#[cfg(target_os = "linux")]
#[test]
fn terminal_wait_reports_postlaunch_custody_drift_after_reaping_the_original_child() {
    use std::os::unix::fs::PermissionsExt as _;
    let fixture = Fixture::new();
    fixture.use_native_probe();
    let bundle = publish(&fixture.owner, "invocation.exit-drift", fixture.inputs()).unwrap();
    let mut command = bundle.program(BinaryKind::Irohad).command().unwrap();
    command.configure(|command| {
        command
            .stdout(iroha_fs::ProcessStdio::null())
            .stderr(iroha_fs::ProcessStdio::null());
    });
    let mut child = command.spawn().unwrap();
    fs::set_permissions(bundle.path(), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(child.wait().is_err());
    assert!(child.try_wait().is_err());
}

#[cfg(all(
    feature = "tokio",
    any(windows, target_os = "linux", target_os = "android")
))]
#[test]
fn asynchronous_output_and_explicit_termination_retain_the_original_bundle() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let fixture = Fixture::new();
    fixture.use_native_probe();
    let bundle = publish(&fixture.owner, "invocation.async", fixture.inputs()).unwrap();
    runtime.block_on(async {
        let mut command = bundle.program(BinaryKind::Iroha).command().unwrap();
        command.configure(|command| {
            command
                .arg("flood")
                .stdout(iroha_fs::ProcessStdio::piped())
                .stderr(iroha_fs::ProcessStdio::piped());
        });
        let child = command.spawn_async().unwrap();
        assert!(child.id().is_some());
        let output = child.wait_with_output().await.unwrap();
        assert_eq!(output.status.code(), Some(19));
        assert_eq!(output.stdout, vec![b'o'; 131_072]);
        assert_eq!(output.stderr, vec![b'e'; 131_072]);
        let mut command = bundle.program(BinaryKind::Irohad).command().unwrap();
        command.configure(|command| {
            command
                .arg("input")
                .stdin(iroha_fs::ProcessStdio::piped())
                .stdout(iroha_fs::ProcessStdio::null());
        });
        let mut child = command.spawn_async().unwrap();
        let input = child.stdin.take().unwrap();
        assert!(child.try_wait().unwrap().is_none());
        child.start_kill().unwrap();
        assert!(!child.wait().await.unwrap().success());
        assert!(child.id().is_none());
        assert!(child.try_wait().unwrap().is_some());
        drop(input);
        let mut command = bundle.program(BinaryKind::IrohadTaira).command().unwrap();
        command.configure(|command| {
            command
                .arg("input")
                .stdin(iroha_fs::ProcessStdio::piped())
                .stdout(iroha_fs::ProcessStdio::null());
        });
        let mut child = command.spawn_async().unwrap();
        let input = child.stdin.take().unwrap();
        child.kill().await.unwrap();
        assert!(child.try_wait().unwrap().is_some());
        drop(input);
    });
    bundle.revalidate().unwrap();
}

#[cfg(all(
    feature = "tokio",
    any(windows, target_os = "linux", target_os = "android")
))]
#[test]
fn cancelled_wait_and_destroyed_runtime_keep_custody_until_actual_native_exit() {
    use tokio::io::AsyncBufReadExt as _;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let fixture = Fixture::new();
    fixture.use_native_probe();
    let bundle = publish(&fixture.owner, "invocation.cancelled", fixture.inputs()).unwrap();
    let weak = Arc::downgrade(&bundle);
    let (child, input) = runtime.block_on(async {
        let mut command = bundle.program(BinaryKind::Irohad).command().unwrap();
        command.configure(|command| {
            command
                .arg("input")
                .stdin(iroha_fs::ProcessStdio::piped())
                .stdout(iroha_fs::ProcessStdio::piped());
        });
        let mut child = command.spawn_async().unwrap();
        let input = child.stdin.take().unwrap();
        let mut output = tokio::io::BufReader::new(child.stdout.take().unwrap());
        let mut ready = String::new();
        output.read_line(&mut ready).await.unwrap();
        assert_eq!(ready.trim_end(), "ready");
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(30), child.wait())
                .await
                .is_err()
        );
        assert!(child.try_wait().unwrap().is_none());
        (child, input)
    });
    drop(runtime);
    drop(bundle);
    drop(child);
    assert!(
        weak.upgrade().is_some(),
        "runtime destruction cannot release a live original"
    );
    drop(input);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while weak.upgrade().is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        weak.upgrade().is_none(),
        "native reaper must work after Tokio runtime destruction"
    );
}

#[cfg(all(
    feature = "tokio",
    any(windows, target_os = "linux", target_os = "android")
))]
#[test]
fn asynchronous_drop_termination_reaps_without_a_runtime() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let fixture = Fixture::new();
    fixture.use_native_probe();
    let bundle = publish(&fixture.owner, "invocation.async-drop", fixture.inputs()).unwrap();
    let weak = Arc::downgrade(&bundle);
    let (child, input) = runtime.block_on(async {
        let mut command = bundle.program(BinaryKind::Irohad).command().unwrap();
        command.kill_on_drop(true).configure(|command| {
            command
                .arg("input")
                .stdin(iroha_fs::ProcessStdio::piped())
                .stdout(iroha_fs::ProcessStdio::null());
        });
        let mut child = command.spawn_async().unwrap();
        let input = child.stdin.take().unwrap();
        (child, input)
    });
    drop(runtime);
    drop(bundle);
    drop(child);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while weak.upgrade().is_some() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(weak.upgrade().is_none());
    drop(input);
}
