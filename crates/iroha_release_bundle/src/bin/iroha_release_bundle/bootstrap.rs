//! Native same-candidate publisher bootstrap transaction.
//!
//! This is entered only by an independently authenticated release tool owner. It
//! does not authenticate its own prior execution by hashing itself. The caller's
//! canonical workspace source keeper remains live across this entire operation.
//!
//! Windows child creation and terminal observation use the shared original Job
//! owner; this session retains its tools, configuration and locks through that wait.
//! TODO: Qualify the complete shipping bootstrap/producer transaction with
//! independently trusted native artifacts.

use super::{
    Arguments, Failure, acknowledge, admit,
    build::{self, Inputs, wait_terminal},
    exact_path, invalid,
    policy::{Cancellation, Configuration, DisjointRoots, Lock, Tool},
};
use iroha_fs::{
    FileSnapshot, FlatBatchEntry, FlatBatchKind, PrivateDirectory, ProcessCommand as Command,
    ProcessStdio as Stdio, RetainedBuildInput, RetainedFile, SealedFlatBatch,
};
use iroha_release_bundle::{bundle::NATIVE_TARGET, manifest::MAX_BINARY_BYTES};
use sha2::{Digest as _, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    io::{self, Read, Write},
    path::Path,
    sync::Arc,
};

mod events;
const ACK_LIMIT: usize = 128 * 1024;
const TOOL_LIMIT: usize = 64 * 1024;
const HELPER: &str = if cfg!(windows) {
    "iroha_release_bundle.exe"
} else {
    "iroha_release_bundle"
};

/// The two closed first-release publication targets; no caller-selected package/image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Artifact {
    Publisher,
    ProcessBridge,
}

impl Artifact {
    fn package(self) -> &'static str {
        match self {
            Self::Publisher => "iroha_release_bundle",
            Self::ProcessBridge => "iroha_process_bridge",
        }
    }

    fn filename(self) -> &'static str {
        match self {
            Self::Publisher => HELPER,
            Self::ProcessBridge if cfg!(windows) => "iroha_process_bridge.dll",
            Self::ProcessBridge => "libiroha_process_bridge.so",
        }
    }

    fn namespace(self) -> &'static str {
        match self {
            Self::Publisher => "publisher-bootstrap",
            Self::ProcessBridge => "process-bridge-publication",
        }
    }

    fn publication_directory(self) -> &'static str {
        match self {
            Self::Publisher => "publishers",
            Self::ProcessBridge => "bridges",
        }
    }

    fn source(self) -> &'static str {
        match self {
            Self::Publisher => "src/bin/iroha_release_bundle.rs",
            Self::ProcessBridge => "src/lib.rs",
        }
    }
}

struct Bootstrap {
    artifact: Artifact,
    environment: super::environment::BuildEnvironment,
    inputs: Inputs,
    roots: [DisjointRoots; 3],
    cargo_root: PrivateDirectory,
    artifact_root: PrivateDirectory,
    configuration: Configuration,
    cancellation: Cancellation,
    cargo: Tool,
    rustc: Tool,
    lock: RetainedFile,
    lock_snapshot: FileSnapshot,
    lock_sha256: String,
    build_lock: Lock,
    publication_lock: Lock,
    invocation: PrivateDirectory,
    target: PrivateDirectory,
    publishers: PrivateDirectory,
}

