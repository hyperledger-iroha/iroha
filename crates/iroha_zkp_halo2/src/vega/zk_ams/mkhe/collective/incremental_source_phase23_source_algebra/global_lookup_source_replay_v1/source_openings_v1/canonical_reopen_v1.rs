//! Sealed second-pass canonical source reopener for source/packing columns.

use super::*;

struct CanonicalReopenRecordV1 {
    source_receipt_digest: [u8; 32],
    source_opening_record_digest: [u8; 32],
    authenticated_read_schedule_root: [u8; 32],
    block_count: u32,
    scalar_count: u64,
    plaintext_bytes: u64,
    authenticated_read_bytes: u64,
    source_same_opening_proved: bool,
    packing_same_opening_proved: bool,
    global_lookup_proof_verified: bool,
    zero_knowledge_accepted: bool,
    authority_accepted: bool,
    operational_receipt_accepted: bool,
    rss_gate_accepted: bool,
    release_ready: bool,
    release_complete: bool,
    record_digest: [u8; 32],
}

/// Opaque next state retaining the complete replay owner and bounded columns.
#[must_use = "dropping this owner closes the source openings and canonical replay"]
pub(in crate::vega::zk_ams::mkhe) struct Phase23GlobalLookupSourceReopenedV1<K, P> {
    replay: Phase23GlobalLookupSourceReplayV1<K, P>,
    weighted_columns: WeightedOpeningColumnsV1,
    record: CanonicalReopenRecordV1,
}

impl<K, P> Phase23GlobalLookupSourceReplayV1<K, P> {
    /// Consume this owner into the later authenticated canonical pass. The
    /// production seal is currently uninhabited; no generic callback escapes.
    pub(in crate::vega::zk_ams::mkhe) fn into_canonical_opening_replay_v1(
        self,
        seal: GlobalLookupCanonicalReopenSealV1,
    ) -> Result<Phase23GlobalLookupSourceReopenedV1<K, P>, ZkAmsMkheErrorV1> {
        CanonicalReopenIngressV1 {
            replay: Some(self),
            seal: Some(seal),
        }
        .run_v1()
    }
}

struct CanonicalReopenIngressV1<K, P> {
    replay: Option<Phase23GlobalLookupSourceReplayV1<K, P>>,
    seal: Option<GlobalLookupCanonicalReopenSealV1>,
}

impl<K, P> CanonicalReopenIngressV1<K, P> {
    fn run_v1(mut self) -> Result<Phase23GlobalLookupSourceReopenedV1<K, P>, ZkAmsMkheErrorV1> {
        // Take both authority owners before validation or authenticated I/O.
        let mut replay = self
            .replay
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        let seal = self
            .seal
            .take()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
        validate_replay_record_v1(&replay.record)?;
        replay.openings.validate_v1()?;
        let source_receipt_digest = replay
            .prerequisite
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .owner
            .source
            .receipt_v1()
            .receipt_digest_v1();
        if source_receipt_digest != replay.record.source_receipt_digest
            || source_receipt_digest != replay.openings.record.source_receipt_digest
            || replay.record.prerequisite_record_digest
                != replay.openings.record.prerequisite_record_digest
            || global_lookup_topology_digest_v1() != replay.openings.record.topology_digest
            || exact_source_opening_mapping_digest_v1()? != replay.openings.record.mapping_digest
            || zk_ams_t256_bulletproof_generator_basis_digest_v1()
                != replay.openings.record.basis_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let mut sink = WeightedOpeningColumnsSinkV1::from_seal_v1(seal)?;
        let mut schedule = Keccak256::new();
        schedule.update(CANONICAL_REOPEN_SCHEDULE_DOMAIN_V1);
        schedule.update(&[SOURCE_OPENING_VERSION_V1]);
        schedule.update(&source_receipt_digest);
        schedule.update(&replay.openings.record.record_digest);
        schedule.update(&(CANONICAL_REOPEN_BLOCK_COUNT_V1 as u32).to_be_bytes());
        for record in 0..PHASE23_RECORD_COUNT_V1 {
            for block in 0..PHASE23_CANONICAL_BLOCKS_PER_RECORD_V1 {
                let record =
                    u16::try_from(record).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
                let block =
                    u16::try_from(block).map_err(|_| ZkAmsMkheErrorV1::ResourceCeilingExceeded)?;
                let mut source = replay
                    .prerequisite
                    .live
                    .as_mut()
                    .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
                    .owner
                    .source
                    .read_canonical_plaintext_block_v1(record, block)?;
                let bytes = source.as_mut_bytes_v1();
                validate_canonical_source_block_v1(bytes)?;
                for encoded in bytes.chunks_exact(32) {
                    let encoded: &[u8; 32] = encoded
                        .try_into()
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?;
                    let scalar = ZeroizingT256ScalarCopyV1::new(
                        Scalar::from_be_bytes_exact_ref(encoded)
                            .map_err(|_| ZkAmsMkheErrorV1::InvalidPhase23Fold)?,
                    );
                    sink.absorb_next_scalar_v1(&scalar)?;
                }
                schedule.update(&record.to_be_bytes());
                schedule.update(&block.to_be_bytes());
            }
        }
        let weighted_columns = sink.finish_v1()?;
        let after_source_receipt = replay
            .prerequisite
            .live
            .as_ref()
            .ok_or(ZkAmsMkheErrorV1::InvalidPhase23Fold)?
            .owner
            .source
            .receipt_v1()
            .receipt_digest_v1();
        if after_source_receipt != source_receipt_digest {
            return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
        }
        let mut record = CanonicalReopenRecordV1 {
            source_receipt_digest,
            source_opening_record_digest: replay.openings.record.record_digest,
            authenticated_read_schedule_root: require_nonzero_opening_digest_v1(
                schedule.finalize(),
            )?,
            block_count: CANONICAL_REOPEN_BLOCK_COUNT_V1 as u32,
            scalar_count: SOURCE_OPENING_SCALAR_COUNT_V1,
            plaintext_bytes: CANONICAL_REOPEN_PLAINTEXT_BYTES_V1,
            authenticated_read_bytes: CANONICAL_REOPEN_AUTHENTICATED_READ_BYTES_V1,
            source_same_opening_proved: SOURCE_SAME_OPENING_PROVED_V1,
            packing_same_opening_proved: PACKING_SAME_OPENING_PROVED_V1,
            global_lookup_proof_verified: GLOBAL_LOOKUP_PROOF_VERIFIED_V1,
            zero_knowledge_accepted: ZERO_KNOWLEDGE_ACCEPTED_V1,
            authority_accepted: AUTHORITY_ACCEPTED_V1,
            operational_receipt_accepted: OPERATIONAL_RECEIPT_ACCEPTED_V1,
            rss_gate_accepted: RSS_GATE_ACCEPTED_V1,
            release_ready: RELEASE_READY_V1,
            release_complete: RELEASE_COMPLETE_V1,
            record_digest: [0; 32],
        };
        record.record_digest = canonical_reopen_record_digest_v1(&record)?;
        validate_canonical_reopen_record_v1(&record)?;
        Ok(Phase23GlobalLookupSourceReopenedV1 {
            replay,
            weighted_columns,
            record,
        })
    }
}

