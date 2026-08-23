// DURABLE_VALIDATE_ASYNC_HANDOFF_IMPLEMENTATION_BEGIN
#[cfg_attr(not(test), allow(dead_code))]
impl DetachedDurableValidateExecution {
    /// Execute the exact detached request through the scheduler-free body-store
    /// validation boundary.
    ///
    /// The request is consumed once. A storage failure returns it intact for a
    /// typed recovery decision; a successful storage call seals the request and
    /// closed outcome together in one move-only token.
    #[allow(clippy::result_large_err)]
    fn execute<F, E>(
        self,
        body_store: &mut V2BodyStore,
        validator: F,
    ) -> Result<
        ExecutedDurableValidateExecution,
        (V2BodyStoreError, DetachedDurableValidateExecution),
    >
    where
        F: FnOnce(&SignedBlock) -> Result<wire::ExecutionCommitment, E>,
        E: BodyValidationError,
    {
        let outcome = match body_store.execute_durable_validation(
            self.durable_receipt.clone(),
            self.expected_manifest_hash,
            validator,
        ) {
            Ok(outcome) => outcome,
            Err(error) => return Err((error, self)),
        };
        if outcome.durable_body() != &self.durable_receipt {
            return Err((V2BodyStoreError::ReceiptMismatch, self));
        }
        Ok(ExecutedDurableValidateExecution {
            request: self,
            outcome,
        })
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl ExecutedDurableValidateExecution {
    /// Borrow the body-store-minted closed result without separating it from
    /// the detached registry authority.
    pub(super) const fn outcome(&self) -> &DurableBodyValidationOutcome {
        &self.outcome
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl PreparedDurableValidateCompletion<'_> {
    /// Return the exact reducer coordinates retained across detached execution.
    pub(super) const fn adapter_preview_inputs(
        &self,
    ) -> (EventTag, wire::ConsensusRound, wire::BlockSubject) {
        (
            self.executed.request.tag,
            self.executed.request.round,
            self.executed.request.subject,
        )
    }
    /// Borrow the closed body-store outcome retained under the exact registry
    /// reattachment.
    pub(super) const fn outcome(&self) -> &DurableBodyValidationOutcome {
        &self.executed.outcome
    }
}
// DURABLE_VALIDATE_ASYNC_HANDOFF_IMPLEMENTATION_END
// DURABLE_VALIDATE_WAIT_DISPATCH_IMPLEMENTATION_BEGIN
#[cfg_attr(not(test), allow(dead_code))]
impl DurableValidateDispatch {
    /// Recheck the registry-attested immutable worker key before queue publication.
    pub(in crate::sumeragi) fn matches_dispatch_key(
        &self,
        key: super::LifecycleValidateDispatchKeyV1,
    ) -> bool {
        self.request.address.owner == key.owner()
            && self.request.address.ordinal == key.lifecycle_ordinal()
            && self.request.address.slot == key.slot()
            && self.request.incumbent_digest == key.digest()
            && self.request.lifecycle_key.context().as_bytes()
                == self.request.round.context_id.0.as_ref()
            && self.request.lifecycle_key.round().height() == self.request.round.height
            && self.request.lifecycle_key.phase() == super::LifecyclePhase::Validate
            && self.request.lifecycle_stage.kind() == super::LifecycleStageKind::ValidateBody
    }
    /// Execute the exact request after its claimed lifecycle row became an
    /// external wait.
    ///
    /// This is the sole externally visible execution path. A body-store error
    /// reconstructs and returns the complete dispatch, including its exact
    /// wake authority, so retry cannot mint a second request or wait token.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn execute<F, E>(
        self,
        body_store: &mut V2BodyStore,
        validator: F,
    ) -> Result<ExecutedDurableValidateDispatch, (V2BodyStoreError, Self)>
    where
        F: FnOnce(&SignedBlock) -> Result<wire::ExecutionCommitment, E>,
        E: BodyValidationError,
    {
        let Self { request, wake } = self;
        match request.execute(body_store, validator) {
            Ok(executed) => Ok(ExecutedDurableValidateDispatch { executed, wake }),
            Err((error, request)) => Err((error, Self { request, wake })),
        }
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl ExecutedDurableValidateDispatch {
    /// Recheck the immutable worker key retained from command through completion.
    pub(in crate::sumeragi) fn matches_dispatch_key(
        &self,
        key: super::LifecycleValidateDispatchKeyV1,
    ) -> bool {
        self.executed.request.address.owner == key.owner()
            && self.executed.request.address.ordinal == key.lifecycle_ordinal()
            && self.executed.request.address.slot == key.slot()
            && self.executed.request.incumbent_digest == key.digest()
    }
    /// Borrow the closed result without separating it from wake authority.
    pub(super) const fn outcome(&self) -> &DurableBodyValidationOutcome {
        self.executed.outcome()
    }
    #[cfg(test)]
    const fn wait_token_for_test(&self) -> WaitToken {
        self.wake.wait_token
    }
}
#[cfg(test)]
impl DurableValidateDispatch {
    const fn wait_token_for_test(&self) -> WaitToken {
        self.wake.wait_token
    }
}
// DURABLE_VALIDATE_WAIT_DISPATCH_IMPLEMENTATION_END
// DURABLE_VALIDATE_VOLATILE_COMPLETION_IMPLEMENTATION_BEGIN
#[cfg_attr(not(test), allow(dead_code))]
impl DurableValidateCompletionAuthority {
    /// Exact immutable owner of the waiting record.
    pub(super) const fn owner(self) -> OwnerId {
        self.address.owner
    }
    /// Existing lifecycle ordinal; completion never allocates another one.
    pub(super) const fn ordinal(self) -> u128 {
        self.address.ordinal
    }
    /// Equal-address physical slot retained across publication.
    pub(super) const fn slot(self) -> PhysicalSlotId {
        self.address.slot
    }
    /// Digest of the original closed Validate carrier.
    pub(super) const fn incumbent_digest(self) -> LifecycleDigest {
        self.incumbent_digest
    }
    /// Outcome-bound digest installed only for executable outcomes.
    pub(super) const fn replacement_digest(self) -> Option<LifecycleDigest> {
        self.replacement_digest
    }
    /// Exact wait token retained from the claimed-side dispatch cut.
    pub(super) const fn wait_token(self) -> WaitToken {
        self.wait_token
    }
    /// Exact immutable lifecycle key validated before async detachment.
    pub(super) const fn lifecycle_key(self) -> LifecycleKey {
        self.lifecycle_key
    }
    /// Exact immutable lifecycle stage validated before async detachment.
    pub(super) const fn lifecycle_stage(self) -> LifecycleStage {
        self.lifecycle_stage
    }
    /// Return whether the waiting row retains this completion's exact frame.
    pub(super) fn matches_durable_payload(self, payload: DurablePayloadReference) -> bool {
        self.payload == payload
            && super::body_pipeline_transition::durable_validate_payload_is_exact(
                self.lifecycle_key,
                payload,
            )
    }
    /// Whether this exact result must remain Waiting for merge-sidecar service.
    pub(super) const fn is_deferred_merge_sidecar(self) -> bool {
        matches!(
            self.outcome_kind,
            DurableValidateOutcomeKind::DeferredMergeSidecar
        )
    }
    /// Construct the only Ready event authorized by this executable outcome.
    pub(super) fn ready_event(self) -> Option<ReadyEvent> {
        let replacement_digest = self.replacement_digest?;
        if self.is_deferred_merge_sidecar() {
            return None;
        }
        Some(ReadyEvent::new(
            self.address.ordinal,
            self.address.owner,
            self.wait_token,
            Some(PhysicalReplacement::new(
                self.address.slot,
                PhysicalSlot::new(self.address.slot, replacement_digest),
            )),
        ))
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'a> PreparedExecutedDurableValidateCompletion<'a> {
    /// Borrow the sealed coordinator publication projection.
    pub(super) const fn authority(&self) -> DurableValidateCompletionAuthority {
        self.authority
    }
    /// Return this preflight only as an ownership-preserving typed failure.
    #[allow(clippy::result_large_err)]
    pub(super) fn fail(
        self,
        error: DurableValidateCompletionPublicationError,
    ) -> (
        DurableValidateCompletionPublicationError,
        ExecutedDurableValidateDispatch,
    ) {
        (error, self.dispatch)
    }
    /// Retain a missing merge-sidecar result without changing either live row.
    ///
    /// The lifecycle sidecar owner consumes this token only in its sealed
    /// registration and same-row wake transaction; raw wait authority remains
    /// inaccessible.
    pub(super) fn defer_merge_sidecar(self) -> DeferredDurableValidateDispatch {
        debug_assert!(self.authority.is_deferred_merge_sidecar());
        debug_assert!(self.dispatch.outcome().missing_merge_sidecar().is_some());
        DeferredDurableValidateDispatch {
            dispatch: self.dispatch,
        }
    }
    /// Stage the exact executable outcome as a same-address closed carrier.
    ///
    /// Every CAS and outcome comparison precedes mutation. Once installed, the
    /// returned guard owns rollback until its infallible commit is called.
    #[allow(clippy::result_large_err)]
    pub(super) fn stage_executable_carrier(
        self,
    ) -> Result<
        StagedDurableValidateCompletion<'a>,
        (
            DurableValidateCompletionPublicationError,
            ExecutedDurableValidateDispatch,
        ),
    > {
        let authority = self.authority;
        let Some(replacement_digest) = authority.replacement_digest else {
            return Err(
                self.fail(DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidReplacementDigest,
                )),
            );
        };
        if authority.is_deferred_merge_sidecar() || replacement_digest == authority.incumbent_digest
        {
            return Err(
                self.fail(DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidOutcome,
                )),
            );
        }
        let request = &self.dispatch.executed.request;
        let outcome = self.dispatch.outcome();
        let validation_error = match self.registry.entries.get(&authority.address) {
            None => Some(DurableValidateExecutionError::Registry(
                RegistryError::Missing,
            )),
            Some(work) if !work.validates_at(authority.address) => Some(
                DurableValidateExecutionError::Registry(RegistryError::CorruptWork),
            ),
            Some(work) if work.digest != authority.incumbent_digest => Some(
                DurableValidateExecutionError::Registry(RegistryError::DigestMismatch),
            ),
            Some(work) => match &work.kind {
                ConcreteLifecycleWorkKind::DurableValidateBody(incumbent)
                    if incumbent.address == request.address
                        && incumbent.durable_receipt == request.durable_receipt
                        && incumbent.expected_manifest_hash == request.expected_manifest_hash
                        && incumbent.pending.causal_lifecycle_key()
                            == &request.causal_lifecycle_key
                        && incumbent.pending.candidate_statement()
                            == request.candidate_statement
                        && outcome.durable_body() == &incumbent.durable_receipt
                        && durable_validate_completion_digest(
                            authority.incumbent_digest,
                            incumbent.expected_manifest_hash,
                            outcome,
                        ) == Some(replacement_digest) =>
                {
                    None
                }
                ConcreteLifecycleWorkKind::DurableValidateBody(_) => {
                    Some(DurableValidateExecutionError::InvalidValidateShape)
                }
                _ => Some(DurableValidateExecutionError::WrongWorkKind),
            },
        };
        if let Some(error) = validation_error {
            return Err(
                self.fail(DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::Execution(error),
                )),
            );
        }
        let location = DurableValidatePublishedLocation {
            address: authority.address,
            incumbent_digest: authority.incumbent_digest,
            replacement_digest,
            round: request.round,
            subject: request.subject,
        };
        let publication = match authority.outcome_kind {
            DurableValidateOutcomeKind::Validated => {
                PublishedDurableValidateCompletion::Validated(PublishedValidated { location })
            }
            DurableValidateOutcomeKind::Rejected => {
                PublishedDurableValidateCompletion::Rejected(PublishedRejected { location })
            }
            DurableValidateOutcomeKind::DeferredMergeSidecar => unreachable!(
                "deferred Validate outcome was rejected before same-address conversion"
            ),
        };
        let PreparedExecutedDurableValidateCompletion {
            registry,
            dispatch,
            authority: _,
        } = self;
        let ExecutedDurableValidateDispatch { executed, wake } = dispatch;
        let ExecutedDurableValidateExecution { request, outcome } = executed;
        let Some(incumbent) = registry.entries.remove(&authority.address) else {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::Execution(
                        DurableValidateExecutionError::Registry(RegistryError::Missing),
                    ),
                ),
                ExecutedDurableValidateDispatch {
                    executed: ExecutedDurableValidateExecution { request, outcome },
                    wake,
                },
            ));
        };
        let ConcreteLifecycleWork {
            digest: incumbent_digest,
            kind,
        } = incumbent;
        let incumbent = match kind {
            ConcreteLifecycleWorkKind::DurableValidateBody(incumbent) => incumbent,
            kind => {
                let _ = registry.entries.insert(
                    authority.address,
                    ConcreteLifecycleWork {
                        digest: incumbent_digest,
                        kind,
                    },
                );
                return Err((
                    DurableValidateCompletionPublicationError::Registry(
                        DurableValidateCompletionConversionError::Execution(
                            DurableValidateExecutionError::WrongWorkKind,
                        ),
                    ),
                    ExecutedDurableValidateDispatch {
                        executed: ExecutedDurableValidateExecution { request, outcome },
                        wake,
                    },
                ));
            }
        };
        let completion = DurableValidateCompletion {
            address: authority.address,
            incumbent,
            incumbent_digest,
            outcome,
        };
        let installed = ConcreteLifecycleWork {
            digest: replacement_digest,
            kind: ConcreteLifecycleWorkKind::DurableValidateCompletion(completion),
        };
        let displaced = registry.entries.insert(authority.address, installed);
        let staged = StagedDurableValidateCompletion {
            rollback: ArmedDurableValidateRollback {
                entries: &mut registry.entries,
                address: authority.address,
                request: Some(request),
                wake: Some(wake),
                armed: true,
            },
            publication,
        };
        debug_assert!(displaced.is_none());
        debug_assert!(
            staged
                .rollback
                .entries
                .get(&authority.address)
                .is_some_and(|work| {
                    work.validates_at(authority.address) && work.digest == replacement_digest
                })
        );
        drop(displaced);
        Ok(staged)
    }
}
impl ArmedDurableValidateRollback<'_> {
    fn restore(&mut self) -> Option<ExecutedDurableValidateDispatch> {
        if !self.armed {
            return None;
        }
        self.armed = false;
        let request = self.request.take();
        let wake = self.wake.take();
        let Some(installed) = self.entries.remove(&self.address) else {
            drop(request);
            drop(wake);
            return None;
        };
        let ConcreteLifecycleWork {
            digest: replacement_digest,
            kind,
        } = installed;
        let completion = match kind {
            ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) => completion,
            kind => {
                let _ = self.entries.insert(
                    self.address,
                    ConcreteLifecycleWork {
                        digest: replacement_digest,
                        kind,
                    },
                );
                drop(request);
                drop(wake);
                return None;
            }
        };
        let DurableValidateCompletion {
            address,
            incumbent,
            incumbent_digest,
            outcome,
        } = completion;
        let _ = self.entries.insert(
            address,
            ConcreteLifecycleWork {
                digest: incumbent_digest,
                kind: ConcreteLifecycleWorkKind::DurableValidateBody(incumbent),
            },
        );
        let (Some(request), Some(wake)) = (request, wake) else {
            return None;
        };
        Some(ExecutedDurableValidateDispatch {
            executed: ExecutedDurableValidateExecution { request, outcome },
            wake,
        })
    }
}
impl Drop for ArmedDurableValidateRollback<'_> {
    fn drop(&mut self) {
        drop(self.restore());
    }
}
impl StagedDurableValidateCompletion<'_> {
    /// Permanently retain the already-installed carrier and return its
    /// precomputed move-only publication metadata.
    pub(super) fn commit(self) -> PublishedDurableValidateCompletion {
        let StagedDurableValidateCompletion {
            mut rollback,
            publication,
        } = self;
        rollback.armed = false;
        publication
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl DeferredDurableValidateDispatch {
    /// Recheck the immutable worker key without exposing the retained request.
    pub(in crate::sumeragi) fn matches_dispatch_key(
        &self,
        key: super::LifecycleValidateDispatchKeyV1,
    ) -> bool {
        self.dispatch.matches_dispatch_key(key)
    }

    /// Project the sole durable sidecar-registration identity from the sealed
    /// request, missing-sidecar outcome, and exact Waiting generation.
    pub(in crate::sumeragi) fn sidecar_registration_identity(
        &self,
        key: super::LifecycleValidateDispatchKeyV1,
    ) -> Option<super::validate_sidecar::LifecycleValidateSidecarRegistrationIdentityV1> {
        self.matches_dispatch_key(key).then(|| {
            let request = &self.dispatch.executed.request;
            super::validate_sidecar::LifecycleValidateSidecarRegistrationIdentityV1::from_sealed_dispatch(
                key,
                request.lifecycle_key,
                request.lifecycle_stage,
                request.round,
                request.subject,
                self.dispatch.wake.wait_token,
                self.missing_reference().clone(),
            )
        })?
    }

    /// Borrow the exact missing sidecar reference without exposing wake parts.
    pub(in crate::sumeragi) fn missing_reference(&self) -> &CertifiedMergeLedgerReference {
        self.dispatch
            .outcome()
            .missing_merge_sidecar()
            .expect("deferred Validate token retains one exact merge-sidecar reference")
    }
    #[cfg(test)]
    const fn dispatch_for_test(&self) -> &ExecutedDurableValidateDispatch {
        &self.dispatch
    }
}
#[cfg(test)]
impl PublishedValidated {
    const fn location_for_test(&self) -> &DurableValidatePublishedLocation {
        &self.location
    }
}
#[cfg(test)]
impl PublishedRejected {
    const fn location_for_test(&self) -> &DurableValidatePublishedLocation {
        &self.location
    }
}
// DURABLE_VALIDATE_VOLATILE_COMPLETION_IMPLEMENTATION_END
impl<'a> PreparedCertifiedFetchCompletion<'a> {
    /// Bind this drop-inert preflight to the exact body-store durability proof.
    ///
    /// Every comparison is read-only. Failure or drop leaves the incumbent
    /// registry row unchanged, while success moves the exclusive borrow and
    /// receipt into the sole post-dequeue authority.
    #[allow(dead_code)]
    pub(super) fn bind_durable_body_receipt(
        self,
        durable_receipt: DurableCertifiedFetchBodyReceipt,
    ) -> Result<
        PreparedDurableCertifiedFetchCompletion<'a>,
        (
            CertifiedFetchCompletionError,
            DurableCertifiedFetchBodyReceipt,
        ),
    > {
        macro_rules! retain_receipt {
            ($error:expr) => {
                return Err(($error, durable_receipt))
            };
        }
        let address = self.location.address();
        let Some(incumbent) = self.registry.entries.get(&address) else {
            retain_receipt!(CertifiedFetchCompletionError::MissingIncumbent);
        };
        if !incumbent.validates_at(address) {
            retain_receipt!(CertifiedFetchCompletionError::CorruptIncumbent);
        }
        let ConcreteLifecycleWorkKind::PendingAdapter {
            effect: incumbent_effect,
            pending: incumbent_pending,
            ..
        } = &incumbent.kind
        else {
            retain_receipt!(CertifiedFetchCompletionError::WrongIncumbentShape);
        };
        if !matches!(incumbent_effect, AdapterEffect::FetchBody { .. }) {
            retain_receipt!(CertifiedFetchCompletionError::WrongIncumbentShape);
        }
        if self.location.owner.causal_root() != incumbent.causal_root() {
            retain_receipt!(CertifiedFetchCompletionError::ForeignCausalOwner);
        }
        if incumbent.digest != self.location.incumbent_digest {
            retain_receipt!(CertifiedFetchCompletionError::IncumbentDigestMismatch);
        }
        if !durable_receipt_matches_fetch(
            &durable_receipt,
            incumbent_effect,
            self.request_hash,
            self.response_hash,
            self.response_round,
            self.response_subject,
            self.response_manifest_hash,
        ) {
            retain_receipt!(CertifiedFetchCompletionError::DurableReceiptMismatch);
        }
        let Some(replay_evidence) = self.replay_origin.bind_durable_body(&durable_receipt) else {
            retain_receipt!(CertifiedFetchCompletionError::InvalidReplayEvidence);
        };
        let Some(ready_projection) = replay_evidence.project_durable_ready_fetch(
            incumbent_effect,
            incumbent_pending,
            durable_receipt.durable_body(),
        ) else {
            retain_receipt!(CertifiedFetchCompletionError::InvalidReplayEvidence);
        };
        if ready_projection.completion_digest() == self.location.incumbent_digest {
            retain_receipt!(CertifiedFetchCompletionError::ReplacementDigestMismatch);
        }
        Ok(PreparedDurableCertifiedFetchCompletion {
            registry: self.registry,
            location: self.location,
            ingress_identity: self.ingress_identity,
            request_hash: self.request_hash,
            response_hash: self.response_hash,
            authenticated_responder: self.authenticated_responder,
            durable_receipt,
            replay_evidence,
            ready_projection,
        })
    }
}
impl PreparedDurableCertifiedFetchCompletion<'_> {
    /// Borrow the opaque durable projection used by the coordinator's staged cut.
    pub(super) const fn ready_projection(&self) -> &DurableCertifiedFetchReplayProjectionV1 {
        &self.ready_projection
    }
    /// Borrow the exact durable body authority retained for terminal publication.
    pub(super) const fn durable_body_receipt(&self) -> &DurableBodyReceipt {
        self.durable_receipt.durable_body()
    }
    /// Revalidate the selector-retained exact response before LedgerV1 fsync.
    ///
    /// The later checked dequeue can then mint only an ownership carrier; its
    /// registry install has no fallible response or durable-receipt checks.
    pub(super) fn matches_selected_response(
        &self,
        ingress_identity: PendingFairIngressIdentity,
        inbound: &InboundBlockMessage,
        disposition: FairV2IngressDequeueDisposition,
    ) -> bool {
        exact_selected_response_matches(
            ingress_identity,
            inbound,
            disposition,
            self.registry
                .entries
                .get(&self.location.address())
                .and_then(|work| match &work.kind {
                    ConcreteLifecycleWorkKind::PendingAdapter { effect, .. } => Some(effect),
                    _ => None,
                }),
            self.request_hash,
            self.response_hash,
            &self.authenticated_responder,
            &self.durable_receipt,
        )
    }
    /// Return the sealed receipt before any external queue mutation.
    ///
    /// The selector uses this only to reconstruct the complete opaque Phase-B
    /// input after a retryable checked-dequeue rejection. The registry remains
    /// byte-for-byte unchanged.
    pub(super) fn abort_before_dequeue(self) -> DurableCertifiedFetchBodyReceipt {
        self.durable_receipt
    }
    /// Install the closed completion only after checked dequeue returned its
    /// exact owned response carrier. The occurrence is authenticated here and
    /// then dropped; installed work retains only restart-stable material.
    ///
    /// Every fallible comparison precedes the first map mutation. Once those
    /// comparisons succeed, the exclusive registry borrow guarantees that the
    /// previously validated incumbent still occupies `location`; removal,
    /// construction, and same-address insertion are then infallible.
    ///
    /// The caller invokes this only after LedgerV1 fsync and exact dequeue,
    /// under an armed fail-stop operation. Assertions therefore represent a
    /// process-fatal invariant violation, never a retryable completion error.
    pub(super) fn commit_after_exact_dequeue(self, dequeued: CertifiedFetchDequeuedResponse) {
        assert_eq!(dequeued.ingress_identity(), self.ingress_identity);
        let address = self.location.address();
        let incumbent = self
            .registry
            .entries
            .get(&address)
            .expect("preflighted certified-Fetch incumbent remains installed");
        let ConcreteLifecycleWorkKind::PendingAdapter {
            effect: incumbent_effect,
            ..
        } = &incumbent.kind
        else {
            panic!("preflighted certified-Fetch incumbent changed work kind")
        };
        assert!(incumbent.validates_at(address));
        assert!(matches!(incumbent_effect, AdapterEffect::FetchBody { .. }));
        assert_eq!(self.location.owner.causal_root(), incumbent.causal_root());
        assert_eq!(incumbent.digest, self.location.incumbent_digest);
        assert!(exact_selected_response_matches(
            dequeued.ingress_identity(),
            dequeued.inbound(),
            dequeued.disposition(),
            Some(incumbent_effect),
            self.request_hash,
            self.response_hash,
            &self.authenticated_responder,
            &self.durable_receipt,
        ));
        let incumbent = self
            .registry
            .entries
            .remove(&address)
            .expect("exclusively borrowed validated incumbent remains installed");
        let ConcreteLifecycleWork {
            digest: incumbent_digest,
            kind,
        } = incumbent;
        let ConcreteLifecycleWorkKind::PendingAdapter {
            effect: incumbent_effect,
            pending: incumbent_pending,
            ..
        } = kind
        else {
            panic!("validated certified-Fetch incumbent remains a pending adapter")
        };
        let durable_receipt = self.durable_receipt.durable_body().clone();
        let installed_digest = self.ready_projection.completion_digest();
        let completion = CertifiedFetchCompletion {
            address,
            incumbent_effect,
            incumbent_pending,
            incumbent_digest,
            durable_receipt,
            replay_evidence: self.replay_evidence,
        };
        let installed = ConcreteLifecycleWork {
            digest: installed_digest,
            kind: ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion),
        };
        assert!(installed.validate_exact());
        assert!(
            self.registry.entries.insert(address, installed).is_none(),
            "removed completion address remains vacant until same-address install"
        );
    }
}
fn ingress_identity_matches_round(
    identity: PendingFairIngressIdentity,
    round: wire::ConsensusRound,
) -> bool {
    let mut context_id = [0_u8; 32];
    context_id.copy_from_slice(round.context_id.0.as_ref());
    identity.context().height() == round.height
        && identity.context().id() == LifecycleDigest::new(context_id)
}
fn fetch_effect_matches_response(
    effect: &AdapterEffect,
    response: &wire::CertifiedBodyResponse,
) -> bool {
    fetch_effect_matches_manifest(effect, response.manifest.round, response.manifest.subject)
}
fn fetch_effect_matches_manifest(
    effect: &AdapterEffect,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
) -> bool {
    matches!(
        effect,
        AdapterEffect::FetchBody {
            round: fetch_round,
            subject: fetch_subject,
            ..
        } if *fetch_round == round && *fetch_subject == subject
    )
}
fn durable_receipt_matches_fetch(
    receipt: &DurableCertifiedFetchBodyReceipt,
    effect: &AdapterEffect,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    manifest_hash: HashOf<wire::PayloadManifest>,
) -> bool {
    let durable_body = receipt.durable_body();
    receipt.request_hash() == request_hash
        && receipt.response_hash() == response_hash
        && durable_body.context_id() == round.context_id
        && durable_body.round() == round
        && durable_body.subject() == subject
        && durable_body.manifest_hash() == manifest_hash
        && fetch_effect_matches_manifest(effect, round, subject)
}
fn exact_selected_response_matches(
    ingress_identity: PendingFairIngressIdentity,
    inbound: &InboundBlockMessage,
    disposition: FairV2IngressDequeueDisposition,
    effect: Option<&AdapterEffect>,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    authenticated_responder: &PeerId,
    durable_receipt: &DurableCertifiedFetchBodyReceipt,
) -> bool {
    if ingress_identity.physical_admission_ordinal() == 0
        || disposition != FairV2IngressDequeueDisposition::Admit
    {
        return false;
    }
    let Some(effect) = effect else {
        return false;
    };
    let Some(response) = selected_certified_response(inbound) else {
        return false;
    };
    inbound.sender() == authenticated_responder
        && ingress_identity_matches_round(ingress_identity, response.manifest.round)
        && response.request_hash == request_hash
        && HashOf::new(response) == response_hash
        && fetch_effect_matches_response(effect, response)
        && durable_receipt_matches_fetch(
            durable_receipt,
            effect,
            request_hash,
            response_hash,
            response.manifest.round,
            response.manifest.subject,
            HashOf::new(&response.manifest),
        )
}
fn selected_certified_response(
    inbound: &InboundBlockMessage,
) -> Option<&wire::CertifiedBodyResponse> {
    let BlockMessage::V2(message) = inbound.message() else {
        return None;
    };
    if message.validate_version().is_err() {
        return None;
    }
    let wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response) = &message.payload else {
        return None;
    };
    Some(response)
}
#[cfg(test)]
fn certified_pipeline_prepare_certificate_for_test(
    manifest: &wire::PayloadManifest,
    receipt: &DurableBodyReceipt,
) -> wire::QuorumCertificate {
    wire::QuorumCertificate {
        round: manifest.round,
        proposal_round: manifest.round,
        phase: wire::GlobalPhase::Prepare,
        subject: manifest.subject,
        execution_commitment: ValidatedBodyReceipt::for_test(receipt.clone())
            .execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xC1; 96],
    }
}
#[cfg(test)]
fn certified_pipeline_replay_evidence_for_test(
    tag: EventTag,
    manifest: &wire::PayloadManifest,
    receipt: &DurableBodyReceipt,
    validate_pending: &PendingRuntimeEffectBinding,
) -> Option<(
    CertifiedStoreReplayEvidenceV1,
    CertifiedValidateReplayEvidenceV1,
)> {
    let certificate = certified_pipeline_prepare_certificate_for_test(manifest, receipt);
    certified_pipeline_replay_evidence_with_certificate_for_test(
        tag,
        manifest,
        receipt,
        validate_pending,
        certificate,
    )
}
#[cfg(test)]
fn certified_pipeline_replay_evidence_with_certificate_for_test(
    tag: EventTag,
    manifest: &wire::PayloadManifest,
    receipt: &DurableBodyReceipt,
    validate_pending: &PendingRuntimeEffectBinding,
    certificate: wire::QuorumCertificate,
) -> Option<(
    CertifiedStoreReplayEvidenceV1,
    CertifiedValidateReplayEvidenceV1,
)> {
    let fetch_effect = AdapterEffect::FetchBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(certificate),
    };
    let response = wire::CertifiedBodyResponse {
        request_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"certified pipeline replay fixture request",
        )),
        manifest: manifest.clone(),
        body: vec![0xC2],
        responder: 0,
        signature: vec![0xC3],
    };
    let fetch = CertifiedFetchReplayEvidenceV1::from_signed_response_for_test(
        &fetch_effect,
        &response,
        receipt,
    )?;
    let store_effect = AdapterEffect::StoreBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let store = fetch.project_store_for_test(&store_effect, receipt)?;
    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let validate =
        store.project_validate(&store_effect, receipt, &validate_effect, validate_pending)?;
    Some((store, validate))
}
fn digest_from_hash(hash: &iroha_crypto::Hash) -> LifecycleDigest {
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(hash.as_ref());
    LifecycleDigest::new(bytes)
}
fn durable_validate_body_payload(receipt: &DurableBodyReceipt) -> Option<DurablePayloadReference> {
    let mut context = [0_u8; 32];
    context.copy_from_slice(receipt.context_id().0.as_ref());
    let active_context =
        LifecycleContext::new(LifecycleDigest::new(context), receipt.round().height);
    projection::durable_body_frame_reference(active_context, receipt)
        .map(DurablePayloadReference::BodyFrame)
}
fn validate_validated_receipt_authority(
    validate: &DurableValidateBody,
    validated_receipt: &ValidatedBodyReceipt,
) -> Result<(), DurableValidateExecutionError> {
    let AdapterEffect::ValidateBody { round, subject, .. } = &validate.effect else {
        return Err(DurableValidateExecutionError::InvalidValidateShape);
    };
    if validated_receipt.durable() != &validate.durable_receipt
        || validated_receipt.execution_commitment().validate().is_err()
    {
        return Err(DurableValidateExecutionError::InvalidValidationReceipt);
    }
    let Some(statement) = validate.pending.candidate_statement() else {
        return Err(DurableValidateExecutionError::InvalidValidateShape);
    };
    if statement.context_id() != round.context_id
        || statement.proposal_round() != *round
        || statement.subject() != Some(*subject)
    {
        return Err(DurableValidateExecutionError::InvalidValidateShape);
    }
    if statement
        .execution_commitment()
        .is_some_and(|commitment| commitment != validated_receipt.execution_commitment())
    {
        return Err(DurableValidateExecutionError::ConflictingValidationCommitment);
    }
    Ok(())
}
fn validated_body_completion_digest(
    incumbent_digest: LifecycleDigest,
    expected_manifest_hash: HashOf<wire::PayloadManifest>,
    validated_receipt: &ValidatedBodyReceipt,
) -> LifecycleDigest {
    const DOMAIN: &[u8] = b"iroha:sumeragi:v2:lifecycle:validated-body-completion:v1";
    let commitment = validated_receipt.execution_commitment().encode();
    let mut preimage = Vec::with_capacity(DOMAIN.len() + 1 + 32 + 32 + 32 + 8 + commitment.len());
    preimage.extend_from_slice(DOMAIN);
    preimage.push(0);
    preimage.extend_from_slice(incumbent_digest.as_bytes());
    preimage.extend_from_slice(expected_manifest_hash.as_ref());
    preimage.extend_from_slice(validated_receipt.durable().frame_hash().as_ref());
    preimage.extend_from_slice(
        &u64::try_from(commitment.len())
            .expect("bounded execution commitment encoding fits u64")
            .to_le_bytes(),
    );
    preimage.extend_from_slice(&commitment);
    digest_from_hash(&Hash::new(preimage))
}
fn rejected_body_completion_digest(
    incumbent_digest: LifecycleDigest,
    expected_manifest_hash: HashOf<wire::PayloadManifest>,
    durable_receipt: &DurableBodyReceipt,
    identity: &BodyValidationRejectionIdentity,
) -> LifecycleDigest {
    const DOMAIN: &[u8] = b"iroha:sumeragi:v2:lifecycle:rejected-body-completion:v2";
    let mut preimage = Vec::with_capacity(DOMAIN.len() + 1 + 32 + 32 + 32 + 1);
    preimage.extend_from_slice(DOMAIN);
    preimage.push(0);
    preimage.extend_from_slice(incumbent_digest.as_bytes());
    preimage.extend_from_slice(expected_manifest_hash.as_ref());
    preimage.extend_from_slice(durable_receipt.frame_hash().as_ref());
    preimage.push(identity.canonical_code());
    digest_from_hash(&Hash::new(preimage))
}
fn durable_validate_outcome_kind(
    outcome: &DurableBodyValidationOutcome,
) -> Option<DurableValidateOutcomeKind> {
    match (
        outcome.validated_receipt().is_some(),
        outcome.rejection_reason().is_some(),
        outcome.rejection_identity().is_some(),
        outcome.missing_merge_sidecar().is_some(),
    ) {
        (true, false, false, false) => Some(DurableValidateOutcomeKind::Validated),
        (false, true, true, false) => Some(DurableValidateOutcomeKind::Rejected),
        (false, false, false, true) => Some(DurableValidateOutcomeKind::DeferredMergeSidecar),
        _ => None,
    }
}
fn durable_validate_completion_digest(
    incumbent_digest: LifecycleDigest,
    expected_manifest_hash: HashOf<wire::PayloadManifest>,
    outcome: &DurableBodyValidationOutcome,
) -> Option<LifecycleDigest> {
    match durable_validate_outcome_kind(outcome)? {
        DurableValidateOutcomeKind::Validated => {
            let receipt = outcome.validated_receipt()?;
            (receipt.execution_commitment().validate().is_ok()
                && receipt.durable() == outcome.durable_body())
            .then(|| {
                validated_body_completion_digest(incumbent_digest, expected_manifest_hash, receipt)
            })
        }
        DurableValidateOutcomeKind::Rejected => {
            let identity = outcome.rejection_identity()?;
            Some(rejected_body_completion_digest(
                incumbent_digest,
                expected_manifest_hash,
                outcome.durable_body(),
                identity,
            ))
        }
        DurableValidateOutcomeKind::DeferredMergeSidecar => None,
    }
}
fn durable_validation_wait_source_for_request(
    request: &DetachedDurableValidateExecution,
) -> WaitSource {
    durable_validation_wait_source_from_exact_parts(
        request.address,
        request.incumbent_digest,
        &request.causal_lifecycle_key,
        request.candidate_statement,
        &request.durable_receipt,
        request.expected_manifest_hash,
        request.lifecycle_key,
        request.lifecycle_stage,
    )
}
fn durable_validation_wait_source_from_exact_parts(
    address: ConcreteWorkAddress,
    incumbent_digest: LifecycleDigest,
    causal_lifecycle_key: &Hash,
    candidate_statement: Option<RuntimeCandidateSemanticStatement>,
    durable_receipt: &DurableBodyReceipt,
    expected_manifest_hash: HashOf<wire::PayloadManifest>,
    lifecycle_key: LifecycleKey,
    lifecycle_stage: LifecycleStage,
) -> WaitSource {
    let durable_frame_hash = durable_receipt.frame_hash();
    projection::durable_validation_wait_source(
        address.owner,
        address.ordinal,
        address.slot,
        incumbent_digest,
        causal_lifecycle_key,
        candidate_statement,
        &durable_frame_hash,
        expected_manifest_hash,
        lifecycle_key,
        lifecycle_stage,
    )
}
