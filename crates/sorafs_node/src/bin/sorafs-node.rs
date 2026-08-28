//! Offline developer CLI helpers for inspecting the SoraFS storage backend.
use iroha_config::base::util::Bytes;
use iroha_data_model::{
    account::{AccountId, address::AccountAddress},
    peer::PeerId,
};
use norito::json::{self, Map, Value};
use sorafs_car::{
    CAR_PLAN_MAX_CHUNKS, CarBuildPlan, CarChunk, CarStreamingWriter, ChunkStore, DirectoryPayload,
    FilePayload, FilePlan, PayloadSource, chunker_registry, compute_chunk_plan_digest_sha3,
    fetch_plan::try_chunk_fetch_plan_to_json,
};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, MAX_MANIFEST_ENCODED_BYTES, ManifestV1, PinPolicyConstraints,
    decode_manifest_v1_canonical,
    operator_preseed::{
        OPERATOR_PRESEED_SESSION_MAX_ARTIFACTS_V1, OPERATOR_PRESEED_SESSION_MAX_STORES_V1,
        OPERATOR_PRESEED_SESSION_RECEIPT_VERSION_V1, OperatorPreseedArtifactReceiptV1,
        OperatorPreseedSessionReceiptV1, OperatorPreseedTargetReceiptV1,
    },
    por::{
        AUDIT_VERDICT_MAX_CANONICAL_BYTES_V1, AuditOutcomeV1, AuditVerdictV1,
        POR_CHALLENGE_MAX_CANONICAL_BYTES_V1, POR_PROOF_MAX_CANONICAL_BYTES_V1, PorChallengeV1,
        PorProofV1, decode_audit_verdict_v1, decode_por_challenge_v1, decode_por_proof_v1,
    },
    validate_manifest,
};
use sorafs_node::{
    NodeHandle, PorVerdictOutcome,
    config::StorageConfig,
    operator_preseed::{
        install_operator_preseed_store_receipt_staging, operator_preseed_store_receipt_dir,
        operator_preseed_store_receipt_path, preflight_operator_preseed_store_receipt,
        read_operator_preseed_store_receipt, recover_operator_preseed_store_receipt_staging,
    },
    store::StorageBackend,
};
#[cfg(unix)]
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt};
use std::{
    collections::BTreeSet,
    env, fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process,
};
const OFFLINE_DIRECTORY_STREAM_BUFFER_BYTES: usize = 1024 * 1024;
const OFFLINE_DIRECTORY_MAX_PAYLOAD_BYTES: u64 = 16 * 1024 * 1024 * 1024;
const OFFLINE_DIRECTORY_MAX_ENTRIES: usize = 4096;
const OFFLINE_DIRECTORY_MAX_DEPTH: usize = 64;
fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return Ok(());
    };
    match command.as_str() {
        "ingest" => ingest_command(args.collect()),
        "preseed-session" => preseed_session_command(args.collect()),
        "export" => export_command(args.collect()),
        "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("unknown command: {other}")),
    }
}
fn print_usage() {
    eprintln!(
        "Usage: sorafs-node <command> [options]\n\n\
         Commands:\n  \
         ingest --data-dir=<dir> --max-capacity-bytes=<bytes> --manifest=<path> (--payload=<path>|--payload-dir=<dir>) [--plan-json-out=<path>]\n  \
         preseed-session --target=<validator-account-id>,<peer-id>,<data-dir>... --max-capacity-bytes=<bytes> [--verify-only] (--manifest=<path> (--payload=<path>|--payload-dir=<dir>))...\n  \
         ingest por --data-dir=<dir> --challenge=<path> --proof=<path> [--verdict=<path>] [--manifest-id=<hex>] [--json-out=<path>]\n  \
         export --data-dir=<dir> --manifest-id=<hex> --manifest-out=<path> --payload-out=<path> [--plan-json-out=<path>]\n  \
         --help, -h   Show this help message"
    );
}
fn print_por_usage() {
    eprintln!(
        "Usage: sorafs-node ingest por --data-dir=<dir> --challenge=<path> --proof=<path> [--verdict=<path>] [--manifest-id=<hex>] [--json-out=<path>]\n\n\
         Offline replay helper: verifies embedded signatures and lifecycle binding, but does not establish provider admission, trusted-auditor membership, or beacon/VRF provenance. Failed verdicts remain in the node's durable repair outbox; production mutation and repair reconciliation must use Torii's authenticated lifecycle."
    );
}
#[derive(Default)]
struct IngestOptions {
    data_dir: Option<PathBuf>,
    max_capacity_bytes: Option<u64>,
    manifest_path: Option<PathBuf>,
    payload_path: Option<PathBuf>,
    payload_dir: Option<PathBuf>,
    plan_json_out: Option<PathBuf>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
enum IngestPayloadSource {
    File(PathBuf),
    Directory(PathBuf),
}
fn ingest_command(mut args: Vec<String>) -> Result<(), String> {
    if args.first().is_some_and(|first| first == "por") {
        args.remove(0);
        return ingest_por_command(args);
    }
    let mut opts = IngestOptions::default();
    for arg in args {
        if let Some(rest) = arg.strip_prefix("--data-dir=") {
            opts.data_dir = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--max-capacity-bytes=") {
            if opts.max_capacity_bytes.is_some() {
                return Err("duplicate option --max-capacity-bytes".to_owned());
            }
            opts.max_capacity_bytes =
                Some(parse_nonzero_canonical_u64(rest, "--max-capacity-bytes")?);
        } else if let Some(rest) = arg.strip_prefix("--manifest=") {
            opts.manifest_path = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--payload=") {
            opts.payload_path = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--payload-dir=") {
            opts.payload_dir = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--plan-json-out=") {
            opts.plan_json_out = Some(PathBuf::from(rest));
        } else {
            return Err(format!("unknown option: {arg}"));
        }
    }
    let data_dir = opts
        .data_dir
        .ok_or_else(|| "missing required option --data-dir".to_string())?;
    let max_capacity_bytes = opts
        .max_capacity_bytes
        .ok_or_else(|| "missing required option --max-capacity-bytes".to_string())?;
    let manifest_path = opts
        .manifest_path
        .ok_or_else(|| "missing required option --manifest".to_string())?;
    let payload_source = require_ingest_payload_source(opts.payload_path, opts.payload_dir)?;
    ingest(
        data_dir,
        max_capacity_bytes,
        manifest_path,
        payload_source,
        opts.plan_json_out,
    )
}
fn parse_nonzero_canonical_u64(value: &str, option: &str) -> Result<u64, String> {
    if value.is_empty()
        || value == "0"
        || value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!(
            "{option} must be a nonzero canonical unsigned decimal"
        ));
    }
    value
        .parse::<u64>()
        .map_err(|_| format!("{option} must fit in an unsigned 64-bit integer"))
}
fn offline_ingest_storage_config(data_dir: PathBuf, max_capacity_bytes: u64) -> StorageConfig {
    StorageConfig::builder()
        .enabled(true)
        .data_dir(data_dir)
        .max_capacity_bytes(Bytes(max_capacity_bytes))
        .build()
}
fn enforce_ingest_capacity(content_length: u64, max_capacity_bytes: u64) -> Result<(), String> {
    if content_length > max_capacity_bytes {
        return Err(format!(
            "manifest payload length {content_length} exceeds --max-capacity-bytes={max_capacity_bytes}"
        ));
    }
    Ok(())
}
fn open_exact_file_payload(path: &Path, expected_length: u64) -> Result<FilePayload, String> {
    let mut source = FilePayload::open(path).map_err(|error| {
        format!(
            "failed to open stable no-follow payload file {}: {error}",
            path.display()
        )
    })?;
    PayloadSource::ensure_exhausted(&mut source, expected_length).map_err(|error| {
        format!(
            "payload file {} does not match manifest content length {expected_length}: {error}",
            path.display()
        )
    })?;
    Ok(source)
}
fn require_ingest_payload_source(
    payload_path: Option<PathBuf>,
    payload_dir: Option<PathBuf>,
) -> Result<IngestPayloadSource, String> {
    match (payload_path, payload_dir) {
        (Some(path), None) => Ok(IngestPayloadSource::File(path)),
        (None, Some(path)) => Ok(IngestPayloadSource::Directory(path)),
        (None, None) => Err(
            "missing payload source: exactly one of --payload or --payload-dir is required"
                .to_owned(),
        ),
        (Some(_), Some(_)) => Err("--payload and --payload-dir are mutually exclusive".to_owned()),
    }
}

#[derive(Default)]
struct PreseedSessionOptions {
    targets: Vec<PreseedSessionTargetOptions>,
    max_capacity_bytes: Option<u64>,
    verify_only: bool,
    artifacts: Vec<PreseedArtifactOptions>,
    pending_manifest: Option<PathBuf>,
}

struct PreseedSessionTargetOptions {
    validator_account_literal: String,
    validator_account_id: AccountId,
    peer_id: String,
    data_dir: PathBuf,
}

struct CanonicalPreseedSessionTarget {
    validator_account_literal: String,
    peer_id: String,
    store_root: PathBuf,
}

struct PreseedArtifactOptions {
    manifest_path: PathBuf,
    payload_source: IngestPayloadSource,
}

struct PreparedPreseedArtifact {
    manifest: ManifestV1,
    manifest_bytes: Vec<u8>,
    plan: CarBuildPlan,
    payload_source: IngestPayloadSource,
}

fn preseed_session_command(args: Vec<String>) -> Result<(), String> {
    let mut options = PreseedSessionOptions::default();
    for arg in args {
        if let Some(rest) = arg.strip_prefix("--target=") {
            if options.targets.len() >= OPERATOR_PRESEED_SESSION_MAX_STORES_V1 {
                return Err(format!(
                    "preseed-session admits at most {OPERATOR_PRESEED_SESSION_MAX_STORES_V1} --target values"
                ));
            }
            options.targets.push(parse_preseed_session_target(rest)?);
        } else if let Some(rest) = arg.strip_prefix("--max-capacity-bytes=") {
            if options.max_capacity_bytes.is_some() {
                return Err("duplicate option --max-capacity-bytes".to_owned());
            }
            options.max_capacity_bytes =
                Some(parse_nonzero_canonical_u64(rest, "--max-capacity-bytes")?);
        } else if arg == "--verify-only" {
            if options.verify_only {
                return Err("duplicate option --verify-only".to_owned());
            }
            options.verify_only = true;
        } else if let Some(rest) = arg.strip_prefix("--manifest=") {
            if options
                .pending_manifest
                .replace(PathBuf::from(rest))
                .is_some()
            {
                return Err("each --manifest must be followed by one payload source".to_owned());
            }
        } else if let Some(rest) = arg.strip_prefix("--payload=") {
            push_preseed_artifact_source(
                &mut options,
                IngestPayloadSource::File(PathBuf::from(rest)),
            )?;
        } else if let Some(rest) = arg.strip_prefix("--payload-dir=") {
            push_preseed_artifact_source(
                &mut options,
                IngestPayloadSource::Directory(PathBuf::from(rest)),
            )?;
        } else {
            return Err(format!("unknown option: {arg}"));
        }
    }
    if options.pending_manifest.is_some() {
        return Err("final --manifest is missing its payload source".to_owned());
    }
    let max_capacity_bytes = options
        .max_capacity_bytes
        .ok_or_else(|| "missing required option --max-capacity-bytes".to_owned())?;
    if options.targets.is_empty() {
        return Err("preseed-session requires at least one --target".to_owned());
    }
    if options.artifacts.is_empty() {
        return Err("preseed-session requires at least one manifest/payload pair".to_owned());
    }
    run_preseed_session(
        options.targets,
        max_capacity_bytes,
        options.artifacts,
        options.verify_only,
    )
}