impl Bootstrap {
    fn open(
        inputs: Inputs,
        environment: super::environment::BuildEnvironment,
        artifact: Artifact,
    ) -> Result<Self, Failure> {
        let roots = inputs.retain_roots()?;
        let cargo_root = PrivateDirectory::open_exact(&inputs.cargo_root)?;
        let artifact_root = PrivateDirectory::open_exact(&inputs.artifact_root)?;
        let cancellation = Cancellation::open(&inputs.cancellation)?;
        cancellation.check()?;
        let configuration = Configuration::open(&inputs.repo, &inputs.cargo_home)?;
        let cargo = Tool::open(&inputs.cargo_bin)?;
        let rustc = Tool::open(&inputs.rustc_bin)?;
        let (lock, lock_snapshot, lock_sha256) =
            super::lock_digest(&inputs.repo.join("Cargo.lock"))?;
        let build_root = cargo_root
            .ensure_child("sumeragi-release")?
            .ensure_child(&inputs.source)?
            .ensure_child(artifact.namespace())?;
        // Distinct namespaces from the child publisher's program-build locks. The
        // originals remain live through the child; self-contention is impossible.
        let build_lock = Lock::acquire(build_root.path(), ".publisher-bootstrap.lock")?;
        let publication_lock = Lock::acquire(artifact_root.path(), ".publisher-bootstrap.lock")?;
        let invocation = build_root.create_child(&inputs.name)?;
        let target = invocation.create_child("cargo")?;
        let publishers = artifact_root
            .ensure_child("sumeragi-release")?
            .ensure_child(&inputs.source)?
            .ensure_child(artifact.publication_directory())?;
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let cargo = cargo.publish_cargo_image(&invocation, "cargo-image")?;
        let result = Self {
            artifact,
            environment,
            inputs,
            roots,
            cargo_root,
            artifact_root,
            configuration,
            cancellation,
            cargo,
            rustc,
            lock,
            lock_snapshot,
            lock_sha256,
            build_lock,
            publication_lock,
            invocation,
            target,
            publishers,
        };
        result.boundary()?;
        Ok(result)
    }

    fn boundary(&self) -> Result<(), Failure> {
        for root in &self.roots {
            root.revalidate()?;
        }
        self.cargo_root.revalidate()?;
        self.artifact_root.revalidate()?;
        self.configuration.revalidate()?;
        self.cancellation.check()?;
        self.cargo.revalidate()?;
        self.rustc.revalidate()?;
        self.lock.revalidate()?;
        if self.lock.snapshot()? != self.lock_snapshot {
            return Err(invalid("bootstrap Cargo.lock original changed").into());
        }
        self.build_lock.revalidate()?;
        self.publication_lock.revalidate()?;
        self.invocation.revalidate()?;
        self.target.revalidate()?;
        self.publishers.revalidate()?;
        Ok(())
    }

