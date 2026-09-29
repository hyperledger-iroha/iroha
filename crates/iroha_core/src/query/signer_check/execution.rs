//! One exact signed Check relation over a borrowed applied view and certified blocks.
//!
//! Preparation and consumption establish execution only. Purpose wrappers own current
//! authority, UTC endpoints and private-use fences; no callback can supply eligibility here.

use super::*;

/// Prepared execution relation. Its source view and original round cannot be replaced.
pub(crate) struct PreparedCheckExecutionV1<'view, 'state, 'round> {
    view: &'view StateView<'state>,
    round: &'round NativeCheckRoundV1,
    bound: BoundNativeCheckV1,
    entry_hash: HashOf<TransactionEntrypoint>,
    check_height: u64,
    applied_height: u64,
    next_height: Option<u64>,
    check_block_hash: Option<[u8; 32]>,
    applied_floor: NativeCheckFloorV1,
    failed: bool,
}

impl<'view, 'state, 'round> PreparedCheckExecutionV1<'view, 'state, 'round> {
    pub(crate) fn new(
        view: &'view StateView<'state>,
        purpose: NativeCustodyCheckPurposeV1,
        bound: BoundNativeCheckV1,
        round: &'round NativeCheckRoundV1,
    ) -> Result<Self, Error> {
        round.ensure_live()?;
        if !round.bound
            || bound.purpose != purpose
            || bound.started != round.started
            || bound.max_elapsed != round.max_elapsed
            || Some(bound.challenge) != round.challenge
        {
            return Err(Error::Invalid);
        }
        if view.network_id().as_bytes() != &bound.network_id
            || view.chain_id().to_string() != bound.chain_id
        {
            return Err(Error::Finality);
        }
        let entry_hash = bound.signed.hash_as_entrypoint();
        let height_index = view
            .transactions
            .get(&entry_hash)
            .ok_or(Error::NotApplied)?;
        let check_height = u64::try_from(height_index.get()).map_err(|_| Error::NotApplied)?;
        let applied_height =
            u64::try_from(view.block_hashes().len()).map_err(|_| Error::NotApplied)?;
        if check_height <= bound.floor.height || check_height > applied_height {
            return Err(Error::NotApplied);
        }
        check_history_span_v1(bound.floor.height, applied_height)?;
        let applied_floor = bound.floor;
        Ok(Self {
            view,
            round,
            bound,
            entry_hash,
            check_height,
            applied_height,
            next_height: Some(applied_floor.height),
            check_block_hash: None,
            applied_floor,
            failed: false,
        })
    }

    pub(crate) const fn floor_height(&self) -> u64 {
        self.bound.floor.height
    }
    pub(crate) const fn check_height(&self) -> u64 {
        self.check_height
    }
    pub(crate) const fn applied_height(&self) -> u64 {
        self.applied_height
    }

    /// Consume every floor..tip block in order from the caller's same-view certified walk.
    /// A failed attempt poisons this preparation, including an attempted retry of that block.
    pub(crate) fn consume(&mut self, block: &SignerCertifiedBlockV1<'_, '_>) -> Result<(), Error> {
        if self.failed {
            return Err(Error::Execution);
        }
        self.failed = true;
        self.round.ensure_live()?;
        let block = block.in_view(self.view)?;
        let height = block.height();
        let offset = height
            .checked_sub(1)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(Error::Finality)?;
        if self.next_height != Some(height)
            || self.view.block_hashes().get(offset).copied() != Some(block.block_hash())
        {
            return Err(Error::Finality);
        }
        let hash = *block.block_hash().as_ref();
        if height == self.bound.floor.height
            && (hash != self.bound.floor.block_hash || block.id() != self.bound.floor.context_id)
        {
            return Err(Error::Finality);
        }
        if height == self.check_height {
            let anchor = block
                .entry_anchor(&self.entry_hash)
                .map_err(|_| Error::Execution)?;
            let body = block.block();
            let proofs = body
                .network_execution_proof(&self.entry_hash)
                .ok_or(Error::Execution)?;
            if !proofs.verify(&anchor) {
                return Err(Error::Execution);
            }
            let entry_index =
                usize::try_from(anchor.entry_index()).map_err(|_| Error::Execution)?;
            let actual = body
                .network_entrypoint_at(entry_index)
                .ok_or(Error::Execution)?;
            let (_, output) = body
                .network_output_at(anchor.entry_index())
                .ok_or(Error::Execution)?;
            if bounded_entry(actual).map_err(|_| Error::Execution)? != self.bound.entry_bytes
                || !output.result.is_ok()
            {
                return Err(Error::Execution);
            }
            self.check_block_hash = Some(hash);
        }
        self.applied_floor = NativeCheckFloorV1 {
            height,
            block_hash: hash,
            context_id: block.id(),
        };
        self.next_height = if height == self.applied_height {
            None
        } else {
            height.checked_add(1)
        };
        self.failed = false;
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<BorrowedCheckExecutionCutV1<'view, 'state>, Error> {
        self.round.ensure_live()?;
        if self.failed
            || self.next_height.is_some()
            || self.applied_floor.height != self.applied_height
        {
            return Err(Error::Execution);
        }
        Ok(BorrowedCheckExecutionCutV1 {
            view: self.view,
            data: CheckExecutionDataV1 {
                check_height: self.check_height,
                applied_floor: self.applied_floor,
                entry_hash: self.entry_hash,
                canonical_external: self.bound.entry_bytes,
                check_block_hash: self.check_block_hash.ok_or(Error::Execution)?,
            },
        })
    }
}

/// Result remains bound to the immutable view used by every block and entry relation.
pub(crate) struct BorrowedCheckExecutionCutV1<'view, 'state> {
    view: &'view StateView<'state>,
    data: CheckExecutionDataV1,
}
impl<'view, 'state> BorrowedCheckExecutionCutV1<'view, 'state> {
    pub(crate) fn view(&self) -> &'view StateView<'state> {
        self.view
    }
    pub(crate) const fn applied_floor(&self) -> NativeCheckFloorV1 {
        self.data.applied_floor
    }
    pub(crate) const fn entry_hash(&self) -> HashOf<TransactionEntrypoint> {
        self.data.entry_hash
    }
    pub(crate) fn into_verified_entry(self) -> (Vec<u8>, [u8; 32]) {
        (self.data.canonical_external, self.data.check_block_hash)
    }
    // Only the parent generic owner may move this data into its already-owned same view.
    pub(super) fn into_data(self) -> CheckExecutionDataV1 {
        self.data
    }
}
pub(super) struct CheckExecutionDataV1 {
    pub(super) check_height: u64,
    pub(super) applied_floor: NativeCheckFloorV1,
    pub(super) entry_hash: HashOf<TransactionEntrypoint>,
    pub(super) canonical_external: Vec<u8>,
    pub(super) check_block_hash: [u8; 32],
}
