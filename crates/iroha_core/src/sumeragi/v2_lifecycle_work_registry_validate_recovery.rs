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

impl<'a> ReadyValidateSignPredecessorAuthority<'a> {
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

    /// Construct the same opaque view for focused adapter tests.
    #[cfg(test)]
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

    /// Compare one retained lease without exposing registry or report parts.
    #[cfg(test)]
    fn exactly_matches_lease_for_test(&self, lease: &TurnLease) -> bool {
        self.validates() && self.registry.matches_exact_lease(lease)
    }

    /// Compare one retained body receipt without exposing the canonical frame.
    #[cfg(test)]
    fn exactly_matches_receipt_for_test(&self, receipt: &DurableBodyReceipt) -> bool {
        self.validates() && self.registry.matches_exact_durable_receipt(receipt)
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

/// Closed live-WAL replay preflight retained until future exact admission.
///
/// Payload-free stages own the canonical source seal immediately. `Apply`
/// additionally owns the exact receipt-bound Validate completion that supplied
/// its body frame. No field, effect, pending binding, receipt, or replay parts
/// can be extracted, and dropping the token publishes no lifecycle work.
#[must_use = "live WAL replay evidence has not entered lifecycle admission"]
#[cfg_attr(not(test), allow(dead_code))]
pub(super) struct PreparedLiveWalReplayPreAdmission<'a> {
    _persisted: SealedLiveWalPersistedEffectV1,
    _origin: LiveWalReplayPreAdmissionOrigin<'a>,
}

#[allow(dead_code, variant_size_differences, clippy::large_enum_variant)]
enum LiveWalReplayPreAdmissionOrigin<'a> {
    PayloadFree,
    Apply(PreparedValidatedBodyCompletion<'a>),
}

#[allow(variant_size_differences, clippy::large_enum_variant)]
enum LiveWalReplayPreAdmissionFailure<'a> {
    PayloadFree {
        _persisted: SealedLiveWalPersistedEffectV1,
    },
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

#[cfg_attr(not(test), allow(dead_code))]
impl PreparedLiveWalReplayPreAdmission<'static> {
    /// Seal one of the five payload-free WAL continuations with its exact pending owner.
    #[allow(clippy::result_large_err)]
    pub(super) fn seal_payload_free(
        persisted: SealedLiveWalPersistedEffectV1,
    ) -> Result<Self, LiveWalReplayPreAdmissionError<'static>> {
        if !persisted.exactly_binds_payload_free_pending() {
            return Err(LiveWalReplayPreAdmissionError {
                _failure: LiveWalReplayPreAdmissionFailure::PayloadFree {
                    _persisted: persisted,
                },
            });
        }
        Ok(Self {
            _persisted: persisted,
            _origin: LiveWalReplayPreAdmissionOrigin::PayloadFree,
        })
    }
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
pub(super) struct ConcreteLifecycleWorkRegistry {
    identity: std::sync::Arc<ConcreteLifecycleWorkRegistryInstanceIdentityMarker>,
    entries: BTreeMap<ConcreteWorkAddress, ConcreteLifecycleWork>,
}

/// Exclusive optional WAL-owned registry slot at startup.
#[derive(Clone, Copy)]
enum RecoveredWalRegistrySlotV1 {
    None,
    PhaseVote(ConcreteWorkAddress),
    ControlSign(ConcreteWorkAddress),
    DecisionFetch(ConcreteWorkAddress),
    DecisionApply(ConcreteWorkAddress),
}

