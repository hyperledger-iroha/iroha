// Resumable publication engine, progress validation, and public error surface.

/// Resumable publication coordinator.
#[derive(Debug)]
pub struct PublicationEngine<'a> {
    store: &'a PublicationJournalStore,
}

impl<'a> PublicationEngine<'a> {
    /// Bind an engine to a durable user-level journal store.
    #[must_use]
    pub const fn new(store: &'a PublicationJournalStore) -> Self {
        Self { store }
    }

    /// Persist a detached operation and return its resumable identifier.
    ///
    /// # Errors
    ///
    /// Returns a publication error when request validation, operation locking, or initial journal
    /// persistence fails.
    pub fn begin_detached(
        &self,
        request: PublicationRequestV1,
    ) -> Result<PublicationOperationIdV1, PublicationError> {
        self.store
            .create(request)
            .map(|journal| journal.operation_id)
    }

    /// Repair the immutable CAR and plan for one pristine pre-ingress journal.
    ///
    /// The caller may rebuild the clean package outside the operation lock, but must supply the
    /// exact journal image it used. This method then compares that complete image under the
    /// per-operation lock, proves that the rebuilt publication and archive commitment equal the
    /// immutable request, and installs only absent or byte-identical sidecars. The journal is not
    /// advanced or rewritten.
    ///
    /// # Errors
    ///
    /// Returns a publication error when the journal is not the initial validation revision, a
    /// concurrent transition changed it, the rebuilt content differs from the immutable request,
    /// or either exact sidecar cannot be verified and durably installed.
    pub(crate) fn recover_pre_ingress_sidecars(
        &self,
        expected: &PublicationJournalV1,
        rebuilt_publication: &MusubiPublicationV1,
        rebuilt_commitment: &MusubiArchiveCommitmentV1,
        plan: &CarBuildPlan,
        car: &[u8],
    ) -> Result<PublicationStagedCarSourceV1, PublicationError> {
        expected.validate()?;
        if expected.phase != PublicationPhaseV1::Validation || expected.revision != 1 {
            return Err(PublicationError::InvalidJournal(
                "pre-ingress sidecar recovery requires the pristine validation revision".to_owned(),
            ));
        }
        if rebuilt_publication != &expected.request.publication
            || rebuilt_commitment != &expected.request.archive_commitment
        {
            return Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Validation,
                reason: "rebuilt publication content differs from the immutable recovery request"
                    .to_owned(),
            });
        }

        let operation_id = expected.operation_id;
        let operation_lock = self.store.lock_operation(operation_id)?;
        let result = (|| {
            let current = self.store.load(operation_id)?;
            if current != *expected {
                return Err(PublicationError::ConcurrentJournalUpdate);
            }
            if current.phase != PublicationPhaseV1::Validation || current.revision != 1 {
                return Err(PublicationError::InvalidJournal(
                    "pre-ingress sidecar recovery requires the pristine validation revision"
                        .to_owned(),
                ));
            }
            if rebuilt_publication != &current.request.publication
                || rebuilt_commitment != &current.request.archive_commitment
            {
                return Err(PublicationError::InvalidEvidence {
                    phase: PublicationPhaseV1::Validation,
                    reason:
                        "rebuilt publication content differs from the immutable recovery request"
                            .to_owned(),
                });
            }

            operation_lock.validate()?;
            let source = PublicationStagedCarSourceV1::stage_bytes(
                self.store.root.path(),
                operation_id,
                rebuilt_commitment,
                plan,
                car,
            )?;
            operation_lock.validate()?;
            if self.store.load(operation_id)? != current {
                return Err(PublicationError::ConcurrentJournalUpdate);
            }
            Ok(source)
        })();
        operation_lock.finish(result)
    }

    /// Persist a secret-free operation journal before installing its immutable CAR and plan.
    ///
    /// This ordering leaves a small authoritative recovery anchor if power is lost or local
    /// storage fails during either sidecar install; it never leaves an unindexed CAR behind. An
    /// identical call against the pristine journal reuses both sidecars idempotently.
    ///
    /// # Errors
    ///
    /// Returns a publication error when the request cannot be journaled first, or when its exact
    /// canonical CAR/plan pair cannot then be verified and durably installed.
    pub fn begin_detached_with_car(
        &self,
        request: PublicationRequestV1,
        plan: &CarBuildPlan,
        car: &[u8],
    ) -> Result<(PublicationOperationIdV1, PublicationStagedCarSourceV1), PublicationError> {
        let operation_id = request.operation_id();
        let publication = request.publication.clone();
        let commitment = request.archive_commitment.clone();
        let journal = self.store.create(request)?;
        if journal.operation_id != operation_id {
            return Err(PublicationError::InvalidJournal(
                "persisted publication operation identity changed".to_owned(),
            ));
        }
        let source =
            self.recover_pre_ingress_sidecars(&journal, &publication, &commitment, plan, car)?;
        Ok((operation_id, source))
    }

    /// Start or idempotently recover an operation, running until finality or a pending poll.
    ///
    /// # Errors
    ///
    /// Returns a publication error when journal recovery, CAR access, backend execution, evidence
    /// validation, or a durable transition fails.
    pub fn publish(
        &self,
        request: PublicationRequestV1,
        source: &dyn PublicationCarSource,
        backend: &mut dyn PublicationBackend,
    ) -> Result<PublicationAdvanceV1, PublicationError> {
        let journal = self.store.create(request)?;
        self.run(journal.operation_id, source, backend)
    }

    /// Resume an operation by id and run until finality or a pending poll.
    ///
    /// # Errors
    ///
    /// Returns a publication error when the journal cannot be loaded or a subsequent backend,
    /// validation, or durable transition fails.
    pub fn resume(
        &self,
        operation_id: PublicationOperationIdV1,
        source: &dyn PublicationCarSource,
        backend: &mut dyn PublicationBackend,
    ) -> Result<PublicationAdvanceV1, PublicationError> {
        self.run(operation_id, source, backend)
    }

    /// Advance exactly one durable phase, making retries observable to callers.
    ///
    /// # Errors
    ///
    /// Returns a publication error when the current journal, CAR, backend evidence, append-only
    /// transition, or persistent state fails validation.
    #[allow(
        clippy::too_many_lines,
        reason = "the publication engine keeps the complete persist-before-send security state machine explicit in one fixed-protocol transition"
    )]
    pub fn advance_once(
        &self,
        operation_id: PublicationOperationIdV1,
        source: &dyn PublicationCarSource,
        backend: &mut dyn PublicationBackend,
    ) -> Result<PublicationAdvanceV1, PublicationError> {
        let journal = self.store.load(operation_id)?;
        if let Some(result) = journal.result() {
            return Ok(PublicationAdvanceV1::Complete(result));
        }
        let phase = journal.phase;
        let mut next = journal.clone();
        match phase {
            PublicationPhaseV1::Validation => {
                let plan = source
                    .car_plan(&journal.request.archive_commitment)
                    .map_err(PublicationError::CarSource)?;
                plan.validate(&journal.request.archive_commitment)
                    .map_err(|error| invalid(PublicationPhaseV1::Validation, error))?;
                let mut car = source.open_car().map_err(PublicationError::CarSource)?;
                let evidence = backend
                    .validate_clean_package(operation_id, &journal.request, car.as_mut())
                    .map_err(PublicationError::Backend)?;
                evidence.validate_for(&journal.request)?;
                next.validation = Some(evidence);
                next.phase = PublicationPhaseV1::SeedIngress;
            }
            PublicationPhaseV1::SeedIngress => {
                if journal.archive_registration_attempts.len()
                    >= MUSUBI_MAX_ARCHIVE_REGISTRATION_ATTEMPTS_V1
                {
                    return Err(PublicationError::Backend(
                        PublicationBackendError::permanent(
                            "ARCHIVE_REGISTRATION_ATTEMPT_LIMIT_REACHED",
                        ),
                    ));
                }
                let expected = journal.request.receipt_binding();
                let plan = source
                    .car_plan(&journal.request.archive_commitment)
                    .map_err(PublicationError::CarSource)?;
                plan.validate(&journal.request.archive_commitment)
                    .map_err(|error| invalid(PublicationPhaseV1::SeedIngress, error))?;
                let mut car = source.open_car().map_err(PublicationError::CarSource)?;
                let receipt = backend
                    .stage_authenticated_seed_ingress(
                        operation_id,
                        &expected,
                        &journal.request.archive_commitment,
                        &plan,
                        car.as_mut(),
                    )
                    .map_err(PublicationError::Backend)?;
                let now = backend
                    .current_time_ms()
                    .map_err(PublicationError::Backend)?;
                verify_seed_ingress_receipt_with_bounded_service_lead(&receipt, &expected, now)?;
                next.staging_receipt = Some(receipt);
                next.phase = PublicationPhaseV1::ArchiveRegistration;
            }
            PublicationPhaseV1::ArchiveRegistration => {
                let receipt = journal.staging_receipt.as_ref().ok_or_else(|| {
                    PublicationError::InvalidJournal("missing staging receipt".to_owned())
                })?;
                let active_attempt = journal
                    .archive_registration_attempts
                    .last()
                    .filter(|attempt| attempt.terminal.is_none());
                if active_attempt.is_none() {
                    let now = backend
                        .current_time_ms()
                        .map_err(PublicationError::Backend)?;
                    if now > receipt.payload.expires_at_ms {
                        next.staging_receipt = None;
                        next.phase = PublicationPhaseV1::SeedIngress;
                    } else {
                        verify_seed_ingress_receipt_with_bounded_service_lead(
                            receipt,
                            &journal.request.receipt_binding(),
                            now,
                        )?;
                        if now < receipt.payload.issued_at_ms {
                            return Ok(PublicationAdvanceV1::Pending(phase));
                        }
                        let intent = backend
                            .prepare_archive_registration_intent(
                                operation_id,
                                &journal.request,
                                receipt,
                            )
                            .map_err(PublicationError::Backend)?;
                        intent.validate_for(operation_id, &journal.request, receipt)?;
                        let generation =
                            u8::try_from(journal.archive_registration_attempts.len() + 1)
                                .expect("archive-registration attempt bound fits u8");
                        next.archive_registration_attempts.push(
                            PublicationArchiveRegistrationAttemptV1::new(generation, intent),
                        );
                    }
                } else if let Some(attempt) = active_attempt
                    && journal.registered_archive.is_none()
                {
                    match backend
                        .submit_or_recover_archive_registration(
                            operation_id,
                            &journal.request,
                            &attempt.intent,
                        )
                        .map_err(PublicationError::Backend)?
                    {
                        PublicationArchiveRegistrationAdvanceV1::Pending => {
                            return Ok(PublicationAdvanceV1::Pending(phase));
                        }
                        PublicationArchiveRegistrationAdvanceV1::Registered(registered) => {
                            registered.validate_for(&journal.request, &attempt.intent)?;
                            next.registered_archive = Some(registered);
                        }
                        PublicationArchiveRegistrationAdvanceV1::TerminalAbsent(terminal) => {
                            terminal.validate_for(&journal.request, &attempt.intent)?;
                            next.archive_registration_attempts
                                .last_mut()
                                .expect("active registration attempt exists")
                                .terminal = Some(terminal);
                            next.staging_receipt = None;
                            next.phase = PublicationPhaseV1::SeedIngress;
                        }
                    }
                } else {
                    let registered = journal
                        .registered_archive
                        .as_ref()
                        .expect("checked authoritative archive");
                    let active_location_attempt = journal
                        .archive_location_attempts
                        .last()
                        .filter(|attempt| attempt.terminal.is_none());
                    if active_location_attempt.is_none() {
                        if journal.archive_location_attempts.len()
                            >= MUSUBI_MAX_ARCHIVE_LOCATION_ATTEMPTS_V1
                        {
                            return Err(PublicationError::Backend(
                                PublicationBackendError::permanent(
                                    "ARCHIVE_LOCATION_ATTEMPT_LIMIT_REACHED",
                                ),
                            ));
                        }
                        let generation = u8::try_from(journal.archive_location_attempts.len() + 1)
                            .expect("archive-location attempt bound fits u8");
                        let prior_location_ids = journal
                            .archive_location_attempts
                            .iter()
                            .map(|attempt| attempt.intent.location_id)
                            .collect::<Vec<_>>();
                        let provider_checkpoint = generation
                            .checked_sub(1)
                            .map(usize::from)
                            .and_then(|index| journal.provider_registration_checkpoints.get(index));
                        match backend
                            .checkpoint_archive_location_provider_registrations(
                                operation_id,
                                &journal.request,
                                registered,
                                generation,
                                &prior_location_ids,
                                provider_checkpoint,
                            )
                            .map_err(PublicationError::Backend)?
                        {
                            PublicationProviderRegistrationCheckpointAdvanceV1::Ready => {
                                let intent = backend
                                    .prepare_archive_location_intent(
                                        operation_id,
                                        &journal.request,
                                        registered,
                                        generation,
                                        &prior_location_ids,
                                    )
                                    .map_err(PublicationError::Backend)?;
                                intent.validate_for(
                                    operation_id,
                                    &journal.request,
                                    registered,
                                    &prior_location_ids,
                                )?;
                                next.archive_location_attempts.push(
                                    PublicationArchiveLocationAttemptV1::new(generation, intent),
                                );
                            }
                            PublicationProviderRegistrationCheckpointAdvanceV1::Updated(
                                checkpoint,
                            ) => {
                                checkpoint.validate_for(&journal.request, generation)?;
                                if provider_checkpoint == Some(&checkpoint) {
                                    return Err(PublicationError::InvalidEvidence {
                                        phase,
                                        reason: "provider-registration checkpoint update made no append-only progress"
                                            .to_owned(),
                                    });
                                }
                                if let Some(existing) = next
                                    .provider_registration_checkpoints
                                    .get_mut(usize::from(generation - 1))
                                {
                                    *existing = checkpoint;
                                } else if next.provider_registration_checkpoints.len()
                                    == usize::from(generation - 1)
                                {
                                    next.provider_registration_checkpoints.push(checkpoint);
                                } else {
                                    return Err(PublicationError::InvalidJournal(
                                        "provider-registration checkpoint generations are not contiguous"
                                            .to_owned(),
                                    ));
                                }
                            }
                        }
                    } else if let Some(attempt) = active_location_attempt {
                        if attempt.registration.is_some() {
                            next.phase = PublicationPhaseV1::Replication;
                        } else {
                            let prior_location_ids = journal.archive_location_attempts
                                [..journal.archive_location_attempts.len() - 1]
                                .iter()
                                .map(|prior| prior.intent.location_id)
                                .collect::<Vec<_>>();
                            match backend
                                .submit_or_recover_archive_location(
                                    operation_id,
                                    &journal.request,
                                    registered,
                                    &attempt.intent,
                                    &prior_location_ids,
                                )
                                .map_err(PublicationError::Backend)?
                            {
                                PublicationArchiveLocationAdvanceV1::Pending => {
                                    return Ok(PublicationAdvanceV1::Pending(phase));
                                }
                                PublicationArchiveLocationAdvanceV1::Registered(registration) => {
                                    registration.validate_for(
                                        operation_id,
                                        &journal.request,
                                        registered,
                                        &prior_location_ids,
                                    )?;
                                    if registration.intent != attempt.intent {
                                        return Err(PublicationError::InvalidEvidence {
                                            phase,
                                            reason: "archive-location finality changed its exact signed intent"
                                                .to_owned(),
                                        });
                                    }
                                    next.archive_location_attempts
                                        .last_mut()
                                        .expect("active location attempt exists")
                                        .registration = Some(registration);
                                    next.phase = PublicationPhaseV1::Replication;
                                }
                                PublicationArchiveLocationAdvanceV1::Terminal(terminal) => {
                                    append_location_terminal(&journal, &mut next, terminal)?;
                                }
                            }
                        }
                    }
                }
            }
            PublicationPhaseV1::Replication => {
                let registration = journal.registration()?;
                match backend
                    .finalized_replication(operation_id, &journal.request, registration)
                    .map_err(PublicationError::Backend)?
                {
                    PublicationReplicationAdvanceV1::Pending => {
                        return Ok(PublicationAdvanceV1::Pending(phase));
                    }
                    PublicationReplicationAdvanceV1::Healthy(checkpoint) => {
                        checkpoint.validate_for(&journal.request, registration)?;
                        next.replication = Some(checkpoint);
                        next.phase = PublicationPhaseV1::Readback;
                    }
                    PublicationReplicationAdvanceV1::Retired(terminal) => {
                        append_location_terminal(&journal, &mut next, terminal)?;
                    }
                }
            }
            PublicationPhaseV1::Readback => {
                let registration = journal.registration()?;
                let journaled_checkpoint = journal.replication.as_ref().ok_or_else(|| {
                    PublicationError::InvalidJournal("missing finalized replication".to_owned())
                })?;
                let checkpoint = match backend
                    .finalized_replication(operation_id, &journal.request, registration)
                    .map_err(PublicationError::Backend)?
                {
                    PublicationReplicationAdvanceV1::Pending => {
                        return Ok(PublicationAdvanceV1::Pending(phase));
                    }
                    PublicationReplicationAdvanceV1::Retired(terminal) => {
                        if retirement_checkpoint_progress(
                            &journal,
                            journaled_checkpoint,
                            &terminal,
                        )? == PublicationLocationProgressV1::Stale
                        {
                            return Ok(PublicationAdvanceV1::Pending(phase));
                        }
                        append_location_terminal(&journal, &mut next, terminal)?;
                        return self.persist_advance(&journal, next);
                    }
                    PublicationReplicationAdvanceV1::Healthy(checkpoint) => checkpoint,
                };
                if replication_checkpoint_progress(
                    &journal.request,
                    registration,
                    journaled_checkpoint,
                    &checkpoint,
                )? == PublicationLocationProgressV1::Stale
                {
                    return Ok(PublicationAdvanceV1::Pending(phase));
                }
                if &checkpoint != journaled_checkpoint {
                    next.replication = Some(checkpoint.clone());
                }
                let location = checkpoint.location(registration)?;
                let mut readbacks = Vec::with_capacity(2);
                let mut first_permanent_failure = None;
                let mut first_retryable_failure = None;
                let mut first_invalid_evidence = None;
                for provider in &location.providers {
                    let evidence = match backend.readback_provider(
                        operation_id,
                        &journal.request,
                        location,
                        *provider,
                    ) {
                        Ok(evidence) => evidence,
                        Err(error) => {
                            match error.class() {
                                PublicationBackendFailureClass::Permanent => {
                                    if first_permanent_failure.is_none() {
                                        first_permanent_failure = Some(error);
                                    }
                                }
                                PublicationBackendFailureClass::Retryable => {
                                    if first_retryable_failure.is_none() {
                                        first_retryable_failure = Some(error);
                                    }
                                }
                            }
                            continue;
                        }
                    };
                    if let Err(error) = evidence.validate_for(&journal.request, location, *provider)
                    {
                        if first_invalid_evidence.is_none() {
                            first_invalid_evidence = Some(error);
                        }
                        continue;
                    }
                    readbacks.push(evidence);
                    if readbacks.len() == 2 {
                        break;
                    }
                }
                if readbacks.len() != 2 {
                    if let Some(error) = first_permanent_failure.or(first_retryable_failure) {
                        return Err(PublicationError::Backend(error));
                    }
                    if let Some(error) = first_invalid_evidence {
                        return Err(error);
                    }
                    return Err(PublicationError::Backend(
                        PublicationBackendError::retryable("PROVIDER_READBACK_QUORUM_UNAVAILABLE"),
                    ));
                }
                validate_readback_subset(&journal.request, location, &readbacks)?;
                let Some(attempt) = prepare_release_submission_attempt(
                    &journal,
                    registration,
                    checkpoint.clone(),
                    readbacks,
                    backend,
                )?
                else {
                    if &checkpoint != journaled_checkpoint {
                        next.replication = Some(checkpoint);
                        return self.persist_advance(&journal, next);
                    }
                    return Ok(PublicationAdvanceV1::Pending(phase));
                };
                next.replication = Some(attempt.intent.preparation.replication.clone());
                next.readbacks
                    .clone_from(&attempt.intent.preparation.readbacks);
                next.release_submission_attempts.push(attempt);
                next.phase = PublicationPhaseV1::ReleaseSubmission;
            }
            PublicationPhaseV1::ReleaseSubmission => {
                let registration = journal.registration()?;
                let journaled_checkpoint = journal.replication.as_ref().ok_or_else(|| {
                    PublicationError::InvalidJournal("missing finalized replication".to_owned())
                })?;
                if let Some(active_attempt) = journal
                    .release_submission_attempts
                    .last()
                    .filter(|attempt| attempt.outcome.is_none())
                {
                    let allow_absent_submission = match backend
                        .finalized_replication(operation_id, &journal.request, registration)
                        .map_err(PublicationError::Backend)?
                    {
                        PublicationReplicationAdvanceV1::Pending => false,
                        PublicationReplicationAdvanceV1::Retired(terminal) => {
                            retirement_checkpoint_progress(
                                &journal,
                                journaled_checkpoint,
                                &terminal,
                            )?;
                            false
                        }
                        PublicationReplicationAdvanceV1::Healthy(checkpoint) => {
                            let progress = replication_checkpoint_progress(
                                &journal.request,
                                registration,
                                journaled_checkpoint,
                                &checkpoint,
                            )?;
                            let current_location = checkpoint.location(registration)?;
                            let signed_location = active_attempt
                                .intent
                                .preparation
                                .location()
                                .ok_or_else(|| {
                                    PublicationError::InvalidJournal(
                                        "release intent is missing its signed location".to_owned(),
                                    )
                                })?;
                            progress == PublicationLocationProgressV1::Current
                                && current_location == signed_location
                        }
                    };
                    match backend
                        .submit_or_recover_release_submission(
                            operation_id,
                            &journal.request,
                            &active_attempt.intent,
                            allow_absent_submission,
                        )
                        .map_err(PublicationError::Backend)?
                    {
                        PublicationReleaseSubmissionAdvanceV1::Pending => {
                            return Ok(PublicationAdvanceV1::Pending(phase));
                        }
                        PublicationReleaseSubmissionAdvanceV1::Applied(submission) => {
                            let outcome = PublicationReleaseSubmissionOutcomeV1::applied(
                                &active_attempt.intent,
                                submission,
                            );
                            outcome.validate_for(
                                operation_id,
                                &journal.request,
                                &active_attempt.intent,
                            )?;
                            next.release_submission_attempts
                                .last_mut()
                                .expect("active release attempt exists")
                                .outcome = Some(outcome);
                            next.submission = Some(submission);
                            next.phase = PublicationPhaseV1::FinalVerification;
                        }
                        PublicationReleaseSubmissionAdvanceV1::Terminal(terminal) => {
                            terminal.validate_for(&journal.request, &active_attempt.intent)?;
                            next.release_submission_attempts
                                .last_mut()
                                .expect("active release attempt exists")
                                .outcome =
                                Some(PublicationReleaseSubmissionOutcomeV1::Terminal(terminal));
                        }
                    }
                    return self.persist_advance(&journal, next);
                }
                let journaled_location = journaled_checkpoint.location(registration)?;
                match backend
                    .finalized_replication(operation_id, &journal.request, registration)
                    .map_err(PublicationError::Backend)?
                {
                    PublicationReplicationAdvanceV1::Pending => {
                        return Ok(PublicationAdvanceV1::Pending(phase));
                    }
                    PublicationReplicationAdvanceV1::Retired(terminal) => {
                        if retirement_checkpoint_progress(
                            &journal,
                            journaled_checkpoint,
                            &terminal,
                        )? == PublicationLocationProgressV1::Stale
                        {
                            return Ok(PublicationAdvanceV1::Pending(phase));
                        }
                        append_location_terminal(&journal, &mut next, terminal)?;
                    }
                    PublicationReplicationAdvanceV1::Healthy(checkpoint) => {
                        if replication_checkpoint_progress(
                            &journal.request,
                            registration,
                            journaled_checkpoint,
                            &checkpoint,
                        )? == PublicationLocationProgressV1::Stale
                        {
                            return Ok(PublicationAdvanceV1::Pending(phase));
                        }
                        let location = checkpoint.location(registration)?;
                        if &checkpoint != journaled_checkpoint {
                            let target_changed = location != journaled_location;
                            next.replication = Some(checkpoint.clone());
                            if target_changed {
                                next.readbacks.clear();
                                next.phase = PublicationPhaseV1::Readback;
                                return self.persist_advance(&journal, next);
                            }
                        }
                        let Some(attempt) = prepare_release_submission_attempt(
                            &journal,
                            registration,
                            checkpoint,
                            journal.readbacks.clone(),
                            backend,
                        )?
                        else {
                            if next.replication != journal.replication {
                                return self.persist_advance(&journal, next);
                            }
                            return Ok(PublicationAdvanceV1::Pending(phase));
                        };
                        next.replication = Some(attempt.intent.preparation.replication.clone());
                        next.readbacks
                            .clone_from(&attempt.intent.preparation.readbacks);
                        next.release_submission_attempts.push(attempt);
                    }
                }
            }
            PublicationPhaseV1::FinalVerification => {
                let submission = journal.submission.as_ref().ok_or_else(|| {
                    PublicationError::InvalidJournal("missing Native AMX submission".to_owned())
                })?;
                let Some(final_evidence) = backend
                    .finalized_release_and_index(operation_id, &journal.request, submission)
                    .map_err(PublicationError::Backend)?
                else {
                    return Ok(PublicationAdvanceV1::Pending(phase));
                };
                next.completion = Some(PublicationFinalCheckpointV1::from_verified(
                    &journal.request,
                    submission,
                    &final_evidence,
                )?);
            }
        }
        self.persist_advance(&journal, next)
    }

    fn persist_advance(
        &self,
        journal: &PublicationJournalV1,
        next: PublicationJournalV1,
    ) -> Result<PublicationAdvanceV1, PublicationError> {
        let persisted = self.store.transition(journal, next)?;
        if let Some(result) = persisted.result() {
            Ok(PublicationAdvanceV1::Complete(result))
        } else {
            Ok(PublicationAdvanceV1::Progressed(persisted.phase))
        }
    }

    fn run(
        &self,
        operation_id: PublicationOperationIdV1,
        source: &dyn PublicationCarSource,
        backend: &mut dyn PublicationBackend,
    ) -> Result<PublicationAdvanceV1, PublicationError> {
        loop {
            match self.advance_once(operation_id, source, backend)? {
                PublicationAdvanceV1::Progressed(_) => {}
                terminal => return Ok(terminal),
            }
        }
    }
}

