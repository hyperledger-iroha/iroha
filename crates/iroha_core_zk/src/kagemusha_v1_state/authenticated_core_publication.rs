//! Exclusive durable publication by the concrete native owner.
//!
//! The original predecessor and complete successor are synced before hardware CAS.
//! A failed exchange retains the pending owner and original retry identity; it cannot
//! return an old usable wallet, initialize another lane or authorize decoded recovery bytes.

use super::*;
use crate::kagemusha_v1_recursion::KagemushaHardwareTransactionV1;

const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "core-checkpoint.norito.wal",
    magic: b"IKGCPW1\0",
    hash_domain: b"iroha:kagemusha:v1:core-checkpoint-publication\0",
    maximum_payload_bytes: 8 * 1024 * 1024,
};

#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::CoreCheckpointPublicationRecordV1")]
struct Record {
    previous: KagemushaStateSnapshotV1,
    previous_anchor: DurabilityAnchorV1,
    successor: KagemushaStateSnapshotV1,
    statement: KagemushaRecoveryCheckpointStatementV1,
    mutation: Mutation,
    committed_authorization: Option<outgoing::CommitAuthorizationOriginal>,
}

// Actual selected snapshot custody retained throughout the returned owner's lifetime.
pub(super) struct HeldOriginal {
    journal: PrivateJournal,
    canonical_record: Vec<u8>,
    selected: DurabilityAnchorStatementV1,
}

impl HeldOriginal {
    pub(super) fn require_selected(&self, machine: &Machine) -> Result<(), KagemushaStateErrorV1> {
        self.journal
            .require_single_record(&self.canonical_record)
            .map_err(material_error)?;
        if machine
            .published_checkpoint
            .as_ref()
            .map(|p| &p.anchor.statement)
            != Some(&self.selected)
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        Ok(())
    }
}

/// An exclusive native owner awaiting one exact durable hardware publication.
/// It exposes checkpoint material and retry only, never mutable wallet operations.
pub struct KagemushaAuthenticatedCorePublicationV1 {
    owner: KagemushaAuthenticatedCoreOwnerV1,
    candidate: KagemushaRecoveryCheckpointCandidateV1,
    record: Record,
    canonical_record: Vec<u8>,
    journal: PrivateJournal,
}

pub(super) struct PublicationParts {
    candidate: KagemushaRecoveryCheckpointCandidateV1,
    record: Record,
    canonical_record: Vec<u8>,
    journal: PrivateJournal,
}

impl PublicationParts {
    pub(super) fn into_publication(
        self,
        owner: KagemushaAuthenticatedCoreOwnerV1,
    ) -> KagemushaAuthenticatedCorePublicationV1 {
        KagemushaAuthenticatedCorePublicationV1 {
            owner,
            candidate: self.candidate,
            record: self.record,
            canonical_record: self.canonical_record,
            journal: self.journal,
        }
    }
}

/// Independently provisioned original recovery authority and exact private store locations.
/// A persisted publication never supplies any of these authority owners.
pub struct KagemushaAuthenticatedCoreRecoveryInputsV1<'a> {
    /// Independently expected enrollment binding for the restored native wallet.
    pub expected_enrollment: &'a KagemushaRecoveryEnrollmentBindingV1,
    /// Independently authenticated release for the historical checkpoint.
    pub historical_release: &'a KagemushaAuthenticatedReleaseV1,
    /// Independently authenticated historical device credentials for retained history.
    pub history_credentials: KagemushaHistoryDeviceCredentialsV1,
    /// Authenticated verifier for the retained recursive State proof.
    pub recursive_verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
    /// Verifier for the original hardware transaction and credential.
    pub hardware_verifier: KagemushaHardwareTransactionVerifierV1,
    /// Provisioned hardware transport used for the exact retained publication.
    pub transaction_transport: Arc<dyn KagemushaHardwareTransactionTransportV1>,
    /// Private directory containing the original authenticated history journal.
    pub history_directory: &'a Path,
    /// Private directory containing the original coordinator operation journal.
    pub coordinator_directory: &'a Path,
    /// Private directory containing the original hardware response journal.
    pub response_directory: &'a Path,
    /// Private directory containing the original hardware transaction journal.
    pub transaction_directory: &'a Path,
    /// Maximum journal reservation admitted by the provisioned storage owner.
    pub maximum_reserved_bytes: u64,
    /// Capacity of the provisioned immutable storage overlay.
    pub overlay_capacity_bytes: u64,
}

