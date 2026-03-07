use std::{
    convert::TryFrom,
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use eyre::{Context as _, Result, ensure, eyre};
use iroha_crypto::PublicKey;
use iroha_data_model::{
    account::{AccountController, AccountId},
    metadata::Metadata,
    offline::{OfflinePosProvisionManifest, OfflineVerdictRevocationBundle},
};
use norito::{
    derive::JsonSerialize,
    json::{self, Value},
};
use serde::{Deserialize, Serialize};

pub struct OfflinePosVerifyOptions {
    pub bundle_json: Option<PathBuf>,
    pub manifest_json: Option<PathBuf>,
    pub allowance_summaries: Vec<PathBuf>,
    pub policy_path: Option<PathBuf>,
    pub audit_log_path: Option<PathBuf>,
    pub api_snapshot_path: Option<PathBuf>,
    pub now_ms_override: Option<u64>,
}

pub fn run(options: OfflinePosVerifyOptions) -> Result<()> {
    if options.bundle_json.is_none()
        && options.manifest_json.is_none()
        && options.allowance_summaries.is_empty()
    {
        return Err(eyre!(
            "offline-pos-verify requires at least one of --bundle, --manifest, or --allowance-summary"
        ));
    }

    if let Some(bundle_path) = &options.bundle_json {
        verify_revocation_bundle(bundle_path, options.api_snapshot_path.as_deref())?;
    } else if options.api_snapshot_path.is_some() {
        return Err(eyre!("--api-snapshot requires --bundle"));
    }

    let policy = load_policy(options.policy_path.as_deref())?;
    let now_ms = options
        .now_ms_override
        .map(Ok)
        .unwrap_or_else(current_unix_time_ms)?;
    let mut audit_logger = match &options.audit_log_path {
        Some(path) => Some(AuditLogger::new(path)?),
        None => None,
    };

    if let Some(manifest_path) = &options.manifest_json {
        let manifest = read_manifest(manifest_path)?;
        verify_manifest_signature(&manifest)?;
        enforce_manifest_policy(&manifest, &policy, now_ms)?;
        println!(
            "Verified POS manifest `{}` (seq={}) signed by {}",
            manifest.manifest_id, manifest.sequence, manifest.operator
        );
        if let Some(logger) = audit_logger.as_mut() {
            logger.log_manifest(&manifest, now_ms)?;
        }
    }

    for summary_path in &options.allowance_summaries {
        let summary = read_allowance_summary(summary_path)?;
        enforce_allowance_policy(summary_path, &summary, &policy, now_ms)?;
        println!(
            "Allowance `{}` (certificate {}) passed TTL/nonce policy",
            summary.label.as_deref().unwrap_or("[unnamed allowance]"),
            summary.certificate_id_hex
        );
        if let Some(logger) = audit_logger.as_mut() {
            logger.log_allowance(&summary, now_ms)?;
        }
    }

    if let Some(logger) = audit_logger.as_mut() {
        logger.flush()?;
    }

    Ok(())
}

fn verify_revocation_bundle(bundle_path: &Path, snapshot_path: Option<&Path>) -> Result<()> {
    let bundle_bytes = fs::read(bundle_path)
        .wrap_err_with(|| format!("failed to read revocation bundle {}", bundle_path.display()))?;
    let bundle: OfflineVerdictRevocationBundle =
        json::from_slice(&bundle_bytes).wrap_err("failed to parse revocation bundle JSON")?;

    verify_bundle_signature(&bundle)?;
    println!(
        "Verified revocation bundle `{}` (seq={}, total={}) signed by {}",
        bundle.bundle_id,
        bundle.sequence,
        bundle.revocations.len(),
        bundle.operator
    );

    if let Some(path) = snapshot_path {
        let snapshot = RevocationApiSnapshot::from_bundle(&bundle)?;
        let serialized = json::to_json_pretty(&snapshot)?;
        fs::write(path, serialized).wrap_err_with(|| {
            format!("failed to write revocation API snapshot {}", path.display())
        })?;
        println!(
            "Wrote Torii-style revocation snapshot to {}",
            path.display()
        );
    }

    Ok(())
}

fn read_manifest(path: &Path) -> Result<OfflinePosProvisionManifest> {
    let bytes =
        fs::read(path).wrap_err_with(|| format!("failed to read manifest {}", path.display()))?;
    json::from_slice(&bytes)
        .wrap_err_with(|| format!("failed to parse manifest {}", path.display()))
}

fn read_allowance_summary(path: &Path) -> Result<AllowanceSummary> {
    let bytes = fs::read(path)
        .wrap_err_with(|| format!("failed to read allowance summary {}", path.display()))?;
    serde_json::from_slice(&bytes)
        .wrap_err_with(|| format!("failed to parse allowance summary {}", path.display()))
}

fn verify_manifest_signature(manifest: &OfflinePosProvisionManifest) -> Result<()> {
    let payload = manifest
        .operator_signing_bytes()
        .wrap_err("failed to serialize manifest payload")?;
    let controller: &AccountController = manifest.operator.controller();
    let operator_key: &PublicKey = controller.expect_single_signatory();
    manifest
        .operator_signature
        .verify(operator_key, &payload)
        .wrap_err("manifest signature verification failed")
}

fn verify_bundle_signature(bundle: &OfflineVerdictRevocationBundle) -> Result<()> {
    let payload = bundle
        .operator_signing_bytes()
        .wrap_err("failed to serialize revocation payload")?;
    let controller: &AccountController = bundle.operator.controller();
    let operator_key: &PublicKey = controller.expect_single_signatory();
    bundle
        .operator_signature
        .verify(operator_key, &payload)
        .wrap_err("revocation bundle signature verification failed")
}

fn enforce_manifest_policy(
    manifest: &OfflinePosProvisionManifest,
    policy: &PolicyConfig,
    now_ms: u64,
) -> Result<()> {
    if !policy.allowed_manifest_ids.is_empty()
        && !policy
            .allowed_manifest_ids
            .iter()
            .any(|allowed| allowed == &manifest.manifest_id)
    {
        return Err(eyre!(
            "manifest `{}` is not permitted by policy",
            manifest.manifest_id
        ));
    }

    if !policy.allowed_operators.is_empty()
        && !policy
            .allowed_operators
            .iter()
            .any(|allowed| allowed == &manifest.operator)
    {
        return Err(eyre!(
            "operator `{}` is not permitted by policy",
            manifest.operator
        ));
    }

    ensure!(
        within_window(
            manifest.valid_from_ms,
            manifest.valid_until_ms,
            now_ms,
            policy.manifest_grace_ms
        ),
        "manifest `{}` outside validity window (valid_from={}, valid_until={}, now={}, grace_ms={})",
        manifest.manifest_id,
        manifest.valid_from_ms,
        manifest.valid_until_ms,
        now_ms,
        policy.manifest_grace_ms
    );

    enforce_pinned_roots(manifest, policy, now_ms)?;
    Ok(())
}

fn enforce_pinned_roots(
    manifest: &OfflinePosProvisionManifest,
    policy: &PolicyConfig,
    now_ms: u64,
) -> Result<()> {
    for pinned in &policy.pinned_backend_roots {
        let Some(root) = manifest
            .backend_roots
            .iter()
            .find(|candidate| candidate.label == pinned.label)
        else {
            return Err(eyre!(
                "manifest `{}` missing pinned backend `{}`",
                manifest.manifest_id,
                pinned.label
            ));
        };

        if let Some(expected_role) = pinned.role.as_deref() {
            ensure!(
                root.role == expected_role,
                "backend `{}` uses role `{}` but policy requires `{}`",
                pinned.label,
                root.role,
                expected_role
            );
        }

        ensure!(
            root.public_key == pinned.public_key,
            "backend `{}` public key mismatch",
            pinned.label
        );
        ensure!(
            within_window(
                root.valid_from_ms,
                root.valid_until_ms,
                now_ms,
                policy.manifest_grace_ms
            ),
            "backend `{}` validity window exhausted (valid_until={}, now={}, grace_ms={})",
            pinned.label,
            root.valid_until_ms,
            now_ms,
            policy.manifest_grace_ms
        );
    }

    Ok(())
}

fn enforce_allowance_policy(
    path: &Path,
    summary: &AllowanceSummary,
    policy: &PolicyConfig,
    now_ms: u64,
) -> Result<()> {
    if policy.require_refresh_deadline {
        let Some(deadline) = summary.refresh_at_ms else {
            return Err(eyre!(
                "{} missing refresh_at_ms while policy requires TTLs",
                path.display()
            ));
        };
        ensure!(
            now_ms <= deadline.saturating_add(policy.verdict_grace_ms),
            "{} refresh_at_ms ({deadline}) expired (now={}, grace_ms={})",
            path.display(),
            now_ms,
            policy.verdict_grace_ms
        );
    } else if let Some(deadline) = summary.refresh_at_ms {
        ensure!(
            now_ms <= deadline.saturating_add(policy.verdict_grace_ms),
            "{} refresh_at_ms ({deadline}) expired (now={}, grace_ms={})",
            path.display(),
            now_ms,
            policy.verdict_grace_ms
        );
    }

    if policy.require_verdict_id {
        ensure!(
            summary
                .verdict_id_hex
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty()),
            "{} missing verdict_id_hex while policy requires one",
            path.display()
        );
    }

    if policy.require_attestation_nonce {
        ensure!(
            summary
                .attestation_nonce_hex
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty()),
            "{} missing attestation_nonce_hex while policy requires one",
            path.display()
        );
    }

    Ok(())
}

