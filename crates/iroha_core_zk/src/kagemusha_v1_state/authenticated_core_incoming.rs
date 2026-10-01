//! Incoming folds under the concrete production proof, Guard and native history owners.
//!
//! A prepared fold consumes its usable owner. The complete selected predecessor is durable
//! before device work, and the exact original proof/Guard/root signature is fsynced before
//! history CAS. No decoded original or historical history prefix can escape as a usable wallet.

#[path = "authenticated_core_incoming_proving.rs"]
mod proving;
pub use proving::KagemushaAuthenticatedIncomingProvingSelectionV1;

use super::*;
use iroha_data_model::kagemusha::{KagemushaDeviceSignatureV1, KagemushaMintCreditV1};

const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "incoming-fold.norito.wal",
    magic: b"IKGIFW1\0",
    hash_domain: b"iroha:kagemusha:v1:incoming-fold-original\0",
    maximum_payload_bytes: 8 * 1024 * 1024,
};

#[derive(Clone, Copy, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::IncomingFoldKindV1")]
enum Kind {
    Mint,
    Receive,
}

#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::IncomingFoldIntentV1")]
struct Intent {
    kind: Kind,
    credit_id: CreditIdV1,
    successor_nonce: DigestV1,
    trusted_time_ms: u64,
    previous: KagemushaStateSnapshotV1,
    checkpoint: DurabilityAnchorV1,
    transaction: KagemushaPreparedHistoryCasV1,
    bridge: KagemushaHistoryProofRootBridgeRequestV1,
}

#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::IncomingFoldProofOriginalV1")]
struct ProofOriginal {
    history_operation_id: DigestV1,
    paired_proof: KagemushaPairedProofV1,
}

/// Untrusted original claims. Its canonical decoder never produces a verification token.
#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::IncomingFoldOriginalV1")]
pub(super) struct IncomingFoldOriginalV1 {
    intent: Intent,
    hardware_certificate: HardwareTransitionCertificateV1,
    paired_proof: KagemushaPairedProofV1,
    root_signature: KagemushaDeviceSignatureV1,
    checkpoint_operation_id: DigestV1,
    snapshot_directory: String,
}

/// Canonical raw mutation originals. Neither decoding nor selecting a variant is authority.
#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::IncomingOriginalV1")]
pub(super) enum IncomingOriginalV1 {
    Fold(IncomingFoldOriginalV1),
    ReserveMint {
        reservation: MintInboxReservationV1,
        certificate: MintReservationCertificateV1,
    },
    StageMint {
        credit: KagemushaMintCreditV1,
        certificate: MintStageCertificateV1,
    },
    StagePeer {
        request: KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
        opening: KagemushaCreditOpeningV1,
        staged_at_ms: u64,
        authorization: PaymentStageAuthorizationV1,
    },
}

impl IncomingOriginalV1 {
    // These original path/ID bytes are retry selectors, never independent authority.
    pub(super) fn require_publication_binding(
        &self,
        directory: &Path,
        checkpoint_id: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        if let Self::Fold(original) = self {
            if original.checkpoint_operation_id != checkpoint_id
                || directory.to_str() != Some(original.snapshot_directory.as_str())
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            publication_binding(directory, checkpoint_id)?;
        }
        Ok(())
    }
}

/// Exclusively mutated successor whose original proof/history CAS was reauthenticated.
/// This parent-private wrapper cannot perform wallet operations or return a predecessor.
pub(super) struct PendingIncomingRecoveryV1 {
    owner: KagemushaAuthenticatedCoreOwnerV1,
    previous: KagemushaStateSnapshotV1,
    canonical_original: Vec<u8>,
}
impl PendingIncomingRecoveryV1 {
    // Only the publication owner consumes this to reproduce its already held exact candidate.
    pub(super) fn into_parts(
        self,
    ) -> (
        KagemushaAuthenticatedCoreOwnerV1,
        KagemushaStateSnapshotV1,
        Vec<u8>,
    ) {
        (self.owner, self.previous, self.canonical_original)
    }
    fn publish(
        self,
        directory: &Path,
        operation_id: DigestV1,
    ) -> Result<KagemushaAuthenticatedCorePublicationV1, KagemushaStateErrorV1> {
        decode_incoming_original_v1(&self.canonical_original)?
            .require_publication_binding(directory, operation_id)?;
        self.owner.stage_complete_checkpoint_with_original(
            directory,
            operation_id,
            self.previous,
            Mutation::Incoming {
                canonical_original: self.canonical_original,
            },
        )
    }
}

enum Preview {
    Mint(CreditFoldPreviewV1),
    Receive(PeerCreditFoldPreviewV1),
}

/// An exclusive incoming attempt awaiting the original proof and hardware selection.
/// It exposes only data needed to produce that same transition; no wallet can be recovered
/// from this object without completing its durable native checkpoint publication.
pub struct KagemushaAuthenticatedIncomingFoldV1 {
    owner: KagemushaAuthenticatedCoreOwnerV1,
    intent: Intent,
    preview: Preview,
    journal: PrivateJournal,
    original_proof: Option<ProofOriginal>,
    completion: Option<FoldCompletion>,
}

struct FoldCompletion {
    original: IncomingFoldOriginalV1,
    bytes: Vec<u8>,
    attempted: bool,
    applied: bool,
}

impl KagemushaAuthenticatedCoreOwnerV1 {
    /// Consume the actual restored predecessor to resume its exact intent-only incoming fold.
    /// The native history preparation must already exist; replay cannot create another CAS,
    /// nonce, time reference or usable predecessor. Completed originals use publication recovery.
    pub fn recover_prepared_incoming_fold_existing(
        mut self,
        incoming_directory: &Path,
    ) -> Result<KagemushaAuthenticatedIncomingFoldV1, KagemushaStateErrorV1> {
        require_finalized_outgoing_before_fold(&self)?;
        self.current_recovery_selection()?;
        let mut journal =
            PrivateJournal::open_existing(incoming_directory, FORMAT).map_err(material_error)?;
        replay_incoming_records(&mut journal, 2)?;
        let mut original = None;
        let mut proof = None;
        journal
            .scan_complete(|sequence, bytes| {
                match sequence {
                    0 if original.is_none() => original = Some(bytes.to_vec()),
                    1 if proof.is_none() => {
                        proof = Some(
                            decode_proof_original(bytes)
                                .map_err(|_| PrivateJournalError::Corrupt)?,
                        )
                    }
                    _ => return Err(PrivateJournalError::Corrupt),
                }
                Ok(())
            })
            .map_err(material_error)?;
        let bytes = original.ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let intent = decode_intent(&bytes)?;
        require_prepared_journal(&journal, &intent, proof.as_ref())?;
        if self.selected_predecessor_snapshot()? != intent.previous
            || self
                .machine
                .published_checkpoint
                .as_ref()
                .map(|value| &value.anchor)
                != Some(&intent.checkpoint)
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        self.machine
            .authenticated_history
            .store
            .require_actual_incoming_history()
            .map_err(map_authenticated_history_error)?;
        self.machine
            .authenticated_history
            .require_prepared(&intent.transaction)
            .map_err(map_authenticated_history_error)?;
        let original_history = self
            .machine
            .authenticated_history
            .store
            .recovery_commitment()
            .map_err(map_authenticated_history_error)?;
        let preview = derive_preview(
            &mut self,
            intent.kind,
            intent.credit_id,
            intent.successor_nonce,
            intent.trusted_time_ms,
        )?;
        let (transaction, bridge) = history_material(&preview);
        if transaction != &intent.transaction
            || bridge != intent.bridge
            || self
                .machine
                .authenticated_history
                .store
                .recovery_commitment()
                .map_err(map_authenticated_history_error)?
                != original_history
        {
            return Err(KagemushaStateErrorV1::AuthenticatedHistoryProofRootBridgeUnavailable);
        }
        self.current_recovery_selection()?;
        require_prepared_journal(&journal, &intent, proof.as_ref())?;
        let recovered = KagemushaAuthenticatedIncomingFoldV1 {
            owner: self,
            intent,
            preview,
            journal,
            original_proof: proof,
            completion: None,
        };
        recovered.recheck_originals()?;
        if let Some(proof) = &recovered.original_proof {
            recovered
                .proving_selection()?
                .authenticate_pair(&proof.paired_proof)?;
            recovered.recheck_originals()?;
        }
        Ok(recovered)
    }

