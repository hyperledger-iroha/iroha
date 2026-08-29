//! Fail-closed Torii boundary for atomic private cross-dataspace settlement.
//!
//! The HTTP layer persists only encrypted sidecars, authenticates the bundle
//! sponsor for uploads, binds auditor access to the exact request-signing key,
//! and returns explicitly allowlisted projections.  It never accepts or emits
//! auditor plaintext or private note openings.

use crate::{JsonBody, NoritoJson, SharedAppState, app_auth::VerifiedCanonicalRequest};
use axum::{
    Extension,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use iroha_core::private_settlement::{
    PrivateSettlementAvailabilityErrorV1, PrivateSettlementAvailabilitySignerV1,
    PrivateSettlementFileSidecarStoreV1, PrivateSettlementPhaseErrorV1,
    PrivateSettlementPhaseSignerV1, PrivateSettlementReconciliationCandidateV1,
    PrivateSettlementRestrictedSidecarV1, PrivateSettlementSidecarLifecycleV1,
    PrivateSettlementSidecarStoreConfigV1, PrivateSettlementSidecarStoreErrorV1,
    PrivateSettlementSidecarStoreOutcomeV1, validate_private_settlement_committee_authority_v1,
};
use iroha_core::state::StateReadOnly as _;
use iroha_crypto::{Hash, PublicKey};
use iroha_data_model::{
    isi::private_settlement::FinalizeAtomicPrivateSettlementV1,
    nexus::{
        PrivateSettlementAbortReasonV1, PrivateSettlementAuditApprovalV1, PrivateSettlementPhaseV1,
    },
    transaction::{Executable, SignedTransaction},
};
use iroha_futures::supervisor::ShutdownSignal;
use iroha_torii_shared::private_settlement_api::{
    PrivateSettlementAuditApprovalRequestV1, PrivateSettlementAuditApprovalResponseV1,
    PrivateSettlementAuditorCapsuleResponseV1, PrivateSettlementAvailabilityShareRequestV1,
    PrivateSettlementAvailabilityShareResponseV1, PrivateSettlementBundleReceiptResponseV1,
    PrivateSettlementBundleStatusResponseV1, PrivateSettlementBundleSubmitRequestV1,
    PrivateSettlementBundleSubmitResponseV1, PrivateSettlementCommitVoteRequestV1,
    PrivateSettlementCommitteeProofResponseV1, PrivateSettlementLegStatusResponseV1,
    PrivateSettlementLegUploadDispositionV1, PrivateSettlementLegUploadRequestV1,
    PrivateSettlementLegUploadResponseV1, PrivateSettlementLifecycleDtoV1,
    PrivateSettlementPhaseCertificateRequestV1, PrivateSettlementPhaseCertificateResponseV1,
    PrivateSettlementPhaseVoteResponseV1, PrivateSettlementPrepareVoteRequestV1,
};
use std::{path::PathBuf, str::FromStr as _, sync::Arc, time::Duration};

/// Kura-relative owner-only directory for encrypted private-settlement sidecars.
pub(crate) const PRIVATE_SETTLEMENT_SIDECAR_DIRECTORY_V1: &str = "private-settlement-sidecars-v1";
const PRIVATE_SETTLEMENT_RECONCILIATION_PAGE_RECORDS_V1: usize = 16;
const PRIVATE_SETTLEMENT_RECONCILIATION_MAX_PAGES_PER_TICK_V1: usize = 16;
const PRIVATE_SETTLEMENT_RECONCILIATION_INTERVAL_V1: Duration = Duration::from_secs(1);

const fn private_settlement_carrier_height_is_live_v1(
    current_height: u64,
    authority_context_height: u64,
    expiry_height: u64,
) -> bool {
    current_height >= authority_context_height && current_height < expiry_height
}

fn governed_sidecar_store_config_v1(
    config: &iroha_config::parameters::actual::NexusAtomicPrivateSettlement,
) -> Result<PrivateSettlementSidecarStoreConfigV1, PrivateSettlementSidecarStoreErrorV1> {
    let max_records = usize::try_from(config.sidecar_max_records.get())
        .map_err(|_| PrivateSettlementSidecarStoreErrorV1::ConfigurationInvalid)?;
    PrivateSettlementSidecarStoreConfigV1::new(max_records, config.sidecar_max_total_bytes.get())
}

/// Route-local durable runtime installed as an Axum extension.
#[derive(Clone)]
pub(crate) struct PrivateSettlementToriiRuntimeV1 {
    store: Option<Arc<PrivateSettlementFileSidecarStoreV1>>,
    availability_signer: Option<Arc<PrivateSettlementAvailabilitySignerV1>>,
    phase_signer: Option<Arc<PrivateSettlementPhaseSignerV1>>,
}

impl PrivateSettlementToriiRuntimeV1 {
    /// Open the durable store when governed configuration enables the path.
    ///
    /// Emergency Fast startup deliberately supplies `None` and therefore
    /// leaves every private-settlement route fail-closed until a Strict restart.
    pub(crate) fn open(
        state: &iroha_core::state::State,
        kura_root: PathBuf,
        emergency_fast: bool,
        availability_signer: Option<Arc<PrivateSettlementAvailabilitySignerV1>>,
        phase_signer: Option<Arc<PrivateSettlementPhaseSignerV1>>,
    ) -> Result<Self, PrivateSettlementSidecarStoreErrorV1> {
        let nexus = state.nexus_snapshot();
        let config = nexus.atomic_private_settlement;
        if !config.enabled || emergency_fast {
            return Ok(Self {
                store: None,
                availability_signer: None,
                phase_signer: None,
            });
        }
        if availability_signer.is_none() || phase_signer.is_none() {
            return Err(PrivateSettlementSidecarStoreErrorV1::ConfigurationInvalid);
        }
        let root = kura_root.join(PRIVATE_SETTLEMENT_SIDECAR_DIRECTORY_V1);
        let store = PrivateSettlementFileSidecarStoreV1::open(
            root,
            governed_sidecar_store_config_v1(&config)?,
        )?;
        Ok(Self {
            store: Some(Arc::new(store)),
            availability_signer,
            phase_signer,
        })
    }

    #[cfg(test)]
    pub(crate) fn from_store(store: Arc<PrivateSettlementFileSidecarStoreV1>) -> Self {
        Self {
            store: Some(store),
            availability_signer: None,
            phase_signer: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_store_and_signer(
        store: Arc<PrivateSettlementFileSidecarStoreV1>,
        availability_signer: Arc<PrivateSettlementAvailabilitySignerV1>,
    ) -> Self {
        Self {
            store: Some(store),
            availability_signer: Some(availability_signer),
            phase_signer: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn from_store_and_phase_signer(
        store: Arc<PrivateSettlementFileSidecarStoreV1>,
        phase_signer: Arc<PrivateSettlementPhaseSignerV1>,
    ) -> Self {
        Self {
            store: Some(store),
            availability_signer: None,
            phase_signer: Some(phase_signer),
        }
    }

    fn store(&self) -> Result<&PrivateSettlementFileSidecarStoreV1, Response> {
        self.store
            .as_deref()
            .ok_or_else(private_settlement_unavailable)
    }

    fn availability_signer(&self) -> Result<&PrivateSettlementAvailabilitySignerV1, Response> {
        self.availability_signer
            .as_deref()
            .ok_or_else(private_settlement_unavailable)
    }

    fn phase_signer(&self) -> Result<&PrivateSettlementPhaseSignerV1, Response> {
        self.phase_signer
            .as_deref()
            .ok_or_else(private_settlement_unavailable)
    }

    fn reconciliation_store(&self) -> Option<Arc<PrivateSettlementFileSidecarStoreV1>> {
        self.store.clone()
    }
}

#[cfg(test)]
mod governed_sidecar_store_config_tests {
    use std::num::{NonZeroU32, NonZeroU64};

    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        NetworkId,
        block::BlockHeader,
        nexus::{
            ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1, DataSpaceId, LaneId,
            PRIVATE_SETTLEMENT_XCHACHA_NONCE_BYTES_V1, PrivateSettlementAuditAadV1,
            PrivateSettlementAuditCapsuleV1, PrivateSettlementCapsulePaddingV1,
            PrivateSettlementRouteV1,
        },
    };

    use super::*;

    #[test]
    fn governed_limits_are_forwarded_exactly() {
        let mut config = iroha_config::parameters::actual::NexusAtomicPrivateSettlement::default();
        config.sidecar_max_records = NonZeroU32::new(17).expect("non-zero record bound");
        config.sidecar_max_total_bytes =
            NonZeroU64::new(23 * 1024 * 1024).expect("non-zero byte bound");

        let store = governed_sidecar_store_config_v1(&config).expect("bounded store config");
        assert_eq!(store.max_records(), 17);
        assert_eq!(store.max_total_bytes(), 23 * 1024 * 1024);
    }

    #[test]
    fn capsule_bound_counts_the_complete_canonical_envelope() {
        let capsule = PrivateSettlementAuditCapsuleV1 {
            version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
            aad: PrivateSettlementAuditAadV1 {
                network_id: NetworkId::from_genesis_hash(
                    HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"capsule bound")),
                ),
                bundle_id: Hash::new(b"capsule-bound-bundle"),
                leg_ordinal: 0,
                route: PrivateSettlementRouteV1 {
                    dataspace_id: DataSpaceId::new(7),
                    lane_id: LaneId::SINGLE,
                    lane_incarnation: Hash::new(b"capsule-bound-incarnation"),
                },
                authority_digest: Hash::new(b"capsule-bound-authority"),
                authority_context_height: 1,
                audit_policy_digest: Hash::new(b"capsule-bound-policy"),
                audit_key_epoch: 1,
                plaintext_commitment: Hash::new(b"capsule-bound-plaintext"),
            },
            padding: PrivateSettlementCapsulePaddingV1::KiB4,
            nonce: [0; PRIVATE_SETTLEMENT_XCHACHA_NONCE_BYTES_V1],
            ciphertext: vec![0; PrivateSettlementCapsulePaddingV1::KiB4.ciphertext_bytes()],
            wrapped_deks: Vec::new(),
        };
        let ciphertext_only_bound = u64::try_from(capsule.ciphertext.len()).expect("fits u64");
        assert!(
            !audit_capsule_within_canonical_bound(&capsule, ciphertext_only_bound),
            "the AAD, nonce, padding tag, and recipient inventory must count against the bound"
        );
        let canonical_bound = u64::try_from(
            norito::encode_canonical(&capsule)
                .expect("capsule encodes")
                .len(),
        )
        .expect("fits u64");
        assert!(audit_capsule_within_canonical_bound(
            &capsule,
            canonical_bound
        ));
    }

    #[test]
    fn deployment_auditor_floor_is_enforced() {
        assert!(audit_threshold_meets_governed_floor(1, 1));
        assert!(audit_threshold_meets_governed_floor(3, 2));
        assert!(!audit_threshold_meets_governed_floor(1, 2));
    }

    #[test]
    fn carrier_ingress_requires_room_for_the_next_block() {
        assert!(!private_settlement_carrier_height_is_live_v1(9, 10, 20));
        assert!(private_settlement_carrier_height_is_live_v1(10, 10, 20));
        assert!(private_settlement_carrier_height_is_live_v1(19, 10, 20));
        assert!(!private_settlement_carrier_height_is_live_v1(20, 10, 20));
        assert!(!private_settlement_carrier_height_is_live_v1(21, 10, 20));
    }

    #[test]
    fn reconciliation_page_runs_retention_pruning_even_without_candidates() {
        let directory = tempfile::tempdir().expect("temporary sidecar directory");
        let config = PrivateSettlementSidecarStoreConfigV1::new(4, 1024 * 1024)
            .expect("bounded sidecar configuration");
        let store = PrivateSettlementFileSidecarStoreV1::open(directory.path(), config)
            .expect("open empty sidecar store");

        let removed = reconcile_and_prune_private_settlement_page_v1(&store, Vec::new(), 10)
            .expect("empty reconciliation page still prunes");

        assert_eq!(removed, 0);
    }
}

struct PrivateSettlementReconciliationWorkV1 {
    payload_digest: Hash,
    receipt: Option<iroha_data_model::nexus::PrivateSettlementReceiptV1>,
    abort: Option<iroha_data_model::nexus::PrivateSettlementAbortReceiptV1>,
}

fn reconcile_and_prune_private_settlement_page_v1(
    store: &PrivateSettlementFileSidecarStoreV1,
    work: Vec<PrivateSettlementReconciliationWorkV1>,
    authoritative_height: u64,
) -> Result<usize, PrivateSettlementReconciliationFailureV1> {
    for item in work {
        store
            .reconcile_terminal_state(
                item.payload_digest,
                item.receipt.as_ref(),
                item.abort.as_ref(),
                authoritative_height,
            )
            .map_err(|_| PrivateSettlementReconciliationFailureV1::StoreRejected)?;
    }
    store
        .prune(authoritative_height)
        .map_err(|_| PrivateSettlementReconciliationFailureV1::StoreRejected)
}

fn snapshot_private_settlement_reconciliation_work_v1(
    state: &iroha_core::state::State,
    candidates: Vec<PrivateSettlementReconciliationCandidateV1>,
) -> Result<
    (u64, Vec<PrivateSettlementReconciliationWorkV1>),
    PrivateSettlementReconciliationFailureV1,
> {
    let view = state.view();
    let authoritative_height = u64::try_from(view.height())
        .map_err(|_| PrivateSettlementReconciliationFailureV1::HeightUnavailable)?;
    let world = view.world();
    let work = candidates
        .into_iter()
        .map(|candidate| PrivateSettlementReconciliationWorkV1 {
            payload_digest: candidate.payload_digest,
            receipt: world
                .private_settlement_receipt_v1(&candidate.bundle_id)
                .cloned(),
            abort: world
                .private_settlement_abort_v1(&candidate.bundle_id)
                .copied(),
        })
        .collect();
    Ok((authoritative_height, work))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PrivateSettlementReconciliationFailureV1 {
    HeightUnavailable,
    StoreRejected,
    CursorDidNotAdvance,
    BlockingWorkerFailed,
    WorkerExitedUnexpectedly,
    WorkerPanicked,
    WorkerCancelled,
}

impl PrivateSettlementReconciliationFailureV1 {
    const fn code(self) -> &'static str {
        match self {
            Self::HeightUnavailable => "height_unavailable",
            Self::StoreRejected => "store_rejected",
            Self::CursorDidNotAdvance => "cursor_did_not_advance",
            Self::BlockingWorkerFailed => "blocking_worker_failed",
            Self::WorkerExitedUnexpectedly => "worker_exited_unexpectedly",
            Self::WorkerPanicked => "worker_panicked",
            Self::WorkerCancelled => "worker_cancelled",
        }
    }
}

async fn reconcile_private_settlement_finality_tick_v1(
    store: Arc<PrivateSettlementFileSidecarStoreV1>,
    state: Arc<iroha_core::state::State>,
    mut cursor: Option<Hash>,
) -> Result<Option<Hash>, PrivateSettlementReconciliationFailureV1> {
    for _ in 0..PRIVATE_SETTLEMENT_RECONCILIATION_MAX_PAGES_PER_TICK_V1 {
        let page = store
            .reconciliation_page(cursor, PRIVATE_SETTLEMENT_RECONCILIATION_PAGE_RECORDS_V1)
            .map_err(|_| PrivateSettlementReconciliationFailureV1::StoreRejected)?;
        if page.next_cursor.is_some() && page.next_cursor == cursor {
            return Err(PrivateSettlementReconciliationFailureV1::CursorDidNotAdvance);
        }
        let (authoritative_height, work) =
            snapshot_private_settlement_reconciliation_work_v1(&state, page.candidates)?;
        let blocking_store = Arc::clone(&store);
        tokio::task::spawn_blocking(move || {
            reconcile_and_prune_private_settlement_page_v1(
                &blocking_store,
                work,
                authoritative_height,
            )
            .map(|_| ())
        })
        .await
        .map_err(|_| PrivateSettlementReconciliationFailureV1::BlockingWorkerFailed)??;
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    Ok(cursor)
}

async fn run_private_settlement_finality_reconciliation_v1(
    store: Arc<PrivateSettlementFileSidecarStoreV1>,
    state: Arc<iroha_core::state::State>,
    shutdown_signal: ShutdownSignal,
) -> Result<(), PrivateSettlementReconciliationFailureV1> {
    let mut ticker = tokio::time::interval(PRIVATE_SETTLEMENT_RECONCILIATION_INTERVAL_V1);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut cursor = None;
    loop {
        tokio::select! {
            _ = shutdown_signal.receive() => return Ok(()),
            _ = ticker.tick() => {
                cursor = reconcile_private_settlement_finality_tick_v1(
                    Arc::clone(&store),
                    Arc::clone(&state),
                    cursor,
                ).await?;
            }
        }
    }
}

/// Start the fail-closed local finality reconciler under the Torii shutdown tree.
pub(crate) fn spawn_private_settlement_finality_reconciliation_v1(
    runtime: PrivateSettlementToriiRuntimeV1,
    state: Arc<iroha_core::state::State>,
    shutdown_signal: ShutdownSignal,
) {
    let Some(store) = runtime.reconciliation_store() else {
        return;
    };
    let worker_shutdown = shutdown_signal.clone();
    let worker = tokio::spawn(run_private_settlement_finality_reconciliation_v1(
        store,
        state,
        worker_shutdown,
    ));
    tokio::spawn(async move {
        let failure = match worker.await {
            Ok(Ok(())) if shutdown_signal.is_sent() => return,
            Ok(Ok(())) => PrivateSettlementReconciliationFailureV1::WorkerExitedUnexpectedly,
            Ok(Err(failure)) => failure,
            Err(error) if error.is_panic() => {
                PrivateSettlementReconciliationFailureV1::WorkerPanicked
            }
            Err(_) => PrivateSettlementReconciliationFailureV1::WorkerCancelled,
        };
        iroha_logger::error!(
            code = failure.code(),
            "private-settlement finality reconciliation failed closed"
        );
        if !shutdown_signal.is_sent() {
            shutdown_signal.send();
        }
    });
}

#[derive(Clone, Debug, crate::json_macros::JsonSerialize)]
struct PrivateSettlementApiErrorV1 {
    code: String,
}

fn error_response(status: StatusCode, code: &'static str) -> Response {
    let mut response = JsonBody(PrivateSettlementApiErrorV1 {
        code: code.to_owned(),
    })
    .into_response();
    *response.status_mut() = status;
    response
}

fn private_settlement_unavailable() -> Response {
    error_response(StatusCode::NOT_FOUND, "private_settlement_unavailable")
}

fn map_store_error(error: PrivateSettlementSidecarStoreErrorV1) -> Response {
    match error {
        PrivateSettlementSidecarStoreErrorV1::InvalidSidecar
        | PrivateSettlementSidecarStoreErrorV1::ConfigurationInvalid => error_response(
            StatusCode::BAD_REQUEST,
            "private_settlement_invalid_request",
        ),
        PrivateSettlementSidecarStoreErrorV1::Conflict => {
            error_response(StatusCode::CONFLICT, "private_settlement_conflict")
        }
        PrivateSettlementSidecarStoreErrorV1::Unavailable => private_settlement_unavailable(),
        PrivateSettlementSidecarStoreErrorV1::InvalidTransition => error_response(
            StatusCode::CONFLICT,
            "private_settlement_invalid_transition",
        ),
        PrivateSettlementSidecarStoreErrorV1::CapacityExceeded => error_response(
            StatusCode::INSUFFICIENT_STORAGE,
            "private_settlement_capacity_exceeded",
        ),
        PrivateSettlementSidecarStoreErrorV1::StoreAlreadyOpen
        | PrivateSettlementSidecarStoreErrorV1::Corrupt
        | PrivateSettlementSidecarStoreErrorV1::Backend
        | PrivateSettlementSidecarStoreErrorV1::UnsupportedPlatform => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "private_settlement_service_unavailable",
        ),
    }
}

fn map_availability_error(error: PrivateSettlementAvailabilityErrorV1) -> Response {
    match error {
        PrivateSettlementAvailabilityErrorV1::InvalidSigner
        | PrivateSettlementAvailabilityErrorV1::Storage => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "private_settlement_service_unavailable",
        ),
        PrivateSettlementAvailabilityErrorV1::InvalidShare
        | PrivateSettlementAvailabilityErrorV1::InvalidQuorum => error_response(
            StatusCode::BAD_REQUEST,
            "private_settlement_invalid_request",
        ),
    }
}

fn map_phase_error(_: PrivateSettlementPhaseErrorV1) -> Response {
    error_response(StatusCode::BAD_REQUEST, "private_settlement_phase_rejected")
}

fn authoritative_height(app: &SharedAppState) -> Result<u64, Response> {
    u64::try_from(app.state.view().height()).map_err(|_| {
        error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            "private_settlement_height_unavailable",
        )
    })
}

