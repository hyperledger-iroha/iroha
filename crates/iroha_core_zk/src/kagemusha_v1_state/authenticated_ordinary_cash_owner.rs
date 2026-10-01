//! Native ordinary cash custody carried from the genuine published zero-State owner.
//!
//! The app key approves an exact Native selection; it does not own the financial witness.
//! This journal retains separate cash attempts and complete platform originals. It never
//! converts a captured Bootstrap approval or a decoded journal into a monetary proof.

use super::*;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1,
    KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1, KagemushaAppOperationApprovalChallengeV1,
    KagemushaAppOperationApprovalPurposeV1, KagemushaAppOperationApprovalV1,
    KagemushaHardwareTransitionSelectionV1, KagemushaOperationKindV1,
    KagemushaVerifiedAppOperationApprovalV1,
    kagemusha_ordinary_financial_authorization_proof_binding_digest_v1,
};
use rand_core_06::{OsRng, RngCore as _};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;

#[path = "ordinary_cash_terminal_owner.rs"]
mod terminal;
pub(crate) use terminal::KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1;

const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "ordinary-cash-approval.norito.wal",
    magic: b"IKGOCA1\0",
    hash_domain: b"iroha:kagemusha:v1:ordinary-cash-approval-frame\0",
    maximum_payload_bytes: 64 * 1024,
};
const MAX_ROWS: u64 = 100_000;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryCashApprovalRecordV1")]
enum Record {
    Initialize {
        originals: [DigestV1; 8],
        counter_floor: Option<u32>,
    },
    Intent {
        operation: DigestV1,
        nonce: DigestV1,
        predecessor: DigestV1,
    },
    Preparation {
        statement: TransitionProofStatementV1,
        successor: KagemushaStateV1,
        normalized: KagemushaNormalizedGuardStatementV1,
        context: KagemushaGuardContextV1,
        challenge: KagemushaAppOperationApprovalChallengeV1,
        lease_original: Option<Vec<u8>>,
        counter_floor: Option<u32>,
    },
    PlatformFence {
        operation: DigestV1,
    },
    ApprovalOriginal {
        operation: DigestV1,
        lower_at_ms: u64,
        upper_at_ms: u64,
        original: Vec<u8>,
        authorization_digest: DigestV1,
        accepted_counter: Option<u32>,
    },
    Capture {
        operation: DigestV1,
        lower_at_ms: u64,
        upper_at_ms: u64,
        authorization_digest: DigestV1,
    },
    Cancel {
        operation: DigestV1,
    },
}

struct Pending {
    operation: DigestV1,
    nonce: DigestV1,
    selected: Option<Selected>,
    fenced: bool,
    retained: Option<(u64, u64, KagemushaVerifiedAppOperationApprovalV1)>,
    capture: Option<(u64, u64, KagemushaVerifiedAppOperationApprovalV1)>,
}
struct Selected {
    statement: TransitionProofStatementV1,
    successor: KagemushaStateV1,
    normalized: KagemushaNormalizedGuardStatementV1,
    context: KagemushaGuardContextV1,
    challenge: KagemushaAppOperationApprovalChallengeV1,
    lease: Option<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
    counter_floor: Option<u32>,
}

/// Exclusive Native cash owner retaining the original publication and all its locks.
/// Its only constructor consumes that actual owner and its original recursive verifier.
/// Publication of a financial successor additionally requires actual paired cash proofs.
pub struct KagemushaNativeOrdinaryCashOwnerV1 {
    publication: KagemushaAuthenticatedOrdinaryCurrentPublicationV1,
    verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
    journal: PrivateJournal,
    prefix: KagemushaRecoveryJournalPrefixV1,
    state: KagemushaStateV1,
    counter_floor: Option<u32>,
    pending: Option<Pending>,
    used_operations: BTreeSet<DigestV1>,
    financial_journal_revision: u64,
    terminal: Option<terminal::TerminalJournal>,
}

/// Borrow of one durably captured purpose2 approval under the still-held cash owner.
/// This is proof selection, not a fresh money grant or a captured Bootstrap conversion.
pub(crate) struct KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'a> {
    owner: &'a KagemushaNativeOrdinaryCashOwnerV1,
    prefix: KagemushaRecoveryJournalPrefixV1,
}

