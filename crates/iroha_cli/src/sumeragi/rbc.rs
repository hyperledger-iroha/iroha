#![allow(clippy::redundant_pub_crate, clippy::needless_pass_by_value)]

use eyre::Result;
use norito::json::Value;

use crate::RunContext;

use super::commands::{RbcCommand, RbcSessionsArgs, RbcStatusArgs};

pub(crate) fn run<C: RunContext>(context: &mut C, cmd: RbcCommand) -> Result<()> {
    match cmd {
        RbcCommand::Status(args) => status(context, args),
        RbcCommand::Sessions(args) => sessions(context, args),
    }
}

fn status<C: RunContext>(context: &mut C, args: RbcStatusArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_rbc_status_json()?;
    if args.summary {
        context.println(summarize_rbc_status(&value))
    } else {
        context.print_data(&value)
    }
}

fn sessions<C: RunContext>(context: &mut C, args: RbcSessionsArgs) -> Result<()> {
    let client = context.client_from_config();
    let value = client.get_sumeragi_rbc_sessions_json()?;
    if args.summary {
        context.println(summarize_rbc_sessions(&value))
    } else {
        context.print_data(&value)
    }
}

fn summarize_rbc_status(v: &Value) -> String {
    let g = |k: &str| v.get(k).and_then(norito::json::Value::as_u64).unwrap_or(0);
    format!(
        "active={} pruned={} ready={} deliver={} bytes={} skip_payload={} skip_ready={}",
        g("sessions_active"),
        g("sessions_pruned_total"),
        g("ready_broadcasts_total"),
        g("deliver_broadcasts_total"),
        g("payload_bytes_delivered_total"),
        g("payload_rebroadcasts_skipped_total"),
        g("ready_rebroadcasts_skipped_total")
    )
}

fn summarize_rbc_sessions(v: &Value) -> String {
    let items = v
        .get("items")
        .and_then(norito::json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let active = v
        .get("sessions_active")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let head = items.first().cloned().unwrap_or(norito::json!({}));
    let h = head
        .get("height")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let view = head
        .get("view")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let hash = head
        .get("block_hash")
        .and_then(|x| x.as_str())
        .map_or("-", |s| if s.len() > 8 { &s[..8] } else { s });
    let ready = head
        .get("ready_count")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let tot = head
        .get("total_chunks")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let got = head
        .get("received_chunks")
        .and_then(norito::json::Value::as_u64)
        .unwrap_or(0);
    let del = head
        .get("delivered")
        .and_then(norito::json::Value::as_bool)
        .unwrap_or(false);
    let invalid = head
        .get("invalid")
        .and_then(norito::json::Value::as_bool)
        .unwrap_or(false);
    let items_len = items.len();
    format!(
        "active={active} first=[hash:{hash} h:{h} v:{view} chunks={got}/{tot} ready={ready} delivered={del} invalid={invalid}] items={items_len}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_rbc_status_reports_totals() {
        let value = norito::json!({
            "sessions_active": 3,
            "sessions_pruned_total": 4,
            "ready_broadcasts_total": 5,
            "deliver_broadcasts_total": 6,
            "payload_bytes_delivered_total": 7,
            "payload_rebroadcasts_skipped_total": 8,
            "ready_rebroadcasts_skipped_total": 9
        });
        assert_eq!(
            summarize_rbc_status(&value),
            "active=3 pruned=4 ready=5 deliver=6 bytes=7 skip_payload=8 skip_ready=9"
        );
    }

    #[test]
    fn summarize_rbc_sessions_picks_first_entry() {
        let value = norito::json!({
            "sessions_active": 2,
            "items": [
                {
                    "block_hash": "0123456789abcdef",
                    "height": 9,
                    "view": 1,
                    "ready_count": 3,
                    "total_chunks": 10,
                    "received_chunks": 8,
                    "delivered": true,
                    "invalid": false
                },
                { "block_hash": "deadbeef" }
            ]
        });
        assert_eq!(
            summarize_rbc_sessions(&value),
            "active=2 first=[hash:01234567 h:9 v:1 chunks=8/10 ready=3 delivered=true invalid=false] items=2"
        );
    }

    #[test]
    fn summarize_rbc_sessions_handles_empty_list() {
        let value = norito::json!({
            "sessions_active": 0,
            "items": []
        });
        assert_eq!(
            summarize_rbc_sessions(&value),
            "active=0 first=[hash:- h:0 v:0 chunks=0/0 ready=0 delivered=false invalid=false] items=0"
        );
    }
}
