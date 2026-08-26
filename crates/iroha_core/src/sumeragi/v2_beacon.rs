//! Live global threshold-beacon production for Sumeragi v2.
//!
//! The height-local lifecycle reconstructs one exact pulse payload from the
//! authenticated height context, finalized parent, and active public DKG
//! session. Runtime-only signers contribute proof-carrying
//! adaptive shares; the reducer authenticates the transport sender, verifies
//! every share, and exposes only the unique threshold-combined pulse to block
//! assembly.

use std::sync::Arc;

use iroha_data_model::{
    block::consensus_v2 as wire,
    consensus::{
        FinalizedGlobalThresholdBeaconPulseV1, GlobalThresholdBeaconChainAnchorV1,
        NposConsensusEffects,
    },
    governance::types::BeaconSessionId,
    peer::PeerId,
};
use mv::storage::StorageReadOnly;
use thiserror::Error;

use crate::{
    beacon::{
        GlobalThresholdBeaconError, GlobalThresholdBeaconPartialSignerV1,
        GlobalThresholdBeaconPulseAggregatorV1, GlobalThresholdBeaconSessionBindingV1,
        ValidatedGlobalThresholdBeaconSessionV1,
        authenticated_global_threshold_beacon_roster_hash_v1,
        validate_global_threshold_beacon_session_v1,
    },
    state::{GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY, State, WorldReadOnly},
};

