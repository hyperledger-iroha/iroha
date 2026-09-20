//! Exact native output custody between the process owner and reliable P2P.
//!
//! This is a bounded physical fanout, not a reducer or an acknowledgement of
//! economic Apply. A blocked destination retains its original post and actor
//! ticket while other destinations and other instances continue to progress.
//! Native framing is registered, but live ingress stays closed. TODO: connect
//! the sole process-lifetime consumer and candidate handoff, retiring the legacy
//! fresh lane signer at that same connected activation.

use std::{
    collections::{BTreeSet, VecDeque},
    num::NonZeroUsize,
    sync::Arc,
};

use iroha_crypto::Hash;
use iroha_data_model::block::{consensus_v2::HeightContextId, lane_consensus::LaneDecisionV1};
use iroha_model_base::peer::PeerId;
use iroha_p2p::{
    Priority,
    network::{NetworkActorAdmissionError, NetworkActorAdmissionTicket, message::Post},
};

use super::{
    message::{BlockMessage, BlockMessageWire},
    output_guard::ConsensusOutputGuard,
    v2::VerifiedHeightContext,
    v2_lane_instance::LaneOutbound,
    v2_lane_wire::LaneAuthenticator,
};
use crate::{
    NetworkMessage,
    state::{State, VerifiedLaneContexts},
};

/// Admission returns original custody on both pressure and invalid ownership.
#[must_use]
pub(crate) enum NativeTransportAdmission {
    Retained,
    Retry(LaneOutbound),
    Rejected {
        packet: LaneOutbound,
        reason: String,
    },
}

/// A proof relay never consumes the original reducer Decision or Apply obligation.
#[must_use]
pub(crate) enum NativeDecisionTransportAdmission {
    Retained,
    Retry(LaneDecisionV1),
    Rejected {
        decision: LaneDecisionV1,
        reason: String,
    },
}

/// One bounded service action. Retirement requires a complete authenticated
/// current-set observation; it never settles the native Decision/Apply owner.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum NativeTransportProgress {
    Idle,
    ObservationChanged,
    AwaitingGlobalRouting {
        instance: HeightContextId,
    },
    Admitted {
        instance: HeightContextId,
        peer: PeerId,
    },
    Backpressured {
        instance: HeightContextId,
        peer: PeerId,
        rank: usize,
    },
    Retired {
        instance: HeightContextId,
        unfinished_destinations: usize,
    },
}

struct Destination {
    peer: PeerId,
    returned: Option<Post<NetworkMessage>>,
    ticket: Option<NetworkActorAdmissionTicket>,
}
enum FanoutScope {
    Control,
    Decision {
        subject: Hash,
        context: HeightContextId,
        completed: BTreeSet<PeerId>,
    },
}
struct Fanout {
    scope: FanoutScope,
    instance: HeightContextId,
    message: Arc<BlockMessageWire>,
    destinations: VecDeque<Destination>,
}

/// A fixed number of original packets; each has at most one frozen committee's
/// destinations. Full admission leaves the original packet with the caller.
/// Neither global height rollover nor actor backpressure resets this owner.
pub(crate) struct NativeLaneTransport {
    state: Arc<State>,
    guard: Arc<ConsensusOutputGuard>,
    local_peer: PeerId,
    capacity: NonZeroUsize,
    fanouts: VecDeque<Fanout>,
}
impl Drop for NativeLaneTransport {
    fn drop(&mut self) {
        if !self.fanouts.is_empty() {
            self.guard.close_admission_for_restart();
        }
    }
}
impl NativeLaneTransport {
    pub(crate) fn new(
        state: Arc<State>,
        guard: Arc<ConsensusOutputGuard>,
        local_peer: PeerId,
        capacity: NonZeroUsize,
    ) -> Self {
        Self {
            state,
            guard,
            local_peer,
            capacity,
            fanouts: VecDeque::new(),
        }
    }

