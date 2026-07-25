//! Finalized-chain SoraFS moderation orchestration.
//!
//! This module deliberately owns no moderation consensus state. It submits the
//! native moderation ISIs and maintains only a bounded, rebuildable projection
//! of one finalized ledger snapshot plus durable delivery state. The projection
//! may be discarded and reconstructed from the finalized snapshot reader.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

#[cfg(unix)]
use std::os::unix::fs::{
    DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _,
};

use iroha_data_model::{
    account::{AccountId, ParsedAccountId},
    events::data::sorafs::SorafsModerationLedgerEventKind,
    isi::{
        InstructionBox,
        sorafs::{
            AcceptSorafsModerationJurorAssignment, ActivateSorafsModerationCase,
            FinalizeSorafsModerationCase, FinalizeSorafsModerationSortition,
            RaiseSorafsModerationChallenge, RegisterSorafsModerationJurorEligibility,
            ResolveSorafsModerationChallenge, SetSorafsModerationPolicy,
            SubmitSorafsModerationAppeal, SubmitSorafsModerationCommit,
            SubmitSorafsModerationReveal,
        },
    },
    sorafs::{
        moderation::{SoraFsModerationBallotCommitV1, SoraFsModerationBallotRevealV1},
        moderation_ledger::{
            MODERATION_LEDGER_MAX_CANDIDATE_POOL_SIZE_V1, MODERATION_LEDGER_MAX_CHALLENGES_V1,
            MODERATION_LEDGER_MAX_NONCE_BYTES_V1, MODERATION_LEDGER_MAX_PANEL_SIZE_V1,
            MODERATION_LEDGER_MAX_REASON_BYTES_V1, ModerationAppealStatusV1,
            ModerationCaseStatusV1, ModerationChallengeDecisionV1, ModerationSortitionError,
            is_canonical_moderation_identifier_v1, sorafs_moderation_select_panel_v1,
        },
    },
};
use norito::{DecodeLimits, NoritoDeserialize, NoritoSerialize, decode_from_bytes_with_limits};
use rand::{rand_core::TryRngCore as _, rngs::OsRng};
use sorafs_manifest::pop_credentials::{POP_MEMBERSHIP_PROOF_MAX_BYTES_V1, PopMembershipProofV1};
use thiserror::Error;

pub use iroha_data_model::sorafs::moderation_ledger::{
    MODERATION_FINALIZED_SNAPSHOT_VERSION_V1, ModerationFinalizedAppealViewV1,
    ModerationFinalizedCaseViewV1, ModerationFinalizedCursorV1, ModerationFinalizedEventCursorV1,
    ModerationFinalizedEventV1, ModerationFinalizedLedgerSnapshotV1,
};

/// Checkpoint schema version.
pub const MODERATION_ORCHESTRATOR_CHECKPOINT_VERSION_V1: u16 = 1;
/// Hard ceiling for one canonical native moderation instruction.
pub const MODERATION_NATIVE_INSTRUCTION_MAX_BYTES_V1: usize = 2 * 1024 * 1024;
/// Hard ceiling for one persisted orchestrator checkpoint.
pub const MODERATION_ORCHESTRATOR_CHECKPOINT_MAX_BYTES_V1: u64 = 32 * 1024 * 1024;

const ACTION_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.moderation.native-action.v1";
const OPERATION_ID_DOMAIN_V1: &[u8] = b"sorafs.moderation.operation-id.v1";
const REQUEST_BINDING_DOMAIN_V1: &[u8] = b"sorafs.moderation.http-request-binding.v1";
const SNAPSHOT_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.moderation.finalized-snapshot.v1";
const HANDOFF_ID_DOMAIN_V1: &[u8] = b"sorafs.moderation.terminal-handoff.v1";
const POP_PROOF_PAYLOAD_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.moderation.pop-proof-payload.v1";
static CHECKPOINT_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);
#[cfg(any(target_os = "linux", target_os = "android"))]
const SAFE_OPEN_FLAGS: std::os::raw::c_int = 0x0002_0000 | 0x0008_0000;
#[cfg(any(target_os = "macos", target_os = "ios"))]
const SAFE_OPEN_FLAGS: std::os::raw::c_int = 0x0000_0100 | 0x0100_0000;
#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios"
    ))
))]
const SAFE_OPEN_FLAGS: std::os::raw::c_int = 0;
const ACTION_LIMITS: DecodeLimits = DecodeLimits::new(
    256,
    MODERATION_NATIVE_INSTRUCTION_MAX_BYTES_V1,
    4_096,
    2 * MODERATION_NATIVE_INSTRUCTION_MAX_BYTES_V1,
    64,
);

/// One exact native moderation mutation.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub enum ModerationNativeActionV1 {
    /// Activate a policy revision.
    SetPolicy(SetSorafsModerationPolicy),
    /// Admit an appeal.
    SubmitAppeal(SubmitSorafsModerationAppeal),
    /// Register a private-proof eligibility result.
    RegisterEligibility(RegisterSorafsModerationJurorEligibility),
    /// Persist the deterministic panel selection.
    FinalizeSortition(FinalizeSorafsModerationSortition),
    /// Accept one primary assignment.
    AcceptAssignment(AcceptSorafsModerationJurorAssignment),
    /// Apply deterministic failover and activate the ballot.
    ActivateCase(ActivateSorafsModerationCase),
    /// Submit a juror commitment.
    SubmitCommit(SubmitSorafsModerationCommit),
    /// Raise a payload-free challenge.
    RaiseChallenge(RaiseSorafsModerationChallenge),
    /// Resolve a challenge.
    ResolveChallenge(ResolveSorafsModerationChallenge),
    /// Submit a juror reveal.
    SubmitReveal(SubmitSorafsModerationReveal),
    /// Commit the single terminal outcome.
    FinalizeCase(FinalizeSorafsModerationCase),
}

impl ModerationNativeActionV1 {
    /// Convert this action to the already-defined native instruction.
    #[must_use]
    pub fn instruction(&self) -> InstructionBox {
        match self {
            Self::SetPolicy(value) => value.clone().into(),
            Self::SubmitAppeal(value) => value.clone().into(),
            Self::RegisterEligibility(value) => value.clone().into(),
            Self::FinalizeSortition(value) => value.clone().into(),
            Self::AcceptAssignment(value) => value.clone().into(),
            Self::ActivateCase(value) => value.clone().into(),
            Self::SubmitCommit(value) => value.clone().into(),
            Self::RaiseChallenge(value) => value.clone().into(),
            Self::ResolveChallenge(value) => value.clone().into(),
            Self::SubmitReveal(value) => value.clone().into(),
            Self::FinalizeCase(value) => value.clone().into(),
        }
    }

    /// Stable action label used in payload-free operational records.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::SetPolicy(_) => "set_policy",
            Self::SubmitAppeal(_) => "submit_appeal",
            Self::RegisterEligibility(_) => "register_eligibility",
            Self::FinalizeSortition(_) => "finalize_sortition",
            Self::AcceptAssignment(_) => "accept_assignment",
            Self::ActivateCase(_) => "activate_case",
            Self::SubmitCommit(_) => "submit_commit",
            Self::RaiseChallenge(_) => "raise_challenge",
            Self::ResolveChallenge(_) => "resolve_challenge",
            Self::SubmitReveal(_) => "submit_reveal",
            Self::FinalizeCase(_) => "finalize_case",
        }
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>, ModerationOrchestratorError> {
        let bytes = norito::to_bytes(self).map_err(|error| {
            ModerationOrchestratorError::InvalidAction(format!(
                "failed to encode native action: {error}"
            ))
        })?;
        if bytes.len() > MODERATION_NATIVE_INSTRUCTION_MAX_BYTES_V1 {
            return Err(ModerationOrchestratorError::ResourceExhausted {
                resource: "native instruction bytes",
                limit: MODERATION_NATIVE_INSTRUCTION_MAX_BYTES_V1,
            });
        }
        let decoded =
            decode_from_bytes_with_limits::<Self>(&bytes, ACTION_LIMITS).map_err(|error| {
                ModerationOrchestratorError::InvalidAction(format!(
                    "failed to decode canonical native action: {error}"
                ))
            })?;
        let reencoded = norito::to_bytes(&decoded).map_err(|error| {
            ModerationOrchestratorError::InvalidAction(format!(
                "failed to re-encode native action: {error}"
            ))
        })?;
        if reencoded != bytes {
            return Err(ModerationOrchestratorError::InvalidAction(
                "native action is not canonically encoded".to_owned(),
            ));
        }
        Ok(bytes)
    }

    fn validate_authority(&self, authority: &AccountId) -> Result<(), ModerationOrchestratorError> {
        match self {
            Self::SetPolicy(value) => value
                .policy()
                .validate()
                .map_err(|error| ModerationOrchestratorError::InvalidAction(error.to_string())),
            Self::SubmitAppeal(value) => {
                value.intake().validate().map_err(|error| {
                    ModerationOrchestratorError::InvalidAction(error.to_string())
                })?;
                require_exact_authority(authority, &value.intake().appellant, self.label())
            }
            Self::RegisterEligibility(value) => {
                validate_scope(value.case_id(), value.round_id())?;
                if value.membership_proof_payload().is_empty()
                    || value.membership_proof_payload().len()
                        > POP_MEMBERSHIP_PROOF_MAX_BYTES_V1 + 32 * 1024
                {
                    return Err(ModerationOrchestratorError::InvalidAction(
                        "membership proof payload is empty or exceeds the native action bound"
                            .to_owned(),
                    ));
                }
                let _: PopMembershipProofV1 =
                    decode_canonical_payload(value.membership_proof_payload(), "membership proof")?;
                Ok(())
            }
            Self::FinalizeSortition(value) => {
                validate_scope(value.case_id(), value.round_id())?;
                if *value.pop_snapshot_digest() == [0; 32]
                    || *value.randomness_anchor() == [0; 32]
                    || value.proposed_jurors().len()
                        > usize::from(MODERATION_LEDGER_MAX_PANEL_SIZE_V1)
                    || value.proposed_waitlist().len()
                        > usize::from(
                            iroha_data_model::sorafs::moderation_ledger::
                                MODERATION_LEDGER_MAX_WAITLIST_SIZE_V1,
                        )
                    || (value.proposed_jurors().is_empty()
                        && !value.proposed_waitlist().is_empty())
                {
                    return Err(ModerationOrchestratorError::InvalidAction(
                        "sortition proposal violates native bounds".to_owned(),
                    ));
                }
                Ok(())
            }
            Self::AcceptAssignment(value) => {
                validate_scope(value.case_id(), value.round_id())?;
                require_nonzero_digest(*value.sortition_digest(), "sortition_digest")
            }
            Self::ActivateCase(value) => {
                validate_scope(value.case_id(), value.round_id())?;
                require_nonzero_digest(*value.sortition_digest(), "sortition_digest")
            }
            Self::SubmitCommit(value) => {
                let commit = decode_canonical_commit(value.commit_payload())?;
                commit.validate().map_err(|error| {
                    ModerationOrchestratorError::InvalidAction(error.to_string())
                })?;
                if commit.commitment_blake2b_256 == [0; 32] || commit.committed_at_unix_ms != 0 {
                    return Err(ModerationOrchestratorError::InvalidAction(
                        "commit digest must be non-zero and caller timestamp must be zero"
                            .to_owned(),
                    ));
                }
                let juror = canonical_account(&commit.juror_id, "commit juror_id")?;
                require_exact_authority(authority, &juror, self.label())
            }
            Self::RaiseChallenge(value) => {
                validate_scope(value.case_id(), value.round_id())?;
                if !is_canonical_moderation_identifier_v1(value.challenge_id())
                    || *value.evidence_digest() == [0; 32]
                    || value.reason().trim().is_empty()
                    || value.reason() != value.reason().trim()
                    || value.reason().len() > MODERATION_LEDGER_MAX_REASON_BYTES_V1
                    || value.reason().chars().any(char::is_control)
                    || (value.kind().requires_target_juror() && value.target_juror().is_none())
                {
                    return Err(ModerationOrchestratorError::InvalidAction(
                        "challenge identity or evidence digest is invalid".to_owned(),
                    ));
                }
                Ok(())
            }
            Self::ResolveChallenge(value) => {
                validate_scope(value.case_id(), value.round_id())?;
                if !is_canonical_moderation_identifier_v1(value.challenge_id())
                    || *value.decision() == ModerationChallengeDecisionV1::Expired
                {
                    return Err(ModerationOrchestratorError::InvalidAction(
                        "challenge identifier is invalid".to_owned(),
                    ));
                }
                Ok(())
            }
            Self::SubmitReveal(value) => {
                let reveal = decode_canonical_reveal(value.reveal_payload())?;
                reveal.validate().map_err(|error| {
                    ModerationOrchestratorError::InvalidAction(error.to_string())
                })?;
                if reveal.nonce.len() > MODERATION_LEDGER_MAX_NONCE_BYTES_V1
                    || reveal.revealed_at_unix_ms != 0
                {
                    return Err(ModerationOrchestratorError::InvalidAction(
                        "reveal nonce exceeds the native bound or caller timestamp is non-zero"
                            .to_owned(),
                    ));
                }
                let juror = canonical_account(&reveal.juror_id, "reveal juror_id")?;
                require_exact_authority(authority, &juror, self.label())
            }
            Self::FinalizeCase(value) => validate_scope(value.case_id(), value.round_id()),
        }
    }

    fn semantic_material(
        &self,
        authority: &AccountId,
    ) -> Result<Vec<u8>, ModerationOrchestratorError> {
        let mut material = Vec::new();
        push_part(&mut material, self.label().as_bytes())?;
        match self {
            Self::SetPolicy(value) => {
                push_part(&mut material, &value.policy().revision.to_le_bytes())?
            }
            Self::SubmitAppeal(value) => {
                push_part(&mut material, value.intake().case_id.as_bytes())?;
                push_part(&mut material, value.intake().round_id.as_bytes())?;
            }
            Self::RegisterEligibility(value) => {
                push_scope(&mut material, value.case_id(), value.round_id())?;
                push_part(&mut material, authority.to_string().as_bytes())?;
            }
            Self::FinalizeSortition(value) => {
                push_scope(&mut material, value.case_id(), value.round_id())?;
            }
            Self::AcceptAssignment(value) => {
                push_scope(&mut material, value.case_id(), value.round_id())?;
                push_part(&mut material, authority.to_string().as_bytes())?;
            }
            Self::ActivateCase(value) => {
                push_scope(&mut material, value.case_id(), value.round_id())?;
            }
            Self::SubmitCommit(value) => {
                let commit = decode_canonical_commit(value.commit_payload())?;
                push_scope(&mut material, &commit.context.case_id, &commit.round_id)?;
                push_part(&mut material, authority.to_string().as_bytes())?;
            }
            Self::RaiseChallenge(value) => {
                push_scope(&mut material, value.case_id(), value.round_id())?;
                push_part(&mut material, value.challenge_id().as_bytes())?;
            }
            Self::ResolveChallenge(value) => {
                push_scope(&mut material, value.case_id(), value.round_id())?;
                push_part(&mut material, value.challenge_id().as_bytes())?;
            }
            Self::SubmitReveal(value) => {
                let reveal = decode_canonical_reveal(value.reveal_payload())?;
                push_scope(&mut material, &reveal.context.case_id, &reveal.round_id)?;
                push_part(&mut material, authority.to_string().as_bytes())?;
            }
            Self::FinalizeCase(value) => {
                push_scope(&mut material, value.case_id(), value.round_id())?;
            }
        }
        Ok(material)
    }

    fn action_digest(&self) -> Result<[u8; 32], ModerationOrchestratorError> {
        let bytes = self.canonical_bytes()?;
        Ok(domain_hash(ACTION_DIGEST_DOMAIN_V1, &[bytes.as_slice()]))
    }

    fn operation_id(&self, authority: &AccountId) -> Result<[u8; 32], ModerationOrchestratorError> {
        let material = self.semantic_material(authority)?;
        Ok(domain_hash(OPERATION_ID_DOMAIN_V1, &[material.as_slice()]))
    }
}

/// Bounds and durable path for one moderation orchestrator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationOrchestratorConfigV1 {
    /// Absolute private checkpoint path.
    pub checkpoint_path: PathBuf,
    /// Maximum appeals and activated cases retained in the projection.
    pub max_cases: usize,
    /// Maximum finalized events retained in one projection.
    pub max_events: usize,
    /// Maximum pending native transactions.
    pub max_outbox_entries: usize,
    /// Maximum durable operation tombstones.
    pub max_idempotency_records: usize,
    /// Maximum terminal handoff records.
    pub max_handoffs: usize,
    /// Maximum safe submission attempts under the same operation identity.
    pub max_submit_attempts: u32,
    /// Maximum checkpoint bytes.
    pub checkpoint_max_bytes: u64,
}

impl ModerationOrchestratorConfigV1 {
    fn validate(&self) -> Result<(), ModerationOrchestratorError> {
        if !self.checkpoint_path.is_absolute() || self.checkpoint_path.file_name().is_none() {
            return Err(ModerationOrchestratorError::InvalidConfiguration(
                "moderation checkpoint path must be an absolute file path".to_owned(),
            ));
        }
        if self
            .checkpoint_path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err(ModerationOrchestratorError::InvalidConfiguration(
                "moderation checkpoint path must not contain dot components".to_owned(),
            ));
        }
        for (name, value) in [
            ("max_cases", self.max_cases),
            ("max_events", self.max_events),
            ("max_outbox_entries", self.max_outbox_entries),
            ("max_idempotency_records", self.max_idempotency_records),
            ("max_handoffs", self.max_handoffs),
        ] {
            if value == 0 {
                return Err(ModerationOrchestratorError::InvalidConfiguration(format!(
                    "{name} must be non-zero"
                )));
            }
        }
        if self.max_submit_attempts == 0
            || self.checkpoint_max_bytes == 0
            || self.checkpoint_max_bytes > MODERATION_ORCHESTRATOR_CHECKPOINT_MAX_BYTES_V1
        {
            return Err(ModerationOrchestratorError::InvalidConfiguration(
                "submission attempts or checkpoint byte bound is invalid".to_owned(),
            ));
        }
        Ok(())
    }
}

/// Canonical request forwarded to the runtime-only HSM transaction service.
#[derive(Debug, Clone)]
pub struct ModerationTransactionRequestV1 {
    /// Stable, replica-independent semantic operation identity.
    pub operation_id: [u8; 32],
    /// Exact authenticated native transaction authority.
    pub authority: AccountId,
    /// Existing native action.
    pub action: ModerationNativeActionV1,
    /// Canonical Norito bytes of `action`.
    pub canonical_action: Vec<u8>,
    /// Digest of the canonical action.
    pub action_digest: [u8; 32],
    /// Digest binding authenticated method, path, body, authority, and action.
    pub request_binding_digest: [u8; 32],
    /// Finalized height reconciled before this submission was created.
    pub baseline_finalized_height: u64,
}

