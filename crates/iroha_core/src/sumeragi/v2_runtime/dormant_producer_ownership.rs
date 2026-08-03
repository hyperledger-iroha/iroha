/// Read-only lookup result for a deterministic fresh root reconstructed after
/// restart. Multiple exact stage records may share one lifecycle, but they
/// must all retain the same immutable ordinal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeDormantProducerLifecycle {
    /// No dormant record owns this causal key.
    Absent,
    /// The exact persisted lifecycle owns this immutable ordinal.
    Exact { admission_ordinal: u128 },
    /// Dormant metadata disagreed about status, durability, or ordinal.
    Conflict,
}

/// Restart-dormant local producer stage which already owns one latent FIFO slot.
///
/// The adjacent producer-continuation snapshot carries only internal admission
/// metadata, never a command payload or wire field.  The runtime installs this
/// projection before admitting any live work, charges it against the existing
/// class-aware queue allocation, and removes it only when the exact restored
/// lifecycle/stage becomes a physical FIFO command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct RuntimeDormantLocalFifoReservation {
    causal_lifecycle_key: iroha_crypto::Hash,
    admission_ordinal: u128,
    producer_stage: u8,
    class: CommandClass,
}

impl RuntimeDormantLocalFifoReservation {
    const TIMEOUT_ELAPSED_STAGE: u8 = 6;

    const fn is_known_stage(producer_stage: u8) -> bool {
        producer_stage <= 10
    }

    const fn is_local_fifo_stage(producer_stage: u8) -> bool {
        matches!(producer_stage, 0 | 8 | 9 | 10)
    }

    /// Bind one locally replayable producer stage to the trusted completion lane.
    pub(crate) const fn completion(
        causal_lifecycle_key: iroha_crypto::Hash,
        admission_ordinal: u128,
        producer_stage: u8,
    ) -> Self {
        Self {
            causal_lifecycle_key,
            admission_ordinal,
            producer_stage,
            class: CommandClass::Completion,
        }
    }
}

