//! GAR enforcement receipt bundling (SNNet-15G1).
//!
//! Collects NORITO/JSON receipts and optional ACK records, then emits a single
//! JSON summary (and optional Markdown) for governance/compliance exports.

use std::{
    collections::BTreeMap,
    fmt::Write as FmtWrite,
    fs,
    path::{Path, PathBuf},
};

use eyre::{Result, WrapErr, eyre};
use hex::{decode_to_slice, encode};
use iroha_data_model::sorafs::gar::{GarEnforcementActionV1, GarEnforcementReceiptV1};
use norito::json::{self, Map, Number, Value};
use walkdir::WalkDir;

/// Options controlling receipt export.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub receipts_dir: PathBuf,
    pub ack_dir: Option<PathBuf>,
    pub pop_label: Option<String>,
    pub markdown_out: Option<PathBuf>,
    pub now_unix: Option<u64>,
}

/// Load receipts/acks, emit a JSON summary, and optionally persist a Markdown report.
pub fn export_receipts(options: ExportOptions) -> Result<Value> {
    let receipts = load_receipts(&options.receipts_dir)?;
    if receipts.is_empty() {
        return Err(eyre!(
            "no GAR enforcement receipts found under `{}`",
            options.receipts_dir.display()
        ));
    }
    let mut acks = if let Some(dir) = &options.ack_dir {
        load_acks(dir)?
    } else {
        BTreeMap::new()
    };

    let mut action_counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut earliest: Option<u64> = None;
    let mut latest: Option<u64> = None;
    let mut receipts_out = Vec::with_capacity(receipts.len());
    let mut missing_acks = 0usize;
    let mut acked = 0usize;

    for receipt in receipts {
        let triggered = receipt.receipt.triggered_at_unix;
        earliest = Some(earliest.map_or(triggered, |current| current.min(triggered)));
        latest = Some(latest.map_or(triggered, |current| current.max(triggered)));

        let action_label = action_label(&receipt.receipt.action);
        *action_counts.entry(action_label.to_string()).or_default() += 1;

        let age_seconds = options.now_unix.map(|now| now.saturating_sub(triggered));

        let ack = acks.remove(&receipt.receipt.receipt_id);
        if ack.is_some() {
            acked += 1;
        } else {
            missing_acks += 1;
        }

        let action_json = action_json(&receipt.receipt.action);
        let policy_digest_hex = receipt.receipt.policy_digest.map(encode);

        let ack_json = ack.as_ref().map(|ack| {
            let mut obj = Map::new();
            obj.insert("path".into(), Value::String(ack.path.display().to_string()));
            if let Some(version) = &ack.applied_version {
                obj.insert("applied_version".into(), Value::String(version.clone()));
            }
            if let Some(pop) = ack.pop_label() {
                obj.insert("pop".into(), Value::String(pop));
            }
            if let Some(acked_at) = ack.acked_at_unix {
                obj.insert(
                    "acked_at_unix".into(),
                    Value::Number(Number::from(acked_at)),
                );
                let lag = acked_at.saturating_sub(triggered);
                obj.insert("ack_lag_seconds".into(), Value::Number(Number::from(lag)));
            }
            Value::Object(obj)
        });

        let mut item = Map::new();
        item.insert(
            "receipt_path".into(),
            Value::String(receipt.path.display().to_string()),
        );
        item.insert(
            "receipt_id".into(),
            Value::String(encode(receipt.receipt.receipt_id)),
        );
        item.insert("gar_name".into(), Value::String(receipt.receipt.gar_name));
        item.insert(
            "canonical_host".into(),
            Value::String(receipt.receipt.canonical_host),
        );
        item.insert("action".into(), action_json);
        item.insert(
            "triggered_at_unix".into(),
            Value::Number(Number::from(triggered)),
        );
        if let Some(expires) = receipt.receipt.expires_at_unix {
            item.insert(
                "expires_at_unix".into(),
                Value::Number(Number::from(expires)),
            );
        }
        if let Some(age) = age_seconds {
            item.insert("age_seconds".into(), Value::Number(Number::from(age)));
        }
        if let Some(version) = receipt.receipt.policy_version {
            item.insert("policy_version".into(), Value::String(version));
        }
        if let Some(digest) = policy_digest_hex {
            item.insert("policy_digest".into(), Value::String(digest));
        }
        item.insert(
            "operator".into(),
            Value::String(receipt.receipt.operator.to_string()),
        );
        item.insert("reason".into(), Value::String(receipt.receipt.reason));
        if let Some(notes) = receipt.receipt.notes {
            item.insert("notes".into(), Value::String(notes));
        }
        item.insert(
            "evidence_uris".into(),
            Value::Array(
                receipt
                    .receipt
                    .evidence_uris
                    .into_iter()
                    .map(Value::String)
                    .collect(),
            ),
        );
        item.insert(
            "labels".into(),
            Value::Array(
                receipt
                    .receipt
                    .labels
                    .into_iter()
                    .map(Value::String)
                    .collect(),
            ),
        );
        if let Some(ack_value) = ack_json {
            item.insert("ack".into(), ack_value);
        }
        receipts_out.push(Value::Object(item));
    }

    let dangling_acks = acks
        .into_values()
        .map(|ack| {
            let mut obj = Map::new();
            obj.insert("path".into(), Value::String(ack.path.display().to_string()));
            obj.insert("receipt_id".into(), Value::String(encode(ack.receipt_id)));
            if let Some(pop) = ack.pop_label() {
                obj.insert("pop".into(), Value::String(pop));
            }
            if let Some(version) = ack.applied_version {
                obj.insert("applied_version".into(), Value::String(version));
            }
            if let Some(acked_at) = ack.acked_at_unix {
                obj.insert(
                    "acked_at_unix".into(),
                    Value::Number(Number::from(acked_at)),
                );
            }
            Value::Object(obj)
        })
        .collect::<Vec<_>>();

    let mut actions_obj = Map::new();
    for (action, count) in action_counts {
        actions_obj.insert(action, Value::Number(Number::from(count)));
    }

    let mut root = Map::new();
    if let Some(pop) = &options.pop_label {
        root.insert("pop_label".into(), Value::String(pop.clone()));
    }
    root.insert(
        "receipts_dir".into(),
        Value::String(options.receipts_dir.display().to_string()),
    );
    if let Some(dir) = &options.ack_dir {
        root.insert("acks_dir".into(), Value::String(dir.display().to_string()));
    }
    let receipts_count = receipts_out.len() as u64;
    let dangling_count = dangling_acks.len() as u64;
    root.insert(
        "counts".into(),
        Value::Object(Map::from_iter([
            (
                "receipts".into(),
                Value::Number(Number::from(receipts_count)),
            ),
            (
                "acknowledged".into(),
                Value::Number(Number::from(acked as u64)),
            ),
            (
                "missing_acks".into(),
                Value::Number(Number::from(missing_acks as u64)),
            ),
            (
                "dangling_acks".into(),
                Value::Number(Number::from(dangling_count)),
            ),
        ])),
    );
    if let Some(earliest) = earliest {
        let mut window = Map::new();
        window.insert(
            "earliest_triggered_at_unix".into(),
            Value::Number(Number::from(earliest)),
        );
        if let Some(latest) = latest {
            window.insert(
                "latest_triggered_at_unix".into(),
                Value::Number(Number::from(latest)),
            );
        }
        root.insert("window".into(), Value::Object(window));
    }
    root.insert("actions".into(), Value::Object(actions_obj));
    root.insert("receipts".into(), Value::Array(receipts_out));
    root.insert("dangling_ack_files".into(), Value::Array(dangling_acks));

    let summary = Value::Object(root);
    if let Some(path) = options.markdown_out {
        let markdown = render_markdown(&summary);
        fs::write(&path, markdown)
            .wrap_err_with(|| format!("failed to write markdown report `{}`", path.display()))?;
    }
    Ok(summary)
}