/// A transaction identity returned by the injected submitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModerationTransactionReceiptV1 {
    /// Native signed transaction hash.
    pub transaction_id: [u8; 32],
    /// Finalized height observed by the submitter while admitting the request.
    pub observed_finalized_height: u64,
}

/// Fixed failure classes; arbitrary HSM/provider diagnostics are never persisted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationSubmissionFailureV1 {
    /// The submitter did not submit and may be retried safely.
    NotSubmittedUnavailable,
    /// The submitter did not submit because its bounded queue is full.
    NotSubmittedBackpressure,
    /// Submission may have succeeded; lookup/reconciliation is mandatory.
    Ambiguous,
    /// The action was rejected before submission and must not be retried.
    PermanentRejection,
    /// Runtime signing or policy is unavailable.
    RuntimeUnavailable,
}

/// Transaction state resolved by stable operation identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationSubmissionLookupV1 {
    /// No matching submission exists as of the supplied finalized height.
    NotFound {
        /// Finalized height at which absence was established.
        observed_finalized_height: u64,
    },
    /// A transaction is pending.
    Pending {
        /// Native transaction hash.
        transaction_id: [u8; 32],
    },
    /// A transaction was applied but is not necessarily in the supplied finalized snapshot yet.
    Applied {
        /// Native transaction hash.
        transaction_id: [u8; 32],
    },
    /// A transaction was terminally rejected as of a finalized height.
    Rejected {
        /// Native transaction hash, when admission assigned one.
        transaction_id: Option<[u8; 32]>,
        /// Height at which rejection/absence is stable.
        observed_finalized_height: u64,
    },
    /// Backend state is inconclusive; retrying would be unsafe.
    Unknown,
}

/// Runtime-only HSM and transaction submission interface.
pub trait ModerationTransactionSubmitterV1: Send + Sync {
    /// Sign and submit exactly one native action under its authenticated authority.
    ///
    /// Implementations must deduplicate `operation_id` across replicas.
    fn submit(
        &self,
        request: &ModerationTransactionRequestV1,
    ) -> Result<ModerationTransactionReceiptV1, ModerationSubmissionFailureV1>;

    /// Resolve an earlier submission by its stable operation identity.
    fn lookup(
        &self,
        operation_id: [u8; 32],
        transaction_id: Option<[u8; 32]>,
    ) -> ModerationSubmissionLookupV1;
}

/// Reader for one complete, internally consistent finalized moderation view.
pub trait ModerationFinalizedSnapshotReaderV1: Send + Sync {
    /// Read every retained lifecycle record and a bounded event window from one finalized anchor.
    fn read_finalized_snapshot(
        &self,
        max_cases: usize,
        max_events: usize,
    ) -> Result<ModerationFinalizedLedgerSnapshotV1, ModerationSnapshotReadErrorV1>;
}

/// Fixed snapshot-reader failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationSnapshotReadErrorV1 {
    /// The finalized reader is temporarily unavailable.
    Unavailable,
    /// The finalized snapshot was too large for configured bounds.
    ResourceExhausted,
    /// The source returned inconsistent or corrupt state.
    InvalidSnapshot,
}

/// Terminal handoff destination.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
pub enum ModerationTerminalHandoffKindV1 {
    /// Appeal-finance settlement.
    Settlement,
    /// Downstream governance/transparency publication.
    Publication,
}

/// Payload-free terminal handoff derived only from finalized ledger state.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationTerminalHandoffV1 {
    /// Stable sink-specific handoff identity.
    pub handoff_id: [u8; 32],
    /// Destination.
    pub kind: ModerationTerminalHandoffKindV1,
    /// Case identifier.
    pub case_id: String,
    /// Round identifier.
    pub round_id: String,
    /// Canonical digest of the authoritative terminal outcome.
    pub outcome_digest: [u8; 32],
    /// Finalized anchor proving the outcome.
    pub finalized_cursor: ModerationFinalizedCursorV1,
}

/// Exactly-once terminal settlement/publication adapter.
pub trait ModerationTerminalHandoffSinkV1: Send + Sync {
    /// Deliver a payload-free finalized handoff, deduplicating `handoff_id`.
    fn deliver(
        &self,
        handoff: &ModerationTerminalHandoffV1,
    ) -> Result<(), ModerationHandoffFailureV1>;
}

/// Fixed handoff failures safe for durable recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationHandoffFailureV1 {
    /// No delivery occurred and retry is safe.
    NotDelivered,
    /// Delivery may have occurred; identical-id retry is required.
    Ambiguous,
    /// The sink permanently rejected the handoff.
    Permanent,
}

/// Runtime-only dependencies.
#[derive(Clone)]
pub struct ModerationOrchestratorDepsV1 {
    /// HSM transaction submitter.
    pub submitter: Arc<dyn ModerationTransactionSubmitterV1>,
    /// Finalized ledger snapshot reader.
    pub snapshot_reader: Arc<dyn ModerationFinalizedSnapshotReaderV1>,
    /// Appeal-finance terminal sink.
    pub settlement_sink: Arc<dyn ModerationTerminalHandoffSinkV1>,
    /// Governance/transparency terminal sink.
    pub publication_sink: Arc<dyn ModerationTerminalHandoffSinkV1>,
}

impl fmt::Debug for ModerationOrchestratorDepsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModerationOrchestratorDepsV1")
            .field("submitter", &"<runtime-only>")
            .field("snapshot_reader", &"<runtime-only>")
            .field("settlement_sink", &"<runtime-only>")
            .field("publication_sink", &"<runtime-only>")
            .finish()
    }
}

/// Public state of a native moderation submission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationOperationStatusV1 {
    /// Durable outbox entry exists and finalization is pending.
    Pending,
    /// Exact native effect is present in the finalized projection.
    Finalized,
    /// Safe terminal rejection is retained as a tombstone.
    Rejected,
}

/// Response returned for a submitted or replayed operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationSubmitOutcomeV1 {
    /// Stable semantic identity.
    pub operation_id: [u8; 32],
    /// Native signed transaction hash, when known.
    pub transaction_id: Option<[u8; 32]>,
    /// Current state.
    pub status: ModerationOperationStatusV1,
    /// Finalized anchor used for reconciliation.
    pub finalized_cursor: ModerationFinalizedCursorV1,
    /// True for an exact replay of a retained identity.
    pub replay: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum StoredOperationStatusV1 {
    Pending,
    Finalized,
    Rejected,
}

