//! Shared content-addressed Kotodama build driver.
//!
//! All developer entry points use this module for cache validation and output
//! publication. A build record is a commit marker: it is written only after the
//! artifact, manifest, interface, and hash-keyed sidecars have been durably
//! published. Cache hits recompute every output digest and the canonical
//! deployable code hash before skipping compilation. Cache reads are bounded so
//! a corrupted or adversarial local target directory cannot force unbounded
//! allocation before authentication.

use std::{
    collections::HashMap,
    error::Error,
    fmt, fs,
    io::{Read as _, Write as _},
    path::{Component, Path, PathBuf},
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

use iroha_crypto::Hash;
use iroha_data_model::smart_contract::manifest::ContractManifest;
use norito::json;

use crate::{
    diagnostic::DiagnosticBundle,
    linker::{ModuleBuildGraph, SourceGraphError, SourceLinkRequest},
    metadata::contract_code_hash,
    session::{CompileOutput, CompileRequest, CompilerSession},
};

const BUILD_RECORD_SCHEMA: &str = "kotodama-build-v1";
const DEFAULT_TARGET_ROOT: &str = "target/kotodama";
const MAX_BUILD_RECORD_BYTES: usize = 4 * 1024;
const MAX_CACHED_OUTPUT_BYTES: usize = 32 * 1024 * 1024;
static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);
static CURRENT_TOOLCHAIN_FINGERPRINT: OnceLock<String> = OnceLock::new();

/// Whether generated files may be updated or must already match exactly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PublishMode {
    /// Atomically publish changed outputs.
    #[default]
    Write,
    /// Verify that every output and build record is current without writing.
    Verify,
}

/// Stable output paths known before compilation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishLayout {
    /// Deployable `.to` bytecode path.
    pub artifact: PathBuf,
    /// Canonical compiler manifest JSON path.
    pub manifest: PathBuf,
    /// Optional generated interface JSON path.
    pub interface: Option<PathBuf>,
    manifest_storage: ManifestStorage,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ManifestStorage {
    #[default]
    Output,
    Sidecar,
}

impl PublishLayout {
    /// Construct standard paths below `target/kotodama/<profile>/`.
    pub fn standard(
        target_root: impl AsRef<Path>,
        profile: &str,
        stem: &str,
        include_interface: bool,
    ) -> Result<Self, BuildError> {
        validate_profile(profile)?;
        validate_stem(stem)?;
        let directory = target_root.as_ref().join(profile);
        Ok(Self {
            artifact: directory.join(format!("{stem}.to")),
            manifest: directory.join(format!("{stem}.manifest.json")),
            interface: include_interface.then(|| directory.join(format!("{stem}.interface.json"))),
            manifest_storage: ManifestStorage::Output,
        })
    }

    /// Construct standard paths below the repository default target root.
    pub fn default_target(
        profile: &str,
        stem: &str,
        include_interface: bool,
    ) -> Result<Self, BuildError> {
        Self::standard(DEFAULT_TARGET_ROOT, profile, stem, include_interface)
    }

    /// Construct a layout around an explicit artifact path.
    pub fn for_artifact(
        artifact: PathBuf,
        manifest: Option<PathBuf>,
        interface: Option<PathBuf>,
    ) -> Result<Self, BuildError> {
        let stem = artifact
            .file_stem()
            .and_then(|value| value.to_str())
            .ok_or_else(|| BuildError::InvalidPath {
                path: artifact.clone(),
                message: "artifact path must have a UTF-8 file stem".to_owned(),
            })?;
        let manifest = manifest
            .unwrap_or_else(|| output_parent(&artifact).join(format!("{stem}.manifest.json")));
        Ok(Self {
            artifact,
            manifest,
            interface,
            manifest_storage: ManifestStorage::Output,
        })
    }

    /// Keep the authenticated manifest as a content-addressed build sidecar.
    ///
    /// This is used when a frontend returns the manifest through another
    /// channel, such as `koto build --manifest-out -`. The manifest remains
    /// available to authenticate no-op builds without publishing an
    /// unexpected sibling file beside the requested artifact.
    #[must_use]
    pub fn with_sidecar_manifest(mut self) -> Self {
        self.manifest_storage = ManifestStorage::Sidecar;
        self
    }

    fn resolve(&self, artifact_hash: &str, input_fingerprint: &str) -> BuildPaths {
        let parent = output_parent(&self.artifact);
        let sidecars = parent
            .join(".sidecars")
            .join(artifact_hash)
            .join(input_fingerprint);
        let file_name = self
            .artifact
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("seiyaku.to"));
        let record = parent
            .join(".fingerprints")
            .join(format!("{}.record", file_name.to_string_lossy()));
        let manifest = match self.manifest_storage {
            ManifestStorage::Output => self.manifest.clone(),
            ManifestStorage::Sidecar => sidecars.join("manifest.json"),
        };
        BuildPaths {
            artifact: self.artifact.clone(),
            manifest,
            interface: self.interface.clone(),
            source_map: sidecars.join("source-map.json"),
            budget: sidecars.join("budget.json"),
            record,
        }
    }

    fn static_paths(&self) -> Vec<PathBuf> {
        let provisional = self.resolve("artifact", "input");
        let mut paths = vec![
            provisional.artifact,
            provisional.manifest,
            provisional.record,
        ];
        paths.extend(provisional.interface);
        paths
    }
}

/// Every output path for a completed build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildPaths {
    /// Deployable `.to` bytecode.
    pub artifact: PathBuf,
    /// Canonical manifest JSON.
    pub manifest: PathBuf,
    /// Optional interface JSON.
    pub interface: Option<PathBuf>,
    /// Hash-keyed source map.
    pub source_map: PathBuf,
    /// Hash-keyed compiler budget report.
    pub budget: PathBuf,
    /// Transactional build commit record.
    pub record: PathBuf,
}

impl BuildPaths {
    fn all_paths(&self) -> Vec<PathBuf> {
        let mut paths = vec![
            self.artifact.clone(),
            self.manifest.clone(),
            self.source_map.clone(),
            self.budget.clone(),
            self.record.clone(),
        ];
        paths.extend(self.interface.iter().cloned());
        paths
    }
}

