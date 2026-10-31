//! Actual process and cancellation controls for both resolver ownership branches.

use super::*;
use std::{
    fs,
    io::{BufRead as _, Write as _},
    sync::{Arc, OnceLock, Weak},
    time::{Duration, Instant},
};

use iroha_fs::{OwnerDirectory, PublishMode};
use iroha_release_bundle::{
    bundle::{Bundle, NATIVE_TARGET},
    manifest::BinaryKind,
    publish::{PublishInputs, publish},
};

pub struct Fixture {
    root: tempfile::TempDir,
    build: PathBuf,
    owner: OwnerDirectory,
    lock: PathBuf,
    cargo: PathBuf,
    rustc: PathBuf,
}

fn probe_bytes() -> &'static [u8] {
    static PROBE: OnceLock<Vec<u8>> = OnceLock::new();
    PROBE.get_or_init(|| {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("resolved_probe.rs");
        fs::write(
            &source,
            br#"//! Genuine native resolver child fixture.
use std::io::{Read as _, Write as _};
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("input") => {
            println!("ready"); std::io::stdout().flush().unwrap();
            let mut bytes=Vec::new(); std::io::stdin().read_to_end(&mut bytes).unwrap();
            print!("{}", bytes.len());
        }
        Some("sleep") => std::thread::sleep(std::time::Duration::from_secs(5)),
        Some("pipe-descendant") => {
            let _child = std::process::Command::new(std::env::current_exe().unwrap())
                .arg("sleep").stdin(std::process::Stdio::null()).spawn().unwrap();
        }
        #[cfg(unix)]
        Some("fd") => {
            use std::os::fd::FromRawFd as _;
            let descriptor: i32 = std::env::args().nth(2).unwrap().parse().unwrap();
            let mut input = unsafe { std::fs::File::from_raw_fd(descriptor) };
            let mut bytes = Vec::new(); input.read_to_end(&mut bytes).unwrap();
            std::io::stdout().write_all(&bytes).unwrap();
        }
        Some("flood") => {
            std::io::stdout().write_all(&vec![b'o';131_072]).unwrap();
            std::io::stderr().write_all(&vec![b'e';131_072]).unwrap();
        }
        _ => { print!("resolver-original"); eprint!("resolver-stderr"); }
    }
    std::process::exit(19);
}

"#,
        )
        .unwrap();
        let image = root
            .path()
            .join(if cfg!(windows) { "probe.exe" } else { "probe" });
        let output =
            process::Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
                .args(["--edition=2024", "-C", "debuginfo=0"])
                .arg(&source)
                .arg("-o")
                .arg(&image)
                .output()
                .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::read(image).unwrap()
    })
}

impl Fixture {
    pub(crate) fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let owner = OwnerDirectory::open_or_create(root.path().join("bundles")).unwrap();
        let build = root.path().join("build");
        fs::create_dir(&build).unwrap();
        for kind in BinaryKind::ALL {
            let path = build.join(kind.filename(cfg!(windows)));
            fs::write(&path, probe_bytes()).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            }
        }
        let lock = root.path().join("Cargo.lock");
        fs::write(&lock, b"version = 4\n").unwrap();
        let controls = OwnerDirectory::open_or_create(root.path().join("controls")).unwrap();
        controls
            .write_atomic("cargo", b"cargo 1.93.1 (test)\n", PublishMode::CreateNew)
            .unwrap();
        controls
            .write_atomic(
                "rustc",
                format!("rustc 1.93.1 (test)\nhost: {NATIVE_TARGET}\n").as_bytes(),
                PublishMode::CreateNew,
            )
            .unwrap();
        Self {
            root,
            build,
            owner,
            lock,
            cargo: controls.path().join("cargo"),
            rustc: controls.path().join("rustc"),
        }
    }

    pub(crate) fn select(&self, retained: bool) -> (ResolvedProgram, Option<Weak<Bundle>>) {
        if !retained {
            return (
                ResolvedProgram::development(
                    self.build.join(BinaryKind::Irohad.filename(cfg!(windows))),
                ),
                None,
            );
        }
        let bundle = publish(
            &self.owner,
            "invocation.control",
            PublishInputs {
                source_sha256: &"a".repeat(64),
                build_directory: &self.build,
                cargo_lock: &self.lock,
                cargo_version: &self.cargo,
                rustc_version: &self.rustc,
            },
        )
        .unwrap();
        let weak = Arc::downgrade(&bundle);
        let program = ResolvedProgram::retained(bundle.program(BinaryKind::Irohad));
        drop(bundle);
        (program, Some(weak))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Test cleanup occurs after every selected program and process owner has dropped.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            if let Ok(entries) = fs::read_dir(self.owner.path()) {
                for entry in entries.flatten() {
                    let _ = fs::set_permissions(entry.path(), fs::Permissions::from_mode(0o700));
                }
            }
        }
        #[cfg(windows)]
        {
            if let Ok(identity) = process::Command::new("whoami")
                .args(["/user", "/fo", "csv", "/nh"])
                .output()
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
                        .all(|b| b.is_ascii_digit() || matches!(b, b'S' | b'-'))
                {
                    let _ = process::Command::new("icacls")
                        .arg(self.owner.path())
                        .arg("/grant:r")
                        .arg(format!("*{sid}:(OI)(CI)F"))
                        .args(["/T", "/Q"])
                        .output();
                }
            }
        }
        assert!(self.owner.path().starts_with(self.root.path()));
    }
}

