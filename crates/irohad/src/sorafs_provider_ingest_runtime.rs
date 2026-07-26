//! Supervised production wiring for finalized-ledger SoraFS provider ingest.
//!
//! Authoritative assignments are copied from one coherent committed
//! [`State::query_view`] into a bounded owned snapshot. Runtime-only source
//! authentication and governed HSM/KMS signing remain deployment-injected
//! boundaries: config contains only identity-pinned opaque handles.

use std::{
    fmt,
    io::{self, Read},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr, bail};
use iroha_config::parameters::actual::SorafsProviderIngestRuntime;
use iroha_core::{
    queue::{Error as QueueError, Queue},
    state::{State, StateReadOnly as _, WorldReadOnly as _, WorldStateSnapshot as _},
    tx::AcceptedTransaction,
};
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    block::BlockHeader,
    isi::sorafs::CompleteReplicationOrder,
    sorafs::{
        capacity::ProviderId,
        pin_registry::{
            PinManifestFinalizedCursorV1, PinManifestFinalizedRecordV1, PinStatus,
            ReplicationOrderId, ReplicationOrderStatus,
        },
    },
    transaction::{
        FeePaymentIntent, SignedTransaction, TransactionBuilder, TransactionEntrypoint,
        TransactionPayload,
    },
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use mv::storage::StorageReadOnly as _;
use norito::{core::DecodeLimits, decode_from_bytes_with_limits};
use rand::{rand_core::TryRngCore as _, rngs::OsRng};
use sorafs_car::{CarBuildPlan, compute_chunk_plan_digest_sha3};
use sorafs_manifest::{
    ManifestV1,
    capacity::{MAX_CAPACITY_METADATA_VALUE_BYTES, ReplicationOrderV1},
};
use sorafs_node::{
    FinalizedProviderIngestAuthorizationV1, NodeHandle, NodeStorageError,
    ProviderIngestAuthenticatedSourceFetchV1, ProviderIngestClaimOwnerV1,
    ProviderIngestCompletionPayloadBuilderV1, ProviderIngestCompletionPayloadErrorV1,
    ProviderIngestCompletionPayloadRequestV1, ProviderIngestCompletionSignerErrorV1,
    ProviderIngestCompletionSignerResolverErrorV1, ProviderIngestCompletionSignerResolverV1,
    ProviderIngestCompletionSignerV1, ProviderIngestFinalizedAssignmentPageV1,
    ProviderIngestFinalizedAssignmentV1, ProviderIngestFinalizedCursorV1,
    ProviderIngestFinalizedLedgerErrorV1, ProviderIngestFinalizedLedgerV1, ProviderIngestFutureV1,
    ProviderIngestIngressDispositionV1, ProviderIngestIngressPrepareErrorV1,
    ProviderIngestLocalStorageErrorV1, ProviderIngestLocalStorageV1, ProviderIngestRuntimePolicyV1,
    ProviderIngestSourceFetchErrorV1, ProviderIngestSourceRequestV1, ProviderIngestSystemClockV1,
    ProviderIngestTickOutcomeV1, ProviderIngestTransactionIngressV1,
    ProviderIngestTransactionObservationV1, store::StorageError,
};

const SHUTDOWN_WAIT_FLOOR: Duration = Duration::from_secs(2);
const READINESS_STALE_TICK_MULTIPLIER_V1: u32 = 3;
const REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1: usize = 256 * 1024;
const SNAPSHOT_ROW_STRUCTURAL_OVERHEAD_BYTES_V1: usize = 512;
const SIGNED_TRANSACTION_ENVELOPE_RESERVE_BYTES_V1: u64 = 4 * 1024;
const REPLICATION_ORDER_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    MAX_CAPACITY_METADATA_VALUE_BYTES,
    REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1,
    131_072,
    REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1 * 4,
    32,
);

/// Exact verified source material passed directly into local SoraFS storage.
///
/// The reader may stream from a bounded authenticated transport or a
/// deployment-owned temporary object. It must already have passed the source
/// boundary's manifest, plan, length, digest, PoR-root, and governed-advert
/// checks, and every underlying read must carry a hard transport deadline no
/// longer than the configured source-operation deadline. URLs, grants, bearer
/// tokens, and credentials are intentionally absent.
pub struct VerifiedProviderIngestPayloadV1 {
    /// Canonical manifest returned by the authenticated source.
    pub manifest: ManifestV1,
    /// Exact CAR build plan bound to `manifest`.
    pub plan: CarBuildPlan,
    /// Verified payload stream consumed once by local storage.
    pub reader: Box<dyn Read + Send>,
}

impl fmt::Debug for VerifiedProviderIngestPayloadV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedProviderIngestPayloadV1")
            .field("manifest_digest", &self.manifest.digest().ok())
            .field("content_length", &self.plan.content_length)
            .field("reader", &"<verified stream>")
            .finish()
    }
}

impl VerifiedProviderIngestPayloadV1 {
    /// Construct exact verified material without copying payload bytes.
    #[must_use]
    pub fn new(manifest: ManifestV1, plan: CarBuildPlan, reader: Box<dyn Read + Send>) -> Self {
        Self {
            manifest,
            plan,
            reader,
        }
    }
}

/// Runtime-only authenticated source provider.
///
/// Implementations must resolve only current governance-admitted signed
/// adverts, use authenticated bounded grants with pinned trust, and perform
/// exact streaming verification before returning. Secrets and source
/// locations must never be exposed through `Debug`, logs, or readiness
/// artifacts.
pub trait ProviderIngestAuthenticatedSourceRuntimeV1:
    ProviderIngestAuthenticatedSourceFetchV1<Fetched = VerifiedProviderIngestPayloadV1>
{
    /// Stable production identity compared with `iroha_config`.
    fn runtime_handle(&self) -> &str;

    /// Non-mutating authenticated readiness probe.
    fn check_readiness(&self) -> std::result::Result<(), ProviderIngestSourceFetchErrorV1>;
}

/// Runtime-only governed signer resolver.
///
/// Resolution must validate the requested owner against the governance state
/// at the supplied finalized cursor, including current key rotation and
/// revocation, and the returned signer must repeat that key-validity check
/// atomically with signing. The returned signer signs only the exact payload
/// provided by the core runtime.
pub trait ProviderIngestGovernedSignerResolverRuntimeV1: Send + Sync + 'static {
    /// Stable production identity compared with `iroha_config`.
    fn runtime_handle(&self) -> &str;

    /// Non-mutating HSM/KMS and governance-readiness probe.
    fn check_readiness(
        &self,
    ) -> std::result::Result<(), ProviderIngestCompletionSignerResolverErrorV1>;

    /// Resolve one governed signer for the exact owner and finalized cursor.
    fn resolve<'a>(
        &'a self,
        provider_owner: AccountId,
        finalized_cursor: ProviderIngestFinalizedCursorV1,
    ) -> ProviderIngestFutureV1<
        'a,
        std::result::Result<
            Option<Arc<dyn ProviderIngestCompletionSignerV1>>,
            ProviderIngestCompletionSignerResolverErrorV1,
        >,
    >;
}

#[derive(Clone)]
struct AuthenticatedSourceAdapterV1(Arc<dyn ProviderIngestAuthenticatedSourceRuntimeV1>);

impl ProviderIngestAuthenticatedSourceFetchV1 for AuthenticatedSourceAdapterV1 {
    type Fetched = VerifiedProviderIngestPayloadV1;

    fn fetch<'a>(
        &'a self,
        request: ProviderIngestSourceRequestV1,
    ) -> ProviderIngestFutureV1<
        'a,
        std::result::Result<Self::Fetched, ProviderIngestSourceFetchErrorV1>,
    > {
        self.0.fetch(request)
    }
}

#[derive(Clone)]
struct GovernedCompletionSignerV1 {
    signer: Arc<dyn ProviderIngestCompletionSignerV1>,
    state: Arc<State>,
    provider_id: ProviderId,
    expected_owner: AccountId,
}

impl ProviderIngestCompletionSignerV1 for GovernedCompletionSignerV1 {
    fn authority(&self) -> &AccountId {
        self.signer.authority()
    }

    fn sign<'a>(
        &'a self,
        payload: TransactionPayload,
    ) -> ProviderIngestFutureV1<
        'a,
        std::result::Result<SignedTransaction, ProviderIngestCompletionSignerErrorV1>,
    > {
        Box::pin(async move {
            if !current_provider_owner_matches(
                self.state.as_ref(),
                self.provider_id,
                &self.expected_owner,
            ) {
                return Err(ProviderIngestCompletionSignerErrorV1::Unavailable);
            }
            let transaction = self.signer.sign(payload).await?;
            if !current_provider_owner_matches(
                self.state.as_ref(),
                self.provider_id,
                &self.expected_owner,
            ) {
                return Err(ProviderIngestCompletionSignerErrorV1::Unavailable);
            }
            Ok(transaction)
        })
    }
}