fn active_config(
    app: &SharedAppState,
    height: u64,
) -> Result<iroha_config::parameters::actual::NexusAtomicPrivateSettlement, Response> {
    let config = app.state.nexus_snapshot().atomic_private_settlement;
    if !config.enabled
        || config
            .activation_height
            .is_none_or(|activation| height < activation)
    {
        return Err(private_settlement_unavailable());
    }
    let activation_height = config
        .activation_height
        .ok_or_else(private_settlement_unavailable)?;
    iroha_core::privacy_engines::atomic_private_settlement::validate_atomic_private_settlement_profile_v1()
        .map_err(|_| private_settlement_unavailable())?;
    let state = app.state.view();
    let capability = state
        .privacy_capability_snapshot_v1()
        .map_err(|_| private_settlement_unavailable())?;
    capability
        .validate()
        .map_err(|_| private_settlement_unavailable())?;
    let row = capability
        .protocols
        .iter()
        .find(|row| {
            row.protocol_id
                == iroha_data_model::privacy::PrivacyProtocolIdV1::IrohaIvmPrivateNoteStarkV1
        })
        .ok_or_else(private_settlement_unavailable)?;
    let activation = row.activation.ok_or_else(private_settlement_unavailable)?;
    let iroha_data_model::privacy::PrivacyProtocolLifecycleV1::Active(active) =
        activation.lifecycle
    else {
        return Err(private_settlement_unavailable());
    };
    let iroha_data_model::privacy::PrivacyProtocolActivationLimitsV1::IrohaIvmPrivateNoteStarkV1(
        limits,
    ) = activation.protocol_limits
    else {
        return Err(private_settlement_unavailable());
    };
    let earliest_activation = active
        .proposed_at_height
        .checked_add(config.minimum_activation_notice_blocks.get())
        .ok_or_else(private_settlement_unavailable)?;
    if capability.committed_height != height
        || activation_height < earliest_activation
        || height < earliest_activation
        || activation_height < active.activated_at_height
        || height < active.activated_at_height
        || height < active.state_since_height
        || limits.max_input_count < iroha_data_model::privacy::IVM_PRIVATE_NOTE_MAX_INPUTS_V1
        || limits.max_output_count < iroha_data_model::privacy::IVM_PRIVATE_NOTE_MAX_OUTPUTS_V1
        || config.proof_profile_version.get()
            != u16::from(iroha_data_model::nexus::ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1)
        || !config.permitted_policy_versions.contains(&u16::from(
            iroha_data_model::nexus::ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        ))
    {
        return Err(private_settlement_unavailable());
    }
    Ok(config)
}

