use std::{
    collections::{BTreeSet, HashMap},
    error::Error,
    fmt::Write,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use arrow_array::{
    ArrayRef, BooleanArray, Float64Array, RecordBatch, StringArray, UInt32Array, UInt64Array,
};
use arrow_schema::{DataType, Field, Schema};
use hex::decode;
use iroha_data_model::{
    block::consensus::{
        LaneBlockCommitment, LaneLiquidityProfile, LaneSettlementReceipt, LaneSwapMetadata,
        LaneVolatilityClass,
    },
    nexus::{DataSpaceId, LaneCompliancePolicy, LaneId},
};
use iroha_telemetry::metrics::Status;
use norito::{
    core::NoritoDeserialize as _,
    derive::{JsonDeserialize, JsonSerialize},
    json,
    json::{self as serde_json, Value as JsonValue},
};
use parquet::{arrow::ArrowWriter, basic::Compression, file::properties::WriterProperties};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

pub fn write_lane_commitment_fixtures(output: &Path) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(output)?;

    for fixture in sample_commitments() {
        let rendered = canonical_json(&fixture.payload)?;
        let json_path = output.join(format!("{}.json", fixture.file_stem));
        fs::write(&json_path, rendered)?;

        let to_path = output.join(format!("{}.to", fixture.file_stem));
        let bytes = norito::to_bytes(&fixture.payload)?;
        fs::write(&to_path, bytes)?;
    }

    Ok(())
}

pub fn verify_lane_commitment_fixtures(dir: &Path) -> Result<(), Box<dyn Error>> {
    let fixtures = sample_commitments();
    let mut expected_entries = BTreeSet::new();
    for fixture in &fixtures {
        expected_entries.insert(format!("{}.json", fixture.file_stem));
        expected_entries.insert(format!("{}.to", fixture.file_stem));
    }

    for fixture in fixtures {
        let json_path = dir.join(format!("{}.json", fixture.file_stem));
        if !json_path.is_file() {
            return Err(format!("missing lane commitment JSON {:?}", json_path).into());
        }
        let raw = fs::read_to_string(&json_path)?;
        let parsed: LaneBlockCommitment = json::from_str(&raw)?;
        if parsed != fixture.payload {
            return Err(format!(
                "lane commitment JSON {:?} does not match the generated payload",
                json_path
            )
            .into());
        }
        let canonical = canonical_json(&fixture.payload)?;
        if raw != canonical {
            return Err(format!(
                "lane commitment JSON {:?} is not canonical; run `cargo xtask nexus-fixtures`",
                json_path
            )
            .into());
        }

        let to_path = dir.join(format!("{}.to", fixture.file_stem));
        if !to_path.is_file() {
            return Err(format!("missing lane commitment Norito bytes {:?}", to_path).into());
        }
        let bytes = fs::read(&to_path)?;
        let decoded = norito::from_bytes::<LaneBlockCommitment>(&bytes)
            .and_then(LaneBlockCommitment::try_deserialize)
            .map_err(|err| format!("failed to deserialize {:?}: {err}", to_path))?;
        if decoded != fixture.payload {
            return Err(format!(
                "lane commitment Norito bytes {:?} do not match the generated payload",
                to_path
            )
            .into());
        }
        let expected_bytes = norito::to_bytes(&fixture.payload)?;
        if bytes != expected_bytes {
            return Err(format!(
                "lane commitment Norito bytes {:?} are not canonical; rerun generator",
                to_path
            )
            .into());
        }
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(file_name) = path.file_name().and_then(|name| name.to_str())
            && expected_entries.contains(file_name)
        {
            continue;
        }
        return Err(format!(
            "unexpected lane commitment artefact {:?}; delete it or extend the generator",
            path
        )
        .into());
    }

    Ok(())
}

fn load_lane_compliance_map(
    path: &Path,
) -> Result<HashMap<u32, LaneComplianceEvidence>, Box<dyn Error>> {
    let raw = fs::read_to_string(path).map_err(|err| {
        format!(
            "failed to read lane compliance evidence {}: {err}",
            path.display()
        )
    })?;
    let file: LaneComplianceEvidenceFile = serde_json::from_str(&raw).map_err(|err| {
        format!(
            "failed to parse lane compliance evidence {}: {err}",
            path.display()
        )
    })?;
    let mut map = HashMap::new();
    for record in file.lanes {
        let serialized_policy = serde_json::to_string(&record.policy).map_err(|err| {
            format!(
                "failed to serialize lane compliance policy for lane {}: {err}",
                record.lane_id
            )
        })?;
        let policy: LaneCompliancePolicy = json::from_str(&serialized_policy).map_err(|err| {
            format!(
                "failed to decode lane compliance policy for lane {}: {err}",
                record.lane_id
            )
        })?;
        let policy_lane = policy.lane_id.as_u32();
        if policy_lane != record.lane_id {
            return Err(format!(
                "lane compliance record lists lane_id {} but policy targets lane_id {}",
                record.lane_id, policy_lane
            )
            .into());
        }
        if map
            .insert(
                record.lane_id,
                LaneComplianceEvidence {
                    policy: record.policy,
                    reviewer_signatures: record.reviewer_signatures,
                    metrics_snapshot: record.metrics_snapshot,
                    audit_log: record.audit_log,
                },
            )
            .is_some()
        {
            return Err(format!(
                "lane compliance evidence contains duplicate entry for lane {policy_lane}",
            )
            .into());
        }
    }
    Ok(map)
}

