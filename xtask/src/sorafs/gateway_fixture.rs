//! Utilities for generating the canonical SoraFS gateway fixture bundle.

use std::{
    fs,
    path::{Path, PathBuf},
};

use blake3::{Hasher, hash as blake3_hash};
use iroha_crypto::{Algorithm, KeyPair, PublicKey, Signature};
use norito::{
    decode_from_bytes,
    derive::{JsonDeserialize, JsonSerialize},
    json::{self, Map, Value},
    to_bytes,
};
use sorafs_car::{CarWriter, compute_chunk_plan_digest_sha3, ingest_single_file};
use sorafs_manifest::{
    DagCodecId, GovernanceProofs, ManifestBuilder, ManifestV1, PinPolicy, StorageClass,
    chunker_registry,
    gateway_fixture::{
        SORAFS_GATEWAY_FIXTURE_RELEASE_UNIX, SORAFS_GATEWAY_FIXTURE_VERSION,
        SORAFS_GATEWAY_PROFILE_VERSION,
    },
    por::{PorChallengeV1, PorProofV1},
};
use thiserror::Error;

const CHALLENGE_FIXTURE_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../fixtures/sorafs_manifest/por/challenge_v1.to"
));
const PROOF_FIXTURE_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../fixtures/sorafs_manifest/por/proof_v1.to"
));

/// Canonical fixture bundle built by the harness.
#[derive(Debug, Clone)]
pub struct FixtureBundle {
    pub manifest: ManifestV1,
    pub challenge: PorChallengeV1,
    pub proof: PorProofV1,
    pub car_bytes: Vec<u8>,
    pub payload: Vec<u8>,
    pub chunk_digest_sha3_256: [u8; 32],
    pub council_envelope: Vec<u8>,
}

