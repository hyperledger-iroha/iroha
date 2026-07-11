//! CLI helper for ingesting payloads with the SoraFS chunk store prototype.

use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

use norito::json::{Map, Value, to_string_pretty};
use sorafs_car::{
    CarBuildPlan, ChunkStore, DirectoryChunkSinkOutput, FilePayload, InMemoryPayload,
    PersistedChunkRecord,
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
    let mut chunk_dir_out: Option<PathBuf> = None;
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
            let id = parse_u32_decimal(rest, "--profile-id")?;
            profile_id = Some(ProfileId(id));
        } else if arg == "--list-profiles" {
            list_profiles = true;
        } else if let Some(rest) = arg.strip_prefix("--promote-profile=") {
            promote_profile = Some(parse_profile_handle_arg(rest, "--promote-profile")?);
        } else if let Some(rest) = arg.strip_prefix("--profile=") {
            profile_handle = Some(parse_profile_handle_arg(rest, "--profile")?);
        } else if let Some(rest) = arg.strip_prefix("--json-out=") {
            json_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--por-json-out=") {
            por_json_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--chunk-dir-out=") {
            chunk_dir_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--por-proof=") {
            proof_spec = Some(parse_proof_spec(rest)?);
        } else if let Some(rest) = arg.strip_prefix("--por-proof-out=") {
            proof_out = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--por-proof-verify=") {
            proof_verify = Some(PathBuf::from(rest));
        } else if let Some(rest) = arg.strip_prefix("--por-sample=") {
            sample_count = Some(parse_nonzero_usize_decimal(rest, "--por-sample")?);
        } else if let Some(rest) = arg.strip_prefix("--por-sample-seed=") {
            sample_seed = Some(parse_u64(rest, "--por-sample-seed")?);
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
        if chunk_dir_out.is_some() {
            return Err("cannot combine --list-profiles with --chunk-dir-out".to_string());
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
            || chunk_dir_out.is_some()
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
        "usage: sorafs_manifest_chunk_store [--profile-id=<id>] [--profile=<namespace.name@semver>] [--json-out=path] [--chunk-dir-out=dir] [--por-json-out=path] [--promote-profile=<handle>] [--por-proof=chunk:segment:leaf] [--por-proof-out=path] [--por-proof-verify=path] [--por-sample=count] [--por-sample-seed=value] [--por-sample-out=path] <payload>"
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
    let persisted_chunks = if let Some(directory) = chunk_dir_out.as_deref() {
        preflight_chunk_dir_out(directory)?;
        let output = if bytes.is_empty() {
            store.ingest_bytes(&bytes);
            persist_empty_payload_chunk_dir(directory, &store)?
        } else {
            let plan = CarBuildPlan::single_file_with_profile(&bytes, descriptor.profile)
                .map_err(|err| format!("failed to build chunk plan for persistence: {err}"))?;
            let mut source = InMemoryPayload::new(&bytes);
            store
                .ingest_plan_to_directory(&plan, &mut source, directory)
                .map_err(|err| {
                    format!("failed to persist chunks to {}: {err}", directory.display())
                })?
        };
        Some(persisted_chunks_to_value(directory, output))
    } else {
        store.ingest_bytes(&bytes);
        None
    };

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
    if let Some(persisted) = persisted_chunks {
        root.insert("persisted_chunks".into(), persisted);
    }

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

fn preflight_chunk_dir_out(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err("--chunk-dir-out must not be empty".to_string());
    }
    if path == Path::new("-") {
        return Err("--chunk-dir-out must be a directory path, not stdout".to_string());
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "--chunk-dir-out {} must not be a symlink",
                    path.display()
                ));
            }
            if !metadata.is_dir() {
                return Err(format!(
                    "--chunk-dir-out {} must be a directory when it exists",
                    path.display()
                ));
            }
            let mut entries = fs::read_dir(path)
                .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
            if entries
                .next()
                .transpose()
                .map_err(|err| format!("failed to inspect {}: {err}", path.display()))?
                .is_some()
            {
                return Err(format!(
                    "--chunk-dir-out {} must be empty or absent",
                    path.display()
                ));
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(format!("failed to inspect {}: {err}", path.display())),
    }
    Ok(())
}