fn canonical_json(commitment: &LaneBlockCommitment) -> Result<String, Box<dyn Error>> {
    let value = json::to_value(commitment)?;
    Ok(format!("{}\n", json::to_string_pretty(&value)?))
}

struct CommitmentFixture {
    file_stem: &'static str,
    payload: LaneBlockCommitment,
}

fn sample_commitments() -> Vec<CommitmentFixture> {
    vec![
        CommitmentFixture {
            file_stem: "default_public_lane_commitment",
            payload: LaneBlockCommitment {
                block_height: 8_642,
                lane_id: LaneId::new(1),
                dataspace_id: DataSpaceId::new(7),
                tx_count: 2,
                total_local_micro: 7_500_000,
                total_xor_due_micro: 3_050_000,
                total_xor_after_haircut_micro: 3_000_000,
                total_xor_variance_micro: 50_000,
                swap_metadata: Some(LaneSwapMetadata {
                    epsilon_bps: 25,
                    twap_window_seconds: 60,
                    liquidity_profile: LaneLiquidityProfile::Tier1,
                    twap_local_per_xor: "8123.445500".to_string(),
                    volatility_class: LaneVolatilityClass::Stable,
                }),
                receipts: vec![
                    receipt(
                        "4f25818e98f7b549a21ceda9a1f3812d95d64c83f7d02c361e13caf113e53344",
                        4_000_000,
                        1_620_000,
                        1_600_000,
                        1_726_296_400_000,
                    ),
                    receipt(
                        "ab56be456758d5be8d3d24ae7ef44c6a0ca1cf4a788ad18cf3b987fe9954f0d2",
                        3_500_000,
                        1_430_000,
                        1_400_000,
                        1_726_296_401_200,
                    ),
                ],
            },
        },
        CommitmentFixture {
            file_stem: "cbdc_private_lane_commitment",
            payload: LaneBlockCommitment {
                block_height: 91_234,
                lane_id: LaneId::new(12),
                dataspace_id: DataSpaceId::new(24),
                tx_count: 3,
                total_local_micro: 9_300_000,
                total_xor_due_micro: 4_200_000,
                total_xor_after_haircut_micro: 4_050_000,
                total_xor_variance_micro: 150_000,
                swap_metadata: Some(LaneSwapMetadata {
                    epsilon_bps: 120,
                    twap_window_seconds: 300,
                    liquidity_profile: LaneLiquidityProfile::Tier3,
                    twap_local_per_xor: "1.245600".to_string(),
                    volatility_class: LaneVolatilityClass::Dislocated,
                }),
                receipts: vec![
                    receipt(
                        "beadf1f4a09fd303cc6971f2f58d7f2c1eca1aa1a5d2cda7088fcfd97994cb8a",
                        3_300_000,
                        1_600_000,
                        1_550_000,
                        1_726_297_000_500,
                    ),
                    receipt(
                        "d74fefc1c3f216e8844141493dfd9e4fb3c947ff9b35331a37c6ae16a5f97028",
                        2_800_000,
                        1_300_000,
                        1_250_000,
                        1_726_297_001_250,
                    ),
                    receipt(
                        "cedf9cb93f1b8f52a08ff19793b0ce6049db0a89c9a6ec7fd65dd8f5ecd0f92b",
                        3_200_000,
                        1_300_000,
                        1_250_000,
                        1_726_297_001_900,
                    ),
                ],
            },
        },
    ]
}

