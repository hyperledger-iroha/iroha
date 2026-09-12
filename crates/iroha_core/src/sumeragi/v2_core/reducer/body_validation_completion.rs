// Exact available, durable and validated body-completion transitions.
impl Reducer {
    fn on_body_available(&mut self, round: Round, subject: Subject) -> StepOutcome {
        let Some(work) = self.body_work.get_mut(&(round, subject)) else {
            return StepOutcome::ignored(IgnoreReason::NoMatchingWork);
        };
        if work.state != BodyState::Missing {
            return StepOutcome::ignored(IgnoreReason::Duplicate);
        }
        work.state = BodyState::Available;
        StepOutcome::applied(vec![Effect::StoreBody {
            tag: self.current_tag(),
            round,
            subject,
        }])
    }
    fn on_body_stored(&mut self, round: Round, subject: Subject) -> StepOutcome {
        let Some(work) = self.body_work.get_mut(&(round, subject)) else {
            return StepOutcome::ignored(IgnoreReason::NoMatchingWork);
        };
        if work.state != BodyState::Available {
            return StepOutcome::ignored(IgnoreReason::Duplicate);
        }
        work.state = BodyState::Durable;
        StepOutcome::applied(vec![Effect::ValidateBody {
            tag: self.current_tag(),
            round,
            subject,
        }])
    }
    fn on_validation(
        &mut self,
        round: Round,
        subject: Subject,
        valid: bool,
    ) -> Result<StepOutcome, ReducerError> {
        let Some(work) = self.body_work.get_mut(&(round, subject)) else {
            return Ok(StepOutcome::ignored(IgnoreReason::NoMatchingWork));
        };
        if work.state != BodyState::Durable {
            return Ok(StepOutcome::ignored(IgnoreReason::Duplicate));
        }
        work.state = if valid {
            BodyState::Validated
        } else {
            BodyState::Invalid
        };
        if !valid {
            let key = CertificateRef::new(self.context.id(), round, Phase::Prepare, subject);
            // A timeout retires current-vote preparation while retaining its
            // exact durable lock. That certificate still authenticates a
            // rejection report for this same body occurrence after replay.
            let effects = self
                .pending_prepare
                .get(&key)
                .or_else(|| {
                    self.durable
                        .locked()
                        .filter(|certificate| certificate.reference() == key)
                })
                .map_or_else(Vec::new, |certificate| {
                    vec![Effect::ReportInvalidCertifiedBody {
                        subject,
                        certificate: certificate.clone(),
                    }]
                });
            return Ok(StepOutcome::applied(effects));
        }
        if let Some(decision) = self.durable.decision().cloned() {
            if self.decision_body_round(&decision) == round && decision.subject() == subject {
                return Ok(StepOutcome::applied(vec![Effect::Apply {
                    tag: self.current_tag(),
                    subject,
                    certificate: decision,
                }]));
            }
            return Ok(StepOutcome::ignored(IgnoreReason::AlreadyDecided));
        }
        if round.view() != self.durable.current_view() {
            return Ok(StepOutcome::ignored(IgnoreReason::IrrelevantView));
        }
        if self.local_validator.is_none() {
            return Ok(StepOutcome::ignored(IgnoreReason::Observer));
        }
        if self.durable.timeout_intent(round).is_some() {
            return Ok(StepOutcome::ignored(IgnoreReason::ViewClosed));
        }
        let key = CertificateRef::new(self.context.id(), round, Phase::Prepare, subject);
        if let Some(prepare) = self.pending_prepare.get(&key).cloned() {
            return self.persist_commit_intent(prepare);
        }
        if self.candidate.as_ref().is_some_and(|proposal| {
            proposal.round() == round && proposal.manifest().subject() == subject
        }) {
            return self.persist_prepare_intent(round, subject);
        }
        Ok(StepOutcome::applied(Vec::new()))
    }
}
