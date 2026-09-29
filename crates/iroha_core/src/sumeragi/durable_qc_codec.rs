//! Qc's exact nested payload, decoding finite metadata separately from its funded witness.

use std::ops::Range;

use iroha_sumeragi::{
    message::{
        AttestationSignature, MAX_ATTESTATION_SIGNATURE_BYTES, MAX_BITMAP_BYTES,
        MAX_RESULT_WITNESS_BYTES, Qc, ResultWitness, VoteKind,
    },
    types::{AggregateSignature, Bitmap, EpochId, Hash32, MAX_COMMITTEE_SIZE},
};
use norito::core as ncore;

use crate::sumeragi::durable_record_codec::{BytesRef, FieldRef, byte_range, field_range};

// Scalar fields/lengths fit 1024 bytes, bitmap fits MAX_BITMAP_BYTES plus framing, and each
// inline attestation uses its advertised bound plus 32 bytes of canonical length framing.
// This cap is checked before generic metadata decoding and excludes the separately funded witness.
pub(super) const MAX_QC_METADATA_BYTES: usize =
    1024 + MAX_BITMAP_BYTES + MAX_COMMITTEE_SIZE * (MAX_ATTESTATION_SIGNATURE_BYTES + 32);

#[derive(norito::Encode)]
pub(super) struct QcRef<'a> {
    kind: FieldRef<'a, VoteKind>,
    instance: FieldRef<'a, Hash32>,
    epoch: FieldRef<'a, EpochId>,
    height: FieldRef<'a, u64>,
    view: FieldRef<'a, u64>,
    block_hash: FieldRef<'a, Hash32>,
    result: FieldRef<'a, Hash32>,
    attest: FieldRef<'a, bool>,
    signers: FieldRef<'a, Bitmap>,
    agg_sig: FieldRef<'a, AggregateSignature>,
    attestations: FieldRef<'a, Vec<AttestationSignature>>,
    attestation_witness: Option<BytesRef<'a>>,
}
impl norito::NoritoSchema for QcRef<'_> {
    fn nominal_name() -> String {
        <Qc as norito::NoritoSchema>::nominal_name()
    }
    fn static_frame_name() -> Option<&'static str> {
        Some("iroha_sumeragi::Qc")
    }
}

impl<'a> From<&'a Qc> for QcRef<'a> {
    fn from(qc: &'a Qc) -> Self {
        Self {
            kind: FieldRef(&qc.kind),
            instance: FieldRef(&qc.instance),
            epoch: FieldRef(&qc.epoch),
            height: FieldRef(&qc.height),
            view: FieldRef(&qc.view),
            block_hash: FieldRef(&qc.block_hash),
            result: FieldRef(&qc.result),
            attest: FieldRef(&qc.attest),
            signers: FieldRef(&qc.signers),
            agg_sig: FieldRef(&qc.agg_sig),
            attestations: FieldRef(&qc.attestations),
            attestation_witness: qc
                .attestation_witness
                .as_ref()
                .map(|w| BytesRef(w.as_slice())),
        }
    }
}

// This is deliberately only Qc's first eleven fields, with the same payload codec/order.
// It has no root schema, no witness and no generic Decode path that can allocate bulk bytes.
#[derive(norito::Encode, norito::Decode)]
pub(super) struct QcMetadata {
    kind: VoteKind,
    instance: Hash32,
    epoch: EpochId,
    height: u64,
    view: u64,
    block_hash: Hash32,
    result: Hash32,
    attest: bool,
    signers: Bitmap,
    agg_sig: AggregateSignature,
    attestations: Vec<AttestationSignature>,
}
impl QcMetadata {
    pub(super) fn borrowed<'a>(&'a self, witness: Option<&'a [u8]>) -> QcRef<'a> {
        QcRef {
            kind: FieldRef(&self.kind),
            instance: FieldRef(&self.instance),
            epoch: FieldRef(&self.epoch),
            height: FieldRef(&self.height),
            view: FieldRef(&self.view),
            block_hash: FieldRef(&self.block_hash),
            result: FieldRef(&self.result),
            attest: FieldRef(&self.attest),
            signers: FieldRef(&self.signers),
            agg_sig: FieldRef(&self.agg_sig),
            attestations: FieldRef(&self.attestations),
            attestation_witness: witness.map(BytesRef),
        }
    }
    pub(super) fn finish(self, witness: Option<ResultWitness>) -> Qc {
        Qc {
            kind: self.kind,
            instance: self.instance,
            epoch: self.epoch,
            height: self.height,
            view: self.view,
            block_hash: self.block_hash,
            result: self.result,
            attest: self.attest,
            signers: self.signers,
            agg_sig: self.agg_sig,
            attestations: self.attestations,
            attestation_witness: witness,
        }
    }
}

