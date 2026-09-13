// Restore only body custody after the production boundary authenticates its
// retained lifecycle source, exact body frame, and live owner. The parent module
// exposes the sealed adapter entry point; the dependency-free reducer primitive
// cannot be called directly from lifecycle or runtime code.
impl Reducer {
    pub(super) fn restore_retained_body_pipeline_custody(
        &mut self,
        tag: EventTag,
        round: Round,
        manifest: PayloadManifest,
        locally_available: bool,
    ) -> Result<(), ReducerError> {
        if tag != self.current_tag()
            || round.height() != self.context.height()
            || round.view() > self.durable.current_view()
        {
            return Err(ReducerError::InvalidRetainedBodyPipeline);
        }
        if !self.replay_resumed
            || self.pending_persistence.is_some()
            || self.awaiting_signature.is_some()
            || !self.signature_queue.is_empty()
        {
            return Err(ReducerError::HeightStillBusy);
        }
        let subject = manifest.subject();
        if self.durable.decision().is_some_and(|decision| {
            self.decision_body_round(decision) != round || decision.subject() != subject
        }) {
            return Err(ReducerError::InvalidRetainedBodyPipeline);
        }
        let key = (round, subject);
        if self.body_work.get(&key).is_some_and(|work| {
            work.state == BodyState::Invalid
                || work
                    .manifest
                    .as_ref()
                    .is_some_and(|existing| existing != &manifest)
        }) {
            return Err(ReducerError::InvalidRetainedBodyPipeline);
        }
        let work = self.body_work.entry(key).or_insert(BodyWork {
            manifest: None,
            state: BodyState::Missing,
        });
        work.manifest = Some(manifest);
        if locally_available && work.state == BodyState::Missing {
            work.state = BodyState::Available;
        }
        // In particular, do not restore candidate, pending_prepare, a local
        // intent, or an outbound message from custody. Signed current proposals
        // enter through the normal proposal transition; obsolete origins retain
        // storage/validation work without acquiring current voting authority.
        Ok(())
    }
}
