#![allow(clippy::redundant_pub_crate, clippy::needless_pass_by_value)]

use eyre::Result;
use norito::json::Value;

use crate::{CliOutputFormat, RunContext};

use super::commands::{PacemakerArgs, PhasesArgs, TelemetryArgs};

pub(crate) fn pacemaker<C: RunContext>(context: &mut C, _args: PacemakerArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_pacemaker_json()?;
    if matches!(context.output_format(), CliOutputFormat::Text) {
        let backoff = value
            .get("backoff_ms")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let rtt = value
            .get("rtt_floor_ms")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let jitter = value
            .get("jitter_ms")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let mul = value
            .get("backoff_multiplier")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let rtt_mul = value
            .get("rtt_floor_multiplier")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let maxb = value
            .get("max_backoff_ms")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let jperm = value
            .get("jitter_frac_permille")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        context.println(format!(
            "backoff={backoff}ms rtt_floor={rtt}ms jitter={jitter}ms mul={mul}/{rtt_mul} max={maxb}ms jitter_permille={jperm}"
        ))
    } else {
        context.print_data(&value)
    }
}

pub(crate) fn phases<C: RunContext>(context: &mut C, _args: PhasesArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_phases_json()?;
    match context.output_format() {
        CliOutputFormat::Text => context.println(summarize_phases(&value)),
        CliOutputFormat::Json => context.print_data(&value),
    }
}

pub(crate) fn telemetry<C: RunContext>(context: &mut C, _args: TelemetryArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_telemetry_json()?;
    match context.output_format() {
        CliOutputFormat::Text => context.println(summarize_telemetry(&value)),
        CliOutputFormat::Json => context.print_data(&value),
    }
}

fn summarize_phases(v: &Value) -> String {
    let g = |k: &str| v.get(k).and_then(norito::json::Value::as_u64).unwrap_or(0);
    let pipeline_total = g("pipeline_total_ms");
    let base = format!(
        "propose={} da={} prevote={} precommit={} aggregator={} commit={} pipeline_total={} ms",
        g("propose_ms"),
        g("collect_da_ms"),
        g("collect_prevote_ms"),
        g("collect_precommit_ms"),
        g("collect_aggregator_ms"),
        g("commit_ms"),
        pipeline_total
    );
    let fallback = v
        .get("collect_aggregator_gossip_total")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let dropped = v
        .get("block_created_dropped_by_lock_total")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let hint = v
        .get("block_created_hint_mismatch_total")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let proposal = v
        .get("block_created_proposal_mismatch_total")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    v.get("ema_ms")
        .and_then(|x| x.as_object())
        .map_or_else(
            || {
                format!(
                    "{base} | gossip_fallbacks={fallback} | block_created_drops={dropped} | block_created_hint_mismatch={hint} | block_created_proposal_mismatch={proposal}"
                )
            },
            |ema| {
                let ge = |k: &str| {
                    ema.get(k)
                        .and_then(norito::json::Value::as_u64)
                        .unwrap_or(0)
                };
                let pipeline_total_ema = ge("pipeline_total_ms");
                format!(
                    "{base} | ema(propose={} da={} prevote={} precommit={} aggregator={} commit={} pipeline_total={}) | gossip_fallbacks={fallback} | block_created_drops={dropped} | block_created_hint_mismatch={hint} | block_created_proposal_mismatch={proposal}",
                    ge("propose_ms"),
                    ge("collect_da_ms"),
                    ge("collect_prevote_ms"),
                    ge("collect_precommit_ms"),
                    ge("collect_aggregator_ms"),
                    ge("commit_ms"),
                    pipeline_total_ema
                )
            },
        )
}

fn summarize_telemetry(v: &Value) -> String {
    let availability = v.get("availability").and_then(|x| x.as_object());
    let total_votes = availability
        .and_then(|o| o.get("total_votes_ingested"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let collectors = availability
        .and_then(|o| o.get("collectors"))
        .and_then(norito::json::Value::as_array)
        .map_or(0, Vec::len);
    let rbc = v.get("rbc_backlog").and_then(|x| x.as_object());
    let pending_sessions = rbc
        .and_then(|o| o.get("pending_sessions"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let vrf = v.get("vrf").and_then(|x| x.as_object());
    let vrf_epoch = vrf
        .and_then(|o| o.get("epoch"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let vrf_finalized = vrf
        .and_then(|o| o.get("finalized"))
        .and_then(norito::json::Value::as_bool)
        .unwrap_or(false);
    let reveals = vrf
        .and_then(|o| o.get("reveals_total"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let late = vrf
        .and_then(|o| o.get("late_reveals_total"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let penalties = vrf
        .and_then(|o| o.get("committed_no_reveal_total"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let no_participation = vrf
        .and_then(|o| o.get("no_participation_total"))
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    format!(
        "availability_votes={total_votes} collectors={collectors} rbc_pending_sessions={pending_sessions} vrf_epoch={vrf_epoch} vrf_finalized={vrf_finalized} reveals={reveals} late_reveals={late} committed_no_reveal={penalties} no_participation={no_participation}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_phases_includes_ema_when_present() {
        let value = norito::json!({
            "propose_ms": 10,
            "collect_da_ms": 20,
            "collect_prevote_ms": 30,
            "collect_precommit_ms": 40,
            "collect_aggregator_ms": 50,
            "commit_ms": 80,
            "collect_aggregator_gossip_total": 2,
            "block_created_dropped_by_lock_total": 3,
            "block_created_hint_mismatch_total": 4,
            "block_created_proposal_mismatch_total": 5,
            "ema_ms": {
                "propose_ms": 11,
                "collect_da_ms": 21,
                "collect_prevote_ms": 31,
                "collect_precommit_ms": 41,
                "collect_aggregator_ms": 51,
                "commit_ms": 81
            }
        });
        assert_eq!(
            summarize_phases(&value),
            "propose=10 da=20 prevote=30 precommit=40 aggregator=50 commit=80 pipeline_total=0 ms | ema(propose=11 da=21 prevote=31 precommit=41 aggregator=51 commit=81 pipeline_total=0) | gossip_fallbacks=2 | block_created_drops=3 | block_created_hint_mismatch=4 | block_created_proposal_mismatch=5"
        );
    }

    #[test]
    fn summarize_telemetry_reports_key_counters() {
        let value = norito::json!({
            "availability": {
                "total_votes_ingested": 100,
                "collectors": [1, 2, 3]
            },
            "rbc_backlog": {
                "pending_sessions": 4
            },
            "vrf": {
                "epoch": 5,
                "finalized": true,
                "reveals_total": 6,
                "late_reveals_total": 7,
                "committed_no_reveal_total": 8,
                "no_participation_total": 9
            }
        });
        assert_eq!(
            summarize_telemetry(&value),
            "availability_votes=100 collectors=3 rbc_pending_sessions=4 vrf_epoch=5 vrf_finalized=true reveals=6 late_reveals=7 committed_no_reveal=8 no_participation=9"
        );
    }
}