impl RecoveredWalRegistrySlotV1 {
    const fn address(self) -> Option<ConcreteWorkAddress> {
        match self {
            Self::None => None,
            Self::PhaseVote(address)
            | Self::ControlSign(address)
            | Self::DecisionFetch(address)
            | Self::DecisionApply(address) => Some(address),
        }
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

impl ConcreteLifecycleWorkRegistry {
    /// Project a comparison-only seal for this exact registry instance.
    pub(super) fn instance_identity(&self) -> ConcreteLifecycleWorkRegistryInstanceIdentity {
        ConcreteLifecycleWorkRegistryInstanceIdentity(std::sync::Arc::clone(&self.identity))
    }

    /// Whether this registry has no installed concrete authority.
    pub(super) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Consume one exact durable control projection into its dedicated carrier.
    ///
    /// Every projection, opened-frame, unique-row, standalone-owner, address,
    /// geometry, digest, and vacancy check precedes the sole insertion. The
    /// existing durable row is never rewritten here; a coalesced restart only
    /// reconstructs this volatile carrier.
    #[allow(clippy::result_large_err)]
    pub(super) fn install_recovered_wal_control_sign<'registry>(
        &'registry mut self,
        verified: &VerifiedHeightContext,
        store: &super::ledger::LifecycleLedgerStoreV1,
        ledger: &super::ledger::LifecycleLedgerV1,
        projection: AuthenticatedRecoveredWalControlProjection,
    ) -> Result<
        InstalledRecoveredWalControlSignRegistryCut<'registry>,
        RecoveredWalControlSignInstallError,
    > {
        if !self.entries.is_empty()
            || !projection.is_exact(verified)
            || !store.load().is_ok_and(|opened| opened == *ledger)
        {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::Projection {
                    _projection: projection,
                },
            });
        }
        let records = ledger
            .records()
            .iter()
            .filter(|record| projection.names_record(record))
            .collect::<Vec<_>>();
        let [record] = records.as_slice() else {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::Projection {
                    _projection: projection,
                },
            });
        };
        if !projection.exactly_matches_record(record) {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::Projection {
                    _projection: projection,
                },
            });
        }
        let slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let Some(address) = ConcreteWorkAddress::new(record.owner(), record.ordinal(), slot) else {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::Projection {
                    _projection: projection,
                },
            });
        };
        let carrier =
            match projection.into_durable_carrier(address.owner, address.ordinal, address.slot) {
                Ok(carrier) => carrier,
                Err(projection) => {
                    return Err(RecoveredWalControlSignInstallError {
                        failure: RecoveredWalControlSignInstallFailure::Projection {
                            _projection: projection,
                        },
                    });
                }
            };
        let digest = carrier.installed_digest();
        if !carrier.validates_in_store(store) || self.entries.contains_key(&address) {
            return Err(RecoveredWalControlSignInstallError {
                failure: RecoveredWalControlSignInstallFailure::Carrier { _carrier: carrier },
            });
        }
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(
                DurableRecoveredWalControlSignWork { carrier, address },
            ),
        };
        debug_assert!(work.validates_at(address));
        let previous = self.entries.insert(address, work);
        debug_assert!(previous.is_none());
        Ok(InstalledRecoveredWalControlSignRegistryCut {
            registry: self,
            address,
            digest,
        })
    }

    /// Consume one exact durable Decision Fetch projection into its carrier.
    ///
    /// All projection, row, owner, address, geometry, digest, store, and
    /// vacancy checks precede the sole insertion. An exact coalesced ledger
    /// row is read-only; this method reconstructs only process-local authority.
    #[allow(clippy::result_large_err)]
    pub(super) fn install_recovered_wal_decision_fetch<'registry>(
        &'registry mut self,
        verified: &VerifiedHeightContext,
        store: &super::ledger::LifecycleLedgerStoreV1,
        ledger: &super::ledger::LifecycleLedgerV1,
        projection: AuthenticatedRecoveredWalDecisionFetchProjection,
    ) -> Result<
        InstalledRecoveredWalDecisionFetchRegistryCut<'registry>,
        RecoveredWalDecisionFetchInstallError,
    > {
        if !self.entries.is_empty()
            || !projection.is_exact(verified)
            || !store.load().is_ok_and(|opened| opened == *ledger)
        {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::Projection {
                    _projection: projection,
                },
            });
        }
        let records = ledger
            .records()
            .iter()
            .filter(|record| projection.names_record(record))
            .collect::<Vec<_>>();
        let [record] = records.as_slice() else {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::Projection {
                    _projection: projection,
                },
            });
        };
        if !projection.exactly_matches_record(record) {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::Projection {
                    _projection: projection,
                },
            });
        }
        let slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let Some(address) = ConcreteWorkAddress::new(record.owner(), record.ordinal(), slot) else {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::Projection {
                    _projection: projection,
                },
            });
        };
        let carrier =
            match projection.into_durable_carrier(address.owner, address.ordinal, address.slot) {
                Ok(carrier) => carrier,
                Err(projection) => {
                    return Err(RecoveredWalDecisionFetchInstallError {
                        failure: RecoveredWalDecisionFetchInstallFailure::Projection {
                            _projection: projection,
                        },
                    });
                }
            };
        let digest = carrier.installed_digest();
        if !carrier.validates_in_store(store) || self.entries.contains_key(&address) {
            return Err(RecoveredWalDecisionFetchInstallError {
                failure: RecoveredWalDecisionFetchInstallFailure::Carrier { _carrier: carrier },
            });
        }
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(
                DurableRecoveredWalDecisionFetchWork { carrier, address },
            ),
        };
        debug_assert!(work.validates_at(address));
        let previous = self.entries.insert(address, work);
        debug_assert!(previous.is_none());
        Ok(InstalledRecoveredWalDecisionFetchRegistryCut {
            registry: self,
            address,
            digest,
        })
    }

    /// Consume one exact recovered Decision body projection into its Apply carrier.
    ///
    /// The supplied ledger is already the fully authenticated prospective
    /// successor. Its exact four-row lineage, final Apply ordinal, carrier
    /// digest, and empty-registry vacancy are all checked before insertion.
    /// Splitting the staged value is permit-gated here so neither the cold
    /// adapter nor the concrete carrier can be substituted by a caller.
    #[allow(clippy::result_large_err)]
    pub(super) fn install_recovered_decision_apply<'registry>(
        &'registry mut self,
        verified: &VerifiedHeightContext,
        ledger: &super::ledger::LifecycleLedgerV1,
        projection: RecoveredDecisionApplyStagedStorageV1,
        effects: Vec<AdapterEffect>,
    ) -> Result<
        (
            ProductionLifecycleAdapterStartupV1,
            InstalledRecoveredDecisionApplyRegistryCut<'registry>,
        ),
        RecoveredDecisionApplyInstallError,
    > {
        if !self.entries.is_empty() || !projection.validates(verified) {
            return Err(RecoveredDecisionApplyInstallError::projection(
                "recovered Decision Apply failed exact registry preflight",
                projection,
                effects,
            ));
        }
        let (restaged, apply_ordinal, _) = match ledger.stage_recovered_decision_apply(&projection)
        {
            Ok(staged) => staged,
            Err(_) => {
                return Err(RecoveredDecisionApplyInstallError::projection(
                    "recovered Decision Apply ledger lineage is not exact",
                    projection,
                    effects,
                ));
            }
        };
        if restaged != *ledger {
            return Err(RecoveredDecisionApplyInstallError::projection(
                "recovered Decision Apply prospective ledger is incomplete",
                projection,
                effects,
            ));
        }
        let Some(record) = ledger
            .records()
            .iter()
            .find(|record| record.ordinal() == apply_ordinal)
        else {
            return Err(RecoveredDecisionApplyInstallError::projection(
                "recovered Decision Apply ledger has no final row",
                projection,
                effects,
            ));
        };
        let slot = PhysicalSlotId::for_capacity(CapacityClass::Effect, 0);
        let Some(address) = ConcreteWorkAddress::new(record.owner(), apply_ordinal, slot) else {
            return Err(RecoveredDecisionApplyInstallError::projection(
                "recovered Decision Apply address is invalid",
                projection,
                effects,
            ));
        };
        let (adapter, carrier) = match projection.into_registry_carrier(
            RecoveredDecisionApplyRegistryProjectionPermit::new(),
            verified,
            effects,
        ) {
            Ok(parts) => parts,
            Err((projection, effects)) => {
                return Err(RecoveredDecisionApplyInstallError::projection(
                    "recovered Decision Apply retained residual adapter effects",
                    projection,
                    effects,
                ));
            }
        };
        let digest = carrier.installed_digest();
        if !carrier.validates(verified) || self.entries.contains_key(&address) {
            return Err(RecoveredDecisionApplyInstallError::carrier(
                "recovered Decision Apply carrier disagrees with durable lineage",
                adapter,
                carrier,
            ));
        }
        let work = ConcreteLifecycleWork {
            digest,
            kind: ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(
                DurableRecoveredDecisionApplyWork {
                    carrier,
                    address,
                    dispatch_key: None,
                },
            ),
        };
        debug_assert!(work.validates_at(address));
        let previous = self.entries.insert(address, work);
        debug_assert!(previous.is_none());
        Ok((
            adapter,
            InstalledRecoveredDecisionApplyRegistryCut {
                registry: self,
                address,
                digest,
            },
        ))
    }

    /// Install the startup Serve/Producer batch only after proving the exact
    /// prospective Fetch/(optional Sign)/Serve/Producer census. Rejection is
    /// before both registry mutation and the publication callback.
    pub(super) fn install_certified_serve_startup_batch_before_publication<T, E>(
        &mut self,
        batch: PreparedCertifiedServeRegistryBatchV1,
        coordinator: &LifecycleCoordinator,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, CertifiedServeRegistryBatchPublicationError<E>> {
        if !batch.preflights_startup_registry(self, coordinator) {
            return Err(CertifiedServeRegistryBatchPublicationError::Preflight(
                batch,
            ));
        }
        self.install_certified_serve_batch_before_publication(batch, publish)
    }

    /// Install one fresh adjacent Serve/Producer batch only after comparing the
    /// complete current and prospective concrete census. No raw ordinal or
    /// digest enters this boundary.
    pub(super) fn install_certified_serve_fresh_batch_before_publication<T, E>(
        &mut self,
        batch: PreparedCertifiedServeRegistryBatchV1,
        current: &LifecycleCoordinator,
        staged: &LifecycleCoordinator,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, CertifiedServeRegistryBatchPublicationError<E>> {
        if !batch.preflights_fresh_registry(self, current, staged) {
            return Err(CertifiedServeRegistryBatchPublicationError::Preflight(
                batch,
            ));
        }
        self.install_certified_serve_batch_before_publication(batch, publish)
    }

    /// Install a complete Certified-Serve/ProducerTurn batch immediately
    /// around one durable publication. The full registry and batch are checked
    /// before the first insertion. Publication failure removes every inserted
    /// carrier and returns the reconstructed move-only batch.
    pub(super) fn install_certified_serve_batch_before_publication<T, E>(
        &mut self,
        batch: PreparedCertifiedServeRegistryBatchV1,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, CertifiedServeRegistryBatchPublicationError<E>> {
        if !batch.preflights_registry(self) {
            return Err(CertifiedServeRegistryBatchPublicationError::Preflight(
                batch,
            ));
        }
        let mut staged = StagedCertifiedServeRegistryBatch {
            entries: &mut self.entries,
            addresses: Vec::with_capacity(batch.entries.len()),
        };
        for (address, work) in batch.entries {
            staged.addresses.push(address);
            let displaced = staged.entries.insert(address, work);
            debug_assert!(displaced.is_none(), "complete preflight fixed vacancy");
            if displaced.is_some() {
                unreachable!("exclusive registry borrow cannot change after preflight")
            }
        }
        match publish() {
            Ok(published) => {
                staged.commit();
                Ok(published)
            }
            Err(error) => Err(CertifiedServeRegistryBatchPublicationError::Publication(
                error,
                staged.rollback(),
            )),
        }
    }

    /// Publish the exact terminal LedgerV1 successor while the registry's
    /// eventual Producer replacement is staged at the same address.
    ///
    /// Ledger failure restores the byte-for-byte incumbent before returning.
    /// Ledger success is followed only by infallible exact-address removals:
    /// Serve always leaves the registry, and cancellation removes Producer as
    /// well. No allocation or fallible callback occurs after Ledger fsync.
    pub(super) fn publish_certified_serve_terminal_transition<T, E>(
        &mut self,
        prepared: PreparedCertifiedServeTerminalRegistryTransitionV1,
        current: &LifecycleCoordinator,
        staged: &LifecycleCoordinator,
        lease: &TurnLease,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, CertifiedServeTerminalRegistryPublicationError<E>> {
        if !prepared.preflights_current(self, current, lease)
            || !prepared.preflights_exact_staged_successor(current, staged, lease)
        {
            return Err(CertifiedServeTerminalRegistryPublicationError::Preflight(
                prepared,
            ));
        }

        if prepared.outcome == super::TerminalOutcome::Cancelled {
            if !prepared.preflights_cancelled_successor(staged) {
                return Err(CertifiedServeTerminalRegistryPublicationError::Preflight(
                    prepared,
                ));
            }
            return match publish() {
                Ok(published) => {
                    drop(
                        self.entries
                            .remove(&prepared.serve_address)
                            .expect("terminal preflight retained the exact Serve carrier"),
                    );
                    drop(
                        self.entries
                            .remove(&prepared.producer_address)
                            .expect("cancel preflight retained the exact Producer carrier"),
                    );
                    Ok(published)
                }
                Err(error) => Err(CertifiedServeTerminalRegistryPublicationError::Publication(
                    error, prepared,
                )),
            };
        }

        let Some(replacement) = prepared.producer_replacement(staged) else {
            return Err(CertifiedServeTerminalRegistryPublicationError::Preflight(
                prepared,
            ));
        };
        let incumbent = std::mem::replace(
            self.entries
                .get_mut(&prepared.producer_address)
                .expect("terminal preflight retained the exact Producer carrier"),
            replacement,
        );
        let staged_registry = StagedCertifiedServeTerminalProducer {
            entries: &mut self.entries,
            producer_address: prepared.producer_address,
            incumbent: Some(incumbent),
        };
        match publish() {
            Ok(published) => {
                staged_registry.commit();
                drop(
                    self.entries
                        .remove(&prepared.serve_address)
                        .expect("terminal preflight retained the exact Serve carrier"),
                );
                Ok(published)
            }
            Err(error) => {
                staged_registry.rollback();
                Err(CertifiedServeTerminalRegistryPublicationError::Publication(
                    error, prepared,
                ))
            }
        }
    }

    /// Whether the registry contains exactly one internally consistent
    /// recovered-WAL authority carrier and no other work.
    ///
    /// This is the only non-empty startup shape into which the post-repair
    /// Ready-Fetch census may install. The phase-vote, control, Decision Fetch,
    /// or Decision Apply carrier remains the exclusive durable authority for
    /// its causal owner; Fetch carriers must use disjoint owners and addresses.
    pub(super) fn contains_only_exact_recovered_wal_authority(&self) -> bool {
        let Some((&address, work)) = self.entries.first_key_value() else {
            return false;
        };
        if self.entries.len() != 1 || !work.validates_at(address) {
            return false;
        }
        match &work.kind {
            ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign) => {
                sign.validates_at(address, work.digest)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign) => {
                sign.validates_at(address, work.digest)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch) => {
                fetch.validates_at(address, work.digest)
            }
            ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) => {
                apply.validates_at(address, work.digest)
            }
            _ => false,
        }
    }

    /// Classify zero or one exact WAL-owned startup carrier.
    ///
    /// `None` from this function means ambiguity (including phase and control
    /// together), while `Some(None)` is the exact zero-carrier shape.
    fn exact_recovered_wal_registry_slot(&self) -> Option<RecoveredWalRegistrySlotV1> {
        let mut signs = self
            .entries
            .iter()
            .filter_map(|(&address, work)| match &work.kind {
                ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign)
                    if work.validates_at(address) && sign.validates_at(address, work.digest) =>
                {
                    Some(RecoveredWalRegistrySlotV1::PhaseVote(address))
                }
                ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign)
                    if work.validates_at(address) && sign.validates_at(address, work.digest) =>
                {
                    Some(RecoveredWalRegistrySlotV1::ControlSign(address))
                }
                ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch)
                    if work.validates_at(address) && fetch.validates_at(address, work.digest) =>
                {
                    Some(RecoveredWalRegistrySlotV1::DecisionFetch(address))
                }
                ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply)
                    if work.validates_at(address) && apply.validates_at(address, work.digest) =>
                {
                    Some(RecoveredWalRegistrySlotV1::DecisionApply(address))
                }
                _ => None,
            });
        let first = signs.next().unwrap_or(RecoveredWalRegistrySlotV1::None);
        signs.next().is_none().then_some(first)
    }

    /// Preflight a complete recovered-Fetch batch beside the sole WAL authority.
    pub(super) fn preflights_recovered_fetches_alongside_wal_authority(
        &self,
        completions: &[&CertifiedFetchCompletion],
    ) -> bool {
        let Some((&sign_address, _)) = self.entries.first_key_value() else {
            return false;
        };
        let mut addresses = std::collections::BTreeSet::new();
        let mut owners = std::collections::BTreeSet::new();
        self.contains_only_exact_recovered_wal_authority()
            && completions.iter().all(|completion| {
                let address = completion.address();
                completion
                    .ready_digest()
                    .is_some_and(|digest| completion.validates(digest))
                    && address.owner != sign_address.owner
                    && !self.entries.contains_key(&address)
                    && addresses.insert(address)
                    && owners.insert(address.owner)
            })
    }

    /// Install one already-closed recovered Fetch completion.
    ///
    /// Callers must complete a whole-census empty-registry preflight first;
    /// failure still returns the exact move-only completion.
    pub(super) fn install_recovered_durable_fetch(
        &mut self,
        completion: CertifiedFetchCompletion,
    ) -> Result<(), (RegistryError, CertifiedFetchCompletion)> {
        let address = completion.address();
        let work = match ConcreteLifecycleWork::from_recovered_durable_fetch(completion) {
            Ok(work) => work,
            Err(completion) => return Err((RegistryError::CorruptWork, completion)),
        };
        let digest = work.digest();
        match self.install(address, digest, work) {
            Ok(()) => Ok(()),
            Err((error, work)) => {
                let ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) = work.kind
                else {
                    unreachable!("recovered Fetch installation retains its closed work kind")
                };
                Err((error, completion))
            }
        }
    }

    /// Verify complete equality between all installed startup Fetch carriers
    /// and all live coordinator Fetch rows.
    pub(super) fn exactly_covers_recovered_ready_fetches(
        &self,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        self.exactly_covers_recovered_ready_fetches_with_extra(
            coordinator,
            RecoveredWalRegistrySlotV1::None,
        )
    }

    /// Verify the complete post-repair startup registry: one exact recovered
    /// WAL authority plus every live Ready-Fetch row and no other carrier.
    pub(super) fn exactly_covers_recovered_ready_fetches_and_wal_authority(
        &self,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        let Some(sign) = self.exact_recovered_wal_registry_slot() else {
            return false;
        };
        !matches!(sign, RecoveredWalRegistrySlotV1::None)
            && self.exactly_covers_recovered_ready_fetches_with_extra(coordinator, sign)
    }

    fn exactly_covers_recovered_ready_fetches_with_extra(
        &self,
        coordinator: &LifecycleCoordinator,
        extra: RecoveredWalRegistrySlotV1,
    ) -> bool {
        let live_fetches = coordinator
            .records
            .values()
            .filter(|record| {
                record.work_class == LifecycleWorkClass::Fetch
                    && !matches!(record.state, super::LifecycleState::Terminal(_))
                    && extra.address().is_none_or(|address| {
                        record.owner != address.owner || record.ordinal != address.ordinal
                    })
            })
            .collect::<Vec<_>>();
        self.entries.len() == live_fetches.len() + usize::from(extra.address().is_some())
            && self.exact_optional_recovered_wal_authority(coordinator, extra)
            && live_fetches.into_iter().all(|record| {
                if record.state != super::LifecycleState::Ready || record.physical_slots.len() != 1
                {
                    return false;
                }
                let Some((&slot, &digest)) = record.physical_slots.first_key_value() else {
                    return false;
                };
                if record.episode.consumed_slots != std::collections::BTreeSet::from([slot])
                    || !record.episode.slot_universe.contains(&slot)
                {
                    return false;
                }
                let Some(address) = ConcreteWorkAddress::new(record.owner, record.ordinal, slot)
                else {
                    return false;
                };
                let Some(metadata) = coordinator.durable_records.get(&record.ordinal) else {
                    return false;
                };
                let candidate = CandidateAdmission::new(
                    record.key,
                    record.owner.causal_root(),
                    record.work_class,
                    record.stage,
                    InitialLifecycleState::Ready,
                    metadata.reconstruction_source,
                    metadata.payload,
                    metadata.replay_authority.clone(),
                    super::PhysicalGeometry::new(
                        record
                            .physical_slots
                            .iter()
                            .map(|(id, digest)| PhysicalSlot::new(*id, *digest)),
                        record.episode.slot_universe.iter().copied(),
                    ),
                    None,
                );
                self.entries.get(&address).is_some_and(|work| {
                    work.digest == digest
                        && work.validates_at(address)
                        && matches!(
                            &work.kind,
                            ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion)
                                if completion.ready_digest() == Some(digest)
                                    && completion.matches_recovered_candidate(&candidate)
                        )
                })
            })
    }

    fn serve_and_producer_carrier_count(&self) -> usize {
        self.entries
            .values()
            .filter(|work| {
                matches!(
                    &work.kind,
                    ConcreteLifecycleWorkKind::DurableCertifiedServe(_)
                        | ConcreteLifecycleWorkKind::DurableProducerTurn(_)
                )
            })
            .count()
    }

    /// Verify exact startup coverage for every live durable Fetch, Serve, and
    /// ProducerTurn row, with no additional concrete carrier.
    pub(super) fn exactly_covers_recovered_ready_work(
        &self,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        self.exactly_covers_recovered_ready_work_with_extra(
            coordinator,
            RecoveredWalRegistrySlotV1::None,
        )
    }

    /// Verify exact startup coverage beside the one recovered-WAL authority.
    pub(super) fn exactly_covers_recovered_ready_work_and_wal_authority(
        &self,
        coordinator: &LifecycleCoordinator,
    ) -> bool {
        let Some(sign) = self.exact_recovered_wal_registry_slot() else {
            return false;
        };
        !matches!(sign, RecoveredWalRegistrySlotV1::None)
            && self.exactly_covers_recovered_ready_work_with_extra(coordinator, sign)
    }

    fn exactly_covers_recovered_ready_work_with_extra(
        &self,
        coordinator: &LifecycleCoordinator,
        extra: RecoveredWalRegistrySlotV1,
    ) -> bool {
        self.exactly_covers_ready_work_with_extra(coordinator, extra, None)
    }

    fn exactly_covers_ready_work_with_extra(
        &self,
        coordinator: &LifecycleCoordinator,
        extra: RecoveredWalRegistrySlotV1,
        active_serve: Option<&TurnLease>,
    ) -> bool {
        let live = coordinator
            .records
            .values()
            .filter(|record| {
                matches!(
                    record.work_class,
                    LifecycleWorkClass::Fetch
                        | LifecycleWorkClass::CertifiedServe
                        | LifecycleWorkClass::ProducerTurn
                ) && !matches!(record.state, super::LifecycleState::Terminal(_))
                    && extra.address().is_none_or(|address| {
                        record.owner != address.owner || record.ordinal != address.ordinal
                    })
            })
            .collect::<Vec<_>>();
        self.entries.len() == live.len() + usize::from(extra.address().is_some())
            && self.exact_optional_recovered_wal_authority(coordinator, extra)
            && live.into_iter().all(|record| {
                let is_active_serve = active_serve.is_some_and(|lease| {
                    record.work_class == LifecycleWorkClass::CertifiedServe
                        && record.ordinal == lease.ordinal
                        && record.state == super::LifecycleState::Claimed(lease.id)
                });
                if record.work_class != LifecycleWorkClass::ProducerTurn
                    && record.state != super::LifecycleState::Ready
                    && !is_active_serve
                {
                    return false;
                }
                let Some((slot, digest)) =
                    exact_single_record_slot(record, record.work_class.capacity_class())
                else {
                    return false;
                };
                let Some(address) = ConcreteWorkAddress::new(record.owner, record.ordinal, slot)
                else {
                    return false;
                };
                let Some(metadata) = coordinator.durable_records.get(&record.ordinal) else {
                    return false;
                };
                self.entries.get(&address).is_some_and(|work| {
                    work.digest == digest
                        && work.validates_at(address)
                        && match (&work.kind, record.work_class) {
                            (
                                ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion),
                                LifecycleWorkClass::Fetch,
                            ) => {
                                let candidate = CandidateAdmission::new(
                                    record.key,
                                    record.owner.causal_root(),
                                    record.work_class,
                                    record.stage,
                                    InitialLifecycleState::Ready,
                                    metadata.reconstruction_source,
                                    metadata.payload,
                                    metadata.replay_authority.clone(),
                                    super::PhysicalGeometry::new(
                                        [PhysicalSlot::new(slot, digest)],
                                        [slot],
                                    ),
                                    None,
                                );
                                completion.ready_digest() == Some(digest)
                                    && completion.matches_recovered_candidate(&candidate)
                            }
                            (
                                ConcreteLifecycleWorkKind::DurableCertifiedServe(serve),
                                LifecycleWorkClass::CertifiedServe,
                            ) => active_serve.map_or_else(
                                || serve.matches_record(record, metadata, digest),
                                |lease| {
                                    if record.ordinal == lease.ordinal {
                                        serve
                                            .matches_claimed_record(record, metadata, digest, lease)
                                    } else {
                                        serve.matches_record(record, metadata, digest)
                                    }
                                },
                            ),
                            (
                                ConcreteLifecycleWorkKind::DurableProducerTurn(producer),
                                LifecycleWorkClass::ProducerTurn,
                            ) => producer.matches_record(record, metadata, digest),
                            _ => false,
                        }
                })
            })
    }

    fn exactly_covers_active_certified_serve_lease(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> bool {
        if coordinator.fault.is_some()
            || coordinator.active_lease.as_ref() != Some(lease)
            || lease.work_class != LifecycleWorkClass::CertifiedServe
        {
            return false;
        }
        let Some(sign) = self.exact_recovered_wal_registry_slot() else {
            return false;
        };
        self.exactly_covers_ready_work_with_extra(coordinator, sign, Some(lease))
    }

    /// Prove the complete private registry and exact active Serve lease without
    /// consulting caller-supplied request material.
    pub(super) fn preflight_certified_serve_terminal_owner_state(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> bool {
        if !self.exactly_covers_active_certified_serve_lease(coordinator, lease) {
            return false;
        }
        let Some(&producer_ordinal) = coordinator.producer_debts.get(&lease.ordinal) else {
            return false;
        };
        let (Some(serve), Some(serve_metadata), Some(producer), Some(producer_metadata)) = (
            coordinator.records.get(&lease.ordinal),
            coordinator.durable_records.get(&lease.ordinal),
            coordinator.records.get(&producer_ordinal),
            coordinator.durable_records.get(&producer_ordinal),
        ) else {
            return false;
        };
        let (Some((serve_slot, _)), Some((producer_slot, _))) = (
            exact_single_record_slot(serve, LifecycleWorkClass::CertifiedServe.capacity_class()),
            exact_single_record_slot(producer, LifecycleWorkClass::ProducerTurn.capacity_class()),
        ) else {
            return false;
        };
        let (Some(serve_address), Some(producer_address)) = (
            ConcreteWorkAddress::new(serve.owner, serve.ordinal, serve_slot),
            ConcreteWorkAddress::new(producer.owner, producer.ordinal, producer_slot),
        ) else {
            return false;
        };
        let (Some(serve_work), Some(producer_work)) = (
            self.entries.get(&serve_address),
            self.entries.get(&producer_address),
        ) else {
            return false;
        };
        matches!(
            (&serve_work.kind, &producer_work.kind),
            (
                ConcreteLifecycleWorkKind::DurableCertifiedServe(serve_carrier),
                ConcreteLifecycleWorkKind::DurableProducerTurn(producer_carrier),
            ) if serve_ordinal_pair_is_exact(serve, producer)
                && serve_metadata
                    .replay_authority
                    .same_persisted_family(&producer_metadata.replay_authority)
                && Arc::ptr_eq(
                    &serve_carrier.replay_evidence,
                    &producer_carrier.replay_evidence,
                )
        )
    }

    /// Join an exact signed request only after the complete owner-private state
    /// has independently passed preflight.
    pub(super) fn preflight_certified_serve_terminal_settlement(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        authenticated: &AuthenticatedCertifiedBodyRequest,
    ) -> bool {
        if !self.preflight_certified_serve_terminal_owner_state(coordinator, lease) {
            return false;
        }
        let Some(&producer_ordinal) = coordinator.producer_debts.get(&lease.ordinal) else {
            return false;
        };
        let (Some(serve_metadata), Some(producer_metadata)) = (
            coordinator.durable_records.get(&lease.ordinal),
            coordinator.durable_records.get(&producer_ordinal),
        ) else {
            return false;
        };
        serve_metadata
            .replay_authority
            .exactly_matches_certified_serve_request(authenticated)
            && producer_metadata
                .replay_authority
                .exactly_matches_certified_serve_request(authenticated)
    }

    /// Close one post-fsync terminal replay family over the already-preflighted
    /// active Serve and adjacent Producer carriers.
    pub(super) fn prepare_certified_serve_terminal_transition(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        authenticated: &AuthenticatedCertifiedBodyRequest,
        terminal: &CertifiedServeTerminalReplayAuthorityPairV1,
    ) -> Option<PreparedCertifiedServeTerminalRegistryTransitionV1> {
        if !self.preflight_certified_serve_terminal_settlement(coordinator, lease, authenticated) {
            return None;
        }
        let producer_ordinal = *coordinator.producer_debts.get(&lease.ordinal)?;
        let serve = coordinator.records.get(&lease.ordinal)?;
        let serve_metadata = coordinator.durable_records.get(&lease.ordinal)?;
        let producer = coordinator.records.get(&producer_ordinal)?;
        let producer_metadata = coordinator.durable_records.get(&producer_ordinal)?;
        if !terminal.exactly_advances_pending_records(
            coordinator.active_context,
            serve,
            serve_metadata,
            producer,
            producer_metadata,
        ) {
            return None;
        }
        let (serve_slot, _) =
            exact_single_record_slot(serve, LifecycleWorkClass::CertifiedServe.capacity_class())?;
        let (producer_slot, _) =
            exact_single_record_slot(producer, LifecycleWorkClass::ProducerTurn.capacity_class())?;
        let serve_address = ConcreteWorkAddress::new(serve.owner, serve.ordinal, serve_slot)?;
        let producer_address =
            ConcreteWorkAddress::new(producer.owner, producer.ordinal, producer_slot)?;
        let serve_work = self.entries.get(&serve_address)?;
        let ConcreteLifecycleWorkKind::DurableCertifiedServe(serve_carrier) = &serve_work.kind
        else {
            return None;
        };
        let terminal_replay_evidence = terminal.terminal_carrier_replay_evidence()?;
        Some(PreparedCertifiedServeTerminalRegistryTransitionV1 {
            serve_address,
            producer_address,
            outcome: terminal.terminal_outcome(),
            pending_replay_evidence: Arc::clone(&serve_carrier.replay_evidence),
            terminal_replay_evidence: Arc::new(terminal_replay_evidence),
        })
    }

    fn exact_optional_recovered_wal_authority(
        &self,
        coordinator: &LifecycleCoordinator,
        extra: RecoveredWalRegistrySlotV1,
    ) -> bool {
        let unsupported_live = coordinator
            .records
            .values()
            .filter(|record| {
                !matches!(record.state, super::LifecycleState::Terminal(_))
                    && !matches!(
                        record.work_class,
                        LifecycleWorkClass::Fetch
                            | LifecycleWorkClass::CertifiedServe
                            | LifecycleWorkClass::ProducerTurn
                    )
            })
            .collect::<Vec<_>>();
        match extra {
            RecoveredWalRegistrySlotV1::None => unsupported_live.is_empty(),
            RecoveredWalRegistrySlotV1::PhaseVote(address) => {
                let [record] = unsupported_live.as_slice() else {
                    return false;
                };
                if record.ordinal != address.ordinal {
                    return false;
                }
                self.entries.get(&address).is_some_and(|work| {
                    matches!(
                        &work.kind,
                        ConcreteLifecycleWorkKind::DurableRecoveredWalSign(sign)
                            if record.physical_slots.get(&address.slot) == Some(&work.digest)
                                && sign.matches_current_ready_record(address, work.digest, coordinator)
                    )
                })
            }
            RecoveredWalRegistrySlotV1::ControlSign(address) => {
                let [record] = unsupported_live.as_slice() else {
                    return false;
                };
                if record.ordinal != address.ordinal {
                    return false;
                }
                self.entries.get(&address).is_some_and(|work| {
                    matches!(
                        &work.kind,
                        ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(sign)
                            if record.physical_slots.get(&address.slot) == Some(&work.digest)
                                && sign.matches_current_ready_record(address, work.digest, coordinator)
                    )
                })
            }
            RecoveredWalRegistrySlotV1::DecisionFetch(address) => {
                if !unsupported_live.is_empty() {
                    return false;
                }
                let Some(record) = coordinator.records.get(&address.ordinal) else {
                    return false;
                };
                if record.owner != address.owner
                    || record.ordinal != address.ordinal
                    || record.work_class != LifecycleWorkClass::Fetch
                    || matches!(record.state, super::LifecycleState::Terminal(_))
                {
                    return false;
                }
                self.entries.get(&address).is_some_and(|work| {
                    matches!(
                        &work.kind,
                        ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(fetch)
                            if record.physical_slots.get(&address.slot) == Some(&work.digest)
                                && fetch.matches_current_ready_record(
                                    address,
                                    work.digest,
                                    coordinator,
                                )
                    )
                })
            }
            RecoveredWalRegistrySlotV1::DecisionApply(address) => {
                let [record] = unsupported_live.as_slice() else {
                    return false;
                };
                if record.ordinal != address.ordinal {
                    return false;
                }
                self.entries.get(&address).is_some_and(|work| {
                    matches!(
                        &work.kind,
                        ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply)
                            if record.physical_slots.get(&address.slot) == Some(&work.digest)
                                && apply.matches_current_ready_record(
                                    address,
                                    work.digest,
                                    coordinator,
                                )
                    )
                })
            }
        }
    }

    /// Install one work value without overwriting an incumbent address.
    ///
    /// Failure returns the move-only value to the caller so a higher-level
    /// admission transaction can roll back without cloning physical work.
    pub(super) fn install(
        &mut self,
        address: ConcreteWorkAddress,
        expected_digest: LifecycleDigest,
        work: ConcreteLifecycleWork,
    ) -> Result<(), (RegistryError, ConcreteLifecycleWork)> {
        if ConcreteWorkAddress::new(address.owner, address.ordinal, address.slot) != Some(address) {
            return Err((RegistryError::InvalidAddress, work));
        }
        if !work.validates_at(address) {
            return Err((RegistryError::CorruptWork, work));
        }
        if address.owner.causal_root() != work.causal_root() {
            return Err((RegistryError::CausalOwnerMismatch, work));
        }
        if work.digest != expected_digest {
            return Err((RegistryError::DigestMismatch, work));
        }
        if self.entries.contains_key(&address) {
            return Err((RegistryError::Occupied, work));
        }
        self.entries.insert(address, work);
        Ok(())
    }

    /// Install exact work, invoke durable publication, and synchronously undo
    /// the installation when publication fails or unwinds.
    ///
    /// The callback cannot access this exclusively borrowed registry, so the
    /// entry installed immediately before it remains the exact rollback target.
    pub(super) fn install_before_publication<T, E>(
        &mut self,
        address: ConcreteWorkAddress,
        expected_digest: LifecycleDigest,
        work: ConcreteLifecycleWork,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, RegistryPublicationError<E>> {
        if let Err((error, work)) = self.install(address, expected_digest, work) {
            return Err(RegistryPublicationError::Install(error, work));
        }
        let staged = StagedRegistryInstall {
            entries: &mut self.entries,
            address,
            armed: true,
        };
        match publish() {
            Ok(published) => {
                staged.commit();
                Ok(published)
            }
            Err(error) => {
                let work = staged.rollback();
                debug_assert!(work.validate_exact());
                debug_assert_eq!(work.digest, expected_digest);
                Err(RegistryPublicationError::Publication(error, work))
            }
        }
    }

    /// Replace one exact address before invoking a reversible publication.
    ///
    /// The incumbent remains recoverable until the callback succeeds. A
    /// callback error removes the replacement and restores the byte-for-byte
    /// incumbent before returning the replacement to the caller. Unwinding
    /// also restores the incumbent through an RAII guard. This map is
    /// exclusively borrowed across the callback, so no other registry entry
    /// can observe the staged value or invalidate the rollback address.
    ///
    /// `Err` is valid only when the callback proves that its external target
    /// did not commit. A durability-ambiguous dequeue or publication must
    /// instead cross the process fail-stop boundary; restoring this volatile
    /// map cannot undo an external transition.
    /// This generic seam accepts pending adapter work only. Certified-Fetch
    /// completion must use the specialized conversion below, which moves the
    /// incumbent binding into its closed carrier rather than constructing an
    /// independent replacement proof.
    pub(super) fn replace_before_publication<T, E>(
        &mut self,
        address: ConcreteWorkAddress,
        expected_incumbent_digest: LifecycleDigest,
        expected_replacement_digest: LifecycleDigest,
        replacement: ConcreteLifecycleWork,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<(T, ConcreteLifecycleWork), RegistryReplacementError<E>> {
        if ConcreteWorkAddress::new(address.owner, address.ordinal, address.slot) != Some(address) {
            return Err(RegistryReplacementError::Validation(
                RegistryError::InvalidAddress,
                replacement,
            ));
        }
        if !replacement.validates_at(address) {
            return Err(RegistryReplacementError::Validation(
                RegistryError::CorruptWork,
                replacement,
            ));
        }
        if !replacement.is_pending_adapter() {
            return Err(RegistryReplacementError::Validation(
                RegistryError::WrongWorkKind,
                replacement,
            ));
        }
        if address.owner.causal_root() != replacement.causal_root() {
            return Err(RegistryReplacementError::Validation(
                RegistryError::CausalOwnerMismatch,
                replacement,
            ));
        }
        if replacement.digest != expected_replacement_digest {
            return Err(RegistryReplacementError::Validation(
                RegistryError::DigestMismatch,
                replacement,
            ));
        }
        let Some(incumbent) = self.entries.get(&address) else {
            return Err(RegistryReplacementError::Validation(
                RegistryError::Missing,
                replacement,
            ));
        };
        if !incumbent.validates_at(address) {
            return Err(RegistryReplacementError::Validation(
                RegistryError::CorruptWork,
                replacement,
            ));
        }
        if !incumbent.is_pending_adapter() {
            return Err(RegistryReplacementError::Validation(
                RegistryError::WrongWorkKind,
                replacement,
            ));
        }
        if address.owner.causal_root() != incumbent.causal_root() {
            return Err(RegistryReplacementError::Validation(
                RegistryError::CausalOwnerMismatch,
                replacement,
            ));
        }
        if incumbent.digest != expected_incumbent_digest {
            return Err(RegistryReplacementError::Validation(
                RegistryError::DigestMismatch,
                replacement,
            ));
        }

        let incumbent = self
            .entries
            .insert(address, replacement)
            .expect("validated replacement address retains its incumbent");
        let staged = StagedRegistryReplacement {
            entries: &mut self.entries,
            address,
            incumbent: Some(incumbent),
        };
        match publish() {
            Ok(published) => Ok((published, staged.commit())),
            Err(error) => {
                let replacement = staged.rollback();
                debug_assert!(replacement.validate_exact());
                debug_assert_eq!(replacement.digest, expected_replacement_digest);
                Err(RegistryReplacementError::Publication(error, replacement))
            }
        }
    }

    /// Prepare an exact incumbent-to-completion conversion without mutation.
    ///
    /// The sealed selector capability is borrowed only for equality validation.
    /// It is deliberately not stored in the returned token: successful
    /// conversion moves the incumbent registry binding and never mints or
    /// retains a second causal proof. Raw response, responder, hash, queue
    /// identity, and pending-binding inputs are not accepted here.
    pub(super) fn prepare_certified_fetch_completion(
        &mut self,
        location: CertifiedFetchWaitingLocation,
        authority: CertifiedFetchCompletionAuthority<'_>,
    ) -> Result<PreparedCertifiedFetchCompletion<'_>, CertifiedFetchCompletionError> {
        let ingress_identity = authority.ingress_identity();
        let request_hash = authority.request_hash();
        let response_hash = authority.response_hash();
        let authenticated_responder = authority.authenticated_responder();
        let authenticated_response = authority.authenticated_response();
        let candidate_pending = authority.candidate_pending();
        let address = location.address();
        if ConcreteWorkAddress::new(location.owner, location.ordinal, location.slot)
            != Some(address)
        {
            return Err(CertifiedFetchCompletionError::InvalidLocation);
        }
        if ingress_identity.physical_admission_ordinal() == 0
            || !ingress_identity_matches_round(
                ingress_identity,
                authenticated_response.manifest.round,
            )
        {
            return Err(CertifiedFetchCompletionError::InvalidQueueIdentity);
        }
        if authenticated_response.request_hash != request_hash
            || HashOf::new(authenticated_response) != response_hash
        {
            return Err(CertifiedFetchCompletionError::ResponseFamilyMismatch);
        }

        let incumbent = self
            .entries
            .get(&address)
            .ok_or(CertifiedFetchCompletionError::MissingIncumbent)?;
        if !incumbent.validates_at(address) {
            return Err(CertifiedFetchCompletionError::CorruptIncumbent);
        }
        let ConcreteLifecycleWorkKind::PendingAdapter {
            effect: incumbent_effect,
            pending: incumbent_pending,
        } = &incumbent.kind
        else {
            return Err(CertifiedFetchCompletionError::WrongIncumbentShape);
        };
        if !matches!(incumbent_effect, AdapterEffect::FetchBody { .. }) {
            return Err(CertifiedFetchCompletionError::WrongIncumbentShape);
        }
        if location.owner.causal_root() != incumbent.causal_root() {
            return Err(CertifiedFetchCompletionError::ForeignCausalOwner);
        }
        if authority.causal_root() != incumbent.causal_root() {
            return Err(CertifiedFetchCompletionError::CandidateBindingMismatch);
        }
        if incumbent.digest != location.incumbent_digest {
            return Err(CertifiedFetchCompletionError::IncumbentDigestMismatch);
        }
        if candidate_pending != incumbent_pending
            || !candidate_pending.exactly_binds_adapter_effect(incumbent_effect)
        {
            return Err(CertifiedFetchCompletionError::CandidateBindingMismatch);
        }
        if !fetch_effect_matches_response(incumbent_effect, authenticated_response) {
            return Err(CertifiedFetchCompletionError::ResponseFamilyMismatch);
        }
        let replay_origin = AuthenticatedCertifiedFetchReplayOriginV1::from_completion_authority(
            &authority,
            incumbent_effect,
        )
        .ok_or(CertifiedFetchCompletionError::InvalidReplayEvidence)?;

        Ok(PreparedCertifiedFetchCompletion {
            registry: self,
            location,
            ingress_identity,
            request_hash,
            response_hash,
            response_round: authenticated_response.manifest.round,
            response_subject: authenticated_response.manifest.subject,
            response_manifest_hash: HashOf::new(&authenticated_response.manifest),
            authenticated_responder: authenticated_responder.clone(),
            replay_origin,
        })
    }

    /// Prepare execution of one exact closed certified-Fetch completion.
    ///
    /// The lease must name the completion's immutable owner, record ordinal,
    /// sole physical slot, and installed response digest, and it must retain
    /// the coordinator's exact independent `FetchBody` stage. No row is taken
    /// or rewritten by this check.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_certified_fetch_execution(
        &mut self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
    ) -> Result<PreparedCertifiedFetchExecution<'_>, CertifiedFetchExecutionError> {
        if lease.work_class() != LifecycleWorkClass::Fetch
            || !lease
                .work_class()
                .accepts_stage(lease.key().phase(), lease.stage())
            || lease.physical_slots().len() != 1
            || !lease.physical_slots().contains_key(&slot)
            || slot.capacity_class() != Some(lease.work_class().capacity_class())
        {
            return Err(CertifiedFetchExecutionError::InvalidLeaseShape);
        }

        let address = self
            .validated_lease_address(lease, slot)
            .map_err(CertifiedFetchExecutionError::Registry)?;
        let work = self
            .entries
            .get(&address)
            .expect("validated certified-Fetch execution address remains present");
        let ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) = &work.kind else {
            return Err(CertifiedFetchExecutionError::WrongWorkKind);
        };
        let AdapterEffect::FetchBody {
            certificate: Some(certificate),
            ..
        } = &completion.incumbent_effect
        else {
            return Err(CertifiedFetchExecutionError::InvalidCompletionShape);
        };
        let active_context =
            LifecycleContext::new(lease.key().context(), lease.key().round().height());
        if certified_fetch_lifecycle_key(
            active_context,
            certificate.round,
            certificate.proposal_round,
            certificate.subject,
            certificate.phase,
            certificate.execution_commitment,
        ) != Some(lease.key())
        {
            return Err(CertifiedFetchExecutionError::InvalidLeaseShape);
        }
        if !completion.validates(work.digest) {
            return Err(CertifiedFetchExecutionError::InvalidCompletionShape);
        }

        Ok(PreparedCertifiedFetchExecution {
            registry: self,
            address,
        })
    }

    /// Prepare execution of one exact closed durable Store carrier.
    ///
    /// In addition to the address and digest checks shared by all registry
    /// leases, this replays the authenticated adapter projection under the
    /// supplied height context. The projected semantic key, causal owner, and
    /// complete one-slot physical geometry must be identical to the claimed
    /// Store lease. No row is taken or rewritten by this check.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_durable_store_execution(
        &mut self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
        verified: &VerifiedHeightContext,
    ) -> Result<PreparedDurableStoreExecution<'_>, DurableStoreExecutionError> {
        if lease.work_class() != LifecycleWorkClass::Store
            || lease.key().phase() != LifecyclePhase::Store
            || lease.stage().kind() != LifecycleStageKind::StoreBody
            || lease.stage().predecessor_scope() != PredecessorScope::Independent
            || !lease
                .work_class()
                .accepts_stage(lease.key().phase(), lease.stage())
            || lease.physical_slots().len() != 1
            || !lease.physical_slots().contains_key(&slot)
            || slot.capacity_class() != Some(LifecycleWorkClass::Store.capacity_class())
        {
            return Err(DurableStoreExecutionError::InvalidLeaseShape);
        }

        let address = self
            .validated_lease_address(lease, slot)
            .map_err(DurableStoreExecutionError::Registry)?;
        let work = self
            .entries
            .get(&address)
            .expect("validated durable Store execution address remains present");
        let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &work.kind else {
            return Err(DurableStoreExecutionError::WrongWorkKind);
        };
        if !store.validates(work.digest) {
            return Err(DurableStoreExecutionError::InvalidStoreShape);
        }

        let candidate = store
            .project_candidate(verified)
            .map_err(DurableStoreExecutionError::Projection)?;
        let expected_payload = durable_validate_body_payload(&store.durable_receipt)
            .ok_or(DurableStoreExecutionError::InvalidProjection)?;
        let (projected_slots, projected_universe, projected_consumed) = candidate
            .physical_geometry
            .normalized()
            .map_err(|_| DurableStoreExecutionError::InvalidProjection)?;
        let lease_slots = lease
            .physical_slots()
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if candidate.key != lease.key()
            || candidate.causal_root != lease.owner().causal_root()
            || candidate.work_class != LifecycleWorkClass::Store
            || candidate.stage != lease.stage()
            || candidate.initial_state != InitialLifecycleState::Ready
            || candidate.reconstruction_source != lease.owner().causal_root().digest()
            || candidate.payload != expected_payload
            || candidate.producer_turn.is_some()
            || projected_slots != *lease.physical_slots()
            || projected_universe != lease_slots
            || projected_consumed != lease_slots
        {
            return Err(DurableStoreExecutionError::InvalidProjection);
        }

        Ok(PreparedDurableStoreExecution {
            registry: self,
            address,
        })
    }

    /// Prepare execution of one exact closed durable Validate carrier.
    ///
    /// The lease, installed carrier, verified projection, and normalized
    /// physical geometry must all describe the same independent one-slot
    /// `ValidateBody` work. No row is taken or rewritten by this check.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_durable_validate_execution(
        &mut self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
        verified: &VerifiedHeightContext,
    ) -> Result<PreparedDurableValidateExecution<'_>, DurableValidateExecutionError> {
        if lease.work_class() != LifecycleWorkClass::Validate
            || lease.key().phase() != LifecyclePhase::Validate
            || lease.stage().kind() != LifecycleStageKind::ValidateBody
            || lease.stage().predecessor_scope() != PredecessorScope::Independent
            || !lease
                .work_class()
                .accepts_stage(lease.key().phase(), lease.stage())
            || lease.physical_slots().len() != 1
            || !lease.physical_slots().contains_key(&slot)
            || slot.capacity_class() != Some(LifecycleWorkClass::Validate.capacity_class())
        {
            return Err(DurableValidateExecutionError::InvalidLeaseShape);
        }

        let address = self
            .validated_lease_address(lease, slot)
            .map_err(DurableValidateExecutionError::Registry)?;
        let work = self
            .entries
            .get(&address)
            .expect("validated durable Validate execution address remains present");
        let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
            return Err(DurableValidateExecutionError::WrongWorkKind);
        };
        if !validate.validates(work.digest) {
            return Err(DurableValidateExecutionError::InvalidValidateShape);
        }

        let candidate = validate
            .project_candidate(verified)
            .map_err(DurableValidateExecutionError::Projection)?;
        let expected_payload = durable_validate_body_payload(&validate.durable_receipt)
            .ok_or(DurableValidateExecutionError::InvalidProjection)?;
        let (projected_slots, projected_universe, projected_consumed) = candidate
            .physical_geometry
            .normalized()
            .map_err(|_| DurableValidateExecutionError::InvalidProjection)?;
        let lease_slots = lease
            .physical_slots()
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if candidate.key != lease.key()
            || candidate.causal_root != lease.owner().causal_root()
            || candidate.work_class != LifecycleWorkClass::Validate
            || candidate.stage != lease.stage()
            || candidate.initial_state != InitialLifecycleState::Ready
            || candidate.reconstruction_source != lease.owner().causal_root().digest()
            || candidate.payload != expected_payload
            || candidate.producer_turn.is_some()
            || projected_slots.len() != 1
            || projected_universe.len() != 1
            || projected_consumed.len() != 1
            || projected_slots != *lease.physical_slots()
            || projected_universe != lease_slots
            || projected_consumed != lease_slots
        {
            return Err(DurableValidateExecutionError::InvalidProjection);
        }

        Ok(PreparedDurableValidateExecution {
            registry: self,
            address,
            lifecycle_key: lease.key(),
            lifecycle_stage: lease.stage(),
        })
    }

    /// Classify one exact Ready Validate carrier without granting scheduler authority.
    ///
    /// The caller supplies coordinator-owned address and digest coordinates.
    /// Successful classification proves only the process-local carrier shape;
    /// the coordinator must still bind it into its complete Ready census.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn classify_ready_validate_carrier(
        &self,
        address: ConcreteWorkAddress,
        expected_digest: LifecycleDigest,
    ) -> Result<ReadyValidateCarrierSeal, ReadyValidateCarrierError> {
        let work = self
            .entries
            .get(&address)
            .ok_or(ReadyValidateCarrierError::Registry(RegistryError::Missing))?;
        if !work.validates_at(address) {
            return Err(ReadyValidateCarrierError::Registry(
                RegistryError::CorruptWork,
            ));
        }
        if work.digest != expected_digest {
            return Err(ReadyValidateCarrierError::Registry(
                RegistryError::DigestMismatch,
            ));
        }
        match &work.kind {
            ConcreteLifecycleWorkKind::DurableValidateBody(validate)
                if validate.validates(expected_digest) =>
            {
                let payload = durable_validate_body_payload(&validate.durable_receipt)
                    .ok_or(ReadyValidateCarrierError::InvalidCarrier)?;
                Ok(ReadyValidateCarrierSeal {
                    address,
                    digest: expected_digest,
                    kind: ReadyValidateCarrierKind::ExecuteBody,
                    payload,
                })
            }
            ConcreteLifecycleWorkKind::DurableValidateCompletion(completion)
                if completion.validates(expected_digest) =>
            {
                match (
                    completion.outcome.validated_receipt(),
                    completion.outcome.rejection_identity(),
                    completion.outcome.missing_merge_sidecar(),
                ) {
                    (Some(receipt), None, None)
                        if validate_validated_receipt_authority(&completion.incumbent, receipt)
                            .is_ok() =>
                    {
                        let payload =
                            durable_validate_body_payload(&completion.incumbent.durable_receipt)
                                .ok_or(ReadyValidateCarrierError::InvalidCarrier)?;
                        Ok(ReadyValidateCarrierSeal {
                            address,
                            digest: expected_digest,
                            kind: ReadyValidateCarrierKind::ValidatedCompletion,
                            payload,
                        })
                    }
                    (None, Some(BodyValidationRejectionIdentity::Rejected), None) => {
                        let payload =
                            durable_validate_body_payload(&completion.incumbent.durable_receipt)
                                .ok_or(ReadyValidateCarrierError::InvalidCarrier)?;
                        Ok(ReadyValidateCarrierSeal {
                            address,
                            digest: expected_digest,
                            kind: ReadyValidateCarrierKind::RejectedCompletion,
                            payload,
                        })
                    }
                    _ => Err(ReadyValidateCarrierError::InvalidCarrier),
                }
            }
            ConcreteLifecycleWorkKind::PendingAdapter { .. }
            | ConcreteLifecycleWorkKind::CertifiedFetchCompletion(_)
            | ConcreteLifecycleWorkKind::DurableStoreBody(_)
            | ConcreteLifecycleWorkKind::DurableValidateBody(_)
            | ConcreteLifecycleWorkKind::DurableValidateCompletion(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalControlSign(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredWalDecisionFetch(_)
            | ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(_)
            | ConcreteLifecycleWorkKind::DurableCertifiedServe(_)
            | ConcreteLifecycleWorkKind::DurableProducerTurn(_) => {
                Err(ReadyValidateCarrierError::WrongWorkKind)
            }
        }
    }

    /// Attest one exact Ready recovered Decision Apply without exposing its carrier.
    ///
    /// This is a read-only join over the coordinator's complete logical row,
    /// durable metadata, reverse indexes, and the registry's immutable closed
    /// carrier. Success discloses only the typed bounded-I/O demand and opaque
    /// exact-position key needed by the production scheduler; it grants no
    /// execution or extraction authority.
    pub(super) fn attest_ready_recovered_decision_apply(
        &self,
        coordinator: &LifecycleCoordinator,
        ordinal: u128,
    ) -> Result<ReadyRecoveredDecisionApplyAttestation, ReadyRecoveredDecisionApplyAttestationError>
    {
        let Some(record) = coordinator.records.get(&ordinal) else {
            return Err(ReadyRecoveredDecisionApplyAttestationError::InvalidCoordinatorIndex);
        };
        let Some(metadata) = coordinator.durable_records.get(&ordinal) else {
            return Err(ReadyRecoveredDecisionApplyAttestationError::InvalidCoordinatorIndex);
        };
        let Some((slot, digest)) =
            exact_single_record_slot(record, LifecycleWorkClass::Apply.capacity_class())
        else {
            return Err(ReadyRecoveredDecisionApplyAttestationError::InvalidCoordinatorIndex);
        };
        if coordinator.fault.is_some()
            || coordinator.active_lease.is_some()
            || record.ordinal != ordinal
            || record.work_class != LifecycleWorkClass::Apply
            || record.key.phase() != LifecyclePhase::Apply
            || record.stage.kind() != LifecycleStageKind::ApplyDecision
            || record.stage.predecessor_scope() != PredecessorScope::Independent
            || record.state != super::LifecycleState::Ready
            || !record.episode.frozen_predecessors.is_empty()
            || coordinator
                .episode_authority
                .universe_for(record.key)
                .as_ref()
                != Some(&record.episode.universe)
            || !coordinator.episode_authority.admits_slots(
                record.work_class.capacity_class(),
                &record.episode.slot_universe,
            )
            || !coordinator.ready_index.contains(&ordinal)
            || coordinator.key_index.get(&record.key) != Some(&ordinal)
            || coordinator.owner_index.get(&record.owner.causal_root()) != Some(&record.owner)
            || coordinator
                .records
                .values()
                .filter(|candidate| candidate.ordinal == ordinal)
                .count()
                != 1
            || coordinator
                .records
                .values()
                .filter(|candidate| candidate.key == record.key)
                .count()
                != 1
            || coordinator
                .key_index
                .values()
                .filter(|candidate| **candidate == ordinal)
                .count()
                != 1
            || coordinator
                .owner_index
                .values()
                .filter(|owner| **owner == record.owner)
                .count()
                != 1
            || metadata.continuation != super::schema::DurableContinuation::None
            || !matches!(metadata.payload, DurablePayloadReference::BodyFrame(_))
        {
            return Err(ReadyRecoveredDecisionApplyAttestationError::InvalidCoordinatorIndex);
        }
        let address = ConcreteWorkAddress::new(record.owner, ordinal, slot)
            .ok_or(ReadyRecoveredDecisionApplyAttestationError::InvalidCoordinatorIndex)?;
        if self
            .entries
            .keys()
            .filter(|candidate| candidate.owner == record.owner)
            .count()
            != 1
        {
            return Err(ReadyRecoveredDecisionApplyAttestationError::InvalidCoordinatorIndex);
        }
        let work = self.entries.get(&address).ok_or(
            ReadyRecoveredDecisionApplyAttestationError::Registry(RegistryError::Missing),
        )?;
        if !work.validates_at(address) {
            return Err(ReadyRecoveredDecisionApplyAttestationError::Registry(
                RegistryError::CorruptWork,
            ));
        }
        if work.digest != digest {
            return Err(ReadyRecoveredDecisionApplyAttestationError::Registry(
                RegistryError::DigestMismatch,
            ));
        }
        let ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) = &work.kind else {
            return Err(ReadyRecoveredDecisionApplyAttestationError::WrongWorkKind);
        };
        if !apply.matches_current_ready_record(address, digest, coordinator) {
            return Err(ReadyRecoveredDecisionApplyAttestationError::InvalidCarrier);
        }
        Ok(ReadyRecoveredDecisionApplyAttestation {
            demand: ReadyRecoveredDecisionApplyDemand::BoundedIo,
            dispatch_key: RecoveredDecisionApplyDispatchKeyV1::new(
                coordinator.active_context,
                address,
                digest,
            ),
            _seal: ReadyRecoveredDecisionApplyAttestationSeal,
        })
    }

    /// Project one exact claimed recovered Decision Apply into its dedicated worker task.
    ///
    /// The coordinator must still retain the sole active lease and the registry
    /// must still contain the unchanged closed carrier at its exact Effect/0
    /// address. Success consumes a registry-minted move-only dispatch identity;
    /// no generic adapter effect, receipt, pending binding, or candidate parts
    /// cross this boundary.
    pub(super) fn prepare_recovered_decision_apply_dispatch(
        &mut self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
    ) -> Result<
        PreparedRecoveredDecisionApplyDispatch<'_>,
        RecoveredDecisionApplyDispatchProjectionError,
    > {
        if coordinator.fault.is_some()
            || coordinator.active_lease.as_ref() != Some(lease)
            || lease.work_class() != LifecycleWorkClass::Apply
            || lease.key().phase() != LifecyclePhase::Apply
            || lease.stage().kind() != LifecycleStageKind::ApplyDecision
            || lease.stage().predecessor_scope() != PredecessorScope::Independent
            || lease.physical_slots().len() != 1
        {
            return Err(RecoveredDecisionApplyDispatchProjectionError::InvalidLease);
        }
        let Some((&slot, &digest)) = lease.physical_slots().first_key_value() else {
            return Err(RecoveredDecisionApplyDispatchProjectionError::InvalidLease);
        };
        if slot.capacity_class() != Some(LifecycleWorkClass::Apply.capacity_class()) {
            return Err(RecoveredDecisionApplyDispatchProjectionError::InvalidLease);
        }
        let address = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)
            .ok_or(RecoveredDecisionApplyDispatchProjectionError::InvalidLease)?;
        let work = self.entries.get_mut(&address).ok_or(
            RecoveredDecisionApplyDispatchProjectionError::Registry(RegistryError::Missing),
        )?;
        if !work.validates_at(address) {
            return Err(RecoveredDecisionApplyDispatchProjectionError::Registry(
                RegistryError::CorruptWork,
            ));
        }
        if work.digest != digest {
            return Err(RecoveredDecisionApplyDispatchProjectionError::Registry(
                RegistryError::DigestMismatch,
            ));
        }
        let ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) = &mut work.kind else {
            return Err(RecoveredDecisionApplyDispatchProjectionError::WrongWorkKind);
        };
        if !apply.matches_claimed_record(address, digest, coordinator, lease) {
            return Err(RecoveredDecisionApplyDispatchProjectionError::InvalidCarrier);
        }
        if apply.dispatch_key.is_some() {
            return Err(RecoveredDecisionApplyDispatchProjectionError::AlreadyDispatched);
        }
        let identity = RecoveredDecisionApplyDispatchIdentityV1::new(
            coordinator.active_context,
            address,
            digest,
        );
        let task = apply
            .carrier
            .project_recovered_apply_task(identity)
            .ok_or(RecoveredDecisionApplyDispatchProjectionError::InvalidCarrier)?;
        let key = task.dispatch_key();
        Ok(PreparedRecoveredDecisionApplyDispatch {
            work: apply,
            task: Some(task),
            key,
        })
    }

    /// Bind one guarded Applied worker result to the exact in-flight carrier.
    pub(super) fn prepare_recovered_decision_apply_terminal_transition(
        &self,
        coordinator: &LifecycleCoordinator,
        lease: &TurnLease,
        completion: &crate::sumeragi::v2_apply::RecoveredDecisionApplyCompletionV1,
    ) -> Option<(
        PreparedRecoveredDecisionApplyTerminalTransitionV1,
        crate::sumeragi::v2::RecoveredDecisionApplyAdapterCompletionAuthorityV1,
    )> {
        if coordinator.fault.is_some()
            || coordinator.active_lease.as_ref() != Some(lease)
            || lease.work_class() != LifecycleWorkClass::Apply
            || lease.key().phase() != LifecyclePhase::Apply
            || lease.stage().kind() != LifecycleStageKind::ApplyDecision
            || lease.stage().predecessor_scope() != PredecessorScope::Independent
            || lease.physical_slots().len() != 1
        {
            return None;
        }
        let (&slot, &digest) = lease.physical_slots().first_key_value()?;
        let address = ConcreteWorkAddress::new(lease.owner(), lease.ordinal(), slot)?;
        let work = self.entries.get(&address)?;
        let ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) = &work.kind else {
            return None;
        };
        let dispatch_key = completion.dispatch_key();
        if work.digest != digest
            || !work.validates_at(address)
            || !apply.matches_claimed_record(address, digest, coordinator, lease)
            || apply.dispatch_key != Some(dispatch_key)
            || !dispatch_key.matches(coordinator.active_context, address, digest)
        {
            return None;
        }
        let authority = apply.carrier.project_recovered_apply_completion(
            RecoveredDecisionApplyCompletionProjectionPermit::new(),
            completion,
        )?;
        Some((
            PreparedRecoveredDecisionApplyTerminalTransitionV1 {
                address,
                digest,
                dispatch_key,
                _linearity: RecoveredDecisionApplyTerminalTransitionLinearity,
            },
            authority,
        ))
    }

    /// Publish one exact recovered Apply terminal around LedgerV1 fsync.
    ///
    /// Every logical and physical check occurs before `publish`. Success is
    /// followed only by the infallible removal of the prevalidated carrier.
    pub(super) fn publish_recovered_decision_apply_terminal_transition<T, E>(
        &mut self,
        prepared: PreparedRecoveredDecisionApplyTerminalTransitionV1,
        current: &LifecycleCoordinator,
        staged: &LifecycleCoordinator,
        lease: &TurnLease,
        publish: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, RecoveredDecisionApplyTerminalPublicationError<E>> {
        let Some(work) = self.entries.get(&prepared.address) else {
            return Err(RecoveredDecisionApplyTerminalPublicationError::Preflight(
                prepared,
            ));
        };
        let ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) = &work.kind else {
            return Err(RecoveredDecisionApplyTerminalPublicationError::Preflight(
                prepared,
            ));
        };
        let exact_current = work.digest == prepared.digest
            && work.validates_at(prepared.address)
            && apply.dispatch_key == Some(prepared.dispatch_key)
            && apply.matches_claimed_record(prepared.address, prepared.digest, current, lease);

        let mut expected = current.stage_durable_transaction();
        expected.reduce_settle_turn(lease.clone(), super::TurnOutcome::Advanced, None);
        let same_ledger_target = matches!(
            (&expected.ledger_store, &staged.ledger_store),
            (Some(expected_store), Some(staged_store))
                if expected_store.same_publication_target(staged_store)
        );
        let exact_staged = expected.episode_authority == staged.episode_authority
            && expected.active_context == staged.active_context
            && expected.records == staged.records
            && expected.key_index == staged.key_index
            && expected.owner_index == staged.owner_index
            && expected.ready_index == staged.ready_index
            && expected.admission_waits == staged.admission_waits
            && expected.active_lease == staged.active_lease
            && expected.high_water == staged.high_water
            && expected.next_lease == staged.next_lease
            && expected.durable_records == staged.durable_records
            && expected.capacity_geometry == staged.capacity_geometry
            && expected.capacity_used == staged.capacity_used
            && expected.capacity_generation == staged.capacity_generation
            && expected.observed_generation == staged.observed_generation
            && expected.producer_debts == staged.producer_debts
            && expected.fault == staged.fault
            && same_ledger_target
            && staged.fault.is_none()
            && staged.active_lease.is_none()
            && staged
                .records
                .get(&prepared.address.ordinal)
                .is_some_and(|record| {
                    record.state
                        == super::LifecycleState::Terminal(super::TerminalOutcome::Advanced)
                });
        if !exact_current || !exact_staged {
            return Err(RecoveredDecisionApplyTerminalPublicationError::Preflight(
                prepared,
            ));
        }
        match publish() {
            Ok(value) => {
                drop(
                    self.entries
                        .remove(&prepared.address)
                        .expect("recovered Apply preflight retained the exact carrier"),
                );
                Ok(value)
            }
            Err(error) => Err(RecoveredDecisionApplyTerminalPublicationError::Publication(
                error, prepared,
            )),
        }
    }

    /// Prepare execution of one exact Ready durable Validate completion.
    ///
    /// The claimed lease must retain the original independent Validate
    /// lifecycle identity while its sole physical slot names the installed
    /// outcome-bound replacement digest. The retained incumbent is replayed
    /// through authenticated projection, and the complete closed outcome is
    /// revalidated before an exclusive, drop-inert registry borrow is issued.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn prepare_ready_durable_validate_execution(
        &mut self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
        verified: &VerifiedHeightContext,
    ) -> Result<PreparedReadyDurableValidateExecution<'_>, ReadyDurableValidateExecutionError> {
        if lease.work_class() != LifecycleWorkClass::Validate
            || lease.key().phase() != LifecyclePhase::Validate
            || lease.stage().kind() != LifecycleStageKind::ValidateBody
            || lease.stage().predecessor_scope() != PredecessorScope::Independent
            || !lease
                .work_class()
                .accepts_stage(lease.key().phase(), lease.stage())
            || lease.physical_slots().len() != 1
            || !lease.physical_slots().contains_key(&slot)
            || slot.capacity_class() != Some(LifecycleWorkClass::Validate.capacity_class())
        {
            return Err(ReadyDurableValidateExecutionError::InvalidLeaseShape);
        }

        let address = self
            .validated_lease_address(lease, slot)
            .map_err(ReadyDurableValidateExecutionError::Registry)?;
        let work = self
            .entries
            .get(&address)
            .expect("validated Ready Validate completion address remains present");
        let ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) = &work.kind else {
            return Err(ReadyDurableValidateExecutionError::WrongWorkKind);
        };
        let AdapterEffect::ValidateBody {
            tag: _,
            round,
            subject,
        } = &completion.incumbent.effect
        else {
            return Err(ReadyDurableValidateExecutionError::InvalidCompletionShape);
        };
        let Some(candidate_statement) = completion.incumbent.pending.candidate_statement() else {
            return Err(ReadyDurableValidateExecutionError::InvalidCompletionShape);
        };
        if !completion.validates(work.digest)
            || completion.address != address
            || completion.incumbent.address != address
            || candidate_statement.context_id() != round.context_id
            || candidate_statement.proposal_round() != *round
            || candidate_statement.subject() != Some(*subject)
            || completion.incumbent.durable_receipt.context_id() != round.context_id
            || completion.incumbent.durable_receipt.round() != *round
            || completion.incumbent.durable_receipt.subject() != *subject
            || completion.incumbent.durable_receipt.manifest_hash()
                != completion.incumbent.expected_manifest_hash
            || completion.outcome.durable_body() != &completion.incumbent.durable_receipt
        {
            return Err(ReadyDurableValidateExecutionError::InvalidCompletionShape);
        }

        let outcome_kind = match (
            completion.outcome.validated_receipt(),
            completion.outcome.rejection_identity(),
            completion.outcome.missing_merge_sidecar(),
        ) {
            (Some(receipt), None, None)
                if receipt.durable() == &completion.incumbent.durable_receipt
                    && receipt.durable().manifest_hash()
                        == completion.incumbent.expected_manifest_hash
                    && validate_validated_receipt_authority(&completion.incumbent, receipt)
                        .is_ok() =>
            {
                ReadyDurableValidateOutcomeKind::Validated
            }
            (None, Some(BodyValidationRejectionIdentity::Rejected), None) => {
                ReadyDurableValidateOutcomeKind::Rejected
            }
            _ => return Err(ReadyDurableValidateExecutionError::InvalidCompletionShape),
        };
        let expected_reservation = match outcome_kind {
            ReadyDurableValidateOutcomeKind::Validated => None,
            ReadyDurableValidateOutcomeKind::Rejected => Some(CapacityClass::Consensus),
        };
        if lease
            .output_reservation()
            .map(|reservation| reservation.class())
            != expected_reservation
        {
            return Err(ReadyDurableValidateExecutionError::InvalidLeaseShape);
        }

        let candidate = completion
            .incumbent
            .project_candidate(verified)
            .map_err(ReadyDurableValidateExecutionError::Projection)?;
        let expected_payload = durable_validate_body_payload(&completion.incumbent.durable_receipt)
            .ok_or(ReadyDurableValidateExecutionError::InvalidProjection)?;
        let (projected_slots, projected_universe, projected_consumed) = candidate
            .physical_geometry
            .normalized()
            .map_err(|_| ReadyDurableValidateExecutionError::InvalidProjection)?;
        let lease_slots = lease
            .physical_slots()
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let incumbent_slots = BTreeMap::from([(slot, completion.incumbent_digest)]);
        if candidate.key != lease.key()
            || candidate.causal_root != lease.owner().causal_root()
            || candidate.work_class != LifecycleWorkClass::Validate
            || candidate.stage != lease.stage()
            || candidate.initial_state != InitialLifecycleState::Ready
            || candidate.reconstruction_source != lease.owner().causal_root().digest()
            || candidate.payload != expected_payload
            || candidate.producer_turn.is_some()
            || projected_slots != incumbent_slots
            || projected_universe != lease_slots
            || projected_consumed != lease_slots
        {
            return Err(ReadyDurableValidateExecutionError::InvalidProjection);
        }

        Ok(PreparedReadyDurableValidateExecution {
            registry: self,
            address,
            outcome_kind,
            lease: lease.clone(),
        })
    }

    /// Reattach one executed Validate outcome only if its original closed row
    /// remains byte-for-byte authoritative at the exact address and digest.
    ///
    /// Failure returns the complete move-only execution token. Success only
    /// establishes a new exclusive borrow; neither path changes the registry.
    // The sole outer consumer joins this reattachment with typed same-address
    // carrier installation and the coordinator Ready replacement. Waiting,
    // Ready, and physical carriers are excluded from the lifecycle ledger, so
    // that volatile cut deliberately performs no ledger rewrite.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(super) fn reattach_durable_validate_execution(
        &mut self,
        executed: ExecutedDurableValidateExecution,
    ) -> Result<
        PreparedDurableValidateCompletion<'_>,
        (
            DurableValidateExecutionError,
            ExecutedDurableValidateExecution,
        ),
    > {
        let request = &executed.request;
        let exact = (|| {
            if ConcreteWorkAddress::new(
                request.address.owner,
                request.address.ordinal,
                request.address.slot,
            ) != Some(request.address)
            {
                return Err(DurableValidateExecutionError::Registry(
                    RegistryError::InvalidAddress,
                ));
            }
            let work = self.entries.get(&request.address).ok_or(
                DurableValidateExecutionError::Registry(RegistryError::Missing),
            )?;
            if !work.validates_at(request.address) {
                return Err(DurableValidateExecutionError::Registry(
                    RegistryError::CorruptWork,
                ));
            }
            if request.address.owner.causal_root() != work.causal_root() {
                return Err(DurableValidateExecutionError::Registry(
                    RegistryError::CausalOwnerMismatch,
                ));
            }
            if work.digest != request.incumbent_digest {
                return Err(DurableValidateExecutionError::Registry(
                    RegistryError::DigestMismatch,
                ));
            }
            let ConcreteLifecycleWorkKind::DurableValidateBody(validate) = &work.kind else {
                return Err(DurableValidateExecutionError::WrongWorkKind);
            };
            let AdapterEffect::ValidateBody {
                tag,
                round,
                subject,
            } = &validate.effect
            else {
                return Err(DurableValidateExecutionError::InvalidValidateShape);
            };
            if validate.address != request.address
                || *tag != request.tag
                || *round != request.round
                || *subject != request.subject
                || validate.durable_receipt != request.durable_receipt
                || validate.expected_manifest_hash != request.expected_manifest_hash
                || !validate
                    .pending
                    .exactly_binds_adapter_effect(&validate.effect)
                || validate.pending.causal_lifecycle_key() != &request.causal_lifecycle_key
                || validate.pending.candidate_statement() != request.candidate_statement
                || request.lifecycle_key.phase() != LifecyclePhase::Validate
                || request.lifecycle_stage.kind() != LifecycleStageKind::ValidateBody
                || request.lifecycle_stage.predecessor_scope() != PredecessorScope::Independent
            {
                return Err(DurableValidateExecutionError::InvalidValidateShape);
            }
            if executed.outcome.durable_body() != &request.durable_receipt {
                return Err(DurableValidateExecutionError::InvalidValidationReceipt);
            }
            if let Some(receipt) = executed.outcome.validated_receipt() {
                validate_validated_receipt_authority(validate, receipt)?;
            }
            Ok(())
        })();
        if let Err(error) = exact {
            return Err((error, executed));
        }

        Ok(PreparedDurableValidateCompletion {
            _registry: self,
            executed,
        })
    }

    /// Reattach the complete executed dispatch and its exact wake authority.
    ///
    /// This is the sole registry entry to volatile Validate completion. Every
    /// failure returns the original move-only dispatch and leaves the map
    /// untouched; success retains the exclusive borrow in a sealed preflight.
    #[cfg_attr(not(test), allow(dead_code))]
    #[allow(clippy::result_large_err)]
    pub(super) fn prepare_executed_durable_validate_completion(
        &mut self,
        dispatch: ExecutedDurableValidateDispatch,
    ) -> Result<
        PreparedExecutedDurableValidateCompletion<'_>,
        (
            DurableValidateCompletionPublicationError,
            ExecutedDurableValidateDispatch,
        ),
    > {
        let ExecutedDurableValidateDispatch { executed, wake } = dispatch;
        let prepared = match self.reattach_durable_validate_execution(executed) {
            Ok(prepared) => prepared,
            Err((error, executed)) => {
                return Err((
                    DurableValidateCompletionPublicationError::Registry(
                        DurableValidateCompletionConversionError::Execution(error),
                    ),
                    ExecutedDurableValidateDispatch { executed, wake },
                ));
            }
        };
        let PreparedDurableValidateCompletion {
            _registry: registry,
            executed,
        } = prepared;
        let dispatch = ExecutedDurableValidateDispatch { executed, wake };
        let request = &dispatch.executed.request;
        let expected_source = durable_validation_wait_source_for_request(request);
        if dispatch.wake.wait_token.source() != expected_source
            || dispatch.wake.wait_token.observed_generation() == u64::MAX
        {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidWakeAuthority,
                ),
                dispatch,
            ));
        }
        let Some(outcome_kind) = durable_validate_outcome_kind(dispatch.outcome()) else {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidOutcome,
                ),
                dispatch,
            ));
        };
        let replacement_digest = durable_validate_completion_digest(
            request.incumbent_digest,
            request.expected_manifest_hash,
            dispatch.outcome(),
        );
        if matches!(
            outcome_kind,
            DurableValidateOutcomeKind::Validated | DurableValidateOutcomeKind::Rejected
        ) && replacement_digest.is_none_or(|digest| digest == request.incumbent_digest)
        {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidReplacementDigest,
                ),
                dispatch,
            ));
        }
        if outcome_kind == DurableValidateOutcomeKind::DeferredMergeSidecar
            && replacement_digest.is_some()
        {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidOutcome,
                ),
                dispatch,
            ));
        }
        let Some(payload) = durable_validate_body_payload(&request.durable_receipt) else {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidOutcome,
                ),
                dispatch,
            ));
        };
        if !super::body_pipeline_transition::durable_validate_payload_is_exact(
            request.lifecycle_key,
            payload,
        ) {
            return Err((
                DurableValidateCompletionPublicationError::Registry(
                    DurableValidateCompletionConversionError::InvalidOutcome,
                ),
                dispatch,
            ));
        }
        let authority = DurableValidateCompletionAuthority {
            address: request.address,
            incumbent_digest: request.incumbent_digest,
            replacement_digest,
            wait_token: dispatch.wake.wait_token,
            outcome_kind,
            lifecycle_key: request.lifecycle_key,
            lifecycle_stage: request.lifecycle_stage,
            payload,
        };
        Ok(PreparedExecutedDurableValidateCompletion {
            registry,
            dispatch,
            authority,
        })
    }

    /// Borrow the still-pending adapter effect advertised by one lease slot.
    /// Closed carriers fail rather than re-executing their retained effects.
    pub(super) fn borrow_for_lease(
        &self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
    ) -> Result<&AdapterEffect, RegistryError> {
        let address = self.validated_lease_address(lease, slot)?;
        let work = self
            .entries
            .get(&address)
            .expect("validated lease address remains present");
        if !work.is_pending_adapter() {
            return Err(RegistryError::WrongWorkKind);
        }
        Ok(work.effect())
    }

    /// Consume the complete still-pending adapter work advertised by one lease slot once.
    ///
    /// Returning the sealed pending authority together with the effect is
    /// essential: execution may report `Blocked` or `Replenished`, in which
    /// case a later atomic settlement must be able to restore the incumbent
    /// without reminting its causal binding. Closed-carrier consumption
    /// remains unavailable until its typed executor lands.
    pub(super) fn take_for_lease(
        &mut self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
    ) -> Result<ConcreteLifecycleWork, RegistryError> {
        let address = self.validated_lease_address(lease, slot)?;
        if !self
            .entries
            .get(&address)
            .expect("validated lease address remains present")
            .is_pending_adapter()
        {
            return Err(RegistryError::WrongWorkKind);
        }
        Ok(self
            .entries
            .remove(&address)
            .expect("validated lease address remains present"))
    }

    /// Remove only the exact digest installed by a failed outer transaction.
    pub(super) fn rollback_exact(
        &mut self,
        address: ConcreteWorkAddress,
        expected_digest: LifecycleDigest,
    ) -> Result<ConcreteLifecycleWork, RegistryError> {
        let work = self.entries.get(&address).ok_or(RegistryError::Missing)?;
        if !work.validates_at(address) {
            return Err(RegistryError::CorruptWork);
        }
        if address.owner.causal_root() != work.causal_root() {
            return Err(RegistryError::CausalOwnerMismatch);
        }
        if work.digest != expected_digest {
            return Err(RegistryError::DigestMismatch);
        }
        Ok(self
            .entries
            .remove(&address)
            .expect("validated rollback address remains present"))
    }

    fn validated_lease_address(
        &self,
        lease: &TurnLease,
        slot: PhysicalSlotId,
    ) -> Result<ConcreteWorkAddress, RegistryError> {
        let address = ConcreteWorkAddress::new(lease.owner, lease.ordinal, slot)
            .ok_or(RegistryError::InvalidAddress)?;
        let expected_digest = lease
            .physical_slots
            .get(&slot)
            .ok_or(RegistryError::DigestMismatch)?;
        let work = self.entries.get(&address).ok_or(RegistryError::Missing)?;
        if !work.validates_at(address) {
            return Err(RegistryError::CorruptWork);
        }
        if address.owner.causal_root() != work.causal_root() {
            return Err(RegistryError::CausalOwnerMismatch);
        }
        if work.digest != *expected_digest {
            return Err(RegistryError::DigestMismatch);
        }
        Ok(address)
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(super) fn certified_serve_and_producer_carrier_counts(&self) -> (usize, usize) {
        self.entries
            .values()
            .fold((0, 0), |counts, work| match &work.kind {
                ConcreteLifecycleWorkKind::DurableCertifiedServe(_) => (counts.0 + 1, counts.1),
                ConcreteLifecycleWorkKind::DurableProducerTurn(_) => (counts.0, counts.1 + 1),
                _ => counts,
            })
    }

    #[cfg(test)]
    pub(super) fn one_certified_serve_pair_shares_replay_family(&self) -> bool {
        let serves = self
            .entries
            .values()
            .filter_map(|work| match &work.kind {
                ConcreteLifecycleWorkKind::DurableCertifiedServe(serve) => Some(serve),
                _ => None,
            })
            .collect::<Vec<_>>();
        let producers = self
            .entries
            .values()
            .filter_map(|work| match &work.kind {
                ConcreteLifecycleWorkKind::DurableProducerTurn(producer) => Some(producer),
                _ => None,
            })
            .collect::<Vec<_>>();
        let ([serve], [producer]) = (serves.as_slice(), producers.as_slice()) else {
            return false;
        };
        Arc::ptr_eq(&serve.replay_evidence, &producer.replay_evidence)
    }

    #[cfg(test)]
    /// Remove one exact Serve carrier to exercise owner-private census faults.
    pub(super) fn remove_one_certified_serve_carrier_for_test(&mut self) -> bool {
        let address = self.entries.iter().find_map(|(address, work)| {
            matches!(
                &work.kind,
                ConcreteLifecycleWorkKind::DurableCertifiedServe(_)
            )
            .then_some(*address)
        });
        address.is_some_and(|address| self.entries.remove(&address).is_some())
    }

    #[cfg(test)]
    pub(super) fn exactly_contains(
        &self,
        address: ConcreteWorkAddress,
        effect: &AdapterEffect,
    ) -> bool {
        self.entries
            .get(&address)
            .is_some_and(|work| work.validates_at(address) && work.effect() == effect)
    }
}