/// Whether compilation was skipped or performed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildStatus {
    /// Every authenticated output was already current; no file was written.
    Fresh,
    /// Compilation ran and outputs were published or verified.
    Built,
}

/// Successful content-addressed build result.
#[derive(Clone, Debug)]
pub struct BuildOutcome {
    /// Cache status.
    pub status: BuildStatus,
    /// Canonical deployable artifact hash.
    pub artifact_hash: Hash,
    /// Deployable bytes (read from cache on a fresh result).
    pub artifact: Vec<u8>,
    /// Canonical contract manifest.
    pub manifest: ContractManifest,
    /// Published or verified paths.
    pub paths: BuildPaths,
}

/// One ordinary source build request.
#[derive(Clone, Debug)]
pub struct SourceBuildRequest {
    /// Complete source text.
    pub source: String,
    /// Logical path used by diagnostics and sidecars.
    pub source_name: String,
    /// Build profile and output namespace.
    pub profile: String,
    /// Output layout.
    pub layout: PublishLayout,
    /// Write or verification policy.
    pub mode: PublishMode,
}

/// One locked source-module graph built lazily after cache authentication.
#[derive(Debug)]
pub struct LinkedSourceBuildRequest {
    /// Seiyaku root, explicit imports, and locked transitive module sources.
    pub graph: SourceLinkRequest,
    /// Logical root path used by diagnostics and sidecars.
    pub source_name: String,
    /// Build profile and output namespace.
    pub profile: String,
    /// Output layout.
    pub layout: PublishLayout,
    /// Write or verification policy.
    pub mode: PublishMode,
}

/// Shared compiler, cache validator, and atomic publisher.
#[derive(Clone, Debug)]
pub struct BuildDriver {
    session: CompilerSession,
    toolchain_fingerprint: String,
}

impl BuildDriver {
    /// Create a driver with an explicit compiler/tool executable identity.
    pub fn new(session: CompilerSession, toolchain_fingerprint: impl Into<String>) -> Self {
        Self {
            session,
            toolchain_fingerprint: toolchain_fingerprint.into(),
        }
    }

    /// Create a driver whose cache identity includes the complete running executable.
    pub fn for_current_executable(session: CompilerSession) -> Result<Self, BuildError> {
        if let Some(fingerprint) = CURRENT_TOOLCHAIN_FINGERPRINT.get() {
            return Ok(Self::new(session, fingerprint.clone()));
        }
        let executable = std::env::current_exe().map_err(|error| BuildError::Io {
            operation: "locate running compiler",
            path: PathBuf::from("<current executable>"),
            message: error.to_string(),
        })?;
        let bytes = fs::read(&executable).map_err(|error| BuildError::Io {
            operation: "fingerprint compiler",
            path: executable,
            message: error.to_string(),
        })?;
        let fingerprint = Hash::new_from_chunks(&[b"kotodama-toolchain-v1\0", &bytes]);
        let fingerprint = fingerprint.to_string();
        let _ = CURRENT_TOOLCHAIN_FINGERPRINT.set(fingerprint.clone());
        Ok(Self::new(
            session,
            CURRENT_TOOLCHAIN_FINGERPRINT
                .get()
                .cloned()
                .unwrap_or(fingerprint),
        ))
    }

    /// Build one source unit, authenticating all cached outputs before a hit.
    pub fn build_source(&self, request: SourceBuildRequest) -> Result<BuildOutcome, BuildError> {
        validate_profile(&request.profile)?;
        reject_layout_collisions(&request.layout, &request.source_name)?;
        let input_fingerprint = self.input_fingerprint(
            b"source",
            &request.source_name,
            &request.profile,
            request.source.as_bytes(),
        );
        if let Some(fresh) = self.try_fresh(&request.layout, &input_fingerprint.to_string()) {
            return Ok(fresh);
        }
        let output = self
            .session
            .build(CompileRequest {
                source: &request.source,
                source_name: Some(&request.source_name),
            })
            .map_err(BuildError::Compile)?;
        self.finish_build(
            output,
            &request.layout,
            &input_fingerprint.to_string(),
            request.mode,
        )
    }

    /// Build a locked source-module graph without compiling an authenticated hit.
    ///
    /// The graph fingerprint is cheap preflight work over bounded input bytes.
    /// Parsing, name resolution, type/effect analysis, HIR linking, and code
    /// generation all occur only after cached outputs fail authentication.
    pub fn build_linked_source(
        &self,
        graph: &ModuleBuildGraph,
        request: LinkedSourceBuildRequest,
    ) -> Result<BuildOutcome, BuildError> {
        validate_profile(&request.profile)?;
        reject_layout_collisions(&request.layout, &request.source_name)?;
        let graph_fingerprint =
            ModuleBuildGraph::fingerprint(&request.graph).map_err(BuildError::SourceGraph)?;
        let input_fingerprint = self.input_fingerprint(
            b"typed-graph",
            &request.source_name,
            &request.profile,
            graph_fingerprint.as_ref(),
        );
        if let Some(fresh) = self.try_fresh(&request.layout, &input_fingerprint.to_string()) {
            return Ok(fresh);
        }
        let linked = graph
            .link(request.graph, self.session.linker_options())
            .map_err(BuildError::SourceGraph)?;
        if linked.fingerprint != graph_fingerprint {
            return Err(BuildError::Internal(
                "module graph identity changed between preflight and linking".to_owned(),
            ));
        }
        let output = self
            .session
            .build_typed_program(linked.program, Some(&request.source_name))
            .map_err(BuildError::Compile)?;
        self.finish_build(
            output,
            &request.layout,
            &input_fingerprint.to_string(),
            request.mode,
        )
    }

