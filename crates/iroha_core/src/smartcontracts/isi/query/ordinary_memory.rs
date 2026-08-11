//! Server-owned memory admission for ordinary Torii queries.
//!
//! This module is deliberately opt-in. IVM and other in-process callers keep
//! the existing query behavior unless they attach [`OrdinaryQueryExecutionLimits`]
//! to [`super::QueryLimits`]. The initial Torii corridor admits only output
//! shapes whose source rows have a protocol-level resident bound before the
//! query implementation can clone them.

use std::{
    fmt,
    sync::{Arc, Mutex},
};

#[cfg(feature = "fast_dsl")]
use std::sync::OnceLock;

#[cfg(feature = "fast_dsl")]
use iroha_data_model::prelude::SelectorTuple;
use iroha_data_model::{
    prelude::{Identifiable as _, QueryParams, RoleId, TriggerId},
    query::{QueryRequest, QueryResponse, SingularQueryBox, error::QueryExecutionFail as Error},
};
use mv::storage::StorageReadOnly as _;
use norito::core::{DecodeFlagsGuard, NoritoSerialize};

use super::{QueryCountMode, QueryExecutionBudget, QueryLimits, STREAMING_SORTED_PREFIX_LIMIT};
use crate::state::{StateReadOnly, WorldReadOnly};

/// Conservative resident charge for one name-backed identifier source row.
///
/// `Name` is protocol-limited to 255 bytes. The additional allowance covers
/// the owned string, the identifier wrapper, and allocator bookkeeping before
/// post-processing can measure the exact encoded row.
pub const ORDINARY_NAME_ID_SOURCE_BYTES: u64 = 1_024;

/// Conservative resident charge for the fixed-width ABI-version result.
pub const ORDINARY_ABI_VERSION_SOURCE_BYTES: u64 = 64;

/// Fixed resident charge for query/cursor containers, allocator metadata, and
/// move-only ownership tokens that do not scale with the result count.
pub const ORDINARY_QUERY_FIXED_CONTAINER_OVERHEAD_BYTES: u64 = 4 * 1_024;

/// Conservative resident charge for each slot in a page or retained cursor.
///
/// This is deliberately separate from the source-value charge. It covers the
/// `Vec` slot, enum/tuple wrappers, iterator bookkeeping, and allocator
/// metadata that remain live alongside the value itself.
pub const ORDINARY_QUERY_RETAINED_ITEM_OVERHEAD_BYTES: u64 = 128;

/// Fixed failure categories for invalid ordinary-query memory geometry.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum OrdinaryQueryExecutionLimitError {
    /// A page cannot contain zero items because bounded scans need an `F + 1`
    /// continuation probe.
    ZeroPageItems,
    /// A checked geometry calculation overflowed.
    GeometryOverflow,
    /// The deterministic execution budget cannot cover one full page plus its
    /// continuation probe at the configured source-row charge.
    ExecutionBudgetTooSmall,
    /// The configured peak reservation does not cover source, page, response,
    /// and encoding overlap.
    ExecutionHeadroomTooSmall,
    /// The configured cursor reservation does not cover retained values, the
    /// archived Start request, and deterministic container overhead.
    CursorRetentionTooSmall,
}

impl fmt::Display for OrdinaryQueryExecutionLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::ZeroPageItems => "ordinary query page size must be non-zero",
            Self::GeometryOverflow => "ordinary query memory geometry overflowed",
            Self::ExecutionBudgetTooSmall => {
                "ordinary query execution budget cannot cover one page plus probe"
            }
            Self::ExecutionHeadroomTooSmall => {
                "ordinary query execution headroom is below the required phase envelope"
            }
            Self::CursorRetentionTooSmall => {
                "ordinary query cursor reservation is below the required retained envelope"
            }
        };
        f.write_str(message)
    }
}

impl std::error::Error for OrdinaryQueryExecutionLimitError {}

/// A weighted reservation owned by the embedding server.
///
/// Core never assumes how the reservation is implemented. Torii may back it
/// with a weighted byte pool, while tests may use a counter. Splitting must be
/// allocation-accounting neutral: the returned reservation owns `bytes`, the
/// receiver owns that many fewer bytes, and the aggregate reserved weight must
/// not change until either reservation is dropped.
pub trait OrdinaryQueryMemoryReservation: fmt::Debug + Send + Sync + 'static {
    /// Number of aggregate pool bytes represented by this reservation.
    fn reserved_bytes(&self) -> u64;

    /// Generation of the embedding server's weighted memory pool.
    ///
    /// Pool replacement or reconfiguration must advance this value. Every
    /// child returned by [`Self::split_off`] must report the same generation.
    fn pool_generation(&self) -> u64;

    /// Transfer `bytes` from this reservation into a new independently
    /// releasable reservation.
    ///
    /// Returning `None` must leave this reservation unchanged.
    fn split_off(&mut self, bytes: u64) -> Option<Box<dyn OrdinaryQueryMemoryReservation>>;
}

/// Move-only semantic ownership token for ordinary-query resident memory.
///
/// A token is moved through worker execution and response encoding. Stored
/// cursors own a split token for their complete lifetime; response bodies own
/// the remaining headroom until the last slow-body reference is dropped.
pub struct OrdinaryQueryMemoryLease {
    reservation: Box<dyn OrdinaryQueryMemoryReservation>,
}

impl OrdinaryQueryMemoryLease {
    /// Wrap an embedding-server weighted reservation.
    pub fn new(reservation: impl OrdinaryQueryMemoryReservation) -> Self {
        Self {
            reservation: Box::new(reservation),
        }
    }

    /// Return the aggregate pool weight owned by this token.
    #[must_use]
    pub fn reserved_bytes(&self) -> u64 {
        self.reservation.reserved_bytes()
    }

    /// Return the weighted-pool generation that owns this token.
    #[must_use]
    pub fn pool_generation(&self) -> u64 {
        self.reservation.pool_generation()
    }

    /// Split an independently releasable child token from this token.
    pub(crate) fn split_off(&mut self, bytes: u64) -> Option<Self> {
        if bytes == 0 {
            return None;
        }
        let pool_generation = self.pool_generation();
        let reservation = self.reservation.split_off(bytes)?;
        if reservation.pool_generation() != pool_generation {
            drop(reservation);
            return None;
        }
        Some(Self { reservation })
    }
}

impl fmt::Debug for OrdinaryQueryMemoryLease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OrdinaryQueryMemoryLease")
            .field("reserved_bytes", &self.reserved_bytes())
            .field("pool_generation", &self.pool_generation())
            .finish_non_exhaustive()
    }
}

/// Server-owned limits for one ordinary Torii query execution.
///
/// Encoded-byte ceilings are deterministic codec work limits, not estimates
/// of Rust heap usage. The embedding server's weighted reservation separately
/// covers the conservative resident phase envelope.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct OrdinaryQueryExecutionLimits {
    policy_generation: u64,
    execution_budget: QueryExecutionBudget,
    max_page_items: u64,
    execution_headroom_bytes: u64,
    max_source_item_bytes: u64,
    max_response_bytes: u64,
    max_cursor_retained_items: u64,
    max_cursor_value_bytes: u64,
    max_cursor_retained_bytes: u64,
    max_revalidation_archive_bytes: u64,
    revalidation_decode_limits: norito::DecodeLimits,
}