#[derive(Debug)]
pub struct LaneAuditOptions {
    pub status_path: PathBuf,
    pub json_output: PathBuf,
    pub parquet_output: PathBuf,
    pub markdown_output: PathBuf,
    pub captured_at: Option<String>,
    pub lane_compliance: Option<PathBuf>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct LaneComplianceEvidence {
    pub policy: JsonValue,
    #[norito(default)]
    pub reviewer_signatures: Vec<LaneComplianceReviewerSignature>,
    #[norito(default = "json_null")]
    pub metrics_snapshot: JsonValue,
    #[norito(default)]
    pub audit_log: Vec<JsonValue>,
}

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
pub struct LaneComplianceReviewerSignature {
    pub reviewer: String,
    pub signature_hex: String,
    #[norito(default)]
    pub signed_at: Option<String>,
    #[norito(default)]
    pub digest_hex: Option<String>,
    #[norito(default)]
    pub notes: Option<String>,
}

#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct LaneComplianceEvidenceFile {
    lanes: Vec<LaneComplianceEvidenceRecord>,
}

#[derive(Debug, JsonSerialize, JsonDeserialize)]
struct LaneComplianceEvidenceRecord {
    lane_id: u32,
    policy: JsonValue,
    #[norito(default)]
    reviewer_signatures: Vec<LaneComplianceReviewerSignature>,
    #[norito(default = "json_null")]
    metrics_snapshot: JsonValue,
    #[norito(default)]
    audit_log: Vec<JsonValue>,
}

fn json_null() -> JsonValue {
    JsonValue::Null
}

#[derive(Clone, JsonSerialize)]
struct LaneAuditRow {
    lane_id: u32,
    lane_alias: String,
    dataspace_id: u64,
    dataspace_alias: Option<String>,
    block_height: u64,
    finality_lag_slots: u64,
    teu_capacity: u64,
    teu_committed: u64,
    teu_utilization_pct: f64,
    trigger_level: u64,
    must_serve_truncations: u64,
    scheduler_utilization_pct: u64,
    tx_vertices: u64,
    tx_edges: u64,
    rbc_chunks: u64,
    rbc_bytes_total: u64,
    settlement_backlog_xor_micro: String,
    settlement_backlog_xor: f64,
    governance: Option<String>,
    settlement: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    lane_compliance: Option<LaneComplianceEvidence>,
    manifest_required: bool,
    manifest_ready: bool,
    captured_at: String,
    status_height: u64,
}

impl LaneAuditRow {
    fn compliance_json_string(&self) -> Result<Option<String>, serde_json::Error> {
        self.lane_compliance
            .as_ref()
            .map(serde_json::to_json)
            .transpose()
    }
}

pub fn run_lane_audit(options: &LaneAuditOptions) -> Result<(), Box<dyn Error>> {
    let raw = fs::read_to_string(&options.status_path).map_err(|err| {
        format!(
            "failed to read status blob {}: {err}",
            options.status_path.display()
        )
    })?;
    let status: Status = json::from_str(&raw)?;
    let mut compliance_map = if let Some(path) = &options.lane_compliance {
        load_lane_compliance_map(path)?
    } else {
        HashMap::new()
    };
    let captured_at = options.captured_at.clone().unwrap_or_else(|| {
        OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
    });
    let Status {
        teu_lane_commit,
        blocks: status_height,
        ..
    } = status;
    let mut rows: Vec<LaneAuditRow> = Vec::with_capacity(teu_lane_commit.len());
    for lane in teu_lane_commit {
        let compliance = compliance_map.remove(&lane.lane_id);
        rows.push(LaneAuditRow {
            lane_id: lane.lane_id,
            lane_alias: lane.alias,
            dataspace_id: lane.dataspace_id,
            dataspace_alias: lane.dataspace_alias,
            block_height: lane.block_height,
            finality_lag_slots: lane.finality_lag_slots,
            teu_capacity: lane.capacity,
            teu_committed: lane.committed,
            teu_utilization_pct: compute_teu_utilization_pct(lane.capacity, lane.committed),
            trigger_level: lane.trigger_level,
            must_serve_truncations: lane.must_serve_truncations,
            scheduler_utilization_pct: lane.scheduler_utilization_pct,
            tx_vertices: lane.tx_vertices,
            tx_edges: lane.tx_edges,
            rbc_chunks: lane.rbc_chunks,
            rbc_bytes_total: lane.rbc_bytes_total,
            settlement_backlog_xor_micro: lane.settlement_backlog_xor_micro.to_string(),
            settlement_backlog_xor: micro_xor_to_units(lane.settlement_backlog_xor_micro),
            governance: lane.governance,
            settlement: lane.settlement,
            lane_compliance: compliance,
            manifest_required: lane.manifest_required,
            manifest_ready: lane.manifest_ready,
            captured_at: captured_at.clone(),
            status_height,
        });
    }
    if !compliance_map.is_empty() {
        let mut unknown: Vec<String> = compliance_map.keys().map(|id| id.to_string()).collect();
        unknown.sort();
        return Err(format!(
            "lane compliance evidence includes lanes not present in the status blob: {}",
            unknown.join(", ")
        )
        .into());
    }
    rows.sort_by(|a, b| {
        a.lane_id
            .cmp(&b.lane_id)
            .then(a.dataspace_id.cmp(&b.dataspace_id))
    });
    write_json_rows(&options.json_output, &rows)?;
    write_parquet_rows(&options.parquet_output, &rows)?;
    write_markdown_summary(&options.markdown_output, &rows)?;
    println!(
        "lane audit exported to {}, {}, and {}",
        options.json_output.display(),
        options.parquet_output.display(),
        options.markdown_output.display()
    );
    Ok(())
}

