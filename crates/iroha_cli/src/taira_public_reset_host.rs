//! Authenticated OpenSSH transport and compiled remote host dispatcher for public Taira reset.

use super::executor_model::{ExecutionStep, RecoveryProgress, ResetTransport};
use super::{
    AdmittedReset, ArtifactV1, AuthorizationEnvelopeV1, EdgeV1, EndpointV1, InventoryV1,
    PinnedArtifact, RecoveryIntentV1, RecoveryMutationStateV1, RecoveryMutationV1, RecoveryOutcome,
    TrustedKeyV1, ValidatorV1, artifact, authorization_semantic_sha256,
    ensure_authorization_current, ensure_pinned_unchanged, now_unix_ms, open_pinned_regular,
    pin_owner_private_file, read_private_json, revalidate_pinned, sha256_hex, validate_inventory,
    validate_owner_private_dir, verify_execution_authorization,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use eyre::{Context as _, Result, eyre};
use iroha::{
    client::{
        AccountFaucetPolicyV1, AccountFaucetPreparedTransactionV1, AccountOnboardingPlanReceiptV1,
        AccountOnboardingPreparedTransactionV1, AccountOnboardingProofRequiredPrepareResponseV1,
        TairaPublicResetMutationBindingV1, verify_account_faucet_prepared_transaction_v1,
        verify_account_onboarding_prepared_transaction_v1,
        verify_account_onboarding_proof_required_result_v1,
    },
    config::{Config as ClientConfig, LoadPath},
    data_model::{
        NetworkId,
        account::{AccountId, address::ChainDiscriminantGuard},
        alias_setup::AccountAliasName,
        asset::AssetDefinitionId,
        block::{BlockHeader, consensus_v2::SumeragiV2Status},
        nexus::FeeSponsorProgramId,
        prelude::{Name, SignedTransaction},
        transaction::FeePaymentIntent,
    },
};
use iroha_crypto::{Hash, HashOf};
use iroha_torii_shared::{
    AccountOnboardingCurrentStateRequestV1, AccountOnboardingCurrentStateResponseV1,
    FeeQuoteResponse,
};
use iroha_version::codec::DecodeVersioned as _;
use norito::json::{self, JsonDeserialize, JsonSerialize};
use reqwest::{
    StatusCode,
    blocking::{Client as HttpClient, Response as HttpResponse},
    header::{ACCEPT, CONTENT_TYPE},
};
use sha2::{Digest as _, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::{OsStr, OsString},
    fs::{self, File, OpenOptions},
    io::{Read, Seek as _, Write},
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    str::FromStr as _,
    time::{Duration, Instant},
};
use url::Url;

#[cfg(unix)]
use std::{
    os::fd::{AsFd as _, AsRawFd as _},
    os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _, symlink},
    os::unix::process::CommandExt as _,
};

#[cfg(target_os = "linux")]
use std::os::unix::ffi::OsStrExt as _;

const HOST_REQUEST_SCHEMA_V1: &str = "iroha.taira.public-reset.host-request.v1";
const HOST_RECEIPT_SCHEMA_V1: &str = "iroha.taira.public-reset.host-receipt.v1";
const HOST_GUARD_SCHEMA_V1: &str = "iroha.taira.public-reset.host-guard.v1";
const LOCAL_RECEIPT_SCHEMA_V1: &str = "iroha.taira.public-reset.local-receipt.v1";
const GENERATED_MARKER_SCHEMA_V1: &str = "iroha.taira.public-reset.generated-path.v1";
#[cfg(any(target_os = "linux", test))]
const CLEANUP_PLAN_SCHEMA_V1: &str = "iroha.taira.public-reset.cleanup-plan.v1";
const LEASE_SCHEMA_V1: &str = "iroha.taira.public-reset.host-lease.v1";
const HOST_INTENT_SCHEMA_V1: &str = "iroha.taira.public-reset.host-intent.v1";
const MANAGER_INTENT_SCHEMA_V1: &str = "iroha.taira.public-reset.manager-intent.v1";
const STATE_MOVE_INTENT_SCHEMA_V1: &str = "iroha.taira.public-reset.state-move-intent.v1";
const HOST_PROGRESS_SCHEMA_V1: &str = "iroha.taira.public-reset.host-progress.v1";
const PREPARED_MUTATION_SCHEMA_V1: &str = "iroha.taira.public-reset.prepared-mutation.v1";
const PREPARED_MUTATION_STATE_SCHEMA_V1: &str =
    "iroha.taira.public-reset.prepared-mutation-state.v1";
const SSH: &str = "/usr/bin/ssh";
const SYSTEMCTL: &str = "/usr/bin/systemctl";
const SYSTEMD_RUN: &str = "/usr/bin/systemd-run";
const NGINX: &str = "/usr/sbin/nginx";
const FIXED_DISPATCHER: &str = "/usr/local/libexec/iroha-taira-public-reset-v1";
const HOST_DISPATCH_SUFFIX: &str = "taira public-reset host-dispatch --protocol v1";
const MAX_PROCESS_OUTPUT: usize = 4 * 1024 * 1024;
const MAX_HOST_REQUEST_BYTES: usize = 32 * 1024 * 1024;
const MAX_PREPARED_ENVELOPE_BYTES: usize = 4 * 1024 * 1024;
const MAX_PREPARED_TRANSACTION_BYTES: usize = 1024 * 1024;
const MAX_REPORT_EVIDENCE_TOKEN_BYTES: usize = 32;
const MAX_PROOF_READ_RESPONSE_BYTES: usize =
    iroha_torii_shared::ACCOUNT_ONBOARDING_CURRENT_STATE_RESPONSE_MAX_BYTES;
const FRAME_LENGTH_BYTES: usize = 8;
const PROCESS_POLL_INTERVAL: Duration = Duration::from_millis(20);
const HOST_ACTION_IN_PROGRESS_MARKER: &str = "iroha.taira.public-reset.host-action-in-progress.v1";
#[cfg(target_os = "linux")]
const MAX_CLEANUP_ENTRIES: usize = 65_536;
#[cfg(any(target_os = "linux", test))]
const MAX_CLEANUP_NAMESPACE_ENTRIES: usize = 4_096;
#[cfg(any(target_os = "linux", test))]
const CLEANUP_TOMBSTONE_PREFIX: &str = ".public-reset-cleanup-v1-";
#[cfg(any(target_os = "linux", test))]
const CLEANUP_TOMBSTONE_SUFFIX: &str = ".tombstone";
const RESET_GENERATED_ENTRIES: [&str; 6] = [
    "storage",
    "snapshots",
    "privacy",
    "torii-data",
    "sorafs-data",
    "inrou-data",
];

#[derive(Debug)]
pub(super) struct LocalMutationRecoveryPending {
    action: &'static str,
}

impl std::fmt::Display for LocalMutationRecoveryPending {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "local mutation `{}` requires exact idempotency recovery before rollback",
            self.action
        )
    }
}

impl std::error::Error for LocalMutationRecoveryPending {}

pub(super) fn is_local_mutation_recovery_pending(error: &eyre::Report) -> bool {
    error
        .downcast_ref::<LocalMutationRecoveryPending>()
        .is_some()
}

/// Hidden remote dispatcher command. Requests are accepted only on stdin.
#[derive(clap::Args, Debug)]
pub(super) struct PublicResetHost {
    /// Internal protocol marker; no request field is accepted through argv.
    #[arg(long, hide = true, value_parser = ["v1"])]
    protocol: String,
}

impl PublicResetHost {
    pub(super) fn run<R: Read, W: Write>(&self, mut input: R, mut output: W) -> Result<()> {
        if self.protocol != "v1" {
            return Err(eyre!("public-reset host protocol must be exact V1"));
        }
        let mut length = [0_u8; FRAME_LENGTH_BYTES + 1];
        input
            .read_exact(&mut length)
            .wrap_err("public-reset request frame length is truncated")?;
        if length[FRAME_LENGTH_BYTES] != b'\n'
            || !length[..FRAME_LENGTH_BYTES]
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
        {
            return Err(eyre!(
                "public-reset request frame length is not canonical hex"
            ));
        }
        let length = usize::from_str_radix(std::str::from_utf8(&length[..FRAME_LENGTH_BYTES])?, 16)
            .wrap_err("public-reset request frame length is invalid")?;
        if length == 0 || length > MAX_HOST_REQUEST_BYTES {
            return Err(eyre!("public-reset host request exceeds the V1 byte limit"));
        }
        let mut bytes = vec![0_u8; length];
        input
            .read_exact(&mut bytes)
            .wrap_err("public-reset host request frame is truncated")?;
        let receipt = dispatch_host_request(&bytes, &mut input)?;
        let rendered = json::to_json(&receipt).wrap_err("failed to encode host receipt")?;
        output
            .write_all(rendered.as_bytes())
            .and_then(|()| output.write_all(b"\n"))
            .wrap_err("failed to write host receipt")
    }
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct HostRequestV1 {
    schema: String,
    action: String,
    recovery_only: bool,
    host_slug: String,
    inventory_base64: String,
    authorization_base64: String,
    authorization_semantic_sha256: String,
    trusted_key_base64: String,
    trusted_key_sha256: String,
    action_deadline_unix_ms: u64,
    artifact_role: String,
    artifact_sha256: String,
    artifact_size: u64,
    artifact_mode: u16,
    mutation_kind: String,
    mutation_phase: String,
    mutation_idempotency_key: String,
    mutation_operation: String,
    mutation_prepared_base64: String,
    mutation_prepared_sha256: String,
    mutation_transaction_hash: String,
    mutation_evidence_base64: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct HostReceiptV1 {
    schema: String,
    action: String,
    host_slug: String,
    request_sha256: String,
    inventory_sha256: String,
    authorization_sha256: String,
    authorization_nonce: String,
    status: String,
    idempotent: bool,
    bytes_before: u64,
    bytes_after: u64,
    reclaimed_bytes: u64,
    detail: String,
    mutation_state: String,
    mutation_prepared_base64: String,
    mutation_prepared_sha256: String,
    mutation_transaction_hash: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct HostGuardV1 {
    schema: String,
    host_slug: String,
    service_root: String,
    state_root: String,
    trusted_key_sha256: String,
    dispatcher_path: String,
    dispatcher_sha256: String,
    upload_parent: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct HostLeaseV1 {
    schema: String,
    inventory_sha256: String,
    authorization_semantic_sha256: String,
    authorization_nonce: String,
    execution_expires_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct HostActionKeyV1 {
    host_slug: String,
    action: String,
    artifact_role: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct HostProgressV1 {
    schema: String,
    inventory_sha256: String,
    authorization_sha256: String,
    authorization_nonce: String,
    next_forward_ordinal: u16,
    #[norito(required)]
    prepared_action: Option<HostActionKeyV1>,
    touched_hosts: Vec<String>,
    sealed: bool,
    rolling_back: bool,
    last_rollback_rank: u8,
    rolled_back_hosts: Vec<String>,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct HostIntentV1 {
    schema: String,
    action: String,
    host_slug: String,
    request_sha256: String,
    authorization_nonce: String,
    boot_id: String,
    before_evidence: String,
    created_at_unix_ms: u64,
    action_deadline_unix_ms: u64,
    created_at_monotonic_ms: u64,
    action_deadline_monotonic_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct ManagerIntentV1 {
    schema: String,
    action: String,
    host_slug: String,
    request_sha256: String,
    authorization_nonce: String,
    boot_id: String,
    operation_unit: String,
    verb: String,
    target_unit: String,
    created_at_unix_ms: u64,
    action_deadline_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PreparedMutationV1 {
    schema: String,
    inventory_sha256: String,
    authorization_sha256: String,
    authorization_nonce: String,
    kind: String,
    phase: String,
    idempotency_key: String,
    operation: String,
    prepared_base64: String,
    prepared_sha256: String,
    transaction_hash: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PreparedMutationLifetimeCheck {
    Structural,
    LiveForward,
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct PreparedMutationStateV1 {
    schema: String,
    inventory_sha256: String,
    authorization_sha256: String,
    authorization_nonce: String,
    kind: String,
    phase: String,
    idempotency_key: String,
    prepared_sha256: String,
    transaction_hash: String,
    state: String,
    evidence_sha256: String,
}

#[derive(Clone, Debug, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct StateMoveIntentV1 {
    schema: String,
    host_slug: String,
    inventory_sha256: String,
    authorization_nonce: String,
    state_root: String,
    prior_device: u64,
    prior_inode: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct GeneratedMarkerV1 {
    schema: String,
    kind: String,
    host_slug: String,
    inventory_sha256: String,
    authorization_nonce: String,
    revision: String,
    created_at_unix_ms: u64,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct CleanupPlanEntryV1 {
    kind: String,
    original_name: String,
    original_path: String,
    marker: GeneratedMarkerV1,
    marker_sha256: String,
    directory_device: u64,
    directory_inode: u64,
    initial_bytes: u64,
}

#[cfg(any(target_os = "linux", test))]
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
struct CleanupPlanV1 {
    schema: String,
    action: String,
    host_slug: String,
    request_sha256: String,
    inventory_sha256: String,
    authorization_sha256: String,
    authorization_nonce: String,
    max_reclaim_bytes: u64,
    bytes_before: u64,
    entries: Vec<CleanupPlanEntryV1>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HostAction {
    Preflight,
    Upload,
    Stage,
    Stop,
    Install,
    Reset,
    Start,
    Restart,
    EdgeStage,
    EdgeCutover,
    EdgeVerify,
    Seal,
    Cleanup,
    Rollback,
    MutationReserve,
}

impl HostAction {
    const fn label(self) -> &'static str {
        match self {
            Self::Preflight => "preflight",
            Self::Upload => "upload",
            Self::Stage => "stage",
            Self::Stop => "stop",
            Self::Install => "install",
            Self::Reset => "reset",
            Self::Start => "start",
            Self::Restart => "restart",
            Self::EdgeStage => "edge_stage",
            Self::EdgeCutover => "edge_cutover",
            Self::EdgeVerify => "edge_verify",
            Self::Seal => "seal",
            Self::Cleanup => "cleanup",
            Self::Rollback => "rollback",
            Self::MutationReserve => "mutation_reserve",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "preflight" => Ok(Self::Preflight),
            "upload" => Ok(Self::Upload),
            "stage" => Ok(Self::Stage),
            "stop" => Ok(Self::Stop),
            "install" => Ok(Self::Install),
            "reset" => Ok(Self::Reset),
            "start" => Ok(Self::Start),
            "restart" => Ok(Self::Restart),
            "edge_stage" => Ok(Self::EdgeStage),
            "edge_cutover" => Ok(Self::EdgeCutover),
            "edge_verify" => Ok(Self::EdgeVerify),
            "seal" => Ok(Self::Seal),
            "cleanup" => Ok(Self::Cleanup),
            "rollback" => Ok(Self::Rollback),
            "mutation_reserve" => Ok(Self::MutationReserve),
            _ => Err(eyre!("unsupported public-reset host action")),
        }
    }

    const fn timeout_secs(self, inventory: &InventoryV1) -> u64 {
        match self {
            Self::Preflight | Self::Upload | Self::Stage => inventory.timeouts.install_secs,
            Self::Stop => inventory.timeouts.stop_secs,
            Self::Install => inventory.timeouts.install_secs,
            Self::Reset => inventory.timeouts.reset_secs,
            Self::Start => inventory.timeouts.start_secs,
            Self::Restart => inventory.timeouts.restart_secs,
            Self::EdgeStage | Self::EdgeCutover | Self::EdgeVerify | Self::Seal => {
                inventory.timeouts.edge_secs
            }
            Self::Cleanup => inventory.timeouts.cleanup_secs,
            Self::Rollback => inventory.timeouts.rollback_secs,
            Self::MutationReserve => inventory.timeouts.canary_secs,
        }
    }
}

#[derive(Clone)]
enum HostTarget {
    Validator(ValidatorV1),
    Edge(EdgeV1),
}

impl HostTarget {
    fn slug(&self) -> &str {
        match self {
            Self::Validator(value) => &value.slug,
            Self::Edge(value) => &value.slug,
        }
    }

    fn endpoint(&self) -> &EndpointV1 {
        match self {
            Self::Validator(value) => &value.endpoint,
            Self::Edge(value) => &value.endpoint,
        }
    }

    fn service_root(&self) -> &str {
        match self {
            Self::Validator(value) => &value.service_root,
            Self::Edge(value) => &value.service_root,
        }
    }

    fn state_root(&self) -> &str {
        match self {
            Self::Validator(value) => &value.state_root,
            Self::Edge(value) => &value.state_root,
        }
    }

    fn reset_guard(&self) -> &str {
        match self {
            Self::Validator(value) => &value.reset_guard,
            Self::Edge(value) => &value.reset_guard,
        }
    }

    fn artifacts(&self) -> &[ArtifactV1] {
        match self {
            Self::Validator(value) => &value.artifacts,
            Self::Edge(value) => &value.artifacts,
        }
    }
}

struct HostAdmission {
    request: HostRequestV1,
    request_sha256: String,
    inventory: InventoryV1,
    inventory_sha256: String,
    authorization: AuthorizationEnvelopeV1,
    authorization_sha256: String,
    target: HostTarget,
    guard: HostGuardV1,
    action_deadline: Instant,
    execution_expired: bool,
}

fn dispatch_host_request(request_bytes: &[u8], body: &mut impl Read) -> Result<HostReceiptV1> {
    let request: HostRequestV1 =
        json::from_slice(request_bytes).wrap_err("failed to decode exact host request JSON")?;
    if json::to_json(&request)?.as_bytes() != request_bytes {
        return Err(eyre!("host request JSON is not canonical"));
    }
    let action = HostAction::parse(&request.action)?;
    if request.recovery_only && !matches!(action, HostAction::Restart | HostAction::MutationReserve)
    {
        return Err(eyre!(
            "read-only host recovery is supported only for a submitted restart or prepared mutation"
        ));
    }
    let (admitted, _chain_guard) = admit_host_request(request, action)?;
    if action != HostAction::Upload {
        require_stream_eof(body)?;
    }
    if action == HostAction::Preflight {
        host_preflight(&admitted)?;
        return Ok(host_receipt(
            &admitted,
            action,
            false,
            0,
            0,
            "read-only preflight",
        ));
    }
    let _action_lock = lock_host_action(&admitted)?;
    ensure_host_lease(&admitted, action)?;
    if action == HostAction::MutationReserve {
        let progress = load_or_create_host_progress(&admitted)?;
        return coordinate_prepared_mutation(&admitted, &progress);
    }
    let mut progress = load_or_create_host_progress(&admitted)?;
    let progress_decision = admit_host_action_progress(&admitted, action, &progress)?;
    let receipt_name = host_receipt_name(action, &admitted.request.artifact_role)?;
    let receipt_dir = ensure_host_receipt_dir(&admitted)?;
    if admitted.request.recovery_only {
        return recover_submitted_restart(
            &admitted,
            &receipt_dir,
            &receipt_name,
            &mut progress,
            progress_decision,
        );
    }
    if progress_decision == HostProgressDecision::AbsentNoOp {
        verify_conservative_rollback_absence(&admitted)?;
        if let Some(mut receipt) =
            read_existing_host_receipt(&receipt_dir, &receipt_name, &admitted, action)?
        {
            receipt.idempotent = true;
            return Ok(receipt);
        }
        progress.rolling_back = true;
        progress.prepared_action = None;
        replace_host_progress(&admitted, &progress)?;
        let receipt = host_receipt(
            &admitted,
            action,
            true,
            0,
            0,
            "conservative rollback target proved absent from live state",
        );
        publish_host_receipt(&receipt_dir, &receipt_name, &receipt)?;
        return Ok(receipt);
    }
    if let Some(mut receipt) =
        read_existing_host_receipt(&receipt_dir, &receipt_name, &admitted, action)?
    {
        if action == HostAction::Upload {
            verify_upload_body(&admitted, body, None)?;
        }
        if action == HostAction::Cleanup {
            verify_completed_cleanup_plan(&admitted, Some(&receipt))?;
        }
        match progress_decision {
            HostProgressDecision::Advance if action != HostAction::Cleanup => {
                revalidate_cached_action_postcondition(&admitted, action)?;
            }
            HostProgressDecision::Replay if action != HostAction::Cleanup => {
                if action == HostAction::Rollback {
                    revalidate_cached_action_postcondition(&admitted, HostAction::Rollback)?;
                } else {
                    revalidate_current_target_postcondition(&admitted, &progress)?;
                }
            }
            HostProgressDecision::Advance | HostProgressDecision::Replay => {}
            HostProgressDecision::AbsentNoOp => unreachable!("handled before receipt replay"),
        }
        if progress_decision == HostProgressDecision::Advance {
            advance_host_progress(&admitted, action, &mut progress)?;
        }
        receipt.idempotent = true;
        return Ok(receipt);
    }
    if progress_decision == HostProgressDecision::Replay {
        return Err(eyre!(
            "host progress records a completed action without its immutable receipt"
        ));
    }
    if progress.prepared_action.as_ref() == Some(&host_action_key(&admitted, action))
        && !matches!(
            action,
            HostAction::Cleanup
                | HostAction::Restart
                | HostAction::EdgeCutover
                | HostAction::Rollback
        )
        && revalidate_cached_action_postcondition(&admitted, action).is_ok()
    {
        if action == HostAction::Upload {
            verify_upload_body(&admitted, body, None)?;
        }
        let receipt = host_receipt(
            &admitted,
            action,
            true,
            0,
            0,
            "host action recovered from its durable intent and exact postcondition",
        );
        publish_host_receipt(&receipt_dir, &receipt_name, &receipt)?;
        advance_host_progress(&admitted, action, &mut progress)?;
        return Ok(receipt);
    }
    if admitted.execution_expired
        && !matches!(
            action,
            HostAction::Rollback | HostAction::Seal | HostAction::Cleanup
        )
    {
        return Err(eyre!(
            "expired execution authorization permits only exact receipt/postcondition reconciliation, seal, cleanup, or rollback"
        ));
    }
    prepare_host_progress(&admitted, action, &mut progress)?;
    if action == HostAction::Restart {
        match prepare_or_recover_restart_intent(&receipt_dir, &admitted)? {
            RestartIntentDecision::SubmitNew => {}
            RestartIntentDecision::Recovered => {
                let receipt = host_receipt(
                    &admitted,
                    action,
                    true,
                    0,
                    0,
                    "validator restart recovered from durable invocation evidence",
                );
                publish_host_receipt(&receipt_dir, &receipt_name, &receipt)?;
                advance_host_progress(&admitted, action, &mut progress)?;
                return Ok(receipt);
            }
            RestartIntentDecision::Pending => {
                return Err(LocalMutationRecoveryPending {
                    action: "validator_restart",
                }
                .into());
            }
        }
    }
    let (before, after, detail) = execute_host_action(&admitted, action, body)?;
    let receipt = host_receipt(&admitted, action, false, before, after, &detail);
    publish_host_receipt(&receipt_dir, &receipt_name, &receipt)?;
    advance_host_progress(&admitted, action, &mut progress)?;
    Ok(receipt)
}

fn coordinate_prepared_mutation(
    admitted: &HostAdmission,
    progress: &HostProgressV1,
) -> Result<HostReceiptV1> {
    validate_prepared_mutation_identity(admitted)?;
    let operation = admitted.request.mutation_operation.as_str();
    if !matches!(operation, "prepare" | "fetch" | "submitted" | "applied") {
        return Err(eyre!(
            "prepared mutation operation is outside the V1 protocol"
        ));
    }
    if admitted.execution_expired && !matches!(operation, "fetch" | "applied") {
        return Err(eyre!(
            "expired execution authorization cannot prepare or submit a ledger mutation"
        ));
    }
    let coordination = host_coordination_root(admitted)?;
    let root = coordination.join("prepared-mutations");
    ensure_root_private_directory(&root)?;
    let identity = prepared_mutation_identity_sha256(
        &admitted.authorization_sha256,
        &admitted.inventory.authorization_nonce,
        &admitted.request.mutation_kind,
        &admitted.request.mutation_phase,
        &admitted.request.mutation_idempotency_key,
    );
    let prepared_name = format!("{identity}.prepared.json");
    let prepared_path = root.join(&prepared_name);
    if operation == "prepare" {
        validate_prepared_mutation_progress(admitted, progress)?;
        let candidate = prepared_mutation_from_request(admitted)?;
        if !prepared_path.exists() {
            require_prepared_mutation_predecessor(&root, admitted)?;
            publish_root_private_noreplace(
                &root,
                &prepared_name,
                json::to_json(&candidate)?.as_bytes(),
            )?;
        }
    }
    if !prepared_path.exists() {
        return Ok(prepared_mutation_receipt(
            admitted,
            "absent",
            "prepared_mutation_absent",
            None,
        ));
    }
    let (prepared, _) =
        read_private_json::<PreparedMutationV1>(&prepared_path, "prepared mutation")?;
    validate_loaded_prepared_mutation(admitted, &prepared)?;
    // A durable successor never freezes an earlier ProofRequired observation. Reopening or
    // fetching any successor must re-prove the onboarding account and alias in one atomic
    // observation before the caller can reuse persisted state, including an Applied successor.
    require_current_proof_required_onboarding(&root, admitted)?;
    if operation == "prepare" && !admitted.request.mutation_evidence_base64.is_empty() {
        return Err(eyre!("preparation cannot publish terminal evidence"));
    }
    match operation {
        "prepare" | "fetch" => {}
        "submitted" => {
            if prepared.operation == "onboarding_proof_required" {
                return Err(eyre!(
                    "proof-required onboarding has no transaction and cannot be Submitted"
                ));
            }
            validate_prepared_mutation_progress(admitted, progress)?;
            publish_prepared_mutation_state(&root, &identity, &prepared, "submitted", "")?;
        }
        "applied" => {
            let current_state = prepared_mutation_state(&root, &identity, &prepared)?;
            if current_state == "prepared" && prepared.operation != "onboarding_proof_required" {
                return Err(eyre!(
                    "prepared mutation cannot become Applied before Submitted"
                ));
            }
            let evidence = BASE64
                .decode(&admitted.request.mutation_evidence_base64)
                .wrap_err("prepared mutation Applied evidence is not base64")?;
            let evidence_sha256 = if prepared.operation == "onboarding_proof_required" {
                validate_prepared_mutation_proof_required_evidence(admitted, &prepared, &evidence)?
            } else {
                validate_prepared_mutation_applied_evidence(admitted, &prepared, &evidence)?
            };
            publish_prepared_mutation_state(
                &root,
                &identity,
                &prepared,
                "applied",
                &evidence_sha256,
            )?;
        }
        _ => unreachable!("closed above"),
    }
    let state = prepared_mutation_state(&root, &identity, &prepared)?;
    if state == "applied" && prepared.operation == "onboarding_proof_required" {
        let envelope = BASE64
            .decode(&prepared.prepared_base64)
            .wrap_err("proof-required onboarding prepared envelope is not base64")?;
        let proof_required = prepared_onboarding_proof_required_result(&envelope)?;
        prove_fresh_onboarding_current_state(
            admitted,
            &proof_required.account_id,
            &proof_required.alias,
        )
        .wrap_err("durable proof-required onboarding state is no longer current")?;
    }
    Ok(prepared_mutation_receipt(
        admitted,
        state,
        "shared immutable prepared mutation",
        Some(&prepared),
    ))
}

fn prepared_mutation_identity_sha256(
    authorization_sha256: &str,
    authorization_nonce: &str,
    kind: &str,
    phase: &str,
    idempotency_key: &str,
) -> String {
    let mut digest = Sha256::new();
    for frame in [
        b"iroha:taira:public-reset:prepared-mutation:v1\0".as_slice(),
        authorization_sha256.as_bytes(),
        authorization_nonce.as_bytes(),
        kind.as_bytes(),
        phase.as_bytes(),
        idempotency_key.as_bytes(),
    ] {
        update_frame(&mut digest, frame);
    }
    hex::encode(digest.finalize())
}

fn validate_prepared_mutation_identity(admitted: &HostAdmission) -> Result<()> {
    let request = &admitted.request;
    let valid_phase = matches!(
        request.mutation_phase.as_str(),
        "pre_edge"
            | "post_edge"
            | "restart-wave-1"
            | "restart-wave-2"
            | "restart-wave-3"
            | "restart-wave-4"
    );
    let valid_kind = match request.mutation_phase.as_str() {
        "pre_edge" => matches!(
            request.mutation_kind.as_str(),
            "onboarding"
                | "faucet"
                | "write_canary"
                | "inrou_bundle_pin"
                | "inrou_guest_pin"
                | "inrou_canary"
        ),
        "post_edge" | "restart-wave-1" | "restart-wave-2" | "restart-wave-3" | "restart-wave-4" => {
            matches!(
                request.mutation_kind.as_str(),
                "onboarding" | "faucet" | "write_canary"
            )
        }
        _ => false,
    };
    if !valid_phase
        || !valid_kind
        || request.mutation_idempotency_key
            != child_mutation_idempotency_key(
                &admitted.inventory.authorization_nonce,
                &request.mutation_phase,
                &request.mutation_kind,
            )
    {
        return Err(eyre!(
            "prepared mutation kind, phase, or child idempotency key is not exact"
        ));
    }
    Ok(())
}

fn validate_prepared_mutation_progress(
    admitted: &HostAdmission,
    progress: &HostProgressV1,
) -> Result<()> {
    validate_host_progress(admitted, progress)?;
    if progress.rolling_back || progress.sealed || progress.prepared_action.is_some() {
        return Err(eyre!(
            "prepared mutation is forbidden while a host action or terminal phase is active"
        ));
    }
    let plan = host_forward_plan(admitted);
    let first_restart = plan
        .iter()
        .position(|key| key.action == HostAction::Restart.label())
        .ok_or_else(|| eyre!("host plan omits restart phase"))?;
    let first_seal = plan
        .iter()
        .position(|key| key.action == HostAction::Seal.label())
        .ok_or_else(|| eyre!("host plan omits seal phase"))?;
    let expected = match admitted.request.mutation_phase.as_str() {
        "pre_edge" => first_restart,
        "restart-wave-1" => first_restart + 1,
        "restart-wave-2" => first_restart + 2,
        "restart-wave-3" => first_restart + 3,
        "restart-wave-4" => first_restart + 4,
        "post_edge" => first_seal,
        _ => return Err(eyre!("prepared mutation phase is outside the host plan")),
    };
    if usize::from(progress.next_forward_ordinal) != expected {
        return Err(eyre!(
            "prepared mutation is not at its exact shared host progress boundary"
        ));
    }
    Ok(())
}

fn mutation_predecessor_kind(kind: &str, phase: &str) -> Option<&'static str> {
    match (phase, kind) {
        (_, "faucet") => Some("onboarding"),
        (_, "write_canary") => Some("faucet"),
        ("pre_edge", "inrou_bundle_pin") => Some("write_canary"),
        ("pre_edge", "inrou_guest_pin") => Some("inrou_bundle_pin"),
        ("pre_edge", "inrou_canary") => Some("inrou_guest_pin"),
        _ => None,
    }
}

fn require_prepared_mutation_predecessor(root: &Path, admitted: &HostAdmission) -> Result<()> {
    let Some(kind) = mutation_predecessor_kind(
        &admitted.request.mutation_kind,
        &admitted.request.mutation_phase,
    ) else {
        return Ok(());
    };
    require_current_proof_required_onboarding(root, admitted)?;
    let key = child_mutation_idempotency_key(
        &admitted.inventory.authorization_nonce,
        &admitted.request.mutation_phase,
        kind,
    );
    let identity = prepared_mutation_identity_sha256(
        &admitted.authorization_sha256,
        &admitted.inventory.authorization_nonce,
        kind,
        &admitted.request.mutation_phase,
        &key,
    );
    let path = root.join(format!("{identity}.prepared.json"));
    let (prepared, _) =
        read_private_json::<PreparedMutationV1>(&path, "prepared mutation predecessor")
            .wrap_err("prepared mutation predecessor is absent")?;
    validate_loaded_prepared_mutation_for_identity(
        admitted,
        &prepared,
        kind,
        &admitted.request.mutation_phase,
        &key,
        PreparedMutationLifetimeCheck::Structural,
    )?;
    if prepared_mutation_state(root, &identity, &prepared)? != "applied" {
        return Err(eyre!(
            "prepared mutation predecessor has not reached exact Applied evidence"
        ));
    }
    Ok(())
}

fn require_current_proof_required_onboarding(root: &Path, admitted: &HostAdmission) -> Result<()> {
    if admitted.request.mutation_kind == "onboarding" {
        return Ok(());
    }
    let kind = "onboarding";
    let phase = admitted.request.mutation_phase.as_str();
    let key = child_mutation_idempotency_key(&admitted.inventory.authorization_nonce, phase, kind);
    let identity = prepared_mutation_identity_sha256(
        &admitted.authorization_sha256,
        &admitted.inventory.authorization_nonce,
        kind,
        phase,
        &key,
    );
    let path = root.join(format!("{identity}.prepared.json"));
    let (prepared, _) =
        read_private_json::<PreparedMutationV1>(&path, "proof-required onboarding predecessor")
            .wrap_err("proof-required onboarding predecessor is absent")?;
    validate_loaded_prepared_mutation_for_identity(
        admitted,
        &prepared,
        kind,
        phase,
        &key,
        PreparedMutationLifetimeCheck::Structural,
    )?;
    if prepared_mutation_state(root, &identity, &prepared)? != "applied" {
        return Err(eyre!(
            "proof-required onboarding predecessor is not durably Applied"
        ));
    }
    if prepared.operation == "onboarding_proof_required" {
        let envelope = BASE64
            .decode(&prepared.prepared_base64)
            .wrap_err("proof-required onboarding predecessor is not base64")?;
        let proof_required = prepared_onboarding_proof_required_result(&envelope)?;
        prove_fresh_onboarding_current_state(
            admitted,
            &proof_required.account_id,
            &proof_required.alias,
        )
        .wrap_err("proof-required onboarding predecessor is no longer current")?;
    }
    Ok(())
}

fn prepared_mutation_from_request(admitted: &HostAdmission) -> Result<PreparedMutationV1> {
    let bytes = BASE64
        .decode(&admitted.request.mutation_prepared_base64)
        .wrap_err("prepared mutation envelope base64 is invalid")?;
    let (prepared_sha256, transaction_hash, operation) = validate_prepared_mutation_envelope(
        admitted,
        &bytes,
        PreparedMutationLifetimeCheck::LiveForward,
    )?;
    if admitted.request.mutation_prepared_sha256 != prepared_sha256
        || admitted.request.mutation_transaction_hash != transaction_hash
    {
        return Err(eyre!(
            "prepared mutation request does not match its exact envelope digest and transaction hash"
        ));
    }
    Ok(PreparedMutationV1 {
        schema: PREPARED_MUTATION_SCHEMA_V1.to_owned(),
        inventory_sha256: admitted.inventory_sha256.clone(),
        authorization_sha256: admitted.authorization_sha256.clone(),
        authorization_nonce: admitted.inventory.authorization_nonce.clone(),
        kind: admitted.request.mutation_kind.clone(),
        phase: admitted.request.mutation_phase.clone(),
        idempotency_key: admitted.request.mutation_idempotency_key.clone(),
        operation,
        prepared_base64: admitted.request.mutation_prepared_base64.clone(),
        prepared_sha256,
        transaction_hash,
    })
}

fn validate_prepared_mutation_envelope(
    admitted: &HostAdmission,
    bytes: &[u8],
    lifetime_check: PreparedMutationLifetimeCheck,
) -> Result<(String, String, String)> {
    if bytes.is_empty() || bytes.len() > MAX_PREPARED_ENVELOPE_BYTES || bytes.last() != Some(&b'\n')
    {
        return Err(eyre!(
            "prepared mutation envelope is empty, oversized, or lacks its canonical newline"
        ));
    }
    let value: norito::json::Value =
        json::from_slice(bytes).wrap_err("prepared mutation envelope is not exact JSON")?;
    let mut canonical = json::to_json(&value)?.into_bytes();
    canonical.push(b'\n');
    if canonical != bytes {
        return Err(eyre!("prepared mutation envelope JSON is not canonical"));
    }
    let root = value
        .as_object()
        .ok_or_else(|| eyre!("prepared mutation envelope root is not an object"))?;
    let binding = root
        .get("binding")
        .and_then(norito::json::Value::as_object)
        .ok_or_else(|| eyre!("prepared mutation envelope omits its binding"))?;
    let binding_string = |name: &str| {
        binding
            .get(name)
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("prepared mutation binding omits `{name}`"))
    };
    let execution_expiry = binding
        .get("execution_expires_at_unix_ms")
        .and_then(norito::json::Value::as_u64)
        .ok_or_else(|| eyre!("prepared mutation binding omits execution expiry"))?;
    if binding_string("authorization_sha256")? != admitted.authorization_sha256
        || binding_string("authorization_nonce")? != admitted.inventory.authorization_nonce
        || binding_string("kind")? != admitted.request.mutation_kind
        || binding_string("phase")? != admitted.request.mutation_phase
        || binding_string("idempotency_key")? != admitted.request.mutation_idempotency_key
        || execution_expiry != admitted.authorization.claims.execution_expires_at_unix_ms
    {
        return Err(eyre!(
            "prepared mutation envelope binding differs from the signed authorization"
        ));
    }
    let is_inrou = matches!(
        admitted.request.mutation_kind.as_str(),
        "inrou_bundle_pin" | "inrou_guest_pin" | "inrou_canary"
    );
    if !is_inrou
        && !matches!(
            admitted.request.mutation_kind.as_str(),
            "onboarding" | "faucet" | "write_canary"
        )
    {
        return Err(eyre!("prepared mutation child kind is outside exact V1"));
    }
    require_exact_json_fields(
        root,
        if is_inrou {
            &[
                "schema",
                "binding",
                "public_root",
                "chain_id",
                "network_id",
                "authority",
                "stage",
                "operation",
            ]
        } else {
            &[
                "schema",
                "binding",
                "public_root",
                "chain_id",
                "network_id",
                "authority",
                "operation",
            ]
        },
        "prepared mutation envelope",
    )?;
    let binding_value = root
        .get("binding")
        .expect("binding object was validated immediately above");
    let exact_write_binding = if is_inrou {
        let exact: crate::soracloud::TairaMutationBindingV1 =
            json::from_value(binding_value.clone())
                .wrap_err("prepared Inrou binding is not exact typed V1 JSON")?;
        if json::to_value(&exact)? != *binding_value {
            return Err(eyre!(
                "prepared Inrou binding is outside its exact typed V1 closure"
            ));
        }
        None
    } else {
        let exact: TairaPublicResetMutationBindingV1 = json::from_value(binding_value.clone())
            .wrap_err("prepared write binding is not exact typed V1 JSON")?;
        if exact.schema != TairaPublicResetMutationBindingV1::SCHEMA
            || json::to_value(&exact)? != *binding_value
        {
            return Err(eyre!(
                "prepared write binding is outside its exact typed V1 closure"
            ));
        }
        Some(exact)
    };
    let network_id_value = root
        .get("network_id")
        .ok_or_else(|| eyre!("prepared mutation envelope omits its network identity"))?;
    network_id_value
        .as_str()
        .ok_or_else(|| eyre!("prepared mutation network identity is not a string"))?;
    let network_id: NetworkId = json::from_value(network_id_value.clone())
        .wrap_err("prepared mutation network identity is not canonical")?;
    let canonical_network_id = json::to_value(&network_id)
        .wrap_err("failed to encode the canonical prepared mutation network identity")?;
    let expected_genesis_hash = hex::decode(&admitted.inventory.next_genesis_hash)
        .wrap_err("signed inventory next genesis hash is invalid")?;
    let canary_authority_literal = admitted
        .inventory
        .canary_onboarding_request
        .account_id
        .as_str();
    let canary_authority = AccountId::parse_encoded(canary_authority_literal)
        .wrap_err("signed canary authority is not canonical")?;
    if canary_authority.to_string() != canary_authority_literal {
        return Err(eyre!("signed canary authority is not canonical"));
    }
    if root.get("schema").and_then(norito::json::Value::as_str)
        != Some("iroha.taira.prepared-mutation-envelope.v1")
        || root
            .get("public_root")
            .and_then(norito::json::Value::as_str)
            != Some(admitted.inventory.inrou_canary.public_root.as_str())
        || root.get("chain_id").and_then(norito::json::Value::as_str)
            != Some(admitted.inventory.chain_id.as_str())
        || canonical_network_id != *network_id_value
        || network_id.as_bytes().as_slice() != expected_genesis_hash.as_slice()
        || root.get("authority").and_then(norito::json::Value::as_str)
            != Some(canary_authority_literal)
    {
        return Err(eyre!(
            "prepared mutation envelope differs from the signed public Taira canary identity"
        ));
    }
    let operation = root
        .get("operation")
        .and_then(norito::json::Value::as_object)
        .ok_or_else(|| eyre!("prepared mutation envelope omits its operation"))?;
    require_exact_json_fields(
        operation,
        &["kind", "envelope"],
        "prepared mutation tagged operation",
    )?;
    let operation_kind = operation
        .get("kind")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("prepared mutation envelope omits its operation tag"))?;
    let operation_envelope = operation
        .get("envelope")
        .and_then(norito::json::Value::as_object)
        .ok_or_else(|| eyre!("prepared mutation operation omits its exact envelope"))?;
    if operation_kind == "onboarding_proof_required" {
        let proof_required = operation_envelope;
        let receipt_value = proof_required
            .get("receipt")
            .ok_or_else(|| eyre!("proof-required onboarding omits its signed receipt"))?;
        let result_value = proof_required
            .get("result")
            .ok_or_else(|| eyre!("proof-required onboarding omits its signed result"))?;
        let receipt: AccountOnboardingPlanReceiptV1 = json::from_value(receipt_value.clone())
            .wrap_err("proof-required onboarding receipt is not exact typed V1 JSON")?;
        let canonical_receipt = json::to_value(&receipt)
            .wrap_err("proof-required onboarding receipt cannot be canonicalized")?;
        if canonical_receipt != *receipt_value {
            return Err(eyre!(
                "proof-required onboarding receipt is outside its exact typed V1 JSON closure"
            ));
        }
        let result: AccountOnboardingProofRequiredPrepareResponseV1 =
            json::from_value(result_value.clone())
                .wrap_err("proof-required onboarding result is not exact typed V1 JSON")?;
        let exact_binding = exact_write_binding
            .as_ref()
            .expect("proof-required onboarding uses a write binding");
        if admitted.request.mutation_kind != "onboarding"
            || proof_required.len() != 3
            || proof_required
                .get("schema")
                .and_then(norito::json::Value::as_str)
                != Some("iroha.taira.prepared-onboarding-proof-required.v1")
            || exact_binding != &result.binding
            || receipt.body.request != admitted.inventory.canary_onboarding_request
        {
            return Err(eyre!(
                "proof-required onboarding envelope is outside its exact tagged V1 closure"
            ));
        }
        verify_account_onboarding_proof_required_result_v1(
            network_id,
            &admitted.inventory.canary_onboarding_request,
            &result,
            &receipt,
            exact_binding,
        )
        .wrap_err("proof-required onboarding receipt or result authentication failed")?;
        return Ok((
            sha256_hex(bytes),
            String::new(),
            "onboarding_proof_required".to_owned(),
        ));
    }
    if operation.len() != 2 || operation_envelope.get("binding") != root.get("binding") {
        return Err(eyre!(
            "prepared mutation operation does not duplicate the exact binding"
        ));
    }
    let operation_label = operation_envelope
        .get("operation")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("prepared mutation operation omits its label"))?;
    let (expected_tag, expected_operation) =
        prepared_operation_identity(&admitted.request.mutation_kind)?;
    if operation_kind != expected_tag || operation_label != expected_operation {
        return Err(eyre!(
            "prepared mutation operation tag or label does not match its child kind"
        ));
    }
    let inrou_stage = if is_inrou {
        let stage = root
            .get("stage")
            .and_then(norito::json::Value::as_object)
            .ok_or_else(|| eyre!("prepared Inrou envelope omits its retained stage identity"))?;
        require_exact_json_fields(
            stage,
            &[
                "service_name",
                "service_version",
                "route_host",
                "route_path_prefix",
                "healthcheck_path",
                "stage_mode",
                "bundle_hash",
                "bundle_content_cid",
                "bundle_manifest_digest_hex",
                "guest_content_cid",
                "guest_manifest_digest_hex",
                "container_manifest_hash",
                "service_manifest_hash",
            ],
            "prepared Inrou stage identity",
        )?;
        let canary = &admitted.inventory.inrou_canary;
        for (field, expected) in [
            ("service_name", canary.service_name.as_str()),
            ("service_version", canary.service_version.as_str()),
            ("route_host", canary.route_host.as_str()),
            ("route_path_prefix", canary.route_path_prefix.as_str()),
            ("healthcheck_path", canary.healthcheck_path.as_str()),
            ("stage_mode", "deploy"),
            ("bundle_hash", canary.bundle_hash.as_str()),
            ("bundle_content_cid", canary.bundle_content_cid.as_str()),
            (
                "bundle_manifest_digest_hex",
                canary.bundle_manifest_digest_hex.as_str(),
            ),
            ("guest_content_cid", canary.guest_content_cid.as_str()),
            (
                "guest_manifest_digest_hex",
                canary.guest_manifest_digest_hex.as_str(),
            ),
            (
                "container_manifest_hash",
                canary.container_manifest_hash.as_str(),
            ),
            (
                "service_manifest_hash",
                canary.service_manifest_hash.as_str(),
            ),
        ] {
            if stage.get(field).and_then(norito::json::Value::as_str) != Some(expected) {
                return Err(eyre!(
                    "prepared Inrou stage identity differs from inventory field `{field}`"
                ));
            }
        }
        Some(crate::soracloud::TairaInrouStageIdentity {
            service_name: canary.service_name.clone(),
            service_version: canary.service_version.clone(),
            route_host: canary.route_host.clone(),
            route_path_prefix: canary.route_path_prefix.clone(),
            healthcheck_path: canary.healthcheck_path.clone(),
            stage_mode: "deploy".to_owned(),
            bundle_hash: canary.bundle_hash.clone(),
            bundle_content_cid: canary.bundle_content_cid.clone(),
            bundle_manifest_digest_hex: canary.bundle_manifest_digest_hex.clone(),
            guest_content_cid: canary.guest_content_cid.clone(),
            guest_manifest_digest_hex: canary.guest_manifest_digest_hex.clone(),
            container_manifest_hash: canary.container_manifest_hash.clone(),
            service_manifest_hash: canary.service_manifest_hash.clone(),
        })
    } else {
        None
    };
    let expected_fee_payment = inventory_fee_payment_intent(&admitted.inventory)?;
    let expected_faucet_policy = inventory_faucet_policy(&admitted.inventory)?;
    match admitted.request.mutation_kind.as_str() {
        "onboarding" => {
            let prepared: AccountOnboardingPreparedTransactionV1 =
                json::from_value(norito::json::Value::Object(operation_envelope.clone()))
                    .wrap_err("prepared onboarding operation is not exact typed V1 JSON")?;
            if json::to_value(&prepared)? != norito::json::Value::Object(operation_envelope.clone())
                || prepared.schema != AccountOnboardingPreparedTransactionV1::SCHEMA
                || prepared.operation != AccountOnboardingPreparedTransactionV1::OPERATION
                || Some(&prepared.binding) != exact_write_binding.as_ref()
                || prepared.receipt.body.request != admitted.inventory.canary_onboarding_request
            {
                return Err(eyre!(
                    "prepared onboarding operation is outside its exact typed V1 closure"
                ));
            }
            require_lower_sha256(
                &prepared.semantic_hash_hex,
                "prepared onboarding semantic hash",
            )?;
            verify_account_onboarding_prepared_transaction_v1(
                network_id,
                &admitted.inventory.canary_onboarding_request,
                &prepared,
                &prepared.receipt,
                exact_write_binding
                    .as_ref()
                    .expect("prepared onboarding uses a write binding"),
                &expected_fee_payment,
            )
            .wrap_err("prepared onboarding transaction authentication failed")?;
        }
        "faucet" => {
            let prepared: AccountFaucetPreparedTransactionV1 =
                json::from_value(norito::json::Value::Object(operation_envelope.clone()))
                    .wrap_err("prepared faucet operation is not exact typed V1 JSON")?;
            if json::to_value(&prepared)? != norito::json::Value::Object(operation_envelope.clone())
                || prepared.schema != AccountFaucetPreparedTransactionV1::SCHEMA
                || prepared.operation != AccountFaucetPreparedTransactionV1::OPERATION
                || Some(&prepared.binding) != exact_write_binding.as_ref()
                || prepared.account_id != admitted.inventory.canary_onboarding_request.account_id
            {
                return Err(eyre!(
                    "prepared faucet operation is outside its exact typed V1 closure"
                ));
            }
            require_lower_sha256(&prepared.semantic_hash_hex, "prepared faucet semantic hash")?;
            if prepared.asset_definition_id != crate::taira::DEFAULT_GAS_ASSET_ID {
                return Err(eyre!(
                    "prepared faucet asset differs from the canonical Taira V1 fee asset"
                ));
            }
            verify_account_faucet_prepared_transaction_v1(
                network_id,
                &prepared,
                &prepared.claim,
                exact_write_binding
                    .as_ref()
                    .expect("prepared faucet uses a write binding"),
                &expected_fee_payment,
                &expected_faucet_policy,
            )
            .wrap_err("prepared faucet transaction authentication failed")?;
        }
        "write_canary" => {
            require_exact_json_fields(
                operation_envelope,
                &[
                    "schema",
                    "binding",
                    "operation",
                    "transaction_hash_hex",
                    "signed_transaction_wire_hex",
                    "signed_transaction_wire_sha256",
                    "semantic_hash_hex",
                    "fee_payment",
                    "fee_quote",
                ],
                "prepared final-canary operation",
            )?;
            if operation_envelope
                .get("schema")
                .and_then(norito::json::Value::as_str)
                != Some("iroha.taira.prepared-transaction.v1")
            {
                return Err(eyre!(
                    "prepared final-canary operation has an invalid V1 schema"
                ));
            }
            require_lower_sha256(
                operation_envelope
                    .get("semantic_hash_hex")
                    .and_then(norito::json::Value::as_str)
                    .ok_or_else(|| eyre!("prepared final-canary omits its semantic hash"))?,
                "prepared final-canary semantic hash",
            )?;
            crate::taira::verify_final_canary_prepared_operation_v1(
                &norito::json::Value::Object(operation_envelope.clone()),
                &network_id,
                &canary_authority,
            )
            .wrap_err("prepared final-canary transaction authentication failed")?;
        }
        "inrou_bundle_pin" | "inrou_guest_pin" | "inrou_canary" => {
            require_exact_json_fields(
                operation_envelope,
                &[
                    "schema",
                    "binding",
                    "operation",
                    "transaction_hash_hex",
                    "signed_transaction_wire_hex",
                    "signed_transaction_wire_sha256",
                    "fee_payment",
                    "fee_quote",
                ],
                "prepared Inrou transaction",
            )?;
            if operation_envelope
                .get("schema")
                .and_then(norito::json::Value::as_str)
                != Some("iroha.taira.prepared-soracloud-transaction.v1")
            {
                return Err(eyre!("prepared Inrou transaction has an invalid V1 schema"));
            }
        }
        _ => unreachable!("prepared child kind was closed above"),
    }
    let wire_hex = operation_envelope
        .get("signed_transaction_wire_hex")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("prepared mutation operation omits exact transaction wire"))?;
    if wire_hex.len() > MAX_PREPARED_TRANSACTION_BYTES.saturating_mul(2) {
        return Err(eyre!("prepared transaction wire is oversized"));
    }
    let wire = hex::decode(wire_hex).wrap_err("prepared transaction wire is not lowercase hex")?;
    if hex::encode(&wire) != wire_hex
        || wire.is_empty()
        || wire.len() > MAX_PREPARED_TRANSACTION_BYTES
    {
        return Err(eyre!(
            "prepared transaction wire is not canonical or bounded"
        ));
    }
    let wire_sha256 = hex::encode(Sha256::digest(&wire));
    if operation_envelope
        .get("signed_transaction_wire_sha256")
        .and_then(norito::json::Value::as_str)
        != Some(wire_sha256.as_str())
    {
        return Err(eyre!("prepared transaction wire digest is invalid"));
    }
    let transaction = SignedTransaction::decode_all_versioned(&wire)
        .wrap_err("prepared mutation wire is not a versioned SignedTransaction")?;
    let canonical_wire = transaction
        .encode_wire_v1()
        .map_err(|error| eyre!("failed to re-encode prepared transaction: {error}"))?;
    transaction
        .verify_signature()
        .wrap_err("prepared mutation transaction signature is invalid")?;
    let transaction_hash = hex::encode(transaction.hash().as_ref());
    if canonical_wire != wire
        || operation_envelope
            .get("transaction_hash_hex")
            .and_then(norito::json::Value::as_str)
            != Some(transaction_hash.as_str())
    {
        return Err(eyre!(
            "prepared mutation transaction hash or canonical wire is invalid"
        ));
    }
    if transaction.network_id() != Some(&network_id) {
        return Err(eyre!(
            "prepared mutation transaction network differs from the signed reset network"
        ));
    }
    if (is_inrou || admitted.request.mutation_kind == "write_canary")
        && transaction.authority() != &canary_authority
    {
        return Err(eyre!(
            "prepared canary transaction authority differs from the signed reset authority"
        ));
    }
    let creation_ms = u64::try_from(transaction.creation_time().as_millis())
        .wrap_err("prepared mutation transaction creation time exceeds u64")?;
    let ttl_ms = u64::try_from(
        transaction
            .time_to_live()
            .ok_or_else(|| eyre!("prepared mutation transaction omits its required TTL"))?
            .as_millis(),
    )
    .wrap_err("prepared mutation transaction TTL exceeds u64")?;
    let now_ms = match lifetime_check {
        PreparedMutationLifetimeCheck::Structural => 0,
        PreparedMutationLifetimeCheck::LiveForward => now_unix_ms()?,
    };
    validate_prepared_transaction_time_window(
        creation_ms,
        ttl_ms,
        now_ms,
        admitted.authorization.claims.not_before_unix_ms,
        execution_expiry,
        lifetime_check,
    )?;
    transaction
        .fee_payment_intent()
        .validate()
        .wrap_err("prepared mutation fee payment is invalid")?;
    if !expected_fee_payment.has_same_payer_and_gas_bound(transaction.fee_payment_intent()) {
        return Err(eyre!(
            "prepared mutation fee payer or sponsor revision differs from the signed reset inventory"
        ));
    }
    let signed_sponsor = transaction
        .fee_payment_intent()
        .sponsor_program()
        .map(|(program, revision)| (program.to_string(), revision));
    let inventory_fee_matches = match admitted.inventory.fee_intent.payer.as_str() {
        "authority" => signed_sponsor.is_none(),
        "sponsor" => signed_sponsor.as_ref().is_some_and(|(program, revision)| {
            admitted.inventory.fee_intent.sponsor_program.as_deref() == Some(program.as_str())
                && admitted.inventory.fee_intent.sponsor_program_revision == Some(*revision)
        }),
        _ => false,
    };
    if !inventory_fee_matches {
        return Err(eyre!(
            "prepared mutation fee payer differs from the signed reset inventory"
        ));
    }
    let binding_json = json::to_json(
        root.get("binding")
            .expect("binding was validated immediately above"),
    )?;
    let binding_name = Name::from_str("taira_public_reset_binding")?;
    let committed_binding_matches = transaction
        .metadata()
        .get(&binding_name)
        .map(iroha_primitives::json::Json::get)
        .map(String::as_str)
        == Some(binding_json.as_str());
    if !committed_binding_matches {
        return Err(eyre!(
            "prepared transaction metadata does not bind its exact reset authorization"
        ));
    }
    let fee_payment = operation_envelope
        .get("fee_payment")
        .ok_or_else(|| eyre!("prepared transaction omits its fee payment"))?;
    if json::to_value(transaction.fee_payment_intent())? != *fee_payment {
        return Err(eyre!(
            "prepared transaction fee payment differs from its signed wire"
        ));
    }
    if is_inrou || admitted.request.mutation_kind == "write_canary" {
        let fee_quote_value = operation_envelope
            .get("fee_quote")
            .and_then(norito::json::Value::as_object)
            .ok_or_else(|| eyre!("prepared transaction omits its fee quote"))?;
        let fee_quote: FeeQuoteResponse =
            json::from_value(norito::json::Value::Object(fee_quote_value.clone()))
                .wrap_err("prepared transaction fee quote is not exact typed V1 JSON")?;
        fee_quote
            .validate_for_signed_payload(transaction.payload())
            .map_err(|error| {
                eyre!("prepared transaction fee quote is semantically invalid: {error}")
            })?;
        if json::to_value(&fee_quote)? != norito::json::Value::Object(fee_quote_value.clone()) {
            return Err(eyre!(
                "prepared transaction fee quote is not canonical V1 JSON"
            ));
        }
    }
    if is_inrou {
        if transaction.metadata().iter().count() != 8
            || transaction
                .metadata()
                .get(&Name::from_str("taira_public_reset_authorization_sha256")?)
                .and_then(|value| value.try_into_any_norito::<String>().ok())
                .as_deref()
                != Some(admitted.authorization_sha256.as_str())
            || transaction
                .metadata()
                .get(&Name::from_str("taira_public_reset_authorization_nonce")?)
                .and_then(|value| value.try_into_any_norito::<String>().ok())
                .as_deref()
                != Some(admitted.inventory.authorization_nonce.as_str())
            || transaction
                .metadata()
                .get(&Name::from_str("taira_public_reset_mutation_kind")?)
                .and_then(|value| value.try_into_any_norito::<String>().ok())
                .as_deref()
                != Some(admitted.request.mutation_kind.as_str())
            || transaction
                .metadata()
                .get(&Name::from_str("taira_public_reset_mutation_phase")?)
                .and_then(|value| value.try_into_any_norito::<String>().ok())
                .as_deref()
                != Some(admitted.request.mutation_phase.as_str())
            || transaction
                .metadata()
                .get(&Name::from_str("taira_public_reset_idempotency_key")?)
                .and_then(|value| value.try_into_any_norito::<String>().ok())
                .as_deref()
                != Some(admitted.request.mutation_idempotency_key.as_str())
            || transaction
                .metadata()
                .get(&Name::from_str(
                    "taira_public_reset_execution_expires_at_unix_ms",
                )?)
                .and_then(|value| value.try_into_any_norito::<u64>().ok())
                != Some(admitted.authorization.claims.execution_expires_at_unix_ms)
            || transaction
                .metadata()
                .get(&Name::from_str("taira_public_reset_mutation_operation")?)
                .and_then(|value| value.try_into_any_norito::<String>().ok())
                .as_deref()
                != Some(operation_label)
        {
            return Err(eyre!(
                "prepared Inrou transaction metadata is outside its exact V1 closure"
            ));
        }
        let prepared_operation = match admitted.request.mutation_kind.as_str() {
            "inrou_bundle_pin" => crate::soracloud::TairaInrouCanaryPreparedOperationV1::BundlePin,
            "inrou_guest_pin" => crate::soracloud::TairaInrouCanaryPreparedOperationV1::GuestPin,
            "inrou_canary" => {
                crate::soracloud::TairaInrouCanaryPreparedOperationV1::ServiceMutation
            }
            _ => unreachable!("prepared Inrou child kind was closed above"),
        };
        crate::soracloud::verify_taira_inrou_prepared_transaction_identity_v1(
            &transaction,
            prepared_operation,
            inrou_stage
                .as_ref()
                .expect("prepared Inrou stage identity was validated above"),
            &admitted.request.mutation_idempotency_key,
        )
        .wrap_err("prepared Inrou transaction executable authentication failed")?;
    } else {
        let semantic = operation_envelope
            .get("semantic_hash_hex")
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("prepared write transaction omits its semantic hash"))?;
        let is_final_canary = admitted.request.mutation_kind == "write_canary";
        if transaction.metadata().iter().count() != if is_final_canary { 5 } else { 3 }
            || transaction
                .metadata()
                .get(&Name::from_str("taira_prepared_operation")?)
                .and_then(|value| value.try_into_any_norito::<String>().ok())
                .as_deref()
                != Some(operation_label)
            || transaction
                .metadata()
                .get(&Name::from_str("taira_prepared_semantic_hash")?)
                .and_then(|value| value.try_into_any_norito::<String>().ok())
                .as_deref()
                != Some(semantic)
            || (is_final_canary
                && (transaction
                    .metadata()
                    .get(&Name::from_str("taira_canary")?)
                    .and_then(|value| value.try_into_any_norito::<String>().ok())
                    .as_deref()
                    != Some("write-canary")
                    || transaction
                        .metadata()
                        .get(&Name::from_str("taira_write_canary_idempotency_v1")?)
                        .and_then(|value| value.try_into_any_norito::<String>().ok())
                        .as_deref()
                        != Some(admitted.request.mutation_idempotency_key.as_str())))
        {
            return Err(eyre!(
                "prepared write transaction metadata is outside its exact V1 closure"
            ));
        }
    }
    Ok((
        sha256_hex(bytes),
        transaction_hash,
        operation_label.to_owned(),
    ))
}

fn inventory_fee_payment_intent(inventory: &InventoryV1) -> Result<FeePaymentIntent> {
    let intent = match inventory.fee_intent.payer.as_str() {
        "authority" => FeePaymentIntent::authority(Vec::new(), None),
        "sponsor" => {
            let program = inventory
                .fee_intent
                .sponsor_program
                .as_deref()
                .ok_or_else(|| eyre!("signed reset inventory omits its sponsor program"))?
                .parse::<FeeSponsorProgramId>()
                .wrap_err("signed reset inventory sponsor program is invalid")?;
            let revision = inventory
                .fee_intent
                .sponsor_program_revision
                .filter(|revision| *revision > 0)
                .ok_or_else(|| eyre!("signed reset inventory omits its sponsor revision"))?;
            FeePaymentIntent::sponsor(program, revision, Vec::new(), None)
        }
        _ => return Err(eyre!("signed reset inventory has an unsupported fee payer")),
    };
    intent
        .validate()
        .map_err(|error| eyre!("signed reset inventory fee intent is invalid: {error}"))?;
    Ok(intent)
}

fn inventory_faucet_policy(inventory: &InventoryV1) -> Result<AccountFaucetPolicyV1> {
    let authority = AccountId::parse_encoded(&inventory.faucet_policy.authority)
        .wrap_err("signed inventory faucet authority is invalid")?;
    let asset_definition_id =
        AssetDefinitionId::from_str(&inventory.faucet_policy.asset_definition_id)
            .wrap_err("signed inventory faucet asset definition is invalid")?;
    AccountFaucetPolicyV1::try_new(
        authority,
        asset_definition_id,
        inventory.faucet_policy.amount.clone(),
    )
    .wrap_err("signed inventory faucet policy is invalid")
}

fn validate_prepared_transaction_time_window(
    creation_ms: u64,
    ttl_ms: u64,
    now_ms: u64,
    authorization_not_before_ms: u64,
    execution_expiry_ms: u64,
    lifetime_check: PreparedMutationLifetimeCheck,
) -> Result<()> {
    let expiry_ms = creation_ms
        .checked_add(ttl_ms)
        .ok_or_else(|| eyre!("prepared mutation transaction lifetime overflows u64"))?;
    if ttl_ms == 0
        || creation_ms.saturating_add(super::MAX_CLOCK_SKEW_MS) < authorization_not_before_ms
        || expiry_ms > execution_expiry_ms
    {
        return Err(eyre!(
            "prepared mutation transaction lifetime is outside the signed execution window"
        ));
    }
    if lifetime_check == PreparedMutationLifetimeCheck::LiveForward {
        if creation_ms > now_ms.saturating_add(super::MAX_CLOCK_SKEW_MS) {
            return Err(eyre!(
                "prepared mutation transaction lifetime is outside the signed execution window"
            ));
        }
        if now_ms >= expiry_ms {
            return Err(eyre!("prepared mutation transaction is already expired"));
        }
    }
    Ok(())
}

fn prepared_operation_identity(kind: &str) -> Result<(&'static str, &'static str)> {
    Ok(match kind {
        "onboarding" => ("onboarding_prepared", "onboarding"),
        "faucet" => ("faucet_prepared", "faucet"),
        "write_canary" => ("final_canary", "final_canary"),
        "inrou_bundle_pin" => ("inrou_bundle_pin", "bundle_pin"),
        "inrou_guest_pin" => ("inrou_guest_pin", "guest_pin"),
        "inrou_canary" => ("inrou_canary", "service_mutation"),
        _ => return Err(eyre!("prepared mutation child kind is outside V1")),
    })
}

fn validate_loaded_prepared_mutation(
    admitted: &HostAdmission,
    prepared: &PreparedMutationV1,
) -> Result<()> {
    let lifetime_check = match admitted.request.mutation_operation.as_str() {
        "prepare" | "submitted" => PreparedMutationLifetimeCheck::LiveForward,
        "fetch" | "applied" => PreparedMutationLifetimeCheck::Structural,
        _ => {
            return Err(eyre!(
                "prepared mutation operation is outside the V1 protocol"
            ));
        }
    };
    validate_loaded_prepared_mutation_for_identity(
        admitted,
        prepared,
        &admitted.request.mutation_kind,
        &admitted.request.mutation_phase,
        &admitted.request.mutation_idempotency_key,
        lifetime_check,
    )
}

fn validate_loaded_prepared_mutation_for_identity(
    admitted: &HostAdmission,
    prepared: &PreparedMutationV1,
    kind: &str,
    phase: &str,
    idempotency_key: &str,
    lifetime_check: PreparedMutationLifetimeCheck,
) -> Result<()> {
    if prepared.schema != PREPARED_MUTATION_SCHEMA_V1
        || prepared.inventory_sha256 != admitted.inventory_sha256
        || prepared.authorization_sha256 != admitted.authorization_sha256
        || prepared.authorization_nonce != admitted.inventory.authorization_nonce
        || prepared.kind != kind
        || prepared.phase != phase
        || prepared.idempotency_key != idempotency_key
        || prepared.prepared_sha256.len() != 64
        || !(prepared.transaction_hash.len() == 64
            && prepared
                .transaction_hash
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            || (prepared.operation == "onboarding_proof_required"
                && prepared.transaction_hash.is_empty()))
    {
        return Err(eyre!(
            "loaded prepared mutation does not bind the exact authorization child"
        ));
    }
    let bytes = BASE64
        .decode(&prepared.prepared_base64)
        .wrap_err("loaded prepared mutation base64 is invalid")?;
    let (digest, transaction_hash, operation) = validate_prepared_mutation_envelope_for_identity(
        admitted,
        &bytes,
        kind,
        phase,
        idempotency_key,
        lifetime_check,
    )?;
    if digest != prepared.prepared_sha256
        || transaction_hash != prepared.transaction_hash
        || operation != prepared.operation
    {
        return Err(eyre!("loaded prepared mutation envelope has drifted"));
    }
    Ok(())
}

fn validate_prepared_mutation_applied_evidence(
    admitted: &HostAdmission,
    prepared: &PreparedMutationV1,
    bytes: &[u8],
) -> Result<String> {
    if bytes.is_empty() || bytes.len() > MAX_PROCESS_OUTPUT {
        return Err(eyre!(
            "prepared mutation Applied evidence is empty or oversized"
        ));
    }
    let value: norito::json::Value =
        json::from_slice(bytes).wrap_err("prepared mutation Applied evidence is not JSON")?;
    let mut canonical = json::to_json(&value)?.into_bytes();
    canonical.push(b'\n');
    if canonical != bytes {
        return Err(eyre!(
            "prepared mutation Applied evidence is not canonical JSON"
        ));
    }
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("prepared mutation Applied evidence is not an object"))?;
    let is_inrou = matches!(
        prepared.kind.as_str(),
        "inrou_bundle_pin" | "inrou_guest_pin" | "inrou_canary"
    );
    let service_applied = prepared.kind == "inrou_canary";
    require_exact_json_fields(
        object,
        if service_applied {
            PREPARED_INROU_SERVICE_APPLIED_REPORT_FIELDS
        } else if is_inrou {
            PREPARED_INROU_REPORT_FIELDS
        } else if prepared.kind == "write_canary" {
            PREPARED_FINAL_CANARY_REPORT_FIELDS
        } else {
            PREPARED_WRITE_REPORT_FIELDS
        },
        "prepared mutation Applied evidence",
    )?;
    validate_prepared_report_common_arrays(
        object,
        service_applied,
        "prepared mutation Applied evidence",
    )?;
    let string = |name: &str| {
        object
            .get(name)
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("prepared mutation Applied evidence omits `{name}`"))
    };
    let command = string("command")?;
    if command
        != if is_inrou {
            "taira_inrou_canary"
        } else {
            "taira_write_canary"
        }
        || string("status")? != "ok"
        || string("public_root")? != admitted.inventory.inrou_canary.public_root
        || string("authorization_sha256")? != admitted.authorization_sha256
        || string("authorization_nonce")? != admitted.inventory.authorization_nonce
        || string("mutation_kind")? != prepared.kind
        || string("mutation_phase")? != prepared.phase
        || string("idempotency_key")? != prepared.idempotency_key
        || string("operation")? != prepared.operation
        || string("transaction_hash_hex")? != prepared.transaction_hash
        || string("prepared_envelope_sha256")? != prepared.prepared_sha256
        || string("recovery_outcome")? != "Applied"
        || object
            .get("execution_expires_at_unix_ms")
            .and_then(norito::json::Value::as_u64)
            != Some(admitted.authorization.claims.execution_expires_at_unix_ms)
        || object
            .get("applied_block_height")
            .and_then(norito::json::Value::as_u64)
            .is_none_or(|height| height == 0)
    {
        return Err(eyre!(
            "prepared mutation Applied evidence does not bind its exact transaction and authorization"
        ));
    }
    require_lower_sha256(
        string("evidence")?,
        "prepared mutation Applied committed evidence",
    )?;
    if is_inrou {
        if string("evidence")? != prepared.transaction_hash || string("mutation_mode")? != "deploy"
        {
            return Err(eyre!(
                "prepared Inrou Applied evidence differs from its exact transaction"
            ));
        }
        if service_applied {
            validate_applied_inrou_service_identity(object, &admitted.inventory)?;
            if !object
                .get("observed_at_unix_ms")
                .and_then(norito::json::Value::as_u64)
                .is_some_and(|timestamp| timestamp > 0)
            {
                return Err(eyre!(
                    "prepared Inrou Applied evidence omits its observation timestamp"
                ));
            }
        }
    }
    let envelope = BASE64
        .decode(&prepared.prepared_base64)
        .wrap_err("prepared mutation Applied envelope is not base64")?;
    validate_prepared_report_envelope_bytes(&value, &envelope)?;
    Ok(sha256_hex(bytes))
}

fn validate_prepared_mutation_proof_required_evidence(
    admitted: &HostAdmission,
    prepared: &PreparedMutationV1,
    bytes: &[u8],
) -> Result<String> {
    if bytes.is_empty() || bytes.len() > MAX_PROCESS_OUTPUT {
        return Err(eyre!(
            "proof-required onboarding evidence is empty or oversized"
        ));
    }
    let value: norito::json::Value =
        json::from_slice(bytes).wrap_err("proof-required onboarding evidence is not JSON")?;
    let mut canonical = json::to_json(&value)?.into_bytes();
    canonical.push(b'\n');
    if canonical != bytes {
        return Err(eyre!(
            "proof-required onboarding evidence is not canonical JSON"
        ));
    }
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("proof-required onboarding evidence is not an object"))?;
    require_exact_json_fields(
        object,
        PREPARED_WRITE_REPORT_FIELDS,
        "proof-required onboarding evidence",
    )?;
    validate_prepared_report_common_arrays(object, false, "proof-required onboarding evidence")?;
    let string = |name: &str| {
        object
            .get(name)
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("proof-required onboarding evidence omits `{name}`"))
    };
    let envelope = BASE64
        .decode(&prepared.prepared_base64)
        .wrap_err("proof-required onboarding prepared envelope is not base64")?;
    let proof_required = prepared_onboarding_proof_required_result(&envelope)?;
    if prepared.operation != "onboarding_proof_required"
        || !prepared.transaction_hash.is_empty()
        || string("command")? != "taira_write_canary"
        || string("status")? != "ok"
        || string("public_root")? != admitted.inventory.inrou_canary.public_root
        || string("authorization_sha256")? != admitted.authorization_sha256
        || string("authorization_nonce")? != admitted.inventory.authorization_nonce
        || string("mutation_kind")? != "onboarding"
        || string("mutation_phase")? != prepared.phase
        || string("idempotency_key")? != prepared.idempotency_key
        || string("operation")? != "onboarding"
        || string("recovery_outcome")? != "Applied"
        || string("prepared_envelope_sha256")? != prepared.prepared_sha256
        || object
            .get("execution_expires_at_unix_ms")
            .and_then(norito::json::Value::as_u64)
            != Some(admitted.authorization.claims.execution_expires_at_unix_ms)
        || object.get("transaction_hash_hex") != Some(&norito::json::Value::Null)
        || object.get("applied_block_height") != Some(&norito::json::Value::Null)
        || require_lower_sha256(
            string("evidence")?,
            "proof-required onboarding semantic evidence",
        )
        .is_err()
        || string("evidence")? != proof_required.semantic_hash_hex
    {
        return Err(eyre!(
            "proof-required onboarding evidence does not bind the fresh atomic account-and-alias proof"
        ));
    }
    validate_prepared_report_envelope_bytes(&value, &envelope)?;
    prove_fresh_onboarding_current_state(
        admitted,
        &proof_required.account_id,
        &proof_required.alias,
    )?;
    Ok(sha256_hex(bytes))
}

fn prepared_onboarding_proof_required_result(
    envelope: &[u8],
) -> Result<AccountOnboardingProofRequiredPrepareResponseV1> {
    let value: norito::json::Value = json::from_slice(envelope)
        .wrap_err("proof-required onboarding prepared envelope is not JSON")?;
    let result = value
        .as_object()
        .and_then(|root| root.get("operation"))
        .and_then(norito::json::Value::as_object)
        .and_then(|operation| operation.get("envelope"))
        .and_then(norito::json::Value::as_object)
        .and_then(|proof_required| proof_required.get("result"))
        .cloned()
        .ok_or_else(|| eyre!("proof-required onboarding envelope omits its exact result"))?;
    json::from_value(result).wrap_err("proof-required onboarding result is not exact typed V1 JSON")
}

fn prove_fresh_onboarding_current_state(
    admitted: &HostAdmission,
    account_id_literal: &str,
    alias_literal: &str,
) -> Result<()> {
    let parsed_account = AccountId::parse_encoded(account_id_literal)
        .wrap_err("proof-required onboarding account is not canonical I105")?;
    if parsed_account.to_string() != account_id_literal {
        return Err(eyre!(
            "proof-required onboarding account is not canonical I105"
        ));
    }
    let expected_account = parsed_account;
    let expected_alias = alias_literal
        .parse::<AccountAliasName>()
        .wrap_err("proof-required onboarding alias is not canonical")?;
    if expected_alias.to_string() != alias_literal {
        return Err(eyre!(
            "proof-required onboarding alias is not canonically encoded"
        ));
    }
    let request = AccountOnboardingCurrentStateRequestV1::new(&expected_account, &expected_alias);
    request
        .validate_exact()
        .map_err(|error| eyre!("atomic onboarding state request is invalid: {error}"))?;
    let request_body = json::to_vec(&request)
        .wrap_err("failed to encode exact atomic onboarding state request")?;

    let genesis_hash: [u8; Hash::LENGTH] = hex::decode(&admitted.inventory.next_genesis_hash)
        .wrap_err("signed inventory next genesis hash is invalid")?
        .try_into()
        .map_err(|_| eyre!("signed inventory next genesis hash is not 32 bytes"))?;
    let expected_network_id = NetworkId::from_genesis_hash(
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(genesis_hash)),
    );
    let remaining = admitted
        .action_deadline
        .checked_duration_since(Instant::now())
        .ok_or_else(|| eyre!("fresh onboarding proof deadline has elapsed"))?;
    let timeout = remaining.min(Duration::from_secs(30));
    if timeout.is_zero() {
        return Err(eyre!("fresh onboarding proof has no remaining deadline"));
    }
    let root = Url::parse(&format!(
        "{}/",
        admitted
            .inventory
            .inrou_canary
            .public_root
            .trim_end_matches('/')
    ))
    .wrap_err("signed public Taira root is invalid")?;
    let proof_url = root
        .join("v1/accounts/onboarding/current-state")
        .wrap_err("failed to construct atomic onboarding state URL")?;
    let http = HttpClient::builder()
        .timeout(timeout)
        .connect_timeout(timeout)
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("iroha-taira-public-reset-host/1")
        .build()
        .wrap_err("failed to build fresh-state proof HTTP client")?;

    let response_value = bounded_proof_read_json(
        http.post(proof_url)
            .header(ACCEPT, "application/json")
            .header(CONTENT_TYPE, "application/json")
            .body(request_body)
            .send()
            .wrap_err("atomic onboarding current-state POST failed")?,
        "atomic current-state",
    )?;
    let response: AccountOnboardingCurrentStateResponseV1 = json::from_value(response_value)
        .wrap_err("atomic onboarding current-state response is not exact typed V1 JSON")?;
    let (_, alias_target) = response
        .validate_for(&request, &expected_network_id)
        .map_err(|error| eyre!("atomic onboarding current-state response is invalid: {error}"))?;
    if !response.account_exists {
        return Err(eyre!(
            "atomic onboarding current-state response reports the exact account absent"
        ));
    }
    match alias_target {
        Some(target) if target == expected_account => Ok(()),
        Some(_) => Err(eyre!(
            "atomic onboarding current-state response reports an alias target conflict"
        )),
        None => Err(eyre!(
            "atomic onboarding current-state response reports the exact alias absent"
        )),
    }
}

fn bounded_proof_read_json(
    response: HttpResponse,
    label: &'static str,
) -> Result<norito::json::Value> {
    if response.status() != StatusCode::OK {
        return Err(eyre!(
            "fresh onboarding {label} read returned HTTP {}",
            response.status()
        ));
    }
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .unwrap_or_default();
    if !content_type.eq_ignore_ascii_case("application/json") {
        return Err(eyre!(
            "fresh onboarding {label} read returned non-JSON content type"
        ));
    }
    if response.content_length().is_some_and(|length| {
        usize::try_from(length).map_or(true, |length| length > MAX_PROOF_READ_RESPONSE_BYTES)
    }) {
        return Err(eyre!(
            "fresh onboarding {label} response exceeds its V1 byte limit"
        ));
    }
    let mut bytes = Vec::new();
    let read_limit = u64::try_from(MAX_PROOF_READ_RESPONSE_BYTES)
        .expect("atomic onboarding response byte limit fits u64")
        + 1;
    response
        .take(read_limit)
        .read_to_end(&mut bytes)
        .wrap_err_with(|| format!("failed to read fresh onboarding {label} response"))?;
    if bytes.is_empty() || bytes.len() > MAX_PROOF_READ_RESPONSE_BYTES {
        return Err(eyre!(
            "fresh onboarding {label} response is empty or oversized"
        ));
    }
    json::from_slice(&bytes)
        .wrap_err_with(|| format!("fresh onboarding {label} response is not JSON"))
}

fn validate_prepared_mutation_envelope_for_identity(
    admitted: &HostAdmission,
    bytes: &[u8],
    kind: &str,
    phase: &str,
    idempotency_key: &str,
    lifetime_check: PreparedMutationLifetimeCheck,
) -> Result<(String, String, String)> {
    let mut request = admitted.request.clone();
    request.mutation_kind = kind.to_owned();
    request.mutation_phase = phase.to_owned();
    request.mutation_idempotency_key = idempotency_key.to_owned();
    let borrowed = HostAdmission {
        request,
        request_sha256: admitted.request_sha256.clone(),
        inventory: admitted.inventory.clone(),
        inventory_sha256: admitted.inventory_sha256.clone(),
        authorization: admitted.authorization.clone(),
        authorization_sha256: admitted.authorization_sha256.clone(),
        target: admitted.target.clone(),
        guard: admitted.guard.clone(),
        action_deadline: admitted.action_deadline,
        execution_expired: admitted.execution_expired,
    };
    validate_prepared_mutation_envelope(&borrowed, bytes, lifetime_check)
}

fn publish_prepared_mutation_state(
    root: &Path,
    identity: &str,
    prepared: &PreparedMutationV1,
    state: &str,
    evidence_sha256: &str,
) -> Result<()> {
    if !matches!(state, "submitted" | "applied") {
        return Err(eyre!("prepared mutation state is outside V1"));
    }
    let marker = PreparedMutationStateV1 {
        schema: PREPARED_MUTATION_STATE_SCHEMA_V1.to_owned(),
        inventory_sha256: prepared.inventory_sha256.clone(),
        authorization_sha256: prepared.authorization_sha256.clone(),
        authorization_nonce: prepared.authorization_nonce.clone(),
        kind: prepared.kind.clone(),
        phase: prepared.phase.clone(),
        idempotency_key: prepared.idempotency_key.clone(),
        prepared_sha256: prepared.prepared_sha256.clone(),
        transaction_hash: prepared.transaction_hash.clone(),
        state: state.to_owned(),
        evidence_sha256: evidence_sha256.to_owned(),
    };
    publish_root_private_noreplace(
        root,
        &format!("{identity}.{state}.json"),
        json::to_json(&marker)?.as_bytes(),
    )
}

fn prepared_mutation_state(
    root: &Path,
    identity: &str,
    prepared: &PreparedMutationV1,
) -> Result<&'static str> {
    for state in ["applied", "submitted"] {
        let path = root.join(format!("{identity}.{state}.json"));
        if !path.exists() {
            continue;
        }
        let (marker, _) =
            read_private_json::<PreparedMutationStateV1>(&path, "prepared mutation state")?;
        if marker.schema != PREPARED_MUTATION_STATE_SCHEMA_V1
            || marker.inventory_sha256 != prepared.inventory_sha256
            || marker.authorization_sha256 != prepared.authorization_sha256
            || marker.authorization_nonce != prepared.authorization_nonce
            || marker.kind != prepared.kind
            || marker.phase != prepared.phase
            || marker.idempotency_key != prepared.idempotency_key
            || marker.prepared_sha256 != prepared.prepared_sha256
            || marker.transaction_hash != prepared.transaction_hash
            || marker.state != state
            || (state == "applied") != !marker.evidence_sha256.is_empty()
            || (!marker.evidence_sha256.is_empty()
                && require_lower_sha256(
                    &marker.evidence_sha256,
                    "prepared mutation evidence digest",
                )
                .is_err())
        {
            return Err(eyre!(
                "prepared mutation state marker does not bind its immutable envelope"
            ));
        }
        return Ok(state);
    }
    Ok("prepared")
}

fn prepared_mutation_receipt(
    admitted: &HostAdmission,
    state: &str,
    detail: &str,
    prepared: Option<&PreparedMutationV1>,
) -> HostReceiptV1 {
    let mut receipt = host_recovery_receipt(admitted, HostAction::MutationReserve, state, detail);
    receipt.mutation_state = state.to_owned();
    if let Some(prepared) = prepared {
        receipt.mutation_prepared_base64 = prepared.prepared_base64.clone();
        receipt.mutation_prepared_sha256 = prepared.prepared_sha256.clone();
        receipt.mutation_transaction_hash = prepared.transaction_hash.clone();
    }
    receipt
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RestartRecoveryEvidence {
    Applied,
    Pending,
    Rejected(&'static str),
}

fn recover_submitted_restart(
    admitted: &HostAdmission,
    receipt_dir: &Path,
    receipt_name: &str,
    progress: &mut HostProgressV1,
    decision: HostProgressDecision,
) -> Result<HostReceiptV1> {
    let existing =
        read_existing_host_receipt(receipt_dir, receipt_name, admitted, HostAction::Restart)?;
    match recover_existing_restart_intent(receipt_dir, admitted)? {
        RestartRecoveryEvidence::Applied => {
            if let Some(mut receipt) = existing {
                if decision == HostProgressDecision::Advance {
                    advance_host_progress(admitted, HostAction::Restart, progress)?;
                }
                receipt.idempotent = true;
                return Ok(receipt);
            }
            if decision == HostProgressDecision::Replay {
                return Err(eyre!(
                    "restart progress is complete without its immutable host receipt"
                ));
            }
            if progress.prepared_action.as_ref()
                != Some(&host_action_key(admitted, HostAction::Restart))
            {
                return Ok(host_recovery_receipt(
                    admitted,
                    HostAction::Restart,
                    "rejected",
                    "restart_not_submitted",
                ));
            }
            let receipt = host_receipt(
                admitted,
                HostAction::Restart,
                true,
                0,
                0,
                "validator restart recovered from durable same-boot invocation evidence",
            );
            publish_host_receipt(receipt_dir, receipt_name, &receipt)?;
            advance_host_progress(admitted, HostAction::Restart, progress)?;
            Ok(receipt)
        }
        RestartRecoveryEvidence::Pending => Ok(host_recovery_receipt(
            admitted,
            HostAction::Restart,
            "pending",
            "restart_outcome_pending",
        )),
        RestartRecoveryEvidence::Rejected(class) => Ok(host_recovery_receipt(
            admitted,
            HostAction::Restart,
            "rejected",
            class,
        )),
    }
}

fn recover_existing_restart_intent(
    receipt_dir: &Path,
    admitted: &HostAdmission,
) -> Result<RestartRecoveryEvidence> {
    let HostTarget::Validator(validator) = &admitted.target else {
        return Ok(RestartRecoveryEvidence::Rejected(
            "restart_target_not_validator",
        ));
    };
    let path = receipt_dir.join("restart.intent.json");
    if !path.exists() {
        return Ok(RestartRecoveryEvidence::Rejected("restart_not_submitted"));
    }
    let (intent, _) = read_private_json::<HostIntentV1>(&path, "restart intent")?;
    if intent.schema != HOST_INTENT_SCHEMA_V1
        || intent.action != HostAction::Restart.label()
        || intent.host_slug != admitted.target.slug()
        || intent.request_sha256 != admitted.request_sha256
        || intent.authorization_nonce != admitted.inventory.authorization_nonce
        || validate_boot_id(&intent.boot_id).is_err()
        || intent.before_evidence.is_empty()
        || intent.created_at_unix_ms >= intent.action_deadline_unix_ms
        || intent.created_at_monotonic_ms >= intent.action_deadline_monotonic_ms
        || intent.action_deadline_unix_ms
            > admitted.authorization.claims.execution_expires_at_unix_ms
    {
        return Ok(RestartRecoveryEvidence::Rejected("restart_intent_mismatch"));
    }
    let manager_path = receipt_dir.join(manager_intent_name("restart")?);
    if !manager_path.exists() {
        return Ok(RestartRecoveryEvidence::Rejected("restart_not_submitted"));
    }
    let (manager_intent, _) =
        read_private_json::<ManagerIntentV1>(&manager_path, "restart manager intent")?;
    validate_manager_intent(
        admitted,
        "restart",
        "restart",
        &validator.systemd_unit,
        &manager_intent,
    )?;
    match inspect_manager_operation(&manager_intent, admitted.action_deadline) {
        Ok(ManagerOperationEvidence::Applied) => {}
        Ok(ManagerOperationEvidence::Absent | ManagerOperationEvidence::Pending) | Err(_) => {
            return Ok(RestartRecoveryEvidence::Pending);
        }
        Ok(ManagerOperationEvidence::Rejected) => {
            return Ok(RestartRecoveryEvidence::Rejected(
                "restart_manager_rejected",
            ));
        }
    }
    let evidence = unit_restart_evidence(validator, admitted.action_deadline)?;
    if !evidence.is_terminal_active() {
        return Ok(RestartRecoveryEvidence::Pending);
    }
    let release = Path::new(&validator.service_root)
        .join("releases")
        .join(&admitted.inventory.revision.commit);
    let process_attested = attest_validator_process(admitted, validator, &release, true).is_ok();
    Ok(classify_restart_observation(
        &intent,
        &evidence.invocation,
        evidence.active_enter_monotonic_ms,
        &evidence.boot_id,
        process_attested,
    ))
}

fn classify_restart_observation(
    intent: &HostIntentV1,
    invocation: &str,
    active_enter_monotonic_ms: u64,
    boot_id: &str,
    process_attested: bool,
) -> RestartRecoveryEvidence {
    if boot_id != intent.boot_id {
        return RestartRecoveryEvidence::Rejected("restart_boot_changed");
    }
    if invocation == intent.before_evidence {
        // A timed-out systemctl client is not proof that PID 1 did not retain
        // the matching manager job. Unchanged invocation evidence therefore
        // remains ambiguous and cannot authorize rollback or resubmission.
        return RestartRecoveryEvidence::Pending;
    }
    if active_enter_monotonic_ms < intent.created_at_monotonic_ms
        || active_enter_monotonic_ms > intent.action_deadline_monotonic_ms
    {
        return RestartRecoveryEvidence::Rejected("restart_outside_authorized_window");
    }
    if !process_attested {
        return RestartRecoveryEvidence::Pending;
    }
    RestartRecoveryEvidence::Applied
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RestartIntentDecision {
    SubmitNew,
    Recovered,
    Pending,
}

fn prepare_or_recover_restart_intent(
    receipt_dir: &Path,
    admitted: &HostAdmission,
) -> Result<RestartIntentDecision> {
    let HostTarget::Validator(validator) = &admitted.target else {
        return Err(eyre!("restart intent requires a validator target"));
    };
    let evidence = unit_restart_evidence(validator, admitted.action_deadline)?;
    let before = evidence.invocation;
    let active_enter_ms = evidence.active_enter_monotonic_ms;
    let boot_id = evidence.boot_id;
    let now = now_unix_ms()?;
    let monotonic_now = host_monotonic_ms()?;
    let monotonic_deadline = monotonic_now
        .checked_add(
            u64::try_from(
                admitted
                    .action_deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis(),
            )
            .wrap_err("restart monotonic deadline exceeds u64")?,
        )
        .ok_or_else(|| eyre!("restart monotonic deadline overflow"))?;
    let intent = HostIntentV1 {
        schema: HOST_INTENT_SCHEMA_V1.to_owned(),
        action: HostAction::Restart.label().to_owned(),
        host_slug: admitted.target.slug().to_owned(),
        request_sha256: admitted.request_sha256.clone(),
        authorization_nonce: admitted.inventory.authorization_nonce.clone(),
        boot_id: boot_id.clone(),
        before_evidence: before.clone(),
        created_at_unix_ms: now,
        action_deadline_unix_ms: admitted.request.action_deadline_unix_ms,
        created_at_monotonic_ms: monotonic_now,
        action_deadline_monotonic_ms: monotonic_deadline,
    };
    let path = receipt_dir.join("restart.intent.json");
    if path.exists() {
        let (existing, _) = read_private_json::<HostIntentV1>(&path, "restart intent")?;
        if existing.schema != intent.schema
            || existing.action != intent.action
            || existing.host_slug != intent.host_slug
            || existing.request_sha256 != intent.request_sha256
            || existing.authorization_nonce != intent.authorization_nonce
            || validate_boot_id(&existing.boot_id).is_err()
            || existing.before_evidence.is_empty()
            || existing.created_at_unix_ms >= existing.action_deadline_unix_ms
            || existing.created_at_monotonic_ms >= existing.action_deadline_monotonic_ms
            || existing.action_deadline_unix_ms
                > admitted.authorization.claims.execution_expires_at_unix_ms
        {
            return Err(eyre!("restart intent does not bind this exact action"));
        }
        if existing.boot_id != boot_id {
            return Ok(RestartIntentDecision::Pending);
        }
        if before != existing.before_evidence {
            if active_enter_ms >= existing.created_at_monotonic_ms
                && active_enter_ms <= existing.action_deadline_monotonic_ms
            {
                let release = Path::new(&validator.service_root)
                    .join("releases")
                    .join(&admitted.inventory.revision.commit);
                attest_validator_process(admitted, validator, &release, true)?;
                return Ok(RestartIntentDecision::Recovered);
            }
            return Ok(RestartIntentDecision::Pending);
        }
        return Ok(RestartIntentDecision::Pending);
    }
    publish_root_private_noreplace(
        receipt_dir,
        "restart.intent.json",
        json::to_json(&intent)?.as_bytes(),
    )?;
    Ok(RestartIntentDecision::SubmitNew)
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UnitRestartEvidence {
    invocation: String,
    active_enter_monotonic_ms: u64,
    boot_id: String,
    active_state: String,
    sub_state: String,
    job: String,
}

impl UnitRestartEvidence {
    fn is_terminal_active(&self) -> bool {
        self.active_state == "active"
            && !matches!(self.sub_state.as_str(), "start" | "stop" | "running")
            && matches!(self.job.as_str(), "" | "0")
    }
}

fn unit_restart_evidence(
    validator: &ValidatorV1,
    deadline: Instant,
) -> Result<UnitRestartEvidence> {
    let boot_before = host_boot_id()?;
    let bytes = run_host_command(
        SYSTEMCTL,
        &[
            "show",
            "--property=InvocationID",
            "--property=ActiveEnterTimestampMonotonic",
            "--property=ActiveState",
            "--property=SubState",
            "--property=Job",
            &validator.systemd_unit,
        ],
        deadline,
    )?;
    let text = std::str::from_utf8(&bytes)?;
    let mut invocation = None;
    let mut active_enter = None;
    let mut active_state = None;
    let mut sub_state = None;
    let mut job = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("InvocationID=") {
            invocation = Some(value.to_owned());
        } else if let Some(value) = line.strip_prefix("ActiveEnterTimestampMonotonic=") {
            active_enter = value.parse::<u64>().ok();
        } else if let Some(value) = line.strip_prefix("ActiveState=") {
            active_state = Some(value.to_owned());
        } else if let Some(value) = line.strip_prefix("SubState=") {
            sub_state = Some(value.to_owned());
        } else if let Some(value) = line.strip_prefix("Job=") {
            job = Some(value.to_owned());
        } else {
            return Err(eyre!(
                "systemd restart evidence contains an unknown property"
            ));
        }
    }
    let value = invocation.ok_or_else(|| eyre!("systemd restart evidence omits InvocationID"))?;
    if value.len() != 32
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(eyre!("systemd InvocationID is not canonical lowercase hex"));
    }
    let active_enter_ms = active_enter
        .and_then(|value| value.checked_div(1_000))
        .filter(|value| *value > 0)
        .ok_or_else(|| eyre!("systemd ActiveEnterTimestampMonotonic is not positive"))?;
    let boot_after = host_boot_id()?;
    if boot_before != boot_after {
        return Err(eyre!(
            "kernel boot identity changed during restart evidence"
        ));
    }
    Ok(UnitRestartEvidence {
        invocation: value,
        active_enter_monotonic_ms: active_enter_ms,
        boot_id: boot_before,
        active_state: active_state
            .ok_or_else(|| eyre!("systemd restart evidence omits ActiveState"))?,
        sub_state: sub_state.ok_or_else(|| eyre!("systemd restart evidence omits SubState"))?,
        job: job.ok_or_else(|| eyre!("systemd restart evidence omits Job"))?,
    })
}

fn host_boot_id() -> Result<String> {
    let path = Path::new("/proc/sys/kernel/random/boot_id");
    let (mut file, snapshot) = open_pinned_regular(path, "kernel boot identity")?;
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(38)
        .read_to_end(&mut bytes)?;
    ensure_pinned_unchanged(path, "kernel boot identity", &file, &snapshot)?;
    let value = std::str::from_utf8(&bytes)?
        .strip_suffix('\n')
        .ok_or_else(|| eyre!("kernel boot identity lacks one trailing newline"))?
        .to_owned();
    validate_boot_id(&value)?;
    Ok(value)
}

fn validate_boot_id(value: &str) -> Result<()> {
    if value.len() != 36
        || value.bytes().enumerate().any(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte != b'-'
            } else {
                !(byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            }
        })
    {
        return Err(eyre!(
            "kernel boot identity is not canonical lowercase UUID"
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[allow(
    unsafe_code,
    reason = "clock_gettime(CLOCK_MONOTONIC) is the fixed Linux systemd timestamp clock"
)]
fn host_monotonic_ms() -> Result<u64> {
    #[repr(C)]
    struct Timespec {
        seconds: std::ffi::c_long,
        nanoseconds: std::ffi::c_long,
    }
    unsafe extern "C" {
        fn clock_gettime(clock_id: std::ffi::c_int, value: *mut Timespec) -> std::ffi::c_int;
    }
    const CLOCK_MONOTONIC: std::ffi::c_int = 1;
    let mut value = Timespec {
        seconds: 0,
        nanoseconds: 0,
    };
    if unsafe { clock_gettime(CLOCK_MONOTONIC, &raw mut value) } != 0 {
        return Err(std::io::Error::last_os_error())
            .wrap_err("failed to read Linux CLOCK_MONOTONIC");
    }
    let seconds = u64::try_from(value.seconds).wrap_err("CLOCK_MONOTONIC seconds are negative")?;
    let nanos =
        u64::try_from(value.nanoseconds).wrap_err("CLOCK_MONOTONIC nanoseconds are negative")?;
    seconds
        .checked_mul(1_000)
        .and_then(|value| value.checked_add(nanos / 1_000_000))
        .ok_or_else(|| eyre!("CLOCK_MONOTONIC millisecond conversion overflow"))
}

#[cfg(not(target_os = "linux"))]
fn host_monotonic_ms() -> Result<u64> {
    Err(eyre!(
        "systemd restart evidence requires Linux CLOCK_MONOTONIC"
    ))
}

fn admit_host_request(
    request: HostRequestV1,
    action: HostAction,
) -> Result<(HostAdmission, ChainDiscriminantGuard)> {
    if request.schema != HOST_REQUEST_SCHEMA_V1 {
        return Err(eyre!("host request schema is not exact V1"));
    }
    require_lower_sha256(
        &request.authorization_semantic_sha256,
        "host request authorization digest",
    )?;
    require_lower_sha256(
        &request.trusted_key_sha256,
        "host request trusted-key digest",
    )?;
    let inventory_bytes = BASE64
        .decode(&request.inventory_base64)
        .wrap_err("host request inventory base64 is invalid")?;
    let authorization_bytes = BASE64
        .decode(&request.authorization_base64)
        .wrap_err("host request authorization base64 is invalid")?;
    let trusted_key_bytes = BASE64
        .decode(&request.trusted_key_base64)
        .wrap_err("host request trusted-key base64 is invalid")?;
    if inventory_bytes.len() > super::MAX_JSON_BYTES as usize
        || authorization_bytes.len() > super::MAX_JSON_BYTES as usize
        || trusted_key_bytes.len() > super::MAX_JSON_BYTES as usize
        || sha256_hex(&trusted_key_bytes) != request.trusted_key_sha256
    {
        return Err(eyre!(
            "host request embedded closure exceeded a bound or hash drifted"
        ));
    }
    let inventory: InventoryV1 =
        json::from_slice(&inventory_bytes).wrap_err("host request inventory is invalid")?;
    let authorization: AuthorizationEnvelopeV1 =
        json::from_slice(&authorization_bytes).wrap_err("host request authorization is invalid")?;
    let trusted_key: TrustedKeyV1 =
        json::from_slice(&trusted_key_bytes).wrap_err("host request trusted key is invalid")?;
    let chain_guard = super::enter_inventory_chain_discriminant(&inventory)?;
    validate_inventory(&inventory)?;
    validate_first_release_physical_host(&inventory)?;
    let inventory_sha256 = sha256_hex(&inventory_bytes);
    let authorization_sha256 = authorization_semantic_sha256(&authorization, &trusted_key)?;
    if authorization_sha256 != request.authorization_semantic_sha256 {
        return Err(eyre!("host request semantic authorization digest drifted"));
    }
    let target = inventory
        .validators
        .iter()
        .find(|validator| validator.slug == request.host_slug)
        .cloned()
        .map(HostTarget::Validator)
        .or_else(|| {
            (inventory.edge.slug == request.host_slug)
                .then(|| HostTarget::Edge(inventory.edge.clone()))
        })
        .ok_or_else(|| eyre!("host request slug is not in the signed inventory"))?;
    let guard_path = Path::new(target.reset_guard()).join("guard.json");
    let (guard, guard_bytes) = read_private_json::<HostGuardV1>(&guard_path, "host guard")?;
    if sha256_hex(&guard_bytes) != target.endpoint().upload_guard_sha256
        || guard.schema != HOST_GUARD_SCHEMA_V1
        || guard.host_slug != target.slug()
        || guard.service_root != target.service_root()
        || guard.state_root != target.state_root()
        || guard.trusted_key_sha256 != request.trusted_key_sha256
        || guard.dispatcher_path != FIXED_DISPATCHER
        || guard.upload_parent != upload_parent(target.service_root())
    {
        return Err(eyre!(
            "independently provisioned host guard does not bind this request"
        ));
    }
    verify_dispatcher_identity(&guard)?;
    let now = now_unix_ms()?;
    let monotonic_now = Instant::now();
    let execution_expired = now
        > authorization
            .claims
            .execution_expires_at_unix_ms
            .saturating_add(super::MAX_CLOCK_SKEW_MS);
    validate_action_deadline(&request, action, &inventory, &authorization, now)?;
    if action == HostAction::Preflight {
        super::verify_authorization(
            &inventory,
            &inventory_sha256,
            &authorization,
            &trusted_key,
            now,
        )?;
    } else if action == HostAction::Rollback || execution_expired {
        // Terminal recovery is authorized only by the already-acquired,
        // semantically matching durable lease. Verify the exact signed
        // execution claims at an instant inside their original window here;
        // `ensure_host_lease` rejects absent or foreign durable state.
        let signed_instant = authorization
            .claims
            .execution_expires_at_unix_ms
            .min(authorization.claims.expires_at_unix_ms)
            .saturating_sub(1)
            .max(authorization.claims.not_before_unix_ms);
        verify_execution_authorization(
            &inventory,
            &inventory_sha256,
            &authorization,
            &trusted_key,
            signed_instant,
        )?;
    } else {
        verify_execution_authorization(
            &inventory,
            &inventory_sha256,
            &authorization,
            &trusted_key,
            now,
        )?;
    }
    validate_host_artifact_request(&request, &target, action)?;
    let request_sha256 = host_request_identity_sha256(&request)?;
    let action_deadline = monotonic_now
        .checked_add(Duration::from_millis(
            request
                .action_deadline_unix_ms
                .checked_sub(now)
                .ok_or_else(|| eyre!("host action deadline elapsed during admission"))?,
        ))
        .ok_or_else(|| eyre!("host monotonic action deadline overflow"))?;
    Ok((
        HostAdmission {
            request,
            request_sha256,
            inventory,
            inventory_sha256,
            authorization,
            authorization_sha256,
            target,
            guard,
            action_deadline,
            execution_expired,
        },
        chain_guard,
    ))
}

fn upload_parent(service_root: &str) -> String {
    Path::new(service_root)
        .join(".public-reset-upload-v1")
        .to_string_lossy()
        .into_owned()
}

pub(super) fn validate_first_release_physical_host(inventory: &InventoryV1) -> Result<()> {
    let expected = inventory.edge.endpoint.host_identity_sha256.as_str();
    if inventory
        .validators
        .iter()
        .any(|validator| validator.endpoint.host_identity_sha256 != expected)
    {
        return Err(eyre!(
            "first-release public reset requires edge and all four validator aliases on one exact physical SSH host identity"
        ));
    }
    Ok(())
}

fn host_request_identity_sha256(request: &HostRequestV1) -> Result<String> {
    let mut logical = request.clone();
    // The absolute deadline is independently bounded against the signed lease
    // at admission. It is intentionally excluded from the logical action key
    // so a crash retry can receive a fresh bounded process window without
    // duplicating the mutation or conflicting with its durable receipt.
    logical.action_deadline_unix_ms = 0;
    // Recovery is a zero-effect query for the receipt/evidence of the same
    // logical action and therefore shares its original durable action key.
    logical.recovery_only = false;
    let bytes =
        json::to_json(&logical).wrap_err("failed to encode logical host action identity")?;
    let mut digest = Sha256::new();
    digest.update(b"iroha:taira:public-reset:host-action:v1\0");
    digest.update(bytes.as_bytes());
    Ok(hex::encode(digest.finalize()))
}

fn validate_action_deadline(
    request: &HostRequestV1,
    action: HostAction,
    inventory: &InventoryV1,
    authorization: &AuthorizationEnvelopeV1,
    now_ms: u64,
) -> Result<()> {
    let action_window_ms = action
        .timeout_secs(inventory)
        .checked_mul(1_000)
        .and_then(|value| value.checked_add(super::MAX_CLOCK_SKEW_MS))
        .ok_or_else(|| eyre!("host action deadline overflow"))?;
    let execution_expired = now_ms
        > authorization
            .claims
            .execution_expires_at_unix_ms
            .saturating_add(super::MAX_CLOCK_SKEW_MS);
    let authorization_limit = if execution_expired
        || matches!(
            action,
            HostAction::Rollback | HostAction::Seal | HostAction::Cleanup
        ) {
        u64::MAX
    } else {
        authorization.claims.execution_expires_at_unix_ms
    };
    if request.action_deadline_unix_ms <= now_ms
        || request.action_deadline_unix_ms > now_ms.saturating_add(action_window_ms)
        || request.action_deadline_unix_ms > authorization_limit
    {
        return Err(eyre!(
            "host action deadline is expired or outside its signed execution bound"
        ));
    }
    Ok(())
}

fn validate_host_artifact_request(
    request: &HostRequestV1,
    target: &HostTarget,
    action: HostAction,
) -> Result<()> {
    if action == HostAction::Upload {
        let artifact = artifact(target.artifacts(), &request.artifact_role)?;
        if artifact.sha256 != request.artifact_sha256
            || artifact.size != request.artifact_size
            || artifact.mode != request.artifact_mode
        {
            return Err(eyre!(
                "upload request does not bind one exact inventory artifact"
            ));
        }
    } else if !request.artifact_role.is_empty()
        || !request.artifact_sha256.is_empty()
        || request.artifact_size != 0
        || request.artifact_mode != 0
    {
        return Err(eyre!("non-upload host request contains artifact fields"));
    }
    if action == HostAction::MutationReserve {
        if request.mutation_kind.is_empty()
            || request.mutation_phase.is_empty()
            || request.mutation_idempotency_key.is_empty()
            || request.mutation_operation.is_empty()
        {
            return Err(eyre!("prepared mutation request is incomplete"));
        }
        if request.mutation_operation == "prepare" {
            if request.mutation_prepared_base64.is_empty()
                || request.mutation_prepared_sha256.is_empty()
                || request.recovery_only
            {
                return Err(eyre!("prepared mutation publication is incomplete"));
            }
        } else if !request.mutation_prepared_base64.is_empty()
            || !request.mutation_prepared_sha256.is_empty()
            || !request.mutation_transaction_hash.is_empty()
        {
            return Err(eyre!(
                "prepared mutation state request contains candidate envelope fields"
            ));
        }
        if request.mutation_operation == "applied" && request.mutation_evidence_base64.is_empty() {
            return Err(eyre!(
                "prepared mutation Applied transition must carry exactly one canonical evidence report"
            ));
        }
        if !matches!(request.mutation_operation.as_str(), "prepare" | "applied")
            && !request.mutation_evidence_base64.is_empty()
        {
            return Err(eyre!(
                "only prepare or Applied transition may carry mutation evidence"
            ));
        }
        if request.recovery_only
            && !matches!(request.mutation_operation.as_str(), "fetch" | "applied")
        {
            return Err(eyre!(
                "prepared mutation recovery may only fetch or publish exact Applied evidence"
            ));
        }
    } else if !request.mutation_kind.is_empty()
        || !request.mutation_phase.is_empty()
        || !request.mutation_idempotency_key.is_empty()
        || !request.mutation_operation.is_empty()
        || !request.mutation_prepared_base64.is_empty()
        || !request.mutation_prepared_sha256.is_empty()
        || !request.mutation_transaction_hash.is_empty()
        || !request.mutation_evidence_base64.is_empty()
    {
        return Err(eyre!(
            "non-reservation host request contains mutation identity fields"
        ));
    }
    Ok(())
}

fn verify_dispatcher_identity(guard: &HostGuardV1) -> Result<()> {
    let executable = std::env::current_exe().wrap_err("failed to resolve host dispatcher")?;
    if executable != Path::new(FIXED_DISPATCHER) {
        return Err(eyre!(
            "host dispatcher is not running from its fixed provisioned path"
        ));
    }
    let (mut file, snapshot) = open_pinned_regular(&executable, "fixed host dispatcher")?;
    #[cfg(unix)]
    if snapshot.uid != 0 || snapshot.mode & 0o022 != 0 || snapshot.mode & 0o111 == 0 {
        return Err(eyre!("fixed host dispatcher custody is unsafe"));
    }
    let digest = hash_reader(&mut file)?;
    ensure_pinned_unchanged(&executable, "fixed host dispatcher", &file, &snapshot)?;
    if digest != guard.dispatcher_sha256 {
        return Err(eyre!(
            "fixed host dispatcher hash differs from provisioned guard"
        ));
    }
    Ok(())
}

fn require_stream_eof(body: &mut impl Read) -> Result<()> {
    let mut trailing = [0_u8; 1];
    if body.read(&mut trailing)? != 0 {
        return Err(eyre!(
            "non-upload host request contains trailing body bytes"
        ));
    }
    Ok(())
}

fn host_preflight(admitted: &HostAdmission) -> Result<()> {
    #[cfg(unix)]
    if rustix::process::geteuid().as_raw() != 0 {
        return Err(eyre!("fixed public-reset dispatcher must run as root"));
    }
    require_root_directory(
        Path::new(admitted.target.service_root()),
        false,
        "service root",
    )?;
    require_root_directory(Path::new(admitted.target.state_root()), false, "state root")?;
    require_root_directory(
        Path::new(admitted.target.reset_guard()),
        true,
        "reset guard",
    )?;
    require_root_directory(
        Path::new(&admitted.guard.upload_parent),
        true,
        "upload parent",
    )?;
    let current = validated_current_release_target(admitted)?;
    let expected_rollback = match &admitted.target {
        HostTarget::Validator(validator) => Path::new(&validator.rollback.release_root),
        HostTarget::Edge(edge) => Path::new(&edge.rollback_release_root),
    };
    if current != expected_rollback {
        return Err(eyre!(
            "host preflight current selector does not equal the signed rollback release"
        ));
    }
    let uname_system = run_host_command("/usr/bin/uname", &["-s"], admitted.action_deadline)?;
    let uname_machine = run_host_command("/usr/bin/uname", &["-m"], admitted.action_deadline)?;
    if uname_system != b"Linux\n" || uname_machine != b"aarch64\n" {
        return Err(eyre!("host platform is not exact Linux/AArch64"));
    }
    if matches!(admitted.target, HostTarget::Validator(_)) {
        require_kvm_api_v12()?;
    }
    match &admitted.target {
        HostTarget::Validator(validator) => {
            attest_loaded_systemd_unit(validator, admitted.action_deadline)?;
            require_root_directory(
                Path::new(&validator.rollback.release_root),
                false,
                "validator rollback release",
            )?;
            for (name, expected) in [
                ("bin/iroha3d_taira", &validator.rollback.iroha3d_sha256),
                ("bin/iroha", &validator.rollback.iroha_cli_sha256),
                ("bin/sorafs-node", &validator.rollback.sorafs_node_sha256),
                ("config/config.toml", &validator.rollback.config_sha256),
                ("genesis/genesis.json", &validator.rollback.genesis_sha256),
                (
                    "genesis/genesis.sha256",
                    &validator.rollback.genesis_hash_sha256,
                ),
            ] {
                verify_regular_hash(
                    &Path::new(&validator.rollback.release_root).join(name),
                    expected,
                )?;
            }
            let active = run_host_command(
                SYSTEMCTL,
                &[
                    "show",
                    "--property=ActiveState",
                    "--value",
                    &validator.systemd_unit,
                ],
                admitted.action_deadline,
            )?;
            if active != b"active\n" {
                return Err(eyre!(
                    "validator unit must be active before public reset so rollback preserves its signed preflight state"
                ));
            }
            attest_validator_process(
                admitted,
                validator,
                Path::new(&validator.rollback.release_root),
                false,
            )?;
        }
        HostTarget::Edge(edge) => {
            let root = Path::new(&edge.rollback_release_root);
            require_root_directory(root, false, "edge rollback release")?;
            verify_regular_hash(&root.join("bin/iroha"), &edge.rollback_cli_sha256)?;
            verify_regular_hash(&root.join("taira.conf"), &edge.rollback_edge_config_sha256)?;
            verify_regular_hash(
                Path::new(&edge.nginx_config),
                &edge.rollback_edge_config_sha256,
            )?;
            run_host_command(NGINX, &["-t"], admitted.action_deadline)?;
            let active = run_host_command(
                SYSTEMCTL,
                &["show", "--property=ActiveState", "--value", "nginx.service"],
                admitted.action_deadline,
            )?;
            if active != b"active\n" {
                return Err(eyre!(
                    "public edge nginx must be active with the signed rollback configuration"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[allow(
    unsafe_code,
    reason = "KVM_GET_API_VERSION is the fixed Linux host preflight ABI"
)]
fn require_kvm_api_v12() -> Result<()> {
    use std::os::fd::AsRawFd as _;
    unsafe extern "C" {
        fn ioctl(fd: i32, request: std::ffi::c_ulong, ...) -> i32;
    }
    const KVM_GET_API_VERSION: std::ffi::c_ulong = 0xAE00;
    let file = File::from(
        rustix::fs::open(
            "/dev/kvm",
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .wrap_err("failed to open fixed /dev/kvm")?,
    );
    if unsafe { ioctl(file.as_raw_fd(), KVM_GET_API_VERSION) } != 12 {
        return Err(eyre!("validator KVM API version is not exact 12"));
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn require_kvm_api_v12() -> Result<()> {
    Err(eyre!("public validator dispatcher requires Linux KVM"))
}

fn require_root_directory(path: &Path, private: bool, label: &str) -> Result<()> {
    require_root_no_symlink_ancestors(path, label)?;
    let metadata = fs::symlink_metadata(path)
        .wrap_err_with(|| format!("failed to inspect {label} `{}`", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(eyre!("{label} must be one direct directory"));
    }
    #[cfg(unix)]
    if metadata.uid() != 0
        || metadata.mode() & 0o022 != 0
        || (private && metadata.mode() & 0o7777 != 0o700)
    {
        return Err(eyre!("{label} has unsafe root custody"));
    }
    Ok(())
}

#[cfg(unix)]
fn require_root_no_symlink_ancestors(path: &Path, label: &str) -> Result<()> {
    if !path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::CurDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(eyre!("{label} must be an absolute normalized path"));
    }
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("{label} has no parent"))?;
    let mut cursor = PathBuf::new();
    for component in parent.components() {
        cursor.push(component.as_os_str());
        if cursor == Path::new("/") {
            continue;
        }
        let metadata = fs::symlink_metadata(&cursor).wrap_err_with(|| {
            format!("failed to inspect {label} ancestor `{}`", cursor.display())
        })?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || metadata.uid() != 0
            || metadata.mode() & 0o022 != 0
        {
            return Err(eyre!(
                "{label} ancestor `{}` has unsafe root custody",
                cursor.display()
            ));
        }
    }
    Ok(())
}

#[cfg(not(unix))]
fn require_root_no_symlink_ancestors(_path: &Path, _label: &str) -> Result<()> {
    Err(eyre!("public-reset host custody requires Unix"))
}

fn run_host_command(program: &str, args: &[&str], deadline: Instant) -> Result<Vec<u8>> {
    if deadline <= Instant::now() {
        return Err(eyre!(
            "signed host action deadline elapsed before child spawn"
        ));
    }
    let mut runner = RealProcessRunner;
    require_success(
        runner.run(&ProcessSpec {
            program: PathBuf::from(program),
            args: args.iter().map(OsString::from).collect(),
            stdin_prefix: Vec::new(),
            stdin_file: None,
            inherited_files: Vec::new(),
            deadline,
        })?,
        "fixed host command",
    )
}

fn verify_regular_hash(path: &Path, expected: &str) -> Result<()> {
    let (mut file, snapshot) = open_pinned_regular(path, "host artifact")?;
    let actual = hash_reader(&mut file)?;
    ensure_pinned_unchanged(path, "host artifact", &file, &snapshot)?;
    if actual != expected {
        return Err(eyre!("host artifact hash mismatch"));
    }
    Ok(())
}

fn sync_verified_regular_hash(path: &Path, expected: &str) -> Result<()> {
    let (mut file, snapshot) = open_pinned_regular(path, "host staging file")?;
    let actual = hash_reader(&mut file)?;
    if actual != expected {
        return Err(eyre!("host staging file hash mismatch"));
    }
    file.sync_all()?;
    ensure_pinned_unchanged(path, "host staging file", &file, &snapshot)
}

fn reconcile_verified_staging(
    path: &Path,
    expected_len: u64,
    expected_sha256: &str,
) -> Result<bool> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    #[cfg(unix)]
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.uid() != 0
        || metadata.nlink() != 1
    {
        return Err(eyre!("host staging file has unsafe custody"));
    }
    if metadata.len() < expected_len {
        let parent = path
            .parent()
            .ok_or_else(|| eyre!("host staging file has no parent"))?;
        fs::remove_file(path)?;
        sync_directory(parent)?;
        return Ok(false);
    }
    if metadata.len() != expected_len {
        return Err(eyre!("host staging file has a conflicting length"));
    }
    sync_verified_regular_hash(path, expected_sha256)?;
    Ok(true)
}

fn ensure_host_lease(admitted: &HostAdmission, action: HostAction) -> Result<()> {
    let guard = host_coordination_root(admitted)?;
    let lease_path = guard.join("lease.json");
    let now = now_unix_ms()?;
    if lease_path.exists() {
        let (lease, bytes) = read_private_json::<HostLeaseV1>(&lease_path, "host lease")?;
        require_lower_sha256(
            &lease.authorization_semantic_sha256,
            "loaded host lease authorization digest",
        )?;
        require_lower_sha256(
            &lease.inventory_sha256,
            "loaded host lease inventory digest",
        )?;
        if lease.schema == LEASE_SCHEMA_V1
            && lease.inventory_sha256 == admitted.inventory_sha256
            && lease.authorization_semantic_sha256 == admitted.authorization_sha256
            && lease.authorization_nonce == admitted.inventory.authorization_nonce
            && lease.execution_expires_at_unix_ms
                == admitted.authorization.claims.execution_expires_at_unix_ms
        {
            drop(bytes);
            return Ok(());
        }
        if action == HostAction::Rollback || admitted.execution_expired {
            return Err(eyre!(
                "expired or rollback recovery cannot replace a foreign host lease"
            ));
        }
        if !expired_host_session_releasable(&guard, &lease, now)? {
            return Err(eyre!(
                "another authorization owns a nonterminal host deployment session"
            ));
        }
        let expired_dir = guard.join("expired-leases");
        ensure_root_private_directory(&expired_dir)?;
        archive_expired_host_progress(&guard, &expired_dir, &lease)?;
        let destination = expired_dir.join(format!("{}.json", lease.authorization_semantic_sha256));
        archive_file_noreplace_exact(&lease_path, &destination)
            .wrap_err("failed to archive released host deployment lease")?;
    } else if action == HostAction::Rollback || admitted.execution_expired {
        return Err(eyre!(
            "expired recovery requires an already-acquired matching host lease"
        ));
    }
    let lease = HostLeaseV1 {
        schema: LEASE_SCHEMA_V1.to_owned(),
        inventory_sha256: admitted.inventory_sha256.clone(),
        authorization_semantic_sha256: admitted.authorization_sha256.clone(),
        authorization_nonce: admitted.inventory.authorization_nonce.clone(),
        execution_expires_at_unix_ms: admitted.authorization.claims.execution_expires_at_unix_ms,
    };
    let bytes = json::to_json(&lease)?.into_bytes();
    publish_host_lease(&guard, &admitted.authorization_sha256, &bytes)
}

fn expired_host_session_releasable(
    guard: &Path,
    lease: &HostLeaseV1,
    now_unix_ms: u64,
) -> Result<bool> {
    if now_unix_ms <= lease.execution_expires_at_unix_ms {
        return Ok(false);
    }
    let parent = File::open(guard)?;
    let mut saw_progress = false;
    for name in ["progress.json", "progress.successor.json"] {
        let Some(bytes) = read_regular_at(&parent, name, MAX_HOST_REQUEST_BYTES)? else {
            continue;
        };
        saw_progress = true;
        let progress: HostProgressV1 =
            json::from_slice(&bytes).wrap_err("released host progress is not exact V1 JSON")?;
        if progress.schema != HOST_PROGRESS_SCHEMA_V1
            || progress.inventory_sha256 != lease.inventory_sha256
            || progress.authorization_sha256 != lease.authorization_semantic_sha256
            || progress.authorization_nonce != lease.authorization_nonce
        {
            return Err(eyre!(
                "released host progress is not bound to its exact lease"
            ));
        }
        let untouched = progress.touched_hosts.is_empty();
        let sealed = progress.sealed && progress.prepared_action.is_none();
        let rolled_back = progress.rolling_back
            && progress.prepared_action.is_none()
            && progress.rolled_back_hosts.len() == progress.touched_hosts.len()
            && progress
                .rolled_back_hosts
                .iter()
                .all(|slug| progress.touched_hosts.contains(slug));
        if !(untouched || sealed || rolled_back) {
            return Ok(false);
        }
    }
    // A lease can crash before its initial progress publication. With no
    // touched state and an expired signed execution window, it is releasable.
    Ok(saw_progress || !guard.join("progress.json").exists())
}

fn publish_host_lease(guard: &Path, authorization_sha256: &str, bytes: &[u8]) -> Result<()> {
    require_lower_sha256(
        authorization_sha256,
        "host lease candidate authorization digest",
    )?;
    let candidate_name = format!("lease-candidate-{authorization_sha256}.json");
    publish_root_private_noreplace(guard, &candidate_name, bytes)?;
    let candidate = guard.join(&candidate_name);
    let destination = guard.join("lease.json");
    match rename_noreplace(&candidate, &destination) {
        Ok(()) => sync_directory(guard),
        Err(error) if destination.exists() => {
            let parent = File::open(guard)?;
            let actual = read_regular_at(&parent, "lease.json", bytes.len().max(1) + 1)?
                .ok_or_else(|| eyre!("host lease vanished during publication"))?;
            if actual != bytes {
                return Err(error).wrap_err("host lease race published different bytes");
            }
            archive_file_noreplace_exact(
                &candidate,
                &guard.join(format!(
                    "lease-candidate-{authorization_sha256}.replayed.json"
                )),
            )?;
            sync_directory(guard)
        }
        Err(error) => Err(error),
    }
}

fn archive_expired_host_progress(
    guard: &Path,
    expired_dir: &Path,
    lease: &HostLeaseV1,
) -> Result<()> {
    for (source_name, suffix, may_be_partial) in [
        ("progress.json", "progress.json", false),
        ("progress.successor.json", "progress.successor.json", false),
        (".progress.json.next", "progress.next.partial", true),
        (
            ".progress.successor.json.next",
            "progress.successor.next.partial",
            true,
        ),
    ] {
        let source = guard.join(source_name);
        if !source.exists() {
            continue;
        }
        let parent = File::open(guard)?;
        let bytes = read_regular_at(&parent, source_name, MAX_HOST_REQUEST_BYTES)?
            .ok_or_else(|| eyre!("expired host progress vanished during rotation"))?;
        match json::from_slice::<HostProgressV1>(&bytes) {
            Ok(progress)
                if progress.schema == HOST_PROGRESS_SCHEMA_V1
                    && progress.inventory_sha256 == lease.inventory_sha256
                    && progress.authorization_sha256 == lease.authorization_semantic_sha256
                    && progress.authorization_nonce == lease.authorization_nonce => {}
            Ok(_) => {
                return Err(eyre!(
                    "expired host progress is not bound to the lease being rotated"
                ));
            }
            Err(_) if may_be_partial => {}
            Err(error) => {
                return Err(error).wrap_err("expired host progress JSON is invalid");
            }
        }
        let destination = expired_dir.join(format!(
            "{}.{}",
            lease.authorization_semantic_sha256, suffix
        ));
        archive_file_noreplace_exact(&source, &destination)
            .wrap_err("failed to archive expired host progress closure")?;
    }
    sync_directory(guard)?;
    sync_directory(expired_dir)
}

fn archive_file_noreplace_exact(source: &Path, destination: &Path) -> Result<()> {
    if !source.exists() {
        return Ok(());
    }
    if destination.exists() {
        let source_parent = File::open(
            source
                .parent()
                .ok_or_else(|| eyre!("archive source has no parent"))?,
        )?;
        let destination_parent = File::open(
            destination
                .parent()
                .ok_or_else(|| eyre!("archive destination has no parent"))?,
        )?;
        let source_name = source
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| eyre!("archive source name is not UTF-8"))?;
        let destination_name = destination
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| eyre!("archive destination name is not UTF-8"))?;
        let source_bytes = read_regular_at(&source_parent, source_name, MAX_HOST_REQUEST_BYTES)?
            .ok_or_else(|| eyre!("archive source vanished"))?;
        let destination_bytes = read_regular_at(
            &destination_parent,
            destination_name,
            MAX_HOST_REQUEST_BYTES,
        )?
        .ok_or_else(|| eyre!("archive destination vanished"))?;
        if source_bytes != destination_bytes {
            return Err(eyre!("archive destination conflicts with source bytes"));
        }
        rustix::fs::unlinkat(&source_parent, source_name, rustix::fs::AtFlags::empty())?;
        source_parent.sync_all()?;
        return Ok(());
    }
    rename_noreplace(source, destination)?;
    sync_directory(source.parent().expect("archive source has parent"))?;
    sync_directory(
        destination
            .parent()
            .expect("archive destination has parent"),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HostProgressDecision {
    Advance,
    Replay,
    AbsentNoOp,
}

fn host_action_key(admitted: &HostAdmission, action: HostAction) -> HostActionKeyV1 {
    HostActionKeyV1 {
        host_slug: admitted.target.slug().to_owned(),
        action: action.label().to_owned(),
        artifact_role: if action == HostAction::Upload {
            admitted.request.artifact_role.clone()
        } else {
            String::new()
        },
    }
}

fn host_forward_plan(admitted: &HostAdmission) -> Vec<HostActionKeyV1> {
    let identity = admitted.target.endpoint().host_identity_sha256.as_str();
    let validators = admitted
        .inventory
        .validators
        .iter()
        .filter(|validator| validator.endpoint.host_identity_sha256 == identity)
        .collect::<Vec<_>>();
    let edge = (admitted.inventory.edge.endpoint.host_identity_sha256 == identity)
        .then_some(&admitted.inventory.edge);
    let mut plan = Vec::new();
    for validator in &validators {
        for artifact in &validator.artifacts {
            plan.push(HostActionKeyV1 {
                host_slug: validator.slug.clone(),
                action: HostAction::Upload.label().to_owned(),
                artifact_role: artifact.role.clone(),
            });
        }
        plan.push(HostActionKeyV1 {
            host_slug: validator.slug.clone(),
            action: HostAction::Stage.label().to_owned(),
            artifact_role: String::new(),
        });
    }
    for action in [
        HostAction::Stop,
        HostAction::Install,
        HostAction::Reset,
        HostAction::Start,
        HostAction::Restart,
    ] {
        for validator in &validators {
            plan.push(HostActionKeyV1 {
                host_slug: validator.slug.clone(),
                action: action.label().to_owned(),
                artifact_role: String::new(),
            });
        }
    }
    if let Some(edge) = edge {
        for artifact in &edge.artifacts {
            plan.push(HostActionKeyV1 {
                host_slug: edge.slug.clone(),
                action: HostAction::Upload.label().to_owned(),
                artifact_role: artifact.role.clone(),
            });
        }
        for action in [
            HostAction::EdgeStage,
            HostAction::EdgeCutover,
            HostAction::EdgeVerify,
        ] {
            plan.push(HostActionKeyV1 {
                host_slug: edge.slug.clone(),
                action: action.label().to_owned(),
                artifact_role: String::new(),
            });
        }
    }
    for validator in &validators {
        plan.push(HostActionKeyV1 {
            host_slug: validator.slug.clone(),
            action: HostAction::Seal.label().to_owned(),
            artifact_role: String::new(),
        });
    }
    if let Some(edge) = edge {
        plan.push(HostActionKeyV1 {
            host_slug: edge.slug.clone(),
            action: HostAction::Seal.label().to_owned(),
            artifact_role: String::new(),
        });
    }
    for validator in validators {
        plan.push(HostActionKeyV1 {
            host_slug: validator.slug.clone(),
            action: HostAction::Cleanup.label().to_owned(),
            artifact_role: String::new(),
        });
    }
    if let Some(edge) = edge {
        plan.push(HostActionKeyV1 {
            host_slug: edge.slug.clone(),
            action: HostAction::Cleanup.label().to_owned(),
            artifact_role: String::new(),
        });
    }
    plan
}

fn initial_host_progress(admitted: &HostAdmission) -> HostProgressV1 {
    HostProgressV1 {
        schema: HOST_PROGRESS_SCHEMA_V1.to_owned(),
        inventory_sha256: admitted.inventory_sha256.clone(),
        authorization_sha256: admitted.authorization_sha256.clone(),
        authorization_nonce: admitted.inventory.authorization_nonce.clone(),
        next_forward_ordinal: 0,
        prepared_action: None,
        touched_hosts: Vec::new(),
        sealed: false,
        rolling_back: false,
        last_rollback_rank: 0,
        rolled_back_hosts: Vec::new(),
    }
}

fn validate_host_progress(admitted: &HostAdmission, progress: &HostProgressV1) -> Result<()> {
    let plan_len = host_forward_plan(admitted).len();
    if progress.schema != HOST_PROGRESS_SCHEMA_V1
        || progress.inventory_sha256 != admitted.inventory_sha256
        || progress.authorization_sha256 != admitted.authorization_sha256
        || progress.authorization_nonce != admitted.inventory.authorization_nonce
        || usize::from(progress.next_forward_ordinal) > plan_len
        || progress.last_rollback_rank > 5
        || progress.touched_hosts.len() > 5
        || progress.rolled_back_hosts.len() > 5
        || (!progress.rolling_back
            && (progress.last_rollback_rank != 0 || !progress.rolled_back_hosts.is_empty()))
    {
        return Err(eyre!(
            "host progress is not this exact authorized execution"
        ));
    }
    let plan = host_forward_plan(admitted);
    if let Some(prepared) = &progress.prepared_action
        && plan.get(usize::from(progress.next_forward_ordinal)) != Some(prepared)
        && prepared.action != HostAction::Rollback.label()
    {
        return Err(eyre!("host prepared action is not the exact next action"));
    }
    let mut unique_touched = BTreeSet::new();
    if progress.touched_hosts.iter().any(|slug| {
        !unique_touched.insert(slug) || rollback_rank(&admitted.inventory, slug).is_none()
    }) {
        return Err(eyre!("host touched-target progress is not canonical"));
    }
    let targets = required_rollback_targets(admitted, progress)?;
    if progress.rolled_back_hosts
        != targets
            .iter()
            .take(progress.rolled_back_hosts.len())
            .cloned()
            .collect::<Vec<_>>()
    {
        return Err(eyre!("host rollback progress is not canonical"));
    }
    let expected_last_rank = progress
        .rolled_back_hosts
        .last()
        .and_then(|slug| rollback_rank(&admitted.inventory, slug))
        .unwrap_or(0);
    if progress.last_rollback_rank != expected_last_rank {
        return Err(eyre!(
            "host rollback rank does not match its exact target prefix"
        ));
    }
    let first_cleanup = plan
        .iter()
        .position(|key| key.action == HostAction::Cleanup.label())
        .unwrap_or(plan.len());
    if progress.sealed != (usize::from(progress.next_forward_ordinal) >= first_cleanup) {
        return Err(eyre!(
            "host success seal does not match its exact forward phase"
        ));
    }
    Ok(())
}

fn load_or_create_host_progress(admitted: &HostAdmission) -> Result<HostProgressV1> {
    let root = host_coordination_root(admitted)?;
    let path = root.join("progress.json");
    if path.exists() {
        let (progress, _) = read_private_json::<HostProgressV1>(&path, "host progress")?;
        validate_host_progress(admitted, &progress)?;
        return Ok(progress);
    }
    let progress = initial_host_progress(admitted);
    publish_root_private_noreplace(&root, "progress.json", json::to_json(&progress)?.as_bytes())?;
    Ok(progress)
}

fn admit_host_action_progress(
    admitted: &HostAdmission,
    action: HostAction,
    progress: &HostProgressV1,
) -> Result<HostProgressDecision> {
    validate_host_progress(admitted, progress)?;
    if action == HostAction::Rollback {
        if progress.sealed
            || host_forward_plan(admitted)
                .get(..usize::from(progress.next_forward_ordinal))
                .is_some_and(|completed| {
                    completed
                        .iter()
                        .any(|key| key.action == HostAction::Seal.label())
                })
            || progress
                .prepared_action
                .as_ref()
                .is_some_and(|key| key.action == HostAction::Seal.label())
        {
            return Err(eyre!(
                "host deployment success is sealing or sealed; rollback is terminally rejected"
            ));
        }
        if progress
            .rolled_back_hosts
            .iter()
            .any(|slug| slug == admitted.target.slug())
        {
            return Ok(HostProgressDecision::Replay);
        }
        let targets = required_rollback_targets(admitted, progress)?;
        if !progress
            .touched_hosts
            .iter()
            .any(|slug| slug == admitted.target.slug())
        {
            return Ok(HostProgressDecision::AbsentNoOp);
        }
        let expected = targets
            .get(progress.rolled_back_hosts.len())
            .ok_or_else(|| {
                eyre!("host rollback has no remaining touched target on this physical host")
            })?;
        if admitted.target.slug() != expected {
            return Err(eyre!(
                "host rollback target is not the exact next touched target `{expected}`"
            ));
        }
        return Ok(HostProgressDecision::Advance);
    }
    if progress.rolling_back {
        return Err(eyre!(
            "host execution is rolling back; every forward action is terminally rejected"
        ));
    }
    let key = host_action_key(admitted, action);
    if let Some(prepared) = &progress.prepared_action
        && prepared != &key
    {
        return Err(eyre!(
            "host action cannot supersede a different durably prepared action"
        ));
    }
    let plan = host_forward_plan(admitted);
    let ordinal = plan
        .iter()
        .position(|expected| expected == &key)
        .ok_or_else(|| eyre!("host action is not present in the exact physical-host plan"))?;
    match ordinal.cmp(&usize::from(progress.next_forward_ordinal)) {
        std::cmp::Ordering::Less => Ok(HostProgressDecision::Replay),
        std::cmp::Ordering::Equal => Ok(HostProgressDecision::Advance),
        std::cmp::Ordering::Greater => Err(eyre!(
            "host action violates the exact monotonic physical-host phase order"
        )),
    }
}

fn prepare_host_progress(
    admitted: &HostAdmission,
    action: HostAction,
    progress: &mut HostProgressV1,
) -> Result<()> {
    let key = host_action_key(admitted, action);
    if progress.prepared_action.as_ref() == Some(&key) {
        return Ok(());
    }
    if progress.prepared_action.is_some() && action != HostAction::Rollback {
        return Err(eyre!(
            "host action cannot supersede a different durably prepared action"
        ));
    }
    match admit_host_action_progress(admitted, action, progress)? {
        HostProgressDecision::Replay => {
            return Err(eyre!("completed host action cannot be prepared again"));
        }
        HostProgressDecision::Advance => {}
        HostProgressDecision::AbsentNoOp => {
            return Err(eyre!("absent rollback target cannot prepare a mutation"));
        }
    }
    if action != HostAction::Rollback && host_action_touches_live_state(&key) {
        if !progress
            .touched_hosts
            .iter()
            .any(|slug| slug == &key.host_slug)
        {
            progress.touched_hosts.push(key.host_slug.clone());
        }
    }
    progress.prepared_action = Some(key);
    if action == HostAction::Rollback {
        progress.rolling_back = true;
    }
    replace_host_progress(admitted, progress)
}

fn advance_host_progress(
    admitted: &HostAdmission,
    action: HostAction,
    progress: &mut HostProgressV1,
) -> Result<()> {
    match admit_host_action_progress(admitted, action, progress)? {
        HostProgressDecision::Replay => return Ok(()),
        HostProgressDecision::Advance => {}
        HostProgressDecision::AbsentNoOp => {
            return Err(eyre!(
                "absent rollback target cannot advance mutation progress"
            ));
        }
    }
    if action == HostAction::Rollback {
        let rank = rollback_rank(&admitted.inventory, admitted.target.slug())
            .expect("admitted rollback rank");
        progress.rolling_back = true;
        progress.last_rollback_rank = rank;
        progress
            .rolled_back_hosts
            .push(admitted.target.slug().to_owned());
    } else {
        progress.next_forward_ordinal = progress
            .next_forward_ordinal
            .checked_add(1)
            .ok_or_else(|| eyre!("host progress ordinal overflow"))?;
        let plan = host_forward_plan(admitted);
        let first_cleanup = plan
            .iter()
            .position(|key| key.action == HostAction::Cleanup.label())
            .unwrap_or(plan.len());
        progress.sealed = usize::from(progress.next_forward_ordinal) >= first_cleanup;
    }
    progress.prepared_action = None;
    replace_host_progress(admitted, progress)
}

fn rollback_rank(inventory: &InventoryV1, slug: &str) -> Option<u8> {
    if slug == inventory.edge.slug {
        return Some(5);
    }
    super::VALIDATOR_SLUGS
        .iter()
        .position(|candidate| *candidate == slug)
        .and_then(|index| u8::try_from(index + 1).ok())
}

fn host_action_touches_live_state(key: &HostActionKeyV1) -> bool {
    matches!(
        key.action.as_str(),
        "stop" | "install" | "reset" | "start" | "restart" | "edge_cutover" | "edge_verify"
    )
}

fn required_rollback_targets(
    admitted: &HostAdmission,
    progress: &HostProgressV1,
) -> Result<Vec<String>> {
    let mut ranked = progress
        .touched_hosts
        .iter()
        .cloned()
        .map(|slug| match rollback_rank(&admitted.inventory, &slug) {
            Some(rank) => Ok((rank, slug)),
            None => Err(eyre!(
                "touched host `{slug}` has no canonical rollback rank"
            )),
        })
        .collect::<Result<Vec<_>>>()?;
    ranked.sort_by(|left, right| right.0.cmp(&left.0));
    Ok(ranked.into_iter().map(|(_, slug)| slug).collect())
}

fn replace_host_progress(admitted: &HostAdmission, progress: &HostProgressV1) -> Result<()> {
    validate_host_progress(admitted, progress)?;
    let root = host_coordination_root(admitted)?;
    let bytes = json::to_json(progress)?.into_bytes();
    publish_root_private_noreplace(&root, "progress.successor.json", &bytes)?;
    let directory = File::open(&root)?;
    rustix::fs::renameat_with(
        &directory,
        "progress.successor.json",
        &directory,
        "progress.json",
        rustix::fs::RenameFlags::empty(),
    )?;
    directory.sync_all()?;
    Ok(())
}

fn require_lower_sha256(value: &str, label: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(eyre!("{label} is not lowercase SHA-256 hexadecimal"));
    }
    Ok(())
}

fn lock_host_action(admitted: &HostAdmission) -> Result<File> {
    let guard = host_coordination_root(admitted)?;
    require_root_directory(&guard, true, "host-global coordination root")?;
    let file = File::from(
        rustix::fs::openat(
            File::open(&guard).wrap_err("failed to open host-global coordination root")?,
            "action.lock",
            rustix::fs::OFlags::RDWR
                | rustix::fs::OFlags::CREATE
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::from_raw_mode(0o600),
        )
        .wrap_err("failed to open host action lock")?,
    );
    let metadata = file.metadata()?;
    #[cfg(unix)]
    if !metadata.is_file()
        || metadata.uid() != 0
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(eyre!("host action lock has unsafe custody"));
    }
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) => {
                let remaining = admitted
                    .action_deadline
                    .saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(eyre!(
                        "{HOST_ACTION_IN_PROGRESS_MARKER}: matching host action is still in progress"
                    ));
                }
                std::thread::sleep(PROCESS_POLL_INTERVAL.min(remaining));
            }
            Err(std::fs::TryLockError::Error(error)) => {
                return Err(error).wrap_err("failed to acquire the host-global action lock");
            }
        }
    }
}

fn host_coordination_root(admitted: &HostAdmission) -> Result<PathBuf> {
    let guard = Path::new(admitted.target.reset_guard());
    let control = guard
        .parent()
        .ok_or_else(|| eyre!("reset guard has no host-global control parent"))?;
    if control != Path::new("/var/lib/taira/.public-reset-control-v1") {
        return Err(eyre!(
            "reset guard escaped the fixed host-global control root"
        ));
    }
    require_root_directory(control, true, "host-global control root")?;
    let hosts = control.join("hosts");
    ensure_root_directory_with_mode(&hosts, 0o700)?;
    let root = hosts.join(&admitted.target.endpoint().host_identity_sha256);
    ensure_root_directory_with_mode(&root, 0o700)?;
    Ok(root)
}

fn ensure_root_private_directory(path: &Path) -> Result<()> {
    if !path.exists() {
        fs::create_dir(path).wrap_err_with(|| format!("failed to create `{}`", path.display()))?;
        #[cfg(unix)]
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        sync_directory(
            path.parent()
                .ok_or_else(|| eyre!("directory has no parent"))?,
        )?;
    }
    require_root_directory(path, true, "generated root-private directory")
}

fn ensure_host_receipt_dir(admitted: &HostAdmission) -> Result<PathBuf> {
    let receipts = Path::new(admitted.target.reset_guard()).join("receipts");
    ensure_root_private_directory(&receipts)?;
    let nonce = receipts.join(&admitted.inventory.authorization_nonce);
    ensure_root_private_directory(&nonce)?;
    Ok(nonce)
}

fn host_receipt_name(action: HostAction, role: &str) -> Result<String> {
    let name = if action == HostAction::Upload {
        staged_artifact_name(role)?;
        format!("upload-{role}.json")
    } else {
        format!("{}.json", action.label())
    };
    validate_receipt_name(&name)?;
    Ok(name)
}

fn read_existing_host_receipt(
    directory: &Path,
    name: &str,
    admitted: &HostAdmission,
    action: HostAction,
) -> Result<Option<HostReceiptV1>> {
    reconcile_host_receipt_staging(directory, name, admitted, action)?;
    let path = directory.join(name);
    if !path.exists() {
        return Ok(None);
    }
    let (receipt, _) = read_private_json::<HostReceiptV1>(&path, "host action receipt")?;
    if receipt.schema != HOST_RECEIPT_SCHEMA_V1
        || receipt.action != action.label()
        || receipt.host_slug != admitted.target.slug()
        || receipt.request_sha256 != admitted.request_sha256
        || receipt.inventory_sha256 != admitted.inventory_sha256
        || receipt.authorization_sha256 != admitted.authorization_sha256
        || receipt.authorization_nonce != admitted.inventory.authorization_nonce
        || receipt.status != "ok"
    {
        return Err(eyre!(
            "existing host receipt is not an exact idempotent retry"
        ));
    }
    Ok(Some(receipt))
}

fn reconcile_host_receipt_staging(
    directory: &Path,
    name: &str,
    admitted: &HostAdmission,
    action: HostAction,
) -> Result<()> {
    let parent = File::from(rustix::fs::open(
        directory,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?);
    let staging_name = format!(".{name}.next");
    let Some(bytes) = read_regular_at(&parent, &staging_name, MAX_HOST_REQUEST_BYTES)? else {
        return Ok(());
    };
    let complete = json::from_slice::<HostReceiptV1>(&bytes)
        .ok()
        .is_some_and(|receipt| {
            receipt.schema == HOST_RECEIPT_SCHEMA_V1
                && receipt.action == action.label()
                && receipt.host_slug == admitted.target.slug()
                && receipt.request_sha256 == admitted.request_sha256
                && receipt.inventory_sha256 == admitted.inventory_sha256
                && receipt.authorization_sha256 == admitted.authorization_sha256
                && receipt.authorization_nonce == admitted.inventory.authorization_nonce
                && receipt.status == "ok"
        });
    if complete {
        sync_regular_at(&parent, &staging_name)?;
        match rustix::fs::renameat_with(
            &parent,
            staging_name.as_str(),
            &parent,
            name,
            rustix::fs::RenameFlags::NOREPLACE,
        ) {
            Ok(()) => {}
            Err(rustix::io::Errno::EXIST) => {
                let actual = read_regular_at(&parent, name, MAX_HOST_REQUEST_BYTES)?
                    .ok_or_else(|| eyre!("host receipt vanished during recovery"))?;
                if actual != bytes {
                    return Err(eyre!("staged host receipt conflicts with destination"));
                }
                sync_regular_at(&parent, name)?;
                rustix::fs::unlinkat(&parent, staging_name.as_str(), rustix::fs::AtFlags::empty())?;
            }
            Err(error) => return Err(error.into()),
        }
    } else {
        rustix::fs::unlinkat(&parent, staging_name.as_str(), rustix::fs::AtFlags::empty())?;
    }
    parent.sync_all()?;
    Ok(())
}

fn revalidate_cached_action_postcondition(
    admitted: &HostAdmission,
    action: HostAction,
) -> Result<()> {
    ensure_action_deadline(admitted)?;
    let candidate = Path::new(admitted.target.service_root())
        .join("releases")
        .join(&admitted.inventory.revision.commit);
    match action {
        HostAction::Preflight => Err(eyre!("preflight does not have a durable action receipt")),
        HostAction::MutationReserve => Err(eyre!(
            "mutation reservation has no reusable action postcondition"
        )),
        HostAction::Upload => {
            let path = staged_artifact_path(admitted, &admitted.request.artifact_role)?;
            let expected = artifact(admitted.target.artifacts(), &admitted.request.artifact_role)?;
            verify_regular_hash(&path, &expected.sha256)
        }
        HostAction::Stage | HostAction::EdgeStage => verify_staged_artifacts(admitted),
        HostAction::Stop => {
            let HostTarget::Validator(validator) = &admitted.target else {
                return Err(eyre!("cached stop receipt requires a validator target"));
            };
            require_session_manager_operation_applied(
                admitted,
                "stop",
                "stop",
                &validator.systemd_unit,
            )?;
            require_unit_stopped(&validator.systemd_unit, admitted.action_deadline)
        }
        HostAction::Install => {
            verify_installed_release(admitted, &candidate)?;
            if validated_current_release_target(admitted)? != candidate {
                return Err(eyre!(
                    "cached install selector no longer selects the candidate"
                ));
            }
            Ok(())
        }
        HostAction::Reset => verify_fresh_state(admitted),
        HostAction::Start | HostAction::Restart => {
            let HostTarget::Validator(validator) = &admitted.target else {
                return Err(eyre!("cached active receipt requires a validator target"));
            };
            let label = if action == HostAction::Start {
                "start"
            } else {
                "restart"
            };
            require_session_manager_operation_applied(
                admitted,
                label,
                label,
                &validator.systemd_unit,
            )?;
            require_unit_active(&validator.systemd_unit, admitted.action_deadline)?;
            attest_validator_process(admitted, validator, &candidate, true)
        }
        HostAction::EdgeCutover | HostAction::EdgeVerify => {
            let HostTarget::Edge(edge) = &admitted.target else {
                return Err(eyre!("cached edge receipt requires the edge target"));
            };
            verify_installed_release(admitted, &candidate)?;
            if validated_current_release_target(admitted)? != candidate {
                return Err(eyre!(
                    "cached edge selector no longer selects the candidate"
                ));
            }
            verify_regular_hash(
                Path::new(&edge.nginx_config),
                &artifact(&edge.artifacts, "edge_config")?.sha256,
            )?;
            require_session_manager_operation_applied(
                admitted,
                "edge-cutover-reload",
                "reload",
                "nginx.service",
            )?;
            require_unit_active("nginx.service", admitted.action_deadline)?;
            run_host_command(NGINX, &["-t"], admitted.action_deadline)?;
            Ok(())
        }
        HostAction::Seal => verify_success_seal_postcondition(admitted),
        // Cleanup replay proves only the immutable plan absent. A later aged
        // candidate is outside the already-published receipt and is untouched.
        HostAction::Cleanup => verify_completed_cleanup_plan(admitted, None),
        HostAction::Rollback => verify_rollback_postcondition(admitted),
    }
}

fn revalidate_current_target_postcondition(
    admitted: &HostAdmission,
    progress: &HostProgressV1,
) -> Result<()> {
    let plan = host_forward_plan(admitted);
    let completed = plan
        .get(..usize::from(progress.next_forward_ordinal))
        .ok_or_else(|| eyre!("host progress ordinal exceeds its exact plan"))?;
    let current = completed
        .iter()
        .rev()
        .find(|key| {
            key.host_slug == admitted.target.slug()
                && !matches!(
                    key.action.as_str(),
                    "upload" | "stage" | "edge_stage" | "cleanup"
                )
        })
        .map(|key| HostAction::parse(&key.action))
        .transpose()?;
    match current {
        Some(HostAction::Cleanup) => Ok(()),
        Some(action) => revalidate_cached_action_postcondition(admitted, action),
        None => host_preflight(admitted),
    }
}

fn verify_conservative_rollback_absence(admitted: &HostAdmission) -> Result<()> {
    let selected = validated_current_release_target(admitted)?;
    match &admitted.target {
        HostTarget::Validator(validator) => {
            if selected != Path::new(&validator.rollback.release_root) {
                return Err(eyre!(
                    "conservative rollback target has a non-rollback current selector"
                ));
            }
            let state = Path::new(&validator.state_root);
            if state.join(".public-reset-generated-v1.json").exists() {
                return Err(eyre!(
                    "conservative rollback target still has authorization-generated state"
                ));
            }
            require_unit_active(&validator.systemd_unit, admitted.action_deadline)?;
            attest_validator_process(admitted, validator, &selected, false)
        }
        HostTarget::Edge(edge) => {
            if selected != Path::new(&edge.rollback_release_root) {
                return Err(eyre!(
                    "conservative edge rollback has a non-rollback current selector"
                ));
            }
            verify_regular_hash(
                Path::new(&edge.nginx_config),
                &edge.rollback_edge_config_sha256,
            )?;
            require_unit_active("nginx.service", admitted.action_deadline)?;
            run_host_command(NGINX, &["-t"], admitted.action_deadline)?;
            Ok(())
        }
    }
}

fn verify_success_seal_postcondition(admitted: &HostAdmission) -> Result<()> {
    let candidate = Path::new(admitted.target.service_root())
        .join("releases")
        .join(&admitted.inventory.revision.commit);
    verify_installed_release(admitted, &candidate)?;
    if validated_current_release_target(admitted)? != candidate {
        return Err(eyre!(
            "success seal selector no longer selects the candidate"
        ));
    }
    match &admitted.target {
        HostTarget::Validator(validator) => {
            require_unit_active(&validator.systemd_unit, admitted.action_deadline)?;
            attest_validator_process(admitted, validator, &candidate, true)
        }
        HostTarget::Edge(edge) => {
            verify_regular_hash(
                Path::new(&edge.nginx_config),
                &artifact(&edge.artifacts, "edge_config")?.sha256,
            )?;
            require_session_manager_operation_applied(
                admitted,
                "edge-cutover-reload",
                "reload",
                "nginx.service",
            )?;
            require_unit_active("nginx.service", admitted.action_deadline)?;
            run_host_command(NGINX, &["-t"], admitted.action_deadline)?;
            Ok(())
        }
    }
}

fn host_receipt(
    admitted: &HostAdmission,
    action: HostAction,
    idempotent: bool,
    bytes_before: u64,
    bytes_after: u64,
    detail: &str,
) -> HostReceiptV1 {
    HostReceiptV1 {
        schema: HOST_RECEIPT_SCHEMA_V1.to_owned(),
        action: action.label().to_owned(),
        host_slug: admitted.target.slug().to_owned(),
        request_sha256: admitted.request_sha256.clone(),
        inventory_sha256: admitted.inventory_sha256.clone(),
        authorization_sha256: admitted.authorization_sha256.clone(),
        authorization_nonce: admitted.inventory.authorization_nonce.clone(),
        status: "ok".to_owned(),
        idempotent,
        bytes_before,
        bytes_after,
        reclaimed_bytes: bytes_before.saturating_sub(bytes_after),
        detail: detail.to_owned(),
        mutation_state: String::new(),
        mutation_prepared_base64: String::new(),
        mutation_prepared_sha256: String::new(),
        mutation_transaction_hash: String::new(),
    }
}

fn host_recovery_receipt(
    admitted: &HostAdmission,
    action: HostAction,
    status: &str,
    detail: &str,
) -> HostReceiptV1 {
    let mut receipt = host_receipt(admitted, action, true, 0, 0, detail);
    receipt.status = status.to_owned();
    receipt
}

fn publish_host_receipt(directory: &Path, name: &str, receipt: &HostReceiptV1) -> Result<()> {
    let bytes = json::to_json(receipt)?.into_bytes();
    publish_root_private_noreplace(directory, name, &bytes)
}

fn execute_host_action(
    admitted: &HostAdmission,
    action: HostAction,
    body: &mut impl Read,
) -> Result<(u64, u64, String)> {
    ensure_action_deadline(admitted)?;
    let result = match action {
        HostAction::Preflight => Err(eyre!("preflight is not a mutating host action")),
        HostAction::MutationReserve => Err(eyre!(
            "mutation reservation is handled before the host action executor"
        )),
        HostAction::Upload => {
            let destination = staged_artifact_path(admitted, &admitted.request.artifact_role)?;
            verify_upload_body(admitted, body, Some(&destination))?;
            Ok((0, 0, "artifact uploaded and rehashed".to_owned()))
        }
        HostAction::Stage | HostAction::EdgeStage => {
            verify_staged_artifacts(admitted)?;
            Ok((0, 0, "complete staged artifact closure verified".to_owned()))
        }
        HostAction::Stop => {
            let HostTarget::Validator(validator) = &admitted.target else {
                return Err(eyre!("stop action requires a validator target"));
            };
            stop_unit(admitted, "stop", &validator.systemd_unit)?;
            Ok((0, 0, "validator stopped".to_owned()))
        }
        HostAction::Install => {
            let HostTarget::Validator(_) = &admitted.target else {
                return Err(eyre!("install action requires a validator target"));
            };
            install_release(admitted)?;
            Ok((0, 0, "validator release installed and selected".to_owned()))
        }
        HostAction::Reset => {
            let HostTarget::Validator(_) = &admitted.target else {
                return Err(eyre!("reset action requires a validator target"));
            };
            reset_validator_state(admitted)?;
            Ok((0, 0, "validator state atomically reset".to_owned()))
        }
        HostAction::Start => {
            let HostTarget::Validator(validator) = &admitted.target else {
                return Err(eyre!("start action requires a validator target"));
            };
            start_unit(admitted, "start", &validator.systemd_unit)?;
            let release = Path::new(&validator.service_root)
                .join("releases")
                .join(&admitted.inventory.revision.commit);
            attest_validator_process(admitted, validator, &release, true)?;
            Ok((0, 0, "validator started".to_owned()))
        }
        HostAction::Restart => {
            let HostTarget::Validator(validator) = &admitted.target else {
                return Err(eyre!("restart action requires a validator target"));
            };
            restart_unit(admitted, validator)?;
            let release = Path::new(&validator.service_root)
                .join("releases")
                .join(&admitted.inventory.revision.commit);
            attest_validator_process(admitted, validator, &release, true)?;
            Ok((0, 0, "validator restarted".to_owned()))
        }
        HostAction::EdgeCutover => {
            let HostTarget::Edge(_) = &admitted.target else {
                return Err(eyre!("edge cutover requires the edge target"));
            };
            install_release(admitted)?;
            cutover_edge(admitted)?;
            Ok((0, 0, "edge release cut over and nginx reloaded".to_owned()))
        }
        HostAction::EdgeVerify => {
            let HostTarget::Edge(edge) = &admitted.target else {
                return Err(eyre!("edge verify requires the edge target"));
            };
            verify_regular_hash(
                Path::new(&edge.nginx_config),
                &artifact(&edge.artifacts, "edge_config")?.sha256,
            )?;
            run_host_command(NGINX, &["-t"], admitted.action_deadline)?;
            Ok((0, 0, "edge configuration rehashed and verified".to_owned()))
        }
        HostAction::Seal => {
            verify_success_seal_postcondition(admitted)?;
            Ok((0, 0, "deployment success sealed on this target".to_owned()))
        }
        HostAction::Cleanup => {
            let (before, after, paths) = cleanup_generated_waste(admitted)?;
            Ok((
                before,
                after,
                format!("removed marker-authorized paths: {}", paths.join(",")),
            ))
        }
        HostAction::Rollback => {
            rollback_host(admitted)?;
            Ok((
                0,
                0,
                "host rollback restored the authorized prior closure".to_owned(),
            ))
        }
    };
    ensure_action_deadline(admitted)?;
    result
}

fn ensure_action_deadline(admitted: &HostAdmission) -> Result<()> {
    if Instant::now() >= admitted.action_deadline {
        return Err(eyre!("signed host action deadline elapsed"));
    }
    Ok(())
}

fn staged_artifact_path(admitted: &HostAdmission, role: &str) -> Result<PathBuf> {
    let root = ensure_upload_nonce_root(admitted)?;
    Ok(root.join(staged_artifact_name(role)?))
}

fn ensure_upload_nonce_root(admitted: &HostAdmission) -> Result<PathBuf> {
    let parent = Path::new(&admitted.guard.upload_parent);
    require_root_directory(parent, true, "upload parent")?;
    let root = parent.join(&admitted.inventory.authorization_nonce);
    ensure_generated_directory(&root, admitted, "upload", 0o700)?;
    Ok(root)
}

fn verify_upload_body(
    admitted: &HostAdmission,
    body: &mut impl Read,
    destination: Option<&Path>,
) -> Result<()> {
    let artifact = artifact(admitted.target.artifacts(), &admitted.request.artifact_role)?;
    let mut digest = Sha256::new();
    let mut copied = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    let staging = destination.map(|destination| {
        destination.with_file_name(format!(
            ".{}.{}.next",
            staged_artifact_name(&artifact.role).expect("validated role"),
            admitted.request_sha256
        ))
    });
    let staging_exists = staging
        .as_deref()
        .map(|path| reconcile_verified_staging(path, artifact.size, &artifact.sha256))
        .transpose()?
        .unwrap_or(false);
    let mut output = match staging.as_deref() {
        Some(_) if staging_exists => None,
        Some(path) => {
            #[cfg(unix)]
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
                .wrap_err("failed to create exact upload staging file")?;
            #[cfg(not(unix))]
            let file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .wrap_err("failed to create exact upload staging file")?;
            Some(file)
        }
        None => None,
    };
    while copied < artifact.size {
        ensure_action_deadline(admitted)?;
        let remaining = usize::try_from((artifact.size - copied).min(buffer.len() as u64))
            .expect("bounded upload chunk");
        let count = body
            .read(&mut buffer[..remaining])
            .wrap_err("failed to read framed artifact body")?;
        if count == 0 {
            return Err(eyre!("artifact body ended before its declared length"));
        }
        copied += u64::try_from(count).expect("read count fits u64");
        digest.update(&buffer[..count]);
        if let Some(file) = output.as_mut() {
            file.write_all(&buffer[..count])
                .wrap_err("failed to write staged artifact body")?;
        }
    }
    let mut trailing = [0_u8; 1];
    if body.read(&mut trailing)? != 0 {
        return Err(eyre!("artifact body contains trailing bytes"));
    }
    if copied != artifact.size || hex::encode(digest.finalize()) != artifact.sha256 {
        return Err(eyre!(
            "uploaded artifact length or SHA-256 does not match inventory"
        ));
    }
    let staging_was_written = output.is_some();
    if let Some(mut file) = output {
        file.flush()?;
        file.sync_all()?;
    }
    let Some(destination) = destination else {
        return Ok(());
    };
    let staging = staging.expect("destination supplies staging path");
    if !staging_was_written {
        sync_verified_regular_hash(&staging, &artifact.sha256)?;
    }
    #[cfg(unix)]
    fs::set_permissions(
        &staging,
        fs::Permissions::from_mode(u32::from(artifact.mode)),
    )?;
    sync_verified_regular_hash(&staging, &artifact.sha256)?;
    ensure_action_deadline(admitted)?;
    match rename_noreplace(&staging, destination) {
        Ok(()) => {}
        Err(error) if destination.exists() => {
            verify_regular_hash(destination, &artifact.sha256)?;
            let _ = fs::remove_file(&staging);
            drop(error);
        }
        Err(error) => return Err(error),
    }
    verify_regular_hash(destination, &artifact.sha256)?;
    sync_directory(destination.parent().expect("artifact has parent"))
}

fn verify_staged_artifacts(admitted: &HostAdmission) -> Result<()> {
    let root = ensure_upload_nonce_root(admitted)?;
    verify_generated_marker(&root, admitted, "upload")?;
    for artifact in admitted.target.artifacts() {
        ensure_action_deadline(admitted)?;
        let path = root.join(staged_artifact_name(&artifact.role)?);
        let metadata = fs::symlink_metadata(&path)
            .wrap_err_with(|| format!("staged artifact `{}` is missing", artifact.role))?;
        #[cfg(unix)]
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.uid() != 0
            || metadata.mode() & 0o7777 != u32::from(artifact.mode)
            || metadata.nlink() != 1
            || metadata.len() != artifact.size
        {
            return Err(eyre!("staged artifact custody/mode/size drifted"));
        }
        verify_regular_hash(&path, &artifact.sha256)?;
    }
    Ok(())
}

fn install_release(admitted: &HostAdmission) -> Result<()> {
    verify_staged_artifacts(admitted)?;
    let service_root = Path::new(admitted.target.service_root());
    let releases = service_root.join("releases");
    ensure_root_directory_with_mode(&releases, 0o755)?;
    let final_root = releases.join(&admitted.inventory.revision.commit);
    if final_root.exists() {
        verify_installed_release(admitted, &final_root)?;
        return select_current_release(admitted, &final_root);
    }
    let staging = releases.join(format!(
        ".{}.{}.next",
        admitted.inventory.revision.commit, admitted.inventory.authorization_nonce
    ));
    ensure_generated_directory(&staging, admitted, "release", 0o700)?;
    let upload = ensure_upload_nonce_root(admitted)?;
    for artifact in admitted.target.artifacts() {
        ensure_action_deadline(admitted)?;
        let relative = Path::new(&artifact.remote_path)
            .strip_prefix(&final_root)
            .wrap_err("artifact remote path escaped the exact release root")?;
        if relative.components().any(|component| {
            matches!(
                component,
                std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
                    | std::path::Component::ParentDir
                    | std::path::Component::CurDir
            )
        }) {
            return Err(eyre!("artifact relative install path is not canonical"));
        }
        let destination = staging.join(relative);
        if let Some(parent) = destination.parent() {
            ensure_generated_subdirectories(&staging, parent)?;
        }
        let source = upload.join(staged_artifact_name(&artifact.role)?);
        copy_verified_file(&source, &destination, artifact)?;
    }
    verify_installed_release(admitted, &staging)?;
    sync_release_tree(&staging, admitted)?;
    ensure_action_deadline(admitted)?;
    rename_noreplace(&staging, &final_root)?;
    sync_directory(&releases)?;
    select_current_release(admitted, &final_root)
}

fn verify_installed_release(admitted: &HostAdmission, root: &Path) -> Result<()> {
    verify_generated_marker(root, admitted, "release")?;
    let expected_root = Path::new(admitted.target.service_root())
        .join("releases")
        .join(&admitted.inventory.revision.commit);
    for artifact in admitted.target.artifacts() {
        let relative = Path::new(&artifact.remote_path)
            .strip_prefix(&expected_root)
            .wrap_err("artifact remote path escaped installed release")?;
        let path = root.join(relative);
        let metadata = fs::symlink_metadata(&path)?;
        #[cfg(unix)]
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.uid() != 0
            || metadata.mode() & 0o7777 != u32::from(artifact.mode)
            || metadata.nlink() != 1
            || metadata.len() != artifact.size
        {
            return Err(eyre!("installed release artifact custody drifted"));
        }
        verify_regular_hash(&path, &artifact.sha256)?;
    }
    verify_exact_release_tree(admitted, root, &expected_root)?;
    Ok(())
}

fn verify_exact_release_tree(
    admitted: &HostAdmission,
    root: &Path,
    expected_root: &Path,
) -> Result<()> {
    let mut files = BTreeSet::from([PathBuf::from(".public-reset-generated-v1.json")]);
    let mut directories = BTreeSet::new();
    for artifact in admitted.target.artifacts() {
        let relative = Path::new(&artifact.remote_path).strip_prefix(expected_root)?;
        files.insert(relative.to_path_buf());
        let mut parent = relative.parent();
        while let Some(path) = parent {
            if path.as_os_str().is_empty() {
                break;
            }
            directories.insert(path.to_path_buf());
            parent = path.parent();
        }
    }
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        ensure_action_deadline(admitted)?;
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            let relative = path.strip_prefix(root)?.to_path_buf();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(eyre!("installed release closure contains a symlink"));
            }
            if metadata.is_dir() {
                if !directories.remove(&relative) {
                    return Err(eyre!("installed release contains an unbound directory"));
                }
                pending.push(path);
            } else if metadata.is_file() {
                if !files.remove(&relative) {
                    return Err(eyre!("installed release contains an unbound file"));
                }
            } else {
                return Err(eyre!("installed release contains a special file"));
            }
        }
    }
    if !files.is_empty() || !directories.is_empty() {
        return Err(eyre!(
            "installed release omits part of its exact artifact closure"
        ));
    }
    Ok(())
}

fn copy_verified_file(source: &Path, destination: &Path, artifact: &ArtifactV1) -> Result<()> {
    if destination.exists() {
        return sync_verified_regular_hash(destination, &artifact.sha256);
    }
    let (mut input, snapshot) = open_pinned_regular(source, "uploaded artifact")?;
    if snapshot.len != artifact.size {
        return Err(eyre!("uploaded artifact size drifted before install"));
    }
    #[cfg(unix)]
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(u32::from(artifact.mode))
        .open(destination)?;
    #[cfg(not(unix))]
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    std::io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    ensure_pinned_unchanged(source, "uploaded artifact", &input, &snapshot)?;
    verify_regular_hash(destination, &artifact.sha256)
}

fn ensure_generated_subdirectories(root: &Path, target: &Path) -> Result<()> {
    let relative = target.strip_prefix(root)?;
    let mut cursor = root.to_path_buf();
    for component in relative.components() {
        let std::path::Component::Normal(name) = component else {
            return Err(eyre!("generated subdirectory path escaped its root"));
        };
        cursor.push(name);
        if !cursor.exists() {
            fs::create_dir(&cursor)?;
            #[cfg(unix)]
            fs::set_permissions(&cursor, fs::Permissions::from_mode(0o700))?;
        }
        require_root_directory(&cursor, true, "generated release subdirectory")?;
    }
    Ok(())
}

fn sync_release_tree(root: &Path, admitted: &HostAdmission) -> Result<()> {
    let mut directories = vec![root.to_path_buf()];
    let mut cursor = 0;
    while cursor < directories.len() {
        ensure_action_deadline(admitted)?;
        let directory = directories[cursor].clone();
        cursor += 1;
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(eyre!("release sync closure contains a symlink"));
            }
            if metadata.is_dir() {
                directories.push(path);
            } else if !metadata.is_file() {
                return Err(eyre!("release sync closure contains a special file"));
            }
        }
    }
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        ensure_action_deadline(admitted)?;
        File::open(&directory)?.sync_all()?;
    }
    Ok(())
}

fn select_current_release(admitted: &HostAdmission, release: &Path) -> Result<()> {
    let service_root = Path::new(admitted.target.service_root());
    let current = service_root.join("current");
    if let Ok(target) = fs::read_link(&current)
        && target == release
    {
        return Ok(());
    }
    if fs::symlink_metadata(&current).is_ok() {
        let target = validated_current_release_target(admitted)?;
        if target == release {
            return Ok(());
        }
    }
    let next = service_root.join(format!(
        ".current.{}.next",
        admitted.inventory.authorization_nonce
    ));
    if next.exists() {
        if fs::read_link(&next)? != release {
            return Err(eyre!("stale current-release selector has different target"));
        }
    } else {
        symlink(release, &next)?;
    }
    fs::rename(&next, &current)?;
    sync_directory(service_root)
}

fn validated_current_release_target(admitted: &HostAdmission) -> Result<PathBuf> {
    let service_root = Path::new(admitted.target.service_root());
    let current = service_root.join("current");
    let metadata =
        fs::symlink_metadata(&current).wrap_err("stable current release selector is missing")?;
    if !metadata.file_type().is_symlink() {
        return Err(eyre!("stable current release selector is not a symlink"));
    }
    let target = fs::read_link(&current)?;
    let rollback_release = match &admitted.target {
        HostTarget::Validator(validator) => Path::new(&validator.rollback.release_root),
        HostTarget::Edge(edge) => Path::new(&edge.rollback_release_root),
    };
    if target == rollback_release {
        require_root_directory(&target, false, "stable rollback release")?;
        return Ok(target);
    }
    let releases = service_root.join("releases");
    let relative = target
        .strip_prefix(&releases)
        .wrap_err("stable current release selector escaped releases")?;
    let mut components = relative.components();
    let Some(std::path::Component::Normal(revision)) = components.next() else {
        return Err(eyre!("stable current release selector lacks a revision"));
    };
    let revision = revision
        .to_str()
        .ok_or_else(|| eyre!("stable current release revision is not UTF-8"))?;
    if components.next().is_some()
        || revision.len() != 40
        || !revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(eyre!("stable current release selector is not canonical"));
    }
    require_root_directory(&target, false, "stable current release")?;
    Ok(target)
}

fn reset_validator_state(admitted: &HostAdmission) -> Result<()> {
    let state = Path::new(admitted.target.state_root());
    let rollback = rollback_nonce_root(admitted)?;
    let previous = rollback.join("state");
    if previous.exists() {
        let intent = load_state_move_intent(&rollback, admitted)?;
        require_state_identity(&previous, &intent)?;
        return reconcile_fresh_state(state, admitted);
    }
    require_root_directory(state, false, "authorized validator state root")?;
    if state.metadata()?.dev() != rollback.metadata()?.dev() {
        return Err(eyre!(
            "state and rollback roots are not on one atomic filesystem"
        ));
    }
    let intent = ensure_state_move_intent(&rollback, state, admitted)?;
    rename_noreplace(state, &previous)?;
    require_state_identity(&previous, &intent)?;
    reconcile_fresh_state(state, admitted)?;
    sync_directory(state.parent().expect("state has parent"))
}

fn ensure_state_move_intent(
    rollback: &Path,
    state: &Path,
    admitted: &HostAdmission,
) -> Result<StateMoveIntentV1> {
    let path = rollback.join("state-move.intent.json");
    if path.exists() {
        let intent = load_state_move_intent(rollback, admitted)?;
        require_state_identity(state, &intent)?;
        return Ok(intent);
    }
    let metadata = state.metadata()?;
    let intent = StateMoveIntentV1 {
        schema: STATE_MOVE_INTENT_SCHEMA_V1.to_owned(),
        host_slug: admitted.target.slug().to_owned(),
        inventory_sha256: admitted.inventory_sha256.clone(),
        authorization_nonce: admitted.inventory.authorization_nonce.clone(),
        state_root: admitted.target.state_root().to_owned(),
        prior_device: metadata.dev(),
        prior_inode: metadata.ino(),
    };
    publish_root_private_noreplace(
        rollback,
        "state-move.intent.json",
        json::to_json(&intent)?.as_bytes(),
    )?;
    load_state_move_intent(rollback, admitted)
}

fn load_state_move_intent(rollback: &Path, admitted: &HostAdmission) -> Result<StateMoveIntentV1> {
    let (intent, _) = read_private_json::<StateMoveIntentV1>(
        &rollback.join("state-move.intent.json"),
        "validator state-move intent",
    )?;
    if intent.schema != STATE_MOVE_INTENT_SCHEMA_V1
        || intent.host_slug != admitted.target.slug()
        || intent.inventory_sha256 != admitted.inventory_sha256
        || intent.authorization_nonce != admitted.inventory.authorization_nonce
        || intent.state_root != admitted.target.state_root()
        || intent.prior_inode == 0
    {
        return Err(eyre!("validator state-move intent is not this exact reset"));
    }
    Ok(intent)
}

fn require_state_identity(state: &Path, intent: &StateMoveIntentV1) -> Result<()> {
    let metadata = state.metadata()?;
    if !metadata.is_dir()
        || metadata.uid() != 0
        || metadata.dev() != intent.prior_device
        || metadata.ino() != intent.prior_inode
    {
        return Err(eyre!(
            "validator state root does not match the durable pre-reset identity"
        ));
    }
    Ok(())
}

fn reconcile_fresh_state(state: &Path, admitted: &HostAdmission) -> Result<()> {
    if !state.exists() {
        ensure_root_directory_with_mode(state, 0o700)?;
    }
    require_root_directory(state, true, "fresh validator state root")?;
    for entry in fs::read_dir(state)? {
        let entry = entry?;
        let name = entry.file_name();
        let allowed = name == OsStr::new(".public-reset-generated-v1.json")
            || RESET_GENERATED_ENTRIES
                .iter()
                .any(|expected| name == OsStr::new(expected));
        if !allowed {
            return Err(eyre!(
                "partial fresh state contains an entry outside the exact reset closure"
            ));
        }
    }
    ensure_generated_directory(state, admitted, "fresh_state", 0o700)?;
    for name in RESET_GENERATED_ENTRIES {
        let path = state.join(name);
        ensure_generated_directory(&path, admitted, "fresh_state_entry", 0o700)?;
    }
    Ok(())
}

fn verify_fresh_state(admitted: &HostAdmission) -> Result<()> {
    let state = Path::new(admitted.target.state_root());
    require_root_directory(state, true, "fresh validator state root")?;
    verify_generated_marker(state, admitted, "fresh_state")?;
    let expected = BTreeSet::from_iter(
        std::iter::once(OsString::from(".public-reset-generated-v1.json"))
            .chain(RESET_GENERATED_ENTRIES.into_iter().map(OsString::from)),
    );
    let actual = fs::read_dir(state)?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<std::io::Result<BTreeSet<_>>>()?;
    if actual != expected {
        return Err(eyre!(
            "fresh validator state differs from its exact reset closure"
        ));
    }
    for name in RESET_GENERATED_ENTRIES {
        let path = state.join(name);
        require_root_directory(&path, true, "fresh validator state entry")?;
        verify_generated_marker(&path, admitted, "fresh_state_entry")?;
        if fs::read_dir(&path)?.count() != 1 {
            return Err(eyre!(
                "fresh validator state entry contains unexpected data"
            ));
        }
    }
    Ok(())
}

fn rollback_nonce_root(admitted: &HostAdmission) -> Result<PathBuf> {
    let rollback_parent = Path::new(admitted.target.reset_guard()).join("rollback");
    ensure_root_directory_with_mode(&rollback_parent, 0o700)?;
    let root = rollback_parent.join(&admitted.inventory.authorization_nonce);
    ensure_generated_directory(&root, admitted, "rollback", 0o700)?;
    Ok(root)
}

fn cutover_edge(admitted: &HostAdmission) -> Result<()> {
    let HostTarget::Edge(edge) = &admitted.target else {
        return Err(eyre!("edge cutover requires the edge target"));
    };
    let release = Path::new(&edge.service_root)
        .join("releases")
        .join(&admitted.inventory.revision.commit);
    verify_installed_release(admitted, &release)?;
    let rollback = rollback_nonce_root(admitted)?;
    let backup = rollback.join("edge-config.before");
    if !backup.exists() {
        verify_regular_hash(
            Path::new(&edge.nginx_config),
            &edge.rollback_edge_config_sha256,
        )?;
        snapshot_root_file(
            admitted,
            Path::new(&edge.nginx_config),
            &backup,
            &edge.rollback_edge_config_sha256,
        )?;
    }
    verify_regular_hash(&backup, &edge.rollback_edge_config_sha256)?;
    let config = artifact(&edge.artifacts, "edge_config")?;
    let source = release.join("taira.conf");
    atomic_replace_verified_file(admitted, &source, Path::new(&edge.nginx_config), config)?;
    run_host_command(NGINX, &["-t"], admitted.action_deadline)?;
    run_durable_manager_operation(admitted, "edge-cutover-reload", "reload", "nginx.service")?;
    Ok(())
}

fn snapshot_root_file(
    admitted: &HostAdmission,
    source: &Path,
    destination: &Path,
    expected_sha256: &str,
) -> Result<()> {
    if destination.exists() {
        sync_verified_regular_hash(destination, expected_sha256)?;
        return sync_directory(
            destination
                .parent()
                .ok_or_else(|| eyre!("rollback snapshot has no parent"))?,
        );
    }
    let parent = destination
        .parent()
        .ok_or_else(|| eyre!("rollback snapshot has no parent"))?;
    require_root_directory(parent, false, "rollback snapshot parent")?;
    let next = parent.join(".edge-config.before.next");
    let (mut input, snapshot) = open_pinned_regular(source, "rollback source")?;
    if reconcile_verified_staging(&next, snapshot.len, expected_sha256)? {
        ensure_action_deadline(admitted)?;
        rename_noreplace(&next, destination)?;
        return verify_regular_hash(destination, expected_sha256);
    }
    #[cfg(unix)]
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&next)?;
    #[cfg(not(unix))]
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&next)?;
    std::io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    ensure_pinned_unchanged(source, "rollback source", &input, &snapshot)?;
    verify_regular_hash(&next, expected_sha256)?;
    ensure_action_deadline(admitted)?;
    rename_noreplace(&next, destination)?;
    verify_regular_hash(destination, expected_sha256)
}

fn atomic_replace_verified_file(
    admitted: &HostAdmission,
    source: &Path,
    destination: &Path,
    artifact: &ArtifactV1,
) -> Result<()> {
    verify_regular_hash(source, &artifact.sha256)?;
    let parent = destination
        .parent()
        .ok_or_else(|| eyre!("replacement destination has no parent"))?;
    require_root_directory(parent, false, "replacement parent")?;
    let next = parent.join(format!(
        ".{}.public-reset.next",
        destination
            .file_name()
            .and_then(OsStr::to_str)
            .ok_or_else(|| eyre!("non-UTF8 replacement name"))?
    ));
    if !reconcile_verified_staging(&next, artifact.size, &artifact.sha256)? {
        copy_verified_file(source, &next, artifact)?;
    }
    ensure_action_deadline(admitted)?;
    fs::rename(&next, destination)?;
    sync_directory(parent)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ManagerOperationEvidence {
    Absent,
    Applied,
    Pending,
    Rejected,
}

fn manager_intent_name(label: &str) -> Result<String> {
    if label.is_empty()
        || label.len() > 64
        || !label
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(eyre!(
            "manager operation label is outside its closed grammar"
        ));
    }
    Ok(format!("manager-{label}.intent.json"))
}

fn manager_operation_unit(admitted: &HostAdmission, label: &str) -> Result<String> {
    manager_intent_name(label)?;
    let authorization = admitted
        .authorization_sha256
        .get(..16)
        .ok_or_else(|| eyre!("authorization digest is too short for manager unit"))?;
    let unit = format!(
        "iroha-taira-reset-{authorization}-{}-{label}.service",
        admitted.target.slug()
    );
    if unit.len() > 240
        || !unit.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'.' | b'_')
        })
    {
        return Err(eyre!(
            "deterministic manager unit is outside its closed grammar"
        ));
    }
    Ok(unit)
}

fn ensure_manager_intent(
    admitted: &HostAdmission,
    label: &str,
    verb: &str,
    target_unit: &str,
) -> Result<(ManagerIntentV1, bool)> {
    if !matches!(verb, "stop" | "start" | "restart" | "reload")
        || target_unit.is_empty()
        || target_unit.len() > 255
        || !target_unit
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'@'))
    {
        return Err(eyre!(
            "manager operation argv is outside its closed grammar"
        ));
    }
    let directory = ensure_host_receipt_dir(admitted)?;
    let name = manager_intent_name(label)?;
    let path = directory.join(&name);
    if path.exists() {
        let (intent, _) = read_private_json::<ManagerIntentV1>(&path, "manager operation intent")?;
        validate_manager_intent(admitted, label, verb, target_unit, &intent)?;
        return Ok((intent, false));
    }
    ensure_action_deadline(admitted)?;
    let intent = ManagerIntentV1 {
        schema: MANAGER_INTENT_SCHEMA_V1.to_owned(),
        action: label.to_owned(),
        host_slug: admitted.target.slug().to_owned(),
        request_sha256: admitted.request_sha256.clone(),
        authorization_nonce: admitted.inventory.authorization_nonce.clone(),
        boot_id: host_boot_id()?,
        operation_unit: manager_operation_unit(admitted, label)?,
        verb: verb.to_owned(),
        target_unit: target_unit.to_owned(),
        created_at_unix_ms: now_unix_ms()?,
        action_deadline_unix_ms: admitted.request.action_deadline_unix_ms,
    };
    publish_root_private_noreplace(&directory, &name, json::to_json(&intent)?.as_bytes())?;
    let (loaded, _) = read_private_json::<ManagerIntentV1>(&path, "manager operation intent")?;
    validate_manager_intent(admitted, label, verb, target_unit, &loaded)?;
    Ok((loaded, true))
}

fn validate_manager_intent(
    admitted: &HostAdmission,
    label: &str,
    verb: &str,
    target_unit: &str,
    intent: &ManagerIntentV1,
) -> Result<()> {
    if intent.schema != MANAGER_INTENT_SCHEMA_V1
        || intent.action != label
        || intent.host_slug != admitted.target.slug()
        || intent.request_sha256 != admitted.request_sha256
        || intent.authorization_nonce != admitted.inventory.authorization_nonce
        || intent.operation_unit != manager_operation_unit(admitted, label)?
        || intent.verb != verb
        || intent.target_unit != target_unit
        || validate_boot_id(&intent.boot_id).is_err()
        || intent.created_at_unix_ms >= intent.action_deadline_unix_ms
    {
        return Err(eyre!(
            "manager operation intent does not bind the exact host action"
        ));
    }
    Ok(())
}

fn validate_manager_intent_for_session(
    admitted: &HostAdmission,
    label: &str,
    verb: &str,
    target_unit: &str,
    intent: &ManagerIntentV1,
) -> Result<()> {
    if intent.schema != MANAGER_INTENT_SCHEMA_V1
        || intent.action != label
        || intent.host_slug != admitted.target.slug()
        || intent.authorization_nonce != admitted.inventory.authorization_nonce
        || intent.operation_unit != manager_operation_unit(admitted, label)?
        || intent.verb != verb
        || intent.target_unit != target_unit
        || validate_boot_id(&intent.boot_id).is_err()
        || intent.request_sha256.len() != 64
        || !intent
            .request_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || intent.created_at_unix_ms >= intent.action_deadline_unix_ms
    {
        return Err(eyre!(
            "manager operation intent is not bound to this authorization session"
        ));
    }
    Ok(())
}

fn inspect_manager_operation(
    intent: &ManagerIntentV1,
    deadline: Instant,
) -> Result<ManagerOperationEvidence> {
    if host_boot_id()? != intent.boot_id {
        return Ok(ManagerOperationEvidence::Rejected);
    }
    let output = run_host_command(
        SYSTEMCTL,
        &[
            "show",
            "--property=LoadState",
            "--property=ActiveState",
            "--property=SubState",
            "--property=Result",
            "--property=ExecMainCode",
            "--property=ExecMainStatus",
            "--property=InvocationID",
            "--property=ExecStart",
            "--property=Job",
            &intent.operation_unit,
        ],
        deadline,
    )?;
    classify_manager_operation_at(&output, intent, now_unix_ms()?)
}

fn classify_manager_operation_at(
    output: &[u8],
    intent: &ManagerIntentV1,
    observed_at_unix_ms: u64,
) -> Result<ManagerOperationEvidence> {
    match classify_manager_operation_evidence(output, intent)? {
        ManagerOperationEvidence::Absent
            if observed_at_unix_ms >= intent.action_deadline_unix_ms =>
        {
            Ok(ManagerOperationEvidence::Rejected)
        }
        evidence => Ok(evidence),
    }
}

fn classify_manager_operation_evidence(
    output: &[u8],
    intent: &ManagerIntentV1,
) -> Result<ManagerOperationEvidence> {
    let text = std::str::from_utf8(output)?;
    let mut values = BTreeMap::new();
    for line in text.lines() {
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| eyre!("manager evidence is not key=value"))?;
        if !matches!(
            key,
            "LoadState"
                | "ActiveState"
                | "SubState"
                | "Result"
                | "ExecMainCode"
                | "ExecMainStatus"
                | "InvocationID"
                | "ExecStart"
                | "Job"
        ) || values.insert(key, value).is_some()
        {
            return Err(eyre!(
                "manager evidence has an unknown or duplicate property"
            ));
        }
    }
    for key in [
        "LoadState",
        "ActiveState",
        "SubState",
        "Result",
        "ExecMainCode",
        "ExecMainStatus",
        "InvocationID",
        "ExecStart",
        "Job",
    ] {
        if !values.contains_key(key) {
            return Err(eyre!("manager evidence omits a required property"));
        }
    }
    if values["LoadState"] == "not-found" {
        return Ok(ManagerOperationEvidence::Absent);
    }
    let job = values["Job"];
    if !matches!(job, "" | "0")
        || matches!(
            values["ActiveState"],
            "activating" | "deactivating" | "reloading"
        )
        || matches!(values["SubState"], "start" | "stop" | "running")
    {
        return Ok(ManagerOperationEvidence::Pending);
    }
    let invocation = values["InvocationID"];
    let expected_argv = format!(
        "argv[]={} {} {}",
        SYSTEMCTL, intent.verb, intent.target_unit
    );
    if values["LoadState"] != "loaded"
        || invocation.len() != 32
        || !invocation
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || !values["ExecStart"].contains("path=/usr/bin/systemctl")
        || !values["ExecStart"].contains(&expected_argv)
    {
        return Err(eyre!(
            "manager evidence does not bind the exact transient unit"
        ));
    }
    if values["ActiveState"] == "active"
        && values["SubState"] == "exited"
        && values["Result"] == "success"
        && values["ExecMainCode"] == "exited"
        && values["ExecMainStatus"] == "0"
    {
        return Ok(ManagerOperationEvidence::Applied);
    }
    if values["ActiveState"] == "failed"
        || values["Result"] != "success"
        || values["ExecMainStatus"] != "0"
    {
        return Ok(ManagerOperationEvidence::Rejected);
    }
    Ok(ManagerOperationEvidence::Pending)
}

fn run_durable_manager_operation(
    admitted: &HostAdmission,
    label: &str,
    verb: &str,
    target_unit: &str,
) -> Result<()> {
    let (intent, created) = ensure_manager_intent(admitted, label, verb, target_unit)?;
    let mut submission_attempted = false;
    if created {
        submit_durable_manager_operation(admitted, &intent)?;
        submission_attempted = true;
    }
    loop {
        if admitted.action_deadline <= Instant::now() {
            return Err(LocalMutationRecoveryPending {
                action: "systemd_manager_operation",
            }
            .into());
        }
        match inspect_manager_operation(&intent, admitted.action_deadline) {
            Err(error) => {
                eprintln!(
                    "durable manager operation observation is ambiguous (ephemeral): {error:#}"
                );
                return Err(LocalMutationRecoveryPending {
                    action: "systemd_manager_operation",
                }
                .into());
            }
            Ok(ManagerOperationEvidence::Absent) if !submission_attempted => {
                submit_durable_manager_operation(admitted, &intent)?;
                submission_attempted = true;
            }
            Ok(ManagerOperationEvidence::Absent | ManagerOperationEvidence::Pending) => {}
            Ok(ManagerOperationEvidence::Applied) => return Ok(()),
            Ok(ManagerOperationEvidence::Rejected) => {
                return Err(eyre!("durable manager operation was definitively rejected"));
            }
        }
        let remaining = admitted
            .action_deadline
            .saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(LocalMutationRecoveryPending {
                action: "systemd_manager_operation",
            }
            .into());
        }
        std::thread::sleep(PROCESS_POLL_INTERVAL.min(remaining));
    }
}

fn submit_durable_manager_operation(
    admitted: &HostAdmission,
    intent: &ManagerIntentV1,
) -> Result<()> {
    if now_unix_ms()? >= intent.action_deadline_unix_ms {
        return Err(LocalMutationRecoveryPending {
            action: "systemd_manager_operation",
        }
        .into());
    }
    ensure_action_deadline(admitted)?;
    let unit_arg = format!("--unit={}", intent.operation_unit);
    if let Err(error) = run_host_command(
        SYSTEMD_RUN,
        &[
            &unit_arg,
            "--property=Type=oneshot",
            "--property=RemainAfterExit=yes",
            "--no-block",
            "--",
            SYSTEMCTL,
            &intent.verb,
            &intent.target_unit,
        ],
        admitted.action_deadline,
    ) {
        eprintln!("durable systemd-run submission outcome is ambiguous (ephemeral): {error:#}");
        return Err(LocalMutationRecoveryPending {
            action: "systemd_manager_operation",
        }
        .into());
    }
    Ok(())
}

fn reconcile_prior_manager_operations(admitted: &HostAdmission) -> Result<()> {
    let directory = ensure_host_receipt_dir(admitted)?;
    let operations: Vec<(&str, &str, &str)> = match &admitted.target {
        HostTarget::Validator(validator) => vec![
            ("stop", "stop", validator.systemd_unit.as_str()),
            ("start", "start", validator.systemd_unit.as_str()),
            ("restart", "restart", validator.systemd_unit.as_str()),
        ],
        HostTarget::Edge(_) => vec![("edge-cutover-reload", "reload", "nginx.service")],
    };
    for (label, verb, target_unit) in operations {
        let path = directory.join(manager_intent_name(label)?);
        if !path.exists() {
            continue;
        }
        let (intent, _) =
            read_private_json::<ManagerIntentV1>(&path, "prior manager operation intent")?;
        validate_manager_intent_for_session(admitted, label, verb, target_unit, &intent)?;
        loop {
            if admitted.action_deadline <= Instant::now() {
                return Err(LocalMutationRecoveryPending {
                    action: "prior_systemd_manager_operation",
                }
                .into());
            }
            match inspect_manager_operation(&intent, admitted.action_deadline) {
                Ok(ManagerOperationEvidence::Applied | ManagerOperationEvidence::Rejected) => {
                    break;
                }
                Ok(ManagerOperationEvidence::Absent | ManagerOperationEvidence::Pending)
                | Err(_) => {}
            }
            let remaining = admitted
                .action_deadline
                .saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(LocalMutationRecoveryPending {
                    action: "prior_systemd_manager_operation",
                }
                .into());
            }
            std::thread::sleep(PROCESS_POLL_INTERVAL.min(remaining));
        }
    }
    Ok(())
}

fn require_session_manager_operation_applied(
    admitted: &HostAdmission,
    label: &str,
    verb: &str,
    target_unit: &str,
) -> Result<()> {
    let directory = ensure_host_receipt_dir(admitted)?;
    let path = directory.join(manager_intent_name(label)?);
    let (intent, _) = read_private_json::<ManagerIntentV1>(&path, "manager operation evidence")?;
    validate_manager_intent_for_session(admitted, label, verb, target_unit, &intent)?;
    if inspect_manager_operation(&intent, admitted.action_deadline)?
        != ManagerOperationEvidence::Applied
    {
        return Err(eyre!(
            "manager operation has no exact terminal Applied evidence"
        ));
    }
    Ok(())
}

fn stop_unit(admitted: &HostAdmission, label: &str, unit: &str) -> Result<()> {
    run_durable_manager_operation(admitted, label, "stop", unit)?;
    require_unit_stopped(unit, admitted.action_deadline)
}

fn require_unit_stopped(unit: &str, deadline: Instant) -> Result<()> {
    let state = run_host_command(
        SYSTEMCTL,
        &["show", "--property=ActiveState", "--value", unit],
        deadline,
    )?;
    if !matches!(state.as_slice(), b"inactive\n" | b"failed\n") {
        return Err(eyre!("validator unit did not reach an exact stopped state"));
    }
    Ok(())
}

fn start_unit(admitted: &HostAdmission, label: &str, unit: &str) -> Result<()> {
    run_durable_manager_operation(admitted, label, "start", unit)?;
    require_unit_active(unit, admitted.action_deadline)
}

fn restart_unit(admitted: &HostAdmission, validator: &ValidatorV1) -> Result<()> {
    let receipt_dir = ensure_host_receipt_dir(admitted)?;
    let (intent, _) = read_private_json::<HostIntentV1>(
        &receipt_dir.join("restart.intent.json"),
        "restart intent",
    )?;
    if intent.schema != HOST_INTENT_SCHEMA_V1
        || intent.action != HostAction::Restart.label()
        || intent.host_slug != validator.slug
        || intent.request_sha256 != admitted.request_sha256
        || intent.authorization_nonce != admitted.inventory.authorization_nonce
        || intent.boot_id != host_boot_id()?
    {
        return Err(eyre!(
            "restart manager submission lacks its exact durable intent"
        ));
    }
    run_durable_manager_operation(admitted, "restart", "restart", &validator.systemd_unit)?;
    loop {
        ensure_action_deadline(admitted)?;
        let evidence = unit_restart_evidence(validator, admitted.action_deadline)?;
        if evidence.boot_id != intent.boot_id {
            return Err(eyre!("kernel boot identity changed during restart job"));
        }
        if evidence.invocation != intent.before_evidence && evidence.is_terminal_active() {
            if evidence.active_enter_monotonic_ms < intent.created_at_monotonic_ms
                || evidence.active_enter_monotonic_ms > intent.action_deadline_monotonic_ms
            {
                return Err(eyre!(
                    "validator restart completed outside its durable monotonic intent window"
                ));
            }
            return require_unit_active(&validator.systemd_unit, admitted.action_deadline);
        }
        let remaining = admitted
            .action_deadline
            .saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(eyre!("validator restart manager job remained pending"));
        }
        std::thread::sleep(PROCESS_POLL_INTERVAL.min(remaining));
    }
}

fn require_unit_active(unit: &str, deadline: Instant) -> Result<()> {
    let state = run_host_command(
        SYSTEMCTL,
        &["show", "--property=ActiveState", "--value", unit],
        deadline,
    )?;
    if state != b"active\n" {
        return Err(eyre!("validator unit is not exactly active"));
    }
    Ok(())
}

fn attest_loaded_systemd_unit(validator: &ValidatorV1, deadline: Instant) -> Result<()> {
    let expected_fragment = Path::new("/etc/systemd/system").join(&validator.systemd_unit);
    let evidence = run_host_command(
        SYSTEMCTL,
        &[
            "show",
            "--property=FragmentPath",
            "--property=DropInPaths",
            "--property=NeedDaemonReload",
            &validator.systemd_unit,
        ],
        deadline,
    )?;
    validate_loaded_unit_evidence(&evidence, &expected_fragment)?;
    verify_regular_hash(&expected_fragment, &validator.systemd_unit_sha256)
}

fn validate_loaded_unit_evidence(bytes: &[u8], expected_fragment: &Path) -> Result<()> {
    let text = std::str::from_utf8(bytes)?;
    let mut fragment = None;
    let mut drop_ins = None;
    let mut reload = None;
    for line in text.lines() {
        let (name, value) = line
            .split_once('=')
            .ok_or_else(|| eyre!("systemd unit evidence is not key=value"))?;
        let slot = match name {
            "FragmentPath" => &mut fragment,
            "DropInPaths" => &mut drop_ins,
            "NeedDaemonReload" => &mut reload,
            _ => return Err(eyre!("systemd unit evidence contains an unknown property")),
        };
        if slot.replace(value).is_some() {
            return Err(eyre!("systemd unit evidence repeats a property"));
        }
    }
    if fragment != expected_fragment.to_str() || drop_ins != Some("") || reload != Some("no") {
        return Err(eyre!(
            "loaded systemd unit is not the exact signed fragment without drop-ins or stale manager state"
        ));
    }
    Ok(())
}

fn attest_validator_process(
    admitted: &HostAdmission,
    validator: &ValidatorV1,
    release_root: &Path,
    fresh_state: bool,
) -> Result<()> {
    ensure_action_deadline(admitted)?;
    attest_loaded_systemd_unit(validator, admitted.action_deadline)?;
    if validated_current_release_target(admitted)? != release_root {
        return Err(eyre!(
            "validator stable current selector does not resolve to the attested release"
        ));
    }
    let pid_bytes = run_host_command(
        SYSTEMCTL,
        &[
            "show",
            "--property=MainPID",
            "--value",
            &validator.systemd_unit,
        ],
        admitted.action_deadline,
    )?;
    let pid = std::str::from_utf8(&pid_bytes)?
        .trim_end_matches('\n')
        .parse::<u32>()
        .ok()
        .filter(|pid| *pid > 1)
        .ok_or_else(|| eyre!("validator unit has no exact positive MainPID"))?;
    let proc_root = PathBuf::from(format!("/proc/{pid}"));
    let expected_executable = release_root.join("bin/iroha3d_taira");
    if fs::read_link(proc_root.join("exe"))? != expected_executable {
        return Err(eyre!(
            "validator MainPID does not execute the selected release"
        ));
    }
    let cmdline = fs::read(proc_root.join("cmdline"))?;
    if cmdline.is_empty() || cmdline.len() > 8 * 1024 || !cmdline.ends_with(&[0]) {
        return Err(eyre!(
            "validator MainPID cmdline is outside the exact bound"
        ));
    }
    let arguments = cmdline[..cmdline.len() - 1]
        .split(|byte| *byte == 0)
        .map(|bytes| std::str::from_utf8(bytes).map(PathBuf::from))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let stable_current = Path::new(&validator.service_root).join("current");
    let stable_executable = stable_current.join("bin/iroha3d_taira");
    let stable_config = stable_current.join("config/config.toml");
    let stable_genesis = stable_current.join("genesis/genesis.json");
    let expected_config = release_root.join("config/config.toml");
    let expected_genesis = release_root.join("genesis/genesis.json");
    validate_validator_argv(
        &arguments,
        &stable_executable,
        &stable_config,
        &stable_genesis,
    )?;
    let config_hash = if fresh_state {
        &artifact(&validator.artifacts, "config")?.sha256
    } else {
        &validator.rollback.config_sha256
    };
    let genesis_hash = if fresh_state {
        &artifact(&validator.artifacts, "genesis")?.sha256
    } else {
        &validator.rollback.genesis_sha256
    };
    verify_regular_hash(&expected_config, config_hash)?;
    verify_regular_hash(&expected_genesis, genesis_hash)?;
    require_root_directory(
        Path::new(&validator.state_root),
        fresh_state,
        "attested validator state root",
    )?;
    if fresh_state {
        verify_generated_marker(Path::new(&validator.state_root), admitted, "fresh_state")?;
    }
    let pid_after = run_host_command(
        SYSTEMCTL,
        &[
            "show",
            "--property=MainPID",
            "--value",
            &validator.systemd_unit,
        ],
        admitted.action_deadline,
    )?;
    if pid_after != pid_bytes {
        return Err(eyre!(
            "validator MainPID changed during process attestation"
        ));
    }
    Ok(())
}

fn validate_validator_argv(
    arguments: &[PathBuf],
    executable: &Path,
    config: &Path,
    genesis: &Path,
) -> Result<()> {
    let expected = [
        executable.to_path_buf(),
        PathBuf::from("--config"),
        config.to_path_buf(),
        PathBuf::from("--genesis-manifest-json"),
        genesis.to_path_buf(),
        PathBuf::from("--sora"),
    ];
    if arguments != expected {
        return Err(eyre!(
            "validator MainPID cmdline is not the exact signed V1 argv grammar"
        ));
    }
    Ok(())
}

fn rollback_host(admitted: &HostAdmission) -> Result<()> {
    reconcile_prior_manager_operations(admitted)?;
    let rollback = rollback_nonce_root(admitted)?;
    match &admitted.target {
        HostTarget::Validator(validator) => {
            for (name, expected) in [
                ("bin/iroha3d_taira", &validator.rollback.iroha3d_sha256),
                ("bin/iroha", &validator.rollback.iroha_cli_sha256),
                ("bin/sorafs-node", &validator.rollback.sorafs_node_sha256),
                ("config/config.toml", &validator.rollback.config_sha256),
                ("genesis/genesis.json", &validator.rollback.genesis_sha256),
                (
                    "genesis/genesis.sha256",
                    &validator.rollback.genesis_hash_sha256,
                ),
            ] {
                verify_regular_hash(
                    &Path::new(&validator.rollback.release_root).join(name),
                    expected,
                )?;
            }
            stop_unit(admitted, "rollback-stop", &validator.systemd_unit)?;
            let previous = rollback.join("state");
            let fresh_trash = rollback.join("fresh-state.after");
            if previous.exists() {
                let intent = load_state_move_intent(&rollback, admitted)?;
                require_state_identity(&previous, &intent)?;
                let state = Path::new(&validator.state_root);
                if state.exists() {
                    // A crash may leave the exact post-move fresh-state
                    // closure before its final marker publication. The
                    // retained prior inode proves this path is the reset
                    // destination; reconcile only the closed generated
                    // layout before quarantining it.
                    reconcile_fresh_state(state, admitted)?;
                    verify_generated_marker(state, admitted, "fresh_state")?;
                    if fresh_trash.exists() {
                        verify_generated_marker(&fresh_trash, admitted, "fresh_state")?;
                        return Err(eyre!(
                            "rollback has both live fresh state and retained fresh-state trash"
                        ));
                    }
                    rename_noreplace(state, &fresh_trash)?;
                }
                rename_noreplace(&previous, state)?;
                require_state_identity(state, &intent)?;
            } else {
                let state = Path::new(&validator.state_root);
                if state.join(".public-reset-generated-v1.json").exists() {
                    return Err(eyre!(
                        "rollback cannot claim success while the authorization fresh-state marker remains live"
                    ));
                }
                require_root_directory(state, false, "restored validator state root")?;
                let intent_path = rollback.join("state-move.intent.json");
                if intent_path.exists() {
                    let intent = load_state_move_intent(&rollback, admitted)?;
                    require_state_identity(state, &intent)?;
                } else {
                    let intent = ensure_state_move_intent(&rollback, state, admitted)?;
                    require_state_identity(state, &intent)?;
                }
            }
            select_rollback_release(admitted, Path::new(&validator.rollback.release_root))?;
            start_unit(admitted, "rollback-start", &validator.systemd_unit)?;
            attest_validator_process(
                admitted,
                validator,
                Path::new(&validator.rollback.release_root),
                false,
            )
        }
        HostTarget::Edge(edge) => {
            verify_regular_hash(
                &Path::new(&edge.rollback_release_root).join("bin/iroha"),
                &edge.rollback_cli_sha256,
            )?;
            verify_regular_hash(
                &Path::new(&edge.rollback_release_root).join("taira.conf"),
                &edge.rollback_edge_config_sha256,
            )?;
            let backup = rollback.join("edge-config.before");
            if backup.exists() {
                verify_regular_hash(&backup, &edge.rollback_edge_config_sha256)?;
                atomic_restore_file(
                    admitted,
                    &backup,
                    Path::new(&edge.nginx_config),
                    &edge.rollback_edge_config_sha256,
                )?;
            } else if verify_regular_hash(
                Path::new(&edge.nginx_config),
                &edge.rollback_edge_config_sha256,
            )
            .is_err()
            {
                atomic_restore_file(
                    admitted,
                    &Path::new(&edge.rollback_release_root).join("taira.conf"),
                    Path::new(&edge.nginx_config),
                    &edge.rollback_edge_config_sha256,
                )?;
            }
            select_rollback_release(admitted, Path::new(&edge.rollback_release_root))?;
            run_host_command(NGINX, &["-t"], admitted.action_deadline)?;
            run_durable_manager_operation(
                admitted,
                "rollback-edge-reload",
                "reload",
                "nginx.service",
            )?;
            Ok(())
        }
    }
}

fn verify_rollback_postcondition(admitted: &HostAdmission) -> Result<()> {
    match &admitted.target {
        HostTarget::Validator(validator) => {
            let rollback = Path::new(&validator.rollback.release_root);
            if validated_current_release_target(admitted)? != rollback {
                return Err(eyre!(
                    "rollback selector no longer selects the signed prior release"
                ));
            }
            let nonce_root = Path::new(&validator.reset_guard)
                .join("rollback")
                .join(&admitted.inventory.authorization_nonce);
            require_root_directory(&nonce_root, true, "completed rollback nonce root")?;
            verify_generated_marker(&nonce_root, admitted, "rollback")?;
            if nonce_root.join("state").exists() {
                return Err(eyre!(
                    "cached rollback still retains the prior state move source"
                ));
            }
            let state = Path::new(&validator.state_root);
            require_root_directory(state, false, "restored validator state root")?;
            let intent = load_state_move_intent(&nonce_root, admitted)?;
            require_state_identity(state, &intent)?;
            let fresh_trash = nonce_root.join("fresh-state.after");
            if fresh_trash.exists() {
                verify_generated_marker(&fresh_trash, admitted, "fresh_state")?;
            }
            require_session_manager_operation_applied(
                admitted,
                "rollback-start",
                "start",
                &validator.systemd_unit,
            )?;
            require_unit_active(&validator.systemd_unit, admitted.action_deadline)?;
            attest_validator_process(admitted, validator, rollback, false)
        }
        HostTarget::Edge(edge) => {
            let rollback = Path::new(&edge.rollback_release_root);
            if validated_current_release_target(admitted)? != rollback {
                return Err(eyre!(
                    "edge rollback selector no longer selects the prior release"
                ));
            }
            verify_regular_hash(
                Path::new(&edge.nginx_config),
                &edge.rollback_edge_config_sha256,
            )?;
            require_session_manager_operation_applied(
                admitted,
                "rollback-edge-reload",
                "reload",
                "nginx.service",
            )?;
            require_unit_active("nginx.service", admitted.action_deadline)?;
            run_host_command(NGINX, &["-t"], admitted.action_deadline)?;
            Ok(())
        }
    }
}

fn select_rollback_release(admitted: &HostAdmission, release: &Path) -> Result<()> {
    require_root_directory(release, false, "authorized rollback release")?;
    select_current_release(admitted, release)
}

fn atomic_restore_file(
    admitted: &HostAdmission,
    source: &Path,
    destination: &Path,
    expected_sha256: &str,
) -> Result<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| eyre!("restore path has no parent"))?;
    require_root_directory(parent, false, "restore parent")?;
    let next = parent.join(".taira.conf.public-reset-rollback.next");
    let (mut input, snapshot) = open_pinned_regular(source, "rollback config")?;
    if !reconcile_verified_staging(&next, snapshot.len, expected_sha256)? {
        #[cfg(unix)]
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o640)
            .open(&next)?;
        #[cfg(not(unix))]
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&next)?;
        std::io::copy(&mut input, &mut output)?;
        output.sync_all()?;
        ensure_pinned_unchanged(source, "rollback config", &input, &snapshot)?;
    }
    verify_regular_hash(&next, expected_sha256)?;
    ensure_action_deadline(admitted)?;
    fs::rename(&next, destination)?;
    sync_directory(parent)
}

#[cfg(target_os = "linux")]
struct CleanupCandidate {
    path: PathBuf,
    kind: &'static str,
    parent: File,
    directory: File,
    name: OsString,
    marker: GeneratedMarkerV1,
}

#[cfg(target_os = "linux")]
struct CleanupTombstone {
    path: PathBuf,
    kind: &'static str,
    parent: File,
    directory: File,
    name: OsString,
    original_name: OsString,
    marker: Option<GeneratedMarkerV1>,
}

#[cfg(target_os = "linux")]
enum CleanupPlanEntryState {
    Complete,
    Candidate(CleanupCandidate),
    Tombstone(CleanupTombstone),
}

#[cfg(any(target_os = "linux", test))]
fn validate_cleanup_original_name(name: &str, kind: &str) -> Result<()> {
    match kind {
        "upload" => super::validate_nonce(name),
        "release"
            if name.len() == 40
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) =>
        {
            Ok(())
        }
        "release" => Err(eyre!("cleanup release name is not a canonical commit")),
        _ => Err(eyre!("generated cleanup kind is not exact V1")),
    }
}

#[cfg(any(target_os = "linux", test))]
fn cleanup_tombstone_name(original_name: &OsStr, kind: &str) -> Result<OsString> {
    let original_name = original_name
        .to_str()
        .ok_or_else(|| eyre!("generated cleanup root name is not UTF-8"))?;
    validate_cleanup_original_name(original_name, kind)?;
    Ok(OsString::from(format!(
        "{CLEANUP_TOMBSTONE_PREFIX}{original_name}{CLEANUP_TOMBSTONE_SUFFIX}"
    )))
}

#[cfg(any(target_os = "linux", test))]
fn cleanup_original_name_from_tombstone(name: &OsStr, kind: &str) -> Result<Option<OsString>> {
    if !name
        .as_encoded_bytes()
        .starts_with(CLEANUP_TOMBSTONE_PREFIX.as_bytes())
    {
        return Ok(None);
    }
    let name = name
        .to_str()
        .ok_or_else(|| eyre!("cleanup tombstone name is not exact UTF-8 V1"))?;
    let original_name = name
        .strip_prefix(CLEANUP_TOMBSTONE_PREFIX)
        .and_then(|name| name.strip_suffix(CLEANUP_TOMBSTONE_SUFFIX))
        .ok_or_else(|| eyre!("cleanup tombstone name is not exact V1"))?;
    validate_cleanup_original_name(original_name, kind)?;
    Ok(Some(OsString::from(original_name)))
}

#[cfg(target_os = "linux")]
fn cleanup_generated_waste(admitted: &HostAdmission) -> Result<(u64, u64, Vec<String>)> {
    let plan = load_or_create_cleanup_plan(admitted)?;
    execute_cleanup_plan(admitted, &plan)
}

#[cfg(target_os = "linux")]
fn load_or_create_cleanup_plan(admitted: &HostAdmission) -> Result<CleanupPlanV1> {
    let directory = ensure_host_receipt_dir(admitted)?;
    let path = directory.join("cleanup.plan.json");
    if path.exists() {
        return read_durable_cleanup_plan(admitted);
    }
    let plan = build_cleanup_plan(admitted)?;
    validate_cleanup_plan(admitted, &plan)?;
    ensure_action_deadline(admitted)?;
    publish_root_private_noreplace(
        &directory,
        "cleanup.plan.json",
        json::to_json(&plan)?.as_bytes(),
    )?;
    read_durable_cleanup_plan(admitted)
}

#[cfg(target_os = "linux")]
fn read_durable_cleanup_plan(admitted: &HostAdmission) -> Result<CleanupPlanV1> {
    let directory = ensure_host_receipt_dir(admitted)?;
    let path = directory.join("cleanup.plan.json");
    if !path.exists() {
        return Err(eyre!("cleanup receipt has no durable cleanup plan"));
    }
    sync_private_regular(&path, "cleanup plan")?;
    sync_directory(&directory)?;
    let (plan, bytes) = read_private_json::<CleanupPlanV1>(&path, "cleanup plan")?;
    if json::to_json(&plan)?.as_bytes() != bytes {
        return Err(eyre!("cleanup plan JSON is not canonical"));
    }
    validate_cleanup_plan(admitted, &plan)?;
    Ok(plan)
}

#[cfg(target_os = "linux")]
fn build_cleanup_plan(admitted: &HostAdmission) -> Result<CleanupPlanV1> {
    let policy = &admitted.inventory.cleanup;
    if !scan_cleanup_tombstone_entries(admitted, "upload", policy)?.is_empty() {
        return Err(eyre!(
            "cleanup tombstone exists without this request's durable plan"
        ));
    }
    let mut entries = Vec::new();
    let mut namespace_entries = 0_usize;
    let upload_parent = Path::new(&admitted.guard.upload_parent);
    for entry in fs::read_dir(upload_parent)? {
        ensure_action_deadline(admitted)?;
        let entry = entry?;
        namespace_entries = namespace_entries
            .checked_add(1)
            .ok_or_else(|| eyre!("cleanup namespace entry count overflow"))?;
        if namespace_entries > MAX_CLEANUP_NAMESPACE_ENTRIES {
            return Err(eyre!("cleanup namespace exceeds its entry bound"));
        }
        let name = entry.file_name();
        if cleanup_original_name_from_tombstone(&name, "upload")?.is_some() {
            continue;
        }
        let Some(name) = name.to_str() else { continue };
        if validate_cleanup_original_name(name, "upload").is_err() {
            continue;
        }
        let path = entry.path();
        if let Some(candidate) = admit_cleanup_candidate(&path, admitted, "upload", policy)? {
            entries.push(cleanup_plan_entry_from_candidate(&candidate, admitted)?);
        }
    }
    let releases = Path::new(admitted.target.service_root()).join("releases");
    if releases.exists() {
        if !scan_cleanup_tombstone_entries(admitted, "release", policy)?.is_empty() {
            return Err(eyre!(
                "cleanup tombstone exists without this request's durable plan"
            ));
        }
        let current_release = validated_current_release_target(admitted)?;
        let mut generated = Vec::new();
        for entry in fs::read_dir(&releases)? {
            ensure_action_deadline(admitted)?;
            let entry = entry?;
            namespace_entries = namespace_entries
                .checked_add(1)
                .ok_or_else(|| eyre!("cleanup namespace entry count overflow"))?;
            if namespace_entries > MAX_CLEANUP_NAMESPACE_ENTRIES {
                return Err(eyre!("cleanup namespace exceeds its entry bound"));
            }
            let name = entry.file_name();
            if cleanup_original_name_from_tombstone(&name, "release")?.is_some() {
                continue;
            }
            let Some(name) = name.to_str() else { continue };
            if name == admitted.inventory.revision.commit
                || name.len() != 40
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                continue;
            }
            let path = entry.path();
            if path == current_release {
                continue;
            }
            if let Some(candidate) = admit_cleanup_candidate(&path, admitted, "release", policy)? {
                let entry = cleanup_plan_entry_from_candidate(&candidate, admitted)?;
                generated.push((
                    candidate.marker.created_at_unix_ms,
                    entry.original_name.clone(),
                    entry,
                ));
            }
        }
        generated.sort_by(|left, right| (left.0, &left.1).cmp(&(right.0, &right.1)));
        let retain = usize::from(policy.retain_successful_releases);
        let delete_count = generated.len().saturating_sub(retain);
        entries.extend(
            generated
                .into_iter()
                .take(delete_count)
                .map(|(_, _, entry)| entry),
        );
    }
    entries.sort_by(|left, right| {
        (&left.kind, &left.original_name).cmp(&(&right.kind, &right.original_name))
    });
    let bytes_before = entries.iter().try_fold(0_u64, |total, entry| {
        total
            .checked_add(entry.initial_bytes)
            .ok_or_else(|| eyre!("cleanup byte count overflow"))
    })?;
    Ok(CleanupPlanV1 {
        schema: CLEANUP_PLAN_SCHEMA_V1.to_owned(),
        action: HostAction::Cleanup.label().to_owned(),
        host_slug: admitted.target.slug().to_owned(),
        request_sha256: admitted.request_sha256.clone(),
        inventory_sha256: admitted.inventory_sha256.clone(),
        authorization_sha256: admitted.authorization_sha256.clone(),
        authorization_nonce: admitted.inventory.authorization_nonce.clone(),
        max_reclaim_bytes: policy.max_reclaim_bytes_per_host,
        bytes_before,
        entries,
    })
}

#[cfg(any(target_os = "linux", test))]
fn validate_cleanup_plan(admitted: &HostAdmission, plan: &CleanupPlanV1) -> Result<()> {
    if plan.schema != CLEANUP_PLAN_SCHEMA_V1
        || plan.action != HostAction::Cleanup.label()
        || plan.host_slug != admitted.target.slug()
        || plan.request_sha256 != admitted.request_sha256
        || plan.inventory_sha256 != admitted.inventory_sha256
        || plan.authorization_sha256 != admitted.authorization_sha256
        || plan.authorization_nonce != admitted.inventory.authorization_nonce
        || plan.max_reclaim_bytes != admitted.inventory.cleanup.max_reclaim_bytes_per_host
        || plan.entries.len() > MAX_CLEANUP_NAMESPACE_ENTRIES
    {
        return Err(eyre!("cleanup plan is not bound to this exact request"));
    }
    let mut previous: Option<(&str, &str)> = None;
    let mut total = 0_u64;
    for entry in &plan.entries {
        validate_cleanup_original_name(&entry.original_name, &entry.kind)?;
        let expected_path = cleanup_original_path(admitted, &entry.kind, &entry.original_name)?;
        if entry.original_path != expected_path.to_string_lossy().as_ref()
            || entry.directory_inode == 0
            || entry.initial_bytes == 0
        {
            return Err(eyre!("cleanup plan entry identity is not exact"));
        }
        let marker = validate_generic_marker_fields(
            entry.marker.clone(),
            admitted.target.slug(),
            &entry.kind,
        )?;
        validate_cleanup_marker_name(&marker, OsStr::new(&entry.original_name), &entry.kind)?;
        let marker_bytes = json::to_json(&marker)?.into_bytes();
        if entry.marker_sha256 != sha256_hex(&marker_bytes) {
            return Err(eyre!("cleanup plan marker digest is not exact"));
        }
        let key = (entry.kind.as_str(), entry.original_name.as_str());
        if previous.is_some_and(|previous| previous >= key) {
            return Err(eyre!("cleanup plan entries are not unique canonical order"));
        }
        previous = Some(key);
        total = total
            .checked_add(entry.initial_bytes)
            .ok_or_else(|| eyre!("cleanup plan byte count overflow"))?;
    }
    if total != plan.bytes_before || total > plan.max_reclaim_bytes {
        return Err(eyre!(
            "cleanup plan exceeds its exact signed reclaim-byte closure"
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", test))]
fn cleanup_original_path(admitted: &HostAdmission, kind: &str, name: &str) -> Result<PathBuf> {
    validate_cleanup_original_name(name, kind)?;
    match kind {
        "upload" => Ok(Path::new(&admitted.guard.upload_parent).join(name)),
        "release" => Ok(Path::new(admitted.target.service_root())
            .join("releases")
            .join(name)),
        _ => Err(eyre!("generated cleanup kind is not exact V1")),
    }
}

#[cfg(any(target_os = "linux", test))]
fn ensure_cleanup_plan_entry_not_selected(
    kind: &str,
    original: &Path,
    selected_release: &Path,
) -> Result<()> {
    match kind {
        "upload" => Ok(()),
        "release" if original != selected_release => Ok(()),
        "release" => Err(eyre!(
            "cleanup plan release became the selected live release"
        )),
        _ => Err(eyre!("generated cleanup kind is not exact V1")),
    }
}

#[cfg(any(target_os = "linux", test))]
fn cleanup_plan_deleted_prefix(
    initial_bytes: u64,
    current_bytes: u64,
    allow_partial: bool,
) -> Result<u64> {
    if !allow_partial && current_bytes != initial_bytes {
        return Err(eyre!(
            "cleanup candidate changed before its first tombstone rename"
        ));
    }
    initial_bytes
        .checked_sub(current_bytes)
        .ok_or_else(|| eyre!("cleanup plan root grew after durable admission"))
}

#[cfg(target_os = "linux")]
fn execute_cleanup_plan(
    admitted: &HostAdmission,
    plan: &CleanupPlanV1,
) -> Result<(u64, u64, Vec<String>)> {
    validate_cleanup_plan(admitted, plan)?;
    for entry in &plan.entries {
        ensure_action_deadline(admitted)?;
        let state = open_cleanup_plan_entry(admitted, entry)?;
        let current_bytes = match &state {
            CleanupPlanEntryState::Complete => 0,
            CleanupPlanEntryState::Candidate(candidate) => {
                secure_generated_tree_bytes(candidate, admitted)?
            }
            CleanupPlanEntryState::Tombstone(tombstone) => {
                secure_cleanup_tombstone_bytes(tombstone, admitted)?
            }
        };
        let mut deleted_bytes = cleanup_plan_deleted_prefix(
            entry.initial_bytes,
            current_bytes,
            !matches!(&state, CleanupPlanEntryState::Candidate(_)),
        )?;
        match state {
            CleanupPlanEntryState::Complete => {}
            CleanupPlanEntryState::Candidate(candidate) => remove_generated_tree_beneath(
                &candidate,
                admitted,
                &mut deleted_bytes,
                entry.initial_bytes,
            )?,
            CleanupPlanEntryState::Tombstone(tombstone) => remove_cleanup_tombstone(
                &tombstone,
                admitted,
                &mut deleted_bytes,
                entry.initial_bytes,
            )?,
        }
        if deleted_bytes != entry.initial_bytes {
            return Err(eyre!(
                "cleanup plan entry did not consume its exact admitted byte closure"
            ));
        }
        let original = Path::new(&entry.original_path);
        let tombstone = original.with_file_name(cleanup_tombstone_name(
            OsStr::new(&entry.original_name),
            &entry.kind,
        )?);
        if path_exists_no_follow(original)? || path_exists_no_follow(&tombstone)? {
            return Err(eyre!(
                "cleanup plan entry remains after its completed deletion"
            ));
        }
    }
    Ok((
        plan.bytes_before,
        0,
        plan.entries
            .iter()
            .map(|entry| entry.original_path.clone())
            .collect(),
    ))
}

#[cfg(target_os = "linux")]
fn verify_completed_cleanup_plan(
    admitted: &HostAdmission,
    receipt: Option<&HostReceiptV1>,
) -> Result<()> {
    let plan = read_durable_cleanup_plan(admitted)?;
    for entry in &plan.entries {
        ensure_action_deadline(admitted)?;
        let kind = cleanup_kind(&entry.kind)?;
        let original = cleanup_original_path(admitted, kind, &entry.original_name)?;
        let tombstone = original.with_file_name(cleanup_tombstone_name(
            OsStr::new(&entry.original_name),
            kind,
        )?);
        let (_, parent) = open_cleanup_parent(admitted, kind)?;
        parent.sync_all()?;
        if path_exists_no_follow(&original)? || path_exists_no_follow(&tombstone)? {
            return Err(eyre!(
                "cached cleanup receipt has an incomplete planned root"
            ));
        }
        parent.sync_all()?;
        if path_exists_no_follow(&original)? || path_exists_no_follow(&tombstone)? {
            return Err(eyre!(
                "cached cleanup root reappeared during durable replay proof"
            ));
        }
    }
    if let Some(receipt) = receipt {
        let detail = cleanup_plan_detail(&plan);
        if receipt.bytes_before != plan.bytes_before
            || receipt.bytes_after != 0
            || receipt.reclaimed_bytes != plan.bytes_before
            || receipt.detail != detail
        {
            return Err(eyre!(
                "cached cleanup receipt differs from its immutable cleanup plan"
            ));
        }
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn verify_completed_cleanup_plan(
    _admitted: &HostAdmission,
    _receipt: Option<&HostReceiptV1>,
) -> Result<()> {
    Err(eyre!(
        "cleanup receipt replay requires Linux openat2 NO_XDEV custody"
    ))
}

#[cfg(any(target_os = "linux", test))]
fn cleanup_plan_detail(plan: &CleanupPlanV1) -> String {
    format!(
        "removed marker-authorized paths: {}",
        plan.entries
            .iter()
            .map(|entry| entry.original_path.as_str())
            .collect::<Vec<_>>()
            .join(",")
    )
}

#[cfg(target_os = "linux")]
fn open_cleanup_plan_entry(
    admitted: &HostAdmission,
    entry: &CleanupPlanEntryV1,
) -> Result<CleanupPlanEntryState> {
    let kind = cleanup_kind(&entry.kind)?;
    let original_path = cleanup_original_path(admitted, kind, &entry.original_name)?;
    if kind == "release" {
        ensure_cleanup_plan_entry_not_selected(
            kind,
            &original_path,
            &validated_current_release_target(admitted)?,
        )?;
    }
    let tombstone_name = cleanup_tombstone_name(OsStr::new(&entry.original_name), kind)?;
    let tombstone_path = original_path.with_file_name(&tombstone_name);
    let original_exists = path_exists_no_follow(&original_path)?;
    let tombstone_exists = path_exists_no_follow(&tombstone_path)?;
    match (original_exists, tombstone_exists) {
        (false, false) => {
            let (_, parent) = open_cleanup_parent(admitted, kind)?;
            parent.sync_all()?;
            if path_exists_no_follow(&original_path)? || path_exists_no_follow(&tombstone_path)? {
                return Err(eyre!(
                    "cleanup plan root reappeared while confirming durable completion"
                ));
            }
            Ok(CleanupPlanEntryState::Complete)
        }
        (true, true) => Err(eyre!("cleanup plan root and its tombstone both exist")),
        (true, false) => {
            let (parent, directory, name) =
                open_generated_cleanup_root(&original_path, admitted, kind)?;
            validate_cleanup_plan_directory(entry, &directory)?;
            let marker = validate_generic_marker_fields(
                read_generated_marker_at(&directory)?,
                admitted.target.slug(),
                kind,
            )?;
            validate_cleanup_marker_name(&marker, &name, kind)?;
            if marker != entry.marker
                || entry.marker_sha256 != sha256_hex(json::to_json(&marker)?.as_bytes())
            {
                return Err(eyre!("cleanup plan root marker drifted"));
            }
            Ok(CleanupPlanEntryState::Candidate(CleanupCandidate {
                path: original_path,
                kind,
                parent,
                directory,
                name,
                marker,
            }))
        }
        (false, true) => {
            let (_, parent) = open_cleanup_parent(admitted, kind)?;
            let before = rustix::fs::statat(
                &parent,
                &tombstone_name,
                rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
            )?;
            if rustix::fs::FileType::from_raw_mode(before.st_mode)
                != rustix::fs::FileType::Directory
            {
                return Err(eyre!("cleanup plan tombstone is not a directory"));
            }
            let directory = File::from(rustix::fs::openat2(
                &parent,
                &tombstone_name,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
                cleanup_resolve_flags(),
            )?);
            require_opened_entry_identity(&before, &directory)?;
            verify_named_root_directory(&parent, &tombstone_name, &directory)?;
            validate_cleanup_plan_directory(entry, &directory)?;
            let marker = read_regular_at(
                &directory,
                ".public-reset-generated-v1.json",
                MAX_HOST_REQUEST_BYTES,
            )?
            .map(|bytes| decode_generated_marker(&bytes))
            .transpose()?;
            if let Some(marker) = marker.as_ref() {
                let marker =
                    validate_generic_marker_fields(marker.clone(), admitted.target.slug(), kind)?;
                validate_cleanup_marker_name(&marker, OsStr::new(&entry.original_name), kind)?;
                if marker != entry.marker
                    || entry.marker_sha256 != sha256_hex(json::to_json(&marker)?.as_bytes())
                {
                    return Err(eyre!("cleanup plan tombstone marker drifted"));
                }
            } else {
                require_empty_cleanup_directory(&directory)?;
            }
            parent.sync_all()?;
            verify_named_root_directory(&parent, &tombstone_name, &directory)?;
            if path_exists_no_follow(&original_path)? {
                return Err(eyre!(
                    "cleanup original reappeared while admitting its tombstone"
                ));
            }
            Ok(CleanupPlanEntryState::Tombstone(CleanupTombstone {
                path: original_path,
                kind,
                parent,
                directory,
                name: tombstone_name,
                original_name: OsString::from(entry.original_name.as_str()),
                marker,
            }))
        }
    }
}

#[cfg(target_os = "linux")]
fn validate_cleanup_plan_directory(entry: &CleanupPlanEntryV1, directory: &File) -> Result<()> {
    let metadata = directory.metadata()?;
    if !metadata.is_dir()
        || metadata.uid() != 0
        || metadata.mode() & 0o7777 != 0o700
        || metadata.nlink() < 2
        || metadata.dev() != entry.directory_device
        || metadata.ino() != entry.directory_inode
    {
        return Err(eyre!("cleanup plan directory identity drifted"));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn cleanup_kind(kind: &str) -> Result<&'static str> {
    match kind {
        "upload" => Ok("upload"),
        "release" => Ok("release"),
        _ => Err(eyre!("generated cleanup kind is not exact V1")),
    }
}

#[cfg(target_os = "linux")]
fn path_exists_no_follow(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

#[cfg(not(target_os = "linux"))]
fn cleanup_generated_waste(_admitted: &HostAdmission) -> Result<(u64, u64, Vec<String>)> {
    Err(eyre!(
        "marker-bound cleanup requires Linux openat2 NO_XDEV custody"
    ))
}

#[cfg(any(target_os = "linux", test))]
fn validate_generic_marker_fields(
    marker: GeneratedMarkerV1,
    host_slug: &str,
    kind: &str,
) -> Result<GeneratedMarkerV1> {
    if marker.schema != GENERATED_MARKER_SCHEMA_V1
        || marker.kind != kind
        || marker.host_slug != host_slug
    {
        return Err(eyre!("generated cleanup marker identity is not exact V1"));
    }
    require_lower_sha256(
        &marker.inventory_sha256,
        "generated marker inventory digest",
    )?;
    super::validate_nonce(&marker.authorization_nonce)?;
    if marker.revision.len() != 40
        || !marker
            .revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(eyre!("generated cleanup marker revision is not canonical"));
    }
    Ok(marker)
}

#[cfg(target_os = "linux")]
fn admit_cleanup_candidate(
    root: &Path,
    admitted: &HostAdmission,
    kind: &'static str,
    policy: &super::CleanupV1,
) -> Result<Option<CleanupCandidate>> {
    let (parent, directory, name) = open_generated_cleanup_root(root, admitted, kind)?;
    let marker = validate_generic_marker_fields(
        read_generated_marker_at(&directory)?,
        admitted.target.slug(),
        kind,
    )?;
    validate_cleanup_marker_name(&marker, &name, kind)?;
    let minimum_ms = policy.minimum_age_secs.saturating_mul(1_000);
    if now_unix_ms()?.saturating_sub(marker.created_at_unix_ms) < minimum_ms {
        return Ok(None);
    }
    Ok(Some(CleanupCandidate {
        path: root.to_path_buf(),
        kind,
        parent,
        directory,
        name,
        marker,
    }))
}

#[cfg(any(target_os = "linux", test))]
fn validate_cleanup_marker_name(
    marker: &GeneratedMarkerV1,
    original_name: &OsStr,
    kind: &str,
) -> Result<()> {
    if (kind == "upload" && original_name != OsStr::new(&marker.authorization_nonce))
        || (kind == "release" && original_name != OsStr::new(&marker.revision))
    {
        return Err(eyre!(
            "generated cleanup marker does not bind its directory name"
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn read_generated_marker_at(directory: &File) -> Result<GeneratedMarkerV1> {
    let bytes = read_regular_at(
        directory,
        ".public-reset-generated-v1.json",
        MAX_HOST_REQUEST_BYTES,
    )?
    .ok_or_else(|| eyre!("generated cleanup marker is missing"))?;
    decode_generated_marker(&bytes)
}

#[cfg(target_os = "linux")]
fn decode_generated_marker(bytes: &[u8]) -> Result<GeneratedMarkerV1> {
    let marker: GeneratedMarkerV1 =
        json::from_slice(bytes).wrap_err("generated cleanup marker is invalid")?;
    if json::to_json(&marker)?.as_bytes() != bytes {
        return Err(eyre!("generated cleanup marker JSON is not canonical"));
    }
    Ok(marker)
}

#[cfg(target_os = "linux")]
fn scan_cleanup_tombstone_entries(
    admitted: &HostAdmission,
    kind: &'static str,
    policy: &super::CleanupV1,
) -> Result<Vec<CleanupPlanEntryV1>> {
    let (parent_path, parent) = open_cleanup_parent(admitted, kind)?;
    let mut names = Vec::new();
    for entry in rustix::fs::Dir::read_from(&parent)? {
        let entry = entry?;
        if matches!(entry.file_name().to_bytes(), b"." | b"..") {
            continue;
        }
        if names.len() >= MAX_CLEANUP_NAMESPACE_ENTRIES {
            return Err(eyre!("cleanup namespace exceeds its entry bound"));
        }
        names.push(OsStr::from_bytes(entry.file_name().to_bytes()).to_os_string());
    }
    names.sort();
    let mut entries = Vec::new();
    for name in names {
        let Some(original_name) = cleanup_original_name_from_tombstone(&name, kind)? else {
            continue;
        };
        ensure_action_deadline(admitted)?;
        let before = rustix::fs::statat(&parent, &name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)?;
        if rustix::fs::FileType::from_raw_mode(before.st_mode) != rustix::fs::FileType::Directory {
            return Err(eyre!("cleanup tombstone is not a directory"));
        }
        let directory = File::from(rustix::fs::openat2(
            &parent,
            &name,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
            cleanup_resolve_flags(),
        )?);
        require_opened_entry_identity(&before, &directory)?;
        verify_named_root_directory(&parent, &name, &directory)?;
        let marker = read_regular_at(
            &directory,
            ".public-reset-generated-v1.json",
            MAX_HOST_REQUEST_BYTES,
        )?
        .map(|bytes| decode_generated_marker(&bytes))
        .transpose()?
        .map(|marker| validate_generic_marker_fields(marker, admitted.target.slug(), kind))
        .transpose()?;
        if let Some(marker) = marker.as_ref() {
            validate_cleanup_marker_name(marker, &original_name, kind)?;
            let minimum_ms = policy.minimum_age_secs.saturating_mul(1_000);
            if now_unix_ms()?.saturating_sub(marker.created_at_unix_ms) < minimum_ms {
                return Err(eyre!(
                    "in-progress cleanup tombstone predates the signed minimum age"
                ));
            }
        } else {
            require_empty_cleanup_directory(&directory)?;
        }
        let tombstone = CleanupTombstone {
            path: parent_path.join(&original_name),
            kind,
            parent: parent.try_clone()?,
            directory,
            name,
            original_name,
            marker,
        };
        entries.push(cleanup_plan_entry_from_tombstone(&tombstone, admitted)?);
    }
    Ok(entries)
}

#[cfg(target_os = "linux")]
fn secure_generated_tree_bytes(
    candidate: &CleanupCandidate,
    admitted: &HostAdmission,
) -> Result<u64> {
    let mut entries = 0_usize;
    inspect_generated_directory(&candidate.directory, admitted, 0, &mut entries)
}

#[cfg(target_os = "linux")]
fn secure_cleanup_tombstone_bytes(
    tombstone: &CleanupTombstone,
    admitted: &HostAdmission,
) -> Result<u64> {
    if tombstone.marker.is_none() {
        require_empty_cleanup_directory(&tombstone.directory)?;
        return Ok(0);
    }
    let mut entries = 0_usize;
    inspect_generated_directory(&tombstone.directory, admitted, 0, &mut entries)
}

#[cfg(target_os = "linux")]
fn cleanup_plan_entry_from_candidate(
    candidate: &CleanupCandidate,
    admitted: &HostAdmission,
) -> Result<CleanupPlanEntryV1> {
    cleanup_plan_entry(
        candidate.kind,
        &candidate.name,
        &candidate.path,
        &candidate.marker,
        &candidate.directory,
        secure_generated_tree_bytes(candidate, admitted)?,
    )
}

#[cfg(target_os = "linux")]
fn cleanup_plan_entry_from_tombstone(
    tombstone: &CleanupTombstone,
    admitted: &HostAdmission,
) -> Result<CleanupPlanEntryV1> {
    let marker = tombstone
        .marker
        .as_ref()
        .ok_or_else(|| eyre!("markerless cleanup tombstone has no durable cleanup plan"))?;
    cleanup_plan_entry(
        tombstone.kind,
        &tombstone.original_name,
        &tombstone.path,
        marker,
        &tombstone.directory,
        secure_cleanup_tombstone_bytes(tombstone, admitted)?,
    )
}

#[cfg(target_os = "linux")]
fn cleanup_plan_entry(
    kind: &str,
    original_name: &OsStr,
    original_path: &Path,
    marker: &GeneratedMarkerV1,
    directory: &File,
    initial_bytes: u64,
) -> Result<CleanupPlanEntryV1> {
    let original_name = original_name
        .to_str()
        .ok_or_else(|| eyre!("cleanup plan root name is not UTF-8"))?;
    validate_cleanup_original_name(original_name, kind)?;
    validate_cleanup_marker_name(marker, OsStr::new(original_name), kind)?;
    let original_path = original_path
        .to_str()
        .ok_or_else(|| eyre!("cleanup plan path is not UTF-8"))?;
    let marker_bytes = json::to_json(marker)?.into_bytes();
    let metadata = directory.metadata()?;
    if metadata.ino() == 0 || metadata.mode() & 0o7777 != 0o700 || initial_bytes == 0 {
        return Err(eyre!(
            "cleanup plan entry lacks a stable directory identity"
        ));
    }
    Ok(CleanupPlanEntryV1 {
        kind: kind.to_owned(),
        original_name: original_name.to_owned(),
        original_path: original_path.to_owned(),
        marker: marker.clone(),
        marker_sha256: sha256_hex(&marker_bytes),
        directory_device: metadata.dev(),
        directory_inode: metadata.ino(),
        initial_bytes,
    })
}

#[cfg(target_os = "linux")]
fn open_generated_cleanup_root(
    root: &Path,
    admitted: &HostAdmission,
    kind: &str,
) -> Result<(File, File, OsString)> {
    let name = root
        .file_name()
        .ok_or_else(|| eyre!("generated cleanup root has no name"))?
        .to_os_string();
    if !matches!(
        Path::new(&name).components().collect::<Vec<_>>().as_slice(),
        [std::path::Component::Normal(_)]
    ) {
        return Err(eyre!("generated cleanup root name is not one component"));
    }
    let (parent_path, parent_file) = open_cleanup_parent(admitted, kind)?;
    if root != parent_path.join(&name) {
        return Err(eyre!("generated cleanup root escaped its exact namespace"));
    }
    let directory = File::from(
        rustix::fs::openat2(
            &parent_file,
            &name,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
            cleanup_resolve_flags(),
        )
        .wrap_err("failed to open generated cleanup root beneath its parent")?,
    );
    verify_named_root_directory(&parent_file, &name, &directory)?;
    Ok((parent_file, directory, name))
}

#[cfg(target_os = "linux")]
fn cleanup_parent_relative(kind: &str) -> Result<&'static Path> {
    match kind {
        "upload" => Ok(Path::new(".public-reset-upload-v1")),
        "release" => Ok(Path::new("releases")),
        _ => Err(eyre!("generated cleanup kind is not exact V1")),
    }
}

#[cfg(target_os = "linux")]
fn open_cleanup_parent(admitted: &HostAdmission, kind: &str) -> Result<(PathBuf, File)> {
    let service_root = Path::new(admitted.target.service_root());
    require_root_directory(service_root, false, "cleanup service root")?;
    let relative = cleanup_parent_relative(kind)?;
    let parent_path = service_root.join(relative);
    require_root_directory(
        &parent_path,
        kind == "upload",
        "generated cleanup namespace",
    )?;
    let service = File::open(service_root)?;
    let parent = File::from(rustix::fs::openat2(
        &service,
        relative,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::DIRECTORY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
        cleanup_resolve_flags(),
    )?);
    let metadata = parent.metadata()?;
    if !metadata.is_dir()
        || metadata.uid() != 0
        || metadata.mode() & 0o022 != 0
        || metadata.nlink() < 2
    {
        return Err(eyre!("generated cleanup namespace lacks root custody"));
    }
    Ok((parent_path, parent))
}

#[cfg(target_os = "linux")]
fn cleanup_resolve_flags() -> rustix::fs::ResolveFlags {
    rustix::fs::ResolveFlags::BENEATH
        | rustix::fs::ResolveFlags::NO_SYMLINKS
        | rustix::fs::ResolveFlags::NO_XDEV
}

#[cfg(target_os = "linux")]
fn verify_named_root_directory(parent: &File, name: &OsStr, directory: &File) -> Result<()> {
    let metadata = directory.metadata()?;
    let named = rustix::fs::statat(parent, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)?;
    if !metadata.is_dir()
        || metadata.uid() != 0
        || metadata.mode() & 0o022 != 0
        || metadata.nlink() < 2
        || named.st_dev as u64 != metadata.dev()
        || named.st_ino as u64 != metadata.ino()
    {
        return Err(eyre!(
            "generated cleanup root changed or lacks root directory custody"
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn inspect_generated_directory(
    directory: &File,
    admitted: &HostAdmission,
    depth: usize,
    entries: &mut usize,
) -> Result<u64> {
    ensure_action_deadline(admitted)?;
    if depth > 128 {
        return Err(eyre!("generated cleanup tree exceeds its depth bound"));
    }
    let mut total = 0_u64;
    for entry in rustix::fs::Dir::read_from(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        if matches!(name.to_bytes(), b"." | b"..") {
            continue;
        }
        *entries = entries
            .checked_add(1)
            .ok_or_else(|| eyre!("generated cleanup entry count overflow"))?;
        if *entries > MAX_CLEANUP_ENTRIES {
            return Err(eyre!("generated cleanup tree exceeds its entry bound"));
        }
        ensure_action_deadline(admitted)?;
        let before = rustix::fs::statat(directory, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)?;
        match rustix::fs::FileType::from_raw_mode(before.st_mode) {
            rustix::fs::FileType::Directory => {
                let child = File::from(rustix::fs::openat2(
                    directory,
                    name,
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::DIRECTORY
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                    cleanup_resolve_flags(),
                )?);
                require_opened_entry_identity(&before, &child)?;
                total = total
                    .checked_add(inspect_generated_directory(
                        &child,
                        admitted,
                        depth + 1,
                        entries,
                    )?)
                    .ok_or_else(|| eyre!("cleanup byte count overflow"))?;
            }
            rustix::fs::FileType::RegularFile => {
                let file = File::from(rustix::fs::openat2(
                    directory,
                    name,
                    rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                    cleanup_resolve_flags(),
                )?);
                require_opened_entry_identity(&before, &file)?;
                total = total
                    .checked_add(file.metadata()?.len())
                    .ok_or_else(|| eyre!("cleanup byte count overflow"))?;
            }
            _ => return Err(eyre!("generated cleanup tree contains a special entry")),
        }
    }
    Ok(total)
}

#[cfg(target_os = "linux")]
fn require_opened_entry_identity(before: &rustix::fs::Stat, opened: &File) -> Result<()> {
    let metadata = opened.metadata()?;
    if before.st_dev as u64 != metadata.dev()
        || before.st_ino as u64 != metadata.ino()
        || metadata.uid() != 0
        || metadata.mode() & 0o022 != 0
        || (metadata.is_file() && metadata.nlink() != 1)
        || (metadata.is_dir() && metadata.nlink() < 2)
    {
        return Err(eyre!(
            "generated cleanup entry changed or lacks root custody"
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn remove_generated_tree_beneath(
    candidate: &CleanupCandidate,
    admitted: &HostAdmission,
    deleted_bytes: &mut u64,
    byte_cap: u64,
) -> Result<()> {
    verify_named_root_directory(&candidate.parent, &candidate.name, &candidate.directory)?;
    let marker = validate_generic_marker_fields(
        read_generated_marker_at(&candidate.directory)?,
        admitted.target.slug(),
        candidate.kind,
    )?;
    if marker != candidate.marker {
        return Err(eyre!(
            "generated cleanup marker changed after candidate admission"
        ));
    }
    let tombstone_name = cleanup_tombstone_name(&candidate.name, candidate.kind)?;
    ensure_action_deadline(admitted)?;
    if candidate.kind == "release" {
        ensure_cleanup_plan_entry_not_selected(
            candidate.kind,
            &candidate.path,
            &validated_current_release_target(admitted)?,
        )?;
    }
    ensure_action_deadline(admitted)?;
    rustix::fs::renameat_with(
        &candidate.parent,
        &candidate.name,
        &candidate.parent,
        &tombstone_name,
        rustix::fs::RenameFlags::NOREPLACE,
    )?;
    candidate.parent.sync_all()?;
    let tombstone = CleanupTombstone {
        path: candidate.path.clone(),
        kind: candidate.kind,
        parent: candidate.parent.try_clone()?,
        directory: candidate.directory.try_clone()?,
        name: tombstone_name,
        original_name: candidate.name.clone(),
        marker: Some(marker),
    };
    remove_cleanup_tombstone(&tombstone, admitted, deleted_bytes, byte_cap)
}

#[cfg(target_os = "linux")]
fn remove_cleanup_tombstone(
    tombstone: &CleanupTombstone,
    admitted: &HostAdmission,
    deleted_bytes: &mut u64,
    byte_cap: u64,
) -> Result<()> {
    verify_named_root_directory(&tombstone.parent, &tombstone.name, &tombstone.directory)?;
    if let Some(expected_marker) = tombstone.marker.as_ref() {
        let marker = validate_generic_marker_fields(
            read_generated_marker_at(&tombstone.directory)?,
            admitted.target.slug(),
            tombstone.kind,
        )?;
        validate_cleanup_marker_name(&marker, &tombstone.original_name, tombstone.kind)?;
        if &marker != expected_marker {
            return Err(eyre!(
                "generated cleanup marker changed after tombstone admission"
            ));
        }
        let mut entries = 0_usize;
        remove_generated_directory_contents(
            &tombstone.directory,
            admitted,
            0,
            &mut entries,
            deleted_bytes,
            byte_cap,
        )?;
        let marker_before = rustix::fs::statat(
            &tombstone.directory,
            ".public-reset-generated-v1.json",
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )?;
        let marker_bytes = read_regular_at(
            &tombstone.directory,
            ".public-reset-generated-v1.json",
            MAX_HOST_REQUEST_BYTES,
        )?
        .ok_or_else(|| eyre!("generated cleanup marker vanished before final unlink"))?;
        let marker = validate_generic_marker_fields(
            decode_generated_marker(&marker_bytes)?,
            admitted.target.slug(),
            tombstone.kind,
        )?;
        validate_cleanup_marker_name(&marker, &tombstone.original_name, tombstone.kind)?;
        if &marker != expected_marker {
            return Err(eyre!(
                "generated cleanup marker changed before final unlink"
            ));
        }
        let prospective = deleted_bytes
            .checked_add(
                u64::try_from(marker_bytes.len())
                    .map_err(|_| eyre!("generated cleanup marker length overflow"))?,
            )
            .ok_or_else(|| eyre!("cleanup byte count overflow"))?;
        if prospective > byte_cap {
            return Err(eyre!(
                "generated cleanup grew beyond the signed reclaim cap before marker unlink"
            ));
        }
        let marker_after = rustix::fs::statat(
            &tombstone.directory,
            ".public-reset-generated-v1.json",
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )?;
        require_unchanged_regular_stat(&marker_before, &marker_after)?;
        ensure_action_deadline(admitted)?;
        rustix::fs::unlinkat(
            &tombstone.directory,
            ".public-reset-generated-v1.json",
            rustix::fs::AtFlags::empty(),
        )?;
        *deleted_bytes = prospective;
        tombstone.directory.sync_all()?;
    } else {
        require_empty_cleanup_directory(&tombstone.directory)?;
        tombstone.directory.sync_all()?;
    }
    verify_named_root_directory(&tombstone.parent, &tombstone.name, &tombstone.directory)?;
    ensure_action_deadline(admitted)?;
    rustix::fs::unlinkat(
        &tombstone.parent,
        &tombstone.name,
        rustix::fs::AtFlags::REMOVEDIR,
    )?;
    tombstone.parent.sync_all()?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn require_empty_cleanup_directory(directory: &File) -> Result<()> {
    for entry in rustix::fs::Dir::read_from(directory)? {
        let entry = entry?;
        if !matches!(entry.file_name().to_bytes(), b"." | b"..") {
            return Err(eyre!(
                "markerless cleanup tombstone contains unfinished entries"
            ));
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn remove_generated_directory_contents(
    directory: &File,
    admitted: &HostAdmission,
    depth: usize,
    entries: &mut usize,
    deleted_bytes: &mut u64,
    byte_cap: u64,
) -> Result<()> {
    ensure_action_deadline(admitted)?;
    if depth > 128 {
        return Err(eyre!("generated cleanup tree exceeds its depth bound"));
    }
    let mut names = Vec::new();
    for entry in rustix::fs::Dir::read_from(directory)? {
        let entry = entry?;
        if matches!(entry.file_name().to_bytes(), b"." | b"..") {
            continue;
        }
        ensure_action_deadline(admitted)?;
        if names.len() >= MAX_CLEANUP_ENTRIES.saturating_sub(*entries) {
            return Err(eyre!("generated cleanup tree exceeds its entry bound"));
        }
        names.push(OsStr::from_bytes(entry.file_name().to_bytes()).to_os_string());
    }
    names.sort();
    for name in names {
        *entries = entries
            .checked_add(1)
            .ok_or_else(|| eyre!("generated cleanup entry count overflow"))?;
        if *entries > MAX_CLEANUP_ENTRIES {
            return Err(eyre!("generated cleanup tree exceeds its entry bound"));
        }
        ensure_action_deadline(admitted)?;
        if depth == 0 && name == OsStr::new(".public-reset-generated-v1.json") {
            continue;
        }
        let before = rustix::fs::statat(directory, &name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)?;
        match rustix::fs::FileType::from_raw_mode(before.st_mode) {
            rustix::fs::FileType::Directory => {
                let child = File::from(rustix::fs::openat2(
                    directory,
                    &name,
                    rustix::fs::OFlags::RDONLY
                        | rustix::fs::OFlags::DIRECTORY
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                    cleanup_resolve_flags(),
                )?);
                require_opened_entry_identity(&before, &child)?;
                remove_generated_directory_contents(
                    &child,
                    admitted,
                    depth + 1,
                    entries,
                    deleted_bytes,
                    byte_cap,
                )?;
                let after =
                    rustix::fs::statat(directory, &name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)?;
                require_same_stat(&before, &after)?;
                ensure_action_deadline(admitted)?;
                rustix::fs::unlinkat(directory, &name, rustix::fs::AtFlags::REMOVEDIR)?;
            }
            rustix::fs::FileType::RegularFile => {
                let file = File::from(rustix::fs::openat2(
                    directory,
                    &name,
                    rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::empty(),
                    cleanup_resolve_flags(),
                )?);
                require_opened_entry_identity(&before, &file)?;
                let prospective = deleted_bytes
                    .checked_add(file.metadata()?.len())
                    .ok_or_else(|| eyre!("cleanup byte count overflow"))?;
                if prospective > byte_cap {
                    return Err(eyre!(
                        "generated cleanup grew beyond the signed reclaim cap before unlink"
                    ));
                }
                let after =
                    rustix::fs::statat(directory, &name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)?;
                require_unchanged_regular_stat(&before, &after)?;
                ensure_action_deadline(admitted)?;
                rustix::fs::unlinkat(directory, &name, rustix::fs::AtFlags::empty())?;
                *deleted_bytes = prospective;
            }
            _ => return Err(eyre!("generated cleanup tree contains a special entry")),
        }
    }
    directory.sync_all()?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn require_same_stat(before: &rustix::fs::Stat, after: &rustix::fs::Stat) -> Result<()> {
    if before.st_dev != after.st_dev
        || before.st_ino != after.st_ino
        || before.st_mode != after.st_mode
        || before.st_uid != after.st_uid
        || before.st_gid != after.st_gid
    {
        return Err(eyre!("generated cleanup entry changed before unlink"));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn require_unchanged_regular_stat(
    before: &rustix::fs::Stat,
    after: &rustix::fs::Stat,
) -> Result<()> {
    require_same_stat(before, after)?;
    if before.st_nlink != 1
        || after.st_nlink != 1
        || before.st_size != after.st_size
        || rustix::fs::FileType::from_raw_mode(after.st_mode) != rustix::fs::FileType::RegularFile
    {
        return Err(eyre!(
            "generated cleanup regular file changed before unlink"
        ));
    }
    Ok(())
}

fn ensure_generated_directory(
    path: &Path,
    admitted: &HostAdmission,
    kind: &str,
    mode: u32,
) -> Result<()> {
    ensure_root_directory_with_mode(path, mode)?;
    let marker = GeneratedMarkerV1 {
        schema: GENERATED_MARKER_SCHEMA_V1.to_owned(),
        kind: kind.to_owned(),
        host_slug: admitted.target.slug().to_owned(),
        inventory_sha256: admitted.inventory_sha256.clone(),
        authorization_nonce: admitted.inventory.authorization_nonce.clone(),
        revision: admitted.inventory.revision.commit.clone(),
        created_at_unix_ms: admitted.authorization.claims.issued_at_unix_ms,
    };
    let bytes = json::to_json(&marker)?.into_bytes();
    publish_root_private_noreplace(path, ".public-reset-generated-v1.json", &bytes)
}

fn read_generated_marker(root: &Path) -> Result<GeneratedMarkerV1> {
    let path = root.join(".public-reset-generated-v1.json");
    let (marker, _) = read_private_json(&path, "generated-path marker")?;
    Ok(marker)
}

fn verify_generated_marker(root: &Path, admitted: &HostAdmission, kind: &str) -> Result<()> {
    verify_marker_fields(&read_generated_marker(root)?, admitted, kind)
}

fn verify_marker_fields(
    marker: &GeneratedMarkerV1,
    admitted: &HostAdmission,
    kind: &str,
) -> Result<()> {
    if marker.schema != GENERATED_MARKER_SCHEMA_V1
        || marker.kind != kind
        || marker.host_slug != admitted.target.slug()
        || marker.inventory_sha256 != admitted.inventory_sha256
        || marker.authorization_nonce != admitted.inventory.authorization_nonce
        || marker.revision != admitted.inventory.revision.commit
        || marker.created_at_unix_ms != admitted.authorization.claims.issued_at_unix_ms
    {
        return Err(eyre!(
            "generated path marker is not bound to this authorization"
        ));
    }
    Ok(())
}

fn ensure_root_directory_with_mode(path: &Path, mode: u32) -> Result<()> {
    if path.exists() {
        let private = mode == 0o700;
        return require_root_directory(path, private, "generated directory");
    }
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("generated directory has no parent"))?;
    require_root_directory(parent, false, "generated directory parent")?;
    fs::create_dir(path)?;
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    sync_directory(parent)?;
    require_root_directory(path, mode == 0o700, "generated directory")
}

fn publish_root_private_noreplace(directory: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    if name.is_empty()
        || name.len() > 192
        || name.contains('/')
        || name == "."
        || name == ".."
        || bytes.len() > MAX_HOST_REQUEST_BYTES
    {
        return Err(eyre!(
            "root-private publication escaped its closed namespace"
        ));
    }
    require_root_directory(directory, true, "root-private publication directory")?;
    let parent = File::from(rustix::fs::open(
        directory,
        rustix::fs::OFlags::RDONLY
            | rustix::fs::OFlags::DIRECTORY
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?);
    if let Some(actual) = read_regular_at(&parent, name, bytes.len().max(1) + 1)? {
        return if actual == bytes {
            sync_regular_at(&parent, name)?;
            parent.sync_all()?;
            Ok(())
        } else {
            Err(eyre!(
                "root-private destination exists with different bytes"
            ))
        };
    }
    let staging_name = format!(".{name}.next");
    match rustix::fs::openat(
        &parent,
        staging_name.as_str(),
        rustix::fs::OFlags::WRONLY
            | rustix::fs::OFlags::CREATE
            | rustix::fs::OFlags::EXCL
            | rustix::fs::OFlags::NOFOLLOW
            | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::from_raw_mode(0o600),
    ) {
        Ok(fd) => {
            let mut file = File::from(fd);
            file.write_all(bytes)?;
            file.sync_all()?;
        }
        Err(rustix::io::Errno::EXIST) => {
            let actual = read_regular_at(&parent, &staging_name, bytes.len().max(1) + 1)?
                .ok_or_else(|| eyre!("stale root-private staging vanished"))?;
            if actual == bytes {
                // Equality can be observed from page cache before a prior
                // fsync. Make the retained inode durable before publication.
                sync_regular_at(&parent, &staging_name)?;
            } else if actual.len() < bytes.len() {
                rustix::fs::unlinkat(&parent, staging_name.as_str(), rustix::fs::AtFlags::empty())?;
                parent.sync_all()?;
                let mut file = File::from(rustix::fs::openat(
                    &parent,
                    staging_name.as_str(),
                    rustix::fs::OFlags::WRONLY
                        | rustix::fs::OFlags::CREATE
                        | rustix::fs::OFlags::EXCL
                        | rustix::fs::OFlags::NOFOLLOW
                        | rustix::fs::OFlags::CLOEXEC,
                    rustix::fs::Mode::from_raw_mode(0o600),
                )?);
                file.write_all(bytes)?;
                file.sync_all()?;
            } else {
                return Err(eyre!("stale root-private staging differs from retry"));
            }
        }
        Err(error) => return Err(error.into()),
    }
    match rustix::fs::renameat_with(
        &parent,
        staging_name.as_str(),
        &parent,
        name,
        rustix::fs::RenameFlags::NOREPLACE,
    ) {
        Ok(()) => parent.sync_all()?,
        Err(rustix::io::Errno::EXIST) => {
            let actual = read_regular_at(&parent, name, bytes.len().max(1) + 1)?
                .ok_or_else(|| eyre!("root-private destination vanished after race"))?;
            if actual != bytes {
                return Err(eyre!(
                    "root-private no-replace race produced different bytes"
                ));
            }
            sync_regular_at(&parent, name)?;
            rustix::fs::unlinkat(&parent, staging_name.as_str(), rustix::fs::AtFlags::empty())?;
            parent.sync_all()?;
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn read_regular_at(parent: &File, name: &str, maximum: usize) -> Result<Option<Vec<u8>>> {
    let fd = match rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut file = File::from(fd);
    let metadata = file.metadata()?;
    #[cfg(unix)]
    if !metadata.is_file()
        || metadata.uid() != 0
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(eyre!("root-private publication file has unsafe custody"));
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(u64::try_from(maximum).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)?;
    if bytes.len() >= maximum {
        return Err(eyre!("root-private publication exceeds its byte bound"));
    }
    Ok(Some(bytes))
}

fn sync_regular_at(parent: &File, name: &str) -> Result<()> {
    let file = File::from(rustix::fs::openat(
        parent,
        name,
        rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?);
    let metadata = file.metadata()?;
    #[cfg(unix)]
    if !metadata.is_file()
        || metadata.uid() != 0
        || metadata.mode() & 0o7777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(eyre!("root-private staging file has unsafe custody"));
    }
    file.sync_all()
        .wrap_err("failed to fsync retained root-private file")
}

fn rename_noreplace(source: &Path, destination: &Path) -> Result<()> {
    let source_parent = source
        .parent()
        .ok_or_else(|| eyre!("rename source has no parent"))?;
    let destination_parent = destination
        .parent()
        .ok_or_else(|| eyre!("rename destination has no parent"))?;
    require_root_directory(source_parent, false, "rename source parent")?;
    require_root_directory(destination_parent, false, "rename destination parent")?;
    let source_dir = File::open(source_parent)?;
    let destination_dir = File::open(destination_parent)?;
    rustix::fs::renameat_with(
        &source_dir,
        source
            .file_name()
            .ok_or_else(|| eyre!("rename source has no name"))?,
        &destination_dir,
        destination
            .file_name()
            .ok_or_else(|| eyre!("rename destination has no name"))?,
        rustix::fs::RenameFlags::NOREPLACE,
    )?;
    source_dir.sync_all()?;
    destination_dir.sync_all()?;
    Ok(())
}

#[derive(Debug)]
pub(super) struct ProcessSpec {
    program: PathBuf,
    args: Vec<OsString>,
    stdin_prefix: Vec<u8>,
    stdin_file: Option<(File, u64)>,
    inherited_files: Vec<File>,
    deadline: Instant,
}

#[derive(Debug)]
pub(super) struct ProcessOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

pub(super) trait ProcessRunner {
    fn run(&mut self, spec: &ProcessSpec) -> Result<ProcessOutput>;
}

#[derive(Default)]
pub(super) struct RealProcessRunner;

impl ProcessRunner for RealProcessRunner {
    fn run(&mut self, spec: &ProcessSpec) -> Result<ProcessOutput> {
        run_bounded_process(spec)
    }
}

#[cfg(unix)]
#[allow(
    unsafe_code,
    reason = "kill(-pgid, SIGKILL) is the fixed Unix process-group cleanup primitive"
)]
fn terminate_owned_child(child: &mut Child) -> Result<()> {
    unsafe extern "C" {
        fn kill(pid: i32, signal: i32) -> i32;
    }
    let process_group = i32::try_from(child.id()).wrap_err("child PID does not fit i32")?;
    // Every child is made its own process-group leader before spawn returns.
    // ESRCH is success: it means the group has already drained.
    if unsafe { kill(-process_group, 9) } != 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(3) {
            if child
                .try_wait()
                .wrap_err("failed to inspect owned child after group-signal failure")?
                .is_some()
            {
                // The leader can become a zombie between the group signal and
                // `try_wait` on Darwin. Recheck the group after reaping: ESRCH
                // proves no descendant retained the process group, while a
                // successful signal drains any remaining descendants.
                if unsafe { kill(-process_group, 9) } == 0
                    || std::io::Error::last_os_error().raw_os_error() == Some(3)
                {
                    return Ok(());
                }
                return Err(error).wrap_err("failed to terminate owned child process group");
            }
            let _ = child.kill();
            child
                .wait()
                .wrap_err("failed to reap owned child after group-signal failure")?;
            if unsafe { kill(-process_group, 9) } == 0
                || std::io::Error::last_os_error().raw_os_error() == Some(3)
            {
                return Ok(());
            }
            return Err(error).wrap_err("failed to terminate owned child process group");
        }
    }
    let _ = child.kill();
    child
        .wait()
        .wrap_err("failed to reap owned child process")?;
    Ok(())
}

#[cfg(not(unix))]
fn terminate_owned_child(child: &mut Child) -> Result<()> {
    let _ = child.kill();
    child
        .wait()
        .wrap_err("failed to reap owned child process")?;
    Ok(())
}

#[allow(
    unsafe_code,
    reason = "pre_exec clears CLOEXEC only on retained, already-open input descriptors"
)]
fn run_bounded_process(spec: &ProcessSpec) -> Result<ProcessOutput> {
    if spec.deadline <= Instant::now() {
        return Err(eyre!("child process deadline must be in the future"));
    }
    let mut source = match &spec.stdin_file {
        Some((file, expected)) => {
            let mut clone = file
                .try_clone()
                .wrap_err("failed to duplicate pinned streamed input descriptor")?;
            clone
                .rewind()
                .wrap_err("failed to rewind pinned streamed input")?;
            Some((clone, *expected, 0_u64))
        }
        None => None,
    };
    let deadline = spec.deadline;
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .env_clear()
        .env("LC_ALL", "C")
        .current_dir("/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        command.process_group(0);
        let inherited_descriptors = spec
            .inherited_files
            .iter()
            .map(|file| file.as_raw_fd())
            .collect::<Vec<_>>();
        unsafe {
            command.pre_exec(move || {
                for descriptor in &inherited_descriptors {
                    let descriptor = std::os::fd::BorrowedFd::borrow_raw(*descriptor);
                    rustix::io::fcntl_setfd(descriptor, rustix::io::FdFlags::empty())
                        .map_err(std::io::Error::from)?;
                }
                Ok(())
            });
        }
    }
    let mut child = command
        .spawn()
        .wrap_err_with(|| format!("failed to spawn `{}`", spec.program.display()))?;
    let mut stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            terminate_owned_child(&mut child)?;
            return Err(eyre!("child stdout pipe missing"));
        }
    };
    let mut stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            terminate_owned_child(&mut child)?;
            return Err(eyre!("child stderr pipe missing"));
        }
    };
    let stdin = match child.stdin.take() {
        Some(stdin) => stdin,
        None => {
            terminate_owned_child(&mut child)?;
            return Err(eyre!("child stdin pipe missing"));
        }
    };
    for (descriptor, label) in [
        (stdin.as_fd(), "stdin"),
        (stdout.as_fd(), "stdout"),
        (stderr.as_fd(), "stderr"),
    ] {
        let flags = match rustix::fs::fcntl_getfl(descriptor) {
            Ok(flags) => flags,
            Err(error) => {
                terminate_owned_child(&mut child)?;
                return Err(error).wrap_err_with(|| format!("failed to inspect child {label}"));
            }
        };
        if let Err(error) =
            rustix::fs::fcntl_setfl(descriptor, flags | rustix::fs::OFlags::NONBLOCK)
        {
            terminate_owned_child(&mut child)?;
            return Err(error)
                .wrap_err_with(|| format!("failed to make child {label} nonblocking"));
        }
    }
    let mut stdin = Some(stdin);
    let mut prefix_offset = 0_usize;
    let mut file_buffer = [0_u8; 64 * 1024];
    let mut file_range = 0..0;
    let mut stdin_complete = false;
    let mut stdin_aborted = false;
    let mut stdout_bytes = Vec::new();
    let mut stderr_bytes = Vec::new();
    let mut stdout_eof = false;
    let mut stderr_eof = false;
    let mut status = None;
    loop {
        if Instant::now() >= deadline {
            terminate_owned_child(&mut child)?;
            return Err(eyre!(
                "`{}` exceeded its absolute deadline while streaming or draining pipes",
                spec.program.display()
            ));
        }
        if !stdin_complete && !stdin_aborted {
            let writer = stdin.as_mut().expect("stdin exists until complete");
            let write_result = if prefix_offset < spec.stdin_prefix.len() {
                writer
                    .write(&spec.stdin_prefix[prefix_offset..])
                    .map(|written| {
                        prefix_offset += written;
                    })
            } else if let Some((file, expected, copied)) = source.as_mut() {
                if file_range.is_empty() && *copied < *expected {
                    let remaining =
                        usize::try_from((*expected - *copied).min(file_buffer.len() as u64))
                            .expect("bounded stream chunk");
                    let read = match file.read(&mut file_buffer[..remaining]) {
                        Ok(read) => read,
                        Err(error) => {
                            terminate_owned_child(&mut child)?;
                            return Err(error).wrap_err("failed to read pinned streamed input");
                        }
                    };
                    if read == 0 {
                        terminate_owned_child(&mut child)?;
                        return Err(eyre!(
                            "pinned streamed input ended before its declared length"
                        ));
                    }
                    file_range = 0..read;
                }
                if file_range.is_empty() {
                    stdin_complete = true;
                    Ok(())
                } else {
                    writer
                        .write(&file_buffer[file_range.clone()])
                        .map(|written| {
                            file_range.start += written;
                            *copied += u64::try_from(written).expect("write count fits u64");
                        })
                }
            } else {
                stdin_complete = true;
                Ok(())
            };
            match write_result {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(error) => {
                    terminate_owned_child(&mut child)?;
                    return Err(error).wrap_err("failed to stream bounded child stdin");
                }
            }
            if stdin_complete {
                drop(stdin.take());
            }
        }
        if let Err(error) = drain_nonblocking(&mut stdout, &mut stdout_bytes, &mut stdout_eof) {
            terminate_owned_child(&mut child)?;
            return Err(error).wrap_err("failed to drain bounded child stdout");
        }
        if let Err(error) = drain_nonblocking(&mut stderr, &mut stderr_bytes, &mut stderr_eof) {
            terminate_owned_child(&mut child)?;
            return Err(error).wrap_err("failed to drain bounded child stderr");
        }
        if status.is_none() {
            status = match child.try_wait() {
                Ok(value) => value,
                Err(error) => {
                    terminate_owned_child(&mut child)?;
                    return Err(error).wrap_err("failed to poll child process");
                }
            };
            if status.is_some() {
                stdin_aborted = !stdin_complete;
                drop(stdin.take());
            }
        }
        if status.is_some() && stdout_eof && stderr_eof {
            break;
        }
        let remaining = spec.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            continue;
        }
        std::thread::sleep(PROCESS_POLL_INTERVAL.min(remaining));
    }
    if !stdin_complete {
        return Err(eyre!(
            "child exited before consuming its exact framed stdin"
        ));
    }
    Ok(ProcessOutput {
        status: status.expect("loop exits only after child status"),
        stdout: stdout_bytes,
        stderr: stderr_bytes,
    })
}

fn drain_nonblocking(reader: &mut impl Read, output: &mut Vec<u8>, eof: &mut bool) -> Result<()> {
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => {
                *eof = true;
                return Ok(());
            }
            Ok(count) => {
                if output.len().saturating_add(count) > MAX_PROCESS_OUTPUT {
                    return Err(eyre!("child process output exceeded the V1 byte limit"));
                }
                output.extend_from_slice(&buffer[..count]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => return Err(error.into()),
        }
    }
}

fn require_success(output: ProcessOutput, label: &str) -> Result<Vec<u8>> {
    if output.status.success() {
        return Ok(output.stdout);
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(eyre!("{label} failed with {}: {stderr}", output.status))
}

#[derive(Debug)]
struct LocalArtifactClosure {
    files: BTreeMap<(String, String), StagedArtifact>,
}

#[derive(Debug)]
struct StagedArtifact {
    path: PathBuf,
    file: File,
    snapshot: super::FileSnapshot,
}

impl LocalArtifactClosure {
    fn snapshot(journal_dir: &Path, admitted: &AdmittedReset) -> Result<Self> {
        let staged_root = journal_dir.join("staged-artifacts-v1");
        ensure_private_directory(&staged_root)?;
        let root = staged_root.join(&admitted.inventory_sha256);
        ensure_private_directory(&root)?;
        let mut files = BTreeMap::new();
        let mut synced_hosts = BTreeSet::new();
        for pinned in &admitted.pinned_artifacts {
            let slug = pinned.slug.as_str();
            let host_root = root.join(slug);
            ensure_private_directory(&host_root)?;
            let nonce_root = host_root.join(&admitted.inventory.authorization_nonce);
            ensure_private_directory(&nonce_root)?;
            let name = staged_artifact_name(&pinned.role)?;
            let destination = nonce_root.join(name);
            snapshot_artifact(pinned, &destination)?;
            let (file, snapshot) = open_pinned_regular(&destination, "staged apply artifact")?;
            files.insert(
                (slug.to_owned(), pinned.role.clone()),
                StagedArtifact {
                    path: destination,
                    file,
                    snapshot,
                },
            );
            synced_hosts.insert(slug.to_owned());
        }
        for slug in synced_hosts {
            let host_root = root.join(&slug);
            sync_directory(&host_root.join(&admitted.inventory.authorization_nonce))?;
            sync_directory(&host_root)?;
        }
        sync_directory(&root)?;
        Ok(Self { files })
    }

    fn recover_cli(journal_dir: &Path, admitted: &AdmittedReset) -> Result<Self> {
        let validator = admitted
            .inventory
            .validators
            .first()
            .ok_or_else(|| eyre!("recovery inventory has no validator CLI source"))?;
        let expected = artifact(&validator.artifacts, "iroha_cli")?;
        let root = journal_dir
            .join("staged-artifacts-v1")
            .join(&admitted.inventory_sha256);
        validate_owner_private_dir(&root, "retained recovery artifact root")?;
        let path = root
            .join(&validator.slug)
            .join(&admitted.inventory.authorization_nonce)
            .join(staged_artifact_name("iroha_cli")?);
        validate_snapshot_file(&path, expected)?;
        let (file, snapshot) = open_pinned_regular(&path, "retained recovery CLI")?;
        let files = BTreeMap::from([(
            (validator.slug.clone(), "iroha_cli".to_owned()),
            StagedArtifact {
                path,
                file,
                snapshot,
            },
        )]);
        Ok(Self { files })
    }

    fn file(&self, slug: &str, role: &str) -> Result<&Path> {
        self.files
            .get(&(slug.to_owned(), role.to_owned()))
            .map(|artifact| artifact.path.as_path())
            .ok_or_else(|| eyre!("local staged closure is missing `{slug}:{role}`"))
    }

    fn stream_file(&self, slug: &str, role: &str) -> Result<File> {
        let artifact = self
            .files
            .get(&(slug.to_owned(), role.to_owned()))
            .ok_or_else(|| eyre!("local staged closure is missing `{slug}:{role}`"))?;
        ensure_pinned_unchanged(
            &artifact.path,
            "staged apply artifact",
            &artifact.file,
            &artifact.snapshot,
        )?;
        artifact
            .file
            .try_clone()
            .wrap_err("failed to duplicate pinned staged artifact descriptor")
    }

    fn revalidate_host(&self, slug: &str, artifacts: &[ArtifactV1]) -> Result<()> {
        for artifact in artifacts {
            let path = self.file(slug, &artifact.role)?;
            validate_snapshot_file(path, artifact)?;
        }
        Ok(())
    }
}

fn staged_artifact_name(role: &str) -> Result<String> {
    if role.is_empty()
        || role.len() > 64
        || !role
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(eyre!(
            "artifact role is not safe for the closed staging namespace"
        ));
    }
    Ok(if role == "iroha_cli" {
        "iroha".to_owned()
    } else {
        format!("artifact-{role}")
    })
}

fn ensure_private_directory(path: &Path) -> Result<()> {
    if !path.exists() {
        fs::create_dir(path).wrap_err_with(|| format!("failed to create `{}`", path.display()))?;
        #[cfg(unix)]
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .wrap_err_with(|| format!("failed to protect `{}`", path.display()))?;
        sync_directory(
            path.parent()
                .ok_or_else(|| eyre!("directory has no parent"))?,
        )?;
    }
    validate_owner_private_dir(path, "local public-reset artifact closure")
}

fn snapshot_artifact(pinned: &PinnedArtifact, destination: &Path) -> Result<()> {
    let artifact = &pinned.artifact;
    if pinned.input.snapshot.len != artifact.size {
        return Err(eyre!("apply artifact size drifted before staging"));
    }
    copy_pinned_private_noreplace(
        &pinned.input,
        destination,
        artifact.size,
        &artifact.sha256,
        if artifact.role == "iroha_cli" {
            0o500
        } else {
            0o400
        },
        "apply artifact",
    )
}

fn validate_snapshot_file(path: &Path, artifact: &ArtifactV1) -> Result<()> {
    validate_private_snapshot(
        path,
        artifact.size,
        &artifact.sha256,
        if artifact.role == "iroha_cli" {
            0o500
        } else {
            0o400
        },
        "staged apply artifact",
    )
}

fn copy_pinned_private_noreplace(
    input: &super::PinnedInput,
    destination: &Path,
    expected_size: u64,
    expected_sha256: &str,
    mode: u32,
    label: &str,
) -> Result<()> {
    let parent = destination
        .parent()
        .ok_or_else(|| eyre!("private snapshot destination has no parent"))?;
    validate_owner_private_dir(parent, "private snapshot parent")?;
    if destination.exists() {
        return validate_private_snapshot(destination, expected_size, expected_sha256, mode, label);
    }
    let name = destination
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| eyre!("private snapshot destination name is not UTF-8"))?;
    let staging = parent.join(format!(".{name}.next"));
    if staging.exists() {
        match validate_private_snapshot(
            &staging,
            expected_size,
            expected_sha256,
            mode,
            "staged private snapshot",
        ) {
            Ok(()) => {
                rename_private_noreplace(&staging, destination)?;
                return validate_private_snapshot(
                    destination,
                    expected_size,
                    expected_sha256,
                    mode,
                    label,
                );
            }
            Err(error) if discard_provably_partial_private(&staging, expected_size, mode)? => {
                eprintln!(
                    "discarded provably partial private reset snapshot (ephemeral): {error:#}"
                );
            }
            Err(error) => {
                return Err(error.wrap_err(
                    "stale private snapshot staging is neither complete nor provably partial",
                ));
            }
        }
    }
    let mut source = input
        .file
        .try_clone()
        .wrap_err_with(|| format!("failed to duplicate retained {label} descriptor"))?;
    source.rewind()?;
    #[cfg(unix)]
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&staging)?;
    #[cfg(not(unix))]
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging)?;
    let copied = std::io::copy(&mut source, &mut output)?;
    output.sync_all()?;
    ensure_pinned_unchanged(&input.path, label, &source, &input.snapshot)?;
    if copied != expected_size {
        return Err(eyre!("{label} changed length while snapshotting"));
    }
    validate_private_snapshot(
        &staging,
        expected_size,
        expected_sha256,
        mode,
        "staged private snapshot",
    )?;
    rename_private_noreplace(&staging, destination)?;
    validate_private_snapshot(destination, expected_size, expected_sha256, mode, label)
}

fn validate_private_snapshot(
    path: &Path,
    expected_size: u64,
    expected_sha256: &str,
    mode: u32,
    label: &str,
) -> Result<()> {
    let (mut file, snapshot) = open_pinned_regular(path, label)?;
    if snapshot.len != expected_size {
        return Err(eyre!("{label} length mismatch"));
    }
    let digest = hash_reader(&mut file)?;
    ensure_pinned_unchanged(path, label, &file, &snapshot)?;
    if digest != expected_sha256 {
        return Err(eyre!("{label} SHA-256 mismatch"));
    }
    #[cfg(unix)]
    {
        let metadata = file.metadata()?;
        if metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o7777 != mode
            || metadata.nlink() != 1
        {
            return Err(eyre!("{label} custody drifted"));
        }
    }
    Ok(())
}

fn discard_provably_partial_private(path: &Path, expected_size: u64, mode: u32) -> Result<bool> {
    let (file, snapshot) = open_pinned_regular(path, "partial private reset snapshot")?;
    #[cfg(unix)]
    {
        let metadata = file.metadata()?;
        if snapshot.len >= expected_size
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o7777 != mode
            || metadata.nlink() != 1
        {
            return Ok(false);
        }
    }
    #[cfg(not(unix))]
    if snapshot.len >= expected_size {
        return Ok(false);
    }
    ensure_pinned_unchanged(path, "partial private reset snapshot", &file, &snapshot)?;
    let parent = path
        .parent()
        .ok_or_else(|| eyre!("partial private reset snapshot has no parent"))?;
    let directory = File::open(parent)?;
    rustix::fs::unlinkat(
        &directory,
        path.file_name().expect("partial snapshot has name"),
        rustix::fs::AtFlags::empty(),
    )?;
    directory.sync_all()?;
    Ok(true)
}

fn rename_private_noreplace(source: &Path, destination: &Path) -> Result<()> {
    let parent = source
        .parent()
        .filter(|parent| Some(*parent) == destination.parent())
        .ok_or_else(|| eyre!("private snapshot rename must stay in one directory"))?;
    validate_owner_private_dir(parent, "private snapshot parent")?;
    let directory = File::open(parent)?;
    rustix::fs::renameat_with(
        &directory,
        source
            .file_name()
            .expect("private snapshot staging has name"),
        &directory,
        destination
            .file_name()
            .expect("private snapshot destination has name"),
        rustix::fs::RenameFlags::NOREPLACE,
    )?;
    directory.sync_all()?;
    Ok(())
}

fn hash_reader(reader: &mut impl Read) -> Result<String> {
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer).wrap_err("failed to hash bytes")?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(hex::encode(digest.finalize()))
}

fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .wrap_err_with(|| format!("failed to fsync directory `{}`", path.display()))
}

#[derive(Clone, Debug)]
pub(super) struct RuntimeCanaryInputs {
    pub(super) client_config: PathBuf,
    pub(super) validator_client_configs: Vec<PathBuf>,
    pub(super) onboarding_token: PathBuf,
    pub(super) inrou_stage_dir: PathBuf,
    pub(super) fee_args: Vec<OsString>,
}

struct RuntimeCustody {
    client_config: super::PinnedInput,
    validator_client_configs: Vec<super::PinnedInput>,
    onboarding_token: Option<super::PinnedInput>,
    inrou_stage_dir: PathBuf,
    snapshot_stage_files: Vec<(String, super::PinnedInput)>,
    stage_identity: crate::soracloud::TairaInrouStageIdentity,
    fee_args: Vec<OsString>,
}

impl RuntimeCustody {
    fn admit(
        inputs: RuntimeCanaryInputs,
        admitted: &AdmittedReset,
        journal_dir: &Path,
    ) -> Result<Self> {
        if inputs.validator_client_configs.len() != 4 {
            return Err(eyre!(
                "apply requires exactly four ordered validator client configs"
            ));
        }
        validate_fee_args(&inputs.fee_args)?;
        let client_config =
            pin_owner_private_file(&inputs.client_config, "Taira runtime client config")?;
        let onboarding_token =
            pin_owner_private_file(&inputs.onboarding_token, "Taira onboarding token")?;
        let validator_client_configs = inputs
            .validator_client_configs
            .iter()
            .enumerate()
            .map(|(index, path)| {
                pin_owner_private_file(path, &format!("validator {} client config", index + 1))
            })
            .collect::<Result<Vec<_>>>()?;
        let client_hash = hash_pinned_input(&client_config, "Taira runtime client config", None)?;
        let token_hash = hash_pinned_input(&onboarding_token, "Taira onboarding token", None)?;
        let validator_hash = validator_config_closure_sha256(&validator_client_configs, None)?;
        validate_validator_client_semantics(&validator_client_configs, admitted)?;
        let (stage_hash, stage_bytes, stage_files, fixed) =
            pin_stage_tree(&inputs.inrou_stage_dir, None)?;
        let claims = &admitted.authorization.claims;
        if client_hash != claims.runtime_client_config_sha256
            || token_hash != claims.onboarding_token_sha256
            || validator_hash != claims.validator_client_configs_sha256
            || stage_hash != claims.inrou_stage_tree_sha256
            || stage_hash != admitted.inventory.inrou_canary.stage_tree_sha256
            || stage_bytes != admitted.inventory.inrou_canary.stage_bytes
        {
            return Err(eyre!(
                "runtime signing/stage closure is not authorization-bound"
            ));
        }
        for (path, expected) in [
            (
                "receipt.json",
                &admitted.inventory.inrou_canary.receipt_sha256,
            ),
            (
                "container.json",
                &admitted.inventory.inrou_canary.container_sha256,
            ),
            (
                "service.json",
                &admitted.inventory.inrou_canary.service_sha256,
            ),
            (
                "payloads/bundle.bin",
                &admitted.inventory.inrou_canary.bundle_payload_sha256,
            ),
            (
                "manifests/bundle.to",
                &admitted.inventory.inrou_canary.bundle_manifest_sha256,
            ),
            (
                "manifests/aarch64.to",
                &admitted.inventory.inrou_canary.guest_manifest_sha256,
            ),
        ] {
            if fixed.get(path) != Some(expected) {
                return Err(eyre!("retained Inrou stage fixed-file hash mismatch"));
            }
        }
        let runtime_config =
            load_client_config_from_pinned(&client_config, "Taira runtime client config")?;
        let expected_public_root = format!("{}/", admitted.inventory.inrou_canary.public_root);
        if runtime_config.torii_api_url.as_str() != expected_public_root
            || runtime_config.account.to_string()
                != admitted.inventory.canary_onboarding_request.account_id
        {
            return Err(eyre!(
                "runtime canary config does not target the exact signed public Taira canary"
            ));
        }
        let retained_stage_dir =
            snapshot_stage_tree(journal_dir, &admitted.authorization_sha256, &stage_files)?;
        let (retained_hash, retained_bytes, snapshot_stage_files, _) =
            pin_stage_tree(&retained_stage_dir, None)?;
        if retained_hash != stage_hash || retained_bytes != stage_bytes {
            return Err(eyre!("snapshotted Inrou stage closure hash drifted"));
        }
        let stage_identity = crate::soracloud::load_taira_inrou_stage_identity(
            &runtime_config,
            &retained_stage_dir,
            crate::taira::InrouCanaryMode::Deploy,
        )?;
        let (rechecked_hash, rechecked_bytes) =
            revalidate_stage_files(&inputs.inrou_stage_dir, &stage_files, None)?;
        if rechecked_hash != stage_hash
            || rechecked_bytes != stage_bytes
            || !stage_identity_matches_inventory(&stage_identity, &admitted.inventory.inrou_canary)
        {
            return Err(eyre!(
                "fully validated Inrou stage identity differs from inventory"
            ));
        }
        Ok(Self {
            client_config,
            validator_client_configs,
            onboarding_token: Some(onboarding_token),
            inrou_stage_dir: retained_stage_dir,
            snapshot_stage_files,
            stage_identity,
            fee_args: inputs.fee_args,
        })
    }

    fn revalidate(
        &self,
        admitted: &AdmittedReset,
        deadline: Instant,
        require_onboarding_token: bool,
    ) -> Result<()> {
        ensure_local_deadline(Some(deadline))?;
        revalidate_pinned(&self.client_config, "Taira runtime client config")?;
        if require_onboarding_token {
            revalidate_pinned(
                self.onboarding_token
                    .as_ref()
                    .ok_or_else(|| eyre!("recovery custody has no onboarding token"))?,
                "Taira onboarding token",
            )?;
        }
        for input in &self.validator_client_configs {
            revalidate_pinned(input, "validator client config")?;
        }
        for (_, input) in &self.snapshot_stage_files {
            revalidate_pinned(input, "snapshotted Inrou stage file")?;
        }
        validate_owner_private_dir(&self.inrou_stage_dir, "snapshotted Inrou stage")?;
        if !stage_identity_matches_inventory(&self.stage_identity, &admitted.inventory.inrou_canary)
        {
            return Err(eyre!("runtime signing/stage custody drifted"));
        }
        ensure_local_deadline(Some(deadline))?;
        Ok(())
    }

    fn recover(
        client_config_path: PathBuf,
        validator_config_paths: Vec<PathBuf>,
        admitted: &AdmittedReset,
        journal_dir: &Path,
    ) -> Result<Self> {
        let client_config =
            pin_owner_private_file(&client_config_path, "Taira recovery client config")?;
        let client_hash = hash_pinned_input(&client_config, "Taira recovery client config", None)?;
        if client_hash != admitted.authorization.claims.runtime_client_config_sha256 {
            return Err(eyre!(
                "recovery client config is not bound by the signed authorization"
            ));
        }
        let runtime_config =
            load_client_config_from_pinned(&client_config, "Taira recovery client config")?;
        let expected_public_root = format!("{}/", admitted.inventory.inrou_canary.public_root);
        if runtime_config.torii_api_url.as_str() != expected_public_root
            || runtime_config.account.to_string()
                != admitted.inventory.canary_onboarding_request.account_id
        {
            return Err(eyre!(
                "recovery client config does not target the exact signed public Taira canary"
            ));
        }
        if !matches!(validator_config_paths.len(), 0 | 4) {
            return Err(eyre!(
                "recovery requires either zero or exactly four validator client configs"
            ));
        }
        let validator_client_configs = validator_config_paths
            .iter()
            .enumerate()
            .map(|(index, path)| {
                pin_owner_private_file(
                    path,
                    &format!("recovery validator {} client config", index + 1),
                )
            })
            .collect::<Result<Vec<_>>>()?;
        if !validator_client_configs.is_empty() {
            let validator_hash = validator_config_closure_sha256(&validator_client_configs, None)?;
            if validator_hash
                != admitted
                    .authorization
                    .claims
                    .validator_client_configs_sha256
            {
                return Err(eyre!(
                    "recovery validator client configs are not authorization-bound"
                ));
            }
            validate_validator_client_semantics(&validator_client_configs, admitted)?;
        }
        let inrou_stage_dir = journal_dir
            .join("runtime-stage-v1")
            .join(&admitted.authorization_sha256);
        validate_owner_private_dir(&inrou_stage_dir, "retained recovery Inrou stage")?;
        let (stage_hash, stage_bytes, snapshot_stage_files, fixed) =
            pin_stage_tree(&inrou_stage_dir, None)?;
        if stage_hash != admitted.authorization.claims.inrou_stage_tree_sha256
            || stage_hash != admitted.inventory.inrou_canary.stage_tree_sha256
            || stage_bytes != admitted.inventory.inrou_canary.stage_bytes
        {
            return Err(eyre!(
                "retained recovery Inrou stage is not authorization-bound"
            ));
        }
        for (path, expected) in [
            (
                "receipt.json",
                &admitted.inventory.inrou_canary.receipt_sha256,
            ),
            (
                "container.json",
                &admitted.inventory.inrou_canary.container_sha256,
            ),
            (
                "service.json",
                &admitted.inventory.inrou_canary.service_sha256,
            ),
            (
                "payloads/bundle.bin",
                &admitted.inventory.inrou_canary.bundle_payload_sha256,
            ),
            (
                "manifests/bundle.to",
                &admitted.inventory.inrou_canary.bundle_manifest_sha256,
            ),
            (
                "manifests/aarch64.to",
                &admitted.inventory.inrou_canary.guest_manifest_sha256,
            ),
        ] {
            if fixed.get(path) != Some(expected) {
                return Err(eyre!(
                    "retained recovery Inrou stage fixed-file hash mismatch"
                ));
            }
        }
        let stage_identity = crate::soracloud::load_taira_inrou_stage_identity(
            &runtime_config,
            &inrou_stage_dir,
            crate::taira::InrouCanaryMode::Deploy,
        )?;
        if !stage_identity_matches_inventory(&stage_identity, &admitted.inventory.inrou_canary) {
            return Err(eyre!(
                "retained recovery Inrou stage identity differs from inventory"
            ));
        }
        let fee_args = super::PublicResetApply::fee_args(&admitted.inventory)?;
        Ok(Self {
            client_config,
            validator_client_configs,
            onboarding_token: None,
            inrou_stage_dir,
            snapshot_stage_files,
            stage_identity,
            fee_args,
        })
    }
}

fn stage_identity_matches_inventory(
    identity: &crate::soracloud::TairaInrouStageIdentity,
    expected: &super::InrouCanaryV1,
) -> bool {
    identity.service_name == expected.service_name
        && identity.service_version == expected.service_version
        && identity.route_host == expected.route_host
        && identity.route_path_prefix == expected.route_path_prefix
        && identity.healthcheck_path == expected.healthcheck_path
        && identity.stage_mode == "deploy"
        && identity.bundle_hash == expected.bundle_hash
        && identity.bundle_content_cid == expected.bundle_content_cid
        && identity.bundle_manifest_digest_hex == expected.bundle_manifest_digest_hex
        && identity.guest_content_cid == expected.guest_content_cid
        && identity.guest_manifest_digest_hex == expected.guest_manifest_digest_hex
        && identity.container_manifest_hash == expected.container_manifest_hash
        && identity.service_manifest_hash == expected.service_manifest_hash
}

fn validate_validator_client_semantics(
    inputs: &[super::PinnedInput],
    admitted: &AdmittedReset,
) -> Result<()> {
    if inputs.len() != admitted.inventory.validator_clients.len() {
        return Err(eyre!("validator client semantic closure length drifted"));
    }
    for (input, expected) in inputs.iter().zip(&admitted.inventory.validator_clients) {
        revalidate_pinned(input, "validator client config")?;
        let config = load_client_config_from_pinned(input, "validator client config")?;
        let expected_account =
            iroha::data_model::account::AccountId::parse_encoded(&expected.account_id)
                .wrap_err("signed validator client account is invalid")?;
        if config.torii_api_url.as_str() != expected.torii_origin
            || config.account != expected_account
        {
            return Err(eyre!(
                "validator client config does not match its signed Torii origin/account"
            ));
        }
    }
    Ok(())
}

fn load_client_config_from_pinned(input: &super::PinnedInput, label: &str) -> Result<ClientConfig> {
    revalidate_pinned(input, label)?;
    let retained = input
        .file
        .try_clone()
        .wrap_err_with(|| format!("failed to duplicate {label} descriptor"))?;
    #[cfg(target_os = "linux")]
    let descriptor_path = PathBuf::from(format!("/proc/self/fd/{}", retained.as_raw_fd()));
    #[cfg(all(unix, not(target_os = "linux")))]
    let descriptor_path = PathBuf::from(format!("/dev/fd/{}", retained.as_raw_fd()));
    #[cfg(not(unix))]
    return Err(eyre!(
        "client config semantic admission requires Unix descriptors"
    ));
    let config = ClientConfig::load(LoadPath::Explicit(descriptor_path))
        .map_err(|_| eyre!("{label} failed strict semantic loading"))?;
    revalidate_pinned(input, label)?;
    Ok(config)
}

fn hash_pinned_input(
    input: &super::PinnedInput,
    label: &str,
    deadline: Option<Instant>,
) -> Result<String> {
    revalidate_pinned(input, label)?;
    let mut file = input
        .file
        .try_clone()
        .wrap_err("failed to duplicate pinned runtime input")?;
    file.rewind()
        .wrap_err("failed to rewind pinned runtime input")?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        ensure_local_deadline(deadline)?;
        let count = file
            .read(&mut buffer)
            .wrap_err("failed to hash runtime input")?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    revalidate_pinned(input, label)?;
    ensure_local_deadline(deadline)?;
    Ok(hex::encode(digest.finalize()))
}

fn ensure_local_deadline(deadline: Option<Instant>) -> Result<()> {
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        return Err(eyre!(
            "local reset action deadline elapsed during custody revalidation"
        ));
    }
    Ok(())
}

fn inherited_input_path(input: &super::PinnedInput, label: &str) -> Result<(PathBuf, File)> {
    revalidate_pinned(input, label)?;
    let file = input
        .file
        .try_clone()
        .wrap_err_with(|| format!("failed to duplicate retained {label} descriptor"))?;
    Ok((inherited_file_path(&file)?, file))
}

#[cfg(target_os = "linux")]
fn inherited_file_path(file: &File) -> Result<PathBuf> {
    Ok(PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd())))
}

#[cfg(not(target_os = "linux"))]
fn inherited_file_path(_file: &File) -> Result<PathBuf> {
    Err(eyre!(
        "public-reset apply requires Linux descriptor-backed process inputs"
    ))
}

fn validator_config_closure_sha256(
    inputs: &[super::PinnedInput],
    deadline: Option<Instant>,
) -> Result<String> {
    let mut digest = Sha256::new();
    digest.update(b"iroha:taira:public-reset:validator-client-configs:v1\0");
    for (slug, input) in super::VALIDATOR_SLUGS.iter().zip(inputs) {
        update_frame(&mut digest, slug.as_bytes());
        update_frame(
            &mut digest,
            hash_pinned_input(input, "validator client config", deadline)?.as_bytes(),
        );
    }
    Ok(hex::encode(digest.finalize()))
}

fn pin_stage_tree(
    root: &Path,
    deadline: Option<Instant>,
) -> Result<(
    String,
    u64,
    Vec<(String, super::PinnedInput)>,
    BTreeMap<String, String>,
)> {
    validate_owner_private_dir(root, "retained Inrou stage")?;
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        ensure_local_deadline(deadline)?;
        let mut entries = fs::read_dir(&directory)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries.into_iter().rev() {
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() {
                return Err(eyre!("retained Inrou stage contains a symlink"));
            }
            if metadata.is_dir() {
                validate_owner_private_dir(&path, "retained Inrou stage directory")?;
                pending.push(path);
            } else if metadata.is_file() {
                let relative = path
                    .strip_prefix(root)?
                    .to_str()
                    .ok_or_else(|| eyre!("Inrou stage paths must be UTF-8"))?
                    .replace(std::path::MAIN_SEPARATOR, "/");
                if relative.is_empty()
                    || relative.contains("..")
                    || !relative.bytes().all(|byte| {
                        byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'-' | b'_')
                    })
                {
                    return Err(eyre!("Inrou stage path escaped its closed grammar"));
                }
                files.push((
                    relative,
                    pin_owner_private_file(&path, "retained Inrou stage file")?,
                ));
            } else {
                return Err(eyre!("retained Inrou stage contains a special file"));
            }
        }
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let (hash, bytes) = revalidate_stage_files(root, &files, deadline)?;
    let fixed = files
        .iter()
        .map(|(path, input)| {
            Ok((
                path.clone(),
                hash_pinned_input(input, "Inrou stage file", deadline)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    Ok((hash, bytes, files, fixed))
}

fn revalidate_stage_files(
    root: &Path,
    files: &[(String, super::PinnedInput)],
    deadline: Option<Instant>,
) -> Result<(String, u64)> {
    validate_owner_private_dir(root, "retained Inrou stage")?;
    let mut digest = Sha256::new();
    digest.update(b"iroha:taira:public-reset:inrou-stage-tree:v1\0");
    let mut total = 0_u64;
    for (relative, input) in files {
        ensure_local_deadline(deadline)?;
        if input.path != root.join(relative) {
            return Err(eyre!("retained Inrou stage path binding drifted"));
        }
        let hash = hash_pinned_input(input, "retained Inrou stage file", deadline)?;
        total = total
            .checked_add(input.snapshot.len)
            .filter(|value| *value <= super::MAX_INROU_STAGE_BYTES)
            .ok_or_else(|| eyre!("retained Inrou stage exceeds its byte bound"))?;
        update_frame(&mut digest, relative.as_bytes());
        update_frame(&mut digest, &input.snapshot.len.to_be_bytes());
        update_frame(&mut digest, hash.as_bytes());
    }
    Ok((hex::encode(digest.finalize()), total))
}

fn snapshot_stage_tree(
    journal_dir: &Path,
    authorization_sha256: &str,
    files: &[(String, super::PinnedInput)],
) -> Result<PathBuf> {
    let parent = journal_dir.join("runtime-stage-v1");
    ensure_private_directory(&parent)?;
    let root = parent.join(authorization_sha256);
    ensure_private_directory(&root)?;
    for (relative, input) in files {
        let destination = root.join(relative);
        let directory = destination
            .parent()
            .ok_or_else(|| eyre!("retained stage destination has no parent"))?;
        let relative_directory = directory
            .strip_prefix(&root)
            .wrap_err("retained stage directory escaped its root")?;
        let mut cursor = root.clone();
        for component in relative_directory.components() {
            cursor.push(component.as_os_str());
            ensure_private_directory(&cursor)?;
        }
        let expected_hash = hash_pinned_input(input, "retained Inrou stage file", None)?;
        copy_pinned_private_noreplace(
            input,
            &destination,
            input.snapshot.len,
            &expected_hash,
            0o400,
            "snapshotted Inrou stage",
        )?;
    }
    sync_directory(&root)?;
    Ok(root)
}

fn update_frame(digest: &mut Sha256, value: &[u8]) {
    digest.update(u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    digest.update(value);
}

pub(super) struct OpenSshTransport<'a, R = RealProcessRunner> {
    admitted: &'a AdmittedReset,
    runtime: RuntimeCustody,
    closure: LocalArtifactClosure,
    local_receipt_root: PathBuf,
    runner: R,
}

#[derive(Clone, Copy)]
struct MutationReservationIdentity<'a> {
    operation: &'a str,
    kind: &'a str,
    phase: &'a str,
    idempotency_key: &'a str,
    prepared: Option<&'a [u8]>,
    prepared_sha256: &'a str,
    transaction_hash: &'a str,
    evidence: Option<&'a [u8]>,
}

struct RetainedPreparedMutation {
    state: String,
    bytes: Vec<u8>,
    sha256: String,
    transaction_hash: String,
}

enum PreparedMutationOutcome {
    Applied {
        value: norito::json::Value,
        evidence: Vec<u8>,
    },
    Pending,
    Rejected(String),
}

/// Minimal read-only transport for reconciling a durably submitted local
/// ledger mutation after the forward artifact and signing-token closure has
/// been released.
pub(super) struct RecoverySshTransport<'a> {
    inner: OpenSshTransport<'a, RealProcessRunner>,
}

impl<'a> OpenSshTransport<'a, RealProcessRunner> {
    pub(super) fn new(
        admitted: &'a AdmittedReset,
        journal_dir: &Path,
        ssh_identity: PathBuf,
        known_hosts: PathBuf,
        runtime: RuntimeCanaryInputs,
    ) -> Result<Self> {
        if ssh_identity != admitted.ssh_identity.path || known_hosts != admitted.known_hosts.path {
            return Err(eyre!(
                "apply transport inputs differ from admitted pinned SSH inputs"
            ));
        }
        revalidate_pinned(&admitted.ssh_identity, "OpenSSH identity")?;
        revalidate_pinned(&admitted.known_hosts, "OpenSSH known-hosts")?;
        validate_first_release_physical_host(&admitted.inventory)?;
        validate_shared_cli(admitted)?;
        let closure = LocalArtifactClosure::snapshot(journal_dir, admitted)?;
        let runtime = RuntimeCustody::admit(runtime, admitted, journal_dir)?;
        let local_receipt_parent = journal_dir.join("local-receipts-v1");
        ensure_private_directory(&local_receipt_parent)?;
        let local_receipt_root = local_receipt_parent.join(&admitted.authorization_sha256);
        ensure_private_directory(&local_receipt_root)?;
        Ok(Self {
            admitted,
            runtime,
            closure,
            local_receipt_root,
            runner: RealProcessRunner,
        })
    }
}

impl<'a> RecoverySshTransport<'a> {
    pub(super) fn new(
        admitted: &'a AdmittedReset,
        journal_dir: &Path,
        runtime_client_config: PathBuf,
        validator_client_configs: Vec<PathBuf>,
    ) -> Result<Self> {
        validate_owner_private_dir(journal_dir, "public-reset journal directory")?;
        let runtime = RuntimeCustody::recover(
            runtime_client_config,
            validator_client_configs,
            admitted,
            journal_dir,
        )?;
        let closure = LocalArtifactClosure::recover_cli(journal_dir, admitted)?;
        let local_receipt_parent = journal_dir.join("local-receipts-v1");
        ensure_private_directory(&local_receipt_parent)?;
        let local_receipt_root = local_receipt_parent.join(&admitted.authorization_sha256);
        ensure_private_directory(&local_receipt_root)?;
        Ok(Self {
            inner: OpenSshTransport {
                admitted,
                runtime,
                closure,
                local_receipt_root,
                runner: RealProcessRunner,
            },
        })
    }
}

pub(super) struct RollbackSshTransport<'a> {
    admitted: &'a AdmittedReset,
    runner: RealProcessRunner,
}

/// Minimal transport for resuming the terminal seal/cleanup tail after the
/// forward artifact and runtime-signing closure has been released.
pub(super) struct SealCleanupSshTransport<'a> {
    admitted: &'a AdmittedReset,
    runner: RealProcessRunner,
}

impl<'a> RollbackSshTransport<'a> {
    pub(super) fn new(admitted: &'a AdmittedReset, journal_dir: &Path) -> Result<Self> {
        validate_owner_private_dir(journal_dir, "public-reset journal directory")?;
        revalidate_pinned(&admitted.ssh_identity, "OpenSSH identity")?;
        revalidate_pinned(&admitted.known_hosts, "OpenSSH known-hosts")?;
        validate_first_release_physical_host(&admitted.inventory)?;
        Ok(Self {
            admitted,
            runner: RealProcessRunner,
        })
    }

    fn rollback_target(
        &mut self,
        slug: &str,
        endpoint: &EndpointV1,
        timeout_secs: u64,
    ) -> Result<()> {
        dispatch_custodied_host_action(
            self.admitted,
            &mut self.runner,
            slug,
            endpoint,
            HostAction::Rollback,
            timeout_secs,
        )
    }
}

impl<'a> SealCleanupSshTransport<'a> {
    pub(super) fn new(admitted: &'a AdmittedReset, journal_dir: &Path) -> Result<Self> {
        validate_owner_private_dir(journal_dir, "public-reset journal directory")?;
        revalidate_pinned(&admitted.ssh_identity, "OpenSSH identity")?;
        revalidate_pinned(&admitted.known_hosts, "OpenSSH known-hosts")?;
        validate_first_release_physical_host(&admitted.inventory)?;
        Ok(Self {
            admitted,
            runner: RealProcessRunner,
        })
    }

    fn dispatch_target(
        &mut self,
        slug: &str,
        endpoint: &EndpointV1,
        action: HostAction,
        timeout_secs: u64,
    ) -> Result<()> {
        if !matches!(action, HostAction::Seal | HostAction::Cleanup) {
            return Err(eyre!(
                "seal/cleanup-only transport rejects `{}`",
                action.label()
            ));
        }
        dispatch_custodied_host_action(
            self.admitted,
            &mut self.runner,
            slug,
            endpoint,
            action,
            timeout_secs,
        )
    }
}

fn dispatch_custodied_host_action<R: ProcessRunner>(
    admitted: &AdmittedReset,
    runner: &mut R,
    slug: &str,
    endpoint: &EndpointV1,
    action: HostAction,
    timeout_secs: u64,
) -> Result<()> {
    if !matches!(
        action,
        HostAction::Rollback | HostAction::Seal | HostAction::Cleanup
    ) {
        return Err(eyre!("minimal host dispatch rejects forward action"));
    }
    let timeout_ms = timeout_secs
        .checked_mul(1_000)
        .ok_or_else(|| eyre!("minimal host action timeout overflow"))?;
    let action_deadline_unix_ms = now_unix_ms()?
        .checked_add(timeout_ms)
        .ok_or_else(|| eyre!("minimal host action deadline overflow"))?;
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(timeout_secs))
        .ok_or_else(|| eyre!("minimal host monotonic deadline overflow"))?;
    revalidate_pinned(&admitted.ssh_identity, "OpenSSH identity")?;
    revalidate_pinned(&admitted.known_hosts, "OpenSSH known-hosts")?;
    let request = HostRequestV1 {
        schema: HOST_REQUEST_SCHEMA_V1.to_owned(),
        action: action.label().to_owned(),
        recovery_only: false,
        host_slug: slug.to_owned(),
        inventory_base64: BASE64.encode(&admitted.inventory_bytes),
        authorization_base64: BASE64.encode(&admitted.authorization_bytes),
        authorization_semantic_sha256: admitted.authorization_sha256.clone(),
        trusted_key_base64: BASE64.encode(&admitted.trusted_key_bytes),
        trusted_key_sha256: sha256_hex(&admitted.trusted_key_bytes),
        action_deadline_unix_ms,
        artifact_role: String::new(),
        artifact_sha256: String::new(),
        artifact_size: 0,
        artifact_mode: 0,
        mutation_kind: String::new(),
        mutation_phase: String::new(),
        mutation_idempotency_key: String::new(),
        mutation_operation: String::new(),
        mutation_prepared_base64: String::new(),
        mutation_prepared_sha256: String::new(),
        mutation_transaction_hash: String::new(),
        mutation_evidence_base64: String::new(),
    };
    let request_bytes = json::to_json(&request)?.into_bytes();
    let mut frame = format!("{:08x}\n", request_bytes.len()).into_bytes();
    frame.extend_from_slice(&request_bytes);
    let remote_command = format!("{FIXED_DISPATCHER} {HOST_DISPATCH_SUFFIX}");
    validate_remote_command(&remote_command)?;
    let (identity_path, identity_file) =
        inherited_input_path(&admitted.ssh_identity, "OpenSSH identity")?;
    let (known_hosts_path, known_hosts_file) =
        inherited_input_path(&admitted.known_hosts, "OpenSSH known-hosts")?;
    let mut args = ssh_common_args(endpoint, &identity_path, &known_hosts_path, timeout_secs);
    args.push(OsString::from("--"));
    args.push(OsString::from(format!(
        "{}@{}",
        endpoint.user, endpoint.hostname
    )));
    args.push(OsString::from(remote_command));
    let output = require_success(
        runner.run(&ProcessSpec {
            program: PathBuf::from(SSH),
            args,
            stdin_prefix: frame,
            stdin_file: None,
            inherited_files: vec![identity_file, known_hosts_file],
            deadline,
        })?,
        "pinned SSH minimal terminal dispatch",
    )?;
    let receipt: HostReceiptV1 =
        json::from_slice(&output).wrap_err("minimal host dispatch returned no exact receipt")?;
    verify_remote_receipt(&request, &receipt)?;
    Ok(())
}

impl ResetTransport for RollbackSshTransport<'_> {
    fn validator_step(
        &mut self,
        _inventory: &InventoryV1,
        _validator: &ValidatorV1,
        _step: ExecutionStep,
        _timeout_secs: u64,
    ) -> Result<()> {
        Err(eyre!(
            "rollback-only transport rejects forward validator work"
        ))
    }

    fn cohort_step(
        &mut self,
        _inventory: &InventoryV1,
        _step: ExecutionStep,
        _timeout_secs: u64,
    ) -> Result<()> {
        Err(eyre!("rollback-only transport rejects forward cohort work"))
    }

    fn edge_step(
        &mut self,
        _inventory: &InventoryV1,
        _step: ExecutionStep,
        _timeout_secs: u64,
    ) -> Result<()> {
        Err(eyre!("rollback-only transport rejects forward edge work"))
    }

    fn rollback_validator(
        &mut self,
        _inventory: &InventoryV1,
        validator: &ValidatorV1,
        timeout_secs: u64,
    ) -> Result<()> {
        self.rollback_target(&validator.slug, &validator.endpoint, timeout_secs)
    }

    fn rollback_edge(&mut self, inventory: &InventoryV1, timeout_secs: u64) -> Result<()> {
        self.rollback_target(&inventory.edge.slug, &inventory.edge.endpoint, timeout_secs)
    }
}

impl ResetTransport for SealCleanupSshTransport<'_> {
    fn validator_step(
        &mut self,
        _inventory: &InventoryV1,
        _validator: &ValidatorV1,
        _step: ExecutionStep,
        _timeout_secs: u64,
    ) -> Result<()> {
        Err(eyre!(
            "seal/cleanup-only transport rejects validator forward work"
        ))
    }

    fn cohort_step(
        &mut self,
        _inventory: &InventoryV1,
        step: ExecutionStep,
        timeout_secs: u64,
    ) -> Result<()> {
        let action = match step {
            ExecutionStep::Seal => HostAction::Seal,
            ExecutionStep::Cleanup => HostAction::Cleanup,
            _ => {
                return Err(eyre!(
                    "seal/cleanup-only transport rejects cohort step `{}`",
                    step.label()
                ));
            }
        };
        for validator in &self.admitted.inventory.validators {
            self.dispatch_target(&validator.slug, &validator.endpoint, action, timeout_secs)?;
        }
        self.dispatch_target(
            &self.admitted.inventory.edge.slug,
            &self.admitted.inventory.edge.endpoint,
            action,
            timeout_secs,
        )
    }

    fn edge_step(
        &mut self,
        _inventory: &InventoryV1,
        _step: ExecutionStep,
        _timeout_secs: u64,
    ) -> Result<()> {
        Err(eyre!("seal/cleanup-only transport rejects edge work"))
    }

    fn rollback_validator(
        &mut self,
        _inventory: &InventoryV1,
        _validator: &ValidatorV1,
        _timeout_secs: u64,
    ) -> Result<()> {
        Err(eyre!("seal/cleanup-only transport rejects rollback"))
    }

    fn rollback_edge(&mut self, _inventory: &InventoryV1, _timeout_secs: u64) -> Result<()> {
        Err(eyre!("seal/cleanup-only transport rejects rollback"))
    }
}

impl ResetTransport for RecoverySshTransport<'_> {
    fn recovery_intent(
        &self,
        inventory: &InventoryV1,
        step: ExecutionStep,
    ) -> Result<Option<RecoveryIntentV1>> {
        self.inner.recovery_intent(inventory, step)
    }

    fn recover_step(
        &mut self,
        inventory: &InventoryV1,
        step: ExecutionStep,
        timeout_secs: u64,
        intent: &RecoveryIntentV1,
        progress: &mut dyn RecoveryProgress,
    ) -> Result<RecoveryOutcome> {
        self.inner
            .recover_step(inventory, step, timeout_secs, intent, progress)
    }

    fn run_recoverable_step(
        &mut self,
        _inventory: &InventoryV1,
        _step: ExecutionStep,
        _timeout_secs: u64,
        _intent: &RecoveryIntentV1,
        _progress: &mut dyn RecoveryProgress,
    ) -> Result<()> {
        Err(eyre!("recovery-only transport rejects new mutation work"))
    }

    fn validator_step(
        &mut self,
        _inventory: &InventoryV1,
        _validator: &ValidatorV1,
        _step: ExecutionStep,
        _timeout_secs: u64,
    ) -> Result<()> {
        Err(eyre!("recovery-only transport rejects validator work"))
    }

    fn cohort_step(
        &mut self,
        _inventory: &InventoryV1,
        _step: ExecutionStep,
        _timeout_secs: u64,
    ) -> Result<()> {
        Err(eyre!("recovery-only transport rejects cohort work"))
    }

    fn edge_step(
        &mut self,
        _inventory: &InventoryV1,
        _step: ExecutionStep,
        _timeout_secs: u64,
    ) -> Result<()> {
        Err(eyre!("recovery-only transport rejects edge work"))
    }

    fn rollback_validator(
        &mut self,
        _inventory: &InventoryV1,
        _validator: &ValidatorV1,
        _timeout_secs: u64,
    ) -> Result<()> {
        Err(eyre!("recovery-only transport rejects rollback"))
    }

    fn rollback_edge(&mut self, _inventory: &InventoryV1, _timeout_secs: u64) -> Result<()> {
        Err(eyre!("recovery-only transport rejects rollback"))
    }
}

impl<R: ProcessRunner> OpenSshTransport<'_, R> {
    fn bootstrap_and_dispatch_validator(
        &mut self,
        validator: &ValidatorV1,
        action: HostAction,
        timeout_secs: u64,
    ) -> Result<HostReceiptV1> {
        if action == HostAction::Stage {
            self.stream_host_artifacts(
                &validator.slug,
                &validator.endpoint,
                &validator.service_root,
                &validator.artifacts,
                timeout_secs,
            )?;
        }
        self.dispatch(
            &validator.slug,
            &validator.endpoint,
            &validator.service_root,
            action,
            None,
            false,
            None,
            timeout_secs,
        )
    }

    fn bootstrap_and_dispatch_edge(
        &mut self,
        edge: &EdgeV1,
        action: HostAction,
        timeout_secs: u64,
    ) -> Result<HostReceiptV1> {
        if action == HostAction::EdgeStage {
            self.stream_host_artifacts(
                &edge.slug,
                &edge.endpoint,
                &edge.service_root,
                &edge.artifacts,
                timeout_secs,
            )?;
        }
        self.dispatch(
            &edge.slug,
            &edge.endpoint,
            &edge.service_root,
            action,
            None,
            false,
            None,
            timeout_secs,
        )
    }

    fn stream_host_artifacts(
        &mut self,
        slug: &str,
        endpoint: &EndpointV1,
        service_root: &str,
        artifacts: &[ArtifactV1],
        timeout_secs: u64,
    ) -> Result<()> {
        ensure_authorization_current(self.admitted)?;
        self.closure.revalidate_host(slug, artifacts)?;
        for artifact in artifacts {
            self.dispatch(
                slug,
                endpoint,
                service_root,
                HostAction::Upload,
                Some(artifact),
                false,
                None,
                timeout_secs,
            )?;
        }
        Ok(())
    }

    fn dispatch(
        &mut self,
        slug: &str,
        endpoint: &EndpointV1,
        _service_root: &str,
        action: HostAction,
        upload: Option<&ArtifactV1>,
        recovery_only: bool,
        mutation: Option<MutationReservationIdentity<'_>>,
        timeout_secs: u64,
    ) -> Result<HostReceiptV1> {
        let timeout_ms = timeout_secs
            .checked_mul(1_000)
            .ok_or_else(|| eyre!("host action timeout overflow"))?;
        let action_deadline_unix_ms = now_unix_ms()?
            .checked_add(timeout_ms)
            .ok_or_else(|| eyre!("host action deadline overflow"))?;
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(timeout_secs))
            .ok_or_else(|| eyre!("host dispatch monotonic deadline overflow"))?;
        revalidate_pinned(&self.admitted.ssh_identity, "OpenSSH identity")?;
        revalidate_pinned(&self.admitted.known_hosts, "OpenSSH known-hosts")?;
        if !recovery_only && action != HostAction::Preflight && action != HostAction::Rollback {
            ensure_authorization_current(self.admitted)?;
            if action != HostAction::Cleanup {
                require_forward_lease_budget(self.admitted, timeout_secs)?;
            }
        }
        let (artifact_role, artifact_sha256, artifact_size, artifact_mode, stdin_file) =
            match upload {
                Some(artifact) if action == HostAction::Upload => (
                    artifact.role.clone(),
                    artifact.sha256.clone(),
                    artifact.size,
                    artifact.mode,
                    Some((
                        self.closure.stream_file(slug, &artifact.role)?,
                        artifact.size,
                    )),
                ),
                None if action != HostAction::Upload => (String::new(), String::new(), 0, 0, None),
                _ => return Err(eyre!("host upload request shape is inconsistent")),
            };
        let (
            mutation_operation,
            mutation_kind,
            mutation_phase,
            mutation_idempotency_key,
            mutation_prepared_base64,
            mutation_prepared_sha256,
            mutation_transaction_hash,
            mutation_evidence_base64,
        ) = mutation
            .map(|identity| {
                (
                    identity.operation.to_owned(),
                    identity.kind.to_owned(),
                    identity.phase.to_owned(),
                    identity.idempotency_key.to_owned(),
                    identity
                        .prepared
                        .map(|bytes| BASE64.encode(bytes))
                        .unwrap_or_default(),
                    identity.prepared_sha256.to_owned(),
                    identity.transaction_hash.to_owned(),
                    identity
                        .evidence
                        .map(|bytes| BASE64.encode(bytes))
                        .unwrap_or_default(),
                )
            })
            .unwrap_or_default();
        let request = HostRequestV1 {
            schema: HOST_REQUEST_SCHEMA_V1.to_owned(),
            action: action.label().to_owned(),
            recovery_only,
            host_slug: slug.to_owned(),
            inventory_base64: BASE64.encode(&self.admitted.inventory_bytes),
            authorization_base64: BASE64.encode(&self.admitted.authorization_bytes),
            authorization_semantic_sha256: self.admitted.authorization_sha256.clone(),
            trusted_key_base64: BASE64.encode(&self.admitted.trusted_key_bytes),
            trusted_key_sha256: sha256_hex(&self.admitted.trusted_key_bytes),
            action_deadline_unix_ms,
            artifact_role,
            artifact_sha256,
            artifact_size,
            artifact_mode,
            mutation_kind,
            mutation_phase,
            mutation_idempotency_key,
            mutation_operation,
            mutation_prepared_base64,
            mutation_prepared_sha256,
            mutation_transaction_hash,
            mutation_evidence_base64,
        };
        let request_bytes = json::to_json(&request)
            .wrap_err("failed to encode exact host request")?
            .into_bytes();
        let mut frame = format!("{:08x}\n", request_bytes.len()).into_bytes();
        frame.extend_from_slice(&request_bytes);
        let remote_command = format!("{FIXED_DISPATCHER} {HOST_DISPATCH_SUFFIX}");
        validate_remote_command(&remote_command)?;
        let (identity_path, identity_file) =
            inherited_input_path(&self.admitted.ssh_identity, "OpenSSH identity")?;
        let (known_hosts_path, known_hosts_file) =
            inherited_input_path(&self.admitted.known_hosts, "OpenSSH known-hosts")?;
        let mut args = ssh_common_args(endpoint, &identity_path, &known_hosts_path, timeout_secs);
        args.push(OsString::from("--"));
        args.push(OsString::from(format!(
            "{}@{}",
            endpoint.user, endpoint.hostname
        )));
        args.push(OsString::from(remote_command));
        let spec = ProcessSpec {
            program: PathBuf::from(SSH),
            args,
            stdin_prefix: frame,
            stdin_file,
            inherited_files: vec![identity_file, known_hosts_file],
            deadline,
        };
        let ambiguous_recoverable =
            matches!(action, HostAction::Restart | HostAction::MutationReserve) && !recovery_only;
        let process = match self.runner.run(&spec) {
            Ok(process) => process,
            Err(error) if ambiguous_recoverable => {
                eprintln!("public-reset remote host outcome is ambiguous (ephemeral): {error:#}");
                return Err(LocalMutationRecoveryPending {
                    action: "remote_host_action",
                }
                .into());
            }
            Err(error) => return Err(error),
        };
        if !process.status.success() && ambiguous_recoverable {
            eprintln!(
                "public-reset remote host rejected or disconnected after spawn (ephemeral): {}",
                String::from_utf8_lossy(&process.stderr)
            );
            return Err(LocalMutationRecoveryPending {
                action: "remote_host_action",
            }
            .into());
        }
        let output = require_success(process, "pinned SSH host dispatch")?;
        let receipt: HostReceiptV1 = match json::from_slice(&output) {
            Ok(receipt) => receipt,
            Err(error) if ambiguous_recoverable => {
                eprintln!("public-reset remote host receipt is ambiguous (ephemeral): {error:#}");
                return Err(LocalMutationRecoveryPending {
                    action: "remote_host_action",
                }
                .into());
            }
            Err(error) => {
                return Err(error).wrap_err("remote host did not return an exact V1 receipt");
            }
        };
        let verification = if action == HostAction::MutationReserve {
            verify_remote_reservation_receipt(&request, &receipt)
        } else if recovery_only {
            verify_remote_recovery_receipt(&request, &receipt)
        } else {
            verify_remote_receipt(&request, &receipt)
        };
        if let Err(error) = verification {
            if ambiguous_recoverable {
                eprintln!(
                    "public-reset remote host receipt binding is ambiguous (ephemeral): {error:#}"
                );
                return Err(LocalMutationRecoveryPending {
                    action: "remote_host_action",
                }
                .into());
            }
            return Err(error);
        }
        Ok(receipt)
    }

    fn recover_validator_restart(
        &mut self,
        validator: &ValidatorV1,
        timeout_secs: u64,
    ) -> Result<HostReceiptV1> {
        self.dispatch(
            &validator.slug,
            &validator.endpoint,
            &validator.service_root,
            HostAction::Restart,
            None,
            true,
            None,
            timeout_secs,
        )
    }

    fn coordinate_shared_prepared_mutation(
        &mut self,
        operation: &str,
        kind: &str,
        phase: &str,
        idempotency_key: &str,
        prepared: Option<&[u8]>,
        prepared_sha256: &str,
        transaction_hash: &str,
        evidence: Option<&[u8]>,
        recovery_only: bool,
        timeout_secs: u64,
    ) -> Result<RetainedPreparedMutation> {
        let edge = &self.admitted.inventory.edge;
        let slug = edge.slug.clone();
        let endpoint = edge.endpoint.clone();
        let service_root = edge.service_root.clone();
        let receipt = self.dispatch(
            &slug,
            &endpoint,
            &service_root,
            HostAction::MutationReserve,
            None,
            recovery_only,
            Some(MutationReservationIdentity {
                operation,
                kind,
                phase,
                idempotency_key,
                prepared,
                prepared_sha256,
                transaction_hash,
                evidence,
            }),
            timeout_secs,
        )?;
        if receipt.status == "absent" {
            return Ok(RetainedPreparedMutation {
                state: receipt.status,
                bytes: Vec::new(),
                sha256: String::new(),
                transaction_hash: String::new(),
            });
        }
        let bytes = BASE64
            .decode(&receipt.mutation_prepared_base64)
            .wrap_err("shared prepared mutation response is not base64")?;
        if bytes.is_empty()
            || bytes.len() > MAX_PREPARED_ENVELOPE_BYTES
            || sha256_hex(&bytes) != receipt.mutation_prepared_sha256
        {
            return Err(eyre!(
                "shared prepared mutation response bytes do not match their digest"
            ));
        }
        Ok(RetainedPreparedMutation {
            state: receipt.status,
            bytes,
            sha256: receipt.mutation_prepared_sha256,
            transaction_hash: receipt.mutation_transaction_hash,
        })
    }

    fn run_local_cli(
        &mut self,
        args: Vec<OsString>,
        inherited_files: Vec<File>,
        timeout_secs: u64,
        label: &str,
    ) -> Result<Vec<u8>> {
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(timeout_secs))
            .ok_or_else(|| eyre!("local reset action deadline overflow"))?;
        require_forward_lease_budget(self.admitted, timeout_secs)?;
        self.run_local_cli_until(args, inherited_files, timeout_secs, deadline, false, label)
    }

    fn run_local_cli_until(
        &mut self,
        args: Vec<OsString>,
        inherited_files: Vec<File>,
        _action_timeout_secs: u64,
        deadline: Instant,
        recovery_only: bool,
        label: &str,
    ) -> Result<Vec<u8>> {
        let output =
            self.run_local_cli_process_until(args, inherited_files, deadline, recovery_only)?;
        require_success(output, label)
    }

    fn run_local_cli_process_until(
        &mut self,
        args: Vec<OsString>,
        mut inherited_files: Vec<File>,
        deadline: Instant,
        recovery_only: bool,
    ) -> Result<ProcessOutput> {
        if !recovery_only {
            ensure_authorization_current(self.admitted)?;
        }
        self.runtime
            .revalidate(self.admitted, deadline, !recovery_only)?;
        if !recovery_only {
            ensure_authorization_current(self.admitted)?;
        }
        if deadline <= Instant::now() {
            return Err(eyre!(
                "local reset action deadline elapsed before CLI spawn"
            ));
        }
        let cli_file = self
            .closure
            .stream_file(&self.admitted.inventory.validators[0].slug, "iroha_cli")?;
        let cli = inherited_file_path(&cli_file)?;
        inherited_files.push(cli_file);
        self.runner.run(&ProcessSpec {
            program: cli,
            args,
            stdin_prefix: Vec::new(),
            stdin_file: None,
            inherited_files,
            deadline,
        })
    }

    fn doctor(&mut self, timeout_secs: u64) -> Result<()> {
        self.doctor_with_mode(timeout_secs, false)
    }

    fn doctor_with_mode(&mut self, timeout_secs: u64, recovery_only: bool) -> Result<()> {
        let args = vec![
            "taira".into(),
            "doctor".into(),
            "--public-root".into(),
            self.admitted
                .inventory
                .inrou_canary
                .public_root
                .clone()
                .into(),
            "--json".into(),
        ];
        let output = if recovery_only {
            let deadline = Instant::now()
                .checked_add(Duration::from_secs(timeout_secs))
                .ok_or_else(|| eyre!("doctor recovery deadline overflow"))?;
            self.run_local_cli_until(
                args,
                Vec::new(),
                timeout_secs,
                deadline,
                true,
                "same-revision Taira doctor",
            )?
        } else {
            self.run_local_cli(args, Vec::new(), timeout_secs, "same-revision Taira doctor")?
        };
        let value = parse_json_report(&output, "same-revision Taira doctor")?;
        validate_doctor_report(&value, &self.admitted.inventory.inrou_canary.public_root)
    }

    fn write_canary(&mut self, timeout_secs: u64) -> Result<()> {
        self.write_canary_for_phase(timeout_secs, "pre_edge", "write-canary.json")
    }

    fn write_canary_for_phase(
        &mut self,
        timeout_secs: u64,
        phase: &str,
        _receipt_name: &str,
    ) -> Result<()> {
        for kind in ["onboarding", "faucet", "write_canary"] {
            self.execute_write_canary_child(timeout_secs, phase, kind)?;
        }
        Ok(())
    }

    fn execute_write_canary_child(
        &mut self,
        timeout_secs: u64,
        phase: &str,
        kind: &str,
    ) -> Result<()> {
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(timeout_secs))
            .ok_or_else(|| eyre!("prepared write child deadline overflow"))?;
        let prepared =
            self.prepare_write_canary_child_until(deadline, timeout_secs, phase, kind)?;
        if prepared.state == "applied" {
            return Ok(());
        }
        let proof_required = prepared.transaction_hash.is_empty();
        let prepared = if proof_required {
            prepared
        } else {
            self.coordinate_shared_prepared_mutation(
                "submitted",
                kind,
                phase,
                &child_mutation_idempotency_key(
                    &self.admitted.inventory.authorization_nonce,
                    phase,
                    kind,
                ),
                None,
                "",
                "",
                None,
                false,
                remaining_seconds(deadline)?,
            )?
        };
        match self.run_write_canary_prepared_until(
            deadline,
            phase,
            kind,
            &prepared,
            proof_required,
        )? {
            PreparedMutationOutcome::Applied { value, evidence } => {
                self.mark_shared_prepared_applied(phase, kind, &prepared, &evidence, deadline)?;
                self.publish_local_receipt(
                    &format!("{}-{phase}.json", kind.replace('_', "-")),
                    &value,
                )
            }
            PreparedMutationOutcome::Pending => Err(LocalMutationRecoveryPending {
                action: "write_canary_child",
            }
            .into()),
            PreparedMutationOutcome::Rejected(class) => {
                Err(eyre!("prepared write child was rejected: {class}"))
            }
        }
    }

    fn run_journaled_write_canary_child(
        &mut self,
        progress: &mut dyn RecoveryProgress,
        mutation_index: usize,
        timeout_secs: u64,
        phase: &str,
        kind: &str,
    ) -> Result<()> {
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(timeout_secs))
            .ok_or_else(|| eyre!("journaled write child deadline overflow"))?;
        let prepared =
            self.prepare_write_canary_child_until(deadline, timeout_secs, phase, kind)?;
        if prepared.state == "applied" {
            let outcome =
                self.run_write_canary_prepared_until(deadline, phase, kind, &prepared, true)?;
            let PreparedMutationOutcome::Applied { value, .. } = outcome else {
                return Err(LocalMutationRecoveryPending {
                    action: "preapplied_write_canary_child",
                }
                .into());
            };
            progress.mark_submitted(mutation_index)?;
            self.publish_local_receipt(
                &format!("{}-{phase}.json", kind.replace('_', "-")),
                &value,
            )?;
            return progress.mark_applied(mutation_index);
        }
        progress.mark_submitted(mutation_index)?;
        let proof_required = prepared.transaction_hash.is_empty();
        let prepared = if proof_required {
            prepared
        } else {
            self.coordinate_shared_prepared_mutation(
                "submitted",
                kind,
                phase,
                &child_mutation_idempotency_key(
                    &self.admitted.inventory.authorization_nonce,
                    phase,
                    kind,
                ),
                None,
                "",
                "",
                None,
                false,
                remaining_seconds(deadline)?,
            )?
        };
        match self.run_write_canary_prepared_until(
            deadline,
            phase,
            kind,
            &prepared,
            proof_required,
        )? {
            PreparedMutationOutcome::Applied { value, evidence } => {
                self.mark_shared_prepared_applied(phase, kind, &prepared, &evidence, deadline)?;
                self.publish_local_receipt(
                    &format!("{}-{phase}.json", kind.replace('_', "-")),
                    &value,
                )?;
                progress.mark_applied(mutation_index)
            }
            PreparedMutationOutcome::Pending => Err(LocalMutationRecoveryPending {
                action: "write_canary_child",
            }
            .into()),
            PreparedMutationOutcome::Rejected(class) => {
                Err(eyre!("prepared write child was rejected: {class}"))
            }
        }
    }

    fn prepare_write_canary_child_until(
        &mut self,
        deadline: Instant,
        timeout_secs: u64,
        phase: &str,
        kind: &str,
    ) -> Result<RetainedPreparedMutation> {
        let idempotency_key = child_mutation_idempotency_key(
            &self.admitted.inventory.authorization_nonce,
            phase,
            kind,
        );
        let existing = self.coordinate_shared_prepared_mutation(
            "fetch",
            kind,
            phase,
            &idempotency_key,
            None,
            "",
            "",
            None,
            false,
            remaining_seconds(deadline)?,
        )?;
        if existing.state != "absent" {
            return Ok(existing);
        }
        let prerequisite = mutation_predecessor_kind(kind, phase)
            .map(|predecessor| {
                self.fetch_shared_prepared_mutation(
                    phase,
                    predecessor,
                    false,
                    remaining_seconds(deadline)?,
                )
            })
            .transpose()?
            .filter(|value| value.state == "applied");
        if mutation_predecessor_kind(kind, phase).is_some() && prerequisite.is_none() {
            return Err(eyre!(
                "write child preparation requires its exact Applied predecessor envelope"
            ));
        }
        let (candidate, _prepare_report) = self.run_write_canary_prepare_until(
            deadline,
            timeout_secs,
            phase,
            kind,
            &idempotency_key,
            prerequisite.as_ref(),
        )?;
        let candidate_sha256 = sha256_hex(&candidate);
        let transaction_hash = prepared_envelope_transaction_hash(&candidate)?;
        self.coordinate_shared_prepared_mutation(
            "prepare",
            kind,
            phase,
            &idempotency_key,
            Some(&candidate),
            &candidate_sha256,
            &transaction_hash,
            None,
            false,
            remaining_seconds(deadline)?,
        )
    }

    fn fetch_shared_prepared_mutation(
        &mut self,
        phase: &str,
        kind: &str,
        recovery_only: bool,
        timeout_secs: u64,
    ) -> Result<RetainedPreparedMutation> {
        self.coordinate_shared_prepared_mutation(
            "fetch",
            kind,
            phase,
            &child_mutation_idempotency_key(
                &self.admitted.inventory.authorization_nonce,
                phase,
                kind,
            ),
            None,
            "",
            "",
            None,
            recovery_only,
            timeout_secs,
        )
    }

    fn mark_shared_prepared_applied(
        &mut self,
        phase: &str,
        kind: &str,
        prepared: &RetainedPreparedMutation,
        evidence: &[u8],
        deadline: Instant,
    ) -> Result<()> {
        let result = self.coordinate_shared_prepared_mutation(
            "applied",
            kind,
            phase,
            &child_mutation_idempotency_key(
                &self.admitted.inventory.authorization_nonce,
                phase,
                kind,
            ),
            None,
            "",
            "",
            Some(evidence),
            true,
            remaining_seconds(deadline)?,
        )?;
        if result.state != "applied"
            || result.sha256 != prepared.sha256
            || result.transaction_hash != prepared.transaction_hash
        {
            return Err(eyre!(
                "shared prepared mutation Applied marker changed its immutable envelope"
            ));
        }
        Ok(())
    }

    fn run_write_canary_prepare_until(
        &mut self,
        deadline: Instant,
        timeout_secs: u64,
        phase: &str,
        kind: &str,
        idempotency_key: &str,
        prerequisite: Option<&RetainedPreparedMutation>,
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        let scratch = self
            .local_receipt_root
            .join(format!(".{idempotency_key}.candidate.next"));
        if scratch.exists() {
            let metadata = fs::symlink_metadata(&scratch)?;
            #[cfg(unix)]
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || metadata.uid() != rustix::process::geteuid().as_raw()
                || metadata.nlink() != 1
            {
                return Err(eyre!("prepared envelope scratch file has unsafe custody"));
            }
            fs::remove_file(&scratch)?;
            sync_directory(&self.local_receipt_root)?;
        }
        let output_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&scratch)?;
        let mut output_reader = output_file.try_clone()?;
        let output_path = inherited_file_path(&output_file)?;
        let (mut args, mut inherited_files) =
            self.write_canary_base_args(phase, kind, idempotency_key, timeout_secs, true)?;
        args.push(OsString::from("--prepare-envelope"));
        args.push(OsString::from("--prepared-output-fd"));
        args.push(output_file.as_raw_fd().to_string().into());
        debug_assert_eq!(
            output_path,
            PathBuf::from(format!("/proc/self/fd/{}", output_file.as_raw_fd()))
        );
        inherited_files.push(output_file);
        if let Some(prerequisite) = prerequisite {
            let file = self.open_retained_prepared_envelope(prerequisite)?;
            args.push(OsString::from("--prerequisite-envelope-fd"));
            args.push(file.as_raw_fd().to_string().into());
            inherited_files.push(file);
        }
        let stdout = self.run_local_cli_until(
            args,
            inherited_files,
            timeout_secs,
            deadline,
            false,
            "prepare exact write-canary child envelope",
        )?;
        let report = parse_json_report(&stdout, "prepared write child")?;
        let outcome = report
            .as_object()
            .and_then(|object| object.get("recovery_outcome"))
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("prepared write child report omits recovery_outcome"))?;
        if !matches!(outcome, "Prepared" | "ProofRequired") {
            return Err(eyre!(
                "prepared write child produced an invalid preparation outcome"
            ));
        }
        validate_prepared_write_report(
            &report,
            self.admitted,
            phase,
            kind,
            idempotency_key,
            outcome,
            None,
        )?;
        output_reader.rewind()?;
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut output_reader)
            .take(u64::try_from(MAX_PREPARED_ENVELOPE_BYTES + 1).expect("bounded"))
            .read_to_end(&mut bytes)?;
        output_reader.sync_all()?;
        fs::remove_file(&scratch)?;
        sync_directory(&self.local_receipt_root)?;
        if bytes.is_empty() || bytes.len() > MAX_PREPARED_ENVELOPE_BYTES {
            return Err(eyre!("prepared write child envelope is empty or oversized"));
        }
        validate_prepared_report_envelope_bytes(&report, &bytes)?;
        let transaction_hash = prepared_envelope_transaction_hash(&bytes)?;
        if (outcome == "ProofRequired") != transaction_hash.is_empty() {
            return Err(eyre!(
                "prepared write child proof requirement does not match its tagged envelope"
            ));
        }
        Ok((bytes, stdout))
    }

    fn run_write_canary_prepared_until(
        &mut self,
        deadline: Instant,
        phase: &str,
        kind: &str,
        prepared: &RetainedPreparedMutation,
        recover_only: bool,
    ) -> Result<PreparedMutationOutcome> {
        let idempotency_key = child_mutation_idempotency_key(
            &self.admitted.inventory.authorization_nonce,
            phase,
            kind,
        );
        let file = self.open_retained_prepared_envelope(prepared)?;
        let (mut args, mut inherited_files) = self.write_canary_base_args(
            phase,
            kind,
            &idempotency_key,
            remaining_seconds(deadline)?,
            !recover_only,
        )?;
        args.push(if recover_only {
            OsString::from("--recover-prepared-envelope-fd")
        } else {
            OsString::from("--submit-prepared-envelope-fd")
        });
        args.push(file.as_raw_fd().to_string().into());
        inherited_files.push(file);
        let process =
            self.run_local_cli_process_until(args, inherited_files, deadline, recover_only)?;
        let value = parse_json_report(&process.stdout, "exact prepared write child")?;
        let outcome = value
            .as_object()
            .and_then(|object| object.get("recovery_outcome"))
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("prepared write child report omits recovery_outcome"))?;
        validate_prepared_write_report(
            &value,
            self.admitted,
            phase,
            kind,
            &idempotency_key,
            outcome,
            Some(prepared),
        )?;
        match outcome {
            "Applied" => {
                let evidence = canonical_json_report_bytes(&value)?;
                Ok(PreparedMutationOutcome::Applied { value, evidence })
            }
            "Pending" => Ok(PreparedMutationOutcome::Pending),
            "Rejected" => Ok(PreparedMutationOutcome::Rejected(
                required_report_evidence(&value, "prepared write child")?.to_owned(),
            )),
            _ => Err(eyre!("prepared write child report has an invalid outcome")),
        }
    }

    fn write_canary_base_args(
        &self,
        phase: &str,
        kind: &str,
        idempotency_key: &str,
        _timeout_secs: u64,
        include_submission_secret: bool,
    ) -> Result<(Vec<OsString>, Vec<File>)> {
        let (config_path, config_file) =
            inherited_input_path(&self.runtime.client_config, "Taira runtime client config")?;
        let mut args = vec!["-c".into(), config_path.into_os_string()];
        let mut inherited_files = vec![config_file];
        args.extend(self.runtime.fee_args.iter().cloned());
        args.extend([
            OsString::from("taira"),
            OsString::from("write-canary"),
            OsString::from("--public-root"),
            OsString::from(&self.admitted.inventory.inrou_canary.public_root),
            OsString::from("--operation"),
            OsString::from(match kind {
                "onboarding" => "onboarding",
                "faucet" => "faucet",
                "write_canary" => "final-canary",
                _ => return Err(eyre!("unsupported prepared write child kind")),
            }),
            OsString::from("--authorization-sha256"),
            OsString::from(&self.admitted.authorization_sha256),
            OsString::from("--authorization-nonce"),
            OsString::from(&self.admitted.inventory.authorization_nonce),
            OsString::from("--mutation-phase"),
            OsString::from(phase),
            OsString::from("--idempotency-key"),
            OsString::from(idempotency_key),
            OsString::from("--execution-expires-at-unix-ms"),
            OsString::from(
                self.admitted
                    .authorization
                    .claims
                    .execution_expires_at_unix_ms
                    .to_string(),
            ),
        ]);
        if kind == "faucet" {
            args.extend([
                OsString::from("--faucet-authority"),
                OsString::from(&self.admitted.inventory.faucet_policy.authority),
                OsString::from("--faucet-asset-id"),
                OsString::from(&self.admitted.inventory.faucet_policy.asset_definition_id),
                OsString::from("--faucet-amount"),
                OsString::from(self.admitted.inventory.faucet_policy.amount.to_string()),
            ]);
        }
        if kind == "onboarding" && include_submission_secret {
            let (token_path, token_file) = inherited_input_path(
                self.runtime
                    .onboarding_token
                    .as_ref()
                    .ok_or_else(|| eyre!("write-canary submission lacks onboarding custody"))?,
                "Taira onboarding token",
            )?;
            args.push(OsString::from("--onboarding-token-file"));
            args.push(token_path.into_os_string());
            inherited_files.push(token_file);
        }
        args.push(OsString::from("--json"));
        Ok((args, inherited_files))
    }

    fn open_retained_prepared_envelope(&self, prepared: &RetainedPreparedMutation) -> Result<File> {
        if prepared.bytes.is_empty()
            || prepared.bytes.len() > MAX_PREPARED_ENVELOPE_BYTES
            || sha256_hex(&prepared.bytes) != prepared.sha256
        {
            return Err(eyre!("retained prepared envelope identity is invalid"));
        }
        let root = self.local_receipt_root.join("prepared-envelopes-v1");
        ensure_private_directory(&root)?;
        let name = format!("{}.json", prepared.sha256);
        publish_private_noreplace(&root, &name, &prepared.bytes)?;
        let path = root.join(name);
        let file = File::open(&path)?;
        let metadata = file.metadata()?;
        #[cfg(unix)]
        if !metadata.is_file()
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o7777 != 0o600
            || metadata.nlink() != 1
        {
            return Err(eyre!("retained prepared envelope has unsafe custody"));
        }
        Ok(file)
    }

    fn recover_write_canary_child_until(
        &mut self,
        deadline: Instant,
        mutation: &RecoveryMutationV1,
    ) -> Result<PreparedMutationOutcome> {
        let prepared = self.fetch_shared_prepared_mutation(
            &mutation.phase,
            &mutation.kind,
            true,
            remaining_seconds(deadline)?,
        )?;
        if matches!(prepared.state.as_str(), "absent" | "prepared") {
            return Ok(PreparedMutationOutcome::Rejected(
                "prepared_child_not_submitted".to_owned(),
            ));
        }
        let outcome = self.run_write_canary_prepared_until(
            deadline,
            &mutation.phase,
            &mutation.kind,
            &prepared,
            true,
        )?;
        match outcome {
            PreparedMutationOutcome::Applied { value, evidence } => {
                if prepared.state != "applied" {
                    self.mark_shared_prepared_applied(
                        &mutation.phase,
                        &mutation.kind,
                        &prepared,
                        &evidence,
                        deadline,
                    )?;
                }
                Ok(PreparedMutationOutcome::Applied { value, evidence })
            }
            other => Ok(other),
        }
    }

    fn recover_inrou_prepared_child_until(
        &mut self,
        deadline: Instant,
        mutation: &RecoveryMutationV1,
    ) -> Result<PreparedMutationOutcome> {
        let prepared = self.fetch_shared_prepared_mutation(
            &mutation.phase,
            &mutation.kind,
            true,
            remaining_seconds(deadline)?,
        )?;
        if matches!(prepared.state.as_str(), "absent" | "prepared") {
            return Ok(PreparedMutationOutcome::Rejected(
                "prepared_inrou_child_not_submitted".to_owned(),
            ));
        }
        let outcome = self.run_inrou_prepared_until(
            deadline,
            &mutation.phase,
            &mutation.kind,
            &prepared,
            true,
        )?;
        if let PreparedMutationOutcome::Applied { evidence, .. } = &outcome {
            if prepared.state != "applied" {
                self.mark_shared_prepared_applied(
                    &mutation.phase,
                    &mutation.kind,
                    &prepared,
                    evidence,
                    deadline,
                )?;
            }
        }
        Ok(outcome)
    }

    fn run_journaled_inrou_prepared_child(
        &mut self,
        progress: &mut dyn RecoveryProgress,
        mutation_index: usize,
        timeout_secs: u64,
        phase: &str,
        kind: &str,
    ) -> Result<()> {
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(timeout_secs))
            .ok_or_else(|| eyre!("journaled Inrou child deadline overflow"))?;
        let prepared = self.prepare_inrou_child_until(deadline, timeout_secs, phase, kind)?;
        if prepared.state == "applied" {
            let outcome = self.run_inrou_prepared_until(deadline, phase, kind, &prepared, true)?;
            let PreparedMutationOutcome::Applied { value, .. } = outcome else {
                return Err(LocalMutationRecoveryPending {
                    action: "preapplied_inrou_child",
                }
                .into());
            };
            progress.mark_submitted(mutation_index)?;
            self.publish_local_receipt(
                &format!("{}-{phase}.json", kind.replace('_', "-")),
                &value,
            )?;
            return progress.mark_applied(mutation_index);
        }
        progress.mark_submitted(mutation_index)?;
        let prepared = self.coordinate_shared_prepared_mutation(
            "submitted",
            kind,
            phase,
            &child_mutation_idempotency_key(
                &self.admitted.inventory.authorization_nonce,
                phase,
                kind,
            ),
            None,
            "",
            "",
            None,
            false,
            remaining_seconds(deadline)?,
        )?;
        match self.run_inrou_prepared_until(deadline, phase, kind, &prepared, false)? {
            PreparedMutationOutcome::Applied { value, evidence } => {
                self.mark_shared_prepared_applied(phase, kind, &prepared, &evidence, deadline)?;
                self.publish_local_receipt(
                    &format!("{}-{phase}.json", kind.replace('_', "-")),
                    &value,
                )?;
                progress.mark_applied(mutation_index)
            }
            PreparedMutationOutcome::Pending => Err(LocalMutationRecoveryPending {
                action: "inrou_prepared_child",
            }
            .into()),
            PreparedMutationOutcome::Rejected(class) => {
                Err(eyre!("prepared Inrou child was rejected: {class}"))
            }
        }
    }

    fn prepare_inrou_child_until(
        &mut self,
        deadline: Instant,
        timeout_secs: u64,
        phase: &str,
        kind: &str,
    ) -> Result<RetainedPreparedMutation> {
        let idempotency_key = child_mutation_idempotency_key(
            &self.admitted.inventory.authorization_nonce,
            phase,
            kind,
        );
        let existing = self.coordinate_shared_prepared_mutation(
            "fetch",
            kind,
            phase,
            &idempotency_key,
            None,
            "",
            "",
            None,
            false,
            remaining_seconds(deadline)?,
        )?;
        if existing.state != "absent" {
            return Ok(existing);
        }
        let predecessor = mutation_predecessor_kind(kind, phase)
            .ok_or_else(|| eyre!("Inrou child has no exact predecessor"))?;
        let prerequisite = self.fetch_shared_prepared_mutation(
            phase,
            predecessor,
            false,
            remaining_seconds(deadline)?,
        )?;
        if prerequisite.state != "applied" {
            return Err(eyre!(
                "Inrou child preparation requires its exact Applied predecessor"
            ));
        }
        let candidate = self.run_inrou_prepare_until(
            deadline,
            timeout_secs,
            phase,
            kind,
            &idempotency_key,
            &prerequisite,
        )?;
        let candidate_sha256 = sha256_hex(&candidate);
        let transaction_hash = prepared_envelope_transaction_hash(&candidate)?;
        if transaction_hash.is_empty() {
            return Err(eyre!(
                "an Inrou prepared child must contain one exact signed transaction"
            ));
        }
        self.coordinate_shared_prepared_mutation(
            "prepare",
            kind,
            phase,
            &idempotency_key,
            Some(&candidate),
            &candidate_sha256,
            &transaction_hash,
            None,
            false,
            remaining_seconds(deadline)?,
        )
    }

    fn run_inrou_prepare_until(
        &mut self,
        deadline: Instant,
        timeout_secs: u64,
        phase: &str,
        kind: &str,
        idempotency_key: &str,
        prerequisite: &RetainedPreparedMutation,
    ) -> Result<Vec<u8>> {
        let scratch = self
            .local_receipt_root
            .join(format!(".{idempotency_key}.inrou-candidate.next"));
        if scratch.exists() {
            let metadata = fs::symlink_metadata(&scratch)?;
            #[cfg(unix)]
            if metadata.file_type().is_symlink()
                || !metadata.is_file()
                || metadata.uid() != rustix::process::geteuid().as_raw()
                || metadata.nlink() != 1
            {
                return Err(eyre!("Inrou prepared-envelope scratch has unsafe custody"));
            }
            fs::remove_file(&scratch)?;
            sync_directory(&self.local_receipt_root)?;
        }
        let output_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&scratch)?;
        let mut output_reader = output_file.try_clone()?;
        let (mut args, mut inherited_files) =
            self.inrou_prepared_base_args(phase, kind, idempotency_key, timeout_secs)?;
        args.push(OsString::from("--prepare-envelope"));
        args.push(OsString::from("--prepared-output-fd"));
        args.push(output_file.as_raw_fd().to_string().into());
        inherited_files.push(output_file);
        let predecessor_file = self.open_retained_prepared_envelope(prerequisite)?;
        args.push(OsString::from("--prerequisite-envelope-fd"));
        args.push(predecessor_file.as_raw_fd().to_string().into());
        inherited_files.push(predecessor_file);
        let stdout = self.run_local_cli_until(
            args,
            inherited_files,
            timeout_secs,
            deadline,
            false,
            "prepare exact Inrou child envelope",
        )?;
        let report = parse_json_report(&stdout, "prepared Inrou child")?;
        validate_prepared_inrou_report(
            &report,
            self.admitted,
            phase,
            kind,
            idempotency_key,
            "Prepared",
            None,
        )?;
        output_reader.rewind()?;
        let mut bytes = Vec::new();
        std::io::Read::by_ref(&mut output_reader)
            .take(u64::try_from(MAX_PREPARED_ENVELOPE_BYTES + 1).expect("bounded"))
            .read_to_end(&mut bytes)?;
        output_reader.sync_all()?;
        fs::remove_file(&scratch)?;
        sync_directory(&self.local_receipt_root)?;
        if bytes.is_empty() || bytes.len() > MAX_PREPARED_ENVELOPE_BYTES {
            return Err(eyre!("prepared Inrou child envelope is empty or oversized"));
        }
        validate_prepared_report_envelope_bytes(&report, &bytes)?;
        Ok(bytes)
    }

    fn run_inrou_prepared_until(
        &mut self,
        deadline: Instant,
        phase: &str,
        kind: &str,
        prepared: &RetainedPreparedMutation,
        recover_only: bool,
    ) -> Result<PreparedMutationOutcome> {
        let idempotency_key = child_mutation_idempotency_key(
            &self.admitted.inventory.authorization_nonce,
            phase,
            kind,
        );
        let file = self.open_retained_prepared_envelope(prepared)?;
        let (mut args, mut inherited_files) = self.inrou_prepared_base_args(
            phase,
            kind,
            &idempotency_key,
            remaining_seconds(deadline)?,
        )?;
        args.push(if recover_only {
            OsString::from("--recover-prepared-envelope-fd")
        } else {
            OsString::from("--submit-prepared-envelope-fd")
        });
        args.push(file.as_raw_fd().to_string().into());
        inherited_files.push(file);
        let process =
            self.run_local_cli_process_until(args, inherited_files, deadline, recover_only)?;
        let value = parse_json_report(&process.stdout, "exact prepared Inrou child")?;
        let outcome = value
            .as_object()
            .and_then(|object| object.get("recovery_outcome"))
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("prepared Inrou child report omits recovery_outcome"))?;
        validate_prepared_inrou_report(
            &value,
            self.admitted,
            phase,
            kind,
            &idempotency_key,
            outcome,
            Some(prepared),
        )?;
        match outcome {
            "Applied" => {
                let evidence = canonical_json_report_bytes(&value)?;
                Ok(PreparedMutationOutcome::Applied { value, evidence })
            }
            "Pending" => Ok(PreparedMutationOutcome::Pending),
            "Rejected" => Ok(PreparedMutationOutcome::Rejected(
                required_report_evidence(&value, "prepared Inrou child")?.to_owned(),
            )),
            _ => Err(eyre!("prepared Inrou child report has an invalid outcome")),
        }
    }

    fn inrou_prepared_base_args(
        &self,
        phase: &str,
        kind: &str,
        idempotency_key: &str,
        timeout_secs: u64,
    ) -> Result<(Vec<OsString>, Vec<File>)> {
        let (config_path, config_file) =
            inherited_input_path(&self.runtime.client_config, "Taira runtime client config")?;
        let operation = match kind {
            "inrou_bundle_pin" => "bundle-pin",
            "inrou_guest_pin" => "guest-pin",
            "inrou_canary" => "service-mutation",
            _ => return Err(eyre!("unsupported prepared Inrou child kind")),
        };
        let mut args = vec!["-c".into(), config_path.into_os_string()];
        args.extend(self.runtime.fee_args.iter().cloned());
        args.extend([
            OsString::from("taira"),
            OsString::from("inrou-canary"),
            OsString::from("--public-root"),
            OsString::from(&self.admitted.inventory.inrou_canary.public_root),
            OsString::from("--stage-dir"),
            self.runtime.inrou_stage_dir.as_os_str().to_owned(),
            OsString::from("--mode"),
            OsString::from("deploy"),
            OsString::from("--operation"),
            OsString::from(operation),
            OsString::from("--authorization-sha256"),
            OsString::from(&self.admitted.authorization_sha256),
            OsString::from("--authorization-nonce"),
            OsString::from(&self.admitted.inventory.authorization_nonce),
            OsString::from("--mutation-phase"),
            OsString::from(phase),
            OsString::from("--idempotency-key"),
            OsString::from(idempotency_key),
            OsString::from("--execution-expires-at-unix-ms"),
            OsString::from(
                self.admitted
                    .authorization
                    .claims
                    .execution_expires_at_unix_ms
                    .to_string(),
            ),
            OsString::from("--timeout-secs"),
            OsString::from(timeout_secs.to_string()),
            OsString::from("--json"),
        ]);
        Ok((args, vec![config_file]))
    }

    fn inrou_canary(&mut self, timeout_secs: u64) -> Result<()> {
        for kind in ["inrou_bundle_pin", "inrou_guest_pin", "inrou_canary"] {
            let deadline = Instant::now()
                .checked_add(Duration::from_secs(timeout_secs))
                .ok_or_else(|| eyre!("prepared Inrou child deadline overflow"))?;
            let prepared =
                self.prepare_inrou_child_until(deadline, timeout_secs, "pre_edge", kind)?;
            if prepared.state == "applied" {
                continue;
            }
            let prepared = self.coordinate_shared_prepared_mutation(
                "submitted",
                kind,
                "pre_edge",
                &child_mutation_idempotency_key(
                    &self.admitted.inventory.authorization_nonce,
                    "pre_edge",
                    kind,
                ),
                None,
                "",
                "",
                None,
                false,
                remaining_seconds(deadline)?,
            )?;
            match self.run_inrou_prepared_until(deadline, "pre_edge", kind, &prepared, false)? {
                PreparedMutationOutcome::Applied { value, evidence } => {
                    self.mark_shared_prepared_applied(
                        "pre_edge", kind, &prepared, &evidence, deadline,
                    )?;
                    self.publish_local_receipt(
                        &format!("{}-pre-edge.json", kind.replace('_', "-")),
                        &value,
                    )?;
                }
                PreparedMutationOutcome::Pending => {
                    return Err(LocalMutationRecoveryPending {
                        action: "inrou_prepared_child",
                    }
                    .into());
                }
                PreparedMutationOutcome::Rejected(class) => {
                    return Err(eyre!("prepared Inrou child was rejected: {class}"));
                }
            }
        }
        Ok(())
    }

    fn inrou_check(&mut self, timeout_secs: u64, wave: usize) -> Result<()> {
        self.inrou_check_with_mode(timeout_secs, wave, false)
    }

    fn inrou_check_with_mode(
        &mut self,
        timeout_secs: u64,
        wave: usize,
        recovery_only: bool,
    ) -> Result<()> {
        let receipt_name = format!("inrou-restart-wave-{wave}.json");
        self.require_fresh_inrou_check(&receipt_name, timeout_secs, recovery_only)
    }

    fn require_fresh_inrou_check(
        &mut self,
        receipt_name: &str,
        timeout_secs: u64,
        recovery_only: bool,
    ) -> Result<()> {
        let had_prior_receipt = self
            .validate_existing_local_receipt(receipt_name, validate_retained_inrou_check_report)?;
        require_fresh_liveness_report(
            self,
            had_prior_receipt,
            |transport| transport.run_inrou_check_report_with_mode(timeout_secs, recovery_only),
            |value, transport| validate_fresh_inrou_check_report(value, transport.admitted),
            |transport, value| transport.publish_local_receipt(receipt_name, value),
        )
    }

    fn run_inrou_check_report_with_mode(
        &mut self,
        timeout_secs: u64,
        recovery_only: bool,
    ) -> Result<norito::json::Value> {
        let (config_path, config_file) =
            inherited_input_path(&self.runtime.client_config, "Taira runtime client config")?;
        let args = vec![
            "-c".into(),
            config_path.into_os_string(),
            "taira".into(),
            "inrou-check".into(),
            "--public-root".into(),
            self.admitted
                .inventory
                .inrou_canary
                .public_root
                .clone()
                .into(),
            "--stage-dir".into(),
            self.runtime.inrou_stage_dir.as_os_str().to_owned(),
            "--mode".into(),
            "deploy".into(),
            "--timeout-secs".into(),
            timeout_secs.to_string().into(),
            "--json".into(),
        ];
        let output = if recovery_only {
            let deadline = Instant::now()
                .checked_add(Duration::from_secs(timeout_secs))
                .ok_or_else(|| eyre!("Inrou-check recovery deadline overflow"))?;
            self.run_local_cli_until(
                args,
                vec![config_file],
                timeout_secs,
                deadline,
                true,
                "same-revision Inrou restart check",
            )?
        } else {
            self.run_local_cli(
                args,
                vec![config_file],
                timeout_secs,
                "same-revision Inrou restart check",
            )?
        };
        let value = parse_json_report(&output, "same-revision Inrou restart check")?;
        Ok(value)
    }

    fn convergence(&mut self, timeout_secs: u64, wave: usize, recovery_only: bool) -> Result<()> {
        if !recovery_only {
            require_forward_lease_budget(self.admitted, timeout_secs)?;
        }
        let receipt_name = format!("convergence-wave-{wave}.json");
        let previous = if wave == 0 {
            None
        } else {
            let previous_name = format!("convergence-wave-{}.json", wave - 1);
            let value = self
                .read_local_receipt(&previous_name)?
                .ok_or_else(|| eyre!("restart convergence lacks its preceding wave receipt"))?;
            Some(validate_convergence_wave(
                &value,
                wave - 1,
                &self.admitted.inventory,
            )?)
        };
        let had_prior_receipt = if let Some(value) = self.read_local_receipt(&receipt_name)? {
            let checkpoint = validate_convergence_wave(&value, wave, &self.admitted.inventory)?;
            require_successor_checkpoint(previous.as_ref(), &checkpoint)?;
            true
        } else {
            false
        };
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            let mut reports = Vec::with_capacity(4);
            let mut common = None;
            let mut agrees = true;
            for index in 0..self.runtime.validator_client_configs.len() {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(eyre!(
                        "four-validator convergence did not reach one atomic checkpoint"
                    ));
                }
                let (config_path, config_file) = inherited_input_path(
                    &self.runtime.validator_client_configs[index],
                    "validator client config",
                )?;
                let poll_deadline = Instant::now()
                    .checked_add(Duration::from_secs(10))
                    .ok_or_else(|| eyre!("convergence poll deadline overflow"))?
                    .min(deadline);
                let output = self.run_local_cli_until(
                    vec![
                        "-c".into(),
                        config_path.into_os_string(),
                        "--output-format".into(),
                        "json".into(),
                        "ops".into(),
                        "sumeragi".into(),
                        "status".into(),
                    ],
                    vec![config_file],
                    timeout_secs,
                    poll_deadline,
                    recovery_only,
                    "operator-signed validator convergence status",
                )?;
                let value = parse_json_report(&output, "validator convergence status")?;
                let observed = validate_convergence_status(
                    &value,
                    &self.admitted.inventory.validators[index],
                )?;
                if common
                    .as_ref()
                    .is_some_and(|expected| expected != &observed)
                {
                    agrees = false;
                } else if common.is_none() {
                    common = Some(observed);
                }
                reports.push(value);
            }
            if agrees {
                let checkpoint = common.expect("four reports yield a checkpoint");
                if require_successor_checkpoint(previous.as_ref(), &checkpoint).is_ok() {
                    let mut report = norito::json::Map::new();
                    report.insert(
                        "schema".to_owned(),
                        norito::json::Value::from("iroha.taira.public-reset.convergence-wave.v1"),
                    );
                    report.insert(
                        "wave".to_owned(),
                        norito::json::Value::from(u64::try_from(wave).expect("bounded wave")),
                    );
                    report.insert("height".to_owned(), norito::json::Value::from(checkpoint.0));
                    report.insert(
                        "height_context_id".to_owned(),
                        norito::json::Value::from(checkpoint.1.clone()),
                    );
                    report.insert(
                        "block_hash".to_owned(),
                        norito::json::Value::from(checkpoint.2.clone()),
                    );
                    report.insert(
                        "last_commit_qc".to_owned(),
                        json::from_str(&checkpoint.3)
                            .wrap_err("failed to retain canonical commit-QC evidence")?,
                    );
                    report.insert(
                        "validator_reports".to_owned(),
                        norito::json::Value::Array(reports),
                    );
                    if !had_prior_receipt {
                        self.publish_local_receipt(
                            &receipt_name,
                            &norito::json::Value::Object(report),
                        )?;
                    }
                    return Ok(());
                }
            }
            if Instant::now() >= deadline {
                return Err(eyre!(
                    "four-validator convergence did not reach one successor checkpoint"
                ));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                continue;
            }
            std::thread::sleep(Duration::from_millis(250).min(remaining));
        }
    }

    fn read_local_receipt(&self, name: &str) -> Result<Option<norito::json::Value>> {
        validate_receipt_name(name)?;
        self.reconcile_local_receipt_staging(name)?;
        let path = self.local_receipt_root.join(name);
        if !path.exists() {
            return Ok(None);
        }
        let (file, snapshot) = open_pinned_regular(&path, "local reset receipt")?;
        let bytes = super::read_pinned_bytes(
            &path,
            "local reset receipt",
            file,
            &snapshot,
            MAX_PROCESS_OUTPUT as u64,
        )?;
        let envelope: norito::json::Value =
            json::from_slice(&bytes).wrap_err("local receipt envelope is invalid")?;
        let object = envelope
            .as_object()
            .ok_or_else(|| eyre!("local receipt envelope must be an object"))?;
        if object.get("schema").and_then(norito::json::Value::as_str)
            != Some(LOCAL_RECEIPT_SCHEMA_V1)
            || object
                .get("authorization_sha256")
                .and_then(norito::json::Value::as_str)
                != Some(self.admitted.authorization_sha256.as_str())
            || object
                .get("authorization_nonce")
                .and_then(norito::json::Value::as_str)
                != Some(self.admitted.inventory.authorization_nonce.as_str())
            || object
                .get("receipt_name")
                .and_then(norito::json::Value::as_str)
                != Some(name)
        {
            return Err(eyre!("local receipt envelope does not bind this execution"));
        }
        Ok(Some(object.get("report").cloned().ok_or_else(|| {
            eyre!("local receipt envelope omits report")
        })?))
    }

    fn reconcile_local_receipt_staging(&self, name: &str) -> Result<()> {
        let staging_name = format!(".{name}.next");
        let staging = self.local_receipt_root.join(&staging_name);
        let Ok(metadata) = fs::symlink_metadata(&staging) else {
            return Ok(());
        };
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(eyre!("local receipt staging is not a regular file"));
        }
        let (file, snapshot) = open_pinned_regular(&staging, "staged local reset receipt")?;
        super::require_owner_private_snapshot(&snapshot, "staged local reset receipt")?;
        let bytes = super::read_pinned_bytes(
            &staging,
            "staged local reset receipt",
            file,
            &snapshot,
            MAX_PROCESS_OUTPUT as u64,
        )?;
        let destination = self.local_receipt_root.join(name);
        if destination.exists() {
            let (file, snapshot) = open_pinned_regular(&destination, "local reset receipt")?;
            let actual = super::read_pinned_bytes(
                &destination,
                "local reset receipt",
                file,
                &snapshot,
                MAX_PROCESS_OUTPUT as u64,
            )?;
            if actual != bytes {
                return Err(eyre!(
                    "staged local receipt conflicts with its published destination"
                ));
            }
            sync_private_regular(&destination, "published local reset receipt")?;
            fs::remove_file(&staging)?;
            return sync_directory(&self.local_receipt_root);
        }
        let complete = json::from_slice::<norito::json::Value>(&bytes)
            .ok()
            .and_then(|value| value.as_object().cloned())
            .is_some_and(|object| {
                object.get("schema").and_then(norito::json::Value::as_str)
                    == Some(LOCAL_RECEIPT_SCHEMA_V1)
                    && object
                        .get("authorization_sha256")
                        .and_then(norito::json::Value::as_str)
                        == Some(self.admitted.authorization_sha256.as_str())
                    && object
                        .get("authorization_nonce")
                        .and_then(norito::json::Value::as_str)
                        == Some(self.admitted.inventory.authorization_nonce.as_str())
                    && object
                        .get("receipt_name")
                        .and_then(norito::json::Value::as_str)
                        == Some(name)
                    && object.contains_key("report")
            });
        let parent = File::open(&self.local_receipt_root)?;
        if complete {
            sync_private_regular(&staging, "staged local reset receipt")?;
            rustix::fs::renameat_with(
                &parent,
                OsStr::new(&staging_name),
                &parent,
                OsStr::new(name),
                rustix::fs::RenameFlags::NOREPLACE,
            )?;
        } else if bytes.last() != Some(&b'}') {
            rustix::fs::unlinkat(
                &parent,
                OsStr::new(&staging_name),
                rustix::fs::AtFlags::empty(),
            )?;
        } else {
            return Err(eyre!(
                "complete-looking local receipt staging is invalid or belongs to another execution"
            ));
        }
        parent.sync_all()?;
        Ok(())
    }

    fn validate_existing_local_receipt(
        &self,
        name: &str,
        validator: fn(&norito::json::Value, &AdmittedReset) -> Result<()>,
    ) -> Result<bool> {
        let Some(bytes) = self.read_local_receipt(name)? else {
            return Ok(false);
        };
        validator(&bytes, self.admitted)?;
        Ok(true)
    }

    fn publish_local_receipt(&self, name: &str, report: &norito::json::Value) -> Result<()> {
        validate_receipt_name(name)?;
        let report = canonical_local_report(report)?;
        let mut envelope = norito::json::Map::new();
        envelope.insert(
            "schema".to_owned(),
            norito::json::Value::from(LOCAL_RECEIPT_SCHEMA_V1),
        );
        envelope.insert(
            "authorization_sha256".to_owned(),
            norito::json::Value::from(self.admitted.authorization_sha256.clone()),
        );
        envelope.insert(
            "authorization_nonce".to_owned(),
            norito::json::Value::from(self.admitted.inventory.authorization_nonce.clone()),
        );
        envelope.insert(
            "receipt_name".to_owned(),
            norito::json::Value::from(name.to_owned()),
        );
        envelope.insert("report".to_owned(), report);
        let bytes = json::to_json(&norito::json::Value::Object(envelope))?.into_bytes();
        publish_private_noreplace(&self.local_receipt_root, name, &bytes)
    }
}

fn canonical_local_report(report: &norito::json::Value) -> Result<norito::json::Value> {
    let Some(source) = report.as_object() else {
        return Err(eyre!("local CLI report root must be an object"));
    };
    let mut canonical = source.clone();
    if matches!(
        source.get("command").and_then(norito::json::Value::as_str),
        Some("taira_inrou_canary" | "taira_inrou_check")
    ) {
        // A fresh liveness timestamp is intentionally not part of the durable identity. Every
        // reopen reruns the live probe; retaining it would make a crash retry conflict with an
        // otherwise identical applied child receipt.
        canonical.remove("observed_at_unix_ms");
    }
    Ok(norito::json::Value::Object(canonical))
}

fn require_forward_lease_budget(admitted: &AdmittedReset, action_secs: u64) -> Result<()> {
    let rollback_secs = admitted
        .inventory
        .timeouts
        .rollback_secs
        .checked_mul(5)
        .ok_or_else(|| eyre!("rollback lease reserve overflow"))?;
    let required_ms = action_secs
        .checked_add(rollback_secs)
        .and_then(|seconds| seconds.checked_mul(1_000))
        .and_then(|millis| millis.checked_add(30_000))
        .ok_or_else(|| eyre!("forward action lease reserve overflow"))?;
    if now_unix_ms()?.saturating_add(required_ms)
        > admitted.authorization.claims.execution_expires_at_unix_ms
    {
        return Err(eyre!(
            "signed execution lease lacks this action plus worst-case rollback reserve"
        ));
    }
    Ok(())
}

fn recovery_intent_identity_matches(
    actual: &RecoveryIntentV1,
    expected: &RecoveryIntentV1,
) -> bool {
    actual.schema == expected.schema
        && actual.step_label == expected.step_label
        && actual.mutations.len() == expected.mutations.len()
        && actual
            .mutations
            .iter()
            .zip(&expected.mutations)
            .all(|(actual, expected)| {
                actual.kind == expected.kind
                    && actual.phase == expected.phase
                    && actual.idempotency_key == expected.idempotency_key
                    && actual.receipt_name == expected.receipt_name
            })
}

fn build_recovery_intent(inventory: &InventoryV1, step: ExecutionStep) -> Option<RecoveryIntentV1> {
    let nonce = &inventory.authorization_nonce;
    let mutations = match step {
        ExecutionStep::Canary => [
            "onboarding",
            "faucet",
            "write_canary",
            "inrou_bundle_pin",
            "inrou_guest_pin",
            "inrou_canary",
        ]
        .into_iter()
        .map(|kind| recovery_child_mutation(nonce, "pre_edge", kind, None))
        .collect(),
        ExecutionStep::RestartProof => (1..=4)
            .flat_map(|wave| {
                let phase = format!("restart-wave-{wave}");
                let restart = recovery_child_mutation(
                    nonce,
                    &phase,
                    "host_restart",
                    Some(format!("host-restart-wave-{wave}.json")),
                );
                std::iter::once(restart).chain(
                    ["onboarding", "faucet", "write_canary"]
                        .into_iter()
                        .map(move |kind| recovery_child_mutation(nonce, &phase, kind, None)),
                )
            })
            .collect(),
        ExecutionStep::EdgeVerify => ["onboarding", "faucet", "write_canary"]
            .into_iter()
            .map(|kind| recovery_child_mutation(nonce, "post_edge", kind, None))
            .collect(),
        _ => return None,
    };
    Some(RecoveryIntentV1 {
        schema: super::RECOVERY_INTENT_SCHEMA_V1.to_owned(),
        step_label: step.label().to_owned(),
        next_mutation: 0,
        mutations,
    })
}

fn recovery_child_mutation(
    nonce: &str,
    phase: &str,
    kind: &str,
    receipt_name: Option<String>,
) -> RecoveryMutationV1 {
    RecoveryMutationV1 {
        kind: kind.to_owned(),
        phase: phase.to_owned(),
        idempotency_key: child_mutation_idempotency_key(nonce, phase, kind),
        receipt_name: receipt_name
            .unwrap_or_else(|| format!("{}-{}.json", kind.replace('_', "-"), phase)),
        state: RecoveryMutationStateV1::Prepared,
    }
}

impl<R: ProcessRunner> ResetTransport for OpenSshTransport<'_, R> {
    fn recovery_intent(
        &self,
        inventory: &InventoryV1,
        step: ExecutionStep,
    ) -> Result<Option<RecoveryIntentV1>> {
        Ok(build_recovery_intent(inventory, step))
    }

    fn recover_step(
        &mut self,
        inventory: &InventoryV1,
        step: ExecutionStep,
        timeout_secs: u64,
        intent: &RecoveryIntentV1,
        progress: &mut dyn RecoveryProgress,
    ) -> Result<RecoveryOutcome> {
        let expected = self
            .recovery_intent(inventory, step)?
            .ok_or_else(|| eyre!("step has no read-only recovery identity"))?;
        if !recovery_intent_identity_matches(intent, &expected) {
            return Ok(RecoveryOutcome::Rejected(
                "recovery_intent_mismatch".to_owned(),
            ));
        }
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(timeout_secs))
            .ok_or_else(|| eyre!("read-only recovery deadline overflow"))?;
        let next = usize::from(intent.next_mutation);
        if intent
            .mutations
            .iter()
            .take(next)
            .any(|mutation| mutation.state != RecoveryMutationStateV1::Applied)
        {
            return Ok(RecoveryOutcome::Rejected(
                "recovery_cursor_mismatch".to_owned(),
            ));
        }
        if next < intent.mutations.len() {
            let mutation = &intent.mutations[next];
            if mutation.state != RecoveryMutationStateV1::Submitted {
                return Ok(RecoveryOutcome::Rejected("not_attempted".to_owned()));
            }
            let outcome = match mutation.kind.as_str() {
                "host_restart" => {
                    let wave = mutation
                        .phase
                        .strip_prefix("restart-wave-")
                        .and_then(|value| value.parse::<usize>().ok())
                        .filter(|wave| (1..=4).contains(wave));
                    let Some(validator) = wave.and_then(|wave| inventory.validators.get(wave - 1))
                    else {
                        return Ok(RecoveryOutcome::Rejected(
                            "restart_validator_missing".to_owned(),
                        ));
                    };
                    match self.recover_validator_restart(validator, inventory.timeouts.restart_secs)
                    {
                        Ok(receipt) if receipt.status == "ok" => PreparedMutationOutcome::Applied {
                            value: norito::json::Value::Null,
                            evidence: Vec::new(),
                        },
                        Ok(receipt) if receipt.status == "pending" => {
                            PreparedMutationOutcome::Pending
                        }
                        Ok(receipt) if receipt.status == "rejected" => {
                            PreparedMutationOutcome::Rejected(receipt.detail)
                        }
                        Ok(_) => {
                            PreparedMutationOutcome::Rejected("restart_recovery_status".to_owned())
                        }
                        Err(_) => PreparedMutationOutcome::Pending,
                    }
                }
                "onboarding" | "faucet" | "write_canary" => {
                    self.recover_write_canary_child_until(deadline, mutation)?
                }
                "inrou_bundle_pin" | "inrou_guest_pin" | "inrou_canary" => {
                    self.recover_inrou_prepared_child_until(deadline, mutation)?
                }
                _ => {
                    return Ok(RecoveryOutcome::Rejected(
                        "recovery_mutation_kind".to_owned(),
                    ));
                }
            };
            match outcome {
                PreparedMutationOutcome::Applied { value, .. } => {
                    if !value.is_null() {
                        self.publish_local_receipt(&mutation.receipt_name, &value)?;
                    }
                    progress.mark_applied(next)?;
                }
                PreparedMutationOutcome::Pending => return Ok(RecoveryOutcome::Pending),
                PreparedMutationOutcome::Rejected(class) => {
                    return Ok(RecoveryOutcome::Rejected(class));
                }
            }
            if next + 1 < intent.mutations.len() {
                return Ok(RecoveryOutcome::Rejected("not_attempted".to_owned()));
            }
        }
        match step {
            ExecutionStep::Canary => {}
            ExecutionStep::RestartProof => {
                for wave in 1..=4 {
                    self.convergence(inventory.timeouts.convergence_secs, wave, true)?;
                    self.inrou_check_with_mode(inventory.timeouts.canary_secs, wave, true)?;
                }
                self.doctor_with_mode(inventory.timeouts.canary_secs, true)?;
            }
            ExecutionStep::EdgeVerify => {
                self.doctor_with_mode(inventory.timeouts.canary_secs, true)?;
                self.require_fresh_inrou_check(
                    "inrou-post-edge.json",
                    inventory.timeouts.canary_secs,
                    true,
                )?;
            }
            _ => {
                return Ok(RecoveryOutcome::Rejected("recovery_step_kind".to_owned()));
            }
        }
        Ok(RecoveryOutcome::Applied)
    }

    fn run_recoverable_step(
        &mut self,
        inventory: &InventoryV1,
        step: ExecutionStep,
        _timeout_secs: u64,
        intent: &RecoveryIntentV1,
        progress: &mut dyn RecoveryProgress,
    ) -> Result<()> {
        let expected = self
            .recovery_intent(inventory, step)?
            .ok_or_else(|| eyre!("step has no recoverable mutation identity"))?;
        if intent != &expected {
            return Err(eyre!("prepared mutation intent is not exact"));
        }
        match step {
            ExecutionStep::Canary => {
                for (index, kind) in ["onboarding", "faucet", "write_canary"]
                    .into_iter()
                    .enumerate()
                {
                    self.run_journaled_write_canary_child(
                        progress,
                        index,
                        inventory.timeouts.canary_secs,
                        "pre_edge",
                        kind,
                    )?;
                }
                for (offset, kind) in ["inrou_bundle_pin", "inrou_guest_pin", "inrou_canary"]
                    .into_iter()
                    .enumerate()
                {
                    self.run_journaled_inrou_prepared_child(
                        progress,
                        offset + 3,
                        inventory.timeouts.canary_secs,
                        "pre_edge",
                        kind,
                    )?;
                }
                Ok(())
            }
            ExecutionStep::RestartProof => {
                for (index, validator) in inventory.validators.iter().enumerate() {
                    let wave = index + 1;
                    let restart_index = index * 4;
                    progress.mark_submitted(restart_index)?;
                    self.bootstrap_and_dispatch_validator(
                        validator,
                        HostAction::Restart,
                        inventory.timeouts.restart_secs,
                    )?;
                    progress.mark_applied(restart_index)?;
                    let phase = format!("restart-wave-{wave}");
                    for (offset, kind) in ["onboarding", "faucet", "write_canary"]
                        .into_iter()
                        .enumerate()
                    {
                        self.run_journaled_write_canary_child(
                            progress,
                            restart_index + offset + 1,
                            inventory.timeouts.canary_secs,
                            &phase,
                            kind,
                        )?;
                    }
                    self.convergence(inventory.timeouts.convergence_secs, wave, false)?;
                    self.inrou_check(inventory.timeouts.canary_secs, wave)?;
                }
                self.doctor(inventory.timeouts.canary_secs)
            }
            ExecutionStep::EdgeVerify => {
                self.bootstrap_and_dispatch_edge(
                    &inventory.edge,
                    HostAction::EdgeVerify,
                    inventory.timeouts.edge_secs,
                )?;
                self.doctor(inventory.timeouts.canary_secs)?;
                for (index, kind) in ["onboarding", "faucet", "write_canary"]
                    .into_iter()
                    .enumerate()
                {
                    self.run_journaled_write_canary_child(
                        progress,
                        index,
                        inventory.timeouts.canary_secs,
                        "post_edge",
                        kind,
                    )?;
                }
                self.require_fresh_inrou_check(
                    "inrou-post-edge.json",
                    inventory.timeouts.canary_secs,
                    false,
                )?;
                Ok(())
            }
            _ => Err(eyre!("step is not recovery-sensitive")),
        }
    }

    fn validator_step(
        &mut self,
        _inventory: &InventoryV1,
        validator: &ValidatorV1,
        step: ExecutionStep,
        timeout_secs: u64,
    ) -> Result<()> {
        let action = match step {
            ExecutionStep::Preflight => HostAction::Preflight,
            ExecutionStep::Stage => HostAction::Stage,
            ExecutionStep::Stop => HostAction::Stop,
            ExecutionStep::Install => HostAction::Install,
            ExecutionStep::Reset => HostAction::Reset,
            ExecutionStep::Start => HostAction::Start,
            other => return Err(eyre!("validator received invalid step `{}`", other.label())),
        };
        self.bootstrap_and_dispatch_validator(validator, action, timeout_secs)?;
        Ok(())
    }

    fn cohort_step(
        &mut self,
        inventory: &InventoryV1,
        step: ExecutionStep,
        timeout_secs: u64,
    ) -> Result<()> {
        match step {
            ExecutionStep::Convergence => {
                self.doctor(timeout_secs)?;
                self.convergence(timeout_secs, 0, false)
            }
            ExecutionStep::Canary => {
                self.write_canary(timeout_secs)?;
                self.inrou_canary(timeout_secs)
            }
            ExecutionStep::RestartProof => {
                for (index, validator) in inventory.validators.iter().enumerate() {
                    let wave = index + 1;
                    self.bootstrap_and_dispatch_validator(
                        validator,
                        HostAction::Restart,
                        inventory.timeouts.restart_secs,
                    )?;
                    self.write_canary_for_phase(
                        inventory.timeouts.canary_secs,
                        &format!("restart-wave-{wave}"),
                        &format!("write-canary-restart-wave-{wave}.json"),
                    )?;
                    self.convergence(inventory.timeouts.convergence_secs, wave, false)?;
                    self.inrou_check(inventory.timeouts.canary_secs, wave)?;
                }
                self.doctor(inventory.timeouts.canary_secs)?;
                Ok(())
            }
            ExecutionStep::Seal => {
                for validator in &inventory.validators {
                    self.bootstrap_and_dispatch_validator(
                        validator,
                        HostAction::Seal,
                        timeout_secs,
                    )?;
                }
                self.bootstrap_and_dispatch_edge(&inventory.edge, HostAction::Seal, timeout_secs)?;
                Ok(())
            }
            ExecutionStep::Cleanup => {
                for validator in &inventory.validators {
                    self.bootstrap_and_dispatch_validator(
                        validator,
                        HostAction::Cleanup,
                        timeout_secs,
                    )?;
                }
                self.bootstrap_and_dispatch_edge(
                    &inventory.edge,
                    HostAction::Cleanup,
                    timeout_secs,
                )?;
                Ok(())
            }
            other => Err(eyre!("cohort received invalid step `{}`", other.label())),
        }
    }

    fn edge_step(
        &mut self,
        inventory: &InventoryV1,
        step: ExecutionStep,
        timeout_secs: u64,
    ) -> Result<()> {
        let action = match step {
            ExecutionStep::Preflight => HostAction::Preflight,
            ExecutionStep::EdgeStage => HostAction::EdgeStage,
            ExecutionStep::EdgeCutover => HostAction::EdgeCutover,
            ExecutionStep::EdgeVerify => HostAction::EdgeVerify,
            other => return Err(eyre!("edge received invalid step `{}`", other.label())),
        };
        self.bootstrap_and_dispatch_edge(&inventory.edge, action, timeout_secs)?;
        if step == ExecutionStep::EdgeVerify {
            let canary_timeout = inventory.timeouts.canary_secs;
            self.doctor(canary_timeout)?;
            self.write_canary_for_phase(
                canary_timeout,
                "post_edge",
                "write-canary-post-edge.json",
            )?;
            self.require_fresh_inrou_check("inrou-post-edge.json", canary_timeout, false)?;
        }
        Ok(())
    }

    fn rollback_validator(
        &mut self,
        _inventory: &InventoryV1,
        validator: &ValidatorV1,
        timeout_secs: u64,
    ) -> Result<()> {
        self.bootstrap_and_dispatch_validator(validator, HostAction::Rollback, timeout_secs)?;
        Ok(())
    }

    fn rollback_edge(&mut self, inventory: &InventoryV1, timeout_secs: u64) -> Result<()> {
        self.bootstrap_and_dispatch_edge(&inventory.edge, HostAction::Rollback, timeout_secs)?;
        Ok(())
    }
}

fn child_mutation_idempotency_key(nonce: &str, phase: &str, child_kind: &str) -> String {
    let mut digest = Sha256::new();
    update_frame(
        &mut digest,
        b"iroha:taira:public-reset:child-idempotency:v1\0",
    );
    update_frame(&mut digest, nonce.as_bytes());
    update_frame(&mut digest, phase.as_bytes());
    update_frame(&mut digest, child_kind.as_bytes());
    hex::encode(digest.finalize())
}

fn remaining_seconds(deadline: Instant) -> Result<u64> {
    let remaining = deadline.saturating_duration_since(Instant::now());
    let seconds = remaining.as_secs();
    if seconds == 0 {
        return Err(eyre!(
            "absolute mutation deadline has less than one whole second remaining"
        ));
    }
    Ok(seconds)
}

fn prepared_envelope_transaction_hash(bytes: &[u8]) -> Result<String> {
    let value: norito::json::Value =
        json::from_slice(bytes).wrap_err("prepared envelope is not JSON")?;
    let operation = value
        .as_object()
        .and_then(|root| root.get("operation"))
        .and_then(norito::json::Value::as_object)
        .ok_or_else(|| eyre!("prepared envelope omits its operation"))?;
    if operation.get("kind").and_then(norito::json::Value::as_str)
        == Some("onboarding_proof_required")
    {
        return Ok(String::new());
    }
    let hash = operation
        .get("envelope")
        .and_then(norito::json::Value::as_object)
        .ok_or_else(|| eyre!("prepared envelope omits its tagged operation payload"))?
        .get("transaction_hash_hex")
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("prepared envelope omits transaction_hash_hex"))?;
    require_lower_sha256(hash, "prepared envelope transaction hash")?;
    Ok(hash.to_owned())
}

const PREPARED_WRITE_REPORT_FIELDS: &[&str] = &[
    "command",
    "status",
    "public_root",
    "checks",
    "warnings",
    "failures",
    "authorization_sha256",
    "authorization_nonce",
    "mutation_kind",
    "mutation_phase",
    "idempotency_key",
    "operation",
    "transaction_hash_hex",
    "prepared_envelope_sha256",
    "prepared_envelope_size",
    "recovery_outcome",
    "applied_block_height",
    "evidence",
    "execution_expires_at_unix_ms",
];

const PREPARED_FINAL_CANARY_REPORT_FIELDS: &[&str] = &[
    "command",
    "status",
    "public_root",
    "checks",
    "warnings",
    "failures",
    "authorization_sha256",
    "authorization_nonce",
    "mutation_kind",
    "mutation_phase",
    "idempotency_key",
    "operation",
    "transaction_hash_hex",
    "prepared_envelope_sha256",
    "prepared_envelope_size",
    "recovery_outcome",
    "applied_block_height",
    "evidence",
    "execution_expires_at_unix_ms",
    "fee_payment",
    "fee_quote",
];

const PREPARED_INROU_REPORT_FIELDS: &[&str] = &[
    "command",
    "status",
    "public_root",
    "checks",
    "warnings",
    "failures",
    "authorization_sha256",
    "authorization_nonce",
    "mutation_kind",
    "mutation_phase",
    "idempotency_key",
    "operation",
    "transaction_hash_hex",
    "prepared_envelope_sha256",
    "prepared_envelope_size",
    "recovery_outcome",
    "applied_block_height",
    "evidence",
    "execution_expires_at_unix_ms",
    "fee_payment",
    "fee_quote",
    "mutation_mode",
];

const PREPARED_INROU_SERVICE_APPLIED_REPORT_FIELDS: &[&str] = &[
    "command",
    "status",
    "public_root",
    "checks",
    "warnings",
    "failures",
    "authorization_sha256",
    "authorization_nonce",
    "mutation_kind",
    "mutation_phase",
    "idempotency_key",
    "operation",
    "transaction_hash_hex",
    "prepared_envelope_sha256",
    "prepared_envelope_size",
    "recovery_outcome",
    "applied_block_height",
    "evidence",
    "execution_expires_at_unix_ms",
    "fee_payment",
    "fee_quote",
    "mutation_mode",
    "service_name",
    "service_version",
    "route_host",
    "route_path",
    "active_host_adverts",
    "hosted_replica_count",
    "bundle_hash",
    "bundle_content_cid",
    "bundle_manifest_digest_hex",
    "guest_content_cid",
    "guest_manifest_digest_hex",
    "container_manifest_hash",
    "service_manifest_hash",
    "observed_at_unix_ms",
    "replica_identities",
];

const PREPARED_INROU_REJECTED_EVIDENCE: &[&str] = &[
    "Rejected",
    "Expired",
    "ExecutionExpiredBeforeSubmit",
    "SubmittedHashMismatch",
];

fn require_exact_json_fields(
    object: &norito::json::Map,
    expected: &[&str],
    label: &str,
) -> Result<()> {
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        let missing = expected.difference(&actual).copied().collect::<Vec<_>>();
        let unexpected = actual.difference(&expected).copied().collect::<Vec<_>>();
        return Err(eyre!(
            "{label} is outside its exact V1 field set (missing: {missing:?}; unexpected: {unexpected:?})"
        ));
    }
    Ok(())
}

fn nullable_report_string<'a>(
    object: &'a norito::json::Map,
    name: &str,
    label: &str,
) -> Result<Option<&'a str>> {
    match object.get(name) {
        Some(norito::json::Value::Null) => Ok(None),
        Some(norito::json::Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(eyre!("{label} field `{name}` must be a string or null")),
        None => Err(eyre!("{label} omits required nullable field `{name}`")),
    }
}

fn nullable_report_u64(object: &norito::json::Map, name: &str, label: &str) -> Result<Option<u64>> {
    match object.get(name) {
        Some(norito::json::Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or_else(|| eyre!("{label} field `{name}` must be an unsigned integer or null")),
        None => Err(eyre!("{label} omits required nullable field `{name}`")),
    }
}

fn required_report_evidence<'a>(value: &'a norito::json::Value, label: &str) -> Result<&'a str> {
    value
        .as_object()
        .and_then(|object| object.get("evidence"))
        .and_then(norito::json::Value::as_str)
        .ok_or_else(|| eyre!("{label} omits required terminal evidence"))
}

fn canonical_json_report_bytes(value: &norito::json::Value) -> Result<Vec<u8>> {
    let mut bytes = json::to_json(value)
        .wrap_err("failed to canonicalize validated child report")?
        .into_bytes();
    bytes.push(b'\n');
    if bytes.len() > MAX_PROCESS_OUTPUT {
        return Err(eyre!("canonical child report exceeds its V1 byte bound"));
    }
    Ok(bytes)
}

fn validate_report_evidence_token(value: &str, allowed: &[&str], label: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > MAX_REPORT_EVIDENCE_TOKEN_BYTES
        || !value.bytes().all(|byte| byte.is_ascii_alphanumeric())
        || !allowed.contains(&value)
    {
        return Err(eyre!("{label} is not an allowed exact V1 evidence class"));
    }
    Ok(())
}

fn require_empty_report_array(object: &norito::json::Map, field: &str, label: &str) -> Result<()> {
    if !object
        .get(field)
        .and_then(norito::json::Value::as_array)
        .is_some_and(Vec::is_empty)
    {
        return Err(eyre!(
            "{label} field `{field}` must be an explicit empty array"
        ));
    }
    Ok(())
}

fn validate_prepared_report_common_arrays(
    object: &norito::json::Map,
    service_applied: bool,
    label: &str,
) -> Result<()> {
    require_empty_report_array(object, "warnings", label)?;
    require_empty_report_array(object, "failures", label)?;
    if !service_applied {
        return require_empty_report_array(object, "checks", label);
    }
    let checks = object
        .get("checks")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("{label} field `checks` must be an array"))?;
    let expected = [
        (
            "inrou_authoritative_status",
            "active_adverts=4, hosted_replicas=4",
        ),
        (
            "inrou_public_routes",
            "observed deterministic identities for replica slots 1, 2, 3, and 4",
        ),
    ];
    if checks.len() != expected.len() {
        return Err(eyre!(
            "{label} must contain the exact two successful Inrou checks"
        ));
    }
    for (check, (name, detail)) in checks.iter().zip(expected) {
        let check = check
            .as_object()
            .ok_or_else(|| eyre!("{label} check must be an object"))?;
        require_exact_json_fields(check, &["name", "http_status", "ok", "detail"], label)?;
        if check.get("name").and_then(norito::json::Value::as_str) != Some(name)
            || check
                .get("http_status")
                .and_then(norito::json::Value::as_u64)
                != Some(200)
            || check.get("ok").and_then(norito::json::Value::as_bool) != Some(true)
            || check.get("detail").and_then(norito::json::Value::as_str) != Some(detail)
        {
            return Err(eyre!(
                "{label} does not carry the exact successful Inrou check evidence"
            ));
        }
    }
    Ok(())
}

fn validate_prepared_report_envelope_bytes(
    value: &norito::json::Value,
    bytes: &[u8],
) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("prepared report root must be an object"))?;
    let size = u64::try_from(bytes.len()).wrap_err("prepared envelope size exceeds u64")?;
    let transaction_hash = prepared_envelope_transaction_hash(bytes)?;
    let transaction_matches = if transaction_hash.is_empty() {
        object.get("transaction_hash_hex") == Some(&norito::json::Value::Null)
    } else {
        object
            .get("transaction_hash_hex")
            .and_then(norito::json::Value::as_str)
            == Some(transaction_hash.as_str())
    };
    if object
        .get("prepared_envelope_sha256")
        .and_then(norito::json::Value::as_str)
        != Some(sha256_hex(bytes).as_str())
        || object
            .get("prepared_envelope_size")
            .and_then(norito::json::Value::as_u64)
            != Some(size)
        || !transaction_matches
    {
        return Err(eyre!(
            "prepared report does not bind the exact canonical envelope bytes"
        ));
    }
    let envelope: norito::json::Value =
        json::from_slice(bytes).wrap_err("prepared report envelope is not JSON")?;
    let operation = envelope
        .as_object()
        .and_then(|root| root.get("operation"))
        .and_then(norito::json::Value::as_object)
        .and_then(|operation| operation.get("envelope"))
        .and_then(norito::json::Value::as_object)
        .ok_or_else(|| eyre!("prepared report envelope omits its operation payload"))?;
    for field in ["fee_payment", "fee_quote"] {
        if let Some(reported) = object.get(field)
            && operation.get(field) != Some(reported)
        {
            return Err(eyre!(
                "prepared report `{field}` differs from the retained envelope"
            ));
        }
    }
    if transaction_hash.is_empty()
        && matches!(
            object
                .get("recovery_outcome")
                .and_then(norito::json::Value::as_str),
            Some("ProofRequired" | "Applied")
        )
    {
        let semantic = prepared_onboarding_proof_required_result(bytes)?.semantic_hash_hex;
        if object.get("evidence").and_then(norito::json::Value::as_str) != Some(semantic.as_str()) {
            return Err(eyre!(
                "proof-required prepared report does not bind its semantic evidence"
            ));
        }
    }
    Ok(())
}

fn validate_prepared_write_report(
    value: &norito::json::Value,
    admitted: &AdmittedReset,
    phase: &str,
    kind: &str,
    idempotency_key: &str,
    expected_outcome: &str,
    retained: Option<&RetainedPreparedMutation>,
) -> Result<()> {
    const PENDING_EVIDENCE: &[&str] = &[
        "Absent",
        "AcceptedNotVisible",
        "OnboardingAliasConflict",
        "OnboardingStateAbsent",
        "Queued",
        "Approved",
        "Committed",
        "Applied",
        "Rejected",
        "Expired",
    ];
    const REJECTED_EVIDENCE: &[&str] = &["Rejected", "Expired"];
    validate_common_report(
        value,
        "taira_write_canary",
        &admitted.inventory.inrou_canary.public_root,
    )?;
    let object = value.as_object().expect("common report checked object");
    require_exact_json_fields(
        object,
        if kind == "write_canary" {
            PREPARED_FINAL_CANARY_REPORT_FIELDS
        } else {
            PREPARED_WRITE_REPORT_FIELDS
        },
        "prepared write report",
    )?;
    validate_prepared_report_common_arrays(object, false, "prepared write report")?;
    let string = |name: &str| {
        object
            .get(name)
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("prepared write report omits `{name}`"))
    };
    let expected_operation = match kind {
        "onboarding" => "onboarding",
        "faucet" => "faucet",
        "write_canary" => "final_canary",
        _ => return Err(eyre!("prepared write report has an invalid child kind")),
    };
    if !matches!(
        expected_outcome,
        "Prepared" | "ProofRequired" | "Applied" | "Pending" | "Rejected"
    ) || (retained.is_none() != matches!(expected_outcome, "Prepared" | "ProofRequired"))
        || (expected_outcome == "ProofRequired" && kind != "onboarding")
    {
        return Err(eyre!(
            "prepared write report outcome is outside the exact V1 transition"
        ));
    }
    if string("authorization_sha256")? != admitted.authorization_sha256
        || string("authorization_nonce")? != admitted.inventory.authorization_nonce
        || string("mutation_kind")? != kind
        || string("mutation_phase")? != phase
        || string("idempotency_key")? != idempotency_key
        || string("operation")? != expected_operation
        || string("recovery_outcome")? != expected_outcome
        || object
            .get("execution_expires_at_unix_ms")
            .and_then(norito::json::Value::as_u64)
            != Some(admitted.authorization.claims.execution_expires_at_unix_ms)
    {
        return Err(eyre!(
            "prepared write report does not bind its exact authorization child"
        ));
    }
    require_lower_sha256(
        string("prepared_envelope_sha256")?,
        "prepared write envelope digest",
    )?;
    let reported_size = object
        .get("prepared_envelope_size")
        .and_then(norito::json::Value::as_u64)
        .filter(|size| {
            *size > 0 && *size <= u64::try_from(MAX_PREPARED_ENVELOPE_BYTES).expect("bounded")
        })
        .ok_or_else(|| eyre!("prepared write report has an invalid envelope size"))?;
    let proof_required = kind == "onboarding"
        && (expected_outcome == "ProofRequired"
            || retained.is_some_and(|value| value.transaction_hash.is_empty()));
    let transaction_hash =
        nullable_report_string(object, "transaction_hash_hex", "prepared write report")?;
    if proof_required {
        if transaction_hash.is_some() {
            return Err(eyre!(
                "proof-required prepared write report must carry an explicit null transaction hash"
            ));
        }
    } else {
        require_lower_sha256(
            transaction_hash
                .ok_or_else(|| eyre!("prepared write report omits its transaction hash"))?,
            "prepared write transaction hash",
        )?;
    }
    let applied_height =
        nullable_report_u64(object, "applied_block_height", "prepared write report")?;
    let evidence = nullable_report_string(object, "evidence", "prepared write report")?;
    match expected_outcome {
        "Prepared" if applied_height.is_none() && evidence.is_none() => {}
        "ProofRequired" if applied_height.is_none() => require_lower_sha256(
            evidence.ok_or_else(|| eyre!("proof-required write report omits evidence"))?,
            "proof-required semantic evidence",
        )?,
        "Applied" if proof_required && applied_height.is_none() => require_lower_sha256(
            evidence.ok_or_else(|| eyre!("proof-required Applied report omits evidence"))?,
            "proof-required semantic evidence",
        )?,
        "Applied" if applied_height.is_some_and(|height| height > 0) => require_lower_sha256(
            evidence.ok_or_else(|| eyre!("Applied write report omits committed evidence"))?,
            "prepared write committed evidence",
        )?,
        "Pending" if applied_height.is_none() => validate_report_evidence_token(
            evidence.ok_or_else(|| eyre!("Pending write report omits its evidence class"))?,
            PENDING_EVIDENCE,
            "prepared write Pending evidence",
        )?,
        "Rejected" if applied_height.is_none() => validate_report_evidence_token(
            evidence.ok_or_else(|| eyre!("Rejected write report omits its evidence class"))?,
            REJECTED_EVIDENCE,
            "prepared write Rejected evidence",
        )?,
        _ => {
            return Err(eyre!(
                "prepared write report height/evidence does not match its outcome"
            ));
        }
    }
    if let Some(retained) = retained {
        if reported_size != u64::try_from(retained.bytes.len()).expect("bounded")
            || string("prepared_envelope_sha256")? != retained.sha256
            || (!retained.transaction_hash.is_empty()
                && transaction_hash != Some(retained.transaction_hash.as_str()))
        {
            return Err(eyre!(
                "prepared write report does not bind the retained shared envelope"
            ));
        }
        validate_prepared_report_envelope_bytes(value, &retained.bytes)?;
    }
    Ok(())
}

fn validate_prepared_inrou_report(
    value: &norito::json::Value,
    admitted: &AdmittedReset,
    phase: &str,
    kind: &str,
    idempotency_key: &str,
    expected_outcome: &str,
    retained: Option<&RetainedPreparedMutation>,
) -> Result<()> {
    const PENDING_EVIDENCE: &[&str] = &[
        "Absent",
        "ObservationUnavailable",
        "Queued",
        "Approved",
        "Committed",
        "Applied",
        "Rejected",
        "Expired",
    ];
    validate_common_report(
        value,
        "taira_inrou_canary",
        &admitted.inventory.inrou_canary.public_root,
    )?;
    let object = value.as_object().expect("common report checked object");
    let service_applied = kind == "inrou_canary" && expected_outcome == "Applied";
    require_exact_json_fields(
        object,
        if service_applied {
            PREPARED_INROU_SERVICE_APPLIED_REPORT_FIELDS
        } else {
            PREPARED_INROU_REPORT_FIELDS
        },
        "prepared Inrou report",
    )?;
    validate_prepared_report_common_arrays(object, service_applied, "prepared Inrou report")?;
    let string = |name: &str| {
        object
            .get(name)
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("prepared Inrou report omits `{name}`"))
    };
    let expected_operation = match kind {
        "inrou_bundle_pin" => "bundle_pin",
        "inrou_guest_pin" => "guest_pin",
        "inrou_canary" => "service_mutation",
        _ => return Err(eyre!("prepared Inrou report has an invalid child kind")),
    };
    if !matches!(
        expected_outcome,
        "Prepared" | "Applied" | "Pending" | "Rejected"
    ) || (retained.is_none() != (expected_outcome == "Prepared"))
    {
        return Err(eyre!(
            "prepared Inrou report outcome is outside the exact V1 transition"
        ));
    }
    if string("authorization_sha256")? != admitted.authorization_sha256
        || string("authorization_nonce")? != admitted.inventory.authorization_nonce
        || string("mutation_kind")? != kind
        || string("mutation_phase")? != phase
        || string("idempotency_key")? != idempotency_key
        || string("operation")? != expected_operation
        || string("mutation_mode")? != "deploy"
        || string("recovery_outcome")? != expected_outcome
        || object
            .get("execution_expires_at_unix_ms")
            .and_then(norito::json::Value::as_u64)
            != Some(admitted.authorization.claims.execution_expires_at_unix_ms)
    {
        return Err(eyre!(
            "prepared Inrou report does not bind its exact authorization child"
        ));
    }
    require_lower_sha256(
        string("transaction_hash_hex")?,
        "prepared Inrou transaction hash",
    )?;
    require_lower_sha256(
        string("prepared_envelope_sha256")?,
        "prepared Inrou envelope digest",
    )?;
    let reported_size = object
        .get("prepared_envelope_size")
        .and_then(norito::json::Value::as_u64)
        .filter(|size| {
            *size > 0 && *size <= u64::try_from(MAX_PREPARED_ENVELOPE_BYTES).expect("bounded")
        })
        .ok_or_else(|| eyre!("prepared Inrou report has an invalid envelope size"))?;
    if !object
        .get("fee_payment")
        .is_some_and(norito::json::Value::is_object)
        || !object
            .get("fee_quote")
            .is_some_and(norito::json::Value::is_object)
    {
        return Err(eyre!(
            "prepared Inrou report must carry its exact fee payment and quote objects"
        ));
    }
    if let Some(retained) = retained {
        if reported_size != u64::try_from(retained.bytes.len()).expect("bounded")
            || retained.transaction_hash.is_empty()
            || string("transaction_hash_hex")? != retained.transaction_hash
            || string("prepared_envelope_sha256")? != retained.sha256
        {
            return Err(eyre!(
                "prepared Inrou report does not bind the retained shared envelope"
            ));
        }
        validate_prepared_report_envelope_bytes(value, &retained.bytes)?;
    }
    let applied_height =
        nullable_report_u64(object, "applied_block_height", "prepared Inrou report")?;
    let evidence = nullable_report_string(object, "evidence", "prepared Inrou report")?;
    match expected_outcome {
        "Prepared" if applied_height.is_none() && evidence.is_none() => {}
        "Applied" if applied_height.is_some_and(|height| height > 0) => {
            let evidence =
                evidence.ok_or_else(|| eyre!("Applied Inrou report omits committed evidence"))?;
            require_lower_sha256(evidence, "prepared Inrou committed evidence")?;
            if retained.is_none_or(|retained| evidence != retained.transaction_hash) {
                return Err(eyre!(
                    "Applied Inrou report evidence differs from the retained transaction"
                ));
            }
        }
        "Pending" if applied_height.is_none() => validate_report_evidence_token(
            evidence.ok_or_else(|| eyre!("Pending Inrou report omits its evidence class"))?,
            PENDING_EVIDENCE,
            "prepared Inrou Pending evidence",
        )?,
        "Rejected" if applied_height.is_none() => validate_report_evidence_token(
            evidence.ok_or_else(|| eyre!("Rejected Inrou report omits its evidence class"))?,
            PREPARED_INROU_REJECTED_EVIDENCE,
            "prepared Inrou Rejected evidence",
        )?,
        _ => {
            return Err(eyre!(
                "prepared Inrou report height/evidence does not match its outcome"
            ));
        }
    }
    if service_applied {
        validate_applied_inrou_service_identity(object, &admitted.inventory)?;
        if !object
            .get("observed_at_unix_ms")
            .and_then(norito::json::Value::as_u64)
            .is_some_and(|timestamp| timestamp > 0)
        {
            return Err(eyre!(
                "prepared Inrou service report omits its observation timestamp"
            ));
        }
    }
    Ok(())
}

fn validate_applied_inrou_service_identity(
    object: &norito::json::Map,
    inventory: &InventoryV1,
) -> Result<()> {
    let canary = &inventory.inrou_canary;
    let route_path = format!("{}{}", canary.route_path_prefix, canary.healthcheck_path);
    for (field, expected) in [
        ("service_name", canary.service_name.as_str()),
        ("service_version", canary.service_version.as_str()),
        ("route_host", canary.route_host.as_str()),
        ("route_path", route_path.as_str()),
        ("bundle_hash", canary.bundle_hash.as_str()),
        ("bundle_content_cid", canary.bundle_content_cid.as_str()),
        (
            "bundle_manifest_digest_hex",
            canary.bundle_manifest_digest_hex.as_str(),
        ),
        ("guest_content_cid", canary.guest_content_cid.as_str()),
        (
            "guest_manifest_digest_hex",
            canary.guest_manifest_digest_hex.as_str(),
        ),
        (
            "container_manifest_hash",
            canary.container_manifest_hash.as_str(),
        ),
        (
            "service_manifest_hash",
            canary.service_manifest_hash.as_str(),
        ),
    ] {
        if object.get(field).and_then(norito::json::Value::as_str) != Some(expected) {
            return Err(eyre!(
                "prepared Inrou service report differs from retained stage field `{field}`"
            ));
        }
    }
    if object
        .get("active_host_adverts")
        .and_then(norito::json::Value::as_u64)
        != Some(4)
        || object
            .get("hosted_replica_count")
            .and_then(norito::json::Value::as_u64)
            != Some(4)
    {
        return Err(eyre!(
            "prepared Inrou service report lacks the exact four-replica posture"
        ));
    }
    let replicas = object
        .get("replica_identities")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("prepared Inrou service report omits replica identities"))?;
    if replicas.len() != 4 {
        return Err(eyre!(
            "prepared Inrou service report must contain four replica identities"
        ));
    }
    for (index, replica) in replicas.iter().enumerate() {
        let replica = replica
            .as_object()
            .ok_or_else(|| eyre!("prepared Inrou replica identity must be an object"))?;
        require_exact_json_fields(
            replica,
            &["replica_slot", "identity", "response_sha256"],
            "prepared Inrou replica identity",
        )?;
        let slot = u64::try_from(index + 1).expect("bounded replica slot");
        let identity = format!("{}:replica:{slot}", canary.service_name);
        let response = replica
            .get("response_sha256")
            .and_then(norito::json::Value::as_str)
            .ok_or_else(|| eyre!("prepared Inrou replica response hash is missing"))?;
        if replica
            .get("replica_slot")
            .and_then(norito::json::Value::as_u64)
            != Some(slot)
            || replica
                .get("identity")
                .and_then(norito::json::Value::as_str)
                != Some(identity.as_str())
            || require_lower_sha256(response, "prepared Inrou replica response hash").is_err()
        {
            return Err(eyre!(
                "prepared Inrou replica identities are not exact slots 1 through 4"
            ));
        }
    }
    Ok(())
}

fn parse_json_report(bytes: &[u8], label: &str) -> Result<norito::json::Value> {
    if bytes.len() > MAX_PROCESS_OUTPUT {
        return Err(eyre!("{label} exceeded the report byte bound"));
    }
    json::from_slice(bytes).wrap_err_with(|| format!("{label} did not emit exact JSON"))
}

fn validate_common_report(
    value: &norito::json::Value,
    command: &str,
    public_root: &str,
) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("local CLI report root must be an object"))?;
    let failures_empty = object
        .get("failures")
        .and_then(norito::json::Value::as_array)
        .is_some_and(Vec::is_empty);
    if object.get("command").and_then(norito::json::Value::as_str) != Some(command)
        || object.get("status").and_then(norito::json::Value::as_str) != Some("ok")
        || object
            .get("public_root")
            .and_then(norito::json::Value::as_str)
            != Some(public_root)
        || !failures_empty
    {
        return Err(eyre!(
            "local CLI report failed its exact command/status/root/failures contract"
        ));
    }
    Ok(())
}

const DOCTOR_EXPECTED_CHECKS: &[(&str, u64, Option<&str>)] = &[
    ("status", 200, None),
    ("time_now", 200, None),
    (
        "sumeragi_status",
        401,
        Some("mounted route is expected to return HTTP 401 for this preflight shape"),
    ),
    (
        "pipeline_transaction_status",
        400,
        Some("mounted route is expected to return HTTP 400 for this preflight shape"),
    ),
    ("sccp_capabilities", 200, None),
    ("zk_proofs_count", 200, None),
    ("public_lane_validators", 200, None),
    (
        "contracts_state",
        400,
        Some("mounted route is expected to return HTTP 400 for this preflight shape"),
    ),
    (
        "musubi_ordered_prefix",
        401,
        Some("mounted route is expected to return HTTP 401 for this preflight shape"),
    ),
    (
        "soracloud_status",
        401,
        Some("mounted route is expected to return HTTP 401 for this preflight shape"),
    ),
    ("mcp_get", 200, None),
    ("mcp_initialize", 200, None),
    ("mcp_tools_list", 200, None),
    (
        "mcp_required_tools",
        200,
        Some("all required curated tools are present"),
    ),
];

fn validate_doctor_report(value: &norito::json::Value, public_root: &str) -> Result<()> {
    validate_common_report(value, "taira_doctor", public_root)?;
    let object = value.as_object().expect("common report checked object");
    require_exact_json_fields(
        object,
        &[
            "command",
            "status",
            "public_root",
            "checks",
            "warnings",
            "failures",
        ],
        "Taira doctor report",
    )?;
    require_empty_report_array(object, "failures", "Taira doctor report")?;
    let warnings = object
        .get("warnings")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("Taira doctor warnings must be an array"))?;
    if warnings.len() > 32
        || warnings.iter().any(|warning| {
            warning.as_str().is_none_or(|warning| {
                warning.is_empty()
                    || warning.len() > 1_024
                    || warning.bytes().any(|byte| byte.is_ascii_control())
            })
        })
    {
        return Err(eyre!(
            "Taira doctor warnings are outside the exact V1 bound"
        ));
    }
    let checks = object
        .get("checks")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("Taira doctor checks must be an array"))?;
    if checks.len() != DOCTOR_EXPECTED_CHECKS.len() {
        return Err(eyre!(
            "Taira doctor report must contain exactly {} checks",
            DOCTOR_EXPECTED_CHECKS.len()
        ));
    }
    for (check, &(name, http_status, detail)) in checks.iter().zip(DOCTOR_EXPECTED_CHECKS) {
        let check = check
            .as_object()
            .ok_or_else(|| eyre!("Taira doctor check must be an object"))?;
        require_exact_json_fields(
            check,
            if detail.is_some() {
                &["name", "http_status", "ok", "detail"]
            } else {
                &["name", "http_status", "ok"]
            },
            "Taira doctor check",
        )?;
        if check.get("name").and_then(norito::json::Value::as_str) != Some(name)
            || check
                .get("http_status")
                .and_then(norito::json::Value::as_u64)
                != Some(http_status)
            || check.get("ok").and_then(norito::json::Value::as_bool) != Some(true)
            || detail.is_some_and(|detail| {
                check.get("detail").and_then(norito::json::Value::as_str) != Some(detail)
            })
        {
            return Err(eyre!("Taira doctor check `{name}` is not exact V1"));
        }
    }
    Ok(())
}

fn require_fresh_liveness_report<C>(
    context: &mut C,
    had_prior_receipt: bool,
    run: impl FnOnce(&mut C) -> Result<norito::json::Value>,
    validate: impl FnOnce(&norito::json::Value, &mut C) -> Result<()>,
    publish: impl FnOnce(&mut C, &norito::json::Value) -> Result<()>,
) -> Result<()> {
    let value = run(context)?;
    validate(&value, context)?;
    if !had_prior_receipt {
        publish(context, &value)?;
    }
    Ok(())
}

const FRESH_INROU_CHECK_REPORT_FIELDS: &[&str] = &[
    "command",
    "status",
    "public_root",
    "checks",
    "warnings",
    "failures",
    "service_name",
    "service_version",
    "route_host",
    "route_path",
    "active_host_adverts",
    "hosted_replica_count",
    "bundle_hash",
    "bundle_content_cid",
    "bundle_manifest_digest_hex",
    "guest_content_cid",
    "guest_manifest_digest_hex",
    "container_manifest_hash",
    "service_manifest_hash",
    "observed_at_unix_ms",
    "replica_identities",
];

const RETAINED_INROU_CHECK_REPORT_FIELDS: &[&str] = &[
    "command",
    "status",
    "public_root",
    "checks",
    "warnings",
    "failures",
    "service_name",
    "service_version",
    "route_host",
    "route_path",
    "active_host_adverts",
    "hosted_replica_count",
    "bundle_hash",
    "bundle_content_cid",
    "bundle_manifest_digest_hex",
    "guest_content_cid",
    "guest_manifest_digest_hex",
    "container_manifest_hash",
    "service_manifest_hash",
    "replica_identities",
];

fn validate_fresh_inrou_check_report(
    value: &norito::json::Value,
    admitted: &AdmittedReset,
) -> Result<()> {
    validate_exact_inrou_check_report(value, admitted, true)
}

fn validate_retained_inrou_check_report(
    value: &norito::json::Value,
    admitted: &AdmittedReset,
) -> Result<()> {
    validate_exact_inrou_check_report(value, admitted, false)
}

fn validate_exact_inrou_check_report(
    value: &norito::json::Value,
    admitted: &AdmittedReset,
    fresh: bool,
) -> Result<()> {
    let canary = &admitted.inventory.inrou_canary;
    validate_common_report(value, "taira_inrou_check", &canary.public_root)?;
    let object = value.as_object().expect("common report checked object");
    require_exact_json_fields(
        object,
        if fresh {
            FRESH_INROU_CHECK_REPORT_FIELDS
        } else {
            RETAINED_INROU_CHECK_REPORT_FIELDS
        },
        if fresh {
            "fresh Inrou check report"
        } else {
            "retained Inrou check report"
        },
    )?;
    validate_prepared_report_common_arrays(object, true, "Inrou check report")?;
    validate_applied_inrou_service_identity(object, &admitted.inventory)?;
    if fresh
        && !object
            .get("observed_at_unix_ms")
            .and_then(norito::json::Value::as_u64)
            .is_some_and(|timestamp| timestamp > 0)
    {
        return Err(eyre!(
            "fresh Inrou check report omits its observation timestamp"
        ));
    }
    Ok(())
}

/// Decode and validate one exact first-release Sumeragi V2 status report.
///
/// The typed JSON contract requires every current field, including explicit
/// `null` for absent optional evidence. The public-reset coordinator therefore
/// never accepts an older sparse status projection as convergence evidence.
fn validate_convergence_status(
    value: &norito::json::Value,
    expected: &ValidatorV1,
) -> Result<(u64, String, String, String)> {
    let status: SumeragiV2Status = json::from_value(value.clone())
        .wrap_err("Sumeragi status is not exact canonical V1 JSON")?;
    status
        .validate()
        .map_err(|error| eyre!("Sumeragi status invariants failed: {error:?}"))?;
    let expected_node = expected
        .node_fingerprint
        .parse::<iroha_crypto::Hash>()
        .wrap_err("signed node fingerprint is invalid")?;
    let expected_build = expected
        .build_fingerprint
        .parse::<iroha_crypto::Hash>()
        .wrap_err("signed build fingerprint is invalid")?;
    let expected_config = expected
        .config_fingerprint
        .parse::<iroha_crypto::Hash>()
        .wrap_err("signed config fingerprint is invalid")?;
    if status.protocol_version != 4
        || status.restart_required
        || status.node_fingerprint != expected_node
        || status.build_fingerprint != expected_build
        || status.config_fingerprint != expected_config
        || status.height_context.validator_count != 4
        || status.height_context.quorum.min_signers != 3
        || status.height_context.quorum.total_power != 4
        || status.last_committed_height == 0
    {
        return Err(eyre!(
            "Sumeragi status differs from the signed four-validator runtime identity"
        ));
    }
    let subject = status
        .last_committed_subject
        .ok_or_else(|| eyre!("Sumeragi status omits its committed block subject"))?;
    let commit = status
        .last_commit_qc
        .ok_or_else(|| eyre!("Sumeragi status omits its authenticated CommitQC"))?;
    if commit.validator_count != 4
        || commit.signer_count != 3
        || commit.min_signers != 3
        || commit.signed_power != 3
        || commit.total_power != 4
        || commit.certificate.subject != subject
        || commit.certificate.round.height != status.last_committed_height
        || commit.certificate.proposal_round.context_id != commit.certificate.round.context_id
    {
        return Err(eyre!(
            "Sumeragi CommitQC is not the exact authenticated 3-of-4 committed checkpoint"
        ));
    }
    let context = json::to_value(&commit.certificate.round.context_id)?
        .as_str()
        .ok_or_else(|| eyre!("CommitQC context identifier is not canonical JSON"))?
        .to_owned();
    let block = json::to_value(&subject.block_hash)?
        .as_str()
        .ok_or_else(|| eyre!("committed block hash is not canonical JSON"))?
        .to_owned();
    let commit = json::to_json(&commit).wrap_err("failed to canonicalize CommitQC evidence")?;
    Ok((status.last_committed_height, context, block, commit))
}

fn validate_convergence_wave(
    value: &norito::json::Value,
    expected_wave: usize,
    inventory: &InventoryV1,
) -> Result<(u64, String, String, String)> {
    let object = value
        .as_object()
        .ok_or_else(|| eyre!("convergence-wave receipt must be an object"))?;
    if object.get("schema").and_then(norito::json::Value::as_str)
        != Some("iroha.taira.public-reset.convergence-wave.v1")
        || object.get("wave").and_then(norito::json::Value::as_u64)
            != Some(u64::try_from(expected_wave).expect("bounded wave"))
    {
        return Err(eyre!("convergence-wave receipt identity is not exact"));
    }
    let reports = object
        .get("validator_reports")
        .and_then(norito::json::Value::as_array)
        .ok_or_else(|| eyre!("convergence-wave receipt omits validator reports"))?;
    if reports.len() != 4 {
        return Err(eyre!("convergence-wave receipt must contain four reports"));
    }
    let mut common = None;
    for (report, validator) in reports.iter().zip(&inventory.validators) {
        let observed = validate_convergence_status(report, validator)?;
        match &common {
            None => common = Some(observed),
            Some(expected) if expected == &observed => {}
            Some(_) => return Err(eyre!("convergence-wave validator reports disagree")),
        }
    }
    let checkpoint = common.expect("four reports yield a checkpoint");
    if object.get("height").and_then(norito::json::Value::as_u64) != Some(checkpoint.0)
        || object
            .get("height_context_id")
            .and_then(norito::json::Value::as_str)
            != Some(checkpoint.1.as_str())
        || object
            .get("block_hash")
            .and_then(norito::json::Value::as_str)
            != Some(checkpoint.2.as_str())
        || object
            .get("last_commit_qc")
            .map(json::to_json)
            .transpose()?
            .as_deref()
            != Some(checkpoint.3.as_str())
    {
        return Err(eyre!(
            "convergence-wave summary differs from its four reports"
        ));
    }
    Ok(checkpoint)
}

fn require_successor_checkpoint(
    previous: Option<&(u64, String, String, String)>,
    current: &(u64, String, String, String),
) -> Result<()> {
    if current.0 == 0 || previous.is_some_and(|previous| current.0 <= previous.0) {
        return Err(eyre!(
            "restart convergence did not prove a strictly successor committed height"
        ));
    }
    Ok(())
}

fn validate_receipt_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 128
        || !name.ends_with(".json")
        || !name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'.')
        })
    {
        return Err(eyre!("receipt name escaped the closed local namespace"));
    }
    Ok(())
}

fn publish_private_noreplace(directory: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    validate_owner_private_dir(directory, "local receipt directory")?;
    let destination = directory.join(name);
    if destination.exists() {
        let actual = fs::read(&destination).wrap_err("failed to read existing local receipt")?;
        return if actual == bytes {
            sync_private_regular(&destination, "published local receipt")?;
            sync_directory(directory)?;
            Ok(())
        } else {
            Err(eyre!(
                "local receipt destination already exists with different bytes"
            ))
        };
    }
    let staging_name = format!(".{name}.next");
    let staging = directory.join(&staging_name);
    let create_staging = if staging.exists() {
        let actual = fs::read(&staging).wrap_err("failed to read stale local receipt staging")?;
        if actual == bytes {
            sync_private_regular(&staging, "staged local receipt")?;
            false
        } else if actual.len() < bytes.len() {
            fs::remove_file(&staging)?;
            sync_directory(directory)?;
            true
        } else {
            return Err(eyre!(
                "stale local receipt staging differs from idempotent retry"
            ));
        }
    } else {
        true
    };
    if create_staging {
        #[cfg(unix)]
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&staging)?;
        #[cfg(not(unix))]
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    let parent = File::open(directory)?;
    match rustix::fs::renameat_with(
        &parent,
        OsStr::new(&staging_name),
        &parent,
        OsStr::new(name),
        rustix::fs::RenameFlags::NOREPLACE,
    ) {
        Ok(()) => sync_directory(directory),
        Err(error) if destination.exists() => {
            let actual = fs::read(&destination)?;
            if actual == bytes {
                sync_private_regular(&destination, "published local receipt")?;
                let _ = fs::remove_file(&staging);
                sync_directory(directory)
            } else {
                Err(eyre!(error).wrap_err("local receipt no-replace race produced different bytes"))
            }
        }
        Err(error) => {
            Err(eyre!(error).wrap_err("failed to publish local receipt without replacement"))
        }
    }
}

fn sync_private_regular(path: &Path, label: &str) -> Result<()> {
    let (file, snapshot) = open_pinned_regular(path, label)?;
    super::require_owner_private_snapshot(&snapshot, label)?;
    file.sync_all()?;
    ensure_pinned_unchanged(path, label, &file, &snapshot)
}

fn validate_shared_cli(admitted: &AdmittedReset) -> Result<()> {
    let mut expected: Option<(&str, u64)> = None;
    for artifacts in admitted
        .inventory
        .validators
        .iter()
        .map(|validator| validator.artifacts.as_slice())
        .chain(std::iter::once(
            admitted.inventory.edge.artifacts.as_slice(),
        ))
    {
        let cli = artifact(artifacts, "iroha_cli")?;
        match expected {
            None => expected = Some((&cli.sha256, cli.size)),
            Some((sha256, size)) if sha256 == cli.sha256 && size == cli.size => {}
            Some(_) => {
                return Err(eyre!(
                    "every host must use one byte-identical same-revision CLI dispatcher"
                ));
            }
        }
    }
    Ok(())
}

fn validate_fee_args(args: &[OsString]) -> Result<()> {
    let values: Vec<&str> = args
        .iter()
        .map(|value| {
            value
                .to_str()
                .ok_or_else(|| eyre!("fee selection must be UTF-8"))
        })
        .collect::<Result<_>>()?;
    match values.as_slice() {
        ["--fee-payer", "authority"] => Ok(()),
        [
            "--fee-payer",
            "sponsor",
            "--fee-program",
            program,
            "--fee-program-revision",
            revision,
        ] if !program.is_empty()
            && program.len() <= 512
            && program.bytes().all(|byte| byte.is_ascii_graphic())
            && revision.parse::<u64>().is_ok_and(|value| value > 0) =>
        {
            Ok(())
        }
        _ => Err(eyre!(
            "apply fee selection is not one exact supported argv shape"
        )),
    }
}

fn common_open_ssh_options(
    endpoint: &EndpointV1,
    identity: &Path,
    known_hosts: &Path,
    timeout_secs: u64,
) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("-F"),
        OsString::from("/dev/null"),
        OsString::from("-o"),
        OsString::from("BatchMode=yes"),
        OsString::from("-o"),
        OsString::from("IdentitiesOnly=yes"),
        OsString::from("-o"),
        OsString::from("IdentityAgent=none"),
        OsString::from("-o"),
        OsString::from("PasswordAuthentication=no"),
        OsString::from("-o"),
        OsString::from("KbdInteractiveAuthentication=no"),
        OsString::from("-o"),
        OsString::from("PreferredAuthentications=publickey"),
        OsString::from("-o"),
        OsString::from("StrictHostKeyChecking=yes"),
        OsString::from("-o"),
        OsString::from(format!("UserKnownHostsFile={}", known_hosts.display())),
        OsString::from("-o"),
        OsString::from("GlobalKnownHostsFile=/dev/null"),
        OsString::from("-o"),
        OsString::from("VerifyHostKeyDNS=no"),
        OsString::from("-o"),
        OsString::from("UpdateHostKeys=no"),
        OsString::from("-o"),
        OsString::from("CanonicalizeHostname=no"),
        OsString::from("-o"),
        OsString::from(format!("HostKeyAlias={}", endpoint.hostname)),
        OsString::from("-o"),
        OsString::from("ClearAllForwardings=yes"),
        OsString::from("-o"),
        OsString::from("ForwardAgent=no"),
        OsString::from("-o"),
        OsString::from("ForwardX11=no"),
        OsString::from("-o"),
        OsString::from("PermitLocalCommand=no"),
        OsString::from("-o"),
        OsString::from("ProxyCommand=none"),
        OsString::from("-o"),
        OsString::from("ProxyJump=none"),
        OsString::from("-o"),
        OsString::from("ControlMaster=no"),
        OsString::from("-o"),
        OsString::from("ControlPath=none"),
        OsString::from("-o"),
        OsString::from(format!("ConnectTimeout={timeout_secs}")),
        OsString::from("-i"),
        identity.as_os_str().to_owned(),
    ];
    args.shrink_to_fit();
    args
}

fn ssh_common_args(
    endpoint: &EndpointV1,
    identity: &Path,
    known_hosts: &Path,
    timeout_secs: u64,
) -> Vec<OsString> {
    let mut args = common_open_ssh_options(endpoint, identity, known_hosts, timeout_secs);
    args.extend([
        OsString::from("-T"),
        OsString::from("-x"),
        OsString::from("-p"),
        OsString::from(endpoint.port.to_string()),
    ]);
    args
}

fn safe_remote_path(value: &str) -> bool {
    value.starts_with('/')
        && !value.contains("//")
        && !value.split('/').any(|part| matches!(part, "." | ".."))
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'/' | b'.' | b'-' | b'_')
        })
}

fn validate_remote_command(value: &str) -> Result<()> {
    let Some((program, suffix)) = value.split_once(' ') else {
        return Err(eyre!("remote dispatcher command is incomplete"));
    };
    if !safe_remote_path(program)
        || suffix != HOST_DISPATCH_SUFFIX
        || value.bytes().any(|byte| {
            !(byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'/' | b'.' | b'-' | b'_' | b' '))
        })
    {
        return Err(eyre!(
            "remote dispatcher command escaped its closed grammar"
        ));
    }
    Ok(())
}

fn verify_remote_receipt(request: &HostRequestV1, receipt: &HostReceiptV1) -> Result<()> {
    let inventory = BASE64
        .decode(&request.inventory_base64)
        .wrap_err("request inventory base64 is invalid")?;
    let inventory_value: InventoryV1 =
        json::from_slice(&inventory).wrap_err("request inventory JSON is invalid")?;
    if receipt.schema != HOST_RECEIPT_SCHEMA_V1
        || receipt.action != request.action
        || receipt.host_slug != request.host_slug
        || receipt.request_sha256 != host_request_identity_sha256(request)?
        || receipt.inventory_sha256 != sha256_hex(&inventory)
        || receipt.authorization_sha256 != request.authorization_semantic_sha256
        || receipt.authorization_nonce != inventory_value.authorization_nonce
        || receipt.status != "ok"
        || receipt.bytes_after > receipt.bytes_before
        || receipt.reclaimed_bytes != receipt.bytes_before - receipt.bytes_after
    {
        return Err(eyre!(
            "remote host receipt does not exactly bind its request"
        ));
    }
    Ok(())
}

fn verify_remote_recovery_receipt(request: &HostRequestV1, receipt: &HostReceiptV1) -> Result<()> {
    if !request.recovery_only {
        return Err(eyre!("host recovery verifier requires recovery_only"));
    }
    let inventory = BASE64
        .decode(&request.inventory_base64)
        .wrap_err("request inventory base64 is invalid")?;
    let inventory_value: InventoryV1 =
        json::from_slice(&inventory).wrap_err("request inventory JSON is invalid")?;
    if receipt.schema != HOST_RECEIPT_SCHEMA_V1
        || receipt.action != request.action
        || receipt.host_slug != request.host_slug
        || receipt.request_sha256 != host_request_identity_sha256(request)?
        || receipt.inventory_sha256 != sha256_hex(&inventory)
        || receipt.authorization_sha256 != request.authorization_semantic_sha256
        || receipt.authorization_nonce != inventory_value.authorization_nonce
        || !matches!(receipt.status.as_str(), "ok" | "pending" | "rejected")
        || receipt.bytes_before != 0
        || receipt.bytes_after != 0
        || receipt.reclaimed_bytes != 0
    {
        return Err(eyre!(
            "remote host recovery receipt does not exactly bind its request"
        ));
    }
    Ok(())
}

fn verify_remote_reservation_receipt(
    request: &HostRequestV1,
    receipt: &HostReceiptV1,
) -> Result<()> {
    if request.action != HostAction::MutationReserve.label()
        || request.mutation_kind.is_empty()
        || request.mutation_phase.is_empty()
        || request.mutation_idempotency_key.is_empty()
    {
        return Err(eyre!(
            "host reservation verifier received a non-reservation request"
        ));
    }
    let inventory = BASE64
        .decode(&request.inventory_base64)
        .wrap_err("request inventory base64 is invalid")?;
    let inventory_value: InventoryV1 =
        json::from_slice(&inventory).wrap_err("request inventory JSON is invalid")?;
    if receipt.schema != HOST_RECEIPT_SCHEMA_V1
        || receipt.action != request.action
        || receipt.host_slug != request.host_slug
        || receipt.request_sha256 != host_request_identity_sha256(request)?
        || receipt.inventory_sha256 != sha256_hex(&inventory)
        || receipt.authorization_sha256 != request.authorization_semantic_sha256
        || receipt.authorization_nonce != inventory_value.authorization_nonce
        || !matches!(
            receipt.status.as_str(),
            "prepared" | "submitted" | "applied" | "absent"
        )
        || receipt.mutation_state != receipt.status
        || receipt.bytes_before != 0
        || receipt.bytes_after != 0
        || receipt.reclaimed_bytes != 0
    {
        return Err(eyre!(
            "remote prepared-mutation receipt does not exactly bind its request"
        ));
    }
    if receipt.status == "absent" {
        if !receipt.mutation_prepared_base64.is_empty()
            || !receipt.mutation_prepared_sha256.is_empty()
            || !receipt.mutation_transaction_hash.is_empty()
        {
            return Err(eyre!(
                "absent prepared-mutation receipt contains envelope evidence"
            ));
        }
    } else {
        let bytes = BASE64
            .decode(&receipt.mutation_prepared_base64)
            .wrap_err("prepared-mutation receipt base64 is invalid")?;
        let proof_required = json::from_slice::<norito::json::Value>(&bytes)
            .ok()
            .and_then(|value| {
                value
                    .as_object()
                    .and_then(|root| root.get("operation"))
                    .and_then(norito::json::Value::as_object)
                    .and_then(|operation| operation.get("kind"))
                    .and_then(norito::json::Value::as_str)
                    .map(|kind| kind == "onboarding_proof_required")
            })
            .unwrap_or(false);
        if bytes.is_empty()
            || bytes.len() > MAX_PREPARED_ENVELOPE_BYTES
            || sha256_hex(&bytes) != receipt.mutation_prepared_sha256
            || (!receipt.mutation_transaction_hash.is_empty()
                && require_lower_sha256(
                    &receipt.mutation_transaction_hash,
                    "prepared-mutation receipt transaction hash",
                )
                .is_err())
            || (receipt.mutation_transaction_hash.is_empty() != proof_required)
            || (proof_required && receipt.status == "submitted")
        {
            return Err(eyre!(
                "prepared-mutation receipt envelope evidence is invalid"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, clap::Parser)]
    struct HostProtocolProbe {
        #[command(flatten)]
        host: PublicResetHost,
    }

    #[test]
    fn host_dispatch_requires_explicit_exact_v1_protocol() {
        let parsed = <HostProtocolProbe as clap::Parser>::try_parse_from([
            "host-dispatch",
            "--protocol",
            "v1",
        ])
        .expect("explicit exact V1 protocol");
        assert_eq!(parsed.host.protocol, "v1");

        for malformed in ["v0", "V1", "v1 "] {
            <HostProtocolProbe as clap::Parser>::try_parse_from([
                "host-dispatch",
                "--protocol",
                malformed,
            ])
            .expect_err("noncanonical protocol marker must fail during argument admission");
        }
        <HostProtocolProbe as clap::Parser>::try_parse_from(["host-dispatch"])
            .expect_err("omitting the hidden protocol marker must fail closed");
    }

    #[test]
    fn prepared_transaction_time_window_is_structurally_authorization_bound() {
        validate_prepared_transaction_time_window(
            1_000,
            200,
            1_100,
            990,
            1_300,
            PreparedMutationLifetimeCheck::Structural,
        )
        .expect("prepared transaction inside authorization window");
        for (creation, ttl, now, not_before, execution_expiry) in [
            (1_000, 0, 1_000, 990, 1_300),
            (
                1_000,
                200,
                1_100,
                1_000 + super::super::MAX_CLOCK_SKEW_MS + 1,
                1_300,
            ),
            (1_000, 301, 1_100, 990, 1_300),
            (u64::MAX, 1, u64::MAX, 990, u64::MAX),
        ] {
            let _error = validate_prepared_transaction_time_window(
                creation,
                ttl,
                now,
                not_before,
                execution_expiry,
                PreparedMutationLifetimeCheck::Structural,
            )
            .expect_err("invalid prepared transaction time window must fail closed");
        }
    }

    #[test]
    fn prepared_transaction_lifetime_only_requires_freshness_for_live_forwarding() {
        validate_prepared_transaction_time_window(
            1_000,
            200,
            1_200,
            990,
            1_300,
            PreparedMutationLifetimeCheck::Structural,
        )
        .expect("durable identity recovery accepts an expired signed transaction");
        let _error = validate_prepared_transaction_time_window(
            1_000,
            200,
            1_200,
            990,
            1_300,
            PreparedMutationLifetimeCheck::LiveForward,
        )
        .expect_err("live forwarding rejects an expired signed transaction");

        let future_creation = 1_000 + super::super::MAX_CLOCK_SKEW_MS + 1;
        validate_prepared_transaction_time_window(
            future_creation,
            200,
            1_000,
            990,
            u64::MAX,
            PreparedMutationLifetimeCheck::Structural,
        )
        .expect("durable identity recovery does not depend on the current wall clock");
        let _error = validate_prepared_transaction_time_window(
            future_creation,
            200,
            1_000,
            990,
            u64::MAX,
            PreparedMutationLifetimeCheck::LiveForward,
        )
        .expect_err("live forwarding rejects a transaction created too far in the future");
    }

    #[test]
    fn signed_inventory_derives_the_only_accepted_fee_selection() {
        let mut inventory = progress_admission().inventory;
        let authority = inventory_fee_payment_intent(&inventory).expect("authority fee intent");
        assert!(authority.sponsor_program().is_none());
        assert_eq!(authority.gas_limit(), None);
        assert!(authority.charge_limits().is_empty());

        let _chain_guard = ChainDiscriminantGuard::enter(inventory.chain_discriminant);
        let sponsor_owner =
            AccountId::parse_encoded(&inventory.canary_onboarding_request.account_id)
                .expect("canonical inventory canary account");
        let sponsor = FeeSponsorProgramId::new(
            sponsor_owner,
            Name::from_str("public_reset").expect("program name"),
        );
        inventory.fee_intent.payer = "sponsor".to_owned();
        inventory.fee_intent.sponsor_program = Some(sponsor.to_string());
        inventory.fee_intent.sponsor_program_revision = Some(7);
        let selected = inventory_fee_payment_intent(&inventory).expect("sponsor fee intent");
        assert_eq!(selected.sponsor_program(), Some((&sponsor, 7)));
        assert_eq!(selected.gas_limit(), None);
        assert!(selected.charge_limits().is_empty());

        inventory.fee_intent.sponsor_program_revision = Some(0);
        let _error = inventory_fee_payment_intent(&inventory)
            .expect_err("zero sponsor revision must fail closed");
    }

    #[test]
    fn signed_inventory_derives_the_only_accepted_faucet_policy() {
        let mut inventory = progress_admission().inventory;
        let policy = inventory_faucet_policy(&inventory).expect("canonical faucet policy");
        assert_eq!(
            policy.faucet_authority().to_string(),
            inventory.faucet_policy.authority
        );
        assert_eq!(
            policy.asset_definition_id().to_string(),
            inventory.faucet_policy.asset_definition_id
        );
        assert_eq!(policy.amount(), &inventory.faucet_policy.amount);

        inventory.faucet_policy.authority = "not-an-account".to_owned();
        let _error = inventory_faucet_policy(&inventory)
            .expect_err("an invalid signed faucet authority must fail closed");
    }

    #[test]
    fn retained_liveness_receipt_never_suppresses_a_fresh_failure() {
        #[derive(Default)]
        struct Probe {
            runs: usize,
            publications: usize,
        }

        let mut success = Probe::default();
        require_fresh_liveness_report(
            &mut success,
            true,
            |probe| {
                probe.runs += 1;
                Ok(norito::json::Value::from("fresh"))
            },
            |value, _| {
                if value.as_str() == Some("fresh") {
                    Ok(())
                } else {
                    Err(eyre!("fresh liveness value drifted"))
                }
            },
            |probe, _| {
                probe.publications += 1;
                Ok(())
            },
        )
        .expect("a valid fresh observation succeeds");
        assert_eq!(success.runs, 1);
        assert_eq!(success.publications, 0);

        let mut first = Probe::default();
        require_fresh_liveness_report(
            &mut first,
            false,
            |probe| {
                probe.runs += 1;
                Ok(norito::json::Value::from("fresh"))
            },
            |_, _| Ok(()),
            |probe, _| {
                probe.publications += 1;
                Ok(())
            },
        )
        .expect("the first fresh observation is retained");
        assert_eq!(first.runs, 1);
        assert_eq!(first.publications, 1);

        let mut failure = Probe::default();
        let error = require_fresh_liveness_report(
            &mut failure,
            true,
            |probe| {
                probe.runs += 1;
                Err(eyre!("live Inrou route is unavailable"))
            },
            |_, _| Ok(()),
            |probe, _| {
                probe.publications += 1;
                Ok(())
            },
        )
        .expect_err("old evidence must not hide current liveness failure");
        assert!(
            error
                .to_string()
                .contains("live Inrou route is unavailable")
        );
        assert_eq!(failure.runs, 1);
        assert_eq!(failure.publications, 0);
    }

    #[test]
    fn cleanup_tombstones_use_one_exact_first_release_name() {
        let upload = OsString::from("a".repeat(32));
        let upload_tombstone =
            cleanup_tombstone_name(&upload, "upload").expect("canonical upload tombstone");
        assert_eq!(
            cleanup_original_name_from_tombstone(&upload_tombstone, "upload")
                .expect("parse upload tombstone"),
            Some(upload)
        );

        let release = OsString::from("1".repeat(40));
        let release_tombstone =
            cleanup_tombstone_name(&release, "release").expect("canonical release tombstone");
        assert_eq!(
            cleanup_original_name_from_tombstone(&release_tombstone, "release")
                .expect("parse release tombstone"),
            Some(release)
        );

        assert!(
            cleanup_original_name_from_tombstone(OsStr::new("ordinary-release"), "release")
                .expect("ordinary entry is not a tombstone")
                .is_none()
        );
        assert!(
            cleanup_original_name_from_tombstone(
                OsStr::new(".public-reset-cleanup-v1-short.tombstone"),
                "upload",
            )
            .is_err()
        );
        assert!(
            cleanup_tombstone_name(OsStr::new("A2345678901234567890123456789012"), "upload")
                .is_err()
        );
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt as _;

            let mut reserved_bytes = CLEANUP_TOMBSTONE_PREFIX.as_bytes().to_vec();
            reserved_bytes.push(0xff);
            assert!(
                cleanup_original_name_from_tombstone(
                    &OsString::from_vec(reserved_bytes),
                    "upload",
                )
                .is_err()
            );
            assert!(
                cleanup_original_name_from_tombstone(
                    &OsString::from_vec(vec![b'o', b't', b'h', b'e', b'r', 0xff]),
                    "upload",
                )
                .expect("non-reserved non-UTF-8 entry remains foreign")
                .is_none()
            );
        }
    }

    #[test]
    fn cleanup_plan_binds_retry_evidence_and_signed_cap() {
        let admitted = progress_admission();
        let original_name = admitted.inventory.authorization_nonce.clone();
        let marker = GeneratedMarkerV1 {
            schema: GENERATED_MARKER_SCHEMA_V1.to_owned(),
            kind: "upload".to_owned(),
            host_slug: admitted.target.slug().to_owned(),
            inventory_sha256: admitted.inventory_sha256.clone(),
            authorization_nonce: original_name.clone(),
            revision: admitted.inventory.revision.commit.clone(),
            created_at_unix_ms: 1,
        };
        let marker_sha256 = sha256_hex(json::to_json(&marker).expect("marker JSON").as_bytes());
        let entry = CleanupPlanEntryV1 {
            kind: "upload".to_owned(),
            original_name: original_name.clone(),
            original_path: Path::new(&admitted.guard.upload_parent)
                .join(&original_name)
                .to_string_lossy()
                .into_owned(),
            marker,
            marker_sha256,
            directory_device: 1,
            directory_inode: 1,
            initial_bytes: 1,
        };
        let plan = CleanupPlanV1 {
            schema: CLEANUP_PLAN_SCHEMA_V1.to_owned(),
            action: HostAction::Cleanup.label().to_owned(),
            host_slug: admitted.target.slug().to_owned(),
            request_sha256: admitted.request_sha256.clone(),
            inventory_sha256: admitted.inventory_sha256.clone(),
            authorization_sha256: admitted.authorization_sha256.clone(),
            authorization_nonce: admitted.inventory.authorization_nonce.clone(),
            max_reclaim_bytes: admitted.inventory.cleanup.max_reclaim_bytes_per_host,
            bytes_before: 1,
            entries: vec![entry],
        };
        validate_cleanup_plan(&admitted, &plan).expect("exact cleanup plan");
        assert_eq!(
            cleanup_plan_detail(&plan),
            format!(
                "removed marker-authorized paths: {}",
                plan.entries[0].original_path
            )
        );

        let mut wrong_request = plan.clone();
        wrong_request.request_sha256 = "0".repeat(64);
        let _error = validate_cleanup_plan(&admitted, &wrong_request)
            .expect_err("another request cannot replay the plan");

        let mut duplicate = plan.clone();
        duplicate.entries.push(duplicate.entries[0].clone());
        duplicate.bytes_before = 2;
        let _error = validate_cleanup_plan(&admitted, &duplicate)
            .expect_err("duplicate cleanup roots are not canonical");

        let mut oversized = plan;
        oversized.entries[0].initial_bytes = oversized.max_reclaim_bytes + 1;
        oversized.bytes_before = oversized.entries[0].initial_bytes;
        let _error = validate_cleanup_plan(&admitted, &oversized)
            .expect_err("cleanup plan cannot exceed the signed cap");
    }

    #[test]
    fn cleanup_plan_accounts_only_tombstone_or_completed_prefixes() {
        assert_eq!(
            cleanup_plan_deleted_prefix(100, 100, false).expect("unchanged candidate"),
            0
        );
        let _error = cleanup_plan_deleted_prefix(100, 99, false)
            .expect_err("an unrenamed candidate cannot be partially deleted");
        assert_eq!(
            cleanup_plan_deleted_prefix(100, 40, true).expect("partial tombstone"),
            60
        );
        assert_eq!(
            cleanup_plan_deleted_prefix(100, 0, true).expect("durably completed root"),
            100
        );
        let _error = cleanup_plan_deleted_prefix(100, 101, true)
            .expect_err("a planned root cannot grow past its admitted byte closure");
    }

    #[test]
    fn cleanup_plan_rechecks_the_selected_release_before_mutation() {
        let planned =
            Path::new("/srv/taira/node/releases/1111111111111111111111111111111111111111");
        let other = Path::new("/srv/taira/node/releases/2222222222222222222222222222222222222222");
        ensure_cleanup_plan_entry_not_selected("release", planned, other)
            .expect("an unselected old release remains eligible");
        let _error = ensure_cleanup_plan_entry_not_selected("release", planned, planned)
            .expect_err("a release selected after planning must never be deleted");
        ensure_cleanup_plan_entry_not_selected("upload", planned, planned)
            .expect("the release selector does not govern upload roots");
    }

    #[expect(
        clippy::too_many_lines,
        reason = "the fixture constructs one complete authenticated status graph so every nested nullable field is exercised by the public-reset decoder"
    )]
    fn canonical_convergence_status_fixture() -> (ValidatorV1, norito::json::Value) {
        use iroha::data_model::block::consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DualQuorum, ExecutionCommitment,
            GlobalPhase, HeightContext, HeightContextId, PROTOCOL_VERSION, QuorumCertificateRef,
            SumeragiV2BodyState, SumeragiV2CommitQcStatus, SumeragiV2HeightContextStatus,
            SumeragiV2LivenessStatus, SumeragiV2OutboundIntentKind, SumeragiV2OutboundIntentStage,
            SumeragiV2OutboundIntentStatus, SumeragiV2QueueKind, SumeragiV2QueueStatus,
            SumeragiV2StatusPhase,
        };

        let validator = progress_admission().inventory.validators[0].clone();
        let current_context_id = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
            Hash::new(b"public reset current status context"),
        ));
        let committed_context_id =
            HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(Hash::new(
                b"public reset committed status context",
            )));
        let committed_round = ConsensusRound {
            context_id: committed_context_id,
            height: 7,
            view: 1,
        };
        let committed_subject = BlockSubject {
            parent_block_hash: Some(HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"public reset committed parent",
            ))),
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
                b"public reset committed block",
            )),
            payload_hash: Hash::new(b"public reset committed payload"),
        };
        let execution_commitment = ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"public reset parent state"),
            Hash::new(b"public reset post state"),
            Hash::new(b"public reset writes"),
            1,
            Hash::new(b"public reset executed block"),
        );
        let certificate = QuorumCertificateRef {
            round: committed_round,
            proposal_round: committed_round,
            phase: GlobalPhase::Commit,
            subject: committed_subject,
            execution_commitment,
        };
        let active_round = ConsensusRound {
            context_id: current_context_id,
            height: 8,
            view: 0,
        };
        let status = SumeragiV2Status {
            protocol_version: PROTOCOL_VERSION,
            node_fingerprint: validator
                .node_fingerprint
                .parse()
                .expect("fixture node fingerprint"),
            build_fingerprint: validator
                .build_fingerprint
                .parse()
                .expect("fixture build fingerprint"),
            config_fingerprint: validator
                .config_fingerprint
                .parse()
                .expect("fixture config fingerprint"),
            restart_required: false,
            height_context_id: current_context_id,
            height: 8,
            view: 0,
            phase: SumeragiV2StatusPhase::AwaitingProposal,
            leader: 0,
            locked_prepare_qc: None,
            highest_prepare_qc: None,
            last_timeout_certificate: None,
            body_state: SumeragiV2BodyState::Missing,
            pending_persistence_id: None,
            last_committed_height: 7,
            last_committed_subject: Some(committed_subject),
            height_context: SumeragiV2HeightContextStatus {
                epoch: 1,
                epoch_end_height: 100,
                mode: ConsensusMode::Permissioned,
                epoch_seed: [0x71; 32],
                validator_count: 4,
                quorum: DualQuorum {
                    min_signers: 3,
                    total_power: 4,
                },
            },
            last_commit_qc: Some(SumeragiV2CommitQcStatus {
                certificate,
                validator_count: 4,
                signer_count: 3,
                min_signers: 3,
                signed_power: 3,
                total_power: 4,
            }),
            liveness: SumeragiV2LivenessStatus {
                outbound_intents: vec![SumeragiV2OutboundIntentStatus {
                    kind: SumeragiV2OutboundIntentKind::TimeoutVote,
                    round: active_round,
                    proposal_round: None,
                    subject: None,
                    execution_commitment: None,
                    stage: SumeragiV2OutboundIntentStage::Sent,
                }],
                queues: vec![SumeragiV2QueueStatus {
                    queue: SumeragiV2QueueKind::RuntimeProgress,
                    depth: 0,
                    capacity: 1,
                    oldest_age_ms: None,
                    service_debt: 0,
                }],
                ..SumeragiV2LivenessStatus::default()
            },
        };
        status.validate().expect("canonical convergence status");
        (
            validator,
            json::to_value(&status).expect("canonical convergence status JSON"),
        )
    }

    fn assert_missing_convergence_field_rejected(
        value: norito::json::Value,
        validator: &ValidatorV1,
        field: &str,
    ) {
        let error = validate_convergence_status(&value, validator)
            .expect_err("sparse convergence status must reject");
        let diagnostic = format!("{error:?}");
        assert!(
            diagnostic.contains("Sumeragi status is not exact canonical V1 JSON")
                && diagnostic.contains(&format!("missing field `{field}`")),
            "unexpected sparse-status error for `{field}`: {diagnostic}"
        );
    }

    #[test]
    fn public_reset_convergence_rejects_omitted_nullable_status_fields() {
        let (validator, canonical) = canonical_convergence_status_fixture();
        validate_convergence_status(&canonical, &validator)
            .expect("complete first-release convergence status");

        for field in [
            "locked_prepare_qc",
            "highest_prepare_qc",
            "last_timeout_certificate",
            "pending_persistence_id",
            "last_committed_subject",
            "last_commit_qc",
        ] {
            let mut missing = canonical.clone();
            missing
                .as_object_mut()
                .expect("status object")
                .remove(field);
            assert_missing_convergence_field_rejected(missing, &validator, field);
        }
        for field in ["last_progress", "blocker"] {
            let mut missing = canonical.clone();
            missing
                .as_object_mut()
                .and_then(|status| status.get_mut("liveness"))
                .and_then(norito::json::Value::as_object_mut)
                .expect("liveness object")
                .remove(field);
            assert_missing_convergence_field_rejected(missing, &validator, field);
        }
        for field in ["proposal_round", "subject", "execution_commitment"] {
            let mut missing = canonical.clone();
            missing
                .as_object_mut()
                .and_then(|status| status.get_mut("liveness"))
                .and_then(norito::json::Value::as_object_mut)
                .and_then(|liveness| liveness.get_mut("outbound_intents"))
                .and_then(norito::json::Value::as_array_mut)
                .and_then(|intents| intents.first_mut())
                .and_then(norito::json::Value::as_object_mut)
                .expect("outbound intent object")
                .remove(field);
            assert_missing_convergence_field_rejected(missing, &validator, field);
        }
        let mut missing = canonical;
        missing
            .as_object_mut()
            .and_then(|status| status.get_mut("liveness"))
            .and_then(norito::json::Value::as_object_mut)
            .and_then(|liveness| liveness.get_mut("queues"))
            .and_then(norito::json::Value::as_array_mut)
            .and_then(|queues| queues.first_mut())
            .and_then(norito::json::Value::as_object_mut)
            .expect("queue status object")
            .remove("oldest_age_ms");
        assert_missing_convergence_field_rejected(missing, &validator, "oldest_age_ms");
    }

    fn sample_request() -> HostRequestV1 {
        HostRequestV1 {
            schema: HOST_REQUEST_SCHEMA_V1.to_owned(),
            action: "stage".to_owned(),
            recovery_only: false,
            host_slug: "taira-validator-1".to_owned(),
            inventory_base64: BASE64.encode(b"inventory"),
            authorization_base64: BASE64.encode(b"authorization"),
            authorization_semantic_sha256: "11".repeat(32),
            trusted_key_base64: BASE64.encode(b"trusted"),
            trusted_key_sha256: "22".repeat(32),
            action_deadline_unix_ms: 1_000,
            artifact_role: String::new(),
            artifact_sha256: String::new(),
            artifact_size: 0,
            artifact_mode: 0,
            mutation_kind: String::new(),
            mutation_phase: String::new(),
            mutation_idempotency_key: String::new(),
            mutation_operation: String::new(),
            mutation_prepared_base64: String::new(),
            mutation_prepared_sha256: String::new(),
            mutation_transaction_hash: String::new(),
            mutation_evidence_base64: String::new(),
        }
    }

    fn progress_admission() -> HostAdmission {
        let mut inventory = super::super::sample_inventory_fixture();
        let shared_identity = "a".repeat(64);
        for validator in &mut inventory.validators {
            validator.endpoint.host_identity_sha256 = shared_identity.clone();
        }
        inventory.edge.endpoint.host_identity_sha256 = shared_identity;
        let inventory_bytes = json::to_json(&inventory)
            .expect("inventory JSON")
            .into_bytes();
        let inventory_sha256 = sha256_hex(&inventory_bytes);
        let claims = super::super::AuthorizationClaimsV1 {
            action: "reset_and_deploy".to_owned(),
            deployment_id: inventory.deployment_id.clone(),
            inventory_sha256: inventory_sha256.clone(),
            artifact_closure_sha256: inventory.artifact_closure_sha256.clone(),
            runtime_client_config_sha256: inventory.runtime_client_config_sha256.clone(),
            onboarding_token_sha256: inventory.onboarding_token_sha256.clone(),
            validator_client_configs_sha256: inventory.validator_client_configs_sha256.clone(),
            inrou_stage_tree_sha256: inventory.inrou_stage_tree_sha256.clone(),
            faucet_policy: inventory.faucet_policy.clone(),
            fee_intent: inventory.fee_intent.clone(),
            authorization_nonce: inventory.authorization_nonce.clone(),
            issued_at_unix_ms: 1,
            not_before_unix_ms: 1,
            expires_at_unix_ms: u64::MAX - 2,
            execution_expires_at_unix_ms: u64::MAX - 1,
        };
        let authorization = AuthorizationEnvelopeV1 {
            schema: "iroha.taira.public-reset.authorization.v1".to_owned(),
            claims,
            signature_hex: "11".repeat(64),
        };
        let target = HostTarget::Validator(inventory.validators[0].clone());
        HostAdmission {
            request: HostRequestV1 {
                schema: HOST_REQUEST_SCHEMA_V1.to_owned(),
                action: HostAction::Stage.label().to_owned(),
                recovery_only: false,
                host_slug: inventory.validators[0].slug.clone(),
                inventory_base64: BASE64.encode(&inventory_bytes),
                authorization_base64: String::new(),
                authorization_semantic_sha256: "b".repeat(64),
                trusted_key_base64: String::new(),
                trusted_key_sha256: "c".repeat(64),
                action_deadline_unix_ms: u64::MAX - 1,
                artifact_role: String::new(),
                artifact_sha256: String::new(),
                artifact_size: 0,
                artifact_mode: 0,
                mutation_kind: String::new(),
                mutation_phase: String::new(),
                mutation_idempotency_key: String::new(),
                mutation_operation: String::new(),
                mutation_prepared_base64: String::new(),
                mutation_prepared_sha256: String::new(),
                mutation_transaction_hash: String::new(),
                mutation_evidence_base64: String::new(),
            },
            request_sha256: "d".repeat(64),
            inventory,
            inventory_sha256,
            authorization,
            authorization_sha256: "b".repeat(64),
            target,
            guard: HostGuardV1 {
                schema: HOST_GUARD_SCHEMA_V1.to_owned(),
                host_slug: "taira-validator-1".to_owned(),
                service_root: "/srv/taira/taira-validator-1".to_owned(),
                state_root: "/var/lib/taira/taira-validator-1".to_owned(),
                trusted_key_sha256: "c".repeat(64),
                dispatcher_path: FIXED_DISPATCHER.to_owned(),
                dispatcher_sha256: "e".repeat(64),
                upload_parent: "/srv/taira/taira-validator-1/.public-reset-upload-v1".to_owned(),
            },
            action_deadline: Instant::now() + Duration::from_secs(60),
            execution_expired: false,
        }
    }

    fn select_target(admitted: &mut HostAdmission, slug: &str) {
        admitted.target = admitted
            .inventory
            .validators
            .iter()
            .find(|validator| validator.slug == slug)
            .cloned()
            .map(HostTarget::Validator)
            .or_else(|| {
                (admitted.inventory.edge.slug == slug)
                    .then(|| HostTarget::Edge(admitted.inventory.edge.clone()))
            })
            .expect("fixture target");
        admitted.request.host_slug = slug.to_owned();
    }

    #[test]
    fn first_release_reset_rejects_mixed_physical_hosts() {
        let mut mixed = super::super::sample_inventory_fixture();
        mixed.validators[0].endpoint.host_identity_sha256 = "f".repeat(64);
        let _ = validate_first_release_physical_host(&mixed)
            .expect_err("mixed physical host identities must fail closed");
        let shared = progress_admission();
        validate_first_release_physical_host(&shared.inventory)
            .expect("one physical host identity is canonical");
    }

    #[test]
    fn host_progress_requires_explicit_nullable_prepared_action_slot() {
        let admitted = progress_admission();
        let progress = initial_host_progress(&admitted);
        let canonical = json::to_value(&progress).expect("host-progress JSON value");
        assert!(
            canonical
                .as_object()
                .and_then(|object| object.get("prepared_action"))
                .is_some_and(norito::json::Value::is_null),
            "empty host progress must serialize an explicit prepared_action null slot"
        );
        json::from_value::<HostProgressV1>(canonical.clone())
            .expect("explicit nullable host-progress slot");

        let mut missing = canonical;
        missing
            .as_object_mut()
            .expect("host-progress object")
            .remove("prepared_action");
        assert!(
            json::from_value::<HostProgressV1>(missing).is_err(),
            "the exact V1 host progress must reject an omitted prepared_action slot"
        );
    }

    #[test]
    fn shared_host_progress_enforces_cohort_barriers_and_exact_rollback_target() {
        let mut admitted = progress_admission();
        let plan = host_forward_plan(&admitted);
        let stop_one = HostActionKeyV1 {
            host_slug: "taira-validator-1".to_owned(),
            action: HostAction::Stop.label().to_owned(),
            artifact_role: String::new(),
        };
        let stop_ordinal = plan
            .iter()
            .position(|key| key == &stop_one)
            .expect("stop ordinal");
        let mut progress = initial_host_progress(&admitted);
        progress.next_forward_ordinal = u16::try_from(stop_ordinal + 1).expect("bounded plan");
        progress.touched_hosts.push(stop_one.host_slug.clone());

        select_target(&mut admitted, "taira-validator-1");
        assert_eq!(
            admit_host_action_progress(&admitted, HostAction::Install, &progress)
                .expect_err("install must wait for every validator stop")
                .to_string(),
            "host action violates the exact monotonic physical-host phase order"
        );
        assert_eq!(
            admit_host_action_progress(&admitted, HostAction::Rollback, &progress)
                .expect("only touched validator is exact rollback target"),
            HostProgressDecision::Advance
        );
        select_target(&mut admitted, "taira-validator-2");
        assert_eq!(
            admit_host_action_progress(&admitted, HostAction::Rollback, &progress)
                .expect("untouched conservative target must be a proven no-op"),
            HostProgressDecision::AbsentNoOp
        );
        select_target(&mut admitted, "taira-validator-1");
        assert_eq!(
            admit_host_action_progress(&admitted, HostAction::Rollback, &progress)
                .expect("the exact touched target remains next after absent no-op"),
            HostProgressDecision::Advance
        );
    }

    #[test]
    fn edge_cutover_is_touched_first_and_success_seal_forbids_rollback() {
        let mut admitted = progress_admission();
        let plan = host_forward_plan(&admitted);
        let edge = admitted.inventory.edge.slug.clone();
        let cutover = HostActionKeyV1 {
            host_slug: edge.clone(),
            action: HostAction::EdgeCutover.label().to_owned(),
            artifact_role: String::new(),
        };
        assert!(host_action_touches_live_state(&cutover));
        let mut progress = initial_host_progress(&admitted);
        progress.touched_hosts = vec!["taira-validator-1".to_owned(), edge.clone()];
        progress.next_forward_ordinal = u16::try_from(
            plan.iter()
                .position(|key| key == &cutover)
                .expect("cutover ordinal"),
        )
        .expect("bounded plan");
        progress.prepared_action = Some(cutover);
        select_target(&mut admitted, &edge);
        assert_eq!(
            admit_host_action_progress(&admitted, HostAction::Rollback, &progress)
                .expect("edge is exact first rollback target"),
            HostProgressDecision::Advance
        );

        let first_seal = plan
            .iter()
            .position(|key| key.action == HostAction::Seal.label())
            .expect("seal ordinal");
        progress.next_forward_ordinal = u16::try_from(first_seal + 1).expect("bounded plan");
        progress.prepared_action = None;
        progress.touched_hosts.clear();
        assert!(
            admit_host_action_progress(&admitted, HostAction::Rollback, &progress).is_err(),
            "sealing deployment must reject every rollback"
        );
    }

    #[test]
    fn canonical_boot_id_rejects_reboot_or_noncanonical_identity() {
        validate_boot_id("01234567-89ab-cdef-0123-456789abcdef")
            .expect("canonical lowercase boot UUID");
        assert!(validate_boot_id("01234567-89AB-cdef-0123-456789abcdef").is_err());
        assert!(validate_boot_id("0123456789abcdef0123456789abcdef").is_err());
    }

    #[test]
    fn host_request_retry_identity_excludes_only_deadline() {
        let first = sample_request();
        let mut retry = first.clone();
        retry.action_deadline_unix_ms = 2_000;
        assert_eq!(
            host_request_identity_sha256(&first).expect("first identity"),
            host_request_identity_sha256(&retry).expect("retry identity")
        );
        retry.action = "install".to_owned();
        assert_ne!(
            host_request_identity_sha256(&first).expect("first identity"),
            host_request_identity_sha256(&retry).expect("different action identity")
        );
    }

    #[test]
    fn host_frame_rejects_noncanonical_uppercase_length() {
        let host = PublicResetHost {
            protocol: "v1".to_owned(),
        };
        let error = host
            .run(&b"0000000A\n0123456789"[..], Vec::new())
            .expect_err("uppercase frame must fail before admission");
        assert!(error.to_string().contains("canonical hex"));
    }

    #[test]
    fn process_runner_preserves_exact_named_descriptor() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("payload");
        fs::write(&path, b"descriptor-custody").expect("write payload");
        let file = File::open(&path).expect("open payload");
        let descriptor = file.as_raw_fd();
        #[cfg(target_os = "linux")]
        let inherited = format!("/proc/self/fd/{descriptor}");
        #[cfg(all(unix, not(target_os = "linux")))]
        let inherited = format!("/dev/fd/{descriptor}");
        let output = run_bounded_process(&ProcessSpec {
            program: PathBuf::from("/bin/cat"),
            args: vec![OsString::from(inherited)],
            stdin_prefix: Vec::new(),
            stdin_file: None,
            inherited_files: vec![file],
            deadline: Instant::now() + Duration::from_secs(2),
        })
        .expect("descriptor-backed child");
        assert!(output.status.success());
        assert_eq!(output.stdout, b"descriptor-custody");
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn process_runner_clears_environment() {
        let output = run_bounded_process(&ProcessSpec {
            program: PathBuf::from("/usr/bin/env"),
            args: Vec::new(),
            stdin_prefix: Vec::new(),
            stdin_file: None,
            inherited_files: Vec::new(),
            deadline: Instant::now() + Duration::from_secs(2),
        })
        .expect("sanitized environment child");
        assert!(output.status.success());
        assert_eq!(output.stdout, b"LC_ALL=C\n");
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn process_runner_enforces_one_absolute_timeout() {
        let started = Instant::now();
        let error = run_bounded_process(&ProcessSpec {
            program: PathBuf::from("/bin/sleep"),
            args: vec![OsString::from("5")],
            stdin_prefix: Vec::new(),
            stdin_file: None,
            inherited_files: Vec::new(),
            deadline: Instant::now() + Duration::from_millis(50),
        })
        .expect_err("sleep must be terminated and reaped");
        assert!(error.to_string().contains("exceeded"));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn process_runner_handles_child_that_closes_stdin_early() {
        let error = run_bounded_process(&ProcessSpec {
            program: PathBuf::from("/usr/bin/true"),
            args: Vec::new(),
            stdin_prefix: vec![b'x'; 4 * 1024 * 1024],
            stdin_file: None,
            inherited_files: Vec::new(),
            deadline: Instant::now() + Duration::from_secs(2),
        })
        .expect_err("early child exit must reject an incomplete framed stdin");
        assert!(
            error.to_string().contains("stdin") || error.to_string().contains("stream"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn loaded_systemd_unit_evidence_rejects_dropins_and_stale_manager_state() {
        let fragment = Path::new("/etc/systemd/system/taira-validator-1.service");
        validate_loaded_unit_evidence(
            b"FragmentPath=/etc/systemd/system/taira-validator-1.service\nDropInPaths=\nNeedDaemonReload=no\n",
            fragment,
        )
        .expect("exact loaded unit evidence");
        for invalid in [
            b"FragmentPath=/etc/systemd/system/taira-validator-1.service\nDropInPaths=/etc/systemd/system/taira-validator-1.service.d/override.conf\nNeedDaemonReload=no\n".as_slice(),
            b"FragmentPath=/etc/systemd/system/taira-validator-1.service\nDropInPaths=\nNeedDaemonReload=yes\n".as_slice(),
            b"FragmentPath=/etc/systemd/system/taira-validator-1.service\nFragmentPath=/etc/systemd/system/taira-validator-1.service\nDropInPaths=\nNeedDaemonReload=no\n".as_slice(),
        ] {
            let _ = validate_loaded_unit_evidence(invalid, fragment)
                .expect_err("noncanonical loaded unit evidence must fail");
        }
    }

    #[test]
    fn validator_argv_rejects_duplicate_last_wins_flags() {
        let executable = PathBuf::from("/srv/taira/taira-validator-1/current/bin/iroha3d_taira");
        let config = PathBuf::from("/srv/taira/taira-validator-1/current/config/config.toml");
        let genesis = PathBuf::from("/srv/taira/taira-validator-1/current/genesis/genesis.json");
        let exact = vec![
            executable.clone(),
            PathBuf::from("--config"),
            config.clone(),
            PathBuf::from("--genesis-manifest-json"),
            genesis.clone(),
            PathBuf::from("--sora"),
        ];
        validate_validator_argv(&exact, &executable, &config, &genesis)
            .expect("exact validator argv");
        let mut duplicate = exact;
        duplicate.extend([
            PathBuf::from("--config"),
            PathBuf::from("/tmp/attacker.toml"),
        ]);
        let _ = validate_validator_argv(&duplicate, &executable, &config, &genesis)
            .expect_err("duplicate last-wins config flag must fail");
    }

    #[test]
    fn canonical_local_report_preserves_prepared_identity_and_drops_only_fresh_time() {
        let report = norito::json!({
            "command": "taira_inrou_canary",
            "status": "ok",
            "public_root": "https://taira.sora.org",
            "checks": [],
            "warnings": [],
            "failures": [],
            "authorization_sha256": ("11".repeat(32)),
            "authorization_nonce": ("22".repeat(32)),
            "mutation_kind": "inrou_bundle_pin",
            "mutation_phase": "pre_edge",
            "idempotency_key": ("33".repeat(32)),
            "operation": "bundle_pin",
            "transaction_hash_hex": ("44".repeat(32)),
            "prepared_envelope_sha256": ("55".repeat(32)),
            "prepared_envelope_size": 1024,
            "recovery_outcome": "Applied",
            "applied_block_height": 7,
            "evidence": ("44".repeat(32)),
            "execution_expires_at_unix_ms": 99,
            "fee_payment": {},
            "fee_quote": {},
            "mutation_mode": "deploy",
            "observed_at_unix_ms": 1,
        });
        let canonical = canonical_local_report(&report).expect("canonical prepared receipt");
        let canonical = canonical.as_object().expect("canonical object");
        assert!(!canonical.contains_key("observed_at_unix_ms"));
        for field in [
            "authorization_sha256",
            "authorization_nonce",
            "mutation_kind",
            "mutation_phase",
            "idempotency_key",
            "operation",
            "transaction_hash_hex",
            "prepared_envelope_sha256",
            "prepared_envelope_size",
            "recovery_outcome",
            "applied_block_height",
            "evidence",
        ] {
            assert_eq!(
                canonical.get(field),
                report.get(field),
                "retained `{field}`"
            );
        }
    }

    fn admitted_reset_fixture() -> AdmittedReset {
        let remote = progress_admission();
        let inventory_bytes = json::to_vec(&remote.inventory).expect("fixture inventory JSON");
        let authorization_bytes =
            json::to_vec(&remote.authorization).expect("fixture authorization JSON");
        let ssh = File::open("/dev/null").expect("open fixture SSH input");
        let ssh_snapshot = super::super::file_snapshot(&ssh.metadata().expect("SSH metadata"))
            .expect("SSH snapshot");
        let known_hosts = File::open("/dev/null").expect("open fixture known-hosts input");
        let known_hosts_snapshot =
            super::super::file_snapshot(&known_hosts.metadata().expect("known-hosts metadata"))
                .expect("known-hosts snapshot");
        AdmittedReset {
            inventory: remote.inventory,
            inventory_bytes,
            inventory_sha256: remote.inventory_sha256,
            authorization_bytes,
            authorization_sha256: remote.authorization_sha256,
            trusted_key_bytes: Vec::new(),
            authorization: remote.authorization,
            trusted_key: TrustedKeyV1 {
                schema: "iroha.taira.public-reset.trusted-key.v1".to_owned(),
                algorithm: "ed25519".to_owned(),
                public_key: "ed0120".to_owned(),
            },
            pinned_artifacts: Vec::new(),
            ssh_identity: super::super::PinnedInput {
                path: PathBuf::from("/dev/null"),
                file: ssh,
                snapshot: ssh_snapshot,
            },
            known_hosts: super::super::PinnedInput {
                path: PathBuf::from("/dev/null"),
                file: known_hosts,
                snapshot: known_hosts_snapshot,
            },
        }
    }

    fn prepared_write_report_fixture(
        admitted: &AdmittedReset,
        kind: &str,
        idempotency_key: &str,
    ) -> norito::json::Value {
        let operation = match kind {
            "onboarding" => "onboarding",
            "faucet" => "faucet",
            "write_canary" => "final_canary",
            _ => panic!("unsupported write fixture kind"),
        };
        let mut report = norito::json!({
            "command": "taira_write_canary",
            "status": "ok",
            "public_root": (admitted.inventory.inrou_canary.public_root.clone()),
            "checks": [],
            "warnings": [],
            "failures": [],
            "authorization_sha256": (admitted.authorization_sha256.clone()),
            "authorization_nonce": (admitted.inventory.authorization_nonce.clone()),
            "mutation_kind": kind,
            "mutation_phase": "pre_edge",
            "idempotency_key": idempotency_key,
            "operation": operation,
            "transaction_hash_hex": ("4".repeat(64)),
            "prepared_envelope_sha256": ("5".repeat(64)),
            "prepared_envelope_size": 128,
            "recovery_outcome": "Prepared",
            "applied_block_height": null,
            "evidence": null,
            "execution_expires_at_unix_ms": (admitted.authorization.claims.execution_expires_at_unix_ms),
        });
        if kind == "write_canary" {
            let object = report.as_object_mut().expect("write report object");
            object.insert("fee_payment".to_owned(), norito::json!({}));
            object.insert("fee_quote".to_owned(), norito::json!({}));
        }
        report
    }

    fn prepared_inrou_report_fixture(
        admitted: &AdmittedReset,
        idempotency_key: &str,
    ) -> norito::json::Value {
        norito::json!({
            "command": "taira_inrou_canary",
            "status": "ok",
            "public_root": (admitted.inventory.inrou_canary.public_root.clone()),
            "checks": [],
            "warnings": [],
            "failures": [],
            "authorization_sha256": (admitted.authorization_sha256.clone()),
            "authorization_nonce": (admitted.inventory.authorization_nonce.clone()),
            "mutation_kind": "inrou_bundle_pin",
            "mutation_phase": "pre_edge",
            "idempotency_key": idempotency_key,
            "operation": "bundle_pin",
            "transaction_hash_hex": ("4".repeat(64)),
            "prepared_envelope_sha256": ("5".repeat(64)),
            "prepared_envelope_size": 128,
            "recovery_outcome": "Prepared",
            "applied_block_height": null,
            "evidence": null,
            "execution_expires_at_unix_ms": (admitted.authorization.claims.execution_expires_at_unix_ms),
            "fee_payment": {},
            "fee_quote": {},
            "mutation_mode": "deploy",
        })
    }

    #[test]
    fn prepared_write_report_rejects_every_missing_or_extra_v1_field() {
        let admitted = admitted_reset_fixture();
        let idempotency_key = "3".repeat(64);
        let canonical = prepared_write_report_fixture(&admitted, "faucet", &idempotency_key);
        validate_prepared_write_report(
            &canonical,
            &admitted,
            "pre_edge",
            "faucet",
            &idempotency_key,
            "Prepared",
            None,
        )
        .expect("canonical prepared write report");
        for field in PREPARED_WRITE_REPORT_FIELDS {
            let mut missing = canonical.clone();
            missing
                .as_object_mut()
                .expect("write report object")
                .remove(*field);
            let _error = validate_prepared_write_report(
                &missing,
                &admitted,
                "pre_edge",
                "faucet",
                &idempotency_key,
                "Prepared",
                None,
            )
            .expect_err("every prepared write report field is mandatory");
        }
        let mut extra = canonical;
        extra
            .as_object_mut()
            .expect("write report object")
            .insert("legacy_terminal_kind".to_owned(), "Applied".into());
        let _error = validate_prepared_write_report(
            &extra,
            &admitted,
            "pre_edge",
            "faucet",
            &idempotency_key,
            "Prepared",
            None,
        )
        .expect_err("unknown prepared write report fields must fail closed");
    }

    #[test]
    fn prepared_final_canary_report_requires_its_exact_operation_name() {
        let admitted = admitted_reset_fixture();
        let idempotency_key = "3".repeat(64);
        let canonical = prepared_write_report_fixture(&admitted, "write_canary", &idempotency_key);
        validate_prepared_write_report(
            &canonical,
            &admitted,
            "pre_edge",
            "write_canary",
            &idempotency_key,
            "Prepared",
            None,
        )
        .expect("canonical final-canary report");
        for field in PREPARED_FINAL_CANARY_REPORT_FIELDS {
            let mut missing = canonical.clone();
            missing
                .as_object_mut()
                .expect("final-canary report object")
                .remove(*field);
            let _error = validate_prepared_write_report(
                &missing,
                &admitted,
                "pre_edge",
                "write_canary",
                &idempotency_key,
                "Prepared",
                None,
            )
            .expect_err("every final-canary report field is mandatory");
        }
        let mut report = canonical;
        report
            .as_object_mut()
            .expect("final-canary report object")
            .insert("operation".to_owned(), "write_canary".into());
        let _error = validate_prepared_write_report(
            &report,
            &admitted,
            "pre_edge",
            "write_canary",
            &idempotency_key,
            "Prepared",
            None,
        )
        .expect_err("retired operation spelling must fail closed");
    }

    #[test]
    fn prepared_inrou_report_rejects_every_missing_or_extra_v1_field() {
        let admitted = admitted_reset_fixture();
        let idempotency_key = "3".repeat(64);
        let canonical = prepared_inrou_report_fixture(&admitted, &idempotency_key);
        validate_prepared_inrou_report(
            &canonical,
            &admitted,
            "pre_edge",
            "inrou_bundle_pin",
            &idempotency_key,
            "Prepared",
            None,
        )
        .expect("canonical prepared Inrou report");
        for field in PREPARED_INROU_REPORT_FIELDS {
            let mut missing = canonical.clone();
            missing
                .as_object_mut()
                .expect("Inrou report object")
                .remove(*field);
            let _error = validate_prepared_inrou_report(
                &missing,
                &admitted,
                "pre_edge",
                "inrou_bundle_pin",
                &idempotency_key,
                "Prepared",
                None,
            )
            .expect_err("every prepared Inrou report field is mandatory");
        }
        let mut extra = canonical;
        extra
            .as_object_mut()
            .expect("Inrou report object")
            .insert("submitted_tx_hash".to_owned(), "4".repeat(64).into());
        let _error = validate_prepared_inrou_report(
            &extra,
            &admitted,
            "pre_edge",
            "inrou_bundle_pin",
            &idempotency_key,
            "Prepared",
            None,
        )
        .expect_err("retired prepared Inrou report fields must fail closed");
    }

    #[test]
    fn report_evidence_classes_and_durable_json_are_exact() {
        validate_report_evidence_token("Rejected", &["Rejected", "Expired"], "rejection")
            .expect("canonical rejection class");
        for invalid in ["", "ledger_rejected", "Rejected\n"] {
            let _error =
                validate_report_evidence_token(invalid, &["Rejected", "Expired"], "rejection")
                    .expect_err("synthetic, unbounded, or noncanonical evidence must fail");
        }
        let _error =
            validate_report_evidence_token(&"R".repeat(33), &["Rejected", "Expired"], "rejection")
                .expect_err("unbounded evidence must fail");
        for evidence in PREPARED_INROU_REJECTED_EVIDENCE {
            validate_report_evidence_token(
                evidence,
                PREPARED_INROU_REJECTED_EVIDENCE,
                "Inrou rejection",
            )
            .expect("producer-compatible Inrou rejection evidence");
        }
        for retired in ["execution_expired_before_submit", "submitted_hash_mismatch"] {
            let _error = validate_report_evidence_token(
                retired,
                PREPARED_INROU_REJECTED_EVIDENCE,
                "Inrou rejection",
            )
            .expect_err("retired Inrou rejection spelling must fail closed");
        }

        let report = norito::json!({"status": "ok", "evidence": "Rejected"});
        let pretty = json::to_json_pretty(&report).expect("pretty report");
        let canonical = canonical_json_report_bytes(&report).expect("canonical durable report");
        assert_ne!(canonical, pretty.into_bytes());
        assert_eq!(canonical.last(), Some(&b'\n'));
        assert_eq!(
            json::from_slice::<norito::json::Value>(&canonical).expect("canonical report JSON"),
            report
        );
    }

    fn exact_inrou_check_report_fixture(admitted: &AdmittedReset) -> norito::json::Value {
        let canary = &admitted.inventory.inrou_canary;
        let replicas = (1_u64..=4)
            .map(|slot| {
                norito::json!({
                    "replica_slot": slot,
                    "identity": (format!("{}:replica:{slot}", canary.service_name)),
                    "response_sha256": (format!("{slot}").repeat(64)),
                })
            })
            .collect::<Vec<_>>();
        norito::json!({
            "command": "taira_inrou_check",
            "status": "ok",
            "public_root": (canary.public_root.clone()),
            "checks": [
                {
                    "name": "inrou_authoritative_status",
                    "http_status": 200,
                    "ok": true,
                    "detail": "active_adverts=4, hosted_replicas=4",
                },
                {
                    "name": "inrou_public_routes",
                    "http_status": 200,
                    "ok": true,
                    "detail": "observed deterministic identities for replica slots 1, 2, 3, and 4",
                },
            ],
            "warnings": [],
            "failures": [],
            "service_name": (canary.service_name.clone()),
            "service_version": (canary.service_version.clone()),
            "route_host": (canary.route_host.clone()),
            "route_path": (format!("{}{}", canary.route_path_prefix, canary.healthcheck_path)),
            "active_host_adverts": 4,
            "hosted_replica_count": 4,
            "bundle_hash": (canary.bundle_hash.clone()),
            "bundle_content_cid": (canary.bundle_content_cid.clone()),
            "bundle_manifest_digest_hex": (canary.bundle_manifest_digest_hex.clone()),
            "guest_content_cid": (canary.guest_content_cid.clone()),
            "guest_manifest_digest_hex": (canary.guest_manifest_digest_hex.clone()),
            "container_manifest_hash": (canary.container_manifest_hash.clone()),
            "service_manifest_hash": (canary.service_manifest_hash.clone()),
            "observed_at_unix_ms": 1,
            "replica_identities": replicas,
        })
    }

    #[test]
    fn fresh_and_retained_inrou_checks_have_distinct_exact_v1_schemas() {
        let admitted = admitted_reset_fixture();
        let fresh = exact_inrou_check_report_fixture(&admitted);
        validate_fresh_inrou_check_report(&fresh, &admitted)
            .expect("exact fresh Inrou check report");
        let retained = canonical_local_report(&fresh).expect("retained Inrou check report");
        validate_retained_inrou_check_report(&retained, &admitted)
            .expect("exact retained Inrou check report");
        let _error = validate_fresh_inrou_check_report(&retained, &admitted)
            .expect_err("retained schema cannot masquerade as fresh evidence");
        let _error = validate_retained_inrou_check_report(&fresh, &admitted)
            .expect_err("fresh timestamp is outside the retained schema");

        for field in FRESH_INROU_CHECK_REPORT_FIELDS {
            let mut missing = fresh.clone();
            missing
                .as_object_mut()
                .expect("Inrou check object")
                .remove(*field);
            let _error = validate_fresh_inrou_check_report(&missing, &admitted)
                .expect_err("every fresh Inrou check field is mandatory");
        }
        let mut extra = fresh;
        extra
            .as_object_mut()
            .expect("Inrou check object")
            .insert("mutation_mode".to_owned(), "deploy".into());
        let _error = validate_fresh_inrou_check_report(&extra, &admitted)
            .expect_err("retired mixed-mode fields must fail closed");
    }

    fn exact_doctor_report_fixture(public_root: &str) -> norito::json::Value {
        let checks = DOCTOR_EXPECTED_CHECKS
            .iter()
            .map(|&(name, http_status, detail)| {
                let mut check = norito::json::Map::new();
                check.insert("name".to_owned(), name.into());
                check.insert("http_status".to_owned(), http_status.into());
                check.insert("ok".to_owned(), true.into());
                if let Some(detail) = detail {
                    check.insert("detail".to_owned(), detail.into());
                }
                norito::json::Value::Object(check)
            })
            .collect::<Vec<_>>();
        norito::json!({
            "command": "taira_doctor",
            "status": "ok",
            "public_root": public_root,
            "checks": checks,
            "warnings": [],
            "failures": [],
        })
    }

    #[test]
    fn doctor_report_requires_the_exact_first_release_check_surface() {
        let public_root = "https://taira.sora.org";
        let canonical = exact_doctor_report_fixture(public_root);
        validate_doctor_report(&canonical, public_root).expect("exact doctor report");

        let mut sparse = canonical.clone();
        sparse
            .as_object_mut()
            .expect("doctor object")
            .remove("warnings");
        let _error = validate_doctor_report(&sparse, public_root)
            .expect_err("sparse doctor report must fail closed");

        let mut extra = canonical.clone();
        extra
            .as_object_mut()
            .expect("doctor object")
            .insert("legacy_routes".to_owned(), norito::json!([]));
        let _error = validate_doctor_report(&extra, public_root)
            .expect_err("unknown doctor report fields must fail closed");

        let mut nonfinal_mcp = canonical.clone();
        let mcp_get = nonfinal_mcp
            .as_object_mut()
            .and_then(|root| root.get_mut("checks"))
            .and_then(norito::json::Value::as_array_mut)
            .and_then(|checks| {
                checks.iter_mut().find(|check| {
                    check
                        .as_object()
                        .and_then(|check| check.get("name"))
                        .and_then(norito::json::Value::as_str)
                        == Some("mcp_get")
                })
            })
            .and_then(norito::json::Value::as_object_mut)
            .expect("MCP GET doctor check");
        mcp_get.insert("http_status".to_owned(), 204_u64.into());
        let _error = validate_doctor_report(&nonfinal_mcp, public_root)
            .expect_err("MCP GET must require exact HTTP 200");

        let mut substituted = canonical;
        substituted
            .as_object_mut()
            .and_then(|root| root.get_mut("checks"))
            .and_then(norito::json::Value::as_array_mut)
            .and_then(|checks| checks.first_mut())
            .and_then(norito::json::Value::as_object_mut)
            .expect("first doctor check")
            .insert("name".to_owned(), "health".into());
        let _error = validate_doctor_report(&substituted, public_root)
            .expect_err("substituted doctor route name must fail closed");
    }

    #[test]
    fn remote_command_grammar_has_one_fixed_dispatcher_shape() {
        assert!(
            validate_remote_command(&format!("{FIXED_DISPATCHER} {HOST_DISPATCH_SUFFIX}")).is_ok()
        );
        assert!(validate_remote_command("/bin/sh -c anything").is_err());
        assert!(
            validate_remote_command(
                "/fixed/dispatcher;id taira public-reset host-dispatch --protocol v1"
            )
            .is_err()
        );
    }

    #[test]
    fn child_idempotency_uses_exact_length_framed_formula_and_unique_kinds() {
        let nonce = "0123456789abcdef0123456789abcdef";
        assert_eq!(
            child_mutation_idempotency_key(nonce, "pre_edge", "onboarding"),
            "bc5ab74351c08661cbabe715feee557e893475a58d964ce92d3f4dc4d9eb803a"
        );
        let keys = [
            "onboarding",
            "faucet",
            "write_canary",
            "inrou_bundle_pin",
            "inrou_guest_pin",
            "inrou_canary",
        ]
        .map(|kind| child_mutation_idempotency_key(nonce, "pre_edge", kind));
        assert_eq!(keys.iter().collect::<BTreeSet<_>>().len(), keys.len());
    }

    #[test]
    fn prepared_operation_identity_has_one_exact_label_per_child() {
        assert_eq!(
            prepared_operation_identity("write_canary").expect("write canary identity"),
            ("final_canary", "final_canary")
        );
        assert_eq!(
            prepared_operation_identity("inrou_bundle_pin").expect("bundle identity"),
            ("inrou_bundle_pin", "bundle_pin")
        );
        assert_eq!(
            prepared_operation_identity("inrou_guest_pin").expect("guest identity"),
            ("inrou_guest_pin", "guest_pin")
        );
        assert_eq!(
            prepared_operation_identity("inrou_canary").expect("service identity"),
            ("inrou_canary", "service_mutation")
        );
        assert!(prepared_operation_identity("final_canary").is_err());
    }

    fn authenticated_proof_required_fixture(
        public_root: &str,
    ) -> (HostAdmission, PreparedMutationV1, Vec<u8>, String, String) {
        let fixture: norito::json::Value = json::from_str(include_str!(
            "../../../fixtures/prepared_transactions/prepared_transaction_signature_v1.json"
        ))
        .expect("prepared signature fixture");
        let vectors = fixture
            .as_object()
            .and_then(|root| root.get("vectors"))
            .and_then(norito::json::Value::as_array)
            .expect("fixture vectors");
        let vector = |name: &str| {
            vectors
                .iter()
                .find(|vector| {
                    vector
                        .as_object()
                        .and_then(|vector| vector.get("name"))
                        .and_then(norito::json::Value::as_str)
                        == Some(name)
                })
                .and_then(norito::json::Value::as_object)
                .expect("named fixture vector")
        };
        let prepared_vector = vector("onboarding_prepared");
        let proof_vector = vector("onboarding_proof_required");
        let receipt = prepared_vector
            .get("response")
            .and_then(norito::json::Value::as_object)
            .and_then(|response| response.get("receipt"))
            .cloned()
            .expect("fixture onboarding receipt");
        let result = proof_vector
            .get("response")
            .cloned()
            .expect("fixture proof-required result");
        let typed_receipt: AccountOnboardingPlanReceiptV1 =
            json::from_value(receipt.clone()).expect("typed fixture receipt");
        let typed_result: AccountOnboardingProofRequiredPrepareResponseV1 =
            json::from_value(result.clone()).expect("typed fixture proof result");
        let network_id_value = proof_vector.get("network_id").expect("fixture network id");
        let network_id: NetworkId =
            json::from_value(network_id_value.clone()).expect("typed fixture network id");
        let network_id_literal = network_id_value
            .as_str()
            .expect("string fixture network id");

        let mut admitted = progress_admission();
        admitted.inventory.chain_id = "fixture-chain".to_owned();
        admitted.inventory.next_genesis_hash = hex::encode(network_id.as_bytes());
        admitted.inventory.inrou_canary.public_root = public_root.to_owned();
        admitted.inventory.canary_onboarding_request = typed_receipt.body.request.clone();
        admitted.inventory.authorization_nonce = typed_result.binding.authorization_nonce.clone();
        admitted.authorization_sha256 = typed_result.binding.authorization_sha256.clone();
        admitted.authorization.claims.authorization_nonce =
            typed_result.binding.authorization_nonce.clone();
        admitted.authorization.claims.execution_expires_at_unix_ms =
            typed_result.binding.execution_expires_at_unix_ms;
        admitted.request.authorization_semantic_sha256 = admitted.authorization_sha256.clone();
        admitted.request.mutation_kind = typed_result.binding.kind.clone();
        admitted.request.mutation_phase = typed_result.binding.phase.clone();
        admitted.request.mutation_idempotency_key = typed_result.binding.idempotency_key.clone();
        admitted.action_deadline = Instant::now() + Duration::from_secs(10);
        let inventory_bytes = json::to_vec(&admitted.inventory).expect("fixture inventory");
        admitted.inventory_sha256 = sha256_hex(&inventory_bytes);

        let envelope = norito::json!({
            "schema": "iroha.taira.prepared-mutation-envelope.v1",
            "binding": (json::to_value(&typed_result.binding).expect("fixture binding")),
            "public_root": public_root,
            "chain_id": (admitted.inventory.chain_id.clone()),
            "network_id": network_id_literal,
            "authority": (typed_receipt.body.request.account_id.clone()),
            "operation": {
                "kind": "onboarding_proof_required",
                "envelope": {
                    "schema": "iroha.taira.prepared-onboarding-proof-required.v1",
                    "receipt": receipt,
                    "result": result,
                }
            }
        });
        let mut bytes = json::to_json(&envelope)
            .expect("canonical proof-required envelope")
            .into_bytes();
        bytes.push(b'\n');
        let (digest, transaction_hash, operation) = validate_prepared_mutation_envelope(
            &admitted,
            &bytes,
            PreparedMutationLifetimeCheck::LiveForward,
        )
        .expect("authenticated fixture proof-required envelope");
        let prepared = PreparedMutationV1 {
            schema: PREPARED_MUTATION_SCHEMA_V1.to_owned(),
            inventory_sha256: admitted.inventory_sha256.clone(),
            authorization_sha256: admitted.authorization_sha256.clone(),
            authorization_nonce: admitted.inventory.authorization_nonce.clone(),
            kind: admitted.request.mutation_kind.clone(),
            phase: admitted.request.mutation_phase.clone(),
            idempotency_key: admitted.request.mutation_idempotency_key.clone(),
            operation,
            prepared_base64: BASE64.encode(&bytes),
            prepared_sha256: digest,
            transaction_hash,
        };
        (
            admitted,
            prepared,
            bytes,
            typed_result.account_id,
            typed_result.alias,
        )
    }

    fn proof_required_evidence(admitted: &HostAdmission, prepared: &PreparedMutationV1) -> Vec<u8> {
        let envelope = BASE64
            .decode(&prepared.prepared_base64)
            .expect("prepared envelope base64");
        let result = prepared_onboarding_proof_required_result(&envelope)
            .expect("typed proof-required result");
        let report = norito::json!({
            "command": "taira_write_canary",
            "status": "ok",
            "public_root": (admitted.inventory.inrou_canary.public_root.clone()),
            "checks": [],
            "warnings": [],
            "failures": [],
            "authorization_sha256": (admitted.authorization_sha256.clone()),
            "authorization_nonce": (admitted.inventory.authorization_nonce.clone()),
            "mutation_kind": "onboarding",
            "mutation_phase": (prepared.phase.clone()),
            "idempotency_key": (prepared.idempotency_key.clone()),
            "operation": "onboarding",
            "recovery_outcome": "Applied",
            "prepared_envelope_sha256": (prepared.prepared_sha256.clone()),
            "prepared_envelope_size": (u64::try_from(envelope.len()).expect("bounded envelope")),
            "transaction_hash_hex": null,
            "applied_block_height": null,
            "evidence": (result.semantic_hash_hex),
            "execution_expires_at_unix_ms": (admitted.authorization.claims.execution_expires_at_unix_ms),
        });
        let mut bytes = json::to_json(&report)
            .expect("canonical proof-required evidence")
            .into_bytes();
        bytes.push(b'\n');
        bytes
    }

    #[test]
    fn retained_proof_required_pending_report_accepts_only_live_state_classes() {
        let (host_admission, prepared, bytes, _, _) =
            authenticated_proof_required_fixture("https://taira.sora.org");
        let mut admitted = admitted_reset_fixture();
        admitted.inventory = host_admission.inventory.clone();
        admitted.inventory_sha256 = host_admission.inventory_sha256.clone();
        admitted.authorization = host_admission.authorization.clone();
        admitted.authorization_sha256 = host_admission.authorization_sha256.clone();
        let retained = RetainedPreparedMutation {
            state: "prepared".to_owned(),
            bytes: bytes.clone(),
            sha256: prepared.prepared_sha256.clone(),
            transaction_hash: String::new(),
        };
        for evidence in ["OnboardingAliasConflict", "OnboardingStateAbsent"] {
            let report = norito::json!({
                "command": "taira_write_canary",
                "status": "ok",
                "public_root": (admitted.inventory.inrou_canary.public_root.clone()),
                "checks": [],
                "warnings": [],
                "failures": [],
                "authorization_sha256": (admitted.authorization_sha256.clone()),
                "authorization_nonce": (admitted.inventory.authorization_nonce.clone()),
                "mutation_kind": "onboarding",
                "mutation_phase": (prepared.phase.clone()),
                "idempotency_key": (prepared.idempotency_key.clone()),
                "operation": "onboarding",
                "transaction_hash_hex": null,
                "prepared_envelope_sha256": (prepared.prepared_sha256.clone()),
                "prepared_envelope_size": (u64::try_from(bytes.len()).expect("bounded envelope")),
                "recovery_outcome": "Pending",
                "applied_block_height": null,
                "evidence": evidence,
                "execution_expires_at_unix_ms": (admitted.authorization.claims.execution_expires_at_unix_ms),
            });
            validate_prepared_write_report(
                &report,
                &admitted,
                &prepared.phase,
                "onboarding",
                &prepared.idempotency_key,
                "Pending",
                Some(&retained),
            )
            .expect("retained proof-required Pending state remains nonterminal");
        }
    }

    fn proof_required_network_id(prepared: &PreparedMutationV1) -> NetworkId {
        let envelope = BASE64
            .decode(&prepared.prepared_base64)
            .expect("prepared envelope base64");
        let value: norito::json::Value =
            json::from_slice(&envelope).expect("prepared envelope JSON");
        json::from_value(
            value
                .as_object()
                .and_then(|root| root.get("network_id"))
                .cloned()
                .expect("prepared envelope network identity"),
        )
        .expect("typed prepared network identity")
    }

    fn atomic_current_state_response(
        prepared: &PreparedMutationV1,
        account_id: &str,
        alias: &str,
    ) -> AccountOnboardingCurrentStateResponseV1 {
        AccountOnboardingCurrentStateResponseV1 {
            version: AccountOnboardingCurrentStateResponseV1::VERSION,
            network_id: proof_required_network_id(prepared),
            account_id: account_id.to_owned(),
            alias: alias.to_owned(),
            account_exists: true,
            alias_target_account_id: Some(account_id.to_owned()),
            observed_block_height: 7,
            observed_block_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"atomic-onboarding-current-state-fixture",
            )),
        }
    }

    fn spawn_atomic_proof_server(
        status: u16,
        body: norito::json::Value,
    ) -> (String, std::thread::JoinHandle<String>) {
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind proof server");
        let address = listener.local_addr().expect("proof server address");
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept atomic proof read");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("set proof read timeout");
            let mut request = Vec::new();
            let mut chunk = [0_u8; 4096];
            loop {
                let read = stream.read(&mut chunk).expect("read proof request");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&chunk[..read]);
                let Some(header_end) = request.windows(4).position(|part| part == b"\r\n\r\n")
                else {
                    continue;
                };
                let header_end = header_end + 4;
                let headers = String::from_utf8_lossy(&request[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().ok())
                            .flatten()
                    })
                    .unwrap_or(0);
                if request.len() >= header_end + content_length {
                    break;
                }
            }
            let rendered = json::to_json(&body).expect("render atomic proof response");
            let reason = if status == 200 {
                "OK"
            } else {
                "Service Unavailable"
            };
            write!(
                stream,
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{rendered}",
                rendered.len()
            )
            .expect("write proof response");
            String::from_utf8(request).expect("proof request UTF-8")
        });
        (format!("http://{address}"), handle)
    }

    #[test]
    fn proof_required_receipt_rejects_every_omitted_v1_onboarding_slot() {
        let (admitted, _, bytes, _, _) =
            authenticated_proof_required_fixture("https://taira.sora.org");
        for receipt_path in [
            &["body", "request", "permissions"][..],
            &["body", "owner_auto_renew_instruction"][..],
            &["body", "acquisition", "pricing_class_hint"][..],
            &["body", "resource", "quote"][..],
            &["body", "resource", "instruction_index"][..],
        ] {
            let mut envelope: norito::json::Value =
                json::from_slice(&bytes).expect("fixture envelope JSON");
            let mut receipt = envelope
                .as_object_mut()
                .and_then(|root| root.get_mut("operation"))
                .and_then(norito::json::Value::as_object_mut)
                .and_then(|operation| operation.get_mut("envelope"))
                .and_then(norito::json::Value::as_object_mut)
                .and_then(|proof| proof.get_mut("receipt"))
                .and_then(norito::json::Value::as_object_mut)
                .expect("fixture receipt object");
            for segment in &receipt_path[..receipt_path.len() - 1] {
                receipt = receipt
                    .get_mut(*segment)
                    .and_then(norito::json::Value::as_object_mut)
                    .expect("nested fixture receipt object");
            }
            let field = receipt_path[receipt_path.len() - 1];
            assert!(receipt.remove(field).is_some(), "fixture contains {field}");
            let mut missing = json::to_json(&envelope)
                .expect("canonical omitted-field envelope")
                .into_bytes();
            missing.push(b'\n');
            let error = validate_prepared_mutation_envelope(
                &admitted,
                &missing,
                PreparedMutationLifetimeCheck::LiveForward,
            )
            .expect_err("an omitted V1 onboarding receipt field must fail closed");
            assert!(
                format!("{error:#}").contains("receipt is not exact typed V1 JSON"),
                "unexpected rejection for {}: {error:#}",
                receipt_path.join(".")
            );
        }
    }

    #[test]
    fn proof_required_rejects_a_forged_server_signature() {
        let (admitted, _, bytes, _, _) =
            authenticated_proof_required_fixture("https://taira.sora.org");
        let mut envelope: norito::json::Value =
            json::from_slice(&bytes).expect("fixture envelope JSON");
        let result = envelope
            .as_object_mut()
            .and_then(|root| root.get_mut("operation"))
            .and_then(norito::json::Value::as_object_mut)
            .and_then(|operation| operation.get_mut("envelope"))
            .and_then(norito::json::Value::as_object_mut)
            .and_then(|proof| proof.get_mut("result"))
            .and_then(norito::json::Value::as_object_mut)
            .expect("fixture proof result");
        let mut forged_signature = result
            .get("server_signature")
            .and_then(norito::json::Value::as_str)
            .expect("fixture server signature")
            .as_bytes()
            .to_vec();
        forged_signature[0] = if forged_signature[0] == b'A' {
            b'B'
        } else {
            b'A'
        };
        result.insert(
            "server_signature".to_owned(),
            norito::json::Value::from(
                String::from_utf8(forged_signature).expect("forged signature remains ASCII"),
            ),
        );
        let mut forged = json::to_json(&envelope)
            .expect("canonical forged envelope")
            .into_bytes();
        forged.push(b'\n');
        let error = validate_prepared_mutation_envelope(
            &admitted,
            &forged,
            PreparedMutationLifetimeCheck::LiveForward,
        )
        .expect_err("forged proof-required signature must fail");
        assert!(
            error.to_string().contains("authentication failed"),
            "unexpected rejection: {error:?}"
        );
    }

    #[test]
    fn proof_required_rejects_network_and_complete_request_substitution() {
        let (admitted, _, bytes, _, _) =
            authenticated_proof_required_fixture("https://taira.sora.org");
        let mut wrong_network = progress_admission();
        wrong_network.inventory = admitted.inventory.clone();
        wrong_network.inventory.next_genesis_hash = "00".repeat(32);
        wrong_network.authorization_sha256 = admitted.authorization_sha256.clone();
        wrong_network.authorization = admitted.authorization.clone();
        wrong_network.request = admitted.request.clone();
        let _ = validate_prepared_mutation_envelope(
            &wrong_network,
            &bytes,
            PreparedMutationLifetimeCheck::LiveForward,
        )
        .expect_err("another genesis identity must fail closed");

        let mut wrong_request = admitted;
        wrong_request
            .inventory
            .canary_onboarding_request
            .permissions
            .push("CanReadAccounts".to_owned());
        let _ = validate_prepared_mutation_envelope(
            &wrong_request,
            &bytes,
            PreparedMutationLifetimeCheck::LiveForward,
        )
        .expect_err("another complete original request must fail closed");
    }

    fn exact_atomic_state_fixture() -> (AccountOnboardingCurrentStateResponseV1, String, String) {
        let (_, prepared, _, account_id, alias) =
            authenticated_proof_required_fixture("http://127.0.0.1:1");
        (
            atomic_current_state_response(&prepared, &account_id, &alias),
            account_id,
            alias,
        )
    }

    fn exercise_atomic_proof_response(
        body: norito::json::Value,
        execution_expired: bool,
    ) -> (Result<String>, String) {
        let (public_root, server) = spawn_atomic_proof_server(200, body);
        let (mut admitted, prepared, _, _, _) = authenticated_proof_required_fixture(&public_root);
        admitted.execution_expired = execution_expired;
        let evidence = proof_required_evidence(&admitted, &prepared);
        let result =
            validate_prepared_mutation_proof_required_evidence(&admitted, &prepared, &evidence);
        (result, server.join().expect("atomic proof server joins"))
    }

    fn assert_exact_atomic_proof_post(request: &str, account_id: &str, alias: &str) {
        assert!(request.starts_with("POST /v1/accounts/onboarding/current-state HTTP/1.1\r\n"));
        assert!(!request.contains("GET /status"));
        assert!(!request.contains("GET /v1/accounts/"));
        assert!(!request.contains("POST /v1/aliases/resolve"));
        let (_, body) = request
            .split_once("\r\n\r\n")
            .expect("atomic proof request has a body separator");
        let decoded: AccountOnboardingCurrentStateRequestV1 =
            json::from_str(body).expect("atomic proof request is exact typed V1 JSON");
        assert_eq!(
            decoded.version,
            AccountOnboardingCurrentStateRequestV1::VERSION
        );
        assert_eq!(decoded.account_id, account_id);
        assert_eq!(decoded.alias, alias);
    }

    #[test]
    fn proof_required_reopens_with_one_atomic_post_even_after_expiry() {
        let (response, account_id, alias) = exact_atomic_state_fixture();
        let (result, request) = exercise_atomic_proof_response(
            json::to_value(&response).expect("encode exact atomic response"),
            true,
        );
        result.expect("one atomic snapshot proves the exact no-op");
        assert_exact_atomic_proof_post(&request, &account_id, &alias);
    }

    #[test]
    fn proof_required_atomic_state_rejects_network_version_and_identity_substitution() {
        let (response, account_id, alias) = exact_atomic_state_fixture();
        let foreign_key =
            iroha_crypto::KeyPair::try_from_seed(vec![0x76; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("foreign key");
        let foreign_account = AccountId::new(foreign_key.public_key().clone()).to_string();
        let mut candidates = Vec::new();

        let mut foreign_network = response.clone();
        foreign_network.network_id = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"foreign-atomic-network")),
        );
        candidates.push(foreign_network);
        let mut wrong_version = response.clone();
        wrong_version.version = AccountOnboardingCurrentStateResponseV1::VERSION + 1;
        candidates.push(wrong_version);
        let mut wrong_account = response.clone();
        wrong_account.account_id = foreign_account;
        candidates.push(wrong_account);
        let mut wrong_alias = response;
        wrong_alias.alias = "other@banka.paynet".to_owned();
        candidates.push(wrong_alias);

        for candidate in candidates {
            let (result, request) = exercise_atomic_proof_response(
                json::to_value(&candidate).expect("encode substituted atomic response"),
                false,
            );
            let _error =
                result.expect_err("atomic network, version, and identity substitution must fail");
            assert_exact_atomic_proof_post(&request, &account_id, &alias);
        }
    }

    #[test]
    fn proof_required_atomic_state_rejects_account_and_alias_absence() {
        let (response, account_id, alias) = exact_atomic_state_fixture();
        let mut account_missing = response.clone();
        account_missing.account_exists = false;
        account_missing.alias_target_account_id = None;
        let mut alias_missing = response;
        alias_missing.alias_target_account_id = None;

        for (candidate, expected) in [
            (account_missing, "exact account absent"),
            (alias_missing, "exact alias absent"),
        ] {
            let (result, request) = exercise_atomic_proof_response(
                json::to_value(&candidate).expect("encode absent atomic response"),
                false,
            );
            let error = result.expect_err("absent account or alias must remain nonterminal");
            assert!(
                error.to_string().contains(expected),
                "unexpected error: {error:#}"
            );
            assert_exact_atomic_proof_post(&request, &account_id, &alias);
        }
    }

    #[test]
    fn proof_required_atomic_state_rejects_an_alias_target_conflict() {
        let (mut response, account_id, alias) = exact_atomic_state_fixture();
        let conflict_key =
            iroha_crypto::KeyPair::try_from_seed(vec![0x77; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("conflict key");
        response.alias_target_account_id =
            Some(AccountId::new(conflict_key.public_key().clone()).to_string());
        let (result, request) = exercise_atomic_proof_response(
            json::to_value(&response).expect("encode conflicting atomic response"),
            false,
        );
        let error = result.expect_err("conflicting alias target must remain nonterminal");
        assert!(error.to_string().contains("alias target conflict"));
        assert_exact_atomic_proof_post(&request, &account_id, &alias);
    }

    #[test]
    fn proof_required_atomic_state_rejects_zero_or_untyped_anchor() {
        let (response, account_id, alias) = exact_atomic_state_fixture();
        let mut zero_height = response.clone();
        zero_height.observed_block_height = 0;
        let (result, request) = exercise_atomic_proof_response(
            json::to_value(&zero_height).expect("encode zero-height atomic response"),
            false,
        );
        let error = result.expect_err("zero-height atomic response must fail");
        assert!(error.to_string().contains("zero committed height"));
        assert_exact_atomic_proof_post(&request, &account_id, &alias);

        let mut invalid_hash =
            json::to_value(&response).expect("encode invalid-hash atomic response");
        invalid_hash
            .as_object_mut()
            .expect("atomic response object")
            .insert(
                "observed_block_hash".to_owned(),
                norito::json::Value::from("not-a-typed-block-hash"),
            );
        let (result, request) = exercise_atomic_proof_response(invalid_hash, false);
        let _error = result.expect_err("untyped atomic block hash must fail closed");
        assert_exact_atomic_proof_post(&request, &account_id, &alias);
    }

    #[test]
    fn recovery_intent_exposes_every_ordered_child_mutation() {
        let inventory = super::super::sample_inventory_fixture();
        let canary = build_recovery_intent(&inventory, ExecutionStep::Canary)
            .expect("canary recovery intent");
        assert_eq!(canary.mutations.len(), 6);
        assert_eq!(
            canary
                .mutations
                .iter()
                .map(|mutation| mutation.kind.as_str())
                .collect::<Vec<_>>(),
            [
                "onboarding",
                "faucet",
                "write_canary",
                "inrou_bundle_pin",
                "inrou_guest_pin",
                "inrou_canary",
            ]
        );
        let restart = build_recovery_intent(&inventory, ExecutionStep::RestartProof)
            .expect("restart recovery intent");
        assert_eq!(restart.mutations.len(), 16);
        for wave in 0..4 {
            assert_eq!(restart.mutations[wave * 4].kind, "host_restart");
            assert_eq!(
                restart.mutations[wave * 4 + 1..wave * 4 + 4]
                    .iter()
                    .map(|mutation| mutation.kind.as_str())
                    .collect::<Vec<_>>(),
                ["onboarding", "faucet", "write_canary"]
            );
        }
        let edge = build_recovery_intent(&inventory, ExecutionStep::EdgeVerify)
            .expect("edge recovery intent");
        assert_eq!(edge.mutations.len(), 3);
    }

    fn manager_intent_fixture() -> ManagerIntentV1 {
        ManagerIntentV1 {
            schema: MANAGER_INTENT_SCHEMA_V1.to_owned(),
            action: "restart".to_owned(),
            host_slug: "taira-validator-1".to_owned(),
            request_sha256: "11".repeat(32),
            authorization_nonce: "0123456789abcdef0123456789abcdef".to_owned(),
            boot_id: "01234567-89ab-cdef-0123-456789abcdef".to_owned(),
            operation_unit: "iroha-taira-reset-1111111111111111-taira-validator-1-restart.service"
                .to_owned(),
            verb: "restart".to_owned(),
            target_unit: "taira-validator-1.service".to_owned(),
            created_at_unix_ms: 1,
            action_deadline_unix_ms: 2,
        }
    }

    fn manager_evidence(active: &str, sub: &str, result: &str, status: &str, job: &str) -> Vec<u8> {
        format!(
            "LoadState=loaded\nActiveState={active}\nSubState={sub}\nResult={result}\nExecMainCode=exited\nExecMainStatus={status}\nInvocationID=0123456789abcdef0123456789abcdef\nExecStart={{ path=/usr/bin/systemctl ; argv[]=/usr/bin/systemctl restart taira-validator-1.service ; ignore_errors=no ; start_time=[n/a] ; stop_time=[n/a] ; pid=0 ; code=(null) ; status=0/0 }}\nJob={job}\n"
        )
        .into_bytes()
    }

    #[test]
    fn manager_evidence_stays_pending_until_exact_terminal_job() {
        let intent = manager_intent_fixture();
        assert_eq!(
            classify_manager_operation_evidence(
                &manager_evidence("activating", "start", "success", "0", "123"),
                &intent,
            )
            .expect("activating evidence"),
            ManagerOperationEvidence::Pending
        );
        assert_eq!(
            classify_manager_operation_evidence(
                &manager_evidence("active", "exited", "success", "0", ""),
                &intent,
            )
            .expect("terminal evidence"),
            ManagerOperationEvidence::Applied
        );
        assert_eq!(
            classify_manager_operation_evidence(
                &manager_evidence("failed", "failed", "exit-code", "1", ""),
                &intent,
            )
            .expect("rejected evidence"),
            ManagerOperationEvidence::Rejected
        );
    }

    #[test]
    fn manager_recovery_uses_immutable_mutation_deadline_but_observes_terminal_state() {
        let intent = manager_intent_fixture();
        let absent = manager_evidence("inactive", "dead", "success", "0", "")
            .into_iter()
            .collect::<Vec<_>>();
        let absent = String::from_utf8(absent)
            .expect("UTF-8")
            .replace("LoadState=loaded", "LoadState=not-found")
            .into_bytes();
        assert_eq!(
            classify_manager_operation_at(&absent, &intent, intent.action_deadline_unix_ms - 1,)
                .expect("pre-deadline absence remains retryable"),
            ManagerOperationEvidence::Absent
        );
        assert_eq!(
            classify_manager_operation_at(&absent, &intent, intent.action_deadline_unix_ms,)
                .expect("deadline makes proven absence terminal"),
            ManagerOperationEvidence::Rejected
        );
        assert_eq!(
            classify_manager_operation_at(
                &manager_evidence("activating", "start", "success", "0", "123"),
                &intent,
                intent.action_deadline_unix_ms + 1,
            )
            .expect("late observation of an in-flight exact unit"),
            ManagerOperationEvidence::Pending
        );
        assert_eq!(
            classify_manager_operation_at(
                &manager_evidence("active", "exited", "success", "0", ""),
                &intent,
                intent.action_deadline_unix_ms + 1,
            )
            .expect("late read-only observation may prove exact Applied"),
            ManagerOperationEvidence::Applied
        );
    }

    #[test]
    fn manager_evidence_rejects_wrong_or_duplicate_exec_identity() {
        let intent = manager_intent_fixture();
        let wrong = manager_evidence("active", "exited", "success", "0", "")
            .into_iter()
            .collect::<Vec<_>>();
        let wrong = String::from_utf8(wrong)
            .expect("UTF-8")
            .replace(" restart ", " start ")
            .into_bytes();
        let _ = classify_manager_operation_evidence(&wrong, &intent)
            .expect_err("wrong exact manager argv must fail");
        let mut duplicate = manager_evidence("active", "exited", "success", "0", "");
        duplicate.extend_from_slice(b"Result=success\n");
        let _ = classify_manager_operation_evidence(&duplicate, &intent)
            .expect_err("duplicate manager property must fail");
    }
}
