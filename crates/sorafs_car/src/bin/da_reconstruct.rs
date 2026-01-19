#![allow(clippy::missing_panics_doc)]

use std::{
    fs::{self, File},
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use clap::Parser;
use eyre::{Result, WrapErr, eyre};
use iroha_data_model::da::manifest::DaManifestV1;
use norito::{
    decode_from_bytes,
    json::{Map, Value},
};

const DEFAULT_CHUNK_TEMPLATE: &str = "chunk_{index:05}.bin";

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Deterministic DA chunk reconstruction harness"
)]
struct Args {
    /// Path to the Norito-encoded DA manifest (binary or hex).
    #[arg(long = "manifest", value_name = "PATH")]
    manifest_path: PathBuf,
    /// Directory containing chunk_{index:05}.bin files produced by the chunk store.
    #[arg(long = "chunks-dir", value_name = "DIR")]
    chunks_dir: PathBuf,
    /// Output path for the reconstructed payload (defaults to <chunks-dir>/payload.bin).
    #[arg(long = "output", value_name = "PATH")]
    output: Option<PathBuf>,
    /// Optional JSON summary output path.
    #[arg(long = "json-out", value_name = "PATH")]
    json_out: Option<PathBuf>,
    /// Filename template for chunk files (must contain {index} or {index:width}).
    #[arg(long = "chunk-template", default_value = DEFAULT_CHUNK_TEMPLATE)]
    chunk_template: String,
}

fn main() {
    if let Err(err) = run(Args::parse()) {
        eprintln!("error: {err:?}");
        std::process::exit(1);
    }
}

