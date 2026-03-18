//! Generates chunk metadata and a Norito manifest stub for a given payload.
use std::{
    env,
    fs::{self, File, read},
    io::{self, BufReader, BufWriter, Cursor, Read, Write},
    path::{Path, PathBuf},
    process,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use blake3::Hash;
use iroha_crypto::{Algorithm, HybridPublicKey, HybridSuite, PublicKey, Signature};
use norito::{
    json::{Map, Value, to_string_pretty},
    to_bytes,
};
use rand::rng;
use sha3::{Digest, Sha3_256};
use sorafs_car::{
    CarBuildPlan, CarChunk, CarStreamingWriter, ChunkStore, DirectoryPayload, FilePayload,
    FilePlan, InMemoryPayload, PorMerkleTree,
    fetch_plan::{chunk_fetch_specs_from_json, chunk_fetch_specs_to_json},
    por_json::{parse_proof_spec, proof_from_value, proof_to_value, sample_to_map, tree_to_value},
};
use sorafs_manifest::{
    AliasClaim, ChunkingProfileV1, DagCodecId, GovernanceProofs, ManifestBuilder, PinPolicy,
    ProfileId, StorageClass, chunker_registry,
    hybrid_envelope::{HybridPayloadEnvelopeV1, encrypt_payload},
};

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        process::exit(1);
    }
}

#[derive(Clone)]
enum JsonSource {
    File(PathBuf),
    Stdin,
}