fn verify_seed_ingress_receipt_with_bounded_service_lead(
    receipt: &MusubiSeedIngressReceiptV1,
    expected: &MusubiSeedIngressReceiptBindingV1,
    current_time_ms: u64,
) -> Result<(), PublicationError> {
    let latest_accepted_issue_time = current_time_ms
        .checked_add(MUSUBI_PUBLICATION_SERVICE_MAX_CLOCK_SKEW_MS_V1)
        .ok_or_else(|| PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::SeedIngress,
            reason: "seed-ingress receipt clock bound overflowed".to_owned(),
        })?;
    if current_time_ms == 0 || receipt.payload.issued_at_ms > latest_accepted_issue_time {
        return Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::SeedIngress,
            reason: "seed-ingress receipt exceeds the bounded service clock lead".to_owned(),
        });
    }
    receipt
        .verify(expected, current_time_ms.max(receipt.payload.issued_at_ms))
        .map_err(|error| invalid(PublicationPhaseV1::SeedIngress, error))
}

fn append_location_terminal(
    journal: &PublicationJournalV1,
    next: &mut PublicationJournalV1,
    terminal: PublicationArchiveLocationTerminalV1,
) -> Result<(), PublicationError> {
    let floor = location_terminal_floor(journal)?;
    validate_location_terminal(journal, &terminal, &floor)?;
    let attempt = next
        .archive_location_attempts
        .last_mut()
        .expect("active location attempt exists");
    attempt.terminal = Some(terminal);
    attempt.terminal_floor = Some(floor);
    next.replication = None;
    next.readbacks.clear();
    next.phase = PublicationPhaseV1::ArchiveRegistration;
    Ok(())
}