fn write_json_rows(path: &Path, rows: &[LaneAuditRow]) -> Result<(), Box<dyn Error>> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let rendered = serde_json::to_json_pretty(&rows.to_vec())?;
    fs::write(path, rendered)?;
    Ok(())
}

fn write_parquet_rows(path: &Path, rows: &[LaneAuditRow]) -> Result<(), Box<dyn Error>> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let schema = Arc::new(arrow_schema());
    let compliance_serialized: Vec<Option<String>> = rows
        .iter()
        .map(|row| row.compliance_json_string())
        .collect::<Result<_, _>>()
        .map_err(|err| format!("failed to serialize lane compliance evidence: {err}"))?;
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            make_u32_array(rows.iter().map(|row| row.lane_id)),
            make_string_array(rows.iter().map(|row| Some(row.lane_alias.as_str()))),
            make_u64_array(rows.iter().map(|row| row.dataspace_id)),
            make_string_array(rows.iter().map(|row| row.dataspace_alias.as_deref())),
            make_u64_array(rows.iter().map(|row| row.block_height)),
            make_u64_array(rows.iter().map(|row| row.finality_lag_slots)),
            make_u64_array(rows.iter().map(|row| row.teu_capacity)),
            make_u64_array(rows.iter().map(|row| row.teu_committed)),
            make_f64_array(rows.iter().map(|row| row.teu_utilization_pct)),
            make_u64_array(rows.iter().map(|row| row.trigger_level)),
            make_u64_array(rows.iter().map(|row| row.must_serve_truncations)),
            make_u64_array(rows.iter().map(|row| row.scheduler_utilization_pct)),
            make_u64_array(rows.iter().map(|row| row.tx_vertices)),
            make_u64_array(rows.iter().map(|row| row.tx_edges)),
            make_u64_array(rows.iter().map(|row| row.rbc_chunks)),
            make_u64_array(rows.iter().map(|row| row.rbc_bytes_total)),
            make_string_array(
                rows.iter()
                    .map(|row| Some(row.settlement_backlog_xor_micro.as_str())),
            ),
            make_f64_array(rows.iter().map(|row| row.settlement_backlog_xor)),
            make_string_array(rows.iter().map(|row| row.governance.as_deref())),
            make_string_array(rows.iter().map(|row| row.settlement.as_deref())),
            make_string_array(compliance_serialized.iter().map(|value| value.as_deref())),
            make_bool_array(rows.iter().map(|row| row.manifest_required)),
            make_bool_array(rows.iter().map(|row| row.manifest_ready)),
            make_string_array(rows.iter().map(|row| Some(row.captured_at.as_str()))),
            make_u64_array(rows.iter().map(|row| row.status_height)),
        ],
    )?;
    let file = fs::File::create(path)?;
    let props = WriterProperties::builder()
        .set_compression(Compression::UNCOMPRESSED)
        .build();
    let mut writer = ArrowWriter::try_new(file, schema, Some(props))?;
    writer.write(&batch)?;
    writer.close()?;
    Ok(())
}

