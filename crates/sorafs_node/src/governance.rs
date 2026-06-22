use std::{
    fmt,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use hex::ToHex;
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, Signature as IrohaSignature};
use norito::json::{self, Map as JsonMap, Value as JsonValue};
use sorafs_car::{CarBuildPlan, CarWriter, FileEntry};
use sorafs_manifest::{
    GOVERNANCE_DAG_BLOCK_VERSION_V1, GOVERNANCE_DAG_HEAD_VERSION_V1, GOVERNANCE_LOG_VERSION_V1,
    GovernanceDagBlockV1, GovernanceDagHeadV1, GovernanceLogNodeV1, GovernanceLogPayloadV1,
    GovernanceLogSignatureV1, GovernanceSignatureAlgorithm, ReputationSnapshotV1,
    SorafsReconciliationReportV1,
    deal::{DealSettlementStatusV1, DealSettlementV1},
    governance_dag_block_cid_v1,
    repair::{GcAuditEventV1, RepairAuditEventV1, RepairSlashProposalV1, RepairTaskStatusV1},
};

use crate::{GovernancePublishError, GovernancePublisher, RepairSlashStage};

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);
const GOVERNANCE_DAG_SINK_FILESYSTEM: &str = "filesystem";
const GOVERNANCE_PUBLISH_INDEX_FILE: &str = "publish-index.json";
const GOVERNANCE_PUBLISH_INDEX_SCHEMA: &str = "sorafs.governance_dag.local_publish_index.v1";
const GOVERNANCE_CAR_QUEUE_FILE: &str = "car-queue.json";
const GOVERNANCE_CAR_QUEUE_SCHEMA: &str = "sorafs.governance_dag.local_car_queue.v1";
const GOVERNANCE_CAR_SEGMENT_SCHEMA: &str = "sorafs.governance_dag.local_car_segment.v1";
const GOVERNANCE_CAR_PLAN_SCHEMA: &str = "sorafs.governance_dag.local_car_plan.v1";
const GOVERNANCE_CAR_SEGMENTS_DIR: &str = "car-segments";
const GOVERNANCE_RUNTIME_DAG_INDEX_FILE: &str = "runtime-dag-index.json";
const GOVERNANCE_RUNTIME_DAG_INDEX_SCHEMA: &str = "sorafs.governance_dag.runtime_signed_index.v1";
const GOVERNANCE_RUNTIME_DAG_DIR: &str = "runtime-dag";
const GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR: &str = "blocks";
const GOVERNANCE_RUNTIME_DAG_HEAD_FILE: &str = "head.to";

#[derive(Debug, Clone)]
struct PublishIndexEntryForCar {
    position: usize,
    payload_kind: String,
    encoded_path: String,
    json_path: String,
    encoded_blake3: String,
    encoded_len: usize,
}

/// Persists governance artefacts on the filesystem for downstream ingestion.
#[derive(Debug)]
pub struct FilesystemGovernancePublisher {
    root: PathBuf,
    runtime_dag_signer: Option<GovernanceRuntimeDagSigner>,
}

#[derive(Clone)]
struct GovernanceRuntimeDagSigner {
    publisher_peer_id: Vec<u8>,
    private_key: PrivateKey,
    public_key: Vec<u8>,
}

impl fmt::Debug for GovernanceRuntimeDagSigner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceRuntimeDagSigner")
            .field("publisher_peer_id", &self.publisher_peer_id)
            .field("public_key", &hex::encode(&self.public_key))
            .finish_non_exhaustive()
    }
}

impl FilesystemGovernancePublisher {
    /// Construct a new publisher rooted at the supplied directory.
    pub fn try_new(root: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&root)?;
        Ok(Self {
            root,
            runtime_dag_signer: None,
        })
    }

    /// Enable signed runtime Governance DAG block/head assembly.
    pub fn with_runtime_dag_signer(
        mut self,
        publisher_peer_id: impl Into<Vec<u8>>,
        signing_key_path: impl AsRef<Path>,
    ) -> Result<Self, GovernancePublishError> {
        let private_key = load_runtime_dag_signing_key(signing_key_path.as_ref())?;
        self.runtime_dag_signer = Some(GovernanceRuntimeDagSigner::try_new(
            publisher_peer_id.into(),
            private_key,
        )?);
        Ok(self)
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

    fn reputation_root(&self) -> PathBuf {
        self.root.join("reputation")
    }

    fn reputation_snapshot_root(&self) -> PathBuf {
        self.reputation_root().join("snapshots")
    }

    fn record_publish_index(
        &self,
        payload_kind: &str,
        encoded_path: &Path,
        json_path: &Path,
        digest_hex: &str,
        encoded_len: usize,
        labels: JsonMap,
    ) -> Result<(), GovernancePublishError> {
        let entry = update_publish_index(
            &self.root,
            payload_kind,
            encoded_path,
            json_path,
            digest_hex,
            encoded_len,
            labels,
        )?;
        ensure_governance_car_segment(&self.root, &entry)
    }

    fn record_runtime_signed_payload(
        &self,
        payload_kind: &str,
        payload: GovernanceLogPayloadV1,
        encoded_path: &Path,
        json_path: &Path,
        digest_hex: &str,
        encoded_len: usize,
    ) -> Result<(), GovernancePublishError> {
        let Some(signer) = &self.runtime_dag_signer else {
            return Ok(());
        };
        append_runtime_signed_dag_payload(
            &self.root,
            signer,
            payload_kind,
            payload,
            encoded_path,
            json_path,
            digest_hex,
            encoded_len,
        )
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

    fn reputation_snapshot_path(
        &self,
        snapshot: &ReputationSnapshotV1,
        digest_hex: &str,
    ) -> PathBuf {
        let snapshot_hex = hex::encode(snapshot.snapshot_id);
        let digest_prefix = &digest_hex[..16];
        let base = format!(
            "{:020}_{}_{}",
            snapshot.generated_at_unix, snapshot_hex, digest_prefix
        );
        self.reputation_snapshot_root()
            .join(snapshot_hex)
            .join(base)
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
    let digest_path = digest_sidecar_path_for(path);
    let mut body = hex;
    body.push('\n');
    write_atomic(&digest_path, body.as_bytes())
}

fn digest_sidecar_path_for(path: &Path) -> PathBuf {
    let suffix = match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if !ext.is_empty() => format!("{ext}.blake3"),
        _ => "blake3".to_string(),
    };
    path.with_extension(suffix)
}

fn temp_path_for_atomic(path: &Path, pid: u32, counter: u64) -> PathBuf {
    let suffix = format!("tmp-{pid}-{counter}");
    let candidate = path.with_added_extension(&suffix);
    match candidate.file_name().and_then(|name| name.to_str()) {
        Some(name) => candidate.with_file_name(format!(".{name}")),
        None => candidate,
    }
}

fn current_unix_timestamp_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

impl GovernanceRuntimeDagSigner {
    fn try_new(
        publisher_peer_id: Vec<u8>,
        private_key: PrivateKey,
    ) -> Result<Self, GovernancePublishError> {
        if publisher_peer_id.is_empty() {
            return Err(GovernancePublishError::other(
                "governance runtime DAG publisher peer id must not be empty",
            ));
        }
        let keypair = KeyPair::from_private_key(private_key.clone()).map_err(|err| {
            GovernancePublishError::other(format!(
                "failed to derive governance runtime DAG signing keypair: {err}"
            ))
        })?;
        let (algorithm, public_key) = keypair.public_key().try_to_bytes().map_err(|err| {
            GovernancePublishError::other(format!(
                "failed to extract governance runtime DAG signing public key: {err}"
            ))
        })?;
        if algorithm != Algorithm::Ed25519 {
            return Err(GovernancePublishError::other(format!(
                "governance runtime DAG signing key must derive an Ed25519 public key, found {}",
                algorithm.as_static_str()
            )));
        }
        if public_key.len() != 32 {
            return Err(GovernancePublishError::other(format!(
                "governance runtime DAG signing public key must be 32 bytes, found {}",
                public_key.len()
            )));
        }
        Ok(Self {
            publisher_peer_id,
            private_key,
            public_key: public_key.to_vec(),
        })
    }

    fn sign(&self, payload: &[u8]) -> Result<GovernanceLogSignatureV1, GovernancePublishError> {
        let signature = IrohaSignature::try_new(&self.private_key, payload).map_err(|err| {
            GovernancePublishError::other(format!("failed to sign governance runtime DAG: {err}"))
        })?;
        Ok(GovernanceLogSignatureV1 {
            algorithm: GovernanceSignatureAlgorithm::Ed25519,
            public_key: self.public_key.clone(),
            signature: signature.payload().to_vec(),
        })
    }

    fn publisher_peer_id_hex(&self) -> String {
        hex::encode(&self.publisher_peer_id)
    }

    fn publisher_public_key_hex(&self) -> String {
        hex::encode(&self.public_key)
    }
}

fn load_runtime_dag_signing_key(path: &Path) -> Result<PrivateKey, GovernancePublishError> {
    let raw = fs::read(path).map_err(|err| {
        GovernancePublishError::other(format!(
            "failed to read governance runtime DAG signing key from {}: {err}",
            path.display()
        ))
    })?;
    let trimmed = String::from_utf8_lossy(&raw).trim().to_owned();
    let key_bytes = if trimmed.len() == 64 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        hex::decode(trimmed).map_err(|err| {
            GovernancePublishError::other(format!(
                "failed to decode governance runtime DAG hex signing key: {err}"
            ))
        })?
    } else {
        raw
    };
    if key_bytes.len() != 32 {
        return Err(GovernancePublishError::other(format!(
            "governance runtime DAG signing key at {} must be 32 bytes, found {}",
            path.display(),
            key_bytes.len()
        )));
    }

    let mut array = [0u8; 32];
    array.copy_from_slice(&key_bytes);
    PrivateKey::from_bytes(Algorithm::Ed25519, &array).map_err(|err| {
        GovernancePublishError::other(format!(
            "failed to parse governance runtime DAG signing key: {err}"
        ))
    })
}

