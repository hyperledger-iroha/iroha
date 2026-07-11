#![allow(clippy::redundant_pub_crate, clippy::needless_pass_by_value)]

use eyre::Result;
use iroha::data_model::block::consensus_v2::{QuorumCertificateRef, SumeragiV2StatusResponse};
use norito::json::Value;

use crate::{CliOutputFormat, RunContext};

use super::commands::{CollectorsArgs, LeaderArgs, ParamsArgs, QcArgs, StatusArgs};

pub(crate) fn status<C: RunContext>(context: &mut C, _args: StatusArgs) -> Result<()> {
    let client = context.client_from_config();
    let status = client.get_sumeragi_v2_status()?;
    match context.output_format() {
        CliOutputFormat::Text => context.println(summarize_status(&status)),
        CliOutputFormat::Json => context.print_data(&status),
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

pub(crate) fn collectors<C: RunContext>(context: &mut C, _args: CollectorsArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_collectors_json()?;
    match context.output_format() {
        CliOutputFormat::Text => context.println(summarize_collectors(&value)),
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

fn summarize_qc_ref(reference: Option<QuorumCertificateRef>) -> String {
    reference.map_or_else(
        || "none".to_owned(),
        |certificate| {
            format!(
                "{:?}@{}/{}:{}",
                certificate.phase,
                certificate.round.height,
                certificate.round.view,
                certificate.subject.block_hash
            )
        },
    )
}

fn summarize_status(status: &SumeragiV2StatusResponse) -> String {
    let authoritative = &status.authoritative;
    let context = authoritative.height_context;
    let operator = status.operator;
    let queue = operator.tx_queue;
    let commit = authoritative.last_commit_qc.map_or_else(
        || "none".to_owned(),
        |certificate| {
            format!(
                "{}@{}/{} signers={}/{} power={}/{}",
                certificate.certificate.subject.block_hash,
                certificate.certificate.round.height,
                certificate.certificate.round.view,
                certificate.signer_count,
                certificate.min_signers,
                certificate.signed_power,
                certificate.total_power
            )
        },
    );
    let incomplete_sessions = status
        .lane_block_sessions
        .iter()
        .filter(|session| !session.has_commit_qc || session.pending_committed_session_drain)
        .count();
    format!(
        "protocol={} height={}/{} view={} phase={:?} body={:?} mode={:?} epoch={}..{} leader={}/{} quorum={}/{} locked={} highest={} commit={} queue={}/{} bytes={}/{} age={}ms saturated={} view_changes={} busy_deferrals={} lanes=settlement:{}/relay:{}/ownership:{}/committed:{}/sessions:{} incomplete:{} local_peer_removed={}",
        authoritative.protocol_version,
        authoritative.last_committed_height,
        authoritative.height,
        authoritative.view,
        authoritative.phase,
        authoritative.body_state,
        context.mode,
        context.epoch,
        context.epoch_end_height,
        authoritative.leader,
        context.validator_count,
        context.quorum.min_signers,
        context.quorum.total_power,
        summarize_qc_ref(authoritative.locked_prepare_qc),
        summarize_qc_ref(authoritative.highest_prepare_qc),
        commit,
        queue.queued_transactions,
        queue.capacity,
        queue.retained_bytes,
        queue.max_retained_bytes,
        queue.oldest_queued_age_ms,
        queue.is_saturated(),
        operator.view_change_install_total,
        operator.busy_deferral_total,
        status.lane_settlement_commitments.len(),
        status.lane_relay_envelopes.len(),
        status.lane_payload_ownerships.len(),
        status.committed_lane_blocks.len(),
        status.lane_block_sessions.len(),
        incomplete_sessions,
        status.local_peer_removed,
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
    let k = value
        .get("collectors_k")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let r = value
        .get("redundant_send_r")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let da_enabled = value
        .get("da_enabled")
        .and_then(norito::json::Value::as_bool)
        .unwrap_or(false);
    let bt = value
        .get("block_time_ms")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let ct = value
        .get("commit_time_ms")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let mode = value
        .get("next_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let act = value
        .get("mode_activation_height")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    format!(
        "bt={bt}ms ct={ct}ms k={k} r={r} da_enabled={da_enabled} next_mode={mode} act_height={act}"
    )
}

fn summarize_collectors(value: &Value) -> String {
    let n = value
        .get("topology_len")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let mv = value
        .get("min_votes_for_commit")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let tail = value
        .get("proxy_tail_index")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let k = value
        .get("collectors_k")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let r = value
        .get("redundant_send_r")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let cols = value
        .get("collectors")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let list = cols
        .iter()
        .filter_map(|it| {
            let idx = it.get("index").and_then(norito::json::Value::as_u64)?;
            let pid = it.get("peer_id").and_then(|x| x.as_str())?;
            Some(format!("{idx}:{pid}"))
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("n={n} min_votes={mv} tail={tail} k={k} r={r} collectors=[{list}]")
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
    fn summarize_qc_ref_reports_absence_without_fabricating_state() {
        assert_eq!(summarize_qc_ref(None), "none");
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
    fn summarize_params_reports_all_knobs() {
        let value = norito::json!({
            "collectors_k": 3,
            "redundant_send_r": 2,
            "da_enabled": true,
            "block_time_ms": 1000,
            "commit_time_ms": 1500,
            "next_mode": "npos",
            "mode_activation_height": 42
        });
        assert_eq!(
            summarize_params(&value),
            "bt=1000ms ct=1500ms k=3 r=2 da_enabled=true next_mode=npos act_height=42"
        );
    }

    #[test]
    fn summarize_collectors_formats_list() {
        let value = norito::json!({
            "topology_len": 6,
            "min_votes_for_commit": 4,
            "proxy_tail_index": 3,
            "collectors_k": 2,
            "redundant_send_r": 1,
            "collectors": [
                { "index": 3, "peer_id": "peer#0" },
                { "index": 4, "peer_id": "peer#1" }
            ]
        });
        assert_eq!(
            summarize_collectors(&value),
            "n=6 min_votes=4 tail=3 k=2 r=1 collectors=[3:peer#0,4:peer#1]"
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
