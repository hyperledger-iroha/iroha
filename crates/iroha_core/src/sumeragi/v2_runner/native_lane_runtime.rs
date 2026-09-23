//! One process-lived native reducer, physical workers and transport owner.
//!
//! Global-height replacement must borrow this owner rather than reconstruct it.
//! It retains an original refused outbox packet and continues clocks and physical
//! completions while actor admission is blocked. Only genuine publication can
//! settle the original Apply. This is the executable runner seam, not activation.
//! TODO: construct once in `run_inner` only when the complete reserved validator
//! retains original execution through Validate/Apply, and retire the old fresh
//! lane signer and scheduler in that same change. No parallel engine is enabled.

use std::{collections::BTreeMap, num::NonZeroUsize, sync::Arc, time::Instant};

use iroha_crypto::{Hash, KeyPair};
use iroha_data_model::{NetworkId, block::consensus_v2::HeightContextId};
use iroha_model_base::peer::PeerId;
use iroha_p2p::network::{NetworkActorAdmissionError, NetworkActorAdmissionTicket, message::Post};

#[cfg(test)]
use crate::sumeragi::v2_lane_driver::{NativeLaneAdmission, NativeLaneInput};
use crate::{
    NetworkMessage,
    state::{PublishedNativeApply, State, VerifiedLaneContexts},
    sumeragi::{
        FairV2Ingress,
        output_guard::ConsensusOutputGuard,
        v2::VerifiedHeightContext,
        v2_lane_driver::{
            NativeLaneDecisionHandoff, NativeLaneDriver, NativeLaneDriverLimits, message_instance,
        },
        v2_lane_instance::{
            LaneApplySettlement, LaneClosedInstance, LaneOutbound, LanePhysicalShutdown,
            LaneProcessOwner, NativeLaneRetirementReceipt,
        },
        v2_lane_transport::{
            NativeLaneTransport, NativeTransportAdmission, NativeTransportProgress,
        },
    },
};

use super::{
    lane_engine_owner::LaneEngineLease,
    native_ingress_carrier::{NativeFairIngressPump, NativeFairIngressServiceOutcome},
};

type Result<T> = std::result::Result<T, String>;

// A packet removed from the driver's bounded channel needs its own drop guard:
// neither dropping the runtime nor failed actor admission acknowledges delivery.
struct PendingOutput {
    packet: Option<LaneOutbound>,
    guard: Arc<ConsensusOutputGuard>,
}
impl Drop for PendingOutput {
    fn drop(&mut self) {
        if self.packet.is_some() {
            self.guard.close_admission_for_restart();
        }
    }
}

/// The sole native process owner, independent of the enclosing global view.
pub(crate) struct NativeLaneRuntime {
    _lane_engine_lease: LaneEngineLease,
    fair_ingress: NativeFairIngressPump,
    pending: PendingOutput,
    transport: NativeLaneTransport,
    driver: NativeLaneDriver,
    state: Arc<State>,
    // Refused terminal retirement remains in this sole process owner; a
    // closed instance is never dropped merely because Kura/Queue is busy.
    pending_retirement: BTreeMap<HeightContextId, LaneClosedInstance>,
}

impl NativeLaneRuntime {
    /// Open no instance synchronously and start only the existing bounded workers.
    pub(crate) fn new_for_runner(
        network_id: NetworkId,
        state: Arc<State>,
        guard: Arc<ConsensusOutputGuard>,
        key: KeyPair,
        lane_engine_lease: LaneEngineLease,
        limits: NativeLaneDriverLimits,
        transport_capacity: NonZeroUsize,
    ) -> Result<Self> {
        let local_peer = PeerId::new(key.public_key().clone());
        if !lane_engine_lease.matches_native(network_id, &local_peer) {
            return Err(
                "Native lane signer does not hold this network and peer's exclusive engine lease"
                    .to_owned(),
            );
        }
        let transport = NativeLaneTransport::new(
            Arc::clone(&state),
            Arc::clone(&guard),
            local_peer,
            transport_capacity,
        );
        let driver = NativeLaneDriver::new(Arc::clone(&state), Arc::clone(&guard), key, limits)?;
        Ok(Self {
            _lane_engine_lease: lane_engine_lease,
            fair_ingress: NativeFairIngressPump::new(Arc::clone(&guard)),
            pending: PendingOutput {
                packet: None,
                guard,
            },
            transport,
            driver,
            state,
            pending_retirement: BTreeMap::new(),
        })
    }

    /// Exercise the same exclusive constructor with an isolated test worker.
    #[cfg(test)]
    pub(crate) fn new(
        state: Arc<State>,
        guard: Arc<ConsensusOutputGuard>,
        key: KeyPair,
        limits: NativeLaneDriverLimits,
        transport_capacity: NonZeroUsize,
    ) -> Result<Self> {
        let network_id = *state.network_id_ref();
        let owner = super::lane_engine_owner::LaneEngineOwner::new(
            network_id,
            PeerId::new(key.public_key().clone()),
        );
        let lease = owner.claim_native()?;
        Self::new_for_runner(
            network_id,
            state,
            guard,
            key,
            lease,
            limits,
            transport_capacity,
        )
    }

    /// Return the original full-queue ingress to its caller; never create a retry copy.
    #[cfg(test)]
    pub(crate) fn admit(
        &mut self,
        observed: &VerifiedLaneContexts,
        input: NativeLaneInput,
    ) -> NativeLaneAdmission {
        self.driver.admit(observed, input)
    }