/// Errors reported during fixture generation.
#[derive(Debug, Error)]
pub enum FixtureError {
    #[error("failed to serialize Norito payload: {0}")]
    Norito(#[from] norito::Error),
    #[error("failed to write fixture artifact: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to encode Norito JSON payload: {0}")]
    Json(#[from] norito::json::Error),
    #[error("{0}")]
    Invalid(String),
}

const COUNCIL_ENVELOPE_JSON_FILE: &str = "manifest_council_envelope.json";

/// Generate the canonical fixture bundle deterministically.
pub fn generate_bundle() -> FixtureBundle {
    let payload = sample_payload_bytes();
    let summary = ingest_single_file(&payload).expect("fixture ingestion");
    let plan = summary.plan.clone();
    let chunk_digest_sha3_256 = compute_chunk_plan_digest_sha3(&plan.chunks);
    let mut car_bytes = Vec::new();
    let stats = CarWriter::new(&plan, &payload)
        .expect("fixture car writer")
        .write_to(&mut car_bytes)
        .expect("fixture car write");
    let mut car_digest = [0u8; 32];
    car_digest.copy_from_slice(blake3_hash(&car_bytes).as_bytes());
    let root_cid = stats
        .root_cids
        .first()
        .cloned()
        .unwrap_or_else(|| b"bafysorafsmanifest".to_vec());

    let mut manifest = ManifestBuilder::new()
        .root_cid(root_cid)
        .dag_codec(DagCodecId(stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, chunker_registry::DEFAULT_MULTIHASH_CODE)
        .content_length(plan.content_length)
        .car_digest(car_digest)
        .car_size(stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 3,
            storage_class: StorageClass::Hot,
            retention_epoch: 86_400,
        })
        .governance(GovernanceProofs::default())
        .build()
        .expect("manifest fixture construction");

    let descriptor = chunker_registry::default_descriptor();
    let alias_list: Vec<String> = descriptor
        .aliases
        .iter()
        .copied()
        .map(str::to_string)
        .collect();
    let canonical_alias = alias_list.first().cloned().unwrap_or_else(|| {
        format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        )
    });
    manifest.chunking.aliases = alias_list;

    let mut challenge: PorChallengeV1 =
        decode_from_bytes(CHALLENGE_FIXTURE_BYTES).expect("decode PoR challenge fixture");
    let mut proof: PorProofV1 =
        decode_from_bytes(PROOF_FIXTURE_BYTES).expect("decode PoR proof fixture");
    challenge.chunking_profile.clone_from(&canonical_alias);

    let manifest_digest = manifest
        .digest()
        .expect("manifest digest computation from fixture");
    let digest_bytes = manifest_digest.as_bytes();
    challenge.manifest_digest.copy_from_slice(digest_bytes);
    proof.manifest_digest.copy_from_slice(digest_bytes);
    proof.challenge_id = challenge.challenge_id;
    proof.provider_id = challenge.provider_id;
    let council_envelope = build_council_envelope(
        &manifest,
        chunk_digest_sha3_256,
        canonical_alias.as_str(),
        &manifest.chunking.aliases,
    )
    .expect("fixture council envelope construction");

    FixtureBundle {
        manifest,
        challenge,
        proof,
        car_bytes,
        payload,
        chunk_digest_sha3_256,
        council_envelope,
    }
}

/// Write the canonical fixture bundle to disk and return the computed metadata.
pub fn write_bundle(output_dir: &Path) -> Result<GeneratedMetadata, FixtureError> {
    fs::create_dir_all(output_dir)?;
    let bundle = generate_bundle();
    let metadata = metadata_from_bundle(&bundle);

    fs::write(
        output_dir.join("manifest_v1.to"),
        to_bytes(&bundle.manifest)?,
    )?;
    fs::write(
        output_dir.join("manifest_v1.json"),
        json::to_vec(&bundle.manifest)?,
    )?;
    fs::write(
        output_dir.join("challenge_v1.to"),
        to_bytes(&bundle.challenge)?,
    )?;
    fs::write(
        output_dir.join("challenge_v1.json"),
        json::to_vec(&bundle.challenge)?,
    )?;
    fs::write(output_dir.join("proof_v1.to"), to_bytes(&bundle.proof)?)?;
    fs::write(
        output_dir.join("proof_v1.json"),
        json::to_vec(&bundle.proof)?,
    )?;

    fs::write(output_dir.join("payload.bin"), &bundle.payload)?;
    fs::write(
        output_dir.join("payload.blake3"),
        format!("{}\n", metadata.payload_blake3_hex),
    )?;

    fs::write(output_dir.join("gateway.car"), &bundle.car_bytes)?;
    fs::write(
        output_dir.join("gateway_car.blake3"),
        format!("{}\n", metadata.car_blake3_hex),
    )?;
    fs::write(
        output_dir.join(COUNCIL_ENVELOPE_JSON_FILE),
        &bundle.council_envelope,
    )?;

    fs::write(
        output_dir.join("scenarios.json"),
        json::to_vec(&scenarios_json_value())?,
    )?;
    fs::write(
        output_dir.join("metadata.json"),
        json::to_vec(&metadata_json_value(&metadata))?,
    )?;

    Ok(metadata)
}

/// Metadata produced after generating the fixtures.
#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct GeneratedMetadata {
    pub version: String,
    pub profile_version: String,
    pub released_at_unix: u64,
    pub fixtures_digest_blake3_hex: String,
    pub manifest_blake3_hex: String,
    pub payload_blake3_hex: String,
    pub car_blake3_hex: String,
    pub chunk_digest_sha3_256_hex: String,
    pub council_envelope_blake3_hex: String,
}