fn lifecycle_dto(
    lifecycle: PrivateSettlementSidecarLifecycleV1,
) -> PrivateSettlementLifecycleDtoV1 {
    match lifecycle {
        PrivateSettlementSidecarLifecycleV1::Collecting => {
            PrivateSettlementLifecycleDtoV1::Collecting
        }
        PrivateSettlementSidecarLifecycleV1::Audited => PrivateSettlementLifecycleDtoV1::Audited,
        PrivateSettlementSidecarLifecycleV1::Prepared => PrivateSettlementLifecycleDtoV1::Prepared,
        PrivateSettlementSidecarLifecycleV1::CommitCertified => {
            PrivateSettlementLifecycleDtoV1::CommitCertified
        }
        PrivateSettlementSidecarLifecycleV1::Finalized => {
            PrivateSettlementLifecycleDtoV1::Finalized
        }
        PrivateSettlementSidecarLifecycleV1::Aborted => PrivateSettlementLifecycleDtoV1::Aborted,
        PrivateSettlementSidecarLifecycleV1::Expired => PrivateSettlementLifecycleDtoV1::Expired,
    }
}

fn parse_digest(literal: &str) -> Result<Hash, Response> {
    Hash::from_str(literal.trim()).map_err(|_| {
        error_response(
            StatusCode::BAD_REQUEST,
            "private_settlement_invalid_identifier",
        )
    })
}