#[derive(Clone)]
struct GovernedSignerResolverAdapterV1 {
    resolver: Arc<dyn ProviderIngestGovernedSignerResolverRuntimeV1>,
    state: Arc<State>,
    provider_id: ProviderId,
}

impl ProviderIngestCompletionSignerResolverV1 for GovernedSignerResolverAdapterV1 {
    type Signer = GovernedCompletionSignerV1;

    fn resolve<'a>(
        &'a self,
        provider_owner: AccountId,
        finalized_cursor: ProviderIngestFinalizedCursorV1,
    ) -> ProviderIngestFutureV1<
        'a,
        std::result::Result<Option<Self::Signer>, ProviderIngestCompletionSignerResolverErrorV1>,
    > {
        Box::pin(async move {
            if !current_provider_owner_matches(
                self.state.as_ref(),
                self.provider_id,
                &provider_owner,
            ) {
                return Ok(None);
            }
            let expected_owner = provider_owner.clone();
            self.resolver
                .resolve(provider_owner, finalized_cursor)
                .await
                .map(|signer| {
                    signer.map(|signer| GovernedCompletionSignerV1 {
                        signer,
                        state: Arc::clone(&self.state),
                        provider_id: self.provider_id,
                        expected_owner,
                    })
                })
        })
    }
}

fn current_provider_owner_matches(
    state: &State,
    provider_id: ProviderId,
    expected_owner: &AccountId,
) -> bool {
    state
        .query_view()
        .world()
        .provider_owners()
        .get(&provider_id)
        == Some(expected_owner)
}

#[derive(Debug, Clone, Copy)]
struct FinalizedSnapshotProbeV1 {
    completed_cursor: Option<ProviderIngestFinalizedCursorV1>,
}

#[derive(Debug)]
struct OwnedFinalizedAssignmentSnapshotV1 {
    cursor: ProviderIngestFinalizedCursorV1,
    rows: Vec<ProviderIngestFinalizedAssignmentV1>,
    expected_after_order_id: Option<[u8; 32]>,
}

/// Native immutable finalized assignment reader.
///
/// A scan copies every relevant row from one `State::query_view`; it never
/// tries to reconstruct an historical view from a newer head. Continuations
/// are served only from that owned snapshot and must provide the exact cursor
/// and previous page boundary. Until the ledger exposes a provider-indexed
/// committed query, the row and byte budget is charged against every order
/// inspected, including orders assigned only to other providers.
#[derive(Clone)]
struct NativeFinalizedAssignmentLedgerV1 {
    state: Arc<State>,
    provider_id: ProviderId,
    max_snapshot_rows: usize,
    max_snapshot_bytes: u64,
    active: Arc<Mutex<Option<OwnedFinalizedAssignmentSnapshotV1>>>,
    probe: Arc<Mutex<FinalizedSnapshotProbeV1>>,
}

impl NativeFinalizedAssignmentLedgerV1 {
    fn new(
        state: Arc<State>,
        provider_id: ProviderId,
        max_snapshot_rows: usize,
        max_snapshot_bytes: u64,
        probe: Arc<Mutex<FinalizedSnapshotProbeV1>>,
    ) -> Self {
        Self {
            state,
            provider_id,
            max_snapshot_rows,
            max_snapshot_bytes,
            active: Arc::new(Mutex::new(None)),
            probe,
        }
    }

    fn build_snapshot(
        &self,
    ) -> std::result::Result<OwnedFinalizedAssignmentSnapshotV1, ProviderIngestFinalizedLedgerErrorV1>
    {
        let view = self.state.query_view();
        let height = u64::try_from(view.height())
            .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
        let block_hash = view
            .latest_block_hash()
            .map(|hash| *hash.as_ref())
            .filter(|hash| *hash != [0; 32])
            .ok_or(ProviderIngestFinalizedLedgerErrorV1::Unavailable)?;
        if height == 0
            || !committed_head_matches_hash_journal(height, block_hash, view.block_hashes())
        {
            return Err(ProviderIngestFinalizedLedgerErrorV1::Unavailable);
        }
        let cursor = ProviderIngestFinalizedCursorV1 { height, block_hash };
        let pin_cursor = PinManifestFinalizedCursorV1 { height, block_hash };
        let provider_owner = view
            .world()
            .provider_owners()
            .get(&self.provider_id)
            .cloned();
        let mut rows = Vec::new();
        let mut inspected_rows = 0_usize;
        let mut inspected_bytes = 0_u64;
        let mut selected_snapshot_bytes = 0_u64;

        for (order_id, order_record) in view.world().replication_orders().iter() {
            let estimated_row_bytes = charge_snapshot_scan_budget(
                &mut inspected_rows,
                &mut inspected_bytes,
                order_record.canonical_order.len(),
                order_record.manifest_root_cid.as_bytes().len(),
                order_record.provider_completions.len(),
                self.max_snapshot_rows,
                self.max_snapshot_bytes,
            )?;
            let exact_row_bytes = u64::try_from(
                norito::to_bytes(order_record)
                    .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?
                    .len(),
            )
            .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
            if let Some(additional_bytes) = exact_row_bytes.checked_sub(estimated_row_bytes) {
                inspected_bytes = inspected_bytes
                    .checked_add(additional_bytes)
                    .filter(|bytes| *bytes <= self.max_snapshot_bytes)
                    .ok_or(ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
            }
            if order_record.canonical_order.is_empty()
                || order_record.canonical_order.len() > REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1
            {
                return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
            }
            let order = decode_from_bytes_with_limits::<ReplicationOrderV1>(
                &order_record.canonical_order,
                REPLICATION_ORDER_DECODE_LIMITS_V1,
            )
            .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
            order
                .validate()
                .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
            let canonical = norito::to_bytes(&order)
                .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
            if canonical != order_record.canonical_order
                || order_id != &order_record.order_id
                || order.order_id != *order_record.order_id.as_bytes()
                || order.manifest_digest != *order_record.manifest_digest.as_bytes()
                || order.manifest_cid.as_slice() != order_record.manifest_root_cid.as_bytes()
            {
                return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
            }
            if !order
                .assignments
                .iter()
                .any(|assignment| assignment.provider_id == *self.provider_id.as_bytes())
            {
                continue;
            }
            let pin = view
                .world()
                .pin_manifests()
                .get(&order_record.manifest_digest)
                .cloned()
                .ok_or(ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
            if pin.digest != order_record.manifest_digest
                || pin.root_cid != order_record.manifest_root_cid
                || pin.chunker.to_handle() != order.chunking_profile
            {
                return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
            }
            let completion_epoch = matches!(order_record.status, ReplicationOrderStatus::Pending)
                .then_some(height)
                .filter(|epoch| {
                    *epoch >= order_record.issued_epoch && *epoch <= order_record.deadline_epoch
                });
            let row = ProviderIngestFinalizedAssignmentV1 {
                pin: PinManifestFinalizedRecordV1 {
                    finalized_cursor: pin_cursor,
                    manifest: pin,
                },
                order: order_record.clone(),
                provider_owner: provider_owner.clone(),
                completion_epoch,
                committed_transaction_hash: None,
            };
            let row_bytes = norito::to_bytes(&row.pin)
                .and_then(|pin_bytes| {
                    norito::to_bytes(&row.order).map(|order_bytes| {
                        pin_bytes
                            .len()
                            .checked_add(order_bytes.len())
                            .and_then(|bytes| {
                                row.provider_owner.as_ref().map_or(Some(bytes), |owner| {
                                    norito::to_bytes(owner).ok().and_then(|owner_bytes| {
                                        bytes.checked_add(owner_bytes.len())
                                    })
                                })
                            })
                            .and_then(|bytes| {
                                bytes.checked_add(SNAPSHOT_ROW_STRUCTURAL_OVERHEAD_BYTES_V1)
                            })
                    })
                })
                .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?
                .ok_or(ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
            selected_snapshot_bytes = selected_snapshot_bytes
                .checked_add(
                    u64::try_from(row_bytes)
                        .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?,
                )
                .ok_or(ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
            if rows.len() >= self.max_snapshot_rows
                || selected_snapshot_bytes > self.max_snapshot_bytes
            {
                return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
            }
            rows.push(row);
        }
        rows.sort_by_key(|row| *row.order.order_id.as_bytes());
        Ok(OwnedFinalizedAssignmentSnapshotV1 {
            cursor,
            rows,
            expected_after_order_id: None,
        })
    }

    fn read_page(
        &self,
        at_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
        after_order_id: Option<[u8; 32]>,
        limit: usize,
    ) -> std::result::Result<
        ProviderIngestFinalizedAssignmentPageV1,
        ProviderIngestFinalizedLedgerErrorV1,
    > {
        if limit == 0 || limit > self.max_snapshot_rows {
            return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
        }
        let mut active = self
            .active
            .lock()
            .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Unavailable)?;
        if at_finalized_cursor.is_none() {
            if after_order_id.is_some() || active.is_some() {
                return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
            }
            *active = Some(self.build_snapshot()?);
        }
        let snapshot = active
            .as_mut()
            .ok_or(ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
        if at_finalized_cursor.is_some_and(|cursor| cursor != snapshot.cursor)
            || after_order_id != snapshot.expected_after_order_id
        {
            return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
        }
        let start = after_order_id.map_or(0, |after| {
            snapshot
                .rows
                .binary_search_by_key(&after, |row| *row.order.order_id.as_bytes())
                .map_or(usize::MAX, |index| index.saturating_add(1))
        });
        if start == usize::MAX || start > snapshot.rows.len() {
            return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
        }
        let end = start.saturating_add(limit).min(snapshot.rows.len());
        let rows = snapshot.rows[start..end].to_vec();
        let next_after_order_id = (end < snapshot.rows.len()).then(|| {
            *rows
                .last()
                .expect("non-empty page")
                .order
                .order_id
                .as_bytes()
        });
        let cursor = snapshot.cursor;
        if let Some(next) = next_after_order_id {
            snapshot.expected_after_order_id = Some(next);
        } else {
            *active = None;
            let mut probe = self
                .probe
                .lock()
                .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Unavailable)?;
            probe.completed_cursor = Some(cursor);
        }
        Ok(ProviderIngestFinalizedAssignmentPageV1 {
            finalized_cursor: cursor,
            rows,
            next_after_order_id,
        })
    }
}

fn charge_snapshot_scan_budget(
    inspected_rows: &mut usize,
    inspected_bytes: &mut u64,
    canonical_order_bytes: usize,
    manifest_cid_bytes: usize,
    completion_count: usize,
    max_rows: usize,
    max_bytes: u64,
) -> std::result::Result<u64, ProviderIngestFinalizedLedgerErrorV1> {
    if *inspected_rows >= max_rows {
        return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
    }
    let completion_bytes = completion_count
        .checked_mul(SNAPSHOT_ROW_STRUCTURAL_OVERHEAD_BYTES_V1)
        .ok_or(ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
    let row_bytes = canonical_order_bytes
        .checked_add(manifest_cid_bytes)
        .and_then(|bytes| bytes.checked_add(completion_bytes))
        .and_then(|bytes| bytes.checked_add(SNAPSHOT_ROW_STRUCTURAL_OVERHEAD_BYTES_V1))
        .ok_or(ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
    let row_bytes =
        u64::try_from(row_bytes).map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
    let next_bytes = inspected_bytes
        .checked_add(row_bytes)
        .ok_or(ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
    if next_bytes > max_bytes {
        return Err(ProviderIngestFinalizedLedgerErrorV1::Rejected);
    }
    *inspected_rows = inspected_rows
        .checked_add(1)
        .ok_or(ProviderIngestFinalizedLedgerErrorV1::Rejected)?;
    *inspected_bytes = next_bytes;
    Ok(row_bytes)
}

impl ProviderIngestFinalizedLedgerV1 for NativeFinalizedAssignmentLedgerV1 {
    fn read_assignment_page<'a>(
        &'a self,
        at_finalized_cursor: Option<ProviderIngestFinalizedCursorV1>,
        after_order_id: Option<[u8; 32]>,
        limit: usize,
    ) -> ProviderIngestFutureV1<
        'a,
        std::result::Result<
            ProviderIngestFinalizedAssignmentPageV1,
            ProviderIngestFinalizedLedgerErrorV1,
        >,
    > {
        let ledger = self.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                ledger.read_page(at_finalized_cursor, after_order_id, limit)
            })
            .await
            .map_err(|_| ProviderIngestFinalizedLedgerErrorV1::Unavailable)?
        })
    }
}

#[derive(Clone)]
struct NativeProviderIngestLocalStorageV1 {
    node: NodeHandle,
    operation_timeout: Duration,
}

impl NativeProviderIngestLocalStorageV1 {
    fn new(node: NodeHandle, operation_timeout: Duration) -> Self {
        Self {
            node,
            operation_timeout,
        }
    }
}

struct DeadlineBoundedReaderV1 {
    inner: Box<dyn Read + Send>,
    deadline: Instant,
    remaining: u64,
}

impl DeadlineBoundedReaderV1 {
    fn new(inner: Box<dyn Read + Send>, timeout: Duration, expected_bytes: u64) -> Self {
        Self {
            inner,
            deadline: Instant::now()
                .checked_add(timeout)
                .unwrap_or_else(Instant::now),
            remaining: expected_bytes,
        }
    }
}

impl Read for DeadlineBoundedReaderV1 {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() || self.remaining == 0 {
            return Ok(0);
        }
        if Instant::now() >= self.deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "provider-ingest verified reader exceeded its operation deadline",
            ));
        }
        let remaining = usize::try_from(self.remaining).unwrap_or(usize::MAX);
        let read_limit = remaining.min(output.len());
        let read = self.inner.read(&mut output[..read_limit])?;
        if Instant::now() > self.deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "provider-ingest verified reader exceeded its operation deadline",
            ));
        }
        self.remaining = self
            .remaining
            .checked_sub(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "provider-ingest verified reader exceeded its authorized length",
                )
            })?;
        Ok(read)
    }
}

