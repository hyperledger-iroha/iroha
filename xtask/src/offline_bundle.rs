use std::{
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use eyre::{Context as _, Result, eyre};
use iroha_crypto::{Hash, PublicKey, Signature};
use iroha_data_model::{
    metadata::Metadata,
    offline::{
        AggregateProofEnvelope, AndroidMarkerKeyProof, AndroidProvisionedProof,
        AppleAppAttestProof, OfflineAllowanceCommitment, OfflineBalanceProof, OfflinePlatformProof,
        OfflinePlatformTokenSnapshot, OfflineProofRequestReplay, OfflineSpendReceipt,
        OfflineToOnlineTransfer, OfflineTransferRecord, OfflineTransferStatus,
        OfflineWalletCertificate, OfflineWalletPolicy, PoseidonDigest, canonical_app_attest_key_id,
    },
};
use norito::{
    derive::{JsonDeserialize, JsonSerialize},
    json::{self as serde_json, Value as JsonValue},
};

use crate::offline_tooling::{
    build_metadata, parse_account_id, parse_asset_id, parse_hash_hex, parse_numeric, write_json,
    write_norito_bytes, write_norito_json,
};

#[derive(Debug)]
pub(crate) struct OfflineBundleOptions {
    pub spec_path: PathBuf,
    pub output_root: PathBuf,
}

pub(crate) fn run(options: OfflineBundleOptions) -> Result<()> {
    let spec_bytes = fs::read(&options.spec_path)
        .wrap_err_with(|| format!("failed to read spec {}", options.spec_path.display()))?;
    let spec: BundleSpecFile = serde_json::from_slice(&spec_bytes)
        .wrap_err_with(|| format!("failed to parse spec JSON {}", options.spec_path.display()))?;
    if spec.bundles.is_empty() {
        return Err(eyre!(
            "spec `{}` does not contain any bundles",
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
    for bundle in &spec.bundles {
        handle_bundle(bundle, &base_dir, &options.output_root)?;
    }
    Ok(())
}

fn handle_bundle(spec: &BundleSpec, base_dir: &Path, output_root: &Path) -> Result<()> {
    if spec.label.trim().is_empty() {
        return Err(eyre!("bundle label cannot be empty"));
    }
    let transfer = build_transfer(spec, base_dir)?;
    let dir = output_root.join(&spec.label);
    fs::create_dir_all(&dir)
        .wrap_err_with(|| format!("failed to create bundle directory {}", dir.display()))?;

    write_norito_json(&dir.join("bundle.json"), &transfer)?;
    write_norito_bytes(&dir.join("bundle.norito"), &transfer)?;

    let receipts_root = compute_receipts_root(&transfer)?;
    let record = build_record(spec, transfer, base_dir)?;
    let sum_request = record.to_proof_request_sum().map_err(|err| eyre!(err))?;
    let counter_checkpoint = match spec.counter_checkpoint {
        Some(value) => value,
        None => record.counter_checkpoint_hint().map_err(|err| eyre!(err))?,
    };
    let counter_request = record
        .to_proof_request_counter(counter_checkpoint)
        .map_err(|err| eyre!(err))?;
    let replay_request = build_replay_request(spec, &record)?;

    write_norito_json(&dir.join("proof_sum_request.json"), &sum_request)?;
    write_norito_json(&dir.join("proof_counter_request.json"), &counter_request)?;
    if let Some(replay) = &replay_request {
        write_norito_json(&dir.join("proof_replay_request.json"), replay)?;
    }

    let manifest = BundleManifest {
        label: spec.label.clone(),
        receipt_count: record.transfer.receipts.len(),
        receipts_root_hex: receipts_root.to_hex_upper(),
        bundle_json: "bundle.json".into(),
        bundle_norito: "bundle.norito".into(),
        proof_sum_request: "proof_sum_request.json".into(),
        proof_counter_request: "proof_counter_request.json".into(),
        proof_replay_request: replay_request.map(|_| "proof_replay_request.json".into()),
    };
    write_json(&dir.join("bundle_summary.json"), &manifest)?;
    println!(
        "generated bundle `{}` ({} receipts, root {})",
        spec.label, manifest.receipt_count, manifest.receipts_root_hex
    );
    Ok(())
}

fn build_transfer(spec: &BundleSpec, base_dir: &Path) -> Result<OfflineToOnlineTransfer> {
    let bundle_id = parse_hash_or_hex("bundle_id_hex", &spec.bundle_id_hex)?;
    let receiver = parse_account_id(&spec.receiver)?;
    let deposit_account = parse_account_id(&spec.deposit_account)?;
    let receipts = build_receipts(&spec.receipts, base_dir)?;
    if receipts.is_empty() {
        return Err(eyre!(
            "bundle `{}` must contain at least one receipt",
            spec.label
        ));
    }
    let balance_proof = build_balance_proof(&spec.balance_proof)?;
    let aggregate_proof = build_aggregate_proof(spec.aggregate_proof.as_ref(), &receipts)?;
    Ok(OfflineToOnlineTransfer {
        bundle_id,
        receiver,
        deposit_account,
        receipts,
        balance_proof,
        balance_proofs: None,
        aggregate_proof,
        attachments: None,
        platform_snapshot: None,
    })
}

fn build_balance_proof(spec: &BalanceProofSpec) -> Result<OfflineBalanceProof> {
    let asset = parse_asset_id(&spec.initial_asset)?;
    let amount = parse_numeric(&spec.initial_amount)?;
    let initial_commitment = hex::decode(&spec.initial_commitment_hex).wrap_err_with(|| {
        format!(
            "invalid initial commitment hex `{}`",
            spec.initial_commitment_hex
        )
    })?;
    let resulting_commitment = hex::decode(&spec.resulting_commitment_hex).wrap_err_with(|| {
        format!(
            "invalid resulting commitment hex `{}`",
            spec.resulting_commitment_hex
        )
    })?;
    let claimed_delta = parse_numeric(&spec.claimed_delta)?;
    let zk_proof = spec
        .zk_proof_hex
        .as_ref()
        .map(hex::decode)
        .transpose()
        .wrap_err("failed to decode zk proof hex")?;
    Ok(OfflineBalanceProof {
        initial_commitment: OfflineAllowanceCommitment {
            asset,
            amount,
            commitment: initial_commitment,
        },
        resulting_commitment,
        claimed_delta,
        zk_proof,
    })
}

fn build_receipts(specs: &[ReceiptSpec], base_dir: &Path) -> Result<Vec<OfflineSpendReceipt>> {
    let mut receipts = Vec::with_capacity(specs.len());
    for spec in specs {
        let tx_id = parse_hash_or_hex("tx_id_hex", &spec.tx_id_hex)?;
        let from = parse_account_id(&spec.from)?;
        let to = parse_account_id(&spec.to)?;
        let asset = parse_asset_id(&spec.asset)?;
        let amount = parse_numeric(&spec.amount)?;
        let platform_proof = spec.platform_proof.to_model()?;
        let platform_snapshot = spec
            .platform_snapshot
            .as_ref()
            .map(PlatformSnapshotSpec::to_model);
        let certificate = spec.sender_certificate.load(base_dir)?;
        let sender_signature = Signature::from_hex(&spec.sender_signature_hex)
            .wrap_err_with(|| format!("invalid sender signature `{}`", spec.label()))?;
        receipts.push(OfflineSpendReceipt {
            tx_id,
            from,
            to,
            asset,
            amount,
            issued_at_ms: spec.issued_at_ms,
            invoice_id: spec.invoice_id.clone(),
            platform_proof,
            platform_snapshot,
            sender_certificate_id: certificate.certificate_id(),
            sender_signature,
        });
    }
    Ok(receipts)
}

fn build_record(
    spec: &BundleSpec,
    transfer: OfflineToOnlineTransfer,
    base_dir: &Path,
) -> Result<OfflineTransferRecord> {
    let controller = if let Some(id) = &spec.controller {
        parse_account_id(id)?
    } else {
        transfer
            .receipts
            .first()
            .map(|receipt| receipt.from.clone())
            .ok_or_else(|| eyre!("bundle `{}` missing receipts", spec.label))?
    };
    let status = if let Some(value) = &spec.status {
        value
            .parse::<OfflineTransferStatus>()
            .map_err(|_| eyre!("invalid offline transfer status `{value}`"))?
    } else {
        OfflineTransferStatus::Settled
    };
    let primary_certificate = spec
        .receipts
        .first()
        .ok_or_else(|| eyre!("bundle `{}` must contain at least one receipt", spec.label))?
        .sender_certificate
        .load(base_dir)?;
    let pos_verdict_snapshots =
        iroha_data_model::offline::OfflineTransferRecord::collect_pos_verdict_snapshots(
            &transfer,
            &primary_certificate,
        );
    Ok(OfflineTransferRecord {
        transfer,
        controller,
        status,
        rejection_reason: None,
        recorded_at_ms: spec.recorded_at_ms.unwrap_or_default(),
        recorded_at_height: spec.recorded_at_height.unwrap_or_default(),
        archived_at_height: spec.archived_at_height,
        history: Vec::new(),
        pos_verdict_snapshots,
        verdict_snapshot: None,
        platform_snapshot: None,
    })
}

fn build_replay_request(
    spec: &BundleSpec,
    record: &OfflineTransferRecord,
) -> Result<Option<OfflineProofRequestReplay>> {
    let (head_hex, tail_hex) = match (&spec.replay_log_head_hex, &spec.replay_log_tail_hex) {
        (Some(head), Some(tail)) => (head, tail),
        (None, None) => return Ok(None),
        _ => {
            return Err(eyre!(
                "bundle `{}` must provide both replay head and tail",
                spec.label
            ));
        }
    };
    let head = parse_hash_or_hex("replay_log_head_hex", head_hex)?;
    let tail = parse_hash_or_hex("replay_log_tail_hex", tail_hex)?;
    let request = record
        .to_proof_request_replay(head, tail)
        .wrap_err("failed to build replay request")?;
    Ok(Some(request))
}

fn compute_receipts_root(transfer: &OfflineToOnlineTransfer) -> Result<PoseidonDigest> {
    iroha_data_model::offline::compute_receipts_root(&transfer.receipts)
        .wrap_err("failed to compute receipts root")
}

fn parse_hash_or_hex(field: &str, value: &str) -> Result<Hash> {
    match Hash::from_str(value) {
        Ok(hash) => Ok(hash),
        Err(_) => {
            let bytes =
                hex::decode(value).map_err(|err| eyre!("invalid {field} `{value}`: {err}"))?;
            if bytes.len() != Hash::LENGTH {
                return Err(eyre!(
                    "invalid {field} `{value}`: expected {} bytes (got {})",
                    Hash::LENGTH,
                    bytes.len()
                ));
            }
            let mut array = [0u8; Hash::LENGTH];
            array.copy_from_slice(&bytes);
            Ok(Hash::prehashed(array))
        }
    }
}

fn build_aggregate_proof(
    input: Option<&AggregateProofSpec>,
    receipts: &[OfflineSpendReceipt],
) -> Result<Option<AggregateProofEnvelope>> {
    if let Some(spec) = input {
        let receipts_root = iroha_data_model::offline::compute_receipts_root(receipts)
            .wrap_err("failed to compute aggregate proof receipts root")?;
        let metadata = spec
            .metadata
            .as_ref()
            .map(|value| build_metadata(Some(value), None))
            .transpose()
            .wrap_err("failed to parse aggregate proof metadata")?
            .unwrap_or_else(Metadata::default);
        let proof_sum = parse_hex_blob(spec.proof_sum_hex.as_deref())?;
        let proof_counter = parse_hex_blob(spec.proof_counter_hex.as_deref())?;
        let proof_replay = parse_hex_blob(spec.proof_replay_hex.as_deref())?;
        return Ok(Some(AggregateProofEnvelope {
            version: spec.version.unwrap_or(1),
            receipts_root,
            proof_sum,
            proof_counter,
            proof_replay,
            metadata,
        }));
    }
    Ok(None)
}

fn parse_hex_blob(value: Option<&str>) -> Result<Option<Vec<u8>>> {
    value
        .map(|hex| hex::decode(hex).map_err(|err| eyre!("invalid hex payload `{hex}`: {err}")))
        .transpose()
}

#[derive(JsonDeserialize)]
struct BundleSpecFile {
    bundles: Vec<BundleSpec>,
}

#[derive(JsonDeserialize)]
struct BundleSpec {
    label: String,
    bundle_id_hex: String,
    receiver: String,
    deposit_account: String,
    #[norito(default)]
    controller: Option<String>,
    #[norito(default)]
    status: Option<String>,
    #[norito(default)]
    recorded_at_ms: Option<u64>,
    #[norito(default)]
    recorded_at_height: Option<u64>,
    #[norito(default)]
    archived_at_height: Option<u64>,
    balance_proof: BalanceProofSpec,
    receipts: Vec<ReceiptSpec>,
    #[norito(default)]
    aggregate_proof: Option<AggregateProofSpec>,
    #[norito(default)]
    counter_checkpoint: Option<u64>,
    #[norito(default)]
    replay_log_head_hex: Option<String>,
    #[norito(default)]
    replay_log_tail_hex: Option<String>,
}

#[derive(JsonDeserialize)]
struct BalanceProofSpec {
    initial_commitment_hex: String,
    resulting_commitment_hex: String,
    claimed_delta: String,
    #[norito(default)]
    zk_proof_hex: Option<String>,
    initial_asset: String,
    initial_amount: String,
}

#[derive(JsonDeserialize)]
struct ReceiptSpec {
    tx_id_hex: String,
    from: String,
    to: String,
    asset: String,
    amount: String,
    issued_at_ms: u64,
    invoice_id: String,
    platform_proof: PlatformProofSpec,
    #[norito(default)]
    platform_snapshot: Option<PlatformSnapshotSpec>,
    sender_certificate: CertificateSource,
    sender_signature_hex: String,
}

impl ReceiptSpec {
    fn label(&self) -> String {
        format!("receipt {}", self.tx_id_hex)
    }
}

#[derive(JsonDeserialize)]
#[norito(tag = "kind", content = "payload")]
enum PlatformProofSpec {
    #[norito(rename = "apple_app_attest")]
    Apple {
        key_id: String,
        counter: u64,
        assertion_b64: String,
        challenge_hash_hex: String,
    },
    #[norito(rename = "android_marker_key")]
    AndroidMarkerKey {
        series: String,
        counter: u64,
        marker_public_key: String,
        #[norito(default)]
        marker_signature_hex: Option<String>,
        attestation_b64: String,
    },
    #[norito(rename = "android_provisioned")]
    AndroidProvisioned {
        manifest_schema: String,
        #[norito(default)]
        manifest_version: Option<u32>,
        manifest_issued_at_ms: u64,
        challenge_hash_hex: String,
        counter: u64,
        device_manifest: JsonValue,
        inspector_signature_hex: String,
    },
}

#[derive(JsonDeserialize)]
struct PlatformSnapshotSpec {
    policy: String,
    attestation_jws_b64: String,
}

impl PlatformSnapshotSpec {
    fn to_model(&self) -> OfflinePlatformTokenSnapshot {
        OfflinePlatformTokenSnapshot {
            policy: self.policy.clone(),
            attestation_jws_b64: self.attestation_jws_b64.clone(),
        }
    }
}

impl PlatformProofSpec {
    fn to_model(&self) -> Result<OfflinePlatformProof> {
        match self {
            PlatformProofSpec::Apple {
                key_id,
                counter,
                assertion_b64,
                challenge_hash_hex,
            } => {
                let key_id =
                    canonical_app_attest_key_id(key_id).wrap_err("invalid App Attest key id")?;
                let assertion = BASE64
                    .decode(assertion_b64)
                    .wrap_err("invalid App Attest assertion base64")?;
                let challenge_hash = parse_hash_or_hex("challenge_hash_hex", challenge_hash_hex)?;
                Ok(OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof {
                    key_id,
                    counter: *counter,
                    assertion,
                    challenge_hash,
                }))
            }
            PlatformProofSpec::AndroidMarkerKey {
                series,
                counter,
                marker_public_key,
                marker_signature_hex,
                attestation_b64,
            } => {
                let marker_public_key = hex::decode(marker_public_key).wrap_err_with(|| {
                    format!("invalid marker public key hex `{marker_public_key}`")
                })?;
                if marker_public_key.len() != 65 {
                    return Err(eyre!(
                        "invalid marker public key length: expected 65 bytes (got {})",
                        marker_public_key.len()
                    ));
                }
                let marker_signature = marker_signature_hex
                    .as_ref()
                    .map(|hex| {
                        hex::decode(hex)
                            .wrap_err_with(|| format!("invalid marker signature hex `{hex}`"))
                    })
                    .transpose()?;
                if let Some(signature) = &marker_signature
                    && signature.len() != 64
                {
                    return Err(eyre!(
                        "invalid marker signature length: expected 64 bytes (got {})",
                        signature.len()
                    ));
                }
                let attestation = BASE64
                    .decode(attestation_b64)
                    .wrap_err("invalid attestation base64")?;
                Ok(OfflinePlatformProof::AndroidMarkerKey(
                    AndroidMarkerKeyProof {
                        series: series.clone(),
                        counter: *counter,
                        marker_public_key,
                        marker_signature,
                        attestation,
                    },
                ))
            }
            PlatformProofSpec::AndroidProvisioned {
                manifest_schema,
                manifest_version,
                manifest_issued_at_ms,
                challenge_hash_hex,
                counter,
                device_manifest,
                inspector_signature_hex,
            } => {
                let metadata = build_metadata(Some(device_manifest), None).wrap_err_with(
                    || "android provisioned device manifest must be a JSON object",
                )?;
                let challenge_hash = parse_hash_or_hex("challenge_hash_hex", challenge_hash_hex)?;
                let inspector_signature = Signature::from_hex(inspector_signature_hex)
                    .wrap_err_with(|| {
                        format!("invalid inspector signature `{inspector_signature_hex}`")
                    })?;
                Ok(OfflinePlatformProof::Provisioned(AndroidProvisionedProof {
                    manifest_schema: manifest_schema.clone(),
                    manifest_version: *manifest_version,
                    manifest_issued_at_ms: *manifest_issued_at_ms,
                    challenge_hash,
                    counter: *counter,
                    device_manifest: metadata,
                    inspector_signature,
                }))
            }
        }
    }
}

#[derive(JsonDeserialize)]
struct AggregateProofSpec {
    #[norito(default)]
    version: Option<u16>,
    #[norito(default)]
    proof_sum_hex: Option<String>,
    #[norito(default)]
    proof_counter_hex: Option<String>,
    #[norito(default)]
    proof_replay_hex: Option<String>,
    #[norito(default)]
    metadata: Option<JsonValue>,
}

#[derive(JsonDeserialize)]
struct CertificateSource {
    certificate_path: PathBuf,
}

impl CertificateSource {
    fn load(&self, base_dir: &Path) -> Result<OfflineWalletCertificate> {
        let path = base_dir.join(&self.certificate_path);
        let bytes = fs::read(&path)
            .wrap_err_with(|| format!("failed to read certificate {}", path.display()))?;
        let value: JsonValue = serde_json::from_slice(&bytes)
            .wrap_err_with(|| format!("failed to parse certificate JSON {}", path.display()))?;
        parse_certificate_json(value)
    }
}

fn parse_certificate_json(value: JsonValue) -> Result<OfflineWalletCertificate> {
    let obj = value
        .as_object()
        .ok_or_else(|| eyre!("certificate JSON must be an object"))?;
    let controller = obj
        .get("controller")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre!("certificate missing controller"))?;
    let controller = parse_account_id(controller)?;
    let operator = obj
        .get("operator")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre!("certificate missing operator"))?;
    let operator = parse_account_id(operator)?;
    let allowance = obj
        .get("allowance")
        .and_then(|v| v.as_object())
        .ok_or_else(|| eyre!("certificate missing allowance block"))?;
    let allowance_asset = allowance
        .get("asset")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre!("allowance missing asset"))?;
    let asset = parse_asset_id(allowance_asset)?;
    let amount_str = allowance
        .get("amount")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre!("allowance missing amount"))?;
    let amount = parse_numeric(amount_str)?;
    let commitment_hex = allowance
        .get("commitment_hex")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre!("allowance missing commitment_hex"))?;
    let commitment = hex::decode(commitment_hex)
        .wrap_err_with(|| format!("invalid commitment hex `{commitment_hex}`"))?;
    let spend_public_key = obj
        .get("spend_public_key")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre!("certificate missing spend_public_key"))?;
    let spend_public_key =
        PublicKey::from_str(spend_public_key).wrap_err("invalid spend public key")?;
    let attestation_report_b64 = obj
        .get("attestation_report_b64")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre!("certificate missing attestation_report_b64"))?;
    let attestation_report = BASE64
        .decode(attestation_report_b64)
        .wrap_err("invalid attestation base64")?;
    let issued_at_ms = obj
        .get("issued_at_ms")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| eyre!("certificate missing issued_at_ms"))?;
    let expires_at_ms = obj
        .get("expires_at_ms")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| eyre!("certificate missing expires_at_ms"))?;
    let policy = obj
        .get("policy")
        .and_then(|v| v.as_object())
        .ok_or_else(|| eyre!("certificate missing policy"))?;
    let max_balance = policy
        .get("max_balance")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre!("policy missing max_balance"))?;
    let max_tx_value = policy
        .get("max_tx_value")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre!("policy missing max_tx_value"))?;
    let policy_expires = policy
        .get("expires_at_ms")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| eyre!("policy missing expires_at_ms"))?;
    let operator_signature_hex = obj
        .get("operator_signature_hex")
        .and_then(|v| v.as_str())
        .ok_or_else(|| eyre!("certificate missing operator_signature_hex"))?;
    let operator_signature = Signature::from_hex(operator_signature_hex)
        .wrap_err_with(|| format!("invalid operator signature `{operator_signature_hex}`"))?;
    let metadata = obj
        .get("metadata")
        .map(|value| build_metadata(Some(value), None))
        .transpose()
        .wrap_err("failed to parse certificate metadata")?
        .unwrap_or_else(Metadata::default);
    let verdict_id = parse_optional_hash(obj.get("verdict_id_hex"))?;
    let attestation_nonce = parse_optional_hash(obj.get("attestation_nonce_hex"))?;
    let refresh_at_ms = obj.get("refresh_at_ms").and_then(|v| v.as_u64());

    Ok(OfflineWalletCertificate {
        controller,
        operator,
        allowance: OfflineAllowanceCommitment {
            asset,
            amount,
            commitment,
        },
        spend_public_key,
        attestation_report,
        issued_at_ms,
        expires_at_ms,
        policy: OfflineWalletPolicy {
            max_balance: parse_numeric(max_balance)?,
            max_tx_value: parse_numeric(max_tx_value)?,
            expires_at_ms: policy_expires,
        },
        operator_signature,
        metadata,
        verdict_id,
        attestation_nonce,
        refresh_at_ms,
    })
}

