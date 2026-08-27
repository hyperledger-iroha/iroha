use crate::{JsonTarget, workspace_root, write_json_output};
use blake3::hash as blake3_hash;
use eyre::{Context as _, Result, ensure, eyre};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, PublicKey, Signature};
use iroha_data_model::taikai::{
    TAIKAI_ANCHOR_RECEIPT_BASE_ID_MAX_BYTES_V1, TaikaiAnchorReceiptV1,
    is_canonical_taikai_anchor_base_id,
};
use norito::{derive::JsonSerialize, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
const ANCHOR_REQUEST_PREFIX: &str = "taikai-anchor-request-";
const ANCHOR_REQUEST_SUFFIX: &str = ".json";
const SENTINEL_PREFIX: &str = "taikai-anchor-";
const SENTINEL_SUFFIX: &str = ".ok";
const ENVELOPE_PREFIX: &str = "taikai-envelope-";
const ENVELOPE_SUFFIX: &str = ".norito";
const INDEXES_PREFIX: &str = "taikai-indexes-";
const INDEXES_SUFFIX: &str = ".json";
const SSM_PREFIX: &str = "taikai-ssm-";
const SSM_SUFFIX: &str = ".norito";
const TRM_PREFIX: &str = "taikai-trm-";
const TRM_SUFFIX: &str = ".norito";
const TRM_STATE_PREFIX: &str = "taikai-trm-state-";
const TRM_STATE_SUFFIX: &str = ".json";
const LINEAGE_PREFIX: &str = "taikai-lineage-";
const LINEAGE_SUFFIX: &str = ".json";
const ANCHOR_RECEIPT_MAX_BYTES: usize = 8 * 1024;
const ANCHOR_REQUEST_MAX_BYTES: usize = 16 * 1024 * 1024;
#[derive(Debug)]
pub struct AnchorBundleOptions {
    pub spool_dir: PathBuf,
    pub copy_dir: Option<PathBuf>,
    pub signing_key: Option<PathBuf>,
    pub receipt_public_key: Option<PublicKey>,
    pub output: JsonTarget,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AnchorStatus {
    Pending,
    ReceiptUnverified,
    InvalidReceipt,
    Delivered,
}
impl AnchorStatus {
    fn label(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::ReceiptUnverified => "receipt_unverified",
            Self::InvalidReceipt => "invalid_receipt",
            Self::Delivered => "delivered",
        }
    }
}
#[derive(Debug, Clone, JsonSerialize)]
struct ArtifactDigest {
    path: String,
    bytes: u64,
    sha256: String,
    blake3: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    copied_path: Option<String>,
}
#[derive(Debug, Clone, JsonSerialize)]
struct SentinelInfo {
    path: String,
    bytes: u64,
    sha256: String,
    blake3: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    marker: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    copied_path: Option<String>,
}
#[derive(Debug, Clone, JsonSerialize)]
struct AnchorEntry {
    base_id: String,
    status: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    receipt_validation_error: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    envelope: Option<ArtifactDigest>,
    #[norito(skip_serializing_if = "Option::is_none")]
    indexes: Option<ArtifactDigest>,
    #[norito(skip_serializing_if = "Option::is_none")]
    ssm: Option<ArtifactDigest>,
    #[norito(skip_serializing_if = "Option::is_none")]
    trm: Option<ArtifactDigest>,
    #[norito(skip_serializing_if = "Option::is_none")]
    trm_state: Option<ArtifactDigest>,
    #[norito(skip_serializing_if = "Option::is_none")]
    lineage: Option<ArtifactDigest>,
    #[norito(skip_serializing_if = "Option::is_none")]
    anchor_request: Option<ArtifactDigest>,
    #[norito(skip_serializing_if = "Option::is_none")]
    sentinel: Option<SentinelInfo>,
}
#[derive(Debug, JsonSerialize)]
struct AnchorBundleSummary {
    generated_unix_ms: u64,
    spool_dir: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    receipt_public_key: Option<String>,
    total_entries: usize,
    delivered: usize,
    pending: usize,
    receipt_unverified: usize,
    invalid_receipt: usize,
    entries: Vec<AnchorEntry>,
    #[norito(skip_serializing_if = "Option::is_none")]
    signing: Option<SignatureEnvelope>,
}
#[derive(Debug, JsonSerialize)]
struct SignatureEnvelope {
    algorithm: String,
    public_key_hex: String,
    signature_hex: String,
    payload_sha256: String,
    payload_blake3: String,
}
#[derive(Default)]
struct AnchorPaths {
    envelope: Option<PathBuf>,
    indexes: Option<PathBuf>,
    ssm: Option<PathBuf>,
    trm: Option<PathBuf>,
    trm_state: Option<PathBuf>,
    lineage: Option<PathBuf>,
    anchor_request: Option<PathBuf>,
    sentinel: Option<PathBuf>,
}
fn artifact_base_id<'a>(name: &'a str, prefix: &str, suffix: &str) -> Result<Option<&'a str>> {
    let Some(base_id) = name
        .strip_prefix(prefix)
        .and_then(|rest| rest.strip_suffix(suffix))
    else {
        return Ok(None);
    };
    let mut components = Path::new(base_id).components();
    ensure!(
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none(),
        "Taikai spool artifact `{name}` has unsafe base id `{base_id}`"
    );
    Ok(Some(base_id))
}
pub fn run_anchor_bundle(options: AnchorBundleOptions) -> Result<()> {
    let entries = collect_anchor_entries(
        &options.spool_dir,
        options.copy_dir.as_deref(),
        options.receipt_public_key.as_ref(),
    )?;
    let status_count = |status: AnchorStatus| {
        entries
            .iter()
            .filter(|entry| entry.status == status.label())
            .count()
    };
    let mut summary = AnchorBundleSummary {
        generated_unix_ms: unix_ms_now(),
        spool_dir: display_path(&options.spool_dir),
        receipt_public_key: options.receipt_public_key.as_ref().map(ToString::to_string),
        total_entries: entries.len(),
        delivered: status_count(AnchorStatus::Delivered),
        pending: status_count(AnchorStatus::Pending),
        receipt_unverified: status_count(AnchorStatus::ReceiptUnverified),
        invalid_receipt: status_count(AnchorStatus::InvalidReceipt),
        entries,
        signing: None,
    };
    let unsigned = json::to_value(&summary)?;
    if let Some(signing_key) = options.signing_key.as_ref() {
        let payload = json::to_vec(&unsigned)?;
        let signature = sign_payload(&payload, signing_key)?;
        summary.signing = Some(signature);
    }
    let value = json::to_value(&summary)?;
    write_json_output(&value, options.output).map_err(|err| eyre!(err.to_string()))
}
fn collect_anchor_entries(
    spool_dir: &Path,
    copy_dir: Option<&Path>,
    receipt_public_key: Option<&PublicKey>,
) -> Result<Vec<AnchorEntry>> {
    let mut entries: BTreeMap<String, AnchorPaths> = BTreeMap::new();
    let dir_iter = match fs::read_dir(spool_dir) {
        Ok(iter) => iter,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => {
            return Err(eyre!(
                "failed to read Taikai spool directory {}: {err}",
                spool_dir.display()
            ));
        }
    };
    for entry in dir_iter {
        let entry = entry?;
        let name = match entry.file_name().into_string() {
            Ok(value) => value,
            Err(_) => continue,
        };
        if let Some(base) = artifact_base_id(&name, ANCHOR_REQUEST_PREFIX, ANCHOR_REQUEST_SUFFIX)? {
            entries.entry(base.to_string()).or_default().anchor_request = Some(entry.path());
            continue;
        }
        if let Some(base) = artifact_base_id(&name, SENTINEL_PREFIX, SENTINEL_SUFFIX)? {
            entries.entry(base.to_string()).or_default().sentinel = Some(entry.path());
            continue;
        }
        if let Some(base) = artifact_base_id(&name, ENVELOPE_PREFIX, ENVELOPE_SUFFIX)? {
            entries.entry(base.to_string()).or_default().envelope = Some(entry.path());
            continue;
        }
        if let Some(base) = artifact_base_id(&name, INDEXES_PREFIX, INDEXES_SUFFIX)? {
            entries.entry(base.to_string()).or_default().indexes = Some(entry.path());
            continue;
        }
        if let Some(base) = artifact_base_id(&name, SSM_PREFIX, SSM_SUFFIX)? {
            entries.entry(base.to_string()).or_default().ssm = Some(entry.path());
            continue;
        }
        if let Some(base) = artifact_base_id(&name, TRM_PREFIX, TRM_SUFFIX)? {
            entries.entry(base.to_string()).or_default().trm = Some(entry.path());
            continue;
        }
        if let Some(base) = artifact_base_id(&name, TRM_STATE_PREFIX, TRM_STATE_SUFFIX)? {
            entries.entry(base.to_string()).or_default().trm_state = Some(entry.path());
            continue;
        }
        if let Some(base) = artifact_base_id(&name, LINEAGE_PREFIX, LINEAGE_SUFFIX)? {
            entries.entry(base.to_string()).or_default().lineage = Some(entry.path());
            continue;
        }
    }
    let mut summaries = Vec::with_capacity(entries.len());
    for (base_id, paths) in entries {
        let (status, receipt_validation_error) =
            classify_anchor_status(&base_id, &paths, receipt_public_key);
        let anchor_request = digest_optional_file(&paths.anchor_request, copy_dir, &base_id)?;
        let sentinel = digest_sentinel(&paths.sentinel, copy_dir, &base_id)?;
        let envelope = digest_optional_file(&paths.envelope, copy_dir, &base_id)?;
        let indexes = digest_optional_file(&paths.indexes, copy_dir, &base_id)?;
        let ssm = digest_optional_file(&paths.ssm, copy_dir, &base_id)?;
        let trm = digest_optional_file(&paths.trm, copy_dir, &base_id)?;
        let trm_state = digest_optional_file(&paths.trm_state, copy_dir, &base_id)?;
        let lineage = digest_optional_file(&paths.lineage, copy_dir, &base_id)?;
        summaries.push(AnchorEntry {
            base_id,
            status: status.label().to_string(),
            receipt_validation_error,
            envelope,
            indexes,
            ssm,
            trm,
            trm_state,
            lineage,
            anchor_request,
            sentinel,
        });
    }
    Ok(summaries)
}