struct BlockingStoreJoinGuardV1(Option<std::thread::JoinHandle<()>>);

impl BlockingStoreJoinGuardV1 {
    fn join(mut self) -> bool {
        self.0.take().is_some_and(|thread| thread.join().is_ok())
    }
}

impl Drop for BlockingStoreJoinGuardV1 {
    fn drop(&mut self) {
        if let Some(thread) = self.0.take() {
            let _ = thread.join();
        }
    }
}

impl ProviderIngestLocalStorageV1<VerifiedProviderIngestPayloadV1>
    for NativeProviderIngestLocalStorageV1
{
    fn verify_existing<'a>(
        &'a self,
        authorization: FinalizedProviderIngestAuthorizationV1,
    ) -> ProviderIngestFutureV1<
        'a,
        std::result::Result<Option<String>, ProviderIngestLocalStorageErrorV1>,
    > {
        let node = self.node.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || verify_existing_manifest(&node, &authorization))
                .await
                .map_err(|_| ProviderIngestLocalStorageErrorV1::Retryable)?
        })
    }

    fn store<'a>(
        &'a self,
        authorization: FinalizedProviderIngestAuthorizationV1,
        mut fetched: VerifiedProviderIngestPayloadV1,
    ) -> ProviderIngestFutureV1<'a, std::result::Result<String, ProviderIngestLocalStorageErrorV1>>
    {
        let node = self.node.clone();
        let operation_timeout = self.operation_timeout;
        Box::pin(async move {
            validate_verified_payload(&authorization, &fetched.manifest, &fetched.plan)?;
            fetched.reader = Box::new(DeadlineBoundedReaderV1::new(
                fetched.reader,
                operation_timeout,
                authorization.content_length(),
            ));
            let (result_sender, result_receiver) = tokio::sync::oneshot::channel();
            let thread = std::thread::Builder::new()
                .name("sorafs-provider-ingest-store".to_owned())
                .spawn(move || {
                    let result = match node.ingest_manifest(
                        &fetched.manifest,
                        &fetched.plan,
                        &mut fetched.reader,
                    ) {
                        Ok(manifest_id) => Ok(manifest_id),
                        Err(NodeStorageError::Storage(StorageError::ManifestExists { .. })) => {
                            match verify_existing_manifest(&node, &authorization) {
                                Ok(Some(manifest_id)) => Ok(manifest_id),
                                Ok(None) => Err(ProviderIngestLocalStorageErrorV1::Permanent),
                                Err(error) => Err(error),
                            }
                        }
                        Err(error) => Err(classify_storage_error(&error)),
                    };
                    let _ = result_sender.send(result);
                })
                .map_err(|_| ProviderIngestLocalStorageErrorV1::Retryable)?;
            let guard = BlockingStoreJoinGuardV1(Some(thread));
            let result = result_receiver
                .await
                .map_err(|_| ProviderIngestLocalStorageErrorV1::Retryable)?;
            if !guard.join() {
                return Err(ProviderIngestLocalStorageErrorV1::Retryable);
            }
            result
        })
    }
}

fn verify_existing_manifest(
    node: &NodeHandle,
    authorization: &FinalizedProviderIngestAuthorizationV1,
) -> std::result::Result<Option<String>, ProviderIngestLocalStorageErrorV1> {
    let stored = match node.manifest_metadata_by_digest(&authorization.manifest_digest()) {
        Ok(stored) => stored,
        Err(NodeStorageError::Storage(StorageError::ManifestNotFound { .. })) => return Ok(None),
        Err(error) => return Err(classify_storage_error(&error)),
    };
    if stored.manifest_digest() != &authorization.manifest_digest()
        || stored.manifest_cid() != authorization.manifest_cid()
        || stored.content_length() != authorization.content_length()
        || stored.chunk_profile_handle() != authorization.chunker_handle()
    {
        return Err(ProviderIngestLocalStorageErrorV1::Permanent);
    }
    let manifest = stored
        .load_manifest()
        .map_err(|_| ProviderIngestLocalStorageErrorV1::Permanent)?;
    validate_manifest_binding(authorization, &manifest)?;
    if stored.payload_digest() != &manifest.car_digest {
        return Err(ProviderIngestLocalStorageErrorV1::Permanent);
    }
    Ok(Some(stored.manifest_id().to_owned()))
}