#[path = "sorafs_manifest_stub/capacity.rs"]
mod capacity;
#[path = "sorafs_manifest_stub/provider_admission.rs"]
mod provider_admission;

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let first_arg = args.next().ok_or_else(|| usage().to_string())?;

    if first_arg == "capacity" {
        let remaining: Vec<String> = args.collect();
        return capacity::run(remaining.into_iter());
    }

    if first_arg == "provider-admission" {
        let remaining: Vec<String> = args.collect();
        return provider_admission::run(remaining.into_iter());
    }

    if first_arg == "--list-chunker-profiles" {
        return list_chunker_profiles(args);
    }

    let input_arg = first_arg;
    let mut opts = Options::default();

    for arg in args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value option, got: {arg}"))?;
        match key {
            "--plan" => {
                opts.plan_in = Some(if value == "-" {
                    JsonSource::Stdin
                } else {
                    JsonSource::File(PathBuf::from(value))
                });
            }
            "--root-cid" => opts.root_cid = Some(parse_hex_vec(value)?),
            "--dag-codec" => opts.dag_codec = Some(parse_u64(value)?),
            "--car-digest" => opts.car_digest = Some(parse_hex_array(value)?),
            "--car-size" => opts.car_size = Some(parse_u64(value)?),
            "--car-cid" => opts.car_cid = Some(parse_hex_vec(value)?),
            "--chunker-profile-id" => {
                let id = parse_u32(value)?;
                if let Some(existing) = opts.chunker_profile_id
                    && existing != id
                {
                    return Err("chunker profile specified multiple times".to_string());
                }
                opts.chunker_profile_id = Some(id);
            }
            "--chunker-profile" => {
                let handle = value.trim();
                let descriptor = chunker_registry::lookup_by_handle(handle).ok_or_else(|| {
                    format!(
                        "unknown chunker profile handle: {handle}. expected namespace.name@semver"
                    )
                })?;
                let id = descriptor.id.0;
                if let Some(existing) = opts.chunker_profile_id
                    && existing != id
                {
                    return Err(
                            "chunker profile specified via both --chunker-profile-id and --chunker-profile"
                                .to_string(),
                    );
                }
                opts.chunker_profile_id = Some(id);
            }
            "--min-replicas" => opts.min_replicas = Some(parse_u16(value)?),
            "--storage-class" => opts.storage_class = Some(parse_storage_class(value)?),
            "--retention-epoch" => opts.retention_epoch = Some(parse_u64(value)?),
            "--alias" => opts.alias_claims.push(parse_alias_hex(value)?),
            "--alias-file" => opts.alias_claims.push(parse_alias_file(value)?),
            "--metadata" => opts.metadata.push(parse_metadata(value)?),
            "--council-signature" => push_council_signature(&mut opts, value)?,
            "--council-signature-file" => push_council_signature_file(&mut opts, value)?,
            "--council-signature-public-key" => {
                opts.council_signature_public = Some(parse_hex_vec(value)?)
            }
            "--council-signature-public-key-file" => {
                opts.council_signature_public = Some(read_file_bytes(value)?)
            }
            "--hybrid-recipient-x25519" => {
                set_unique_vec(
                    &mut opts.hybrid_public_x25519,
                    parse_hex_vec(value)?,
                    "--hybrid-recipient-x25519",
                )?;
            }
            "--hybrid-recipient-x25519-file" => {
                set_unique_vec(
                    &mut opts.hybrid_public_x25519,
                    read_file_bytes(value)?,
                    "--hybrid-recipient-x25519-file",
                )?;
            }
            "--hybrid-recipient-kyber" => {
                set_unique_vec(
                    &mut opts.hybrid_public_kyber,
                    parse_hex_vec(value)?,
                    "--hybrid-recipient-kyber",
                )?;
            }
            "--hybrid-recipient-kyber-file" => {
                set_unique_vec(
                    &mut opts.hybrid_public_kyber,
                    read_file_bytes(value)?,
                    "--hybrid-recipient-kyber-file",
                )?;
            }
            "--manifest-out" => opts.manifest_out = Some(PathBuf::from(value)),
            "--manifest-signatures-out" => {
                opts.manifest_signatures_out = Some(PathBuf::from(value))
            }
            "--manifest-signatures-in" => opts.manifest_signatures_in = Some(PathBuf::from(value)),
            "--car-out" => opts.car_out = Some(PathBuf::from(value)),
            "--json-out" => opts.json_out = Some(PathBuf::from(value)),
            "--hybrid-envelope-out" => {
                opts.hybrid_envelope_out = Some(PathBuf::from(value));
            }
            "--hybrid-envelope-json-out" => {
                opts.hybrid_envelope_json_out = Some(PathBuf::from(value));
            }
            "--por-json-out" => opts.por_json_out = Some(PathBuf::from(value)),
            "--chunk-fetch-plan-out" => opts.chunk_fetch_plan_out = Some(PathBuf::from(value)),
            "--por-proof" => opts.por_proof = Some(parse_proof_spec(value)?),
            "--por-proof-out" => opts.por_proof_out = Some(PathBuf::from(value)),
            "--por-proof-verify" => opts.por_proof_verify = Some(PathBuf::from(value)),
            "--por-sample" => {
                let count = value
                    .parse::<usize>()
                    .map_err(|err| format!("invalid --por-sample count: {err}"))?;
                opts.por_sample_count = Some(count);
            }
            "--por-sample-seed" => {
                let seed = parse_u64(value)
                    .map_err(|err| format!("invalid --por-sample-seed value: {err}"))?;
                opts.por_sample_seed = Some(seed);
            }
            "--por-sample-out" => opts.por_sample_out = Some(PathBuf::from(value)),
            "--public-key-out" => opts.public_key_out = Some(PathBuf::from(value)),
            "--signature-out" => opts.signature_out = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown option: {key}")),
        }
    }

    let descriptor = if let Some(id) = opts.chunker_profile_id {
        chunker_registry::lookup(ProfileId(id)).ok_or_else(|| {
            format!("unknown chunker profile id {id}; see sorafs/chunker_registry.md")
        })?
    } else {
        chunker_registry::default_descriptor()
    };

    let produce_hybrid_envelope = opts.hybrid_envelope_out.is_some()
        || opts.hybrid_envelope_json_out.is_some()
        || opts.hybrid_public_x25519.is_some()
        || opts.hybrid_public_kyber.is_some();

    if produce_hybrid_envelope {
        if opts.hybrid_public_x25519.is_none() {
            return Err(
                "hybrid manifest envelopes require --hybrid-recipient-x25519 or --hybrid-recipient-x25519-file"
                    .into(),
            );
        }
        if opts.hybrid_public_kyber.is_none() {
            return Err(
                "hybrid manifest envelopes require --hybrid-recipient-kyber or --hybrid-recipient-kyber-file"
                    .into(),
            );
        }
        ensure_metadata_entry(&mut opts.metadata, "manifest.requires_envelope", "true");
        let suite_label = HybridSuite::X25519MlKem768ChaCha20Poly1305.to_string();
        ensure_metadata_entry(&mut opts.metadata, "manifest.hybrid_suite", &suite_label);
    }

    if opts.manifest_signatures_out.is_some() && opts.manifest_out.is_none() {
        return Err(
            "--manifest-signatures-out requires --manifest-out=<path> to name the manifest file"
                .to_string(),
        );
    }

    let input_kind = if input_arg == "-" {
        InputKind::Stdin
    } else {
        let src_path = Path::new(&input_arg);
        if src_path.is_dir() {
            InputKind::Directory(src_path.to_path_buf())
        } else {
            InputKind::File(src_path.to_path_buf())
        }
    };

    if matches!(input_kind, InputKind::Stdin) && matches!(opts.plan_in, Some(JsonSource::Stdin)) {
        return Err("cannot read both payload and chunk fetch plan from stdin".into());
    }

    let (car_plan, mut payload) = match &input_kind {
        InputKind::Stdin => {
            let data = read_input(&input_arg)?;
            let plan = CarBuildPlan::single_file_with_profile(&data, descriptor.profile)
                .map_err(|err| format!("car planning failed: {err}"))?;
            (plan, data)
        }
        InputKind::File(_) => {
            let data = read_input(&input_arg)?;
            let plan = CarBuildPlan::single_file_with_profile(&data, descriptor.profile)
                .map_err(|err| format!("car planning failed: {err}"))?;
            (plan, data)
        }
        InputKind::Directory(root) => {
            CarBuildPlan::from_directory_with_profile(root.as_path(), descriptor.profile)
                .map_err(|err| format!("car planning failed: {err}"))?
        }
    };
    if let Some(source) = opts.plan_in.as_ref() {
        let plan_value = load_json_source(source)?;
        let specs = chunk_fetch_specs_from_json(&plan_value)
            .map_err(|err| format!("failed to parse chunk fetch specs: {err}"))?;
        if specs.len() != car_plan.chunks.len() {
            return Err(format!(
                "chunk fetch specs length {} does not match computed plan length {}",
                specs.len(),
                car_plan.chunks.len()
            ));
        }
        for (index, (chunk, spec)) in car_plan.chunks.iter().zip(specs.iter()).enumerate() {
            if spec.chunk_index != index {
                return Err(format!(
                    "chunk fetch spec index {} does not match expected chunk index {}",
                    spec.chunk_index, index
                ));
            }
            if spec.offset != chunk.offset {
                return Err(format!(
                    "chunk fetch spec offset {} does not match computed offset {} for chunk {}",
                    spec.offset, chunk.offset, index
                ));
            }
            if spec.length != chunk.length {
                return Err(format!(
                    "chunk fetch spec length {} does not match computed length {} for chunk {}",
                    spec.length, chunk.length, index
                ));
            }
            if spec.digest != chunk.digest {
                return Err(format!(
                    "chunk fetch spec digest {} does not match computed digest {} for chunk {}",
                    to_hex(&spec.digest),
                    to_hex(&chunk.digest),
                    index
                ));
            }
        }
    }

    if car_plan.chunk_profile != descriptor.profile {
        return Err("computed chunk plan used unexpected profile".into());
    }

    let mut chunk_store = ChunkStore::with_profile(descriptor.profile);
    match &input_kind {
        InputKind::Directory(root) => {
            let mut source = DirectoryPayload::new(root.as_path(), &car_plan.files)
                .map_err(|err| format!("failed to reopen directory payload {root:?}: {err}"))?;
            chunk_store
                .ingest_plan_source(&car_plan, &mut source)
                .map_err(|err| format!("failed to ingest directory payload: {err}"))?;
            if opts.car_out.is_some() {
                payload.clear();
                payload.shrink_to_fit();
            }
        }
        InputKind::File(path) => {
            let mut source = FilePayload::open(path)
                .map_err(|err| format!("failed to reopen file payload {path:?}: {err}"))?;
            chunk_store
                .ingest_plan_source(&car_plan, &mut source)
                .map_err(|err| format!("failed to ingest file payload: {err}"))?;
            if opts.car_out.is_some() {
                payload.clear();
                payload.shrink_to_fit();
            }
        }
        InputKind::Stdin => {
            chunk_store.ingest_plan(&payload, &car_plan);
        }
    }
    if chunk_store.por_tree().chunks().len() != car_plan.chunks.len() {
        return Err("chunk store PoR layout diverged from CAR plan".into());
    }

    if car_plan.chunk_profile != descriptor.profile {
        return Err("computed chunk plan used unexpected profile".into());
    }

    let chunk_profile = ChunkingProfileV1::from_descriptor(descriptor);

    if opts.council_signatures.is_empty() {
        return Err("specify at least one --council-signature".into());
    }
    if opts.council_signature_public.is_some() && opts.council_signatures.len() != 1 {
        return Err(
            "when using --council-signature-public-key, provide exactly one --council-signature"
                .into(),
        );
    }

    if opts.council_signature_public.is_some() && opts.council_signatures.len() != 1 {
        return Err(
            "when using --council-signature-public-key, provide exactly one --council-signature"
                .into(),
        );
    }

    let car_stats = if let Some(path) = &opts.car_out {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
            && !parent.exists()
        {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {parent:?}: {err}"))?;
        }
        let file = File::create(path).map_err(|err| format!("failed to create {path:?}: {err}"))?;
        let mut writer = BufWriter::new(file);
        let stats = match &input_kind {
            InputKind::File(src) => {
                let file = File::open(src)
                    .map_err(|err| format!("failed to reopen input {src:?}: {err}"))?;
                let mut reader = BufReader::new(file);
                CarStreamingWriter::new(&car_plan)
                    .write_from_reader(&mut reader, &mut writer)
                    .map_err(|err| err.to_string())?
            }
            InputKind::Directory(root) => {
                let mut reader = DirectoryPlanReader::new(root.as_path(), &car_plan.files)
                    .map_err(|err| format!("failed to stream directory input {root:?}: {err}"))?;
                CarStreamingWriter::new(&car_plan)
                    .write_from_reader(&mut reader, &mut writer)
                    .map_err(|err| err.to_string())?
            }
            InputKind::Stdin => {
                let mut reader = Cursor::new(&payload);
                CarStreamingWriter::new(&car_plan)
                    .write_from_reader(&mut reader, &mut writer)
                    .map_err(|err| err.to_string())?
            }
        };
        writer
            .flush()
            .map_err(|err| format!("failed to flush {path:?}: {err}"))?;
        stats
    } else {
        let mut reader = Cursor::new(&payload);
        let mut sink = io::sink();
        CarStreamingWriter::new(&car_plan)
            .write_from_reader(&mut reader, &mut sink)
            .map_err(|err| err.to_string())?
    };

    if car_stats.chunk_profile != descriptor.profile {
        return Err("computed CAR used unexpected chunking profile".into());
    }

    let computed_root = car_stats
        .root_cids
        .first()
        .cloned()
        .ok_or_else(|| "CAR emission produced no root CID".to_string())?;

    if let Some(expected) = opts.root_cid.as_ref()
        && expected != &computed_root
    {
        return Err("provided --root-cid does not match computed CAR root".into());
    }

    if let Some(expected_codec) = opts.dag_codec
        && expected_codec != car_stats.dag_codec
    {
        return Err("provided --dag-codec does not match computed root codec".into());
    }
    let dag_codec = car_stats.dag_codec;

    let mut computed_car_payload_digest = [0u8; 32];
    computed_car_payload_digest.copy_from_slice(car_stats.car_payload_digest.as_bytes());
    let car_payload_digest = match opts.car_digest {
        Some(provided) => {
            if provided != computed_car_payload_digest {
                return Err("provided --car-digest does not match CAR output".into());
            }
            provided
        }
        None => computed_car_payload_digest,
    };

    let car_size = match opts.car_size {
        Some(expected) => {
            if expected != car_stats.car_size {
                return Err("provided --car-size does not match CAR output".into());
            }
            expected
        }
        None => car_stats.car_size,
    };

    if let Some(expected) = opts.car_cid.as_ref()
        && expected != &car_stats.car_cid
    {
        return Err("provided --car-cid does not match CAR output".into());
    }

    let mut por_proof_json: Option<Value> = None;
    let mut por_proof_verified = false;
    let mut por_samples: Option<Vec<Value>> = None;
    let mut por_samples_truncated = false;

    if let Some((chunk_idx, segment_idx, leaf_idx)) = opts.por_proof {
        let proof_result = match &input_kind {
            InputKind::Stdin => {
                let mut source = InMemoryPayload::new(&payload);
                chunk_store.por_tree().prove_leaf_with(
                    chunk_idx,
                    segment_idx,
                    leaf_idx,
                    &mut source,
                )
            }
            InputKind::File(path) => {
                let mut source = FilePayload::open(path)
                    .map_err(|err| format!("failed to reopen file payload {path:?}: {err}"))?;
                chunk_store.por_tree().prove_leaf_with(
                    chunk_idx,
                    segment_idx,
                    leaf_idx,
                    &mut source,
                )
            }
            InputKind::Directory(root) => {
                let mut source = DirectoryPayload::new(root.as_path(), &car_plan.files)
                    .map_err(|err| format!("failed to reopen directory payload {root:?}: {err}"))?;
                chunk_store.por_tree().prove_leaf_with(
                    chunk_idx,
                    segment_idx,
                    leaf_idx,
                    &mut source,
                )
            }
        }
        .map_err(|err| format!("failed to build PoR proof: {err}"))?;
        let proof = proof_result.ok_or_else(|| {
            format!(
                "invalid --por-proof indices chunk={chunk_idx} segment={segment_idx} leaf={leaf_idx}"
            )
        })?;
        let proof_value = proof_to_value(&proof);
        if let Some(path) = &opts.por_proof_out {
            let mut serialized = to_string_pretty(&proof_value)
                .map_err(|err| format!("failed to serialise PoR proof: {err}"))?;
            serialized.push('\n');
            write_json(path, &serialized)?;
        }
        por_proof_json = Some(proof_value);
    }

    if let Some(path) = &opts.por_proof_verify {
        let proof_bytes =
            fs::read(path).map_err(|err| format!("failed to read PoR proof {path:?}: {err}"))?;
        let proof_value: Value =
            norito::json::from_slice(&proof_bytes).map_err(|err| err.to_string())?;
        let proof = proof_from_value(&proof_value)?;
        if !proof.verify(chunk_store.por_tree().root()) {
            return Err("provided PoR proof does not verify against computed root".into());
        }
        por_proof_verified = true;
        if por_proof_json.is_none() {
            por_proof_json = Some(proof_value);
        }
    }

    if let Some(count) = opts.por_sample_count {
        let total_leaves = chunk_store.por_tree().leaf_count();
        if total_leaves == 0 {
            return Err("cannot sample PoR leaves from an empty tree".into());
        }
        let seed = opts.por_sample_seed.unwrap_or(0x9e3779b97f4a7c15);
        let samples_vec = match &input_kind {
            InputKind::Stdin => {
                let mut source = InMemoryPayload::new(&payload);
                chunk_store
                    .sample_leaves_with(count, seed, &mut source)
                    .map_err(|err| format!("failed to sample PoR leaves: {err}"))?
            }
            InputKind::File(path) => {
                let mut source = FilePayload::open(path)
                    .map_err(|err| format!("failed to reopen file payload {path:?}: {err}"))?;
                chunk_store
                    .sample_leaves_with(count, seed, &mut source)
                    .map_err(|err| format!("failed to sample PoR leaves: {err}"))?
            }
            InputKind::Directory(root) => {
                let mut source = DirectoryPayload::new(root.as_path(), &car_plan.files)
                    .map_err(|err| format!("failed to reopen directory payload {root:?}: {err}"))?;
                chunk_store
                    .sample_leaves_with(count, seed, &mut source)
                    .map_err(|err| format!("failed to sample PoR leaves: {err}"))?
            }
        };
        if samples_vec.is_empty() {
            return Err("cannot sample PoR leaves from an empty tree".into());
        }
        if count > total_leaves || samples_vec.len() < count {
            por_samples_truncated = true;
        }
        let proofs: Vec<Value> = samples_vec
            .into_iter()
            .map(|(flat, proof)| Value::Object(sample_to_map(flat, &proof)))
            .collect();
        if let Some(path) = &opts.por_sample_out {
            let mut serialized =
                to_string_pretty(&Value::Array(proofs.clone())).map_err(|err| err.to_string())?;
            serialized.push('\n');
            write_json(path, &serialized)?;
        }
        por_samples = Some(proofs);
    }

    let manifest = ManifestBuilder::new()
        .root_cid(computed_root.clone())
        .dag_codec(DagCodecId(dag_codec))
        .chunking_profile(chunk_profile.clone())
        .content_length(car_plan.content_length)
        .car_digest(car_payload_digest)
        .car_size(car_size)
        .pin_policy(PinPolicy {
            min_replicas: opts.min_replicas.unwrap_or(3),
            storage_class: opts.storage_class.unwrap_or_default(),
            retention_epoch: opts.retention_epoch.unwrap_or(0),
        })
        .governance(GovernanceProofs {
            council_signatures: opts.council_signatures.clone(),
        })
        .extend_aliases(opts.alias_claims.into_iter())
        .extend_metadata(opts.metadata.into_iter())
        .build()
        .map_err(|err| err.to_string())?;

    let manifest_bytes = manifest.encode().map_err(|err| err.to_string())?;
    let manifest_digest = manifest.digest().map_err(|err| err.to_string())?;
    let manifest_filename = opts.manifest_out.as_ref().and_then(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
    });
    let chunk_digest_sha3 = compute_chunk_digest_sha3(&car_plan.chunks);
    let mut hybrid_output: Option<HybridEnvelopeArtefact> = None;

    if produce_hybrid_envelope {
        let recipient = HybridPublicKey::from_bytes(
            opts.hybrid_public_x25519
                .as_ref()
                .expect("recipient x25519 key checked above"),
            opts.hybrid_public_kyber
                .as_ref()
                .expect("recipient kyber key checked above"),
        )
        .map_err(|err| format!("invalid hybrid recipient key material: {err}"))?;
        let aad = build_hybrid_manifest_aad(
            &manifest_digest,
            chunk_digest_sha3,
            manifest_filename.as_deref(),
        );
        let mut rng = rng();
        let envelope = encrypt_payload(&manifest_bytes, &aad, &recipient, &mut rng)
            .map_err(|err| format!("failed to encrypt hybrid payload envelope: {err}"))?;
        let envelope_bytes = to_bytes(&envelope)
            .map_err(|err| format!("failed to encode hybrid payload envelope: {err}"))?;
        hybrid_output = Some(HybridEnvelopeArtefact {
            envelope,
            bytes: envelope_bytes,
            aad,
        });
    }

    if let Some(path) = &opts.manifest_out {
        write_manifest(path, &manifest_bytes)?;
    }

    if let Some(path) = &opts.manifest_signatures_in {
        verify_manifest_signatures_file(
            path,
            descriptor,
            &manifest_digest,
            chunk_digest_sha3,
            manifest_filename.as_deref(),
        )?;
    }

    if let Some(path) = &opts.manifest_signatures_out {
        if manifest.governance.council_signatures.is_empty() {
            return Err("--manifest-signatures-out requires at least one council signature".into());
        }
        let manifest_filename = manifest_filename.as_deref().ok_or_else(|| {
            "manifest filename unavailable; provide --manifest-out=<path>".to_string()
        })?;
        write_manifest_signatures_file(
            path,
            descriptor,
            &manifest,
            &manifest_digest,
            chunk_digest_sha3,
            manifest_filename,
        )?;
    }

    if let Some(hybrid) = hybrid_output.as_ref() {
        if let Some(path) = &opts.hybrid_envelope_out {
            write_binary(path, &hybrid.bytes)?;
        }
        if let Some(path) = &opts.hybrid_envelope_json_out {
            let json_value = norito::json::to_value(&hybrid.envelope)
                .map_err(|err| format!("failed to encode hybrid envelope JSON: {err}"))?;
            let mut json_string = to_string_pretty(&json_value)
                .map_err(|err| format!("failed to render hybrid envelope JSON: {err}"))?;
            json_string.push('\n');
            write_json(path, &json_string)?;
        }
    }

    if let Some(path) = &opts.public_key_out {
        let first = manifest
            .governance
            .council_signatures
            .first()
            .ok_or_else(|| "manifest contains no council signatures".to_string())?;
        write_binary(path, &first.signer)?;
    }

    if let Some(path) = &opts.signature_out {
        let first = manifest
            .governance
            .council_signatures
            .first()
            .ok_or_else(|| "manifest contains no council signatures".to_string())?;
        write_binary(path, &first.signature)?;
    }

    let mut report = build_report(ReportContext {
        profile: &chunk_profile,
        plan: &car_plan,
        car_stats: &car_stats,
        root_cid: &computed_root,
        manifest: &manifest,
        manifest_bytes: &manifest_bytes,
        manifest_digest: &manifest_digest,
        por_tree: chunk_store.por_tree(),
    });
    let report_object = report
        .as_object_mut()
        .ok_or_else(|| "internal error: report root is not a JSON object".to_string())?;
    if por_proof_verified {
        report_object.insert("por_proof_verified".into(), Value::from(true));
    }
    if let Some(value) = por_proof_json {
        report_object.insert("por_proof".into(), value);
    }
    if let Some(samples) = por_samples {
        report_object.insert("por_samples".into(), Value::Array(samples));
    }
    if por_samples_truncated {
        report_object.insert("por_samples_truncated".into(), Value::from(true));
    }
    if let Some(path) = &opts.chunk_fetch_plan_out {
        let specs_value = chunk_fetch_specs_to_json(&car_plan);
        let mut specs_text = to_string_pretty(&specs_value)
            .map_err(|err| format!("failed to serialise chunk fetch specs: {err}"))?;
        specs_text.push('\n');
        write_json(path, &specs_text)?;
    }
    if let Some(hybrid) = hybrid_output.as_ref() {
        let mut obj = Map::new();
        obj.insert("suite".into(), Value::from(hybrid.envelope.suite.clone()));
        obj.insert(
            "nonce_hex".into(),
            Value::from(to_hex(&hybrid.envelope.nonce)),
        );
        obj.insert(
            "ciphertext_len".into(),
            Value::from(hybrid.envelope.ciphertext.len() as u64),
        );
        obj.insert(
            "ciphertext_blake3".into(),
            Value::from(to_hex(blake3::hash(&hybrid.envelope.ciphertext).as_bytes())),
        );
        obj.insert("aad_hex".into(), Value::from(to_hex(&hybrid.aad)));
        obj.insert(
            "encoded_base64".into(),
            Value::from(BASE64_STANDARD.encode(&hybrid.bytes)),
        );
        if let Some(path) = &opts.hybrid_envelope_out {
            obj.insert("binary_out".into(), Value::from(path.display().to_string()));
        }
        if let Some(path) = &opts.hybrid_envelope_json_out {
            obj.insert("json_out".into(), Value::from(path.display().to_string()));
        }
        report_object.insert("hybrid_envelope".into(), Value::Object(obj));
    }
    let mut report_string = to_string_pretty(&report)
        .map_err(|err| format!("failed to serialise JSON report: {err}"))?;
    report_string.push('\n');

    let mut report_written_to_stdout = false;
    if let Some(path) = &opts.json_out {
        if path == Path::new("-") {
            report_written_to_stdout = true;
        }
        write_json(path, &report_string)?;
    }

    if let Some(path) = &opts.por_json_out {
        let por_json = to_string_pretty(&tree_to_value(chunk_store.por_tree()))
            .map_err(|err| format!("failed to serialise PoR JSON: {err}"))?
            + "\n";
        write_json(path, &por_json)?;
    }

    if !report_written_to_stdout {
        print!("{report_string}");
        io::stdout()
            .flush()
            .map_err(|err| format!("failed to flush stdout: {err}"))?;
    }
    Ok(())
}