fn update_publish_index(
    root: &Path,
    payload_kind: &str,
    encoded_path: &Path,
    json_path: &Path,
    digest_hex: &str,
    encoded_len: usize,
    labels: JsonMap,
) -> Result<PublishIndexEntryForCar, GovernancePublishError> {
    let index_path = root.join(GOVERNANCE_PUBLISH_INDEX_FILE);
    let mut index = read_publish_index(root, &index_path)?;
    let mut entries = match index.remove("entries") {
        Some(JsonValue::Array(entries)) => entries,
        Some(_) => {
            return Err(GovernancePublishError::other(
                "governance publish index has non-array `entries`",
            ));
        }
        None => Vec::new(),
    };
    let encoded_path = index_path_string(root, encoded_path);
    let json_path = index_path_string(root, json_path);
    let duplicate_position = entries.iter().position(|entry| {
        entry.get("payload_kind").and_then(JsonValue::as_str) == Some(payload_kind)
            && entry.get("encoded_blake3").and_then(JsonValue::as_str) == Some(digest_hex)
            && entry.get("encoded_path").and_then(JsonValue::as_str) == Some(encoded_path.as_str())
    });
    let position = duplicate_position.unwrap_or(entries.len());
    if duplicate_position.is_none() {
        let mut entry = JsonMap::new();
        entry.insert("position".into(), JsonValue::from(position as u64));
        entry.insert("payload_kind".into(), JsonValue::from(payload_kind));
        entry.insert("encoded_path".into(), JsonValue::from(encoded_path.clone()));
        entry.insert("json_path".into(), JsonValue::from(json_path.clone()));
        entry.insert(
            "encoded_blake3".into(),
            JsonValue::from(digest_hex.to_string()),
        );
        entry.insert(
            "encoded_len".into(),
            JsonValue::from(u64::try_from(encoded_len).unwrap_or(u64::MAX)),
        );
        entry.insert(
            "published_at_unix".into(),
            JsonValue::from(current_unix_timestamp_seconds()),
        );
        entry.insert("labels".into(), JsonValue::Object(labels));
        entries.push(JsonValue::Object(entry));
    }
    rebuild_publish_index(root, index, entries, &index_path)?;
    Ok(PublishIndexEntryForCar {
        position,
        payload_kind: payload_kind.to_owned(),
        encoded_path,
        json_path,
        encoded_blake3: digest_hex.to_owned(),
        encoded_len,
    })
}

fn read_publish_index(root: &Path, index_path: &Path) -> Result<JsonMap, GovernancePublishError> {
    match fs::read(index_path) {
        Ok(bytes) => {
            let value: JsonValue = json::from_slice(&bytes).map_err(|err| {
                GovernancePublishError::other(format!(
                    "failed to parse governance publish index `{}`: {err}",
                    index_path.display()
                ))
            })?;
            let JsonValue::Object(map) = value else {
                return Err(GovernancePublishError::other(
                    "governance publish index root is not an object",
                ));
            };
            if map.get("schema").and_then(JsonValue::as_str)
                != Some(GOVERNANCE_PUBLISH_INDEX_SCHEMA)
            {
                return Err(GovernancePublishError::other(
                    "governance publish index uses an unsupported schema",
                ));
            }
            Ok(map)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            let mut map = JsonMap::new();
            map.insert(
                "schema".into(),
                JsonValue::from(GOVERNANCE_PUBLISH_INDEX_SCHEMA),
            );
            map.insert(
                "source".into(),
                JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
            );
            map.insert("root".into(), JsonValue::from(root.display().to_string()));
            map.insert("entries".into(), JsonValue::Array(Vec::new()));
            Ok(map)
        }
        Err(err) => Err(err.into()),
    }
}

fn rebuild_publish_index(
    root: &Path,
    mut index: JsonMap,
    mut entries: Vec<JsonValue>,
    index_path: &Path,
) -> Result<(), GovernancePublishError> {
    let mut payload_kind_counts = JsonMap::new();
    let mut by_encoded_blake3 = JsonMap::new();
    let mut by_payload_kind = JsonMap::new();

    for (position, entry) in entries.iter_mut().enumerate() {
        let Some(entry_map) = entry.as_object_mut() else {
            return Err(GovernancePublishError::other(
                "governance publish index entry is not an object",
            ));
        };
        entry_map.insert("position".into(), JsonValue::from(position as u64));
        let Some(payload_kind) = entry_map
            .get("payload_kind")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
        else {
            return Err(GovernancePublishError::other(
                "governance publish index entry is missing `payload_kind`",
            ));
        };
        let count = payload_kind_counts
            .get(&payload_kind)
            .and_then(JsonValue::as_u64)
            .unwrap_or(0)
            .saturating_add(1);
        payload_kind_counts.insert(payload_kind.clone(), JsonValue::from(count));
        append_index_position(&mut by_payload_kind, &payload_kind, position);

        let Some(digest_hex) = entry_map
            .get("encoded_blake3")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
        else {
            return Err(GovernancePublishError::other(
                "governance publish index entry is missing `encoded_blake3`",
            ));
        };
        append_index_position(&mut by_encoded_blake3, &digest_hex, position);
    }

    index.insert(
        "schema".into(),
        JsonValue::from(GOVERNANCE_PUBLISH_INDEX_SCHEMA),
    );
    index.insert(
        "source".into(),
        JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
    );
    index.insert("root".into(), JsonValue::from(root.display().to_string()));
    index.insert(
        "generated_at".into(),
        JsonValue::from(current_unix_timestamp_seconds()),
    );
    index.insert("entry_count".into(), JsonValue::from(entries.len() as u64));
    index.insert(
        "payload_kind_counts".into(),
        JsonValue::Object(payload_kind_counts),
    );
    index.insert(
        "by_encoded_blake3".into(),
        JsonValue::Object(by_encoded_blake3),
    );
    index.insert("by_payload_kind".into(), JsonValue::Object(by_payload_kind));
    index.insert("entries".into(), JsonValue::Array(entries));

    let body = json::to_json_pretty(&JsonValue::Object(index)).map_err(|err| {
        GovernancePublishError::other(format!("serialize governance publish index: {err}"))
    })?;
    write_atomic(index_path, body.as_bytes())?;
    write_digest_sidecar(index_path, body.as_bytes())?;
    Ok(())
}

fn append_index_position(index: &mut JsonMap, key: &str, position: usize) {
    let position = JsonValue::from(position as u64);
    match index.get_mut(key).and_then(JsonValue::as_array_mut) {
        Some(positions) => positions.push(position),
        None => {
            index.insert(key.to_string(), JsonValue::Array(vec![position]));
        }
    }
}

fn index_path_string(root: &Path, path: &Path) -> String {
    let path = path.strip_prefix(root).unwrap_or(path);
    let parts = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        ".".to_string()
    } else {
        parts.join("/")
    }
}

fn ensure_governance_car_segment(
    root: &Path,
    entry: &PublishIndexEntryForCar,
) -> Result<(), GovernancePublishError> {
    let queue_path = root.join(GOVERNANCE_CAR_QUEUE_FILE);
    let mut queue = read_car_queue(root, &queue_path)?;
    let mut segments = match queue.remove("segments") {
        Some(JsonValue::Array(segments)) => segments,
        Some(_) => {
            return Err(GovernancePublishError::other(
                "governance CAR queue has non-array `segments`",
            ));
        }
        None => Vec::new(),
    };
    let existing_position = segments.iter().position(|segment| {
        segment
            .get("source_publish_index_position")
            .and_then(JsonValue::as_u64)
            == Some(entry.position as u64)
            && segment.get("encoded_blake3").and_then(JsonValue::as_str)
                == Some(entry.encoded_blake3.as_str())
    });
    if let Some(position) = existing_position
        && governance_car_segment_files_exist(root, &segments[position])
    {
        record_governance_dag_backlog(governance_car_queue_pending_count(&segments));
        return Ok(());
    }

    let segment = assemble_governance_car_segment(root, entry)?;
    match existing_position {
        Some(position) => segments[position] = JsonValue::Object(segment),
        None => segments.push(JsonValue::Object(segment)),
    }
    rebuild_car_queue(root, queue, segments, &queue_path)
}

fn read_car_queue(root: &Path, queue_path: &Path) -> Result<JsonMap, GovernancePublishError> {
    match fs::read(queue_path) {
        Ok(bytes) => {
            let value: JsonValue = json::from_slice(&bytes).map_err(|err| {
                GovernancePublishError::other(format!(
                    "failed to parse governance CAR queue `{}`: {err}",
                    queue_path.display()
                ))
            })?;
            let JsonValue::Object(map) = value else {
                return Err(GovernancePublishError::other(
                    "governance CAR queue root is not an object",
                ));
            };
            if map.get("schema").and_then(JsonValue::as_str) != Some(GOVERNANCE_CAR_QUEUE_SCHEMA) {
                return Err(GovernancePublishError::other(
                    "governance CAR queue uses an unsupported schema",
                ));
            }
            Ok(map)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            let mut map = JsonMap::new();
            map.insert(
                "schema".into(),
                JsonValue::from(GOVERNANCE_CAR_QUEUE_SCHEMA),
            );
            map.insert(
                "source".into(),
                JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
            );
            map.insert("root".into(), JsonValue::from(root.display().to_string()));
            map.insert("segments".into(), JsonValue::Array(Vec::new()));
            Ok(map)
        }
        Err(err) => Err(err.into()),
    }
}

