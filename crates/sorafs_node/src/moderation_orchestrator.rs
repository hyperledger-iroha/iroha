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
    transaction::{Executable, SignedTransaction},
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
///
/// Version four adds generation-fenced leases for every external collaborator
/// call. Earlier pre-release state is intentionally rejected instead of migrated.
pub const MODERATION_ORCHESTRATOR_CHECKPOINT_VERSION_V1: u16 = 4;
/// Hard ceiling for one canonical native moderation instruction.
pub const MODERATION_NATIVE_INSTRUCTION_MAX_BYTES_V1: usize = 2 * 1024 * 1024;
/// Hard ceiling for one persisted orchestrator checkpoint.
pub const MODERATION_ORCHESTRATOR_CHECKPOINT_MAX_BYTES_V1: u64 = 32 * 1024 * 1024;
/// Hard ceiling for one canonical signed moderation transaction.
pub const MODERATION_SIGNED_TRANSACTION_MAX_BYTES_V1: usize = 10 * 1024 * 1024;
/// Exact first-release lifetime for every signed moderation transaction.
pub const MODERATION_TRANSACTION_TTL_MS_V1: u64 = 5 * 60 * 1_000;
/// Exact first-release worker lease for one panel-notification delivery.
pub const MODERATION_PANEL_NOTIFICATION_LEASE_MS_V1: u64 = 30_000;
/// Exact first-release lease for signer, ingress, lookup, and terminal-sink work.
pub const MODERATION_EXTERNAL_WORK_LEASE_MS_V1: u64 = 30_000;
/// Initial retry delay for a panel-notification delivery.
pub const MODERATION_PANEL_NOTIFICATION_BACKOFF_BASE_MS_V1: u64 = 1_000;
/// Maximum retry delay for a panel-notification delivery.
pub const MODERATION_PANEL_NOTIFICATION_BACKOFF_MAX_MS_V1: u64 = 5 * 60 * 1_000;

const ACTION_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.moderation.native-action.v1";
const OPERATION_ID_DOMAIN_V1: &[u8] = b"sorafs.moderation.operation-id.v1";
const REQUEST_BINDING_DOMAIN_V1: &[u8] = b"sorafs.moderation.http-request-binding.v1";
const SNAPSHOT_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.moderation.finalized-snapshot.v1";
const HANDOFF_ID_DOMAIN_V1: &[u8] = b"sorafs.moderation.terminal-handoff.v1";
const POP_PROOF_PAYLOAD_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.moderation.pop-proof-payload.v1";
const SIGNED_TRANSACTION_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.moderation.signed-transaction.v1";
const RETIRED_ENVELOPE_RECORD_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.retired-envelope-record.v1";
const PANEL_NOTIFICATION_ID_DOMAIN_V1: &[u8] = b"sorafs.moderation.panel-notification-id.v1";
const PANEL_NOTIFICATION_SCOPE_DOMAIN_V1: &[u8] = b"sorafs.moderation.panel-notification-scope.v1";
const PANEL_NOTIFICATION_LEASE_DOMAIN_V1: &[u8] = b"sorafs.moderation.panel-notification-lease.v1";
const PANEL_NOTIFICATION_RECORD_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.panel-notification-record.v1";
const PANEL_NOTIFICATION_OUTBOX_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.panel-notification-outbox.v1";
const EXTERNAL_WORK_DIGEST_DOMAIN_V1: &[u8] = b"sorafs.moderation.external-work-digest.v1";
const EXTERNAL_WORK_LEASE_DOMAIN_V1: &[u8] = b"sorafs.moderation.external-work-lease.v1";
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
    MODERATION_NATIVE_INSTRUCTION_MAX_BYTES_V1,
    MODERATION_NATIVE_INSTRUCTION_MAX_BYTES_V1,
    2 * MODERATION_NATIVE_INSTRUCTION_MAX_BYTES_V1,
    2 * MODERATION_NATIVE_INSTRUCTION_MAX_BYTES_V1,
    128,
);
const SIGNED_TRANSACTION_LIMITS: DecodeLimits = DecodeLimits::new(
    MODERATION_SIGNED_TRANSACTION_MAX_BYTES_V1,
    MODERATION_SIGNED_TRANSACTION_MAX_BYTES_V1,
    2 * MODERATION_SIGNED_TRANSACTION_MAX_BYTES_V1,
    2 * MODERATION_SIGNED_TRANSACTION_MAX_BYTES_V1,
    128,
);

/// One exact native moderation mutation.
#[expect(
    clippy::large_enum_variant,
    reason = "boxing a variant would change the canonical public Norito action shape"
)]
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

    /// Validate the canonical V1 action and its caller authority binding.
    ///
    /// Caller-signed Torii command adapters use this same validator as the
    /// governed orchestration path so no route can admit a looser envelope.
    ///
    /// # Errors
    ///
    /// Fails when the action exceeds its bound, contains noncanonical embedded
    /// payloads, violates native field rules, or is signed by the wrong caller.
    pub fn validate_for_authority(
        &self,
        authority: &AccountId,
    ) -> Result<(), ModerationOrchestratorError> {
        self.validate_authority(authority)?;
        self.canonical_bytes()?;
        Ok(())
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

    fn operation_id(
        &self,
        chain_id: &iroha_data_model::ChainId,
        authority: &AccountId,
    ) -> Result<[u8; 32], ModerationOrchestratorError> {
        let material = self.semantic_material(authority)?;
        Ok(domain_hash(
            OPERATION_ID_DOMAIN_V1,
            &[chain_id.as_str().as_bytes(), material.as_slice()],
        ))
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
    /// Maximum retained records in each finalized delivery family.
    pub max_handoffs: usize,
    /// Maximum safe submission attempts and envelope generations under one operation identity.
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
    /// Exact ledger chain bound into the signed transaction and operation id.
    pub chain_id: iroha_data_model::ChainId,
    /// Durable signed-envelope generation; semantic operation identity remains stable.
    pub envelope_generation: u32,
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
    /// Finalized block hash paired with `baseline_finalized_height`.
    pub baseline_finalized_block_hash: [u8; 32],
}

impl ModerationTransactionRequestV1 {
    /// Construct the canonical submission request for one authenticated action.
    ///
    /// # Errors
    ///
    /// Fails closed when the authority does not match the action, the action
    /// cannot be canonically encoded, the request binding is inert, or the
    /// finalized baseline is invalid.
    pub fn new(
        chain_id: iroha_data_model::ChainId,
        envelope_generation: u32,
        authority: AccountId,
        action: ModerationNativeActionV1,
        request_binding_digest: [u8; 32],
        baseline_finalized_height: u64,
        baseline_finalized_block_hash: [u8; 32],
    ) -> Result<Self, ModerationOrchestratorError> {
        action.validate_authority(&authority)?;
        let canonical_action = action.canonical_bytes()?;
        let action_digest = action.action_digest()?;
        if chain_id.as_str().is_empty() || chain_id.as_str() != chain_id.as_str().trim() {
            return Err(ModerationOrchestratorError::InvalidAction(
                "submission chain id must be non-empty and canonical".to_owned(),
            ));
        }
        if envelope_generation == 0 {
            return Err(ModerationOrchestratorError::InvalidAction(
                "submission envelope generation must be non-zero".to_owned(),
            ));
        }
        let operation_id = action.operation_id(&chain_id, &authority)?;
        let request = Self {
            chain_id,
            envelope_generation,
            operation_id,
            authority,
            action,
            canonical_action,
            action_digest,
            request_binding_digest,
            baseline_finalized_height,
            baseline_finalized_block_hash,
        };
        request.validate()?;
        Ok(request)
    }

    /// Recompute and validate every canonical identity and authority binding.
    ///
    /// This is the sole request validator used at transaction-adapter
    /// boundaries; callers must not reproduce its digest domains locally.
    ///
    /// # Errors
    ///
    /// Fails closed for inert fields, authority substitution, noncanonical
    /// action bytes, or mismatched action/operation digests.
    pub fn validate(&self) -> Result<(), ModerationOrchestratorError> {
        if self.request_binding_digest == [0; 32] {
            return Err(ModerationOrchestratorError::InvalidRequestBinding);
        }
        if self.baseline_finalized_height == 0 || self.baseline_finalized_block_hash == [0; 32] {
            return Err(ModerationOrchestratorError::InvalidAction(
                "submission baseline finalized cursor must be non-zero".to_owned(),
            ));
        }
        self.action.validate_authority(&self.authority)?;
        let canonical_action = self.action.canonical_bytes()?;
        if canonical_action != self.canonical_action {
            return Err(ModerationOrchestratorError::InvalidAction(
                "submission canonical action bytes do not match the native action".to_owned(),
            ));
        }
        let action_digest = self.action.action_digest()?;
        if action_digest == [0; 32] || action_digest != self.action_digest {
            return Err(ModerationOrchestratorError::InvalidAction(
                "submission action digest does not match the native action".to_owned(),
            ));
        }
        if self.chain_id.as_str().is_empty()
            || self.chain_id.as_str() != self.chain_id.as_str().trim()
        {
            return Err(ModerationOrchestratorError::InvalidAction(
                "submission chain id must be non-empty and canonical".to_owned(),
            ));
        }
        if self.envelope_generation == 0 {
            return Err(ModerationOrchestratorError::InvalidAction(
                "submission envelope generation must be non-zero".to_owned(),
            ));
        }
        let operation_id = self.action.operation_id(&self.chain_id, &self.authority)?;
        if operation_id == [0; 32] || operation_id != self.operation_id {
            return Err(ModerationOrchestratorError::InvalidAction(
                "submission operation identity does not match the authority and native action"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}

/// Exact signed transaction retained before any ingress call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationSignedTransactionV1 {
    /// Native signed transaction hash.
    pub transaction_id: [u8; 32],
    /// Digest binding the exact canonical signed envelope bytes.
    pub canonical_bytes_digest: [u8; 32],
    /// Canonical Norito signed transaction bytes.
    pub canonical_bytes: Vec<u8>,
}

impl ModerationSignedTransactionV1 {
    /// Validate and retain one exact signed transaction for `request`.
    ///
    /// # Errors
    ///
    /// Fails closed for invalid signatures, substituted authority or
    /// instructions, noncanonical or oversized bytes, and inert identities.
    pub fn from_signed_transaction(
        request: &ModerationTransactionRequestV1,
        transaction: &SignedTransaction,
    ) -> Result<Self, ModerationSubmissionFailureV1> {
        request
            .validate()
            .map_err(|_| ModerationSubmissionFailureV1::PermanentRejection)?;
        validate_signed_transaction_for_request(request, transaction)?;
        let canonical_bytes = norito::to_bytes(transaction)
            .map_err(|_| ModerationSubmissionFailureV1::PermanentRejection)?;
        let signed = Self {
            transaction_id: *transaction.hash().as_ref(),
            canonical_bytes_digest: signed_transaction_digest(&canonical_bytes),
            canonical_bytes,
        };
        signed.decode_for_request(request)?;
        Ok(signed)
    }

    /// Decode and revalidate the exact retained transaction.
    ///
    /// # Errors
    ///
    /// Fails closed if any retained field or canonical byte differs from the
    /// signed transaction bound to `request`.
    pub fn decode_for_request(
        &self,
        request: &ModerationTransactionRequestV1,
    ) -> Result<SignedTransaction, ModerationSubmissionFailureV1> {
        request
            .validate()
            .map_err(|_| ModerationSubmissionFailureV1::PermanentRejection)?;
        if self.transaction_id == [0; 32]
            || self.canonical_bytes_digest == [0; 32]
            || self.canonical_bytes.is_empty()
            || self.canonical_bytes.len() > MODERATION_SIGNED_TRANSACTION_MAX_BYTES_V1
            || signed_transaction_digest(&self.canonical_bytes) != self.canonical_bytes_digest
        {
            return Err(ModerationSubmissionFailureV1::PermanentRejection);
        }
        let transaction = decode_from_bytes_with_limits::<SignedTransaction>(
            &self.canonical_bytes,
            SIGNED_TRANSACTION_LIMITS,
        )
        .map_err(|_| ModerationSubmissionFailureV1::PermanentRejection)?;
        let canonical = norito::to_bytes(&transaction)
            .map_err(|_| ModerationSubmissionFailureV1::PermanentRejection)?;
        if canonical != self.canonical_bytes || *transaction.hash().as_ref() != self.transaction_id
        {
            return Err(ModerationSubmissionFailureV1::PermanentRejection);
        }
        validate_signed_transaction_for_request(request, &transaction)?;
        Ok(transaction)
    }
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
    ///
    /// Envelope renewal treats this as strong absence only when the height is
    /// exactly the independently queried finalized snapshot anchor. A lesser
    /// height may permit replay of the same unexpired bytes but never renewal.
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

/// Runtime-only HSM and strict transaction-ingress interface.
pub trait ModerationTransactionSubmitterV1: Send + Sync {
    /// Exact ledger chain implemented by this runtime boundary.
    ///
    /// The orchestrator freezes this value at open and rejects every retained
    /// or newly signed envelope whose chain differs.
    fn chain_id(&self) -> iroha_data_model::ChainId;

    /// Sign exactly one native action without exposing it to transaction ingress.
    ///
    /// The orchestrator durably retains the returned canonical bytes and hash
    /// before calling [`Self::submit_signed`].
    fn sign(
        &self,
        request: &ModerationTransactionRequestV1,
    ) -> Result<ModerationSignedTransactionV1, ModerationSubmissionFailureV1>;

    /// Submit the exact signed envelope already retained by the orchestrator.
    ///
    /// Implementations must never sign or replace a transaction here. They
    /// must atomically deduplicate `operation_id` and `signed.transaction_id`
    /// at their strict durable ingress boundary.
    fn submit_signed(
        &self,
        request: &ModerationTransactionRequestV1,
        signed: &ModerationSignedTransactionV1,
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

/// Payload-free panel-notification category derived from finalized ledger state.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
pub enum ModerationPanelNotificationKindV1 {
    /// A primary juror may accept the finalized assignment.
    PrimaryAssignment,
    /// A failover candidate should remain available during the acceptance window.
    WaitlistStandby,
    /// The authoritative commit/reveal ballot is open for this juror.
    BallotActivated,
}

impl ModerationPanelNotificationKindV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::PrimaryAssignment => 0,
            Self::WaitlistStandby => 1,
            Self::BallotActivated => 2,
        }
    }
}

/// One payload-free notification derived from an exact finalized operation.
///
/// The record intentionally contains no case identifier, evidence locator,
/// reason, attestation, holder material, or message body. The recipient resolves
/// current assignment details from the finalized ledger after receiving the
/// stable notification identity.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationPanelNotificationV1 {
    /// Stable delivery identity used for sink-side idempotency.
    pub notification_id: [u8; 32],
    /// Semantic native-operation identity that caused the notification.
    pub source_operation_id: [u8; 32],
    /// Digest of the case/round scope; the raw scope is deliberately not retained.
    pub scope_digest: [u8; 32],
    /// Payload-free lifecycle category.
    pub kind: ModerationPanelNotificationKindV1,
    /// Public ledger account to notify.
    pub recipient: AccountId,
    /// Exact committed event proving that the source operation finalized.
    pub finalized_event_cursor: ModerationFinalizedEventCursorV1,
    /// Consensus timestamp of the source event.
    pub source_occurred_at_unix_ms: u64,
}

/// Lease returned to a notification worker after the claim is durable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationPanelNotificationClaimV1 {
    /// Payload-free notification metadata.
    pub notification: ModerationPanelNotificationV1,
    /// Runtime worker identity.
    pub worker_id: [u8; 32],
    /// Generation-bound compare-and-swap token.
    pub lease_token: [u8; 32],
    /// Exclusive lease expiry.
    pub lease_expires_at_unix_ms: u64,
    /// One-based bounded delivery attempt.
    pub attempt: u32,
    /// Immutable attempt ceiling captured when the finalized event was scanned.
    pub attempt_limit: u32,
}

/// Stable sink receipt used to finalize a notification delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModerationPanelNotificationDeliveryReceiptV1 {
    /// Notification identity supplied to the sink.
    pub notification_id: [u8; 32],
    /// Non-secret digest of the sink's idempotent receipt.
    pub receipt_digest: [u8; 32],
    /// Runtime time at which the sink durably accepted the notification.
    pub delivered_at_unix_ms: u64,
}

/// Safe, payload-free notification delivery failure class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationPanelNotificationFailureV1 {
    /// The sink established that no delivery occurred.
    NotDelivered,
    /// Delivery may have occurred; retrying the identical identity is required.
    Ambiguous,
    /// The sink permanently rejected this notification.
    Permanent,
}

/// Durable terminal reason for a panel notification.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
pub enum ModerationPanelNotificationDeadLetterReasonV1 {
    /// The sink permanently rejected the payload-free notification.
    PermanentRejection,
    /// Every bounded claim was consumed without a durable receipt.
    RetryExhausted,
}

/// Public persisted state of a panel notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationPanelNotificationStatusV1 {
    /// Awaiting its first attempt or a bounded retry delay.
    Pending {
        /// Earliest runtime time at which a worker may claim it.
        available_at_unix_ms: u64,
        /// Attempts already consumed.
        attempts: u32,
        /// Immutable attempt ceiling.
        attempt_limit: u32,
    },
    /// Durably leased to exactly one worker generation.
    Claimed {
        /// Current worker identity.
        worker_id: [u8; 32],
        /// Exclusive lease expiry.
        lease_expires_at_unix_ms: u64,
        /// Attempts already consumed.
        attempts: u32,
        /// Immutable attempt ceiling.
        attempt_limit: u32,
    },
    /// A stable sink receipt was durably recorded.
    Delivered {
        /// Non-secret stable receipt digest.
        receipt_digest: [u8; 32],
        /// Durable sink delivery time.
        delivered_at_unix_ms: u64,
        /// Attempts consumed before delivery.
        attempts: u32,
        /// Immutable attempt ceiling.
        attempt_limit: u32,
    },
    /// No more delivery attempts are permitted.
    DeadLetter {
        /// Fixed terminal reason.
        reason: ModerationPanelNotificationDeadLetterReasonV1,
        /// Runtime time at which the record became terminal.
        dead_lettered_at_unix_ms: u64,
        /// Attempts consumed before terminalization.
        attempts: u32,
        /// Immutable attempt ceiling.
        attempt_limit: u32,
    },
}

/// Outcome of an idempotent receipt finalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationPanelNotificationFinalizeOutcomeV1 {
    /// The claim and receipt became durable in this call.
    Delivered,
    /// The byte-identical receipt was already durable.
    AlreadyDelivered,
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
    Signing,
    Signed,
    Ambiguous,
    Submitted,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
enum StoredExternalWorkKindV1 {
    Sign,
    Submit,
    Lookup,
    Handoff,
}

