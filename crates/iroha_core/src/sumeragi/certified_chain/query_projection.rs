//! Private structural projections for the original native query verifier.
//!
//! These values are data, not finality capabilities. The complete source frame has already
//! passed the existing native frame/result checks before it can be consumed here. Its quorum,
//! signed availability, parent authority and fresh source join still have to pass the shared
//! verifier. No projection can be decoded, imported, or publicly constructed.
//!

use super::*;
use iroha_allocation::AllocationBudget;
use iroha_data_model::{
    consensus::FinalizedGlobalThresholdBeaconPulseV1,
    sumeragi_finality::{ExecutionResultCommitment, ScheduleOutcome},
};

/// The exact already checked predecessor fields needed by the shared successor relation.
/// Keeping this value does not retain its transaction/output graph or authenticate a new cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ParentProjection {
    pub(super) height: u64,
    pub(super) iroha_hash: HashOf<IrohaHeader>,
    pub(super) core_hash: Hash32,
    pub(super) result: Hash32,
    pub(super) root_scope: SumeragiRootScope,
    pub(super) beacon: Option<FinalizedGlobalThresholdBeaconPulseV1>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        state::World,
        sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
    };

    fn original(chain: &CertifiedTestChain, pool: &AllocationBudget) -> CommittedBlock {
        let block = read_durable_pinned_block_bounded(
            chain.kura(),
            NonZeroUsize::new(2).unwrap(),
            chain.committed(2).block_hash(),
            pool,
            iroha_data_model::sumeragi_finality::MAX_FINALITY_BLOCK_BYTES,
        )
        .unwrap();
        read_frame(block, 2, SumeragiRootScope::Global).unwrap()
    }

    #[test]
    fn query_projection_moves_checked_fields_and_retains_only_original_certificate_custody() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000)).unwrap();
        chain.commit(Vec::new());
        let pool = AllocationBudget::new(32 * 1024 * 1024);
        let original = original(&chain, &pool);
        let expected_parent = ParentProjection::from_original(&original);
        let expected_header = original.block.header();
        let certificate = original.block.commit_certificate().unwrap().clone();
        let before = pool.reserved_bytes();
        assert!(certificate.admitted_to(&pool));
        let projection = CheckedFrameProjection::from_original(original, &pool).unwrap();
        assert_eq!(projection.source_header(), expected_header);
        assert_eq!(projection.parent(), expected_parent);
        assert!(CommitCertificate::ptr_eq(
            &projection.certificate,
            &certificate,
        ));
        assert!(
            pool.reserved_bytes() < before,
            "the original block shell and its other funded fields must retire"
        );
        assert!(
            pool.reserved_bytes() > 0,
            "certificate custody remains live"
        );
        drop(certificate);
        assert!(pool.reserved_bytes() > 0);
        drop(projection);
        assert_eq!(pool.reserved_bytes(), 0);
    }

    #[test]
    fn query_payload_capture_matches_original_stream_and_enclosing_frame() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000)).unwrap();
        chain.commit(Vec::new());
        let pool = AllocationBudget::new(32 * 1024 * 1024);
        for height in [1, 2] {
            let block = chain.committed(height);
            let block = block.block();
            let maximum = block.encode_wire().unwrap().len();
            let length = block.resultless_proposal_wire_len().unwrap();
            assert!(
                length <= maximum,
                "certificate/result removal cannot grow canonical framing"
            );
            let mut reference = Vec::new();
            block
                .write_resultless_proposal_wire(&mut reference)
                .unwrap();
            let captured =
                query_reader::capture_payload(block, length, maximum, &pool, height).unwrap();
            assert_eq!(captured.as_slice(), reference);
            drop(captured);
            assert_eq!(pool.reserved_bytes(), 0);
            assert!(
                query_reader::capture_payload(block, length, length - 1, &pool, height).is_err()
            );
            assert_eq!(pool.reserved_bytes(), 0);
        }
    }

    #[test]
    fn query_header_reconstruction_rechecks_exact_original_hash_and_scalars() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000)).unwrap();
        chain.commit(Vec::new());
        let pool = AllocationBudget::new(32 * 1024 * 1024);
        let mut projection =
            CheckedFrameProjection::from_original(original(&chain, &pool), &pool).unwrap();
        assert!(projection.reconstruct_header().unwrap().is_some());
        projection.header.as_mut().unwrap().origin_view += 1;
        assert!(matches!(
            projection.reconstruct_header(),
            Err(ExecutionAttemptError::Rejected(
                ChainReadError::HeaderMismatch { height: 2 }
            ))
        ));
        projection.header.as_mut().unwrap().origin_view -= 1;
        projection.core_hash = Hash32([0x91; 32]);
        assert!(matches!(
            projection.reconstruct_header(),
            Err(ExecutionAttemptError::Rejected(
                ChainReadError::HeaderMismatch { height: 2 }
            ))
        ));
        drop(projection);
        assert_eq!(pool.reserved_bytes(), 0);
    }

    #[test]
    fn query_projection_refuses_an_equal_sized_foreign_pool_without_recharging() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 10_000)).unwrap();
        chain.commit(Vec::new());
        let pool = AllocationBudget::new(32 * 1024 * 1024);
        let foreign = AllocationBudget::new(pool.limit_bytes());
        let original = original(&chain, &pool);
        assert!(matches!(
            CheckedFrameProjection::from_original(original, &foreign),
            Err(ExecutionAttemptError::Deferred(_))
        ));
        assert_eq!(foreign.reserved_bytes(), 0);
        assert_eq!(pool.reserved_bytes(), 0);
    }
}

