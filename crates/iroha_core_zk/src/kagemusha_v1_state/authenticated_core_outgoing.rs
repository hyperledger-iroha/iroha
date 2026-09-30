//! Typed outgoing work under the concrete native owner and exclusive publication stages.

use super::*;
use iroha_data_model::kagemusha::{KagemushaCommitCertificateV1, KagemushaRedemptionProofV1};

const TERMINAL_FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "outgoing-commit.norito.wal",
    magic: b"IKGOCW1\0",
    hash_domain: b"iroha:kagemusha:v1:outgoing-commit-original\0",
    maximum_payload_bytes: 4 * 1024 * 1024,
};

#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OutgoingCommitOriginalV1")]
enum CommitRecord {
    Intent {
        operation_id: DigestV1,
        previous: KagemushaStateSnapshotV1,
        checkpoint: DurabilityAnchorV1,
    },
    Command {
        canonical_command: Vec<u8>,
    },
    Completed {
        certificate: KagemushaCommitCertificateV1,
        device_original: KagemushaOriginalOutgoingHardwareCommitV1,
        // Persist verifiable original evidence, never the opaque history authorization.
        // Reading this record grants no Core owner or transition capability.
        hardware_certificate: HardwareTransitionCertificateV1,
        proof: KagemushaPairedProofV1,
        reference_ms: u64,
        checkpoint_operation_id: DigestV1,
        snapshot_directory: String,
    },
}

impl CommitRecord {
    fn completion_evidence(
        certificate: KagemushaCommitCertificateV1,
        authorization: &TransitionAuthorizationV1,
        device_original: &KagemushaOriginalOutgoingHardwareCommitV1,
        reference_ms: u64,
        checkpoint_operation_id: DigestV1,
        snapshot_directory: &Path,
    ) -> Result<Self, KagemushaStateErrorV1> {
        Ok(Self::Completed {
            certificate,
            device_original: device_original.clone(),
            hardware_certificate: authorization.hardware_certificate.clone(),
            proof: authorization.proof.clone(),
            reference_ms,
            checkpoint_operation_id,
            snapshot_directory: snapshot_directory
                .to_str()
                .ok_or(KagemushaStateErrorV1::InvalidRecoveryMaterial)?
                .to_owned(),
        })
    }
}

/// Exclusive original candidate retained before requesting the device's irreversible commit.
/// Its constructor consumes an authenticated Core owner; decoded candidate bytes cannot create it.
pub struct KagemushaAuthenticatedOutgoingCommitV1 {
    owner: KagemushaAuthenticatedCoreOwnerV1,
    operation_id: DigestV1,
    previous: KagemushaStateSnapshotV1,
    intent: Vec<u8>,
    journal: PrivateJournal,
    command: Option<CommandOriginal>,
    completion: Option<CompletionState>,
}

struct CommandOriginal {
    canonical_command: Vec<u8>,
    bytes: Vec<u8>,
}

struct CompletionState {
    bytes: Vec<u8>,
    mutation: Mutation,
    checkpoint_operation_id: DigestV1,
    snapshot_directory: std::path::PathBuf,
    applied: bool,
}

// Raw original evidence retained only after genuine native State and Guard verification.
// Serialization is custody material; every recovered value is independently reverified.
#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::CommittedAuthorizationOriginalV1")]
pub(super) struct CommitAuthorizationOriginal {
    pub(super) operation_id: DigestV1,
    pub(super) hardware_certificate: HardwareTransitionCertificateV1,
    pub(super) proof: KagemushaPairedProofV1,
    pub(super) reference_ms: u64,
    pub(super) device_original: KagemushaOriginalOutgoingHardwareCommitV1,
}

impl CommitAuthorizationOriginal {
    pub(super) fn authorization(&self) -> TransitionAuthorizationV1 {
        TransitionAuthorizationV1 {
            hardware_certificate: self.hardware_certificate.clone(),
            proof: self.proof.clone(),
            authenticated_history: None,
        }
    }