fn load_policy(path: Option<&Path>) -> Result<PolicyConfig> {
    let Some(path) = path else {
        return Ok(PolicyConfig::default());
    };
    let bytes =
        fs::read(path).wrap_err_with(|| format!("failed to read policy {}", path.display()))?;
    let raw: RawPolicy = serde_json::from_slice(&bytes)
        .wrap_err_with(|| format!("failed to parse policy {}", path.display()))?;
    PolicyConfig::try_from(raw).wrap_err_with(|| format!("invalid policy {}", path.display()))
}

fn current_unix_time_ms() -> Result<u64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .wrap_err("system clock drifted before UNIX_EPOCH")?;
    Ok(duration.as_millis() as u64)
}

fn within_window(start_ms: u64, end_ms: u64, now_ms: u64, grace_ms: u64) -> bool {
    if now_ms + grace_ms < start_ms {
        return false;
    }
    now_ms <= end_ms.saturating_add(grace_ms)
}

fn metadata_to_value(metadata: &Metadata) -> Option<Value> {
    if metadata.is_empty() {
        return None;
    }
    json::to_value(metadata).ok()
}

#[derive(Debug, Deserialize)]
struct AllowanceSummary {
    label: Option<String>,
    certificate_id_hex: String,
    refresh_at_ms: Option<u64>,
    verdict_id_hex: Option<String>,
    attestation_nonce_hex: Option<String>,
}