fn audit_capsule_within_canonical_bound(
    capsule: &iroha_data_model::nexus::PrivateSettlementAuditCapsuleV1,
    maximum_bytes: u64,
) -> bool {
    norito::encode_canonical(capsule)
        .ok()
        .and_then(|encoded| u64::try_from(encoded.len()).ok())
        .is_some_and(|encoded_bytes| encoded_bytes <= maximum_bytes)
}

fn audit_threshold_meets_governed_floor(policy_min_approvals: u8, governed_floor: u16) -> bool {
    u16::from(policy_min_approvals) >= governed_floor
}

fn committee_authority_is_authoritative(
    app: &SharedAppState,
    authority_context_height: u64,
    authority: &iroha_data_model::nexus::PrivateSettlementCommitteeAuthorityV1,
) -> bool {
    let view = app.state.view();
    validate_private_settlement_committee_authority_v1(&view, authority_context_height, authority)
        .is_ok()
}

fn validate_upload_policy(
    app: &SharedAppState,
    request: &PrivateSettlementLegUploadRequestV1,
    stored_at_height: u64,
) -> Result<(), Response> {
    let config = active_config(app, stored_at_height)?;
    if !committee_authority_is_authoritative(
        app,
        request.manifest.authority_context_height,
        &request.committee_authority,
    ) || request.manifest.network_id != app.state.network_id
        || request.manifest.legs.len() > usize::from(config.max_participants.get())
        || request
            .manifest
            .expiry_height
            .checked_sub(request.manifest.authority_context_height)
            .is_none_or(|span| span > config.max_expiry_blocks.get())
        || request.payload.proof.len() as u64 > config.max_proof_bytes.get()
        || !audit_capsule_within_canonical_bound(
            &request.payload.audit_capsule,
            config.max_capsule_bytes.get(),
        )
        || !config
            .permitted_policy_versions
            .contains(&u16::from(request.audit_policy.body.version))
        || !audit_threshold_meets_governed_floor(
            request.audit_policy.body.min_approvals,
            config.default_min_auditor_approvals.get(),
        )
        || !config.capsule_padding_classes_bytes.iter().any(|class| {
            usize::try_from(class.get()).ok()
                == Some(request.payload.audit_capsule.padding.plaintext_bytes())
        })
        || request.payload.availability.body.retention_until_height
            < stored_at_height.saturating_add(config.sidecar_retention_blocks.get())
    {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "private_settlement_invalid_request",
        ));
    }
    Ok(())
}