fn run(args: Args) -> Result<()> {
    let manifest = load_manifest(&args.manifest_path)
        .wrap_err_with(|| format!("failed to load manifest `{}`", args.manifest_path.display()))?;

    let output_path = args
        .output
        .unwrap_or_else(|| args.chunks_dir.join("payload.bin"));

    let summary = reconstruct_payload(
        &manifest,
        &args.chunks_dir,
        &output_path,
        &args.chunk_template,
    )?;

    if let Some(path) = args.json_out {
        write_summary_json(&summary, &manifest, &path)?;
        println!(
            "reconstructed {} bytes to {} (hash {}, data {}, parity {}) [summary: {}]",
            summary.payload_bytes,
            summary.output_path,
            summary.expected_blob_hash_hex,
            summary.data_chunks,
            summary.parity_chunks,
            path.display()
        );
    } else {
        println!(
            "reconstructed {} bytes to {} (hash {}, data {}, parity {})",
            summary.payload_bytes,
            summary.output_path,
            summary.expected_blob_hash_hex,
            summary.data_chunks,
            summary.parity_chunks
        );
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct ReconstructionSummary {
    chunk_count: usize,
    data_chunks: usize,
    parity_chunks: usize,
    payload_bytes: u64,
    expected_blob_hash_hex: String,
    output_path: String,
}

fn reconstruct_payload(
    manifest: &DaManifestV1,
    chunk_dir: &Path,
    output_path: &Path,
    chunk_template: &str,
) -> Result<ReconstructionSummary> {
    if !chunk_dir.is_dir() {
        return Err(eyre!(
            "chunk directory `{}` does not exist or is not a directory",
            chunk_dir.display()
        ));
    }

    if manifest.chunks.is_empty() {
        return Err(eyre!("manifest does not contain any chunk commitments"));
    }
    let mut ordered_chunks = manifest.chunks.iter().collect::<Vec<_>>();
    ordered_chunks.sort_by_key(|chunk| chunk.index);

    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!("failed to create output directory `{}`", parent.display())
        })?;
    }
    let mut writer = BufWriter::new(
        File::create(output_path)
            .wrap_err_with(|| format!("failed to create `{}`", output_path.display()))?,
    );

    let mut hasher = blake3::Hasher::new();
    let mut buffer = Vec::new();
    let mut total_bytes = 0u64;
    let mut data_chunks = 0usize;
    let mut parity_chunks = 0usize;

    for chunk in ordered_chunks {
        let file_name = render_chunk_template(chunk_template, chunk.index as usize)?;
        let path = chunk_dir.join(file_name);
        let mut file = File::open(&path)
            .wrap_err_with(|| format!("failed to open chunk `{}`", path.display()))?;

        let mut metadata_len = file
            .metadata()
            .map(|meta| meta.len())
            .unwrap_or(chunk.length as u64);
        if metadata_len == 0 {
            metadata_len = chunk.length as u64;
        }
        if metadata_len != chunk.length as u64 {
            return Err(eyre!(
                "chunk {} expected length {} bytes but file `{}` is {} bytes",
                chunk.index,
                chunk.length,
                path.display(),
                metadata_len
            ));
        }

        buffer.resize(chunk.length as usize, 0);
        file.read_exact(&mut buffer).wrap_err_with(|| {
            format!(
                "failed to read {} bytes from `{}`",
                chunk.length,
                path.display()
            )
        })?;

        let digest = blake3::hash(&buffer);
        if digest.as_bytes() != chunk.commitment.as_ref() {
            return Err(eyre!(
                "chunk {} digest mismatch (expected {}, computed {})",
                chunk.index,
                hex::encode(chunk.commitment.as_ref()),
                hex::encode(digest.as_bytes())
            ));
        }

        if chunk.parity {
            parity_chunks = parity_chunks
                .checked_add(1)
                .ok_or_else(|| eyre!("parity chunk counter overflow"))?;
            continue;
        }

        if chunk.offset != total_bytes {
            return Err(eyre!(
                "chunk {} offset {} does not match reconstructed cursor {}",
                chunk.index,
                chunk.offset,
                total_bytes
            ));
        }

        writer
            .write_all(&buffer)
            .wrap_err("failed to write reconstructed payload")?;
        hasher.update(&buffer);
        total_bytes = total_bytes
            .checked_add(buffer.len() as u64)
            .ok_or_else(|| eyre!("payload length overflow"))?;
        data_chunks = data_chunks
            .checked_add(1)
            .ok_or_else(|| eyre!("data chunk counter overflow"))?;
    }
    writer
        .flush()
        .wrap_err("failed to flush reconstructed payload")?;

    if total_bytes != manifest.total_size {
        return Err(eyre!(
            "reconstructed length {} does not match manifest content_length {}",
            total_bytes,
            manifest.total_size
        ));
    }

    let expected = manifest.blob_hash.as_bytes();
    let computed = hasher.finalize();
    if expected != computed.as_bytes() {
        return Err(eyre!(
            "payload digest mismatch (manifest {}, reconstructed {})",
            hex::encode(expected),
            hex::encode(computed.as_bytes())
        ));
    }

    Ok(ReconstructionSummary {
        chunk_count: manifest.chunks.len(),
        data_chunks,
        parity_chunks,
        payload_bytes: total_bytes,
        expected_blob_hash_hex: hex::encode(expected),
        output_path: output_path.display().to_string(),
    })
}

fn load_manifest(path: &Path) -> Result<DaManifestV1> {
    let bytes =
        fs::read(path).wrap_err_with(|| format!("failed to read manifest `{}`", path.display()))?;
    let decoded = decode_manifest_bytes(&bytes)?;
    decode_from_bytes(&decoded).wrap_err("failed to decode Norito manifest")
}

fn decode_manifest_bytes(raw: &[u8]) -> Result<Vec<u8>> {
    let Ok(text) = std::str::from_utf8(raw) else {
        return Ok(raw.to_vec());
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(eyre!("manifest file is empty"));
    }
    let mut body = trimmed.strip_prefix("0x").unwrap_or(trimmed).to_owned();
    body.retain(|c| !c.is_whitespace());
    if body.is_empty() {
        return Err(eyre!("manifest hex payload is empty"));
    }
    if body.chars().all(|c| c.is_ascii_hexdigit()) && body.len() % 2 == 0 {
        let bytes = hex::decode(body).wrap_err("failed to decode manifest hex payload")?;
        return Ok(bytes);
    }
    Ok(raw.to_vec())
}