    /// Publish a genuine hardware-certified recipient reservation before its online debit.
    pub fn stage_incoming_mint_reservation(
        self,
        directory: &Path,
        checkpoint_operation_id: DigestV1,
        reservation: MintInboxReservationV1,
        certificate: MintReservationCertificateV1,
    ) -> Result<KagemushaAuthenticatedCorePublicationV1, KagemushaStateErrorV1> {
        self.stage_incoming_original(
            directory,
            checkpoint_operation_id,
            IncomingOriginalV1::ReserveMint {
                reservation,
                certificate,
            },
        )
    }

    /// Verify real governed authorization/finality and checkpoint the exact local staged mint.
    pub fn stage_incoming_mint_credit(
        self,
        directory: &Path,
        checkpoint_operation_id: DigestV1,
        credit: KagemushaMintCreditV1,
        certificate: MintStageCertificateV1,
    ) -> Result<KagemushaAuthenticatedCorePublicationV1, KagemushaStateErrorV1> {
        self.stage_incoming_original(
            directory,
            checkpoint_operation_id,
            IncomingOriginalV1::StageMint {
                credit,
                certificate,
            },
        )
    }

    /// Verify and checkpoint a peer credit before its acknowledgement may leave native custody.
    #[allow(clippy::too_many_arguments)]
    pub fn stage_incoming_peer_payment(
        self,
        directory: &Path,
        checkpoint_operation_id: DigestV1,
        request: KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
        opening: KagemushaCreditOpeningV1,
        staged_at_ms: u64,
        authorization: PaymentStageAuthorizationV1,
    ) -> Result<KagemushaAuthenticatedCorePublicationV1, KagemushaStateErrorV1> {
        self.stage_incoming_original(
            directory,
            checkpoint_operation_id,
            IncomingOriginalV1::StagePeer {
                request,
                payment,
                opening,
                staged_at_ms,
                authorization,
            },
        )
    }

    fn stage_incoming_original(
        mut self,
        directory: &Path,
        checkpoint_operation_id: DigestV1,
        original: IncomingOriginalV1,
    ) -> Result<KagemushaAuthenticatedCorePublicationV1, KagemushaStateErrorV1> {
        self.current_recovery_selection()?;
        let previous = self.selected_predecessor_snapshot()?;
        let bytes = bounded_canonical(&original)?;
        apply_incoming_original_v1(&mut self, &original, false)?;
        self.stage_complete_checkpoint_with_original(
            directory,
            checkpoint_operation_id,
            previous,
            Mutation::Incoming {
                canonical_original: bytes,
            },
        )
    }

    /// Retain one exact, already staged mint before requesting its irreversible fold.
    /// Missing governed proof/hardware authority or changed custody refuses preparation.
    pub fn prepare_incoming_mint_fold(
        self,
        directory: &Path,
        credit_id: CreditIdV1,
        successor_nonce: DigestV1,
        trusted_time_ms: u64,
    ) -> Result<KagemushaAuthenticatedIncomingFoldV1, KagemushaStateErrorV1> {
        self.prepare_incoming_fold(
            directory,
            Kind::Mint,
            credit_id,
            successor_nonce,
            trusted_time_ms,
        )
    }

    /// Retain one exact, already staged peer payment before requesting its ReceiveFold.
    /// The peer proof/opening remains selected from the authenticated native inbox.
    pub fn prepare_incoming_receive_fold(
        self,
        directory: &Path,
        credit_id: CreditIdV1,
        successor_nonce: DigestV1,
        trusted_time_ms: u64,
    ) -> Result<KagemushaAuthenticatedIncomingFoldV1, KagemushaStateErrorV1> {
        self.prepare_incoming_fold(
            directory,
            Kind::Receive,
            credit_id,
            successor_nonce,
            trusted_time_ms,
        )
    }

    fn prepare_incoming_fold(
        mut self,
        directory: &Path,
        kind: Kind,
        credit_id: CreditIdV1,
        successor_nonce: DigestV1,
        trusted_time_ms: u64,
    ) -> Result<KagemushaAuthenticatedIncomingFoldV1, KagemushaStateErrorV1> {
        require_finalized_outgoing_before_fold(&self)?;
        self.current_recovery_selection()?;
        let previous = self.selected_predecessor_snapshot()?;
        let checkpoint = self
            .machine
            .published_checkpoint
            .as_ref()
            .ok_or(KagemushaStateErrorV1::SnapshotRollback)?
            .anchor
            .clone();
        let preview = derive_preview(&mut self, kind, credit_id, successor_nonce, trusted_time_ms)?;
        let (transaction, bridge) = history_material(&preview);
        let intent = Intent {
            kind,
            credit_id,
            successor_nonce,
            trusted_time_ms,
            previous,
            checkpoint,
            transaction: transaction.clone(),
            bridge,
        };
        // Prepared external history is an unselected suffix. It cannot renew the complete floor.
        self.current_recovery_selection()?;
        let bytes = bounded_canonical(&intent)?;
        let mut journal = PrivateJournal::create_new(directory, FORMAT).map_err(material_error)?;
        journal.append(&bytes).map_err(material_error)?;
        journal
            .require_single_record(&bytes)
            .map_err(material_error)?;
        Ok(KagemushaAuthenticatedIncomingFoldV1 {
            owner: self,
            intent,
            preview,
            journal,
            original_proof: None,
            completion: None,
        })
    }
}