fn location_terminal_floor(
    journal: &PublicationJournalV1,
) -> Result<PublicationArchiveLocationTerminalFloorV1, PublicationError> {
    let attempt = journal
        .archive_location_attempts
        .last()
        .filter(|attempt| attempt.terminal.is_none())
        .ok_or_else(|| {
            PublicationError::InvalidJournal(
                "archive-location terminal evidence has no active generation".to_owned(),
            )
        })?;
    Ok(journal.replication.as_ref().map_or_else(
        || {
            if attempt.registration.is_some() {
                PublicationArchiveLocationTerminalFloorV1::Registered
            } else {
                PublicationArchiveLocationTerminalFloorV1::Prepared
            }
        },
        |checkpoint| PublicationArchiveLocationTerminalFloorV1::Replication(checkpoint.clone()),
    ))
}

fn validate_location_terminal(
    journal: &PublicationJournalV1,
    terminal: &PublicationArchiveLocationTerminalV1,
    floor: &PublicationArchiveLocationTerminalFloorV1,
) -> Result<(), PublicationError> {
    let attempt = journal
        .archive_location_attempts
        .last()
        .filter(|attempt| attempt.terminal.is_none())
        .ok_or_else(|| {
            PublicationError::InvalidJournal(
                "archive-location terminal evidence has no active generation".to_owned(),
            )
        })?;
    let registered = journal.registered_archive.as_ref().ok_or_else(|| {
        PublicationError::InvalidJournal(
            "archive-location terminal evidence is missing archive finality".to_owned(),
        )
    })?;
    let prior_location_ids = journal.archive_location_attempts
        [..journal.archive_location_attempts.len() - 1]
        .iter()
        .map(|prior| prior.intent.location_id)
        .collect::<Vec<_>>();
    terminal.validate_for(
        journal.operation_id,
        &journal.request,
        registered,
        attempt,
        &prior_location_ids,
        floor,
    )?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PublicationLocationProgressV1 {
    Stale,
    Current,
}

fn finalized_page_progress(
    previous: &MusubiArchiveLocationPageV1,
    current: &MusubiArchiveLocationPageV1,
) -> Result<PublicationLocationProgressV1, PublicationError> {
    if (current.snapshot.finalized_height == previous.snapshot.finalized_height
        && current.snapshot != previous.snapshot)
        || (current.snapshot == previous.snapshot
            && (current.archive != previous.archive || current.items != previous.items))
        || (current.archive.location_revision == previous.archive.location_revision
            && (current.archive != previous.archive || current.items != previous.items))
    {
        return Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::Replication,
            reason: "equal finalized archive-location checkpoints carried different state"
                .to_owned(),
        });
    }
    if current.snapshot.finalized_height < previous.snapshot.finalized_height
        || current.snapshot.index_revision < previous.snapshot.index_revision
        || current.archive.location_revision < previous.archive.location_revision
    {
        return Ok(PublicationLocationProgressV1::Stale);
    }
    Ok(PublicationLocationProgressV1::Current)
}