    /// Build independent source roots in parallel and return results in request order.
    pub fn build_source_batch(
        &self,
        requests: Vec<SourceBuildRequest>,
    ) -> Result<Vec<BuildOutcome>, BuildError> {
        reject_output_collisions(&requests)?;
        let jobs = std::thread::available_parallelism()
            .map_or(1, std::num::NonZeroUsize::get)
            .max(1);
        let mut outcomes = Vec::with_capacity(requests.len());
        for chunk in requests.chunks(jobs) {
            let results = std::thread::scope(|scope| {
                let handles = chunk
                    .iter()
                    .cloned()
                    .map(|request| scope.spawn(move || self.build_source(request)))
                    .collect::<Vec<_>>();
                handles
                    .into_iter()
                    .map(|handle| {
                        handle.join().map_err(|_| {
                            BuildError::Internal("Kotodama build worker panicked".to_owned())
                        })?
                    })
                    .collect::<Result<Vec<_>, BuildError>>()
            })?;
            outcomes.extend(results);
        }
        Ok(outcomes)
    }

    fn input_fingerprint(
        &self,
        kind: &[u8],
        source_name: &str,
        profile: &str,
        payload: &[u8],
    ) -> Hash {
        let mut transcript = b"kotodama-build-input-v1\0".to_vec();
        append_field(&mut transcript, kind);
        append_field(&mut transcript, self.toolchain_fingerprint.as_bytes());
        append_field(&mut transcript, self.session.policy_fingerprint().as_ref());
        append_field(&mut transcript, source_name.as_bytes());
        append_field(&mut transcript, profile.as_bytes());
        append_field(&mut transcript, payload);
        Hash::new(transcript)
    }

    fn try_fresh(&self, layout: &PublishLayout, input: &str) -> Option<BuildOutcome> {
        let provisional = layout.resolve("artifact", input);
        let record_bytes = read_bounded_file(&provisional.record, MAX_BUILD_RECORD_BYTES)?;
        let record = BuildRecord::parse(std::str::from_utf8(&record_bytes).ok()?)?;
        if record.input != input {
            return None;
        }
        let paths = layout.resolve(&record.artifact_hash, input);
        if paths.record != provisional.record {
            return None;
        }
        let artifact = read_bounded_file(&paths.artifact, MAX_CACHED_OUTPUT_BYTES)?;
        let manifest_bytes = read_bounded_file(&paths.manifest, MAX_CACHED_OUTPUT_BYTES)?;
        let source_map = read_bounded_file(&paths.source_map, MAX_CACHED_OUTPUT_BYTES)?;
        let budget = read_bounded_file(&paths.budget, MAX_CACHED_OUTPUT_BYTES)?;
        let interface = match (&paths.interface, &record.interface) {
            (Some(path), Some(_)) => Some(read_bounded_file(path, MAX_CACHED_OUTPUT_BYTES)?),
            (None, None) => None,
            _ => return None,
        };
        if output_hash("artifact", &artifact).to_string() != record.artifact
            || output_hash("manifest", &manifest_bytes).to_string() != record.manifest
            || output_hash("source-map", &source_map).to_string() != record.source_map
            || output_hash("budget", &budget).to_string() != record.budget
            || interface
                .as_ref()
                .zip(record.interface.as_ref())
                .is_some_and(|(bytes, expected)| {
                    output_hash("interface", bytes).to_string() != *expected
                })
        {
            return None;
        }
        let artifact_hash = contract_code_hash(&artifact);
        if artifact_hash.to_string() != record.artifact_hash {
            return None;
        }
        let manifest_text = std::str::from_utf8(&manifest_bytes).ok()?;
        let manifest: ContractManifest = json::from_str(manifest_text).ok()?;
        if manifest.code_hash.as_ref() != Some(&artifact_hash) {
            return None;
        }
        if !valid_sidecar(&source_map, "source-map", &record.artifact_hash)
            || !valid_sidecar(&budget, "budget", &record.artifact_hash)
            || interface
                .as_ref()
                .is_some_and(|bytes| !valid_interface(bytes, &record.artifact_hash))
        {
            return None;
        }
        Some(BuildOutcome {
            status: BuildStatus::Fresh,
            artifact_hash,
            artifact,
            manifest,
            paths,
        })
    }

    fn finish_build(
        &self,
        output: CompileOutput,
        layout: &PublishLayout,
        input: &str,
        mode: PublishMode,
    ) -> Result<BuildOutcome, BuildError> {
        let artifact_hash = contract_code_hash(&output.artifact);
        if output.report.artifact_hash != artifact_hash {
            return Err(BuildError::Internal(
                "compiler report hash does not match deployable artifact".to_owned(),
            ));
        }
        if output.manifest.code_hash.as_ref() != Some(&artifact_hash) {
            return Err(BuildError::Internal(
                "compiler manifest hash does not match deployable artifact".to_owned(),
            ));
        }
        let artifact_hash_text = artifact_hash.to_string();
        let paths = layout.resolve(&artifact_hash_text, input);
        reject_path_collisions(paths.all_paths(), "resolved Kotodama build")?;
        let manifest = json::to_json_pretty(&output.manifest)
            .map_err(|error| BuildError::Render(error.to_string()))?;
        let source_map = output
            .report
            .render_source_map_json()
            .map_err(|error| BuildError::Render(error.to_string()))?;
        let budget = output
            .report
            .render_budget_json()
            .map_err(|error| BuildError::Render(error.to_string()))?;
        let interface = paths
            .interface
            .as_ref()
            .map(|_| render_interface_json(&output.manifest))
            .transpose()?;
        let record = BuildRecord {
            input: input.to_owned(),
            artifact_hash: artifact_hash_text,
            artifact: output_hash("artifact", &output.artifact).to_string(),
            manifest: output_hash("manifest", manifest.as_bytes()).to_string(),
            source_map: output_hash("source-map", source_map.as_bytes()).to_string(),
            budget: output_hash("budget", budget.as_bytes()).to_string(),
            interface: interface
                .as_ref()
                .map(|value| output_hash("interface", value.as_bytes()).to_string()),
        };
        let record_text = record.render();

        let mut expected = vec![
            (&paths.artifact, output.artifact.as_slice()),
            (&paths.manifest, manifest.as_bytes()),
            (&paths.source_map, source_map.as_bytes()),
            (&paths.budget, budget.as_bytes()),
        ];
        if let (Some(path), Some(value)) = (&paths.interface, interface.as_ref()) {
            expected.push((path, value.as_bytes()));
        }
        match mode {
            PublishMode::Write => {
                for (path, bytes) in &expected {
                    atomic_write_if_changed(path, bytes)?;
                }
                // This commit marker is deliberately last. An interrupted build
                // can leave files behind but can never create an authenticated hit.
                atomic_write_if_changed(&paths.record, record_text.as_bytes())?;
            }
            PublishMode::Verify => {
                for (path, bytes) in &expected {
                    verify_exact(path, bytes)?;
                }
                verify_exact(&paths.record, record_text.as_bytes())?;
            }
        }
        Ok(BuildOutcome {
            status: BuildStatus::Built,
            artifact_hash,
            artifact: output.artifact,
            manifest: output.manifest,
            paths,
        })
    }
}