    pub(super) fn verify(
        &self,
        owner: &KagemushaAuthenticatedCoreOwnerV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        let committed = owner.machine.committed_candidate_for_finalization()?;
        let record = owner
            .machine
            .outgoing_operation_index()
            .lookup(self.operation_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if record.phase != KagemushaOutgoingOperationPhaseV1::Committed
            || record.preparation_id != committed.candidate.prepared.preparation_id
            || record.commit_certificate_digest != Some(committed.commit_certificate_digest)
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        record
            .validate_against_prepared(&committed.candidate.prepared)
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
        verify_original_commit(
            &owner.machine,
            &committed.candidate,
            &self.authorization(),
            self.reference_ms,
            &committed.commit_certificate,
            self.operation_id,
            &self.device_original,
        )?;
        if CommittedOutgoingCandidateV1::from_hardware_commit(
            committed.candidate.clone(),
            committed.commit_certificate.clone(),
        )? != committed
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }
}

/// Existing original storage either retains device commit preparation or its pending publication.
pub enum KagemushaAuthenticatedOutgoingCommitRecoveryV1 {
    /// Exclusive retained preparation awaiting the original device commit.
    Prepared(KagemushaAuthenticatedOutgoingCommitV1),
    /// Completed original commit retained in its exact publication recovery stage.
    Publication(KagemushaAuthenticatedCoreRecoveryV1),
}

impl KagemushaAuthenticatedCoreOwnerV1 {
    /// Recover an interrupted device commit from its exact held original WAL.
    /// The independently supplied publication destination must match the persisted completion.
    /// No caller certificate, decoded snapshot or fabricated history capability grants authority.
    pub fn recover_outgoing_commit_existing(
        intent_directory: &Path,
        snapshot_directory: &Path,
        checkpoint_operation_id: DigestV1,
        inputs: KagemushaAuthenticatedCoreRecoveryInputsV1<'_>,
    ) -> Result<KagemushaAuthenticatedOutgoingCommitRecoveryV1, KagemushaStateErrorV1> {
        let mut journal = PrivateJournal::open_existing(intent_directory, TERMINAL_FORMAT)
            .map_err(material_error)?;
        replay_commit_records(&mut journal)?;
        let mut frames = Vec::new();
        journal
            .scan_complete(|sequence, bytes| {
                if sequence > 2 || frames.len() as u64 != sequence {
                    return Err(PrivateJournalError::Corrupt);
                }
                frames.push(bytes.to_vec());
                Ok(())
            })
            .map_err(material_error)?;
        let intent = frames
            .first()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .clone();
        let CommitRecord::Intent {
            operation_id,
            previous,
            checkpoint,
        } = decode_commit_record(&intent)?
        else {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        };
        if previous.recovery_anchor() != checkpoint.statement {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        let command = if let Some(bytes) = frames.get(1) {
            let CommitRecord::Command { canonical_command } = decode_commit_record(bytes)? else {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            };
            Some(CommandOriginal {
                canonical_command,
                bytes: bytes.clone(),
            })
        } else {
            None
        };
        if frames.len() < 3 {
            require_commit_records(
                &journal,
                &intent,
                command.as_ref().map(|value| value.bytes.as_slice()),
                None,
            )?;
            let owner = inputs.restore(previous.clone(), &checkpoint)?;
            owner
                .machine
                .recover_indexed_outgoing_commit_capability(operation_id)?;
            let pending = KagemushaAuthenticatedOutgoingCommitV1 {
                owner,
                operation_id,
                previous,
                intent,
                journal,
                command,
                completion: None,
            };
            pending.original_command()?;
            return Ok(KagemushaAuthenticatedOutgoingCommitRecoveryV1::Prepared(
                pending,
            ));
        }
        let command = command.ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let completion = frames[2].clone();
        let CommitRecord::Completed {
            certificate,
            device_original,
            hardware_certificate,
            proof,
            reference_ms,
            checkpoint_operation_id: retained_checkpoint,
            snapshot_directory: retained_directory,
        } = decode_commit_record(&completion)?
        else {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        };
        if retained_checkpoint != checkpoint_operation_id
            || snapshot_directory.to_str() != Some(retained_directory.as_str())
            || device_original.canonical_command != command.canonical_command
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        require_commit_records(&journal, &intent, Some(&command.bytes), Some(&completion))?;
        let mutation = Mutation::Commit {
            operation_id,
            certificate,
            device_original,
            hardware_certificate,
            proof,
            reference_ms,
        };
        let recovered = match std::fs::symlink_metadata(snapshot_directory) {
            Ok(_) => Self::recover_checkpoint_original(
                snapshot_directory,
                inputs,
                Some(&previous),
                Some(&mutation),
            )?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let mut owner = inputs.restore(previous.clone(), &checkpoint)?;
                apply_mutation_original(&mut owner, &mutation)?;
                require_commit_records(&journal, &intent, Some(&command.bytes), Some(&completion))?;
                let pending = owner.stage_complete_checkpoint_with_original(
                    snapshot_directory,
                    checkpoint_operation_id,
                    previous,
                    mutation,
                )?;
                KagemushaAuthenticatedCoreRecoveryV1::Pending(pending)
            }
            Err(error) => return Err(material_error(error)),
        };
        require_commit_records(&journal, &intent, Some(&command.bytes), Some(&completion))?;
        Ok(KagemushaAuthenticatedOutgoingCommitRecoveryV1::Publication(
            recovered,
        ))
    }
    /// Derive and durably stage a receiver-authenticated send under the actual credential/key.
    pub fn stage_send_split(
        mut self,
        snapshot_directory: &Path,
        checkpoint_operation_id: DigestV1,
        operation_id: DigestV1,
        preparation: SendSplitPreparationV1,
    ) -> Result<KagemushaAuthenticatedCorePublicationV1, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let previous = self.selected_predecessor_snapshot()?;
        let mutation = Mutation::PrepareSend {
            operation_id,
            preparation: preparation.clone(),
        };
        let prepared = self.machine.prepare_send_split(preparation)?;
        self.stage_indexed_original(operation_id, prepared)?;
        self.stage_complete_checkpoint_with_original(
            snapshot_directory,
            checkpoint_operation_id,
            previous,
            mutation,
        )
    }