fn rebuild_car_queue(
    root: &Path,
    mut queue: JsonMap,
    mut segments: Vec<JsonValue>,
    queue_path: &Path,
) -> Result<(), GovernancePublishError> {
    let mut by_encoded_blake3 = JsonMap::new();
    let mut by_payload_kind = JsonMap::new();
    let mut assembled_count = 0u64;

    for (position, segment) in segments.iter_mut().enumerate() {
        let Some(segment_map) = segment.as_object_mut() else {
            return Err(GovernancePublishError::other(
                "governance CAR queue segment is not an object",
            ));
        };
        segment_map.insert("queue_position".into(), JsonValue::from(position as u64));
        if segment_map.get("schema").and_then(JsonValue::as_str)
            != Some(GOVERNANCE_CAR_SEGMENT_SCHEMA)
        {
            return Err(GovernancePublishError::other(
                "governance CAR queue segment uses an unsupported schema",
            ));
        }
        let Some(payload_kind) = segment_map
            .get("payload_kind")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
        else {
            return Err(GovernancePublishError::other(
                "governance CAR queue segment is missing `payload_kind`",
            ));
        };
        append_index_position(&mut by_payload_kind, &payload_kind, position);
        let Some(digest_hex) = segment_map
            .get("encoded_blake3")
            .and_then(JsonValue::as_str)
            .map(str::to_owned)
        else {
            return Err(GovernancePublishError::other(
                "governance CAR queue segment is missing `encoded_blake3`",
            ));
        };
        append_index_position(&mut by_encoded_blake3, &digest_hex, position);
        if segment_map
            .get("status")
            .and_then(JsonValue::as_str)
            .is_some_and(|status| status == "assembled")
        {
            assembled_count = assembled_count.saturating_add(1);
        }
    }

    queue.insert(
        "schema".into(),
        JsonValue::from(GOVERNANCE_CAR_QUEUE_SCHEMA),
    );
    queue.insert(
        "source".into(),
        JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
    );
    queue.insert("root".into(), JsonValue::from(root.display().to_string()));
    queue.insert(
        "generated_at".into(),
        JsonValue::from(current_unix_timestamp_seconds()),
    );
    queue.insert(
        "segment_count".into(),
        JsonValue::from(segments.len() as u64),
    );
    queue.insert("assembled_count".into(), JsonValue::from(assembled_count));
    let pending_count = (segments.len() as u64).saturating_sub(assembled_count);
    queue.insert("pending_count".into(), JsonValue::from(pending_count));
    queue.insert(
        "by_encoded_blake3".into(),
        JsonValue::Object(by_encoded_blake3),
    );
    queue.insert("by_payload_kind".into(), JsonValue::Object(by_payload_kind));
    queue.insert("segments".into(), JsonValue::Array(segments));

    let body = json::to_json_pretty(&JsonValue::Object(queue)).map_err(|err| {
        GovernancePublishError::other(format!("serialize governance CAR queue: {err}"))
    })?;
    write_atomic(queue_path, body.as_bytes())?;
    write_digest_sidecar(queue_path, body.as_bytes())?;
    record_governance_dag_backlog(pending_count);
    Ok(())
}

fn governance_car_queue_pending_count(segments: &[JsonValue]) -> u64 {
    let assembled_count = segments
        .iter()
        .filter(|segment| {
            segment
                .get("status")
                .and_then(JsonValue::as_str)
                .is_some_and(|status| status == "assembled")
        })
        .count() as u64;
    (segments.len() as u64).saturating_sub(assembled_count)
}

fn governance_car_segment_files_exist(root: &Path, segment: &JsonValue) -> bool {
    let Some(segment) = segment.as_object() else {
        return false;
    };
    ["car_path", "plan_path", "manifest_path"]
        .iter()
        .all(|field| {
            segment
                .get(*field)
                .and_then(JsonValue::as_str)
                .and_then(|path| resolve_index_path(root, path).ok())
                .is_some_and(|path| path.is_file())
        })
}

fn assemble_governance_car_segment(
    root: &Path,
    entry: &PublishIndexEntryForCar,
) -> Result<JsonMap, GovernancePublishError> {
    let (files, file_records) = governance_car_segment_files(root, entry)?;
    let (plan, payload) = CarBuildPlan::from_files(files).map_err(|err| {
        GovernancePublishError::other(format!("build governance CAR segment plan: {err}"))
    })?;
    let mut car_bytes = Vec::new();
    let stats = CarWriter::new(&plan, &payload)
        .map_err(|err| GovernancePublishError::other(format!("initialise CAR writer: {err}")))?
        .write_to(&mut car_bytes)
        .map_err(|err| GovernancePublishError::other(format!("write CAR segment: {err}")))?;

    let base_path = governance_car_segment_base_path(root, entry);
    let car_path = base_path.with_extension("car");
    let plan_path = base_path.with_extension("plan.json");
    let manifest_path = base_path.with_extension("json");

    write_atomic(&car_path, &car_bytes)?;
    write_digest_sidecar(&car_path, &car_bytes)?;

    let plan_json = governance_car_plan_json(entry, &plan, &stats, &file_records);
    let plan_body = json::to_json_pretty(&JsonValue::Object(plan_json)).map_err(|err| {
        GovernancePublishError::other(format!("serialize governance CAR plan: {err}"))
    })?;
    write_atomic(&plan_path, plan_body.as_bytes())?;
    write_digest_sidecar(&plan_path, plan_body.as_bytes())?;

    let segment_json = governance_car_segment_json(
        root,
        entry,
        &stats,
        &file_records,
        &car_path,
        &plan_path,
        &manifest_path,
    );
    let segment_body =
        json::to_json_pretty(&JsonValue::Object(segment_json.clone())).map_err(|err| {
            GovernancePublishError::other(format!("serialize governance CAR segment: {err}"))
        })?;
    write_atomic(&manifest_path, segment_body.as_bytes())?;
    write_digest_sidecar(&manifest_path, segment_body.as_bytes())?;
    Ok(segment_json)
}

fn governance_car_segment_base_path(root: &Path, entry: &PublishIndexEntryForCar) -> PathBuf {
    let digest_prefix = &entry.encoded_blake3[..entry.encoded_blake3.len().min(16)];
    let base = format!(
        "{:020}_{}_{}",
        entry.position,
        sanitize_label(&entry.payload_kind),
        digest_prefix
    );
    root.join(GOVERNANCE_CAR_SEGMENTS_DIR).join(base)
}

fn governance_car_segment_files(
    root: &Path,
    entry: &PublishIndexEntryForCar,
) -> Result<(Vec<FileEntry>, Vec<JsonValue>), GovernancePublishError> {
    let encoded_path = resolve_index_path(root, &entry.encoded_path)?;
    let json_path = resolve_index_path(root, &entry.json_path)?;
    let encoded_sidecar = digest_sidecar_path_for(&encoded_path);
    let json_sidecar = digest_sidecar_path_for(&json_path);
    let encoded_sidecar_path = index_path_string(root, &encoded_sidecar);
    let json_sidecar_path = index_path_string(root, &json_sidecar);
    let specs = [
        ("encoded", entry.encoded_path.as_str(), encoded_path),
        (
            "encoded_blake3_sidecar",
            encoded_sidecar_path.as_str(),
            encoded_sidecar,
        ),
        ("json", entry.json_path.as_str(), json_path),
        (
            "json_blake3_sidecar",
            json_sidecar_path.as_str(),
            json_sidecar,
        ),
    ];
    let mut files = Vec::with_capacity(specs.len());
    let mut records = Vec::with_capacity(specs.len());
    for (role, relative_path, absolute_path) in specs {
        let bytes = fs::read(&absolute_path).map_err(|err| {
            GovernancePublishError::other(format!(
                "read governance CAR segment source `{}`: {err}",
                absolute_path.display()
            ))
        })?;
        let mut record = JsonMap::new();
        record.insert("role".into(), JsonValue::from(role));
        record.insert("path".into(), JsonValue::from(relative_path));
        record.insert("bytes".into(), JsonValue::from(bytes.len() as u64));
        record.insert(
            "blake3".into(),
            JsonValue::from(blake3::hash(&bytes).to_hex().to_string()),
        );
        files.push(FileEntry {
            path: index_path_components(relative_path)?,
            data: bytes,
        });
        records.push(JsonValue::Object(record));
    }
    Ok((files, records))
}

fn governance_car_plan_json(
    entry: &PublishIndexEntryForCar,
    plan: &CarBuildPlan,
    stats: &sorafs_car::CarWriteStats,
    file_records: &[JsonValue],
) -> JsonMap {
    let mut root = JsonMap::new();
    root.insert("schema".into(), JsonValue::from(GOVERNANCE_CAR_PLAN_SCHEMA));
    root.insert(
        "source_publish_index_position".into(),
        JsonValue::from(entry.position as u64),
    );
    root.insert(
        "payload_kind".into(),
        JsonValue::from(entry.payload_kind.clone()),
    );
    root.insert(
        "encoded_blake3".into(),
        JsonValue::from(entry.encoded_blake3.clone()),
    );
    root.insert(
        "encoded_len".into(),
        JsonValue::from(u64::try_from(entry.encoded_len).unwrap_or(u64::MAX)),
    );
    root.insert(
        "content_length".into(),
        JsonValue::from(plan.content_length),
    );
    root.insert(
        "payload_blake3".into(),
        JsonValue::from(plan.payload_digest.to_hex().to_string()),
    );
    root.insert("dag_codec".into(), JsonValue::from(stats.dag_codec));
    root.insert(
        "chunk_count".into(),
        JsonValue::from(plan.chunks.len() as u64),
    );
    root.insert("files".into(), JsonValue::Array(file_records.to_vec()));
    root.insert("chunk_profile".into(), chunk_profile_json(plan));
    root.insert("chunks".into(), governance_car_chunks_json(plan));
    root
}

