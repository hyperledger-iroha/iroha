#![allow(clippy::redundant_pub_crate, clippy::needless_pass_by_value)]

use eyre::Result;
use norito::json::Value;

use crate::RunContext;

use super::commands::{CollectorsArgs, LeaderArgs, ParamsArgs, QcArgs, StatusArgs};

pub(crate) fn status<C: RunContext>(context: &mut C, args: StatusArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_status_json()?;
    if args.summary {
        context.println(summarize_status(&value))
    } else {
        context.print_data(&value)
    }
}

pub(crate) fn leader<C: RunContext>(context: &mut C, args: LeaderArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_leader_json()?;
    if args.summary {
        context.println(summarize_leader(&value))
    } else {
        context.print_data(&value)
    }
}

pub(crate) fn params<C: RunContext>(context: &mut C, args: ParamsArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_params_json()?;
    if args.summary {
        context.println(summarize_params(&value))
    } else {
        context.print_data(&value)
    }
}

pub(crate) fn collectors<C: RunContext>(context: &mut C, args: CollectorsArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_collectors_json()?;
    if args.summary {
        context.println(summarize_collectors(&value))
    } else {
        context.print_data(&value)
    }
}

pub(crate) fn qc<C: RunContext>(context: &mut C, args: QcArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_qc_json()?;
    if args.summary {
        context.println(summarize_qc(&value))
    } else {
        context.print_data(&value)
    }
}

