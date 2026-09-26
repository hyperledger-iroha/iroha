//! Production Native validation retains the actual execution through marker retries.
//!
//! The finite shared pool covers two World journal wrapper sets, the retained
//! effects box, the candidate phase box and service descriptors. Nested World
//! values, decoded proposals and execution payloads keep their existing bounds;
//! this policy does not claim an aggregate process-memory ceiling.

use super::validation_custody::{
    CarrierValidator, RetainedBodyValidationService, RetainedValidationOwner,
};
use super::{V2ApplyError, V2ApplyService, VerifiedHeightContext};
use crate::{
    state::{
        AuthenticatedLaneAdmittedInputSourceV1, NativeLaneBatchSourcePreparationV1,
        PreparedCarrier, RetainedCarrier, VerifiedFirstLaneAdmittedInputV1,
    },
    sumeragi::{
        v2_body_store::{
            BodyValidationBusy, BodyValidationError, LocalValidationRefusal, V2BodyStore,
            V2BodyStoreError,
        },
        v2_transport::{AuthenticatedCertifiedBodyRequest, AuthenticatedCertifiedBodyResponse},
    },
};
use iroha_data_model::block::{SignedBlock, consensus_v2 as wire};
use iroha_data_model::{
    isi::consensus_keys::ApplyThresholdKeyLifecycleCertificateV1,
    transaction::{Executable, TransactionAdmissionIntent, TransactionEntrypoint},
};
use iroha_model_base::topology::DataSpaceId;
use mv::allocation::{AllocationCharge, AllocationRefusal, AllocationReservation};
use std::{alloc::Layout, collections::BTreeSet, convert::Infallible, sync::Arc};