/// Recovery either reauthenticates an already selected successor or retains exclusive pending work.
pub enum KagemushaAuthenticatedCoreRecoveryV1 {
    /// Freshly reauthenticated owner of the already selected original successor.
    Selected(KagemushaAuthenticatedCoreOwnerV1),
    /// Exclusive retained publication requiring the original hardware exchange.
    Pending(KagemushaAuthenticatedCorePublicationV1),
}

impl KagemushaAuthenticatedCoreRecoveryInputsV1<'_> {
    pub(super) fn restore(
        self,
        snapshot: KagemushaStateSnapshotV1,
        anchor: &DurabilityAnchorV1,
    ) -> Result<KagemushaAuthenticatedCoreOwnerV1, KagemushaStateErrorV1> {
        KagemushaAuthenticatedCoreOwnerV1::restore_existing(
            snapshot,
            anchor,
            self.expected_enrollment,
            self.historical_release,
            self.history_credentials,
            self.recursive_verifier,
            self.hardware_verifier,
            self.transaction_transport,
            self.history_directory,
            self.coordinator_directory,
            self.response_directory,
            self.transaction_directory,
            self.maximum_reserved_bytes,
            self.overlay_capacity_bytes,
        )
    }
}

impl KagemushaAuthenticatedCoreOwnerV1 {
    /// Reopen the one original private publication and reconstruct its exact native mutation.
    ///
    /// Raw disk bytes cannot request a CAS. An already selected successor first needs a fresh
    /// qualified hardware challenge. Otherwise the complete predecessor is independently restored,
    /// every original proof and Guard is reverified and the complete derived successor must match
    /// the held canonical record. No old usable owner escapes the uncertain publication interval.
    pub fn recover_checkpoint_existing(
        snapshot_directory: &Path,
        inputs: KagemushaAuthenticatedCoreRecoveryInputsV1<'_>,
    ) -> Result<KagemushaAuthenticatedCoreRecoveryV1, KagemushaStateErrorV1> {
        Self::recover_checkpoint_original(snapshot_directory, inputs, None, None)
    }