fn render_chunk_template(template: &str, index: usize) -> Result<String> {
    let mut output = String::new();
    let mut cursor = template;
    let mut replaced = false;

    while let Some(pos) = cursor.find("{index") {
        output.push_str(&cursor[..pos]);
        cursor = &cursor[pos + "{index".len()..];
        let mut width: Option<usize> = None;
        if let Some(rest) = cursor.strip_prefix(':') {
            let end = rest
                .find('}')
                .ok_or_else(|| eyre!("chunk template missing closing `}}`"))?;
            let spec = &rest[..end];
            if !spec.is_empty() {
                width = Some(
                    spec.parse::<usize>()
                        .map_err(|err| eyre!("invalid width `{spec}`: {err}"))?,
                );
            }
            cursor = &rest[end..];
        }
        if !cursor.starts_with('}') {
            return Err(eyre!(
                "chunk template must contain `{{index}}` or `{{index:<width>}}`"
            ));
        }
        cursor = &cursor[1..];
        replaced = true;
        let formatted = if let Some(w) = width {
            format!("{index:0width$}", width = w)
        } else {
            index.to_string()
        };
        output.push_str(&formatted);
    }

    output.push_str(cursor);
    if !replaced {
        return Err(eyre!(
            "chunk template `{template}` must contain `{{index}}` placeholder"
        ));
    }
    Ok(output)
}