fn write_markdown_summary(path: &Path, rows: &[LaneAuditRow]) -> Result<(), Box<dyn Error>> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut rendered = String::new();
    writeln!(rendered, "# Nexus Lane Audit")?;
    if rows.is_empty() {
        writeln!(
            rendered,
            "\nNo lane telemetry records were found in the provided status snapshot."
        )?;
        fs::write(path, rendered)?;
        return Ok(());
    }

    let captured_at = &rows[0].captured_at;
    let status_height = rows[0].status_height;
    let total = rows.len();
    let lagging = rows.iter().filter(|row| row.finality_lag_slots > 0).count();
    let backlog_lanes = rows
        .iter()
        .filter(|row| row.settlement_backlog_xor > 0.0)
        .count();
    let pending_manifests = rows
        .iter()
        .filter(|row| row.manifest_required && !row.manifest_ready)
        .count();
    let missing_compliance = rows
        .iter()
        .filter(|row| row.manifest_required && row.lane_compliance.is_none())
        .count();
    let max_backlog = rows
        .iter()
        .map(|row| row.settlement_backlog_xor)
        .fold(0.0_f64, f64::max);
    let max_lag = rows
        .iter()
        .map(|row| row.finality_lag_slots)
        .max()
        .unwrap_or(0);

    writeln!(rendered)?;
    writeln!(rendered, "- Captured at: {captured_at}")?;
    writeln!(rendered, "- Status height: {status_height}")?;
    writeln!(
        rendered,
        "- Lanes: {total} (lagging: {lagging}; backlog>0: {backlog_lanes}; pending manifests: {pending_manifests}; missing compliance: {missing_compliance})"
    )?;
    writeln!(rendered, "- Max finality lag (slots): {max_lag}")?;
    writeln!(rendered, "- Peak backlog (XOR): {:.6}", max_backlog)?;
    writeln!(rendered)?;
    writeln!(
        rendered,
        "| Lane | Dataspace | Height | Finality Lag | Backlog (XOR) | TEU Util (%) | Flags |"
    )?;
    writeln!(rendered, "| --- | --- | --- | --- | --- | --- | --- |")?;

    for row in rows {
        let lane_label = format!("{} (#{})", row.lane_alias, row.lane_id);
        let dataspace_label = match row.dataspace_alias.as_deref() {
            Some(alias) => format!("{alias} (#{})", row.dataspace_id),
            None => format!("#{}", row.dataspace_id),
        };
        let flags = {
            let mut entries: Vec<&str> = Vec::new();
            if row.finality_lag_slots > 0 {
                entries.push("lag");
            }
            if row.settlement_backlog_xor > 0.0 {
                entries.push("backlog");
            }
            if row.manifest_required && !row.manifest_ready {
                entries.push("manifest");
            }
            if row.manifest_required && row.lane_compliance.is_none() {
                entries.push("compliance");
            }
            if row.teu_utilization_pct >= 95.0 {
                entries.push("teu-high");
            }
            if entries.is_empty() {
                "ok".to_string()
            } else {
                entries.join(",")
            }
        };
        writeln!(
            rendered,
            "| {lane_label} | {dataspace_label} | {} | {} | {:.6} | {:.1} | {flags} |",
            row.block_height,
            row.finality_lag_slots,
            row.settlement_backlog_xor,
            row.teu_utilization_pct
        )?;
    }

    fs::write(path, rendered)?;
    Ok(())
}

fn arrow_schema() -> Schema {
    Schema::new(vec![
        Field::new("lane_id", DataType::UInt32, false),
        Field::new("lane_alias", DataType::Utf8, false),
        Field::new("dataspace_id", DataType::UInt64, false),
        Field::new("dataspace_alias", DataType::Utf8, true),
        Field::new("block_height", DataType::UInt64, false),
        Field::new("finality_lag_slots", DataType::UInt64, false),
        Field::new("teu_capacity", DataType::UInt64, false),
        Field::new("teu_committed", DataType::UInt64, false),
        Field::new("teu_utilization_pct", DataType::Float64, false),
        Field::new("trigger_level", DataType::UInt64, false),
        Field::new("must_serve_truncations", DataType::UInt64, false),
        Field::new("scheduler_utilization_pct", DataType::UInt64, false),
        Field::new("tx_vertices", DataType::UInt64, false),
        Field::new("tx_edges", DataType::UInt64, false),
        Field::new("rbc_chunks", DataType::UInt64, false),
        Field::new("rbc_bytes_total", DataType::UInt64, false),
        Field::new("settlement_backlog_xor_micro", DataType::Utf8, false),
        Field::new("settlement_backlog_xor", DataType::Float64, false),
        Field::new("governance", DataType::Utf8, true),
        Field::new("settlement", DataType::Utf8, true),
        Field::new("lane_compliance", DataType::Utf8, true),
        Field::new("manifest_required", DataType::Boolean, false),
        Field::new("manifest_ready", DataType::Boolean, false),
        Field::new("captured_at", DataType::Utf8, false),
        Field::new("status_height", DataType::UInt64, false),
    ])
}

fn make_u32_array<I>(values: I) -> ArrayRef
where
    I: IntoIterator<Item = u32>,
{
    Arc::new(UInt32Array::from_iter_values(values))
}

fn make_u64_array<I>(values: I) -> ArrayRef
where
    I: IntoIterator<Item = u64>,
{
    Arc::new(UInt64Array::from_iter_values(values))
}

fn make_f64_array<I>(values: I) -> ArrayRef
where
    I: IntoIterator<Item = f64>,
{
    Arc::new(Float64Array::from_iter_values(values))
}

fn make_bool_array<I>(values: I) -> ArrayRef
where
    I: IntoIterator<Item = bool>,
{
    Arc::new(BooleanArray::from_iter(values.into_iter().map(Some)))
}