fn governance_car_segment_json(
    root: &Path,
    entry: &PublishIndexEntryForCar,
    stats: &sorafs_car::CarWriteStats,
    file_records: &[JsonValue],
    car_path: &Path,
    plan_path: &Path,
    manifest_path: &Path,
) -> JsonMap {
    let mut segment = JsonMap::new();
    segment.insert(
        "schema".into(),
        JsonValue::from(GOVERNANCE_CAR_SEGMENT_SCHEMA),
    );
    segment.insert("status".into(), JsonValue::from("assembled"));
    segment.insert(
        "source".into(),
        JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
    );
    segment.insert(
        "source_publish_index_position".into(),
        JsonValue::from(entry.position as u64),
    );
    segment.insert(
        "payload_kind".into(),
        JsonValue::from(entry.payload_kind.clone()),
    );
    segment.insert(
        "encoded_path".into(),
        JsonValue::from(entry.encoded_path.clone()),
    );
    segment.insert("json_path".into(), JsonValue::from(entry.json_path.clone()));
    segment.insert(
        "encoded_blake3".into(),
        JsonValue::from(entry.encoded_blake3.clone()),
    );
    segment.insert(
        "encoded_len".into(),
        JsonValue::from(u64::try_from(entry.encoded_len).unwrap_or(u64::MAX)),
    );
    segment.insert(
        "car_path".into(),
        JsonValue::from(index_path_string(root, car_path)),
    );
    segment.insert(
        "plan_path".into(),
        JsonValue::from(index_path_string(root, plan_path)),
    );
    segment.insert(
        "manifest_path".into(),
        JsonValue::from(index_path_string(root, manifest_path)),
    );
    segment.insert("car_size".into(), JsonValue::from(stats.car_size));
    segment.insert(
        "car_archive_blake3".into(),
        JsonValue::from(stats.car_archive_digest.to_hex().to_string()),
    );
    segment.insert(
        "car_payload_blake3".into(),
        JsonValue::from(stats.car_payload_digest.to_hex().to_string()),
    );
    segment.insert(
        "car_cid_hex".into(),
        JsonValue::from(hex::encode(&stats.car_cid)),
    );
    segment.insert(
        "root_cids_hex".into(),
        JsonValue::Array(
            stats
                .root_cids
                .iter()
                .map(|cid| JsonValue::from(hex::encode(cid)))
                .collect(),
        ),
    );
    segment.insert("dag_codec".into(), JsonValue::from(stats.dag_codec));
    segment.insert(
        "chunk_count".into(),
        JsonValue::from(stats.chunk_count as u64),
    );
    segment.insert("payload_bytes".into(), JsonValue::from(stats.payload_bytes));
    segment.insert(
        "assembled_at_unix".into(),
        JsonValue::from(current_unix_timestamp_seconds()),
    );
    segment.insert("files".into(), JsonValue::Array(file_records.to_vec()));
    segment.insert("chunk_profile".into(), chunk_profile_json_from_stats(stats));
    segment
}

fn chunk_profile_json(plan: &CarBuildPlan) -> JsonValue {
    let profile = plan.chunk_profile;
    let mut value = JsonMap::new();
    value.insert("min_size".into(), JsonValue::from(profile.min_size as u64));
    value.insert(
        "target_size".into(),
        JsonValue::from(profile.target_size as u64),
    );
    value.insert("max_size".into(), JsonValue::from(profile.max_size as u64));
    value.insert("break_mask".into(), JsonValue::from(profile.break_mask));
    JsonValue::Object(value)
}

fn chunk_profile_json_from_stats(stats: &sorafs_car::CarWriteStats) -> JsonValue {
    let profile = stats.chunk_profile;
    let mut value = JsonMap::new();
    value.insert("min_size".into(), JsonValue::from(profile.min_size as u64));
    value.insert(
        "target_size".into(),
        JsonValue::from(profile.target_size as u64),
    );
    value.insert("max_size".into(), JsonValue::from(profile.max_size as u64));
    value.insert("break_mask".into(), JsonValue::from(profile.break_mask));
    JsonValue::Object(value)
}

fn governance_car_chunks_json(plan: &CarBuildPlan) -> JsonValue {
    JsonValue::Array(
        plan.chunks
            .iter()
            .enumerate()
            .map(|(index, chunk)| {
                let mut value = JsonMap::new();
                value.insert("index".into(), JsonValue::from(index as u64));
                value.insert("offset".into(), JsonValue::from(chunk.offset));
                value.insert("length".into(), JsonValue::from(chunk.length as u64));
                value.insert("blake3".into(), JsonValue::from(hex::encode(chunk.digest)));
                JsonValue::Object(value)
            })
            .collect(),
    )
}

fn resolve_index_path(root: &Path, relative_path: &str) -> Result<PathBuf, GovernancePublishError> {
    let components = index_path_components(relative_path)?;
    let mut path = root.to_path_buf();
    for component in components {
        path.push(component);
    }
    Ok(path)
}

fn index_path_components(relative_path: &str) -> Result<Vec<String>, GovernancePublishError> {
    if relative_path.is_empty()
        || relative_path == "."
        || relative_path.starts_with('/')
        || relative_path.contains('\\')
    {
        return Err(GovernancePublishError::other(
            "governance CAR queue path must be a relative slash-separated path",
        ));
    }
    let mut components = Vec::new();
    for component in relative_path.split('/') {
        if component.is_empty() || component == "." || component == ".." {
            return Err(GovernancePublishError::other(
                "governance CAR queue path contains an invalid component",
            ));
        }
        components.push(component.to_owned());
    }
    Ok(components)
}

#[derive(Debug, Clone)]
struct RuntimeDagTip {
    sequence: u64,
    block_cid: Vec<u8>,
    node_cid: Vec<u8>,
}

fn append_runtime_signed_dag_payload(
    root: &Path,
    signer: &GovernanceRuntimeDagSigner,
    payload_kind: &str,
    payload: GovernanceLogPayloadV1,
    encoded_path: &Path,
    json_path: &Path,
    digest_hex: &str,
    encoded_len: usize,
) -> Result<(), GovernancePublishError> {
    let index_path = root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE);
    let mut index = read_runtime_dag_index(root, signer, &index_path)?;
    let mut blocks = match index.remove("blocks") {
        Some(JsonValue::Array(blocks)) => blocks,
        Some(_) => {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index has non-array `blocks`",
            ));
        }
        None => Vec::new(),
    };

    let duplicate_position = blocks.iter().position(|entry| {
        entry.get("payload_kind").and_then(JsonValue::as_str) == Some(payload_kind)
            && entry.get("encoded_blake3").and_then(JsonValue::as_str) == Some(digest_hex)
    });
    if let Some(position) = duplicate_position {
        if runtime_dag_index_entry_files_exist(root, &blocks[position]) {
            record_governance_dag_head_age_from_index(&index);
            return Ok(());
        }
        return Err(GovernancePublishError::other(
            "governance runtime DAG index references a missing block file",
        ));
    }

    let tip = runtime_dag_tip_from_entries(&blocks)?;
    let sequence = tip
        .as_ref()
        .map(|tip| tip.sequence.saturating_add(1))
        .unwrap_or(0);
    let timestamp = current_unix_timestamp_seconds();
    let mut node = GovernanceLogNodeV1 {
        version: GOVERNANCE_LOG_VERSION_V1,
        node_cid: Vec::new(),
        prev_cid: tip.as_ref().map(|tip| tip.node_cid.clone()),
        timestamp,
        publisher_peer_id: signer.publisher_peer_id.clone(),
        payload,
        publisher_signature: empty_governance_ed25519_signature(),
    };
    node.node_cid = node.recompute_node_cid().map_err(|err| {
        GovernancePublishError::other(format!("derive governance runtime DAG node CID: {err}"))
    })?;
    let node_payload = node.signature_payload_bytes().map_err(|err| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG node signing payload: {err}"
        ))
    })?;
    node.publisher_signature = signer.sign(&node_payload)?;
    node.validate().map_err(|err| {
        GovernancePublishError::other(format!("validate governance runtime DAG node: {err}"))
    })?;
    node.verify_publisher_signature().map_err(|err| {
        GovernancePublishError::other(format!(
            "verify governance runtime DAG node signature: {err}"
        ))
    })?;

    let prev_block_cid = tip.as_ref().map(|tip| tip.block_cid.clone());
    let block_cid = governance_dag_block_cid_v1(
        prev_block_cid.as_deref(),
        sequence,
        timestamp,
        &signer.publisher_peer_id,
        &node,
    )
    .map_err(|err| {
        GovernancePublishError::other(format!("derive governance runtime DAG block CID: {err}"))
    })?;
    let mut block = GovernanceDagBlockV1 {
        version: GOVERNANCE_DAG_BLOCK_VERSION_V1,
        block_cid,
        prev_block_cid,
        sequence,
        timestamp,
        publisher_peer_id: signer.publisher_peer_id.clone(),
        node,
        block_signature: empty_governance_ed25519_signature(),
    };
    let block_payload = block.signature_payload_bytes().map_err(|err| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG block signing payload: {err}"
        ))
    })?;
    block.block_signature = signer.sign(&block_payload)?;
    block.validate().map_err(|err| {
        GovernancePublishError::other(format!("validate governance runtime DAG block: {err}"))
    })?;

    let mut head = GovernanceDagHeadV1 {
        version: GOVERNANCE_DAG_HEAD_VERSION_V1,
        head_block_cid: block.block_cid.clone(),
        block_count: sequence.saturating_add(1),
        generated_at: timestamp,
        publisher_peer_id: signer.publisher_peer_id.clone(),
        checkpoint_cid: None,
        head_signature: empty_governance_ed25519_signature(),
    };
    let head_payload = head.signature_payload_bytes().map_err(|err| {
        GovernancePublishError::other(format!(
            "encode governance runtime DAG head signing payload: {err}"
        ))
    })?;
    head.head_signature = signer.sign(&head_payload)?;
    head.validate().map_err(|err| {
        GovernancePublishError::other(format!("validate governance runtime DAG head: {err}"))
    })?;

    let block_bytes = norito::to_bytes(&block).map_err(|err| {
        GovernancePublishError::other(format!("encode governance runtime DAG block: {err}"))
    })?;
    let block_cid_hex = hex::encode(&block.block_cid);
    let block_path = runtime_dag_block_path(root, sequence, &block_cid_hex);
    write_atomic(&block_path, &block_bytes)?;
    write_digest_sidecar(&block_path, &block_bytes)?;

    let head_bytes = norito::to_bytes(&head).map_err(|err| {
        GovernancePublishError::other(format!("encode governance runtime DAG head: {err}"))
    })?;
    let head_path = runtime_dag_head_path(root);
    write_atomic(&head_path, &head_bytes)?;
    write_digest_sidecar(&head_path, &head_bytes)?;

    let mut entry = JsonMap::new();
    entry.insert("position".into(), JsonValue::from(blocks.len() as u64));
    entry.insert("sequence".into(), JsonValue::from(sequence));
    entry.insert("payload_kind".into(), JsonValue::from(payload_kind));
    entry.insert(
        "encoded_blake3".into(),
        JsonValue::from(digest_hex.to_owned()),
    );
    entry.insert(
        "encoded_len".into(),
        JsonValue::from(u64::try_from(encoded_len).unwrap_or(u64::MAX)),
    );
    entry.insert(
        "encoded_path".into(),
        JsonValue::from(index_path_string(root, encoded_path)),
    );
    entry.insert(
        "json_path".into(),
        JsonValue::from(index_path_string(root, json_path)),
    );
    entry.insert(
        "node_cid_hex".into(),
        JsonValue::from(hex::encode(&block.node.node_cid)),
    );
    entry.insert(
        "prev_node_cid_hex".into(),
        tip.as_ref()
            .map(|tip| JsonValue::from(hex::encode(&tip.node_cid)))
            .unwrap_or(JsonValue::Null),
    );
    entry.insert(
        "block_cid_hex".into(),
        JsonValue::from(block_cid_hex.clone()),
    );
    entry.insert(
        "prev_block_cid_hex".into(),
        tip.as_ref()
            .map(|tip| JsonValue::from(hex::encode(&tip.block_cid)))
            .unwrap_or(JsonValue::Null),
    );
    entry.insert(
        "block_path".into(),
        JsonValue::from(index_path_string(root, &block_path)),
    );
    entry.insert("published_at_unix".into(), JsonValue::from(timestamp));
    blocks.push(JsonValue::Object(entry));

    rebuild_runtime_dag_index(root, signer, index, blocks, &head, &head_path, &index_path)
}