fn load_receipts(dir: &Path) -> Result<Vec<ReceiptFile>> {
    if !dir.exists() {
        return Err(eyre!(
            "receipt directory `{}` does not exist",
            dir.display()
        ));
    }
    let mut receipts = Vec::new();
    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        let bytes = fs::read(path)
            .wrap_err_with(|| format!("failed to read receipt `{}`", path.display()))?;
        let receipt = if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            json::from_slice(&bytes)
                .wrap_err_with(|| format!("failed to parse JSON receipt `{}`", path.display()))?
        } else {
            norito::decode_from_bytes(&bytes)
                .wrap_err_with(|| format!("failed to parse Norito receipt `{}`", path.display()))?
        };
        receipts.push(ReceiptFile {
            path: path.to_path_buf(),
            receipt,
        });
    }
    receipts.sort_by_key(|receipt| receipt.receipt.triggered_at_unix);
    Ok(receipts)
}

fn load_acks(dir: &Path) -> Result<BTreeMap<[u8; 16], AckRecord>> {
    if !dir.exists() {
        return Err(eyre!("ack directory `{}` does not exist", dir.display()));
    }
    let mut records = BTreeMap::new();
    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
    {
        let path = entry.path();
        let bytes =
            fs::read(path).wrap_err_with(|| format!("failed to read ack `{}`", path.display()))?;
        let value: Value = json::from_slice(&bytes)
            .wrap_err_with(|| format!("failed to parse ack JSON `{}`", path.display()))?;
        let map = value.as_object().ok_or_else(|| {
            eyre!(
                "ack `{}` must be a JSON object (found {:?})",
                path.display(),
                value
            )
        })?;
        let Some(Value::String(receipt_id_hex)) = map.get("receipt_id") else {
            return Err(eyre!("ack `{}` missing `receipt_id` field", path.display()));
        };
        let receipt_id = parse_hex_array::<16>(receipt_id_hex, "receipt_id", path)
            .wrap_err_with(|| format!("failed to decode receipt id in ack `{}`", path.display()))?;
        let applied_version = map
            .get("applied_version")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let acked_at_unix = map.get("acked_at_unix").and_then(Value::as_u64);
        let pop = map
            .get("pop")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned);
        let ack = AckRecord {
            path: path.to_path_buf(),
            receipt_id,
            applied_version,
            acked_at_unix,
            pop,
        };
        records.insert(receipt_id, ack);
    }
    Ok(records)
}