#[allow(dead_code)]
impl<'a> PreparedCertifiedFetchExecution<'a> {
    /// Return the exact reducer tag and authenticated manifest accepted by the
    /// direct adapter preview. Both are derived from the installed completion;
    /// neither can be supplied independently by the caller.
    pub(super) fn adapter_preview_inputs(&self) -> (EventTag, &wire::PayloadManifest) {
        let work = self
            .registry
            .entries
            .get(&self.address)
            .expect("prepared certified-Fetch completion remains installed");
        let ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) = &work.kind else {
            unreachable!("prepared certified-Fetch execution retains a closed completion")
        };
        completion
            .replay_evidence
            .adapter_preview_inputs(
                &completion.incumbent_effect,
                &completion.incumbent_pending,
                &completion.durable_receipt,
            )
            .expect("prepared certified-Fetch completion retains exact durable replay inputs")
    }

    /// Borrow the durable body proof retained by the exact completion.
    ///
    /// The receipt remains nested and non-decomposable; callers may use it only
    /// for the future body-catalog equality check and canonical reload.
    pub(super) fn durable_body_receipt(&self) -> &DurableBodyReceipt {
        let work = self
            .registry
            .entries
            .get(&self.address)
            .expect("prepared certified-Fetch completion remains installed");
        let ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) = &work.kind else {
            unreachable!("prepared certified-Fetch execution retains a closed completion")
        };
        &completion.durable_receipt
    }

    /// Seal the ordinal-free pending binding for the exact Store effect emitted
    /// by the direct adapter preview.
    ///
    /// This pure projection checks the certified predecessor, exact tag/round/
    /// subject, inherited candidate statement, unchanged causal key, and a new
    /// physical effect identity. Neither success nor failure changes the
    /// installed completion.
    pub(super) fn seal_store_successor(
        self,
        successor: &AdapterEffect,
    ) -> Result<PreparedCertifiedFetchStoreSuccessor<'a>, CertifiedFetchExecutionError> {
        let (
            store_effect,
            store_pending,
            store_digest,
            durable_body,
            expected_manifest_hash,
            replay_evidence,
        ) = {
            let work = self
                .registry
                .entries
                .get(&self.address)
                .expect("prepared certified-Fetch completion remains installed");
            let ConcreteLifecycleWorkKind::CertifiedFetchCompletion(completion) = &work.kind else {
                return Err(CertifiedFetchExecutionError::InvalidCompletionShape);
            };
            if !completion.validates(work.digest) {
                return Err(CertifiedFetchExecutionError::InvalidCompletionShape);
            }
            let Some(store_pending) = completion
                .incumbent_pending
                .project_certified_fetch_store_successor(&completion.incumbent_effect, successor)
            else {
                return Err(CertifiedFetchExecutionError::InvalidStoreSuccessor);
            };
            if store_pending.causal_lifecycle_key()
                != completion.incumbent_pending.causal_lifecycle_key()
                || store_pending.candidate_statement()
                    != completion.incumbent_pending.candidate_statement()
                || store_pending.exact_effect_identity()
                    == completion.incumbent_pending.exact_effect_identity()
                || !store_pending.exactly_binds_adapter_effect(successor)
            {
                return Err(CertifiedFetchExecutionError::InvalidStoreSuccessor);
            }
            let store_digest = digest_from_hash(store_pending.exact_effect_identity());
            let durable_body = completion.durable_receipt.clone();
            let Some(ready_projection) = completion.replay_evidence.project_durable_ready_fetch(
                &completion.incumbent_effect,
                &completion.incumbent_pending,
                &completion.durable_receipt,
            ) else {
                return Err(CertifiedFetchExecutionError::InvalidCompletionShape);
            };
            let expected_manifest_hash = ready_projection.expected_manifest_hash();
            let Some(replay_evidence) = completion.replay_evidence.project_store(
                &completion.incumbent_effect,
                &completion.incumbent_pending,
                &completion.durable_receipt,
                successor,
            ) else {
                return Err(CertifiedFetchExecutionError::InvalidStoreSuccessor);
            };
            (
                successor.clone(),
                store_pending,
                store_digest,
                durable_body,
                expected_manifest_hash,
                replay_evidence,
            )
        };

        Ok(PreparedCertifiedFetchStoreSuccessor {
            _registry: self.registry,
            _completion_address: self.address,
            _store_effect: store_effect,
            _store_digest: store_digest,
            _store_pending: store_pending,
            _durable_body: durable_body,
            _expected_manifest_hash: expected_manifest_hash,
            _replay_evidence: replay_evidence,
        })
    }
}