impl ParentProjection {
    pub(super) fn from_original(original: &CommittedBlock) -> Self {
        Self {
            height: original.height,
            iroha_hash: original.block_hash(),
            core_hash: original.core_hash,
            result: original.result,
            root_scope: original.root_scope,
            beacon: original.commitment().beacon,
        }
    }
}

/// Borrowed inputs to the same native signature, scope and boundary predicates.
/// Both ordinary retained bodies and a query's private projection lend this exact shape.
#[derive(Clone, Copy)]
pub(super) struct CheckedFrameRef<'a> {
    pub(super) parent: ParentProjection,
    pub(super) previous_iroha_hash: Option<HashOf<IrohaHeader>>,
    pub(super) header: Option<&'a PrefixHeader>,
    pub(super) active_result: bool,
    pub(super) schedule: &'a ScheduleOutcome,
    pub(super) certificate: Option<&'a CommitCertificate>,
}

/// The exact source data lent to the existing certificate relation.
#[derive(Clone, Copy)]
pub(super) enum CertificateSource<'a> {
    Original(&'a CommittedBlock),
    Projected {
        frame: &'a CheckedFrameProjection,
        header: Option<&'a PrefixHeader>,
        payload: Option<&'a iroha_sumeragi::availability::PayloadBytes>,
        wire: (u64, Hash),
    },
}

impl<'a> CertificateSource<'a> {
    pub(super) fn frame(self) -> CheckedFrameRef<'a> {
        match self {
            Self::Original(original) => CheckedFrameRef::from_original(original),
            Self::Projected { frame, header, .. } => frame.borrowed(header),
        }
    }

    pub(super) fn wire_identity(self) -> Option<(u64, Hash)> {
        match self {
            Self::Original(original) => original.block.canonical_wire_identity().ok(),
            Self::Projected { wire, .. } => Some(wire),
        }
    }
}

/// Borrowed original prefix fields; both lifetime owners enter one successor kernel.
pub(super) struct PrefixRef<'a> {
    pub(super) tip: ParentProjection,
    pub(super) schedule: &'a schedule::ConsensusSchedule,
    pub(super) authority: &'a PrefixAuthority,
    pub(super) validation: &'a mut EpochValidationScope,
    pub(super) proof_source: Option<Hash>,
}

impl<'a> PrefixRef<'a> {
    pub(super) fn from_original(prefix: &'a mut VerifiedPrefix) -> Self {
        Self {
            tip: ParentProjection::from_original(&prefix.tip),
            schedule: &prefix.schedule,
            authority: &prefix.authority,
            validation: &mut prefix.validation,
            proof_source: prefix.proof_source,
        }
    }
}

impl<'a> CheckedFrameRef<'a> {
    pub(super) fn extends(self, parent: ParentProjection) -> bool {
        parent.height.checked_add(1) == Some(self.parent.height)
            && self.previous_iroha_hash == Some(parent.iroha_hash)
            && self.header.is_some_and(|header| {
                header.parent_hash == parent.core_hash && header.parent_result == parent.result
            })
    }

    pub(super) fn from_original(original: &'a CommittedBlock) -> Self {
        Self {
            parent: ParentProjection::from_original(original),
            previous_iroha_hash: original.block.header().prev_block_hash(),
            header: original.header.as_ref(),
            active_result: original.certified_result.header().is_active(),
            schedule: &original.commitment().schedule,
            certificate: original.block.commit_certificate(),
        }
    }
}

/// The bounded structural fields of one native body, retained after that body is destroyed.
/// The certificate shares its original four charged immutable leaves without copying them.
/// Its QC and availability bytes remain undecoded until their original verifier positions.
pub(super) struct CheckedFrameProjection {
    source_header: IrohaHeader,
    iroha_hash: HashOf<IrohaHeader>,
    header: Option<HeaderIdentity>,
    core_hash: Hash32,
    result: Hash32,
    root_scope: SumeragiRootScope,
    active_result: bool,
    schedule: ScheduleOutcome,
    beacon: Option<FinalizedGlobalThresholdBeaconPulseV1>,
    certificate: CommitCertificate,
}