fn list_chunker_profiles(args: impl Iterator<Item = String>) -> Result<(), String> {
    let mut json_out: Option<PathBuf> = None;
    for arg in args {
        let (key, value) = arg
            .split_once('=')
            .ok_or_else(|| format!("expected key=value option, got: {arg}"))?;
        match key {
            "--json-out" => json_out = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown option: {key}")),
        }
    }

    let profiles: Vec<Value> = chunker_registry::registry()
        .iter()
        .map(descriptor_to_json)
        .collect();
    let json = to_string_pretty(&Value::Array(profiles))
        .map_err(|err| format!("failed to serialise JSON: {err}"))?
        + "\n";

    if let Some(path) = json_out {
        write_json(&path, &json)?;
        if path == Path::new("-") {
            return Ok(());
        }
    }

    print!("{json}");
    Ok(())
}

fn usage() -> &'static str {
    "usage: sorafs-manifest-stub <path|-> \
     [--root-cid=hex] (verifies computed root) \
     [--dag-codec=0x71] (verifies computed codec) \
     [--car-digest=hex] (defaults to computed payload BLAKE3) \
     [--car-size=bytes] (defaults to computed CAR size) \
     [--car-cid=hex] (verifies computed raw-encoded CAR CID) \
    [--chunker-profile-id=1 | --chunker-profile=sorafs.sf1@1.0.0] (choose registered chunker profile) \
     [--min-replicas=3] \
     [--storage-class=hot|warm|cold] \
     [--retention-epoch=0] \
     [--alias=name:namespace:proofhex] \
     [--alias-file=name:namespace:path] \
     [--council-signature=signerhex:signaturehex|signaturehex (after --council-signature-public-key)] \
     [--council-signature-file=signerhex:path|path (after --council-signature-public-key)] \
     [--council-signature-public-key=hex] \
     [--council-signature-public-key-file=path] \
     [--public-key-out=path] \
     [--signature-out=path] \
     [--hybrid-recipient-x25519=hex|--hybrid-recipient-x25519-file=path] \
     [--hybrid-recipient-kyber=hex|--hybrid-recipient-kyber-file=path] \
     [--hybrid-envelope-out=path] \
     [--hybrid-envelope-json-out=path] \
     [--metadata=key:value] \
     [--manifest-out=path] \
     [--manifest-signatures-in=path] \
     [--manifest-signatures-out=path] \
     [--car-out=path] \
     [--json-out=path] \
     [--chunk-fetch-plan-out=path] \
     [--plan=chunk_fetch_specs.json|-] \
     [--por-json-out=path] \
     [--por-proof=chunk:segment:leaf] \
     [--por-proof-out=path] \
     [--por-proof-verify=path] \
     [--por-sample=count] \
     [--por-sample-seed=value] \
     [--por-sample-out=path]