fn replication_checkpoint_progress(
    request: &PublicationRequestV1,
    registration: &PublicationArchiveRegistrationV1,
    previous: &PublicationReplicationCheckpointV1,
    current: &PublicationReplicationCheckpointV1,
) -> Result<PublicationLocationProgressV1, PublicationError> {
    previous.validate_for(request, registration)?;
    current.validate_for(request, registration)?;
    if finalized_page_progress(&previous.finalized_page, &current.finalized_page)?
        == PublicationLocationProgressV1::Stale
    {
        return Ok(PublicationLocationProgressV1::Stale);
    }
    location_progress(
        previous.location(registration)?,
        current.location(registration)?,
    )
}

fn retirement_checkpoint_progress(
    journal: &PublicationJournalV1,
    checkpoint: &PublicationReplicationCheckpointV1,
    terminal: &PublicationArchiveLocationTerminalV1,
) -> Result<PublicationLocationProgressV1, PublicationError> {
    let registration = journal.registration()?;
    checkpoint.validate_for(&journal.request, registration)?;
    validate_location_terminal(
        journal,
        terminal,
        &PublicationArchiveLocationTerminalFloorV1::Registered,
    )?;
    if finalized_page_progress(&checkpoint.finalized_page, &terminal.finalized_page)?
        == PublicationLocationProgressV1::Stale
        || terminal.finalized_page.archive.location_revision
            <= checkpoint.finalized_page.archive.location_revision
    {
        return Ok(PublicationLocationProgressV1::Stale);
    }
    let floor = location_terminal_floor(journal)?;
    validate_location_terminal(journal, terminal, &floor)?;
    Ok(PublicationLocationProgressV1::Current)
}