fn parse_optional_hash(value: Option<&JsonValue>) -> Result<Option<Hash>> {
    value
        .and_then(|v| v.as_str())
        .map(|hex| parse_hash_hex("hash", hex))
        .transpose()
}

#[derive(JsonSerialize)]
struct BundleManifest {
    label: String,
    receipt_count: usize,
    receipts_root_hex: String,
    bundle_json: String,
    bundle_norito: String,
    proof_sum_request: String,
    proof_counter_request: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    proof_replay_request: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_proof_apple_roundtrip() {
        let key_id = BASE64.encode(b"demo");
        let spec = PlatformProofSpec::Apple {
            key_id: key_id.clone(),
            counter: 42,
            assertion_b64: BASE64.encode([1, 2, 3]),
            challenge_hash_hex: "E8A8D90BF72F280BBB4AB6D1F759521D29A08DA83CCFBB3E2EE0EDE22606FB9B"
                .into(),
        };
        let proof = spec.to_model().expect("proof");
        match proof {
            OfflinePlatformProof::AppleAppAttest(inner) => {
                assert_eq!(inner.key_id, key_id);
                assert_eq!(inner.counter, 42);
                assert_eq!(inner.assertion, vec![1, 2, 3]);
            }
            _ => panic!("unexpected variant"),
        }
    }
}
