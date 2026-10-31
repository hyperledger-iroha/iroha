//! One native owner for the source-bound cache, fixed builds, transcripts, and publication.

use std::{
    ffi::OsString,
    io::{self, Read, Write},
    path::PathBuf,
    process::ExitStatus,
};

use iroha_fs::{
    FileSnapshot, OwnerDirectory, PrivateDirectory, ProcessCommand as Command,
    ProcessStdio as Stdio, RetainedFile,
};
use iroha_release_bundle::{
    bundle::NATIVE_TARGET,
    manifest::BinaryKind,
    publish::{PublishInputs, publish},
};

use super::{
    Arguments, Failure, acknowledge, admit, digest_argument, exact_path, invalid, invocation_name,
    lock_digest,
    policy::{Cancellation, Configuration, DisjointRoots, Lock, Tool},
    private_owner,
};

const TOOL_LIMIT: usize = 64 * 1024;

pub(super) struct Inputs {
    pub(super) source: String,
    pub(super) repo: PathBuf,
    pub(super) cargo_root: PathBuf,
    pub(super) artifact_root: PathBuf,
    pub(super) cargo_home: PathBuf,
    pub(super) cargo_bin: PathBuf,
    pub(super) rustc_bin: PathBuf,
    pub(super) cancellation: PathBuf,
    pub(super) name: OsString,
    pub(super) inherited: Option<(PathBuf, String)>,
}

impl Inputs {
    pub(super) fn parse(mut arguments: Arguments) -> io::Result<Self> {
        let repo = arguments.path("--repo-root")?;
        let source = arguments.digest("--source-sha256")?;
        let cargo_root = arguments.path("--cargo-target-dir")?;
        let artifact_root = arguments.path("--artifact-root")?;
        let cargo_home = arguments.path("--cargo-home")?;
        let cargo_bin = arguments.path("--cargo-bin")?;
        let rustc_bin = arguments.path("--rustc-bin")?;
        let cancellation = arguments.path("--cancel-request")?;
        let name = invocation_name(arguments.take("--name")?)?;
        let inherited = match (
            arguments.0.remove("--bundle-dir"),
            arguments.0.remove("--manifest-sha256"),
        ) {
            (None, None) => None,
            (Some(path), Some(digest)) => Some((exact_path(path)?, digest_argument(digest)?)),
            _ => {
                return Err(invalid(
                    "inherited bundle path and manifest anchor must both be supplied",
                ));
            }
        };
        arguments.finish()?;
        super::policy::disjoint(&[&repo, &cargo_root, &artifact_root, &cancellation])?;
        super::policy::disjoint(&[&cargo_home, &cargo_root, &artifact_root])?;
        super::policy::disjoint(&[
            &repo,
            cancellation
                .parent()
                .ok_or_else(|| invalid("cancellation path has no parent"))?,
        ])?;
        Ok(Self {
            source,
            repo,
            cargo_root,
            artifact_root,
            cargo_home,
            cargo_bin,
            rustc_bin,
            cancellation,
            name,
            inherited,
        })
    }

    fn programs(&self) -> PathBuf {
        self.artifact_root
            .join("sumeragi-release")
            .join(&self.source)
            .join("programs")
    }

    pub(super) fn retain_roots(&self) -> io::Result<[DisjointRoots; 3]> {
        let mut source_outputs =
            DisjointRoots::open(&[&self.repo, &self.cargo_root, &self.artifact_root])?;
        source_outputs.exclude_marker(&self.cancellation)?;
        let cache_outputs =
            DisjointRoots::open(&[&self.cargo_home, &self.cargo_root, &self.artifact_root])?;
        let source_cancellation = DisjointRoots::open(&[
            &self.repo,
            self.cancellation
                .parent()
                .ok_or_else(|| invalid("cancellation path has no parent"))?,
        ])?;
        Ok([source_outputs, cache_outputs, source_cancellation])
    }
}

struct Session {
    environment: super::environment::BuildEnvironment,
    cache: PrivateDirectory,
    programs: PrivateDirectory,
    cargo_root: PrivateDirectory,
    artifact_root: PrivateDirectory,
    build_lock: Lock,
    artifact_lock: Lock,
    configuration: Configuration,
    cancellation: Cancellation,
    cargo: Tool,
    rustc: Tool,
    lock: RetainedFile,
    lock_snapshot: FileSnapshot,
    roots: [DisjointRoots; 3],
}

