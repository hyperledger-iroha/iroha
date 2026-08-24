impl LifecycleDecisionApplyAdapterCompletionAuthorityV1 {
    /// Check a guarded completion against the exact dispatched pending-Kura replay.
    ///
    /// The registry has already rebound this result to the sole active carrier;
    /// this executor-side oracle independently preserves the native recovery
    /// context, reducer tag, CommitQC, Kura receipt, and finality artifact before
    /// any LedgerV1 terminal publication is attempted.
    pub(in crate::sumeragi) fn exactly_matches_pending_kura_recovery(
        &self,
        context: &wire::HeightContext,
        evidence: &super::v2_effects::PendingKuraApplyRecoveryEvidence,
    ) -> bool {
        self.lineage() == LifecycleDecisionApplyLineageV1::Recovered
            && evidence.stage()
                == super::v2_effects::PendingKuraApplyRecoveryStage::ApplicationDispatched
            && evidence.is_exact(context)
            && self.tag == evidence.replay_tag()
            && self.subject == evidence.commit_subject()
            && self.dispatch_key.matches_height_context(context)
            && self.artifact.validate().is_ok()
            && self.artifact.height_context == *context
            && self.artifact.subject == evidence.commit_subject()
            && &self.artifact.commit_qc == evidence.commit_qc()
            && self.receipt.height() == evidence.frozen_height()
            && self.receipt.context_id() == evidence.frozen_context_id()
            && self.receipt.block_hash() == evidence.commit_subject().block_hash
            && self.receipt.subject() == evidence.commit_subject()
            && self.receipt.certificate() == evidence.commit_qc().as_ref()
            && self.receipt.artifact_hash() == HashOf::new(&self.artifact)
    }
}