#[allow(dead_code)]
impl<'a> PreparedDurableStoreExecution<'a> {
    fn durable_store(&self) -> &DurableStoreBody {
        let work = self
            .registry
            .entries
            .get(&self.address)
            .expect("prepared durable Store carrier remains installed");
        let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &work.kind else {
            unreachable!("prepared durable Store execution retains its closed carrier")
        };
        store
    }

    /// Return the exact reducer coordinates accepted by the direct
    /// `BodyStored` adapter preview.
    pub(super) fn adapter_preview_inputs(
        &self,
    ) -> (EventTag, wire::ConsensusRound, wire::BlockSubject) {
        let AdapterEffect::StoreBody {
            tag,
            round,
            subject,
        } = &self.durable_store().effect
        else {
            unreachable!("prepared durable Store carrier retains its Store effect")
        };
        (*tag, *round, *subject)
    }

    /// Borrow the exact post-fsync body receipt retained by the Store carrier.
    pub(super) fn durable_body_receipt(&self) -> &DurableBodyReceipt {
        &self.durable_store().durable_receipt
    }

    /// Return the manifest hash transferred independently from the parent response.
    pub(super) fn expected_manifest_hash(&self) -> HashOf<wire::PayloadManifest> {
        self.durable_store().expected_manifest_hash
    }