impl From<StoredOperationStatusV1> for ModerationOperationStatusV1 {
    fn from(value: StoredOperationStatusV1) -> Self {
        match value {
            StoredOperationStatusV1::Pending => Self::Pending,
            StoredOperationStatusV1::Finalized => Self::Finalized,
            StoredOperationStatusV1::Rejected => Self::Rejected,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredOperationV1 {
    operation_id: [u8; 32],
    authority: AccountId,
    action_digest: [u8; 32],
    status: StoredOperationStatusV1,
    transaction_id: Option<[u8; 32]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum StoredOutboxStateV1 {
    Ready,
    Ambiguous,
    Submitted,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredOutboxEntryV1 {
    operation_id: [u8; 32],
    authority: AccountId,
    action: ModerationNativeActionV1,
    action_digest: [u8; 32],
    request_binding_digest: [u8; 32],
    baseline_finalized_height: u64,
    transaction_id: Option<[u8; 32]>,
    attempts: u32,
    state: StoredOutboxStateV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum StoredDeadLetterReasonV1 {
    PermanentRejection,
    FinalizedConflict,
    RetryExhaustedNotFound,
    HandoffPermanentRejection,
    HandoffRetryExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredDeadLetterV1 {
    identity: [u8; 32],
    action_label: String,
    reason: StoredDeadLetterReasonV1,
    finalized_cursor: ModerationFinalizedCursorV1,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredHandoffV1 {
    handoff: ModerationTerminalHandoffV1,
    attempts: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationOrchestratorCheckpointV1 {
    version: u16,
    generation: u64,
    finalized_snapshot: Option<ModerationFinalizedLedgerSnapshotV1>,
    finalized_snapshot_digest: Option<[u8; 32]>,
    operations: Vec<StoredOperationV1>,
    outbox: Vec<StoredOutboxEntryV1>,
    dead_letters: Vec<StoredDeadLetterV1>,
    pending_handoffs: Vec<StoredHandoffV1>,
    completed_handoffs: Vec<[u8; 32]>,
}

impl Default for ModerationOrchestratorCheckpointV1 {
    fn default() -> Self {
        Self {
            version: MODERATION_ORCHESTRATOR_CHECKPOINT_VERSION_V1,
            generation: 0,
            finalized_snapshot: None,
            finalized_snapshot_digest: None,
            operations: Vec::new(),
            outbox: Vec::new(),
            dead_letters: Vec::new(),
            pending_handoffs: Vec::new(),
            completed_handoffs: Vec::new(),
        }
    }
}

/// Finalized-chain moderation orchestrator.
pub struct ModerationOrchestratorV1 {
    config: ModerationOrchestratorConfigV1,
    deps: ModerationOrchestratorDepsV1,
    state: Mutex<ModerationOrchestratorCheckpointV1>,
    durability_faulted: AtomicBool,
}

impl fmt::Debug for ModerationOrchestratorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModerationOrchestratorV1")
            .field("config", &self.config)
            .field("deps", &self.deps)
            .field(
                "durability_faulted",
                &self.durability_faulted.load(Ordering::Acquire),
            )
            .finish_non_exhaustive()
    }
}

impl ModerationOrchestratorV1 {
    /// Open an orchestrator from a private bounded checkpoint.
    ///
    /// # Errors
    ///
    /// Fails closed for invalid bounds, unsafe filesystem objects, oversized or
    /// noncanonical checkpoint bytes, or internally inconsistent durable state.
    pub fn open(
        config: ModerationOrchestratorConfigV1,
        deps: ModerationOrchestratorDepsV1,
    ) -> Result<Self, ModerationOrchestratorError> {
        config.validate()?;
        ensure_secure_parent(&config.checkpoint_path)?;
        let state = match read_bounded_file(&config.checkpoint_path, config.checkpoint_max_bytes)? {
            None => ModerationOrchestratorCheckpointV1::default(),
            Some(bytes) => {
                let limits = checkpoint_decode_limits(config.checkpoint_max_bytes)?;
                let checkpoint =
                    decode_from_bytes_with_limits::<ModerationOrchestratorCheckpointV1>(
                        &bytes, limits,
                    )
                    .map_err(|error| {
                        ModerationOrchestratorError::CheckpointCorrupt(format!(
                            "decode checkpoint: {error}"
                        ))
                    })?;
                let canonical = norito::to_bytes(&checkpoint).map_err(|error| {
                    ModerationOrchestratorError::CheckpointCorrupt(format!(
                        "re-encode checkpoint: {error}"
                    ))
                })?;
                if canonical != bytes {
                    return Err(ModerationOrchestratorError::CheckpointCorrupt(
                        "checkpoint is not canonical Norito".to_owned(),
                    ));
                }
                checkpoint
            }
        };
        validate_checkpoint(&state, &config)?;
        Ok(Self {
            config,
            deps,
            state: Mutex::new(state),
            durability_faulted: AtomicBool::new(false),
        })
    }

    /// Return the validated non-secret runtime configuration.
    #[must_use]
    pub fn config(&self) -> &ModerationOrchestratorConfigV1 {
        &self.config
    }

    /// Reconcile the complete local projection and all durable delivery state.
    ///
    /// # Errors
    ///
    /// Fails closed on stale/equivocating finalized anchors, invalid snapshots,
    /// unsafe ambiguous retry state, capacity exhaustion, or checkpoint failure.
    pub fn reconcile(&self) -> Result<ModerationFinalizedCursorV1, ModerationOrchestratorError> {
        let mut state = self.lock_state()?;
        self.reconcile_locked(&mut state)?;
        let cursor = snapshot_cursor(&state)?;
        self.process_handoffs_locked(&mut state)?;
        Ok(cursor)
    }

    /// Submit one authenticated native action after finalized-state reconciliation.
    ///
    /// The `request_binding_digest` must be computed from the exact authenticated
    /// method, normalized path/query, raw body, verified account, and action.
    ///
    /// # Errors
    ///
    /// Fails for invalid authority binding, conflicting semantic replay,
    /// finalized conflict, missing runtime state, bounds, or durability errors.
    pub fn submit(
        &self,
        authority: AccountId,
        action: ModerationNativeActionV1,
        request_binding_digest: [u8; 32],
    ) -> Result<ModerationSubmitOutcomeV1, ModerationOrchestratorError> {
        if request_binding_digest == [0; 32] {
            return Err(ModerationOrchestratorError::InvalidRequestBinding);
        }
        action.validate_authority(&authority)?;
        action.canonical_bytes()?;
        let action_digest = action.action_digest()?;
        let operation_id = action.operation_id(&authority)?;

        let mut state = self.lock_state()?;
        self.reconcile_locked(&mut state)?;
        let cursor = snapshot_cursor(&state)?;

        if let Some(existing) = find_operation(&state, operation_id) {
            if existing.action_digest != action_digest || existing.authority != authority {
                return Err(ModerationOrchestratorError::IdempotencyConflict { operation_id });
            }
            return Ok(ModerationSubmitOutcomeV1 {
                operation_id,
                transaction_id: existing.transaction_id,
                status: existing.status.into(),
                finalized_cursor: cursor,
                replay: true,
            });
        }

        match action_effect(
            state
                .finalized_snapshot
                .as_ref()
                .ok_or(ModerationOrchestratorError::FinalizedReaderUnavailable)?,
            &authority,
            &action,
        )? {
            ActionEffect::Exact => {
                ensure_operation_capacity(&state, &self.config)?;
                state.operations.push(StoredOperationV1 {
                    operation_id,
                    authority,
                    action_digest,
                    status: StoredOperationStatusV1::Finalized,
                    transaction_id: None,
                });
                self.persist_checkpoint_locked(&mut state)?;
                return Ok(ModerationSubmitOutcomeV1 {
                    operation_id,
                    transaction_id: None,
                    status: ModerationOperationStatusV1::Finalized,
                    finalized_cursor: cursor,
                    replay: true,
                });
            }
            ActionEffect::Conflict => {
                return Err(ModerationOrchestratorError::FinalizedConflict { operation_id });
            }
            ActionEffect::Absent => {}
        }

        ensure_operation_capacity(&state, &self.config)?;
        ensure_outbox_capacity(&state, &self.config)?;
        state.operations.push(StoredOperationV1 {
            operation_id,
            authority: authority.clone(),
            action_digest,
            status: StoredOperationStatusV1::Pending,
            transaction_id: None,
        });
        state.outbox.push(StoredOutboxEntryV1 {
            operation_id,
            authority: authority.clone(),
            action: action.clone(),
            action_digest,
            request_binding_digest,
            baseline_finalized_height: cursor.height,
            transaction_id: None,
            attempts: 0,
            state: StoredOutboxStateV1::Ready,
        });
        self.persist_checkpoint_locked(&mut state)?;

        // Consult the stable cross-replica operation registry before the first
        // submission. A racing peer may already own the same transaction.
        self.reconcile_outbox_locked(&mut state)?;
        let operation = find_operation(&state, operation_id).ok_or_else(|| {
            ModerationOrchestratorError::CheckpointCorrupt(
                "submitted operation disappeared".to_owned(),
            )
        })?;
        Ok(ModerationSubmitOutcomeV1 {
            operation_id,
            transaction_id: operation.transaction_id,
            status: operation.status.into(),
            finalized_cursor: cursor,
            replay: false,
        })
    }

    /// Run deterministic deadline maintenance after a finalized reconciliation.
    ///
    /// Only native sortition, activation/failover, and finalization ISIs are
    /// emitted. Each uses the injected governance authority and stable semantic
    /// identity. At most `limit` actions are attempted.
    ///
    /// # Errors
    ///
    /// Fails for invalid finalized state, bounds, submission, or durability errors.
    pub fn run_maintenance(
        &self,
        governance_authority: AccountId,
        now_unix_ms: u64,
        limit: usize,
    ) -> Result<Vec<ModerationSubmitOutcomeV1>, ModerationOrchestratorError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let cursor = self.reconcile()?;
        let snapshot = self
            .snapshot()
            .ok_or(ModerationOrchestratorError::FinalizedReaderUnavailable)?;
        let mut actions = Vec::new();
        for appeal_view in &snapshot.appeals {
            if actions.len() >= limit {
                break;
            }
            let appeal = &appeal_view.appeal;
            match appeal.status {
                ModerationAppealStatusV1::RegisteringJurors
                    if now_unix_ms > appeal.intake.registration_deadline_unix_ms =>
                {
                    let selection = sorafs_moderation_select_panel_v1(
                        appeal.intake_digest,
                        appeal.pop_snapshot_digest,
                        cursor.block_hash,
                        &appeal_view.eligibility,
                        appeal.intake.panel_size,
                        appeal.intake.waitlist_size,
                        appeal.intake.quorum,
                    );
                    let (jurors, waitlist) = match selection {
                        Ok((jurors, waitlist, _, _)) => (jurors, waitlist),
                        Err(ModerationSortitionError::InsufficientEligiblePool { .. }) => {
                            (Vec::new(), Vec::new())
                        }
                        Err(error) => {
                            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                                format!("finalized eligibility cannot be sorted: {error}"),
                            ));
                        }
                    };
                    actions.push(ModerationNativeActionV1::FinalizeSortition(
                        FinalizeSorafsModerationSortition::new(
                            appeal.intake.case_id.clone(),
                            appeal.intake.round_id.clone(),
                            appeal.pop_snapshot_digest,
                            cursor.block_hash,
                            jurors,
                            waitlist,
                        ),
                    ));
                }
                ModerationAppealStatusV1::AwaitingAcceptance
                    if appeal.selection.is_some()
                        && now_unix_ms > appeal.intake.acceptance_deadline_unix_ms =>
                {
                    let Some(selection) = appeal.selection.as_ref() else {
                        return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                            "awaiting-acceptance appeal lost its selection".to_owned(),
                        ));
                    };
                    actions.push(ModerationNativeActionV1::ActivateCase(
                        ActivateSorafsModerationCase::new(
                            appeal.intake.case_id.clone(),
                            appeal.intake.round_id.clone(),
                            selection.sortition_digest,
                        ),
                    ));
                }
                ModerationAppealStatusV1::BallotOpen => {
                    if let Some(case) =
                        snapshot.case(&appeal.intake.case_id, &appeal.intake.round_id)
                        && now_unix_ms > case.case.spec.reveal_deadline_unix_ms
                        && case.outcome.is_none()
                    {
                        actions.push(ModerationNativeActionV1::FinalizeCase(
                            FinalizeSorafsModerationCase::new(
                                appeal.intake.case_id.clone(),
                                appeal.intake.round_id.clone(),
                            ),
                        ));
                    }
                }
                ModerationAppealStatusV1::InsufficientEligiblePool
                | ModerationAppealStatusV1::FailoverExhausted
                | ModerationAppealStatusV1::Finalized
                | ModerationAppealStatusV1::RegisteringJurors
                | ModerationAppealStatusV1::AwaitingAcceptance => {}
            }
        }

        let mut outcomes = Vec::with_capacity(actions.len());
        for action in actions {
            let binding =
                maintenance_request_binding_digest(&governance_authority, &action, cursor)?;
            outcomes.push(self.submit(governance_authority.clone(), action, binding)?);
        }
        Ok(outcomes)
    }

    /// Return the complete current finalized projection.
    #[must_use]
    pub fn snapshot(&self) -> Option<ModerationFinalizedLedgerSnapshotV1> {
        if self.durability_faulted.load(Ordering::Acquire) {
            return None;
        }
        self.state.lock().ok().and_then(|state| {
            if self.durability_faulted.load(Ordering::Acquire) {
                None
            } else {
                state.finalized_snapshot.clone()
            }
        })
    }

    /// Return one committed appeal projection.
    #[must_use]
    pub fn appeal(&self, case_id: &str, round_id: &str) -> Option<ModerationFinalizedAppealViewV1> {
        self.snapshot()
            .and_then(|snapshot| snapshot.appeal(case_id, round_id).cloned())
    }

    /// Return one committed case projection.
    #[must_use]
    pub fn case(&self, case_id: &str, round_id: &str) -> Option<ModerationFinalizedCaseViewV1> {
        self.snapshot()
            .and_then(|snapshot| snapshot.case(case_id, round_id).cloned())
    }

    /// Return committed events strictly after an exclusive cursor.
    #[must_use]
    pub fn events_after(
        &self,
        after: Option<ModerationFinalizedEventCursorV1>,
        limit: usize,
    ) -> Vec<ModerationFinalizedEventV1> {
        self.snapshot().map_or_else(Vec::new, |snapshot| {
            snapshot
                .events
                .into_iter()
                .filter(|event| after.is_none_or(|cursor| event.sequence > cursor.sequence))
                .take(limit.min(self.config.max_events))
                .collect()
        })
    }

    fn lock_state(
        &self,
    ) -> Result<
        std::sync::MutexGuard<'_, ModerationOrchestratorCheckpointV1>,
        ModerationOrchestratorError,
    > {
        if self.durability_faulted.load(Ordering::Acquire) {
            return Err(ModerationOrchestratorError::DurabilityFaulted);
        }
        let state = self
            .state
            .lock()
            .map_err(|_| ModerationOrchestratorError::StateLockPoisoned)?;
        if self.durability_faulted.load(Ordering::Acquire) {
            return Err(ModerationOrchestratorError::DurabilityFaulted);
        }
        Ok(state)
    }

    fn persist_checkpoint_locked(
        &self,
        state: &mut ModerationOrchestratorCheckpointV1,
    ) -> Result<(), ModerationOrchestratorError> {
        if self.durability_faulted.load(Ordering::Acquire) {
            return Err(ModerationOrchestratorError::DurabilityFaulted);
        }
        if let Err(error) = persist_checkpoint(&self.config, state) {
            self.durability_faulted.store(true, Ordering::Release);
            return Err(error);
        }
        Ok(())
    }

    fn reconcile_locked(
        &self,
        state: &mut ModerationOrchestratorCheckpointV1,
    ) -> Result<(), ModerationOrchestratorError> {
        let snapshot = self
            .deps
            .snapshot_reader
            .read_finalized_snapshot(self.config.max_cases, self.config.max_events)
            .map_err(|error| match error {
                ModerationSnapshotReadErrorV1::Unavailable => {
                    ModerationOrchestratorError::FinalizedReaderUnavailable
                }
                ModerationSnapshotReadErrorV1::ResourceExhausted => {
                    ModerationOrchestratorError::ResourceExhausted {
                        resource: "finalized snapshot",
                        limit: self.config.max_cases,
                    }
                }
                ModerationSnapshotReadErrorV1::InvalidSnapshot => {
                    ModerationOrchestratorError::InvalidFinalizedSnapshot(
                        "reader rejected finalized snapshot".to_owned(),
                    )
                }
            })?;
        validate_finalized_snapshot(&snapshot, &self.config)?;
        let digest = finalized_snapshot_digest(&snapshot)?;
        if let (Some(previous), Some(previous_digest)) = (
            state.finalized_snapshot.as_ref(),
            state.finalized_snapshot_digest,
        ) {
            if snapshot.finalized_height < previous.finalized_height {
                return Err(ModerationOrchestratorError::StaleFinalizedCursor {
                    current: previous.finalized_height,
                    observed: snapshot.finalized_height,
                });
            }
            if snapshot.finalized_height == previous.finalized_height {
                if snapshot.finalized_block_hash != previous.finalized_block_hash
                    || digest != previous_digest
                {
                    return Err(ModerationOrchestratorError::FinalizedEquivocation {
                        height: snapshot.finalized_height,
                    });
                }
            }
        }
        state.finalized_snapshot = Some(snapshot);
        state.finalized_snapshot_digest = Some(digest);
        self.reconcile_outbox_locked(state)?;
        self.queue_terminal_handoffs_locked(state)?;
        self.persist_checkpoint_locked(state)
    }

    fn reconcile_outbox_locked(
        &self,
        state: &mut ModerationOrchestratorCheckpointV1,
    ) -> Result<(), ModerationOrchestratorError> {
        let snapshot = state
            .finalized_snapshot
            .as_ref()
            .ok_or(ModerationOrchestratorError::FinalizedReaderUnavailable)?
            .clone();
        let cursor = snapshot.anchor();
        let operation_index = state
            .operations
            .iter()
            .enumerate()
            .map(|(index, operation)| (operation.operation_id, index))
            .collect::<BTreeMap<_, _>>();
        let mut retained = Vec::with_capacity(state.outbox.len());
        let mut dead = Vec::new();

        for mut entry in std::mem::take(&mut state.outbox) {
            let operation_position = operation_index
                .get(&entry.operation_id)
                .copied()
                .ok_or_else(|| {
                    ModerationOrchestratorError::CheckpointCorrupt(
                        "outbox entry has no idempotency record".to_owned(),
                    )
                })?;
            match action_effect(&snapshot, &entry.authority, &entry.action)? {
                ActionEffect::Exact => {
                    let operation = &mut state.operations[operation_position];
                    operation.status = StoredOperationStatusV1::Finalized;
                    if operation.transaction_id.is_none() {
                        operation.transaction_id = entry.transaction_id;
                    }
                    continue;
                }
                ActionEffect::Conflict => {
                    state.operations[operation_position].status = StoredOperationStatusV1::Rejected;
                    dead.push(StoredDeadLetterV1 {
                        identity: entry.operation_id,
                        action_label: entry.action.label().to_owned(),
                        reason: StoredDeadLetterReasonV1::FinalizedConflict,
                        finalized_cursor: cursor,
                    });
                    continue;
                }
                ActionEffect::Absent => {}
            }

            let lookup = self
                .deps
                .submitter
                .lookup(entry.operation_id, entry.transaction_id);
            match lookup {
                ModerationSubmissionLookupV1::Pending { transaction_id }
                | ModerationSubmissionLookupV1::Applied { transaction_id } => {
                    entry.transaction_id = Some(transaction_id);
                    entry.state = StoredOutboxStateV1::Submitted;
                    state.operations[operation_position].transaction_id = Some(transaction_id);
                    retained.push(entry);
                }
                ModerationSubmissionLookupV1::Unknown => {
                    entry.state = StoredOutboxStateV1::Ambiguous;
                    retained.push(entry);
                }
                ModerationSubmissionLookupV1::Rejected {
                    transaction_id,
                    observed_finalized_height,
                } if observed_finalized_height >= entry.baseline_finalized_height => {
                    state.operations[operation_position].status = StoredOperationStatusV1::Rejected;
                    if transaction_id.is_some() {
                        state.operations[operation_position].transaction_id = transaction_id;
                    }
                    dead.push(StoredDeadLetterV1 {
                        identity: entry.operation_id,
                        action_label: entry.action.label().to_owned(),
                        reason: StoredDeadLetterReasonV1::PermanentRejection,
                        finalized_cursor: cursor,
                    });
                }
                ModerationSubmissionLookupV1::NotFound {
                    observed_finalized_height,
                } if observed_finalized_height >= entry.baseline_finalized_height => {
                    if entry.attempts >= self.config.max_submit_attempts {
                        state.operations[operation_position].status =
                            StoredOperationStatusV1::Rejected;
                        dead.push(StoredDeadLetterV1 {
                            identity: entry.operation_id,
                            action_label: entry.action.label().to_owned(),
                            reason: StoredDeadLetterReasonV1::RetryExhaustedNotFound,
                            finalized_cursor: cursor,
                        });
                    } else {
                        entry.state = StoredOutboxStateV1::Ready;
                        retained.push(entry);
                    }
                }
                ModerationSubmissionLookupV1::Rejected { .. }
                | ModerationSubmissionLookupV1::NotFound { .. } => {
                    entry.state = StoredOutboxStateV1::Ambiguous;
                    retained.push(entry);
                }
            }
        }
        if state.dead_letters.len().saturating_add(dead.len()) > self.config.max_idempotency_records
        {
            return Err(ModerationOrchestratorError::ResourceExhausted {
                resource: "dead letters",
                limit: self.config.max_idempotency_records,
            });
        }
        state.outbox = retained;
        state.dead_letters.extend(dead);

        let ready = state
            .outbox
            .iter()
            .filter(|entry| entry.state == StoredOutboxStateV1::Ready)
            .map(|entry| entry.operation_id)
            .collect::<Vec<_>>();
        for operation_id in ready {
            let canonical = state
                .outbox
                .iter()
                .find(|entry| entry.operation_id == operation_id)
                .ok_or_else(|| {
                    ModerationOrchestratorError::CheckpointCorrupt(
                        "ready outbox entry disappeared".to_owned(),
                    )
                })?
                .action
                .canonical_bytes()?;
            self.try_submit_locked(state, operation_id, canonical)?;
        }
        Ok(())
    }

    fn try_submit_locked(
        &self,
        state: &mut ModerationOrchestratorCheckpointV1,
        operation_id: [u8; 32],
        canonical_action: Vec<u8>,
    ) -> Result<(), ModerationOrchestratorError> {
        let position = state
            .outbox
            .iter()
            .position(|entry| entry.operation_id == operation_id)
            .ok_or_else(|| {
                ModerationOrchestratorError::CheckpointCorrupt(
                    "submission outbox entry is missing".to_owned(),
                )
            })?;
        if state.outbox[position].attempts >= self.config.max_submit_attempts {
            return Ok(());
        }
        state.outbox[position].attempts = state.outbox[position].attempts.saturating_add(1);
        state.outbox[position].state = StoredOutboxStateV1::Ambiguous;
        self.persist_checkpoint_locked(state)?;

        let entry = state.outbox[position].clone();
        let request = ModerationTransactionRequestV1 {
            operation_id,
            authority: entry.authority,
            action: entry.action,
            canonical_action,
            action_digest: entry.action_digest,
            request_binding_digest: entry.request_binding_digest,
            baseline_finalized_height: entry.baseline_finalized_height,
        };
        let result = self.deps.submitter.submit(&request);
        match result {
            Ok(receipt) => {
                if receipt.transaction_id == [0; 32]
                    || receipt.observed_finalized_height < entry.baseline_finalized_height
                {
                    if receipt.transaction_id != [0; 32] {
                        state.outbox[position].transaction_id = Some(receipt.transaction_id);
                        if let Some(operation) = state
                            .operations
                            .iter_mut()
                            .find(|operation| operation.operation_id == operation_id)
                        {
                            operation.transaction_id = Some(receipt.transaction_id);
                        }
                    }
                    state.outbox[position].state = StoredOutboxStateV1::Ambiguous;
                } else {
                    state.outbox[position].transaction_id = Some(receipt.transaction_id);
                    state.outbox[position].state = StoredOutboxStateV1::Submitted;
                    if let Some(operation) = state
                        .operations
                        .iter_mut()
                        .find(|operation| operation.operation_id == operation_id)
                    {
                        operation.transaction_id = Some(receipt.transaction_id);
                    }
                }
            }
            Err(
                ModerationSubmissionFailureV1::Ambiguous
                | ModerationSubmissionFailureV1::RuntimeUnavailable,
            ) => {
                state.outbox[position].state = StoredOutboxStateV1::Ambiguous;
            }
            Err(
                ModerationSubmissionFailureV1::NotSubmittedUnavailable
                | ModerationSubmissionFailureV1::NotSubmittedBackpressure,
            ) => {
                state.outbox[position].state = StoredOutboxStateV1::Ready;
            }
            Err(ModerationSubmissionFailureV1::PermanentRejection) => {
                ensure_dead_letter_capacity(state, &self.config, 1)?;
                let cursor = snapshot_cursor(state)?;
                state.outbox.remove(position);
                if let Some(operation) = state
                    .operations
                    .iter_mut()
                    .find(|operation| operation.operation_id == operation_id)
                {
                    operation.status = StoredOperationStatusV1::Rejected;
                }
                state.dead_letters.push(StoredDeadLetterV1 {
                    identity: operation_id,
                    action_label: request.action.label().to_owned(),
                    reason: StoredDeadLetterReasonV1::PermanentRejection,
                    finalized_cursor: cursor,
                });
            }
        }
        self.persist_checkpoint_locked(state)
    }

    fn queue_terminal_handoffs_locked(
        &self,
        state: &mut ModerationOrchestratorCheckpointV1,
    ) -> Result<(), ModerationOrchestratorError> {
        let snapshot = state
            .finalized_snapshot
            .as_ref()
            .ok_or(ModerationOrchestratorError::FinalizedReaderUnavailable)?;
        let completed = state
            .completed_handoffs
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let pending = state
            .pending_handoffs
            .iter()
            .map(|entry| entry.handoff.handoff_id)
            .collect::<BTreeSet<_>>();
        let mut additions = Vec::new();
        for case in &snapshot.cases {
            let Some(outcome) = case.outcome.as_ref() else {
                continue;
            };
            let outcome_bytes = norito::to_bytes(outcome).map_err(|error| {
                ModerationOrchestratorError::InvalidFinalizedSnapshot(format!(
                    "encode terminal outcome: {error}"
                ))
            })?;
            let outcome_digest = domain_hash(ACTION_DIGEST_DOMAIN_V1, &[&outcome_bytes]);
            for kind in [
                ModerationTerminalHandoffKindV1::Settlement,
                ModerationTerminalHandoffKindV1::Publication,
            ] {
                let kind_byte = match kind {
                    ModerationTerminalHandoffKindV1::Settlement => 0_u8,
                    ModerationTerminalHandoffKindV1::Publication => 1_u8,
                };
                let handoff_id = domain_hash(
                    HANDOFF_ID_DOMAIN_V1,
                    &[
                        &[kind_byte],
                        outcome.case_id.as_bytes(),
                        outcome.round_id.as_bytes(),
                        &outcome_digest,
                    ],
                );
                if !completed.contains(&handoff_id) && !pending.contains(&handoff_id) {
                    additions.push(StoredHandoffV1 {
                        handoff: ModerationTerminalHandoffV1 {
                            handoff_id,
                            kind,
                            case_id: outcome.case_id.clone(),
                            round_id: outcome.round_id.clone(),
                            outcome_digest,
                            finalized_cursor: snapshot.anchor(),
                        },
                        attempts: 0,
                    });
                }
            }
        }
        if state
            .pending_handoffs
            .len()
            .saturating_add(state.completed_handoffs.len())
            .saturating_add(additions.len())
            > self.config.max_handoffs
        {
            return Err(ModerationOrchestratorError::ResourceExhausted {
                resource: "terminal handoffs",
                limit: self.config.max_handoffs,
            });
        }
        state.pending_handoffs.extend(additions);
        Ok(())
    }

    fn process_handoffs_locked(
        &self,
        state: &mut ModerationOrchestratorCheckpointV1,
    ) -> Result<(), ModerationOrchestratorError> {
        ensure_dead_letter_capacity(state, &self.config, state.pending_handoffs.len())?;
        let cursor = snapshot_cursor(state)?;
        let mut retained = Vec::with_capacity(state.pending_handoffs.len());
        let mut completed = Vec::new();
        let mut dead = Vec::new();
        for mut entry in std::mem::take(&mut state.pending_handoffs) {
            entry.attempts = entry.attempts.saturating_add(1);
            let sink = match entry.handoff.kind {
                ModerationTerminalHandoffKindV1::Settlement => &self.deps.settlement_sink,
                ModerationTerminalHandoffKindV1::Publication => &self.deps.publication_sink,
            };
            match sink.deliver(&entry.handoff) {
                Ok(()) => completed.push(entry.handoff.handoff_id),
                Err(ModerationHandoffFailureV1::Permanent) => {
                    dead.push(StoredDeadLetterV1 {
                        identity: entry.handoff.handoff_id,
                        action_label: handoff_label(entry.handoff.kind).to_owned(),
                        reason: StoredDeadLetterReasonV1::HandoffPermanentRejection,
                        finalized_cursor: cursor,
                    });
                }
                Err(
                    ModerationHandoffFailureV1::NotDelivered
                    | ModerationHandoffFailureV1::Ambiguous,
                ) if entry.attempts >= self.config.max_submit_attempts => {
                    dead.push(StoredDeadLetterV1 {
                        identity: entry.handoff.handoff_id,
                        action_label: handoff_label(entry.handoff.kind).to_owned(),
                        reason: StoredDeadLetterReasonV1::HandoffRetryExhausted,
                        finalized_cursor: cursor,
                    });
                }
                Err(
                    ModerationHandoffFailureV1::NotDelivered
                    | ModerationHandoffFailureV1::Ambiguous,
                ) => retained.push(entry),
            }
        }
        state.pending_handoffs = retained;
        state.completed_handoffs.extend(completed);
        state.completed_handoffs.sort_unstable();
        state.completed_handoffs.dedup();
        state.dead_letters.extend(dead);
        self.persist_checkpoint_locked(state)
    }
}

/// Compute the digest that binds authenticated HTTP material to one native action.
///
/// `canonical_path_and_query` must be the same normalized path/query string
/// covered by canonical app authentication.
///
/// # Errors
///
/// Returns an error if the action cannot be canonically encoded.
pub fn moderation_request_binding_digest_v1(
    method: &str,
    canonical_path_and_query: &str,
    raw_body: &[u8],
    authority: &AccountId,
    action: &ModerationNativeActionV1,
) -> Result<[u8; 32], ModerationOrchestratorError> {
    validate_request_binding_target(method, canonical_path_and_query)?;
    let action_bytes = action.canonical_bytes()?;
    let authority = authority.to_string();
    Ok(domain_hash(
        REQUEST_BINDING_DOMAIN_V1,
        &[
            method.as_bytes(),
            canonical_path_and_query.as_bytes(),
            raw_body,
            authority.as_bytes(),
            &action_bytes,
        ],
    ))
}

fn validate_request_binding_target(
    method: &str,
    canonical_path_and_query: &str,
) -> Result<(), ModerationOrchestratorError> {
    fn is_http_token_byte(byte: u8) -> bool {
        byte.is_ascii_uppercase()
            || byte.is_ascii_digit()
            || matches!(
                byte,
                b'!' | b'#'
                    | b'$'
                    | b'%'
                    | b'&'
                    | b'\''
                    | b'*'
                    | b'+'
                    | b'-'
                    | b'.'
                    | b'^'
                    | b'_'
                    | b'`'
                    | b'|'
                    | b'~'
            )
    }

    if method.is_empty() || !method.bytes().all(is_http_token_byte) {
        return Err(ModerationOrchestratorError::InvalidRequestBinding);
    }
    if !canonical_path_and_query.starts_with('/')
        || canonical_path_and_query.contains('#')
        || canonical_path_and_query.contains('\\')
        || canonical_path_and_query
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == b' ')
        || canonical_path_and_query.matches('?').count() > 1
    {
        return Err(ModerationOrchestratorError::InvalidRequestBinding);
    }
    let path = canonical_path_and_query
        .split_once('?')
        .map_or(canonical_path_and_query, |(path, _)| path);
    if path
        .split('/')
        .any(|segment| segment == "." || segment == "..")
    {
        return Err(ModerationOrchestratorError::InvalidRequestBinding);
    }
    Ok(())
}

fn maintenance_request_binding_digest(
    authority: &AccountId,
    action: &ModerationNativeActionV1,
    cursor: ModerationFinalizedCursorV1,
) -> Result<[u8; 32], ModerationOrchestratorError> {
    let body = norito::to_bytes(action).map_err(|error| {
        ModerationOrchestratorError::InvalidAction(format!("encode maintenance action: {error}"))
    })?;
    moderation_request_binding_digest_v1(
        "INTERNAL",
        "/v1/sorafs/moderation/orchestrator/maintenance",
        &[
            body,
            cursor.height.to_le_bytes().to_vec(),
            cursor.block_hash.to_vec(),
        ]
        .concat(),
        authority,
        action,
    )
}

fn validate_finalized_snapshot(
    snapshot: &ModerationFinalizedLedgerSnapshotV1,
    config: &ModerationOrchestratorConfigV1,
) -> Result<(), ModerationOrchestratorError> {
    if snapshot.version != MODERATION_FINALIZED_SNAPSHOT_VERSION_V1
        || snapshot.finalized_height == 0
        || snapshot.finalized_block_hash == [0; 32]
    {
        return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
            "snapshot version or finalized anchor is invalid".to_owned(),
        ));
    }
    if snapshot.appeals.len() > config.max_cases
        || snapshot.cases.len() > config.max_cases
        || snapshot.events.len() > config.max_events
    {
        return Err(ModerationOrchestratorError::ResourceExhausted {
            resource: "finalized moderation projection",
            limit: config.max_cases,
        });
    }
    match (&snapshot.policy, &snapshot.status) {
        (None, None)
            if snapshot.appeals.is_empty()
                && snapshot.cases.is_empty()
                && snapshot.events.is_empty() => {}
        (Some(policy), Some(_)) => {
            policy.policy.validate().map_err(|error| {
                ModerationOrchestratorError::InvalidFinalizedSnapshot(error.to_string())
            })?;
            if policy.activated_at_unix_ms == 0
                || policy.policy.digest().map_err(|error| {
                    ModerationOrchestratorError::InvalidFinalizedSnapshot(error.to_string())
                })? != policy.policy_digest
            {
                return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                    "active policy digest mismatch".to_owned(),
                ));
            }
        }
        _ => {
            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "policy/status presence is inconsistent".to_owned(),
            ));
        }
    }

    let mut prior_appeal = None;
    let mut eligibility_total = 0_u64;
    let mut panel_selection_total = 0_u64;
    let mut assignment_acceptance_total = 0_u64;
    let mut failover_replacement_total = 0_u64;
    let mut failed_panel_formation_total = 0_u64;
    for entry in &snapshot.appeals {
        let key = appeal_key(entry);
        require_strict_key_order(&mut prior_appeal, &key, "appeals")?;
        let appeal = &entry.appeal;
        appeal.intake.validate().map_err(|error| {
            ModerationOrchestratorError::InvalidFinalizedSnapshot(error.to_string())
        })?;
        appeal.policy.validate().map_err(|error| {
            ModerationOrchestratorError::InvalidFinalizedSnapshot(error.to_string())
        })?;
        appeal.pop_snapshot.validate().map_err(|error| {
            ModerationOrchestratorError::InvalidFinalizedSnapshot(error.to_string())
        })?;
        let policy_digest = appeal.policy.digest().map_err(|error| {
            ModerationOrchestratorError::InvalidFinalizedSnapshot(error.to_string())
        })?;
        let pop_snapshot_digest = appeal.pop_snapshot.digest().map_err(|error| {
            ModerationOrchestratorError::InvalidFinalizedSnapshot(error.to_string())
        })?;
        if appeal.intake.digest().map_err(|error| {
            ModerationOrchestratorError::InvalidFinalizedSnapshot(error.to_string())
        })? != appeal.intake_digest
            || appeal.intake.case_id != key.0
            || appeal.intake.round_id != key.1
            || appeal.intake.policy_digest != policy_digest
            || appeal.pop_snapshot_digest != pop_snapshot_digest
            || appeal.submitted_by != appeal.intake.appellant
            || appeal.submitted_at_unix_ms == 0
            || appeal.eligible_jurors.len()
                > usize::from(MODERATION_LEDGER_MAX_CANDIDATE_POOL_SIZE_V1)
            || entry.eligibility.len() > usize::from(MODERATION_LEDGER_MAX_CANDIDATE_POOL_SIZE_V1)
        {
            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "appeal identity, digest, or candidate bound is invalid".to_owned(),
            ));
        }
        let mut prior_juror = None;
        for eligibility in &entry.eligibility {
            if eligibility.case_id != key.0 || eligibility.round_id != key.1 {
                return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                    "eligibility record escapes its appeal scope".to_owned(),
                ));
            }
            if eligibility.proof_digest == [0; 32]
                || eligibility.nullifier == [0; 32]
                || eligibility.pop_snapshot_digest != appeal.pop_snapshot_digest
                || eligibility.credential_expires_at_epoch == 0
                || eligibility.registered_at_unix_ms == 0
            {
                return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                    "eligibility record contains an inert or mismatched anchor".to_owned(),
                ));
            }
            let juror = eligibility.juror.to_string();
            require_strict_key_order(&mut prior_juror, &juror, "eligibility")?;
        }
        let projected_jurors = entry
            .eligibility
            .iter()
            .map(|record| record.juror.to_string())
            .collect::<Vec<_>>();
        let recorded_jurors = appeal
            .eligible_jurors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        if projected_jurors != recorded_jurors {
            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "eligibility projection does not match appeal candidate index".to_owned(),
            ));
        }
        validate_appeal_lifecycle(entry)?;
        eligibility_total = eligibility_total
            .checked_add(u64::try_from(entry.eligibility.len()).unwrap_or(u64::MAX))
            .ok_or_else(|| {
                ModerationOrchestratorError::InvalidFinalizedSnapshot(
                    "eligibility total overflow".to_owned(),
                )
            })?;
        panel_selection_total = panel_selection_total
            .checked_add(u64::from(appeal.selection.is_some()))
            .ok_or_else(|| {
                ModerationOrchestratorError::InvalidFinalizedSnapshot(
                    "panel selection total overflow".to_owned(),
                )
            })?;
        assignment_acceptance_total = assignment_acceptance_total
            .checked_add(u64::try_from(appeal.accepted_jurors.len()).unwrap_or(u64::MAX))
            .ok_or_else(|| {
                ModerationOrchestratorError::InvalidFinalizedSnapshot(
                    "assignment acceptance total overflow".to_owned(),
                )
            })?;
        failover_replacement_total = failover_replacement_total
            .checked_add(u64::try_from(appeal.replacements.len()).unwrap_or(u64::MAX))
            .ok_or_else(|| {
                ModerationOrchestratorError::InvalidFinalizedSnapshot(
                    "failover replacement total overflow".to_owned(),
                )
            })?;
        failed_panel_formation_total = failed_panel_formation_total
            .checked_add(u64::from(matches!(
                appeal.status,
                ModerationAppealStatusV1::InsufficientEligiblePool
                    | ModerationAppealStatusV1::FailoverExhausted
            )))
            .ok_or_else(|| {
                ModerationOrchestratorError::InvalidFinalizedSnapshot(
                    "failed panel formation total overflow".to_owned(),
                )
            })?;
    }

    let mut prior_case = None;
    let mut commit_total = 0_u64;
    let mut reveal_total = 0_u64;
    let mut challenge_total = 0_u64;
    let mut outcome_total = 0_u64;
    let mut no_show_total = 0_u64;
    let mut open_cases = 0_u64;
    let mut finalized_cases = 0_u64;
    for entry in &snapshot.cases {
        let key = case_key(entry);
        require_strict_key_order(&mut prior_case, &key, "cases")?;
        entry.case.spec.validate().map_err(|error| {
            ModerationOrchestratorError::InvalidFinalizedSnapshot(error.to_string())
        })?;
        if entry.case.spec.context.case_id != key.0
            || entry.case.spec.round_id != key.1
            || usize::try_from(entry.case.commitment_count).ok() != Some(entry.commits.len())
            || usize::try_from(entry.case.reveal_count).ok() != Some(entry.reveals.len())
            || usize::try_from(entry.case.challenge_count).ok() != Some(entry.challenges.len())
            || entry.challenges.len() > usize::from(MODERATION_LEDGER_MAX_CHALLENGES_V1)
        {
            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "case identity or lifecycle counters are inconsistent".to_owned(),
            ));
        }
        validate_case_records(entry, &key)?;
        match (entry.case.status, entry.outcome.as_ref()) {
            (ModerationCaseStatusV1::Finalized, Some(_)) => {
                finalized_cases = finalized_cases.saturating_add(1);
                outcome_total = outcome_total.saturating_add(1);
            }
            (ModerationCaseStatusV1::Open | ModerationCaseStatusV1::Challenged, None) => {
                open_cases = open_cases.saturating_add(1);
            }
            _ => {
                return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                    "case status and terminal outcome are inconsistent".to_owned(),
                ));
            }
        }
        commit_total = commit_total.saturating_add(entry.commits.len() as u64);
        reveal_total = reveal_total.saturating_add(entry.reveals.len() as u64);
        challenge_total = challenge_total.saturating_add(entry.challenges.len() as u64);
        no_show_total = no_show_total.saturating_add(entry.no_shows.len() as u64);
    }
    for appeal in &snapshot.appeals {
        let case = snapshot.case(
            &appeal.appeal.intake.case_id,
            &appeal.appeal.intake.round_id,
        );
        match appeal.appeal.status {
            ModerationAppealStatusV1::BallotOpen | ModerationAppealStatusV1::Finalized => {
                let case = case.ok_or_else(|| {
                    ModerationOrchestratorError::InvalidFinalizedSnapshot(
                        "activated appeal has no authoritative case".to_owned(),
                    )
                })?;
                validate_case_against_appeal(case, appeal)?;
            }
            ModerationAppealStatusV1::RegisteringJurors
            | ModerationAppealStatusV1::AwaitingAcceptance
            | ModerationAppealStatusV1::InsufficientEligiblePool
            | ModerationAppealStatusV1::FailoverExhausted => {
                if case.is_some() {
                    return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                        "pre-activation or failed appeal unexpectedly has a case".to_owned(),
                    ));
                }
            }
        }
    }
    for case in &snapshot.cases {
        if snapshot
            .appeal(&case.case.spec.context.case_id, &case.case.spec.round_id)
            .is_none()
        {
            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "authoritative case has no appeal lifecycle record".to_owned(),
            ));
        }
    }

    if let Some(status) = snapshot.status {
        let appeal_count = snapshot.appeals.len() as u64;
        if status.updated_at_unix_ms == 0
            || status.appeal_intakes != appeal_count
            || status.eligibility_proofs != eligibility_total
            || status.panel_selections != panel_selection_total
            || status.assignment_acceptances != assignment_acceptance_total
            || status.failover_replacements != failover_replacement_total
            || status.failed_panel_formations != failed_panel_formation_total
            || status.open_cases != open_cases
            || status.finalized_cases != finalized_cases
            || status.commitments != commit_total
            || status.reveals != reveal_total
            || status.challenges != challenge_total
            || status.outcomes != outcome_total
            || status.no_shows != no_show_total
        {
            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "native moderation status counters do not match the complete projection".to_owned(),
            ));
        }
    }

    let mut previous_event = None;
    let mut previous_sequence = None;
    let mut previous_event_block: Option<(u64, [u8; 32], u32)> = None;
    for event in &snapshot.events {
        if event.sequence == 0
            || event.block_height == 0
            || event.block_height > snapshot.finalized_height
            || event.block_hash == [0; 32]
            || *event.event.occurred_at_unix_ms() == 0
            || (event.block_height == snapshot.finalized_height
                && event.block_hash != snapshot.finalized_block_hash)
        {
            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "event cursor is outside the finalized snapshot".to_owned(),
            ));
        }
        match (
            *event.event.kind(),
            event.event.case_id().as_deref(),
            event.event.round_id().as_deref(),
        ) {
            (SorafsModerationLedgerEventKind::PolicyActivated, None, None) => {}
            (SorafsModerationLedgerEventKind::PolicyActivated, _, _)
            | (_, None, _)
            | (_, _, None) => {
                return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                    "typed moderation event has invalid lifecycle scope".to_owned(),
                ));
            }
            (_, Some(case_id), Some(round_id)) => {
                validate_scope(case_id, round_id).map_err(|error| {
                    ModerationOrchestratorError::InvalidFinalizedSnapshot(error.to_string())
                })?
            }
        }
        if previous_sequence.is_some_and(|sequence: u64| {
            sequence
                .checked_add(1)
                .is_none_or(|next| event.sequence != next)
        }) {
            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "committed moderation event sequence is not contiguous".to_owned(),
            ));
        }
        let key = (event.sequence, event.block_height, event.event_index);
        require_strict_key_order(&mut previous_event, &key, "events")?;
        if let Some((height, hash, index)) = previous_event_block {
            match event.block_height.cmp(&height) {
                std::cmp::Ordering::Less => {
                    return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                        "committed moderation event block height regressed".to_owned(),
                    ));
                }
                std::cmp::Ordering::Equal => {
                    if hash != event.block_hash {
                        return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                            "events from one block disagree on the finalized hash".to_owned(),
                        ));
                    }
                    if index
                        .checked_add(1)
                        .is_none_or(|next| event.event_index != next)
                    {
                        return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                            "committed moderation event block index is not contiguous".to_owned(),
                        ));
                    }
                }
                std::cmp::Ordering::Greater => {
                    if event.event_index != 0 {
                        return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                            "committed moderation event did not reset its block index".to_owned(),
                        ));
                    }
                }
            }
        }
        previous_sequence = Some(event.sequence);
        previous_event_block = Some((event.block_height, event.block_hash, event.event_index));
    }
    if let Some(status) = snapshot.status {
        let latest_event = snapshot.events.last().ok_or_else(|| {
            ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "initialized moderation snapshot has no committed event suffix".to_owned(),
            )
        })?;
        if *latest_event.event.occurred_at_unix_ms() != status.updated_at_unix_ms {
            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "latest committed moderation event disagrees with the status timestamp".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_case_records(
    entry: &ModerationFinalizedCaseViewV1,
    key: &(String, String),
) -> Result<(), ModerationOrchestratorError> {
    let mut prior = None;
    for record in &entry.commits {
        require_record_scope(&record.case_id, &record.round_id, key, "commit")?;
        let juror = record.juror.to_string();
        require_strict_key_order(&mut prior, &juror, "commits")?;
        let commit = decode_canonical_commit(&record.canonical_commit)?;
        commit.validate().map_err(|error| {
            ModerationOrchestratorError::InvalidFinalizedSnapshot(error.to_string())
        })?;
        if commit.context.case_id != key.0
            || commit.round_id != key.1
            || commit.juror_id != juror
            || commit.commitment_blake2b_256 == [0; 32]
            || commit.committed_at_unix_ms != record.accepted_at_unix_ms
            || record.accepted_at_unix_ms == 0
        {
            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "commit payload does not match its ledger key".to_owned(),
            ));
        }
    }
    prior = None;
    for record in &entry.reveals {
        require_record_scope(&record.case_id, &record.round_id, key, "reveal")?;
        let juror = record.juror.to_string();
        require_strict_key_order(&mut prior, &juror, "reveals")?;
        let reveal = decode_canonical_reveal(&record.canonical_reveal)?;
        reveal.validate().map_err(|error| {
            ModerationOrchestratorError::InvalidFinalizedSnapshot(error.to_string())
        })?;
        if reveal.context.case_id != key.0
            || reveal.round_id != key.1
            || reveal.juror_id != juror
            || reveal.nonce.len() > MODERATION_LEDGER_MAX_NONCE_BYTES_V1
            || reveal.revealed_at_unix_ms != record.accepted_at_unix_ms
            || record.accepted_at_unix_ms == 0
        {
            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "reveal payload does not match its ledger key".to_owned(),
            ));
        }
    }
    let mut prior_challenge = None;
    let mut pending_challenges = 0_u32;
    let mut accepted_challenges = 0_u32;
    let mut expired_challenges = 0_u32;
    for record in &entry.challenges {
        require_record_scope(&record.case_id, &record.round_id, key, "challenge")?;
        require_strict_key_order(&mut prior_challenge, &record.challenge_id, "challenges")?;
        if !is_canonical_moderation_identifier_v1(&record.challenge_id)
            || record.evidence_digest == [0; 32]
            || record.reason.trim().is_empty()
            || record.reason != record.reason.trim()
            || record.reason.len() > MODERATION_LEDGER_MAX_REASON_BYTES_V1
            || record.reason.chars().any(char::is_control)
            || (record.kind.requires_target_juror() && record.target_juror.is_none())
            || record.raised_at_unix_ms == 0
        {
            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "challenge record contains invalid bounded material".to_owned(),
            ));
        }
        match (
            record.decision,
            record.resolved_by.as_ref(),
            record.resolved_at_unix_ms,
        ) {
            (None, None, None) => pending_challenges = pending_challenges.saturating_add(1),
            (Some(ModerationChallengeDecisionV1::Accepted), Some(_), Some(timestamp))
                if timestamp != 0 =>
            {
                accepted_challenges = accepted_challenges.saturating_add(1);
            }
            (Some(ModerationChallengeDecisionV1::Expired), Some(_), Some(timestamp))
                if timestamp != 0 =>
            {
                expired_challenges = expired_challenges.saturating_add(1);
            }
            (Some(ModerationChallengeDecisionV1::Rejected), Some(_), Some(timestamp))
                if timestamp != 0 => {}
            _ => {
                return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                    "challenge resolution provenance is inconsistent".to_owned(),
                ));
            }
        }
    }
    let indexed = entry.case.challenge_ids.clone();
    let projected = entry
        .challenges
        .iter()
        .map(|record| record.challenge_id.clone())
        .collect::<Vec<_>>();
    if indexed != projected {
        return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
            "challenge index does not match projected challenge records".to_owned(),
        ));
    }
    if entry.case.pending_challenge_count != pending_challenges
        || entry.case.accepted_challenge_count != accepted_challenges
        || entry.case.expired_challenge_count != expired_challenges
        || (entry.case.status == ModerationCaseStatusV1::Challenged && accepted_challenges == 0)
        || (entry.case.status == ModerationCaseStatusV1::Open && accepted_challenges != 0)
    {
        return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
            "challenge lifecycle counters do not match projected records".to_owned(),
        ));
    }
    if let Some(outcome) = &entry.outcome {
        require_record_scope(&outcome.case_id, &outcome.round_id, key, "outcome")?;
        if outcome.no_show_count as usize != entry.no_shows.len()
            || outcome.votes_total != entry.case.reveal_count
            || outcome.counts.checked_total() != Some(outcome.votes_total)
        {
            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "terminal outcome counters are inconsistent".to_owned(),
            ));
        }
    } else if !entry.no_shows.is_empty() {
        return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
            "nonterminal case contains no-show records".to_owned(),
        ));
    }
    let mut prior_no_show = None;
    for record in &entry.no_shows {
        require_record_scope(&record.case_id, &record.round_id, key, "no-show")?;
        require_strict_key_order(&mut prior_no_show, &record.juror.to_string(), "no-shows")?;
    }
    Ok(())
}

