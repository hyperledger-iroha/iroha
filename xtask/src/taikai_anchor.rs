use std::{
    collections::BTreeMap,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use blake3::hash as blake3_hash;
use eyre::{Context as _, Result, ensure, eyre};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, Signature};
use norito::{derive::JsonSerialize, json};
use sha2::{Digest, Sha256};

use crate::{JsonTarget, workspace_root, write_json_output};

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

#[derive(Debug)]
pub struct AnchorBundleOptions {
    pub spool_dir: PathBuf,
    pub copy_dir: Option<PathBuf>,
    pub signing_key: Option<PathBuf>,
    pub output: JsonTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AnchorStatus {
    Pending,
    Delivered,
}

impl AnchorStatus {
    fn label(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
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
    total_entries: usize,
    delivered: usize,
    pending: usize,
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

pub fn run_anchor_bundle(options: AnchorBundleOptions) -> Result<()> {
    let (entries, delivered, pending) =
        collect_anchor_entries(&options.spool_dir, options.copy_dir.as_deref())?;

    let mut summary = AnchorBundleSummary {
        generated_unix_ms: unix_ms_now(),
        spool_dir: display_path(&options.spool_dir),
        total_entries: entries.len(),
        delivered,
        pending,
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
) -> Result<(Vec<AnchorEntry>, usize, usize)> {
    let mut entries: BTreeMap<String, AnchorPaths> = BTreeMap::new();

    let dir_iter = match fs::read_dir(spool_dir) {
        Ok(iter) => iter,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok((Vec::new(), 0, 0)),
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
        if let Some(base) = name
            .strip_prefix(ANCHOR_REQUEST_PREFIX)
            .and_then(|rest| rest.strip_suffix(ANCHOR_REQUEST_SUFFIX))
        {
            entries.entry(base.to_string()).or_default().anchor_request = Some(entry.path());
            continue;
        }
        if let Some(base) = name
            .strip_prefix(SENTINEL_PREFIX)
            .and_then(|rest| rest.strip_suffix(SENTINEL_SUFFIX))
        {
            entries.entry(base.to_string()).or_default().sentinel = Some(entry.path());
            continue;
        }
        if let Some(base) = name
            .strip_prefix(ENVELOPE_PREFIX)
            .and_then(|rest| rest.strip_suffix(ENVELOPE_SUFFIX))
        {
            entries.entry(base.to_string()).or_default().envelope = Some(entry.path());
            continue;
        }
        if let Some(base) = name
            .strip_prefix(INDEXES_PREFIX)
            .and_then(|rest| rest.strip_suffix(INDEXES_SUFFIX))
        {
            entries.entry(base.to_string()).or_default().indexes = Some(entry.path());
            continue;
        }
        if let Some(base) = name
            .strip_prefix(SSM_PREFIX)
            .and_then(|rest| rest.strip_suffix(SSM_SUFFIX))
        {
            entries.entry(base.to_string()).or_default().ssm = Some(entry.path());
            continue;
        }
        if let Some(base) = name
            .strip_prefix(TRM_PREFIX)
            .and_then(|rest| rest.strip_suffix(TRM_SUFFIX))
        {
            entries.entry(base.to_string()).or_default().trm = Some(entry.path());
            continue;
        }
        if let Some(base) = name
            .strip_prefix(TRM_STATE_PREFIX)
            .and_then(|rest| rest.strip_suffix(TRM_STATE_SUFFIX))
        {
            entries.entry(base.to_string()).or_default().trm_state = Some(entry.path());
            continue;
        }
        if let Some(base) = name
            .strip_prefix(LINEAGE_PREFIX)
            .and_then(|rest| rest.strip_suffix(LINEAGE_SUFFIX))
        {
            entries.entry(base.to_string()).or_default().lineage = Some(entry.path());
            continue;
        }
    }

    let mut summaries = Vec::with_capacity(entries.len());
    for (base_id, paths) in entries {
        let anchor_request = digest_optional_file(&paths.anchor_request, copy_dir, &base_id)?;
        let sentinel = digest_sentinel(&paths.sentinel, copy_dir, &base_id)?;
        let envelope = digest_optional_file(&paths.envelope, copy_dir, &base_id)?;
        let indexes = digest_optional_file(&paths.indexes, copy_dir, &base_id)?;
        let ssm = digest_optional_file(&paths.ssm, copy_dir, &base_id)?;
        let trm = digest_optional_file(&paths.trm, copy_dir, &base_id)?;
        let trm_state = digest_optional_file(&paths.trm_state, copy_dir, &base_id)?;
        let lineage = digest_optional_file(&paths.lineage, copy_dir, &base_id)?;

        let status = if sentinel.is_some() {
            AnchorStatus::Delivered
        } else {
            AnchorStatus::Pending
        };

        summaries.push(AnchorEntry {
            base_id,
            status: status.label().to_string(),
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

    let delivered = summaries
        .iter()
        .filter(|entry| entry.status == AnchorStatus::Delivered.label())
        .count();
    let pending = summaries.len().saturating_sub(delivered);
    Ok((summaries, delivered, pending))
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
    let bytes =
        fs::read(path).with_context(|| format!("failed to read sentinel {}", path.display()))?;
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
    let signature = Signature::new(key_pair.private_key(), payload);
    let (algorithm, public) = key_pair.public_key().to_bytes();
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
    use iroha_crypto::Algorithm;
    use norito::json::Value;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn collects_pending_and_delivered_entries() -> Result<()> {
        let dir = tempdir()?;
        let spool = dir.path().join("taikai");
        fs::create_dir_all(&spool)?;

        let delivered = "0001-0002-deadbeef";
        fs::write(
            spool.join(format!(
                "{ANCHOR_REQUEST_PREFIX}{delivered}{ANCHOR_REQUEST_SUFFIX}"
            )),
            b"{\"payload\":\"delivered\"}",
        )?;
        fs::write(
            spool.join(format!("{SENTINEL_PREFIX}{delivered}{SENTINEL_SUFFIX}")),
            b"1736111111",
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

        let pending = "0001-0003-baadf00d";
        fs::write(
            spool.join(format!(
                "{ANCHOR_REQUEST_PREFIX}{pending}{ANCHOR_REQUEST_SUFFIX}"
            )),
            b"{\"payload\":\"pending\"}",
        )?;

        let copy_dir = dir.path().join("bundle");
        let (entries, delivered_count, pending_count) =
            collect_anchor_entries(&spool, Some(copy_dir.as_path()))?;

        assert_eq!(entries.len(), 2);
        assert_eq!(delivered_count, 1);
        assert_eq!(pending_count, 1);

        let delivered_entry = entries
            .iter()
            .find(|entry| entry.base_id == delivered)
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
            .find(|entry| entry.base_id == pending)
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
    fn bundle_signs_summary_when_key_provided() -> Result<()> {
        let dir = tempdir()?;
        let spool = dir.path().join("taikai");
        fs::create_dir_all(&spool)?;
        let base = "0001-0004-feedfeed";
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
            output: JsonTarget::File(out_path.clone()),
        })?;

        let value: Value = norito::json::from_reader(
            std::fs::File::open(&out_path).expect("bundle output readable"),
        )?;
        assert!(value.get("signing").is_some(), "signing block missing");
        Ok(())
    }
}