impl OrdinaryQueryExecutionLimits {
    /// Construct and validate the complete Core-side limit set.
    ///
    /// `execution_headroom_bytes` and `max_cursor_retained_bytes` are accepted
    /// reservations, not independent tuning knobs: construction fails unless
    /// they cover the checked phase envelopes returned by
    /// [`Self::required_execution_headroom_bytes`] and
    /// [`Self::required_cursor_retained_bytes`].
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        policy_generation: u64,
        execution_budget: QueryExecutionBudget,
        max_page_items: u64,
        execution_headroom_bytes: u64,
        max_source_item_bytes: u64,
        max_response_bytes: u64,
        max_cursor_retained_items: u64,
        max_cursor_value_bytes: u64,
        max_cursor_retained_bytes: u64,
        max_revalidation_archive_bytes: u64,
        revalidation_decode_limits: norito::DecodeLimits,
    ) -> Result<Self, OrdinaryQueryExecutionLimitError> {
        if max_page_items == 0 {
            return Err(OrdinaryQueryExecutionLimitError::ZeroPageItems);
        }
        let page_with_probe = max_page_items
            .checked_add(1)
            .ok_or(OrdinaryQueryExecutionLimitError::GeometryOverflow)?;
        let probe_source_bytes = page_with_probe
            .checked_mul(max_source_item_bytes)
            .ok_or(OrdinaryQueryExecutionLimitError::GeometryOverflow)?;
        execution_budget
            .ensure(page_with_probe, probe_source_bytes)
            .map_err(|_| OrdinaryQueryExecutionLimitError::ExecutionBudgetTooSmall)?;

        let required_execution = Self::required_execution_headroom_bytes(
            max_page_items,
            max_source_item_bytes,
            max_response_bytes,
            max_revalidation_archive_bytes,
            revalidation_decode_limits,
        )?;
        if execution_headroom_bytes < required_execution {
            return Err(OrdinaryQueryExecutionLimitError::ExecutionHeadroomTooSmall);
        }
        let required_cursor = Self::required_cursor_retained_bytes(
            max_cursor_retained_items,
            max_source_item_bytes,
            max_cursor_value_bytes,
            max_revalidation_archive_bytes,
        )?;
        if max_cursor_retained_bytes < required_cursor {
            return Err(OrdinaryQueryExecutionLimitError::CursorRetentionTooSmall);
        }

        Ok(Self {
            policy_generation,
            execution_budget,
            max_page_items,
            execution_headroom_bytes,
            max_source_item_bytes,
            max_response_bytes,
            max_cursor_retained_items,
            max_cursor_value_bytes,
            max_cursor_retained_bytes,
            max_revalidation_archive_bytes,
            revalidation_decode_limits,
        })
    }

    /// Compute the minimum peak reservation `P` for source/work, page
    /// materialization, response ownership, and encoding overlap.
    pub fn required_execution_headroom_bytes(
        max_page_items: u64,
        max_source_item_bytes: u64,
        max_response_bytes: u64,
        max_revalidation_archive_bytes: u64,
        revalidation_decode_limits: norito::DecodeLimits,
    ) -> Result<u64, OrdinaryQueryExecutionLimitError> {
        let page_with_probe = max_page_items
            .checked_add(1)
            .ok_or(OrdinaryQueryExecutionLimitError::GeometryOverflow)?;
        let source_work = page_with_probe
            .checked_mul(max_source_item_bytes)
            .ok_or(OrdinaryQueryExecutionLimitError::GeometryOverflow)?;
        let owned_page_values = max_page_items
            .checked_mul(max_source_item_bytes)
            .ok_or(OrdinaryQueryExecutionLimitError::GeometryOverflow)?;
        let page_container = max_page_items
            .checked_mul(ORDINARY_QUERY_RETAINED_ITEM_OVERHEAD_BYTES)
            .and_then(|bytes| bytes.checked_add(ORDINARY_QUERY_FIXED_CONTAINER_OVERHEAD_BYTES))
            .ok_or(OrdinaryQueryExecutionLimitError::GeometryOverflow)?;
        let execution_phase = source_work
            .checked_add(owned_page_values)
            .and_then(|bytes| bytes.checked_add(page_container))
            .and_then(|bytes| bytes.checked_add(max_response_bytes))
            .ok_or(OrdinaryQueryExecutionLimitError::GeometryOverflow)?;

        // On `Continue`, the retained archive remains charged to `R` while a
        // decoded Start request and its bounded canonical re-encode coexist in
        // fresh headroom `P`. Revalidation completes before source execution,
        // so the required peak is the larger phase rather than their sum.
        let decoded_archive_bytes =
            u64::try_from(revalidation_decode_limits.max_total_allocated_bytes())
                .map_err(|_| OrdinaryQueryExecutionLimitError::GeometryOverflow)?;
        let revalidation_phase = max_revalidation_archive_bytes
            .checked_add(decoded_archive_bytes)
            .and_then(|bytes| bytes.checked_add(ORDINARY_QUERY_FIXED_CONTAINER_OVERHEAD_BYTES))
            .ok_or(OrdinaryQueryExecutionLimitError::GeometryOverflow)?;
        Ok(execution_phase.max(revalidation_phase))
    }

    /// Compute the minimum retained reservation `R` for cursor values, the
    /// canonical Start archive, and deterministic container overhead.
    pub fn required_cursor_retained_bytes(
        max_cursor_retained_items: u64,
        max_source_item_bytes: u64,
        max_cursor_value_bytes: u64,
        max_revalidation_archive_bytes: u64,
    ) -> Result<u64, OrdinaryQueryExecutionLimitError> {
        let resident_values = max_cursor_retained_items
            .checked_mul(max_source_item_bytes)
            .ok_or(OrdinaryQueryExecutionLimitError::GeometryOverflow)?;
        let retained_value_envelope = resident_values.max(max_cursor_value_bytes);
        let container = max_cursor_retained_items
            .checked_mul(ORDINARY_QUERY_RETAINED_ITEM_OVERHEAD_BYTES)
            .and_then(|bytes| bytes.checked_add(ORDINARY_QUERY_FIXED_CONTAINER_OVERHEAD_BYTES))
            .ok_or(OrdinaryQueryExecutionLimitError::GeometryOverflow)?;
        retained_value_envelope
            .checked_add(max_revalidation_archive_bytes)
            .and_then(|bytes| bytes.checked_add(container))
            .ok_or(OrdinaryQueryExecutionLimitError::GeometryOverflow)
    }

    /// Configuration generation that produced this policy.
    #[must_use]
    pub const fn policy_generation(self) -> u64 {
        self.policy_generation
    }

    /// Deterministic work budget applied while producing an ephemeral page.
    #[must_use]
    pub const fn execution_budget(self) -> QueryExecutionBudget {
        self.execution_budget
    }

    /// Maximum response-page item count covered by the peak geometry.
    #[must_use]
    pub const fn max_page_items(self) -> u64 {
        self.max_page_items
    }

    /// Weighted resident bytes required in addition to any stored cursor.
    #[must_use]
    pub const fn execution_headroom_bytes(self) -> u64 {
        self.execution_headroom_bytes
    }

    /// Maximum resident bytes allowed for one source row before exact sizing.
    #[must_use]
    pub const fn max_source_item_bytes(self) -> u64 {
        self.max_source_item_bytes
    }

    /// Maximum complete framed response bytes accepted before Torii encoding.
    #[must_use]
    pub const fn max_response_bytes(self) -> u64 {
        self.max_response_bytes
    }

    /// Maximum values retained by one stored cursor.
    #[must_use]
    pub const fn max_cursor_retained_items(self) -> u64 {
        self.max_cursor_retained_items
    }

    /// Maximum encoded bytes occupied by retained cursor values.
    #[must_use]
    pub const fn max_cursor_value_bytes(self) -> u64 {
        self.max_cursor_value_bytes
    }

    /// Aggregate weighted charge split into a stored cursor token.
    #[must_use]
    pub const fn max_cursor_retained_bytes(self) -> u64 {
        self.max_cursor_retained_bytes
    }

    /// Maximum canonical request archive retained for continuation revalidation.
    #[must_use]
    pub const fn max_revalidation_archive_bytes(self) -> u64 {
        self.max_revalidation_archive_bytes
    }

    /// Schema-audited limits for decoding a stored Start archive during
    /// continuation revalidation.
    #[must_use]
    pub const fn revalidation_decode_limits(self) -> norito::DecodeLimits {
        self.revalidation_decode_limits
    }
}

/// Immutable policy identity archived alongside one ordinary stored cursor.
///
/// Exact equality is intentional. A continuation may not combine retained
/// memory admitted under an old configuration with execution headroom from a
/// different policy or weighted-pool generation, even when the new values look
/// individually wider.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) struct OrdinaryQueryCursorPolicy {
    limits: OrdinaryQueryExecutionLimits,
    max_fetch_size: u64,
    count_mode: QueryCountMode,
    pool_generation: u64,
}

impl OrdinaryQueryCursorPolicy {
    pub(crate) const fn new(
        limits: OrdinaryQueryExecutionLimits,
        max_fetch_size: u64,
        count_mode: QueryCountMode,
        pool_generation: u64,
    ) -> Self {
        Self {
            limits,
            max_fetch_size,
            count_mode,
            pool_generation,
        }
    }