fn await_release(bundle: &Weak<Bundle>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while bundle.upgrade().is_some() {
        assert!(
            Instant::now() < deadline,
            "terminal reaper retained the bundle beyond its bound"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn synchronous_output_and_cached_selection_preserve_both_modes() {
    for retained in [false, true] {
        let fixture = Fixture::new();
        let (program, weak) = fixture.select(retained);
        assert_eq!(program.is_release(), retained);
        program.revalidate().unwrap();
        let cached = program.clone();
        let path = program.diagnostic_path().unwrap();
        drop(program);
        assert_eq!(cached.diagnostic_path().unwrap(), path);
        let mut command = cached.command().unwrap();
        command.configure(|options| {
            options
                .env("RESOLVER_TEST", "configured")
                .current_dir(fixture.root.path());
        });
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(19));
        assert_eq!(output.stdout, b"resolver-original");
        assert_eq!(output.stderr, b"resolver-stderr");
        let mut flood = cached.command().unwrap();
        flood.arg("flood");
        let output = flood.output().unwrap();
        assert_eq!(output.stdout, vec![b'o'; 131_072]);
        assert_eq!(output.stderr, vec![b'e'; 131_072]);
        drop(cached);
        if let Some(weak) = weak {
            await_release(&weak);
        }
    }
}

#[test]
fn synchronous_taken_stdin_keeps_explicit_eof_and_child_owned_bundle() {
    for retained in [false, true] {
        let fixture = Fixture::new();
        let (program, weak) = fixture.select(retained);
        let mut command = program.command().unwrap();
        command
            .arg("input")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn().unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let mut output = std::io::BufReader::new(child.stdout.take().unwrap());
        let mut ready = String::new();
        output.read_line(&mut ready).unwrap();
        assert_eq!(ready, "ready\n");
        stdin.write_all(b"retained input").unwrap();
        assert!(child.try_wait().unwrap().is_none());
        assert!(child.id() > 0);
        drop(program);
        if let Some(weak) = &weak {
            assert!(weak.upgrade().is_some());
        }
        drop(stdin);
        assert_eq!(child.wait().unwrap().code(), Some(19));
        drop(child);
        if let Some(weak) = weak {
            await_release(&weak);
        }
    }
}

#[test]
fn asynchronous_output_and_original_termination_support_both_modes() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for retained in [false, true] {
        let fixture = Fixture::new();
        let (program, weak) = fixture.select(retained);
        runtime.block_on(async {
            let mut command = program.command().unwrap();
            command.arg("flood");
            let output = command.output_async().await.unwrap();
            assert_eq!(output.status.code(), Some(19));
            assert_eq!(output.stdout, vec![b'o'; 131_072]);
            assert_eq!(output.stderr, vec![b'e'; 131_072]);
            let mut command = program.command().unwrap();
            command
                .arg("input")
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .kill_on_drop(true);
            let mut child = command.spawn_async().unwrap();
            assert!(child.id().is_some());
            child.kill().await.unwrap();
            assert!(child.try_wait().unwrap().is_some());
        });
        drop(program);
        if let Some(weak) = weak {
            await_release(&weak);
        }
    }
}

#[test]
fn cancelled_wait_and_destroyed_runtime_keep_qualified_original_until_eof() {
    use tokio::io::AsyncBufReadExt as _;
    let fixture = Fixture::new();
    let (program, weak) = fixture.select(true);
    let weak = weak.unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let (child, input, output) = runtime.block_on(async {
        let mut command = program.command().unwrap();
        command
            .arg("input")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command.spawn_async().unwrap();
        let input = child.stdin.take().unwrap();
        let mut output = tokio::io::BufReader::new(child.stdout.take().unwrap());
        let mut ready = String::new();
        output.read_line(&mut ready).await.unwrap();
        assert_eq!(ready, "ready\n");
        assert!(
            tokio::time::timeout(Duration::from_millis(20), child.wait())
                .await
                .is_err()
        );
        assert!(child.try_wait().unwrap().is_none());
        (child, input, output)
    });
    drop(program);
    drop(runtime);
    drop(child);
    assert!(
        weak.upgrade().is_some(),
        "live child lost qualified bundle after runtime shutdown"
    );
    drop(input);
    await_release(&weak);
    drop(output);
}

#[test]
fn dropping_synchronous_supervisor_closes_owned_stdin_before_release_reaping() {
    let fixture = Fixture::new();
    let (program, weak) = fixture.select(true);
    let weak = weak.unwrap();
    let mut command = program.command().unwrap();
    command
        .arg("input")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = command.spawn().unwrap();
    drop(program);
    drop(child);
    await_release(&weak);
}

#[test]
fn native_development_child_adoption_keeps_original_pipes_and_terminal_identity() {
    let fixture = Fixture::new();
    let executable = fixture
        .build
        .join(BinaryKind::Irohad.filename(cfg!(windows)));
    let mut command = ProcessCommand::new(&executable).unwrap();
    command
        .arg("flood")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let original = command.spawn().unwrap();
    let identity = original.id();
    let child = ResolvedChild::from_development_child(original);
    assert_eq!(child.id(), identity);
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(19));
    assert_eq!(output.stdout, vec![b'o'; 131_072]);
    assert_eq!(output.stderr, vec![b'e'; 131_072]);
}

