#![allow(clippy::redundant_pub_crate, clippy::needless_pass_by_value)]

use std::fs;

use eyre::{Result, eyre};
use norito::json::Value;

use crate::RunContext;

use super::commands::{EvidenceCountArgs, EvidenceKindArg, EvidenceListArgs, EvidenceSubmitArgs};

pub(crate) fn list<C: RunContext>(context: &mut C, args: EvidenceListArgs) -> Result<()> {
    let client = context.client_from_config();
    let kind = args.kind.map(EvidenceKindArg::as_str);
    let filter = iroha::client::SumeragiEvidenceListFilter {
        limit: args.limit,
        offset: args.offset,
        kind,
    };
    let value = client.get_sumeragi_evidence_list_json(&filter)?;
    if args.summary || args.summary_only {
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
    }
    if !args.summary_only {
        context.print_data(&value)?;
    }
    Ok(())
}

pub(crate) fn count<C: RunContext>(context: &mut C, args: EvidenceCountArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_evidence_count_json()?;
    if args.summary || args.summary_only {
        let count = value
            .get("count")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        context.println(format!("count={count}"))?;
    }
    if !args.summary_only {
        context.print_data(&value)?;
    }
    Ok(())
}

pub(crate) fn submit<C: RunContext>(context: &mut C, args: EvidenceSubmitArgs) -> Result<()> {
    let hex = load_evidence_hex(&args)?;
    let client = context.client_from_config();
    let value = client.post_sumeragi_evidence_hex(&hex)?;
    if args.summary || args.summary_only {
        let status = value
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("accepted");
        let kind = value.get("kind").and_then(Value::as_str).unwrap_or("-");
        context.println(format!("submitted kind={kind} status={status}"))?;
    }
    if !args.summary_only {
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
        "phase",
        "height",
        "view",
        "epoch",
        "signer",
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

fn normalize_hex_input(raw: &str) -> String {
    raw.chars().filter(|c| !c.is_whitespace()).collect()
}

fn load_evidence_hex(args: &EvidenceSubmitArgs) -> Result<String> {
    match (&args.evidence_hex, &args.evidence_hex_file) {
        (Some(inline), None) => {
            let cleaned = normalize_hex_input(inline);
            if cleaned.is_empty() {
                Err(eyre!("--evidence-hex cannot be empty"))
            } else {
                Ok(cleaned)
            }
        }
        (None, Some(path)) => {
            let contents = fs::read_to_string(path).map_err(|err| {
                eyre!("failed to read evidence hex from {}: {err}", path.display())
            })?;
            let cleaned = normalize_hex_input(&contents);
            if cleaned.is_empty() {
                Err(eyre!(
                    "evidence hex file {} contains no data",
                    path.display()
                ))
            } else {
                Ok(cleaned)
            }
        }
        (Some(_), Some(_)) => Err(eyre!(
            "provide either --evidence-hex or --evidence-hex-file (not both)"
        )),
        (None, None) => Err(eyre!(
            "provide --evidence-hex or --evidence-hex-file with the Norito payload"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_evidence_hex_prefers_inline() {
        let args = EvidenceSubmitArgs {
            evidence_hex: Some(" 0xABCDEF ".to_string()),
            evidence_hex_file: None,
            summary: false,
            summary_only: false,
        };
        let loaded = load_evidence_hex(&args).expect("inline hex");
        assert_eq!(loaded, "0xABCDEF");
    }

    #[test]
    fn load_evidence_hex_reads_file() {
        let path = std::env::temp_dir().join("iroha_cli_evidence_test.hex");
        fs::write(&path, "aa bb cc").expect("write temp hex file");
        let args = EvidenceSubmitArgs {
            evidence_hex: None,
            evidence_hex_file: Some(path.clone()),
            summary: false,
            summary_only: false,
        };
        let loaded = load_evidence_hex(&args).expect("file hex");
        assert_eq!(loaded, "aabbcc");
        let _ = fs::remove_file(path);
    }

    #[test]
    fn format_evidence_summary_includes_core_fields() {
        let mut map = norito::json::Map::new();
        map.insert("kind".to_owned(), Value::from("InvalidCommitCertificate"));
        map.insert("height".to_owned(), Value::from(42u64));
        map.insert("view".to_owned(), Value::from(7u64));
        map.insert("epoch".to_owned(), Value::from(1u64));
        map.insert("reason".to_owned(), Value::from("shape mismatch"));
        map.insert("recorded_ms".to_owned(), Value::from(1234u64));
        map.insert("signer".to_owned(), Value::from(3u64));
        let summary = format_evidence_summary(0, &Value::from(map));
        assert!(summary.contains("1: kind=InvalidCommitCertificate"));
        assert!(summary.contains("height=42"));
        assert!(summary.contains("view=7"));
        assert!(summary.contains("epoch=1"));
        assert!(summary.contains("signer=3"));
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