    pub(crate) const fn pool_generation(self) -> u64 {
        self.pool_generation
    }

    pub(crate) const fn retained_bytes(self) -> u64 {
        self.limits.max_cursor_retained_bytes()
    }
}

/// Move-only retained cursor memory and the exact policy that admitted it.
#[derive(Debug)]
pub(crate) struct OrdinaryQueryCursorMemory {
    lease: OrdinaryQueryMemoryLease,
    policy: OrdinaryQueryCursorPolicy,
}

impl OrdinaryQueryCursorMemory {
    fn new(lease: OrdinaryQueryMemoryLease, policy: OrdinaryQueryCursorPolicy) -> Option<Self> {
        if lease.pool_generation() != policy.pool_generation()
            || lease.reserved_bytes() < policy.retained_bytes()
        {
            return None;
        }
        Some(Self { lease, policy })
    }

    pub(crate) fn binding(&self) -> OrdinaryQueryCursorBinding {
        OrdinaryQueryCursorBinding {
            retained_bytes: self.lease.reserved_bytes(),
            policy: self.policy,
        }
    }
}

/// Copyable continuation admission facts returned without leaking a map guard.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) struct OrdinaryQueryCursorBinding {
    retained_bytes: u64,
    policy: OrdinaryQueryCursorPolicy,
}

impl OrdinaryQueryCursorBinding {
    pub(crate) const fn retained_bytes(self) -> u64 {
        self.retained_bytes
    }

    pub(crate) const fn policy(self) -> OrdinaryQueryCursorPolicy {
        self.policy
    }

    pub(crate) fn is_compatible_with(self, current: OrdinaryQueryCursorPolicy) -> bool {
        self.policy == current && self.retained_bytes >= current.retained_bytes()
    }
}

/// Shared mutable ownership used only while a Start request may split a cursor
/// reservation from its admitted peak reservation.
///
/// The mutex is never held while waiting for capacity or while executing a
/// query. It protects one synchronous `split_off`/`take` handoff because
/// [`crate::query::store::LiveQueryStoreHandle`] methods accept `&self`.
#[derive(Clone, Debug)]
pub(crate) struct OrdinaryQueryMemoryAdmission {
    state: Arc<Mutex<OrdinaryQueryMemoryAdmissionState>>,
    cursor_retained_bytes: u64,
    cursor_policy: Option<OrdinaryQueryCursorPolicy>,
}

#[derive(Debug)]
struct OrdinaryQueryMemoryAdmissionState {
    lease: Option<OrdinaryQueryMemoryLease>,
    cursor_split: bool,
}

impl OrdinaryQueryMemoryAdmission {
    pub(crate) fn new(
        lease: OrdinaryQueryMemoryLease,
        cursor_retained_bytes: u64,
        cursor_policy: Option<OrdinaryQueryCursorPolicy>,
    ) -> Result<Self, Error> {
        let cursor_policy_invalid = match cursor_policy {
            Some(policy) => {
                cursor_retained_bytes == 0
                    || policy.retained_bytes() != cursor_retained_bytes
                    || policy.pool_generation() != lease.pool_generation()
            }
            None => cursor_retained_bytes != 0,
        };
        if lease.reserved_bytes() < cursor_retained_bytes || cursor_policy_invalid {
            return Err(Error::CapacityLimit);
        }
        Ok(Self {
            state: Arc::new(Mutex::new(OrdinaryQueryMemoryAdmissionState {
                lease: Some(lease),
                cursor_split: false,
            })),
            cursor_retained_bytes,
            cursor_policy,
        })
    }

    pub(crate) fn split_cursor_lease(&self) -> Result<OrdinaryQueryCursorMemory, Error> {
        let mut state = self.state.lock().map_err(|_| Error::CapacityLimit)?;
        if state.cursor_split {
            return Err(Error::CapacityLimit);
        }
        let cursor_lease = state
            .lease
            .as_mut()
            .and_then(|lease| lease.split_off(self.cursor_retained_bytes))
            .ok_or(Error::CapacityLimit)?;
        let policy = self.cursor_policy.ok_or(Error::CapacityLimit)?;
        let cursor_memory =
            OrdinaryQueryCursorMemory::new(cursor_lease, policy).ok_or(Error::CapacityLimit)?;
        state.cursor_split = true;
        Ok(cursor_memory)
    }

    pub(crate) fn take_response_lease(
        &self,
        release_unused_cursor_charge: bool,
    ) -> Result<OrdinaryQueryMemoryLease, Error> {
        let mut state = self.state.lock().map_err(|_| Error::CapacityLimit)?;
        if release_unused_cursor_charge && state.cursor_split {
            return Err(Error::CapacityLimit);
        }
        let mut lease = state.lease.take().ok_or(Error::CapacityLimit)?;
        if release_unused_cursor_charge && self.cursor_retained_bytes != 0 {
            drop(
                lease
                    .split_off(self.cursor_retained_bytes)
                    .ok_or(Error::CapacityLimit)?,
            );
        }
        Ok(lease)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum OrdinaryCursorMode {
    Ephemeral,
    Stored,
}

pub(super) fn ensure_request_admitted(
    request: &QueryRequest,
    mode: OrdinaryCursorMode,
    query_limits: QueryLimits,
    limits: OrdinaryQueryExecutionLimits,
) -> Result<(), Error> {
    match request {
        QueryRequest::Singular(SingularQueryBox::FindAbiVersion(_)) => {
            ensure_source_bound(limits, ORDINARY_ABI_VERSION_SOURCE_BYTES)
        }
        QueryRequest::Singular(_) => {
            // TODO: Add borrowed pre-clone adapters for state-backed singular
            // entities and bespoke bounded builders for synthesized/vector
            // outputs before admitting more variants here.
            Err(Error::Conversion(
                "ordinary Torii query rejects an unadapted singular result before query execution"
                    .to_owned(),
            ))
        }
        QueryRequest::Start(start) => {
            ensure_identifier_start_shape(start, mode, query_limits, limits)
        }
        QueryRequest::Continue(_) if mode == OrdinaryCursorMode::Stored => Ok(()),
        QueryRequest::Continue(_) => Err(Error::Conversion(
            "ordinary ephemeral query rejects continuation before cursor execution".to_owned(),
        )),
    }
}

pub(crate) fn ensure_stored_revalidation_admitted(
    request: &QueryRequest,
    query_limits: QueryLimits,
    limits: OrdinaryQueryExecutionLimits,
) -> Result<(), Error> {
    ensure_request_admitted(request, OrdinaryCursorMode::Stored, query_limits, limits)
}

pub(super) fn ensure_response_admitted(
    response: &QueryResponse,
    limits: OrdinaryQueryExecutionLimits,
) -> Result<(), Error> {
    super::bounded_framed_encoded_len(response, limits.max_response_bytes()).map(drop)
}

fn ensure_source_bound(
    limits: OrdinaryQueryExecutionLimits,
    required_bytes: u64,
) -> Result<(), Error> {
    if limits.max_source_item_bytes() < required_bytes {
        return Err(Error::CapacityLimit);
    }
    Ok(())
}

fn ensure_identifier_start_shape(
    start: &iroha_data_model::query::QueryWithParams,
    mode: OrdinaryCursorMode,
    query_limits: QueryLimits,
    limits: OrdinaryQueryExecutionLimits,
) -> Result<(), Error> {
    ensure_source_bound(limits, ORDINARY_NAME_ID_SOURCE_BYTES)?;

    let identity_protocol_bounded = if let Some(query_box) = start.query_box() {
        non_fast_identifier_shape(query_box)
    } else {
        fast_identifier_shape(start)
    };
    if !identity_protocol_bounded {
        // TODO: Add query-specific borrowed source adapters. Item-kind-only
        // admission is unsound because `AccountId`, entities, blocks, and
        // transactions can own attacker-sized nested values before metering.
        return Err(Error::Conversion(
            "ordinary Torii query rejects an unadapted iterable source before payload decoding or query execution"
                .to_owned(),
        ));
    }

    ensure_identifier_params(&start.params, mode, query_limits, limits)
}

fn non_fast_identifier_shape(
    query_box: &iroha_data_model::query::QueryBox<iroha_data_model::query::QueryOutputBatchBox>,
) -> bool {
    macro_rules! admitted {
        ($item:ty) => {
            iroha_data_model::query::iter_query_inner::<$item>(query_box).is_some_and(|erased| {
                erased.payload().is_empty()
                    && erased.predicate().is_pass()
                    && erased.selector().iter().next().is_none()
            })
        };
    }
    admitted!(RoleId) || admitted!(TriggerId)
}

#[cfg(feature = "fast_dsl")]
fn fast_identifier_shape(start: &iroha_data_model::query::QueryWithParams) -> bool {
    use iroha_data_model::query::{QueryItemKind, dsl::CompoundPredicate};
    use norito::codec::Encode as _;

    fn role_predicate() -> &'static [u8] {
        static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
        BYTES
            .get_or_init(|| CompoundPredicate::<RoleId>::PASS.encode())
            .as_slice()
    }

    fn role_selector() -> &'static [u8] {
        static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
        BYTES
            .get_or_init(|| SelectorTuple::<RoleId>::default().encode())
            .as_slice()
    }

