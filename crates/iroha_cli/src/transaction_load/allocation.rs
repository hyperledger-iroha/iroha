//! Closed public admission receipt and move-only per-writer reservations.
//!
//! The fixed worker re-admits the full ten-run budget. This owner checks its
//! projection against a launcher-supplied canonical identity, never reimplements
//! a weaker experiment decoder or accepts an unallocated output writer.

use super::*;

const ADMISSION_SCHEMA: &str = "iroha.sumeragi_v2.resource_probe.admission.v1";

pub(super) struct JournalAllocation {
    pub(super) _label: String,
    pub(super) max_bytes: usize,
}
pub(super) struct TraceAllocation {
    pub(super) _label: String,
    pub(super) max_bytes: usize,
}
pub(super) struct Writers {
    pub(super) journal: JournalAllocation,
    pub(super) trace: TraceAllocation,
}
#[derive(Clone)]
pub(super) struct Expected {
    digest: String,
    pair: u8,
    variant: &'static str,
    interval_ns: i64,
    measurement_ns: i64,
    drain_ns: i64,
}
impl Expected {
    pub(super) fn new(args: &Args, schedule: &Schedule, interval_ns: i64) -> Result<Self> {
        let digest = &args.resource.resource_budget_sha256;
        if !digest_valid(digest) {
            bail!("resource budget requires the trusted canonical SHA-256 identity");
        }
        Ok(Self {
            digest: digest.clone(),
            pair: args.pair_index,
            variant: args.variant.text(),
            interval_ns,
            measurement_ns: schedule.measurement_ns,
            drain_ns: schedule.drain_ns,
        })
    }
}
fn digest_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn exact<'a>(value: &'a Value, fields: &[&str]) -> Result<&'a norito::json::Map> {
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("resource admission object missing"))?;
    if object.len() != fields.len() || !fields.iter().all(|field| object.contains_key(*field)) {
        bail!("resource admission fields differ from the closed receipt");
    }
    Ok(object)
}
fn allocation(value: &Value) -> Result<(String, usize)> {
    let object = exact(value, &["label", "max_bytes"])?;
    let label = object["label"]
        .as_str()
        .ok_or_else(|| eyre!("resource allocation label missing"))?;
    if label.is_empty()
        || label.len() > 128
        || !label.as_bytes()[0].is_ascii_lowercase()
        || !label
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b))
    {
        bail!("resource allocation label is invalid");
    }
    let cap = object["max_bytes"]
        .as_u64()
        .and_then(|v| usize::try_from(v).ok())
        .filter(|v| (1..=MAX_FILE_BYTES).contains(v))
        .ok_or_else(|| eyre!("resource writer allocation exceeds its byte bound"))?;
    Ok((label.to_owned(), cap))
}
pub(super) fn parse(expected: &Expected, line: &[u8]) -> Result<Writers> {
    if line.is_empty()
        || line.len() > resource::MAX_IPC_BYTES
        || line.last() != Some(&b'\n')
        || line[..line.len() - 1].contains(&b'\n')
    {
        bail!("resource admission response is not one bounded line");
    }
    let value: Value =
        json::from_slice(line).map_err(|_| eyre!("resource admission JSON invalid"))?;
    let response = exact(
        &value,
        &["schema", "kind", "sequence", "outcome", "admission"],
    )?;
    if response["schema"].as_str() != Some(resource::RESPONSE_SCHEMA)
        || response["kind"].as_str() != Some("admit")
        || response["sequence"].as_u64() != Some(0)
        || response["outcome"].as_str() != Some("complete")
    {
        bail!("resource budget was not admitted");
    }
    let receipt = exact(
        &response["admission"],
        &[
            "schema",
            "budget_sha256",
            "pair_index",
            "variant",
            "geometry",
            "journal",
            "trace",
        ],
    )?;
    if receipt["schema"].as_str() != Some(ADMISSION_SCHEMA)
        || receipt["budget_sha256"].as_str() != Some(expected.digest.as_str())
        || receipt["pair_index"].as_u64() != Some(u64::from(expected.pair))
        || receipt["variant"].as_str() != Some(expected.variant)
    {
        bail!("resource admission does not match the trusted selected run");
    }
    let geometry = exact(
        &receipt["geometry"],
        &["peers", "interval_ns", "measurement_ns", "drain_ns"],
    )?;
    if !geometry["peers"]
        .as_u64()
        .is_some_and(|v| (4..=64).contains(&v))
        || geometry["interval_ns"].as_i64() != Some(expected.interval_ns)
        || geometry["measurement_ns"].as_i64() != Some(expected.measurement_ns)
        || geometry["drain_ns"].as_i64() != Some(expected.drain_ns)
    {
        bail!("resource admission changes the collector geometry");
    }
    let (journal_label, journal_cap) = allocation(&receipt["journal"])?;
    let (trace_label, trace_cap) = allocation(&receipt["trace"])?;
    if journal_label == trace_label {
        bail!("resource writer allocations duplicate a role label");
    }
    Ok(Writers {
        journal: JournalAllocation {
            _label: journal_label,
            max_bytes: journal_cap,
        },
        trace: TraceAllocation {
            _label: trace_label,
            max_bytes: trace_cap,
        },
    })
}

#[cfg(test)]
pub(super) mod tests;
