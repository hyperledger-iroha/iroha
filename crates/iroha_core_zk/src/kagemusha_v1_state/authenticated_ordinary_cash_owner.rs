//! Native ordinary cash custody carried from the genuine published zero-State owner.
//!
//! The app key approves an exact Native selection; it does not own the financial witness.
//! One exclusive WAL orders preparation, terminal capture and subsequent financial State commits.
//! This journal retains separate cash attempts and complete platform originals. It never
//! converts a captured Bootstrap approval or a decoded journal into a monetary proof.

use super::*;
use crate::kagemusha_v1_recursion::{
    KagemushaOrdinaryCashCarrierBudgetV1, KagemushaOrdinaryLineageStateOriginalV1,
    KagemushaOrdinaryLineageStateProofBundleV1, ordinary_cash_carrier_budget_v1,
};
use iroha_data_model::kagemusha::{
    KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_BYTES_V1, KagemushaAppOperationApprovalChallengeV1,
    KagemushaAppOperationApprovalPurposeV1, KagemushaAppOperationApprovalV1,
    KagemushaHardwareTransitionSelectionV1, KagemushaOperationKindV1,
    KagemushaOrdinaryCashClockContextV1, KagemushaOrdinaryLineageAnchorV1,
    KagemushaOrdinaryPaymentOutputV1, KagemushaOutboxReservationV1,
    KagemushaVerifiedAppOperationApprovalV1,
    kagemusha_ordinary_financial_authorization_proof_binding_digest_v1,
};
use rand_core_06::{OsRng, RngCore as _};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[path = "ordinary_send_credit_factory.rs"]
mod send_credit;
#[path = "ordinary_cash_terminal_owner.rs"]
mod terminal;
use send_credit::SendCreditOriginals;
#[path = "ordinary_redeem_factory.rs"]
mod redeem_credit;
use redeem_credit::RedeemOriginals;
#[path = "ordinary_incoming_owner.rs"]
mod incoming;
#[path = "ordinary_incoming_preparation.rs"]
mod incoming_preparation;
use incoming_preparation::IncomingApprovalRecord;
#[path = "ordinary_incoming_reservation_owner.rs"]
mod incoming_reservation;
use incoming_reservation::IncomingReservationCandidateOriginals;
#[path = "ordinary_outgoing_proof_operands.rs"]
mod outgoing_proof_operands;
use outgoing_proof_operands::OutgoingProofOperandOriginals;
#[path = "ordinary_outgoing_reservation_owner.rs"]
mod outgoing_reservation;
use outgoing_reservation::OutgoingReservationCandidateOriginals;
#[path = "ordinary_incoming_native_driver.rs"]
mod incoming_native_driver;
#[path = "ordinary_incoming_terminal.rs"]
mod incoming_terminal;
#[path = "ordinary_outgoing_native_driver.rs"]
mod outgoing_native_driver;
pub(crate) use incoming_preparation::KagemushaAuthenticatedOrdinaryIncomingApprovalSelectionV1;
pub(crate) use incoming_terminal::KagemushaAuthenticatedOrdinaryIncomingTerminalApprovalSelectionV1;
#[path = "ordinary_cash_lineage_transport.rs"]
mod lineage_transport;
#[path = "ordinary_mint_capture.rs"]
mod mint_capture;
use incoming::{IncomingIntentOriginals, PendingIncoming};
#[path = "ordinary_cash_preparation_originals.rs"]
mod preparation_originals;
#[path = "ordinary_received_source_inbox.rs"]
mod received_source;
#[path = "ordinary_receiver_request_factory.rs"]
mod receiver_request;
pub(crate) use mint_capture::KagemushaAuthenticatedOrdinaryMintApprovalSelectionV1;
use mint_capture::{MintRecord, PendingMint};
#[path = "ordinary_mint_funding.rs"]
mod mint_funding;
pub use mint_funding::{
    KagemushaAuthenticatedOrdinaryMintAccountSigningV1,
    KagemushaAuthenticatedOrdinaryMintFundingTransportV1,
    KagemushaAuthenticatedOrdinaryMintTransactionSigningV1,
};
#[path = "ordinary_incoming_state_commit.rs"]
mod incoming_state_commit;
#[path = "ordinary_cash_state_commit.rs"]
mod state_commit;
use incoming_state_commit::{
    IncomingPreparedCommitAdmission, IncomingPreparedCommitOriginals,
    IncomingStateAdvanceAcknowledgment, IncomingStateAdvanceOriginals,
    RetainedFinancialStateAdvance, RetainedIncomingCommit,
};
use received_source::{ReceivedSourceAdmission, ReceivedSourceOriginals};
use receiver_request::{CapturedReceiverRequestOriginals, ReceiverRequestOriginals};
pub(crate) use receiver_request::{
    KagemushaAuthenticatedOrdinaryReceiverRequestCustodyV1,
    KagemushaHistoricalOrdinaryReceiverRequestCustodyV1,
};
use state_commit::{
    FinalizedDeliveryOriginals, PreparedCommitAdmission, PreparedCommitOriginals, RetainedDelivery,
    RetainedStateAdvance, StateAdvanceAcknowledgment,
};
pub(crate) use terminal::KagemushaAuthenticatedOrdinaryCashTerminalApprovalSelectionV1;

#[path = "ordinary_cash_platform_preparation.rs"]
mod platform_preparation;
pub use platform_preparation::KagemushaNativeOrdinaryPreparedCashApprovalV1;

fn cash_journal_format(maximum_payload_bytes: u64) -> PrivateJournalFormat {
    PrivateJournalFormat {
        filename: "ordinary-cash.norito.wal",
        magic: b"IKGOCS1\0",
        hash_domain: b"iroha:kagemusha:v1:ordinary-cash-state-frame\0",
        maximum_payload_bytes,
    }
}

