//! Concrete native dispatch over the original hardware-selected Core and held journals.
//!
//! Public selectors never become a qualification, proof, new-work permit or a substitute
//! for an authenticated checkpoint. Journal reservations retain original caller IDs only.

use super::*;
use iroha_data_model::kagemusha::KagemushaAggregateStateCommitmentV1;

/// Public wallet head copied from the complete freshly authenticated native checkpoint.
/// These projections disclose no private opening and grant no monetary or hardware authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaAuthenticatedWalletObservationV1 {
    /// Exact release, asset, policy registry root, logical sequence and state commitment.
    pub aggregate: KagemushaAggregateStateCommitmentV1,
    /// Native rollback-resistant wallet journal revision.
    pub journal_revision: u128,
    /// Mint and peer credits durably staged but not yet folded into the state.
    pub pending_credit_count: u128,
    /// Installed terminal envelopes retained for byte-identical retry.
    pub retry_outbox_count: u128,
}

/// Borrowed original proving selection obtainable only from the concrete current native owner.
/// It cannot be decoded, cloned into a new owner or manufactured from a public candidate.
pub struct KagemushaAuthenticatedOutgoingProvingSelectionV1<'a> {
    owner: &'a KagemushaAuthenticatedCoreOwnerV1,
    operation_id: DigestV1,
    prepared: &'a PreparedOutgoingCandidateV1,
}

/// Original committed proving admission retained by the concrete freshly selected native owner.
/// It cannot be manufactured from a decoded completed record or a public commit certificate.
pub struct KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1<'a> {
    owner: &'a KagemushaAuthenticatedCoreOwnerV1,
    committed: &'a CommittedOutgoingCandidateV1,
    original: &'a outgoing::CommitAuthorizationOriginal,
}

impl KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1<'_> {
    /// Reauthenticate the selected Core checkpoint and exact retained commit evidence.
    pub fn recheck(&self) -> Result<(), KagemushaStateErrorV1> {
        self.owner.current_recovery_selection()?;
        self.original.verify(self.owner)?;
        if self.owner.machine.committed_candidate_for_finalization()? != *self.committed {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.owner.current_recovery_selection()?;
        Ok(())
    }

    /// Return the original committed operation after rechecking its native custody.
    pub fn operation_id(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(self.original.operation_id)
    }

    /// Borrow the exact committed native candidate after rechecking its custody.
    pub fn committed(&self) -> Result<&CommittedOutgoingCandidateV1, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(self.committed)
    }

    /// Reverified original raw proof and Guard. The projection carries no history capability.
    pub fn authorization(&self) -> Result<TransitionAuthorizationV1, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(self.original.authorization())
    }

    /// Return the retained commit verification time after rechecking its native evidence.
    pub fn reference_ms(&self) -> Result<u64, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(self.original.reference_ms)
    }

    /// The independently reverified original signed op7 frame retained before funds changed.
    pub fn original_hardware_commit(
        &self,
    ) -> Result<&KagemushaOriginalOutgoingHardwareCommitV1, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(&self.original.device_original)
    }

    /// Derive the canonical Guard statement from the reverified committed transition.
    pub fn normalized_guard_statement(
        &self,
    ) -> Result<KagemushaNormalizedGuardStatementV1, KagemushaStateErrorV1> {
        self.recheck()?;
        let prepared = &self.committed.candidate.prepared;
        KagemushaNormalizedGuardStatementV1::derive_from_transition(
            &prepared.proof_statement,
            transition_guard_context(
                self.owner.machine.proof_release.artifacts,
                &prepared.proof_statement,
                self.original.reference_ms,
            )?,
        )
        .map_err(|e| KagemushaStateErrorV1::ProofRejected(e.to_string()))
    }

    /// Return the independently authenticated production release retained by this owner.
    pub fn authenticated_release(
        &self,
    ) -> Result<Arc<KagemushaAuthenticatedReleaseV1>, KagemushaStateErrorV1> {
        self.recheck()?;
        self.owner.authenticated_release()
    }
}

impl KagemushaAuthenticatedOutgoingProvingSelectionV1<'_> {
    /// Exact caller operation retained by this actual native preparation.
    pub fn operation_id(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(self.operation_id)
    }
    /// Reauthenticate the original selected Core checkpoint and retained journal custody.
    pub fn recheck(&self) -> Result<(), KagemushaStateErrorV1> {
        self.owner.current_recovery_selection()?;
        let record = self
            .owner
            .machine
            .outgoing_operation_index()
            .lookup(self.operation_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if record.preparation_id != self.prepared.preparation_id
            || !matches!(
                record.phase,
                KagemushaOutgoingOperationPhaseV1::Prepared
                    | KagemushaOutgoingOperationPhaseV1::CandidatePersisted
            )
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        record
            .validate_against_prepared(self.prepared)
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
        self.owner.current_recovery_selection()?;
        Ok(())
    }

    /// Original prepared native input. These fields alone are never a proving admission.
    pub fn prepared(&self) -> Result<&PreparedOutgoingCandidateV1, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(self.prepared)
    }

    /// Production release retained by this exact native owner, independently authenticated.
    pub fn authenticated_release(
        &self,
    ) -> Result<Arc<KagemushaAuthenticatedReleaseV1>, KagemushaStateErrorV1> {
        self.recheck()?;
        self.owner.authenticated_release()
    }
}

