use std::{
    fs,
    path::{Path, PathBuf},
};

use eyre::{Context as _, Result, eyre};
use iroha::data_model::{
    metadata::Metadata,
    offline::{AndroidProvisionedProof, OfflineReceiptChallengePreimage},
};
use iroha_crypto::{Hash, PrivateKey, Signature};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json::{self as serde_json, Value as JsonValue},
};

use crate::offline_tooling::{
    build_metadata, parse_account_id, parse_asset_id, parse_hash_hex, parse_numeric,
    parse_private_key, write_json, write_norito_bytes, write_norito_json,
};

const DEVICE_ID_KEY: &str = "android.provisioned.device_id";

#[derive(Debug)]
pub(crate) struct OfflineProvisionOptions {
    pub spec_path: PathBuf,
    pub output_root: PathBuf,
    pub inspector_key_override: Option<String>,
}

pub(crate) fn run(options: OfflineProvisionOptions) -> Result<()> {
    let spec_bytes = fs::read(&options.spec_path)
        .wrap_err_with(|| format!("failed to read spec file {}", options.spec_path.display()))?;
    let spec: ProvisionSpec = serde_json::from_slice(&spec_bytes)
        .wrap_err_with(|| format!("failed to parse JSON spec {}", options.spec_path.display()))?;
    if spec.proofs.is_empty() {
        return Err(eyre!(
            "spec `{}` contains no provisioning proofs",
            options.spec_path.display()
        ));
    }

    fs::create_dir_all(&options.output_root).wrap_err_with(|| {
        format!(
            "failed to create output directory {}",
            options.output_root.display()
        )
    })?;
    let base_dir = options
        .spec_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));

    let mut summaries = Vec::with_capacity(spec.proofs.len());
    for proof in &spec.proofs {
        let summary = handle_proof(
            proof,
            &spec.inspector,
            options.inspector_key_override.as_deref(),
            &base_dir,
            &options.output_root,
        )?;
        summaries.push(summary);
    }

    let manifest = ProvisionFixturesManifest { proofs: summaries };
    let manifest_path = options.output_root.join("provision_fixtures.manifest.json");
    write_json(&manifest_path, &manifest)
        .wrap_err_with(|| format!("failed to write {}", manifest_path.display()))?;

    Ok(())
}

#[derive(Debug, JsonDeserialize)]
struct ProvisionSpec {
    inspector: InspectorSpec,
    proofs: Vec<ProofSpec>,
}

#[derive(Debug, JsonDeserialize)]
struct InspectorSpec {
    private_key: Option<String>,
    manifest_schema: Option<String>,
    manifest_version: Option<u32>,
}

#[derive(Debug, JsonDeserialize)]
struct ProofSpec {
    label: String,
    manifest_issued_at_ms: u64,
    counter: u64,
    manifest_schema: Option<String>,
    manifest_version: Option<u32>,
    inspector_key: Option<String>,
    device_manifest: Option<JsonValue>,
    device_manifest_file: Option<PathBuf>,
    challenge: Option<ChallengeSpec>,
    challenge_file: Option<PathBuf>,
}

#[derive(Debug, Clone, JsonDeserialize)]
struct ChallengeSpec {
    invoice_id: String,
    receiver: String,
    asset: String,
    amount: String,
    issued_at_ms: Option<u64>,
    nonce_hex: String,
    #[norito(default)]
    sender_certificate_id_hex: Option<String>,
}

#[derive(Debug, JsonSerialize)]
struct ProofFilesSummary {
    proof_json: String,
    proof_norito: String,
}

#[derive(Debug, JsonSerialize)]
struct ProofSummary {
    label: String,
    manifest_schema: String,
    manifest_version: Option<u32>,
    manifest_issued_at_ms: u64,
    counter: u64,
    device_id: String,
    challenge_hash_hex: String,
    challenge_bytes_hex: String,
    files: ProofFilesSummary,
}

#[derive(Debug, JsonSerialize)]
struct ProvisionFixturesManifest {
    proofs: Vec<ProofSummary>,
}