fn validate_availability_share_policy(
    app: &SharedAppState,
    request: &PrivateSettlementAvailabilityShareRequestV1,
    stored_at_height: u64,
) -> Result<(), Response> {
    let config = active_config(app, stored_at_height)?;
    let material = &request.material;
    if !committee_authority_is_authoritative(
        app,
        material.manifest.authority_context_height,
        &material.committee_authority,
    ) || material.manifest.network_id != app.state.network_id
        || material.manifest.legs.len() > usize::from(config.max_participants.get())
        || material
            .manifest
            .expiry_height
            .checked_sub(material.manifest.authority_context_height)
            .is_none_or(|span| span > config.max_expiry_blocks.get())
        || material.proof.len() as u64 > config.max_proof_bytes.get()
        || !audit_capsule_within_canonical_bound(
            &material.audit_capsule,
            config.max_capsule_bytes.get(),
        )
        || !config
            .permitted_policy_versions
            .contains(&u16::from(material.audit_policy.body.version))
        || !audit_threshold_meets_governed_floor(
            material.audit_policy.body.min_approvals,
            config.default_min_auditor_approvals.get(),
        )
        || !config.capsule_padding_classes_bytes.iter().any(|class| {
            usize::try_from(class.get()).ok()
                == Some(material.audit_capsule.padding.plaintext_bytes())
        })
        || material.availability_body.retention_until_height
            < stored_at_height.saturating_add(config.sidecar_retention_blocks.get())
        || material.validate().is_err()
    {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "private_settlement_invalid_request",
        ));
    }
    Ok(())
}

fn validate_phase_manifest_policy(
    app: &SharedAppState,
    manifest: &iroha_data_model::nexus::AtomicPrivateSettlementV1,
    authoritative_height: u64,
) -> Result<(), Response> {
    let config = active_config(app, authoritative_height)?;
    if manifest.validate().is_err()
        || manifest.network_id != app.state.network_id
        || authoritative_height < manifest.authority_context_height
        || authoritative_height > manifest.expiry_height
        || manifest.legs.len() > usize::from(config.max_participants.get())
        || manifest
            .expiry_height
            .checked_sub(manifest.authority_context_height)
            .is_none_or(|span| span > config.max_expiry_blocks.get())
    {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "private_settlement_phase_rejected",
        ));
    }
    Ok(())
}

fn validate_phase_deadline(
    app: &SharedAppState,
    store: &PrivateSettlementFileSidecarStoreV1,
    payload_digest: Hash,
    phase: PrivateSettlementPhaseV1,
    authoritative_height: u64,
) -> Result<(), Response> {
    let config = active_config(app, authoritative_height)?;
    let status = store
        .public_status(payload_digest, authoritative_height)
        .map_err(|_| map_phase_error(PrivateSettlementPhaseErrorV1))?;
    let prepare_deadline = status
        .stored_at_height
        .saturating_add(config.audit_timeout_blocks.get())
        .saturating_add(config.prepare_timeout_blocks.get());
    let deadline = match phase {
        PrivateSettlementPhaseV1::Prepare => prepare_deadline,
        PrivateSettlementPhaseV1::Commit => {
            prepare_deadline.saturating_add(config.commit_timeout_blocks.get())
        }
    };
    if authoritative_height > deadline {
        return Err(map_phase_error(PrivateSettlementPhaseErrorV1));
    }
    Ok(())
}

/// Persist sponsor-authenticated provisional bytes and issue this node's share.
pub(crate) async fn handler_availability_share(
    State(app): State<SharedAppState>,
    Extension(runtime): Extension<PrivateSettlementToriiRuntimeV1>,
    Extension(authenticated): Extension<VerifiedCanonicalRequest>,
    NoritoJson(request): NoritoJson<PrivateSettlementAvailabilityShareRequestV1>,
) -> Response {
    let Ok(store) = runtime.store() else {
        return private_settlement_unavailable();
    };
    let Ok(signer) = runtime.availability_signer() else {
        return private_settlement_unavailable();
    };
    let height = match authoritative_height(&app) {
        Ok(height) => height,
        Err(response) => return response,
    };
    if authenticated.account != request.material.manifest.sponsor {
        return error_response(StatusCode::FORBIDDEN, "private_settlement_sponsor_required");
    }
    if let Err(response) = validate_availability_share_policy(&app, &request, height) {
        return response;
    }
    let bundle_id = request.material.manifest.bundle_id;
    let payload_digest = request.material.availability_body.payload_digest;
    let leg_ordinal = request.material.statement.leg_ordinal;
    let (outcome, share) = match signer.persist_and_sign(store, request.material, height) {
        Ok(result) => result,
        Err(error) => return map_availability_error(error),
    };
    let disposition = match outcome {
        PrivateSettlementSidecarStoreOutcomeV1::Stored => {
            PrivateSettlementLegUploadDispositionV1::Stored
        }
        PrivateSettlementSidecarStoreOutcomeV1::AlreadyStored => {
            PrivateSettlementLegUploadDispositionV1::AlreadyStored
        }
    };
    JsonBody(PrivateSettlementAvailabilityShareResponseV1 {
        bundle_id,
        payload_digest,
        leg_ordinal,
        disposition,
        share,
    })
    .into_response()
}

/// Independently validate current WSV, fsync-stage, and issue one Prepare vote.
pub(crate) async fn handler_prepare_vote(
    State(app): State<SharedAppState>,
    Extension(runtime): Extension<PrivateSettlementToriiRuntimeV1>,
    Extension(authenticated): Extension<VerifiedCanonicalRequest>,
    NoritoJson(request): NoritoJson<PrivateSettlementPrepareVoteRequestV1>,
) -> Response {
    let Ok(store) = runtime.store() else {
        return private_settlement_unavailable();
    };
    let Ok(signer) = runtime.phase_signer() else {
        return private_settlement_unavailable();
    };
    let height = match authoritative_height(&app) {
        Ok(height) => height,
        Err(response) => return response,
    };
    if authenticated.account != request.manifest.sponsor {
        return error_response(StatusCode::FORBIDDEN, "private_settlement_sponsor_required");
    }
    if let Err(response) = validate_phase_manifest_policy(&app, &request.manifest, height) {
        return response;
    }
    if let Err(response) = validate_phase_deadline(
        &app,
        store,
        request.payload_digest,
        PrivateSettlementPhaseV1::Prepare,
        height,
    ) {
        return response;
    }
    let state = app.state.view();
    let vote = match signer.prepare_vote(
        &state,
        store,
        &request.manifest,
        request.payload_digest,
        height,
    ) {
        Ok(vote) => vote,
        Err(error) => return map_phase_error(error),
    };
    JsonBody(PrivateSettlementPhaseVoteResponseV1 {
        bundle_id: vote.body.bundle_id,
        payload_digest: request.payload_digest,
        leg_ordinal: vote.body.leg_ordinal,
        vote,
    })
    .into_response()
}