impl StoredExternalWorkKindV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::Sign => 0,
            Self::Submit => 1,
            Self::Lookup => 2,
            Self::Handoff => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredExternalWorkClaimV1 {
    kind: StoredExternalWorkKindV1,
    generation: u32,
    claimed_at_finalized_height: u64,
    claimed_at_finalized_block_hash: [u8; 32],
    claimed_at_unix_ms: u64,
    lease_expires_at_unix_ms: u64,
    work_digest: [u8; 32],
    lease_token: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum StoredRetiredEnvelopeDispositionV1 {
    NotFound,
    Pending,
    Applied,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredRetiredEnvelopeV1 {
    generation: u32,
    transaction_id: [u8; 32],
    signed_transaction_digest: [u8; 32],
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
    retired_at_finalized_height: u64,
    retired_at_finalized_block_hash: [u8; 32],
    retired_at_finalized_unix_ms: u64,
    disposition: StoredRetiredEnvelopeDispositionV1,
    record_digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredOutboxEntryV1 {
    operation_id: [u8; 32],
    authority: AccountId,
    action: ModerationNativeActionV1,
    action_digest: [u8; 32],
    request_binding_digest: [u8; 32],
    envelope_generation: u32,
    retired_envelopes: Vec<StoredRetiredEnvelopeV1>,
    baseline_finalized_height: u64,
    baseline_finalized_block_hash: [u8; 32],
    transaction_id: Option<[u8; 32]>,
    signed_transaction_digest: Option<[u8; 32]>,
    signed_transaction_bytes: Option<Vec<u8>>,
    attempts: u32,
    state: StoredOutboxStateV1,
    work_generation: u32,
    work_claim: Option<StoredExternalWorkClaimV1>,
    last_lookup_finalized_height: u64,
    last_lookup_finalized_block_hash: [u8; 32],
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
    work_generation: u32,
    work_claim: Option<StoredExternalWorkClaimV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum StoredPanelNotificationStateV1 {
    Pending,
    Claimed,
    Delivered,
    DeadLetter,
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredPanelNotificationV1 {
    notification: ModerationPanelNotificationV1,
    attempt_limit: u32,
    attempts: u32,
    claim_generation: u32,
    available_at_unix_ms: u64,
    state: StoredPanelNotificationStateV1,
    claimed_by: Option<[u8; 32]>,
    lease_token: Option<[u8; 32]>,
    claimed_at_unix_ms: Option<u64>,
    lease_expires_at_unix_ms: Option<u64>,
    receipt_digest: Option<[u8; 32]>,
    delivered_at_unix_ms: Option<u64>,
    dead_letter_reason: Option<ModerationPanelNotificationDeadLetterReasonV1>,
    dead_lettered_at_unix_ms: Option<u64>,
    record_digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationOrchestratorCheckpointV1 {
    version: u16,
    generation: u64,
    panel_notification_clock_unix_ms: u64,
    panel_notification_scanned_cursor: Option<ModerationFinalizedEventCursorV1>,
    panel_notification_outbox_digest: [u8; 32],
    finalized_snapshot: Option<ModerationFinalizedLedgerSnapshotV1>,
    finalized_snapshot_digest: Option<[u8; 32]>,
    operations: Vec<StoredOperationV1>,
    outbox: Vec<StoredOutboxEntryV1>,
    dead_letters: Vec<StoredDeadLetterV1>,
    pending_handoffs: Vec<StoredHandoffV1>,
    completed_handoffs: Vec<[u8; 32]>,
    panel_notifications: Vec<StoredPanelNotificationV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ExternalWorkIdentityV1 {
    identity: [u8; 32],
    kind: StoredExternalWorkKindV1,
    work_digest: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExternalLookupProbeV1 {
    transaction_id: [u8; 32],
}

#[derive(Debug, Clone)]
enum PreparedExternalWorkV1 {
    Sign {
        identity: ExternalWorkIdentityV1,
        claim: StoredExternalWorkClaimV1,
        request: ModerationTransactionRequestV1,
    },
    Submit {
        identity: ExternalWorkIdentityV1,
        claim: StoredExternalWorkClaimV1,
        request: ModerationTransactionRequestV1,
        signed: ModerationSignedTransactionV1,
    },
    Lookup {
        identity: ExternalWorkIdentityV1,
        claim: StoredExternalWorkClaimV1,
        operation_id: [u8; 32],
        probes: Vec<ExternalLookupProbeV1>,
    },
    Handoff {
        identity: ExternalWorkIdentityV1,
        claim: StoredExternalWorkClaimV1,
        handoff: ModerationTerminalHandoffV1,
    },
}

impl PreparedExternalWorkV1 {
    fn identity(&self) -> ExternalWorkIdentityV1 {
        match self {
            Self::Sign { identity, .. }
            | Self::Submit { identity, .. }
            | Self::Lookup { identity, .. }
            | Self::Handoff { identity, .. } => *identity,
        }
    }
}

impl Default for ModerationOrchestratorCheckpointV1 {
    fn default() -> Self {
        let mut state = Self {
            version: MODERATION_ORCHESTRATOR_CHECKPOINT_VERSION_V1,
            generation: 0,
            panel_notification_clock_unix_ms: 0,
            panel_notification_scanned_cursor: None,
            panel_notification_outbox_digest: [0; 32],
            finalized_snapshot: None,
            finalized_snapshot_digest: None,
            operations: Vec::new(),
            outbox: Vec::new(),
            dead_letters: Vec::new(),
            pending_handoffs: Vec::new(),
            completed_handoffs: Vec::new(),
            panel_notifications: Vec::new(),
        };
        refresh_panel_notification_outbox_digest(&mut state);
        state
    }
}

/// Finalized-chain moderation orchestrator.
pub struct ModerationOrchestratorV1 {
    config: ModerationOrchestratorConfigV1,
    chain_id: iroha_data_model::ChainId,
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
        let chain_id = deps.submitter.chain_id();
        if chain_id.as_str().is_empty() || chain_id.as_str() != chain_id.as_str().trim() {
            return Err(ModerationOrchestratorError::InvalidConfiguration(
                "moderation submitter chain id must be non-empty and canonical".to_owned(),
            ));
        }
        ensure_secure_parent(&config.checkpoint_path)?;
        let mut state =
            match read_bounded_file(&config.checkpoint_path, config.checkpoint_max_bytes)? {
                None => ModerationOrchestratorCheckpointV1::default(),
                Some(bytes) => {
                    let limits = checkpoint_decode_limits(config.checkpoint_max_bytes)?;
                    let checkpoint = decode_from_bytes_with_limits::<
                        ModerationOrchestratorCheckpointV1,
                    >(&bytes, limits)
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
        validate_checkpoint(&state, &config, &chain_id)?;
        if recover_external_work_after_restart(&mut state) {
            persist_checkpoint(&config, &chain_id, &mut state)?;
        }
        Ok(Self {
            config,
            chain_id,
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
        let (snapshot, digest) = self.read_validated_finalized_snapshot()?;
        let cursor = snapshot.anchor();
        {
            let mut state = self.lock_state()?;
            self.install_finalized_snapshot_locked(&mut state, snapshot, digest)?;
        }
        self.drive_external_work()?;
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
        let operation_id = action.operation_id(&self.chain_id, &authority)?;
        let (snapshot, digest) = self.read_validated_finalized_snapshot()?;
        let cursor = snapshot.anchor();
        let replay = {
            let mut state = self.lock_state()?;
            self.install_finalized_snapshot_locked(&mut state, snapshot, digest)?;

            if let Some(existing) = find_operation(&state, operation_id) {
                if existing.action_digest != action_digest || existing.authority != authority {
                    return Err(ModerationOrchestratorError::IdempotencyConflict { operation_id });
                }
                true
            } else {
                let finalized_snapshot = state
                    .finalized_snapshot
                    .as_ref()
                    .ok_or(ModerationOrchestratorError::FinalizedReaderUnavailable)?;
                validate_finalized_action_authority(finalized_snapshot, &authority, &action)?;
                match action_effect(finalized_snapshot, &authority, &action)? {
                    ActionEffect::Exact => {
                        ensure_operation_capacity(&state, &self.config)?;
                        state.operations.push(StoredOperationV1 {
                            operation_id,
                            authority: authority.clone(),
                            action_digest,
                            status: StoredOperationStatusV1::Finalized,
                            transaction_id: None,
                        });
                        self.persist_checkpoint_locked(&mut state)?;
                        true
                    }
                    ActionEffect::Conflict => {
                        return Err(ModerationOrchestratorError::FinalizedConflict {
                            operation_id,
                        });
                    }
                    ActionEffect::Absent => {
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
                            envelope_generation: 1,
                            retired_envelopes: Vec::new(),
                            baseline_finalized_height: 0,
                            baseline_finalized_block_hash: [0; 32],
                            transaction_id: None,
                            signed_transaction_digest: None,
                            signed_transaction_bytes: None,
                            attempts: 0,
                            state: StoredOutboxStateV1::Ready,
                            work_generation: 0,
                            work_claim: None,
                            last_lookup_finalized_height: 0,
                            last_lookup_finalized_block_hash: [0; 32],
                        });
                        self.persist_checkpoint_locked(&mut state)?;
                        false
                    }
                }
            }
        };

        // All HSM, ingress, lookup, and terminal-sink calls happen after the
        // snapshot/action mutation lock has been released.
        self.drive_external_work()?;
        let state = self.lock_state()?;
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
            replay,
        })
    }

    /// Run deterministic deadline maintenance after a finalized reconciliation.
    ///
    /// Only native sortition, activation/failover, and finalization ISIs are
    /// emitted. The injected operational identity must equal the authority in
    /// the finalized policy or panel selection; configuration cannot override
    /// ledger authority. Deadline evaluation uses only the signed creation time
    /// of the exact finalized block retained in the snapshot. At most `limit`
    /// actions are attempted.
    ///
    /// # Errors
    ///
    /// Fails for invalid finalized state, bounds, submission, or durability errors.
    pub fn run_maintenance(
        &self,
        governance_authority: AccountId,
        limit: usize,
    ) -> Result<Vec<ModerationSubmitOutcomeV1>, ModerationOrchestratorError> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let cursor = self.reconcile()?;
        let snapshot = self
            .snapshot()
            .ok_or(ModerationOrchestratorError::FinalizedReaderUnavailable)?;
        let now_unix_ms = snapshot.finalized_at_unix_ms;
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

    /// Durably claim due panel notifications for delivery outside the state lock.
    ///
    /// The returned metadata is deliberately payload-free. Workers must use
    /// `notification_id` as the sink idempotency key and resolve any user-facing
    /// content from current finalized ledger state. A delivery that outlives its
    /// lease must be retried under a new claim; the stale worker cannot commit.
    ///
    /// # Errors
    ///
    /// Fails for a zero worker/time, clock rollback, an excessive batch,
    /// arithmetic exhaustion, corrupt state, or checkpoint failure.
    pub fn claim_panel_notifications(
        &self,
        worker_id: [u8; 32],
        now_unix_ms: u64,
        limit: usize,
    ) -> Result<Vec<ModerationPanelNotificationClaimV1>, ModerationOrchestratorError> {
        if worker_id == [0; 32] || now_unix_ms == 0 {
            return Err(ModerationOrchestratorError::InvalidPanelNotificationClaim);
        }
        if limit == 0 {
            return Ok(Vec::new());
        }
        if limit > self.config.max_handoffs {
            return Err(ModerationOrchestratorError::ResourceExhausted {
                resource: "panel notification claim batch",
                limit: self.config.max_handoffs,
            });
        }
        let lease_expires_at_unix_ms = now_unix_ms
            .checked_add(MODERATION_PANEL_NOTIFICATION_LEASE_MS_V1)
            .ok_or(ModerationOrchestratorError::GenerationOverflow)?;

        let mut state = self.lock_state()?;
        validate_panel_notification_clock(&state, now_unix_ms)?;
        preflight_expired_panel_notification_claims(&state, now_unix_ms)?;
        state.panel_notification_clock_unix_ms = now_unix_ms;
        recover_expired_panel_notification_claims(&mut state, now_unix_ms)?;

        let due = state
            .panel_notifications
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry.state == StoredPanelNotificationStateV1::Pending
                    && entry.available_at_unix_ms <= now_unix_ms
            })
            .map(|(index, _)| index)
            .take(limit)
            .collect::<Vec<_>>();
        let mut claims = Vec::with_capacity(due.len());
        for index in due {
            let entry = &mut state.panel_notifications[index];
            if entry.attempts >= entry.attempt_limit {
                dead_letter_panel_notification(
                    entry,
                    ModerationPanelNotificationDeadLetterReasonV1::RetryExhausted,
                    now_unix_ms,
                );
                continue;
            }
            entry.attempts = entry
                .attempts
                .checked_add(1)
                .ok_or(ModerationOrchestratorError::GenerationOverflow)?;
            entry.claim_generation = entry
                .claim_generation
                .checked_add(1)
                .ok_or(ModerationOrchestratorError::GenerationOverflow)?;
            let lease_token = panel_notification_lease_token(
                entry.notification.notification_id,
                worker_id,
                entry.claim_generation,
                entry.attempts,
                now_unix_ms,
                lease_expires_at_unix_ms,
            );
            entry.state = StoredPanelNotificationStateV1::Claimed;
            entry.claimed_by = Some(worker_id);
            entry.lease_token = Some(lease_token);
            entry.claimed_at_unix_ms = Some(now_unix_ms);
            entry.lease_expires_at_unix_ms = Some(lease_expires_at_unix_ms);
            entry.receipt_digest = None;
            entry.delivered_at_unix_ms = None;
            entry.dead_letter_reason = None;
            entry.dead_lettered_at_unix_ms = None;
            refresh_panel_notification_record_digest(entry);
            claims.push(ModerationPanelNotificationClaimV1 {
                notification: entry.notification.clone(),
                worker_id,
                lease_token,
                lease_expires_at_unix_ms,
                attempt: entry.attempts,
                attempt_limit: entry.attempt_limit,
            });
        }
        self.persist_checkpoint_locked(&mut state)?;
        Ok(claims)
    }

    /// Atomically record one stable delivery receipt under the exact live claim.
    ///
    /// Calling this again with the same worker, lease token, and receipt returns
    /// `AlreadyDelivered`; any substituted receipt or reclaimed lease fails
    /// closed.
    ///
    /// # Errors
    ///
    /// Fails for invalid receipt material, clock rollback, a missing or stale
    /// claim, receipt substitution, corrupt state, or checkpoint failure.
    pub fn finalize_panel_notification_delivery(
        &self,
        worker_id: [u8; 32],
        lease_token: [u8; 32],
        receipt: ModerationPanelNotificationDeliveryReceiptV1,
        now_unix_ms: u64,
    ) -> Result<ModerationPanelNotificationFinalizeOutcomeV1, ModerationOrchestratorError> {
        if worker_id == [0; 32]
            || lease_token == [0; 32]
            || receipt.notification_id == [0; 32]
            || receipt.receipt_digest == [0; 32]
            || receipt.delivered_at_unix_ms == 0
            || now_unix_ms == 0
            || receipt.delivered_at_unix_ms > now_unix_ms
        {
            return Err(ModerationOrchestratorError::InvalidPanelNotificationReceipt);
        }
        let mut state = self.lock_state()?;
        validate_panel_notification_clock(&state, now_unix_ms)?;
        let position = state
            .panel_notifications
            .iter()
            .position(|entry| entry.notification.notification_id == receipt.notification_id)
            .ok_or(ModerationOrchestratorError::PanelNotificationNotFound {
                notification_id: receipt.notification_id,
            })?;
        let entry = &state.panel_notifications[position];

        if entry.state == StoredPanelNotificationStateV1::Delivered {
            if entry.claimed_by == Some(worker_id)
                && entry.lease_token == Some(lease_token)
                && entry.receipt_digest == Some(receipt.receipt_digest)
                && entry.delivered_at_unix_ms == Some(receipt.delivered_at_unix_ms)
            {
                state.panel_notification_clock_unix_ms = now_unix_ms;
                self.persist_checkpoint_locked(&mut state)?;
                return Ok(ModerationPanelNotificationFinalizeOutcomeV1::AlreadyDelivered);
            }
            return Err(
                ModerationOrchestratorError::PanelNotificationReceiptConflict {
                    notification_id: receipt.notification_id,
                },
            );
        }

        let live_claim = entry.state == StoredPanelNotificationStateV1::Claimed
            && entry.claimed_by == Some(worker_id)
            && entry.lease_token == Some(lease_token)
            && receipt.delivered_at_unix_ms >= entry.notification.source_occurred_at_unix_ms
            && entry
                .lease_expires_at_unix_ms
                .is_some_and(|expires_at| now_unix_ms < expires_at);
        if !live_claim {
            return Err(
                ModerationOrchestratorError::PanelNotificationClaimConflict {
                    notification_id: receipt.notification_id,
                },
            );
        }

        state.panel_notification_clock_unix_ms = now_unix_ms;
        let entry = &mut state.panel_notifications[position];
        entry.state = StoredPanelNotificationStateV1::Delivered;
        entry.receipt_digest = Some(receipt.receipt_digest);
        entry.delivered_at_unix_ms = Some(receipt.delivered_at_unix_ms);
        entry.dead_letter_reason = None;
        entry.dead_lettered_at_unix_ms = None;
        refresh_panel_notification_record_digest(entry);
        self.persist_checkpoint_locked(&mut state)?;
        Ok(ModerationPanelNotificationFinalizeOutcomeV1::Delivered)
    }

    /// Release one live claim after a fixed, payload-free delivery result.
    ///
    /// Safe/ambiguous failures use deterministic bounded exponential backoff;
    /// permanent failures and exhausted attempts become durable dead letters.
    ///
    /// # Errors
    ///
    /// Fails for a missing/stale claim, clock rollback, arithmetic exhaustion,
    /// corrupt state, or checkpoint failure.
    pub fn release_panel_notification_claim(
        &self,
        notification_id: [u8; 32],
        worker_id: [u8; 32],
        lease_token: [u8; 32],
        failure: ModerationPanelNotificationFailureV1,
        now_unix_ms: u64,
    ) -> Result<(), ModerationOrchestratorError> {
        if notification_id == [0; 32]
            || worker_id == [0; 32]
            || lease_token == [0; 32]
            || now_unix_ms == 0
        {
            return Err(ModerationOrchestratorError::InvalidPanelNotificationClaim);
        }
        let mut state = self.lock_state()?;
        validate_panel_notification_clock(&state, now_unix_ms)?;
        let position = state
            .panel_notifications
            .iter()
            .position(|entry| entry.notification.notification_id == notification_id)
            .ok_or(ModerationOrchestratorError::PanelNotificationNotFound { notification_id })?;
        let entry = &state.panel_notifications[position];
        if entry.state == StoredPanelNotificationStateV1::Delivered
            && entry.claimed_by == Some(worker_id)
            && entry.lease_token == Some(lease_token)
        {
            state.panel_notification_clock_unix_ms = now_unix_ms;
            self.persist_checkpoint_locked(&mut state)?;
            return Ok(());
        }
        let live_claim = entry.state == StoredPanelNotificationStateV1::Claimed
            && entry.claimed_by == Some(worker_id)
            && entry.lease_token == Some(lease_token)
            && entry
                .lease_expires_at_unix_ms
                .is_some_and(|expires_at| now_unix_ms < expires_at);
        if !live_claim {
            return Err(
                ModerationOrchestratorError::PanelNotificationClaimConflict { notification_id },
            );
        }
        let retry_available_at_unix_ms = match failure {
            ModerationPanelNotificationFailureV1::NotDelivered
            | ModerationPanelNotificationFailureV1::Ambiguous
                if entry.attempts < entry.attempt_limit =>
            {
                Some(
                    now_unix_ms
                        .checked_add(panel_notification_backoff_ms(entry.attempts))
                        .ok_or(ModerationOrchestratorError::GenerationOverflow)?,
                )
            }
            ModerationPanelNotificationFailureV1::NotDelivered
            | ModerationPanelNotificationFailureV1::Ambiguous
            | ModerationPanelNotificationFailureV1::Permanent => None,
        };

        state.panel_notification_clock_unix_ms = now_unix_ms;
        let entry = &mut state.panel_notifications[position];
        match (failure, retry_available_at_unix_ms) {
            (ModerationPanelNotificationFailureV1::Permanent, _) => {
                dead_letter_panel_notification(
                    entry,
                    ModerationPanelNotificationDeadLetterReasonV1::PermanentRejection,
                    now_unix_ms,
                );
            }
            (
                ModerationPanelNotificationFailureV1::NotDelivered
                | ModerationPanelNotificationFailureV1::Ambiguous,
                None,
            ) => {
                dead_letter_panel_notification(
                    entry,
                    ModerationPanelNotificationDeadLetterReasonV1::RetryExhausted,
                    now_unix_ms,
                );
            }
            (
                ModerationPanelNotificationFailureV1::NotDelivered
                | ModerationPanelNotificationFailureV1::Ambiguous,
                Some(available_at_unix_ms),
            ) => {
                entry.available_at_unix_ms = available_at_unix_ms;
                entry.state = StoredPanelNotificationStateV1::Pending;
                clear_panel_notification_claim(entry);
                entry.receipt_digest = None;
                entry.delivered_at_unix_ms = None;
                entry.dead_letter_reason = None;
                entry.dead_lettered_at_unix_ms = None;
                refresh_panel_notification_record_digest(entry);
            }
        }
        self.persist_checkpoint_locked(&mut state)
    }

    /// Return the durable payload-free state for one notification.
    ///
    /// # Errors
    ///
    /// Fails after a durability fault or poisoned state lock.
    pub fn panel_notification_status(
        &self,
        notification_id: [u8; 32],
    ) -> Result<Option<ModerationPanelNotificationStatusV1>, ModerationOrchestratorError> {
        let state = self.lock_state()?;
        state
            .panel_notifications
            .iter()
            .find(|entry| entry.notification.notification_id == notification_id)
            .map(panel_notification_status)
            .transpose()
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
        if let Err(error) = persist_checkpoint(&self.config, &self.chain_id, state) {
            self.durability_faulted.store(true, Ordering::Release);
            return Err(error);
        }
        Ok(())
    }

    fn read_validated_finalized_snapshot(
        &self,
    ) -> Result<(ModerationFinalizedLedgerSnapshotV1, [u8; 32]), ModerationOrchestratorError> {
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
        validate_panel_notification_source_provenance(&snapshot)?;
        let digest = finalized_snapshot_digest(&snapshot)?;
        Ok((snapshot, digest))
    }

    fn install_finalized_snapshot_locked(
        &self,
        state: &mut ModerationOrchestratorCheckpointV1,
        snapshot: ModerationFinalizedLedgerSnapshotV1,
        digest: [u8; 32],
    ) -> Result<(), ModerationOrchestratorError> {
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
            if snapshot.finalized_at_unix_ms < previous.finalized_at_unix_ms {
                return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                    "finalized moderation time regressed".to_owned(),
                ));
            }
            if snapshot.finalized_height == previous.finalized_height
                && (snapshot.finalized_block_hash != previous.finalized_block_hash
                    || digest != previous_digest)
            {
                return Err(ModerationOrchestratorError::FinalizedEquivocation {
                    height: snapshot.finalized_height,
                });
            }
        }
        let previous_snapshot = state.finalized_snapshot.replace(snapshot);
        let previous_snapshot_digest = state.finalized_snapshot_digest.replace(digest);
        if let Err(error) = self.queue_panel_notifications_locked(state) {
            state.finalized_snapshot = previous_snapshot;
            state.finalized_snapshot_digest = previous_snapshot_digest;
            return Err(error);
        }
        recover_expired_external_work_claims(state)?;
        self.reconcile_outbox_authoritative_locked(state)?;
        self.queue_terminal_handoffs_locked(state)?;
        self.persist_checkpoint_locked(state)
    }

    fn reconcile_outbox_authoritative_locked(
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
        let effects = state
            .outbox
            .iter()
            .map(|entry| action_effect(&snapshot, &entry.authority, &entry.action))
            .collect::<Result<Vec<_>, _>>()?;
        ensure_dead_letter_capacity(
            state,
            &self.config,
            effects
                .iter()
                .filter(|effect| **effect == ActionEffect::Conflict)
                .count(),
        )?;
        let mut retained = Vec::with_capacity(state.outbox.len());
        let mut dead = Vec::new();
        for (entry, effect) in std::mem::take(&mut state.outbox).into_iter().zip(effects) {
            let operation_position = operation_index
                .get(&entry.operation_id)
                .copied()
                .ok_or_else(|| {
                    ModerationOrchestratorError::CheckpointCorrupt(
                        "outbox entry has no idempotency record".to_owned(),
                    )
                })?;
            match effect {
                ActionEffect::Exact => {
                    let operation = &mut state.operations[operation_position];
                    operation.status = StoredOperationStatusV1::Finalized;
                    if operation.transaction_id.is_none() {
                        operation.transaction_id =
                            retired_history_fence_transaction_id(&entry).or(entry.transaction_id);
                    }
                }
                ActionEffect::Conflict => {
                    state.operations[operation_position].status = StoredOperationStatusV1::Rejected;
                    dead.push(StoredDeadLetterV1 {
                        identity: entry.operation_id,
                        action_label: entry.action.label().to_owned(),
                        reason: StoredDeadLetterReasonV1::FinalizedConflict,
                        finalized_cursor: cursor,
                    });
                }
                ActionEffect::Absent => {
                    state.operations[operation_position].transaction_id =
                        retired_history_fence_transaction_id(&entry).or(entry.transaction_id);
                    retained.push(entry);
                }
            }
        }
        state.outbox = retained;
        state.dead_letters.extend(dead);
        Ok(())
    }

    fn drive_external_work(&self) -> Result<(), ModerationOrchestratorError> {
        let mut attempted = BTreeSet::new();
        let mut deferred_operations = BTreeSet::new();
        loop {
            let prepared = {
                let mut state = self.lock_state()?;
                self.prepare_next_external_work_locked(
                    &mut state,
                    &attempted,
                    &deferred_operations,
                )?
            };
            let Some(prepared) = prepared else {
                return Ok(());
            };
            let identity = prepared.identity();
            attempted.insert(identity);
            self.execute_external_work(prepared)?;
            if identity.kind == StoredExternalWorkKindV1::Submit {
                deferred_operations.insert(identity.identity);
            }
        }
    }

    fn prepare_next_external_work_locked(
        &self,
        state: &mut ModerationOrchestratorCheckpointV1,
        attempted: &BTreeSet<ExternalWorkIdentityV1>,
        deferred_operations: &BTreeSet<[u8; 32]>,
    ) -> Result<Option<PreparedExternalWorkV1>, ModerationOrchestratorError> {
        let recovered = recover_expired_external_work_claims(state)?;
        let snapshot = state
            .finalized_snapshot
            .as_ref()
            .ok_or(ModerationOrchestratorError::FinalizedReaderUnavailable)?;
        let cursor = snapshot.anchor();
        let finalized_at_unix_ms = snapshot.finalized_at_unix_ms;
        let mut position = 0;
        while position < state.outbox.len() {
            if deferred_operations.contains(&state.outbox[position].operation_id) {
                position += 1;
                continue;
            }
            if state.outbox[position].work_claim.is_some() {
                position += 1;
                continue;
            }
            if state.outbox[position].state == StoredOutboxStateV1::Signing {
                return Err(ModerationOrchestratorError::CheckpointCorrupt(
                    "signing outbox entry has no external-work claim".to_owned(),
                ));
            }

            let entry = &state.outbox[position];
            let complete_transaction = entry.transaction_id.is_some()
                && entry.signed_transaction_digest.is_some()
                && entry.signed_transaction_bytes.is_some();
            let expired = if complete_transaction {
                let request = moderation_transaction_request(&self.chain_id, entry)?;
                let signed = moderation_signed_transaction(entry)?;
                let transaction = signed.decode_for_request(&request).map_err(|_| {
                    ModerationOrchestratorError::CheckpointCorrupt(
                        "active outbox envelope failed exact validation".to_owned(),
                    )
                })?;
                finalized_at_unix_ms >= signed_envelope_timing(&transaction)?.expires_at_unix_ms
            } else {
                false
            };
            let unresolved_retired = entry.retired_envelopes.iter().any(|record| {
                matches!(
                    record.disposition,
                    StoredRetiredEnvelopeDispositionV1::NotFound
                        | StoredRetiredEnvelopeDispositionV1::Pending
                )
            });
            let include_active_lookup = matches!(
                entry.state,
                StoredOutboxStateV1::Ambiguous | StoredOutboxStateV1::Submitted
            ) || (entry.state == StoredOutboxStateV1::Signed
                && expired);
            let lookup_needed =
                complete_transaction && (unresolved_retired || include_active_lookup);
            if lookup_needed {
                let work_digest = outbox_lookup_work_digest(entry, cursor);
                let identity = ExternalWorkIdentityV1 {
                    identity: entry.operation_id,
                    kind: StoredExternalWorkKindV1::Lookup,
                    work_digest,
                };
                if !attempted.contains(&identity) {
                    let mut candidate = entry.clone();
                    let generation = next_external_work_generation(candidate.work_generation)?;
                    let claim = external_work_claim(
                        StoredExternalWorkKindV1::Lookup,
                        candidate.operation_id,
                        generation,
                        work_digest,
                        cursor,
                        finalized_at_unix_ms,
                    )?;
                    let mut probes = candidate
                        .retired_envelopes
                        .iter()
                        .map(|record| ExternalLookupProbeV1 {
                            transaction_id: record.transaction_id,
                        })
                        .collect::<Vec<_>>();
                    if include_active_lookup && let Some(transaction_id) = candidate.transaction_id
                    {
                        probes.push(ExternalLookupProbeV1 { transaction_id });
                    }
                    candidate.work_generation = generation;
                    candidate.work_claim = Some(claim.clone());
                    state.outbox[position] = candidate;
                    self.persist_checkpoint_locked(state)?;
                    return Ok(Some(PreparedExternalWorkV1::Lookup {
                        identity,
                        claim,
                        operation_id: state.outbox[position].operation_id,
                        probes,
                    }));
                }
            }

            if retired_history_fence_transaction_id(entry).is_some() {
                position += 1;
                continue;
            }
            match entry.state {
                StoredOutboxStateV1::Ready => {
                    let mut candidate = entry.clone();
                    candidate.baseline_finalized_height = cursor.height;
                    candidate.baseline_finalized_block_hash = cursor.block_hash;
                    candidate.state = StoredOutboxStateV1::Signing;
                    let request = moderation_transaction_request(&self.chain_id, &candidate)?;
                    let work_digest = outbox_sign_work_digest(&candidate);
                    let identity = ExternalWorkIdentityV1 {
                        identity: candidate.operation_id,
                        kind: StoredExternalWorkKindV1::Sign,
                        work_digest,
                    };
                    if attempted.contains(&identity) {
                        position += 1;
                        continue;
                    }
                    let generation = next_external_work_generation(candidate.work_generation)?;
                    let claim = external_work_claim(
                        StoredExternalWorkKindV1::Sign,
                        candidate.operation_id,
                        generation,
                        work_digest,
                        cursor,
                        finalized_at_unix_ms,
                    )?;
                    candidate.work_generation = generation;
                    candidate.work_claim = Some(claim.clone());
                    state.outbox[position] = candidate;
                    self.persist_checkpoint_locked(state)?;
                    return Ok(Some(PreparedExternalWorkV1::Sign {
                        identity,
                        claim,
                        request,
                    }));
                }
                StoredOutboxStateV1::Signed if !expired => {
                    if entry.attempts >= self.config.max_submit_attempts {
                        self.dead_letter_submission_locked(
                            state,
                            position,
                            StoredDeadLetterReasonV1::RetryExhaustedNotFound,
                        )?;
                        continue;
                    }
                    let mut candidate = entry.clone();
                    let request = moderation_transaction_request(&self.chain_id, &candidate)?;
                    let signed = moderation_signed_transaction(&candidate)?;
                    signed.decode_for_request(&request).map_err(|_| {
                        ModerationOrchestratorError::CheckpointCorrupt(
                            "retained signed transaction failed exact validation".to_owned(),
                        )
                    })?;
                    candidate.attempts = candidate
                        .attempts
                        .checked_add(1)
                        .ok_or(ModerationOrchestratorError::GenerationOverflow)?;
                    candidate.state = StoredOutboxStateV1::Ambiguous;
                    let work_digest = outbox_submit_work_digest(&candidate);
                    let identity = ExternalWorkIdentityV1 {
                        identity: candidate.operation_id,
                        kind: StoredExternalWorkKindV1::Submit,
                        work_digest,
                    };
                    if attempted.contains(&identity) {
                        position += 1;
                        continue;
                    }
                    let generation = next_external_work_generation(candidate.work_generation)?;
                    let claim = external_work_claim(
                        StoredExternalWorkKindV1::Submit,
                        candidate.operation_id,
                        generation,
                        work_digest,
                        cursor,
                        finalized_at_unix_ms,
                    )?;
                    candidate.work_generation = generation;
                    candidate.work_claim = Some(claim.clone());
                    state.outbox[position] = candidate;
                    self.persist_checkpoint_locked(state)?;
                    return Ok(Some(PreparedExternalWorkV1::Submit {
                        identity,
                        claim,
                        request,
                        signed,
                    }));
                }
                StoredOutboxStateV1::Signing
                | StoredOutboxStateV1::Signed
                | StoredOutboxStateV1::Ambiguous
                | StoredOutboxStateV1::Submitted => {
                    position += 1;
                }
            }
        }

        for position in 0..state.pending_handoffs.len() {
            let entry = &state.pending_handoffs[position];
            if entry.work_claim.is_some() {
                continue;
            }
            let work_digest = handoff_work_digest(&entry.handoff);
            let identity = ExternalWorkIdentityV1 {
                identity: entry.handoff.handoff_id,
                kind: StoredExternalWorkKindV1::Handoff,
                work_digest,
            };
            if attempted.contains(&identity) {
                continue;
            }
            let mut candidate = entry.clone();
            if candidate.attempts < self.config.max_submit_attempts {
                candidate.attempts = candidate
                    .attempts
                    .checked_add(1)
                    .ok_or(ModerationOrchestratorError::GenerationOverflow)?;
            }
            let generation = next_external_work_generation(candidate.work_generation)?;
            let claim = external_work_claim(
                StoredExternalWorkKindV1::Handoff,
                candidate.handoff.handoff_id,
                generation,
                work_digest,
                cursor,
                finalized_at_unix_ms,
            )?;
            candidate.work_generation = generation;
            candidate.work_claim = Some(claim.clone());
            let handoff = candidate.handoff.clone();
            state.pending_handoffs[position] = candidate;
            self.persist_checkpoint_locked(state)?;
            return Ok(Some(PreparedExternalWorkV1::Handoff {
                identity,
                claim,
                handoff,
            }));
        }
        if recovered {
            self.persist_checkpoint_locked(state)?;
        }
        Ok(None)
    }

    fn execute_external_work(
        &self,
        prepared: PreparedExternalWorkV1,
    ) -> Result<(), ModerationOrchestratorError> {
        match prepared {
            PreparedExternalWorkV1::Sign { claim, request, .. } => {
                match self.deps.submitter.sign(&request) {
                    Ok(signed) => {
                        let validated = signed
                            .decode_for_request(&request)
                            .map_err(|_| {
                                ModerationOrchestratorError::InvalidAction(
                                    "runtime signer returned an invalid exact transaction"
                                        .to_owned(),
                                )
                            })
                            .and_then(|transaction| signed_envelope_timing(&transaction));
                        match validated {
                            Ok(timing) => self.finalize_sign_work(&claim, signed, timing),
                            Err(error) => {
                                self.finalize_invalid_sign_work(&claim)?;
                                Err(error)
                            }
                        }
                    }
                    Err(failure) => self.finalize_sign_failure(&claim, failure),
                }
            }
            PreparedExternalWorkV1::Submit {
                claim,
                request,
                signed,
                ..
            } => {
                let result = self.deps.submitter.submit_signed(&request, &signed);
                self.finalize_submit_work(&claim, &signed, result)
            }
            PreparedExternalWorkV1::Lookup {
                claim,
                operation_id,
                probes,
                ..
            } => {
                let observations = probes
                    .into_iter()
                    .map(|probe| {
                        let lookup = self
                            .deps
                            .submitter
                            .lookup(operation_id, Some(probe.transaction_id));
                        (probe, lookup)
                    })
                    .collect::<Vec<_>>();
                self.finalize_lookup_work(&claim, observations)
            }
            PreparedExternalWorkV1::Handoff { claim, handoff, .. } => {
                let result = match handoff.kind {
                    ModerationTerminalHandoffKindV1::Settlement => {
                        self.deps.settlement_sink.deliver(&handoff)
                    }
                    ModerationTerminalHandoffKindV1::Publication => {
                        self.deps.publication_sink.deliver(&handoff)
                    }
                };
                self.finalize_handoff_work(&claim, result)
            }
        }
    }

    fn finalize_sign_work(
        &self,
        claim: &StoredExternalWorkClaimV1,
        signed: ModerationSignedTransactionV1,
        timing: SignedEnvelopeTimingV1,
    ) -> Result<(), ModerationOrchestratorError> {
        let mut state = self.lock_state()?;
        let Some(position) = outbox_claim_position(&state, StoredExternalWorkKindV1::Sign, claim)
        else {
            return Ok(());
        };
        let duplicate_or_stale = state.outbox[position]
            .retired_envelopes
            .iter()
            .any(|record| {
                record.transaction_id == signed.transaction_id
                    || record.signed_transaction_digest == signed.canonical_bytes_digest
            })
            || state.outbox[position]
                .retired_envelopes
                .last()
                .is_some_and(|previous| {
                    timing.created_at_unix_ms <= previous.created_at_unix_ms
                        || timing.expires_at_unix_ms <= previous.retired_at_finalized_unix_ms
                });
        if duplicate_or_stale {
            reset_sign_claim(&mut state.outbox[position]);
            self.persist_checkpoint_locked(&mut state)?;
            return Err(ModerationOrchestratorError::InvalidAction(
                "runtime signer did not advance the retired envelope generation".to_owned(),
            ));
        }
        state.outbox[position].transaction_id = Some(signed.transaction_id);
        state.outbox[position].signed_transaction_digest = Some(signed.canonical_bytes_digest);
        state.outbox[position].signed_transaction_bytes = Some(signed.canonical_bytes);
        state.outbox[position].state = StoredOutboxStateV1::Signed;
        state.outbox[position].work_claim = None;
        state.outbox[position].last_lookup_finalized_height = 0;
        state.outbox[position].last_lookup_finalized_block_hash = [0; 32];
        let operation_id = state.outbox[position].operation_id;
        let transaction_id = state.outbox[position].transaction_id;
        if let Some(operation) = state
            .operations
            .iter_mut()
            .find(|operation| operation.operation_id == operation_id)
        {
            operation.transaction_id = transaction_id;
        }
        self.persist_checkpoint_locked(&mut state)
    }

    fn finalize_invalid_sign_work(
        &self,
        claim: &StoredExternalWorkClaimV1,
    ) -> Result<(), ModerationOrchestratorError> {
        let mut state = self.lock_state()?;
        if let Some(position) = outbox_claim_position(&state, StoredExternalWorkKindV1::Sign, claim)
        {
            reset_sign_claim(&mut state.outbox[position]);
            self.persist_checkpoint_locked(&mut state)?;
        }
        Ok(())
    }

    fn finalize_sign_failure(
        &self,
        claim: &StoredExternalWorkClaimV1,
        failure: ModerationSubmissionFailureV1,
    ) -> Result<(), ModerationOrchestratorError> {
        let mut state = self.lock_state()?;
        let Some(position) = outbox_claim_position(&state, StoredExternalWorkKindV1::Sign, claim)
        else {
            return Ok(());
        };
        match failure {
            ModerationSubmissionFailureV1::PermanentRejection => self
                .dead_letter_submission_locked(
                    &mut state,
                    position,
                    StoredDeadLetterReasonV1::PermanentRejection,
                ),
            ModerationSubmissionFailureV1::NotSubmittedUnavailable
            | ModerationSubmissionFailureV1::NotSubmittedBackpressure
            | ModerationSubmissionFailureV1::Ambiguous
            | ModerationSubmissionFailureV1::RuntimeUnavailable => {
                reset_sign_claim(&mut state.outbox[position]);
                self.persist_checkpoint_locked(&mut state)
            }
        }
    }

    fn finalize_submit_work(
        &self,
        claim: &StoredExternalWorkClaimV1,
        signed: &ModerationSignedTransactionV1,
        result: Result<ModerationTransactionReceiptV1, ModerationSubmissionFailureV1>,
    ) -> Result<(), ModerationOrchestratorError> {
        let mut state = self.lock_state()?;
        let Some(position) = outbox_claim_position(&state, StoredExternalWorkKindV1::Submit, claim)
        else {
            return Ok(());
        };
        state.outbox[position].work_claim = None;
        match result {
            Ok(receipt)
                if receipt.transaction_id == signed.transaction_id
                    && receipt.observed_finalized_height
                        >= state.outbox[position].baseline_finalized_height =>
            {
                state.outbox[position].state = StoredOutboxStateV1::Submitted;
            }
            Ok(_) => {
                state.outbox[position].state = StoredOutboxStateV1::Ambiguous;
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
                state.outbox[position].state = StoredOutboxStateV1::Signed;
            }
            Err(ModerationSubmissionFailureV1::PermanentRejection) => {
                return self.dead_letter_submission_locked(
                    &mut state,
                    position,
                    StoredDeadLetterReasonV1::PermanentRejection,
                );
            }
        }
        self.persist_checkpoint_locked(&mut state)
    }

    fn finalize_lookup_work(
        &self,
        claim: &StoredExternalWorkClaimV1,
        observations: Vec<(ExternalLookupProbeV1, ModerationSubmissionLookupV1)>,
    ) -> Result<(), ModerationOrchestratorError> {
        let mut state = self.lock_state()?;
        let Some(position) = outbox_claim_position(&state, StoredExternalWorkKindV1::Lookup, claim)
        else {
            return Ok(());
        };
        let operation_id = state.outbox[position].operation_id;
        let operation_position = state
            .operations
            .iter()
            .position(|operation| operation.operation_id == operation_id)
            .ok_or_else(|| {
                ModerationOrchestratorError::CheckpointCorrupt(
                    "lookup outbox entry has no idempotency record".to_owned(),
                )
            })?;
        let cursor = ModerationFinalizedCursorV1 {
            height: claim.claimed_at_finalized_height,
            block_hash: claim.claimed_at_finalized_block_hash,
        };
        let mut candidate = state.outbox[position].clone();
        candidate.work_claim = None;
        candidate.last_lookup_finalized_height = cursor.height;
        candidate.last_lookup_finalized_block_hash = cursor.block_hash;
        for record in &mut candidate.retired_envelopes {
            let Some((_, lookup)) = observations
                .iter()
                .find(|(probe, _)| probe.transaction_id == record.transaction_id)
            else {
                continue;
            };
            let next = retired_envelope_disposition_after_lookup(record, *lookup, cursor);
            if next != record.disposition {
                record.disposition = next;
                refresh_retired_envelope_record_digest(operation_id, record);
            }
        }
        if let Some(fenced_transaction_id) = retired_history_fence_transaction_id(&candidate) {
            state.operations[operation_position].transaction_id = Some(fenced_transaction_id);
            state.outbox[position] = candidate;
            return self.persist_checkpoint_locked(&mut state);
        }

        let Some(expected_transaction_id) = candidate.transaction_id else {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "lookup claim lost its active transaction".to_owned(),
            ));
        };
        let Some(lookup) = observations
            .iter()
            .find(|(probe, _)| probe.transaction_id == expected_transaction_id)
            .map(|(_, lookup)| *lookup)
        else {
            state.operations[operation_position].transaction_id = Some(expected_transaction_id);
            state.outbox[position] = candidate;
            return self.persist_checkpoint_locked(&mut state);
        };
        let request = moderation_transaction_request(&self.chain_id, &candidate)?;
        let signed = moderation_signed_transaction(&candidate)?;
        let transaction = signed.decode_for_request(&request).map_err(|_| {
            ModerationOrchestratorError::CheckpointCorrupt(
                "lookup active envelope failed exact validation".to_owned(),
            )
        })?;
        let timing = signed_envelope_timing(&transaction)?;
        let expired = claim.claimed_at_unix_ms >= timing.expires_at_unix_ms;
        let mut dead_reason = None;
        match lookup {
            ModerationSubmissionLookupV1::Pending { transaction_id }
            | ModerationSubmissionLookupV1::Applied { transaction_id }
                if transaction_id == expected_transaction_id =>
            {
                candidate.state = StoredOutboxStateV1::Submitted;
                state.operations[operation_position].transaction_id = Some(expected_transaction_id);
            }
            ModerationSubmissionLookupV1::Rejected {
                transaction_id,
                observed_finalized_height,
            } if transaction_id
                .is_none_or(|transaction_id| transaction_id == expected_transaction_id)
                && observed_finalized_height > candidate.baseline_finalized_height
                && observed_finalized_height <= cursor.height =>
            {
                dead_reason = Some(StoredDeadLetterReasonV1::PermanentRejection);
            }
            ModerationSubmissionLookupV1::NotFound {
                observed_finalized_height,
            } if observed_finalized_height > candidate.baseline_finalized_height
                && observed_finalized_height <= cursor.height =>
            {
                if candidate.attempts >= self.config.max_submit_attempts {
                    dead_reason = Some(StoredDeadLetterReasonV1::RetryExhaustedNotFound);
                } else if expired {
                    let next_generation = next_envelope_generation(candidate.envelope_generation)?;
                    if next_generation > self.config.max_submit_attempts {
                        dead_reason = Some(StoredDeadLetterReasonV1::RetryExhaustedNotFound);
                    } else if observed_finalized_height == cursor.height {
                        self.retire_expired_envelope(
                            &mut candidate,
                            &signed,
                            timing,
                            cursor,
                            claim.claimed_at_unix_ms,
                            next_generation,
                        )?;
                        state.operations[operation_position].transaction_id = None;
                    } else {
                        candidate.state = StoredOutboxStateV1::Ambiguous;
                    }
                } else {
                    candidate.baseline_finalized_height = cursor.height;
                    candidate.baseline_finalized_block_hash = cursor.block_hash;
                    candidate.state = StoredOutboxStateV1::Signed;
                }
            }
            ModerationSubmissionLookupV1::Unknown
            | ModerationSubmissionLookupV1::Pending { .. }
            | ModerationSubmissionLookupV1::Applied { .. }
            | ModerationSubmissionLookupV1::Rejected { .. }
            | ModerationSubmissionLookupV1::NotFound { .. } => {
                candidate.state = StoredOutboxStateV1::Ambiguous;
            }
        }
        state.outbox[position] = candidate;
        if let Some(reason) = dead_reason {
            state.operations[operation_position].transaction_id = Some(expected_transaction_id);
            return self.dead_letter_submission_locked(&mut state, position, reason);
        }
        self.persist_checkpoint_locked(&mut state)
    }

    fn finalize_handoff_work(
        &self,
        claim: &StoredExternalWorkClaimV1,
        result: Result<(), ModerationHandoffFailureV1>,
    ) -> Result<(), ModerationOrchestratorError> {
        let mut state = self.lock_state()?;
        let Some(position) = handoff_claim_position(&state, claim) else {
            return Ok(());
        };
        let cursor = snapshot_cursor(&state)?;
        let needs_dead_letter = matches!(result, Err(ModerationHandoffFailureV1::Permanent))
            || (matches!(
                result,
                Err(ModerationHandoffFailureV1::NotDelivered
                    | ModerationHandoffFailureV1::Ambiguous)
            ) && state.pending_handoffs[position].attempts >= self.config.max_submit_attempts);
        if needs_dead_letter {
            ensure_dead_letter_capacity(&state, &self.config, 1)?;
        }
        let mut entry = state.pending_handoffs.remove(position);
        entry.work_claim = None;
        match result {
            Ok(()) => {
                state.completed_handoffs.push(entry.handoff.handoff_id);
                state.completed_handoffs.sort_unstable();
                state.completed_handoffs.dedup();
            }
            Err(ModerationHandoffFailureV1::Permanent) => {
                state.dead_letters.push(StoredDeadLetterV1 {
                    identity: entry.handoff.handoff_id,
                    action_label: handoff_label(entry.handoff.kind).to_owned(),
                    reason: StoredDeadLetterReasonV1::HandoffPermanentRejection,
                    finalized_cursor: cursor,
                });
            }
            Err(
                ModerationHandoffFailureV1::NotDelivered | ModerationHandoffFailureV1::Ambiguous,
            ) if entry.attempts >= self.config.max_submit_attempts => {
                state.dead_letters.push(StoredDeadLetterV1 {
                    identity: entry.handoff.handoff_id,
                    action_label: handoff_label(entry.handoff.kind).to_owned(),
                    reason: StoredDeadLetterReasonV1::HandoffRetryExhausted,
                    finalized_cursor: cursor,
                });
            }
            Err(
                ModerationHandoffFailureV1::NotDelivered | ModerationHandoffFailureV1::Ambiguous,
            ) => {
                state.pending_handoffs.insert(position, entry);
            }
        }
        self.persist_checkpoint_locked(&mut state)
    }

    fn retire_expired_envelope(
        &self,
        entry: &mut StoredOutboxEntryV1,
        signed: &ModerationSignedTransactionV1,
        timing: SignedEnvelopeTimingV1,
        cursor: ModerationFinalizedCursorV1,
        finalized_at_unix_ms: u64,
        next_generation: u32,
    ) -> Result<(), ModerationOrchestratorError> {
        if next_generation != next_envelope_generation(entry.envelope_generation)?
            || next_generation > self.config.max_submit_attempts
        {
            return Err(ModerationOrchestratorError::GenerationOverflow);
        }
        if finalized_at_unix_ms < timing.expires_at_unix_ms {
            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "cannot retire an unexpired moderation envelope".to_owned(),
            ));
        }
        let mut retired = StoredRetiredEnvelopeV1 {
            generation: entry.envelope_generation,
            transaction_id: signed.transaction_id,
            signed_transaction_digest: signed.canonical_bytes_digest,
            created_at_unix_ms: timing.created_at_unix_ms,
            expires_at_unix_ms: timing.expires_at_unix_ms,
            retired_at_finalized_height: cursor.height,
            retired_at_finalized_block_hash: cursor.block_hash,
            retired_at_finalized_unix_ms: finalized_at_unix_ms,
            disposition: StoredRetiredEnvelopeDispositionV1::NotFound,
            record_digest: [0; 32],
        };
        refresh_retired_envelope_record_digest(entry.operation_id, &mut retired);

        entry.retired_envelopes.push(retired);
        entry.envelope_generation = next_generation;
        entry.baseline_finalized_height = 0;
        entry.baseline_finalized_block_hash = [0; 32];
        entry.transaction_id = None;
        entry.signed_transaction_digest = None;
        entry.signed_transaction_bytes = None;
        entry.state = StoredOutboxStateV1::Ready;
        entry.work_claim = None;
        entry.last_lookup_finalized_height = 0;
        entry.last_lookup_finalized_block_hash = [0; 32];
        Ok(())
    }

    fn dead_letter_submission_locked(
        &self,
        state: &mut ModerationOrchestratorCheckpointV1,
        position: usize,
        reason: StoredDeadLetterReasonV1,
    ) -> Result<(), ModerationOrchestratorError> {
        ensure_dead_letter_capacity(state, &self.config, 1)?;
        let cursor = snapshot_cursor(state)?;
        let entry = state.outbox.remove(position);
        if let Some(operation) = state
            .operations
            .iter_mut()
            .find(|operation| operation.operation_id == entry.operation_id)
        {
            operation.status = StoredOperationStatusV1::Rejected;
            if operation.transaction_id.is_none() {
                operation.transaction_id = entry.transaction_id;
            }
        }
        state.dead_letters.push(StoredDeadLetterV1 {
            identity: entry.operation_id,
            action_label: entry.action.label().to_owned(),
            reason,
            finalized_cursor: cursor,
        });
        self.persist_checkpoint_locked(state)
    }

    fn queue_panel_notifications_locked(
        &self,
        state: &mut ModerationOrchestratorCheckpointV1,
    ) -> Result<(), ModerationOrchestratorError> {
        let snapshot = state
            .finalized_snapshot
            .as_ref()
            .ok_or(ModerationOrchestratorError::FinalizedReaderUnavailable)?;
        let existing = state
            .panel_notifications
            .iter()
            .map(|entry| entry.notification.notification_id)
            .collect::<BTreeSet<_>>();
        let scanned_cursor = state.panel_notification_scanned_cursor;
        let new_events = snapshot
            .events
            .iter()
            .filter(|event| scanned_cursor.is_none_or(|cursor| event.sequence > cursor.sequence))
            .collect::<Vec<_>>();
        if let (Some(scanned), Some(first)) = (scanned_cursor, new_events.first())
            && scanned
                .sequence
                .checked_add(1)
                .is_none_or(|expected| first.sequence != expected)
        {
            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "panel notification event scan contains a sequence gap".to_owned(),
            ));
        }
        let new_scanned_cursor = new_events.last().map(|event| event.cursor());
        let mut additions = Vec::new();
        let mut added = BTreeSet::new();

        for event in new_events {
            let (Some(case_id), Some(round_id)) = (
                event.event.case_id().as_deref(),
                event.event.round_id().as_deref(),
            ) else {
                continue;
            };
            let scope_digest = panel_notification_scope_digest(case_id, round_id);
            match *event.event.kind() {
                SorafsModerationLedgerEventKind::SortitionFinalized => {
                    let appeal = snapshot.appeal(case_id, round_id).ok_or_else(|| {
                        ModerationOrchestratorError::InvalidFinalizedSnapshot(
                            "sortition event has no authoritative appeal".to_owned(),
                        )
                    })?;
                    let selection = appeal.appeal.selection.as_ref().ok_or_else(|| {
                        ModerationOrchestratorError::InvalidFinalizedSnapshot(
                            "sortition event has no authoritative selection".to_owned(),
                        )
                    })?;
                    if selection.selected_at_unix_ms != *event.event.occurred_at_unix_ms()
                        || &selection.selected_by != event.event.authority()
                    {
                        return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                            "sortition event provenance differs from the authoritative selection"
                                .to_owned(),
                        ));
                    }
                    let action = ModerationNativeActionV1::FinalizeSortition(
                        FinalizeSorafsModerationSortition::new(
                            case_id.to_owned(),
                            round_id.to_owned(),
                            appeal.appeal.pop_snapshot_digest,
                            selection.randomness_anchor,
                            selection.jurors.clone(),
                            selection.waitlist.clone(),
                        ),
                    );
                    let source_operation_id =
                        action.operation_id(&self.chain_id, event.event.authority())?;
                    for (kind, recipients) in [
                        (
                            ModerationPanelNotificationKindV1::PrimaryAssignment,
                            selection.jurors.as_slice(),
                        ),
                        (
                            ModerationPanelNotificationKindV1::WaitlistStandby,
                            selection.waitlist.as_slice(),
                        ),
                    ] {
                        for recipient in recipients {
                            let entry = new_panel_notification_entry(
                                &self.chain_id,
                                source_operation_id,
                                scope_digest,
                                kind,
                                recipient.clone(),
                                event,
                                self.config.max_submit_attempts,
                            );
                            let notification_id = entry.notification.notification_id;
                            if !existing.contains(&notification_id) && added.insert(notification_id)
                            {
                                additions.push(entry);
                            }
                        }
                    }
                }
                SorafsModerationLedgerEventKind::CaseActivated => {
                    let appeal = snapshot.appeal(case_id, round_id).ok_or_else(|| {
                        ModerationOrchestratorError::InvalidFinalizedSnapshot(
                            "activation event has no authoritative appeal".to_owned(),
                        )
                    })?;
                    let case = snapshot.case(case_id, round_id).ok_or_else(|| {
                        ModerationOrchestratorError::InvalidFinalizedSnapshot(
                            "activation event has no authoritative case".to_owned(),
                        )
                    })?;
                    let selection = appeal.appeal.selection.as_ref().ok_or_else(|| {
                        ModerationOrchestratorError::InvalidFinalizedSnapshot(
                            "activation event has no authoritative selection".to_owned(),
                        )
                    })?;
                    if case.case.opened_at_unix_ms != *event.event.occurred_at_unix_ms()
                        || &case.case.opened_by != event.event.authority()
                        || appeal.appeal.activated_at_unix_ms
                            != Some(*event.event.occurred_at_unix_ms())
                    {
                        return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                            "activation event provenance differs from the authoritative case"
                                .to_owned(),
                        ));
                    }
                    let action =
                        ModerationNativeActionV1::ActivateCase(ActivateSorafsModerationCase::new(
                            case_id.to_owned(),
                            round_id.to_owned(),
                            selection.sortition_digest,
                        ));
                    let source_operation_id =
                        action.operation_id(&self.chain_id, event.event.authority())?;
                    for recipient in &case.case.spec.jurors {
                        let entry = new_panel_notification_entry(
                            &self.chain_id,
                            source_operation_id,
                            scope_digest,
                            ModerationPanelNotificationKindV1::BallotActivated,
                            recipient.clone(),
                            event,
                            self.config.max_submit_attempts,
                        );
                        let notification_id = entry.notification.notification_id;
                        if !existing.contains(&notification_id) && added.insert(notification_id) {
                            additions.push(entry);
                        }
                    }
                }
                SorafsModerationLedgerEventKind::PolicyActivated
                | SorafsModerationLedgerEventKind::AppealSubmitted
                | SorafsModerationLedgerEventKind::EligibilityRegistered
                | SorafsModerationLedgerEventKind::SortitionFailed
                | SorafsModerationLedgerEventKind::AssignmentAccepted
                | SorafsModerationLedgerEventKind::CaseActivationFailed
                | SorafsModerationLedgerEventKind::CommitAccepted
                | SorafsModerationLedgerEventKind::ChallengeRaised
                | SorafsModerationLedgerEventKind::ChallengeResolved
                | SorafsModerationLedgerEventKind::RevealAccepted
                | SorafsModerationLedgerEventKind::CaseFinalized => {}
            }
        }
        make_panel_notification_capacity(state, additions.len(), self.config.max_handoffs)?;
        state.panel_notifications.extend(additions);
        state
            .panel_notifications
            .sort_by_key(|entry| entry.notification.notification_id);
        if let Some(cursor) = new_scanned_cursor {
            state.panel_notification_scanned_cursor = Some(cursor);
        }
        Ok(())
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
                        work_generation: 0,
                        work_claim: None,
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
        || snapshot.finalized_at_unix_ms == 0
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