/// Parse only the declared canonical Qc field layout; the caller holds the advertised context.
/// ResultWitness is returned as a range into the original record, never decoded into a Vec.
pub(super) fn parse(bytes: &[u8]) -> Result<(QcMetadata, Option<Range<usize>>), norito::Error> {
    let mut offset = 0;
    for index in 0..11 {
        let field = field_range(bytes, &mut offset)?;
        let cap = if index == 10 {
            MAX_QC_METADATA_BYTES
        } else {
            1024
        };
        if field.len() > cap || offset > MAX_QC_METADATA_BYTES {
            return Err(norito::Error::FieldLengthExceeded {
                length: offset as u64,
                limit: MAX_QC_METADATA_BYTES as u64,
            });
        }
    }
    let metadata_end = offset;
    let metadata: QcMetadata = ncore::with_decode_limits(
        norito::DecodeLimits::new(
            MAX_COMMITTEE_SIZE,
            MAX_QC_METADATA_BYTES,
            MAX_QC_METADATA_BYTES * 2,
            MAX_QC_METADATA_BYTES * 8,
            ncore::MAX_VALUE_NESTING_DEPTH,
        ),
        || ncore::decode_field_canonical(&bytes[..metadata_end]).map(|(value, _)| value),
    )?;
    if metadata.signers.as_bytes().len() > MAX_BITMAP_BYTES
        || metadata.attestations.len() > MAX_COMMITTEE_SIZE
    {
        return Err(norito::Error::NonCanonicalEncoding);
    }
    let witness_field = field_range(bytes, &mut offset)?;
    if offset != bytes.len() {
        return Err(norito::Error::LengthMismatch);
    }
    let value = &bytes[witness_field.clone()];
    let witness = match value.first() {
        Some(0) if value.len() == 1 => None,
        Some(1) => {
            let mut at = 1;
            let field = field_range(value, &mut at)?;
            if at != value.len() {
                return Err(norito::Error::LengthMismatch);
            }
            let range = byte_range(value, field, 1, MAX_RESULT_WITNESS_BYTES)?;
            Some((witness_field.start + range.start)..(witness_field.start + range.end))
        }
        _ => return Err(norito::Error::NonCanonicalEncoding),
    };
    Ok((metadata, witness))
}

/// Validate the standalone canonical Qc frame, leaving its bulk witness in the original source.
/// Returned witness ranges are relative to `raw`; no destination or admission occurs here.
pub(super) fn parse_frame(raw: &[u8]) -> Result<(QcMetadata, Option<Range<usize>>), norito::Error> {
    let view = ncore::from_bytes_view(raw)?;
    if view.schema() != norito::schema::identity::frame_hash::<Qc>() {
        return Err(norito::Error::SchemaMismatch);
    }
    let bytes = view.as_bytes();
    let base = raw
        .len()
        .checked_sub(bytes.len())
        .ok_or(norito::Error::LengthMismatch)?;
    let (metadata, witness) = {
        let _context =
            ncore::PayloadCtxGuard::enter_with_schema_and_flags(bytes, view.schema(), view.flags());
        parse(bytes)?
    };
    norito::verify_exact_canonical_frame(
        &metadata.borrowed(witness.as_ref().map(|r| &bytes[r.clone()])),
        raw,
    )?;
    Ok((metadata, witness.map(|r| (base + r.start)..(base + r.end))))
}