/// Verify one exact complete all-Prepare barrier and issue a read-only Commit vote.
pub(crate) async fn handler_commit_vote(
    State(app): State<SharedAppState>,
    Extension(runtime): Extension<PrivateSettlementToriiRuntimeV1>,
    Extension(authenticated): Extension<VerifiedCanonicalRequest>,
    NoritoJson(request): NoritoJson<PrivateSettlementCommitVoteRequestV1>,
) -> Response {
    let Ok(store) = runtime.store() else {
        return private_settlement_unavailable();
    };
    let Ok(signer) = runtime.phase_signer() else {
        return private_settlement_unavailable();
    };
    let height = match authoritative_height(&app) {
        Ok(height) => height,
        Err(response) => return response,
    };
    if authenticated.account != request.barrier.manifest.sponsor {
        return error_response(StatusCode::FORBIDDEN, "private_settlement_sponsor_required");
    }
    if request.barrier.validate_shape().is_err() {
        return map_phase_error(PrivateSettlementPhaseErrorV1);
    }
    if let Err(response) = validate_phase_manifest_policy(&app, &request.barrier.manifest, height) {
        return response;
    }
    if let Err(response) = validate_phase_deadline(
        &app,
        store,
        request.payload_digest,
        PrivateSettlementPhaseV1::Commit,
        height,
    ) {
        return response;
    }
    let vote = match signer.commit_vote(store, request.payload_digest, &request.barrier, height) {
        Ok(vote) => vote,
        Err(error) => return map_phase_error(error),
    };
    JsonBody(PrivateSettlementPhaseVoteResponseV1 {
        bundle_id: vote.body.bundle_id,
        payload_digest: request.payload_digest,
        leg_ordinal: vote.body.leg_ordinal,
        vote,
    })
    .into_response()
}

/// Verify and fsync one aggregate Prepare or Commit certificate on a signer node.
pub(crate) async fn handler_phase_certificate(
    State(app): State<SharedAppState>,
    Extension(runtime): Extension<PrivateSettlementToriiRuntimeV1>,
    Extension(authenticated): Extension<VerifiedCanonicalRequest>,
    NoritoJson(request): NoritoJson<PrivateSettlementPhaseCertificateRequestV1>,
) -> Response {
    let Ok(store) = runtime.store() else {
        return private_settlement_unavailable();
    };
    let Ok(signer) = runtime.phase_signer() else {
        return private_settlement_unavailable();
    };
    let height = match authoritative_height(&app) {
        Ok(height) => height,
        Err(response) => return response,
    };
    if authenticated.account != request.manifest.sponsor {
        return error_response(StatusCode::FORBIDDEN, "private_settlement_sponsor_required");
    }
    if let Err(response) = validate_phase_manifest_policy(&app, &request.manifest, height) {
        return response;
    }
    let phase = request.certificate.body.phase;
    if let Err(response) =
        validate_phase_deadline(&app, store, request.payload_digest, phase, height)
    {
        return response;
    }
    let leg_ordinal = request.certificate.body.leg_ordinal;
    if signer
        .persist_certificate(
            store,
            &request.manifest,
            request.payload_digest,
            request.certificate,
            height,
        )
        .is_err()
    {
        return map_phase_error(PrivateSettlementPhaseErrorV1);
    }
    let status = match store.public_status(request.payload_digest, height) {
        Ok(status) => status,
        Err(_) => return map_phase_error(PrivateSettlementPhaseErrorV1),
    };
    let expected_lifecycle = match phase {
        PrivateSettlementPhaseV1::Prepare => PrivateSettlementSidecarLifecycleV1::Prepared,
        PrivateSettlementPhaseV1::Commit => PrivateSettlementSidecarLifecycleV1::CommitCertified,
    };
    if status.leg_ordinal != leg_ordinal || status.lifecycle != expected_lifecycle {
        return map_phase_error(PrivateSettlementPhaseErrorV1);
    }
    JsonBody(PrivateSettlementPhaseCertificateResponseV1 {
        bundle_id: status.bundle_id,
        payload_digest: status.payload_digest,
        leg_ordinal,
        phase,
        lifecycle: lifecycle_dto(status.lifecycle),
    })
    .into_response()
}

/// Persist one sponsor-authenticated encrypted leg through restricted DA.
pub(crate) async fn handler_leg_upload(
    State(app): State<SharedAppState>,
    Extension(runtime): Extension<PrivateSettlementToriiRuntimeV1>,
    Extension(authenticated): Extension<VerifiedCanonicalRequest>,
    NoritoJson(request): NoritoJson<PrivateSettlementLegUploadRequestV1>,
) -> Response {
    let Ok(store) = runtime.store() else {
        return private_settlement_unavailable();
    };
    let height = match authoritative_height(&app) {
        Ok(height) => height,
        Err(response) => return response,
    };
    if authenticated.account != request.manifest.sponsor {
        return error_response(StatusCode::FORBIDDEN, "private_settlement_sponsor_required");
    }
    if let Err(response) = validate_upload_policy(&app, &request, height) {
        return response;
    }
    let bundle_id = request.manifest.bundle_id;
    let payload_digest = request.payload.availability.body.payload_digest;
    let leg_ordinal = request.payload.statement.leg_ordinal;
    let sidecar = PrivateSettlementRestrictedSidecarV1 {
        manifest: request.manifest,
        policy: request.audit_policy,
        authority: request.committee_authority,
        payload: request.payload,
        stored_at_height: height,
    };
    let disposition = match store.promote(sidecar) {
        Ok(PrivateSettlementSidecarStoreOutcomeV1::Stored) => {
            PrivateSettlementLegUploadDispositionV1::Stored
        }
        Ok(PrivateSettlementSidecarStoreOutcomeV1::AlreadyStored) => {
            PrivateSettlementLegUploadDispositionV1::AlreadyStored
        }
        Err(error) => return map_store_error(error),
    };
    let lifecycle = match store.public_status(payload_digest, height) {
        Ok(status) => lifecycle_dto(status.lifecycle),
        Err(error) => return map_store_error(error),
    };
    JsonBody(PrivateSettlementLegUploadResponseV1 {
        bundle_id,
        payload_digest,
        leg_ordinal,
        disposition,
        lifecycle,
    })
    .into_response()
}

/// Return an allowlisted lifecycle projection for one encrypted leg.
pub(crate) async fn handler_leg_status(
    State(app): State<SharedAppState>,
    Extension(runtime): Extension<PrivateSettlementToriiRuntimeV1>,
    Path(payload_digest): Path<String>,
) -> Response {
    let Ok(store) = runtime.store() else {
        return private_settlement_unavailable();
    };
    let height = match authoritative_height(&app) {
        Ok(height) => height,
        Err(response) => return response,
    };
    if active_config(&app, height).is_err() {
        return private_settlement_unavailable();
    }
    let digest = match parse_digest(&payload_digest) {
        Ok(digest) => digest,
        Err(response) => return response,
    };
    let status = match store.public_status(digest, height) {
        Ok(status) => status,
        Err(error) => return map_store_error(error),
    };
    JsonBody(PrivateSettlementLegStatusResponseV1 {
        bundle_id: status.bundle_id,
        payload_digest: status.payload_digest,
        leg_ordinal: status.leg_ordinal,
        route: status.route,
        stored_at_height: status.stored_at_height,
        lifecycle_height: status.lifecycle_height,
        expiry_height: status.expiry_height,
        lifecycle: lifecycle_dto(status.lifecycle),
    })
    .into_response()
}

