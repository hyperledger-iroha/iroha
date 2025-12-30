//! CLI helpers for interacting with the SoraFS storage backend.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process,
};

use norito::json::{self, Map, Value};
use sorafs_car::{CarBuildPlan, chunker_registry, fetch_plan::chunk_fetch_specs_to_json};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, ManifestV1,
    por::{AuditOutcomeV1, AuditVerdictV1, PorChallengeV1, PorProofV1},
};
use sorafs_node::{NodeHandle, PorVerdictOutcome, config::StorageConfig, store::StorageBackend};

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
         ingest [manifest] --data-dir=<dir> --manifest=<path> --payload=<path> [--plan-json-out=<path>]\n  \
         ingest por --data-dir=<dir> --challenge=<path> --proof=<path> [--verdict=<path>] [--manifest-id=<hex>] [--json-out=<path>]\n  \
         export --data-dir=<dir> --manifest-id=<hex> --manifest-out=<path> --payload-out=<path> [--plan-json-out=<path>]\n  \
         --help, -h   Show this help message"
    );
}

fn print_por_usage() {
    eprintln!(
        "Usage: sorafs-node ingest por --data-dir=<dir> --challenge=<path> --proof=<path> [--verdict=<path>] [--manifest-id=<hex>] [--json-out=<path>]\n\n\
         Replay a PoR challenge/proof/verdict locally before submitting it to Torii."
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
    if let Some(first) = args.first().cloned() {
        if first == "por" {
            args.remove(0);
            return ingest_por_command(args);
        }
        if first == "manifest" {
            args.remove(0);
        }
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
    let manifest: ManifestV1 = norito::decode_from_bytes(&manifest_bytes)
        .map_err(|err| format!("failed to parse manifest: {err}"))?;

    let chunk_profile = chunk_profile_from_manifest(&manifest)?;

    let payload_bytes = fs::read(&payload_path)
        .map_err(|err| format!("failed to read payload {}: {err}", payload_path.display()))?;
    if payload_bytes.is_empty() {
        return Err("payload must not be empty".to_string());
    }

    let plan = CarBuildPlan::single_file_with_profile(&payload_bytes, chunk_profile)
        .map_err(|err| format!("failed to build chunk plan: {err}"))?;

    ensure_manifest_plan_alignment(&manifest, &plan)?;

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
        let json_value = chunk_fetch_specs_to_json(&plan);
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
        } else if let Some(rest) = arg.strip_prefix("--manifest=") {
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

    let challenge_bytes = fs::read(&challenge_path).map_err(|err| {
        format!(
            "failed to read challenge {}: {err}",
            challenge_path.display()
        )
    })?;
    let proof_bytes = fs::read(&proof_path)
        .map_err(|err| format!("failed to read proof {}: {err}", proof_path.display()))?;

    let challenge: PorChallengeV1 = norito::decode_from_bytes(&challenge_bytes)
        .map_err(|err| format!("failed to decode challenge: {err}"))?;
    challenge
        .validate()
        .map_err(|err| format!("invalid challenge: {err}"))?;
    let proof: PorProofV1 = norito::decode_from_bytes(&proof_bytes)
        .map_err(|err| format!("failed to decode proof: {err}"))?;
    proof
        .validate()
        .map_err(|err| format!("invalid proof: {err}"))?;

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

    let handle = NodeHandle::new(storage_config);
    handle
        .record_por_challenge(&challenge)
        .map_err(|err| format!("failed to record challenge: {err}"))?;
    handle
        .record_por_proof(&proof)
        .map_err(|err| format!("failed to record proof: {err}"))?;

    let verdict_snapshot = if let Some(verdict_path) = opts.verdict_path {
        let verdict_bytes = fs::read(&verdict_path)
            .map_err(|err| format!("failed to read verdict {}: {err}", verdict_path.display()))?;
        let verdict: AuditVerdictV1 = norito::decode_from_bytes(&verdict_bytes)
            .map_err(|err| format!("failed to decode verdict: {err}"))?;
        if verdict.challenge_id != proof.challenge_id {
            return Err("verdict challenge id mismatches proof".to_string());
        }
        if verdict.manifest_digest != proof.manifest_digest {
            return Err("verdict manifest digest mismatches proof".to_string());
        }
        let outcome = handle
            .record_por_verdict(&verdict)
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
    if let Some(history_id) = outcome.repair_history_id {
        map.insert("repair_history_id".to_owned(), Value::from(history_id));
    }
    map.insert(
        "consecutive_failures".to_owned(),
        Value::from(outcome.consecutive_failures),
    );
    if let Some(slash) = &outcome.slash {
        let mut slash_map = Map::new();
        slash_map.insert(
            "provider_id_hex".to_owned(),
            Value::from(hex::encode(slash.provider_id.as_bytes())),
        );
        slash_map.insert(
            "manifest_digest_hex".to_owned(),
            Value::from(hex::encode(slash.manifest_digest)),
        );
        slash_map.insert(
            "penalty_nano".to_owned(),
            Value::from(slash.penalty_nano.to_string()),
        );
        slash_map.insert("strikes".to_owned(), Value::from(slash.strikes));
        slash_map.insert("reason".to_owned(), Value::from(slash.reason.clone()));
        map.insert("slash_recommendation".to_owned(), Value::Object(slash_map));
    }
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
        let json_value = chunk_fetch_specs_to_json(&plan);
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
) -> Result<(), String> {
    if manifest.content_length != plan.content_length {
        return Err(format!(
            "manifest content length {} differs from plan {}",
            manifest.content_length, plan.content_length
        ));
    }
    if manifest.car_digest != *plan.payload_digest.as_bytes() {
        return Err("manifest CAR digest does not match computed payload digest".to_string());
    }
    Ok(())
}

fn write_json_file(path: &Path, value: Value) -> Result<(), String> {
    let text = json::to_string_pretty(&value).map_err(|err| err.to_string())?;
    write_text(path, &text)
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    fs::write(path, bytes).map_err(|err| format!("failed to write {}: {err}", path.display()))
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