    /// Derive and durably stage a redemption under the original native owner.
    pub fn stage_redeem_split(
        mut self,
        snapshot_directory: &Path,
        checkpoint_operation_id: DigestV1,
        operation_id: DigestV1,
        preparation: RedeemSplitPreparationV1,
    ) -> Result<KagemushaAuthenticatedCorePublicationV1, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let previous = self.selected_predecessor_snapshot()?;
        let mutation = Mutation::PrepareRedemption {
            operation_id,
            preparation: preparation.clone(),
        };
        let prepared = self.machine.prepare_redeem_split(preparation)?;
        self.stage_indexed_original(operation_id, prepared)?;
        self.stage_complete_checkpoint_with_original(
            snapshot_directory,
            checkpoint_operation_id,
            previous,
            mutation,
        )
    }

    fn stage_indexed_original(
        &mut self,
        operation_id: DigestV1,
        prepared: PreparedOutgoingCandidateV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.machine.prepare_indexed_outgoing_candidate(
            operation_id,
            self.machine
                .accepted_credential_floor()
                .credential
                .credential_id,
            self.machine
                .enrollment_binding()
                .core_authorization_key_reference,
            prepared,
        )?;
        self.journals.validate_pair(&self.machine)
    }

    /// Verify the genuine paired State proof before persisting a hardware-committable candidate.
    pub fn stage_outgoing_state_proof(
        mut self,
        snapshot_directory: &Path,
        checkpoint_operation_id: DigestV1,
        operation_id: DigestV1,
        proof: KagemushaPairedProofV1,
    ) -> Result<KagemushaAuthenticatedCorePublicationV1, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let previous = self.selected_predecessor_snapshot()?;
        let mutation = Mutation::CandidateProof {
            operation_id,
            proof: proof.clone(),
        };
        let capability = self
            .machine
            .recover_indexed_outgoing_commit_capability(operation_id)?;
        let KagemushaOutgoingJournalStageV1::Prepared(prepared) =
            self.machine.outgoing_candidate_journal.stage()
        else {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        };
        capability.authorizes(prepared)?;
        let candidate = match prepared.recovery_view() {
            PreparedOutgoingRecoveryViewV1::Send { .. } => {
                PersistedOutgoingCandidateV1::verify_and_persist_send(
                    prepared.clone(),
                    proof,
                    self.machine.proof_release.artifacts,
                    &self.machine.recursive_verifier,
                )?
            }
            PreparedOutgoingRecoveryViewV1::Redemption { .. } => {
                PersistedOutgoingCandidateV1::verify_and_persist_redemption(
                    prepared.clone(),
                    proof,
                    self.machine.proof_release.artifacts,
                    &self.machine.recursive_verifier,
                )?
            }
        };
        self.machine
            .persist_verified_outgoing_candidate(candidate)?;
        self.stage_complete_checkpoint_with_original(
            snapshot_directory,
            checkpoint_operation_id,
            previous,
            mutation,
        )
    }

    /// Persist the complete original candidate before allowing an irreversible device commit.
    /// The usable owner cannot escape while this operation is pending.
    pub fn prepare_outgoing_commit(
        self,
        intent_directory: &Path,
        operation_id: DigestV1,
    ) -> Result<KagemushaAuthenticatedOutgoingCommitV1, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        self.machine
            .recover_indexed_outgoing_commit_capability(operation_id)?;
        if !matches!(
            self.machine.outgoing_candidate_journal.stage(),
            KagemushaOutgoingJournalStageV1::Candidate(_)
        ) {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let previous = self.selected_predecessor_snapshot()?;
        let checkpoint = self
            .machine
            .published_checkpoint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotRollback)?
            .anchor
            .clone();
        let intent = norito::encode_canonical(&CommitRecord::Intent {
            operation_id,
            previous: previous.clone(),
            checkpoint,
        })
        .map_err(material_error)?;
        let mut journal = PrivateJournal::create_new(intent_directory, TERMINAL_FORMAT)
            .map_err(material_error)?;
        journal.append(&intent).map_err(material_error)?;
        journal
            .require_single_record(&intent)
            .map_err(material_error)?;
        Ok(KagemushaAuthenticatedOutgoingCommitV1 {
            owner: self,
            operation_id,
            previous,
            intent,
            journal,
            command: None,
            completion: None,
        })
    }

    /// Verify and durably install the original compact payment before exposing its retry bytes.
    pub fn stage_final_payment(
        mut self,
        snapshot_directory: &Path,
        checkpoint_operation_id: DigestV1,
        payment: KagemushaPaymentV1,
        retry_metadata: Vec<u8>,
    ) -> Result<KagemushaAuthenticatedCorePublicationV1, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let previous = self.selected_predecessor_snapshot()?;
        let mutation = Mutation::FinalPayment {
            payment: payment.clone(),
            retry_metadata: retry_metadata.clone(),
        };
        let committed = self.machine.committed_candidate_for_finalization()?;
        let final_envelope = DurableOutgoingEnvelopeV1::finalize_payment(
            committed,
            payment,
            retry_metadata,
            self.machine.proof_release.artifacts,
            &self.machine.recursive_verifier,
        )?;
        self.machine
            .install_finalized_outgoing_envelope(final_envelope)?;
        self.committed_authorization = None;
        self.stage_complete_checkpoint_with_original(
            snapshot_directory,
            checkpoint_operation_id,
            previous,
            mutation,
        )
    }

    /// Verify and durably install the original redemption proof and canonical voucher.
    pub fn stage_final_redemption(
        mut self,
        snapshot_directory: &Path,
        checkpoint_operation_id: DigestV1,
        proof: KagemushaRedemptionProofV1,
        retry_metadata: Vec<u8>,
    ) -> Result<KagemushaAuthenticatedCorePublicationV1, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let previous = self.selected_predecessor_snapshot()?;
        let mutation = Mutation::FinalRedemption {
            proof: proof.clone(),
            retry_metadata: retry_metadata.clone(),
        };
        self.machine
            .finalize_outgoing_redemption(proof, retry_metadata)?;
        self.committed_authorization = None;
        self.stage_complete_checkpoint_with_original(
            snapshot_directory,
            checkpoint_operation_id,
            previous,
            mutation,
        )
    }
}

impl KagemushaAuthenticatedOutgoingCommitV1 {
    /// Return the independently authenticated immutable release under this original custody.
    /// The catalog confers no new wallet, signing permission or hardware freshness lease.
    pub fn authenticated_release(
        &self,
    ) -> Result<Arc<KagemushaAuthenticatedReleaseV1>, KagemushaStateErrorV1> {
        self.recheck_originals()?;
        let release = self.owner.authenticated_release()?;
        self.recheck_originals()?;
        Ok(release)
    }

    /// Authenticate and fsync the exact original Core-signed op7 before hardware dispatch.
    /// An uncertain append retains these same bytes; a different nonce, authorization or
    /// command cannot replace them. This provides retry custody, never another Core owner.
    pub fn retain_original_command(
        &mut self,
        canonical_command: &[u8],
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.completion.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.current_recovery_selection()?;
        device_commit::verify_command(
            &self.owner.machine,
            self.operation_id,
            self.candidate()?,
            canonical_command,
        )?;
        let bytes = norito::encode_canonical(&CommitRecord::Command {
            canonical_command: canonical_command.to_vec(),
        })
        .map_err(material_error)?;
        if bytes.is_empty() || bytes.len() as u64 > TERMINAL_FORMAT.maximum_payload_bytes {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        if let Some(retained) = &self.command {
            if retained.canonical_command != canonical_command || retained.bytes != bytes {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        } else {
            self.command = Some(CommandOriginal {
                canonical_command: canonical_command.to_vec(),
                bytes: bytes.clone(),
            });
            self.journal.append(&bytes).map_err(material_error)?;
        }
        require_commit_records(&self.journal, &self.intent, Some(&bytes), None)?;
        self.current_recovery_selection()?;
        Ok(())
    }

    /// Borrow the independently reverified original dispatched op7 command, when retained.
    /// Absence is permitted only before command exposure; recovery never creates a replacement.
    pub fn original_command(&self) -> Result<Option<&[u8]>, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        if let Some(command) = &self.command {
            device_commit::verify_command(
                &self.owner.machine,
                self.operation_id,
                self.candidate()?,
                &command.canonical_command,
            )?;
        }
        self.current_recovery_selection()?;
        Ok(self
            .command
            .as_ref()
            .map(|command| command.canonical_command.as_slice()))
    }

    /// Fresh actual selected predecessor while no completed terminal original was retained.
    /// The borrowed token cannot yield a usable Core owner or authorize another operation.
    pub fn current_recovery_selection(
        &self,
    ) -> Result<KagemushaCurrentRecoverySelectionV1<'_>, KagemushaStateErrorV1> {
        if self.completion.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.candidate()?;
        self.owner.current_recovery_selection()
    }