fn parse_hex_array<const N: usize>(hex: &str, field: &str, path: &Path) -> Result<[u8; N]> {
    let mut buffer = [0u8; N];
    decode_to_slice(hex, &mut buffer).map_err(|_| {
        eyre!(
            "{field} in `{}` must be {N} bytes of hex (got `{hex}`)",
            path.display()
        )
    })?;
    Ok(buffer)
}

fn action_label(action: &GarEnforcementActionV1) -> &'static str {
    match action {
        GarEnforcementActionV1::PurgeStaticZone => "purge-static-zone",
        GarEnforcementActionV1::CacheBypass => "cache-bypass",
        GarEnforcementActionV1::TtlOverride => "ttl-override",
        GarEnforcementActionV1::RateLimitOverride => "rate-limit-override",
        GarEnforcementActionV1::GeoFence => "geo-fence",
        GarEnforcementActionV1::LegalHold => "legal-hold",
        GarEnforcementActionV1::Moderation => "moderation",
        GarEnforcementActionV1::AuditNotice => "audit-notice",
        GarEnforcementActionV1::Custom(_) => "custom",
    }
}

fn action_json(action: &GarEnforcementActionV1) -> Value {
    match action {
        GarEnforcementActionV1::Custom(slug) => {
            let mut obj = Map::new();
            obj.insert("kind".into(), Value::String("custom".to_string()));
            obj.insert("slug".into(), Value::String(slug.clone()));
            Value::Object(obj)
        }
        _ => {
            let mut obj = Map::new();
            obj.insert(
                "kind".into(),
                Value::String(action_label(action).to_string()),
            );
            Value::Object(obj)
        }
    }
}

fn render_markdown(summary: &Value) -> String {
    let mut out = String::new();
    let counts = summary
        .get("counts")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let receipts = counts.get("receipts").and_then(Value::as_u64).unwrap_or(0);
    let acked = counts
        .get("acknowledged")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let missing = counts
        .get("missing_acks")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let dangling = counts
        .get("dangling_acks")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let _ = writeln!(out, "# GAR Enforcement Receipt Export");
    let _ = writeln!(out, "- Receipts: {receipts}");
    let _ = writeln!(out, "- Acknowledged: {acked}");
    let _ = writeln!(out, "- Missing ACKs: {missing}");
    let _ = writeln!(out, "- Dangling ACK files: {dangling}");

    if let Some(actions) = summary.get("actions").and_then(Value::as_object) {
        let _ = writeln!(out, "\n## Actions");
        for (action, count) in actions {
            if let Some(value) = count.as_u64() {
                let _ = writeln!(out, "- {action}: {value}");
            }
        }
    }

    if let Some(receipts) = summary.get("receipts").and_then(Value::as_array) {
        let _ = writeln!(out, "\n## Receipts");
        let _ = writeln!(
            out,
            "| Receipt ID | Action | Triggered (unix) | ACKed? | Operator | GAR | Host |"
        );
        let _ = writeln!(out, "| --- | --- | --- | --- | --- | --- | --- |");
        for receipt in receipts {
            let id = receipt
                .get("receipt_id")
                .and_then(Value::as_str)
                .unwrap_or("-");
            let action = receipt
                .get("action")
                .and_then(Value::as_object)
                .and_then(|obj| obj.get("kind"))
                .and_then(Value::as_str)
                .unwrap_or("-");
            let triggered = receipt
                .get("triggered_at_unix")
                .and_then(Value::as_u64)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".to_string());
            let acked = if receipt.get("ack").is_some() {
                "yes"
            } else {
                "no"
            };
            let operator = receipt
                .get("operator")
                .and_then(Value::as_str)
                .unwrap_or("-");
            let gar = receipt
                .get("gar_name")
                .and_then(Value::as_str)
                .unwrap_or("-");
            let host = receipt
                .get("canonical_host")
                .and_then(Value::as_str)
                .unwrap_or("-");
            let _ = writeln!(
                out,
                "| `{id}` | {action} | {triggered} | {acked} | `{operator}` | {gar} | {host} |"
            );
        }
    }

    out
}