fn validate_manifest_binding(
    authorization: &FinalizedProviderIngestAuthorizationV1,
    manifest: &ManifestV1,
) -> std::result::Result<(), ProviderIngestLocalStorageErrorV1> {
    let digest = manifest
        .digest()
        .map_err(|_| ProviderIngestLocalStorageErrorV1::Permanent)?;
    let profile = format!(
        "{}.{}@{}",
        manifest.chunking.namespace, manifest.chunking.name, manifest.chunking.semver
    );
    if digest.as_bytes() != &authorization.manifest_digest()
        || manifest.root_cid.as_slice() != authorization.manifest_cid()
        || profile != authorization.chunker_handle()
        || manifest.chunk_digest_sha3_256 != authorization.chunk_digest_sha3_256()
        || manifest.por_root != authorization.por_root()
        || manifest.content_length != authorization.content_length()
    {
        return Err(ProviderIngestLocalStorageErrorV1::Permanent);
    }
    Ok(())
}

fn validate_verified_payload(
    authorization: &FinalizedProviderIngestAuthorizationV1,
    manifest: &ManifestV1,
    plan: &CarBuildPlan,
) -> std::result::Result<(), ProviderIngestLocalStorageErrorV1> {
    validate_manifest_binding(authorization, manifest)?;
    if plan.content_length != authorization.content_length()
        || plan.payload_digest.as_bytes() != &manifest.car_digest
        || compute_chunk_plan_digest_sha3(&plan.chunks) != authorization.chunk_digest_sha3_256()
        || u32::try_from(plan.chunk_profile.min_size).ok() != Some(manifest.chunking.min_size)
        || u32::try_from(plan.chunk_profile.target_size).ok() != Some(manifest.chunking.target_size)
        || u32::try_from(plan.chunk_profile.max_size).ok() != Some(manifest.chunking.max_size)
        || u32::try_from(plan.chunk_profile.break_mask).ok() != Some(manifest.chunking.break_mask)
    {
        return Err(ProviderIngestLocalStorageErrorV1::Permanent);
    }
    Ok(())
}

fn classify_storage_error(error: &NodeStorageError) -> ProviderIngestLocalStorageErrorV1 {
    match error {
        NodeStorageError::Storage(
            StorageError::ChunkDigestMismatch { .. }
            | StorageError::ManifestContentLengthMismatch
            | StorageError::InvalidFileLayout { .. }
            | StorageError::CorruptStorageState { .. }
            | StorageError::UnsupportedIndexVersion { .. },
        ) => ProviderIngestLocalStorageErrorV1::Permanent,
        NodeStorageError::Disabled
        | NodeStorageError::Scheduler(_)
        | NodeStorageError::Storage(_) => ProviderIngestLocalStorageErrorV1::Retryable,
    }
}

#[derive(Clone)]
struct NativeCompletionPayloadBuilderV1 {
    chain_id: ChainId,
    state: Arc<State>,
    queue: Arc<Queue>,
    ttl: Duration,
    max_signed_transaction_bytes: u64,
}

impl NativeCompletionPayloadBuilderV1 {
    fn build_payload_sync(
        &self,
        request: ProviderIngestCompletionPayloadRequestV1,
    ) -> std::result::Result<TransactionPayload, ProviderIngestCompletionPayloadErrorV1> {
        if request.chain_id != self.chain_id
            || request.finalized_cursor.height == 0
            || request.finalized_cursor.block_hash == [0; 32]
        {
            return Err(ProviderIngestCompletionPayloadErrorV1::Rejected);
        }
        let view = self.state.query_view();
        let height = u64::try_from(view.height())
            .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        let head_hash = view
            .latest_block_hash()
            .map(|hash| *hash.as_ref())
            .ok_or(ProviderIngestCompletionPayloadErrorV1::Unavailable)?;
        if view.chain_id != self.chain_id
            || height < request.finalized_cursor.height
            || !committed_head_matches_hash_journal(height, head_hash, view.block_hashes())
            || !cursor_matches_committed_hashes(request.finalized_cursor, view.block_hashes())
            || request.completion_epoch != request.finalized_cursor.height
        {
            return Err(ProviderIngestCompletionPayloadErrorV1::Rejected);
        }
        let provider_id = ProviderId::new(request.authorization.provider_id());
        let order_id = ReplicationOrderId::new(request.authorization.order_id());
        let world = view.world();
        if world.provider_owners().get(&provider_id) != Some(&request.provider_owner) {
            return Err(ProviderIngestCompletionPayloadErrorV1::Rejected);
        }
        let order_record = world
            .replication_orders()
            .get(&order_id)
            .ok_or(ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        if !matches!(order_record.status, ReplicationOrderStatus::Pending)
            || order_record.provider_completion(provider_id).is_some()
            || request.completion_epoch < order_record.issued_epoch
            || request.completion_epoch > order_record.deadline_epoch
            || height > order_record.deadline_epoch
            || order_record.order_id != order_id
            || *order_record.manifest_digest.as_bytes() != request.authorization.manifest_digest()
            || order_record.manifest_root_cid.as_bytes() != request.authorization.manifest_cid()
            || order_record.canonical_order.is_empty()
            || order_record.canonical_order.len() > REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1
        {
            return Err(ProviderIngestCompletionPayloadErrorV1::Rejected);
        }
        let order = decode_from_bytes_with_limits::<ReplicationOrderV1>(
            &order_record.canonical_order,
            REPLICATION_ORDER_DECODE_LIMITS_V1,
        )
        .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        order
            .validate()
            .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        if norito::to_bytes(&order).map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)?
            != order_record.canonical_order
            || order.order_id != request.authorization.order_id()
            || order.manifest_digest != request.authorization.manifest_digest()
            || order.manifest_cid.as_slice() != request.authorization.manifest_cid()
            || !order
                .assignments
                .iter()
                .any(|assignment| assignment.provider_id == request.authorization.provider_id())
        {
            return Err(ProviderIngestCompletionPayloadErrorV1::Rejected);
        }
        let pin = world
            .pin_manifests()
            .get(&order_record.manifest_digest)
            .ok_or(ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        if !matches!(pin.status, PinStatus::Approved(_))
            || pin.digest != order_record.manifest_digest
            || pin.root_cid != order_record.manifest_root_cid
            || pin.chunker.to_handle() != request.authorization.chunker_handle()
            || pin.chunk_digest_sha3_256 != request.authorization.chunk_digest_sha3_256()
            || pin.por_root != request.authorization.por_root()
            || pin.content_length != request.authorization.content_length()
        {
            return Err(ProviderIngestCompletionPayloadErrorV1::Rejected);
        }
        let instruction =
            CompleteReplicationOrder::new(order_id, provider_id, request.completion_epoch);
        let mut builder = TransactionBuilder::new(
            self.chain_id.clone(),
            request.provider_owner,
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction]);
        builder.set_ttl(self.ttl);
        let mut payload = builder
            .into_payload()
            .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        let route = self
            .queue
            .route_payload_with_state(&payload, self.state.as_ref())
            .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        let next_height = height
            .checked_add(1)
            .ok_or(ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        let quote = iroha_core::executor::quote_nexus_fee_admission_draft(
            view.world(),
            &view.nexus,
            &view.pipeline,
            &payload,
            payload.creation_time_ms,
            next_height,
            Some(route.dataspace_id),
        )
        .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        payload.fee_payment = quote.recommended_intent;
        let encoded = norito::to_bytes(&payload)
            .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        let encoded_len = u64::try_from(encoded.len())
            .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Rejected)?;
        if encoded_len
            .checked_add(SIGNED_TRANSACTION_ENVELOPE_RESERVE_BYTES_V1)
            .is_none_or(|bytes| bytes > self.max_signed_transaction_bytes)
        {
            return Err(ProviderIngestCompletionPayloadErrorV1::Rejected);
        }
        Ok(payload)
    }
}

impl ProviderIngestCompletionPayloadBuilderV1 for NativeCompletionPayloadBuilderV1 {
    fn build_payload<'a>(
        &'a self,
        request: ProviderIngestCompletionPayloadRequestV1,
    ) -> ProviderIngestFutureV1<
        'a,
        std::result::Result<TransactionPayload, ProviderIngestCompletionPayloadErrorV1>,
    > {
        let builder = self.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || builder.build_payload_sync(request))
                .await
                .map_err(|_| ProviderIngestCompletionPayloadErrorV1::Unavailable)?
        })
    }
}

