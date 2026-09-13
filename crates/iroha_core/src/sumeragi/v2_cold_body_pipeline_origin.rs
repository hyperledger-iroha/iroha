// Reconstruct retained body custody before replaying its durable completions.
impl SumeragiV2Adapter {
    fn restore_authenticated_cold_body_origin(
        &mut self,
        origin: &AuthenticatedBodyPipelineColdReplayOriginV1,
    ) -> Result<(), &'static str> {
        self.ensure_ingress()
            .map_err(|_| "cold body origin adapter is fenced")?;
        let tag = origin.tag();
        let manifest = origin.manifest();
        if tag != self.current_tag() || manifest.validate(&self.wire_context).is_err() {
            return Err("cold body origin changed its current tag or manifest");
        }
        let mut registry = self.registry.clone();
        let round = registry
            .round_to_core(manifest.round, &self.wire_context)
            .map_err(|_| "cold body origin changed its round")?;
        let core_manifest = registry
            .manifest_to_core(manifest, &self.wire_context)
            .map_err(|_| "cold body origin conflicts with the registered manifest")?;
        let mut next = self.reducer.clone();
        let mut active_subject = self.active_subject;
        if let Some(certificate) = origin.certificate() {
            if certificate.proposal_round != manifest.round
                || certificate.subject != manifest.subject
                || verify_quorum_certificate(
                    &self.wire_context,
                    certificate,
                    &self.proofs_of_possession,
                )
                .is_err()
            {
                return Err("cold body origin changed its authenticated certificate");
            }
            // Register the already authenticated execution commitment without
            // replaying a QC event that could start another WAL transaction.
            registry
                .qc_to_core(certificate, &self.wire_context)
                .map_err(|_| "cold body origin certificate conflicts with the registry")?;
        }
        if let Some(proposal) = origin.proposal() {
            let message = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
                proposal.clone(),
            ));
            if &proposal.manifest != manifest
                || verify_authenticated_message(
                    &self.wire_context,
                    self.parent_verification.as_ref(),
                    &message,
                    &self.proofs_of_possession,
                )
                .is_err()
            {
                return Err("cold body origin changed its authenticated Proposal");
            }
            let proposal = registry
                .proposal_to_core(proposal, &self.wire_context)
                .map_err(|_| "cold body origin Proposal conversion failed")?;
            let result = next
                .step(reducer::Event::ProposalReceived { tag, proposal })
                .map_err(|_| "cold body origin Proposal admission failed")?;
            let disposition = result.disposition();
            let effects = result.into_effects();
            // The ledger already owns this exact body occurrence. An existing
            // durable PrepareQC may upgrade its Fetch sources during admission.
            // Neither form may create another physical acquisition on cold open.
            let exact_fetch = matches!(effects.as_slice(), [reducer::Effect::FetchBody {
                tag: fetch_tag, round: fetch_round, subject, manifest: Some(fetch_manifest),
                certificate, ..
            }] if *fetch_tag == tag && *fetch_round == round
                && *subject == core_manifest.subject() && *fetch_manifest == core_manifest
                && certificate.as_ref().is_none_or(|qc|
                    qc.proposal_round() == round && qc.subject() == *subject));
            match disposition {
                reducer::StepDisposition::Applied if effects.is_empty() || exact_fetch => {
                    active_subject = Some((round, core_manifest.subject()));
                }
                reducer::StepDisposition::Ignored(
                    reducer::IgnoreReason::Duplicate
                    | reducer::IgnoreReason::IrrelevantView
                    | reducer::IgnoreReason::ViewClosed
                    | reducer::IgnoreReason::UnsafeProposal
                    | reducer::IgnoreReason::AlreadyDecided,
                ) if effects.is_empty() => {}
                _ => return Err("cold body origin Proposal emitted foreign work"),
            }
        }
        next.restore_retained_body_pipeline(origin, tag, round, core_manifest)
            .map_err(|_| "cold body origin custody conflicts with the recovered reducer")?;
        self.registry = registry;
        self.reducer = next;
        self.active_subject = active_subject;
        Ok(())
    }
}

#[cfg(test)]
impl SumeragiV2Adapter {
    pub(in crate::sumeragi) fn preview_cold_body_validation_wal_for_test(
        &mut self,
        tag: reducer::EventTag,
        round: wire::ConsensusRound,
        subject: wire::BlockSubject,
        receipt: &ValidatedBodyReceipt,
    ) -> Result<Option<reducer::WalRecord>, AdapterError> {
        match self.prepare_direct_validation_succeeded(tag, round, subject, receipt)? {
            DirectValidationSucceededPreparation::Persist(prepared) => {
                let reducer::Effect::Persist { entry, .. } = &prepared.persist_effect else {
                    unreachable!("direct Persist preview has one Persist effect")
                };
                Ok(Some(entry.record().clone()))
            }
            DirectValidationSucceededPreparation::NoEffect(_) => Ok(None),
            _ => Err(AdapterError::DirectValidationSucceededContractViolation),
        }
    }
}
