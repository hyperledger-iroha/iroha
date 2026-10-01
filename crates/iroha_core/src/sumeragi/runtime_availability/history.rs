//! One retained authenticated archive scan, bounded to a captured original State publication.

use crate::sumeragi::certified_chain::PrefixArtifactsRead;

use super::{invalid, pending, selection::LaneSelection};
use crate::query::native_receipts::lane_payload::{
    LaneAuthority, LaneAuthorityRead, LanePayload, LanePayloadError, LanePayloadRead,
};
use crate::{
    kura::Kura,
    query::native_context_archive::{
        NativeContextArchive, NativeContextArchiveError, NativeContextRead,
    },
    state::{
        NativeExecutionEvidenceLimits, NativeExecutionEvidenceVerifier, State, StateReadOnly,
        StateView, WorldReadOnly,
    },
};
use iroha_allocation::{AllocationBudget, ChargedBuffer};
use iroha_crypto::HashOf;
use iroha_data_model::{
    block::{BlockHeader, SignedBlock},
    sumeragi_finality::MAX_FINALITY_BLOCK_BYTES,
};
use iroha_model_base::topology::LaneId;
use std::{io, num::NonZeroUsize, sync::Arc};

enum TipPayloadRead {
    Pending(LanePayloadRead),
    Ready(LanePayload),
}

enum AuthorityRead {
    Payload(LanePayloadRead),
    Config(LaneAuthorityRead),
    Ready(LaneAuthority),
}

/// A fixed original State cut captured without disk I/O or another State view. Its fields
/// are private and it has no decoder. Installation must still recheck the same generation.
pub(in crate::sumeragi) struct HistoryCapture {
    generation: u64,
    original_tip: crate::state::NativeExecutionTip,
    kura: Arc<Kura>,
    budget: AllocationBudget,
    network: iroha_data_model::NetworkId,
    chain_id: iroha_model_base::chain::ChainId,
    policy: Option<(u64, u64)>,
}
impl HistoryCapture {
    // The caller samples this generation before acquiring this exact original State view.
    pub(in crate::sumeragi) fn from_view(
        state: &State,
        view: &StateView<'_>,
        generation: u64,
    ) -> io::Result<Option<Self>> {
        if !crate::state::is_stable_state_view_generation(generation, state.state_view_generation())
        {
            return Err(pending(
                "native State publication changed before history capture",
            ));
        }
        let kura = state.kura_handle();
        let budget = state.ivm_execution_budget();
        if !std::ptr::eq(view.ivm, &state.ivm)
            || !view
                .pipeline_ivm_prepared_cache
                .execution_budget()
                .same_pool(&budget)
            || !std::ptr::eq(view.kura(), kura.as_ref())
            || view.network_id() != state.network_id_ref()
            || view.chain_id() != state.chain_id_ref()
        {
            return Err(invalid(
                "native history capture belongs to another original State",
            ));
        }
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
        if tip.height() < 2 {
            return Ok(None);
        }
        let policy = view.world().sumeragi_npos_parameters().map(|policy| {
            (
                policy.evidence_horizon_blocks(),
                policy.slashing_delay_blocks(),
            )
        });
        if !crate::state::is_stable_state_view_generation(generation, state.state_view_generation())
        {
            return Err(pending(
                "native State publication changed during history capture",
            ));
        }
        Ok(Some(Self {
            generation,
            original_tip: tip,
            kura,
            budget,
            network: *view.network_id(),
            chain_id: view.chain_id().clone(),
            policy,
        }))
    }
}