/// Finite whole Main row ceiling selected from the same authenticated release protocols.
/// These numeric bounds grant no owner. Every complete physical frame is checked separately.
fn cash_record_payload_limit(
    private_service: u64,
    outbox_slot: u64,
    incoming_commit: u64,
    private_checkpoint: u64,
) -> Result<u64, KagemushaStateErrorV1> {
    [
        private_service,
        outbox_slot,
        incoming_commit,
        private_checkpoint,
    ]
    .into_iter()
    .try_fold(128 * 1024u64, |sum, bytes| {
        if bytes == 0 {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        sum.checked_add(bytes)
            .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)
    })
}
fn released_cash_record_payload_limit(
    verifier: &KagemushaAuthenticatedRecursiveVerifierV1,
    budget: &KagemushaOrdinaryCashCarrierBudgetV1,
) -> Result<u64, KagemushaStateErrorV1> {
    cash_record_payload_limit(
        u64::from(budget.private_service_max_bytes()),
        u64::from(budget.required_outbox_slot_bytes()),
        u64::try_from(crate::kagemusha_v1_recursion::KAGEMUSHA_ORDINARY_INCOMING_COMMIT_BUNDLE_MAX_BYTES_V1).map_err(material)?,
        u64::try_from(crate::kagemusha_v1_recursion::KagemushaRecursiveStateCheckpointV1::maximum_encoded_bytes(verifier).map_err(material)?).map_err(material)?,
    )
}
const MAX_ROWS: u64 = 100_000;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryCashApprovalRecordV1")]
enum Record {
    Initialize {
        originals: [DigestV1; 8],
        counter_floor: Option<u32>,
        capacity: KagemushaDurableCapacityV1,
        lineage_originals: [DigestV1; 4],
        maximum_record_payload_bytes: u64,
    },
    LineageAnchorAcknowledged {
        request_original_sha256: DigestV1,
    },
    Intent {
        operation: DigestV1,
        nonce: DigestV1,
        predecessor: DigestV1,
        financial_control: CapturedFinancialControlIdentity,
        preparation_clock: KagemushaOrdinaryCashClockContextV1,
        reservation: KagemushaOutboxReservationV1,
    },
    SendCredit {
        successor: KagemushaStateV1,
        originals: SendCreditOriginals,
    },
    RedeemCredit {
        successor: KagemushaStateV1,
        originals: RedeemOriginals,
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
    Terminal(terminal::TerminalRecord),
    IncomingReservationCandidate(IncomingReservationCandidateOriginals),
    OutgoingProofOperands(OutgoingProofOperandOriginals),
    OutgoingReservationCandidate(OutgoingReservationCandidateOriginals),
    IncomingPrepareCommit(IncomingPreparedCommitOriginals),
    IncomingStateAdvance(IncomingStateAdvanceOriginals),
    IncomingStateAdvanceAcknowledged {
        commit_request_original_sha256: DigestV1,
        acknowledgement: IncomingStateAdvanceAcknowledgment,
    },
    PrepareCommit(PreparedCommitOriginals),
    StateAdvance {
        prepared_original_sha256: DigestV1,
        delivery: FinalizedDeliveryOriginals,
    },
    StateAdvanceAcknowledged {
        commit_request_original_sha256: DigestV1,
        acknowledgment: StateAdvanceAcknowledgment,
    },
    ReceiverReserve {
        originals: ReceiverRequestOriginals,
        financial_control: CapturedFinancialControlIdentity,
    },
    ReceiverPlatformFence {
        request_id: DigestV1,
    },
    ReceiverCapture(CapturedReceiverRequestOriginals),
    Mint(MintRecord),
    ReceivedSource(ReceivedSourceOriginals),
    IncomingIntent(IncomingIntentOriginals),
    IncomingApproval(IncomingApprovalRecord),
    IncomingTerminal(incoming_terminal::IncomingTerminalRecord),
    ReceiverCancel {
        request_id: DigestV1,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_core::zk::kagemusha_v1_state::CapturedOrdinaryFinancialControlIdentityV1"
)]
struct CapturedFinancialControlIdentity {
    original_sha256: DigestV1,
    lower_ms: u64,
    upper_ms: u64,
}

// Private operation identity selection only. These data cannot construct a captured
// financial decision; the actual control owner independently verifies the selected original.
#[derive(Clone, Copy, Debug)]
enum ProvingHistoryOperation {
    OutgoingApproval,
    TerminalApproval,
    IncomingApproval,
    IncomingTerminal,
}
impl ProvingHistoryOperation {
    fn select_financial_control_identity(
        self,
        outgoing: Option<CapturedFinancialControlIdentity>,
        terminal: Option<CapturedFinancialControlIdentity>,
        incoming: Option<CapturedFinancialControlIdentity>,
        incoming_terminal: Option<CapturedFinancialControlIdentity>,
    ) -> Result<CapturedFinancialControlIdentity, KagemushaStateErrorV1> {
        match self {
            Self::OutgoingApproval => outgoing,
            Self::TerminalApproval => terminal,
            Self::IncomingApproval => incoming,
            Self::IncomingTerminal => incoming_terminal,
        }
        .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)
    }
}

struct RecoveryCatalog {
    leases: Vec<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
    receivers: Vec<Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>>,
}

