//! Availability authority from the original native State and authenticated historical carriers.
//!
//! Stored lane/body artifacts never select their own epoch, committee, layout or instance.
//! Historical lane incarnations are recovered from the original context archive and complete
//! certified prefix, including after retirement removes them from the current World projection.

use parking_lot::Mutex;
use std::{io, sync::Arc};

use iroha_data_model::{
    NetworkId,
    sumeragi_finality::{ScheduledConfig, ScheduledSlot},
};
use iroha_model_base::topology::LaneId;
use iroha_sumeragi::types::{Hash32, HeightConfig};

use super::{
    attestation::NativePastaVerifier,
    availability_schedule::AvailabilitySchedule,
    certified_chain::{CertifiedChain, committed_block},
    crypto::BlsCrypto,
    lanes::{
        incarnation_instance, lane_height_config,
        registry::{LaneStoreAuthorities, LaneStoreAuthority},
    },
};
use crate::state::{State, StateReadOnly, WorldReadOnly};

mod history;
mod selection;
use history::HistoryScan;

fn invalid(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
fn pending(reason: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::WouldBlock, reason)
}

/// Resolve global authority under one immutable configured instance and original State owner.
pub struct NativeGlobalAvailability {
    state: Arc<State>,
    instance: Hash32,
    crypto: Arc<BlsCrypto>,
}
impl NativeGlobalAvailability {
    /// Bind the supplied instance to State's authenticated signed genesis and configured chain.
    /// Genesis must already be applied; its hash is the exact network identifier.
    ///
    /// # Errors
    /// Genesis cannot be authenticated, or the supplied instance does not name that genesis.
    pub fn new(state: Arc<State>, instance: Hash32, crypto: Arc<BlsCrypto>) -> io::Result<Self> {
        let expected = {
            let view = state.view();
            CertifiedChain::new(&view).map_err(invalid)?.instance()
        };
        if expected != instance {
            return Err(invalid("global availability instance differs from State"));
        }
        Ok(Self {
            state,
            instance,
            crypto,
        })
    }

    fn resolve(&self, height: u64) -> io::Result<Option<ScheduledConfig>> {
        if height <= 1 {
            return Ok(None);
        }
        let view = self.state.view();
        if view.kura().native_consensus_gate().is_closed() {
            return Err(io::Error::other(
                "native storage gate is closed; restart is required",
            ));
        }
        let tip = view
            .native_execution_tip()
            .ok_or_else(|| pending("original native execution is not published"))?;
        if tip.height() != view.height() as u64
            || Some(tip.iroha_hash()) != view.latest_block_hash()
        {
            return Err(invalid(
                "native execution tip differs from captured State publication",
            ));
        }
        if height > tip.height() {
            return match view.world().consensus_schedule().get(height) {
                Some(ScheduledSlot::Ready(config)) => Ok(Some(config.clone())),
                Some(ScheduledSlot::PendingBoundary { .. }) | None => Ok(None),
            };
        }
        // The original native tip in this same view authenticates execution ancestry. This
        // reader verifies each parent core hash, R, Iroha hash and executed-frame binding;
        // local CommitQC bytes are deliberately not the source of deterministic authority.
        let parent = committed_block(&view, height - 1).map_err(invalid)?;
        if parent
            .header()
            .is_some_and(|header| header.instance != self.instance)
        {
            return Err(invalid("historical global instance differs"));
        }
        // The requested body is never the source of its own parameters or authority.
        match &parent.commitment().schedule.next {
            ScheduledSlot::Ready(config) if config.height == height => Ok(Some(config.clone())),
            _ => Err(invalid(
                "certified parent has no exact next-height authority",
            )),
        }
    }
}
impl AvailabilitySchedule for NativeGlobalAvailability {
    fn instance(&self) -> Hash32 {
        self.instance
    }
    fn height_config(&self, height: u64) -> io::Result<Option<HeightConfig>> {
        let Some(scheduled) = self.resolve(height)? else {
            return Ok(None);
        };
        let config = scheduled.height_config().map_err(invalid)?;
        self.crypto
            .admit_committee(scheduled.epoch.committee.iter().map(|member| {
                (
                    member.validator.public_key(),
                    member.proof_of_possession.as_slice(),
                )
            }))
            .map_err(|(index, error)| {
                invalid(format!("historical global member {index}: {error}"))
            })?;
        Ok(Some(config))
    }
}