fn parse_preseed_session_target(value: &str) -> Result<PreseedSessionTargetOptions, String> {
    let mut fields = value.splitn(3, ',');
    let validator_account_literal = fields
        .next()
        .filter(|field| !field.is_empty())
        .ok_or_else(|| "--target must be <validator-account-id>,<peer-id>,<data-dir>".to_owned())?;
    if validator_account_literal.trim() != validator_account_literal {
        return Err(
            "--target validator account must not contain surrounding whitespace".to_owned(),
        );
    }
    let validator_account_id = AccountAddress::parse_encoded(validator_account_literal, None)
        .map_err(|error| format!("invalid --target validator account: {error}"))?
        .to_account_id()
        .map_err(|error| format!("invalid --target validator account: {error}"))?;
    validator_account_id.try_signatory().ok_or_else(|| {
        "invalid --target validator account: expected a single-signatory account".to_owned()
    })?;
    let peer_id = fields
        .next()
        .filter(|field| !field.is_empty())
        .ok_or_else(|| "--target must be <validator-account-id>,<peer-id>,<data-dir>".to_owned())?
        .to_owned();
    let parsed_peer_id = peer_id
        .parse::<PeerId>()
        .map_err(|error| format!("invalid --target peer id: {error}"))?;
    if parsed_peer_id.to_string() != peer_id {
        return Err("--target peer id must use its exact canonical V1 spelling".to_owned());
    }
    let data_dir = fields
        .next()
        .filter(|field| !field.is_empty())
        .ok_or_else(|| "--target must be <validator-account-id>,<peer-id>,<data-dir>".to_owned())?;
    Ok(PreseedSessionTargetOptions {
        validator_account_literal: validator_account_literal.to_owned(),
        validator_account_id,
        peer_id,
        data_dir: PathBuf::from(data_dir),
    })
}

fn push_preseed_artifact_source(
    options: &mut PreseedSessionOptions,
    payload_source: IngestPayloadSource,
) -> Result<(), String> {
    if options.artifacts.len() >= OPERATOR_PRESEED_SESSION_MAX_ARTIFACTS_V1 {
        return Err(format!(
            "preseed-session admits at most {OPERATOR_PRESEED_SESSION_MAX_ARTIFACTS_V1} artifacts"
        ));
    }
    let manifest_path = options
        .pending_manifest
        .take()
        .ok_or_else(|| "payload source must follow one --manifest".to_owned())?;
    options.artifacts.push(PreseedArtifactOptions {
        manifest_path,
        payload_source,
    });
    Ok(())
}