impl Session {
    fn open(
        inputs: &Inputs,
        environment: super::environment::BuildEnvironment,
    ) -> Result<Self, Failure> {
        let roots = inputs.retain_roots()?;
        let cargo_root = PrivateDirectory::open_exact(&inputs.cargo_root)?;
        let artifact_root = PrivateDirectory::open_exact(&inputs.artifact_root)?;
        let cancellation = Cancellation::open(&inputs.cancellation)?;
        cancellation.check()?;
        let configuration = Configuration::open(&inputs.repo, &inputs.cargo_home)?;
        let cargo = Tool::open(&inputs.cargo_bin)?;
        let rustc = Tool::open(&inputs.rustc_bin)?;
        let lock = RetainedFile::open_regular(inputs.repo.join("Cargo.lock"))?;
        let lock_snapshot = lock.snapshot()?;
        // Persistent ordinary file locks survive process crashes and are never unlinked.
        // A conflicting lock is an error; no lock owner is interrupted or inspected.
        let artifact_lock = Lock::acquire(artifact_root.path(), ".sumeragi-cargo.lock")?;
        let build_root = cargo_root
            .ensure_child("sumeragi-release")?
            .ensure_child(&inputs.source)?
            .ensure_child("program-build-cache")?;
        let build_lock = Lock::acquire(build_root.path(), ".sumeragi-prebuild.lock")?;
        let cache = build_root.ensure_child("default")?;
        let programs = artifact_root
            .ensure_child("sumeragi-release")?
            .ensure_child(&inputs.source)?
            .ensure_child("programs")?;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let cargo = cargo.publish_cargo_image(
            &cache,
            &format!("cargo-image.{}", inputs.name.to_string_lossy()),
        )?;
        let result = Self {
            environment,
            cache,
            programs,
            cargo_root,
            artifact_root,
            build_lock,
            artifact_lock,
            configuration,
            cancellation,
            cargo,
            rustc,
            lock,
            lock_snapshot,
            roots,
        };
        result.boundary()?;
        Ok(result)
    }

    fn boundary(&self) -> Result<(), Failure> {
        for roots in &self.roots {
            roots.revalidate()?;
        }
        self.cache.revalidate()?;
        self.programs.revalidate()?;
        self.cargo_root.revalidate()?;
        self.artifact_root.revalidate()?;
        self.build_lock.revalidate()?;
        self.artifact_lock.revalidate()?;
        self.configuration.revalidate()?;
        self.cargo.revalidate()?;
        self.rustc.revalidate()?;
        self.lock.revalidate()?;
        if self.lock.snapshot()? != self.lock_snapshot {
            return Err(invalid("Cargo.lock changed during the build session").into());
        }
        self.cancellation.check()
    }

    fn configure(&self, command: &mut Command, inputs: &Inputs) -> io::Result<()> {
        self.environment.apply(command);
        self.configuration.bind_current_directory(command)?;
        command
            .env("CARGO_TARGET_DIR", self.cache.path())
            .env("CARGO_HOME", &inputs.cargo_home)
            .env("RUSTUP_AUTO_INSTALL", "0")
            .env("ENABLE_RANS_BUNDLES", "1")
            .env("NORITO_SKIP_BINDINGS_SYNC", "1")
            .stdin(Stdio::null())
            .stderr(Stdio::inherit());
        // On Linux/Android Cargo selects an independently sealed image whose
        // non-Cargo basename preserves the inherited original descriptor through
        // Cargo's descendant environment construction. Both originals remain held.
        self.cargo.bind(command, "CARGO")?;
        self.rustc.bind(command, "RUSTC")?;
        Ok(())
    }

    fn command(
        &self,
        mut command: Command,
        inputs: &Inputs,
        capture: bool,
    ) -> Result<Vec<u8>, Failure> {
        self.boundary()?;
        self.configure(&mut command, inputs)?;
        command.stdout(Stdio::piped());
        let result = drain_child(command, capture);
        // Native observation failures take precedence over accepting child success/failure.
        self.boundary()?;
        result
    }
}

fn build_arguments(package: &str, binaries: &[&str]) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = [
        "build",
        "-j1",
        "--locked",
        "--offline",
        "--release",
        "--target",
        NATIVE_TARGET,
        "-p",
        package,
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    for binary in binaries {
        arguments.extend([OsString::from("--bin"), OsString::from(binary)]);
    }
    arguments
}

fn drain_child(command: Command, capture: bool) -> Result<Vec<u8>, Failure> {
    let mut child = command.spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| invalid("missing owned child stdout pipe"))?;
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut output_error = None;
    loop {
        match stdout.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                if capture {
                    if bytes
                        .len()
                        .checked_add(count)
                        .is_some_and(|length| length <= TOOL_LIMIT)
                        && output_error.is_none()
                    {
                        bytes.extend_from_slice(&buffer[..count]);
                    } else if output_error.is_none() {
                        output_error = Some(invalid("tool stdout exceeds its ceiling"));
                    }
                } else if output_error.is_none()
                    && let Err(error) = io::stderr().lock().write_all(&buffer[..count])
                {
                    output_error = Some(error);
                }
                // Keep draining after an output error so the owned child cannot deadlock
                // on its full pipe. Never terminate or reinterpret an active command.
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                output_error = Some(error);
                break;
            }
        }
    }
    drop(stdout);
    let (status, wait_error) = wait_terminal(|| child.wait());
    if let Some(error) = wait_error {
        return Err(error.into());
    }
    if let Some(error) = output_error {
        return Err(error.into());
    }
    if !status.success() {
        return Err(Failure {
            error: io::Error::other("release tool command failed; publication was not accepted"),
            code: status
                .code()
                .and_then(|code| u8::try_from(code).ok())
                .filter(|code| *code != 0)
                .unwrap_or(1),
        });
    }
    Ok(bytes)
}