fn validate_finalized_action_authority(
    snapshot: &ModerationFinalizedLedgerSnapshotV1,
    authenticated: &AccountId,
    action: &ModerationNativeActionV1,
) -> Result<(), ModerationOrchestratorError> {
    let expected = match action {
        ModerationNativeActionV1::FinalizeSortition(_) => snapshot
            .policy
            .as_ref()
            .map(|record| &record.activated_by)
            .ok_or_else(|| {
                ModerationOrchestratorError::InvalidAction(
                    "finalize_sortition requires an active finalized moderation policy".to_owned(),
                )
            })?,
        ModerationNativeActionV1::ActivateCase(value) => finalized_selection_authority(
            snapshot,
            value.case_id(),
            value.round_id(),
            action.label(),
        )?,
        ModerationNativeActionV1::ResolveChallenge(value) => finalized_selection_authority(
            snapshot,
            value.case_id(),
            value.round_id(),
            action.label(),
        )?,
        ModerationNativeActionV1::FinalizeCase(value) => finalized_selection_authority(
            snapshot,
            value.case_id(),
            value.round_id(),
            action.label(),
        )?,
        ModerationNativeActionV1::SetPolicy(_)
        | ModerationNativeActionV1::SubmitAppeal(_)
        | ModerationNativeActionV1::RegisterEligibility(_)
        | ModerationNativeActionV1::AcceptAssignment(_)
        | ModerationNativeActionV1::SubmitCommit(_)
        | ModerationNativeActionV1::RaiseChallenge(_)
        | ModerationNativeActionV1::SubmitReveal(_) => return Ok(()),
    };
    require_exact_authority(authenticated, expected, action.label())
}

fn finalized_selection_authority<'a>(
    snapshot: &'a ModerationFinalizedLedgerSnapshotV1,
    case_id: &str,
    round_id: &str,
    action: &'static str,
) -> Result<&'a AccountId, ModerationOrchestratorError> {
    snapshot
        .appeal(case_id, round_id)
        .and_then(|entry| entry.appeal.selection.as_ref())
        .map(|selection| &selection.selected_by)
        .ok_or_else(|| {
            ModerationOrchestratorError::InvalidAction(format!(
                "{action} requires a finalized panel selection for the exact case and round"
            ))
        })
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

fn validate_panel_notification_source_provenance(
    snapshot: &ModerationFinalizedLedgerSnapshotV1,
) -> Result<(), ModerationOrchestratorError> {
    for event in &snapshot.events {
        let (Some(case_id), Some(round_id)) = (
            event.event.case_id().as_deref(),
            event.event.round_id().as_deref(),
        ) else {
            continue;
        };
        match *event.event.kind() {
            SorafsModerationLedgerEventKind::SortitionFinalized => {
                let appeal = snapshot.appeal(case_id, round_id).ok_or_else(|| {
                    ModerationOrchestratorError::InvalidFinalizedSnapshot(
                        "sortition event has no authoritative appeal".to_owned(),
                    )
                })?;
                let selection = appeal.appeal.selection.as_ref().ok_or_else(|| {
                    ModerationOrchestratorError::InvalidFinalizedSnapshot(
                        "sortition event has no authoritative selection".to_owned(),
                    )
                })?;
                if selection.selected_at_unix_ms != *event.event.occurred_at_unix_ms()
                    || &selection.selected_by != event.event.authority()
                {
                    return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                        "sortition event provenance differs from the authoritative selection"
                            .to_owned(),
                    ));
                }
            }
            SorafsModerationLedgerEventKind::CaseActivated => {
                let appeal = snapshot.appeal(case_id, round_id).ok_or_else(|| {
                    ModerationOrchestratorError::InvalidFinalizedSnapshot(
                        "activation event has no authoritative appeal".to_owned(),
                    )
                })?;
                let case = snapshot.case(case_id, round_id).ok_or_else(|| {
                    ModerationOrchestratorError::InvalidFinalizedSnapshot(
                        "activation event has no authoritative case".to_owned(),
                    )
                })?;
                if appeal.appeal.selection.is_none()
                    || case.case.opened_at_unix_ms != *event.event.occurred_at_unix_ms()
                    || &case.case.opened_by != event.event.authority()
                    || appeal.appeal.activated_at_unix_ms
                        != Some(*event.event.occurred_at_unix_ms())
                {
                    return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                        "activation event provenance differs from the authoritative case"
                            .to_owned(),
                    ));
                }
            }
            SorafsModerationLedgerEventKind::PolicyActivated
            | SorafsModerationLedgerEventKind::AppealSubmitted
            | SorafsModerationLedgerEventKind::EligibilityRegistered
            | SorafsModerationLedgerEventKind::SortitionFailed
            | SorafsModerationLedgerEventKind::AssignmentAccepted
            | SorafsModerationLedgerEventKind::CaseActivationFailed
            | SorafsModerationLedgerEventKind::CommitAccepted
            | SorafsModerationLedgerEventKind::ChallengeRaised
            | SorafsModerationLedgerEventKind::ChallengeResolved
            | SorafsModerationLedgerEventKind::RevealAccepted
            | SorafsModerationLedgerEventKind::CaseFinalized => {}
        }
    }
    Ok(())
}

fn validate_retained_panel_notification_source(
    notification: &ModerationPanelNotificationV1,
    snapshot: &ModerationFinalizedLedgerSnapshotV1,
    chain_id: &iroha_data_model::ChainId,
) -> Result<(), ModerationOrchestratorError> {
    let Some(event) = snapshot
        .events
        .iter()
        .find(|event| event.sequence == notification.finalized_event_cursor.sequence)
    else {
        return Ok(());
    };
    let (Some(case_id), Some(round_id)) = (
        event.event.case_id().as_deref(),
        event.event.round_id().as_deref(),
    ) else {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "panel notification points to an unscoped retained event".to_owned(),
        ));
    };
    if event.cursor() != notification.finalized_event_cursor
        || *event.event.occurred_at_unix_ms() != notification.source_occurred_at_unix_ms
        || panel_notification_scope_digest(case_id, round_id) != notification.scope_digest
    {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "panel notification differs from its retained finalized event".to_owned(),
        ));
    }
    let source_operation_id = match notification.kind {
        ModerationPanelNotificationKindV1::PrimaryAssignment
        | ModerationPanelNotificationKindV1::WaitlistStandby => {
            if *event.event.kind() != SorafsModerationLedgerEventKind::SortitionFinalized {
                return Err(ModerationOrchestratorError::CheckpointCorrupt(
                    "assignment notification points to a non-sortition event".to_owned(),
                ));
            }
            let appeal = snapshot.appeal(case_id, round_id).ok_or_else(|| {
                ModerationOrchestratorError::CheckpointCorrupt(
                    "assignment notification has no authoritative appeal".to_owned(),
                )
            })?;
            let selection = appeal.appeal.selection.as_ref().ok_or_else(|| {
                ModerationOrchestratorError::CheckpointCorrupt(
                    "assignment notification has no authoritative selection".to_owned(),
                )
            })?;
            let expected_recipient =
                if notification.kind == ModerationPanelNotificationKindV1::PrimaryAssignment {
                    selection.jurors.contains(&notification.recipient)
                } else {
                    selection.waitlist.contains(&notification.recipient)
                };
            if !expected_recipient {
                return Err(ModerationOrchestratorError::CheckpointCorrupt(
                    "assignment notification recipient is outside the finalized roster".to_owned(),
                ));
            }
            ModerationNativeActionV1::FinalizeSortition(FinalizeSorafsModerationSortition::new(
                case_id.to_owned(),
                round_id.to_owned(),
                appeal.appeal.pop_snapshot_digest,
                selection.randomness_anchor,
                selection.jurors.clone(),
                selection.waitlist.clone(),
            ))
            .operation_id(chain_id, event.event.authority())
            .map_err(|_| {
                ModerationOrchestratorError::CheckpointCorrupt(
                    "assignment notification operation identity cannot be reconstructed".to_owned(),
                )
            })?
        }
        ModerationPanelNotificationKindV1::BallotActivated => {
            if *event.event.kind() != SorafsModerationLedgerEventKind::CaseActivated {
                return Err(ModerationOrchestratorError::CheckpointCorrupt(
                    "activation notification points to a different event kind".to_owned(),
                ));
            }
            let appeal = snapshot.appeal(case_id, round_id).ok_or_else(|| {
                ModerationOrchestratorError::CheckpointCorrupt(
                    "activation notification has no authoritative appeal".to_owned(),
                )
            })?;
            let selection = appeal.appeal.selection.as_ref().ok_or_else(|| {
                ModerationOrchestratorError::CheckpointCorrupt(
                    "activation notification has no authoritative selection".to_owned(),
                )
            })?;
            let case = snapshot.case(case_id, round_id).ok_or_else(|| {
                ModerationOrchestratorError::CheckpointCorrupt(
                    "activation notification has no authoritative case".to_owned(),
                )
            })?;
            if !case.case.spec.jurors.contains(&notification.recipient) {
                return Err(ModerationOrchestratorError::CheckpointCorrupt(
                    "activation notification recipient is outside the finalized panel".to_owned(),
                ));
            }
            ModerationNativeActionV1::ActivateCase(ActivateSorafsModerationCase::new(
                case_id.to_owned(),
                round_id.to_owned(),
                selection.sortition_digest,
            ))
            .operation_id(chain_id, event.event.authority())
            .map_err(|_| {
                ModerationOrchestratorError::CheckpointCorrupt(
                    "activation notification operation identity cannot be reconstructed".to_owned(),
                )
            })?
        }
    };
    if source_operation_id != notification.source_operation_id {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "panel notification source operation identity is invalid".to_owned(),
        ));
    }
    Ok(())
}

fn external_work_cursor_is_valid(
    height: u64,
    block_hash: [u8; 32],
    snapshot: Option<&ModerationFinalizedLedgerSnapshotV1>,
) -> bool {
    if height == 0 || block_hash == [0; 32] {
        return false;
    }
    snapshot.is_some_and(|snapshot| {
        height <= snapshot.finalized_height
            && (height != snapshot.finalized_height || block_hash == snapshot.finalized_block_hash)
    })
}

fn external_work_claim_is_valid(
    identity: [u8; 32],
    claim: &StoredExternalWorkClaimV1,
    snapshot: Option<&ModerationFinalizedLedgerSnapshotV1>,
) -> bool {
    claim.generation != 0
        && claim.work_digest != [0; 32]
        && claim.lease_token != [0; 32]
        && claim.claimed_at_unix_ms != 0
        && external_work_cursor_is_valid(
            claim.claimed_at_finalized_height,
            claim.claimed_at_finalized_block_hash,
            snapshot,
        )
        && snapshot
            .is_some_and(|snapshot| claim.claimed_at_unix_ms <= snapshot.finalized_at_unix_ms)
        && claim
            .claimed_at_unix_ms
            .checked_add(MODERATION_EXTERNAL_WORK_LEASE_MS_V1)
            == Some(claim.lease_expires_at_unix_ms)
        && external_work_lease_token(identity, claim) == claim.lease_token
}