/// Compute fixture metadata from an in-memory bundle.
pub fn metadata_from_bundle(bundle: &FixtureBundle) -> GeneratedMetadata {
    let manifest_bytes =
        to_bytes(&bundle.manifest).expect("serialize manifest fixture deterministically");
    let challenge_bytes =
        to_bytes(&bundle.challenge).expect("serialize challenge fixture deterministically");
    let proof_bytes = to_bytes(&bundle.proof).expect("serialize proof fixture deterministically");
    let council_envelope_digest = blake3_hash(&bundle.council_envelope).to_hex().to_string();
    let fixtures_digest = fixtures_digest_bytes(
        &manifest_bytes,
        &challenge_bytes,
        &proof_bytes,
        &bundle.car_bytes,
        &bundle.payload,
        &bundle.council_envelope,
    )
    .to_hex()
    .to_string();
    let manifest_digest = blake3_hash(&manifest_bytes).to_hex().to_string();
    let payload_digest = blake3_hash(&bundle.payload).to_hex().to_string();
    let car_digest = blake3_hash(&bundle.car_bytes).to_hex().to_string();

    GeneratedMetadata {
        version: SORAFS_GATEWAY_FIXTURE_VERSION.to_string(),
        profile_version: SORAFS_GATEWAY_PROFILE_VERSION.to_string(),
        released_at_unix: SORAFS_GATEWAY_FIXTURE_RELEASE_UNIX,
        fixtures_digest_blake3_hex: fixtures_digest,
        manifest_blake3_hex: manifest_digest,
        payload_blake3_hex: payload_digest,
        car_blake3_hex: car_digest,
        chunk_digest_sha3_256_hex: hex::encode(bundle.chunk_digest_sha3_256),
        council_envelope_blake3_hex: council_envelope_digest,
    }
}

fn fixtures_digest_bytes(
    manifest_bytes: &[u8],
    challenge_bytes: &[u8],
    proof_bytes: &[u8],
    car_bytes: &[u8],
    payload: &[u8],
    council_envelope: &[u8],
) -> blake3::Hash {
    let mut hasher = Hasher::new();
    hasher.update(manifest_bytes);
    hasher.update(challenge_bytes);
    hasher.update(proof_bytes);
    hasher.update(car_bytes);
    hasher.update(payload);
    hasher.update(council_envelope);
    hasher.finalize()
}

fn metadata_json_value(metadata: &GeneratedMetadata) -> Value {
    let mut map = Map::new();
    map.insert("version".into(), Value::from(metadata.version.clone()));
    map.insert(
        "profile_version".into(),
        Value::from(metadata.profile_version.clone()),
    );
    map.insert(
        "released_at_unix".into(),
        Value::from(metadata.released_at_unix),
    );
    map.insert(
        "fixtures_digest_blake3_hex".into(),
        Value::from(metadata.fixtures_digest_blake3_hex.clone()),
    );
    map.insert(
        "manifest_blake3_hex".into(),
        Value::from(metadata.manifest_blake3_hex.clone()),
    );
    map.insert(
        "payload_blake3_hex".into(),
        Value::from(metadata.payload_blake3_hex.clone()),
    );
    map.insert(
        "car_blake3_hex".into(),
        Value::from(metadata.car_blake3_hex.clone()),
    );
    map.insert(
        "chunk_digest_sha3_256_hex".into(),
        Value::from(metadata.chunk_digest_sha3_256_hex.clone()),
    );
    map.insert(
        "council_envelope_blake3_hex".into(),
        Value::from(metadata.council_envelope_blake3_hex.clone()),
    );
    Value::Object(map)
}