    fn configure(&self, command: &mut Command) -> io::Result<()> {
        self.environment.apply(command);
        self.configuration.bind_current_directory(command)?;
        command
            .env("CARGO_HOME", &self.inputs.cargo_home)
            .env("CARGO_TARGET_DIR", self.target.path())
            .env("RUSTUP_AUTO_INSTALL", "0")
            .env("ENABLE_RANS_BUNDLES", "1")
            .env("NORITO_SKIP_BINDINGS_SYNC", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        // Linux/Android bind the newly sealed non-Cargo-named executable original;
        // its installed source original remains retained by this session too.
        self.cargo.bind(command, "CARGO")?;
        self.rustc.bind(command, "RUSTC")?;
        Ok(())
    }

    fn capture(&self, mut command: Command, maximum: usize) -> Result<Vec<u8>, Failure> {
        self.boundary()?;
        self.configure(&mut command)?;
        let mut bytes = Vec::new();
        let result = drain(command, |chunk| {
            if bytes
                .len()
                .checked_add(chunk.len())
                .is_none_or(|length| length > maximum)
            {
                return Err(invalid("bootstrap command output exceeds its bound"));
            }
            bytes.extend_from_slice(chunk);
            Ok(())
        });
        self.boundary()?;
        result?;
        Ok(bytes)
    }

    fn build(&self, expected: events::Expected) -> Result<BTreeMap<String, String>, Failure> {
        self.boundary()?;
        let mut command = self.cargo.command()?;
        command.args([
            "build",
            "-j1",
            "--locked",
            "--offline",
            "--release",
            "--target",
            NATIVE_TARGET,
            "-p",
            self.artifact.package(),
        ]);
        match self.artifact {
            Artifact::Publisher => {
                command.args(["--bin", "iroha_release_bundle"]);
            }
            Artifact::ProcessBridge => {
                command.arg("--lib");
            }
        }
        command.arg("--message-format=json-render-diagnostics");
        self.configure(&mut command)?;
        // Capture the actual consumed command, including any original-descriptor
        // environment bindings; reconstructing a command after exit is not history.
        let environment = super::environment::BuildEnvironment::configured(&command)?;
        let mut observer = events::Observer::new(expected);
        let mut pending = Vec::new();
        let mut total = 0_usize;
        let result = drain(command, |chunk| {
            total = total
                .checked_add(chunk.len())
                .ok_or_else(|| invalid("Cargo event stream length overflow"))?;
            if total > events::OUTPUT_LIMIT {
                return Err(invalid("Cargo event stream exceeds its total bound"));
            }
            for byte in chunk {
                if *byte == b'\n' {
                    if pending.last() == Some(&b'\r') {
                        pending.pop();
                    }
                    observer.line(&pending)?;
                    pending.clear();
                } else if pending.len() < events::EVENT_LIMIT {
                    pending.push(*byte);
                } else {
                    return Err(invalid("Cargo event exceeds its line bound"));
                }
            }
            Ok(())
        });
        self.boundary()?;
        result?;
        if !pending.is_empty() {
            return Err(invalid("Cargo event stream has an unterminated final line").into());
        }
        observer.finish()?;
        Ok(environment)
    }
}

// Every observation error remains a refusal after the actual original child is
// terminal. No command, process handle, unobserved child, or source authority escapes.
fn drain(
    command: Command,
    mut observe: impl FnMut(&[u8]) -> io::Result<()>,
) -> Result<(), Failure> {
    let mut child = command.spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .expect("bootstrap config installs stdout pipe");
    let mut first_error = None;
    let mut buffer = [0_u8; 8192];
    loop {
        match stdout.read(&mut buffer) {
            Ok(0) => break,
            Ok(length) if first_error.is_none() => {
                if let Err(error) = observe(&buffer[..length]) {
                    first_error = Some(error);
                }
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
                break;
            }
        }
    }
    drop(stdout);
    let (status, wait_error) = wait_terminal(|| child.wait());
    if let Some(error) = first_error.or(wait_error) {
        return Err(error.into());
    }
    if !status.success() {
        return Err(Failure {
            error: io::Error::other("bootstrap child failed; no release acknowledgement accepted"),
            code: status
                .code()
                .and_then(|value| u8::try_from(value).ok())
                .filter(|value| *value != 0)
                .unwrap_or(1),
        });
    }
    Ok(())
}

struct Publisher {
    batch: Arc<SealedFlatBatch<2>>,
    executable_sha256: String,
    provenance_sha256: String,
    original: RetainedBuildInput,
    snapshot: FileSnapshot,
}

impl Publisher {
    fn seal(
        session: &Bootstrap,
        path: &Path,
        package: &str,
        cargo: &[u8],
        rustc: &[u8],
        environment: BTreeMap<String, String>,
    ) -> Result<Self, Failure> {
        session.boundary()?;
        let mut original = RetainedBuildInput::open(path)?;
        let snapshot = original.snapshot()?;
        let length = original.len()?;
        if length == 0 || length > MAX_BINARY_BYTES {
            return Err(invalid("emitted publisher has invalid extent").into());
        }
        let entries = [
            FlatBatchEntry::new(
                session.artifact.filename(),
                FlatBatchKind::Executable,
                usize::try_from(MAX_BINARY_BYTES)
                    .map_err(|_| invalid("unsupported executable bound"))?,
            )?,
            FlatBatchEntry::new(
                "publisher-source.json",
                FlatBatchKind::ReadOnly,
                super::provenance::MAX_BYTES,
            )?,
        ];
        let mut pending = session
            .publishers
            .create_flat_batch(&session.inputs.name, entries)?;
        let mut hasher = Sha256::new();
        let mut copied = 0_u64;
        let mut buffer = [0_u8; 16 * 1024];
        {
            let mut writer = pending.writer(0)?;
            loop {
                let count = original.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                copied = copied
                    .checked_add(count as u64)
                    .ok_or_else(|| invalid("publisher extent overflow"))?;
                if copied > length {
                    return Err(invalid("original publisher grew during copy").into());
                }
                writer.write_all(&buffer[..count])?;
                hasher.update(&buffer[..count]);
            }
        }
        if copied != length || original.snapshot()? != snapshot {
            return Err(invalid("original publisher changed during copy").into());
        }
        let digest = format!("{:x}", hasher.finalize());
        let environment_policy = super::environment::BuildEnvironment::policy_digest();
        let provenance = super::provenance::Publisher {
            schema: "iroha.publisher.v1".into(),
            source_manifest_sha256: session.inputs.source.clone(),
            cargo_lock_sha256: session.lock_sha256.clone(),
            native_target: NATIVE_TARGET.into(),
            package_id: package.into(),
            executable_filename: session.artifact.filename().into(),
            executable_sha256: digest.clone(),
            executable_bytes: length,
            cargo_tool: session.cargo.evidence()?,
            rustc_tool: session.rustc.evidence()?,
            cargo_version: std::str::from_utf8(cargo)
                .map_err(|_| invalid("Cargo transcript is not UTF-8"))?
                .into(),
            rustc_version: std::str::from_utf8(rustc)
                .map_err(|_| invalid("rustc transcript is not UTF-8"))?
                .into(),
            configurations: session.configuration.evidence()?,
            build_environment_policy_sha256: environment_policy,
            bootstrap_build_environment: environment,
        }
        .encode()?;
        pending.writer(1)?.write_all(&provenance)?;
        let batch = Arc::new(pending.seal()?);
        let mut reader = batch.reader(0)?;
        let mut observed = Sha256::new();
        let mut observed_length = 0_u64;
        loop {
            let count = reader.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            observed.update(&buffer[..count]);
            observed_length += count as u64;
        }
        if observed_length != length || format!("{:x}", observed.finalize()) != digest {
            return Err(invalid("sealed publisher differs from original emitted bytes").into());
        }
        let mut observed = Vec::new();
        batch.reader(1)?.read_to_end(&mut observed)?;
        if observed != provenance {
            return Err(
                invalid("sealed publisher source binding differs from authored bytes").into(),
            );
        }
        let result = Self {
            batch,
            executable_sha256: digest,
            provenance_sha256: format!("{:x}", Sha256::digest(&provenance)),
            original,
            snapshot,
        };
        result.revalidate()?;
        session.boundary()?;
        Ok(result)
    }

