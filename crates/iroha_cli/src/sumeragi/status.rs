#![allow(clippy::redundant_pub_crate, clippy::needless_pass_by_value)]

use eyre::Result;
use norito::json::Value;

use crate::{CliOutputFormat, RunContext};

use super::commands::{DiagnosticsArgs, LeaderArgs, ParamsArgs, QcArgs, StatusArgs};

pub(crate) fn status<C: RunContext>(context: &mut C, _args: StatusArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_status_json()?;
    match context.output_format() {
        CliOutputFormat::Text => context.println(summarize_status(&value)),
        CliOutputFormat::Json => context.print_data(&value),
    }
}

pub(crate) fn diagnostics<C: RunContext>(context: &mut C, _args: DiagnosticsArgs) -> Result<()> {
    let client = context.client_from_config();
    let diagnostics = client.get_sumeragi_diagnostics()?;
    let value = norito::json::to_value(&diagnostics)?;
    match context.output_format() {
        CliOutputFormat::Text => context.println(summarize_diagnostics(&value)),
        CliOutputFormat::Json => context.print_data(&value),
    }
}

pub(crate) fn leader<C: RunContext>(context: &mut C, _args: LeaderArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_leader_json()?;
    match context.output_format() {
        CliOutputFormat::Text => context.println(summarize_leader(&value)),
        CliOutputFormat::Json => context.print_data(&value),
    }
}

pub(crate) fn params<C: RunContext>(context: &mut C, _args: ParamsArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_params_json()?;
    match context.output_format() {
        CliOutputFormat::Text => context.println(summarize_params(&value)),
        CliOutputFormat::Json => context.print_data(&value),
    }
}

pub(crate) fn qc<C: RunContext>(context: &mut C, _args: QcArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_qc_json()?;
    match context.output_format() {
        CliOutputFormat::Text => context.println(summarize_qc(&value)),
        CliOutputFormat::Json => context.print_data(&value),
    }
}

fn summarize_status(value: &Value) -> String {
    let protocol = value
        .get("protocol_version")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let height = value.get("height").and_then(Value::as_u64).unwrap_or(0);
    let view = value.get("view").and_then(Value::as_u64).unwrap_or(0);
    let leader = value.get("leader").and_then(Value::as_u64).unwrap_or(0);
    let committed = value
        .get("last_committed_height")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let phase = tagged_unit(value.get("phase"), "phase");
    let body = tagged_unit(value.get("body_state"), "state");
    let persistence = value
        .get("pending_persistence_id")
        .and_then(Value::as_u64)
        .map_or_else(|| "-".to_owned(), |id| id.to_string());
    let restart_required = value
        .get("restart_required")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    format!(
        "protocol={protocol} height={height} view={view} phase={phase} leader={leader} body={body} pending_persistence={persistence} last_committed={committed} restart_required={restart_required}"
    )
}

fn tagged_unit<'a>(value: Option<&'a Value>, tag: &str) -> &'a str {
    value
        .and_then(Value::as_object)
        .and_then(|object| object.get(tag))
        .and_then(Value::as_str)
        .or_else(|| value.and_then(Value::as_str))
        .unwrap_or("unknown")
}

fn summarize_diagnostics(value: &Value) -> String {
    let depth = value
        .get("tx_queue_depth")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let capacity = value
        .get("tx_queue_capacity")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let saturated = value
        .get("tx_queue_saturated")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let lanes = value
        .get("lane_commitments")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let relays = value
        .get("lane_relay_envelopes")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let sealed = value
        .get("lane_governance_sealed_total")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let npos = value.get("npos").and_then(Value::as_object);
    let election = npos.map_or_else(
        || "permissioned".to_owned(),
        |npos| {
            let epoch = npos
                .get("epoch_length_blocks")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let commit = npos
                .get("vrf_commit_deadline_offset")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let reveal = npos
                .get("vrf_reveal_deadline_offset")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            format!("npos(epoch={epoch},commit={commit},reveal={reveal})")
        },
    );
    format!(
        "queue={depth}/{capacity} saturated={saturated} election={election} lanes={lanes} relays={relays} sealed={sealed}"
    )
}