    /// Service one checked Native fair occurrence through this process owner.
    /// A refused driver transfer remains in its one physical retry slot.
    pub(crate) fn service_checked_fair_ingress(
        &mut self,
        ingress: &FairV2Ingress,
        observed: &VerifiedLaneContexts,
    ) -> Result<NativeFairIngressServiceOutcome> {
        let driver = &mut self.driver;
        self.fair_ingress
            .service_with(ingress, |input| driver.admit(observed, input))
    }

    /// Service clocks/work before one actor attempt and one original output handoff.
    /// Network backpressure cannot prevent the next reducer/worker service turn.
    pub(crate) fn poll(
        &mut self,
        observed: &VerifiedLaneContexts,
        global: Option<&VerifiedHeightContext>,
        now: Instant,
        network: &crate::IrohaNetwork,
    ) -> Result<NativeTransportProgress> {
        self.poll_with(observed, global, now, |post, ticket| {
            network.post_recoverable(post, ticket)
        })
    }

    fn poll_with(
        &mut self,
        observed: &VerifiedLaneContexts,
        global: Option<&VerifiedHeightContext>,
        now: Instant,
        post: impl FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
        )
            -> std::result::Result<(), NetworkActorAdmissionError<Post<NetworkMessage>>>,
    ) -> Result<NativeTransportProgress> {
        self.driver.poll(observed, now)?;
        let progress = self.transport.poll_with(observed, global, post)?;
        if self.pending.packet.is_none() {
            self.pending.packet = self.driver.take_outbound()?;
        }
        {
            let _lease = self.state.consensus_publication_lease();
            if !observed.is_current(&self.state) {
                return Ok(progress);
            }
            if self.pending.packet.as_ref().is_some_and(|packet| {
                let id = message_instance(&packet.envelope.message);
                !observed
                    .contexts()
                    .iter()
                    .any(|lane| Hash::from(lane.instance_id().0) == id)
            }) {
                // Complete authenticated absence retires only this transport copy.
                // The original reducer Decision/Apply stays inside the driver.
                self.pending.packet = None;
            }
        }
        if self.pending.packet.is_some() {
            let guard = Arc::clone(&self.pending.guard);
            let Some(operation) = guard.begin_fail_stop_operation() else {
                return Err("native runner output requires restart".into());
            };
            let packet = self
                .pending
                .packet
                .take()
                .expect("retained original output");
            match self.transport.retain(observed, packet) {
                NativeTransportAdmission::Retained => {}
                NativeTransportAdmission::Retry(packet) => self.pending.packet = Some(packet),
                NativeTransportAdmission::Rejected { packet, reason } => {
                    self.pending.packet = Some(packet);
                    self.pending.guard.close_admission_for_restart();
                    return Err(reason);
                }
            }
            operation.complete();
        }
        Ok(progress)
    }

    /// Borrow exact local evidence without moving or acknowledging Apply ownership.
    pub(crate) fn capture_decisions(
        &self,
        observed: &VerifiedLaneContexts,
    ) -> Result<Option<NativeLaneDecisionHandoff>> {
        self.driver.capture_decisions(observed)
    }

    /// Only original globally published execution can consume a local Apply effect.
    pub(crate) fn settle_published_apply(
        &mut self,
        id: HeightContextId,
        published: &PublishedNativeApply<'_>,
    ) -> Result<Option<LaneApplySettlement>> {
        self.driver.settle_published_apply(id, published)
    }

    /// Consume only the exact original closed owner under a published proof.
    /// Physical drain may still be pending; any refusal returns the same closed
    /// instance to this runtime for a later retry.
    pub(crate) fn retire_published<'proof, 'carrier>(
        &mut self,
        id: HeightContextId,
        published: &'proof PublishedNativeApply<'carrier>,
    ) -> Result<Option<NativeLaneRetirementReceipt<'proof, 'carrier>>> {
        let Some(closed) = self
            .pending_retirement
            .remove(&id)
            .or_else(|| self.driver.take_closed(id))
        else {
            return Ok(None);
        };
        match closed.retire_published(published) {
            Ok(receipt) => Ok(Some(receipt)),
            Err((closed, error)) => {
                assert!(
                    self.pending_retirement.insert(id, closed).is_none(),
                    "one original closed instance owns each Native retirement slot"
                );
                Err(error.to_string())
            }
        }
    }

    /// Native deadlines have no global-height or view input.
    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        self.driver.next_deadline()
    }

    /// Inspect the sole local owner without exporting another scheduler or signer.
    pub(crate) fn process(&self) -> &LaneProcessOwner {
        self.driver.process()
    }

    /// Transfer an original closed owner only to its explicit publication consumer.
    pub(crate) fn take_closed(
        &mut self,
        id: HeightContextId,
    ) -> Option<crate::sumeragi::v2_lane_instance::LaneClosedInstance> {
        self.pending_retirement
            .remove(&id)
            .or_else(|| self.driver.take_closed(id))
    }

    /// Close physical admission; retain join custody for the blocking shutdown owner.
    pub(crate) fn shutdown(self) -> LanePhysicalShutdown {
        self.driver.shutdown()
    }

    /// Exercise the same actor custody boundary with deterministic delivery in tests.
    #[cfg(test)]
    pub(crate) fn poll_for_test(
        &mut self,
        observed: &VerifiedLaneContexts,
        now: Instant,
        post: impl FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
        )
            -> std::result::Result<(), NetworkActorAdmissionError<Post<NetworkMessage>>>,
    ) -> Result<NativeTransportProgress> {
        self.poll_with(observed, None, now, post)
    }

    /// Non-owning inspection verifies that output pressure retains the same allocation.
    #[cfg(test)]
    pub(crate) fn pending_output_for_test(&self) -> Option<&LaneOutbound> {
        self.pending.packet.as_ref()
    }
}