fn classify_anchor_status(
    base_id: &str,
    paths: &AnchorPaths,
    receipt_public_key: Option<&PublicKey>,
) -> (AnchorStatus, Option<String>) {
    if paths.sentinel.is_none() {
        return (AnchorStatus::Pending, None);
    }
    let Some(receipt_public_key) = receipt_public_key else {
        return (AnchorStatus::ReceiptUnverified, None);
    };
    match verify_anchor_receipt(base_id, paths, receipt_public_key) {
        Ok(()) => (AnchorStatus::Delivered, None),
        Err(err) => (AnchorStatus::InvalidReceipt, Some(err.to_string())),
    }
}

fn verify_anchor_receipt(
    base_id: &str,
    paths: &AnchorPaths,
    receipt_public_key: &PublicKey,
) -> Result<()> {
    ensure!(
        base_id.len() <= TAIKAI_ANCHOR_RECEIPT_BASE_ID_MAX_BYTES_V1
            && is_canonical_taikai_anchor_base_id(base_id),
        "Taikai anchor receipt filename base_id `{base_id}` is not canonical"
    );
    let sentinel_path = paths
        .sentinel
        .as_deref()
        .ok_or_else(|| eyre!("Taikai anchor receipt is missing"))?;
    let request_path = paths
        .anchor_request
        .as_deref()
        .ok_or_else(|| eyre!("Taikai anchor receipt `{base_id}` has no exact request capture"))?;
    let encoded = read_regular_file_bounded(
        sentinel_path,
        ANCHOR_RECEIPT_MAX_BYTES,
        "Taikai anchor receipt",
    )?;
    ensure!(!encoded.is_empty(), "Taikai anchor receipt is empty");
    let request = read_regular_file_bounded(
        request_path,
        ANCHOR_REQUEST_MAX_BYTES,
        "Taikai anchor request capture",
    )?;
    let receipt: TaikaiAnchorReceiptV1 = json::from_slice(&encoded)
        .map_err(|err| eyre!("failed to decode Taikai anchor receipt JSON: {err}"))?;
    ensure!(
        receipt.body.base_id == base_id,
        "Taikai anchor receipt base_id `{}` does not match filename `{base_id}`",
        receipt.body.base_id
    );
    ensure!(
        receipt.body.request_digest == *blake3_hash(&request).as_bytes(),
        "Taikai anchor receipt request digest does not match the exact request capture"
    );
    receipt
        .verify(receipt_public_key)
        .map_err(|err| eyre!("Taikai anchor receipt signature validation failed: {err}"))
}