fn read_runtime_dag_index(
    root: &Path,
    signer: &GovernanceRuntimeDagSigner,
    index_path: &Path,
) -> Result<JsonMap, GovernancePublishError> {
    match fs::read(index_path) {
        Ok(bytes) => {
            let value: JsonValue = json::from_slice(&bytes).map_err(|err| {
                GovernancePublishError::other(format!(
                    "failed to parse governance runtime DAG index `{}`: {err}",
                    index_path.display()
                ))
            })?;
            let JsonValue::Object(map) = value else {
                return Err(GovernancePublishError::other(
                    "governance runtime DAG index root is not an object",
                ));
            };
            if map.get("schema").and_then(JsonValue::as_str)
                != Some(GOVERNANCE_RUNTIME_DAG_INDEX_SCHEMA)
            {
                return Err(GovernancePublishError::other(
                    "governance runtime DAG index uses an unsupported schema",
                ));
            }
            validate_runtime_dag_signer_fields(&map, signer)?;
            Ok(map)
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            let mut map = JsonMap::new();
            map.insert(
                "schema".into(),
                JsonValue::from(GOVERNANCE_RUNTIME_DAG_INDEX_SCHEMA),
            );
            map.insert(
                "source".into(),
                JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
            );
            map.insert("root".into(), JsonValue::from(root.display().to_string()));
            insert_runtime_dag_signer_fields(&mut map, signer);
            map.insert("blocks".into(), JsonValue::Array(Vec::new()));
            Ok(map)
        }
        Err(err) => Err(err.into()),
    }
}

fn validate_runtime_dag_signer_fields(
    index: &JsonMap,
    signer: &GovernanceRuntimeDagSigner,
) -> Result<(), GovernancePublishError> {
    let expected_peer = signer.publisher_peer_id_hex();
    let expected_public_key = signer.publisher_public_key_hex();
    let peer = index
        .get("publisher_peer_id_hex")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG index is missing `publisher_peer_id_hex`",
            )
        })?;
    if peer != expected_peer {
        return Err(GovernancePublishError::other(
            "governance runtime DAG index publisher peer id does not match configured signer",
        ));
    }
    let public_key = index
        .get("publisher_public_key_hex")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            GovernancePublishError::other(
                "governance runtime DAG index is missing `publisher_public_key_hex`",
            )
        })?;
    if public_key != expected_public_key {
        return Err(GovernancePublishError::other(
            "governance runtime DAG index publisher public key does not match configured signer",
        ));
    }
    Ok(())
}

fn insert_runtime_dag_signer_fields(index: &mut JsonMap, signer: &GovernanceRuntimeDagSigner) {
    index.insert(
        "publisher_peer_id".into(),
        JsonValue::from(String::from_utf8_lossy(&signer.publisher_peer_id).to_string()),
    );
    index.insert(
        "publisher_peer_id_hex".into(),
        JsonValue::from(signer.publisher_peer_id_hex()),
    );
    index.insert(
        "publisher_public_key_hex".into(),
        JsonValue::from(signer.publisher_public_key_hex()),
    );
}

fn runtime_dag_tip_from_entries(
    blocks: &[JsonValue],
) -> Result<Option<RuntimeDagTip>, GovernancePublishError> {
    let Some(last) = blocks.last() else {
        return Ok(None);
    };
    let Some(map) = last.as_object() else {
        return Err(GovernancePublishError::other(
            "governance runtime DAG index block entry is not an object",
        ));
    };
    Ok(Some(RuntimeDagTip {
        sequence: required_runtime_u64(map, "sequence")?,
        block_cid: required_runtime_hex(map, "block_cid_hex")?,
        node_cid: required_runtime_hex(map, "node_cid_hex")?,
    }))
}

fn rebuild_runtime_dag_index(
    root: &Path,
    signer: &GovernanceRuntimeDagSigner,
    mut index: JsonMap,
    mut blocks: Vec<JsonValue>,
    head: &GovernanceDagHeadV1,
    head_path: &Path,
    index_path: &Path,
) -> Result<(), GovernancePublishError> {
    let mut by_encoded_blake3 = JsonMap::new();
    let mut by_payload_kind = JsonMap::new();
    let mut previous_block_cid_hex: Option<String> = None;
    let mut previous_node_cid_hex: Option<String> = None;

    for (position, block) in blocks.iter_mut().enumerate() {
        let Some(block_map) = block.as_object_mut() else {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index block entry is not an object",
            ));
        };
        block_map.insert("position".into(), JsonValue::from(position as u64));
        let sequence = required_runtime_u64(block_map, "sequence")?;
        if sequence != position as u64 {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index sequence does not match block position",
            ));
        }
        let payload_kind = required_runtime_string(block_map, "payload_kind")?;
        append_index_position(&mut by_payload_kind, &payload_kind, position);
        let encoded_blake3 = required_runtime_string(block_map, "encoded_blake3")?;
        append_index_position(&mut by_encoded_blake3, &encoded_blake3, position);
        let block_cid_hex = required_runtime_string(block_map, "block_cid_hex")?;
        let node_cid_hex = required_runtime_string(block_map, "node_cid_hex")?;
        let prev_block_cid_hex = optional_runtime_string(block_map, "prev_block_cid_hex")?;
        let prev_node_cid_hex = optional_runtime_string(block_map, "prev_node_cid_hex")?;
        if prev_block_cid_hex != previous_block_cid_hex
            || prev_node_cid_hex != previous_node_cid_hex
        {
            return Err(GovernancePublishError::other(
                "governance runtime DAG index parent links are inconsistent",
            ));
        }
        previous_block_cid_hex = Some(block_cid_hex);
        previous_node_cid_hex = Some(node_cid_hex);
    }

    index.insert(
        "schema".into(),
        JsonValue::from(GOVERNANCE_RUNTIME_DAG_INDEX_SCHEMA),
    );
    index.insert(
        "source".into(),
        JsonValue::from(GOVERNANCE_DAG_SINK_FILESYSTEM),
    );
    index.insert("root".into(), JsonValue::from(root.display().to_string()));
    index.insert(
        "generated_at".into(),
        JsonValue::from(current_unix_timestamp_seconds()),
    );
    insert_runtime_dag_signer_fields(&mut index, signer);
    index.insert(
        "head_block_cid_hex".into(),
        JsonValue::from(hex::encode(&head.head_block_cid)),
    );
    index.insert(
        "head_generated_at".into(),
        JsonValue::from(head.generated_at),
    );
    index.insert(
        "head_path".into(),
        JsonValue::from(index_path_string(root, head_path)),
    );
    index.insert("block_count".into(), JsonValue::from(head.block_count));
    index.insert(
        "by_encoded_blake3".into(),
        JsonValue::Object(by_encoded_blake3),
    );
    index.insert("by_payload_kind".into(), JsonValue::Object(by_payload_kind));
    index.insert("blocks".into(), JsonValue::Array(blocks));

    let body = json::to_json_pretty(&JsonValue::Object(index)).map_err(|err| {
        GovernancePublishError::other(format!("serialize governance runtime DAG index: {err}"))
    })?;
    write_atomic(index_path, body.as_bytes())?;
    write_digest_sidecar(index_path, body.as_bytes())?;
    record_governance_dag_head_age(head.generated_at);
    Ok(())
}

fn runtime_dag_index_entry_files_exist(root: &Path, entry: &JsonValue) -> bool {
    entry
        .get("block_path")
        .and_then(JsonValue::as_str)
        .and_then(|path| resolve_index_path(root, path).ok())
        .is_some_and(|path| path.is_file())
}

fn runtime_dag_block_path(root: &Path, sequence: u64, block_cid_hex: &str) -> PathBuf {
    root.join(GOVERNANCE_RUNTIME_DAG_DIR)
        .join(GOVERNANCE_RUNTIME_DAG_BLOCKS_DIR)
        .join(format!("{sequence:020}_{block_cid_hex}.to"))
}