pub(super) fn wait_terminal(
    mut wait: impl FnMut() -> io::Result<ExitStatus>,
) -> (ExitStatus, Option<io::Error>) {
    let mut first_error = None;
    loop {
        match wait() {
            Ok(status) => return (status, first_error),
            Err(error) => {
                if first_error.is_none() {
                    eprintln!(
                        "release bundle: child terminal observation failed; retaining all originals and awaiting the owned child"
                    );
                    first_error = Some(error);
                }
                // A failed wait cannot authorize either releasing live build custody or
                // killing a Cargo process. Keep this synchronous session alive until an
                // actual terminal observation; the original failure still rejects it.
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
}

struct Transcript {
    original: RetainedFile,
    snapshot: FileSnapshot,
}

impl Transcript {
    fn revalidate(&self) -> io::Result<()> {
        self.original.revalidate()?;
        if self.original.snapshot()? != self.snapshot {
            return Err(invalid("original sealed transcript changed"));
        }
        Ok(())
    }
}

fn transcript(parent: &PrivateDirectory, name: &str, bytes: &[u8]) -> io::Result<Transcript> {
    let mut writer = parent.create_retained_private(name, TOOL_LIMIT)?;
    writer.write_all(bytes)?;
    let creator = writer.seal_read_only()?;
    let snapshot = creator.snapshot()?;
    // The Windows creator still owns WRITE/DELETE, so a restrictive reader cannot
    // coexist with it. Retain a separate read-only bridge to the exact same sealed
    // original before closing the creator; then install the restrictive reader while
    // that bridge remains alive. No permissions are widened and no handle-free gap
    // authorizes a replacement name.
    let bridge = parent.open_retained_read_only(name, TOOL_LIMIT)?;
    if bridge.snapshot()? != snapshot {
        return Err(invalid("sealed transcript bridge differs from its creator"));
    }
    creator.revalidate()?;
    drop(creator);
    let mut original = RetainedFile::open_private(parent.path().join(name))?;
    bridge.revalidate()?;
    let mut observed = Vec::new();
    original
        .file_mut()
        .take(TOOL_LIMIT as u64 + 1)
        .read_to_end(&mut observed)?;
    if observed != bytes {
        return Err(invalid(
            "sealed transcript differs from successful tool stdout",
        ));
    }
    let transcript = Transcript { original, snapshot };
    transcript.revalidate()?;
    Ok(transcript)
}

pub(super) fn run(
    arguments: Arguments,
    output: &mut impl Write,
    environment: fn() -> io::Result<super::environment::BuildEnvironment>,
) -> Result<(), Failure> {
    let inputs = Inputs::parse(arguments)?;
    if let Some((path, manifest)) = &inputs.inherited {
        // Admission is deliberately before every creator, transcript, lock and build.
        // A valid-looking inherited digest never grants permission to replace a bundle.
        let expected_parent = inputs.programs();
        if path.parent() != Some(expected_parent.as_path()) {
            return Err(invalid(
                "inherited bundle is outside the exact source-bound programs root",
            )
            .into());
        }
        let name = invocation_name(
            path.file_name()
                .ok_or_else(|| invalid("bundle has no name"))?
                .to_owned(),
        )?;
        let roots = inputs.retain_roots()?;
        let _cargo = PrivateDirectory::open_exact(&inputs.cargo_root)?;
        let _artifact = PrivateDirectory::open_exact(&inputs.artifact_root)?;
        let cancellation = Cancellation::open(&inputs.cancellation)?;
        let (lock_file, lock_snapshot, lock) = lock_digest(&inputs.repo.join("Cargo.lock"))?;
        let (bundle, private) = admit(&expected_parent, &name, &inputs.source, &lock, manifest)?;
        lock_file.revalidate()?;
        if lock_file.snapshot()? != lock_snapshot {
            return Err(invalid("Cargo.lock changed during inherited admission").into());
        }
        acknowledge(&bundle, output)?;
        lock_file.revalidate()?;
        if lock_file.snapshot()? != lock_snapshot {
            return Err(invalid("Cargo.lock changed during inherited acknowledgement").into());
        }
        private.revalidate()?;
        cancellation.revalidate()?;
        for roots in &roots {
            roots.revalidate()?;
        }
        return Ok(());
    }
    let session = Session::open(&inputs, environment()?)?;
    // Refuse tools whose direct argv0 works but descriptor-spelled nested argv0
    // selects another proxy role, before any output removal or release build.
    for (tool, argument) in [(&session.cargo, "--version"), (&session.rustc, "-vV")] {
        let mut direct = tool.source_command()?;
        direct.arg(argument);
        let expected = session.command(direct, &inputs, true)?;
        tool.require_nested_transcript(argument, &expected, |probe| {
            session.command(probe, &inputs, true)
        })?;
    }
    let target = OwnerDirectory::open(session.cache.path())?.ensure_child(NATIVE_TARGET)?;
    let release = target.ensure_child("release")?;
    for kind in BinaryKind::ALL {
        // The canonical lower owner supports Cargo's exact initially shared build
        // identity. It removes only this selected name and preserves other aliases.
        release.remove_build_output(kind.filename(cfg!(windows)))?;
    }
    session.boundary()?;
    for (package, binaries) in [
        ("irohad", &["iroha3d", "iroha3d_taira"][..]),
        ("iroha_cli", &["iroha"][..]),
        ("iroha_kagami", &["kagami"][..]),
    ] {
        let mut command = session.cargo.command()?;
        command.args(build_arguments(package, binaries));
        session.command(command, &inputs, false)?;
    }
    let mut cargo_command = session.cargo.command()?;
    cargo_command.arg("--version");
    let cargo_bytes = session.command(cargo_command, &inputs, true)?;
    let mut rustc_command = session.rustc.command()?;
    rustc_command.arg("-vV");
    let rustc_bytes = session.command(rustc_command, &inputs, true)?;
    let controls = session
        .cache
        .create_child(format!("tools.{}", inputs.name.to_string_lossy()))?;
    let cargo_transcript = transcript(&controls, "cargo-version.txt", &cargo_bytes)?;
    let rustc_transcript = transcript(&controls, "rustc-version.txt", &rustc_bytes)?;
    session.boundary()?;
    let (_private, programs) = private_owner(session.programs.path())?;
    let bundle = publish(
        &programs,
        &inputs.name,
        PublishInputs {
            source_sha256: &inputs.source,
            build_directory: release.path(),
            cargo_lock: &inputs.repo.join("Cargo.lock"),
            cargo_version: &controls.path().join("cargo-version.txt"),
            rustc_version: &controls.path().join("rustc-version.txt"),
        },
    )?;
    cargo_transcript.revalidate()?;
    rustc_transcript.revalidate()?;
    session.boundary()?;
    acknowledge(&bundle, output)?;
    cargo_transcript.revalidate()?;
    rustc_transcript.revalidate()?;
    session.boundary()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_fixture(
        values: impl Iterator<Item = OsString>,
        output: &mut impl Write,
    ) -> Result<(), Failure> {
        super::super::run_with_environment(
            values,
            output,
            super::super::environment::BuildEnvironment::for_fixture,
        )
    }

    #[test]
    fn fixed_build_plan_has_exact_native_target_and_locked_offline_jobs() {
        let arguments = build_arguments("irohad", &["iroha3d", "iroha3d_taira"]);
        assert_eq!(
            arguments,
            [
                "build",
                "-j1",
                "--locked",
                "--offline",
                "--release",
                "--target",
                NATIVE_TARGET,
                "-p",
                "irohad",
                "--bin",
                "iroha3d",
                "--bin",
                "iroha3d_taira"
            ]
            .map(OsString::from)
        );
    }

    #[test]
    fn sealed_transcript_preserves_authored_bytes_and_rejects_existing_name() {
        use std::io::{Seek as _, SeekFrom};
        let temporary = tempfile::tempdir().unwrap();
        let directory =
            PrivateDirectory::open_or_create(temporary.path().join("controls")).unwrap();
        let mut sealed = transcript(&directory, "cargo", b"cargo fixture\n").unwrap();
        sealed.original.file_mut().seek(SeekFrom::Start(0)).unwrap();
        let mut bytes = Vec::new();
        sealed.original.file_mut().read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"cargo fixture\n");
        assert!(transcript(&directory, "cargo", b"replacement\n").is_err());
        sealed.revalidate().unwrap();
        let readback = RetainedFile::open_private(directory.path().join("cargo")).unwrap();
        assert_eq!(readback.snapshot().unwrap(), sealed.snapshot);
    }

    struct BuildFixture {
        root: tempfile::TempDir,
        source: String,
    }

    impl BuildFixture {
        fn new() -> Self {
            use std::sync::OnceLock;
            static TOOL: OnceLock<Vec<u8>> = OnceLock::new();
            let bytes = TOOL.get_or_init(|| {
                let temporary = tempfile::tempdir().unwrap();
                let source = temporary.path().join("tool.rs");
                let executable = temporary.path().join(if cfg!(windows) { "tool.exe" } else { "tool" });
                let text = r#"//! Genuine native process fixture for publisher orchestration only.
use std::{env, fs, io::Write as _, path::Path};
fn main() {
    let args: Vec<_> = env::args().skip(1).collect();
    let home = env::var_os("CARGO_HOME").unwrap();
    if Path::new(&home).join("proxy-sensitive").exists()
        && Path::new(&env::args_os().next().unwrap()).file_name().unwrap()
            .to_string_lossy().bytes().all(|byte| byte.is_ascii_digit()) {
        println!("different proxy role selected by descriptor argv0"); return;
    }
    if args == ["--owned-descendant"] {
        println!("descendant-ready");
        std::io::stdout().flush().unwrap();
        std::io::copy(&mut std::io::stdin().lock(), &mut std::io::sink()).unwrap();
        println!("descendant-observed-eof");
        return;
    }
    if args == ["--exit-with-owned-descendant"] {
        let _descendant = std::process::Command::new(env::current_exe().unwrap())
            .arg("--owned-descendant")
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .spawn().unwrap();
        // The fixture deliberately exits before its inherited-Job child. The
        // supervisor owns the original root/Job, never a rediscovered PID.
        std::process::exit(0);
    }
    if args == ["--nested-tools"] {
        println!("original-cargo-selected");
        for (key, argument) in [("CARGO", "--version"), ("RUSTC", "-vV")] {
            let output = std::process::Command::new(env::var_os(key).unwrap())
                .arg(argument).output().unwrap();
            assert!(output.status.success(), "{output:?}");
            std::io::stdout().write_all(&output.stdout).unwrap();
        }
        return;
    }
    if args == ["--version"] {
        println!("cargo native-fixture");
        if Path::new(&home).join("fail-probe-version").exists()
            || (Path::new(&home).join("fail-version").exists() && Path::new(&home).join("calls").exists()) {
            std::process::exit(23);
        }
        return;
    }
    if args == ["-vV"] { println!("rustc native-fixture\nhost: @NATIVE@"); return; }
    assert_eq!(&args[..9], ["build", "-j1", "--locked", "--offline", "--release", "--target", "@NATIVE@", "-p", args[8].as_str()]);
    let mut calls = fs::OpenOptions::new().create(true).append(true).open(Path::new(&home).join("calls")).unwrap();
    writeln!(calls, "{}", args.join(" ")).unwrap();
    if Path::new(&home).join("fail-build").exists() && args[8] == "iroha_cli" { std::process::exit(24); }
    let release = Path::new(&env::var_os("CARGO_TARGET_DIR").unwrap()).join("@NATIVE@").join("release");
    for binary in args[9..].chunks_exact(2) {
        assert_eq!(binary[0], "--bin");
        let name = format!("{}{}", binary[1], if cfg!(windows) { ".exe" } else { "" });
        #[cfg(any(target_os = "linux", target_os = "android"))]
        fs::copy("/proc/self/exe", release.join(name)).unwrap();
        #[cfg(not(any(target_os = "linux", target_os = "android")))]
        fs::copy(env::current_exe().unwrap(), release.join(name)).unwrap();
    }
    println!("fixture build stdout must be redirected to stderr");
}
"#.replace("@NATIVE@", NATIVE_TARGET);
                std::fs::write(&source, text).unwrap();
                let result = std::process::Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
                    .args(["--edition=2024", "--crate-name=release_cli_fixture", "-C", "debuginfo=0"])
                    .arg(&source).arg("-o").arg(&executable).output().unwrap();
                assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
                std::fs::read(executable).unwrap()
            });
            let root = tempfile::tempdir().unwrap();
            for name in ["source", "cargo", "artifact", "home", "cancel", "tools"] {
                PrivateDirectory::open_or_create(root.path().join(name)).unwrap();
            }
            std::fs::write(
                root.path().join("source").join("Cargo.lock"),
                b"version = 4\n",
            )
            .unwrap();
            for name in ["cargo", "rustc"] {
                let path = root
                    .path()
                    .join("tools")
                    .join(format!("{name}{}", if cfg!(windows) { ".exe" } else { "" }));
                std::fs::write(&path, bytes).unwrap();
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt as _;
                    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
                }
            }
            Self {
                root,
                source: "a".repeat(64),
            }
        }

        fn arguments(&self) -> Vec<OsString> {
            let suffix = if cfg!(windows) { ".exe" } else { "" };
            let mut arguments = vec![OsString::from("build-publish")];
            for (name, value) in [
                ("--repo-root", self.root.path().join("source")),
                ("--cargo-target-dir", self.root.path().join("cargo")),
                ("--artifact-root", self.root.path().join("artifact")),
                ("--cargo-home", self.root.path().join("home")),
                (
                    "--cargo-bin",
                    self.root
                        .path()
                        .join("tools")
                        .join(format!("cargo{suffix}")),
                ),
                (
                    "--rustc-bin",
                    self.root
                        .path()
                        .join("tools")
                        .join(format!("rustc{suffix}")),
                ),
                (
                    "--cancel-request",
                    self.root.path().join("cancel").join("request"),
                ),
            ] {
                arguments.extend([OsString::from(name), value.into_os_string()]);
            }
            arguments.extend(
                [
                    "--source-sha256",
                    &self.source,
                    "--name",
                    "invocation.abc123",
                ]
                .map(OsString::from),
            );
            arguments
        }

        fn calls(&self) -> String {
            std::fs::read_to_string(self.root.path().join("home").join("calls")).unwrap()
        }
    }

    #[test]
    fn native_process_fixture_builds_exact_inventory_and_reuses_only_valid_anchor() {
        let fixture = BuildFixture::new();
        let mut output = Vec::new();
        run_fixture(fixture.arguments().into_iter(), &mut output)
            .unwrap_or_else(|error| panic!("{error}"));
        let output = String::from_utf8(output).unwrap();
        let lines: Vec<_> = output.lines().collect();
        assert_eq!(lines.len(), 2);
        let path = lines[0].strip_prefix("bundle_dir\t").unwrap();
        let manifest = lines[1].strip_prefix("manifest_sha256\t").unwrap();
        let calls = fixture.calls();
        assert_eq!(calls.lines().count(), 3);
        assert!(calls.contains("-p irohad --bin iroha3d --bin iroha3d_taira"));
        for kind in BinaryKind::ALL {
            assert!(
                std::path::Path::new(path)
                    .join(kind.filename(cfg!(windows)))
                    .is_file()
            );
        }
        let mut inherited = fixture.arguments();
        inherited.extend(["--bundle-dir", path, "--manifest-sha256", manifest].map(OsString::from));
        let mut reused = Vec::new();
        super::super::run_with_environment(inherited.clone().into_iter(), &mut reused, || {
            panic!("valid inherited admission must not capture a build environment")
        })
        .unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(reused, output.as_bytes());
        assert_eq!(fixture.calls(), calls);
        *inherited.last_mut().unwrap() = "0".repeat(64).into();
        let mut denied_output = Vec::new();
        assert!(run_fixture(inherited.into_iter(), &mut denied_output).is_err());
        assert!(denied_output.is_empty());
        assert_eq!(fixture.calls(), calls);
    }

    #[test]
    fn failed_second_build_or_plausible_failed_version_never_acknowledges() {
        for (marker, expected_calls, expected_status) in
            [("fail-build", 2, 24), ("fail-version", 3, 23)]
        {
            let fixture = BuildFixture::new();
            std::fs::write(fixture.root.path().join("home").join(marker), b"fail").unwrap();
            let mut output = Vec::new();
            let error = run_fixture(fixture.arguments().into_iter(), &mut output)
                .expect_err("failed native tool must refuse publication");
            assert_eq!(error.exit_code(), expected_status, "{error}");
            assert!(output.is_empty());
            assert_eq!(fixture.calls().lines().count(), expected_calls);
            assert!(
                !fixture
                    .root
                    .path()
                    .join("artifact")
                    .join("sumeragi-release")
                    .join(&fixture.source)
                    .join("programs")
                    .join("invocation.abc123")
                    .exists()
            );
        }
    }

    #[test]
    fn unsuccessful_nested_preflight_refuses_before_any_release_build() {
        let fixture = BuildFixture::new();
        std::fs::write(
            fixture.root.path().join("home").join("fail-probe-version"),
            b"refuse",
        )
        .unwrap();
        let mut output = Vec::new();
        let error = run_fixture(fixture.arguments().into_iter(), &mut output)
            .expect_err("unsuccessful original version probe must refuse");
        assert_eq!(error.exit_code(), 23);
        assert!(output.is_empty());
        assert!(!fixture.root.path().join("home").join("calls").exists());
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn descriptor_sensitive_proxy_is_refused_before_any_release_build() {
        let fixture = BuildFixture::new();
        std::fs::write(
            fixture.root.path().join("home").join("proxy-sensitive"),
            b"refuse",
        )
        .unwrap();
        let mut output = Vec::new();
        let error = run_fixture(fixture.arguments().into_iter(), &mut output)
            .expect_err("different nested argv0 role must refuse");
        assert!(
            error
                .to_string()
                .contains("nested original tool transcript differs")
        );
        assert!(output.is_empty());
        assert!(!fixture.root.path().join("home").join("calls").exists());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn actual_native_toolchain_versions_match_owned_nested_descriptor_spelling() {
        for (name, argument) in [("cargo", "--version"), ("rustc", "-vV")] {
            // Test-only discovery, never tool admission or release provenance.
            let selected = std::process::Command::new("rustup")
                .args(["which", name])
                .output()
                .unwrap();
            assert!(selected.status.success(), "{selected:?}");
            let path = PathBuf::from(std::str::from_utf8(&selected.stdout).unwrap().trim());
            let tool = Tool::open(&path).unwrap();
            let capture = |mut command: Command| {
                command
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::inherit());
                drain_child(command, true)
            };
            let mut direct = tool.command().unwrap();
            direct.arg(argument);
            let expected = capture(direct).unwrap_or_else(|error| panic!("{error}"));
            assert!(!expected.is_empty());
            tool.require_nested_transcript(argument, &expected, capture)
                .unwrap_or_else(|error| panic!("{error}"));
            tool.revalidate().unwrap();
        }
    }

    #[test]
    fn partial_inherited_anchor_refuses_before_any_cache_mutation() {
        let fixture = BuildFixture::new();
        for option in ["--manifest-sha256", "--bundle-dir"] {
            let mut arguments = fixture.arguments();
            arguments.extend([OsString::from(option), OsString::from("untrusted")]);
            let mut output = Vec::new();
            assert!(run_fixture(arguments.into_iter(), &mut output).is_err());
            assert!(output.is_empty());
            assert_eq!(
                std::fs::read_dir(fixture.root.path().join("cargo"))
                    .unwrap()
                    .count(),
                0
            );
            assert_eq!(
                std::fs::read_dir(fixture.root.path().join("artifact"))
                    .unwrap()
                    .count(),
                0
            );
            assert!(!fixture.root.path().join("home").join("calls").exists());
        }
    }

    #[test]
    fn private_shared_parent_accepts_sibling_outputs_and_cancellation_file() {
        let fixture = BuildFixture::new();
        let shared = PrivateDirectory::open_or_create(fixture.root.path().join("shared")).unwrap();
        let cargo = shared.create_child("cargo").unwrap();
        let artifacts = shared.create_child("artifacts").unwrap();
        let mut arguments = fixture.arguments();
        for (option, path) in [
            ("--cargo-target-dir", cargo.path().to_owned()),
            ("--artifact-root", artifacts.path().to_owned()),
            ("--cancel-request", shared.path().join("request")),
        ] {
            let index = arguments
                .iter()
                .position(|argument| argument == option)
                .unwrap();
            arguments[index + 1] = path.into_os_string();
        }
        let inputs =
            Inputs::parse(Arguments::parse(arguments.into_iter().skip(1)).unwrap()).unwrap();
        let session = Session::open(
            &inputs,
            super::super::environment::BuildEnvironment::for_fixture().unwrap(),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        session.boundary().unwrap_or_else(|error| panic!("{error}"));
        assert!(cargo.open_child("sumeragi-release").is_ok());
        assert!(artifacts.open_child("sumeragi-release").is_ok());
        shared
            .write_atomic(
                "request",
                b"{\"reason\":\"operator-request\",\"schema_version\":1}\n",
                iroha_fs::PublishMode::CreateNew,
            )
            .unwrap();
        assert_eq!(session.boundary().err().unwrap().exit_code(), 125);
        let mut roots = DisjointRoots::open(&[cargo.path(), artifacts.path()]).unwrap();
        assert!(roots.exclude_marker(&cargo.path().join("inside")).is_err());
        let mut overlapping = fixture.arguments();
        let index = overlapping
            .iter()
            .position(|value| value == "--cancel-request")
            .unwrap();
        overlapping[index + 1] = fixture.root.path().join("request").into_os_string();
        assert!(Inputs::parse(Arguments::parse(overlapping.into_iter().skip(1)).unwrap()).is_err());
    }

    #[cfg(any(windows, target_os = "linux", target_os = "android"))]
    #[test]
    fn configured_tools_select_original_images_through_ancestor_swap() {
        let fixture = BuildFixture::new();
        let inputs =
            Inputs::parse(Arguments::parse(fixture.arguments().into_iter().skip(1)).unwrap())
                .unwrap();
        let session = Session::open(
            &inputs,
            super::super::environment::BuildEnvironment::for_fixture().unwrap(),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        let mut command = session.cargo.command().unwrap();
        session.configure(&mut command, &inputs).unwrap();
        command.arg("--nested-tools");
        session.boundary().unwrap_or_else(|error| panic!("{error}"));
        let tools = inputs.cargo_bin.parent().unwrap();
        let displaced = fixture.root.path().join("original-tools");
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            std::fs::rename(tools, &displaced).unwrap();
            std::fs::create_dir(tools).unwrap();
            // Both replacement names select a different genuine native image;
            // neither the outer tool nor its two environment-selected children
            // may resolve these mutable names during the launch window.
            for path in [&inputs.cargo_bin, &inputs.rustc_bin] {
                std::fs::copy(std::env::current_exe().unwrap(), path).unwrap();
            }
        }
        #[cfg(windows)]
        assert!(std::fs::rename(tools, &displaced).is_err());
        let output = command.output().unwrap();
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            std::fs::remove_dir_all(tools).unwrap();
            std::fs::rename(&displaced, tools).unwrap();
        }
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!(
                "original-cargo-selected\ncargo native-fixture\nrustc native-fixture\nhost: {NATIVE_TARGET}\n"
            )
        );
        session.boundary().unwrap_or_else(|error| panic!("{error}"));
    }

    #[test]
    fn transient_wait_error_preserves_session_until_real_terminal_observation() {
        let fixture = BuildFixture::new();
        let inputs =
            Inputs::parse(Arguments::parse(fixture.arguments().into_iter().skip(1)).unwrap())
                .unwrap();
        let session = Session::open(
            &inputs,
            super::super::environment::BuildEnvironment::for_fixture().unwrap(),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        let mut command = session.cargo.command().unwrap();
        session.configure(&mut command, &inputs).unwrap();
        command.arg("--version").stdout(Stdio::null());
        let mut child = command.spawn().unwrap();
        let mut observations = 0;
        let (status, error) = wait_terminal(|| {
            observations += 1;
            session.boundary().unwrap_or_else(|error| panic!("{error}"));
            assert_eq!(
                Lock::acquire(&inputs.artifact_root, ".sumeragi-cargo.lock")
                    .err()
                    .unwrap()
                    .kind(),
                io::ErrorKind::WouldBlock
            );
            if observations == 1 {
                Err(io::Error::other("injected transient observation failure"))
            } else {
                child.wait()
            }
        });
        assert_eq!(child.wait().unwrap(), status);
        assert!(status.success());
        assert_eq!(observations, 2);
        assert_eq!(
            error.unwrap().to_string(),
            "injected transient observation failure"
        );
        session.boundary().unwrap_or_else(|error| panic!("{error}"));
    }

    #[cfg(windows)]
    #[test]
    #[allow(
        unsafe_code,
        reason = "observe root exit only through the original fixture process handle"
    )]
    fn exited_root_keeps_session_custody_until_original_job_descendant_exits() {
        use std::{
            fs::{File, OpenOptions},
            io::{BufRead as _, BufReader},
            os::windows::io::AsRawHandle as _,
        };

        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn WaitForSingleObject(
                handle: std::os::windows::io::RawHandle,
                milliseconds: u32,
            ) -> u32;
        }

        let fixture = BuildFixture::new();
        let inputs =
            Inputs::parse(Arguments::parse(fixture.arguments().into_iter().skip(1)).unwrap())
                .unwrap();
        let tools = [&inputs.cargo_bin, &inputs.rustc_bin];
        let identities = tools.map(|path| {
            // The owner can write these closed fixture originals before admission;
            // the later refusal must be caused by live retained custody.
            drop(OpenOptions::new().write(true).open(path).unwrap());
            iroha_fs::FileMetadata::of(&File::open(path).unwrap())
                .unwrap()
                .identity()
        });
        let session = Session::open(
            &inputs,
            super::super::environment::BuildEnvironment::for_fixture().unwrap(),
        )
        .unwrap_or_else(|error| panic!("{error}"));
        let mut command = session.cargo.command().unwrap();
        session.configure(&mut command, &inputs).unwrap();
        command
            .arg("--exit-with-owned-descendant")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command.spawn().unwrap();
        // Keep this sole control writer outside wait(), so only this test can
        // deliberately request descendant EOF after proving the root exited.
        let control = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        assert_eq!(line, "descendant-ready\n");
        // SAFETY: child retains this original process handle throughout this wait.
        assert_eq!(
            unsafe { WaitForSingleObject(child.as_raw_handle(), 10_000) },
            0
        );
        assert!(child.try_wait().unwrap().is_none());
        session.boundary().unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            Lock::acquire(&inputs.artifact_root, ".sumeragi-cargo.lock")
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::WouldBlock
        );
        for (path, identity) in tools.into_iter().zip(identities) {
            assert!(OpenOptions::new().write(true).open(path).is_err());
            assert!(std::fs::rename(path, path.with_extension("displaced")).is_err());
            assert_eq!(
                iroha_fs::FileMetadata::of(&File::open(path).unwrap())
                    .unwrap()
                    .identity(),
                identity
            );
        }
        // This closes the descendant's original inherited input, not a newly
        // selected process. Whole-Job wait must now observe its real terminal state.
        drop(control);
        let (status, error) = wait_terminal(|| {
            session.boundary().unwrap_or_else(|error| panic!("{error}"));
            child.wait()
        });
        assert!(error.is_none());
        assert!(status.success());
        assert_eq!(child.try_wait().unwrap(), Some(status));
        line.clear();
        output.read_to_string(&mut line).unwrap();
        assert_eq!(line, "descendant-observed-eof\n");
        session.boundary().unwrap_or_else(|error| panic!("{error}"));
        drop(session);
        drop(Lock::acquire(&inputs.artifact_root, ".sumeragi-cargo.lock").unwrap());
    }
}