impl CheckedFrameProjection {
    /// Consume only an original native read whose body and certificate share the supplied pool.
    /// This moves the already checked header/schedule and shares certificate custody. It does
    /// not encode, decode, validate a quorum, create a cache entry or issue a receipt.
    pub(super) fn from_original(
        original: CommittedBlock,
        budget: &AllocationBudget,
    ) -> Result<Self, ExecutionAttemptError<ChainReadError>> {
        let certificate =
            original
                .block
                .commit_certificate()
                .ok_or(ChainReadError::MissingCertificate {
                    height: original.height,
                })?;
        if !original.block.belongs_to(budget) || !certificate.admitted_to(budget) {
            return Err(ExecutionAttemptError::Deferred(
                retained_query_authority::invariant(),
            ));
        }
        let source_header = original.block.header();
        let iroha_hash = original.block_hash();
        // Sharing is allocation-free and retains all four original charged byte leaves.
        let certificate = certificate.clone();
        let CommittedBlock {
            height: _,
            block,
            header,
            core_hash,
            result,
            certified_result,
            root_scope,
        } = original;
        let (result_header, commitment) = certified_result.into_parts();
        let ExecutionResultCommitment {
            height: _,
            execution,
            schedule,
            beacon,
            native_lanes,
        } = commitment;
        // The common successor/signature predicates need none of these full-body fields.
        // Their source-bound C1-C5 relations already ran before this private conversion.
        drop((block, execution, native_lanes));
        Ok(Self {
            source_header,
            iroha_hash,
            header: header
                .as_ref()
                .map(|header| HeaderIdentity::capture(header)),
            core_hash,
            result,
            root_scope,
            active_result: result_header.is_active(),
            schedule,
            beacon,
            certificate,
        })
    }

    pub(super) fn borrowed<'a>(&'a self, header: Option<&'a PrefixHeader>) -> CheckedFrameRef<'a> {
        CheckedFrameRef {
            parent: ParentProjection {
                height: self.source_header.height().get(),
                iroha_hash: self.iroha_hash,
                core_hash: self.core_hash,
                result: self.result,
                root_scope: self.root_scope,
                beacon: self.beacon,
            },
            previous_iroha_hash: self.source_header.prev_block_hash(),
            header,
            active_result: self.active_result,
            schedule: &self.schedule,
            certificate: Some(&self.certificate),
        }
    }

    pub(super) fn parent(&self) -> ParentProjection {
        ParentProjection {
            height: self.source_header.height().get(),
            iroha_hash: self.iroha_hash,
            core_hash: self.core_hash,
            result: self.result,
            root_scope: self.root_scope,
            beacon: self.beacon,
        }
    }

    pub(super) fn schedule(&self) -> &ScheduleOutcome {
        &self.schedule
    }

    /// Reconstruct only within the heavy verification stage from the exact charged
    /// original leaf. This is the same canonical header decoder used by C1, followed
    /// by its original hash/scalar join; it supplies no new authority.
    pub(super) fn reconstruct_header(
        &self,
    ) -> Result<Option<PrefixHeader>, ExecutionAttemptError<ChainReadError>> {
        let Some(expected) = self.header else {
            return Ok(None);
        };
        let height = self.source_header.height().get();
        let header = decode_original_header(&self.certificate, height)?;
        if HeaderIdentity::capture(&header) != expected
            || header.hash(&BlsCrypto::new()) != self.core_hash
        {
            return Err(ChainReadError::HeaderMismatch { height }.into());
        }
        Ok(Some(header))
    }

    pub(super) fn source_header(&self) -> IrohaHeader {
        self.source_header
    }
}

/// Constant-size identity of the exact earlier C1-checked header. Variable header
/// children are discarded with the decoded native body, even for malformed input.
#[derive(Clone, Copy, PartialEq, Eq)]
struct HeaderIdentity {
    instance: Hash32,
    epoch: iroha_sumeragi::types::EpochId,
    height: u64,
    origin_view: u64,
    parent_hash: Hash32,
    parent_result: Hash32,
    payload_hash: Hash32,
    availability_digest: Hash32,
    payload_len: u32,
    proposer: iroha_sumeragi::types::ValidatorIndex,
    skipped_leaders: usize,
    control_bytes: usize,
}
impl HeaderIdentity {
    fn capture(header: &BlockHeader) -> Self {
        Self {
            instance: header.instance,
            epoch: header.epoch,
            height: header.height,
            origin_view: header.origin_view,
            parent_hash: header.parent_hash,
            parent_result: header.parent_result,
            payload_hash: header.payload_hash,
            availability_digest: header.availability_digest,
            payload_len: header.payload_len,
            proposer: header.proposer,
            skipped_leaders: header.skipped_leaders.len(),
            control_bytes: header.control_witness.as_slice().len(),
        }
    }
}
