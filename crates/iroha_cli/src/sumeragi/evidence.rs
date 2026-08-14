#![allow(clippy::redundant_pub_crate, clippy::needless_pass_by_value)]
use eyre::Result;
use norito::json::Value;
use crate::{CliOutputFormat, RunContext};
use super::commands::{EvidenceCountArgs, EvidenceKindArg, EvidenceListArgs};
pub(crate) fn list<C: RunContext>(context: &mut C, args: EvidenceListArgs) -> Result<()> {
    let client = context.client_from_config();
    let kind = args.kind.map(EvidenceKindArg::as_str);
    let filter = iroha::client::SumeragiEvidenceListFilter {
        limit: args.limit,
        offset: args.offset,
        kind,
    };
    let value = client.get_sumeragi_evidence_list_json(&filter)?;
    if matches!(context.output_format(), CliOutputFormat::Text) {
        let total = value
            .get("total")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        context.println(format!("total={total}"))?;
        if let Some(items) = value.get("items").and_then(Value::as_array) {
            for (idx, item) in items.iter().enumerate() {
                context.println(format_evidence_summary(idx, item))?;
            }
        }
    } else {
        context.print_data(&value)?;
    }
    Ok(())
}
pub(crate) fn count<C: RunContext>(context: &mut C, _args: EvidenceCountArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_evidence_count_json()?;
    if matches!(context.output_format(), CliOutputFormat::Text) {
        let count = value
            .get("count")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        context.println(format!("count={count}"))?;
    } else {
        context.print_data(&value)?;
    }
    Ok(())
}
fn format_evidence_summary(idx: usize, item: &Value) -> String {
    let mut parts = Vec::new();
    let ordinal = idx + 1;
    let kind = item.get("kind").and_then(Value::as_str).unwrap_or("-");
    parts.push(format!("{ordinal}: kind={kind}"));
    for key in [
        "class",
        "phase",
        "height",
        "view",
        "epoch",
        "signer",
        "context_id",
        "artifact_hash_1",
        "artifact_hash_2",
        "block_hash",
        "block_hash_1",
        "block_hash_2",
        "subject_block_hash",
        "payload_hash",
        "parent_state_root",
        "post_state_root_1",
        "post_state_root_2",
        "recorded_height",
        "recorded_view",
        "submitted_at_height_min",
        "submitted_at_height_max",
        "consensus_admitted_height",
    ] {
        if let Some(value) = item.get(key)
            && let Some(rendered) = value_to_string(value)
        {
            parts.push(format!("{key}={rendered}"));
        }
    }
    if let Some(reason) = item.get("reason").and_then(Value::as_str) {
        parts.push(format!("reason={reason}"));
    }
    if let Some(ms) = item.get("recorded_ms").and_then(Value::as_u64) {
        parts.push(format!("recorded_ms={ms}"));
    }
    parts.join(" ")
}
fn value_to_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(ToString::to_string)
        .or_else(|| value.as_u64().map(|n| n.to_string()))
        .or_else(|| value.as_i64().map(|n| n.to_string()))
        .or_else(|| value.as_bool().map(|b| b.to_string()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn format_evidence_summary_includes_core_fields() {
        let mut map = norito::json::Map::new();
        map.insert("kind".to_owned(), Value::from("InvalidQc"));
        map.insert("height".to_owned(), Value::from(42u64));
        map.insert("view".to_owned(), Value::from(7u64));
        map.insert("epoch".to_owned(), Value::from(1u64));
        map.insert("reason".to_owned(), Value::from("shape mismatch"));
        map.insert("recorded_ms".to_owned(), Value::from(1234u64));
        map.insert("signer".to_owned(), Value::from(3u64));
        map.insert("class".to_owned(), Value::from("phase_vote"));
        map.insert("context_id".to_owned(), Value::from("AA".repeat(32)));
        map.insert("consensus_admitted_height".to_owned(), Value::from(43u64));
        let summary = format_evidence_summary(0, &Value::from(map));
        assert!(summary.contains("1: kind=InvalidQc"));
        assert!(summary.contains("height=42"));
        assert!(summary.contains("view=7"));
        assert!(summary.contains("epoch=1"));
        assert!(summary.contains("signer=3"));
        assert!(summary.contains("class=phase_vote"));
        assert!(summary.contains("context_id="));
        assert!(summary.contains("consensus_admitted_height=43"));
        assert!(summary.contains("recorded_ms=1234"));
        assert!(summary.contains("reason=shape mismatch"));
    }
    #[test]
    fn format_evidence_summary_uses_index_offset() {
        let mut map = norito::json::Map::new();
        map.insert("kind".to_owned(), Value::from("DoublePrepare"));
        let summary = format_evidence_summary(5, &Value::from(map));
        assert!(
            summary.starts_with("6: kind=DoublePrepare"),
            "unexpected summary: {summary}"
        );
    }
}
