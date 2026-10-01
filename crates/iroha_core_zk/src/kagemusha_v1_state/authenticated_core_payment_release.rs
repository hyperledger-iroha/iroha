//! Exclusive native outbox release after a full terminal receipt and original signed op12.
//!
//! Core retains the complete original command before exposing its authorization. The
//! device response and publication destination are synced before capacity changes. No old
//! usable owner escapes an uncertain device exchange or checkpoint publication.

use super::*;
use crate::kagemusha_sender_wire::{
    SENDER_REPLY_MAX_BYTES_V1, SenderCommandBodyV1, SenderCommandV1, SenderHardwareAuthorizationV1,
    SenderPhaseV1, SenderReplyBodyV1, SenderReplyV1, SenderTerminalReceiptV1,
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1, KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1,
    KagemushaHardwareTerminalBodyV1, kagemusha_verify_device_response_v1,
};

const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "outbox-release.norito.wal",
    magic: b"IKGPRW1\0",
    hash_domain: b"iroha:kagemusha:v1:terminal-outbox-release\0",
    maximum_payload_bytes: (redemption_finality::ORIGINAL_MAX_BYTES + 8 * 1024 * 1024) as u64,
};

#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::PaymentReleaseOriginalV1")]
#[allow(variant_size_differences)]
enum TerminalOriginal {
    Payment,
    Redemption(redemption_finality::Original),
}

#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OutboxReleaseOriginalV1")]
#[allow(variant_size_differences)]
enum Record {
    Command {
        previous: KagemushaStateSnapshotV1,
        canonical_command: Vec<u8>,
        terminal_original: TerminalOriginal,
    },
    Completed {
        original_response: Vec<u8>,
        publication_directory: String,
        checkpoint_operation_id: DigestV1,
    },
}

/// Borrowed actual installed payment selected under the concrete current native owner.
/// Its public/private projections cannot create another Core or a release capability.
pub struct KagemushaAuthenticatedPaymentReleaseSelectionV1<'a> {
    owner: &'a KagemushaAuthenticatedCoreOwnerV1,
    operation_id: DigestV1,
    acknowledgement: Vec<u8>,
}

/// Exclusive actual owner awaiting its exact signed release and complete checkpoint CAS.
/// Only original retry material is exposed while this value exists.
pub struct KagemushaAuthenticatedOutboxReleaseV1 {
    owner: KagemushaAuthenticatedCoreOwnerV1,
    operation_id: DigestV1,
    previous: KagemushaStateSnapshotV1,
    canonical_command: Vec<u8>,
    terminal_original: TerminalOriginal,
    original_record: Vec<u8>,
    journal: PrivateJournal,
    completed: Option<(Vec<u8>, Vec<u8>, String, DigestV1)>,
    applied: Option<KagemushaStateSnapshotV1>,
}

impl KagemushaAuthenticatedPaymentReleaseSelectionV1<'_> {
    /// Reauthenticate the exact installed terminal envelope and original signed receiver ACK.
    ///
    /// # Errors
    /// Rejects stale original custody, foreign operations, invalid proofs or invalid ACK signatures.
    pub fn recheck(&self) -> Result<(), KagemushaStateErrorV1> {
        self.owner.current_recovery_selection()?;
        selected_payment(
            &self.owner.machine,
            self.operation_id,
            &self.acknowledgement,
        )?;
        self.owner.current_recovery_selection()?;
        Ok(())
    }

    /// Borrow the actual native preparation for the fixed Core-to-hardware release signer.
    ///
    /// # Errors
    /// Rejects changed native originals or unavailable current checkpoint authentication.
    pub fn prepared(&self) -> Result<&PreparedOutgoingCandidateV1, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(&selected_payment(
            &self.owner.machine,
            self.operation_id,
            &self.acknowledgement,
        )?
        .0
        .committed
        .candidate
        .prepared)
    }

    /// Recompute the original terminal nullifier, outcome and reservation for the fixed signer.
    /// These values come from the actual installed native candidate, never caller envelope fields.
    ///
    /// # Errors
    /// Rejects stale native originals, invalid ACKs or inconsistent terminal preparation.
    pub fn hardware_terminal_body(
        &self,
    ) -> Result<KagemushaHardwareTerminalBodyV1, KagemushaStateErrorV1> {
        self.recheck()?;
        let body = selected_payment(
            &self.owner.machine,
            self.operation_id,
            &self.acknowledgement,
        )?
        .0
        .committed
        .candidate
        .hardware_terminal_body()?;
        self.recheck()?;
        Ok(body)
    }

    /// Copy the exact installed native operation record, never a caller-projected phase.
    ///
    /// # Errors
    /// Rejects lost native custody, missing operations or a substituted acknowledgement.
    pub fn record(&self) -> Result<KagemushaOutgoingOperationRecordV1, KagemushaStateErrorV1> {
        self.recheck()?;
        self.owner
            .machine
            .outgoing_operation_index()
            .lookup(self.operation_id)
            .cloned()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)
    }

    /// Copy the genuine byte-identical payment still retained in the native outbox.
    ///
    /// # Errors
    /// Rejects stale custody, released operations or invalid terminal proof material.
    pub fn canonical_envelope(&self) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.recheck()?;
        self.owner.original_terminal_envelope(self.operation_id)
    }

    /// Return the independently verified ACK digest used by the original op12 authorization.
    ///
    /// # Errors
    /// Rejects changed held originals or a malformed, foreign or invalid signed ACK.
    pub fn terminal_receipt_digest(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(selected_payment(
            &self.owner.machine,
            self.operation_id,
            &self.acknowledgement,
        )?
        .1)
    }
}