fn handle_proof(
    spec: &ProofSpec,
    inspector: &InspectorSpec,
    cli_key_override: Option<&str>,
    base_dir: &Path,
    output_root: &Path,
) -> Result<ProofSummary> {
    let manifest_schema = spec
        .manifest_schema
        .as_deref()
        .or(inspector.manifest_schema.as_deref())
        .ok_or_else(|| {
            eyre!(
                "proof `{}` is missing manifest schema (set inspector.manifest_schema or proof.manifest_schema)",
                spec.label
            )
        })?
        .to_string();
    let manifest_version = spec.manifest_version.or(inspector.manifest_version);

    let manifest_metadata = build_metadata(
        spec.device_manifest.as_ref(),
        spec.device_manifest_file
            .as_ref()
            .map(|path| base_dir.join(path)),
    )
    .wrap_err_with(|| format!("invalid device manifest for proof `{}`", spec.label))?;
    let device_id = extract_device_id(&manifest_metadata, &spec.label)?;

    let challenge = load_challenge(spec, base_dir)?;
    let challenge_bytes = challenge
        .to_bytes()
        .wrap_err_with(|| format!("failed to encode challenge for `{}`", spec.label))?;
    let challenge_hash = Hash::new(&challenge_bytes);

    let inspector_key = resolve_inspector_key(spec, inspector, cli_key_override)?;
    let mut proof = AndroidProvisionedProof {
        manifest_schema: manifest_schema.clone(),
        manifest_version,
        manifest_issued_at_ms: spec.manifest_issued_at_ms,
        challenge_hash,
        counter: spec.counter,
        device_manifest: manifest_metadata.clone(),
        inspector_signature: empty_signature(),
    };
    let payload = proof
        .signing_bytes()
        .wrap_err_with(|| format!("failed to encode manifest payload for `{}`", spec.label))?;
    proof.inspector_signature = Signature::new(&inspector_key, &payload);

    let proof_dir = output_root.join(&spec.label);
    fs::create_dir_all(&proof_dir).wrap_err_with(|| {
        format!(
            "failed to create output directory for proof `{}`: {}",
            spec.label,
            proof_dir.display()
        )
    })?;

    let proof_json_path = proof_dir.join("proof.json");
    let proof_norito_path = proof_dir.join("proof.norito");
    write_norito_json(&proof_json_path, &proof).wrap_err_with(|| {
        format!(
            "failed to write JSON proof for `{}` ({})",
            spec.label,
            proof_json_path.display()
        )
    })?;
    write_norito_bytes(&proof_norito_path, &proof).wrap_err_with(|| {
        format!(
            "failed to write Norito proof for `{}` ({})",
            spec.label,
            proof_norito_path.display()
        )
    })?;

    Ok(ProofSummary {
        label: spec.label.clone(),
        manifest_schema,
        manifest_version,
        manifest_issued_at_ms: spec.manifest_issued_at_ms,
        counter: spec.counter,
        device_id,
        challenge_hash_hex: hex::encode(challenge_hash.as_ref()),
        challenge_bytes_hex: hex::encode(challenge_bytes),
        files: ProofFilesSummary {
            proof_json: path_name(&proof_json_path, output_root),
            proof_norito: path_name(&proof_norito_path, output_root),
        },
    })
}

fn extract_device_id(metadata: &Metadata, label: &str) -> Result<String> {
    let (_, value) = metadata
        .iter()
        .find(|(name, _)| name.as_ref() == DEVICE_ID_KEY)
        .ok_or_else(|| {
            eyre!(
                "device manifest for `{label}` is missing `{DEVICE_ID_KEY}` (required for counters)"
            )
        })?;
    let parsed: JsonValue = serde_json::from_str(value.get()).map_err(|err| {
        eyre!("failed to parse `{DEVICE_ID_KEY}` for `{label}` as canonical JSON: {err}")
    })?;
    parsed
        .as_str()
        .map(|value| value.to_string())
        .ok_or_else(|| eyre!("`{DEVICE_ID_KEY}` must be a string in manifest for `{label}`"))
}

fn resolve_inspector_key(
    spec: &ProofSpec,
    inspector: &InspectorSpec,
    override_key: Option<&str>,
) -> Result<PrivateKey> {
    if let Some(value) = override_key {
        return parse_private_key(value);
    }
    if let Some(value) = spec.inspector_key.as_deref() {
        return parse_private_key(value);
    }
    if let Some(value) = inspector.private_key.as_deref() {
        return parse_private_key(value);
    }
    Err(eyre!(
        "no inspector private key supplied for proof `{}` (set inspector.private_key, proof.inspector_key, or pass --inspector-key)",
        spec.label
    ))
}