impl SumeragiV2Adapter {
    /// Preview one lifecycle-owned recovered signature without appending WAL
    /// or publishing output.
    ///
    /// The guarded worker result is unpacked only through this module's
    /// private permit. The cloned reducer must emit exactly one signed
    /// Broadcast, optionally followed by one already-durable Sign, or—for a
    /// local Proposal—one Prepare-intent persistence request. A recovered
    /// Proposal whose exact Prepare intent was already fsynced may instead
    /// emit Broadcast followed by its Prepare Sign. Any quorum,
    /// timeout-install, report, body, or unrelated persistence shape fails
    /// closed before lifecycle publication.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion(
        &mut self,
        authority: super::v2_worker::RecoveredLifecycleSignAdapterCompletionAuthorityV1,
    ) -> Result<PreparedRecoveredLifecycleSignAdapterCompletionV1<'_>, AdapterError> {
        self.ensure_ingress()?;
        let (dispatch_key, tag, request, signature, outbound_payload) =
            authority.consume_for_adapter(RecoveredLifecycleSignAdapterCompletionPermitV1::new());
        if !dispatch_key.matches_height_context(&self.wire_context) {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let signer = match &request {
            SignRequest::Proposal(proposal) => proposal.proposer,
            SignRequest::Vote(vote) => vote.signer,
            SignRequest::TimeoutVote(vote) => vote.signer,
        };
        let local_signer = self
            .reducer
            .local_validator()
            .map(|validator| self.registry.validator_index(validator))
            .transpose()?
            .ok_or(AdapterError::RecoveredLifecycleSignCompletionMismatch)?;
        if signer != local_signer
            || verify_individual_signature(
                &self.wire_context,
                signer,
                &signature,
                &request.signature_preimage(),
            )
            .is_err()
            || match (&request, &outbound_payload) {
                (SignRequest::Proposal(proposal), Some(payload)) => {
                    payload.manifest() != &proposal.manifest
                }
                (SignRequest::Vote(_) | SignRequest::TimeoutVote(_), None) => false,
                (SignRequest::Proposal(_), None)
                | (SignRequest::Vote(_) | SignRequest::TimeoutVote(_), Some(_)) => true,
            }
        {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        // Certified TC/CommitQC ingress may legally bypass a signature fence.
        // Authenticate the completed worker output first, then distinguish its
        // old exact fence from corruption. This prevents an arbitrary stale or
        // forged completion from claiming the supersession retirement path.
        let superseded = self.recovered_lifecycle_sign_is_certifiably_superseded(tag);
        if self.current_tag() != tag || self.pending_persistence_id.is_some() {
            return Err(if superseded {
                AdapterError::RecoveredLifecycleSignCompletionSuperseded
            } else {
                AdapterError::RecoveredLifecycleSignCompletionMismatch
            });
        }
        let mut next_registry = self.registry.clone();
        let Some(awaiting) = self.reducer.awaiting_signature() else {
            return Err(if superseded {
                AdapterError::RecoveredLifecycleSignCompletionSuperseded
            } else {
                AdapterError::RecoveredLifecycleSignCompletionMismatch
            });
        };
        let awaiting_request = match awaiting {
            reducer::SignableMessage::Proposal(proposal) => SignRequest::Proposal(
                next_registry.unsigned_proposal_to_wire(proposal, self.aggregator.as_ref())?,
            ),
            reducer::SignableMessage::Vote(vote) => {
                SignRequest::Vote(next_registry.unsigned_vote_to_wire(*vote)?)
            }
            reducer::SignableMessage::TimeoutVote(vote) => SignRequest::TimeoutVote(
                next_registry.unsigned_timeout_vote_to_wire(vote, self.aggregator.as_ref())?,
            ),
        };
        if awaiting_request != request {
            return Err(if superseded {
                AdapterError::RecoveredLifecycleSignCompletionSuperseded
            } else {
                AdapterError::RecoveredLifecycleSignCompletionMismatch
            });
        }
        let event = reducer::Event::Signed {
            tag,
            signature: reducer::OpaqueSignature::new(signature.clone()),
        };
        let mut next_reducer = self.reducer.clone();
        let outcome = next_reducer.step(event.clone())?;
        if outcome.disposition() != reducer::StepDisposition::Applied {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let core_effects = outcome.into_effects();
        let mut converted = Vec::with_capacity(core_effects.len());
        for effect in &core_effects {
            let converted_effect = match effect {
                reducer::Effect::Broadcast(message) => AdapterEffect::Broadcast(
                    next_registry.message_to_wire(message.clone(), self.aggregator.as_ref())?,
                ),
                reducer::Effect::Sign { tag, message } => {
                    let request = match message {
                        reducer::SignableMessage::Proposal(proposal) => SignRequest::Proposal(
                            next_registry
                                .unsigned_proposal_to_wire(proposal, self.aggregator.as_ref())?,
                        ),
                        reducer::SignableMessage::Vote(vote) => {
                            SignRequest::Vote(next_registry.unsigned_vote_to_wire(*vote)?)
                        }
                        reducer::SignableMessage::TimeoutVote(vote) => SignRequest::TimeoutVote(
                            next_registry
                                .unsigned_timeout_vote_to_wire(vote, self.aggregator.as_ref())?,
                        ),
                    };
                    AdapterEffect::Sign { tag: *tag, request }
                }
                reducer::Effect::Persist { .. }
                | reducer::Effect::FetchBody { .. }
                | reducer::Effect::StoreBody { .. }
                | reducer::Effect::ValidateBody { .. }
                | reducer::Effect::Apply { .. }
                | reducer::Effect::EnterView { .. }
                | reducer::Effect::ReportEquivocation { .. }
                | reducer::Effect::ReportInvalidCertifiedBody { .. } => continue,
            };
            converted.push(converted_effect);
        }
        let mut expected_signed_request = request.clone();
        match &mut expected_signed_request {
            SignRequest::Proposal(proposal) => proposal.signature.clone_from(&signature),
            SignRequest::Vote(vote) => vote.signature.clone_from(&signature),
            SignRequest::TimeoutVote(vote) => vote.signature.clone_from(&signature),
        }
        let expected_broadcast = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
            match expected_signed_request {
                SignRequest::Proposal(proposal) => {
                    wire::ConsensusMessageV2Payload::Proposal(proposal)
                }
                SignRequest::Vote(vote) => wire::ConsensusMessageV2Payload::Vote(vote),
                SignRequest::TimeoutVote(vote) => {
                    wire::ConsensusMessageV2Payload::TimeoutVote(vote)
                }
            },
        ));
        if converted.first() != Some(&expected_broadcast) {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let broadcast = converted.remove(0);
        let mut pending_prepare = None;
        let mut persist_count = 0_usize;
        for effect in &core_effects {
            if let reducer::Effect::Persist {
                tag: persist_tag,
                entry,
            } = effect
            {
                persist_count = persist_count.saturating_add(1);
                if !matches!(
                    (&request, entry.record()),
                    (
                        SignRequest::Proposal(proposal),
                        reducer::WalRecord::PrepareIntent(vote),
                    ) if vote.phase() == reducer::Phase::Prepare
                        && vote.round().height() == proposal.round.height
                        && vote.round().view() == proposal.round.view
                        && self.registry.subject(vote.subject()).ok() == Some(proposal.subject)
                ) {
                    return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
                }
                pending_prepare = Some((*persist_tag, entry.clone()));
            }
        }
        if persist_count > 1 || converted.len() > 1 {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let next_sign = converted.pop();
        if next_sign
            .as_ref()
            .is_some_and(|effect| !matches!(effect, AdapterEffect::Sign { .. }))
        {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let expected_shape_is_exact = match (&request, &pending_prepare, &next_sign) {
            (SignRequest::Proposal(_), Some((persist_tag, entry)), None) => {
                *persist_tag == tag
                    && next_reducer.pending_persistence_record() == Some(entry.record())
                    && next_reducer.awaiting_signature().is_none()
                    && core_effects.len() == 2
            }
            (
                SignRequest::Proposal(_),
                None,
                Some(AdapterEffect::Sign {
                    request: SignRequest::Vote(vote),
                    ..
                }),
            ) => {
                vote.phase == wire::GlobalPhase::Prepare
                    && next_reducer.pending_persistence_record().is_none()
                    && next_reducer.awaiting_signature().is_some()
                    && core_effects.len() == 2
            }
            (SignRequest::Vote(_) | SignRequest::TimeoutVote(_), None, possible_next_sign) => {
                next_reducer.pending_persistence_record().is_none()
                    && match (next_reducer.awaiting_signature(), possible_next_sign) {
                        (None, None) => true,
                        (Some(_), Some(AdapterEffect::Sign { .. })) => true,
                        _ => false,
                    }
                    && core_effects.len() == 1 + usize::from(possible_next_sign.is_some())
            }
            _ => false,
        };
        if !expected_shape_is_exact {
            return Err(AdapterError::RecoveredLifecycleSignCompletionMismatch);
        }
        let next_fence = ReducerFenceProjection {
            pending_persistence: next_reducer.pending_persistence_record().cloned(),
            awaiting_signature: next_reducer.awaiting_signature().cloned(),
            replay_complete: self.replay_complete,
        };
        let next_fence_generation = if next_fence == self.reducer_fence_projection() {
            self.reducer_fence_generation
        } else {
            self.reducer_fence_generation
                .checked_add(1)
                .filter(|next| *next != u64::MAX)
                .ok_or(AdapterError::ReducerFenceGenerationExhausted)?
        };
        Ok(PreparedRecoveredLifecycleSignAdapterCompletionV1 {
            adapter: self,
            next_reducer,
            next_registry,
            event,
            core_effects,
            broadcast,
            next_sign,
            combined_authority_minted: false,
            proposal_output_authority_minted: false,
            next_vote_body_store_identity: None,
            next_vote_output_guard: None,
            pending_prepare,
            prepared_prepare_wal: None,
            persisted_prepare_wal: None,
            outbound_payload,
            next_fence_generation,
            dispatch_key,
        })
    }
    /// Return whether authenticated safety progress has retired an older exact
    /// recovered Sign fence at this height.
    fn recovered_lifecycle_sign_is_certifiably_superseded(
        &self,
        completed_tag: reducer::EventTag,
    ) -> bool {
        self.current_tag().strictly_advances(completed_tag)
            || self.reducer.durable_state().decision().is_some()
            || matches!(
                self.reducer.pending_persistence_record(),
                Some(
                    reducer::WalRecord::ObservePrepare(_)
                        | reducer::WalRecord::LockAndCommit { .. }
                        | reducer::WalRecord::InstallTimeout(_)
                        | reducer::WalRecord::Decision(_)
                )
            )
    }
}