    fn trigger_predicate() -> &'static [u8] {
        static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
        BYTES
            .get_or_init(|| CompoundPredicate::<TriggerId>::PASS.encode())
            .as_slice()
    }

    fn trigger_selector() -> &'static [u8] {
        static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
        BYTES
            .get_or_init(|| SelectorTuple::<TriggerId>::default().encode())
            .as_slice()
    }

    let Some((item, predicate, selector, payload)) = start.fast_dsl_parts() else {
        return false;
    };
    if !payload.is_empty() {
        return false;
    }
    match item {
        QueryItemKind::RoleId => predicate == role_predicate() && selector == role_selector(),
        QueryItemKind::TriggerId => {
            predicate == trigger_predicate() && selector == trigger_selector()
        }
        _ => false,
    }
}

#[cfg(not(feature = "fast_dsl"))]
fn fast_identifier_shape(_start: &iroha_data_model::query::QueryWithParams) -> bool {
    false
}

fn ensure_identifier_params(
    params: &QueryParams,
    mode: OrdinaryCursorMode,
    query_limits: QueryLimits,
    limits: OrdinaryQueryExecutionLimits,
) -> Result<(), Error> {
    let fetch_size = params
        .fetch_size
        .fetch_size
        .unwrap_or(iroha_data_model::query::parameters::DEFAULT_FETCH_SIZE)
        .get();
    if fetch_size > limits.max_page_items() || fetch_size > limits.execution_budget().max_items() {
        return Err(Error::CapacityLimit);
    }

    if mode == OrdinaryCursorMode::Stored {
        if params.sorting.sort_by_metadata_key.is_some() {
            // TODO: Replace the legacy stored-sorted overflow tail with a
            // snapshot-consistent bounded replay/top-K continuation.
            return Err(Error::Conversion(
                "ordinary stored query rejects sorting before source execution".to_owned(),
            ));
        }
        let offset = params.pagination.offset_value();
        let (scanned_items, retained_items) = match query_limits.count_mode {
            QueryCountMode::Exact => {
                let Some(limit) = params.pagination.limit_value() else {
                    return Err(Error::Conversion(
                        "ordinary stored exact-count query requires an explicit bounded limit"
                            .to_owned(),
                    ));
                };
                let items = limit.get();
                let scanned_items = offset.checked_add(items).ok_or(Error::CapacityLimit)?;
                (scanned_items, items)
            }
            QueryCountMode::Bounded => {
                let requested = params.pagination.limit_value().map(|limit| limit.get());
                let first_page_items = requested.map_or(fetch_size, |limit| limit.min(fetch_size));
                let requested_tail = requested
                    .map(|limit| limit - first_page_items)
                    .unwrap_or(u64::MAX);
                let retained_items = requested_tail.min(limits.max_cursor_retained_items());
                let overflow_probe = u64::from(requested_tail > retained_items);
                let scanned_items = offset
                    .checked_add(first_page_items)
                    .and_then(|items| items.checked_add(retained_items))
                    .and_then(|items| items.checked_add(overflow_probe))
                    .ok_or(Error::CapacityLimit)?;
                (scanned_items, retained_items)
            }
        };
        let scanned_bytes = scanned_items
            .checked_mul(limits.max_source_item_bytes())
            .ok_or(Error::CapacityLimit)?;
        limits
            .execution_budget()
            .ensure(scanned_items, scanned_bytes)
            .map_err(|_| Error::CapacityLimit)?;
        let retained_bytes = retained_items
            .checked_mul(limits.max_source_item_bytes())
            .ok_or(Error::CapacityLimit)?;
        if retained_items > limits.max_cursor_retained_items()
            || retained_bytes > limits.max_cursor_value_bytes()
        {
            return Err(Error::CapacityLimit);
        }
        return Ok(());
    }

    let Some(_sort_key) = params.sorting.sort_by_metadata_key.as_ref() else {
        return Ok(());
    };
    let offset = usize::try_from(params.pagination.offset_value()).unwrap_or(usize::MAX);
    let limit = params.pagination.limit_value().map_or(usize::MAX, |limit| {
        usize::try_from(limit.get()).unwrap_or(usize::MAX)
    });
    let fetch_size = usize::try_from(fetch_size).unwrap_or(usize::MAX);
    let keep = offset
        .checked_add(limit.min(fetch_size))
        .ok_or(Error::CapacityLimit)?;
    let configured = usize::try_from(limits.max_cursor_retained_items()).unwrap_or(usize::MAX);
    if keep > STREAMING_SORTED_PREFIX_LIMIT || keep > configured {
        return Err(Error::CapacityLimit);
    }
    Ok(())
}