#[derive(Clone)]
struct NativeTransactionIngressV1 {
    chain_id: ChainId,
    state: Arc<State>,
    queue: Arc<Queue>,
}

impl NativeTransactionIngressV1 {
    fn prepare_sync(
        &self,
        transaction: SignedTransaction,
    ) -> std::result::Result<AcceptedTransaction<'static>, ProviderIngestIngressPrepareErrorV1>
    {
        let (max_clock_drift, transaction_parameters) = self.state.transaction_admission_limits();
        AcceptedTransaction::accept(
            transaction,
            &self.chain_id,
            max_clock_drift,
            transaction_parameters,
            self.state.crypto().as_ref(),
        )
        .map_err(|_| ProviderIngestIngressPrepareErrorV1::Rejected)
    }

    fn expose_sync(
        &self,
        prepared: AcceptedTransaction<'static>,
        transaction: SignedTransaction,
    ) -> ProviderIngestIngressDispositionV1 {
        if prepared.hash() != transaction.hash() {
            return ProviderIngestIngressDispositionV1::Rejected;
        }
        match self
            .queue
            .push_with_lane_with_state(prepared, self.state.as_ref())
        {
            Ok(_) => ProviderIngestIngressDispositionV1::Submitted,
            Err(failure)
                if matches!(
                    failure.err,
                    QueueError::InBlockchain | QueueError::IsInQueue
                ) =>
            {
                ProviderIngestIngressDispositionV1::Submitted
            }
            Err(failure)
                if matches!(
                    failure.err,
                    QueueError::PlanJournalDurabilityIndeterminate { .. }
                ) =>
            {
                ProviderIngestIngressDispositionV1::Ambiguous
            }
            Err(failure)
                if matches!(
                    failure.err,
                    QueueError::Full
                        | QueueError::LatencySaturated
                        | QueueError::MaximumTransactionsPerUser
                        | QueueError::PlanJournalDurabilityRejected { .. }
                ) =>
            {
                ProviderIngestIngressDispositionV1::DefinitelyNotSubmitted
            }
            Err(_) => ProviderIngestIngressDispositionV1::Rejected,
        }
    }

    fn observe_sync(&self, transaction_hash: [u8; 32]) -> ProviderIngestTransactionObservationV1 {
        if transaction_hash == [0; 32] {
            return ProviderIngestTransactionObservationV1::CommittedRejected;
        }
        let hash =
            HashOf::<SignedTransaction>::from_untyped_unchecked(Hash::prehashed(transaction_hash));
        let Some(height) = self.state.committed_transaction_height(&hash) else {
            return if self.queue.contains_pending_hash(hash, self.state.as_ref()) {
                ProviderIngestTransactionObservationV1::Pending
            } else {
                ProviderIngestTransactionObservationV1::Unknown
            };
        };
        let Some(block) = self.state.block_by_height(height) else {
            return ProviderIngestTransactionObservationV1::Unavailable;
        };
        let Some(index) =
            block
                .external_entrypoints_cloned()
                .enumerate()
                .find_map(|(index, entrypoint)| match entrypoint {
                    TransactionEntrypoint::External(transaction) if transaction.hash() == hash => {
                        Some(index)
                    }
                    _ => None,
                })
        else {
            return ProviderIngestTransactionObservationV1::Unavailable;
        };
        if !block.has_results() {
            return ProviderIngestTransactionObservationV1::Unavailable;
        }
        match block.results().nth(index).map(|result| result.as_ref()) {
            Some(Ok(_)) => ProviderIngestTransactionObservationV1::CommittedSuccess,
            Some(Err(_)) => ProviderIngestTransactionObservationV1::CommittedRejected,
            None => ProviderIngestTransactionObservationV1::Unavailable,
        }
    }
}

impl ProviderIngestTransactionIngressV1 for NativeTransactionIngressV1 {
    type Prepared = AcceptedTransaction<'static>;

    fn prepare<'a>(
        &'a self,
        transaction: SignedTransaction,
    ) -> ProviderIngestFutureV1<
        'a,
        std::result::Result<Self::Prepared, ProviderIngestIngressPrepareErrorV1>,
    > {
        let ingress = self.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || ingress.prepare_sync(transaction))
                .await
                .map_err(|_| ProviderIngestIngressPrepareErrorV1::Rejected)?
        })
    }

    fn expose<'a>(
        &'a self,
        prepared: Self::Prepared,
        transaction: SignedTransaction,
    ) -> ProviderIngestFutureV1<'a, ProviderIngestIngressDispositionV1> {
        let ingress = self.clone();
        Box::pin(async move {
            match tokio::task::spawn_blocking(move || ingress.expose_sync(prepared, transaction))
                .await
            {
                Ok(disposition) => disposition,
                Err(_) => ProviderIngestIngressDispositionV1::Ambiguous,
            }
        })
    }

    fn observe<'a>(
        &'a self,
        transaction_hash: [u8; 32],
    ) -> ProviderIngestFutureV1<'a, ProviderIngestTransactionObservationV1> {
        let ingress = self.clone();
        Box::pin(async move {
            match tokio::task::spawn_blocking(move || ingress.observe_sync(transaction_hash)).await
            {
                Ok(observation) => observation,
                Err(_) => ProviderIngestTransactionObservationV1::Unavailable,
            }
        })
    }
}

#[derive(Debug, Default)]
struct ProviderIngestDaemonCountersV1 {
    successful_ticks: AtomicU64,
    failed_ticks: AtomicU64,
    rows_scanned: AtomicU64,
    jobs_inserted: AtomicU64,
    jobs_finalized: AtomicU64,
    jobs_cancelled: AtomicU64,
    source_jobs_claimed: AtomicU64,
    manifests_stored: AtomicU64,
    completions_signed: AtomicU64,
    completion_submissions: AtomicU64,
}

/// Payload-free supervised provider-ingest metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderIngestDaemonMetricsV1 {
    /// Successful bounded reconciliation ticks.
    pub successful_ticks: u64,
    /// Failed dependency probes or reconciliation ticks.
    pub failed_ticks: u64,
    /// Finalized assignment rows inspected.
    pub rows_scanned: u64,
    /// Durable jobs first admitted.
    pub jobs_inserted: u64,
    /// Jobs reconciled to finalized completion.
    pub jobs_finalized: u64,
    /// Jobs reconciled to authoritative cancellation.
    pub jobs_cancelled: u64,
    /// Source jobs claimed.
    pub source_jobs_claimed: u64,
    /// Exact manifests confirmed durable.
    pub manifests_stored: u64,
    /// Completion transactions signed.
    pub completions_signed: u64,
    /// Completion transaction exposure attempts.
    pub completion_submissions: u64,
}

/// Payload-free provider-ingest readiness projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderIngestDaemonStatusV1 {
    /// Whether the supervised worker task is still alive.
    pub worker_running: bool,
    /// Whether both runtime-only adapters passed the latest probe.
    pub external_dependencies_healthy: bool,
    /// Whether one bounded tick is currently executing.
    pub tick_in_flight: bool,
    /// Whether a successful tick is within the configured freshness bound.
    pub last_tick_fresh: bool,
    /// Latest fully scanned immutable finalized cursor.
    pub completed_scan_cursor: Option<ProviderIngestFinalizedCursorV1>,
    /// Current committed head height.
    pub finalized_head_height: u64,
    /// Whether the completed cursor can still be a prefix of the current
    /// finalized head.
    pub finalized_cursor_consistent: bool,
    /// Difference between current head and completed scan.
    pub finalized_lag_blocks: u64,
    /// Non-terminal durable outbox rows.
    pub active_jobs: usize,
    /// Terminal durable outbox rows.
    pub terminal_jobs: usize,
    /// Terminal dead letters, which block readiness.
    pub dead_letters: usize,
    /// Operational health/readiness, independent of whether admitted work is
    /// currently drained.
    pub ready: bool,
    /// Whether all admitted work reached a terminal non-dead-letter state.
    pub drained: bool,
    /// Release-gate readiness: operationally ready and fully drained.
    pub release_ready: bool,
}

/// Cloneable status/metrics handle retained by `irohad`.
#[derive(Clone)]
pub struct ProviderIngestRuntimeHandleV1 {
    node: NodeHandle,
    state: Arc<State>,
    config: SorafsProviderIngestRuntime,
    probe: Arc<Mutex<FinalizedSnapshotProbeV1>>,
    counters: Arc<ProviderIngestDaemonCountersV1>,
    worker_running: Arc<AtomicBool>,
    external_dependencies_healthy: Arc<AtomicBool>,
    tick_in_flight: Arc<AtomicBool>,
    last_successful_tick: Arc<Mutex<Option<Instant>>>,
}

impl fmt::Debug for ProviderIngestRuntimeHandleV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProviderIngestRuntimeHandleV1")
            .field("runtime_state", &"[PAYLOAD-FREE]")
            .finish_non_exhaustive()
    }
}