    /// Transfer an actual native outbox packet only after the exact current
    /// instance, signatures, original bytes and frozen destinations rejoin.
    /// No writer, timer, or reducer state is acquired here.
    pub(crate) fn retain(
        &mut self,
        observed: &VerifiedLaneContexts,
        packet: LaneOutbound,
    ) -> NativeTransportAdmission {
        if self.fanouts.len() == self.capacity.get() || !observed.is_current(&self.state) {
            return NativeTransportAdmission::Retry(packet);
        }
        let identity = super::v2_lane_driver::message_instance(&packet.envelope.message);
        let Some(lane) = observed
            .contexts()
            .iter()
            .find(|lane| Hash::from(lane.instance_id().0) == identity)
        else {
            return NativeTransportAdmission::Rejected {
                packet,
                reason: "native outbox instance is not current".into(),
            };
        };
        let id = lane.instance_id();
        let checked = (|| {
            if packet.envelope.version
                != iroha_data_model::block::lane_consensus::LANE_MESSAGE_VERSION_V1
            {
                return Err("native outbox changed its envelope revision".to_owned());
            }
            if packet.destinations != lane.frozen().committee {
                return Err("native outbox changed its frozen destinations".to_owned());
            }
            let encoded =
                norito::encode_canonical(&packet.envelope).map_err(|error| error.to_string())?;
            if encoded != packet.canonical_bytes {
                return Err("native outbox changed its original canonical bytes".to_owned());
            }
            packet
                .envelope
                .message
                .validate_shape(lane.frozen().committee.len())
                .map_err(|error| error.to_string())?;
            LaneAuthenticator::new(lane)
                .event(
                    &packet.envelope.message,
                    super::v2_core::EventTag::new(
                        lane.reducer_context().height(),
                        0,
                        super::v2_core::Generation::INITIAL,
                    ),
                )
                .map_err(|error| error.to_string())?;
            BlockMessageWire::try_preencoded(Arc::new(BlockMessage::NativeLane(
                packet.envelope.clone(),
            )))
            .map(Arc::new)
            .map_err(|error| error.to_string())
        })();
        let message = match checked {
            Ok(message) => message,
            Err(reason) => return NativeTransportAdmission::Rejected { packet, reason },
        };
        let _lease = self.state.consensus_publication_lease();
        if !observed.is_current(&self.state) {
            return NativeTransportAdmission::Retry(packet);
        }
        let destinations = packet
            .destinations
            .into_iter()
            .filter(|peer| peer != &self.local_peer)
            .map(|peer| Destination {
                peer,
                returned: None,
                ticket: None,
            })
            .collect::<VecDeque<_>>();
        if !destinations.is_empty() {
            self.fanouts.push_back(Fanout {
                scope: FanoutScope::Control,
                instance: id,
                message,
                destinations,
            });
        }
        NativeTransportAdmission::Retained
    }