pub(in crate::sumeragi) struct HistoryScan {
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
    tip_payload: Option<TipPayloadRead>,
    instance: iroha_sumeragi::types::Hash32,
    policy: Option<(u64, u64)>,
    verifier: NativeExecutionEvidenceVerifier,
    selected: LaneSelection,
    next: u64,
    current: Option<Arc<SignedBlock>>,
    current_bytes: Option<ChargedBuffer<u8>>,
    artifacts: Option<PrefixArtifactsRead>,
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
        let capture = {
            let view = state.view();
            HistoryCapture::from_view(state, &view, generation)?
        };
        capture
            .map(|capture| Self::open_captured(capture, lane, incarnation))
            .transpose()
    }

    /// The original World guard is gone before opening or reading any lane/global artifact.
    pub(in crate::sumeragi) fn open_captured(
        capture: HistoryCapture,
        lane: LaneId,
        incarnation: [u8; 32],
    ) -> io::Result<Self> {
        let HistoryCapture {
            generation,
            original_tip,
            kura,
            budget,
            network,
            chain_id,
            policy,
        } = capture;
        let height = original_tip.height();
        let carrier = original_tip.iroha_hash();
        let archive = NativeContextArchive::open_existing(
            &kura,
            budget.clone(),
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
        let instance = crate::sumeragi::lanes::incarnation_instance(
            &crate::sumeragi::crypto::BlsCrypto::new(),
            &network,
            chain_id.as_str(),
            lane,
            &incarnation,
        );
        let verifier = NativeExecutionEvidenceVerifier::new(
            chain_id,
            network,
            NativeExecutionEvidenceLimits {
                max_carriers: height,
                max_carrier_bytes: block_bound,
                max_context_bytes: state_bound,
                max_retained_bytes: retained,
            },
        )
        .map_err(invalid)?;
        Ok(Self {
            lane,
            incarnation,
            generation,
            height,
            carrier,
            original_tip,
            kura,
            archive: Some(archive),
            read: None,
            budget,
            network,
            authority: None,
            tip_payload: None,
            instance,
            policy,
            verifier,
            selected: LaneSelection::new(lane, incarnation),
            next: 1,
            current: None,
            current_bytes: None,
            artifacts: None,
            genesis_bytes: None,
        })
    }

    pub(super) fn matches(&self, lane: LaneId, incarnation: &[u8; 32]) -> bool {
        self.lane == lane && &self.incarnation == incarnation
    }

    pub(in crate::sumeragi) fn generation(&self) -> u64 {
        self.generation
    }

    // Local archive admission refusal leaves the verified prefix, selected original record,
    // exact current carrier and retained directory descriptor untouched for the next call.
    pub(in crate::sumeragi) fn complete(&mut self) -> io::Result<()> {
        while self.next <= self.height {
            if self.current.is_none() {
                let index = usize::try_from(self.next)
                    .ok()
                    .and_then(NonZeroUsize::new)
                    .ok_or_else(|| invalid("native carrier height overflow"))?;
                let block = self.kura.get_block(index).ok_or_else(|| {
                    invalid(format!("native carrier {} is unreadable", self.next))
                })?;
                let length = norito::canonical_frame_len(block.as_ref())
                    .map_err(invalid)?
                    .checked_add(1)
                    .ok_or_else(|| invalid("native carrier byte length overflow"))?;
                if length > MAX_FINALITY_BLOCK_BYTES {
                    return Err(invalid(
                        "native carrier exceeds its independent reader bound",
                    ));
                }
                self.current = Some(block);
            }
            let block = self.current.as_ref().expect("retained original carrier");
            if self.next == self.height && block.hash() != self.carrier {
                return Err(invalid(
                    "native authority prefix differs from captured State tip",
                ));
            }
            if self.current_bytes.is_none() {
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
                self.current_bytes = Some(bytes);
            }
            // Large certificate fields and the exact proposal image acquire real owners
            // before verification can advance/poison its prefix. A refusal preserves both
            // archive bytes and partial artifact acquisition from this original carrier.
            let artifacts = if self.next > 1 {
                let read = self.artifacts.take().unwrap_or_else(|| {
                    PrefixArtifactsRead::new(Arc::clone(block), self.budget.clone())
                });
                match read.complete(&self.budget) {
                    Ok(artifacts) => Some(artifacts),
                    Err((read, error)) => {
                        self.artifacts = Some(read);
                        let kind = error.kind();
                        return Err(if kind == io::ErrorKind::WouldBlock {
                            // Reporting local allocation refusal must not allocate a box.
                            io::Error::from(kind)
                        } else {
                            io::Error::new(kind, error)
                        });
                    }
                }
            } else {
                None
            };
            let bytes = self.current_bytes.take().expect("original context bytes");
            let selected = &mut self.selected;
            let authority = &mut self.authority;
            let budget = &self.budget;
            let network = self.network;
            let genesis_bytes = &mut self.genesis_bytes;
            let accept_genesis = |genesis: crate::state::VerifiedNativeExecutionCarrier| {
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
            };
            let receipt = match artifacts {
                Some(artifacts) => self.verifier.push_prepared_height_with_genesis(
                    artifacts,
                    bytes.as_slice(),
                    accept_genesis,
                ),
                None => self.verifier.push_shared_height_with_genesis(
                    Arc::clone(block),
                    bytes.as_slice(),
                    accept_genesis,
                ),
            }
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
                } else if self.next == self.height {
                    self.tip_payload = Some(TipPayloadRead::Pending(
                        LanePayloadRead::from_verified(bytes, budget.clone(), network, &receipt),
                    ));
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
        if matches!(self.tip_payload, Some(TipPayloadRead::Pending(_))) {
            let Some(TipPayloadRead::Pending(read)) = self.tip_payload.take() else {
                unreachable!("checked pending original payload")
            };
            match read.authenticate() {
                Ok(payload) => self.tip_payload = Some(TipPayloadRead::Ready(payload)),
                Err((read, error)) => {
                    self.tip_payload = Some(TipPayloadRead::Pending(read));
                    return Err(payload_error(error));
                }
            }
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
                AuthorityRead::Ready(owner) => {
                    let validation = match &self.tip_payload {
                        Some(TipPayloadRead::Ready(payload)) => owner.validate_custody(
                            payload,
                            self.instance,
                            self.policy,
                            &self.budget,
                        ),
                        _ => Err(LanePayloadError::Source),
                    };
                    self.authority = Some(AuthorityRead::Ready(owner));
                    validation.map_err(payload_error)?;
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
    if error.is_local_refusal() {
        return io::ErrorKind::WouldBlock.into();
    }
    io::Error::new(io::ErrorKind::InvalidData, error)
}

fn archive_error(error: NativeContextArchiveError) -> io::Error {
    if error.is_local_refusal() {
        return io::ErrorKind::WouldBlock.into();
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
    fn archive_allocation_refusal_remains_retryable_without_diagnostic_allocation() {
        let budget = AllocationBudget::new(1);
        let held = ChargedBuffer::<u8>::new(1, &budget).unwrap();
        for requested in [1, 2] {
            let refused = ChargedBuffer::<u8>::new(requested, &budget).err().unwrap();
            let mapped = archive_error(NativeContextArchiveError::Allocation(refused));
            assert_eq!(mapped.kind(), io::ErrorKind::WouldBlock);
            assert!(mapped.get_ref().is_none());
            assert_eq!(budget.reserved_bytes(), 1);
        }
        let physical = archive_error(NativeContextArchiveError::Allocation(
            iroha_allocation::ChargedBufferError::Allocator { requested_bytes: 1 },
        ));
        assert_eq!(physical.kind(), io::ErrorKind::WouldBlock);
        assert!(physical.get_ref().is_none());
        drop(held);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn payload_refusal_avoids_diagnostic_allocation_but_source_errors_remain_typed() {
        let budget = AllocationBudget::new(1);
        let held = ChargedBuffer::<u8>::new(1, &budget).unwrap();
        for requested in [1, 2] {
            let refused = budget.try_reserve_bytes(requested).err().unwrap();
            let mapped = payload_error(LanePayloadError::Admission(refused));
            assert_eq!(mapped.kind(), io::ErrorKind::WouldBlock);
            assert!(mapped.get_ref().is_none());
            assert_eq!(budget.reserved_bytes(), 1);
        }
        let physical = payload_error(LanePayloadError::Materialization(
            iroha_allocation::PrepaidBufferError::Allocation(
                iroha_allocation::ChargedBufferError::Allocator { requested_bytes: 1 },
            ),
        ));
        assert_eq!(physical.kind(), io::ErrorKind::WouldBlock);
        assert!(physical.get_ref().is_none());
        for error in [LanePayloadError::Source, LanePayloadError::Commitment] {
            let mapped = payload_error(error);
            assert_eq!(mapped.kind(), io::ErrorKind::InvalidData);
            assert!(mapped.get_ref().unwrap().is::<LanePayloadError>());
        }
        drop(held);
        assert_eq!(budget.reserved_bytes(), 0);
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

#[cfg(test)]
mod artifact_tests {
    use super::*;

    #[test]
    fn certificate_refusal_keeps_completed_archive_and_prefix_until_original_retry() {
        let (chain, record, _epoch) = super::super::tests::fixed_lane_chain();
        let budget = chain.state().ivm_execution_budget();
        let before = budget.reserved_bytes();
        let limit = budget.limit_bytes();
        let mut scan = HistoryScan::open(chain.state(), record.lane, record.incarnation)
            .unwrap()
            .unwrap();
        // Complete the exact H1 trust-root phase. H1 still grants no execution authority;
        // H2 must authenticate it after acquiring its original bulk certificate artifacts.
        let genesis = chain
            .kura()
            .get_block(NonZeroUsize::new(1).unwrap())
            .unwrap();
        let archive = scan.archive.as_ref().unwrap();
        let genesis_bytes = archive.read_exact(1, genesis.hash()).unwrap();
        assert!(
            scan.verifier
                .push_shared_height(Arc::clone(&genesis), genesis_bytes.as_slice())
                .unwrap()
                .is_none()
        );
        scan.genesis_bytes = Some(genesis_bytes);
        scan.next = 2;
        let block = chain
            .kura()
            .get_block(NonZeroUsize::new(2).unwrap())
            .unwrap();
        scan.current_bytes = Some(archive.read_exact(2, block.hash()).unwrap());
        let bytes = scan.current_bytes.as_ref().unwrap().as_slice().as_ptr();
        scan.current = Some(Arc::clone(&block));
        // The fixture pin retains only retired State generations. The reader's table,
        // proposal and shared-control owners still refund immediately while it is live.
        let artifact_baseline = budget.reserved_bytes();
        let artifacts = PrefixArtifactsRead::new(Arc::clone(&block), budget.clone())
            .complete(&budget)
            .unwrap_or_else(|(_, error)| panic!("original artifacts: {error}"));
        assert!(budget.reserved_bytes() > artifact_baseline);
        drop(artifacts);
        assert_eq!(budget.reserved_bytes(), artifact_baseline);
        budget.set_limit_bytes(budget.reserved_bytes());
        let error = scan.complete().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert!(
            error.get_ref().is_none(),
            "local refusal has no diagnostic box"
        );
        assert_eq!(scan.next, 2);
        assert!(scan.artifacts.is_some());
        assert!(Arc::ptr_eq(scan.current.as_ref().unwrap(), &block));
        assert_eq!(
            scan.current_bytes.as_ref().unwrap().as_slice().as_ptr(),
            bytes
        );
        assert_eq!(
            scan.complete().unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert_eq!(
            scan.current_bytes.as_ref().unwrap().as_slice().as_ptr(),
            bytes
        );
        // A fresh name now denotes corrupt bytes. This pending acquisition already owns
        // complete original bytes and must never use the replacement as its retry source.
        let path = chain
            .kura()
            .store_root()
            .join("native-contexts")
            .join(format!(
                "{:020}-{}.nrt",
                2,
                hex::encode(block.hash().as_ref()),
            ));
        let held = path.with_extension("original-artifact-retry");
        std::fs::rename(&path, &held).unwrap();
        std::fs::write(&path, b"replacement must not become pending source").unwrap();
        budget.set_limit_bytes(limit);
        scan.complete().unwrap();
        assert!(scan.next > scan.height);
        let owner = scan.finish().expect("same original full prefix completes");
        assert!(owner.belongs_to(&budget));
        drop(owner);
        assert_eq!(budget.reserved_bytes(), before);
        let mut fresh = HistoryScan::open(chain.state(), record.lane, record.incarnation)
            .unwrap()
            .unwrap();
        assert!(
            fresh.complete().is_err(),
            "fresh acquisition rejects replaced source"
        );
        drop(fresh);
        assert_eq!(budget.reserved_bytes(), before);
        std::fs::remove_file(&path).unwrap();
        std::fs::rename(held, path).unwrap();
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;

    #[test]
    fn original_cut_capture_does_not_open_the_archive_and_never_upgrades_to_a_successor() {
        let (mut chain, record, _epoch) = super::super::tests::fixed_lane_chain();
        let state = Arc::clone(chain.state());
        let generation = state.state_view_generation();
        let view = state.view();
        let original = view.native_execution_tip().unwrap();
        let path = chain.kura().store_root().join("native-contexts");
        let held = path.with_extension("capture-held");
        std::fs::rename(&path, &held).unwrap();
        let captured = HistoryCapture::from_view(&state, &view, generation)
            .unwrap()
            .unwrap();
        assert_eq!(captured.original_tip, original);
        std::fs::rename(&held, &path).unwrap();
        drop(view);
        chain.commit(Vec::new());
        assert_ne!(state.state_view_generation(), generation);
        let budget = state.ivm_execution_budget();
        let baseline = budget.reserved_bytes();
        let mut read =
            HistoryScan::open_captured(captured, record.lane, record.incarnation).unwrap();
        read.complete().unwrap();
        assert_eq!(read.original_tip, original);
        assert_eq!(read.height, original.height());
        assert_eq!(read.generation(), generation);
        assert!(!crate::state::is_stable_state_view_generation(
            read.generation(),
            state.state_view_generation()
        ));
        let Some(TipPayloadRead::Ready(payload)) = &read.tip_payload else {
            panic!("exact original payload");
        };
        assert_eq!(payload.carrier().1, original.height());
        assert_eq!(payload.carrier().2, original.iroha_hash());
        drop(read);
        assert_eq!(budget.reserved_bytes(), baseline);
    }

    #[test]
    fn foreign_state_or_stale_generation_cannot_create_an_original_history_capture() {
        let (chain, _, _epoch) = super::super::tests::fixed_lane_chain();
        let (other, _, _other_epoch) = super::super::tests::fixed_lane_chain();
        let view = chain.state().view();
        let generation = chain.state().state_view_generation();
        for invalid in [generation | 1, generation.checked_add(2).unwrap()] {
            assert_eq!(
                HistoryCapture::from_view(chain.state(), &view, invalid)
                    .err()
                    .unwrap()
                    .kind(),
                io::ErrorKind::WouldBlock
            );
        }
        assert_eq!(
            HistoryCapture::from_view(other.state(), &view, other.state().state_view_generation())
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn original_capture_rejects_another_state_even_with_the_same_storage_and_identity() {
        use crate::{query::store::LiveQueryStore, state::World};
        let (chain, _, _epoch) = super::super::tests::fixed_lane_chain();
        let state = chain.state();
        let other = State::new_with_chain_and_network_id_for_testing(
            World::new(),
            Arc::clone(chain.kura()),
            LiveQueryStore::start_test(),
            state.chain_id_ref().clone(),
            chain.network_id(),
        );
        let view = state.view();
        assert!(std::ptr::eq(view.kura(), other.kura()));
        assert_eq!(view.network_id(), other.network_id_ref());
        assert_eq!(view.chain_id(), other.chain_id_ref());
        assert_eq!(
            HistoryCapture::from_view(&other, &view, other.state_view_generation())
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::InvalidData,
        );
        let mut view = state.view();
        view.pipeline_ivm_prepared_cache = other.view().pipeline_ivm_prepared_cache.clone();
        assert!(std::ptr::eq(view.ivm, &state.ivm));
        assert!(
            !view
                .pipeline_ivm_prepared_cache
                .execution_budget()
                .same_pool(&state.ivm_execution_budget())
        );
        assert_eq!(
            HistoryCapture::from_view(state, &view, state.state_view_generation())
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::InvalidData,
        );
    }
}