fn read_regular_file_bounded(path: &Path, maximum: usize, label: &str) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {label} {}", path.display()))?;
    ensure!(
        metadata.file_type().is_file(),
        "{label} is not a regular file: {}",
        path.display()
    );
    ensure!(
        metadata.len() <= u64::try_from(maximum).unwrap_or(u64::MAX),
        "{label} {} is {} bytes, exceeding {maximum}",
        path.display(),
        metadata.len()
    );
    let bytes =
        fs::read(path).with_context(|| format!("failed to read {label} {}", path.display()))?;
    ensure!(
        bytes.len() <= maximum,
        "{label} {} grew beyond {maximum} bytes while being read",
        path.display()
    );
    Ok(bytes)
}

fn digest_optional_file(
    path: &Option<PathBuf>,
    copy_dir: Option<&Path>,
    base_id: &str,
) -> Result<Option<ArtifactDigest>> {
    let Some(path) = path else {
        return Ok(None);
    };
    let digests = hash_file(path)?;
    let copied = copy_if_requested(path, copy_dir, base_id)?;
    Ok(Some(ArtifactDigest {
        path: display_path(path),
        bytes: digests.bytes,
        sha256: digests.sha256,
        blake3: digests.blake3,
        copied_path: copied,
    }))
}
fn digest_sentinel(
    path: &Option<PathBuf>,
    copy_dir: Option<&Path>,
    base_id: &str,
) -> Result<Option<SentinelInfo>> {
    let Some(path) = path else {
        return Ok(None);
    };
    let bytes = read_regular_file_bounded(path, ANCHOR_RECEIPT_MAX_BYTES, "Taikai anchor receipt")?;
    let digests = hash_bytes(&bytes);
    let marker = std::str::from_utf8(&bytes)
        .ok()
        .map(|s| s.trim().to_string());
    let copied = copy_if_requested(path, copy_dir, base_id)?;
    Ok(Some(SentinelInfo {
        path: display_path(path),
        bytes: digests.bytes,
        sha256: digests.sha256,
        blake3: digests.blake3,
        marker,
        copied_path: copied,
    }))
}
fn hash_file(path: &Path) -> Result<HashedBytes> {
    let bytes =
        fs::read(path).with_context(|| format!("failed to read artefact {}", path.display()))?;
    Ok(hash_bytes(&bytes))
}
fn hash_bytes(bytes: &[u8]) -> HashedBytes {
    let mut sha = Sha256::new();
    sha.update(bytes);
    let sha = sha.finalize();
    let blake3 = blake3_hash(bytes);
    HashedBytes {
        bytes: bytes.len() as u64,
        sha256: hex::encode(sha),
        blake3: blake3.to_hex().to_string(),
    }
}
fn copy_if_requested(
    path: &Path,
    copy_dir: Option<&Path>,
    base_id: &str,
) -> Result<Option<String>> {
    let Some(root) = copy_dir else {
        return Ok(None);
    };
    let copy_root = root.join(base_id);
    fs::create_dir_all(&copy_root)
        .with_context(|| format!("failed to create copy dir {}", copy_root.display()))?;
    let file_name = path
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("artefact"));
    let dest = copy_root.join(file_name);
    fs::copy(path, &dest)
        .with_context(|| format!("failed to copy {} to {}", path.display(), dest.display()))?;
    Ok(Some(display_path(&dest)))
}
fn sign_payload(payload: &[u8], signing_key: &Path) -> Result<SignatureEnvelope> {
    let key_text = fs::read_to_string(signing_key)
        .with_context(|| format!("failed to read signing key {}", signing_key.display()))?;
    let cleaned: String = key_text
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let private_key = PrivateKey::from_hex(Algorithm::Ed25519, &cleaned).map_err(|err| {
        eyre!(
            "failed to parse signing key {} as Ed25519 hex: {err}",
            signing_key.display()
        )
    })?;
    let key_pair: KeyPair = private_key.into();
    let signature = Signature::try_new(key_pair.private_key(), payload)
        .map_err(|err| eyre!("failed to sign Taikai anchor payload: {err}"))?;
    let (algorithm, public) = key_pair
        .public_key()
        .try_to_bytes()
        .map_err(|err| eyre!("signing public key is malformed: {err}"))?;
    ensure!(
        algorithm == Algorithm::Ed25519,
        "only Ed25519 signing keys are supported"
    );
    let payload_hashes = hash_bytes(payload);
    Ok(SignatureEnvelope {
        algorithm: "ed25519".to_string(),
        public_key_hex: hex::encode(public),
        signature_hex: hex::encode(signature.payload()),
        payload_sha256: payload_hashes.sha256,
        payload_blake3: payload_hashes.blake3,
    })
}
fn unix_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
fn display_path(path: &Path) -> String {
    if let Ok(relative) = path.strip_prefix(workspace_root()) {
        relative.display().to_string()
    } else {
        path.display().to_string()
    }
}
struct HashedBytes {
    bytes: u64,
    sha256: String,
    blake3: String,
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::Algorithm;
    use iroha_data_model::taikai::{
        TAIKAI_ANCHOR_RECEIPT_SCHEMA_V1, TAIKAI_ANCHOR_RECEIPT_VERSION_V1,
        TaikaiAnchorReceiptBodyV1,
    };
    use norito::json::Value;
    use tempfile::tempdir;