    pub(super) fn recover_checkpoint_original(
        snapshot_directory: &Path,
        inputs: KagemushaAuthenticatedCoreRecoveryInputsV1<'_>,
        expected_previous: Option<&KagemushaStateSnapshotV1>,
        expected_mutation: Option<&Mutation>,
    ) -> Result<KagemushaAuthenticatedCoreRecoveryV1, KagemushaStateErrorV1> {
        let mut journal =
            PrivateJournal::open_existing(snapshot_directory, FORMAT).map_err(material_error)?;
        let canonical_record = read_original(&mut journal)?;
        let record = decode_original(&canonical_record)?;
        if let Mutation::Incoming { canonical_original } = &record.mutation {
            incoming::decode_incoming_original_v1(canonical_original)?
                .require_publication_binding(snapshot_directory, record.statement.operation_id)?;
        }
        if expected_previous.is_some_and(|previous| previous != &record.previous)
            || expected_mutation
                .map(norito::encode_canonical)
                .transpose()
                .map_err(material_error)?
                .is_some_and(|expected| {
                    norito::encode_canonical(&record.mutation).ok().as_ref() != Some(&expected)
                })
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        journal
            .require_single_record(&canonical_record)
            .map_err(material_error)?;
        let guard = KagemushaAuthenticatedGuardBundleVerifierV1::new(Arc::clone(
            &inputs.recursive_verifier,
        ))
        .and_then(|guard| guard.with_hardware_transactions(inputs.hardware_verifier.clone()))
        .map_err(material_error)?;
        if guard
            .verify_current_recovery_checkpoint(
                &record.statement.successor,
                &record.successor.recovery_metadata.journals,
            )
            .is_ok()
        {
            // Fresh physical selection precedes recovering the original transaction. A fabricated
            // record therefore cannot drive commit_or_recover ahead of the actual selected state.
            let mut transactions = KagemushaHardwareTransactionJournalV1::open_existing(
                inputs.transaction_directory,
                inputs.hardware_verifier.clone(),
                Arc::clone(&inputs.transaction_transport),
            )
            .map_err(material_error)?;
            journal
                .require_single_record(&canonical_record)
                .map_err(material_error)?;
            let certificate = transactions
                .commit_or_recover(
                    record.statement.operation_id,
                    KagemushaHardwareTransactionV1::RecoveryCheckpoint(record.statement.clone()),
                )
                .map_err(material_error)?;
            guard
                .verify_recovery_checkpoint_cas(&record.statement, &certificate)
                .map_err(material_error)?;
            drop(transactions);
            let anchor = DurabilityAnchorV1 {
                statement: record.statement.successor.clone(),
                guard_bundle: certificate,
            };
            let mut owner = inputs.restore(record.successor.clone(), &anchor)?;
            owner.committed_authorization = record.committed_authorization.clone();
            if let Some(original) = &owner.committed_authorization {
                original.verify(&owner)?;
            }
            journal
                .require_single_record(&canonical_record)
                .map_err(material_error)?;
            owner.selected_publication = Some(HeldOriginal {
                journal,
                canonical_record,
                selected: record.statement.successor,
            });
            owner.current_recovery_selection()?;
            return Ok(KagemushaAuthenticatedCoreRecoveryV1::Selected(owner));
        }
        let owner = if let Mutation::Incoming { canonical_original } = &record.mutation {
            let original = incoming::decode_incoming_original_v1(canonical_original)?;
            if matches!(original, incoming::IncomingOriginalV1::Fold(_)) {
                let pending = incoming::recover_pending_incoming_v1(
                    &original,
                    inputs.expected_enrollment,
                    inputs.historical_release,
                    inputs.history_credentials,
                    inputs.recursive_verifier,
                    inputs.hardware_verifier,
                    inputs.transaction_transport,
                    inputs.history_directory,
                    inputs.coordinator_directory,
                    inputs.response_directory,
                    inputs.transaction_directory,
                    inputs.maximum_reserved_bytes,
                    inputs.overlay_capacity_bytes,
                )?;
                let (owner, previous, reproduced_original) = pending.into_parts();
                if previous != record.previous || reproduced_original != *canonical_original {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                owner
            } else {
                let mut owner = inputs.restore(record.previous.clone(), &record.previous_anchor)?;
                restore_retained_authorization(&mut owner, &record)?;
                outgoing::apply_mutation_original(&mut owner, &record.mutation)?;
                owner
            }
        } else {
            let mut owner = inputs.restore(record.previous.clone(), &record.previous_anchor)?;
            restore_retained_authorization(&mut owner, &record)?;
            outgoing::apply_mutation_original(&mut owner, &record.mutation)?;
            owner
        };
        let candidate = owner.machine.prepare_checkpoint(
            record.statement.operation_id,
            owner
                .journals
                .current_prefixes(&owner.machine.recovery_metadata.journals)?,
            owner.machine.recovery_metadata.accepted_credential.clone(),
        )?;
        if candidate.snapshot != record.successor
            || candidate.statement != record.statement
            || norito::encode_canonical(&owner.committed_authorization).map_err(material_error)?
                != norito::encode_canonical(&record.committed_authorization)
                    .map_err(material_error)?
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        owner.journals.validate_pair(&owner.machine)?;
        journal
            .require_single_record(&canonical_record)
            .map_err(material_error)?;
        Ok(KagemushaAuthenticatedCoreRecoveryV1::Pending(
            KagemushaAuthenticatedCorePublicationV1 {
                owner,
                candidate,
                record,
                canonical_record,
                journal,
            },
        ))
    }
    /// Checkpoint the actual current operation and response journal heads.
    ///
    /// This consumes the usable owner. The new private path is created with no replacement,
    /// and both complete snapshots are fsynced before any device transaction is attempted.
    /// The operation ID remains an immutable retry selector and grants no authority.
    ///
    /// # Errors
    /// Rejects stale hardware, changed custody, conflicting IDs, oversized snapshots or I/O loss.
    pub fn stage_journal_checkpoint(
        self,
        snapshot_directory: &Path,
        checkpoint_operation_id: DigestV1,
    ) -> Result<KagemushaAuthenticatedCorePublicationV1, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let previous = self.selected_predecessor_snapshot()?;
        self.stage_complete_checkpoint(snapshot_directory, checkpoint_operation_id, previous)
    }