/// Fatal construction or local-signing failure for one beacon height.
#[derive(Debug, Error)]
pub(crate) enum V2GlobalBeaconError {
    /// The frozen consensus context is malformed.
    #[error("invalid frozen context for global threshold-beacon production: {0}")]
    Context(#[from] wire::ValidationError),
    /// Required finalized public beacon state is absent or inconsistent.
    #[error("global threshold-beacon production state is unavailable: {0}")]
    State(&'static str),
    /// The active public session or a signature share failed cryptographic validation.
    #[error(transparent)]
    Beacon(#[from] GlobalThresholdBeaconError),
    /// The active public DKG roster differs from the frozen consensus roster.
    #[error("active global threshold-beacon roster differs from the frozen height roster")]
    RosterMismatch,
    /// The injected runtime signer does not own this node's frozen DKG seat.
    #[error("runtime global threshold-beacon signer differs from the local validator seat")]
    LocalSignerMismatch,
    /// The secure runtime could not produce the requested local share.
    #[error("runtime global threshold-beacon signing failed")]
    LocalSigning,
    /// A partial belongs to another active view.
    #[error("global threshold-beacon partial belongs to another active view")]
    WrongView,
    /// The authenticated transport sender differs from the claimed DKG seat.
    #[error("global threshold-beacon partial sender differs from its frozen DKG seat")]
    SenderMismatch,
}

/// Result of admitting one proof-carrying beacon share.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum V2GlobalBeaconIngressOutcome {
    /// A new verified signer share was retained.
    Accepted,
    /// The same signature share was retried, possibly with fresh proof randomness.
    Duplicate,
    /// This share reached the threshold and produced the unique finalized pulse.
    Finalized,
}

struct ActiveGlobalBeaconRound {
    session: ValidatedGlobalThresholdBeaconSessionV1,
    aggregator: GlobalThresholdBeaconPulseAggregatorV1,
    finalized: Option<FinalizedGlobalThresholdBeaconPulseV1>,
    view: Option<wire::View>,
    retransmit: Option<wire::ConsensusMessageV2>,
}

/// Per-height live threshold-beacon producer owned by the serialized v2 runner.
pub(crate) struct V2GlobalBeaconLifecycle {
    context: wire::HeightContext,
    roster: Vec<PeerId>,
    local_validator: Option<wire::ValidatorIndex>,
    signer: Option<Arc<dyn GlobalThresholdBeaconPartialSignerV1>>,
    requested: bool,
    required_for_consensus: bool,
    active: Option<ActiveGlobalBeaconRound>,
    outbound: Vec<wire::ConsensusMessageV2>,
}

impl core::fmt::Debug for V2GlobalBeaconLifecycle {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("V2GlobalBeaconLifecycle")
            .field("height", &self.context.height)
            .field("local_validator", &self.local_validator)
            .field("signer_available", &self.signer.is_some())
            .field("active", &self.active.is_some())
            .field("outbound_len", &self.outbound.len())
            .finish()
    }
}

impl V2GlobalBeaconLifecycle {
    /// Open the exact height producer from committed public state.
    ///
    /// NPoS pre-boundary slots are consensus-mandatory. Committed Parliament
    /// sortition and timed-ballot slots are also produced, but remain optional
    /// for chain liveness so their objective missing-pulse retry paths remain
    /// reachable. Every requested slot uses the same authenticated producer.
    pub(crate) fn open(
        context: &wire::HeightContext,
        state: &State,
        local_validator: Option<wire::ValidatorIndex>,
        signer: Option<Arc<dyn GlobalThresholdBeaconPartialSignerV1>>,
    ) -> Result<Self, V2GlobalBeaconError> {
        context.validate()?;
        let roster = context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>();
        if local_validator.is_none() {
            return Ok(Self {
                context: context.clone(),
                roster,
                local_validator: None,
                signer: None,
                requested: false,
                required_for_consensus: false,
                active: None,
                outbound: Vec::new(),
            });
        }
        let required_for_consensus = context.mode == wire::ConsensusMode::Npos
            && context
                .height
                .checked_add(1)
                .is_some_and(|next| next == context.epoch_end_height);
        let world = state.world_view();
        let logical_beacon_id = BeaconSessionId::for_network_v1(&context.network_id);
        let parliament_requested_at_height =
            world.parliament_attempts().iter().any(|(_, attempt)| {
                attempt.requires_beacon_pulse_at(logical_beacon_id, context.height)
            });
        if !required_for_consensus && !parliament_requested_at_height {
            return Ok(Self {
                context: context.clone(),
                roster,
                local_validator,
                signer,
                requested: false,
                required_for_consensus,
                active: None,
                outbound: Vec::new(),
            });
        }

        let active = (|| -> Result<ActiveGlobalBeaconRound, V2GlobalBeaconError> {
            let session_id = world
                .global_beacon_active_session()
                .get(&GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY)
                .copied()
                .ok_or(V2GlobalBeaconError::State("active key session is absent"))?;
            let key_record = world
                .global_beacon_key_sessions()
                .get(&session_id)
                .cloned()
                .ok_or(V2GlobalBeaconError::State("active key record is absent"))?;
            if !key_record.is_active_at(context.height) {
                return Err(V2GlobalBeaconError::State(
                    "active key is not live at the pulse height",
                ));
            }
            let expected_roster_hash =
                authenticated_global_threshold_beacon_roster_hash_v1(&key_record.session, &roster)
                    .map_err(|_| V2GlobalBeaconError::RosterMismatch)?;
            let binding = GlobalThresholdBeaconSessionBindingV1 {
                network_id: context.network_id,
                session_id,
                roster_hash: expected_roster_hash,
                transcript_hash: key_record.session.transcript_hash,
            };
            let session =
                validate_global_threshold_beacon_session_v1(key_record.session, &binding)?;

            let anchor_height = context
                .height
                .checked_sub(1)
                .ok_or(V2GlobalBeaconError::State("pulse height has no parent"))?;
            let block_hashes = state.block_hashes.view();
            let expected_chain_len = usize::try_from(anchor_height).map_err(|_| {
                V2GlobalBeaconError::State("finalized chain length is not representable")
            })?;
            if block_hashes.len() != expected_chain_len {
                return Err(V2GlobalBeaconError::State(
                    "finalized chain does not end at the pulse parent",
                ));
            }
            let block_hash = block_hashes
                .last()
                .copied()
                .ok_or(V2GlobalBeaconError::State(
                    "finalized parent hash is absent",
                ))?;
            let context_parent_hash = context
                .parent_commit_qc
                .as_ref()
                .map(|certificate| certificate.subject.block_hash)
                .or_else(|| {
                    context
                        .snapshot_bootstrap
                        .as_ref()
                        .map(|snapshot| snapshot.snapshot_block_hash)
                })
                .ok_or(V2GlobalBeaconError::State(
                    "height context does not authenticate its finalized parent",
                ))?;
            if context_parent_hash != block_hash {
                return Err(V2GlobalBeaconError::State(
                    "height context parent differs from the finalized-chain journal",
                ));
            }
            let anchor = GlobalThresholdBeaconChainAnchorV1 {
                height: anchor_height,
                block_hash,
            };
            let aggregator = GlobalThresholdBeaconPulseAggregatorV1::new(
                session.clone(),
                context.height,
                anchor,
            )?;
            Ok(ActiveGlobalBeaconRound {
                session,
                aggregator,
                finalized: None,
                view: None,
                retransmit: None,
            })
        })();

        let active = match active {
            Ok(active) => Some(active),
            Err(_) if !required_for_consensus => None,
            Err(error) => return Err(error),
        };

        Ok(Self {
            context: context.clone(),
            roster,
            local_validator,
            signer,
            requested: true,
            required_for_consensus,
            active,
            outbound: Vec::new(),
        })
    }