#[allow(clippy::too_many_lines)]
fn summarize_status(value: &Value) -> String {
    let leader = value
        .get("leader_index")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
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
    let lqc_height = lq
        .and_then(|o| o.get("height"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let lqc_view = lq
        .and_then(|o| o.get("view"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let lqc_subject = lq
        .and_then(|o| o.get("subject_block_hash"))
        .and_then(|v| v.as_str())
        .map_or_else(
            || "-".to_string(),
            |s| {
                let mut t = s.to_string();
                if t.len() > 8 {
                    t.truncate(8);
                }
                t
            },
        );
    let subj = hq
        .and_then(|o| o.get("subject_block_hash"))
        .and_then(|v| v.as_str())
        .map_or_else(
            || "-".to_string(),
            |s| {
                let mut t = s.to_string();
                if t.len() > 8 {
                    t.truncate(8);
                }
                t
            },
        );
    let gossip = value
        .get("gossip_fallback_total")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let dropped = value
        .get("block_created_dropped_by_lock_total")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let hint = value
        .get("block_created_hint_mismatch_total")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let proposal = value
        .get("block_created_proposal_mismatch_total")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let epoch_obj = value.get("epoch").and_then(|v| v.as_object());
    let epoch_length = epoch_obj
        .and_then(|o| o.get("length_blocks"))
        .and_then(norito::json::Value::as_u64)
        .or_else(|| {
            value
                .get("epoch_length_blocks")
                .and_then(norito::json::Value::as_u64)
        })
        .unwrap_or(0);
    let epoch_commit_offset = epoch_obj
        .and_then(|o| o.get("commit_deadline_offset"))
        .and_then(norito::json::Value::as_u64)
        .or_else(|| {
            value
                .get("epoch_commit_deadline_offset")
                .and_then(norito::json::Value::as_u64)
        })
        .unwrap_or(0);
    let epoch_reveal_offset = epoch_obj
        .and_then(|o| o.get("reveal_deadline_offset"))
        .and_then(norito::json::Value::as_u64)
        .or_else(|| {
            value
                .get("epoch_reveal_deadline_offset")
                .and_then(norito::json::Value::as_u64)
        })
        .unwrap_or(0);
    let vrf_epoch = value
        .get("vrf_penalty_epoch")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let vrf_non_reveal = value
        .get("vrf_committed_no_reveal_total")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let vrf_no_participation = value
        .get("vrf_no_participation_total")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let vrf_late = value
        .get("vrf_late_reveals_total")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let rbc = value.get("rbc_store").and_then(|v| v.as_object());
    let rbc_evictions = rbc
        .and_then(|o| o.get("evictions_total"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let rbc_persist_drops = rbc
        .and_then(|o| o.get("persist_drops_total"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let da_gate = value.get("da_gate").and_then(|v| v.as_object());
    let da_reason = da_gate
        .and_then(|o| o.get("reason"))
        .and_then(norito::json::Value::as_str)
        .unwrap_or("none");
    let da_last_satisfied = da_gate
        .and_then(|o| o.get("last_satisfied"))
        .and_then(norito::json::Value::as_str)
        .unwrap_or("none");
    let da_missing_local_data = da_gate
        .and_then(|o| o.get("missing_local_data_total"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let da_gate_summary = format!(
        "{da_reason}(last={da_last_satisfied};missing={da_missing_local_data})"
    );
    let rbc_sessions = rbc
        .and_then(|o| o.get("sessions"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let rbc_bytes = rbc
        .and_then(|o| o.get("bytes"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let rbc_pressure = rbc
        .and_then(|o| o.get("pressure_level"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let (rbc_last_hash, rbc_last_height, rbc_last_view) = rbc
        .and_then(|o| o.get("recent_evictions"))
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|entry| entry.as_object())
        .map_or_else(
            || ("-".to_string(), 0, 0),
            |entry| {
                let hash = entry
                    .get("block_hash")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let truncated = if hash.len() > 8 { &hash[..8] } else { hash };
                let height = entry
                    .get("height")
                    .and_then(norito::json::Value::as_u64)
                    .unwrap_or(0);
                let view = entry
                    .get("view")
                    .and_then(norito::json::Value::as_u64)
                    .unwrap_or(0);
                (truncated.to_string(), height, view)
            },
        );
    let da_resched = value
        .get("da_reschedule_total")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let membership = value.get("membership").and_then(|v| v.as_object());
    let membership_height = membership
        .and_then(|o| o.get("height"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let membership_view = membership
        .and_then(|o| o.get("view"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let membership_epoch = membership
        .and_then(|o| o.get("epoch"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let membership_hash = membership
        .and_then(|o| o.get("view_hash"))
        .and_then(|v| v.as_str())
        .map_or_else(
            || "-".to_string(),
            |s| {
                let mut t = s.to_string();
                if t.len() > 8 {
                    t.truncate(8);
                }
                t
            },
        );
    let sealed_total = value
        .get("lane_governance_sealed_total")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let sealed_aliases = value
        .get("lane_governance_sealed_aliases")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| entry.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let aliases = if sealed_aliases.is_empty() {
        "-".to_string()
    } else {
        sealed_aliases.join(",")
    };
    let settlement = value.get("settlement").and_then(|v| v.as_object());
    let dvp = summarize_settlement_phase(settlement.and_then(|o| o.get("dvp")));
    let pvp = summarize_settlement_phase(settlement.and_then(|o| o.get("pvp")));
    format!(
        "leader={leader} hqc={hqc_height}/{hqc_view} subj={subj} lqc={lqc_height}/{lqc_view} subj={lqc_subject} gossip={gossip} drop={dropped} hint={hint} proposal={proposal} da_resched={da_resched} da_gate={da_gate_summary} epoch_len={epoch_length} epoch_commit={epoch_commit_offset} epoch_reveal={epoch_reveal_offset} vrf_epoch={vrf_epoch} vrf_late={vrf_late} vrf_non_reveal={vrf_non_reveal} vrf_no_part={vrf_no_participation} membership={membership_height}/{membership_view}/{membership_epoch} hash={membership_hash} rbc_sessions={rbc_sessions} rbc_bytes={rbc_bytes} rbc_evictions={rbc_evictions} rbc_persist_drops={rbc_persist_drops} rbc_pressure={rbc_pressure} rbc_last={rbc_last_hash}@{rbc_last_height}/{rbc_last_view} sealed={sealed_total} aliases=[{aliases}] dvp={dvp} pvp={pvp}"
    )
}

fn summarize_settlement_phase(entry: Option<&Value>) -> String {
    let Some(object) = entry.and_then(|v| v.as_object()) else {
        return "none".to_string();
    };
    let Some(last_event) = object.get("last_event").and_then(|v| v.as_object()) else {
        return "none".to_string();
    };
    let final_state = last_event
        .get("final_state")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let outcome = last_event
        .get("outcome")
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let mut tags = Vec::new();
    match outcome {
        "failure" => {
            if let Some(reason) = last_event.get("failure_reason").and_then(|v| v.as_str()) {
                tags.push(format!("failure:{reason}"));
            } else {
                tags.push("failure".to_string());
            }
        }
        other => tags.push(other.to_string()),
    }
    if let Some(fx_ms) = last_event
        .get("fx_window_ms")
        .and_then(norito::json::Value::as_u64)
    {
        tags.push(format!("fx={fx_ms}ms"));
    }
    let failure_total = object
        .get("failure_total")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    if failure_total > 0 {
        tags.push(format!("fail={failure_total}"));
    }
    format!("{final_state}({})", tags.join(";"))
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
    format!("bt={bt}ms ct={ct}ms k={k} r={r} da_enabled={da_enabled} next_mode={mode} act_height={act}")
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
    fn summarize_status_handles_defaults() {
        let value = norito::json!({
            "leader_index": 4,
            "highest_qc": {
                "height": 7,
                "view": 3,
                "subject_block_hash": "0xABCDEF01"
            },
            "locked_qc": {
                "height": 5,
                "view": 2
            },
            "rbc_store": {
                "evictions_total": 1,
                "persist_drops_total": 3,
                "sessions": 2,
                "bytes": 3,
                "pressure_level": 4,
                "recent_evictions": [{
                    "block_hash": "feedfacecafebeef",
                    "height": 10,
                    "view": 11
                }]
            },
            "gossip_fallback_total": 8,
            "block_created_dropped_by_lock_total": 9,
            "block_created_hint_mismatch_total": 6,
            "block_created_proposal_mismatch_total": 5,
            "vrf_penalty_epoch": 12,
            "vrf_committed_no_reveal_total": 13,
            "vrf_no_participation_total": 14,
            "vrf_late_reveals_total": 15
        });
        assert_eq!(
            summarize_status(&value),
            "leader=4 hqc=7/3 subj=0xABCDEF lqc=5/2 subj=- gossip=8 drop=9 hint=6 proposal=5 da_resched=0 da_gate=none(last=none;missing=0) epoch_len=0 epoch_commit=0 epoch_reveal=0 vrf_epoch=12 vrf_late=15 vrf_non_reveal=13 vrf_no_part=14 membership=0/0/0 hash=- rbc_sessions=2 rbc_bytes=3 rbc_evictions=1 rbc_persist_drops=3 rbc_pressure=4 rbc_last=feedface@10/11 sealed=0 aliases=[-] dvp=none pvp=none"
        );
    }

    #[test]
    fn summarize_status_reports_epoch_offsets() {
        let value = norito::json!({
            "leader_index": 1,
            "highest_qc": { "height": 2, "view": 1 },
            "locked_qc": { "height": 1, "view": 0 },
            "gossip_fallback_total": 0,
            "block_created_dropped_by_lock_total": 0,
            "block_created_hint_mismatch_total": 0,
            "block_created_proposal_mismatch_total": 0,
            "vrf_penalty_epoch": 0,
            "vrf_committed_no_reveal_total": 0,
            "vrf_no_participation_total": 0,
            "vrf_late_reveals_total": 0,
            "rbc_store": {
                "evictions_total": 0,
                "sessions": 0,
                "bytes": 0,
                "pressure_level": 0,
                "recent_evictions": []
            },
            "epoch": {
                "length_blocks": 3_600,
                "commit_deadline_offset": 120,
                "reveal_deadline_offset": 160
            }
        });
        let summary = summarize_status(&value);
        assert!(
            summary.contains("epoch_len=3600"),
            "epoch length missing in summary: {summary}"
        );
        assert!(
            summary.contains("epoch_commit=120"),
            "epoch commit offset missing in summary: {summary}"
        );
        assert!(
            summary.contains("epoch_reveal=160"),
            "epoch reveal offset missing in summary: {summary}"
        );
    }

    #[test]
    fn summarize_status_reports_settlement_finality() {
        let value = norito::json!({
            "leader_index": 0,
            "highest_qc": { "height": 1, "view": 0 },
            "locked_qc": { "height": 0, "view": 0 },
            "gossip_fallback_total": 0,
            "block_created_dropped_by_lock_total": 0,
            "block_created_hint_mismatch_total": 0,
            "block_created_proposal_mismatch_total": 0,
            "vrf_penalty_epoch": 0,
            "vrf_committed_no_reveal_total": 0,
            "vrf_no_participation_total": 0,
            "vrf_late_reveals_total": 0,
            "rbc_store": {
                "evictions_total": 0,
                "sessions": 0,
                "bytes": 0,
                "pressure_level": 0,
                "recent_evictions": []
            },
            "settlement": {
                "dvp": {
                    "success_total": 2,
                    "failure_total": 1,
                    "final_state_totals": { "delivery_only": 1 },
                    "failure_reasons": {},
                    "last_event": {
                        "observed_at_ms": 123,
                        "settlement_id": "trade-1",
                        "plan": { "order": "delivery_then_payment", "atomicity": "commit_first_leg" },
                        "outcome": "success",
                        "failure_reason": null,
                        "final_state": "delivery_only",
                        "legs": { "delivery_committed": true, "payment_committed": false }
                    }
                },
                "pvp": {
                    "success_total": 0,
                    "failure_total": 3,
                    "final_state_totals": { "primary_only": 2 },
                    "failure_reasons": { "insufficient_funds": 2 },
                    "last_event": {
                        "observed_at_ms": 456,
                        "settlement_id": "fx-1",
                        "plan": { "order": "payment_then_delivery", "atomicity": "commit_first_leg" },
                        "outcome": "failure",
                        "failure_reason": "insufficient_funds",
                        "final_state": "primary_only",
                        "legs": { "primary_committed": true, "counter_committed": false },
                        "fx_window_ms": 99
                    }
                }
            }
        });
        let summary = summarize_status(&value);
        assert!(
            summary.contains("dvp=delivery_only(success;fail=1)"),
            "missing dvp summary: {summary}"
        );
        assert!(
            summary.contains("pvp=primary_only(failure:insufficient_funds;fx=99ms;fail=3)"),
            "missing pvp summary: {summary}"
        );
    }

    #[test]
    fn summarize_status_reports_da_gate_reason_and_counts() {
        let value = norito::json!({
            "leader_index": 2,
            "highest_qc": { "height": 3, "view": 1, "subject_block_hash": "0x1" },
            "locked_qc": { "height": 2, "view": 1 },
            "gossip_fallback_total": 0,
            "block_created_dropped_by_lock_total": 0,
            "block_created_hint_mismatch_total": 0,
            "block_created_proposal_mismatch_total": 0,
            "vrf_penalty_epoch": 0,
            "vrf_committed_no_reveal_total": 0,
            "vrf_no_participation_total": 0,
            "vrf_late_reveals_total": 0,
            "rbc_store": {
                "evictions_total": 0,
                "sessions": 1,
                "bytes": 128,
                "pressure_level": 0,
                "recent_evictions": []
            },
            "da_gate": {
                "reason": "missing_local_data",
                "last_satisfied": "missing_data_recovered",
                "missing_local_data_total": 4
            }
        });
        let summary = summarize_status(&value);
        assert!(
            summary.contains("da_gate=missing_local_data(last=missing_data_recovered;missing=4)"),
            "DA availability summary missing or malformed: {summary}"
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