fn validate_appeal_lifecycle(
    entry: &ModerationFinalizedAppealViewV1,
) -> Result<(), ModerationOrchestratorError> {
    let appeal = &entry.appeal;
    let mut previous_accepted = None;
    for accepted in &appeal.accepted_jurors {
        let canonical = accepted.to_string();
        require_strict_key_order(&mut previous_accepted, &canonical, "assignment acceptances")?;
    }

    let Some(selection) = appeal.selection.as_ref() else {
        if !appeal.accepted_jurors.is_empty()
            || !appeal.replacements.is_empty()
            || appeal.activated_at_unix_ms.is_some()
            || appeal.finalized_at_unix_ms.is_some()
            || !matches!(
                appeal.status,
                ModerationAppealStatusV1::RegisteringJurors
                    | ModerationAppealStatusV1::InsufficientEligiblePool
            )
            || (appeal.status == ModerationAppealStatusV1::InsufficientEligiblePool
                && entry.eligibility.len() >= usize::from(appeal.intake.panel_size))
        {
            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "appeal lifecycle is inconsistent without a panel selection".to_owned(),
            ));
        }
        return Ok(());
    };

    if selection.randomness_anchor == [0; 32]
        || selection.seed_digest == [0; 32]
        || selection.sortition_digest == [0; 32]
        || selection.selected_at_unix_ms == 0
    {
        return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
            "panel selection contains inert provenance".to_owned(),
        ));
    }
    let (jurors, waitlist, seed_digest, sortition_digest) = sorafs_moderation_select_panel_v1(
        appeal.intake_digest,
        appeal.pop_snapshot_digest,
        selection.randomness_anchor,
        &entry.eligibility,
        appeal.intake.panel_size,
        appeal.intake.waitlist_size,
        appeal.intake.quorum,
    )
    .map_err(|error| {
        ModerationOrchestratorError::InvalidFinalizedSnapshot(format!(
            "persisted panel selection cannot be reproduced: {error}"
        ))
    })?;
    if selection.jurors != jurors
        || selection.waitlist != waitlist
        || selection.seed_digest != seed_digest
        || selection.sortition_digest != sortition_digest
        || appeal
            .accepted_jurors
            .iter()
            .any(|accepted| !selection.jurors.contains(accepted))
    {
        return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
            "panel selection, digest, or assignment acceptance is not deterministic".to_owned(),
        ));
    }

    let mut waitlist_iter = selection.waitlist.iter();
    let mut expected_replacements = Vec::new();
    let mut failover_exhausted = false;
    for primary in &selection.jurors {
        if appeal.accepted_jurors.contains(primary) {
            continue;
        }
        let Some(replacement) = waitlist_iter.next() else {
            failover_exhausted = true;
            break;
        };
        expected_replacements.push((primary.to_string(), replacement.to_string()));
    }
    let replacements = appeal
        .replacements
        .iter()
        .map(|replacement| {
            (
                replacement.absent_juror.to_string(),
                replacement.replacement_juror.to_string(),
            )
        })
        .collect::<Vec<_>>();
    if replacements != expected_replacements
        && matches!(
            appeal.status,
            ModerationAppealStatusV1::BallotOpen
                | ModerationAppealStatusV1::FailoverExhausted
                | ModerationAppealStatusV1::Finalized
        )
    {
        return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
            "persisted no-show failover is not deterministic".to_owned(),
        ));
    }

    let lifecycle_valid = match appeal.status {
        ModerationAppealStatusV1::AwaitingAcceptance => {
            appeal.replacements.is_empty()
                && appeal.activated_at_unix_ms.is_none()
                && appeal.finalized_at_unix_ms.is_none()
        }
        ModerationAppealStatusV1::BallotOpen => {
            !failover_exhausted
                && appeal.activated_at_unix_ms.is_some()
                && appeal.finalized_at_unix_ms.is_none()
        }
        ModerationAppealStatusV1::FailoverExhausted => {
            failover_exhausted
                && appeal.activated_at_unix_ms.is_none()
                && appeal.finalized_at_unix_ms.is_none()
        }
        ModerationAppealStatusV1::Finalized => {
            !failover_exhausted
                && appeal.activated_at_unix_ms.is_some()
                && appeal.finalized_at_unix_ms.is_some()
        }
        ModerationAppealStatusV1::RegisteringJurors
        | ModerationAppealStatusV1::InsufficientEligiblePool => false,
    };
    if !lifecycle_valid {
        return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
            "appeal status, selection, activation, or finalization is inconsistent".to_owned(),
        ));
    }
    Ok(())
}

