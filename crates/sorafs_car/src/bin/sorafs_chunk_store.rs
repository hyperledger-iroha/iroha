//! CLI helper for ingesting payloads with the SoraFS chunk store prototype.

use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use norito::json::{Map, Value, to_string_pretty};
use sorafs_car::{
    CarBuildPlan, CarChunk, ChunkStore, ProfileId,
    chunker_registry::{self, ChunkerProfileDescriptor},
    fetch_plan::{chunk_fetch_specs_to_json, chunk_fetch_specs_to_string},
    por_json::{parse_proof_spec, proof_from_value, proof_to_value, sample_to_map, tree_to_value},
};

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn write_text(path: &Path, text: &str) -> Result<(), String> {
    if path == Path::new("-") {
        io::stdout()
            .write_all(text.as_bytes())
            .map_err(|err| format!("failed to write text to stdout: {err}"))?;
        return Ok(());
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    fs::write(path, text.as_bytes())
        .map_err(|err| format!("failed to write {}: {err}", path.display()))
}

fn run() -> Result<(), String> {
    let mut profile_id: Option<u32> = None;
    let mut profile_handle: Option<String> = None;
    let mut json_out: Option<PathBuf> = None;
    let mut por_json_out: Option<PathBuf> = None;
    let mut chunk_fetch_plan_out: Option<PathBuf> = None;
    let mut payload_path: Option<PathBuf> = None;
    let mut list_profiles = false;
    let mut proof_spec: Option<(usize, usize, usize)> = None;
    let mut proof_out: Option<PathBuf> = None;
    let mut proof_verify: Option<PathBuf> = None;
    let mut sample_count: Option<usize> = None;
    let mut sample_seed: Option<u64> = None;
    let mut sample_out: Option<PathBuf> = None;

    for arg in env::args().skip(1) {
        if let Some(rest) = arg.strip_prefix("--profile-id=") {
            let id = rest
                .parse::<u32>()
                .map_err(|err| format!("invalid profile id: {err}"))?;
            profile_id = Some(id);
        } else if arg == "--list-profiles" {
            list_profiles = true;
        } else if let Some(rest) = arg.strip_prefix("--profile=") {
            profile_handle = Some(rest.trim().to_string());
        } else if let Some(rest) = arg.strip_prefix("--json-out=") {
            json_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--por-json-out=") {
            por_json_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--chunk-fetch-plan-out=") {
            chunk_fetch_plan_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--por-proof=") {
            proof_spec = Some(parse_proof_spec(rest)?);
        } else if let Some(rest) = arg.strip_prefix("--por-proof-out=") {
            proof_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--por-proof-verify=") {
            proof_verify = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--por-sample=") {
            sample_count = Some(
                rest.parse::<usize>()
                    .map_err(|err| format!("invalid --por-sample count: {err}"))?,
            );
        } else if let Some(rest) = arg.strip_prefix("--por-sample-seed=") {
            sample_seed = Some(
                parse_u64(rest).map_err(|err| format!("invalid --por-sample-seed value: {err}"))?,
            );
        } else if let Some(rest) = arg.strip_prefix("--por-sample-out=") {
            sample_out = Some(PathBuf::from(rest));
        } else if arg.starts_with("--") {
            return Err(format!("unknown option: {arg}"));
        } else if payload_path.is_none() {
            payload_path = Some(PathBuf::from(arg));
        } else {
            return Err(format!("unexpected argument: {arg}"));
        }
    }

    if list_profiles {
        if payload_path.is_some() {
            return Err("cannot supply a payload path when using --list-profiles".to_string());
        }

        let profiles: Vec<Value> = chunker_registry::registry()
            .iter()
            .map(|descriptor| Value::Object(descriptor_to_json(descriptor)))
            .collect();
        let json = to_string_pretty(&Value::Array(profiles)).map_err(|err| err.to_string())? + "\n";

        if let Some(path) = json_out {
            fs::write(&path, json.as_bytes())
                .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
        }

        print!("{json}");
        return Ok(());
    }

    let path = payload_path.ok_or_else(|| {
    "usage: sorafs-chunk-store [--profile-id=<id>] [--profile=<namespace.name@semver>] [--json-out=path] [--chunk-fetch-plan-out=path] [--por-json-out=path] [--por-proof=chunk:segment:leaf] [--por-proof-out=path] [--por-proof-verify=path] [--por-sample=count] [--por-sample-seed=value] [--por-sample-out=path] <payload>"
            .to_string()
    })?;

    if profile_id.is_some() && profile_handle.is_some() {
        return Err("use either --profile-id or --profile, not both".to_string());
    }

    let descriptor = if let Some(handle) = profile_handle.as_deref() {
        chunker_registry::lookup_by_handle(handle).ok_or_else(|| {
            format!("unknown chunker profile handle: {handle}. expected namespace.name@semver")
        })?
    } else if let Some(id) = profile_id {
        chunker_registry::lookup(ProfileId(id)).ok_or_else(|| {
            format!("unknown chunker profile id: {id}. use --list-profiles to inspect registered entries")
        })?
    } else {
        chunker_registry::default_descriptor()
    };

    let bytes =
        fs::read(&path).map_err(|err| format!("failed to read {}: {err}", path.display()))?;

    let mut store = ChunkStore::with_profile(descriptor.profile);
    store.ingest_bytes(&bytes);

    let mut chunk_array = Vec::with_capacity(store.chunks().len());
    for chunk in store.chunks() {
        let mut obj = Map::new();
        obj.insert("offset".into(), Value::from(chunk.offset));
        obj.insert("length".into(), Value::from(chunk.length));
        obj.insert("digest_blake3".into(), Value::from(to_hex(&chunk.blake3)));
        chunk_array.push(Value::Object(obj));
    }

    let por_tree = store.por_tree();

    let mut root = Map::new();
    root.insert("input_bytes".into(), Value::from(store.payload_len()));
    root.insert(
        "payload_digest_blake3".into(),
        Value::from(to_hex(store.payload_digest().as_bytes())),
    );
    root.insert("chunk_count".into(), Value::from(chunk_array.len() as u64));
    root.insert("por_root_hex".into(), Value::from(to_hex(por_tree.root())));
    root.insert(
        "por_chunk_count".into(),
        Value::from(por_tree.chunks().len() as u64),
    );
    root.insert(
        "profile".into(),
        Value::Object(descriptor_to_json(descriptor)),
    );
    root.insert("chunks".into(), Value::Array(chunk_array));
    let plan = plan_from_store(&store);
    let chunk_fetch_specs = chunk_fetch_specs_to_json(&plan);
    root.insert("chunk_fetch_specs".into(), chunk_fetch_specs.clone());

    let mut proof_json: Option<Value> = None;
    if let Some((chunk_idx, segment_idx, leaf_idx)) = proof_spec {
        let proof = store
            .por_tree()
            .prove_leaf(chunk_idx, segment_idx, leaf_idx, &bytes)
            .ok_or_else(|| {
                format!(
                    "invalid --por-proof indices chunk={chunk_idx} segment={segment_idx} leaf={leaf_idx}"
                )
            })?;
        let proof_value = proof_to_value(&proof);
        if let Some(path) = &proof_out {
            let mut serialized = to_string_pretty(&proof_value).map_err(|err| err.to_string())?;
            serialized.push('\n');
            fs::write(path, serialized.as_bytes())
                .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
        }
        proof_json = Some(proof_value);
    }

    if let Some(path) = proof_verify.take() {
        let proof_bytes =
            fs::read(&path).map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        let proof_value: Value =
            norito::json::from_slice(&proof_bytes).map_err(|err| err.to_string())?;
        let proof = proof_from_value(&proof_value)?;
        if !proof.verify(store.por_tree().root()) {
            return Err("provided PoR proof does not verify against computed root".into());
        }
        root.insert("por_proof_verified".into(), Value::from(true));
        if proof_json.is_none() {
            proof_json = Some(proof_value);
        }
    }

    if let Some(value) = proof_json {
        root.insert("por_proof".into(), value);
    }

    if let Some(count) = sample_count {
        let total_leaves = store.por_tree().leaf_count();
        if total_leaves == 0 {
            return Err("cannot sample PoR leaves from an empty tree".into());
        }
        let seed = sample_seed.unwrap_or(0x9e3779b97f4a7c15);
        let samples_vec = store.sample_leaves(count, seed, &bytes);
        if samples_vec.is_empty() {
            return Err("cannot sample PoR leaves from an empty tree".into());
        }
        if count > total_leaves || samples_vec.len() < count {
            root.insert("por_samples_truncated".into(), Value::from(true));
        }
        let samples: Vec<Value> = samples_vec
            .into_iter()
            .map(|(flat, proof)| Value::Object(sample_to_map(flat, &proof)))
            .collect();
        if let Some(path) = sample_out {
            let mut serialized =
                to_string_pretty(&Value::Array(samples.clone())).map_err(|err| err.to_string())?;
            serialized.push('\n');
            write_text(path.as_path(), &serialized)?;
        }
        root.insert("por_samples".into(), Value::Array(samples));
    }

    let report = Value::Object(root);
    let json_bytes =
        to_string_pretty(&report).map_err(|err| format!("failed to serialise JSON: {err}"))? + "\n";

    let mut report_written_to_stdout = false;
    if let Some(path) = json_out {
        if path.as_os_str() == "-" {
            report_written_to_stdout = true;
        }
        write_text(path.as_path(), &json_bytes)?;
    }

    if let Some(path) = chunk_fetch_plan_out {
        let plan_text = chunk_fetch_specs_to_string(&plan.chunk_fetch_specs())
            .map_err(|err| format!("failed to serialise chunk fetch specs: {err}"))?;
        write_text(path.as_path(), &plan_text)?;
        if path.as_os_str() == "-" {
            report_written_to_stdout = true;
        }
    }

    if let Some(path) = por_json_out {
        let por_json = to_string_pretty(&tree_to_value(por_tree))
            .map_err(|err| format!("failed to serialise PoR JSON: {err}"))?
            + "\n";
        write_text(path.as_path(), &por_json)?;
        if path.as_os_str() == "-" {
            report_written_to_stdout = true;
        }
    }

    if !report_written_to_stdout {
        print!("{json_bytes}");
    }
    Ok(())
}

fn descriptor_to_json(descriptor: &ChunkerProfileDescriptor) -> Map {
    let mut descriptor_map = Map::new();
    descriptor_map.insert("namespace".into(), Value::from(descriptor.namespace));
    descriptor_map.insert("name".into(), Value::from(descriptor.name));
    descriptor_map.insert("semver".into(), Value::from(descriptor.semver));
    descriptor_map.insert(
        "handle".into(),
        Value::from(format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        )),
    );
    descriptor_map.insert("profile_id".into(), Value::from(descriptor.id.0 as u64));
    descriptor_map.insert(
        "min_size".into(),
        Value::from(descriptor.profile.min_size as u64),
    );
    descriptor_map.insert(
        "target_size".into(),
        Value::from(descriptor.profile.target_size as u64),
    );
    descriptor_map.insert(
        "max_size".into(),
        Value::from(descriptor.profile.max_size as u64),
    );
    descriptor_map.insert(
        "break_mask".into(),
        Value::from(format!("0x{:04x}", descriptor.profile.break_mask)),
    );
    descriptor_map.insert(
        "multihash_code".into(),
        Value::from(descriptor.multihash_code),
    );
    descriptor_map
}