fn singular_query_has_preexecute_bounded_producer(query: &SingularQueryBox) -> bool {
    match query {
        SingularQueryBox::FindAccountByAlias(_)
        | SingularQueryBox::FindAliasesByAccountId(_)
        | SingularQueryBox::FindAccountRecoveryPolicyByAlias(_)
        | SingularQueryBox::FindAccountRecoveryRequestByAlias(_)
        | SingularQueryBox::FindDataspaceNameOwnerById(_) => false,
        SingularQueryBox::FindExecutorDataModel(_)
        | SingularQueryBox::FindParameters(_)
        | SingularQueryBox::FindAccountById(_)
        | SingularQueryBox::FindProofRecordById(_)
        | SingularQueryBox::FindContractManifestByCodeHash(_)
        | SingularQueryBox::FindAbiVersion(_)
        | SingularQueryBox::FindAssetById(_)
        | SingularQueryBox::FindDomainById(_)
        | SingularQueryBox::FindAssetDefinitionById(_)
        | SingularQueryBox::FindAssetEscrowById(_)
        | SingularQueryBox::FindTriggerById(_)
        | SingularQueryBox::FindTwitterBindingByHash(_)
        | SingularQueryBox::FindOracleFeedById(_)
        | SingularQueryBox::FindOracleDisputeById(_)
        | SingularQueryBox::FindOracleChangeById(_)
        | SingularQueryBox::FindOracleProviderStatsByKey(_)
        | SingularQueryBox::FindLatestDefiOracleAttestation(_)
        | SingularQueryBox::FindDomainEndorsements(_)
        | SingularQueryBox::FindDomainEndorsementPolicy(_)
        | SingularQueryBox::FindDomainCommittee(_)
        | SingularQueryBox::FindDaPinIntentByTicket(_)
        | SingularQueryBox::FindDaPinIntentByManifest(_)
        | SingularQueryBox::FindDaPinIntentByAlias(_)
        | SingularQueryBox::FindDaPinIntentByLaneEpochSequence(_)
        | SingularQueryBox::FindLaneRelayEnvelopeByRef(_)
        | SingularQueryBox::FindFeeSponsorProgramById(_)
        | SingularQueryBox::FindFxCorridorPolicyRegistry(_)
        | SingularQueryBox::FindFxCorridorPolicyById(_)
        | SingularQueryBox::FindSorafsProviderOwner(_)
        | SingularQueryBox::FindSorafsPinManifest(_)
        | SingularQueryBox::FindSorafsPinManifests(_)
        | SingularQueryBox::FindSorafsOrderbookPolicy(_)
        | SingularQueryBox::FindSorafsOrderbookOrderById(_)
        | SingularQueryBox::FindSorafsOrderbookCancellationByOrderId(_)
        | SingularQueryBox::FindSorafsOrderbookReceiptById(_)
        | SingularQueryBox::FindSorafsOrderbookTradeById(_)
        | SingularQueryBox::FindSorafsOrderbookChannelById(_)
        | SingularQueryBox::FindSorafsOrderbookStatus(_)
        | SingularQueryBox::FindSorafsOrderbookOrders(_)
        | SingularQueryBox::FindSorafsOrderbookReceipts(_)
        | SingularQueryBox::FindSorafsOrderbookTrades(_)
        | SingularQueryBox::FindSorafsOrderbookChannels(_)
        | SingularQueryBox::FindSorafsOrderbookEvents(_)
        | SingularQueryBox::FindSorafsReservePolicy(_)
        | SingularQueryBox::FindSorafsReserveProviderById(_)
        | SingularQueryBox::FindSorafsReserveMovementById(_)
        | SingularQueryBox::FindSorafsReserveAppealById(_)
        | SingularQueryBox::FindSorafsReserveProviders(_)
        | SingularQueryBox::FindSorafsReserveMovements(_)
        | SingularQueryBox::FindSorafsReserveAppeals(_)
        | SingularQueryBox::FindSorafsReserveEvents(_)
        | SingularQueryBox::FindSorafsPopIssuerPolicy(_)
        | SingularQueryBox::FindSorafsPopCredentialCommitmentByDigest(_)
        | SingularQueryBox::FindSorafsPopCommitmentRootByVersion(_)
        | SingularQueryBox::FindSorafsPopRevocationPublicationByVersion(_)
        | SingularQueryBox::FindSorafsPopRevocationByNonceCommitment(_)
        | SingularQueryBox::FindSorafsPopAuditDigestBySequence(_)
        | SingularQueryBox::FindSorafsPopRegistryStatus(_)
        | SingularQueryBox::FindSorafsRepairTask(_)
        | SingularQueryBox::FindSorafsRepairTasks(_)
        | SingularQueryBox::FindSorafsRepairStatus(_)
        | SingularQueryBox::FindSorafsRepairEvents(_)
        | SingularQueryBox::FindSorafsProofOutcome(_)
        | SingularQueryBox::FindSorafsProofOutcomeEvents(_)
        | SingularQueryBox::FindSorafsReputationJournalAuthorityPolicy(_)
        | SingularQueryBox::FindSorafsReputationJournalEventBySourceId(_)
        | SingularQueryBox::FindSorafsReputationJournalEvents(_)
        | SingularQueryBox::FindSorafsModerationPolicy(_)
        | SingularQueryBox::FindSorafsModerationAppeal(_)
        | SingularQueryBox::FindSorafsModerationJurorEligibility(_)
        | SingularQueryBox::FindSorafsModerationCase(_)
        | SingularQueryBox::FindSorafsModerationCommit(_)
        | SingularQueryBox::FindSorafsModerationReveal(_)
        | SingularQueryBox::FindSorafsModerationChallenge(_)
        | SingularQueryBox::FindSorafsModerationOutcome(_)
        | SingularQueryBox::FindSorafsModerationNoShow(_)
        | SingularQueryBox::FindSorafsModerationStatus(_)
        | SingularQueryBox::FindSorafsModerationSnapshot(_)
        | SingularQueryBox::FindSorafsModerationEvents(_)
        | SingularQueryBox::FindMusubiExactPackageV1(_)
        | SingularQueryBox::FindMusubiExactReleaseV1(_)
        | SingularQueryBox::FindMusubiProviderBundleAttestationV1(_)
        | SingularQueryBox::FindMusubiResolverIndexV1(_)
        | SingularQueryBox::FindMusubiVersionsV1(_)
        | SingularQueryBox::FindMusubiMaintainersV1(_)
        | SingularQueryBox::FindMusubiArchiveLocationsV1(_)
        | SingularQueryBox::FindMusubiArchiveRetentionV1(_)
        | SingularQueryBox::FindMusubiAliasV1(_)
        | SingularQueryBox::FindMusubiAliasHistoryV1(_)
        | SingularQueryBox::FindMusubiOrderedPrefixV1(_)
        | SingularQueryBox::FindNftById(_) => true,
    }
}