/// Render the canonical generated interface used by developer tooling.
pub fn render_interface_json(manifest: &ContractManifest) -> Result<String, BuildError> {
    let manifest_value =
        json::to_value(manifest).map_err(|error| BuildError::Render(error.to_string()))?;
    let entrypoints = manifest
        .entrypoints
        .as_ref()
        .map(json::to_value)
        .transpose()
        .map_err(|error| BuildError::Render(error.to_string()))?
        .unwrap_or(json::Value::Array(Vec::new()));
    let states = manifest
        .states
        .as_ref()
        .map(json::to_value)
        .transpose()
        .map_err(|error| BuildError::Render(error.to_string()))?
        .unwrap_or(json::Value::Array(Vec::new()));
    json::to_json_pretty(&norito::json!({
        "interface_version": 1_u64,
        "manifest": (manifest_value),
        "entrypoints": (entrypoints),
        "states": (states),
    }))
    .map_err(|error| BuildError::Render(error.to_string()))
}

/// Read one bounded UTF-8 Kotodama source and map I/O failures for build tools.
pub fn read_source_file(path: &Path) -> Result<String, BuildError> {
    crate::source::read_source_file(path).map_err(|error| match error {
        crate::source::SourceReadError::Io(error) => BuildError::Io {
            operation: "read Kotodama source",
            path: path.to_path_buf(),
            message: error.to_string(),
        },
        crate::source::SourceReadError::TooLarge { limit } => BuildError::SourceTooLarge {
            path: path.to_path_buf(),
            limit,
        },
        crate::source::SourceReadError::InvalidUtf8 {
            valid_up_to,
            error_len,
        } => BuildError::InvalidSourceUtf8 {
            path: path.to_path_buf(),
            valid_up_to,
            error_len,
        },
    })
}