fn scenarios_json_value() -> Value {
    let scenarios = [
        ("A1", "Full CAR replay (sf1 profile)", 200, "success"),
        ("A2", "Aligned byte-range replay", 206, "success"),
        ("A3", "Misaligned byte-range refusal", 416, "refusal"),
        ("A4", "Multi-range byte replay", 206, "success"),
        ("B1", "Unsupported chunker handle refusal", 406, "refusal"),
        (
            "B2",
            "Missing required SoraFS headers refusal",
            428,
            "refusal",
        ),
        ("B3", "Corrupted PoR proof refusal", 422, "refusal"),
        (
            "B4",
            "Corrupted CAR payload (digest mismatch)",
            422,
            "refusal",
        ),
        ("B5", "Provider not admitted", 412, "refusal"),
        ("B6", "Client exceeds rate limit window", 429, "refusal"),
        (
            "C1",
            "1k concurrent range streaming (warm cache)",
            200,
            "success",
        ),
        (
            "C2",
            "1k concurrent streaming with injected 1% corruption",
            422,
            "refusal",
        ),
        ("D1", "Load with GAR denylist trigger", 451, "refusal"),
    ];
    let entries = scenarios
        .iter()
        .map(|(id, description, status, outcome)| {
            let mut map = Map::new();
            map.insert("id".into(), Value::from(*id));
            map.insert("description".into(), Value::from(*description));
            map.insert("expected_status".into(), Value::from(*status as u64));
            map.insert("expected_outcome".into(), Value::from(*outcome));
            Value::Object(map)
        })
        .collect();
    Value::Array(entries)
}

fn sample_payload_bytes() -> Vec<u8> {
    const LEN: usize = 1_048_576;
    let mut payload = Vec::with_capacity(LEN);
    let mut state: u64 = 0x5A_EC_D4_BA;
    for _ in 0..LEN {
        state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let byte = u8::try_from((state >> 24) & 0xFF).expect("masked to byte");
        payload.push(byte);
    }
    payload
}

/// Helper returning the default fixture directory.
pub fn default_output_dir() -> PathBuf {
    PathBuf::from("fixtures")
        .join("sorafs_gateway")
        .join("1.0.0")
}

pub fn verify_bundle(dir: &Path) -> Result<GeneratedMetadata, FixtureError> {
    let metadata_path = dir.join("metadata.json");
    let metadata_raw = fs::read(&metadata_path)?;
    let mut metadata: GeneratedMetadata = norito::json::from_slice(&metadata_raw)?;
    ensure_metadata_constants(&metadata)?;

    let manifest_path = dir.join("manifest_v1.to");
    let challenge_path = dir.join("challenge_v1.to");
    let proof_path = dir.join("proof_v1.to");
    let payload_path = dir.join("payload.bin");
    let car_path = dir.join("gateway.car");
    let council_envelope_path = dir.join(COUNCIL_ENVELOPE_JSON_FILE);

    let manifest_bytes = fs::read(&manifest_path)?;
    let challenge_bytes = fs::read(&challenge_path)?;
    let proof_bytes = fs::read(&proof_path)?;
    let payload_bytes = fs::read(&payload_path)?;
    let car_bytes = fs::read(&car_path)?;
    let council_envelope_bytes = fs::read(&council_envelope_path)?;

    // Ensure artifacts decode cleanly.
    let manifest: ManifestV1 = decode_from_bytes(&manifest_bytes)?;
    let _challenge: PorChallengeV1 = decode_from_bytes(&challenge_bytes)?;
    let _proof: PorProofV1 = decode_from_bytes(&proof_bytes)?;
    let chunk_digest_sha3_256 = compute_chunk_plan_digest_sha3(
        &ingest_single_file(&payload_bytes)
            .map_err(|err| {
                FixtureError::Invalid(format!(
                    "failed to recompute fixture chunk plan digest: {err}"
                ))
            })?
            .plan
            .chunks,
    );
    verify_council_envelope(&manifest, chunk_digest_sha3_256, &council_envelope_bytes)?;

    let fixtures_digest = fixtures_digest_bytes(
        &manifest_bytes,
        &challenge_bytes,
        &proof_bytes,
        &car_bytes,
        &payload_bytes,
        &council_envelope_bytes,
    )
    .to_hex()
    .to_string();
    let manifest_digest = blake3_hash(&manifest_bytes).to_hex().to_string();
    let payload_digest = blake3_hash(&payload_bytes).to_hex().to_string();
    let car_digest = blake3_hash(&car_bytes).to_hex().to_string();
    let council_envelope_digest = blake3_hash(&council_envelope_bytes).to_hex().to_string();

    ensure_digest_match(
        "fixtures_digest_blake3_hex",
        &metadata.fixtures_digest_blake3_hex,
        &fixtures_digest,
    )?;
    ensure_digest_match(
        "manifest_blake3_hex",
        &metadata.manifest_blake3_hex,
        &manifest_digest,
    )?;
    ensure_digest_match(
        "payload_blake3_hex",
        &metadata.payload_blake3_hex,
        &payload_digest,
    )?;
    ensure_digest_match("car_blake3_hex", &metadata.car_blake3_hex, &car_digest)?;
    ensure_digest_match(
        "chunk_digest_sha3_256_hex",
        &metadata.chunk_digest_sha3_256_hex,
        &hex::encode(chunk_digest_sha3_256),
    )?;
    ensure_digest_match(
        "council_envelope_blake3_hex",
        &metadata.council_envelope_blake3_hex,
        &council_envelope_digest,
    )?;

    let payload_hex_file = read_hex_file(&dir.join("payload.blake3"))?;
    ensure_digest_match("payload.blake3", &payload_hex_file, &payload_digest)?;
    let car_hex_file = read_hex_file(&dir.join("gateway_car.blake3"))?;
    ensure_digest_match("gateway_car.blake3", &car_hex_file, &car_digest)?;

    let scenarios_raw = fs::read(dir.join("scenarios.json"))?;
    let actual_scenarios: Value = norito::json::from_slice(&scenarios_raw)?;
    if actual_scenarios != scenarios_json_value() {
        return Err(FixtureError::Invalid(
            "scenarios.json drift detected".to_string(),
        ));
    }

    metadata.fixtures_digest_blake3_hex = fixtures_digest;
    metadata.manifest_blake3_hex = manifest_digest;
    metadata.payload_blake3_hex = payload_digest;
    metadata.car_blake3_hex = car_digest;
    metadata.chunk_digest_sha3_256_hex = hex::encode(chunk_digest_sha3_256);
    metadata.council_envelope_blake3_hex = council_envelope_digest;
    Ok(metadata)
}