usage: sorafs-manifest-stub --list-chunker-profiles [--json-out=path]"
}

struct ReportContext<'a> {
    profile: &'a ChunkingProfileV1,
    plan: &'a CarBuildPlan,
    car_stats: &'a sorafs_car::CarWriteStats,
    root_cid: &'a [u8],
    manifest: &'a sorafs_manifest::ManifestV1,
    manifest_bytes: &'a [u8],
    manifest_digest: &'a Hash,
    por_tree: &'a PorMerkleTree,
}

fn build_report(ctx: ReportContext<'_>) -> Value {
    let chunk_digests: Vec<Value> = ctx
        .plan
        .chunks
        .iter()
        .map(|chunk| {
            let mut obj = Map::new();
            obj.insert("offset".into(), Value::from(chunk.offset));
            obj.insert("length".into(), Value::from(chunk.length));
            obj.insert("digest_blake3".into(), Value::from(to_hex(&chunk.digest)));
            Value::Object(obj)
        })
        .collect();

    let chunk_fetch_specs = chunk_fetch_specs_to_json(ctx.plan);

    let mut chunking_obj = Map::new();
    chunking_obj.insert(
        "namespace".into(),
        Value::from(ctx.profile.namespace.clone()),
    );
    chunking_obj.insert("name".into(), Value::from(ctx.profile.name.clone()));
    chunking_obj.insert("semver".into(), Value::from(ctx.profile.semver.clone()));
    chunking_obj.insert(
        "handle".into(),
        Value::from(format!(
            "{}.{}@{}",
            ctx.profile.namespace, ctx.profile.name, ctx.profile.semver
        )),
    );
    chunking_obj.insert("profile_id".into(), Value::from(ctx.profile.profile_id.0));
    let alias_values: Vec<Value> = ctx
        .profile
        .aliases
        .iter()
        .cloned()
        .map(Value::from)
        .collect();
    chunking_obj.insert("profile_aliases".into(), Value::Array(alias_values.clone()));
    chunking_obj.insert("min_size".into(), Value::from(ctx.profile.min_size as u64));
    chunking_obj.insert(
        "target_size".into(),
        Value::from(ctx.profile.target_size as u64),
    );
    chunking_obj.insert("max_size".into(), Value::from(ctx.profile.max_size as u64));
    chunking_obj.insert(
        "break_mask".into(),
        Value::from(format!("0x{:04x}", ctx.profile.break_mask)),
    );
    chunking_obj.insert(
        "multihash_code".into(),
        Value::from(ctx.profile.multihash_code),
    );

    let mut pin_policy_obj = Map::new();
    pin_policy_obj.insert(
        "min_replicas".into(),
        Value::from(ctx.manifest.pin_policy.min_replicas as u64),
    );
    pin_policy_obj.insert(
        "storage_class".into(),
        Value::from(format!("{:?}", ctx.manifest.pin_policy.storage_class)),
    );
    pin_policy_obj.insert(
        "retention_epoch".into(),
        Value::from(ctx.manifest.pin_policy.retention_epoch),
    );

    let alias_claims: Vec<Value> = ctx
        .manifest
        .alias_claims
        .iter()
        .map(|alias| {
            let mut obj = Map::new();
            obj.insert("name".into(), Value::from(alias.name.clone()));
            obj.insert("namespace".into(), Value::from(alias.namespace.clone()));
            obj.insert("proof_hex".into(), Value::from(to_hex(&alias.proof)));
            Value::Object(obj)
        })
        .collect();

    let metadata_entries: Vec<Value> = ctx
        .manifest
        .metadata
        .iter()
        .map(|entry| {
            let mut obj = Map::new();
            obj.insert("key".into(), Value::from(entry.key.clone()));
            obj.insert("value".into(), Value::from(entry.value.clone()));
            Value::Object(obj)
        })
        .collect();

    let mut manifest_obj = Map::new();
    manifest_obj.insert("version".into(), Value::from(ctx.manifest.version));
    manifest_obj.insert(
        "root_cid_hex".into(),
        Value::from(to_hex(&ctx.manifest.root_cid)),
    );
    manifest_obj.insert("dag_codec".into(), Value::from(ctx.manifest.dag_codec.0));
    manifest_obj.insert(
        "handle".into(),
        Value::from(format!(
            "{}.{}@{}",
            ctx.profile.namespace, ctx.profile.name, ctx.profile.semver
        )),
    );
    manifest_obj.insert("profile_aliases".into(), Value::Array(alias_values));
    manifest_obj.insert(
        "content_length".into(),
        Value::from(ctx.manifest.content_length),
    );
    manifest_obj.insert(
        "car_digest_hex".into(),
        Value::from(to_hex(&ctx.manifest.car_digest)),
    );
    manifest_obj.insert(
        "car_cid_hex".into(),
        Value::from(to_hex(&ctx.car_stats.car_cid)),
    );
    manifest_obj.insert("car_size".into(), Value::from(ctx.manifest.car_size));
    manifest_obj.insert("pin_policy".into(), Value::Object(pin_policy_obj));
    manifest_obj.insert(
        "digest_hex".into(),
        Value::from(to_hex(ctx.manifest_digest.as_bytes())),
    );
    manifest_obj.insert(
        "manifest_hex".into(),
        Value::from(to_hex(ctx.manifest_bytes)),
    );
    manifest_obj.insert(
        "manifest_len".into(),
        Value::from(ctx.manifest_bytes.len() as u64),
    );
    manifest_obj.insert("alias_claims".into(), Value::Array(alias_claims));
    manifest_obj.insert("metadata".into(), Value::Array(metadata_entries));
    let council_entries: Vec<Value> = ctx
        .manifest
        .governance
        .council_signatures
        .iter()
        .map(|sig| {
            let mut obj = Map::new();
            obj.insert("signer_hex".into(), Value::from(to_hex(&sig.signer)));
            obj.insert("signature_hex".into(), Value::from(to_hex(&sig.signature)));
            Value::Object(obj)
        })
        .collect();
    manifest_obj.insert("council_signatures".into(), Value::Array(council_entries));

    let mut report_obj = Map::new();
    report_obj.insert("chunking".into(), Value::Object(chunking_obj));
    report_obj.insert("chunk_digests".into(), Value::Array(chunk_digests));
    report_obj.insert("chunk_fetch_specs".into(), chunk_fetch_specs);
    report_obj.insert(
        "payload_digest_hex".into(),
        Value::from(to_hex(ctx.plan.payload_digest.as_bytes())),
    );
    report_obj.insert("car_size".into(), Value::from(ctx.car_stats.car_size));
    report_obj.insert(
        "car_payload_digest_hex".into(),
        Value::from(to_hex(ctx.car_stats.car_payload_digest.as_bytes())),
    );
    report_obj.insert(
        "car_archive_digest_hex".into(),
        Value::from(to_hex(ctx.car_stats.car_archive_digest.as_bytes())),
    );
    report_obj.insert(
        "car_cid_hex".into(),
        Value::from(to_hex(&ctx.car_stats.car_cid)),
    );
    report_obj.insert("car_root_hex".into(), Value::from(to_hex(ctx.root_cid)));
    report_obj.insert("dag_codec".into(), Value::from(ctx.car_stats.dag_codec));
    report_obj.insert("manifest".into(), Value::Object(manifest_obj));
    report_obj.insert(
        "manifest_digest_hex".into(),
        Value::from(to_hex(ctx.manifest_digest.as_bytes())),
    );
    report_obj.insert(
        "manifest_size".into(),
        Value::from(ctx.manifest_bytes.len() as u64),
    );
    report_obj.insert(
        "chunk_count".into(),
        Value::from(ctx.plan.chunks.len() as u64),
    );
    report_obj.insert(
        "por_root_hex".into(),
        Value::from(to_hex(ctx.por_tree.root())),
    );
    report_obj.insert(
        "por_chunk_count".into(),
        Value::from(ctx.por_tree.chunks().len() as u64),
    );

    Value::Object(report_obj)
}