/// Atomically replace a generated file only when its bytes changed.
///
/// Returns `true` when publication occurred. Returning `false` performs no
/// directory creation, timestamp update, or temporary-file write.
pub fn atomic_write_if_changed(path: &Path, bytes: &[u8]) -> Result<bool, BuildError> {
    if file_equals(path, bytes).unwrap_or(false) {
        return Ok(false);
    }
    let parent = output_parent(path);
    fs::create_dir_all(parent).map_err(|error| BuildError::Io {
        operation: "create output directory",
        path: parent.to_path_buf(),
        message: error.to_string(),
    })?;
    let file_name = path
        .file_name()
        .ok_or_else(|| BuildError::InvalidPath {
            path: path.to_path_buf(),
            message: "output path has no file name".to_owned(),
        })?
        .to_string_lossy();
    let mut temporary = None;
    let mut file = None;
    for _ in 0..32 {
        let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            sequence
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(opened) => {
                temporary = Some(candidate);
                file = Some(opened);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(BuildError::Io {
                    operation: "create temporary output",
                    path: candidate,
                    message: error.to_string(),
                });
            }
        }
    }
    let temporary = temporary.ok_or_else(|| {
        BuildError::Internal(format!(
            "could not allocate a unique temporary file for {}",
            path.display()
        ))
    })?;
    let mut file = file.expect("temporary path and file are assigned together");
    let publication = (|| {
        file.write_all(bytes).map_err(|error| BuildError::Io {
            operation: "write temporary output",
            path: temporary.clone(),
            message: error.to_string(),
        })?;
        file.sync_all().map_err(|error| BuildError::Io {
            operation: "sync temporary output",
            path: temporary.clone(),
            message: error.to_string(),
        })?;
        drop(file);
        fs::rename(&temporary, path).map_err(|error| BuildError::Io {
            operation: "publish output",
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
        let directory = fs::File::open(parent).map_err(|error| BuildError::Io {
            operation: "open output directory",
            path: parent.to_path_buf(),
            message: error.to_string(),
        })?;
        directory.sync_all().map_err(|error| BuildError::Io {
            operation: "sync output directory",
            path: parent.to_path_buf(),
            message: error.to_string(),
        })
    })();
    if publication.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    publication.map(|()| true)
}

fn output_parent(path: &Path) -> &Path {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn read_bounded_file(path: &Path, limit: usize) -> Option<Vec<u8>> {
    let file = fs::File::open(path).ok()?;
    let declared = file.metadata().ok()?.len();
    let limit_u64 = u64::try_from(limit).unwrap_or(u64::MAX);
    if declared > limit_u64 {
        return None;
    }
    let capacity = usize::try_from(declared).ok()?.min(limit);
    let mut bytes = Vec::with_capacity(capacity);
    file.take(limit_u64.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    (bytes.len() <= limit).then_some(bytes)
}

fn file_equals(path: &Path, expected: &[u8]) -> std::io::Result<bool> {
    let mut file = fs::File::open(path)?;
    if file.metadata()?.len() != u64::try_from(expected.len()).unwrap_or(u64::MAX) {
        return Ok(false);
    }
    let mut offset = 0_usize;
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(offset == expected.len());
        }
        let end = offset.saturating_add(read);
        if expected.get(offset..end) != Some(&buffer[..read]) {
            return Ok(false);
        }
        offset = end;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BuildRecord {
    input: String,
    artifact_hash: String,
    artifact: String,
    manifest: String,
    source_map: String,
    budget: String,
    interface: Option<String>,
}

impl BuildRecord {
    fn render(&self) -> String {
        format!(
            "schema={BUILD_RECORD_SCHEMA}\ninput={}\nartifact_hash={}\nartifact={}\nmanifest={}\nsource_map={}\nbudget={}\ninterface={}\n",
            self.input,
            self.artifact_hash,
            self.artifact,
            self.manifest,
            self.source_map,
            self.budget,
            self.interface.as_deref().unwrap_or("-"),
        )
    }

    fn parse(raw: &str) -> Option<Self> {
        let mut fields = HashMap::new();
        for line in raw.lines() {
            let (key, value) = line.split_once('=')?;
            if !matches!(
                key,
                "schema"
                    | "input"
                    | "artifact_hash"
                    | "artifact"
                    | "manifest"
                    | "source_map"
                    | "budget"
                    | "interface"
            ) || value.is_empty()
                || fields.insert(key, value).is_some()
            {
                return None;
            }
        }
        if fields.len() != 8 || fields.get("schema") != Some(&BUILD_RECORD_SCHEMA) {
            return None;
        }
        let interface = *fields.get("interface")?;
        Some(Self {
            input: (*fields.get("input")?).to_owned(),
            artifact_hash: (*fields.get("artifact_hash")?).to_owned(),
            artifact: (*fields.get("artifact")?).to_owned(),
            manifest: (*fields.get("manifest")?).to_owned(),
            source_map: (*fields.get("source_map")?).to_owned(),
            budget: (*fields.get("budget")?).to_owned(),
            interface: (interface != "-").then(|| interface.to_owned()),
        })
    }
}

fn output_hash(kind: &str, bytes: &[u8]) -> Hash {
    let length = (bytes.len() as u64).to_le_bytes();
    Hash::new_from_chunks(&[
        b"kotodama-build-output-v1\0",
        kind.as_bytes(),
        b"\0",
        &length,
        bytes,
    ])
}

fn valid_sidecar(bytes: &[u8], kind: &str, artifact_hash: &str) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let Ok(value) = json::parse_value(text) else {
        return false;
    };
    value.get("kind").and_then(json::Value::as_str) == Some(kind)
        && value.get("artifact_hash").and_then(json::Value::as_str) == Some(artifact_hash)
}

fn valid_interface(bytes: &[u8], artifact_hash: &str) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let Ok(value) = json::parse_value(text) else {
        return false;
    };
    if value.get("interface_version").and_then(json::Value::as_u64) != Some(1) {
        return false;
    }
    let Some(manifest) = value.get("manifest").cloned() else {
        return false;
    };
    let Ok(manifest) = json::from_value::<ContractManifest>(manifest) else {
        return false;
    };
    manifest
        .code_hash
        .as_ref()
        .is_some_and(|hash| hash.to_string() == artifact_hash)
}

fn append_field(transcript: &mut Vec<u8>, value: &[u8]) {
    transcript.extend_from_slice(&(value.len() as u64).to_le_bytes());
    transcript.extend_from_slice(value);
}

fn validate_profile(profile: &str) -> Result<(), BuildError> {
    let mut chars = profile.chars();
    if !chars
        .next()
        .is_some_and(|value| value.is_ascii_alphanumeric())
        || !chars.all(|value| value.is_ascii_alphanumeric() || matches!(value, '-' | '_'))
    {
        return Err(BuildError::InvalidProfile(profile.to_owned()));
    }
    Ok(())
}

fn validate_stem(stem: &str) -> Result<(), BuildError> {
    if stem.is_empty()
        || matches!(stem, "." | "..")
        || Path::new(stem).components().count() != 1
        || Path::new(stem).file_name().and_then(|value| value.to_str()) != Some(stem)
    {
        return Err(BuildError::InvalidStem(stem.to_owned()));
    }
    Ok(())
}

fn reject_output_collisions(requests: &[SourceBuildRequest]) -> Result<(), BuildError> {
    let mut owners = HashMap::<PathBuf, String>::new();
    for request in requests {
        for path in request.layout.static_paths() {
            let normalized = normalize_path(&path)?;
            if let Some(previous) = owners.insert(normalized.clone(), request.source_name.clone()) {
                return Err(BuildError::OutputCollision {
                    path: normalized,
                    first: previous,
                    second: request.source_name.clone(),
                });
            }
        }
    }
    Ok(())
}

fn reject_layout_collisions(layout: &PublishLayout, owner: &str) -> Result<(), BuildError> {
    reject_path_collisions(layout.static_paths(), owner)
}

fn reject_path_collisions(
    paths: impl IntoIterator<Item = PathBuf>,
    owner: &str,
) -> Result<(), BuildError> {
    let mut seen = HashMap::<PathBuf, PathBuf>::new();
    for path in paths {
        let normalized = normalize_path(&path)?;
        if let Some(previous) = seen.insert(normalized.clone(), path.clone()) {
            return Err(BuildError::OutputCollision {
                path: normalized,
                first: format!("{owner}:{}", previous.display()),
                second: format!("{owner}:{}", path.display()),
            });
        }
    }
    Ok(())
}

fn normalize_path(path: &Path) -> Result<PathBuf, BuildError> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| BuildError::Io {
                operation: "resolve output path",
                path: path.to_path_buf(),
                message: error.to_string(),
            })?
            .join(path)
    };
    let normalized = normalize_lexically(&absolute);

    // Resolve the longest existing prefix so two lexical paths that traverse
    // different symlinked directories cannot evade parallel-output collision
    // checks. Nonexistent output suffixes are then appended without touching
    // the filesystem.
    let mut existing = normalized.as_path();
    let mut suffix = Vec::new();
    while !existing.exists() {
        let Some(name) = existing.file_name() else {
            break;
        };
        suffix.push(name.to_owned());
        let Some(parent) = existing.parent() else {
            break;
        };
        existing = parent;
    }
    let mut physical = fs::canonicalize(existing).map_err(|error| BuildError::Io {
        operation: "resolve output directory",
        path: existing.to_path_buf(),
        message: error.to_string(),
    })?;
    for component in suffix.into_iter().rev() {
        physical.push(component);
    }
    Ok(normalize_lexically(&physical))
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn verify_exact(path: &Path, expected: &[u8]) -> Result<(), BuildError> {
    match file_equals(path, expected) {
        Ok(true) => Ok(()),
        Ok(false) => Err(BuildError::LockedStale(path.to_path_buf())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(BuildError::LockedMissing(path.to_path_buf()))
        }
        Err(error) => Err(BuildError::Io {
            operation: "verify generated output",
            path: path.to_path_buf(),
            message: error.to_string(),
        }),
    }
}

