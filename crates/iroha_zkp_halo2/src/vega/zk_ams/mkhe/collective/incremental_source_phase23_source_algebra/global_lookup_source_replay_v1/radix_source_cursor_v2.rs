//! One-shot authenticated canonical-source cursor for radix materialization.

use super::super::super::{
    PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1, PHASE23_RECORD_COUNT_V1,
    ZkAmsPhase23RnsLinkSecretChunkV1,
    radix_range_v2::{
        Phase23RadixSourceCursorAxesV2, Phase23RadixWitnessMaterializedV2,
        Phase23RadixWitnessScratchSinkV2, materialize_phase23_radix_witness_v2,
    },
};
use super::*;

const RADIX_SOURCE_CURSOR_VERSION_V2: u8 = 2;
const RADIX_SOURCE_READ_BLOCKS_V2: usize =
    PHASE23_RECORD_COUNT_V1 * PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1;
const RADIX_SOURCE_READ_SCHEDULE_DOMAIN_V2: &[u8] =
    b"iroha.zk-ams.v2.phase23.radix-witness.authenticated-source-read-schedule\0";

struct RadixSourceReadScheduleV2 {
    next_record: u16,
    next_block: u16,
    hash: Keccak256,
}

impl RadixSourceReadScheduleV2 {
    fn begin_v2(replay_record_digest: [u8; 32], source_receipt_digest: [u8; 32]) -> Self {
        let mut hash = Keccak256::new();
        hash.update(RADIX_SOURCE_READ_SCHEDULE_DOMAIN_V2);
        hash.update(&[RADIX_SOURCE_CURSOR_VERSION_V2]);
        hash.update(&replay_record_digest);
        hash.update(&source_receipt_digest);
        hash.update(&(RADIX_SOURCE_READ_BLOCKS_V2 as u32).to_be_bytes());
        Self {
            next_record: 0,
            next_block: 0,
            hash,
        }
    }

    fn require_next_v2(&self, record: u16, block: u16) -> Result<(), ZkAmsMkheErrorV1> {
        if record != self.next_record
            || block != self.next_block
            || usize::from(record) >= PHASE23_RECORD_COUNT_V1
            || usize::from(block) >= PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok(())
    }

    fn absorb_next_v2(&mut self, record: u16, block: u16) -> Result<(), ZkAmsMkheErrorV1> {
        self.require_next_v2(record, block)?;
        self.hash.update(&record.to_be_bytes());
        self.hash.update(&block.to_be_bytes());
        self.next_block = self
            .next_block
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        if usize::from(self.next_block) == PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1 {
            self.next_block = 0;
            self.next_record = self
                .next_record
                .checked_add(1)
                .ok_or(ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        }
        Ok(())
    }

    fn finish_v2(self) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
        if usize::from(self.next_record) != PHASE23_RECORD_COUNT_V1 || self.next_block != 0 {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        require_nonzero_v1(self.hash.finalize())
    }
}

/// Narrow move-only cursor. It owns replay evidence, exposes no callback, and
/// accepts only the next canonical `(record, block)` pair.
pub(in crate::vega::zk_ams::mkhe::collective::incremental_source_phase23) struct Phase23GlobalLookupRadixSourceCursorV2<
    K,
    P,
> {
    evidence: Option<Phase23GlobalLookupSourceReplayEvidenceV1<K, P>>,
    replay_record_digest: [u8; 32],
    source_receipt_digest: [u8; 32],
    schedule: Option<RadixSourceReadScheduleV2>,
}

impl<K, P> Phase23GlobalLookupRadixSourceCursorV2<K, P> {
    fn begin_v2(
        evidence: Phase23GlobalLookupSourceReplayEvidenceV1<K, P>,
    ) -> Result<(Self, Phase23RadixSourceCursorAxesV2), ZkAmsMkheErrorV1> {
        validate_replay_evidence_v1(&evidence)?;
        let replay_record_digest = evidence.record.record_digest;
        let source_receipt_digest = evidence.record.source_receipt_digest;
        Ok((
            Self {
                evidence: Some(evidence),
                replay_record_digest,
                source_receipt_digest,
                schedule: Some(RadixSourceReadScheduleV2::begin_v2(
                    replay_record_digest,
                    source_receipt_digest,
                )),
            },
            Phase23RadixSourceCursorAxesV2 {
                replay_record_digest,
                source_receipt_digest,
            },
        ))
    }

    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source_phase23) fn read_next_canonical_block_v2(
        &mut self,
        record: usize,
        block: usize,
    ) -> Result<ZkAmsPhase23RnsLinkSecretChunkV1, ZkAmsMkheErrorV1> {
        // Remove the evidence before order validation or authenticated I/O. Any
        // failure or unwind consumes it and leaves this cursor poisoned.
        let mut evidence = self
            .evidence
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let record =
            u16::try_from(record).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let block = u16::try_from(block).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
        let schedule = self
            .schedule
            .as_mut()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        schedule.require_next_v2(record, block)?;
        let owner = evidence
            .prerequisite
            .live
            .as_mut()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let mut source = owner
            .owner
            .source
            .read_canonical_plaintext_block_v1(record, block)?;
        validate_canonical_source_block_v1(source.as_mut_bytes_v1())?;
        schedule.absorb_next_v2(record, block)?;
        self.evidence = Some(evidence);
        Ok(source)
    }