fn persisted_chunks_to_value(directory: &Path, output: DirectoryChunkSinkOutput) -> Value {
    let mut root = Map::new();
    root.insert(
        "directory".into(),
        Value::from(directory.display().to_string()),
    );
    root.insert("total_bytes".into(), Value::from(output.total_bytes));
    let records = output
        .records
        .into_iter()
        .map(|record| {
            let mut obj = Map::new();
            obj.insert("file_name".into(), Value::from(record.file_name));
            obj.insert("offset".into(), Value::from(record.offset));
            obj.insert("length".into(), Value::from(record.length));
            obj.insert("digest_blake3".into(), Value::from(to_hex(&record.digest)));
            Value::Object(obj)
        })
        .collect();
    root.insert("records".into(), Value::Array(records));
    Value::Object(root)
}

fn persist_empty_payload_chunk_dir(
    directory: &Path,
    store: &ChunkStore,
) -> Result<DirectoryChunkSinkOutput, String> {
    fs::create_dir_all(directory)
        .map_err(|err| format!("failed to create {}: {err}", directory.display()))?;
    let chunk = store
        .chunks()
        .first()
        .ok_or_else(|| "empty payload did not produce a logical chunk".to_string())?;
    let file_name = "chunk_00000.bin".to_string();
    let path = directory.join(&file_name);
    let file = fs::File::create(&path)
        .map_err(|err| format!("failed to create {}: {err}", path.display()))?;
    file.sync_all()
        .map_err(|err| format!("failed to sync {}: {err}", path.display()))?;
    Ok(DirectoryChunkSinkOutput {
        records: vec![PersistedChunkRecord {
            file_name,
            offset: chunk.offset,
            length: chunk.length,
            digest: chunk.blake3,
        }],
        total_bytes: u64::from(chunk.length),
    })
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
    let mut file = open_output_file(path, "text output")?;
    file.write_all(text.as_bytes())
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    Ok(false)
}