#[test]
fn native_async_child_adoption_keeps_original_pipes_and_terminal_identity() {
    let fixture = Fixture::new();
    let executable = fixture
        .build
        .join(BinaryKind::Irohad.filename(cfg!(windows)));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let mut command = ProcessCommand::new(&executable).unwrap();
        command
            .arg("flood")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let original = command.spawn().unwrap();
        let identity = Some(original.id());
        let child = ResolvedAsyncChild::from_development_child(original).unwrap();
        assert_eq!(child.id(), identity);
        let output = child.wait_with_output().await.unwrap();
        assert_eq!(output.status.code(), Some(19));
        assert_eq!(output.stdout, vec![b'o'; 131_072]);
        assert_eq!(output.stderr, vec![b'e'; 131_072]);
    });
}

#[test]
fn explicit_development_command_adoption_preserves_settings_and_actual_output() {
    let fixture = Fixture::new();
    let executable = fixture
        .build
        .join(BinaryKind::Irohad.filename(cfg!(windows)));
    let mut command = ProcessCommand::new(executable).unwrap();
    command.arg("flood").stdin(Stdio::null()).kill_on_drop(true);
    let command = ResolvedCommand::from_development_command(command);
    assert_eq!(command.get_args().collect::<Vec<_>>(), ["flood"]);
    let output = command.output().unwrap();
    assert_eq!(output.status.code(), Some(19));
    assert_eq!(output.stdout, vec![b'o'; 131_072]);
    assert_eq!(output.stderr, vec![b'e'; 131_072]);
}

#[test]
fn io_only_runtime_wait_preserves_both_original_process_branches() {
    use tokio::io::{AsyncBufReadExt as _, BufReader};
    let mut builder = tokio::runtime::Builder::new_current_thread();
    // Unix pipe registration needs the I/O driver. Native Windows process pipes
    // use Tokio's blocking adapter and need neither a timer nor a network driver.
    #[cfg(unix)]
    builder.enable_io();
    let runtime = builder.build().unwrap();
    for release in [false, true] {
        let fixture = Fixture::new();
        let (program, weak) = fixture.select(release);
        runtime.block_on(async {
            let mut command = program.command().unwrap();
            command
                .arg("input")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            let mut child = command.spawn_async().unwrap();
            drop(program);
            let mut output = BufReader::new(child.stdout.take().unwrap());
            let mut line = String::new();
            output.read_line(&mut line).await.unwrap();
            assert_eq!(line, "ready\n");
            assert!(child.try_wait().unwrap().is_none());
            assert_eq!(child.wait().await.unwrap().code(), Some(19));
            assert!(child.id().is_none());
            if let Some(weak) = &weak {
                assert!(weak.upgrade().is_some());
            }
            drop(child);
            if let Some(weak) = &weak {
                assert!(weak.upgrade().is_none());
            }
        });
    }
}

#[cfg(unix)]
#[test]
fn descriptor_facade_retains_original_inputs_for_both_resolved_branches() {
    use std::os::fd::AsFd as _;
    const CHILD: &str = "IROHA_RESOLVED_DESCRIPTOR_FIXTURE_CHILD";
    if std::env::var_os(CHILD).is_none() {
        // Other concurrently running custody fixtures deliberately retain many
        // descriptors. Isolate this fixed-slot positive control; occupied slots
        // must still be refused by the production owner, never overwritten.
        let module = module_path!().split_once("::").unwrap().1;
        let output = process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                &format!(
                    "{module}::descriptor_facade_retains_original_inputs_for_both_resolved_branches"
                ),
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"),
            "the exact descriptor control must execute one test"
        );
        return;
    }
    for retained in [false, true] {
        let fixture = Fixture::new();
        let (program, _) = fixture.select(retained);
        let source = fixture.root.path().join("descriptor-input");
        fs::write(&source, b"custody\0descriptor").unwrap();
        let input = fs::File::open(source).unwrap();
        let mut command = program.command().unwrap();
        command.args(["fd", "198"]);
        command
            .inherit_descriptors(&[(input.as_fd(), 198)])
            .unwrap();
        drop(input);
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(19));
        assert_eq!(output.stdout, b"custody\0descriptor");
    }
}