fn descriptor_to_json(descriptor: &chunker_registry::ChunkerProfileDescriptor) -> Value {
    let mut obj = Map::new();
    obj.insert("profile_id".into(), Value::from(descriptor.id.0 as u64));
    obj.insert("namespace".into(), Value::from(descriptor.namespace));
    obj.insert("name".into(), Value::from(descriptor.name));
    obj.insert("semver".into(), Value::from(descriptor.semver));
    obj.insert(
        "min_size".into(),
        Value::from(descriptor.profile.min_size as u64),
    );
    obj.insert(
        "target_size".into(),
        Value::from(descriptor.profile.target_size as u64),
    );
    obj.insert(
        "max_size".into(),
        Value::from(descriptor.profile.max_size as u64),
    );
    obj.insert(
        "break_mask".into(),
        Value::from(format!("0x{:04x}", descriptor.profile.break_mask)),
    );
    obj.insert(
        "multihash_code".into(),
        Value::from(descriptor.multihash_code),
    );
    Value::Object(obj)
}

fn read_input(path: &str) -> Result<Vec<u8>, String> {
    if path == "-" {
        let mut buf = Vec::new();
        io::stdin()
            .read_to_end(&mut buf)
            .map_err(|err| format!("failed to read stdin: {err}"))?;
        return Ok(buf);
    }

    let path_ref = Path::new(path);
    let mut file = File::open(path_ref).map_err(|err| format!("failed to open {path}: {err}"))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|err| format!("failed to read {path}: {err}"))?;
    Ok(buf)
}

