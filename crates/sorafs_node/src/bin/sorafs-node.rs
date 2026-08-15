//! Offline developer CLI helpers for inspecting the SoraFS storage backend.
use norito::json::{self, Map, Value};
use sorafs_car::{
    CarBuildPlan, CarWriter, chunker_registry, fetch_plan::try_chunk_fetch_plan_to_json,
    verifier::CarVerifier,
};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, ManifestV1, decode_manifest_v1_canonical,
    por::{
        AUDIT_VERDICT_MAX_CANONICAL_BYTES_V1, AuditOutcomeV1, AuditVerdictV1,
        POR_CHALLENGE_MAX_CANONICAL_BYTES_V1, POR_PROOF_MAX_CANONICAL_BYTES_V1, PorChallengeV1,
        PorProofV1, decode_audit_verdict_v1, decode_por_challenge_v1, decode_por_proof_v1,
    },
};
use sorafs_node::{NodeHandle, PorVerdictOutcome, config::StorageConfig, store::StorageBackend};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::{
    env, fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process,
};
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
         ingest --data-dir=<dir> --manifest=<path> --payload=<path> [--plan-json-out=<path>]\n  \
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
    manifest_path: Option<PathBuf>,
    payload_path: Option<PathBuf>,
    plan_json_out: Option<PathBuf>,
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
        } else if let Some(rest) = arg.strip_prefix("--manifest=") {
            opts.manifest_path = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--payload=") {
            opts.payload_path = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--plan-json-out=") {
            opts.plan_json_out = Some(PathBuf::from(rest));
        } else {
            return Err(format!("unknown option: {arg}"));
        }
    }
    let data_dir = opts
        .data_dir
        .ok_or_else(|| "missing required option --data-dir".to_string())?;
    let manifest_path = opts
        .manifest_path
        .ok_or_else(|| "missing required option --manifest".to_string())?;
    let payload_path = opts
        .payload_path
        .ok_or_else(|| "missing required option --payload".to_string())?;
    ingest(data_dir, manifest_path, payload_path, opts.plan_json_out)
}
fn ingest(
    data_dir: PathBuf,
    manifest_path: PathBuf,
    payload_path: PathBuf,
    plan_json_out: Option<PathBuf>,
) -> Result<(), String> {
    let manifest_bytes = fs::read(&manifest_path)
        .map_err(|err| format!("failed to read manifest {}: {err}", manifest_path.display()))?;
    let manifest: ManifestV1 = decode_manifest_v1_canonical(&manifest_bytes)
        .map_err(|err| format!("failed to parse manifest: {err}"))?;
    let chunk_profile = chunk_profile_from_manifest(&manifest)?;
    let payload_bytes = fs::read(&payload_path)
        .map_err(|err| format!("failed to read payload {}: {err}", payload_path.display()))?;
    if payload_bytes.is_empty() {
        return Err("payload must not be empty".to_string());
    }
    let plan = CarBuildPlan::single_file_with_profile(&payload_bytes, chunk_profile)
        .map_err(|err| format!("failed to build chunk plan: {err}"))?;
    ensure_manifest_plan_alignment(&manifest, &plan, &payload_bytes)?;
    let config = StorageConfig::builder()
        .enabled(true)
        .data_dir(data_dir.clone())
        .build();
    let backend = StorageBackend::new(config)
        .map_err(|err| format!("failed to open storage backend: {err}"))?;
    let mut reader = io::Cursor::new(payload_bytes);
    let manifest_id = backend
        .ingest_manifest(&manifest, &plan, &mut reader)
        .map_err(|err| format!("failed to ingest manifest: {err}"))?;
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
fn ensure_manifest_plan_alignment(
    manifest: &ManifestV1,
    plan: &CarBuildPlan,
    payload_bytes: &[u8],
) -> Result<(), String> {
    if manifest.content_length != plan.content_length {
        return Err(format!(
            "manifest content length {} differs from plan {}",
            manifest.content_length, plan.content_length
        ));
    }
    let writer =
        CarWriter::new(plan, payload_bytes).map_err(|err| format!("failed to build CAR: {err}"))?;
    let mut car_bytes = Vec::new();
    writer
        .write_to(&mut car_bytes)
        .map_err(|err| format!("failed to materialize CAR: {err}"))?;
    CarVerifier::verify_full_car_with_plan(manifest, plan, &car_bytes)
        .map_err(|err| format!("manifest verification failed: {err}"))?;
    Ok(())
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
    use tempfile::TempDir;
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