fn validate_case_against_appeal(
    case: &ModerationFinalizedCaseViewV1,
    appeal: &ModerationFinalizedAppealViewV1,
) -> Result<(), ModerationOrchestratorError> {
    let intake = &appeal.appeal.intake;
    let spec = &case.case.spec;
    let selection = appeal.appeal.selection.as_ref().ok_or_else(|| {
        ModerationOrchestratorError::InvalidFinalizedSnapshot(
            "activated appeal has no panel selection".to_owned(),
        )
    })?;
    let replacement_map = appeal
        .appeal
        .replacements
        .iter()
        .map(|replacement| {
            (
                replacement.absent_juror.to_string(),
                replacement.replacement_juror.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let expected_jurors = selection
        .jurors
        .iter()
        .map(|primary| {
            replacement_map
                .get(&primary.to_string())
                .cloned()
                .unwrap_or_else(|| primary.clone())
        })
        .collect::<Vec<_>>();
    if spec.context.case_id != intake.case_id
        || spec.round_id != intake.round_id
        || spec.context.evidence_bundle_digest != intake.evidence_bundle_digest
        || spec.context.appeal_finance_config_version != intake.appeal_finance_config_version
        || spec.context.policy_reference != intake.policy_reference
        || spec.context.evidence_uri != intake.evidence_uri
        || spec.jurors != expected_jurors
        || spec.quorum != intake.quorum
        || spec.commit_deadline_unix_ms != intake.commit_deadline_unix_ms
        || spec.challenge_deadline_unix_ms != intake.challenge_deadline_unix_ms
        || spec.reveal_deadline_unix_ms != intake.reveal_deadline_unix_ms
        || spec.policy_digest != intake.policy_digest
        || case.case.policy != appeal.appeal.policy
        || Some(case.case.opened_at_unix_ms) != appeal.appeal.activated_at_unix_ms
    {
        return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
            "activated case differs from its immutable appeal state".to_owned(),
        ));
    }
    match (&appeal.appeal.status, &case.outcome) {
        (ModerationAppealStatusV1::BallotOpen, None) => {}
        (ModerationAppealStatusV1::Finalized, Some(outcome))
            if Some(outcome.finalized_at_unix_ms) == appeal.appeal.finalized_at_unix_ms => {}
        _ => {
            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "case terminal state differs from its appeal lifecycle".to_owned(),
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionEffect {
    Absent,
    Exact,
    Conflict,
}

fn action_effect(
    snapshot: &ModerationFinalizedLedgerSnapshotV1,
    authority: &AccountId,
    action: &ModerationNativeActionV1,
) -> Result<ActionEffect, ModerationOrchestratorError> {
    match action {
        ModerationNativeActionV1::SetPolicy(value) => {
            Ok(snapshot
                .policy
                .as_ref()
                .map_or(ActionEffect::Absent, |record| {
                    if &record.policy == value.policy() {
                        ActionEffect::Exact
                    } else if record.policy.revision >= value.policy().revision {
                        ActionEffect::Conflict
                    } else {
                        ActionEffect::Absent
                    }
                }))
        }
        ModerationNativeActionV1::SubmitAppeal(value) => Ok(snapshot
            .appeal(&value.intake().case_id, &value.intake().round_id)
            .map_or(ActionEffect::Absent, |entry| {
                if &entry.appeal.intake == value.intake() {
                    ActionEffect::Exact
                } else {
                    ActionEffect::Conflict
                }
            })),
        ModerationNativeActionV1::RegisterEligibility(value) => {
            let proof_digest = pop_proof_payload_digest(value.membership_proof_payload());
            Ok(snapshot
                .appeal(value.case_id(), value.round_id())
                .and_then(|entry| {
                    entry
                        .eligibility
                        .iter()
                        .find(|record| &record.juror == authority)
                })
                .map_or(ActionEffect::Absent, |record| {
                    if record.proof_digest == proof_digest {
                        ActionEffect::Exact
                    } else {
                        ActionEffect::Conflict
                    }
                }))
        }
        ModerationNativeActionV1::FinalizeSortition(value) => Ok(snapshot
            .appeal(value.case_id(), value.round_id())
            .map_or(ActionEffect::Absent, |entry| {
                if entry.appeal.pop_snapshot_digest != *value.pop_snapshot_digest() {
                    return ActionEffect::Conflict;
                }
                if entry.appeal.status == ModerationAppealStatusV1::InsufficientEligiblePool {
                    return if value.proposed_jurors().is_empty()
                        && value.proposed_waitlist().is_empty()
                    {
                        ActionEffect::Exact
                    } else {
                        ActionEffect::Conflict
                    };
                }
                entry
                    .appeal
                    .selection
                    .as_ref()
                    .map_or(ActionEffect::Absent, |selection| {
                        if &selection.randomness_anchor == value.randomness_anchor()
                            && &selection.jurors == value.proposed_jurors()
                            && &selection.waitlist == value.proposed_waitlist()
                        {
                            ActionEffect::Exact
                        } else {
                            ActionEffect::Conflict
                        }
                    })
            })),
        ModerationNativeActionV1::AcceptAssignment(value) => Ok(snapshot
            .appeal(value.case_id(), value.round_id())
            .map_or(ActionEffect::Absent, |entry| {
                if entry.appeal.selection.as_ref().is_some_and(|selection| {
                    selection.sortition_digest != *value.sortition_digest()
                }) {
                    ActionEffect::Conflict
                } else if entry.appeal.accepted_jurors.contains(authority) {
                    ActionEffect::Exact
                } else {
                    ActionEffect::Absent
                }
            })),
        ModerationNativeActionV1::ActivateCase(value) => Ok(snapshot
            .appeal(value.case_id(), value.round_id())
            .map_or(ActionEffect::Absent, |entry| {
                let Some(selection) = entry.appeal.selection.as_ref() else {
                    return ActionEffect::Absent;
                };
                if selection.sortition_digest != *value.sortition_digest() {
                    return ActionEffect::Conflict;
                }
                if matches!(
                    entry.appeal.status,
                    ModerationAppealStatusV1::BallotOpen
                        | ModerationAppealStatusV1::FailoverExhausted
                        | ModerationAppealStatusV1::Finalized
                ) {
                    ActionEffect::Exact
                } else {
                    ActionEffect::Absent
                }
            })),
        ModerationNativeActionV1::SubmitCommit(value) => {
            let commit = decode_canonical_commit(value.commit_payload())?;
            Ok(snapshot
                .case(&commit.context.case_id, &commit.round_id)
                .and_then(|entry| {
                    entry
                        .commits
                        .iter()
                        .find(|record| &record.juror == authority)
                })
                .map_or(ActionEffect::Absent, |record| {
                    let Ok(mut stored) = decode_canonical_commit(&record.canonical_commit) else {
                        return ActionEffect::Conflict;
                    };
                    stored.committed_at_unix_ms = 0;
                    if stored == commit {
                        ActionEffect::Exact
                    } else {
                        ActionEffect::Conflict
                    }
                }))
        }
        ModerationNativeActionV1::RaiseChallenge(value) => Ok(snapshot
            .case(value.case_id(), value.round_id())
            .and_then(|entry| {
                entry
                    .challenges
                    .iter()
                    .find(|record| &record.challenge_id == value.challenge_id())
            })
            .map_or(ActionEffect::Absent, |record| {
                if record.challenger == *authority
                    && &record.kind == value.kind()
                    && &record.target_juror == value.target_juror()
                    && &record.evidence_digest == value.evidence_digest()
                    && &record.reason == value.reason()
                {
                    ActionEffect::Exact
                } else {
                    ActionEffect::Conflict
                }
            })),
        ModerationNativeActionV1::ResolveChallenge(value) => Ok(snapshot
            .case(value.case_id(), value.round_id())
            .and_then(|entry| {
                entry
                    .challenges
                    .iter()
                    .find(|record| &record.challenge_id == value.challenge_id())
            })
            .map_or(ActionEffect::Absent, |record| match record.decision {
                Some(decision) if &decision == value.decision() => ActionEffect::Exact,
                Some(_) => ActionEffect::Conflict,
                None => ActionEffect::Absent,
            })),
        ModerationNativeActionV1::SubmitReveal(value) => {
            let reveal = decode_canonical_reveal(value.reveal_payload())?;
            Ok(snapshot
                .case(&reveal.context.case_id, &reveal.round_id)
                .and_then(|entry| {
                    entry
                        .reveals
                        .iter()
                        .find(|record| &record.juror == authority)
                })
                .map_or(ActionEffect::Absent, |record| {
                    let Ok(mut stored) = decode_canonical_reveal(&record.canonical_reveal) else {
                        return ActionEffect::Conflict;
                    };
                    stored.revealed_at_unix_ms = 0;
                    if stored == reveal {
                        ActionEffect::Exact
                    } else {
                        ActionEffect::Conflict
                    }
                }))
        }
        ModerationNativeActionV1::FinalizeCase(value) => Ok(snapshot
            .case(value.case_id(), value.round_id())
            .and_then(|entry| entry.outcome.as_ref())
            .map_or(ActionEffect::Absent, |_| ActionEffect::Exact)),
    }
}

fn validate_checkpoint(
    state: &ModerationOrchestratorCheckpointV1,
    config: &ModerationOrchestratorConfigV1,
) -> Result<(), ModerationOrchestratorError> {
    if state.version != MODERATION_ORCHESTRATOR_CHECKPOINT_VERSION_V1 {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "unsupported checkpoint version".to_owned(),
        ));
    }
    if state.operations.len() > config.max_idempotency_records
        || state.outbox.len() > config.max_outbox_entries
        || state.dead_letters.len() > config.max_idempotency_records
        || state
            .pending_handoffs
            .len()
            .saturating_add(state.completed_handoffs.len())
            > config.max_handoffs
    {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "checkpoint exceeds configured retention bounds".to_owned(),
        ));
    }
    match (
        state.finalized_snapshot.as_ref(),
        state.finalized_snapshot_digest,
    ) {
        (None, None) => {}
        (Some(snapshot), Some(digest)) => {
            validate_finalized_snapshot(snapshot, config)?;
            if finalized_snapshot_digest(snapshot)? != digest {
                return Err(ModerationOrchestratorError::CheckpointCorrupt(
                    "finalized snapshot digest mismatch".to_owned(),
                ));
            }
        }
        _ => {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "snapshot and snapshot digest presence mismatch".to_owned(),
            ));
        }
    }
    let mut operations = BTreeSet::new();
    for operation in &state.operations {
        if operation.operation_id == [0; 32]
            || operation.action_digest == [0; 32]
            || !operations.insert(operation.operation_id)
        {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "invalid or duplicate operation tombstone".to_owned(),
            ));
        }
    }
    let mut outbox = BTreeSet::new();
    for entry in &state.outbox {
        if !outbox.insert(entry.operation_id)
            || !operations.contains(&entry.operation_id)
            || entry.action_digest != entry.action.action_digest()?
            || entry.request_binding_digest == [0; 32]
            || entry.attempts > config.max_submit_attempts
        {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "invalid, duplicate, or orphaned outbox entry".to_owned(),
            ));
        }
        entry.action.validate_authority(&entry.authority)?;
        if entry.action.operation_id(&entry.authority)? != entry.operation_id {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "outbox semantic identity mismatch".to_owned(),
            ));
        }
    }
    let mut handoffs = state
        .completed_handoffs
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if handoffs.len() != state.completed_handoffs.len() {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "duplicate completed handoff".to_owned(),
        ));
    }
    for entry in &state.pending_handoffs {
        if entry.handoff.handoff_id == [0; 32]
            || !handoffs.insert(entry.handoff.handoff_id)
            || entry.attempts > config.max_submit_attempts
        {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "invalid or duplicate pending handoff".to_owned(),
            ));
        }
    }
    Ok(())
}

fn persist_checkpoint(
    config: &ModerationOrchestratorConfigV1,
    state: &mut ModerationOrchestratorCheckpointV1,
) -> Result<(), ModerationOrchestratorError> {
    state.generation = state
        .generation
        .checked_add(1)
        .ok_or(ModerationOrchestratorError::GenerationOverflow)?;
    validate_checkpoint(state, config)?;
    let bytes = norito::to_bytes(state).map_err(|error| {
        ModerationOrchestratorError::CheckpointIo(format!("encode checkpoint: {error}"))
    })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > config.checkpoint_max_bytes {
        return Err(ModerationOrchestratorError::ResourceExhausted {
            resource: "checkpoint bytes",
            limit: usize::try_from(config.checkpoint_max_bytes).unwrap_or(usize::MAX),
        });
    }
    write_atomic(&config.checkpoint_path, &bytes)?;
    let persisted = read_bounded_file(&config.checkpoint_path, config.checkpoint_max_bytes)?
        .ok_or_else(|| {
            ModerationOrchestratorError::CheckpointDurabilityUncertain(
                "checkpoint disappeared after atomic rename".to_owned(),
            )
        })?;
    if persisted != bytes {
        return Err(ModerationOrchestratorError::CheckpointDurabilityUncertain(
            "checkpoint bytes changed after atomic rename".to_owned(),
        ));
    }
    Ok(())
}

