#[cfg(test)]
struct V2IoCommandTryIter<'a> {
    receiver: &'a V2IoCommandReceiver,
}
#[cfg(test)]
impl Iterator for V2IoCommandTryIter<'_> {
    type Item = V2IoCommand;
    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.try_recv().ok()
    }
}
/// Persisted certified-Fetch completion guarded fail-stop until typed drain
/// validates its work index and prepares the exact acknowledgement.
struct CertifiedFetchBodyPersistenceDropGuard {
    output_guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
impl CertifiedFetchBodyPersistenceDropGuard {
    fn new(output_guard: Arc<ConsensusOutputGuard>) -> Self {
        Self {
            output_guard,
            armed: true,
        }
    }
    fn disarm(&mut self) {
        self.armed = false;
    }
}
impl Drop for CertifiedFetchBodyPersistenceDropGuard {
    fn drop(&mut self) {
        if self.armed {
            self.output_guard.close_admission_for_restart();
        }
    }
}
struct GuardedCertifiedFetchBodyPersistenceCompletion {
    completion: Option<CertifiedFetchBodyPersistenceCompletion>,
    drop_guard: CertifiedFetchBodyPersistenceDropGuard,
}
struct RecoveredDecisionFetchBodyCompletionDropGuardV1 {
    output_guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
impl RecoveredDecisionFetchBodyCompletionDropGuardV1 {
    fn new(output_guard: Arc<ConsensusOutputGuard>) -> Self {
        Self {
            output_guard,
            armed: true,
        }
    }
    fn disarm(&mut self) {
        self.armed = false;
    }
}
impl Drop for RecoveredDecisionFetchBodyCompletionDropGuardV1 {
    fn drop(&mut self) {
        if self.armed {
            self.output_guard.close_admission_for_restart();
        }
    }
}
struct GuardedRecoveredDecisionFetchBodyPersistenceCompletionV1 {
    completion: Option<RecoveredDecisionFetchBodyPersistenceCompletionV1>,
    drop_guard: RecoveredDecisionFetchBodyCompletionDropGuardV1,
}
impl GuardedRecoveredDecisionFetchBodyPersistenceCompletionV1 {
    fn new(
        completion: RecoveredDecisionFetchBodyPersistenceCompletionV1,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Self {
        Self {
            completion: Some(completion),
            drop_guard: RecoveredDecisionFetchBodyCompletionDropGuardV1::new(output_guard),
        }
    }
    fn completion(&self) -> &RecoveredDecisionFetchBodyPersistenceCompletionV1 {
        self.completion
            .as_ref()
            .expect("armed recovered Decision Fetch completion retains its payload")
    }
    fn acknowledge_after_publication(mut self) {
        let _completion = self
            .completion
            .take()
            .expect("settled recovered Decision Fetch consumes its completion once");
        self.drop_guard.disarm();
    }
}
impl GuardedCertifiedFetchBodyPersistenceCompletion {
    fn new(
        completion: CertifiedFetchBodyPersistenceCompletion,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Self {
        Self {
            completion: Some(completion),
            drop_guard: CertifiedFetchBodyPersistenceDropGuard::new(output_guard),
        }
    }
    fn completion(&self) -> &CertifiedFetchBodyPersistenceCompletion {
        self.completion
            .as_ref()
            .expect("armed certified-Fetch completion retains its payload")
    }
    fn into_completion(mut self) -> CertifiedFetchBodyPersistenceCompletion {
        let completion = self
            .completion
            .take()
            .expect("prepared WorkAck consumes the guarded completion once");
        self.drop_guard.disarm();
        completion
    }
}
struct LifecycleDecisionApplyCompletionDropGuardV1 {
    output_guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
impl LifecycleDecisionApplyCompletionDropGuardV1 {
    fn new(output_guard: Arc<ConsensusOutputGuard>) -> Self {
        Self {
            output_guard,
            armed: true,
        }
    }
    fn disarm(&mut self) {
        self.armed = false;
    }
}
impl Drop for LifecycleDecisionApplyCompletionDropGuardV1 {
    fn drop(&mut self) {
        if self.armed {
            self.output_guard.close_admission_for_restart();
        }
    }
}
struct GuardedLifecycleDecisionApplyWorkerResultV1 {
    result: Option<LifecycleDecisionApplyWorkerResultV1>,
    drop_guard: LifecycleDecisionApplyCompletionDropGuardV1,
}
struct RecoveredLifecycleSignCompletionDropGuardV1 {
    output_guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
impl RecoveredLifecycleSignCompletionDropGuardV1 {
    fn new(output_guard: Arc<ConsensusOutputGuard>) -> Self {
        Self {
            output_guard,
            armed: true,
        }
    }
    fn disarm(&mut self) {
        self.armed = false;
    }
}
impl Drop for RecoveredLifecycleSignCompletionDropGuardV1 {
    fn drop(&mut self) {
        if self.armed {
            self.output_guard.close_admission_for_restart();
        }
    }
}
struct GuardedRecoveredLifecycleSignWorkerResultV1 {
    result: RecoveredLifecycleSignWorkerResultV1,
    drop_guard: RecoveredLifecycleSignCompletionDropGuardV1,
}
impl GuardedRecoveredLifecycleSignWorkerResultV1 {
    fn new(
        result: RecoveredLifecycleSignWorkerResultV1,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Self {
        Self {
            result,
            drop_guard: RecoveredLifecycleSignCompletionDropGuardV1::new(output_guard),
        }
    }
    const fn result(&self) -> &RecoveredLifecycleSignWorkerResultV1 {
        &self.result
    }
    fn acknowledge_after_publication(mut self) {
        self.drop_guard.disarm();
    }
}
impl GuardedLifecycleDecisionApplyWorkerResultV1 {
    fn new(
        result: LifecycleDecisionApplyWorkerResultV1,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Self {
        Self {
            result: Some(result),
            drop_guard: LifecycleDecisionApplyCompletionDropGuardV1::new(output_guard),
        }
    }
    fn result(&self) -> &LifecycleDecisionApplyWorkerResultV1 {
        self.result
            .as_ref()
            .expect("armed lifecycle Decision Apply completion retains its result")
    }
    fn into_result(mut self) -> LifecycleDecisionApplyWorkerResultV1 {
        let result = self
            .result
            .take()
            .expect("settled lifecycle Decision Apply consumes its result once");
        self.drop_guard.disarm();
        result
    }
    fn into_retry_parts(
        self,
    ) -> (
        LifecycleDecisionApplyWorkerResultV1,
        LifecycleDecisionApplyCompletionDropGuardV1,
    ) {
        let Self { result, drop_guard } = self;
        (
            result.expect("armed lifecycle Decision Apply completion retains its result"),
            drop_guard,
        )
    }
    fn from_retry_parts(
        result: LifecycleDecisionApplyWorkerResultV1,
        drop_guard: LifecycleDecisionApplyCompletionDropGuardV1,
    ) -> Self {
        Self {
            result: Some(result),
            drop_guard,
        }
    }
}
/// Move-only lifecycle Decision Apply acknowledgement consumed after durable settlement;
/// Drop closes output without releasing its queue index.
#[must_use = "lifecycle Decision Apply work remains indexed until owner settlement"]
struct LifecycleDecisionApplyWorkAckV1 {
    queue: Arc<V2IoCommandQueue>,
    output_guard: Arc<ConsensusOutputGuard>,
    key: LifecycleDecisionApplyDispatchKeyV1,
    armed: bool,
}
impl LifecycleDecisionApplyWorkAckV1 {
    fn acknowledge(mut self) {
        self.queue.acknowledge_lifecycle_decision_apply(self.key);
        self.queue
            .admission
            .acknowledge_lifecycle_decision_apply_completion(self.key);
        self.armed = false;
    }
    fn acknowledge_retry_publication(mut self) {
        self.armed = false;
    }
}
impl Drop for LifecycleDecisionApplyWorkAckV1 {
    fn drop(&mut self) {
        if self.armed {
            self.output_guard.close_admission_for_restart();
        }
    }
}
/// Guarded worker result which can be consumed only after lifecycle settlement.
#[must_use = "the lifecycle Decision Apply result still requires owner settlement"]
pub(in crate::sumeragi) struct PreparedLifecycleDecisionApplyCompletionV1 {
    guarded: Box<GuardedLifecycleDecisionApplyWorkerResultV1>,
    work_ack: LifecycleDecisionApplyWorkAckV1,
}
/// Guarded recovered-Sign completion with only a fixed adapter-private preview;
/// abandonment closes output while its command owner remains recoverable.
#[must_use = "recovered Sign completion must enter restart-closed owner settlement"]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::sumeragi) struct PreparedRecoveredLifecycleSignCompletionV1 {
    guarded: Box<GuardedRecoveredLifecycleSignWorkerResultV1>,
    queue: Arc<V2IoCommandQueue>,
}
/// Guarded durable recovered-Fetch body parked for restart-closed Store settlement.
#[must_use = "recovered Decision Fetch persistence remains guarded and indexed"]
pub(in crate::sumeragi) struct PreparedRecoveredDecisionFetchBodyCompletionV1 {
    guarded: Box<GuardedRecoveredDecisionFetchBodyPersistenceCompletionV1>,
    queue: Arc<V2IoCommandQueue>,
}
/// Guarded lifecycle Validate completion retained until its registry Ready replacement commits.
#[must_use = "lifecycle Validate completion must remain guarded until publication"]
pub(in crate::sumeragi) struct PreparedLifecycleValidateCompletionV1 {
    guarded: Box<GuardedLifecycleValidateWorkerResultV1>,
    queue: Arc<V2IoCommandQueue>,
}
/// Guarded lifecycle Serve completion retained through LedgerV1 and reply delivery.
#[must_use = "Certified-Serve completion must be settled and acknowledged"]
pub(in crate::sumeragi) struct PreparedLifecycleCertifiedServeCompletionV1 {
    guarded: Box<GuardedLifecycleCertifiedServeWorkerResultV1>,
    queue: Arc<V2IoCommandQueue>,
}
impl PreparedLifecycleValidateCompletionV1 {
    fn new(
        guarded: Box<GuardedLifecycleValidateWorkerResultV1>,
        queue: Arc<V2IoCommandQueue>,
        ownership_position: usize,
    ) -> Option<Self> {
        let key = guarded.key();
        (guarded.result().matches_dispatch_key(key)
            && queue.transfer_lifecycle_validate_completion(key, ownership_position))
        .then_some(Self { guarded, queue })
    }