fn load_json_source(source: &JsonSource) -> Result<Value, String> {
    match source {
        JsonSource::File(path) => {
            let bytes = fs::read(path)
                .map_err(|err| format!("failed to read chunk fetch plan {path:?}: {err}"))?;
            norito::json::from_slice(&bytes)
                .map_err(|err| format!("failed to parse JSON from {path:?}: {err}"))
        }
        JsonSource::Stdin => {
            let mut buf = Vec::new();
            io::stdin()
                .read_to_end(&mut buf)
                .map_err(|err| format!("failed to read chunk fetch plan from stdin: {err}"))?;
            norito::json::from_slice(&buf)
                .map_err(|err| format!("failed to parse JSON chunk fetch plan from stdin: {err}"))
        }
    }
}

#[derive(Default)]
struct Options {
    plan_in: Option<JsonSource>,
    root_cid: Option<Vec<u8>>,
    dag_codec: Option<u64>,
    car_digest: Option<[u8; 32]>,
    car_size: Option<u64>,
    car_cid: Option<Vec<u8>>,
    chunker_profile_id: Option<u32>,
    min_replicas: Option<u16>,
    storage_class: Option<StorageClass>,
    retention_epoch: Option<u64>,
    alias_claims: Vec<AliasClaim>,
    metadata: Vec<(String, String)>,
    council_signatures: Vec<sorafs_manifest::CouncilSignature>,
    council_signature_public: Option<Vec<u8>>,
    public_key_out: Option<PathBuf>,
    signature_out: Option<PathBuf>,
    hybrid_public_x25519: Option<Vec<u8>>,
    hybrid_public_kyber: Option<Vec<u8>>,
    hybrid_envelope_out: Option<PathBuf>,
    hybrid_envelope_json_out: Option<PathBuf>,
    manifest_out: Option<PathBuf>,
    manifest_signatures_out: Option<PathBuf>,
    car_out: Option<PathBuf>,
    json_out: Option<PathBuf>,
    por_json_out: Option<PathBuf>,
    chunk_fetch_plan_out: Option<PathBuf>,
    por_proof: Option<(usize, usize, usize)>,
    por_proof_out: Option<PathBuf>,
    por_proof_verify: Option<PathBuf>,
    por_sample_count: Option<usize>,
    por_sample_seed: Option<u64>,
    por_sample_out: Option<PathBuf>,
    manifest_signatures_in: Option<PathBuf>,
}

enum InputKind {
    Stdin,
    File(PathBuf),
    Directory(PathBuf),
}

const HYBRID_MANIFEST_AAD_DOMAIN: &[u8] = b"sorafs.hybrid.manifest.v1";

struct HybridEnvelopeArtefact {
    envelope: HybridPayloadEnvelopeV1,
    bytes: Vec<u8>,
    aad: Vec<u8>,
}

fn parse_hex_vec(value: &str) -> Result<Vec<u8>, String> {
    if value.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::with_capacity(value.len().div_ceil(2));
    let bytes = value.as_bytes();
    let mut idx = 0;
    if !value.len().is_multiple_of(2) {
        let nibble = decode_hex_nibble(bytes[0])?;
        out.push(nibble);
        idx = 1;
    }
    while idx < bytes.len() {
        let hi = decode_hex_nibble(bytes[idx])?;
        let lo = decode_hex_nibble(bytes[idx + 1])?;
        out.push((hi << 4) | lo);
        idx += 2;
    }
    Ok(out)
}

fn parse_alias_hex(value: &str) -> Result<AliasClaim, String> {
    let mut parts = value.splitn(3, ':');
    let name = parts
        .next()
        .ok_or_else(|| "alias requires name:namespace:proofhex".to_string())?;
    let namespace = parts
        .next()
        .ok_or_else(|| "alias requires name:namespace:proofhex".to_string())?;
    let proof_hex = parts
        .next()
        .ok_or_else(|| "alias requires name:namespace:proofhex".to_string())?;
    Ok(AliasClaim {
        name: name.to_string(),
        namespace: namespace.to_string(),
        proof: parse_hex_vec(proof_hex)?,
    })
}

fn push_council_signature(opts: &mut Options, value: &str) -> Result<(), String> {
    if value.contains(':') {
        opts.council_signatures.push(parse_signature_hex(value)?);
        return Ok(());
    }
    let signer_bytes = opts.council_signature_public.clone().ok_or_else(|| {
        "provide --council-signature-public-key before --council-signature without signer"
            .to_string()
    })?;
    let signature = parse_hex_vec(value)?;
    opts.council_signatures
        .push(build_council_signature(signer_bytes, signature)?);
    Ok(())
}

fn push_council_signature_file(opts: &mut Options, value: &str) -> Result<(), String> {
    if value.contains(':') {
        opts.council_signatures.push(parse_signature_file(value)?);
        return Ok(());
    }
    let signer_bytes = opts.council_signature_public.clone().ok_or_else(|| {
        "provide --council-signature-public-key before --council-signature-file without signer"
            .to_string()
    })?;
    let signature = read_file_bytes(value)?;
    opts.council_signatures
        .push(build_council_signature(signer_bytes, signature)?);
    Ok(())
}

fn parse_alias_file(value: &str) -> Result<AliasClaim, String> {
    let mut parts = value.splitn(3, ':');
    let name = parts
        .next()
        .ok_or_else(|| "alias-file requires name:namespace:path".to_string())?;
    let namespace = parts
        .next()
        .ok_or_else(|| "alias-file requires name:namespace:path".to_string())?;
    let path = parts
        .next()
        .ok_or_else(|| "alias-file requires name:namespace:path".to_string())?;
    let proof = read(path).map_err(|err| format!("failed to read alias proof {path}: {err}"))?;
    Ok(AliasClaim {
        name: name.to_string(),
        namespace: namespace.to_string(),
        proof,
    })
}

fn parse_metadata(value: &str) -> Result<(String, String), String> {
    let (key, val) = value
        .split_once(':')
        .ok_or_else(|| "metadata requires key:value".to_string())?;
    Ok((key.to_string(), val.to_string()))
}