fn runtime_dag_head_path(root: &Path) -> PathBuf {
    root.join(GOVERNANCE_RUNTIME_DAG_DIR)
        .join(GOVERNANCE_RUNTIME_DAG_HEAD_FILE)
}

fn empty_governance_ed25519_signature() -> GovernanceLogSignatureV1 {
    GovernanceLogSignatureV1 {
        algorithm: GovernanceSignatureAlgorithm::Ed25519,
        public_key: Vec::new(),
        signature: Vec::new(),
    }
}

fn required_runtime_string(map: &JsonMap, field: &str) -> Result<String, GovernancePublishError> {
    map.get(field)
        .and_then(JsonValue::as_str)
        .map(str::to_owned)
        .ok_or_else(|| {
            GovernancePublishError::other(format!(
                "governance runtime DAG index entry is missing `{field}`"
            ))
        })
}

fn optional_runtime_string(
    map: &JsonMap,
    field: &str,
) -> Result<Option<String>, GovernancePublishError> {
    match map.get(field) {
        Some(JsonValue::Null) | None => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_owned()))
            .ok_or_else(|| {
                GovernancePublishError::other(format!(
                    "governance runtime DAG index entry field `{field}` is not a string or null"
                ))
            }),
    }
}

fn required_runtime_u64(map: &JsonMap, field: &str) -> Result<u64, GovernancePublishError> {
    map.get(field).and_then(JsonValue::as_u64).ok_or_else(|| {
        GovernancePublishError::other(format!(
            "governance runtime DAG index entry is missing `{field}`"
        ))
    })
}

fn required_runtime_hex(map: &JsonMap, field: &str) -> Result<Vec<u8>, GovernancePublishError> {
    let value = required_runtime_string(map, field)?;
    if value.is_empty() {
        return Err(GovernancePublishError::other(format!(
            "governance runtime DAG index entry field `{field}` is empty"
        )));
    }
    hex::decode(&value).map_err(|err| {
        GovernancePublishError::other(format!(
            "governance runtime DAG index entry field `{field}` is not hex: {err}"
        ))
    })
}

fn record_governance_dag_publish_result(
    payload_kind: &str,
    result: &Result<(), GovernancePublishError>,
    encoded_len: usize,
) {
    let Some(metrics) = iroha_telemetry::metrics::global() else {
        return;
    };
    let result_label = if result.is_ok() { "success" } else { "failure" };
    let encoded_len = u64::try_from(encoded_len).unwrap_or(u64::MAX);
    metrics.record_sorafs_governance_dag_publish(
        payload_kind,
        result_label,
        GOVERNANCE_DAG_SINK_FILESYSTEM,
        encoded_len,
        current_unix_timestamp_seconds(),
    );
}

fn record_governance_dag_backlog(pending_count: u64) {
    let Some(metrics) = iroha_telemetry::metrics::global() else {
        return;
    };
    metrics.set_sorafs_governance_dag_backlog(GOVERNANCE_DAG_SINK_FILESYSTEM, pending_count);
}

fn record_governance_dag_head_age_from_index(index: &JsonMap) {
    if let Some(generated_at) = governance_dag_head_generated_at_from_index(index) {
        record_governance_dag_head_age(generated_at);
    }
}

fn governance_dag_head_generated_at_from_index(index: &JsonMap) -> Option<u64> {
    index
        .get("head_generated_at")
        .and_then(JsonValue::as_u64)
        .or_else(|| index.get("generated_at").and_then(JsonValue::as_u64))
}

fn record_governance_dag_head_age(generated_at: u64) {
    let Some(metrics) = iroha_telemetry::metrics::global() else {
        return;
    };
    metrics.set_sorafs_governance_dag_head_age_seconds(
        GOVERNANCE_DAG_SINK_FILESYSTEM,
        governance_dag_head_age_seconds(generated_at, current_unix_timestamp_seconds()),
    );
}

fn governance_dag_head_age_seconds(generated_at: u64, now: u64) -> u64 {
    now.saturating_sub(generated_at)
}

impl GovernancePublisher for FilesystemGovernancePublisher {
    fn publish_deal_settlement(
        &self,
        settlement: &DealSettlementV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
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
            let mut labels = JsonMap::new();
            labels.insert(
                "deal_id".into(),
                JsonValue::from(settlement.deal_id.encode_hex::<String>()),
            );
            labels.insert(
                "provider_id".into(),
                JsonValue::from(settlement.ledger.provider_id.encode_hex::<String>()),
            );
            labels.insert(
                "client_id".into(),
                JsonValue::from(settlement.ledger.client_id.encode_hex::<String>()),
            );
            labels.insert(
                "status".into(),
                JsonValue::from(status_label(settlement.status)),
            );
            labels.insert("settled_at".into(), JsonValue::from(settlement.settled_at));
            self.record_publish_index(
                "deal_settlement",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;
            self.record_runtime_signed_payload(
                "deal_settlement",
                GovernanceLogPayloadV1::DealSettlement(settlement.clone()),
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;

            Ok(())
        })();
        record_governance_dag_publish_result("deal_settlement", &result, encoded.len());
        result
    }

    fn publish_repair_audit_event(
        &self,
        event: &RepairAuditEventV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
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
            let mut labels = JsonMap::new();
            labels.insert(
                "ticket_id".into(),
                JsonValue::from(event.payload.ticket_id.0.clone()),
            );
            labels.insert(
                "manifest".into(),
                JsonValue::from(hex::encode(event.payload.manifest_digest)),
            );
            labels.insert(
                "provider".into(),
                JsonValue::from(hex::encode(event.payload.provider_id)),
            );
            labels.insert(
                "status".into(),
                JsonValue::from(repair_status_label(event.payload.status)),
            );
            labels.insert("sequence".into(), JsonValue::from(event.header.sequence));
            labels.insert(
                "occurred_at_unix".into(),
                JsonValue::from(event.header.occurred_at_unix),
            );
            self.record_publish_index(
                "repair_audit",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;

            Ok(())
        })();
        record_governance_dag_publish_result("repair_audit", &result, encoded.len());
        result
    }

    fn publish_repair_slash_proposal(
        &self,
        proposal: &RepairSlashProposalV1,
        encoded: &[u8],
        stage: RepairSlashStage,
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
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
            let mut labels = JsonMap::new();
            labels.insert(
                "ticket_id".into(),
                JsonValue::from(proposal.ticket_id.0.clone()),
            );
            labels.insert(
                "manifest".into(),
                JsonValue::from(hex::encode(proposal.manifest_digest)),
            );
            labels.insert(
                "provider".into(),
                JsonValue::from(hex::encode(proposal.provider_id)),
            );
            labels.insert("stage".into(), JsonValue::from(stage.as_str()));
            labels.insert(
                "submitted_at_unix".into(),
                JsonValue::from(proposal.submitted_at_unix),
            );
            self.record_publish_index(
                "repair_slash",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;

            Ok(())
        })();
        record_governance_dag_publish_result("repair_slash", &result, encoded.len());
        result
    }

    fn publish_gc_audit_event(
        &self,
        event: &GcAuditEventV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
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
            let mut labels = JsonMap::new();
            labels.insert(
                "manifest".into(),
                JsonValue::from(hex::encode(event.payload.manifest_digest)),
            );
            labels.insert(
                "provider".into(),
                JsonValue::from(hex::encode(event.payload.provider_id)),
            );
            labels.insert(
                "reason".into(),
                JsonValue::from(event.payload.reason.clone()),
            );
            labels.insert("sequence".into(), JsonValue::from(event.header.sequence));
            labels.insert(
                "evicted_at_unix".into(),
                JsonValue::from(event.payload.evicted_at_unix),
            );
            self.record_publish_index(
                "gc_audit",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;

            Ok(())
        })();
        record_governance_dag_publish_result("gc_audit", &result, encoded.len());
        result
    }

    fn publish_reconciliation_report(
        &self,
        report: &SorafsReconciliationReportV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
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
                GovernancePublishError::other(format!(
                    "serialize reconciliation report json: {err}"
                ))
            })?;

            let json_path = base_path.with_extension("json");
            write_atomic(&json_path, json_body.as_bytes())?;
            write_digest_sidecar(&json_path, json_body.as_bytes())?;
            let mut labels = JsonMap::new();
            labels.insert(
                "provider".into(),
                JsonValue::from(hex::encode(report.provider_id)),
            );
            labels.insert(
                "generated_at_unix".into(),
                JsonValue::from(report.generated_at_unix),
            );
            labels.insert(
                "divergence_count".into(),
                JsonValue::from(report.divergence_count as u64),
            );
            self.record_publish_index(
                "reconciliation",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;

            Ok(())
        })();
        record_governance_dag_publish_result("reconciliation", &result, encoded.len());
        result
    }

    fn publish_reputation_snapshot(
        &self,
        snapshot: &ReputationSnapshotV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let result = (|| -> Result<(), GovernancePublishError> {
            let digest = blake3::hash(encoded);
            let digest_hex = digest.to_hex().to_string();
            let base_path = self.reputation_snapshot_path(snapshot, &digest_hex);

            let encoded_path = base_path.with_extension("to");
            write_atomic(&encoded_path, encoded)?;
            write_digest_sidecar(&encoded_path, encoded)?;

            let json_body = reputation_snapshot_json(snapshot, encoded, &digest_hex)?;
            let json_path = base_path.with_extension("json");
            write_atomic(&json_path, json_body.as_bytes())?;
            write_digest_sidecar(&json_path, json_body.as_bytes())?;

            let latest_path = self.reputation_root().join("latest");
            let latest_encoded_path = latest_path.with_extension("to");
            write_atomic(&latest_encoded_path, encoded)?;
            write_digest_sidecar(&latest_encoded_path, encoded)?;
            let latest_json_path = latest_path.with_extension("json");
            write_atomic(&latest_json_path, json_body.as_bytes())?;
            write_digest_sidecar(&latest_json_path, json_body.as_bytes())?;
            let mut labels = JsonMap::new();
            labels.insert(
                "snapshot_id_hex".into(),
                JsonValue::from(hex::encode(snapshot.snapshot_id)),
            );
            labels.insert(
                "generated_at_unix".into(),
                JsonValue::from(snapshot.generated_at_unix),
            );
            labels.insert(
                "provider_count".into(),
                JsonValue::from(snapshot.providers.len() as u64),
            );
            labels.insert(
                "merkle_root_hex".into(),
                JsonValue::from(hex::encode(snapshot.merkle_root)),
            );
            self.record_publish_index(
                "reputation_snapshot",
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
                labels,
            )?;
            self.record_runtime_signed_payload(
                "reputation_snapshot",
                GovernanceLogPayloadV1::ReputationSnapshot(snapshot.clone()),
                &encoded_path,
                &json_path,
                &digest_hex,
                encoded.len(),
            )?;

            Ok(())
        })();
        record_governance_dag_publish_result("reputation_snapshot", &result, encoded.len());
        result
    }
}