    /// Return whether committed state requests a pulse attempt at this height.
    #[must_use]
    pub(crate) const fn pulse_requested(&self) -> bool {
        self.requested
    }

    /// Return whether absence of the pulse must stop consensus at this height.
    #[must_use]
    pub(crate) const fn pulse_required_for_consensus(&self) -> bool {
        self.required_for_consensus
    }

    /// Route the height-bound pulse through a view and emit the local share.
    ///
    /// The outer round changes for routing, but the retained aggregator and its
    /// threshold-signed payload stay fixed across every view at this height.
    pub(crate) fn begin_round(&mut self, view: wire::View) -> Result<(), V2GlobalBeaconError> {
        let Some(active) = self.active.as_mut() else {
            return Ok(());
        };
        if active.view == Some(view) {
            return Ok(());
        }
        if active.view.is_some_and(|previous| view < previous) {
            return Err(V2GlobalBeaconError::WrongView);
        }
        active.view = Some(view);
        active.retransmit = None;

        if let (Some(local_validator), Some(signer)) = (self.local_validator, self.signer.as_ref())
        {
            let partial = signer
                .sign_partial(&active.session, active.aggregator.payload())
                .map_err(|_| V2GlobalBeaconError::LocalSigning)?;
            let expected_index = u16::try_from(local_validator)
                .ok()
                .and_then(|index| index.checked_add(1))
                .ok_or(V2GlobalBeaconError::LocalSignerMismatch)?;
            if partial.signer_index != expected_index {
                return Err(V2GlobalBeaconError::LocalSignerMismatch);
            }
            #[cfg(feature = "test-network-parliament-signers")]
            let deliberately_invalid_outbound =
                signer.test_network_emit_invalid_outbound_partial_v1();
            #[cfg(feature = "test-network-parliament-signers")]
            let partial = if deliberately_invalid_outbound {
                let mut partial = partial;
                partial.signature_share[0] ^= 1;
                partial
            } else {
                let _ = active.aggregator.accept_partial(partial)?;
                partial
            };
            #[cfg(not(feature = "test-network-parliament-signers"))]
            let _ = active.aggregator.accept_partial(partial)?;
            let message = wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::GlobalBeaconPartialSignature(
                    wire::GlobalBeaconPartialSignature {
                        round: wire::ConsensusRound {
                            context_id: self.context.id(),
                            height: self.context.height,
                            view,
                        },
                        partial,
                    },
                ),
            );
            active.retransmit = Some(message.clone());
            self.outbound.push(message);
            #[cfg(not(feature = "test-network-parliament-signers"))]
            if active.finalized.is_none()
                && active.aggregator.verified_partial_count()
                    >= usize::from(active.session.record().threshold)
            {
                active.finalized = Some(active.aggregator.finalize()?);
            }
            #[cfg(feature = "test-network-parliament-signers")]
            if !deliberately_invalid_outbound
                && active.finalized.is_none()
                && active.aggregator.verified_partial_count()
                    >= usize::from(active.session.record().threshold)
            {
                active.finalized = Some(active.aggregator.finalize()?);
            }
        }
        Ok(())
    }

