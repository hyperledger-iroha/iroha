//! Sole-encoder bounded grammar for complete original ordinary signed receiver requests.
//!
//! These inert codec templates authenticate no platform or receiver. A consuming circuit must
//! prove the actual DER/CBOR equation and reconstruct every semantic field and CRC in the one
//! selected complete stream. Specimen-specific offsets never select a circuit key.
use super::{
    KagemushaAppOperationApprovalEvidenceV1, KagemushaOrdinaryCashOriginalLayoutV1,
    KagemushaOrdinaryPaymentRequestV1, ORIGINAL_DOMAIN,
};

/// One encoder-owned phase/length variant outside the actual raw platform evidence units.
#[derive(Debug, Clone)]
pub struct KagemushaOrdinaryPaymentRequestStreamVariantV1 {
    /// Platform selector, false Android DER and true App Attest original CBOR.
    pub apple: bool,
    /// Actual original evidence length, within Android8..72 or Apple1..311.
    pub evidence_length: usize,
    /// Exact bytes preceding the first raw evidence byte, with semantics left unassigned.
    pub prefix: Vec<Option<u8>>,
    /// Exact bytes following the final raw evidence byte, with semantics left unassigned.
    pub suffix: Vec<Option<u8>>,
    /// Complete model-owned semantic position map; raw evidence positions are included.
    pub layout: KagemushaOrdinaryCashOriginalLayoutV1,
    /// Exact header payload-length LE64 positions relative to the full assembled stream.
    pub header_payload_length_bytes: [usize; 8],
    /// Exact header CRC64-XZ positions; must be computed over the complete archive payload.
    pub header_crc_bytes: [usize; 8],
    /// Canonical archive payload, excluding its header and encoder-owned alignment padding.
    pub archive_payload: core::ops::Range<usize>,
    /// Exact LE64 full-canonical-original length after its domain, before the frame.
    pub original_length_bytes: [usize; 8],
}
/// One fixed topology for both original platform variants and every evidence width.
#[derive(Debug, Clone)]
pub struct KagemushaOrdinaryPaymentRequestStreamGrammarV1 {
    /// A raw byte None followed by the sole encoder's framing up to the next raw byte.
    pub repeated_evidence_byte_unit: Vec<Option<u8>>,
    /// Maximum selected-prefix capacity over all variants.
    pub maximum_prefix_bytes: usize,
    /// Maximum selected-suffix capacity over all variants.
    pub maximum_suffix_bytes: usize,
    /// Fixed full-original stream capacity for every platform/width.
    pub maximum_stream_bytes: usize,
    /// Android8..72 followed by Apple1..311, exactly once for each valid codec width.
    pub variants: Vec<KagemushaOrdinaryPaymentRequestStreamVariantV1>,
}
impl KagemushaOrdinaryPaymentRequestV1 {
    /// Derive all complete-original byte framing from the maintained canonical Norito encoder.
    /// This metadata is data only. Actual original evidence admission remains independent.
    /// # Errors
    /// Rejects malformed body data or a codec topology that cannot be represented exactly.
    pub fn original_canonical_stream_grammar(
        &self,
    ) -> Result<KagemushaOrdinaryPaymentRequestStreamGrammarV1, String> {
        self.body.validate_shape()?;
        let mut variants = Vec::with_capacity(376);
        let mut common_unit: Option<Vec<Option<u8>>> = None;
        for apple in [false, true] {
            let minimum = if apple { 1 } else { 8 };
            let maximum = if apple { 311 } else { 72 };
            for length in minimum..=maximum {
                let mut specimen = self.clone();
                // Width-only inert originals: neither their DER nor CBOR is admitted here.
                specimen.evidence = if apple {
                    KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
                        raw_assertion: vec![0x5a; length],
                    }
                } else {
                    KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                        signature_der: vec![0x5a; length],
                    }
                };
                let layout = specimen.original_preimage_layout_for_specimen()?;
                let evidence_name = if apple {
                    "evidence.raw_assertion"
                } else {
                    "evidence.signature_der"
                };
                let positions = &layout
                    .fields
                    .iter()
                    .find(|f| f.name == evidence_name)
                    .ok_or("request evidence layout absent")?
                    .positions;
                if positions.len() != length {
                    return Err("request evidence codec width differs".into());
                }
                let first = positions[0];
                // Width1 has no second raw byte; derive its repeated unit from the same encoder
                // at width2, then verify exact assembly of this single-byte variant below.
                let stride = if length == 1 {
                    let mut two = specimen.clone();
                    two.evidence = KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
                        raw_assertion: vec![0x5a; 2],
                    };
                    let l = two.original_preimage_layout_for_specimen()?;
                    let p = &l
                        .fields
                        .iter()
                        .find(|f| f.name == evidence_name)
                        .ok_or("request width2 evidence absent")?
                        .positions;
                    p[1].checked_sub(p[0])
                        .ok_or("request width2 ordering differs")?
                } else {
                    positions[1]
                        .checked_sub(first)
                        .ok_or("request evidence ordering differs")?
                };
                if stride == 0
                    || positions
                        .iter()
                        .enumerate()
                        .any(|(i, p)| *p != first + i * stride)
                {
                    return Err("request evidence units are not uniform".into());
                }
                let unit = if length == 1 {
                    common_unit.clone().ok_or("request Android unit absent")?
                } else {
                    layout.bytes[first..first + stride].to_vec()
                };
                if unit.first() != Some(&None)
                    || unit.iter().skip(1).any(Option::is_none)
                    || common_unit.as_ref().is_some_and(|u| *u != unit)
                {
                    return Err("request raw byte framing changes with platform/width".into());
                }
                let last = *positions.last().ok_or("request raw evidence absent")?;
                let prefix = layout.bytes[..first].to_vec();
                let suffix = layout.bytes[last + 1..].to_vec();
                let mut assembled = prefix.clone();
                for _ in 0..length - 1 {
                    assembled.extend_from_slice(&unit)
                }
                assembled.push(None);
                assembled.extend_from_slice(&suffix);
                if assembled != layout.bytes {
                    return Err("request complete codec stream assembly differs".into());
                }
                common_unit = Some(unit);
                let frame = norito::encode_canonical(&specimen).map_err(|e| e.to_string())?;
                let header =
                    norito::core::Header::read(frame.as_slice()).map_err(|e| e.to_string())?;
                let start = layout.original.start;
                // The sole encoder's header length and CRC cover the archive payload only.
                // OriginalBuilder already verifies the intervening canonical zero alignment.
                let archive_start = start
                    .checked_add(layout.payload_offset)
                    .filter(|offset| *offset <= layout.original.end)
                    .ok_or("request selected frame header/payload differs")?;
                let archive_payload = archive_start..layout.original.end;
                if frame[23..31] != header.length.to_le_bytes()
                    || frame[31..39] != header.checksum.to_le_bytes()
                    || start + norito::core::Header::SIZE > prefix.len()
                    || archive_payload.start > prefix.len()
                    || usize::try_from(header.length).map_err(|e| e.to_string())?
                        != archive_payload.len()
                {
                    return Err("request selected frame header/payload differs".into());
                }
                variants.push(KagemushaOrdinaryPaymentRequestStreamVariantV1 {
                    apple,
                    evidence_length: length,
                    prefix,
                    suffix,
                    header_payload_length_bytes: core::array::from_fn(|i| start + 23 + i),
                    header_crc_bytes: core::array::from_fn(|i| start + 31 + i),
                    archive_payload,
                    original_length_bytes: core::array::from_fn(|i| ORIGINAL_DOMAIN.len() + i),
                    layout,
                });
            }
        }
        Ok(KagemushaOrdinaryPaymentRequestStreamGrammarV1 {
            repeated_evidence_byte_unit: common_unit.ok_or("request stream unit absent")?,
            maximum_prefix_bytes: variants
                .iter()
                .map(|v| v.prefix.len())
                .max()
                .ok_or("request prefix absent")?,
            maximum_suffix_bytes: variants
                .iter()
                .map(|v| v.suffix.len())
                .max()
                .ok_or("request suffix absent")?,
            maximum_stream_bytes: variants
                .iter()
                .map(|v| v.layout.bytes.len())
                .max()
                .ok_or("request stream absent")?,
            variants,
        })
    }
}
#[cfg(test)]
#[path = "ordinary_payment_request_stream_tests.rs"]
mod tests;
