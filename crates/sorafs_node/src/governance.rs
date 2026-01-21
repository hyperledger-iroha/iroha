use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use hex::ToHex;
use norito::json::{self, Map as JsonMap, Value as JsonValue};
use sorafs_manifest::{
    SorafsReconciliationReportV1,
    deal::{DealSettlementStatusV1, DealSettlementV1},
    repair::{GcAuditEventV1, RepairAuditEventV1, RepairSlashProposalV1, RepairTaskStatusV1},
};

use crate::{GovernancePublishError, GovernancePublisher, RepairSlashStage};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Persists governance artefacts on the filesystem for downstream ingestion.
#[derive(Debug)]
pub struct FilesystemGovernancePublisher {
    root: PathBuf,
}

impl FilesystemGovernancePublisher {
    /// Construct a new publisher rooted at the supplied directory.
    pub fn try_new(root: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn settlements_root(&self) -> PathBuf {
        self.root.join("settlements")
    }

    fn repairs_root(&self) -> PathBuf {
        self.root.join("repairs")
    }

    fn repair_audit_root(&self) -> PathBuf {
        self.repairs_root().join("audit")
    }

    fn repair_slash_root(&self) -> PathBuf {
        self.repairs_root().join("slash")
    }

    fn gc_audit_root(&self) -> PathBuf {
        self.root.join("gc").join("audit")
    }

    fn reconciliation_root(&self) -> PathBuf {
        self.root.join("reconciliation")
    }

    fn base_path(&self, settlement: &DealSettlementV1, digest_hex: &str) -> PathBuf {
        let deal_hex = settlement.deal_id.encode_hex::<String>();
        let status = status_label(settlement.status);
        let digest_prefix = &digest_hex[..16];
        let base = format!("{:020}_{}_{}", settlement.settled_at, status, digest_prefix);
        self.settlements_root().join(deal_hex).join(base)
    }

    fn repair_audit_path(&self, event: &RepairAuditEventV1, digest_hex: &str) -> PathBuf {
        let sequence = format!("{:020}", event.header.sequence);
        let status = repair_status_label(event.payload.status);
        let ticket = sanitize_label(event.payload.ticket_id.0.as_str());
        let digest_prefix = &digest_hex[..16];
        let base = format!("{sequence}_{status}_{ticket}_{digest_prefix}");
        self.repair_audit_root().join(base)
    }

    fn repair_slash_path(
        &self,
        proposal: &RepairSlashProposalV1,
        stage: RepairSlashStage,
        digest_hex: &str,
    ) -> PathBuf {
        let submitted = format!("{:020}", proposal.submitted_at_unix);
        let ticket = sanitize_label(proposal.ticket_id.0.as_str());
        let stage_label = stage.as_str();
        let digest_prefix = &digest_hex[..16];
        let base = format!("{submitted}_{stage_label}_{ticket}_{digest_prefix}");
        self.repair_slash_root().join(base)
    }

    fn gc_audit_path(&self, event: &GcAuditEventV1, digest_hex: &str) -> PathBuf {
        let sequence = format!("{:020}", event.header.sequence);
        let reason = sanitize_label(event.payload.reason.as_str());
        let manifest_hex = hex::encode(event.payload.manifest_digest);
        let digest_prefix = &digest_hex[..16];
        let base = format!("{sequence}_{reason}_{manifest_hex}_{digest_prefix}");
        self.gc_audit_root().join(base)
    }

    fn reconciliation_path(
        &self,
        report: &SorafsReconciliationReportV1,
        digest_hex: &str,
    ) -> PathBuf {
        let provider_hex = hex::encode(report.provider_id);
        let provider_prefix = &provider_hex[..16];
        let digest_prefix = &digest_hex[..16];
        let base = format!(
            "{:020}_{}_{}",
            report.generated_at_unix, provider_prefix, digest_prefix
        );
        self.reconciliation_root().join(base)
    }
}

fn status_label(status: DealSettlementStatusV1) -> &'static str {
    match status {
        DealSettlementStatusV1::Completed => "completed",
        DealSettlementStatusV1::Cancelled => "cancelled",
        DealSettlementStatusV1::Slashed => "slashed",
    }
}