fn build_council_envelope(
    manifest: &ManifestV1,
    chunk_digest_sha3_256: [u8; 32],
    profile: &str,
    profile_aliases: &[String],
) -> Result<Vec<u8>, FixtureError> {
    let manifest_digest = manifest.digest()?;
    let keypair = fixture_council_keypair()?;
    let signature = Signature::try_new(keypair.private_key(), manifest_digest.as_bytes())
        .map_err(|err| FixtureError::Invalid(format!("failed to sign council envelope: {err}")))?;
    let public_key = keypair.public_key();
    let (_, signer_bytes) = public_key.try_to_bytes().map_err(|err| {
        FixtureError::Invalid(format!("failed to export council public key: {err}"))
    })?;

    let mut signature_entry = Map::new();
    signature_entry.insert("algorithm".into(), Value::from("ed25519"));
    signature_entry.insert("signer".into(), Value::from(hex::encode(signer_bytes)));
    signature_entry.insert(
        "signature".into(),
        Value::from(hex::encode(signature.payload())),
    );
    signature_entry.insert(
        "signer_multihash".into(),
        Value::from(public_key.to_string()),
    );

    let mut envelope = Map::new();
    envelope.insert(
        "chunk_digest_sha3_256".into(),
        Value::from(hex::encode(chunk_digest_sha3_256)),
    );
    envelope.insert(
        "manifest_blake3".into(),
        Value::from(hex::encode(manifest_digest.as_bytes())),
    );
    envelope.insert("profile".into(), Value::from(profile.to_string()));
    envelope.insert(
        "profile_aliases".into(),
        Value::Array(
            profile_aliases
                .iter()
                .cloned()
                .map(Value::from)
                .collect::<Vec<_>>(),
        ),
    );
    envelope.insert(
        "signatures".into(),
        Value::Array(vec![Value::Object(signature_entry)]),
    );

    let mut encoded = json::to_vec_pretty(&Value::Object(envelope))?;
    encoded.push(b'\n');
    Ok(encoded)
}