    /// Seal the ordinal-free pending binding for the exact Validate effect
    /// emitted by the direct `BodyStored` adapter preview.
    ///
    /// The Store's full inherited candidate statement and causal root must be
    /// unchanged, while the concrete effect identity must be replaced by the
    /// exact Validate identity. Neither success nor failure changes the Store
    /// row retained under the exclusive registry borrow.
    pub(super) fn seal_validate_successor(
        self,
        successor: &AdapterEffect,
    ) -> Result<PreparedDurableStoreValidateSuccessor<'a>, DurableStoreExecutionError> {
        let (
            validate_effect,
            validate_pending,
            validate_digest,
            durable_body,
            expected_manifest_hash,
            replay_evidence,
        ) = {
            let work = self
                .registry
                .entries
                .get(&self.address)
                .expect("prepared durable Store carrier remains installed");
            let ConcreteLifecycleWorkKind::DurableStoreBody(store) = &work.kind else {
                return Err(DurableStoreExecutionError::InvalidStoreShape);
            };
            if !store.validates(work.digest) {
                return Err(DurableStoreExecutionError::InvalidStoreShape);
            }
            let Some(validate_pending) = store
                .pending
                .project_store_validate_successor(&store.effect, successor)
            else {
                return Err(DurableStoreExecutionError::InvalidValidateSuccessor);
            };
            if validate_pending.causal_lifecycle_key() != store.pending.causal_lifecycle_key()
                || super::CausalRoot::new(digest_from_hash(validate_pending.causal_lifecycle_key()))
                    != store.address.owner.causal_root()
                || validate_pending.candidate_statement() != store.pending.candidate_statement()
                || validate_pending.exact_effect_identity() == store.pending.exact_effect_identity()
                || !validate_pending.exactly_binds_adapter_effect(successor)
            {
                return Err(DurableStoreExecutionError::InvalidValidateSuccessor);
            }
            let validate_digest = digest_from_hash(validate_pending.exact_effect_identity());
            if validate_digest == work.digest {
                return Err(DurableStoreExecutionError::InvalidValidateSuccessor);
            }
            let Some(replay_evidence) = store.replay_evidence.project_validate(
                &store.effect,
                &store.durable_receipt,
                successor,
                &validate_pending,
            ) else {
                return Err(DurableStoreExecutionError::InvalidValidateSuccessor);
            };
            (
                successor.clone(),
                validate_pending,
                validate_digest,
                store.durable_receipt.clone(),
                store.expected_manifest_hash,
                replay_evidence,
            )
        };