    /// Relay a complete authenticated native Decision to the current global
    /// committee, including global proposers outside the frozen lane committee.
    /// The caller continues to retain the original reducer Decision/Apply owner.
    pub(crate) fn retain_decision(
        &mut self,
        observed: &VerifiedLaneContexts,
        global: &VerifiedHeightContext,
        decision: LaneDecisionV1,
    ) -> NativeDecisionTransportAdmission {
        if !observed.is_current(&self.state) || !self.global_routing_is_current(observed, global) {
            return NativeDecisionTransportAdmission::Retry(decision);
        }
        let Some(lane) = observed
            .contexts()
            .iter()
            .find(|lane| Hash::from(lane.instance_id().0) == decision.value().instance_id)
        else {
            return NativeDecisionTransportAdmission::Rejected {
                decision,
                reason: "native Decision instance is not current".into(),
            };
        };
        let id = lane.instance_id();
        let checked = (|| {
            LaneAuthenticator::new(lane)
                .decision_certificate(&decision)
                .map_err(|error| error.to_string())?;
            let subject = decision
                .value()
                .subject_hash()
                .map_err(|error| error.to_string())?;
            let wire = BlockMessageWire::try_preencoded(Arc::new(
                BlockMessage::NativeLaneDecision(Box::new(decision.clone())),
            ))
            .map_err(|error| error.to_string())?;
            Ok::<_, String>((subject, Arc::new(wire)))
        })();
        let (subject, message) = match checked {
            Ok(exact) => exact,
            Err(reason) => return NativeDecisionTransportAdmission::Rejected { decision, reason },
        };
        let _lease = self.state.consensus_publication_lease();
        if !observed.is_current(&self.state) || !self.global_routing_is_current(observed, global) {
            return NativeDecisionTransportAdmission::Retry(decision);
        }
        for retained in &self.fanouts {
            if retained.instance == id
                && let FanoutScope::Decision {
                    subject: previous, ..
                } = &retained.scope
            {
                return if *previous == subject {
                    NativeDecisionTransportAdmission::Retained
                } else {
                    NativeDecisionTransportAdmission::Rejected {
                        decision,
                        reason: "native relay already retains a different decided value".into(),
                    }
                };
            }
        }
        if self.fanouts.len() == self.capacity.get() {
            return NativeDecisionTransportAdmission::Retry(decision);
        }
        let destinations = global
            .context()
            .roster
            .iter()
            .map(|entry| &entry.validator)
            .filter(|peer| *peer != &self.local_peer)
            .cloned()
            .map(|peer| Destination {
                peer,
                returned: None,
                ticket: None,
            })
            .collect::<VecDeque<_>>();
        if !destinations.is_empty() {
            self.fanouts.push_back(Fanout {
                scope: FanoutScope::Decision {
                    subject,
                    context: global.context().id(),
                    completed: BTreeSet::new(),
                },
                instance: id,
                message,
                destinations,
            });
        }
        NativeDecisionTransportAdmission::Retained
    }

    fn global_routing_is_current(
        &self,
        observed: &VerifiedLaneContexts,
        global: &VerifiedHeightContext,
    ) -> bool {
        let context = global.context();
        let parent = context
            .parent_commit_qc
            .as_ref()
            .map(|qc| qc.subject.block_hash)
            .or_else(|| {
                context
                    .snapshot_bootstrap
                    .map(|anchor| anchor.snapshot_block_hash)
            });
        context.network_id == *self.state.network_id_ref()
            && observed.carrier_height().checked_add(1) == Some(context.height)
            && parent == self.state.latest_block_hash_fast()
    }

    /// Use the real recoverable actor API. One blocked peer cannot monopolize
    /// either its fanout or the native process's next control turn.
    pub(crate) fn poll(
        &mut self,
        observed: &VerifiedLaneContexts,
        global: Option<&VerifiedHeightContext>,
        network: &crate::IrohaNetwork,
    ) -> Result<NativeTransportProgress, String> {
        self.poll_with(observed, global, |post, ticket| {
            network.post_recoverable(post, ticket)
        })
    }