fn plan_from_store(store: &ChunkStore) -> CarBuildPlan {
    let chunks = store
        .chunks()
        .iter()
        .map(|chunk| CarChunk {
            offset: chunk.offset,
            length: chunk.length,
            digest: chunk.blake3,
            taikai_segment_hint: None,
        })
        .collect();
    CarBuildPlan {
        chunk_profile: store.profile(),
        payload_digest: *store.payload_digest(),
        content_length: store.payload_len(),
        chunks,
        files: Vec::new(),
    }
}

fn to_hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(TABLE[(byte >> 4) as usize] as char);
        out.push(TABLE[(byte & 0x0f) as usize] as char);
    }
    out
}

fn parse_u64(value: &str) -> Result<u64, std::num::ParseIntError> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16)
    } else {
        value.parse::<u64>()
    }
}

#[cfg(test)]
mod tests {
    use norito::json::Value;
    use sorafs_chunker::ChunkProfile;

    use super::*;

    #[test]
    fn default_descriptor_matches_known_values() {
        let descriptor = chunker_registry::default_descriptor();
        assert_eq!(descriptor.id.0, 1);
        assert_eq!(descriptor.namespace, "sorafs");
        assert_eq!(descriptor.name, "sf1");
        assert_eq!(descriptor.semver, "1.0.0");
        assert_eq!(descriptor.profile, ChunkProfile::DEFAULT);
        assert_eq!(descriptor.multihash_code, 0x1f);
    }