/// Return proof and opaque delta material only to an exact committee validator.
pub(crate) async fn handler_committee_proof(
    State(app): State<SharedAppState>,
    Extension(runtime): Extension<PrivateSettlementToriiRuntimeV1>,
    Extension(authenticated): Extension<crate::operator_signatures::AuthenticatedOperatorPublicKey>,
    Path(payload_digest): Path<String>,
) -> Response {
    let Ok(store) = runtime.store() else {
        return private_settlement_unavailable();
    };
    let height = match authoritative_height(&app) {
        Ok(height) => height,
        Err(response) => return response,
    };
    if active_config(&app, height).is_err() {
        return private_settlement_unavailable();
    }
    let digest = match parse_digest(&payload_digest) {
        Ok(digest) => digest,
        Err(response) => return response,
    };
    let validator = iroha_data_model::peer::PeerId::from(authenticated.0);
    let view = match store.fetch_for_committee(digest, &validator, height) {
        Ok(view) => view,
        Err(error) => return map_store_error(error),
    };
    JsonBody(PrivateSettlementCommitteeProofResponseV1 {
        manifest: view.manifest,
        audit_policy: view.policy,
        committee_authority: view.authority,
        statement: view.statement,
        proof: view.proof,
        delta: view.delta,
        audit_approvals: view.audit_approvals,
        audit_capsule_digest: view.audit_capsule_digest,
        availability: view.availability,
        lifecycle: lifecycle_dto(view.lifecycle),
    })
    .into_response()
}

fn governed_auditor_view(
    app: &SharedAppState,
    runtime: &PrivateSettlementToriiRuntimeV1,
    payload_digest: &str,
    signing_key: &PublicKey,
) -> Result<
    (
        Hash,
        u64,
        iroha_core::private_settlement::PrivateSettlementAuthenticatedAuditorViewV1,
    ),
    Response,
> {
    let store = runtime.store()?;
    let height = authoritative_height(app)?;
    active_config(app, height)?;
    let digest = parse_digest(payload_digest)?;
    let authenticated = store
        .fetch_for_auditor_signing_key(digest, signing_key, height)
        .map_err(map_store_error)?;
    Ok((digest, height, authenticated))
}

/// Return the padded encrypted capsule only to an exact governed auditor key.
pub(crate) async fn handler_auditor_capsule(
    State(app): State<SharedAppState>,
    Extension(runtime): Extension<PrivateSettlementToriiRuntimeV1>,
    Extension(authenticated): Extension<crate::operator_signatures::AuthenticatedOperatorPublicKey>,
    Path(payload_digest): Path<String>,
) -> Response {
    let (_, _, authenticated_view) =
        match governed_auditor_view(&app, &runtime, &payload_digest, &authenticated.0) {
            Ok(value) => value,
            Err(response) => return response,
        };
    let view = authenticated_view.view;
    JsonBody(PrivateSettlementAuditorCapsuleResponseV1 {
        manifest: view.manifest,
        audit_policy: view.policy,
        committee_authority: view.authority,
        statement: view.statement,
        delta: view.delta,
        audit_capsule: view.audit_capsule,
        availability: view.availability,
        lifecycle: lifecycle_dto(view.lifecycle),
    })
    .into_response()
}

/// Persist one approval after binding its body to the authenticated auditor key.
pub(crate) async fn handler_auditor_approval(
    State(app): State<SharedAppState>,
    Extension(runtime): Extension<PrivateSettlementToriiRuntimeV1>,
    Extension(authenticated): Extension<crate::operator_signatures::AuthenticatedOperatorPublicKey>,
    Path(payload_digest): Path<String>,
    NoritoJson(request): NoritoJson<PrivateSettlementAuditApprovalRequestV1>,
) -> Response {
    let (digest, height, authenticated_view) =
        match governed_auditor_view(&app, &runtime, &payload_digest, &authenticated.0) {
            Ok(value) => value,
            Err(response) => return response,
        };
    let approval: PrivateSettlementAuditApprovalV1 = request.approval;
    if approval.body.auditor_id != authenticated_view.auditor_id {
        return private_settlement_unavailable();
    }
    let config = match active_config(&app, height) {
        Ok(config) => config,
        Err(response) => return response,
    };
    let status = match runtime
        .store()
        .and_then(|store| store.public_status(digest, height).map_err(map_store_error))
    {
        Ok(status) => status,
        Err(response) => return response,
    };
    if height
        > status
            .stored_at_height
            .saturating_add(config.audit_timeout_blocks.get())
    {
        return error_response(StatusCode::CONFLICT, "private_settlement_audit_timeout");
    }
    let outcome = match runtime.store().and_then(|store| {
        store
            .record_audit_approval(digest, approval, height)
            .map_err(map_store_error)
    }) {
        Ok(outcome) => outcome,
        Err(response) => return response,
    };
    JsonBody(PrivateSettlementAuditApprovalResponseV1 {
        bundle_id: authenticated_view.view.manifest.bundle_id,
        payload_digest: digest,
        leg_ordinal: authenticated_view.view.statement.leg_ordinal,
        collected: outcome.collected,
        required: outcome.required,
        newly_recorded: outcome.newly_recorded,
        lifecycle: if outcome.audited {
            PrivateSettlementLifecycleDtoV1::Audited
        } else {
            PrivateSettlementLifecycleDtoV1::Collecting
        },
    })
    .into_response()
}

fn exact_finalization_carrier(
    transaction: &SignedTransaction,
) -> Result<&FinalizeAtomicPrivateSettlementV1, Response> {
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "private_settlement_invalid_carrier",
        ));
    };
    if instructions.len() != 1 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "private_settlement_invalid_carrier",
        ));
    }
    instructions[0]
        .as_any()
        .downcast_ref::<FinalizeAtomicPrivateSettlementV1>()
        .ok_or_else(|| {
            error_response(
                StatusCode::BAD_REQUEST,
                "private_settlement_invalid_carrier",
            )
        })
}