fn summarize_leader(value: &Value) -> String {
    let leader = value
        .get("leader_index")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let prf = value.get("prf").and_then(|v| v.as_object());
    let height = prf
        .and_then(|o| o.get("height"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let view = prf
        .and_then(|o| o.get("view"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let seed = prf
        .and_then(|o| o.get("epoch_seed"))
        .and_then(|v| v.as_str())
        .map_or("-", |s| if s.len() > 8 { &s[..8] } else { s });
    format!("leader={leader} prf_h={height} prf_v={view} seed={seed}")
}

fn summarize_params(value: &Value) -> String {
    let cadence = value
        .get("block_cadence_ms")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let drift = value
        .get("max_clock_drift_ms")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let height = value
        .get("chain_height")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    format!("block_cadence={cadence}ms max_clock_drift={drift}ms chain_height={height}")
}

fn summarize_qc(value: &Value) -> String {
    let hq = value.get("highest_qc").and_then(|v| v.as_object());
    let lq = value.get("locked_qc").and_then(|v| v.as_object());
    let hqc_height = hq
        .and_then(|o| o.get("height"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let hqc_view = hq
        .and_then(|o| o.get("view"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let subj = hq
        .and_then(|o| o.get("subject_block_hash"))
        .and_then(|v| v.as_str())
        .map_or("-", |s| if s.len() > 8 { &s[..8] } else { s });
    let lqc_height = lq
        .and_then(|o| o.get("height"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let lqc_view = lq
        .and_then(|o| o.get("view"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    format!("hqc={hqc_height}/{hqc_view} subj={subj} lqc={lqc_height}/{lqc_view}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_status_handles_defaults() {
        let value = norito::json!({
            "protocol_version": 2,
            "height": 7,
            "view": 3,
            "phase": { "phase": "prepare" },
            "leader": 4,
            "body_state": { "state": "validated" },
            "last_committed_height": 6,
            "restart_required": false
        });
        assert_eq!(
            summarize_status(&value),
            "protocol=2 height=7 view=3 phase=prepare leader=4 body=validated pending_persistence=- last_committed=6 restart_required=false"
        );
    }

    #[test]
    fn summarize_diagnostics_keeps_operator_state_separate() {
        let value = norito::json!({
            "tx_queue_depth": 4,
            "tx_queue_capacity": 10,
            "tx_queue_saturated": false,
            "npos": {
                "epoch_length_blocks": 100,
                "vrf_commit_deadline_offset": 20,
                "vrf_reveal_deadline_offset": 40
            },
            "lane_commitments": [{ "lane_id": 1 }],
            "lane_relay_envelopes": [],
            "lane_governance_sealed_total": 0
        });
        assert_eq!(
            summarize_diagnostics(&value),
            "queue=4/10 saturated=false election=npos(epoch=100,commit=20,reveal=40) lanes=1 relays=0 sealed=0"
        );
    }

    #[test]
    fn summarize_leader_truncates_seed() {
        let value = norito::json!({
            "leader_index": 1,
            "prf": {
                "height": 10,
                "view": 2,
                "epoch_seed": "0x1234567890abcdef"
            }
        });
        assert_eq!(
            summarize_leader(&value),
            "leader=1 prf_h=10 prf_v=2 seed=0x123456"
        );
    }

    #[test]
    fn summarize_params_reports_signed_cadence_and_height() {
        let value = norito::json!({
            "block_cadence_ms": 1000,
            "max_clock_drift_ms": 500,
            "chain_height": 42
        });
        assert_eq!(
            summarize_params(&value),
            "block_cadence=1000ms max_clock_drift=500ms chain_height=42"
        );
    }

    #[test]
    fn summarize_qc_reports_subject_hash() {
        let value = norito::json!({
            "highest_qc": {
                "height": 8,
                "view": 1,
                "subject_block_hash": "1234567890abcdef"
            },
            "locked_qc": {
                "height": 6,
                "view": 0
            }
        });
        assert_eq!(summarize_qc(&value), "hqc=8/1 subj=12345678 lqc=6/0");
    }
}