impl KagemushaAuthenticatedCoreOwnerV1 {
    /// Copy the actual public wallet head and counts under fresh complete checkpoint custody.
    /// A signed device observation must match every returned field; matching wallet scope alone
    /// does not establish the currently selected state. The policy ID is the registry root,
    /// distinct from the manifest's hardware-policy digest.
    ///
    /// # Errors
    /// Rejects lost original custody, changed journals, stale hardware or invalid native state.
    pub fn current_wallet_observation(
        &self,
    ) -> Result<KagemushaAuthenticatedWalletObservationV1, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let observation = wallet_observation(&self.machine)?;
        self.current_recovery_selection()?;
        Ok(observation)
    }

    /// Find the original native record for an exact payment credit or redemption identity.
    /// This read-only result is a retry selector, never a proof or transition capability.
    ///
    /// # Errors
    /// Rejects zero identities, ambiguous or malformed indexes and stale original custody.
    pub fn outgoing_record_for_terminal(
        &self,
        terminal_id: DigestV1,
    ) -> Result<Option<KagemushaOutgoingOperationRecordV1>, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let record = terminal_record(&self.machine, terminal_id)?;
        self.current_recovery_selection()?;
        Ok(record)
    }

    /// Look up the actual retained native operation under fresh complete checkpoint custody.
    /// An absent record is an authenticated index observation, never a swallowed recovery error.
    ///
    /// # Errors
    /// Rejects zero identities, malformed or foreign records and stale original custody.
    pub fn outgoing_record_for_operation(
        &self,
        operation_id: DigestV1,
    ) -> Result<Option<KagemushaOutgoingOperationRecordV1>, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let record = operation_record(&self.machine, operation_id)?;
        self.current_recovery_selection()?;
        Ok(record)
    }

    /// Return the exact installed terminal payment or redemption bytes for one native operation.
    /// The native journal, original proof, reservation, index and selected checkpoint are
    /// reauthenticated before these public retry bytes are copied. They grant no new work.
    ///
    /// # Errors
    /// Rejects stale custody, foreign or unfinished operations and changed terminal material.
    pub fn original_terminal_envelope(
        &self,
        operation_id: DigestV1,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let original = self.machine.original_terminal_envelope(operation_id)?;
        self.current_recovery_selection()?;
        Ok(original)
    }

    /// Admit final payment/redemption proving only for the actual committed native candidate.
    /// Independently unsealed nonce, credential and reference witnesses remain mandatory.
    pub fn committed_outgoing_proving_selection(
        &self,
        operation_id: DigestV1,
    ) -> Result<KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1<'_>, KagemushaStateErrorV1>
    {
        self.current_recovery_selection()?;
        let KagemushaOutgoingJournalStageV1::Committed(committed) =
            self.machine.outgoing_candidate_journal.stage()
        else {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        };
        let original = self
            .committed_authorization
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if original.operation_id != operation_id {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let selection = KagemushaAuthenticatedCommittedOutgoingProvingSelectionV1 {
            owner: self,
            committed,
            original,
        };
        selection.recheck()?;
        Ok(selection)
    }
    /// Select one genuine uncommitted outgoing preparation for the production prover owner.
    /// Private witness unsealing and original hardware preparation authentication remain mandatory.
    pub fn outgoing_proving_selection(
        &self,
        operation_id: DigestV1,
    ) -> Result<KagemushaAuthenticatedOutgoingProvingSelectionV1<'_>, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let prepared = match self.machine.outgoing_candidate_journal.stage() {
            KagemushaOutgoingJournalStageV1::Prepared(prepared) => prepared,
            KagemushaOutgoingJournalStageV1::Candidate(candidate) => &candidate.prepared,
            _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
        };
        let selection = KagemushaAuthenticatedOutgoingProvingSelectionV1 {
            owner: self,
            operation_id,
            prepared,
        };
        selection.recheck()?;
        Ok(selection)
    }
    /// Actual native wallet context for independent original sender-reply authentication.
    /// This public projection grants no proof, signer, monetary permit or fresh device lease.
    pub fn sender_context(
        &self,
    ) -> Result<KagemushaOutgoingOperationContextV1, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let state = &self.machine.state;
        let context = KagemushaOutgoingOperationContextV1 {
            lane: state.lane.clone(),
            release: state.context(),
            credential_id: self
                .machine
                .recovery_metadata
                .accepted_credential
                .original_digest()?,
            hardware_epoch: state.hardware_epoch,
            device_policy_binding: state.device_policy_binding,
            core_authorization_key_reference: self
                .machine
                .enrollment_binding()
                .core_authorization_key_reference,
        };
        self.current_recovery_selection()?;
        Ok(context)
    }
    /// Retain one exact original operation binding in the locked coordinator WAL.
    ///
    /// The native frame adapter must decode the operation's closed command schema before
    /// this call. This durable ID is only a retry selector; returning it grants no monetary
    /// work or device permission. The complete current checkpoint and original descriptors
    /// are reauthenticated before and after the actual append.
    ///
    /// # Errors
    /// Rejects changed custody, stale hardware, conflicting IDs, malformed bindings or I/O loss.
    pub fn reserve_coordinator_operation(
        &mut self,
        operation_id: DigestV1,
        operation: u8,
        public_binding: &[u8],
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let reserved = self
            .machine
            .reserve_coordinator_operation(
                self.journals.coordinator_mut(),
                operation_id,
                operation,
                public_binding,
            )
            .map_err(material_error)?;
        self.current_recovery_selection()?;
        Ok(reserved)
    }

    /// Durably retain the native sender's exact public intent before device preparation.
    ///
    /// A new intent must name the credential already authenticated in the current Core
    /// floor and the separately retained Core authorization key. Device and Core key
    /// references are distinct. Existing indexed retries retain their historical context;
    /// they cannot create another preparation or renew its credential.
    ///
    /// # Errors
    /// Rejects substituted scope, key, credential, operation, checkpoint or original WAL.
    pub fn begin_coordinator_sender_intent(
        &mut self,
        intent: &KagemushaOutgoingPublicInputPreimageV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let existing = self
            .machine
            .classify_outgoing_operation_prepare(intent)
            .map_err(material_error)?;
        if existing.is_none()
            && (intent.context.credential_id
                != self
                    .machine
                    .recovery_metadata()
                    .accepted_credential
                    .original_digest()?
                || intent.context.core_authorization_key_reference
                    != self
                        .machine
                        .enrollment_binding()
                        .core_authorization_key_reference)
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        self.machine
            .begin_coordinator_sender_intent(self.journals.coordinator_mut(), intent)
            .map_err(material_error)?;
        self.current_recovery_selection()?;
        Ok(())
    }

    /// Read the original reserved intent and its actual authenticated Core index phase.
    /// No caller-projected archive or byte-store replay supplies that phase.
    ///
    /// # Errors
    /// Rejects absent, conflicting or foreign operations and any loss of current custody.
    pub fn recover_coordinator_sender_intent(
        &self,
        operation_id: DigestV1,
    ) -> Result<KagemushaCoordinatorSenderIntentRecoveryV1, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let retained = self
            .machine
            .recover_coordinator_sender_intent(self.journals.coordinator(), operation_id)
            .map_err(material_error)?;
        self.current_recovery_selection()?;
        Ok(retained)
    }

    /// Export the retained operation's genuine paired State proof under current owner custody.
    /// The receiving verifier independently authenticates these public bytes. Exporting them
    /// grants no transition, session, hardware or ledger authority.
    ///
    /// # Errors
    /// Rejects stale custody, released or missing operations and invalid native proof material.
    pub fn export_outgoing_state_proof_archives(
        &self,
        operation_id: DigestV1,
    ) -> Result<KagemushaOutgoingStateProofArchivePairV1, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let pair = self
            .machine
            .export_outgoing_state_proof_archives(operation_id)?;
        self.current_recovery_selection()?;
        Ok(pair)
    }
}