impl KagemushaAuthenticatedCoreOwnerV1 {
    /// Verify an exact completed release without consuming or replacing this installed owner.
    /// Both native WAL records must match the expected original command, response and destination;
    /// the current authenticated native index must contain that exact Released tombstone.
    /// This verifies retries under the same accepted qualification. Retained public credential
    /// fields cannot authenticate a historical response after native qualification rotation.
    /// TODO: authenticate rotated-history retries only from genuine retained signed native custody.
    ///
    /// # Errors
    /// Rejects pending, substituted, corrupt or foreign originals, unavailable current custody,
    /// historical qualification changes and any command/response/destination mismatch.
    pub fn verify_completed_outbox_release_existing(
        &self,
        directory: &Path,
        publication_directory: &Path,
        checkpoint_operation_id: DigestV1,
        operation_id: DigestV1,
        canonical_command: &[u8],
        original_response: &[u8],
    ) -> Result<(), KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        require_completed_operation(canonical_command, operation_id)?;
        let mut journal =
            PrivateJournal::open_existing(directory, FORMAT).map_err(material_error)?;
        let (command_record, completion_record, terminal_original) = completed_release_original(
            &mut journal,
            publication_directory,
            checkpoint_operation_id,
            canonical_command,
            original_response,
        )?;
        verify_released_original(
            &self.machine,
            canonical_command,
            original_response,
            &terminal_original,
        )?;
        self.current_recovery_selection()?;
        require_records(&journal, &command_record, Some(&completion_record))?;
        Ok(())
    }

    /// Recover a completed release through its exact original checkpoint publication.
    /// An already selected successor is freshly authenticated and its released native tombstone
    /// must match the original signed op12. Retained bytes cannot create another release.
    ///
    /// # Errors
    /// Rejects incomplete or changed original WALs, foreign destinations, mutations or signatures.
    pub fn recover_payment_release_checkpoint_existing(
        directory: &Path,
        publication_directory: &Path,
        inputs: KagemushaAuthenticatedCoreRecoveryInputsV1<'_>,
        expected_checkpoint_operation_id: DigestV1,
    ) -> Result<(KagemushaAuthenticatedCoreRecoveryV1, Vec<u8>), KagemushaStateErrorV1> {
        Self::recover_release_checkpoint_existing(
            directory,
            publication_directory,
            inputs,
            expected_checkpoint_operation_id,
            false,
        )
    }

    /// Recover an original completed redemption release and its complete native checkpoint.
    /// Full retained consensus evidence and the exact signed op12 remain mandatory.
    ///
    /// # Errors
    /// Rejects absent or changed full originals, foreign destinations or invalid signatures.
    pub fn recover_redemption_release_checkpoint_existing(
        directory: &Path,
        publication_directory: &Path,
        inputs: KagemushaAuthenticatedCoreRecoveryInputsV1<'_>,
        expected_checkpoint_operation_id: DigestV1,
    ) -> Result<(KagemushaAuthenticatedCoreRecoveryV1, Vec<u8>), KagemushaStateErrorV1> {
        Self::recover_release_checkpoint_existing(
            directory,
            publication_directory,
            inputs,
            expected_checkpoint_operation_id,
            true,
        )
    }

    fn recover_release_checkpoint_existing(
        directory: &Path,
        publication_directory: &Path,
        inputs: KagemushaAuthenticatedCoreRecoveryInputsV1<'_>,
        expected_checkpoint_operation_id: DigestV1,
        expect_redemption: bool,
    ) -> Result<(KagemushaAuthenticatedCoreRecoveryV1, Vec<u8>), KagemushaStateErrorV1> {
        let mut journal =
            PrivateJournal::open_existing(directory, FORMAT).map_err(material_error)?;
        replay_release_records(&mut journal)?;
        let mut frames = Vec::new();
        journal
            .scan_complete(|sequence, bytes| {
                if sequence > 1 {
                    return Err(PrivateJournalError::Corrupt);
                }
                frames.push(bytes.to_vec());
                Ok(())
            })
            .map_err(material_error)?;
        if frames.len() != 2 {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let Record::Command {
            previous,
            canonical_command,
            terminal_original,
        } = decode_record(&frames[0])?
        else {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        };
        require_terminal_kind(&terminal_original, expect_redemption)?;
        let Record::Completed {
            original_response,
            publication_directory: retained_directory,
            checkpoint_operation_id,
        } = decode_record(&frames[1])?
        else {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        };
        if publication_directory.to_str() != Some(retained_directory.as_str())
            || checkpoint_operation_id != expected_checkpoint_operation_id
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        require_records(&journal, &frames[0], Some(&frames[1]))?;
        let mutation = Mutation::OutboxRelease {
            canonical_command: canonical_command.clone(),
            original_response: original_response.clone(),
            canonical_terminal_original: encode_terminal_original(&terminal_original)?,
        };
        let recovery = Self::recover_checkpoint_original(
            publication_directory,
            inputs,
            Some(&previous),
            Some(&mutation),
        )?;
        recovery.require_original_operation(expected_checkpoint_operation_id)?;
        if let KagemushaAuthenticatedCoreRecoveryV1::Selected(owner) = &recovery {
            owner.current_recovery_selection()?;
            verify_released_original(
                &owner.machine,
                &canonical_command,
                &original_response,
                &terminal_original,
            )?;
            owner.current_recovery_selection()?;
        }
        require_records(&journal, &frames[0], Some(&frames[1]))?;
        Ok((recovery, original_response))
    }

    /// Reopen an existing original release under the same freshly selected native predecessor.
    /// The decoded snapshot is compared with this owner; it cannot construct one. A retained
    /// completion additionally requires its independently expected publication destination.
    ///
    /// # Errors
    /// Rejects changed or extra frames, foreign predecessors, substituted destinations or signatures.
    pub fn recover_payment_release_existing(
        self,
        directory: &Path,
        expected_publication_directory: &Path,
        expected_checkpoint_operation_id: DigestV1,
    ) -> Result<KagemushaAuthenticatedOutboxReleaseV1, KagemushaStateErrorV1> {
        self.recover_release_existing(
            directory,
            expected_publication_directory,
            expected_checkpoint_operation_id,
            false,
        )
    }

    /// Recover the exact uncertain redemption op12 under its freshly selected predecessor.
    /// Full signed finality originals are independently reverified before returning retry custody.
    ///
    /// # Errors
    /// Rejects substituted original evidence, predecessors, destination or response signatures.
    pub fn recover_redemption_release_existing(
        self,
        directory: &Path,
        expected_publication_directory: &Path,
        expected_checkpoint_operation_id: DigestV1,
    ) -> Result<KagemushaAuthenticatedOutboxReleaseV1, KagemushaStateErrorV1> {
        self.recover_release_existing(
            directory,
            expected_publication_directory,
            expected_checkpoint_operation_id,
            true,
        )
    }

    fn recover_release_existing(
        self,
        directory: &Path,
        expected_publication_directory: &Path,
        expected_checkpoint_operation_id: DigestV1,
        expect_redemption: bool,
    ) -> Result<KagemushaAuthenticatedOutboxReleaseV1, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let mut journal =
            PrivateJournal::open_existing(directory, FORMAT).map_err(material_error)?;
        replay_release_records(&mut journal)?;
        let mut frames = Vec::new();
        journal
            .scan_complete(|sequence, bytes| {
                if sequence > 1 {
                    return Err(PrivateJournalError::Corrupt);
                }
                frames.push(bytes.to_vec());
                Ok(())
            })
            .map_err(material_error)?;
        let first = frames
            .first()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let Record::Command {
            previous,
            canonical_command,
            terminal_original,
        } = decode_record(first)?
        else {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        };
        require_terminal_kind(&terminal_original, expect_redemption)?;
        if self.selected_predecessor_snapshot()? != previous {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        let command = verify_command(&self.machine, &canonical_command, &terminal_original)?;
        let completed = if let Some(bytes) = frames.get(1) {
            let Record::Completed {
                original_response,
                publication_directory,
                checkpoint_operation_id,
            } = decode_record(bytes)?
            else {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            };
            if expected_publication_directory.to_str() != Some(publication_directory.as_str())
                || expected_checkpoint_operation_id != checkpoint_operation_id
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            verify_response(
                &self.machine,
                &canonical_command,
                &original_response,
                &terminal_original,
            )?;
            Some((
                bytes.clone(),
                original_response,
                publication_directory,
                checkpoint_operation_id,
            ))
        } else {
            None
        };
        let retained = KagemushaAuthenticatedOutboxReleaseV1 {
            owner: self,
            operation_id: command.operation_id,
            previous,
            canonical_command,
            terminal_original,
            original_record: first.clone(),
            journal,
            completed,
            applied: None,
        };
        retained.recheck_originals()?;
        Ok(retained)
    }

    /// Select an actual installed payment after verifying its receiver's original signed ACK.
    /// A decoded ACK alone grants no release or hardware permission.
    ///
    /// # Errors
    /// Rejects foreign or unfinished operations, lost current custody or invalid ACK signatures.
    pub fn payment_release_selection(
        &self,
        operation_id: DigestV1,
        canonical_acknowledgement: &[u8],
    ) -> Result<KagemushaAuthenticatedPaymentReleaseSelectionV1<'_>, KagemushaStateErrorV1> {
        if canonical_acknowledgement.is_empty()
            || canonical_acknowledgement.len() > KAGEMUSHA_ACKNOWLEDGEMENT_MAX_BYTES_V1
        {
            return Err(KagemushaStateErrorV1::InvalidAcknowledgement);
        }
        let selection = KagemushaAuthenticatedPaymentReleaseSelectionV1 {
            owner: self,
            operation_id,
            acknowledgement: canonical_acknowledgement.to_vec(),
        };
        selection.recheck()?;
        Ok(selection)
    }

    /// Consume the actual owner and fsync its exact independently verified op12 before exposure.
    /// This changes no capacity and returns no usable monetary owner.
    ///
    /// # Errors
    /// Rejects invalid original signatures or bindings, stale custody and any uncertain storage.
    pub fn prepare_payment_release(
        self,
        directory: &Path,
        canonical_command: &[u8],
    ) -> Result<KagemushaAuthenticatedOutboxReleaseV1, KagemushaStateErrorV1> {
        self.prepare_terminal_release(directory, canonical_command, TerminalOriginal::Payment)
    }

    /// Retain both full finality archives and the exact signed redemption op12 before exposure.
    /// Native provisioning independently owns `pins`; compact selectors alone are refused.
    ///
    /// # Errors
    /// Rejects missing or invalid full finality, foreign native vouchers or uncertain storage.
    pub fn prepare_redemption_release(
        self,
        directory: &Path,
        canonical_command: &[u8],
        canonical_status: &[u8],
        canonical_bootstrap: &[u8],
        pins: iroha_data_model::kagemusha::KagemushaMobileBootstrapPinsV1<'_>,
    ) -> Result<KagemushaAuthenticatedOutboxReleaseV1, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let command = decode_command(canonical_command)?;
        let original = redemption_finality::capture_original(
            &self.machine,
            command.operation_id,
            canonical_status,
            canonical_bootstrap,
            pins,
        )?;
        self.current_recovery_selection()?;
        self.prepare_terminal_release(
            directory,
            canonical_command,
            TerminalOriginal::Redemption(original),
        )
    }

    fn prepare_terminal_release(
        self,
        directory: &Path,
        canonical_command: &[u8],
        terminal_original: TerminalOriginal,
    ) -> Result<KagemushaAuthenticatedOutboxReleaseV1, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let command = verify_command(&self.machine, canonical_command, &terminal_original)?;
        let previous = self.selected_predecessor_snapshot()?;
        let original_record = encode_record(&Record::Command {
            previous: previous.clone(),
            canonical_command: canonical_command.to_vec(),
            terminal_original: terminal_original.clone(),
        })?;
        let mut journal = PrivateJournal::create_new(directory, FORMAT).map_err(material_error)?;
        journal.append(&original_record).map_err(material_error)?;
        journal
            .require_single_record(&original_record)
            .map_err(material_error)?;
        self.current_recovery_selection()?;
        Ok(KagemushaAuthenticatedOutboxReleaseV1 {
            owner: self,
            operation_id: command.operation_id,
            previous,
            canonical_command: canonical_command.to_vec(),
            terminal_original,
            original_record,
            journal,
            completed: None,
            applied: None,
        })
    }
}