/// Build driver failure.
#[derive(Clone, Debug)]
pub enum BuildError {
    /// A profile could escape or ambiguously name its target directory.
    InvalidProfile(String),
    /// A target stem was not one safe path component.
    InvalidStem(String),
    /// An output path could not be represented safely.
    InvalidPath {
        /// Rejected path.
        path: PathBuf,
        /// Reason for rejection.
        message: String,
    },
    /// Two roots attempted to publish the same static output.
    OutputCollision {
        /// Colliding normalized output path.
        path: PathBuf,
        /// First source owner.
        first: String,
        /// Second source owner.
        second: String,
    },
    /// Canonical compiler diagnostics.
    Compile(DiagnosticBundle),
    /// Parsing, resolution, or typed-HIR linking of a source graph failed.
    SourceGraph(SourceGraphError),
    /// A source exceeded the mandatory V1 byte limit.
    SourceTooLarge {
        /// Rejected source path.
        path: PathBuf,
        /// Maximum accepted source size.
        limit: usize,
    },
    /// A source contained malformed UTF-8.
    InvalidSourceUtf8 {
        /// Rejected source path.
        path: PathBuf,
        /// Number of valid bytes before the malformed sequence.
        valid_up_to: usize,
        /// Malformed-sequence length when it was fully present.
        error_len: Option<usize>,
    },
    /// JSON or sidecar rendering failed.
    Render(String),
    /// A locked output did not exist.
    LockedMissing(PathBuf),
    /// A locked output differed from its canonical bytes.
    LockedStale(PathBuf),
    /// Filesystem operation failed.
    Io {
        /// Short operation label.
        operation: &'static str,
        /// Affected path.
        path: PathBuf,
        /// Underlying error.
        message: String,
    },
    /// Compiler/driver invariant failure.
    Internal(String),
}

impl fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidProfile(profile) => write!(
                formatter,
                "invalid Kotodama build profile `{profile}`; use ASCII letters, digits, '-' or '_'"
            ),
            Self::InvalidStem(stem) => {
                write!(formatter, "invalid Kotodama output stem `{stem}`")
            }
            Self::InvalidPath { path, message } => {
                write!(
                    formatter,
                    "invalid output path `{}`: {message}",
                    path.display()
                )
            }
            Self::OutputCollision {
                path,
                first,
                second,
            } => write!(
                formatter,
                "Kotodama sources `{first}` and `{second}` both publish `{}`",
                path.display()
            ),
            Self::Compile(diagnostics) => diagnostics.render_human().fmt(formatter),
            Self::SourceGraph(error) => error.fmt(formatter),
            Self::SourceTooLarge { path, limit } => write!(
                formatter,
                "Kotodama source `{}` exceeds the {limit}-byte V1 limit",
                path.display()
            ),
            Self::InvalidSourceUtf8 {
                path,
                valid_up_to,
                error_len,
            } => match error_len {
                Some(length) => write!(
                    formatter,
                    "Kotodama source `{}` is not valid UTF-8 at byte {valid_up_to} (invalid sequence length {length})",
                    path.display()
                ),
                None => write!(
                    formatter,
                    "Kotodama source `{}` ends with an incomplete UTF-8 sequence at byte {valid_up_to}",
                    path.display()
                ),
            },
            Self::Render(message) => write!(formatter, "render Kotodama build output: {message}"),
            Self::LockedMissing(path) => {
                write!(
                    formatter,
                    "generated artifact is missing: {}",
                    path.display()
                )
            }
            Self::LockedStale(path) => {
                write!(formatter, "generated artifact is stale: {}", path.display())
            }
            Self::Io {
                operation,
                path,
                message,
            } => write!(formatter, "{operation} `{}`: {message}", path.display()),
            Self::Internal(message) => write!(formatter, "Kotodama build invariant: {message}"),
        }
    }
}