fn canonical_preseed_path(path: &Path, label: &str, directory: bool) -> Result<PathBuf, String> {
    if !path.is_absolute() {
        return Err(format!(
            "{label} must be an absolute path: {}",
            path.display()
        ));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to inspect {label} {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink()
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        let kind = if directory { "directory" } else { "file" };
        return Err(format!(
            "{label} {} must be one existing real {kind}",
            path.display()
        ));
    }
    fs::canonicalize(path)
        .map_err(|error| format!("failed to canonicalize {label} {}: {error}", path.display()))
}

fn canonical_preseed_targets(
    targets: Vec<PreseedSessionTargetOptions>,
) -> Result<Vec<CanonicalPreseedSessionTarget>, String> {
    let mut canonical_targets = Vec::with_capacity(targets.len());
    let mut distinct = BTreeSet::new();
    let mut validators = BTreeSet::new();
    let mut peers = BTreeSet::new();
    for target in targets {
        if !validators.insert(target.validator_account_id.clone())
            || !peers.insert(target.peer_id.clone())
        {
            return Err(
                "preseed-session --target validator and peer identities must each be distinct"
                    .to_owned(),
            );
        }
        let root = canonical_preseed_path(&target.data_dir, "--target data-dir", true)?;
        if !distinct.insert(root.clone()) {
            return Err(format!(
                "preseed-session --target roots must be distinct; {} is repeated",
                root.display()
            ));
        }
        if let Some(existing) =
            canonical_targets
                .iter()
                .find(|existing: &&CanonicalPreseedSessionTarget| {
                    root.starts_with(existing.store_root.as_path())
                        || existing.store_root.starts_with(&root)
                })
        {
            return Err(format!(
                "preseed-session --target roots must not overlap: {} and {}",
                existing.store_root.display(),
                root.display()
            ));
        }
        canonical_targets.push(CanonicalPreseedSessionTarget {
            validator_account_literal: target.validator_account_literal,
            peer_id: target.peer_id,
            store_root: root,
        });
    }
    canonical_targets.sort_by(|left, right| {
        (
            left.validator_account_literal.as_str(),
            left.peer_id.as_str(),
            left.store_root.as_path(),
        )
            .cmp(&(
                right.validator_account_literal.as_str(),
                right.peer_id.as_str(),
                right.store_root.as_path(),
            ))
    });
    Ok(canonical_targets)
}

fn prepare_preseed_artifact(
    options: PreseedArtifactOptions,
    max_capacity_bytes: u64,
) -> Result<PreparedPreseedArtifact, String> {
    let manifest_path = canonical_preseed_path(&options.manifest_path, "--manifest", false)?;
    let manifest_bytes =
        read_bounded_por_file(&manifest_path, "manifest", MAX_MANIFEST_ENCODED_BYTES)?;
    let manifest = decode_manifest_v1_canonical(&manifest_bytes)
        .map_err(|error| format!("failed to parse manifest: {error}"))?;
    validate_manifest(
        &manifest,
        &PinPolicyConstraints {
            require_council_signatures: true,
            ..PinPolicyConstraints::default()
        },
    )
    .map_err(|error| format!("failed to validate manifest: {error}"))?;
    enforce_ingest_capacity(manifest.content_length, max_capacity_bytes)?;
    let chunk_profile = chunk_profile_from_manifest(&manifest)?;
    let (payload_source, plan) = match options.payload_source {
        IngestPayloadSource::File(path) => {
            let path = canonical_preseed_path(&path, "--payload", false)?;
            let mut source = open_exact_file_payload(&path, manifest.content_length)?;
            let plan =
                build_streaming_file_plan(&mut source, manifest.content_length, chunk_profile)
                    .map_err(|error| {
                        format!(
                            "failed to build exact preseed file plan from {}: {error}",
                            path.display()
                        )
                    })?;
            ensure_streaming_manifest_alignment(&manifest, &plan, &mut source, "file")?;
            (IngestPayloadSource::File(path), plan)
        }
        IngestPayloadSource::Directory(path) => {
            let path = canonical_preseed_path(&path, "--payload-dir", true)?;
            let plan = build_streaming_directory_plan(&path, chunk_profile).map_err(|error| {
                format!(
                    "failed to build exact preseed directory plan from {}: {error}",
                    path.display()
                )
            })?;
            ensure_streaming_directory_manifest_alignment(&manifest, &plan, &path)?;
            (IngestPayloadSource::Directory(path), plan)
        }
    };
    Ok(PreparedPreseedArtifact {
        manifest,
        manifest_bytes,
        plan,
        payload_source,
    })
}

fn ingest_or_verify_preseed_artifact<P: PayloadSource>(
    backend: &StorageBackend,
    artifact: &PreparedPreseedArtifact,
    source: &mut P,
    allow_ingest: bool,
) -> Result<(), String> {
    let manifest_digest = artifact
        .manifest
        .digest()
        .map_err(|error| format!("failed to digest preseed manifest: {error}"))?;
    if backend
        .manifest_by_digest(manifest_digest.as_bytes())
        .is_none()
    {
        if !allow_ingest {
            return Err(format!(
                "verify-only preseed store {} is missing the expected manifest",
                backend.root_dir().display()
            ));
        }
        let mut reader = OfflineSequentialPayloadReader::new(source, artifact.plan.content_length);
        backend
            .ingest_manifest(&artifact.manifest, &artifact.plan, &mut reader)
            .map_err(|error| format!("failed to ingest exact preseed artifact: {error}"))?;
        reader.finish()?;
    }
    verify_exact_preseed_artifact(backend, artifact, source)
}

fn verify_exact_preseed_artifact<P: PayloadSource>(
    backend: &StorageBackend,
    artifact: &PreparedPreseedArtifact,
    source: &mut P,
) -> Result<(), String> {
    let manifest_digest = artifact
        .manifest
        .digest()
        .map_err(|error| format!("failed to digest preseed manifest: {error}"))?;
    if manifest_digest.as_bytes() != blake3::hash(&artifact.manifest_bytes).as_bytes() {
        return Err("preseed manifest bytes and canonical digest disagree".to_owned());
    }
    let stored = backend
        .manifest_by_digest(manifest_digest.as_bytes())
        .ok_or_else(|| {
            format!(
                "preseed store {} is missing the expected manifest after ingest",
                backend.root_dir().display()
            )
        })?;
    if stored
        .load_manifest_bytes()
        .map_err(|error| format!("failed to re-read stored preseed manifest: {error}"))?
        != artifact.manifest_bytes
        || stored.manifest_cid() != artifact.manifest.root_cid.as_slice()
        || stored.payload_digest() != artifact.plan.payload_digest.as_bytes()
        || stored.content_length() != artifact.plan.content_length
        || stored.chunk_count() != artifact.plan.chunks.len()
        || stored.files().len() != artifact.plan.files.len()
    {
        return Err(format!(
            "preseed store {} retained different manifest, CID, payload, chunk, or file geometry",
            backend.root_dir().display()
        ));
    }
    let mut file_offset = 0_u64;
    for (stored_file, planned_file) in stored.files().iter().zip(&artifact.plan.files) {
        if stored_file.path != planned_file.path
            || stored_file.offset != file_offset
            || stored_file.size != planned_file.size
            || stored_file.first_chunk != planned_file.first_chunk
            || stored_file.chunk_count != planned_file.chunk_count
        {
            return Err(format!(
                "preseed store {} retained different logical-file geometry",
                backend.root_dir().display()
            ));
        }
        file_offset = file_offset
            .checked_add(planned_file.size)
            .ok_or_else(|| "preseed logical-file offset overflow".to_owned())?;
    }
    if file_offset != artifact.plan.content_length {
        return Err("preseed logical files do not cover the exact payload".to_owned());
    }
    for (index, planned_chunk) in artifact.plan.chunks.iter().enumerate() {
        let stored_chunk = stored.chunk(index).ok_or_else(|| {
            format!(
                "preseed store {} omitted chunk {index}",
                backend.root_dir().display()
            )
        })?;
        if stored_chunk.offset != planned_chunk.offset
            || stored_chunk.length != planned_chunk.length
            || stored_chunk.digest != planned_chunk.digest
        {
            return Err(format!(
                "preseed store {} retained different chunk {index} metadata",
                backend.root_dir().display()
            ));
        }
        let mut expected = vec![
            0_u8;
            usize::try_from(planned_chunk.length).map_err(|_| {
                "preseed chunk length exceeds host width".to_owned()
            })?
        ];
        PayloadSource::read_exact(source, planned_chunk.offset, &mut expected)
            .map_err(|error| format!("failed to read exact preseed chunk {index}: {error}"))?;
        let actual = backend
            .read_payload_range(stored.manifest_id(), planned_chunk.offset, expected.len())
            .map_err(|error| format!("failed to re-read stored preseed chunk {index}: {error}"))?;
        if actual != expected {
            return Err(format!(
                "preseed store {} retained different bytes at chunk {index}",
                backend.root_dir().display()
            ));
        }
    }
    source
        .ensure_exhausted(artifact.plan.content_length)
        .map_err(|error| format!("failed to revalidate exact preseed source length: {error}"))?;
    Ok(())
}

fn ingest_preseed_artifact_into_store(
    backend: &StorageBackend,
    artifact: &PreparedPreseedArtifact,
    allow_ingest: bool,
) -> Result<(), String> {
    match &artifact.payload_source {
        IngestPayloadSource::File(path) => {
            let mut source = open_exact_file_payload(path, artifact.plan.content_length)?;
            ingest_or_verify_preseed_artifact(backend, artifact, &mut source, allow_ingest)
        }
        IngestPayloadSource::Directory(path) => {
            let mut source = DirectoryPayload::new(path, &artifact.plan.files)
                .map_err(|error| format!("failed to open exact preseed directory: {error}"))?;
            ingest_or_verify_preseed_artifact(backend, artifact, &mut source, allow_ingest)
        }
    }
}

fn validate_distinct_preseed_artifacts(
    artifacts: &[PreparedPreseedArtifact],
) -> Result<(), String> {
    let mut manifest_digests = BTreeSet::new();
    for artifact in artifacts {
        let digest = artifact
            .manifest
            .digest()
            .map_err(|error| format!("failed to digest preseed manifest: {error}"))?;
        if !manifest_digests.insert(*digest.as_bytes()) {
            return Err(
                "preseed-session artifact manifest digests must be distinct; duplicate manifest/payload pairs are not canonical"
                    .to_owned(),
            );
        }
    }
    Ok(())
}

fn run_preseed_session(
    target_options: Vec<PreseedSessionTargetOptions>,
    max_capacity_bytes: u64,
    artifact_options: Vec<PreseedArtifactOptions>,
    verify_only: bool,
) -> Result<(), String> {
    let targets = canonical_preseed_targets(target_options)?;
    let artifacts = artifact_options
        .into_iter()
        .map(|artifact| prepare_preseed_artifact(artifact, max_capacity_bytes))
        .collect::<Result<Vec<_>, _>>()?;
    validate_distinct_preseed_artifacts(&artifacts)?;
    let mut stores = Vec::with_capacity(targets.len());
    for target in &targets {
        let root = &target.store_root;
        let backend = StorageBackend::new(offline_ingest_storage_config(
            root.clone(),
            max_capacity_bytes,
        ))
        .map_err(|error| {
            format!(
                "failed to lock and validate preseed store {}: {error}",
                root.display()
            )
        })?;
        let locked_root = fs::canonicalize(backend.root_dir()).map_err(|error| {
            format!(
                "failed to revalidate locked preseed store {}: {error}",
                root.display()
            )
        })?;
        if &locked_root != root {
            return Err(format!(
                "locked preseed store root changed from {} to {}",
                root.display(),
                locked_root.display()
            ));
        }
        stores.push(backend);
    }
    let store_count =
        u32::try_from(targets.len()).map_err(|_| "preseed store count exceeds u32".to_owned())?;
    let receipt_targets = targets
        .iter()
        .map(|target| {
            target
                .store_root
                .to_str()
                .map(|store_root| OperatorPreseedTargetReceiptV1 {
                    validator_account_id: target.validator_account_literal.clone(),
                    peer_id: target.peer_id.clone(),
                    store_root: store_root.to_owned(),
                })
                .ok_or_else(|| "preseed store roots must be valid UTF-8".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut receipt_artifacts = artifacts
        .iter()
        .map(|artifact| {
            let manifest_digest = artifact
                .manifest
                .digest()
                .map_err(|error| format!("failed to digest preseed receipt: {error}"))?;
            Ok(OperatorPreseedArtifactReceiptV1 {
                manifest_digest_blake3: hex::encode(manifest_digest.as_bytes()),
                payload_digest_blake3: hex::encode(artifact.plan.payload_digest.as_bytes()),
                content_length: artifact.plan.content_length,
                store_count,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    receipt_artifacts.sort_by(|left, right| {
        left.manifest_digest_blake3
            .cmp(&right.manifest_digest_blake3)
    });
    let receipt = OperatorPreseedSessionReceiptV1 {
        schema_version: OPERATOR_PRESEED_SESSION_RECEIPT_VERSION_V1,
        status: "ready".to_owned(),
        mode: if verify_only {
            "verify_only".to_owned()
        } else {
            "ingest".to_owned()
        },
        max_capacity_bytes,
        targets: receipt_targets,
        artifacts: receipt_artifacts,
    };
    receipt.validate()?;
    let mut durable_receipt = receipt.clone();
    durable_receipt.mode = "ingest".to_owned();
    let durable_receipt_bytes = json::to_vec(&durable_receipt)
        .map_err(|error| format!("failed to encode durable preseed receipt: {error}"))?;
    for store in &stores {
        recover_operator_preseed_store_receipt_staging(store.root_dir())?;
        let exact_exists =
            preflight_operator_preseed_store_receipt(store.root_dir(), &durable_receipt_bytes)?;
        if verify_only && !exact_exists {
            return Err(format!(
                "verify-only preseed store {} has no exact durable ingest qualification",
                store.root_dir().display()
            ));
        }
    }
    for artifact in &artifacts {
        for store in &stores {
            ingest_preseed_artifact_into_store(store, artifact, !verify_only)?;
        }
    }
    if verify_only {
        for store in &stores {
            let path =
                operator_preseed_store_receipt_path(store.root_dir(), &durable_receipt_bytes);
            let (installed, installed_bytes) = read_operator_preseed_store_receipt(&path)?;
            if installed != durable_receipt || installed_bytes != durable_receipt_bytes {
                return Err(format!(
                    "verify-only preseed store {} has a different durable qualification receipt",
                    store.root_dir().display()
                ));
            }
        }
    } else {
        for (index, store) in stores.iter().enumerate() {
            persist_operator_preseed_store_receipt(
                store.root_dir(),
                &durable_receipt_bytes,
                index,
            )?;
        }
    }
    let receipt_bytes = json::to_vec(&receipt)
        .map_err(|error| format!("failed to encode preseed ready receipt: {error}"))?;
    let mut stdout = io::stdout().lock();
    stdout
        .write_all(&receipt_bytes)
        .and_then(|()| stdout.write_all(b"\n"))
        .and_then(|()| stdout.flush())
        .map_err(|error| format!("failed to emit preseed ready receipt: {error}"))?;
    let mut input = [0_u8; 1];
    match io::stdin().read(&mut input) {
        Ok(0) => Ok(()),
        Ok(_) => Err("preseed-session stdin accepts only EOF after readiness".to_owned()),
        Err(error) => Err(format!(
            "failed while waiting for preseed-session EOF: {error}"
        )),
    }
}

fn persist_operator_preseed_store_receipt(
    store_root: &Path,
    receipt_bytes: &[u8],
    store_index: usize,
) -> Result<(), String> {
    let directory = operator_preseed_store_receipt_dir(store_root);
    match fs::symlink_metadata(&directory) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(format!(
                "operator-preseed qualification root {} must be one direct directory",
                directory.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            builder.mode(0o700);
            builder.create(&directory).map_err(|error| {
                format!(
                    "failed to create operator-preseed qualification root {}: {error}",
                    directory.display()
                )
            })?;
            fs::File::open(store_root)
                .and_then(|root| root.sync_all())
                .map_err(|error| {
                    format!(
                        "failed to synchronize operator-preseed store root {}: {error}",
                        store_root.display()
                    )
                })?;
        }
        Err(error) => {
            return Err(format!(
                "failed to inspect operator-preseed qualification root {}: {error}",
                directory.display()
            ));
        }
    }
    preflight_operator_preseed_store_receipt(store_root, receipt_bytes)?;
    let destination = operator_preseed_store_receipt_path(store_root, receipt_bytes);
    if destination.exists() {
        let (installed, installed_bytes) = read_operator_preseed_store_receipt(&destination)?;
        let expected: OperatorPreseedSessionReceiptV1 = json::from_slice(receipt_bytes)
            .map_err(|error| format!("failed to decode expected preseed receipt: {error}"))?;
        if installed == expected && installed_bytes == receipt_bytes {
            return Ok(());
        }
        return Err(format!(
            "content-addressed operator-preseed qualification {} has conflicting bytes",
            destination.display()
        ));
    }
    let staging = directory.join(format!(
        ".qualification.{}.{}.tmp",
        process::id(),
        store_index
    ));
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    set_no_follow_flag(&mut options);
    let mut file = options.open(&staging).map_err(|error| {
        format!(
            "failed to create staged operator-preseed receipt {}: {error}",
            staging.display()
        )
    })?;
    let write_result = file
        .write_all(receipt_bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| {
            format!(
                "failed to persist staged operator-preseed receipt {}: {error}",
                staging.display()
            )
        });
    if let Err(error) = write_result {
        drop(file);
        let _ = fs::remove_file(&staging);
        return Err(error);
    }
    drop(file);
    install_operator_preseed_store_receipt_staging(&staging, &destination).map_err(|error| {
        format!(
            "failed to install operator-preseed receipt {} without replacement: {error}",
            destination.display()
        )
    })?;
    let (installed, installed_bytes) = read_operator_preseed_store_receipt(&destination)?;
    let expected: OperatorPreseedSessionReceiptV1 = json::from_slice(receipt_bytes)
        .map_err(|error| format!("failed to decode expected preseed receipt: {error}"))?;
    if installed != expected || installed_bytes != receipt_bytes {
        return Err(format!(
            "installed operator-preseed receipt {} differs from its exact staged bytes",
            destination.display()
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct OfflineDirectoryFile {
    path: Vec<String>,
    size: u64,
}
fn offline_directory_inventory(root: &Path) -> Result<Vec<OfflineDirectoryFile>, String> {
    let root_metadata = fs::symlink_metadata(root).map_err(|error| {
        format!(
            "failed to inspect payload directory {}: {error}",
            root.display()
        )
    })?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(format!(
            "payload directory {} must be one direct directory",
            root.display()
        ));
    }
    fn visit(
        current: &Path,
        logical: &mut Vec<String>,
        files: &mut Vec<OfflineDirectoryFile>,
        total_bytes: &mut u64,
        total_entries: &mut usize,
    ) -> Result<(), String> {
        let reader = fs::read_dir(current).map_err(|error| {
            format!(
                "failed to read payload directory {}: {error}",
                current.display()
            )
        })?;
        let mut entries = Vec::new();
        for entry in reader {
            if *total_entries >= OFFLINE_DIRECTORY_MAX_ENTRIES {
                return Err(format!(
                    "payload directory contains more than {OFFLINE_DIRECTORY_MAX_ENTRIES} total entries"
                ));
            }
            let entry = entry.map_err(|error| {
                format!(
                    "failed to read payload directory {}: {error}",
                    current.display()
                )
            })?;
            *total_entries += 1;
            entries.push(entry);
        }
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let component = entry
                .file_name()
                .into_string()
                .map_err(|_| format!("payload directory path is not UTF-8: {}", path.display()))?;
            if component.is_empty() || matches!(component.as_str(), "." | "..") {
                return Err(format!(
                    "payload directory contains a non-canonical component: {}",
                    path.display()
                ));
            }
            if logical.len() >= OFFLINE_DIRECTORY_MAX_DEPTH {
                return Err(format!(
                    "payload directory exceeds {OFFLINE_DIRECTORY_MAX_DEPTH} path components at {}",
                    path.display()
                ));
            }
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                format!(
                    "failed to inspect payload entry {}: {error}",
                    path.display()
                )
            })?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "symbolic links are not allowed in payload directories: {}",
                    path.display()
                ));
            }
            logical.push(component);
            if metadata.is_dir() {
                visit(&path, logical, files, total_bytes, total_entries)?;
            } else if metadata.is_file() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt as _;
                    if metadata.nlink() != 1 {
                        return Err(format!(
                            "hard-linked files are not allowed in payload directories: {}",
                            path.display()
                        ));
                    }
                }
                *total_bytes = total_bytes
                    .checked_add(metadata.len())
                    .ok_or_else(|| "payload directory byte length overflow".to_owned())?;
                if *total_bytes > OFFLINE_DIRECTORY_MAX_PAYLOAD_BYTES {
                    return Err(format!(
                        "payload directory contains {} bytes; maximum is {OFFLINE_DIRECTORY_MAX_PAYLOAD_BYTES}",
                        *total_bytes
                    ));
                }
                files.push(OfflineDirectoryFile {
                    path: logical.clone(),
                    size: metadata.len(),
                });
            } else {
                return Err(format!(
                    "payload directory entry must be a regular file or directory: {}",
                    path.display()
                ));
            }
            logical.pop();
        }
        Ok(())
    }
    let mut files = Vec::new();
    let mut logical = Vec::new();
    let mut total_bytes = 0_u64;
    let mut total_entries = 0_usize;
    visit(
        root,
        &mut logical,
        &mut files,
        &mut total_bytes,
        &mut total_entries,
    )?;
    if files.is_empty() || total_bytes == 0 {
        return Err("payload directory must contain at least one non-empty byte".to_owned());
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}
struct OfflineSequentialPayloadReader<'a, P> {
    source: &'a mut P,
    offset: u64,
    length: u64,
}
impl<'a, P> OfflineSequentialPayloadReader<'a, P> {
    const fn new(source: &'a mut P, length: u64) -> Self {
        Self {
            source,
            offset: 0,
            length,
        }
    }
}
impl<P: PayloadSource> Read for OfflineSequentialPayloadReader<'_, P> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.offset == self.length || buffer.is_empty() {
            return Ok(0);
        }
        let buffer_len = u64::try_from(buffer.len())
            .map_err(|_| io::Error::other("payload read buffer exceeds u64"))?;
        let count = usize::try_from((self.length - self.offset).min(buffer_len))
            .map_err(|_| io::Error::other("payload length exceeds host width"))?;
        PayloadSource::read_exact(self.source, self.offset, &mut buffer[..count])
            .map_err(|error| io::Error::other(error.to_string()))?;
        self.offset = self
            .offset
            .checked_add(count as u64)
            .ok_or_else(|| io::Error::other("payload reader offset overflow"))?;
        Ok(count)
    }
}
impl<P: PayloadSource> OfflineSequentialPayloadReader<'_, P> {
    fn finish(self) -> Result<(), String> {
        if self.offset != self.length {
            return Err(format!(
                "payload reader consumed {} of {} bytes",
                self.offset, self.length
            ));
        }
        self.source
            .ensure_exhausted(self.length)
            .map_err(|error| format!("failed to validate exact payload source: {error}"))
    }
}
fn build_streaming_file_plan<P: PayloadSource>(
    source: &mut P,
    content_length: u64,
    profile: ChunkProfile,
) -> Result<CarBuildPlan, String> {
    if content_length == 0 {
        return Err("payload must not be empty".to_owned());
    }
    let mut chunker = sorafs_chunker::Chunker::try_with_profile(profile)
        .map_err(|error| format!("failed to construct streaming chunker: {error}"))?;
    let min_size =
        u64::try_from(profile.min_size).map_err(|_| "chunk minimum size exceeds u64".to_owned())?;
    let maximum_chunks = content_length.div_ceil(min_size);
    if maximum_chunks > CAR_PLAN_MAX_CHUNKS as u64 {
        return Err(format!(
            "file payload permits {maximum_chunks} chunk slots; maximum is {CAR_PLAN_MAX_CHUNKS}"
        ));
    }
    let maximum_chunks = usize::try_from(maximum_chunks)
        .map_err(|_| "file chunk inventory exceeds host width".to_owned())?;
    let mut chunks = Vec::new();
    chunks
        .try_reserve(maximum_chunks)
        .map_err(|_| "failed to reserve bounded file chunk inventory".to_owned())?;
    let mut payload_hasher = blake3::Hasher::new();
    let mut read_buffer = vec![0_u8; OFFLINE_DIRECTORY_STREAM_BUFFER_BYTES];
    let pending_capacity = OFFLINE_DIRECTORY_STREAM_BUFFER_BYTES
        .checked_add(profile.max_size)
        .ok_or_else(|| "streaming file buffer capacity overflow".to_owned())?;
    let mut pending = Vec::new();
    pending
        .try_reserve(pending_capacity)
        .map_err(|_| "failed to reserve bounded streaming file buffer".to_owned())?;
    let mut read_offset = 0_u64;
    let mut emitted_offset = 0_usize;
    while read_offset < content_length {
        let buffer_len = u64::try_from(read_buffer.len())
            .map_err(|_| "streaming buffer length exceeds u64".to_owned())?;
        let count = usize::try_from((content_length - read_offset).min(buffer_len))
            .map_err(|_| "payload file length exceeds host width".to_owned())?;
        PayloadSource::read_exact(source, read_offset, &mut read_buffer[..count])
            .map_err(|error| format!("failed to read stable payload file: {error}"))?;
        payload_hasher.update(&read_buffer[..count]);
        pending.extend_from_slice(&read_buffer[..count]);
        let mut boundaries = Vec::new();
        chunker.feed(&read_buffer[..count], |boundary| boundaries.push(boundary));
        let mut consumed = 0_usize;
        for boundary in boundaries {
            if boundary.offset != emitted_offset {
                return Err("streaming file chunker emitted non-contiguous geometry".to_owned());
            }
            let end = consumed
                .checked_add(boundary.length)
                .ok_or_else(|| "streaming file chunk length overflow".to_owned())?;
            let bytes = pending
                .get(consumed..end)
                .ok_or_else(|| "file chunk exceeded the bounded streaming buffer".to_owned())?;
            if chunks.len() >= CAR_PLAN_MAX_CHUNKS {
                return Err(format!("file payload exceeds {CAR_PLAN_MAX_CHUNKS} chunks"));
            }
            chunks.push(CarChunk {
                offset: u64::try_from(boundary.offset)
                    .map_err(|_| "streaming file chunk offset exceeds u64".to_owned())?,
                length: u32::try_from(boundary.length)
                    .map_err(|_| "streaming file chunk length exceeds u32".to_owned())?,
                digest: blake3::hash(bytes).into(),
                taikai_segment_hint: None,
            });
            emitted_offset = emitted_offset
                .checked_add(boundary.length)
                .ok_or_else(|| "streaming file emitted offset overflow".to_owned())?;
            consumed = end;
        }
        if consumed != 0 {
            pending.drain(..consumed);
        }
        read_offset = read_offset
            .checked_add(count as u64)
            .ok_or_else(|| "streaming file read offset overflow".to_owned())?;
    }
    let mut boundaries = Vec::new();
    chunker.finish(|boundary| boundaries.push(boundary));
    for boundary in boundaries {
        if boundary.offset != emitted_offset || pending.len() != boundary.length {
            return Err("streaming file chunker final geometry is not canonical".to_owned());
        }
        if chunks.len() >= CAR_PLAN_MAX_CHUNKS {
            return Err(format!("file payload exceeds {CAR_PLAN_MAX_CHUNKS} chunks"));
        }
        chunks.push(CarChunk {
            offset: u64::try_from(boundary.offset)
                .map_err(|_| "streaming file chunk offset exceeds u64".to_owned())?,
            length: u32::try_from(boundary.length)
                .map_err(|_| "streaming file chunk length exceeds u32".to_owned())?,
            digest: blake3::hash(&pending).into(),
            taikai_segment_hint: None,
        });
        emitted_offset = emitted_offset
            .checked_add(boundary.length)
            .ok_or_else(|| "streaming file emitted offset overflow".to_owned())?;
        pending.clear();
    }
    let emitted_length = u64::try_from(emitted_offset)
        .map_err(|_| "streaming file emitted length exceeds u64".to_owned())?;
    if emitted_length != content_length || !pending.is_empty() {
        return Err("streaming file chunker did not cover the payload exactly".to_owned());
    }
    PayloadSource::ensure_exhausted(source, content_length)
        .map_err(|error| format!("failed to validate stable payload file: {error}"))?;
    let chunk_count = chunks.len();
    let plan = CarBuildPlan {
        chunk_profile: profile,
        payload_digest: payload_hasher.finalize(),
        content_length,
        chunks,
        files: vec![FilePlan {
            path: Vec::new(),
            first_chunk: 0,
            chunk_count,
            size: content_length,
        }],
    };
    plan.validate()
        .map_err(|error| format!("invalid streaming file plan: {error}"))?;
    Ok(plan)
}
fn build_streaming_directory_plan(
    root: &Path,
    profile: ChunkProfile,
) -> Result<CarBuildPlan, String> {
    let inventory = offline_directory_inventory(root)?;
    let provisional = inventory
        .iter()
        .map(|file| FilePlan {
            path: file.path.clone(),
            first_chunk: 0,
            chunk_count: 0,
            size: file.size,
        })
        .collect::<Vec<_>>();
    let mut source = DirectoryPayload::new(root, &provisional)
        .map_err(|error| format!("failed to open exact directory payload: {error}"))?;
    let mut chunks = Vec::new();
    let mut files = Vec::with_capacity(provisional.len());
    let mut payload_hasher = blake3::Hasher::new();
    let mut global_offset = 0_u64;
    let mut read_buffer = vec![0_u8; OFFLINE_DIRECTORY_STREAM_BUFFER_BYTES];
    for provisional_file in provisional {
        let first_chunk = chunks.len();
        let mut local_offset = 0_u64;
        let mut emitted_offset = 0_usize;
        let mut pending = Vec::with_capacity(
            OFFLINE_DIRECTORY_STREAM_BUFFER_BYTES.saturating_add(profile.max_size),
        );
        let mut chunker = sorafs_chunker::Chunker::try_with_profile(profile)
            .map_err(|error| format!("failed to construct streaming chunker: {error}"))?;
        while local_offset < provisional_file.size {
            let buffer_len = u64::try_from(read_buffer.len())
                .map_err(|_| "streaming buffer length exceeds u64".to_owned())?;
            let count = usize::try_from((provisional_file.size - local_offset).min(buffer_len))
                .map_err(|_| "payload file length exceeds host width".to_owned())?;
            PayloadSource::read_exact(
                &mut source,
                global_offset + local_offset,
                &mut read_buffer[..count],
            )
            .map_err(|error| format!("failed to read exact directory payload bytes: {error}"))?;
            payload_hasher.update(&read_buffer[..count]);
            pending.extend_from_slice(&read_buffer[..count]);
            let mut boundaries = Vec::new();
            chunker.feed(&read_buffer[..count], |boundary| boundaries.push(boundary));
            let mut consumed = 0_usize;
            for boundary in boundaries {
                if boundary.offset != emitted_offset {
                    return Err("streaming chunker emitted non-contiguous geometry".to_owned());
                }
                let end = consumed
                    .checked_add(boundary.length)
                    .ok_or_else(|| "streaming chunk length overflow".to_owned())?;
                let bytes = pending
                    .get(consumed..end)
                    .ok_or_else(|| "chunk exceeded the bounded streaming buffer".to_owned())?;
                if chunks.len() >= CAR_PLAN_MAX_CHUNKS {
                    return Err(format!(
                        "directory payload exceeds {CAR_PLAN_MAX_CHUNKS} chunks"
                    ));
                }
                chunks.push(CarChunk {
                    offset: global_offset + boundary.offset as u64,
                    length: u32::try_from(boundary.length)
                        .map_err(|_| "streaming chunk length exceeds u32".to_owned())?,
                    digest: blake3::hash(bytes).into(),
                    taikai_segment_hint: None,
                });
                emitted_offset = emitted_offset
                    .checked_add(boundary.length)
                    .ok_or_else(|| "streaming emitted offset overflow".to_owned())?;
                consumed = end;
            }
            if consumed != 0 {
                pending.drain(..consumed);
            }
            local_offset = local_offset
                .checked_add(count as u64)
                .ok_or_else(|| "streaming local offset overflow".to_owned())?;
        }
        if provisional_file.size != 0 {
            let mut boundaries = Vec::new();
            chunker.finish(|boundary| boundaries.push(boundary));
            for boundary in boundaries {
                if boundary.offset != emitted_offset || pending.len() != boundary.length {
                    return Err("streaming chunker final geometry is not canonical".to_owned());
                }
                if chunks.len() >= CAR_PLAN_MAX_CHUNKS {
                    return Err(format!(
                        "directory payload exceeds {CAR_PLAN_MAX_CHUNKS} chunks"
                    ));
                }
                chunks.push(CarChunk {
                    offset: global_offset + boundary.offset as u64,
                    length: u32::try_from(boundary.length)
                        .map_err(|_| "streaming chunk length exceeds u32".to_owned())?,
                    digest: blake3::hash(&pending).into(),
                    taikai_segment_hint: None,
                });
                emitted_offset = emitted_offset
                    .checked_add(boundary.length)
                    .ok_or_else(|| "streaming emitted offset overflow".to_owned())?;
                pending.clear();
            }
        }
        if emitted_offset as u64 != provisional_file.size || !pending.is_empty() {
            return Err("streaming chunker did not cover one file exactly".to_owned());
        }
        files.push(FilePlan {
            path: provisional_file.path,
            first_chunk,
            chunk_count: chunks.len() - first_chunk,
            size: provisional_file.size,
        });
        global_offset = global_offset
            .checked_add(provisional_file.size)
            .ok_or_else(|| "streaming global offset overflow".to_owned())?;
    }
    PayloadSource::ensure_exhausted(&mut source, global_offset)
        .map_err(|error| format!("failed to validate exact directory source: {error}"))?;
    if offline_directory_inventory(root)? != inventory {
        return Err("payload directory changed while its streaming plan was built".to_owned());
    }
    let plan = CarBuildPlan {
        chunk_profile: profile,
        payload_digest: payload_hasher.finalize(),
        content_length: global_offset,
        chunks,
        files,
    };
    plan.validate()
        .map_err(|error| format!("invalid streaming directory plan: {error}"))?;
    Ok(plan)
}
fn ensure_streaming_manifest_alignment<P: PayloadSource>(
    manifest: &ManifestV1,
    plan: &CarBuildPlan,
    source: &mut P,
    source_label: &str,
) -> Result<(), String> {
    if manifest.content_length != plan.content_length
        || manifest.chunk_digest_sha3_256 != compute_chunk_plan_digest_sha3(&plan.chunks)
    {
        return Err(format!(
            "manifest geometry differs from the exact {source_label} plan"
        ));
    }
    let mut car_reader = OfflineSequentialPayloadReader::new(&mut *source, plan.content_length);
    let stats = CarStreamingWriter::new(plan)
        .write_from_reader(&mut car_reader, io::sink())
        .map_err(|error| format!("failed to rebuild canonical {source_label} CAR: {error}"))?;
    car_reader.finish()?;
    if stats.root_cids != vec![manifest.root_cid.clone()]
        || stats.dag_codec != manifest.dag_codec.0
        || stats.payload_bytes != plan.content_length
        || stats.chunk_count != plan.chunks.len()
        || stats.car_size != manifest.car_size
        || stats.car_archive_digest.as_bytes() != &manifest.car_digest
    {
        return Err(format!(
            "manifest CAR commitments differ from the exact {source_label} payload"
        ));
    }
    let mut store = ChunkStore::with_profile(plan.chunk_profile);
    store
        .ingest_plan_source(plan, source)
        .map_err(|error| format!("failed to rebuild {source_label} PoR tree: {error}"))?;
    PayloadSource::ensure_exhausted(source, plan.content_length)
        .map_err(|error| format!("failed to validate {source_label} PoR source: {error}"))?;
    if store.por_tree().root() != &manifest.por_root {
        return Err(format!(
            "manifest PoR root differs from the exact {source_label} payload"
        ));
    }
    Ok(())
}
fn ensure_streaming_directory_manifest_alignment(
    manifest: &ManifestV1,
    plan: &CarBuildPlan,
    payload_dir: &Path,
) -> Result<(), String> {
    let mut source = DirectoryPayload::new(payload_dir, &plan.files)
        .map_err(|error| format!("failed to open exact directory payload: {error}"))?;
    ensure_streaming_manifest_alignment(manifest, plan, &mut source, "directory")
}
fn ingest(
    data_dir: PathBuf,
    max_capacity_bytes: u64,
    manifest_path: PathBuf,
    payload_source: IngestPayloadSource,
    plan_json_out: Option<PathBuf>,
) -> Result<(), String> {
    let manifest_bytes =
        read_bounded_por_file(&manifest_path, "manifest", MAX_MANIFEST_ENCODED_BYTES)?;
    let manifest: ManifestV1 = decode_manifest_v1_canonical(&manifest_bytes)
        .map_err(|err| format!("failed to parse manifest: {err}"))?;
    let policy = PinPolicyConstraints {
        require_council_signatures: true,
        ..PinPolicyConstraints::default()
    };
    validate_manifest(&manifest, &policy)
        .map_err(|error| format!("failed to validate manifest: {error}"))?;
    enforce_ingest_capacity(manifest.content_length, max_capacity_bytes)?;
    let chunk_profile = chunk_profile_from_manifest(&manifest)?;
    let mut stable_file_source = match &payload_source {
        IngestPayloadSource::File(payload_path) => Some(open_exact_file_payload(
            payload_path,
            manifest.content_length,
        )?),
        IngestPayloadSource::Directory(_) => None,
    };
    let config = offline_ingest_storage_config(data_dir.clone(), max_capacity_bytes);
    let backend = StorageBackend::new(config)
        .map_err(|err| format!("failed to open storage backend: {err}"))?;
    let (plan, manifest_id) = match payload_source {
        IngestPayloadSource::File(payload_path) => {
            let mut source = stable_file_source
                .take()
                .ok_or_else(|| "stable file source was not prepared".to_owned())?;
            let plan =
                build_streaming_file_plan(&mut source, manifest.content_length, chunk_profile)
                    .map_err(|error| {
                        format!(
                            "failed to build streaming file chunk plan from {}: {error}",
                            payload_path.display()
                        )
                    })?;
            ensure_streaming_manifest_alignment(&manifest, &plan, &mut source, "file")?;
            let mut reader =
                OfflineSequentialPayloadReader::new(&mut source, manifest.content_length);
            let manifest_id = backend
                .ingest_manifest(&manifest, &plan, &mut reader)
                .map_err(|err| format!("failed to ingest manifest: {err}"))?;
            reader.finish()?;
            (plan, manifest_id)
        }
        IngestPayloadSource::Directory(payload_dir) => {
            let plan =
                build_streaming_directory_plan(&payload_dir, chunk_profile).map_err(|error| {
                    format!(
                        "failed to build streaming directory chunk plan from {}: {error}",
                        payload_dir.display()
                    )
                })?;
            ensure_streaming_directory_manifest_alignment(&manifest, &plan, &payload_dir)?;
            let mut source = DirectoryPayload::new(&payload_dir, &plan.files)
                .map_err(|error| format!("failed to open directory ingest source: {error}"))?;
            let mut reader = OfflineSequentialPayloadReader::new(&mut source, plan.content_length);
            let manifest_id = backend
                .ingest_manifest(&manifest, &plan, &mut reader)
                .map_err(|err| format!("failed to ingest manifest: {err}"))?;
            reader.finish()?;
            (plan, manifest_id)
        }
    };
    if let Some(path) = plan_json_out {
        let json_value = try_chunk_fetch_plan_to_json(&plan).map_err(|err| err.to_string())?;
        write_json_file(&path, json_value)?;
    }
    let manifest_digest = manifest
        .digest()
        .map_err(|err| format!("failed to compute manifest digest: {err}"))?;
    let mut root = Map::new();
    root.insert("manifest_id_hex".to_owned(), Value::from(manifest_id));
    root.insert("payload_bytes".to_owned(), Value::from(plan.content_length));
    root.insert(
        "chunk_count".to_owned(),
        Value::from(plan.chunks.len() as u64),
    );
    root.insert(
        "payload_digest_blake3".to_owned(),
        Value::from(hex::encode(plan.payload_digest.as_bytes())),
    );
    root.insert(
        "manifest_digest_blake3".to_owned(),
        Value::from(hex::encode(manifest_digest.as_bytes())),
    );
    root.insert(
        "data_dir".to_owned(),
        Value::from(data_dir.to_string_lossy().to_string()),
    );
    print_json(root)?;
    Ok(())
}
#[derive(Default)]
struct PorIngestOptions {
    data_dir: Option<PathBuf>,
    manifest_id: Option<String>,
    challenge_path: Option<PathBuf>,
    proof_path: Option<PathBuf>,
    verdict_path: Option<PathBuf>,
    json_out: Option<PathBuf>,
}
fn ingest_por_command(args: Vec<String>) -> Result<(), String> {
    let mut opts = PorIngestOptions::default();
    for arg in args {
        if arg == "--help" || arg == "-h" {
            print_por_usage();
            return Ok(());
        }
        if let Some(rest) = arg.strip_prefix("--data-dir=") {
            opts.data_dir = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--manifest-id=") {
            opts.manifest_id = Some(rest.trim().to_ascii_lowercase());
        } else if let Some(rest) = arg.strip_prefix("--challenge=") {
            opts.challenge_path = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--proof=") {
            opts.proof_path = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--verdict=") {
            opts.verdict_path = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--json-out=") {
            opts.json_out = Some(PathBuf::from(rest));
        } else {
            return Err(format!("unknown option: {arg}"));
        }
    }
    let data_dir = opts
        .data_dir
        .ok_or_else(|| "missing required option --data-dir".to_string())?;
    let challenge_path = opts
        .challenge_path
        .ok_or_else(|| "missing required option --challenge".to_string())?;
    let proof_path = opts
        .proof_path
        .ok_or_else(|| "missing required option --proof".to_string())?;
    let challenge_bytes = read_bounded_por_file(
        &challenge_path,
        "challenge",
        POR_CHALLENGE_MAX_CANONICAL_BYTES_V1,
    )?;
    let proof_bytes =
        read_bounded_por_file(&proof_path, "proof", POR_PROOF_MAX_CANONICAL_BYTES_V1)?;
    let challenge: PorChallengeV1 = decode_por_challenge_v1(&challenge_bytes)
        .map_err(|err| format!("failed to decode challenge: {err}"))?;
    challenge
        .validate()
        .map_err(|err| format!("invalid challenge: {err}"))?;
    let proof: PorProofV1 = decode_por_proof_v1(&proof_bytes)
        .map_err(|err| format!("failed to decode proof: {err}"))?;
    proof
        .validate()
        .map_err(|err| format!("invalid proof: {err}"))?;
    proof
        .verify_signature()
        .map_err(|err| format!("invalid proof signature: {err}"))?;
    if challenge.challenge_id != proof.challenge_id {
        return Err("challenge/proof mismatch: challenge ids differ".to_string());
    }
    if challenge.manifest_digest != proof.manifest_digest {
        return Err("challenge/proof mismatch: manifest digests differ".to_string());
    }
    if challenge.provider_id != proof.provider_id {
        return Err("challenge/proof mismatch: provider ids differ".to_string());
    }
    let storage_config = StorageConfig::builder()
        .enabled(true)
        .data_dir(data_dir.clone())
        .build();
    if let Some(manifest_id) = opts.manifest_id.as_ref() {
        let backend = StorageBackend::new(storage_config.clone())
            .map_err(|err| format!("failed to open storage backend: {err}"))?;
        let stored = backend
            .manifest(manifest_id)
            .ok_or_else(|| format!("manifest {manifest_id} not found in storage"))?;
        let stored_digest = stored.manifest_digest();
        if stored_digest != &challenge.manifest_digest {
            return Err(format!(
                "manifest digest mismatch: stored {} vs challenge {}",
                hex::encode(stored_digest),
                hex::encode(challenge.manifest_digest)
            ));
        }
    }
    let handle = NodeHandle::try_new(storage_config).map_err(|err| {
        format!(
            "failed to initialise SoraFS runtime from {}: {err}",
            data_dir.display()
        )
    })?;
    handle
        .record_por_challenge_with_authority_update(&challenge)
        .map_err(|err| format!("failed to record challenge: {err}"))?;
    handle
        .record_por_proof_with_authority_update(&proof, &proof.signature.public_key)
        .map_err(|err| format!("failed to record proof: {err}"))?;
    let verdict_snapshot = if let Some(verdict_path) = opts.verdict_path {
        let verdict_bytes = read_bounded_por_file(
            &verdict_path,
            "verdict",
            AUDIT_VERDICT_MAX_CANONICAL_BYTES_V1,
        )?;
        let verdict: AuditVerdictV1 = decode_audit_verdict_v1(&verdict_bytes)
            .map_err(|err| format!("failed to decode verdict: {err}"))?;
        verdict
            .validate()
            .map_err(|err| format!("invalid verdict: {err}"))?;
        verdict
            .verify_signatures()
            .map_err(|err| format!("invalid verdict signature: {err}"))?;
        if verdict.challenge_id != proof.challenge_id {
            return Err("verdict challenge id mismatches proof".to_string());
        }
        if verdict.manifest_digest != proof.manifest_digest {
            return Err("verdict manifest digest mismatches proof".to_string());
        }
        let embedded_auditor_keys = verdict
            .auditor_signatures
            .iter()
            .map(|signature| signature.public_key.clone())
            .collect::<Vec<_>>();
        let outcome = handle
            .record_por_verdict_with_authority_update(&verdict, &embedded_auditor_keys, 1)
            .map(|(outcome, _update)| outcome)
            .map_err(|err| format!("failed to record verdict: {err}"))?;
        Some((verdict, outcome))
    } else {
        None
    };
    let summary = build_por_summary(
        &opts.manifest_id,
        &challenge,
        &proof,
        verdict_snapshot.as_ref(),
    );
    let json_value = Value::Object(summary.clone());
    print_json(summary.clone())?;
    if let Some(path) = opts.json_out {
        write_json_file(&path, json_value)?;
    }
    Ok(())
}
fn read_bounded_por_file(path: &Path, kind: &str, maximum: usize) -> Result<Vec<u8>, String> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    set_no_follow_flag(&mut options);
    let file = options
        .open(path)
        .map_err(|err| format!("failed to open {kind} {}: {err}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|err| format!("failed to inspect {kind} {}: {err}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("{kind} {} must be a regular file", path.display()));
    }
    if metadata.len() > u64::try_from(maximum).unwrap_or(u64::MAX) {
        return Err(format!(
            "{kind} {} exceeds the {maximum}-byte canonical limit",
            path.display()
        ));
    }
    let read_limit = u64::try_from(
        maximum
            .checked_add(1)
            .ok_or_else(|| format!("{kind} file size limit overflow"))?,
    )
    .map_err(|_| format!("{kind} file size limit cannot be represented"))?;
    let mut bytes = Vec::new();
    file.take(read_limit)
        .read_to_end(&mut bytes)
        .map_err(|err| format!("failed to read {kind} {}: {err}", path.display()))?;
    if bytes.len() > maximum {
        return Err(format!(
            "{kind} {} exceeds the {maximum}-byte canonical limit",
            path.display()
        ));
    }
    Ok(bytes)
}
fn build_por_summary(
    manifest_id: &Option<String>,
    challenge: &PorChallengeV1,
    proof: &PorProofV1,
    verdict: Option<&(AuditVerdictV1, PorVerdictOutcome)>,
) -> norito::json::Map {
    use norito::json::{Map, Value};
    let mut map = Map::new();
    map.insert("status".to_owned(), Value::from("accepted"));
    if let Some(id) = manifest_id {
        map.insert("manifest_id_hex".to_owned(), Value::from(id.clone()));
    }
    map.insert(
        "manifest_digest_hex".to_owned(),
        Value::from(hex::encode(challenge.manifest_digest)),
    );
    map.insert(
        "provider_id_hex".to_owned(),
        Value::from(hex::encode(challenge.provider_id)),
    );
    map.insert(
        "challenge_id_hex".to_owned(),
        Value::from(hex::encode(challenge.challenge_id)),
    );
    map.insert(
        "sample_count".to_owned(),
        Value::from(u64::from(challenge.sample_count)),
    );
    map.insert("forced".to_owned(), Value::from(challenge.forced));
    map.insert("submitted_at".to_owned(), Value::from(proof.submitted_at));
    map.insert(
        "proof_digest_hex".to_owned(),
        Value::from(hex::encode(proof.proof_digest())),
    );
    if let Some((verdict, outcome)) = verdict {
        map.insert(
            "verdict".to_owned(),
            Value::Object(render_verdict_summary(verdict, outcome)),
        );
    }
    map
}
fn render_verdict_summary(verdict: &AuditVerdictV1, outcome: &PorVerdictOutcome) -> Map {
    let mut map = Map::new();
    let outcome_literal = match verdict.outcome {
        AuditOutcomeV1::Success => "success",
        AuditOutcomeV1::Failed => "failed",
        AuditOutcomeV1::Repaired => "repaired",
    };
    map.insert("outcome".to_owned(), Value::from(outcome_literal));
    map.insert(
        "success_samples".to_owned(),
        Value::from(outcome.stats.success_samples),
    );
    map.insert(
        "failed_samples".to_owned(),
        Value::from(outcome.stats.failed_samples),
    );
    if let Some(reason) = verdict.failure_reason.clone() {
        map.insert("failure_reason".to_owned(), Value::from(reason));
    }
    if let Some(task_id) = outcome.repair_task_id {
        map.insert(
            "repair_task_id_hex".to_owned(),
            Value::from(hex::encode(task_id)),
        );
    }
    map.insert(
        "consecutive_failures".to_owned(),
        Value::from(outcome.consecutive_failures),
    );
    map.insert(
        "decided_at_unix".to_owned(),
        Value::from(verdict.decided_at),
    );
    map
}
#[derive(Default)]
struct ExportOptions {
    data_dir: Option<PathBuf>,
    manifest_id: Option<String>,
    manifest_out: Option<PathBuf>,
    payload_out: Option<PathBuf>,
    plan_json_out: Option<PathBuf>,
}
fn export_command(args: Vec<String>) -> Result<(), String> {
    let mut opts = ExportOptions::default();
    for arg in args {
        if let Some(rest) = arg.strip_prefix("--data-dir=") {
            opts.data_dir = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--manifest-id=") {
            opts.manifest_id = Some(rest.trim().to_ascii_lowercase());
        } else if let Some(rest) = arg.strip_prefix("--manifest-out=") {
            opts.manifest_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--payload-out=") {
            opts.payload_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--plan-json-out=") {
            opts.plan_json_out = Some(PathBuf::from(rest));
        } else {
            return Err(format!("unknown option: {arg}"));
        }
    }
    let data_dir = opts
        .data_dir
        .ok_or_else(|| "missing required option --data-dir".to_string())?;
    let manifest_id = opts
        .manifest_id
        .ok_or_else(|| "missing required option --manifest-id".to_string())?;
    let manifest_out = opts
        .manifest_out
        .ok_or_else(|| "missing required option --manifest-out".to_string())?;
    let payload_out = opts
        .payload_out
        .ok_or_else(|| "missing required option --payload-out".to_string())?;
    export(
        data_dir,
        manifest_id,
        manifest_out,
        payload_out,
        opts.plan_json_out,
    )
}
fn export(
    data_dir: PathBuf,
    manifest_id: String,
    manifest_out: PathBuf,
    payload_out: PathBuf,
    plan_json_out: Option<PathBuf>,
) -> Result<(), String> {
    let config = StorageConfig::builder()
        .enabled(true)
        .data_dir(data_dir.clone())
        .build();
    let backend = StorageBackend::new(config)
        .map_err(|err| format!("failed to open storage backend: {err}"))?;
    let stored_manifest = backend
        .manifest(&manifest_id)
        .ok_or_else(|| format!("manifest {manifest_id} not found"))?;
    let manifest_bytes = fs::read(stored_manifest.manifest_path())
        .map_err(|err| format!("failed to read stored manifest: {err}"))?;
    write_bytes(&manifest_out, &manifest_bytes)?;
    let content_length = stored_manifest.content_length();
    if content_length > usize::MAX as u64 {
        return Err("stored payload exceeds platform limits".to_string());
    }
    let payload = backend
        .read_payload_range(&manifest_id, 0, content_length as usize)
        .map_err(|err| format!("failed to read stored payload: {err}"))?;
    write_bytes(&payload_out, &payload)?;
    if let Some(path) = plan_json_out {
        let manifest_v1 = stored_manifest
            .load_manifest()
            .map_err(|err| format!("failed to decode stored manifest: {err}"))?;
        let chunk_profile = chunk_profile_from_manifest(&manifest_v1)?;
        let taikai_hint = sorafs_car::taikai_segment_hint_from_sorafs_manifest(&manifest_v1)
            .map_err(|err| format!("failed to derive Taikai metadata: {err}"))?;
        let plan = stored_manifest.to_car_plan_with_hint(chunk_profile, taikai_hint);
        let json_value = try_chunk_fetch_plan_to_json(&plan).map_err(|err| err.to_string())?;
        write_json_file(&path, json_value)?;
    }
    let payload_digest_hex = hex::encode(stored_manifest.payload_digest());
    let mut root = Map::new();
    root.insert("manifest_id_hex".to_owned(), Value::from(manifest_id));
    root.insert("payload_bytes".to_owned(), Value::from(content_length));
    root.insert(
        "chunk_count".to_owned(),
        Value::from(stored_manifest.chunk_count() as u64),
    );
    root.insert(
        "payload_digest_blake3".to_owned(),
        Value::from(payload_digest_hex),
    );
    root.insert(
        "data_dir".to_owned(),
        Value::from(data_dir.to_string_lossy().to_string()),
    );
    print_json(root)?;
    Ok(())
}
fn chunk_profile_from_manifest(manifest: &ManifestV1) -> Result<ChunkProfile, String> {
    if let Some(descriptor) =
        chunker_registry::lookup(sorafs_car::ProfileId(manifest.chunking.profile_id.0))
    {
        if descriptor.multihash_code != manifest.chunking.multihash_code {
            return Err(format!(
                "manifest multihash code {} does not match registered profile {}",
                manifest.chunking.multihash_code, descriptor.multihash_code
            ));
        }
        Ok(descriptor.profile)
    } else {
        if manifest.chunking.multihash_code != BLAKE3_256_MULTIHASH_CODE {
            return Err(format!(
                "unknown chunker profile id {} with unsupported multihash code {}",
                manifest.chunking.profile_id.0, manifest.chunking.multihash_code
            ));
        }
        if manifest.chunking.min_size == 0
            || manifest.chunking.target_size == 0
            || manifest.chunking.max_size == 0
            || manifest.chunking.break_mask == 0
        {
            return Err("manifest chunking profile fields must be non-zero".to_string());
        }
        Ok(ChunkProfile {
            min_size: manifest.chunking.min_size as usize,
            target_size: manifest.chunking.target_size as usize,
            max_size: manifest.chunking.max_size as usize,
            break_mask: manifest.chunking.break_mask as u64,
        })
    }
}
fn write_json_file(path: &Path, value: Value) -> Result<(), String> {
    let text = json::to_string_pretty(&value).map_err(|err| err.to_string())?;
    write_text(path, &text)
}
fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    validate_output_path(path)?;
    ensure_parent_dir(path)?;
    validate_output_path(path)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    set_no_follow_flag(&mut options);
    let mut file = options
        .open(path)
        .map_err(|err| format!("failed to open {} for writing: {err}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|err| format!("failed to inspect {} after open: {err}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!(
            "failed to write {}: output must be a regular file",
            path.display()
        ));
    }
    file.write_all(bytes)
        .map_err(|err| format!("failed to write {}: {err}", path.display()))
}
fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    Ok(())
}
fn validate_output_path(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(format!("output {} must not be a symlink", path.display()));
            }
            if metadata.is_dir() {
                return Err(format!("output {} must not be a directory", path.display()));
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(format!(
                "failed to inspect output {}: {err}",
                path.display()
            ));
        }
    }
    if let Some(parent) = path.parent() {
        for ancestor in std::iter::once(parent).chain(parent.ancestors().skip(1)) {
            if ancestor.as_os_str().is_empty() {
                continue;
            }
            match fs::symlink_metadata(ancestor) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() {
                        return Err(format!(
                            "output parent {} must not be a symlink",
                            ancestor.display()
                        ));
                    }
                    if !metadata.is_dir() {
                        return Err(format!(
                            "output parent {} must be a directory",
                            ancestor.display()
                        ));
                    }
                }
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err) => {
                    return Err(format!(
                        "failed to inspect output parent {}: {err}",
                        ancestor.display()
                    ));
                }
            }
        }
    }
    Ok(())
}
#[cfg(unix)]
fn set_no_follow_flag(options: &mut fs::OpenOptions) {
    options.custom_flags(platform_no_follow_flag());
}
#[cfg(not(unix))]
fn set_no_follow_flag(_options: &mut fs::OpenOptions) {}
#[cfg(all(
    target_os = "android",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "riscv64",
        target_arch = "x86",
        target_arch = "x86_64"
    ))
))]
compile_error!("SoraFS node output flags are not qualified for this Android architecture");
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))
))]
compile_error!("SoraFS node output flags are not qualified for this Unix target");
#[cfg(all(target_os = "android", target_arch = "riscv64"))]
fn platform_no_follow_flag() -> i32 {
    0x400000
}
#[cfg(all(
    target_os = "android",
    any(target_arch = "aarch64", target_arch = "arm")
))]
fn platform_no_follow_flag() -> i32 {
    0x8000
}
#[cfg(all(
    target_os = "android",
    any(target_arch = "x86", target_arch = "x86_64")
))]
fn platform_no_follow_flag() -> i32 {
    0x20000
}
#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
))]
fn platform_no_follow_flag() -> i32 {
    0x8000
}
#[cfg(all(
    target_os = "linux",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    ))
))]
fn platform_no_follow_flag() -> i32 {
    0x20000
}
#[cfg(all(
    unix,
    not(any(target_os = "linux", target_os = "android")),
    any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    )
))]
fn platform_no_follow_flag() -> i32 {
    0x100
}
fn write_text(path: &Path, text: &str) -> Result<(), String> {
    let mut buf = text.to_owned();
    if !buf.ends_with('\n') {
        buf.push('\n');
    }
    write_bytes(path, buf.as_bytes())
}
fn print_json(map: Map) -> Result<(), String> {
    let json = json::to_string_pretty(&Value::Object(map)).map_err(|err| err.to_string())?;
    println!("{json}");
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempfile::TempDir;
    #[test]
    fn ingest_capacity_requires_nonzero_canonical_u64() {
        assert_eq!(
            parse_nonzero_canonical_u64("1", "--max-capacity-bytes"),
            Ok(1)
        );
        assert_eq!(
            parse_nonzero_canonical_u64("18446744073709551615", "--max-capacity-bytes"),
            Ok(u64::MAX)
        );
        for invalid in [
            "",
            "0",
            "00",
            "01",
            "+1",
            "-1",
            " 1",
            "1 ",
            "1_000",
            "18446744073709551616",
        ] {
            assert!(
                parse_nonzero_canonical_u64(invalid, "--max-capacity-bytes").is_err(),
                "{invalid:?} must be rejected"
            );
        }
    }
    #[test]
    fn ingest_capacity_preflight_rejects_payload_over_exact_cap() {
        assert_eq!(enforce_ingest_capacity(4096, 4096), Ok(()));
        assert_eq!(
            enforce_ingest_capacity(4097, 4096),
            Err("manifest payload length 4097 exceeds --max-capacity-bytes=4096".to_owned())
        );
    }
    #[cfg(unix)]
    #[test]
    fn file_payload_preflight_requires_exact_stable_regular_file() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().expect("tempdir");
        let payload = temp.path().join("payload.bin");
        fs::write(&payload, b"exact").expect("payload");
        open_exact_file_payload(&payload, 5).expect("exact stable file");
        let mismatch = open_exact_file_payload(&payload, 4)
            .err()
            .expect("length mismatch must fail");
        assert!(mismatch.contains("manifest content length 4"), "{mismatch}");

        let linked = temp.path().join("linked.bin");
        symlink(&payload, &linked).expect("symlink");
        let symlink_error = open_exact_file_payload(&linked, 5)
            .err()
            .expect("symlink must fail");
        assert!(
            symlink_error.contains("stable no-follow payload file"),
            "{symlink_error}"
        );

        let hard_linked = temp.path().join("hard-linked.bin");
        fs::hard_link(&payload, &hard_linked).expect("hard link");
        let hard_link_error = open_exact_file_payload(&payload, 5)
            .err()
            .expect("multiply linked file must fail");
        assert!(
            hard_link_error.contains("stable no-follow payload file"),
            "{hard_link_error}"
        );
    }
    #[cfg(unix)]
    #[test]
    fn streaming_file_plan_matches_canonical_eager_plan() {
        let temp = TempDir::new().expect("tempdir");
        let path = temp.path().join("payload.bin");
        let mut payload = vec![0x5A; OFFLINE_DIRECTORY_STREAM_BUFFER_BYTES + 333_333];
        payload[777_777..888_888].fill(0xA5);
        fs::write(&path, &payload).expect("payload");
        let mut source =
            open_exact_file_payload(&path, payload.len() as u64).expect("stable payload");
        let streaming =
            build_streaming_file_plan(&mut source, payload.len() as u64, ChunkProfile::DEFAULT)
                .expect("streaming plan");
        let eager = CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT)
            .expect("eager plan");
        assert_eq!(streaming, eager);
    }
    #[test]
    fn ingest_command_requires_explicit_capacity() {
        let error = ingest_command(vec![
            "--data-dir=storage".to_owned(),
            "--manifest=manifest.to".to_owned(),
            "--payload=payload.bin".to_owned(),
        ])
        .expect_err("ingest capacity must be explicit");
        assert_eq!(error, "missing required option --max-capacity-bytes");

        let duplicate = ingest_command(vec![
            "--max-capacity-bytes=1".to_owned(),
            "--max-capacity-bytes=2".to_owned(),
        ])
        .expect_err("duplicate capacity must be rejected");
        assert_eq!(duplicate, "duplicate option --max-capacity-bytes");
    }
    #[test]
    fn ingest_storage_config_uses_exact_cli_capacity() {
        let data_dir = PathBuf::from("storage");
        let config = offline_ingest_storage_config(data_dir.clone(), 10_737_418_240);
        assert!(config.enabled());
        assert_eq!(config.data_dir(), &data_dir);
        assert_eq!(config.max_capacity_bytes().0, 10_737_418_240);
    }
    #[test]
    fn ingest_payload_source_requires_exactly_one_canonical_input() {
        let file = PathBuf::from("payload.bin");
        let directory = PathBuf::from("payload");
        assert_eq!(
            require_ingest_payload_source(Some(file.clone()), None).expect("file source"),
            IngestPayloadSource::File(file.clone())
        );
        assert_eq!(
            require_ingest_payload_source(None, Some(directory.clone())).expect("directory source"),
            IngestPayloadSource::Directory(directory.clone())
        );
        assert!(require_ingest_payload_source(None, None).is_err());
        assert!(require_ingest_payload_source(Some(file), Some(directory)).is_err());
    }
    #[test]
    fn streaming_directory_plan_matches_canonical_eager_plan() {
        let temp = TempDir::new().expect("tempdir");
        let nested = temp.path().join("aarch64");
        fs::create_dir(&nested).expect("nested dir");
        fs::write(nested.join("initrd.img"), vec![0x11; 333_333]).expect("initrd");
        fs::write(nested.join("rootfs.ext4"), vec![0x22; 1_500_321]).expect("rootfs");
        fs::write(nested.join("vmlinuz"), vec![0x33; 700_777]).expect("kernel");
        let (eager, payload) =
            CarBuildPlan::from_directory_with_profile(temp.path(), ChunkProfile::DEFAULT)
                .expect("eager plan");
        let streaming = build_streaming_directory_plan(temp.path(), ChunkProfile::DEFAULT)
            .expect("streaming plan");
        assert_eq!(streaming, eager);
        assert_eq!(streaming.payload_digest, blake3::hash(&payload));
    }
    #[test]
    fn sparse_taira_scale_inventory_stays_metadata_only() {
        let temp = TempDir::new().expect("tempdir");
        let nested = temp.path().join("aarch64");
        fs::create_dir(&nested).expect("nested dir");
        let sizes = [27_236_288_u64, 3_085_959_168, 13_923_072];
        for (name, size) in ["vmlinuz", "rootfs.ext4", "initrd.img"]
            .into_iter()
            .zip(sizes)
        {
            File::create(nested.join(name))
                .expect("create sparse guest member")
                .set_len(size)
                .expect("size sparse guest member");
        }
        let inventory = offline_directory_inventory(temp.path()).expect("large inventory");
        let total = inventory.iter().map(|file| file.size).sum::<u64>();
        assert_eq!(inventory.len(), 3);
        assert_eq!(total, sizes.into_iter().sum::<u64>());
        assert_eq!(total, 3_127_118_528);
        assert!(total > 3_000_000_000);
        assert!(total > 512 * 1024 * 1024);
    }
    #[test]
    fn directory_inventory_accepts_exact_total_entry_boundary() {
        let temp = TempDir::new().expect("tempdir");
        for index in 0..(OFFLINE_DIRECTORY_MAX_ENTRIES - 1) {
            fs::create_dir(temp.path().join(format!("empty-{index:04}")))
                .expect("boundary empty directory");
        }
        fs::write(temp.path().join("payload.bin"), b"payload").expect("boundary payload");

        let inventory = offline_directory_inventory(temp.path()).expect("boundary inventory");
        assert_eq!(
            inventory,
            vec![OfflineDirectoryFile {
                path: vec!["payload.bin".to_owned()],
                size: 7,
            }]
        );
    }
    #[test]
    fn directory_inventory_rejects_entry_over_limit_before_sorting() {
        let temp = TempDir::new().expect("tempdir");
        for index in 0..=OFFLINE_DIRECTORY_MAX_ENTRIES {
            fs::create_dir(temp.path().join(format!("empty-{index:04}")))
                .expect("over-limit empty directory");
        }

        let error = offline_directory_inventory(temp.path())
            .expect_err("entry count over the bounded inventory must fail");
        assert_eq!(
            error,
            format!(
                "payload directory contains more than {OFFLINE_DIRECTORY_MAX_ENTRIES} total entries"
            )
        );
    }
    #[test]
    fn taira_scale_plan_fits_default_ingest_heap_without_payload_allocation() {
        let profile = ChunkProfile::DEFAULT;
        let members = [
            (
                vec!["aarch64".to_owned(), "initrd.img".to_owned()],
                13_923_072_u64,
            ),
            (
                vec!["aarch64".to_owned(), "rootfs.ext4".to_owned()],
                3_085_959_168_u64,
            ),
            (
                vec!["aarch64".to_owned(), "vmlinuz".to_owned()],
                27_236_288_u64,
            ),
        ];
        let mut chunks = Vec::new();
        let mut files = Vec::new();
        let mut offset = 0_u64;
        for (path, size) in members {
            let first_chunk = chunks.len();
            let mut remaining = size;
            while remaining != 0 {
                // The minimum canonical chunk size maximizes metadata and therefore
                // exercises the worst valid ingest-heap geometry for this payload.
                let length = remaining.min(profile.min_size as u64);
                chunks.push(CarChunk {
                    offset,
                    length: u32::try_from(length).expect("default chunk length"),
                    digest: [0xA5; 32],
                    taikai_segment_hint: None,
                });
                offset += length;
                remaining -= length;
            }
            files.push(FilePlan {
                path,
                first_chunk,
                chunk_count: chunks.len() - first_chunk,
                size,
            });
        }
        assert_eq!(offset, 3_127_118_528);
        let plan = CarBuildPlan {
            chunk_profile: profile,
            payload_digest: blake3::hash(b"metadata-only Taira plan"),
            content_length: offset,
            chunks,
            files,
        };
        let validation = plan
            .validate_for_ingest()
            .expect("real Taira geometry must fit default ingest heap");
        assert!(
            validation.estimated_ingest_heap_bytes()
                <= sorafs_car::DEFAULT_CHUNK_STORE_MAX_ESTIMATED_HEAP_BYTES
        );
    }
    #[cfg(unix)]
    #[test]
    fn streaming_directory_plan_rejects_symlinked_intermediate_directory() {
        use std::os::unix::fs::symlink;
        let temp = TempDir::new().expect("tempdir");
        let real = temp.path().join("real");
        fs::create_dir(&real).expect("real dir");
        fs::write(real.join("rootfs.ext4"), b"payload").expect("payload");
        symlink(&real, temp.path().join("aarch64")).expect("symlink");
        let error = build_streaming_directory_plan(temp.path(), ChunkProfile::DEFAULT)
            .expect_err("symlink must fail");
        assert!(error.contains("symbolic links"), "{error}");
    }
    #[test]
    fn bounded_por_file_reader_accepts_boundary_and_rejects_one_over() {
        let temp = TempDir::new().expect("tempdir");
        let input_path = temp.path().join("por.to");
        fs::write(&input_path, [1_u8, 2, 3]).expect("write boundary input");
        assert_eq!(
            read_bounded_por_file(&input_path, "proof", 3).expect("boundary input"),
            vec![1, 2, 3]
        );
        fs::write(&input_path, [1_u8, 2, 3, 4]).expect("write oversized input");
        assert!(
            read_bounded_por_file(&input_path, "proof", 3)
                .expect_err("one-over input must fail")
                .contains("exceeds")
        );
    }
    #[cfg(unix)]
    #[test]
    fn bounded_por_file_reader_rejects_symlink() {
        let temp = TempDir::new().expect("tempdir");
        let target_path = temp.path().join("target.to");
        fs::write(&target_path, [1_u8, 2, 3]).expect("write target");
        let input_path = temp.path().join("input.to");
        std::os::unix::fs::symlink(&target_path, &input_path).expect("create symlink");
        assert!(read_bounded_por_file(&input_path, "proof", 3).is_err());
    }
    // Keep one target-gated assertion for every ABI branch. Overlapping branches
    // fail with duplicate definitions; missing branches fail to resolve the flag.
    #[cfg(all(
        target_os = "linux",
        any(
            target_arch = "aarch64",
            target_arch = "arm",
            target_arch = "m68k",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        )
    ))]
    #[test]
    fn linux_no_follow_flag_matches_low_flag_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x8000);
    }
    #[cfg(all(
        target_os = "linux",
        not(any(
            target_arch = "aarch64",
            target_arch = "arm",
            target_arch = "m68k",
            target_arch = "powerpc",
            target_arch = "powerpc64"
        ))
    ))]
    #[test]
    fn linux_no_follow_flag_matches_generic_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x20000);
    }
    #[cfg(all(
        target_os = "android",
        any(target_arch = "aarch64", target_arch = "arm")
    ))]
    #[test]
    fn android_arm_no_follow_flag_matches_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x8000);
    }
    #[cfg(all(
        target_os = "android",
        any(target_arch = "x86", target_arch = "x86_64")
    ))]
    #[test]
    fn android_x86_no_follow_flag_matches_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x20000);
    }
    #[cfg(all(target_os = "android", target_arch = "riscv64"))]
    #[test]
    fn android_riscv64_no_follow_flag_matches_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x400000);
    }
    #[cfg(all(
        target_os = "linux",
        any(target_arch = "riscv32", target_arch = "riscv64")
    ))]
    #[test]
    fn linux_riscv_no_follow_flag_remains_generic_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x20000);
    }
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    #[test]
    fn apple_and_bsd_no_follow_flag_matches_target_abi() {
        assert_eq!(platform_no_follow_flag(), 0x100);
    }
    #[test]
    fn write_bytes_creates_parent_and_writes_all_bytes() {
        let temp = TempDir::new().expect("tempdir");
        let temp_path = temp.path().canonicalize().expect("canonical tempdir");
        let output_path = temp_path.join("nested").join("payload.bin");
        write_bytes(&output_path, b"sorafs-node-output").expect("write output");
        assert_eq!(
            fs::read(&output_path).expect("read output"),
            b"sorafs-node-output"
        );
    }
    #[cfg(unix)]
    #[test]
    fn write_bytes_rejects_symlink_output() {
        let temp = TempDir::new().expect("tempdir");
        let temp_path = temp.path().canonicalize().expect("canonical tempdir");
        let target_path = temp_path.join("target.bin");
        fs::write(&target_path, b"unchanged").expect("write target");
        let output_path = temp_path.join("output.bin");
        std::os::unix::fs::symlink(&target_path, &output_path).expect("create symlink");
        let err = write_bytes(&output_path, b"changed").expect_err("reject symlink output");
        assert!(
            err.contains("must not be a symlink"),
            "unexpected error: {err}"
        );
        assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged");
    }
    #[cfg(unix)]
    #[test]
    fn write_bytes_rejects_symlink_parent() {
        let temp = TempDir::new().expect("tempdir");
        let temp_path = temp.path().canonicalize().expect("canonical tempdir");
        let real_dir = temp_path.join("real");
        fs::create_dir(&real_dir).expect("create real dir");
        let linked_dir = temp_path.join("linked");
        std::os::unix::fs::symlink(&real_dir, &linked_dir).expect("create symlink");
        let output_path = linked_dir.join("payload.bin");
        let err = write_bytes(&output_path, b"changed").expect_err("reject symlink parent");
        assert!(
            err.contains("parent") && err.contains("must not be a symlink"),
            "unexpected error: {err}"
        );
        assert!(
            !real_dir.join("payload.bin").exists(),
            "symlink parent should not receive output"
        );
    }
}