fn parse_signature_hex(value: &str) -> Result<sorafs_manifest::CouncilSignature, String> {
    let (signer_hex, sig_hex) = value
        .split_once(':')
        .ok_or_else(|| "council-signature requires signerhex:signaturehex".to_string())?;
    build_council_signature(parse_hex_vec(signer_hex)?, parse_hex_vec(sig_hex)?)
}

fn parse_signature_file(value: &str) -> Result<sorafs_manifest::CouncilSignature, String> {
    let (signer_hex, path) = value
        .split_once(':')
        .ok_or_else(|| "council-signature-file requires signerhex:path".to_string())?;
    build_council_signature(parse_hex_vec(signer_hex)?, read_file_bytes(path)?)
}

fn build_council_signature(
    signer_bytes: Vec<u8>,
    signature: Vec<u8>,
) -> Result<sorafs_manifest::CouncilSignature, String> {
    if signer_bytes.len() != 32 {
        return Err("council-signature public key must be 32 bytes".into());
    }
    let mut signer = [0u8; 32];
    signer.copy_from_slice(&signer_bytes);
    Ok(sorafs_manifest::CouncilSignature { signer, signature })
}

fn read_file_bytes(path: &str) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|err| format!("failed to read {path}: {err}"))
}

fn set_unique_vec(target: &mut Option<Vec<u8>>, value: Vec<u8>, flag: &str) -> Result<(), String> {
    if let Some(existing) = target.as_ref() {
        if existing != &value {
            return Err(format!(
                "{flag} specified multiple times with different values"
            ));
        }
        return Ok(());
    }
    *target = Some(value);
    Ok(())
}

fn ensure_metadata_entry(metadata: &mut Vec<(String, String)>, key: &str, value: &str) {
    if metadata
        .iter()
        .any(|(existing_key, _)| existing_key.eq_ignore_ascii_case(key))
    {
        return;
    }
    metadata.push((key.to_string(), value.to_string()));
}

fn build_hybrid_manifest_aad(
    manifest_digest: &Hash,
    chunk_digest_sha3: [u8; 32],
    manifest_filename: Option<&str>,
) -> Vec<u8> {
    let mut aad = Vec::with_capacity(
        HYBRID_MANIFEST_AAD_DOMAIN.len()
            + manifest_digest.as_bytes().len()
            + chunk_digest_sha3.len()
            + manifest_filename.map_or(0, |name| 4 + name.len()),
    );
    aad.extend_from_slice(HYBRID_MANIFEST_AAD_DOMAIN);
    aad.extend_from_slice(manifest_digest.as_bytes());
    aad.extend_from_slice(&chunk_digest_sha3);
    if let Some(name) = manifest_filename {
        let name_bytes = name.as_bytes();
        aad.extend_from_slice(&(name_bytes.len() as u32).to_be_bytes());
        aad.extend_from_slice(name_bytes);
    }
    aad
}

fn parse_hex_array(value: &str) -> Result<[u8; 32], String> {
    let vec = parse_hex_vec(value)?;
    if vec.len() != 32 {
        return Err("expected 32-byte hex string".into());
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&vec);
    Ok(arr)
}

fn parse_u64(value: &str) -> Result<u64, String> {
    if let Some(stripped) = value.strip_prefix("0x") {
        u64::from_str_radix(stripped, 16).map_err(|err| err.to_string())
    } else {
        value.parse::<u64>().map_err(|err| err.to_string())
    }
}

fn parse_u32(value: &str) -> Result<u32, String> {
    if let Some(stripped) = value.strip_prefix("0x") {
        u32::from_str_radix(stripped, 16).map_err(|err| err.to_string())
    } else {
        value.parse::<u32>().map_err(|err| err.to_string())
    }
}

fn parse_u16(value: &str) -> Result<u16, String> {
    if let Some(stripped) = value.strip_prefix("0x") {
        u16::from_str_radix(stripped, 16).map_err(|err| err.to_string())
    } else {
        value.parse::<u16>().map_err(|err| err.to_string())
    }
}

fn parse_u128(value: &str) -> Result<u128, String> {
    if let Some(stripped) = value.strip_prefix("0x") {
        u128::from_str_radix(stripped, 16).map_err(|err| err.to_string())
    } else {
        value.parse::<u128>().map_err(|err| err.to_string())
    }
}

fn parse_storage_class(value: &str) -> Result<StorageClass, String> {
    match value.to_ascii_lowercase().as_str() {
        "hot" => Ok(StorageClass::Hot),
        "warm" => Ok(StorageClass::Warm),
        "cold" => Ok(StorageClass::Cold),
        other => Err(format!("unknown storage class: {other}")),
    }
}

fn decode_hex_nibble(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!("invalid hex digit: {}", byte as char)),
    }
}

fn compute_chunk_digest_sha3(chunks: &[CarChunk]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    for chunk in chunks {
        hasher.update(chunk.offset.to_le_bytes());
        hasher.update((chunk.length as u64).to_le_bytes());
        hasher.update(chunk.digest);
    }
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
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

struct DirectoryPlanReader<'a> {
    root: &'a Path,
    files: &'a [FilePlan],
    file_index: usize,
    current: Option<BufReader<File>>, // active file reader
    remaining_in_file: u64,
}

impl<'a> DirectoryPlanReader<'a> {
    fn new(root: &'a Path, files: &'a [FilePlan]) -> io::Result<Self> {
        let mut reader = Self {
            root,
            files,
            file_index: 0,
            current: None,
            remaining_in_file: 0,
        };
        reader.ensure_reader()?;
        Ok(reader)
    }

    fn ensure_reader(&mut self) -> io::Result<()> {
        while self.current.is_none() && self.file_index < self.files.len() {
            self.open_current_file()?;
            if self.remaining_in_file == 0 {
                self.finish_current_file();
            }
        }
        Ok(())
    }

    fn open_current_file(&mut self) -> io::Result<()> {
        let entry = self
            .files
            .get(self.file_index)
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "plan exhausted"))?;
        let abs_path = resolve_plan_path(self.root, &entry.path);
        let file = File::open(&abs_path)?;
        let metadata = file.metadata()?;
        if metadata.len() != entry.size {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "file {} size changed (expected {} bytes, found {} bytes)",
                    abs_path.display(),
                    entry.size,
                    metadata.len()
                ),
            ));
        }
        self.remaining_in_file = entry.size;
        self.current = Some(BufReader::new(file));
        Ok(())
    }

    fn finish_current_file(&mut self) {
        self.current = None;
        self.remaining_in_file = 0;
        self.file_index += 1;
    }
}

impl Read for DirectoryPlanReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            if self.file_index >= self.files.len() {
                return Ok(0);
            }
            if self.current.is_none() {
                self.ensure_reader()?;
                if self.current.is_none() {
                    return Ok(0);
                }
            }
            if self.remaining_in_file == 0 {
                self.finish_current_file();
                continue;
            }

            let to_read = (self.remaining_in_file as usize).min(buf.len());
            let reader = self
                .current
                .as_mut()
                .expect("current reader must be available");
            let read = reader.read(&mut buf[..to_read])?;
            if read == 0 {
                let entry = &self.files[self.file_index];
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!(
                        "file {} ended before reaching expected size {} bytes",
                        display_plan_path(&entry.path),
                        entry.size
                    ),
                ));
            }
            self.remaining_in_file -= read as u64;
            if self.remaining_in_file == 0 {
                self.finish_current_file();
            }
            return Ok(read);
        }
    }
}

fn resolve_plan_path(root: &Path, components: &[String]) -> PathBuf {
    let mut path = root.to_path_buf();
    for component in components {
        path.push(component);
    }
    path
}

fn display_plan_path(components: &[String]) -> String {
    if components.is_empty() {
        ".".to_owned()
    } else {
        components.join("/")
    }
}