impl KagemushaAuthenticatedOutboxReleaseV1 {
    /// Return this actual retained WAL's original native directory for later exact retry lookup.
    /// The path creates no capability: a completed retry must reopen and verify its full WAL
    /// against the current installed owner and actual Released record.
    ///
    /// # Errors
    /// Rejects changed original custody, substituted paths or unavailable current hardware.
    pub fn original_intent_directory(&self) -> Result<std::path::PathBuf, KagemushaStateErrorV1> {
        self.recheck_originals()?;
        let directory = self.journal.original_directory().map_err(material_error)?;
        self.recheck_originals()?;
        Ok(directory)
    }

    /// Copy the immutable enrollment from this retained original owner for recovery correlation.
    /// This projection grants neither a selected owner nor a new hardware operation.
    ///
    /// # Errors
    /// Rejects changed original custody or unavailable fresh predecessor authentication.
    pub fn retained_enrollment_binding(
        &self,
    ) -> Result<KagemushaRecoveryEnrollmentBindingV1, KagemushaStateErrorV1> {
        self.recheck_originals()?;
        let enrollment = self.owner.machine.enrollment_binding().clone();
        if enrollment != self.previous.recovery_metadata.enrollment {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.recheck_originals()?;
        Ok(enrollment)
    }

    /// Copy the actual originally held predecessor certificate, including after completion.
    /// Only its immutable recovery correlation may be used while publication is pending.
    ///
    /// # Errors
    /// Rejects changed native originals, a different predecessor or lost fresh custody.
    pub fn retained_predecessor_checkpoint(
        &self,
    ) -> Result<DurabilityAnchorV1, KagemushaStateErrorV1> {
        self.recheck_originals()?;
        let checkpoint = self
            .owner
            .machine
            .published_checkpoint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotRollback)?
            .anchor
            .clone();
        if checkpoint.statement != self.previous.recovery_anchor() {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        self.recheck_originals()?;
        Ok(checkpoint)
    }

    /// Current observation scope from the retained actual owner for the exact op12 retry.
    /// The command retains its historical creation context; this getter cannot rewrite it.
    ///
    /// # Errors
    /// Rejects completed releases, changed original custody or unavailable current hardware.
    pub fn sender_context(
        &self,
    ) -> Result<crate::kagemusha_sender_wire::SenderWalletContextV1, KagemushaStateErrorV1> {
        if self.completed.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.recheck_originals()?;
        let context = self.owner.sender_context()?;
        self.recheck_originals()?;
        Ok(context)
    }

    /// Borrow the actual selected predecessor for the exact original op12 observation only.
    /// Completion closes this route; the token cannot yield a usable Core or another operation.
    ///
    /// # Errors
    /// Rejects completed releases, changed originals or unavailable current hardware selection.
    pub fn current_recovery_selection(
        &self,
    ) -> Result<KagemushaCurrentRecoverySelectionV1<'_>, KagemushaStateErrorV1> {
        if self.completed.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.recheck_originals()?;
        self.owner.current_recovery_selection()
    }