fn wallet_observation<R, G, H>(
    machine: &KagemushaStateMachineV1<R, G, H>,
) -> Result<KagemushaAuthenticatedWalletObservationV1, KagemushaStateErrorV1>
where
    R: KagemushaRecursiveVerifierV1,
    G: KagemushaGuardBundleVerifierV1,
    H: KagemushaAuthenticatedHistoryStoreV1,
{
    let state = &machine.state;
    let aggregate = KagemushaAggregateStateCommitmentV1 {
        version: KAGEMUSHA_STATE_VERSION_V1,
        release_id: state.release_id,
        network_id: state.lane.network_id,
        asset: state.lane.asset.clone(),
        asset_incarnation: state.asset_incarnation,
        scale: state.lane.scale,
        liability_pool_id: state.liability_pool_id,
        lane_id: state.lane.device_lane_id,
        hardware_epoch_id: state.hardware_epoch.epoch_id,
        key_reference: state.device_policy_binding.device_key_reference,
        hardware_policy_id: state.device_policy_binding.hardware_policy_id,
        sequence: state.logical_sequence,
        state_commitment: state.state_commitment,
    };
    aggregate.validate().map_err(material_error)?;
    let retry_outbox_count = machine
        .outgoing_operation_index()
        .records()
        .filter(|record| record.phase == KagemushaOutgoingOperationPhaseV1::Installed)
        .count() as u128;
    Ok(KagemushaAuthenticatedWalletObservationV1 {
        aggregate,
        journal_revision: machine.journal_revision,
        pending_credit_count: machine.pending_credit_count() as u128,
        retry_outbox_count,
    })
}

