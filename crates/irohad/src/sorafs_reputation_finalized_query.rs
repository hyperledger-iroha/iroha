//! Exact-anchor SoraFS reputation queries backed by the durable core archive.
//!
//! This adapter never consults [`iroha_core::state::State`] and therefore has
//! no current-head fallback. Every response is materialized from one fully
//! validated immutable archive record. Requests below the archive's explicit
//! activation floor fail with a distinct payload-free receipt. The daemon
//! launcher opens this archive from explicit `iroha_config` bounds, reconciles
//! the authenticated Kura tip before constructing the adapter, and passes the
//! same archive to Sumeragi so every fresh v2 commit captures its immutable
//! post-execution view after the Kura/WSV checkpoint boundary and before live
//! State publication.
//!
//! Journal delivery is revalidated as one policy/page request before it leaves
//! this boundary. The other V1 page types do not expose data-model validation
//! methods; their source-order and exact-anchor invariants are validated when
//! the archive record is opened, their exclusive cursor is resolved against
//! that immutable row here, and the reputation worker independently validates
//! the returned anchor and continuation before consuming the page.

use std::{fmt, sync::Arc};

use eyre::{Result, bail};
use iroha_config::parameters::{actual::SorafsReputationRuntime, is_production_runtime_handle};
use iroha_core::{
    kura::Kura,
    query::reputation_finalized::{
        ReputationFinalizedArchive, ReputationFinalizedArchiveBounds,
        ReputationFinalizedArchiveCompactionOutcomeV1, ReputationFinalizedArchiveError,
        ReputationFinalizedArchiveKeyV1, ReputationFinalizedArchiveQualificationV1,
        ReputationFinalizedArchiveReconcileOutcomeV1,
        ReputationFinalizedArchiveRetentionAuthorityBindingV1,
        ReputationFinalizedArchiveRetentionAuthorityV1, ReputationFinalizedProjectionV1,
    },
    state::{
        State, StateQueryView, StateReadOnly as _, WorldReadOnly as _, WorldStateSnapshot as _,
    },
    sumeragi::{V2StartupReplayPlan, plan_v2_startup_replay},
};
use iroha_data_model::{
    ChainId,
    query::sorafs::prelude::{
        FindSorafsReputationJournalAuthorityPolicy, FindSorafsReputationJournalEventBySourceId,
    },
    sorafs::{
        capacity::ProviderId,
        moderation_ledger::{
            REPAIR_QUERY_MAX_EVENT_PAGE_BYTES_V1, REPAIR_QUERY_MAX_ITEMS_V1,
            RepairFinalizedCursorV1, RepairFinalizedEventCursorV1, RepairFinalizedEventPageV1,
            RepairFinalizedEventV1,
        },
        orderbook::{
            ORDERBOOK_QUERY_MAX_EVENT_PAGE_BYTES_V1, ORDERBOOK_QUERY_MAX_ITEMS_V1,
            OrderbookFinalizedCursorV1, OrderbookFinalizedEventCursorV1,
            OrderbookFinalizedEventPageV1, OrderbookFinalizedEventV1,
        },
        proof_ledger::{
            PROOF_OUTCOME_QUERY_MAX_EVENT_PAGE_BYTES_V1, PROOF_OUTCOME_QUERY_MAX_ITEMS_V1,
            ProofOutcomeFinalizedCursorV1, ProofOutcomeFinalizedEventCursorV1,
            ProofOutcomeFinalizedEventPageV1, ProofOutcomeFinalizedEventV1,
        },
        reputation::{
            REPUTATION_JOURNAL_QUERY_MAX_EVENT_PAGE_BYTES_V1,
            REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1, ReputationFinalizedArchiveRetentionRequestV1,
            ReputationJournalAuthorityPolicyRecordV1 as AuthorityPolicyRecord,
            ReputationJournalFinalizedCursorV1, ReputationJournalFinalizedEventCursorV1,
            ReputationJournalFinalizedEventPageV1, ReputationJournalFinalizedEventV1,
            ReputationJournalSourceIdV1,
        },
        reserve::{
            RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1, RESERVE_QUERY_MAX_ITEMS_V1,
            ReserveFinalizedCursorV1, ReserveFinalizedEventCursorV1, ReserveFinalizedEventPageV1,
            ReserveFinalizedEventV1, ReserveProviderAccountPageV1,
        },
    },
};
use norito::core::NoritoSerialize;
use sorafs_node::reputation::{
    ReputationFinalizedIdentityV1,
    runtime::{
        REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1, ReputationExternalFailureV1,
        ReputationFinalizedAnchorV1, ReputationFinalizedQueryV1,
        ReputationJournalDeliveryFinalizedViewV1, ReputationJournalSourceFinalizedViewV1,
        ReputationRuntimeProviderQualificationV1, ReputationRuntimeProviderV1,
    },
};

const FAILURE_INVALID_REQUEST: u8 = 0xA1;
const FAILURE_ARCHIVE_READ: u8 = 0xA2;
const FAILURE_MISSING_ANCHOR: u8 = 0xA3;
const FAILURE_ANCHOR_MISMATCH: u8 = 0xA4;
const FAILURE_INVALID_LIMIT: u8 = 0xA5;
const FAILURE_INVALID_CURSOR: u8 = 0xA6;
const FAILURE_PAGE_BOUNDS: u8 = 0xA7;
const FAILURE_BELOW_ACTIVATION_FLOOR: u8 = 0xA8;

/// Typed failure while qualifying the configured finalized archive for startup.
#[derive(Debug)]
pub(crate) enum ReputationFinalizedArchiveStartupErrorV1 {
    /// State, Kura, or the authenticated pending-tip plan disagreed.
    StartupBoundary {
        /// Stable payload-free rejection reason.
        reason: &'static str,
    },
    /// Reading the exact durable Kura boundary failed.
    KuraBoundary {
        /// Payload-free storage diagnostic.
        detail: String,
    },
    /// Runtime retention-authority presence disagreed with public configuration.
    RetentionAuthorityConfiguration {
        /// Stable payload-free rejection reason.
        reason: &'static str,
    },
    /// One named archive qualification stage failed.
    Archive {
        /// Payload-free startup stage.
        stage: &'static str,
        /// Typed durable archive failure.
        source: Box<iroha_core::query::reputation_finalized::ReputationFinalizedArchiveError>,
    },
}

impl fmt::Display for ReputationFinalizedArchiveStartupErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StartupBoundary { reason } => {
                write!(
                    formatter,
                    "invalid finalized reputation startup boundary: {reason}"
                )
            }
            Self::KuraBoundary { detail } => write!(
                formatter,
                "finalized reputation startup could not read the exact durable Kura boundary: {detail}"
            ),
            Self::RetentionAuthorityConfiguration { reason } => write!(
                formatter,
                "invalid finalized reputation retention-authority configuration: {reason}"
            ),
            Self::Archive { stage, .. } => {
                write!(formatter, "finalized reputation archive failed at {stage}")
            }
        }
    }
}

impl std::error::Error for ReputationFinalizedArchiveStartupErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::StartupBoundary { .. }
            | Self::KuraBoundary { .. }
            | Self::RetentionAuthorityConfiguration { .. } => None,
            Self::Archive { source, .. } => Some(source.as_ref()),
        }
    }
}

/// Recovery mode authenticated while opening the reputation archive.
#[derive(Debug)]
pub(crate) enum ReputationFinalizedArchiveStartupModeV1 {
    /// Fresh height-zero State/Kura with a completely empty archive namespace.
    BootstrapAwaitingGenesisCapture,
    /// Ordinary startup reconciled and qualified with an exact zero-gap tip.
    Qualified {
        /// Exact startup reconciliation against committed State and Kura.
        reconciliation: ReputationFinalizedArchiveReconcileOutcomeV1,
        /// Subsequent configured live-lag qualification.
        live_qualification: ReputationFinalizedArchiveQualificationV1,
    },
    /// One authenticated pending V2 tip will finish capture through Apply.
    PendingTipReplay {
        /// Exact pending height retained by the validated V2 replay plan.
        pending_tip_height: u64,
        /// Current authenticated qualification, absent only for empty
        /// pre-genesis height zero.
        qualification: Option<ReputationFinalizedArchiveQualificationV1>,
        /// Whether startup established a nonhistorical floor at the committed
        /// State view immediately preceding pending replay.
        activation_floor_created: bool,
    },
}

impl ReputationFinalizedArchiveStartupModeV1 {
    fn activation_gate(&self) -> ArchiveActivationGateV1 {
        match self {
            Self::BootstrapAwaitingGenesisCapture => ArchiveActivationGateV1::AwaitingGenesis,
            Self::Qualified { .. } => ArchiveActivationGateV1::StrictLive,
            Self::PendingTipReplay {
                pending_tip_height, ..
            } => ArchiveActivationGateV1::PendingTip {
                height: *pending_tip_height,
            },
        }
    }
}

/// Exact archive qualification retained between startup and adapter assembly.
#[derive(Debug)]
#[must_use]
pub(crate) struct PreparedReputationFinalizedArchiveV1 {
    archive: Arc<ReputationFinalizedArchive>,
    startup_mode: ReputationFinalizedArchiveStartupModeV1,
    activation: ReputationFinalizedArchiveActivationV1,
    retention_authority: Option<QualifiedReputationRetentionAuthorityV1>,
}

impl PreparedReputationFinalizedArchiveV1 {
    /// Return the archive installed into the Sumeragi commit corridor.
    pub(crate) fn archive(&self) -> &Arc<ReputationFinalizedArchive> {
        &self.archive
    }

    /// Return the exact authenticated startup mode.
    pub(crate) const fn startup_mode(&self) -> &ReputationFinalizedArchiveStartupModeV1 {
        &self.startup_mode
    }

    /// Return the cloneable deferred-activation probe retained by the runtime
    /// launcher.
    pub(crate) const fn activation(&self) -> &ReputationFinalizedArchiveActivationV1 {
        &self.activation
    }

    /// Return the authority qualified for explicit finalized-prefix retention.
    pub(crate) const fn retention_authority(
        &self,
    ) -> Option<&QualifiedReputationRetentionAuthorityV1> {
        self.retention_authority.as_ref()
    }

    /// Build the supervised explicit-retention controller when configured.
    pub(crate) fn retention_controller(
        &self,
    ) -> Option<ReputationFinalizedArchiveRetentionControllerV1> {
        let authority = self.retention_authority.as_ref()?.clone();
        Some(ReputationFinalizedArchiveRetentionControllerV1 {
            chain_id: self.activation.chain_id.clone(),
            archive: Arc::clone(&self.archive),
            state: Arc::clone(&self.activation.state),
            kura: Arc::clone(&self.activation.kura),
            binding: authority.binding,
            authority: authority.authority,
        })
    }
}

/// Exact configured retention authority retained after startup qualification.
#[derive(Clone)]
pub(crate) struct QualifiedReputationRetentionAuthorityV1 {
    binding: ReputationFinalizedArchiveRetentionAuthorityBindingV1,
    authority: Arc<dyn ReputationFinalizedArchiveRetentionAuthorityV1>,
}

impl fmt::Debug for QualifiedReputationRetentionAuthorityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedReputationRetentionAuthorityV1")
            .field("handle", &self.binding.handle())
            .field("qualification", &self.binding.qualification())
            .finish_non_exhaustive()
    }
}

/// One supervised result for the explicit finalized-archive retention control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReputationFinalizedArchiveRetentionControlOutcomeV1 {
    /// No governed retention request is committed.
    NoRequest,
    /// The exact requested prefix is already covered by a durable checkpoint.
    AlreadyApplied {
        /// Canonical request identity.
        request_digest: [u8; 32],
        /// Fresh committed anchor that exposed the request.
        authorization_anchor: ReputationFinalizedArchiveKeyV1,
        /// Current virtual-base retention floor.
        retention_floor: ReputationFinalizedArchiveKeyV1,
    },
    /// The exact request was approved externally and installed locally.
    Applied {
        /// Canonical request identity.
        request_digest: [u8; 32],
        /// Fresh committed anchor that exposed the request.
        authorization_anchor: ReputationFinalizedArchiveKeyV1,
        /// Durable compaction result.
        compaction: ReputationFinalizedArchiveCompactionOutcomeV1,
    },
}

/// Fail-closed error while reconciling explicit finalized-archive retention.
#[derive(Debug)]
pub(crate) enum ReputationFinalizedArchiveRetentionControlErrorV1 {
    /// State, Kura, archive, or request lineage violated a fixed boundary.
    Boundary {
        /// Stable payload-free reason.
        reason: &'static str,
    },
    /// A matching committed custom parameter was malformed.
    Request {
        /// Payload-free structural diagnostic.
        detail: String,
    },
    /// Durable archive authentication or publication failed.
    Archive {
        /// Underlying typed archive failure.
        source: Box<ReputationFinalizedArchiveError>,
    },
}