    /// Return the independently authenticated immutable release under this original custody.
    /// This catalog grants no fresh hardware session or monetary permission by itself.
    ///
    /// # Errors
    /// Rejects changed retained originals or lost current native custody.
    pub fn authenticated_release(
        &self,
    ) -> Result<Arc<KagemushaAuthenticatedReleaseV1>, KagemushaStateErrorV1> {
        self.recheck_originals()?;
        let release = self.owner.authenticated_release()?;
        self.recheck_originals()?;
        Ok(release)
    }

    /// Exact native operation retained by this exclusive original release.
    ///
    /// # Errors
    /// Rejects changed original journal custody or unavailable current hardware selection.
    pub fn operation_id(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.recheck_originals()?;
        Ok(self.operation_id)
    }

    /// Return the already retained completion bytes and exact destination for checkpoint retry.
    /// The returned projection cannot create a capability or release another envelope.
    ///
    /// # Errors
    /// Rejects changed original records or lost fresh predecessor custody.
    pub fn completed_original(
        &self,
    ) -> Result<Option<(Vec<u8>, String, DigestV1)>, KagemushaStateErrorV1> {
        self.recheck_originals()?;
        Ok(self
            .completed
            .as_ref()
            .map(|(_, response, directory, id)| (response.clone(), directory.clone(), *id)))
    }

    /// Borrow only the exact already synced original command for an uncertain device retry.
    /// Completion closes this dispatch path; it never creates a replacement nonce or signature.
    ///
    /// # Errors
    /// Rejects completed operations, changed native originals or lost hardware selection.
    pub fn original_command(&self) -> Result<&[u8], KagemushaStateErrorV1> {
        if self.completed.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.recheck_originals()?;
        verify_command(
            &self.owner.machine,
            &self.canonical_command,
            &self.terminal_original,
        )?;
        self.recheck_originals()?;
        Ok(&self.canonical_command)
    }

    /// Recheck retained bytes, held journal ownership and the freshly challenged original cut.
    /// This is exact-retry custody, never permission for another monetary operation.
    ///
    /// # Errors
    /// Rejects changed descriptors, substituted snapshots or unavailable current hardware selection.
    pub fn recheck_originals(&self) -> Result<(), KagemushaStateErrorV1> {
        require_records(
            &self.journal,
            &self.original_record,
            self.completed.as_ref().map(|value| value.0.as_slice()),
        )?;
        self.owner.journals.validate_pair(&self.owner.machine)?;
        self.owner
            .transactions
            .recovery_prefix()
            .map_err(material_error)?;
        if let Some(original) = &self.owner.selected_publication {
            original.require_selected(&self.owner.machine)?;
        }
        if let Some(expected) = &self.applied {
            if &self.owner.machine.snapshot_with_history_checkpoint(Some(
                self.previous.authenticated_history_commitment,
            ))? != expected
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        } else {
            self.owner.current_recovery_selection()?;
            if self.owner.selected_predecessor_snapshot()? != self.previous {
                return Err(KagemushaStateErrorV1::SnapshotRollback);
            }
        }
        self.owner
            .machine
            .guard_verifier
            .verify_current_recovery_checkpoint(
                &self.previous.recovery_anchor(),
                &self.previous.recovery_metadata.journals,
            )
            .map_err(material_error)?;
        require_records(
            &self.journal,
            &self.original_record,
            self.completed.as_ref().map(|value| value.0.as_slice()),
        )?;
        self.owner.journals.validate_pair(&self.owner.machine)?;
        self.owner
            .transactions
            .recovery_prefix()
            .map_err(material_error)?;
        Ok(())
    }

