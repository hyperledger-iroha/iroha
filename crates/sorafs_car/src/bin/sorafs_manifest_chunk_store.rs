//! CLI helper for ingesting payloads with the SoraFS chunk store prototype.

use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use norito::json::{Map, Value, to_string_pretty};
use sorafs_car::{
    ChunkStore, FilePayload, InMemoryPayload,
    por_json::{parse_proof_spec, proof_from_value, proof_to_value, sample_to_map, tree_to_value},
};
use sorafs_manifest::{
    ProfileId,
    chunker_registry::{self, ChunkerProfileDescriptor},
};

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut profile_id: Option<ProfileId> = None;
    let mut profile_handle: Option<String> = None;
    let mut json_out: Option<PathBuf> = None;
    let mut por_json_out: Option<PathBuf> = None;
    let mut payload_path: Option<PathBuf> = None;
    let mut list_profiles = false;
    let mut promote_profile: Option<String> = None;
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
            profile_id = Some(ProfileId(id));
        } else if arg == "--list-profiles" {
            list_profiles = true;
        } else if let Some(rest) = arg.strip_prefix("--promote-profile=") {
            promote_profile = Some(rest.trim().to_string());
        } else if let Some(rest) = arg.strip_prefix("--profile=") {
            profile_handle = Some(rest.trim().to_string());
        } else if let Some(rest) = arg.strip_prefix("--json-out=") {
            json_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--por-json-out=") {
            por_json_out = Some(PathBuf::from(rest));
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
        if promote_profile.is_some() {
            return Err("cannot combine --list-profiles with --promote-profile".to_string());
        }
        let profiles: Vec<Value> = chunker_registry::registry()
            .iter()
            .map(|descriptor| Value::Object(descriptor_to_json(descriptor)))
            .collect();
        let json = to_string_pretty(&Value::Array(profiles)).map_err(|err| err.to_string())? + "\n";
        let wrote_stdout = if let Some(path) = json_out.as_ref() {
            write_text(path.as_path(), &json)?
        } else {
            false
        };
        if !wrote_stdout {
            print!("{json}");
        }
        return Ok(());
    }

    if let Some(candidate) = promote_profile {
        if payload_path.is_some()
            || profile_id.is_some()
            || profile_handle.is_some()
            || por_json_out.is_some()
            || proof_spec.is_some()
            || proof_out.is_some()
            || proof_verify.is_some()
            || sample_count.is_some()
            || sample_out.is_some()
        {
            return Err(
                "--promote-profile cannot be combined with payload processing or PoR options"
                    .into(),
            );
        }

        chunker_registry::ensure_charter_compliance()
            .map_err(|err| format!("registry charter violation: {err}"))?;

        let canonical = resolve_profile_handle(&candidate)?;
        let descriptor = chunker_registry::lookup_by_handle(&canonical).ok_or_else(|| {
            format!(
                "unknown chunker profile handle: {canonical}. use --list-profiles to inspect registered entries"
            )
        })?;

        let mut meta = Map::new();
        meta.insert("canonical_handle".into(), Value::from(canonical.clone()));
        meta.insert("profile_id".into(), Value::from(descriptor.id.0));
        meta.insert("namespace".into(), Value::from(descriptor.namespace));
        meta.insert("name".into(), Value::from(descriptor.name));
        meta.insert("semver".into(), Value::from(descriptor.semver));
        meta.insert(
            "min_size".into(),
            Value::from(descriptor.profile.min_size as u64),
        );
        meta.insert(
            "target_size".into(),
            Value::from(descriptor.profile.target_size as u64),
        );
        meta.insert(
            "max_size".into(),
            Value::from(descriptor.profile.max_size as u64),
        );
        meta.insert(
            "break_mask".into(),
            Value::from(format!("0x{:04x}", descriptor.profile.break_mask)),
        );
        meta.insert(
            "multihash_code".into(),
            Value::from(format!("0x{:x}", descriptor.multihash_code)),
        );
        let alias_values: Vec<Value> = descriptor
            .aliases
            .iter()
            .map(|alias| Value::from(*alias))
            .collect();
        meta.insert("aliases".into(), Value::Array(alias_values));
        meta.insert(
            "promotion_hint".into(),
            Value::from(
                "Move this descriptor to the front of RAW_REGISTRY in crates/sorafs_car/src/chunker_registry_data.rs to make it the default profile."
            ),
        );

        let json = to_string_pretty(&Value::Object(meta)).map_err(|err| err.to_string())? + "\n";
        let wrote_stdout = if let Some(path) = json_out.as_ref() {
            write_text(path.as_path(), &json)?
        } else {
            false
        };
        if !wrote_stdout {
            print!("{json}");
        }
        return Ok(());
    }

    let path = payload_path.ok_or_else(|| {
        "usage: sorafs_manifest_chunk_store [--profile-id=<id>] [--profile=<namespace.name@semver>] [--json-out=path] [--por-json-out=path] [--promote-profile=<handle>] [--por-proof=chunk:segment:leaf] [--por-proof-out=path] [--por-proof-verify=path] [--por-sample=count] [--por-sample-seed=value] [--por-sample-out=path] <payload>"
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
        chunker_registry::lookup(id).ok_or_else(|| {
            format!(
                "unknown chunker profile id: {}. use --list-profiles to inspect registered entries",
                id.0
            )
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

    let mut root = Map::new();
    root.insert("input_bytes".into(), Value::from(store.payload_len()));
    root.insert(
        "payload_digest_blake3".into(),
        Value::from(to_hex(store.payload_digest().as_bytes())),
    );
    root.insert("chunk_count".into(), Value::from(chunk_array.len() as u64));
    let por_root_hex = to_hex(store.por_tree().root());
    root.insert("por_root_hex".into(), Value::from(por_root_hex.clone()));
    root.insert(
        "por_chunk_count".into(),
        Value::from(store.por_tree().chunks().len() as u64),
    );
    root.insert(
        "profile".into(),
        Value::Object(descriptor_to_json(descriptor)),
    );
    root.insert("chunks".into(), Value::Array(chunk_array));

    let mut file_payload = FilePayload::open(&path).ok();

    let mut proof_json: Option<Value> = None;
    if let Some((chunk_idx, segment_idx, leaf_idx)) = proof_spec {
        let proof_result = if let Some(src) = file_payload.as_mut() {
            store
                .por_tree()
                .prove_leaf_with(chunk_idx, segment_idx, leaf_idx, src)
        } else {
            let mut fallback = InMemoryPayload::new(&bytes);
            store
                .por_tree()
                .prove_leaf_with(chunk_idx, segment_idx, leaf_idx, &mut fallback)
        };
        let proof = proof_result
            .map_err(|err| format!("failed to build PoR proof: {err}"))?
            .ok_or_else(|| {
                format!(
                    "invalid --por-proof indices chunk={chunk_idx} segment={segment_idx} leaf={leaf_idx}"
                )
            })?;
        let proof_value = proof_to_value(&proof);
        if let Some(path) = &proof_out {
            let mut serialized = to_string_pretty(&proof_value).map_err(|err| err.to_string())?;
            serialized.push('\n');
            write_text(path.as_path(), &serialized)?;
        }
        proof_json = Some(proof_value);
    }

    if let Some(path) = proof_verify {
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
        let samples = if let Some(src) = file_payload.as_mut() {
            store.sample_leaves_with(count, sample_seed.unwrap_or(0x9e3779b97f4a7c15), src)
        } else {
            let mut fallback = InMemoryPayload::new(&bytes);
            store.sample_leaves_with(
                count,
                sample_seed.unwrap_or(0x9e3779b97f4a7c15),
                &mut fallback,
            )
        }
        .map_err(|err| format!("failed to sample PoR leaves: {err}"))?;
        let target = count.min(total_leaves);
        if samples.len() < target {
            root.insert("por_samples_truncated".into(), Value::from(true));
        }
        let proofs: Vec<Value> = samples
            .into_iter()
            .map(|(idx, proof)| Value::Object(sample_to_map(idx, &proof)))
            .collect();
        if let Some(path) = sample_out {
            let mut serialized =
                to_string_pretty(&Value::Array(proofs.clone())).map_err(|err| err.to_string())?;
            serialized.push('\n');
            write_text(path.as_path(), &serialized)?;
        }
        root.insert("por_samples".into(), Value::Array(proofs));
    }

    let report = Value::Object(root);
    let json_bytes =
        to_string_pretty(&report).map_err(|err| format!("failed to serialise JSON: {err}"))? + "\n";

    let mut report_written_to_stdout = false;
    if let Some(path) = json_out.as_ref()
        && write_text(path.as_path(), &json_bytes)?
    {
        report_written_to_stdout = true;
    }

    if let Some(path) = por_json_out {
        let por_json = to_string_pretty(&tree_to_value(store.por_tree()))
            .map_err(|err| format!("failed to serialise PoR JSON: {err}"))?
            + "\n";
        write_text(path.as_path(), &por_json)?;
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

fn write_text(path: &Path, text: &str) -> Result<bool, String> {
    if path == Path::new("-") {
        io::stdout()
            .write_all(text.as_bytes())
            .map_err(|err| format!("failed to write to stdout: {err}"))?;
        return Ok(true);
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    fs::write(path, text.as_bytes())
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    Ok(false)
}

fn resolve_profile_handle(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("chunker profile cannot be empty".into());
    }
    if let Some(descriptor) = chunker_registry::lookup_by_handle(trimmed) {
        return Ok(format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        ));
    }
    if let Ok(id) = trimmed.parse::<u32>() {
        if let Some(descriptor) = chunker_registry::lookup(ProfileId(id)) {
            return Ok(format!(
                "{}.{}@{}",
                descriptor.namespace, descriptor.name, descriptor.semver
            ));
        }
        return Err(format!(
            "unknown chunker profile id: {id}. Use --list-profiles to inspect the registry"
        ));
    }
    if let Some(descriptor) = chunker_registry::registry().iter().find(|entry| {
        entry
            .aliases
            .iter()
            .any(|alias| alias.eq_ignore_ascii_case(trimmed))
    }) {
        return Ok(format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        ));
    }
    Err(format!(
        "unknown chunker profile handle '{trimmed}'. expected namespace.name@semver"
    ))
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

    use super::*;

    #[test]
    fn registry_lookup_round_trips_profile_id() {
        let descriptor = chunker_registry::default_descriptor();
        let looked_up = chunker_registry::lookup(descriptor.id).expect("descriptor present");
        assert!(std::ptr::eq(descriptor, looked_up));
    }

    #[test]
    fn descriptor_to_json_exposes_core_metadata() {
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
    fn lookup_by_handle_resolves_registered_descriptor() {
        let descriptor = chunker_registry::default_descriptor();
        let handle = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        let resolved = chunker_registry::lookup_by_handle(&handle).expect("handle resolves");
        assert!(std::ptr::eq(descriptor, resolved));
    }
}