fn terminal_record<R, G, H>(
    machine: &KagemushaStateMachineV1<R, G, H>,
    terminal_id: DigestV1,
) -> Result<Option<KagemushaOutgoingOperationRecordV1>, KagemushaStateErrorV1>
where
    R: KagemushaRecursiveVerifierV1,
    G: KagemushaGuardBundleVerifierV1,
    H: KagemushaAuthenticatedHistoryStoreV1,
{
    if terminal_id == [0; 32] {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    }
    let mut matches = machine
        .outgoing_operation_index()
        .records()
        .filter(|record| record.outcome_id == terminal_id);
    let record = matches.next().cloned();
    if matches.next().is_some() {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    if let Some(record) = &record {
        record.validate().map_err(material_error)?;
        record
            .context
            .validate_retained_against_state(&machine.state)
            .map_err(material_error)?;
    }
    Ok(record)
}

fn operation_record<R, G, H>(
    machine: &KagemushaStateMachineV1<R, G, H>,
    operation_id: DigestV1,
) -> Result<Option<KagemushaOutgoingOperationRecordV1>, KagemushaStateErrorV1>
where
    R: KagemushaRecursiveVerifierV1,
    G: KagemushaGuardBundleVerifierV1,
    H: KagemushaAuthenticatedHistoryStoreV1,
{
    if operation_id == [0; 32] {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    }
    let record = machine
        .outgoing_operation_index()
        .lookup(operation_id)
        .cloned();
    if let Some(record) = &record {
        record.validate().map_err(material_error)?;
        if record.operation_id != operation_id {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        record
            .context
            .validate_retained_against_state(&machine.state)
            .map_err(material_error)?;
    }
    Ok(record)
}

fn material_error(error: impl std::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::RecoveryMaterial(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_uses_actual_registry_root_and_full_width_native_head() {
        // Pure projection test only. This generic test machine cannot construct the concrete
        // production owner whose public getter additionally challenges hardware twice.
        let (mut machine, _, _) =
            super::super::super::tests::coordinator_operation_store_tests::machine();
        machine.state.logical_sequence = u128::from(u64::MAX) + 19;
        machine.journal_revision = u128::from(u64::MAX) + 23;
        let observed = wallet_observation(&machine).unwrap();
        assert_eq!(
            observed.aggregate.hardware_policy_id,
            machine.state.device_policy_binding.hardware_policy_id
        );
        assert_eq!(observed.aggregate.sequence, machine.state.logical_sequence);
        assert_eq!(
            observed.aggregate.state_commitment,
            machine.state.state_commitment
        );
        assert_eq!(observed.journal_revision, machine.journal_revision);
        assert_eq!(
            observed.pending_credit_count,
            machine.pending_credit_count() as u128
        );
        assert_eq!(observed.retry_outbox_count, 0);
        let encoded = norito::encode_canonical(&observed.aggregate).unwrap();
        assert_eq!(
            KagemushaAggregateStateCommitmentV1::decode_canonical_exact(&encoded).unwrap(),
            observed.aggregate
        );
        machine.state.device_policy_binding.hardware_policy_id = [0; 32];
        assert!(wallet_observation(&machine).is_err());
    }

    #[test]
    fn terminal_selector_rejects_zero_and_never_invents_an_absent_operation() {
        let (machine, _, _) =
            super::super::super::tests::coordinator_operation_store_tests::machine();
        assert!(terminal_record(&machine, [0; 32]).is_err());
        assert_eq!(terminal_record(&machine, [0x81; 32]).unwrap(), None);
    }

    #[test]
    fn operation_selector_distinguishes_invalid_identity_from_authenticated_absence() {
        let (machine, _, _) =
            super::super::super::tests::coordinator_operation_store_tests::machine();
        assert!(operation_record(&machine, [0; 32]).is_err());
        assert_eq!(operation_record(&machine, [0x82; 32]).unwrap(), None);
    }
}