    fn anchor_base_id(sequence: u64) -> String {
        format!(
            "00000001-0000000000000002-{sequence:016x}-\
             aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-\
             bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        )
    }

    fn anchor_signer(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive deterministic anchor signer")
    }

    fn signed_receipt(base_id: &str, request: &[u8], signer: &KeyPair) -> Vec<u8> {
        let body = TaikaiAnchorReceiptBodyV1 {
            schema: TAIKAI_ANCHOR_RECEIPT_SCHEMA_V1.to_owned(),
            version: TAIKAI_ANCHOR_RECEIPT_VERSION_V1,
            base_id: base_id.to_owned(),
            request_digest: *blake3_hash(request).as_bytes(),
            acknowledged_unix_secs: 1_750_000_000,
        };
        let receipt = TaikaiAnchorReceiptV1::try_sign(body, signer).expect("sign anchor receipt");
        json::to_vec(&receipt).expect("encode anchor receipt")
    }

    #[test]
    fn collects_pending_and_delivered_entries() -> Result<()> {
        let dir = tempdir()?;
        let spool = dir.path().join("taikai");
        fs::create_dir_all(&spool)?;
        let signer = anchor_signer(0xA7);
        let delivered = anchor_base_id(3);
        let delivered_request = b"{\"payload\":\"delivered\"}";
        fs::write(
            spool.join(format!(
                "{ANCHOR_REQUEST_PREFIX}{delivered}{ANCHOR_REQUEST_SUFFIX}"
            )),
            delivered_request,
        )?;
        fs::write(
            spool.join(format!("{SENTINEL_PREFIX}{delivered}{SENTINEL_SUFFIX}")),
            signed_receipt(&delivered, delivered_request, &signer),
        )?;
        fs::write(
            spool.join(format!("{ENVELOPE_PREFIX}{delivered}{ENVELOPE_SUFFIX}")),
            b"env",
        )?;
        fs::write(
            spool.join(format!("{INDEXES_PREFIX}{delivered}{INDEXES_SUFFIX}")),
            b"{\"idx\":1}",
        )?;
        fs::write(
            spool.join(format!("{SSM_PREFIX}{delivered}{SSM_SUFFIX}")),
            b"ssm",
        )?;
        fs::write(
            spool.join(format!("{TRM_PREFIX}{delivered}{TRM_SUFFIX}")),
            b"trm",
        )?;
        fs::write(
            spool.join(format!("{TRM_STATE_PREFIX}{delivered}{TRM_STATE_SUFFIX}")),
            br#"{"alias":"docs"}"#,
        )?;
        fs::write(
            spool.join(format!("{LINEAGE_PREFIX}{delivered}{LINEAGE_SUFFIX}")),
            b"{\"version\":1}",
        )?;
        let pending = anchor_base_id(4);
        fs::write(
            spool.join(format!(
                "{ANCHOR_REQUEST_PREFIX}{pending}{ANCHOR_REQUEST_SUFFIX}"
            )),
            b"{\"payload\":\"pending\"}",
        )?;
        let copy_dir = dir.path().join("bundle");
        let entries =
            collect_anchor_entries(&spool, Some(copy_dir.as_path()), Some(signer.public_key()))?;
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.status == AnchorStatus::Delivered.label())
                .count(),
            1
        );
        assert_eq!(
            entries
                .iter()
                .filter(|entry| entry.status == AnchorStatus::Pending.label())
                .count(),
            1
        );
        let delivered_entry = entries
            .iter()
            .find(|entry| entry.base_id == delivered.as_str())
            .expect("delivered entry");
        assert_eq!(delivered_entry.status, "delivered");
        assert!(delivered_entry.sentinel.is_some());
        assert!(
            delivered_entry
                .envelope
                .as_ref()
                .and_then(|digest| digest.copied_path.as_ref())
                .is_some()
        );
        assert!(
            delivered_entry
                .trm_state
                .as_ref()
                .and_then(|digest| digest.copied_path.as_ref())
                .is_some(),
            "expected TRM state copy to be recorded"
        );
        let pending_entry = entries
            .iter()
            .find(|entry| entry.base_id == pending.as_str())
            .expect("pending entry");
        assert_eq!(pending_entry.status, "pending");
        assert!(pending_entry.sentinel.is_none());
        assert!(
            pending_entry
                .anchor_request
                .as_ref()
                .and_then(|digest| digest.copied_path.as_ref())
                .is_some()
        );
        Ok(())
    }

    #[test]
    fn receipt_without_verifier_is_never_reported_delivered() -> Result<()> {
        let dir = tempdir()?;
        let spool = dir.path().join("taikai");
        fs::create_dir_all(&spool)?;
        let base_id = anchor_base_id(6);
        let request = b"{\"payload\":\"unverified\"}";
        let signer = anchor_signer(0xA7);
        fs::write(
            spool.join(format!(
                "{ANCHOR_REQUEST_PREFIX}{base_id}{ANCHOR_REQUEST_SUFFIX}"
            )),
            request,
        )?;
        fs::write(
            spool.join(format!("{SENTINEL_PREFIX}{base_id}{SENTINEL_SUFFIX}")),
            signed_receipt(&base_id, request, &signer),
        )?;

        let entries = collect_anchor_entries(&spool, None, None)?;

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].status, AnchorStatus::ReceiptUnverified.label());
        assert_ne!(entries[0].status, AnchorStatus::Delivered.label());
        Ok(())
    }

    #[test]
    fn invalid_receipt_bindings_are_reported_not_delivered() -> Result<()> {
        let dir = tempdir()?;
        let spool = dir.path().join("taikai");
        fs::create_dir_all(&spool)?;
        let verifier = anchor_signer(0xA7);
        let wrong_signer = anchor_signer(0xB8);

        let legacy = anchor_base_id(10);
        fs::write(
            spool.join(format!(
                "{ANCHOR_REQUEST_PREFIX}{legacy}{ANCHOR_REQUEST_SUFFIX}"
            )),
            b"legacy-request",
        )?;
        fs::write(
            spool.join(format!("{SENTINEL_PREFIX}{legacy}{SENTINEL_SUFFIX}")),
            b"1750000000\n",
        )?;

        let wrong_key = anchor_base_id(11);
        let wrong_key_request = b"wrong-key-request";
        fs::write(
            spool.join(format!(
                "{ANCHOR_REQUEST_PREFIX}{wrong_key}{ANCHOR_REQUEST_SUFFIX}"
            )),
            wrong_key_request,
        )?;
        fs::write(
            spool.join(format!("{SENTINEL_PREFIX}{wrong_key}{SENTINEL_SUFFIX}")),
            signed_receipt(&wrong_key, wrong_key_request, &wrong_signer),
        )?;

        let wrong_base = anchor_base_id(12);
        let other_base = anchor_base_id(13);
        let wrong_base_request = b"wrong-base-request";
        fs::write(
            spool.join(format!(
                "{ANCHOR_REQUEST_PREFIX}{wrong_base}{ANCHOR_REQUEST_SUFFIX}"
            )),
            wrong_base_request,
        )?;
        fs::write(
            spool.join(format!("{SENTINEL_PREFIX}{wrong_base}{SENTINEL_SUFFIX}")),
            signed_receipt(&other_base, wrong_base_request, &verifier),
        )?;

        let tampered_request = anchor_base_id(14);
        let original = b"original-request";
        fs::write(
            spool.join(format!(
                "{ANCHOR_REQUEST_PREFIX}{tampered_request}{ANCHOR_REQUEST_SUFFIX}"
            )),
            b"tampered-request",
        )?;
        fs::write(
            spool.join(format!(
                "{SENTINEL_PREFIX}{tampered_request}{SENTINEL_SUFFIX}"
            )),
            signed_receipt(&tampered_request, original, &verifier),
        )?;

        let missing_request = anchor_base_id(15);
        fs::write(
            spool.join(format!(
                "{SENTINEL_PREFIX}{missing_request}{SENTINEL_SUFFIX}"
            )),
            signed_receipt(&missing_request, b"missing", &verifier),
        )?;

        let entries = collect_anchor_entries(&spool, None, Some(verifier.public_key()))?;

        assert_eq!(entries.len(), 5);
        assert!(entries.iter().all(|entry| {
            entry.status == AnchorStatus::InvalidReceipt.label()
                && entry.receipt_validation_error.is_some()
        }));
        assert!(
            entries
                .iter()
                .all(|entry| entry.status != AnchorStatus::Delivered.label())
        );
        Ok(())
    }

    #[test]
    fn bundle_signs_summary_when_key_provided() -> Result<()> {
        let dir = tempdir()?;
        let spool = dir.path().join("taikai");
        fs::create_dir_all(&spool)?;
        let base = anchor_base_id(5);
        fs::write(
            spool.join(format!(
                "{ANCHOR_REQUEST_PREFIX}{base}{ANCHOR_REQUEST_SUFFIX}"
            )),
            b"{\"payload\":\"x\"}",
        )?;
        let key = iroha_crypto::KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let private_hex = hex::encode(key.private_key().to_bytes().1);
        let key_path = dir.path().join("signing.key");
        fs::write(&key_path, &private_hex)?;
        let out_path = dir.path().join("anchor_bundle.json");
        run_anchor_bundle(AnchorBundleOptions {
            spool_dir: spool,
            copy_dir: None,
            signing_key: Some(key_path.clone()),
            receipt_public_key: None,
            output: JsonTarget::File(out_path.clone()),
        })?;
        let value: Value = norito::json::from_reader(
            std::fs::File::open(&out_path).expect("bundle output readable"),
        )?;
        assert!(value.get("signing").is_some(), "signing block missing");
        Ok(())
    }

    #[test]
    fn rejects_artifact_base_id_that_escapes_copy_directory() -> Result<()> {
        let dir = tempdir()?;
        let spool = dir.path().join("taikai");
        fs::create_dir_all(&spool)?;
        let malicious_name = format!("{ANCHOR_REQUEST_PREFIX}..{ANCHOR_REQUEST_SUFFIX}");
        fs::write(spool.join(&malicious_name), b"malicious")?;
        let copy_dir = dir.path().join("bundle");

        let error = collect_anchor_entries(&spool, Some(&copy_dir), None)
            .expect_err("parent-directory base ids must be rejected");

        assert!(error.to_string().contains("unsafe base id"));
        assert!(
            !dir.path().join(malicious_name).exists(),
            "malicious artifact must not be copied outside the bundle root"
        );
        Ok(())
    }
}