#[cfg(test)]
std::thread_local! {
    static FAIL_POST_PUBLICATION_QUEUE_TAIL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Refuse one completion step after actual publication, on this test thread only.
#[cfg(test)]
pub(crate) fn fail_next_post_publication_queue_tail_for_test() {
    FAIL_POST_PUBLICATION_QUEUE_TAIL.set(true);
}

/// Credits for both coexisting World wrapper sets and the actual effects box.
/// The original pool reservation moves with the journals until final destruction.
pub(crate) struct CarrierShellAdmission {
    _reservation: AllocationReservation,
}

/// The actual published Native carrier, including its original local shell owner.
pub(crate) type PublishedNativeCarrier = crate::state::PublishedCarrier<CarrierShellAdmission>;

impl CarrierShellAdmission {
    fn requested_bytes() -> Result<usize, AllocationRefusal> {
        PreparedCarrier::world_journal_shell_bytes()?
            .checked_add(PreparedCarrier::retained_effects_layout().size())
            .ok_or(AllocationRefusal::DemandOverflow)
    }

    /// Inspect the original finite pool and retained demand in custody tests.
    #[cfg(test)]
    pub(crate) fn reserved_bytes(&self) -> usize {
        self._reservation.remaining_bytes()
    }
}

/// One immutable application service and verified height context for BodyStore.
pub(crate) struct OwnedNativeCarrierValidator {
    service: Arc<V2ApplyService>,
    context: VerifiedHeightContext,
}

struct PendingNativeSource {
    execution_index: usize,
    source: Arc<AuthenticatedLaneAdmittedInputSourceV1>,
}

#[derive(Clone, Copy)]
enum CurrentCarrierSourceClass {
    Genesis,
    Control,
    Native,
}

struct AwaitingNativeSource {
    class: CurrentCarrierSourceClass,
    context: VerifiedHeightContext,
    proposal_hash: iroha_crypto::Hash,
    recovered: Vec<(usize, VerifiedFirstLaneAdmittedInputV1)>,
    pending: Option<PendingNativeSource>,
    // Payloads retire before the original shell reservation is refunded.
    shell_admission: Option<CarrierShellAdmission>,
}

enum NativeValidationPhase {
    AwaitingSource(AwaitingNativeSource),
    Stopped {
        context_id: wire::HeightContextId,
        proposal_hash: iroha_crypto::Hash,
        reason: String,
    },
    Executed {
        carrier: RetainedCarrier<CarrierShellAdmission>,
        evidence_ready: bool,
    },
    Published {
        carrier: PublishedNativeCarrier,
        outbox_published: bool,
        queue_cleaned: bool,
    },
}

/// A preallocated phase box stays identical across source, archive and marker waits.
/// The temporary vacant phase exists only inside a synchronous consuming retry;
/// panic is fail-stop and cannot recreate execution authority.
pub(crate) struct NativeValidationCandidate {
    phase: Box<Option<NativeValidationPhase>>,
    _container_admission: AllocationCharge,
}

impl RetainedValidationOwner for NativeValidationCandidate {
    fn matches_candidate(&self, context: &wire::HeightContext, body: &SignedBlock) -> bool {
        match self
            .phase
            .as_ref()
            .as_ref()
            .expect("original Native validation phase")
        {
            NativeValidationPhase::AwaitingSource(source) => {
                // The retaining service compares the complete signed body before
                // retrying this phase, so no second deep hash/encode is needed.
                source.context.context() == context
            }
            NativeValidationPhase::Stopped {
                context_id,
                proposal_hash,
                ..
            } => {
                *context_id == context.id()
                    && body
                        .canonical_proposal_wire_hash()
                        .is_ok_and(|hash| hash == *proposal_hash)
            }
            NativeValidationPhase::Executed { carrier, .. } => {
                carrier.matches_validation_candidate(context, body)
            }
            NativeValidationPhase::Published { carrier, .. } => {
                carrier.artifact().height_context == *context
                    && carrier.block().canonical_proposal_wire_hash().ok()
                        == body.canonical_proposal_wire_hash().ok()
            }
        }
    }

    fn needs_decoded_body(&self) -> bool {
        matches!(
            self.phase
                .as_ref()
                .as_ref()
                .expect("original Native validation phase"),
            NativeValidationPhase::AwaitingSource(_)
        )
    }

    fn ready_commitment(&self) -> Option<wire::ExecutionCommitment> {
        match self
            .phase
            .as_ref()
            .as_ref()
            .expect("original Native validation phase")
        {
            NativeValidationPhase::AwaitingSource(_) | NativeValidationPhase::Stopped { .. } => {
                None
            }
            NativeValidationPhase::Executed {
                carrier,
                evidence_ready,
            } => evidence_ready.then(|| carrier.ready_commitment()).flatten(),
            NativeValidationPhase::Published { carrier, .. } => {
                Some(carrier.artifact().commit_qc.execution_commitment)
            }
        }
    }
}

impl NativeValidationCandidate {
    /// Settle only the response authenticated by this candidate's original source.
    pub(crate) fn complete_native_source(
        &mut self,
        subject: wire::BlockSubject,
        request: &AuthenticatedCertifiedBodyRequest,
        response: &AuthenticatedCertifiedBodyResponse,
    ) -> Result<NativeSourceRecoveryCompletion, LocalValidationRefusal> {
        let Some(NativeValidationPhase::AwaitingSource(source)) = self.phase.as_mut() else {
            return Err(LocalValidationRefusal::RecoveryRequired(
                "Native source response arrived after original source execution".into(),
            ));
        };
        let pending = source.pending.as_ref().ok_or_else(|| {
            LocalValidationRefusal::RecoveryRequired(
                "Native source response has no pending original request".into(),
            )
        })?;
        let recovered = pending
            .source
            .complete_from_authenticated_response(request, response)
            .map_err(LocalValidationRefusal::RecoveryRequired)?;
        if source
            .recovered
            .iter()
            .any(|(index, _)| *index == pending.execution_index)
        {
            return Err(LocalValidationRefusal::RecoveryRequired(
                "Native source recovery repeated an already settled execution index".into(),
            ));
        }
        source.recovered.push((pending.execution_index, recovered));
        let completion = NativeSourceRecoveryCompletion {
            subject,
            execution_index: pending.execution_index,
            source: Arc::clone(&pending.source),
        };
        source.pending = None;
        Ok(completion)
    }

    /// Publish only this exact executed phase through the original service's dependencies.
    pub(crate) fn try_publish(
        mut self,
        validator: &OwnedNativeCarrierValidator,
        finality: crate::block::VerifiedV2FinalityArtifact,
    ) -> Result<PublishedNativeCarrier, (Self, LocalValidationRefusal)> {
        let phase = self.phase.take().expect("original Native validation phase");
        let (published, mut outbox_published, mut queue_cleaned) = match phase {
            NativeValidationPhase::Executed {
                carrier,
                evidence_ready: true,
            } => {
                let queue = super::carrier_queue_retirement::OriginalCarrierQueue::from_service(
                    &validator.service,
                );
                match carrier.try_publish(
                    &validator.service.state,
                    &queue,
                    finality,
                    validator.service.queue.sumeragi_waker(),
                ) {
                    Ok(published) => (published, false, false),
                    Err((carrier, refusal)) => {
                        *self.phase = Some(NativeValidationPhase::Executed {
                            carrier,
                            evidence_ready: true,
                        });
                        return Err((self, refusal));
                    }
                }
            }
            NativeValidationPhase::Published {
                carrier,
                outbox_published,
                queue_cleaned,
            } => {
                if carrier.artifact() != finality.artifact() {
                    *self.phase = Some(NativeValidationPhase::Published {
                        carrier,
                        outbox_published,
                        queue_cleaned,
                    });
                    return Err((
                        self,
                        LocalValidationRefusal::RecoveryRequired(
                            "post-publication retry changed the original finality".into(),
                        ),
                    ));
                }
                (carrier, outbox_published, queue_cleaned)
            }
            phase => {
                *self.phase = Some(phase);
                return Err((
                    self,
                    LocalValidationRefusal::RecoveryRequired(
                        "Native publication requires its original evidence-ready execution".into(),
                    ),
                ));
            }
        };
        let service = validator.service.as_ref();
        // Keep the actual irreversible publication in this original descriptor until
        // every fallible completion step succeeds. These durable outbox writes are
        // immutable and idempotent; completed steps are never repeated in-process.
        let tail = (|| {
            if !outbox_published {
                service.publish_kagemusha_mint_outbox_v1(published.artifact())?;
                outbox_published = true;
            }
            if !queue_cleaned {
                #[cfg(test)]
                if FAIL_POST_PUBLICATION_QUEUE_TAIL.replace(false) {
                    return Err(LocalValidationRefusal::RecoveryRequired(
                        "injected post-publication Queue completion refusal".into(),
                    )
                    .into());
                }
                service
                    .queue
                    .remove_state_committed_replay_owners_preserving_globally_bound(
                        &service.state.view(),
                        None,
                    )
                    .map_err(|error| {
                        V2ApplyError::committed_recovery_required(
                            "Native replay-terminal Queue cleanup",
                            &error,
                        )
                    })?;
                queue_cleaned = true;
            }
            Ok::<(), V2ApplyError>(())
        })();
        if let Err(error) = tail {
            let refusal = error
                .local_refusal()
                .unwrap_or_else(|| LocalValidationRefusal::RecoveryRequired(error.to_string()));
            *self.phase = Some(NativeValidationPhase::Published {
                carrier: published,
                outbox_published,
                queue_cleaned,
            });
            return Err((self, refusal));
        }
        // No fallible operation follows notifications, so a retained tail retry
        // cannot emit committed events or reconfigure the Queue a second time.
        let nexus = service.state.nexus_snapshot();
        let compliance = service.queue.lane_compliance_engine();
        service
            .queue
            .reconfigure_nexus_with_state(&nexus, service.state.as_ref(), compliance);
        let _ = service
            .events_sender
            .send(iroha_data_model::events::EventBox::Pipeline(
                iroha_data_model::events::pipeline::PipelineEventBox::Block(
                    published.committed_event().clone(),
                ),
            ));
        for event in published.events() {
            let _ = service.events_sender.send(event.clone());
        }
        Ok(published)
    }

    /// Inspect the original phase allocation and completed tail steps after refusal.
    #[cfg(test)]
    pub(crate) fn published_progress_for_test(&self) -> Option<(usize, bool, bool)> {
        match self.phase.as_ref().as_ref()? {
            NativeValidationPhase::Published {
                outbox_published,
                queue_cleaned,
                ..
            } => Some((
                std::ptr::from_ref(self.phase.as_ref()) as usize,
                *outbox_published,
                *queue_cleaned,
            )),
            _ => None,
        }
    }

    /// Inspect the original phase allocation before publication.
    #[cfg(test)]
    pub(crate) fn phase_allocation_for_test(&self) -> usize {
        std::ptr::from_ref(self.phase.as_ref()) as usize
    }

    /// Borrow the original journals for allocation-identity checks.
    #[cfg(test)]
    pub(crate) fn carrier_for_test(&self) -> Option<&RetainedCarrier<CarrierShellAdmission>> {
        match self.phase.as_ref().as_ref()? {
            NativeValidationPhase::Executed { carrier, .. } => Some(carrier),
            NativeValidationPhase::AwaitingSource(_)
            | NativeValidationPhase::Stopped { .. }
            | NativeValidationPhase::Published { .. } => None,
        }
    }
}

impl V2ApplyService {
    /// Share one original finite pool across all candidates of this service.
    pub(super) fn reserve_carrier_shells(&self) -> Result<CarrierShellAdmission, V2ApplyError> {
        let bytes = CarrierShellAdmission::requested_bytes()
            .map_err(|error| self.carrier_allocation_refusal(error))?;
        let reservation = self
            .carrier_shell_budget
            .try_reserve_bytes(bytes)
            .map_err(|error| self.carrier_allocation_refusal(error))?;
        Ok(CarrierShellAdmission {
            _reservation: reservation,
        })
    }

    fn carrier_allocation_refusal(&self, error: AllocationRefusal) -> V2ApplyError {
        match error {
            AllocationRefusal::Capacity { release, .. } => {
                LocalValidationRefusal::PhysicalBusy(BodyValidationBusy::new(
                    "retained_carrier_shell_pool",
                    release,
                    self.queue.sumeragi_waker(),
                ))
                .into()
            }
            error => LocalValidationRefusal::RecoveryRequired(error.to_string()).into(),
        }
    }

    /// Construct the one retained validator bound to this original open BodyStore.
    pub(crate) fn retained_validation_service(
        self: &Arc<Self>,
        store: &V2BodyStore,
        context: VerifiedHeightContext,
    ) -> Result<RetainedBodyValidationService<OwnedNativeCarrierValidator>, V2BodyStoreError> {
        if !store.matches_context(context.context())
            || context.context().network_id != self.network_id
        {
            return Err(V2BodyStoreError::ContextMismatch);
        }
        store.retained_validation_service(
            OwnedNativeCarrierValidator {
                service: Arc::clone(self),
                context,
            },
            &self.carrier_shell_budget,
        )
    }

    /// Subscribe before publication to count actual completion notifications.
    #[cfg(test)]
    pub(crate) fn events_for_test(
        &self,
    ) -> tokio::sync::broadcast::Receiver<iroha_data_model::events::EventBox> {
        self.events_sender.subscribe()
    }

    /// Original pool handle for focused tests; clones retain the same pool identity.
    #[cfg(test)]
    pub(crate) fn carrier_shell_budget_for_test(&self) -> mv::allocation::AllocationBudget {
        self.carrier_shell_budget.clone()
    }
}

impl OwnedNativeCarrierValidator {
    /// A lifecycle certificate is a special exact-height direct input.
    /// Reauthenticate it here: a Torii decision is never voting authority.
    fn authenticate_lifecycle_control(&self, body: &SignedBlock) -> Result<(), V2ApplyError> {
        let invalid = |reason: &str| V2ApplyError::Validation(reason.to_owned());
        let [entrypoint] = body.external_entrypoints_slice() else {
            return Err(invalid(
                "lifecycle control must be the sole external entrypoint",
            ));
        };
        let TransactionEntrypoint::External(transaction) = entrypoint else {
            return Err(invalid(
                "lifecycle control must be a signed external transaction",
            ));
        };
        if transaction.admission_intent() != TransactionAdmissionIntent::Ordinary
            || transaction.attachments().is_some()
            || transaction.multisig_signatures().is_some()
            || !transaction.metadata().is_empty()
        {
            return Err(invalid(
                "lifecycle control has noncanonical transaction attachments",
            ));
        }
        let Executable::Instructions(instructions) = transaction.instructions() else {
            return Err(invalid(
                "lifecycle control must contain one certificate instruction",
            ));
        };
        if instructions.len() != 1 {
            return Err(invalid(
                "lifecycle control must contain one certificate instruction",
            ));
        }
        let certificate = &instructions[0]
            .as_any()
            .downcast_ref::<ApplyThresholdKeyLifecycleCertificateV1>()
            .ok_or_else(|| invalid("lifecycle control has a non-certificate instruction"))?
            .certificate;
        let context = self.context.context();
        if certificate.effective_height != context.height
            || certificate.network_id != context.network_id
            || body.header().height().get() != context.height
        {
            return Err(invalid(
                "lifecycle certificate differs from the authenticated height",
            ));
        }
        let bundle = body
            .execution_context()
            .ok_or_else(|| invalid("lifecycle control lacks its global route context"))?;
        let [external] = bundle.external.as_slice() else {
            return Err(invalid(
                "lifecycle control requires one global route context",
            ));
        };
        if external.entrypoint_hash != entrypoint.hash() {
            return Err(invalid(
                "lifecycle route context differs from its entrypoint",
            ));
        }
        let plan = crate::queue::routing_plan_from_execution_context(external)
            .map_err(|reason| invalid(&reason))?;
        let crate::queue::RoutingPlan::Single(leg) = &plan else {
            return Err(invalid("lifecycle control requires one global route"));
        };
        if leg.role != crate::queue::RouteLegRole::Coordinator
            || leg.route.dataspace_id != DataSpaceId::UNIVERSAL
        {
            return Err(invalid("lifecycle control has a non-global route"));
        }
        let (parent_hash, roster) = self
            .service
            .state
            .verify_next_height_threshold_key_lifecycle_certificate_v1(certificate)
            .map_err(|reason| invalid(&reason))?;
        if body.header().prev_block_hash() != Some(parent_hash)
            || context
                .roster
                .iter()
                .map(|validator| &validator.validator)
                .ne(roster.iter())
        {
            return Err(invalid(
                "lifecycle control differs from its authenticated parent or roster",
            ));
        }
        let route = self
            .service
            .queue
            .plan_admission_context_with_state(&self.service.state, &plan)
            .map_err(|error| invalid(&error.to_string()))?;
        if route.proposal_height != context.height
            || route.predecessor_block_hash != Some(parent_hash)
            || route.route_incarnations.len() != 1
            || route.route_incarnations[0].leg != *leg
            || route.route_incarnations[0]
                .validator_set
                .iter()
                .collect::<BTreeSet<_>>()
                != roster.iter().collect::<BTreeSet<_>>()
        {
            return Err(invalid(
                "lifecycle control route lacks authenticated global authority",
            ));
        }
        Ok(())
    }

    /// A peer validates the leader's signed ordinary input and route from the
    /// proposal itself; its local queue may contain a different async snapshot.
    fn authenticate_direct_external(&self, body: &SignedBlock) -> Result<(), V2ApplyError> {
        let bundle = body.execution_context().ok_or_else(|| {
            V2ApplyError::Validation("direct external carrier lacks execution context".into())
        })?;
        if bundle.external.len() != body.external_entrypoint_count() {
            return Err(V2ApplyError::Validation(
                "direct external route count differs from its inputs".into(),
            ));
        }
        let mut lifecycle = false;
        for (entrypoint, context) in body
            .external_entrypoints_slice()
            .iter()
            .zip(&bundle.external)
        {
            let TransactionEntrypoint::External(signed) = entrypoint else {
                return Err(V2ApplyError::Validation(
                    "direct external input must be a signed transaction".into(),
                ));
            };
            if signed.admission_intent() != TransactionAdmissionIntent::Ordinary
                || context.entrypoint_hash != entrypoint.hash()
                || context.native_amx_receipt.is_some()
                || !matches!(
                    crate::queue::routing_plan_from_execution_context(context),
                    Ok(crate::queue::RoutingPlan::Single(_))
                )
            {
                return Err(V2ApplyError::Validation(
                    "direct external input must bind one ordinary route".into(),
                ));
            }
            lifecycle |= signed
                .instructions()
                .explicit_instructions()
                .any(|instruction| {
                    instruction
                        .as_any()
                        .downcast_ref::<ApplyThresholdKeyLifecycleCertificateV1>()
                        .is_some()
                });
        }
        if lifecycle {
            self.authenticate_lifecycle_control(body)?;
        }
        Ok(())
    }

    // These are disjoint first-release producers. Failed Native authentication
    // never enters the genesis/control producer or a second execution attempt.
    fn classify_source(
        &self,
        body: &SignedBlock,
    ) -> Result<CurrentCarrierSourceClass, V2ApplyError> {
        if !body.is_resultless_proposal() {
            return Err(V2ApplyError::ResultBearingProposal);
        }
        if body.execution_context().is_some_and(|bundle| {
            bundle.merge_entry.is_some()
                || !bundle.lane_payload_ownerships.is_empty()
                || !bundle.autonomous_lane_payloads.is_empty()
        }) {
            return Err(V2ApplyError::Validation(
                "obsolete merge, lane-ownership and autonomous payload carriers are unsupported"
                    .into(),
            ));
        }
        let native = body
            .execution_context()
            .is_some_and(|bundle| bundle.native_lane_decisions.is_some());
        if self.context.context().height == 1 {
            if native
                || body.header().prev_block_hash().is_some()
                || self.context.context().parent_commit_qc.is_some()
                || self.context.context().snapshot_bootstrap.is_some()
            {
                return Err(V2ApplyError::TaskMismatch);
            }
            return Ok(CurrentCarrierSourceClass::Genesis);
        }
        if !body.external_entrypoints_slice().is_empty() {
            if native {
                return Err(V2ApplyError::Validation(
                    "Native Decisions cannot share a carrier with direct external entrypoints"
                        .into(),
                ));
            }
            self.authenticate_direct_external(body)?;
            return Ok(CurrentCarrierSourceClass::Control);
        }
        Ok(if native {
            CurrentCarrierSourceClass::Native
        } else {
            CurrentCarrierSourceClass::Control
        })
    }

    fn execute_source(
        &self,
        mut waiting: AwaitingNativeSource,
        proposal: &SignedBlock,
    ) -> Result<NativeValidationPhase, V2ApplyError> {
        if matches!(
            waiting.class,
            CurrentCarrierSourceClass::Genesis | CurrentCarrierSourceClass::Control
        ) {
            if waiting.pending.is_some() || !waiting.recovered.is_empty() {
                return Err(LocalValidationRefusal::RecoveryRequired(
                    "control carrier acquired a foreign Native source recovery owner".into(),
                )
                .into());
            }
            let prepared = self.service.prepare_current_control_source_admitted(
                proposal,
                &waiting.context,
                waiting
                    .shell_admission
                    .take()
                    .expect("control source retains its original shell admission"),
            )?;
            return Self::detach_prepared(prepared);
        }
        let prepared = self
            .service
            .state
            .prepare_proposed_native_lane_batch_source(proposal, &waiting.recovered)
            .map_err(|reason| LocalValidationRefusal::RecoveryRequired(reason))?;
        let source = match prepared {
            NativeLaneBatchSourcePreparationV1::Ready(source) => source,
            NativeLaneBatchSourcePreparationV1::FirstInputRecoveryRequired {
                execution_index,
                source,
            } => {
                waiting.pending = Some(PendingNativeSource {
                    execution_index,
                    source: Arc::new(source),
                });
                return Ok(NativeValidationPhase::AwaitingSource(waiting));
            }
            NativeLaneBatchSourcePreparationV1::ObservationChanged => {
                return Ok(NativeValidationPhase::AwaitingSource(waiting));
            }
            NativeLaneBatchSourcePreparationV1::Superseded => {
                return Err(LocalValidationRefusal::Superseded.into());
            }
        };
        let prepared = self.service.prepare_native_source_admitted(
            proposal,
            source,
            waiting.context.clone(),
            &mut waiting.shell_admission,
        )?;
        let Some(prepared) = prepared else {
            return Ok(NativeValidationPhase::AwaitingSource(waiting));
        };
        Self::detach_prepared(prepared)
    }

    fn detach_prepared(
        prepared: super::native_preparation::PreparedNativeServiceCandidate<'_>,
    ) -> Result<NativeValidationPhase, V2ApplyError> {
        let (carrier, provider, reputation, admission) = prepared.into_parts();
        let carrier = match carrier
            .prepare_journals(provider, reputation, |_| Ok::<_, Infallible>(admission))
        {
            Ok(journals) => RetainedCarrier::Validated(journals),
            Err(crate::state::CarrierJournalPreparationError::ArchivePreparation {
                carrier,
                ..
            }) => RetainedCarrier::Capturing(carrier),
            Err(error) => {
                return Err(LocalValidationRefusal::RecoveryRequired(error.to_string()).into());
            }
        };
        Ok(NativeValidationPhase::Executed {
            carrier,
            evidence_ready: false,
        })
    }

    fn archive_refusal(
        &self,
        error: crate::state::CarrierArchivePreparationError,
    ) -> LocalValidationRefusal {
        use crate::query::{
            provider_ingest_finalized::ProviderIngestFinalizedArchiveErrorV1 as Provider,
            reputation_finalized::ReputationFinalizedArchiveError as Reputation,
        };
        let busy = |name, wait| {
            LocalValidationRefusal::PhysicalBusy(BodyValidationBusy::new(
                name,
                wait,
                self.service.queue.sumeragi_waker(),
            ))
        };
        match &error {
            crate::state::CarrierArchivePreparationError::Provider(error) => match error.as_ref() {
                Provider::IndexBusy { wait } => {
                    return busy("provider_archive_index", wait.clone());
                }
                Provider::CaptureReserved { wait } => {
                    return busy("provider_archive_capture", wait.release_wait().clone());
                }
                _ => {}
            },
            crate::state::CarrierArchivePreparationError::Reputation(error) => match error.as_ref()
            {
                Reputation::IndexBusy { wait } => {
                    return busy("reputation_archive_index", wait.clone());
                }
                Reputation::CaptureReserved { wait } => {
                    return busy("reputation_archive_capture", wait.release_wait().clone());
                }
                _ => {}
            },
        }
        LocalValidationRefusal::RecoveryRequired(error.to_string())
    }
}

/// Exercise the production source classifier without allocating a retained
/// candidate or promoting a BodyStore validation marker.
#[cfg(test)]
pub(super) fn classify_current_carrier_for_test(
    service: Arc<V2ApplyService>,
    context: VerifiedHeightContext,
    body: &SignedBlock,
) -> Result<(), V2ApplyError> {
    OwnedNativeCarrierValidator { service, context }
        .classify_source(body)
        .map(|_| ())
}

impl CarrierValidator for OwnedNativeCarrierValidator {
    type Owner = NativeValidationCandidate;
    type Error = V2ApplyError;

    fn prepare(
        &mut self,
        context: &wire::HeightContext,
        body: &SignedBlock,
    ) -> Result<Self::Owner, Self::Error> {
        if context != self.context.context() {
            return Err(V2ApplyError::TaskMismatch);
        }
        let class = self.classify_source(body)?;
        let layout = Layout::new::<Option<NativeValidationPhase>>();
        let mut container = self
            .service
            .carrier_shell_budget
            .try_reserve_layouts([layout])
            .map_err(|error| self.service.carrier_allocation_refusal(error))?;
        let container_admission = container
            .try_split(layout)
            .map_err(|error| LocalValidationRefusal::RecoveryRequired(error.to_string()))?;
        let shell_admission = self.service.reserve_carrier_shells()?;
        let context_id = context.id();
        let proposal_hash = body
            .canonical_proposal_wire_hash()
            .map_err(|error| V2ApplyError::CanonicalBlock(error.to_string()))?;
        let result = self.execute_source(
            AwaitingNativeSource {
                class,
                context: self.context.clone(),
                proposal_hash,
                recovered: Vec::new(),
                pending: None,
                shell_admission: Some(shell_admission),
            },
            body,
        );
        let phase = match result {
            Ok(phase) => phase,
            // Before execution, original archive or shell occupancy can retry normally.
            Err(error)
                if matches!(
                    error.local_refusal(),
                    Some(
                        LocalValidationRefusal::PhysicalBusy(_)
                            | LocalValidationRefusal::QueueRelease { .. }
                    )
                ) =>
            {
                return Err(error);
            }
            Err(error) if error.rejection_identity().is_some() => return Err(error),
            Err(error)
                if matches!(
                    error.local_refusal(),
                    Some(LocalValidationRefusal::Superseded)
                ) =>
            {
                return Err(error);
            }
            // Fatal journal/capture failure after execution must occupy the same slot:
            // a later marker attempt must never manufacture a second execution.
            Err(error) => NativeValidationPhase::Stopped {
                context_id,
                proposal_hash,
                reason: error.to_string(),
            },
        };
        Ok(NativeValidationCandidate {
            phase: Box::new(Some(phase)),
            _container_admission: container_admission,
        })
    }

    fn resume(
        &mut self,
        mut owner: Self::Owner,
        proposal: &SignedBlock,
    ) -> Result<Self::Owner, (Self::Owner, LocalValidationRefusal)> {
        let phase = owner
            .phase
            .take()
            .expect("original Native validation phase");
        let phase = match phase {
            NativeValidationPhase::AwaitingSource(waiting) => {
                if let Some(pending) = &waiting.pending {
                    let refusal = LocalValidationRefusal::NativeSourceRecovery {
                        execution_index: pending.execution_index,
                        authenticated_source: Arc::clone(&pending.source),
                        wake: self.service.queue.sumeragi_waker(),
                    };
                    *owner.phase = Some(NativeValidationPhase::AwaitingSource(waiting));
                    return Err((owner, refusal));
                }
                // All completed original responses stay attached before the one execution.
                let context_id = waiting.context.context().id();
                let proposal_hash = waiting.proposal_hash;
                match self.execute_source(waiting, proposal) {
                    Ok(phase) => phase,
                    Err(error)
                        if matches!(
                            error.local_refusal(),
                            Some(LocalValidationRefusal::Superseded)
                        ) =>
                    {
                        *owner.phase = Some(NativeValidationPhase::Stopped {
                            context_id,
                            proposal_hash,
                            reason: "superseded before Native execution".into(),
                        });
                        return Err((owner, LocalValidationRefusal::Superseded));
                    }
                    Err(error) => {
                        // Execution failure is fail-stop once the descriptor is retained;
                        // it cannot release that descriptor for a second execution.
                        let reason = error.to_string();
                        *owner.phase = Some(NativeValidationPhase::Stopped {
                            context_id,
                            proposal_hash,
                            reason: reason.clone(),
                        });
                        return Err((owner, LocalValidationRefusal::RecoveryRequired(reason)));
                    }
                }
            }
            phase => phase,
        };
        match phase {
            phase @ NativeValidationPhase::Published { .. } => {
                *owner.phase = Some(phase);
                Ok(owner)
            }
            NativeValidationPhase::Stopped {
                context_id,
                proposal_hash,
                reason,
            } => {
                let refusal = LocalValidationRefusal::RecoveryRequired(reason.clone());
                *owner.phase = Some(NativeValidationPhase::Stopped {
                    context_id,
                    proposal_hash,
                    reason,
                });
                Err((owner, refusal))
            }
            NativeValidationPhase::AwaitingSource(waiting) => {
                let refusal = if let Some(pending) = waiting.pending.as_ref() {
                    LocalValidationRefusal::NativeSourceRecovery {
                        execution_index: pending.execution_index,
                        authenticated_source: Arc::clone(&pending.source),
                        wake: self.service.queue.sumeragi_waker(),
                    }
                } else {
                    LocalValidationRefusal::ObservationChanged {
                        wake: self.service.queue.sumeragi_waker(),
                    }
                };
                *owner.phase = Some(NativeValidationPhase::AwaitingSource(waiting));
                Err((owner, refusal))
            }
            NativeValidationPhase::Executed { carrier, .. } => {
                let carrier = match carrier.resume_capture() {
                    Ok(carrier) => carrier,
                    Err((carrier, error)) => {
                        *owner.phase = Some(NativeValidationPhase::Executed {
                            carrier,
                            evidence_ready: false,
                        });
                        return Err((owner, self.archive_refusal(error)));
                    }
                };
                let evidence = self
                    .service
                    .kura
                    .validate_native_amx_participant_application_evidence_byte_budget(
                        carrier.native_amx_manifest(),
                        None,
                    )
                    .map_err(V2ApplyService::classify_native_amx_evidence_byte_budget_error);
                *owner.phase = Some(NativeValidationPhase::Executed {
                    carrier,
                    evidence_ready: evidence.is_ok(),
                });
                match evidence {
                    Ok(()) => Ok(owner),
                    Err(error) => {
                        let refusal = error.local_refusal().unwrap_or_else(|| {
                            LocalValidationRefusal::RecoveryRequired(error.to_string())
                        });
                        Err((owner, refusal))
                    }
                }
            }
        }
    }
}

/// Original authenticated response joined to one exact waiting source occurrence.
pub(crate) struct NativeSourceRecoveryCompletion {
    subject: wire::BlockSubject,
    execution_index: usize,
    source: Arc<AuthenticatedLaneAdmittedInputSourceV1>,
}
impl NativeSourceRecoveryCompletion {
    /// Equality of the retained source allocation and exact applying occurrence.
    pub(crate) fn matches(
        &self,
        subject: wire::BlockSubject,
        execution_index: usize,
        source: &Arc<AuthenticatedLaneAdmittedInputSourceV1>,
    ) -> bool {
        self.subject == subject
            && self.execution_index == execution_index
            && Arc::ptr_eq(&self.source, source)
    }
}

/// One actual publication kept beside its reducer completion across worker delivery.
pub(crate) struct PublishedApplyCompletion {
    /// Original reducer work association and actual Kura receipt.
    pub(crate) completion: crate::sumeragi::v2_effects::DurableApplyCompletion,
    /// Actual sole-publisher output retained for Native driver settlement.
    pub(crate) published: PublishedNativeCarrier,
}
impl PublishedApplyCompletion {
    /// Original queue work identifier.
    pub(crate) fn work_id(&self) -> crate::sumeragi::v2_effects::EffectWorkId {
        self.completion.work_id()
    }
    /// The complete finality retained by the original publication.
    pub(crate) fn artifact(&self) -> &wire::finality::V2FinalityArtifact {
        self.published.artifact()
    }
}

impl V2ApplyService {
    fn publish_retained_task(
        &self,
        context: &wire::HeightContext,
        body_store: &mut V2BodyStore,
        retained: &mut RetainedBodyValidationService<OwnedNativeCarrierValidator>,
        task: super::ExactApplyTaskRef<'_>,
    ) -> Result<PublishedNativeCarrier, V2ApplyError> {
        if !retained.matches_store(&body_store.instance_identity()) {
            return Err(V2ApplyError::TaskMismatch);
        }
        if task.certificate().subject != task.subject()
            || task.validated_receipt().durable().subject() != task.subject()
            || task.validated_receipt().durable().context_id() != context.id()
            || task.certificate().execution_commitment
                != task.validated_receipt().execution_commitment()
        {
            return Err(V2ApplyError::TaskMismatch);
        }
        // Recheck the exact body authority before selecting the retained
        // owner. A cold terminal result may authorize only an ordinary or
        // recovered lifecycle Apply; live lifecycle requires a voting marker.
        match task {
            super::ExactApplyTaskRef::Ordinary(_)
            | super::ExactApplyTaskRef::LifecycleRecovered(_) => {
                body_store.verify_recovered_apply_validated_receipt(task.validated_receipt())?
            }
            super::ExactApplyTaskRef::LifecycleLive(_) => {
                body_store.verify_validated_receipt(task.validated_receipt())?;
            }
        }
        let finality = crate::block::VerifiedV2FinalityArtifact::verify(
            wire::finality::V2FinalityArtifact::new(
                context.clone(),
                task.subject(),
                task.certificate().clone(),
                self.validator_set_pops.clone(),
            ),
        )
        .map_err(V2ApplyError::FinalityCryptography)?;
        let selected = retained
            .select(task.validated_receipt())
            .map_err(|error| LocalValidationRefusal::RecoveryRequired(error.to_string()))?;
        selected
            .try_consume(|validator, owner| {
                if !std::ptr::eq(self, validator.service.as_ref()) {
                    return Err((
                        owner,
                        LocalValidationRefusal::RecoveryRequired(
                            "retained Apply changed its original service owner".into(),
                        ),
                    ));
                }
                owner.try_publish(validator, finality)
            })
            .map_err(Into::into)
    }

    /// Consume the original validated journals once for the reducer's exact Apply.
    pub(crate) fn execute_retained_apply(
        &self,
        context: &wire::HeightContext,
        body_store: &mut V2BodyStore,
        retained: &mut RetainedBodyValidationService<OwnedNativeCarrierValidator>,
        task: &crate::sumeragi::v2_effects::ApplyTask,
    ) -> Result<PublishedApplyCompletion, V2ApplyError> {
        let published = self.publish_retained_task(
            context,
            body_store,
            retained,
            super::ExactApplyTaskRef::Ordinary(task),
        )?;
        let completion = crate::sumeragi::v2_effects::DurableApplyCompletion::new(
            task.id(),
            published.receipt().clone(),
            published.artifact().clone(),
        );
        Ok(PublishedApplyCompletion {
            completion,
            published,
        })
    }

    /// Keep the original publication beside a dedicated lifecycle Apply completion.
    pub(in crate::sumeragi) fn execute_retained_lifecycle_apply(
        &self,
        context: &wire::HeightContext,
        body_store: &mut V2BodyStore,
        retained: &mut RetainedBodyValidationService<OwnedNativeCarrierValidator>,
        task: super::LifecycleDecisionApplyTaskV1,
    ) -> Result<super::LifecycleDecisionApplyWorkerResultV1, V2ApplyError> {
        if !task.dispatch_identity.matches_height_context(context) {
            return Err(V2ApplyError::TaskMismatch);
        }
        let exact = match task.exact_lineage() {
            Some(super::LifecycleDecisionApplyLineageV1::Live) => {
                super::ExactApplyTaskRef::LifecycleLive(&task)
            }
            Some(super::LifecycleDecisionApplyLineageV1::Recovered) => {
                super::ExactApplyTaskRef::LifecycleRecovered(&task)
            }
            None => return Err(V2ApplyError::TaskMismatch),
        };
        let published = match self.publish_retained_task(context, body_store, retained, exact) {
            Ok(published) => published,
            Err(V2ApplyError::LocalValidation(refusal)) => {
                return Ok(super::LifecycleDecisionApplyWorkerResultV1::Deferred { task, refusal });
            }
            Err(error) => return Err(error),
        };
        Ok(super::LifecycleDecisionApplyWorkerResultV1::Applied(
            super::LifecycleDecisionApplyCompletionV1 {
                dispatch_identity: task.dispatch_identity,
                subject: task.subject,
                certificate: task.certificate,
                validated_receipt: task.validated_receipt,
                receipt: published.receipt().clone(),
                artifact: published.artifact().clone(),
                publication: super::LifecycleCarrierPublication::Actual(published),
            },
        ))
    }
}

/// The startup-created application service and every retained replay owner move together.
pub(crate) struct NativeApplyService {
    service: Arc<V2ApplyService>,
    validation: RetainedBodyValidationService<OwnedNativeCarrierValidator>,
}
impl std::ops::Deref for NativeApplyService {
    type Target = V2ApplyService;
    fn deref(&self) -> &Self::Target {
        &self.service
    }
}
impl NativeApplyService {
    /// Create one descriptor owner for this exact open store before marker replay.
    pub(crate) fn new(
        service: V2ApplyService,
        store: &V2BodyStore,
        context: VerifiedHeightContext,
    ) -> Result<Self, V2ApplyError> {
        let service = Arc::new(service);
        let validation = service.retained_validation_service(store, context)?;
        Ok(Self {
            service,
            validation,
        })
    }
    /// Replay with the same candidate owners later transferred into the live worker.
    pub(crate) fn revalidate_recovered_markers(
        &mut self,
        store: &mut V2BodyStore,
    ) -> Result<(), V2BodyStoreError> {
        store.revalidate_retained_markers(&mut self.validation)
    }
    /// Move both original owners into the one serialized worker.
    pub(crate) fn into_parts(
        self,
    ) -> (
        Arc<V2ApplyService>,
        RetainedBodyValidationService<OwnedNativeCarrierValidator>,
    ) {
        (self.service, self.validation)
    }
}