fn repair_status_label(status: RepairTaskStatusV1) -> &'static str {
    match status {
        RepairTaskStatusV1::Queued => "queued",
        RepairTaskStatusV1::InProgress => "in_progress",
        RepairTaskStatusV1::Verifying => "verifying",
        RepairTaskStatusV1::Completed => "completed",
        RepairTaskStatusV1::Failed => "failed",
        RepairTaskStatusV1::Escalated => "escalated",
    }
}

fn sanitize_label(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out
}

fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let counter = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let tmp_path = temp_path_for_atomic(path, pid, counter);
    {
        let mut file = File::create(&tmp_path)?;
        file.write_all(data)?;
        file.sync_all()?;
    }
    fs::rename(tmp_path, path)?;
    Ok(())
}

fn write_digest_sidecar(path: &Path, data: &[u8]) -> io::Result<()> {
    let digest = blake3::hash(data);
    let hex = digest.to_hex().to_string();
    let suffix = match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if !ext.is_empty() => format!("{ext}.blake3"),
        _ => "blake3".to_string(),
    };
    let digest_path = path.with_extension(suffix);
    let mut body = hex;
    body.push('\n');
    write_atomic(&digest_path, body.as_bytes())
}

fn temp_path_for_atomic(path: &Path, pid: u32, counter: u64) -> PathBuf {
    let suffix = format!("tmp-{pid}-{counter}");
    let candidate = path.with_added_extension(&suffix);
    match candidate.file_name().and_then(|name| name.to_str()) {
        Some(name) => candidate.with_file_name(format!(".{name}")),
        None => candidate,
    }
}

