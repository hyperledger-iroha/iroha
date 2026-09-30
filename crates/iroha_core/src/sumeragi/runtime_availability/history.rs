//! One retained authenticated archive scan, bounded to a captured original State publication.

use super::{invalid, pending, selection::LaneSelection};
use crate::{
    kura::Kura,
    query::native_context_archive::{NativeContextArchive, NativeContextArchiveError},
    state::{NativeExecutionEvidenceLimits, NativeExecutionEvidenceVerifier, State, StateReadOnly},
};
use iroha_crypto::HashOf;
use iroha_data_model::{
    block::{BlockHeader, SignedBlock},
    sumeragi_finality::MAX_FINALITY_BLOCK_BYTES,
    sumeragi_lanes::SumeragiLaneRecord,
};
use iroha_model_base::topology::LaneId;
use std::{io, num::NonZeroUsize, sync::Arc};

pub(super) struct HistoryScan {
    lane: LaneId,
    incarnation: [u8; 32],
    generation: u64,
    height: u64,
    carrier: HashOf<BlockHeader>,
    kura: Arc<Kura>,
    archive: NativeContextArchive,
    verifier: NativeExecutionEvidenceVerifier,
    selected: LaneSelection,
    next: u64,
    current: Option<Arc<SignedBlock>>,
}

impl HistoryScan {
    pub(super) fn open(
        state: &State,
        lane: LaneId,
        incarnation: [u8; 32],
    ) -> io::Result<Option<Self>> {
        let generation = state.state_view_generation();
        if generation % 2 != 0 {
            return Err(pending("native State publication is in progress"));
        }
        let (height, carrier) = {
            let view = state.view();
            if view.kura().native_consensus_gate().is_closed() {
                return Err(io::Error::other(
                    "native storage gate is closed; restart is required",
                ));
            }
            let Some(tip) = view.native_execution_tip() else {
                return Ok(None);
            };
            if tip.height() != view.height() as u64
                || Some(tip.iroha_hash()) != view.latest_block_hash()
            {
                return Err(invalid(
                    "native lane lookup has no matching original State tip",
                ));
            }
            (tip.height(), tip.iroha_hash())
        };
        if height < 2 {
            return Ok(None);
        }
        let kura = state.kura_handle();
        let archive = NativeContextArchive::open_existing(
            &kura,
            state.ivm_execution_budget(),
            kura.native_context_archive_max_bytes(),
        )
        .map_err(archive_error)?;
        let state_bound =
            u64::try_from(kura.native_context_archive_max_bytes().get()).map_err(invalid)?;
        let block_bound = MAX_FINALITY_BLOCK_BYTES as u64;
        let retained = block_bound
            .checked_add(state_bound)
            .and_then(|bytes| bytes.checked_mul(height))
            .ok_or_else(|| invalid("native authority history bound overflow"))?;
        let verifier = NativeExecutionEvidenceVerifier::new(
            state.chain_id_ref().clone(),
            *state.network_id_ref(),
            NativeExecutionEvidenceLimits {
                max_carriers: height,
                max_carrier_bytes: block_bound,
                max_context_bytes: state_bound,
                max_retained_bytes: retained,
            },
        )
        .map_err(invalid)?;
        Ok(Some(Self {
            lane,
            incarnation,
            generation,
            height,
            carrier,
            kura,
            archive,
            verifier,
            selected: LaneSelection::new(lane, incarnation),
            next: 1,
            current: None,
        }))
    }

    pub(super) fn matches(&self, lane: LaneId, incarnation: &[u8; 32]) -> bool {
        self.lane == lane && &self.incarnation == incarnation
    }

    pub(super) fn generation(&self) -> u64 {
        self.generation
    }

    // Local archive admission refusal leaves the verified prefix, selected original record,
    // exact current carrier and retained directory descriptor untouched for the next call.
    pub(super) fn complete(&mut self) -> io::Result<()> {
        while self.next <= self.height {
            if self.current.is_none() {
                let index = usize::try_from(self.next)
                    .ok()
                    .and_then(NonZeroUsize::new)
                    .ok_or_else(|| invalid("native carrier height overflow"))?;
                self.current = Some(self.kura.get_block(index).ok_or_else(|| {
                    invalid(format!("native carrier {} is unreadable", self.next))
                })?);
            }
            let block = self.current.as_ref().expect("retained original carrier");
            if self.next == self.height && block.hash() != self.carrier {
                return Err(invalid(
                    "native authority prefix differs from captured State tip",
                ));
            }
            let bytes = self
                .archive
                .read_exact(self.next, block.hash())
                .map_err(archive_error)?;
            if let Some(receipt) = self
                .verifier
                .push_shared_height(Arc::clone(block), bytes.as_slice())
                .map_err(invalid)?
            {
                self.selected
                    .observe(receipt.block().header().height().get(), receipt.lanes())?;
            }
            self.current = None;
            self.next = self
                .next
                .checked_add(1)
                .ok_or_else(|| invalid("native authority height exhausted"))?;
        }
        self.archive.recheck_namespace().map_err(archive_error)
    }

    pub(super) fn finish(self) -> Option<SumeragiLaneRecord> {
        self.selected.finish(self.height)
    }
}

fn archive_error(error: NativeContextArchiveError) -> io::Error {
    if error.is_local_refusal() {
        return io::Error::new(io::ErrorKind::WouldBlock, error);
    }
    match error {
        NativeContextArchiveError::Io(error) => error,
        error => io::Error::new(io::ErrorKind::InvalidData, error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_allocation::{AllocationBudget, ChargedBuffer};

    #[test]
    fn archive_allocation_refusal_remains_retryable_with_its_original_error() {
        let budget = AllocationBudget::new(1);
        let held = ChargedBuffer::<u8>::new(1, &budget).unwrap();
        let refused = ChargedBuffer::<u8>::new(1, &budget).err().unwrap();
        let mapped = archive_error(NativeContextArchiveError::Allocation(refused));
        assert_eq!(mapped.kind(), io::ErrorKind::WouldBlock);
        assert!(mapped.get_ref().unwrap().is::<NativeContextArchiveError>());
        assert_eq!(budget.reserved_bytes(), 1);
        drop(held);
    }

    #[test]
    fn missing_archive_and_malformed_source_are_not_absence_or_capacity_refusal() {
        let io = archive_error(NativeContextArchiveError::Io(
            io::ErrorKind::NotFound.into(),
        ));
        assert_eq!(io.kind(), io::ErrorKind::NotFound);
        for error in [
            NativeContextArchiveError::Source("substituted archive"),
            NativeContextArchiveError::Limit {
                maximum: 16,
                actual: 17,
            },
        ] {
            let mapped = archive_error(error);
            assert_eq!(mapped.kind(), io::ErrorKind::InvalidData);
            assert!(mapped.get_ref().unwrap().is::<NativeContextArchiveError>());
        }
    }
}