fn make_string_array<'a, I>(values: I) -> ArrayRef
where
    I: IntoIterator<Item = Option<&'a str>>,
{
    Arc::new(StringArray::from_iter(values))
}

fn compute_teu_utilization_pct(capacity: u64, committed: u64) -> f64 {
    if capacity == 0 {
        return 0.0;
    }
    (committed as f64 / capacity as f64) * 100.0
}

fn micro_xor_to_units(value: u128) -> f64 {
    (value as f64) / 1_000_000.0
}

fn receipt(
    source_hex: &str,
    local_amount_micro: u128,
    xor_due_micro: u128,
    xor_after_haircut_micro: u128,
    timestamp_ms: u64,
) -> LaneSettlementReceipt {
    let variance = xor_due_micro.saturating_sub(xor_after_haircut_micro);
    LaneSettlementReceipt {
        source_id: hex32(source_hex),
        local_amount_micro,
        xor_due_micro,
        xor_after_haircut_micro,
        xor_variance_micro: variance,
        timestamp_ms,
    }
}

fn hex32(input: &str) -> [u8; 32] {
    let bytes = decode(input).expect("fixture hex payload");
    assert_eq!(
        bytes.len(),
        32,
        "lane commitment fixture ids must be 32 bytes"
    );
    let mut out = [0_u8; 32];
    out.copy_from_slice(&bytes);
    out
}

#[cfg(test)]
mod tests {
    use std::fs;

    use arrow_array::{Array, BooleanArray, Float64Array, StringArray, UInt32Array, UInt64Array};
    use iroha_data_model::{
        metadata::Metadata,
        nexus::{AuditControls, JurisdictionSet, LaneCompliancePolicyId},
    };
    use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
    use tempfile::{NamedTempFile, tempdir};

    use super::*;

    #[test]
    fn teu_utilization_helper_handles_zero_capacity() {
        assert_eq!(compute_teu_utilization_pct(0, 10), 0.0);
        let pct = compute_teu_utilization_pct(200, 100);
        assert!((pct - 50.0).abs() < 1e-6);
    }

    #[test]
    fn parquet_writer_round_trips_rows() {
        let dir = tempdir().expect("tempdir");
        let parquet_path = dir.path().join("lane.parquet");
        let rows = vec![
            LaneAuditRow {
                lane_id: 4,
                lane_alias: "lane-4".to_string(),
                dataspace_id: 7,
                dataspace_alias: Some("payments".to_string()),
                block_height: 42,
                finality_lag_slots: 3,
                teu_capacity: 1200,
                teu_committed: 640,
                teu_utilization_pct: compute_teu_utilization_pct(1200, 640),
                trigger_level: 0,
                must_serve_truncations: 2,
                scheduler_utilization_pct: 87,
                tx_vertices: 48,
                tx_edges: 12,
                rbc_chunks: 1_024,
                rbc_bytes_total: 65_536,
                settlement_backlog_xor_micro: "1250000".to_string(),
                settlement_backlog_xor: 1.25,
                governance: Some("council".to_string()),
                settlement: None,
                lane_compliance: Some(sample_compliance(4, 7)),
                manifest_required: true,
                manifest_ready: false,
                captured_at: "2026-02-12T09:00:00Z".to_string(),
                status_height: 81,
            },
            LaneAuditRow {
                lane_id: 9,
                lane_alias: "lane-9".to_string(),
                dataspace_id: 11,
                dataspace_alias: None,
                block_height: 7,
                finality_lag_slots: 1,
                teu_capacity: 250,
                teu_committed: 250,
                teu_utilization_pct: compute_teu_utilization_pct(250, 250),
                trigger_level: 1,
                must_serve_truncations: 0,
                scheduler_utilization_pct: 64,
                tx_vertices: 12,
                tx_edges: 2,
                rbc_chunks: 512,
                rbc_bytes_total: 16_384,
                settlement_backlog_xor_micro: "0".to_string(),
                settlement_backlog_xor: 0.0,
                governance: None,
                settlement: Some("settlement-ok".to_string()),
                lane_compliance: None,
                manifest_required: false,
                manifest_ready: true,
                captured_at: "2026-02-12T10:00:00Z".to_string(),
                status_height: 99,
            },
        ];

        write_parquet_rows(&parquet_path, &rows).expect("parquet writer");

        let file = fs::File::open(&parquet_path).expect("parquet exists");
        let mut reader = ParquetRecordBatchReaderBuilder::try_new(file)
            .expect("reader builder")
            .with_batch_size(64)
            .build()
            .expect("reader");
        let batch = reader.next().expect("one batch").expect("batch ok");

        assert_eq!(batch.num_rows(), rows.len());

        let lane_ids = batch
            .column_by_name("lane_id")
            .and_then(|array| array.as_any().downcast_ref::<UInt32Array>())
            .expect("lane_id column");
        assert_eq!(lane_ids.value(0), rows[0].lane_id);
        assert_eq!(lane_ids.value(1), rows[1].lane_id);

        let dataspace_alias = batch
            .column_by_name("dataspace_alias")
            .and_then(|array| array.as_any().downcast_ref::<StringArray>())
            .expect("dataspace alias");
        assert_eq!(dataspace_alias.value(0), "payments");
        assert!(dataspace_alias.is_null(1));

        let utilization = batch
            .column_by_name("teu_utilization_pct")
            .and_then(|array| array.as_any().downcast_ref::<Float64Array>())
            .expect("utilization column");
        assert!((utilization.value(0) - rows[0].teu_utilization_pct).abs() < 1e-6);

        let backlog = batch
            .column_by_name("settlement_backlog_xor")
            .and_then(|array| array.as_any().downcast_ref::<Float64Array>())
            .expect("backlog column");
        assert!((backlog.value(0) - rows[0].settlement_backlog_xor).abs() < 1e-6);

        let manifest_required = batch
            .column_by_name("manifest_required")
            .and_then(|array| array.as_any().downcast_ref::<BooleanArray>())
            .expect("manifest_required column");
        assert!(manifest_required.value(0));
        assert!(!manifest_required.value(1));

        let status_height = batch
            .column_by_name("status_height")
            .and_then(|array| array.as_any().downcast_ref::<UInt64Array>())
            .expect("status height");
        assert_eq!(status_height.value(1), rows[1].status_height);

        let trigger_level = batch
            .column_by_name("trigger_level")
            .and_then(|array| array.as_any().downcast_ref::<UInt64Array>())
            .expect("trigger level");
        assert_eq!(trigger_level.value(1), rows[1].trigger_level);

        let compliance = batch
            .column_by_name("lane_compliance")
            .and_then(|array| array.as_any().downcast_ref::<StringArray>())
            .expect("lane_compliance column");
        assert!(compliance.is_valid(0));
        assert!(compliance.is_null(1));
        let parsed: JsonValue =
            serde_json::from_str(compliance.value(0)).expect("compliance json parses");
        assert_eq!(
            parsed
                .get("reviewer_signatures")
                .and_then(|value| value.as_array())
                .map(|arr| arr.len()),
            Some(1)
        );

        assert!(
            reader.next().is_none(),
            "parquet reader should yield a single batch"
        );
    }