#[derive(Debug, Clone)]
struct ReceiptFile {
    path: PathBuf,
    receipt: GarEnforcementReceiptV1,
}

#[derive(Debug, Clone)]
struct AckRecord {
    path: PathBuf,
    receipt_id: [u8; 16],
    applied_version: Option<String>,
    acked_at_unix: Option<u64>,
    pop: Option<String>,
}

impl AckRecord {
    fn pop_label(&self) -> Option<String> {
        self.pop.clone()
    }
}

#[cfg(test)]
mod tests {
    use iroha_data_model::account::AccountId;
    use norito::json;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn exports_receipts_and_acks() -> Result<()> {
        let temp = TempDir::new()?;
        let receipts_dir = temp.path().join("receipts");
        let acks_dir = temp.path().join("acks");
        fs::create_dir_all(&receipts_dir)?;
        fs::create_dir_all(&acks_dir)?;

        let operator =
            AccountId::parse_encoded("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn")
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                .unwrap();
        let other_operator =
            AccountId::parse_encoded("6cmzPVPX4Vs6C1nbbQ7UD7Q6AWKJFC12abs4kZtXEE9SsFf6QRpp8rU")
                .map(iroha_data_model::account::ParsedAccountId::into_account_id)
                .unwrap();

        let first = GarEnforcementReceiptV1 {
            receipt_id: *b"0123456789abcdef",
            gar_name: "docs.sora".to_string(),
            canonical_host: "docs.gateway.sora.net".to_string(),
            action: GarEnforcementActionV1::RateLimitOverride,
            triggered_at_unix: 1_700_000_000,
            expires_at_unix: None,
            policy_version: Some("2026-q2".to_string()),
            policy_digest: Some([0xAB; 32]),
            operator: operator.clone(),
            reason: "drill".to_string(),
            notes: None,
            evidence_uris: vec!["sora://gar/drill".to_string()],
            labels: vec!["drill".to_string()],
        };
        let second = GarEnforcementReceiptV1 {
            receipt_id: *b"fedcba9876543210",
            gar_name: "cdn.sora".to_string(),
            canonical_host: "cdn.gateway.sora.net".to_string(),
            action: GarEnforcementActionV1::AuditNotice,
            triggered_at_unix: 1_700_000_500,
            expires_at_unix: Some(1_700_001_000),
            policy_version: None,
            policy_digest: None,
            operator: other_operator.clone(),
            reason: "notice".to_string(),
            notes: Some("note".to_string()),
            evidence_uris: vec![],
            labels: vec![],
        };

        let first_json = json::to_vec(&first)?;
        fs::write(receipts_dir.join("first.json"), first_json)?;

        let second_bytes = norito::to_bytes(&second)?;
        fs::write(receipts_dir.join("second.to"), second_bytes)?;

        let ack_value = Value::Object(Map::from_iter([
            (
                "receipt_id".into(),
                Value::String("30313233343536373839616263646566".to_string()),
            ),
            ("applied_version".into(), Value::String("m0".to_string())),
            (
                "acked_at_unix".into(),
                Value::Number(Number::from(1_700_000_100u64)),
            ),
            ("pop".into(), Value::String("soranet-pop-m0".to_string())),
        ]));
        fs::write(
            acks_dir.join("ack.json"),
            json::to_string_pretty(&ack_value)?,
        )?;

        let summary = export_receipts(ExportOptions {
            receipts_dir: receipts_dir.clone(),
            ack_dir: Some(acks_dir.clone()),
            pop_label: Some("soranet-pop-m0".to_string()),
            markdown_out: Some(temp.path().join("report.md")),
            now_unix: Some(1_700_000_900),
        })?;

        assert_eq!(
            summary["counts"]["receipts"].as_u64(),
            Some(2),
            "should count two receipts"
        );
        assert_eq!(
            summary["counts"]["acknowledged"].as_u64(),
            Some(1),
            "one receipt is acked"
        );
        assert_eq!(
            summary["counts"]["missing_acks"].as_u64(),
            Some(1),
            "one receipt is missing ack"
        );

        let receipts = summary["receipts"].as_array().cloned().expect("receipts");
        assert_eq!(receipts.len(), 2);
        let acked = receipts
            .iter()
            .find(|value| value["receipt_id"].as_str() == Some("30313233343536373839616263646566"))
            .expect("first receipt present");
        assert_eq!(
            acked["ack"]["applied_version"].as_str(),
            Some("m0"),
            "acks should surface applied_version"
        );
        let _ = fs::read_to_string(temp.path().join("report.md"))
            .expect("markdown report should be written");
        Ok(())
    }
}