fn reputation_snapshot_json(
    snapshot: &ReputationSnapshotV1,
    encoded: &[u8],
    digest_hex: &str,
) -> Result<String, GovernancePublishError> {
    let mut payload = JsonMap::new();
    payload.insert(
        "snapshot".into(),
        json::to_value(snapshot).map_err(|err| {
            GovernancePublishError::other(format!("serialize reputation snapshot: {err}"))
        })?,
    );

    let mut metadata = JsonMap::new();
    metadata.insert(
        "snapshot_id_hex".into(),
        JsonValue::from(hex::encode(snapshot.snapshot_id)),
    );
    metadata.insert(
        "generated_at_unix".into(),
        JsonValue::from(snapshot.generated_at_unix),
    );
    metadata.insert(
        "provider_count".into(),
        JsonValue::from(snapshot.providers.len() as u64),
    );
    metadata.insert(
        "merkle_root_hex".into(),
        JsonValue::from(hex::encode(snapshot.merkle_root)),
    );
    metadata.insert(
        "encoded_blake3".into(),
        JsonValue::from(digest_hex.to_string()),
    );
    metadata.insert("encoded_len".into(), JsonValue::from(encoded.len() as u64));
    metadata.insert(
        "encoded_base64".into(),
        JsonValue::from(BASE64_STANDARD.encode(encoded)),
    );
    payload.insert("metadata".into(), JsonValue::Object(metadata));

    json::to_json_pretty(&JsonValue::Object(payload)).map_err(|err| {
        GovernancePublishError::other(format!("serialize reputation snapshot json: {err}"))
    })
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

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
    use sorafs_manifest::{
        GovernanceDagBlockV1, GovernanceDagHeadV1, GovernanceLogPayloadV1,
        REPUTATION_PROVIDER_INPUT_VERSION_V1, REPUTATION_PROVIDER_METRICS_VERSION_V1,
        ReputationProviderInputV1, ReputationProviderMetricsV1, ReputationReserveStageV1,
        ReputationWeightsV1, SORAFS_RECONCILIATION_REPORT_VERSION_V1, SorafsReconciliationReportV1,
        build_reputation_snapshot, validate_governance_dag_head_against_chain_v1,
    };
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

    fn sample_reputation_snapshot() -> (ReputationSnapshotV1, Vec<u8>) {
        let metrics = ReputationProviderMetricsV1 {
            version: REPUTATION_PROVIDER_METRICS_VERSION_V1,
            por_success_bps: 9_800,
            pdp_success_bps: 9_700,
            potr_success_bps: 9_600,
            latency_health_bps: 9_000,
            dispute_rate_bps: 100,
            token_violation_rate_bps: 50,
            repair_breach_rate_bps: 0,
        };
        let input = ReputationProviderInputV1 {
            version: REPUTATION_PROVIDER_INPUT_VERSION_V1,
            provider_id: "provider-a".to_string(),
            metrics,
            reserve_stage: ReputationReserveStageV1::Active,
            previous_score_bps: None,
            active_dispute: false,
            slashing_event: false,
        };
        let snapshot = build_reputation_snapshot(
            [0x42; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &[input],
            None,
        )
        .expect("reputation snapshot");
        let encoded = norito::to_bytes(&snapshot).expect("encode reputation snapshot");
        (snapshot, encoded)
    }

    #[test]
    fn governance_car_queue_pending_count_tracks_unassembled_segments() {
        let mut assembled = JsonMap::new();
        assembled.insert("status".into(), JsonValue::from("assembled"));
        let mut pending = JsonMap::new();
        pending.insert("status".into(), JsonValue::from("pending"));
        let malformed = JsonValue::from("not-a-segment");

        assert_eq!(
            governance_car_queue_pending_count(&[
                JsonValue::Object(assembled),
                JsonValue::Object(pending),
                malformed,
            ]),
            2
        );
    }

    #[test]
    fn governance_dag_head_age_seconds_saturates_for_future_heads() {
        assert_eq!(
            governance_dag_head_age_seconds(1_800_000_000, 1_800_000_045),
            45
        );
        assert_eq!(
            governance_dag_head_age_seconds(1_800_000_100, 1_800_000_045),
            0
        );
    }

    #[test]
    fn governance_dag_head_generated_at_from_index_prefers_head_timestamp() {
        let mut index = JsonMap::new();
        assert_eq!(governance_dag_head_generated_at_from_index(&index), None);

        index.insert("generated_at".into(), JsonValue::from(1_800_000_000u64));
        assert_eq!(
            governance_dag_head_generated_at_from_index(&index),
            Some(1_800_000_000)
        );

        index.insert(
            "head_generated_at".into(),
            JsonValue::from(1_800_000_045u64),
        );
        assert_eq!(
            governance_dag_head_generated_at_from_index(&index),
            Some(1_800_000_045)
        );
    }

    fn signed_runtime_publisher(root: &Path) -> FilesystemGovernancePublisher {
        let key_path = root.join("governance-dag-ed25519.key");
        fs::write(&key_path, hex::encode([0x31; 32])).expect("write runtime DAG key");
        FilesystemGovernancePublisher::try_new(root.to_path_buf())
            .expect("publisher")
            .with_runtime_dag_signer(b"12D3KooWRuntimeDagPublisher".to_vec(), &key_path)
            .expect("runtime DAG signer")
    }

    fn runtime_index(root: &Path) -> JsonValue {
        let bytes =
            fs::read(root.join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE)).expect("runtime index exists");
        norito::json::from_slice(&bytes).expect("runtime index parses")
    }

    fn runtime_blocks_from_index(root: &Path, index: &JsonValue) -> Vec<GovernanceDagBlockV1> {
        index
            .get("blocks")
            .and_then(JsonValue::as_array)
            .expect("runtime blocks")
            .iter()
            .map(|entry| {
                let block_path = entry
                    .get("block_path")
                    .and_then(JsonValue::as_str)
                    .expect("block path");
                let block_path = resolve_index_path(root, block_path).expect("resolve block path");
                let bytes = fs::read(block_path).expect("read runtime block");
                norito::decode_from_bytes(&bytes).expect("decode runtime block")
            })
            .collect()
    }

    #[test]
    fn filesystem_publisher_appends_signed_runtime_dag_for_supported_payloads() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        let (settlement, encoded) = sample_settlement();

        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("publish settlement into runtime DAG");
        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("duplicate publish is idempotent");
        let index = runtime_index(temp.path());
        assert_eq!(
            index.get("block_count").and_then(JsonValue::as_u64),
            Some(1)
        );
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("deal_settlement"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );

        let (snapshot, snapshot_encoded) = sample_reputation_snapshot();
        publisher
            .publish_reputation_snapshot(&snapshot, &snapshot_encoded)
            .expect("publish reputation snapshot into runtime DAG");

        let index = runtime_index(temp.path());
        assert_eq!(
            index.get("block_count").and_then(JsonValue::as_u64),
            Some(2)
        );
        assert_eq!(
            index
                .get("by_payload_kind")
                .and_then(|value| value.get("reputation_snapshot"))
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(1)
        );

        let head_bytes = fs::read(runtime_dag_head_path(temp.path())).expect("read runtime head");
        let head: GovernanceDagHeadV1 =
            norito::decode_from_bytes(&head_bytes).expect("decode runtime head");
        let blocks = runtime_blocks_from_index(temp.path(), &index);
        validate_governance_dag_head_against_chain_v1(&head, &blocks)
            .expect("runtime head validates against signed blocks");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].sequence, 0);
        assert_eq!(blocks[1].sequence, 1);
        assert_eq!(blocks[1].prev_block_cid, Some(blocks[0].block_cid.clone()));
        assert_eq!(
            blocks[1].node.prev_cid,
            Some(blocks[0].node.node_cid.clone())
        );
        match &blocks[0].node.payload {
            GovernanceLogPayloadV1::DealSettlement(value) => {
                assert_eq!(value.deal_id, settlement.deal_id);
            }
            other => panic!("unexpected first runtime DAG payload: {other:?}"),
        }
        match &blocks[1].node.payload {
            GovernanceLogPayloadV1::ReputationSnapshot(value) => {
                assert_eq!(value.snapshot_id, snapshot.snapshot_id);
            }
            other => panic!("unexpected second runtime DAG payload: {other:?}"),
        }
    }

    #[test]
    fn filesystem_publisher_rejects_malformed_runtime_dag_index() {
        let temp = tempdir().expect("tempdir");
        let publisher = signed_runtime_publisher(temp.path());
        fs::write(
            temp.path().join(GOVERNANCE_RUNTIME_DAG_INDEX_FILE),
            br#"{"schema":"sorafs.governance_dag.wrong","blocks":[]}"#,
        )
        .expect("write bad runtime index");
        let (settlement, encoded) = sample_settlement();

        let err = publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect_err("malformed runtime DAG index must fail closed");
        assert!(
            err.to_string().contains("unsupported schema"),
            "unexpected error: {err}"
        );
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

        let index_path = temp.path().join(GOVERNANCE_PUBLISH_INDEX_FILE);
        let index_bytes = fs::read(&index_path).expect("read publish index");
        let index: JsonValue = norito::json::from_slice(&index_bytes).expect("index json");
        assert_eq!(
            index.get("schema").and_then(JsonValue::as_str),
            Some(GOVERNANCE_PUBLISH_INDEX_SCHEMA)
        );
        assert_eq!(
            index.get("entry_count").and_then(JsonValue::as_u64),
            Some(1)
        );
        assert_eq!(
            index
                .get("payload_kind_counts")
                .and_then(JsonValue::as_object)
                .and_then(|counts| counts.get("deal_settlement"))
                .and_then(JsonValue::as_u64),
            Some(1)
        );
        let digest_hex = blake3::hash(&encoded).to_hex().to_string();
        let digest_positions = index
            .get("by_encoded_blake3")
            .and_then(JsonValue::as_object)
            .and_then(|map| map.get(digest_hex.as_str()))
            .and_then(JsonValue::as_array)
            .expect("digest lookup");
        assert_eq!(digest_positions.len(), 1);
        assert_eq!(digest_positions[0].as_u64(), Some(0));
        let kind_positions = index
            .get("by_payload_kind")
            .and_then(JsonValue::as_object)
            .and_then(|map| map.get("deal_settlement"))
            .and_then(JsonValue::as_array)
            .expect("kind lookup");
        assert_eq!(kind_positions[0].as_u64(), Some(0));
        let entry = index
            .get("entries")
            .and_then(JsonValue::as_array)
            .and_then(|entries| entries.first())
            .and_then(JsonValue::as_object)
            .expect("first index entry");
        assert_eq!(
            entry.get("payload_kind").and_then(JsonValue::as_str),
            Some("deal_settlement")
        );
        assert_eq!(
            entry.get("encoded_path").and_then(JsonValue::as_str),
            Some(index_path_string(temp.path(), encoded_path).as_str())
        );
        assert_eq!(
            entry
                .get("labels")
                .and_then(JsonValue::as_object)
                .and_then(|labels| labels.get("status"))
                .and_then(JsonValue::as_str),
            Some("completed")
        );
        let index_digest_path = index_path.with_extension("json.blake3");
        let index_digest = fs::read_to_string(index_digest_path).expect("read index digest");
        assert_eq!(
            index_digest.trim(),
            blake3::hash(&index_bytes).to_hex().as_str()
        );

        let queue_path = temp.path().join(GOVERNANCE_CAR_QUEUE_FILE);
        let queue_bytes = fs::read(&queue_path).expect("read CAR queue");
        let queue: JsonValue = norito::json::from_slice(&queue_bytes).expect("queue json");
        assert_eq!(
            queue.get("schema").and_then(JsonValue::as_str),
            Some(GOVERNANCE_CAR_QUEUE_SCHEMA)
        );
        assert_eq!(
            queue.get("segment_count").and_then(JsonValue::as_u64),
            Some(1)
        );
        assert_eq!(
            queue.get("assembled_count").and_then(JsonValue::as_u64),
            Some(1)
        );
        let queue_digest_path = queue_path.with_extension("json.blake3");
        let queue_digest = fs::read_to_string(queue_digest_path).expect("read queue digest");
        assert_eq!(
            queue_digest.trim(),
            blake3::hash(&queue_bytes).to_hex().as_str()
        );
        let segment = queue
            .get("segments")
            .and_then(JsonValue::as_array)
            .and_then(|segments| segments.first())
            .and_then(JsonValue::as_object)
            .expect("first CAR segment");
        assert_eq!(
            segment.get("schema").and_then(JsonValue::as_str),
            Some(GOVERNANCE_CAR_SEGMENT_SCHEMA)
        );
        assert_eq!(
            segment.get("status").and_then(JsonValue::as_str),
            Some("assembled")
        );
        assert_eq!(
            segment
                .get("source_publish_index_position")
                .and_then(JsonValue::as_u64),
            Some(0)
        );
        assert_eq!(
            segment.get("encoded_blake3").and_then(JsonValue::as_str),
            Some(digest_hex.as_str())
        );
        let car_path = resolve_index_path(
            temp.path(),
            segment
                .get("car_path")
                .and_then(JsonValue::as_str)
                .expect("car path"),
        )
        .expect("resolve car path");
        let car_bytes = fs::read(&car_path).expect("read CAR segment");
        assert_eq!(
            segment.get("car_size").and_then(JsonValue::as_u64),
            Some(car_bytes.len() as u64)
        );
        assert_eq!(
            segment
                .get("car_archive_blake3")
                .and_then(JsonValue::as_str),
            Some(blake3::hash(&car_bytes).to_hex().as_str())
        );
        let car_digest =
            fs::read_to_string(digest_sidecar_path_for(&car_path)).expect("read car sidecar");
        assert_eq!(
            car_digest.trim(),
            blake3::hash(&car_bytes).to_hex().as_str()
        );

        let plan_path = resolve_index_path(
            temp.path(),
            segment
                .get("plan_path")
                .and_then(JsonValue::as_str)
                .expect("plan path"),
        )
        .expect("resolve plan path");
        let plan_bytes = fs::read(&plan_path).expect("read CAR plan");
        let plan: JsonValue = norito::json::from_slice(&plan_bytes).expect("plan json");
        assert_eq!(
            plan.get("schema").and_then(JsonValue::as_str),
            Some(GOVERNANCE_CAR_PLAN_SCHEMA)
        );
        assert_eq!(
            plan.get("source_publish_index_position")
                .and_then(JsonValue::as_u64),
            Some(0)
        );
        assert_eq!(
            plan.get("files")
                .and_then(JsonValue::as_array)
                .map(Vec::len),
            Some(4)
        );
        assert!(
            plan.get("chunks")
                .and_then(JsonValue::as_array)
                .is_some_and(|chunks| !chunks.is_empty()),
            "CAR plan should expose deterministic chunks"
        );
        let manifest_path = resolve_index_path(
            temp.path(),
            segment
                .get("manifest_path")
                .and_then(JsonValue::as_str)
                .expect("manifest path"),
        )
        .expect("resolve segment manifest path");
        let manifest_bytes = fs::read(&manifest_path).expect("read segment manifest");
        let manifest: JsonValue =
            norito::json::from_slice(&manifest_bytes).expect("segment manifest json");
        assert_eq!(
            manifest.get("schema").and_then(JsonValue::as_str),
            Some(GOVERNANCE_CAR_SEGMENT_SCHEMA)
        );

        publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect("republish same settlement");
        let index_bytes = fs::read(&index_path).expect("read republished index");
        let index: JsonValue = norito::json::from_slice(&index_bytes).expect("index json");
        assert_eq!(
            index.get("entry_count").and_then(JsonValue::as_u64),
            Some(1),
            "republishing the same artifact must not duplicate the index entry"
        );
        let queue_bytes = fs::read(&queue_path).expect("read republished queue");
        let queue: JsonValue = norito::json::from_slice(&queue_bytes).expect("queue json");
        assert_eq!(
            queue.get("segment_count").and_then(JsonValue::as_u64),
            Some(1),
            "republishing the same artifact must not duplicate the CAR queue segment"
        );
    }

    #[test]
    fn filesystem_publisher_rejects_malformed_car_queue() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (settlement, encoded) = sample_settlement();
        fs::write(
            temp.path().join(GOVERNANCE_CAR_QUEUE_FILE),
            br#"{"schema":"wrong","segments":[]}"#,
        )
        .expect("write malformed queue");

        let err = publisher
            .publish_deal_settlement(&settlement, &encoded)
            .expect_err("malformed CAR queue must fail closed");
        assert!(
            err.to_string()
                .contains("governance CAR queue uses an unsupported schema"),
            "unexpected error: {err}"
        );
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
            actor: Some("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into()),
            message: Some("queued".into()),
        };
        let digest = iroha_crypto::Hash::new(payload.encode());
        let header = SorafsAuditHeaderV1 {
            sequence: 42,
            occurred_at_unix: payload.occurred_at_unix,
            signer: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
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
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
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

    #[test]
    fn filesystem_publisher_writes_reputation_snapshot_files() {
        let temp = tempdir().expect("tempdir");
        let publisher =
            FilesystemGovernancePublisher::try_new(temp.path().to_path_buf()).expect("publisher");
        let (snapshot, encoded) = sample_reputation_snapshot();

        publisher
            .publish_reputation_snapshot(&snapshot, &encoded)
            .expect("publish reputation snapshot");

        let snapshot_hex = hex::encode(snapshot.snapshot_id);
        let dir = temp
            .path()
            .join("reputation")
            .join("snapshots")
            .join(&snapshot_hex);
        let entries = fs::read_dir(&dir)
            .expect("snapshot directory exists")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 4, "expected encoded + json + digests");

        let latest_to = temp.path().join("reputation").join("latest.to");
        assert_eq!(
            fs::read(&latest_to).expect("read latest reputation snapshot"),
            encoded,
            "latest pointer must contain canonical Norito bytes"
        );

        let latest_json = temp.path().join("reputation").join("latest.json");
        let json_bytes = fs::read(latest_json).expect("read latest reputation json");
        let value: JsonValue = norito::json::from_slice(&json_bytes).expect("json should parse");
        let metadata = value
            .get("metadata")
            .and_then(JsonValue::as_object)
            .expect("metadata");
        assert_eq!(
            metadata.get("snapshot_id_hex").and_then(JsonValue::as_str),
            Some(snapshot_hex.as_str())
        );
        assert_eq!(
            metadata.get("provider_count").and_then(JsonValue::as_u64),
            Some(snapshot.providers.len() as u64)
        );
    }
}