    #[cfg(test)]
    pub(crate) fn poll_for_test(
        &mut self,
        observed: &VerifiedLaneContexts,
        post: impl FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
        ) -> Result<(), NetworkActorAdmissionError<Post<NetworkMessage>>>,
    ) -> Result<NativeTransportProgress, String> {
        self.poll_with(observed, None, post)
    }

    #[cfg(test)]
    pub(crate) fn poll_with_global_for_test(
        &mut self,
        observed: &VerifiedLaneContexts,
        global: &VerifiedHeightContext,
        post: impl FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
        ) -> Result<(), NetworkActorAdmissionError<Post<NetworkMessage>>>,
    ) -> Result<NativeTransportProgress, String> {
        self.poll_with(observed, Some(global), post)
    }

    fn poll_with(
        &mut self,
        observed: &VerifiedLaneContexts,
        global: Option<&VerifiedHeightContext>,
        mut post: impl FnMut(
            Post<NetworkMessage>,
            Option<NetworkActorAdmissionTicket>,
        ) -> Result<(), NetworkActorAdmissionError<Post<NetworkMessage>>>,
    ) -> Result<NativeTransportProgress, String> {
        let _lease = self.state.consensus_publication_lease();
        if !observed.is_current(&self.state) {
            return Ok(NativeTransportProgress::ObservationChanged);
        }
        let Some(mut fanout) = self.fanouts.pop_front() else {
            return Ok(NativeTransportProgress::Idle);
        };
        if !observed
            .contexts()
            .iter()
            .any(|lane| lane.instance_id() == fanout.instance)
        {
            return Ok(NativeTransportProgress::Retired {
                instance: fanout.instance,
                unfinished_destinations: fanout.destinations.len(),
            });
        }
        if let FanoutScope::Decision {
            context, completed, ..
        } = &mut fanout.scope
        {
            let Some(global) =
                global.filter(|global| self.global_routing_is_current(observed, global))
            else {
                let instance = fanout.instance;
                self.fanouts.push_back(fanout);
                return Ok(NativeTransportProgress::AwaitingGlobalRouting { instance });
            };
            let current = global.context();
            if *context != current.id() {
                // Rebind destinations only after authenticating the actual successor
                // context. Intersecting peers keep their original Post and ticket;
                // removed recipients release only transport custody, never Apply.
                let peers = current
                    .roster
                    .iter()
                    .map(|entry| entry.validator.clone())
                    .collect::<BTreeSet<_>>();
                fanout
                    .destinations
                    .retain(|destination| peers.contains(&destination.peer));
                completed.retain(|peer| peers.contains(peer));
                for peer in peers {
                    if peer != self.local_peer
                        && !completed.contains(&peer)
                        && !fanout
                            .destinations
                            .iter()
                            .any(|destination| destination.peer == peer)
                    {
                        fanout.destinations.push_back(Destination {
                            peer,
                            returned: None,
                            ticket: None,
                        });
                    }
                }
                *context = current.id();
            }
            if fanout.destinations.is_empty() {
                return Ok(NativeTransportProgress::Idle);
            }
        }
        // The guard is acquired before moving any retained post out. Every
        // ordinary pressure/failure path restores custody before returning.
        let guard = Arc::clone(&self.guard);
        let Some(operation) = guard.begin_fail_stop_operation() else {
            self.fanouts.push_front(fanout);
            return Err("native transport output requires restart".into());
        };
        let Some(mut destination) = fanout.destinations.pop_front() else {
            self.fanouts.push_front(fanout);
            return Err("native fanout lost its unfinished destination".into());
        };
        let original = destination.returned.take().unwrap_or_else(|| Post {
            data: NetworkMessage::SumeragiBlock(Arc::clone(&fanout.message)),
            peer_id: destination.peer.clone(),
            priority: Priority::High,
        });
        let progress = match post(original, destination.ticket.take()) {
            Ok(()) => {
                if let FanoutScope::Decision { completed, .. } = &mut fanout.scope {
                    completed.insert(destination.peer.clone());
                }
                Ok(NativeTransportProgress::Admitted {
                    instance: fanout.instance,
                    peer: destination.peer,
                })
            }
            Err(error) => {
                let (returned, ticket, outcome) = match error {
                    NetworkActorAdmissionError::Backpressured {
                        message,
                        ticket,
                        rank,
                    } => (
                        message,
                        ticket,
                        Ok(NativeTransportProgress::Backpressured {
                            instance: fanout.instance,
                            peer: destination.peer.clone(),
                            rank,
                        }),
                    ),
                    NetworkActorAdmissionError::Closed { message } => {
                        (message, None, Err("native output actor closed".to_owned()))
                    }
                    NetworkActorAdmissionError::Rejected { message, reason } => (
                        message,
                        None,
                        Err(format!(
                            "native output actor rejected exact frame: {reason:?}"
                        )),
                    ),
                };
                let exact = returned.peer_id == destination.peer
                    && returned.priority == Priority::High
                    && matches!(&returned.data, NetworkMessage::SumeragiBlock(message) if Arc::ptr_eq(message, &fanout.message));
                destination.returned = Some(returned);
                destination.ticket = ticket;
                fanout.destinations.push_back(destination);
                if exact {
                    outcome
                } else {
                    Err("native output actor substituted its returned occurrence".into())
                }
            }
        };
        if !fanout.destinations.is_empty() {
            self.fanouts.push_back(fanout);
        }
        if progress.is_ok() {
            operation.complete();
        }
        progress
    }
}
