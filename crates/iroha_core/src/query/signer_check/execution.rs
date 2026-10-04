//! One exact signed Check relation over a borrowed applied view and certified blocks.
//!
//! Preparation and consumption establish execution only. Purpose wrappers own current
//! authority, UTC endpoints and private-use fences; no callback can supply eligibility here.

use super::*;
use crate::execution_attempt::{ExecutionAttemptError, norito_decode_attempt_error};

/// Prepared execution relation. Its source view and original round cannot be replaced.
pub(crate) struct PreparedCheckExecutionV1<'view, 'state, 'round, 'bound> {
    view: &'view StateView<'state>,
    round: &'round NativeCheckRoundV1,
    bound: &'bound mut Option<BoundNativeCheckV1>,
    entry_hash: HashOf<TransactionEntrypoint>,
    check_height: u64,
    applied_height: u64,
    next_height: Option<u64>,
    check_block_hash: Option<[u8; 32]>,
    applied_floor: NativeCheckFloorV1,
    failed: bool,
}

impl<'view, 'state, 'round, 'bound> PreparedCheckExecutionV1<'view, 'state, 'round, 'bound> {
    /// Borrow the only original binding slot; refusal leaves it occupied unchanged.
    pub(crate) fn new(
        view: &'view StateView<'state>,
        purpose: NativeCustodyCheckPurposeV1,
        bound: &'bound mut Option<BoundNativeCheckV1>,
        round: &'round NativeCheckRoundV1,
    ) -> Result<Self, ExecutionAttemptError<Error>> {
        let admitted = (|| -> Result<_, ExecutionAttemptError<Error>> {
            round.ensure_live()?;
            let bound = bound.as_ref().ok_or(Error::Invalid)?;
            if !round.bound
                || bound.purpose != purpose
                || bound.started != round.started
                || bound.max_elapsed != round.max_elapsed
                || Some(bound.challenge) != round.challenge
            {
                return Err(Error::Invalid.into());
            }
            if view.network_id().as_bytes() != &bound.network_id
                || view.chain_id().as_str().as_bytes() != bound.chain_id.as_bytes()
            {
                return Err(Error::Finality.into());
            }
            let entry_hash = bound
                .signed
                .try_hash_as_entrypoint()
                .map_err(native_codec_attempt_error)?;
            let height_index = view
                .transactions
                .get(&entry_hash)
                .ok_or(Error::NotApplied)?;
            let check_height = u64::try_from(height_index.get()).map_err(|_| Error::NotApplied)?;
            let applied_height =
                u64::try_from(view.block_hashes().len()).map_err(|_| Error::NotApplied)?;
            if check_height <= bound.floor.height || check_height > applied_height {
                return Err(Error::NotApplied.into());
            }
            check_history_span_v1(bound.floor.height, applied_height)?;
            Ok((entry_hash, check_height, applied_height))
        })();
        let (entry_hash, check_height, applied_height) = admitted?;
        let applied_floor = bound
            .as_ref()
            .expect("validated original binding slot")
            .floor;
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

    pub(crate) fn floor_height(&self) -> u64 {
        self.bound
            .as_ref()
            .expect("original binding remains in its exclusive slot")
            .floor
            .height
    }
    pub(crate) const fn check_height(&self) -> u64 {
        self.check_height
    }
    pub(crate) const fn applied_height(&self) -> u64 {
        self.applied_height
    }

    /// Consume every floor..tip block in order from the caller's same-view certified walk.
    /// A failed attempt poisons this preparation, including an attempted retry of that block.
    pub(crate) fn consume(
        &mut self,
        block: &SignerCertifiedBlockV1<'_, '_>,
    ) -> Result<(), ExecutionAttemptError<Error>> {
        if self.failed {
            return Err(Error::Execution.into());
        }
        self.failed = true;
        self.round.ensure_live()?;
        let block = block.in_view(self.view)?;
        let bound = self
            .bound
            .as_ref()
            .expect("original binding remains in its exclusive slot");
        let height = block.height();
        let offset = height
            .checked_sub(1)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(Error::Finality)?;
        if self.next_height != Some(height)
            || self.view.block_hashes().get(offset).copied() != Some(block.block_hash())
        {
            return Err(Error::Finality.into());
        }
        let hash = *block.block_hash().as_ref();
        if height == bound.floor.height
            && (hash != bound.floor.block_hash || block.id() != bound.floor.context_id)
        {
            return Err(Error::Finality.into());
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
                return Err(Error::Execution.into());
            }
            let entry_index =
                usize::try_from(anchor.entry_index()).map_err(|_| Error::Execution)?;
            let actual = body
                .network_entrypoint_at(entry_index)
                .ok_or(Error::Execution)?;
            let (_, output) = body
                .network_output_at(anchor.entry_index())
                .ok_or(Error::Execution)?;
            norito::verify_exact_canonical_frame(actual, bound.entry_bytes.as_slice())
                .map_err(|error| norito_decode_attempt_error(error, |_| Error::Execution))?;
            if !output.result.is_ok() {
                return Err(Error::Execution.into());
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

    fn validate_finish(&self) -> Result<(), ExecutionAttemptError<Error>> {
        self.round.ensure_live()?;
        if self.failed
            || self.next_height.is_some()
            || self.applied_floor.height != self.applied_height
        {
            return Err(Error::Execution.into());
        }
        Ok(())
    }

    /// Finish while retaining the exclusive original slot through all late purpose checks.
    pub(crate) fn finish(
        self,
    ) -> Result<BorrowedCheckExecutionCutV1<'view, 'state, 'bound>, ExecutionAttemptError<Error>>
    {
        self.validate_finish()?;
        let check_block_hash = self.check_block_hash.ok_or(Error::Execution)?;
        Ok(BorrowedCheckExecutionCutV1 {
            view: self.view,
            bound: self.bound,
            data: CheckExecutionDataV1 {
                check_height: self.check_height,
                applied_floor: self.applied_floor,
                entry_hash: self.entry_hash,
                check_block_hash,
            },
        })
    }
}

/// Metadata proof retains the exclusive original signed binding until final discharge.
/// No method accepts an interchangeable frame or a replacement binding.
pub(crate) struct BorrowedCheckExecutionCutV1<'view, 'state, 'bound> {
    view: &'view StateView<'state>,
    bound: &'bound mut Option<BoundNativeCheckV1>,
    data: CheckExecutionDataV1,
}
impl<'view, 'state, 'bound> BorrowedCheckExecutionCutV1<'view, 'state, 'bound> {
    pub(crate) fn view(&self) -> &'view StateView<'state> {
        self.view
    }
    pub(crate) const fn applied_floor(&self) -> NativeCheckFloorV1 {
        self.data.applied_floor
    }
    pub(crate) const fn entry_hash(&self) -> HashOf<TransactionEntrypoint> {
        self.data.entry_hash
    }
    pub(crate) const fn check_block_hash(&self) -> [u8; 32] {
        self.data.check_block_hash
    }

    /// Consume only the original slot borrowed before proof work; refusal paths never take it.
    pub(crate) fn into_verified_entry(self) -> (iroha_allocation::ChargedBuffer<u8>, [u8; 32]) {
        let bound = self
            .bound
            .take()
            .expect("verified original binding remains in its exclusive slot");
        (bound.entry_bytes, self.data.check_block_hash)
    }

    // The parent may move its own view only while retaining this same exclusive slot.
    pub(super) fn into_parts(
        self,
    ) -> (CheckExecutionDataV1, &'bound mut Option<BoundNativeCheckV1>) {
        (self.data, self.bound)
    }
}

pub(super) struct CheckExecutionDataV1 {
    pub(super) check_height: u64,
    pub(super) applied_floor: NativeCheckFloorV1,
    pub(super) entry_hash: HashOf<TransactionEntrypoint>,
    pub(super) check_block_hash: [u8; 32],
}
