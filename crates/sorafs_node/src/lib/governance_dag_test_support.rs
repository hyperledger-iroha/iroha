#[derive(Debug)]
struct TestGovernanceDagSigner {
    handle: String,
    publisher_peer_id: Vec<u8>,
    key_pair: KeyPair,
    qualification_refuse: AtomicBool,
    qualification_reads: AtomicU64,
    drift_on_second_qualification_read: AtomicBool,
}
impl TestGovernanceDagSigner {
    fn new() -> Self {
        Self {
            handle: "provider:governance-dag:node-primary".to_owned(),
            publisher_peer_id: b"12D3KooWNodeTestGovernancePublisher".to_vec(),
            key_pair: KeyPair::try_from_seed(vec![0x39; 32], Algorithm::Ed25519)
                .expect("derive test Governance DAG key"),
            qualification_refuse: AtomicBool::new(false),
            qualification_reads: AtomicU64::new(0),
            drift_on_second_qualification_read: AtomicBool::new(false),
        }
    }
    fn public_key_bytes(&self) -> [u8; 32] {
        let (algorithm, bytes) = self
            .key_pair
            .public_key()
            .try_to_bytes()
            .expect("serialize test Governance DAG public key");
        assert_eq!(algorithm, Algorithm::Ed25519);
        bytes.try_into().expect("Ed25519 public key width")
    }
    fn expected_qualification() -> GovernanceDagRuntimeProviderQualificationV1 {
        GovernanceDagRuntimeProviderQualificationV1::new(1, [0x84; 32])
    }
}
impl GovernanceDagRuntimeSigner for TestGovernanceDagSigner {
    fn handle(&self) -> &str {
        &self.handle
    }
    fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
        if self.qualification_refuse.load(Ordering::SeqCst) {
            return Err("hsm_credential=must-never-escape".to_owned());
        }
        let read_index = self.qualification_reads.fetch_add(1, Ordering::SeqCst);
        if self
            .drift_on_second_qualification_read
            .load(Ordering::SeqCst)
            && read_index == 1
        {
            return Ok(GovernanceDagRuntimeProviderQualificationV1::new(
                2, [0x84; 32],
            ));
        }
        Ok(Self::expected_qualification())
    }
    fn publisher_peer_id(&self) -> &[u8] {
        &self.publisher_peer_id
    }
    fn public_key(&self) -> [u8; 32] {
        self.public_key_bytes()
    }
    fn sign(
        &self,
        _purpose: crate::GovernanceDagSigningPurposeV1,
        payload: &[u8],
    ) -> Result<[u8; 64], String> {
        IrohaSignature::try_new(self.key_pair.private_key(), payload)
            .map_err(|_| "test Governance DAG signer refused request".to_owned())?
            .payload()
            .try_into()
            .map_err(|_| "test Governance DAG signature width changed".to_owned())
    }
}
#[derive(Debug)]
struct TestGovernanceDagCheckpointStoreState {
    records: [Option<GovernanceDagSealedStateRecord>; 6],
    generation_floors: [u64; 6],
}
impl Default for TestGovernanceDagCheckpointStoreState {
    fn default() -> Self {
        Self {
            records: std::array::from_fn(|_| None),
            generation_floors: [0; 6],
        }
    }
}
#[derive(Debug, Default)]
struct TestGovernanceDagCheckpointStore {
    state: Mutex<TestGovernanceDagCheckpointStoreState>,
    qualification_refuse: AtomicBool,
    fail_after_next_checkpoint_cas: AtomicBool,
}
impl TestGovernanceDagCheckpointStore {
    const HANDLE: &'static str = "kms:governance-dag:node-producer-checkpoint";
    const fn expected_qualification() -> GovernanceDagRuntimeProviderQualificationV1 {
        GovernanceDagRuntimeProviderQualificationV1::new(1, [0x94; 32])
    }
    const fn slot_index(slot: GovernanceDagSealedStateSlot) -> usize {
        match slot {
            GovernanceDagSealedStateSlot::Checkpoint => 0,
            GovernanceDagSealedStateSlot::PublishIntent => 1,
            GovernanceDagSealedStateSlot::ProducerCheckpoint => 2,
            GovernanceDagSealedStateSlot::ProducerPublishIntent => 3,
            GovernanceDagSealedStateSlot::IpfsRequestReplay => 4,
            GovernanceDagSealedStateSlot::SignedHeadRequestReplay => 5,
        }
    }
}
impl GovernanceDagSealedCheckpointStore for TestGovernanceDagCheckpointStore {
    fn handle(&self) -> &str {
        Self::HANDLE
    }
    fn qualification(&self) -> Result<GovernanceDagRuntimeProviderQualificationV1, String> {
        if self.qualification_refuse.load(Ordering::SeqCst) {
            return Err("checkpoint_credential=must-never-escape".to_owned());
        }
        Ok(Self::expected_qualification())
    }
    fn load(
        &self,
        slot: GovernanceDagSealedStateSlot,
    ) -> Result<Option<GovernanceDagSealedStateRecord>, String> {
        let state = self.state.lock().map_err(|_| "poisoned".to_owned())?;
        Ok(state.records[Self::slot_index(slot)].clone())
    }
    fn compare_and_swap(
        &self,
        slot: GovernanceDagSealedStateSlot,
        expected_revision: Option<[u8; 32]>,
        next: GovernanceDagSealedStateRecord,
    ) -> Result<(), String> {
        let index = Self::slot_index(slot);
        let mut state = self.state.lock().map_err(|_| "poisoned".to_owned())?;
        if state.records[index].as_ref().map(|record| record.revision) != expected_revision {
            return Err("compare-and-swap conflict".to_owned());
        }
        if next.generation <= state.generation_floors[index]
            || next.payload.is_empty()
            || !next.has_valid_revision(slot)
        {
            return Err("invalid or non-monotonic record".to_owned());
        }
        state.generation_floors[index] = next.generation;
        state.records[index] = Some(next);
        drop(state);
        if slot == GovernanceDagSealedStateSlot::ProducerCheckpoint
            && self
                .fail_after_next_checkpoint_cas
                .swap(false, Ordering::SeqCst)
        {
            return Err("ambiguous checkpoint CAS response".to_owned());
        }
        Ok(())
    }
    fn delete(
        &self,
        slot: GovernanceDagSealedStateSlot,
        expected_revision: [u8; 32],
    ) -> Result<(), String> {
        let index = Self::slot_index(slot);
        let mut state = self.state.lock().map_err(|_| "poisoned".to_owned())?;
        if state.records[index].as_ref().map(|record| record.revision) != Some(expected_revision) {
            return Err("delete conflict".to_owned());
        }
        state.records[index] = None;
        Ok(())
    }
}
#[derive(Debug)]
struct FailingRepairOrchestrator {
    calls: Arc<AtomicUsize>,
}
impl RepairOrchestrator for FailingRepairOrchestrator {
    fn rehydrate_missing_chunks(
        &self,
        _context: &native_repair_worker::NativeRepairExecutionContextV1,
        _manifest: &StoredManifest,
        _missing_chunks: &[ChunkFileRecord],
    ) -> Result<Vec<RepairChunkPayload>, RepairOrchestratorError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
        Err(RepairOrchestratorError::other(
            "simulated transient remote provider outage",
        ))
    }
}
#[derive(Debug, Default)]
struct RecordingPublisher {
    payloads: Mutex<Vec<Vec<u8>>>,
}
impl RecordingPublisher {
    fn take(&self) -> Vec<Vec<u8>> {
        let mut guard = self.payloads.lock().expect("publisher lock poisoned");
        guard.drain(..).collect()
    }
}
impl GovernancePublisher for RecordingPublisher {
    fn publish_deal_settlement(
        &self,
        _settlement: &DealSettlementV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.payloads.lock().expect("publisher lock poisoned");
        guard.push(encoded.to_vec());
        Ok(())
    }
    fn publish_pdp_archive(
        &self,
        _archive: &PdpGovernanceArchiveV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.payloads.lock().expect("publisher lock poisoned");
        guard.push(encoded.to_vec());
        Ok(())
    }
    fn publish_por_challenge_publication(
        &self,
        _publication: &PorChallengePublicationV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.payloads.lock().expect("publisher lock poisoned");
        guard.push(encoded.to_vec());
        Ok(())
    }
    fn publish_por_weekly_report(
        &self,
        _report: &PorWeeklyReportV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.payloads.lock().expect("publisher lock poisoned");
        guard.push(encoded.to_vec());
        Ok(())
    }
    fn publish_gc_audit_event(
        &self,
        _event: &GcAuditEventV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.payloads.lock().expect("publisher lock poisoned");
        guard.push(encoded.to_vec());
        Ok(())
    }
    fn publish_reconciliation_report(
        &self,
        _report: &SorafsReconciliationReportV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.payloads.lock().expect("publisher lock poisoned");
        guard.push(encoded.to_vec());
        Ok(())
    }
    fn publish_reputation_snapshot(
        &self,
        _snapshot: &SignedReputationSnapshotV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.payloads.lock().expect("publisher lock poisoned");
        guard.push(encoded.to_vec());
        Ok(())
    }
    fn publish_moderation_ballot_event(
        &self,
        _event: &SoraFsModerationBallotGovernanceEventV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.payloads.lock().expect("publisher lock poisoned");
        guard.push(encoded.to_vec());
        Ok(())
    }
    fn publish_transparency_ledger_publication(
        &self,
        _publication: &ModerationLedgerCyclePublicationV1,
        encoded: &[u8],
        _authorization: Option<&PrivacyPublicationAuthorizationV1>,
        _provenance: Option<&GovernanceSubmissionProvenanceV1>,
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.payloads.lock().expect("publisher lock poisoned");
        guard.push(encoded.to_vec());
        Ok(())
    }
    fn publish_proof_token_issuance(
        &self,
        _issuance: &ProofTokenIssuanceV1,
        encoded: &[u8],
        _provenance: Option<&GovernanceSubmissionProvenanceV1>,
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.payloads.lock().expect("publisher lock poisoned");
        guard.push(encoded.to_vec());
        Ok(())
    }
    fn publish_appeal_finance_report(
        &self,
        _report: &SoraFsAppealFinanceReportV1,
        encoded: &[u8],
        _provenance: &GovernanceSubmissionProvenanceV1,
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.payloads.lock().expect("publisher lock poisoned");
        guard.push(encoded.to_vec());
        Ok(())
    }
    fn publish_appeal_finance_weekly_rollup(
        &self,
        _rollup: &SoraFsAppealFinanceWeeklyRollupV1,
        encoded: &[u8],
        _provenance: &GovernanceSubmissionProvenanceV1,
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.payloads.lock().expect("publisher lock poisoned");
        guard.push(encoded.to_vec());
        Ok(())
    }
    fn publish_appeal_finance_settlement_receipt(
        &self,
        _receipt: &SoraFsAppealFinanceSettlementReceiptV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.payloads.lock().expect("publisher lock poisoned");
        guard.push(encoded.to_vec());
        Ok(())
    }
}
#[derive(Debug, Default)]
struct FailingPublisher {
    attempts: Mutex<usize>,
}
impl FailingPublisher {
    fn attempts(&self) -> usize {
        *self.attempts.lock().expect("publisher lock poisoned")
    }
}
impl GovernancePublisher for FailingPublisher {
    fn publish_deal_settlement(
        &self,
        _settlement: &DealSettlementV1,
        _encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.attempts.lock().expect("publisher lock poisoned");
        *guard += 1;
        Err(GovernancePublishError::other("simulated publish failure"))
    }
    fn publish_pdp_archive(
        &self,
        _archive: &PdpGovernanceArchiveV1,
        _encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.attempts.lock().expect("publisher lock poisoned");
        *guard += 1;
        Err(GovernancePublishError::other("simulated publish failure"))
    }
    fn publish_por_challenge_publication(
        &self,
        _publication: &PorChallengePublicationV1,
        _encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.attempts.lock().expect("publisher lock poisoned");
        *guard += 1;
        Err(GovernancePublishError::other("simulated publish failure"))
    }
    fn publish_por_weekly_report(
        &self,
        _report: &PorWeeklyReportV1,
        _encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.attempts.lock().expect("publisher lock poisoned");
        *guard += 1;
        Err(GovernancePublishError::other("simulated publish failure"))
    }
    fn publish_gc_audit_event(
        &self,
        _event: &GcAuditEventV1,
        _encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.attempts.lock().expect("publisher lock poisoned");
        *guard += 1;
        Err(GovernancePublishError::other("simulated publish failure"))
    }
    fn publish_reconciliation_report(
        &self,
        _report: &SorafsReconciliationReportV1,
        _encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.attempts.lock().expect("publisher lock poisoned");
        *guard += 1;
        Err(GovernancePublishError::other("simulated publish failure"))
    }
    fn publish_reputation_snapshot(
        &self,
        _snapshot: &SignedReputationSnapshotV1,
        _encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.attempts.lock().expect("publisher lock poisoned");
        *guard += 1;
        Err(GovernancePublishError::other("simulated publish failure"))
    }
    fn publish_moderation_ballot_event(
        &self,
        _event: &SoraFsModerationBallotGovernanceEventV1,
        _encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.attempts.lock().expect("publisher lock poisoned");
        *guard += 1;
        Err(GovernancePublishError::other("simulated publish failure"))
    }
    fn publish_transparency_ledger_publication(
        &self,
        _publication: &ModerationLedgerCyclePublicationV1,
        _encoded: &[u8],
        _authorization: Option<&PrivacyPublicationAuthorizationV1>,
        _provenance: Option<&GovernanceSubmissionProvenanceV1>,
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.attempts.lock().expect("publisher lock poisoned");
        *guard += 1;
        Err(GovernancePublishError::other("simulated publish failure"))
    }
    fn publish_proof_token_issuance(
        &self,
        _issuance: &ProofTokenIssuanceV1,
        _encoded: &[u8],
        _provenance: Option<&GovernanceSubmissionProvenanceV1>,
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.attempts.lock().expect("publisher lock poisoned");
        *guard += 1;
        Err(GovernancePublishError::other("simulated publish failure"))
    }
    fn publish_appeal_finance_report(
        &self,
        _report: &SoraFsAppealFinanceReportV1,
        _encoded: &[u8],
        _provenance: &GovernanceSubmissionProvenanceV1,
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.attempts.lock().expect("publisher lock poisoned");
        *guard += 1;
        Err(GovernancePublishError::other("simulated publish failure"))
    }
    fn publish_appeal_finance_weekly_rollup(
        &self,
        _rollup: &SoraFsAppealFinanceWeeklyRollupV1,
        _encoded: &[u8],
        _provenance: &GovernanceSubmissionProvenanceV1,
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.attempts.lock().expect("publisher lock poisoned");
        *guard += 1;
        Err(GovernancePublishError::other("simulated publish failure"))
    }
    fn publish_appeal_finance_settlement_receipt(
        &self,
        _receipt: &SoraFsAppealFinanceSettlementReceiptV1,
        _encoded: &[u8],
    ) -> Result<(), GovernancePublishError> {
        let mut guard = self.attempts.lock().expect("publisher lock poisoned");
        *guard += 1;
        Err(GovernancePublishError::other("simulated publish failure"))
    }
}