    /// Authenticate and retain one network partial for the exact active view.
    pub(crate) fn accept_partial(
        &mut self,
        message: wire::GlobalBeaconPartialSignature,
        sender: &PeerId,
        active_view: wire::View,
    ) -> Result<V2GlobalBeaconIngressOutcome, V2GlobalBeaconError> {
        message.validate(&self.context)?;
        if message.round.view != active_view {
            return Err(V2GlobalBeaconError::WrongView);
        }
        self.begin_round(active_view)?;
        let seat = message
            .partial
            .signer_index
            .checked_sub(1)
            .map(usize::from)
            .ok_or(V2GlobalBeaconError::SenderMismatch)?;
        if self.roster.get(seat) != Some(sender) {
            return Err(V2GlobalBeaconError::SenderMismatch);
        }
        let active = self.active.as_mut().ok_or(V2GlobalBeaconError::State(
            "partial arrived outside a required beacon height",
        ))?;
        let aggregator = &mut active.aggregator;
        let inserted = aggregator.accept_partial(message.partial)?;
        if !inserted {
            return Ok(V2GlobalBeaconIngressOutcome::Duplicate);
        }
        if aggregator.verified_partial_count() < usize::from(active.session.record().threshold) {
            return Ok(V2GlobalBeaconIngressOutcome::Accepted);
        }
        let pulse = aggregator.finalize()?;
        match active.finalized {
            Some(previous) if previous != pulse => Err(V2GlobalBeaconError::Beacon(
                GlobalThresholdBeaconError::PersistenceConflict,
            )),
            Some(_) => Ok(V2GlobalBeaconIngressOutcome::Duplicate),
            None => {
                active.finalized = Some(pulse);
                Ok(V2GlobalBeaconIngressOutcome::Finalized)
            }
        }
    }

    /// Drain freshly generated local partials for bounded broadcast.
    pub(crate) fn take_outbound(&mut self) -> Vec<wire::ConsensusMessageV2> {
        core::mem::take(&mut self.outbound)
    }

    /// Clone the local exact-view partial for periodic retransmission.
    pub(crate) fn retransmission(&self) -> Vec<wire::ConsensusMessageV2> {
        self.active
            .as_ref()
            .and_then(|active| active.retransmit.clone())
            .into_iter()
            .collect()
    }

    /// Return the unique finalized pulse for candidate effects in `view`.
    #[must_use]
    pub(crate) fn finalized_pulse(
        &self,
        view: wire::View,
    ) -> Option<FinalizedGlobalThresholdBeaconPulseV1> {
        self.active.as_ref().and_then(|active| {
            (active.view == Some(view))
                .then_some(active.finalized)
                .flatten()
        })
    }

    /// Attach the exact-view pulse to candidate effects, failing closed when
    /// a consensus-mandatory pre-boundary height has not reconstructed it yet.
    pub(crate) fn attach_candidate_effects(
        &self,
        view: wire::View,
        effects: &mut NposConsensusEffects,
    ) -> Result<(), V2GlobalBeaconError> {
        let pulse = self.finalized_pulse(view);
        if self.pulse_required_for_consensus() && pulse.is_none() {
            return Err(V2GlobalBeaconError::State(
                "required finalized pulse is absent for the candidate view",
            ));
        }
        effects.finalized_global_beacon_pulse = pulse;
        Ok(())
    }
}