fn snapshot_cursor(
    state: &ModerationOrchestratorCheckpointV1,
) -> Result<ModerationFinalizedCursorV1, ModerationOrchestratorError> {
    state
        .finalized_snapshot
        .as_ref()
        .map(ModerationFinalizedLedgerSnapshotV1::anchor)
        .ok_or(ModerationOrchestratorError::FinalizedReaderUnavailable)
}

fn find_operation(
    state: &ModerationOrchestratorCheckpointV1,
    operation_id: [u8; 32],
) -> Option<&StoredOperationV1> {
    state
        .operations
        .iter()
        .find(|record| record.operation_id == operation_id)
}

fn ensure_operation_capacity(
    state: &ModerationOrchestratorCheckpointV1,
    config: &ModerationOrchestratorConfigV1,
) -> Result<(), ModerationOrchestratorError> {
    if state.operations.len() >= config.max_idempotency_records {
        return Err(ModerationOrchestratorError::ResourceExhausted {
            resource: "operation tombstones",
            limit: config.max_idempotency_records,
        });
    }
    Ok(())
}

fn ensure_outbox_capacity(
    state: &ModerationOrchestratorCheckpointV1,
    config: &ModerationOrchestratorConfigV1,
) -> Result<(), ModerationOrchestratorError> {
    if state.outbox.len() >= config.max_outbox_entries {
        return Err(ModerationOrchestratorError::ResourceExhausted {
            resource: "native transaction outbox",
            limit: config.max_outbox_entries,
        });
    }
    Ok(())
}

fn ensure_dead_letter_capacity(
    state: &ModerationOrchestratorCheckpointV1,
    config: &ModerationOrchestratorConfigV1,
    additional: usize,
) -> Result<(), ModerationOrchestratorError> {
    if state.dead_letters.len().saturating_add(additional) > config.max_idempotency_records {
        return Err(ModerationOrchestratorError::ResourceExhausted {
            resource: "dead letters",
            limit: config.max_idempotency_records,
        });
    }
    Ok(())
}

fn finalized_snapshot_digest(
    snapshot: &ModerationFinalizedLedgerSnapshotV1,
) -> Result<[u8; 32], ModerationOrchestratorError> {
    let bytes = norito::to_bytes(snapshot).map_err(|error| {
        ModerationOrchestratorError::InvalidFinalizedSnapshot(format!("encode snapshot: {error}"))
    })?;
    Ok(domain_hash(SNAPSHOT_DIGEST_DOMAIN_V1, &[&bytes]))
}

fn decode_canonical_commit(
    payload: &[u8],
) -> Result<SoraFsModerationBallotCommitV1, ModerationOrchestratorError> {
    decode_canonical_payload(payload, "commit")
}

fn decode_canonical_reveal(
    payload: &[u8],
) -> Result<SoraFsModerationBallotRevealV1, ModerationOrchestratorError> {
    decode_canonical_payload(payload, "reveal")
}

fn decode_canonical_payload<T>(
    payload: &[u8],
    label: &'static str,
) -> Result<T, ModerationOrchestratorError>
where
    T: NoritoSerialize,
    for<'de> T: NoritoDeserialize<'de>,
{
    if payload.is_empty() || payload.len() > MODERATION_NATIVE_INSTRUCTION_MAX_BYTES_V1 {
        return Err(ModerationOrchestratorError::InvalidAction(format!(
            "{label} payload is empty or oversized"
        )));
    }
    let decoded = decode_from_bytes_with_limits::<T>(payload, ACTION_LIMITS).map_err(|error| {
        ModerationOrchestratorError::InvalidAction(format!("decode {label} payload: {error}"))
    })?;
    let canonical = norito::to_bytes(&decoded).map_err(|error| {
        ModerationOrchestratorError::InvalidAction(format!("encode {label} payload: {error}"))
    })?;
    if canonical != payload {
        return Err(ModerationOrchestratorError::InvalidAction(format!(
            "{label} payload is not canonical Norito"
        )));
    }
    Ok(decoded)
}

fn canonical_account(
    value: &str,
    field: &'static str,
) -> Result<AccountId, ModerationOrchestratorError> {
    let parsed = AccountId::parse_encoded(value).map_err(|error| {
        ModerationOrchestratorError::InvalidAction(format!("invalid {field}: {error}"))
    })?;
    if parsed.canonical() != value {
        return Err(ModerationOrchestratorError::InvalidAction(format!(
            "{field} is not canonical"
        )));
    }
    Ok(ParsedAccountId::into_account_id(parsed))
}

fn validate_scope(case_id: &str, round_id: &str) -> Result<(), ModerationOrchestratorError> {
    if !is_canonical_moderation_identifier_v1(case_id)
        || !is_canonical_moderation_identifier_v1(round_id)
    {
        return Err(ModerationOrchestratorError::InvalidAction(
            "case or round identifier is not bounded canonical ASCII".to_owned(),
        ));
    }
    Ok(())
}

fn require_nonzero_digest(
    digest: [u8; 32],
    field: &'static str,
) -> Result<(), ModerationOrchestratorError> {
    if digest == [0; 32] {
        return Err(ModerationOrchestratorError::InvalidAction(format!(
            "{field} must be non-zero"
        )));
    }
    Ok(())
}

fn require_exact_authority(
    authenticated: &AccountId,
    native: &AccountId,
    action: &'static str,
) -> Result<(), ModerationOrchestratorError> {
    if authenticated != native {
        return Err(ModerationOrchestratorError::AuthorityMismatch {
            action,
            authenticated: authenticated.to_string(),
            native: native.to_string(),
        });
    }
    Ok(())
}

fn push_scope(
    output: &mut Vec<u8>,
    case_id: &str,
    round_id: &str,
) -> Result<(), ModerationOrchestratorError> {
    push_part(output, case_id.as_bytes())?;
    push_part(output, round_id.as_bytes())
}

fn push_part(output: &mut Vec<u8>, part: &[u8]) -> Result<(), ModerationOrchestratorError> {
    let len = u64::try_from(part.len()).map_err(|_| {
        ModerationOrchestratorError::InvalidAction(
            "operation identity part is too large".to_owned(),
        )
    })?;
    output.extend_from_slice(&len.to_le_bytes());
    output.extend_from_slice(part);
    Ok(())
}

fn domain_hash(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    for part in parts {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    *hasher.finalize().as_bytes()
}

fn pop_proof_payload_digest(payload: &[u8]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(POP_PROOF_PAYLOAD_DIGEST_DOMAIN_V1);
    hasher.update(payload);
    *hasher.finalize().as_bytes()
}

fn appeal_key(entry: &ModerationFinalizedAppealViewV1) -> (String, String) {
    (
        entry.appeal.intake.case_id.clone(),
        entry.appeal.intake.round_id.clone(),
    )
}

fn case_key(entry: &ModerationFinalizedCaseViewV1) -> (String, String) {
    (
        entry.case.spec.context.case_id.clone(),
        entry.case.spec.round_id.clone(),
    )
}

fn require_strict_key_order<T: Ord + Clone>(
    previous: &mut Option<T>,
    current: &T,
    label: &'static str,
) -> Result<(), ModerationOrchestratorError> {
    if previous.as_ref().is_some_and(|value| value >= current) {
        return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
            format!("{label} are not strictly sorted and unique"),
        ));
    }
    *previous = Some(current.clone());
    Ok(())
}

fn require_record_scope(
    case_id: &str,
    round_id: &str,
    expected: &(String, String),
    label: &'static str,
) -> Result<(), ModerationOrchestratorError> {
    if case_id != expected.0 || round_id != expected.1 {
        return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
            format!("{label} record escapes its case scope"),
        ));
    }
    Ok(())
}

fn handoff_label(kind: ModerationTerminalHandoffKindV1) -> &'static str {
    match kind {
        ModerationTerminalHandoffKindV1::Settlement => "terminal_settlement",
        ModerationTerminalHandoffKindV1::Publication => "terminal_publication",
    }
}

fn checkpoint_decode_limits(max_bytes: u64) -> Result<DecodeLimits, ModerationOrchestratorError> {
    let max_bytes = usize::try_from(max_bytes).map_err(|_| {
        ModerationOrchestratorError::InvalidConfiguration(
            "checkpoint byte limit does not fit usize".to_owned(),
        )
    })?;
    Ok(DecodeLimits::new(
        512,
        max_bytes,
        262_144,
        max_bytes.saturating_mul(2),
        128,
    ))
}

fn ensure_secure_parent(path: &Path) -> Result<(), ModerationOrchestratorError> {
    let parent = path.parent().ok_or_else(|| {
        ModerationOrchestratorError::InvalidConfiguration(
            "checkpoint path has no parent".to_owned(),
        )
    })?;
    let mut current = PathBuf::new();
    for component in parent.components() {
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(ModerationOrchestratorError::CheckpointIo(format!(
                        "checkpoint ancestor `{}` must be a real directory",
                        current.display()
                    )));
                }
                #[cfg(unix)]
                if metadata.permissions().mode() & 0o022 != 0
                    && metadata.permissions().mode() & 0o1000 == 0
                {
                    return Err(ModerationOrchestratorError::CheckpointIo(format!(
                        "checkpoint ancestor `{}` is writable by other users",
                        current.display()
                    )));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let mut builder = fs::DirBuilder::new();
                #[cfg(unix)]
                builder.mode(0o700);
                builder.create(&current).map_err(|error| {
                    ModerationOrchestratorError::CheckpointIo(format!(
                        "create checkpoint directory `{}`: {error}",
                        current.display()
                    ))
                })?;
            }
            Err(error) => {
                return Err(ModerationOrchestratorError::CheckpointIo(format!(
                    "inspect checkpoint ancestor `{}`: {error}",
                    current.display()
                )));
            }
        }
    }
    Ok(())
}

fn read_bounded_file(
    path: &Path,
    max_bytes: u64,
) -> Result<Option<Vec<u8>>, ModerationOrchestratorError> {
    let before = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(ModerationOrchestratorError::CheckpointIo(format!(
                "inspect checkpoint: {error}"
            )));
        }
    };
    validate_checkpoint_metadata(path, &before)?;
    if before.len() > max_bytes {
        return Err(ModerationOrchestratorError::ResourceExhausted {
            resource: "checkpoint bytes",
            limit: usize::try_from(max_bytes).unwrap_or(usize::MAX),
        });
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(SAFE_OPEN_FLAGS);
    let mut file = options.open(path).map_err(|error| {
        ModerationOrchestratorError::CheckpointIo(format!("open checkpoint: {error}"))
    })?;
    let opened = file.metadata().map_err(|error| {
        ModerationOrchestratorError::CheckpointIo(format!("inspect opened checkpoint: {error}"))
    })?;
    validate_checkpoint_metadata(path, &opened)?;
    #[cfg(unix)]
    if before.dev() != opened.dev() || before.ino() != opened.ino() {
        return Err(ModerationOrchestratorError::CheckpointIo(
            "checkpoint changed identity while opening".to_owned(),
        ));
    }
    let mut bytes = Vec::with_capacity(
        usize::try_from(before.len())
            .unwrap_or(usize::MAX)
            .min(usize::try_from(max_bytes).unwrap_or(usize::MAX)),
    );
    Read::by_ref(&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| {
            ModerationOrchestratorError::CheckpointIo(format!("read checkpoint: {error}"))
        })?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(ModerationOrchestratorError::ResourceExhausted {
            resource: "checkpoint bytes",
            limit: usize::try_from(max_bytes).unwrap_or(usize::MAX),
        });
    }
    let after = fs::symlink_metadata(path).map_err(|error| {
        ModerationOrchestratorError::CheckpointIo(format!("reinspect checkpoint: {error}"))
    })?;
    validate_checkpoint_metadata(path, &after)?;
    #[cfg(unix)]
    if before.dev() != after.dev() || before.ino() != after.ino() {
        return Err(ModerationOrchestratorError::CheckpointIo(
            "checkpoint changed identity during read".to_owned(),
        ));
    }
    Ok(Some(bytes))
}

fn validate_checkpoint_metadata(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<(), ModerationOrchestratorError> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ModerationOrchestratorError::CheckpointIo(format!(
            "checkpoint `{}` must be a non-symlink regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        if metadata.nlink() != 1 {
            return Err(ModerationOrchestratorError::CheckpointIo(format!(
                "checkpoint `{}` must have exactly one hard link",
                path.display()
            )));
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(ModerationOrchestratorError::CheckpointIo(format!(
                "checkpoint `{}` must not be accessible by group or other users",
                path.display()
            )));
        }
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), ModerationOrchestratorError> {
    ensure_secure_parent(path)?;
    if let Ok(metadata) = fs::symlink_metadata(path) {
        validate_checkpoint_metadata(path, &metadata)?;
    }
    let parent = path.parent().ok_or_else(|| {
        ModerationOrchestratorError::CheckpointIo("checkpoint path has no parent".to_owned())
    })?;
    let parent_before = fs::symlink_metadata(parent).map_err(|error| {
        ModerationOrchestratorError::CheckpointIo(format!(
            "inspect checkpoint parent before write: {error}"
        ))
    })?;
    if parent_before.file_type().is_symlink() || !parent_before.is_dir() {
        return Err(ModerationOrchestratorError::CheckpointIo(
            "checkpoint parent must remain a real directory".to_owned(),
        ));
    }
    let mut directory_options = OpenOptions::new();
    directory_options.read(true);
    #[cfg(unix)]
    directory_options.custom_flags(SAFE_OPEN_FLAGS);
    let parent_directory = directory_options.open(parent).map_err(|error| {
        ModerationOrchestratorError::CheckpointIo(format!(
            "open checkpoint parent before write: {error}"
        ))
    })?;
    let opened_parent = parent_directory.metadata().map_err(|error| {
        ModerationOrchestratorError::CheckpointIo(format!(
            "inspect opened checkpoint parent before write: {error}"
        ))
    })?;
    if !opened_parent.is_dir() {
        return Err(ModerationOrchestratorError::CheckpointIo(
            "opened checkpoint parent is not a directory".to_owned(),
        ));
    }
    #[cfg(unix)]
    if parent_before.dev() != opened_parent.dev() || parent_before.ino() != opened_parent.ino() {
        return Err(ModerationOrchestratorError::CheckpointIo(
            "checkpoint parent changed identity while opening".to_owned(),
        ));
    }
    let mut random_nonce = [0_u8; 16];
    OsRng.try_fill_bytes(&mut random_nonce).map_err(|error| {
        ModerationOrchestratorError::CheckpointIo(format!(
            "generate atomic checkpoint nonce: {error}"
        ))
    })?;
    let counter = CHECKPOINT_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            ModerationOrchestratorError::CheckpointIo(
                "checkpoint file name is not UTF-8".to_owned(),
            )
        })?;
    let temp = parent.join(format!(
        ".{file_name}.{}.{counter}.{}.tmp",
        std::process::id(),
        hex::encode(random_nonce)
    ));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            options.mode(0o600);
            options.custom_flags(SAFE_OPEN_FLAGS);
        }
        let mut file = options.open(&temp).map_err(|error| {
            ModerationOrchestratorError::CheckpointIo(format!("create atomic checkpoint: {error}"))
        })?;
        let opened_temp = file.metadata().map_err(|error| {
            ModerationOrchestratorError::CheckpointIo(format!("inspect atomic checkpoint: {error}"))
        })?;
        validate_checkpoint_metadata(&temp, &opened_temp)?;
        file.write_all(bytes).map_err(|error| {
            ModerationOrchestratorError::CheckpointIo(format!("write checkpoint: {error}"))
        })?;
        file.sync_all().map_err(|error| {
            ModerationOrchestratorError::CheckpointIo(format!("sync checkpoint: {error}"))
        })?;
        let named_temp = fs::symlink_metadata(&temp).map_err(|error| {
            ModerationOrchestratorError::CheckpointIo(format!(
                "reinspect atomic checkpoint before rename: {error}"
            ))
        })?;
        validate_checkpoint_metadata(&temp, &named_temp)?;
        #[cfg(unix)]
        if opened_temp.dev() != named_temp.dev() || opened_temp.ino() != named_temp.ino() {
            return Err(ModerationOrchestratorError::CheckpointIo(
                "atomic checkpoint changed identity before rename".to_owned(),
            ));
        }
        let parent_before_rename = fs::symlink_metadata(parent).map_err(|error| {
            ModerationOrchestratorError::CheckpointIo(format!(
                "reinspect checkpoint parent before rename: {error}"
            ))
        })?;
        if parent_before_rename.file_type().is_symlink() || !parent_before_rename.is_dir() {
            return Err(ModerationOrchestratorError::CheckpointIo(
                "checkpoint parent changed to a non-directory before rename".to_owned(),
            ));
        }
        #[cfg(unix)]
        if opened_parent.dev() != parent_before_rename.dev()
            || opened_parent.ino() != parent_before_rename.ino()
        {
            return Err(ModerationOrchestratorError::CheckpointIo(
                "checkpoint parent changed identity before rename".to_owned(),
            ));
        }
        drop(file);
        if let Ok(metadata) = fs::symlink_metadata(path) {
            validate_checkpoint_metadata(path, &metadata)?;
        }
        fs::rename(&temp, path).map_err(|error| {
            ModerationOrchestratorError::CheckpointIo(format!("replace checkpoint: {error}"))
        })?;
        let committed = fs::symlink_metadata(path).map_err(|error| {
            ModerationOrchestratorError::CheckpointDurabilityUncertain(format!(
                "inspect committed checkpoint after rename: {error}"
            ))
        })?;
        validate_checkpoint_metadata(path, &committed).map_err(|error| {
            ModerationOrchestratorError::CheckpointDurabilityUncertain(error.to_string())
        })?;
        #[cfg(unix)]
        if opened_temp.dev() != committed.dev() || opened_temp.ino() != committed.ino() {
            return Err(ModerationOrchestratorError::CheckpointDurabilityUncertain(
                "committed checkpoint changed identity after rename".to_owned(),
            ));
        }
        let parent_after = fs::symlink_metadata(parent).map_err(|error| {
            ModerationOrchestratorError::CheckpointDurabilityUncertain(format!(
                "inspect checkpoint parent after rename: {error}"
            ))
        })?;
        if parent_after.file_type().is_symlink() || !parent_after.is_dir() {
            return Err(ModerationOrchestratorError::CheckpointDurabilityUncertain(
                "checkpoint parent changed to a non-directory".to_owned(),
            ));
        }
        #[cfg(unix)]
        if opened_parent.dev() != parent_after.dev() || opened_parent.ino() != parent_after.ino() {
            return Err(ModerationOrchestratorError::CheckpointDurabilityUncertain(
                "checkpoint parent changed identity during atomic replacement".to_owned(),
            ));
        }
        parent_directory.sync_all().map_err(|error| {
            ModerationOrchestratorError::CheckpointDurabilityUncertain(format!(
                "sync checkpoint parent after rename: {error}"
            ))
        })
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

