//! Private metadata borrowed only from native receipts or sealed portable ancestry.

use super::*;
use iroha_data_model::sumeragi_finality::PortableCertifiedBlockV1;

/// This private trait has exactly two authenticated owners and grants no body custody.
pub(super) trait ParentServiceBlock {
    fn height(&self) -> u64;
    fn block_hash(&self) -> HashOf<IrohaHeader>;
    fn core_hash(&self) -> Hash32;
    fn result(&self) -> Hash32;
    fn header(&self) -> Option<&BlockHeader>;
    fn iroha_header(&self) -> IrohaHeader;
    fn root_scope(&self) -> SumeragiRootScope;
    fn active_result(&self) -> bool;
    fn commitment(&self) -> &ExecutionResultCommitment;
    fn block_time_ms(&self) -> u64 {
        u64::try_from(self.iroha_header().creation_time().as_millis()).unwrap_or(u64::MAX)
    }
}
impl ParentServiceBlock for CommittedBlock {
    fn height(&self) -> u64 {
        CommittedBlock::height(self)
    }
    fn block_hash(&self) -> HashOf<IrohaHeader> {
        CommittedBlock::block_hash(self)
    }
    fn core_hash(&self) -> Hash32 {
        CommittedBlock::core_hash(self)
    }
    fn result(&self) -> Hash32 {
        CommittedBlock::result(self)
    }
    fn header(&self) -> Option<&BlockHeader> {
        CommittedBlock::header(self)
    }
    fn iroha_header(&self) -> IrohaHeader {
        self.block().header()
    }
    fn root_scope(&self) -> SumeragiRootScope {
        CommittedBlock::root_scope(self)
    }
    fn active_result(&self) -> bool {
        self.certified_result().header().is_active()
    }
    fn commitment(&self) -> &ExecutionResultCommitment {
        CommittedBlock::commitment(self)
    }
}
impl ParentServiceBlock for PortableCertifiedBlockV1 {
    fn height(&self) -> u64 {
        self.header().height().get()
    }
    fn block_hash(&self) -> HashOf<IrohaHeader> {
        self.header().hash()
    }
    fn core_hash(&self) -> Hash32 {
        PortableCertifiedBlockV1::core_hash(self)
    }
    fn result(&self) -> Hash32 {
        PortableCertifiedBlockV1::result(self)
    }
    fn header(&self) -> Option<&BlockHeader> {
        self.core_header()
    }
    fn iroha_header(&self) -> IrohaHeader {
        *PortableCertifiedBlockV1::header(self)
    }
    fn root_scope(&self) -> SumeragiRootScope {
        PortableCertifiedBlockV1::root_scope(self)
    }
    fn active_result(&self) -> bool {
        self.certified_result().header().is_active()
    }
    fn commitment(&self) -> &ExecutionResultCommitment {
        self.certified_result().commitment()
    }
}

pub(super) fn parent_service_authority<'a>(
    predecessor: &'a dyn ParentServiceBlock,
    parent: &dyn ParentServiceBlock,
    budget: &iroha_allocation::AllocationBudget,
) -> Result<FundedExecutionAuthority<'a>, VerificationReadError> {
    let height = parent.height();
    let header = parent
        .header()
        .ok_or(ChainReadError::HeaderMismatch { height })?;
    if predecessor.height().checked_add(1) != Some(height)
        || parent.root_scope() != predecessor.root_scope()
        || parent.iroha_header().prev_block_hash() != Some(predecessor.block_hash())
        || header.parent_hash != predecessor.core_hash()
        || header.parent_result != predecessor.result()
    {
        return Err(ChainReadError::Discontinuous { height }.into());
    }
    predecessor
        .commitment()
        .schedule
        .validate_successor(&parent.commitment().schedule)
        .map_err(|error| ChainReadError::Committee {
            height,
            reason: error.to_string(),
        })?;
    let schedule::ScheduledSlot::Ready(scheduled) = &predecessor.commitment().schedule.next else {
        return Err(ChainReadError::Committee {
            height,
            reason: "parent has no authorized successor".into(),
        }
        .into());
    };
    FundedExecutionAuthority::admit(scheduled, height, budget)
}

pub(super) fn verify_service_boundary(
    current: &dyn ParentServiceBlock,
    predecessor: &dyn ParentServiceBlock,
    authority: AuthorityRef<'_>,
) -> Result<(), ChainReadError> {
    verify_boundary_context(
        current.height(),
        &current.commitment().schedule,
        &ParentProjection {
            height: predecessor.height(),
            iroha_hash: predecessor.block_hash(),
            core_hash: predecessor.core_hash(),
            result: predecessor.result(),
            root_scope: predecessor.root_scope(),
            beacon: predecessor.commitment().beacon,
        },
        authority,
    )
}