    /// Independently verify and retain the complete original signed op12 before releasing space.
    /// A publication failure retains this same exclusive owner and exact original response.
    /// An uncertain journal append poisons its descriptor; close and reopen the surviving exact
    /// prefix before recovery. Retaining the value does not make that descriptor retryable.
    ///
    /// # Errors
    /// Retains this exclusive value for invalid or conflicting evidence, storage or hardware failure.
    #[allow(clippy::result_large_err)]
    pub fn complete_or_retain(
        mut self,
        original_response: &[u8],
        publication_directory: &Path,
        checkpoint_operation_id: DigestV1,
    ) -> Result<KagemushaAuthenticatedCorePublicationV1, (Self, KagemushaStateErrorV1)> {
        let attempt = (|| {
            self.recheck_originals()?;
            let directory = publication_directory
                .to_str()
                .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                .to_owned();
            if directory.is_empty() || checkpoint_operation_id == [0; 32] {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            if let Some((_, response, retained_directory, retained_id)) = &self.completed {
                if response != original_response
                    || retained_directory != &directory
                    || *retained_id != checkpoint_operation_id
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
            } else {
                verify_response(
                    &self.owner.machine,
                    &self.canonical_command,
                    original_response,
                    &self.terminal_original,
                )?;
                let bytes = encode_record(&Record::Completed {
                    original_response: original_response.to_vec(),
                    publication_directory: directory.clone(),
                    checkpoint_operation_id,
                })?;
                // Pin these bytes before any uncertain append; no alternate result can replace them.
                self.completed = Some((
                    bytes.clone(),
                    original_response.to_vec(),
                    directory,
                    checkpoint_operation_id,
                ));
                self.journal.append(&bytes).map_err(material_error)?;
            }
            require_records(
                &self.journal,
                &self.original_record,
                self.completed.as_ref().map(|value| value.0.as_slice()),
            )?;
            if self.applied.is_none() {
                apply_original(
                    &mut self.owner,
                    &self.canonical_command,
                    original_response,
                    &encode_terminal_original(&self.terminal_original)?,
                )?;
                self.applied = Some(self.owner.machine.snapshot_with_history_checkpoint(Some(
                    self.previous.authenticated_history_commitment,
                ))?);
            }
            self.recheck_originals()?;
            self.owner.build_checkpoint_original(
                publication_directory,
                checkpoint_operation_id,
                self.previous.clone(),
                Mutation::OutboxRelease {
                    canonical_command: self.canonical_command.clone(),
                    original_response: original_response.to_vec(),
                    canonical_terminal_original: encode_terminal_original(&self.terminal_original)?,
                },
            )
        })();
        match attempt {
            Ok(parts) => Ok(parts.into_publication(self.owner)),
            Err(error) => Err((self, error)),
        }
    }
}

fn selected_payment<'a, R, G, H>(
    machine: &'a KagemushaStateMachineV1<R, G, H>,
    operation_id: DigestV1,
    acknowledgement: &[u8],
) -> Result<(&'a DurableOutgoingEnvelopeV1, DigestV1), KagemushaStateErrorV1>
where
    R: KagemushaRecursiveVerifierV1,
    G: KagemushaGuardBundleVerifierV1,
    H: KagemushaAuthenticatedHistoryStoreV1,
{
    machine.original_terminal_envelope(operation_id)?;
    let record = machine
        .outgoing_operation_index()
        .lookup(operation_id)
        .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
    if record.phase != KagemushaOutgoingOperationPhaseV1::Installed {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    }
    let envelope = machine
        .outgoing_candidate_journal
        .finalized_envelope(record.outbox_reservation_id)
        .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
    let KagemushaOutgoingEnvelopeV1::Payment(payment) = &envelope.envelope else {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    };
    let PreparedOutgoingRecoveryViewV1::Send { request, .. } =
        envelope.committed.candidate.prepared.recovery_view()
    else {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    };
    KagemushaAcknowledgementV1::decode_canonical_shape_exact_against(
        acknowledgement,
        request,
        payment,
    )
    .map_err(|_| KagemushaStateErrorV1::InvalidAcknowledgement)?;
    let digest = crate::kagemusha_sender_wire::acknowledgement_digest_v1(acknowledgement)
        .map_err(|_| KagemushaStateErrorV1::InvalidAcknowledgement)?;
    Ok((envelope, digest))
}

fn selected_installed_envelope(
    machine: &Machine,
    operation_id: DigestV1,
) -> Result<&DurableOutgoingEnvelopeV1, KagemushaStateErrorV1> {
    machine.original_terminal_envelope(operation_id)?;
    let record = machine
        .outgoing_operation_index()
        .lookup(operation_id)
        .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
    if record.phase != KagemushaOutgoingOperationPhaseV1::Installed {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    }
    machine
        .outgoing_candidate_journal
        .finalized_envelope(record.outbox_reservation_id)
        .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)
}

fn require_terminal_kind(
    original: &TerminalOriginal,
    expect_redemption: bool,
) -> Result<(), KagemushaStateErrorV1> {
    if matches!(original, TerminalOriginal::Redemption(_)) != expect_redemption {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    }
    Ok(())
}

fn encode_terminal_original(original: &TerminalOriginal) -> Result<Vec<u8>, KagemushaStateErrorV1> {
    let bytes = norito::encode_canonical(original).map_err(material_error)?;
    if bytes.len() > redemption_finality::ORIGINAL_MAX_BYTES {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(bytes)
}

fn decode_terminal_original(bytes: &[u8]) -> Result<TerminalOriginal, KagemushaStateErrorV1> {
    if bytes.is_empty() || bytes.len() > redemption_finality::ORIGINAL_MAX_BYTES {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
        .map_err(material_error)
}

fn verify_command(
    machine: &Machine,
    bytes: &[u8],
    terminal_original: &TerminalOriginal,
) -> Result<SenderCommandV1, KagemushaStateErrorV1> {
    let command = decode_command(bytes)?;
    let SenderCommandBodyV1::Release {
        inputs_digest,
        envelope_digest,
        envelope,
        terminal_receipt,
        hardware_authorization,
        ..
    } = &command.body
    else {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    };
    let (actual, receipt_digest) = match (terminal_original, terminal_receipt) {
        (TerminalOriginal::Payment, SenderTerminalReceiptV1::PaymentAcknowledgement(ack)) => {
            selected_payment(machine, command.operation_id, ack)?
        }
        (
            TerminalOriginal::Redemption(original),
            SenderTerminalReceiptV1::RedemptionSettlement(receipt),
        ) => {
            let expected = original.authenticate(machine, command.operation_id)?;
            if *receipt != expected {
                return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
            }
            let actual = selected_installed_envelope(machine, command.operation_id)?;
            (actual, expected.canonical_digest()?)
        }
        _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
    };
    let record = machine
        .outgoing_operation_index()
        .lookup(command.operation_id)
        .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
    let authorization =
        SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization)
            .map_err(material_error)?;
    let prepared = &actual.committed.candidate.prepared;
    let terminal = actual.committed.candidate.hardware_terminal_body()?;
    if command.context != record.context
        || inputs_digest != &record.inputs_digest
        || envelope_digest != &actual.envelope_digest
        || envelope != &actual.canonical_envelope_bytes
        || record.envelope_digest != Some(actual.envelope_digest)
        || record.context.core_authorization_key_reference
            != machine
                .enrollment_binding()
                .core_authorization_key_reference
        || authorization.hardware_transition_statement != prepared.hardware_statement()
        || authorization.preparation_id != prepared.preparation_id
        || authorization.prepared_one_use_authorization_digest
            != prepared.prepared_one_use_authorization_digest
        || authorization.outbox_reservation_commitment != terminal.outbox_reservation_commitment
        || authorization.terminal_receipt_digest != Some(receipt_digest)
        || authorization.hardware_one_use_nonce == [0; 32]
    {
        return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
    }
    Ok(command)
}

fn decode_command(bytes: &[u8]) -> Result<SenderCommandV1, KagemushaStateErrorV1> {
    // Decode with bounded exact framing before using any projected identity.
    let command: SenderCommandV1 = norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(16 * 1024, 16 * 1024, 64 * 1024, 128 * 1024, 32),
    )
    .map_err(material_error)?;
    SenderCommandV1::decode_canonical_exact(12, command.operation_id, bytes).map_err(material_error)
}

