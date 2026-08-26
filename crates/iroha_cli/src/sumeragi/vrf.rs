#![allow(clippy::redundant_pub_crate, clippy::needless_pass_by_value)]
use super::commands::{VrfEpochArgs, VrfPenaltiesArgs};
use crate::{CliOutputFormat, RunContext};
use eyre::{Result, eyre};
pub(crate) fn penalties<C: RunContext>(context: &mut C, args: VrfPenaltiesArgs) -> Result<()> {
    let client = context.client_from_config();
    let epoch = parse_epoch(&args.epoch)?;
    let value = client.get_sumeragi_vrf_penalties_json(epoch)?;
    if matches!(context.output_format(), CliOutputFormat::Text) {
        let ep = value
            .get("epoch")
            .and_then(norito::json::Value::as_u64)
            .ok_or_else(|| eyre!("VRF penalties report is missing numeric `epoch`"))?;
        let roster = value
            .get("roster_len")
            .and_then(norito::json::Value::as_u64)
            .ok_or_else(|| eyre!("VRF penalties report is missing numeric `roster_len`"))?;
        let committed_no_reveal = value
            .get("committed_no_reveal")
            .and_then(|v| v.as_array())
            .ok_or_else(|| eyre!("VRF penalties report is missing `committed_no_reveal`"))?
            .len();
        let no_participation = value
            .get("no_participation")
            .and_then(|v| v.as_array())
            .ok_or_else(|| eyre!("VRF penalties report is missing `no_participation`"))?
            .len();
        if ep != epoch {
            return Err(eyre!(
                "VRF penalties report epoch {ep} does not match requested epoch {epoch}"
            ));
        }
        context.println(format!(
            "epoch={ep} roster={roster} committed_no_reveal={committed_no_reveal} no_participation={no_participation}"
        ))?;
    } else {
        context.print_data(&value)?;
    }
    Ok(())
}
pub(crate) fn epoch<C: RunContext>(context: &mut C, args: VrfEpochArgs) -> Result<()> {
    let client = context.client_from_config();
    let epoch = parse_epoch(&args.epoch)?;
    let value = client.get_sumeragi_vrf_epoch_json(epoch)?;
    if matches!(context.output_format(), CliOutputFormat::Text) {
        let found = value
            .get("found")
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        if !found {
            context.println(format!("epoch={epoch} not found"))?;
            return Ok(());
        }
        let finalized = value
            .get("finalized")
            .and_then(norito::json::Value::as_bool)
            .unwrap_or(false);
        let roster_len = value
            .get("roster_len")
            .and_then(norito::json::Value::as_u64)
            .unwrap_or(0);
        let participants = value
            .get("participants")
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        let seed_hex = value
            .get("seed_hex")
            .and_then(|v| v.as_str())
            .unwrap_or("null");
        context.println(format!(
            "epoch={epoch} finalized={finalized} roster={roster_len} participants={participants} seed={seed_hex}"
        ))?;
    } else {
        context.print_data(&value)?;
    }
    Ok(())
}
fn parse_epoch(raw: &str) -> Result<u64> {
    raw.strip_prefix("0x")
        .map_or_else(|| raw.parse::<u64>(), |rest| u64::from_str_radix(rest, 16))
        .map_err(|_| {
            eyre!("invalid epoch `{raw}`: expected decimal or 0x-prefixed hexadecimal u64")
        })
}
#[cfg(test)]
mod tests {
    use super::parse_epoch;
    #[test]
    fn parse_epoch_accepts_decimal() {
        assert_eq!(parse_epoch("42").expect("decimal epoch"), 42);
    }
    #[test]
    fn parse_epoch_accepts_hex_with_prefix() {
        assert_eq!(parse_epoch("0xff").expect("hex epoch"), 255);
    }
    #[test]
    fn parse_epoch_rejects_invalid_input() {
        assert!(parse_epoch("not-a-number").is_err());
    }
}