    #[test]
    fn lookup_descriptor_resolves_registry_entries() {
        let descriptor = chunker_registry::default_descriptor();
        let looked_up = chunker_registry::lookup(descriptor.id).expect("descriptor present");
        assert!(std::ptr::eq(descriptor, looked_up));
        assert!(chunker_registry::lookup(ProfileId(9999)).is_none());
    }

    #[test]
    fn descriptor_to_json_includes_core_fields() {
        let descriptor = chunker_registry::default_descriptor();
        let map = descriptor_to_json(descriptor);
        assert_eq!(
            map.get("namespace").and_then(Value::as_str),
            Some(descriptor.namespace)
        );
        assert_eq!(
            map.get("name").and_then(Value::as_str),
            Some(descriptor.name)
        );
        assert_eq!(
            map.get("semver").and_then(Value::as_str),
            Some(descriptor.semver)
        );
        assert_eq!(
            map.get("profile_id").and_then(Value::as_u64),
            Some(descriptor.id.0 as u64)
        );
        assert_eq!(
            map.get("handle").and_then(Value::as_str),
            Some("sorafs.sf1@1.0.0")
        );
    }

    #[test]
    fn lookup_by_handle_matches_registry_descriptor() {
        let descriptor = chunker_registry::default_descriptor();
        let handle = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        let resolved = chunker_registry::lookup_by_handle(&handle).expect("handle resolves");
        assert!(std::ptr::eq(descriptor, resolved));
    }
}
