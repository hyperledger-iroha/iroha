//! One retained authenticated archive scan, bounded to a captured original State publication.

use super::{invalid, pending, selection::LaneSelection};
use crate::query::native_receipts::lane_payload::{
    LaneAuthority, LaneAuthorityRead, LanePayloadError, LanePayloadRead,
};
use crate::{
    kura::Kura,
    query::native_context_archive::{
        NativeContextArchive, NativeContextArchiveError, NativeContextRead,
    },
    state::{NativeExecutionEvidenceLimits, NativeExecutionEvidenceVerifier, State, StateReadOnly},
};
use iroha_allocation::{AllocationBudget, ChargedBuffer};
use iroha_crypto::HashOf;
use iroha_data_model::{
    block::{BlockHeader, SignedBlock},
    sumeragi_finality::MAX_FINALITY_BLOCK_BYTES,
};
use iroha_model_base::topology::LaneId;
use std::{io, num::NonZeroUsize, sync::Arc};

enum AuthorityRead {
    Payload(LanePayloadRead),
    Config(LaneAuthorityRead),
    Ready(LaneAuthority),
}

pub(super) struct HistoryScan {
    lane: LaneId,
    incarnation: [u8; 32],
    generation: u64,
    height: u64,
    carrier: HashOf<BlockHeader>,
    original_tip: crate::state::NativeExecutionTip,
    kura: Arc<Kura>,
    archive: Option<NativeContextArchive>,
    read: Option<NativeContextRead>,
    budget: AllocationBudget,
    network: iroha_data_model::NetworkId,
    authority: Option<AuthorityRead>,
    verifier: NativeExecutionEvidenceVerifier,
    selected: LaneSelection,
    next: u64,
    current: Option<Arc<SignedBlock>>,
    genesis_bytes: Option<ChargedBuffer<u8>>,
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
        let original_tip = {
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
            tip
        };
        let height = original_tip.height();
        let carrier = original_tip.iroha_hash();
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
            original_tip,
            kura,
            archive: Some(archive),
            read: None,
            budget: state.ivm_execution_budget(),
            network: *state.network_id_ref(),
            authority: None,
            verifier,
            selected: LaneSelection::new(lane, incarnation),
            next: 1,
            current: None,
            genesis_bytes: None,
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
            if self.read.is_none() {
                self.read = Some(
                    self.archive
                        .take()
                        .expect("original archive owner")
                        .read_job(self.next, block.hash()),
                );
            }
            let bytes = loop {
                match self.read.as_mut().expect("original acquisition").poll() {
                    Ok(Some(bytes)) => break bytes,
                    Ok(None) => {}
                    Err(NativeContextArchiveError::Io(error))
                        if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(error) => return Err(archive_error(error)),
                }
            };
            self.archive = Some(
                self.read
                    .take()
                    .expect("completed acquisition")
                    .into_archive(),
            );
            let selected = &mut self.selected;
            let authority = &mut self.authority;
            let budget = &self.budget;
            let network = self.network;
            let genesis_bytes = &mut self.genesis_bytes;
            let receipt = self
                .verifier
                .push_shared_height_with_genesis(Arc::clone(block), bytes.as_slice(), |genesis| {
                    let bytes = genesis_bytes
                        .take()
                        .ok_or("original genesis archive owner is missing")?;
                    if selected
                        .observe(1, genesis.lanes())
                        .map_err(|error| error.to_string())?
                    {
                        *authority = Some(AuthorityRead::Payload(LanePayloadRead::from_verified(
                            bytes,
                            budget.clone(),
                            network,
                            &genesis,
                        )));
                    }
                    Ok(())
                })
                .map_err(invalid)?;
            if let Some(receipt) = receipt {
                if self.next == self.height && !receipt.matches_original_tip(self.original_tip) {
                    return Err(invalid(
                        "verified authority prefix differs from original native execution result",
                    ));
                }
                if selected.observe(receipt.block().header().height().get(), receipt.lanes())? {
                    *authority = Some(AuthorityRead::Payload(LanePayloadRead::from_verified(
                        bytes,
                        budget.clone(),
                        network,
                        &receipt,
                    )));
                }
            } else {
                *genesis_bytes = Some(bytes);
            }
            self.current = None;
            self.next = self
                .next
                .checked_add(1)
                .ok_or_else(|| invalid("native authority height exhausted"))?;
        }
        self.archive
            .as_ref()
            .expect("complete namespace")
            .recheck_namespace()
            .map_err(archive_error)?;
        if !self.selected.is_active(self.height) {
            self.authority = None;
            return Ok(());
        }
        loop {
            match self.authority.take().expect("selected original creation") {
                AuthorityRead::Payload(read) => match read.authenticate() {
                    Ok(source) => {
                        self.authority = Some(AuthorityRead::Config(LaneAuthorityRead::new(
                            source,
                            self.incarnation,
                        )))
                    }
                    Err((read, error)) => {
                        self.authority = Some(AuthorityRead::Payload(read));
                        return Err(payload_error(error));
                    }
                },
                AuthorityRead::Config(read) => match read.complete(&self.budget) {
                    Ok(owner) => self.authority = Some(AuthorityRead::Ready(owner)),
                    Err((read, error)) => {
                        self.authority = Some(AuthorityRead::Config(read));
                        return Err(payload_error(error));
                    }
                },
                ready @ AuthorityRead::Ready(_) => {
                    self.authority = Some(ready);
                    return self
                        .archive
                        .as_ref()
                        .expect("same completed namespace")
                        .recheck_namespace()
                        .map_err(archive_error);
                }
            }
        }
    }

    pub(super) fn finish(self) -> Option<LaneAuthority> {
        match self.authority {
            Some(AuthorityRead::Ready(owner)) => Some(owner),
            _ => None,
        }
    }
}

fn payload_error(error: LanePayloadError) -> io::Error {
    io::Error::new(
        if error.is_local_refusal() {
            io::ErrorKind::WouldBlock
        } else {
            io::ErrorKind::InvalidData
        },
        error,
    )
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
