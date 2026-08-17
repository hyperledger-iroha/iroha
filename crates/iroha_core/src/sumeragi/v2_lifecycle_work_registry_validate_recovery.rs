/// Non-forgeable successful-validation input accepted only by the adapter's
/// sealed direct-preview entry point.
///
/// Construction stays private to the exact Ready registry preflight. The only
/// consuming projection is used by `v2` and this value is never returned from
/// the registry-owned join.
#[must_use = "validated adapter authority must enter the sealed preview join"]
pub(crate) struct ReadyValidatedAdapterAuthority<'a> {
    tag: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    receipt: &'a ValidatedBodyReceipt,
}
/// Non-forgeable installed-Validate predecessor accepted only by the
/// adapter's sealed WAL-sign binding step.
///
/// The exact effect and pending binding remain borrowed from the closed Ready
/// completion. Only the adapter may consume this view, so no caller can supply
/// a detached Validate effect, causal root, or candidate statement.
#[must_use = "Validate predecessor authority must bind the sealed WAL Sign preview"]
pub(in crate::sumeragi) struct ReadyValidateSignPredecessorAuthority<'a> {
    effect: &'a AdapterEffect,
    pending: &'a PendingRuntimeEffectBinding,
    _linearity: ReadyValidateSignPredecessorLinearity,
}
struct ReadyValidateSignPredecessorLinearity;
impl Drop for ReadyValidateSignPredecessorLinearity {
    fn drop(&mut self) {}
}
impl ReadyValidateSignPredecessorAuthority<'_> {
    /// Project only the exact Prepare/Commit vote successor retained by the
    /// adapter preflight. No predecessor parts or certificate can escape.
    pub(in crate::sumeragi) fn project_successor(
        self,
        successor: &AdapterEffect,
        registered_prepare: Option<&RegisteredPrepareValidateSignCapability>,
    ) -> Option<PendingRuntimeEffectBinding> {
        let AdapterEffect::Sign {
            request: crate::sumeragi::v2::SignRequest::Vote(vote),
            ..
        } = successor
        else {
            return None;
        };
        match vote.phase {
            wire::GlobalPhase::Prepare if registered_prepare.is_none() => self
                .pending
                .project_validate_sign_prepare_successor(self.effect, successor),
            wire::GlobalPhase::Commit => self
                .pending
                .project_validate_sign_commit_successor(self.effect, successor)
                .or_else(|| {
                    self.pending
                        .project_validate_sign_commit_successor_with_registered_prepare(
                            self.effect,
                            successor,
                            registered_prepare?,
                        )
                }),
            wire::GlobalPhase::Prepare => None,
        }
    }
}
#[cfg(test)]
impl<'a> ReadyValidateSignPredecessorAuthority<'a> {
    /// Construct the same opaque view for focused adapter tests.
    pub(in crate::sumeragi) const fn for_test(
        effect: &'a AdapterEffect,
        pending: &'a PendingRuntimeEffectBinding,
    ) -> Self {
        Self {
            effect,
            pending,
            _linearity: ReadyValidateSignPredecessorLinearity,
        }
    }
}
impl<'a> ReadyValidatedAdapterAuthority<'a> {
    /// Consume the unforgeable registry authority inside the adapter module.
    pub(crate) fn into_parts(
        self,
    ) -> (
        EventTag,
        wire::ConsensusRound,
        wire::BlockSubject,
        &'a ValidatedBodyReceipt,
    ) {
        (self.tag, self.round, self.subject, self.receipt)
    }
}
/// Non-forgeable rejected-validation input accepted only by the adapter's
/// sealed direct-preview entry point.
///
/// Diagnostic text is deliberately absent. The registry constructs this value
/// only after proving the one canonical reducer-level rejection identity.
#[must_use = "rejected adapter authority must enter the sealed preview join"]
pub(crate) struct ReadyRejectedAdapterAuthority<'a> {
    tag: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    receipt: &'a DurableBodyReceipt,
}
impl<'a> ReadyRejectedAdapterAuthority<'a> {
    /// Consume the unforgeable registry authority inside the adapter module.
    pub(crate) fn into_parts(
        self,
    ) -> (
        EventTag,
        wire::ConsensusRound,
        wire::BlockSubject,
        &'a DurableBodyReceipt,
    ) {
        (self.tag, self.round, self.subject, self.receipt)
    }
}
/// Fixed dual-borrow result of joining one Ready Validate carrier to the
/// adapter's fully preflighted, still-inert publication.
///
/// The fields remain private and there is no extraction or commit operation.
/// Dropping this inert token releases both borrows without mutating either
/// subsystem.
#[allow(dead_code)]
#[must_use = "a Ready Validate adapter preview retains both authority borrows"]
pub(super) struct PreparedReadyDurableValidateAdapterPreview<'registry, 'adapter> {
    _registry: PreparedReadyDurableValidateExecution<'registry>,
    _adapter: PreparedReadyDurableValidateAdapterPublication<'adapter>,
}
/// Opaque post-fsync Validate-to-Sign authority retaining both subsystem
/// borrows until the sole coordinator/LedgerV1 publication consumes it.
#[allow(dead_code)]
#[must_use = "a persisted Validate Sign has not entered lifecycle publication"]
pub(super) struct PreparedReadyDurableValidatePersistedSignPreAdmission<'registry, 'adapter> {
    _registry: PreparedReadyDurableValidateExecution<'registry>,
    _adapter: PreparedReadyDurableValidatePersistedSign<'adapter>,
}
/// Pre-fsync live registry publication using the recovered-WAL exclusive
/// detached-parent/child-vacancy reservation.
///
/// The detached parent is deliberately non-restoring. The adapter half owns
/// one closed ordinary Sign carrier whose digest already matches the reserved
/// child. Dropping this token therefore requires restart and cannot resurrect
/// a volatile Validate row after the WAL may be durable.
#[must_use = "a live Validate-to-Sign registry publication awaits LedgerV1 fsync"]
pub(super) struct PreparedLiveValidateSignRegistryPublication<'registry, 'adapter> {
    reservation: LiveValidateSignRegistryReservation<'registry>,
    adapter: PreparedReadyDurableValidatePersistedSign<'adapter>,
}
/// Opaque fail-stop error from live Sign registry preparation.
#[must_use = "failed live Sign registry preparation retains post-WAL authority"]
pub(super) struct LiveValidateSignRegistryPublicationError<'registry, 'adapter> {
    _failure: LiveValidateSignRegistryPublicationFailure<'registry, 'adapter>,
}
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum LiveValidateSignRegistryPublicationFailure<'registry, 'adapter> {
    AdapterWork {
        _registry: PreparedReadyDurableValidateExecution<'registry>,
        _adapter: PreparedReadyDurableValidatePersistedSign<'adapter>,
    },
    InvalidCoordinates {
        _registry: PreparedReadyDurableValidateExecution<'registry>,
        _adapter: PreparedReadyDurableValidatePersistedSign<'adapter>,
    },
    Detach {
        _registry: PreparedReadyDurableValidateExecution<'registry>,
        _adapter: PreparedReadyDurableValidatePersistedSign<'adapter>,
    },
    Reservation {
        _reservation: LiveValidateSignRegistryReservation<'registry>,
        _adapter: PreparedReadyDurableValidatePersistedSign<'adapter>,
    },
}
/// One-shot authority for consuming the nested post-WAL Sign into closed
/// ordinary registry work.
///
/// Construction remains private to the fixed live publication transaction.
/// The replay module accepts this token only so no sibling can extract the
/// effect or pending binding from the post-fsync seal.
pub(in crate::sumeragi) struct LiveValidateSignWorkProjectionPermit {
    _linearity: LiveValidateSignWorkProjectionLinearity,
}
struct LiveValidateSignWorkProjectionLinearity;
impl Drop for LiveValidateSignWorkProjectionLinearity {
    fn drop(&mut self) {}
}
impl LiveValidateSignWorkProjectionPermit {
    fn new() -> Self {
        Self {
            _linearity: LiveValidateSignWorkProjectionLinearity,
        }
    }
}
/// Ownership-retaining failure from the fixed live-WAL Validate-to-Sign join.
#[allow(dead_code)]
#[must_use = "failed Validate Sign sealing still owns both subsystem borrows"]
pub(super) struct ReadyDurableValidateSignPreAdmissionError<'registry, 'adapter> {
    failure: ReadyDurableValidateSignPreAdmissionFailure<'registry, 'adapter>,
}
#[allow(dead_code, variant_size_differences, clippy::large_enum_variant)]
enum ReadyDurableValidateSignPreAdmissionFailure<'registry, 'adapter> {
    PreWal {
        _preview: PreparedReadyDurableValidateAdapterPreview<'registry, 'adapter>,
    },
    Wal {
        _registry: PreparedReadyDurableValidateExecution<'registry>,
        _error: ReadyDurableValidateSignWalError<'adapter>,
    },
}
/// Closed inert replay pre-admission for one exact invalid certified body report.
///
/// The Ready registry row and staged adapter rejection remain exclusively
/// borrowed. The report effect, derived child pending binding, and canonical
/// runtime evidence stay sealed in the adapter half; no installation or
/// execution surface exists on this token.
#[allow(dead_code)]
#[must_use = "invalid-body replay evidence has not entered lifecycle admission"]
pub(super) struct PreparedInvalidBodyReportReplayPreAdmission<'registry, 'adapter> {
    registry: PreparedReadyDurableValidateExecution<'registry>,
    adapter: PreparedInvalidBodyReportAdapterReplay<'adapter>,
}
/// Ownership-preserving failure from the fixed invalid-body replay join.
#[allow(dead_code)]
#[must_use = "failed invalid-body replay preparation retains both authority borrows"]
pub(super) struct InvalidBodyReportReplayPreAdmissionError<'registry, 'adapter> {
    preview: PreparedReadyDurableValidateAdapterPreview<'registry, 'adapter>,
}
impl PreparedReadyDurableValidateAdapterPreview<'_, '_> {
    /// Project one exact no-successor cut without exposing the retained body.
    ///
    /// The transition module supplies its private one-shot permit. The frame is
    /// derived from the still-installed completion, and only inactive or
    /// no-effect branches can return the opaque projection.
    pub(super) fn project_no_successor_for_body_transition(
        &self,
        permit: SealedValidateNoSuccessorProjectionPermit,
        lease: &TurnLease,
    ) -> Result<SealedValidateNoSuccessorProjection, SealedValidateTerminalProjectionError> {
        if !self._registry.matches_exact_lease(lease) {
            return Err(SealedValidateTerminalProjectionError::ForeignParent);
        }
        let release_consensus_reservation = sealed_validate_no_successor_reservation(
            self._adapter.kind(),
            self._registry.outcome_kind,
        )?;
        let completion = self
            ._registry
            .completion()
            .ok_or(SealedValidateTerminalProjectionError::InvalidCarrier)?;
        let parent_payload = durable_validate_body_payload(&completion.incumbent.durable_receipt)
            .filter(|payload| {
                super::body_pipeline_transition::durable_validate_payload_is_exact(
                    lease.key(),
                    *payload,
                )
            })
            .ok_or(SealedValidateTerminalProjectionError::InvalidCarrier)?;
        if completion.outcome.durable_body() != &completion.incumbent.durable_receipt {
            return Err(SealedValidateTerminalProjectionError::InvalidCarrier);
        }
        Ok(SealedValidateNoSuccessorProjection::from_registry(
            permit,
            lease.clone(),
            parent_payload,
            release_consensus_reservation,
        ))
    }
}
impl<'registry, 'adapter> PreparedReadyDurableValidateAdapterPreview<'registry, 'adapter> {
    /// Consume only the exact validated Persist branch into a real post-fsync
    /// vote-sign seal.
    ///
    /// The registry mints the predecessor authority directly from its still-
    /// installed completion. A branch or lineage mismatch returns the whole
    /// dual-borrow preview before WAL I/O. Once append is attempted, every
    /// error is opaque and restart-only.
    #[allow(dead_code)]
    #[allow(clippy::result_large_err)]
    pub(super) fn seal_live_wal_validate_sign(
        self,
    ) -> Result<
        PreparedReadyDurableValidatePersistedSignPreAdmission<'registry, 'adapter>,
        ReadyDurableValidateSignPreAdmissionError<'registry, 'adapter>,
    > {
        let Self {
            _registry: registry,
            _adapter: adapter,
        } = self;
        let Some(predecessor) = registry.validate_sign_predecessor_authority() else {
            return Err(ReadyDurableValidateSignPreAdmissionError {
                failure: ReadyDurableValidateSignPreAdmissionFailure::PreWal {
                    _preview: Self {
                        _registry: registry,
                        _adapter: adapter,
                    },
                },
            });
        };
        let adapter = match adapter.bind_validate_sign_predecessor(predecessor) {
            Ok(adapter) => adapter,
            Err(adapter) => {
                return Err(ReadyDurableValidateSignPreAdmissionError {
                    failure: ReadyDurableValidateSignPreAdmissionFailure::PreWal {
                        _preview: Self {
                            _registry: registry,
                            _adapter: adapter,
                        },
                    },
                });
            }
        };
        match adapter.append_live_wal() {
            Ok(adapter) => Ok(PreparedReadyDurableValidatePersistedSignPreAdmission {
                _registry: registry,
                _adapter: adapter,
            }),
            Err(error) => Err(ReadyDurableValidateSignPreAdmissionError {
                failure: ReadyDurableValidateSignPreAdmissionFailure::Wal {
                    _registry: registry,
                    _error: error,
                },
            }),
        }
    }
    /// Consume only the exact Ready/rejected report preview into replay pre-admission.
    ///
    /// All inputs are read from the still-installed completion and staged
    /// adapter publication. Failure reconstructs the complete dual-borrow
    /// preview, while success remains publication-inert.
    #[allow(clippy::result_large_err)]
    #[cfg_attr(
        test,
        expect(dead_code, reason = "invalid-body atomic-publication gap")
    )]
    pub(super) fn seal_invalid_body_report_replay(
        self,
    ) -> Result<
        PreparedInvalidBodyReportReplayPreAdmission<'registry, 'adapter>,
        InvalidBodyReportReplayPreAdmissionError<'registry, 'adapter>,
    > {
        let Self {
            _registry: registry,
            _adapter: adapter,
        } = self;
        let Some(completion) = registry.completion() else {
            return Err(InvalidBodyReportReplayPreAdmissionError {
                preview: Self {
                    _registry: registry,
                    _adapter: adapter,
                },
            });
        };
        if registry.outcome_kind != ReadyDurableValidateOutcomeKind::Rejected
            || completion.outcome.validated_receipt().is_some()
            || completion.outcome.rejection_identity()
                != Some(&BodyValidationRejectionIdentity::Rejected)
            || completion.outcome.missing_merge_sidecar().is_some()
        {
            return Err(InvalidBodyReportReplayPreAdmissionError {
                preview: Self {
                    _registry: registry,
                    _adapter: adapter,
                },
            });
        }
        let validate_origin = completion.incumbent.replay_evidence.clone();
        let adapter = match adapter.seal_invalid_body_report_replay(
            validate_origin,
            &completion.incumbent.effect,
            &completion.incumbent.pending,
            &completion.incumbent.durable_receipt,
        ) {
            Ok(adapter) => adapter,
            Err(adapter) => {
                return Err(InvalidBodyReportReplayPreAdmissionError {
                    preview: Self {
                        _registry: registry,
                        _adapter: adapter,
                    },
                });
            }
        };
        let sealed = PreparedInvalidBodyReportReplayPreAdmission { registry, adapter };
        debug_assert!(sealed.validates());
        Ok(sealed)
    }
}
impl PreparedInvalidBodyReportReplayPreAdmission<'_, '_> {
    fn validates(&self) -> bool {
        self.registry.completion().is_some_and(|completion| {
            self.registry.outcome_kind == ReadyDurableValidateOutcomeKind::Rejected
                && completion.outcome.rejection_identity()
                    == Some(&BodyValidationRejectionIdentity::Rejected)
                && completion.outcome.validated_receipt().is_none()
                && completion.outcome.missing_merge_sidecar().is_none()
                && self.adapter.exactly_matches(
                    &completion.incumbent.effect,
                    &completion.incumbent.pending,
                    &completion.incumbent.durable_receipt,
                )
        })
    }
    /// Project the invalid-body child while every raw part remains sealed.
    ///
    /// Only the body-transition module can mint the one-shot permit. Candidate
    /// projection occurs inside this adapter/registry join and the returned
    /// value has no field or candidate accessor outside that module.
    pub(super) fn project_for_body_transition(
        &self,
        permit: SealedInvalidBodyReportProjectionPermit,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
    ) -> Result<SealedInvalidBodyReportProjection, SealedValidateTerminalProjectionError> {
        if !self.registry.matches_exact_lease(lease) {
            return Err(SealedValidateTerminalProjectionError::ForeignParent);
        }
        if !self.validates() {
            return Err(SealedValidateTerminalProjectionError::InvalidCarrier);
        }
        let completion = self
            .registry
            .completion()
            .ok_or(SealedValidateTerminalProjectionError::InvalidCarrier)?;
        let parent_payload = durable_validate_body_payload(&completion.incumbent.durable_receipt)
            .filter(|payload| {
                super::body_pipeline_transition::durable_validate_payload_is_exact(
                    lease.key(),
                    *payload,
                )
            })
            .ok_or(SealedValidateTerminalProjectionError::InvalidCarrier)?;
        let candidate = self
            .adapter
            .project_invalid_body_report_candidate(
                &permit,
                verified,
                &completion.incumbent.effect,
                &completion.incumbent.pending,
                &completion.incumbent.durable_receipt,
            )
            .map_err(SealedValidateTerminalProjectionError::Projection)?;
        let expected_slot = PhysicalSlotId::for_capacity(CapacityClass::Consensus, 0);
        let (projected_slots, projected_universe, projected_consumed) = candidate
            .physical_geometry
            .normalized()
            .map_err(|_| SealedValidateTerminalProjectionError::InvalidCarrier)?;
        let mut context = [0_u8; 32];
        context.copy_from_slice(completion.incumbent.durable_receipt.context_id().0.as_ref());
        let active_context = LifecycleContext::new(
            LifecycleDigest::new(context),
            completion.incumbent.durable_receipt.round().height,
        );
        if candidate.causal_root != lease.owner().causal_root()
            || candidate.work_class != LifecycleWorkClass::InvalidBodyReport
            || candidate.stage.kind() != LifecycleStageKind::ReportInvalidBody
            || candidate.stage.predecessor_scope() != PredecessorScope::Independent
            || candidate.initial_state != InitialLifecycleState::Ready
            || candidate.reconstruction_source != lease.owner().causal_root().digest()
            || candidate.payload != DurablePayloadReference::None
            || !candidate.replay_authority_is_exact(active_context)
            || candidate.producer_turn.is_some()
            || projected_slots.len() != 1
            || !projected_slots.contains_key(&expected_slot)
            || projected_universe.len() != 1
            || !projected_universe.contains(&expected_slot)
            || projected_consumed != projected_universe
        {
            return Err(SealedValidateTerminalProjectionError::InvalidCarrier);
        }
        Ok(SealedInvalidBodyReportProjection::from_registry(
            permit,
            lease.clone(),
            candidate,
            parent_payload,
        ))
    }
}
impl PreparedReadyDurableValidatePersistedSignPreAdmission<'_, '_> {
    /// Project the exact post-WAL Sign child while all effect, pending, replay,
    /// and durable-body authority remains nested in this fixed join.
    pub(super) fn project_for_body_transition(
        &self,
        permit: SealedValidateSignProjectionPermit,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
    ) -> Result<SealedValidateSignProjection, SealedValidateTerminalProjectionError> {
        if !self._registry.matches_exact_lease(lease) {
            return Err(SealedValidateTerminalProjectionError::ForeignParent);
        }
        let completion = self
            ._registry
            .completion()
            .filter(|completion| {
                self._registry.outcome_kind == ReadyDurableValidateOutcomeKind::Validated
                    && completion.outcome.validated_receipt().is_some()
                    && completion.outcome.rejection_identity().is_none()
                    && completion.outcome.missing_merge_sidecar().is_none()
            })
            .ok_or(SealedValidateTerminalProjectionError::InvalidCarrier)?;
        let parent_payload = durable_validate_body_payload(&completion.incumbent.durable_receipt)
            .filter(|payload| {
                super::body_pipeline_transition::durable_validate_payload_is_exact(
                    lease.key(),
                    *payload,
                )
            })
            .ok_or(SealedValidateTerminalProjectionError::InvalidCarrier)?;
        let candidate = self
            ._adapter
            .project_validate_sign_candidate(&permit, verified)
            .map_err(SealedValidateTerminalProjectionError::Projection)?;
        let expected_slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let (projected_slots, projected_universe, projected_consumed) = candidate
            .physical_geometry
            .normalized()
            .map_err(|_| SealedValidateTerminalProjectionError::InvalidCarrier)?;
        let expected_stage = match candidate.key.phase() {
            LifecyclePhase::Prepare => LifecycleStageKind::SignPrepareVote,
            LifecyclePhase::Commit => LifecycleStageKind::SignCommitVote,
            _ => return Err(SealedValidateTerminalProjectionError::InvalidCarrier),
        };
        let mut context = [0_u8; 32];
        context.copy_from_slice(completion.incumbent.durable_receipt.context_id().0.as_ref());
        let active_context = LifecycleContext::new(
            LifecycleDigest::new(context),
            completion.incumbent.durable_receipt.round().height,
        );
        if candidate.causal_root != lease.owner().causal_root()
            || candidate.work_class != LifecycleWorkClass::SignVote
            || candidate.stage.kind() != expected_stage
            || candidate.stage.predecessor_scope() != PredecessorScope::Independent
            || candidate.initial_state != InitialLifecycleState::Ready
            || candidate.reconstruction_source != lease.owner().causal_root().digest()
            || candidate.payload != DurablePayloadReference::None
            || !candidate.replay_authority_is_exact(active_context)
            || candidate.producer_turn.is_some()
            || projected_slots.len() != 1
            || !projected_slots.contains_key(&expected_slot)
            || projected_universe.len() != 1
            || !projected_universe.contains(&expected_slot)
            || projected_consumed != projected_universe
        {
            return Err(SealedValidateTerminalProjectionError::InvalidCarrier);
        }
        Ok(SealedValidateSignProjection::from_registry(
            permit,
            lease.clone(),
            candidate,
            parent_payload,
        ))
    }
}
impl<'registry, 'adapter>
    PreparedReadyDurableValidatePersistedSignPreAdmission<'registry, 'adapter>
{
    /// Prepare the exact detached-parent and reserved-child registry half.
    ///
    /// The adapter first consumes its nested replay seal into closed ordinary
    /// Sign work without exposing parts. Only after every coordinate, digest,
    /// and vacancy check succeeds is the existing restorable recovered-WAL cut
    /// converted into a non-restoring live reservation.
    #[allow(clippy::result_large_err)]
    pub(super) fn prepare_registry_publication(
        self,
        lease: &TurnLease,
        child_ordinal: u128,
        child_slot: PhysicalSlotId,
        child_digest: LifecycleDigest,
    ) -> Result<
        PreparedLiveValidateSignRegistryPublication<'registry, 'adapter>,
        LiveValidateSignRegistryPublicationError<'registry, 'adapter>,
    > {
        let Self {
            _registry: registry,
            _adapter: adapter,
        } = self;
        let adapter =
            match adapter.prepare_registry_work(LiveValidateSignWorkProjectionPermit::new()) {
                Ok(adapter) => adapter,
                Err(adapter) => {
                    return Err(LiveValidateSignRegistryPublicationError {
                        _failure: LiveValidateSignRegistryPublicationFailure::AdapterWork {
                            _registry: registry,
                            _adapter: adapter,
                        },
                    });
                }
            };
        let child_address = ConcreteWorkAddress::new(lease.owner(), child_ordinal, child_slot);
        let coordinates_are_exact = registry.matches_exact_lease(lease)
            && registry.outcome_kind == ReadyDurableValidateOutcomeKind::Validated
            && registry.completion().is_some()
            && child_address.is_some_and(|address| {
                address != registry.address
                    && address.owner == registry.address.owner
                    && address.ordinal == child_ordinal
                    && address.slot == PhysicalSlotId::for_capacity(CapacityClass::Effect, 0)
                    && !registry.registry.entries.contains_key(&address)
            })
            && adapter.registry_work_matches(
                lease.owner(),
                child_ordinal,
                child_slot,
                child_digest,
            );
        if !coordinates_are_exact {
            return Err(LiveValidateSignRegistryPublicationError {
                _failure: LiveValidateSignRegistryPublicationFailure::InvalidCoordinates {
                    _registry: registry,
                    _adapter: adapter,
                },
            });
        }
        let child_address = child_address.expect("exact coordinates retain one child address");
        let cut = match registry.into_recovered_wal_validate_registry_cut() {
            Ok(cut) => cut,
            Err(registry) => {
                return Err(LiveValidateSignRegistryPublicationError {
                    _failure: LiveValidateSignRegistryPublicationFailure::Detach {
                        _registry: registry,
                        _adapter: adapter,
                    },
                });
            }
        };
        let mut reservation = cut
            .into_live_validate_sign_reservation()
            .expect("validated recovered cut transfers both retained fields");
        if !reservation.bind_exact_child(child_address, child_digest) {
            return Err(LiveValidateSignRegistryPublicationError {
                _failure: LiveValidateSignRegistryPublicationFailure::Reservation {
                    _reservation: reservation,
                    _adapter: adapter,
                },
            });
        }
        Ok(PreparedLiveValidateSignRegistryPublication {
            reservation,
            adapter,
        })
    }
}
impl PreparedLiveValidateSignRegistryPublication<'_, '_> {
    /// Complete the already-fsynced registry and adapter publication.
    ///
    /// All checks ran before LedgerV1 persistence. This method contains only
    /// the fixed reserved-row insertion and staged adapter swaps.
    pub(super) fn publish_after_ledger_fsync(self) {
        self.adapter
            .install_registry_and_commit_adapter(self.reservation);
    }
}
/// Ownership-preserving failure from the fixed Ready Validate adapter join.
#[allow(dead_code)]
#[must_use = "a failed Ready Validate preview still retains its registry cut"]
pub(super) struct ReadyDurableValidateAdapterPreviewError<'registry> {
    _registry: PreparedReadyDurableValidateExecution<'registry>,
    _failure: ReadyDurableValidateAdapterPreviewFailure,
}
#[allow(dead_code, variant_size_differences, clippy::large_enum_variant)]
enum ReadyDurableValidateAdapterPreviewFailure {
    RegistryAuthority,
    Adapter(crate::sumeragi::v2::AdapterError),
}
// DURABLE_VALIDATE_ASYNC_HANDOFF_DECLARATIONS_BEGIN
/// Move-only registry authority detached from one exact durable Validate row.
///
/// All fields are private. The exact address and incumbent digest exist only
/// to recheck the unchanged registry row after storage work; the validation
/// service can neither decompose this value nor derive scheduling authority
/// from it.
#[derive(Debug)]
#[must_use = "detached durable Validate authority must be executed or retained"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct DetachedDurableValidateExecution {
    address: ConcreteWorkAddress,
    incumbent_digest: LifecycleDigest,
    tag: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    durable_receipt: DurableBodyReceipt,
    expected_manifest_hash: HashOf<wire::PayloadManifest>,
    causal_lifecycle_key: Hash,
    candidate_statement: Option<RuntimeCandidateSemanticStatement>,
    lifecycle_key: LifecycleKey,
    lifecycle_stage: LifecycleStage,
}
/// Move-only result of executing one detached durable Validate request.
///
/// The request remains sealed beside the body-store-minted closed outcome, so
/// every later registry check retains the original physical and durable
/// authority instead of accepting caller-supplied coordinates.
#[derive(Debug)]
#[must_use = "executed durable Validate authority has not been reattached"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct ExecutedDurableValidateExecution {
    request: DetachedDurableValidateExecution,
    outcome: DurableBodyValidationOutcome,
}
/// Borrow-bound exact-row reattachment of one executed Validate outcome.
///
/// Reattachment and drop mutate nothing. This token deliberately exposes no
/// registry replacement or coordinator publication operation.
#[must_use = "reattached durable Validate outcome has not entered atomic publication"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedDurableValidateCompletion<'a> {
    _registry: &'a mut ConcreteLifecycleWorkRegistry,
    executed: ExecutedDurableValidateExecution,
}
// DURABLE_VALIDATE_ASYNC_HANDOFF_DECLARATIONS_END
// DURABLE_VALIDATE_WAIT_DISPATCH_DECLARATIONS_BEGIN
/// Move-only wake authority paired only with its exact detached validation.
///
/// The token is deliberately private: neither its source nor observed
/// generation can be separated from the request and used to wake another
/// lifecycle row.
#[derive(Debug)]
#[cfg_attr(not(test), allow(dead_code))]
struct DurableValidateWakeAuthority {
    wait_token: WaitToken,
}
/// One exact durable validation whose claimed lifecycle lease has already
/// become an explicit external wait.
#[derive(Debug)]
#[must_use = "a durable Validate dispatch must be executed or retained"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct DurableValidateDispatch {
    request: DetachedDurableValidateExecution,
    wake: DurableValidateWakeAuthority,
}
/// Closed validation result retaining the exact external-wait authority.
///
/// The sole volatile completion transaction reattaches `executed`, installs
/// its executable typed outcome carrier, and publishes `wake` at the same
/// physical address atomically. Sidecar deferral retains this value intact.
#[derive(Debug)]
#[must_use = "an executed durable Validate dispatch awaits typed completion publication"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct ExecutedDurableValidateDispatch {
    executed: ExecutedDurableValidateExecution,
    wake: DurableValidateWakeAuthority,
}
// DURABLE_VALIDATE_WAIT_DISPATCH_DECLARATIONS_END
// DURABLE_VALIDATE_VOLATILE_COMPLETION_DECLARATIONS_BEGIN
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DurableValidateOutcomeKind {
    Validated,
    Rejected,
    DeferredMergeSidecar,
}
/// Sealed exact authority for one Waiting-to-Ready Validate publication.
///
/// Construction is private to exact registry reattachment. The coordinator
/// receives this typed projection instead of caller-supplied address, wait, or
/// digest parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DurableValidateCompletionAuthority {
    address: ConcreteWorkAddress,
    incumbent_digest: LifecycleDigest,
    replacement_digest: Option<LifecycleDigest>,
    wait_token: WaitToken,
    outcome_kind: DurableValidateOutcomeKind,
    lifecycle_key: LifecycleKey,
    lifecycle_stage: LifecycleStage,
    payload: DurablePayloadReference,
}
/// Typed location of one published successful validation carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PublishedValidated {
    location: DurableValidatePublishedLocation,
}
/// Typed location of one published deterministic-rejection carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PublishedRejected {
    location: DurableValidatePublishedLocation,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