impl ProviderIngestRuntimeHandleV1 {
    /// Return payload-free status without performing runtime work.
    pub fn status(&self) -> Result<ProviderIngestDaemonStatusV1> {
        let completed_scan_cursor = self
            .probe
            .lock()
            .map_err(|_| eyre::eyre!("provider-ingest finalized snapshot probe is poisoned"))?
            .completed_cursor;
        let view = self.state.query_view();
        let finalized_head_height = u64::try_from(view.height())
            .wrap_err("provider-ingest committed height is not representable")?;
        let finalized_head_hash = view
            .latest_block_hash()
            .map(|hash| *hash.as_ref())
            .filter(|hash| *hash != [0; 32])
            .ok_or_else(|| eyre::eyre!("provider-ingest committed head is unavailable"))?;
        let finalized_cursor_consistent = completed_cursor_matches_committed_chain(
            completed_scan_cursor,
            finalized_head_height,
            finalized_head_hash,
            view.block_hashes(),
        );
        let finalized_lag_blocks = finalized_head_height
            .saturating_sub(completed_scan_cursor.map_or(0, |cursor| cursor.height));
        let last_tick_fresh = self
            .last_successful_tick
            .lock()
            .map_err(|_| eyre::eyre!("provider-ingest tick freshness state is poisoned"))?
            .as_ref()
            .is_some_and(|instant| {
                instant.elapsed()
                    <= Duration::from_millis(self.config.scan_interval_ms)
                        .checked_mul(READINESS_STALE_TICK_MULTIPLIER_V1)
                        .unwrap_or(Duration::MAX)
            });
        let (active_jobs, terminal_jobs, dead_letters) = self.outbox_counts()?;
        let worker_running = self.worker_running.load(Ordering::Acquire);
        let external_dependencies_healthy =
            self.external_dependencies_healthy.load(Ordering::Acquire);
        let tick_in_flight = self.tick_in_flight.load(Ordering::Acquire);
        let ready = worker_running
            && external_dependencies_healthy
            && !tick_in_flight
            && last_tick_fresh
            && finalized_cursor_consistent
            && finalized_lag_blocks <= self.config.max_finalized_lag_blocks
            && dead_letters == 0;
        let drained = active_jobs == 0 && dead_letters == 0;
        let release_ready = ready && drained;
        Ok(ProviderIngestDaemonStatusV1 {
            worker_running,
            external_dependencies_healthy,
            tick_in_flight,
            last_tick_fresh,
            completed_scan_cursor,
            finalized_head_height,
            finalized_cursor_consistent,
            finalized_lag_blocks,
            active_jobs,
            terminal_jobs,
            dead_letters,
            ready,
            drained,
            release_ready,
        })
    }

    /// Return payload-free monotonic counters.
    #[must_use]
    pub fn metrics(&self) -> ProviderIngestDaemonMetricsV1 {
        ProviderIngestDaemonMetricsV1 {
            successful_ticks: self.counters.successful_ticks.load(Ordering::Relaxed),
            failed_ticks: self.counters.failed_ticks.load(Ordering::Relaxed),
            rows_scanned: self.counters.rows_scanned.load(Ordering::Relaxed),
            jobs_inserted: self.counters.jobs_inserted.load(Ordering::Relaxed),
            jobs_finalized: self.counters.jobs_finalized.load(Ordering::Relaxed),
            jobs_cancelled: self.counters.jobs_cancelled.load(Ordering::Relaxed),
            source_jobs_claimed: self.counters.source_jobs_claimed.load(Ordering::Relaxed),
            manifests_stored: self.counters.manifests_stored.load(Ordering::Relaxed),
            completions_signed: self.counters.completions_signed.load(Ordering::Relaxed),
            completion_submissions: self.counters.completion_submissions.load(Ordering::Relaxed),
        }
    }

    fn outbox_counts(&self) -> Result<(usize, usize, usize)> {
        let counts = self
            .node
            .finalized_provider_ingest_counts()
            .wrap_err("inspect provider-ingest durable outbox counts")?;
        Ok((counts.active, counts.terminal, counts.dead_letters))
    }
}

fn committed_head_matches_hash_journal(
    head_height: u64,
    head_hash: [u8; 32],
    committed_hashes: &[HashOf<BlockHeader>],
) -> bool {
    usize::try_from(head_height).ok() == Some(committed_hashes.len())
        && committed_hashes
            .last()
            .is_some_and(|hash| *hash.as_ref() == head_hash)
}

fn cursor_matches_committed_hashes(
    cursor: ProviderIngestFinalizedCursorV1,
    committed_hashes: &[HashOf<BlockHeader>],
) -> bool {
    let Some(index) = usize::try_from(cursor.height)
        .ok()
        .and_then(|height| height.checked_sub(1))
    else {
        return false;
    };
    cursor.block_hash != [0; 32]
        && committed_hashes
            .get(index)
            .is_some_and(|hash| *hash.as_ref() == cursor.block_hash)
}