fn location_progress(
    previous: &MusubiArchiveLocationV1,
    current: &MusubiArchiveLocationV1,
) -> Result<PublicationLocationProgressV1, PublicationError> {
    if current.archive_id != previous.archive_id || current.location_id != previous.location_id {
        return Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::Replication,
            reason: "finalized archive location changed its stable identity".to_owned(),
        });
    }
    if current.revision < previous.revision {
        return Ok(PublicationLocationProgressV1::Stale);
    }
    if current.revision == previous.revision {
        return if current == previous {
            Ok(PublicationLocationProgressV1::Current)
        } else {
            Err(PublicationError::InvalidEvidence {
                phase: PublicationPhaseV1::Replication,
                reason: "equal archive-location revisions carried different records".to_owned(),
            })
        };
    }
    if current.finalized_height < previous.finalized_height {
        return Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::Replication,
            reason: "archive-location revision advanced while finality regressed".to_owned(),
        });
    }
    Ok(PublicationLocationProgressV1::Current)
}

/// Validate a current healthy location for the immutable publication archive.
///
/// The stable location identity cannot be reused after retirement. Its renewable pin, order,
/// provider set, and epochs may legitimately advance after the coordinator checkpoint. Core
/// exact-resolves the location's aggregate digest to immutable provider proofs before publishing
/// this compact finalized state, while the publisher independently verifies archive bytes through
/// two selected providers before release submission.
pub(crate) fn validate_replication(
    request: &PublicationRequestV1,
    registration: &PublicationArchiveRegistrationV1,
    location: &MusubiArchiveLocationV1,
) -> Result<(), PublicationError> {
    location
        .validate()
        .map_err(|error| invalid(PublicationPhaseV1::Replication, error))?;
    let registered_location = registration.location()?;
    if location.archive_id != request.archive_commitment.archive_id()
        || location.location_id != registration.location_id()
        || location.state != MusubiArchiveLocationStateV1::Healthy
        || location.providers.len() < usize::from(MUSUBI_MIN_HEALTHY_REPLICAS_V1)
        || location.finalized_height < registration.applied_height
        || location_progress(registered_location, location)?
            != PublicationLocationProgressV1::Current
    {
        return Err(PublicationError::InvalidEvidence {
            phase: PublicationPhaseV1::Replication,
            reason: "finalized archive location, pin, order, or quorum was substituted".to_owned(),
        });
    }
    Ok(())
}