impl Error for BuildError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linker::{ImportBinding, SourceModuleUnit, SourcePackageUnit};
    use std::collections::BTreeSet;

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "kotodama-driver-{label}-{}-{}",
            std::process::id(),
            TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn request(root: &Path, source: &str) -> SourceBuildRequest {
        SourceBuildRequest {
            source: source.to_owned(),
            source_name: "contracts/demo.ko".to_owned(),
            profile: "test".to_owned(),
            layout: PublishLayout::standard(root, "test", "demo", true).expect("valid test layout"),
            mode: PublishMode::Write,
        }
    }

    fn linked_request(root: &Path, module_source: &str) -> LinkedSourceBuildRequest {
        LinkedSourceBuildRequest {
            graph: SourceLinkRequest {
                root: SourceModuleUnit {
                    source_name: "contracts/app.ko".to_owned(),
                    source: "seiyaku App { view fn run() -> i64 { return helpers::value(); } }"
                        .to_owned(),
                },
                imports: vec![ImportBinding {
                    alias: "helpers".to_owned(),
                    package: "std/math@1.0.0".to_owned(),
                }],
                packages: vec![SourcePackageUnit {
                    identity: "std/math@1.0.0".to_owned(),
                    modules: vec![SourceModuleUnit {
                        source_name: "modules/math.ko".to_owned(),
                        source: module_source.to_owned(),
                    }],
                    exports: BTreeSet::from(["value".to_owned()]),
                    imports: Vec::new(),
                }],
            },
            source_name: "contracts/app.ko".to_owned(),
            profile: "test".to_owned(),
            layout: PublishLayout::standard(root, "test", "app", true)
                .expect("valid linked test layout"),
            mode: PublishMode::Write,
        }
    }

    #[test]
    fn bounded_source_reader_preserves_typed_budget_and_utf8_failures() {
        let root = temp_root("source-errors");
        fs::create_dir_all(&root).expect("create source error root");
        let oversized = root.join("oversized.ko");
        fs::write(&oversized, vec![b' '; crate::source::MAX_SOURCE_BYTES + 1])
            .expect("write oversized source");
        assert!(matches!(
            read_source_file(&oversized),
            Err(BuildError::SourceTooLarge {
                limit: crate::source::MAX_SOURCE_BYTES,
                ..
            })
        ));

        let invalid = root.join("invalid.ko");
        fs::write(&invalid, [0xff]).expect("write invalid UTF-8 source");
        assert!(matches!(
            read_source_file(&invalid),
            Err(BuildError::InvalidSourceUtf8 {
                valid_up_to: 0,
                error_len: Some(1),
                ..
            })
        ));
        fs::remove_dir_all(root).expect("remove source error root");
    }

    #[test]
    fn cache_reader_rejects_sparse_oversized_files_without_allocating_them() {
        let root = temp_root("bounded-cache");
        fs::create_dir_all(&root).expect("create bounded cache root");
        let path = root.join("hostile-cache-output");
        let file = fs::File::create(&path).expect("create sparse cache output");
        file.set_len(
            u64::try_from(MAX_CACHED_OUTPUT_BYTES)
                .expect("cache limit fits u64")
                .saturating_add(1),
        )
        .expect("extend sparse cache output");
        drop(file);

        assert!(read_bounded_file(&path, MAX_CACHED_OUTPUT_BYTES).is_none());
        assert!(atomic_write_if_changed(&path, b"repaired").expect("replace hostile cache output"));
        assert_eq!(fs::read(&path).expect("read repaired output"), b"repaired");
        fs::remove_dir_all(root).expect("remove bounded cache root");
    }

    #[test]
    fn authenticated_noop_build_writes_nothing_and_tampering_rebuilds() {
        let root = temp_root("fresh");
        let source = "seiyaku Demo { view fn ping() -> i64 { return 1; } }";
        let driver = BuildDriver::new(CompilerSession::default(), "test-toolchain");
        let initial = driver
            .build_source(request(&root, source))
            .expect("initial build");
        assert_eq!(initial.status, BuildStatus::Built);
        let tracked = [
            initial.paths.artifact.clone(),
            initial.paths.manifest.clone(),
            initial.paths.source_map.clone(),
            initial.paths.budget.clone(),
            initial.paths.interface.clone().expect("interface"),
            initial.paths.record.clone(),
        ];
        let before = tracked
            .iter()
            .map(|path| fs::metadata(path).expect("output metadata").modified().ok())
            .collect::<Vec<_>>();
        let fresh = driver
            .build_source(request(&root, source))
            .expect("authenticated no-op");
        assert_eq!(fresh.status, BuildStatus::Fresh);
        let after = tracked
            .iter()
            .map(|path| fs::metadata(path).expect("output metadata").modified().ok())
            .collect::<Vec<_>>();
        assert_eq!(before, after, "a fresh build must not rewrite any output");

        for path in &tracked {
            let original = fs::read(path).expect("read generated output");
            fs::write(path, b"tampered").expect("tamper generated output");
            let rebuilt = driver
                .build_source(request(&root, source))
                .expect("tampered output must rebuild");
            assert_eq!(
                rebuilt.status,
                BuildStatus::Built,
                "tampered {}",
                path.display()
            );
            if *path == rebuilt.paths.record {
                assert_ne!(fs::read(path).expect("rebuilt record"), b"tampered");
            } else {
                assert_eq!(fs::read(path).expect("rebuilt output"), original);
            }
        }
        fs::remove_dir_all(root).expect("remove test build root");
    }

    #[test]
    fn authenticated_module_graph_hit_with_fresh_driver_performs_zero_work_or_writes() {
        let root = temp_root("linked-fresh");
        let graph = ModuleBuildGraph::default();
        let driver = BuildDriver::new(CompilerSession::default(), "test-toolchain");
        let module_v1 = "module Math { fn value() -> i64 { return 1; } }";

        let first = driver
            .build_linked_source(&graph, linked_request(&root, module_v1))
            .expect("initial linked build");
        assert_eq!(first.status, BuildStatus::Built);
        assert_eq!(graph.parse_attempt_count(), 2);
        assert_eq!(graph.link_attempt_count(), 1);
        let tracked = [
            first.paths.artifact.clone(),
            first.paths.manifest.clone(),
            first.paths.source_map.clone(),
            first.paths.budget.clone(),
            first.paths.interface.clone().expect("interface"),
            first.paths.record.clone(),
        ];
        let before = tracked
            .iter()
            .map(|path| fs::metadata(path).expect("output metadata").modified().ok())
            .collect::<Vec<_>>();

        let fresh_graph = ModuleBuildGraph::default();
        let fresh_driver = BuildDriver::new(CompilerSession::default(), "test-toolchain");
        let fresh = fresh_driver
            .build_linked_source(&fresh_graph, linked_request(&root, module_v1))
            .expect("authenticated linked no-op");
        assert_eq!(fresh.status, BuildStatus::Fresh);
        assert_eq!(
            fresh_graph.parse_attempt_count(),
            0,
            "a fresh process-local graph must not parse an authenticated output hit",
        );
        assert_eq!(
            fresh_graph.link_attempt_count(),
            0,
            "a fresh driver must return before typed-HIR linking or compilation",
        );
        let after = tracked
            .iter()
            .map(|path| fs::metadata(path).expect("output metadata").modified().ok())
            .collect::<Vec<_>>();
        assert_eq!(
            before, after,
            "an authenticated module-graph hit must rewrite none of its six outputs",
        );

        let changed = driver
            .build_linked_source(
                &graph,
                linked_request(&root, "module Math { fn value() -> i64 { return 2; } }"),
            )
            .expect("changed module rebuild");
        assert_eq!(changed.status, BuildStatus::Built);
        assert_eq!(
            graph.parse_attempt_count(),
            3,
            "only the changed module should require a new parse",
        );
        assert_eq!(graph.link_attempt_count(), 2);
        assert_ne!(first.artifact_hash, changed.artifact_hash);
        fs::remove_dir_all(root).expect("remove linked build root");
    }

    #[test]
    fn sidecar_manifest_avoids_an_unrequested_sibling_and_remains_cacheable() {
        let root = temp_root("sidecar-manifest");
        let source = "seiyaku Demo { view fn ping() -> i64 { return 1; } }";
        let driver = BuildDriver::new(CompilerSession::default(), "test-toolchain");
        let mut build = request(&root, source);
        let sibling_manifest = build.layout.manifest.clone();
        build.layout = build.layout.with_sidecar_manifest();

        let first = driver.build_source(build.clone()).expect("initial build");
        assert_eq!(first.status, BuildStatus::Built);
        assert!(!sibling_manifest.exists());
        assert!(first.paths.manifest.is_file());
        assert!(first.paths.manifest.to_string_lossy().contains(".sidecars"));

        let second = driver.build_source(build).expect("authenticated no-op");
        assert_eq!(second.status, BuildStatus::Fresh);
        assert_eq!(second.paths.manifest, first.paths.manifest);
        assert!(!sibling_manifest.exists());
        fs::remove_dir_all(root).expect("remove sidecar manifest root");
    }

    #[test]
    fn record_parser_rejects_duplicates_unknown_fields_and_truncation() {
        let valid = BuildRecord {
            input: "input".to_owned(),
            artifact_hash: "code".to_owned(),
            artifact: "artifact".to_owned(),
            manifest: "manifest".to_owned(),
            source_map: "source".to_owned(),
            budget: "budget".to_owned(),
            interface: None,
        }
        .render();
        assert!(BuildRecord::parse(&valid).is_some());
        assert!(BuildRecord::parse(&format!("{valid}input=again\n")).is_none());
        assert!(BuildRecord::parse(&format!("{valid}unknown=value\n")).is_none());
        assert!(BuildRecord::parse("schema=kotodama-build-v1\ninput=x\n").is_none());
    }

    #[test]
    fn batch_rejects_lexically_colliding_outputs_before_building() {
        let root = temp_root("collision");
        let source = "seiyaku Demo { view fn ping() -> i64 { return 1; } }";
        let first = request(&root, source);
        let mut second = request(&root, source);
        second.source_name = "contracts/other.ko".to_owned();
        second.layout.artifact = root.join("test/sub/../demo.to");
        let driver = BuildDriver::new(CompilerSession::default(), "test-toolchain");
        let error = driver
            .build_source_batch(vec![first, second])
            .expect_err("colliding output roots must fail preflight");
        assert!(matches!(error, BuildError::OutputCollision { .. }));
        assert!(!root.exists(), "preflight failure must not publish outputs");
    }

    #[test]
    fn direct_build_rejects_colliding_artifact_and_manifest_paths() {
        let root = temp_root("direct-collision");
        let output = root.join("demo.to");
        let mut build = request(
            &root,
            "seiyaku Demo { view fn ping() -> i64 { return 1; } }",
        );
        build.layout = PublishLayout::for_artifact(output.clone(), Some(output.clone()), None)
            .expect("construct adversarial layout");
        let driver = BuildDriver::new(CompilerSession::default(), "test-toolchain");
        let error = driver
            .build_source(build)
            .expect_err("one path cannot contain both artifact and manifest bytes");
        assert!(matches!(error, BuildError::OutputCollision { .. }));
        assert!(!output.exists());
    }

    #[cfg(unix)]
    #[test]
    fn batch_rejects_output_aliases_through_symlinked_directories() {
        use std::os::unix::fs::symlink;

        let root = temp_root("symlink-collision");
        let real = root.join("real");
        let alias = root.join("alias");
        fs::create_dir_all(&real).expect("create real output directory");
        symlink(&real, &alias).expect("create output directory alias");
        let source = "seiyaku Demo { view fn ping() -> i64 { return 1; } }";
        let mut first = request(&real, source);
        first.source_name = "contracts/first.ko".to_owned();
        let mut second = request(&alias, source);
        second.source_name = "contracts/second.ko".to_owned();
        let driver = BuildDriver::new(CompilerSession::default(), "test-toolchain");
        let error = driver
            .build_source_batch(vec![first, second])
            .expect_err("physical output aliases must fail before parallel publication");
        assert!(matches!(error, BuildError::OutputCollision { .. }));
        assert!(!real.join("test/demo.to").exists());
        fs::remove_dir_all(root).expect("remove symlink collision root");
    }

    #[test]
    fn profile_and_policy_are_cache_dimensions() {
        let root = temp_root("policy");
        let source = "seiyaku Demo { view fn ping() -> i64 { return 1; } }";
        let plain = BuildDriver::new(CompilerSession::default(), "test-toolchain");
        let mut zk_options = crate::compiler::CompilerOptions::default();
        zk_options.force_zk = true;
        let zk = BuildDriver::new(CompilerSession::new(zk_options), "test-toolchain");
        let request = request(&root, source);
        let plain_key = plain.input_fingerprint(
            b"source",
            &request.source_name,
            &request.profile,
            request.source.as_bytes(),
        );
        let zk_key = zk.input_fingerprint(
            b"source",
            &request.source_name,
            &request.profile,
            request.source.as_bytes(),
        );
        assert_ne!(plain_key, zk_key);
        let release_key = plain.input_fingerprint(
            b"source",
            &request.source_name,
            "release",
            request.source.as_bytes(),
        );
        assert_ne!(plain_key, release_key);
        assert!(matches!(
            PublishLayout::standard(&root, "../escape", "demo", false),
            Err(BuildError::InvalidProfile(_))
        ));
    }

    #[test]
    fn atomic_writer_does_not_replace_equal_file() {
        let root = temp_root("atomic");
        let path = root.join("value.bin");
        assert!(atomic_write_if_changed(&path, b"same").expect("initial write"));
        assert!(!atomic_write_if_changed(&path, b"same").expect("no-op write"));
        fs::remove_dir_all(root).expect("remove atomic test root");
    }

    #[test]
    fn atomic_writer_supports_relative_leaf_outputs() {
        let path = PathBuf::from(format!(
            ".kotodama-relative-output-{}-{}",
            std::process::id(),
            TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed),
        ));
        assert_eq!(output_parent(&path), Path::new("."));
        assert!(atomic_write_if_changed(&path, b"relative").expect("publish relative output"));
        assert_eq!(fs::read(&path).expect("read relative output"), b"relative");
        fs::remove_file(path).expect("remove relative output");
    }
}