/// Lane authority pinned to the native State's network, chain and actual BLS admission owner.
pub struct NativeLaneStoreAuthorities {
    state: Arc<State>,
    crypto: Arc<BlsCrypto>,
    scan: Mutex<Option<HistoryScan>>,
}
impl NativeLaneStoreAuthorities {
    /// Use State's original archive pool and the same BLS owner as the lane stores.
    #[must_use]
    pub fn new(state: Arc<State>, crypto: Arc<BlsCrypto>) -> Self {
        Self {
            state,
            crypto,
            scan: Mutex::new(None),
        }
    }

    fn historical_record(
        &self,
        lane: LaneId,
        incarnation: [u8; 32],
    ) -> io::Result<Option<iroha_data_model::sumeragi_lanes::SumeragiLaneRecord>> {
        let mut pending_scan = self
            .scan
            .try_lock()
            .ok_or_else(|| pending("native authority scan is busy"))?;
        if let Some(scan) = pending_scan.as_ref() {
            if !scan.matches(lane, &incarnation) {
                return Err(pending("another original native authority scan is pending"));
            }
        } else {
            *pending_scan = HistoryScan::open(&self.state, lane, incarnation)?;
        }
        let Some(scan) = pending_scan.as_mut() else {
            return Ok(None);
        };
        if let Err(error) = scan.complete() {
            if error.kind() != io::ErrorKind::WouldBlock {
                // A terminally rejected prefix grants no authority. Do not leave its poisoned
                // verifier occupying the sole retry slot for unrelated incarnation requests.
                *pending_scan = None;
            }
            return Err(error);
        }
        if !crate::state::is_stable_state_view_generation(
            scan.generation(),
            self.state.state_view_generation(),
        ) {
            // This completed prefix belongs to a different publication cut. A new lookup
            // starts against the new original cut; no authority has escaped this attempt.
            *pending_scan = None;
            return Err(pending(
                "native State publication changed during authority lookup",
            ));
        }
        Ok(pending_scan
            .take()
            .expect("completed original scan")
            .finish())
    }
}

struct PinnedLaneAvailability {
    instance: Hash32,
    config: HeightConfig,
}
impl AvailabilitySchedule for PinnedLaneAvailability {
    fn instance(&self) -> Hash32 {
        self.instance
    }
    fn height_config(&self, height: u64) -> io::Result<Option<HeightConfig>> {
        Ok(self
            .config
            .epoch
            .contains(height)
            .then(|| self.config.clone()))
    }
}
impl LaneStoreAuthorities for NativeLaneStoreAuthorities {
    fn authority(
        &self,
        lane: LaneId,
        incarnation: &[u8; 32],
        instance: Hash32,
    ) -> io::Result<Option<LaneStoreAuthority>> {
        let network: NetworkId = *self.state.network_id_ref();
        let expected = incarnation_instance(
            &*self.crypto,
            &network,
            &self.state.chain_id_ref().to_string(),
            lane,
            incarnation,
        );
        if expected != instance {
            return Err(invalid(
                "requested lane instance differs from native network and incarnation",
            ));
        }
        let Some(record) = self.historical_record(lane, *incarnation)? else {
            return Ok(None);
        };
        let config = lane_height_config(&record).map_err(invalid)?;
        self.crypto
            .admit_committee(
                record
                    .committee
                    .iter()
                    .map(|member| (member.peer.public_key(), member.pop.as_slice())),
            )
            .map_err(|(index, error)| {
                invalid(format!("historical lane member {index}: {error}"))
            })?;
        Ok(Some(LaneStoreAuthority {
            schedule: Arc::new(PinnedLaneAvailability { instance, config }),
            verifier: Arc::new(NativePastaVerifier::new(instance, network)),
        }))
    }
}

#[cfg(test)]
#[path = "runtime_availability/tests.rs"]
mod tests;