    /// Recheck exact native retry custody. A completed original permits retry only, never new work.
    pub fn recheck_originals(&self) -> Result<(), KagemushaStateErrorV1> {
        if let Some(completion) = &self.completion {
            require_commit_records(
                &self.journal,
                &self.intent,
                self.command.as_ref().map(|value| value.bytes.as_slice()),
                Some(&completion.bytes),
            )?;
            if let Some(original) = &self.owner.selected_publication {
                original.require_selected(&self.owner.machine)?;
            }
            self.owner.journals.validate_pair(&self.owner.machine)?;
            self.owner
                .transactions
                .recovery_prefix()
                .map_err(material_error)?;
            if completion.applied {
                self.owner
                    .committed_authorization
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                    .verify(&self.owner)?;
            } else {
                let KagemushaOutgoingJournalStageV1::Candidate(candidate) =
                    self.owner.machine.outgoing_candidate_journal.stage()
                else {
                    return Err(KagemushaStateErrorV1::InvalidCandidateStage);
                };
                let Mutation::Commit {
                    certificate,
                    device_original,
                    hardware_certificate,
                    proof,
                    reference_ms,
                    ..
                } = &completion.mutation
                else {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                };
                verify_original_commit(
                    &self.owner.machine,
                    candidate,
                    &TransitionAuthorizationV1 {
                        hardware_certificate: hardware_certificate.clone(),
                        proof: proof.clone(),
                        authenticated_history: None,
                    },
                    *reference_ms,
                    certificate,
                    self.operation_id,
                    device_original,
                )?;
                CommittedOutgoingCandidateV1::from_hardware_commit(
                    candidate.clone(),
                    certificate.clone(),
                )?;
            }
            self.owner
                .machine
                .guard_verifier
                .verify_current_recovery_checkpoint(
                    &self.previous.recovery_anchor(),
                    &self.previous.recovery_metadata.journals,
                )
                .map_err(material_error)?;
            require_commit_records(
                &self.journal,
                &self.intent,
                self.command.as_ref().map(|value| value.bytes.as_slice()),
                Some(&completion.bytes),
            )?;
            self.owner.journals.validate_pair(&self.owner.machine)?;
            self.owner
                .transactions
                .recovery_prefix()
                .map_err(material_error)?;
            Ok(())
        } else {
            self.current_recovery_selection()?;
            self.candidate()?;
            self.current_recovery_selection()?;
            Ok(())
        }
    }
    /// Public recovery record selected by this original native-owned operation ID.
    /// This projection supplies no independent commit or hardware authority.
    pub fn operation_record(
        &self,
    ) -> Result<KagemushaOutgoingOperationRecordV1, KagemushaStateErrorV1> {
        self.candidate()?;
        self.owner
            .machine
            .outgoing_operation_index()
            .lookup(self.operation_id)
            .cloned()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)
    }

    /// Original prepared fields retained by the verified native candidate.
    pub fn prepared(&self) -> Result<&PreparedOutgoingCandidateV1, KagemushaStateErrorV1> {
        Ok(&self.candidate()?.prepared)
    }
    /// Original verified candidate under this exclusive native owner, for native command7 signing.
    /// Borrowing its public fields does not grant a second commit or create another owner.
    pub fn candidate(&self) -> Result<&PersistedOutgoingCandidateV1, KagemushaStateErrorV1> {
        if let Some(original) = &self.owner.selected_publication {
            original.require_selected(&self.owner.machine)?;
        }
        self.owner.journals.validate_pair(&self.owner.machine)?;
        self.owner
            .transactions
            .recovery_prefix()
            .map_err(material_error)?;
        require_commit_records(
            &self.journal,
            &self.intent,
            self.command.as_ref().map(|value| value.bytes.as_slice()),
            None,
        )?;
        let KagemushaOutgoingJournalStageV1::Candidate(candidate) =
            self.owner.machine.outgoing_candidate_journal.stage()
        else {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        };
        let record = self
            .owner
            .machine
            .outgoing_operation_index()
            .lookup(self.operation_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if record.phase != KagemushaOutgoingOperationPhaseV1::CandidatePersisted {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        record
            .validate_against_prepared(&candidate.prepared)
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
        if let Some(original) = &self.owner.selected_publication {
            original.require_selected(&self.owner.machine)?;
        }
        Ok(candidate)
    }

    /// Authenticate the original paired proof and native Guard before installing any successor.
    /// The verified completion is synced before changing Core memory or requesting checkpoint CAS.
    pub fn complete(
        self,
        certificate: KagemushaCommitCertificateV1,
        authorization: TransitionAuthorizationV1,
        device_original: KagemushaOriginalOutgoingHardwareCommitV1,
        reference_ms: u64,
        snapshot_directory: &Path,
        checkpoint_operation_id: DigestV1,
    ) -> Result<KagemushaAuthenticatedCorePublicationV1, KagemushaStateErrorV1> {
        self.complete_or_retain(
            certificate,
            authorization,
            device_original,
            reference_ms,
            snapshot_directory,
            checkpoint_operation_id,
        )
        .map_err(|(_, error)| error)
    }

    /// Retain this exact exclusive owner on proof, storage or publication failure.
    /// Once completion was persisted, retries must keep every original byte and destination.
    /// Neither an old usable owner nor a second commit request is exposed after completion.
    pub fn complete_or_retain(
        mut self,
        certificate: KagemushaCommitCertificateV1,
        authorization: TransitionAuthorizationV1,
        device_original: KagemushaOriginalOutgoingHardwareCommitV1,
        reference_ms: u64,
        snapshot_directory: &Path,
        checkpoint_operation_id: DigestV1,
    ) -> Result<KagemushaAuthenticatedCorePublicationV1, (Box<Self>, KagemushaStateErrorV1)> {
        match self.complete_attempt(
            certificate,
            authorization,
            device_original,
            reference_ms,
            snapshot_directory,
            checkpoint_operation_id,
        ) {
            Ok(parts) => Ok(parts.into_publication(self.owner)),
            Err(error) => Err((Box::new(self), error)),
        }
    }

    fn complete_attempt(
        &mut self,
        certificate: KagemushaCommitCertificateV1,
        authorization: TransitionAuthorizationV1,
        device_original: KagemushaOriginalOutgoingHardwareCommitV1,
        reference_ms: u64,
        snapshot_directory: &Path,
        checkpoint_operation_id: DigestV1,
    ) -> Result<publication::PublicationParts, KagemushaStateErrorV1> {
        if authorization.authenticated_history.is_some() {
            return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
        }
        let retained_command = self
            .command
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if retained_command.canonical_command != device_original.canonical_command {
            return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
        }
        let completed = CommitRecord::completion_evidence(
            certificate.clone(),
            &authorization,
            &device_original,
            reference_ms,
            checkpoint_operation_id,
            snapshot_directory,
        )?;
        let completion = norito::encode_canonical(&completed).map_err(material_error)?;
        if completion.is_empty() || completion.len() as u64 > TERMINAL_FORMAT.maximum_payload_bytes
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        if let Some(retained) = &self.completion {
            if retained.bytes != completion
                || retained.checkpoint_operation_id != checkpoint_operation_id
                || retained.snapshot_directory != snapshot_directory
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        } else {
            let candidate = self.candidate()?.clone();
            self.owner.journals.validate_pair(&self.owner.machine)?;
            verify_original_commit(
                &self.owner.machine,
                &candidate,
                &authorization,
                reference_ms,
                &certificate,
                self.operation_id,
                &device_original,
            )?;
            CommittedOutgoingCandidateV1::from_hardware_commit(candidate, certificate.clone())?;
            let mutation = Mutation::Commit {
                operation_id: self.operation_id,
                certificate: certificate.clone(),
                device_original,
                hardware_certificate: authorization.hardware_certificate.clone(),
                proof: authorization.proof.clone(),
                reference_ms,
            };
            // Retain the exact original before an append whose acknowledgement may be lost.
            self.completion = Some(CompletionState {
                bytes: completion.clone(),
                mutation,
                checkpoint_operation_id,
                snapshot_directory: snapshot_directory.to_owned(),
                applied: false,
            });
            self.journal.append(&completion).map_err(material_error)?;
        }
        require_commit_records(
            &self.journal,
            &self.intent,
            self.command.as_ref().map(|value| value.bytes.as_slice()),
            Some(&completion),
        )?;
        let retained = self
            .completion
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if !retained.applied {
            // Reauthenticate the retained original even after a complete append retry. The native
            // private mutation kernel consumes the actual indexed candidate exactly once.
            apply_mutation_original(&mut self.owner, &retained.mutation)?;
            retained.applied = true;
        }
        self.owner.journals.validate_pair(&self.owner.machine)?;
        require_commit_records(
            &self.journal,
            &self.intent,
            self.command.as_ref().map(|value| value.bytes.as_slice()),
            Some(&completion),
        )?;
        self.owner.build_checkpoint_original(
            snapshot_directory,
            checkpoint_operation_id,
            self.previous.clone(),
            retained.mutation.clone(),
        )
    }
}

// The owner accepts at most Intent, Command and Completed. Inspect one extra frame only
// to reject a suffix; malformed/torn frames remain errors from the native byte owner.
// A failed recovery drops this private descriptor and never exposes an owner or partial scan.
fn replay_commit_records(journal: &mut PrivateJournal) -> Result<(), KagemushaStateErrorV1> {
    for expected_sequence in 0..3 {
        match journal.replay_next().map_err(material_error)? {
            Some((sequence, _)) if sequence == expected_sequence => {}
            Some(_) => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
            None => return Ok(()),
        }
    }
    if journal.replay_next().map_err(material_error)?.is_some() {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}

fn require_commit_records(
    journal: &PrivateJournal,
    intent: &[u8],
    command: Option<&[u8]>,
    completion: Option<&[u8]>,
) -> Result<(), KagemushaStateErrorV1> {
    if completion.is_some() && command.is_none() {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let mut expected = vec![intent];
    if let Some(command) = command {
        expected.push(command);
    }
    if let Some(completion) = completion {
        expected.push(completion);
    }
    let mut count = 0;
    journal
        .scan_complete(|sequence, bytes| {
            if usize::try_from(sequence)
                .ok()
                .and_then(|index| expected.get(index))
                .copied()
                != Some(bytes)
            {
                return Err(PrivateJournalError::Corrupt);
            }
            count += 1;
            Ok(())
        })
        .map_err(material_error)?;
    if count != expected.len() {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}

fn decode_commit_record(bytes: &[u8]) -> Result<CommitRecord, KagemushaStateErrorV1> {
    if bytes.is_empty() || bytes.len() as u64 > TERMINAL_FORMAT.maximum_payload_bytes {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let record = norito::decode_canonical::<CommitRecord>(bytes).map_err(material_error)?;
    if norito::encode_canonical(&record).map_err(material_error)? != bytes {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(record)
}

pub(super) fn verify_original_commit(
    machine: &Machine,
    candidate: &PersistedOutgoingCandidateV1,
    authorization: &TransitionAuthorizationV1,
    reference_ms: u64,
    certificate: &KagemushaCommitCertificateV1,
    operation_id: DigestV1,
    device_original: &KagemushaOriginalOutgoingHardwareCommitV1,
) -> Result<(), KagemushaStateErrorV1> {
    // A precommit State Guard cannot attest irreversible hardware consumption. The actual
    // signed op7 success, complete candidate, original Core command and certificate identity
    // are independently authenticated before any funds mutation or recovered proving admission.
    device_commit::verify(
        machine,
        operation_id,
        candidate,
        certificate,
        device_original,
    )?;
    let prepared = &candidate.prepared;
    if authorization.authenticated_history.is_some()
        || reference_ms == 0
        || authorization.proof != *candidate.recovery_view()?.candidate_proof
        || authorization.hardware_certificate.statement != prepared.hardware_statement()
    {
        return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
    }
    let normalized = KagemushaNormalizedGuardStatementV1::derive_from_transition(
        &prepared.proof_statement,
        transition_guard_context(
            machine.proof_release.artifacts,
            &prepared.proof_statement,
            reference_ms,
        )?,
    )
    .map_err(|e| KagemushaStateErrorV1::ProofRejected(e.to_string()))?;
    if normalized
        .canonical_digest()
        .map_err(|e| KagemushaStateErrorV1::ProofRejected(e.to_string()))?
        != prepared.normalized_guard_statement_digest
    {
        return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
    }
    let inputs = prepared
        .candidate_public_inputs(machine.proof_release.artifacts, &authorization.proof)
        .map_err(KagemushaStateErrorV1::ProofRejected)?;
    verify_kagemusha_state_proof_v1(
        &machine.recursive_verifier,
        machine.proof_release.artifacts,
        &inputs,
        &authorization.proof,
    )
    .map_err(|e| KagemushaStateErrorV1::ProofRejected(e.to_string()))?;
    validate_guard_bytes(&authorization.hardware_certificate.guard_bundle)?;
    machine
        .guard_verifier
        .verify_transition(
            &prepared.hardware_statement(),
            &prepared.proof_statement,
            &normalized,
            &authorization.hardware_certificate.guard_bundle,
        )
        .map_err(KagemushaStateErrorV1::GuardRejected)
}

// Reconstruct only against an independently restored, freshly selected concrete predecessor.
// Every serialized input is untrusted; the real proof/Guard and actual private kernels are reused.
pub(super) fn apply_mutation_original(
    owner: &mut KagemushaAuthenticatedCoreOwnerV1,
    mutation: &Mutation,
) -> Result<(), KagemushaStateErrorV1> {
    owner.current_recovery_selection()?;
    match mutation {
        Mutation::JournalHeads => {}
        Mutation::PrepareSend {
            operation_id,
            preparation,
        } => {
            let prepared = owner.machine.prepare_send_split(preparation.clone())?;
            owner.stage_indexed_original(*operation_id, prepared)?;
        }
        Mutation::PrepareRedemption {
            operation_id,
            preparation,
        } => {
            let prepared = owner.machine.prepare_redeem_split(preparation.clone())?;
            owner.stage_indexed_original(*operation_id, prepared)?;
        }
        Mutation::CandidateProof {
            operation_id,
            proof,
        } => {
            let capability = owner
                .machine
                .recover_indexed_outgoing_commit_capability(*operation_id)?;
            let KagemushaOutgoingJournalStageV1::Prepared(prepared) =
                owner.machine.outgoing_candidate_journal.stage()
            else {
                return Err(KagemushaStateErrorV1::InvalidCandidateStage);
            };
            capability.authorizes(prepared)?;
            let candidate = match prepared.recovery_view() {
                PreparedOutgoingRecoveryViewV1::Send { .. } => {
                    PersistedOutgoingCandidateV1::verify_and_persist_send(
                        prepared.clone(),
                        proof.clone(),
                        owner.machine.proof_release.artifacts,
                        &owner.machine.recursive_verifier,
                    )?
                }
                PreparedOutgoingRecoveryViewV1::Redemption { .. } => {
                    PersistedOutgoingCandidateV1::verify_and_persist_redemption(
                        prepared.clone(),
                        proof.clone(),
                        owner.machine.proof_release.artifacts,
                        &owner.machine.recursive_verifier,
                    )?
                }
            };
            owner
                .machine
                .persist_verified_outgoing_candidate(candidate)?;
        }
        Mutation::Commit {
            operation_id,
            certificate,
            device_original,
            hardware_certificate,
            proof,
            reference_ms,
        } => {
            let KagemushaOutgoingJournalStageV1::Candidate(candidate) =
                owner.machine.outgoing_candidate_journal.stage()
            else {
                return Err(KagemushaStateErrorV1::InvalidCandidateStage);
            };
            let authorization = TransitionAuthorizationV1 {
                hardware_certificate: hardware_certificate.clone(),
                proof: proof.clone(),
                authenticated_history: None,
            };
            verify_original_commit(
                &owner.machine,
                candidate,
                &authorization,
                *reference_ms,
                certificate,
                *operation_id,
                device_original,
            )?;
            CommittedOutgoingCandidateV1::from_hardware_commit(
                candidate.clone(),
                certificate.clone(),
            )?;
            let capability = owner
                .machine
                .recover_indexed_outgoing_commit_capability(*operation_id)?;
            owner
                .machine
                .commit_outgoing_candidate(capability, certificate.clone())?;
            owner.committed_authorization = Some(CommitAuthorizationOriginal {
                operation_id: *operation_id,
                hardware_certificate: hardware_certificate.clone(),
                proof: proof.clone(),
                reference_ms: *reference_ms,
                device_original: device_original.clone(),
            });
        }
        Mutation::FinalPayment {
            payment,
            retry_metadata,
        } => {
            let committed = owner.machine.committed_candidate_for_finalization()?;
            let envelope = DurableOutgoingEnvelopeV1::finalize_payment(
                committed,
                payment.clone(),
                retry_metadata.clone(),
                owner.machine.proof_release.artifacts,
                &owner.machine.recursive_verifier,
            )?;
            owner
                .machine
                .install_finalized_outgoing_envelope(envelope)?;
            owner.committed_authorization = None;
        }
        Mutation::FinalRedemption {
            proof,
            retry_metadata,
        } => {
            owner
                .machine
                .finalize_outgoing_redemption(proof.clone(), retry_metadata.clone())?;
            owner.committed_authorization = None;
        }
        Mutation::Incoming { canonical_original } => {
            let original = incoming::decode_incoming_original_v1(canonical_original)?;
            incoming::apply_incoming_original_v1(owner, &original, true)?;
        }
    }
    owner.journals.validate_pair(&owner.machine)?;
    owner
        .transactions
        .recovery_prefix()
        .map_err(material_error)?;
    Ok(())
}

fn material_error(error: impl std::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::RecoveryMaterial(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kagemusha_v1_state::tests::{
        coordinator_operation_store_tests, snapshot_paired_proof,
    };
    use iroha_data_model::kagemusha::KagemushaTrustedCommitTimeV1;

    // Structural evidence fixture only. Accepting test verifiers cannot construct the concrete
    // native owner; these tests cover canonical storage and original-byte custody, not proofs.
    fn original_device_fixture() -> KagemushaOriginalOutgoingHardwareCommitV1 {
        KagemushaOriginalOutgoingHardwareCommitV1 {
            canonical_command: vec![181],
            original_response: vec![182],
        }
    }

    fn completion() -> (KagemushaCommitCertificateV1, TransitionAuthorizationV1) {
        let (machine, _, beneficiary) = coordinator_operation_store_tests::machine();
        let prepared = machine
            .prepare_redeem_split(RedeemSplitPreparationV1 {
                amount: 20,
                beneficiary,
                terminal_nullifier: [61; 32],
                redemption_commitment: [62; 32],
                successor_state_nonce_commitment: [63; 32],
                commit_evidence: KagemushaCommitEvidenceV1::TrustedTime(
                    KagemushaTrustedCommitTimeV1 {
                        time_evidence_commitment: [64; 32],
                    },
                ),
                commit_authorization_reference_ms: 500,
                outbox_reservation: KagemushaOutboxReservationV1 {
                    reservation_id: [65; 32],
                    operation_kind: KagemushaOperationKindV1::RedeemSplit,
                    reserved_outbox_bytes: u32::try_from(
                        crate::kagemusha_v1_state::candidate_lifecycle::implementation_live_outbox_slot_bytes_v1(
                            KagemushaOperationKindV1::RedeemSplit,
                        ).unwrap(),
                    ).unwrap(),
                    issued_at_ms: 100,
                    expires_at_ms: 10000,
                },
                prepared_one_use_authorization_digest: [66; 32],
                sealed_transition_inputs: vec![67],
                sealed_recovery_seeds: vec![68],
            })
            .unwrap();
        let artifacts = machine.proof_release.artifacts;
        let authorization = TransitionAuthorizationV1::new(
            HardwareTransitionCertificateV1 {
                statement: prepared.hardware_statement(),
                guard_bundle: vec![95],
            },
            snapshot_paired_proof(
                prepared.semantic_digest().unwrap(),
                artifacts.eq_protocol_digest,
                artifacts.ep_protocol_digest,
                91,
            ),
        );
        let persisted = PersistedOutgoingCandidateV1::verify_and_persist_redemption(
            prepared,
            authorization.proof.clone(),
            artifacts,
            &crate::kagemusha_v1_state::tests::AcceptSnapshotRecursiveVerifierV1,
        )
        .unwrap();
        let body = persisted.hardware_terminal_body().unwrap();
        let certificate = KagemushaCommitCertificateV1 {
            version: body.version,
            certificate_id: [0; 32],
            candidate_envelope_digest: body.candidate_envelope_digest,
            lifecycle_binding_digest: body.lifecycle_binding_digest,
            transition_nullifier: body.transition_nullifier,
            outbox_reservation_commitment: body.outbox_reservation_commitment,
            commit_evidence: body.commit_evidence,
            hardware_profile_id: body.hardware_profile_id,
            policy_epoch: body.policy_epoch,
            hardware_terminal_commitment: [0; 32],
        }
        .seal_with_terminal_body(&body)
        .unwrap();
        CommittedOutgoingCandidateV1::from_hardware_commit(persisted, certificate.clone()).unwrap();
        (certificate, authorization)
    }

    #[test]
    fn commit_command_wal_replay_bounds_work_and_refuses_extra_frames() {
        // Structural storage only. Real recovery never returns a descriptor on this error;
        // inspecting it here proves that the fifth frame was not read before rejection.
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .canonicalize()
            .unwrap()
            .join("extra-frames");
        let mut journal = PrivateJournal::create_new(&path, TERMINAL_FORMAT).unwrap();
        for value in 1..=5 {
            journal.append(&[value]).unwrap();
        }
        drop(journal);
        let mut reopened = PrivateJournal::open_existing(&path, TERMINAL_FORMAT).unwrap();
        assert!(matches!(
            replay_commit_records(&mut reopened),
            Err(KagemushaStateErrorV1::SnapshotIntegrity)
        ));
        assert!(reopened.recovery_prefix().is_err());
        assert_eq!(reopened.replay_next().unwrap(), Some((4, vec![5])));
    }

    #[test]
    fn commit_command_wal_replay_preserves_complete_and_torn_prefix_admission() {
        // These bytes test framing/order only; no synthetic data grants a Core owner.
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let frames: [&[u8]; 3] = [
            b"structural intent",
            b"structural command",
            b"structural completed",
        ];
        for count in 0..=3 {
            let path = root.join(format!("complete-{count}"));
            let mut journal = PrivateJournal::create_new(&path, TERMINAL_FORMAT).unwrap();
            for frame in frames.iter().take(count) {
                journal.append(frame).unwrap();
            }
            drop(journal);
            let mut reopened = PrivateJournal::open_existing(&path, TERMINAL_FORMAT).unwrap();
            if count == 0 {
                assert!(replay_commit_records(&mut reopened).is_err());
                continue;
            }
            replay_commit_records(&mut reopened).unwrap();
            require_commit_records(
                &reopened,
                frames[0],
                (count >= 2).then_some(frames[1]),
                (count == 3).then_some(frames[2]),
            )
            .unwrap();
        }
        for completed_frames in 0..=3 {
            let path = root.join(format!("torn-{completed_frames}"));
            let mut journal = PrivateJournal::create_new(&path, TERMINAL_FORMAT).unwrap();
            for frame in frames.iter().take(completed_frames) {
                journal.append(frame).unwrap();
            }
            journal.append(b"torn next frame").unwrap();
            drop(journal);
            let file = std::fs::OpenOptions::new()
                .write(true)
                .open(path.join(TERMINAL_FORMAT.filename))
                .unwrap();
            let length = file.metadata().unwrap().len();
            file.set_len(length - 1).unwrap();
            file.sync_all().unwrap();
            drop(file);
            let mut reopened = PrivateJournal::open_existing(&path, TERMINAL_FORMAT).unwrap();
            assert!(replay_commit_records(&mut reopened).is_err());
            assert!(reopened.recovery_prefix().is_err());
        }
    }

    #[test]
    fn commit_command_original_roundtrips_without_granting_authority() {
        // Structural raw bytes only: the shipping retain/recovery path separately authenticates
        // the actual Core signature, native candidate and original operation context.
        let command = vec![101; 96];
        let record = CommitRecord::Command {
            canonical_command: command.clone(),
        };
        let bytes = norito::encode_canonical(&record).unwrap();
        let CommitRecord::Command { canonical_command } = decode_commit_record(&bytes).unwrap()
        else {
            panic!("command custody must preserve its exact closed record variant");
        };
        assert_eq!(canonical_command, command);
        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode_commit_record(&trailing).is_err());
    }

    #[test]
    fn commit_command_wal_retains_exact_order_and_refuses_missing_or_substituted_frames() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .canonicalize()
            .unwrap()
            .join("original-command");
        let intent = b"structural original intent";
        let command = norito::encode_canonical(&CommitRecord::Command {
            canonical_command: vec![102; 96],
        })
        .unwrap();
        let completion = b"structural original completion";
        let mut journal = PrivateJournal::create_new(&path, TERMINAL_FORMAT).unwrap();
        journal.append(intent).unwrap();
        require_commit_records(&journal, intent, None, None).unwrap();
        assert!(require_commit_records(&journal, intent, None, Some(completion)).is_err());
        journal.append(&command).unwrap();
        require_commit_records(&journal, intent, Some(&command), None).unwrap();
        journal.append(completion).unwrap();
        require_commit_records(&journal, intent, Some(&command), Some(completion)).unwrap();
        drop(journal);
        let mut reopened = PrivateJournal::open_existing(&path, TERMINAL_FORMAT).unwrap();
        while reopened.replay_next().unwrap().is_some() {}
        require_commit_records(&reopened, intent, Some(&command), Some(completion)).unwrap();
        let mut substituted = command.clone();
        substituted[0] ^= 1;
        assert!(
            require_commit_records(&reopened, intent, Some(&substituted), Some(completion))
                .is_err()
        );
        // A mismatched full scan poisons that same descriptor; original bytes cannot revive it.
        assert!(
            require_commit_records(&reopened, intent, Some(&command), Some(completion)).is_err()
        );
    }

    #[test]
    fn completion_record_roundtrips_original_evidence_without_an_authorization_owner() {
        let (certificate, authorization) = completion();
        let record = CommitRecord::completion_evidence(
            certificate.clone(),
            &authorization,
            &original_device_fixture(),
            500,
            [71; 32],
            Path::new("/private/checkpoint"),
        )
        .unwrap();
        let bytes = norito::encode_canonical(&record).unwrap();
        let decoded = norito::decode_from_bytes::<CommitRecord>(&bytes).unwrap();
        let CommitRecord::Completed {
            certificate: retained,
            device_original,
            hardware_certificate,
            proof,
            reference_ms,
            checkpoint_operation_id,
            snapshot_directory,
        } = decoded
        else {
            panic!("completion evidence must retain its record kind")
        };
        assert_eq!(retained, certificate);
        assert_eq!(device_original, original_device_fixture());
        assert_eq!(hardware_certificate, authorization.hardware_certificate);
        assert_eq!(proof, authorization.proof);
        assert_eq!(reference_ms, 500);
        assert_eq!(checkpoint_operation_id, [71; 32]);
        assert_eq!(snapshot_directory, "/private/checkpoint");
        assert_eq!(norito::encode_canonical(&record).unwrap(), bytes);
    }

    #[test]
    fn completion_journal_refuses_substituted_certificate_guard_proof_and_reference() {
        let (certificate, authorization) = completion();
        let checkpoint_id = [71; 32];
        let destination = "/private/checkpoint";
        let original = norito::encode_canonical(
            &CommitRecord::completion_evidence(
                certificate.clone(),
                &authorization,
                &original_device_fixture(),
                500,
                checkpoint_id,
                Path::new(destination),
            )
            .unwrap(),
        )
        .unwrap();
        let directory = tempfile::tempdir().unwrap();
        for field in 0..6 {
            let mut journal = PrivateJournal::create_new(
                &directory
                    .path()
                    .canonicalize()
                    .unwrap()
                    .join(format!("completion-{field}")),
                TERMINAL_FORMAT,
            )
            .unwrap();
            journal.append(&original).unwrap();
            journal.require_single_record(&original).unwrap();
            let mut different_certificate = certificate.clone();
            let mut different_authorization = authorization.clone();
            let mut reference_ms = 500;
            let mut checkpoint_id = [71; 32];
            let mut destination = "/private/checkpoint";
            match field {
                0 => different_certificate.hardware_terminal_commitment[0] ^= 1,
                1 => different_authorization.hardware_certificate.guard_bundle[0] ^= 1,
                2 => different_authorization.proof.eq_proof[0] ^= 1,
                3 => reference_ms += 1,
                4 => checkpoint_id[0] ^= 1,
                _ => destination = "/private/substituted",
            }
            let substituted = norito::encode_canonical(
                &CommitRecord::completion_evidence(
                    different_certificate,
                    &different_authorization,
                    &original_device_fixture(),
                    reference_ms,
                    checkpoint_id,
                    Path::new(destination),
                )
                .unwrap(),
            )
            .unwrap();
            assert_ne!(substituted, original);
            assert_eq!(
                journal.require_single_record(&substituted),
                Err(PrivateJournalError::Corrupt),
            );
            // Failed original-byte custody poisons this held owner; even the original bytes
            // cannot revive it. Each mutation therefore uses a fresh exclusive journal.
            assert_eq!(
                journal.require_single_record(&original),
                Err(PrivateJournalError::Uncertain),
            );
        }
    }
}
