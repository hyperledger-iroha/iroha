//! Bounded SDK fixture capture from an already completed native execution export.
//!
//! This owner does not synthesize blocks, signatures, lane_evidence, results or proof
//! verdicts. Exact canonical artifact bytes and rows come from `VerifiedExport`.
//! Consumers still need the independently pinned launch plan to authenticate a
//! received artifact; a fixture document is not a new source of trust.

use super::*;

const PREFIX: &[u8] = b"{\"version\":1,\"artifact_schema\":\"iroha_kagami::scaling_evidence::ExportEnvelopeV1\",\"artifact_hash\":";
const PROOF_KEY: &[u8] = b",\"canonical_artifact_hex\":\"";
const ROWS_KEY: &[u8] = b"\",\"requests\":";
const HEX: &[u8; 16] = b"0123456789abcdef";

impl VerifiedExport {
    /// Capture exact complete native evidence and its verified request projection.
    ///
    /// The document is a bounded SDK parity fixture, not a finality attestation.
    /// There is one complete canonical artifact and no old global certificate,
    /// fabricated execution commitment, status projection or successful prefix.
    /// Generation must use the sole native export/replay owner after verification.
    ///
    /// # Errors
    /// Rejects an invalid limit, empty artifact, insufficient output capacity or
    /// allocation failure; no partial document is returned.
    pub fn sdk_fixture_json(&self, maximum: u64) -> Result<Vec<u8>> {
        ensure!(
            maximum > 0 && maximum <= MAX_PROOF_BYTES,
            "SDK fixture output bound is invalid"
        );
        let proof = self.canonical_bytes();
        ensure!(
            !proof.is_empty(),
            "SDK fixture requires a complete canonical artifact"
        );
        let hash = norito::json::to_json(&Hash::new(proof))?;
        let prefix = PREFIX
            .len()
            .checked_add(hash.len())
            .and_then(|n| n.checked_add(PROOF_KEY.len()))
            .and_then(|n| n.checked_add(proof.len().checked_mul(2)?))
            .and_then(|n| n.checked_add(ROWS_KEY.len()))
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| eyre!("SDK fixture size overflow"))?;
        let limit = usize::try_from(maximum)?;
        ensure!(
            prefix.checked_add(2).is_some_and(|n| n <= limit),
            "SDK fixture exceeds its output bound"
        );
        let rows = self.json_projection(u64::try_from(limit - prefix)?)?;
        let size = prefix
            .checked_add(rows.len())
            .ok_or_else(|| eyre!("SDK fixture size overflow"))?;
        ensure!(size <= limit, "SDK fixture exceeds its output bound");
        let mut document = Vec::new();
        document
            .try_reserve_exact(size)
            .map_err(|_| eyre!("SDK fixture output allocation refused"))?;
        document.extend_from_slice(PREFIX);
        document.extend_from_slice(hash.as_bytes());
        document.extend_from_slice(PROOF_KEY);
        for byte in proof {
            document.push(HEX[usize::from(byte >> 4)]);
            document.push(HEX[usize::from(byte & 15)]);
        }
        document.extend_from_slice(ROWS_KEY);
        document.extend_from_slice(&rows);
        document.push(b'}');
        debug_assert_eq!(document.len(), size);
        Ok(document)
    }
}