fn validate_checkpoint(
    state: &ModerationOrchestratorCheckpointV1,
    config: &ModerationOrchestratorConfigV1,
    chain_id: &iroha_data_model::ChainId,
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
        || state.panel_notifications.len() > config.max_handoffs
    {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "checkpoint exceeds configured retention bounds".to_owned(),
        ));
    }
    if state.panel_notification_outbox_digest == [0; 32]
        || state.panel_notification_outbox_digest != panel_notification_outbox_digest(state)
    {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "panel notification outbox digest mismatch".to_owned(),
        ));
    }
    if let Some(cursor) = state.panel_notification_scanned_cursor {
        if cursor.sequence == 0 || cursor.block_height == 0 || cursor.block_hash == [0; 32] {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "panel notification scan cursor is inert".to_owned(),
            ));
        }
    } else if !state.panel_notifications.is_empty() {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "panel notifications exist without a finalized scan cursor".to_owned(),
        ));
    }
    match (
        state.finalized_snapshot.as_ref(),
        state.finalized_snapshot_digest,
    ) {
        (None, None) => {
            if state.panel_notification_scanned_cursor.is_some() {
                return Err(ModerationOrchestratorError::CheckpointCorrupt(
                    "panel notification scan cursor exists without a finalized snapshot".to_owned(),
                ));
            }
        }
        (Some(snapshot), Some(digest)) => {
            validate_finalized_snapshot(snapshot, config)?;
            validate_panel_notification_source_provenance(snapshot)?;
            if !snapshot.events.is_empty() && state.panel_notification_scanned_cursor.is_none() {
                return Err(ModerationOrchestratorError::CheckpointCorrupt(
                    "finalized moderation events were not scanned for panel notifications"
                        .to_owned(),
                ));
            }
            if let Some(scanned) = state.panel_notification_scanned_cursor {
                let retained_exact = snapshot
                    .events
                    .iter()
                    .find(|event| event.sequence == scanned.sequence);
                let first_after = snapshot
                    .events
                    .iter()
                    .find(|event| event.sequence > scanned.sequence);
                let invalid_gap = retained_exact.is_none()
                    && first_after.is_some_and(|event| {
                        scanned
                            .sequence
                            .checked_add(1)
                            .is_none_or(|expected| event.sequence != expected)
                    });
                if scanned.block_height > snapshot.finalized_height
                    || snapshot
                        .events
                        .last()
                        .is_some_and(|event| scanned.sequence > event.sequence)
                    || retained_exact.is_some_and(|event| event.cursor() != scanned)
                    || invalid_gap
                {
                    return Err(ModerationOrchestratorError::CheckpointCorrupt(
                        "panel notification scan cursor differs from the finalized snapshot"
                            .to_owned(),
                    ));
                }
            }
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
    let mut operations = BTreeMap::new();
    for operation in &state.operations {
        if operation.operation_id == [0; 32]
            || operation.action_digest == [0; 32]
            || operations
                .insert(operation.operation_id, operation)
                .is_some()
        {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "invalid or duplicate operation tombstone".to_owned(),
            ));
        }
    }
    let mut outbox = BTreeSet::new();
    for entry in &state.outbox {
        let Some(operation) = operations.get(&entry.operation_id).copied() else {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "outbox entry has no operation tombstone".to_owned(),
            ));
        };
        if !outbox.insert(entry.operation_id)
            || entry.action_digest != entry.action.action_digest()?
            || entry.request_binding_digest == [0; 32]
            || entry.attempts > config.max_submit_attempts
            || operation.authority != entry.authority
            || operation.action_digest != entry.action_digest
            || operation.status != StoredOperationStatusV1::Pending
        {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "invalid, duplicate, or orphaned outbox entry".to_owned(),
            ));
        }
        validate_retired_envelope_history(entry, config)?;
        let expected_operation_transaction_id =
            retired_history_fence_transaction_id(entry).or(entry.transaction_id);
        if operation.transaction_id != expected_operation_transaction_id {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "operation transaction identity does not match active or fenced history".to_owned(),
            ));
        }
        entry.action.validate_authority(&entry.authority)?;
        if entry.action.operation_id(chain_id, &entry.authority)? != entry.operation_id {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "outbox semantic identity mismatch".to_owned(),
            ));
        }
        let empty_cursor =
            entry.baseline_finalized_height == 0 && entry.baseline_finalized_block_hash == [0; 32];
        let nonzero_cursor =
            entry.baseline_finalized_height != 0 && entry.baseline_finalized_block_hash != [0; 32];
        let empty_transaction = entry.transaction_id.is_none()
            && entry.signed_transaction_digest.is_none()
            && entry.signed_transaction_bytes.is_none();
        let complete_transaction = entry.transaction_id.is_some()
            && entry.signed_transaction_digest.is_some()
            && entry.signed_transaction_bytes.is_some();
        let empty_lookup_cursor = entry.last_lookup_finalized_height == 0
            && entry.last_lookup_finalized_block_hash == [0; 32];
        let valid_lookup_cursor = empty_lookup_cursor
            || external_work_cursor_is_valid(
                entry.last_lookup_finalized_height,
                entry.last_lookup_finalized_block_hash,
                state.finalized_snapshot.as_ref(),
            );
        let valid_delivery = match entry.state {
            StoredOutboxStateV1::Ready => empty_cursor && empty_transaction,
            StoredOutboxStateV1::Signing => nonzero_cursor && empty_transaction,
            StoredOutboxStateV1::Signed => nonzero_cursor && complete_transaction,
            StoredOutboxStateV1::Ambiguous | StoredOutboxStateV1::Submitted => {
                nonzero_cursor && complete_transaction && entry.attempts != 0
            }
        };
        let valid_claim = match entry.work_claim.as_ref() {
            None => entry.state != StoredOutboxStateV1::Signing,
            Some(claim)
                if entry.work_generation == claim.generation
                    && external_work_claim_is_valid(
                        entry.operation_id,
                        claim,
                        state.finalized_snapshot.as_ref(),
                    ) =>
            {
                match claim.kind {
                    StoredExternalWorkKindV1::Sign => {
                        entry.state == StoredOutboxStateV1::Signing
                            && empty_transaction
                            && claim.work_digest == outbox_sign_work_digest(entry)
                    }
                    StoredExternalWorkKindV1::Submit => {
                        entry.state == StoredOutboxStateV1::Ambiguous
                            && complete_transaction
                            && entry.attempts != 0
                            && claim.work_digest == outbox_submit_work_digest(entry)
                    }
                    StoredExternalWorkKindV1::Lookup => {
                        matches!(
                            entry.state,
                            StoredOutboxStateV1::Signed
                                | StoredOutboxStateV1::Ambiguous
                                | StoredOutboxStateV1::Submitted
                        ) && complete_transaction
                            && claim.work_digest
                                == outbox_lookup_work_digest(
                                    entry,
                                    ModerationFinalizedCursorV1 {
                                        height: claim.claimed_at_finalized_height,
                                        block_hash: claim.claimed_at_finalized_block_hash,
                                    },
                                )
                    }
                    StoredExternalWorkKindV1::Handoff => false,
                }
            }
            Some(_) => false,
        };
        if !valid_delivery
            || !valid_lookup_cursor
            || !valid_claim
            || (entry.work_claim.is_some() && entry.work_generation == 0)
        {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "outbox crash, lookup cursor, or external-work claim is inconsistent".to_owned(),
            ));
        }
        if !empty_cursor {
            let request = moderation_transaction_request(chain_id, entry)?;
            if complete_transaction {
                let signed = moderation_signed_transaction(entry)?;
                let transaction = signed.decode_for_request(&request).map_err(|_| {
                    ModerationOrchestratorError::CheckpointCorrupt(
                        "outbox signed transaction is invalid".to_owned(),
                    )
                })?;
                let timing = signed_envelope_timing(&transaction)?;
                if entry.retired_envelopes.iter().any(|record| {
                    record.transaction_id == signed.transaction_id
                        || record.signed_transaction_digest == signed.canonical_bytes_digest
                }) {
                    return Err(ModerationOrchestratorError::CheckpointCorrupt(
                        "active signed envelope duplicates retired history".to_owned(),
                    ));
                }
                if let Some(previous) = entry.retired_envelopes.last()
                    && (timing.created_at_unix_ms <= previous.created_at_unix_ms
                        || timing.expires_at_unix_ms <= previous.retired_at_finalized_unix_ms)
                {
                    return Err(ModerationOrchestratorError::CheckpointCorrupt(
                        "active signed envelope does not advance retired history".to_owned(),
                    ));
                }
            }
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
        let valid_claim = match entry.work_claim.as_ref() {
            None => true,
            Some(claim) => {
                entry.work_generation == claim.generation
                    && claim.kind == StoredExternalWorkKindV1::Handoff
                    && claim.work_digest == handoff_work_digest(&entry.handoff)
                    && external_work_claim_is_valid(
                        entry.handoff.handoff_id,
                        claim,
                        state.finalized_snapshot.as_ref(),
                    )
            }
        };
        if entry.handoff.handoff_id == [0; 32]
            || !handoffs.insert(entry.handoff.handoff_id)
            || entry.attempts > config.max_submit_attempts
            || !valid_claim
            || (entry.work_claim.is_some() && (entry.work_generation == 0 || entry.attempts == 0))
        {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "invalid, duplicate, or unfenced pending handoff".to_owned(),
            ));
        }
    }
    let mut previous_notification_id = None;
    for entry in &state.panel_notifications {
        let notification = &entry.notification;
        let notification_id = notification.notification_id;
        if previous_notification_id.is_some_and(|previous| previous >= notification_id) {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "panel notifications are not strictly sorted and unique".to_owned(),
            ));
        }
        previous_notification_id = Some(notification_id);
        if notification_id == [0; 32]
            || notification.source_operation_id == [0; 32]
            || notification.scope_digest == [0; 32]
            || notification.finalized_event_cursor.sequence == 0
            || notification.finalized_event_cursor.block_height == 0
            || notification.finalized_event_cursor.block_hash == [0; 32]
            || notification.source_occurred_at_unix_ms == 0
            || notification_id
                != panel_notification_id(
                    chain_id,
                    notification.source_operation_id,
                    notification.scope_digest,
                    notification.kind,
                    &notification.recipient,
                    notification.finalized_event_cursor,
                    notification.source_occurred_at_unix_ms,
                )
            || entry.attempt_limit == 0
            || entry.attempts > entry.attempt_limit
            || entry.claim_generation != entry.attempts
            || entry.available_at_unix_ms == 0
            || entry.available_at_unix_ms < notification.source_occurred_at_unix_ms
            || entry.record_digest == [0; 32]
            || entry.record_digest != panel_notification_record_digest(entry)
            || state
                .panel_notification_scanned_cursor
                .is_none_or(|cursor| {
                    notification.finalized_event_cursor.sequence > cursor.sequence
                        || notification.finalized_event_cursor.block_height > cursor.block_height
                })
        {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "panel notification identity, cursor, generation, or record digest is invalid"
                    .to_owned(),
            ));
        }
        if let Some(snapshot) = state.finalized_snapshot.as_ref() {
            validate_retained_panel_notification_source(notification, snapshot, chain_id)?;
        }
        let claim_fields = (
            entry.claimed_by,
            entry.lease_token,
            entry.claimed_at_unix_ms,
            entry.lease_expires_at_unix_ms,
        );
        let delivery_fields = (entry.receipt_digest, entry.delivered_at_unix_ms);
        let dead_fields = (entry.dead_letter_reason, entry.dead_lettered_at_unix_ms);
        match entry.state {
            StoredPanelNotificationStateV1::Pending => {
                if claim_fields != (None, None, None, None)
                    || delivery_fields != (None, None)
                    || dead_fields != (None, None)
                {
                    return Err(ModerationOrchestratorError::CheckpointCorrupt(
                        "pending panel notification retains terminal or lease state".to_owned(),
                    ));
                }
            }
            StoredPanelNotificationStateV1::Claimed | StoredPanelNotificationStateV1::Delivered => {
                let (
                    Some(worker_id),
                    Some(lease_token),
                    Some(claimed_at_unix_ms),
                    Some(lease_expires_at_unix_ms),
                ) = claim_fields
                else {
                    return Err(ModerationOrchestratorError::CheckpointCorrupt(
                        "claimed or delivered panel notification has an incomplete lease"
                            .to_owned(),
                    ));
                };
                let expected_expiry =
                    claimed_at_unix_ms.checked_add(MODERATION_PANEL_NOTIFICATION_LEASE_MS_V1);
                let expected_token = panel_notification_lease_token(
                    notification_id,
                    worker_id,
                    entry.claim_generation,
                    entry.attempts,
                    claimed_at_unix_ms,
                    lease_expires_at_unix_ms,
                );
                if entry.attempts == 0
                    || worker_id == [0; 32]
                    || lease_token == [0; 32]
                    || claimed_at_unix_ms == 0
                    || claimed_at_unix_ms < entry.available_at_unix_ms
                    || claimed_at_unix_ms > state.panel_notification_clock_unix_ms
                    || expected_expiry != Some(lease_expires_at_unix_ms)
                    || expected_token != lease_token
                    || dead_fields != (None, None)
                {
                    return Err(ModerationOrchestratorError::CheckpointCorrupt(
                        "panel notification lease is invalid".to_owned(),
                    ));
                }
                match entry.state {
                    StoredPanelNotificationStateV1::Claimed => {
                        if delivery_fields != (None, None) {
                            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                                "claimed panel notification already contains a receipt".to_owned(),
                            ));
                        }
                    }
                    StoredPanelNotificationStateV1::Delivered => {
                        let (Some(receipt_digest), Some(delivered_at_unix_ms)) = delivery_fields
                        else {
                            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                                "delivered panel notification has no complete receipt".to_owned(),
                            ));
                        };
                        if receipt_digest == [0; 32]
                            || delivered_at_unix_ms < notification.source_occurred_at_unix_ms
                            || delivered_at_unix_ms > state.panel_notification_clock_unix_ms
                        {
                            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                                "delivered panel notification receipt is invalid".to_owned(),
                            ));
                        }
                    }
                    StoredPanelNotificationStateV1::Pending
                    | StoredPanelNotificationStateV1::DeadLetter => unreachable!(),
                }
            }
            StoredPanelNotificationStateV1::DeadLetter => {
                let (Some(reason), Some(dead_lettered_at_unix_ms)) = dead_fields else {
                    return Err(ModerationOrchestratorError::CheckpointCorrupt(
                        "dead-letter panel notification has no terminal provenance".to_owned(),
                    ));
                };
                if claim_fields != (None, None, None, None)
                    || delivery_fields != (None, None)
                    || entry.attempts == 0
                    || dead_lettered_at_unix_ms == 0
                    || dead_lettered_at_unix_ms < notification.source_occurred_at_unix_ms
                    || dead_lettered_at_unix_ms > state.panel_notification_clock_unix_ms
                    || (reason == ModerationPanelNotificationDeadLetterReasonV1::RetryExhausted
                        && entry.attempts != entry.attempt_limit)
                {
                    return Err(ModerationOrchestratorError::CheckpointCorrupt(
                        "dead-letter panel notification state is invalid".to_owned(),
                    ));
                }
            }
        }
    }
    Ok(())
}