#[derive(Debug, Default)]
struct PolicyConfig {
    allowed_operators: Vec<AccountId>,
    allowed_manifest_ids: Vec<String>,
    pinned_backend_roots: Vec<PinnedRoot>,
    manifest_grace_ms: u64,
    verdict_grace_ms: u64,
    require_refresh_deadline: bool,
    require_verdict_id: bool,
    require_attestation_nonce: bool,
}

#[derive(Debug, Clone)]
struct PinnedRoot {
    label: String,
    role: Option<String>,
    public_key: PublicKey,
}

#[derive(Debug, Deserialize, Default)]
struct RawPolicy {
    #[serde(default)]
    allowed_operators: Vec<String>,
    #[serde(default)]
    allowed_manifest_ids: Vec<String>,
    #[serde(default)]
    pinned_backend_roots: Vec<RawPinnedRoot>,
    #[serde(default)]
    manifest_grace_ms: u64,
    #[serde(default)]
    verdict_grace_ms: u64,
    #[serde(default)]
    require_refresh_deadline: bool,
    #[serde(default)]
    require_verdict_id: bool,
    #[serde(default)]
    require_attestation_nonce: bool,
}

#[derive(Debug, Deserialize)]
struct RawPinnedRoot {
    label: String,
    #[serde(default)]
    role: Option<String>,
    public_key: String,
}

impl TryFrom<RawPolicy> for PolicyConfig {
    type Error = eyre::Report;