fn canonical_reopen_record_digest_v1(
    record: &CanonicalReopenRecordV1,
) -> Result<[u8; 32], ZkAmsMkheErrorV1> {
    let mut hash = Keccak256::new();
    hash.update(CANONICAL_REOPEN_RECORD_DOMAIN_V1);
    hash.update(&[SOURCE_OPENING_VERSION_V1]);
    for digest in [
        record.source_receipt_digest,
        record.source_opening_record_digest,
        record.authenticated_read_schedule_root,
    ] {
        hash.update(&require_nonzero_opening_digest_v1(digest)?);
    }
    hash.update(&record.block_count.to_be_bytes());
    hash.update(&record.scalar_count.to_be_bytes());
    hash.update(&record.plaintext_bytes.to_be_bytes());
    hash.update(&record.authenticated_read_bytes.to_be_bytes());
    hash.update(&[
        record.source_same_opening_proved as u8,
        record.packing_same_opening_proved as u8,
        record.global_lookup_proof_verified as u8,
        record.zero_knowledge_accepted as u8,
        record.authority_accepted as u8,
        record.operational_receipt_accepted as u8,
        record.rss_gate_accepted as u8,
        record.release_ready as u8,
        record.release_complete as u8,
    ]);
    require_nonzero_opening_digest_v1(hash.finalize())
}

fn validate_canonical_reopen_record_v1(
    record: &CanonicalReopenRecordV1,
) -> Result<(), ZkAmsMkheErrorV1> {
    if record.source_receipt_digest == [0; 32]
        || record.source_opening_record_digest == [0; 32]
        || record.authenticated_read_schedule_root == [0; 32]
        || record.block_count != CANONICAL_REOPEN_BLOCK_COUNT_V1 as u32
        || record.scalar_count != SOURCE_OPENING_SCALAR_COUNT_V1
        || record.plaintext_bytes != CANONICAL_REOPEN_PLAINTEXT_BYTES_V1
        || record.authenticated_read_bytes != CANONICAL_REOPEN_AUTHENTICATED_READ_BYTES_V1
        || record.source_same_opening_proved
        || record.packing_same_opening_proved
        || record.global_lookup_proof_verified
        || record.zero_knowledge_accepted
        || record.authority_accepted
        || record.operational_receipt_accepted
        || record.rss_gate_accepted
        || record.release_ready
        || record.release_complete
        || record.record_digest != canonical_reopen_record_digest_v1(record)?
    {
        return Err(ZkAmsMkheErrorV1::InvalidPhase23Fold);
    }
    Ok(())
}