fn persist_checkpoint(
    config: &ModerationOrchestratorConfigV1,
    chain_id: &iroha_data_model::ChainId,
    state: &mut ModerationOrchestratorCheckpointV1,
) -> Result<(), ModerationOrchestratorError> {
    state.generation = state
        .generation
        .checked_add(1)
        .ok_or(ModerationOrchestratorError::GenerationOverflow)?;
    refresh_panel_notification_outbox_digest(state);
    validate_checkpoint(state, config, chain_id)?;
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

fn make_panel_notification_capacity(
    state: &mut ModerationOrchestratorCheckpointV1,
    additional: usize,
    limit: usize,
) -> Result<(), ModerationOrchestratorError> {
    if additional > limit {
        return Err(ModerationOrchestratorError::ResourceExhausted {
            resource: "panel notifications",
            limit,
        });
    }
    let excess = state
        .panel_notifications
        .len()
        .saturating_add(additional)
        .saturating_sub(limit);
    if excess == 0 {
        return Ok(());
    }
    let mut terminal = state
        .panel_notifications
        .iter()
        .filter(|entry| {
            matches!(
                entry.state,
                StoredPanelNotificationStateV1::Delivered
                    | StoredPanelNotificationStateV1::DeadLetter
            )
        })
        .map(|entry| {
            (
                entry.notification.finalized_event_cursor.sequence,
                entry.notification.notification_id,
            )
        })
        .collect::<Vec<_>>();
    terminal.sort_unstable();
    if terminal.len() < excess {
        return Err(ModerationOrchestratorError::ResourceExhausted {
            resource: "panel notifications",
            limit,
        });
    }
    let remove = terminal
        .into_iter()
        .take(excess)
        .map(|(_, notification_id)| notification_id)
        .collect::<BTreeSet<_>>();
    state
        .panel_notifications
        .retain(|entry| !remove.contains(&entry.notification.notification_id));
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

fn moderation_transaction_request(
    chain_id: &iroha_data_model::ChainId,
    entry: &StoredOutboxEntryV1,
) -> Result<ModerationTransactionRequestV1, ModerationOrchestratorError> {
    ModerationTransactionRequestV1::new(
        chain_id.clone(),
        entry.envelope_generation,
        entry.authority.clone(),
        entry.action.clone(),
        entry.request_binding_digest,
        entry.baseline_finalized_height,
        entry.baseline_finalized_block_hash,
    )
}

fn moderation_signed_transaction(
    entry: &StoredOutboxEntryV1,
) -> Result<ModerationSignedTransactionV1, ModerationOrchestratorError> {
    match (
        entry.transaction_id,
        entry.signed_transaction_digest,
        entry.signed_transaction_bytes.as_ref(),
    ) {
        (Some(transaction_id), Some(canonical_bytes_digest), Some(canonical_bytes)) => {
            Ok(ModerationSignedTransactionV1 {
                transaction_id,
                canonical_bytes_digest,
                canonical_bytes: canonical_bytes.clone(),
            })
        }
        _ => Err(ModerationOrchestratorError::CheckpointCorrupt(
            "signed outbox state has no exact retained transaction".to_owned(),
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SignedEnvelopeTimingV1 {
    created_at_unix_ms: u64,
    expires_at_unix_ms: u64,
}

fn signed_envelope_timing(
    transaction: &SignedTransaction,
) -> Result<SignedEnvelopeTimingV1, ModerationOrchestratorError> {
    let created_at_unix_ms =
        u64::try_from(transaction.creation_time().as_millis()).map_err(|_| {
            ModerationOrchestratorError::CheckpointCorrupt(
                "signed transaction creation time exceeds u64".to_owned(),
            )
        })?;
    let expires_at_unix_ms = created_at_unix_ms
        .checked_add(MODERATION_TRANSACTION_TTL_MS_V1)
        .ok_or_else(|| {
            ModerationOrchestratorError::CheckpointCorrupt(
                "signed transaction expiration overflows u64".to_owned(),
            )
        })?;
    Ok(SignedEnvelopeTimingV1 {
        created_at_unix_ms,
        expires_at_unix_ms,
    })
}

fn next_envelope_generation(generation: u32) -> Result<u32, ModerationOrchestratorError> {
    generation
        .checked_add(1)
        .ok_or(ModerationOrchestratorError::GenerationOverflow)
}

fn retired_envelope_disposition_tag(disposition: StoredRetiredEnvelopeDispositionV1) -> [u8; 1] {
    [match disposition {
        StoredRetiredEnvelopeDispositionV1::NotFound => 0,
        StoredRetiredEnvelopeDispositionV1::Pending => 1,
        StoredRetiredEnvelopeDispositionV1::Applied => 2,
        StoredRetiredEnvelopeDispositionV1::Rejected => 3,
    }]
}

fn retired_envelope_record_digest(
    operation_id: [u8; 32],
    record: &StoredRetiredEnvelopeV1,
) -> [u8; 32] {
    let generation = record.generation.to_le_bytes();
    let created_at_unix_ms = record.created_at_unix_ms.to_le_bytes();
    let expires_at_unix_ms = record.expires_at_unix_ms.to_le_bytes();
    let retired_at_finalized_height = record.retired_at_finalized_height.to_le_bytes();
    let retired_at_finalized_unix_ms = record.retired_at_finalized_unix_ms.to_le_bytes();
    let disposition = retired_envelope_disposition_tag(record.disposition);
    domain_hash(
        RETIRED_ENVELOPE_RECORD_DIGEST_DOMAIN_V1,
        &[
            &operation_id,
            &generation,
            &record.transaction_id,
            &record.signed_transaction_digest,
            &created_at_unix_ms,
            &expires_at_unix_ms,
            &retired_at_finalized_height,
            &record.retired_at_finalized_block_hash,
            &retired_at_finalized_unix_ms,
            &disposition,
        ],
    )
}

fn refresh_retired_envelope_record_digest(
    operation_id: [u8; 32],
    record: &mut StoredRetiredEnvelopeV1,
) {
    record.record_digest = retired_envelope_record_digest(operation_id, record);
}

fn retired_history_fence_transaction_id(entry: &StoredOutboxEntryV1) -> Option<[u8; 32]> {
    entry
        .retired_envelopes
        .iter()
        .rev()
        .find(|record| record.disposition == StoredRetiredEnvelopeDispositionV1::Applied)
        .or_else(|| {
            entry
                .retired_envelopes
                .iter()
                .rev()
                .find(|record| record.disposition == StoredRetiredEnvelopeDispositionV1::Pending)
        })
        .map(|record| record.transaction_id)
}

fn validate_retired_envelope_history(
    entry: &StoredOutboxEntryV1,
    config: &ModerationOrchestratorConfigV1,
) -> Result<(), ModerationOrchestratorError> {
    if entry.envelope_generation == 0
        || entry.envelope_generation > config.max_submit_attempts
        || usize::try_from(entry.envelope_generation.saturating_sub(1)).ok()
            != Some(entry.retired_envelopes.len())
    {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "outbox envelope generation/history is inconsistent".to_owned(),
        ));
    }
    let mut transaction_ids = BTreeSet::new();
    let mut signed_digests = BTreeSet::new();
    let mut previous_retirement: Option<(u64, u64, u64)> = None;
    for (index, record) in entry.retired_envelopes.iter().enumerate() {
        let expected_generation = u32::try_from(index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(ModerationOrchestratorError::GenerationOverflow)?;
        let expected_expiry = record
            .created_at_unix_ms
            .checked_add(MODERATION_TRANSACTION_TTL_MS_V1);
        if record.generation != expected_generation
            || record.transaction_id == [0; 32]
            || record.signed_transaction_digest == [0; 32]
            || !transaction_ids.insert(record.transaction_id)
            || !signed_digests.insert(record.signed_transaction_digest)
            || record.created_at_unix_ms == 0
            || expected_expiry != Some(record.expires_at_unix_ms)
            || record.retired_at_finalized_height == 0
            || record.retired_at_finalized_block_hash == [0; 32]
            || record.retired_at_finalized_unix_ms < record.expires_at_unix_ms
            || record.record_digest == [0; 32]
            || record.record_digest != retired_envelope_record_digest(entry.operation_id, record)
        {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "retired signed-envelope history is invalid".to_owned(),
            ));
        }
        if let Some((height, finalized_at_unix_ms, created_at_unix_ms)) = previous_retirement {
            if record.retired_at_finalized_height <= height
                || record.retired_at_finalized_unix_ms < finalized_at_unix_ms
                || record.created_at_unix_ms <= created_at_unix_ms
            {
                return Err(ModerationOrchestratorError::CheckpointCorrupt(
                    "retired signed-envelope history is not monotonic".to_owned(),
                ));
            }
        }
        previous_retirement = Some((
            record.retired_at_finalized_height,
            record.retired_at_finalized_unix_ms,
            record.created_at_unix_ms,
        ));
    }
    Ok(())
}

fn validate_signed_transaction_for_request(
    request: &ModerationTransactionRequestV1,
    transaction: &SignedTransaction,
) -> Result<(), ModerationSubmissionFailureV1> {
    let canonical_payload = norito::to_bytes(transaction.payload())
        .map_err(|_| ModerationSubmissionFailureV1::PermanentRejection)?;
    let canonical_envelope = norito::to_bytes(transaction)
        .map_err(|_| ModerationSubmissionFailureV1::PermanentRejection)?;
    if transaction.verify_signature().is_err()
        || transaction.chain() != &request.chain_id
        || transaction.authority() != &request.authority
        || *transaction.hash().as_ref() == [0; 32]
        || transaction.creation_time().is_zero()
        || transaction.time_to_live()
            != Some(core::time::Duration::from_millis(
                MODERATION_TRANSACTION_TTL_MS_V1,
            ))
        || transaction.nonce().is_some()
        || !transaction.metadata().is_empty()
        || transaction.fee_payment_intent().validate().is_err()
        || transaction.attachments().is_some()
        || canonical_payload.is_empty()
        || canonical_payload.len() > MODERATION_SIGNED_TRANSACTION_MAX_BYTES_V1
        || canonical_envelope.is_empty()
        || canonical_envelope.len() > MODERATION_SIGNED_TRANSACTION_MAX_BYTES_V1
    {
        return Err(ModerationSubmissionFailureV1::PermanentRejection);
    }
    let expected = request.action.instruction();
    match transaction.instructions() {
        Executable::Instructions(instructions)
            if instructions.len() == 1 && instructions.first() == Some(&expected) =>
        {
            Ok(())
        }
        _ => Err(ModerationSubmissionFailureV1::PermanentRejection),
    }
}

fn signed_transaction_digest(bytes: &[u8]) -> [u8; 32] {
    domain_hash(SIGNED_TRANSACTION_DIGEST_DOMAIN_V1, &[bytes])
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

fn next_external_work_generation(generation: u32) -> Result<u32, ModerationOrchestratorError> {
    generation
        .checked_add(1)
        .ok_or(ModerationOrchestratorError::GenerationOverflow)
}

fn outbox_sign_work_digest(entry: &StoredOutboxEntryV1) -> [u8; 32] {
    let kind = [StoredExternalWorkKindV1::Sign.tag()];
    let envelope_generation = entry.envelope_generation.to_le_bytes();
    let baseline_height = entry.baseline_finalized_height.to_le_bytes();
    domain_hash(
        EXTERNAL_WORK_DIGEST_DOMAIN_V1,
        &[
            &kind,
            &entry.operation_id,
            &envelope_generation,
            &entry.action_digest,
            &entry.request_binding_digest,
            &baseline_height,
            &entry.baseline_finalized_block_hash,
        ],
    )
}

fn outbox_submit_work_digest(entry: &StoredOutboxEntryV1) -> [u8; 32] {
    let kind = [StoredExternalWorkKindV1::Submit.tag()];
    let envelope_generation = entry.envelope_generation.to_le_bytes();
    let baseline_height = entry.baseline_finalized_height.to_le_bytes();
    let zero = [0; 32];
    domain_hash(
        EXTERNAL_WORK_DIGEST_DOMAIN_V1,
        &[
            &kind,
            &entry.operation_id,
            &envelope_generation,
            &entry.action_digest,
            &entry.request_binding_digest,
            &baseline_height,
            &entry.baseline_finalized_block_hash,
            entry.transaction_id.as_ref().unwrap_or(&zero),
            entry.signed_transaction_digest.as_ref().unwrap_or(&zero),
        ],
    )
}

fn outbox_lookup_work_digest(
    entry: &StoredOutboxEntryV1,
    cursor: ModerationFinalizedCursorV1,
) -> [u8; 32] {
    let zero = [0; 32];
    let mut material = Vec::with_capacity(
        1 + 32 + 8 + 32 + 4 + entry.retired_envelopes.len().saturating_mul(36) + 32,
    );
    material.push(StoredExternalWorkKindV1::Lookup.tag());
    material.extend_from_slice(&entry.operation_id);
    material.extend_from_slice(&cursor.height.to_le_bytes());
    material.extend_from_slice(&cursor.block_hash);
    material.extend_from_slice(
        &u32::try_from(entry.retired_envelopes.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    for record in &entry.retired_envelopes {
        material.extend_from_slice(&record.generation.to_le_bytes());
        material.extend_from_slice(&record.transaction_id);
    }
    material.extend_from_slice(entry.transaction_id.as_ref().unwrap_or(&zero));
    domain_hash(EXTERNAL_WORK_DIGEST_DOMAIN_V1, &[&material])
}

fn handoff_work_digest(handoff: &ModerationTerminalHandoffV1) -> [u8; 32] {
    let kind = [StoredExternalWorkKindV1::Handoff.tag()];
    let destination = [match handoff.kind {
        ModerationTerminalHandoffKindV1::Settlement => 0,
        ModerationTerminalHandoffKindV1::Publication => 1,
    }];
    let finalized_height = handoff.finalized_cursor.height.to_le_bytes();
    domain_hash(
        EXTERNAL_WORK_DIGEST_DOMAIN_V1,
        &[
            &kind,
            &handoff.handoff_id,
            &destination,
            handoff.case_id.as_bytes(),
            handoff.round_id.as_bytes(),
            &handoff.outcome_digest,
            &finalized_height,
            &handoff.finalized_cursor.block_hash,
        ],
    )
}

fn external_work_lease_token(identity: [u8; 32], claim: &StoredExternalWorkClaimV1) -> [u8; 32] {
    let kind = [claim.kind.tag()];
    let generation = claim.generation.to_le_bytes();
    let height = claim.claimed_at_finalized_height.to_le_bytes();
    let claimed_at = claim.claimed_at_unix_ms.to_le_bytes();
    let expires_at = claim.lease_expires_at_unix_ms.to_le_bytes();
    domain_hash(
        EXTERNAL_WORK_LEASE_DOMAIN_V1,
        &[
            &kind,
            &identity,
            &generation,
            &height,
            &claim.claimed_at_finalized_block_hash,
            &claimed_at,
            &expires_at,
            &claim.work_digest,
        ],
    )
}

fn external_work_claim(
    kind: StoredExternalWorkKindV1,
    identity: [u8; 32],
    generation: u32,
    work_digest: [u8; 32],
    cursor: ModerationFinalizedCursorV1,
    claimed_at_unix_ms: u64,
) -> Result<StoredExternalWorkClaimV1, ModerationOrchestratorError> {
    let lease_expires_at_unix_ms = claimed_at_unix_ms
        .checked_add(MODERATION_EXTERNAL_WORK_LEASE_MS_V1)
        .ok_or(ModerationOrchestratorError::GenerationOverflow)?;
    let mut claim = StoredExternalWorkClaimV1 {
        kind,
        generation,
        claimed_at_finalized_height: cursor.height,
        claimed_at_finalized_block_hash: cursor.block_hash,
        claimed_at_unix_ms,
        lease_expires_at_unix_ms,
        work_digest,
        lease_token: [0; 32],
    };
    claim.lease_token = external_work_lease_token(identity, &claim);
    Ok(claim)
}

fn reset_sign_claim(entry: &mut StoredOutboxEntryV1) {
    entry.baseline_finalized_height = 0;
    entry.baseline_finalized_block_hash = [0; 32];
    entry.transaction_id = None;
    entry.signed_transaction_digest = None;
    entry.signed_transaction_bytes = None;
    entry.state = StoredOutboxStateV1::Ready;
    entry.work_claim = None;
}

fn recover_external_work_after_restart(state: &mut ModerationOrchestratorCheckpointV1) -> bool {
    let mut recovered = false;
    for entry in &mut state.outbox {
        let Some(kind) = entry.work_claim.as_ref().map(|claim| claim.kind) else {
            continue;
        };
        match kind {
            StoredExternalWorkKindV1::Sign => reset_sign_claim(entry),
            StoredExternalWorkKindV1::Submit | StoredExternalWorkKindV1::Lookup => {
                entry.work_claim = None;
            }
            StoredExternalWorkKindV1::Handoff => continue,
        }
        recovered = true;
    }
    for entry in &mut state.pending_handoffs {
        if entry.work_claim.take().is_some() {
            recovered = true;
        }
    }
    recovered
}

fn recover_expired_external_work_claims(
    state: &mut ModerationOrchestratorCheckpointV1,
) -> Result<bool, ModerationOrchestratorError> {
    let now_unix_ms = state
        .finalized_snapshot
        .as_ref()
        .ok_or(ModerationOrchestratorError::FinalizedReaderUnavailable)?
        .finalized_at_unix_ms;
    let mut recovered = false;
    for entry in &mut state.outbox {
        let Some((kind, lease_expires_at_unix_ms)) = entry
            .work_claim
            .as_ref()
            .map(|claim| (claim.kind, claim.lease_expires_at_unix_ms))
        else {
            continue;
        };
        if lease_expires_at_unix_ms > now_unix_ms {
            continue;
        }
        match kind {
            StoredExternalWorkKindV1::Sign => reset_sign_claim(entry),
            StoredExternalWorkKindV1::Submit | StoredExternalWorkKindV1::Lookup => {
                entry.work_claim = None;
            }
            StoredExternalWorkKindV1::Handoff => {
                return Err(ModerationOrchestratorError::CheckpointCorrupt(
                    "outbox contains a terminal-handoff claim".to_owned(),
                ));
            }
        }
        recovered = true;
    }
    for entry in &mut state.pending_handoffs {
        if entry
            .work_claim
            .as_ref()
            .is_some_and(|claim| claim.lease_expires_at_unix_ms <= now_unix_ms)
        {
            entry.work_claim = None;
            recovered = true;
        }
    }
    Ok(recovered)
}

fn outbox_claim_position(
    state: &ModerationOrchestratorCheckpointV1,
    kind: StoredExternalWorkKindV1,
    claim: &StoredExternalWorkClaimV1,
) -> Option<usize> {
    state.outbox.iter().position(|entry| {
        claim.kind == kind
            && entry.work_generation == claim.generation
            && entry.work_claim.as_ref() == Some(claim)
    })
}

fn handoff_claim_position(
    state: &ModerationOrchestratorCheckpointV1,
    claim: &StoredExternalWorkClaimV1,
) -> Option<usize> {
    state.pending_handoffs.iter().position(|entry| {
        claim.kind == StoredExternalWorkKindV1::Handoff
            && entry.work_generation == claim.generation
            && entry.work_claim.as_ref() == Some(claim)
    })
}

fn retired_envelope_disposition_after_lookup(
    record: &StoredRetiredEnvelopeV1,
    lookup: ModerationSubmissionLookupV1,
    cursor: ModerationFinalizedCursorV1,
) -> StoredRetiredEnvelopeDispositionV1 {
    match lookup {
        ModerationSubmissionLookupV1::Applied { transaction_id }
            if transaction_id == record.transaction_id =>
        {
            StoredRetiredEnvelopeDispositionV1::Applied
        }
        ModerationSubmissionLookupV1::Pending { transaction_id }
            if transaction_id == record.transaction_id =>
        {
            match record.disposition {
                StoredRetiredEnvelopeDispositionV1::Applied
                | StoredRetiredEnvelopeDispositionV1::Rejected => record.disposition,
                StoredRetiredEnvelopeDispositionV1::NotFound
                | StoredRetiredEnvelopeDispositionV1::Pending => {
                    StoredRetiredEnvelopeDispositionV1::Pending
                }
            }
        }
        ModerationSubmissionLookupV1::Rejected {
            transaction_id: Some(transaction_id),
            observed_finalized_height,
        } if transaction_id == record.transaction_id
            && observed_finalized_height >= record.retired_at_finalized_height
            && observed_finalized_height <= cursor.height =>
        {
            if record.disposition == StoredRetiredEnvelopeDispositionV1::Applied {
                StoredRetiredEnvelopeDispositionV1::Applied
            } else {
                StoredRetiredEnvelopeDispositionV1::Rejected
            }
        }
        ModerationSubmissionLookupV1::NotFound { .. }
        | ModerationSubmissionLookupV1::Pending { .. }
        | ModerationSubmissionLookupV1::Applied { .. }
        | ModerationSubmissionLookupV1::Rejected { .. }
        | ModerationSubmissionLookupV1::Unknown => record.disposition,
    }
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

fn panel_notification_scope_digest(case_id: &str, round_id: &str) -> [u8; 32] {
    domain_hash(
        PANEL_NOTIFICATION_SCOPE_DOMAIN_V1,
        &[case_id.as_bytes(), round_id.as_bytes()],
    )
}

fn panel_notification_id(
    chain_id: &iroha_data_model::ChainId,
    source_operation_id: [u8; 32],
    scope_digest: [u8; 32],
    kind: ModerationPanelNotificationKindV1,
    recipient: &AccountId,
    cursor: ModerationFinalizedEventCursorV1,
    source_occurred_at_unix_ms: u64,
) -> [u8; 32] {
    let kind = [kind.tag()];
    let recipient = recipient.to_string();
    let sequence = cursor.sequence.to_le_bytes();
    let block_height = cursor.block_height.to_le_bytes();
    let event_index = cursor.event_index.to_le_bytes();
    let source_occurred_at_unix_ms = source_occurred_at_unix_ms.to_le_bytes();
    domain_hash(
        PANEL_NOTIFICATION_ID_DOMAIN_V1,
        &[
            chain_id.as_str().as_bytes(),
            &source_operation_id,
            &scope_digest,
            &kind,
            recipient.as_bytes(),
            &sequence,
            &block_height,
            &cursor.block_hash,
            &event_index,
            &source_occurred_at_unix_ms,
        ],
    )
}

fn new_panel_notification_entry(
    chain_id: &iroha_data_model::ChainId,
    source_operation_id: [u8; 32],
    scope_digest: [u8; 32],
    kind: ModerationPanelNotificationKindV1,
    recipient: AccountId,
    event: &ModerationFinalizedEventV1,
    attempt_limit: u32,
) -> StoredPanelNotificationV1 {
    let cursor = event.cursor();
    let available_at_unix_ms = *event.event.occurred_at_unix_ms();
    let notification_id = panel_notification_id(
        chain_id,
        source_operation_id,
        scope_digest,
        kind,
        &recipient,
        cursor,
        available_at_unix_ms,
    );
    let mut entry = StoredPanelNotificationV1 {
        notification: ModerationPanelNotificationV1 {
            notification_id,
            source_operation_id,
            scope_digest,
            kind,
            recipient,
            finalized_event_cursor: cursor,
            source_occurred_at_unix_ms: available_at_unix_ms,
        },
        attempt_limit,
        attempts: 0,
        claim_generation: 0,
        available_at_unix_ms,
        state: StoredPanelNotificationStateV1::Pending,
        claimed_by: None,
        lease_token: None,
        claimed_at_unix_ms: None,
        lease_expires_at_unix_ms: None,
        receipt_digest: None,
        delivered_at_unix_ms: None,
        dead_letter_reason: None,
        dead_lettered_at_unix_ms: None,
        record_digest: [0; 32],
    };
    refresh_panel_notification_record_digest(&mut entry);
    entry
}

fn panel_notification_lease_token(
    notification_id: [u8; 32],
    worker_id: [u8; 32],
    claim_generation: u32,
    attempt: u32,
    claimed_at_unix_ms: u64,
    lease_expires_at_unix_ms: u64,
) -> [u8; 32] {
    let claim_generation = claim_generation.to_le_bytes();
    let attempt = attempt.to_le_bytes();
    let claimed_at_unix_ms = claimed_at_unix_ms.to_le_bytes();
    let lease_expires_at_unix_ms = lease_expires_at_unix_ms.to_le_bytes();
    domain_hash(
        PANEL_NOTIFICATION_LEASE_DOMAIN_V1,
        &[
            &notification_id,
            &worker_id,
            &claim_generation,
            &attempt,
            &claimed_at_unix_ms,
            &lease_expires_at_unix_ms,
        ],
    )
}

fn panel_notification_backoff_ms(attempts: u32) -> u64 {
    let shift = attempts.saturating_sub(1).min(63);
    MODERATION_PANEL_NOTIFICATION_BACKOFF_BASE_MS_V1
        .checked_shl(shift)
        .unwrap_or(u64::MAX)
        .min(MODERATION_PANEL_NOTIFICATION_BACKOFF_MAX_MS_V1)
}

fn clear_panel_notification_claim(entry: &mut StoredPanelNotificationV1) {
    entry.claimed_by = None;
    entry.lease_token = None;
    entry.claimed_at_unix_ms = None;
    entry.lease_expires_at_unix_ms = None;
}

fn dead_letter_panel_notification(
    entry: &mut StoredPanelNotificationV1,
    reason: ModerationPanelNotificationDeadLetterReasonV1,
    at_unix_ms: u64,
) {
    entry.state = StoredPanelNotificationStateV1::DeadLetter;
    clear_panel_notification_claim(entry);
    entry.receipt_digest = None;
    entry.delivered_at_unix_ms = None;
    entry.dead_letter_reason = Some(reason);
    entry.dead_lettered_at_unix_ms = Some(at_unix_ms);
    refresh_panel_notification_record_digest(entry);
}

fn validate_panel_notification_clock(
    state: &ModerationOrchestratorCheckpointV1,
    observed_unix_ms: u64,
) -> Result<(), ModerationOrchestratorError> {
    if observed_unix_ms == 0 {
        return Err(ModerationOrchestratorError::InvalidPanelNotificationClaim);
    }
    if observed_unix_ms < state.panel_notification_clock_unix_ms {
        return Err(
            ModerationOrchestratorError::PanelNotificationClockRollback {
                current: state.panel_notification_clock_unix_ms,
                observed: observed_unix_ms,
            },
        );
    }
    Ok(())
}

fn preflight_expired_panel_notification_claims(
    state: &ModerationOrchestratorCheckpointV1,
    now_unix_ms: u64,
) -> Result<(), ModerationOrchestratorError> {
    for entry in &state.panel_notifications {
        if entry.state != StoredPanelNotificationStateV1::Claimed {
            continue;
        }
        let lease_expires_at_unix_ms = entry.lease_expires_at_unix_ms.ok_or_else(|| {
            ModerationOrchestratorError::CheckpointCorrupt(
                "claimed panel notification has no lease expiry".to_owned(),
            )
        })?;
        if now_unix_ms >= lease_expires_at_unix_ms && entry.attempts < entry.attempt_limit {
            lease_expires_at_unix_ms
                .checked_add(panel_notification_backoff_ms(entry.attempts))
                .ok_or(ModerationOrchestratorError::GenerationOverflow)?;
        }
    }
    Ok(())
}

fn recover_expired_panel_notification_claims(
    state: &mut ModerationOrchestratorCheckpointV1,
    now_unix_ms: u64,
) -> Result<(), ModerationOrchestratorError> {
    for entry in &mut state.panel_notifications {
        if entry.state != StoredPanelNotificationStateV1::Claimed {
            continue;
        }
        let lease_expires_at_unix_ms = entry.lease_expires_at_unix_ms.ok_or_else(|| {
            ModerationOrchestratorError::CheckpointCorrupt(
                "claimed panel notification has no lease expiry".to_owned(),
            )
        })?;
        if now_unix_ms < lease_expires_at_unix_ms {
            continue;
        }
        if entry.attempts >= entry.attempt_limit {
            dead_letter_panel_notification(
                entry,
                ModerationPanelNotificationDeadLetterReasonV1::RetryExhausted,
                now_unix_ms,
            );
            continue;
        }
        let backoff = panel_notification_backoff_ms(entry.attempts);
        entry.available_at_unix_ms = lease_expires_at_unix_ms
            .checked_add(backoff)
            .ok_or(ModerationOrchestratorError::GenerationOverflow)?;
        entry.state = StoredPanelNotificationStateV1::Pending;
        clear_panel_notification_claim(entry);
        entry.receipt_digest = None;
        entry.delivered_at_unix_ms = None;
        entry.dead_letter_reason = None;
        entry.dead_lettered_at_unix_ms = None;
        refresh_panel_notification_record_digest(entry);
    }
    Ok(())
}

fn panel_notification_status(
    entry: &StoredPanelNotificationV1,
) -> Result<ModerationPanelNotificationStatusV1, ModerationOrchestratorError> {
    match entry.state {
        StoredPanelNotificationStateV1::Pending => {
            Ok(ModerationPanelNotificationStatusV1::Pending {
                available_at_unix_ms: entry.available_at_unix_ms,
                attempts: entry.attempts,
                attempt_limit: entry.attempt_limit,
            })
        }
        StoredPanelNotificationStateV1::Claimed => {
            let worker_id = entry.claimed_by.ok_or_else(|| {
                ModerationOrchestratorError::CheckpointCorrupt(
                    "claimed panel notification has no worker".to_owned(),
                )
            })?;
            let lease_expires_at_unix_ms = entry.lease_expires_at_unix_ms.ok_or_else(|| {
                ModerationOrchestratorError::CheckpointCorrupt(
                    "claimed panel notification has no lease expiry".to_owned(),
                )
            })?;
            Ok(ModerationPanelNotificationStatusV1::Claimed {
                worker_id,
                lease_expires_at_unix_ms,
                attempts: entry.attempts,
                attempt_limit: entry.attempt_limit,
            })
        }
        StoredPanelNotificationStateV1::Delivered => {
            let receipt_digest = entry.receipt_digest.ok_or_else(|| {
                ModerationOrchestratorError::CheckpointCorrupt(
                    "delivered panel notification has no receipt".to_owned(),
                )
            })?;
            let delivered_at_unix_ms = entry.delivered_at_unix_ms.ok_or_else(|| {
                ModerationOrchestratorError::CheckpointCorrupt(
                    "delivered panel notification has no delivery time".to_owned(),
                )
            })?;
            Ok(ModerationPanelNotificationStatusV1::Delivered {
                receipt_digest,
                delivered_at_unix_ms,
                attempts: entry.attempts,
                attempt_limit: entry.attempt_limit,
            })
        }
        StoredPanelNotificationStateV1::DeadLetter => {
            let reason = entry.dead_letter_reason.ok_or_else(|| {
                ModerationOrchestratorError::CheckpointCorrupt(
                    "dead-letter panel notification has no reason".to_owned(),
                )
            })?;
            let dead_lettered_at_unix_ms = entry.dead_lettered_at_unix_ms.ok_or_else(|| {
                ModerationOrchestratorError::CheckpointCorrupt(
                    "dead-letter panel notification has no terminal time".to_owned(),
                )
            })?;
            Ok(ModerationPanelNotificationStatusV1::DeadLetter {
                reason,
                dead_lettered_at_unix_ms,
                attempts: entry.attempts,
                attempt_limit: entry.attempt_limit,
            })
        }
    }
}

fn panel_notification_record_digest(entry: &StoredPanelNotificationV1) -> [u8; 32] {
    let notification = &entry.notification;
    let kind = [notification.kind.tag()];
    let recipient = notification.recipient.to_string();
    let sequence = notification.finalized_event_cursor.sequence.to_le_bytes();
    let block_height = notification
        .finalized_event_cursor
        .block_height
        .to_le_bytes();
    let event_index = notification
        .finalized_event_cursor
        .event_index
        .to_le_bytes();
    let source_occurred_at_unix_ms = notification.source_occurred_at_unix_ms.to_le_bytes();
    let attempt_limit = entry.attempt_limit.to_le_bytes();
    let attempts = entry.attempts.to_le_bytes();
    let claim_generation = entry.claim_generation.to_le_bytes();
    let available_at_unix_ms = entry.available_at_unix_ms.to_le_bytes();
    let state = [match entry.state {
        StoredPanelNotificationStateV1::Pending => 0,
        StoredPanelNotificationStateV1::Claimed => 1,
        StoredPanelNotificationStateV1::Delivered => 2,
        StoredPanelNotificationStateV1::DeadLetter => 3,
    }];
    let claimed_by_presence = [u8::from(entry.claimed_by.is_some())];
    let claimed_by = entry.claimed_by.unwrap_or([0; 32]);
    let lease_token_presence = [u8::from(entry.lease_token.is_some())];
    let lease_token = entry.lease_token.unwrap_or([0; 32]);
    let claimed_at_presence = [u8::from(entry.claimed_at_unix_ms.is_some())];
    let claimed_at = entry.claimed_at_unix_ms.unwrap_or(0).to_le_bytes();
    let lease_expiry_presence = [u8::from(entry.lease_expires_at_unix_ms.is_some())];
    let lease_expiry = entry.lease_expires_at_unix_ms.unwrap_or(0).to_le_bytes();
    let receipt_presence = [u8::from(entry.receipt_digest.is_some())];
    let receipt_digest = entry.receipt_digest.unwrap_or([0; 32]);
    let delivered_at_presence = [u8::from(entry.delivered_at_unix_ms.is_some())];
    let delivered_at = entry.delivered_at_unix_ms.unwrap_or(0).to_le_bytes();
    let dead_reason = [entry.dead_letter_reason.map_or(0, |reason| match reason {
        ModerationPanelNotificationDeadLetterReasonV1::PermanentRejection => 1,
        ModerationPanelNotificationDeadLetterReasonV1::RetryExhausted => 2,
    })];
    let dead_at_presence = [u8::from(entry.dead_lettered_at_unix_ms.is_some())];
    let dead_at = entry.dead_lettered_at_unix_ms.unwrap_or(0).to_le_bytes();
    domain_hash(
        PANEL_NOTIFICATION_RECORD_DOMAIN_V1,
        &[
            &notification.notification_id,
            &notification.source_operation_id,
            &notification.scope_digest,
            &kind,
            recipient.as_bytes(),
            &sequence,
            &block_height,
            &notification.finalized_event_cursor.block_hash,
            &event_index,
            &source_occurred_at_unix_ms,
            &attempt_limit,
            &attempts,
            &claim_generation,
            &available_at_unix_ms,
            &state,
            &claimed_by_presence,
            &claimed_by,
            &lease_token_presence,
            &lease_token,
            &claimed_at_presence,
            &claimed_at,
            &lease_expiry_presence,
            &lease_expiry,
            &receipt_presence,
            &receipt_digest,
            &delivered_at_presence,
            &delivered_at,
            &dead_reason,
            &dead_at_presence,
            &dead_at,
        ],
    )
}

fn refresh_panel_notification_record_digest(entry: &mut StoredPanelNotificationV1) {
    entry.record_digest = panel_notification_record_digest(entry);
}

fn panel_notification_outbox_digest(state: &ModerationOrchestratorCheckpointV1) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(PANEL_NOTIFICATION_OUTBOX_DOMAIN_V1);
    hasher.update(&state.panel_notification_clock_unix_ms.to_le_bytes());
    match state.panel_notification_scanned_cursor {
        Some(cursor) => {
            hasher.update(&[1]);
            hasher.update(&cursor.sequence.to_le_bytes());
            hasher.update(&cursor.block_height.to_le_bytes());
            hasher.update(&cursor.block_hash);
            hasher.update(&cursor.event_index.to_le_bytes());
        }
        None => {
            hasher.update(&[0]);
        }
    }
    hasher.update(
        &u64::try_from(state.panel_notifications.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for entry in &state.panel_notifications {
        hasher.update(&entry.notification.notification_id);
        hasher.update(&entry.record_digest);
    }
    *hasher.finalize().as_bytes()
}

fn refresh_panel_notification_outbox_digest(state: &mut ModerationOrchestratorCheckpointV1) {
    state.panel_notification_outbox_digest = panel_notification_outbox_digest(state);
}

fn checkpoint_decode_limits(max_bytes: u64) -> Result<DecodeLimits, ModerationOrchestratorError> {
    let max_bytes = usize::try_from(max_bytes).map_err(|_| {
        ModerationOrchestratorError::InvalidConfiguration(
            "checkpoint byte limit does not fit usize".to_owned(),
        )
    })?;
    Ok(DecodeLimits::new(
        max_bytes,
        max_bytes,
        max_bytes.saturating_mul(2),
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
    /// Authenticated principal differs from the action or finalized-ledger authority.
    #[error(
        "moderation authority mismatch for {action}: authenticated `{authenticated}`, required `{native}`"
    )]
    AuthorityMismatch {
        /// Action label.
        action: &'static str,
        /// Verified principal.
        authenticated: String,
        /// Authority required by the native payload or finalized projection.
        native: String,
    },
    /// Authenticated request binding is malformed or inert.
    #[error("invalid moderation authenticated request binding")]
    InvalidRequestBinding,
    /// Panel notification worker, lease, or clock input is inert.
    #[error("invalid moderation panel-notification claim")]
    InvalidPanelNotificationClaim,
    /// Panel notification receipt contains inert or inconsistent material.
    #[error("invalid moderation panel-notification delivery receipt")]
    InvalidPanelNotificationReceipt,
    /// Runtime notification time moved behind the durable high-water mark.
    #[error("moderation panel-notification clock rollback: current {current}, observed {observed}")]
    PanelNotificationClockRollback {
        /// Durable runtime clock high-water mark.
        current: u64,
        /// Regressed runtime observation.
        observed: u64,
    },
    /// A requested notification identity is not retained.
    #[error(
        "moderation panel notification {} is not retained",
        hex::encode(.notification_id)
    )]
    PanelNotificationNotFound {
        /// Stable notification identity.
        notification_id: [u8; 32],
    },
    /// The supplied worker or lease generation is stale or substituted.
    #[error(
        "moderation panel-notification claim conflict for {}",
        hex::encode(.notification_id)
    )]
    PanelNotificationClaimConflict {
        /// Stable notification identity.
        notification_id: [u8; 32],
    },
    /// A delivered notification was replayed with a different receipt.
    #[error(
        "moderation panel-notification receipt conflict for {}",
        hex::encode(.notification_id)
    )]
    PanelNotificationReceiptConflict {
        /// Stable notification identity.
        notification_id: [u8; 32],
    },
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
        collections::{BTreeMap, BTreeSet},
        num::NonZeroU32,
        sync::{
            Arc, Condvar, Mutex, Weak,
            atomic::{AtomicUsize, Ordering as AtomicOrdering},
            mpsc,
        },
        thread,
    };

    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        ChainId,
        events::data::sorafs::SorafsModerationLedgerEvent,
        metadata::Metadata,
        prelude::Json,
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
        transaction::{FeePaymentIntent, TransactionBuilder},
    };
    use tempfile::TempDir;

    use super::*;

    const TEST_ENVELOPE_CREATION_UNIX_MS: u64 = 1_700_000_000_000;

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
        sign_calls: usize,
        actions: Vec<ModerationNativeActionV1>,
        signed: BTreeMap<([u8; 32], u32), ModerationSignedTransactionV1>,
        operations: BTreeMap<([u8; 32], [u8; 32]), ModerationSubmissionLookupV1>,
        fallback: ModerationSubmissionLookupV1,
        sign_failure: Option<ModerationSubmissionFailureV1>,
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
                    sign_calls: 0,
                    actions: Vec::new(),
                    signed: BTreeMap::new(),
                    operations: BTreeMap::new(),
                    fallback,
                    sign_failure: None,
                    failure: None,
                    ambiguous_is_applied: false,
                }),
            }
        }

        fn ambiguous_applied(fallback: ModerationSubmissionLookupV1) -> Self {
            Self {
                state: Mutex::new(MockSubmitterState {
                    calls: 0,
                    sign_calls: 0,
                    actions: Vec::new(),
                    signed: BTreeMap::new(),
                    operations: BTreeMap::new(),
                    fallback,
                    sign_failure: None,
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

        fn sign_calls(&self) -> usize {
            self.state.lock().expect("submitter lock").sign_calls
        }

        fn set_failure(&self, failure: Option<ModerationSubmissionFailureV1>) {
            self.state.lock().expect("submitter lock").failure = failure;
        }

        fn set_sign_failure(&self, failure: Option<ModerationSubmissionFailureV1>) {
            self.state.lock().expect("submitter lock").sign_failure = failure;
        }

        fn set_lookup(
            &self,
            operation_id: [u8; 32],
            transaction_id: [u8; 32],
            lookup: ModerationSubmissionLookupV1,
        ) {
            self.state
                .lock()
                .expect("submitter lock")
                .operations
                .insert((operation_id, transaction_id), lookup);
        }
    }

    impl ModerationTransactionSubmitterV1 for MockSubmitter {
        fn chain_id(&self) -> ChainId {
            ChainId::from("moderation-orchestrator-test")
        }

        fn sign(
            &self,
            request: &ModerationTransactionRequestV1,
        ) -> Result<ModerationSignedTransactionV1, ModerationSubmissionFailureV1> {
            let mut state = self.state.lock().expect("submitter lock");
            state.sign_calls = state.sign_calls.saturating_add(1);
            if let Some(failure) = state.sign_failure {
                return Err(failure);
            }
            let signed_key = (request.operation_id, request.envelope_generation);
            if let Some(signed) = state.signed.get(&signed_key) {
                return Ok(signed.clone());
            }
            let signer = key_for_authority(&request.authority);
            let mut builder = TransactionBuilder::new(
                request.chain_id.clone(),
                request.authority.clone(),
                FeePaymentIntent::authority(Vec::new(), None),
            );
            builder.set_ttl(core::time::Duration::from_millis(
                MODERATION_TRANSACTION_TTL_MS_V1,
            ));
            let generation_offset = u64::from(request.envelope_generation.saturating_sub(1))
                .checked_mul(MODERATION_TRANSACTION_TTL_MS_V1.saturating_add(1))
                .ok_or(ModerationSubmissionFailureV1::PermanentRejection)?;
            let creation_time = TEST_ENVELOPE_CREATION_UNIX_MS
                .checked_add(generation_offset)
                .ok_or(ModerationSubmissionFailureV1::PermanentRejection)?;
            builder.set_creation_time(core::time::Duration::from_millis(creation_time));
            let transaction = builder
                .with_instructions([request.action.instruction()])
                .sign(signer.private_key());
            let signed =
                ModerationSignedTransactionV1::from_signed_transaction(request, &transaction)?;
            state.signed.insert(signed_key, signed.clone());
            Ok(signed)
        }

        fn submit_signed(
            &self,
            request: &ModerationTransactionRequestV1,
            signed: &ModerationSignedTransactionV1,
        ) -> Result<ModerationTransactionReceiptV1, ModerationSubmissionFailureV1> {
            signed.decode_for_request(request)?;
            let mut state = self.state.lock().expect("submitter lock");
            let lookup_key = (request.operation_id, signed.transaction_id);
            if let Some(existing) = state.operations.get(&lookup_key).copied() {
                let existing_transaction_id = match existing {
                    ModerationSubmissionLookupV1::Pending { transaction_id }
                    | ModerationSubmissionLookupV1::Applied { transaction_id } => transaction_id,
                    ModerationSubmissionLookupV1::Rejected {
                        transaction_id: Some(transaction_id),
                        ..
                    } => transaction_id,
                    ModerationSubmissionLookupV1::NotFound { .. }
                    | ModerationSubmissionLookupV1::Rejected {
                        transaction_id: None,
                        ..
                    }
                    | ModerationSubmissionLookupV1::Unknown => {
                        return Err(ModerationSubmissionFailureV1::Ambiguous);
                    }
                };
                return if existing_transaction_id == signed.transaction_id {
                    Ok(ModerationTransactionReceiptV1 {
                        transaction_id: signed.transaction_id,
                        observed_finalized_height: request.baseline_finalized_height,
                    })
                } else {
                    Err(ModerationSubmissionFailureV1::Ambiguous)
                };
            }
            state.calls = state.calls.saturating_add(1);
            state.actions.push(request.action.clone());
            if state.ambiguous_is_applied {
                state.operations.insert(
                    lookup_key,
                    ModerationSubmissionLookupV1::Applied {
                        transaction_id: signed.transaction_id,
                    },
                );
            }
            if let Some(failure) = state.failure {
                return Err(failure);
            }
            state.operations.insert(
                lookup_key,
                ModerationSubmissionLookupV1::Pending {
                    transaction_id: signed.transaction_id,
                },
            );
            Ok(ModerationTransactionReceiptV1 {
                transaction_id: signed.transaction_id,
                observed_finalized_height: request.baseline_finalized_height,
            })
        }

        fn lookup(
            &self,
            operation_id: [u8; 32],
            transaction_id: Option<[u8; 32]>,
        ) -> ModerationSubmissionLookupV1 {
            let state = self.state.lock().expect("submitter lock");
            transaction_id
                .and_then(|transaction_id| {
                    state
                        .operations
                        .get(&(operation_id, transaction_id))
                        .copied()
                })
                .unwrap_or(state.fallback)
        }
    }

    #[derive(Debug, Default)]
    struct MockHandoffSink {
        delivered: Mutex<Vec<[u8; 32]>>,
        calls: AtomicUsize,
    }

    impl MockHandoffSink {
        fn delivered(&self) -> Vec<[u8; 32]> {
            self.delivered.lock().expect("handoff sink lock").clone()
        }

        fn calls(&self) -> usize {
            self.calls.load(AtomicOrdering::Relaxed)
        }
    }

    impl ModerationTerminalHandoffSinkV1 for MockHandoffSink {
        fn deliver(
            &self,
            handoff: &ModerationTerminalHandoffV1,
        ) -> Result<(), ModerationHandoffFailureV1> {
            self.calls.fetch_add(1, AtomicOrdering::Relaxed);
            let mut delivered = self.delivered.lock().expect("handoff sink lock");
            if !delivered.contains(&handoff.handoff_id) {
                delivered.push(handoff.handoff_id);
            }
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    struct ReentrantLockProbe {
        orchestrator: Mutex<Option<Weak<ModerationOrchestratorV1>>>,
        checks: AtomicUsize,
    }

    impl ReentrantLockProbe {
        fn attach(&self, orchestrator: &Arc<ModerationOrchestratorV1>) {
            *self.orchestrator.lock().expect("probe lock") = Some(Arc::downgrade(orchestrator));
        }

        fn check(&self) {
            let orchestrator = self
                .orchestrator
                .lock()
                .expect("probe lock")
                .as_ref()
                .and_then(Weak::upgrade);
            let Some(orchestrator) = orchestrator else {
                return;
            };
            assert!(
                orchestrator.state.try_lock().is_ok(),
                "external collaborator ran while the orchestrator mutex was held"
            );
            let _ = orchestrator.snapshot();
            self.checks.fetch_add(1, AtomicOrdering::Relaxed);
        }

        fn checks(&self) -> usize {
            self.checks.load(AtomicOrdering::Relaxed)
        }
    }

    #[derive(Debug)]
    struct ProbedSnapshotReader {
        inner: Arc<MockSnapshotReader>,
        probe: Arc<ReentrantLockProbe>,
    }

    impl ModerationFinalizedSnapshotReaderV1 for ProbedSnapshotReader {
        fn read_finalized_snapshot(
            &self,
            max_cases: usize,
            max_events: usize,
        ) -> Result<ModerationFinalizedLedgerSnapshotV1, ModerationSnapshotReadErrorV1> {
            self.probe.check();
            self.inner.read_finalized_snapshot(max_cases, max_events)
        }
    }

    #[derive(Debug)]
    struct ProbedSubmitter {
        inner: Arc<MockSubmitter>,
        probe: Arc<ReentrantLockProbe>,
    }

    impl ModerationTransactionSubmitterV1 for ProbedSubmitter {
        fn chain_id(&self) -> ChainId {
            self.inner.chain_id()
        }

        fn sign(
            &self,
            request: &ModerationTransactionRequestV1,
        ) -> Result<ModerationSignedTransactionV1, ModerationSubmissionFailureV1> {
            self.probe.check();
            self.inner.sign(request)
        }

        fn submit_signed(
            &self,
            request: &ModerationTransactionRequestV1,
            signed: &ModerationSignedTransactionV1,
        ) -> Result<ModerationTransactionReceiptV1, ModerationSubmissionFailureV1> {
            self.probe.check();
            self.inner.submit_signed(request, signed)
        }

        fn lookup(
            &self,
            operation_id: [u8; 32],
            transaction_id: Option<[u8; 32]>,
        ) -> ModerationSubmissionLookupV1 {
            self.probe.check();
            self.inner.lookup(operation_id, transaction_id)
        }
    }

    #[derive(Debug)]
    struct ProbedHandoffSink {
        inner: Arc<MockHandoffSink>,
        probe: Arc<ReentrantLockProbe>,
    }

    impl ModerationTerminalHandoffSinkV1 for ProbedHandoffSink {
        fn deliver(
            &self,
            handoff: &ModerationTerminalHandoffV1,
        ) -> Result<(), ModerationHandoffFailureV1> {
            self.probe.check();
            self.inner.deliver(handoff)
        }
    }

    #[derive(Debug)]
    struct BlockingSignSubmitter {
        inner: Arc<MockSubmitter>,
        entered: Mutex<Option<mpsc::Sender<()>>>,
        released: Mutex<bool>,
        release: Condvar,
    }

    impl BlockingSignSubmitter {
        fn new(inner: Arc<MockSubmitter>, entered: mpsc::Sender<()>) -> Self {
            Self {
                inner,
                entered: Mutex::new(Some(entered)),
                released: Mutex::new(false),
                release: Condvar::new(),
            }
        }

        fn release(&self) {
            *self.released.lock().expect("release lock") = true;
            self.release.notify_all();
        }
    }

    impl ModerationTransactionSubmitterV1 for BlockingSignSubmitter {
        fn chain_id(&self) -> ChainId {
            self.inner.chain_id()
        }

        fn sign(
            &self,
            request: &ModerationTransactionRequestV1,
        ) -> Result<ModerationSignedTransactionV1, ModerationSubmissionFailureV1> {
            let entered = self.entered.lock().expect("entered lock").take();
            if let Some(entered) = entered {
                entered.send(()).expect("signal blocking signer");
                let released = self.released.lock().expect("release lock");
                drop(
                    self.release
                        .wait_while(released, |released| !*released)
                        .expect("wait for signer release"),
                );
            }
            self.inner.sign(request)
        }

        fn submit_signed(
            &self,
            request: &ModerationTransactionRequestV1,
            signed: &ModerationSignedTransactionV1,
        ) -> Result<ModerationTransactionReceiptV1, ModerationSubmissionFailureV1> {
            self.inner.submit_signed(request, signed)
        }

        fn lookup(
            &self,
            operation_id: [u8; 32],
            transaction_id: Option<[u8; 32]>,
        ) -> ModerationSubmissionLookupV1 {
            self.inner.lookup(operation_id, transaction_id)
        }
    }

    #[derive(Debug, Default)]
    struct MockPanelNotificationSink {
        calls: Mutex<usize>,
        receipts: Mutex<BTreeMap<[u8; 32], ModerationPanelNotificationDeliveryReceiptV1>>,
    }

    impl MockPanelNotificationSink {
        fn deliver(
            &self,
            claim: &ModerationPanelNotificationClaimV1,
            delivered_at_unix_ms: u64,
        ) -> ModerationPanelNotificationDeliveryReceiptV1 {
            let mut calls = self.calls.lock().expect("panel sink calls lock");
            *calls = calls.saturating_add(1);
            let mut receipts = self.receipts.lock().expect("panel sink receipt lock");
            *receipts
                .entry(claim.notification.notification_id)
                .or_insert_with(|| ModerationPanelNotificationDeliveryReceiptV1 {
                    notification_id: claim.notification.notification_id,
                    receipt_digest: domain_hash(
                        b"sorafs.moderation.test-panel-receipt.v1",
                        &[&claim.notification.notification_id],
                    ),
                    delivered_at_unix_ms,
                })
        }

        fn calls(&self) -> usize {
            *self.calls.lock().expect("panel sink calls lock")
        }

        fn unique_deliveries(&self) -> usize {
            self.receipts.lock().expect("panel sink receipt lock").len()
        }
    }

    fn account(seed: u8) -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![seed.max(1); 32], Algorithm::Ed25519)
            .expect("deterministic account");
        AccountId::new(keypair.public_key().clone())
    }

    fn key_for_authority(authority: &AccountId) -> KeyPair {
        (1_u8..=u8::MAX)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .expect("deterministic authority key")
            })
            .find(|key| key.public_key() == authority.signatory())
            .expect("test authority must use the deterministic account fixture")
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
            finalized_at_unix_ms: height.max(1),
            policy: None,
            status: None,
            appeals: Vec::new(),
            cases: Vec::new(),
            events: Vec::new(),
        }
    }

    fn empty_snapshot_at(
        height: u64,
        block_hash: [u8; 32],
        finalized_at_unix_ms: u64,
    ) -> ModerationFinalizedLedgerSnapshotV1 {
        let mut snapshot = empty_snapshot(height, block_hash);
        snapshot.finalized_at_unix_ms = finalized_at_unix_ms;
        snapshot
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
            finalized_at_unix_ms: height.max(1),
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
                finalized_at_unix_ms: 31,
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
            policy: appeal.policy,
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
        snapshot.finalized_at_unix_ms = FINALIZED_AT_UNIX_MS;
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

    fn seed_ready_operation_without_delivery(
        orchestrator: &ModerationOrchestratorV1,
        authority: AccountId,
        action: ModerationNativeActionV1,
        request_binding_digest: [u8; 32],
    ) -> [u8; 32] {
        orchestrator.reconcile().expect("initial reconciliation");
        let action_digest = action.action_digest().expect("action digest");
        let operation_id = action
            .operation_id(&orchestrator.chain_id, &authority)
            .expect("operation id");
        let mut state = orchestrator.state.lock().expect("orchestrator state");
        state.operations.push(StoredOperationV1 {
            operation_id,
            authority: authority.clone(),
            action_digest,
            status: StoredOperationStatusV1::Pending,
            transaction_id: None,
        });
        state.outbox.push(StoredOutboxEntryV1 {
            operation_id,
            authority,
            action,
            action_digest,
            request_binding_digest,
            envelope_generation: 1,
            retired_envelopes: Vec::new(),
            baseline_finalized_height: 0,
            baseline_finalized_block_hash: [0; 32],
            transaction_id: None,
            signed_transaction_digest: None,
            signed_transaction_bytes: None,
            attempts: 0,
            state: StoredOutboxStateV1::Ready,
            work_generation: 0,
            work_claim: None,
            last_lookup_finalized_height: 0,
            last_lookup_finalized_block_hash: [0; 32],
        });
        orchestrator
            .persist_checkpoint_locked(&mut state)
            .expect("persist ready operation");
        operation_id
    }

    fn execute_one_prepared_sign(orchestrator: &ModerationOrchestratorV1, operation_id: [u8; 32]) {
        let prepared = {
            let mut state = orchestrator.state.lock().expect("orchestrator state");
            orchestrator
                .prepare_next_external_work_locked(&mut state, &BTreeSet::new(), &BTreeSet::new())
                .expect("prepare signer work")
                .expect("one signer claim")
        };
        assert!(matches!(
            &prepared,
            PreparedExternalWorkV1::Sign { identity, .. }
                if identity.identity == operation_id
        ));
        orchestrator
            .execute_external_work(prepared)
            .expect("execute signer work");
    }

    fn prepare_one_submit(
        orchestrator: &ModerationOrchestratorV1,
        operation_id: [u8; 32],
    ) -> PreparedExternalWorkV1 {
        let prepared = {
            let mut state = orchestrator.state.lock().expect("orchestrator state");
            orchestrator
                .prepare_next_external_work_locked(&mut state, &BTreeSet::new(), &BTreeSet::new())
                .expect("prepare ingress work")
                .expect("one ingress claim")
        };
        assert!(matches!(
            &prepared,
            PreparedExternalWorkV1::Submit { identity, .. }
                if identity.identity == operation_id
        ));
        prepared
    }

    fn retained_envelope(
        orchestrator: &ModerationOrchestratorV1,
    ) -> (
        [u8; 32],
        u32,
        ModerationSignedTransactionV1,
        SignedEnvelopeTimingV1,
        StoredOutboxStateV1,
    ) {
        let state = orchestrator.state.lock().expect("orchestrator state");
        let [entry] = state.outbox.as_slice() else {
            panic!("one retained moderation envelope");
        };
        let request = moderation_transaction_request(&orchestrator.chain_id, entry)
            .expect("retained transaction request");
        let signed = moderation_signed_transaction(entry).expect("retained signed transaction");
        let transaction = signed
            .decode_for_request(&request)
            .expect("valid retained signed transaction");
        let timing = signed_envelope_timing(&transaction).expect("retained envelope timing");
        (
            entry.operation_id,
            entry.envelope_generation,
            signed,
            timing,
            entry.state,
        )
    }

    fn assert_finalized_authority_rejection_has_no_native_mutation(
        snapshot: ModerationFinalizedLedgerSnapshotV1,
        authenticated: AccountId,
        required: &AccountId,
        action: ModerationNativeActionV1,
    ) {
        let temp = tempfile::tempdir().expect("tempdir");
        let finalized_height = snapshot.finalized_height;
        let reader = Arc::new(MockSnapshotReader::new(snapshot));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: finalized_height,
        }));
        let orchestrator = ModerationOrchestratorV1::open(
            config(&temp, "authority-negative.norito"),
            deps(reader, Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        let action_label = action.label();

        let error = orchestrator
            .submit(authenticated.clone(), action, [0xE1; 32])
            .expect_err("non-ledger authority must fail closed");
        assert_eq!(
            error,
            ModerationOrchestratorError::AuthorityMismatch {
                action: action_label,
                authenticated: authenticated.to_string(),
                native: required.to_string(),
            }
        );
        assert_eq!(submitter.calls(), 0);
        let state = orchestrator.state.lock().expect("orchestrator state");
        assert!(state.operations.is_empty());
        assert!(state.outbox.is_empty());
        assert!(state.dead_letters.is_empty());
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
    fn finalize_sortition_rejects_non_policy_authority_without_mutation() {
        let governance = account(90);
        let imposter = account(91);
        let action =
            ModerationNativeActionV1::FinalizeSortition(FinalizeSorafsModerationSortition::new(
                "case-authority".to_owned(),
                "round-1".to_owned(),
                [0x31; 32],
                [0x32; 32],
                Vec::new(),
                Vec::new(),
            ));
        assert_finalized_authority_rejection_has_no_native_mutation(
            snapshot_with_policy(1, [1; 32], policy(1), governance.clone()),
            imposter,
            &governance,
            action,
        );
    }

    #[test]
    fn selection_governed_actions_reject_non_selected_authority_without_mutation() {
        let governance = account(90);
        let imposter = account(91);
        let (awaiting, sortition_digest) =
            awaiting_acceptance_snapshot(2, [2; 32], governance.clone());
        assert_finalized_authority_rejection_has_no_native_mutation(
            awaiting,
            imposter.clone(),
            &governance,
            ModerationNativeActionV1::ActivateCase(ActivateSorafsModerationCase::new(
                "case-failover".to_owned(),
                "round-1".to_owned(),
                sortition_digest,
            )),
        );

        for action in [
            ModerationNativeActionV1::ResolveChallenge(ResolveSorafsModerationChallenge::new(
                "case-failover".to_owned(),
                "round-1".to_owned(),
                "challenge-authority".to_owned(),
                ModerationChallengeDecisionV1::Rejected,
            )),
            ModerationNativeActionV1::FinalizeCase(FinalizeSorafsModerationCase::new(
                "case-failover".to_owned(),
                "round-1".to_owned(),
            )),
        ] {
            assert_finalized_authority_rejection_has_no_native_mutation(
                activated_case_snapshot(2, [2; 32], governance.clone()),
                imposter.clone(),
                &governance,
                action,
            );
        }
    }

    #[test]
    fn historical_operation_replay_precedes_rotated_finalized_authority() {
        let temp = tempfile::tempdir().expect("tempdir");
        let original_governance = account(90);
        let rotated_governance = account(91);
        let reader = Arc::new(MockSnapshotReader::new(snapshot_with_policy(
            1,
            [1; 32],
            policy(1),
            original_governance.clone(),
        )));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 1,
        }));
        let orchestrator = ModerationOrchestratorV1::open(
            config(&temp, "authority-replay.norito"),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        let action =
            ModerationNativeActionV1::FinalizeSortition(FinalizeSorafsModerationSortition::new(
                "case-authority-replay".to_owned(),
                "round-1".to_owned(),
                [0x41; 32],
                [0x42; 32],
                Vec::new(),
                Vec::new(),
            ));
        let first = orchestrator
            .submit(original_governance.clone(), action.clone(), [0xA1; 32])
            .expect("initial submission");
        assert!(!first.replay);
        assert_eq!(submitter.calls(), 1);

        let mut rotated_snapshot = snapshot_with_policy(2, [2; 32], policy(2), rotated_governance);
        rotated_snapshot.events[0].sequence = 2;
        reader.replace(rotated_snapshot);
        let replay = orchestrator
            .submit(original_governance, action, [0xA1; 32])
            .expect("retained historical replay");
        assert!(replay.replay);
        assert_eq!(replay.operation_id, first.operation_id);
        assert_eq!(submitter.calls(), 1);
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
    fn every_external_collaborator_is_reentrant_without_holding_the_state_mutex() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 1,
        }));
        let settlement = Arc::new(MockHandoffSink::default());
        let publication = Arc::new(MockHandoffSink::default());
        let probe = Arc::new(ReentrantLockProbe::default());
        let orchestrator = Arc::new(
            ModerationOrchestratorV1::open(
                config(&temp, "reentrant-collaborators.norito"),
                ModerationOrchestratorDepsV1 {
                    submitter: Arc::new(ProbedSubmitter {
                        inner: Arc::clone(&submitter),
                        probe: Arc::clone(&probe),
                    }),
                    snapshot_reader: Arc::new(ProbedSnapshotReader {
                        inner: Arc::clone(&reader),
                        probe: Arc::clone(&probe),
                    }),
                    settlement_sink: Arc::new(ProbedHandoffSink {
                        inner: Arc::clone(&settlement),
                        probe: Arc::clone(&probe),
                    }),
                    publication_sink: Arc::new(ProbedHandoffSink {
                        inner: Arc::clone(&publication),
                        probe: Arc::clone(&probe),
                    }),
                },
            )
            .expect("orchestrator"),
        );
        probe.attach(&orchestrator);

        orchestrator
            .submit(account(1), policy_action(policy(1)), [0x45; 32])
            .expect("sign and submit outside the mutex");
        orchestrator.reconcile().expect("lookup outside the mutex");

        let governance = account(99);
        let open = activated_case_snapshot(2, [2; 32], governance.clone());
        reader.replace(finalized_case_snapshot(open, 3, [3; 32], governance));
        orchestrator
            .reconcile()
            .expect("terminal sinks outside the mutex");

        assert!(probe.checks() >= 6);
        assert_eq!(settlement.delivered().len(), 1);
        assert_eq!(publication.delivered().len(), 1);
    }

    #[test]
    fn blocking_signer_claim_allows_concurrent_duplicate_worker_to_exit() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let inner = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 1,
        }));
        let (entered_tx, entered_rx) = mpsc::channel();
        let blocking = Arc::new(BlockingSignSubmitter::new(Arc::clone(&inner), entered_tx));
        let orchestrator = Arc::new(
            ModerationOrchestratorV1::open(
                config(&temp, "blocking-duplicate-workers.norito"),
                ModerationOrchestratorDepsV1 {
                    submitter: blocking.clone(),
                    snapshot_reader: reader,
                    settlement_sink: Arc::new(MockHandoffSink::default()),
                    publication_sink: Arc::new(MockHandoffSink::default()),
                },
            )
            .expect("orchestrator"),
        );
        seed_ready_operation_without_delivery(
            &orchestrator,
            account(1),
            policy_action(policy(1)),
            [0x46; 32],
        );

        let first = {
            let orchestrator = Arc::clone(&orchestrator);
            thread::spawn(move || orchestrator.drive_external_work())
        };
        entered_rx
            .recv_timeout(core::time::Duration::from_secs(5))
            .expect("signer entered");
        let lock_was_free = orchestrator.state.try_lock().is_ok();
        let (duplicate_tx, duplicate_rx) = mpsc::channel();
        let duplicate = {
            let orchestrator = Arc::clone(&orchestrator);
            thread::spawn(move || {
                let result = orchestrator.drive_external_work();
                duplicate_tx.send(result).expect("signal duplicate worker");
            })
        };
        let duplicate_result = duplicate_rx.recv_timeout(core::time::Duration::from_secs(5));
        blocking.release();
        first
            .join()
            .expect("first worker thread")
            .expect("first worker finishes");
        duplicate.join().expect("duplicate worker thread");

        assert!(lock_was_free);
        duplicate_result
            .expect("duplicate worker exits while signer is blocked")
            .expect("duplicate worker exits without duplicate work");
        assert_eq!(inner.sign_calls(), 1);
        assert_eq!(inner.calls(), 1);
    }

    #[test]
    fn generic_signed_envelope_contract_rejects_chain_ttl_nonce_metadata_and_action_substitution() {
        let chain_id = ChainId::from("moderation-orchestrator-test");
        let authority = account(1);
        let action = policy_action(policy(1));
        let request = ModerationTransactionRequestV1::new(
            chain_id.clone(),
            1,
            authority.clone(),
            action.clone(),
            [0x71; 32],
            7,
            [0x72; 32],
        )
        .expect("canonical generic request");
        let other_chain_request = ModerationTransactionRequestV1::new(
            ChainId::from("other-moderation-chain"),
            1,
            authority.clone(),
            action.clone(),
            [0x71; 32],
            7,
            [0x72; 32],
        )
        .expect("canonical cross-chain request");
        assert_ne!(request.operation_id, other_chain_request.operation_id);
        let next_generation_request = ModerationTransactionRequestV1::new(
            chain_id.clone(),
            2,
            authority.clone(),
            action.clone(),
            [0x71; 32],
            8,
            [0x73; 32],
        )
        .expect("canonical next-generation request");
        assert_eq!(request.operation_id, next_generation_request.operation_id);
        let mut zero_generation_request = request.clone();
        zero_generation_request.envelope_generation = 0;
        assert!(matches!(
            zero_generation_request.validate(),
            Err(ModerationOrchestratorError::InvalidAction(message))
                if message.contains("generation")
        ));
        let signer = key_for_authority(&authority);

        let exact_builder = || {
            TransactionBuilder::new(
                chain_id.clone(),
                authority.clone(),
                FeePaymentIntent::authority(Vec::new(), None),
            )
        };
        let sign_exact = |mut builder: TransactionBuilder, instruction: InstructionBox| {
            builder.set_ttl(core::time::Duration::from_millis(
                MODERATION_TRANSACTION_TTL_MS_V1,
            ));
            builder
                .with_instructions([instruction])
                .sign(signer.private_key())
        };

        let exact = sign_exact(exact_builder(), action.instruction());
        ModerationSignedTransactionV1::from_signed_transaction(&request, &exact)
            .expect("exact generic signed envelope");

        let wrong_chain = sign_exact(
            TransactionBuilder::new(
                ChainId::from("other-moderation-chain"),
                authority.clone(),
                FeePaymentIntent::authority(Vec::new(), None),
            ),
            action.instruction(),
        );
        assert_eq!(
            ModerationSignedTransactionV1::from_signed_transaction(&request, &wrong_chain),
            Err(ModerationSubmissionFailureV1::PermanentRejection),
        );

        let mut wrong_ttl_builder = exact_builder();
        wrong_ttl_builder.set_ttl(core::time::Duration::from_millis(
            MODERATION_TRANSACTION_TTL_MS_V1 + 1,
        ));
        let wrong_ttl = wrong_ttl_builder
            .with_instructions([action.instruction()])
            .sign(signer.private_key());
        assert_eq!(
            ModerationSignedTransactionV1::from_signed_transaction(&request, &wrong_ttl),
            Err(ModerationSubmissionFailureV1::PermanentRejection),
        );

        let mut nonce_builder = exact_builder();
        nonce_builder.set_ttl(core::time::Duration::from_millis(
            MODERATION_TRANSACTION_TTL_MS_V1,
        ));
        nonce_builder.set_nonce(NonZeroU32::new(9).expect("non-zero nonce"));
        let with_nonce = nonce_builder
            .with_instructions([action.instruction()])
            .sign(signer.private_key());
        assert_eq!(
            ModerationSignedTransactionV1::from_signed_transaction(&request, &with_nonce),
            Err(ModerationSubmissionFailureV1::PermanentRejection),
        );

        let mut metadata = Metadata::default();
        metadata.insert(
            "moderation_action_hint"
                .parse()
                .expect("valid metadata key"),
            Json::new("set_policy".to_owned()),
        );
        let with_metadata = sign_exact(
            exact_builder().with_metadata(metadata),
            action.instruction(),
        );
        assert_eq!(
            ModerationSignedTransactionV1::from_signed_transaction(&request, &with_metadata),
            Err(ModerationSubmissionFailureV1::PermanentRejection),
        );

        let substituted_action = ModerationNativeActionV1::FinalizeCase(
            FinalizeSorafsModerationCase::new("case-substitute".to_owned(), "round-1".to_owned()),
        );
        let substituted = sign_exact(exact_builder(), substituted_action.instruction());
        assert_eq!(
            ModerationSignedTransactionV1::from_signed_transaction(&request, &substituted),
            Err(ModerationSubmissionFailureV1::PermanentRejection),
        );
    }

    #[test]
    fn expired_finalized_not_found_renews_one_generation_and_preserves_history() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 1,
        }));
        let orchestrator = ModerationOrchestratorV1::open(
            config(&temp, "renew-expired.norito"),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        orchestrator
            .submit(account(1), policy_action(policy(1)), [0x81; 32])
            .expect("initial submission");
        let (operation_id, generation, first_signed, first_timing, state) =
            retained_envelope(&orchestrator);
        assert_eq!(generation, 1);
        assert_eq!(state, StoredOutboxStateV1::Submitted);

        submitter.set_lookup(
            operation_id,
            first_signed.transaction_id,
            ModerationSubmissionLookupV1::NotFound {
                observed_finalized_height: 2,
            },
        );
        reader.replace(empty_snapshot_at(
            2,
            [2; 32],
            first_timing.expires_at_unix_ms,
        ));
        orchestrator
            .reconcile()
            .expect("renew after exact finalized absence");

        let (_, generation, second_signed, _, state) = retained_envelope(&orchestrator);
        assert_eq!(generation, 2);
        assert_eq!(state, StoredOutboxStateV1::Submitted);
        assert_ne!(second_signed.transaction_id, first_signed.transaction_id);
        assert_ne!(
            second_signed.canonical_bytes_digest,
            first_signed.canonical_bytes_digest
        );
        assert_eq!(submitter.sign_calls(), 2);
        assert_eq!(submitter.calls(), 2);
        let state = orchestrator.state.lock().expect("orchestrator state");
        let [entry] = state.outbox.as_slice() else {
            panic!("one renewed outbox entry");
        };
        let [retired] = entry.retired_envelopes.as_slice() else {
            panic!("one retired envelope");
        };
        assert_eq!(retired.generation, 1);
        assert_eq!(retired.transaction_id, first_signed.transaction_id);
        assert_eq!(
            retired.signed_transaction_digest,
            first_signed.canonical_bytes_digest
        );
        assert_eq!(
            retired.disposition,
            StoredRetiredEnvelopeDispositionV1::NotFound
        );
        assert_eq!(
            retired.record_digest,
            retired_envelope_record_digest(operation_id, retired)
        );
    }

    #[test]
    fn expired_envelope_does_not_renew_for_positive_unknown_rejected_or_stale_absence() {
        let scenarios = [
            (
                "pending",
                ModerationSubmissionLookupV1::Pending {
                    transaction_id: [0; 32],
                },
            ),
            (
                "applied",
                ModerationSubmissionLookupV1::Applied {
                    transaction_id: [0; 32],
                },
            ),
            ("unknown", ModerationSubmissionLookupV1::Unknown),
            (
                "rejected",
                ModerationSubmissionLookupV1::Rejected {
                    transaction_id: Some([0; 32]),
                    observed_finalized_height: 2,
                },
            ),
            (
                "stale-not-found",
                ModerationSubmissionLookupV1::NotFound {
                    observed_finalized_height: 2,
                },
            ),
        ];
        for (index, (label, lookup)) in scenarios.into_iter().enumerate() {
            let temp = tempfile::tempdir().expect("tempdir");
            let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
            let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
            let orchestrator = ModerationOrchestratorV1::open(
                config(&temp, &format!("no-renew-{label}.norito")),
                deps(Arc::clone(&reader), Arc::clone(&submitter)),
            )
            .expect("orchestrator");
            orchestrator
                .submit(
                    account(1),
                    policy_action(policy(1)),
                    [u8::try_from(index).unwrap_or(0).saturating_add(1); 32],
                )
                .expect("initial submission");
            let (operation_id, _, signed, timing, _) = retained_envelope(&orchestrator);
            let exact_lookup = match lookup {
                ModerationSubmissionLookupV1::Pending { .. } => {
                    ModerationSubmissionLookupV1::Pending {
                        transaction_id: signed.transaction_id,
                    }
                }
                ModerationSubmissionLookupV1::Applied { .. } => {
                    ModerationSubmissionLookupV1::Applied {
                        transaction_id: signed.transaction_id,
                    }
                }
                ModerationSubmissionLookupV1::Rejected {
                    observed_finalized_height,
                    ..
                } => ModerationSubmissionLookupV1::Rejected {
                    transaction_id: Some(signed.transaction_id),
                    observed_finalized_height,
                },
                other => other,
            };
            submitter.set_lookup(operation_id, signed.transaction_id, exact_lookup);
            let finalized_height = if label == "stale-not-found" { 3 } else { 2 };
            reader.replace(empty_snapshot_at(
                finalized_height,
                [u8::try_from(finalized_height).unwrap_or(9); 32],
                timing.expires_at_unix_ms.saturating_add(1),
            ));
            orchestrator
                .reconcile()
                .expect("expired non-renewal reconciliation");
            assert_eq!(submitter.sign_calls(), 1, "{label}");
            let state = orchestrator.state.lock().expect("orchestrator state");
            if label == "rejected" {
                assert!(state.outbox.is_empty(), "{label}");
                assert_eq!(
                    state.operations[0].status,
                    StoredOperationStatusV1::Rejected
                );
            } else {
                let [entry] = state.outbox.as_slice() else {
                    panic!("{label}: one retained envelope");
                };
                assert_eq!(entry.envelope_generation, 1, "{label}");
                assert!(entry.retired_envelopes.is_empty(), "{label}");
                if matches!(label, "unknown" | "stale-not-found") {
                    assert_eq!(entry.state, StoredOutboxStateV1::Ambiguous, "{label}");
                }
            }
        }
    }

    #[test]
    fn ambiguous_submission_never_renews_without_exact_finalized_absence() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
        submitter.set_failure(Some(ModerationSubmissionFailureV1::Ambiguous));
        let orchestrator = ModerationOrchestratorV1::open(
            config(&temp, "ambiguous-no-renew.norito"),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        orchestrator
            .submit(account(1), policy_action(policy(1)), [0x82; 32])
            .expect("ambiguous submission retained");
        let (_, _, _, timing, state) = retained_envelope(&orchestrator);
        assert_eq!(state, StoredOutboxStateV1::Ambiguous);
        reader.replace(empty_snapshot_at(
            2,
            [2; 32],
            timing.expires_at_unix_ms.saturating_add(1),
        ));
        orchestrator
            .reconcile()
            .expect("unknown lookup remains ambiguous");
        let (_, generation, _, _, state) = retained_envelope(&orchestrator);
        assert_eq!(generation, 1);
        assert_eq!(state, StoredOutboxStateV1::Ambiguous);
        assert_eq!(submitter.sign_calls(), 1);
    }

    #[test]
    fn late_applied_retired_envelope_fences_new_generation_until_semantic_finality() {
        let temp = tempfile::tempdir().expect("tempdir");
        let authority = account(1);
        let active_policy = policy(1);
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 1,
        }));
        let orchestrator = ModerationOrchestratorV1::open(
            config(&temp, "late-old-envelope.norito"),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        orchestrator
            .submit(
                authority.clone(),
                policy_action(active_policy.clone()),
                [0x83; 32],
            )
            .expect("initial submission");
        let (operation_id, _, first_signed, first_timing, _) = retained_envelope(&orchestrator);
        submitter.set_lookup(
            operation_id,
            first_signed.transaction_id,
            ModerationSubmissionLookupV1::NotFound {
                observed_finalized_height: 2,
            },
        );
        reader.replace(empty_snapshot_at(
            2,
            [2; 32],
            first_timing.expires_at_unix_ms,
        ));
        orchestrator.reconcile().expect("renew envelope");
        let (_, generation, second_signed, second_timing, _) = retained_envelope(&orchestrator);
        assert_eq!(generation, 2);

        submitter.set_lookup(
            operation_id,
            first_signed.transaction_id,
            ModerationSubmissionLookupV1::Pending {
                transaction_id: first_signed.transaction_id,
            },
        );
        submitter.set_lookup(
            operation_id,
            second_signed.transaction_id,
            ModerationSubmissionLookupV1::NotFound {
                observed_finalized_height: 3,
            },
        );
        reader.replace(empty_snapshot_at(
            3,
            [3; 32],
            second_timing.expires_at_unix_ms.saturating_add(1),
        ));
        orchestrator
            .reconcile()
            .expect("late old pending result fences replacement");
        {
            let state = orchestrator.state.lock().expect("orchestrator state");
            let [entry] = state.outbox.as_slice() else {
                panic!("fenced renewed entry");
            };
            assert_eq!(entry.envelope_generation, 2);
            assert_eq!(
                entry.retired_envelopes[0].disposition,
                StoredRetiredEnvelopeDispositionV1::Pending
            );
            assert_eq!(
                state.operations[0].transaction_id,
                Some(first_signed.transaction_id)
            );
        }
        assert_eq!(submitter.sign_calls(), 2);
        assert_eq!(submitter.calls(), 2);

        submitter.set_lookup(
            operation_id,
            first_signed.transaction_id,
            ModerationSubmissionLookupV1::Applied {
                transaction_id: first_signed.transaction_id,
            },
        );
        reader.replace(empty_snapshot_at(
            4,
            [4; 32],
            second_timing
                .expires_at_unix_ms
                .saturating_add(MODERATION_TRANSACTION_TTL_MS_V1),
        ));
        orchestrator
            .reconcile()
            .expect("late old application makes the history fence terminal");
        {
            let state = orchestrator.state.lock().expect("orchestrator state");
            assert_eq!(
                state.outbox[0].retired_envelopes[0].disposition,
                StoredRetiredEnvelopeDispositionV1::Applied
            );
        }

        submitter.set_lookup(
            operation_id,
            first_signed.transaction_id,
            ModerationSubmissionLookupV1::NotFound {
                observed_finalized_height: 5,
            },
        );
        reader.replace(empty_snapshot_at(
            5,
            [5; 32],
            second_timing
                .expires_at_unix_ms
                .saturating_add(MODERATION_TRANSACTION_TTL_MS_V1.saturating_mul(2)),
        ));
        orchestrator
            .reconcile()
            .expect("applied history fence is sticky");
        assert_eq!(retained_envelope(&orchestrator).1, 2);
        assert_eq!(submitter.sign_calls(), 2);

        let mut finalized = snapshot_with_policy(6, [6; 32], active_policy, authority);
        finalized.finalized_at_unix_ms = second_timing
            .expires_at_unix_ms
            .saturating_add(MODERATION_TRANSACTION_TTL_MS_V1.saturating_mul(3));
        reader.replace(finalized);
        orchestrator
            .reconcile()
            .expect("authoritative semantic effect finalizes operation");
        let state = orchestrator.state.lock().expect("orchestrator state");
        assert!(state.outbox.is_empty());
        assert_eq!(
            state.operations[0].status,
            StoredOperationStatusV1::Finalized
        );
        assert_eq!(
            state.operations[0].transaction_id,
            Some(first_signed.transaction_id)
        );
    }

    #[test]
    fn restart_recovers_retired_generation_after_signer_outage() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 1,
        }));
        let checkpoint = config(&temp, "retired-before-resign.norito");
        let orchestrator = ModerationOrchestratorV1::open(
            checkpoint.clone(),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        orchestrator
            .submit(account(1), policy_action(policy(1)), [0x84; 32])
            .expect("initial submission");
        let (operation_id, _, first_signed, first_timing, _) = retained_envelope(&orchestrator);
        submitter.set_lookup(
            operation_id,
            first_signed.transaction_id,
            ModerationSubmissionLookupV1::NotFound {
                observed_finalized_height: 2,
            },
        );
        submitter.set_sign_failure(Some(ModerationSubmissionFailureV1::RuntimeUnavailable));
        reader.replace(empty_snapshot_at(
            2,
            [2; 32],
            first_timing.expires_at_unix_ms,
        ));
        orchestrator
            .reconcile()
            .expect("persist retired generation despite signer outage");
        {
            let state = orchestrator.state.lock().expect("orchestrator state");
            let [entry] = state.outbox.as_slice() else {
                panic!("one retired ready entry");
            };
            assert_eq!(entry.envelope_generation, 2);
            assert_eq!(entry.state, StoredOutboxStateV1::Ready);
            assert_eq!(entry.retired_envelopes.len(), 1);
            assert!(entry.transaction_id.is_none());
        }
        drop(orchestrator);

        submitter.set_sign_failure(None);
        let restarted =
            ModerationOrchestratorV1::open(checkpoint, deps(reader, Arc::clone(&submitter)))
                .expect("restart from retired ready generation");
        restarted
            .reconcile()
            .expect("sign and submit the next generation after restart");
        let (_, generation, second_signed, _, state) = retained_envelope(&restarted);
        assert_eq!(generation, 2);
        assert_eq!(state, StoredOutboxStateV1::Submitted);
        assert_ne!(second_signed.transaction_id, first_signed.transaction_id);
        assert_eq!(submitter.sign_calls(), 3);
    }

    #[test]
    fn renewed_envelope_restart_replays_byte_identical_bytes_without_resigning() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 1,
        }));
        let checkpoint = config(&temp, "renewed-byte-identical.norito");
        let orchestrator = ModerationOrchestratorV1::open(
            checkpoint.clone(),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        orchestrator
            .submit(account(1), policy_action(policy(1)), [0x85; 32])
            .expect("initial submission");
        let (operation_id, _, first_signed, first_timing, _) = retained_envelope(&orchestrator);
        submitter.set_lookup(
            operation_id,
            first_signed.transaction_id,
            ModerationSubmissionLookupV1::NotFound {
                observed_finalized_height: 2,
            },
        );
        submitter.set_failure(Some(ModerationSubmissionFailureV1::NotSubmittedUnavailable));
        reader.replace(empty_snapshot_at(
            2,
            [2; 32],
            first_timing.expires_at_unix_ms,
        ));
        orchestrator
            .reconcile()
            .expect("retain renewed envelope after definite non-submission");
        let (_, generation, retained, _, state) = retained_envelope(&orchestrator);
        assert_eq!(generation, 2);
        assert_eq!(state, StoredOutboxStateV1::Signed);
        assert_eq!(submitter.sign_calls(), 2);
        drop(orchestrator);

        submitter.set_failure(None);
        let restarted =
            ModerationOrchestratorV1::open(checkpoint, deps(reader, Arc::clone(&submitter)))
                .expect("restart with renewed retained bytes");
        restarted
            .reconcile()
            .expect("replay exact renewed envelope");
        let (_, generation, replayed, _, state) = retained_envelope(&restarted);
        assert_eq!(generation, 2);
        assert_eq!(state, StoredOutboxStateV1::Submitted);
        assert_eq!(replayed, retained);
        assert_eq!(submitter.sign_calls(), 2);
    }

    #[test]
    fn tampered_retired_envelope_history_fails_closed_on_restart() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 1,
        }));
        let checkpoint = config(&temp, "tampered-retired-history.norito");
        let orchestrator = ModerationOrchestratorV1::open(
            checkpoint.clone(),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        orchestrator
            .submit(account(1), policy_action(policy(1)), [0x86; 32])
            .expect("initial submission");
        let (operation_id, _, first_signed, first_timing, _) = retained_envelope(&orchestrator);
        submitter.set_lookup(
            operation_id,
            first_signed.transaction_id,
            ModerationSubmissionLookupV1::NotFound {
                observed_finalized_height: 2,
            },
        );
        reader.replace(empty_snapshot_at(
            2,
            [2; 32],
            first_timing.expires_at_unix_ms,
        ));
        orchestrator.reconcile().expect("renew envelope");
        drop(orchestrator);

        let original = fs::read(&checkpoint.checkpoint_path).expect("read checkpoint");
        for tamper in 0_u8..3 {
            let mut state: ModerationOrchestratorCheckpointV1 =
                norito::decode_from_bytes(&original).expect("decode checkpoint");
            let retired = state.outbox[0]
                .retired_envelopes
                .first_mut()
                .expect("retired history");
            match tamper {
                0 => retired.transaction_id[0] ^= 0x80,
                1 => retired.signed_transaction_digest[0] ^= 0x80,
                2 => retired.record_digest[0] ^= 0x80,
                _ => unreachable!(),
            }
            write_atomic(
                &checkpoint.checkpoint_path,
                &norito::to_bytes(&state).expect("encode tampered history"),
            )
            .expect("write tampered history");
            assert!(matches!(
                ModerationOrchestratorV1::open(
                    checkpoint.clone(),
                    deps(Arc::clone(&reader), Arc::clone(&submitter)),
                ),
                Err(ModerationOrchestratorError::CheckpointCorrupt(_))
            ));
            write_atomic(&checkpoint.checkpoint_path, &original).expect("restore checkpoint");
        }
    }

    #[test]
    fn envelope_generation_increment_fails_closed_on_overflow() {
        assert_eq!(
            next_envelope_generation(u32::MAX),
            Err(ModerationOrchestratorError::GenerationOverflow)
        );
    }

    #[test]
    fn restart_reconciles_crash_before_ingress_without_replacing_signed_bytes() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 2,
        }));
        let checkpoint = config(&temp, "crash-before-ingress.norito");
        let orchestrator = ModerationOrchestratorV1::open(
            checkpoint.clone(),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        let operation_id = seed_ready_operation_without_delivery(
            &orchestrator,
            account(1),
            policy_action(policy(1)),
            [0x47; 32],
        );
        execute_one_prepared_sign(&orchestrator, operation_id);
        let interrupted = prepare_one_submit(&orchestrator, operation_id);
        let retained = match &interrupted {
            PreparedExternalWorkV1::Submit { signed, .. } => signed.clone(),
            _ => unreachable!("submit claim"),
        };
        drop(interrupted);
        drop(orchestrator);

        reader.replace(empty_snapshot(2, [2; 32]));
        let restarted =
            ModerationOrchestratorV1::open(checkpoint, deps(reader, Arc::clone(&submitter)))
                .expect("restart");
        restarted
            .reconcile()
            .expect("lookup proves no ingress before exact retry");

        let state = restarted.state.lock().expect("restarted state");
        let entry = state
            .outbox
            .iter()
            .find(|entry| entry.operation_id == operation_id)
            .expect("submitted entry");
        assert_eq!(entry.state, StoredOutboxStateV1::Submitted);
        assert_eq!(
            moderation_signed_transaction(entry).expect("retained exact envelope"),
            retained
        );
        assert_eq!(submitter.sign_calls(), 1);
        assert_eq!(submitter.calls(), 1);
    }

    #[test]
    fn restart_reconciles_crash_after_ingress_effect_without_duplicate_submit() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 2,
        }));
        let checkpoint = config(&temp, "crash-after-ingress.norito");
        let orchestrator = ModerationOrchestratorV1::open(
            checkpoint.clone(),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        let operation_id = seed_ready_operation_without_delivery(
            &orchestrator,
            account(1),
            policy_action(policy(1)),
            [0x48; 32],
        );
        execute_one_prepared_sign(&orchestrator, operation_id);
        let interrupted = prepare_one_submit(&orchestrator, operation_id);
        let retained = match &interrupted {
            PreparedExternalWorkV1::Submit {
                request, signed, ..
            } => {
                submitter
                    .submit_signed(request, signed)
                    .expect("ingress effect before crash");
                signed.clone()
            }
            _ => unreachable!("submit claim"),
        };
        drop(interrupted);
        drop(orchestrator);

        reader.replace(empty_snapshot(2, [2; 32]));
        let restarted =
            ModerationOrchestratorV1::open(checkpoint, deps(reader, Arc::clone(&submitter)))
                .expect("restart");
        restarted
            .reconcile()
            .expect("lookup finds the pre-crash ingress effect");

        let state = restarted.state.lock().expect("restarted state");
        let entry = state
            .outbox
            .iter()
            .find(|entry| entry.operation_id == operation_id)
            .expect("submitted entry");
        assert_eq!(entry.state, StoredOutboxStateV1::Submitted);
        assert_eq!(
            moderation_signed_transaction(entry).expect("retained exact envelope"),
            retained
        );
        assert_eq!(submitter.sign_calls(), 1);
        assert_eq!(submitter.calls(), 1);
    }

    #[test]
    fn expired_work_lease_rejects_stale_signer_completion() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 1,
        }));
        let orchestrator = ModerationOrchestratorV1::open(
            config(&temp, "stale-signer-lease.norito"),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        let operation_id = seed_ready_operation_without_delivery(
            &orchestrator,
            account(1),
            policy_action(policy(1)),
            [0x49; 32],
        );
        let stale = {
            let mut state = orchestrator.state.lock().expect("orchestrator state");
            orchestrator
                .prepare_next_external_work_locked(&mut state, &BTreeSet::new(), &BTreeSet::new())
                .expect("prepare stale signer")
                .expect("stale signer claim")
        };
        assert!(matches!(
            &stale,
            PreparedExternalWorkV1::Sign { identity, claim, .. }
                if identity.identity == operation_id && claim.generation == 1
        ));

        reader.replace(empty_snapshot_at(
            2,
            [2; 32],
            1 + MODERATION_EXTERNAL_WORK_LEASE_MS_V1,
        ));
        orchestrator
            .reconcile()
            .expect("new generation reclaims expired signer lease");
        let before_stale_completion =
            fs::read(&orchestrator.config.checkpoint_path).expect("checkpoint before stale result");
        orchestrator
            .execute_external_work(stale)
            .expect("stale completion is ignored");
        let after_stale_completion =
            fs::read(&orchestrator.config.checkpoint_path).expect("checkpoint after stale result");

        assert_eq!(before_stale_completion, after_stale_completion);
        assert_eq!(submitter.sign_calls(), 2);
        assert_eq!(submitter.calls(), 1);
        let state = orchestrator.state.lock().expect("orchestrator state");
        let entry = state
            .outbox
            .iter()
            .find(|entry| entry.operation_id == operation_id)
            .expect("submitted entry");
        assert_eq!(entry.state, StoredOutboxStateV1::Submitted);
        assert!(entry.work_generation >= 3);
        assert!(entry.work_claim.is_none());
    }

    #[test]
    fn expired_ingress_lease_fences_stale_receipt_and_duplicate_effect() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 2,
        }));
        let orchestrator = ModerationOrchestratorV1::open(
            config(&temp, "stale-ingress-lease.norito"),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        let operation_id = seed_ready_operation_without_delivery(
            &orchestrator,
            account(1),
            policy_action(policy(1)),
            [0x4B; 32],
        );
        execute_one_prepared_sign(&orchestrator, operation_id);
        let stale = prepare_one_submit(&orchestrator, operation_id);
        assert!(matches!(
            &stale,
            PreparedExternalWorkV1::Submit { claim, .. }
                if claim.generation == 2
        ));

        reader.replace(empty_snapshot_at(
            2,
            [2; 32],
            1 + MODERATION_EXTERNAL_WORK_LEASE_MS_V1,
        ));
        orchestrator
            .reconcile()
            .expect("lookup and exact retry reclaim expired ingress lease");
        let before_stale_completion = fs::read(&orchestrator.config.checkpoint_path)
            .expect("checkpoint before stale receipt");
        orchestrator
            .execute_external_work(stale)
            .expect("stale ingress receipt is ignored");
        let after_stale_completion =
            fs::read(&orchestrator.config.checkpoint_path).expect("checkpoint after stale receipt");

        assert_eq!(before_stale_completion, after_stale_completion);
        assert_eq!(submitter.sign_calls(), 1);
        assert_eq!(submitter.calls(), 1);
        let state = orchestrator.state.lock().expect("orchestrator state");
        let entry = state
            .outbox
            .iter()
            .find(|entry| entry.operation_id == operation_id)
            .expect("submitted entry");
        assert_eq!(entry.state, StoredOutboxStateV1::Submitted);
        assert_eq!(entry.attempts, 2);
        assert!(entry.work_claim.is_none());
    }

    #[test]
    fn tampered_external_work_claim_fails_closed_on_restart() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
        let checkpoint = config(&temp, "tampered-external-claim.norito");
        let orchestrator = ModerationOrchestratorV1::open(
            checkpoint.clone(),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        let operation_id = seed_ready_operation_without_delivery(
            &orchestrator,
            account(1),
            policy_action(policy(1)),
            [0x4A; 32],
        );
        let claimed = {
            let mut state = orchestrator.state.lock().expect("orchestrator state");
            orchestrator
                .prepare_next_external_work_locked(&mut state, &BTreeSet::new(), &BTreeSet::new())
                .expect("prepare signer claim")
                .expect("one signer claim")
        };
        assert!(matches!(
            claimed,
            PreparedExternalWorkV1::Sign { identity, .. }
                if identity.identity == operation_id
        ));
        drop(orchestrator);

        let bytes = fs::read(&checkpoint.checkpoint_path).expect("read claimed checkpoint");
        let mut state: ModerationOrchestratorCheckpointV1 =
            norito::decode_from_bytes(&bytes).expect("decode claimed checkpoint");
        state.outbox[0]
            .work_claim
            .as_mut()
            .expect("retained work claim")
            .lease_token[0] ^= 0x80;
        write_atomic(
            &checkpoint.checkpoint_path,
            &norito::to_bytes(&state).expect("encode tampered claim"),
        )
        .expect("write tampered claim");

        assert!(matches!(
            ModerationOrchestratorV1::open(checkpoint, deps(reader, submitter)),
            Err(ModerationOrchestratorError::CheckpointCorrupt(message))
                if message.contains("external-work claim")
        ));
    }

    #[test]
    fn restart_submits_the_exact_envelope_persisted_before_ingress() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 1,
        }));
        let checkpoint = config(&temp, "signed-before-ingress.norito");
        let orchestrator = ModerationOrchestratorV1::open(
            checkpoint.clone(),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        let operation_id = seed_ready_operation_without_delivery(
            &orchestrator,
            account(1),
            policy_action(policy(1)),
            [0x51; 32],
        );
        execute_one_prepared_sign(&orchestrator, operation_id);
        let (retained_id, retained_digest, retained_bytes) = {
            let state = orchestrator.state.lock().expect("orchestrator state");
            let entry = state
                .outbox
                .iter()
                .find(|entry| entry.operation_id == operation_id)
                .expect("signed entry");
            assert_eq!(entry.state, StoredOutboxStateV1::Signed);
            (
                entry.transaction_id.expect("transaction id"),
                entry.signed_transaction_digest.expect("transaction digest"),
                entry
                    .signed_transaction_bytes
                    .clone()
                    .expect("signed transaction bytes"),
            )
        };
        assert_eq!(submitter.sign_calls(), 1);
        assert_eq!(submitter.calls(), 0);
        drop(orchestrator);

        let restarted =
            ModerationOrchestratorV1::open(checkpoint, deps(reader, Arc::clone(&submitter)))
                .expect("restart from signed checkpoint");
        restarted
            .reconcile()
            .expect("submit retained envelope after restart");
        let state = restarted.state.lock().expect("restarted state");
        let entry = state
            .outbox
            .iter()
            .find(|entry| entry.operation_id == operation_id)
            .expect("submitted entry");
        assert_eq!(entry.state, StoredOutboxStateV1::Submitted);
        assert_eq!(entry.transaction_id, Some(retained_id));
        assert_eq!(entry.signed_transaction_digest, Some(retained_digest));
        assert_eq!(
            entry.signed_transaction_bytes.as_deref(),
            Some(retained_bytes.as_slice())
        );
        assert_eq!(submitter.sign_calls(), 1);
        assert_eq!(submitter.calls(), 1);
    }

    #[test]
    fn restart_recovers_signing_claim_to_unsigned_ready_only() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
        let checkpoint = config(&temp, "interrupted-signing.norito");
        let orchestrator = ModerationOrchestratorV1::open(
            checkpoint.clone(),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        let operation_id = seed_ready_operation_without_delivery(
            &orchestrator,
            account(1),
            policy_action(policy(1)),
            [0x52; 32],
        );
        let interrupted = {
            let mut state = orchestrator.state.lock().expect("orchestrator state");
            orchestrator
                .prepare_next_external_work_locked(&mut state, &BTreeSet::new(), &BTreeSet::new())
                .expect("prepare interrupted signer work")
                .expect("one interrupted signer claim")
        };
        assert!(matches!(
            interrupted,
            PreparedExternalWorkV1::Sign { identity, .. }
                if identity.identity == operation_id
        ));
        drop(orchestrator);

        let restarted = ModerationOrchestratorV1::open(checkpoint, deps(reader, submitter))
            .expect("recover signer-only crash state");
        let state = restarted.state.lock().expect("restarted state");
        let entry = state
            .outbox
            .iter()
            .find(|entry| entry.operation_id == operation_id)
            .expect("recovered entry");
        assert_eq!(entry.state, StoredOutboxStateV1::Ready);
        assert_eq!(entry.baseline_finalized_height, 0);
        assert_eq!(entry.baseline_finalized_block_hash, [0; 32]);
        assert!(entry.transaction_id.is_none());
        assert!(entry.signed_transaction_digest.is_none());
        assert!(entry.signed_transaction_bytes.is_none());
        assert_eq!(entry.attempts, 0);
    }

    #[test]
    fn tampered_retained_transaction_bytes_digest_and_hash_fail_closed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
        let checkpoint = config(&temp, "tampered-signed.norito");
        let orchestrator = ModerationOrchestratorV1::open(
            checkpoint.clone(),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        let operation_id = seed_ready_operation_without_delivery(
            &orchestrator,
            account(1),
            policy_action(policy(1)),
            [0x53; 32],
        );
        execute_one_prepared_sign(&orchestrator, operation_id);
        drop(orchestrator);

        let original =
            fs::read(&checkpoint.checkpoint_path).expect("read canonical signed checkpoint");
        for tamper in 0_u8..3 {
            let mut state: ModerationOrchestratorCheckpointV1 =
                norito::decode_from_bytes(&original).expect("decode checkpoint");
            let entry = state.outbox.first_mut().expect("signed outbox entry");
            match tamper {
                0 => {
                    let bytes = entry
                        .signed_transaction_bytes
                        .as_mut()
                        .expect("signed bytes");
                    let last = bytes.last_mut().expect("non-empty signed bytes");
                    *last ^= 0x80;
                    let digest = signed_transaction_digest(bytes);
                    entry.signed_transaction_digest = Some(digest);
                }
                1 => {
                    entry
                        .signed_transaction_digest
                        .as_mut()
                        .expect("signed digest")[0] ^= 0x80;
                }
                2 => {
                    entry.transaction_id.as_mut().expect("transaction id")[0] ^= 0x80;
                    state.operations[0]
                        .transaction_id
                        .as_mut()
                        .expect("operation transaction id")[0] ^= 0x80;
                }
                _ => unreachable!(),
            }
            write_atomic(
                &checkpoint.checkpoint_path,
                &norito::to_bytes(&state).expect("encode tampered checkpoint"),
            )
            .expect("write tampered checkpoint");
            assert!(matches!(
                ModerationOrchestratorV1::open(
                    checkpoint.clone(),
                    deps(Arc::clone(&reader), Arc::clone(&submitter)),
                ),
                Err(ModerationOrchestratorError::CheckpointCorrupt(_))
            ));
            write_atomic(&checkpoint.checkpoint_path, &original).expect("restore checkpoint");
        }
    }

    #[test]
    fn definitely_not_submitted_reuses_retained_envelope_without_resigning() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(1, [1; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 1,
        }));
        submitter.set_failure(Some(ModerationSubmissionFailureV1::NotSubmittedUnavailable));
        let orchestrator = ModerationOrchestratorV1::open(
            config(&temp, "not-submitted.norito"),
            deps(reader, Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        orchestrator
            .submit(account(1), policy_action(policy(1)), [0x54; 32])
            .expect("retain exact envelope after pre-ingress failure");
        let retained = {
            let state = orchestrator.state.lock().expect("orchestrator state");
            let [entry] = state.outbox.as_slice() else {
                panic!("one retained outbox entry");
            };
            assert_eq!(entry.state, StoredOutboxStateV1::Signed);
            moderation_signed_transaction(entry).expect("retained envelope")
        };
        assert_eq!(submitter.sign_calls(), 1);
        assert_eq!(submitter.calls(), 1);

        submitter.set_failure(None);
        orchestrator
            .reconcile()
            .expect("retry the exact retained envelope");
        let state = orchestrator.state.lock().expect("orchestrator state");
        let [entry] = state.outbox.as_slice() else {
            panic!("one submitted outbox entry");
        };
        assert_eq!(entry.state, StoredOutboxStateV1::Submitted);
        assert_eq!(
            moderation_signed_transaction(entry).expect("submitted retained envelope"),
            retained
        );
        assert_eq!(submitter.sign_calls(), 1);
        assert_eq!(submitter.calls(), 2);
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
        let retained_transaction_id;
        let retained_transaction_digest;
        {
            let orchestrator = ModerationOrchestratorV1::open(
                checkpoint.clone(),
                deps(Arc::clone(&reader), Arc::clone(&submitter)),
            )
            .expect("orchestrator");
            orchestrator
                .submit(authority.clone(), policy_action(active_policy), [0x11; 32])
                .expect("ambiguous submit remains pending");
            let state = orchestrator.state.lock().expect("orchestrator state");
            let [entry] = state.outbox.as_slice() else {
                panic!("one ambiguous outbox entry must remain");
            };
            assert_eq!(entry.state, StoredOutboxStateV1::Ambiguous);
            retained_transaction_id = entry.transaction_id.expect("retained transaction id");
            retained_transaction_digest = entry
                .signed_transaction_digest
                .expect("retained transaction digest");
            let retained_bytes = entry
                .signed_transaction_bytes
                .as_deref()
                .expect("retained signed bytes");
            assert_eq!(
                signed_transaction_digest(retained_bytes),
                retained_transaction_digest
            );
        }

        reader.replace(empty_snapshot(2, [2; 32]));
        let restarted = ModerationOrchestratorV1::open(
            checkpoint.clone(),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("restart with retained exact transaction");
        restarted
            .reconcile()
            .expect("exact transaction lookup after restart");
        {
            let state = restarted.state.lock().expect("restarted state");
            let [entry] = state.outbox.as_slice() else {
                panic!("one submitted outbox entry must remain");
            };
            assert_eq!(entry.state, StoredOutboxStateV1::Submitted);
            assert_eq!(entry.transaction_id, Some(retained_transaction_id));
            assert_eq!(
                entry.signed_transaction_digest,
                Some(retained_transaction_digest)
            );
        }
        assert_eq!(submitter.calls(), 1);
        assert_eq!(submitter.sign_calls(), 1);

        reader.replace(snapshot_with_policy(
            3,
            [3; 32],
            active_policy,
            authority.clone(),
        ));
        restarted.reconcile().expect("finalized reconciliation");
        let replay = restarted
            .submit(authority, policy_action(active_policy), [0x11; 32])
            .expect("finalized replay");

        assert_eq!(submitter.calls(), 1);
        assert_eq!(submitter.sign_calls(), 1);
        assert_eq!(replay.status, ModerationOperationStatusV1::Finalized);
        assert!(replay.replay);
    }

    #[test]
    fn terminal_handoff_crash_after_effect_retries_same_id_after_restart() {
        let temp = tempfile::tempdir().expect("tempdir");
        let governance = account(99);
        let finalized = finalized_case_snapshot(
            activated_case_snapshot(2, [2; 32], governance.clone()),
            3,
            [3; 32],
            governance,
        );
        let reader = Arc::new(MockSnapshotReader::new(finalized));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 3,
        }));
        let settlement = Arc::new(MockHandoffSink::default());
        let publication = Arc::new(MockHandoffSink::default());
        let checkpoint = config(&temp, "handoff-crash-after-effect.norito");
        let runtime_deps = || ModerationOrchestratorDepsV1 {
            submitter: submitter.clone(),
            snapshot_reader: reader.clone(),
            settlement_sink: settlement.clone(),
            publication_sink: publication.clone(),
        };
        let orchestrator = ModerationOrchestratorV1::open(checkpoint.clone(), runtime_deps())
            .expect("orchestrator");
        let (snapshot, digest) = orchestrator
            .read_validated_finalized_snapshot()
            .expect("read finalized snapshot");
        {
            let mut state = orchestrator.state.lock().expect("orchestrator state");
            orchestrator
                .install_finalized_snapshot_locked(&mut state, snapshot, digest)
                .expect("queue terminal handoffs");
        }
        let interrupted = {
            let mut state = orchestrator.state.lock().expect("orchestrator state");
            orchestrator
                .prepare_next_external_work_locked(&mut state, &BTreeSet::new(), &BTreeSet::new())
                .expect("prepare terminal handoff")
                .expect("one terminal handoff claim")
        };
        let handoff = match &interrupted {
            PreparedExternalWorkV1::Handoff { handoff, .. } => handoff.clone(),
            _ => unreachable!("terminal handoff claim"),
        };
        assert_eq!(handoff.kind, ModerationTerminalHandoffKindV1::Settlement);
        settlement
            .deliver(&handoff)
            .expect("sink effect before checkpoint finalization");
        drop(interrupted);
        drop(orchestrator);

        let restarted =
            ModerationOrchestratorV1::open(checkpoint, runtime_deps()).expect("restart");
        restarted
            .reconcile()
            .expect("retry identical terminal handoff after crash");
        assert_eq!(settlement.calls(), 2);
        assert_eq!(settlement.delivered(), vec![handoff.handoff_id]);
        assert_eq!(publication.calls(), 1);
        assert_eq!(publication.delivered().len(), 1);
        let state = restarted.state.lock().expect("restarted state");
        assert!(state.pending_handoffs.is_empty());
        assert_eq!(state.completed_handoffs.len(), 2);
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
            .run_maintenance(governance.clone(), 1)
            .expect("first failover scan");
        let replay = orchestrator
            .run_maintenance(governance, 1)
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
    fn same_finalized_tip_produces_byte_identical_maintenance_actions_across_replicas() {
        let temp = tempfile::tempdir().expect("tempdir");
        let governance = account(100);
        let (snapshot, _) = awaiting_acceptance_snapshot(2, [2; 32], governance.clone());
        let first_submitter =
            Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
                observed_finalized_height: 2,
            }));
        let second_submitter =
            Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
                observed_finalized_height: 2,
            }));
        let first = ModerationOrchestratorV1::open(
            config(&temp, "replica-a.norito"),
            deps(
                Arc::new(MockSnapshotReader::new(snapshot.clone())),
                Arc::clone(&first_submitter),
            ),
        )
        .expect("first replica");
        let second = ModerationOrchestratorV1::open(
            config(&temp, "replica-b.norito"),
            deps(
                Arc::new(MockSnapshotReader::new(snapshot)),
                Arc::clone(&second_submitter),
            ),
        )
        .expect("second replica");

        let first_outcomes = first
            .run_maintenance(governance.clone(), 1)
            .expect("first replica maintenance");
        let second_outcomes = second
            .run_maintenance(governance, 1)
            .expect("second replica maintenance");
        let first_actions = first_submitter.actions();
        let second_actions = second_submitter.actions();

        assert_eq!(first_outcomes.len(), 1);
        assert_eq!(second_outcomes.len(), 1);
        assert_eq!(
            first_outcomes[0].operation_id,
            second_outcomes[0].operation_id
        );
        assert_eq!(first_actions, second_actions);
        assert_eq!(
            norito::to_bytes(&first_actions).expect("encode first actions"),
            norito::to_bytes(&second_actions).expect("encode second actions")
        );
    }

    #[test]
    fn finalized_panel_notifications_are_operation_bound_payload_free_and_byte_identical() {
        let temp = tempfile::tempdir().expect("tempdir");
        let governance = account(99);
        let (snapshot, _) = awaiting_acceptance_snapshot(2, [0x22; 32], governance.clone());
        let selection = snapshot.appeals[0]
            .appeal
            .selection
            .as_ref()
            .expect("selection")
            .clone();
        let expected_source_operation =
            ModerationNativeActionV1::FinalizeSortition(FinalizeSorafsModerationSortition::new(
                "case-failover".to_owned(),
                "round-1".to_owned(),
                snapshot.appeals[0].appeal.pop_snapshot_digest,
                selection.randomness_anchor,
                selection.jurors.clone(),
                selection.waitlist.clone(),
            ))
            .operation_id(&ChainId::from("moderation-orchestrator-test"), &governance)
            .expect("source operation");
        let first = ModerationOrchestratorV1::open(
            config(&temp, "panel-replica-a.norito"),
            deps(
                Arc::new(MockSnapshotReader::new(snapshot.clone())),
                Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
            ),
        )
        .expect("first orchestrator");
        let second = ModerationOrchestratorV1::open(
            config(&temp, "panel-replica-b.norito"),
            deps(
                Arc::new(MockSnapshotReader::new(snapshot)),
                Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
            ),
        )
        .expect("second orchestrator");
        first.reconcile().expect("first reconciliation");
        second.reconcile().expect("second reconciliation");

        let first_entries = first
            .state
            .lock()
            .expect("first state")
            .panel_notifications
            .clone();
        let second_entries = second
            .state
            .lock()
            .expect("second state")
            .panel_notifications
            .clone();
        assert_eq!(first_entries.len(), 3);
        assert_eq!(
            first_entries
                .iter()
                .filter(|entry| {
                    entry.notification.kind == ModerationPanelNotificationKindV1::PrimaryAssignment
                })
                .count(),
            2
        );
        assert_eq!(
            first_entries
                .iter()
                .filter(|entry| {
                    entry.notification.kind == ModerationPanelNotificationKindV1::WaitlistStandby
                })
                .count(),
            1
        );
        assert!(first_entries.iter().all(|entry| {
            entry.notification.source_operation_id == expected_source_operation
                && entry.notification.finalized_event_cursor.sequence == 5
                && entry.notification.source_occurred_at_unix_ms == 21
        }));
        let first_bytes = norito::to_bytes(&first_entries).expect("encode first notifications");
        let second_bytes = norito::to_bytes(&second_entries).expect("encode second notifications");
        assert_eq!(first_bytes, second_bytes);
        assert_eq!(
            std::fs::read(&first.config().checkpoint_path).expect("read first checkpoint"),
            std::fs::read(&second.config().checkpoint_path).expect("read second checkpoint")
        );
        for forbidden in [b"case-failover".as_slice(), b"round-1", b"ipfs://"] {
            assert!(
                !first_bytes
                    .windows(forbidden.len())
                    .any(|window| window == forbidden),
                "payload-free checkpoint leaked {}",
                String::from_utf8_lossy(forbidden)
            );
        }
        for forbidden_digest in [[0x41; 32], [0x43; 32]] {
            assert!(
                !first_bytes
                    .windows(forbidden_digest.len())
                    .any(|window| window == forbidden_digest.as_slice()),
                "payload-free checkpoint retained a private intake digest"
            );
        }
    }

    #[test]
    fn finalized_activation_notifies_only_the_authoritative_ballot_roster() {
        let temp = tempfile::tempdir().expect("tempdir");
        let governance = account(99);
        let snapshot = activated_case_snapshot(3, [0x27; 32], governance.clone());
        let selection = snapshot.appeals[0]
            .appeal
            .selection
            .as_ref()
            .expect("selection");
        let expected_source_operation =
            ModerationNativeActionV1::ActivateCase(ActivateSorafsModerationCase::new(
                "case-failover".to_owned(),
                "round-1".to_owned(),
                selection.sortition_digest,
            ))
            .operation_id(&ChainId::from("moderation-orchestrator-test"), &governance)
            .expect("activation operation");
        let expected_recipients = snapshot.cases[0]
            .case
            .spec
            .jurors
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let orchestrator = ModerationOrchestratorV1::open(
            config(&temp, "panel-activation.norito"),
            deps(
                Arc::new(MockSnapshotReader::new(snapshot)),
                Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
            ),
        )
        .expect("orchestrator");
        orchestrator.reconcile().expect("queue activation notices");
        let state = orchestrator.state.lock().expect("state");
        let actual_recipients = state
            .panel_notifications
            .iter()
            .map(|entry| entry.notification.recipient.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(actual_recipients, expected_recipients);
        assert_eq!(state.panel_notifications.len(), 2);
        assert!(state.panel_notifications.iter().all(|entry| {
            entry.notification.kind == ModerationPanelNotificationKindV1::BallotActivated
                && entry.notification.source_operation_id == expected_source_operation
                && entry.notification.finalized_event_cursor.sequence == 6
        }));
    }

    #[test]
    fn panel_notification_terminal_compaction_preserves_scan_progress_and_live_work() {
        let temp = tempfile::tempdir().expect("tempdir");
        let governance = account(99);
        let (awaiting, _) = awaiting_acceptance_snapshot(2, [0x29; 32], governance.clone());
        let reader = Arc::new(MockSnapshotReader::new(awaiting));
        let mut bounds = config(&temp, "panel-compaction.norito");
        bounds.max_handoffs = 3;
        let orchestrator = ModerationOrchestratorV1::open(
            bounds,
            deps(
                Arc::clone(&reader),
                Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
            ),
        )
        .expect("orchestrator");
        orchestrator.reconcile().expect("queue assignments");
        reader.replace(activated_case_snapshot(3, [0x2A; 32], governance));
        assert!(matches!(
            orchestrator.reconcile(),
            Err(ModerationOrchestratorError::ResourceExhausted {
                resource: "panel notifications",
                limit: 3
            })
        ));
        assert_eq!(
            orchestrator
                .snapshot()
                .expect("failed queue preserves prior projection")
                .finalized_height,
            2
        );

        let sink = MockPanelNotificationSink::default();
        let assignments = orchestrator
            .claim_panel_notifications([0xA1; 32], 1_000, 3)
            .expect("claim assignments");
        for claim in &assignments {
            let receipt = sink.deliver(claim, 1_001);
            orchestrator
                .finalize_panel_notification_delivery(
                    claim.worker_id,
                    claim.lease_token,
                    receipt,
                    1_001,
                )
                .expect("finalize assignment");
        }
        orchestrator
            .reconcile()
            .expect("terminal assignment records compact for activation");
        {
            let state = orchestrator.state.lock().expect("state");
            assert_eq!(state.panel_notifications.len(), 3);
            assert_eq!(
                state
                    .panel_notifications
                    .iter()
                    .filter(|entry| {
                        entry.notification.kind
                            == ModerationPanelNotificationKindV1::BallotActivated
                    })
                    .count(),
                2
            );
            assert_eq!(
                state
                    .panel_notification_scanned_cursor
                    .expect("scan cursor")
                    .sequence,
                6
            );
        }
        orchestrator
            .reconcile()
            .expect("same finalized event cannot requeue compacted identities");
        assert_eq!(
            orchestrator
                .state
                .lock()
                .expect("state")
                .panel_notifications
                .len(),
            3
        );
    }

    #[test]
    fn panel_notification_claims_recover_crashes_and_finalize_one_stable_receipt() {
        let temp = tempfile::tempdir().expect("tempdir");
        let governance = account(99);
        let (snapshot, _) = awaiting_acceptance_snapshot(2, [0x23; 32], governance);
        let reader = Arc::new(MockSnapshotReader::new(snapshot));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
        let checkpoint = config(&temp, "panel-crash-recovery.norito");
        let orchestrator = ModerationOrchestratorV1::open(
            checkpoint.clone(),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        orchestrator.reconcile().expect("queue notifications");
        let first_claims = orchestrator
            .claim_panel_notifications([0xA1; 32], 1_000, 3)
            .expect("first worker claims all notifications");
        assert_eq!(first_claims.len(), 3);
        assert!(first_claims.iter().all(|claim| claim.attempt_limit == 3));
        assert!(
            orchestrator
                .claim_panel_notifications([0xB2; 32], 1_000, 3)
                .expect("duplicate worker scan")
                .is_empty()
        );
        let sink = MockPanelNotificationSink::default();
        let first_receipt = sink.deliver(&first_claims[0], 1_001);
        let target_id = first_claims[0].notification.notification_id;
        drop(orchestrator);

        let restarted = ModerationOrchestratorV1::open(checkpoint, deps(reader, submitter))
            .expect("restart with durable claims");
        assert!(
            restarted
                .claim_panel_notifications([0xB2; 32], 30_999, 3)
                .expect("leases remain exclusive before expiry")
                .is_empty()
        );
        assert!(
            restarted
                .claim_panel_notifications([0xB2; 32], 31_000, 3)
                .expect("expiry begins deterministic backoff")
                .is_empty()
        );
        let second_claims = restarted
            .claim_panel_notifications([0xB2; 32], 32_000, 3)
            .expect("expired claims are reclaimed after backoff");
        assert_eq!(second_claims.len(), 3);
        assert!(second_claims.iter().all(|claim| claim.attempt == 2));
        let second_claim = second_claims
            .iter()
            .find(|claim| claim.notification.notification_id == target_id)
            .expect("same notification reclaimed");
        let deduplicated_receipt = sink.deliver(second_claim, 32_001);
        assert_eq!(deduplicated_receipt, first_receipt);
        assert_eq!(sink.calls(), 2);
        assert_eq!(sink.unique_deliveries(), 1);

        assert!(matches!(
            restarted.finalize_panel_notification_delivery(
                first_claims[0].worker_id,
                first_claims[0].lease_token,
                first_receipt,
                32_001,
            ),
            Err(ModerationOrchestratorError::PanelNotificationClaimConflict {
                notification_id
            }) if notification_id == target_id
        ));
        assert_eq!(
            restarted
                .finalize_panel_notification_delivery(
                    second_claim.worker_id,
                    second_claim.lease_token,
                    deduplicated_receipt,
                    32_001,
                )
                .expect("reclaimed receipt finalization"),
            ModerationPanelNotificationFinalizeOutcomeV1::Delivered
        );
        assert_eq!(
            restarted
                .finalize_panel_notification_delivery(
                    second_claim.worker_id,
                    second_claim.lease_token,
                    deduplicated_receipt,
                    32_002,
                )
                .expect("idempotent receipt replay"),
            ModerationPanelNotificationFinalizeOutcomeV1::AlreadyDelivered
        );
        let mut substituted = deduplicated_receipt;
        substituted.receipt_digest = [0xEE; 32];
        assert!(matches!(
            restarted.finalize_panel_notification_delivery(
                second_claim.worker_id,
                second_claim.lease_token,
                substituted,
                32_003,
            ),
            Err(ModerationOrchestratorError::PanelNotificationReceiptConflict {
                notification_id
            }) if notification_id == target_id
        ));
        assert!(matches!(
            restarted
                .panel_notification_status(target_id)
                .expect("durable status"),
            Some(ModerationPanelNotificationStatusV1::Delivered {
                receipt_digest,
                attempts: 2,
                ..
            }) if receipt_digest == first_receipt.receipt_digest
        ));
    }

    #[test]
    fn panel_notification_backoff_poison_and_retry_exhaustion_are_bounded() {
        let temp = tempfile::tempdir().expect("tempdir");
        let governance = account(99);
        let (snapshot, _) = awaiting_acceptance_snapshot(2, [0x24; 32], governance.clone());
        let orchestrator = ModerationOrchestratorV1::open(
            config(&temp, "panel-retry.norito"),
            deps(
                Arc::new(MockSnapshotReader::new(snapshot.clone())),
                Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
            ),
        )
        .expect("retry orchestrator");
        orchestrator.reconcile().expect("queue retry fixtures");
        let first = orchestrator
            .claim_panel_notifications([0xA1; 32], 1_000, 3)
            .expect("first claims");
        for claim in &first {
            orchestrator
                .release_panel_notification_claim(
                    claim.notification.notification_id,
                    claim.worker_id,
                    claim.lease_token,
                    ModerationPanelNotificationFailureV1::NotDelivered,
                    1_001,
                )
                .expect("first safe failure");
        }
        assert!(
            orchestrator
                .claim_panel_notifications([0xB2; 32], 2_000, 3)
                .expect("backoff scan")
                .is_empty()
        );
        let second = orchestrator
            .claim_panel_notifications([0xB2; 32], 2_001, 3)
            .expect("second claims");
        for claim in &second {
            orchestrator
                .release_panel_notification_claim(
                    claim.notification.notification_id,
                    claim.worker_id,
                    claim.lease_token,
                    ModerationPanelNotificationFailureV1::Ambiguous,
                    2_002,
                )
                .expect("ambiguous delivery is safely retryable by identity");
        }
        let third = orchestrator
            .claim_panel_notifications([0xC3; 32], 4_002, 3)
            .expect("third claims");
        for claim in &third {
            orchestrator
                .release_panel_notification_claim(
                    claim.notification.notification_id,
                    claim.worker_id,
                    claim.lease_token,
                    ModerationPanelNotificationFailureV1::NotDelivered,
                    4_003,
                )
                .expect("exhaust final attempt");
            assert!(matches!(
                orchestrator
                    .panel_notification_status(claim.notification.notification_id)
                    .expect("retry terminal status"),
                Some(ModerationPanelNotificationStatusV1::DeadLetter {
                    reason: ModerationPanelNotificationDeadLetterReasonV1::RetryExhausted,
                    attempts: 3,
                    ..
                })
            ));
        }

        let poison = ModerationOrchestratorV1::open(
            config(&temp, "panel-poison.norito"),
            deps(
                Arc::new(MockSnapshotReader::new(snapshot)),
                Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
            ),
        )
        .expect("poison orchestrator");
        poison.reconcile().expect("queue poison fixture");
        let poison_claim = poison
            .claim_panel_notifications([0xD4; 32], 1_000, 1)
            .expect("poison claim")
            .into_iter()
            .next()
            .expect("one poison claim");
        poison
            .release_panel_notification_claim(
                poison_claim.notification.notification_id,
                poison_claim.worker_id,
                poison_claim.lease_token,
                ModerationPanelNotificationFailureV1::Permanent,
                1_001,
            )
            .expect("permanent failure dead letters");
        assert!(matches!(
            poison
                .panel_notification_status(poison_claim.notification.notification_id)
                .expect("poison status"),
            Some(ModerationPanelNotificationStatusV1::DeadLetter {
                reason: ModerationPanelNotificationDeadLetterReasonV1::PermanentRejection,
                attempts: 1,
                ..
            })
        ));
    }

    #[test]
    fn panel_notification_claim_inputs_tokens_and_clock_are_fail_closed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let governance = account(99);
        let (snapshot, _) = awaiting_acceptance_snapshot(2, [0x28; 32], governance);
        let mut bounds = config(&temp, "panel-negative-inputs.norito");
        bounds.max_handoffs = 3;
        let orchestrator = ModerationOrchestratorV1::open(
            bounds,
            deps(
                Arc::new(MockSnapshotReader::new(snapshot)),
                Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
            ),
        )
        .expect("orchestrator");
        orchestrator.reconcile().expect("queue notifications");
        assert_eq!(
            orchestrator.claim_panel_notifications([0; 32], 1_000, 1),
            Err(ModerationOrchestratorError::InvalidPanelNotificationClaim)
        );
        assert!(matches!(
            orchestrator.claim_panel_notifications([0xA1; 32], 1_000, 4),
            Err(ModerationOrchestratorError::ResourceExhausted {
                resource: "panel notification claim batch",
                limit: 3
            })
        ));
        assert_eq!(
            orchestrator.claim_panel_notifications([0xA1; 32], u64::MAX, 1),
            Err(ModerationOrchestratorError::GenerationOverflow)
        );
        let claim = orchestrator
            .claim_panel_notifications([0xA1; 32], 1_000, 1)
            .expect("valid claim")
            .into_iter()
            .next()
            .expect("one valid claim");
        assert_eq!(
            orchestrator.claim_panel_notifications([0xB2; 32], 999, 1),
            Err(
                ModerationOrchestratorError::PanelNotificationClockRollback {
                    current: 1_000,
                    observed: 999,
                }
            )
        );
        assert!(matches!(
            orchestrator.release_panel_notification_claim(
                claim.notification.notification_id,
                [0xB2; 32],
                claim.lease_token,
                ModerationPanelNotificationFailureV1::NotDelivered,
                1_001,
            ),
            Err(ModerationOrchestratorError::PanelNotificationClaimConflict {
                notification_id
            }) if notification_id == claim.notification.notification_id
        ));
        assert_eq!(
            orchestrator.finalize_panel_notification_delivery(
                claim.worker_id,
                claim.lease_token,
                ModerationPanelNotificationDeliveryReceiptV1 {
                    notification_id: claim.notification.notification_id,
                    receipt_digest: [0; 32],
                    delivered_at_unix_ms: 1_001,
                },
                1_001,
            ),
            Err(ModerationOrchestratorError::InvalidPanelNotificationReceipt)
        );
    }

    #[test]
    fn panel_notification_checkpoint_tampering_and_old_versions_fail_closed() {
        let temp = tempfile::tempdir().expect("tempdir");
        let governance = account(99);
        let (snapshot, _) = awaiting_acceptance_snapshot(2, [0x25; 32], governance);
        let reader = Arc::new(MockSnapshotReader::new(snapshot));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown));
        let bounds = config(&temp, "panel-tamper.norito");
        let orchestrator = ModerationOrchestratorV1::open(
            bounds.clone(),
            deps(Arc::clone(&reader), Arc::clone(&submitter)),
        )
        .expect("orchestrator");
        orchestrator.reconcile().expect("queue notifications");
        orchestrator
            .claim_panel_notifications([0xA1; 32], 1_000, 3)
            .expect("durable claims");
        drop(orchestrator);

        let original = std::fs::read(&bounds.checkpoint_path).expect("read checkpoint");
        let limits = checkpoint_decode_limits(bounds.checkpoint_max_bytes).expect("decode limits");
        let mut checkpoint =
            decode_from_bytes_with_limits::<ModerationOrchestratorCheckpointV1>(&original, limits)
                .expect("decode checkpoint");
        checkpoint.panel_notifications[0].lease_expires_at_unix_ms = checkpoint.panel_notifications
            [0]
        .lease_expires_at_unix_ms
        .map(|value| value.saturating_add(1));
        std::fs::write(
            &bounds.checkpoint_path,
            norito::to_bytes(&checkpoint).expect("encode tampered checkpoint"),
        )
        .expect("write tampered checkpoint");
        assert!(matches!(
            ModerationOrchestratorV1::open(
                bounds.clone(),
                deps(Arc::clone(&reader), Arc::clone(&submitter)),
            ),
            Err(ModerationOrchestratorError::CheckpointCorrupt(_))
        ));

        for version in [2, 3] {
            let mut old_checkpoint = decode_from_bytes_with_limits::<
                ModerationOrchestratorCheckpointV1,
            >(&original, limits)
            .expect("decode original checkpoint");
            old_checkpoint.version = version;
            std::fs::write(
                &bounds.checkpoint_path,
                norito::to_bytes(&old_checkpoint).expect("encode old checkpoint"),
            )
            .expect("write old checkpoint");
            assert!(matches!(
                ModerationOrchestratorV1::open(
                    bounds.clone(),
                    deps(Arc::clone(&reader), Arc::clone(&submitter)),
                ),
                Err(ModerationOrchestratorError::CheckpointCorrupt(message))
                    if message.contains("unsupported checkpoint version")
            ));
        }
    }

    #[test]
    fn panel_notification_source_provenance_mismatch_is_rejected() {
        let temp = tempfile::tempdir().expect("tempdir");
        let governance = account(99);
        let (mut snapshot, _) = awaiting_acceptance_snapshot(2, [0x26; 32], governance);
        snapshot.events[0].event = SorafsModerationLedgerEvent::new(
            SorafsModerationLedgerEventKind::SortitionFinalized,
            Some("case-failover".to_owned()),
            Some("round-1".to_owned()),
            account(98),
            21,
        );
        let orchestrator = ModerationOrchestratorV1::open(
            config(&temp, "panel-provenance.norito"),
            deps(
                Arc::new(MockSnapshotReader::new(snapshot)),
                Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
            ),
        )
        .expect("orchestrator");
        assert!(matches!(
            orchestrator.reconcile(),
            Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(message))
                if message.contains("sortition event provenance")
        ));
    }

    #[test]
    fn panel_notification_scan_rejects_cross_snapshot_event_gaps() {
        let temp = tempfile::tempdir().expect("tempdir");
        let governance = account(99);
        let (awaiting, _) = awaiting_acceptance_snapshot(2, [0x2B; 32], governance.clone());
        let reader = Arc::new(MockSnapshotReader::new(awaiting));
        let orchestrator = ModerationOrchestratorV1::open(
            config(&temp, "panel-event-gap.norito"),
            deps(
                Arc::clone(&reader),
                Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::Unknown)),
            ),
        )
        .expect("orchestrator");
        orchestrator.reconcile().expect("scan sortition event");
        let activated = activated_case_snapshot(3, [0x2C; 32], governance.clone());
        reader.replace(finalized_case_snapshot(
            activated, 4, [0x2D; 32], governance,
        ));
        assert!(matches!(
            orchestrator.reconcile(),
            Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(message))
                if message.contains("sequence gap")
        ));
    }

    #[test]
    fn same_tip_with_a_changed_finalized_timestamp_is_rejected_as_equivocation() {
        let temp = tempfile::tempdir().expect("tempdir");
        let reader = Arc::new(MockSnapshotReader::new(empty_snapshot(2, [2; 32])));
        let submitter = Arc::new(MockSubmitter::new(ModerationSubmissionLookupV1::NotFound {
            observed_finalized_height: 2,
        }));
        let orchestrator = ModerationOrchestratorV1::open(
            config(&temp, "timestamp-equivocation.norito"),
            deps(Arc::clone(&reader), submitter),
        )
        .expect("orchestrator");
        orchestrator.reconcile().expect("initial finalized tip");

        let mut forged = empty_snapshot(2, [2; 32]);
        forged.finalized_at_unix_ms = forged.finalized_at_unix_ms.saturating_add(1);
        reader.replace(forged);

        assert_eq!(
            orchestrator.reconcile(),
            Err(ModerationOrchestratorError::FinalizedEquivocation { height: 2 })
        );
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