    fn try_from(raw: RawPolicy) -> Result<Self> {
        let mut allowed_operators = Vec::with_capacity(raw.allowed_operators.len());
        for literal in raw.allowed_operators {
            let id = AccountId::parse_encoded(&literal)
                .map(|parsed| parsed.into_account_id())
                .map_err(|err| eyre!("invalid policy operator `{literal}`: {err}"))?;
            allowed_operators.push(id);
        }

        let mut pinned = Vec::with_capacity(raw.pinned_backend_roots.len());
        for root in raw.pinned_backend_roots {
            pinned.push(PinnedRoot::try_from(root)?);
        }

        Ok(Self {
            allowed_operators,
            allowed_manifest_ids: raw.allowed_manifest_ids,
            pinned_backend_roots: pinned,
            manifest_grace_ms: raw.manifest_grace_ms,
            verdict_grace_ms: raw.verdict_grace_ms,
            require_refresh_deadline: raw.require_refresh_deadline,
            require_verdict_id: raw.require_verdict_id,
            require_attestation_nonce: raw.require_attestation_nonce,
        })
    }
}

impl TryFrom<RawPinnedRoot> for PinnedRoot {
    type Error = eyre::Report;

    fn try_from(raw: RawPinnedRoot) -> Result<Self> {
        let public_key = PublicKey::from_str(&raw.public_key)
            .map_err(|err| eyre!("invalid public key `{}`: {err}", raw.public_key))?;
        Ok(Self {
            label: raw.label,
            role: raw.role,
            public_key,
        })
    }
}

struct AuditLogger {
    path: PathBuf,
    writer: BufWriter<std::fs::File>,
}

impl AuditLogger {
    fn new(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).wrap_err_with(|| {
                format!("failed to create audit directory {}", parent.display())
            })?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .wrap_err_with(|| format!("failed to open audit log {}", path.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            writer: BufWriter::new(file),
        })
    }

    fn log_manifest(
        &mut self,
        manifest: &OfflinePosProvisionManifest,
        checked_at_ms: u64,
    ) -> Result<()> {
        let entry = AuditEntry::Manifest {
            checked_at_ms,
            manifest_id: manifest.manifest_id.clone(),
            sequence: manifest.sequence,
            operator: manifest.operator.to_string(),
            valid_from_ms: manifest.valid_from_ms,
            valid_until_ms: manifest.valid_until_ms,
            status: "accepted",
        };
        self.log(&entry)
    }

    fn log_allowance(&mut self, summary: &AllowanceSummary, checked_at_ms: u64) -> Result<()> {
        let entry = AuditEntry::Allowance {
            checked_at_ms,
            certificate_id_hex: summary.certificate_id_hex.clone(),
            label: summary.label.clone(),
            refresh_at_ms: summary.refresh_at_ms,
            status: "accepted",
        };
        self.log(&entry)
    }

    fn log(&mut self, entry: &AuditEntry) -> Result<()> {
        let line = serde_json::to_string(entry)?;
        writeln!(self.writer, "{line}")
            .wrap_err_with(|| format!("failed to write audit log {}", self.path.display()))
    }

    fn flush(&mut self) -> Result<()> {
        self.writer
            .flush()
            .wrap_err_with(|| format!("failed to flush audit log {}", self.path.display()))
    }
}

#[derive(Serialize)]
#[serde(tag = "event", content = "payload", rename_all = "snake_case")]
enum AuditEntry {
    Manifest {
        checked_at_ms: u64,
        manifest_id: String,
        sequence: u32,
        operator: String,
        valid_from_ms: u64,
        valid_until_ms: u64,
        status: &'static str,
    },
    Allowance {
        checked_at_ms: u64,
        certificate_id_hex: String,
        label: Option<String>,
        refresh_at_ms: Option<u64>,
        status: &'static str,
    },
}

#[derive(JsonSerialize)]
struct RevocationApiSnapshot {
    total: usize,
    items: Vec<RevocationApiItem>,
}

#[derive(JsonSerialize)]
struct RevocationApiItem {
    verdict_id_hex: String,
    issuer_id: String,
    issuer_display: String,
    revoked_at_ms: u64,
    reason: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    metadata: Option<Value>,
    record: Value,
}

