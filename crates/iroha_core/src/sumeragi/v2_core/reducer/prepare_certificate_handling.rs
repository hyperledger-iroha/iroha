impl Reducer {
    fn on_certificate(
        &mut self,
        certificate: QuorumCertificate,
        formed_locally: bool,
    ) -> Result<StepOutcome, ReducerError> {
        certificate.validate(&self.context)?;
        match certificate.phase() {
            Phase::Prepare => self.on_prepare_certificate(&certificate, formed_locally),
            Phase::Commit => self.on_commit_certificate(certificate, formed_locally),
        }
    }
    fn on_prepare_certificate(
        &mut self,
        certificate: &QuorumCertificate,
        formed_locally: bool,
    ) -> Result<StepOutcome, ReducerError> {
        if self.durable.decision().is_some() {
            return Ok(StepOutcome::ignored(IgnoreReason::AlreadyDecided));
        }
        if certificate.round().view() > self.durable.current_view() {
            return Ok(StepOutcome::ignored(IgnoreReason::IrrelevantView));
        }
        if let Some(existing) = self.durable.highest_prepare() {
            if existing.round().view() == certificate.round().view()
                && existing.subject() != certificate.subject()
            {
                return Err(ReducerError::ConflictingPrepareCertificates);
            }
            if certificate.round().view() < existing.round().view() {
                return Ok(StepOutcome::ignored(IgnoreReason::IrrelevantView));
            }
        }
        let reference = certificate.reference();
        let certificate = self
            .pending_prepare
            .entry(reference)
            .or_insert_with(|| certificate.clone())
            .clone();
        self.known_prepare
            .entry(reference)
            .or_insert_with(|| certificate.clone());
        self.remember_control(ConsensusMessageV2::QuorumCertificate(certificate.clone()));
        let mut effects = Vec::new();
        if formed_locally {
            let message = ConsensusMessageV2::QuorumCertificate(certificate.clone());
            self.remember_control(message.clone());
            effects.push(Effect::Broadcast(message));
        }
        let current = certificate.round().view() == self.durable.current_view();
        let validated =
            self.body_state(certificate.round(), certificate.subject()) == BodyState::Validated;
        let view_closed = self.durable.timeout_intent(certificate.round()).is_some();
        if current
            && validated
            && !view_closed
            && self.local_validator.is_some()
            && self.local_candidate_body_eligible()
        {
            let mut outcome = self.persist_commit_intent(certificate)?;
            effects.append(&mut outcome.effects);
            return Ok(StepOutcome::applied(effects));
        }
        if current
            && !view_closed
            && self.body_state(certificate.round(), certificate.subject()) == BodyState::Missing
            && self.local_certified_candidate_body_eligible()
        {
            effects.push(self.ensure_body_fetch(&certificate));
        }
        if self
            .durable
            .highest_prepare()
            .is_none_or(|existing| certificate.round().view() > existing.round().view())
        {
            let persist =
                self.start_persistence(WalRecord::ObservePrepare(certificate), Continuation::None)?;
            effects.push(persist);
        }
        Ok(StepOutcome::applied(effects))
    }
    fn prune_observed_prepare_caches(&mut self) {
        let current_view = self.durable.current_view();
        self.pending_prepare
            .retain(|_, certificate| certificate.round().view() == current_view);
        self.known_prepare = self
            .durable
            .highest_prepare()
            .into_iter()
            .chain(self.durable.locked())
            .chain(self.pending_prepare.values())
            .map(|certificate| (certificate.reference(), certificate.clone()))
            .collect();
    }
}