fn verify_released_original(
    machine: &Machine,
    command: &[u8],
    original_response: &[u8],
    terminal_original: &TerminalOriginal,
) -> Result<(), KagemushaStateErrorV1> {
    let command = decode_command(command)?;
    let SenderCommandBodyV1::Release {
        inputs_digest,
        envelope_digest,
        terminal_receipt,
        envelope,
        hardware_authorization,
        ..
    } = &command.body
    else {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    };
    let record = machine
        .outgoing_operation_index()
        .lookup(command.operation_id)
        .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
    let authorization =
        SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization)
            .map_err(material_error)?;
    record.validate().map_err(material_error)?;
    record
        .context
        .validate_retained_against_state(&machine.state)
        .map_err(material_error)?;
    if record.phase != KagemushaOutgoingOperationPhaseV1::Released
        || command.context != record.context
        || inputs_digest != &record.inputs_digest
        || record.envelope_digest != Some(*envelope_digest)
        || record.context.core_authorization_key_reference
            != machine
                .enrollment_binding()
                .core_authorization_key_reference
        || authorization.preparation_id != record.preparation_id
        || Some(authorization.candidate_digest) != record.candidate_digest
        || authorization.outcome_id != record.outcome_id
        || authorization.terminal_receipt_digest != record.terminal_receipt_digest
    {
        return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
    }
    match (terminal_original, terminal_receipt) {
        (TerminalOriginal::Payment, SenderTerminalReceiptV1::PaymentAcknowledgement(_)) => {}
        (
            TerminalOriginal::Redemption(original),
            SenderTerminalReceiptV1::RedemptionSettlement(receipt),
        ) => {
            let expected =
                original.authenticate_released(machine, command.operation_id, envelope)?;
            if *receipt != expected
                || Some(expected.canonical_digest()?) != record.terminal_receipt_digest
            {
                return Err(KagemushaStateErrorV1::InvalidRedemptionSettlementReceipt);
            }
        }
        _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
    }
    verify_device_reply(machine, &command, original_response)
}

fn verify_response(
    machine: &Machine,
    command: &[u8],
    original_response: &[u8],
    terminal_original: &TerminalOriginal,
) -> Result<(), KagemushaStateErrorV1> {
    let command = verify_command(machine, command, terminal_original)?;
    verify_device_reply(machine, &command, original_response)
}

fn verify_device_reply(
    machine: &Machine,
    command: &SenderCommandV1,
    original_response: &[u8],
) -> Result<(), KagemushaStateErrorV1> {
    let release = machine
        .guard_verifier
        .authenticated_release()
        .map_err(material_error)?;
    let credential = &machine.recovery_metadata.accepted_credential.credential;
    let profile = release
        .enabled_profile(credential.hardware_profile_id)
        .ok_or(KagemushaStateErrorV1::HardwareCertificateMismatch)?;
    let encoded = command.encode_canonical().map_err(material_error)?;
    let response = kagemusha_verify_device_response_v1(
        original_response,
        &encoded,
        12,
        command.operation_id,
        release.hardware_policy_digest(),
        profile.hardware_profile.qualification_report_digest,
        &credential.device_public_key,
    )
    .map_err(material_error)?;
    let reply: SenderReplyV1 = norito::decode_canonical_with_limits(
        response.payload,
        norito::DecodeLimits::new(
            SENDER_REPLY_MAX_BYTES_V1,
            SENDER_REPLY_MAX_BYTES_V1,
            SENDER_REPLY_MAX_BYTES_V1 * 4,
            SENDER_REPLY_MAX_BYTES_V1 * 8,
            32,
        ),
    )
    .map_err(material_error)?;
    let state = &machine.state;
    let current_context = crate::kagemusha_sender_wire::SenderWalletContextV1 {
        lane: state.lane.clone(),
        release: state.context(),
        credential_id: credential.credential_id,
        hardware_epoch: state.hardware_epoch,
        device_policy_binding: state.device_policy_binding,
        core_authorization_key_reference: machine
            .enrollment_binding()
            .core_authorization_key_reference,
    };
    reply
        .validate_against(command, &current_context)
        .map_err(material_error)?;
    let SenderReplyBodyV1::Lookup(Some(item)) = &reply.body else {
        return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
    };
    let actual = machine
        .outgoing_operation_index()
        .lookup(command.operation_id)
        .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
    let authorization = match &command.body {
        SenderCommandBodyV1::Release {
            hardware_authorization,
            ..
        } => SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization)
            .map_err(material_error)?,
        _ => return Err(KagemushaStateErrorV1::HardwareCertificateMismatch),
    };
    let item = &item.record;
    if item.phase != SenderPhaseV1::Released
        || item.operation_id != actual.operation_id
        || item.context != actual.context
        || item.inputs_digest != actual.inputs_digest
        || item.operation_kind != actual.operation_kind
        || item.preparation_id != actual.preparation_id
        || item.outbox_reservation_id != actual.outbox_reservation_id
        || item.outcome_id != actual.outcome_id
        || item.candidate_digest != actual.candidate_digest
        || item.commit_certificate_digest != actual.commit_certificate_digest
        || item.envelope_digest != actual.envelope_digest
        || item.terminal_receipt_digest != authorization.terminal_receipt_digest
    {
        return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
    }
    Ok(())
}

pub(super) fn apply_original(
    owner: &mut KagemushaAuthenticatedCoreOwnerV1,
    command: &[u8],
    response: &[u8],
    canonical_terminal_original: &[u8],
) -> Result<(), KagemushaStateErrorV1> {
    let terminal_original = decode_terminal_original(canonical_terminal_original)?;
    verify_response(&owner.machine, command, response, &terminal_original)?;
    let command = verify_command(&owner.machine, command, &terminal_original)?;
    let record = owner
        .machine
        .outgoing_operation_index()
        .lookup(command.operation_id)
        .cloned()
        .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
    let SenderCommandBodyV1::Release {
        hardware_authorization,
        ..
    } = &command.body
    else {
        return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
    };
    let authorization =
        SenderHardwareAuthorizationV1::decode_canonical_exact(hardware_authorization)
            .map_err(material_error)?;
    owner
        .machine
        .outgoing_candidate_journal
        .release_verified_terminal(
            &mut owner.machine.sender_outbox_capacity,
            record.outbox_reservation_id,
            record
                .envelope_digest
                .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?,
            authorization
                .terminal_receipt_digest
                .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?,
        )
}