fn verify_council_envelope(
    manifest: &ManifestV1,
    chunk_digest_sha3_256: [u8; 32],
    envelope: &[u8],
) -> Result<(), FixtureError> {
    let manifest_digest = manifest.digest()?;
    let expected_manifest_hex = hex::encode(manifest_digest.as_bytes());
    let expected_chunk_hex = hex::encode(chunk_digest_sha3_256);
    let value: Value = json::from_slice(envelope)?;
    let obj = value
        .as_object()
        .ok_or_else(|| FixtureError::Invalid("council envelope must be a JSON object".into()))?;
    let manifest_hex = required_str(obj, "manifest_blake3")?;
    if manifest_hex != expected_manifest_hex {
        return Err(FixtureError::Invalid(format!(
            "council envelope manifest_blake3 mismatch: envelope={manifest_hex} computed={expected_manifest_hex}"
        )));
    }
    let chunk_hex = required_str(obj, "chunk_digest_sha3_256")?;
    if chunk_hex != expected_chunk_hex {
        return Err(FixtureError::Invalid(format!(
            "council envelope chunk_digest_sha3_256 mismatch: envelope={chunk_hex} computed={expected_chunk_hex}"
        )));
    }

    let profile = required_str(obj, "profile")?;
    if !manifest
        .chunking
        .aliases
        .iter()
        .any(|alias| alias == profile)
    {
        return Err(FixtureError::Invalid(format!(
            "council envelope profile `{profile}` is not a manifest alias"
        )));
    }
    let profile_aliases = obj
        .get("profile_aliases")
        .and_then(Value::as_array)
        .ok_or_else(|| FixtureError::Invalid("council envelope missing profile_aliases".into()))?;
    let envelope_aliases = profile_aliases
        .iter()
        .map(|alias| {
            alias.as_str().ok_or_else(|| {
                FixtureError::Invalid("council envelope profile_aliases must be strings".into())
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let manifest_aliases = manifest
        .chunking
        .aliases
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    if envelope_aliases != manifest_aliases {
        return Err(FixtureError::Invalid(format!(
            "council envelope profile_aliases mismatch: envelope={envelope_aliases:?} manifest={manifest_aliases:?}"
        )));
    }

    let signatures = obj
        .get("signatures")
        .and_then(Value::as_array)
        .ok_or_else(|| FixtureError::Invalid("council envelope missing signatures".into()))?;
    if signatures.is_empty() {
        return Err(FixtureError::Invalid(
            "council envelope must include at least one signature".into(),
        ));
    }
    for signature in signatures {
        verify_council_signature(signature, manifest_digest.as_bytes())?;
    }
    Ok(())
}

fn verify_council_signature(signature: &Value, payload: &[u8]) -> Result<(), FixtureError> {
    let obj = signature
        .as_object()
        .ok_or_else(|| FixtureError::Invalid("council signature must be an object".into()))?;
    let algorithm = required_str(obj, "algorithm")?;
    if !algorithm.eq_ignore_ascii_case("ed25519") {
        return Err(FixtureError::Invalid(format!(
            "unsupported council signature algorithm `{algorithm}`"
        )));
    }
    let signer_hex = required_str(obj, "signer")?;
    let signer_bytes = hex::decode(signer_hex)
        .map_err(|err| FixtureError::Invalid(format!("invalid signer hex: {err}")))?;
    let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &signer_bytes)
        .map_err(|err| FixtureError::Invalid(format!("invalid council public key: {err}")))?;
    if let Some(multihash) = obj.get("signer_multihash").and_then(Value::as_str) {
        let expected = public_key.to_string();
        if multihash != expected {
            return Err(FixtureError::Invalid(format!(
                "council signer_multihash mismatch: envelope={multihash} computed={expected}"
            )));
        }
    }
    let signature_hex = required_str(obj, "signature")?;
    let signature_bytes = hex::decode(signature_hex)
        .map_err(|err| FixtureError::Invalid(format!("invalid signature hex: {err}")))?;
    if signature_bytes.len() != 64 {
        return Err(FixtureError::Invalid(format!(
            "council signature must be 64 bytes, found {}",
            signature_bytes.len()
        )));
    }
    let signature = Signature::from_bytes(&signature_bytes);
    signature
        .verify(&public_key, payload)
        .map_err(|err| FixtureError::Invalid(format!("council signature did not verify: {err}")))
}

fn required_str<'a>(obj: &'a Map, field: &str) -> Result<&'a str, FixtureError> {
    obj.get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| FixtureError::Invalid(format!("council envelope missing `{field}`")))
}

