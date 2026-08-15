//! Restart-safe finalized reserve-event ingestion for transparency publication.
//!
//! The scanner consumes only exact-anchor immutable reserve pages, verifies
//! every referenced block against a fresh committed projection, records the
//! existing payload-free transparency adapter output, and advances its local
//! cursor only after the durable source index accepts the entry. Query access,
//! committed-state access, and source storage remain explicit injected seams.
use crate::{
    NodeHandle, TransparencyLedgerSourceEntry, decode_local_checkpoint_canonical,
    read_local_checkpoint_bounded,
    reputation::runtime::{
        REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1, ReputationExternalFailureV1,
        ReputationFinalizedAnchorV1, ReputationFinalizedQueryV1,
        ReputationRuntimeProviderQualificationV1,
    },
    reserve_finalized_event_source_entry, write_local_checkpoint_atomic_bounded,
};
use iroha_config::parameters::{
    actual::SorafsReserveTransparencyRuntime, validate_production_runtime_handle,
};
use iroha_data_model::{
    NetworkId,
    sorafs::reserve::{
        RESERVE_QUERY_MAX_ITEMS_V1, ReserveFinalizedCursorV1, ReserveFinalizedEventCursorV1,
        ReserveFinalizedEventPageV1,
    },
};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use std::{
    collections::BTreeSet,
    fmt,
    path::{Component, PathBuf},
    sync::Arc,
};
use thiserror::Error;
/// Version of the durable reserve-transparency scanner checkpoint.
pub const RESERVE_TRANSPARENCY_CHECKPOINT_VERSION_V1: u8 = 1;
/// Fixed checkpoint filename below the configured private state directory.
pub const RESERVE_TRANSPARENCY_CHECKPOINT_FILE_V1: &str = "reserve-transparency-scanner-v1.to";
const CHECKPOINT_DIGEST_DOMAIN_V1: &[u8] = b"sorafs-reserve-transparency-checkpoint-v1";
const CHECKPOINT_MAX_SEQUENCE_ELEMENTS_V1: usize = 1_024;
/// Failure returned by a fresh committed-state projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ReserveTransparencyCommittedProjectionErrorV1 {
    /// A fresh committed projection could not be opened.
    #[error("fresh committed projection is unavailable")]
    Unavailable,
    /// One expected height/hash is not on the current committed chain.
    #[error("finalized reserve cursor is not on the current committed chain")]
    ForkOrReorg,
}
/// Fresh committed-state verifier used by the scanner.
pub trait ReserveTransparencyCommittedProjectionV1: Send + Sync {
    /// Open one fresh committed projection, verify every supplied exact
    /// height/hash, and return the head from that same projection.
    ///
    /// # Errors
    ///
    /// Returns `Unavailable` when no fresh view can be opened and
    /// `ForkOrReorg` when any expected cursor is absent or substituted.
    fn verify_committed_anchors(
        &self,
        network_id: &NetworkId,
        expected: &[ReserveFinalizedCursorV1],
    ) -> Result<ReserveFinalizedCursorV1, ReserveTransparencyCommittedProjectionErrorV1>;
}
/// Opaque failure from the durable transparency source index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("durable transparency source index rejected the entry")]
pub struct ReserveTransparencySourceSinkErrorV1;
/// Durable idempotent sink for payload-free transparency source entries.
pub trait ReserveTransparencySourceSinkV1: Send + Sync {
    /// Record one canonical source entry.
    ///
    /// Exact replay must succeed and conflicting replay must fail.
    ///
    /// # Errors
    ///
    /// Returns an opaque error when validation, retention, or persistence
    /// prevents the record from becoming durable.
    fn record_source_entry(
        &self,
        entry: TransparencyLedgerSourceEntry,
    ) -> Result<(), ReserveTransparencySourceSinkErrorV1>;
}
impl ReserveTransparencySourceSinkV1 for NodeHandle {
    fn record_source_entry(
        &self,
        entry: TransparencyLedgerSourceEntry,
    ) -> Result<(), ReserveTransparencySourceSinkErrorV1> {
        self.record_transparency_ledger_source_entry(entry)
            .map_err(|_| ReserveTransparencySourceSinkErrorV1)
    }
}
/// Minimal exact-anchor query surface required by the scanner.
pub trait ReserveTransparencyFinalizedQueryV1: Send + Sync {
    /// Stable credential-free provider handle.
    fn handle(&self) -> &str;
    /// Observe the active provider qualification.
    ///
    /// # Errors
    ///
    /// Returns a payload-free failure when the provider cannot prove its
    /// configured revision and public-policy digest.
    fn qualification(
        &self,
    ) -> Result<ReputationRuntimeProviderQualificationV1, ReputationExternalFailureV1>;
    /// Select the exact immutable finalized anchor at or below the bound.
    ///
    /// # Errors
    ///
    /// Returns a payload-free failure when the archive cannot serve the view.
    fn finalized_at_or_before(
        &self,
        network_id: &NetworkId,
        maximum_height: u64,
    ) -> Result<ReputationFinalizedAnchorV1, ReputationExternalFailureV1>;
    /// Fetch one bounded reserve-event page at the exact supplied anchor.
    ///
    /// # Errors
    ///
    /// Returns a payload-free failure when the immutable page is unavailable.
    fn reserve_page(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after: Option<ReserveFinalizedEventCursorV1>,
        limit: u32,
    ) -> Result<ReserveFinalizedEventPageV1, ReputationExternalFailureV1>;
}
/// Adapter narrowing the shared reputation finalized-query provider to the
/// exact reserve-event methods required by the transparency scanner.
#[derive(Clone)]
pub struct ReputationReserveTransparencyQueryAdapterV1 {
    inner: Arc<dyn ReputationFinalizedQueryV1>,
}
impl ReputationReserveTransparencyQueryAdapterV1 {
    /// Wrap a qualified immutable finalized-query provider.
    #[must_use]
    pub fn new(inner: Arc<dyn ReputationFinalizedQueryV1>) -> Self {
        Self { inner }
    }
}
impl fmt::Debug for ReputationReserveTransparencyQueryAdapterV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReputationReserveTransparencyQueryAdapterV1")
            .field("handle", &self.inner.handle())
            .finish_non_exhaustive()
    }
}
impl ReserveTransparencyFinalizedQueryV1 for ReputationReserveTransparencyQueryAdapterV1 {
    fn handle(&self) -> &str {
        self.inner.handle()
    }
    fn qualification(
        &self,
    ) -> Result<ReputationRuntimeProviderQualificationV1, ReputationExternalFailureV1> {
        self.inner.qualification()
    }
    fn finalized_at_or_before(
        &self,
        network_id: &NetworkId,
        maximum_height: u64,
    ) -> Result<ReputationFinalizedAnchorV1, ReputationExternalFailureV1> {
        self.inner
            .finalized_at_or_before(network_id, maximum_height)
    }
    fn reserve_page(
        &self,
        anchor: &ReputationFinalizedAnchorV1,
        after: Option<ReserveFinalizedEventCursorV1>,
        limit: u32,
    ) -> Result<ReserveFinalizedEventPageV1, ReputationExternalFailureV1> {
        self.inner.reserve_page(anchor, after, limit)
    }
}
/// Scanner failure class used by supervision to distinguish bounded retry
/// from fail-closed shutdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ReserveTransparencyScannerErrorV1 {
    /// Configuration or constructor input is invalid.
    #[error("reserve transparency scanner policy is invalid")]
    InvalidPolicy,
    /// The durable local checkpoint is absent from its expected lineage,
    /// malformed, unsafe, corrupt, or could not be persisted.
    #[error("reserve transparency scanner checkpoint failed validation or persistence")]
    Checkpoint,
    /// The finalized query handle, revision, policy digest, or readiness
    /// changed.
    #[error("reserve transparency finalized-query binding changed")]
    QueryBinding,
    /// The exact immutable query view is temporarily unavailable.
    #[error("reserve transparency finalized-query view is unavailable")]
    QueryUnavailable,
    /// A fresh committed projection is temporarily unavailable.
    #[error("reserve transparency committed projection is unavailable")]
    ProjectionUnavailable,
    /// The immutable archive has not yet captured the fresh committed head.
    #[error("reserve transparency finalized archive is behind committed head")]
    ArchiveLag,
    /// A persisted, selected, or event cursor is not on the current committed
    /// chain.
    #[error("reserve transparency scanner detected a fork or reorganization")]
    ForkOrReorg,
    /// The immutable query returned a malformed, unbounded, or cursor-divergent
    /// page.
    #[error("reserve transparency finalized query returned an invalid page")]
    InvalidPage,
    /// The existing reserve-to-transparency adapter rejected the committed
    /// event.
    #[error("reserve transparency source adapter rejected a finalized event")]
    SourceAdapter,
    /// The durable source index rejected or could not persist an entry.
    #[error("reserve transparency source entry could not be made durable")]
    SourceSink,
}
impl ReserveTransparencyScannerErrorV1 {
    /// Return whether supervision may retry this failure with bounded backoff.
    #[must_use]
    pub const fn is_retryable(self) -> bool {
        matches!(
            self,
            Self::QueryUnavailable | Self::ProjectionUnavailable | Self::ArchiveLag
        )
    }
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ReserveTransparencyCheckpointPayloadV1 {
    version: u8,
    generation: u64,
    network_id: NetworkId,
    query_handle: String,
    query_revision: u64,
    query_policy_digest: [u8; 32],
    finalized_anchor: ReserveFinalizedCursorV1,
    after: Option<ReserveFinalizedEventCursorV1>,
}
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct ReserveTransparencyCheckpointV1 {
    payload: ReserveTransparencyCheckpointPayloadV1,
    digest: [u8; 32],
}
impl ReserveTransparencyCheckpointV1 {
    fn try_new(
        generation: u64,
        network_id: NetworkId,
        query_handle: String,
        query_qualification: ReputationRuntimeProviderQualificationV1,
        finalized_anchor: ReserveFinalizedCursorV1,
        after: Option<ReserveFinalizedEventCursorV1>,
    ) -> Result<Self, ReserveTransparencyScannerErrorV1> {
        let payload = ReserveTransparencyCheckpointPayloadV1 {
            version: RESERVE_TRANSPARENCY_CHECKPOINT_VERSION_V1,
            generation,
            network_id,
            query_handle,
            query_revision: query_qualification.revision(),
            query_policy_digest: query_qualification.policy_digest(),
            finalized_anchor,
            after,
        };
        let digest = checkpoint_payload_digest(&payload)?;
        Ok(Self { payload, digest })
    }
    fn validate_for(
        &self,
        network_id: &NetworkId,
        query_handle: &str,
        query_qualification: ReputationRuntimeProviderQualificationV1,
    ) -> Result<(), ReserveTransparencyScannerErrorV1> {
        let anchor = self.payload.finalized_anchor;
        let cursor_valid = self.payload.after.is_none_or(|cursor| {
            cursor.sequence != 0
                && cursor.block_height != 0
                && cursor.block_hash != [0; 32]
                && cursor.block_height <= anchor.height
                && (cursor.block_height != anchor.height || cursor.block_hash == anchor.block_hash)
        });
        if self.payload.version != RESERVE_TRANSPARENCY_CHECKPOINT_VERSION_V1
            || self.payload.generation == 0
            || &self.payload.network_id != network_id
            || self.payload.query_handle != query_handle
            || self.payload.query_revision != query_qualification.revision()
            || self.payload.query_policy_digest != query_qualification.policy_digest()
            || anchor.height == 0
            || anchor.block_hash == [0; 32]
            || !cursor_valid
            || self.digest == [0; 32]
            || checkpoint_payload_digest(&self.payload)? != self.digest
        {
            return Err(ReserveTransparencyScannerErrorV1::Checkpoint);
        }
        Ok(())
    }
}
fn checkpoint_payload_digest(
    payload: &ReserveTransparencyCheckpointPayloadV1,
) -> Result<[u8; 32], ReserveTransparencyScannerErrorV1> {
    let bytes =
        norito::to_bytes(payload).map_err(|_| ReserveTransparencyScannerErrorV1::Checkpoint)?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(CHECKPOINT_DIGEST_DOMAIN_V1);
    let byte_len =
        u64::try_from(bytes.len()).map_err(|_| ReserveTransparencyScannerErrorV1::Checkpoint)?;
    hasher.update(&byte_len.to_le_bytes());
    hasher.update(&bytes);
    Ok(*hasher.finalize().as_bytes())
}
/// Payload-free result of one bounded scanner tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReserveTransparencyTickOutcomeV1 {
    /// Immutable pages consumed during this tick.
    pub pages: u32,
    /// Source entries durably accepted during this tick, including exact
    /// idempotent replay.
    pub events: u32,
    /// Whether the selected exact anchor had no remaining continuation.
    pub caught_up: bool,
    /// Exact immutable anchor selected for this tick.
    pub finalized_anchor: ReserveFinalizedCursorV1,
}
/// Bounded restart-safe scanner for finalized reserve transparency entries.
pub struct ReserveTransparencyScannerV1 {
    network_id: NetworkId,
    checkpoint_path: PathBuf,
    checkpoint_max_bytes: u64,
    query_handle: String,
    query_qualification: ReputationRuntimeProviderQualificationV1,
    page_items: u32,
    max_pages_per_tick: u32,
    query: Arc<dyn ReserveTransparencyFinalizedQueryV1>,
    projection: Arc<dyn ReserveTransparencyCommittedProjectionV1>,
    sink: Arc<dyn ReserveTransparencySourceSinkV1>,
    checkpoint: Option<ReserveTransparencyCheckpointV1>,
}
impl fmt::Debug for ReserveTransparencyScannerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReserveTransparencyScannerV1")
            .field("network_id", &self.network_id)
            .field("checkpoint_path", &self.checkpoint_path)
            .field("query_handle", &self.query_handle)
            .field("page_items", &self.page_items)
            .field("max_pages_per_tick", &self.max_pages_per_tick)
            .field("checkpoint", &self.checkpoint)
            .finish_non_exhaustive()
    }
}
impl ReserveTransparencyScannerV1 {
    /// Validate dependencies and restore the canonical checkpoint.
    ///
    /// # Errors
    ///
    /// Fails before scanning when the policy, exact query binding, checkpoint
    /// path, checkpoint bytes, or checkpoint lineage is invalid.
    pub fn try_new(
        config: &SorafsReserveTransparencyRuntime,
        network_id: NetworkId,
        query_qualification: ReputationRuntimeProviderQualificationV1,
        query: Arc<dyn ReserveTransparencyFinalizedQueryV1>,
        projection: Arc<dyn ReserveTransparencyCommittedProjectionV1>,
        sink: Arc<dyn ReserveTransparencySourceSinkV1>,
    ) -> Result<Self, ReserveTransparencyScannerErrorV1> {
        if network_id.as_bytes()[31] & 1 != 1
            || !config.state_dir.is_absolute()
            || config.state_dir.file_name().is_none()
            || config
                .state_dir
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
            || validate_production_runtime_handle(&config.finalized_query_handle).is_err()
            || query_qualification.revision()
                != REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1
            || query_qualification.policy_digest() == [0; 32]
            || config.page_items == 0
            || config.page_items > RESERVE_QUERY_MAX_ITEMS_V1
            || config.max_pages_per_tick == 0
            || config.checkpoint_max_bytes.0 == 0
        {
            return Err(ReserveTransparencyScannerErrorV1::InvalidPolicy);
        }
        let checkpoint_path = config
            .state_dir
            .join(RESERVE_TRANSPARENCY_CHECKPOINT_FILE_V1);
        let query_handle = config.finalized_query_handle.clone();
        assert_query_binding(query.as_ref(), &query_handle, query_qualification)?;
        let checkpoint =
            read_local_checkpoint_bounded(&checkpoint_path, config.checkpoint_max_bytes.0)
                .map_err(|_| ReserveTransparencyScannerErrorV1::Checkpoint)?
                .map(|bytes| {
                    let checkpoint: ReserveTransparencyCheckpointV1 =
                        decode_local_checkpoint_canonical(
                            &bytes,
                            config.checkpoint_max_bytes.0,
                            CHECKPOINT_MAX_SEQUENCE_ELEMENTS_V1,
                        )
                        .map_err(|_| ReserveTransparencyScannerErrorV1::Checkpoint)?;
                    checkpoint.validate_for(&network_id, &query_handle, query_qualification)?;
                    Ok(checkpoint)
                })
                .transpose()?;
        Ok(Self {
            network_id,
            checkpoint_path,
            checkpoint_max_bytes: config.checkpoint_max_bytes.0,
            query_handle,
            query_qualification,
            page_items: config.page_items,
            max_pages_per_tick: config.max_pages_per_tick,
            query,
            projection,
            sink,
            checkpoint,
        })
    }
    /// Consume at most the configured page budget from one exact immutable
    /// committed anchor.
    ///
    /// The source entry becomes durable before the cursor advances. A crash in
    /// between replays the exact entry into the idempotent source index.
    ///
    /// # Errors
    ///
    /// Returns a retryable error only for query/projection unavailability or
    /// normal archive lag. Binding drift, fork/reorg evidence, malformed pages,
    /// source rejection, and checkpoint failure are fail-closed.
    pub fn tick(
        &mut self,
    ) -> Result<ReserveTransparencyTickOutcomeV1, ReserveTransparencyScannerErrorV1> {
        assert_query_binding(
            self.query.as_ref(),
            &self.query_handle,
            self.query_qualification,
        )?;
        let prior_anchors = self.checkpoint_anchors();
        let committed_head = self.verify_projection(&prior_anchors)?;
        validate_anchor(committed_head)?;
        let selected = self
            .query
            .finalized_at_or_before(&self.network_id, committed_head.height)
            .map_err(|_| ReserveTransparencyScannerErrorV1::QueryUnavailable)?;
        assert_query_binding(
            self.query.as_ref(),
            &self.query_handle,
            self.query_qualification,
        )?;
        validate_selected_anchor(&selected, &self.network_id)?;
        let selected_cursor = ReserveFinalizedCursorV1 {
            height: selected.identity.height,
            block_hash: selected.identity.block_hash,
        };
        if selected_cursor.height < committed_head.height {
            return Err(ReserveTransparencyScannerErrorV1::ArchiveLag);
        }
        if selected_cursor != committed_head {
            return Err(ReserveTransparencyScannerErrorV1::ForkOrReorg);
        }
        let mut after = self
            .checkpoint
            .as_ref()
            .and_then(|checkpoint| checkpoint.payload.after);
        let mut pages = 0_u32;
        let mut events = 0_u32;
        let mut caught_up = false;
        while pages < self.max_pages_per_tick {
            assert_query_binding(
                self.query.as_ref(),
                &self.query_handle,
                self.query_qualification,
            )?;
            let page = self
                .query
                .reserve_page(&selected, after, self.page_items)
                .map_err(|_| ReserveTransparencyScannerErrorV1::QueryUnavailable)?;
            assert_query_binding(
                self.query.as_ref(),
                &self.query_handle,
                self.query_qualification,
            )?;
            let page_anchors = validate_page(&page, selected_cursor, after, self.page_items)?;
            let mut expected = prior_anchors.clone();
            expected.push(selected_cursor);
            expected.extend(page_anchors);
            self.verify_projection(&deduplicate_anchors(expected))?;
            pages = pages
                .checked_add(1)
                .ok_or(ReserveTransparencyScannerErrorV1::InvalidPage)?;
            if page.events.is_empty() {
                self.persist_progress(selected_cursor, after)?;
            } else {
                for event in &page.events {
                    let entry = reserve_finalized_event_source_entry(event)
                        .map_err(|_| ReserveTransparencyScannerErrorV1::SourceAdapter)?;
                    self.sink
                        .record_source_entry(entry)
                        .map_err(|_| ReserveTransparencyScannerErrorV1::SourceSink)?;
                    after = Some(event.cursor());
                    self.persist_progress(selected_cursor, after)?;
                    events = events
                        .checked_add(1)
                        .ok_or(ReserveTransparencyScannerErrorV1::InvalidPage)?;
                }
            }
            caught_up = !page.has_more;
            if caught_up {
                break;
            }
            after = page.next_after;
        }
        Ok(ReserveTransparencyTickOutcomeV1 {
            pages,
            events,
            caught_up,
            finalized_anchor: selected_cursor,
        })
    }
    fn checkpoint_anchors(&self) -> Vec<ReserveFinalizedCursorV1> {
        let Some(checkpoint) = self.checkpoint.as_ref() else {
            return Vec::new();
        };
        let mut anchors = vec![checkpoint.payload.finalized_anchor];
        if let Some(after) = checkpoint.payload.after {
            anchors.push(ReserveFinalizedCursorV1 {
                height: after.block_height,
                block_hash: after.block_hash,
            });
        }
        deduplicate_anchors(anchors)
    }
    fn verify_projection(
        &self,
        expected: &[ReserveFinalizedCursorV1],
    ) -> Result<ReserveFinalizedCursorV1, ReserveTransparencyScannerErrorV1> {
        self.projection
            .verify_committed_anchors(&self.network_id, expected)
            .map_err(|error| match error {
                ReserveTransparencyCommittedProjectionErrorV1::Unavailable => {
                    ReserveTransparencyScannerErrorV1::ProjectionUnavailable
                }
                ReserveTransparencyCommittedProjectionErrorV1::ForkOrReorg => {
                    ReserveTransparencyScannerErrorV1::ForkOrReorg
                }
            })
    }
    fn persist_progress(
        &mut self,
        finalized_anchor: ReserveFinalizedCursorV1,
        after: Option<ReserveFinalizedEventCursorV1>,
    ) -> Result<(), ReserveTransparencyScannerErrorV1> {
        if self.checkpoint.as_ref().is_some_and(|checkpoint| {
            checkpoint.payload.finalized_anchor == finalized_anchor
                && checkpoint.payload.after == after
        }) {
            return Ok(());
        }
        let generation = self.checkpoint.as_ref().map_or(1, |checkpoint| {
            checkpoint.payload.generation.checked_add(1).unwrap_or(0)
        });
        if generation == 0 {
            return Err(ReserveTransparencyScannerErrorV1::Checkpoint);
        }
        let next = ReserveTransparencyCheckpointV1::try_new(
            generation,
            self.network_id,
            self.query_handle.clone(),
            self.query_qualification,
            finalized_anchor,
            after,
        )?;
        next.validate_for(
            &self.network_id,
            &self.query_handle,
            self.query_qualification,
        )?;
        let bytes =
            norito::to_bytes(&next).map_err(|_| ReserveTransparencyScannerErrorV1::Checkpoint)?;
        write_local_checkpoint_atomic_bounded(
            &self.checkpoint_path,
            &bytes,
            self.checkpoint_max_bytes,
        )
        .map_err(|_| ReserveTransparencyScannerErrorV1::Checkpoint)?;
        self.checkpoint = Some(next);
        Ok(())
    }
}
fn assert_query_binding(
    query: &dyn ReserveTransparencyFinalizedQueryV1,
    expected_handle: &str,
    expected_qualification: ReputationRuntimeProviderQualificationV1,
) -> Result<(), ReserveTransparencyScannerErrorV1> {
    if query.handle() != expected_handle
        || validate_production_runtime_handle(query.handle()).is_err()
        || query.qualification().ok() != Some(expected_qualification)
        || query.handle() != expected_handle
    {
        return Err(ReserveTransparencyScannerErrorV1::QueryBinding);
    }
    Ok(())
}
fn validate_anchor(
    cursor: ReserveFinalizedCursorV1,
) -> Result<(), ReserveTransparencyScannerErrorV1> {
    if cursor.height == 0 || cursor.block_hash == [0; 32] {
        return Err(ReserveTransparencyScannerErrorV1::ProjectionUnavailable);
    }
    Ok(())
}
fn validate_selected_anchor(
    anchor: &ReputationFinalizedAnchorV1,
    expected_network_id: &NetworkId,
) -> Result<(), ReserveTransparencyScannerErrorV1> {
    if &anchor.network_id != expected_network_id
        || anchor.identity.height == 0
        || anchor.identity.block_hash == [0; 32]
        || anchor.finalized_at_unix_ms == 0
        || anchor.finalized_at_unix_ms == u64::MAX
    {
        return Err(ReserveTransparencyScannerErrorV1::InvalidPage);
    }
    Ok(())
}
fn validate_page(
    page: &ReserveFinalizedEventPageV1,
    selected: ReserveFinalizedCursorV1,
    after: Option<ReserveFinalizedEventCursorV1>,
    limit: u32,
) -> Result<Vec<ReserveFinalizedCursorV1>, ReserveTransparencyScannerErrorV1> {
    if page.finalized_cursor != selected
        || page.events.len() > usize::try_from(limit).unwrap_or(usize::MAX)
        || (page.has_more && page.events.is_empty())
        || (page.has_more && page.next_after != page.events.last().map(|event| event.cursor()))
        || (!page.has_more && page.next_after.is_some())
    {
        return Err(ReserveTransparencyScannerErrorV1::InvalidPage);
    }
    let mut previous = after;
    let mut anchors = Vec::with_capacity(page.events.len());
    for event in &page.events {
        let cursor = event.cursor();
        let same_block_is_ordered = previous.is_none_or(|prior| {
            cursor.block_height != prior.block_height
                || (cursor.block_hash == prior.block_hash && cursor.event_index > prior.event_index)
        });
        if cursor.sequence == 0
            || cursor.block_height == 0
            || cursor.block_hash == [0; 32]
            || cursor.block_height > selected.height
            || (cursor.block_height == selected.height && cursor.block_hash != selected.block_hash)
            || previous.is_some_and(|prior| cursor <= prior)
            || previous.is_some_and(|prior| cursor.block_height < prior.block_height)
            || !same_block_is_ordered
        {
            return Err(ReserveTransparencyScannerErrorV1::InvalidPage);
        }
        anchors.push(ReserveFinalizedCursorV1 {
            height: cursor.block_height,
            block_hash: cursor.block_hash,
        });
        previous = Some(cursor);
    }
    Ok(deduplicate_anchors(anchors))
}
fn deduplicate_anchors(anchors: Vec<ReserveFinalizedCursorV1>) -> Vec<ReserveFinalizedCursorV1> {
    anchors
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::reputation::ReputationFinalizedIdentityV1;
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::{
        account::{AccountId, ParsedAccountId},
        events::data::sorafs::{SorafsReserveLedgerEvent, SorafsReserveLedgerEventKind},
        sorafs::{
            capacity::ProviderId,
            reserve::{ReserveFinalizedEventV1, ReserveLifecycleStage},
        },
    };
    use std::{
        collections::BTreeMap,
        sync::{
            Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };
    const QUERY_HANDLE: &str = "ledger.finalized.primary";
    const QUERY_POLICY_DIGEST: [u8; 32] = [0xA5; 32];
    fn test_network_id() -> NetworkId {
        NetworkId::from_genesis_hash(
            HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(Hash::new(
                b"reserve-transparency-test-genesis",
            )),
        )
    }
    #[derive(Debug)]
    struct MockQuery {
        network_id: NetworkId,
        anchor: ReserveFinalizedCursorV1,
        events: Vec<ReserveFinalizedEventV1>,
    }
    impl ReserveTransparencyFinalizedQueryV1 for MockQuery {
        fn handle(&self) -> &str {
            QUERY_HANDLE
        }
        fn qualification(
            &self,
        ) -> Result<ReputationRuntimeProviderQualificationV1, ReputationExternalFailureV1> {
            Ok(query_qualification())
        }
        fn finalized_at_or_before(
            &self,
            network_id: &NetworkId,
            maximum_height: u64,
        ) -> Result<ReputationFinalizedAnchorV1, ReputationExternalFailureV1> {
            assert_eq!(network_id, &self.network_id);
            assert!(maximum_height >= self.anchor.height);
            Ok(ReputationFinalizedAnchorV1 {
                network_id: self.network_id,
                identity: ReputationFinalizedIdentityV1 {
                    height: self.anchor.height,
                    block_hash: self.anchor.block_hash,
                },
                finalized_at_unix_ms: 1_800_000_000_000,
            })
        }
        fn reserve_page(
            &self,
            anchor: &ReputationFinalizedAnchorV1,
            after: Option<ReserveFinalizedEventCursorV1>,
            limit: u32,
        ) -> Result<ReserveFinalizedEventPageV1, ReputationExternalFailureV1> {
            assert_eq!(anchor.identity.height, self.anchor.height);
            assert_eq!(anchor.identity.block_hash, self.anchor.block_hash);
            let remaining = self
                .events
                .iter()
                .filter(|event| after.is_none_or(|cursor| event.cursor() > cursor))
                .cloned()
                .collect::<Vec<_>>();
            let take = usize::try_from(limit).expect("test page limit fits usize");
            let events = remaining.iter().take(take).cloned().collect::<Vec<_>>();
            let has_more = remaining.len() > events.len();
            Ok(ReserveFinalizedEventPageV1 {
                finalized_cursor: self.anchor,
                next_after: has_more.then(|| {
                    events
                        .last()
                        .expect("continued test page is nonempty")
                        .cursor()
                }),
                events,
                has_more,
            })
        }
    }
    #[derive(Debug)]
    struct MockProjection {
        network_id: NetworkId,
        hashes: Mutex<Vec<[u8; 32]>>,
    }
    impl MockProjection {
        fn replace_hash(&self, height: u64, hash: [u8; 32]) {
            let mut hashes = self.hashes.lock().expect("test projection lock");
            hashes[usize::try_from(height - 1).expect("test height fits usize")] = hash;
        }
    }
    impl ReserveTransparencyCommittedProjectionV1 for MockProjection {
        fn verify_committed_anchors(
            &self,
            network_id: &NetworkId,
            expected: &[ReserveFinalizedCursorV1],
        ) -> Result<ReserveFinalizedCursorV1, ReserveTransparencyCommittedProjectionErrorV1>
        {
            if network_id != &self.network_id {
                return Err(ReserveTransparencyCommittedProjectionErrorV1::ForkOrReorg);
            }
            let hashes = self
                .hashes
                .lock()
                .map_err(|_| ReserveTransparencyCommittedProjectionErrorV1::Unavailable)?;
            if expected.iter().any(|cursor| {
                usize::try_from(cursor.height.saturating_sub(1))
                    .ok()
                    .and_then(|index| hashes.get(index))
                    .is_none_or(|hash| *hash != cursor.block_hash)
            }) {
                return Err(ReserveTransparencyCommittedProjectionErrorV1::ForkOrReorg);
            }
            let block_hash = *hashes
                .last()
                .ok_or(ReserveTransparencyCommittedProjectionErrorV1::Unavailable)?;
            Ok(ReserveFinalizedCursorV1 {
                height: u64::try_from(hashes.len())
                    .map_err(|_| ReserveTransparencyCommittedProjectionErrorV1::Unavailable)?,
                block_hash,
            })
        }
    }
    #[derive(Debug, Default)]
    struct MockSink {
        attempts: AtomicUsize,
        entries: Mutex<BTreeMap<String, TransparencyLedgerSourceEntry>>,
    }
    impl ReserveTransparencySourceSinkV1 for MockSink {
        fn record_source_entry(
            &self,
            entry: TransparencyLedgerSourceEntry,
        ) -> Result<(), ReserveTransparencySourceSinkErrorV1> {
            self.attempts.fetch_add(1, Ordering::Relaxed);
            let mut entries = self
                .entries
                .lock()
                .map_err(|_| ReserveTransparencySourceSinkErrorV1)?;
            if let Some(retained) = entries.get(&entry.event_id) {
                return (retained == &entry)
                    .then_some(())
                    .ok_or(ReserveTransparencySourceSinkErrorV1);
            }
            entries.insert(entry.event_id.clone(), entry);
            Ok(())
        }
    }
    fn query_qualification() -> ReputationRuntimeProviderQualificationV1 {
        ReputationRuntimeProviderQualificationV1::new(
            REPUTATION_RUNTIME_PROVIDER_QUALIFICATION_REVISION_V1,
            QUERY_POLICY_DIGEST,
        )
    }
    fn test_account() -> AccountId {
        AccountId::parse_encoded("sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE")
            .map(ParsedAccountId::into_account_id)
            .expect("test account id")
    }
    fn finalized_event(
        sequence: u64,
        block_height: u64,
        block_hash: [u8; 32],
    ) -> ReserveFinalizedEventV1 {
        ReserveFinalizedEventV1 {
            sequence,
            block_height,
            block_hash,
            event_index: 0,
            event: SorafsReserveLedgerEvent {
                kind: SorafsReserveLedgerEventKind::MovementApproved,
                provider_id: Some(ProviderId::new([0xB2; 32])),
                operation_id: Some([u8::try_from(sequence).expect("test sequence fits u8"); 32]),
                policy_digest: [0xD4; 32],
                provider_revision: sequence,
                resulting_lifecycle_stage: Some(ReserveLifecycleStage::Active),
                authority: test_account(),
                occurred_at_unix_ms: 1_800_000_123_000 + sequence,
            },
        }
    }
    fn scanner_config(state_dir: PathBuf) -> SorafsReserveTransparencyRuntime {
        SorafsReserveTransparencyRuntime {
            state_dir,
            finalized_query_handle: QUERY_HANDLE.to_owned(),
            poll_interval: Duration::from_millis(100),
            retry_max_interval: Duration::from_secs(1),
            page_items: 1,
            max_pages_per_tick: 1,
            checkpoint_max_bytes:
                iroha_config::parameters::defaults::sorafs::storage::reserve_transparency_runtime::CHECKPOINT_MAX_BYTES,
        }
    }
    type TestDependencies = (Arc<MockQuery>, Arc<MockProjection>, Arc<MockSink>);
    fn test_dependencies() -> TestDependencies {
        let network_id = test_network_id();
        let hashes = vec![[0x11; 32], [0x22; 32], [0x33; 32]];
        let anchor = ReserveFinalizedCursorV1 {
            height: 3,
            block_hash: hashes[2],
        };
        (
            Arc::new(MockQuery {
                network_id,
                anchor,
                events: vec![
                    finalized_event(1, 2, hashes[1]),
                    finalized_event(2, 3, hashes[2]),
                ],
            }),
            Arc::new(MockProjection {
                network_id,
                hashes: Mutex::new(hashes),
            }),
            Arc::new(MockSink::default()),
        )
    }
    fn scanner(
        config: &SorafsReserveTransparencyRuntime,
        query: Arc<MockQuery>,
        projection: Arc<MockProjection>,
        sink: Arc<MockSink>,
    ) -> ReserveTransparencyScannerV1 {
        let query: Arc<dyn ReserveTransparencyFinalizedQueryV1> = query;
        let projection: Arc<dyn ReserveTransparencyCommittedProjectionV1> = projection;
        let sink: Arc<dyn ReserveTransparencySourceSinkV1> = sink;
        ReserveTransparencyScannerV1::try_new(
            config,
            test_network_id(),
            query_qualification(),
            query,
            projection,
            sink,
        )
        .expect("construct test scanner")
    }
    #[test]
    fn restart_resumes_after_last_durable_event_without_duplication() {
        let temp = tempfile::tempdir().expect("temporary scanner root");
        let config = scanner_config(temp.path().join("scanner"));
        let (query, projection, sink) = test_dependencies();
        let mut first = scanner(
            &config,
            Arc::clone(&query),
            Arc::clone(&projection),
            Arc::clone(&sink),
        );
        let first_outcome = first.tick().expect("first bounded page");
        assert_eq!(first_outcome.events, 1);
        assert!(!first_outcome.caught_up);
        drop(first);
        let mut restarted = scanner(
            &config,
            Arc::clone(&query),
            Arc::clone(&projection),
            Arc::clone(&sink),
        );
        let second_outcome = restarted.tick().expect("resume after checkpoint cursor");
        assert_eq!(second_outcome.events, 1);
        assert!(second_outcome.caught_up);
        drop(restarted);
        let mut caught_up = scanner(&config, query, projection, Arc::clone(&sink));
        let replay_outcome = caught_up.tick().expect("restart at caught-up cursor");
        assert_eq!(replay_outcome.events, 0);
        assert!(replay_outcome.caught_up);
        assert_eq!(sink.attempts.load(Ordering::Relaxed), 2);
        assert_eq!(sink.entries.lock().expect("test sink lock").len(), 2);
    }
    #[test]
    fn durable_source_replay_before_cursor_write_is_idempotent() {
        let temp = tempfile::tempdir().expect("temporary scanner root");
        let config = scanner_config(temp.path().join("scanner"));
        let (query, projection, sink) = test_dependencies();
        let replay_entry = reserve_finalized_event_source_entry(&query.events[0])
            .expect("derive pre-crash source entry");
        sink.record_source_entry(replay_entry)
            .expect("simulate durable source write before crash");
        let mut scanner = scanner(&config, query, projection, Arc::clone(&sink));
        let outcome = scanner.tick().expect("replay exact source after restart");
        assert_eq!(outcome.events, 1);
        assert_eq!(sink.attempts.load(Ordering::Relaxed), 2);
        assert_eq!(sink.entries.lock().expect("test sink lock").len(), 1);
    }
    #[test]
    fn restart_fails_closed_when_persisted_anchor_left_committed_chain() {
        let temp = tempfile::tempdir().expect("temporary scanner root");
        let config = scanner_config(temp.path().join("scanner"));
        let (query, projection, sink) = test_dependencies();
        let mut first = scanner(
            &config,
            Arc::clone(&query),
            Arc::clone(&projection),
            Arc::clone(&sink),
        );
        first.tick().expect("persist initial exact cursor");
        drop(first);
        projection.replace_hash(3, [0xF3; 32]);
        let mut restarted = scanner(&config, query, projection, sink);
        assert_eq!(
            restarted.tick(),
            Err(ReserveTransparencyScannerErrorV1::ForkOrReorg)
        );
    }
}