impl fmt::Display for ReputationFinalizedArchiveRetentionControlErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Boundary { reason } => {
                write!(formatter, "invalid reputation retention boundary: {reason}")
            }
            Self::Request { detail } => {
                write!(
                    formatter,
                    "invalid committed reputation retention request: {detail}"
                )
            }
            Self::Archive { .. } => {
                write!(formatter, "explicit reputation archive retention failed")
            }
        }
    }
}

impl std::error::Error for ReputationFinalizedArchiveRetentionControlErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Archive { source } => Some(source.as_ref()),
            Self::Boundary { .. } | Self::Request { .. } => None,
        }
    }
}

/// Runtime contract consumed by the supervised reputation worker.
pub(crate) trait ReputationFinalizedArchiveRetentionControlV1:
    fmt::Debug + Send + Sync
{
    /// Revalidate the deployment-owned authority without performing work.
    fn revalidate(
        &self,
    ) -> std::result::Result<(), ReputationFinalizedArchiveRetentionControlErrorV1>;

    /// Reconcile the latest explicit caller-signed committed request once.
    fn reconcile_once(
        &self,
    ) -> std::result::Result<
        ReputationFinalizedArchiveRetentionControlOutcomeV1,
        ReputationFinalizedArchiveRetentionControlErrorV1,
    >;
}

/// State/Kura/archive-bound implementation of explicit governed retention.
#[derive(Clone)]
pub(crate) struct ReputationFinalizedArchiveRetentionControllerV1 {
    chain_id: ChainId,
    archive: Arc<ReputationFinalizedArchive>,
    state: Arc<State>,
    kura: Arc<Kura>,
    binding: ReputationFinalizedArchiveRetentionAuthorityBindingV1,
    authority: Arc<dyn ReputationFinalizedArchiveRetentionAuthorityV1>,
}

