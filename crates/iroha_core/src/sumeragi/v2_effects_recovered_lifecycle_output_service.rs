impl V2EffectExecutor<SerializedV2Runtime> {
    /// Snapshot inert runtime ingress around lifecycle completion tests.
    #[cfg(test)]
    pub(in crate::sumeragi) fn runtime_queue_snapshot_for_test(
        &self,
        now: Instant,
    ) -> RuntimeQueueSnapshot {
        self.runtime.queue_snapshot(now)
    }

    /// Execute one authenticated cold-open output without inventing runtime ownership.
    ///
    /// Proposal fanout is excluded because its active-view producer owner is
    /// process-local. Every accepted effect was re-authenticated from LedgerV1
    /// and remains owned by the lifecycle cold-recovery cut until that same row
    /// is durably terminalized.
    pub(in crate::sumeragi) fn execute_recovered_lifecycle_output_service<S: V2EffectServices>(
        &mut self,
        effect: &AdapterEffect,
        services: &mut S,
    ) -> Result<LifecycleOutputServiceDispositionV1, EffectExecutorError> {
        self.ensure_open()?;
        let result = (|| -> Result<LifecycleOutputServiceDispositionV1, EffectExecutorError> {
            match effect {
                AdapterEffect::Broadcast(message) => {
                    if matches!(
                        &message.payload,
                        wire::ConsensusMessageV2Payload::Proposal(_)
                    ) {
                        Err(EffectExecutorError::Contract(
                            "cold lifecycle output cannot synthesize Proposal producer ownership"
                                .to_owned(),
                        ))
                    } else {
                        message
                            .validate_version()
                            .map_err(|error| EffectExecutorError::Contract(error.to_string()))?;
                        services
                            .broadcast_consensus(message.clone())
                            .map(|disposition| match disposition {
                                ConsensusBroadcastDisposition::ExactServiceAccepted => {
                                    LifecycleOutputServiceDispositionV1::Accepted
                                }
                                ConsensusBroadcastDisposition::SourceRetained => {
                                    LifecycleOutputServiceDispositionV1::SourceRetained
                                }
                            })
                            .map_err(service_error)
                    }
                }
                AdapterEffect::ReportEquivocation { evidence } => {
                    evidence
                        .validate_structure(&self.context)
                        .map_err(|reason| {
                            EffectExecutorError::Contract(format!(
                                "recovered ReportEquivocation carried invalid evidence: {reason}"
                            ))
                        })?;
                    services
                        .report_equivocation(evidence.to_wire())
                        .map_err(service_error)?;
                    Ok(LifecycleOutputServiceDispositionV1::Accepted)
                }
                AdapterEffect::ReportInvalidCertifiedBody {
                    subject,
                    certificate,
                } => {
                    services
                        .report_invalid_certified_body(*subject, certificate.clone())
                        .map_err(service_error)?;
                    Ok(LifecycleOutputServiceDispositionV1::Accepted)
                }
                AdapterEffect::Sign { .. }
                | AdapterEffect::FetchBody { .. }
                | AdapterEffect::StoreBody { .. }
                | AdapterEffect::ValidateBody { .. }
                | AdapterEffect::Apply { .. }
                | AdapterEffect::EnterView { .. } => Err(EffectExecutorError::Contract(
                    "non-output effect crossed the recovered lifecycle output seam".to_owned(),
                )),
            }
        })();
        result.map_err(|error| self.close(error, services))
    }
}