    fn revalidate(&self) -> io::Result<()> {
        self.original.revalidate()?;
        if self.original.snapshot()? != self.snapshot {
            return Err(invalid("original Cargo publisher identity changed"));
        }
        self.batch.revalidate()
    }
}

fn arguments(values: &Arguments) -> Vec<OsString> {
    let mut result = vec!["build-publish".into()];
    for (key, value) in &values.0 {
        result.push(key.into());
        result.push(value.clone());
    }
    result
}

pub(super) fn run(
    arguments: Arguments,
    output: &mut impl Write,
    environment: fn() -> io::Result<super::environment::BuildEnvironment>,
) -> Result<(), Failure> {
    let inputs = Inputs::parse(arguments.clone())?;
    if inputs.inherited.is_some() {
        // Inherited admission performs zero bootstrap or program build commands.
        return build::run(arguments, output, environment);
    }
    let (session, publisher) = build_publisher(inputs, environment, Artifact::Publisher)?;
    let mut command = publisher.batch.command(0)?;
    command.args(self::arguments(&arguments));
    publisher.revalidate()?;
    let acknowledgement = session.capture(command, ACK_LIMIT)?;
    publisher.revalidate()?;
    session.boundary()?;
    let acknowledgement = std::str::from_utf8(&acknowledgement)
        .map_err(|_| invalid("publisher acknowledgement is not UTF-8"))?;
    let fields: Vec<_> = acknowledgement.split('\n').collect();
    if fields.len() != 3 || !fields[2].is_empty() {
        return Err(invalid("publisher acknowledgement is not exactly two fields").into());
    }
    let path = exact_path(
        fields[0]
            .strip_prefix("bundle_dir\t")
            .ok_or_else(|| invalid("publisher acknowledgement omitted bundle path"))?
            .into(),
    )?;
    let digest = super::digest_argument(
        fields[1]
            .strip_prefix("manifest_sha256\t")
            .ok_or_else(|| invalid("publisher acknowledgement omitted manifest anchor"))?
            .into(),
    )?;
    let parent = session
        .inputs
        .artifact_root
        .join("sumeragi-release")
        .join(&session.inputs.source)
        .join("programs");
    if path.as_os_str() != parent.join(&session.inputs.name).as_os_str() {
        return Err(invalid("publisher acknowledged another invocation").into());
    }
    let (bundle, private) = admit(
        &parent,
        &session.inputs.name,
        &session.inputs.source,
        &session.lock_sha256,
        &digest,
    )?;
    publisher.revalidate()?;
    session.boundary()?;
    private.revalidate()?;
    acknowledge(&bundle, output)?;
    publisher.revalidate()?;
    session.boundary()?;
    private.revalidate()?;
    Ok(())
}

fn build_publisher(
    inputs: Inputs,
    environment: fn() -> io::Result<super::environment::BuildEnvironment>,
    artifact: Artifact,
) -> Result<(Bootstrap, Publisher), Failure> {
    let session = Bootstrap::open(inputs, environment()?, artifact)?;
    let package_root = session.inputs.repo.join("crates").join(artifact.package());
    let manifest = package_root.join("Cargo.toml");
    let source = package_root.join(artifact.source());
    let executable = session
        .target
        .path()
        .join(NATIVE_TARGET)
        .join("release")
        .join(artifact.filename());
    let mut metadata = session.cargo.command()?;
    metadata.args([
        "metadata",
        "--locked",
        "--offline",
        "--no-deps",
        "--format-version",
        "1",
    ]);
    let metadata = session.capture(metadata, events::EVENT_LIMIT)?;
    let package = events::package_identity(&metadata, &manifest, &source, artifact)?;
    let mut cargo_version = session.cargo.source_command()?;
    cargo_version.arg("--version");
    let cargo_version = session.capture(cargo_version, TOOL_LIMIT)?;
    let mut rustc_version = session.rustc.source_command()?;
    rustc_version.arg("-vV");
    let rustc_version = session.capture(rustc_version, TOOL_LIMIT)?;
    for (tool, argument, expected) in [
        (&session.cargo, "--version", cargo_version.as_slice()),
        (&session.rustc, "-vV", rustc_version.as_slice()),
    ] {
        tool.require_nested_transcript(argument, expected, |probe| {
            session.capture(probe, TOOL_LIMIT)
        })?;
    }
    let build_environment = session.build(events::Expected {
        artifact,
        package: package.clone(),
        manifest,
        source,
        executable: executable.clone(),
    })?;
    let publisher = Publisher::seal(
        &session,
        &executable,
        &package,
        &cargo_version,
        &rustc_version,
        build_environment,
    )?;
    Ok((session, publisher))
}

/// Publish only the exact native shared library and its canonical source record.
///
/// The caller retains the canonical workspace source owner and independently
/// authenticates this publisher before entry. Returned digests describe observed
/// output; they do not authorize the artifact's later loading by themselves.
pub(super) fn run_bridge(
    arguments: Arguments,
    output: &mut impl Write,
    environment: fn() -> io::Result<super::environment::BuildEnvironment>,
) -> Result<(), Failure> {
    if !cfg!(any(target_os = "linux", windows)) {
        return Err(invalid("native bridge publication is unsupported on this host").into());
    }
    let inputs = Inputs::parse(arguments)?;
    if inputs.inherited.is_some() {
        return Err(invalid("bridge publication cannot inherit a program bundle").into());
    }
    let (session, publisher) = build_publisher(inputs, environment, Artifact::ProcessBridge)?;
    publisher.revalidate()?;
    session.boundary()?;
    let path = publisher
        .batch
        .path()
        .to_str()
        .ok_or_else(|| invalid("bridge artifact path is not UTF-8"))?;
    if path.chars().any(char::is_control) {
        return Err(invalid("bridge artifact path contains control characters").into());
    }
    write!(
        output,
        "bridge_dir\t{path}\nmodule_sha256\t{}\nprovenance_sha256\t{}\n",
        publisher.executable_sha256, publisher.provenance_sha256
    )?;
    output.flush()?;
    publisher.revalidate()?;
    session.boundary()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, fs};