struct DurableValidatePublishedLocation {
    address: ConcreteWorkAddress,
    incumbent_digest: LifecycleDigest,
    replacement_digest: LifecycleDigest,
}
/// Move-only merge-sidecar dependency retaining its exact executed dispatch.
#[derive(Debug)]
#[must_use = "a deferred Validate dispatch still requires sealed sidecar registration"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct DeferredDurableValidateDispatch {
    dispatch: ExecutedDurableValidateDispatch,
}
/// Closed result of the volatile Validate completion transaction.
#[derive(Debug)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
#[must_use = "published or deferred Validate completion authority must be retained"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) enum DurableValidateCompletionPublication {
    /// The exact validated carrier and logical Ready replacement committed.
    PublishedValidated(PublishedValidated),
    /// The exact deterministic rejection carrier and Ready replacement committed.
    PublishedRejected(PublishedRejected),
    /// Merge-sidecar absence left both volatile sides exactly Waiting/original.
    DeferredMergeSidecar(DeferredDurableValidateDispatch),
}
/// Borrow-bound exact executed-dispatch reattachment.
///
/// Drop changes nothing. The only consuming paths either return the dispatch
/// with a typed failure, retain it in a deferral, or stage the specialized
/// unwind-safe same-address carrier conversion.
#[must_use = "an exact executed Validate reattachment has not been published"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedExecutedDurableValidateCompletion<'a> {
    registry: &'a mut ConcreteLifecycleWorkRegistry,
    dispatch: ExecutedDurableValidateDispatch,
    authority: DurableValidateCompletionAuthority,
}
/// Armed same-address Validate conversion restored automatically on unwind.
#[must_use = "the staged Validate carrier must commit or roll back"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct StagedDurableValidateCompletion<'a> {
    entries: &'a mut BTreeMap<ConcreteWorkAddress, ConcreteLifecycleWork>,
    address: ConcreteWorkAddress,
    request: Option<DetachedDurableValidateExecution>,
    wake: Option<DurableValidateWakeAuthority>,
    publication: PublishedDurableValidateCompletion,
    armed: bool,
}
/// Infallible Copy metadata returned when an armed carrier is committed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PublishedDurableValidateCompletion {
    /// Exact successful-validation publication metadata.
    Validated(PublishedValidated),
    /// Exact deterministic-rejection publication metadata.
    Rejected(PublishedRejected),
}
// DURABLE_VALIDATE_VOLATILE_COMPLETION_DECLARATIONS_END
/// Receipt-bound successful validation of one closed Validate carrier.
///
/// The live registry row remains untouched and exclusively borrowed. The
/// deterministic completion digest is ready for a future same-address
/// coordinator replacement, but this token deliberately exposes no registry
/// installation, removal, or commit operation.
#[must_use = "a validated body completion has not entered its atomic publication"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedValidatedBodyCompletion<'a> {
    _registry: &'a mut ConcreteLifecycleWorkRegistry,
    address: ConcreteWorkAddress,
    incumbent_digest: LifecycleDigest,
    replacement_digest: LifecycleDigest,
    validated_receipt: ValidatedBodyReceipt,
    tag: EventTag,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
}
/// Closed Apply-WAL replay preflight retained until future exact admission.
///
/// The exact receipt-bound Validate completion that supplied the Apply body
/// frame remains attached. No field, effect, pending binding, receipt, or
/// replay parts can be extracted, and dropping the token publishes no work.
#[must_use = "live WAL replay evidence has not entered lifecycle admission"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedLiveWalReplayPreAdmission<'a> {
    _persisted: SealedLiveWalPersistedEffectV1,
    _origin: LiveWalReplayPreAdmissionOrigin<'a>,
}
#[allow(dead_code, variant_size_differences, clippy::large_enum_variant)]
enum LiveWalReplayPreAdmissionOrigin<'a> {
    Apply(PreparedValidatedBodyCompletion<'a>),
}
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum LiveWalReplayPreAdmissionFailure<'a> {
    Apply {
        _completion: PreparedValidatedBodyCompletion<'a>,
        _persisted: SealedLiveWalPersistedEffectV1,
        _pending: PendingRuntimeEffectBinding,
    },
}
/// Ownership-preserving failure from exact live-WAL replay preflight.
pub(super) struct LiveWalReplayPreAdmissionError<'a> {
    _failure: LiveWalReplayPreAdmissionFailure<'a>,
}
/// Move-only Validate projection sealed under its closed durable Store parent.
///
/// No field can be extracted. Its only cross-module consuming path retains the
/// whole token inside inert coordinator staging; no registry installation or
/// publication exists in this tranche.
///
/// TODO: Add publication only when the registry, coordinator, durable-catalog,
/// and adapter cuts can commit together.
#[must_use = "a sealed Validate successor has not entered a parent-to-child transaction"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedDurableStoreValidateSuccessor<'a> {
    _registry: &'a mut ConcreteLifecycleWorkRegistry,
    _store_address: ConcreteWorkAddress,
    _validate_effect: AdapterEffect,
    _validate_digest: LifecycleDigest,
    _validate_pending: PendingRuntimeEffectBinding,
    _durable_body: DurableBodyReceipt,
    _expected_manifest_hash: HashOf<wire::PayloadManifest>,
    _replay_evidence: CertifiedValidateReplayEvidenceV1,
}
/// Move-only Store-successor projection sealed under its closed Fetch parent.
///
/// The projected pending binding never escapes this token. In particular,
/// callers cannot clone or install it independently of the still-borrowed
/// completion. Its inert coordinator staging path retains this entire token;
/// no child installation or publication is exposed.
///
/// TODO: Add publication only with a typed output from the real checked-dequeue
/// witness; never add a constructor from raw response parts.
#[must_use = "a sealed Store successor has not entered a parent-to-child transaction"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedCertifiedFetchStoreSuccessor<'a> {
    _registry: &'a mut ConcreteLifecycleWorkRegistry,
    _completion_address: ConcreteWorkAddress,
    _store_effect: AdapterEffect,
    _store_digest: LifecycleDigest,
    _store_pending: PendingRuntimeEffectBinding,
    _durable_body: DurableBodyReceipt,
    _expected_manifest_hash: HashOf<wire::PayloadManifest>,
    _replay_evidence: CertifiedStoreReplayEvidenceV1,
}
/// Closed recovered-WAL Fetch-to-Store registry/adapter successor.
///
/// The installed dedicated Fetch carrier remains borrowed until publication.
/// The child projection is body-frame-bound and the adapter preview keeps the
/// serialized runtime exclusively borrowed. Dropping this token changes no
/// registry or adapter state.
#[must_use = "recovered Decision Store successor has not been published"]
pub(super) struct PreparedRecoveredDecisionFetchStoreSuccessor<'registry, 'adapter> {
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    fetch_address: ConcreteWorkAddress,
    store_address: ConcreteWorkAddress,
    store: super::wal_recovery::RecoveredDecisionFetchStoreProjectionV1,
    adapter: crate::sumeragi::v2::PreparedRecoveredDecisionFetchStoreAdapterV1<'adapter>,
}
/// Closed recovered Sign-to-Broadcast registry/adapter successor.
///
/// The claimed Sign stays installed until LedgerV1 publication. The child is
/// projected only from the adapter-authenticated signature and the exact WAL
/// carrier, while the preview keeps the serialized runtime borrowed.
#[must_use = "recovered signed Broadcast successor has not been published"]
pub(super) struct PreparedRecoveredLifecycleSignBroadcastSuccessor<'registry, 'adapter> {
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    sign_address: ConcreteWorkAddress,
    broadcast_address: ConcreteWorkAddress,
    broadcast: super::wal_recovery::RecoveredLifecycleSignedBroadcastProjectionV1,
    verified: VerifiedHeightContext,
    adapter: crate::sumeragi::v2::PreparedRecoveredLifecycleSignAdapterCompletionV1<'adapter>,
}
/// Closed recovered Sign successor retaining Broadcast plus one WAL Vote Sign.
///
/// Child addresses are intentionally absent at this point. The lifecycle
/// transition must first admit both opaque candidates into one unpublished
/// coordinator copy, then bind this token to those exact fresh rows.
#[must_use = "combined recovered Sign successor has not entered durable staging"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedRecoveredLifecycleSignBroadcastAndSignSuccessor<'registry, 'adapter> {
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    sign_address: ConcreteWorkAddress,
    successor: RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
    verified: VerifiedHeightContext,
    adapter: crate::sumeragi::v2::PreparedRecoveredLifecycleSignAdapterCompletionV1<'adapter>,
}
/// Combined registry successor rebound to both exact staged child rows.
///
/// Dropping this before LedgerV1 publication leaves the original Sign carrier
/// installed. Only the post-fsync commit may split the opaque combined
/// projection into its two concrete registry entries.
#[must_use = "bound combined recovered Sign successor has not been published"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct BoundRecoveredLifecycleSignBroadcastAndSignSuccessor<'registry, 'adapter> {
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    sign_address: ConcreteWorkAddress,
    broadcast_address: ConcreteWorkAddress,
    next_sign_address: ConcreteWorkAddress,
    successor: RecoveredLifecycleSignedBroadcastAndSignProjectionV1,
    verified: VerifiedHeightContext,
    adapter: crate::sumeragi::v2::PreparedRecoveredLifecycleSignAdapterCompletionV1<'adapter>,
}
/// Pre-fsync failure to join a claimed recovered Sign to its signed Broadcast.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RecoveredLifecycleSignBroadcastPreparationErrorV1 {
    /// The claimed lease or dispatch key no longer names the installed Sign.
    InvalidSignCarrier,
    /// The adapter-authenticated Broadcast does not descend from that carrier.
    InvalidBroadcastProjection,
    /// The deterministic Consensus child address collides with installed work.
    ChildCollision,
}
/// Pre-fsync failure to retain both children of one recovered `Signed` event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) enum RecoveredLifecycleSignBroadcastAndSignPreparationErrorV1 {
    /// The claimed lease or dispatch key no longer names the installed Sign.
    InvalidSignCarrier,
    /// The adapter/WAL/body projection is not the exact two-child successor.
    InvalidCombinedProjection,
    /// Either staged child address collides with process-local work.
    ChildCollision,
}
/// Pre-fsync failure to join recovered carrier, body, adapter, and child address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RecoveredDecisionFetchStorePreparationErrorV1 {
    /// The claimed lease or dispatch key no longer names the installed Fetch.
    InvalidFetchCarrier,
    /// The body authority could not be bound to the recovered Fetch event.
    InvalidBody,
    /// The reducer-derived Store effect did not preserve recovered WAL lineage.
    InvalidStoreProjection,
    /// The deterministic child address collides with installed work.
    ChildCollision,
}
/// Closed failure while projecting a sealed body-stage successor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SealedBodySuccessorProjectionError {
    /// The retained registry parent is not the lease's exact owner/ordinal/slot.
    ForeignParent,
    /// The move-only successor no longer matches its retained parent or body frame.
    InvalidCarrier,
    /// Authenticated replay projection rejected the verified height context.
    Projection(AdapterEffectAdmissionError),
}
/// Closed failure inventory for sealed terminal Validate projections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SealedValidateTerminalProjectionError {
    /// The retained completion is not the supplied coordinator parent.
    ForeignParent,
    /// The installed completion or its nested adapter seal is inconsistent.
    InvalidCarrier,
    /// This adapter branch cannot enter the requested terminal edge.
    InvalidBranch,
    /// Canonical report projection rejected the verified height context.
    Projection(AdapterEffectAdmissionError),
}
const fn sealed_validate_no_successor_reservation(
    publication: ReadyDurableValidateAdapterPublicationKind,
    outcome: ReadyDurableValidateOutcomeKind,
) -> Result<bool, SealedValidateTerminalProjectionError> {
    match (publication, outcome) {
        (
            ReadyDurableValidateAdapterPublicationKind::ValidatedInactive
            | ReadyDurableValidateAdapterPublicationKind::ValidatedNoEffect,
            ReadyDurableValidateOutcomeKind::Validated,
        ) => Ok(false),
        (
            ReadyDurableValidateAdapterPublicationKind::RejectedInactive
            | ReadyDurableValidateAdapterPublicationKind::RejectedNoEffect,
            ReadyDurableValidateOutcomeKind::Rejected,
        ) => Ok(true),
        (
            ReadyDurableValidateAdapterPublicationKind::ValidatedBusy
            | ReadyDurableValidateAdapterPublicationKind::ValidatedApply
            | ReadyDurableValidateAdapterPublicationKind::ValidatedPersist
            | ReadyDurableValidateAdapterPublicationKind::RejectedBusy
            | ReadyDurableValidateAdapterPublicationKind::RejectedReport,
            _,
        ) => Err(SealedValidateTerminalProjectionError::InvalidBranch),
        (
            ReadyDurableValidateAdapterPublicationKind::ValidatedInactive
            | ReadyDurableValidateAdapterPublicationKind::ValidatedNoEffect
            | ReadyDurableValidateAdapterPublicationKind::RejectedInactive
            | ReadyDurableValidateAdapterPublicationKind::RejectedNoEffect,
            _,
        ) => Err(SealedValidateTerminalProjectionError::InvalidCarrier),
    }
}
/// Borrow-bound registry conversion prepared before the exact queue CAS.
///
/// Preparation is read-only. Dropping this value therefore leaves every map
/// allocation, key, and move-only incumbent untouched. This token has no
/// dequeue commit; it must first consume a store-minted durable receipt whose
/// complete response and body bindings match this preflight.
#[must_use = "prepared completion conversion has not observed a successful queue CAS"]
pub(super) struct PreparedCertifiedFetchCompletion<'a> {
    registry: &'a mut ConcreteLifecycleWorkRegistry,
    location: CertifiedFetchWaitingLocation,
    ingress_identity: PendingFairIngressIdentity,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    response_round: wire::ConsensusRound,
    response_subject: wire::BlockSubject,
    response_manifest_hash: HashOf<wire::PayloadManifest>,
    authenticated_responder: PeerId,
    replay_origin: AuthenticatedCertifiedFetchReplayOriginV1,
}
/// Receipt-bound completion conversion authorized to consume one exact
/// checked-dequeue result.
///
/// This is the sole owner of the post-CAS registry commit. Construction
/// consumes the drop-inert selector preflight plus a sealed body-store receipt;
/// neither raw response parts nor a caller-minted body acknowledgement are
/// accepted.
#[must_use = "durable completion conversion has not observed a successful queue CAS"]
pub(super) struct PreparedDurableCertifiedFetchCompletion<'a> {
    registry: &'a mut ConcreteLifecycleWorkRegistry,
    location: CertifiedFetchWaitingLocation,
    ingress_identity: PendingFairIngressIdentity,
    request_hash: HashOf<wire::CertifiedBodyRequest>,
    response_hash: HashOf<wire::CertifiedBodyResponse>,
    authenticated_responder: PeerId,
    durable_receipt: DurableCertifiedFetchBodyReceipt,
    replay_evidence: CertifiedFetchReplayEvidenceV1,
    ready_projection: DurableCertifiedFetchReplayProjectionV1,
}
/// Failure from the registry-before-ledger publication boundary.
pub(super) enum RegistryPublicationError<E> {
    /// Exact-address installation failed before publication was attempted.
    Install(RegistryError, ConcreteLifecycleWork),
    /// Durable publication failed and the just-installed work was removed.
    Publication(E, ConcreteLifecycleWork),
}
/// Failure from an exact same-address replacement boundary.
#[derive(Debug)]
pub(super) enum RegistryReplacementError<E> {
    /// The incumbent or replacement failed exact validation before mutation.
    Validation(RegistryError, ConcreteLifecycleWork),
    /// The callback rejected the staged replacement and the incumbent was restored.
    Publication(E, ConcreteLifecycleWork),
}
/// Unwind-safe staging guard for one new registry installation.
struct StagedRegistryInstall<'a> {
    entries: &'a mut BTreeMap<ConcreteWorkAddress, ConcreteLifecycleWork>,
    address: ConcreteWorkAddress,
    armed: bool,
}
impl StagedRegistryInstall<'_> {
    fn commit(mut self) {
        self.armed = false;
    }
    fn rollback(mut self) -> ConcreteLifecycleWork {
        self.armed = false;
        self.entries
            .remove(&self.address)
            .expect("staged installation remains at its exact address")
    }
}
impl Drop for StagedRegistryInstall<'_> {
    fn drop(&mut self) {
        if self.armed {
            let removed = self
                .entries
                .remove(&self.address)
                .expect("unwinding installation remains at its exact address");
            drop(removed);
        }
    }
}
/// Unwind-safe staging guard for one exact registry replacement.
struct StagedRegistryReplacement<'a> {
    entries: &'a mut BTreeMap<ConcreteWorkAddress, ConcreteLifecycleWork>,
    address: ConcreteWorkAddress,
    incumbent: Option<ConcreteLifecycleWork>,
}
impl StagedRegistryReplacement<'_> {
    fn commit(mut self) -> ConcreteLifecycleWork {
        self.incumbent
            .take()
            .expect("staged replacement retains its incumbent until commit")
    }
    fn rollback(mut self) -> ConcreteLifecycleWork {
        let incumbent = self
            .incumbent
            .take()
            .expect("staged replacement retains its incumbent until rollback");
        self.entries
            .insert(self.address, incumbent)
            .expect("staged replacement remains installed at its exact address")
    }
}
impl Drop for StagedRegistryReplacement<'_> {
    fn drop(&mut self) {
        let Some(incumbent) = self.incumbent.take() else {
            return;
        };
        let replacement = self
            .entries
            .insert(self.address, incumbent)
            .expect("unwinding replacement remains installed at its exact address");
        drop(replacement);
    }
}
/// Closed failure inventory for concrete-work registration and resolution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RegistryError {
    /// The sealed pending authority does not name the supplied effect.
    UnboundEffect,
    /// Owner and ordinal do not form a valid admitted address.
    InvalidAddress,
    /// The admitted owner does not name the pending work's causal root.
    CausalOwnerMismatch,
    /// The coordinator's slot digest and concrete effect digest disagree.
    DigestMismatch,
    /// One concrete work value already occupies the exact logical address.
    Occupied,
    /// No concrete work value exists at the lease's exact address.
    Missing,
    /// A stored value lost its sealed effect binding.
    CorruptWork,
    /// The exact row is a closed carrier and cannot re-enter generic adapter execution.
    WrongWorkKind,
    /// The coordinator's admitted record did not name exactly one effect slot.
    InvalidAdmissionShape,
}
/// Deterministic process-local map from admitted slots to concrete effects.
///
/// This registry is deliberately not a scheduler. It owns no readiness,
/// ordinal allocation, rank, retry, wait, generation, capacity, or lease state.
#[derive(Debug)]
pub(in crate::sumeragi) struct ConcreteLifecycleWorkRegistry {
    identity: std::sync::Arc<ConcreteLifecycleWorkRegistryInstanceIdentityMarker>,
    entries: BTreeMap<ConcreteWorkAddress, ConcreteLifecycleWork>,
}
/// Exclusive optional WAL-owned registry slot at startup.
#[derive(Clone, Copy)]
enum RecoveredWalRegistrySlotV1 {
    None,
    PhaseVote(ConcreteWorkAddress),
    ControlSign(ConcreteWorkAddress),
    NextVote(ConcreteWorkAddress),
    SignedBroadcast(ConcreteWorkAddress),
    SignedBroadcastAndNextVote {
        broadcast: ConcreteWorkAddress,
        next_sign: ConcreteWorkAddress,
    },
    DecisionFetch(ConcreteWorkAddress),
    DecisionStore(ConcreteWorkAddress),
    DecisionApply(ConcreteWorkAddress),
}
impl RecoveredWalRegistrySlotV1 {
    const fn addresses(self) -> [Option<ConcreteWorkAddress>; 2] {
        match self {
            Self::None => [None, None],
            Self::PhaseVote(address)
            | Self::ControlSign(address)
            | Self::NextVote(address)
            | Self::SignedBroadcast(address)
            | Self::DecisionFetch(address)
            | Self::DecisionStore(address)
            | Self::DecisionApply(address) => [Some(address), None],
            Self::SignedBroadcastAndNextVote {
                broadcast,
                next_sign,
            } => [Some(broadcast), Some(next_sign)],
        }
    }
    const fn cardinality(self) -> usize {
        match self {
            Self::None => 0,
            Self::SignedBroadcastAndNextVote { .. } => 2,
            Self::PhaseVote(_)
            | Self::ControlSign(_)
            | Self::NextVote(_)
            | Self::SignedBroadcast(_)
            | Self::DecisionFetch(_)
            | Self::DecisionStore(_)
            | Self::DecisionApply(_) => 1,
        }
    }
    fn contains_record(self, record: &super::LifecycleRecord) -> bool {
        self.addresses()
            .into_iter()
            .flatten()
            .any(|address| record.owner == address.owner && record.ordinal == address.ordinal)
    }
    fn contains_owner(self, owner: OwnerId) -> bool {
        self.addresses()
            .into_iter()
            .flatten()
            .any(|address| address.owner == owner)
    }
}
#[derive(Debug)]
struct ConcreteLifecycleWorkRegistryInstanceIdentityMarker;
/// Comparison-only identity for one exact concrete registry instance.
#[derive(Clone, Debug)]
pub(super) struct ConcreteLifecycleWorkRegistryInstanceIdentity(
    std::sync::Arc<ConcreteLifecycleWorkRegistryInstanceIdentityMarker>,
);
impl ConcreteLifecycleWorkRegistryInstanceIdentity {
    /// Return whether both seals came from the same registry owner.
    pub(super) fn same_instance(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Default for ConcreteLifecycleWorkRegistry {
    fn default() -> Self {
        Self {
            identity: std::sync::Arc::new(ConcreteLifecycleWorkRegistryInstanceIdentityMarker),
            entries: BTreeMap::new(),
        }
    }
}
include!("v2_lifecycle_work_registry_validate_recovery_registry_impl.rs");
fn sealed_successor_parent<'a>(
    registry: &'a ConcreteLifecycleWorkRegistry,
    address: ConcreteWorkAddress,
    lease: &TurnLease,
) -> Result<&'a ConcreteLifecycleWork, SealedBodySuccessorProjectionError> {
    let Some((&slot, &digest)) = lease.physical_slots().first_key_value() else {
        return Err(SealedBodySuccessorProjectionError::ForeignParent);
    };
    if lease.physical_slots().len() != 1
        || slot.capacity_class() != Some(CapacityClass::Effect)
        || ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot) != Some(address)
    {
        return Err(SealedBodySuccessorProjectionError::ForeignParent);
    }
    let work = registry
        .entries
        .get(&address)
        .ok_or(SealedBodySuccessorProjectionError::ForeignParent)?;
    if !work.validates_at(address) || work.digest != digest {
        return Err(SealedBodySuccessorProjectionError::ForeignParent);
    }
    Ok(work)
}
fn sealed_successor_candidate_has_exact_geometry(
    candidate: &CandidateAdmission,
    expected_class: LifecycleWorkClass,
    expected_digest: LifecycleDigest,
) -> bool {
    let expected_slot = PhysicalSlotId::for_capacity(expected_class.capacity_class(), 0);
    candidate
        .physical_geometry
        .normalized()
        .is_ok_and(|(slots, universe, consumed)| {
            slots.len() == 1
                && slots.get(&expected_slot) == Some(&expected_digest)
                && universe.len() == 1
                && universe.contains(&expected_slot)
                && consumed == universe
        })
}
#[allow(dead_code)]
impl PreparedCertifiedFetchStoreSuccessor<'_> {
    /// Project the exact Store candidate while retaining its Fetch registry cut.
    ///
    /// The lease supplies only coordinator ownership coordinates. Effect,
    /// pending binding, durable frame, replay authority, and child digest stay
    /// sealed in this token and are revalidated before projection.
    pub(super) fn project_for_body_transition(
        &self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
    ) -> Result<CandidateAdmission, SealedBodySuccessorProjectionError> {
        let work = sealed_successor_parent(self._registry, self._completion_address, lease)?;
        let ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) = &work.kind else {
            return Err(SealedBodySuccessorProjectionError::InvalidCarrier);
        };
        let ready_projection = completion.replay_evidence.project_durable_ready_fetch(
            &completion.incumbent_effect,
            &completion.incumbent_pending,
            &completion.durable_receipt,
        );
        if !completion.validates(work.digest)
            || completion.address != self._completion_address
            || completion.durable_receipt != self._durable_body
            || ready_projection
                .as_ref()
                .map(DurableCertifiedFetchReplayProjectionV1::expected_manifest_hash)
                != Some(self._expected_manifest_hash)
            || self._durable_body.manifest_hash() != self._expected_manifest_hash
            || !self
                ._store_pending
                .exactly_binds_adapter_effect(&self._store_effect)
            || super::CausalRoot::new(digest_from_hash(self._store_pending.causal_lifecycle_key()))
                != self._completion_address.owner.causal_root()
            || digest_from_hash(self._store_pending.exact_effect_identity()) != self._store_digest
            || !self
                ._replay_evidence
                .exactly_matches_store(&self._store_effect, &self._durable_body)
        {
            return Err(SealedBodySuccessorProjectionError::InvalidCarrier);
        }
        let candidate = self
            ._replay_evidence
            .project_sealed_store_successor_candidate(
                SealedBodySuccessorProjectionPermit::new(),
                verified,
                &self._store_effect,
                &self._durable_body,
                &self._store_pending,
            )
            .map_err(SealedBodySuccessorProjectionError::Projection)?;
        if candidate.causal_root != self._completion_address.owner.causal_root()
            || candidate.payload
                != durable_validate_body_payload(&self._durable_body)
                    .ok_or(SealedBodySuccessorProjectionError::InvalidCarrier)?
            || !sealed_successor_candidate_has_exact_geometry(
                &candidate,
                LifecycleWorkClass::Store,
                self._store_digest,
            )
        {
            return Err(SealedBodySuccessorProjectionError::InvalidCarrier);
        }
        Ok(candidate)
    }
}
#[allow(dead_code)]
impl PreparedDurableStoreValidateSuccessor<'_> {
    /// Project the exact Validate candidate while retaining its Store registry cut.
    ///
    /// The candidate is derived only from the Store-projected pending binding,
    /// independently transferred manifest hash, exact durable frame, and
    /// certified replay evidence already owned by this move-only token.
    pub(super) fn project_for_body_transition(
        &self,
        lease: &TurnLease,
        verified: &VerifiedHeightContext,
    ) -> Result<CandidateAdmission, SealedBodySuccessorProjectionError> {
        let work = sealed_successor_parent(self._registry, self._store_address, lease)?;
        let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &work.kind else {
            return Err(SealedBodySuccessorProjectionError::InvalidCarrier);
        };
        if !store.validates(work.digest)
            || store.address != self._store_address
            || store.durable_receipt != self._durable_body
            || store.expected_manifest_hash != self._expected_manifest_hash
            || self._durable_body.manifest_hash() != self._expected_manifest_hash
            || !self
                ._validate_pending
                .exactly_binds_adapter_effect(&self._validate_effect)
            || super::CausalRoot::new(digest_from_hash(
                self._validate_pending.causal_lifecycle_key(),
            )) != self._store_address.owner.causal_root()
            || digest_from_hash(self._validate_pending.exact_effect_identity())
                != self._validate_digest
        {
            return Err(SealedBodySuccessorProjectionError::InvalidCarrier);
        }
        let replay_evidence =
            DurableValidateReplayEvidenceV1::certified(self._replay_evidence.clone());
        let candidate = replay_evidence
            .project_sealed_validate_successor_candidate(
                SealedBodySuccessorProjectionPermit::new(),
                verified,
                &self._validate_effect,
                &self._durable_body,
                &self._validate_pending,
            )
            .map_err(SealedBodySuccessorProjectionError::Projection)?;
        if candidate.causal_root != self._store_address.owner.causal_root()
            || candidate.payload
                != durable_validate_body_payload(&self._durable_body)
                    .ok_or(SealedBodySuccessorProjectionError::InvalidCarrier)?
            || !sealed_successor_candidate_has_exact_geometry(
                &candidate,
                LifecycleWorkClass::Validate,
                self._validate_digest,
            )
        {
            return Err(SealedBodySuccessorProjectionError::InvalidCarrier);
        }
        Ok(candidate)
    }
}
// READY_DURABLE_VALIDATE_ADAPTER_JOIN_BEGIN
#[allow(dead_code)]
impl<'registry> PreparedReadyDurableValidateExecution<'registry> {
    fn completion(&self) -> Option<&DurableValidateCompletion> {
        let work = self.registry.entries.get(&self.address)?;
        let ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) = &work.kind else {
            return None;
        };
        completion.validates(work.digest).then_some(completion)
    }
    /// Return only the closed reducer-level outcome discriminator.
    pub(crate) const fn outcome_kind(&self) -> ReadyDurableValidateOutcomeKind {
        self.outcome_kind
    }
    fn matches_exact_lease(&self, lease: &TurnLease) -> bool {
        &self.lease == lease
    }
    fn matches_exact_durable_receipt(&self, receipt: &DurableBodyReceipt) -> bool {
        self.completion()
            .is_some_and(|completion| &completion.incumbent.durable_receipt == receipt)
    }
    fn validated_completion(&self) -> Option<&DurableValidateCompletion> {
        if self.outcome_kind != ReadyDurableValidateOutcomeKind::Validated {
            return None;
        }
        let completion = self.completion()?;
        let AdapterEffect::ValidateBody { .. } = &completion.incumbent.effect else {
            return None;
        };
        let receipt = completion.outcome.validated_receipt()?;
        if completion.outcome.rejection_identity().is_some()
            || completion.outcome.missing_merge_sidecar().is_some()
            || receipt.durable() != &completion.incumbent.durable_receipt
            || receipt.durable().manifest_hash() != completion.incumbent.expected_manifest_hash
            || completion.incumbent.durable_receipt.manifest_hash()
                != completion.incumbent.expected_manifest_hash
            || validate_validated_receipt_authority(&completion.incumbent, receipt).is_err()
        {
            return None;
        }
        Some(completion)
    }
    fn validated_authority(&self) -> Option<ReadyValidatedAdapterAuthority<'_>> {
        let completion = self.validated_completion()?;
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = &completion.incumbent.effect
        else {
            return None;
        };
        let receipt = completion.outcome.validated_receipt()?;
        Some(ReadyValidatedAdapterAuthority {
            tag: *tag,
            round: *round,
            subject: *subject,
            receipt,
        })
    }
    fn validate_sign_predecessor_authority(
        &self,
    ) -> Option<ReadyValidateSignPredecessorAuthority<'_>> {
        let completion = self.validated_completion()?;
        Some(ReadyValidateSignPredecessorAuthority {
            effect: &completion.incumbent.effect,
            pending: &completion.incumbent.pending,
            _linearity: ReadyValidateSignPredecessorLinearity,
        })
    }
    fn rejected_authority(&self) -> Option<ReadyRejectedAdapterAuthority<'_>> {
        if self.outcome_kind != ReadyDurableValidateOutcomeKind::Rejected {
            return None;
        }
        let completion = self.completion()?;
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = &completion.incumbent.effect
        else {
            return None;
        };
        if completion.outcome.validated_receipt().is_some()
            || completion.outcome.rejection_identity()
                != Some(&BodyValidationRejectionIdentity::Rejected)
            || completion.outcome.missing_merge_sidecar().is_some()
            || completion.outcome.durable_body() != &completion.incumbent.durable_receipt
            || completion.outcome.durable_body().manifest_hash()
                != completion.incumbent.expected_manifest_hash
            || completion.incumbent.durable_receipt.manifest_hash()
                != completion.incumbent.expected_manifest_hash
        {
            return None;
        }
        Some(ReadyRejectedAdapterAuthority {
            tag: *tag,
            round: *round,
            subject: *subject,
            receipt: completion.outcome.durable_body(),
        })
    }
    /// Consume this exact registry cut into the adapter's sealed direct preview.
    ///
    /// The fixed join exposes no generic callback or raw receipt result. Every
    /// successful and failed classification retains the complete registry
    /// authority, so the operation is single-use and drop-inert.
    #[allow(clippy::result_large_err)]
    pub(super) fn prepare_adapter_preview<'adapter>(
        self,
        adapter: &'adapter mut SumeragiV2Adapter,
    ) -> Result<
        PreparedReadyDurableValidateAdapterPreview<'registry, 'adapter>,
        ReadyDurableValidateAdapterPreviewError<'registry>,
    > {
        let adapter_preview = match self.outcome_kind {
            ReadyDurableValidateOutcomeKind::Validated => {
                let Some(authority) = self.validated_authority() else {
                    return Err(ReadyDurableValidateAdapterPreviewError {
                        _registry: self,
                        _failure: ReadyDurableValidateAdapterPreviewFailure::RegistryAuthority,
                    });
                };
                adapter.prepare_sealed_ready_durable_validate_succeeded(authority)
            }
            ReadyDurableValidateOutcomeKind::Rejected => {
                let Some(authority) = self.rejected_authority() else {
                    return Err(ReadyDurableValidateAdapterPreviewError {
                        _registry: self,
                        _failure: ReadyDurableValidateAdapterPreviewFailure::RegistryAuthority,
                    });
                };
                adapter.prepare_sealed_ready_durable_validate_failed(authority)
            }
        };
        match adapter_preview {
            Ok(adapter_preview) => match adapter_preview.preflight_publication() {
                Ok(_adapter) => Ok(PreparedReadyDurableValidateAdapterPreview {
                    _registry: self,
                    _adapter,
                }),
                Err(error) => Err(ReadyDurableValidateAdapterPreviewError {
                    _registry: self,
                    _failure: ReadyDurableValidateAdapterPreviewFailure::Adapter(error),
                }),
            },
            Err(error) => Err(ReadyDurableValidateAdapterPreviewError {
                _registry: self,
                _failure: ReadyDurableValidateAdapterPreviewFailure::Adapter(error),
            }),
        }
    }
}
// READY_DURABLE_VALIDATE_ADAPTER_JOIN_END
// RECOVERED_WAL_VALIDATE_REGISTRY_DETACH_BEGIN
impl<'registry> PreparedReadyDurableValidateExecution<'registry> {
    /// Detach the exact Ready/validated carrier for the fixed recovered-WAL join.
    ///
    /// Rejected completions and malformed carriers are returned unchanged. A
    /// successfully detached cut restores the byte-for-byte carrier on drop
    /// until its recovered vote consumes the predecessor binding.
    #[allow(clippy::result_large_err)]
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn into_recovered_wal_validate_registry_cut(
        self,
    ) -> Result<RecoveredWalValidateRegistryCut<'registry>, Self> {
        if self.outcome_kind != ReadyDurableValidateOutcomeKind::Validated
            || self.completion().is_none()
        {
            return Err(self);
        }
        let address = self.address;
        let Some(work) = self.registry.entries.remove(&address) else {
            return Err(self);
        };
        let Self {
            registry,
            address: _,
            outcome_kind: _,
            lease: _,
        } = self;
        Ok(RecoveredWalValidateRegistryCut {
            registry: Some(registry),
            address,
            work: Some(work),
        })
    }
}
// RECOVERED_WAL_VALIDATE_REGISTRY_DETACH_END
include!("v2_lifecycle_work_registry_validate_recovery_parent.rs");
// RECOVERED_WAL_VALIDATE_REGISTRY_JOIN_BEGIN
#[must_use = "a prepared recovered WAL Validate join still owns its detached authority"]
struct PreparedRecoveredWalValidateRegistryJoin<'registry> {
    cut: RecoveredWalValidateRegistryCut<'registry>,
    completion: DetachedRecoveredValidateCompletion,
    successor: Option<crate::sumeragi::v2_runtime::RecoveredWalVoteSuccessor>,
}
impl<'registry> RecoveredWalValidateRegistryCut<'registry> {
    /// Convert the existing restorable recovery cut into the sole fail-stop
    /// live parent/child reservation after WAL fsync.
    ///
    /// Taking both optional fields disarms the recovery cut's restoring Drop.
    /// The returned parent is retained opaquely and is never reinstalled: any
    /// later error must restart through the durable WAL.
    fn into_live_validate_sign_reservation(
        mut self,
    ) -> Option<LiveValidateSignRegistryReservation<'registry>> {
        let registry = self.registry.take()?;
        let parent = self.work.take()?;
        let parent_address = self.address;
        Some(LiveValidateSignRegistryReservation {
            reservation: RecoveredWalValidateRegistryReservation {
                registry,
                parent_address,
                child: None,
            },
            _detached_parent: parent,
        })
    }
    #[cfg(test)]
    fn detached_work_is_exact_for_test(&self) -> bool {
        self.work.as_ref().is_some_and(|work| {
            work.validates_at(self.address)
                && matches!(
                    &work.kind,
                    ConcreteLifecycleWorkKind::DurableValidateCompletion(completion)
                        if completion.address == self.address
                            && completion.outcome.validated_receipt().is_some()
                            && completion.outcome.rejection_identity().is_none()
                            && completion.outcome.missing_merge_sidecar().is_none()
                )
        })
    }
    /// Consume this exact closed Validate carrier and the adapter-authenticated
    /// current reducer vote from its latest exact WAL owner into one typed
    /// lifecycle repair.
    ///
    /// The pending binding never leaves this module. Projection failure
    /// reconstructs the detached completion so dropping the returned error
    /// restores it at the exact address. A later lifecycle-authentication
    /// failure retains every move-only input and requires restart.
    #[allow(clippy::result_large_err)]
    pub(crate) fn join_recovered_vote(
        self,
        verified: &VerifiedHeightContext,
        recovered: RecoveredWalVoteSign,
    ) -> Result<
        AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
        RecoveredWalValidateRegistryJoinError<'registry>,
    > {
        let prepared = self.prepare_recovered_vote_join(recovered)?;
        prepared.authenticate(verified)
    }
    #[allow(clippy::result_large_err)]
    #[inline(never)]
    fn prepare_recovered_vote_join(
        mut self,
        recovered: RecoveredWalVoteSign,
    ) -> Result<
        Box<PreparedRecoveredWalValidateRegistryJoin<'registry>>,
        RecoveredWalValidateRegistryJoinError<'registry>,
    > {
        let recovered_commitment = recovered.vote().execution_commitment;
        let valid = self.work.as_ref().is_some_and(|work| {
            work.validates_at(self.address)
                && matches!(
                    &work.kind,
                    ConcreteLifecycleWorkKind::DurableValidateCompletion(completion)
                        if completion.address == self.address
                            && completion.outcome.validated_receipt().is_some_and(|receipt| {
                                receipt.execution_commitment() == recovered_commitment
                            })
                            && completion.outcome.rejection_identity().is_none()
                            && completion.outcome.missing_merge_sidecar().is_none()
                )
        });
        if !valid {
            return Err(RecoveredWalValidateRegistryJoinError {
                failure: Box::new(RecoveredWalValidateRegistryJoinFailure::InvalidCarrier {
                    _cut: self,
                    _recovered: recovered,
                }),
            });
        }
        let work = self
            .work
            .take()
            .expect("validated recovered WAL cut retains its detached carrier");
        let ConcreteLifecycleWork {
            digest: installed_digest,
            kind: ConcreteLifecycleWorkKind::DurableValidateCompletion(completion),
        } = work
        else {
            unreachable!("recovered WAL cut validated one completion carrier")
        };
        let DurableValidateCompletion {
            address,
            incumbent,
            incumbent_digest,
            outcome,
        } = completion;
        let DurableValidateBody {
            address: incumbent_address,
            effect,
            pending,
            durable_receipt,
            expected_manifest_hash,
            replay_evidence,
        } = incumbent;
        let completion = DetachedRecoveredValidateCompletion {
            address,
            installed_digest,
            incumbent_address,
            incumbent_digest,
            durable_receipt,
            expected_manifest_hash,
            replay_evidence: DetachedValidateReplayEvidenceV1::Retained(replay_evidence),
            outcome,
        };
        let successor = match pending.project_recovered_wal_vote_successor(&effect, recovered) {
            Ok(successor) => successor,
            Err((pending, recovered)) => {
                self.work = Some(completion.restore(effect, pending));
                return Err(RecoveredWalValidateRegistryJoinError {
                    failure: Box::new(RecoveredWalValidateRegistryJoinFailure::Projection {
                        _cut: self,
                        _recovered: recovered,
                    }),
                });
            }
        };
        Ok(Box::new(PreparedRecoveredWalValidateRegistryJoin {
            cut: self,
            completion,
            successor: Some(successor),
        }))
    }
}
impl<'registry> PreparedRecoveredWalValidateRegistryJoin<'registry> {
    #[allow(clippy::result_large_err)]
    #[inline(never)]
    fn authenticate(
        mut self: Box<Self>,
        verified: &VerifiedHeightContext,
    ) -> Result<
        AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
        RecoveredWalValidateRegistryJoinError<'registry>,
    > {
        let successor = self
            .successor
            .take()
            .expect("prepared recovered WAL join retains one successor");
        let DetachedValidateReplayEvidenceV1::Retained(replay_evidence) =
            &self.completion.replay_evidence
        else {
            unreachable!("a live detached Validate completion retains its replay origin")
        };
        let authenticated = authenticate_recovered_wal_vote_lifecycle_from_durable_body(
            verified,
            &self.completion.durable_receipt,
            replay_evidence,
            successor,
        );
        let Self {
            mut cut,
            completion,
            successor,
        } = *self;
        debug_assert!(successor.is_none());
        match authenticated {
            Ok(repair) => {
                let registry = cut.registry.take();
                let registry =
                    registry.expect("recovered WAL join retains its exclusive registry borrow");
                Ok(AuthenticatedRecoveredWalValidateLifecycleRepair {
                    repair,
                    validation: completion,
                    reservation: RecoveredWalValidateRegistryReservation {
                        registry,
                        parent_address: cut.address,
                        child: None,
                    },
                })
            }
            Err(error) => Err(RecoveredWalValidateRegistryJoinError {
                failure: Box::new(RecoveredWalValidateRegistryJoinFailure::Lifecycle {
                    _cut: cut,
                    _error: error,
                    _completion: completion,
                }),
            }),
        }
    }
}
// RECOVERED_WAL_VALIDATE_REGISTRY_JOIN_END
#[allow(dead_code)]
impl<'a> PreparedDurableValidateExecution<'a> {
    fn durable_validate(&self) -> &DurableValidateBody {
        let work = self
            .registry
            .entries
            .get(&self.address)
            .expect("prepared durable Validate carrier remains installed");
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
            unreachable!("prepared durable Validate execution retains its closed carrier")
        };
        validate
    }
    /// Return the exact reducer coordinates accepted by the future body
    /// validation preview.
    pub(super) fn adapter_preview_inputs(
        &self,
    ) -> (EventTag, wire::ConsensusRound, wire::BlockSubject) {
        let AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } = &self.durable_validate().effect
        else {
            unreachable!("prepared durable Validate carrier retains its Validate effect")
        };
        (*tag, *round, *subject)
    }
    /// Borrow the exact post-fsync body receipt retained by the Validate carrier.
    pub(super) fn durable_body_receipt(&self) -> &DurableBodyReceipt {
        &self.durable_validate().durable_receipt
    }
    /// Match the coordinator's durable payload against this exact body receipt.
    pub(super) fn matches_durable_payload(&self, payload: DurablePayloadReference) -> bool {
        durable_validate_body_payload(&self.durable_validate().durable_receipt).is_some_and(
            |expected| {
                expected == payload
                    && super::body_pipeline_transition::durable_validate_payload_is_exact(
                        self.lifecycle_key,
                        payload,
                    )
            },
        )
    }
    /// Return the manifest hash transferred independently through the Store parent.
    pub(super) fn expected_manifest_hash(&self) -> HashOf<wire::PayloadManifest> {
        self.durable_validate().expected_manifest_hash
    }
    /// Derive the external wake source only from this revalidated closed row.
    ///
    /// Callers cannot supply address, digest, causal-key, or inherited
    /// statement parts independently. The coordinator samples the generation
    /// for this source before consuming the preflight into a dispatch.
    pub(super) fn durable_validation_wait_source(&self) -> WaitSource {
        let work = self
            .registry
            .entries
            .get(&self.address)
            .expect("prepared durable Validate carrier remains installed");
        let validate = self.durable_validate();
        durable_validation_wait_source_from_exact_parts(
            self.address,
            work.digest,
            validate.pending.causal_lifecycle_key(),
            validate.pending.candidate_statement(),
            &validate.durable_receipt,
            validate.expected_manifest_hash,
            self.lifecycle_key,
            self.lifecycle_stage,
        )
    }
    /// Seal this preflight beside the exact coordinator-minted external wait.
    ///
    /// A foreign source or the reserved maximum generation returns the
    /// borrow-bound preflight intact and mints no detached request.
    pub(super) fn seal_waiting_dispatch(
        self,
        wait_token: WaitToken,
    ) -> Result<DurableValidateDispatch, Self> {
        if wait_token.source() != self.durable_validation_wait_source()
            || wait_token.observed_generation() == u64::MAX
        {
            return Err(self);
        }
        Ok(DurableValidateDispatch {
            request: self.detach(),
            wake: DurableValidateWakeAuthority { wait_token },
        })
    }
    /// Consume the borrow-bound preflight into an owned validation request.
    ///
    /// The registry row is not removed or changed. Returning the owned token
    /// ends the exclusive registry borrow before any body-store I/O or
    /// deterministic validation callback can run.
    pub(super) fn detach(self) -> DetachedDurableValidateExecution {
        let (
            incumbent_digest,
            tag,
            round,
            subject,
            durable_receipt,
            expected_manifest_hash,
            causal_lifecycle_key,
            candidate_statement,
            lifecycle_key,
            lifecycle_stage,
        ) = {
            let work = self
                .registry
                .entries
                .get(&self.address)
                .expect("prepared durable Validate carrier remains installed");
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
                unreachable!("prepared durable Validate execution retains its closed carrier")
            };
            let AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            } = &validate.effect
            else {
                unreachable!("prepared durable Validate carrier retains its Validate effect")
            };
            (
                work.digest,
                *tag,
                *round,
                *subject,
                validate.durable_receipt.clone(),
                validate.expected_manifest_hash,
                validate.pending.causal_lifecycle_key().clone(),
                validate.pending.candidate_statement(),
                self.lifecycle_key,
                self.lifecycle_stage,
            )
        };
        DetachedDurableValidateExecution {
            address: self.address,
            incumbent_digest,
            tag,
            round,
            subject,
            durable_receipt,
            expected_manifest_hash,
            causal_lifecycle_key,
            candidate_statement,
            lifecycle_key,
            lifecycle_stage,
        }
    }
    /// Bind one store-minted successful-validation receipt to this exact
    /// Validate carrier without changing the registry row.
    ///
    /// Existing Prepare or Commit authority must name the same deterministic
    /// execution result. An ordinary body may acquire its first commitment,
    /// but only the later receipt-bound Apply projection may use it.
    pub(super) fn bind_validated_receipt(
        self,
        validated_receipt: ValidatedBodyReceipt,
    ) -> Result<
        PreparedValidatedBodyCompletion<'a>,
        (DurableValidateExecutionError, ValidatedBodyReceipt),
    > {
        let (tag, round, subject, incumbent_digest, replacement_digest) = {
            let validate = self.durable_validate();
            let AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            } = &validate.effect
            else {
                return Err((
                    DurableValidateExecutionError::InvalidValidateShape,
                    validated_receipt,
                ));
            };
            if let Err(error) = validate_validated_receipt_authority(validate, &validated_receipt) {
                return Err((error, validated_receipt));
            }
            let incumbent_digest = self
                .registry
                .entries
                .get(&self.address)
                .expect("prepared durable Validate carrier remains installed")
                .digest;
            let replacement_digest = validated_body_completion_digest(
                incumbent_digest,
                validate.expected_manifest_hash,
                &validated_receipt,
            );
            if replacement_digest == incumbent_digest {
                return Err((
                    DurableValidateExecutionError::InvalidValidationCompletionDigest,
                    validated_receipt,
                ));
            }
            (*tag, *round, *subject, incumbent_digest, replacement_digest)
        };
        Ok(PreparedValidatedBodyCompletion {
            _registry: self.registry,
            address: self.address,
            incumbent_digest,
            replacement_digest,
            validated_receipt,
            tag,
            round,
            subject,
        })
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'a> PreparedValidatedBodyCompletion<'a> {
    fn retained_validated_receipt_is_exact(&self) -> bool {
        let Some(work) = self._registry.entries.get(&self.address) else {
            return false;
        };
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
            return false;
        };
        work.digest == self.incumbent_digest
            && validate.validates(self.incumbent_digest)
            && validate_validated_receipt_authority(validate, &self.validated_receipt).is_ok()
            && validated_body_completion_digest(
                self.incumbent_digest,
                validate.expected_manifest_hash,
                &self.validated_receipt,
            ) == self.replacement_digest
    }
    fn retained_apply_join_is_exact(&self, persisted: &SealedLiveWalPersistedEffectV1) -> bool {
        let Some(work) = self._registry.entries.get(&self.address) else {
            return false;
        };
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
            return false;
        };
        self.retained_validated_receipt_is_exact()
            && persisted.exactly_binds_validated_apply_successor(
                &validate.effect,
                &validate.pending,
                self.validated_receipt.durable(),
            )
    }
    /// Join one exact live `Decision -> Apply` WAL seal to this retained Validate result.
    ///
    /// This is the sole production receipt-bearing completion surface. It
    /// first revalidates the installed carrier and store-minted receipt, then
    /// consumes the source-only WAL seal into its canonical body-frame-bound
    /// replay envelope. Failure retains every move-only input.
    #[allow(clippy::result_large_err)]
    pub(super) fn seal_live_wal_apply(
        self,
        persisted: SealedLiveWalPersistedEffectV1,
        pending: PendingRuntimeEffectBinding,
    ) -> Result<PreparedLiveWalReplayPreAdmission<'a>, LiveWalReplayPreAdmissionError<'a>> {
        if !self.retained_validated_receipt_is_exact() {
            return Err(LiveWalReplayPreAdmissionError {
                _failure: LiveWalReplayPreAdmissionFailure::Apply {
                    _completion: self,
                    _persisted: persisted,
                    _pending: pending,
                },
            });
        }
        let (predecessor_effect, predecessor_pending) = {
            let work = self
                ._registry
                .entries
                .get(&self.address)
                .expect("revalidated Apply join retains its installed Validate row");
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
                unreachable!("revalidated Apply join retains its durable Validate carrier")
            };
            (&validate.effect, &validate.pending)
        };
        let persisted = match persisted.complete_exact_apply(
            predecessor_effect,
            predecessor_pending,
            pending,
            self.validated_receipt.durable(),
        ) {
            Ok(persisted) => persisted,
            Err((persisted, pending)) => {
                return Err(LiveWalReplayPreAdmissionError {
                    _failure: LiveWalReplayPreAdmissionFailure::Apply {
                        _completion: self,
                        _persisted: persisted,
                        _pending: pending,
                    },
                });
            }
        };
        debug_assert!(self.retained_apply_join_is_exact(&persisted));
        Ok(PreparedLiveWalReplayPreAdmission {
            _persisted: persisted,
            _origin: LiveWalReplayPreAdmissionOrigin::Apply(self),
        })
    }
    /// Exact reducer coordinates retained by the completed Validate carrier.
    pub(super) const fn adapter_preview_inputs(
        &self,
    ) -> (EventTag, wire::ConsensusRound, wire::BlockSubject) {
        (self.tag, self.round, self.subject)
    }
    /// Borrow the exact store-minted deterministic validation result.
    pub(super) const fn validated_receipt(&self) -> &ValidatedBodyReceipt {
        &self.validated_receipt
    }
    /// Digest currently installed for the closed Validate work.
    pub(super) const fn incumbent_digest(&self) -> LifecycleDigest {
        self.incumbent_digest
    }
    /// Domain-separated physical digest for the receipt-bound completion.
    pub(super) const fn replacement_digest(&self) -> LifecycleDigest {
        self.replacement_digest
    }
}