    /// Split the executed dispatch from its still-armed queue/publication owner.
    pub(in crate::sumeragi) fn into_publication_parts(
        self,
    ) -> (
        ExecutedDurableValidateDispatch,
        LifecycleValidateCompletionAckV1,
    ) {
        let Self { guarded, queue } = self;
        let (key, dispatch, drop_guard) = (*guarded).into_parts();
        (
            dispatch,
            LifecycleValidateCompletionAckV1 {
                key,
                queue,
                drop_guard,
            },
        )
    }
}
/// Armed owner of one completion-pending lifecycle Validate queue index.
#[must_use = "lifecycle Validate ownership must be acknowledged or restored"]
pub(in crate::sumeragi) struct LifecycleValidateCompletionAckV1 {
    key: LifecycleValidateDispatchKeyV1,
    queue: Arc<V2IoCommandQueue>,
    drop_guard: LifecycleValidateCompletionDropGuardV1,
}
impl LifecycleValidateCompletionAckV1 {
    /// Retire the exact command index only after registry/coordinator publication.
    pub(in crate::sumeragi) fn acknowledge_after_publication(mut self) {
        self.queue.acknowledge_lifecycle_validate(self.key);
        self.drop_guard.disarm();
    }

    /// Bind the still-armed owner to a deferred missing-sidecar dispatch.
    pub(in crate::sumeragi) fn bind_deferred(
        self,
        dispatch: DeferredDurableValidateDispatch,
    ) -> PreparedDeferredLifecycleValidateCompletionV1 {
        PreparedDeferredLifecycleValidateCompletionV1 {
            dispatch,
            ack: self,
        }
    }
}
/// Missing-sidecar Validate completion retained under its exact worker/publication owner.
#[must_use = "deferred lifecycle Validate must register and wake its exact row"]
pub(in crate::sumeragi) struct PreparedDeferredLifecycleValidateCompletionV1 {
    dispatch: DeferredDurableValidateDispatch,
    ack: LifecycleValidateCompletionAckV1,
}
impl PreparedDeferredLifecycleValidateCompletionV1 {
    /// Borrow the sealed address, dependency, wait source, owner, and
    /// generation that must be published as one durable sidecar registration.
    pub(in crate::sumeragi) fn sidecar_registration_identity(
        &self,
    ) -> Option<
        crate::sumeragi::v2_lifecycle_coordinator::LifecycleValidateSidecarRegistrationIdentityV1,
    > {
        self.dispatch.sidecar_registration_identity(self.ack.key)
    }

    /// Split the still-armed queue owner from its move-only deferred dispatch.
    /// This is consumed only after the exact Waiting row has become Ready.
    pub(in crate::sumeragi) fn into_sidecar_wake_parts(
        self,
    ) -> (
        DeferredDurableValidateDispatch,
        LifecycleValidateCompletionAckV1,
    ) {
        (self.dispatch, self.ack)
    }
}
/// Authority consumed by one successfully settled Certified-Serve completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "the Certified-Serve completion authority must be observed"]
pub(in crate::sumeragi) enum LifecycleCertifiedServeCompletionSettlementV1 {
    /// A live Certified-Serve lease reached its terminal and was released.
    Claimed,
    /// An already-terminal request was revalidated without owning a live lease.
    TerminalReplay,
}
impl PreparedLifecycleCertifiedServeCompletionV1 {
    fn new(
        guarded: Box<GuardedLifecycleCertifiedServeWorkerResultV1>,
        queue: Arc<V2IoCommandQueue>,
        ownership_position: usize,
    ) -> Option<Self> {
        let result = guarded.result();
        queue
            .transfer_lifecycle_certified_serve_completion(
                result.lifecycle_ordinal(),
                result.request_hash(),
                ownership_position,
            )
            .then_some(Self { guarded, queue })
    }