fn encode_record(record: &Record) -> Result<Vec<u8>, KagemushaStateErrorV1> {
    let bytes = norito::encode_canonical(record).map_err(material_error)?;
    if bytes.is_empty() || bytes.len() as u64 > FORMAT.maximum_payload_bytes {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(bytes)
}

fn decode_record(bytes: &[u8]) -> Result<Record, KagemushaStateErrorV1> {
    let maximum = FORMAT.maximum_payload_bytes as usize;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let record: Record = norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(maximum, maximum, maximum * 4, maximum * 8, 32),
    )
    .map_err(material_error)?;
    if encode_record(&record)? != bytes {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    match &record {
        Record::Command {
            canonical_command, ..
        } if canonical_command.is_empty()
            || canonical_command.len()
                > crate::kagemusha_sender_wire::SENDER_COMMAND_MAX_BYTES_V1 =>
        {
            Err(KagemushaStateErrorV1::SnapshotIntegrity)
        }
        Record::Completed {
            original_response,
            publication_directory,
            checkpoint_operation_id,
        } if original_response.is_empty()
            || original_response.len() > KAGEMUSHA_DEVICE_RESPONSE_MAX_BYTES_V1
            || publication_directory.is_empty()
            || *checkpoint_operation_id == [0; 32] =>
        {
            Err(KagemushaStateErrorV1::SnapshotIntegrity)
        }
        _ => Ok(record),
    }
}

fn require_completed_operation(
    command: &[u8],
    operation: DigestV1,
) -> Result<(), KagemushaStateErrorV1> {
    if decode_command(command)?.operation_id != operation {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}

// This reader provides structural original correlation only. The caller must independently
// authenticate the current native owner, Released index record and original device signature.
fn completed_release_original(
    journal: &mut PrivateJournal,
    publication_directory: &Path,
    checkpoint_operation_id: DigestV1,
    canonical_command: &[u8],
    original_response: &[u8],
) -> Result<(Vec<u8>, Vec<u8>, TerminalOriginal), KagemushaStateErrorV1> {
    replay_release_records(journal)?;
    let mut frames = Vec::new();
    journal
        .scan_complete(|sequence, bytes| {
            if sequence > 1 {
                return Err(PrivateJournalError::Corrupt);
            }
            frames.push(bytes.to_vec());
            Ok(())
        })
        .map_err(material_error)?;
    if frames.len() != 2 {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let Record::Command {
        canonical_command: retained_command,
        terminal_original,
        ..
    } = decode_record(&frames[0])?
    else {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    };
    let Record::Completed {
        original_response: retained_response,
        publication_directory: retained_directory,
        checkpoint_operation_id: retained_id,
    } = decode_record(&frames[1])?
    else {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    };
    if retained_command != canonical_command
        || retained_response != original_response
        || publication_directory.to_str() != Some(retained_directory.as_str())
        || retained_id != checkpoint_operation_id
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    require_records(journal, &frames[0], Some(&frames[1]))?;
    Ok((frames.remove(0), frames.remove(0), terminal_original))
}

// Recovery admits only Command plus an optional Completed. Read one additional native frame
// solely to reject an appended suffix. Frame corruption remains a native WAL refusal, and
// failed recovery drops the descriptor before any owner or partially scanned records escape.
fn replay_release_records(journal: &mut PrivateJournal) -> Result<(), KagemushaStateErrorV1> {
    for expected_sequence in 0..2 {
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

fn require_records(
    journal: &PrivateJournal,
    original: &[u8],
    completed: Option<&[u8]>,
) -> Result<(), KagemushaStateErrorV1> {
    let mut count = 0;
    journal
        .scan_complete(|sequence, bytes| {
            let expected = match sequence {
                0 => Some(original),
                1 => completed,
                _ => None,
            };
            if expected != Some(bytes) {
                return Err(PrivateJournalError::Corrupt);
            }
            count += 1;
            Ok(())
        })
        .map_err(material_error)?;
    if count != 1 + usize::from(completed.is_some()) {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}

fn material_error(error: impl std::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::RecoveryMaterial(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn borrowed_completion_identity_requires_the_maintained_exact_op12_command() {
        let command = crate::kagemusha_sender_wire::canonical_command_body_for_tests(12).unwrap();
        let decoded = decode_command(&command).unwrap();
        require_completed_operation(&command, decoded.operation_id).unwrap();
        assert!(require_completed_operation(&command, [0; 32]).is_err());
        let mut changed = decoded.operation_id;
        changed[0] ^= 1;
        assert!(require_completed_operation(&command, changed).is_err());
        assert!(require_completed_operation(&[], decoded.operation_id).is_err());
        let wrong_kind =
            crate::kagemusha_sender_wire::canonical_command_body_for_tests(10).unwrap();
        assert!(require_completed_operation(&wrong_kind, decoded.operation_id).is_err());
        let mut trailing = command;
        trailing.push(0);
        assert!(require_completed_operation(&trailing, decoded.operation_id).is_err());
        // This is typed original-command identity only, not a qualified owner completion proof.
    }

    #[test]
    fn completed_original_reader_requires_exact_both_records_and_destination() {
        // Real private WAL/codec correlation only. This structural machine and marker response
        // never create a qualified Core owner, hardware response or Released authority.
        let (machine, _, _) =
            crate::kagemusha_v1_state::tests::coordinator_operation_store_tests::machine();
        let previous = machine.snapshot().unwrap();
        let command = vec![0x11];
        let response = vec![0x12];
        let root = tempfile::tempdir().unwrap();
        let root = root.path().canonicalize().unwrap();
        let destination = root.join("publication");
        let first = encode_record(&Record::Command {
            previous,
            canonical_command: command.clone(),
            terminal_original: TerminalOriginal::Payment,
        })
        .unwrap();
        let second = encode_record(&Record::Completed {
            original_response: response.clone(),
            publication_directory: destination.to_str().unwrap().to_owned(),
            checkpoint_operation_id: [9; 32],
        })
        .unwrap();
        for variant in 0..8 {
            let directory = root.join(format!("release-{variant}"));
            let mut journal = PrivateJournal::create_new(&directory, FORMAT).unwrap();
            assert_eq!(journal.original_directory().unwrap(), directory);
            journal.append(&first).unwrap();
            if variant != 1 {
                journal.append(&second).unwrap();
            }
            if variant == 2 {
                journal.append(&second).unwrap();
            }
            drop(journal);
            let mut journal = PrivateJournal::open_existing(&directory, FORMAT).unwrap();
            let expected_command = if variant == 3 {
                vec![0x13]
            } else {
                command.clone()
            };
            let expected_response = if variant == 4 {
                vec![0x14]
            } else {
                response.clone()
            };
            let expected_destination = if variant == 5 {
                root.join("foreign")
            } else {
                destination.clone()
            };
            let expected_id = if variant == 6 { [8; 32] } else { [9; 32] };
            if variant == 7 {
                std::fs::rename(
                    directory.join(FORMAT.filename),
                    directory.join("displaced.wal"),
                )
                .unwrap();
                assert!(journal.original_directory().is_err());
            }
            let result = completed_release_original(
                &mut journal,
                &expected_destination,
                expected_id,
                &expected_command,
                &expected_response,
            );
            if variant == 0 {
                let (actual_first, actual_second, mode) = result.unwrap();
                assert_eq!(actual_first, first);
                assert_eq!(actual_second, second);
                require_terminal_kind(&mode, false).unwrap();
                require_records(&journal, &first, Some(&second)).unwrap();
            } else {
                assert!(result.is_err(), "variant {variant}");
            }
        }
    }

    #[test]
    fn terminal_original_mode_is_canonical_and_cannot_cross_operation_kind() {
        // A structural mode is not an ACK, consensus receipt or production release capability.
        let bytes = encode_terminal_original(&TerminalOriginal::Payment).unwrap();
        let decoded = decode_terminal_original(&bytes).unwrap();
        require_terminal_kind(&decoded, false).unwrap();
        assert!(require_terminal_kind(&decoded, true).is_err());
        assert!(decode_terminal_original(&[]).is_err());
        assert!(decode_terminal_original(&[0x7f]).is_err());
        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode_terminal_original(&trailing).is_err());
    }

    #[test]
    fn sender_failures_retain_distinct_recovery_diagnostics() {
        use crate::kagemusha_sender_wire::SenderErrorV1;

        let failures = [
            SenderErrorV1::Size,
            SenderErrorV1::CanonicalEncoding,
            SenderErrorV1::Binding,
            SenderErrorV1::PublicShape,
            SenderErrorV1::Conflict,
            SenderErrorV1::StateRegression,
            SenderErrorV1::Snapshot,
        ];
        let mut diagnostics = std::collections::BTreeSet::new();
        for failure in failures {
            let error: &dyn std::error::Error = &failure;
            assert!(error.source().is_none());
            let KagemushaStateErrorV1::RecoveryMaterial(detail) = material_error(failure) else {
                panic!("a sender refusal must remain a recovery failure");
            };
            assert!(
                !detail.is_empty(),
                "the refusal reason must survive adaptation"
            );
            assert!(
                !detail.contains('\n'),
                "closed failures have one-line diagnostics"
            );
            assert!(
                diagnostics.insert(detail),
                "distinct refusals must remain distinguishable"
            );
        }
    }

    #[test]
    fn release_completion_codec_rejects_missing_identity_and_unbounded_originals() {
        let record = Record::Completed {
            original_response: vec![0x42],
            publication_directory: "/native/owned/publication".to_owned(),
            checkpoint_operation_id: [0x43; 32],
        };
        let canonical = encode_record(&record).unwrap();
        assert!(matches!(
            decode_record(&canonical).unwrap(),
            Record::Completed { .. }
        ));
        assert!(decode_record(&[]).is_err());
        assert!(decode_record(&[0x42]).is_err());
        let zero = Record::Completed {
            original_response: vec![0x42],
            publication_directory: "/native/owned/publication".to_owned(),
            checkpoint_operation_id: [0; 32],
        };
        assert!(decode_record(&encode_record(&zero).unwrap()).is_err());
        let empty = Record::Completed {
            original_response: Vec::new(),
            publication_directory: "/native/owned/publication".to_owned(),
            checkpoint_operation_id: [0x43; 32],
        };
        assert!(decode_record(&encode_record(&empty).unwrap()).is_err());
        assert!(decode_record(&vec![0; FORMAT.maximum_payload_bytes as usize + 1]).is_err());
    }

    #[test]
    fn release_wal_replay_bounds_work_and_refuses_extra_frames() {
        // Raw native framing only. Production recovery drops this rejected descriptor;
        // reading its next row here proves that the fourth frame was not processed.
        let directory = tempfile::tempdir().unwrap();
        let path = directory
            .path()
            .canonicalize()
            .unwrap()
            .join("extra-frames");
        let mut journal = PrivateJournal::create_new(&path, FORMAT).unwrap();
        for value in 1..=4 {
            journal.append(&[value]).unwrap();
        }
        drop(journal);
        let mut reopened = PrivateJournal::open_existing(&path, FORMAT).unwrap();
        assert!(matches!(
            replay_release_records(&mut reopened),
            Err(KagemushaStateErrorV1::SnapshotIntegrity)
        ));
        assert!(reopened.recovery_prefix().is_err());
        assert_eq!(reopened.replay_next().unwrap(), Some((3, vec![4])));
    }

    #[test]
    fn release_wal_replay_preserves_complete_empty_and_torn_prefixes() {
        // These complete bytes test disk framing/order only. Actual recovery still verifies
        // the exact command/response variants, native signatures, current owner and custody.
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let frames: [&[u8]; 2] = [b"structural command", b"structural completed"];
        for count in 0..=2 {
            let path = root.join(format!("complete-{count}"));
            let mut journal = PrivateJournal::create_new(&path, FORMAT).unwrap();
            for frame in frames.iter().take(count) {
                journal.append(frame).unwrap();
            }
            drop(journal);
            let mut reopened = PrivateJournal::open_existing(&path, FORMAT).unwrap();
            if count == 0 {
                assert!(replay_release_records(&mut reopened).is_err());
                continue;
            }
            replay_release_records(&mut reopened).unwrap();
            require_records(&reopened, frames[0], (count == 2).then_some(frames[1])).unwrap();
            assert_eq!(
                reopened.recovery_prefix().unwrap().sequence,
                u64::try_from(count).unwrap()
            );
        }
        for completed_frames in 0..=2 {
            let path = root.join(format!("torn-{completed_frames}"));
            let mut journal = PrivateJournal::create_new(&path, FORMAT).unwrap();
            for frame in frames.iter().take(completed_frames) {
                journal.append(frame).unwrap();
            }
            journal.append(b"torn next frame").unwrap();
            drop(journal);
            let file = std::fs::OpenOptions::new()
                .write(true)
                .open(path.join(FORMAT.filename))
                .unwrap();
            let length = file.metadata().unwrap().len();
            file.set_len(length - 1).unwrap();
            file.sync_all().unwrap();
            drop(file);
            let mut reopened = PrivateJournal::open_existing(&path, FORMAT).unwrap();
            assert!(replay_release_records(&mut reopened).is_err());
            assert!(reopened.recovery_prefix().is_err());
        }
    }

    #[test]
    fn original_release_wal_admits_only_the_exact_complete_two_frame_prefix() {
        // Structural disk ownership only: these marker bytes create no production Core,
        // valid device command, receiver ACK or release authority.
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().canonicalize().unwrap().join("release");
        let mut journal = PrivateJournal::create_new(&directory, FORMAT).unwrap();
        journal.append(&[0x11]).unwrap();
        require_records(&journal, &[0x11], None).unwrap();
        journal.append(&[0x12]).unwrap();
        require_records(&journal, &[0x11], Some(&[0x12])).unwrap();
        drop(journal);
        let mut reopened = PrivateJournal::open_existing(&directory, FORMAT).unwrap();
        replay_release_records(&mut reopened).unwrap();
        require_records(&reopened, &[0x11], Some(&[0x12])).unwrap();
        assert!(require_records(&reopened, &[0x11], Some(&[0x13])).is_err());
        assert!(reopened.check_owned().is_err());
        drop(reopened);
    }
}