impl RevocationApiSnapshot {
    fn from_bundle(bundle: &OfflineVerdictRevocationBundle) -> Result<Self> {
        let mut items = Vec::with_capacity(bundle.revocations.len());
        for record in &bundle.revocations {
            let metadata_value = metadata_to_value(&record.metadata);
            let item = RevocationApiItem {
                verdict_id_hex: hex::encode(record.verdict_id.as_ref()),
                issuer_id: record.issuer.to_string(),
                issuer_display: record.issuer.to_string(),
                revoked_at_ms: record.revoked_at_ms,
                reason: record.reason.as_str().to_string(),
                note: record.note.clone(),
                metadata: metadata_value,
                record: json::to_value(record).wrap_err("failed to serialize revocation record")?,
            };
            items.push(item);
        }
        Ok(Self {
            total: items.len(),
            items,
        })
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{account::AccountId, domain::DomainId, offline::OfflinePosBackendRoot};

    use super::*;

    fn operator_pair() -> KeyPair {
        KeyPair::from_seed(b"pos-operator".to_vec(), Algorithm::Ed25519)
    }

    fn backend_pair() -> KeyPair {
        KeyPair::from_seed(b"backend-root".to_vec(), Algorithm::Ed25519)
    }

    fn sample_domain() -> DomainId {
        DomainId::from_str("wonderland").expect("domain parses")
    }

    fn signed_manifest(
        valid_from: u64,
        valid_until: u64,
        backend: &KeyPair,
    ) -> OfflinePosProvisionManifest {
        let operator = operator_pair();
        let operator_id = AccountId::new(sample_domain(), operator.public_key().clone());
        let mut manifest = OfflinePosProvisionManifest {
            manifest_id: "pos-demo".into(),
            sequence: 1,
            published_at_ms: valid_from,
            valid_from_ms: valid_from,
            valid_until_ms: valid_until,
            rotation_hint_ms: None,
            operator: operator_id,
            backend_roots: vec![OfflinePosBackendRoot {
                label: "torii-admission".into(),
                role: "offline_admission_signer".into(),
                public_key: backend.public_key().clone(),
                valid_from_ms: valid_from,
                valid_until_ms: valid_until,
                metadata: Metadata::default(),
            }],
            metadata: Metadata::default(),
            operator_signature: Signature::from_bytes(&[0; 64]),
        };
        let payload = manifest.operator_signing_bytes().expect("manifest payload");
        manifest.operator_signature = Signature::new(operator.private_key(), &payload);
        manifest
    }

    #[test]
    fn enforce_manifest_policy_accepts_pinned_root() {
        let backend_key = backend_pair();
        let manifest = signed_manifest(10, 20, &backend_key);
        let policy = PolicyConfig {
            allowed_operators: vec![manifest.operator.clone()],
            allowed_manifest_ids: vec![manifest.manifest_id.clone()],
            pinned_backend_roots: vec![PinnedRoot {
                label: "torii-admission".into(),
                role: Some("offline_admission_signer".into()),
                public_key: backend_key.public_key().clone(),
            }],
            manifest_grace_ms: 5,
            verdict_grace_ms: 0,
            require_refresh_deadline: false,
            require_verdict_id: false,
            require_attestation_nonce: false,
        };

        assert!(enforce_manifest_policy(&manifest, &policy, 15).is_ok());
    }

    #[test]
    fn enforce_allowance_policy_checks_ttl() {
        let summary = AllowanceSummary {
            label: Some("demo".into()),
            certificate_id_hex: "deadbeef".into(),
            refresh_at_ms: Some(50),
            verdict_id_hex: Some("abcd".into()),
            attestation_nonce_hex: Some("ffff".into()),
        };
        let policy = PolicyConfig {
            allowed_operators: Vec::new(),
            allowed_manifest_ids: Vec::new(),
            pinned_backend_roots: Vec::new(),
            manifest_grace_ms: 0,
            verdict_grace_ms: 5,
            require_refresh_deadline: true,
            require_verdict_id: true,
            require_attestation_nonce: true,
        };
        let path = Path::new("fixtures/demo.json");
        enforce_allowance_policy(path, &summary, &policy, 40).expect("summary should pass policy");
        let err = enforce_allowance_policy(path, &summary, &policy, 61)
            .expect_err("ttl should be enforced");
        assert!(
            err.to_string().contains("refresh_at_ms"),
            "unexpected error message: {err}"
        );
    }
}