    #[test]
    fn lane_compliance_loader_rejects_mismatched_lane_ids() {
        let file = NamedTempFile::new().expect("temp file");
        let mut record = record_from_evidence(sample_compliance(4, 7), 4);
        if let JsonValue::Object(ref mut map) = record.policy {
            map.insert("lane_id".to_string(), JsonValue::from(3_u64));
        } else {
            panic!("policy should be an object");
        }
        write_compliance_file(file.path(), vec![record]);
        let err = load_lane_compliance_map(file.path()).expect_err("loader must fail");
        assert!(
            err.to_string().contains("policy targets lane_id"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn lane_compliance_loader_rejects_duplicate_entries() {
        let file = NamedTempFile::new().expect("temp file");
        let record_a = record_from_evidence(sample_compliance(4, 7), 4);
        let record_b = record_from_evidence(sample_compliance(4, 7), 4);
        write_compliance_file(file.path(), vec![record_a, record_b]);
        let err = load_lane_compliance_map(file.path()).expect_err("loader must fail");
        assert!(
            err.to_string().contains("duplicate entry for lane"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn lane_compliance_loader_accepts_valid_entries() {
        let file = NamedTempFile::new().expect("temp file");
        let record_a = record_from_evidence(sample_compliance(4, 7), 4);
        let record_b = record_from_evidence(sample_compliance(9, 11), 9);
        write_compliance_file(file.path(), vec![record_a, record_b]);
        let map = load_lane_compliance_map(file.path()).expect("loader ok");
        assert_eq!(map.len(), 2);
        assert!(map.contains_key(&4));
        assert!(map.contains_key(&9));
    }

    #[test]
    fn markdown_summary_includes_flags_and_counts() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("lane.md");
        let rows = vec![
            LaneAuditRow {
                lane_id: 4,
                lane_alias: "lane-4".to_string(),
                dataspace_id: 7,
                dataspace_alias: Some("payments".to_string()),
                block_height: 42,
                finality_lag_slots: 3,
                teu_capacity: 1200,
                teu_committed: 1188,
                teu_utilization_pct: compute_teu_utilization_pct(1200, 1188),
                trigger_level: 0,
                must_serve_truncations: 2,
                scheduler_utilization_pct: 87,
                tx_vertices: 48,
                tx_edges: 12,
                rbc_chunks: 1_024,
                rbc_bytes_total: 65_536,
                settlement_backlog_xor_micro: "1250000".to_string(),
                settlement_backlog_xor: 1.25,
                governance: Some("council".to_string()),
                settlement: None,
                lane_compliance: None,
                manifest_required: true,
                manifest_ready: false,
                captured_at: "2026-02-12T09:00:00Z".to_string(),
                status_height: 81,
            },
            LaneAuditRow {
                lane_id: 9,
                lane_alias: "lane-9".to_string(),
                dataspace_id: 11,
                dataspace_alias: None,
                block_height: 7,
                finality_lag_slots: 0,
                teu_capacity: 250,
                teu_committed: 200,
                teu_utilization_pct: compute_teu_utilization_pct(250, 200),
                trigger_level: 1,
                must_serve_truncations: 0,
                scheduler_utilization_pct: 64,
                tx_vertices: 12,
                tx_edges: 2,
                rbc_chunks: 512,
                rbc_bytes_total: 16_384,
                settlement_backlog_xor_micro: "0".to_string(),
                settlement_backlog_xor: 0.0,
                governance: None,
                settlement: Some("settlement-ok".to_string()),
                lane_compliance: Some(sample_compliance(9, 11)),
                manifest_required: false,
                manifest_ready: true,
                captured_at: "2026-02-12T09:00:00Z".to_string(),
                status_height: 81,
            },
        ];

        write_markdown_summary(&path, &rows).expect("markdown writer ok");
        let rendered = fs::read_to_string(&path).expect("markdown exists");
        assert!(rendered.contains("# Nexus Lane Audit"));
        assert!(rendered.contains(
            "Lanes: 2 (lagging: 1; backlog>0: 1; pending manifests: 1; missing compliance: 1)"
        ));
        assert!(rendered.contains(
            "| lane-4 (#4) | payments (#7) | 42 | 3 | 1.250000 | 99.0 | lag,backlog,manifest,compliance,teu-high |"
        ));
        assert!(rendered.contains("| lane-9 (#9) | #11 | 7 | 0 | 0.000000 | 80.0 | ok |"));
    }

    fn sample_compliance(lane_id: u32, dataspace_id: u64) -> LaneComplianceEvidence {
        let policy = LaneCompliancePolicy {
            id: LaneCompliancePolicyId::default(),
            version: 1,
            lane_id: LaneId::new(lane_id),
            dataspace_id: DataSpaceId::new(dataspace_id),
            jurisdiction: JurisdictionSet::default(),
            deny: Vec::new(),
            allow: Vec::new(),
            transfer_limits: Vec::new(),
            audit_controls: AuditControls::default(),
            metadata: Metadata::default(),
        };
        LaneComplianceEvidence {
            policy: policy_to_json_value(&policy),
            reviewer_signatures: vec![LaneComplianceReviewerSignature {
                reviewer: "auditor@example.com".to_string(),
                signature_hex: "deadbeef".to_string(),
                signed_at: Some("2026-02-01T00:00:00Z".to_string()),
                digest_hex: None,
                notes: Some("baseline".to_string()),
            }],
            metrics_snapshot: json!({
                "nexus_lane_policy_decisions_total": {
                    "allow": 42,
                    "deny": 1
                }
            }),
            audit_log: vec![json!({
                "decision": "allow",
                "policy_id": "lane-4-policy",
                "recorded_at": "2026-02-12T09:00:00Z"
            })],
        }
    }

    fn record_from_evidence(
        evidence: LaneComplianceEvidence,
        lane_id: u32,
    ) -> LaneComplianceEvidenceRecord {
        LaneComplianceEvidenceRecord {
            lane_id,
            policy: evidence.policy,
            reviewer_signatures: evidence.reviewer_signatures,
            metrics_snapshot: evidence.metrics_snapshot,
            audit_log: evidence.audit_log,
        }
    }

    fn write_compliance_file(path: &Path, records: Vec<LaneComplianceEvidenceRecord>) {
        let file = LaneComplianceEvidenceFile { lanes: records };
        let value = serde_json::to_value(&file).expect("lane compliance value");
        let mut rendered =
            serde_json::to_string_pretty(&value).expect("lane compliance serialization");
        rendered.push('\n');
        fs::write(path, rendered).expect("write compliance file");
    }

    fn policy_to_json_value(policy: &LaneCompliancePolicy) -> JsonValue {
        let norito_value = json::to_value(policy).expect("policy json value");
        let rendered = json::to_string(&norito_value).expect("policy json encode");
        serde_json::from_str(&rendered).expect("serde policy json")
    }
}