/// Publication workflow error with retry class preserved for backend failures.
#[derive(Debug)]
pub enum PublicationError {
    /// The immutable CAR or canonical plan sidecar could not be reopened or read.
    CarSource(io::Error),
    /// A backend transition failed without persisting secrets in the journal.
    Backend(PublicationBackendError),
    /// Signed, finalized, compiler, or readback evidence did not exactly match the request.
    InvalidEvidence {
        /// Phase that rejected the evidence.
        phase: PublicationPhaseV1,
        /// Public non-secret failure reason.
        reason: String,
    },
    /// A journal was malformed, inconsistent, unsafe, or noncanonical.
    InvalidJournal(String),
    /// Atomic durable journal replacement failed.
    JournalWrite(AtomicWriteError),
    /// A journal filesystem operation failed.
    JournalIo(io::Error),
    /// No journal exists for this typed operation id.
    NotFound(PublicationOperationIdV1),
    /// Another resume changed the journal between load and durable transition.
    ConcurrentJournalUpdate,
}

impl fmt::Display for PublicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CarSource(error) => {
                write!(
                    formatter,
                    "failed to open publication CAR or plan sidecar: {error}"
                )
            }
            Self::Backend(error) => write!(formatter, "publication backend failed: {error}"),
            Self::InvalidEvidence { phase, reason } => {
                write!(formatter, "invalid {phase:?} evidence: {reason}")
            }
            Self::InvalidJournal(reason) => {
                write!(formatter, "invalid publication journal: {reason}")
            }
            Self::JournalWrite(error) => {
                write!(formatter, "failed to write publication journal: {error}")
            }
            Self::JournalIo(error) => write!(formatter, "publication journal I/O failed: {error}"),
            Self::NotFound(operation_id) => {
                write!(
                    formatter,
                    "publication operation `{operation_id}` was not found"
                )
            }
            Self::ConcurrentJournalUpdate => {
                formatter.write_str("publication journal changed during a resumable transition")
            }
        }
    }
}

impl Error for PublicationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CarSource(error) | Self::JournalIo(error) => Some(error),
            Self::Backend(error) => Some(error),
            Self::JournalWrite(error) => Some(error),
            Self::InvalidEvidence { .. }
            | Self::InvalidJournal(_)
            | Self::NotFound(_)
            | Self::ConcurrentJournalUpdate => None,
        }
    }
}

fn invalid(phase: PublicationPhaseV1, error: impl fmt::Display) -> PublicationError {
    PublicationError::InvalidEvidence {
        phase,
        reason: error.to_string(),
    }
}

fn decode_publication_journal(bytes: &[u8]) -> Result<PublicationJournalV1, PublicationError> {
    if bytes.is_empty() || bytes.len() > MAX_JOURNAL_BYTES_USIZE {
        return Err(PublicationError::InvalidJournal(
            "journal exceeds its fixed canonical frame bound".to_owned(),
        ));
    }
    // First-release reset semantics are fail-closed: there is no parser or field synthesis for
    // any pre-release journal layout.
    norito::decode_canonical_with_limits(bytes, JOURNAL_DECODE_LIMITS).map_err(|error| {
        PublicationError::InvalidJournal(format!("journal is not canonical Norito: {error}"))
    })
}