impl GovernancePublisher for FilesystemGovernancePublisher {
    fn publish_deal_settlement(
        &self,
        settlement: &DealSettlementV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let digest = blake3::hash(encoded);
        let digest_hex = digest.to_hex().to_string();
        let base_path = self.base_path(settlement, &digest_hex);

        let encoded_path = base_path.with_extension("to");
        write_atomic(&encoded_path, encoded)?;
        write_digest_sidecar(&encoded_path, encoded)?;

        let mut settlement_obj = JsonMap::new();
        settlement_obj.insert("version".into(), JsonValue::from(settlement.version as u64));
        settlement_obj.insert(
            "deal_id".into(),
            JsonValue::from(settlement.deal_id.encode_hex::<String>()),
        );
        settlement_obj.insert(
            "provider_id".into(),
            JsonValue::from(settlement.ledger.provider_id.encode_hex::<String>()),
        );
        settlement_obj.insert(
            "client_id".into(),
            JsonValue::from(settlement.ledger.client_id.encode_hex::<String>()),
        );
        settlement_obj.insert(
            "status".into(),
            JsonValue::from(status_label(settlement.status)),
        );
        settlement_obj.insert("settled_at".into(), JsonValue::from(settlement.settled_at));
        settlement_obj.insert(
            "ledger_captured_at".into(),
            JsonValue::from(settlement.ledger.captured_at),
        );
        settlement_obj.insert(
            "provider_accrual_micro".into(),
            JsonValue::from(settlement.ledger.provider_accrual.as_micro().to_string()),
        );
        settlement_obj.insert(
            "client_liability_micro".into(),
            JsonValue::from(settlement.ledger.client_liability.as_micro().to_string()),
        );
        settlement_obj.insert(
            "bond_locked_micro".into(),
            JsonValue::from(settlement.ledger.bond_locked.as_micro().to_string()),
        );
        settlement_obj.insert(
            "bond_slashed_micro".into(),
            JsonValue::from(settlement.ledger.bond_slashed.as_micro().to_string()),
        );
        if let Some(notes) = &settlement.audit_notes {
            settlement_obj.insert("audit_notes".into(), JsonValue::from(notes.clone()));
        }

        let mut payload = JsonMap::new();
        payload.insert("settlement".into(), JsonValue::Object(settlement_obj));

        let mut metadata = JsonMap::new();
        metadata.insert(
            "status".into(),
            JsonValue::from(status_label(settlement.status)),
        );
        metadata.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
        metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
        metadata.insert(
            "encoded_base64".into(),
            JsonValue::from(BASE64_STANDARD.encode(encoded)),
        );
        payload.insert("metadata".into(), JsonValue::Object(metadata));

        let json_body = json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
            GovernancePublishError::other(format!("serialize settlement json: {err}"))
        })?;

        let json_path = base_path.with_extension("json");
        write_atomic(&json_path, json_body.as_bytes())?;
        write_digest_sidecar(&json_path, json_body.as_bytes())?;

        Ok(())
    }

    fn publish_repair_audit_event(
        &self,
        event: &RepairAuditEventV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let digest = blake3::hash(encoded);
        let digest_hex = digest.to_hex().to_string();
        let base_path = self.repair_audit_path(event, &digest_hex);

        let encoded_path = base_path.with_extension("to");
        write_atomic(&encoded_path, encoded)?;
        write_digest_sidecar(&encoded_path, encoded)?;

        let mut payload = JsonMap::new();
        payload.insert(
            "event".into(),
            json::to_value(event).map_err(|err| {
                GovernancePublishError::other(format!("serialize audit event: {err}"))
            })?,
        );

        let mut metadata = JsonMap::new();
        metadata.insert(
            "ticket_id".into(),
            JsonValue::from(event.payload.ticket_id.0.clone()),
        );
        metadata.insert(
            "manifest".into(),
            JsonValue::from(hex::encode(event.payload.manifest_digest)),
        );
        metadata.insert(
            "provider".into(),
            JsonValue::from(hex::encode(event.payload.provider_id)),
        );
        metadata.insert(
            "status".into(),
            JsonValue::from(repair_status_label(event.payload.status)),
        );
        metadata.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
        metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
        metadata.insert(
            "encoded_base64".into(),
            JsonValue::from(BASE64_STANDARD.encode(encoded)),
        );
        payload.insert("metadata".into(), JsonValue::Object(metadata));

        let json_body = json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
            GovernancePublishError::other(format!("serialize repair audit json: {err}"))
        })?;

        let json_path = base_path.with_extension("json");
        write_atomic(&json_path, json_body.as_bytes())?;
        write_digest_sidecar(&json_path, json_body.as_bytes())?;

        Ok(())
    }

    fn publish_repair_slash_proposal(
        &self,
        proposal: &RepairSlashProposalV1,
        encoded: &[u8],
        stage: RepairSlashStage,
    ) -> Result<(), GovernancePublishError> {
        let digest = blake3::hash(encoded);
        let digest_hex = digest.to_hex().to_string();
        let base_path = self.repair_slash_path(proposal, stage, &digest_hex);

        let encoded_path = base_path.with_extension("to");
        write_atomic(&encoded_path, encoded)?;
        write_digest_sidecar(&encoded_path, encoded)?;

        let mut payload = JsonMap::new();
        payload.insert(
            "proposal".into(),
            json::to_value(proposal).map_err(|err| {
                GovernancePublishError::other(format!("serialize slash proposal: {err}"))
            })?,
        );

        let mut metadata = JsonMap::new();
        metadata.insert(
            "ticket_id".into(),
            JsonValue::from(proposal.ticket_id.0.clone()),
        );
        metadata.insert(
            "manifest".into(),
            JsonValue::from(hex::encode(proposal.manifest_digest)),
        );
        metadata.insert(
            "provider".into(),
            JsonValue::from(hex::encode(proposal.provider_id)),
        );
        metadata.insert("stage".into(), JsonValue::from(stage.as_str()));
        metadata.insert("outcome".into(), JsonValue::from(stage.as_str()));
        metadata.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
        metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
        metadata.insert(
            "encoded_base64".into(),
            JsonValue::from(BASE64_STANDARD.encode(encoded)),
        );
        payload.insert("metadata".into(), JsonValue::Object(metadata));

        let json_body = json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
            GovernancePublishError::other(format!("serialize slash proposal json: {err}"))
        })?;

        let json_path = base_path.with_extension("json");
        write_atomic(&json_path, json_body.as_bytes())?;
        write_digest_sidecar(&json_path, json_body.as_bytes())?;

        Ok(())
    }

    fn publish_gc_audit_event(
        &self,
        event: &GcAuditEventV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let digest = blake3::hash(encoded);
        let digest_hex = digest.to_hex().to_string();
        let base_path = self.gc_audit_path(event, &digest_hex);

        let encoded_path = base_path.with_extension("to");
        write_atomic(&encoded_path, encoded)?;
        write_digest_sidecar(&encoded_path, encoded)?;

        let mut payload = JsonMap::new();
        payload.insert(
            "event".into(),
            json::to_value(event).map_err(|err| {
                GovernancePublishError::other(format!("serialize gc event: {err}"))
            })?,
        );

        let mut metadata = JsonMap::new();
        metadata.insert(
            "reason".into(),
            JsonValue::from(event.payload.reason.clone()),
        );
        if let Some(blocked) = &event.payload.blocked_reason {
            metadata.insert("blocked_reason".into(), JsonValue::from(blocked.clone()));
        }
        metadata.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
        metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
        metadata.insert(
            "encoded_base64".into(),
            JsonValue::from(BASE64_STANDARD.encode(encoded)),
        );
        payload.insert("metadata".into(), JsonValue::Object(metadata));

        let json_body = json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
            GovernancePublishError::other(format!("serialize gc audit json: {err}"))
        })?;

        let json_path = base_path.with_extension("json");
        write_atomic(&json_path, json_body.as_bytes())?;
        write_digest_sidecar(&json_path, json_body.as_bytes())?;

        Ok(())
    }

    fn publish_reconciliation_report(
        &self,
        report: &SorafsReconciliationReportV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let digest = blake3::hash(encoded);
        let digest_hex = digest.to_hex().to_string();
        let base_path = self.reconciliation_path(report, &digest_hex);

        let encoded_path = base_path.with_extension("to");
        write_atomic(&encoded_path, encoded)?;
        write_digest_sidecar(&encoded_path, encoded)?;

        let mut payload = JsonMap::new();
        payload.insert(
            "report".into(),
            json::to_value(report).map_err(|err| {
                GovernancePublishError::other(format!("serialize reconciliation report: {err}"))
            })?,
        );

        let mut metadata = JsonMap::new();
        metadata.insert(
            "provider".into(),
            JsonValue::from(hex::encode(report.provider_id)),
        );
        metadata.insert(
            "generated_at_unix".into(),
            JsonValue::from(report.generated_at_unix),
        );
        metadata.insert(
            "repair_snapshot_hash".into(),
            JsonValue::from(hex::encode(report.repair_snapshot_hash)),
        );
        metadata.insert(
            "retention_snapshot_hash".into(),
            JsonValue::from(hex::encode(report.retention_snapshot_hash)),
        );
        metadata.insert(
            "gc_snapshot_hash".into(),
            JsonValue::from(hex::encode(report.gc_snapshot_hash)),
        );
        metadata.insert(
            "divergence_count".into(),
            JsonValue::from(report.divergence_count as u64),
        );
        metadata.insert("encoded_blake3".into(), JsonValue::from(digest_hex.clone()));
        metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
        metadata.insert(
            "encoded_base64".into(),
            JsonValue::from(BASE64_STANDARD.encode(encoded)),
        );
        payload.insert("metadata".into(), JsonValue::Object(metadata));

        let json_body = json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
            GovernancePublishError::other(format!("serialize reconciliation report json: {err}"))
        })?;

        let json_path = base_path.with_extension("json");
        write_atomic(&json_path, json_body.as_bytes())?;
        write_digest_sidecar(&json_path, json_body.as_bytes())?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use norito::codec::Encode;
    use sorafs_manifest::deal::{
        DEAL_LEDGER_VERSION_V1, DEAL_SETTLEMENT_VERSION_V1, DealLedgerSnapshotV1,
    };
    use sorafs_manifest::repair::{
        GC_AUDIT_EVENT_VERSION_V1, GC_AUDIT_PAYLOAD_VERSION_V1, GcAuditEventV1, GcAuditPayloadV1,
        REPAIR_AUDIT_EVENT_VERSION_V1, REPAIR_SLASH_PROPOSAL_VERSION_V1,
        REPAIR_TASK_EVENT_VERSION_V1, RepairAuditEventV1, RepairTaskEventV1, RepairTaskStatusV1,
        RepairTicketId, SorafsAuditHeaderV1,
    };
    use sorafs_manifest::{SORAFS_RECONCILIATION_REPORT_VERSION_V1, SorafsReconciliationReportV1};
    use tempfile::tempdir;

    use super::*;

    fn sample_settlement() -> (DealSettlementV1, Vec<u8>) {
        let deal_id = [0xAB; 32];
        let provider_id = [0xCD; 32];
        let client_id = [0xEF; 32];
        let ledger = DealLedgerSnapshotV1 {
            version: DEAL_LEDGER_VERSION_V1,
            deal_id,
            provider_id,
            client_id,
            provider_accrual: sorafs_manifest::deal::XorAmount::from_micro(500_000),
            client_liability: sorafs_manifest::deal::XorAmount::from_micro(500_000),
            bond_locked: sorafs_manifest::deal::XorAmount::from_micro(1_000_000),
            bond_slashed: sorafs_manifest::deal::XorAmount::zero(),
            captured_at: 1_700_000_000,
        };
        let settlement = DealSettlementV1 {
            version: DEAL_SETTLEMENT_VERSION_V1,
            deal_id,
            ledger,
            status: DealSettlementStatusV1::Completed,
            settled_at: 1_700_000_010,
            audit_notes: None,
        };
        let encoded = Encode::encode(&settlement);
        (settlement, encoded)
    }

    #[test]
    fn filesystem_publisher_writes_settlement_files() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");

        let (settlement, encoded) = sample_settlement();

        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("publish");

        let deal_hex = settlement.deal_id.encode_hex::<String>();
        let dir = temp.path().join("settlements").join(deal_hex);

        let entries = fs::read_dir(&dir)
            .expect("directory exists")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 4, "expected encoded + json + digests");

        let mut encoded_paths = entries
            .iter()
            .filter(|path| path.extension().map(|ext| ext == "to").unwrap_or(false));
        let encoded_path = encoded_paths.next().expect("encoded artefact present");
        assert_eq!(
            fs::read(encoded_path).expect("read encoded"),
            encoded,
            "encoded payload must match original bytes"
        );

        let json_path = entries
            .iter()
            .find(|path| path.extension().map(|ext| ext == "json").unwrap_or(false))
            .expect("json artefact present");
        let json_bytes = fs::read(json_path).expect("read json");
        let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
        let status = value
            .get("metadata")
            .and_then(|meta| meta.get("status"))
            .and_then(JsonValue::as_str)
            .expect("status");
        assert_eq!(status, "completed");

        let encoded_digest_path = entries
            .iter()
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.ends_with("to.blake3"))
                    .unwrap_or(false)
            })
            .expect("encoded digest present");
        let encoded_digest = fs::read_to_string(encoded_digest_path).expect("read encoded digest");
        let encoded_digest = encoded_digest.trim();
        assert_eq!(encoded_digest, blake3::hash(&encoded).to_hex().as_str());

        let json_digest_path = entries
            .iter()
            .find(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.ends_with("json.blake3"))
                    .unwrap_or(false)
            })
            .expect("json digest present");
        let json_digest = fs::read_to_string(json_digest_path).expect("read json digest");
        let json_digest = json_digest.trim();
        assert_eq!(json_digest, blake3::hash(&json_bytes).to_hex().as_str());
    }

    #[test]
    fn atomic_temp_path_preserves_extensions_and_hides_file() {
        let base = Path::new("/tmp/settlement/artifact.norito.to");
        let tmp = temp_path_for_atomic(base, 42, 7);
        let tmp_name = tmp
            .file_name()
            .and_then(|name| name.to_str())
            .expect("name");
        assert!(
            tmp_name.starts_with(".artifact.norito.to.tmp-42-7"),
            "tmp name should keep extensions and add suffix, got {tmp_name}"
        );
        assert!(
            tmp.as_os_str()
                .to_string_lossy()
                .ends_with(".norito.to.tmp-42-7"),
            "tmp path should append to existing extensions"
        );
    }

    #[test]
    fn filesystem_publisher_writes_repair_audit_files() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");

        let payload = RepairTaskEventV1 {
            version: REPAIR_TASK_EVENT_VERSION_V1,
            ticket_id: RepairTicketId("REP-901".into()),
            manifest_digest: [0x21; 32],
            provider_id: [0x22; 32],
            status: RepairTaskStatusV1::Queued,
            occurred_at_unix: 1_700_000_111,
            actor: Some("auditor#sora".into()),
            message: Some("queued".into()),
        };
        let digest = iroha_crypto::Hash::new(payload.encode());
        let header = SorafsAuditHeaderV1 {
            sequence: 42,
            occurred_at_unix: payload.occurred_at_unix,
            signer: "auditor#sora".into(),
            payload_digest: *digest.as_ref(),
        };
        let event = RepairAuditEventV1 {
            version: REPAIR_AUDIT_EVENT_VERSION_V1,
            header,
            payload,
        };
        let encoded = Encode::encode(&event);

        publisher
            .publish_repair_audit_event(&event, &encoded)
            .expect("publish repair audit");

        let dir = temp.path().join("repairs").join("audit");
        let entries = fs::read_dir(&dir)
            .expect("directory exists")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 4, "expected encoded + json + digests");

        let json_path = entries
            .iter()
            .find(|path| path.extension().map(|ext| ext == "json").unwrap_or(false))
            .expect("json artefact present");
        let json_bytes = fs::read(json_path).expect("read json");
        let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
        let manifest_hex = hex::encode(event.payload.manifest_digest);
        let provider_hex = hex::encode(event.payload.provider_id);
        let metadata = value
            .get("metadata")
            .and_then(JsonValue::as_object)
            .expect("metadata");
        let status = metadata
            .get("status")
            .and_then(JsonValue::as_str)
            .expect("status");
        let ticket_id = metadata
            .get("ticket_id")
            .and_then(JsonValue::as_str)
            .expect("ticket_id");
        let manifest = metadata
            .get("manifest")
            .and_then(JsonValue::as_str)
            .expect("manifest");
        let provider = metadata
            .get("provider")
            .and_then(JsonValue::as_str)
            .expect("provider");
        assert_eq!(status, "queued");
        assert_eq!(ticket_id, event.payload.ticket_id.0.as_str());
        assert_eq!(manifest, manifest_hex.as_str());
        assert_eq!(provider, provider_hex.as_str());
    }

    #[test]
    fn filesystem_publisher_writes_repair_slash_files() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");

        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: RepairTicketId("REP-902".into()),
            provider_id: [0x11; 32],
            manifest_digest: [0x22; 32],
            auditor_account: "auditor#sora".into(),
            proposed_penalty_nano: 50_000,
            submitted_at_unix: 1_700_000_222,
            rationale: "missed SLA".into(),
            approval: None,
        };
        let encoded = Encode::encode(&proposal);

        publisher
            .publish_repair_slash_proposal(&proposal, &encoded, RepairSlashStage::Drafted)
            .expect("publish repair slash");

        let dir = temp.path().join("repairs").join("slash");
        let entries = fs::read_dir(&dir)
            .expect("directory exists")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 4, "expected encoded + json + digests");

        let json_path = entries
            .iter()
            .find(|path| path.extension().map(|ext| ext == "json").unwrap_or(false))
            .expect("json artefact present");
        let json_bytes = fs::read(json_path).expect("read json");
        let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
        let manifest_hex = hex::encode(proposal.manifest_digest);
        let provider_hex = hex::encode(proposal.provider_id);
        let metadata = value
            .get("metadata")
            .and_then(JsonValue::as_object)
            .expect("metadata");
        let stage = metadata
            .get("stage")
            .and_then(JsonValue::as_str)
            .expect("stage");
        let outcome = metadata
            .get("outcome")
            .and_then(JsonValue::as_str)
            .expect("outcome");
        let ticket_id = metadata
            .get("ticket_id")
            .and_then(JsonValue::as_str)
            .expect("ticket_id");
        let manifest = metadata
            .get("manifest")
            .and_then(JsonValue::as_str)
            .expect("manifest");
        let provider = metadata
            .get("provider")
            .and_then(JsonValue::as_str)
            .expect("provider");
        assert_eq!(stage, "drafted");
        assert_eq!(outcome, "drafted");
        assert_eq!(ticket_id, proposal.ticket_id.0.as_str());
        assert_eq!(manifest, manifest_hex.as_str());
        assert_eq!(provider, provider_hex.as_str());
    }

    #[test]
    fn filesystem_publisher_writes_gc_audit_files() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");

        let payload = GcAuditPayloadV1 {
            version: GC_AUDIT_PAYLOAD_VERSION_V1,
            manifest_digest: [0x33; 32],
            provider_id: [0x44; 32],
            evicted_at_unix: 1_700_000_333,
            freed_bytes: 4_096,
            reason: "retention_expired".into(),
            blocked_reason: None,
        };
        let digest = iroha_crypto::Hash::new(payload.encode());
        let header = SorafsAuditHeaderV1 {
            sequence: 7,
            occurred_at_unix: payload.evicted_at_unix,
            signer: "sorafs-gc".into(),
            payload_digest: *digest.as_ref(),
        };
        let event = GcAuditEventV1 {
            version: GC_AUDIT_EVENT_VERSION_V1,
            header,
            payload,
        };
        let encoded = Encode::encode(&event);

        publisher
            .publish_gc_audit_event(&event, &encoded)
            .expect("publish gc audit");

        let dir = temp.path().join("gc").join("audit");
        let entries = fs::read_dir(&dir)
            .expect("directory exists")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 4, "expected encoded + json + digests");

        let json_path = entries
            .iter()
            .find(|path| path.extension().map(|ext| ext == "json").unwrap_or(false))
            .expect("json artefact present");
        let json_bytes = fs::read(json_path).expect("read json");
        let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
        let reason = value
            .get("metadata")
            .and_then(|meta| meta.get("reason"))
            .and_then(JsonValue::as_str)
            .expect("reason");
        assert_eq!(reason, "retention_expired");
    }

    #[test]
    fn filesystem_publisher_writes_reconciliation_report_files() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");

        let report = SorafsReconciliationReportV1 {
            version: SORAFS_RECONCILIATION_REPORT_VERSION_V1,
            provider_id: [0x55; 32],
            generated_at_unix: 1_700_000_444,
            repair_snapshot_hash: [0x01; 32],
            retention_snapshot_hash: [0x02; 32],
            gc_snapshot_hash: [0x03; 32],
            repair_task_count: 2,
            retention_manifest_count: 3,
            gc_evictions_total: 4,
            gc_freed_bytes_total: 5,
            divergence_count: 1,
        };
        let encoded = Encode::encode(&report);

        publisher
            .publish_reconciliation_report(&report, &encoded)
            .expect("publish reconciliation report");

        let dir = temp.path().join("reconciliation");
        let entries = fs::read_dir(&dir)
            .expect("directory exists")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 4, "expected encoded + json + digests");

        let json_path = entries
            .iter()
            .find(|path| path.extension().map(|ext| ext == "json").unwrap_or(false))
            .expect("json artefact present");
        let json_bytes = fs::read(json_path).expect("read json");
        let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
        let metadata = value
            .get("metadata")
            .and_then(JsonValue::as_object)
            .expect("metadata");
        let provider = metadata
            .get("provider")
            .and_then(JsonValue::as_str)
            .expect("provider");
        let divergence = metadata
            .get("divergence_count")
            .and_then(JsonValue::as_u64)
            .expect("divergence_count");
        assert_eq!(provider, hex::encode(report.provider_id));
        assert_eq!(divergence, 1);
    }
}