impl KagemushaNativeOrdinaryCashOwnerV1 {
    pub(in crate::kagemusha_v1_state::authenticated_core_owner) fn from_publication(
        path: &Path,
        publication: KagemushaAuthenticatedOrdinaryCurrentPublicationV1,
        verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
        recover: bool,
        historical_leases: &[Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>],
        historical_receivers: &[Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>],
    ) -> Result<Self, KagemushaStateErrorV1> {
        publication.recheck()?;
        if historical_leases.len() > 1024 || historical_receivers.len() > 1024 {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let release = admitted_release(&verifier)?;
        if !Arc::ptr_eq(&release, publication.cash_approvals().retained_release()) {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let state = publication.initial_state()?.clone();
        let counter_floor = publication
            .cash_approvals()
            .retained_app_attest_counter_floor();
        let initial = Record::Initialize {
            originals: publication.original_commitments()?,
            counter_floor,
        };
        let mut journal = if recover {
            PrivateJournal::open_existing(path, FORMAT)
        } else {
            PrivateJournal::create_new(path, FORMAT)
        }
        .map_err(storage)?;
        if !recover {
            journal.append(&encode(&initial)?).map_err(storage)?;
        }
        let prefix = journal.recovery_prefix().map_err(storage)?;
        let mut this = Self {
            publication,
            verifier,
            journal,
            prefix,
            state,
            counter_floor,
            pending: None,
            used_operations: BTreeSet::new(),
            financial_journal_revision: 0,
            terminal: None,
        };
        if recover {
            let (first_sequence, first) = this
                .journal
                .replay_next()
                .map_err(storage)?
                .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
            if first_sequence != 0 || decode(&first)? != initial {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            while let Some((sequence, original)) = this.journal.replay_next().map_err(storage)? {
                if sequence >= MAX_ROWS {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                this.replay(decode(&original)?, historical_leases)?;
            }
        }
        this.recheck()?;
        this.terminal = Some(terminal::TerminalJournal::open(
            &path.join("terminal"),
            &this,
            recover,
            historical_leases,
            historical_receivers,
        )?);
        this.recheck()?;
        Ok(this)
    }

    /// Observe the current authenticated private State while its original custody remains held.
    /// A projection cannot reconstruct this owner or authorize a payment.
    pub fn current_state(&self) -> Result<&KagemushaStateV1, KagemushaStateErrorV1> {
        self.recheck()?;
        Ok(&self.state)
    }

    fn recheck(&self) -> Result<(), KagemushaStateErrorV1> {
        self.publication.recheck()?;
        self.journal.check_owned().map_err(storage)?;
        if self.journal.recovery_prefix().map_err(storage)? != self.prefix
            || self.state != *self.publication.initial_state()?
            || !Arc::ptr_eq(
                &admitted_release(&self.verifier)?,
                self.publication.cash_approvals().retained_release(),
            )
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.state.validate()?;
        if let Some(terminal) = &self.terminal {
            terminal.recheck()?;
        }
        self.publication
            .cash_financial()
            .trusted_time_interval()
            .map_err(material)?
            .check_both(|now| {
                self.publication
                    .cash_approvals()
                    .recheck_at_trusted_time(now)
            })
    }

    fn persist(&mut self, record: &Record) -> Result<(), KagemushaStateErrorV1> {
        self.recheck()?;
        if self.prefix.sequence >= MAX_ROWS {
            return Err(KagemushaStateErrorV1::JournalRevisionOverflow);
        }
        self.journal.append(&encode(record)?).map_err(storage)?;
        self.prefix = self.journal.recovery_prefix().map_err(storage)?;
        self.recheck()
    }

    /// Reserve actual Native entropy and exact held predecessor before deriving purpose2 S/W.
    /// This internal operation grants only an approval attempt, not funds or an outbox slot.
    pub(crate) fn reserve_preparation(&mut self) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.recheck()?;
        if self.pending.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let mut entropy = [0; 64];
        OsRng.try_fill_bytes(&mut entropy).map_err(material)?;
        let mut hash = Sha256::new();
        hash.update(b"iroha:kagemusha:v1:ordinary-cash-native-operation\0");
        hash.update(self.prefix.head);
        hash.update(self.prefix.sequence.to_le_bytes());
        hash.update(self.state.state_commitment);
        hash.update(&entropy[..32]);
        let operation = hash.finalize().into();
        let nonce: DigestV1 = entropy[32..].try_into().map_err(material)?;
        if operation == [0; 32] || nonce == [0; 32] || self.used_operations.contains(&operation) {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.persist(&Record::Intent {
            operation,
            nonce,
            predecessor: self.state.state_commitment,
        })?;
        self.used_operations.insert(operation);
        self.pending = Some(Pending {
            operation,
            nonce,
            selected: None,
            fenced: false,
            retained: None,
            capture: None,
        });
        Ok(operation)
    }

    /// Bind Native-derived transition data to the reserved cash attempt before any OS call.
    /// Arguments are internal financial preparation data; no C/JNI/raw-owner constructor exists.
    /// The predecessor, subtraction, complete successor, original C and both clock bounds are
    /// independently reconstructed or checked here. Proof/transport/outbox admission is separate.
    pub(crate) fn select_preparation(
        &mut self,
        operation: DigestV1,
        statement: TransitionProofStatementV1,
        successor: KagemushaStateV1,
        context: KagemushaGuardContextV1,
    ) -> Result<KagemushaAppOperationApprovalChallengeV1, KagemushaStateErrorV1> {
        self.recheck()?;
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if pending.operation != operation || pending.selected.is_some() || pending.fenced {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        require_outgoing(
            &self.state,
            &successor,
            &statement,
            self.financial_journal_revision,
        )?;
        let normalized =
            KagemushaNormalizedGuardStatementV1::derive_from_transition(&statement, context)
                .map_err(material)?;
        if normalized.terminal_commit_binding_digest != [0; 32]
            || normalized.sender_one_time_authorization_digest != [0; 32]
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let floor = self.credential_floor()?;
        floor.validate_current(&self.state)?;
        floor.validate_current(&successor)?;
        let c = floor.credential();
        let interval = self
            .publication
            .cash_financial()
            .trusted_time_interval()
            .map_err(material)?;
        let issued_at_ms = interval.lower_ms();
        let expires_at_ms = issued_at_ms
            .checked_add(KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1)
            .ok_or(KagemushaStateErrorV1::InvalidTrustedCommitTime)?
            .min(floor.approval_valid_until_ms());
        interval
            .require_validity(issued_at_ms, expires_at_ms)
            .map_err(material)?;
        let subject = preparation_subject(&self.state, &successor, &statement, c)?;
        let challenge = KagemushaAppOperationApprovalChallengeV1 {
            version: 1,
            purpose: KagemushaAppOperationApprovalPurposeV1::PrepareTransition,
            operation_id: operation,
            nonce: pending.nonce,
            account_binding: c.subject().account_binding,
            authority_policy_digest: c.subject().app_authority_policy_digest,
            attested_key_id: c.subject().attested_key_id,
            enrollment_digest: c.digest(),
            subject_signing_digest: Sha256::digest(
                subject
                    .canonical_prepare_signing_bytes()
                    .map_err(material)?,
            )
            .into(),
            normalized_guard_digest: normalized.canonical_digest().map_err(material)?,
            issued_at_ms,
            expires_at_ms,
            subject,
        };
        challenge.canonical_signing_bytes().map_err(material)?;
        let lease = self
            .publication
            .cash_financial()
            .retained_integrity_lease()
            .cloned();
        self.persist(&Record::Preparation {
            statement: statement.clone(),
            successor: successor.clone(),
            normalized: normalized.clone(),
            context,
            challenge,
            lease_original: lease.as_ref().map(|l| l.original().to_vec()),
            counter_floor: self.counter_floor,
        })?;
        self.pending
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .selected = Some(Selected {
            statement,
            successor,
            normalized,
            context,
            challenge,
            lease,
            counter_floor: self.counter_floor,
        });
        self.require_live_preparation(operation)?;
        Ok(challenge)
    }

    /// Durably fence exactly one selected platform invocation; retry never repeats that call.
    pub(crate) fn fence_preparation_platform(
        &mut self,
        operation: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_live_preparation(operation)?;
        if self.pending.as_ref().is_some_and(|p| p.fenced) {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.persist(&Record::PlatformFence { operation })?;
        self.pending
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .fenced = true;
        self.require_live_preparation(operation)
    }

    /// Verify exact DER/CBOR and selected PI under the Native interval, fsync the complete
    /// original, and recheck the live interval after publication before exposing proof selection.
    pub(crate) fn capture_preparation_original(
        &mut self,
        operation: DigestV1,
        original: &[u8],
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_live_preparation(operation)?;
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if !pending.fenced || pending.retained.is_some() || pending.capture.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let selected = pending
            .selected
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let interval = self
            .publication
            .cash_financial()
            .trusted_time_interval()
            .map_err(material)?;
        let approved = self.authenticate(original, selected, interval.lower_ms())?;
        self.authenticate(original, selected, interval.upper_ms())?;
        let authorization_digest = authorization(&approved, selected.lease.as_deref())?;
        self.persist(&Record::ApprovalOriginal {
            operation,
            lower_at_ms: interval.lower_ms(),
            upper_at_ms: interval.upper_ms(),
            original: original.to_vec(),
            authorization_digest,
            accepted_counter: approved.app_attest_counter(),
        })?;
        self.counter_floor = approved.app_attest_counter().or(self.counter_floor);
        self.pending
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .retained = Some((interval.lower_ms(), interval.upper_ms(), approved));
        // The complete original is now fsynced. A fresh Native sample must still be live before
        // acknowledging capture; an expired/uncertain original cannot become a historical loan.
        self.acknowledge_preparation_capture(operation)
    }

    pub(crate) fn acknowledge_preparation_capture(
        &mut self,
        operation: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_live_preparation(operation)?;
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if pending.capture.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let selected = pending
            .selected
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let retained = pending
            .retained
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let interval = self
            .publication
            .cash_financial()
            .trusted_time_interval()
            .map_err(material)?;
        interval.check_both(|now| retained.2.recheck_at_trusted_time(now).map_err(material))?;
        let authorization_digest = authorization(&retained.2, selected.lease.as_deref())?;
        self.persist(&Record::Capture {
            operation,
            lower_at_ms: interval.lower_ms(),
            upper_at_ms: interval.upper_ms(),
            authorization_digest,
        })?;
        let retained = self
            .pending
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .retained
            .take()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        self.pending
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .capture = Some((interval.lower_ms(), interval.upper_ms(), retained.2));
        // Capture follows a verified post-fsync live sample. Ack persistence/proving may outlast W;
        // subsequent use keeps that immutable instant and rechecks current FI/C/PI separately.
        self.captured_preparation()?
            .recheck_selected_originals_and_current_custody()
    }

    pub(crate) fn captured_preparation(
        &self,
    ) -> Result<KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_>, KagemushaStateErrorV1>
    {
        let loan = KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1 {
            owner: self,
            prefix: self.prefix,
        };
        loan.recheck_selected_originals_and_current_custody()?;
        Ok(loan)
    }

    pub(crate) fn cancel_preparation(
        &mut self,
        operation: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck()?;
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if pending.operation != operation || pending.capture.is_some() {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.persist(&Record::Cancel { operation })?;
        self.pending = None;
        Ok(())
    }

    fn credential_floor(
        &self,
    ) -> Result<KagemushaAuthenticatedOrdinaryCredentialFloorV1<'_>, KagemushaStateErrorV1> {
        let financial = self.publication.cash_financial();
        let release = Arc::clone(self.publication.cash_approvals().retained_release());
        match financial.retained_integrity_lease() {
            Some(lease) => KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment_with_integrity_lease(
                financial.enrollment(), release, lease, financial.trusted_time_ms().map_err(material)?),
            None => KagemushaAuthenticatedOrdinaryCredentialFloorV1::from_verified_enrollment(financial.enrollment(), release),
        }
    }

    fn require_live_preparation(&self, operation: DigestV1) -> Result<(), KagemushaStateErrorV1> {
        self.recheck()?;
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let selected = pending
            .selected
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if pending.operation != operation {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        require_outgoing(
            &self.state,
            &selected.successor,
            &selected.statement,
            self.financial_journal_revision,
        )?;
        let interval = self
            .publication
            .cash_financial()
            .trusted_time_interval()
            .map_err(material)?;
        interval
            .require_validity(
                selected.challenge.issued_at_ms,
                selected.challenge.expires_at_ms,
            )
            .map_err(material)?;
        if let Some((_, _, approval)) = &pending.capture {
            interval.check_both(|now| approval.recheck_at_trusted_time(now).map_err(material))?;
        }
        Ok(())
    }

    fn authenticate(
        &self,
        original: &[u8],
        selected: &Selected,
        now: u64,
    ) -> Result<KagemushaVerifiedAppOperationApprovalV1, KagemushaStateErrorV1> {
        if original.is_empty() || original.len() > KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1 {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let value: KagemushaAppOperationApprovalV1 = norito::decode_canonical_with_limits(
            original,
            norito::canonical_decode_limits(original.len()),
        )
        .map_err(material)?;
        if norito::encode_canonical(&value).map_err(material)? != original {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let c = self
            .publication
            .cash_financial()
            .enrollment()
            .app_credential();
        match &selected.lease {
            Some(lease) => value.authenticate_with_integrity_lease(
                &selected.challenge,
                c,
                lease,
                selected.counter_floor,
                now,
            ),
            None => value.authenticate(&selected.challenge, c, selected.counter_floor, now),
        }
        .map_err(material)
    }

    fn replay(
        &mut self,
        record: Record,
        historical_leases: &[Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>],
    ) -> Result<(), KagemushaStateErrorV1> {
        match record {
            Record::Intent {
                operation,
                nonce,
                predecessor,
            } if self.pending.is_none()
                && operation != [0; 32]
                && nonce != [0; 32]
                && predecessor == self.state.state_commitment
                && !self.used_operations.contains(&operation) =>
            {
                self.used_operations.insert(operation);
                self.pending = Some(Pending {
                    operation,
                    nonce,
                    selected: None,
                    fenced: false,
                    retained: None,
                    capture: None,
                });
            }
            Record::Preparation {
                statement,
                successor,
                normalized,
                context,
                challenge,
                lease_original,
                counter_floor,
            } => {
                let pending = self
                    .pending
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                if pending.selected.is_some()
                    || pending.fenced
                    || challenge.operation_id != pending.operation
                    || challenge.nonce != pending.nonce
                    || challenge.purpose
                        != KagemushaAppOperationApprovalPurposeV1::PrepareTransition
                    || counter_floor != self.counter_floor
                    || challenge.subject.transition_statement_digest != statement.digest()?
                    || challenge.normalized_guard_digest
                        != normalized.canonical_digest().map_err(material)?
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                require_outgoing(
                    &self.state,
                    &successor,
                    &statement,
                    self.financial_journal_revision,
                )?;
                if KagemushaNormalizedGuardStatementV1::derive_from_transition(&statement, context)
                    .map_err(material)?
                    != normalized
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                let c = self
                    .publication
                    .cash_financial()
                    .enrollment()
                    .app_credential();
                if challenge.subject != preparation_subject(&self.state, &successor, &statement, c)?
                    || challenge.account_binding != c.subject().account_binding
                    || challenge.authority_policy_digest != c.subject().app_authority_policy_digest
                    || challenge.attested_key_id != c.subject().attested_key_id
                    || challenge.enrollment_digest != c.digest()
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                challenge.canonical_signing_bytes().map_err(material)?;
                let lease = match lease_original {
                    None => None,
                    Some(raw) => Some(Arc::clone(
                        historical_leases
                            .iter()
                            .find(|l| l.original() == raw)
                            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
                    )),
                };
                self.pending
                    .as_mut()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                    .selected = Some(Selected {
                    statement,
                    successor,
                    normalized,
                    context,
                    challenge,
                    lease,
                    counter_floor,
                });
            }
            Record::PlatformFence { operation } => {
                let pending = self
                    .pending
                    .as_mut()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                if pending.operation != operation || pending.selected.is_none() || pending.fenced {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                pending.fenced = true;
            }
            Record::ApprovalOriginal {
                operation,
                lower_at_ms,
                upper_at_ms,
                original,
                authorization_digest,
                accepted_counter,
            } => {
                let pending = self
                    .pending
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                if pending.operation != operation
                    || !pending.fenced
                    || pending.retained.is_some()
                    || pending.capture.is_some()
                    || lower_at_ms == 0
                    || lower_at_ms > upper_at_ms
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                let selected = pending
                    .selected
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                let approved = self.authenticate(&original, selected, lower_at_ms)?;
                self.authenticate(&original, selected, upper_at_ms)?;
                if authorization(&approved, selected.lease.as_deref())? != authorization_digest
                    || approved.app_attest_counter() != accepted_counter
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                self.counter_floor = accepted_counter.or(self.counter_floor);
                self.pending
                    .as_mut()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                    .retained = Some((lower_at_ms, upper_at_ms, approved));
            }
            Record::Capture {
                operation,
                lower_at_ms,
                upper_at_ms,
                authorization_digest,
            } => {
                let pending = self
                    .pending
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                let selected = pending
                    .selected
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                let retained = pending
                    .retained
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                if pending.operation != operation
                    || !pending.fenced
                    || pending.capture.is_some()
                    || lower_at_ms < retained.0
                    || upper_at_ms < retained.1
                    || lower_at_ms > upper_at_ms
                    || authorization(&retained.2, selected.lease.as_deref())?
                        != authorization_digest
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                retained
                    .2
                    .recheck_at_trusted_time(lower_at_ms)
                    .map_err(material)?;
                retained
                    .2
                    .recheck_at_trusted_time(upper_at_ms)
                    .map_err(material)?;
                let retained = self
                    .pending
                    .as_mut()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                    .retained
                    .take()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                self.pending
                    .as_mut()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                    .capture = Some((lower_at_ms, upper_at_ms, retained.2));
            }
            Record::Cancel { operation }
                if self
                    .pending
                    .as_ref()
                    .is_some_and(|p| p.operation == operation && p.capture.is_none()) =>
            {
                self.pending = None;
            }
            _ => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
        }
        Ok(())
    }
}

impl KagemushaAuthenticatedOrdinaryCashApprovalSelectionV1<'_> {
    pub(crate) fn selected_predecessor_state(&self) -> &KagemushaStateV1 {
        &self.owner.state
    }
    pub(crate) fn selected_successor_state(&self) -> &KagemushaStateV1 {
        &self.selected().successor
    }
    pub(crate) fn transition_statement(&self) -> &TransitionProofStatementV1 {
        &self.selected().statement
    }
    pub(crate) fn with_borrowed_financial_secret(
        &self,
        consume: &mut dyn for<'secret> FnMut(
            &'secret [u8; 32],
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        let financial = self.owner.publication.cash_financial();
        let secret = financial.financial_secret().map_err(material)?;
        if crate::kagemusha_v1_recursion::device_authority_commitment_v1(*secret)
            != self
                .enrollment()
                .app_credential()
                .subject()
                .financial_authority_commitment
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let result = consume(secret);
        self.recheck_selected_originals_and_current_custody()?;
        result
    }
    pub(crate) fn recheck_selected_originals_and_current_custody(
        &self,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.owner.recheck()?;
        if self.owner.prefix != self.prefix {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let pending = self
            .owner
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let selected = pending
            .selected
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let (lower, upper, approval) = pending
            .capture
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if !pending.fenced || approval.challenge() != &selected.challenge {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        require_outgoing(
            &self.owner.state,
            &selected.successor,
            &selected.statement,
            self.owner.financial_journal_revision,
        )?;
        approval.recheck_at_trusted_time(*lower).map_err(material)?;
        approval.recheck_at_trusted_time(*upper).map_err(material)?;
        self.owner
            .publication
            .cash_financial()
            .trusted_time_interval()
            .map_err(material)?
            .check_both(|now| {
                if now < *lower {
                    return Err(KagemushaStateErrorV1::SnapshotRollback);
                }
                Ok(())
            })
    }
    fn selected(&self) -> &Selected {
        self.owner
            .pending
            .as_ref()
            .expect("retained cash attempt")
            .selected
            .as_ref()
            .expect("retained cash selection")
    }
    fn approved(&self) -> &KagemushaVerifiedAppOperationApprovalV1 {
        &self
            .owner
            .pending
            .as_ref()
            .expect("retained cash attempt")
            .capture
            .as_ref()
            .expect("retained cash capture")
            .2
    }
    pub(crate) fn enrollment(&self) -> &KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1 {
        self.owner.publication.cash_financial().enrollment()
    }
    pub(crate) fn authenticated_release(
        &self,
    ) -> Result<Arc<KagemushaAuthenticatedReleaseV1>, KagemushaStateErrorV1> {
        self.recheck_selected_originals_and_current_custody()?;
        Ok(Arc::clone(
            self.owner.publication.cash_approvals().retained_release(),
        ))
    }
    pub(crate) fn recursive_verifier(&self) -> &KagemushaAuthenticatedRecursiveVerifierV1 {
        &self.owner.verifier
    }
    pub(crate) fn normalized_guard_statement(&self) -> &KagemushaNormalizedGuardStatementV1 {
        &self.selected().normalized
    }
    pub(crate) fn challenge(&self) -> &KagemushaAppOperationApprovalChallengeV1 {
        &self.selected().challenge
    }
    pub(crate) fn authorization_binding_digest(&self) -> Result<DigestV1, KagemushaStateErrorV1> {
        authorization(self.approved(), self.selected().lease.as_deref())
    }
    pub(crate) fn original(&self) -> &[u8] {
        self.approved().original()
    }
    pub(crate) fn original_approval_integrity_lease(
        &self,
    ) -> Option<&Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>> {
        self.selected().lease.as_ref()
    }
    pub(crate) fn previous_app_attest_counter(&self) -> Option<u32> {
        self.selected().counter_floor
    }
    pub(crate) fn approval_admission_time_ms(&self) -> u64 {
        self.owner
            .pending
            .as_ref()
            .expect("retained cash attempt")
            .capture
            .as_ref()
            .expect("retained cash capture")
            .0
    }
}

fn require_outgoing(
    before: &KagemushaStateV1,
    after: &KagemushaStateV1,
    statement: &TransitionProofStatementV1,
    financial_journal_revision: u64,
) -> Result<(), KagemushaStateErrorV1> {
    if !matches!(
        statement.kind,
        KagemushaTransitionKindV1::SendSplit | KagemushaTransitionKindV1::RedeemSplit
    ) || statement.amount == 0
    {
        return Err(KagemushaStateErrorV1::InvalidCandidateStage);
    }
    let balance = before
        .balance
        .checked_sub(statement.amount)
        .ok_or(KagemushaStateErrorV1::InsufficientBalance)?;
    let expected = KagemushaStateV1::build(
        before.context(),
        before.liability_pool_id,
        before.lane.clone(),
        balance,
        before
            .logical_sequence
            .checked_add(1)
            .ok_or(KagemushaStateErrorV1::SequenceOverflow)?,
        before
            .secure_index
            .checked_add(1)
            .ok_or(KagemushaStateErrorV1::SequenceOverflow)?,
        before.hardware_epoch,
        before.device_policy_binding,
        after.state_nonce_commitment,
        before.consumed_credit_root,
    )?;
    if statement.journal_revision_before != u128::from(financial_journal_revision)
        || statement.journal_revision_after
            != u128::from(
                financial_journal_revision
                    .checked_add(1)
                    .ok_or(KagemushaStateErrorV1::JournalRevisionOverflow)?,
            )
        || expected != *after
        || statement.predecessor_commitment != before.state_commitment
        || statement.successor_commitment != after.state_commitment
        || statement.predecessor_sequence != before.logical_sequence
        || statement.successor_sequence != after.logical_sequence
        || statement.predecessor_state_nonce_commitment != before.state_nonce_commitment
        || statement.successor_state_nonce_commitment != after.state_nonce_commitment
        || statement.predecessor_suite_id != before.suite_id
        || statement.predecessor_vk_digest != before.vk_digest
        || statement.successor_suite_id != after.suite_id
        || statement.successor_vk_digest != after.vk_digest
        || statement.predecessor_release_id != before.release_id
        || statement.release_id != after.release_id
        || statement.asset_incarnation != before.asset_incarnation
        || statement.liability_pool_id != before.liability_pool_id
        || statement.hardware_profile_id != before.hardware_profile_id
        || statement.policy_epoch != before.policy_epoch
        || statement.lane != before.lane
        || statement.predecessor_epoch != before.hardware_epoch
        || statement.successor_epoch != after.hardware_epoch
        || statement.predecessor_device_policy_binding != before.device_policy_binding
        || statement.successor_device_policy_binding != after.device_policy_binding
        || before.next_one_use_key_reference != [0; 32]
        || after.state_nonce_commitment == before.state_nonce_commitment
    {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(())
}
fn authorization(
    approval: &KagemushaVerifiedAppOperationApprovalV1,
    lease: Option<&KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
) -> Result<DigestV1, KagemushaStateErrorV1> {
    kagemusha_ordinary_financial_authorization_proof_binding_digest_v1(
        approval.proof_binding_digest(),
        lease.map(|l| l.digest()),
    )
    .map_err(material)
}
fn encode(record: &Record) -> Result<Vec<u8>, KagemushaStateErrorV1> {
    let original = norito::encode_canonical(record).map_err(material)?;
    if original.is_empty() || original.len() as u64 > FORMAT.maximum_payload_bytes {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(original)
}
fn decode(original: &[u8]) -> Result<Record, KagemushaStateErrorV1> {
    if original.is_empty() || original.len() as u64 > FORMAT.maximum_payload_bytes {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let record = norito::decode_canonical_with_limits(
        original,
        norito::canonical_decode_limits(original.len()),
    )
    .map_err(material)?;
    if encode(&record)? != original {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(record)
}
fn storage(_: PrivateJournalError) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::SnapshotIntegrity
}
fn material(_: impl std::fmt::Display) -> KagemushaStateErrorV1 {
    KagemushaStateErrorV1::SnapshotIntegrity
}

fn preparation_subject(
    before: &KagemushaStateV1,
    after: &KagemushaStateV1,
    statement: &TransitionProofStatementV1,
    c: &KagemushaVerifiedOrdinaryAppCredentialV1,
) -> Result<KagemushaHardwareTransitionSelectionV1, KagemushaStateErrorV1> {
    Ok(KagemushaHardwareTransitionSelectionV1 {
        version: 1,
        release_id: before.release_id,
        provider_policy_root: before.device_policy_binding.hardware_policy_id,
        app_policy_digest: c.static_binding_digest(),
        credential_id: c.digest(),
        network_id: before.lane.network_id,
        lane_commitment: before.lane.device_lane_id,
        hardware_profile_id: before.hardware_profile_id,
        policy_epoch: before.policy_epoch,
        hardware_epoch_id: before.hardware_epoch.epoch_id,
        hardware_epoch_generation: u64::try_from(before.hardware_epoch.generation)
            .map_err(material)?,
        operation_kind: match statement.kind {
            KagemushaTransitionKindV1::SendSplit => KagemushaOperationKindV1::SendSplit,
            KagemushaTransitionKindV1::RedeemSplit => KagemushaOperationKindV1::RedeemSplit,
            _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
        },
        transition_statement_digest: statement.digest()?,
        candidate_envelope_digest: [0; 32],
        terminal_body_commitment: [0; 32],
        secure_index_before: before.secure_index,
        secure_index_after: after.secure_index,
    })
}

#[cfg(test)]
#[path = "ordinary_cash_owner_tests.rs"]
mod tests;
