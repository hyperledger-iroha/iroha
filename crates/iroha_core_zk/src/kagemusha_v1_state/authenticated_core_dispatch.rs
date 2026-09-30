//! Concrete native dispatch over the original hardware-selected Core and held journals.
//!
//! Public selectors never become a qualification, proof, new-work permit or a substitute
//! for an authenticated checkpoint. Journal reservations retain original caller IDs only.

use super::*;

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
                .credential
                .credential_id,
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
                    .credential
                    .credential_id
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

fn material_error(error: impl std::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::RecoveryMaterial(error.to_string())
}
