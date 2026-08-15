//! Move-only admission for one complete evaluated-key evidence pair.
//!
//! This capability certifies exact ordered, independently verified source and CKS records plus
//! their manifest digests. It also commits the ordered expected CKS compact outputs so the runtime
//! can compare them with streamed ZARK digits before minting a validated handle. It deliberately
//! does not claim the stronger cross-set algebraic equality between accumulated source outputs and
//! the CKS source/compact-output relation.
use super::*;
const VERIFIED_EVIDENCE_SET_CAPABILITY_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.verified-evaluated-key-evidence-set";
const EVIDENCE_DESCRIPTOR_SET_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.evaluated-key-evidence-descriptor-set";
const CKS_COMPACT_OUTPUT_SET_DOMAIN_V1: &[u8] =
    b"iroha.zk-ams.v1.mkhe.evaluated-key-cks-compact-output-set";
const RELEASE_GADGET_DIGITS_V1: usize = 38;
pub(super) const RELEASE_RELIN_PAIR_COUNT_V1: usize = 36;
const RELEASE_RELIN_SOURCE_RECORDS_V1: usize = 21_888;
const RELEASE_GALOIS_SOURCE_RECORDS_V1: usize = 304;
const RELEASE_CKS_RECORDS_V1: usize = 38;
const _: () = assert!(
    ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 * (ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1 + 1) / 2
        == RELEASE_RELIN_PAIR_COUNT_V1
);
const _: () = assert!(
    RELEASE_RELIN_PAIR_COUNT_V1 * RELEASE_GADGET_DIGITS_V1 * 2 * ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        == RELEASE_RELIN_SOURCE_RECORDS_V1
);
const _: () = assert!(
    RELEASE_GADGET_DIGITS_V1 * ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1
        == RELEASE_GALOIS_SOURCE_RECORDS_V1
);
const _: () = assert!(RELEASE_GADGET_DIGITS_V1 == RELEASE_CKS_RECORDS_V1);
mod private {
    pub(super) struct CapabilitySealV1;
}
/// Compact axes recovered only from a privately sealed trusted context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct EvidenceContextAxesV1 {
    pub(super) profile_digest: [u8; 32],
    pub(super) roster_digest: [u8; 32],
    pub(super) key_material_digest: [u8; 32],
    pub(super) epoch: u64,
    pub(super) transcript_digest: [u8; 32],
    pub(super) collective_key_digest: [u8; 32],
}
#[derive(Clone, Copy)]
pub(super) struct SourceEvidenceContextSummaryV1 {
    pub(super) axes: EvidenceContextAxesV1,
    pub(super) context_seal: [u8; 32],
    pub(super) linked_cks_context_seal: [u8; 32],
}
#[derive(Clone, Copy)]
pub(super) struct CksEvidenceContextSummaryV1 {
    pub(super) axes: EvidenceContextAxesV1,
    pub(super) context_seal: [u8; 32],
}
pub(super) struct SourceEvidenceReceiptSummaryV1 {
    pub(super) kind: ZkAmsMkheCollectiveEvidenceRecordKindV1,
    pub(super) ordinal: u8,
    pub(super) record_index: u32,
    pub(super) party_index: u8,
    pub(super) canonical_bytes: u64,
    pub(super) canonical_digest: [u8; 32],
}
pub(super) struct CksEvidenceReceiptSummaryV1 {
    pub(super) ordinal: u8,
    pub(super) digit_index: u8,
    pub(super) canonical_bytes: u64,
    pub(super) canonical_digest: [u8; 32],
    pub(super) compact_constant_digest: [u8; 32],
}
/// Shared generator/verifier recurrence for one ordered evidence set.
pub(super) struct EvidenceSetDigestV1 {
    hash: Keccak256,
    header: ZkAmsMkheCollectiveEvidenceSetHeaderV1,
    records: u32,
}
impl EvidenceSetDigestV1 {
    pub(super) fn new(
        header: ZkAmsMkheCollectiveEvidenceSetHeaderV1,
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if header.collective_key_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut hash = Keccak256::new();
        hash.update(EVIDENCE_DESCRIPTOR_SET_DOMAIN_V1);
        hash.update(&[
            MKHE_VERSION_V1,
            header.kind as u8,
            header.purpose as u8,
            header.ordinal,
        ]);
        hash.update(&header.galois_exponent.to_be_bytes());
        hash.update(&header.collective_key_digest);
        Ok(Self {
            hash,
            header,
            records: 0,
        })
    }
    pub(super) fn absorb_record(
        &mut self,
        record_index: u32,
        record_kind: ZkAmsMkheCollectiveEvidenceRecordKindV1,
        canonical_bytes: u64,
        canonical_digest: [u8; 32],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        let kind_matches = match self.header.kind {
            ZkAmsMkheCollectiveEvidenceSetKindV1::Source => {
                record_kind != ZkAmsMkheCollectiveEvidenceRecordKindV1::CksDigit
            }
            ZkAmsMkheCollectiveEvidenceSetKindV1::Cks => {
                record_kind == ZkAmsMkheCollectiveEvidenceRecordKindV1::CksDigit
            }
        };
        if !kind_matches
            || record_index != self.records
            || canonical_bytes < EVIDENCE_RECORD_DIGEST_BYTES_V1 as u64
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.hash.update(&record_index.to_be_bytes());
        self.hash.update(&[record_kind as u8]);
        self.hash.update(&canonical_bytes.to_be_bytes());
        self.hash.update(&canonical_digest);
        self.records = self
            .records
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        Ok(())
    }
    pub(super) fn finish(mut self, expected_records: u32) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        if expected_records == 0 || self.records != expected_records {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.hash.update(&self.records.to_be_bytes());
        Ok(self.hash.finalize())
    }
}
/// Ordered digest of the CKS compact constants carried by the ZARK digits.
pub(super) struct CksCompactOutputSetDigestV1 {
    hash: Keccak256,
    records: u32,
}
impl CksCompactOutputSetDigestV1 {
    pub(super) fn new(
        purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
        ordinal: u8,
        exponent: u32,
        collective_key_digest: [u8; 32],
    ) -> Result<Self, ZkAmsMkheErrorV1> {
        if collective_key_digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let mut hash = Keccak256::new();
        hash.update(CKS_COMPACT_OUTPUT_SET_DOMAIN_V1);
        hash.update(&[MKHE_VERSION_V1, purpose as u8, ordinal]);
        hash.update(&exponent.to_be_bytes());
        hash.update(&collective_key_digest);
        Ok(Self { hash, records: 0 })
    }
    pub(super) fn absorb(
        &mut self,
        digit_index: u32,
        digest: [u8; 32],
    ) -> Result<(), ZkAmsMkheErrorV1> {
        if digit_index != self.records || digest == [0; 32] {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.hash.update(&digit_index.to_be_bytes());
        self.hash.update(&digest);
        self.records = self
            .records
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        Ok(())
    }
    pub(super) fn finish(mut self, expected_records: u32) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        if expected_records == 0 || self.records != expected_records {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        self.hash.update(&self.records.to_be_bytes());
        Ok(self.hash.finalize())
    }
}
/// Sealed proof that both evidence streams for one exact manifest entry were
/// consumed completely and in canonical order.
///
/// This owner is move-only, non-serializable, and carries no receipt, payload, reader, provider, or
/// dynamically allocated object. It certifies ordered record verification and expected CKS outputs
/// only. CKS-output/ZARK equality is established later by the authenticated provider scan. It does
/// not certify cross-set source-output algebraic equality.
pub struct ZkAmsMkheVerifiedEvaluatedKeyEvidenceSetV1 {
    _private: private::CapabilitySealV1,
    version: u8,
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    ordinal: u8,
    galois_exponent: u32,
    payload_offset: u64,
    payload_bytes: u64,
    payload_blake3: [u8; 32],
    profile_digest: [u8; 32],
    roster_digest: [u8; 32],
    key_material_digest: [u8; 32],
    epoch: u64,
    transcript_digest: [u8; 32],
    collective_key_digest: [u8; 32],
    source_record_count: u32,
    source_proof_set_digest: [u8; 32],
    cks_record_count: u32,
    cks_proof_set_digest: [u8; 32],
    cks_compact_output_set_digest: [u8; 32],
    source_context_seal: [u8; 32],
    cks_context_seal: [u8; 32],
    capability_seal: [u8; 32],
}
impl core::fmt::Debug for ZkAmsMkheVerifiedEvaluatedKeyEvidenceSetV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ZkAmsMkheVerifiedEvaluatedKeyEvidenceSetV1")
            .field("purpose", &self.purpose)
            .field("ordinal", &self.ordinal)
            .field("galois_exponent", &self.galois_exponent)
            .field("source_record_count", &self.source_record_count)
            .field("cks_record_count", &self.cks_record_count)
            .field("profile_digest", &hex::encode(self.profile_digest))
            .field("roster_digest", &hex::encode(self.roster_digest))
            .field("privately_sealed", &(self.capability_seal != [0; 32]))
            .finish_non_exhaustive()
    }
}
#[derive(Clone, Copy)]
pub(super) struct EvidenceSetRuntimeBindingV1 {
    pub(super) entry: ZkAmsMkheCollectiveEvaluatedKeyEntryV1,
    pub(super) profile_digest: [u8; 32],
    pub(super) roster_digest: [u8; 32],
    pub(super) key_material_digest: [u8; 32],
    pub(super) epoch: u64,
    pub(super) transcript_digest: [u8; 32],
    pub(super) collective_key_digest: [u8; 32],
}
pub(super) struct VerifiedEvidenceSetRuntimeAdmissionV1 {
    pub(super) capability_seal: [u8; 32],
    pub(super) cks_compact_output_set_digest: [u8; 32],
}
impl ZkAmsMkheVerifiedEvaluatedKeyEvidenceSetV1 {
    pub(super) fn consume_for_runtime_v1(
        self,
        expected: EvidenceSetRuntimeBindingV1,
    ) -> Result<VerifiedEvidenceSetRuntimeAdmissionV1, ZkAmsMkheErrorV1> {
        let counts = release_evidence_record_counts_v1(expected.entry.purpose())?;
        if self.version != MKHE_VERSION_V1
            || self.purpose != expected.entry.purpose()
            || self.ordinal != expected.entry.ordinal()
            || self.galois_exponent != expected.entry.galois_exponent()
            || self.payload_offset != expected.entry.payload_offset()
            || self.payload_bytes != expected.entry.payload_bytes()
            || self.payload_blake3 != expected.entry.payload_blake3()
            || self.profile_digest != expected.profile_digest
            || self.roster_digest != expected.roster_digest
            || self.key_material_digest != expected.key_material_digest
            || self.epoch != expected.epoch
            || self.transcript_digest != expected.transcript_digest
            || self.collective_key_digest != expected.collective_key_digest
            || self.source_record_count != counts.source
            || self.cks_record_count != counts.cks
            || self.source_proof_set_digest != expected.entry.source_proof_set_digest()
            || self.cks_proof_set_digest != expected.entry.cks_proof_set_digest()
            || self.source_context_seal == [0; 32]
            || self.cks_context_seal == [0; 32]
            || self.cks_compact_output_set_digest == [0; 32]
            || self.capability_seal == [0; 32]
            || self.capability_seal != evidence_set_capability_seal_v1(&self)
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(VerifiedEvidenceSetRuntimeAdmissionV1 {
            capability_seal: self.capability_seal,
            cks_compact_output_set_digest: self.cks_compact_output_set_digest,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct EvidenceRecordCountsV1 {
    pub(super) source: u32,
    pub(super) cks: u32,
}
pub(super) fn checked_evidence_record_counts_v1(
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    gadget_digits: usize,
    roster_size: usize,
) -> Result<EvidenceRecordCountsV1, ZkAmsMkheErrorV1> {
    let cks =
        u32::try_from(gadget_digits).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let source = match purpose {
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization => roster_size
            .checked_mul(
                roster_size
                    .checked_add(1)
                    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?,
            )
            .and_then(|value| value.checked_div(2))
            .and_then(|pairs| pairs.checked_mul(gadget_digits))
            .and_then(|records| records.checked_mul(2))
            .and_then(|records| records.checked_mul(roster_size)),
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois => gadget_digits.checked_mul(roster_size),
    }
    .and_then(|records| u32::try_from(records).ok())
    .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    if source == 0 || cks == 0 {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    Ok(EvidenceRecordCountsV1 { source, cks })
}
fn release_evidence_record_counts_v1(
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
) -> Result<EvidenceRecordCountsV1, ZkAmsMkheErrorV1> {
    let profile = release_profile_v1();
    profile.validate()?;
    let counts = checked_evidence_record_counts_v1(
        purpose,
        profile.gadget_digits,
        ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1,
    )?;
    let exact = match purpose {
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization => {
            RELEASE_RELIN_SOURCE_RECORDS_V1
        }
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois => RELEASE_GALOIS_SOURCE_RECORDS_V1,
    };
    if profile.gadget_digits != RELEASE_GADGET_DIGITS_V1
        || profile.moduli.len() != RELEASE_GADGET_DIGITS_V1
        || usize::try_from(counts.source).ok() != Some(exact)
        || usize::try_from(counts.cks).ok() != Some(RELEASE_CKS_RECORDS_V1)
    {
        return Err(ZkAmsMkheErrorV1::InvalidProfile);
    }
    Ok(counts)
}
fn validate_entry_coordinate_v1(
    entry: ZkAmsMkheCollectiveEvaluatedKeyEntryV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    match entry.purpose() {
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization => {
            if entry.ordinal() != 0 || entry.galois_exponent() != 0 {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
        }
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois => {
            let ordinal = usize::from(entry.ordinal());
            let schedule = zk_ams_t256_galois_key_schedule_v1()?;
            validate_zk_ams_t256_galois_key_schedule_v1(&schedule)?;
            if ordinal == 0
                || schedule
                    .entries
                    .get(ordinal - 1)
                    .is_none_or(|value| value.exponent != entry.galois_exponent())
            {
                return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
            }
        }
    }
    Ok(())
}
pub(super) fn expected_source_descriptor_v1(
    purpose: ZkAmsMkheCollectiveEvaluatedKeyPurposeV1,
    record_index: u32,
) -> Result<(ZkAmsMkheCollectiveEvidenceRecordKindV1, u8), ZkAmsMkheErrorV1> {
    let roster = ZK_AMS_MKHE_RELEASE_ROSTER_SIZE_V1;
    let index =
        usize::try_from(record_index).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
    let (kind, party) = match purpose {
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Relinearization => {
            let pair_count = roster
                .checked_mul(roster + 1)
                .and_then(|value| value.checked_div(2))
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            let per_digit = pair_count
                .checked_mul(2)
                .and_then(|value| value.checked_mul(roster))
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
            let pair_slot = (index % per_digit) % (2 * roster);
            if pair_slot < roster {
                (
                    ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundOne,
                    pair_slot,
                )
            } else {
                (
                    ZkAmsMkheCollectiveEvidenceRecordKindV1::RkgRoundTwo,
                    pair_slot - roster,
                )
            }
        }
        ZkAmsMkheCollectiveEvaluatedKeyPurposeV1::Galois => (
            ZkAmsMkheCollectiveEvidenceRecordKindV1::GaloisSource,
            index % roster,
        ),
    };
    Ok((
        kind,
        u8::try_from(party).map_err(|_| ZkAmsMkheErrorV1::InvalidPartySet)?,
    ))
}
/// Consume and aggregate both exact receipt streams for one manifest entry.
///
/// Each iterator is lazy and is advanced exactly once per expected record plus one final
/// extra-record probe. Receipts are consumed immediately; none is retained or collected. Any
/// failure is terminal because all preceding move-only receipts have already been consumed.
pub fn verify_zk_ams_mkhe_evaluated_key_evidence_set_v1<SI, CI>(
    source_context: &ZkAmsMkheTrustedSourceContextV1,
    cks_context: &ZkAmsMkheTrustedCksContextV1,
    entry: &ZkAmsMkheCollectiveEvaluatedKeyEntryV1,
    source_receipts: SI,
    cks_receipts: CI,
) -> Result<ZkAmsMkheVerifiedEvaluatedKeyEvidenceSetV1, ZkAmsMkheErrorV1>
where
    SI: IntoIterator<
        Item = Result<ZkAmsMkheValidatedCollectiveSourceEvidenceReceiptV1, ZkAmsMkheErrorV1>,
    >,
    CI: IntoIterator<Item = Result<ZkAmsMkheOwnedCollectiveCksDigitEvidenceV1, ZkAmsMkheErrorV1>>,
{
    let source_summary = source_stream::verified_evidence_context_summary_v1(source_context)?;
    let cks_summary = cks_stream::verified_evidence_context_summary_v1(cks_context)?;
    if source_summary.axes != cks_summary.axes
        || source_summary.linked_cks_context_seal != cks_summary.context_seal
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    validate_entry_coordinate_v1(*entry)?;
    let counts = release_evidence_record_counts_v1(entry.purpose())?;
    let source_header = ZkAmsMkheCollectiveEvidenceSetHeaderV1 {
        kind: ZkAmsMkheCollectiveEvidenceSetKindV1::Source,
        purpose: entry.purpose(),
        ordinal: entry.ordinal(),
        galois_exponent: entry.galois_exponent(),
        collective_key_digest: source_summary.axes.collective_key_digest,
    };
    let cks_header = ZkAmsMkheCollectiveEvidenceSetHeaderV1 {
        kind: ZkAmsMkheCollectiveEvidenceSetKindV1::Cks,
        ..source_header
    };
    let mut source_digest = EvidenceSetDigestV1::new(source_header)?;
    let mut source_receipts = source_receipts.into_iter();
    for record_index in 0..counts.source {
        let receipt = source_receipts
            .next()
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)??;
        let summary = source_stream::consume_verified_evidence_receipt_v1(source_context, receipt)?;
        let (kind, party_index) = expected_source_descriptor_v1(entry.purpose(), record_index)?;
        if summary.ordinal != entry.ordinal()
            || summary.record_index != record_index
            || summary.kind != kind
            || summary.party_index != party_index
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        source_digest.absorb_record(
            record_index,
            summary.kind,
            summary.canonical_bytes,
            summary.canonical_digest,
        )?;
    }
    match source_receipts.next() {
        None => {}
        Some(Ok(_)) => return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
        Some(Err(error)) => return Err(error),
    }
    let source_proof_set_digest = source_digest.finish(counts.source)?;
    let mut cks_digest = EvidenceSetDigestV1::new(cks_header)?;
    let mut compact_output_digest = CksCompactOutputSetDigestV1::new(
        entry.purpose(),
        entry.ordinal(),
        entry.galois_exponent(),
        source_summary.axes.collective_key_digest,
    )?;
    let mut cks_receipts = cks_receipts.into_iter();
    for digit_index in 0..counts.cks {
        let receipt = cks_receipts
            .next()
            .ok_or(ZkAmsMkheErrorV1::InvalidKeyMaterial)??;
        let summary = cks_stream::consume_verified_evidence_receipt_v1(cks_context, receipt)?;
        if summary.ordinal != entry.ordinal() || u32::from(summary.digit_index) != digit_index {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        cks_digest.absorb_record(
            digit_index,
            ZkAmsMkheCollectiveEvidenceRecordKindV1::CksDigit,
            summary.canonical_bytes,
            summary.canonical_digest,
        )?;
        compact_output_digest.absorb(digit_index, summary.compact_constant_digest)?;
    }
    match cks_receipts.next() {
        None => {}
        Some(Ok(_)) => return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial),
        Some(Err(error)) => return Err(error),
    }
    let cks_proof_set_digest = cks_digest.finish(counts.cks)?;
    let cks_compact_output_set_digest = compact_output_digest.finish(counts.cks)?;
    if source_proof_set_digest != entry.source_proof_set_digest()
        || cks_proof_set_digest != entry.cks_proof_set_digest()
    {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    let axes = source_summary.axes;
    let mut capability = ZkAmsMkheVerifiedEvaluatedKeyEvidenceSetV1 {
        _private: private::CapabilitySealV1,
        version: MKHE_VERSION_V1,
        purpose: entry.purpose(),
        ordinal: entry.ordinal(),
        galois_exponent: entry.galois_exponent(),
        payload_offset: entry.payload_offset(),
        payload_bytes: entry.payload_bytes(),
        payload_blake3: entry.payload_blake3(),
        profile_digest: axes.profile_digest,
        roster_digest: axes.roster_digest,
        key_material_digest: axes.key_material_digest,
        epoch: axes.epoch,
        transcript_digest: axes.transcript_digest,
        collective_key_digest: axes.collective_key_digest,
        source_record_count: counts.source,
        source_proof_set_digest,
        cks_record_count: counts.cks,
        cks_proof_set_digest,
        cks_compact_output_set_digest,
        source_context_seal: source_summary.context_seal,
        cks_context_seal: cks_summary.context_seal,
        capability_seal: [0; 32],
    };
    capability.capability_seal = evidence_set_capability_seal_v1(&capability);
    if capability.capability_seal == [0; 32] {
        return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
    }
    Ok(capability)
}
fn evidence_set_capability_seal_v1(
    capability: &ZkAmsMkheVerifiedEvaluatedKeyEvidenceSetV1,
) -> [u8; 32] {
    let mut hash = Keccak256::new();
    hash.update(VERIFIED_EVIDENCE_SET_CAPABILITY_DOMAIN_V1);
    hash.update(&[
        capability.version,
        capability.purpose as u8,
        capability.ordinal,
    ]);
    hash.update(&capability.galois_exponent.to_be_bytes());
    hash.update(&capability.payload_offset.to_be_bytes());
    hash.update(&capability.payload_bytes.to_be_bytes());
    hash.update(&capability.payload_blake3);
    hash.update(&capability.profile_digest);
    hash.update(&capability.roster_digest);
    hash.update(&capability.key_material_digest);
    hash.update(&capability.epoch.to_be_bytes());
    hash.update(&capability.transcript_digest);
    hash.update(&capability.collective_key_digest);
    hash.update(&capability.source_record_count.to_be_bytes());
    hash.update(&capability.source_proof_set_digest);
    hash.update(&capability.cks_record_count.to_be_bytes());
    hash.update(&capability.cks_proof_set_digest);
    hash.update(&capability.cks_compact_output_set_digest);
    hash.update(&capability.source_context_seal);
    hash.update(&capability.cks_context_seal);
    hash.finalize()
}
#[cfg(test)]
pub(super) fn test_tamper_capability_seal_v1(
    capability: &mut ZkAmsMkheVerifiedEvaluatedKeyEvidenceSetV1,
) {
    capability.capability_seal[0] ^= 1;
}