/// Measure a singular source before a metered server lane can clone or decode it.
///
/// The capability match is deliberately exhaustive. A new singular query must
/// opt into a pre-execute bounded producer before it can use the singular
/// output lane. Without that lane, only the borrowed source adapters below are
/// accepted; legacy in-process callers never invoke this function.
pub(super) fn preflight_server_singular_source_materialization(
    query: &SingularQueryBox,
    state: &impl StateReadOnly,
    budget: QueryExecutionBudget,
    singular_output_lane_active: bool,
) -> Result<u64, Error> {
    fn charge<T: NoritoSerialize>(value: &T, remaining: &mut u64) -> Result<(), Error> {
        let bytes = super::bounded_bare_encoded_len(value, *remaining)?;
        *remaining = remaining
            .checked_sub(bytes)
            .ok_or(Error::GasBudgetExceeded)?;
        Ok(())
    }

    fn charge_fixed(bytes: u64, remaining: &mut u64) -> Result<(), Error> {
        *remaining = remaining
            .checked_sub(bytes)
            .ok_or(Error::GasBudgetExceeded)?;
        Ok(())
    }

    fn reject_unbounded(name: &str) -> Error {
        Error::Conversion(format!(
            "metered server singular query `{name}` has no pre-execute bounded materialization adapter"
        ))
    }

    fn require_active_adapter(active: bool, name: &str) -> Result<(), Error> {
        if active {
            Ok(())
        } else {
            Err(reject_unbounded(name))
        }
    }

    if singular_output_lane_active && !singular_query_has_preexecute_bounded_producer(query) {
        return Err(reject_unbounded("unsupported singular producer"));
    }

    let _canonical_flags = DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let limit = budget.remaining_bytes(1, 0)?;
    let world = state.world();
    let mut remaining = limit;

    match query {
        SingularQueryBox::FindExecutorDataModel(_) => {
            let model = world.executor_data_model();
            if model.permissions().is_empty() {
                return Err(reject_unbounded("FindExecutorDataModel fallback"));
            }
            charge(model, &mut remaining)?;
        }
        SingularQueryBox::FindParameters(_) => charge(world.parameters(), &mut remaining)?,
        SingularQueryBox::FindAccountById(query) => {
            if let Some((account_id, account_value)) =
                world.accounts().get_key_value(query.account_id())
            {
                charge(account_id, &mut remaining)?;
                charge(account_value.as_ref(), &mut remaining)?;
                charge_fixed(64, &mut remaining)?;
            }
        }
        SingularQueryBox::FindAccountByAlias(_)
        | SingularQueryBox::FindAliasesByAccountId(_)
        | SingularQueryBox::FindAccountRecoveryPolicyByAlias(_)
        | SingularQueryBox::FindAccountRecoveryRequestByAlias(_) => {
            return Err(reject_unbounded("account alias resolution"));
        }
        SingularQueryBox::FindProofRecordById(query) => {
            if let Some(record) = world.proofs().get(&query.id) {
                charge(record, &mut remaining)?;
            }
        }
        SingularQueryBox::FindContractManifestByCodeHash(query) => {
            if let Some(manifest) = world.contract_manifests().get(&query.code_hash) {
                charge(manifest, &mut remaining)?;
            }
        }
        SingularQueryBox::FindAbiVersion(_) => {}
        SingularQueryBox::FindAssetById(query) => {
            if let Ok(asset) = world.asset(query.asset_id()) {
                charge(asset.id(), &mut remaining)?;
                charge(asset.value().as_ref(), &mut remaining)?;
                charge_fixed(32, &mut remaining)?;
            }
        }
        SingularQueryBox::FindDomainById(query) => {
            if let Ok(domain) = world.domain(query.domain_id()) {
                charge(domain, &mut remaining)?;
            }
        }
        SingularQueryBox::FindAssetDefinitionById(query) => {
            if let Some(definition) = world.asset_definitions().get(query.asset_definition_id()) {
                charge(definition, &mut remaining)?;
                if let Some(binding) = world
                    .asset_definition_alias_bindings()
                    .get(query.asset_definition_id())
                {
                    charge(binding, &mut remaining)?;
                }
                charge_fixed(128, &mut remaining)?;
            }
        }
        SingularQueryBox::FindAssetEscrowById(query) => {
            if let Some(record) = world.asset_escrows().get(&query.escrow_id) {
                charge(record, &mut remaining)?;
            }
        }
        SingularQueryBox::FindTriggerById(_) => {
            require_active_adapter(singular_output_lane_active, "FindTriggerById")?;
        }
        SingularQueryBox::FindTwitterBindingByHash(query) => {
            if let Some(record) = world.twitter_bindings().get(&query.binding_hash.digest) {
                charge(record, &mut remaining)?;
            }
        }
        SingularQueryBox::FindOracleFeedById(query) => {
            if let Some(record) = world.oracle_feeds().get(&query.feed_id) {
                charge(record, &mut remaining)?;
            }
        }
        SingularQueryBox::FindOracleDisputeById(query) => {
            if let Some(record) = world.oracle_disputes().get(&query.dispute_id) {
                charge(record, &mut remaining)?;
            }
        }
        SingularQueryBox::FindOracleChangeById(query) => {
            if let Some(record) = world.oracle_changes().get(&query.change_id) {
                charge(record, &mut remaining)?;
            }
        }
        SingularQueryBox::FindOracleProviderStatsByKey(_) => {}
        SingularQueryBox::FindLatestDefiOracleAttestation(query) => {
            if let Some(record) = world
                .defi_oracle_attestations()
                .get(&query.key)
                .and_then(|records| records.last())
            {
                charge(record, &mut remaining)?;
            }
        }
        SingularQueryBox::FindDomainEndorsements(query) => {
            if let Some(hashes) = world.domain_endorsements_by_domain().get(&query.domain_id) {
                charge_fixed(8, &mut remaining)?;
                for hash in hashes {
                    if let Some(record) = world.domain_endorsements().get(hash) {
                        charge(record, &mut remaining)?;
                        charge_fixed(16, &mut remaining)?;
                    }
                }
            }
        }
        SingularQueryBox::FindDomainEndorsementPolicy(query) => {
            if let Some(policy) = world.domain_endorsement_policies().get(&query.domain_id) {
                charge(policy, &mut remaining)?;
            }
        }
        SingularQueryBox::FindDomainCommittee(query) => {
            if let Some(committee) = world.domain_committees().get(&query.committee_id) {
                charge(committee, &mut remaining)?;
            }
        }
        SingularQueryBox::FindDaPinIntentByTicket(query) => {
            if let Some(intent) = world.da_pin_intents_by_ticket().get(&query.storage_ticket) {
                charge(intent, &mut remaining)?;
            }
        }
        SingularQueryBox::FindDaPinIntentByManifest(query) => {
            if let Some(ticket) = world.da_pin_intents_by_manifest().get(&query.manifest_hash)
                && let Some(intent) = world.da_pin_intents_by_ticket().get(ticket)
            {
                charge(intent, &mut remaining)?;
            }
        }
        SingularQueryBox::FindDaPinIntentByAlias(query) => {
            if let Some(ticket) = world.da_pin_intents_by_alias().get(&query.alias)
                && let Some(intent) = world.da_pin_intents_by_ticket().get(ticket)
            {
                charge(intent, &mut remaining)?;
            }
        }
        SingularQueryBox::FindDaPinIntentByLaneEpochSequence(query) => {
            if let Some(ticket) = world.da_pin_intents_by_lane_epoch().get(&(
                query.lane_id,
                query.epoch,
                query.sequence,
            )) && let Some(intent) = world.da_pin_intents_by_ticket().get(ticket)
            {
                charge(intent, &mut remaining)?;
            }
        }
        SingularQueryBox::FindLaneRelayEnvelopeByRef(_) => {
            require_active_adapter(singular_output_lane_active, "FindLaneRelayEnvelopeByRef")?;
        }
        SingularQueryBox::FindFeeSponsorProgramById(query) => {
            if let Some(policy) = world.fee_sponsor_programs().get(&query.id) {
                charge(policy, &mut remaining)?;
            }
        }
        SingularQueryBox::FindFxCorridorPolicyRegistry(_)
        | SingularQueryBox::FindFxCorridorPolicyById(_) => {
            require_active_adapter(
                singular_output_lane_active,
                "FX corridor policy materialization",
            )?;
            let parameter_id =
                iroha_data_model::isi::settlement::FxCorridorPolicyRegistry::parameter_id();
            if let Some(parameter) = world.parameters().custom().get(&parameter_id) {
                charge(parameter.payload(), &mut remaining)?;
            }
        }
        SingularQueryBox::FindSorafsProviderOwner(query) => {
            if let Some(owner) = world.provider_owners().get(&query.provider_id) {
                charge(owner, &mut remaining)?;
            }
        }
        SingularQueryBox::FindSorafsPinManifest(query) => {
            if let Some(manifest) = world.pin_manifests().get(&query.digest) {
                charge(manifest, &mut remaining)?;
            }
        }
        SingularQueryBox::FindSorafsPinManifests(_) => {
            require_active_adapter(
                singular_output_lane_active,
                "SoraFS pin-manifest page query",
            )?;
        }
        SingularQueryBox::FindSorafsOrderbookPolicy(_)
        | SingularQueryBox::FindSorafsOrderbookOrderById(_)
        | SingularQueryBox::FindSorafsOrderbookCancellationByOrderId(_)
        | SingularQueryBox::FindSorafsOrderbookReceiptById(_)
        | SingularQueryBox::FindSorafsOrderbookTradeById(_)
        | SingularQueryBox::FindSorafsOrderbookChannelById(_)
        | SingularQueryBox::FindSorafsOrderbookStatus(_)
        | SingularQueryBox::FindSorafsOrderbookOrders(_)
        | SingularQueryBox::FindSorafsOrderbookReceipts(_)
        | SingularQueryBox::FindSorafsOrderbookTrades(_)
        | SingularQueryBox::FindSorafsOrderbookChannels(_)
        | SingularQueryBox::FindSorafsOrderbookEvents(_) => {
            require_active_adapter(singular_output_lane_active, "SoraFS orderbook query")?;
        }
        SingularQueryBox::FindSorafsReservePolicy(_)
        | SingularQueryBox::FindSorafsReserveProviderById(_)
        | SingularQueryBox::FindSorafsReserveMovementById(_)
        | SingularQueryBox::FindSorafsReserveAppealById(_)
        | SingularQueryBox::FindSorafsReserveProviders(_)
        | SingularQueryBox::FindSorafsReserveMovements(_)
        | SingularQueryBox::FindSorafsReserveAppeals(_)
        | SingularQueryBox::FindSorafsReserveEvents(_) => {
            require_active_adapter(singular_output_lane_active, "SoraFS reserve query")?;
        }
        SingularQueryBox::FindSorafsPopIssuerPolicy(_)
        | SingularQueryBox::FindSorafsPopCredentialCommitmentByDigest(_)
        | SingularQueryBox::FindSorafsPopCommitmentRootByVersion(_)
        | SingularQueryBox::FindSorafsPopRevocationPublicationByVersion(_)
        | SingularQueryBox::FindSorafsPopRevocationByNonceCommitment(_)
        | SingularQueryBox::FindSorafsPopAuditDigestBySequence(_)
        | SingularQueryBox::FindSorafsPopRegistryStatus(_) => {
            require_active_adapter(singular_output_lane_active, "SoraFS PoP registry query")?;
        }
        SingularQueryBox::FindSorafsRepairTask(_)
        | SingularQueryBox::FindSorafsRepairTasks(_)
        | SingularQueryBox::FindSorafsRepairStatus(_)
        | SingularQueryBox::FindSorafsRepairEvents(_) => {
            require_active_adapter(singular_output_lane_active, "SoraFS repair query")?;
        }
        SingularQueryBox::FindSorafsProofOutcome(_)
        | SingularQueryBox::FindSorafsProofOutcomeEvents(_) => {
            require_active_adapter(singular_output_lane_active, "SoraFS proof-outcome query")?;
        }
        SingularQueryBox::FindSorafsReputationJournalAuthorityPolicy(_)
        | SingularQueryBox::FindSorafsReputationJournalEventBySourceId(_)
        | SingularQueryBox::FindSorafsReputationJournalEvents(_) => {
            require_active_adapter(
                singular_output_lane_active,
                "SoraFS reputation-journal query",
            )?;
        }
        SingularQueryBox::FindSorafsModerationPolicy(_)
        | SingularQueryBox::FindSorafsModerationAppeal(_)
        | SingularQueryBox::FindSorafsModerationJurorEligibility(_)
        | SingularQueryBox::FindSorafsModerationCase(_)
        | SingularQueryBox::FindSorafsModerationCommit(_)
        | SingularQueryBox::FindSorafsModerationReveal(_)
        | SingularQueryBox::FindSorafsModerationChallenge(_)
        | SingularQueryBox::FindSorafsModerationOutcome(_)
        | SingularQueryBox::FindSorafsModerationNoShow(_)
        | SingularQueryBox::FindSorafsModerationStatus(_)
        | SingularQueryBox::FindSorafsModerationSnapshot(_)
        | SingularQueryBox::FindSorafsModerationEvents(_) => {
            require_active_adapter(singular_output_lane_active, "SoraFS moderation query")?;
        }
        SingularQueryBox::FindDataspaceNameOwnerById(_) => {
            return Err(reject_unbounded("FindDataspaceNameOwnerById"));
        }
        SingularQueryBox::FindMusubiExactPackageV1(_)
        | SingularQueryBox::FindMusubiExactReleaseV1(_)
        | SingularQueryBox::FindMusubiProviderBundleAttestationV1(_)
        | SingularQueryBox::FindMusubiResolverIndexV1(_)
        | SingularQueryBox::FindMusubiVersionsV1(_)
        | SingularQueryBox::FindMusubiMaintainersV1(_)
        | SingularQueryBox::FindMusubiArchiveLocationsV1(_)
        | SingularQueryBox::FindMusubiArchiveRetentionV1(_)
        | SingularQueryBox::FindMusubiAliasV1(_)
        | SingularQueryBox::FindMusubiAliasHistoryV1(_)
        | SingularQueryBox::FindMusubiOrderedPrefixV1(_) => {
            require_active_adapter(singular_output_lane_active, "Musubi V1 query")?;
        }
        SingularQueryBox::FindNftById(query) => {
            if let Ok(nft) = world.nft(query.nft_id()) {
                charge(nft.id(), &mut remaining)?;
                charge(nft.value().as_ref(), &mut remaining)?;
                charge_fixed(48, &mut remaining)?;
            }
        }
    }
    limit.checked_sub(remaining).ok_or(Error::GasBudgetExceeded)
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };

    use iroha_data_model::query::{
        parameters::{FetchSize, Pagination},
        runtime::prelude::FindAbiVersion,
    };
    use nonzero_ext::nonzero;

    use super::*;

    #[derive(Debug)]
    struct TestReservation {
        bytes: u64,
        pool_generation: u64,
        released: Arc<AtomicU64>,
    }

    impl Drop for TestReservation {
        fn drop(&mut self) {
            self.released.fetch_add(self.bytes, Ordering::SeqCst);
        }
    }

    impl OrdinaryQueryMemoryReservation for TestReservation {
        fn reserved_bytes(&self) -> u64 {
            self.bytes
        }

        fn pool_generation(&self) -> u64 {
            self.pool_generation
        }

        fn split_off(&mut self, bytes: u64) -> Option<Box<dyn OrdinaryQueryMemoryReservation>> {
            if bytes == 0 || bytes > self.bytes {
                return None;
            }
            self.bytes -= bytes;
            Some(Box::new(Self {
                bytes,
                pool_generation: self.pool_generation,
                released: Arc::clone(&self.released),
            }))
        }
    }

    #[derive(Debug)]
    struct FailingSplitReservation(TestReservation);

    impl OrdinaryQueryMemoryReservation for FailingSplitReservation {
        fn reserved_bytes(&self) -> u64 {
            self.0.bytes
        }

        fn pool_generation(&self) -> u64 {
            self.0.pool_generation
        }

        fn split_off(&mut self, _bytes: u64) -> Option<Box<dyn OrdinaryQueryMemoryReservation>> {
            None
        }
    }

    fn limits() -> OrdinaryQueryExecutionLimits {
        OrdinaryQueryExecutionLimits::try_new(
            11,
            QueryExecutionBudget::from_weighted_limit(64 * 1024, 1, 1),
            16,
            64 * 1024,
            ORDINARY_NAME_ID_SOURCE_BYTES,
            16 * 1024,
            16,
            16 * ORDINARY_NAME_ID_SOURCE_BYTES,
            32 * 1024,
            4 * 1024,
            norito::DecodeLimits::new(64, 4 * 1024, 256, 16 * 1024, 16),
        )
        .expect("valid ordinary query geometry")
    }

    fn cursor_policy(
        limits: OrdinaryQueryExecutionLimits,
        pool_generation: u64,
    ) -> OrdinaryQueryCursorPolicy {
        QueryLimits::new(limits.max_page_items())
            .with_count_mode(QueryCountMode::Bounded)
            .with_ordinary_execution_limits(limits)
            .ordinary_cursor_policy(pool_generation)
            .expect("ordinary policy")
    }

    #[test]
    fn validated_geometry_requires_exact_f_plus_one_work() {
        let max_page_items = 4_u64;
        let max_source_item_bytes = 1_024_u64;
        let page_with_probe = max_page_items.checked_add(1).expect("F + 1");
        let probe_bytes = page_with_probe
            .checked_mul(max_source_item_bytes)
            .expect("probe bytes");
        let exact_units = page_with_probe
            .checked_add(probe_bytes)
            .expect("weighted units");
        let decode = norito::DecodeLimits::new(16, 1_024, 32, 4 * 1_024, 8);
        let execution_headroom = OrdinaryQueryExecutionLimits::required_execution_headroom_bytes(
            max_page_items,
            max_source_item_bytes,
            4 * 1_024,
            1_024,
            decode,
        )
        .expect("execution geometry");
        let cursor_retained = OrdinaryQueryExecutionLimits::required_cursor_retained_bytes(
            4,
            max_source_item_bytes,
            4 * 1_024,
            1_024,
        )
        .expect("cursor geometry");

        OrdinaryQueryExecutionLimits::try_new(
            1,
            QueryExecutionBudget::from_weighted_limit(exact_units, 1, 1),
            max_page_items,
            execution_headroom,
            max_source_item_bytes,
            4 * 1_024,
            4,
            4 * 1_024,
            cursor_retained,
            1_024,
            decode,
        )
        .expect("exact F + 1 work must be admitted");

        assert_eq!(
            OrdinaryQueryExecutionLimits::try_new(
                1,
                QueryExecutionBudget::from_weighted_limit(exact_units - 1, 1, 1),
                max_page_items,
                execution_headroom,
                max_source_item_bytes,
                4 * 1_024,
                4,
                4 * 1_024,
                cursor_retained,
                1_024,
                decode,
            ),
            Err(OrdinaryQueryExecutionLimitError::ExecutionBudgetTooSmall)
        );
    }

    #[test]
    fn validated_geometry_rejects_underreservation_and_overflow() {
        let decode = norito::DecodeLimits::new(16, 1_024, 32, 4 * 1_024, 8);
        let required_execution = OrdinaryQueryExecutionLimits::required_execution_headroom_bytes(
            4,
            1_024,
            4 * 1_024,
            1_024,
            decode,
        )
        .expect("execution geometry");
        let required_cursor = OrdinaryQueryExecutionLimits::required_cursor_retained_bytes(
            4,
            1_024,
            4 * 1_024,
            1_024,
        )
        .expect("cursor geometry");
        let budget = QueryExecutionBudget::from_weighted_limit(64 * 1_024, 1, 1);

        assert_eq!(
            OrdinaryQueryExecutionLimits::try_new(
                1,
                budget,
                4,
                required_execution - 1,
                1_024,
                4 * 1_024,
                4,
                4 * 1_024,
                required_cursor,
                1_024,
                decode,
            ),
            Err(OrdinaryQueryExecutionLimitError::ExecutionHeadroomTooSmall)
        );
        assert_eq!(
            OrdinaryQueryExecutionLimits::try_new(
                1,
                budget,
                4,
                required_execution,
                1_024,
                4 * 1_024,
                4,
                4 * 1_024,
                required_cursor - 1,
                1_024,
                decode,
            ),
            Err(OrdinaryQueryExecutionLimitError::CursorRetentionTooSmall)
        );
        assert_eq!(
            OrdinaryQueryExecutionLimits::required_execution_headroom_bytes(
                u64::MAX,
                2,
                1,
                1,
                decode,
            ),
            Err(OrdinaryQueryExecutionLimitError::GeometryOverflow)
        );
        assert_eq!(
            OrdinaryQueryExecutionLimits::required_cursor_retained_bytes(u64::MAX, 2, 1, 1),
            Err(OrdinaryQueryExecutionLimitError::GeometryOverflow)
        );
    }

    #[test]
    fn validated_geometry_covers_continue_decode_and_reencode_overlap() {
        let decode = norito::DecodeLimits::new(8, 32, 8, 16 * 1_024, 4);
        let required = OrdinaryQueryExecutionLimits::required_execution_headroom_bytes(
            1,
            1,
            1,
            8 * 1_024,
            decode,
        )
        .expect("revalidation geometry");
        assert_eq!(
            required,
            8 * 1_024 + 16 * 1_024 + ORDINARY_QUERY_FIXED_CONTAINER_OVERHEAD_BYTES
        );

        let retained =
            OrdinaryQueryExecutionLimits::required_cursor_retained_bytes(1, 1, 1, 8 * 1_024)
                .expect("cursor geometry");
        assert_eq!(
            OrdinaryQueryExecutionLimits::try_new(
                1,
                QueryExecutionBudget::from_weighted_limit(2, 0, 1),
                1,
                required - 1,
                1,
                1,
                1,
                1,
                retained,
                8 * 1_024,
                decode,
            ),
            Err(OrdinaryQueryExecutionLimitError::ExecutionHeadroomTooSmall)
        );
    }

    #[test]
    fn cursor_policy_rejects_config_and_pool_generation_changes() {
        let limits = limits();
        let original = cursor_policy(limits, 7);
        let binding = OrdinaryQueryCursorBinding {
            retained_bytes: limits.max_cursor_retained_bytes(),
            policy: original,
        };
        assert!(binding.is_compatible_with(original));
        assert!(!binding.is_compatible_with(cursor_policy(limits, 8)));

        let changed_limits = OrdinaryQueryExecutionLimits::try_new(
            limits.policy_generation() + 1,
            limits.execution_budget(),
            limits.max_page_items(),
            limits.execution_headroom_bytes(),
            limits.max_source_item_bytes(),
            limits.max_response_bytes(),
            limits.max_cursor_retained_items(),
            limits.max_cursor_value_bytes(),
            limits.max_cursor_retained_bytes(),
            limits.max_revalidation_archive_bytes(),
            limits.revalidation_decode_limits(),
        )
        .expect("same geometry with a new policy generation");
        assert!(!binding.is_compatible_with(cursor_policy(changed_limits, 7)));
    }

    #[test]
    fn split_cursor_charge_releases_independently() {
        let released = Arc::new(AtomicU64::new(0));
        let limits = limits();
        let retained = limits.max_cursor_retained_bytes();
        let total = limits
            .execution_headroom_bytes()
            .checked_add(retained)
            .expect("test geometry");
        let lease = OrdinaryQueryMemoryLease::new(TestReservation {
            bytes: total,
            pool_generation: 7,
            released: Arc::clone(&released),
        });
        let admission =
            OrdinaryQueryMemoryAdmission::new(lease, retained, Some(cursor_policy(limits, 7)))
                .expect("admission");

        let cursor = admission.split_cursor_lease().expect("cursor split");
        let response = admission
            .take_response_lease(false)
            .expect("response remainder");
        assert_eq!(cursor.binding().retained_bytes(), retained);
        assert_eq!(response.reserved_bytes(), limits.execution_headroom_bytes());
        drop(cursor);
        assert_eq!(released.load(Ordering::SeqCst), retained);
        drop(response);
        assert_eq!(released.load(Ordering::SeqCst), total);
    }

    #[test]
    fn failed_split_leaves_the_whole_reservation_owned() {
        let released = Arc::new(AtomicU64::new(0));
        let limits = limits();
        let retained = limits.max_cursor_retained_bytes();
        let total = limits
            .execution_headroom_bytes()
            .checked_add(retained)
            .expect("test geometry");
        let lease = OrdinaryQueryMemoryLease::new(FailingSplitReservation(TestReservation {
            bytes: total,
            pool_generation: 7,
            released: Arc::clone(&released),
        }));
        let admission =
            OrdinaryQueryMemoryAdmission::new(lease, retained, Some(cursor_policy(limits, 7)))
                .expect("admission");

        assert!(matches!(
            admission.split_cursor_lease(),
            Err(Error::CapacityLimit)
        ));
        let response = admission
            .take_response_lease(false)
            .expect("failed split must leave the parent token available");
        assert_eq!(response.reserved_bytes(), total);
        assert_eq!(released.load(Ordering::SeqCst), 0);
        drop(response);
        assert_eq!(released.load(Ordering::SeqCst), total);
    }

    #[test]
    fn fixed_scalar_singular_is_admitted() {
        let request = QueryRequest::Singular(FindAbiVersion.into());
        ensure_request_admitted(
            &request,
            OrdinaryCursorMode::Ephemeral,
            QueryLimits::new(16),
            limits(),
        )
        .expect("fixed scalar must be admitted");
    }

    #[test]
    fn unadapted_singular_fails_before_execution() {
        let request = QueryRequest::Singular(
            iroha_data_model::query::account::prelude::FindAccountById::new(
                iroha_test_samples::ALICE_ID.clone(),
            )
            .into(),
        );
        let error = ensure_request_admitted(
            &request,
            OrdinaryCursorMode::Ephemeral,
            QueryLimits::new(16),
            limits(),
        )
        .expect_err("account clone is not adapted");
        assert!(matches!(error, Error::Conversion(_)));
    }

    #[test]
    fn stored_exact_requires_and_bounds_pagination_before_execution() {
        let query_limits = QueryLimits::new(16);
        let mut params = QueryParams::default();
        params.fetch_size = FetchSize::new(Some(nonzero!(16_u64)));
        let error =
            ensure_identifier_params(&params, OrdinaryCursorMode::Stored, query_limits, limits())
                .expect_err("exact count without a limit must fail closed");
        assert!(matches!(error, Error::Conversion(_)));

        params.pagination = Pagination::new(Some(nonzero!(16_u64)), 0);
        ensure_identifier_params(&params, OrdinaryCursorMode::Stored, query_limits, limits())
            .expect("the configured retained bound is admitted");

        params.pagination = Pagination::new(Some(nonzero!(17_u64)), 0);
        assert_eq!(
            ensure_identifier_params(&params, OrdinaryCursorMode::Stored, query_limits, limits(),),
            Err(Error::CapacityLimit)
        );

        params.pagination = Pagination::new(Some(nonzero!(16_u64)), 64 * 1_024);
        assert_eq!(
            ensure_identifier_params(&params, OrdinaryCursorMode::Stored, query_limits, limits(),),
            Err(Error::CapacityLimit),
            "offset plus limit may not scan beyond the server work budget"
        );
    }

    #[test]
    fn stored_bounded_offset_and_tail_share_the_weighted_work_budget() {
        let query_limits = QueryLimits::new(16).with_count_mode(QueryCountMode::Bounded);
        let mut params = QueryParams {
            fetch_size: FetchSize::new(Some(nonzero!(16_u64))),
            ..QueryParams::default()
        };
        params.pagination = Pagination::new(Some(nonzero!(16_u64)), 47);
        ensure_identifier_params(&params, OrdinaryCursorMode::Stored, query_limits, limits())
            .expect("offset plus page fits the shared item/byte budget exactly enough");

        params.pagination = Pagination::new(Some(nonzero!(16_u64)), 48);
        assert_eq!(
            ensure_identifier_params(&params, OrdinaryCursorMode::Stored, query_limits, limits(),),
            Err(Error::CapacityLimit),
            "offset and page bytes may not each consume the same weighted pool"
        );

        params.pagination = Pagination::new(None, 31);
        assert_eq!(
            ensure_identifier_params(&params, OrdinaryCursorMode::Stored, query_limits, limits(),),
            Err(Error::CapacityLimit),
            "bounded Start must account for offset, first page, retained tail, and overflow probe"
        );
    }
}