fn completed_cursor_matches_committed_chain(
    completed: Option<ProviderIngestFinalizedCursorV1>,
    head_height: u64,
    head_hash: [u8; 32],
    committed_hashes: &[HashOf<BlockHeader>],
) -> bool {
    let Some(completed) = completed else {
        return false;
    };
    completed.height <= head_height
        && committed_head_matches_hash_journal(head_height, head_hash, committed_hashes)
        && cursor_matches_committed_hashes(completed, committed_hashes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeDependencyProbeV1 {
    Ready,
    NotReady,
    TimedOutOrPanicked,
}

async fn bounded_blocking_readiness_probe<F>(
    deadline: Duration,
    probe: F,
) -> RuntimeDependencyProbeV1
where
    F: FnOnce() -> bool + Send + 'static,
{
    match tokio::time::timeout(deadline, tokio::task::spawn_blocking(probe)).await {
        Ok(Ok(true)) => RuntimeDependencyProbeV1::Ready,
        Ok(Ok(false)) => RuntimeDependencyProbeV1::NotReady,
        Ok(Err(_)) | Err(_) => RuntimeDependencyProbeV1::TimedOutOrPanicked,
    }
}

async fn probe_runtime_dependencies(
    authenticated_source: Arc<dyn ProviderIngestAuthenticatedSourceRuntimeV1>,
    signer_resolver: Arc<dyn ProviderIngestGovernedSignerResolverRuntimeV1>,
    source_deadline: Duration,
    signer_deadline: Duration,
) -> RuntimeDependencyProbeV1 {
    let source = bounded_blocking_readiness_probe(source_deadline, move || {
        authenticated_source.check_readiness().is_ok()
    });
    let signer = bounded_blocking_readiness_probe(signer_deadline, move || {
        signer_resolver.check_readiness().is_ok()
    });
    let (source, signer) = tokio::join!(source, signer);
    if source == RuntimeDependencyProbeV1::TimedOutOrPanicked
        || signer == RuntimeDependencyProbeV1::TimedOutOrPanicked
    {
        RuntimeDependencyProbeV1::TimedOutOrPanicked
    } else if source == RuntimeDependencyProbeV1::Ready && signer == RuntimeDependencyProbeV1::Ready
    {
        RuntimeDependencyProbeV1::Ready
    } else {
        RuntimeDependencyProbeV1::NotReady
    }
}

fn provider_ingest_shutdown_wait(config: &SorafsProviderIngestRuntime) -> Duration {
    let source_budget = config.source_operation_timeout_ms.saturating_mul(3);
    let signer_budget = config.signer_timeout_ms.saturating_mul(4);
    let ingress_budget = config.ingress_timeout_ms.saturating_mul(4);
    Duration::from_millis(
        source_budget
            .saturating_add(signer_budget)
            .saturating_add(ingress_budget),
    )
    .saturating_add(SHUTDOWN_WAIT_FLOOR)
}

/// Assemble and start supervised finalized-ledger provider ingest.
///
/// Missing, test-marked, unready, or identity-substituted runtime adapters
/// fail startup before the worker is spawned.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn start(
    config: SorafsProviderIngestRuntime,
    chain_id: ChainId,
    state: Arc<State>,
    queue: Arc<Queue>,
    node: NodeHandle,
    authenticated_source: Arc<dyn ProviderIngestAuthenticatedSourceRuntimeV1>,
    signer_resolver: Arc<dyn ProviderIngestGovernedSignerResolverRuntimeV1>,
    shutdown_signal: ShutdownSignal,
) -> Result<(ProviderIngestRuntimeHandleV1, Child)> {
    validate_config(&config)?;
    validate_dependency_identity(
        "authenticated source-fetch",
        &config.authenticated_source_fetch_handle,
        authenticated_source.runtime_handle(),
    )?;
    validate_dependency_identity(
        "completion signer-resolver",
        &config.completion_signer_resolver_handle,
        signer_resolver.runtime_handle(),
    )?;
    match probe_runtime_dependencies(
        Arc::clone(&authenticated_source),
        Arc::clone(&signer_resolver),
        Duration::from_millis(config.source_operation_timeout_ms),
        Duration::from_millis(config.signer_timeout_ms),
    )
    .await
    {
        RuntimeDependencyProbeV1::Ready => {}
        RuntimeDependencyProbeV1::NotReady => {
            bail!("SoraFS provider-ingest runtime dependencies are not ready");
        }
        RuntimeDependencyProbeV1::TimedOutOrPanicked => {
            bail!("SoraFS provider-ingest runtime dependency readiness probe failed its deadline");
        }
    }
    let provider_id = node
        .config()
        .provider_id()
        .ok_or_else(|| eyre::eyre!("provider-ingest runtime requires a configured provider id"))?;
    let claim_owner = random_claim_owner()?;
    let probe = Arc::new(Mutex::new(FinalizedSnapshotProbeV1 {
        completed_cursor: None,
    }));
    let ledger = Arc::new(NativeFinalizedAssignmentLedgerV1::new(
        Arc::clone(&state),
        provider_id,
        config.max_snapshot_rows,
        config.max_snapshot_bytes.0,
        Arc::clone(&probe),
    ));
    let fetch = Arc::new(AuthenticatedSourceAdapterV1(Arc::clone(
        &authenticated_source,
    )));
    let storage = Arc::new(NativeProviderIngestLocalStorageV1::new(
        node.clone(),
        Duration::from_millis(config.source_operation_timeout_ms),
    ));
    let payload_builder = Arc::new(NativeCompletionPayloadBuilderV1 {
        chain_id: chain_id.clone(),
        state: Arc::clone(&state),
        queue: Arc::clone(&queue),
        ttl: Duration::from_millis(config.completion_transaction_ttl_ms),
        max_signed_transaction_bytes: config.outbox.max_signed_transaction_bytes.0,
    });
    let resolver = Arc::new(GovernedSignerResolverAdapterV1 {
        resolver: Arc::clone(&signer_resolver),
        state: Arc::clone(&state),
        provider_id,
    });
    let ingress = Arc::new(NativeTransactionIngressV1 {
        chain_id: chain_id.clone(),
        state: Arc::clone(&state),
        queue,
    });
    let clock = Arc::new(ProviderIngestSystemClockV1);
    let policy = ProviderIngestRuntimePolicyV1 {
        max_page_rows: config.max_page_rows,
        max_pages_per_tick: config.max_pages_per_tick,
        max_source_jobs_per_tick: config.max_source_jobs_per_tick,
        max_source_providers: config.max_source_providers,
        scan_interval_ms: config.scan_interval_ms,
        source_operation_timeout_ms: config.source_operation_timeout_ms,
        source_lease_renew_interval_ms: config.source_lease_renew_interval_ms,
        signer_timeout_ms: config.signer_timeout_ms,
        ingress_timeout_ms: config.ingress_timeout_ms,
    };
    let mut runtime = node
        .build_provider_ingest_runtime(
            chain_id,
            claim_owner,
            policy,
            ledger,
            fetch,
            storage,
            payload_builder,
            resolver,
            ingress,
            clock,
        )
        .wrap_err("assemble finalized-ledger provider-ingest runtime")?;
    let handle = ProviderIngestRuntimeHandleV1 {
        node,
        state,
        config: config.clone(),
        probe,
        counters: Arc::new(ProviderIngestDaemonCountersV1::default()),
        worker_running: Arc::new(AtomicBool::new(false)),
        external_dependencies_healthy: Arc::new(AtomicBool::new(false)),
        tick_in_flight: Arc::new(AtomicBool::new(false)),
        last_successful_tick: Arc::new(Mutex::new(None)),
    };
    let worker = handle.clone();
    let shutdown_wait = provider_ingest_shutdown_wait(&config);
    let task = tokio::spawn(async move {
        let _liveness = ProviderIngestWorkerLivenessGuardV1::new(
            Arc::clone(&worker.worker_running),
            Arc::clone(&worker.tick_in_flight),
        );
        let mut interval = tokio::time::interval(Duration::from_millis(config.scan_interval_ms));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    worker.tick_in_flight.store(true, Ordering::Release);
                    worker
                        .external_dependencies_healthy
                        .store(false, Ordering::Release);
                    if validate_dependency_identity(
                        "authenticated source-fetch",
                        &config.authenticated_source_fetch_handle,
                        authenticated_source.runtime_handle(),
                    )
                    .is_err()
                    || validate_dependency_identity(
                        "completion signer-resolver",
                        &config.completion_signer_resolver_handle,
                        signer_resolver.runtime_handle(),
                    )
                    .is_err()
                    {
                        worker
                            .external_dependencies_healthy
                            .store(false, Ordering::Release);
                        iroha_logger::error!(
                            "SoraFS provider-ingest runtime adapter identity changed; stopping supervised worker"
                        );
                        break;
                    }
                    let dependency_probe = probe_runtime_dependencies(
                        Arc::clone(&authenticated_source),
                        Arc::clone(&signer_resolver),
                        Duration::from_millis(config.source_operation_timeout_ms),
                        Duration::from_millis(config.signer_timeout_ms),
                    );
                    tokio::pin!(dependency_probe);
                    let dependency_probe = tokio::select! {
                        probe = &mut dependency_probe => probe,
                        () = shutdown_signal.receive() => {
                            iroha_logger::debug!(
                                "SoraFS provider-ingest runtime is being shut down during dependency probing"
                            );
                            break;
                        }
                    };
                    match dependency_probe {
                        RuntimeDependencyProbeV1::Ready => {}
                        RuntimeDependencyProbeV1::NotReady => {
                            worker
                                .counters
                                .failed_ticks
                                .fetch_add(1, Ordering::Relaxed);
                            worker.tick_in_flight.store(false, Ordering::Release);
                            iroha_logger::warn!(
                                "SoraFS provider-ingest runtime dependency probe failed closed"
                            );
                            continue;
                        }
                        RuntimeDependencyProbeV1::TimedOutOrPanicked => {
                            worker
                                .counters
                                .failed_ticks
                                .fetch_add(1, Ordering::Relaxed);
                            worker.tick_in_flight.store(false, Ordering::Release);
                            iroha_logger::error!(
                                "SoraFS provider-ingest runtime dependency probe exceeded its deadline or panicked; stopping supervised worker"
                            );
                            break;
                        }
                    }
                    worker
                        .external_dependencies_healthy
                        .store(true, Ordering::Release);
                    let shutdown_requested = AtomicBool::new(false);
                    let tick = runtime.tick_with_shutdown(&shutdown_requested);
                    tokio::pin!(tick);
                    let mut stop_after_tick = false;
                    let tick_result = loop {
                        tokio::select! {
                            result = &mut tick => break result,
                            () = shutdown_signal.receive(), if !stop_after_tick => {
                                shutdown_requested.store(true, Ordering::Release);
                                worker
                                    .external_dependencies_healthy
                                    .store(false, Ordering::Release);
                                stop_after_tick = true;
                            }
                        }
                    };
                    match tick_result {
                        Ok(outcome) => {
                            record_tick_outcome(&worker.counters, outcome);
                            if stop_after_tick {
                                worker.tick_in_flight.store(false, Ordering::Release);
                                iroha_logger::debug!(
                                    "SoraFS provider-ingest runtime drained its active row for shutdown"
                                );
                                break;
                            }
                            worker
                                .counters
                                .successful_ticks
                                .fetch_add(1, Ordering::Relaxed);
                            if let Ok(mut last_tick) = worker.last_successful_tick.lock() {
                                *last_tick = Some(Instant::now());
                            } else {
                                worker.tick_in_flight.store(false, Ordering::Release);
                                iroha_logger::error!(
                                    "SoraFS provider-ingest freshness state is poisoned; stopping supervised worker"
                                );
                                break;
                            }
                        }
                        Err(error) => {
                            worker
                                .counters
                                .failed_ticks
                                .fetch_add(1, Ordering::Relaxed);
                            worker
                                .external_dependencies_healthy
                                .store(false, Ordering::Release);
                            worker.tick_in_flight.store(false, Ordering::Release);
                            iroha_logger::error!(
                                error = %error,
                                "SoraFS provider-ingest reconciliation failed; stopping supervised worker"
                            );
                            break;
                        }
                    }
                    worker.tick_in_flight.store(false, Ordering::Release);
                }
                () = shutdown_signal.receive() => {
                    worker.tick_in_flight.store(false, Ordering::Release);
                    iroha_logger::debug!(
                        "SoraFS provider-ingest runtime is being shut down"
                    );
                    break;
                }
                else => break,
            }
        }
    });
    Ok((handle, Child::new(task, OnShutdown::Wait(shutdown_wait))))
}