struct Pending {
    operation: DigestV1,
    nonce: DigestV1,
    financial_control: CapturedFinancialControlIdentity,
    preparation_clock: KagemushaOrdinaryCashClockContextV1,
    reservation: KagemushaOutboxReservationV1,
    send_credit: Option<RetainedSendCredit>,
    redeem_credit: Option<(KagemushaStateV1, RedeemOriginals)>,
    selected: Option<Selected>,
    fenced: bool,
    retained: Option<(u64, u64, KagemushaVerifiedAppOperationApprovalV1)>,
    capture: Option<(u64, u64, KagemushaVerifiedAppOperationApprovalV1)>,
}
struct PendingReceiverRequest {
    originals: ReceiverRequestOriginals,
    financial_control: CapturedFinancialControlIdentity,
    lease: Option<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
    fenced: bool,
}
impl PendingReceiverRequest {
    fn require_identity(&self, request_id: DigestV1) -> Result<(), KagemushaStateErrorV1> {
        if self.originals.request_id() != request_id {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        Ok(())
    }
    fn require_unfenced(&self, request_id: DigestV1) -> Result<(), KagemushaStateErrorV1> {
        self.require_identity(request_id)?;
        if self.fenced {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        Ok(())
    }
    fn require_fenced(&self, request_id: DigestV1) -> Result<(), KagemushaStateErrorV1> {
        self.require_identity(request_id)?;
        if !self.fenced {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        Ok(())
    }
}
struct RetainedReceiverRequest {
    captured: CapturedReceiverRequestOriginals,
    financial_control: CapturedFinancialControlIdentity,
    lease: Option<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
}
struct RetainedSendCredit {
    successor: KagemushaStateV1,
    originals: SendCreditOriginals,
    receiver: Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
    receiver_lease: Option<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
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
    control: KagemushaOrdinaryCurrentFinancialControlOwnerV1,
    lineage_cas: KagemushaOrdinaryLineageCasOwnerV1,
    initial_lineage_anchor: KagemushaOrdinaryLineageAnchorV1,
    initial_lineage_anchor_bundle_original: Vec<u8>,
    lineage_originals: [DigestV1; 4],
    public_state_original: Vec<u8>,
    anchor_request_sha256: Option<DigestV1>,
    prepared_commit: Option<PreparedCommitAdmission>,
    state_advance: Option<RetainedFinancialStateAdvance>,
    prepared_incoming_commit: Option<IncomingPreparedCommitAdmission>,
    incoming_reservation_candidate: Option<IncomingReservationCandidateOriginals>,
    outgoing_proof_operands: Option<OutgoingProofOperandOriginals>,
    outgoing_reservation_candidate: Option<OutgoingReservationCandidateOriginals>,
    incoming_commits: BTreeMap<DigestV1, RetainedIncomingCommit>,
    outbox: BTreeMap<DigestV1, RetainedDelivery>,
    verifier: Arc<KagemushaAuthenticatedRecursiveVerifierV1>,
    journal: PrivateJournal,
    prefix: KagemushaRecoveryJournalPrefixV1,
    state: KagemushaStateV1,
    capacity: KagemushaDurableCapacityV1,
    carrier_budget: KagemushaOrdinaryCashCarrierBudgetV1,
    maximum_record_payload_bytes: u64,
    counter_floor: Option<u32>,
    pending: Option<Pending>,
    pending_mint: Option<PendingMint>,
    pending_incoming: Option<PendingIncoming>,
    incoming_consumed: sparse_merkle::ExactConsumedCreditIndex,
    used_operations: BTreeSet<DigestV1>,
    pending_receiver_request: Option<PendingReceiverRequest>,
    retained_receiver_requests: BTreeMap<DigestV1, RetainedReceiverRequest>,
    received_sources: BTreeMap<DigestV1, ReceivedSourceAdmission>,
    financial_journal_revision: u64,
    terminal: Option<terminal::TerminalJournal>,
    recovery_catalog: Option<RecoveryCatalog>,
    recovery_failed: bool,
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
        capacity: KagemushaDurableCapacityV1,
        installed_lineage_policy_original: &[u8],
        recover: bool,
        historical_leases: &[Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>],
        historical_receivers: &[Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>],
    ) -> Result<Self, KagemushaStateErrorV1> {
        publication.recheck()?;
        capacity.validate()?;
        if historical_leases.len() > 1024 || historical_receivers.len() > 1024 {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let release = admitted_release(&verifier)?;
        if !Arc::ptr_eq(&release, publication.cash_approvals().retained_release()) {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let state = publication.initial_state()?.clone();
        let (initial_lineage_anchor, initial_lineage_anchor_bundle_original) =
            publication.lineage_anchor_public_originals(Arc::clone(&verifier), capacity)?;
        let initial_bundle = KagemushaOrdinaryLineageStateProofBundleV1::decode_original(
            &initial_lineage_anchor_bundle_original,
        )
        .map_err(material)?;
        let public_state_original = initial_bundle.state_original().to_vec();
        if initial_lineage_anchor.initial_head.state_commitment != state.state_commitment
            || initial_lineage_anchor.initial_head.logical_sequence != state.logical_sequence
            || initial_lineage_anchor.initial_head.state_original_sha256
                != <DigestV1>::from(Sha256::digest(&public_state_original))
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let lineage_originals = [
            Sha256::digest(installed_lineage_policy_original).into(),
            Sha256::digest(norito::encode_canonical(&initial_lineage_anchor).map_err(material)?)
                .into(),
            Sha256::digest(&initial_lineage_anchor_bundle_original).into(),
            Sha256::digest(&public_state_original).into(),
        ];
        let carrier_budget =
            ordinary_cash_carrier_budget_v1(verifier.as_ref()).map_err(material)?;
        let maximum_record_payload_bytes =
            released_cash_record_payload_limit(verifier.as_ref(), &carrier_budget)?;
        let format = cash_journal_format(maximum_record_payload_bytes);
        if carrier_budget.release_id() != state.release_id
            || u64::from(carrier_budget.required_outbox_slot_bytes()) > maximum_record_payload_bytes
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let counter_floor = publication
            .cash_approvals()
            .retained_app_attest_counter_floor();
        let initial = Record::Initialize {
            originals: publication.original_commitments()?,
            counter_floor,
            capacity,
            lineage_originals,
            maximum_record_payload_bytes,
        };
        let mut journal = if recover {
            PrivateJournal::open_existing(path, format)
        } else {
            PrivateJournal::create_new(path, format)
        }
        .map_err(storage)?;
        if !recover {
            journal
                .append(&encode(&initial, maximum_record_payload_bytes)?)
                .map_err(storage)?;
        }
        if recover {
            let (first_sequence, first) = journal
                .replay_next()
                .map_err(storage)?
                .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
            let first = zeroize::Zeroizing::new(first);
            if first_sequence != 0 || decode(&first, maximum_record_payload_bytes)? != initial {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            // Authenticate the complete physical prefix before semantic proof replay. A cold
            // journal cannot lend recovery_prefix while its physical cursor is incomplete.
            while let Some((sequence, original)) = journal.replay_next().map_err(storage)? {
                let _original = zeroize::Zeroizing::new(original);
                if sequence >= MAX_ROWS {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
            }
        }
        let control = if recover {
            KagemushaOrdinaryCurrentFinancialControlOwnerV1::open_existing(
                path,
                publication.cash_financial(),
            )
        } else {
            KagemushaOrdinaryCurrentFinancialControlOwnerV1::create(
                path,
                publication.cash_financial(),
            )
        }
        .map_err(material)?;
        let lineage_cas = if recover {
            KagemushaOrdinaryLineageCasOwnerV1::open_existing(
                path,
                publication.cash_financial(),
                installed_lineage_policy_original,
            )
        } else {
            KagemushaOrdinaryLineageCasOwnerV1::create(
                path,
                publication.cash_financial(),
                installed_lineage_policy_original,
            )
        }
        .map_err(material)?;
        let prefix = journal.recovery_prefix().map_err(storage)?;
        let this = Self {
            publication,
            control,
            lineage_cas,
            initial_lineage_anchor,
            initial_lineage_anchor_bundle_original,
            lineage_originals,
            public_state_original,
            anchor_request_sha256: None,
            prepared_commit: None,
            state_advance: None,
            prepared_incoming_commit: None,
            incoming_reservation_candidate: None,
            outgoing_proof_operands: None,
            outgoing_reservation_candidate: None,
            incoming_commits: BTreeMap::new(),
            outbox: BTreeMap::new(),
            verifier,
            journal,
            prefix,
            state,
            capacity,
            carrier_budget,
            maximum_record_payload_bytes,
            counter_floor,
            pending: None,
            pending_mint: None,
            pending_incoming: None,
            incoming_consumed: sparse_merkle::ExactConsumedCreditIndex::empty(),
            used_operations: BTreeSet::new(),
            pending_receiver_request: None,
            retained_receiver_requests: BTreeMap::new(),
            received_sources: BTreeMap::new(),
            financial_journal_revision: 0,
            terminal: Some(terminal::TerminalJournal::new()),
            recovery_catalog: recover.then(|| RecoveryCatalog {
                leases: historical_leases.to_vec(),
                receivers: historical_receivers.to_vec(),
            }),
            recovery_failed: false,
        };
        // Semantic replay is deliberately deferred until a newly admitted current FI read
        // has acknowledged the actual private historical captures. No State/effect is exposed
        // by this holder while its recovery catalog remains pending.
        this.recheck_current_storage()?;
        Ok(this)
    }

    /// Reserve the genuine globally serialized zero anchor using this owner's exact retained
    /// public originals and actual installed recursive verifier. Returning request bytes does
    /// not acknowledge a global anchor or permit financial effects.
    /// # Errors
    /// Refuses an existing anchor, unavailable actual current FI/proof or failed request durability.
    pub fn prepare_lineage_anchor(&mut self) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        if self.anchor_request_sha256.is_some()
            || self
                .lineage_cas
                .acknowledged_anchor_request(
                    self.publication.cash_financial(),
                    &self.initial_lineage_anchor,
                )
                .map_err(material)?
                .is_some()
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let financial = self.publication.cash_financial();
        let approval = self
            .publication
            .cash_approvals()
            .historical_bootstrap_approval()?;
        let proof = crate::kagemusha_v1_recursion::verify_ordinary_lineage_anchor_v1(
            self.verifier.as_ref(),
            &self.initial_lineage_anchor,
            &self.initial_lineage_anchor_bundle_original,
            financial.enrollment().app_credential(),
            approval
                .original_approval_integrity_lease()
                .map(|lease| lease.as_ref()),
        )
        .map_err(material)?;
        let current = self.control.loan(financial).map_err(material)?;
        let original = self
            .lineage_cas
            .reserve_anchor(financial, &current, &proof)
            .map_err(material)?;
        self.require_current_financial_control()?;
        Ok(original)
    }

    /// Observe the current authenticated private State while its original custody remains held.
    /// A projection cannot reconstruct this owner or authorize a payment.
    pub fn current_state(&self) -> Result<&KagemushaStateV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        Ok(&self.state)
    }

    fn recheck_current_storage(&self) -> Result<(), KagemushaStateErrorV1> {
        self.capacity.validate()?;
        if self.carrier_budget.release_id() != self.state.release_id {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.publication.recheck()?;
        self.recheck_lineage_retained_custody()?;
        self.journal.check_owned().map_err(storage)?;
        if self.journal.recovery_prefix().map_err(storage)? != self.prefix
            || !Arc::ptr_eq(
                &admitted_release(&self.verifier)?,
                self.publication.cash_approvals().retained_release(),
            )
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.state.validate()?;
        self.recheck_receiver_request_storage()?;
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

    fn recheck_lineage_retained_custody(&self) -> Result<(), KagemushaStateErrorV1> {
        self.publication.recheck_historical_cash_custody()?;
        let (lineage, policy_sha256) = self
            .lineage_cas
            .retained_lineage_originals(self.publication.cash_financial())
            .map_err(material)?;
        let anchor = &self.initial_lineage_anchor;
        let bundle = &self.initial_lineage_anchor_bundle_original;
        let initial_state = self.publication.historical_initial_state()?;
        let initial_bundle = KagemushaOrdinaryLineageStateProofBundleV1::decode_original(bundle)
            .map_err(material)?;
        // Constructor/reopen already admitted the actual immutable publication. Routine
        // custody checks hash its retained originals; they do not repeat recursive proving.
        if lineage != &anchor.lineage
            || policy_sha256 != self.lineage_originals[0]
            || <DigestV1>::from(Sha256::digest(
                norito::encode_canonical(anchor).map_err(material)?,
            )) != self.lineage_originals[1]
            || <DigestV1>::from(Sha256::digest(bundle)) != self.lineage_originals[2]
            || anchor.proof_bundle_original_sha256 != self.lineage_originals[2]
            || <DigestV1>::from(Sha256::digest(initial_bundle.state_original()))
                != self.lineage_originals[3]
            || anchor.initial_head.state_original_sha256 != self.lineage_originals[3]
            || anchor.initial_head.state_commitment != initial_state.state_commitment
            || anchor.initial_head.logical_sequence != initial_state.logical_sequence
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        KagemushaOrdinaryLineageStateOriginalV1::decode_original(&self.public_state_original)
            .map_err(material)?;
        self.recheck_state_advance_historical()?;
        self.recheck_initial_lineage_anchor_historical()
    }

    fn recheck(&self) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_current_storage()?;
        if self.recovery_catalog.is_some() || self.recovery_failed {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        Ok(())
    }

    fn require_current_financial_control(&self) -> Result<(), KagemushaStateErrorV1> {
        self.recheck()?;
        self.require_state_advance_acknowledged()?;
        self.control
            .loan(self.publication.cash_financial())
            .map_err(material)?
            .recheck()
            .map_err(material)
    }

    fn recheck_proving_history(
        &self,
        operation: ProvingHistoryOperation,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.recovery_catalog.is_some() || self.recovery_failed {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.publication.recheck_historical_cash_custody()?;
        self.recheck_lineage_retained_custody()?;
        self.journal.check_owned().map_err(storage)?;
        if self.journal.recovery_prefix().map_err(storage)? != self.prefix
            || !Arc::ptr_eq(
                &admitted_release(&self.verifier)?,
                self.publication.cash_approvals().retained_release(),
            )
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.state.validate()?;
        self.recheck_receiver_request_storage()?;
        if let Some(terminal) = &self.terminal {
            terminal.recheck()?;
        }
        // Each proof loan uses only its own retained approval's FI identity. The
        // original incoming source FI and an unrelated outgoing slot cannot replace it.
        let identity = operation.select_financial_control_identity(
            self.pending.as_ref().map(|p| p.financial_control),
            self.terminal
                .as_ref()
                .and_then(terminal::TerminalJournal::proving_financial_control_identity),
            self.pending_incoming
                .as_ref()
                .and_then(|p| p.approval.as_ref())
                .map(incoming_preparation::PendingIncomingApproval::proving_financial_control_identity),
            self.pending_incoming
                .as_ref()
                .and_then(|p| p.terminal.as_ref())
                .map(incoming_terminal::IncomingTerminalPending::proving_financial_control_identity),
        )?;
        self.control
            .borrow_captured_proof_decision(
                self.publication.cash_financial(),
                identity.original_sha256,
                identity.lower_ms,
                identity.upper_ms,
            )
            .map_err(material)?
            .recheck_historical_originals()
            .map_err(material)
    }

    /// Reserve the sole actual current FI read under this cash holder and installed clock.
    /// Only canonical public request/signing fields are returned; they create no live grant.
    /// # Errors
    /// Refuses changed original storage, unavailable genuine clock or failed Native durability.
    pub fn prepare_current_financial_control_read(
        &mut self,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.recheck_current_storage()?;
        if self.recovery_failed {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.control
            .prepare_current_read(self.publication.cash_financial())
            .map_err(material)
    }

    /// Sign through the real installed Native account/session caller after its invocation fence.
    /// The callback borrows the actual same control/financial holders; managed code cannot
    /// supply a financial owner, signing subject, account key, status or replacement clock.
    /// # Errors
    /// Refuses subject drift, foreign account/session, unknown invocation or unretained Ed64.
    pub fn sign_current_financial_control_request(
        &mut self,
        sign: impl FnOnce(
            &mut KagemushaOrdinaryCurrentFinancialControlOwnerV1,
            &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1>,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.recheck_current_storage()?;
        if self.recovery_failed {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let financial = self.publication.cash_financial();
        let request = self
            .control
            .pending_account_request(financial)
            .map_err(material)?
            .canonical_bytes()
            .map_err(material)?;
        let fields = sign(&mut self.control, financial)?;
        if fields
            != self
                .control
                .retained_current_read_fields(financial)
                .map_err(material)?
            || fields.first() != Some(&request)
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.recheck_current_storage()?;
        Ok(fields)
    }

    /// Authenticate exact issuer-signed control and complete independently certified World.
    /// This is the descriptor-bound original intake; no managed decoding supplies authority.
    /// # Errors
    /// Refuses substituted/stale originals, wrong actual owner/cut, or failed original durability.
    pub fn accept_current_financial_control_read(
        &mut self,
        signed_original: &[u8],
        authority_original: &[u8],
    ) -> Result<(), KagemushaStateErrorV1> {
        self.recheck_current_storage()?;
        if self.recovery_failed
            || signed_original.len() > 64 * 1024
            || authority_original.len() > 128 * 1024 * 1024
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.control
            .accept_current_read(
                self.publication.cash_financial(),
                signed_original,
                authority_original,
            )
            .map_err(material)?;
        self.control
            .resume_all_retained_proof_decisions(self.publication.cash_financial())
            .map_err(material)?;
        if let Some(catalog) = self.recovery_catalog.take() {
            // Partial semantic replay never exposes State or retries from an advanced cursor.
            // Failure freezes this holder; reopening must independently verify all originals.
            if let Err(error) = self.replay_financial_history(&catalog) {
                self.recovery_failed = true;
                return Err(error);
            }
        }
        // FI intake may recover an unacknowledged durable StateAdvance. Money remains
        // unavailable until its distinct actual post-fsync acknowledgment is completed.
        self.recheck()?;
        self.control
            .loan(self.publication.cash_financial())
            .map_err(material)?
            .recheck()
            .map_err(material)
    }

    /// Lend actual custody for PI refresh even when the old PI has expired. This checks
    /// static originals and the Native clock, never current FI or monetary authority.
    /// # Errors
    /// Refuses changed original publication, owned storage, state scope or refresh custody.
    pub fn with_integrity_refresh_custody<T>(
        &mut self,
        consume: impl FnOnce(
            &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        ) -> Result<T, KagemushaStateErrorV1>,
    ) -> Result<T, KagemushaStateErrorV1> {
        self.recheck_integrity_refresh_storage()?;
        let result = consume(self.publication.cash_financial())?;
        self.recheck_integrity_refresh_storage()?;
        Ok(result)
    }

    fn recheck_integrity_refresh_storage(&self) -> Result<(), KagemushaStateErrorV1> {
        if self.recovery_failed {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.publication.recheck_historical_cash_custody()?;
        self.recheck_lineage_retained_custody()?;
        self.journal.check_owned().map_err(storage)?;
        if self.journal.recovery_prefix().map_err(storage)? != self.prefix
            || !Arc::ptr_eq(
                &admitted_release(&self.verifier)?,
                self.publication.cash_approvals().retained_release(),
            )
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.state.validate()?;
        self.publication
            .cash_financial()
            .recheck_integrity_refresh_custody()
            .map(|_| ())
            .map_err(material)
    }

    /// Durably retain an independently verified current PI original through the actual
    /// publication and its same logical/financial holders. Existing captured attempts keep
    /// their immutable original lease; this refresh creates no FI control or cash grant.
    /// # Errors
    /// Refuses wrong credential/release, stale PI, changed original storage, failed durability
    /// or a recovery catalog that cannot retain the exact original for semantic replay.
    pub fn accept_integrity_lease(
        &mut self,
        lease: Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.recovery_failed {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.journal.check_owned().map_err(storage)?;
        if self.journal.recovery_prefix().map_err(storage)? != self.prefix {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let needs_catalog_entry = self.recovery_catalog.as_ref().is_some_and(|catalog| {
            !catalog
                .leases
                .iter()
                .any(|held| held.original() == lease.original())
        });
        if needs_catalog_entry
            && self
                .recovery_catalog
                .as_ref()
                .is_some_and(|catalog| catalog.leases.len() >= 1024)
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.publication
            .accept_integrity_lease(Arc::clone(&lease))?;
        if needs_catalog_entry {
            self.recovery_catalog
                .as_mut()
                .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                .leases
                .push(lease);
        }
        self.recheck_current_storage()
    }

    fn replay_financial_history(
        &mut self,
        catalog: &RecoveryCatalog,
    ) -> Result<(), KagemushaStateErrorV1> {
        let mut cursor = self.journal.replay_cursor().map_err(storage)?;
        let (sequence, first) = self
            .journal
            .read_cursor_next(&mut cursor)
            .map_err(storage)?
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let expected = Record::Initialize {
            originals: self.publication.historical_original_commitments()?,
            counter_floor: self.counter_floor,
            capacity: self.capacity,
            lineage_originals: self.lineage_originals,
            maximum_record_payload_bytes: self.maximum_record_payload_bytes,
        };
        let first = zeroize::Zeroizing::new(first);
        if sequence != 0 || decode(&first, self.maximum_record_payload_bytes)? != expected {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        loop {
            let preceding = cursor.consumed_prefix();
            let Some((sequence, original)) = self
                .journal
                .read_cursor_next(&mut cursor)
                .map_err(storage)?
            else {
                break;
            };
            if sequence >= MAX_ROWS {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            let original = zeroize::Zeroizing::new(original);
            self.replay(
                decode(&original, self.maximum_record_payload_bytes)?,
                &catalog.leases,
                &catalog.receivers,
                preceding,
            )?;
        }
        self.journal.check_owned().map_err(storage)?;
        if self.journal.recovery_prefix().map_err(storage)? != self.prefix {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(())
    }

    fn persist(&mut self, record: &Record) -> Result<(), KagemushaStateErrorV1> {
        self.recheck()?;
        if self.prefix.sequence >= MAX_ROWS {
            return Err(KagemushaStateErrorV1::JournalRevisionOverflow);
        }
        // Commit in-memory chronology before a caller's post-fsync freshness recheck.
        // Any uncertain append/cursor freezes this holder; it cannot dispatch another attempt.
        let original = encode(record, self.maximum_record_payload_bytes)?;
        if self.journal.append(&original).is_err() {
            self.recovery_failed = true;
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        match self.journal.recovery_prefix() {
            Ok(prefix) => {
                self.prefix = prefix;
                Ok(())
            }
            Err(_) => {
                self.recovery_failed = true;
                Err(KagemushaStateErrorV1::SnapshotIntegrity)
            }
        }
    }

    /// Reserve one Native-owned X25519 key and exact receiver request in the single cash WAL.
    /// Amount is user intent. This operation creates no credit, ReceiveFold or funding grant.
    /// # Errors
    /// Refuses conflicting app-key attempts, unavailable current FI/clock, insufficient capacity,
    /// changed custody or a failed durable append. An exact pending amount retry reuses its ID/key.
    pub fn reserve_receiver_request(
        &mut self,
        amount: u128,
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        if let Some(pending) = &self.pending_receiver_request {
            if pending.originals.amount() != amount {
                return Err(KagemushaStateErrorV1::InvalidCandidateStage);
            }
            pending
                .originals
                .recheck_live_source(self, pending.lease.as_deref())?;
            return Ok(pending.originals.request_id());
        }
        if self.pending.is_some()
            || self.pending_mint.is_some()
            || self.pending_incoming.is_some()
            || self.terminal.as_ref().is_none_or(|t| t.has_pending())
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        let captured = self
            .control
            .capture_proof_decision(self.publication.cash_financial())
            .map_err(material)?;
        let financial_control = CapturedFinancialControlIdentity {
            original_sha256: captured.original_sha256().map_err(material)?,
            lower_ms: captured.captured_lower_ms(),
            upper_ms: captured.captured_upper_ms(),
        };
        let originals = ReceiverRequestOriginals::create(self, amount)?;
        let id = originals.request_id();
        if id == [0; 32] || self.used_operations.contains(&id) {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.require_receiver_request_capacity(&originals)?;
        self.require_receiver_request_control(financial_control, &originals)?;
        let lease = self
            .publication
            .cash_financial()
            .retained_integrity_lease()
            .cloned();
        self.append_receiver_frame(&Record::ReceiverReserve {
            originals: originals.clone(),
            financial_control,
        })?;
        self.used_operations.insert(id);
        self.pending_receiver_request = Some(PendingReceiverRequest {
            originals,
            financial_control,
            lease,
            fenced: false,
        });
        // Memory follows actual durable chronology before a post-fsync clock failure can return.
        self.require_current_financial_control()?;
        Ok(id)
    }

    /// Read only the exact model signing message from the durable, unfenced Native reservation.
    /// No caller key/message/clock or private encryption key is accepted or returned.
    /// # Errors
    /// Refuses another request, a retained invocation fence, expired request or changed custody.
    pub fn receiver_request_signing_message(
        &self,
        request_id: DigestV1,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let pending = self
            .pending_receiver_request
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        pending.require_unfenced(request_id)?;
        self.require_receiver_request_control(pending.financial_control, &pending.originals)?;
        pending
            .originals
            .signing_message(self, pending.lease.as_deref())
    }

    /// Project the complete same original C for the hardware app-key signer selection.
    /// It is data only; managed code cannot select another alias or construct request custody.
    /// # Errors
    /// Uses the same current, exact unfenced reservation checks as the signing-message borrow.
    pub fn receiver_request_credential_original(
        &self,
        request_id: DigestV1,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.receiver_request_signing_message(request_id)?;
        Ok(self
            .pending_receiver_request
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .originals
            .credential_original()
            .to_vec())
    }

    /// Fsync the irreversible single OS invocation fence before handing off to the app signer.
    /// Recovery retains uncertainty; neither a retry nor cancellation can dispatch a second signature.
    /// # Errors
    /// Refuses a different/fenced/expired request, changed original custody or failed durability.
    pub fn fence_receiver_request_platform(
        &mut self,
        request_id: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.receiver_request_signing_message(request_id)?;
        self.append_receiver_frame(&Record::ReceiverPlatformFence { request_id })?;
        self.pending_receiver_request
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .fenced = true;
        self.require_current_financial_control()
    }

    /// Authenticate and fsync the full OS-signed request, then advance the global Apple floor.
    /// Capture exposes public Request bytes only. The one-use X25519 key remains private Native WAL data.
    /// # Errors
    /// Refuses missing fence, changed exact body, wrong platform/key/counter, expiration or conflict.
    /// After durable capture an exact byte retry returns the original without appending/signing again.
    pub fn capture_receiver_request_original(
        &mut self,
        request_id: DigestV1,
        original: &[u8],
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        if let Some(retained) = self.retained_receiver_requests.get(&request_id) {
            if retained.captured.original() != original {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            return self.captured_receiver_request_original(request_id);
        }
        let pending = self
            .pending_receiver_request
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        pending.require_fenced(request_id)?;
        self.require_receiver_request_control(pending.financial_control, &pending.originals)?;
        let captured =
            pending
                .originals
                .clone()
                .capture_original(self, pending.lease.as_deref(), original)?;
        self.append_receiver_frame(&Record::ReceiverCapture(captured.clone()))?;
        self.counter_floor = captured.accepted_counter().or(self.counter_floor);
        let pending = self
            .pending_receiver_request
            .take()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        self.retained_receiver_requests.insert(
            request_id,
            RetainedReceiverRequest {
                captured,
                financial_control: pending.financial_control,
                lease: pending.lease,
            },
        );
        // Even a slow post-fsync freshness failure preserves Capture and the accepted counter.
        self.captured_receiver_request_original(request_id)
    }

    /// Expose only a previously fsynced exact public request while current FI custody is held.
    /// Historical signature/counter originals are checked at their retained admission bounds;
    /// this neither renews the request nor lends decryption, ReceiveFold or monetary authority.
    /// # Errors
    /// Refuses absent capture, changed full enrollment/control originals or current FI failure.
    pub fn captured_receiver_request_original(
        &self,
        request_id: DigestV1,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let retained = self
            .retained_receiver_requests
            .get(&request_id)
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        self.require_receiver_request_control(
            retained.financial_control,
            retained.captured.reservation(),
        )?;
        retained
            .captured
            .recheck_historical_sources(self, retained.lease.as_deref())?;
        self.require_current_financial_control()?;
        Ok(retained.captured.original().to_vec())
    }

    /// Cancel only an unfenced request. Its identity remains in the global never-reuse set.
    /// No captured/uncertain key can be consumed or recycled through this operation.
    /// # Errors
    /// Refuses a different request, any OS fence, current FI failure or failed durable append.
    pub fn cancel_receiver_request(
        &mut self,
        request_id: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let pending = self
            .pending_receiver_request
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        pending.require_unfenced(request_id)?;
        self.append_receiver_frame(&Record::ReceiverCancel { request_id })?;
        self.pending_receiver_request = None;
        self.require_current_financial_control()
    }

    // Only Receiver methods may call this. Commit memory immediately after this actual fsync,
    // then check live freshness. Partial append/cursor uncertainty freezes the holder for reopen.
    fn append_receiver_frame(&mut self, record: &Record) -> Result<(), KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        if self.prefix.sequence >= MAX_ROWS {
            return Err(KagemushaStateErrorV1::JournalRevisionOverflow);
        }
        let bytes = encode(record, self.maximum_record_payload_bytes)?;
        if self.journal.append(&bytes).is_err() {
            self.recovery_failed = true;
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        match self.journal.recovery_prefix() {
            Ok(prefix) => {
                self.prefix = prefix;
                Ok(())
            }
            Err(_) => {
                self.recovery_failed = true;
                Err(KagemushaStateErrorV1::SnapshotIntegrity)
            }
        }
    }

    pub(crate) fn with_retained_predecessor_checkpoint(
        &self,
        consume: &mut dyn for<'a> FnMut(
            &'a crate::kagemusha_v1_recursion::KagemushaGeneratedRecursiveStateProofV1,
        ) -> Result<(), KagemushaStateErrorV1>,
    ) -> core::result::Result<(), KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let prefix = self.prefix;
        match &self.state_advance {
            Some(value) => value.with_successor_checkpoint(
                &self.verifier,
                &self.state,
                &self.public_state_original,
                consume,
            )?,
            None => {
                if self.publication.initial_state()? != &self.state {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                self.publication.with_retained_initial_state_checkpoint(
                    Arc::clone(&self.verifier),
                    self.capacity,
                    consume,
                )?;
            }
        }
        if self.prefix != prefix {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.require_current_financial_control()
    }

    fn require_receiver_request_control(
        &self,
        identity: CapturedFinancialControlIdentity,
        originals: &ReceiverRequestOriginals,
    ) -> Result<(), KagemushaStateErrorV1> {
        self.control
            .recheck_retained_capture_identity(
                self.publication.cash_financial(),
                identity.original_sha256,
                identity.lower_ms,
                identity.upper_ms,
            )
            .map_err(material)?;
        let clock = originals.clock();
        if clock.lower_at_ms < identity.lower_ms || clock.upper_at_ms < identity.upper_ms {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        Ok(())
    }

    fn require_receiver_request_capacity(
        &self,
        next: &ReceiverRequestOriginals,
    ) -> Result<(), KagemushaStateErrorV1> {
        let mut used = self
            .retained_received_source_capacity_charge()?
            .checked_add(next.capacity_charge_bytes()?)
            .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)?;
        for retained in self.retained_receiver_requests.values() {
            used = used
                .checked_add(retained.captured.reservation().capacity_charge_bytes()?)
                .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)?;
        }
        if used > self.capacity.inbox_bytes
            || self.retained_receiver_requests.len() >= MAX_ROWS as usize
        {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        Ok(())
    }

    fn recheck_receiver_request_storage(&self) -> Result<(), KagemushaStateErrorV1> {
        let mut bytes = self.retained_received_source_capacity_charge()?;
        if let Some(mint) = &self.pending_mint {
            bytes = bytes
                .checked_add(mint.capacity_charge_bytes()?)
                .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)?;
        }
        if let Some(pending) = &self.pending_receiver_request {
            if !self
                .used_operations
                .contains(&pending.originals.request_id())
                || self.pending.is_some()
                || self.pending_mint.is_some()
                || self.pending_incoming.is_some()
                || self.terminal.as_ref().is_none_or(|t| t.has_pending())
                || pending.originals.original_counter_floor() != self.counter_floor
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            bytes = bytes
                .checked_add(pending.originals.capacity_charge_bytes()?)
                .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)?;
        }
        for (id, retained) in &self.retained_receiver_requests {
            if *id != retained.captured.reservation().request_id()
                || !self.used_operations.contains(id)
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            bytes = bytes
                .checked_add(retained.captured.reservation().capacity_charge_bytes()?)
                .ok_or(KagemushaStateErrorV1::InvalidDurableCapacity)?;
        }
        if bytes > self.capacity.inbox_bytes
            || self.retained_receiver_requests.len() > MAX_ROWS as usize
        {
            return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
        }
        Ok(())
    }

    fn resolve_receiver_request_lease(
        &self,
        originals: &ReceiverRequestOriginals,
        leases: &[Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>],
    ) -> Result<Option<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>, KagemushaStateErrorV1>
    {
        match originals.lease_original() {
            None => Ok(None),
            Some(raw) => Ok(Some(Arc::clone(
                leases
                    .iter()
                    .find(|held| held.original() == raw)
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
            ))),
        }
    }

    /// Reserve actual Native entropy and exact held predecessor before deriving purpose2 S/W.
    /// This internal operation grants only an approval attempt, not funds or an outbox slot.
    pub(crate) fn reserve_preparation(
        &mut self,
        operation_kind: KagemushaOperationKindV1,
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        self.require_initial_lineage_anchor_current()?;
        self.require_outgoing_rows(15)?;
        self.require_outbox_capacity_for_new_slot()?;
        if self.pending.is_some()
            || self.pending_mint.is_some()
            || self.pending_incoming.is_some()
            || self.pending_receiver_request.is_some()
            || !matches!(
                operation_kind,
                KagemushaOperationKindV1::SendSplit | KagemushaOperationKindV1::RedeemSplit
            )
            || u64::from(self.outgoing_completion_slot_bytes()?) > self.capacity.outbox_bytes
        {
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
        let captured = self
            .control
            .capture_proof_decision(self.publication.cash_financial())
            .map_err(material)?;
        let financial_control = CapturedFinancialControlIdentity {
            original_sha256: captured.original_sha256().map_err(material)?,
            lower_ms: captured.captured_lower_ms(),
            upper_ms: captured.captured_upper_ms(),
        };
        let preparation_clock = self
            .publication
            .cash_financial()
            .current_cash_clock_context()
            .map_err(material)?;
        preparation_clock.validate_shape().map_err(material)?;
        if preparation_clock.lower_at_ms < financial_control.lower_ms
            || preparation_clock.upper_at_ms < financial_control.upper_ms
        {
            return Err(KagemushaStateErrorV1::SnapshotRollback);
        }
        let mut reservation_hash = Sha256::new();
        reservation_hash.update(b"iroha:kagemusha:v1:ordinary-native-outbox-slot\0");
        reservation_hash.update(operation);
        reservation_hash.update(self.state.state_commitment);
        reservation_hash.update(preparation_clock.binding_digest().map_err(material)?);
        let reservation = KagemushaOutboxReservationV1 {
            reservation_id: reservation_hash.finalize().into(),
            operation_kind,
            reserved_outbox_bytes: self.outgoing_completion_slot_bytes()?,
            issued_at_ms: preparation_clock.lower_at_ms,
            expires_at_ms: self.credential_floor()?.approval_valid_until_ms(),
        };
        reservation.validate().map_err(material)?;
        preparation_clock
            .validate_within_original_window(reservation.issued_at_ms, reservation.expires_at_ms)
            .map_err(material)?;
        self.require_current_financial_control()?;
        self.persist(&Record::Intent {
            operation,
            nonce,
            predecessor: self.state.state_commitment,
            financial_control,
            preparation_clock,
            reservation,
        })?;
        self.used_operations.insert(operation);
        self.pending = Some(Pending {
            operation,
            nonce,
            financial_control,
            preparation_clock,
            reservation,
            send_credit: None,
            redeem_credit: None,
            selected: None,
            fenced: false,
            retained: None,
            capture: None,
        });
        self.require_current_financial_control()?;
        Ok(operation)
    }

    /// Produce and fsync exactly one credit before W2. Exact retries reuse all retained
    /// entropy and ciphertext; independently authenticated receiver originals remain held.
    pub(crate) fn retain_send_credit(
        &mut self,
        operation: DigestV1,
        successor: KagemushaStateV1,
        request_original: &[u8],
        receiver: Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
        receiver_lease: Option<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
        receiver_counter_floor: Option<u32>,
    ) -> Result<KagemushaOrdinaryPaymentOutputV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if pending.operation != operation
            || pending.reservation.operation_kind != KagemushaOperationKindV1::SendSplit
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        if let Some(retained) = &pending.send_credit {
            if retained.successor != successor
                || retained.originals.request_original() != request_original
                || retained.originals.receiver_counter_floor() != receiver_counter_floor
            {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            retained.originals.recheck_originals(
                self,
                operation,
                &successor,
                &receiver,
                receiver_lease.as_deref(),
            )?;
            return Ok(*retained.originals.output());
        }
        let originals = SendCreditOriginals::create(
            self,
            operation,
            &successor,
            request_original,
            &receiver,
            receiver_lease.as_deref(),
            receiver_counter_floor,
        )?;
        self.persist(&Record::SendCredit {
            successor: successor.clone(),
            originals: originals.clone(),
        })?;
        self.pending
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .send_credit = Some(RetainedSendCredit {
            successor,
            originals,
            receiver,
            receiver_lease,
        });
        self.require_current_financial_control()?;
        let retained = self
            .pending
            .as_ref()
            .and_then(|p| p.send_credit.as_ref())
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        retained.originals.recheck_originals(
            self,
            operation,
            &retained.successor,
            &retained.receiver,
            retained.receiver_lease.as_deref(),
        )?;
        Ok(*retained.originals.output())
    }

    /// Select the enrolled beneficiary and actual release manifest before W2. Exact retries
    /// use the original durable selection; offered beneficiary or manifest bytes are excluded.
    pub(crate) fn retain_redeem_credit(
        &mut self,
        operation: DigestV1,
        amount: u128,
        successor: KagemushaStateV1,
    ) -> Result<
        iroha_data_model::kagemusha::KagemushaOrdinaryRedemptionOutputV1,
        KagemushaStateErrorV1,
    > {
        self.require_current_financial_control()?;
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if pending.operation != operation
            || pending.reservation.operation_kind != KagemushaOperationKindV1::RedeemSplit
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        if let Some((held_successor, originals)) = &pending.redeem_credit {
            if *held_successor != successor || originals.output().amount != amount {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            originals.recheck_original_data(self, operation, &successor)?;
            return Ok(*originals.output());
        }
        let originals = RedeemOriginals::create(self, operation, amount, &successor)?;
        self.persist(&Record::RedeemCredit {
            successor: successor.clone(),
            originals: originals.clone(),
        })?;
        self.pending
            .as_mut()
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .redeem_credit = Some((successor, originals));
        self.require_current_financial_control()?;
        Ok(*self
            .pending
            .as_ref()
            .and_then(|p| p.redeem_credit.as_ref())
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
            .1
            .output())
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
        self.require_current_financial_control()?;
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
        require_reserved_selection(pending, &statement, &successor)?;
        self.require_native_preparation_derivation(operation, &statement, &successor, context)?;
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
            .checked_add(ORDINARY_PREPARATION_LIFETIME_MS)
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
        self.require_current_financial_control()?;
        let pending = self
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if pending.operation != operation
            || pending.fenced
            || pending.retained.is_some()
            || pending.capture.is_some()
        {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.persist(&Record::Cancel { operation })?;
        self.pending = None;
        self.require_current_financial_control()
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
        self.require_current_financial_control()?;
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
        self.recheck_native_preparation_derivation(operation)?;
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
        historical_receivers: &[Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>],
        preceding: Option<KagemushaRecoveryJournalPrefixV1>,
    ) -> Result<(), KagemushaStateErrorV1> {
        match record {
            Record::Mint(record) => self.replay_mint(record, historical_leases)?,
            Record::ReceivedSource(originals) => self.replay_received_source(originals)?,
            Record::IncomingIntent(originals) => self.replay_incoming_intent(originals)?,
            Record::IncomingApproval(record) => {
                self.replay_incoming_approval(record, historical_leases)?
            }
            Record::IncomingTerminal(record) => {
                self.replay_incoming_terminal(record, preceding, historical_leases)?
            }
            Record::IncomingReservationCandidate(originals) => {
                self.replay_incoming_reservation_candidate(originals)?
            }
            Record::IncomingPrepareCommit(originals) => {
                self.replay_incoming_prepared_commit(originals)?
            }
            Record::IncomingStateAdvance(advance) => {
                self.install_actual_incoming_state_advance(advance)?
            }
            Record::IncomingStateAdvanceAcknowledged {
                commit_request_original_sha256,
                acknowledgement,
            } => {
                self.install_incoming_state_advance_ack(
                    commit_request_original_sha256,
                    acknowledgement,
                )?;
            }
            Record::OutgoingProofOperands(originals) => {
                self.replay_outgoing_proof_operands(originals)?
            }
            Record::OutgoingReservationCandidate(originals) => {
                self.replay_outgoing_reservation_candidate(originals)?
            }
            Record::PrepareCommit(originals) => self.replay_prepared_commit(originals)?,
            Record::StateAdvance {
                prepared_original_sha256,
                delivery,
            } => {
                self.install_actual_state_advance(prepared_original_sha256, delivery)?;
            }
            Record::StateAdvanceAcknowledged {
                commit_request_original_sha256,
                acknowledgment,
            } => {
                self.replay_state_advance_acknowledgment(
                    commit_request_original_sha256,
                    acknowledgment,
                )?;
            }
            Record::LineageAnchorAcknowledged {
                request_original_sha256,
            } => {
                self.replay_lineage_anchor_acknowledgment(request_original_sha256)?;
            }
            Record::Intent {
                operation,
                nonce,
                predecessor,
                financial_control,
                preparation_clock,
                reservation,
            } if self.anchor_request_sha256.is_some()
                && self.pending.is_none()
                && self.pending_mint.is_none()
                && self.pending_incoming.is_none()
                && self.pending_receiver_request.is_none()
                && operation != [0; 32]
                && nonce != [0; 32]
                && predecessor == self.state.state_commitment
                && !self.used_operations.contains(&operation) =>
            {
                self.require_state_advance_acknowledged()?;
                self.require_outbox_capacity_for_new_slot()?;
                preparation_clock.validate_shape().map_err(material)?;
                reservation.validate().map_err(material)?;
                preparation_clock
                    .validate_within_original_window(
                        reservation.issued_at_ms,
                        reservation.expires_at_ms,
                    )
                    .map_err(material)?;
                if reservation.reserved_outbox_bytes != self.outgoing_completion_slot_bytes()?
                    || u64::from(reservation.reserved_outbox_bytes) > self.capacity.outbox_bytes
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                self.control
                    .recheck_retained_capture_identity(
                        self.publication.cash_financial(),
                        financial_control.original_sha256,
                        financial_control.lower_ms,
                        financial_control.upper_ms,
                    )
                    .map_err(material)?;
                if preparation_clock.lower_at_ms < financial_control.lower_ms
                    || preparation_clock.upper_at_ms < financial_control.upper_ms
                {
                    return Err(KagemushaStateErrorV1::SnapshotRollback);
                }
                self.used_operations.insert(operation);
                self.pending = Some(Pending {
                    operation,
                    nonce,
                    financial_control,
                    preparation_clock,
                    reservation,
                    send_credit: None,
                    redeem_credit: None,
                    selected: None,
                    fenced: false,
                    retained: None,
                    capture: None,
                });
            }
            Record::SendCredit {
                successor,
                originals,
            } => {
                let pending = self
                    .pending
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                if pending.send_credit.is_some()
                    || pending.selected.is_some()
                    || pending.fenced
                    || pending.reservation.operation_kind != KagemushaOperationKindV1::SendSplit
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                let mut receiver = None;
                for held in historical_receivers {
                    if originals.matches_receiver(held)? {
                        receiver = Some(Arc::clone(held));
                        break;
                    }
                }
                let receiver = receiver.ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                let receiver_lease = match originals.receiver_lease_original() {
                    None => None,
                    Some(raw) => Some(Arc::clone(
                        historical_leases
                            .iter()
                            .find(|held| held.original() == raw)
                            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
                    )),
                };
                originals.recheck_at_replay_position(
                    self,
                    pending.operation,
                    &successor,
                    &receiver,
                    receiver_lease.as_deref(),
                )?;
                self.pending
                    .as_mut()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                    .send_credit = Some(RetainedSendCredit {
                    successor,
                    originals,
                    receiver,
                    receiver_lease,
                });
            }
            Record::RedeemCredit {
                successor,
                originals,
            } => {
                let pending = self
                    .pending
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                if pending.redeem_credit.is_some()
                    || pending.send_credit.is_some()
                    || pending.selected.is_some()
                    || pending.fenced
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                originals.recheck_original_data(self, pending.operation, &successor)?;
                self.pending
                    .as_mut()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?
                    .redeem_credit = Some((successor, originals));
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
                require_reserved_selection(pending, &statement, &successor)?;
                self.require_native_preparation_derivation(
                    pending.operation,
                    &statement,
                    &successor,
                    context,
                )?;
                platform_preparation::require_preparation_challenge_window(&challenge)?;
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
                if self.pending.as_ref().is_some_and(|p| {
                    p.operation == operation
                        && !p.fenced
                        && p.retained.is_none()
                        && p.capture.is_none()
                }) =>
            {
                self.pending = None;
            }
            Record::Terminal(record) => {
                if self.pending_receiver_request.is_some()
                    || self.pending_mint.is_some()
                    || self.pending_incoming.is_some()
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                if let Some(selected_prefix) = record.preselection_prefix() {
                    if Some(selected_prefix) != preceding {
                        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                    }
                }
                let operation = record.new_operation();
                let counter = record.accepted_counter();
                let mut terminal = self
                    .terminal
                    .take()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                let result = terminal.replay(self, record, historical_leases, historical_receivers);
                self.terminal = Some(terminal);
                result?;
                if let Some(operation) = operation {
                    if !self.used_operations.insert(operation) {
                        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                    }
                }
                self.counter_floor = counter.or(self.counter_floor);
            }
            Record::ReceiverReserve {
                originals,
                financial_control,
            } => {
                if self.pending.is_some()
                    || self.pending_mint.is_some()
                    || self.pending_incoming.is_some()
                    || self.pending_receiver_request.is_some()
                    || self.terminal.as_ref().is_none_or(|t| t.has_pending())
                    || self.used_operations.contains(&originals.request_id())
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                self.require_receiver_request_capacity(&originals)?;
                let lease = self.resolve_receiver_request_lease(&originals, historical_leases)?;
                originals.recheck_at_replay_position(self, lease.as_deref())?;
                self.require_receiver_request_control(financial_control, &originals)?;
                self.used_operations.insert(originals.request_id());
                self.pending_receiver_request = Some(PendingReceiverRequest {
                    originals,
                    financial_control,
                    lease,
                    fenced: false,
                });
            }
            Record::ReceiverPlatformFence { request_id } => {
                let pending = self
                    .pending_receiver_request
                    .as_mut()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                pending.require_unfenced(request_id)?;
                pending.fenced = true;
            }
            Record::ReceiverCapture(captured) => {
                let pending = self
                    .pending_receiver_request
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                let request_id = pending.originals.request_id();
                if !pending.fenced
                    || captured.reservation() != &pending.originals
                    || self.retained_receiver_requests.contains_key(&request_id)
                {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                pending
                    .originals
                    .recheck_at_replay_position(self, pending.lease.as_deref())?;
                self.require_receiver_request_control(
                    pending.financial_control,
                    &pending.originals,
                )?;
                captured.recheck_historical_sources(self, pending.lease.as_deref())?;
                self.counter_floor = captured.accepted_counter().or(self.counter_floor);
                let pending = self
                    .pending_receiver_request
                    .take()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                self.retained_receiver_requests.insert(
                    request_id,
                    RetainedReceiverRequest {
                        captured,
                        financial_control: pending.financial_control,
                        lease: pending.lease,
                    },
                );
            }
            Record::ReceiverCancel { request_id } => {
                let pending = self
                    .pending_receiver_request
                    .as_ref()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
                pending.require_unfenced(request_id)?;
                self.pending_receiver_request = None;
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
        let identity = self
            .owner
            .pending
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?
            .financial_control;
        let captured = self
            .owner
            .control
            .borrow_captured_proof_decision(
                financial,
                identity.original_sha256,
                identity.lower_ms,
                identity.upper_ms,
            )
            .map_err(material)?;
        let secret = captured.financial_secret().map_err(material)?;
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
        self.owner
            .recheck_proving_history(ProvingHistoryOperation::OutgoingApproval)?;
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
        self.owner
            .recheck_native_preparation_derivation(pending.operation)?;
        approval.recheck_at_trusted_time(*lower).map_err(material)?;
        approval.recheck_at_trusted_time(*upper).map_err(material)?;
        Ok(())
    }
    pub(crate) fn preparation_clock_context(&self) -> &KagemushaOrdinaryCashClockContextV1 {
        &self
            .owner
            .pending
            .as_ref()
            .expect("retained cash intent")
            .preparation_clock
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
fn require_reserved_selection(
    pending: &Pending,
    statement: &TransitionProofStatementV1,
    successor: &KagemushaStateV1,
) -> Result<(), KagemushaStateErrorV1> {
    let operation = match statement.kind {
        KagemushaTransitionKindV1::SendSplit => KagemushaOperationKindV1::SendSplit,
        KagemushaTransitionKindV1::RedeemSplit => KagemushaOperationKindV1::RedeemSplit,
        _ => return Err(KagemushaStateErrorV1::InvalidCandidateStage),
    };
    if operation != pending.reservation.operation_kind {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    if operation == KagemushaOperationKindV1::SendSplit {
        let retained = pending
            .send_credit
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if retained.successor != *successor || retained.originals.operation() != pending.operation {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        retained.originals.require_statement(statement, successor)?;
    } else {
        let (held_successor, originals) = pending
            .redeem_credit
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        if *held_successor != *successor || originals.operation() != pending.operation {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        originals.require_statement(statement, successor)?;
    }
    Ok(())
}

fn encode(
    record: &Record,
    maximum_payload_bytes: u64,
) -> Result<zeroize::Zeroizing<Vec<u8>>, KagemushaStateErrorV1> {
    let predicted =
        u64::try_from(norito::canonical_frame_len(record).map_err(material)?).map_err(material)?;
    if predicted == 0 || predicted > maximum_payload_bytes {
        return Err(KagemushaStateErrorV1::InvalidDurableCapacity);
    }
    let original = zeroize::Zeroizing::new(norito::encode_canonical(record).map_err(material)?);
    if original.is_empty() || original.len() as u64 > maximum_payload_bytes {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(original)
}
fn decode(original: &[u8], maximum_payload_bytes: u64) -> Result<Record, KagemushaStateErrorV1> {
    if original.is_empty() || original.len() as u64 > maximum_payload_bytes {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let record = norito::decode_canonical_with_limits(
        original,
        norito::canonical_decode_limits(original.len()),
    )
    .map_err(material)?;
    if encode(&record, maximum_payload_bytes)?.as_slice() != original {
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
            KagemushaTransitionKindV1::MintFold => KagemushaOperationKindV1::MintFold,
            KagemushaTransitionKindV1::ReceiveFold => KagemushaOperationKindV1::ReceiveFold,
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

#[cfg(all(test, unix))]
pub(crate) use platform_preparation::{
    OrdinarySendPreviewForQualificationV1, ordinary_send_preview_for_qualification_v1,
};