        Ok(PreparedDurableStoreValidateSuccessor {
            _registry: self.registry,
            _store_address: self.address,
            _validate_effect: validate_effect,
            _validate_digest: validate_digest,
            _validate_pending: validate_pending,
            _durable_body: durable_body,
            _expected_manifest_hash: expected_manifest_hash,
            _replay_evidence: replay_evidence,
        })
    }
}

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

/// Reconstruct one exact recovered Validate parent directly from durable storage.
///
/// This is the production restart-only replacement for the scheduler/lease
/// preparation path used by live work. LedgerV1 supplies the immutable owner
/// and ordinal, the body store transfers one exact revalidated marker, and the
/// runtime consumes the authenticated WAL vote into its successor. The holder
/// remains the only concrete-registry owner and returns only the existing
/// opaque authenticated repair plus its exact opened ledger wrapper.
#[cfg_attr(not(test), allow(dead_code))]
#[allow(clippy::result_large_err, clippy::too_many_lines)]
pub(super) fn reconstruct_recovered_wal_validate_parent<'registry, 'body>(
    registry: &'registry mut ConcreteLifecycleWorkRegistry,
    verified: &VerifiedHeightContext,
    body_store: &'body mut V2BodyStore,
    ledger_root: &Path,
    recovered: RecoveredWalVoteSign,
) -> Result<
    (
        OpenedRecoveredWalValidateLedger,
        AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
    ),
    RecoveredWalParentFactoryError<'body>,