    /// Publish the LedgerV1 terminal, deliver the response, and retire its owner.
    pub(in crate::sumeragi) fn settle_deliver_and_acknowledge(
        mut self,
        owner: &mut crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleOwnerV1,
        services: &ProductionV2Services,
    ) -> Result<LifecycleCertifiedServeCompletionSettlementV1, String> {
        let settlement = {
            let result = self
                .guarded
                .result
                .as_mut()
                .expect("armed lifecycle Certified-Serve completion retains its result");
            let body_readback = result.body_readback.take().ok_or_else(|| {
                "lifecycle Certified-Serve completion lost its body readback".to_owned()
            })?;
            let authority = result.task.authority.take().ok_or_else(|| {
                "lifecycle Certified-Serve completion lost its terminal authority".to_owned()
            })?;
            match authority {
                LifecycleCertifiedServeTaskAuthorityV1::Claimed(lease) => {
                    owner
                        .settle_certified_serve_worker_completed(
                            lease,
                            &result.task.authenticated,
                            body_readback,
                            &result.response,
                        )
                        .map_err(|_| {
                            "lifecycle Certified-Serve terminal settlement failed".to_owned()
                        })?;
                    LifecycleCertifiedServeCompletionSettlementV1::Claimed
                }
                LifecycleCertifiedServeTaskAuthorityV1::TerminalReplay(authorization) => {
                    owner
                        .verify_certified_serve_terminal_replay(
                            authorization,
                            &result.task.authenticated,
                            body_readback,
                            &result.response,
                        )
                        .map_err(|_| {
                            "lifecycle Certified-Serve terminal replay verification failed"
                                .to_owned()
                        })?;
                    LifecycleCertifiedServeCompletionSettlementV1::TerminalReplay
                }
            }
        };
        let result = self.guarded.result();
        services.post_to_peer_on_reply_routes(
            result.task.recipient.clone(),
            result.task.reply_routes.clone(),
            result.task.ingress_ownership.clone(),
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::CertifiedBodyResponse(
                result.response.clone(),
            )),
        )?;
        self.queue.acknowledge_lifecycle_certified_serve(
            result.lifecycle_ordinal(),
            result.request_hash(),
        );
        let _ = (*self.guarded).into_result();
        Ok(settlement)
    }
}
impl PreparedRecoveredDecisionFetchBodyCompletionV1 {
    fn new(
        guarded: Box<GuardedRecoveredDecisionFetchBodyPersistenceCompletionV1>,
        queue: Arc<V2IoCommandQueue>,
        ownership_position: usize,
    ) -> Option<Self> {
        let key = guarded.completion().dispatch_key();
        let state_is_exact = {
            let state = queue.lock();
            state
                .recovered_decision_fetch_bodies
                .get(&key)
                .is_some_and(|tracked| {
                    tracked.state == V2IoWorkState::CompletionPending
                        && tracked.id == guarded.completion().id()
                        && tracked.response_hash == guarded.completion().response_hash()
                })
        };
        (state_is_exact
            && queue
                .admission
                .transfer_recovered_decision_fetch_completion_at(key, ownership_position))
        .then_some(Self { guarded, queue })
    }
    /// Borrow the opaque durable completion for fixed settlement projections.
    pub(in crate::sumeragi) fn completion(
        &self,
    ) -> &RecoveredDecisionFetchBodyPersistenceCompletionV1 {
        self.guarded.completion()
    }
    /// Retire the exact command index and disarm restart closure after LedgerV1 publication.
    pub(in crate::sumeragi) fn acknowledge_after_publication(self) {
        let key = self.guarded.completion().dispatch_key();
        let id = self.guarded.completion().id();
        let response_hash = self.guarded.completion().response_hash();
        self.queue
            .acknowledge_recovered_decision_fetch_body(key, id, response_hash);
        self.guarded.acknowledge_after_publication();
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl PreparedRecoveredLifecycleSignCompletionV1 {
    fn new(
        guarded: Box<GuardedRecoveredLifecycleSignWorkerResultV1>,
        queue: Arc<V2IoCommandQueue>,
        ownership_position: usize,
    ) -> Option<Self> {
        queue
            .transfer_recovered_lifecycle_sign_completion(
                guarded.result().dispatch_key(),
                ownership_position,
            )
            .then_some(Self { guarded, queue })
    }
    /// Clone a revalidated opaque result for private adapter preview while the
    /// original remains guarded until LedgerV1 publication.
    pub(in crate::sumeragi) fn project_adapter_completion_authority(
        &self,
    ) -> Option<RecoveredLifecycleSignAdapterCompletionAuthorityV1> {
        let result = self.guarded.result();
        if !result.is_exact() {
            return None;
        }
        Some(RecoveredLifecycleSignAdapterCompletionAuthorityV1 {
            key: result.dispatch_key(),
            tag: result.task.tag,
            request: result.task.request.clone(),
            signature: result.signature.clone(),
            outbound_payload: result.outbound_payload.clone(),
        })
    }
    /// Return the exact dedicated-queue and registry dispatch identity.
    pub(in crate::sumeragi) fn dispatch_key(&self) -> RecoveredLifecycleSignDispatchKeyV1 {
        self.guarded.result().dispatch_key()
    }
    /// Retire the command owner after durable Broadcast publication and all
    /// volatile assertion-only tails, then disarm restart closure.
    pub(in crate::sumeragi) fn acknowledge_after_publication(self) {
        let key = self.guarded.result().dispatch_key();
        self.queue.acknowledge_recovered_lifecycle_sign(key);
        self.guarded.acknowledge_after_publication();
    }
}
/// Result of atomically returning one guarded missing-sidecar Apply to the worker FIFO.
#[must_use = "an unavailable lifecycle Decision Apply retry still owns its guarded completion"]
pub(in crate::sumeragi) enum LifecycleDecisionApplyDeferredRetryV1 {
    /// The same dispatch key and task were republished to the dedicated worker queue.
    Requeued,
    /// Consensus queue capacity is unavailable; the complete guarded result remains owned.
    Unavailable(PreparedLifecycleDecisionApplyCompletionV1),
    /// The dedicated queue index no longer matched the retained completion.
    RestartRequired,
}
impl PreparedLifecycleDecisionApplyCompletionV1 {
    /// Compare service queue, output guard, and recovery owner without releasing
    /// guarded completion or process-local dependencies.
    pub(in crate::sumeragi) fn authorizes_sidecar_owner(
        &self,
        services: &ProductionV2Services,
        lane_work: &V2LaneWorkAdapter,
    ) -> bool {
        services.owns_lifecycle_decision_apply_queue(&self.work_ack.queue)
            && Arc::ptr_eq(&services.output_guard, &self.work_ack.output_guard)
            && services.matches_lifecycle_lane_work(lane_work)
    }
    /// Borrow the exact Applied/Deferred result while the command remains indexed.
    pub(in crate::sumeragi) fn result(&self) -> &LifecycleDecisionApplyWorkerResultV1 {
        self.guarded.result()
    }
    /// Release the dedicated queue index after the owner durably settled this result.
    ///
    /// This is intentionally not a generic worker acknowledgement: its only
    /// caller is the lifecycle Decision Apply owner settlement transaction.
    pub(in crate::sumeragi) fn acknowledge_after_owner_settlement(
        self,
    ) -> LifecycleDecisionApplyWorkerResultV1 {
        let Self { guarded, work_ack } = self;
        work_ack.acknowledge();
        (*guarded).into_result()
    }
    /// Republish a `CompletionPending` sidecar task under its existing owner,
    /// reserving/enqueueing before disarming guards; mismatch requires restart.
    #[allow(clippy::result_large_err)]
    pub(in crate::sumeragi) fn retry_deferred(self) -> LifecycleDecisionApplyDeferredRetryV1 {
        let Self { guarded, work_ack } = self;
        let (result, mut completion_guard) = (*guarded).into_retry_parts();
        let LifecycleDecisionApplyWorkerResultV1::Deferred { task, reference } = result else {
            drop(work_ack);
            drop(completion_guard);
            return LifecycleDecisionApplyDeferredRetryV1::RestartRequired;
        };
        match work_ack.queue.retry_lifecycle_decision_apply(task) {
            Ok(()) => {
                work_ack.acknowledge_retry_publication();
                completion_guard.disarm();
                LifecycleDecisionApplyDeferredRetryV1::Requeued
            }
            Err(LifecycleDecisionApplyRetryQueueErrorV1::Unavailable(task)) => {
                LifecycleDecisionApplyDeferredRetryV1::Unavailable(Self {
                    guarded: Box::new(
                        GuardedLifecycleDecisionApplyWorkerResultV1::from_retry_parts(
                            LifecycleDecisionApplyWorkerResultV1::Deferred { task, reference },
                            completion_guard,
                        ),
                    ),
                    work_ack,
                })
            }
            Err(LifecycleDecisionApplyRetryQueueErrorV1::InvalidOwner(_task)) => {
                drop(work_ack);
                drop(completion_guard);
                LifecycleDecisionApplyDeferredRetryV1::RestartRequired
            }
        }
    }
}
struct LifecycleValidateCompletionDropGuardV1 {
    output_guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
impl LifecycleValidateCompletionDropGuardV1 {
    fn new(output_guard: Arc<ConsensusOutputGuard>) -> Self {
        Self {
            output_guard,
            armed: true,
        }
    }
    fn disarm(&mut self) {
        self.armed = false;
    }
}
impl Drop for LifecycleValidateCompletionDropGuardV1 {
    fn drop(&mut self) {
        if self.armed {
            self.output_guard.close_admission_for_restart();
        }
    }
}
struct GuardedLifecycleValidateWorkerResultV1 {
    key: LifecycleValidateDispatchKeyV1,
    result: Option<ExecutedDurableValidateDispatch>,
    drop_guard: LifecycleValidateCompletionDropGuardV1,
}
impl GuardedLifecycleValidateWorkerResultV1 {
    fn new(
        key: LifecycleValidateDispatchKeyV1,
        result: ExecutedDurableValidateDispatch,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Self {
        Self {
            key,
            result: Some(result),
            drop_guard: LifecycleValidateCompletionDropGuardV1::new(output_guard),
        }
    }
    const fn key(&self) -> LifecycleValidateDispatchKeyV1 {
        self.key
    }
    fn result(&self) -> &ExecutedDurableValidateDispatch {
        self.result
            .as_ref()
            .expect("armed lifecycle Validate completion retains its dispatch")
    }
    fn into_parts(
        mut self,
    ) -> (
        LifecycleValidateDispatchKeyV1,
        ExecutedDurableValidateDispatch,
        LifecycleValidateCompletionDropGuardV1,
    ) {
        let result = self
            .result
            .take()
            .expect("lifecycle Validate dispatch is consumed exactly once");
        (self.key, result, self.drop_guard)
    }
}
struct LifecycleCertifiedServeCompletionDropGuardV1 {
    output_guard: Arc<ConsensusOutputGuard>,
    armed: bool,
}
impl LifecycleCertifiedServeCompletionDropGuardV1 {
    fn new(output_guard: Arc<ConsensusOutputGuard>) -> Self {
        Self {
            output_guard,
            armed: true,
        }
    }
    fn disarm(&mut self) {
        self.armed = false;
    }
}
impl Drop for LifecycleCertifiedServeCompletionDropGuardV1 {
    fn drop(&mut self) {
        if self.armed {
            self.output_guard.close_admission_for_restart();
        }
    }
}
struct GuardedLifecycleCertifiedServeWorkerResultV1 {
    result: Option<LifecycleCertifiedServeWorkerResultV1>,
    drop_guard: LifecycleCertifiedServeCompletionDropGuardV1,
}
impl GuardedLifecycleCertifiedServeWorkerResultV1 {
    fn new(
        result: LifecycleCertifiedServeWorkerResultV1,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Self {
        Self {
            result: Some(result),
            drop_guard: LifecycleCertifiedServeCompletionDropGuardV1::new(output_guard),
        }
    }
    fn result(&self) -> &LifecycleCertifiedServeWorkerResultV1 {
        self.result
            .as_ref()
            .expect("armed lifecycle Certified-Serve completion retains its result")
    }
    fn into_result(mut self) -> LifecycleCertifiedServeWorkerResultV1 {
        let result = self
            .result
            .take()
            .expect("settled lifecycle Certified-Serve consumes its result once");
        self.drop_guard.disarm();
        result
    }
}
enum V2IoCompletion {
    Signature {
        work_id: EffectWorkId,
        signature: Vec<u8>,
        outbound_payload: Option<EncodedV2Payload>,
    },
    Stored(BodyStoreCompletion),
    CertifiedFetchBodyPersisted(GuardedCertifiedFetchBodyPersistenceCompletion),
    RecoveredDecisionFetchBodyPersisted(
        Box<GuardedRecoveredDecisionFetchBodyPersistenceCompletionV1>,
    ),
    Applied(Box<DurableApplyCompletion>),
    LifecycleDecisionApply(Box<GuardedLifecycleDecisionApplyWorkerResultV1>),
    RecoveredLifecycleSign(Box<GuardedRecoveredLifecycleSignWorkerResultV1>),
    LifecycleValidate(Box<GuardedLifecycleValidateWorkerResultV1>),
    LifecycleCertifiedServe(Box<GuardedLifecycleCertifiedServeWorkerResultV1>),
    ApplyDeferred {
        work_id: EffectWorkId,
        reference: CertifiedMergeLedgerReference,
    },
    #[cfg(test)]
    AuxiliaryNoop,
    CandidateLoaded(LockedCandidateLoad),
    CandidateLoadUnavailable {
        acquisition_id: LockedCandidateAcquisitionId,
        subject: wire::BlockSubject,
    },
    CandidateLoadFailed {
        acquisition_id: LockedCandidateAcquisitionId,
        subject: wire::BlockSubject,
        reason: String,
    },
    Retired,
    RetirementFailed(String),
    RecoveryRequired(String),
    Failed(String),
}
impl V2IoCompletion {
    fn lifecycle_decision_apply_key(&self) -> Option<LifecycleDecisionApplyDispatchKeyV1> {
        match self {
            Self::LifecycleDecisionApply(guarded) => Some(guarded.result().dispatch_key()),
            _ => None,
        }
    }
    fn recovered_lifecycle_sign_key(&self) -> Option<RecoveredLifecycleSignDispatchKeyV1> {
        match self {
            Self::RecoveredLifecycleSign(guarded) => Some(guarded.result().dispatch_key()),
            _ => None,
        }
    }
    fn recovered_decision_fetch_key(&self) -> Option<RecoveredDecisionFetchDispatchKeyV1> {
        match self {
            Self::RecoveredDecisionFetchBodyPersisted(guarded) => {
                Some(guarded.completion().dispatch_key())
            }
            _ => None,
        }
    }
    fn lifecycle_validate_key(&self) -> Option<LifecycleValidateDispatchKeyV1> {
        match self {
            Self::LifecycleValidate(guarded) => Some(guarded.key()),
            _ => None,
        }
    }
    fn lifecycle_certified_serve_ordinal(&self) -> Option<u128> {
        match self {
            Self::LifecycleCertifiedServe(guarded) => Some(guarded.result().lifecycle_ordinal()),
            _ => None,
        }
    }
    // `false` variants never enqueue a reducer completion. They operate only
    // on non-reducer effect, network, or service state (or report a terminal
    // failure), so they may be serviced behind one retained runtime result
    // without reordering any reducer-visible completion.
    const fn requires_runtime_capacity(&self) -> bool {
        matches!(
            self,
            Self::Signature { .. }
                | Self::Stored(_)
                | Self::Applied(_)
                | Self::LifecycleDecisionApply(_)
                | Self::RecoveredLifecycleSign(_)
        )
    }
    fn acknowledgement(&self) -> V2IoCompletionAcknowledgement {
        match self {
            Self::Signature { work_id, .. } | Self::ApplyDeferred { work_id, .. } => {
                V2IoCompletionAcknowledgement::Work(*work_id)
            }
            Self::Stored(completion) => V2IoCompletionAcknowledgement::Work(completion.work_id()),
            Self::CertifiedFetchBodyPersisted(_) => {
                V2IoCompletionAcknowledgement::LifecycleWorkRetained
            }
            Self::RecoveredDecisionFetchBodyPersisted(_) => {
                V2IoCompletionAcknowledgement::RecoveredDecisionFetchRetained
            }
            Self::Applied(completion) => V2IoCompletionAcknowledgement::Work(completion.work_id()),
            Self::LifecycleDecisionApply(_) => {
                V2IoCompletionAcknowledgement::LifecycleDecisionApplyRetained
            }
            Self::RecoveredLifecycleSign(_) => {
                V2IoCompletionAcknowledgement::RecoveredLifecycleSignRetained
            }
            Self::LifecycleValidate(_) => V2IoCompletionAcknowledgement::LifecycleValidateRetained,
            Self::LifecycleCertifiedServe(_) => {
                V2IoCompletionAcknowledgement::LifecycleServeRetained
            }
            Self::CandidateLoaded(_)
            | Self::CandidateLoadUnavailable { .. }
            | Self::CandidateLoadFailed { .. }
            | Self::Retired
            | Self::RetirementFailed(_)
            | Self::RecoveryRequired(_)
            | Self::Failed(_) => V2IoCompletionAcknowledgement::Untracked,
            #[cfg(test)]
            Self::AuxiliaryNoop => V2IoCompletionAcknowledgement::Untracked,
        }
    }
}
enum V2IoCompletionAcknowledgement {
    Work(EffectWorkId),
    LifecycleWorkRetained,
    LifecycleDecisionApplyRetained,
    RecoveredLifecycleSignRetained,
    RecoveredDecisionFetchRetained,
    LifecycleValidateRetained,
    LifecycleServeRetained,
    Untracked,
}
/// Move-only persistence acknowledgement retaining `CompletionPending` work so
/// repeated selector probes coalesce until Phase B consumes ingress.
#[must_use = "the exact command index must remain occupied until Phase B commits"]
pub(in crate::sumeragi) struct CertifiedFetchBodyPersistenceWorkAck {
    queue: Arc<V2IoCommandQueue>,
    output_guard: Arc<ConsensusOutputGuard>,
    work_id: EffectWorkId,
    descriptor: V2IoWorkDescriptor,
    armed: bool,
}
impl CertifiedFetchBodyPersistenceWorkAck {
    /// Release the exact command index only in the post-dequeue infallible tail.
    pub(in crate::sumeragi) fn commit(mut self) {
        self.queue
            .acknowledge_exact_lifecycle_completion(self.work_id, &self.descriptor);
        self.armed = false;
    }
}
impl Drop for CertifiedFetchBodyPersistenceWorkAck {
    fn drop(&mut self) {
        if self.armed {
            self.output_guard.close_admission_for_restart();
        }
    }
}
/// Persisted body plus its still-indexed exact command owner.
#[must_use = "the persisted response and duplicate fence require Phase-B consumption"]
pub(crate) struct PreparedCertifiedFetchBodyPersistenceCompletion {
    completion: CertifiedFetchBodyPersistenceCompletion,
    work_ack: CertifiedFetchBodyPersistenceWorkAck,
}
impl PreparedCertifiedFetchBodyPersistenceCompletion {
    /// Return the still-indexed existing executor work identity for diagnostics.
    pub(in crate::sumeragi) const fn work_id(&self) -> EffectWorkId {
        self.completion.work_id()
    }
    /// Split two opaque move-only authorities for the sealed composite transaction.
    pub(in crate::sumeragi) fn into_parts(
        self,
    ) -> (
        CertifiedFetchBodyPersistenceCompletion,
        CertifiedFetchBodyPersistenceWorkAck,
    ) {
        (self.completion, self.work_ack)
    }
    /// Rejoin an unchanged pre-dequeue completion after a retryable failure.
    pub(in crate::sumeragi) fn from_parts(
        completion: CertifiedFetchBodyPersistenceCompletion,
        work_ack: CertifiedFetchBodyPersistenceWorkAck,
    ) -> Self {
        Self {
            completion,
            work_ack,
        }
    }
}
/// Typed outcome of the ordinary bounded completion drain.
///
/// A persisted certified-Fetch body is returned directly to the serialized
/// caller; it is never parked in a service-side flag, latch, or second queue.
#[must_use = "a persisted certified-Fetch body must be consumed by its coordinator owner"]
pub(crate) struct V2CompletionDrainOutcome {
    serviced: usize,
    certified_fetch_body: Option<PreparedCertifiedFetchBodyPersistenceCompletion>,
}
impl V2CompletionDrainOutcome {
    /// Split the count from the move-only lifecycle completion.
    pub(crate) fn into_parts(
        self,
    ) -> (
        usize,
        Option<PreparedCertifiedFetchBodyPersistenceCompletion>,
    ) {
        (self.serviced, self.certified_fetch_body)
    }
}
/// Owner-only drain of at most one guarded recovered Sign completion.
#[must_use = "a recovered Sign drain remains parked under its lifecycle owner"]
pub(in crate::sumeragi) struct RecoveredLifecycleSignCompletionDrainV1 {
    completion: Option<PreparedRecoveredLifecycleSignCompletionV1>,
}
/// Owner-only drain of at most one lifecycle-owned Certified-Serve completion.
#[must_use = "a Certified-Serve completion must remain parked under its lifecycle owner"]
pub(in crate::sumeragi) struct LifecycleCertifiedServeCompletionDrainV1 {
    completion: Option<PreparedLifecycleCertifiedServeCompletionV1>,
}

/// Opaque result of taking the physical completion head exactly once.
///
/// Non-lifecycle I/O and local reconstruction work is never exposed. Such a
/// head is restored into the service's sole held slot before `PassThrough`
/// returns, so the ordinary drain observes the same FIFO item. Lifecycle
/// variants transfer only their guarded, class-specific owner.
#[allow(variant_size_differences)]
#[must_use = "a selected lifecycle completion must remain lifecycle-owned"]
pub(in crate::sumeragi) enum LifecycleCompletionTakeV1 {
    /// No physical I/O completion is currently available.
    None,
    /// The ordinary completion owner must service the current turn.
    PassThrough,
    /// The exact persisted ordinary certified-Fetch body left the FIFO owner.
    CertifiedFetch(PreparedCertifiedFetchBodyPersistenceCompletion),
    /// The exact lifecycle Decision Apply completion left the FIFO owner.
    Apply(PreparedLifecycleDecisionApplyCompletionV1),
    /// The exact recovered Sign completion left the FIFO owner.
    Sign(PreparedRecoveredLifecycleSignCompletionV1),
    /// The exact persisted recovered Decision Fetch body left the FIFO owner.
    DecisionFetch(PreparedRecoveredDecisionFetchBodyCompletionV1),
    /// The exact lifecycle Validate result left the FIFO owner.
    Validate(PreparedLifecycleValidateCompletionV1),
    /// The exact lifecycle-owned Serve result left the FIFO owner.
    CertifiedServe(PreparedLifecycleCertifiedServeCompletionV1),
}

impl LifecycleCertifiedServeCompletionDrainV1 {
    /// Consume the drain result into its optional guarded Serve completion.
    pub(in crate::sumeragi) fn into_completion(
        self,
    ) -> Option<PreparedLifecycleCertifiedServeCompletionV1> {
        self.completion
    }
}
impl RecoveredLifecycleSignCompletionDrainV1 {
    /// Consume the drain into its optional opaque guarded completion.
    pub(in crate::sumeragi) fn into_completion(
        self,
    ) -> Option<PreparedRecoveredLifecycleSignCompletionV1> {
        self.completion
    }
}
struct V2IoHandle {
    command_tx: V2IoCommandSender,
    completion_rx: mpsc::Receiver<V2IoCompletion>,
    join: Option<thread::JoinHandle<()>>,
    allow_finalized_disconnect: Arc<AtomicBool>,
    admission: Arc<V2IoAdmission>,
}
struct V2IoWorkerFailureGuard {
    output_guard: Arc<ConsensusOutputGuard>,
    allow_finalized_disconnect: Arc<AtomicBool>,
    armed: bool,
}
impl V2IoWorkerFailureGuard {
    fn new(
        output_guard: Arc<ConsensusOutputGuard>,
        allow_finalized_disconnect: Arc<AtomicBool>,
    ) -> Self {
        Self {
            output_guard,
            allow_finalized_disconnect,
            armed: true,
        }
    }
    fn disarm(&mut self) {
        self.armed = false;
    }
}
impl Drop for V2IoWorkerFailureGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        if thread::panicking() {
            self.output_guard.close_admission_for_restart();
        } else if !self
            .allow_finalized_disconnect
            .load(AtomicOrdering::Acquire)
        {
            self.output_guard.activate_restart_required();
        }
    }
}
#[derive(Clone, Copy, Debug)]
struct CleanupWorkerIdentity {
    height: u64,
    context_id: wire::HeightContextId,
    block_hash: HashOf<iroha_data_model::block::BlockHeader>,
}
impl CleanupWorkerIdentity {
    fn from_receipt(receipt: &KuraV2CommitReceipt) -> Self {
        Self {
            height: receipt.height(),
            context_id: receipt.context_id(),
            block_hash: receipt.block_hash(),
        }
    }
}
struct PostFinalityCleanupJob {
    identity: CleanupWorkerIdentity,
    bodies: V2BodyRetirementJob,
    chunk_root: PathBuf,
}
const POST_FINALITY_CLEANUP_QUEUE_CAPACITY: usize = 4;
#[derive(Clone)]
struct V2CleanupSubmission {
    sender: mpsc::SyncSender<PostFinalityCleanupJob>,
}
impl V2CleanupSubmission {
    fn try_submit(&self, job: PostFinalityCleanupJob) -> Result<(), String> {
        let identity = job.identity;
        match self.sender.try_send(job) {
            Ok(()) => Ok(()),
            Err(mpsc::TrySendError::Full(_)) => {
                let reason =
                    "bounded Sumeragi v2 cleanup queue is full; retaining finalized local files";
                report_post_finality_cleanup_warning(
                    identity,
                    PostFinalityCleanupTarget::CleanupWorker,
                    reason,
                );
                Err(reason.to_owned())
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                let reason =
                    "Sumeragi v2 cleanup worker is unavailable; retaining finalized local files";
                report_post_finality_cleanup_warning(
                    identity,
                    PostFinalityCleanupTarget::CleanupWorker,
                    reason,
                );
                Err(reason.to_owned())
            }
        }
    }
}
/// Runner-owned cleanup janitor: consensus only uses bounded non-blocking
/// enqueue, and stalled work remains for startup reconciliation.
pub(crate) struct V2CleanupSupervisor {
    submission: Option<V2CleanupSubmission>,
    join: Option<thread::JoinHandle<()>>,
}
impl Default for V2CleanupSupervisor {
    fn default() -> Self {
        Self::with_capacity(
            NonZeroUsize::new(POST_FINALITY_CLEANUP_QUEUE_CAPACITY)
                .expect("cleanup queue capacity is non-zero"),
        )
    }
}
impl V2CleanupSupervisor {
    fn with_capacity(capacity: NonZeroUsize) -> Self {
        let (sender, receiver) = mpsc::sync_channel(capacity.get());
        let submission = V2CleanupSubmission { sender };
        let join = match super::sumeragi_thread_builder("sumeragi-v2-cleanup").spawn(move || {
            while let Ok(job) = receiver.recv() {
                execute_post_finality_cleanup(job);
            }
        }) {
            Ok(join) => Some(join),
            Err(error) => {
                iroha_logger::warn!(
                    cleanup_target = PostFinalityCleanupTarget::CleanupWorker.as_str(),
                    reason = %error,
                    "failed to start the bounded Sumeragi v2 cleanup worker"
                );
                None
            }
        };
        Self {
            submission: Some(submission),
            join,
        }
    }
    fn submission(&self) -> V2CleanupSubmission {
        self.submission
            .as_ref()
            .expect("cleanup submission exists until supervisor drop")
            .clone()
    }
    /// Reap a terminated janitor without ever joining a running thread.
    pub(crate) fn reap_finished(&mut self) {
        if self
            .join
            .as_ref()
            .is_some_and(thread::JoinHandle::is_finished)
        {
            let join = self.join.take().expect("finished cleanup worker exists");
            if join.join().is_err() {
                iroha_logger::warn!(
                    cleanup_target = PostFinalityCleanupTarget::CleanupWorker.as_str(),
                    reason = "bounded Sumeragi v2 cleanup worker panicked",
                    "Sumeragi v2 finalized with retained local cleanup state"
                );
            }
        }
    }
}
impl Drop for V2CleanupSupervisor {
    fn drop(&mut self) {
        self.submission.take();
        if self
            .join
            .as_ref()
            .is_some_and(thread::JoinHandle::is_finished)
        {
            let join = self.join.take().expect("finished cleanup worker exists");
            let _ = join.join();
        }
    }
}
fn execute_post_finality_cleanup(job: PostFinalityCleanupJob) {
    if let Err(error) = job.bodies.execute() {
        report_post_finality_cleanup_warning(
            job.identity,
            PostFinalityCleanupTarget::DurableBodies,
            &error.to_string(),
        );
    }
    match std::fs::remove_dir_all(&job.chunk_root) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => report_post_finality_cleanup_warning(
            job.identity,
            PostFinalityCleanupTarget::PayloadChunks,
            &format!(
                "failed to remove Sumeragi v2 chunk root {}: {error}",
                job.chunk_root.display()
            ),
        ),
    }
}
fn report_post_finality_cleanup_warning(
    identity: CleanupWorkerIdentity,
    target: PostFinalityCleanupTarget,
    reason: &str,
) {
    iroha_logger::warn!(
        height = identity.height,
        context_id = ?identity.context_id,
        block_hash = %identity.block_hash,
        cleanup_target = target.as_str(),
        reason,
        "Sumeragi v2 finalized with retained local cleanup state"
    );
}
impl V2IoHandle {
    fn spawn(
        body_store: V2BodyStore,
        apply_service: V2ApplyService,
        context: wire::HeightContext,
        key_pair: KeyPair,
        local_validator: Option<wire::ValidatorIndex>,
        auxiliary_queue_capacity: usize,
        consensus_queue_capacity: usize,
        observer_serve_capacity: usize,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Result<Self, String> {
        let admission = Arc::new(V2IoAdmission::new(
            auxiliary_queue_capacity,
            consensus_queue_capacity,
        )?);
        let capacity = admission.capacity();
        if observer_serve_capacity == 0 {
            return Err("Sumeragi v2 observer Serve capacity must be non-zero".to_owned());
        }
        let (command_tx, command_rx) =
            build_v2_io_command_channel(capacity, Arc::clone(&admission));
        let (completion_tx, completion_rx) = mpsc::sync_channel(capacity);
        let allow_finalized_disconnect = Arc::new(AtomicBool::new(false));
        let worker_allow_finalized_disconnect = Arc::clone(&allow_finalized_disconnect);
        let worker_admission = Arc::clone(&admission);
        let join = super::sumeragi_thread_builder("sumeragi-v2-io")
            .spawn(move || {
                // A local guard drops before the closure environment releases
                // command/completion channels, closing output first on panic
                // or an implicit producer disconnect.
                let mut worker_failure_guard = V2IoWorkerFailureGuard::new(
                    Arc::clone(&output_guard),
                    worker_allow_finalized_disconnect,
                );
                let mut body_store = Some(body_store);
                while let Ok(command) = command_rx.recv() {
                    let work_id = command.work_id();
                    let lifecycle_decision_apply_key = command.lifecycle_decision_apply_key();
                    let recovered_lifecycle_sign_key = command.recovered_lifecycle_sign_key();
                    let recovered_decision_fetch_key = command.recovered_decision_fetch_key();
                    let lifecycle_validate_key = command.lifecycle_validate_key();
                    let runtime_lifecycle_ordinal = command.runtime_lifecycle_ordinal();
                    match command {
                        V2IoCommand::Retire(retire) => {
                            let Some(completion) = execute_retire_io_command(&output_guard, || {
                                let bodies = body_store
                                    .take()
                                    .expect("Retire consumes the live height-local body store")
                                    .into_retirement_job(&retire.receipt)
                                    .map_err(|error| error.to_string())?;
                                retire.cleanup.try_submit(PostFinalityCleanupJob {
                                    identity: CleanupWorkerIdentity::from_receipt(&retire.receipt),
                                    bodies,
                                    chunk_root: retire.chunk_root,
                                })
                            }) else {
                                break;
                            };
                            let _ = send_tracked_completion(
                                &completion_tx,
                                &worker_admission,
                                completion,
                            );
                            worker_failure_guard.disarm();
                            break;
                        }
                        V2IoCommand::Shutdown => {
                            worker_failure_guard.disarm();
                            break;
                        }
                        V2IoCommand::LoadCandidate {
                            acquisition_id,
                            subject,
                        } => {
                            let completion = match load_candidate_body(
                                body_store
                                    .as_ref()
                                    .expect("body store remains live before Retire"),
                                acquisition_id,
                                subject,
                            ) {
                                Ok(Some(loaded)) => V2IoCompletion::CandidateLoaded(loaded),
                                Ok(None) => V2IoCompletion::CandidateLoadUnavailable {
                                    acquisition_id,
                                    subject,
                                },
                                Err(reason) => V2IoCompletion::CandidateLoadFailed {
                                    acquisition_id,
                                    subject,
                                    reason,
                                },
                            };
                            send_completion(&completion_tx, &worker_admission, Ok(completion));
                        }
                        command => {
                            let completion = execute_fail_stop_io_command(&output_guard, || {
                                match command {
                                    V2IoCommand::Sign {
                                        task,
                                        restore_outbound_payload,
                                    } => sign_consensus_task(
                                        body_store
                                            .as_ref()
                                            .expect("body store remains live before Retire"),
                                        &context,
                                        &key_pair,
                                        task,
                                        restore_outbound_payload,
                                    ),
                                    V2IoCommand::Store(task) => body_store
                                        .as_mut()
                                        .expect("body store remains live before Retire")
                                        .execute_store_task(&task)
                                        .map(V2IoCompletion::Stored)
                                        .map_err(|error| error.to_string()),
                                    V2IoCommand::PersistCertifiedFetchBody(task) => task
                                        .persist(
                                            body_store
                                                .as_mut()
                                                .expect("body store remains live before Retire"),
                                        )
                                        .map(|completion| {
                                            V2IoCompletion::CertifiedFetchBodyPersisted(
                                                GuardedCertifiedFetchBodyPersistenceCompletion::new(
                                                    completion,
                                                    Arc::clone(&output_guard),
                                                ),
                                            )
                                        })
                                        .map_err(|(error, _task)| error.to_string()),
                                    V2IoCommand::PersistRecoveredDecisionFetchBody(task) => task
                                        .persist(
                                            body_store
                                                .as_mut()
                                                .expect("body store remains live before Retire"),
                                        )
                                        .map(|completion| {
                                            V2IoCompletion::RecoveredDecisionFetchBodyPersisted(
                                                Box::new(
                                                    GuardedRecoveredDecisionFetchBodyPersistenceCompletionV1::new(
                                                        completion,
                                                        Arc::clone(&output_guard),
                                                    ),
                                                ),
                                            )
                                        })
                                        .map_err(|(error, _task)| error.to_string()),
                                    V2IoCommand::LifecycleValidate(task) => {
                                        if !task.matches_exact() {
                                            Err("lifecycle Validate command changed after queue publication"
                                                .to_owned())
                                        } else {
                                            let key = task.key;
                                            task.dispatch
                                                .execute(
                                                    body_store.as_mut().expect(
                                                        "body store remains live before Retire",
                                                    ),
                                                    |body| {
                                                        apply_service
                                                            .validate_candidate(&context, body)
                                                    },
                                                )
                                                .map(|result| {
                                                    V2IoCompletion::LifecycleValidate(Box::new(
                                                        GuardedLifecycleValidateWorkerResultV1::new(
                                                            key,
                                                            result,
                                                            Arc::clone(&output_guard),
                                                        ),
                                                    ))
                                                })
                                                .map_err(|(error, _dispatch)| error.to_string())
                                        }
                                    }
                                    V2IoCommand::Apply(task) => match apply_service.execute(
                                        &context,
                                        body_store
                                            .as_mut()
                                            .expect("body store remains live before Retire"),
                                        &task,
                                    ) {
                                        Ok(completion) => {
                                            Ok(V2IoCompletion::Applied(Box::new(completion)))
                                        }
                                        Err(
                                            super::v2_apply::V2ApplyError::MissingCertifiedMergeSidecar {
                                                reference,
                                            },
                                        ) => Ok(V2IoCompletion::ApplyDeferred {
                                            work_id: task.id(),
                                            reference,
                                        }),
                                        Err(error) if error.requires_restart_recovery() => {
                                            Ok(V2IoCompletion::RecoveryRequired(error.to_string()))
                                        }
                                        Err(error) => Err(error.to_string()),
                                    },
                                    V2IoCommand::LifecycleDecisionApply(task) => apply_service
                                        .execute_lifecycle_decision_apply(
                                            &context,
                                            body_store
                                                .as_mut()
                                                .expect("body store remains live before Retire"),
                                            task,
                                        )
                                        .map(|result| {
                                            V2IoCompletion::LifecycleDecisionApply(
                                                Box::new(GuardedLifecycleDecisionApplyWorkerResultV1::new(
                                                    result,
                                                    Arc::clone(&output_guard),
                                                )),
                                            )
                                        })
                                        .or_else(|error| {
                                            if error.requires_restart_recovery() {
                                                Ok(V2IoCompletion::RecoveryRequired(
                                                    error.to_string(),
                                                ))
                                            } else {
                                                Err(error.to_string())
                                            }
                                        }),
                                    V2IoCommand::RecoveredLifecycleSign(task) => {
                                        sign_recovered_lifecycle_task(
                                            body_store
                                                .as_ref()
                                                .expect("body store remains live before Retire"),
                                            &context,
                                            &key_pair,
                                            task,
                                        )
                                        .map(|result| {
                                            V2IoCompletion::RecoveredLifecycleSign(Box::new(
                                                GuardedRecoveredLifecycleSignWorkerResultV1::new(
                                                    result,
                                                    Arc::clone(&output_guard),
                                                ),
                                            ))
                                        })
                                    }
                                    V2IoCommand::LifecycleCertifiedServe(task) => {
                                        serve_lifecycle_certified_body(
                                            body_store
                                                .as_ref()
                                                .expect("body store remains live before Retire"),
                                            &key_pair,
                                            local_validator,
                                            task,
                                        )
                                        .map(|result| {
                                            V2IoCompletion::LifecycleCertifiedServe(Box::new(
                                                GuardedLifecycleCertifiedServeWorkerResultV1::new(
                                                    result,
                                                    Arc::clone(&output_guard),
                                                ),
                                            ))
                                        })
                                    }
                                    V2IoCommand::LoadCandidate { .. }
                                    | V2IoCommand::Retire(_)
                                    | V2IoCommand::Shutdown => {
                                        unreachable!(
                                            "cleanup commands handled before fail-stop I/O"
                                        )
                                    }
                                    #[cfg(test)]
                                    V2IoCommand::LifecycleDecisionApplyFixture(_) => {
                                        unreachable!(
                                            "lifecycle Decision Apply queue fixtures never enter a worker"
                                        )
                                    }
                                }
                            });
                            let failed = match completion {
                                Err(reason) => {
                                    if let Some(work_id) = work_id {
                                        command_rx.complete_work(work_id);
                                    }
                                    if let Some(key) = lifecycle_validate_key {
                                        command_rx.complete_lifecycle_validate_failure(key);
                                    }
                                    let _ = try_send_tracked_completion(
                                        &completion_tx,
                                        &worker_admission,
                                        V2IoCompletion::RecoveryRequired(reason.clone()),
                                    );
                                    true
                                }
                                Ok(completion) => {
                                    if let Some(work_id) = work_id {
                                        command_rx.complete_work(work_id);
                                    }
                                    let seal_result = match &completion {
                                        V2IoCompletion::LifecycleDecisionApply(guarded) => {
                                            lifecycle_decision_apply_key.map_or_else(
                                                || {
                                                    Err("lifecycle Decision Apply completion lost its command key"
                                                        .to_owned())
                                                },
                                                |key| {
                                                    command_rx
                                                        .complete_lifecycle_decision_apply(
                                                            key,
                                                            guarded.result(),
                                                        )
                                                        .map(|()| true)
                                                },
                                            )
                                        }
                                        V2IoCompletion::RecoveredLifecycleSign(guarded) => {
                                            recovered_lifecycle_sign_key.map_or_else(
                                                || {
                                                    Err("recovered Sign completion lost its command key"
                                                        .to_owned())
                                                },
                                                |key| {
                                                    command_rx
                                                        .complete_recovered_lifecycle_sign(
                                                            key,
                                                            guarded.result(),
                                                        )
                                                        .map(|()| true)
                                                },
                                            )
                                        }
                                        V2IoCompletion::RecoveredDecisionFetchBodyPersisted(
                                            guarded,
                                        ) => recovered_decision_fetch_key.map_or_else(
                                            || {
                                                Err("recovered Decision Fetch body completion lost its command key"
                                                    .to_owned())
                                            },
                                            |key| {
                                                command_rx
                                                    .complete_recovered_decision_fetch_body(
                                                        key,
                                                        guarded.completion(),
                                                    )
                                                    .map(|()| true)
                                            },
                                        ),
                                        V2IoCompletion::LifecycleValidate(guarded) => {
                                            lifecycle_validate_key.map_or_else(
                                                || {
                                                    Err("lifecycle Validate completion lost its command key"
                                                        .to_owned())
                                                },
                                                |key| {
                                                    command_rx
                                                        .complete_lifecycle_validate(
                                                            key,
                                                            guarded.result(),
                                                        )
                                                        .map(|()| true)
                                                },
                                            )
                                        }
                                        V2IoCompletion::LifecycleCertifiedServe(guarded) => {
                                            command_rx
                                                .complete_lifecycle_certified_serve(
                                                    guarded.result(),
                                                )
                                                .map(|()| true)
                                        }
                                        _ => Ok(true),
                                    };
                                    match seal_result {
                                        Err(reason) => {
                                            if let Some(key) = lifecycle_validate_key {
                                                command_rx.complete_lifecycle_validate_failure(key);
                                            }
                                            iroha_logger::error!(
                                                %reason,
                                                "failed to seal Sumeragi v2 I/O completion"
                                            );
                                            let _ = try_send_tracked_completion(
                                                &completion_tx,
                                                &worker_admission,
                                                V2IoCompletion::RecoveryRequired(reason.clone()),
                                            );
                                            true
                                        }
                                        Ok(false) => {
                                            // A durable Decision installed
                                            // while this command was active.
                                            // The queue atomically published
                                            // the typed negative and released
                                            // admission, so no stale response
                                            // completion is exposed.
                                            false
                                        }
                                        Ok(true) => {
                                            send_completion_with_lifecycle_ordinal(
                                                &completion_tx,
                                                &worker_admission,
                                                Ok(completion),
                                                runtime_lifecycle_ordinal,
                                            );
                                            false
                                        }
                                    }
                                }
                            };
                            if failed {
                                break;
                            }
                        }
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        Ok(Self {
            command_tx,
            completion_rx,
            join: Some(join),
            allow_finalized_disconnect,
            admission,
        })
    }
    fn enqueue(&self, command: V2IoCommand) -> Result<(), String> {
        self.try_enqueue(command).map_err(|error| match error {
            V2IoTrySendError::Full(_) => "Sumeragi v2 I/O queue is full".to_owned(),
            V2IoTrySendError::Disconnected(_) => {
                "Sumeragi v2 I/O worker is disconnected".to_owned()
            }
            V2IoTrySendError::ConflictingWorkId { work_id, .. } => format!(
                "Sumeragi v2 I/O work {} was reused by a conflicting command",
                work_id.get()
            ),
            V2IoTrySendError::UnreservedLifecycleDecisionApply { .. } => {
                "lifecycle Decision Apply dispatch was reused by conflicting material".to_owned()
            }
        })
    }
    fn try_enqueue(&self, command: V2IoCommand) -> Result<(), V2IoTrySendError> {
        let class = command.admission_class();
        self.try_enqueue_as(class, command)
    }
    fn try_enqueue_as(
        &self,
        class: V2IoAdmissionClass,
        command: V2IoCommand,
    ) -> Result<(), V2IoTrySendError> {
        self.command_tx.try_send_as(class, command)
    }
    fn cancel(
        &self,
        work_id: EffectWorkId,
        expected_kind: V2IoCancellableKind,
    ) -> Result<bool, String> {
        self.command_tx.cancel(work_id, expected_kind)
    }
    fn acknowledge_completion_at(
        &self,
        acknowledgement: V2IoCompletionAcknowledgement,
        ownership_position: usize,
    ) -> Result<(), String> {
        match acknowledgement {
            V2IoCompletionAcknowledgement::Work(work_id) => {
                self.command_tx.acknowledge_completion(work_id);
            }
            V2IoCompletionAcknowledgement::LifecycleWorkRetained => {}
            V2IoCompletionAcknowledgement::LifecycleDecisionApplyRetained => {}
            V2IoCompletionAcknowledgement::RecoveredLifecycleSignRetained => {
                // Generic acknowledgement cannot perform the typed owner
                // settlement, so neither the command index nor its completion
                // owner may be removed here.
                return Ok(());
            }
            V2IoCompletionAcknowledgement::RecoveredDecisionFetchRetained => {
                return Ok(());
            }
            V2IoCompletionAcknowledgement::LifecycleValidateRetained => return Ok(()),
            V2IoCompletionAcknowledgement::LifecycleServeRetained => return Ok(()),
            V2IoCompletionAcknowledgement::Untracked => {}
        }
        self.admission.acknowledge_completion_at(ownership_position);
        Ok(())
    }
    fn prepare_certified_fetch_body_persistence_ack(
        &self,
        completion: &CertifiedFetchBodyPersistenceCompletion,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Result<CertifiedFetchBodyPersistenceWorkAck, String> {
        self.command_tx
            .queue
            .prepare_certified_fetch_body_persistence_ack(completion, output_guard)
    }
    fn prepare_lifecycle_decision_apply_ack(
        &self,
        key: LifecycleDecisionApplyDispatchKeyV1,
        output_guard: Arc<ConsensusOutputGuard>,
    ) -> Result<LifecycleDecisionApplyWorkAckV1, String> {
        self.command_tx
            .queue
            .prepare_lifecycle_decision_apply_ack(key, output_guard)
    }
    fn prepare_recovered_lifecycle_sign_completion(
        &self,
        guarded: Box<GuardedRecoveredLifecycleSignWorkerResultV1>,
        ownership_position: usize,
    ) -> Option<PreparedRecoveredLifecycleSignCompletionV1> {
        PreparedRecoveredLifecycleSignCompletionV1::new(
            guarded,
            Arc::clone(&self.command_tx.queue),
            ownership_position,
        )
    }
    fn prepare_recovered_decision_fetch_body_completion(
        &self,
        guarded: Box<GuardedRecoveredDecisionFetchBodyPersistenceCompletionV1>,
        ownership_position: usize,
    ) -> Option<PreparedRecoveredDecisionFetchBodyCompletionV1> {
        PreparedRecoveredDecisionFetchBodyCompletionV1::new(
            guarded,
            Arc::clone(&self.command_tx.queue),
            ownership_position,
        )
    }
    fn prepare_lifecycle_validate_completion(
        &self,
        guarded: Box<GuardedLifecycleValidateWorkerResultV1>,
        ownership_position: usize,
    ) -> Option<PreparedLifecycleValidateCompletionV1> {
        PreparedLifecycleValidateCompletionV1::new(
            guarded,
            Arc::clone(&self.command_tx.queue),
            ownership_position,
        )
    }
    fn prepare_lifecycle_certified_serve_completion(
        &self,
        guarded: Box<GuardedLifecycleCertifiedServeWorkerResultV1>,
        ownership_position: usize,
    ) -> Option<PreparedLifecycleCertifiedServeCompletionV1> {
        PreparedLifecycleCertifiedServeCompletionV1::new(
            guarded,
            Arc::clone(&self.command_tx.queue),
            ownership_position,
        )
    }
    fn acknowledge_completion(&self, completion: &V2IoCompletion) -> Result<(), String> {
        self.acknowledge_completion_at(completion.acknowledgement(), 0)
    }
    fn record_completion_service_attempt(&self, remaining_runtime_capacity: usize) -> bool {
        remaining_runtime_capacity == 0 && self.admission.record_completion_service_debt()
    }
    fn completion_snapshot(&self, now: Instant) -> RuntimeQueueLaneSnapshot {
        self.admission.completion_snapshot(now)
    }
    fn completion_requires_runtime_capacity_at(&self, position: usize) -> Option<bool> {
        self.admission
            .completion_requires_runtime_capacity_at(position)
    }
    fn completion_ownership_at(&self, position: usize) -> Option<V2IoCompletionOwnership> {
        self.admission.completion_ownership_at(position)
    }
    fn try_recv_completion_unacknowledged(&self) -> Result<V2IoCompletion, mpsc::TryRecvError> {
        self.completion_rx.try_recv()
    }
    #[cfg(test)]
    fn try_recv_completion(&self) -> Result<V2IoCompletion, mpsc::TryRecvError> {
        let completion = self.completion_rx.try_recv()?;
        self.acknowledge_completion(&completion)
            .expect("completion acknowledgement is infallible");
        Ok(completion)
    }
    fn recv_completion(&self) -> Result<V2IoCompletion, mpsc::RecvError> {
        let completion = self.completion_rx.recv()?;
        self.acknowledge_completion(&completion)
            .expect("completion acknowledgement is infallible");
        Ok(completion)
    }
    fn recv_completion_timeout(
        &self,
        timeout: Duration,
    ) -> Result<V2IoCompletion, mpsc::RecvTimeoutError> {
        let completion = self.completion_rx.recv_timeout(timeout)?;
        self.acknowledge_completion(&completion)
            .expect("completion acknowledgement is infallible");
        Ok(completion)
    }
    fn shutdown(mut self) -> Result<(), String> {
        let mut command = V2IoCommand::Shutdown;
        loop {
            match self.try_enqueue(command) {
                Ok(()) => break,
                Err(V2IoTrySendError::Full(returned)) => {
                    command = returned;
                    if self.recv_completion().is_err() {
                        break;
                    }
                }
                Err(V2IoTrySendError::Disconnected(_)) => break,
                Err(
                    V2IoTrySendError::ConflictingWorkId { .. }
                    | V2IoTrySendError::UnreservedLifecycleDecisionApply { .. },
                ) => {
                    unreachable!("shutdown commands do not carry work identifiers");
                }
            }
        }
        // The worker can have commands ahead of Shutdown. Drain their bounded
        // completions so it can reach Shutdown without a cyclic channel wait.
        while self.recv_completion().is_ok() {}
        if let Some(join) = self.join.take() {
            join.join()
                .map_err(|_| "Sumeragi v2 I/O worker panicked".to_owned())?;
        }
        Ok(())
    }
}