fn load_challenge(spec: &ProofSpec, base_dir: &Path) -> Result<OfflineReceiptChallengePreimage> {
    let source = if let Some(challenge) = spec.challenge.clone() {
        challenge
    } else if let Some(path) = &spec.challenge_file {
        let resolved = base_dir.join(path);
        let bytes = fs::read(&resolved)
            .wrap_err_with(|| format!("failed to read challenge file {}", resolved.display()))?;
        serde_json::from_slice::<ChallengeSpec>(&bytes)
            .wrap_err_with(|| format!("challenge file {} is not valid JSON", resolved.display()))?
    } else {
        return Err(eyre!(
            "proof `{}` requires either `challenge` or `challenge_file`",
            spec.label
        ));
    };

    let receiver = parse_account_id(&source.receiver)
        .wrap_err_with(|| format!("invalid receiver in proof `{}`", spec.label))?;
    let asset = parse_asset_id(&source.asset)
        .wrap_err_with(|| format!("invalid asset in proof `{}`", spec.label))?;
    let amount = parse_numeric(&source.amount)
        .wrap_err_with(|| format!("invalid amount in proof `{}`", spec.label))?;
    let nonce = parse_hash_hex("nonce_hex", &source.nonce_hex)
        .wrap_err_with(|| format!("invalid nonce in proof `{}`", spec.label))?;
    let sender_certificate_id = source
        .sender_certificate_id_hex
        .as_deref()
        .map(|value| parse_hash_hex("sender_certificate_id_hex", value))
        .transpose()
        .wrap_err_with(|| {
            format!(
                "invalid sender_certificate_id_hex in proof `{}`",
                spec.label
            )
        })?
        .unwrap_or(nonce);
    let issued_at_ms = source.issued_at_ms.unwrap_or(spec.manifest_issued_at_ms);

    Ok(OfflineReceiptChallengePreimage {
        invoice_id: source.invoice_id,
        receiver,
        asset,
        amount,
        issued_at_ms,
        sender_certificate_id,
        nonce,
    })
}

fn empty_signature() -> Signature {
    Signature::from_bytes(&[0u8; 64])
}

fn path_name(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECEIVER: &str = "soraゴヂアヌョシペギゥルゼプキュビルェッハガヌイタソタィニュチョヵボヮゾバュチョナボポビワグツニュノノツマヘサ";
    const ASSET: &str = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM#soraゴヂアネウテニュメヴヺテヺヌヺツテニョチュゴヒャシャハゼェタゲヹツザヒドラノヒョンコツニョバエドニュトトウオヒミ";
    const NONCE_HEX: &str = "1111111111111111111111111111111111111111111111111111111111111111";

    fn sample_challenge(issued_at_ms: Option<u64>) -> ChallengeSpec {
        ChallengeSpec {
            invoice_id: "INV-TEST".to_string(),
            receiver: RECEIVER.to_string(),
            asset: ASSET.to_string(),
            amount: "25.00".to_string(),
            issued_at_ms,
            nonce_hex: NONCE_HEX.to_string(),
            sender_certificate_id_hex: None,
        }
    }

    fn sample_spec(manifest_issued_at_ms: u64, challenge: ChallengeSpec) -> ProofSpec {
        ProofSpec {
            label: "sample".to_string(),
            manifest_issued_at_ms,
            counter: 1,
            manifest_schema: None,
            manifest_version: None,
            inspector_key: None,
            device_manifest: None,
            device_manifest_file: None,
            challenge: Some(challenge),
            challenge_file: None,
        }
    }

    #[test]
    fn load_challenge_defaults_to_manifest_timestamp() {
        let spec = sample_spec(1_700_000_000, sample_challenge(None));
        let challenge = load_challenge(&spec, Path::new(".")).expect("load challenge");
        assert_eq!(challenge.issued_at_ms, spec.manifest_issued_at_ms);
    }

    #[test]
    fn load_challenge_prefers_explicit_timestamp() {
        let spec = sample_spec(1_700_000_000, sample_challenge(Some(1_800_000_000)));
        let challenge = load_challenge(&spec, Path::new(".")).expect("load challenge");
        assert_eq!(challenge.issued_at_ms, 1_800_000_000);
    }
}