    // Preparation/abort history suffixes remain unselected evidence. The original predecessor
    // must use the exact history commitment retained by the currently published checkpoint.
    pub(super) fn selected_predecessor_snapshot(
        &self,
    ) -> Result<KagemushaStateSnapshotV1, KagemushaStateErrorV1> {
        let checkpoint = self
            .machine
            .published_checkpoint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotRollback)?;
        self.machine
            .snapshot_with_history_checkpoint(Some(checkpoint.authenticated_history_commitment))
    }

    // Native typed mutations call this only after authenticating their original predecessor.
    // No public callback can mutate the machine or supply a successor snapshot.
    pub(super) fn stage_complete_checkpoint(
        self,
        snapshot_directory: &Path,
        checkpoint_operation_id: DigestV1,
        previous: KagemushaStateSnapshotV1,
    ) -> Result<KagemushaAuthenticatedCorePublicationV1, KagemushaStateErrorV1> {
        self.stage_complete_checkpoint_with_original(
            snapshot_directory,
            checkpoint_operation_id,
            previous,
            Mutation::JournalHeads,
        )
    }

    pub(super) fn stage_complete_checkpoint_with_original(
        self,
        snapshot_directory: &Path,
        checkpoint_operation_id: DigestV1,
        previous: KagemushaStateSnapshotV1,
        mutation: Mutation,
    ) -> Result<KagemushaAuthenticatedCorePublicationV1, KagemushaStateErrorV1> {
        let parts = self.build_checkpoint_original(
            snapshot_directory,
            checkpoint_operation_id,
            previous,
            mutation,
        )?;
        Ok(parts.into_publication(self))
    }

    // Stage all fallible original material while the exclusive caller still retains its owner.
    pub(super) fn build_checkpoint_original(
        &self,
        snapshot_directory: &Path,
        checkpoint_operation_id: DigestV1,
        previous: KagemushaStateSnapshotV1,
        mutation: Mutation,
    ) -> Result<PublicationParts, KagemushaStateErrorV1> {
        let anchor = &self
            .machine
            .published_checkpoint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotRollback)?
            .anchor;
        if previous.recovery_anchor() != anchor.statement {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        self.journals.validate_pair(&self.machine)?;
        if let Some(original) = &self.committed_authorization {
            original.verify(self)?;
        }
        let candidate = self.machine.prepare_checkpoint(
            checkpoint_operation_id,
            self.journals
                .current_prefixes(&self.machine.recovery_metadata.journals)?,
            self.machine.recovery_metadata.accepted_credential.clone(),
        )?;
        let record = Record {
            previous,
            previous_anchor: anchor.clone(),
            successor: candidate.snapshot.clone(),
            statement: candidate.statement.clone(),
            mutation,
            committed_authorization: self.committed_authorization.clone(),
        };
        let bytes = norito::encode_canonical(&record).map_err(material_error)?;
        if bytes.is_empty() || bytes.len() as u64 > FORMAT.maximum_payload_bytes {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let journal = match PrivateJournal::create_new(snapshot_directory, FORMAT) {
            Ok(mut journal) => {
                journal.append(&bytes).map_err(material_error)?;
                journal
            }
            Err(_) => {
                // The same exclusive mutation may have persisted its original before losing an
                // acknowledgement. Reopen adopts surviving complete bytes durably; only this exact
                // independently derived original is allowed. Empty, torn or foreign paths reject.
                let mut journal = PrivateJournal::open_existing(snapshot_directory, FORMAT)
                    .map_err(material_error)?;
                if read_original(&mut journal)? != bytes {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                journal
                    .require_single_record(&bytes)
                    .map_err(material_error)?;
                journal
            }
        };
        journal.check_owned().map_err(material_error)?;
        Ok(PublicationParts {
            candidate,
            record,
            canonical_record: bytes,
            journal,
        })
    }
}

fn read_original(journal: &mut PrivateJournal) -> Result<Vec<u8>, KagemushaStateErrorV1> {
    while journal.replay_next().map_err(material_error)?.is_some() {}
    let mut original = None;
    journal
        .scan_complete(|sequence, bytes| {
            if sequence != 0 || original.is_some() {
                return Err(PrivateJournalError::Corrupt);
            }
            original = Some(bytes.to_vec());
            Ok(())
        })
        .map_err(material_error)?;
    original.ok_or(KagemushaStateErrorV1::SnapshotIntegrity)
}

fn restore_retained_authorization(
    owner: &mut KagemushaAuthenticatedCoreOwnerV1,
    record: &Record,
) -> Result<(), KagemushaStateErrorV1> {
    if !matches!(
        record.mutation,
        Mutation::Commit { .. } | Mutation::FinalPayment { .. } | Mutation::FinalRedemption { .. }
    ) {
        if let Some(original) = &record.committed_authorization {
            original.verify(owner)?;
            owner.committed_authorization = Some(original.clone());
        }
    }
    Ok(())
}

fn decode_original(bytes: &[u8]) -> Result<Record, KagemushaStateErrorV1> {
    if bytes.is_empty() || bytes.len() as u64 > FORMAT.maximum_payload_bytes {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let record = norito::decode_canonical::<Record>(bytes).map_err(material_error)?;
    if norito::encode_canonical(&record).map_err(material_error)? != bytes
        || record.previous.recovery_anchor() != record.previous_anchor.statement
        || record.successor.recovery_anchor() != record.statement.successor
        || record
            .successor
            .recovery_metadata
            .checkpoint_statement(record.successor.recovery_anchor())
            != record.statement
        || record.statement.previous
            != KagemushaRecoveryCheckpointIdentityV1::from_anchor(&record.previous_anchor.statement)
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(record)
}

impl KagemushaAuthenticatedCorePublicationV1 {
    /// Actual pending native index, for comparison with the original authenticated device reply.
    /// This public projection grants no owner, monetary operation or proving admission.
    pub fn outgoing_operation_record(
        &self,
        operation_id: DigestV1,
    ) -> Result<KagemushaOutgoingOperationRecordV1, KagemushaStateErrorV1> {
        self.recheck_originals()?;
        let prepared = self.outgoing_prepared(operation_id)?;
        let record = self
            .owner
            .machine
            .outgoing_operation_index()
            .lookup(operation_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        record
            .validate_against_prepared(prepared)
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
        self.recheck_originals()?;
        Ok(record.clone())
    }

    /// Borrow the actual staged preparation inside native custody. It is not a proving permit.
    pub fn outgoing_prepared(
        &self,
        operation_id: DigestV1,
    ) -> Result<&PreparedOutgoingCandidateV1, KagemushaStateErrorV1> {
        self.recheck_originals()?;
        let prepared = match self.owner.machine.outgoing_candidate_journal.stage() {
            KagemushaOutgoingJournalStageV1::Prepared(prepared) => prepared,
            KagemushaOutgoingJournalStageV1::Candidate(candidate) => &candidate.prepared,
            _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
        };
        let record = self
            .owner
            .machine
            .outgoing_operation_index()
            .lookup(operation_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if !matches!(
            record.phase,
            KagemushaOutgoingOperationPhaseV1::Prepared
                | KagemushaOutgoingOperationPhaseV1::CandidatePersisted
        ) {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        record
            .validate_against_prepared(prepared)
            .map_err(|_| KagemushaStateErrorV1::SnapshotIntegrity)?;
        self.recheck_originals()?;
        Ok(prepared)
    }
    /// Recheck exclusive retry custody without exposing a usable monetary owner.
    /// Only one freshly authenticated original predecessor or exact successor selection is
    /// admitted. This cannot authorize observations, new work or a substituted checkpoint.
    pub fn recheck_originals(&self) -> Result<(), KagemushaStateErrorV1> {
        self.journal
            .require_single_record(&self.canonical_record)
            .map_err(material_error)?;
        if norito::encode_canonical(&self.record).map_err(material_error)? != self.canonical_record
            || self.record.successor != self.candidate.snapshot
            || self.record.statement != self.candidate.statement
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let commitment = self.owner.machine.snapshot()?.snapshot_commitment;
        if commitment != self.candidate.before_snapshot_commitment
            && commitment != self.candidate.snapshot.snapshot_commitment
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        let heads = self
            .owner
            .journals
            .current_prefixes(&self.candidate.snapshot.recovery_metadata.journals)?;
        if heads != self.candidate.snapshot.recovery_metadata.journals {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        self.owner.journals.validate_pair(&self.owner.machine)?;
        self.owner
            .transactions
            .recovery_prefix()
            .map_err(material_error)?;
        let guard = &self.owner.machine.guard_verifier;
        if guard
            .verify_current_recovery_checkpoint(
                &self.record.previous_anchor.statement,
                &self.record.previous.recovery_metadata.journals,
            )
            .is_err()
        {
            guard
                .verify_current_recovery_checkpoint(&self.candidate.statement.successor, &heads)
                .map_err(material_error)?;
        }
        self.journal
            .require_single_record(&self.canonical_record)
            .map_err(material_error)?;
        if self
            .owner
            .journals
            .current_prefixes(&self.candidate.snapshot.recovery_metadata.journals)?
            != heads
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        self.owner.journals.validate_pair(&self.owner.machine)?;
        self.owner
            .transactions
            .recovery_prefix()
            .map_err(material_error)?;
        Ok(())
    }
    /// Complete private successor material already synced by this pending owner.
    #[must_use]
    pub fn snapshot(&self) -> &KagemushaStateSnapshotV1 {
        self.candidate.snapshot()
    }

    /// Exact hardware CAS selected by the native machine, never by a caller DTO.
    #[must_use]
    pub fn statement(&self) -> &KagemushaRecoveryCheckpointStatementV1 {
        self.candidate.statement()
    }

    /// Commit or recover the original hardware transaction and verify fresh current selection.
    ///
    /// Failure returns this same exclusive pending owner for exact retry. It cannot expose
    /// the old wallet or renew an ID. Success returns the native owner only after its complete
    /// checkpoint, descriptor custody and unchanged original snapshot have been rechecked.
    pub fn finish(
        mut self,
    ) -> Result<KagemushaAuthenticatedCoreOwnerV1, (Box<Self>, KagemushaStateErrorV1)> {
        match self.finish_attempt() {
            Ok(()) => {
                self.owner.selected_publication = Some(HeldOriginal {
                    journal: self.journal,
                    canonical_record: self.canonical_record,
                    selected: self.candidate.statement.successor.clone(),
                });
                Ok(self.owner)
            }
            Err(error) => Err((Box::new(self), error)),
        }
    }

    fn finish_attempt(&mut self) -> Result<(), KagemushaStateErrorV1> {
        self.journal.check_owned().map_err(material_error)?;
        let exact_heads = self
            .owner
            .journals
            .current_prefixes(&self.candidate.snapshot.recovery_metadata.journals)?;
        if exact_heads != self.candidate.snapshot.recovery_metadata.journals {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        // Re-read the held original before dispatch. A cached inode identity alone does not
        // establish that the complete private snapshot still has its persisted bytes.
        self.journal
            .require_single_record(&self.canonical_record)
            .map_err(material_error)?;
        let certificate = self
            .owner
            .transactions
            .commit_or_recover(
                self.candidate.statement.operation_id,
                KagemushaHardwareTransactionV1::RecoveryCheckpoint(
                    self.candidate.statement.clone(),
                ),
            )
            .map_err(material_error)?;
        self.journal
            .require_single_record(&self.canonical_record)
            .map_err(material_error)?;
        if self
            .owner
            .journals
            .current_prefixes(&self.candidate.snapshot.recovery_metadata.journals)?
            != exact_heads
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        self.owner
            .machine
            .install_recovery_checkpoint(&self.candidate, certificate)?;
        // The old selected snapshot source must not be interpreted as the new publication.
        // This exclusive owner checks the actual new source here; finish then transfers it.
        self.owner.journals.validate_pair(&self.owner.machine)?;
        self.owner.machine.current_recovery_selection()?;
        self.owner.journals.validate_pair(&self.owner.machine)?;
        self.journal
            .require_single_record(&self.canonical_record)
            .map_err(material_error)?;
        Ok(())
    }
}

fn material_error(error: impl std::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::RecoveryMaterial(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Structural fixtures verify closed recovery bytes and actual private WAL custody only.
    // Accepting fixture verifiers cannot construct a concrete production recovery owner.
    fn original() -> Record {
        let (machine, _, _) =
            crate::kagemusha_v1_state::tests::coordinator_operation_store_tests::machine();
        let previous = machine.snapshot().unwrap();
        let candidate = machine
            .prepare_checkpoint(
                [218; 32],
                previous.recovery_metadata.journals.clone(),
                previous.recovery_metadata.accepted_credential.clone(),
            )
            .unwrap();
        Record {
            previous,
            previous_anchor: machine
                .published_checkpoint
                .as_ref()
                .unwrap()
                .anchor
                .clone(),
            successor: candidate.snapshot,
            statement: candidate.statement,
            mutation: Mutation::JournalHeads,
            committed_authorization: None,
        }
    }

    #[test]
    fn checkpoint_original_decoder_rejects_changed_checkpoint_and_anchor_bindings() {
        let original = original();
        let canonical = norito::encode_canonical(&original).unwrap();
        assert_eq!(
            norito::encode_canonical(&decode_original(&canonical).unwrap()).unwrap(),
            canonical
        );
        for field in 0..5 {
            let mut changed = original.clone();
            match field {
                0 => changed.previous_anchor.statement.snapshot_commitment[0] ^= 1,
                1 => changed.successor.state.logical_sequence += 1,
                2 => changed.statement.operation_id[0] ^= 1,
                3 => changed.statement.successor.metadata_revision += 1,
                _ => changed.previous_anchor.statement.metadata_revision += 1,
            }
            assert!(
                decode_original(&norito::encode_canonical(&changed).unwrap()).is_err(),
                "field {field}"
            );
        }
    }

    #[test]
    fn checkpoint_original_decoder_rejects_empty_oversized_truncated_and_trailing_input() {
        let canonical = norito::encode_canonical(&original()).unwrap();
        assert!(decode_original(&[]).is_err());
        assert!(decode_original(&vec![0; FORMAT.maximum_payload_bytes as usize + 1]).is_err());
        assert!(decode_original(&canonical[..canonical.len() - 1]).is_err());
        let mut trailing = canonical;
        trailing.push(0);
        assert!(decode_original(&trailing).is_err());
    }

    #[test]
    fn checkpoint_original_reopens_exact_owned_wal_and_refuses_additional_record() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().canonicalize().unwrap().join("publication");
        let canonical = norito::encode_canonical(&original()).unwrap();
        let mut journal = PrivateJournal::create_new(&path, FORMAT).unwrap();
        journal.append(&canonical).unwrap();
        drop(journal);
        let mut reopened = PrivateJournal::open_existing(&path, FORMAT).unwrap();
        assert_eq!(read_original(&mut reopened).unwrap(), canonical);
        reopened.require_single_record(&canonical).unwrap();
        reopened.append(&canonical).unwrap();
        assert!(read_original(&mut reopened).is_err());
        assert!(reopened.check_owned().is_err());
    }
}