impl KagemushaAuthenticatedIncomingFoldV1 {
    /// Authenticate and fsync the original genuine pair before any physical device request.
    /// Lost acknowledgements retain these exact bytes; recovery cannot generate another pair.
    pub fn retain_original_proof(
        &mut self,
        proof: &KagemushaPairedProofV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_originals()?;
        self.proving_selection()?.authenticate_pair(proof)?;
        let original = ProofOriginal {
            history_operation_id: self.history_operation_id(),
            paired_proof: proof.clone(),
        };
        let bytes = bounded_canonical(&original)?;
        if bytes.len() > 8192 {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        if let Some(retained) = &self.original_proof {
            if retained != &original {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        } else {
            self.original_proof = Some(original);
            self.journal.append(&bytes).map_err(material_error)?;
        }
        self.recheck_originals()
    }

    /// Read the retained original after descriptor checks; this returns no proof authority.
    pub fn original_proof(&self) -> Result<Option<&KagemushaPairedProofV1>, KagemushaStateErrorV1> {
        self.recheck_originals()?;
        Ok(self
            .original_proof
            .as_ref()
            .map(|value| &value.paired_proof))
    }

    /// Borrow a fresh actual predecessor selection for this same uncompleted incoming attempt.
    /// This cannot return a usable owner or admit another credit or device operation.
    pub fn current_recovery_selection(
        &self,
    ) -> Result<KagemushaCurrentRecoverySelectionV1<'_>, KagemushaStateErrorV1> {
        self.recheck_originals()?;
        let selection = self.owner.current_recovery_selection()?;
        self.recheck_originals()?;
        Ok(selection)
    }

    /// Return the independently authenticated release for this original prepared fold.
    /// Its immutable catalog grants no wallet, proving admission or fresh session lease.
    pub fn authenticated_release(
        &self,
    ) -> Result<Arc<KagemushaAuthenticatedReleaseV1>, KagemushaStateErrorV1> {
        self.recheck_originals()?;
        let release = self.owner.authenticated_release()?;
        self.recheck_originals()?;
        Ok(release)
    }

    /// Recheck the same exclusive prepared attempt before returning its original device work.
    /// Completion closes this preparation surface; retries then use complete_or_retain or
    /// authenticated existing-WAL recovery, never a second device fold request.
    pub fn recheck_originals(&self) -> Result<(), KagemushaStateErrorV1> {
        if let Some(completion) = &self.completion {
            require_original_journal(&self.journal, &completion.original)?;
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        require_finalized_outgoing_before_fold(&self.owner)?;
        self.owner.current_recovery_selection()?;
        require_prepared_journal(&self.journal, &self.intent, self.original_proof.as_ref())?;
        if self.owner.selected_predecessor_snapshot()? != self.intent.previous {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        let (transaction, bridge) = history_material(&self.preview);
        if transaction != &self.intent.transaction || bridge != self.intent.bridge {
            return Err(KagemushaStateErrorV1::AuthenticatedHistoryProofRootBridgeUnavailable);
        }
        self.root_selection_signing_bytes()?;
        require_prepared_journal(&self.journal, &self.intent, self.original_proof.as_ref())?;
        Ok(())
    }

    /// Recheck only the retained completion for an exact retry, without reopening preparation.
    /// This does not select historical roots or grant another physical fold. Consumption still
    /// authenticates the actual original native history CAS before any successor publication.
    pub fn recheck_retry_originals(&self) -> Result<(), KagemushaStateErrorV1> {
        let Some(completion) = &self.completion else {
            return self.recheck_originals();
        };
        let original = &completion.original;
        require_original_journal(&self.journal, original)?;
        if original.intent != self.intent
            || bounded_canonical(&IncomingOriginalV1::Fold(original.clone()))? != completion.bytes
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let (transaction, bridge) = history_material(&self.preview);
        if transaction != &self.intent.transaction || bridge != self.intent.bridge {
            return Err(KagemushaStateErrorV1::AuthenticatedHistoryProofRootBridgeUnavailable);
        }
        publication_binding(
            Path::new(&original.snapshot_directory),
            original.checkpoint_operation_id,
        )?;
        let previous = &self.intent.previous;
        let transition = transition(&self.preview);
        self.owner
            .machine
            .guard_verifier
            .verify_current_recovery_checkpoint(
                &self.intent.checkpoint.statement,
                &previous.recovery_metadata.journals,
            )
            .map_err(material_error)?;
        self.owner.machine.verify_transition_authorization(
            transition,
            &TransitionAuthorizationV1::new(
                original.hardware_certificate.clone(),
                original.paired_proof.clone(),
            ),
        )?;
        let key = self
            .owner
            .machine
            .authenticated_history
            .store
            .current_device_key(
                previous.state.hardware_profile_id,
                previous.state.hardware_epoch.generation,
                previous.state.device_policy_binding.device_key_reference,
            )
            .map_err(map_authenticated_history_error)?;
        KagemushaHistoryRootSelectionCertificateV1::new(
            KagemushaHistoryRootSelectionSubjectV1::new(
                transaction,
                previous.state.hardware_profile_id,
                previous.state.hardware_epoch.generation,
                transition.journal_revision_after,
            ),
            original.root_signature,
        )
        .verify(previous.state.hardware_profile_id, &key)
        .map_err(map_authenticated_history_error)?;
        if self.owner.machine.state
            != if completion.applied {
                transition.successor.clone()
            } else {
                previous.state.clone()
            }
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        validate_incoming_owner(&self.owner)?;
        self.owner
            .machine
            .guard_verifier
            .verify_current_recovery_checkpoint(
                &self.intent.checkpoint.statement,
                &previous.recovery_metadata.journals,
            )
            .map_err(material_error)?;
        require_original_journal(&self.journal, original)
    }

    /// Deterministic identity of this original native history CAS, never a caller nonce.
    #[must_use]
    pub fn history_operation_id(&self) -> DigestV1 {
        self.intent.transaction.transaction_id()
    }
    /// Exact original credit identity; it is a retry selector, not authorization.
    #[must_use]
    pub const fn credit_id(&self) -> CreditIdV1 {
        self.intent.credit_id
    }

    /// Actual current key-reference and epoch retained by the selected native predecessor.
    #[must_use]
    pub fn device_binding(&self) -> (DevicePolicyBindingV1, HardwareEpochV1) {
        (
            self.intent.previous.state.device_policy_binding,
            self.intent.previous.state.hardware_epoch,
        )
    }

    /// Exact native-derived statement for the prover and qualified hardware owner.
    /// Private successor/opening data must remain inside native custody.
    #[must_use]
    pub fn transition(&self) -> &TransitionPreviewV1 {
        transition(&self.preview)
    }

    /// Original domain-separated hardware root-selection message for this prepared native CAS.
    pub fn root_selection_signing_bytes(&self) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        match &self.preview {
            Preview::Mint(preview) => self
                .owner
                .machine
                .mint_fold_history_root_selection_signing_bytes(preview),
            Preview::Receive(preview) => self
                .owner
                .machine
                .receive_fold_history_root_selection_signing_bytes(preview),
        }
    }

    /// Persist and verify the original raw proof, Guard and device root signature before CAS.
    /// The usable owner stays consumed; successful completion returns only pending publication.
    /// Restart must use the same held original, never a new credit or a replacement proof.
    pub fn complete(
        self,
        snapshot_directory: &Path,
        checkpoint_operation_id: DigestV1,
        authorization: TransitionAuthorizationV1,
        root_signature: KagemushaDeviceSignatureV1,
    ) -> Result<KagemushaAuthenticatedCorePublicationV1, KagemushaStateErrorV1> {
        self.complete_or_retain(
            snapshot_directory,
            checkpoint_operation_id,
            authorization,
            root_signature,
        )
        .map_err(|(_, error)| error)
    }

    /// Keep the same exclusive attempt and original completion on an uncertain failure.
    /// Changed proof, Guard, signature or publication destination cannot become a new retry.
    pub fn complete_or_retain(
        mut self,
        snapshot_directory: &Path,
        checkpoint_operation_id: DigestV1,
        authorization: TransitionAuthorizationV1,
        root_signature: KagemushaDeviceSignatureV1,
    ) -> Result<KagemushaAuthenticatedCorePublicationV1, (Box<Self>, KagemushaStateErrorV1)> {
        match self.complete_attempt(
            snapshot_directory,
            checkpoint_operation_id,
            authorization,
            root_signature,
        ) {
            Ok(parts) => Ok(parts.into_publication(self.owner)),
            Err(error) => Err((Box::new(self), error)),
        }
    }

    fn complete_attempt(
        &mut self,
        snapshot_directory: &Path,
        checkpoint_operation_id: DigestV1,
        authorization: TransitionAuthorizationV1,
        root_signature: KagemushaDeviceSignatureV1,
    ) -> Result<publication::PublicationParts, KagemushaStateErrorV1> {
        let retained_proof = self
            .original_proof
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if retained_proof.paired_proof != authorization.proof {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        if authorization.authenticated_history.is_some() {
            return Err(KagemushaStateErrorV1::AuthenticatedHistoryProofRootBridgeUnavailable);
        }
        let original = IncomingFoldOriginalV1 {
            intent: self.intent.clone(),
            hardware_certificate: authorization.hardware_certificate,
            paired_proof: authorization.proof,
            root_signature,
            checkpoint_operation_id,
            snapshot_directory: publication_binding(snapshot_directory, checkpoint_operation_id)?,
        };
        let bytes = bounded_canonical(&IncomingOriginalV1::Fold(original.clone()))?;
        if let Some(retained) = &self.completion {
            if retained.bytes != bytes {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        } else {
            self.owner.current_recovery_selection()?;
            require_prepared_journal(&self.journal, &self.intent, self.original_proof.as_ref())?;
            authenticate_original(&self.owner, &self.preview, &original)?;
            // Retain exact original before an append whose acknowledgement may be lost.
            self.completion = Some(FoldCompletion {
                original: original.clone(),
                bytes: bytes.clone(),
                attempted: false,
                applied: false,
            });
            self.journal.append(&bytes).map_err(material_error)?;
        }
        require_original_journal(&self.journal, &original)?;
        let retained = self
            .completion
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        if !retained.applied {
            if retained.attempted {
                select_original_history_predecessor(&mut self.owner, &retained.original)?;
            }
            let recovering = retained.attempted;
            retained.attempted = true;
            install_incoming_fold_original_v1(&mut self.owner, &retained.original, recovering)?;
            // No fallible validation lies between the kernel installing the complete successor
            // and marking it retained. Retry cannot apply that credit a second time.
            retained.applied = true;
        }
        validate_incoming_owner(&self.owner)?;
        require_original_journal(&self.journal, &original)?;
        self.owner.build_checkpoint_original(
            snapshot_directory,
            checkpoint_operation_id,
            self.intent.previous.clone(),
            Mutation::Incoming {
                canonical_original: bytes,
            },
        )
    }
}

fn decode_intent(bytes: &[u8]) -> Result<Intent, KagemushaStateErrorV1> {
    if bytes.is_empty() || bytes.len() as u64 > FORMAT.maximum_payload_bytes {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let intent = norito::decode_canonical::<Intent>(bytes).map_err(material_error)?;
    if bounded_canonical(&intent)? != bytes {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(intent)
}

pub(super) fn decode_incoming_original_v1(
    bytes: &[u8],
) -> Result<IncomingOriginalV1, KagemushaStateErrorV1> {
    if bytes.is_empty() || bytes.len() as u64 > FORMAT.maximum_payload_bytes {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let original = norito::decode_canonical::<IncomingOriginalV1>(bytes).map_err(material_error)?;
    if let IncomingOriginalV1::Fold(fold) = &original {
        require_original_pair_wire_shape(&fold.paired_proof)?;
        publication_binding(
            Path::new(&fold.snapshot_directory),
            fold.checkpoint_operation_id,
        )?;
    }
    if bounded_canonical(&original)? != bytes {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(original)
}

pub(super) fn apply_incoming_original_v1(
    owner: &mut KagemushaAuthenticatedCoreOwnerV1,
    original: &IncomingOriginalV1,
    recovering: bool,
) -> Result<(), KagemushaStateErrorV1> {
    if let IncomingOriginalV1::Fold(original) = original {
        return apply_incoming_fold_original_v1(owner, original, recovering);
    }
    owner.current_recovery_selection()?;
    match original {
        IncomingOriginalV1::ReserveMint {
            reservation,
            certificate,
        } => {
            validate_guard_bytes(&certificate.guard_bundle)?;
            if certificate.statement.version != KAGEMUSHA_STATE_VERSION_V1
                || certificate.statement.lane != owner.machine.state.lane
                || certificate.statement.reservation_digest != reservation.digest()?
            {
                return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
            }
            // Even an already anchored exact reservation cannot authenticate substituted raw
            // Guard bytes. This recheck precedes the generic kernel's duplicate classification.
            owner
                .machine
                .guard_verifier
                .verify_mint_reservation(&certificate.statement, &certificate.guard_bundle)
                .map_err(KagemushaStateErrorV1::GuardRejected)?;
            owner
                .machine
                .recursive_verifier
                .verify_mint_authorization(reservation.authorization())
                .map_err(KagemushaStateErrorV1::ProofRejected)?;
            owner
                .machine
                .reserve_mint_credit(reservation, certificate)?;
        }
        IncomingOriginalV1::StageMint {
            credit,
            certificate,
        } => {
            credit
                .validate_shape()
                .map_err(|_| KagemushaStateErrorV1::InvalidMintCredit)?;
            validate_guard_bytes(&certificate.guard_bundle)?;
            let id = CreditIdV1(credit.statement.lifecycle.credit_id);
            if let Some(receipt) = owner.machine.mint_inbox.accepted_receipt(id) {
                let envelope = mint_envelope_digest_v1(credit)?;
                if receipt.envelope_digest() != envelope
                    || receipt.authorization_digest() != credit.statement.mint_authorization_digest
                    || receipt.stage_certificate() != certificate
                    || owner.machine.consumed_credits.get(id) != Some(envelope)
                    || owner
                        .machine
                        .authenticated_history
                        .classify_replay(id, envelope)
                        .map_err(map_authenticated_history_error)?
                        != KagemushaHistoryIdentityClassificationV1::ExactDuplicate
                {
                    return Err(KagemushaStateErrorV1::CreditConflict(id));
                }
                // This exact historical receipt already belongs to the freshly selected full
                // Core checkpoint. It grants no new authorization, staging or monetary fold.
                owner.current_recovery_selection()?;
                return Ok(());
            }
            if let Some(staged) = owner.machine.mint_inbox.pending_credit(id) {
                if staged.stage_certificate() != certificate {
                    return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
                }
            }
            let reservation = owner
                .machine
                .mint_inbox
                .reservation(id)
                .cloned()
                .or_else(|| {
                    owner
                        .machine
                        .mint_inbox
                        .pending_credit(id)
                        .map(|staged| staged.reservation().clone())
                })
                .ok_or(KagemushaStateErrorV1::CreditNotStaged(id))?;
            let verified = mint_inbox::verify_governed_mint_stage_v1(
                &owner.machine.recursive_verifier,
                owner.machine.proof_release.artifacts,
                &reservation,
                credit,
            )?;
            owner.machine.stage_mint_credit(
                reservation.authorization(),
                credit,
                Some(&verified),
                Some(certificate),
            )?;
        }
        IncomingOriginalV1::StagePeer {
            request,
            payment,
            opening,
            staged_at_ms,
            authorization,
        } => {
            validate_peer_payment_wire_shape_against_lane(&owner.machine.state, request, payment)?;
            validate_guard_bytes(&authorization.stage_certificate.guard_bundle)?;
            authorization
                .acknowledgement
                .validate_shape_against(request, payment)
                .map_err(|_| KagemushaStateErrorV1::InvalidAcknowledgement)?;
            if let Some(receipt) = owner
                .machine
                .accepted_payment_receipts
                .get(&CreditIdV1(payment.output.credit_id))
            {
                if receipt.stage_certificate != authorization.stage_certificate
                    || receipt.durable_acknowledgement.acknowledgement
                        != authorization.acknowledgement
                {
                    return Err(KagemushaStateErrorV1::HardwareCertificateMismatch);
                }
            }
            owner.machine.stage_payment(
                request.clone(),
                payment.clone(),
                *opening,
                *staged_at_ms,
                Some(authorization.clone()),
            )?;
        }
        IncomingOriginalV1::Fold(_) => unreachable!("fold was handled before inbox mutation"),
    }
    owner.journals.validate_pair(&owner.machine)?;
    owner
        .transactions
        .recovery_prefix()
        .map_err(material_error)?;
    Ok(())
}

/// The specialized preceding-checkpoint branch is available only for the original history CAS.
/// Ordinary inbox mutations use the publisher's normal previous-owner restoration instead.
#[allow(clippy::too_many_arguments)]
pub(super) fn recover_pending_incoming_v1(
    original: &IncomingOriginalV1,
    expected_enrollment: &KagemushaRecoveryEnrollmentBindingV1,
    historical_release: &KagemushaAuthenticatedReleaseV1,
    history_credentials: KagemushaHistoryDeviceCredentialsV1,
    recursive_verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
    hardware_verifier: KagemushaHardwareTransactionVerifierV1,
    transaction_transport: Arc<dyn KagemushaHardwareTransactionTransportV1>,
    history_directory: &Path,
    coordinator_directory: &Path,
    response_directory: &Path,
    transaction_directory: &Path,
    maximum_reserved_bytes: u64,
    overlay_capacity_bytes: u64,
) -> Result<PendingIncomingRecoveryV1, KagemushaStateErrorV1> {
    let IncomingOriginalV1::Fold(original) = original else {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    };
    restore_pending_incoming_fold_owner_v1(
        original,
        expected_enrollment,
        historical_release,
        history_credentials,
        recursive_verifier,
        hardware_verifier,
        transaction_transport,
        history_directory,
        coordinator_directory,
        response_directory,
        transaction_directory,
        maximum_reserved_bytes,
        overlay_capacity_bytes,
    )
}

// This specialized constructor never returns the predecessor owner. Its history view is a
// replayed, authenticated original prefix needed only to verify/reapply the one pending CAS.
// The actual committed store is selected again before checkpoint publication can be returned.
#[allow(clippy::too_many_arguments)]
fn restore_pending_incoming_fold_owner_v1(
    original: &IncomingFoldOriginalV1,
    expected_enrollment: &KagemushaRecoveryEnrollmentBindingV1,
    historical_release: &KagemushaAuthenticatedReleaseV1,
    history_credentials: KagemushaHistoryDeviceCredentialsV1,
    recursive_verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
    hardware_verifier: KagemushaHardwareTransactionVerifierV1,
    transaction_transport: Arc<dyn KagemushaHardwareTransactionTransportV1>,
    history_directory: &Path,
    coordinator_directory: &Path,
    response_directory: &Path,
    transaction_directory: &Path,
    maximum_reserved_bytes: u64,
    overlay_capacity_bytes: u64,
) -> Result<PendingIncomingRecoveryV1, KagemushaStateErrorV1> {
    let previous = &original.intent.previous;
    if original.intent.bridge.transaction_id() != original.intent.transaction.transaction_id()
        || original.intent.bridge.external_predecessor_roots()
            != previous.authenticated_history_roots
        || original.intent.checkpoint.statement != previous.recovery_anchor()
    {
        return Err(KagemushaStateErrorV1::SnapshotRollback);
    }
    let guard = KagemushaAuthenticatedGuardBundleVerifierV1::new(Arc::clone(&recursive_verifier))
        .and_then(|guard| guard.with_hardware_transactions(hardware_verifier.clone()))
        .map_err(material_error)?;
    let release = guard.authenticated_release().map_err(material_error)?;
    let proof_release = KagemushaStateProofReleaseV1::from_authenticated_release(&release)?;
    let floor_release =
        KagemushaStateProofReleaseV1::from_authenticated_release(historical_release)?;
    history_credentials
        .require_current_binding(
            previous.state.hardware_profile_id,
            previous.state.hardware_epoch.generation,
            previous.state.device_policy_binding.device_key_reference,
        )
        .map_err(map_authenticated_history_error)?;
    let lane_binding = disk_history_lane_binding(previous.state.context(), &previous.state.lane)?;
    let mut history = KagemushaDiskAuthenticatedHistoryStoreV1::open_existing(
        history_directory,
        lane_binding,
        history_credentials,
        overlay_capacity_bytes,
    )
    .map_err(map_authenticated_history_error)?;
    let subject = KagemushaHistoryRootSelectionSubjectV1::new(
        &original.intent.transaction,
        previous.state.hardware_profile_id,
        previous.state.hardware_epoch.generation,
        original
            .hardware_certificate
            .statement
            .journal_revision_after,
    );
    let certificate =
        KagemushaHistoryRootSelectionCertificateV1::new(subject, original.root_signature);
    // If the CAS is still pending, ordinary restore sees the actual old roots. If it already
    // committed, only its exact authenticated original WAL prefix may supply the old view.
    if history.committed_roots() != previous.authenticated_history_roots {
        history
            .select_pending_incoming_predecessor(
                lane_binding,
                &original.intent.transaction,
                certificate,
                previous.authenticated_history_roots,
                previous.authenticated_history_commitment,
            )
            .map_err(map_authenticated_history_error)?;
    }
    let journals = KagemushaPendingRecoveryJournalsV1::open_existing(
        coordinator_directory,
        response_directory,
        &previous.state.lane,
        previous.state.asset_incarnation,
        maximum_reserved_bytes,
    )?;
    let transactions = KagemushaHardwareTransactionJournalV1::open_existing(
        transaction_directory,
        hardware_verifier,
        transaction_transport,
    )
    .map_err(material_error)?;
    // The complete previous commitment, original checkpoint CAS, fresh native selection,
    // credential floor, real paired incoming proof and each staging Guard are all verified by
    // this actual restoration; this is not a fabricated root or a detached accepted snapshot.
    let machine = Machine::restore(
        previous.clone(),
        &original.intent.checkpoint,
        proof_release,
        floor_release,
        expected_enrollment,
        history,
        recursive_verifier,
        guard,
    )?;
    let mut owner = KagemushaAuthenticatedCoreOwnerV1 {
        machine,
        journals,
        transactions,
        selected_publication: None,
        committed_authorization: None,
    };
    owner.current_recovery_selection()?;
    apply_incoming_fold_original_v1(&mut owner, original, true)?;
    // install_* removes any selected-prefix lens only after the exact original real CAS is
    // recovered. A successor cannot be published with historical roots left selected.
    owner
        .machine
        .authenticated_history
        .store
        .require_actual_incoming_history()
        .map_err(map_authenticated_history_error)?;
    Ok(PendingIncomingRecoveryV1 {
        owner,
        previous: previous.clone(),
        canonical_original: bounded_canonical(&IncomingOriginalV1::Fold(original.clone()))?,
    })
}

impl KagemushaAuthenticatedCoreOwnerV1 {
    /// Recover a completed incoming original from its existing descriptor-owned journal.
    /// The independently installed proof/hardware owners must still select the old complete
    /// checkpoint. An existing publication is reopened with its exact original destination;
    /// an already selected successor is reauthenticated by that same publication owner.
    /// No unverified historical owner, root or failed partial recovery is returned.
    #[allow(clippy::too_many_arguments)]
    pub fn recover_pending_incoming_fold(
        incoming_directory: &Path,
        snapshot_directory: &Path,
        checkpoint_operation_id: DigestV1,
        expected_enrollment: &KagemushaRecoveryEnrollmentBindingV1,
        historical_release: &KagemushaAuthenticatedReleaseV1,
        history_credentials: KagemushaHistoryDeviceCredentialsV1,
        recursive_verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
        hardware_verifier: KagemushaHardwareTransactionVerifierV1,
        transaction_transport: Arc<dyn KagemushaHardwareTransactionTransportV1>,
        history_directory: &Path,
        coordinator_directory: &Path,
        response_directory: &Path,
        transaction_directory: &Path,
        maximum_reserved_bytes: u64,
        overlay_capacity_bytes: u64,
    ) -> Result<KagemushaAuthenticatedCoreRecoveryV1, KagemushaStateErrorV1> {
        let mut journal =
            PrivateJournal::open_existing(incoming_directory, FORMAT).map_err(material_error)?;
        replay_incoming_records(&mut journal, 3)?;
        let mut original = None;
        journal
            .scan_complete(|sequence, bytes| {
                if sequence == 2 {
                    original = Some(
                        match decode_incoming_original_v1(bytes)
                            .map_err(|_| PrivateJournalError::Corrupt)?
                        {
                            IncomingOriginalV1::Fold(original) => original,
                            _ => return Err(PrivateJournalError::Corrupt),
                        },
                    );
                } else if sequence != 0 && sequence != 1 {
                    return Err(PrivateJournalError::Corrupt);
                }
                Ok(())
            })
            .map_err(material_error)?;
        let original = original.ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        require_original_journal(&journal, &original)?;
        let raw_original = IncomingOriginalV1::Fold(original.clone());
        raw_original.require_publication_binding(snapshot_directory, checkpoint_operation_id)?;
        let bytes = bounded_canonical(&raw_original)?;
        let mutation = Mutation::Incoming {
            canonical_original: bytes,
        };
        match std::fs::symlink_metadata(snapshot_directory) {
            Ok(_) => {
                let recovered = Self::recover_checkpoint_original(
                    snapshot_directory,
                    KagemushaAuthenticatedCoreRecoveryInputsV1 {
                        expected_enrollment,
                        historical_release,
                        history_credentials,
                        recursive_verifier,
                        hardware_verifier,
                        transaction_transport,
                        history_directory,
                        coordinator_directory,
                        response_directory,
                        transaction_directory,
                        maximum_reserved_bytes,
                        overlay_capacity_bytes,
                    },
                    Some(&original.intent.previous),
                    Some(&mutation),
                )?;
                require_original_journal(&journal, &original)?;
                return Ok(recovered);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(material_error(error)),
        }
        let pending = restore_pending_incoming_fold_owner_v1(
            &original,
            expected_enrollment,
            historical_release,
            history_credentials,
            recursive_verifier,
            hardware_verifier,
            transaction_transport,
            history_directory,
            coordinator_directory,
            response_directory,
            transaction_directory,
            maximum_reserved_bytes,
            overlay_capacity_bytes,
        )?;
        require_original_journal(&journal, &original)?;
        pending
            .publish(snapshot_directory, checkpoint_operation_id)
            .map(KagemushaAuthenticatedCoreRecoveryV1::Pending)
    }
}

fn apply_incoming_fold_original_v1(
    owner: &mut KagemushaAuthenticatedCoreOwnerV1,
    original: &IncomingFoldOriginalV1,
    recovering: bool,
) -> Result<(), KagemushaStateErrorV1> {
    install_incoming_fold_original_v1(owner, original, recovering)?;
    validate_incoming_owner(owner)
}

fn install_incoming_fold_original_v1(
    owner: &mut KagemushaAuthenticatedCoreOwnerV1,
    original: &IncomingFoldOriginalV1,
    recovering: bool,
) -> Result<(), KagemushaStateErrorV1> {
    require_finalized_outgoing_before_fold(owner)?;
    if owner.selected_predecessor_snapshot()? != original.intent.previous {
        return Err(KagemushaStateErrorV1::SnapshotRollback);
    }
    owner.current_recovery_selection()?;
    let preview = derive_preview(
        owner,
        original.intent.kind,
        original.intent.credit_id,
        original.intent.successor_nonce,
        original.intent.trusted_time_ms,
    )?;
    let (transaction, bridge) = history_material(&preview);
    if *transaction != original.intent.transaction || bridge != original.intent.bridge {
        return Err(KagemushaStateErrorV1::AuthenticatedHistoryProofRootBridgeUnavailable);
    }
    let authorization = authenticate_original(owner, &preview, original)?;
    owner.journals.validate_pair(&owner.machine)?;
    owner
        .transactions
        .recovery_prefix()
        .map_err(material_error)?;
    match preview {
        Preview::Mint(preview) => {
            let credit = selected_mint(owner, original.intent.credit_id)?;
            let finality = crate::kagemusha_v1_recursion::verify_kagemusha_mint_finality_helper_v1(
                &owner.machine.recursive_verifier,
                owner.machine.proof_release.artifacts,
                &credit,
            )
            .map_err(|_| KagemushaStateErrorV1::MintFinalityMismatch)?;
            owner.machine.install_mint_fold(
                credit,
                preview,
                finality,
                authorization,
                recovering,
            )?;
        }
        Preview::Receive(preview) => {
            owner
                .machine
                .install_receive_fold(preview, authorization, recovering)?;
        }
    }
    Ok(())
}

// Final payment/redemption proving consumes the exact committed outgoing successor. A fold
// changes that state, so it must wait until the original terminal envelope is finalized.
fn require_finalized_outgoing_before_fold(
    owner: &KagemushaAuthenticatedCoreOwnerV1,
) -> Result<(), KagemushaStateErrorV1> {
    if matches!(
        owner.machine.outgoing_candidate_journal.stage(),
        KagemushaOutgoingJournalStageV1::Committed(_)
    ) {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    }
    Ok(())
}

fn validate_incoming_owner(
    owner: &KagemushaAuthenticatedCoreOwnerV1,
) -> Result<(), KagemushaStateErrorV1> {
    owner.journals.validate_pair(&owner.machine)?;
    owner
        .transactions
        .recovery_prefix()
        .map_err(material_error)?;
    owner
        .machine
        .authenticated_history
        .store
        .require_actual_incoming_history()
        .map_err(map_authenticated_history_error)
}

// A live retry may select only the exact original native WAL prefix before the signed CAS.
// An uncertain descriptor cannot be adopted in memory: it remains retained for disk recovery.
fn select_original_history_predecessor(
    owner: &mut KagemushaAuthenticatedCoreOwnerV1,
    original: &IncomingFoldOriginalV1,
) -> Result<(), KagemushaStateErrorV1> {
    let previous = &original.intent.previous;
    if owner.machine.state != previous.state {
        return Err(KagemushaStateErrorV1::SnapshotRollback);
    }
    if owner.machine.authenticated_history.committed_roots() == previous.authenticated_history_roots
    {
        return Ok(());
    }
    let subject = KagemushaHistoryRootSelectionSubjectV1::new(
        &original.intent.transaction,
        previous.state.hardware_profile_id,
        previous.state.hardware_epoch.generation,
        original
            .hardware_certificate
            .statement
            .journal_revision_after,
    );
    owner
        .machine
        .authenticated_history
        .store
        .select_pending_incoming_predecessor(
            disk_history_lane_binding(previous.state.context(), &previous.state.lane)?,
            &original.intent.transaction,
            KagemushaHistoryRootSelectionCertificateV1::new(subject, original.root_signature),
            previous.authenticated_history_roots,
            previous.authenticated_history_commitment,
        )
        .map_err(map_authenticated_history_error)
}

fn derive_preview(
    owner: &mut KagemushaAuthenticatedCoreOwnerV1,
    kind: Kind,
    credit: CreditIdV1,
    nonce: DigestV1,
    time: u64,
) -> Result<Preview, KagemushaStateErrorV1> {
    match kind {
        Kind::Mint => {
            let credit = selected_mint(owner, credit)?;
            owner
                .machine
                .preview_mint_fold(&credit, nonce, time)
                .map(Preview::Mint)
        }
        Kind::Receive => owner
            .machine
            .preview_receive_fold(credit, nonce, time)
            .map(Preview::Receive),
    }
}

fn selected_mint(
    owner: &KagemushaAuthenticatedCoreOwnerV1,
    id: CreditIdV1,
) -> Result<KagemushaMintCreditV1, KagemushaStateErrorV1> {
    owner
        .machine
        .mint_inbox
        .pending_credit(id)
        .map(|entry| entry.credit().clone())
        .ok_or(KagemushaStateErrorV1::CreditNotStaged(id))
}

fn authenticate_original(
    owner: &KagemushaAuthenticatedCoreOwnerV1,
    preview: &Preview,
    original: &IncomingFoldOriginalV1,
) -> Result<TransitionAuthorizationV1, KagemushaStateErrorV1> {
    let state = &owner.machine.state;
    let key = owner
        .machine
        .authenticated_history
        .store
        .current_device_key(
            state.hardware_profile_id,
            state.hardware_epoch.generation,
            state.device_policy_binding.device_key_reference,
        )
        .map_err(map_authenticated_history_error)?;
    let authorization = TransitionAuthorizationV1::new(
        original.hardware_certificate.clone(),
        original.paired_proof.clone(),
    );
    match preview {
        Preview::Mint(preview) => owner.machine.authorize_mint_fold_history(
            preview,
            authorization,
            &key,
            original.root_signature,
        ),
        Preview::Receive(preview) => owner.machine.authorize_receive_fold_history(
            preview,
            authorization,
            &key,
            original.root_signature,
        ),
    }
}

fn history_material(
    preview: &Preview,
) -> (
    &KagemushaPreparedHistoryCasV1,
    KagemushaHistoryProofRootBridgeRequestV1,
) {
    match preview {
        Preview::Mint(preview) => (
            &preview.authenticated_history_transaction,
            preview.proof_root_bridge_request,
        ),
        Preview::Receive(preview) => (
            &preview.authenticated_history_transaction,
            preview.proof_root_bridge_request,
        ),
    }
}
fn transition(preview: &Preview) -> &TransitionPreviewV1 {
    match preview {
        Preview::Mint(preview) => &preview.transition,
        Preview::Receive(preview) => &preview.transition,
    }
}
fn bounded_canonical(
    value: &impl norito::NoritoSerialize,
) -> Result<Vec<u8>, KagemushaStateErrorV1> {
    let bytes = norito::encode_canonical(value).map_err(material_error)?;
    if bytes.is_empty() || bytes.len() as u64 > FORMAT.maximum_payload_bytes {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(bytes)
}

fn publication_binding(
    directory: &Path,
    checkpoint_id: DigestV1,
) -> Result<String, KagemushaStateErrorV1> {
    let path = directory
        .to_str()
        .ok_or(KagemushaStateErrorV1::InvalidRecoveryMaterial)?;
    if checkpoint_id == [0; 32]
        || !directory.is_absolute()
        || path.len() > 4096
        || path.as_bytes().contains(&0)
        || directory.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(path.to_owned())
}
// Prepared recovery accepts only Intent/Proof; completed recovery additionally needs Completed.
// Read one extra native frame only to reject a suffix. All raw frames still need the existing
// exact schema/proof/Guard/history checks; failure drops the descriptor without yielding an owner.
fn replay_incoming_records(
    journal: &mut PrivateJournal,
    maximum_records: u64,
) -> Result<(), KagemushaStateErrorV1> {
    if !matches!(maximum_records, 2 | 3) {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    for expected_sequence in 0..maximum_records {
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

fn require_original_journal(
    journal: &PrivateJournal,
    original: &IncomingFoldOriginalV1,
) -> Result<(), KagemushaStateErrorV1> {
    require_original_pair_wire_shape(&original.paired_proof)?;
    let intent = bounded_canonical(&original.intent)?;
    let proof = bounded_canonical(&ProofOriginal {
        history_operation_id: original.intent.transaction.transaction_id(),
        paired_proof: original.paired_proof.clone(),
    })?;
    let completed = bounded_canonical(&IncomingOriginalV1::Fold(original.clone()))?;
    let mut count = 0;
    journal
        .scan_complete(|sequence, bytes| {
            if (sequence == 0 && bytes == intent)
                || (sequence == 1 && bytes == proof)
                || (sequence == 2 && bytes == completed)
            {
                count += 1;
                Ok(())
            } else {
                Err(PrivateJournalError::Corrupt)
            }
        })
        .map_err(material_error)?;
    if count != 3 {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}

// Bound untrusted pair wire shape before cloning/re-encoding any decoded original. The claimed
// digest is used only for this structural cap check: it authenticates no selected transition.
// Independent preview semantics, recursive proof, native Guard/history and owner checks remain.
fn require_original_pair_wire_shape(
    proof: &KagemushaPairedProofV1,
) -> Result<(), KagemushaStateErrorV1> {
    validate_paired_proof(proof, proof.semantic_digest)
}

fn decode_proof_original(bytes: &[u8]) -> Result<ProofOriginal, KagemushaStateErrorV1> {
    if bytes.is_empty() || bytes.len() > 8192 {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let proof: ProofOriginal = norito::decode_canonical(bytes).map_err(material_error)?;
    require_original_pair_wire_shape(&proof.paired_proof)?;
    if bounded_canonical(&proof)? != bytes || proof.history_operation_id == [0; 32] {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(proof)
}

fn require_prepared_journal(
    journal: &PrivateJournal,
    intent: &Intent,
    proof: Option<&ProofOriginal>,
) -> Result<(), KagemushaStateErrorV1> {
    let intent_bytes = bounded_canonical(intent)?;
    let proof_bytes = proof.map(bounded_canonical).transpose()?;
    if proof.is_some_and(|value| value.history_operation_id != intent.transaction.transaction_id())
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let mut count = 0;
    journal
        .scan_complete(|sequence, bytes| {
            if (sequence == 0 && bytes == intent_bytes)
                || (sequence == 1
                    && proof_bytes
                        .as_ref()
                        .is_some_and(|original| original == bytes))
            {
                count += 1;
                Ok(())
            } else {
                Err(PrivateJournalError::Corrupt)
            }
        })
        .map_err(material_error)?;
    if count != if proof.is_some() { 2 } else { 1 } {
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
    fn decoded_proof_original_refuses_native_pair_wire_overlimit_before_reencoding() {
        // Untrusted structural shape only: valid framing is never recursive proof authority.
        let original = ProofOriginal {
            history_operation_id: [1; 32],
            paired_proof: KagemushaPairedProofV1 {
                version: 1,
                eq_protocol_digest: [2; 32],
                ep_protocol_digest: [3; 32],
                semantic_digest: [4; 32],
                guard_eq_credential_audit: [5; 32],
                guard_ep_credential_audit: [6; 32],
                eq_deferred_audit: [7; 32],
                ep_deferred_audit: [8; 32],
                eq_proof: vec![9; 8],
                ep_proof: vec![10; 8],
                eq_history: vec![
                    11;
                    iroha_data_model::kagemusha::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1
                ],
                ep_history: vec![
                    12;
                    iroha_data_model::kagemusha::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1
                ],
            },
        };
        let valid_wire = bounded_canonical(&original).unwrap();
        assert!(decode_proof_original(&valid_wire).is_ok());
        for field in 0..2 {
            let mut oversized = original.clone();
            if field == 0 {
                oversized.paired_proof.eq_proof =
                    vec![13; iroha_data_model::kagemusha::KAGEMUSHA_PARITY_PROOF_MAX_BYTES_V1 + 1];
            } else {
                oversized.paired_proof.eq_history = vec![
                    14;
                    iroha_data_model::kagemusha::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1
                        + 1
                ];
            }
            let raw_wire = bounded_canonical(&oversized).unwrap();
            // Both corrupt records fit the old 8192-byte outer cap; the real native pair
            // shape check must reject before require_original_journal clones that pair.
            assert!(raw_wire.len() < 8192);
            assert!(matches!(
                require_original_pair_wire_shape(&oversized.paired_proof),
                Err(KagemushaStateErrorV1::InvalidProofBundle)
            ));
            assert!(matches!(
                decode_proof_original(&raw_wire),
                Err(KagemushaStateErrorV1::InvalidProofBundle)
            ));
        }
    }

    #[test]
    fn incoming_original_wal_replay_bounds_prepared_and_completed_extra_frames() {
        // Raw framing only: no Intent/Proof bytes here authenticate an incoming owner.
        // Production recovery drops a rejected descriptor. Inspecting its next row proves
        // the first record beyond the single rejection lookahead was never processed.
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        for maximum_records in [2, 3] {
            let path = root.join(format!("extra-{maximum_records}"));
            let mut journal = PrivateJournal::create_new(&path, FORMAT).unwrap();
            for value in 1..=(maximum_records + 2) {
                journal.append(&[u8::try_from(value).unwrap()]).unwrap();
            }
            drop(journal);
            let mut reopened = PrivateJournal::open_existing(&path, FORMAT).unwrap();
            assert!(matches!(
                replay_incoming_records(&mut reopened, maximum_records),
                Err(KagemushaStateErrorV1::SnapshotIntegrity)
            ));
            assert!(reopened.recovery_prefix().is_err());
            assert_eq!(
                reopened.replay_next().unwrap(),
                Some((
                    maximum_records + 1,
                    vec![u8::try_from(maximum_records + 2).unwrap()]
                ))
            );
        }
    }

    #[test]
    fn incoming_original_wal_replay_preserves_complete_empty_and_torn_prefixes() {
        // Successful raw replay is data custody only. Shipping recovery separately requires
        // the exact canonical variants and genuine history/candidate/native proof owners.
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let frames: [&[u8]; 3] = [
            b"structural intent",
            b"structural proof",
            b"structural completed",
        ];
        for maximum_records in [2, 3] {
            for count in 0..=maximum_records {
                let path = root.join(format!("complete-{maximum_records}-{count}"));
                let mut journal = PrivateJournal::create_new(&path, FORMAT).unwrap();
                for frame in frames.iter().take(usize::try_from(count).unwrap()) {
                    journal.append(frame).unwrap();
                }
                drop(journal);
                let mut reopened = PrivateJournal::open_existing(&path, FORMAT).unwrap();
                if count == 0 {
                    assert!(replay_incoming_records(&mut reopened, maximum_records).is_err());
                    continue;
                }
                replay_incoming_records(&mut reopened, maximum_records).unwrap();
                assert_eq!(reopened.recovery_prefix().unwrap().sequence, count);
            }
            for completed_frames in 0..=maximum_records {
                let path = root.join(format!("torn-{maximum_records}-{completed_frames}"));
                let mut journal = PrivateJournal::create_new(&path, FORMAT).unwrap();
                for frame in frames
                    .iter()
                    .take(usize::try_from(completed_frames).unwrap())
                {
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
                assert!(replay_incoming_records(&mut reopened, maximum_records).is_err());
                assert!(reopened.recovery_prefix().is_err());
            }
        }
    }

    #[test]
    fn canonical_original_producer_preserves_typed_frame_and_refuses_oversized_payload() {
        let bytes = bounded_canonical(&Kind::Mint).unwrap();
        assert!(matches!(
            norito::decode_from_bytes::<Kind>(&bytes).unwrap(),
            Kind::Mint
        ));
        assert_eq!(bytes, norito::encode_canonical(&Kind::Mint).unwrap());
        // The bound covers the complete canonical frame, including its schema/layout header.
        // Encoding payload bytes alone is never sufficient to enter the original journal.
        assert_eq!(
            bounded_canonical(&vec![0_u8; FORMAT.maximum_payload_bytes as usize]),
            Err(KagemushaStateErrorV1::SnapshotIntegrity),
        );
    }

    #[test]
    fn incoming_original_decoder_refuses_missing_and_oversized_originals() {
        assert!(decode_incoming_original_v1(&[]).is_err());
        assert!(decode_incoming_original_v1(&[1, 2, 3]).is_err());
        assert!(
            decode_incoming_original_v1(&vec![0; FORMAT.maximum_payload_bytes as usize + 1])
                .is_err()
        );
        assert!(decode_intent(&[]).is_err());
        assert!(decode_intent(&[1, 2, 3]).is_err());
        assert!(decode_intent(&bounded_canonical(&Kind::Mint).unwrap()).is_err());
        assert!(decode_intent(&vec![0; FORMAT.maximum_payload_bytes as usize + 1]).is_err());
        assert!(decode_proof_original(&[]).is_err());
        assert!(decode_proof_original(&[1, 2, 3]).is_err());
        assert!(decode_proof_original(&vec![0; 8193]).is_err());
        assert!(decode_proof_original(&bounded_canonical(&Kind::Mint).unwrap()).is_err());
    }

    #[test]
    fn incoming_publication_binding_requires_nonzero_exact_absolute_bounded_destination() {
        let selected = Path::new("/private/wallet/checkpoints/incoming-7");
        assert_eq!(
            publication_binding(selected, [7; 32]).unwrap(),
            selected.to_str().unwrap()
        );
        assert!(publication_binding(selected, [0; 32]).is_err());
        assert!(publication_binding(Path::new("relative/checkpoint"), [7; 32]).is_err());
        assert!(publication_binding(Path::new("/private/../changed/checkpoint"), [7; 32]).is_err());
        assert!(
            publication_binding(Path::new(&format!("/{}", "p".repeat(4096))), [7; 32]).is_err()
        );
    }
}