/// Orchestrator failure.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ModerationOrchestratorError {
    /// Non-secret orchestration configuration is invalid.
    #[error("invalid moderation orchestrator configuration: {0}")]
    InvalidConfiguration(String),
    /// Native action is malformed or noncanonical.
    #[error("invalid native moderation action: {0}")]
    InvalidAction(String),
    /// Authenticated principal differs from the authority encoded by the action.
    #[error(
        "moderation authority mismatch for {action}: authenticated `{authenticated}`, native `{native}`"
    )]
    AuthorityMismatch {
        /// Action label.
        action: &'static str,
        /// Verified principal.
        authenticated: String,
        /// Native payload authority.
        native: String,
    },
    /// Authenticated request binding is malformed or inert.
    #[error("invalid moderation authenticated request binding")]
    InvalidRequestBinding,
    /// A stable semantic identity was reused for different action bytes.
    #[error("moderation operation idempotency conflict for {}", hex::encode(.operation_id))]
    IdempotencyConflict {
        /// Stable semantic operation id.
        operation_id: [u8; 32],
    },
    /// Another finalized mutation already occupies the semantic identity.
    #[error("moderation finalized-state conflict for {}", hex::encode(.operation_id))]
    FinalizedConflict {
        /// Stable semantic operation id.
        operation_id: [u8; 32],
    },
    /// Finalized reader was absent or unavailable.
    #[error("moderation finalized snapshot reader is unavailable")]
    FinalizedReaderUnavailable,
    /// Finalized snapshot is malformed.
    #[error("invalid finalized moderation snapshot: {0}")]
    InvalidFinalizedSnapshot(String),
    /// Reader returned a lower finalized cursor.
    #[error("stale finalized moderation cursor: current {current}, observed {observed}")]
    StaleFinalizedCursor {
        /// Persisted height.
        current: u64,
        /// Reader height.
        observed: u64,
    },
    /// Same finalized height produced a different hash or snapshot.
    #[error("finalized moderation equivocation at height {height}")]
    FinalizedEquivocation {
        /// Equivocating height.
        height: u64,
    },
    /// A configured bound was exhausted.
    #[error("moderation {resource} exhausted configured limit {limit}")]
    ResourceExhausted {
        /// Bounded resource.
        resource: &'static str,
        /// Configured ceiling.
        limit: usize,
    },
    /// Durable state is corrupt.
    #[error("moderation checkpoint is corrupt: {0}")]
    CheckpointCorrupt(String),
    /// Checkpoint filesystem operation failed before commit.
    #[error("moderation checkpoint I/O failed: {0}")]
    CheckpointIo(String),
    /// Rename committed but durability could not be established.
    #[error("moderation checkpoint durability is uncertain: {0}")]
    CheckpointDurabilityUncertain(String),
    /// A prior checkpoint failure made the in-memory state unsafe to reuse.
    #[error("moderation orchestrator is fail-stopped after a checkpoint failure")]
    DurabilityFaulted,
    /// Durable generation overflowed.
    #[error("moderation checkpoint generation overflow")]
    GenerationOverflow,
    /// In-memory state lock is poisoned.
    #[error("moderation orchestrator state lock is poisoned")]
    StateLockPoisoned,
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        events::data::sorafs::SorafsModerationLedgerEvent,
        sorafs::{
            moderation::{
                SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1, SoraFsModerationBallotContextV1,
            },
            moderation_ledger::{
                MODERATION_APPEAL_INTAKE_VERSION_V1, MODERATION_LEDGER_CASE_VERSION_V1,
                MODERATION_LEDGER_POLICY_VERSION_V1, ModerationAppealIntakeV1,
                ModerationAppealRecordV1, ModerationCaseRecordV1, ModerationCaseSpecV1,
                ModerationJurorEligibilityClassV1, ModerationJurorEligibilityRecordV1,
                ModerationLedgerPolicyRecord, ModerationLedgerPolicyV1, ModerationLedgerStatusV1,
                ModerationNoShowKindV1, ModerationNoShowRecordV1, ModerationOutcomeKindV1,
                ModerationOutcomeRecordV1, ModerationPanelSelectionV1,
                ModerationPoPRegistrySnapshotV1, ModerationVoteCountsV1,
                sorafs_moderation_panel_roster_hash_v1,
            },
        },
    };
    use tempfile::TempDir;

    use super::*;

    #[derive(Debug)]
    struct MockSnapshotReader {
        snapshot: Mutex<ModerationFinalizedLedgerSnapshotV1>,
    }

    impl MockSnapshotReader {
        fn new(snapshot: ModerationFinalizedLedgerSnapshotV1) -> Self {
            Self {
                snapshot: Mutex::new(snapshot),
            }
        }

        fn replace(&self, snapshot: ModerationFinalizedLedgerSnapshotV1) {
            *self.snapshot.lock().expect("snapshot lock") = snapshot;
        }
    }

    impl ModerationFinalizedSnapshotReaderV1 for MockSnapshotReader {
        fn read_finalized_snapshot(
            &self,
            _max_cases: usize,
            _max_events: usize,
        ) -> Result<ModerationFinalizedLedgerSnapshotV1, ModerationSnapshotReadErrorV1> {
            Ok(self.snapshot.lock().expect("snapshot lock").clone())
        }
    }

    #[derive(Debug)]
    struct MockSubmitterState {
        calls: usize,
        actions: Vec<ModerationNativeActionV1>,
        operations: BTreeMap<[u8; 32], ModerationSubmissionLookupV1>,
        fallback: ModerationSubmissionLookupV1,
        failure: Option<ModerationSubmissionFailureV1>,
        ambiguous_is_applied: bool,
    }

    #[derive(Debug)]
    struct MockSubmitter {
        state: Mutex<MockSubmitterState>,
    }

    impl MockSubmitter {
        fn new(fallback: ModerationSubmissionLookupV1) -> Self {
            Self {
                state: Mutex::new(MockSubmitterState {
                    calls: 0,
                    actions: Vec::new(),
                    operations: BTreeMap::new(),
                    fallback,
                    failure: None,
                    ambiguous_is_applied: false,
                }),
            }
        }

        fn ambiguous_applied(fallback: ModerationSubmissionLookupV1) -> Self {
            Self {
                state: Mutex::new(MockSubmitterState {
                    calls: 0,
                    actions: Vec::new(),
                    operations: BTreeMap::new(),
                    fallback,
                    failure: Some(ModerationSubmissionFailureV1::Ambiguous),
                    ambiguous_is_applied: true,
                }),
            }
        }

        fn calls(&self) -> usize {
            self.state.lock().expect("submitter lock").calls
        }

        fn actions(&self) -> Vec<ModerationNativeActionV1> {
            self.state.lock().expect("submitter lock").actions.clone()
        }
    }

    impl ModerationTransactionSubmitterV1 for MockSubmitter {
        fn submit(
            &self,
            request: &ModerationTransactionRequestV1,
        ) -> Result<ModerationTransactionReceiptV1, ModerationSubmissionFailureV1> {
            let mut state = self.state.lock().expect("submitter lock");
            state.calls = state.calls.saturating_add(1);
            state.actions.push(request.action.clone());
            let transaction_id = [u8::try_from(state.calls).unwrap_or(u8::MAX); 32];
            if state.ambiguous_is_applied {
                state.operations.insert(
                    request.operation_id,
                    ModerationSubmissionLookupV1::Applied { transaction_id },
                );
            }
            if let Some(failure) = state.failure {
                return Err(failure);
            }
            state.operations.insert(
                request.operation_id,
                ModerationSubmissionLookupV1::Pending { transaction_id },
            );
            Ok(ModerationTransactionReceiptV1 {
                transaction_id,
                observed_finalized_height: request.baseline_finalized_height,
            })
        }

        fn lookup(
            &self,
            operation_id: [u8; 32],
            _transaction_id: Option<[u8; 32]>,
        ) -> ModerationSubmissionLookupV1 {
            let state = self.state.lock().expect("submitter lock");
            state
                .operations
                .get(&operation_id)
                .copied()
                .unwrap_or(state.fallback)
        }
    }

    #[derive(Debug, Default)]
    struct MockHandoffSink {
        delivered: Mutex<Vec<[u8; 32]>>,
    }

    impl MockHandoffSink {
        fn delivered(&self) -> Vec<[u8; 32]> {
            self.delivered.lock().expect("handoff sink lock").clone()
        }
    }

    impl ModerationTerminalHandoffSinkV1 for MockHandoffSink {
        fn deliver(
            &self,
            handoff: &ModerationTerminalHandoffV1,
        ) -> Result<(), ModerationHandoffFailureV1> {
            let mut delivered = self.delivered.lock().expect("handoff sink lock");
            if !delivered.contains(&handoff.handoff_id) {
                delivered.push(handoff.handoff_id);
            }
            Ok(())
        }
    }

    fn account(seed: u8) -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![seed.max(1); 32], Algorithm::Ed25519)
            .expect("deterministic account");
        AccountId::new(keypair.public_key().clone())
    }

    fn policy(revision: u64) -> ModerationLedgerPolicyV1 {
        ModerationLedgerPolicyV1 {
            version: MODERATION_LEDGER_POLICY_VERSION_V1,
            revision,
            predecessor_policy_digest: (revision > 1).then_some([0xA5; 32]),
            max_panel_size: 5,
            max_candidate_pool_size: 32,
            max_waitlist_size: 5,
            max_exclusions_per_case: 16,
            max_total_window_ms: 60_000,
            max_challenges_per_case: 4,
            missing_commit_penalty_points: 10,
            unrevealed_commit_penalty_points: 20,
        }
    }

    fn policy_action(policy: ModerationLedgerPolicyV1) -> ModerationNativeActionV1 {
        ModerationNativeActionV1::SetPolicy(SetSorafsModerationPolicy::new(policy))
    }

    fn empty_snapshot(height: u64, block_hash: [u8; 32]) -> ModerationFinalizedLedgerSnapshotV1 {
        ModerationFinalizedLedgerSnapshotV1 {
            version: MODERATION_FINALIZED_SNAPSHOT_VERSION_V1,
            finalized_height: height,
            finalized_block_hash: block_hash,
            policy: None,
            status: None,
            appeals: Vec::new(),
            cases: Vec::new(),
            events: Vec::new(),
        }
    }

    fn snapshot_with_policy(
        height: u64,
        block_hash: [u8; 32],
        policy: ModerationLedgerPolicyV1,
        authority: AccountId,
    ) -> ModerationFinalizedLedgerSnapshotV1 {
        let policy_digest = policy.digest().expect("policy digest");
        ModerationFinalizedLedgerSnapshotV1 {
            version: MODERATION_FINALIZED_SNAPSHOT_VERSION_V1,
            finalized_height: height,
            finalized_block_hash: block_hash,
            policy: Some(ModerationLedgerPolicyRecord {
                policy,
                policy_digest,
                activated_at_unix_ms: 1,
                activated_by: authority.clone(),
            }),
            status: Some(ModerationLedgerStatusV1 {
                updated_at_unix_ms: 1,
                ..ModerationLedgerStatusV1::default()
            }),
            appeals: Vec::new(),
            cases: Vec::new(),
            events: vec![ModerationFinalizedEventV1 {
                sequence: 1,
                block_height: height,
                block_hash,
                event_index: 0,
                event: SorafsModerationLedgerEvent::new(
                    SorafsModerationLedgerEventKind::PolicyActivated,
                    None,
                    None,
                    authority,
                    1,
                ),
            }],
        }
    }

    fn awaiting_acceptance_snapshot(
        height: u64,
        block_hash: [u8; 32],
        governance: AccountId,
    ) -> (ModerationFinalizedLedgerSnapshotV1, [u8; 32]) {
        let active_policy = policy(1);
        let policy_digest = active_policy.digest().expect("policy digest");
        let appellant = account(90);
        let pop_snapshot = ModerationPoPRegistrySnapshotV1 {
            issuer_policy_digest: [0x31; 32],
            commitment_root: [0x32; 32],
            commitment_tree_version: 1,
            revocation_root: [0x33; 32],
            revocation_list_version: 1,
            registry_audit_sequence: 1,
            registry_audit_head: [0x34; 32],
            captured_at_unix_ms: 2,
        };
        let pop_snapshot_digest = pop_snapshot.digest().expect("PoP snapshot digest");
        let intake = ModerationAppealIntakeV1 {
            version: MODERATION_APPEAL_INTAKE_VERSION_V1,
            case_id: "case-failover".to_owned(),
            round_id: "round-1".to_owned(),
            appellant: appellant.clone(),
            appealed_decision_digest: [0x41; 32],
            proof_token_digest: [0x42; 32],
            evidence_bundle_digest: [0x43; 32],
            appeal_deposit_lock_digest: [0x44; 32],
            appeal_finance_config_version: "finance-v1".to_owned(),
            policy_reference: "policy-v1".to_owned(),
            evidence_uri: Some("ipfs://case-failover".to_owned()),
            panel_size: 2,
            waitlist_size: 1,
            quorum: 1,
            exclusions: vec![appellant.clone()],
            registration_deadline_unix_ms: 20,
            acceptance_deadline_unix_ms: 30,
            commit_deadline_unix_ms: 40,
            challenge_deadline_unix_ms: 50,
            reveal_deadline_unix_ms: 60,
            policy_digest,
        };
        let intake_digest = intake.digest().expect("intake digest");
        let mut eligibility = (1_u8..=3)
            .map(|seed| ModerationJurorEligibilityRecordV1 {
                case_id: intake.case_id.clone(),
                round_id: intake.round_id.clone(),
                juror: account(seed),
                eligibility_class: ModerationJurorEligibilityClassV1::General,
                proof_digest: [seed.saturating_add(0x50); 32],
                nullifier: [seed.saturating_add(0x60); 32],
                pop_snapshot_digest,
                credential_expires_at_epoch: 1_000,
                registered_at_unix_ms: 10 + u64::from(seed),
            })
            .collect::<Vec<_>>();
        eligibility.sort_by_key(|record| record.juror.to_string());
        let eligible_jurors = eligibility
            .iter()
            .map(|record| record.juror.clone())
            .collect::<Vec<_>>();
        let randomness_anchor = [0x71; 32];
        let (jurors, waitlist, seed_digest, sortition_digest) = sorafs_moderation_select_panel_v1(
            intake_digest,
            pop_snapshot_digest,
            randomness_anchor,
            &eligibility,
            intake.panel_size,
            intake.waitlist_size,
            intake.quorum,
        )
        .expect("deterministic sortition");
        let selection = ModerationPanelSelectionV1 {
            randomness_anchor,
            seed_digest,
            jurors,
            waitlist,
            sortition_digest,
            selected_at_unix_ms: 21,
            selected_by: governance.clone(),
        };
        let appeal = ModerationAppealRecordV1 {
            intake,
            intake_digest,
            policy: active_policy,
            pop_snapshot,
            pop_snapshot_digest,
            status: ModerationAppealStatusV1::AwaitingAcceptance,
            submitted_by: appellant,
            submitted_at_unix_ms: 3,
            eligible_jurors,
            selection: Some(selection),
            accepted_jurors: Vec::new(),
            replacements: Vec::new(),
            activated_at_unix_ms: None,
            finalized_at_unix_ms: None,
        };
        (
            ModerationFinalizedLedgerSnapshotV1 {
                version: MODERATION_FINALIZED_SNAPSHOT_VERSION_V1,
                finalized_height: height,
                finalized_block_hash: block_hash,
                policy: Some(ModerationLedgerPolicyRecord {
                    policy: active_policy,
                    policy_digest,
                    activated_at_unix_ms: 1,
                    activated_by: governance.clone(),
                }),
                status: Some(ModerationLedgerStatusV1 {
                    appeal_intakes: 1,
                    eligibility_proofs: 3,
                    panel_selections: 1,
                    updated_at_unix_ms: 21,
                    ..ModerationLedgerStatusV1::default()
                }),
                appeals: vec![ModerationFinalizedAppealViewV1 {
                    appeal,
                    eligibility,
                }],
                cases: Vec::new(),
                events: vec![ModerationFinalizedEventV1 {
                    sequence: 5,
                    block_height: height,
                    block_hash,
                    event_index: 0,
                    event: SorafsModerationLedgerEvent::new(
                        SorafsModerationLedgerEventKind::SortitionFinalized,
                        Some("case-failover".to_owned()),
                        Some("round-1".to_owned()),
                        governance,
                        21,
                    ),
                }],
            },
            sortition_digest,
        )
    }

    fn activated_case_snapshot(
        height: u64,
        block_hash: [u8; 32],
        governance: AccountId,
    ) -> ModerationFinalizedLedgerSnapshotV1 {
        let (mut snapshot, _) =
            awaiting_acceptance_snapshot(height, block_hash, governance.clone());
        let appeal_view = snapshot.appeals.first_mut().expect("appeal projection");
        let appeal = &mut appeal_view.appeal;
        let selection = appeal.selection.clone().expect("panel selection");
        let mut accepted_jurors = selection.jurors.clone();
        accepted_jurors.sort_by_key(ToString::to_string);
        appeal.status = ModerationAppealStatusV1::BallotOpen;
        appeal.accepted_jurors = accepted_jurors;
        appeal.activated_at_unix_ms = Some(31);

        let intake = &appeal.intake;
        let jurors = selection.jurors;
        let case = ModerationCaseRecordV1 {
            spec: ModerationCaseSpecV1 {
                version: MODERATION_LEDGER_CASE_VERSION_V1,
                context: SoraFsModerationBallotContextV1 {
                    version: SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1,
                    case_id: intake.case_id.clone(),
                    evidence_bundle_digest: intake.evidence_bundle_digest,
                    appeal_finance_config_version: intake.appeal_finance_config_version.clone(),
                    panel_roster_hash: sorafs_moderation_panel_roster_hash_v1(
                        &jurors,
                        intake.quorum,
                    ),
                    policy_reference: intake.policy_reference.clone(),
                    evidence_uri: intake.evidence_uri.clone(),
                },
                round_id: intake.round_id.clone(),
                jurors,
                quorum: intake.quorum,
                commit_deadline_unix_ms: intake.commit_deadline_unix_ms,
                challenge_deadline_unix_ms: intake.challenge_deadline_unix_ms,
                reveal_deadline_unix_ms: intake.reveal_deadline_unix_ms,
                policy_digest: intake.policy_digest,
            },
            policy: appeal.policy.clone(),
            status: ModerationCaseStatusV1::Open,
            opened_at_unix_ms: 31,
            opened_by: governance.clone(),
            commitment_count: 0,
            reveal_count: 0,
            challenge_count: 0,
            challenge_ids: Vec::new(),
            pending_challenge_count: 0,
            accepted_challenge_count: 0,
            expired_challenge_count: 0,
        };
        snapshot.cases = vec![ModerationFinalizedCaseViewV1 {
            case,
            commits: Vec::new(),
            reveals: Vec::new(),
            challenges: Vec::new(),
            outcome: None,
            no_shows: Vec::new(),
        }];
        snapshot.status = Some(ModerationLedgerStatusV1 {
            appeal_intakes: 1,
            eligibility_proofs: 3,
            panel_selections: 1,
            assignment_acceptances: 2,
            open_cases: 1,
            updated_at_unix_ms: 31,
            ..ModerationLedgerStatusV1::default()
        });
        snapshot.events = vec![ModerationFinalizedEventV1 {
            sequence: 6,
            block_height: height,
            block_hash,
            event_index: 0,
            event: SorafsModerationLedgerEvent::new(
                SorafsModerationLedgerEventKind::CaseActivated,
                Some("case-failover".to_owned()),
                Some("round-1".to_owned()),
                governance,
                31,
            ),
        }];
        snapshot
    }

    fn finalized_case_snapshot(
        mut snapshot: ModerationFinalizedLedgerSnapshotV1,
        height: u64,
        block_hash: [u8; 32],
        governance: AccountId,
    ) -> ModerationFinalizedLedgerSnapshotV1 {
        const FINALIZED_AT_UNIX_MS: u64 = 61;

        snapshot.finalized_height = height;
        snapshot.finalized_block_hash = block_hash;
        let appeal = &mut snapshot
            .appeals
            .first_mut()
            .expect("appeal projection")
            .appeal;
        appeal.status = ModerationAppealStatusV1::Finalized;
        appeal.finalized_at_unix_ms = Some(FINALIZED_AT_UNIX_MS);
        let case_view = snapshot.cases.first_mut().expect("case projection");
        case_view.case.status = ModerationCaseStatusV1::Finalized;
        let policy_digest = case_view.case.spec.policy_digest;
        case_view.no_shows = case_view
            .case
            .spec
            .jurors
            .iter()
            .cloned()
            .map(|juror| ModerationNoShowRecordV1 {
                case_id: "case-failover".to_owned(),
                round_id: "round-1".to_owned(),
                juror,
                kind: ModerationNoShowKindV1::MissingCommit,
                penalty_points: case_view.case.policy.missing_commit_penalty_points,
                policy_digest,
                recorded_at_unix_ms: FINALIZED_AT_UNIX_MS,
            })
            .collect();
        case_view
            .no_shows
            .sort_by_key(|record| record.juror.to_string());
        case_view.outcome = Some(ModerationOutcomeRecordV1 {
            case_id: "case-failover".to_owned(),
            round_id: "round-1".to_owned(),
            kind: ModerationOutcomeKindV1::QuorumNotMet,
            counts: ModerationVoteCountsV1::default(),
            votes_total: 0,
            quorum: case_view.case.spec.quorum,
            no_show_count: u32::try_from(case_view.no_shows.len()).expect("bounded no-show count"),
            finalized_at_unix_ms: FINALIZED_AT_UNIX_MS,
            finalized_by: governance.clone(),
        });
        snapshot.status = Some(ModerationLedgerStatusV1 {
            appeal_intakes: 1,
            eligibility_proofs: 3,
            panel_selections: 1,
            assignment_acceptances: 2,
            finalized_cases: 1,
            outcomes: 1,
            no_shows: u64::try_from(case_view.no_shows.len()).expect("bounded no-show count"),
            updated_at_unix_ms: FINALIZED_AT_UNIX_MS,
            ..ModerationLedgerStatusV1::default()
        });
        snapshot.events = vec![ModerationFinalizedEventV1 {
            sequence: 7,
            block_height: height,
            block_hash,
            event_index: 0,
            event: SorafsModerationLedgerEvent::new(
                SorafsModerationLedgerEventKind::CaseFinalized,
                Some("case-failover".to_owned()),
                Some("round-1".to_owned()),
                governance,
                FINALIZED_AT_UNIX_MS,
            ),
        }];
        snapshot
    }

    fn config(temp: &TempDir, name: &str) -> ModerationOrchestratorConfigV1 {
        let canonical_temp = temp.path().canonicalize().expect("canonical tempdir");
        ModerationOrchestratorConfigV1 {
            checkpoint_path: canonical_temp.join(name),
            max_cases: 64,
            max_events: 256,
            max_outbox_entries: 16,
            max_idempotency_records: 64,
            max_handoffs: 64,
            max_submit_attempts: 3,
            checkpoint_max_bytes: 4 * 1024 * 1024,
        }
    }

    #[test]
    fn canonical_committed_event_sequence_must_be_contiguous() {
        let temp = TempDir::new().expect("tempdir");
        let config = config(&temp, "event-sequence.bin");
        let authority = account(7);
        let mut snapshot = snapshot_with_policy(5, [0x55; 32], policy(1), authority.clone());
        snapshot.events.clear();
        assert!(matches!(
            validate_finalized_snapshot(&snapshot, &config),
            Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(message))
                if message.contains("no committed event")
        ));
        snapshot.events.push(ModerationFinalizedEventV1 {
            sequence: 7,
            block_height: 4,
            block_hash: [0x44; 32],
            event_index: 0,
            event: SorafsModerationLedgerEvent::new(
                SorafsModerationLedgerEventKind::PolicyActivated,
                None,
                None,
                authority.clone(),
                1,
            ),
        });
        validate_finalized_snapshot(&snapshot, &config).expect("single retained event suffix");

        let mut skipped_block_index = snapshot.clone();
        skipped_block_index.events.push(ModerationFinalizedEventV1 {
            sequence: 8,
            block_height: 4,
            block_hash: [0x44; 32],
            event_index: 2,
            event: SorafsModerationLedgerEvent::new(
                SorafsModerationLedgerEventKind::PolicyActivated,
                None,
                None,
                authority.clone(),
                1,
            ),
        });
        assert!(matches!(
            validate_finalized_snapshot(&skipped_block_index, &config),
            Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(message))
                if message.contains("block index")
        ));

        snapshot.events.push(ModerationFinalizedEventV1 {
            sequence: 9,
            block_height: 4,
            block_hash: [0x44; 32],
            event_index: 1,
            event: SorafsModerationLedgerEvent::new(
                SorafsModerationLedgerEventKind::PolicyActivated,
                None,
                None,
                authority,
                1,
            ),
        });
        assert!(matches!(
            validate_finalized_snapshot(&snapshot, &config),
            Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(message))
                if message.contains("sequence")
        ));
    }

    fn deps(
        reader: Arc<MockSnapshotReader>,
        submitter: Arc<MockSubmitter>,
    ) -> ModerationOrchestratorDepsV1 {
        ModerationOrchestratorDepsV1 {
            submitter,
            snapshot_reader: reader,
            settlement_sink: Arc::new(MockHandoffSink::default()),
            publication_sink: Arc::new(MockHandoffSink::default()),
        }
    }

    #[test]
    fn duplicate_cross_replica_submission_reuses_one_transaction() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 1,
        }));
        let first = ModerationOrchestratorV1::open(
            config(&temp, "first.norito"),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("first orchestrator");
        let second = ModerationOrchestratorV1::open(
            config(&temp, "second.norito"),
            deps(reader, Arc::clone(&submitter)),
        )
        .expect("second orchestrator");
        let authority = account(1);
        let action = policy_action(policy(1));

        let first_outcome = first
            .submit(authority.clone(), action.clone(), [0x11; 32])
            .expect("first submit");
        let second_outcome = second
            .submit(authority, action, [0x22; 32])
            .expect("second submit");

        assert_eq!(submitter.calls(), 1);
        assert_eq!(first_outcome.operation_id, second_outcome.operation_id);
        assert_eq!(first_outcome.transaction_id, second_outcome.transaction_id);
    }

    #[test]
    fn same_semantic_identity_with_different_action_is_rejected() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 1,
        }));
        let orchestrator = ModerationOrchestratorV1::open(
            config(&temp, "checkpoint.norito"),
            deps(reader, submitter),
        )
        .expect("orchestrator");
        let authority = account(1);
        orchestrator
            .submit(authority.clone(), policy_action(policy(1)), [0x11; 32])
            .expect("first submit");
        let mut conflicting = policy(1);
        conflicting.missing_commit_penalty_points = 11;

        let error = orchestrator
            .submit(authority, policy_action(conflicting), [0x22; 32])
            .expect_err("conflicting replay must fail");
        assert!(matches!(
            error,
            ModerationOrchestratorError::IdempotencyConflict { .. }
        ));
    }

    #[test]
    fn stale_and_equivocating_finalized_cursors_fail_closed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(2, [2; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 2,
        }));
        let orchestrator = ModerationOrchestratorV1::open(
            config(&temp, "checkpoint.norito"),
            deps(Arc::clone(&reader), submitter),
        )
        .expect("orchestrator");
        orchestrator.reconcile().expect("initial reconcile");

        reader.replace(empty_snapshot(1, [1; 32]));
        assert!(matches!(
            orchestrator.reconcile(),
            Err(ModerationOrchestratorError::StaleFinalizedCursor { .. })
        ));

        reader.replace(empty_snapshot(2, [9; 32]));
        assert!(matches!(
            orchestrator.reconcile(),
            Err(ModerationOrchestratorError::FinalizedEquivocation { .. })
        ));
    }

    #[test]
    fn ambiguous_submission_is_reconciled_after_restart_without_resubmit() {
        let temp = tempfile::tempdir().expect("tempdir");
        let authority = account(1);
        let active_policy = policy(1);
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::ambiguous_applied(
            ModerationSubmissionLookupV1::NotFound {
                observed_finalized_height: 1,
            },
        ));
        let checkpoint = config(&temp, "checkpoint.norito");
        {
            let orchestrator = ModerationOrchestratorV1::open(
                checkpoint.clone(),
                deps(Arc::clone(&reader), Arc::clone(&submitter)),
            )
            .expect("orchestrator");
            orchestrator
                .submit(
                    authority.clone(),
                    policy_action(active_policy.clone()),
                    [0x11; 32],
                )
                .expect("ambiguous submit remains pending");
        }

        reader.replace(snapshot_with_policy(
            2,
            [2; 32],
            active_policy.clone(),
            authority.clone(),
        ));
        let restarted =
            ModerationOrchestratorV1::open(checkpoint, deps(reader, Arc::clone(&submitter)))
                .expect("restarted orchestrator");
        restarted.reconcile().expect("finalized reconciliation");
        let replay = restarted
            .submit(authority, policy_action(active_policy), [0x11; 32])
            .expect("finalized replay");

        assert_eq!(submitter.calls(), 1);
        assert_eq!(replay.status, ModerationOperationStatusV1::Finalized);
        assert!(replay.replay);
    }

    #[test]
    fn terminal_finalization_converges_after_restart_and_split_peer_replay() {
        let temp = tempfile::tempdir().expect("tempdir");
        let governance = account(99);
        let open_snapshot = activated_case_snapshot(2, [2; 32], governance.clone());
        let finalized_snapshot =
            finalized_case_snapshot(open_snapshot.clone(), 3, [3; 32], governance.clone());
        let reader = Arc::new(MockSnapshotReader::new(open_snapshot));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 2,
        }));
        let settlement_sink = Arc::new(MockHandoffSink::default());
        let publication_sink = Arc::new(MockHandoffSink::default());
        let runtime_deps = || ModerationOrchestratorDepsV1 {
            submitter: submitter.clone(),
            snapshot_reader: reader.clone(),
            settlement_sink: settlement_sink.clone(),
            publication_sink: publication_sink.clone(),
        };
        let first_checkpoint = config(&temp, "terminal-first.norito");
        let second_checkpoint = config(&temp, "terminal-second.norito");
        let action = ModerationNativeActionV1::FinalizeCase(FinalizeSorafsModerationCase::new(
            "case-failover".to_owned(),
            "round-1".to_owned(),
        ));

        let first = ModerationOrchestratorV1::open(first_checkpoint.clone(), runtime_deps())
            .expect("first orchestrator");
        let second = ModerationOrchestratorV1::open(second_checkpoint, runtime_deps())
            .expect("second orchestrator");
        let first_submit = first
            .submit(governance.clone(), action.clone(), [0x11; 32])
            .expect("first terminal submit");
        let split_peer_submit = second
            .submit(governance.clone(), action.clone(), [0x22; 32])
            .expect("split-peer terminal replay");
        assert_eq!(first_submit.status, ModerationOperationStatusV1::Pending);
        assert_eq!(
            split_peer_submit.status,
            ModerationOperationStatusV1::Pending
        );
        assert_eq!(first_submit.operation_id, split_peer_submit.operation_id);
        assert_eq!(
            first_submit.transaction_id,
            split_peer_submit.transaction_id
        );
        assert_eq!(submitter.calls(), 1);

        drop(first);
        reader.replace(finalized_snapshot);
        let restarted = ModerationOrchestratorV1::open(first_checkpoint, runtime_deps())
            .expect("restarted orchestrator");
        restarted
            .reconcile()
            .expect("restart reconciles finalized case");
        second
            .reconcile()
            .expect("split peer reconciles finalized case");
        let restarted_replay = restarted
            .submit(governance.clone(), action.clone(), [0x11; 32])
            .expect("restarted finalized replay");
        let split_peer_replay = second
            .submit(governance, action, [0x22; 32])
            .expect("split-peer finalized replay");
        assert_eq!(
            restarted_replay.status,
            ModerationOperationStatusV1::Finalized
        );
        assert_eq!(
            split_peer_replay.status,
            ModerationOperationStatusV1::Finalized
        );
        assert!(restarted_replay.replay);
        assert!(split_peer_replay.replay);
        assert_eq!(submitter.calls(), 1);

        let restarted_case = restarted
            .case("case-failover", "round-1")
            .expect("restarted case projection");
        let split_peer_case = second
            .case("case-failover", "round-1")
            .expect("split-peer case projection");
        assert!(restarted_case.outcome.is_some());
        assert_eq!(
            norito::to_bytes(&restarted_case).expect("encode restarted projection"),
            norito::to_bytes(&split_peer_case).expect("encode split-peer projection")
        );
        assert_eq!(settlement_sink.delivered().len(), 1);
        assert_eq!(publication_sink.delivered().len(), 1);

        restarted.reconcile().expect("idempotent restart reconcile");
        second.reconcile().expect("idempotent split-peer reconcile");
        assert_eq!(settlement_sink.delivered().len(), 1);
        assert_eq!(publication_sink.delivered().len(), 1);
    }

    #[test]
    fn outbox_capacity_exhaustion_is_fail_closed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
        let mut bounds = config(&temp, "checkpoint.norito");
        bounds.max_outbox_entries = 1;
        let orchestrator =
            ModerationOrchestratorV1::open(bounds, deps(reader, submitter)).expect("orchestrator");
        let authority = account(1);
        orchestrator
            .submit(authority.clone(), policy_action(policy(1)), [0x11; 32])
            .expect("first pending operation");

        let error = orchestrator
            .submit(authority, policy_action(policy(2)), [0x22; 32])
            .expect_err("second pending operation must exceed the bound");
        assert!(matches!(
            error,
            ModerationOrchestratorError::ResourceExhausted {
                resource: "native transaction outbox",
                limit: 1
            }
        ));
    }

    #[test]
    fn no_show_failover_uses_one_stable_native_activation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let governance = account(99);
        let (snapshot, expected_sortition_digest) =
            awaiting_acceptance_snapshot(2, [2; 32], governance.clone());
        let reader = Arc::new(MockSnapshotReader::new(snapshot));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 2,
        }));
        let orchestrator = ModerationOrchestratorV1::open(
            config(&temp, "checkpoint.norito"),
            deps(reader, Arc::clone(&submitter)),
        )
        .expect("orchestrator");

        let first = orchestrator
            .run_maintenance(governance.clone(), 31, 1)
            .expect("first failover scan");
        let replay = orchestrator
            .run_maintenance(governance, 31, 1)
            .expect("replayed failover scan");

        assert_eq!(first.len(), 1);
        assert_eq!(replay.len(), 1);
        assert_eq!(first[0].operation_id, replay[0].operation_id);
        assert!(replay[0].replay);
        assert_eq!(submitter.calls(), 1);
        let actions = submitter.actions();
        let [ModerationNativeActionV1::ActivateCase(activation)] = actions.as_slice() else {
            panic!("expected one native activation action");
        };
        assert_eq!(activation.case_id(), "case-failover");
        assert_eq!(activation.round_id(), "round-1");
        assert_eq!(*activation.sortition_digest(), expected_sortition_digest);
    }

    #[test]
    fn authenticated_request_binding_is_exact_and_canonical() {
        let authority = account(1);
        let action = policy_action(policy(1));
        let first = moderation_request_binding_digest_v1(
            "POST",
            "/v1/sorafs/moderation/actions?revision=1",
            b"body",
            &authority,
            &action,
        )
        .expect("canonical binding");
        let changed_body = moderation_request_binding_digest_v1(
            "POST",
            "/v1/sorafs/moderation/actions?revision=1",
            b"changed",
            &authority,
            &action,
        )
        .expect("canonical binding");
        let changed_query = moderation_request_binding_digest_v1(
            "POST",
            "/v1/sorafs/moderation/actions?revision=2",
            b"body",
            &authority,
            &action,
        )
        .expect("canonical binding");

        assert_ne!(first, changed_body);
        assert_ne!(first, changed_query);
        assert!(matches!(
            moderation_request_binding_digest_v1(
                "post",
                "/v1/sorafs/moderation/actions",
                b"body",
                &authority,
                &action,
            ),
            Err(ModerationOrchestratorError::InvalidRequestBinding)
        ));
        assert!(matches!(
            moderation_request_binding_digest_v1(
                "POST",
                "/v1/sorafs/../moderation/actions",
                b"body",
                &authority,
                &action,
            ),
            Err(ModerationOrchestratorError::InvalidRequestBinding)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn checkpoint_failure_latches_the_process_fail_closed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 1,
        }));
        let bounds = config(&temp, "checkpoint.norito");
        let checkpoint_path = bounds.checkpoint_path.clone();
        let orchestrator =
            ModerationOrchestratorV1::open(bounds, deps(reader, submitter)).expect("orchestrator");
        std::os::unix::fs::symlink(
            checkpoint_path.with_extension("untrusted-target"),
            &checkpoint_path,
        )
        .expect("install checkpoint symlink");

        assert!(matches!(
            orchestrator.reconcile(),
            Err(ModerationOrchestratorError::CheckpointIo(_))
        ));
        std::fs::remove_file(&checkpoint_path).expect("remove checkpoint symlink");
        assert_eq!(
            orchestrator.reconcile(),
            Err(ModerationOrchestratorError::DurabilityFaulted)
        );
        assert!(orchestrator.snapshot().is_none());
    }
}
