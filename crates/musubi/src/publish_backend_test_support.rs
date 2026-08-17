// Publication backend fault-injection implementations and shared evidence fixtures.
struct EarlyBackend {
    broker: KeyPair,
    fail_validation_once: bool,
    substitute_receipt: bool,
    now_ms: u64,
    receipt_window: Option<(u64, u64)>,
    prepare_calls: usize,
}
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent fault-injection switches make each publication phase explicit in tests"
)]
struct CompleteBackend {
    broker: KeyPair,
    replication_pending_once: bool,
    finality_pending_once: bool,
    substitute_readback: bool,
    substitute_all_readbacks: bool,
    readback_backend_failure: Option<(ProviderId, PublicationBackendError)>,
    readback_providers: Vec<ProviderId>,
    submissions: usize,
}
struct ArchiveRecoveryBackend {
    broker: KeyPair,
    now_ms: u64,
    staged_receipts: Vec<MusubiSeedIngressReceiptV1>,
    prepare_calls: usize,
    registration_calls: usize,
    pin_calls: usize,
    archive_committed: bool,
    drop_commit_response_once: bool,
    return_conflicting_archive: bool,
    registration_mode: ArchiveRecoveryMode,
}
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent crash and rejection switches model distinct recovery cuts in tests"
)]
struct LocationRecoveryBackend {
    broker: KeyPair,
    replication_script: VecDeque<LocationPollV1>,
    prepared_generations: Vec<(u8, Vec<MusubiArchiveLocationIdV1>)>,
    applied_generations: Vec<u8>,
    drop_location_response_once: bool,
    reject_release: bool,
    release_preparations: usize,
    release_submissions: usize,
    release_intents: Vec<[u8; 32]>,
    release_pending_responses: usize,
    drop_release_response_once: bool,
    release_applied: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArchiveRecoveryMode {
    Commit,
    Pending,
    ExpiredAbsent,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LocationPollV1 {
    Healthy,
    HealthyRevisionOffset(u64),
    HealthyDirectoryAdvance,
    Retired,
    RetiredRevisionOffset(u64),
}
impl EarlyBackend {
    fn unsupported() -> PublicationBackendError {
        PublicationBackendError::permanent("UNEXPECTED_TEST_PHASE")
    }
}
fn validate_seed_stage_fixture(
    expected: &MusubiSeedIngressReceiptBindingV1,
    commitment: &MusubiArchiveCommitmentV1,
    plan: &MusubiSeedIngressCarPlanV1,
) -> Result<(), PublicationBackendError> {
    if expected.archive_id != commitment.archive_id()
        || expected.car_body_digest != commitment.car_digest
        || expected.car_body_length != commitment.car_size
    {
        return Err(PublicationBackendError::permanent(
            "TEST_SEED_COMMITMENT_INVALID",
        ));
    }
    plan.validate(commitment)
        .map_err(|_| PublicationBackendError::permanent("TEST_SEED_PLAN_INVALID"))
}
impl PublicationBackend for EarlyBackend {
    fn current_time_ms(&mut self) -> Result<u64, PublicationBackendError> {
        Ok(self.now_ms)
    }
    fn validate_clean_package(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        car: &mut dyn Read,
    ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError> {
        if self.fail_validation_once {
            self.fail_validation_once = false;
            return Err(PublicationBackendError::retryable(
                "COMPILER_TEMPORARILY_UNAVAILABLE",
            ));
        }
        let mut consumed = Vec::new();
        car.read_to_end(&mut consumed)
            .map_err(|_| PublicationBackendError::permanent("CAR_READ_FAILED"))?;
        if consumed.is_empty() {
            return Err(PublicationBackendError::permanent("EMPTY_TEST_CAR"));
        }
        Ok(validation_evidence(request))
    }
    fn stage_authenticated_seed_ingress(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        expected: &MusubiSeedIngressReceiptBindingV1,
        commitment: &MusubiArchiveCommitmentV1,
        plan: &MusubiSeedIngressCarPlanV1,
        _car: &mut dyn Read,
    ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError> {
        validate_seed_stage_fixture(expected, commitment, plan)?;
        let mut receipt = self.receipt_window.map_or_else(
            || signed_receipt(expected, &self.broker),
            |(issued_at_ms, expires_at_ms)| {
                signed_receipt_at(expected, &self.broker, issued_at_ms, expires_at_ms)
            },
        );
        if self.substitute_receipt {
            receipt.payload.binding.archive_id = ArchiveId::new([0xEE; 32]);
        }
        Ok(receipt)
    }
    fn prepare_archive_registration_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        receipt: &MusubiSeedIngressReceiptV1,
    ) -> Result<PublicationArchiveRegistrationIntentV1, PublicationBackendError> {
        self.prepare_calls += 1;
        Ok(registration_intent(operation_id, request, receipt.clone()))
    }
    fn submit_or_recover_archive_registration(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _intent: &PublicationArchiveRegistrationIntentV1,
    ) -> Result<PublicationArchiveRegistrationAdvanceV1, PublicationBackendError> {
        Err(Self::unsupported())
    }
    fn prepare_archive_location_intent(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _registered: &PublicationRegisteredArchiveV1,
        _generation: u8,
        _prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationIntentV1, PublicationBackendError> {
        Err(Self::unsupported())
    }
    fn submit_or_recover_archive_location(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _registered: &PublicationRegisteredArchiveV1,
        _intent: &PublicationArchiveLocationIntentV1,
        _prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationAdvanceV1, PublicationBackendError> {
        Err(Self::unsupported())
    }
    fn finalized_replication(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _registration: &PublicationArchiveRegistrationV1,
    ) -> Result<PublicationReplicationAdvanceV1, PublicationBackendError> {
        Err(Self::unsupported())
    }
    fn readback_provider(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _location: &MusubiArchiveLocationV1,
        _provider: ProviderId,
    ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError> {
        Err(Self::unsupported())
    }
    fn prepare_release_submission_intent(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _preparation: &PublicationReleasePreparationFloorV1,
    ) -> Result<PublicationReleaseSubmissionIntentV1, PublicationBackendError> {
        Err(Self::unsupported())
    }
    fn submit_or_recover_release_submission(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _intent: &PublicationReleaseSubmissionIntentV1,
        _allow_absent_submission: bool,
    ) -> Result<PublicationReleaseSubmissionAdvanceV1, PublicationBackendError> {
        Err(Self::unsupported())
    }
    fn finalized_release_and_index(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _submission: &PublicationAmxSubmissionV1,
    ) -> Result<Option<PublicationFinalEvidenceV1>, PublicationBackendError> {
        Err(Self::unsupported())
    }
}
impl PublicationBackend for CompleteBackend {
    fn current_time_ms(&mut self) -> Result<u64, PublicationBackendError> {
        Ok(1_500)
    }
    fn validate_clean_package(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _car: &mut dyn Read,
    ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError> {
        Ok(validation_evidence(request))
    }
    fn stage_authenticated_seed_ingress(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        expected: &MusubiSeedIngressReceiptBindingV1,
        commitment: &MusubiArchiveCommitmentV1,
        plan: &MusubiSeedIngressCarPlanV1,
        _car: &mut dyn Read,
    ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError> {
        validate_seed_stage_fixture(expected, commitment, plan)?;
        Ok(signed_receipt(expected, &self.broker))
    }
    fn prepare_archive_registration_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        receipt: &MusubiSeedIngressReceiptV1,
    ) -> Result<PublicationArchiveRegistrationIntentV1, PublicationBackendError> {
        Ok(registration_intent(operation_id, request, receipt.clone()))
    }
    fn submit_or_recover_archive_registration(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        intent: &PublicationArchiveRegistrationIntentV1,
    ) -> Result<PublicationArchiveRegistrationAdvanceV1, PublicationBackendError> {
        Ok(PublicationArchiveRegistrationAdvanceV1::Registered(
            registered_archive(request, &self.broker, intent),
        ))
    }
    fn prepare_archive_location_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _registered: &PublicationRegisteredArchiveV1,
        generation: u8,
        _prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationIntentV1, PublicationBackendError> {
        let mut result = registration(request, &self.broker).intent;
        result.operation_id = operation_id;
        result.generation = generation;
        Ok(result)
    }
    fn submit_or_recover_archive_location(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _registered: &PublicationRegisteredArchiveV1,
        intent: &PublicationArchiveLocationIntentV1,
        _prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationAdvanceV1, PublicationBackendError> {
        let mut result = registration(request, &self.broker);
        result.intent = intent.clone();
        Ok(PublicationArchiveLocationAdvanceV1::Registered(result))
    }
    fn finalized_replication(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
    ) -> Result<PublicationReplicationAdvanceV1, PublicationBackendError> {
        if self.replication_pending_once {
            self.replication_pending_once = false;
            return Ok(PublicationReplicationAdvanceV1::Pending);
        }
        Ok(PublicationReplicationAdvanceV1::Healthy(
            replication_checkpoint(request, registration, 3),
        ))
    }
    fn readback_provider(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        location: &MusubiArchiveLocationV1,
        provider: ProviderId,
    ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError> {
        self.readback_providers.push(provider);
        if let Some((failed_provider, error)) = &self.readback_backend_failure
            && *failed_provider == provider
        {
            return Err(error.clone());
        }
        let mut evidence = PublicationReadbackEvidenceV1 {
            provider,
            location_id: location.location_id,
            replication_order: location.replication_order,
            commitment: request.archive_commitment.clone(),
            semantic_release_digest: request.publication.manifest.semantic_digest(),
            verification_lock_digest: request.publication.manifest.verification_lock_digest,
        };
        if self.substitute_all_readbacks
            || (self.substitute_readback && provider == location.providers[0])
        {
            evidence.commitment.car_digest = MusubiContentDigestV1::new([0xEE; 32]);
        }
        Ok(evidence)
    }
    fn prepare_release_submission_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        preparation: &PublicationReleasePreparationFloorV1,
    ) -> Result<PublicationReleaseSubmissionIntentV1, PublicationBackendError> {
        let nonce = u32::try_from(self.submissions + 1).expect("test submission count fits u32");
        PublicationReleaseSubmissionIntentV1::try_new(
            operation_id,
            request,
            preparation.clone(),
            &signed_release_transaction(request, nonce),
        )
        .map_err(|_| PublicationBackendError::permanent("TEST_RELEASE_INTENT_INVALID"))
    }
    fn submit_or_recover_release_submission(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        intent: &PublicationReleaseSubmissionIntentV1,
        allow_absent_submission: bool,
    ) -> Result<PublicationReleaseSubmissionAdvanceV1, PublicationBackendError> {
        if !allow_absent_submission {
            return Ok(PublicationReleaseSubmissionAdvanceV1::Pending);
        }
        self.submissions += 1;
        Ok(PublicationReleaseSubmissionAdvanceV1::Applied(
            PublicationAmxSubmissionV1::new(
                operation_id,
                &request.publish_instruction(),
                intent.transaction_hash,
                80,
            ),
        ))
    }
    fn finalized_release_and_index(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _submission: &PublicationAmxSubmissionV1,
    ) -> Result<Option<PublicationFinalEvidenceV1>, PublicationBackendError> {
        if self.finality_pending_once {
            self.finality_pending_once = false;
            return Ok(None);
        }
        Ok(Some(final_evidence(request)))
    }
}
impl ArchiveRecoveryBackend {
    fn unsupported() -> PublicationBackendError {
        PublicationBackendError::permanent("UNEXPECTED_TEST_PHASE")
    }
}
impl PublicationBackend for ArchiveRecoveryBackend {
    fn current_time_ms(&mut self) -> Result<u64, PublicationBackendError> {
        Ok(self.now_ms)
    }
    fn validate_clean_package(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _car: &mut dyn Read,
    ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError> {
        Ok(validation_evidence(request))
    }
    fn stage_authenticated_seed_ingress(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        expected: &MusubiSeedIngressReceiptBindingV1,
        commitment: &MusubiArchiveCommitmentV1,
        plan: &MusubiSeedIngressCarPlanV1,
        _car: &mut dyn Read,
    ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError> {
        validate_seed_stage_fixture(expected, commitment, plan)?;
        let receipt = signed_receipt_at(expected, &self.broker, self.now_ms, self.now_ms + 100);
        self.staged_receipts.push(receipt.clone());
        Ok(receipt)
    }
    fn prepare_archive_registration_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        receipt: &MusubiSeedIngressReceiptV1,
    ) -> Result<PublicationArchiveRegistrationIntentV1, PublicationBackendError> {
        self.prepare_calls += 1;
        Ok(registration_intent(operation_id, request, receipt.clone()))
    }
    fn submit_or_recover_archive_registration(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        intent: &PublicationArchiveRegistrationIntentV1,
    ) -> Result<PublicationArchiveRegistrationAdvanceV1, PublicationBackendError> {
        self.registration_calls += 1;
        match self.registration_mode {
            ArchiveRecoveryMode::Pending => {
                return Ok(PublicationArchiveRegistrationAdvanceV1::Pending);
            }
            ArchiveRecoveryMode::ExpiredAbsent => {
                self.now_ms = intent.staging_receipt.payload.expires_at_ms + 1;
                return Ok(PublicationArchiveRegistrationAdvanceV1::TerminalAbsent(
                    PublicationArchiveRegistrationTerminalV1::registry_expired(
                        intent,
                        Some(60),
                        archive_absence_evidence(request, 60),
                    ),
                ));
            }
            ArchiveRecoveryMode::Commit => {}
        }
        if !self.archive_committed {
            self.archive_committed = true;
            self.now_ms = intent.staging_receipt.payload.expires_at_ms + 1;
            if self.drop_commit_response_once {
                self.drop_commit_response_once = false;
                return Err(PublicationBackendError::retryable(
                    "ARCHIVE_COMMIT_RESPONSE_DROPPED",
                ));
            }
        }
        let mut recovered = registered_archive(request, &self.broker, intent);
        if self.return_conflicting_archive {
            recovered.archive.registered_by = account(99).0;
        }
        Ok(PublicationArchiveRegistrationAdvanceV1::Registered(
            recovered,
        ))
    }
    fn prepare_archive_location_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registered: &PublicationRegisteredArchiveV1,
        generation: u8,
        _prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationIntentV1, PublicationBackendError> {
        self.pin_calls += 1;
        Ok(location_registration_generation(operation_id, request, registered, generation).intent)
    }
    fn submit_or_recover_archive_location(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _registered: &PublicationRegisteredArchiveV1,
        intent: &PublicationArchiveLocationIntentV1,
        _prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationAdvanceV1, PublicationBackendError> {
        Ok(PublicationArchiveLocationAdvanceV1::Registered(
            finalized_location_registration(request, intent),
        ))
    }
    fn finalized_replication(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _registration: &PublicationArchiveRegistrationV1,
    ) -> Result<PublicationReplicationAdvanceV1, PublicationBackendError> {
        Err(Self::unsupported())
    }
    fn readback_provider(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _location: &MusubiArchiveLocationV1,
        _provider: ProviderId,
    ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError> {
        Err(Self::unsupported())
    }
    fn prepare_release_submission_intent(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _preparation: &PublicationReleasePreparationFloorV1,
    ) -> Result<PublicationReleaseSubmissionIntentV1, PublicationBackendError> {
        Err(Self::unsupported())
    }
    fn submit_or_recover_release_submission(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _intent: &PublicationReleaseSubmissionIntentV1,
        _allow_absent_submission: bool,
    ) -> Result<PublicationReleaseSubmissionAdvanceV1, PublicationBackendError> {
        Err(Self::unsupported())
    }
    fn finalized_release_and_index(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        _request: &PublicationRequestV1,
        _submission: &PublicationAmxSubmissionV1,
    ) -> Result<Option<PublicationFinalEvidenceV1>, PublicationBackendError> {
        Err(Self::unsupported())
    }
}
impl LocationRecoveryBackend {
    fn new(broker: KeyPair, replication_script: impl IntoIterator<Item = LocationPollV1>) -> Self {
        Self {
            broker,
            replication_script: replication_script.into_iter().collect(),
            prepared_generations: Vec::new(),
            applied_generations: Vec::new(),
            drop_location_response_once: false,
            reject_release: false,
            release_preparations: 0,
            release_submissions: 0,
            release_intents: Vec::new(),
            release_pending_responses: 0,
            drop_release_response_once: false,
            release_applied: false,
        }
    }
}
impl PublicationBackend for LocationRecoveryBackend {
    fn current_time_ms(&mut self) -> Result<u64, PublicationBackendError> {
        Ok(1_500)
    }
    fn validate_clean_package(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _car: &mut dyn Read,
    ) -> Result<PublicationValidationEvidenceV1, PublicationBackendError> {
        Ok(validation_evidence(request))
    }
    fn stage_authenticated_seed_ingress(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        expected: &MusubiSeedIngressReceiptBindingV1,
        commitment: &MusubiArchiveCommitmentV1,
        plan: &MusubiSeedIngressCarPlanV1,
        _car: &mut dyn Read,
    ) -> Result<MusubiSeedIngressReceiptV1, PublicationBackendError> {
        validate_seed_stage_fixture(expected, commitment, plan)?;
        Ok(signed_receipt(expected, &self.broker))
    }
    fn prepare_archive_registration_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        receipt: &MusubiSeedIngressReceiptV1,
    ) -> Result<PublicationArchiveRegistrationIntentV1, PublicationBackendError> {
        Ok(registration_intent(operation_id, request, receipt.clone()))
    }
    fn submit_or_recover_archive_registration(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        intent: &PublicationArchiveRegistrationIntentV1,
    ) -> Result<PublicationArchiveRegistrationAdvanceV1, PublicationBackendError> {
        Ok(PublicationArchiveRegistrationAdvanceV1::Registered(
            registered_archive(request, &self.broker, intent),
        ))
    }
    fn prepare_archive_location_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registered: &PublicationRegisteredArchiveV1,
        generation: u8,
        prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationIntentV1, PublicationBackendError> {
        self.prepared_generations
            .push((generation, prior_location_ids.to_vec()));
        Ok(location_registration_generation(operation_id, request, registered, generation).intent)
    }
    fn submit_or_recover_archive_location(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _registered: &PublicationRegisteredArchiveV1,
        intent: &PublicationArchiveLocationIntentV1,
        _prior_location_ids: &[MusubiArchiveLocationIdV1],
    ) -> Result<PublicationArchiveLocationAdvanceV1, PublicationBackendError> {
        if !self.applied_generations.contains(&intent.generation) {
            self.applied_generations.push(intent.generation);
            if self.drop_location_response_once {
                self.drop_location_response_once = false;
                return Err(PublicationBackendError::retryable(
                    "ARCHIVE_LOCATION_COMMIT_RESPONSE_DROPPED",
                ));
            }
        }
        Ok(PublicationArchiveLocationAdvanceV1::Registered(
            finalized_location_registration(request, intent),
        ))
    }
    fn finalized_replication(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        registration: &PublicationArchiveRegistrationV1,
    ) -> Result<PublicationReplicationAdvanceV1, PublicationBackendError> {
        match self
            .replication_script
            .pop_front()
            .unwrap_or(LocationPollV1::Healthy)
        {
            LocationPollV1::Healthy => Ok(PublicationReplicationAdvanceV1::Healthy(
                replication_checkpoint(request, registration, 3),
            )),
            LocationPollV1::HealthyRevisionOffset(offset) => {
                Ok(PublicationReplicationAdvanceV1::Healthy(
                    replication_checkpoint_with_revision_offset(request, registration, offset),
                ))
            }
            LocationPollV1::HealthyDirectoryAdvance => {
                Ok(PublicationReplicationAdvanceV1::Healthy(
                    replication_checkpoint_with_directory_advance(request, registration),
                ))
            }
            LocationPollV1::Retired => Ok(PublicationReplicationAdvanceV1::Retired(
                retired_location_terminal(registration),
            )),
            LocationPollV1::RetiredRevisionOffset(offset) => {
                Ok(PublicationReplicationAdvanceV1::Retired(
                    retired_location_terminal_with_revision_offset(registration, offset),
                ))
            }
        }
    }
    fn readback_provider(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        location: &MusubiArchiveLocationV1,
        provider: ProviderId,
    ) -> Result<PublicationReadbackEvidenceV1, PublicationBackendError> {
        Ok(PublicationReadbackEvidenceV1 {
            provider,
            location_id: location.location_id,
            replication_order: location.replication_order,
            commitment: request.archive_commitment.clone(),
            semantic_release_digest: request.publication.manifest.semantic_digest(),
            verification_lock_digest: request.publication.manifest.verification_lock_digest,
        })
    }
    fn prepare_release_submission_intent(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        preparation: &PublicationReleasePreparationFloorV1,
    ) -> Result<PublicationReleaseSubmissionIntentV1, PublicationBackendError> {
        self.release_preparations += 1;
        let nonce = u32::try_from(self.release_preparations)
            .expect("test release submission count fits u32");
        PublicationReleaseSubmissionIntentV1::try_new(
            operation_id,
            request,
            preparation.clone(),
            &signed_release_transaction(request, nonce),
        )
        .map_err(|_| PublicationBackendError::permanent("TEST_RELEASE_INTENT_INVALID"))
    }
    fn submit_or_recover_release_submission(
        &mut self,
        operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        intent: &PublicationReleaseSubmissionIntentV1,
        allow_absent_submission: bool,
    ) -> Result<PublicationReleaseSubmissionAdvanceV1, PublicationBackendError> {
        self.release_intents.push(intent.signed_transaction_digest);
        if self.release_applied {
            return Ok(PublicationReleaseSubmissionAdvanceV1::Applied(
                PublicationAmxSubmissionV1::new(
                    operation_id,
                    &request.publish_instruction(),
                    intent.transaction_hash,
                    80,
                ),
            ));
        }
        if self.release_pending_responses > 0 {
            self.release_pending_responses -= 1;
            return Ok(PublicationReleaseSubmissionAdvanceV1::Pending);
        }
        if !allow_absent_submission {
            return Ok(PublicationReleaseSubmissionAdvanceV1::Pending);
        }
        self.release_submissions += 1;
        if self.reject_release {
            let block_height = intent
                .preparation
                .replication
                .finalized_page
                .snapshot
                .finalized_height
                .saturating_add(1);
            let finalized_time_ms = release_submission_valid_until_ms(intent)
                .expect("test release deadline")
                .saturating_add(1);
            let mut absence = release_absence_evidence(request, block_height, finalized_time_ms);
            let preparation_revision = intent
                .preparation
                .replication
                .finalized_page
                .snapshot
                .index_revision;
            absence.resolver_page.snapshot.index_revision = preparation_revision;
            absence.retention_query.expected_snapshot = Some(absence.resolver_page.snapshot);
            absence.retention_page.snapshot.index_revision = preparation_revision;
            absence.retention_page.items[0]
                .storage
                .as_mut()
                .expect("test archive remains known")
                .index_revision = preparation_revision;
            return Ok(PublicationReleaseSubmissionAdvanceV1::Terminal(
                PublicationReleaseSubmissionTerminalV1::registry_rejected(
                    intent,
                    block_height,
                    absence,
                ),
            ));
        }
        self.release_applied = true;
        if self.drop_release_response_once {
            self.drop_release_response_once = false;
            return Err(PublicationBackendError::retryable(
                "RELEASE_COMMIT_RESPONSE_DROPPED",
            ));
        }
        Ok(PublicationReleaseSubmissionAdvanceV1::Applied(
            PublicationAmxSubmissionV1::new(
                operation_id,
                &request.publish_instruction(),
                intent.transaction_hash,
                80,
            ),
        ))
    }
    fn finalized_release_and_index(
        &mut self,
        _operation_id: PublicationOperationIdV1,
        request: &PublicationRequestV1,
        _submission: &PublicationAmxSubmissionV1,
    ) -> Result<Option<PublicationFinalEvidenceV1>, PublicationBackendError> {
        Ok(Some(final_evidence(request)))
    }
}
fn account(seed: u8) -> (AccountId, KeyPair) {
    let keypair =
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("fixture keypair");
    (AccountId::new(keypair.public_key().clone()), keypair)
}
fn maximum_legal_musubi_account() -> AccountId {
    let members = (0_u16..256)
        .map(|index| {
            let mut seed = [0xC4; 32];
            seed[..2].copy_from_slice(&index.to_le_bytes());
            let keypair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519)
                .expect("near-limit account keypair");
            MultisigMember::new(keypair.public_key().clone(), 1).expect("near-limit account member")
        })
        .collect::<Vec<_>>();
    for count in (1..=members.len()).rev() {
        let policy =
            MultisigPolicy::new(1, members[..count].to_vec()).expect("near-limit account policy");
        let account = AccountId::new_multisig(policy);
        let size = norito::to_bytes(&account)
            .expect("near-limit account canonical bytes")
            .len();
        if size <= MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1 {
            assert!(count < members.len(), "fixture must cross the Musubi bound");
            let larger = AccountId::new_multisig(
                MultisigPolicy::new(1, members[..=count].to_vec())
                    .expect("one-member-larger account policy"),
            );
            assert!(
                norito::to_bytes(&larger)
                    .expect("one-member-larger account canonical bytes")
                    .len()
                    > MUSUBI_MAX_ACCOUNT_ID_CANONICAL_BYTES_V1,
                "selected account must be the largest legal member prefix"
            );
            return account;
        }
    }
    panic!("at least one multisig member must fit the Musubi account bound");
}
fn snapshot() -> MusubiRegistrySnapshotV1 {
    MusubiRegistrySnapshotV1 {
        finalized_height: 42,
        finalized_block_hash: [0x42; 32],
        index_revision: 3,
    }
}
const PUBLICATION_FIXTURE_PLAN_PAYLOAD: &[u8] = b"canonical publication source payload";
const PUBLICATION_FIXTURE_CAR_BODY: &[u8] = b"canonical publication CAR body";
fn publication_fixture_car_plan() -> CarBuildPlan {
    publication_fixture_car_plan_with_source(PUBLICATION_FIXTURE_PLAN_PAYLOAD)
}
fn publication_fixture_car_plan_with_source(source: &[u8]) -> CarBuildPlan {
    publication_fixture_car_plan_and_payload_with_source(source).0
}
fn publication_fixture_car_plan_and_payload_with_source(source: &[u8]) -> (CarBuildPlan, Vec<u8>) {
    let entries = [
        sorafs_car::FileEntry {
            path: vec!["src".to_owned(), "lib.ko".to_owned()],
            data: source.to_vec(),
        },
        sorafs_car::FileEntry {
            path: vec![".musubi".to_owned(), "semantic-release.norito".to_owned()],
            data: b"semantic release".to_vec(),
        },
        sorafs_car::FileEntry {
            path: vec![
                ".musubi".to_owned(),
                "artifact-descriptor.norito".to_owned(),
            ],
            data: b"artifact descriptor".to_vec(),
        },
        sorafs_car::FileEntry {
            path: vec![".musubi".to_owned(), "verification-lock.norito".to_owned()],
            data: b"verification lock".to_vec(),
        },
    ];
    CarBuildPlan::from_files(entries.into_iter().collect()).expect("fixture CAR plan")
}
fn publication_fixture_canonical_car() -> (CarBuildPlan, Vec<u8>, MusubiArchiveCommitmentV1) {
    let (plan, payload) =
        publication_fixture_car_plan_and_payload_with_source(PUBLICATION_FIXTURE_PLAN_PAYLOAD);
    let mut car = Vec::new();
    let stats = sorafs_car::CarWriter::new(&plan, &payload)
        .expect("fixture CAR writer")
        .write_to(&mut car)
        .expect("canonical fixture CAR");
    let descriptor = sorafs_car::chunker_registry::default_descriptor();
    assert_eq!(descriptor.profile, plan.chunk_profile);
    let commitment = MusubiArchiveCommitmentV1 {
        root_cid: ManifestRootCid::try_from(
            stats.root_cids.first().expect("fixture CAR root").clone(),
        )
        .expect("canonical fixture root CID"),
        chunker: ChunkerProfileHandle {
            profile_id: descriptor.id.0,
            namespace: descriptor.namespace.to_owned(),
            name: descriptor.name.to_owned(),
            semver: descriptor.semver.to_owned(),
            multihash_code: descriptor.multihash_code,
        },
        chunk_plan_digest: MusubiContentDigestV1::new(sorafs_car::compute_chunk_plan_digest_sha3(
            &plan.chunks,
        )),
        por_root: MusubiContentDigestV1::new(
            sorafs_car::compute_por_root(&payload, &plan).expect("fixture PoR root"),
        ),
        content_length: plan.content_length,
        car_digest: MusubiContentDigestV1::new(*stats.car_archive_digest.as_bytes()),
        car_size: stats.car_size,
        bundle_digest: MusubiContentDigestV1::new([5; 32]),
        source_tree_digest: MusubiContentDigestV1::new([6; 32]),
        descriptor_digest: MusubiContentDigestV1::new([7; 32]),
        file_count: u32::try_from(
            plan.files
                .len()
                .checked_sub(3)
                .expect("fixture contains the mandatory bundle entries"),
        )
        .expect("fixture source file count fits u32"),
        chunk_count: u32::try_from(plan.chunks.len()).expect("fixture chunk count fits u32"),
    };
    commitment.validate().expect("fixture archive commitment");
    (plan, car, commitment)
}
fn publication_fixture_commitment_for_car(car: &[u8]) -> MusubiArchiveCommitmentV1 {
    publication_fixture_commitment_for_plan(car, &publication_fixture_car_plan())
}
fn publication_fixture_commitment_for_plan(
    car: &[u8],
    plan: &CarBuildPlan,
) -> MusubiArchiveCommitmentV1 {
    let descriptor = sorafs_car::chunker_registry::default_descriptor();
    assert_eq!(descriptor.profile, plan.chunk_profile);
    MusubiArchiveCommitmentV1 {
        root_cid: ManifestRootCid::from_blake3_digest([1; 32]).expect("root CID"),
        chunker: ChunkerProfileHandle {
            profile_id: descriptor.id.0,
            namespace: descriptor.namespace.to_owned(),
            name: descriptor.name.to_owned(),
            semver: descriptor.semver.to_owned(),
            multihash_code: descriptor.multihash_code,
        },
        chunk_plan_digest: MusubiContentDigestV1::new(sorafs_car::compute_chunk_plan_digest_sha3(
            &plan.chunks,
        )),
        por_root: MusubiContentDigestV1::new([3; 32]),
        content_length: plan.content_length,
        car_digest: MusubiContentDigestV1::new(*blake3::hash(car).as_bytes()),
        car_size: u64::try_from(car.len()).expect("fixture CAR length fits u64"),
        bundle_digest: MusubiContentDigestV1::new([5; 32]),
        source_tree_digest: MusubiContentDigestV1::new([6; 32]),
        descriptor_digest: MusubiContentDigestV1::new([7; 32]),
        file_count: u32::try_from(
            plan.files
                .len()
                .checked_sub(3)
                .expect("fixture contains the mandatory bundle entries"),
        )
        .expect("fixture source file count fits u32"),
        chunk_count: u32::try_from(plan.chunks.len()).expect("fixture chunk count fits u32"),
    }
}
fn archive_commitment() -> MusubiArchiveCommitmentV1 {
    publication_fixture_commitment_for_car(PUBLICATION_FIXTURE_CAR_BODY)
}
fn request() -> (PublicationRequestV1, KeyPair) {
    let commitment = archive_commitment();
    let package = MusubiPackageIdV1::new(
        DataSpaceId::new(7),
        MusubiPackageScopeV1::DataspaceRoot,
        "demo".parse().expect("package"),
    );
    let release = MusubiReleaseIdV1::new(
        package,
        "1.0.0".parse::<MusubiVersionV1>().expect("release version"),
    );
    let lock = MusubiVerificationLockV1 {
        schema: MusubiVerificationLockV1::SCHEMA.to_owned(),
        version: MUSUBI_REGISTRY_VERSION_V1,
        root: release.clone(),
        root_dependencies: Vec::new(),
        nodes: Vec::new(),
    };
    let manifest = MusubiReleaseManifestV1 {
        release,
        edition: MusubiKotodamaEditionV1::V1,
        abi: MusubiAbiBindingV1::new([8; 32]).expect("ABI"),
        dependencies: Vec::new(),
        exports: Vec::new(),
        interface_digest: MusubiContentDigestV1::new([9; 32]),
        metadata: MusubiReleaseMetadataV1::default(),
        archive_id: commitment.archive_id(),
        verification_lock_digest: lock.digest(),
    };
    let (publisher, _) = account(20);
    let (broker, broker_keypair) = account(21);
    (
        PublicationRequestV1 {
            network_id: publication_test_network_id(0x15),
            publisher,
            ingress_broker: broker,
            seed_provider: ProviderId::new([0x16; 32]),
            namespace: MusubiNamespaceV1::new("dex").expect("namespace"),
            publication: MusubiPublicationV1 {
                manifest,
                resolution: MusubiResolutionProofV1 {
                    snapshot: snapshot(),
                    lock,
                },
            },
            archive_commitment: commitment,
            namespace_delegation: None,
            expected_policy_revision: 1,
            expected_governance_revision: None,
            nonce: [0x18; 32],
        },
        broker_keypair,
    )
}
fn request_with_archive_commitment(
    commitment: MusubiArchiveCommitmentV1,
) -> (PublicationRequestV1, KeyPair) {
    let (mut request, broker) = request();
    request.publication.manifest.archive_id = commitment.archive_id();
    request.archive_commitment = commitment;
    request.validate().expect("canonical CAR request");
    (request, broker)
}
fn signed_release_transaction(request: &PublicationRequestV1, nonce: u32) -> SignedTransaction {
    let (publisher, publisher_keypair) = account(20);
    assert_eq!(publisher, request.publisher);
    let mut builder = TransactionBuilder::new(
        request.network_id(),
        request.publisher.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([request.publish_instruction()]);
    builder.set_creation_time(std::time::Duration::from_millis(2_000 + u64::from(nonce)));
    builder.set_nonce(NonZeroU32::new(nonce).expect("release fixture nonce is non-zero"));
    builder.sign(publisher_keypair.private_key())
}
fn maximum_multisig_release_transaction(
    mut request: PublicationRequestV1,
) -> (PublicationRequestV1, SignedTransaction) {
    let signers = (0..MUSUBI_MAX_RELEASE_SIGNATURES_V1)
        .map(|index| {
            KeyPair::try_from_seed(
                vec![u8::try_from(index + 100).expect("fixture seed"); 32],
                Algorithm::Ed25519,
            )
            .expect("multisig fixture key")
        })
        .collect::<Vec<_>>();
    let members = signers
        .iter()
        .map(|signer| {
            MultisigMember::new(signer.public_key().clone(), 1).expect("multisig fixture member")
        })
        .collect::<Vec<_>>();
    request.publisher = AccountId::new_multisig(
        MultisigPolicy::new(
            u16::try_from(MUSUBI_MAX_RELEASE_SIGNATURES_V1).expect("signature maximum fits u16"),
            members,
        )
        .expect("multisig fixture policy"),
    );
    let mut builder = TransactionBuilder::new(
        request.network_id(),
        request.publisher.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([request.publish_instruction()]);
    builder.set_creation_time(std::time::Duration::from_millis(2_000));
    let signed = builder.sign_multisig(signers.iter().map(KeyPair::private_key));
    (request, signed)
}
fn release_preparation_fixture(
    request: &PublicationRequestV1,
    broker: &KeyPair,
) -> (
    PublicationArchiveRegistrationV1,
    PublicationReleasePreparationFloorV1,
) {
    release_preparation_fixture_with_offset(request, broker, 0)
}
fn release_preparation_fixture_with_offset(
    request: &PublicationRequestV1,
    broker: &KeyPair,
    offset: u64,
) -> (
    PublicationArchiveRegistrationV1,
    PublicationReleasePreparationFloorV1,
) {
    let registration = registration(request, broker);
    let replication = replication_checkpoint_with_revision_offset(request, &registration, offset);
    let floor = release_preparation_for_registration(request, &registration, replication);
    (registration, floor)
}
fn release_preparation_for_registration(
    request: &PublicationRequestV1,
    registration: &PublicationArchiveRegistrationV1,
    replication: PublicationReplicationCheckpointV1,
) -> PublicationReleasePreparationFloorV1 {
    let location = replication
        .location(registration)
        .expect("release fixture location");
    let readbacks = location
        .providers
        .iter()
        .take(2)
        .map(|provider| PublicationReadbackEvidenceV1 {
            provider: *provider,
            location_id: location.location_id,
            replication_order: location.replication_order,
            commitment: request.archive_commitment.clone(),
            semantic_release_digest: request.publication.manifest.semantic_digest(),
            verification_lock_digest: request.publication.manifest.verification_lock_digest,
        })
        .collect::<Vec<_>>();
    PublicationReleasePreparationFloorV1::try_new(
        registration.intent.generation,
        replication,
        readbacks,
        request,
        registration,
    )
    .expect("release preparation floor")
}
fn release_ready_journal(request: &PublicationRequestV1, broker: &KeyPair) -> PublicationJournalV1 {
    let operation_id = request.operation_id();
    let (location_registration, floor) = release_preparation_fixture(request, broker);
    let receipt = location_registration
        .intent
        .prepared_page
        .archive
        .staging_receipt
        .clone();
    let archive_intent = registration_intent(operation_id, request, receipt.clone());
    let registered = registered_archive(request, broker, &archive_intent);
    let mut journal = PublicationJournalV1::new(request.clone()).expect("release journal");
    journal.phase = PublicationPhaseV1::ReleaseSubmission;
    journal.validation = Some(validation_evidence(request));
    journal.staging_receipt = Some(receipt);
    journal.archive_registration_attempts = vec![PublicationArchiveRegistrationAttemptV1::new(
        1,
        archive_intent,
    )];
    journal.registered_archive = Some(registered);
    journal.archive_location_attempts = vec![PublicationArchiveLocationAttemptV1 {
        generation: 1,
        intent: location_registration.intent.clone(),
        registration: Some(location_registration),
        terminal: None,
        terminal_floor: None,
    }];
    journal.replication = Some(floor.replication.clone());
    journal.readbacks.clone_from(&floor.readbacks);
    let intent = PublicationReleaseSubmissionIntentV1::try_new(
        operation_id,
        request,
        floor,
        &signed_release_transaction(request, 1),
    )
    .expect("release-ready exact intent");
    journal.release_submission_attempts =
        vec![PublicationReleaseSubmissionAttemptV1::new(1, intent)];
    journal.validate().expect("release-ready journal");
    journal
}
fn release_absence_evidence(
    request: &PublicationRequestV1,
    finalized_height: u64,
    finalized_time_ms: u64,
) -> PublicationReleaseAbsenceEvidenceV1 {
    assert!(finalized_height > 1);
    let index_revision = finalized_height.saturating_sub(68).max(1);
    let snapshot = MusubiRegistrySnapshotV1 {
        finalized_height,
        finalized_block_hash: [0xD1; 32],
        index_revision,
    };
    let retention = MusubiArchiveRetentionDecisionV1 {
        archive_id: request.archive_commitment.archive_id(),
        disposition: MusubiArchiveRetentionDispositionV1::PruneUnreferenced,
        active_releases: 0,
        yanked_releases: 0,
        taken_down_releases: 0,
        storage: Some(MusubiArchiveAvailabilityV1 {
            archive_id: request.archive_commitment.archive_id(),
            availability: MusubiStorageAvailabilityV1::Selectable,
            healthy_replicas: MUSUBI_MIN_HEALTHY_REPLICAS_V1,
            active_locations: 1,
            finalized_height: finalized_height - 1,
            finalized_block_hash: [0xD0; 32],
            index_revision,
        }),
    };
    PublicationReleaseAbsenceEvidenceV1 {
        resolver_page: MusubiResolverIndexPageV1 {
            query: MusubiResolverIndexQueryV1 {
                package: request.publication.manifest.release.package.clone(),
                requirement: Some(
                    format!("={}", request.publication.manifest.release.version)
                        .parse()
                        .expect("exact fixture requirement"),
                ),
                page: MusubiPageRequestV1 {
                    limit: 1,
                    cursor: None,
                },
            },
            network_id: request.network_id(),
            items: Vec::new(),
            next_cursor: None,
            snapshot,
        },
        retention_query: MusubiArchiveRetentionQueryV1 {
            archive_ids: vec![request.archive_commitment.archive_id()],
            expected_snapshot: Some(snapshot),
        },
        retention_page: MusubiArchiveRetentionPageV1 {
            network_id: request.network_id(),
            items: vec![retention],
            snapshot,
            finalized_time_ms,
        },
    }
}
fn validation_evidence(request: &PublicationRequestV1) -> PublicationValidationEvidenceV1 {
    PublicationValidationEvidenceV1 {
        archive_id: request.archive_commitment.archive_id(),
        semantic_release_digest: request.publication.manifest.semantic_digest(),
        release_digest: request.publication.manifest.release_digest(),
        source_tree_digest: request.archive_commitment.source_tree_digest,
        descriptor_digest: request.archive_commitment.descriptor_digest,
        verification_lock_digest: request.publication.manifest.verification_lock_digest,
        car_digest: request.archive_commitment.car_digest,
        car_size: request.archive_commitment.car_size,
        compiler_output_digest: MusubiContentDigestV1::new([0x63; 32]),
        resolution_snapshot: request.publication.resolution.snapshot,
    }
}
fn signed_receipt(
    binding: &MusubiSeedIngressReceiptBindingV1,
    broker: &KeyPair,
) -> MusubiSeedIngressReceiptV1 {
    signed_receipt_at(binding, broker, 1_000, 2_000)
}
fn signed_receipt_at(
    binding: &MusubiSeedIngressReceiptBindingV1,
    broker: &KeyPair,
    issued_at_ms: u64,
    expires_at_ms: u64,
) -> MusubiSeedIngressReceiptV1 {
    let payload = MusubiSeedIngressReceiptPayloadV1 {
        version: MUSUBI_REGISTRY_VERSION_V1,
        binding: binding.clone(),
        issued_at_ms,
        expires_at_ms,
    };
    MusubiSeedIngressReceiptV1 {
        approvals: vec![MusubiSeedIngressReceiptApprovalV1 {
            public_key: broker.public_key().clone(),
            signature: SignatureOf::try_from_hash(broker.private_key(), payload.signing_hash())
                .expect("sign fixture ingress receipt"),
        }],
        payload,
    }
}
fn archive_absence_evidence(
    request: &PublicationRequestV1,
    finalized_height: u64,
) -> PublicationArchiveAbsenceEvidenceV1 {
    PublicationArchiveAbsenceEvidenceV1 {
        network_id: request.network_id,
        snapshot: MusubiRegistrySnapshotV1 {
            finalized_height,
            finalized_block_hash: [0xA5; 32],
            index_revision: finalized_height,
        },
        finalized_time_ms: 1_700_000_000_000,
        decision: MusubiArchiveRetentionDecisionV1 {
            archive_id: request.archive_commitment.archive_id(),
            disposition: MusubiArchiveRetentionDispositionV1::RetainUnknown,
            active_releases: 0,
            yanked_releases: 0,
            taken_down_releases: 0,
            storage: None,
        },
    }
}
fn registration_intent(
    operation_id: PublicationOperationIdV1,
    request: &PublicationRequestV1,
    receipt: MusubiSeedIngressReceiptV1,
) -> PublicationArchiveRegistrationIntentV1 {
    let (publisher, publisher_keypair) = account(20);
    assert_eq!(publisher, request.publisher);
    let mut builder = TransactionBuilder::new(
        request.network_id(),
        request.publisher.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([request.archive_registration_instruction(&receipt)]);
    builder.set_creation_time(std::time::Duration::from_millis(
        receipt.payload.issued_at_ms,
    ));
    let signed_transaction = builder.sign(publisher_keypair.private_key());
    PublicationArchiveRegistrationIntentV1::new(operation_id, request, receipt, signed_transaction)
}
fn registered_archive(
    request: &PublicationRequestV1,
    broker: &KeyPair,
    intent: &PublicationArchiveRegistrationIntentV1,
) -> PublicationRegisteredArchiveV1 {
    let mut archive = archive_record(request, broker);
    archive.staging_receipt = intent.staging_receipt.clone();
    PublicationRegisteredArchiveV1 {
        finalized_transaction_hash: intent.transaction_hash,
        network_id: request.network_id,
        snapshot: MusubiRegistrySnapshotV1 {
            finalized_height: 60,
            finalized_block_hash: [0x3C; 32],
            index_revision: 2,
        },
        archive,
    }
}
fn archive_record(request: &PublicationRequestV1, broker: &KeyPair) -> MusubiArchiveRecordV1 {
    MusubiArchiveRecordV1 {
        archive_id: request.archive_commitment.archive_id(),
        commitment: request.archive_commitment.clone(),
        staging_receipt: signed_receipt(&request.receipt_binding(), broker),
        registered_by: request.publisher.clone(),
        registered_at_height: 50,
        location_revision: 1,
        location_ids: Vec::new(),
    }
}
fn registration(
    request: &PublicationRequestV1,
    broker: &KeyPair,
) -> PublicationArchiveRegistrationV1 {
    let archive = archive_record(request, broker);
    let prepared_page = MusubiArchiveLocationPageV1 {
        network_id: request.network_id(),
        archive: archive.clone(),
        items: Vec::new(),
        next_cursor: None,
        snapshot: MusubiRegistrySnapshotV1 {
            finalized_height: 60,
            finalized_block_hash: [0x3C; 32],
            index_revision: 2,
        },
    };
    let replication_order = ReplicationOrderId::new([0x33; 32]);
    let provider_attestations = (1..=3)
        .map(|provider| provider_attestation(request, replication_order, provider))
        .collect::<Vec<_>>();
    let provider_attestation_set_digest = provider_attestation_set_digest(
        request.archive_commitment.archive_id(),
        replication_order,
        &provider_attestations,
    );
    let instruction = AddMusubiArchiveLocationV1 {
        archive_id: request.archive_commitment.archive_id(),
        location_id: MusubiArchiveLocationIdV1::new([0x31; 32]),
        pin_manifest: ManifestDigest::new([0x32; 32]),
        replication_order,
        provider_attestation_set_digest,
        renew_after_epoch: 10,
        expires_at_epoch: 20,
        expected_location_revision: archive.location_revision,
    };
    let (_, publisher_keypair) = account(20);
    let mut builder = TransactionBuilder::new(
        request.network_id(),
        request.publisher.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction.clone()]);
    builder.set_creation_time(std::time::Duration::from_millis(1_000));
    let intent = PublicationArchiveLocationIntentV1::new(
        request.operation_id(),
        1,
        prepared_page,
        instruction,
        builder.sign(publisher_keypair.private_key()),
    );
    let location = MusubiArchiveLocationV1 {
        location_id: intent.location_id,
        archive_id: request.archive_commitment.archive_id(),
        pin_manifest: intent.pin_manifest,
        replication_order: intent.replication_order,
        providers: provider_attestations
            .iter()
            .map(|attestation| attestation.payload.binding.provider_id)
            .collect(),
        provider_attestation_set_digest,
        renew_after_epoch: intent.renew_after_epoch,
        expires_at_epoch: intent.expires_at_epoch,
        finalized_height: 70,
        revision: 2,
        state: MusubiArchiveLocationStateV1::Healthy,
    };
    let mut finalized_archive = archive;
    finalized_archive.location_revision = 2;
    finalized_archive.location_ids = vec![intent.location_id];
    PublicationArchiveRegistrationV1 {
        intent,
        applied_height: 70,
        finalized_page: MusubiArchiveLocationPageV1 {
            network_id: request.network_id(),
            archive: finalized_archive,
            items: vec![location],
            next_cursor: None,
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height: 70,
                finalized_block_hash: [0x46; 32],
                index_revision: 3,
            },
        },
    }
}
fn location_registration_generation(
    operation_id: PublicationOperationIdV1,
    request: &PublicationRequestV1,
    registered: &PublicationRegisteredArchiveV1,
    generation: u8,
) -> PublicationArchiveRegistrationV1 {
    assert!(generation > 0);
    let generation_u64 = u64::from(generation);
    let completed_generations = generation_u64 - 1;
    let prepared_height = registered.snapshot.finalized_height + completed_generations * 2;
    let prepared_revision = registered.archive.location_revision + completed_generations * 2;
    let prepared_snapshot = if generation == 1 {
        registered.snapshot
    } else {
        MusubiRegistrySnapshotV1 {
            finalized_height: prepared_height,
            finalized_block_hash: [0x6F_u8.saturating_add(generation); 32],
            index_revision: registered.snapshot.index_revision + completed_generations * 2,
        }
    };
    let mut prepared_archive = registered.archive.clone();
    prepared_archive.location_revision = prepared_revision;
    prepared_archive.location_ids.clear();
    let prepared_page = MusubiArchiveLocationPageV1 {
        network_id: request.network_id(),
        archive: prepared_archive,
        items: Vec::new(),
        next_cursor: None,
        snapshot: prepared_snapshot,
    };
    let replication_order = ReplicationOrderId::new([0x40_u8.saturating_add(generation); 32]);
    let provider_attestations = (1..=3)
        .map(|provider| provider_attestation(request, replication_order, provider))
        .collect::<Vec<_>>();
    let provider_attestation_set_digest = provider_attestation_set_digest(
        request.archive_commitment.archive_id(),
        replication_order,
        &provider_attestations,
    );
    let instruction = AddMusubiArchiveLocationV1 {
        archive_id: request.archive_commitment.archive_id(),
        location_id: MusubiArchiveLocationIdV1::new([0x30_u8.saturating_add(generation); 32]),
        pin_manifest: ManifestDigest::new([0x50_u8.saturating_add(generation); 32]),
        replication_order,
        provider_attestation_set_digest,
        renew_after_epoch: 20 + generation_u64,
        expires_at_epoch: 40 + generation_u64,
        expected_location_revision: prepared_revision,
    };
    let (_, publisher_keypair) = account(20);
    let mut builder = TransactionBuilder::new(
        request.network_id(),
        request.publisher.clone(),
        FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([instruction.clone()]);
    builder.set_creation_time(std::time::Duration::from_millis(1_000 + generation_u64));
    let intent = PublicationArchiveLocationIntentV1::new(
        operation_id,
        generation,
        prepared_page,
        instruction,
        builder.sign(publisher_keypair.private_key()),
    );
    finalized_location_registration(request, &intent)
}
fn finalized_location_registration(
    request: &PublicationRequestV1,
    intent: &PublicationArchiveLocationIntentV1,
) -> PublicationArchiveRegistrationV1 {
    let finalized_height = intent.prepared_page.snapshot.finalized_height + 1;
    let finalized_revision = intent.expected_location_revision + 1;
    let provider_attestations = (1..=3)
        .map(|provider| provider_attestation(request, intent.replication_order, provider))
        .collect::<Vec<_>>();
    let providers = provider_attestations
        .iter()
        .map(|attestation| attestation.payload.binding.provider_id)
        .collect::<Vec<_>>();
    let location = MusubiArchiveLocationV1 {
        location_id: intent.location_id,
        archive_id: request.archive_commitment.archive_id(),
        pin_manifest: intent.pin_manifest,
        replication_order: intent.replication_order,
        providers,
        provider_attestation_set_digest: intent.provider_attestation_set_digest,
        renew_after_epoch: intent.renew_after_epoch,
        expires_at_epoch: intent.expires_at_epoch,
        finalized_height,
        revision: finalized_revision,
        state: MusubiArchiveLocationStateV1::Healthy,
    };
    let mut finalized_archive = intent.prepared_page.archive.clone();
    finalized_archive.location_revision = finalized_revision;
    finalized_archive.location_ids = vec![intent.location_id];
    PublicationArchiveRegistrationV1 {
        intent: intent.clone(),
        applied_height: finalized_height,
        finalized_page: MusubiArchiveLocationPageV1 {
            network_id: request.network_id(),
            archive: finalized_archive,
            items: vec![location],
            next_cursor: None,
            snapshot: MusubiRegistrySnapshotV1 {
                finalized_height,
                finalized_block_hash: [0x60_u8.saturating_add(intent.generation); 32],
                index_revision: intent.prepared_page.snapshot.index_revision + 1,
            },
        },
    }
}
fn retired_location_terminal(
    registration: &PublicationArchiveRegistrationV1,
) -> PublicationArchiveLocationTerminalV1 {
    retired_location_terminal_with_revision_offset(registration, 0)
}
fn retired_location_terminal_with_revision_offset(
    registration: &PublicationArchiveRegistrationV1,
    offset: u64,
) -> PublicationArchiveLocationTerminalV1 {
    let mut finalized_page = registration.finalized_page.clone();
    finalized_page.archive.location_revision += 1 + offset;
    finalized_page.archive.location_ids.clear();
    finalized_page.items.clear();
    finalized_page.snapshot.finalized_height += 1 + offset;
    finalized_page.snapshot.finalized_block_hash = [0x70_u8
        .saturating_add(registration.intent.generation)
        .saturating_add(u8::try_from(offset).unwrap_or(u8::MAX));
        32];
    finalized_page.snapshot.index_revision += 1 + offset;
    PublicationArchiveLocationTerminalV1 {
        transaction_hash: registration.intent.transaction_hash,
        reason: PublicationArchiveLocationTerminalReasonV1::Retired,
        finalized_page,
    }
}
fn provider_attestation(
    request: &PublicationRequestV1,
    replication_order: ReplicationOrderId,
    provider_byte: u8,
) -> MusubiProviderBundleVerificationAttestationV1 {
    let (owner, keypair) = account(provider_byte.saturating_add(60));
    let binding = MusubiProviderBundleVerificationBindingV1 {
        network_id: request.network_id(),
        provider_id: ProviderId::new([provider_byte; 32]),
        completed_by: owner.clone(),
        completion_authority: ProviderIngestCompletionAuthorityV1::new(
            owner,
            ProviderIngestCompletionSignerPolicyV1 {
                policy_id: [provider_byte.saturating_add(20); 32],
                revision: 1,
                predecessor_digest: None,
                policy_digest: [provider_byte.saturating_add(30); 32],
            },
        ),
        replication_order,
        assignment_revision: 1,
        completion_epoch: 12,
        finalized_anchor: ProviderIngestFinalizedAnchorV1 {
            height: 60,
            block_hash: [provider_byte.saturating_add(40); 32],
        },
        archive_id: request.archive_commitment.archive_id(),
        bundle_digest: request.archive_commitment.bundle_digest,
        descriptor_digest: request.archive_commitment.descriptor_digest,
        semantic_release_manifest_digest: request.publication.manifest.semantic_digest(),
        verification_lock_digest: request.publication.manifest.verification_lock_digest,
        source_tree_digest: request.archive_commitment.source_tree_digest,
    };
    let payload = MusubiProviderBundleVerificationPayloadV1 {
        version: MUSUBI_REGISTRY_VERSION_V1,
        binding,
    };
    MusubiProviderBundleVerificationAttestationV1 {
        approvals: vec![MusubiProviderBundleVerificationApprovalV1 {
            public_key: keypair.public_key().clone(),
            signature: SignatureOf::try_from_hash(keypair.private_key(), payload.signing_hash())
                .expect("sign provider fixture attestation"),
        }],
        payload,
    }
}
fn provider_attestation_set_digest(
    archive_id: ArchiveId,
    replication_order: ReplicationOrderId,
    attestations: &[MusubiProviderBundleVerificationAttestationV1],
) -> MusubiProviderBundleAttestationSetDigestV1 {
    let references = attestations
        .iter()
        .map(MusubiProviderBundleVerificationAttestationV1::reference)
        .collect::<Vec<_>>();
    musubi_provider_bundle_attestation_set_digest_v1(archive_id, replication_order, &references)
        .expect("canonical provider attestation set")
}
fn location(
    request: &PublicationRequestV1,
    registration: &PublicationArchiveRegistrationV1,
    provider_count: u8,
) -> MusubiArchiveLocationV1 {
    let registered_location = registration
        .location()
        .expect("registered fixture location");
    let attestations = (1..=provider_count)
        .map(|provider| {
            provider_attestation(request, registration.intent.replication_order, provider)
        })
        .collect::<Vec<_>>();
    let provider_attestation_set_digest = provider_attestation_set_digest(
        request.archive_commitment.archive_id(),
        registration.intent.replication_order,
        &attestations,
    );
    MusubiArchiveLocationV1 {
        location_id: registration.location_id(),
        archive_id: request.archive_commitment.archive_id(),
        pin_manifest: registration.intent.pin_manifest,
        replication_order: registration.intent.replication_order,
        providers: attestations
            .iter()
            .map(|attestation| attestation.payload.binding.provider_id)
            .collect(),
        provider_attestation_set_digest,
        renew_after_epoch: registration.intent.renew_after_epoch,
        expires_at_epoch: registration.intent.expires_at_epoch,
        finalized_height: registered_location.finalized_height,
        revision: registered_location.revision,
        state: MusubiArchiveLocationStateV1::Healthy,
    }
}
fn replication_checkpoint(
    request: &PublicationRequestV1,
    registration: &PublicationArchiveRegistrationV1,
    provider_count: u8,
) -> PublicationReplicationCheckpointV1 {
    let mut finalized_page = registration.finalized_page.clone();
    let index = finalized_page
        .items
        .binary_search_by_key(&registration.location_id(), |location| location.location_id)
        .expect("registered fixture location is present");
    finalized_page.items[index] = location(request, registration, provider_count);
    PublicationReplicationCheckpointV1 { finalized_page }
}
fn replication_checkpoint_with_revision_offset(
    request: &PublicationRequestV1,
    registration: &PublicationArchiveRegistrationV1,
    offset: u64,
) -> PublicationReplicationCheckpointV1 {
    let mut checkpoint = replication_checkpoint(request, registration, 3);
    if offset == 0 {
        return checkpoint;
    }
    let location = checkpoint
        .finalized_page
        .items
        .iter_mut()
        .find(|location| location.location_id == registration.location_id())
        .expect("registered fixture location is present");
    location.revision += offset;
    location.finalized_height += offset;
    checkpoint.finalized_page.archive.location_revision += offset;
    checkpoint.finalized_page.snapshot.finalized_height += offset;
    checkpoint.finalized_page.snapshot.finalized_block_hash =
        [0x80_u8.saturating_add(u8::try_from(offset).unwrap_or(u8::MAX)); 32];
    checkpoint.finalized_page.snapshot.index_revision += offset;
    checkpoint
}
fn replication_checkpoint_with_journal_max_shape(
    request: &PublicationRequestV1,
    registration: &PublicationArchiveRegistrationV1,
    offset: u64,
) -> PublicationReplicationCheckpointV1 {
    let provider_count = u8::try_from(MUSUBI_MAX_LOCATION_PROVIDERS_V1)
        .expect("provider maximum fits the fixture counter");
    let mut checkpoint = replication_checkpoint(request, registration, provider_count);
    let target = checkpoint
        .finalized_page
        .items
        .iter_mut()
        .find(|location| location.location_id == registration.location_id())
        .expect("registered fixture location is present");
    target.revision = registration
        .location()
        .expect("registered fixture location")
        .revision
        + 1
        + offset;
    target.finalized_height = registration.finalized_page.snapshot.finalized_height + 1 + offset;
    let target = target.clone();
    checkpoint.finalized_page.archive.location_revision = target.revision + 3;
    checkpoint.finalized_page.snapshot = MusubiRegistrySnapshotV1 {
        finalized_height: target.finalized_height + 3,
        finalized_block_hash: [0xA0_u8.saturating_add(u8::try_from(offset).unwrap_or(u8::MAX)); 32],
        index_revision: registration.finalized_page.snapshot.index_revision + 4 + offset,
    };
    for index in 1..MUSUBI_MAX_ARCHIVE_LOCATIONS_V1 {
        let mut location = target.clone();
        let index_u8 = u8::try_from(index).expect("location maximum fits u8");
        location.location_id =
            MusubiArchiveLocationIdV1::new([0xA0_u8.saturating_add(index_u8); 32]);
        location.revision = target.revision + u64::from(index_u8);
        location.finalized_height = target.finalized_height + u64::from(index_u8);
        checkpoint
            .finalized_page
            .archive
            .location_ids
            .push(location.location_id);
        checkpoint.finalized_page.items.push(location);
    }
    checkpoint
}
fn replication_checkpoint_with_directory_advance(
    request: &PublicationRequestV1,
    registration: &PublicationArchiveRegistrationV1,
) -> PublicationReplicationCheckpointV1 {
    let mut checkpoint = replication_checkpoint(request, registration, 3);
    checkpoint.finalized_page.archive.location_revision += 1;
    checkpoint.finalized_page.snapshot.finalized_height += 1;
    checkpoint.finalized_page.snapshot.finalized_block_hash = [0x91; 32];
    let mut unrelated = checkpoint
        .location(registration)
        .expect("registered fixture location")
        .clone();
    unrelated.location_id = MusubiArchiveLocationIdV1::new([0xF0; 32]);
    unrelated.revision = checkpoint.finalized_page.archive.location_revision;
    unrelated.finalized_height = checkpoint.finalized_page.snapshot.finalized_height;
    checkpoint
        .finalized_page
        .archive
        .location_ids
        .push(unrelated.location_id);
    checkpoint.finalized_page.items.push(unrelated);
    checkpoint
}
fn final_evidence(request: &PublicationRequestV1) -> PublicationFinalEvidenceV1 {
    let snapshot = MusubiRegistrySnapshotV1 {
        finalized_height: 100,
        finalized_block_hash: [0x64; 32],
        index_revision: 4,
    };
    let yank = MusubiReleaseYankV1 {
        release: request.publication.manifest.release.clone(),
        yanked: false,
        reason: MusubiReasonV1::new("initial publication").expect("reason"),
        changed_by: request.publisher.clone(),
        changed_at_height: 80,
        revision: 1,
    };
    let governance = MusubiArtifactGovernanceStateV1::Available;
    let home_release = MusubiReleaseRecordV1 {
        manifest: request.publication.manifest.clone(),
        release_digest: request.publication.manifest.release_digest(),
        published_by: request.publisher.clone(),
        published_at_height: 80,
        yank: yank.clone(),
        artifact_governance: governance.clone(),
        revisions: MusubiReleaseRevisionsV1 {
            yank: 1,
            artifact_governance: 1,
        },
    };
    let universal_release = MusubiResolverReleaseRowV1 {
        release: request.publication.manifest.release.clone(),
        release_digest: request.publication.manifest.release_digest(),
        archive_id: request.archive_commitment.archive_id(),
        source_digest: request.archive_commitment.source_tree_digest,
        interface_digest: request.publication.manifest.interface_digest,
        abi: request.publication.manifest.abi,
        dependencies: request.publication.manifest.dependencies.clone(),
        selection: MusubiReleaseSelectionStateV1 {
            yank,
            storage: MusubiArchiveAvailabilityV1 {
                archive_id: request.archive_commitment.archive_id(),
                availability: MusubiStorageAvailabilityV1::Selectable,
                healthy_replicas: 3,
                active_locations: 1,
                finalized_height: 70,
                finalized_block_hash: [0x46; 32],
                index_revision: snapshot.index_revision,
            },
            governance,
        },
        index_revision: snapshot.index_revision,
    };
    PublicationFinalEvidenceV1 {
        network_id: request.network_id,
        snapshot,
        home_release,
        universal_release,
    }
}