impl<E> RuntimeDriverDispatch<E> {
    #[cfg(test)]
    fn completed(effects: Vec<E>) -> Self {
        Self {
            effects,
            deferred_ingress: None,
            deferred_ordinal: None,
            retry_unadmitted: false,
            producer_handoff: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeEffectSource {
    Startup,
    Fifo,
    Deferred,
    Timeout,
    Retransmit,
}

pub(crate) trait RuntimeDriver {
    /// Command payload consumed by the driver.
    type Command: ExactRuntimeCommandIdentity + Clone;
    /// Effect emitted unchanged to asynchronous adapters.
    type Effect;
    /// Fatal transition error.
    type Error: fmt::Display;

    /// Current authoritative reducer tag.
    fn current_tag(&self) -> EventTag;
    /// Classify an exact command without mutating reducer, registry, queue, or
    /// ordinal state. Authenticated wire ingress is always admitted here and
    /// remains governed by its dedicated authentication/equivocation seam.
    fn preflight_command_admission(
        &self,
        _tag: EventTag,
        _command: &Self::Command,
    ) -> RuntimeCommandAdmissionPreflight {
        RuntimeCommandAdmissionPreflight::Admit
    }
    /// Look up a restart-dormant deterministic root by its recomputed causal
    /// lifecycle key without mutating adapter or scheduler state.
    fn dormant_producer_lifecycle(
        &self,
        _causal_lifecycle_key: &iroha_crypto::Hash,
    ) -> RuntimeDormantProducerLifecycle {
        RuntimeDormantProducerLifecycle::Absent
    }
    /// Enumerate every restart-dormant Local stage whose deterministic replay
    /// will enter the serialized FIFO. Non-FIFO timeout roots and
    /// transport-conditional work are deliberately absent.
    fn dormant_local_fifo_reservations(
        &self,
    ) -> Result<Vec<RuntimeDormantLocalFifoReservation>, String> {
        Ok(Vec::new())
    }
    /// Deliver one admitted command with its original tag.
    fn dispatch(
        &mut self,
        command: TaggedCommand<Self::Command>,
    ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error>;
    /// Bind one scheduler-validated lifecycle to a timer transition whose
    /// compact driver method otherwise carries only the reducer tag.
    fn bind_selected_producer_lifecycle(
        &mut self,
        _owner: &RuntimeLifecycleOwner,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Clear a lifecycle binding after the driver transition returns.
    fn clear_selected_producer_lifecycle(&mut self) {}
    /// Classify the exact producer replacement already retained by this
    /// dispatch. Production must distinguish durable, concrete-successor, and
    /// process-local volatile terminals; effect-count inference alone is not
    /// sufficient.
    fn producer_handoff_evidence(
        &self,
        _token: ProducerContinuationHandoffToken,
        _has_concrete_successor: bool,
    ) -> Result<ProducerContinuationHandoffEvidence, Self::Error> {
        unreachable!("a synthetic driver cannot classify producer handoff tokens")
    }
    /// Acknowledge an exact producer only after the runtime installed its
    /// concrete successor sidecar or retained exact durable terminal evidence.
    fn acknowledge_producer_handoff(
        &mut self,
        _token: ProducerContinuationHandoffToken,
        _evidence: ProducerContinuationHandoffEvidence,
    ) -> Result<ProducerContinuationTerminalToken, Self::Error> {
        unreachable!("a synthetic driver cannot mint producer handoff tokens")
    }
    /// Deliver the absolute round-timeout event.
    fn timeout_elapsed(
        &mut self,
        tag: EventTag,
    ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error>;
    /// Deliver one derived retransmission tick.
    fn retransmit_elapsed(
        &mut self,
        tag: EventTag,
    ) -> Result<RuntimeDriverDispatch<Self::Effect>, Self::Error>;
    /// Return whether this exact causally owned completion is the sole command
    /// which can open the adapter's current Busy-deferred signing fence.
    ///
    /// The runtime uses this only when strictly older adapter debt is present
    /// but unserviceable. Ordinary completions and stale or independent
    /// signature callbacks remain governed by immutable FIFO lifecycle order.
    /// Returning `true` also promises that dispatch consumes the signing fence;
    /// retry or insertion into another deferred lane is a contract failure.
    fn completion_unblocks_deferred_fence(&self, _tag: EventTag, _command: &Self::Command) -> bool {
        false
    }
    /// Return whether this exact queued command is demonstrably blocked by the
    /// same active fence as [`Self::completion_unblocks_deferred_fence`].
    ///
    /// The runtime uses this proof only to ignore that command's queue alias
    /// while locating the exact causal completion. External tasks, producer
    /// reservations, timers, and commands which can terminate before the
    /// reducer remain ordered blockers.
    fn command_is_blocked_by_deferred_fence(
        &self,
        _tag: EventTag,
        _command: &Self::Command,
    ) -> bool {
        false
    }
    /// Return whether adapter-owned Busy-deferred work can cross the reducer
    /// boundary without spinning behind a persistence/signing fence.
    ///
    /// This is an actor-global predicate: when it is true, every retained
    /// deferred owner is past the same reducer fences. A driver with per-owner
    /// readiness must expose an exact ordinal set instead of implementing this
    /// boolean approximately.
    fn deferred_work_is_serviceable(&self) -> bool;
    /// Actor-global source which minted deferred ownership capabilities.
    fn deferred_admission_ordinal_source(&self) -> &DeferredAdmissionOrdinalSource;
    /// Actor-global ordinals of every authenticated occurrence still retained
    /// by the adapter's Busy-deferred queues.
    fn authenticated_deferred_admission_ordinals(&self) -> BTreeSet<u128>;
    /// Actor-global ordinals of every occurrence retained by any Busy lane.
    fn all_deferred_admission_ordinals(&self) -> BTreeSet<u128>;
    /// Private adapter-issued identity of one retained Busy occurrence,
    /// sampled without claiming its service turn.
    fn deferred_occurrence_ownership(
        &self,
        _admission_ordinal: u128,
    ) -> Option<DeferredOccurrenceOwnershipEvidence> {
        None
    }
    /// Seal one newly admitted Busy occurrence to the exact runtime owner and
    /// frozen physical cut before the runtime retains its wrapper.
    fn seal_deferred_runtime_ownership(
        &mut self,
        _admission_ordinal: u128,
        _owner: &RuntimeLifecycleOwner,
        _current_ingress: RuntimeDispatchIngress,
        _source_physical_ordinal: Option<u64>,
        _physical_cut: u128,
    ) -> Result<DeferredRuntimeOwnershipSeal, Self::Error> {
        unreachable!("a synthetic driver cannot admit production Busy ownership")
    }
    /// Test-driver seam for deferred owners created outside production
    /// ingress. Production adapters must return `None` and use the runtime
    /// handoff map populated by `dispatch`.
    #[cfg(test)]
    fn synthetic_deferred_lifecycle_owner(
        &self,
        _evidence: &DeferredServiceEvidence,
    ) -> Option<RuntimeLifecycleOwner> {
        None
    }
    /// Deliver exactly one serviceable adapter-owned deferred transition and
    /// its exact selected-occurrence token. `eligible` is the non-empty set of
    /// adapter admission ordinals selected by the runtime's target-relative
    /// physical-cut relation and then by logical minimum inside each retained
    /// predecessor set.
    fn dispatch_deferred(
        &mut self,
        eligible: &BTreeSet<u128>,
    ) -> Result<
        Option<(
            Vec<Self::Effect>,
            DeferredServiceEvidence,
            Option<ProducerContinuationHandoffToken>,
        )>,
        Self::Error,
    >;
    /// Identify only the effect which authorizes timer restart.
    fn enter_view_tag(effect: &Self::Effect) -> Option<EventTag>;
    /// Classify the exceptional effects which are new TLA roots rather than
    /// causal children of the selected scheduler owner.
    fn effect_causality(
        _effect: &Self::Effect,
        _source: RuntimeEffectSource,
    ) -> RuntimeEffectCausality {
        RuntimeEffectCausality::Inherit
    }
    /// Closed refinement kind for exact effect-to-candidate projection.
    fn effect_refinement_kind(_effect: &Self::Effect) -> u8 {
        RUNTIME_EFFECT_KIND_OPAQUE_TEST
    }
    /// Exact semantic bytes for the complete concrete effect.
    fn effect_semantic_identity(_effect: &Self::Effect) -> Vec<u8> {
        vec![RUNTIME_EFFECT_KIND_OPAQUE_TEST]
    }
    /// Route-neutral candidate kind and semantic bytes, or `None` for a
    /// synchronous/transport/diagnostic effect.
    fn effect_candidate_semantic_identity(_effect: &Self::Effect) -> Option<(u8, Vec<u8>)> {
        None
    }
    /// Bind a candidate to an optional typed causal statement. Synthetic
    /// drivers retain their opaque bytes; the production adapter overrides
    /// this seam to preserve the exact body-pipeline statement.
    fn effect_candidate_semantic_binding(
        effect: &Self::Effect,
        _inherited: Option<&RuntimeCandidateSemanticStatement>,
    ) -> Result<Option<RuntimeEffectCandidateSemantic>, String> {
        Ok(
            Self::effect_candidate_semantic_identity(effect).map(|(kind, semantic_identity)| {
                RuntimeEffectCandidateSemantic {
                    kind,
                    semantic_identity,
                    statement: None,
                }
            }),
        )
    }
    /// Route-neutral semantic identity for a new TLA effect root. Diagnostic
    /// generation and local admission ordinals must not appear here.
    fn fresh_effect_semantic_identity(
        _effect: &Self::Effect,
        kind: RuntimeFreshRootKind,
    ) -> Vec<u8> {
        vec![kind.code()]
    }
    /// Reducer tag carried by an effect which may become a fresh root.
    fn effect_root_tag(_effect: &Self::Effect) -> Option<EventTag> {
        None
    }
    /// Return whether the unauthenticated wire shape could match a protected
    /// active-lock item after authentication.
    #[cfg(test)]
    fn wire_ingress_may_use_progress(&self, payload: &wire::ConsensusMessageV2Payload) -> bool;
}