> {
    let context = projection::lifecycle_context(verified.context());
    let (store, opened) = match super::ledger::LifecycleLedgerStoreV1::open(ledger_root, context) {
        Ok(opened) => opened,
        Err(error) => {
            return Err(RecoveredWalParentFactoryError {
                failure: RecoveredWalParentFactoryFailure::LedgerOpen {
                    _error: error,
                    _recovered: recovered,
                },
            });
        }
    };
    let ledger = OpenedRecoveredWalValidateLedger { store, opened };
    let body = match body_store.detach_recovered_validated_parent(&recovered) {
        Ok(body) => body,
        Err(error) => {
            return Err(RecoveredWalParentFactoryError {
                failure: RecoveredWalParentFactoryFailure::BodyMarker {
                    _error: error,
                    _ledger: ledger,
                    _recovered: recovered,
                },
            });
        }
    };
    if !body.exactly_matches_vote(&recovered) {
        return Err(RecoveredWalParentFactoryError {
            failure: RecoveredWalParentFactoryFailure::LedgerParent {
                _ledger: ledger,
                _body: body,
                _recovered: recovered,
            },
        });
    }
    let Some(parent) = ledger
        .opened
        .authenticate_recovered_wal_validate_parent(&recovered)
    else {
        return Err(RecoveredWalParentFactoryError {
            failure: RecoveredWalParentFactoryFailure::LedgerParent {
                _ledger: ledger,
                _body: body,
                _recovered: recovered,
            },
        });
    };
    if !body.exactly_matches_ledger_parent(context, &parent) {
        return Err(RecoveredWalParentFactoryError {
            failure: RecoveredWalParentFactoryFailure::LedgerParent {
                _ledger: ledger,
                _body: body,
                _recovered: recovered,
            },
        });
    }
    let successor = match reconstruct_recovered_wal_vote_successor(&parent, recovered) {
        Ok(successor) => successor,
        Err(recovered) => {
            return Err(RecoveredWalParentFactoryError {
                failure: RecoveredWalParentFactoryFailure::RuntimeParent {
                    _ledger: ledger,
                    _body: body,
                    _recovered: recovered,
                },
            });
        }
    };
    let repair = match authenticate_recovered_wal_vote_lifecycle_from_ledger_parent(
        verified, &parent, successor,
    ) {
        Ok(repair) => repair,
        Err(error) => {
            return Err(RecoveredWalParentFactoryError {
                failure: RecoveredWalParentFactoryFailure::Lifecycle {
                    _ledger: ledger,
                    _body: body,
                    _error: error,
                },
            });
        }
    };
    let registry_preflight = (|| {
        if !parent.matches_candidate(repair.parent())
            || ledger
                .opened
                .stage_authenticated_wal_vote_repair(&repair)
                .is_err()
        {
            return None;
        }
        let (physical, universe, consumed) = repair.parent().physical_geometry.normalized().ok()?;
        if physical.len() != 1 || universe.len() != 1 || consumed != universe {
            return None;
        }
        let (&slot, &incumbent_digest) = physical.first_key_value()?;
        if slot != PhysicalSlotId::for_capacity(CapacityClass::Effect, 0) {
            return None;
        }
        let address = ConcreteWorkAddress::new(parent.owner(), parent.ordinal(), slot)?;
        registry
            .entries
            .keys()
            .all(|installed| installed.owner != parent.owner())
            .then_some((address, incumbent_digest))
    })();
    let Some((address, incumbent_digest)) = registry_preflight else {
        return Err(RecoveredWalParentFactoryError {
            failure: RecoveredWalParentFactoryFailure::RegistryParent {
                _ledger: ledger,
                _repair: repair,
                _body: body,
            },
        });
    };

    // All fallible parent, ledger, body, and registry checks precede this
    // transfer. From here the detached marker moves directly into the sealed
    // completion and no pre-join error can discard it.
    let outcome = body.into_validation_outcome();
    let validated = outcome
        .validated_receipt()
        .expect("a recovered validated-body cut transfers one success outcome");
    let durable_receipt = validated.durable().clone();
    // Restart recovery obtains this hash from the semantically revalidated
    // marker reopened by this exact body-store instance. Unlike the live
    // transport path, there is no independently in-flight manifest carrier;
    // the checksummed receipt and store manifest were already compared before
    // the marker entered the validated recovery catalog.
    let expected_manifest_hash = durable_receipt.manifest_hash();
    let recovered_body_marker = durable_receipt.clone();
    let installed_digest =
        durable_validate_completion_digest(incumbent_digest, expected_manifest_hash, &outcome)
            .expect("a validated recovered parent has one completion digest");
    let validation = DetachedRecoveredValidateCompletion {
        address,
        installed_digest,
        incumbent_address: address,
        incumbent_digest,
        durable_receipt,
        expected_manifest_hash,
        replay_evidence: DetachedValidateReplayEvidenceV1::RecoveredBodyMarker(
            recovered_body_marker,
        ),
        outcome,
    };
    let authority = AuthenticatedRecoveredWalValidateLifecycleRepair {
        repair,
        validation,
        reservation: RecoveredWalValidateRegistryReservation {
            registry,
            parent_address: address,
            child: None,
        },
    };
    debug_assert!(authority.concrete_pair_and_validation_are_exact());
    Ok((ledger, authority))
}