fn open_output_file(path: &Path, label: &str) -> Result<fs::File, String> {
    validate_output_path(path)?;
    ensure_parent_dir(path)?;
    validate_output_path(path)?;
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    set_no_follow_flag(&mut options);
    let file = options
        .open(path)
        .map_err(|err| format!("failed to open {label} {}: {err}", path.display()))?;
    let metadata = file.metadata().map_err(|err| {
        format!(
            "failed to inspect {label} {} after open: {err}",
            path.display()
        )
    })?;
    if !metadata.is_file() {
        return Err(format!(
            "failed to write {label} {}: output must be a regular file",
            path.display()
        ));
    }
    Ok(file)
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

#[cfg(any(target_os = "linux", target_os = "android"))]
fn platform_no_follow_flag() -> i32 {
    0o400000
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
fn platform_no_follow_flag() -> i32 {
    0
}

fn resolve_profile_handle(input: &str) -> Result<String, String> {
    if input.is_empty() {
        return Err("chunker profile cannot be empty".into());
    }
    if input != input.trim() {
        return Err("chunker profile must not contain leading or trailing whitespace".into());
    }
    if let Some(descriptor) = chunker_registry::lookup_by_handle(input) {
        return Ok(format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        ));
    }
    if input.as_bytes().iter().all(u8::is_ascii_digit) {
        let id = parse_u32_decimal(input, "chunker profile id")?;
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
            .any(|alias| alias.eq_ignore_ascii_case(input))
    }) {
        return Ok(format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        ));
    }
    Err(format!(
        "unknown chunker profile handle '{input}'. expected namespace.name@semver"
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

fn parse_profile_handle_arg(value: &str, label: &str) -> Result<String, String> {
    if value.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if value != value.trim() {
        return Err(format!(
            "{label} must not contain leading or trailing whitespace"
        ));
    }
    Ok(value.to_string())
}

fn parse_u32_decimal(value: &str, label: &str) -> Result<u32, String> {
    require_canonical_unsigned_decimal(value, label)?;
    value
        .parse::<u32>()
        .map_err(|err| format!("{label} value out of range: {err}"))
}

fn parse_nonzero_usize_decimal(value: &str, label: &str) -> Result<usize, String> {
    let parsed = parse_usize_decimal(value, label)?;
    if parsed == 0 {
        return Err(format!("{label} must be greater than zero"));
    }
    Ok(parsed)
}

fn parse_usize_decimal(value: &str, label: &str) -> Result<usize, String> {
    require_canonical_unsigned_decimal(value, label)?;
    value
        .parse::<usize>()
        .map_err(|err| format!("{label} value out of range: {err}"))
}

fn parse_u64(value: &str, label: &str) -> Result<u64, String> {
    if let Some(hex) = value.strip_prefix("0x") {
        require_canonical_hex_unsigned(hex, label)?;
        u64::from_str_radix(hex, 16).map_err(|err| format!("{label} value out of range: {err}"))
    } else {
        require_canonical_unsigned_decimal(value, label)?;
        value
            .parse::<u64>()
            .map_err(|err| format!("{label} value out of range: {err}"))
    }
}

fn require_canonical_unsigned_decimal(value: &str, label: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    if !bytes.is_empty()
        && bytes.iter().all(u8::is_ascii_digit)
        && (bytes.len() == 1 || bytes[0] != b'0')
    {
        Ok(())
    } else {
        Err(format!(
            "{label} must be a canonical unsigned decimal integer"
        ))
    }
}

fn require_canonical_hex_unsigned(value: &str, label: &str) -> Result<(), String> {
    let bytes = value.as_bytes();
    if !bytes.is_empty()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
        && (bytes.len() == 1 || bytes[0] != b'0')
    {
        Ok(())
    } else {
        Err(format!(
            "{label} must be a canonical unsigned decimal integer or lowercase 0x-prefixed hex"
        ))
    }
}

#[cfg(test)]
mod tests {
    use norito::json::Value;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn write_text_creates_parent_and_writes_all_bytes() {
        let temp = tempdir().expect("tempdir");
        let temp_path = temp.path().canonicalize().expect("canonical tempdir");
        let output_path = temp_path.join("nested").join("report.json");

        let wrote_stdout = write_text(&output_path, "{\"ok\":true}\n").expect("write text");

        assert!(!wrote_stdout);
        assert_eq!(
            fs::read(&output_path).expect("read output"),
            b"{\"ok\":true}\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_text_rejects_symlink_output() {
        let temp = tempdir().expect("tempdir");
        let temp_path = temp.path().canonicalize().expect("canonical tempdir");
        let target_path = temp_path.join("target.json");
        fs::write(&target_path, b"unchanged\n").expect("write target");
        let output_path = temp_path.join("report.json");
        std::os::unix::fs::symlink(&target_path, &output_path).expect("create symlink");

        let err = write_text(&output_path, "changed\n").expect_err("reject symlink output");

        assert!(
            err.contains("must not be a symlink"),
            "unexpected error: {err}"
        );
        assert_eq!(fs::read(&target_path).expect("read target"), b"unchanged\n");
    }

    #[cfg(unix)]
    #[test]
    fn write_text_rejects_symlink_parent() {
        let temp = tempdir().expect("tempdir");
        let temp_path = temp.path().canonicalize().expect("canonical tempdir");
        let real_dir = temp_path.join("real");
        fs::create_dir(&real_dir).expect("create real dir");
        let linked_dir = temp_path.join("linked");
        std::os::unix::fs::symlink(&real_dir, &linked_dir).expect("create symlink");
        let output_path = linked_dir.join("report.json");

        let err = write_text(&output_path, "changed\n").expect_err("reject symlink parent");

        assert!(
            err.contains("parent") && err.contains("must not be a symlink"),
            "unexpected error: {err}"
        );
        assert!(
            !real_dir.join("report.json").exists(),
            "symlink parent should not receive output"
        );
    }

    #[test]
    fn registry_lookup_round_trips_profile_id() {
        let descriptor = chunker_registry::default_descriptor();
        let looked_up = chunker_registry::lookup(descriptor.id).expect("descriptor present");
        assert!(std::ptr::eq(descriptor, looked_up));
    }

    #[test]
    fn parse_profile_handle_arg_rejects_empty_and_padded_handles() {
        assert_eq!(
            parse_profile_handle_arg("sorafs.sf1@1.0.0", "--profile").expect("canonical profile"),
            "sorafs.sf1@1.0.0"
        );

        for value in ["", " sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0 "] {
            let err =
                parse_profile_handle_arg(value, "--profile").expect_err("invalid profile handle");
            assert!(
                err.contains("empty") || err.contains("whitespace"),
                "unexpected error for {value:?}: {err}"
            );
        }
    }

    #[test]
    fn parse_decimal_flags_reject_noncanonical_tokens() {
        assert_eq!(
            parse_u32_decimal("1", "--profile-id").expect("profile id"),
            1
        );
        assert_eq!(
            parse_nonzero_usize_decimal("3", "--por-sample").expect("sample count"),
            3
        );

        for value in ["", "01", "00", "+1", " 1", "1 ", "0x1"] {
            let err =
                parse_u32_decimal(value, "--profile-id").expect_err("invalid u32 token must fail");
            assert!(
                err.contains("canonical unsigned decimal"),
                "unexpected error for {value:?}: {err}"
            );
        }

        let zero = parse_nonzero_usize_decimal("0", "--por-sample")
            .expect_err("zero sample count must fail");
        assert!(
            zero.contains("greater than zero"),
            "unexpected zero error: {zero}"
        );
    }

    #[test]
    fn parse_u64_seed_rejects_noncanonical_tokens() {
        assert_eq!(parse_u64("0", "--por-sample-seed").expect("zero"), 0);
        assert_eq!(parse_u64("42", "--por-sample-seed").expect("decimal"), 42);
        assert_eq!(parse_u64("0x0", "--por-sample-seed").expect("hex zero"), 0);
        assert_eq!(
            parse_u64("0xff", "--por-sample-seed").expect("hex seed"),
            255
        );

        for value in [
            "",
            "00",
            "01",
            "+1",
            " 1",
            "1 ",
            "0Xff",
            "0x",
            "0x0f",
            "0xFF",
            "18446744073709551616",
        ] {
            let err =
                parse_u64(value, "--por-sample-seed").expect_err("invalid seed token must fail");
            assert!(
                err.contains("canonical unsigned")
                    || err.contains("out of range")
                    || err.contains("too large"),
                "unexpected error for {value:?}: {err}"
            );
        }
    }

    #[test]
    fn resolve_profile_handle_requires_canonical_numeric_ids() {
        assert_eq!(
            resolve_profile_handle("sorafs.sf1@1.0.0").expect("handle resolves"),
            "sorafs.sf1@1.0.0"
        );
        assert_eq!(
            resolve_profile_handle("1").expect("numeric id resolves"),
            "sorafs.sf1@1.0.0"
        );
        assert_eq!(
            resolve_profile_handle("sorafs-sf1").expect("alias resolves"),
            "sorafs.sf1@1.0.0"
        );

        let padded = resolve_profile_handle("01").expect_err("padded id must fail");
        assert!(
            padded.contains("canonical unsigned decimal"),
            "unexpected padded id error: {padded}"
        );

        let whitespace =
            resolve_profile_handle(" sorafs.sf1@1.0.0").expect_err("whitespace must fail");
        assert!(
            whitespace.contains("whitespace"),
            "unexpected whitespace error: {whitespace}"
        );
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
    fn preflight_chunk_dir_out_rejects_empty_path() {
        let error = preflight_chunk_dir_out(Path::new("")).expect_err("empty path rejected");
        assert!(error.contains("must not be empty"));
    }

    #[test]
    fn persisted_chunks_to_value_includes_records() {
        let value = persisted_chunks_to_value(
            Path::new("chunks"),
            DirectoryChunkSinkOutput {
                records: vec![sorafs_car::PersistedChunkRecord {
                    file_name: "chunk_00000.bin".to_string(),
                    offset: 0,
                    length: 3,
                    digest: [7u8; 32],
                }],
                total_bytes: 3,
            },
        );
        let object = value.as_object().expect("persisted chunks object");
        assert_eq!(
            object.get("directory").and_then(Value::as_str),
            Some("chunks")
        );
        assert_eq!(object.get("total_bytes").and_then(Value::as_u64), Some(3));
        let records = object
            .get("records")
            .and_then(Value::as_array)
            .expect("records array");
        assert_eq!(records.len(), 1);
        let record = records[0].as_object().expect("record object");
        assert_eq!(
            record.get("file_name").and_then(Value::as_str),
            Some("chunk_00000.bin")
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