impl fmt::Debug for ReputationFinalizedArchiveRetentionControllerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReputationFinalizedArchiveRetentionControllerV1")
            .field("chain_id", &self.chain_id)
            .field("archive_root", &self.archive.root())
            .field("authority_handle", &self.binding.handle())
            .field("authority_qualification", &self.binding.qualification())
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct ReputationRetentionAuthorizationSnapshotV1 {
    request: ReputationFinalizedArchiveRetentionRequestV1,
    authorization_anchor: ReputationFinalizedArchiveKeyV1,
    qualification: ReputationFinalizedArchiveQualificationV1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ReputationRetentionDecisionV1 {
    Apply(ReputationFinalizedArchiveKeyV1),
    AlreadyApplied(ReputationFinalizedArchiveKeyV1),
}

impl ReputationFinalizedArchiveRetentionControllerV1 {
    fn archive_error(
        source: ReputationFinalizedArchiveError,
    ) -> ReputationFinalizedArchiveRetentionControlErrorV1 {
        ReputationFinalizedArchiveRetentionControlErrorV1::Archive {
            source: Box::new(source),
        }
    }

    fn revalidate_authority(
        &self,
    ) -> std::result::Result<(), ReputationFinalizedArchiveRetentionControlErrorV1> {
        let handle_before = self.authority.handle();
        let qualification = self.authority.qualification().map_err(|_| {
            ReputationFinalizedArchiveRetentionControlErrorV1::Boundary {
                reason: "retention authority is unavailable",
            }
        })?;
        let handle_after = self.authority.handle();
        if handle_before != self.binding.handle()
            || handle_after != handle_before
            || qualification != self.binding.qualification()
        {
            return Err(
                ReputationFinalizedArchiveRetentionControlErrorV1::Boundary {
                    reason: "retention authority is substituted or stale",
                },
            );
        }
        Ok(())
    }

    fn authorization_snapshot(
        &self,
    ) -> std::result::Result<
        Option<ReputationRetentionAuthorizationSnapshotV1>,
        ReputationFinalizedArchiveRetentionControlErrorV1,
    > {
        let view = self.state.query_view();
        if !std::ptr::eq(view.kura(), self.kura.as_ref()) {
            return Err(
                ReputationFinalizedArchiveRetentionControlErrorV1::Boundary {
                    reason: "committed State is bound to another Kura instance",
                },
            );
        }
        if view.chain_id() != &self.chain_id {
            return Err(
                ReputationFinalizedArchiveRetentionControlErrorV1::Boundary {
                    reason: "committed State is bound to another chain",
                },
            );
        }
        let Some(custom) = view
            .world()
            .parameters()
            .custom()
            .get(&ReputationFinalizedArchiveRetentionRequestV1::parameter_id())
        else {
            return Ok(None);
        };
        let request = ReputationFinalizedArchiveRetentionRequestV1::from_custom_parameter(custom)
            .map_err(
                |error| ReputationFinalizedArchiveRetentionControlErrorV1::Request {
                    detail: error.to_string(),
                },
            )?
            .ok_or(
                ReputationFinalizedArchiveRetentionControlErrorV1::Boundary {
                    reason: "retention parameter changed reserved identity",
                },
            )?;
        if request.chain_id != self.chain_id {
            return Err(
                ReputationFinalizedArchiveRetentionControlErrorV1::Boundary {
                    reason: "retention request targets another chain",
                },
            );
        }
        let authorization_height = u64::try_from(view.height()).map_err(|_| {
            ReputationFinalizedArchiveRetentionControlErrorV1::Boundary {
                reason: "committed State height exceeds the supported range",
            }
        })?;
        if request.compact_through.height >= authorization_height {
            return Err(
                ReputationFinalizedArchiveRetentionControlErrorV1::Boundary {
                    reason: "retention target does not precede its authorization anchor",
                },
            );
        }
        let target_index = usize::try_from(request.compact_through.height)
            .ok()
            .and_then(|height| height.checked_sub(1))
            .ok_or(
                ReputationFinalizedArchiveRetentionControlErrorV1::Boundary {
                    reason: "retention target height is not representable",
                },
            )?;
        if view
            .block_hashes()
            .get(target_index)
            .map(|hash| *hash.as_ref())
            != Some(request.compact_through.block_hash)
        {
            return Err(
                ReputationFinalizedArchiveRetentionControlErrorV1::Boundary {
                    reason: "retention target is not the exact committed ancestor",
                },
            );
        }
        let authorization_hash = view
            .latest_block_hash()
            .map(|hash| *hash.as_ref())
            .filter(|hash| *hash != [0; 32])
            .ok_or(
                ReputationFinalizedArchiveRetentionControlErrorV1::Boundary {
                    reason: "committed authorization anchor has no block hash",
                },
            )?;
        let authorization_anchor = ReputationFinalizedArchiveKeyV1::try_new(
            self.chain_id.clone(),
            authorization_height,
            authorization_hash,
        )
        .map_err(Self::archive_error)?;
        let qualification = self
            .archive
            .qualify_against_kura_tip(&self.chain_id, self.kura.as_ref(), 0)
            .map_err(Self::archive_error)?;
        if qualification.archive_tip() != &authorization_anchor {
            return Err(
                ReputationFinalizedArchiveRetentionControlErrorV1::Boundary {
                    reason: "fresh committed State and exact archive tip disagree",
                },
            );
        }
        Ok(Some(ReputationRetentionAuthorizationSnapshotV1 {
            request,
            authorization_anchor,
            qualification,
        }))
    }

    fn classify(
        snapshot: &ReputationRetentionAuthorizationSnapshotV1,
    ) -> std::result::Result<
        ReputationRetentionDecisionV1,
        ReputationFinalizedArchiveRetentionControlErrorV1,
    > {
        classify_retention_request(
            &snapshot.request,
            &snapshot.authorization_anchor,
            snapshot.qualification.activation_floor(),
            snapshot.qualification.checkpoint_digest(),
        )
    }
}

fn classify_retention_request(
    request: &ReputationFinalizedArchiveRetentionRequestV1,
    authorization_anchor: &ReputationFinalizedArchiveKeyV1,
    activation_floor: &ReputationFinalizedArchiveKeyV1,
    checkpoint_digest: Option<[u8; 32]>,
) -> std::result::Result<
    ReputationRetentionDecisionV1,
    ReputationFinalizedArchiveRetentionControlErrorV1,
> {
    if request.chain_id != authorization_anchor.chain_id
        || request.compact_through.height >= authorization_anchor.height
    {
        return Err(
            ReputationFinalizedArchiveRetentionControlErrorV1::Boundary {
                reason: "retention target is not below its exact authorization anchor",
            },
        );
    }
    let target = ReputationFinalizedArchiveKeyV1::try_new(
        request.chain_id.clone(),
        request.compact_through.height,
        request.compact_through.block_hash,
    )
    .map_err(ReputationFinalizedArchiveRetentionControllerV1::archive_error)?;
    if activation_floor.chain_id != target.chain_id {
        return Err(
            ReputationFinalizedArchiveRetentionControlErrorV1::Boundary {
                reason: "retention activation floor is bound to another chain",
            },
        );
    }
    if checkpoint_digest.is_some() && activation_floor.height >= target.height {
        if activation_floor.height == target.height
            && activation_floor.block_hash != target.block_hash
        {
            return Err(
                ReputationFinalizedArchiveRetentionControlErrorV1::Boundary {
                    reason: "retention checkpoint conflicts with the requested exact target",
                },
            );
        }
        return Ok(ReputationRetentionDecisionV1::AlreadyApplied(
            activation_floor.clone(),
        ));
    }
    if activation_floor.height > target.height {
        return Err(
            ReputationFinalizedArchiveRetentionControlErrorV1::Boundary {
                reason: "retention target predates the archive activation floor",
            },
        );
    }
    Ok(ReputationRetentionDecisionV1::Apply(target))
}

impl ReputationFinalizedArchiveRetentionControlV1
    for ReputationFinalizedArchiveRetentionControllerV1
{
    fn revalidate(
        &self,
    ) -> std::result::Result<(), ReputationFinalizedArchiveRetentionControlErrorV1> {
        self.revalidate_authority()
    }

    fn reconcile_once(
        &self,
    ) -> std::result::Result<
        ReputationFinalizedArchiveRetentionControlOutcomeV1,
        ReputationFinalizedArchiveRetentionControlErrorV1,
    > {
        self.revalidate_authority()?;
        let Some(snapshot) = self.authorization_snapshot()? else {
            return Ok(ReputationFinalizedArchiveRetentionControlOutcomeV1::NoRequest);
        };
        let target = match Self::classify(&snapshot)? {
            ReputationRetentionDecisionV1::Apply(target) => target,
            ReputationRetentionDecisionV1::AlreadyApplied(retention_floor) => {
                return Ok(
                    ReputationFinalizedArchiveRetentionControlOutcomeV1::AlreadyApplied {
                        request_digest: snapshot.request.request_digest,
                        authorization_anchor: snapshot.authorization_anchor,
                        retention_floor,
                    },
                );
            }
        };
        let fence = self
            .archive
            .retention_fence_for(&target)
            .map_err(Self::archive_error)?;
        let proposal = self
            .archive
            .prepare_kura_authenticated_compaction(&fence, self.kura.as_ref())
            .map_err(Self::archive_error)?;

        let refreshed = self.authorization_snapshot()?.ok_or(
            ReputationFinalizedArchiveRetentionControlErrorV1::Boundary {
                reason: "retention authorization was removed before approval",
            },
        )?;
        if refreshed.request != snapshot.request
            || refreshed.authorization_anchor.height < snapshot.authorization_anchor.height
        {
            return Err(
                ReputationFinalizedArchiveRetentionControlErrorV1::Boundary {
                    reason: "retention authorization changed before approval",
                },
            );
        }
        self.revalidate_authority()?;
        let compaction = self
            .archive
            .approve_and_install_kura_authenticated_compaction(
                &proposal,
                self.kura.as_ref(),
                &self.binding,
                self.authority.as_ref(),
            )
            .map_err(Self::archive_error)?;
        Ok(
            ReputationFinalizedArchiveRetentionControlOutcomeV1::Applied {
                request_digest: snapshot.request.request_digest,
                authorization_anchor: refreshed.authorization_anchor,
                compaction,
            },
        )
    }
}

impl QualifiedReputationRetentionAuthorityV1 {
    /// Return the exact credential-free expected authority binding.
    pub(crate) const fn binding(&self) -> &ReputationFinalizedArchiveRetentionAuthorityBindingV1 {
        &self.binding
    }

    /// Return the runtime-only deployment-owned authority.
    pub(crate) const fn authority(
        &self,
    ) -> &Arc<dyn ReputationFinalizedArchiveRetentionAuthorityV1> {
        &self.authority
    }

    fn revalidate(&self) -> Result<(), ReputationFinalizedArchiveStartupErrorV1> {
        let handle_before = self.authority.handle();
        let qualification = self.authority.qualification().map_err(|_| {
            ReputationFinalizedArchiveStartupErrorV1::RetentionAuthorityConfiguration {
                reason: "retention authority became unavailable during startup",
            }
        })?;
        let handle_after = self.authority.handle();
        if handle_before != self.binding.handle()
            || handle_after != handle_before
            || qualification != self.binding.qualification()
        {
            return Err(
                ReputationFinalizedArchiveStartupErrorV1::RetentionAuthorityConfiguration {
                    reason: "retention authority became substituted or stale during startup",
                },
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchiveStartupBoundaryV1 {
    Bootstrap,
    Qualified,
    PendingTip { height: u64 },
}

fn classify_archive_startup_boundary(
    state_height: u64,
    kura_height: u64,
    pending_v2_tip_height: Option<u64>,
) -> Result<ArchiveStartupBoundaryV1, &'static str> {
    let Some(pending_height) = pending_v2_tip_height else {
        if state_height != kura_height {
            return Err("State and Kura heights differ without an authenticated pending V2 tip");
        }
        return Ok(if state_height == 0 {
            ArchiveStartupBoundaryV1::Bootstrap
        } else {
            ArchiveStartupBoundaryV1::Qualified
        });
    };
    if pending_height == 0 || pending_height != kura_height {
        return Err("pending V2 tip does not equal the exact non-zero durable Kura tip");
    }
    if state_height != pending_height && state_height.checked_add(1) != Some(pending_height) {
        return Err("State is not at the pending V2 tip or its exact predecessor");
    }
    Ok(ArchiveStartupBoundaryV1::PendingTip {
        height: pending_height,
    })
}

fn validate_pending_archive_tip(
    pending_tip_height: u64,
    state_height: u64,
    archive_tip_height: Option<u64>,
) -> Result<(), &'static str> {
    match archive_tip_height {
        Some(height) if height == pending_tip_height => Ok(()),
        Some(height)
            if height.checked_add(1) == Some(pending_tip_height) && state_height == height =>
        {
            Ok(())
        }
        None if pending_tip_height == 1 && state_height == 0 => Ok(()),
        Some(_) => {
            Err("archive is not at the pending V2 tip or a replay-capturable exact predecessor")
        }
        None => Err("non-genesis pending V2 replay requires an authenticated archive anchor"),
    }
}

fn classify_pending_replay_completion(
    expected_height: u64,
    durable_height: u64,
    pending_tip_height: Option<u64>,
) -> Result<bool, &'static str> {
    if durable_height < expected_height {
        return Err("recovery durable height regressed below its authenticated pending tip");
    }
    match pending_tip_height {
        None => Ok(true),
        Some(height) if height == expected_height && durable_height == expected_height => Ok(false),
        Some(height) if height == durable_height && height > expected_height => Ok(true),
        Some(_) => Err("recovery exposed a mismatched pending V2 durable tip"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchiveActivationGateV1 {
    StrictLive,
    AwaitingGenesis,
    PendingTip { height: u64 },
}

impl ArchiveActivationGateV1 {
    fn accepts_visible_archive_tip(self, archive_tip_height: u64) -> bool {
        match self {
            Self::PendingTip { height } => archive_tip_height >= height,
            Self::StrictLive | Self::AwaitingGenesis => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ArchiveActivationBoundaryV1 {
    state_height: u64,
    durable_kura_blocks: u64,
}

/// Cloneable fail-closed activation probe for deferred reputation assembly.
#[derive(Clone)]
pub(crate) struct ReputationFinalizedArchiveActivationV1 {
    chain_id: ChainId,
    archive: Arc<ReputationFinalizedArchive>,
    state: Arc<State>,
    kura: Arc<Kura>,
    maximum_kura_tip_lag_blocks: u64,
    gate: ArchiveActivationGateV1,
}

impl fmt::Debug for ReputationFinalizedArchiveActivationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReputationFinalizedArchiveActivationV1")
            .field("chain_id", &self.chain_id)
            .field("archive_root", &self.archive.root())
            .field(
                "maximum_kura_tip_lag_blocks",
                &self.maximum_kura_tip_lag_blocks,
            )
            .field("gate", &self.gate)
            .finish_non_exhaustive()
    }
}

impl ReputationFinalizedArchiveActivationV1 {
    /// Return true only after the archive satisfies ordinary configured live
    /// qualification and any frozen pending tip revalidates as fully
    /// recovered.
    ///
    /// A false result is restricted to the exact bootstrap or pending-tip
    /// boundary authenticated before Sumeragi started.
    ///
    /// # Errors
    ///
    /// Rejects substituted State/Kura bindings, stale pending tips, archive
    /// gaps, forks, and storage failures.
    pub(crate) fn activation_ready(&self) -> Result<bool, ReputationFinalizedArchiveError> {
        let strict_result = self.archive.qualify_against_kura_tip(
            &self.chain_id,
            self.kura.as_ref(),
            self.maximum_kura_tip_lag_blocks,
        );
        if self.strict_qualification_is_ready(strict_result.as_ref().ok())? {
            return Ok(true);
        }
        match self.gate {
            ArchiveActivationGateV1::StrictLive => {
                let _ = strict_result?;
                Err(ReputationFinalizedArchiveError::ArchiveUnavailable {
                    reason: "reputation archive tip is not visible through committed State",
                })
            }
            ArchiveActivationGateV1::AwaitingGenesis => {
                self.awaiting_genesis_activation(strict_result)
            }
            ArchiveActivationGateV1::PendingTip { height } => self.pending_tip_activation(height),
        }
    }

    fn strict_qualification_is_ready(
        &self,
        qualification: Option<&ReputationFinalizedArchiveQualificationV1>,
    ) -> Result<bool, ReputationFinalizedArchiveError> {
        let Some(qualification) = qualification else {
            return Ok(false);
        };
        if !self
            .gate
            .accepts_visible_archive_tip(qualification.archive_tip().height)
            || !self.qualification_is_visible(qualification)?
        {
            return Ok(false);
        }
        match self.gate {
            ArchiveActivationGateV1::PendingTip { height } => self.pending_replay_complete(height),
            ArchiveActivationGateV1::StrictLive | ArchiveActivationGateV1::AwaitingGenesis => {
                Ok(true)
            }
        }
    }

    fn awaiting_genesis_activation(
        &self,
        strict_result: Result<
            ReputationFinalizedArchiveQualificationV1,
            ReputationFinalizedArchiveError,
        >,
    ) -> Result<bool, ReputationFinalizedArchiveError> {
        let boundary = self.activation_boundary("read deferred genesis Kura boundary")?;
        if boundary.state_height == 0 && boundary.durable_kura_blocks <= 1 {
            if self.archive.is_empty()? {
                return Ok(false);
            }
            let qualification =
                self.archive
                    .qualify_against_kura_tip(&self.chain_id, self.kura.as_ref(), 0)?;
            if boundary.durable_kura_blocks == 1 && qualification.archive_tip().height == 1 {
                return Ok(false);
            }
        }
        let _ = strict_result?;
        Err(ReputationFinalizedArchiveError::ArchiveUnavailable {
            reason: "reputation genesis archive tip is not visible through committed State",
        })
    }

    fn pending_tip_activation(
        &self,
        pending_tip_height: u64,
    ) -> Result<bool, ReputationFinalizedArchiveError> {
        let boundary = self.activation_boundary("read deferred pending-tip Kura boundary")?;
        classify_archive_startup_boundary(
            boundary.state_height,
            boundary.durable_kura_blocks,
            Some(pending_tip_height),
        )
        .map_err(|reason| ReputationFinalizedArchiveError::FinalityAuthentication { reason })?;
        if self.archive.is_empty()? {
            validate_pending_archive_tip(pending_tip_height, boundary.state_height, None).map_err(
                |reason| ReputationFinalizedArchiveError::FinalityAuthentication { reason },
            )?;
            return Ok(false);
        }
        let qualification =
            self.archive
                .qualify_against_kura_tip(&self.chain_id, self.kura.as_ref(), 1)?;
        validate_pending_archive_tip(
            pending_tip_height,
            boundary.state_height,
            Some(qualification.archive_tip().height),
        )
        .map_err(|reason| ReputationFinalizedArchiveError::FinalityAuthentication { reason })?;
        Ok(false)
    }

    fn activation_boundary(
        &self,
        kura_operation: &'static str,
    ) -> Result<ArchiveActivationBoundaryV1, ReputationFinalizedArchiveError> {
        let view = self.state.query_view();
        if !std::ptr::eq(view.kura(), self.kura.as_ref()) {
            return Err(ReputationFinalizedArchiveError::FinalityAuthentication {
                reason: "reputation activation State is bound to another Kura instance",
            });
        }
        let state_height = u64::try_from(view.height()).map_err(|_| {
            ReputationFinalizedArchiveError::FinalityAuthentication {
                reason: "reputation activation State height exceeds the supported range",
            }
        })?;
        let kura_height =
            u64::try_from(self.kura.exact_durable_blocks_count().map_err(|error| {
                ReputationFinalizedArchiveError::KuraAuthentication {
                    operation: kura_operation,
                    detail: error.to_string(),
                }
            })?)
            .map_err(|_| {
                ReputationFinalizedArchiveError::FinalityAuthentication {
                    reason: "reputation activation Kura height exceeds the supported range",
                }
            })?;
        Ok(ArchiveActivationBoundaryV1 {
            state_height,
            durable_kura_blocks: kura_height,
        })
    }

    fn qualification_is_visible(
        &self,
        qualification: &ReputationFinalizedArchiveQualificationV1,
    ) -> Result<bool, ReputationFinalizedArchiveError> {
        let view = self.state.query_view();
        if !std::ptr::eq(view.kura(), self.kura.as_ref()) {
            return Err(ReputationFinalizedArchiveError::FinalityAuthentication {
                reason: "reputation activation State is bound to another Kura instance",
            });
        }
        let state_height = u64::try_from(view.height()).map_err(|_| {
            ReputationFinalizedArchiveError::FinalityAuthentication {
                reason: "reputation activation State height exceeds the supported range",
            }
        })?;
        if state_height != qualification.archive_tip().height {
            return Ok(false);
        }
        let state_hash = view
            .latest_block_hash()
            .map(|hash| *hash.as_ref())
            .filter(|hash| *hash != [0; 32])
            .ok_or(ReputationFinalizedArchiveError::FinalityAuthentication {
                reason: "reputation activation State has no committed block hash",
            })?;
        if state_hash != qualification.archive_tip().block_hash {
            return Err(ReputationFinalizedArchiveError::FinalityAuthentication {
                reason: "reputation activation State and archive tips disagree",
            });
        }
        Ok(true)
    }

    fn pending_replay_complete(
        &self,
        expected_height: u64,
    ) -> Result<bool, ReputationFinalizedArchiveError> {
        let replay_plan = plan_v2_startup_replay(self.kura.as_ref()).map_err(|error| {
            ReputationFinalizedArchiveError::KuraAuthentication {
                operation: "revalidate deferred pending-tip recovery",
                detail: error.to_string(),
            }
        })?;
        let durable_height = u64::try_from(replay_plan.durable_height()).map_err(|_| {
            ReputationFinalizedArchiveError::FinalityAuthentication {
                reason: "reputation recovery Kura height exceeds the supported range",
            }
        })?;
        classify_pending_replay_completion(
            expected_height,
            durable_height,
            replay_plan.pending_tip_height(),
        )
        .map_err(|reason| ReputationFinalizedArchiveError::FinalityAuthentication { reason })
    }
}

type ArchiveStartupResultV1<T> = std::result::Result<T, ReputationFinalizedArchiveStartupErrorV1>;

struct AuthenticatedArchiveStartupV1<'state> {
    state_view: StateQueryView<'state>,
    state_height: u64,
    boundary: ArchiveStartupBoundaryV1,
}

fn archive_startup_error(
    stage: &'static str,
    source: ReputationFinalizedArchiveError,
) -> ReputationFinalizedArchiveStartupErrorV1 {
    ReputationFinalizedArchiveStartupErrorV1::Archive {
        stage,
        source: Box::new(source),
    }
}

fn open_reputation_finalized_archive(
    config: &SorafsReputationRuntime,
    chain_id: &ChainId,
    kura: &Kura,
    authority: Option<Arc<dyn ReputationFinalizedArchiveRetentionAuthorityV1>>,
) -> ArchiveStartupResultV1<(
    Arc<ReputationFinalizedArchive>,
    Option<QualifiedReputationRetentionAuthorityV1>,
)> {
    let bounds = ReputationFinalizedArchiveBounds::try_new(
        config.finalized_archive_max_record_bytes,
        config.finalized_archive_max_entries,
        config.finalized_archive_max_total_bytes,
    )
    .map_err(|source| archive_startup_error("resource-bound validation", source))?;
    let (archive, qualified_authority) = match (
        &config.finalized_archive_retention_authority,
        authority,
    ) {
        (None, None) => (
            ReputationFinalizedArchive::try_open(&config.finalized_archive_root, bounds),
            None,
        ),
        (Some(expected), Some(authority)) => {
            let binding = ReputationFinalizedArchiveRetentionAuthorityBindingV1::try_new(
                expected.handle.clone(),
                expected.revision,
                expected.policy_digest,
            )
            .map_err(|source| {
                archive_startup_error("retention-authority binding validation", source)
            })?;
            let archive = ReputationFinalizedArchive::try_open_with_retention_authority(
                &config.finalized_archive_root,
                bounds,
                chain_id,
                kura,
                &binding,
                authority.as_ref(),
            );
            (
                archive,
                Some(QualifiedReputationRetentionAuthorityV1 { binding, authority }),
            )
        }
        (Some(_), None) => {
            return Err(
                ReputationFinalizedArchiveStartupErrorV1::RetentionAuthorityConfiguration {
                    reason: "enabled retention requires its deployment-owned sealed CAS authority",
                },
            );
        }
        (None, Some(_)) => {
            return Err(
                ReputationFinalizedArchiveStartupErrorV1::RetentionAuthorityConfiguration {
                    reason: "manual retention mode rejects an unexpected runtime authority",
                },
            );
        }
    };
    archive
        .map(Arc::new)
        .map(|archive| (archive, qualified_authority))
        .map_err(|source| archive_startup_error("durable open and retention recovery", source))
}

fn authenticate_archive_startup<'state>(
    state: &'state State,
    kura: &Kura,
    startup_replay_plan: &V2StartupReplayPlan,
) -> ArchiveStartupResultV1<AuthenticatedArchiveStartupV1<'state>> {
    let state_view = state.query_view();
    if !std::ptr::eq(state_view.kura(), kura) {
        return Err(ReputationFinalizedArchiveStartupErrorV1::StartupBoundary {
            reason: "State is bound to a substituted Kura instance",
        });
    }
    let state_height = u64::try_from(state_view.height()).map_err(|_| {
        ReputationFinalizedArchiveStartupErrorV1::StartupBoundary {
            reason: "committed State height exceeds the supported range",
        }
    })?;
    let kura_height = u64::try_from(kura.exact_durable_blocks_count().map_err(|source| {
        ReputationFinalizedArchiveStartupErrorV1::KuraBoundary {
            detail: source.to_string(),
        }
    })?)
    .map_err(
        |_| ReputationFinalizedArchiveStartupErrorV1::StartupBoundary {
            reason: "durable Kura height exceeds the supported range",
        },
    )?;
    if u64::try_from(startup_replay_plan.durable_height()).ok() != Some(kura_height) {
        return Err(ReputationFinalizedArchiveStartupErrorV1::StartupBoundary {
            reason: "validated V2 startup plan is bound to another durable Kura height",
        });
    }
    let boundary = classify_archive_startup_boundary(
        state_height,
        kura_height,
        startup_replay_plan.pending_tip_height(),
    )
    .map_err(|reason| ReputationFinalizedArchiveStartupErrorV1::StartupBoundary { reason })?;
    Ok(AuthenticatedArchiveStartupV1 {
        state_view,
        state_height,
        boundary,
    })
}

fn qualify_existing_archive(
    archive: &ReputationFinalizedArchive,
    chain_id: &ChainId,
    kura: &Kura,
    startup: &AuthenticatedArchiveStartupV1<'_>,
    maximum_kura_tip_lag_blocks: u64,
) -> ArchiveStartupResultV1<ReputationFinalizedArchiveStartupModeV1> {
    let reconciliation = archive
        .reconcile_kura_authenticated_state_tip(&startup.state_view, kura)
        .map_err(|source| archive_startup_error("exact Kura-tip reconciliation", source))?;
    let live_qualification = archive
        .qualify_against_kura_tip(chain_id, kura, maximum_kura_tip_lag_blocks)
        .map_err(|source| archive_startup_error("configured live-lag qualification", source))?;
    Ok(ReputationFinalizedArchiveStartupModeV1::Qualified {
        reconciliation,
        live_qualification,
    })
}

fn qualify_pending_archive(
    archive: &ReputationFinalizedArchive,
    chain_id: &ChainId,
    kura: &Kura,
) -> ArchiveStartupResultV1<ReputationFinalizedArchiveQualificationV1> {
    archive
        .qualify_against_kura_tip(chain_id, kura, 1)
        .map_err(|source| archive_startup_error("pending-tip one-block qualification", source))
}

fn capture_pending_archive_predecessor(
    archive: &ReputationFinalizedArchive,
    kura: &Kura,
    startup: &AuthenticatedArchiveStartupV1<'_>,
) -> ArchiveStartupResultV1<()> {
    let (_, receipt) = kura
        .v2_finality_artifact_with_receipt(startup.state_height)
        .map_err(
            |source| ReputationFinalizedArchiveStartupErrorV1::KuraBoundary {
                detail: source.to_string(),
            },
        )?
        .ok_or(ReputationFinalizedArchiveStartupErrorV1::StartupBoundary {
            reason: "committed State predecessor has no authenticated V2 finality receipt",
        })?;
    archive
        .capture_kura_authenticated_view(&startup.state_view, kura, &receipt)
        .map_err(|source| archive_startup_error("pending-tip predecessor capture", source))?;
    Ok(())
}

fn prepare_pending_archive_mode(
    archive: &ReputationFinalizedArchive,
    chain_id: &ChainId,
    kura: &Kura,
    startup: &AuthenticatedArchiveStartupV1<'_>,
    archive_empty: bool,
    pending_tip_height: u64,
) -> ArchiveStartupResultV1<ReputationFinalizedArchiveStartupModeV1> {
    let (qualification, activation_floor_created) = if archive_empty {
        if startup.state_height == 0 {
            (None, false)
        } else {
            capture_pending_archive_predecessor(archive, kura, startup)?;
            (
                Some(qualify_pending_archive(archive, chain_id, kura)?),
                true,
            )
        }
    } else {
        (
            Some(qualify_pending_archive(archive, chain_id, kura)?),
            false,
        )
    };
    validate_pending_archive_tip(
        pending_tip_height,
        startup.state_height,
        qualification
            .as_ref()
            .map(|qualification| qualification.archive_tip().height),
    )
    .map_err(|reason| ReputationFinalizedArchiveStartupErrorV1::StartupBoundary { reason })?;
    Ok(ReputationFinalizedArchiveStartupModeV1::PendingTipReplay {
        pending_tip_height,
        qualification,
        activation_floor_created,
    })
}

fn prepare_archive_startup_mode(
    archive: &ReputationFinalizedArchive,
    chain_id: &ChainId,
    kura: &Kura,
    startup: &AuthenticatedArchiveStartupV1<'_>,
    archive_empty: bool,
    maximum_kura_tip_lag_blocks: u64,
) -> ArchiveStartupResultV1<ReputationFinalizedArchiveStartupModeV1> {
    match startup.boundary {
        ArchiveStartupBoundaryV1::Bootstrap => {
            if !archive_empty {
                return Err(ReputationFinalizedArchiveStartupErrorV1::StartupBoundary {
                    reason: "height-zero bootstrap requires a completely empty archive namespace",
                });
            }
            Ok(ReputationFinalizedArchiveStartupModeV1::BootstrapAwaitingGenesisCapture)
        }
        ArchiveStartupBoundaryV1::Qualified => qualify_existing_archive(
            archive,
            chain_id,
            kura,
            startup,
            maximum_kura_tip_lag_blocks,
        ),
        ArchiveStartupBoundaryV1::PendingTip { height } => {
            prepare_pending_archive_mode(archive, chain_id, kura, startup, archive_empty, height)
        }
    }
}

/// Open and qualify the deployment-owned reputation archive before consensus.
///
/// The exact Kura-tip reconciliation always uses a zero-gap barrier. Only
/// after it succeeds is the configured live-lag allowance evaluated. An
/// existing nonempty chain may establish an explicit activation floor at its
/// current tip; callers must surface that floor rather than claiming earlier
/// historical coverage. Height-zero startup accepts only a completely empty
/// namespace and defers qualification until genesis capture. An authenticated
/// pending V2 tip admits only the exact pending height or its immediate
/// predecessor so Apply can finish either side of the archive-capture crash
/// boundary.
///
/// # Errors
///
/// Fails for invalid resource bounds, unsafe durable storage, a substituted
/// State/Kura/pending-tip boundary, nonempty height-zero storage, incomplete
/// archive coverage, or a configured lag violation.
pub(crate) fn prepare_reputation_finalized_archive_v1(
    config: &SorafsReputationRuntime,
    chain_id: &ChainId,
    state: &Arc<State>,
    kura: &Arc<Kura>,
    startup_replay_plan: &V2StartupReplayPlan,
    retention_authority: Option<Arc<dyn ReputationFinalizedArchiveRetentionAuthorityV1>>,
) -> std::result::Result<
    PreparedReputationFinalizedArchiveV1,
    ReputationFinalizedArchiveStartupErrorV1,
> {
    let (archive, retention_authority) =
        open_reputation_finalized_archive(config, chain_id, kura.as_ref(), retention_authority)?;
    let startup = authenticate_archive_startup(state.as_ref(), kura.as_ref(), startup_replay_plan)?;
    let archive_empty = archive.is_empty().map_err(|source| {
        archive_startup_error("complete bootstrap namespace validation", source)
    })?;
    let startup_mode = prepare_archive_startup_mode(
        archive.as_ref(),
        chain_id,
        kura.as_ref(),
        &startup,
        archive_empty,
        config.finalized_archive_max_kura_tip_lag_blocks,
    )?;
    if let Some(authority) = &retention_authority {
        authority.revalidate()?;
    }
    let activation = ReputationFinalizedArchiveActivationV1 {
        chain_id: chain_id.clone(),
        archive: Arc::clone(&archive),
        state: Arc::clone(state),
        kura: Arc::clone(kura),
        maximum_kura_tip_lag_blocks: config.finalized_archive_max_kura_tip_lag_blocks,
        gate: startup_mode.activation_gate(),
    };
    Ok(PreparedReputationFinalizedArchiveV1 {
        archive,
        startup_mode,
        activation,
        retention_authority,
    })
}

/// Production finalized-query adapter over immutable reputation archive rows.
#[derive(Debug)]
pub struct ArchivedReputationFinalizedQueryV1 {
    handle: String,
    qualification: ReputationRuntimeProviderQualificationV1,
    archive: Arc<ReputationFinalizedArchive>,
}

impl ArchivedReputationFinalizedQueryV1 {
    /// Bind one public runtime identity and qualification to a durable archive.
    ///
    /// This constructor accepts no credentials or signing authority. The
    /// qualification digest must be derived independently from the configured
    /// reputation ingest policy.
    ///
    /// # Errors
    ///
    /// Rejects test-marked or malformed handles, an unsupported provider
    /// revision, a zero policy digest, or an empty archive root.
    pub fn try_new(
        handle: impl Into<String>,
        qualification: ReputationRuntimeProviderQualificationV1,
        archive: Arc<ReputationFinalizedArchive>,
    ) -> Result<Self> {
        let handle = handle.into();
        if !is_production_runtime_handle(&handle) {
            bail!("finalized reputation archive handle is not production-safe");
        }
        if qualification.revision() != REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1
            || qualification.policy_digest() == [0; 32]
        {
            bail!("finalized reputation archive qualification is invalid");
        }
        if archive.root().as_os_str().is_empty() {
            bail!("finalized reputation archive root is not durable");
        }
        Ok(Self {
            handle,
            qualification,
            archive,
        })
    }

    fn ensure_at_or_above_activation_floor(
        &self,
        chain_id: &ChainId,
        height: u64,
    ) -> ExternalResult<()> {
        if self
            .archive
            .activation_floor(chain_id)
            .map_err(|_| external_failure(FAILURE_ARCHIVE_READ))?
            .is_some_and(|floor| height < floor.height)
        {
            return Err(external_failure(FAILURE_BELOW_ACTIVATION_FLOOR));
        }
        Ok(())
    }

    fn select_at_or_before(
        &self,
        chain_id: &ChainId,
        maximum_height: u64,
    ) -> ExternalResult<ReputationFinalizedProjectionV1> {
        if chain_id.as_str().is_empty() || maximum_height == 0 {
            return Err(external_failure(FAILURE_INVALID_REQUEST));
        }
        self.ensure_at_or_above_activation_floor(chain_id, maximum_height)?;
        self.archive
            .latest_at_or_before(chain_id, maximum_height)
            .map_err(|_| external_failure(FAILURE_ARCHIVE_READ))?
            .ok_or_else(|| external_failure(FAILURE_MISSING_ANCHOR))
    }

    fn select_delivery_at_or_before(
        &self,
        chain_id: &ChainId,
        maximum_height: u64,
    ) -> ExternalResult<(ReputationFinalizedProjectionV1, Vec<AuthorityPolicyRecord>)> {
        if chain_id.as_str().is_empty() || maximum_height == 0 {
            return Err(external_failure(FAILURE_INVALID_REQUEST));
        }
        self.ensure_at_or_above_activation_floor(chain_id, maximum_height)?;
        self.archive
            .latest_at_or_before_with_policy_history(chain_id, maximum_height)
            .map_err(|_| external_failure(FAILURE_ARCHIVE_READ))?
            .ok_or_else(|| external_failure(FAILURE_MISSING_ANCHOR))
    }

    fn load_exact(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
    ) -> ExternalResult<ReputationFinalizedProjectionV1> {
        if anchor.chain_id.as_str().is_empty()
            || anchor.identity.height == 0
            || anchor.identity.block_hash == [0; 32]
            || anchor.finalized_at_unix_ms == 0
            || anchor.finalized_at_unix_ms == u64::MAX
        {
            return Err(external_failure(FAILURE_INVALID_REQUEST));
        }
        let key = ReputationFinalizedArchiveKeyV1::try_new(
            anchor.chain_id.clone(),
            anchor.identity.height,
            anchor.identity.block_hash,
        )
        .map_err(|_| external_failure(FAILURE_INVALID_REQUEST))?;
        self.ensure_at_or_above_activation_floor(&anchor.chain_id, key.height)?;
        let projection = self
            .archive
            .get_exact(&key)
            .map_err(|_| external_failure(FAILURE_ARCHIVE_READ))?
            .ok_or_else(|| external_failure(FAILURE_MISSING_ANCHOR))?;
        if projection.finalized_at_unix_ms != anchor.finalized_at_unix_ms {
            return Err(external_failure(FAILURE_ANCHOR_MISMATCH));
        }
        Ok(projection)
    }

    fn journal_page_from_projection(
        projection: &ReputationFinalizedProjectionV1,
        after: Option<ReputationJournalFinalizedEventCursorV1>,
        limit: u32,
    ) -> ExternalResult<ReputationJournalFinalizedEventPageV1> {
        let finalized_cursor = ReputationJournalFinalizedCursorV1 {
            height: projection.key.height,
            block_hash: projection.key.block_hash,
            finalized_at_unix_ms: projection.finalized_at_unix_ms,
        };
        let page = materialize_bounded_page(
            &projection.journal_events,
            after,
            limit,
            REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1,
            REPUTATION_JOURNAL_QUERY_MAX_EVENT_PAGE_BYTES_V1,
            ReputationJournalFinalizedEventV1::cursor,
            |events, has_more, next_after| ReputationJournalFinalizedEventPageV1 {
                finalized_cursor,
                events,
                has_more,
                next_after,
            },
        )?;
        page.validate_after(after)
            .map_err(|_| external_failure(FAILURE_PAGE_BOUNDS))?;
        Ok(page)
    }
}

impl ReputationRuntimeProviderV1 for ArchivedReputationFinalizedQueryV1 {
    fn handle(&self) -> &str {
        &self.handle
    }

    fn qualification(&self) -> ExternalResult<ReputationRuntimeProviderQualificationV1> {
        // Provider identity readiness is valid before genesis capture, but it
        // still rescans the complete empty namespace. Once any record exists,
        // require a synchronized nonempty generation. Data reads and runtime
        // activation remain unavailable until exact Kura-tip qualification.
        let empty = self
            .archive
            .is_empty()
            .map_err(|_| external_failure(FAILURE_ARCHIVE_READ))?;
        if !empty {
            self.archive
                .health_generation()
                .map_err(|_| external_failure(FAILURE_ARCHIVE_READ))?;
        }
        Ok(self.qualification)
    }
}

impl ReputationFinalizedQueryV1 for ArchivedReputationFinalizedQueryV1 {
    fn finalized_at_or_before(
        &self,
        chain_id: &ChainId,
        maximum_height: u64,
    ) -> ExternalResult<ReputationFinalizedAnchorV1> {
        let projection = self.select_at_or_before(chain_id, maximum_height)?;
        Ok(anchor_from_projection(&projection))
    }

    fn reputation_journal_delivery_view(
        &self,
        chain_id: &ChainId,
        maximum_height: u64,
        _policy_query: FindSorafsReputationJournalAuthorityPolicy,
        after: Option<ReputationJournalFinalizedEventCursorV1>,
        limit: u32,
    ) -> ExternalResult<ReputationJournalDeliveryFinalizedViewV1> {
        let (projection, authority_policy_history) =
            self.select_delivery_at_or_before(chain_id, maximum_height)?;
        let journal_page = Self::journal_page_from_projection(&projection, after, limit)?;
        let view = ReputationJournalDeliveryFinalizedViewV1 {
            anchor: anchor_from_projection(&projection),
            authority_policy_history,
            authority_policy: projection.authority_policy,
            journal_page,
        };
        view.validate_for_request(chain_id, after, limit, maximum_height)
            .map_err(|_| external_failure(FAILURE_PAGE_BOUNDS))?;
        Ok(view)
    }

    fn reputation_journal_event_by_source_id(
        &self,
        chain_id: &ChainId,
        maximum_height: u64,
        query: FindSorafsReputationJournalEventBySourceId,
    ) -> ExternalResult<ReputationJournalSourceFinalizedViewV1> {
        if chain_id.as_str().is_empty()
            || maximum_height == 0
            || query.source_id == ReputationJournalSourceIdV1::ZERO
        {
            return Err(external_failure(FAILURE_INVALID_REQUEST));
        }
        let source_view = match query.expected_finalized_cursor {
            Some(cursor) => {
                cursor
                    .validate()
                    .map_err(|_| external_failure(FAILURE_INVALID_REQUEST))?;
                if cursor.height > maximum_height {
                    return Err(external_failure(FAILURE_PAGE_BOUNDS));
                }
                self.ensure_at_or_above_activation_floor(chain_id, cursor.height)?;
                let key = ReputationFinalizedArchiveKeyV1::try_new(
                    chain_id.clone(),
                    cursor.height,
                    cursor.block_hash,
                )
                .map_err(|_| external_failure(FAILURE_INVALID_REQUEST))?;
                let source_view = self
                    .archive
                    .journal_event_by_source_at_exact(&key, query.source_id)
                    .map_err(|_| external_failure(FAILURE_ARCHIVE_READ))?
                    .ok_or_else(|| external_failure(FAILURE_MISSING_ANCHOR))?;
                if source_view.finalized_at_unix_ms != cursor.finalized_at_unix_ms {
                    return Err(external_failure(FAILURE_ANCHOR_MISMATCH));
                }
                source_view
            }
            None => {
                self.ensure_at_or_above_activation_floor(chain_id, maximum_height)?;
                self.archive
                    .latest_journal_event_by_source_at_or_before(
                        chain_id,
                        maximum_height,
                        query.source_id,
                    )
                    .map_err(|_| external_failure(FAILURE_ARCHIVE_READ))?
                    .ok_or_else(|| external_failure(FAILURE_MISSING_ANCHOR))?
            }
        };
        let view = ReputationJournalSourceFinalizedViewV1 {
            anchor: ReputationFinalizedAnchorV1 {
                chain_id: source_view.key.chain_id,
                identity: ReputationFinalizedIdentityV1 {
                    height: source_view.key.height,
                    block_hash: source_view.key.block_hash,
                },
                finalized_at_unix_ms: source_view.finalized_at_unix_ms,
            },
            event: source_view.event,
        };
        view.validate_for_request(chain_id, maximum_height, query)
            .map_err(|_| external_failure(FAILURE_PAGE_BOUNDS))?;
        Ok(view)
    }

    fn proof_outcome_page(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after: Option<ProofOutcomeFinalizedEventCursorV1>,
        limit: u32,
    ) -> ExternalResult<ProofOutcomeFinalizedEventPageV1> {
        let projection = self.load_exact(anchor)?;
        let finalized_cursor = ProofOutcomeFinalizedCursorV1 {
            height: projection.key.height,
            block_hash: projection.key.block_hash,
        };
        materialize_bounded_page(
            &projection.proof_outcomes,
            after,
            limit,
            PROOF_OUTCOME_QUERY_MAX_ITEMS_V1,
            PROOF_OUTCOME_QUERY_MAX_EVENT_PAGE_BYTES_V1,
            ProofOutcomeFinalizedEventV1::cursor,
            |events, has_more, next_after| ProofOutcomeFinalizedEventPageV1 {
                finalized_cursor,
                events,
                has_more,
                next_after,
            },
        )
    }

    fn reputation_journal_page(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after: Option<ReputationJournalFinalizedEventCursorV1>,
        limit: u32,
    ) -> ExternalResult<ReputationJournalFinalizedEventPageV1> {
        let projection = self.load_exact(anchor)?;
        Self::journal_page_from_projection(&projection, after, limit)
    }

    fn repair_page(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after: Option<RepairFinalizedEventCursorV1>,
        limit: u32,
    ) -> ExternalResult<RepairFinalizedEventPageV1> {
        let projection = self.load_exact(anchor)?;
        let finalized_cursor = RepairFinalizedCursorV1 {
            height: projection.key.height,
            block_hash: projection.key.block_hash,
        };
        materialize_bounded_page(
            &projection.repair_events,
            after,
            limit,
            usize::try_from(REPAIR_QUERY_MAX_ITEMS_V1)
                .map_err(|_| external_failure(FAILURE_PAGE_BOUNDS))?,
            REPAIR_QUERY_MAX_EVENT_PAGE_BYTES_V1,
            RepairFinalizedEventV1::cursor,
            |events, has_more, next_after| RepairFinalizedEventPageV1 {
                finalized_cursor,
                events,
                has_more,
                next_after,
            },
        )
    }

    fn orderbook_page(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after: Option<OrderbookFinalizedEventCursorV1>,
        limit: u32,
    ) -> ExternalResult<OrderbookFinalizedEventPageV1> {
        let projection = self.load_exact(anchor)?;
        let finalized_cursor = OrderbookFinalizedCursorV1 {
            height: projection.key.height,
            block_hash: projection.key.block_hash,
        };
        materialize_bounded_page(
            &projection.orderbook_events,
            after,
            limit,
            usize::try_from(ORDERBOOK_QUERY_MAX_ITEMS_V1)
                .map_err(|_| external_failure(FAILURE_PAGE_BOUNDS))?,
            ORDERBOOK_QUERY_MAX_EVENT_PAGE_BYTES_V1,
            OrderbookFinalizedEventV1::cursor,
            |events, has_more, next_after| OrderbookFinalizedEventPageV1 {
                finalized_cursor,
                events,
                has_more,
                next_after,
            },
        )
    }

    fn reserve_page(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after: Option<ReserveFinalizedEventCursorV1>,
        limit: u32,
    ) -> ExternalResult<ReserveFinalizedEventPageV1> {
        let projection = self.load_exact(anchor)?;
        let finalized_cursor = ReserveFinalizedCursorV1 {
            height: projection.key.height,
            block_hash: projection.key.block_hash,
        };
        materialize_bounded_page(
            &projection.reserve_events,
            after,
            limit,
            usize::try_from(RESERVE_QUERY_MAX_ITEMS_V1)
                .map_err(|_| external_failure(FAILURE_PAGE_BOUNDS))?,
            RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1,
            ReserveFinalizedEventV1::cursor,
            |events, has_more, next_after| ReserveFinalizedEventPageV1 {
                finalized_cursor,
                events,
                has_more,
                next_after,
            },
        )
    }

    fn reserve_provider_page(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after_provider_id: Option<ProviderId>,
        limit: u32,
    ) -> ExternalResult<ReserveProviderAccountPageV1> {
        let projection = self.load_exact(anchor)?;
        let finalized_cursor = ReserveFinalizedCursorV1 {
            height: projection.key.height,
            block_hash: projection.key.block_hash,
        };
        materialize_bounded_page(
            &projection.reserve_providers,
            after_provider_id,
            limit,
            usize::try_from(RESERVE_QUERY_MAX_ITEMS_V1)
                .map_err(|_| external_failure(FAILURE_PAGE_BOUNDS))?,
            RESERVE_QUERY_MAX_EVENT_PAGE_BYTES_V1,
            |account| account.terms.provider_id,
            |accounts, has_more, next_after| ReserveProviderAccountPageV1 {
                finalized_cursor,
                accounts,
                has_more,
                next_after,
            },
        )
    }
}

type ExternalResult<T> = std::result::Result<T, ReputationExternalFailureV1>;

fn anchor_from_projection(
    projection: &ReputationFinalizedProjectionV1,
) -> ReputationFinalizedAnchorV1 {
    ReputationFinalizedAnchorV1 {
        chain_id: projection.key.chain_id.clone(),
        identity: ReputationFinalizedIdentityV1 {
            height: projection.key.height,
            block_hash: projection.key.block_hash,
        },
        finalized_at_unix_ms: projection.finalized_at_unix_ms,
    }
}

fn materialize_bounded_page<T, C, P>(
    all: &[T],
    after: Option<C>,
    limit: u32,
    maximum_items: usize,
    maximum_encoded_bytes: usize,
    cursor: impl Fn(&T) -> C,
    build: impl Fn(Vec<T>, bool, Option<C>) -> P,
) -> ExternalResult<P>
where
    T: Clone,
    C: Copy + Eq,
    P: NoritoSerialize,
{
    let limit = usize::try_from(limit).map_err(|_| external_failure(FAILURE_INVALID_LIMIT))?;
    if limit == 0 || limit > maximum_items {
        return Err(external_failure(FAILURE_INVALID_LIMIT));
    }
    let start = if let Some(after) = after {
        all.iter()
            .position(|item| cursor(item) == after)
            .and_then(|index| index.checked_add(1))
            .ok_or_else(|| external_failure(FAILURE_INVALID_CURSOR))?
    } else {
        0
    };
    let maximum_end = start.saturating_add(limit).min(all.len());
    let minimum_end = if start < maximum_end {
        start.saturating_add(1)
    } else {
        start
    };
    let maximum_rows = all[start..maximum_end].to_vec();
    let maximum_has_more = maximum_end < all.len();
    let maximum_next_after = maximum_has_more.then(|| {
        cursor(
            maximum_rows
                .last()
                .expect("a positive page limit produces a row when more rows remain"),
        )
    });
    let maximum_page = build(maximum_rows, maximum_has_more, maximum_next_after);
    let maximum_encoded =
        norito::to_bytes(&maximum_page).map_err(|_| external_failure(FAILURE_PAGE_BOUNDS))?;
    if maximum_encoded.len() <= maximum_encoded_bytes {
        return Ok(maximum_page);
    }
    if minimum_end == maximum_end {
        return Err(external_failure(FAILURE_PAGE_BOUNDS));
    }

    let mut lower = minimum_end;
    let mut upper = maximum_end - 1;
    let mut best = None;
    while lower <= upper {
        let end = lower.saturating_add(upper.saturating_sub(lower) / 2);
        let rows = all[start..end].to_vec();
        let has_more = end < all.len();
        let next_after = has_more.then(|| {
            cursor(
                rows.last()
                    .expect("a positive page limit produces a row when more rows remain"),
            )
        });
        let page = build(rows, has_more, next_after);
        let encoded = norito::to_bytes(&page).map_err(|_| external_failure(FAILURE_PAGE_BOUNDS))?;
        if encoded.len() <= maximum_encoded_bytes {
            best = Some(page);
            lower = end + 1;
        } else {
            upper = end - 1;
        }
    }
    best.ok_or_else(|| external_failure(FAILURE_PAGE_BOUNDS))
}

fn external_failure(marker: u8) -> ReputationExternalFailureV1 {
    ReputationExternalFailureV1::try_new([marker; 32])
        .expect("fixed finalized-query failure markers are non-zero")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use iroha_config::base::util::Bytes;
    use iroha_core::{
        query::{
            reputation_finalized::ReputationFinalizedArchiveInsertOutcome, store::LiveQueryStore,
        },
        state::World,
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        sorafs::reputation::{
            PorTerminalOutcomeV1, PorTerminalStatusV1,
            REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
            ReputationFinalizedArchiveRetentionTargetV1, ReputationJournalAuthorityPolicyRecordV1,
            ReputationJournalAuthorityPolicyV1, ReputationJournalEntryV1,
            ReputationJournalPayloadV1,
        },
    };
    use tempfile::TempDir;

    use super::*;

    const CHAIN_ID: &str = "reputation-archive-chain";
    const FINALIZED_AT_MS: u64 = 1_800_000_010_000;

    fn runtime_config(root: PathBuf) -> SorafsReputationRuntime {
        SorafsReputationRuntime {
            state_dir: root.with_extension("state"),
            finalized_archive_root: root,
            finalized_archive_max_record_bytes: 4 * 1024 * 1024,
            finalized_archive_max_entries: 32,
            finalized_archive_max_total_bytes: 64 * 1024 * 1024,
            finalized_archive_max_kura_tip_lag_blocks: 0,
            finalized_archive_retention_authority: None,
            window_start_height: 1,
            window_end_height: 10,
            finalized_query_handle: "reputation-archive:region-a".to_owned(),
            journal_checkpoint_provider_handle: "sealed.reputation.journal.region-a".to_owned(),
            journal_checkpoint_provider_revision: 1,
            journal_checkpoint_provider_policy_digest: [0x60; 32],
            journal_transaction_submitter_handle: "reputation-journal:region-a".to_owned(),
            journal_transaction_submitter_revision: 1,
            journal_transaction_submitter_policy_digest: [0x61; 32],
            threshold_signer_handle: "reputation-threshold:region-a".to_owned(),
            threshold_signer_revision: 1,
            threshold_signer_policy_digest: [0x62; 32],
            governance_dag_handle: "reputation-dag:region-a".to_owned(),
            governance_dag_revision: 1,
            governance_dag_policy_digest: [0x63; 32],
            governance_publisher_peer_id: b"12D3KooWReputationPublisher".to_vec(),
            governance_publisher_public_key: [0x73; 32],
            poll_interval: std::time::Duration::from_secs(1),
            page_items: 64,
            max_pages_per_batch: 64,
            max_providers: 1_024,
            max_pending_events: 1_024,
            max_replay_receipts: 4_096,
            max_material_delivery_failures: 32,
            ingest_checkpoint_max_bytes: Bytes(4 * 1024 * 1024),
            publication_checkpoint_max_bytes: Bytes(4 * 1024 * 1024),
            por_success_bps: 2_200,
            pdp_success_bps: 2_000,
            potr_success_bps: 1_800,
            latency_bps: 1_500,
            dispute_bps: 1_000,
            token_violation_bps: 500,
            repair_breach_bps: 1_000,
        }
    }

    fn retention_request(
        target_height: u64,
        target_hash: [u8; 32],
    ) -> ReputationFinalizedArchiveRetentionRequestV1 {
        ReputationFinalizedArchiveRetentionRequestV1::try_new(
            ChainId::from(CHAIN_ID),
            1,
            None,
            ReputationFinalizedArchiveRetentionTargetV1::try_new(target_height, target_hash)
                .expect("valid target"),
        )
        .expect("valid request")
    }

    fn archive_key(height: u64, block_hash: [u8; 32]) -> ReputationFinalizedArchiveKeyV1 {
        ReputationFinalizedArchiveKeyV1::try_new(ChainId::from(CHAIN_ID), height, block_hash)
            .expect("valid archive key")
    }

    #[test]
    fn explicit_retention_classification_is_fresh_monotonic_and_idempotent() {
        let request = retention_request(7, [0x71; 32]);
        let authorization = archive_key(8, [0x81; 32]);
        let physical_floor = archive_key(1, [0x11; 32]);
        assert_eq!(
            classify_retention_request(&request, &authorization, &physical_floor, None,)
                .expect("physical target remains actionable"),
            ReputationRetentionDecisionV1::Apply(archive_key(7, [0x71; 32]))
        );

        let checkpoint_floor = archive_key(7, [0x71; 32]);
        assert_eq!(
            classify_retention_request(
                &request,
                &authorization,
                &checkpoint_floor,
                Some([0xC1; 32]),
            )
            .expect("exact checkpoint is idempotent"),
            ReputationRetentionDecisionV1::AlreadyApplied(checkpoint_floor)
        );
        assert!(matches!(
            classify_retention_request(
                &request,
                &authorization,
                &archive_key(7, [0x72; 32]),
                Some([0xC2; 32]),
            ),
            Err(ReputationFinalizedArchiveRetentionControlErrorV1::Boundary { .. })
        ));
        assert!(matches!(
            classify_retention_request(
                &request,
                &archive_key(7, [0x73; 32]),
                &physical_floor,
                None,
            ),
            Err(ReputationFinalizedArchiveRetentionControlErrorV1::Boundary { .. })
        ));
        assert!(matches!(
            classify_retention_request(&request, &authorization, &archive_key(9, [0x91; 32]), None,),
            Err(ReputationFinalizedArchiveRetentionControlErrorV1::Boundary { .. })
        ));
    }

    #[test]
    fn enabled_retention_fails_before_open_without_injected_authority() {
        let directory = TempDir::new().expect("create archive directory");
        let root = std::fs::canonicalize(directory.path())
            .expect("canonicalize archive directory")
            .join("archive");
        let mut config = runtime_config(root);
        config.finalized_archive_retention_authority = Some(
            iroha_config::parameters::actual::SorafsReputationFinalizedArchiveRetentionAuthority {
                handle: "sealed.reputation.archive.primary".to_owned(),
                revision: 7,
                policy_digest: [0xA7; 32],
            },
        );
        let kura = Kura::blank_kura_for_testing();
        let chain_id = ChainId::from("reputation-retention-missing");

        assert!(matches!(
            open_reputation_finalized_archive(&config, &chain_id, kura.as_ref(), None,),
            Err(ReputationFinalizedArchiveStartupErrorV1::RetentionAuthorityConfiguration { .. })
        ));
    }

    #[test]
    fn fresh_height_zero_opens_empty_archive_for_genesis_capture() {
        let directory = TempDir::new().expect("create archive directory");
        let root = std::fs::canonicalize(directory.path())
            .expect("canonicalize archive directory")
            .join("archive");
        let config = runtime_config(root);
        let kura = Kura::blank_kura_for_testing();
        let chain_id = ChainId::from("reputation-empty-state");
        let state = Arc::new(State::new_with_chain_for_testing(
            World::default(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
            chain_id.clone(),
        ));
        let replay_plan =
            iroha_core::sumeragi::plan_v2_startup_replay(kura.as_ref()).expect("startup plan");
        let prepared = prepare_reputation_finalized_archive_v1(
            &config,
            &chain_id,
            &state,
            &kura,
            &replay_plan,
            None,
        )
        .expect("fresh empty archive must await genesis capture");
        assert!(matches!(
            prepared.startup_mode(),
            ReputationFinalizedArchiveStartupModeV1::BootstrapAwaitingGenesisCapture
        ));
        assert!(prepared.archive().is_empty().expect("empty archive"));
        assert!(
            !prepared
                .activation()
                .activation_ready()
                .expect("bootstrap activation gate"),
            "identity readiness must not pretend genesis is already captured"
        );
    }

    #[test]
    fn fresh_height_zero_rejects_archive_for_another_chain() {
        let directory = TempDir::new().expect("create archive directory");
        let root = std::fs::canonicalize(directory.path())
            .expect("canonicalize archive directory")
            .join("archive");
        let config = runtime_config(root.clone());
        let bounds = ReputationFinalizedArchiveBounds::try_new(
            config.finalized_archive_max_record_bytes,
            config.finalized_archive_max_entries,
            config.finalized_archive_max_total_bytes,
        )
        .expect("archive bounds");
        {
            let archive =
                ReputationFinalizedArchive::try_open(&root, bounds).expect("open stale archive");
            archive
                .insert(projection(1, [0x71; 32], Vec::new()))
                .expect("insert stale anchor");
        }
        let kura = Kura::blank_kura_for_testing();
        let chain_id = ChainId::from("different-fresh-chain");
        let state = Arc::new(State::new_with_chain_for_testing(
            World::default(),
            Arc::clone(&kura),
            LiveQueryStore::start_test(),
            chain_id.clone(),
        ));
        let replay_plan =
            iroha_core::sumeragi::plan_v2_startup_replay(kura.as_ref()).expect("startup plan");
        let error = prepare_reputation_finalized_archive_v1(
            &config,
            &chain_id,
            &state,
            &kura,
            &replay_plan,
            None,
        )
        .expect_err("height-zero startup must reject any retained archive namespace");
        assert!(matches!(
            error,
            ReputationFinalizedArchiveStartupErrorV1::StartupBoundary { .. }
        ));
    }

    #[test]
    fn pending_boundary_allows_only_exact_tip_or_predecessor() {
        assert_eq!(
            classify_archive_startup_boundary(7, 8, Some(8)),
            Ok(ArchiveStartupBoundaryV1::PendingTip { height: 8 })
        );
        assert_eq!(
            classify_archive_startup_boundary(8, 8, Some(8)),
            Ok(ArchiveStartupBoundaryV1::PendingTip { height: 8 })
        );
        assert!(classify_archive_startup_boundary(6, 8, Some(8)).is_err());
        assert!(classify_archive_startup_boundary(7, 8, None).is_err());
        assert!(classify_archive_startup_boundary(7, 8, Some(9)).is_err());

        assert_eq!(validate_pending_archive_tip(8, 7, Some(7)), Ok(()));
        assert_eq!(validate_pending_archive_tip(8, 7, Some(8)), Ok(()));
        assert_eq!(validate_pending_archive_tip(8, 8, Some(8)), Ok(()));
        assert!(validate_pending_archive_tip(8, 8, Some(7)).is_err());
        assert!(validate_pending_archive_tip(8, 7, Some(6)).is_err());
        assert!(validate_pending_archive_tip(8, 7, Some(9)).is_err());
        assert!(validate_pending_archive_tip(8, 7, None).is_err());
        assert_eq!(validate_pending_archive_tip(1, 0, None), Ok(()));
        assert!(validate_pending_archive_tip(1, 1, None).is_err());

        let gate = ArchiveActivationGateV1::PendingTip { height: 8 };
        assert!(!gate.accepts_visible_archive_tip(7));
        assert!(gate.accepts_visible_archive_tip(8));
        assert!(gate.accepts_visible_archive_tip(9));
        let pending_mode = ReputationFinalizedArchiveStartupModeV1::PendingTipReplay {
            pending_tip_height: 8,
            qualification: None,
            activation_floor_created: false,
        };
        assert_eq!(pending_mode.activation_gate(), gate);
        assert_eq!(
            ReputationFinalizedArchiveStartupModeV1::BootstrapAwaitingGenesisCapture
                .activation_gate(),
            ArchiveActivationGateV1::AwaitingGenesis
        );

        assert_eq!(classify_pending_replay_completion(8, 8, Some(8)), Ok(false));
        assert_eq!(classify_pending_replay_completion(8, 8, None), Ok(true));
        assert_eq!(classify_pending_replay_completion(8, 9, None), Ok(true));
        assert_eq!(classify_pending_replay_completion(8, 9, Some(9)), Ok(true));
        assert!(classify_pending_replay_completion(8, 7, None).is_err());
        assert!(classify_pending_replay_completion(8, 10, Some(9)).is_err());
        assert!(classify_pending_replay_completion(8, 8, Some(7)).is_err());
    }

    fn account(seed: u8) -> AccountId {
        let keypair = KeyPair::try_from_seed(vec![seed.max(1); 32], Algorithm::Ed25519)
            .expect("derive deterministic account");
        AccountId::new(keypair.public_key().clone())
    }

    fn authority_policy() -> ReputationJournalAuthorityPolicyV1 {
        ReputationJournalAuthorityPolicyV1 {
            version: REPUTATION_JOURNAL_AUTHORITY_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            por_recorder_authority: account(1),
            dispute_recorder_authority: account(2),
            token_recorder_authority: account(3),
            max_source_age_ms: 86_400_000,
        }
    }

    fn authority_record() -> ReputationJournalAuthorityPolicyRecordV1 {
        ReputationJournalAuthorityPolicyRecordV1::try_new(
            authority_policy(),
            account(4),
            FINALIZED_AT_MS - 1_000,
        )
        .expect("valid authority policy record")
    }

    fn journal_event(
        sequence: u64,
        event_index: u32,
        marker: u8,
        height: u64,
        block_hash: [u8; 32],
    ) -> ReputationJournalFinalizedEventV1 {
        let policy = authority_policy();
        let outcome = PorTerminalOutcomeV1 {
            challenge_id: [marker; 32],
            manifest_digest: [0x41; 32],
            epoch_id: 7,
            drand_round: 11,
            forced: false,
            sample_count: 4,
            failed_samples: 0,
            issued_at_unix_ms: FINALIZED_AT_MS - 2_000,
            deadline_at_unix_ms: FINALIZED_AT_MS - 500,
            responded_at_unix_ms: Some(FINALIZED_AT_MS - 750),
            decided_at_unix_ms: FINALIZED_AT_MS - 100,
            proof_digest: Some([marker.wrapping_add(1); 32]),
            repair_task_id: None,
            verifier_latency_ms: Some(17),
            status: PorTerminalStatusV1::Verified,
        };
        let entry = ReputationJournalEntryV1::try_new(
            ProviderId::new([marker; 32]),
            policy
                .canonical_digest()
                .expect("canonical authority policy digest"),
            policy.por_recorder_authority,
            outcome.decided_at_unix_ms,
            None,
            ReputationJournalPayloadV1::PorTerminal(outcome),
        )
        .expect("valid journal entry");
        ReputationJournalFinalizedEventV1 {
            sequence,
            block_height: height,
            block_hash,
            event_index,
            recorded_at_unix_ms: FINALIZED_AT_MS - 50,
            entry,
        }
    }

    fn projection(
        height: u64,
        block_hash: [u8; 32],
        journal_events: Vec<ReputationJournalFinalizedEventV1>,
    ) -> ReputationFinalizedProjectionV1 {
        ReputationFinalizedProjectionV1 {
            key: ReputationFinalizedArchiveKeyV1::try_new(
                ChainId::from(CHAIN_ID),
                height,
                block_hash,
            )
            .expect("valid archive key"),
            finalized_at_unix_ms: FINALIZED_AT_MS,
            authority_policy: authority_record(),
            proof_outcomes: Vec::new(),
            journal_events,
            repair_events: Vec::new(),
            orderbook_events: Vec::new(),
            reserve_events: Vec::new(),
            reserve_providers: Vec::new(),
        }
    }

    fn qualification() -> ReputationRuntimeProviderQualificationV1 {
        ReputationRuntimeProviderQualificationV1::new(
            REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1,
            [0xD1; 32],
        )
    }

    fn adapter_with(
        projections: impl IntoIterator<Item = ReputationFinalizedProjectionV1>,
    ) -> (TempDir, ArchivedReputationFinalizedQueryV1) {
        let directory = TempDir::new().expect("create archive directory");
        let bounds =
            ReputationFinalizedArchiveBounds::try_new(4 * 1024 * 1024, 16, 64 * 1024 * 1024)
                .expect("valid archive bounds");
        let root = std::fs::canonicalize(directory.path()).expect("canonicalize archive directory");
        let archive =
            Arc::new(ReputationFinalizedArchive::try_open(root, bounds).expect("open archive"));
        for projection in projections {
            assert_eq!(
                archive.insert(projection).expect("insert projection"),
                ReputationFinalizedArchiveInsertOutcome::Inserted
            );
        }
        let adapter = ArchivedReputationFinalizedQueryV1::try_new(
            "reputation-archive:region-a",
            qualification(),
            archive,
        )
        .expect("construct archived query");
        (directory, adapter)
    }

    fn anchor(height: u64, block_hash: [u8; 32]) -> ReputationFinalizedAnchorV1 {
        ReputationFinalizedAnchorV1 {
            chain_id: ChainId::from(CHAIN_ID),
            identity: ReputationFinalizedIdentityV1 { height, block_hash },
            finalized_at_unix_ms: FINALIZED_AT_MS,
        }
    }

    #[test]
    fn selects_historical_anchor_at_or_before_bound() {
        let (_directory, adapter) = adapter_with([
            projection(7, [0x71; 32], Vec::new()),
            projection(9, [0x91; 32], Vec::new()),
        ]);

        assert_eq!(adapter.handle(), "reputation-archive:region-a");
        assert_eq!(
            adapter.qualification().expect("qualification"),
            qualification()
        );
        assert_eq!(
            adapter
                .finalized_at_or_before(&ChainId::from(CHAIN_ID), 8)
                .expect("select height seven"),
            anchor(7, [0x71; 32])
        );
        assert_eq!(
            adapter
                .finalized_at_or_before(&ChainId::from(CHAIN_ID), 9)
                .expect("select height nine"),
            anchor(9, [0x91; 32])
        );
        assert!(
            adapter
                .finalized_at_or_before(&ChainId::from("another-chain"), 9)
                .is_err()
        );
        assert_eq!(
            adapter
                .finalized_at_or_before(&ChainId::from(CHAIN_ID), 6)
                .expect_err("request below explicit activation floor")
                .receipt(),
            [FAILURE_BELOW_ACTIVATION_FLOOR; 32]
        );
    }

    #[test]
    fn live_qualification_rejects_post_start_archive_corruption() {
        let exact = projection(7, [0x71; 32], Vec::new());
        let (_directory, adapter) = adapter_with([exact.clone()]);
        assert_eq!(
            adapter.qualification().expect("healthy qualification"),
            qualification()
        );

        let path = adapter
            .archive
            .record_path(&exact.key)
            .expect("derive archive record path");
        let mut bytes = std::fs::read(&path).expect("read archive record");
        let last = bytes.last_mut().expect("archive record is non-empty");
        *last ^= 0xFF;
        std::fs::write(path, bytes).expect("corrupt archive record");

        assert!(
            adapter.qualification().is_err(),
            "live qualification must rescan the durable generation"
        );
    }

    #[test]
    fn journal_cursor_and_delivery_view_remain_on_one_archive_row() {
        let block_hash = [0x91; 32];
        let first = journal_event(1, 0, 0x21, 9, block_hash);
        let second = journal_event(2, 1, 0x22, 9, block_hash);
        let (_directory, adapter) = adapter_with([projection(
            9,
            block_hash,
            vec![first.clone(), second.clone()],
        )]);
        let exact_anchor = anchor(9, block_hash);

        let first_page = adapter
            .reputation_journal_page(&exact_anchor, None, 1)
            .expect("first journal page");
        assert_eq!(first_page.events, vec![first.clone()]);
        assert!(first_page.has_more);
        assert_eq!(first_page.next_after, Some(first.cursor()));

        let second_page = adapter
            .reputation_journal_page(&exact_anchor, first_page.next_after, 1)
            .expect("second journal page");
        assert_eq!(second_page.events, vec![second.clone()]);
        assert!(!second_page.has_more);
        assert_eq!(second_page.next_after, None);

        let delivery = adapter
            .reputation_journal_delivery_view(
                &ChainId::from(CHAIN_ID),
                9,
                FindSorafsReputationJournalAuthorityPolicy,
                Some(first.cursor()),
                1,
            )
            .expect("single-row policy and journal view");
        delivery
            .validate_for_request(&ChainId::from(CHAIN_ID), Some(first.cursor()), 1, 9)
            .expect("adapter returns a coherently revalidated delivery view");
        assert_eq!(delivery.anchor, exact_anchor);
        assert_eq!(delivery.authority_policy, authority_record());
        assert_eq!(delivery.authority_policy_history, vec![authority_record()]);
        assert_eq!(delivery.journal_page.events, vec![second.clone()]);

        let finalized_cursor = ReputationJournalFinalizedCursorV1 {
            height: exact_anchor.identity.height,
            block_hash: exact_anchor.identity.block_hash,
            finalized_at_unix_ms: exact_anchor.finalized_at_unix_ms,
        };
        let source_query = FindSorafsReputationJournalEventBySourceId::new(
            second.entry.source_id,
            Some(finalized_cursor),
        );
        let source = adapter
            .reputation_journal_event_by_source_id(&ChainId::from(CHAIN_ID), 9, source_query)
            .expect("source-indexed immutable view");
        source
            .validate_for_request(&ChainId::from(CHAIN_ID), 9, source_query)
            .expect("source response matches request");
        assert_eq!(source.anchor, exact_anchor);
        assert_eq!(source.event, Some(second));

        let absent_query = FindSorafsReputationJournalEventBySourceId::new(
            iroha_data_model::sorafs::reputation::ReputationJournalSourceIdV1::for_por_challenge(
                [0xFE; 32],
            ),
            None,
        );
        let absent = adapter
            .reputation_journal_event_by_source_id(&ChainId::from(CHAIN_ID), 9, absent_query)
            .expect("complete archive proves source absence");
        assert_eq!(absent.event, None);

        let mut stale_cursor = first.cursor();
        stale_cursor.block_hash = [0x92; 32];
        assert!(
            adapter
                .reputation_journal_page(&exact_anchor, Some(stale_cursor), 1)
                .is_err()
        );
    }

    #[test]
    fn source_query_expected_cursor_selects_exact_historical_archive_row() {
        let historical_hash = [0x81; 32];
        let latest_hash = [0x91; 32];
        let historical_event = journal_event(1, 0, 0x31, 8, historical_hash);
        let latest_event = journal_event(2, 0, 0x32, 9, latest_hash);
        let (_directory, adapter) = adapter_with([
            projection(8, historical_hash, vec![historical_event.clone()]),
            projection(9, latest_hash, vec![historical_event.clone(), latest_event]),
        ]);
        let historical_cursor = ReputationJournalFinalizedCursorV1 {
            height: 8,
            block_hash: historical_hash,
            finalized_at_unix_ms: FINALIZED_AT_MS,
        };
        let query = FindSorafsReputationJournalEventBySourceId::new(
            historical_event.entry.source_id,
            Some(historical_cursor),
        );

        let view = adapter
            .reputation_journal_event_by_source_id(&ChainId::from(CHAIN_ID), 9, query)
            .expect("load exact historical source view");

        assert_eq!(view.anchor, anchor(8, historical_hash));
        assert_eq!(view.event, Some(historical_event));
        view.validate_for_request(&ChainId::from(CHAIN_ID), 9, query)
            .expect("historical response honors the expected cursor");

        let timestamp_substitution = FindSorafsReputationJournalEventBySourceId::new(
            query.source_id,
            Some(ReputationJournalFinalizedCursorV1 {
                finalized_at_unix_ms: historical_cursor.finalized_at_unix_ms + 1,
                ..historical_cursor
            }),
        );
        assert_eq!(
            adapter
                .reputation_journal_event_by_source_id(
                    &ChainId::from(CHAIN_ID),
                    9,
                    timestamp_substitution,
                )
                .expect_err("exact archive identity must include finalized time")
                .receipt(),
            [FAILURE_ANCHOR_MISMATCH; 32]
        );
    }

    #[test]
    fn source_query_reports_explicit_activation_floor_before_archive_selection() {
        let floor_hash = [0x71; 32];
        let floor_event = journal_event(1, 0, 0x41, 7, floor_hash);
        let (_directory, adapter) =
            adapter_with([projection(7, floor_hash, vec![floor_event.clone()])]);
        let chain_id = ChainId::from(CHAIN_ID);
        let cursorless =
            FindSorafsReputationJournalEventBySourceId::new(floor_event.entry.source_id, None);
        assert_eq!(
            adapter
                .reputation_journal_event_by_source_id(&chain_id, 6, cursorless)
                .expect_err("cursorless source request below activation floor")
                .receipt(),
            [FAILURE_BELOW_ACTIVATION_FLOOR; 32]
        );

        let below_floor_cursor = ReputationJournalFinalizedCursorV1 {
            height: 6,
            block_hash: [0x61; 32],
            finalized_at_unix_ms: FINALIZED_AT_MS - 1,
        };
        let exact_below_floor = FindSorafsReputationJournalEventBySourceId::new(
            floor_event.entry.source_id,
            Some(below_floor_cursor),
        );
        assert_eq!(
            adapter
                .reputation_journal_event_by_source_id(&chain_id, 7, exact_below_floor)
                .expect_err("exact source request below activation floor")
                .receipt(),
            [FAILURE_BELOW_ACTIVATION_FLOOR; 32]
        );

        let missing_above_floor = FindSorafsReputationJournalEventBySourceId::new(
            floor_event.entry.source_id,
            Some(ReputationJournalFinalizedCursorV1 {
                height: 8,
                block_hash: [0x81; 32],
                finalized_at_unix_ms: FINALIZED_AT_MS + 1,
            }),
        );
        assert_eq!(
            adapter
                .reputation_journal_event_by_source_id(&chain_id, 8, missing_above_floor)
                .expect_err("missing exact source anchor remains distinct from activation floor")
                .receipt(),
            [FAILURE_MISSING_ANCHOR; 32]
        );

        let mismatched_floor_time = FindSorafsReputationJournalEventBySourceId::new(
            floor_event.entry.source_id,
            Some(ReputationJournalFinalizedCursorV1 {
                height: 7,
                block_hash: floor_hash,
                finalized_at_unix_ms: FINALIZED_AT_MS + 1,
            }),
        );
        assert_eq!(
            adapter
                .reputation_journal_event_by_source_id(&chain_id, 7, mismatched_floor_time)
                .expect_err("exact source timestamp mismatch remains distinct")
                .receipt(),
            [FAILURE_ANCHOR_MISMATCH; 32]
        );
    }

    #[test]
    fn source_query_preserves_archive_read_failure_receipt() {
        let block_hash = [0x71; 32];
        let event = journal_event(1, 0, 0x45, 7, block_hash);
        let exact = projection(7, block_hash, vec![event.clone()]);
        let key = exact.key.clone();
        let (_directory, adapter) = adapter_with([exact]);
        let path = adapter
            .archive
            .record_path(&key)
            .expect("derive archive record path");
        let mut bytes = std::fs::read(&path).expect("read archive record");
        let last = bytes.last_mut().expect("archive record is non-empty");
        *last ^= 0x01;
        std::fs::write(path, bytes).expect("corrupt archive record");

        let query = FindSorafsReputationJournalEventBySourceId::new(event.entry.source_id, None);
        assert_eq!(
            adapter
                .reputation_journal_event_by_source_id(&ChainId::from(CHAIN_ID), 7, query)
                .expect_err("corrupt source archive must stay an archive-read failure")
                .receipt(),
            [FAILURE_ARCHIVE_READ; 32]
        );
    }

    #[test]
    fn page_byte_fitting_selects_the_largest_bounded_prefix() {
        let block_hash = [0x91; 32];
        let events = (0_u8..8)
            .map(|offset| {
                journal_event(
                    u64::from(offset) + 1,
                    u32::from(offset),
                    0x20 + offset,
                    9,
                    block_hash,
                )
            })
            .collect::<Vec<_>>();
        let finalized_cursor = ReputationJournalFinalizedCursorV1 {
            height: 9,
            block_hash,
            finalized_at_unix_ms: FINALIZED_AT_MS,
        };
        let three_row_page = ReputationJournalFinalizedEventPageV1 {
            finalized_cursor,
            events: events[..3].to_vec(),
            has_more: true,
            next_after: Some(events[2].cursor()),
        };
        let maximum_bytes = norito::to_bytes(&three_row_page)
            .expect("encode byte-bound fixture")
            .len();
        let page = materialize_bounded_page(
            &events,
            None,
            8,
            8,
            maximum_bytes,
            ReputationJournalFinalizedEventV1::cursor,
            |events, has_more, next_after| ReputationJournalFinalizedEventPageV1 {
                finalized_cursor,
                events,
                has_more,
                next_after,
            },
        )
        .expect("fit largest journal prefix");
        assert_eq!(page.events, events[..3]);
        assert!(page.has_more);
        assert_eq!(page.next_after, Some(events[2].cursor()));
    }

    #[test]
    fn rejects_invalid_page_limits() {
        let block_hash = [0x71; 32];
        let (_directory, adapter) = adapter_with([projection(7, block_hash, Vec::new())]);
        let exact_anchor = anchor(7, block_hash);

        assert!(adapter.proof_outcome_page(&exact_anchor, None, 0).is_err());
        assert!(
            adapter
                .reputation_journal_page(
                    &exact_anchor,
                    None,
                    u32::try_from(REPUTATION_JOURNAL_QUERY_MAX_ITEMS_V1)
                        .expect("journal maximum fits u32")
                        .saturating_add(1),
                )
                .is_err()
        );
        assert!(
            adapter
                .repair_page(
                    &exact_anchor,
                    None,
                    REPAIR_QUERY_MAX_ITEMS_V1.saturating_add(1),
                )
                .is_err()
        );
        assert!(
            adapter
                .orderbook_page(
                    &exact_anchor,
                    None,
                    ORDERBOOK_QUERY_MAX_ITEMS_V1.saturating_add(1),
                )
                .is_err()
        );
        assert!(
            adapter
                .reserve_page(
                    &exact_anchor,
                    None,
                    RESERVE_QUERY_MAX_ITEMS_V1.saturating_add(1),
                )
                .is_err()
        );
        assert!(
            adapter
                .reserve_provider_page(
                    &exact_anchor,
                    None,
                    RESERVE_QUERY_MAX_ITEMS_V1.saturating_add(1),
                )
                .is_err()
        );
    }

    #[test]
    fn rejects_invalid_cursors_and_mismatched_exact_anchors() {
        let block_hash = [0x71; 32];
        let (_directory, adapter) = adapter_with([projection(7, block_hash, Vec::new())]);
        let exact_anchor = anchor(7, block_hash);

        assert!(
            adapter
                .proof_outcome_page(
                    &exact_anchor,
                    Some(ProofOutcomeFinalizedEventCursorV1 {
                        sequence: 1,
                        block_height: 7,
                        block_hash,
                        event_index: 0,
                    }),
                    1,
                )
                .is_err()
        );
        assert!(
            adapter
                .repair_page(
                    &exact_anchor,
                    Some(RepairFinalizedEventCursorV1 {
                        sequence: 1,
                        block_height: 7,
                        block_hash,
                        event_index: 0,
                    }),
                    1,
                )
                .is_err()
        );
        assert!(
            adapter
                .orderbook_page(
                    &exact_anchor,
                    Some(OrderbookFinalizedEventCursorV1 {
                        sequence: 1,
                        block_height: 7,
                        block_hash,
                        event_index: 0,
                    }),
                    1,
                )
                .is_err()
        );
        assert!(
            adapter
                .reserve_page(
                    &exact_anchor,
                    Some(ReserveFinalizedEventCursorV1 {
                        sequence: 1,
                        block_height: 7,
                        block_hash,
                        event_index: 0,
                    }),
                    1,
                )
                .is_err()
        );
        assert!(
            adapter
                .reserve_provider_page(&exact_anchor, Some(ProviderId::new([0x31; 32])), 1,)
                .is_err()
        );

        let mut wrong_hash = exact_anchor.clone();
        wrong_hash.identity.block_hash = [0x72; 32];
        assert!(adapter.proof_outcome_page(&wrong_hash, None, 1).is_err());
        let mut wrong_timestamp = exact_anchor.clone();
        wrong_timestamp.finalized_at_unix_ms += 1;
        assert!(
            adapter
                .proof_outcome_page(&wrong_timestamp, None, 1)
                .is_err()
        );
        let mut wrong_chain = exact_anchor;
        wrong_chain.chain_id = ChainId::from("another-chain");
        assert!(adapter.proof_outcome_page(&wrong_chain, None, 1).is_err());
    }

    #[test]
    fn empty_pages_are_terminal_and_share_the_exact_anchor() {
        let block_hash = [0x71; 32];
        let (_directory, adapter) = adapter_with([projection(7, block_hash, Vec::new())]);
        let exact_anchor = anchor(7, block_hash);

        let proof = adapter
            .proof_outcome_page(&exact_anchor, None, 1)
            .expect("empty proof page");
        assert!(proof.events.is_empty());
        assert!(!proof.has_more);
        assert_eq!(proof.next_after, None);
        assert_eq!(proof.finalized_cursor.height, 7);
        assert_eq!(proof.finalized_cursor.block_hash, block_hash);

        let journal = adapter
            .reputation_journal_page(&exact_anchor, None, 1)
            .expect("empty journal page");
        assert!(journal.events.is_empty());
        assert!(!journal.has_more);
        assert_eq!(journal.next_after, None);
        assert_eq!(
            journal.finalized_cursor.finalized_at_unix_ms,
            FINALIZED_AT_MS
        );

        let repair = adapter
            .repair_page(&exact_anchor, None, 1)
            .expect("empty repair page");
        assert!(repair.events.is_empty());
        assert!(!repair.has_more);
        assert_eq!(repair.next_after, None);

        let orderbook = adapter
            .orderbook_page(&exact_anchor, None, 1)
            .expect("empty orderbook page");
        assert!(orderbook.events.is_empty());
        assert!(!orderbook.has_more);
        assert_eq!(orderbook.next_after, None);

        let reserve = adapter
            .reserve_page(&exact_anchor, None, 1)
            .expect("empty reserve page");
        assert!(reserve.events.is_empty());
        assert!(!reserve.has_more);
        assert_eq!(reserve.next_after, None);

        let providers = adapter
            .reserve_provider_page(&exact_anchor, None, 1)
            .expect("empty provider page");
        assert!(providers.accounts.is_empty());
        assert!(!providers.has_more);
        assert_eq!(providers.next_after, None);
    }

    #[test]
    fn construction_rejects_test_handles_and_invalid_qualification() {
        let directory = TempDir::new().expect("create archive directory");
        let bounds = ReputationFinalizedArchiveBounds::try_new(1 << 20, 2, 2 << 20)
            .expect("valid archive bounds");
        let root = std::fs::canonicalize(directory.path()).expect("canonicalize archive directory");
        let archive =
            Arc::new(ReputationFinalizedArchive::try_open(root, bounds).expect("open archive"));
        let empty_adapter = ArchivedReputationFinalizedQueryV1::try_new(
            "reputation-archive:region-a",
            qualification(),
            Arc::clone(&archive),
        )
        .expect("construct empty archived query");
        assert_eq!(
            empty_adapter.qualification().expect("identity readiness"),
            qualification(),
            "an empty validated namespace may expose identity readiness"
        );
        assert!(
            empty_adapter
                .finalized_at_or_before(&ChainId::from(CHAIN_ID), u64::MAX)
                .is_err(),
            "data reads remain unavailable before genesis capture"
        );
        for rejected in [
            "test-adapter",
            "https://operator:secret@reputation.example",
            "https://reputation.example/query?token=secret",
            "https://reputation.example/query#fragment",
            "hsm://reputation/dummy/query",
        ] {
            assert!(
                ArchivedReputationFinalizedQueryV1::try_new(
                    rejected,
                    qualification(),
                    Arc::clone(&archive),
                )
                .is_err(),
                "{rejected:?} must fail before archive access"
            );
        }
        assert!(
            ArchivedReputationFinalizedQueryV1::try_new(
                "reputation-archive:region-a",
                ReputationRuntimeProviderQualificationV1::new(
                    REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1 + 1,
                    [0xD1; 32],
                ),
                Arc::clone(&archive),
            )
            .is_err()
        );
        assert!(
            ArchivedReputationFinalizedQueryV1::try_new(
                "reputation-archive:region-a",
                ReputationRuntimeProviderQualificationV1::new(
                    REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1,
                    [0; 32],
                ),
                archive,
            )
            .is_err()
        );
    }
}
