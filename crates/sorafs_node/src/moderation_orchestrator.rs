//! Finalized-chain SoraFS moderation orchestration.
//!
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
use iroha_config::parameters::{ProductionRuntimeHandleError, validate_production_runtime_handle};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PublicKey, Signature as IrohaSignature};
use iroha_data_model::{
    account::{AccountId, ParsedAccountId},
    block::BlockHeader,
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
            MODERATION_LEDGER_MAX_REASON_BYTES_V1, MODERATION_QUERY_MAX_CASES_V1,
            MODERATION_QUERY_MAX_EVENTS_V1, ModerationAppealStatusV1, ModerationCaseStatusV1,
            ModerationChallengeDecisionV1, ModerationSortitionError,
            is_canonical_moderation_identifier_v1, sorafs_moderation_select_panel_v1,
        },
    },
    transaction::{Executable, SignedTransaction},
};
use norito::{DecodeLimits, NoritoDeserialize, NoritoSerialize, decode_from_bytes_with_limits};
use rand::{rand_core::TryRngCore as _, rngs::OsRng};
use sorafs_manifest::pop_credentials::{POP_MEMBERSHIP_PROOF_MAX_BYTES_V1, PopMembershipProofV1};
use thiserror::Error;
#[path = "moderation_orchestrator/checkpoint_store.rs"]
mod checkpoint_store;
#[path = "moderation_orchestrator/terminal_handoff.rs"]
mod terminal_handoff;
pub use checkpoint_store::{
    MODERATION_CHECKPOINT_STORE_RECORD_VERSION_V1, ModerationCheckpointStoreExternalErrorV1,
    ModerationCheckpointStoreRecordV1, ModerationCheckpointStoreV1,
};
use terminal_handoff::{
    terminal_finalization_event_matches_outcome, terminal_handoff_id,
    validate_retained_terminal_handoff,
};
pub use iroha_data_model::sorafs::moderation_ledger::{
    MODERATION_FINALIZED_SNAPSHOT_VERSION_V1, ModerationFinalizedAppealViewV1,
    ModerationFinalizedCaseViewV1, ModerationFinalizedCursorV1, ModerationFinalizedEventCursorV1,
    ModerationFinalizedEventV1, ModerationFinalizedLedgerSnapshotV1,
};
/// Checkpoint schema version.
///
/// Version nine adds source-bound dead-letter resolution receipts plus bounded
/// native-operation and terminal-handoff archive tombstones. Earlier
/// pre-release state is intentionally rejected instead of migrated.
pub const MODERATION_ORCHESTRATOR_CHECKPOINT_VERSION_V1: u16 = 9;
/// Canonical panel-notification terminal-record archive schema version.
pub const MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1: u16 = 4;
/// Hard ceiling for terminal records in one canonical archive artifact.
pub const MODERATION_PANEL_NOTIFICATION_ARCHIVE_MAX_RECORDS_V1: usize = 65_536;
/// Exact maximum for the minimal source manifest plus canonical archive wrapper.
pub const MODERATION_PANEL_NOTIFICATION_ARCHIVE_WRAPPER_MAX_BYTES_V1: u64 = 1024 * 1024;
/// Maximum archive heads authenticated in one incremental audit page.
pub const MODERATION_PANEL_NOTIFICATION_ARCHIVE_AUDIT_PAGE_MAX_V1: u32 = 16;
/// Maximum authenticated archive-signer epochs retained in sealed state.
pub const MODERATION_PANEL_NOTIFICATION_ARCHIVE_MAX_SIGNER_EPOCHS_V1: usize = 256;
/// Exact runtime-provider broker slot bound into every archive receipt signature.
pub const MODERATION_PANEL_NOTIFICATION_ARCHIVE_BROKER_SLOT_V1: u16 = 55;
/// Existing sealed-checkpoint broker slot bound into terminal-source attestations.
pub const MODERATION_PANEL_NOTIFICATION_SOURCE_ATTESTOR_BROKER_SLOT_V1: u16 = 52;
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
const PANEL_NOTIFICATION_WORKER_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.panel-notification-worker.v1";
const PANEL_NOTIFICATION_RECORD_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.panel-notification-record.v1";
const PANEL_NOTIFICATION_OUTBOX_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.panel-notification-outbox.v1";
const PANEL_NOTIFICATION_ARCHIVE_PAYLOAD_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.panel-notification-archive-payload.v4";
const PANEL_NOTIFICATION_ARCHIVE_OPERATION_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.panel-notification-archive-operation.v4";
const PANEL_NOTIFICATION_ARCHIVE_HEAD_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.panel-notification-archive-head.v4";
const PANEL_NOTIFICATION_ARCHIVE_RECEIPT_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.panel-notification-archive-receipt.v4";
const PANEL_NOTIFICATION_ARCHIVE_SOURCE_MANIFEST_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.panel-notification-archive-source-manifest.v1";
const PANEL_NOTIFICATION_ARCHIVE_SOURCE_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.panel-notification-archive-source.v1";
const PANEL_NOTIFICATION_ARCHIVE_SOURCE_ATTESTATION_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.panel-notification-archive-source-attestation.v1";
const PANEL_NOTIFICATION_ARCHIVE_AUDIT_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.panel-notification-archive-audit.v1";
const PANEL_NOTIFICATION_ARCHIVE_SIGNER_EPOCH_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.panel-notification-archive-signer-epoch.v1";
const PANEL_NOTIFICATION_ARCHIVE_SIGNER_ROTATION_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.panel-notification-archive-signer-rotation.v1";
const PANEL_NOTIFICATION_ARCHIVE_SIGNER_POP_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.panel-notification-archive-signer-pop.v1";
const DEAD_LETTER_RESOLUTION_DOMAIN_V1: &[u8] = b"sorafs.moderation.dead-letter-resolution.v1";
const DURABLE_DEAD_LETTER_RECORD_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.durable-dead-letter-record.v1";
const COMPLETED_HANDOFF_RECORD_DOMAIN_V1: &[u8] = b"sorafs.moderation.completed-handoff-record.v1";
const NATIVE_OPERATION_RECORD_DOMAIN_V1: &[u8] = b"sorafs.moderation.native-operation-record.v1";
const TERMINAL_ARCHIVE_RECORD_KEY_DOMAIN_V1: &[u8] =
    b"sorafs.moderation.terminal-archive-record-key.v1";
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
        network_id: &iroha_data_model::NetworkId,
        authority: &AccountId,
    ) -> Result<[u8; 32], ModerationOrchestratorError> {
        let material = self.semantic_material(authority)?;
        Ok(domain_hash(
            OPERATION_ID_DOMAIN_V1,
            &[network_id.as_bytes(), material.as_slice()],
        ))
    }
}
/// Public, non-secret qualification for one moderation runtime provider.
///
/// `revision` identifies the deployment-owned adapter and public policy
/// revision. `policy_digest` binds that exact public policy. The orchestrator
/// pins both values before opening durable state and requires the same values
/// before and after every external provider operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModerationRuntimeProviderQualificationV1 {
    revision: u64,
    policy_digest: [u8; 32],
}
impl ModerationRuntimeProviderQualificationV1 {
    /// Construct one provider qualification observation.
    #[must_use]
    pub const fn new(revision: u64, policy_digest: [u8; 32]) -> Self {
        Self {
            revision,
            policy_digest,
        }
    }
    /// Return the non-zero deployment adapter/policy revision.
    #[must_use]
    pub const fn revision(self) -> u64 {
        self.revision
    }
    /// Return the non-zero digest of the public provider policy.
    #[must_use]
    pub const fn policy_digest(self) -> [u8; 32] {
        self.policy_digest
    }
    fn is_valid(self) -> bool {
        self.revision != 0 && self.policy_digest != [0; 32]
    }
}
/// Stable, payload-free moderation runtime-provider qualification failures.
///
/// Provider implementations keep credentials, key identifiers, and vendor
/// diagnostics behind this boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ModerationRuntimeProviderQualificationErrorV1 {
    /// The configured opaque provider handle is malformed.
    #[error("configured moderation runtime provider handle is invalid")]
    InvalidConfiguredHandle,
    /// The configured handle is explicitly marked for test or development use.
    #[error("configured moderation runtime provider handle is test-marked")]
    TestMarkedConfiguredHandle,
    /// The injected provider's opaque handle is malformed.
    #[error("injected moderation runtime provider handle is invalid")]
    InvalidProviderHandle,
    /// The injected provider advertises a test- or development-marked handle.
    #[error("injected moderation runtime provider handle is test-marked")]
    TestMarkedProviderHandle,
    /// The configured provider revision or public policy digest is zero.
    #[error("configured moderation runtime provider qualification is invalid")]
    InvalidConfiguredQualification,
    /// The injected provider does not match the configured stable handle.
    #[error("moderation runtime provider handle does not match configuration")]
    SubstitutedProvider,
    /// Qualification could not prove that the provider is current and usable.
    #[error("moderation runtime provider is unavailable, stale, or unqualified")]
    UnavailableOrStale,
    /// The provider returned a zero revision or all-zero public policy digest.
    #[error("moderation runtime provider returned an invalid qualification")]
    InvalidQualification,
    /// The provider does not match the independently governed qualification.
    #[error("moderation runtime provider qualification does not match configuration")]
    QualificationMismatch,
    /// The provider identity or public policy changed after it was pinned.
    #[error("moderation runtime provider identity or policy changed after qualification")]
    IdentityOrPolicyChanged,
    /// The immutable archive namespace changed after qualification.
    #[error("moderation receipt archive identity does not match configuration")]
    ArchiveIdentityChanged,
    /// The immutable archive signing key changed after qualification.
    #[error("moderation receipt archive public key does not match configuration")]
    ArchivePublicKeyChanged,
}
/// Fixed readiness failures returned by a moderation runtime provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ModerationRuntimeProviderReadinessErrorV1 {
    /// The provider or a required credential is temporarily unavailable.
    #[error("moderation runtime provider unavailable")]
    Unavailable,
    /// The provider is revoked, stale, unauthorized, or otherwise ineligible.
    #[error("moderation runtime provider rejected qualification")]
    Rejected,
}
/// Stable identity and readiness exposed by an external moderation provider.
///
/// Implementations own credentials, signing keys, authentication material,
/// and provider-specific diagnostics. `qualification` must fail when the
/// provider is unavailable, revoked, stale, test-marked, or otherwise not
/// production-ready.
pub trait ModerationRuntimeProviderV1: Send + Sync + fmt::Debug {
    /// Return the stable opaque deployment handle for this provider.
    fn handle(&self) -> &str;
    /// Qualify the active adapter and its public policy revision.
    fn qualification(
        &self,
    ) -> Result<ModerationRuntimeProviderQualificationV1, ModerationRuntimeProviderReadinessErrorV1>;
}
/// Qualify one provider against an independently configured exact binding.
///
/// # Errors
///
/// Fails for malformed or test-marked handles, unavailable providers, invalid
/// observations, substitutions, and revision or policy-digest mismatches.
pub fn qualify_moderation_runtime_provider_v1<P: ModerationRuntimeProviderV1 + ?Sized>(
    expected_handle: &str,
    expected_qualification: ModerationRuntimeProviderQualificationV1,
    provider: &P,
) -> Result<(), ModerationRuntimeProviderQualificationErrorV1> {
    validate_moderation_runtime_provider_handle(expected_handle, true)?;
    if !expected_qualification.is_valid() {
        return Err(ModerationRuntimeProviderQualificationErrorV1::InvalidConfiguredQualification);
    }
    validate_moderation_runtime_provider_handle(provider.handle(), false)?;
    if provider.handle() != expected_handle {
        return Err(ModerationRuntimeProviderQualificationErrorV1::SubstitutedProvider);
    }
    let qualification = provider
        .qualification()
        .map_err(|_| ModerationRuntimeProviderQualificationErrorV1::UnavailableOrStale)?;
    if !qualification.is_valid() {
        return Err(ModerationRuntimeProviderQualificationErrorV1::InvalidQualification);
    }
    if qualification != expected_qualification {
        return Err(ModerationRuntimeProviderQualificationErrorV1::QualificationMismatch);
    }
    if provider.handle() != expected_handle {
        return Err(ModerationRuntimeProviderQualificationErrorV1::IdentityOrPolicyChanged);
    }
    Ok(())
}
/// Revalidate an already pinned provider immediately around external work.
///
/// # Errors
///
/// Fails when readiness, identity, revision, or public policy differs from the
/// exact binding qualified at startup.
pub fn revalidate_moderation_runtime_provider_v1<P: ModerationRuntimeProviderV1 + ?Sized>(
    expected_handle: &str,
    expected_qualification: ModerationRuntimeProviderQualificationV1,
    provider: &P,
) -> Result<(), ModerationRuntimeProviderQualificationErrorV1> {
    if provider.handle() != expected_handle {
        return Err(ModerationRuntimeProviderQualificationErrorV1::IdentityOrPolicyChanged);
    }
    let qualification = provider
        .qualification()
        .map_err(|_| ModerationRuntimeProviderQualificationErrorV1::UnavailableOrStale)?;
    if !qualification.is_valid() {
        return Err(ModerationRuntimeProviderQualificationErrorV1::InvalidQualification);
    }
    if provider.handle() != expected_handle || qualification != expected_qualification {
        return Err(ModerationRuntimeProviderQualificationErrorV1::IdentityOrPolicyChanged);
    }
    Ok(())
}
fn validate_moderation_runtime_provider_handle(
    handle: &str,
    configured: bool,
) -> Result<(), ModerationRuntimeProviderQualificationErrorV1> {
    validate_production_runtime_handle(handle).map_err(|error| match (configured, error) {
        (true, ProductionRuntimeHandleError::InvalidSyntax) => {
            ModerationRuntimeProviderQualificationErrorV1::InvalidConfiguredHandle
        }
        (false, ProductionRuntimeHandleError::InvalidSyntax) => {
            ModerationRuntimeProviderQualificationErrorV1::InvalidProviderHandle
        }
        (true, ProductionRuntimeHandleError::TestMarked) => {
            ModerationRuntimeProviderQualificationErrorV1::TestMarkedConfiguredHandle
        }
        (false, ProductionRuntimeHandleError::TestMarked) => {
            ModerationRuntimeProviderQualificationErrorV1::TestMarkedProviderHandle
        }
    })
}
fn map_runtime_provider_qualification_error(
    _error: ModerationRuntimeProviderQualificationErrorV1,
) -> ModerationOrchestratorError {
    ModerationOrchestratorError::InvalidConfiguration(
        "moderation runtime provider binding is unavailable or invalid".to_owned(),
    )
}
/// Bounds, provider bindings, and durable path for one moderation orchestrator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationOrchestratorConfigV1 {
    /// Absolute private local checkpoint-cache path.
    pub checkpoint_path: PathBuf,
    /// Governed identity of the authoritative sealed checkpoint store.
    pub checkpoint_store_handle: String,
    /// Exact checkpoint-store adapter and public-policy qualification.
    pub expected_checkpoint_store_qualification: ModerationRuntimeProviderQualificationV1,
    /// Stable Ed25519 trust anchor for checkpoint attestations; V1 rotation preserves it.
    pub checkpoint_store_attestation_public_key: [u8; 32],
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
    /// Maximum canonical notification archive artifact bytes.
    pub panel_notification_archive_max_bytes: u64,
    /// Governed identity of the injected HSM transaction signer.
    pub transaction_signer_handle: String,
    /// Independently governed signer adapter and public-policy qualification.
    pub expected_transaction_signer_qualification: ModerationRuntimeProviderQualificationV1,
    /// Governed identity of the injected strict transaction ingress.
    pub strict_ingress_handle: String,
    /// Independently governed ingress adapter and public-policy qualification.
    pub expected_strict_ingress_qualification: ModerationRuntimeProviderQualificationV1,
    /// Governed identity of the durable appeal-finance handoff boundary.
    pub settlement_handoff_handle: String,
    /// Independently governed settlement adapter and public-policy qualification.
    pub expected_settlement_handoff_qualification: ModerationRuntimeProviderQualificationV1,
    /// Governed identity of the durable governance/publication handoff boundary.
    pub publication_handoff_handle: String,
    /// Independently governed publication adapter and public-policy qualification.
    pub expected_publication_handoff_qualification: ModerationRuntimeProviderQualificationV1,
    /// Governed identity of the durable panel-notification delivery boundary.
    pub panel_notification_handle: String,
    /// Independently governed notification adapter and public-policy qualification.
    pub expected_panel_notification_qualification: ModerationRuntimeProviderQualificationV1,
    /// Governed identity of the immutable notification-receipt archive.
    pub panel_notification_archive_handle: String,
    /// Independently governed archive adapter and public-policy qualification.
    pub expected_panel_notification_archive_qualification: ModerationRuntimeProviderQualificationV1,
    /// Stable non-secret archive namespace identity.
    pub panel_notification_archive_id: [u8; 32],
    /// Bootstrap Ed25519 archive signer pinned for recovery and epoch-log genesis.
    pub panel_notification_archive_bootstrap_public_key: [u8; 32],
    /// Exact Ed25519 key authenticating durable archive readback.
    pub panel_notification_archive_public_key: [u8; 32],
    /// Inclusive final generation authorized for the predecessor signer.
    pub panel_notification_archive_predecessor_revocation_generation: Option<u64>,
    /// Predecessor-key authorization of the configured signer transition.
    pub panel_notification_archive_predecessor_authorization_signature: Option<[u8; 64]>,
    /// Configured new-key proof of possession for the same transition.
    pub panel_notification_archive_new_key_possession_signature: Option<[u8; 64]>,
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
        if self.max_cases > MODERATION_QUERY_MAX_CASES_V1 as usize
            || self.max_events > MODERATION_QUERY_MAX_EVENTS_V1 as usize
        {
            return Err(ModerationOrchestratorError::InvalidConfiguration(
                "moderation snapshot bounds exceed the native query ceiling".to_owned(),
            ));
        }
        if self.max_handoffs > MODERATION_PANEL_NOTIFICATION_ARCHIVE_MAX_RECORDS_V1 {
            return Err(ModerationOrchestratorError::InvalidConfiguration(
                "max_handoffs exceeds the canonical notification archive record ceiling".to_owned(),
            ));
        }
        let minimum_archive_bytes = MODERATION_PANEL_NOTIFICATION_ARCHIVE_WRAPPER_MAX_BYTES_V1;
        if self.max_submit_attempts == 0
            || self.checkpoint_max_bytes == 0
            || self.checkpoint_max_bytes > MODERATION_ORCHESTRATOR_CHECKPOINT_MAX_BYTES_V1
            || self.panel_notification_archive_max_bytes < minimum_archive_bytes
        {
            return Err(ModerationOrchestratorError::InvalidConfiguration(
                "submission attempts, checkpoint bytes, or archive bytes are invalid".to_owned(),
            ));
        }
        if self.checkpoint_store_attestation_public_key == [0; 32]
            || PublicKey::from_bytes(
                Algorithm::Ed25519,
                &self.checkpoint_store_attestation_public_key,
            )
            .is_err()
        {
            return Err(ModerationOrchestratorError::InvalidConfiguration(
                "checkpoint terminal-set attestation key is invalid".to_owned(),
            ));
        }
        for (handle, qualification) in [
            (
                self.checkpoint_store_handle.as_str(),
                self.expected_checkpoint_store_qualification,
            ),
            (
                self.transaction_signer_handle.as_str(),
                self.expected_transaction_signer_qualification,
            ),
            (
                self.strict_ingress_handle.as_str(),
                self.expected_strict_ingress_qualification,
            ),
            (
                self.settlement_handoff_handle.as_str(),
                self.expected_settlement_handoff_qualification,
            ),
            (
                self.publication_handoff_handle.as_str(),
                self.expected_publication_handoff_qualification,
            ),
            (
                self.panel_notification_handle.as_str(),
                self.expected_panel_notification_qualification,
            ),
            (
                self.panel_notification_archive_handle.as_str(),
                self.expected_panel_notification_archive_qualification,
            ),
        ] {
            validate_moderation_runtime_provider_handle(handle, true).map_err(|_| {
                ModerationOrchestratorError::InvalidConfiguration(
                    "moderation runtime provider binding is invalid".to_owned(),
                )
            })?;
            if !qualification.is_valid() {
                return Err(ModerationOrchestratorError::InvalidConfiguration(
                    "moderation runtime provider binding is invalid".to_owned(),
                ));
            }
        }
        if self.panel_notification_archive_id == [0; 32]
            || self.panel_notification_archive_bootstrap_public_key == [0; 32]
            || self.panel_notification_archive_public_key == [0; 32]
            || PublicKey::from_bytes(
                Algorithm::Ed25519,
                &self.panel_notification_archive_bootstrap_public_key,
            )
            .is_err()
            || PublicKey::from_bytes(
                Algorithm::Ed25519,
                &self.panel_notification_archive_public_key,
            )
            .is_err()
        {
            return Err(ModerationOrchestratorError::InvalidConfiguration(
                "moderation receipt archive identity or public key is invalid".to_owned(),
            ));
        }
        if self.checkpoint_store_handle == self.panel_notification_archive_handle
            || self.checkpoint_store_attestation_public_key
                == self.panel_notification_archive_bootstrap_public_key
            || self.checkpoint_store_attestation_public_key
                == self.panel_notification_archive_public_key
        {
            return Err(ModerationOrchestratorError::InvalidConfiguration(
                "checkpoint attestor and immutable archive must be independently administered"
                    .to_owned(),
            ));
        }
        let rotation_fields = (
            self.panel_notification_archive_predecessor_revocation_generation,
            self.panel_notification_archive_predecessor_authorization_signature,
            self.panel_notification_archive_new_key_possession_signature,
        );
        let rotation_shape_is_valid = rotation_fields == (None, None, None)
            || (matches!(rotation_fields, (Some(_), Some(_), Some(_)))
                && rotation_fields.1 != Some([0; 64])
                && rotation_fields.2 != Some([0; 64]));
        if !rotation_shape_is_valid {
            return Err(ModerationOrchestratorError::InvalidConfiguration(
                "archive signer rotation requires exact predecessor cutoff, authorization, and new-key proof of possession"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}
/// Canonical request forwarded to the runtime-only HSM transaction service.
#[derive(Debug, Clone)]
pub struct ModerationTransactionRequestV1 {
    /// Exact genesis-derived network identity signed into the transaction.
    pub network_id: iroha_data_model::NetworkId,
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
    /// Construct one canonical authenticated submission request.
    ///
    /// Fails on invalid authority, action, binding, encoding, or finalized baseline.
    pub fn new(
        network_id: iroha_data_model::NetworkId,
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
        if envelope_generation == 0 {
            return Err(ModerationOrchestratorError::InvalidAction(
                "submission envelope generation must be non-zero".to_owned(),
            ));
        }
        let operation_id = action.operation_id(&network_id, &authority)?;
        let request = Self {
            network_id,
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
        if self.envelope_generation == 0 {
            return Err(ModerationOrchestratorError::InvalidAction(
                "submission envelope generation must be non-zero".to_owned(),
            ));
        }
        let operation_id = self
            .action
            .operation_id(&self.network_id, &self.authority)?;
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
    /// Return the exact external signer provider qualified by this submitter.
    fn transaction_signer_provider(&self) -> &dyn ModerationRuntimeProviderV1;
    /// Return the exact strict-ingress provider qualified by this submitter.
    fn strict_ingress_provider(&self) -> &dyn ModerationRuntimeProviderV1;
    /// Exact genesis-derived network identity implemented by this runtime boundary.
    fn network_id(&self) -> iroha_data_model::NetworkId;
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
    /// Stable sink-specific handoff identity bound to the exact genesis-derived network.
    pub handoff_id: [u8; 32],
    /// Exact genesis-derived network owning this handoff.
    pub network_id: iroha_data_model::NetworkId,
    /// Destination.
    pub kind: ModerationTerminalHandoffKindV1,
    /// Case identifier.
    pub case_id: String,
    /// Round identifier.
    pub round_id: String,
    /// Canonical digest of the authoritative terminal outcome.
    pub outcome_digest: [u8; 32],
    /// Consensus timestamp retained in the authoritative terminal outcome.
    pub outcome_finalized_at_unix_ms: u64,
    /// Exact finalized event that committed the terminal outcome.
    pub finalized_cursor: ModerationFinalizedEventCursorV1,
    /// Sealed minimal committed-event witness retained across bounded event-window eviction.
    pub source_event_witness: ModerationFinalizedEventV1,
}
impl ModerationTerminalHandoffV1 {
    /// Derive the canonical handoff identity from this record's exact network and scope.
    #[must_use]
    pub fn canonical_id(&self) -> [u8; 32] {
        terminal_handoff_id(
            &self.network_id,
            self.kind,
            &self.case_id,
            &self.round_id,
            self.outcome_digest,
        )
    }
    /// Return whether the handoff identity is canonical for `network_id`.
    #[must_use]
    pub fn is_bound_to_network(&self, network_id: &iroha_data_model::NetworkId) -> bool {
        self.network_id == *network_id && self.handoff_id == self.canonical_id()
    }
}
/// Exactly-once terminal settlement/publication adapter.
pub trait ModerationTerminalHandoffSinkV1: ModerationRuntimeProviderV1 {
    /// Deliver a payload-free finalized handoff, deduplicating `handoff_id`.
    fn deliver(
        &self,
        handoff: &ModerationTerminalHandoffV1,
    ) -> Result<(), ModerationHandoffFailureV1>;
    /// Publish or replay one exact signed notification-archive head.
    ///
    /// Publication implementations must enforce monotonic generation and chain
    /// commitment, atomically deduplicate `operation_id` against the canonical
    /// head bytes, and reject forks, gaps, or substituted bytes.
    fn publish_panel_notification_archive_head(
        &self,
        head: &ModerationPanelNotificationArchiveHeadV1,
    ) -> Result<(), ModerationHandoffFailureV1>;
    /// Read the exact publicly visible monotonic archive head.
    ///
    /// The returned value must come from the same authenticated durable store
    /// used by publication; process-local publication caches are invalid.
    fn read_panel_notification_archive_head(
        &self,
    ) -> Result<Option<ModerationPanelNotificationArchiveHeadV1>, ModerationHandoffFailureV1>;
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
    /// Exact genesis-derived network owning this notification.
    pub network_id: iroha_data_model::NetworkId,
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
impl ModerationPanelNotificationV1 {
    /// Derive the canonical notification identity from this record's exact network and scope.
    #[must_use]
    pub fn canonical_id(&self) -> [u8; 32] {
        panel_notification_id(
            &self.network_id,
            self.source_operation_id,
            self.scope_digest,
            self.kind,
            &self.recipient,
            self.finalized_event_cursor,
            self.source_occurred_at_unix_ms,
        )
    }
    /// Return whether the notification identity is canonical for `network_id`.
    #[must_use]
    pub fn is_bound_to_network(&self, network_id: &iroha_data_model::NetworkId) -> bool {
        self.network_id == *network_id && self.notification_id == self.canonical_id()
    }
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
/// Durable dead-letter class addressed by an externally authorized resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub enum ModerationDeadLetterKindV1 {
    /// A native moderation transaction submission.
    NativeSubmission,
    /// A settlement or publication terminal handoff.
    TerminalHandoff,
    /// A payload-free panel notification.
    PanelNotification,
}
impl ModerationDeadLetterKindV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::NativeSubmission => 0,
            Self::TerminalHandoff => 1,
            Self::PanelNotification => 2,
        }
    }
}
/// Authorized disposition for one exact unresolved dead letter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub enum ModerationDeadLetterResolutionActionV1 {
    /// Requeue the exact source-bound work under a fresh bounded attempt cycle.
    Redrive,
    /// Acknowledge the incident without requeueing it.
    Acknowledge,
}
impl ModerationDeadLetterResolutionActionV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::Redrive => 0,
            Self::Acknowledge => 1,
        }
    }
}
/// Externally signed, exact-source authorization resolving one dead letter.
///
/// The statement is bound to the current sealed checkpoint revision and the
/// exact target record digest. It therefore cannot be replayed after any state
/// transition or redirected to another incident. The same archive-lifetime
/// checkpoint trust anchor used for terminal-set attestations authorizes the
/// transition; private key material remains outside the process and checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationDeadLetterResolutionV1 {
    /// Resolution schema version.
    pub version: u16,
    /// Exact genesis-derived network identity.
    pub network_id: iroha_data_model::NetworkId,
    /// Exact sealed checkpoint namespace.
    pub checkpoint_namespace_digest: [u8; 32],
    /// Exact source checkpoint generation.
    pub checkpoint_generation: u64,
    /// Exact source checkpoint revision.
    pub checkpoint_revision: [u8; 32],
    /// Exact source checkpoint digest.
    pub checkpoint_digest: [u8; 32],
    /// Stable failed-work identity.
    pub identity: [u8; 32],
    /// Exact dead-letter class.
    pub kind: ModerationDeadLetterKindV1,
    /// Governed resolution disposition.
    pub action: ModerationDeadLetterResolutionActionV1,
    /// Digest of the exact unresolved source record.
    pub source_record_digest: [u8; 32],
    /// Nonzero operator authorization time retained for audit.
    pub authorized_at_unix_ms: u64,
    /// Stable checkpoint-attestor handle.
    pub attestor_handle: String,
    /// Exact checkpoint-attestor revision.
    pub attestor_revision: u64,
    /// Exact checkpoint-attestor public-policy digest.
    pub attestor_policy_digest: [u8; 32],
    /// Archive-lifetime-stable Ed25519 trust anchor.
    pub attestor_public_key: [u8; 32],
}
impl ModerationDeadLetterResolutionV1 {
    /// Derive the exact Ed25519 authorization message.
    ///
    /// # Errors
    ///
    /// Rejects inert, malformed, or noncanonical resolution coordinates.
    pub fn signing_message(&self) -> Result<[u8; 32], ModerationOrchestratorError> {
        validate_dead_letter_resolution_shape(self)?;
        Ok(dead_letter_resolution_message(self))
    }
}
/// Exactly-once payload-free panel-notification delivery adapter.
///
/// Implementations must atomically deduplicate
/// [`ModerationPanelNotificationV1::notification_id`] against the exact
/// canonical notification bytes before returning a receipt. A replay of the
/// same identity and bytes must return the same stable receipt; a replay with
/// different bytes must return [`ModerationPanelNotificationFailureV1::Permanent`].
pub trait ModerationPanelNotificationSinkV1: ModerationRuntimeProviderV1 {
    /// Deliver one exact claimed notification.
    ///
    /// The sink must not persist message bodies, evidence locators, bearer
    /// grants, or other private payloads. Recipient-facing content is resolved
    /// from the finalized ledger after the payload-free notification arrives.
    fn deliver(
        &self,
        claim: &ModerationPanelNotificationClaimV1,
    ) -> Result<ModerationPanelNotificationDeliveryReceiptV1, ModerationPanelNotificationFailureV1>;
}
/// Fixed payload-free failures returned by the immutable receipt archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModerationPanelNotificationArchiveExternalErrorV1 {
    /// The archive or its authenticated transport is unavailable.
    Unavailable,
    /// Installation may have committed and exact readback is required.
    Ambiguous,
    /// The archive rejected a substitution, stale predecessor, or policy violation.
    Rejected,
}
/// Canonical terminal-set statement signed by the sealed checkpoint authority.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationPanelNotificationSourceAttestationV1 {
    /// Archive schema version.
    pub version: u16,
    /// Exact checkpoint-attestor runtime-provider slot.
    pub attestor_slot: u16,
    /// Exact genesis-derived network containing the authoritative notification state.
    pub network_id: iroha_data_model::NetworkId,
    /// Network-bound sealed-checkpoint namespace.
    pub checkpoint_namespace_digest: [u8; 32],
    /// Exact sealed checkpoint generation.
    pub checkpoint_generation: u64,
    /// Exact sealed checkpoint record revision.
    pub checkpoint_revision: [u8; 32],
    /// Digest of the canonical source checkpoint bytes.
    pub checkpoint_digest: [u8; 32],
    /// Digest of the exact payload-minimal signer/predecessor source manifest.
    pub source_manifest_digest: [u8; 32],
    /// Digest of the exact canonical terminal archive payload.
    pub terminal_set_digest: [u8; 32],
    /// Number of terminal records in the attested set.
    pub terminal_record_count: u32,
    /// First terminal notification identity in canonical order.
    pub first_notification_id: [u8; 32],
    /// Last terminal notification identity in canonical order.
    pub last_notification_id: [u8; 32],
    /// Exact checkpoint-attestor provider handle.
    pub attestor_handle: String,
    /// Exact checkpoint-attestor adapter/public-policy revision.
    pub attestor_revision: u64,
    /// Exact checkpoint-attestor public-policy digest.
    pub attestor_policy_digest: [u8; 32],
    /// Ed25519 key authenticating the statement.
    pub attestor_public_key: [u8; 32],
}
impl ModerationPanelNotificationSourceAttestationV1 {
    /// Verify the canonical terminal-set statement and its Ed25519 signature.
    ///
    /// # Errors
    ///
    /// Rejects malformed source coordinates, substituted provider identity,
    /// or a signature that does not authenticate the exact statement.
    pub fn verify(&self, signature: [u8; 64]) -> Result<(), ModerationOrchestratorError> {
        validate_panel_notification_source_attestation(self)?;
        if signature == [0; 64] {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
        }
        let key = PublicKey::from_bytes(Algorithm::Ed25519, &self.attestor_public_key)
            .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
        let signature = IrohaSignature::try_from_bytes(&signature)
            .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
        signature
            .verify(&key, &panel_notification_source_attestation_message(self))
            .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)
    }
}
/// Authenticated exact readback from the immutable notification-receipt archive.
#[derive(Clone, PartialEq, Eq)]
pub struct ModerationPanelNotificationArchiveReadbackV1 {
    /// Exact canonical payload-free archive artifact.
    pub canonical_artifact: Vec<u8>,
    /// Ed25519 signature emitted only after durable installation.
    pub signature: [u8; 64],
}
impl fmt::Debug for ModerationPanelNotificationArchiveReadbackV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModerationPanelNotificationArchiveReadbackV1")
            .field("canonical_artifact", &"<payload-free-receipt-archive>")
            .field("canonical_artifact_len", &self.canonical_artifact.len())
            .field("signature", &"<ed25519-signature>")
            .finish()
    }
}
/// Deployment-owned immutable archive for terminal panel-notification receipts.
///
/// `install` must atomically bind an operation identifier to the exact receipt
/// message and canonical artifact. Exact replay is idempotent; any substituted
/// bytes or receipt message must be rejected. `read` returns only the exact
/// durable bytes and their provider-issued Ed25519 signature. Credentials and
/// private signing material remain behind this boundary.
pub trait ModerationPanelNotificationArchiveV1: ModerationRuntimeProviderV1 {
    /// Return the stable non-secret archive namespace identity.
    fn archive_id(&self) -> [u8; 32];
    /// Return the exact Ed25519 key authenticating durable readback.
    fn signing_public_key(&self) -> [u8; 32];
    /// Durably install one exact canonical archive artifact.
    fn install(
        &self,
        operation_id: [u8; 32],
        receipt_message: [u8; 32],
        canonical_artifact: &[u8],
    ) -> Result<[u8; 64], ModerationPanelNotificationArchiveExternalErrorV1>;
    /// Read back the exact artifact bound to `operation_id`.
    fn read(
        &self,
        operation_id: [u8; 32],
    ) -> Result<
        Option<ModerationPanelNotificationArchiveReadbackV1>,
        ModerationPanelNotificationArchiveExternalErrorV1,
    >;
}
/// One sealed archive-signer epoch with dual-control rotation evidence.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationPanelNotificationArchiveSignerEpochV1 {
    /// Archive schema version.
    pub version: u16,
    /// One-based signer epoch.
    pub epoch: u64,
    /// First archive generation this signer may authenticate.
    pub activated_at_generation: u64,
    /// Stable archive namespace retained across rotations.
    pub archive_id: [u8; 32],
    /// Provider handle qualified for this epoch.
    pub archive_handle: String,
    /// Provider revision qualified for this epoch.
    pub archive_revision: u64,
    /// Public-policy digest qualified for this epoch.
    pub archive_policy_digest: [u8; 32],
    /// Ed25519 signer public key for this epoch.
    pub archive_public_key: [u8; 32],
    /// Digest of the prior epoch, absent only at bootstrap.
    pub predecessor_epoch_digest: Option<[u8; 32]>,
    /// Inclusive last generation authorized for the predecessor.
    pub predecessor_revocation_generation: Option<u64>,
    /// Prior-key authorization of the transition, absent only at bootstrap.
    pub predecessor_authorization_signature: Option<[u8; 64]>,
    /// New-key proof of possession, absent only at bootstrap.
    pub new_key_possession_signature: Option<[u8; 64]>,
    /// Digest of every preceding field.
    pub epoch_digest: [u8; 32],
}
impl ModerationPanelNotificationArchiveSignerEpochV1 {
    /// Derive the exact predecessor-key authorization message for this transition.
    ///
    /// This method is safe to use before the two signatures and `epoch_digest`
    /// are populated: neither signature nor the self-digest is part of the
    /// authorization message. The returned digest is network-bound and commits
    /// the new provider binding, key, predecessor epoch, and inclusive
    /// predecessor revocation generation.
    ///
    /// # Errors
    ///
    /// Rejects bootstrap epochs and malformed, inert, or noncanonical rotation
    /// coordinates.
    pub fn rotation_authorization_message(
        &self,
        network_id: &iroha_data_model::NetworkId,
    ) -> Result<[u8; 32], ModerationOrchestratorError> {
        if self.version != MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1
            || self.epoch < 2
            || self.activated_at_generation == 0
            || self.archive_id == [0; 32]
            || validate_production_runtime_handle(&self.archive_handle).is_err()
            || !ModerationRuntimeProviderQualificationV1::new(
                self.archive_revision,
                self.archive_policy_digest,
            )
            .is_valid()
            || self.archive_public_key == [0; 32]
            || PublicKey::from_bytes(Algorithm::Ed25519, &self.archive_public_key).is_err()
            || self
                .predecessor_epoch_digest
                .is_none_or(|digest| digest == [0; 32])
            || self
                .predecessor_revocation_generation
                .and_then(|generation| generation.checked_add(1))
                != Some(self.activated_at_generation)
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
        }
        Ok(panel_notification_archive_signer_rotation_message(
            network_id, self,
        ))
    }
    /// Derive the new-key proof-of-possession message for this transition.
    ///
    /// # Errors
    ///
    /// Rejects the same malformed transition coordinates as
    /// [`Self::rotation_authorization_message`].
    pub fn new_key_possession_message(
        &self,
        network_id: &iroha_data_model::NetworkId,
    ) -> Result<[u8; 32], ModerationOrchestratorError> {
        self.rotation_authorization_message(network_id)
            .map(panel_notification_archive_signer_pop_message)
    }
}
/// Signed monotonic head of one immutable notification-receipt archive batch.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct ModerationPanelNotificationArchiveHeadV1 {
    /// Archive schema version.
    pub version: u16,
    /// Exact genesis-derived network whose terminal notifications are archived.
    pub network_id: iroha_data_model::NetworkId,
    /// Monotonic archive generation beginning at one.
    pub generation: u64,
    /// Exact predecessor head digest, absent only at generation one.
    pub predecessor_head_digest: Option<[u8; 32]>,
    /// Exact predecessor archive operation, absent only at generation one.
    pub predecessor_operation_id: Option<[u8; 32]>,
    /// Exact predecessor chain accumulator, absent only at generation one.
    pub predecessor_chain_commitment: Option<[u8; 32]>,
    /// Exact sealed checkpoint generation from which receipts were selected.
    pub source_checkpoint_generation: u64,
    /// Network-bound namespace of the authoritative sealed checkpoint.
    pub source_checkpoint_namespace_digest: [u8; 32],
    /// Exact sealed checkpoint revision from which receipts were selected.
    pub source_checkpoint_revision: [u8; 32],
    /// Exact sealed checkpoint digest from which receipts were selected.
    pub source_checkpoint_digest: [u8; 32],
    /// Digest of the payload-minimal source manifest carried by the artifact.
    pub source_manifest_digest: [u8; 32],
    /// Digest binding the network and exact sealed source checkpoint coordinates.
    pub source_binding_digest: [u8; 32],
    /// Exact qualified checkpoint authority that attested the terminal set.
    pub source_attestor_handle: String,
    /// Exact checkpoint-attestor adapter/public-policy revision.
    pub source_attestor_revision: u64,
    /// Exact checkpoint-attestor public-policy digest.
    pub source_attestor_policy_digest: [u8; 32],
    /// Ed25519 public key authenticating the terminal-set source attestation.
    pub source_attestor_public_key: [u8; 32],
    /// Deterministic terminal-set source-attestation message.
    pub source_attestation_digest: [u8; 32],
    /// Independently administered checkpoint-authority signature.
    pub source_attestation_signature: [u8; 64],
    /// Number of delivered or dead-lettered terminal records in this batch.
    pub terminal_record_count: u32,
    /// Number of terminal dead letters in this exact batch.
    pub dead_letter_record_count: u32,
    /// Permanent cumulative dead letters through this archive generation.
    pub cumulative_dead_letter_count: u64,
    /// First notification identity in canonical archive order.
    pub first_notification_id: [u8; 32],
    /// Last notification identity in canonical archive order.
    pub last_notification_id: [u8; 32],
    /// Digest of the exact canonical archived receipt records.
    pub payload_digest: [u8; 32],
    /// Stable authenticated archive-provider handle.
    pub archive_handle: String,
    /// Exact archive adapter/public-policy revision.
    pub archive_revision: u64,
    /// Exact archive public-policy digest.
    pub archive_policy_digest: [u8; 32],
    /// Stable non-secret archive namespace identity.
    pub archive_id: [u8; 32],
    /// Exact Ed25519 key authenticating durable readback.
    pub archive_public_key: [u8; 32],
    /// One-based sealed signer epoch authenticating this generation.
    pub archive_signer_epoch: u64,
    /// Exact digest of the corresponding sealed signer-epoch record.
    pub archive_signer_epoch_digest: [u8; 32],
    /// Stable deterministic archive operation identifier.
    pub operation_id: [u8; 32],
    /// Content-addressed signed archive-chain head.
    pub head_digest: [u8; 32],
    /// Forward chain accumulator committing every archive generation.
    pub chain_commitment: [u8; 32],
    /// Archive signature emitted only after durable exact installation.
    pub archive_signature: [u8; 64],
}
impl ModerationPanelNotificationArchiveHeadV1 {
    /// Verify the deterministic head and provider-issued readback signature.
    ///
    /// # Errors
    ///
    /// Rejects malformed generations, identities, digests, or signatures.
    pub fn verify(
        &self,
        expected_handle: &str,
        expected_qualification: ModerationRuntimeProviderQualificationV1,
        expected_archive_id: [u8; 32],
        expected_public_key: [u8; 32],
    ) -> Result<(), ModerationOrchestratorError> {
        verify_panel_notification_archive_head_is_current(
            self,
            expected_handle,
            expected_qualification,
            expected_archive_id,
            expected_public_key,
        )
    }
}
/// Result of one bounded authenticated archive-history audit page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModerationPanelNotificationArchiveAuditProgressV1 {
    /// Archive heads authenticated in this page.
    pub verified_heads: u32,
    /// Generation targeted by the current complete-history sweep.
    pub target_generation: u64,
    /// Latest generation for which a complete generation-one-to-head sweep finished.
    pub last_completed_generation: u64,
    /// Whether the current sweep reached the generation-one root.
    pub cycle_complete: bool,
}
/// Derived coordinates returned after strict slot-55 archive broker validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModerationPanelNotificationArchiveBrokerValidationV1 {
    /// Deterministic archive operation derived from the canonical artifact.
    pub operation_id: [u8; 32],
    /// Exact message the independently administered archive signer may sign.
    pub receipt_message: [u8; 32],
    /// Exact epoch-authenticated signer key for this artifact generation.
    ///
    /// Install validation pins this to the current provider binding. Historical
    /// readback validation derives it from the bootstrap-anchored signer log.
    pub archive_public_key: [u8; 32],
    /// Content-addressed archive head.
    pub head_digest: [u8; 32],
    /// Monotonic all-generation archive-chain accumulator.
    pub chain_commitment: [u8; 32],
    /// One-based archive generation.
    pub generation: u64,
    /// Checkpoint-authority message already authenticated inside the artifact.
    pub source_attestation_digest: [u8; 32],
}
/// Stable public expectations used by the slot-55 archive broker validator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationPanelNotificationArchiveBrokerExpectationV1<'a> {
    /// Exact genesis-derived network accepted by this deployment.
    pub network_id: &'a iroha_data_model::NetworkId,
    /// Qualified archive provider handle.
    pub archive_handle: &'a str,
    /// Qualified archive provider revision and public-policy digest.
    pub archive_qualification: ModerationRuntimeProviderQualificationV1,
    /// Stable archive namespace identity.
    pub archive_id: [u8; 32],
    /// Bootstrap signer anchoring the sealed archive epoch log.
    pub archive_bootstrap_public_key: [u8; 32],
    /// Current archive signing public key.
    pub archive_public_key: [u8; 32],
    /// Qualified sealed-checkpoint provider handle.
    pub checkpoint_handle: &'a str,
    /// Qualified checkpoint provider revision and public-policy digest.
    pub checkpoint_qualification: ModerationRuntimeProviderQualificationV1,
    /// Current checkpoint terminal-source attestation key.
    pub checkpoint_attestation_public_key: [u8; 32],
    /// Maximum canonical source-checkpoint bytes.
    pub checkpoint_max_bytes: u64,
    /// Maximum canonical archive artifact bytes.
    pub archive_max_bytes: u64,
    /// Maximum terminal records in one artifact.
    pub max_records: usize,
}
/// Deterministic non-production fixture for cross-crate broker protocol tests.
///
/// This type is public only so the irohad broker can test the real canonical
/// derivation code instead of duplicating private cryptographic domains.
#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct ModerationPanelNotificationArchiveBrokerFixtureV1 {
    /// Exact fixture network.
    pub network_id: iroha_data_model::NetworkId,
    /// Qualified archive provider handle.
    pub archive_handle: String,
    /// Qualified archive provider revision and policy.
    pub archive_qualification: ModerationRuntimeProviderQualificationV1,
    /// Stable archive namespace.
    pub archive_id: [u8; 32],
    /// Bootstrap/current archive public key.
    pub archive_public_key: [u8; 32],
    /// Deterministic test-only archive signing seed.
    pub archive_signing_seed: [u8; 32],
    /// Qualified checkpoint provider handle.
    pub checkpoint_handle: String,
    /// Qualified checkpoint provider revision and policy.
    pub checkpoint_qualification: ModerationRuntimeProviderQualificationV1,
    /// Checkpoint terminal-source attestation public key.
    pub checkpoint_attestation_public_key: [u8; 32],
    /// Deterministic test-only checkpoint signing seed.
    pub checkpoint_attestation_signing_seed: [u8; 32],
    /// Exact current sealed checkpoint record for op115.
    pub current_checkpoint_record: ModerationCheckpointStoreRecordV1,
    /// Exact typed op115 statement.
    pub source_attestation: ModerationPanelNotificationSourceAttestationV1,
    /// Canonical unsigned archive artifact for install op113.
    pub canonical_artifact: Vec<u8>,
    /// Archive signature returned by a successful install.
    pub archive_signature: [u8; 64],
    /// Canonical signed head bytes for slot-20 `ModerationPublicationHandoff` op116.
    pub canonical_signed_head: Vec<u8>,
    /// Expected derived operation and digest coordinates.
    pub validation: ModerationPanelNotificationArchiveBrokerValidationV1,
    /// Source-checkpoint byte bound.
    pub checkpoint_max_bytes: u64,
    /// Archive artifact byte bound.
    pub archive_max_bytes: u64,
}
impl ModerationPanelNotificationArchiveBrokerFixtureV1 {
    /// Borrow this fixture as strict broker validation expectations.
    #[must_use]
    pub fn expectation(&self) -> ModerationPanelNotificationArchiveBrokerExpectationV1<'_> {
        ModerationPanelNotificationArchiveBrokerExpectationV1 {
            network_id: &self.network_id,
            archive_handle: &self.archive_handle,
            archive_qualification: self.archive_qualification,
            archive_id: self.archive_id,
            archive_bootstrap_public_key: self.archive_public_key,
            archive_public_key: self.archive_public_key,
            checkpoint_handle: &self.checkpoint_handle,
            checkpoint_qualification: self.checkpoint_qualification,
            checkpoint_attestation_public_key: self.checkpoint_attestation_public_key,
            checkpoint_max_bytes: self.checkpoint_max_bytes,
            archive_max_bytes: self.archive_max_bytes,
            max_records: 8,
        }
    }
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
    /// Deployment-owned sealed, predecessor-bound monotonic checkpoint authority.
    pub checkpoint_store: Arc<dyn ModerationCheckpointStoreV1>,
    /// HSM transaction submitter.
    pub submitter: Arc<dyn ModerationTransactionSubmitterV1>,
    /// Finalized ledger snapshot reader.
    pub snapshot_reader: Arc<dyn ModerationFinalizedSnapshotReaderV1>,
    /// Appeal-finance terminal sink.
    pub settlement_sink: Arc<dyn ModerationTerminalHandoffSinkV1>,
    /// Governance/transparency terminal sink.
    pub publication_sink: Arc<dyn ModerationTerminalHandoffSinkV1>,
    /// Durable payload-free panel-notification sink.
    pub panel_notification_sink: Arc<dyn ModerationPanelNotificationSinkV1>,
    /// Immutable authenticated panel-notification receipt archive.
    pub panel_notification_archive: Arc<dyn ModerationPanelNotificationArchiveV1>,
}
impl fmt::Debug for ModerationOrchestratorDepsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ModerationOrchestratorDepsV1")
            .field("checkpoint_store", &"<runtime-only>")
            .field("submitter", &"<runtime-only>")
            .field("snapshot_reader", &"<runtime-only>")
            .field("settlement_sink", &"<runtime-only>")
            .field("publication_sink", &"<runtime-only>")
            .field("panel_notification_sink", &"<runtime-only>")
            .field("panel_notification_archive", &"<runtime-only>")
            .finish()
    }
}
struct QualifiedModerationTransactionSubmitterV1 {
    transaction_signer_handle: String,
    transaction_signer_qualification: ModerationRuntimeProviderQualificationV1,
    strict_ingress_handle: String,
    strict_ingress_qualification: ModerationRuntimeProviderQualificationV1,
    submitter: Arc<dyn ModerationTransactionSubmitterV1>,
}
impl QualifiedModerationTransactionSubmitterV1 {
    fn try_new(
        config: &ModerationOrchestratorConfigV1,
        submitter: Arc<dyn ModerationTransactionSubmitterV1>,
    ) -> Result<Self, ModerationRuntimeProviderQualificationErrorV1> {
        qualify_moderation_runtime_provider_v1(
            &config.transaction_signer_handle,
            config.expected_transaction_signer_qualification,
            submitter.transaction_signer_provider(),
        )?;
        qualify_moderation_runtime_provider_v1(
            &config.strict_ingress_handle,
            config.expected_strict_ingress_qualification,
            submitter.strict_ingress_provider(),
        )?;
        Ok(Self {
            transaction_signer_handle: config.transaction_signer_handle.clone(),
            transaction_signer_qualification: config.expected_transaction_signer_qualification,
            strict_ingress_handle: config.strict_ingress_handle.clone(),
            strict_ingress_qualification: config.expected_strict_ingress_qualification,
            submitter,
        })
    }
    fn revalidate_transaction_signer(
        &self,
    ) -> Result<(), ModerationRuntimeProviderQualificationErrorV1> {
        revalidate_moderation_runtime_provider_v1(
            &self.transaction_signer_handle,
            self.transaction_signer_qualification,
            self.submitter.transaction_signer_provider(),
        )
    }
    fn revalidate_strict_ingress(
        &self,
    ) -> Result<(), ModerationRuntimeProviderQualificationErrorV1> {
        revalidate_moderation_runtime_provider_v1(
            &self.strict_ingress_handle,
            self.strict_ingress_qualification,
            self.submitter.strict_ingress_provider(),
        )
    }
    fn network_id(
        &self,
    ) -> Result<iroha_data_model::NetworkId, ModerationRuntimeProviderQualificationErrorV1> {
        self.revalidate_transaction_signer()?;
        self.revalidate_strict_ingress()?;
        let network_id = self.submitter.network_id();
        self.revalidate_transaction_signer()?;
        self.revalidate_strict_ingress()?;
        Ok(network_id)
    }
    fn sign(
        &self,
        request: &ModerationTransactionRequestV1,
    ) -> Result<ModerationSignedTransactionV1, ModerationSubmissionFailureV1> {
        self.revalidate_transaction_signer()
            .map_err(|_| ModerationSubmissionFailureV1::RuntimeUnavailable)?;
        let result = self.submitter.sign(request);
        self.revalidate_transaction_signer()
            .map_err(|_| ModerationSubmissionFailureV1::RuntimeUnavailable)?;
        result
    }
    fn submit_signed(
        &self,
        request: &ModerationTransactionRequestV1,
        signed: &ModerationSignedTransactionV1,
    ) -> Result<ModerationTransactionReceiptV1, ModerationSubmissionFailureV1> {
        self.revalidate_strict_ingress()
            .map_err(|_| ModerationSubmissionFailureV1::NotSubmittedUnavailable)?;
        let result = self.submitter.submit_signed(request, signed);
        self.revalidate_strict_ingress()
            .map_err(|_| ModerationSubmissionFailureV1::Ambiguous)?;
        result
    }
    fn lookup(
        &self,
        operation_id: [u8; 32],
        transaction_id: Option<[u8; 32]>,
    ) -> ModerationSubmissionLookupV1 {
        if self.revalidate_strict_ingress().is_err() {
            return ModerationSubmissionLookupV1::Unknown;
        }
        let lookup = self.submitter.lookup(operation_id, transaction_id);
        if self.revalidate_strict_ingress().is_err() {
            return ModerationSubmissionLookupV1::Unknown;
        }
        lookup
    }
}
impl fmt::Debug for QualifiedModerationTransactionSubmitterV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedModerationTransactionSubmitterV1")
            .field("transaction_signer_handle", &self.transaction_signer_handle)
            .field(
                "transaction_signer_qualification",
                &self.transaction_signer_qualification,
            )
            .field("strict_ingress_handle", &self.strict_ingress_handle)
            .field(
                "strict_ingress_qualification",
                &self.strict_ingress_qualification,
            )
            .field("submitter", &"<runtime-only>")
            .finish()
    }
}
struct QualifiedModerationTerminalHandoffSinkV1 {
    handle: String,
    qualification: ModerationRuntimeProviderQualificationV1,
    sink: Arc<dyn ModerationTerminalHandoffSinkV1>,
}
impl QualifiedModerationTerminalHandoffSinkV1 {
    fn try_new(
        expected_handle: &str,
        expected_qualification: ModerationRuntimeProviderQualificationV1,
        sink: Arc<dyn ModerationTerminalHandoffSinkV1>,
    ) -> Result<Self, ModerationRuntimeProviderQualificationErrorV1> {
        qualify_moderation_runtime_provider_v1(
            expected_handle,
            expected_qualification,
            sink.as_ref(),
        )?;
        Ok(Self {
            handle: expected_handle.to_owned(),
            qualification: expected_qualification,
            sink,
        })
    }
    fn revalidate(&self) -> Result<(), ModerationRuntimeProviderQualificationErrorV1> {
        revalidate_moderation_runtime_provider_v1(
            &self.handle,
            self.qualification,
            self.sink.as_ref(),
        )
    }
    fn deliver(
        &self,
        handoff: &ModerationTerminalHandoffV1,
    ) -> Result<(), ModerationHandoffFailureV1> {
        self.revalidate()
            .map_err(|_| ModerationHandoffFailureV1::NotDelivered)?;
        let result = self.sink.deliver(handoff);
        self.revalidate()
            .map_err(|_| ModerationHandoffFailureV1::Ambiguous)?;
        result
    }
    fn publish_panel_notification_archive_head(
        &self,
        head: &ModerationPanelNotificationArchiveHeadV1,
    ) -> Result<(), ModerationHandoffFailureV1> {
        self.revalidate()
            .map_err(|_| ModerationHandoffFailureV1::NotDelivered)?;
        let result = self.sink.publish_panel_notification_archive_head(head);
        self.revalidate()
            .map_err(|_| ModerationHandoffFailureV1::Ambiguous)?;
        result
    }
    fn read_panel_notification_archive_head(
        &self,
    ) -> Result<Option<ModerationPanelNotificationArchiveHeadV1>, ModerationHandoffFailureV1> {
        self.revalidate()
            .map_err(|_| ModerationHandoffFailureV1::NotDelivered)?;
        let result = self.sink.read_panel_notification_archive_head();
        self.revalidate()
            .map_err(|_| ModerationHandoffFailureV1::Ambiguous)?;
        result
    }
}
impl fmt::Debug for QualifiedModerationTerminalHandoffSinkV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedModerationTerminalHandoffSinkV1")
            .field("handle", &self.handle)
            .field("qualification", &self.qualification)
            .field("sink", &"<runtime-only>")
            .finish()
    }
}
struct QualifiedModerationPanelNotificationSinkV1 {
    handle: String,
    qualification: ModerationRuntimeProviderQualificationV1,
    sink: Arc<dyn ModerationPanelNotificationSinkV1>,
}
impl QualifiedModerationPanelNotificationSinkV1 {
    fn try_new(
        expected_handle: &str,
        expected_qualification: ModerationRuntimeProviderQualificationV1,
        sink: Arc<dyn ModerationPanelNotificationSinkV1>,
    ) -> Result<Self, ModerationRuntimeProviderQualificationErrorV1> {
        qualify_moderation_runtime_provider_v1(
            expected_handle,
            expected_qualification,
            sink.as_ref(),
        )?;
        Ok(Self {
            handle: expected_handle.to_owned(),
            qualification: expected_qualification,
            sink,
        })
    }
    fn revalidate(&self) -> Result<(), ModerationRuntimeProviderQualificationErrorV1> {
        revalidate_moderation_runtime_provider_v1(
            &self.handle,
            self.qualification,
            self.sink.as_ref(),
        )
    }
    fn deliver(
        &self,
        claim: &ModerationPanelNotificationClaimV1,
    ) -> Result<ModerationPanelNotificationDeliveryReceiptV1, ModerationPanelNotificationFailureV1>
    {
        self.revalidate()
            .map_err(|_| ModerationPanelNotificationFailureV1::NotDelivered)?;
        let result = self.sink.deliver(claim);
        self.revalidate()
            .map_err(|_| ModerationPanelNotificationFailureV1::Ambiguous)?;
        result
    }
}
impl fmt::Debug for QualifiedModerationPanelNotificationSinkV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedModerationPanelNotificationSinkV1")
            .field("handle", &self.handle)
            .field("qualification", &self.qualification)
            .field("sink", &"<runtime-only>")
            .finish()
    }
}
struct QualifiedModerationPanelNotificationArchiveV1 {
    handle: String,
    qualification: ModerationRuntimeProviderQualificationV1,
    archive_id: [u8; 32],
    public_key: [u8; 32],
    archive: Arc<dyn ModerationPanelNotificationArchiveV1>,
}
impl QualifiedModerationPanelNotificationArchiveV1 {
    fn try_new(
        expected_handle: &str,
        expected_qualification: ModerationRuntimeProviderQualificationV1,
        expected_archive_id: [u8; 32],
        expected_public_key: [u8; 32],
        archive: Arc<dyn ModerationPanelNotificationArchiveV1>,
    ) -> Result<Self, ModerationRuntimeProviderQualificationErrorV1> {
        qualify_moderation_runtime_provider_v1(
            expected_handle,
            expected_qualification,
            archive.as_ref(),
        )?;
        let identity = Self::read_qualified_identity(
            expected_handle,
            expected_qualification,
            archive.as_ref(),
        )?;
        if identity.0 != expected_archive_id {
            return Err(ModerationRuntimeProviderQualificationErrorV1::ArchiveIdentityChanged);
        }
        if identity.1 != expected_public_key {
            return Err(ModerationRuntimeProviderQualificationErrorV1::ArchivePublicKeyChanged);
        }
        Ok(Self {
            handle: expected_handle.to_owned(),
            qualification: expected_qualification,
            archive_id: expected_archive_id,
            public_key: expected_public_key,
            archive,
        })
    }
    fn read_qualified_identity(
        handle: &str,
        qualification: ModerationRuntimeProviderQualificationV1,
        archive: &dyn ModerationPanelNotificationArchiveV1,
    ) -> Result<([u8; 32], [u8; 32]), ModerationRuntimeProviderQualificationErrorV1> {
        revalidate_moderation_runtime_provider_v1(handle, qualification, archive)?;
        let identity = (archive.archive_id(), archive.signing_public_key());
        revalidate_moderation_runtime_provider_v1(handle, qualification, archive)?;
        Ok(identity)
    }
    fn revalidate_identity(&self) -> Result<(), ModerationPanelNotificationArchiveExternalErrorV1> {
        let identity =
            Self::read_qualified_identity(&self.handle, self.qualification, self.archive.as_ref())
                .map_err(|_| ModerationPanelNotificationArchiveExternalErrorV1::Unavailable)?;
        if identity != (self.archive_id, self.public_key) {
            return Err(ModerationPanelNotificationArchiveExternalErrorV1::Unavailable);
        }
        Ok(())
    }
    fn install(
        &self,
        operation_id: [u8; 32],
        receipt_message: [u8; 32],
        canonical_artifact: &[u8],
    ) -> Result<[u8; 64], ModerationPanelNotificationArchiveExternalErrorV1> {
        self.revalidate_identity()?;
        let result = self
            .archive
            .install(operation_id, receipt_message, canonical_artifact);
        self.revalidate_identity()
            .map_err(|_| ModerationPanelNotificationArchiveExternalErrorV1::Ambiguous)?;
        result
    }
    fn read(
        &self,
        operation_id: [u8; 32],
    ) -> Result<
        Option<ModerationPanelNotificationArchiveReadbackV1>,
        ModerationPanelNotificationArchiveExternalErrorV1,
    > {
        self.revalidate_identity()?;
        let result = self.archive.read(operation_id);
        self.revalidate_identity()
            .map_err(|_| ModerationPanelNotificationArchiveExternalErrorV1::Ambiguous)?;
        result
    }
}
impl fmt::Debug for QualifiedModerationPanelNotificationArchiveV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedModerationPanelNotificationArchiveV1")
            .field("handle", &self.handle)
            .field("qualification", &self.qualification)
            .field("archive_id", &self.archive_id)
            .field("public_key", &self.public_key)
            .field("archive", &"<runtime-only>")
            .finish()
    }
}
struct QualifiedModerationOrchestratorDepsV1 {
    checkpoint_store: checkpoint_store::QualifiedModerationCheckpointStoreV1,
    submitter: QualifiedModerationTransactionSubmitterV1,
    snapshot_reader: Arc<dyn ModerationFinalizedSnapshotReaderV1>,
    settlement_sink: QualifiedModerationTerminalHandoffSinkV1,
    publication_sink: QualifiedModerationTerminalHandoffSinkV1,
    panel_notification_sink: QualifiedModerationPanelNotificationSinkV1,
    panel_notification_archive: QualifiedModerationPanelNotificationArchiveV1,
}
impl QualifiedModerationOrchestratorDepsV1 {
    fn try_new(
        config: &ModerationOrchestratorConfigV1,
        deps: ModerationOrchestratorDepsV1,
    ) -> Result<Self, ModerationRuntimeProviderQualificationErrorV1> {
        let ModerationOrchestratorDepsV1 {
            checkpoint_store,
            submitter,
            snapshot_reader,
            settlement_sink,
            publication_sink,
            panel_notification_sink,
            panel_notification_archive,
        } = deps;
        let checkpoint_store = checkpoint_store::QualifiedModerationCheckpointStoreV1::try_new(
            &config.checkpoint_store_handle,
            config.expected_checkpoint_store_qualification,
            config.checkpoint_store_attestation_public_key,
            checkpoint_store,
        )?;
        let submitter = QualifiedModerationTransactionSubmitterV1::try_new(config, submitter)?;
        let settlement_sink = QualifiedModerationTerminalHandoffSinkV1::try_new(
            &config.settlement_handoff_handle,
            config.expected_settlement_handoff_qualification,
            settlement_sink,
        )?;
        let publication_sink = QualifiedModerationTerminalHandoffSinkV1::try_new(
            &config.publication_handoff_handle,
            config.expected_publication_handoff_qualification,
            publication_sink,
        )?;
        let panel_notification_sink = QualifiedModerationPanelNotificationSinkV1::try_new(
            &config.panel_notification_handle,
            config.expected_panel_notification_qualification,
            panel_notification_sink,
        )?;
        let panel_notification_archive = QualifiedModerationPanelNotificationArchiveV1::try_new(
            &config.panel_notification_archive_handle,
            config.expected_panel_notification_archive_qualification,
            config.panel_notification_archive_id,
            config.panel_notification_archive_public_key,
            panel_notification_archive,
        )?;
        Ok(Self {
            checkpoint_store,
            submitter,
            snapshot_reader,
            settlement_sink,
            publication_sink,
            panel_notification_sink,
            panel_notification_archive,
        })
    }
}
impl fmt::Debug for QualifiedModerationOrchestratorDepsV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedModerationOrchestratorDepsV1")
            .field("checkpoint_store", &self.checkpoint_store)
            .field("submitter", &self.submitter)
            .field("snapshot_reader", &"<local-committed-state-view>")
            .field("settlement_sink", &self.settlement_sink)
            .field("publication_sink", &self.publication_sink)
            .field("panel_notification_sink", &self.panel_notification_sink)
            .field(
                "panel_notification_archive",
                &self.panel_notification_archive,
            )
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
/// Payload-free durable health observed after one moderation worker pass.
///
/// This report contains only bounded queue counts and the finalized anchor. It
/// is safe to use for readiness decisions and never contacts an external
/// signer, ingress, reader, or handoff destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModerationOrchestratorDurableHealthV1 {
    /// Finalized anchor retained in the durable projection, when initialized.
    pub finalized_cursor: Option<ModerationFinalizedCursorV1>,
    /// Native submissions still awaiting a finalized terminal effect.
    pub pending_submissions: usize,
    /// Settlement/publication handoffs still awaiting a durable sink result.
    pub pending_handoffs: usize,
    /// Panel notifications not yet delivered or terminally dead-lettered.
    pub pending_panel_notifications: usize,
    /// Unresolved native-submission or terminal-handoff dead letters.
    pub durable_dead_letters: usize,
    /// Unresolved panel-notification dead letters.
    ///
    /// Signed resolution history remains immutable in the archive, but only an
    /// active unresolved incident blocks projection readiness.
    pub panel_notification_dead_letters: usize,
    /// Latest immutable archive generation retained in the sealed checkpoint.
    pub panel_notification_archive_generation: u64,
    /// Latest archive generation durably published as the public monotonic head.
    pub panel_notification_archive_published_generation: u64,
    /// Latest archive generation covered by an authenticated incremental audit suffix.
    pub panel_notification_archive_audited_generation: u64,
}
impl ModerationOrchestratorDurableHealthV1 {
    /// Return whether durable work has reached a release-blocking terminal failure.
    #[must_use]
    pub const fn has_dead_letters(self) -> bool {
        self.durable_dead_letters != 0 || self.panel_notification_dead_letters != 0
    }
    /// Return whether publication and the incremental authenticated audit cover the current head.
    #[must_use]
    pub const fn archive_is_fresh(self) -> bool {
        self.panel_notification_archive_generation
            == self.panel_notification_archive_published_generation
            && self.panel_notification_archive_generation
                == self.panel_notification_archive_audited_generation
    }
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
    incident_sequence: u64,
    identity: [u8; 32],
    action_label: String,
    reason: StoredDeadLetterReasonV1,
    finalized_cursor: ModerationFinalizedCursorV1,
    dead_lettered_at_unix_ms: u64,
    redrive: Option<StoredDeadLetterRedriveV1>,
    resolution: Option<ModerationDeadLetterResolutionV1>,
    resolution_signature: Option<[u8; 64]>,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum StoredDeadLetterRedriveV1 {
    NativeSubmission {
        authority: AccountId,
        action: ModerationNativeActionV1,
        request_binding_digest: [u8; 32],
    },
    TerminalHandoff(ModerationTerminalHandoffV1),
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredHandoffV1 {
    handoff: ModerationTerminalHandoffV1,
    attempts: u32,
    work_generation: u32,
    work_claim: Option<StoredExternalWorkClaimV1>,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredCompletedHandoffV1 {
    handoff: ModerationTerminalHandoffV1,
    completed_at_finalized_cursor: ModerationFinalizedCursorV1,
    record_digest: [u8; 32],
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
enum ModerationPanelNotificationArchiveTerminalStatusV1 {
    Delivered {
        receipt_digest: [u8; 32],
        delivered_at_unix_ms: u64,
    },
    DeadLettered {
        reason: ModerationPanelNotificationDeadLetterReasonV1,
        dead_lettered_at_unix_ms: u64,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationPanelNotificationArchiveRecordV1 {
    notification_id: [u8; 32],
    terminal_status: ModerationPanelNotificationArchiveTerminalStatusV1,
    source_record_digest: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct StoredPanelNotificationDeadLetterResolutionV1 {
    terminal_record: ModerationPanelNotificationArchiveRecordV1,
    resolution: ModerationDeadLetterResolutionV1,
    resolution_signature: [u8; 64],
    record_digest: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum ModerationTerminalArchiveRecordV1 {
    PanelNotification(ModerationPanelNotificationArchiveRecordV1),
    ResolvedPanelDeadLetter {
        terminal_record: ModerationPanelNotificationArchiveRecordV1,
        resolution: ModerationDeadLetterResolutionV1,
        resolution_signature: [u8; 64],
        source_record_digest: [u8; 32],
    },
    NativeOperation {
        operation_id: [u8; 32],
        status: StoredOperationStatusV1,
        transaction_id: Option<[u8; 32]>,
        source_record_digest: [u8; 32],
    },
    DurableDeadLetter {
        incident_sequence: u64,
        identity: [u8; 32],
        reason: StoredDeadLetterReasonV1,
        finalized_cursor: ModerationFinalizedCursorV1,
        dead_lettered_at_unix_ms: u64,
        resolution: ModerationDeadLetterResolutionV1,
        resolution_signature: [u8; 64],
        operation_source_record_digest: Option<[u8; 32]>,
        handoff_kind: Option<ModerationTerminalHandoffKindV1>,
        handoff_outcome_digest: Option<[u8; 32]>,
        handoff_finalized_cursor: Option<ModerationFinalizedEventCursorV1>,
        source_record_digest: [u8; 32],
    },
    CompletedHandoff {
        handoff_id: [u8; 32],
        kind: ModerationTerminalHandoffKindV1,
        outcome_digest: [u8; 32],
        finalized_cursor: ModerationFinalizedEventCursorV1,
        completed_at_finalized_cursor: ModerationFinalizedCursorV1,
        source_record_digest: [u8; 32],
    },
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationPanelNotificationArchivePayloadV1 {
    version: u16,
    records: Vec<ModerationTerminalArchiveRecordV1>,
}
/// Payload-minimal witness for archive-signer and predecessor validation.
///
/// The checkpoint authority separately verifies terminal membership before it
/// signs the source attestation. This manifest therefore carries no finalized
/// snapshot, moderation scopes, authorities, native actions, outbox entries, or
/// other checkpoint payloads.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationPanelNotificationArchiveSourceManifestV1 {
    version: u16,
    network_id: iroha_data_model::NetworkId,
    checkpoint_namespace_digest: [u8; 32],
    checkpoint_generation: u64,
    checkpoint_revision: [u8; 32],
    checkpoint_digest: [u8; 32],
    archive_signer_epochs: Vec<ModerationPanelNotificationArchiveSignerEpochV1>,
    predecessor_archive_head: Option<ModerationPanelNotificationArchiveHeadV1>,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationPanelNotificationArchiveArtifactV1 {
    version: u16,
    head: ModerationPanelNotificationArchiveHeadV1,
    source_manifest: ModerationPanelNotificationArchiveSourceManifestV1,
    payload: ModerationPanelNotificationArchivePayloadV1,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationPanelNotificationArchiveAuditCursorV1 {
    version: u16,
    target_generation: u64,
    target_head_digest: [u8; 32],
    next_operation_id: Option<[u8; 32]>,
    expected_generation: Option<u64>,
    expected_head_digest: Option<[u8; 32]>,
    expected_chain_commitment: Option<[u8; 32]>,
    verified_head_count: u64,
    chain_commitment: [u8; 32],
    last_completed_generation: u64,
    last_completed_head_digest: Option<[u8; 32]>,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ModerationOrchestratorCheckpointV1 {
    version: u16,
    network_id: iroha_data_model::NetworkId,
    generation: u64,
    panel_notification_clock_unix_ms: u64,
    panel_notification_scanned_cursor: Option<ModerationFinalizedEventCursorV1>,
    terminal_handoff_scanned_cursor: Option<ModerationFinalizedEventCursorV1>,
    panel_notification_outbox_digest: [u8; 32],
    panel_notification_archived_dead_letter_count: u64,
    terminal_handoff_archived_cursor: Option<ModerationFinalizedEventCursorV1>,
    panel_notification_archive_compaction_reservation:
        Option<ModerationPanelNotificationArchivePayloadV1>,
    panel_notification_archive_signer_epochs: Vec<ModerationPanelNotificationArchiveSignerEpochV1>,
    panel_notification_archive_head: Option<ModerationPanelNotificationArchiveHeadV1>,
    panel_notification_archive_pending_publication:
        Option<ModerationPanelNotificationArchiveHeadV1>,
    panel_notification_archive_published_head: Option<ModerationPanelNotificationArchiveHeadV1>,
    panel_notification_archive_audit_cursor:
        Option<ModerationPanelNotificationArchiveAuditCursorV1>,
    finalized_snapshot: Option<ModerationFinalizedLedgerSnapshotV1>,
    finalized_snapshot_digest: Option<[u8; 32]>,
    operations: Vec<StoredOperationV1>,
    outbox: Vec<StoredOutboxEntryV1>,
    dead_letters: Vec<StoredDeadLetterV1>,
    dead_letter_incident_sequence: u64,
    pending_handoffs: Vec<StoredHandoffV1>,
    completed_handoffs: Vec<StoredCompletedHandoffV1>,
    panel_notifications: Vec<StoredPanelNotificationV1>,
    panel_notification_dead_letter_resolutions: Vec<StoredPanelNotificationDeadLetterResolutionV1>,
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
/// Finalized-chain moderation orchestrator.
pub struct ModerationOrchestratorV1 {
    config: ModerationOrchestratorConfigV1,
    network_id: iroha_data_model::NetworkId,
    deps: QualifiedModerationOrchestratorDepsV1,
    state: Mutex<ModerationOrchestratorCheckpointV1>,
    checkpoint_record: Mutex<ModerationCheckpointStoreRecordV1>,
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
    /// Open an orchestrator from an authoritative sealed checkpoint and private bounded cache.
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
        // Every external provider is independently qualified before even the
        // checkpoint parent is inspected. A disabled, substituted, stale, or
        // test-marked boundary therefore cannot influence durable state.
        let deps = QualifiedModerationOrchestratorDepsV1::try_new(&config, deps)
            .map_err(map_runtime_provider_qualification_error)?;
        let network_id = deps
            .submitter
            .network_id()
            .map_err(map_runtime_provider_qualification_error)?;
        let (mut state, mut checkpoint_record) = checkpoint_store::open_authoritative_checkpoint(
            &config,
            &network_id,
            &deps.checkpoint_store,
        )?;
        let signer_epochs_changed =
            reconcile_panel_notification_archive_signer_epochs(&config, &network_id, &mut state)?;
        verify_current_panel_notification_archive_readback(
            &config,
            &network_id,
            &deps.panel_notification_archive,
            state.panel_notification_archive_head.as_ref(),
        )?;
        verify_published_panel_notification_archive_head_readback(
            &deps.publication_sink,
            state.panel_notification_archive_published_head.as_ref(),
        )?;
        let expired_work_recovered = recover_external_work_after_restart(&mut state)?;
        if signer_epochs_changed || expired_work_recovered {
            checkpoint_store::persist_authoritative_checkpoint(
                &config,
                &network_id,
                &deps.checkpoint_store,
                &mut checkpoint_record,
                &mut state,
            )?;
        }
        Ok(Self {
            config,
            network_id,
            deps,
            state: Mutex::new(state),
            checkpoint_record: Mutex::new(checkpoint_record),
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
        let operation_id = action.operation_id(&self.network_id, &authority)?;
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
    /// Return payload-free durable queue health after authenticating the public
    /// archive-head readback.
    ///
    /// # Errors
    ///
    /// Fails when the durable state is fail-stopped or its mutex is poisoned.
    pub fn durable_health(
        &self,
    ) -> Result<ModerationOrchestratorDurableHealthV1, ModerationOrchestratorError> {
        let state = self.lock_state()?;
        let pending_panel_notifications = state
            .panel_notifications
            .iter()
            .filter(|entry| {
                matches!(
                    entry.state,
                    StoredPanelNotificationStateV1::Pending
                        | StoredPanelNotificationStateV1::Claimed
                )
            })
            .count();
        let panel_notification_dead_letters = state
            .panel_notifications
            .iter()
            .filter(|entry| entry.state == StoredPanelNotificationStateV1::DeadLetter)
            .count();
        let published_head = state.panel_notification_archive_published_head.clone();
        let health = ModerationOrchestratorDurableHealthV1 {
            finalized_cursor: state
                .finalized_snapshot
                .as_ref()
                .map(ModerationFinalizedLedgerSnapshotV1::anchor),
            pending_submissions: state.outbox.len(),
            pending_handoffs: state.pending_handoffs.len(),
            pending_panel_notifications,
            durable_dead_letters: state
                .dead_letters
                .iter()
                .filter(|entry| entry.resolution.is_none())
                .count(),
            panel_notification_dead_letters,
            panel_notification_archive_generation: state
                .panel_notification_archive_head
                .as_ref()
                .map_or(0, |head| head.generation),
            panel_notification_archive_published_generation: state
                .panel_notification_archive_published_head
                .as_ref()
                .map_or(0, |head| head.generation),
            panel_notification_archive_audited_generation: state
                .panel_notification_archive_audit_cursor
                .as_ref()
                .map_or(0, |cursor| cursor.last_completed_generation),
        };
        drop(state);
        verify_published_panel_notification_archive_head_readback(
            &self.deps.publication_sink,
            published_head.as_ref(),
        )?;
        Ok(health)
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
    /// Deliver a bounded batch of due panel notifications through the
    /// independently qualified durable sink.
    ///
    /// Claims are checkpointed before the sink is called. The sink must
    /// deduplicate the stable notification identity, so a crash after the
    /// downstream effect but before receipt persistence replays the same
    /// payload-free notification safely. The worker identity is derived from
    /// the exact network and governed sink binding; no process-local randomness
    /// or secret material participates.
    ///
    /// # Errors
    ///
    /// Fails for an invalid runtime timestamp, checkpoint failure, corrupt
    /// provider receipt, stale lease, or provider qualification drift.
    pub fn deliver_due_panel_notifications(
        &self,
        now_unix_ms: u64,
        limit: usize,
    ) -> Result<usize, ModerationOrchestratorError> {
        if now_unix_ms == 0 {
            return Err(ModerationOrchestratorError::InvalidPanelNotificationClaim);
        }
        if limit == 0 {
            return Ok(0);
        }
        let worker_id = panel_notification_worker_id(
            &self.network_id,
            &self.config.panel_notification_handle,
            self.config.expected_panel_notification_qualification,
        );
        let claims = self.claim_panel_notifications(worker_id, now_unix_ms, limit)?;
        let mut delivered = 0_usize;
        for claim in claims {
            match self.deps.panel_notification_sink.deliver(&claim) {
                Ok(receipt)
                    if receipt.notification_id == claim.notification.notification_id
                        && receipt.receipt_digest != [0; 32]
                        && receipt.delivered_at_unix_ms
                            >= claim.notification.source_occurred_at_unix_ms
                        && receipt.delivered_at_unix_ms < claim.lease_expires_at_unix_ms =>
                {
                    let completion_unix_ms = now_unix_ms.max(receipt.delivered_at_unix_ms);
                    if completion_unix_ms >= claim.lease_expires_at_unix_ms {
                        self.release_panel_notification_claim(
                            claim.notification.notification_id,
                            claim.worker_id,
                            claim.lease_token,
                            ModerationPanelNotificationFailureV1::Ambiguous,
                            now_unix_ms,
                        )?;
                        continue;
                    }
                    self.finalize_panel_notification_delivery(
                        claim.worker_id,
                        claim.lease_token,
                        receipt,
                        completion_unix_ms,
                    )?;
                    delivered = delivered
                        .checked_add(1)
                        .ok_or(ModerationOrchestratorError::GenerationOverflow)?;
                }
                Ok(_) => {
                    self.release_panel_notification_claim(
                        claim.notification.notification_id,
                        claim.worker_id,
                        claim.lease_token,
                        ModerationPanelNotificationFailureV1::Ambiguous,
                        now_unix_ms,
                    )?;
                }
                Err(failure) => {
                    self.release_panel_notification_claim(
                        claim.notification.notification_id,
                        claim.worker_id,
                        claim.lease_token,
                        failure,
                        now_unix_ms,
                    )?;
                }
            }
        }
        Ok(delivered)
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
    /// Prepare an exact current-checkpoint authorization statement for one
    /// unresolved durable dead letter.
    ///
    /// The returned statement is unsigned. An independently administered HSM
    /// holding the configured checkpoint-attestor key must sign
    /// [`ModerationDeadLetterResolutionV1::signing_message`] before
    /// [`Self::apply_dead_letter_resolution`] accepts it.
    ///
    /// # Errors
    ///
    /// Fails if the identity is not an unresolved dead letter of the requested
    /// kind, the action cannot be redriven, or sealed source coordinates drift.
    pub fn prepare_dead_letter_resolution(
        &self,
        identity: [u8; 32],
        kind: ModerationDeadLetterKindV1,
        action: ModerationDeadLetterResolutionActionV1,
        authorized_at_unix_ms: u64,
    ) -> Result<ModerationDeadLetterResolutionV1, ModerationOrchestratorError> {
        if identity == [0; 32] || authorized_at_unix_ms == 0 {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
        }
        let state = self.lock_state()?;
        let source_record_digest = unresolved_dead_letter_record_digest(&state, identity, kind)?;
        let incident_time = unresolved_dead_letter_incident_time(&state, identity, kind)?;
        let finalized_time = state
            .finalized_snapshot
            .as_ref()
            .ok_or(ModerationOrchestratorError::FinalizedReaderUnavailable)?
            .finalized_at_unix_ms;
        if authorized_at_unix_ms < incident_time || authorized_at_unix_ms > finalized_time {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveRejected);
        }
        if action == ModerationDeadLetterResolutionActionV1::Redrive
            && !dead_letter_redrive_is_available(&state, identity, kind)
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveRejected);
        }
        let current_record = self
            .checkpoint_record
            .lock()
            .map_err(|_| ModerationOrchestratorError::CheckpointStoreLockPoisoned)?
            .clone();
        if current_record.checkpoint_generation != state.generation
            || current_record.namespace_digest
                != checkpoint_store::checkpoint_namespace(&self.network_id)
        {
            return Err(ModerationOrchestratorError::CheckpointStoreEquivocation);
        }
        Ok(ModerationDeadLetterResolutionV1 {
            version: 1,
            network_id: self.network_id,
            checkpoint_namespace_digest: current_record.namespace_digest,
            checkpoint_generation: current_record.checkpoint_generation,
            checkpoint_revision: current_record.revision,
            checkpoint_digest: current_record.checkpoint_digest,
            identity,
            kind,
            action,
            source_record_digest,
            authorized_at_unix_ms,
            attestor_handle: self.config.checkpoint_store_handle.clone(),
            attestor_revision: self
                .config
                .expected_checkpoint_store_qualification
                .revision(),
            attestor_policy_digest: self
                .config
                .expected_checkpoint_store_qualification
                .policy_digest(),
            attestor_public_key: self.config.checkpoint_store_attestation_public_key,
        })
    }
    /// Apply one externally signed, source-bound dead-letter resolution.
    ///
    /// Resolution never erases the incident: durable incidents retain the
    /// signed receipt until archive compaction, while panel incidents first
    /// move an exact terminal record and signed receipt into resolution
    /// history. Health stops counting only after this sealed transition.
    ///
    /// # Errors
    ///
    /// Rejects invalid signatures, stale source checkpoints, target
    /// substitution, duplicate resolution, unsafe redrive, or durability loss.
    pub fn apply_dead_letter_resolution(
        &self,
        resolution: ModerationDeadLetterResolutionV1,
        signature: [u8; 64],
    ) -> Result<(), ModerationOrchestratorError> {
        verify_dead_letter_resolution_signature(&resolution, signature)?;
        if resolution.network_id != self.network_id
            || resolution.checkpoint_namespace_digest
                != checkpoint_store::checkpoint_namespace(&self.network_id)
            || resolution.attestor_handle != self.config.checkpoint_store_handle
            || resolution.attestor_revision
                != self
                    .config
                    .expected_checkpoint_store_qualification
                    .revision()
            || resolution.attestor_policy_digest
                != self
                    .config
                    .expected_checkpoint_store_qualification
                    .policy_digest()
            || resolution.attestor_public_key != self.config.checkpoint_store_attestation_public_key
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
        }
        let mut state = self.lock_state()?;
        let current_record = self
            .checkpoint_record
            .lock()
            .map_err(|_| ModerationOrchestratorError::CheckpointStoreLockPoisoned)?
            .clone();
        if resolution.checkpoint_generation != current_record.checkpoint_generation
            || resolution.checkpoint_revision != current_record.revision
            || resolution.checkpoint_digest != current_record.checkpoint_digest
            || current_record.checkpoint_generation != state.generation
            || unresolved_dead_letter_record_digest(&state, resolution.identity, resolution.kind)?
                != resolution.source_record_digest
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveRejected);
        }
        let incident_time =
            unresolved_dead_letter_incident_time(&state, resolution.identity, resolution.kind)?;
        let finalized_time = state
            .finalized_snapshot
            .as_ref()
            .ok_or(ModerationOrchestratorError::FinalizedReaderUnavailable)?
            .finalized_at_unix_ms;
        if resolution.authorized_at_unix_ms < incident_time
            || resolution.authorized_at_unix_ms > finalized_time
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveRejected);
        }
        match resolution.kind {
            ModerationDeadLetterKindV1::PanelNotification => {
                if state
                    .panel_notifications
                    .len()
                    .saturating_add(state.panel_notification_dead_letter_resolutions.len())
                    >= self.config.max_handoffs
                {
                    return Err(ModerationOrchestratorError::ResourceExhausted {
                        resource: "panel notification resolution history",
                        limit: self.config.max_handoffs,
                    });
                }
                let position = state
                    .panel_notifications
                    .iter()
                    .position(|entry| {
                        entry.notification.notification_id == resolution.identity
                            && entry.state == StoredPanelNotificationStateV1::DeadLetter
                    })
                    .ok_or(ModerationOrchestratorError::PanelNotificationArchiveRejected)?;
                let terminal_record = panel_notification_archive_record_from_stored(
                    &state.panel_notifications[position],
                )?;
                let record_digest = panel_notification_resolution_record_digest(
                    &terminal_record,
                    &resolution,
                    signature,
                )?;
                state.panel_notification_dead_letter_resolutions.push(
                    StoredPanelNotificationDeadLetterResolutionV1 {
                        terminal_record,
                        resolution: resolution.clone(),
                        resolution_signature: signature,
                        record_digest,
                    },
                );
                match resolution.action {
                    ModerationDeadLetterResolutionActionV1::Acknowledge => {
                        state.panel_notifications.remove(position);
                    }
                    ModerationDeadLetterResolutionActionV1::Redrive => {
                        let entry = &mut state.panel_notifications[position];
                        entry.attempts = 0;
                        entry.claim_generation = 0;
                        entry.available_at_unix_ms = resolution
                            .authorized_at_unix_ms
                            .max(entry.notification.source_occurred_at_unix_ms);
                        entry.state = StoredPanelNotificationStateV1::Pending;
                        entry.claimed_by = None;
                        entry.lease_token = None;
                        entry.claimed_at_unix_ms = None;
                        entry.lease_expires_at_unix_ms = None;
                        entry.receipt_digest = None;
                        entry.delivered_at_unix_ms = None;
                        entry.dead_letter_reason = None;
                        entry.dead_lettered_at_unix_ms = None;
                        refresh_panel_notification_record_digest(entry);
                    }
                }
            }
            ModerationDeadLetterKindV1::NativeSubmission
            | ModerationDeadLetterKindV1::TerminalHandoff => {
                let position = state
                    .dead_letters
                    .iter()
                    .position(|entry| {
                        entry.identity == resolution.identity && entry.resolution.is_none()
                    })
                    .ok_or(ModerationOrchestratorError::PanelNotificationArchiveRejected)?;
                let redrive = state.dead_letters[position].redrive.clone();
                if resolution.action == ModerationDeadLetterResolutionActionV1::Redrive {
                    match (resolution.kind, redrive) {
                        (
                            ModerationDeadLetterKindV1::NativeSubmission,
                            Some(StoredDeadLetterRedriveV1::NativeSubmission {
                                authority,
                                action,
                                request_binding_digest,
                            }),
                        ) => {
                            ensure_outbox_capacity(&state, &self.config)?;
                            let operation = state
                                .operations
                                .iter_mut()
                                .find(|entry| entry.operation_id == resolution.identity)
                                .ok_or(
                                    ModerationOrchestratorError::PanelNotificationArchiveRejected,
                                )?;
                            if operation.status != StoredOperationStatusV1::Rejected
                                || operation.authority != authority
                                || operation.action_digest != action.action_digest()?
                            {
                                return Err(
                                    ModerationOrchestratorError::PanelNotificationArchiveRejected,
                                );
                            }
                            operation.status = StoredOperationStatusV1::Pending;
                            operation.transaction_id = None;
                            state.outbox.push(StoredOutboxEntryV1 {
                                operation_id: resolution.identity,
                                authority,
                                action: action.clone(),
                                action_digest: action.action_digest()?,
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
                        }
                        (
                            ModerationDeadLetterKindV1::TerminalHandoff,
                            Some(StoredDeadLetterRedriveV1::TerminalHandoff(handoff)),
                        ) => {
                            if state.pending_handoffs.len().saturating_add(1)
                                > self.config.max_handoffs
                                || state
                                    .pending_handoffs
                                    .iter()
                                    .any(|entry| entry.handoff.handoff_id == resolution.identity)
                            {
                                return Err(ModerationOrchestratorError::ResourceExhausted {
                                    resource: "terminal handoffs",
                                    limit: self.config.max_handoffs,
                                });
                            }
                            state.pending_handoffs.push(StoredHandoffV1 {
                                handoff,
                                attempts: 0,
                                work_generation: 0,
                                work_claim: None,
                            });
                        }
                        _ => {
                            return Err(
                                ModerationOrchestratorError::PanelNotificationArchiveRejected,
                            );
                        }
                    }
                }
                state.dead_letters[position].resolution = Some(resolution);
                state.dead_letters[position].resolution_signature = Some(signature);
            }
        }
        self.persist_checkpoint_locked(&mut state)
    }
    /// Archive and prune one bounded canonical batch of terminal moderation records.
    ///
    /// The immutable archive is installed and read back under its exact
    /// provider-issued Ed25519 signature before any checkpoint record is
    /// removed. The batch is bound to the current sealed checkpoint revision
    /// and predecessor archive head. The sealed checkpoint CAS is the sole
    /// cross-replica commit fence; there is no process-local fallback.
    ///
    /// Eligible records are delivered notifications, finalized native-operation
    /// tombstones, successful handoff receipts, and externally signed resolved
    /// dead letters. Active unresolved failures are never pruned.
    /// `Ok(None)` means no terminal record is currently eligible.
    ///
    /// # Errors
    ///
    /// Rejects zero or excessive bounds, provider drift, corrupt/noncanonical
    /// readback, signature or predecessor substitution, checkpoint rollback,
    /// concurrent conflicting compaction, and uncertain durability.
    pub fn compact_panel_notification_receipts(
        &self,
        maximum_records: u32,
    ) -> Result<Option<ModerationPanelNotificationArchiveHeadV1>, ModerationOrchestratorError> {
        let maximum_records = usize::try_from(maximum_records).map_err(|_| {
            ModerationOrchestratorError::ResourceExhausted {
                resource: "panel notification archive batch",
                limit: self.config.max_handoffs,
            }
        })?;
        if maximum_records == 0 || maximum_records > self.config.max_handoffs {
            return Err(ModerationOrchestratorError::ResourceExhausted {
                resource: "panel notification archive batch",
                limit: self.config.max_handoffs,
            });
        }
        let mut state = self.lock_state()?;
        if state
            .panel_notification_archive_pending_publication
            .is_some()
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveRejected);
        }
        let predecessor_head = state.panel_notification_archive_head.clone();
        let signer_epoch = state
            .panel_notification_archive_signer_epochs
            .last()
            .cloned()
            .ok_or(ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
        if signer_epoch.archive_handle != self.config.panel_notification_archive_handle
            || signer_epoch.archive_revision
                != self
                    .config
                    .expected_panel_notification_archive_qualification
                    .revision()
            || signer_epoch.archive_policy_digest
                != self
                    .config
                    .expected_panel_notification_archive_qualification
                    .policy_digest()
            || signer_epoch.archive_id != self.config.panel_notification_archive_id
            || signer_epoch.archive_public_key != self.config.panel_notification_archive_public_key
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
        }
        let archive_max_bytes = usize::try_from(self.config.panel_notification_archive_max_bytes)
            .map_err(|_| {
            ModerationOrchestratorError::InvalidConfiguration(
                "notification archive byte limit does not fit usize".to_owned(),
            )
        })?;
        let available_records = collect_terminal_archive_records(&state)?;
        let records = if let Some(reservation) = state
            .panel_notification_archive_compaction_reservation
            .as_ref()
        {
            if reservation.version != MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1
                || reservation.records.is_empty()
                || available_records.len() < reservation.records.len()
                || available_records[..reservation.records.len()] != reservation.records
                || safe_terminal_archive_prefix_len(&available_records, reservation.records.len())?
                    != reservation.records.len()
            {
                return Err(ModerationOrchestratorError::CheckpointCorrupt(
                    "terminal archive reservation is stale or noncanonical".to_owned(),
                ));
            }
            reservation.records.clone()
        } else {
            if available_records.is_empty() {
                return Ok(None);
            }
            let selected = safe_terminal_archive_prefix_len(&available_records, maximum_records)?;
            if selected == 0 {
                return Err(ModerationOrchestratorError::ResourceExhausted {
                    resource: "atomic terminal archive group",
                    limit: maximum_records,
                });
            }
            let mut selected_records = available_records[..selected].to_vec();
            loop {
                let payload = ModerationPanelNotificationArchivePayloadV1 {
                    version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
                    records: selected_records.clone(),
                };
                let payload_bytes = norito::to_bytes(&payload).map_err(|error| {
                    ModerationOrchestratorError::CheckpointCorrupt(format!(
                        "encode terminal archive reservation payload: {error}"
                    ))
                })?;
                if payload_bytes
                    .len()
                    .checked_add(
                        usize::try_from(MODERATION_PANEL_NOTIFICATION_ARCHIVE_WRAPPER_MAX_BYTES_V1)
                            .unwrap_or(usize::MAX),
                    )
                    .is_some_and(|total| total <= archive_max_bytes)
                {
                    state.panel_notification_archive_compaction_reservation = Some(payload);
                    self.persist_checkpoint_locked(&mut state)?;
                    break;
                }
                let next = safe_terminal_archive_prefix_len(
                    &selected_records,
                    selected_records.len() / 2,
                )?;
                if next == 0 {
                    return Err(ModerationOrchestratorError::ResourceExhausted {
                        resource: "panel notification archive bytes",
                        limit: archive_max_bytes,
                    });
                }
                selected_records.truncate(next);
            }
            selected_records
        };
        let source_record = self
            .checkpoint_record
            .lock()
            .map_err(|_| ModerationOrchestratorError::CheckpointStoreLockPoisoned)?
            .clone();
        let source_checkpoint_bytes = norito::to_bytes(&*state).map_err(|error| {
            ModerationOrchestratorError::CheckpointCorrupt(format!(
                "encode receipt archive source checkpoint: {error}"
            ))
        })?;
        if source_record.checkpoint_generation != state.generation
            || source_record.checkpoint_bytes != source_checkpoint_bytes
            || source_record.checkpoint_digest
                != domain_hash(
                    b"sorafs.moderation.checkpoint-bytes.v1",
                    &[source_checkpoint_bytes.as_slice()],
                )
        {
            return Err(ModerationOrchestratorError::CheckpointStoreEquivocation);
        }
        let source_manifest = ModerationPanelNotificationArchiveSourceManifestV1 {
            version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
            network_id: self.network_id,
            checkpoint_namespace_digest: source_record.namespace_digest,
            checkpoint_generation: source_record.checkpoint_generation,
            checkpoint_revision: source_record.revision,
            checkpoint_digest: source_record.checkpoint_digest,
            archive_signer_epochs: state.panel_notification_archive_signer_epochs.clone(),
            predecessor_archive_head: predecessor_head.clone(),
        };
        let source_manifest_digest =
            panel_notification_archive_source_manifest_digest(&source_manifest)?;
        let (
            generation,
            predecessor_head_digest,
            predecessor_operation_id,
            predecessor_chain_commitment,
        ) = if let Some(head) = predecessor_head.as_ref() {
            (
                head.generation
                    .checked_add(1)
                    .ok_or(ModerationOrchestratorError::GenerationOverflow)?,
                Some(head.head_digest),
                Some(head.operation_id),
                Some(head.chain_commitment),
            )
        } else {
            (1, None, None, None)
        };
        let archive = &self.deps.panel_notification_archive;
        drop(state);
        let build_artifact = |selected: &[ModerationTerminalArchiveRecordV1],
                              source_attestation_signature: [u8; 64]|
         -> Result<
            ModerationPanelNotificationArchiveArtifactV1,
            ModerationOrchestratorError,
        > {
            let payload = ModerationPanelNotificationArchivePayloadV1 {
                version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
                records: selected.to_vec(),
            };
            let terminal_record_count = u32::try_from(payload.records.len()).map_err(|_| {
                ModerationOrchestratorError::ResourceExhausted {
                    resource: "panel notification archive batch",
                    limit: self.config.max_handoffs,
                }
            })?;
            let dead_letter_record_count = u32::try_from(
                payload
                    .records
                    .iter()
                    .filter(|record| terminal_archive_record_is_dead_letter(record))
                    .count(),
            )
            .map_err(|_| ModerationOrchestratorError::ResourceExhausted {
                resource: "panel notification archive dead letters",
                limit: self.config.max_handoffs,
            })?;
            let cumulative_dead_letter_count = predecessor_head
                .as_ref()
                .map_or(0, |head| head.cumulative_dead_letter_count)
                .checked_add(u64::from(dead_letter_record_count))
                .ok_or(ModerationOrchestratorError::GenerationOverflow)?;
            let first_notification_id = payload
                .records
                .first()
                .map(terminal_archive_record_boundary_id)
                .transpose()?
                .ok_or_else(|| {
                    ModerationOrchestratorError::CheckpointCorrupt(
                        "receipt archive batch unexpectedly became empty".to_owned(),
                    )
                })?;
            let last_notification_id = payload
                .records
                .last()
                .map(terminal_archive_record_boundary_id)
                .transpose()?
                .ok_or_else(|| {
                    ModerationOrchestratorError::CheckpointCorrupt(
                        "receipt archive batch unexpectedly became empty".to_owned(),
                    )
                })?;
            let payload_digest = panel_notification_archive_payload_digest(&payload)?;
            let source_attestation = ModerationPanelNotificationSourceAttestationV1 {
                version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
                attestor_slot: MODERATION_PANEL_NOTIFICATION_SOURCE_ATTESTOR_BROKER_SLOT_V1,
                network_id: self.network_id,
                checkpoint_namespace_digest: source_record.namespace_digest,
                checkpoint_generation: source_record.checkpoint_generation,
                checkpoint_revision: source_record.revision,
                checkpoint_digest: source_record.checkpoint_digest,
                source_manifest_digest,
                terminal_set_digest: payload_digest,
                terminal_record_count,
                first_notification_id,
                last_notification_id,
                attestor_handle: source_record.checkpoint_store_handle.clone(),
                attestor_revision: source_record.checkpoint_store_revision,
                attestor_policy_digest: source_record.checkpoint_store_policy_digest,
                attestor_public_key: self.config.checkpoint_store_attestation_public_key,
            };
            let mut head = ModerationPanelNotificationArchiveHeadV1 {
                version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
                network_id: self.network_id,
                generation,
                predecessor_head_digest,
                predecessor_operation_id,
                predecessor_chain_commitment,
                source_checkpoint_generation: source_record.checkpoint_generation,
                source_checkpoint_namespace_digest: source_record.namespace_digest,
                source_checkpoint_revision: source_record.revision,
                source_checkpoint_digest: source_record.checkpoint_digest,
                source_manifest_digest,
                source_binding_digest: panel_notification_archive_source_binding_digest(
                    &source_attestation,
                ),
                source_attestor_handle: source_record.checkpoint_store_handle.clone(),
                source_attestor_revision: source_record.checkpoint_store_revision,
                source_attestor_policy_digest: source_record.checkpoint_store_policy_digest,
                source_attestor_public_key: self.config.checkpoint_store_attestation_public_key,
                source_attestation_digest: panel_notification_source_attestation_message(
                    &source_attestation,
                ),
                source_attestation_signature,
                terminal_record_count,
                dead_letter_record_count,
                cumulative_dead_letter_count,
                first_notification_id,
                last_notification_id,
                payload_digest,
                archive_handle: archive.handle.clone(),
                archive_revision: archive.qualification.revision(),
                archive_policy_digest: archive.qualification.policy_digest(),
                archive_id: archive.archive_id,
                archive_public_key: archive.public_key,
                archive_signer_epoch: signer_epoch.epoch,
                archive_signer_epoch_digest: signer_epoch.epoch_digest,
                operation_id: [0; 32],
                head_digest: [0; 32],
                chain_commitment: [0; 32],
                archive_signature: [0; 64],
            };
            head.operation_id = panel_notification_archive_operation_id(&head);
            head.head_digest = panel_notification_archive_head_digest(&head);
            head.chain_commitment = panel_notification_archive_chain_commitment(&head);
            Ok(ModerationPanelNotificationArchiveArtifactV1 {
                version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
                head,
                source_manifest: source_manifest.clone(),
                payload,
            })
        };
        let candidate = build_artifact(&records, [1; 64])?;
        if norito::to_bytes(&candidate)
            .map_err(|error| {
                ModerationOrchestratorError::CheckpointCorrupt(format!(
                    "encode panel notification receipt archive candidate: {error}"
                ))
            })?
            .len()
            > archive_max_bytes
        {
            return Err(ModerationOrchestratorError::ResourceExhausted {
                resource: "panel notification archive bytes",
                limit: archive_max_bytes,
            });
        }
        let source_attestation = panel_notification_source_attestation_from_head(&candidate.head);
        let source_attestation_signature = self
            .deps
            .checkpoint_store
            .attest_terminal_set(&source_attestation)
            .map_err(map_checkpoint_store_attestation_error)?;
        let artifact = build_artifact(&records, source_attestation_signature)?;
        let mut head = artifact.head.clone();
        verify_panel_notification_archive_head_core_is_current(
            &head,
            &self.config.panel_notification_archive_handle,
            self.config
                .expected_panel_notification_archive_qualification,
            self.config.panel_notification_archive_id,
            self.config.panel_notification_archive_public_key,
        )?;
        if let Some(predecessor) = predecessor_head.as_ref() {
            verify_panel_notification_archive_lineage_link(&head, predecessor)?;
        }
        let artifact_bytes = norito::to_bytes(&artifact).map_err(|error| {
            ModerationOrchestratorError::CheckpointCorrupt(format!(
                "encode panel notification receipt archive: {error}"
            ))
        })?;
        // Runtime archive I/O must never execute while the orchestrator state
        // mutex is held. The exact sealed source checkpoint is compared again
        // after authenticated readback and before pruning.
        verify_current_panel_notification_archive_readback(
            &self.config,
            &self.network_id,
            &self.deps.panel_notification_archive,
            predecessor_head.as_ref(),
        )?;
        let verified = verify_panel_notification_archive_artifact(
            &self.config,
            &self.network_id,
            &artifact_bytes,
        )?;
        if verified != artifact {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
        }
        let install_result = archive.install(
            head.operation_id,
            panel_notification_archive_receipt_message(&head),
            &artifact_bytes,
        );
        let readback = match archive.read(head.operation_id) {
            Ok(Some(readback)) => readback,
            Ok(None) => {
                return Err(install_result.err().map_or(
                    ModerationOrchestratorError::PanelNotificationArchiveUnavailable,
                    map_panel_notification_archive_error,
                ));
            }
            Err(error) => return Err(map_panel_notification_archive_error(error)),
        };
        if let Ok(install_signature) = install_result
            && install_signature != readback.signature
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
        }
        let (installed, installed_head) =
            verify_panel_notification_archive_readback(&self.config, &self.network_id, &readback)?;
        if installed != artifact {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
        }
        head = installed_head;
        verify_panel_notification_archive_head_is_current(
            &head,
            &archive.handle,
            archive.qualification,
            archive.archive_id,
            archive.public_key,
        )?;
        if let Some(predecessor) = predecessor_head.as_ref() {
            verify_panel_notification_archive_lineage_link(&head, predecessor)?;
        }
        let mut state = self.lock_state()?;
        let current_checkpoint_bytes = norito::to_bytes(&*state).map_err(|error| {
            ModerationOrchestratorError::CheckpointCorrupt(format!(
                "encode receipt archive commit checkpoint: {error}"
            ))
        })?;
        let current_record = self
            .checkpoint_record
            .lock()
            .map_err(|_| ModerationOrchestratorError::CheckpointStoreLockPoisoned)?
            .clone();
        if state.panel_notification_archive_head != predecessor_head
            || current_checkpoint_bytes != source_checkpoint_bytes
            || current_record != source_record
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveRejected);
        }
        let mut candidate_state = state.clone();
        let mut archived_terminal_groups =
            BTreeMap::<[u8; 32], Vec<ModerationFinalizedEventCursorV1>>::new();
        for record in &artifact.payload.records {
            match record {
                ModerationTerminalArchiveRecordV1::PanelNotification(archived) => {
                    let position = candidate_state
                        .panel_notifications
                        .iter()
                        .position(|entry| {
                            entry.notification.notification_id == archived.notification_id
                                && entry.state == StoredPanelNotificationStateV1::Delivered
                        })
                        .ok_or(ModerationOrchestratorError::CheckpointStoreEquivocation)?;
                    validate_archived_panel_notification_record(
                        archived,
                        &candidate_state.panel_notifications[position],
                    )
                    .map_err(|_| ModerationOrchestratorError::CheckpointStoreEquivocation)?;
                    candidate_state.panel_notifications.remove(position);
                }
                ModerationTerminalArchiveRecordV1::ResolvedPanelDeadLetter {
                    source_record_digest,
                    ..
                } => {
                    let position = candidate_state
                        .panel_notification_dead_letter_resolutions
                        .iter()
                        .position(|entry| entry.record_digest == *source_record_digest)
                        .ok_or(ModerationOrchestratorError::CheckpointStoreEquivocation)?;
                    candidate_state
                        .panel_notification_dead_letter_resolutions
                        .remove(position);
                }
                ModerationTerminalArchiveRecordV1::NativeOperation {
                    operation_id,
                    source_record_digest,
                    ..
                } => {
                    let position = candidate_state
                        .operations
                        .iter()
                        .position(|entry| {
                            entry.operation_id == *operation_id
                                && native_operation_record_digest(entry).ok()
                                    == Some(*source_record_digest)
                        })
                        .ok_or(ModerationOrchestratorError::CheckpointStoreEquivocation)?;
                    candidate_state.operations.remove(position);
                }
                ModerationTerminalArchiveRecordV1::DurableDeadLetter {
                    identity,
                    operation_source_record_digest,
                    source_record_digest,
                    handoff_outcome_digest,
                    handoff_finalized_cursor,
                    ..
                } => {
                    let position = candidate_state
                        .dead_letters
                        .iter()
                        .position(|entry| {
                            entry.identity == *identity
                                && durable_dead_letter_source_record_digest(entry).ok()
                                    == Some(*source_record_digest)
                                && entry.resolution.is_some()
                        })
                        .ok_or(ModerationOrchestratorError::CheckpointStoreEquivocation)?;
                    candidate_state.dead_letters.remove(position);
                    if let Some(operation_digest) = operation_source_record_digest {
                        let operation_position = candidate_state
                            .operations
                            .iter()
                            .position(|entry| {
                                native_operation_record_digest(entry).ok()
                                    == Some(*operation_digest)
                            })
                            .ok_or(ModerationOrchestratorError::CheckpointStoreEquivocation)?;
                        candidate_state.operations.remove(operation_position);
                    }
                    if let (Some(outcome_digest), Some(cursor)) =
                        (handoff_outcome_digest, handoff_finalized_cursor)
                    {
                        archived_terminal_groups
                            .entry(terminal_handoff_outcome_group_identity(
                                *cursor,
                                *outcome_digest,
                            ))
                            .or_default()
                            .push(*cursor);
                    }
                }
                ModerationTerminalArchiveRecordV1::CompletedHandoff {
                    handoff_id,
                    outcome_digest,
                    finalized_cursor,
                    source_record_digest,
                    ..
                } => {
                    let position = candidate_state
                        .completed_handoffs
                        .iter()
                        .position(|entry| {
                            entry.handoff.handoff_id == *handoff_id
                                && entry.record_digest == *source_record_digest
                        })
                        .ok_or(ModerationOrchestratorError::CheckpointStoreEquivocation)?;
                    candidate_state.completed_handoffs.remove(position);
                    archived_terminal_groups
                        .entry(terminal_handoff_outcome_group_identity(
                            *finalized_cursor,
                            *outcome_digest,
                        ))
                        .or_default()
                        .push(*finalized_cursor);
                }
            }
        }
        for cursors in archived_terminal_groups.values() {
            if cursors.len() == 2 && cursors[0] == cursors[1] {
                let cursor = cursors[0];
                if candidate_state
                    .terminal_handoff_archived_cursor
                    .is_none_or(|archived| cursor.sequence > archived.sequence)
                {
                    candidate_state.terminal_handoff_archived_cursor = Some(cursor);
                }
            }
        }
        candidate_state.panel_notification_archive_head = Some(head.clone());
        candidate_state.panel_notification_archive_compaction_reservation = None;
        candidate_state.panel_notification_archived_dead_letter_count =
            head.cumulative_dead_letter_count;
        candidate_state.panel_notification_archive_pending_publication = Some(head.clone());
        self.persist_checkpoint_locked(&mut candidate_state)?;
        *state = candidate_state;
        Ok(Some(head))
    }
    /// Publish or replay the one durable archive-head outbox entry.
    ///
    /// The signed head is retained in the sealed checkpoint before this method
    /// contacts the independently administered publication boundary. A crash or
    /// ambiguous result therefore replays the exact operation and bytes. The
    /// checkpoint advances its public monotonic head only after durable success.
    ///
    /// # Errors
    ///
    /// Fails closed on missing/corrupt archive readback, publication failure,
    /// provider drift, a concurrent substituted head, or sealed-CAS fencing.
    pub fn reconcile_panel_notification_archive_publication(
        &self,
    ) -> Result<bool, ModerationOrchestratorError> {
        let pending = self
            .lock_state()?
            .panel_notification_archive_pending_publication
            .clone();
        let Some(head) = pending else {
            return Ok(false);
        };
        verify_current_panel_notification_archive_readback(
            &self.config,
            &self.network_id,
            &self.deps.panel_notification_archive,
            Some(&head),
        )?;
        self.deps
            .publication_sink
            .publish_panel_notification_archive_head(&head)
            .map_err(map_panel_notification_archive_publication_error)?;
        verify_published_panel_notification_archive_head_readback(
            &self.deps.publication_sink,
            Some(&head),
        )?;
        let mut state = self.lock_state()?;
        if state.panel_notification_archive_head.as_ref() != Some(&head)
            || state
                .panel_notification_archive_pending_publication
                .as_ref()
                != Some(&head)
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveRejected);
        }
        state.panel_notification_archive_pending_publication = None;
        state.panel_notification_archive_published_head = Some(head);
        self.persist_checkpoint_locked(&mut state)?;
        Ok(true)
    }
    /// Return the exact authenticated current receipt-archive head.
    ///
    /// # Errors
    ///
    /// Fails when state is unavailable or archive readback is missing/corrupt.
    pub fn panel_notification_archive_head(
        &self,
    ) -> Result<Option<ModerationPanelNotificationArchiveHeadV1>, ModerationOrchestratorError> {
        let state = self.lock_state()?;
        let head = state.panel_notification_archive_head.clone();
        let published_head = state.panel_notification_archive_published_head.clone();
        drop(state);
        verify_current_panel_notification_archive_readback(
            &self.config,
            &self.network_id,
            &self.deps.panel_notification_archive,
            head.as_ref(),
        )?;
        verify_published_panel_notification_archive_head_readback(
            &self.deps.publication_sink,
            published_head.as_ref(),
        )?;
        Ok(head)
    }
    /// Return the complete authenticated archive-signer epoch log.
    ///
    /// The bootstrap key anchors the first epoch. Every successor is checked
    /// against its predecessor authorization, new-key proof of possession, and
    /// inclusive revocation cutoff before the log is returned.
    ///
    /// # Errors
    ///
    /// Fails closed for a corrupt sealed log, a substituted bootstrap key or
    /// archive identity, or a current epoch that differs from configuration.
    pub fn panel_notification_archive_signer_epochs(
        &self,
    ) -> Result<Vec<ModerationPanelNotificationArchiveSignerEpochV1>, ModerationOrchestratorError>
    {
        let epochs = self
            .lock_state()?
            .panel_notification_archive_signer_epochs
            .clone();
        validate_panel_notification_archive_signer_epochs(
            &epochs,
            &self.network_id,
            self.config.panel_notification_archive_bootstrap_public_key,
            self.config.panel_notification_archive_id,
        )?;
        let current = epochs
            .last()
            .ok_or(ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
        if current.archive_handle != self.config.panel_notification_archive_handle
            || current.archive_revision
                != self
                    .config
                    .expected_panel_notification_archive_qualification
                    .revision()
            || current.archive_policy_digest
                != self
                    .config
                    .expected_panel_notification_archive_qualification
                    .policy_digest()
            || current.archive_id != self.config.panel_notification_archive_id
            || current.archive_public_key != self.config.panel_notification_archive_public_key
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
        }
        Ok(epochs)
    }
    /// Start a bounded audit of the complete archive lineage from the current
    /// published head through generation one.
    ///
    /// This operator-controlled rehearsal first seals a fresh audit cursor
    /// with no trusted floor, making archive readiness fail closed until the
    /// full lineage completes. The first bounded page is processed before this
    /// call returns; if it is incomplete, continue it with
    /// [`Self::audit_panel_notification_archive`].
    ///
    /// # Errors
    ///
    /// Fails closed on invalid bounds, an unpublished or substituted head,
    /// concurrent checkpoint changes, checkpoint fencing, or any archive
    /// validation failure encountered in the first page.
    pub fn audit_panel_notification_archive_full_history(
        &self,
        maximum_heads: u32,
    ) -> Result<ModerationPanelNotificationArchiveAuditProgressV1, ModerationOrchestratorError>
    {
        if maximum_heads == 0
            || maximum_heads > MODERATION_PANEL_NOTIFICATION_ARCHIVE_AUDIT_PAGE_MAX_V1
        {
            return Err(ModerationOrchestratorError::ResourceExhausted {
                resource: "panel notification archive audit page",
                limit: usize::try_from(MODERATION_PANEL_NOTIFICATION_ARCHIVE_AUDIT_PAGE_MAX_V1)
                    .unwrap_or(usize::MAX),
            });
        }
        let state = self.lock_state()?;
        let Some(latest_head) = state.panel_notification_archive_head.clone() else {
            drop(state);
            return self.audit_panel_notification_archive(maximum_heads);
        };
        if state
            .panel_notification_archive_pending_publication
            .is_some()
            || state.panel_notification_archive_published_head.as_ref() != Some(&latest_head)
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveRejected);
        }
        let source_checkpoint_bytes = norito::to_bytes(&*state).map_err(|error| {
            ModerationOrchestratorError::CheckpointCorrupt(format!(
                "encode full-history archive audit source checkpoint: {error}"
            ))
        })?;
        let source_record = self
            .checkpoint_record
            .lock()
            .map_err(|_| ModerationOrchestratorError::CheckpointStoreLockPoisoned)?
            .clone();
        drop(state);
        verify_published_panel_notification_archive_head_readback(
            &self.deps.publication_sink,
            Some(&latest_head),
        )?;
        let mut state = self.lock_state()?;
        let current_checkpoint_bytes = norito::to_bytes(&*state).map_err(|error| {
            ModerationOrchestratorError::CheckpointCorrupt(format!(
                "encode full-history archive audit commit checkpoint: {error}"
            ))
        })?;
        let current_record = self
            .checkpoint_record
            .lock()
            .map_err(|_| ModerationOrchestratorError::CheckpointStoreLockPoisoned)?
            .clone();
        if current_checkpoint_bytes != source_checkpoint_bytes
            || current_record != source_record
            || state.panel_notification_archive_head.as_ref() != Some(&latest_head)
            || state
                .panel_notification_archive_pending_publication
                .is_some()
            || state.panel_notification_archive_published_head.as_ref() != Some(&latest_head)
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveRejected);
        }
        state.panel_notification_archive_audit_cursor =
            Some(ModerationPanelNotificationArchiveAuditCursorV1 {
                version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
                target_generation: latest_head.generation,
                target_head_digest: latest_head.head_digest,
                next_operation_id: Some(latest_head.operation_id),
                expected_generation: Some(latest_head.generation),
                expected_head_digest: Some(latest_head.head_digest),
                expected_chain_commitment: Some(latest_head.chain_commitment),
                verified_head_count: 0,
                chain_commitment: [0; 32],
                last_completed_generation: 0,
                last_completed_head_digest: None,
            });
        self.persist_checkpoint_locked(&mut state)?;
        drop(state);
        self.audit_panel_notification_archive(maximum_heads)
    }
    /// Authenticate one bounded page of the archive suffix added since the
    /// last completed audit.
    ///
    /// The sealed cursor advances from a fixed target head toward the last
    /// authenticated head. Every page checks exact operation, generation, head
    /// digest, chain accumulator, signature, and predecessor coordinates. The
    /// initial audit reaches generation one; later audits verify only new
    /// generations plus the previously trusted boundary head. A separate
    /// operator-controlled full-history rehearsal via
    /// [`Self::audit_panel_notification_archive_full_history`] detects loss
    /// outside the readiness-critical incremental suffix.
    ///
    /// # Errors
    ///
    /// Fails closed on invalid bounds, missing/corrupt history, forks, gaps,
    /// provider drift, concurrent checkpoint changes, or checkpoint fencing.
    pub fn audit_panel_notification_archive(
        &self,
        maximum_heads: u32,
    ) -> Result<ModerationPanelNotificationArchiveAuditProgressV1, ModerationOrchestratorError>
    {
        if maximum_heads == 0
            || maximum_heads > MODERATION_PANEL_NOTIFICATION_ARCHIVE_AUDIT_PAGE_MAX_V1
        {
            return Err(ModerationOrchestratorError::ResourceExhausted {
                resource: "panel notification archive audit page",
                limit: usize::try_from(MODERATION_PANEL_NOTIFICATION_ARCHIVE_AUDIT_PAGE_MAX_V1)
                    .unwrap_or(usize::MAX),
            });
        }
        let state = self.lock_state()?;
        let Some(latest_head) = state.panel_notification_archive_head.clone() else {
            if state.panel_notification_archive_audit_cursor.is_some() {
                return Err(ModerationOrchestratorError::CheckpointStoreEquivocation);
            }
            return Ok(ModerationPanelNotificationArchiveAuditProgressV1 {
                verified_heads: 0,
                target_generation: 0,
                last_completed_generation: 0,
                cycle_complete: true,
            });
        };
        if state
            .panel_notification_archive_pending_publication
            .is_some()
            || state.panel_notification_archive_published_head.as_ref() != Some(&latest_head)
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveRejected);
        }
        let source_cursor = state.panel_notification_archive_audit_cursor.clone();
        let source_signer_epochs = state.panel_notification_archive_signer_epochs.clone();
        let mut cursor = match source_cursor.as_ref() {
            Some(cursor) if cursor.next_operation_id.is_some() => cursor.clone(),
            previous => ModerationPanelNotificationArchiveAuditCursorV1 {
                version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
                target_generation: latest_head.generation,
                target_head_digest: latest_head.head_digest,
                next_operation_id: Some(latest_head.operation_id),
                expected_generation: Some(latest_head.generation),
                expected_head_digest: Some(latest_head.head_digest),
                expected_chain_commitment: Some(latest_head.chain_commitment),
                verified_head_count: 0,
                chain_commitment: [0; 32],
                last_completed_generation: previous
                    .map_or(0, |value| value.last_completed_generation),
                last_completed_head_digest: previous
                    .and_then(|value| value.last_completed_head_digest),
            },
        };
        let source_checkpoint_bytes = norito::to_bytes(&*state).map_err(|error| {
            ModerationOrchestratorError::CheckpointCorrupt(format!(
                "encode archive audit source checkpoint: {error}"
            ))
        })?;
        let source_record = self
            .checkpoint_record
            .lock()
            .map_err(|_| ModerationOrchestratorError::CheckpointStoreLockPoisoned)?
            .clone();
        drop(state);
        verify_published_panel_notification_archive_head_readback(
            &self.deps.publication_sink,
            Some(&latest_head),
        )?;
        let audit_floor_generation = cursor.last_completed_generation;
        let audit_floor_head_digest = cursor.last_completed_head_digest;
        if latest_head.generation < audit_floor_generation
            || (audit_floor_generation == 0) != audit_floor_head_digest.is_none()
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
        }
        let mut verified_heads = 0_u32;
        while verified_heads < maximum_heads {
            let Some(operation_id) = cursor.next_operation_id else {
                break;
            };
            let expected_generation = cursor
                .expected_generation
                .ok_or(ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
            let expected_head_digest = cursor
                .expected_head_digest
                .ok_or(ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
            let expected_chain_commitment = cursor
                .expected_chain_commitment
                .ok_or(ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
            let head = load_verified_panel_notification_archive_head(
                &self.config,
                &self.network_id,
                &self.deps.panel_notification_archive,
                operation_id,
            )?;
            if head.generation != expected_generation
                || head.head_digest != expected_head_digest
                || head.chain_commitment != expected_chain_commitment
                || verify_panel_notification_archive_head_signer_epoch(&head, &source_signer_epochs)
                    .is_err()
            {
                return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
            }
            cursor.chain_commitment =
                panel_notification_archive_audit_page_commitment(cursor.chain_commitment, &head);
            cursor.verified_head_count = cursor
                .verified_head_count
                .checked_add(1)
                .ok_or(ModerationOrchestratorError::GenerationOverflow)?;
            verified_heads = verified_heads
                .checked_add(1)
                .ok_or(ModerationOrchestratorError::GenerationOverflow)?;
            if audit_floor_generation != 0 && head.generation == audit_floor_generation {
                if Some(head.head_digest) != audit_floor_head_digest {
                    return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
                }
                cursor.next_operation_id = None;
                cursor.expected_generation = None;
                cursor.expected_head_digest = None;
                cursor.expected_chain_commitment = None;
                cursor.last_completed_generation = cursor.target_generation;
                cursor.last_completed_head_digest = Some(cursor.target_head_digest);
                continue;
            }
            if head.generation < audit_floor_generation {
                return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
            }
            match head.generation {
                1 => {
                    if head.predecessor_operation_id.is_some()
                        || head.predecessor_head_digest.is_some()
                        || head.predecessor_chain_commitment.is_some()
                        || audit_floor_generation != 0
                    {
                        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
                    }
                    cursor.next_operation_id = None;
                    cursor.expected_generation = None;
                    cursor.expected_head_digest = None;
                    cursor.expected_chain_commitment = None;
                    cursor.last_completed_generation = cursor.target_generation;
                    cursor.last_completed_head_digest = Some(cursor.target_head_digest);
                }
                2.. => {
                    cursor.next_operation_id = head.predecessor_operation_id;
                    cursor.expected_generation = Some(head.generation - 1);
                    cursor.expected_head_digest = head.predecessor_head_digest;
                    cursor.expected_chain_commitment = head.predecessor_chain_commitment;
                    if cursor.next_operation_id.is_none()
                        || cursor.expected_head_digest.is_none()
                        || cursor.expected_chain_commitment.is_none()
                    {
                        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
                    }
                }
                0 => return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid),
            }
        }
        let mut state = self.lock_state()?;
        let current_checkpoint_bytes = norito::to_bytes(&*state).map_err(|error| {
            ModerationOrchestratorError::CheckpointCorrupt(format!(
                "encode archive audit commit checkpoint: {error}"
            ))
        })?;
        let current_record = self
            .checkpoint_record
            .lock()
            .map_err(|_| ModerationOrchestratorError::CheckpointStoreLockPoisoned)?
            .clone();
        if current_checkpoint_bytes != source_checkpoint_bytes
            || current_record != source_record
            || state.panel_notification_archive_head.as_ref() != Some(&latest_head)
            || state.panel_notification_archive_audit_cursor != source_cursor
            || state.panel_notification_archive_signer_epochs != source_signer_epochs
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveRejected);
        }
        let cycle_complete = cursor.next_operation_id.is_none();
        let target_generation = cursor.target_generation;
        let last_completed_generation = cursor.last_completed_generation;
        state.panel_notification_archive_audit_cursor = Some(cursor);
        self.persist_checkpoint_locked(&mut state)?;
        Ok(ModerationPanelNotificationArchiveAuditProgressV1 {
            verified_heads,
            target_generation,
            last_completed_generation,
            cycle_complete,
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
        let mut checkpoint_record = self
            .checkpoint_record
            .lock()
            .map_err(|_| ModerationOrchestratorError::CheckpointStoreLockPoisoned)?;
        if let Err(error) = checkpoint_store::persist_authoritative_checkpoint(
            &self.config,
            &self.network_id,
            &self.deps.checkpoint_store,
            &mut checkpoint_record,
            state,
        ) {
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
                    let incident_sequence = next_dead_letter_incident_sequence(state)?;
                    dead.push(StoredDeadLetterV1 {
                        incident_sequence,
                        identity: entry.operation_id,
                        action_label: entry.action.label().to_owned(),
                        reason: StoredDeadLetterReasonV1::FinalizedConflict,
                        finalized_cursor: cursor,
                        dead_lettered_at_unix_ms: snapshot.finalized_at_unix_ms,
                        redrive: Some(StoredDeadLetterRedriveV1::NativeSubmission {
                            authority: entry.authority,
                            action: entry.action,
                            request_binding_digest: entry.request_binding_digest,
                        }),
                        resolution: None,
                        resolution_signature: None,
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
                let request = moderation_transaction_request(&self.network_id, entry)?;
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
                    let request = moderation_transaction_request(&self.network_id, &candidate)?;
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
                    let request = moderation_transaction_request(&self.network_id, &candidate)?;
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
        let request = moderation_transaction_request(&self.network_id, &candidate)?;
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
        let dead_lettered_at_unix_ms = state
            .finalized_snapshot
            .as_ref()
            .ok_or(ModerationOrchestratorError::FinalizedReaderUnavailable)?
            .finalized_at_unix_ms;
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
                let mut completed = StoredCompletedHandoffV1 {
                    handoff: entry.handoff,
                    completed_at_finalized_cursor: cursor,
                    record_digest: [0; 32],
                };
                completed.record_digest = completed_handoff_record_digest(&completed)?;
                state.completed_handoffs.push(completed);
                state
                    .completed_handoffs
                    .sort_by_key(|entry| entry.handoff.handoff_id);
                state
                    .completed_handoffs
                    .dedup_by_key(|entry| entry.handoff.handoff_id);
            }
            Err(ModerationHandoffFailureV1::Permanent) => {
                let incident_sequence = next_dead_letter_incident_sequence(&mut state)?;
                state.dead_letters.push(StoredDeadLetterV1 {
                    incident_sequence,
                    identity: entry.handoff.handoff_id,
                    action_label: handoff_label(entry.handoff.kind).to_owned(),
                    reason: StoredDeadLetterReasonV1::HandoffPermanentRejection,
                    finalized_cursor: cursor,
                    dead_lettered_at_unix_ms,
                    redrive: Some(StoredDeadLetterRedriveV1::TerminalHandoff(entry.handoff)),
                    resolution: None,
                    resolution_signature: None,
                });
            }
            Err(
                ModerationHandoffFailureV1::NotDelivered | ModerationHandoffFailureV1::Ambiguous,
            ) if entry.attempts >= self.config.max_submit_attempts => {
                let incident_sequence = next_dead_letter_incident_sequence(&mut state)?;
                state.dead_letters.push(StoredDeadLetterV1 {
                    incident_sequence,
                    identity: entry.handoff.handoff_id,
                    action_label: handoff_label(entry.handoff.kind).to_owned(),
                    reason: StoredDeadLetterReasonV1::HandoffRetryExhausted,
                    finalized_cursor: cursor,
                    dead_lettered_at_unix_ms,
                    redrive: Some(StoredDeadLetterRedriveV1::TerminalHandoff(entry.handoff)),
                    resolution: None,
                    resolution_signature: None,
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
        let dead_lettered_at_unix_ms = state
            .finalized_snapshot
            .as_ref()
            .ok_or(ModerationOrchestratorError::FinalizedReaderUnavailable)?
            .finalized_at_unix_ms;
        let incident_sequence = next_dead_letter_incident_sequence(state)?;
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
            incident_sequence,
            identity: entry.operation_id,
            action_label: entry.action.label().to_owned(),
            reason,
            finalized_cursor: cursor,
            dead_lettered_at_unix_ms,
            redrive: Some(StoredDeadLetterRedriveV1::NativeSubmission {
                authority: entry.authority,
                action: entry.action,
                request_binding_digest: entry.request_binding_digest,
            }),
            resolution: None,
            resolution_signature: None,
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
                        action.operation_id(&self.network_id, event.event.authority())?;
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
                                &self.network_id,
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
                        action.operation_id(&self.network_id, event.event.authority())?;
                    for recipient in &case.case.spec.jurors {
                        let entry = new_panel_notification_entry(
                            &self.network_id,
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
            .map(|entry| entry.handoff.handoff_id)
            .collect::<BTreeSet<_>>();
        let pending = state
            .pending_handoffs
            .iter()
            .map(|entry| entry.handoff.handoff_id)
            .collect::<BTreeSet<_>>();
        let unresolved_dead = state
            .dead_letters
            .iter()
            .filter(|entry| {
                entry.resolution.is_none()
                    && matches!(
                        entry.redrive,
                        Some(StoredDeadLetterRedriveV1::TerminalHandoff(_))
                    )
            })
            .map(|entry| entry.identity)
            .collect::<BTreeSet<_>>();
        let scanned_cursor = state.terminal_handoff_scanned_cursor;
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
                "terminal handoff event scan contains a sequence gap".to_owned(),
            ));
        }
        if scanned_cursor.is_none()
            && snapshot.cases.iter().any(|case| {
                case.outcome.as_ref().is_some_and(|outcome| {
                    !new_events.iter().any(|event| {
                        *event.event.kind() == SorafsModerationLedgerEventKind::CaseFinalized
                            && event.event.case_id().as_deref() == Some(&outcome.case_id)
                            && event.event.round_id().as_deref() == Some(&outcome.round_id)
                    })
                })
            })
        {
            return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                "terminal handoff initial scan lacks exact finalized-event history".to_owned(),
            ));
        }
        let mut additions = Vec::new();
        for event in &new_events {
            if *event.event.kind() != SorafsModerationLedgerEventKind::CaseFinalized {
                continue;
            }
            let (Some(case_id), Some(round_id)) = (event.event.case_id(), event.event.round_id())
            else {
                return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                    "terminal finalization event lacks canonical scope".to_owned(),
                ));
            };
            let outcome = snapshot
                .case(case_id, round_id)
                .and_then(|case| case.outcome.as_ref())
                .ok_or_else(|| {
                    ModerationOrchestratorError::InvalidFinalizedSnapshot(
                        "terminal finalization event lacks authoritative outcome".to_owned(),
                    )
                })?;
            if !terminal_finalization_event_matches_outcome(event, outcome) {
                return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                    "terminal finalization event provenance differs from outcome".to_owned(),
                ));
            }
            let outcome_bytes = norito::to_bytes(outcome).map_err(|error| {
                ModerationOrchestratorError::InvalidFinalizedSnapshot(format!(
                    "encode terminal outcome: {error}"
                ))
            })?;
            let outcome_digest = domain_hash(ACTION_DIGEST_DOMAIN_V1, &[&outcome_bytes]);
            let handoff_ids = [
                ModerationTerminalHandoffKindV1::Settlement,
                ModerationTerminalHandoffKindV1::Publication,
            ]
            .map(|kind| {
                (
                    kind,
                    terminal_handoff_id(
                        &self.network_id,
                        kind,
                        &outcome.case_id,
                        &outcome.round_id,
                        outcome_digest,
                    ),
                )
            });
            if handoff_ids.iter().all(|(_, handoff_id)| {
                completed.contains(handoff_id)
                    || pending.contains(handoff_id)
                    || unresolved_dead.contains(handoff_id)
            }) {
                continue;
            }
            let finalized_cursor = event.cursor();
            for (kind, handoff_id) in handoff_ids {
                if !completed.contains(&handoff_id)
                    && !pending.contains(&handoff_id)
                    && !unresolved_dead.contains(&handoff_id)
                {
                    additions.push(StoredHandoffV1 {
                        handoff: ModerationTerminalHandoffV1 {
                            handoff_id,
                            network_id: self.network_id,
                            kind,
                            case_id: outcome.case_id.clone(),
                            round_id: outcome.round_id.clone(),
                            outcome_digest,
                            outcome_finalized_at_unix_ms: outcome.finalized_at_unix_ms,
                            finalized_cursor,
                            source_event_witness: (*event).clone(),
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
        if let Some(last) = new_events.last() {
            state.terminal_handoff_scanned_cursor = Some(last.cursor());
        }
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
            SorafsModerationLedgerEventKind::CaseFinalized => {
                let outcome = snapshot
                    .case(case_id, round_id)
                    .and_then(|case| case.outcome.as_ref())
                    .ok_or_else(|| {
                        ModerationOrchestratorError::InvalidFinalizedSnapshot(
                            "finalization event has no authoritative terminal outcome".to_owned(),
                        )
                    })?;
                if !terminal_finalization_event_matches_outcome(event, outcome) {
                    return Err(ModerationOrchestratorError::InvalidFinalizedSnapshot(
                        "finalization event provenance differs from the authoritative outcome"
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
            | SorafsModerationLedgerEventKind::RevealAccepted => {}
        }
    }
    Ok(())
}
fn validate_retained_panel_notification_source(
    notification: &ModerationPanelNotificationV1,
    snapshot: &ModerationFinalizedLedgerSnapshotV1,
    network_id: &iroha_data_model::NetworkId,
) -> Result<(), ModerationOrchestratorError> {
    if !notification.is_bound_to_network(network_id) {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "panel notification belongs to a different network".to_owned(),
        ));
    }
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
            .operation_id(network_id, event.event.authority())
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
            .operation_id(network_id, event.event.authority())
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
#[derive(Clone, Copy)]
struct RetainedDeadLetterResolutionExpectationV1 {
    signature: [u8; 64],
    source_record_digest: [u8; 32],
    identity: [u8; 32],
    kind: ModerationDeadLetterKindV1,
    incident_time: u64,
}
fn validate_retained_dead_letter_resolution(
    resolution: &ModerationDeadLetterResolutionV1,
    expected: RetainedDeadLetterResolutionExpectationV1,
    state: &ModerationOrchestratorCheckpointV1,
    config: &ModerationOrchestratorConfigV1,
    network_id: &iroha_data_model::NetworkId,
) -> Result<(), ModerationOrchestratorError> {
    verify_dead_letter_resolution_signature(resolution, expected.signature)?;
    let finalized_time = state
        .finalized_snapshot
        .as_ref()
        .ok_or_else(|| {
            ModerationOrchestratorError::CheckpointCorrupt(
                "dead-letter resolution exists without a finalized snapshot".to_owned(),
            )
        })?
        .finalized_at_unix_ms;
    if resolution.network_id != *network_id
        || resolution.checkpoint_namespace_digest
            != checkpoint_store::checkpoint_namespace(network_id)
        || resolution.checkpoint_generation == 0
        || resolution.checkpoint_generation >= state.generation
        || resolution.identity != expected.identity
        || resolution.kind != expected.kind
        || resolution.source_record_digest != expected.source_record_digest
        || resolution.authorized_at_unix_ms < expected.incident_time
        || resolution.authorized_at_unix_ms > finalized_time
        || resolution.attestor_handle != config.checkpoint_store_handle
        || resolution.attestor_revision != config.expected_checkpoint_store_qualification.revision()
        || resolution.attestor_policy_digest
            != config
                .expected_checkpoint_store_qualification
                .policy_digest()
        || resolution.attestor_public_key != config.checkpoint_store_attestation_public_key
    {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "dead-letter resolution binding is stale, substituted, or noncanonical".to_owned(),
        ));
    }
    Ok(())
}
fn validate_checkpoint(
    state: &ModerationOrchestratorCheckpointV1,
    config: &ModerationOrchestratorConfigV1,
    network_id: &iroha_data_model::NetworkId,
) -> Result<(), ModerationOrchestratorError> {
    if state.version != MODERATION_ORCHESTRATOR_CHECKPOINT_VERSION_V1 {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "unsupported checkpoint version".to_owned(),
        ));
    }
    if state.network_id != *network_id {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "checkpoint network binding differs from the qualified runtime".to_owned(),
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
        || state
            .panel_notifications
            .len()
            .saturating_add(state.panel_notification_dead_letter_resolutions.len())
            > config.max_handoffs
    {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "checkpoint exceeds configured retention bounds".to_owned(),
        ));
    }
    if state.panel_notification_archive_signer_epochs.is_empty() {
        if state.panel_notification_archive_head.is_some()
            || state
                .panel_notification_archive_compaction_reservation
                .is_some()
            || state
                .panel_notification_archive_pending_publication
                .is_some()
            || state.panel_notification_archive_published_head.is_some()
            || state.panel_notification_archived_dead_letter_count != 0
        {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "archive state exists without a signer epoch log".to_owned(),
            ));
        }
    } else {
        validate_panel_notification_archive_signer_epochs(
            &state.panel_notification_archive_signer_epochs,
            network_id,
            config.panel_notification_archive_bootstrap_public_key,
            config.panel_notification_archive_id,
        )
        .map_err(|_| {
            ModerationOrchestratorError::CheckpointCorrupt(
                "archive signer epoch log is malformed".to_owned(),
            )
        })?;
    }
    if let Some(head) = state.panel_notification_archive_head.as_ref() {
        verify_panel_notification_archive_head(head)?;
        verify_panel_notification_archive_head_signer_epoch(
            head,
            &state.panel_notification_archive_signer_epochs,
        )?;
        if head.source_checkpoint_generation >= state.generation {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "receipt archive source checkpoint does not precede retained state".to_owned(),
            ));
        }
        if state.panel_notification_archived_dead_letter_count != head.cumulative_dead_letter_count
        {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "archived panel-notification dead-letter count differs from the signed head"
                    .to_owned(),
            ));
        }
    } else if state.panel_notification_archived_dead_letter_count != 0 {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "archived panel-notification dead letters exist without an archive head".to_owned(),
        ));
    }
    let publication_state_is_valid = match (
        state.panel_notification_archive_head.as_ref(),
        state
            .panel_notification_archive_pending_publication
            .as_ref(),
        state.panel_notification_archive_published_head.as_ref(),
    ) {
        (None, None, None) => true,
        (Some(current), None, Some(published)) => current == published,
        (Some(current), Some(pending), None) => pending == current && current.generation == 1,
        (Some(current), Some(pending), Some(published)) => {
            pending == current
                && verify_panel_notification_archive_head(published).is_ok()
                && verify_panel_notification_archive_head_signer_epoch(
                    published,
                    &state.panel_notification_archive_signer_epochs,
                )
                .is_ok()
                && verify_panel_notification_archive_lineage_link(current, published).is_ok()
        }
        _ => false,
    };
    if !publication_state_is_valid {
        return Err(ModerationOrchestratorError::CheckpointCorrupt(
            "archive publication outbox is nonmonotonic or inconsistent".to_owned(),
        ));
    }
    match (
        state.panel_notification_archive_head.as_ref(),
        state.panel_notification_archive_audit_cursor.as_ref(),
    ) {
        (None, None) | (Some(_), None) => {}
        (None, Some(_)) => {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "archive audit cursor exists without an archive head".to_owned(),
            ));
        }
        (Some(head), Some(cursor)) => {
            let pending_coordinates_are_complete = matches!(
                (
                    cursor.next_operation_id,
                    cursor.expected_generation,
                    cursor.expected_head_digest,
                    cursor.expected_chain_commitment,
                ),
                (Some(_), Some(_), Some(_), Some(_)) | (None, None, None, None)
            );
            let last_completed_is_valid = match (
                cursor.last_completed_generation,
                cursor.last_completed_head_digest,
            ) {
                (0, None) => true,
                (generation, Some(digest)) => {
                    generation != 0 && generation <= cursor.target_generation && digest != [0; 32]
                }
                _ => false,
            };
            let progress_is_valid = match cursor.expected_generation {
                Some(expected_generation) => {
                    expected_generation != 0
                        && expected_generation <= cursor.target_generation
                        && expected_generation >= cursor.last_completed_generation
                        && cursor.verified_head_count.checked_add(expected_generation)
                            == Some(cursor.target_generation)
                }
                None => {
                    cursor.verified_head_count != 0
                        && cursor.verified_head_count <= cursor.target_generation
                        && cursor.last_completed_generation == cursor.target_generation
                        && cursor.last_completed_head_digest == Some(cursor.target_head_digest)
                }
            };
            if cursor.version != MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1
                || cursor.target_generation == 0
                || cursor.target_generation > head.generation
                || cursor.target_head_digest == [0; 32]
                || !pending_coordinates_are_complete
                || !last_completed_is_valid
                || !progress_is_valid
                || (cursor.verified_head_count == 0) != (cursor.chain_commitment == [0; 32])
            {
                return Err(ModerationOrchestratorError::CheckpointCorrupt(
                    "archive audit cursor is malformed or nonmonotonic".to_owned(),
                ));
            }
        }
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
            if state.panel_notification_scanned_cursor.is_some()
                || state.terminal_handoff_scanned_cursor.is_some()
                || state.terminal_handoff_archived_cursor.is_some()
            {
                return Err(ModerationOrchestratorError::CheckpointCorrupt(
                    "moderation scan or archive cursor exists without a finalized snapshot"
                        .to_owned(),
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
            if !snapshot.events.is_empty() && state.terminal_handoff_scanned_cursor.is_none() {
                return Err(ModerationOrchestratorError::CheckpointCorrupt(
                    "finalized moderation events were not scanned for terminal handoffs".to_owned(),
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
            if let Some(scanned) = state.terminal_handoff_scanned_cursor {
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
                if scanned.sequence == 0
                    || scanned.block_height == 0
                    || scanned.block_hash == [0; 32]
                    || scanned.block_height > snapshot.finalized_height
                    || snapshot
                        .events
                        .last()
                        .is_some_and(|event| scanned.sequence > event.sequence)
                    || retained_exact.is_some_and(|event| event.cursor() != scanned)
                    || invalid_gap
                {
                    return Err(ModerationOrchestratorError::CheckpointCorrupt(
                        "terminal handoff scan cursor differs from the finalized snapshot"
                            .to_owned(),
                    ));
                }
            }
            match (
                state.terminal_handoff_archived_cursor,
                state.terminal_handoff_scanned_cursor,
            ) {
                (None, _) => {}
                (Some(_), None) => {
                    return Err(ModerationOrchestratorError::CheckpointCorrupt(
                        "terminal handoff archive cursor exists without a scan cursor".to_owned(),
                    ));
                }
                (Some(archived), Some(scanned)) => {
                    let retained_exact = snapshot
                        .events
                        .iter()
                        .find(|event| event.sequence == archived.sequence);
                    if archived.sequence == 0
                        || archived.block_height == 0
                        || archived.block_hash == [0; 32]
                        || archived.sequence > scanned.sequence
                        || archived.block_height > scanned.block_height
                        || (archived.sequence == scanned.sequence && archived != scanned)
                        || retained_exact.is_some_and(|event| event.cursor() != archived)
                    {
                        return Err(ModerationOrchestratorError::CheckpointCorrupt(
                            "terminal handoff archive cursor is nonmonotonic or substituted"
                                .to_owned(),
                        ));
                    }
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
        if entry.action.operation_id(network_id, &entry.authority)? != entry.operation_id {
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
            let request = moderation_transaction_request(network_id, entry)?;
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
    let mut handoffs = BTreeSet::new();
    for completed in &state.completed_handoffs {
        validate_retained_terminal_handoff(
            &completed.handoff,
            state.finalized_snapshot.as_ref(),
            network_id,
        )?;
        if !handoffs.insert(completed.handoff.handoff_id)
            || !external_work_cursor_is_valid(
                completed.completed_at_finalized_cursor.height,
                completed.completed_at_finalized_cursor.block_hash,
                state.finalized_snapshot.as_ref(),
            )
            || completed.completed_at_finalized_cursor.height
                < completed.handoff.finalized_cursor.block_height
            || completed.record_digest == [0; 32]
            || completed.record_digest != completed_handoff_record_digest(completed)?
        {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "completed terminal handoff is duplicate, stale, or corrupt".to_owned(),
            ));
        }
    }
    for entry in &state.pending_handoffs {
        validate_retained_terminal_handoff(
            &entry.handoff,
            state.finalized_snapshot.as_ref(),
            network_id,
        )?;
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
    let finalized_time = state
        .finalized_snapshot
        .as_ref()
        .map(|snapshot| snapshot.finalized_at_unix_ms);
    let mut previous_incident_sequence = 0;
    let mut unresolved_dead_letters = BTreeSet::new();
    for entry in &state.dead_letters {
        let source_record_digest = durable_dead_letter_source_record_digest(entry)?;
        let expected_kind = match (&entry.redrive, entry.reason) {
            (
                Some(StoredDeadLetterRedriveV1::NativeSubmission { .. }),
                StoredDeadLetterReasonV1::PermanentRejection
                | StoredDeadLetterReasonV1::FinalizedConflict
                | StoredDeadLetterReasonV1::RetryExhaustedNotFound,
            ) => ModerationDeadLetterKindV1::NativeSubmission,
            (
                Some(StoredDeadLetterRedriveV1::TerminalHandoff(_)),
                StoredDeadLetterReasonV1::HandoffPermanentRejection
                | StoredDeadLetterReasonV1::HandoffRetryExhausted,
            ) => ModerationDeadLetterKindV1::TerminalHandoff,
            _ => {
                return Err(ModerationOrchestratorError::CheckpointCorrupt(
                    "dead-letter reason and exact redrive source disagree".to_owned(),
                ));
            }
        };
        if entry.incident_sequence == 0
            || entry.incident_sequence <= previous_incident_sequence
            || entry.incident_sequence > state.dead_letter_incident_sequence
            || entry.identity == [0; 32]
            || entry.action_label.is_empty()
            || !external_work_cursor_is_valid(
                entry.finalized_cursor.height,
                entry.finalized_cursor.block_hash,
                state.finalized_snapshot.as_ref(),
            )
            || entry.dead_lettered_at_unix_ms == 0
            || finalized_time.is_none_or(|time| entry.dead_lettered_at_unix_ms > time)
        {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "durable dead-letter identity, sequence, cursor, or timestamp is invalid"
                    .to_owned(),
            ));
        }
        previous_incident_sequence = entry.incident_sequence;
        let Some(redrive) = entry.redrive.as_ref() else {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "dead letter has no exact redrive source".to_owned(),
            ));
        };
        match redrive {
            StoredDeadLetterRedriveV1::NativeSubmission {
                authority,
                action,
                request_binding_digest,
            } => {
                action.validate_authority(authority)?;
                let action_digest = action.action_digest()?;
                let operation = operations.get(&entry.identity).copied();
                if *request_binding_digest == [0; 32]
                    || action.operation_id(network_id, authority)? != entry.identity
                    || action.label() != entry.action_label
                    || (entry.resolution.is_none() && operation.is_none())
                    || operation.is_some_and(|operation| {
                        action_digest != operation.action_digest
                            || &operation.authority != authority
                            || ((entry.resolution.is_none()
                                || entry.resolution.as_ref().is_some_and(|resolution| {
                                    resolution.action
                                        == ModerationDeadLetterResolutionActionV1::Acknowledge
                                }))
                                && operation.status != StoredOperationStatusV1::Rejected)
                    })
                {
                    return Err(ModerationOrchestratorError::CheckpointCorrupt(
                        "native dead-letter source differs from its operation tombstone".to_owned(),
                    ));
                }
            }
            StoredDeadLetterRedriveV1::TerminalHandoff(handoff) => {
                validate_retained_terminal_handoff(
                    handoff,
                    state.finalized_snapshot.as_ref(),
                    network_id,
                )?;
                if handoff.handoff_id != entry.identity
                    || handoff_label(handoff.kind) != entry.action_label
                    || (entry.resolution.is_none() && handoffs.contains(&entry.identity))
                {
                    return Err(ModerationOrchestratorError::CheckpointCorrupt(
                        "terminal-handoff dead letter is substituted or simultaneously active"
                            .to_owned(),
                    ));
                }
            }
        }
        match (entry.resolution.as_ref(), entry.resolution_signature) {
            (None, None) => {
                if !unresolved_dead_letters.insert((expected_kind.tag(), entry.identity)) {
                    return Err(ModerationOrchestratorError::CheckpointCorrupt(
                        "duplicate unresolved dead-letter identity".to_owned(),
                    ));
                }
            }
            (Some(resolution), Some(signature)) => {
                validate_retained_dead_letter_resolution(
                    resolution,
                    RetainedDeadLetterResolutionExpectationV1 {
                        signature,
                        source_record_digest,
                        identity: entry.identity,
                        kind: expected_kind,
                        incident_time: entry.dead_lettered_at_unix_ms,
                    },
                    state,
                    config,
                    network_id,
                )?;
            }
            _ => {
                return Err(ModerationOrchestratorError::CheckpointCorrupt(
                    "dead-letter resolution and signature presence disagree".to_owned(),
                ));
            }
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
            || !notification.is_bound_to_network(network_id)
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
            validate_retained_panel_notification_source(notification, snapshot, network_id)?;
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
    let mut panel_resolution_records = BTreeSet::new();
    for entry in &state.panel_notification_dead_letter_resolutions {
        validate_archived_panel_notification_record_shape(&entry.terminal_record).map_err(
            |_| {
                ModerationOrchestratorError::CheckpointCorrupt(
                    "resolved panel dead-letter terminal record is malformed".to_owned(),
                )
            },
        )?;
        let dead_lettered_at_unix_ms = match &entry.terminal_record.terminal_status {
            ModerationPanelNotificationArchiveTerminalStatusV1::DeadLettered {
                dead_lettered_at_unix_ms,
                ..
            } => *dead_lettered_at_unix_ms,
            ModerationPanelNotificationArchiveTerminalStatusV1::Delivered { .. } => {
                return Err(ModerationOrchestratorError::CheckpointCorrupt(
                    "panel dead-letter resolution history contains a delivered record".to_owned(),
                ));
            }
        };
        if entry.record_digest == [0; 32]
            || !panel_resolution_records.insert(entry.record_digest)
            || entry.record_digest
                != panel_notification_resolution_record_digest(
                    &entry.terminal_record,
                    &entry.resolution,
                    entry.resolution_signature,
                )?
            || (entry.resolution.action == ModerationDeadLetterResolutionActionV1::Acknowledge
                && state.panel_notifications.iter().any(|notification| {
                    notification.notification.notification_id
                        == entry.terminal_record.notification_id
                }))
        {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "panel dead-letter resolution history is duplicate or inconsistent".to_owned(),
            ));
        }
        validate_retained_dead_letter_resolution(
            &entry.resolution,
            RetainedDeadLetterResolutionExpectationV1 {
                signature: entry.resolution_signature,
                source_record_digest: entry.terminal_record.source_record_digest,
                identity: entry.terminal_record.notification_id,
                kind: ModerationDeadLetterKindV1::PanelNotification,
                incident_time: dead_lettered_at_unix_ms,
            },
            state,
            config,
            network_id,
        )?;
    }
    if let Some(reservation) = state
        .panel_notification_archive_compaction_reservation
        .as_ref()
    {
        let available_records = collect_terminal_archive_records(state)?;
        let encoded_reservation = norito::to_bytes(reservation).map_err(|error| {
            ModerationOrchestratorError::CheckpointCorrupt(format!(
                "encode terminal archive reservation: {error}"
            ))
        })?;
        let wrapper_bytes =
            usize::try_from(MODERATION_PANEL_NOTIFICATION_ARCHIVE_WRAPPER_MAX_BYTES_V1)
                .unwrap_or(usize::MAX);
        let archive_max_bytes =
            usize::try_from(config.panel_notification_archive_max_bytes).unwrap_or(usize::MAX);
        if reservation.version != MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1
            || reservation.records.is_empty()
            || reservation.records.len() > MODERATION_PANEL_NOTIFICATION_ARCHIVE_MAX_RECORDS_V1
            || available_records.len() < reservation.records.len()
            || available_records[..reservation.records.len()] != reservation.records
            || safe_terminal_archive_prefix_len(&available_records, reservation.records.len())?
                != reservation.records.len()
            || encoded_reservation
                .len()
                .checked_add(wrapper_bytes)
                .is_none_or(|total| total > archive_max_bytes)
        {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "terminal archive reservation is missing its exact canonical retained prefix"
                    .to_owned(),
            ));
        }
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
fn next_dead_letter_incident_sequence(
    state: &mut ModerationOrchestratorCheckpointV1,
) -> Result<u64, ModerationOrchestratorError> {
    state.dead_letter_incident_sequence = state
        .dead_letter_incident_sequence
        .checked_add(1)
        .ok_or(ModerationOrchestratorError::GenerationOverflow)?;
    Ok(state.dead_letter_incident_sequence)
}
fn make_panel_notification_capacity(
    state: &ModerationOrchestratorCheckpointV1,
    additional: usize,
    limit: usize,
) -> Result<(), ModerationOrchestratorError> {
    // Bounded pruning remains disabled unless an authenticated signed archive
    // durably installs and reads back every terminal notification receipt.
    if state.panel_notifications.len().saturating_add(additional) > limit {
        return Err(ModerationOrchestratorError::ResourceExhausted {
            resource: "panel notifications",
            limit,
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
fn moderation_transaction_request(
    network_id: &iroha_data_model::NetworkId,
    entry: &StoredOutboxEntryV1,
) -> Result<ModerationTransactionRequestV1, ModerationOrchestratorError> {
    ModerationTransactionRequestV1::new(
        *network_id,
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
        if let Some((height, finalized_at_unix_ms, created_at_unix_ms)) = previous_retirement
            && (record.retired_at_finalized_height <= height
                || record.retired_at_finalized_unix_ms < finalized_at_unix_ms
                || record.created_at_unix_ms <= created_at_unix_ms)
        {
            return Err(ModerationOrchestratorError::CheckpointCorrupt(
                "retired signed-envelope history is not monotonic".to_owned(),
            ));
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
        || transaction.network_id() != Some(&request.network_id)
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
    let finalized_height = handoff.finalized_cursor.block_height.to_le_bytes();
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
fn recover_external_work_after_restart(
    state: &mut ModerationOrchestratorCheckpointV1,
) -> Result<bool, ModerationOrchestratorError> {
    let has_claims = state.outbox.iter().any(|entry| entry.work_claim.is_some())
        || state
            .pending_handoffs
            .iter()
            .any(|entry| entry.work_claim.is_some());
    if !has_claims {
        return Ok(false);
    }
    // A process restart is not evidence that another replica's durable lease
    // has expired. Only the sealed finalized-ledger time may release a claim;
    // preserving live claims prevents overlapping sign, submit, lookup, and
    // terminal-handoff calls across replicas that share the checkpoint CAS.
    recover_expired_external_work_claims(state)
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
fn panel_notification_worker_id(
    network_id: &iroha_data_model::NetworkId,
    handle: &str,
    qualification: ModerationRuntimeProviderQualificationV1,
) -> [u8; 32] {
    domain_hash(
        PANEL_NOTIFICATION_WORKER_DOMAIN_V1,
        &[
            network_id.as_bytes(),
            handle.as_bytes(),
            &qualification.revision().to_le_bytes(),
            &qualification.policy_digest(),
        ],
    )
}
fn panel_notification_id(
    network_id: &iroha_data_model::NetworkId,
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
            network_id.as_bytes(),
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
    network_id: &iroha_data_model::NetworkId,
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
        network_id,
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
            network_id: *network_id,
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
fn stored_dead_letter_reason_tag(reason: StoredDeadLetterReasonV1) -> u8 {
    match reason {
        StoredDeadLetterReasonV1::PermanentRejection => 0,
        StoredDeadLetterReasonV1::FinalizedConflict => 1,
        StoredDeadLetterReasonV1::RetryExhaustedNotFound => 2,
        StoredDeadLetterReasonV1::HandoffPermanentRejection => 3,
        StoredDeadLetterReasonV1::HandoffRetryExhausted => 4,
    }
}
fn durable_dead_letter_source_record_digest(
    entry: &StoredDeadLetterV1,
) -> Result<[u8; 32], ModerationOrchestratorError> {
    let redrive = norito::to_bytes(&entry.redrive).map_err(|error| {
        ModerationOrchestratorError::CheckpointCorrupt(format!(
            "encode durable dead-letter redrive source: {error}"
        ))
    })?;
    Ok(domain_hash(
        DURABLE_DEAD_LETTER_RECORD_DOMAIN_V1,
        &[
            &entry.incident_sequence.to_le_bytes(),
            &entry.identity,
            entry.action_label.as_bytes(),
            &[stored_dead_letter_reason_tag(entry.reason)],
            &entry.finalized_cursor.height.to_le_bytes(),
            &entry.finalized_cursor.block_hash,
            &entry.dead_lettered_at_unix_ms.to_le_bytes(),
            &redrive,
        ],
    ))
}
fn native_operation_record_digest(
    entry: &StoredOperationV1,
) -> Result<[u8; 32], ModerationOrchestratorError> {
    let bytes = norito::to_bytes(entry).map_err(|error| {
        ModerationOrchestratorError::CheckpointCorrupt(format!(
            "encode native operation tombstone: {error}"
        ))
    })?;
    Ok(domain_hash(NATIVE_OPERATION_RECORD_DOMAIN_V1, &[&bytes]))
}
fn completed_handoff_record_digest(
    entry: &StoredCompletedHandoffV1,
) -> Result<[u8; 32], ModerationOrchestratorError> {
    let witness = norito::to_bytes(&entry.handoff.source_event_witness).map_err(|error| {
        ModerationOrchestratorError::CheckpointCorrupt(format!(
            "encode completed handoff event witness: {error}"
        ))
    })?;
    Ok(domain_hash(
        COMPLETED_HANDOFF_RECORD_DOMAIN_V1,
        &[
            &entry.handoff.handoff_id,
            &[match entry.handoff.kind {
                ModerationTerminalHandoffKindV1::Settlement => 0,
                ModerationTerminalHandoffKindV1::Publication => 1,
            }],
            &entry.handoff.outcome_digest,
            &entry.handoff.outcome_finalized_at_unix_ms.to_le_bytes(),
            &entry.handoff.finalized_cursor.sequence.to_le_bytes(),
            &entry.handoff.finalized_cursor.block_height.to_le_bytes(),
            &entry.handoff.finalized_cursor.block_hash,
            &entry.handoff.finalized_cursor.event_index.to_le_bytes(),
            &entry.completed_at_finalized_cursor.height.to_le_bytes(),
            &entry.completed_at_finalized_cursor.block_hash,
            &witness,
        ],
    ))
}
fn panel_notification_resolution_record_digest(
    terminal_record: &ModerationPanelNotificationArchiveRecordV1,
    resolution: &ModerationDeadLetterResolutionV1,
    signature: [u8; 64],
) -> Result<[u8; 32], ModerationOrchestratorError> {
    let terminal_bytes = norito::to_bytes(terminal_record).map_err(|error| {
        ModerationOrchestratorError::CheckpointCorrupt(format!(
            "encode resolved panel dead letter: {error}"
        ))
    })?;
    Ok(domain_hash(
        DEAD_LETTER_RESOLUTION_DOMAIN_V1,
        &[
            b"panel-history",
            &terminal_bytes,
            &dead_letter_resolution_message(resolution),
            &signature,
        ],
    ))
}
fn validate_dead_letter_resolution_shape(
    resolution: &ModerationDeadLetterResolutionV1,
) -> Result<(), ModerationOrchestratorError> {
    let qualification = ModerationRuntimeProviderQualificationV1::new(
        resolution.attestor_revision,
        resolution.attestor_policy_digest,
    );
    if resolution.version != 1
        || resolution.checkpoint_namespace_digest == [0; 32]
        || resolution.checkpoint_generation == 0
        || resolution.checkpoint_revision == [0; 32]
        || resolution.checkpoint_digest == [0; 32]
        || resolution.identity == [0; 32]
        || resolution.source_record_digest == [0; 32]
        || resolution.authorized_at_unix_ms == 0
        || validate_production_runtime_handle(&resolution.attestor_handle).is_err()
        || !qualification.is_valid()
        || PublicKey::from_bytes(Algorithm::Ed25519, &resolution.attestor_public_key).is_err()
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    Ok(())
}
fn dead_letter_resolution_message(resolution: &ModerationDeadLetterResolutionV1) -> [u8; 32] {
    domain_hash(
        DEAD_LETTER_RESOLUTION_DOMAIN_V1,
        &[
            &resolution.version.to_le_bytes(),
            resolution.network_id.as_bytes(),
            &resolution.checkpoint_namespace_digest,
            &resolution.checkpoint_generation.to_le_bytes(),
            &resolution.checkpoint_revision,
            &resolution.checkpoint_digest,
            &resolution.identity,
            &[resolution.kind.tag()],
            &[resolution.action.tag()],
            &resolution.source_record_digest,
            &resolution.authorized_at_unix_ms.to_le_bytes(),
            resolution.attestor_handle.as_bytes(),
            &resolution.attestor_revision.to_le_bytes(),
            &resolution.attestor_policy_digest,
            &resolution.attestor_public_key,
        ],
    )
}
fn verify_dead_letter_resolution_signature(
    resolution: &ModerationDeadLetterResolutionV1,
    signature: [u8; 64],
) -> Result<(), ModerationOrchestratorError> {
    validate_dead_letter_resolution_shape(resolution)?;
    let public_key = PublicKey::from_bytes(Algorithm::Ed25519, &resolution.attestor_public_key)
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    let signature = IrohaSignature::try_from_bytes(&signature)
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    signature
        .verify(&public_key, &dead_letter_resolution_message(resolution))
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)
}
fn unresolved_dead_letter_record_digest(
    state: &ModerationOrchestratorCheckpointV1,
    identity: [u8; 32],
    kind: ModerationDeadLetterKindV1,
) -> Result<[u8; 32], ModerationOrchestratorError> {
    match kind {
        ModerationDeadLetterKindV1::PanelNotification => state
            .panel_notifications
            .iter()
            .find(|entry| {
                entry.notification.notification_id == identity
                    && entry.state == StoredPanelNotificationStateV1::DeadLetter
            })
            .map(|entry| entry.record_digest)
            .ok_or(ModerationOrchestratorError::PanelNotificationArchiveRejected),
        ModerationDeadLetterKindV1::NativeSubmission => state
            .dead_letters
            .iter()
            .find(|entry| {
                entry.identity == identity
                    && entry.resolution.is_none()
                    && matches!(
                        entry.redrive,
                        Some(StoredDeadLetterRedriveV1::NativeSubmission { .. })
                    )
            })
            .ok_or(ModerationOrchestratorError::PanelNotificationArchiveRejected)
            .and_then(durable_dead_letter_source_record_digest),
        ModerationDeadLetterKindV1::TerminalHandoff => state
            .dead_letters
            .iter()
            .find(|entry| {
                entry.identity == identity
                    && entry.resolution.is_none()
                    && matches!(
                        entry.redrive,
                        Some(StoredDeadLetterRedriveV1::TerminalHandoff(_))
                    )
            })
            .ok_or(ModerationOrchestratorError::PanelNotificationArchiveRejected)
            .and_then(durable_dead_letter_source_record_digest),
    }
}
fn dead_letter_redrive_is_available(
    state: &ModerationOrchestratorCheckpointV1,
    identity: [u8; 32],
    kind: ModerationDeadLetterKindV1,
) -> bool {
    match kind {
        ModerationDeadLetterKindV1::PanelNotification => {
            state.panel_notifications.iter().any(|entry| {
                entry.notification.notification_id == identity
                    && entry.state == StoredPanelNotificationStateV1::DeadLetter
            })
        }
        ModerationDeadLetterKindV1::NativeSubmission => state.dead_letters.iter().any(|entry| {
            entry.identity == identity
                && entry.resolution.is_none()
                && matches!(
                    entry.redrive,
                    Some(StoredDeadLetterRedriveV1::NativeSubmission { .. })
                )
        }),
        ModerationDeadLetterKindV1::TerminalHandoff => state.dead_letters.iter().any(|entry| {
            entry.identity == identity
                && entry.resolution.is_none()
                && matches!(
                    entry.redrive,
                    Some(StoredDeadLetterRedriveV1::TerminalHandoff(_))
                )
        }),
    }
}
fn unresolved_dead_letter_incident_time(
    state: &ModerationOrchestratorCheckpointV1,
    identity: [u8; 32],
    kind: ModerationDeadLetterKindV1,
) -> Result<u64, ModerationOrchestratorError> {
    match kind {
        ModerationDeadLetterKindV1::PanelNotification => state
            .panel_notifications
            .iter()
            .find(|entry| {
                entry.notification.notification_id == identity
                    && entry.state == StoredPanelNotificationStateV1::DeadLetter
            })
            .and_then(|entry| entry.dead_lettered_at_unix_ms)
            .ok_or(ModerationOrchestratorError::PanelNotificationArchiveRejected),
        ModerationDeadLetterKindV1::NativeSubmission
        | ModerationDeadLetterKindV1::TerminalHandoff => state
            .dead_letters
            .iter()
            .find(|entry| entry.identity == identity && entry.resolution.is_none())
            .map(|entry| entry.dead_lettered_at_unix_ms)
            .filter(|timestamp| *timestamp != 0)
            .ok_or(ModerationOrchestratorError::PanelNotificationArchiveRejected),
    }
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
    for cursor in [
        state.terminal_handoff_scanned_cursor,
        state.terminal_handoff_archived_cursor,
    ] {
        match cursor {
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
    }
    match state
        .panel_notification_archive_compaction_reservation
        .as_ref()
    {
        Some(payload) => {
            hasher.update(&[1]);
            match panel_notification_archive_payload_digest(payload) {
                Ok(digest) => {
                    hasher.update(&digest);
                }
                Err(_) => {
                    hasher.update(&[0; 32]);
                }
            }
        }
        None => {
            hasher.update(&[0]);
        }
    }
    hasher.update(
        &state
            .panel_notification_archived_dead_letter_count
            .to_le_bytes(),
    );
    hasher.update(
        &u64::try_from(state.panel_notification_archive_signer_epochs.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for epoch in &state.panel_notification_archive_signer_epochs {
        hasher.update(&epoch.epoch.to_le_bytes());
        hasher.update(&epoch.epoch_digest);
    }
    match state.panel_notification_archive_head.as_ref() {
        Some(head) => {
            hasher.update(&[1]);
            hasher.update(&head.head_digest);
            hasher.update(&head.operation_id);
            hasher.update(&head.chain_commitment);
        }
        None => {
            hasher.update(&[0]);
        }
    }
    for publication_head in [
        state
            .panel_notification_archive_pending_publication
            .as_ref(),
        state.panel_notification_archive_published_head.as_ref(),
    ] {
        match publication_head {
            Some(head) => {
                hasher.update(&[1]);
                hasher.update(&head.generation.to_le_bytes());
                hasher.update(&head.head_digest);
                hasher.update(&head.operation_id);
                hasher.update(&head.chain_commitment);
            }
            None => {
                hasher.update(&[0]);
            }
        }
    }
    match state.panel_notification_archive_audit_cursor.as_ref() {
        Some(cursor) => {
            hasher.update(&[1]);
            hasher.update(&cursor.target_generation.to_le_bytes());
            hasher.update(&cursor.target_head_digest);
            hash_optional_archive_digest(&mut hasher, cursor.next_operation_id);
            match cursor.expected_generation {
                Some(generation) => {
                    hasher.update(&[1]);
                    hasher.update(&generation.to_le_bytes());
                }
                None => {
                    hasher.update(&[0]);
                }
            }
            hash_optional_archive_digest(&mut hasher, cursor.expected_head_digest);
            hash_optional_archive_digest(&mut hasher, cursor.expected_chain_commitment);
            hasher.update(&cursor.verified_head_count.to_le_bytes());
            hasher.update(&cursor.chain_commitment);
            hasher.update(&cursor.last_completed_generation.to_le_bytes());
            hash_optional_archive_digest(&mut hasher, cursor.last_completed_head_digest);
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
    hasher.update(
        &u64::try_from(state.panel_notification_dead_letter_resolutions.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for entry in &state.panel_notification_dead_letter_resolutions {
        hasher.update(&entry.record_digest);
        hasher.update(&dead_letter_resolution_message(&entry.resolution));
    }
    *hasher.finalize().as_bytes()
}
fn refresh_panel_notification_outbox_digest(state: &mut ModerationOrchestratorCheckpointV1) {
    state.panel_notification_outbox_digest = panel_notification_outbox_digest(state);
}
include!("moderation_orchestrator/panel_notification_archive_lineage.rs");
fn panel_notification_archive_decode_limits(max_bytes: usize, max_records: usize) -> DecodeLimits {
    DecodeLimits::new(
        max_records.max(MODERATION_PANEL_NOTIFICATION_ARCHIVE_MAX_SIGNER_EPOCHS_V1),
        max_bytes,
        max_bytes.saturating_mul(2),
        max_bytes.saturating_mul(2),
        128,
    )
}
fn verify_panel_notification_archive_artifact_with_bounds(
    max_bytes: usize,
    _checkpoint_max_bytes: u64,
    max_records: usize,
    network_id: &iroha_data_model::NetworkId,
    bytes: &[u8],
) -> Result<ModerationPanelNotificationArchiveArtifactV1, ModerationOrchestratorError> {
    if bytes.is_empty() || bytes.len() > max_bytes || max_records == 0 {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    let artifact = decode_from_bytes_with_limits::<ModerationPanelNotificationArchiveArtifactV1>(
        bytes,
        panel_notification_archive_decode_limits(max_bytes, max_records),
    )
    .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    let canonical = norito::to_bytes(&artifact)
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    if canonical != bytes {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    verify_panel_notification_archive_head_core(&artifact.head)?;
    let terminal_record_count = u32::try_from(artifact.payload.records.len())
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    let dead_letter_record_count = u32::try_from(
        artifact
            .payload
            .records
            .iter()
            .filter(|record| terminal_archive_record_is_dead_letter(record))
            .count(),
    )
    .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    let source_manifest = &artifact.source_manifest;
    if artifact.version != MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1
        || artifact.payload.version != MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1
        || artifact.head.network_id != *network_id
        || artifact.head.archive_signature != [0; 64]
        || terminal_record_count == 0
        || terminal_record_count != artifact.head.terminal_record_count
        || dead_letter_record_count != artifact.head.dead_letter_record_count
        || usize::try_from(terminal_record_count).unwrap_or(usize::MAX) > max_records
        || artifact
            .payload
            .records
            .first()
            .map(terminal_archive_record_boundary_id)
            .transpose()?
            != Some(artifact.head.first_notification_id)
        || artifact
            .payload
            .records
            .last()
            .map(terminal_archive_record_boundary_id)
            .transpose()?
            != Some(artifact.head.last_notification_id)
        || panel_notification_archive_payload_digest(&artifact.payload)?
            != artifact.head.payload_digest
        || source_manifest.version != MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1
        || source_manifest.network_id != *network_id
        || source_manifest.checkpoint_namespace_digest
            != artifact.head.source_checkpoint_namespace_digest
        || source_manifest.checkpoint_generation != artifact.head.source_checkpoint_generation
        || source_manifest.checkpoint_revision != artifact.head.source_checkpoint_revision
        || source_manifest.checkpoint_digest != artifact.head.source_checkpoint_digest
        || panel_notification_archive_source_manifest_digest(source_manifest)?
            != artifact.head.source_manifest_digest
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    let mut previous_key = None;
    let mut terminal_groups = BTreeMap::<[u8; 32], (usize, BTreeSet<u8>)>::new();
    for record in &artifact.payload.records {
        let key = terminal_archive_record_key(record)?;
        if previous_key
            .as_ref()
            .is_some_and(|previous| previous >= &key)
            || validate_terminal_archive_record_shape(record).is_err()
        {
            return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
        }
        if key.0 == 4 {
            let kind = match record {
                ModerationTerminalArchiveRecordV1::CompletedHandoff { kind, .. }
                | ModerationTerminalArchiveRecordV1::DurableDeadLetter {
                    handoff_kind: Some(kind),
                    ..
                } => *kind,
                _ => return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid),
            };
            let group = terminal_groups.entry(key.1).or_default();
            group.0 = group.0.saturating_add(1);
            group.1.insert(match kind {
                ModerationTerminalHandoffKindV1::Settlement => 0,
                ModerationTerminalHandoffKindV1::Publication => 1,
            });
        }
        previous_key = Some(key);
    }
    if terminal_groups
        .values()
        .any(|(count, kinds)| *count != 2 || kinds.len() != 2)
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    let statement = panel_notification_source_attestation_from_head(&artifact.head);
    if statement.terminal_set_digest != artifact.head.payload_digest
        || statement.terminal_record_count != terminal_record_count
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    let source_bootstrap_public_key = source_manifest
        .archive_signer_epochs
        .first()
        .map(|epoch| epoch.archive_public_key)
        .ok_or(ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    validate_panel_notification_archive_signer_epochs(
        &source_manifest.archive_signer_epochs,
        network_id,
        source_bootstrap_public_key,
        artifact.head.archive_id,
    )?;
    if source_manifest
        .archive_signer_epochs
        .last()
        .map(|epoch| epoch.epoch)
        != Some(artifact.head.archive_signer_epoch)
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    verify_panel_notification_archive_head_signer_epoch(
        &artifact.head,
        &source_manifest.archive_signer_epochs,
    )?;
    let source_predecessor_matches = match (
        artifact.head.generation,
        source_manifest.predecessor_archive_head.as_ref(),
    ) {
        (1, None) => true,
        (2.., Some(predecessor)) => {
            verify_panel_notification_archive_head(predecessor).is_ok()
                && verify_panel_notification_archive_head_signer_epoch(
                    predecessor,
                    &source_manifest.archive_signer_epochs,
                )
                .is_ok()
                && verify_panel_notification_archive_lineage_link(&artifact.head, predecessor)
                    .is_ok()
        }
        _ => false,
    };
    if !source_predecessor_matches {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    Ok(artifact)
}
/// Strictly validate a canonical archive artifact at the dedicated slot-55 broker boundary.
///
/// All signable values are derived internally from canonical bytes. The caller cannot supply
/// an alternative operation identifier, receipt message, network binding, or source claim.
///
/// # Errors
///
/// Rejects oversized/noncanonical bytes, malformed terminal membership, stale or substituted
/// provider identities, invalid source attestations, chain forks/gaps, and signing-oracle input.
pub fn validate_moderation_panel_notification_archive_artifact_for_broker_v1(
    canonical_artifact: &[u8],
    expected: &ModerationPanelNotificationArchiveBrokerExpectationV1<'_>,
) -> Result<ModerationPanelNotificationArchiveBrokerValidationV1, ModerationOrchestratorError> {
    let archive_max_bytes = usize::try_from(expected.archive_max_bytes)
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    let artifact = verify_panel_notification_archive_artifact_with_bounds(
        archive_max_bytes,
        expected.checkpoint_max_bytes,
        expected.max_records,
        expected.network_id,
        canonical_artifact,
    )?;
    verify_panel_notification_archive_head_core_is_current(
        &artifact.head,
        expected.archive_handle,
        expected.archive_qualification,
        expected.archive_id,
        expected.archive_public_key,
    )?;
    validate_moderation_panel_notification_archive_artifact_source_for_broker_v1(
        &artifact, expected,
    )?;
    Ok(ModerationPanelNotificationArchiveBrokerValidationV1 {
        operation_id: artifact.head.operation_id,
        receipt_message: panel_notification_archive_receipt_message(&artifact.head),
        archive_public_key: artifact.head.archive_public_key,
        head_digest: artifact.head.head_digest,
        chain_commitment: artifact.head.chain_commitment,
        generation: artifact.head.generation,
        source_attestation_digest: artifact.head.source_attestation_digest,
    })
}
fn validate_moderation_panel_notification_archive_artifact_source_for_broker_v1(
    artifact: &ModerationPanelNotificationArchiveArtifactV1,
    expected: &ModerationPanelNotificationArchiveBrokerExpectationV1<'_>,
) -> Result<(), ModerationOrchestratorError> {
    if artifact.head.archive_id != expected.archive_id
        || artifact.head.source_checkpoint_namespace_digest
            != checkpoint_store::checkpoint_namespace(expected.network_id)
        || artifact.head.source_attestor_handle != expected.checkpoint_handle
        || artifact.head.source_attestor_revision != expected.checkpoint_qualification.revision()
        || artifact.head.source_attestor_policy_digest
            != expected.checkpoint_qualification.policy_digest()
        || artifact.head.source_attestor_public_key != expected.checkpoint_attestation_public_key
        || artifact
            .source_manifest
            .archive_signer_epochs
            .first()
            .map(|epoch| epoch.archive_public_key)
            != Some(expected.archive_bootstrap_public_key)
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    Ok(())
}
/// Validate an installed historical archive artifact without pinning it to the current signer.
///
/// Historical signer bindings are authenticated by the minimal embedded epoch
/// log, anchored to `archive_bootstrap_public_key` and the stable archive id.
/// The checkpoint-attestor binding is intentionally archive-lifetime-stable in
/// V1 and remains pinned exactly.
///
/// # Errors
///
/// Rejects malformed bytes, a signer epoch not rooted in the configured
/// bootstrap key, a substituted predecessor or archive id, or any change to the
/// stable checkpoint-attestor trust anchor.
pub fn validate_moderation_panel_notification_archive_readback_for_broker_v1(
    canonical_artifact: &[u8],
    expected: &ModerationPanelNotificationArchiveBrokerExpectationV1<'_>,
) -> Result<ModerationPanelNotificationArchiveBrokerValidationV1, ModerationOrchestratorError> {
    let archive_max_bytes = usize::try_from(expected.archive_max_bytes)
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    let artifact = verify_panel_notification_archive_artifact_with_bounds(
        archive_max_bytes,
        expected.checkpoint_max_bytes,
        expected.max_records,
        expected.network_id,
        canonical_artifact,
    )?;
    validate_moderation_panel_notification_archive_artifact_source_for_broker_v1(
        &artifact, expected,
    )?;
    Ok(ModerationPanelNotificationArchiveBrokerValidationV1 {
        operation_id: artifact.head.operation_id,
        receipt_message: panel_notification_archive_receipt_message(&artifact.head),
        archive_public_key: artifact.head.archive_public_key,
        head_digest: artifact.head.head_digest,
        chain_commitment: artifact.head.chain_commitment,
        generation: artifact.head.generation,
        source_attestation_digest: artifact.head.source_attestation_digest,
    })
}
/// Strictly validate canonical signed archive-head bytes for the existing slot-20
/// `ModerationPublicationHandoff` broker op116.
///
/// # Errors
///
/// Rejects noncanonical bytes, any network/provider/source substitution, invalid source or
/// archive signatures, and inconsistent operation, head, or chain-accumulator derivations.
pub fn validate_moderation_panel_notification_archive_head_for_broker_v1(
    canonical_head: &[u8],
    expected: &ModerationPanelNotificationArchiveBrokerExpectationV1<'_>,
) -> Result<
    (
        ModerationPanelNotificationArchiveHeadV1,
        ModerationPanelNotificationArchiveBrokerValidationV1,
    ),
    ModerationOrchestratorError,
> {
    let max_bytes = usize::try_from(expected.archive_max_bytes)
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    if canonical_head.is_empty() || canonical_head.len() > max_bytes {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    let head = decode_from_bytes_with_limits::<ModerationPanelNotificationArchiveHeadV1>(
        canonical_head,
        DecodeLimits::new(max_bytes, 16, max_bytes, max_bytes, 32),
    )
    .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    if norito::to_bytes(&head)
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?
        != canonical_head
        || head.network_id != *expected.network_id
        || head.source_checkpoint_namespace_digest
            != checkpoint_store::checkpoint_namespace(expected.network_id)
        || head.source_attestor_handle != expected.checkpoint_handle
        || head.source_attestor_revision != expected.checkpoint_qualification.revision()
        || head.source_attestor_policy_digest != expected.checkpoint_qualification.policy_digest()
        || head.source_attestor_public_key != expected.checkpoint_attestation_public_key
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    verify_panel_notification_archive_head_is_current(
        &head,
        expected.archive_handle,
        expected.archive_qualification,
        expected.archive_id,
        expected.archive_public_key,
    )?;
    let validation = ModerationPanelNotificationArchiveBrokerValidationV1 {
        operation_id: head.operation_id,
        receipt_message: panel_notification_archive_receipt_message(&head),
        archive_public_key: head.archive_public_key,
        head_digest: head.head_digest,
        chain_commitment: head.chain_commitment,
        generation: head.generation,
        source_attestation_digest: head.source_attestation_digest,
    };
    Ok((head, validation))
}
/// Build one deterministic generation-one archive fixture for broker protocol tests.
///
/// The fixed seeds are test material, never production credentials. Every signature,
/// source claim, terminal record, operation, and chain commitment is produced by the
/// same implementation used in production validation.
#[doc(hidden)]
pub fn moderation_panel_notification_archive_broker_fixture_v1()
-> Result<ModerationPanelNotificationArchiveBrokerFixtureV1, ModerationOrchestratorError> {
    fn public_key_bytes(key: &KeyPair) -> Result<[u8; 32], ModerationOrchestratorError> {
        key.public_key()
            .to_bytes()
            .1
            .try_into()
            .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)
    }
    fn sign_message(
        key: &KeyPair,
        message: [u8; 32],
    ) -> Result<[u8; 64], ModerationOrchestratorError> {
        IrohaSignature::try_new(key.private_key(), &message)
            .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?
            .payload()
            .try_into()
            .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)
    }
    let archive_signing_seed = [0xA9; 32];
    let checkpoint_attestation_signing_seed = [0xC9; 32];
    let archive_key = KeyPair::try_from_seed(archive_signing_seed.to_vec(), Algorithm::Ed25519)
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    let checkpoint_key = KeyPair::try_from_seed(
        checkpoint_attestation_signing_seed.to_vec(),
        Algorithm::Ed25519,
    )
    .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    let archive_public_key = public_key_bytes(&archive_key)?;
    let checkpoint_attestation_public_key = public_key_bytes(&checkpoint_key)?;
    let network_id = iroha_data_model::NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"moderation-archive-broker-fixture-v1-genesis",
        )),
    );
    let archive_handle = "immutable.moderation-archive.fixture-v1".to_owned();
    let archive_qualification = ModerationRuntimeProviderQualificationV1::new(7, [0x71; 32]);
    let archive_id = [0xA7; 32];
    let checkpoint_handle = "sealed-cas.moderation-checkpoint.fixture-v1".to_owned();
    let checkpoint_qualification = ModerationRuntimeProviderQualificationV1::new(9, [0x91; 32]);
    let cursor = ModerationFinalizedEventCursorV1 {
        sequence: 1,
        block_height: 1,
        block_hash: [0xB1; 32],
        event_index: 0,
    };
    let recipient = AccountId::new(checkpoint_key.public_key().clone());
    let notification_id = panel_notification_id(
        &network_id,
        [0x31; 32],
        [0x32; 32],
        ModerationPanelNotificationKindV1::PrimaryAssignment,
        &recipient,
        cursor,
        1_000,
    );
    let worker_id = [0x41; 32];
    let claimed_at_unix_ms = 1_100;
    let lease_expires_at_unix_ms = claimed_at_unix_ms + MODERATION_PANEL_NOTIFICATION_LEASE_MS_V1;
    let lease_token = panel_notification_lease_token(
        notification_id,
        worker_id,
        1,
        1,
        claimed_at_unix_ms,
        lease_expires_at_unix_ms,
    );
    let mut stored = StoredPanelNotificationV1 {
        notification: ModerationPanelNotificationV1 {
            notification_id,
            network_id,
            source_operation_id: [0x31; 32],
            scope_digest: [0x32; 32],
            kind: ModerationPanelNotificationKindV1::PrimaryAssignment,
            recipient,
            finalized_event_cursor: cursor,
            source_occurred_at_unix_ms: 1_000,
        },
        attempt_limit: 3,
        attempts: 1,
        claim_generation: 1,
        available_at_unix_ms: 1_000,
        state: StoredPanelNotificationStateV1::Delivered,
        claimed_by: Some(worker_id),
        lease_token: Some(lease_token),
        claimed_at_unix_ms: Some(claimed_at_unix_ms),
        lease_expires_at_unix_ms: Some(lease_expires_at_unix_ms),
        receipt_digest: Some([0x51; 32]),
        delivered_at_unix_ms: Some(1_200),
        dead_letter_reason: None,
        dead_lettered_at_unix_ms: None,
        record_digest: [0; 32],
    };
    refresh_panel_notification_record_digest(&mut stored);
    let second_cursor = ModerationFinalizedEventCursorV1 {
        sequence: 2,
        block_height: 1,
        block_hash: [0xB1; 32],
        event_index: 1,
    };
    let second_notification_id = panel_notification_id(
        &network_id,
        [0x33; 32],
        [0x34; 32],
        ModerationPanelNotificationKindV1::PrimaryAssignment,
        &stored.notification.recipient,
        second_cursor,
        1_001,
    );
    let mut second_stored = stored.clone();
    second_stored.notification.notification_id = second_notification_id;
    second_stored.notification.source_operation_id = [0x33; 32];
    second_stored.notification.scope_digest = [0x34; 32];
    second_stored.notification.finalized_event_cursor = second_cursor;
    second_stored.notification.source_occurred_at_unix_ms = 1_001;
    second_stored.available_at_unix_ms = 1_001;
    second_stored.lease_token = Some(panel_notification_lease_token(
        second_notification_id,
        worker_id,
        1,
        1,
        claimed_at_unix_ms,
        lease_expires_at_unix_ms,
    ));
    second_stored.receipt_digest = Some([0x52; 32]);
    refresh_panel_notification_record_digest(&mut second_stored);
    let mut signer_epoch = ModerationPanelNotificationArchiveSignerEpochV1 {
        version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
        epoch: 1,
        activated_at_generation: 1,
        archive_id,
        archive_handle: archive_handle.clone(),
        archive_revision: archive_qualification.revision(),
        archive_policy_digest: archive_qualification.policy_digest(),
        archive_public_key,
        predecessor_epoch_digest: None,
        predecessor_revocation_generation: None,
        predecessor_authorization_signature: None,
        new_key_possession_signature: None,
        epoch_digest: [0; 32],
    };
    signer_epoch.epoch_digest = panel_notification_archive_signer_epoch_digest(&signer_epoch);
    let mut source = ModerationOrchestratorCheckpointV1::new(&network_id);
    source.generation = 1;
    source.panel_notification_clock_unix_ms = 1_200;
    source.panel_notification_scanned_cursor = Some(second_cursor);
    source.panel_notification_archive_signer_epochs = vec![signer_epoch.clone()];
    source.panel_notifications = vec![stored, second_stored];
    source
        .panel_notifications
        .sort_by_key(|entry| entry.notification.notification_id);
    let payload = ModerationPanelNotificationArchivePayloadV1 {
        version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
        records: collect_terminal_archive_records(&source)?,
    };
    source.panel_notification_archive_compaction_reservation = Some(payload.clone());
    refresh_panel_notification_outbox_digest(&mut source);
    let source_checkpoint_bytes = norito::to_bytes(&source)
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    let source_checkpoint_digest = domain_hash(
        b"sorafs.moderation.checkpoint-bytes.v1",
        &[&source_checkpoint_bytes],
    );
    let mut current_checkpoint_record = ModerationCheckpointStoreRecordV1 {
        version: MODERATION_CHECKPOINT_STORE_RECORD_VERSION_V1,
        network_id,
        namespace_digest: checkpoint_store::checkpoint_namespace(&network_id),
        checkpoint_generation: 1,
        predecessor_revision: Some([0x11; 32]),
        predecessor_checkpoint_digest: Some([0x12; 32]),
        checkpoint_digest: source_checkpoint_digest,
        checkpoint_bytes: source_checkpoint_bytes.clone(),
        checkpoint_store_handle: checkpoint_handle.clone(),
        checkpoint_store_revision: checkpoint_qualification.revision(),
        checkpoint_store_policy_digest: checkpoint_qualification.policy_digest(),
        revision: [0; 32],
    };
    current_checkpoint_record.revision =
        checkpoint_store::record_revision(&current_checkpoint_record);
    let source_manifest = ModerationPanelNotificationArchiveSourceManifestV1 {
        version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
        network_id,
        checkpoint_namespace_digest: current_checkpoint_record.namespace_digest,
        checkpoint_generation: current_checkpoint_record.checkpoint_generation,
        checkpoint_revision: current_checkpoint_record.revision,
        checkpoint_digest: current_checkpoint_record.checkpoint_digest,
        archive_signer_epochs: vec![signer_epoch.clone()],
        predecessor_archive_head: None,
    };
    let source_manifest_digest =
        panel_notification_archive_source_manifest_digest(&source_manifest)?;
    let payload_digest = panel_notification_archive_payload_digest(&payload)?;
    let first_boundary_id = terminal_archive_record_boundary_id(&payload.records[0])?;
    let last_boundary_id = terminal_archive_record_boundary_id(
        payload
            .records
            .last()
            .ok_or(ModerationOrchestratorError::PanelNotificationArchiveInvalid)?,
    )?;
    let terminal_record_count = u32::try_from(payload.records.len())
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    let source_attestation = ModerationPanelNotificationSourceAttestationV1 {
        version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
        attestor_slot: MODERATION_PANEL_NOTIFICATION_SOURCE_ATTESTOR_BROKER_SLOT_V1,
        network_id,
        checkpoint_namespace_digest: current_checkpoint_record.namespace_digest,
        checkpoint_generation: current_checkpoint_record.checkpoint_generation,
        checkpoint_revision: current_checkpoint_record.revision,
        checkpoint_digest: current_checkpoint_record.checkpoint_digest,
        source_manifest_digest,
        terminal_set_digest: payload_digest,
        terminal_record_count,
        first_notification_id: first_boundary_id,
        last_notification_id: last_boundary_id,
        attestor_handle: checkpoint_handle.clone(),
        attestor_revision: checkpoint_qualification.revision(),
        attestor_policy_digest: checkpoint_qualification.policy_digest(),
        attestor_public_key: checkpoint_attestation_public_key,
    };
    let source_attestation_signature = sign_message(
        &checkpoint_key,
        panel_notification_source_attestation_message(&source_attestation),
    )?;
    let mut head = ModerationPanelNotificationArchiveHeadV1 {
        version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
        network_id,
        generation: 1,
        predecessor_head_digest: None,
        predecessor_operation_id: None,
        predecessor_chain_commitment: None,
        source_checkpoint_generation: current_checkpoint_record.checkpoint_generation,
        source_checkpoint_namespace_digest: current_checkpoint_record.namespace_digest,
        source_checkpoint_revision: current_checkpoint_record.revision,
        source_checkpoint_digest: current_checkpoint_record.checkpoint_digest,
        source_manifest_digest,
        source_binding_digest: panel_notification_archive_source_binding_digest(
            &source_attestation,
        ),
        source_attestor_handle: checkpoint_handle.clone(),
        source_attestor_revision: checkpoint_qualification.revision(),
        source_attestor_policy_digest: checkpoint_qualification.policy_digest(),
        source_attestor_public_key: checkpoint_attestation_public_key,
        source_attestation_digest: panel_notification_source_attestation_message(
            &source_attestation,
        ),
        source_attestation_signature,
        terminal_record_count,
        dead_letter_record_count: 0,
        cumulative_dead_letter_count: 0,
        first_notification_id: first_boundary_id,
        last_notification_id: last_boundary_id,
        payload_digest,
        archive_handle: archive_handle.clone(),
        archive_revision: archive_qualification.revision(),
        archive_policy_digest: archive_qualification.policy_digest(),
        archive_id,
        archive_public_key,
        archive_signer_epoch: signer_epoch.epoch,
        archive_signer_epoch_digest: signer_epoch.epoch_digest,
        operation_id: [0; 32],
        head_digest: [0; 32],
        chain_commitment: [0; 32],
        archive_signature: [0; 64],
    };
    head.operation_id = panel_notification_archive_operation_id(&head);
    head.head_digest = panel_notification_archive_head_digest(&head);
    head.chain_commitment = panel_notification_archive_chain_commitment(&head);
    let artifact = ModerationPanelNotificationArchiveArtifactV1 {
        version: MODERATION_PANEL_NOTIFICATION_ARCHIVE_VERSION_V1,
        head: head.clone(),
        source_manifest,
        payload,
    };
    let canonical_artifact = norito::to_bytes(&artifact)
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    let archive_signature = sign_message(
        &archive_key,
        panel_notification_archive_receipt_message(&head),
    )?;
    head.archive_signature = archive_signature;
    let canonical_signed_head = norito::to_bytes(&head)
        .map_err(|_| ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    let validation = ModerationPanelNotificationArchiveBrokerValidationV1 {
        operation_id: head.operation_id,
        receipt_message: panel_notification_archive_receipt_message(&head),
        archive_public_key: head.archive_public_key,
        head_digest: head.head_digest,
        chain_commitment: head.chain_commitment,
        generation: head.generation,
        source_attestation_digest: head.source_attestation_digest,
    };
    Ok(ModerationPanelNotificationArchiveBrokerFixtureV1 {
        network_id,
        archive_handle,
        archive_qualification,
        archive_id,
        archive_public_key,
        archive_signing_seed,
        checkpoint_handle,
        checkpoint_qualification,
        checkpoint_attestation_public_key,
        checkpoint_attestation_signing_seed,
        current_checkpoint_record,
        source_attestation,
        canonical_artifact,
        archive_signature,
        canonical_signed_head,
        validation,
        checkpoint_max_bytes: MODERATION_ORCHESTRATOR_CHECKPOINT_MAX_BYTES_V1,
        archive_max_bytes: MODERATION_ORCHESTRATOR_CHECKPOINT_MAX_BYTES_V1
            + MODERATION_PANEL_NOTIFICATION_ARCHIVE_WRAPPER_MAX_BYTES_V1,
    })
}
fn verify_panel_notification_archive_artifact(
    config: &ModerationOrchestratorConfigV1,
    network_id: &iroha_data_model::NetworkId,
    bytes: &[u8],
) -> Result<ModerationPanelNotificationArchiveArtifactV1, ModerationOrchestratorError> {
    let max_bytes = usize::try_from(config.panel_notification_archive_max_bytes).map_err(|_| {
        ModerationOrchestratorError::InvalidConfiguration(
            "notification archive byte limit does not fit usize".to_owned(),
        )
    })?;
    let artifact = verify_panel_notification_archive_artifact_with_bounds(
        max_bytes,
        config.checkpoint_max_bytes,
        config.max_handoffs,
        network_id,
        bytes,
    )?;
    if artifact.head.archive_id != config.panel_notification_archive_id
        || artifact.head.source_checkpoint_namespace_digest
            != checkpoint_store::checkpoint_namespace(network_id)
        || artifact.head.source_attestor_handle != config.checkpoint_store_handle
        || artifact.head.source_attestor_revision
            != config.expected_checkpoint_store_qualification.revision()
        || artifact.head.source_attestor_policy_digest
            != config
                .expected_checkpoint_store_qualification
                .policy_digest()
        || artifact.head.source_attestor_public_key
            != config.checkpoint_store_attestation_public_key
        || artifact
            .source_manifest
            .archive_signer_epochs
            .first()
            .map(|epoch| epoch.archive_public_key)
            != Some(config.panel_notification_archive_bootstrap_public_key)
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    Ok(artifact)
}
fn verify_panel_notification_archive_readback(
    config: &ModerationOrchestratorConfigV1,
    network_id: &iroha_data_model::NetworkId,
    readback: &ModerationPanelNotificationArchiveReadbackV1,
) -> Result<
    (
        ModerationPanelNotificationArchiveArtifactV1,
        ModerationPanelNotificationArchiveHeadV1,
    ),
    ModerationOrchestratorError,
> {
    let artifact = verify_panel_notification_archive_artifact(
        config,
        network_id,
        &readback.canonical_artifact,
    )?;
    let mut head = artifact.head.clone();
    head.archive_signature = readback.signature;
    verify_panel_notification_archive_head(&head)?;
    Ok((artifact, head))
}
fn load_verified_panel_notification_archive_head(
    config: &ModerationOrchestratorConfigV1,
    network_id: &iroha_data_model::NetworkId,
    archive: &QualifiedModerationPanelNotificationArchiveV1,
    operation_id: [u8; 32],
) -> Result<ModerationPanelNotificationArchiveHeadV1, ModerationOrchestratorError> {
    let readback = archive
        .read(operation_id)
        .map_err(map_panel_notification_archive_error)?
        .ok_or(ModerationOrchestratorError::PanelNotificationArchiveInvalid)?;
    let (_, head) = verify_panel_notification_archive_readback(config, network_id, &readback)?;
    if head.operation_id != operation_id {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    Ok(head)
}
fn verify_current_panel_notification_archive_readback(
    config: &ModerationOrchestratorConfigV1,
    network_id: &iroha_data_model::NetworkId,
    archive: &QualifiedModerationPanelNotificationArchiveV1,
    head: Option<&ModerationPanelNotificationArchiveHeadV1>,
) -> Result<(), ModerationOrchestratorError> {
    let Some(head) = head else {
        return Ok(());
    };
    verify_panel_notification_archive_head(head)?;
    let installed = load_verified_panel_notification_archive_head(
        config,
        network_id,
        archive,
        head.operation_id,
    )?;
    if &installed != head {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid);
    }
    match (head.generation, head.predecessor_operation_id) {
        (1, None) => Ok(()),
        (2.., Some(predecessor_operation_id)) => {
            let predecessor = load_verified_panel_notification_archive_head(
                config,
                network_id,
                archive,
                predecessor_operation_id,
            )?;
            verify_panel_notification_archive_lineage_link(head, &predecessor)
        }
        _ => Err(ModerationOrchestratorError::PanelNotificationArchiveInvalid),
    }
}
fn map_panel_notification_archive_error(
    error: ModerationPanelNotificationArchiveExternalErrorV1,
) -> ModerationOrchestratorError {
    match error {
        ModerationPanelNotificationArchiveExternalErrorV1::Unavailable => {
            ModerationOrchestratorError::PanelNotificationArchiveUnavailable
        }
        ModerationPanelNotificationArchiveExternalErrorV1::Ambiguous => {
            ModerationOrchestratorError::PanelNotificationArchiveAmbiguous
        }
        ModerationPanelNotificationArchiveExternalErrorV1::Rejected => {
            ModerationOrchestratorError::PanelNotificationArchiveRejected
        }
    }
}
fn map_checkpoint_store_attestation_error(
    error: ModerationCheckpointStoreExternalErrorV1,
) -> ModerationOrchestratorError {
    match error {
        ModerationCheckpointStoreExternalErrorV1::Unavailable => {
            ModerationOrchestratorError::CheckpointStoreUnavailable
        }
        ModerationCheckpointStoreExternalErrorV1::Rejected => {
            ModerationOrchestratorError::PanelNotificationArchiveRejected
        }
        ModerationCheckpointStoreExternalErrorV1::Ambiguous => {
            ModerationOrchestratorError::CheckpointStoreAmbiguous
        }
    }
}
fn map_panel_notification_archive_publication_error(
    error: ModerationHandoffFailureV1,
) -> ModerationOrchestratorError {
    match error {
        ModerationHandoffFailureV1::NotDelivered => {
            ModerationOrchestratorError::PanelNotificationArchiveUnavailable
        }
        ModerationHandoffFailureV1::Ambiguous => {
            ModerationOrchestratorError::PanelNotificationArchiveAmbiguous
        }
        ModerationHandoffFailureV1::Permanent => {
            ModerationOrchestratorError::PanelNotificationArchiveRejected
        }
    }
}
fn verify_published_panel_notification_archive_head_readback(
    publication_sink: &QualifiedModerationTerminalHandoffSinkV1,
    expected: Option<&ModerationPanelNotificationArchiveHeadV1>,
) -> Result<(), ModerationOrchestratorError> {
    let observed = publication_sink
        .read_panel_notification_archive_head()
        .map_err(map_panel_notification_archive_publication_error)?;
    if observed.as_ref() != expected
        || observed
            .as_ref()
            .is_some_and(|head| verify_panel_notification_archive_head(head).is_err())
    {
        return Err(ModerationOrchestratorError::PanelNotificationArchiveRejected);
    }
    Ok(())
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
    /// The immutable notification-receipt archive is unavailable.
    #[error("moderation panel-notification receipt archive is unavailable")]
    PanelNotificationArchiveUnavailable,
    /// The archive operation has an unresolved commit result.
    #[error("moderation panel-notification receipt archive result is ambiguous")]
    PanelNotificationArchiveAmbiguous,
    /// The archive rejected a stale or substituted transition.
    #[error("moderation panel-notification receipt archive rejected the transition")]
    PanelNotificationArchiveRejected,
    /// Archive bytes, lineage, identity, or signature were invalid.
    #[error("moderation panel-notification receipt archive is invalid")]
    PanelNotificationArchiveInvalid,
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
    /// The authoritative checkpoint store was unavailable.
    #[error("moderation authoritative checkpoint store is unavailable")]
    CheckpointStoreUnavailable,
    /// The authoritative checkpoint result could not be resolved.
    #[error("moderation authoritative checkpoint result is ambiguous")]
    CheckpointStoreAmbiguous,
    /// Another replica committed a different successor.
    #[error("moderation checkpoint writer was fenced by another replica")]
    CheckpointStoreFenced,
    /// The sealed store returned a malformed or equivocal record.
    #[error("moderation authoritative checkpoint store equivocated")]
    CheckpointStoreEquivocation,
    /// Durable generation overflowed.
    #[error("moderation checkpoint generation overflow")]
    GenerationOverflow,
    /// In-memory state lock is poisoned.
    #[error("moderation orchestrator state lock is poisoned")]
    StateLockPoisoned,
    /// In-memory authoritative-record lock is poisoned.
    #[error("moderation orchestrator checkpoint record lock is poisoned")]
    CheckpointStoreLockPoisoned,
}
#[cfg(test)]
mod tests {
    include!("moderation_orchestrator/support_and_provider_tests.rs");
    include!("moderation_orchestrator/network_domain_tests.rs");
    include!("moderation_orchestrator/operation_tests.rs");
    include!("moderation_orchestrator/panel_notification_tests.rs");
}