fn fixture_council_keypair() -> Result<KeyPair, FixtureError> {
    let seed = blake3_hash(b"sorafs-gateway-fixture-council-v1");
    KeyPair::try_from_seed(seed.as_bytes().to_vec(), Algorithm::Ed25519).map_err(|err| {
        FixtureError::Invalid(format!("fixture council key derivation failed: {err}"))
    })
}

fn read_hex_file(path: &Path) -> Result<String, FixtureError> {
    let raw = fs::read_to_string(path)?;
    Ok(raw.trim().to_ascii_lowercase())
}

fn ensure_metadata_constants(metadata: &GeneratedMetadata) -> Result<(), FixtureError> {
    if metadata.version != SORAFS_GATEWAY_FIXTURE_VERSION {
        return Err(FixtureError::Invalid(format!(
            "metadata version {} does not match expected {}",
            metadata.version, SORAFS_GATEWAY_FIXTURE_VERSION
        )));
    }
    if metadata.profile_version != SORAFS_GATEWAY_PROFILE_VERSION {
        return Err(FixtureError::Invalid(format!(
            "metadata profile_version {} does not match expected {}",
            metadata.profile_version, SORAFS_GATEWAY_PROFILE_VERSION
        )));
    }
    if metadata.released_at_unix != SORAFS_GATEWAY_FIXTURE_RELEASE_UNIX {
        return Err(FixtureError::Invalid(format!(
            "metadata released_at_unix {} does not match expected {}",
            metadata.released_at_unix, SORAFS_GATEWAY_FIXTURE_RELEASE_UNIX
        )));
    }
    Ok(())
}

fn ensure_digest_match(label: &str, expected: &str, actual: &str) -> Result<(), FixtureError> {
    let expected_norm = expected.trim().to_ascii_lowercase();
    let actual_norm = actual.trim().to_ascii_lowercase();
    if expected_norm != actual_norm {
        return Err(FixtureError::Invalid(format!(
            "{label} mismatch: metadata={expected_norm} computed={actual_norm}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn verify_bundle_accepts_pristine_output() {
        let dir = tempdir().expect("temp dir");
        write_bundle(dir.path()).expect("write bundle");
        let metadata = verify_bundle(dir.path()).expect("verify bundle");
        assert_eq!(metadata.version, SORAFS_GATEWAY_FIXTURE_VERSION.to_string());
        assert_eq!(
            metadata.profile_version,
            SORAFS_GATEWAY_PROFILE_VERSION.to_string()
        );
    }

    #[test]
    fn verify_bundle_detects_payload_corruption() {
        let dir = tempdir().expect("temp dir");
        write_bundle(dir.path()).expect("write bundle");
        fs::write(dir.path().join("payload.bin"), b"corrupted").expect("overwrite payload");
        let err = verify_bundle(dir.path()).expect_err("verification must fail");
        let matches_expected = matches!(
            &err,
            FixtureError::Invalid(message)
                if message.contains("payload") || message.contains("fixtures_digest")
        );
        assert!(matches_expected, "unexpected error: {err:?}");
    }
}