    /// Sole restricted Evidence return. The radix materializer calls this once
    /// after exactly `43 * 512` successful authenticated reads.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source_phase23) fn complete_for_radix_materializer_v2(
        mut self,
    ) -> Result<(Phase23GlobalLookupSourceReplayEvidenceV1<K, P>, [u8; 32]), ZkAmsMkheErrorV1> {
        let evidence = self
            .evidence
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let schedule = self
            .schedule
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        validate_replay_evidence_v1(&evidence)?;
        let owner = evidence
            .prerequisite
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        if evidence.record.record_digest != self.replay_record_digest
            || owner.owner.source.receipt_v1().receipt_digest_v1() != self.source_receipt_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        Ok((evidence, schedule.finish_v2()?))
    }
}

impl<K, P> Phase23GlobalLookupSourceReplayEvidenceV1<K, P> {
    /// Consume authenticated replay into the sole compact radix materializer.
    pub(in crate::vega::zk_ams::mkhe::collective::incremental_source_phase23) fn into_radix_witness_materialized_v2(
        self,
        sink: Phase23RadixWitnessScratchSinkV2,
    ) -> Result<Phase23RadixWitnessMaterializedV2<K, P>, ZkAmsMkheErrorV1> {
        let (cursor, axes) = Phase23GlobalLookupRadixSourceCursorV2::begin_v2(self)?;
        materialize_phase23_radix_witness_v2(cursor, axes, sink)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_schedule_rejects_skip_duplicate_reverse_early_finish_and_trailing_read() {
        let mut schedule = RadixSourceReadScheduleV2::begin_v2([0x11; 32], [0x22; 32]);
        assert!(schedule.require_next_v2(0, 1).is_err());
        assert!(schedule.require_next_v2(1, 0).is_err());
        assert!(
            RadixSourceReadScheduleV2::begin_v2([0x11; 32], [0x22; 32])
                .finish_v2()
                .is_err()
        );
        for record in 0..PHASE23_RECORD_COUNT_V1 {
            for block in 0..PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1 {
                schedule
                    .absorb_next_v2(record as u16, block as u16)
                    .expect("canonical read coordinate");
            }
        }
        assert!(schedule.require_next_v2(42, 511).is_err());
        assert!(schedule.require_next_v2(43, 0).is_err());
        assert_ne!(schedule.finish_v2().expect("complete schedule"), [0; 32]);
    }
}
