#![allow(clippy::redundant_pub_crate, clippy::needless_pass_by_value)]
use super::commands::PacemakerArgs;
use crate::{CliOutputFormat, RunContext};
use eyre::Result;
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