fn write_summary_json(
    summary: &ReconstructionSummary,
    manifest: &DaManifestV1,
    path: &Path,
) -> Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).wrap_err_with(|| {
            format!("failed to create summary directory `{}`", parent.display())
        })?;
    }
    let mut obj = Map::new();
    obj.insert(
        "chunk_count".into(),
        Value::from(summary.chunk_count as u64),
    );
    obj.insert(
        "data_chunks".into(),
        Value::from(summary.data_chunks as u64),
    );
    obj.insert(
        "parity_chunks".into(),
        Value::from(summary.parity_chunks as u64),
    );
    obj.insert("payload_bytes".into(), Value::from(summary.payload_bytes));
    obj.insert(
        "blob_hash_hex".into(),
        Value::from(summary.expected_blob_hash_hex.clone()),
    );
    obj.insert(
        "storage_ticket_hex".into(),
        Value::from(hex::encode(manifest.storage_ticket.as_bytes())),
    );
    obj.insert(
        "output_path".into(),
        Value::from(summary.output_path.clone()),
    );
    let rendered = norito::json::to_json_pretty(&Value::Object(obj))
        .wrap_err("failed to render summary JSON")?;
    fs::write(path, rendered.as_bytes())
        .wrap_err_with(|| format!("failed to write summary `{}`", path.display()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use blake3::Hasher as Blake3Hasher;
    use iroha_crypto::{Hash, Signature};
    use iroha_data_model::{
        da::{
            commitment::{DaCommitmentBundle, DaCommitmentRecord, DaProofScheme, KzgCommitment},
            manifest::{ChunkCommitment, ChunkRole, DaManifestV1},
            types::{
                BlobClass, BlobCodec, BlobDigest, ChunkDigest, DaRentQuote, ErasureProfile,
                ExtraMetadata, FecScheme, RetentionPolicy, StorageTicketId,
            },
        },
        nexus::LaneId,
        sorafs::pin_registry::ManifestDigest,
    };
    use iroha_primitives::erasure::rs16;
    use norito::{
        json::{self as norito_json, Map as JsonMap, Value as JsonValue},
        to_bytes,
    };
    use sorafs_car::{CarBuildPlan, CarChunk, ChunkStore, build_plan_from_da_manifest};
    use sorafs_chunker::ChunkProfile;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn render_chunk_template_supports_width() {
        assert_eq!(
            render_chunk_template("chunk_{index:03}.bin", 7).expect("render"),
            "chunk_007.bin"
        );
        assert_eq!(
            render_chunk_template("prefix_{index}_suffix", 42).expect("render"),
            "prefix_42_suffix"
        );
        assert!(render_chunk_template("no-placeholder", 1).is_err());
    }

    #[test]
    fn decode_manifest_allows_hex_payloads() {
        let (manifest, _) = sample_manifest();
        let bytes = to_bytes(&manifest).expect("encode manifest");
        let encoded = hex::encode(&bytes);
        let decoded = decode_manifest_bytes(encoded.as_bytes()).expect("decode hex");
        assert_eq!(decoded, bytes);

        let parsed: DaManifestV1 = decode_from_bytes(&decoded).expect("decode manifest");
        assert_eq!(parsed.chunks.len(), manifest.chunks.len());
    }

    #[test]
    fn reconstructs_payload_from_chunks() {
        let (manifest, payload) = sample_manifest();
        let plan = build_plan_from_da_manifest(&manifest).expect("plan");

        let dir = tempdir().expect("tempdir");
        for (index, chunk) in plan.chunks.iter().enumerate() {
            let path = dir
                .path()
                .join(render_chunk_template(DEFAULT_CHUNK_TEMPLATE, index).expect("render"));
            let start = chunk.offset as usize;
            let end = start + chunk.length as usize;
            fs::write(path, &payload[start..end]).expect("write chunk");
        }

        let out_path = dir.path().join("reconstructed.bin");
        let summary = reconstruct_payload(&manifest, dir.path(), &out_path, DEFAULT_CHUNK_TEMPLATE)
            .expect("reconstruct");

        let reconstructed = fs::read(&out_path).expect("read reconstructed");
        assert_eq!(reconstructed, payload);
        assert_eq!(summary.chunk_count, plan.chunks.len());
        assert_eq!(summary.payload_bytes, payload.len() as u64);
    }

    #[test]
    fn reconstructs_fixture_with_parity_chunks() {
        let fixture_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root")
            .join("fixtures/da/reconstruct/rs_parity_v1");
        let manifest_path = fixture_root.join("manifest.norito.hex");
        let manifest = load_manifest(&manifest_path).expect("fixture manifest");
        let chunks_src = fixture_root.join("chunks");
        let temp_dir = tempdir().expect("temp dir");
        for entry in fs::read_dir(&chunks_src).expect("chunks dir") {
            let entry = entry.expect("chunk entry");
            let dst = temp_dir.path().join(entry.file_name());
            fs::copy(entry.path(), dst).expect("copy chunk file");
        }
        let out_path = temp_dir.path().join("reconstructed.bin");
        let summary = reconstruct_payload(
            &manifest,
            temp_dir.path(),
            &out_path,
            DEFAULT_CHUNK_TEMPLATE,
        )
        .expect("reconstruct fixture");
        assert_eq!(
            summary.parity_chunks,
            manifest.chunks.iter().filter(|chunk| chunk.parity).count()
        );
        assert_eq!(
            summary.data_chunks,
            manifest.chunks.iter().filter(|chunk| !chunk.parity).count()
        );
        let reconstructed = fs::read(&out_path).expect("read reconstructed");
        let expected = fs::read(fixture_root.join("payload.bin")).expect("read fixture payload");
        assert_eq!(reconstructed, expected);
    }

    #[test]
    #[ignore = "regenerates DA reconstruction fixtures on disk"]
    fn regenerate_da_reconstruct_fixture_assets() {
        const STRIPES: usize = 3;
        const DATA_SHARDS: u16 = 2;
        const PARITY_SHARDS: u16 = 2;
        const CHUNK_SIZE: u32 = 32;

        let payload =
            build_fixture_payload(STRIPES * usize::from(DATA_SHARDS) * CHUNK_SIZE as usize);
        let chunk_profile = ChunkProfile {
            min_size: CHUNK_SIZE as usize,
            target_size: CHUNK_SIZE as usize,
            max_size: CHUNK_SIZE as usize,
            break_mask: 1,
        };
        let plan = CarBuildPlan::single_file_with_profile(&payload, chunk_profile)
            .expect("plan derivation succeeds");
        let mut chunk_store = ChunkStore::with_profile(chunk_profile);
        let chunk_dir = tempdir().expect("chunk dir");
        let mut reader: &[u8] = payload.as_slice();
        let chunk_output = chunk_store
            .ingest_plan_stream_to_directory(&plan, &mut reader, chunk_dir.path())
            .expect("persist chunk files");

        let (manifest, parity_payloads) = build_fixture_manifest(
            &payload,
            &chunk_store,
            CHUNK_SIZE,
            DATA_SHARDS,
            PARITY_SHARDS,
        );

        let fixture_root = fixture_root_path();
        if fixture_root.exists() {
            fs::remove_dir_all(&fixture_root).expect("clean existing fixture directory");
        }
        let chunks_dir = fixture_root.join("chunks");
        fs::create_dir_all(&chunks_dir).expect("create chunks directory");

        for record in &chunk_output.records {
            let src = chunk_dir.path().join(&record.file_name);
            let dst = chunks_dir.join(&record.file_name);
            fs::copy(&src, &dst).expect("copy chunk file");
        }
        for (index, payload_bytes) in parity_payloads {
            let dst = chunks_dir.join(format!("chunk_{index:05}.bin"));
            fs::write(&dst, &payload_bytes).expect("write parity chunk");
        }

        let manifest_bytes = to_bytes(&manifest).expect("encode manifest");
        fs::write(
            fixture_root.join("manifest.norito.hex"),
            hex::encode(&manifest_bytes),
        )
        .expect("write manifest hex");
        let manifest_json = norito_json::to_json_pretty(&manifest).expect("manifest json");
        fs::write(fixture_root.join("manifest.json"), manifest_json.as_bytes())
            .expect("write manifest json");
        write_chunk_matrix_json(&manifest, &fixture_root.join("chunk_matrix.json"));
        fs::write(fixture_root.join("payload.bin"), &payload).expect("write payload copy");

        let manifest_digest = ManifestDigest::new(*blake3::hash(&manifest_bytes).as_bytes());
        let chunk_root_hash = Hash::prehashed(*manifest.chunk_root.as_ref());
        let mut kzg_bytes = [0u8; 48];
        let mut kzg_hasher = Blake3Hasher::new();
        kzg_hasher.update(manifest.chunk_root.as_ref());
        kzg_hasher.update(manifest.storage_ticket.as_ref());
        kzg_hasher.finalize_xof().fill(&mut kzg_bytes);
        let kzg_commitment = KzgCommitment::new(kzg_bytes);
        let proof_digest = Hash::prehashed(*blake3::hash(b"da-fixture-proof").as_bytes());
        let acknowledgement_sig = Signature::from_bytes(&[0xA5; 64]);
        let record = DaCommitmentRecord::new(
            manifest.lane_id,
            manifest.epoch,
            42,
            manifest.client_blob_id.clone(),
            manifest_digest,
            DaProofScheme::KzgBls12_381,
            chunk_root_hash,
            Some(kzg_commitment),
            Some(proof_digest),
            manifest.retention_policy.clone(),
            manifest.storage_ticket,
            acknowledgement_sig,
        );
        let bundle = DaCommitmentBundle::new(vec![record]);
        let bundle_bytes = to_bytes(&bundle).expect("encode bundle");
        fs::write(
            fixture_root.join("commitment_bundle.norito.hex"),
            hex::encode(&bundle_bytes),
        )
        .expect("write bundle hex");
        let bundle_json = norito_json::to_json_pretty(&bundle).expect("bundle json");
        fs::write(
            fixture_root.join("commitment_bundle.json"),
            bundle_json.as_bytes(),
        )
        .expect("write bundle json");

        println!(
            "refreshed reconstruction fixture at {}",
            fixture_root.display()
        );
    }

    fn build_fixture_payload(total_bytes: usize) -> Vec<u8> {
        let mut payload = Vec::with_capacity(total_bytes);
        for idx in 0..total_bytes {
            let byte = (idx as u8).wrapping_mul(37).wrapping_add(11);
            payload.push(byte);
        }
        payload
    }

    fn build_fixture_manifest(
        payload: &[u8],
        chunk_store: &ChunkStore,
        chunk_size: u32,
        data_shards: u16,
        parity_shards: u16,
    ) -> (DaManifestV1, Vec<(u32, Vec<u8>)>) {
        let stored = chunk_store.chunks();
        let mut chunk_commitments = Vec::with_capacity(stored.len());
        let mut stripe_symbols = Vec::new();
        let symbol_count = (chunk_size as usize / 2).max(1);
        for (index, chunk) in stored.iter().enumerate() {
            let start = chunk.offset as usize;
            let end = start + chunk.length as usize;
            let slice = &payload[start..end];
            let symbols = rs16::symbols_from_chunk(symbol_count, slice);
            stripe_symbols.push(symbols);
            let stripe_id = u32::try_from(index / usize::from(data_shards)).unwrap_or(u32::MAX);
            chunk_commitments.push(ChunkCommitment::new_with_role(
                index as u32,
                chunk.offset,
                chunk.length,
                ChunkDigest::new(chunk.blake3),
                ChunkRole::Data,
                stripe_id,
            ));
        }

        let mut stripes = Vec::new();
        for chunk_group in stripe_symbols.chunks(data_shards as usize) {
            let mut padded = chunk_group.to_vec();
            while padded.len() < data_shards as usize {
                padded.push(vec![0u16; symbol_count]);
            }
            stripes.push(padded);
        }
        let total_stripes = u32::try_from(stripes.len()).unwrap_or(u32::MAX);
        let shards_per_stripe = u32::from(data_shards.saturating_add(parity_shards));

        let mut parity_payloads = Vec::new();
        let mut next_index = chunk_commitments.len() as u32;
        for (stripe_idx, stripe) in stripes.into_iter().enumerate() {
            let parity_vectors = rs16::encode_parity(&stripe, parity_shards as usize)
                .expect("encode parity vectors");
            for (parity_idx, symbols) in parity_vectors.into_iter().enumerate() {
                let mut bytes = Vec::with_capacity(chunk_size as usize);
                for symbol in symbols {
                    bytes.extend_from_slice(&symbol.to_le_bytes());
                }
                let digest = blake3::hash(&bytes);
                let offset = rs16::parity_offset(
                    payload.len() as u64,
                    stripe_idx,
                    parity_idx,
                    parity_shards as usize,
                    chunk_size,
                )
                .expect("parity offset");
                let stripe_id = u32::try_from(stripe_idx).unwrap_or(u32::MAX);
                chunk_commitments.push(ChunkCommitment::new_with_role(
                    next_index,
                    offset,
                    chunk_size,
                    ChunkDigest::new(*digest.as_bytes()),
                    ChunkRole::GlobalParity,
                    stripe_id,
                ));
                parity_payloads.push((next_index, bytes));
                next_index = next_index.checked_add(1).expect("chunk index overflow");
            }
        }

        let manifest = DaManifestV1 {
            version: DaManifestV1::VERSION,
            client_blob_id: BlobDigest::from_hash(blake3::hash(b"fixture-client")),
            lane_id: LaneId::new(9),
            epoch: 7,
            blob_class: BlobClass::TaikaiSegment,
            codec: BlobCodec::new("fixture.binary"),
            blob_hash: BlobDigest::from_hash(blake3::hash(payload)),
            chunk_root: BlobDigest::new(*chunk_store.por_tree().root()),
            storage_ticket: StorageTicketId::new(*blake3::hash(b"fixture-ticket").as_bytes()),
            total_size: payload.len() as u64,
            chunk_size,
            total_stripes,
            shards_per_stripe,
            erasure_profile: ErasureProfile {
                data_shards,
                parity_shards,
                row_parity_stripes: 0,
                chunk_alignment: data_shards,
                fec_scheme: FecScheme::Rs12_10,
            },
            retention_policy: RetentionPolicy::default(),
            rent_quote: DaRentQuote::default(),
            chunks: chunk_commitments,
            ipa_commitment: BlobDigest::new(*chunk_store.por_tree().root()),
            metadata: ExtraMetadata::default(),
            issued_at_unix: 1_701_111_111,
        };

        (manifest, parity_payloads)
    }

    fn fixture_root_path() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root")
            .join("fixtures/da/reconstruct/rs_parity_v1")
    }

    fn write_chunk_matrix_json(manifest: &DaManifestV1, output: &Path) {
        let mut rows = Vec::with_capacity(manifest.chunks.len());
        for chunk in &manifest.chunks {
            let mut obj = JsonMap::new();
            obj.insert("index".into(), JsonValue::from(u64::from(chunk.index)));
            obj.insert("offset".into(), JsonValue::from(chunk.offset));
            obj.insert("length".into(), JsonValue::from(chunk.length));
            obj.insert("parity".into(), JsonValue::from(chunk.parity));
            obj.insert(
                "digest_hex".into(),
                JsonValue::from(hex::encode(chunk.commitment.as_ref())),
            );
            rows.push(JsonValue::Object(obj));
        }
        let rendered =
            norito_json::to_json_pretty(&JsonValue::Array(rows)).expect("render chunk matrix json");
        fs::write(output, rendered.as_bytes()).expect("write chunk matrix json");
    }

    fn sample_manifest() -> (DaManifestV1, Vec<u8>) {
        let payload = b"chunked payload example bytes for reconstruction harness".to_vec();
        let plan = CarBuildPlan::single_file(&payload).expect("plan");
        (manifest_from_plan(&plan, payload.len() as u64), payload)
    }

    fn manifest_from_plan(plan: &CarBuildPlan, total_size: u64) -> DaManifestV1 {
        let profile = ErasureProfile::default();
        let data_shards = usize::from(profile.data_shards);
        let mut chunks = Vec::with_capacity(plan.chunks.len());
        for (
            index,
            CarChunk {
                offset,
                length,
                digest,
                ..
            },
        ) in plan.chunks.iter().enumerate()
        {
            let stripe_id = u32::try_from(index / data_shards).unwrap_or(u32::MAX);
            chunks.push(ChunkCommitment::new_with_role(
                index as u32,
                *offset,
                *length,
                ChunkDigest::new(*digest),
                ChunkRole::Data,
                stripe_id,
            ));
        }
        let total_stripes = plan.chunks.len().div_ceil(data_shards) as u32;
        let shards_per_stripe =
            u32::from(profile.data_shards.saturating_add(profile.parity_shards));
        DaManifestV1 {
            version: DaManifestV1::VERSION,
            client_blob_id: BlobDigest::new(*plan.payload_digest.as_bytes()),
            lane_id: LaneId::new(0),
            epoch: 0,
            blob_class: BlobClass::TaikaiSegment,
            codec: BlobCodec::new("test.binary"),
            blob_hash: BlobDigest::new(*plan.payload_digest.as_bytes()),
            chunk_root: BlobDigest::new([0u8; 32]),
            storage_ticket: StorageTicketId::new([0u8; 32]),
            total_size,
            chunk_size: plan.chunks.first().map(|chunk| chunk.length).unwrap_or(0),
            total_stripes,
            shards_per_stripe,
            erasure_profile: profile,
            retention_policy: RetentionPolicy::default(),
            rent_quote: DaRentQuote::default(),
            chunks,
            ipa_commitment: BlobDigest::new([0u8; 32]),
            metadata: ExtraMetadata::default(),
            issued_at_unix: 0,
        }
    }
}