fn verify_manifest_signatures_file(
    path: &Path,
    descriptor: &chunker_registry::ChunkerProfileDescriptor,
    manifest_digest: &Hash,
    chunk_digest_sha3: [u8; 32],
    manifest_filename: Option<&str>,
) -> Result<(), String> {
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("failed to read manifest signatures {path:?}: {err}"))?;
    let value: Value = norito::json::from_str(&contents)
        .map_err(|err| format!("failed to parse manifest signatures json {path:?}: {err}"))?;

    let canonical_profile = format!(
        "{}.{}@{}",
        descriptor.namespace, descriptor.name, descriptor.semver
    );
    let profile = value
        .get("profile")
        .and_then(Value::as_str)
        .ok_or_else(|| "manifest signatures file missing `profile` field".to_string())?;
    if profile != canonical_profile {
        let aliases = value
            .get("profile_aliases")
            .and_then(Value::as_array)
            .map(|array| {
                array
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<&str>>()
            })
            .unwrap_or_default();
        if !aliases.iter().any(|alias| *alias == canonical_profile) {
            return Err(format!(
                "manifest signatures profile `{profile}` does not match expected `{canonical_profile}`"
            ));
        }
    }

    if let (Some(expected_name), Some(actual_name)) = (
        manifest_filename,
        value.get("manifest").and_then(Value::as_str),
    ) && actual_name != expected_name
    {
        return Err(format!(
            "manifest signatures references `{actual_name}`, expected `{expected_name}`"
        ));
    }

    let manifest_hex = value
        .get("manifest_blake3")
        .and_then(Value::as_str)
        .ok_or_else(|| "manifest signatures file missing `manifest_blake3` field".to_string())?;
    let expected_manifest_hex = to_hex(manifest_digest.as_bytes());
    if manifest_hex != expected_manifest_hex {
        return Err(format!(
            "manifest signatures digest `{manifest_hex}` does not match computed `{expected_manifest_hex}`"
        ));
    }

    let chunk_hex = value
        .get("chunk_digest_sha3_256")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "manifest signatures file missing `chunk_digest_sha3_256` field".to_string()
        })?;
    let expected_chunk_hex = to_hex(&chunk_digest_sha3);
    if chunk_hex != expected_chunk_hex {
        return Err(format!(
            "manifest signatures chunk digest `{chunk_hex}` does not match computed `{expected_chunk_hex}`"
        ));
    }

    let signatures = value
        .get("signatures")
        .and_then(Value::as_array)
        .ok_or_else(|| "manifest signatures file missing `signatures` array".to_string())?;
    if signatures.is_empty() {
        return Err("manifest signatures file must contain at least one signature entry".into());
    }

    for entry in signatures {
        let obj = entry
            .as_object()
            .ok_or_else(|| "manifest signatures entry is not an object".to_string())?;
        let signer_hex = obj
            .get("signer")
            .and_then(Value::as_str)
            .ok_or_else(|| "signature entry missing `signer` field".to_string())?;
        let signature_hex = obj
            .get("signature")
            .and_then(Value::as_str)
            .ok_or_else(|| "signature entry missing `signature` field".to_string())?;

        let signer_bytes = parse_hex_vec(signer_hex)?;
        let signature_bytes = parse_hex_vec(signature_hex)?;
        if signature_bytes.len() != 64 {
            return Err(format!(
                "signature entry for signer `{signer_hex}` must contain 64-byte signature"
            ));
        }

        match PublicKey::from_bytes(Algorithm::Ed25519, &signer_bytes) {
            Ok(public_key) => {
                let signature = Signature::from_bytes(&signature_bytes);
                signature
                    .verify(&public_key, manifest_digest.as_bytes())
                    .map_err(|err| {
                        format!(
                            "failed to verify council signature for signer `{signer_hex}`: {err}"
                        )
                    })?;
                if let Some(multihash) = obj.get("signer_multihash").and_then(Value::as_str) {
                    let expected_multihash = public_key.to_string();
                    if multihash != expected_multihash {
                        return Err(format!(
                            "signer multihash `{multihash}` does not match expected `{expected_multihash}` for signer `{signer_hex}`"
                        ));
                    }
                }
            }
            Err(_) => {
                if let Some(multihash) = obj.get("signer_multihash").and_then(Value::as_str) {
                    let fallback = format!("hex:{signer_hex}");
                    if multihash != fallback {
                        return Err(format!(
                            "signer `{signer_hex}` uses fallback multihash `{multihash}`, expected `{fallback}`"
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}

fn write_manifest_signatures_file(
    path: &Path,
    descriptor: &chunker_registry::ChunkerProfileDescriptor,
    manifest: &sorafs_manifest::ManifestV1,
    manifest_digest: &Hash,
    chunk_digest_sha3: [u8; 32],
    manifest_filename: &str,
) -> Result<(), String> {
    let mut root = Map::new();
    let profile_handle = format!(
        "{}.{}@{}",
        descriptor.namespace, descriptor.name, descriptor.semver
    );
    let profile_aliases = vec![Value::from(profile_handle.clone())];
    root.insert("profile".to_owned(), Value::from(profile_handle));
    root.insert("profile_aliases".to_owned(), Value::Array(profile_aliases));
    root.insert(
        "manifest".to_owned(),
        Value::from(manifest_filename.to_owned()),
    );
    root.insert(
        "manifest_blake3".to_owned(),
        Value::from(to_hex(manifest_digest.as_bytes())),
    );
    root.insert(
        "chunk_digest_sha3_256".to_owned(),
        Value::from(to_hex(&chunk_digest_sha3)),
    );

    let mut signature_entries = Vec::new();
    for sig in &manifest.governance.council_signatures {
        let signer_hex = to_hex(&sig.signer);
        let signature_hex = to_hex(&sig.signature);
        let signer_multihash = match PublicKey::from_bytes(Algorithm::Ed25519, &sig.signer) {
            Ok(public_key) => public_key.to_string(),
            Err(_) => format!("hex:{signer_hex}"),
        };
        let mut entry = Map::new();
        entry.insert("algorithm".to_owned(), Value::from("ed25519"));
        entry.insert("signer".to_owned(), Value::from(signer_hex));
        entry.insert("signature".to_owned(), Value::from(signature_hex));
        entry.insert("signer_multihash".to_owned(), Value::from(signer_multihash));
        signature_entries.push(Value::Object(entry));
    }
    root.insert("signatures".to_owned(), Value::Array(signature_entries));

    let mut serialized = to_string_pretty(&Value::Object(root))
        .map_err(|err| format!("failed to serialise manifest signatures JSON: {err}"))?;
    serialized.push('\n');
    write_json(path, &serialized)
}

fn write_manifest(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if path == Path::new("-") {
        io::stdout()
            .write_all(bytes)
            .map_err(|err| format!("failed to write manifest to stdout: {err}"))?;
        return Ok(());
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|err| format!("failed to create {parent:?}: {err}"))?;
    }
    let mut file = File::create(path).map_err(|err| format!("failed to create {path:?}: {err}"))?;
    file.write_all(bytes)
        .map_err(|err| format!("failed to write {path:?}: {err}"))
}

fn write_json(path: &Path, report: &str) -> Result<(), String> {
    if path == Path::new("-") {
        io::stdout()
            .write_all(report.as_bytes())
            .map_err(|err| format!("failed to write JSON to stdout: {err}"))?;
        return Ok(());
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|err| format!("failed to create {parent:?}: {err}"))?;
    }
    fs::write(path, report.as_bytes())
        .map_err(|err| format!("failed to write JSON report {path:?}: {err}"))
}

fn write_binary(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if path == Path::new("-") {
        io::stdout()
            .write_all(bytes)
            .map_err(|err| format!("failed to write binary output to stdout: {err}"))?;
        return Ok(());
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        fs::create_dir_all(parent).map_err(|err| format!("failed to create {parent:?}: {err}"))?;
    }
    fs::write(path, bytes).map_err(|err| format!("failed to write {path:?}: {err}"))
}
