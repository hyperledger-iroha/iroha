//! Exact existing Android SDK ASCII snapshot preimage parser. No new digest/signing codec.
//! Mirrors AndroidAttestationRevocationPolicyV1 Java/Kotlin canonical bounds, ordered denies
//! and half-open freshness; the enclosing actual governance signature authenticates originals.
use super::MAX_STATUS;
#[derive(Debug)]
pub(super) struct AndroidSnapshot {
    pub(super) payload_sha256: [u8; 32],
    response_date_ms: u64,
    expires_at_ms: u64,
    pub(super) serials: Vec<String>,
    pub(super) tbs_digests: Vec<[u8; 32]>,
}
impl AndroidSnapshot {
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty()
            || bytes.len() > MAX_STATUS
            || bytes.last() != Some(&b'\n')
            || bytes
                .iter()
                .any(|b| *b != b'\n' && !(0x20..=0x7e).contains(b))
        {
            return Err("Android canonical snapshot ASCII/bound/newline".into());
        }
        let text =
            std::str::from_utf8(&bytes[..bytes.len() - 1]).map_err(|_| "Android snapshot UTF8")?;
        let mut lines = text.split('\n');
        if lines.next() != Some("iroha.android.attestation.revocation.snapshot.v1") {
            return Err("Android existing snapshot domain differs".into());
        }
        let mut next = |key: &str| -> Result<&str, String> {
            let line = lines.next().ok_or("Android snapshot missing field")?;
            let prefix = format!("{key}=");
            let value = line
                .strip_prefix(&prefix)
                .ok_or("Android snapshot field/order differs")?;
            if value.is_empty() {
                return Err("Android snapshot empty field".into());
            }
            Ok(value)
        };
        let payload_sha256 = hex_digest(next("payload_sha256")?)?;
        let response_date_ms = positive(next("response_date_ms")?)?;
        if response_date_ms % 1000 != 0 {
            return Err("Android original response whole seconds".into());
        }
        let modified = next("last_modified_ms")?;
        if modified != "-" {
            let m = positive(modified)?;
            if m % 1000 != 0 || m > response_date_ms {
                return Err("Android original modified bound".into());
            }
        }
        let age = positive(next("cache_max_age_seconds")?)?;
        if age > 86400 {
            return Err("Android original cache age bound".into());
        }
        let expires_at_ms = response_date_ms
            .checked_add(age.checked_mul(1000).ok_or("Android status age overflow")?)
            .filter(|n| *n <= i64::MAX as u64)
            .ok_or("Android status expiry overflow")?;
        let serial_count = count(next("serial_count")?, 4096)?;
        let mut serials: Vec<String> = Vec::with_capacity(serial_count);
        for _ in 0..serial_count {
            let s = next("serial")?;
            if !canonical_serial(s) || serials.last().is_some_and(|p| p.as_str() >= s) {
                return Err("Android original serial canonical/order differs".into());
            }
            serials.push(s.into());
        }
        let tbs_count = count(next("tbs_sha256_count")?, 256)?;
        let mut tbs_digests = Vec::with_capacity(tbs_count);
        for _ in 0..tbs_count {
            let d = hex_digest(next("tbs_sha256")?)?;
            if tbs_digests.last().is_some_and(|p| *p >= d) {
                return Err("Android original TBS digest order differs".into());
            }
            tbs_digests.push(d);
        }
        if lines.next().is_some() {
            return Err("Android snapshot trailing/empty field".into());
        }
        Ok(Self {
            payload_sha256,
            response_date_ms,
            expires_at_ms,
            serials,
            tbs_digests,
        })
    }
    pub(super) fn validate_at(&self, now: u64) -> Result<(), String> {
        if now < self.response_date_ms || now >= self.expires_at_ms {
            return Err("Android original status future/stale".into());
        }
        Ok(())
    }
}
pub(super) fn canonical_serial(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 40
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && (s.len() == 1 || !s.starts_with('0'))
}
fn decimal(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && (s.len() == 1 || !s.starts_with('0'))
}
fn positive(s: &str) -> Result<u64, String> {
    if !decimal(s) || s == "0" {
        return Err("Android positive decimal differs".into());
    }
    s.parse::<u64>()
        .ok()
        .filter(|n| *n <= i64::MAX as u64)
        .ok_or_else(|| "Android signed-long decimal overflow".into())
}
fn count(s: &str, max: usize) -> Result<usize, String> {
    if !decimal(s) {
        return Err("Android count decimal differs".into());
    }
    s.parse::<usize>()
        .ok()
        .filter(|n| *n <= max)
        .ok_or_else(|| "Android original count bound".into())
}
fn hex_digest(s: &str) -> Result<[u8; 32], String> {
    if s.len() != 64
        || !s
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("Android original lowercase digest differs".into());
    }
    let mut result = [0u8; 32];
    for (i, p) in s.as_bytes().chunks_exact(2).enumerate() {
        let val = |v: u8| if v <= b'9' { v - b'0' } else { v - b'a' + 10 };
        result[i] = val(p[0]) * 16 + val(p[1]);
    }
    if result == [0; 32] {
        return Err("Android original digest must not be zero".into());
    }
    Ok(result)
}
