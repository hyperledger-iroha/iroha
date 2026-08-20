impl V2EffectExecutor<SerializedV2Runtime> {
    /// Return whether a signed/diagnostic output is parked at the lifecycle cut.
    pub(in crate::sumeragi) fn has_pending_lifecycle_output_admissions(&self) -> bool {
        !self.pending_lifecycle_output_admissions.is_empty()
    }

    /// Execute the exact output service callback after lifecycle ordering grants the row.
    fn execute_lifecycle_output_service<S: V2EffectServices>(
        &mut self,
        effect: &AdapterEffect,
        ownership: &RuntimeEffectOwnership,
        services: &mut S,
    ) -> Result<(), EffectExecutorError> {
        match effect {
            AdapterEffect::Broadcast(message) => {
                message
                    .validate_version()
                    .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
                let proposal_round = match &message.payload {
                    wire::ConsensusMessageV2Payload::Proposal(proposal) => {
                        if proposal.round.context_id != self.context.id()
                            || proposal.round.height != self.context.height
                        {
                            return Err(EffectExecutorError::Contract(
                                "outbound Proposal changed the frozen height context".to_owned(),
                            ));
                        }
                        Some(proposal.round)
                    }
                    _ => None,
                };
                let disposition = services
                    .broadcast_consensus(message.clone())
                    .map_err(service_error)?;
                if let Some(proposal_round) = proposal_round
                    && disposition == ConsensusBroadcastDisposition::ExactServiceAccepted
                {
                    self.runtime
                        .complete_active_view_producer_after_proposal_fanout(
                            proposal_round,
                            ownership,
                        )
                        .map_err(EffectExecutorError::Runtime)?;
                }
                Ok(())
            }
            AdapterEffect::ReportEquivocation { evidence } => {
                evidence
                    .validate_structure(&self.context)
                    .map_err(|reason| {
                        EffectExecutorError::Contract(format!(
                            "ReportEquivocation carried invalid evidence: {reason}"
                        ))
                    })?;
                services
                    .report_equivocation(evidence.to_wire())
                    .map_err(service_error)
            }
            AdapterEffect::ReportInvalidCertifiedBody {
                subject,
                certificate,
            } => services
                .report_invalid_certified_body(*subject, certificate.clone())
                .map_err(service_error),
            AdapterEffect::Sign { .. }
            | AdapterEffect::FetchBody { .. }
            | AdapterEffect::StoreBody { .. }
            | AdapterEffect::ValidateBody { .. }
            | AdapterEffect::Apply { .. }
            | AdapterEffect::EnterView { .. } => Err(EffectExecutorError::Contract(
                "non-output effect crossed the lifecycle output settlement seam".to_owned(),
            )),
        }
    }
    /// Settle each initially parked lifecycle output once in binding-key order.
    pub(in crate::sumeragi) fn settle_pending_lifecycle_output_admissions<S: V2EffectServices>(
        &mut self,
        owner: &mut ProductionLifecycleOwnerV1,
        services: &mut S,
    ) -> Result<usize, EffectExecutorError> {
        self.ensure_open()?;
        let keys = self
            .pending_lifecycle_output_admissions
            .keys()
            .copied()
            .collect::<Vec<_>>();
        let mut completed = 0usize;
        for key in keys {
            let Some(pending) = self.pending_lifecycle_output_admissions.remove(&key) else {
                continue;
            };
            let settlement =
                owner.settle_lifecycle_output_admission(pending, |effect, ownership| {
                    self.execute_lifecycle_output_service(effect, ownership, services)
                });
            match settlement {
                ProductionLifecycleOutputAdmissionSettlementV1::Completed
                | ProductionLifecycleOutputAdmissionSettlementV1::AlreadyCompleted => {
                    completed = completed.saturating_add(1);
                }
                ProductionLifecycleOutputAdmissionSettlementV1::Deferred(pending) => {
                    let previous = self
                        .pending_lifecycle_output_admissions
                        .insert(key, pending);
                    debug_assert!(previous.is_none());
                }
                ProductionLifecycleOutputAdmissionSettlementV1::Failed { failure, pending } => {
                    let previous = self
                        .pending_lifecycle_output_admissions
                        .insert(key, pending);
                    debug_assert!(previous.is_none());
                    let error = match failure {
                        ProductionLifecycleOutputAdmissionFailureV1::Service(error) => error,
                        ProductionLifecycleOutputAdmissionFailureV1::Projection(error) => {
                            EffectExecutorError::Contract(format!(
                                "lifecycle output admission projection failed: {error:?}"
                            ))
                        }
                        ProductionLifecycleOutputAdmissionFailureV1::Registry => {
                            EffectExecutorError::Contract(
                                "lifecycle output registry settlement failed".to_owned(),
                            )
                        }
                        ProductionLifecycleOutputAdmissionFailureV1::Durability => {
                            EffectExecutorError::Contract(
                                "lifecycle output terminal publication failed".to_owned(),
                            )
                        }
                    };
                    return Err(self.close(error, services));
                }
            }
        }
        Ok(completed)
    }

    /// Return whether an exact durable Validate owner is parked at lifecycle admission.
    pub(in crate::sumeragi) fn has_pending_durable_validate_admissions(&self) -> bool {
        !self.pending_durable_validate_admissions.is_empty()
    }

    /// Return whether one exact post-fsync live-WAL Sign is parked at lifecycle admission.
    pub(in crate::sumeragi) fn has_pending_live_wal_sign_admission(&self) -> bool {
        self.pending_live_wal_sign_admission.is_some()
    }

    /// Settle the exact post-fsync live-WAL Sign before generic signed-output dispatch.
    pub(in crate::sumeragi) fn settle_pending_live_wal_sign_admission<S: V2EffectServices>(
        &mut self,
        owner: &mut ProductionLifecycleOwnerV1,
        services: &mut S,
    ) -> Result<usize, EffectExecutorError> {
        self.ensure_open()?;
        let Some(pending) = self.pending_live_wal_sign_admission.take() else {
            return Ok(0);
        };
        match owner.settle_live_wal_sign_admission(pending) {
            ProductionLiveWalSignAdmissionSettlementV1::Admitted(AdmissionDecision::Admitted {
                ..
            })
            | ProductionLiveWalSignAdmissionSettlementV1::Rebound(AdmissionDecision::Retry {
                ..
            }) => Ok(1),
            ProductionLiveWalSignAdmissionSettlementV1::Admitted(decision)
            | ProductionLiveWalSignAdmissionSettlementV1::Rebound(decision) => Err(self.close(
                EffectExecutorError::Contract(format!(
                    "live WAL Sign settlement committed an invalid logical decision: {decision:?}"
                )),
                services,
            )),
            ProductionLiveWalSignAdmissionSettlementV1::Returned {
                decision: AdmissionDecision::WaitForCapacity(_),
                pending,
            } => {
                self.pending_live_wal_sign_admission = Some(pending);
                Ok(0)
            }
            ProductionLiveWalSignAdmissionSettlementV1::Returned {
                decision:
                    AdmissionDecision::Retry { .. }
                    | AdmissionDecision::ReplayTerminal { .. }
                    | AdmissionDecision::StutterTerminal { .. },
                pending: _,
            } => Ok(0),
            ProductionLiveWalSignAdmissionSettlementV1::Returned { decision, pending } => {
                self.pending_live_wal_sign_admission = Some(pending);
                Err(self.close(
                    EffectExecutorError::Contract(format!(
                        "live WAL Sign admission returned a terminally invalid decision: {decision:?}"
                    )),
                    services,
                ))
            }
            ProductionLiveWalSignAdmissionSettlementV1::Failed { failure, pending } => {
                self.pending_live_wal_sign_admission = Some(pending);
                let error = match failure {
                    ProductionLiveWalSignAdmissionFailureV1::Projection(error) => {
                        EffectExecutorError::Contract(format!(
                            "live WAL Sign admission projection failed: {error:?}"
                        ))
                    }
                    ProductionLiveWalSignAdmissionFailureV1::Registry => {
                        EffectExecutorError::Contract(
                            "live WAL Sign registry settlement failed".to_owned(),
                        )
                    }
                    ProductionLiveWalSignAdmissionFailureV1::Durability => {
                        EffectExecutorError::Contract(
                            "live WAL Sign admission publication failed".to_owned(),
                        )
                    }
                };
                Err(self.close(error, services))
            }
        }
    }

    /// Settle each currently pending durable Validate owner once, in body-key order.
    ///
    /// Capacity waits restore the exact move-only owner. An exact logical
    /// `Retry` consumes the duplicate because the incumbent ordinal and
    /// registry carrier already own execution. Any other non-committing
    /// decision is a production invariant violation and closes the shared
    /// output gate while preserving the pending owner for restart diagnosis.
    pub(in crate::sumeragi) fn settle_pending_durable_validate_admissions<S: V2EffectServices>(
        &mut self,
        owner: &mut ProductionLifecycleOwnerV1,
        services: &mut S,
    ) -> Result<usize, EffectExecutorError> {
        self.ensure_open()?;
        let pending_keys = self
            .pending_durable_validate_admissions
            .keys()
            .copied()
            .collect::<Vec<_>>();
        let mut made_ready = 0usize;
        for key in pending_keys {
            let Some(pending) = self.pending_durable_validate_admissions.remove(&key) else {
                continue;
            };
            match owner.settle_durable_validate_admission(pending) {
                ProductionDurableValidateAdmissionSettlementV1::Admitted(
                    AdmissionDecision::Admitted { .. },
                )
                | ProductionDurableValidateAdmissionSettlementV1::Rebound(
                    AdmissionDecision::Retry { .. },
                ) => {
                    made_ready = made_ready.saturating_add(1);
                }
                ProductionDurableValidateAdmissionSettlementV1::Admitted(decision)
                | ProductionDurableValidateAdmissionSettlementV1::Rebound(decision) => {
                    return Err(self.close(
                        EffectExecutorError::Contract(format!(
                            "durable Validate settlement committed an invalid logical decision: {decision:?}"
                        )),
                        services,
                    ));
                }
                ProductionDurableValidateAdmissionSettlementV1::Returned {
                    decision: AdmissionDecision::WaitForCapacity(_),
                    pending,
                } => {
                    let previous = self
                        .pending_durable_validate_admissions
                        .insert(key, pending);
                    debug_assert!(previous.is_none());
                }
                ProductionDurableValidateAdmissionSettlementV1::Returned {
                    decision:
                        AdmissionDecision::Retry { .. }
                        | AdmissionDecision::ReplayTerminal { .. }
                        | AdmissionDecision::StutterTerminal { .. },
                    pending: _,
                } => {}
                ProductionDurableValidateAdmissionSettlementV1::Returned { decision, pending } => {
                    let previous = self
                        .pending_durable_validate_admissions
                        .insert(key, pending);
                    debug_assert!(previous.is_none());
                    return Err(self.close(
                        EffectExecutorError::Contract(format!(
                            "durable Validate admission returned a terminally invalid decision: {decision:?}"
                        )),
                        services,
                    ));
                }
                ProductionDurableValidateAdmissionSettlementV1::Failed { failure, pending } => {
                    let previous = self
                        .pending_durable_validate_admissions
                        .insert(key, pending);
                    debug_assert!(previous.is_none());
                    return Err(self.close(
                        EffectExecutorError::Contract(format!(
                            "durable Validate admission failed before commit: {failure:?}"
                        )),
                        services,
                    ));
                }
            }
        }
        Ok(made_ready)
    }
}