    struct Fixture {
        root: PrivateDirectory,
        _temporary: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let temporary = tempfile::tempdir().unwrap();
            let root = PrivateDirectory::open_or_create(temporary.path().join("private")).unwrap();
            for name in [
                "source",
                "cargo",
                "artifacts",
                "cargo-home",
                "controls",
                "tools",
            ] {
                root.create_child(name).unwrap();
            }
            root.ensure_child("source")
                .unwrap()
                .write_atomic(
                    "Cargo.lock",
                    b"version = 4\n",
                    iroha_fs::PublishMode::CreateNew,
                )
                .unwrap();
            fs::copy(
                std::env::current_exe().unwrap(),
                root.path().join("tools").join(HELPER),
            )
            .unwrap();
            Self {
                root,
                _temporary: temporary,
            }
        }
        fn arguments(&self) -> Arguments {
            let tool = self.root.path().join("tools").join(HELPER);
            Arguments(BTreeMap::from([
                (
                    "--repo-root".into(),
                    self.root.path().join("source").into_os_string(),
                ),
                ("--source-sha256".into(), "a".repeat(64).into()),
                (
                    "--cargo-target-dir".into(),
                    self.root.path().join("cargo").into_os_string(),
                ),
                (
                    "--artifact-root".into(),
                    self.root.path().join("artifacts").into_os_string(),
                ),
                (
                    "--cargo-home".into(),
                    self.root.path().join("cargo-home").into_os_string(),
                ),
                ("--cargo-bin".into(), tool.clone().into_os_string()),
                ("--rustc-bin".into(), tool.into_os_string()),
                (
                    "--cancel-request".into(),
                    self.root
                        .path()
                        .join("controls")
                        .join("cancel.json")
                        .into_os_string(),
                ),
                ("--name".into(), "invocation.abc123".into()),
            ]))
        }
        fn try_session(&self) -> Result<Bootstrap, Failure> {
            Bootstrap::open(
                Inputs::parse(self.arguments()).unwrap(),
                super::super::environment::BuildEnvironment::for_fixture().unwrap(),
                Artifact::Publisher,
            )
        }
        fn session(&self) -> Bootstrap {
            self.try_session().unwrap()
        }
        fn publisher(session: &Bootstrap) -> Publisher {
            let target = iroha_fs::OwnerDirectory::open(session.target.path())
                .unwrap()
                .ensure_child(NATIVE_TARGET)
                .unwrap()
                .ensure_child("release")
                .unwrap();
            let output = target.path().join(HELPER);
            fs::copy(std::env::current_exe().unwrap(), &output).unwrap();
            // Actual initial Cargo-style hardlink; custody may admit it only before
            // the original snapshot is captured, never as a later link-count drift.
            fs::hard_link(&output, target.path().join("cargo-alias")).unwrap();
            // This component fixture does not run Cargo. Its environment evidence
            // belongs to this actual original native child, not a later builder.
            let mut command = session.cargo.command().unwrap();
            session.configure(&mut command).unwrap();
            command.args(["--exact", "cli::bootstrap::tests::sealed_publisher_child"]);
            let environment =
                super::super::environment::BuildEnvironment::configured(&command).unwrap();
            let mut captured = Vec::new();
            drain(command, |bytes| {
                captured.extend_from_slice(bytes);
                Ok(())
            })
            .unwrap();
            let captured = String::from_utf8(captured).unwrap();
            assert!(captured.contains("running 1 test"), "{captured}");
            assert!(
                captured.contains("cli::bootstrap::tests::sealed_publisher_child ... ok"),
                "{captured}"
            );
            Publisher::seal(
                session,
                &output,
                "actual-test-fixture-package",
                b"cargo fixture\n",
                format!("rustc fixture\nhost: {NATIVE_TARGET}\n").as_bytes(),
                environment,
            )
            .unwrap()
        }
    }

    #[test]
    fn original_build_locks_cover_both_namespaces_and_never_adopt_old_invocation() {
        let fixture = Fixture::new();
        let session = fixture.session();
        assert!(fixture.try_session().is_err());
        session.boundary().unwrap();
        let path = session.invocation.path().to_owned();
        drop(session);
        assert!(fixture.try_session().is_err());
        assert!(PrivateDirectory::open_exact(path).is_ok());
    }

    #[test]
    fn initial_cancellation_refuses_before_any_bootstrap_namespace_is_created() {
        let fixture = Fixture::new();
        fixture
            .root
            .ensure_child("controls")
            .unwrap()
            .write_atomic(
                "cancel.json",
                b"{\"reason\":\"operator-request\",\"schema_version\":1}\n",
                iroha_fs::PublishMode::CreateNew,
            )
            .unwrap();
        let error = fixture.try_session().err().unwrap();
        assert_eq!(error.exit_code(), 125);
        assert!(
            !fixture
                .root
                .path()
                .join("cargo")
                .join("sumeragi-release")
                .exists()
        );
        assert!(
            !fixture
                .root
                .path()
                .join("artifacts")
                .join("sumeragi-release")
                .exists()
        );
    }

    const CHILD: &str = "IROHA_BOOTSTRAP_FIXTURE_CHILD";

    #[test]
    fn sealed_publisher_child() {
        match std::env::var(CHILD).as_deref() {
            Ok("success") => println!("original-bootstrap-helper-executed"),
            Ok("failure") => panic!("deliberate original bootstrap child failure"),
            _ => {}
        }
    }

    fn child(publisher: &Publisher, mode: &str) -> Command {
        let mut command = publisher.batch.command(0).unwrap();
        command
            .args([
                "--exact",
                "cli::bootstrap::tests::sealed_publisher_child",
                "--nocapture",
            ])
            .env(CHILD, mode)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        command
    }

    #[test]
    fn emitted_original_and_immutable_parent_survive_actual_helper_execution() {
        let fixture = Fixture::new();
        let session = fixture.session();
        let publisher = Fixture::publisher(&session);
        let original = publisher.snapshot;
        let sealed = publisher.batch.entry_identity(0).unwrap();
        let mut output = Vec::new();
        drain(child(&publisher, "success"), |bytes| {
            output.extend_from_slice(bytes);
            Ok(())
        })
        .unwrap();
        assert!(
            String::from_utf8(output)
                .unwrap()
                .contains("original-bootstrap-helper-executed")
        );
        assert_eq!(publisher.original.snapshot().unwrap(), original);
        assert_eq!(publisher.batch.entry_identity(0).unwrap(), sealed);
        publisher.revalidate().unwrap();
        session.boundary().unwrap();
    }

    #[test]
    fn failed_original_helper_never_grants_acknowledgement_or_releases_custody_early() {
        let fixture = Fixture::new();
        let session = fixture.session();
        let publisher = Fixture::publisher(&session);
        assert!(drain(child(&publisher, "failure"), |_| Ok(())).is_err());
        publisher.revalidate().unwrap();
        session.boundary().unwrap();
    }

    #[test]
    fn observation_refusal_still_drains_and_observes_the_actual_original_child() {
        let fixture = Fixture::new();
        let session = fixture.session();
        let publisher = Fixture::publisher(&session);
        let mut called = false;
        let result = drain(child(&publisher, "success"), |_| {
            called = true;
            Err(io::Error::other("injected output observation failure"))
        });
        assert!(called);
        assert!(result.is_err());
        publisher.revalidate().unwrap();
        session.boundary().unwrap();
    }

    #[test]
    fn bridge_and_helper_publication_policies_remain_closed_and_distinct() {
        assert_eq!(Artifact::Publisher.package(), "iroha_release_bundle");
        assert_eq!(Artifact::Publisher.filename(), HELPER);
        assert_eq!(
            Artifact::Publisher.source(),
            "src/bin/iroha_release_bundle.rs"
        );
        assert_eq!(Artifact::Publisher.namespace(), "publisher-bootstrap");
        assert_eq!(Artifact::Publisher.publication_directory(), "publishers");
        assert_eq!(Artifact::ProcessBridge.package(), "iroha_process_bridge");
        assert_eq!(Artifact::ProcessBridge.source(), "src/lib.rs");
        assert_eq!(
            Artifact::ProcessBridge.namespace(),
            "process-bridge-publication"
        );
        assert_eq!(Artifact::ProcessBridge.publication_directory(), "bridges");
        assert_eq!(
            Artifact::ProcessBridge.filename(),
            if cfg!(windows) {
                "iroha_process_bridge.dll"
            } else {
                "libiroha_process_bridge.so"
            }
        );
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn bridge_inherited_program_bundle_refuses_before_environment_or_build() {
        fn unavailable() -> io::Result<super::super::environment::BuildEnvironment> {
            panic!("inherited bridge request reached build environment");
        }
        let fixture = Fixture::new();
        let mut arguments = fixture.arguments();
        arguments.0.insert(
            "--bundle-dir".into(),
            fixture.root.path().join("foreign").into_os_string(),
        );
        arguments
            .0
            .insert("--manifest-sha256".into(), "a".repeat(64).into());
        let mut output = Vec::new();
        assert!(run_bridge(arguments, &mut output, unavailable).is_err());
        assert!(output.is_empty());
    }
}