/// Admit one exact sponsor-signed global carrier through ordinary transaction ingress.
pub(crate) async fn handler_bundle_submit(
    State(app): State<SharedAppState>,
    Extension(authenticated): Extension<VerifiedCanonicalRequest>,
    headers: HeaderMap,
    accept: Option<crate::utils::extractors::ExtractAccept>,
    NoritoJson(request): NoritoJson<PrivateSettlementBundleSubmitRequestV1>,
) -> Result<Response, crate::Error> {
    let height = authoritative_height(&app).map_err(|_| crate::Error::AppServiceUnavailable {
        code: "private_settlement_height_unavailable",
        message: "private-settlement height is unavailable".to_owned(),
    })?;
    let config = active_config(&app, height).map_err(|_| crate::Error::AppServiceUnavailable {
        code: "private_settlement_unavailable",
        message: "private-settlement carrier admission is unavailable".to_owned(),
    })?;
    let transaction = request.transaction;
    let carrier =
        exact_finalization_carrier(&transaction).map_err(|_| crate::Error::AppQueryValidation {
            code: "private_settlement_invalid_carrier",
            message: "private-settlement carrier is invalid".to_owned(),
        })?;
    let manifest = &carrier.commit_bundle.manifest;
    let candidate_receipt = carrier
        .commit_bundle
        .clone()
        .into_receipt(manifest.authority_context_height);
    let carrier_bytes = norito::encode_canonical(&transaction)
        .and_then(|encoded| {
            u64::try_from(encoded.len()).map_err(|_| {
                norito::Error::Io(std::io::Error::other(
                    "private-settlement carrier transaction is too large",
                ))
            })
        })
        .map_err(|_| crate::Error::AppQueryValidation {
            code: "private_settlement_invalid_carrier",
            message: "private-settlement carrier is not canonically encodable".to_owned(),
        })?;
    if authenticated.account != manifest.sponsor
        || transaction.authority() != &manifest.sponsor
        || transaction.fee_payment_intent() != &manifest.public_fee_intent
        || manifest.network_id != app.state.network_id
        || !private_settlement_carrier_height_is_live_v1(
            height,
            manifest.authority_context_height,
            manifest.expiry_height,
        )
        || candidate_receipt.validate_shape().is_err()
        || carrier.commit_bundle.legs.len() > usize::from(config.max_participants.get())
        || manifest
            .expiry_height
            .checked_sub(manifest.authority_context_height)
            .is_none_or(|span| span > config.max_expiry_blocks.get())
        || carrier_bytes > config.max_carrier_bytes.get()
    {
        return Err(crate::Error::AppQueryValidation {
            code: "private_settlement_invalid_carrier",
            message: "private-settlement carrier is invalid".to_owned(),
        });
    }
    let bundle_id = manifest.bundle_id;
    let carrier_id = Hash::from(transaction.hash());
    let admitted =
        crate::submit_signed_transaction_for_ingress(app, headers, accept, transaction).await?;
    if !admitted.status().is_success() {
        return Ok(admitted);
    }
    let mut response = JsonBody(PrivateSettlementBundleSubmitResponseV1 {
        bundle_id,
        accepted_at_height: height,
        carrier_id,
        lifecycle: PrivateSettlementLifecycleDtoV1::CommitCertified,
    })
    .into_response();
    *response.status_mut() = admitted.status();
    Ok(response)
}

fn terminal_bundle_state(
    app: &SharedAppState,
    bundle_id: Hash,
) -> (
    Option<iroha_data_model::nexus::PrivateSettlementReceiptV1>,
    Option<iroha_data_model::nexus::PrivateSettlementAbortReceiptV1>,
) {
    let view = app.state.view();
    let world = view.world();
    (
        world.private_settlement_receipt_v1(&bundle_id).cloned(),
        world.private_settlement_abort_v1(&bundle_id).copied(),
    )
}

/// Return an allowlisted aggregate bundle lifecycle.
pub(crate) async fn handler_bundle_status(
    State(app): State<SharedAppState>,
    Extension(runtime): Extension<PrivateSettlementToriiRuntimeV1>,
    Path(bundle_id): Path<String>,
) -> Response {
    let bundle_id = match parse_digest(&bundle_id) {
        Ok(bundle_id) => bundle_id,
        Err(response) => return response,
    };
    let height = match authoritative_height(&app) {
        Ok(height) => height,
        Err(response) => return response,
    };
    if active_config(&app, height).is_err() {
        return private_settlement_unavailable();
    }
    let (receipt, abort) = terminal_bundle_state(&app, bundle_id);
    if let Some(receipt) = receipt {
        return JsonBody(PrivateSettlementBundleStatusResponseV1 {
            manifest: Some(receipt.manifest),
            lifecycle: PrivateSettlementLifecycleDtoV1::Finalized,
            finalized_height: Some(receipt.finalized_height),
        })
        .into_response();
    }
    let sidecar = runtime.store().and_then(|store| {
        store
            .public_bundle_status(bundle_id, height)
            .map_err(map_store_error)
    });
    if let Some(abort) = abort {
        let manifest = sidecar.ok().map(|status| status.manifest);
        let lifecycle = if abort.reason == PrivateSettlementAbortReasonV1::Expired {
            PrivateSettlementLifecycleDtoV1::Expired
        } else {
            PrivateSettlementLifecycleDtoV1::Aborted
        };
        return JsonBody(PrivateSettlementBundleStatusResponseV1 {
            manifest,
            lifecycle,
            finalized_height: Some(abort.finalized_height),
        })
        .into_response();
    }
    let status = match sidecar {
        Ok(status) => status,
        Err(response) => return response,
    };
    JsonBody(PrivateSettlementBundleStatusResponseV1 {
        manifest: Some(status.manifest),
        lifecycle: lifecycle_dto(status.lifecycle),
        finalized_height: None,
    })
    .into_response()
}

/// Return the finalized receipt, abort marker, or redacted pending state.
pub(crate) async fn handler_bundle_receipt(
    State(app): State<SharedAppState>,
    Extension(runtime): Extension<PrivateSettlementToriiRuntimeV1>,
    Path(bundle_id): Path<String>,
) -> Response {
    let bundle_id = match parse_digest(&bundle_id) {
        Ok(bundle_id) => bundle_id,
        Err(response) => return response,
    };
    let height = match authoritative_height(&app) {
        Ok(height) => height,
        Err(response) => return response,
    };
    if active_config(&app, height).is_err() {
        return private_settlement_unavailable();
    }
    let (receipt, abort) = terminal_bundle_state(&app, bundle_id);
    if let Some(receipt) = receipt {
        return JsonBody(PrivateSettlementBundleReceiptResponseV1::Finalized(receipt))
            .into_response();
    }
    if let Some(abort) = abort {
        return JsonBody(PrivateSettlementBundleReceiptResponseV1::Aborted(abort)).into_response();
    }
    let status = match runtime.store().and_then(|store| {
        store
            .public_bundle_status(bundle_id, height)
            .map_err(map_store_error)
    }) {
        Ok(status) => status,
        Err(response) => return response,
    };
    JsonBody(PrivateSettlementBundleReceiptResponseV1::Pending {
        bundle_id,
        lifecycle: lifecycle_dto(status.lifecycle),
    })
    .into_response()
}
