use super::*;
/// Permanent reason that a recoverable actor-admission request was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkActorAdmissionRejection {
    /// The payload is not allowed to cross the live outbound boundary.
    OutboundDisallowed,
    /// The topic is not part of the reliable semantic-progress corridor.
    NotReliableProgress,
    /// A reliable-progress route was submitted through the best-effort corridor.
    ReliableProgressRequiresRecoverable,
    /// No durable retransmission source covers delivery gaps.
    MissingReconstruction,
    /// The exact encoded frame length could not be represented.
    WireLength,
    /// The exact encoded frame exceeds its configured topic cap.
    FrameTooLarge,
    /// The encrypted stream charge could not be represented.
    StreamChargeOverflow,
    /// The supplied ticket belongs to another request or actor handle.
    InvalidTicket,
    /// The authenticated reply connection tenure has been retired.
    InactiveReplyRoute,
}
/// Recoverable result of best-effort post admission into the network actor.
///
/// Unlike [`NetworkBaseHandle::post`], every failure returns the exact caller
/// payload. `Ok(())` means that the bounded actor queue (or its bounded
/// high-priority deferral owner) accepted ownership of the message.
#[derive(Debug)]
pub enum NetworkPostAdmissionError<M> {
    /// Actor byte or item capacity is temporarily unavailable.
    Backpressured {
        /// Original message, including its caller-supplied priority.
        message: M,
    },
    /// The network actor has terminated.
    Closed {
        /// Original message, including its caller-supplied priority.
        message: M,
    },
    /// The request is permanently invalid for this admission API.
    Rejected {
        /// Original message, including its caller-supplied priority.
        message: M,
        /// Stable reason for rejection.
        reason: NetworkActorAdmissionRejection,
    },
}
impl<T: Pload + message::ClassifyTopic + Sync, E: Enc + Sync> NetworkBaseHandle<T, E> {
    /// Construct a handle whose ordinary actor-byte corridor is saturated.
    ///
    /// This deterministic fixture lets downstream protocol tests prove that a recoverable post
    /// observes backpressure without binding sockets or retaining a live actor receiver.
    #[must_use]
    pub fn actor_backpressured_for_tests() -> Self {
        let mut handle = Self::closed_for_tests();
        handle.network_actor_byte_budget = NetworkActorByteBudget::new(1, 0)
            .expect("one-byte zero-reserve test actor budget must fit");
        handle
    }
    /// Override one plaintext topic cap on a disconnected test handle.
    ///
    /// This keeps protocol-carrier admission tests coupled to the same exact
    /// frame-cap boundary used by the live actor without opening sockets.
    #[must_use]
    pub fn with_topic_plaintext_frame_cap_for_tests(
        mut self,
        topic: message::Topic,
        cap: usize,
    ) -> Self {
        match topic {
            message::Topic::ConsensusSafety | message::Topic::Control => {
                self.topic_frame_caps.control = cap;
            }
            message::Topic::Consensus => self.topic_frame_caps.consensus = cap,
            message::Topic::ConsensusPayload
            | message::Topic::ConsensusChunk
            | message::Topic::BlockSync => self.topic_frame_caps.block_sync = cap,
            message::Topic::TxGossip | message::Topic::TxGossipRestricted => {
                self.topic_frame_caps.tx_gossip = cap;
            }
            message::Topic::PeerGossip | message::Topic::TrustGossip => {
                self.topic_frame_caps.peer_gossip = cap;
            }
            message::Topic::Health => self.topic_frame_caps.health = cap,
            message::Topic::Other => self.topic_frame_caps.other = cap,
        }
        self
    }
    /// Return the configured plaintext frame cap for one outbound topic.
    ///
    /// Application protocols which retain a response before P2P handoff use
    /// this live value to keep their source allocation below the same ceiling
    /// enforced by exact actor admission. The cap includes the relay/data
    /// envelope, so callers must reserve their own payload overhead within it.
    #[must_use]
    pub fn topic_plaintext_frame_cap(&self, topic: message::Topic) -> usize {
        self.topic_frame_caps.for_topic(topic)
    }
    /// Admit a best-effort post while preserving exact source ownership on failure.
    ///
    /// This is the outcome-returning counterpart of [`Self::post`]. It is intended for bounded
    /// control protocols whose producer must not release a response reservation until the network
    /// actor positively accepts the exact message. Reliable semantic-progress routes must continue
    /// to use [`Self::post_recoverable`] because they require source-keyed tickets.
    ///
    /// # Errors
    ///
    /// Returns [`NetworkPostAdmissionError::Backpressured`] when actor byte/item capacity is
    /// temporarily unavailable, [`NetworkPostAdmissionError::Closed`] after actor shutdown, or
    /// [`NetworkPostAdmissionError::Rejected`] for an outbound policy, wire-length, frame-cap, or
    /// corridor violation. Every error owns the exact original post.
    #[allow(clippy::needless_pass_by_value)]
    pub fn post_best_effort_recoverable(
        &self,
        mut msg: Post<T>,
    ) -> Result<(), NetworkPostAdmissionError<Post<T>>> {
        use tokio::sync::mpsc::error::TrySendError;
        let requested_priority = msg.priority;
        if !msg.data.is_outbound_allowed() {
            return Err(NetworkPostAdmissionError::Rejected {
                message: msg,
                reason: NetworkActorAdmissionRejection::OutboundDisallowed,
            });
        }
        let topic = msg.data.topic();
        let route = msg.data.subscriber_route();
        if is_reliable_progress_route(topic, route) {
            return Err(NetworkPostAdmissionError::Rejected {
                message: msg,
                reason: NetworkActorAdmissionRejection::ReliableProgressRequiresRecoverable,
            });
        }
        msg.priority = canonical_outbound_priority(topic, route, msg.priority);
        let message = NetworkMessage::Post(msg);
        let restore_post = |message: NetworkMessage<T>| match message {
            NetworkMessage::Post(mut post) => {
                post.priority = requested_priority;
                post
            }
            NetworkMessage::Broadcast(_) => {
                unreachable!("post admission must return the submitted post")
            }
        };
        let wire_bytes = match self.outbound_actor_wire_bytes_recoverable(&message, topic) {
            Ok(bytes) => bytes,
            Err(reason) => {
                return Err(NetworkPostAdmissionError::Rejected {
                    message: restore_post(message),
                    reason,
                });
            }
        };
        let priority = canonical_outbound_priority(topic, route, requested_priority);
        let (budget, sender) = if matches!(priority, Priority::Low) {
            (
                &self.network_actor_low_byte_budget,
                &self.network_message_low_sender,
            )
        } else {
            (
                &self.network_actor_byte_budget,
                &self.network_message_high_sender,
            )
        };
        let Some(lease) = budget.try_reserve(wire_bytes, false) else {
            return Err(NetworkPostAdmissionError::Backpressured {
                message: restore_post(message),
            });
        };
        let admitted = AdmittedNetworkMessage::new(message, lease);
        match sender.try_send(admitted) {
            Ok(()) => Ok(()),
            Err(TrySendError::Closed(admitted)) => {
                let (message, lease) = admitted.into_parts();
                drop(lease);
                Err(NetworkPostAdmissionError::Closed {
                    message: restore_post(message),
                })
            }
            Err(TrySendError::Full(admitted)) if matches!(priority, Priority::High) => {
                let admitted = match defer_high_priority_network_message(
                    sender.clone(),
                    admitted,
                    false,
                    topic,
                    &self.network_message_high_deferred_permits,
                ) {
                    Ok(()) => return Ok(()),
                    Err(admitted) => admitted,
                };
                let (message, lease) = admitted.into_parts();
                drop(lease);
                Err(NetworkPostAdmissionError::Backpressured {
                    message: restore_post(message),
                })
            }
            Err(TrySendError::Full(admitted)) => {
                let (message, lease) = admitted.into_parts();
                drop(lease);
                Err(NetworkPostAdmissionError::Backpressured {
                    message: restore_post(message),
                })
            }
        }
    }
}