struct ProviderIngestWorkerLivenessGuardV1 {
    worker_running: Arc<AtomicBool>,
    tick_in_flight: Arc<AtomicBool>,
}

impl ProviderIngestWorkerLivenessGuardV1 {
    fn new(worker_running: Arc<AtomicBool>, tick_in_flight: Arc<AtomicBool>) -> Self {
        worker_running.store(true, Ordering::Release);
        Self {
            worker_running,
            tick_in_flight,
        }
    }
}

impl Drop for ProviderIngestWorkerLivenessGuardV1 {
    fn drop(&mut self) {
        self.tick_in_flight.store(false, Ordering::Release);
        self.worker_running.store(false, Ordering::Release);
    }
}

fn record_tick_outcome(
    counters: &ProviderIngestDaemonCountersV1,
    outcome: ProviderIngestTickOutcomeV1,
) {
    counters.rows_scanned.fetch_add(
        u64::try_from(outcome.rows_scanned).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
    counters.jobs_inserted.fetch_add(
        u64::try_from(outcome.jobs_inserted).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
    counters.jobs_finalized.fetch_add(
        u64::try_from(outcome.jobs_finalized).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
    counters.jobs_cancelled.fetch_add(
        u64::try_from(outcome.jobs_cancelled).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
    counters.source_jobs_claimed.fetch_add(
        u64::try_from(outcome.source_jobs_claimed).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
    counters.manifests_stored.fetch_add(
        u64::try_from(outcome.manifests_stored).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
    counters.completions_signed.fetch_add(
        u64::try_from(outcome.completions_signed).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
    counters.completion_submissions.fetch_add(
        u64::try_from(outcome.completion_submissions).unwrap_or(u64::MAX),
        Ordering::Relaxed,
    );
}

fn random_claim_owner() -> Result<ProviderIngestClaimOwnerV1> {
    for _ in 0..8 {
        let mut bytes = [0_u8; 32];
        OsRng
            .try_fill_bytes(&mut bytes)
            .wrap_err("operating-system randomness unavailable for provider-ingest claim owner")?;
        if let Ok(owner) = ProviderIngestClaimOwnerV1::new(bytes) {
            return Ok(owner);
        }
    }
    bail!("operating-system randomness repeatedly returned a zero provider-ingest claim owner")
}

fn validate_config(config: &SorafsProviderIngestRuntime) -> Result<()> {
    if !is_production_handle(&config.authenticated_source_fetch_handle)
        || !is_production_handle(&config.completion_signer_resolver_handle)
        || config.scan_interval_ms == 0
        || config.max_page_rows == 0
        || config.max_pages_per_tick == 0
        || config.max_source_jobs_per_tick == 0
        || config.max_source_providers == 0
        || config.source_operation_timeout_ms == 0
        || config.source_lease_renew_interval_ms == 0
        || config.signer_timeout_ms == 0
        || config.ingress_timeout_ms == 0
        || config.completion_transaction_ttl_ms == 0
        || config.max_snapshot_rows == 0
        || config.max_snapshot_bytes.0 == 0
        || config.max_finalized_lag_blocks == 0
        || config.max_page_rows > config.max_snapshot_rows
        || config.source_lease_renew_interval_ms >= config.outbox.source_lease_ttl_ms
        || config.outbox.max_signed_transaction_bytes.0
            <= SIGNED_TRANSACTION_ENVELOPE_RESERVE_BYTES_V1
        || config
            .max_page_rows
            .checked_mul(config.max_pages_per_tick)
            .is_none()
    {
        bail!("SoraFS provider-ingest runtime configuration is invalid");
    }
    let policy = sorafs_node::ProviderIngestOutboxPolicyV1 {
        max_active_entries: config.outbox.max_active_entries,
        max_terminal_entries: config.outbox.max_terminal_entries,
        max_attempts: config.outbox.max_attempts,
        checkpoint_max_bytes: config.outbox.checkpoint_max_bytes.0,
        source_lease_ttl_ms: config.outbox.source_lease_ttl_ms,
        retry_base_delay_ms: config.outbox.retry_base_delay_ms,
        retry_max_delay_ms: config.outbox.retry_max_delay_ms,
        terminal_retention_blocks: config.outbox.terminal_retention_blocks,
        max_signed_transaction_bytes: config.outbox.max_signed_transaction_bytes.0,
        max_status_page_size: config.outbox.max_status_page_size,
    };
    policy
        .validate()
        .wrap_err("validate provider-ingest durable outbox policy")?;
    Ok(())
}

fn validate_dependency_identity(label: &str, expected: &str, actual: &str) -> Result<()> {
    if !is_production_handle(actual) || actual != expected {
        bail!("{label} adapter identity does not match SoraFS provider-ingest configuration");
    }
    Ok(())
}

fn is_production_handle(handle: &str) -> bool {
    if handle.is_empty()
        || handle.len() > 256
        || !handle.is_ascii()
        || handle
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return false;
    }
    let lowercase = handle.to_ascii_lowercase();
    !lowercase
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|component| {
            matches!(
                component,
                "null" | "mock" | "test" | "dev" | "fake" | "placeholder"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_handle_validation_rejects_placeholders_and_whitespace() {
        for handle in [
            "",
            "pkcs11 test",
            "source-mock-primary",
            "fake",
            "kms-placeholder",
            "source\nprimary",
        ] {
            assert!(!is_production_handle(handle), "{handle:?}");
        }
        assert!(is_production_handle(
            "pkcs11:sorafs-provider-ingest-primary"
        ));
        assert!(is_production_handle("https-pinned-source-pool:eu-1"));
    }

    #[test]
    fn dependency_identity_rejects_runtime_substitution() {
        assert!(validate_dependency_identity("source", "source:eu-1", "source:eu-2").is_err());
        assert!(validate_dependency_identity("source", "source:eu-1", "source:eu-1").is_ok());
    }

    #[test]
    fn worker_liveness_guard_fails_readiness_closed_on_every_exit() {
        let running = Arc::new(AtomicBool::new(false));
        let in_flight = Arc::new(AtomicBool::new(true));
        {
            let _guard =
                ProviderIngestWorkerLivenessGuardV1::new(running.clone(), in_flight.clone());
            assert!(running.load(Ordering::Acquire));
            assert!(in_flight.load(Ordering::Acquire));
        }
        assert!(!running.load(Ordering::Acquire));
        assert!(!in_flight.load(Ordering::Acquire));
    }

    #[test]
    fn completed_cursor_consistency_rejects_historical_and_head_forks() {
        let committed_hashes = (1_u8..=10)
            .map(|byte| HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([byte; 32])))
            .collect::<Vec<_>>();
        let cursor_hash = *committed_hashes[8].as_ref();
        let head_hash = *committed_hashes[9].as_ref();
        let cursor = ProviderIngestFinalizedCursorV1 {
            height: 9,
            block_hash: cursor_hash,
        };
        assert!(!completed_cursor_matches_committed_chain(
            None,
            10,
            head_hash,
            &committed_hashes
        ));
        assert!(completed_cursor_matches_committed_chain(
            Some(cursor),
            10,
            head_hash,
            &committed_hashes
        ));
        assert!(!completed_cursor_matches_committed_chain(
            Some(ProviderIngestFinalizedCursorV1 {
                height: 9,
                block_hash: [0xA9; 32],
            }),
            10,
            head_hash,
            &committed_hashes
        ));
        assert!(!completed_cursor_matches_committed_chain(
            Some(cursor),
            10,
            [0xAA; 32],
            &committed_hashes
        ));
        assert!(!completed_cursor_matches_committed_chain(
            Some(cursor),
            9,
            cursor_hash,
            &committed_hashes
        ));
    }

    #[test]
    fn unrelated_orders_exhaust_scan_budget_before_provider_filtering() {
        let mut rows = 0;
        let mut bytes = 0;
        assert!(charge_snapshot_scan_budget(&mut rows, &mut bytes, 16, 8, 0, 2, 4_096).is_ok());
        assert!(charge_snapshot_scan_budget(&mut rows, &mut bytes, 16, 8, 0, 2, 4_096).is_ok());
        assert_eq!(rows, 2);
        assert!(charge_snapshot_scan_budget(&mut rows, &mut bytes, 1, 1, 0, 2, 4_096).is_err());
        assert_eq!(rows, 2);
    }

    #[test]
    fn cancelling_store_wait_joins_late_writer() {
        let completed = Arc::new(AtomicBool::new(false));
        let late = Arc::clone(&completed);
        let thread = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            late.store(true, Ordering::Release);
        });
        drop(BlockingStoreJoinGuardV1(Some(thread)));
        assert!(completed.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn hung_readiness_probe_fails_at_explicit_deadline() {
        let result = bounded_blocking_readiness_probe(Duration::from_millis(1), || {
            std::thread::sleep(Duration::from_millis(25));
            true
        })
        .await;
        assert_eq!(result, RuntimeDependencyProbeV1::TimedOutOrPanicked);
    }
}