#[cfg(test)]
impl super::concrete_admission::LifecycleWorkRegistryHolder {
    /// Count only closed recovered-WAL Sign rows after an installed cut drops.
    /// This test oracle exposes no address, effect, pending binding, or receipt.
    pub(crate) fn recovered_wal_sign_entry_count_for_test(&self) -> usize {
        self.registry_for_test()
            .entries
            .values()
            .filter(|work| {
                matches!(
                    &work.kind,
                    ConcreteLifecycleWorkKind::DurableRecoveredWalSign(_)
                )
            })
            .count()
    }

    /// Assemble and install a genuine ordinary-Proposal validated completion.
    ///
    /// The retained signed Proposal enters the same authenticated fair-ingress
    /// replay mint as production dispatch. This helper then projects its exact
    /// Fetch-to-Store-to-Validate lineage, installs the closed completion, and
    /// returns only its exact scheduler coordinates. Callers must still enter
    /// the production Ready preparation path to borrow or detach the carrier.
    #[allow(clippy::too_many_lines)]
    fn install_remote_proposal_validate_completion_for_test(
        &mut self,
        verified: &VerifiedHeightContext,
        tag: EventTag,
        proposal: wire::Proposal,
        manifest: wire::PayloadManifest,
        validated_receipt: ValidatedBodyReceipt,
    ) -> (TurnLease, PhysicalSlotId, CandidateAdmission) {
        assert_eq!(proposal.manifest, manifest);
        let fetch_effect = AdapterEffect::FetchBody {
            tag,
            round: proposal.round,
            subject: proposal.subject,
            manifest: Some(manifest.clone()),
            certified_sources: Vec::new(),
            certificate: None,
        };
        let mut fetch_ownership = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&fetch_effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, 1)],
        )
        .expect("bind genuine remote-Proposal Fetch fixture")
        .pop()
        .expect("one remote-Proposal Fetch fixture owner");
        assert!(
            fetch_ownership
                .bind_authenticated_remote_proposal_replay_for_test(proposal, &fetch_effect,)
        );
        let fetch_pending = fetch_ownership
            .pending_adapter_effect_binding(&fetch_effect)
            .expect("remote-Proposal Fetch retains one pending binding");
        let fetch_replay = fetch_ownership
            .exact_remote_proposal_fetch_replay(&fetch_effect)
            .expect("authenticated Proposal retains exact Fetch replay evidence");
        let store_effect = AdapterEffect::StoreBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        let store_pending = fetch_pending
            .project_proposal_fetch_store_successor(&fetch_effect, &store_effect)
            .expect("remote-Proposal Fetch projects exact Store binding");
        let store_replay = fetch_replay
            .project_exact_store(&store_effect, &store_pending)
            .expect("remote-Proposal Fetch projects exact Store replay evidence");
        let durable_receipt = validated_receipt.durable().clone();
        let stored_replay = store_replay
            .bind_durable_body(&store_effect, &durable_receipt)
            .expect("remote-Proposal Store binds its exact durable frame");
        let effect = AdapterEffect::ValidateBody {
            tag,
            round: manifest.round,
            subject: manifest.subject,
        };
        let pending = store_pending
            .project_store_validate_successor(&store_effect, &effect)
            .expect("remote-Proposal Store projects exact Validate binding");
        let replay_evidence = stored_replay
            .project_exact_validate(&store_effect, &durable_receipt, &effect, &pending)
            .expect("remote-Proposal Store projects exact Validate replay evidence");
        let replay_evidence = DurableValidateReplayEvidenceV1::remote_proposal(replay_evidence);
        let projected = replay_evidence
            .project_installed_validate_candidate(
                InstalledBodyCandidateProjectionPermit::new(),
                verified,
                &effect,
                &durable_receipt,
                &pending,
            )
            .expect("project genuine remote-Proposal recovered-WAL Validate fixture");
        let coordinator_candidate = projected.clone();
        assert_eq!(projected.work_class, LifecycleWorkClass::Validate);
        assert_eq!(projected.key.phase(), LifecyclePhase::Validate);
        assert_eq!(projected.stage.kind(), LifecycleStageKind::ValidateBody);
        assert_eq!(
            projected.stage.predecessor_scope(),
            PredecessorScope::Independent
        );
        assert_eq!(projected.initial_state, InitialLifecycleState::Ready);
        let (physical_slots, universe, consumed) = projected
            .physical_geometry
            .normalized()
            .expect("normalize recovered-WAL Validate fixture geometry");
        assert_eq!(physical_slots.len(), 1);
        assert_eq!(universe.len(), 1);
        assert_eq!(consumed, universe);
        let (&slot, &incumbent_digest) = physical_slots
            .first_key_value()
            .expect("one recovered-WAL Validate fixture slot");
        let ordinal = 1;
        let owner = OwnerId::new(projected.causal_root, ordinal);
        let address = ConcreteWorkAddress::new(owner, ordinal, slot)
            .expect("exact recovered-WAL Validate fixture address");
        let expected_manifest_hash = durable_receipt.manifest_hash();
        assert_eq!(HashOf::new(&manifest), expected_manifest_hash);
        let incumbent = DurableValidateBody {
            address,
            effect,
            pending,
            durable_receipt,
            expected_manifest_hash,
            replay_evidence,
        };
        assert!(validate_validated_receipt_authority(&incumbent, &validated_receipt).is_ok());
        let outcome = DurableBodyValidationOutcome::validated_for_test(validated_receipt);
        let replacement_digest =
            durable_validate_completion_digest(incumbent_digest, expected_manifest_hash, &outcome)
                .expect("validated recovered-WAL completion has one digest");
        assert_ne!(replacement_digest, incumbent_digest);
        let work = ConcreteLifecycleWork {
            digest: replacement_digest,
            kind: ConcreteLifecycleWorkKind::DurableValidateCompletion(DurableValidateCompletion {
                address,
                incumbent,
                incumbent_digest,
                outcome,
            }),
        };
        self.registry_for_test_mut()
            .install(address, replacement_digest, work)
            .unwrap_or_else(|(error, _work)| {
                panic!("install recovered-WAL Validate fixture: {error:?}")
            });
        let mut ready_slots = physical_slots;
        assert_eq!(
            ready_slots.insert(slot, replacement_digest),
            Some(incumbent_digest)
        );
        let lease = TurnLease {
            id: LeaseId(1),
            ordinal,
            owner,
            key: projected.key,
            work_class: projected.work_class,
            stage: projected.stage,
            rank: super::SchedulerRank::new(3, 0, 0, 0, 0, 0, 0, 0),
            physical_slots: ready_slots,
            output_reservation: None,
        };
        (lease, slot, coordinator_candidate)
    }

    /// Assemble and install a genuine validated completion fixture, then reach
    /// the recovered-WAL cut through the production Ready preparation and
    /// detachment path.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn recovered_wal_validate_registry_cut_for_test<'registry>(
        &'registry mut self,
        verified: &VerifiedHeightContext,
        recovered: &RecoveredWalVoteSign,
        proposal: wire::Proposal,
        manifest: wire::PayloadManifest,
        validated_receipt: ValidatedBodyReceipt,
    ) -> RecoveredWalValidateRegistryCut<'registry> {
        let tag = recovered.tag();
        let vote = recovered.vote();
        assert_eq!(proposal.round, vote.proposal_round);
        assert_eq!(proposal.subject, vote.subject);
        let (lease, slot, _candidate) = self.install_remote_proposal_validate_completion_for_test(
            verified,
            tag,
            proposal,
            manifest,
            validated_receipt,
        );
        let prepared = self
            .registry_for_test_mut()
            .prepare_ready_durable_validate_execution(&lease, slot, verified)
            .expect("prepare installed recovered-WAL Validate completion");
        prepared
            .into_recovered_wal_validate_registry_cut()
            .unwrap_or_else(|_prepared| panic!("validated recovered-WAL completion must detach"))
    }
}

// RECOVERED_WAL_VALIDATE_REGISTRY_JOIN_BEGIN
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
        mut self,
        verified: &VerifiedHeightContext,
        recovered: RecoveredWalVoteSign,
    ) -> Result<
        AuthenticatedRecoveredWalValidateLifecycleRepair<'registry>,
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
                failure: RecoveredWalValidateRegistryJoinFailure::InvalidCarrier {
                    _cut: self,
                    _recovered: recovered,
                },
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
                    failure: RecoveredWalValidateRegistryJoinFailure::Projection {
                        _cut: self,
                        _recovered: recovered,
                    },
                });
            }
        };
        let DetachedValidateReplayEvidenceV1::Retained(replay_evidence) =
            &completion.replay_evidence
        else {
            unreachable!("a live detached Validate completion retains its replay origin")
        };
        match authenticate_recovered_wal_vote_lifecycle_from_durable_body(
            verified,
            &completion.durable_receipt,
            replay_evidence,
            successor,
        ) {
            Ok(repair) => {
                let registry = self.registry.take();
                let registry =
                    registry.expect("recovered WAL join retains its exclusive registry borrow");
                Ok(AuthenticatedRecoveredWalValidateLifecycleRepair {
                    repair,
                    validation: completion,
                    reservation: RecoveredWalValidateRegistryReservation {
                        registry,
                        parent_address: self.address,
                        child: None,
                    },
                })
            }
            Err(error) => Err(RecoveredWalValidateRegistryJoinError {
                failure: RecoveredWalValidateRegistryJoinFailure::Lifecycle {
                    _cut: self,
                    _error: error,
                    _completion: completion,
                },
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
