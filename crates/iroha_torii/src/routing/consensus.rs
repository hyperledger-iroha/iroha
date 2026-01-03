//! Consensus-related Torii handlers split out from the main routing module.

use super::*;
use iroha_core::state::StateReadOnly;
use iroha_data_model::prelude::ChainId;
use iroha_data_model::{
    block::consensus::{
        SumeragiBlockSyncRosterStatus, SumeragiCommitCertificateStatus, SumeragiCommitInflightStatus,
        SumeragiCommitQuorumStatus, SumeragiConsensusCapsStatus, SumeragiDataspaceCommitment,
        SumeragiLaneCommitment, SumeragiLaneGovernance, SumeragiMembershipMismatchStatus,
        SumeragiMembershipStatus, SumeragiPeerKeyPolicyStatus, SumeragiPendingRbcEntry,
        SumeragiPendingRbcStatus, SumeragiRuntimeUpgradeHook, SumeragiStatusWire,
        SumeragiValidationRejectStatus, SumeragiViewChangeCauseStatus, SumeragiWorkerLoopStatus,
        SumeragiWorkerQueueDepths, SumeragiWorkerQueueDiagnostics, SumeragiWorkerQueueTotals,
    },
    nexus::{DataSpaceId, LaneId},
};

#[derive(Clone, Debug, Encode, Decode)]
struct EvidenceListWire {
    total: u64,
    items: Vec<EvidenceRecord>,
}

#[derive(Clone, Debug, Encode, Decode)]
struct ExecRootWire {
    block_hash: iroha_crypto::HashOf<BlockHeader>,
    exec_root: Option<iroha_crypto::Hash>,
}

#[derive(Debug, crate::json_macros::JsonSerialize, norito::derive::NoritoSerialize)]
struct SumeragiPacemakerResponse {
    backoff_ms: u64,
    rtt_floor_ms: u64,
    jitter_ms: u64,
    backoff_multiplier: u64,
    rtt_floor_multiplier: u64,
    max_backoff_ms: u64,
    jitter_frac_permille: u64,
    round_elapsed_ms: u64,
    view_timeout_target_ms: u64,
    view_timeout_remaining_ms: u64,
}

#[derive(Debug, crate::json_macros::JsonSerialize, norito::derive::NoritoSerialize)]
#[allow(clippy::struct_field_names)]
struct SumeragiPhasesEma {
    propose_ms: u64,
    collect_da_ms: u64,
    collect_prevote_ms: u64,
    collect_precommit_ms: u64,
    collect_aggregator_ms: u64,
    collect_exec_ms: u64,
    collect_witness_ms: u64,
    commit_ms: u64,
    pipeline_total_ms: u64,
}

#[derive(Debug, crate::json_macros::JsonSerialize, norito::derive::NoritoSerialize)]
struct SumeragiPhasesResponse {
    propose_ms: u64,
    collect_da_ms: u64,
    collect_prevote_ms: u64,
    collect_precommit_ms: u64,
    collect_aggregator_ms: u64,
    collect_exec_ms: u64,
    collect_witness_ms: u64,
    commit_ms: u64,
    pipeline_total_ms: u64,
    collect_aggregator_gossip_total: u64,
    block_created_dropped_by_lock_total: u64,
    block_created_hint_mismatch_total: u64,
    block_created_proposal_mismatch_total: u64,
    ema_ms: SumeragiPhasesEma,
}

#[derive(Debug, crate::json_macros::JsonSerialize, norito::derive::NoritoSerialize)]
struct PrfContext {
    height: u64,
    view: u64,
    #[norito(skip_serializing_if = "Option::is_none")]
    epoch_seed: Option<String>,
}

#[derive(Debug, crate::json_macros::JsonSerialize, norito::derive::NoritoSerialize)]
struct SumeragiLeaderResponse {
    leader_index: u64,
    prf: PrfContext,
}

#[derive(Debug, crate::json_macros::JsonSerialize, norito::derive::NoritoSerialize)]
struct CollectorEntry {
    index: u64,
    peer_id: String,
}

#[derive(Debug, crate::json_macros::JsonSerialize, norito::derive::NoritoSerialize)]
struct CollectorsResponse {
    consensus_mode: &'static str,
    mode: &'static str,
    topology_len: u64,
    min_votes_for_commit: u64,
    proxy_tail_index: u64,
    height: u64,
    view: u64,
    collectors_k: u64,
    redundant_send_r: u64,
    #[norito(skip_serializing_if = "Option::is_none")]
    epoch_seed: Option<String>,
    collectors: Vec<CollectorEntry>,
    prf: PrfContext,
}

#[derive(Debug, crate::json_macros::JsonSerialize, norito::derive::NoritoSerialize)]
struct SumeragiParamsResponse {
    block_time_ms: u64,
    commit_time_ms: u64,
    max_clock_drift_ms: u64,
    collectors_k: u64,
    redundant_send_r: u64,
    da_enabled: bool,
    #[norito(skip_serializing_if = "Option::is_none")]
    next_mode: Option<&'static str>,
    mode_activation_height: Option<u64>,
    chain_height: u64,
}

#[cfg(test)]
fn json_string(value: Value) -> String {
    norito::json::to_string(&value).expect("serialize request body")
}

mod debug_toggle_override {
    pub(super) fn torii_override_active() -> bool {
        state::torii_active()
    }

    #[cfg(test)]
    pub(super) fn set_torii_override(active: bool) -> bool {
        state::set_torii(active)
    }

    #[cfg(test)]
    pub(super) fn set_iroha_override(active: bool) -> bool {
        state::set_iroha(active)
    }

    #[cfg(test)]
    mod state {
        use std::sync::atomic::{AtomicBool, Ordering};

        pub(super) static TORII_DEBUG_MATCH: AtomicBool = AtomicBool::new(false);
        pub(super) static IROHA_DEBUG_TX_EVAL: AtomicBool = AtomicBool::new(false);

        pub(super) fn set_torii(active: bool) -> bool {
            TORII_DEBUG_MATCH.swap(active, Ordering::SeqCst)
        }

        pub(super) fn torii_active() -> bool {
            TORII_DEBUG_MATCH.load(Ordering::SeqCst)
        }

        pub(super) fn set_iroha(active: bool) -> bool {
            IROHA_DEBUG_TX_EVAL.swap(active, Ordering::SeqCst)
        }
    }

    #[cfg(not(test))]
    mod state {
        pub(super) fn set_torii(_active: bool) -> bool {
            false
        }

        pub(super) fn torii_active() -> bool {
            false
        }

        pub(super) fn set_iroha(_active: bool) -> bool {
            false
        }
    }
}

fn torii_debug_match_enabled() -> bool {
    super::debug_match_flag::enabled(debug_toggle_override::torii_override_active())
}

/// Compute start/end bounds for paginating a collection of length `len`.
///
/// - `offset` values that exceed `usize::MAX` (on the current platform) or the
///   collection length clamp to the end of the collection, yielding an empty
///   slice.
/// - When `cap` is provided, user-supplied limits are clamped to that maximum.
fn pagination_bounds(
    len: usize,
    offset: u64,
    limit: Option<u64>,
    cap: Option<u64>,
) -> (usize, usize) {
    let start = match usize::try_from(offset) {
        Ok(off) => off.min(len),
        Err(_) => len,
    };

    let limited = limit.map(|lim| cap.map_or(lim, |cap_lim| lim.min(cap_lim)));

    let end = limited
        .and_then(|lim| usize::try_from(lim).ok())
        .map(|lim| start.saturating_add(lim).min(len))
        .unwrap_or(len);

    (start, end)
}

#[cfg(test)]
mod pagination_tests {
    use super::pagination_bounds;

    #[test]
    fn pagination_bounds_limit_zero_returns_empty() {
        let (start, end) = pagination_bounds(10, 0, Some(0), Some(7));
        assert_eq!((start, end), (0, 0));
    }
}

#[cfg(feature = "app_api")]
#[derive(Debug)]
struct PageEntry<K, T> {
    key: K,
    seq: usize,
    item: T,
}

#[cfg(feature = "app_api")]
#[derive(Clone)]
enum SortKeyValue {
    Text(String),
    Numeric(iroha_primitives::numeric::Numeric),
}

#[cfg(feature = "app_api")]
impl SortKeyValue {
    fn variant_ord(&self) -> usize {
        match self {
            SortKeyValue::Text(_) => 0,
            SortKeyValue::Numeric(_) => 1,
        }
    }
}

#[cfg(feature = "app_api")]
impl From<String> for SortKeyValue {
    fn from(value: String) -> Self {
        SortKeyValue::Text(value)
    }
}

#[cfg(feature = "app_api")]
impl From<&String> for SortKeyValue {
    fn from(value: &String) -> Self {
        SortKeyValue::Text(value.clone())
    }
}

#[cfg(feature = "app_api")]
impl From<&str> for SortKeyValue {
    fn from(value: &str) -> Self {
        SortKeyValue::Text(value.to_owned())
    }
}

#[cfg(feature = "app_api")]
impl From<iroha_primitives::numeric::Numeric> for SortKeyValue {
    fn from(value: iroha_primitives::numeric::Numeric) -> Self {
        SortKeyValue::Numeric(value)
    }
}

#[cfg(feature = "app_api")]
impl From<&iroha_primitives::numeric::Numeric> for SortKeyValue {
    fn from(value: &iroha_primitives::numeric::Numeric) -> Self {
        SortKeyValue::Numeric(value.clone())
    }
}

#[cfg(feature = "app_api")]
impl PartialEq for SortKeyValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SortKeyValue::Text(lhs), SortKeyValue::Text(rhs)) => lhs == rhs,
            (SortKeyValue::Numeric(lhs), SortKeyValue::Numeric(rhs)) => lhs == rhs,
            _ => false,
        }
    }
}

#[cfg(feature = "app_api")]
impl Eq for SortKeyValue {}

#[cfg(feature = "app_api")]
impl Ord for SortKeyValue {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (SortKeyValue::Text(lhs), SortKeyValue::Text(rhs)) => lhs.cmp(rhs),
            (SortKeyValue::Numeric(lhs), SortKeyValue::Numeric(rhs)) => lhs.cmp(rhs),
            _ => self.variant_ord().cmp(&other.variant_ord()),
        }
    }
}

#[cfg(feature = "app_api")]
impl PartialOrd for SortKeyValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(feature = "app_api")]
#[derive(Clone, Eq, PartialEq)]
struct SortKeyComponent {
    value: SortKeyValue,
    ascending: bool,
}

#[cfg(feature = "app_api")]
impl SortKeyComponent {
    fn asc<V: Into<SortKeyValue>>(value: V) -> Self {
        Self {
            value: value.into(),
            ascending: true,
        }
    }

    fn desc<V: Into<SortKeyValue>>(value: V) -> Self {
        Self {
            value: value.into(),
            ascending: false,
        }
    }
}

#[cfg(feature = "app_api")]
#[derive(Clone, Eq, PartialEq)]
struct MultiSortKey {
    components: Vec<SortKeyComponent>,
}

#[cfg(feature = "app_api")]
impl MultiSortKey {
    fn new(components: Vec<SortKeyComponent>) -> Self {
        Self { components }
    }

    fn push(&mut self, component: SortKeyComponent) {
        self.components.push(component);
    }

    fn is_empty(&self) -> bool {
        self.components.is_empty()
    }
}

#[cfg(feature = "app_api")]
impl Ord for MultiSortKey {
    fn cmp(&self, other: &Self) -> Ordering {
        for (lhs, rhs) in self.components.iter().zip(other.components.iter()) {
            let ord = if lhs.ascending {
                lhs.value.cmp(&rhs.value)
            } else {
                rhs.value.cmp(&lhs.value)
            };
            if !ord.is_eq() {
                return ord;
            }
        }
        self.components.len().cmp(&other.components.len())
    }
}

#[cfg(feature = "app_api")]
impl PartialOrd for MultiSortKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(feature = "app_api")]
impl<K: Ord, T> PartialEq for PageEntry<K, T> {
    fn eq(&self, other: &Self) -> bool {
        self.seq == other.seq && self.key == other.key
    }
}

#[cfg(feature = "app_api")]
impl<K: Ord, T> Eq for PageEntry<K, T> {}

#[cfg(feature = "app_api")]
impl<K: Ord, T> PartialOrd for PageEntry<K, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(feature = "app_api")]
impl<K: Ord, T> Ord for PageEntry<K, T> {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.key.cmp(&other.key) {
            Ordering::Equal => self.seq.cmp(&other.seq),
            ord => ord,
        }
    }
}

#[cfg(feature = "app_api")]
fn collect_page_streaming<K, T, I>(
    iter: I,
    offset: u64,
    limit: Option<u64>,
    cap: Option<u64>,
) -> (Vec<T>, usize)
where
    I: IntoIterator<Item = (K, T)>,
    K: Ord,
{
    let offset_usize = if offset > usize::MAX as u64 {
        usize::MAX
    } else {
        offset as usize
    };
    let limit_usize = limit
        .filter(|&lim| lim > 0)
        .map(|lim| cap.map_or(lim, |c| lim.min(c)))
        .map(|lim| lim.min(usize::MAX as u64) as usize);
    let page_cap = limit_usize.map(|lim| offset_usize.saturating_add(lim));

    let mut matched: usize = 0;
    let mut seq: usize = 0;
    let mut heap: BinaryHeap<PageEntry<K, T>> = BinaryHeap::new();
    let mut collected: Vec<PageEntry<K, T>> = Vec::new();

    for (key, item) in iter.into_iter() {
        let entry = PageEntry { key, seq, item };
        seq = seq.wrapping_add(1);
        matched = matched.saturating_add(1);
        if let Some(capacity) = page_cap {
            heap.push(entry);
            if heap.len() > capacity {
                heap.pop();
            }
        } else {
            collected.push(entry);
        }
    }

    let mut entries = if page_cap.is_some() {
        heap.into_vec()
    } else {
        collected
    };

    entries.sort_by(|a, b| match a.key.cmp(&b.key) {
        Ordering::Equal => a.seq.cmp(&b.seq),
        ord => ord,
    });

    let skip = offset_usize.min(entries.len());
    let mut page: Vec<T> = Vec::new();
    for entry in entries.into_iter().skip(skip) {
        if let Some(lim) = limit_usize {
            if page.len() >= lim {
                break;
            }
        }
        page.push(entry.item);
    }

    (page, matched)
}

#[cfg(all(test, feature = "app_api"))]
mod streaming_pager_tests {
    use super::{MultiSortKey, SortKeyComponent, collect_page_streaming};

    #[test]
    fn collects_expected_page_with_limit() {
        let (items, total) = collect_page_streaming((0..10).map(|i| (i, i)), 2, Some(3), None);
        assert_eq!(total, 10);
        assert_eq!(items, vec![2, 3, 4]);
    }

    #[test]
    fn respects_large_offset() {
        let (items, total) = collect_page_streaming((0..5).map(|i| (i, i)), 10, Some(2), None);
        assert_eq!(total, 5);
        assert!(items.is_empty());
    }

    #[test]
    fn collects_all_when_limit_absent() {
        let (items, total) = collect_page_streaming((0..3).map(|i| (i, i)), 1, None, None);
        assert_eq!(total, 3);
        assert_eq!(items, vec![1, 2]);
    }

    #[test]
    fn orders_multi_key_with_mixed_directions() {
        let data = vec![
            (
                MultiSortKey::new(vec![
                    SortKeyComponent::asc("alpha".to_string()),
                    SortKeyComponent::desc("2".to_string()),
                ]),
                "alpha-2",
            ),
            (
                MultiSortKey::new(vec![
                    SortKeyComponent::asc("alpha".to_string()),
                    SortKeyComponent::desc("3".to_string()),
                ]),
                "alpha-3",
            ),
            (
                MultiSortKey::new(vec![
                    SortKeyComponent::asc("beta".to_string()),
                    SortKeyComponent::desc("1".to_string()),
                ]),
                "beta-1",
            ),
        ];
        let (items, total) = collect_page_streaming(data, 0, None, None);
        assert_eq!(total, 3);
        assert_eq!(items, vec!["alpha-3", "alpha-2", "beta-1"]);
    }

    #[test]
    fn preserves_insertion_order_when_keys_equal() {
        let key = MultiSortKey::new(vec![SortKeyComponent::asc("same".to_string())]);
        let data = vec![(key.clone(), 1usize), (key.clone(), 2usize), (key, 3usize)];
        let (items, _) = collect_page_streaming(data, 0, None, None);
        assert_eq!(items, vec![1, 2, 3]);
    }
}

#[derive(Debug, crate::json_macros::JsonSerialize, norito::derive::NoritoSerialize)]
struct CountResponse {
    count: u64,
}

#[derive(Debug, crate::json_macros::JsonSerialize, norito::derive::NoritoSerialize)]
struct OkIdResponse {
    ok: bool,
    id: String,
}

/// GET /v1/sumeragi/pacemaker — snapshot of pacemaker timers and config
#[cfg(feature = "telemetry")]
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_pacemaker(
    telemetry: &MaybeTelemetry,
    accept: Option<axum::http::HeaderValue>,
) -> Result<Response> {
    if !telemetry.allows_developer_outputs() {
        return Err(Error::telemetry_profile_forbidden(
            "sumeragi_pacemaker",
            telemetry.profile(),
        ));
    }

    let m = telemetry.metrics().await;
    let payload = SumeragiPacemakerResponse {
        backoff_ms: m.sumeragi_pacemaker_backoff_ms.get(),
        rtt_floor_ms: m.sumeragi_pacemaker_rtt_floor_ms.get(),
        jitter_ms: m.sumeragi_pacemaker_jitter_ms.get(),
        backoff_multiplier: m.sumeragi_pacemaker_backoff_multiplier.get(),
        rtt_floor_multiplier: m.sumeragi_pacemaker_rtt_floor_multiplier.get(),
        max_backoff_ms: m.sumeragi_pacemaker_max_backoff_ms.get(),
        jitter_frac_permille: m.sumeragi_pacemaker_jitter_frac_permille.get(),
        round_elapsed_ms: m.sumeragi_pacemaker_round_elapsed_ms.get(),
        view_timeout_target_ms: m.sumeragi_pacemaker_view_timeout_target_ms.get(),
        view_timeout_remaining_ms: m.sumeragi_pacemaker_view_timeout_remaining_ms.get(),
    };
    let format = match crate::utils::negotiate_response_format(accept.as_ref()) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    Ok(crate::utils::respond_with_format(payload, format))
}

/// GET /v1/sumeragi/qc — HighestQC/LockedQC snapshot including subject hash if available
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_qc(accept: Option<axum::http::HeaderValue>) -> Result<Response> {
    let snap = sumeragi::status_snapshot();
    let format = match crate::utils::negotiate_response_format(accept.as_ref()) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };

    if matches!(format, crate::utils::ResponseFormat::Norito) {
        let highest_qc = SumeragiQcEntry {
            height: snap.highest_qc_height,
            view: snap.highest_qc_view,
            subject_block_hash: snap.highest_qc_subject,
        };
        let locked_qc = SumeragiQcEntry {
            height: snap.locked_qc_height,
            view: snap.locked_qc_view,
            subject_block_hash: snap.locked_qc_subject,
        };
        let wire = SumeragiQcSnapshot {
            highest_qc,
            locked_qc,
        };
        return Ok(crate::NoritoBody(wire).into_response());
    }
    let subject_value = snap
        .highest_qc_subject
        .map(|h| Value::from(format!("{h}")))
        .unwrap_or(Value::Null);
    let highest_qc = json_object(vec![
        json_entry("height", snap.highest_qc_height),
        json_entry("view", snap.highest_qc_view),
        json_entry("subject_block_hash", subject_value.clone()),
    ]);
    let locked_qc = json_object(vec![
        json_entry("height", snap.locked_qc_height),
        json_entry("view", snap.locked_qc_view),
        json_entry(
            "subject_block_hash",
            snap.locked_qc_subject
                .map(|h| Value::from(format!("{h}")))
                .unwrap_or(Value::Null),
        ),
    ]);
    let payload = json_object(vec![
        json_entry("highest_qc", highest_qc),
        json_entry("locked_qc", locked_qc),
    ]);
    let body = json::to_json_pretty(&payload).map_err(norito_internal_error)?;
    let mut resp = axum::response::Response::new(axum::body::Body::from(body));
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    Ok(resp)
}

/// GET /v1/sumeragi/phases — Compact JSON with latest per-phase latencies (ms)
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_phases(
    accept: Option<axum::http::HeaderValue>,
) -> Result<Response> {
    let snap = status::phase_latencies_snapshot();
    let payload = SumeragiPhasesResponse {
        propose_ms: snap.propose_ms,
        collect_da_ms: snap.collect_da_ms,
        collect_prevote_ms: snap.collect_prevote_ms,
        collect_precommit_ms: snap.collect_precommit_ms,
        collect_aggregator_ms: snap.collect_aggregator_ms,
        collect_exec_ms: snap.collect_exec_ms,
        collect_witness_ms: snap.collect_witness_ms,
        commit_ms: snap.commit_ms,
        pipeline_total_ms: snap.pipeline_total_ms,
        collect_aggregator_gossip_total: snap.gossip_fallback_total,
        block_created_dropped_by_lock_total: snap.block_created_dropped_by_lock_total,
        block_created_hint_mismatch_total: snap.block_created_hint_mismatch_total,
        block_created_proposal_mismatch_total: snap.block_created_proposal_mismatch_total,
        ema_ms: SumeragiPhasesEma {
            propose_ms: snap.propose_ema_ms,
            collect_da_ms: snap.collect_da_ema_ms,
            collect_prevote_ms: snap.collect_prevote_ema_ms,
            collect_precommit_ms: snap.collect_precommit_ema_ms,
            collect_aggregator_ms: snap.collect_aggregator_ema_ms,
            collect_exec_ms: snap.collect_exec_ema_ms,
            collect_witness_ms: snap.collect_witness_ema_ms,
            commit_ms: snap.commit_ema_ms,
            pipeline_total_ms: snap.pipeline_total_ema_ms,
        },
    };
    let format = match crate::utils::negotiate_response_format(accept.as_ref()) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    Ok(crate::utils::respond_with_format(payload, format))
}

/// GET /v1/sumeragi/bls_keys — map of network public keys -> BLS public keys (hex strings)
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_bls_keys(
    State(state): State<Arc<CoreState>>,
    accept: Option<axum::http::HeaderValue>,
) -> Result<Response> {
    // Debug/operator endpoint; non-consensus. Build mapping from world peers where identity key is BLS-normal.
    let view = state.view();
    let peers = view.world.peers().clone();
    drop(view);
    let mut obj: std::collections::BTreeMap<String, Option<String>> =
        std::collections::BTreeMap::new();
    for p in peers {
        let net_pk = p.public_key().to_string();
        let bls_pk_val = if p.public_key().algorithm() == iroha_crypto::Algorithm::BlsNormal {
            Some(p.public_key().to_string())
        } else {
            None
        };
        obj.insert(net_pk, bls_pk_val);
    }
    let format = match crate::utils::negotiate_response_format(accept.as_ref()) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    Ok(crate::utils::respond_with_format(obj, format))
}

/// GET /v1/sumeragi/leader — leader index snapshot; includes PRF context when available
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_leader(
    accept: Option<axum::http::HeaderValue>,
) -> Result<Response> {
    let snap = sumeragi::status_snapshot();
    let seed_opt = snap.prf_epoch_seed;
    let prf_h = snap.prf_height;
    let prf_v = snap.prf_view;
    let payload = SumeragiLeaderResponse {
        leader_index: snap.leader_index,
        prf: PrfContext {
            height: prf_h,
            view: prf_v,
            epoch_seed: seed_opt.map(hex::encode),
        },
    };
    let format = match crate::utils::negotiate_response_format(accept.as_ref()) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    Ok(crate::utils::respond_with_format(payload, format))
}

/// GET /v1/sumeragi/collectors — current collector indices and peers derived from topology and on-chain params
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_collectors(
    State(state): State<std::sync::Arc<CoreState>>,
    accept: Option<axum::http::HeaderValue>,
) -> Result<Response> {
    let view = state.view();
    let peers = view.commit_topology.clone();
    let topology = iroha_core::sumeragi::network_topology::Topology::new(peers.clone());
    let n = peers.len();
    let min_votes = topology.min_votes_for_commit();
    let tail = topology.proxy_tail_index();
    let available = n.saturating_sub(tail);
    let mode = sumeragi::effective_consensus_mode(&view, ConsensusMode::Permissioned);
    let (mut k_raw, redundant_send_r, seed_from_mode) = match mode {
        ConsensusMode::Permissioned => {
            let params = view.world.parameters().sumeragi();
            (
                params.collectors_k as usize,
                params.collectors_redundant_send_r,
                None,
            )
        }
        ConsensusMode::Npos => {
            if let Some(cfg) = sumeragi::load_npos_collector_config(&view) {
                (cfg.k, cfg.redundant_send_r, Some(cfg.seed))
            } else {
                let params = view.world.parameters().sumeragi();
                iroha_logger::warn!(
                    "Missing sumeragi_npos_parameters payload; falling back to permissioned collector settings"
                );
                (
                    params.collectors_k as usize,
                    params.collectors_redundant_send_r,
                    None,
                )
            }
        }
    };
    if k_raw == 0 {
        k_raw = 1;
    }
    let mut k = if available > 0 {
        k_raw.min(available)
    } else {
        0
    };
    if k == 0 && available > 0 {
        k = available;
    }
    let snap = sumeragi::status_snapshot();
    let mut epoch_seed = snap.prf_epoch_seed.or(seed_from_mode);
    if epoch_seed.is_none() {
        epoch_seed = view
            .world
            .sumeragi_npos_parameters()
            .map(|params| params.epoch_seed());
    }
    let plan_height = if snap.prf_height > 0 {
        snap.prf_height
    } else {
        view.height() as u64
    };
    let plan_view = snap.prf_view;
    let collectors = sumeragi::collectors::deterministic_collectors(
        &topology,
        mode,
        k,
        epoch_seed,
        plan_height,
        plan_view,
    );
    let collectors = collectors
        .iter()
        .filter_map(|peer| {
            topology
                .as_ref()
                .iter()
                .position(|p| p == peer)
                .map(|idx| CollectorEntry {
                    index: idx as u64,
                    peer_id: peer.to_string(),
                })
        })
        .collect::<Vec<_>>();
    let consensus_mode_label = match mode {
        ConsensusMode::Permissioned => "Permissioned",
        ConsensusMode::Npos => "Npos",
    };
    let epoch_seed_hex = epoch_seed.map(hex::encode);
    let payload = CollectorsResponse {
        consensus_mode: consensus_mode_label,
        mode: consensus_mode_label,
        topology_len: n as u64,
        min_votes_for_commit: min_votes as u64,
        proxy_tail_index: tail as u64,
        height: plan_height,
        view: plan_view,
        collectors_k: k as u64,
        redundant_send_r: u64::from(redundant_send_r),
        epoch_seed: epoch_seed_hex.clone(),
        collectors,
        prf: PrfContext {
            height: plan_height,
            view: plan_view,
            epoch_seed: epoch_seed_hex,
        },
    };
    drop(view);
    let format = match crate::utils::negotiate_response_format(accept.as_ref()) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    Ok(crate::utils::respond_with_format(payload, format))
}

/// GET /v1/sumeragi/params — snapshot of on-chain Sumeragi parameters
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_params(
    State(state): State<std::sync::Arc<CoreState>>,
    accept: Option<axum::http::HeaderValue>,
) -> Result<Response> {
    let view = state.view();
    let sp = view.world.parameters().sumeragi();
    let payload = SumeragiParamsResponse {
        block_time_ms: sp.block_time_ms,
        commit_time_ms: sp.commit_time_ms,
        max_clock_drift_ms: sp.max_clock_drift_ms,
        collectors_k: u64::from(sp.collectors_k),
        redundant_send_r: u64::from(sp.collectors_redundant_send_r),
        da_enabled: sp.da_enabled,
        next_mode: sp.next_mode.map(|m| match m {
            iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned => {
                "Permissioned"
            }
            iroha_data_model::parameter::system::SumeragiConsensusMode::Npos => "Npos",
        }),
        mode_activation_height: sp.mode_activation_height,
        chain_height: view.height() as u64,
    };
    drop(view);
    let format = match crate::utils::negotiate_response_format(accept.as_ref()) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    Ok(crate::utils::respond_with_format(payload, format))
}

/// GET /v1/sumeragi/evidence/count — returns the number of unique EvidenceV3 entries observed.
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_evidence_count(
    State(state): State<std::sync::Arc<CoreState>>,
    accept: Option<axum::http::HeaderValue>,
) -> Result<Response> {
    let view = state.view();
    let n = iroha_core::query::evidence_count(&view) as u64;
    drop(view);
    let format = match crate::utils::negotiate_response_format(accept.as_ref()) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    Ok(crate::utils::respond_with_format(
        CountResponse { count: n },
        format,
    ))
}

#[derive(
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
)]
pub struct EvidenceListQuery {
    /// Maximum number of entries to return (1..=1000). Default 50.
    pub limit: Option<usize>,
    /// Offset into the snapshot list. Default 0.
    pub offset: Option<usize>,
    /// Optional filter by kind: one of DoublePrevote, DoublePrecommit, DoubleExecVote, InvalidQC, InvalidProposal
    pub kind: Option<String>,
}

/// GET /v1/sumeragi/evidence — list recent evidence entries (in-memory audit snapshot).
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_evidence_list(
    State(state): State<std::sync::Arc<CoreState>>,
    crate::NoritoQuery(q): crate::NoritoQuery<EvidenceListQuery>,
    accept: Option<axum::http::HeaderValue>,
) -> Result<Response> {
    let view = state.view();
    let mut records = iroha_core::query::evidence_list_snapshot(&view);
    // Optional kind filter
    if let Some(kind_s) = q.kind.as_deref() {
        use iroha_core::sumeragi::consensus::EvidenceKind;
        let kind_opt = match kind_s {
            "DoublePrevote" => Some(EvidenceKind::DoublePrevote),
            "DoublePrecommit" => Some(EvidenceKind::DoublePrecommit),
            "DoubleExecVote" => Some(EvidenceKind::DoubleExecVote),
            "InvalidQC" => Some(EvidenceKind::InvalidQC),
            "InvalidProposal" => Some(EvidenceKind::InvalidProposal),
            _ => None,
        };
        if let Some(k) = kind_opt {
            records.retain(|rec| rec.evidence.kind == k);
        }
    }
    // Apply offset/limit
    let offset = q.offset.unwrap_or(0);
    let limit = q.limit.unwrap_or(50).clamp(1, 1000);
    let total = records.len();
    let slice = if offset >= total {
        &[][..]
    } else {
        let end = core::cmp::min(total, offset + limit);
        &records[offset..end]
    };
    let format = match crate::utils::negotiate_response_format(accept.as_ref()) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };

    if matches!(format, crate::utils::ResponseFormat::Norito) {
        let wire = EvidenceListWire {
            total: total as u64,
            items: slice.to_vec(),
        };
        return Ok(crate::NoritoBody(wire).into_response());
    }
    // Map to Norito-JSON response
    let items: Vec<norito::json::Value> = slice.iter().map(evidence_to_json).collect();
    let payload = json_object(vec![
        json_entry("total", total as u64),
        json_entry("items", items),
    ]);
    let body = json::to_json_pretty(&payload).map_err(norito_internal_error)?;
    let mut resp = axum::response::Response::new(axum::body::Body::from(body));
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    Ok(resp)
}

fn hash_to_hex<H>(hash: H) -> String
where
    H: AsRef<[u8; iroha_crypto::Hash::LENGTH]>,
{
    hex::encode(hash.as_ref())
}

#[derive(
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
)]
pub struct EvidenceSubmitRequestDto {
    pub evidence_hex: String,
}

/// Handle POST `/v1/sumeragi/evidence/submit`, validating and forwarding consensus evidence.
pub fn handle_post_sumeragi_evidence_submit(
    sumeragi: SumeragiHandle,
    request: EvidenceSubmitRequestDto,
    state: &iroha_core::state::State,
    chain_id: &ChainId,
) -> Result<axum::response::Response, Error> {
    let evidence = decode_and_validate_evidence(&request.evidence_hex, state, chain_id)?;
    let kind = evidence.kind;
    sumeragi.incoming_consensus_control_flow_message(ControlFlow::Evidence(evidence));
    let payload = json_object(vec![
        json_entry("status", "accepted"),
        json_entry("kind", format!("{kind:?}")),
    ]);
    let body = norito::json::to_json_pretty(&payload).map_err(norito_internal_error)?;
    let mut resp = axum::response::Response::new(axum::body::Body::from(body));
    *resp.status_mut() = StatusCode::ACCEPTED;
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    Ok(resp)
}

fn decode_evidence_hex(value: &str) -> Result<ConsensusEvidence, Error> {
    use norito::codec::DecodeAll as _;

    let trimmed = value.trim();
    let body = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    let bytes = hex::decode(body).map_err(|err| {
        Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                "evidence_hex: {err}"
            )),
        ))
    })?;
    let mut slice: &[u8] = &bytes;
    ConsensusEvidence::decode_all(&mut slice).map_err(|err| {
        Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                "evidence_hex decode: {err}"
            )),
        ))
    })
}

fn decode_and_validate_evidence(
    value: &str,
    state: &iroha_core::state::State,
    chain_id: &ChainId,
) -> Result<ConsensusEvidence, Error> {
    let evidence = decode_evidence_hex(value)?;
    let view = state.view();
    let topology_peers: Vec<_> = view.commit_topology().iter().cloned().collect();
    let (subject_height, _) =
        iroha_core::sumeragi::evidence_subject_height_view(&evidence);
    let npos_seed = if view.world.sumeragi_npos_parameters().is_some() {
        let height = subject_height.unwrap_or_else(|| u64::try_from(view.height()).unwrap_or(0));
        Some(iroha_core::sumeragi::npos_seed_for_height(&view, height))
    } else {
        None
    };
    drop(view);
    if topology_peers.is_empty() {
        return Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(
                "invalid consensus evidence: commit topology unavailable".to_owned(),
            ),
        )));
    }
    let topology = iroha_core::sumeragi::network_topology::Topology::new(topology_peers);
    let mut errors = Vec::new();
    for mode_tag in [
        iroha_core::sumeragi::consensus::PERMISSIONED_TAG,
        iroha_core::sumeragi::consensus::NPOS_TAG,
    ] {
        let context = iroha_core::sumeragi::EvidenceValidationContext {
            topology: &topology,
            chain_id,
            mode_tag,
            prf_seed: if mode_tag == iroha_core::sumeragi::consensus::NPOS_TAG {
                npos_seed
            } else {
                None
            },
        };
        match iroha_core::sumeragi::validate_evidence(&evidence, &context) {
            Ok(()) => return Ok(evidence),
            Err(err) => errors.push(format!("{mode_tag}: {err}")),
        }
    }
    let detail = errors.join("; ");
    Err(Error::Query(iroha_data_model::ValidationFail::QueryFailed(
        iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
            "invalid consensus evidence: {detail}"
        )),
    )))
}

#[cfg(test)]
mod evidence_submit_tests {
    use super::*;
    use iroha_core::sumeragi::consensus::{Evidence, EvidenceKind, EvidencePayload, Phase, Vote};
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::{
        block::BlockHeader,
        consensus::VrfEpochRecord,
        parameter::system::SumeragiNposParameters,
        peer::PeerId,
        prelude::ChainId,
    };
    use norito::codec::Encode as _;

    fn test_state_with_peer(peer: PeerId) -> iroha_core::state::State {
        let kura = iroha_core::kura::Kura::blank_kura_for_testing();
        let query = iroha_core::query::store::LiveQueryStore::start_test();
        let state = iroha_core::state::State::new_for_testing(
            iroha_core::state::World::default(),
            kura,
            query,
        );
        let mut block = state.commit_topology.block();
        block.push(peer);
        block.commit();
        state
    }

    fn make_vote(
        chain_id: &ChainId,
        mode_tag: &str,
        keypair: &KeyPair,
        height: u64,
        view: u64,
        seed: u8,
    ) -> Vote {
        let hash = Hash::prehashed([seed; 32]);
        let mut vote = Vote {
            phase: Phase::Prevote,
            block_hash: HashOf::from_untyped_unchecked(hash),
            height,
            view,
            epoch: 0,
            signer: 0,
            bls_sig: Vec::new(),
            signature: Vec::new(),
        };
        let preimage = iroha_core::sumeragi::consensus::vote_preimage(chain_id, mode_tag, &vote);
        let signature = Signature::new(keypair.private_key(), &preimage);
        let payload = signature.payload().to_vec();
        vote.signature = payload.clone();
        vote.bls_sig = payload;
        vote
    }

    fn sample_evidence(chain_id: &ChainId, keypair: &KeyPair) -> Evidence {
        let mode_tag = iroha_core::sumeragi::consensus::PERMISSIONED_TAG;
        let v1 = make_vote(chain_id, mode_tag, keypair, 10, 3, 0x11);
        let v2 = make_vote(chain_id, mode_tag, keypair, 10, 3, 0x22);
        Evidence {
            kind: EvidenceKind::DoublePrevote,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        }
    }

    #[test]
    fn decode_evidence_hex_accepts_plain_and_prefixed() {
        let chain_id: ChainId = "torii-evidence".parse().expect("chain id parses");
        let keypair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let ev = sample_evidence(&chain_id, &keypair);
        let encoded = norito::codec::Encode::encode(&ev);
        let plain = hex::encode(&encoded);
        let prefixed = format!("0x{plain}");

        let decoded_plain = decode_evidence_hex(&plain).expect("decode plain hex");
        let decoded_prefixed = decode_evidence_hex(&prefixed).expect("decode 0x hex");

        assert_eq!(decoded_plain.kind, EvidenceKind::DoublePrevote);
        assert_eq!(decoded_prefixed.kind, EvidenceKind::DoublePrevote);
    }

    #[test]
    fn decode_evidence_hex_rejects_invalid_hex() {
        let err = decode_evidence_hex("not-a-hex").expect_err("expect error");
        assert!(matches!(
            err,
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(_)
            ))
        ));
    }

    #[test]
    fn decode_evidence_hex_rejects_truncated_payload() {
        let truncated = hex::encode([0x01u8, 0x02u8]);
        let err = decode_evidence_hex(&truncated).expect_err("expect decode failure");
        assert!(matches!(
            err,
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(_)
            ))
        ));
    }

    #[test]
    fn decode_and_validate_evidence_rejects_structurally_invalid_payload() {
        let chain_id: ChainId = "torii-evidence".parse().expect("chain id parses");
        let keypair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let state = test_state_with_peer(PeerId::new(keypair.public_key().clone()));
        let mode_tag = iroha_core::sumeragi::consensus::PERMISSIONED_TAG;
        let vote = make_vote(&chain_id, mode_tag, &keypair, 42, 7, 0xAB);
        let forged = Evidence {
            kind: EvidenceKind::DoublePrevote,
            payload: EvidencePayload::DoubleVote {
                v1: vote.clone(),
                v2: vote,
            },
        };
        let encoded = hex::encode(forged.encode());
        let err =
            decode_and_validate_evidence(&encoded, &state, &chain_id).expect_err("invalid evidence must fail");
        assert!(matches!(
            err,
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(_)
            ))
        ));
    }

    #[test]
    fn decode_and_validate_evidence_uses_subject_height_seed() {
        let chain_id: ChainId = "torii-evidence".parse().expect("chain id parses");
        let keypair0 = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let keypair1 = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer0 = PeerId::new(keypair0.public_key().clone());
        let peer1 = PeerId::new(keypair1.public_key().clone());
        let topology = iroha_core::sumeragi::network_topology::Topology::new(vec![
            peer0.clone(),
            peer1.clone(),
        ]);
        let height = 1_u64;
        let view = 0_u64;
        // Find two seeds that map to different leaders for the same (height, view).
        let mut seed_epoch0 = None;
        let mut seed_epoch1 = None;
        let mut leader_epoch0 = 0usize;
        for byte in 0u8..=u8::MAX {
            let seed = [byte; 32];
            let leader = topology.leader_index_prf(seed, height, view);
            if seed_epoch0.is_none() {
                seed_epoch0 = Some(seed);
                leader_epoch0 = leader;
                continue;
            }
            if leader != leader_epoch0 {
                seed_epoch1 = Some(seed);
                break;
            }
        }
        let seed_epoch0 = seed_epoch0.expect("seed for epoch 0");
        let seed_epoch1 = seed_epoch1.expect("seed for epoch 1");
        let leader_epoch1 = topology.leader_index_prf(seed_epoch1, height, view);
        assert_ne!(leader_epoch0, leader_epoch1, "seed search must pick distinct leaders");

        let signer_keypair = if leader_epoch0 == 0 {
            &keypair0
        } else {
            &keypair1
        };

        let mut world = iroha_core::state::World::default();
        {
            let mut block = world.block();
            let params = SumeragiNposParameters {
                epoch_length_blocks: 1,
                ..SumeragiNposParameters::default()
            };
            block.parameters.get_mut().custom.insert(
                SumeragiNposParameters::parameter_id(),
                params.into_custom_parameter(),
            );
            block.vrf_epochs_mut_for_testing().insert(
                0,
                VrfEpochRecord {
                    epoch: 0,
                    seed: seed_epoch0,
                    epoch_length: 1,
                    commit_deadline_offset: 0,
                    reveal_deadline_offset: 0,
                    roster_len: 2,
                    finalized: false,
                    updated_at_height: 0,
                    participants: Vec::new(),
                    late_reveals: Vec::new(),
                    committed_no_reveal: Vec::new(),
                    no_participation: Vec::new(),
                    penalties_applied: false,
                    penalties_applied_at_height: None,
                    validator_election: None,
                },
            );
            block.vrf_epochs_mut_for_testing().insert(
                1,
                VrfEpochRecord {
                    epoch: 1,
                    seed: seed_epoch1,
                    epoch_length: 1,
                    commit_deadline_offset: 0,
                    reveal_deadline_offset: 0,
                    roster_len: 2,
                    finalized: false,
                    updated_at_height: 1,
                    participants: Vec::new(),
                    late_reveals: Vec::new(),
                    committed_no_reveal: Vec::new(),
                    no_participation: Vec::new(),
                    penalties_applied: false,
                    penalties_applied_at_height: None,
                    validator_election: None,
                },
            );
            block.commit();
        }
        let kura = iroha_core::kura::Kura::blank_kura_for_testing();
        let query = iroha_core::query::store::LiveQueryStore::start_test();
        let state = iroha_core::state::State::new_for_testing(world, kura, query);
        {
            let mut block = state.commit_topology.block();
            block.push(peer0);
            block.push(peer1);
            block.commit();
        }

        let mode_tag = iroha_core::sumeragi::consensus::NPOS_TAG;
        let v1 = make_vote(&chain_id, mode_tag, signer_keypair, height, view, 0x11);
        let v2 = make_vote(&chain_id, mode_tag, signer_keypair, height, view, 0x22);
        let ev = Evidence {
            kind: EvidenceKind::DoublePrevote,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        let encoded = hex::encode(ev.encode());
        let decoded = decode_and_validate_evidence(&encoded, &state, &chain_id)
            .expect("evidence should validate with subject-height seed");
        assert_eq!(decoded.kind, EvidenceKind::DoublePrevote);
    }
}

#[cfg(feature = "app_api")]
/// GET /v1/sumeragi/new_view/sse — SSE stream of NEW_VIEW counts polled periodically.
pub fn handle_v1_new_view_sse(
    poll_ms: u64,
) -> Sse<impl futures::Stream<Item = Result<SseEvent, Infallible>>> {
    let interval = Duration::from_millis(poll_ms.max(100));
    let ticker = tokio::time::interval(interval);
    let stream = stream::unfold(ticker, move |mut ticker| async move {
        ticker.tick().await;
        // Snapshot counts from core
        let items = iroha_core::sumeragi::new_view_snapshot_counts();
        let arr: Vec<norito::json::Value> = items
            .into_iter()
            .map(|(h, v, c)| {
                crate::json_object(vec![
                    json_entry("height", h),
                    json_entry("view", v),
                    json_entry("count", c),
                ])
            })
            .collect();
        let status = fetch_network_time_status().await;
        let ts_ms = status
            .now
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let payload =
            crate::json_object(vec![json_entry("ts_ms", ts_ms), json_entry("items", arr)]);
        let body = norito::json::to_json(&payload).unwrap_or_else(|_| "{}".to_owned());
        let ev = SseEvent::default().data(body);
        Some((Ok(ev), ticker))
    });
    Sse::new(stream)
}

/// Telemetry JSON snapshot for NEW_VIEW counters.
#[iroha_futures::telemetry_future]
pub async fn handle_v1_new_view_json() -> Result<impl IntoResponse> {
    let items = iroha_core::sumeragi::new_view_snapshot_counts();
    let arr: Vec<norito::json::Value> = items
        .into_iter()
        .map(|(h, v, c)| {
            crate::json_object(vec![
                json_entry("height", h),
                json_entry("view", v),
                json_entry("count", c),
            ])
        })
        .collect();
    let status = fetch_network_time_status().await;
    let ts_ms = status
        .now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let payload = crate::json_object(vec![json_entry("ts_ms", ts_ms), json_entry("items", arr)]);
    let body = norito::json::to_json_pretty(&payload).map_err(|e| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(
            e.to_string(),
        ))
    })?;
    let mut resp = axum::response::Response::new(axum::body::Body::from(body));
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    Ok(resp)
}

fn settlement_order_label(
    order: iroha_data_model::isi::settlement::SettlementExecutionOrder,
) -> &'static str {
    match order {
        iroha_data_model::isi::settlement::SettlementExecutionOrder::DeliveryThenPayment => {
            "delivery_then_payment"
        }
        iroha_data_model::isi::settlement::SettlementExecutionOrder::PaymentThenDelivery => {
            "payment_then_delivery"
        }
    }
}

fn settlement_atomicity_label(
    atomicity: iroha_data_model::isi::settlement::SettlementAtomicity,
) -> &'static str {
    match atomicity {
        iroha_data_model::isi::settlement::SettlementAtomicity::AllOrNothing => "all_or_nothing",
        iroha_data_model::isi::settlement::SettlementAtomicity::CommitFirstLeg => {
            "commit_first_leg"
        }
        iroha_data_model::isi::settlement::SettlementAtomicity::CommitSecondLeg => {
            "commit_second_leg"
        }
    }
}

fn settlement_counts_to_value(
    map: &std::collections::BTreeMap<String, u64>,
) -> norito::json::Value {
    let mut obj = norito::json::Map::new();
    for (key, value) in map {
        obj.insert(key.clone(), norito::json::Value::from(*value));
    }
    norito::json::Value::Object(obj)
}

fn dvp_last_event_json(
    event: &sumeragi::status::DvpSettlementEventSnapshot,
) -> norito::json::Value {
    let settlement_id = event
        .settlement_id
        .as_ref()
        .map(|s| norito::json::Value::from(s.clone()))
        .unwrap_or(norito::json::Value::Null);
    let failure_reason = event
        .failure_reason
        .as_ref()
        .map(|s| norito::json::Value::from(s.clone()))
        .unwrap_or(norito::json::Value::Null);
    let plan = json_object(vec![
        json_entry("order", settlement_order_label(event.plan_order)),
        json_entry(
            "atomicity",
            settlement_atomicity_label(event.plan_atomicity),
        ),
    ]);
    let legs = json_object(vec![
        json_entry("delivery_committed", event.delivery_committed),
        json_entry("payment_committed", event.payment_committed),
    ]);
    json_object(vec![
        json_entry("observed_at_ms", event.observed_at_ms),
        json_entry("settlement_id", settlement_id),
        json_entry("plan", plan),
        json_entry("outcome", event.outcome.as_str()),
        json_entry("failure_reason", failure_reason),
        json_entry("final_state", event.final_state_label.clone()),
        json_entry("legs", legs),
    ])
}

fn pvp_last_event_json(
    event: &sumeragi::status::PvpSettlementEventSnapshot,
) -> norito::json::Value {
    let settlement_id = event
        .settlement_id
        .as_ref()
        .map(|s| norito::json::Value::from(s.clone()))
        .unwrap_or(norito::json::Value::Null);
    let failure_reason = event
        .failure_reason
        .as_ref()
        .map(|s| norito::json::Value::from(s.clone()))
        .unwrap_or(norito::json::Value::Null);
    let plan = json_object(vec![
        json_entry("order", settlement_order_label(event.plan_order)),
        json_entry(
            "atomicity",
            settlement_atomicity_label(event.plan_atomicity),
        ),
    ]);
    let legs = json_object(vec![
        json_entry("primary_committed", event.primary_committed),
        json_entry("counter_committed", event.counter_committed),
    ]);
    let fx_window = event
        .fx_window_ms
        .map(norito::json::Value::from)
        .unwrap_or(norito::json::Value::Null);
    json_object(vec![
        json_entry("observed_at_ms", event.observed_at_ms),
        json_entry("settlement_id", settlement_id),
        json_entry("plan", plan),
        json_entry("outcome", event.outcome.as_str()),
        json_entry("failure_reason", failure_reason),
        json_entry("final_state", event.final_state_label.clone()),
        json_entry("legs", legs),
        json_entry("fx_window_ms", fx_window),
    ])
}

fn settlement_snapshot_value(
    settlement: &sumeragi::status::SettlementStatusSnapshot,
) -> norito::json::Value {
    let dvp_last = settlement
        .dvp
        .last_event
        .as_ref()
        .map(dvp_last_event_json)
        .unwrap_or(norito::json::Value::Null);
    let pvp_last = settlement
        .pvp
        .last_event
        .as_ref()
        .map(pvp_last_event_json)
        .unwrap_or(norito::json::Value::Null);
    let dvp = json_object(vec![
        json_entry("success_total", settlement.dvp.success_total),
        json_entry("failure_total", settlement.dvp.failure_total),
        json_entry(
            "final_state_totals",
            settlement_counts_to_value(&settlement.dvp.final_state_totals),
        ),
        json_entry(
            "failure_reasons",
            settlement_counts_to_value(&settlement.dvp.failure_reasons),
        ),
        json_entry("last_event", dvp_last),
    ]);
    let pvp = json_object(vec![
        json_entry("success_total", settlement.pvp.success_total),
        json_entry("failure_total", settlement.pvp.failure_total),
        json_entry(
            "final_state_totals",
            settlement_counts_to_value(&settlement.pvp.final_state_totals),
        ),
        json_entry(
            "failure_reasons",
            settlement_counts_to_value(&settlement.pvp.failure_reasons),
        ),
        json_entry("last_event", pvp_last),
    ]);
    json_object(vec![json_entry("dvp", dvp), json_entry("pvp", pvp)])
}

fn nexus_fee_snapshot_value(fee: &sumeragi::status::NexusFeeSnapshot) -> Value {
    json_object(vec![
        json_entry("charged_total", fee.charged_total),
        json_entry("charged_via_payer_total", fee.charged_via_payer_total),
        json_entry("charged_via_sponsor_total", fee.charged_via_sponsor_total),
        json_entry("sponsor_disabled_total", fee.sponsor_disabled_total),
        json_entry("sponsor_cap_exceeded_total", fee.sponsor_cap_exceeded_total),
        json_entry("config_errors_total", fee.config_errors_total),
        json_entry("transfer_failures_total", fee.transfer_failures_total),
        json_entry(
            "last_amount",
            fee.last_amount
                .map(|amount| Value::from(format!("{amount}")))
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "last_asset_id",
            fee.last_asset_id
                .clone()
                .map(Value::from)
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "last_payer",
            fee.last_payer
                .map(|payer| match payer {
                    sumeragi::status::NexusFeePayer::Payer => Value::from("payer"),
                    sumeragi::status::NexusFeePayer::Sponsor => Value::from("sponsor"),
                })
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "last_payer_id",
            fee.last_payer_id
                .clone()
                .map(Value::from)
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "last_error",
            fee.last_error
                .clone()
                .map(Value::from)
                .unwrap_or(Value::Null),
        ),
    ])
}

fn nexus_staking_snapshot_value(staking: &sumeragi::status::NexusStakingSnapshot) -> Value {
    let lanes = Value::Array(
        staking
            .lanes
            .iter()
            .map(|lane| {
                json_object(vec![
                    json_entry("lane_id", lane.lane_id.as_u32()),
                    json_entry("bonded", lane.bonded.to_string()),
                    json_entry("pending_unbond", lane.pending_unbond.to_string()),
                    json_entry("slash_total", lane.slash_total),
                ])
            })
            .collect(),
    );
    json_object(vec![json_entry("lanes", lanes)])
}

fn status_snapshot_json(snap: &sumeragi::StatusSnapshot) -> norito::json::Value {
    let highest_qc = json_object(vec![
        json_entry("height", snap.highest_qc_height),
        json_entry("view", snap.highest_qc_view),
        json_entry(
            "subject_block_hash",
            snap.highest_qc_subject
                .map(|h| Value::from(format!("{h}")))
                .unwrap_or(Value::Null),
        ),
    ]);
    let locked_qc = json_object(vec![
        json_entry("height", snap.locked_qc_height),
        json_entry("view", snap.locked_qc_view),
        json_entry(
            "subject_block_hash",
            snap.locked_qc_subject
                .map(|h| Value::from(format!("{h}")))
                .unwrap_or(Value::Null),
        ),
    ]);
    let view_change_causes = json_object(vec![
        json_entry(
            "commit_failure_total",
            snap.view_change_causes.commit_failure_total,
        ),
        json_entry(
            "quorum_timeout_total",
            snap.view_change_causes.quorum_timeout_total,
        ),
        json_entry("da_gate_total", snap.view_change_causes.da_gate_total),
        json_entry(
            "missing_payload_total",
            snap.view_change_causes.missing_payload_total,
        ),
        json_entry("missing_qc_total", snap.view_change_causes.missing_qc_total),
        json_entry(
            "validation_reject_total",
            snap.view_change_causes.validation_reject_total,
        ),
        json_entry(
            "last_cause",
            snap.view_change_causes
                .last_cause
                .clone()
                .map(Value::from)
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "last_cause_timestamp_ms",
            snap.view_change_causes.last_cause_timestamp_ms,
        ),
        json_entry(
            "last_commit_failure_timestamp_ms",
            snap.view_change_causes.last_commit_failure_timestamp_ms,
        ),
        json_entry(
            "last_quorum_timeout_timestamp_ms",
            snap.view_change_causes.last_quorum_timeout_timestamp_ms,
        ),
        json_entry(
            "last_da_gate_timestamp_ms",
            snap.view_change_causes.last_da_gate_timestamp_ms,
        ),
        json_entry(
            "last_missing_payload_timestamp_ms",
            snap.view_change_causes.last_missing_payload_timestamp_ms,
        ),
        json_entry(
            "last_missing_qc_timestamp_ms",
            snap.view_change_causes.last_missing_qc_timestamp_ms,
        ),
        json_entry(
            "last_validation_reject_timestamp_ms",
            snap.view_change_causes.last_validation_reject_timestamp_ms,
        ),
    ]);
    let validation_rejects = json_object(vec![
        json_entry("total", snap.validation_rejects.total),
        json_entry("stateless_total", snap.validation_rejects.stateless_total),
        json_entry("execution_total", snap.validation_rejects.execution_total),
        json_entry("prev_hash_total", snap.validation_rejects.prev_hash_total),
        json_entry(
            "prev_height_total",
            snap.validation_rejects.prev_height_total,
        ),
        json_entry("topology_total", snap.validation_rejects.topology_total),
        json_entry(
            "last_reason",
            snap.validation_rejects
                .last_reason
                .map(Value::from)
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "last_height",
            snap.validation_rejects
                .last_height
                .map(Value::from)
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "last_view",
            snap.validation_rejects
                .last_view
                .map(Value::from)
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "last_block",
            snap.validation_rejects
                .last_block
                .map(|hash| Value::from(format!("{hash}")))
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "last_timestamp_ms",
            snap.validation_rejects.last_timestamp_ms,
        ),
    ]);
    let settlement = settlement_snapshot_value(&snap.settlement);
    let dedup_evictions = json_object(vec![
        json_entry(
            "vote_capacity_total",
            snap.dedup_evictions.vote_capacity_total,
        ),
        json_entry("vote_expired_total", snap.dedup_evictions.vote_expired_total),
        json_entry(
            "block_created_capacity_total",
            snap.dedup_evictions.block_created_capacity_total,
        ),
        json_entry(
            "block_created_expired_total",
            snap.dedup_evictions.block_created_expired_total,
        ),
        json_entry(
            "proposal_capacity_total",
            snap.dedup_evictions.proposal_capacity_total,
        ),
        json_entry(
            "proposal_expired_total",
            snap.dedup_evictions.proposal_expired_total,
        ),
        json_entry(
            "rbc_ready_capacity_total",
            snap.dedup_evictions.rbc_ready_capacity_total,
        ),
        json_entry(
            "rbc_ready_expired_total",
            snap.dedup_evictions.rbc_ready_expired_total,
        ),
        json_entry(
            "rbc_deliver_capacity_total",
            snap.dedup_evictions.rbc_deliver_capacity_total,
        ),
        json_entry(
            "rbc_deliver_expired_total",
            snap.dedup_evictions.rbc_deliver_expired_total,
        ),
    ]);
    let tx_queue = json_object(vec![
        json_entry("depth", snap.tx_queue_depth),
        json_entry("capacity", snap.tx_queue_capacity),
        json_entry("saturated", snap.tx_queue_saturated),
    ]);
    let queue_depths_value = |depths: &sumeragi::status::WorkerQueueDepthSnapshot| {
        json_object(vec![
            json_entry("vote_rx", depths.vote_rx),
            json_entry("block_payload_rx", depths.block_payload_rx),
            json_entry("rbc_chunk_rx", depths.rbc_chunk_rx),
            json_entry("block_rx", depths.block_rx),
            json_entry("consensus_rx", depths.consensus_rx),
            json_entry("lane_relay_rx", depths.lane_relay_rx),
            json_entry("background_rx", depths.background_rx),
        ])
    };
    let queue_totals_value = |totals: &sumeragi::status::WorkerQueueTotalsSnapshot| {
        json_object(vec![
            json_entry("vote_rx", totals.vote_rx),
            json_entry("block_payload_rx", totals.block_payload_rx),
            json_entry("rbc_chunk_rx", totals.rbc_chunk_rx),
            json_entry("block_rx", totals.block_rx),
            json_entry("consensus_rx", totals.consensus_rx),
            json_entry("lane_relay_rx", totals.lane_relay_rx),
            json_entry("background_rx", totals.background_rx),
        ])
    };
    let worker_queue_depths = queue_depths_value(&snap.worker_loop.queue_depths);
    let worker_queue_diagnostics = json_object(vec![
        json_entry(
            "blocked_total",
            queue_totals_value(&snap.worker_loop.queue_diagnostics.blocked_total),
        ),
        json_entry(
            "blocked_ms_total",
            queue_totals_value(&snap.worker_loop.queue_diagnostics.blocked_ms_total),
        ),
        json_entry(
            "blocked_max_ms",
            queue_totals_value(&snap.worker_loop.queue_diagnostics.blocked_max_ms),
        ),
        json_entry(
            "dropped_total",
            queue_totals_value(&snap.worker_loop.queue_diagnostics.dropped_total),
        ),
    ]);
    let commit_pause_queue_depths =
        queue_depths_value(&snap.commit_inflight.pause_queue_depths);
    let commit_resume_queue_depths =
        queue_depths_value(&snap.commit_inflight.resume_queue_depths);
    let worker_loop = json_object(vec![
        json_entry("stage", snap.worker_loop.stage.as_str()),
        json_entry("stage_started_ms", snap.worker_loop.stage_started_ms),
        json_entry("last_iteration_ms", snap.worker_loop.last_iteration_ms),
        json_entry("queue_depths", worker_queue_depths),
        json_entry("queue_diagnostics", worker_queue_diagnostics),
    ]);
    let commit_inflight = json_object(vec![
        json_entry("active", snap.commit_inflight.active),
        json_entry("id", snap.commit_inflight.id),
        json_entry("height", snap.commit_inflight.height),
        json_entry("view", snap.commit_inflight.view),
        json_entry(
            "block_hash",
            snap.commit_inflight
                .block_hash
                .map(|hash| Value::from(format!("{hash}")))
                .unwrap_or(Value::Null),
        ),
        json_entry("started_ms", snap.commit_inflight.started_ms),
        json_entry("elapsed_ms", snap.commit_inflight.elapsed_ms),
        json_entry("timeout_ms", snap.commit_inflight.timeout_ms),
        json_entry("timeout_total", snap.commit_inflight.timeout_total),
        json_entry(
            "last_timeout_timestamp_ms",
            snap.commit_inflight.last_timeout_timestamp_ms,
        ),
        json_entry(
            "last_timeout_elapsed_ms",
            snap.commit_inflight.last_timeout_elapsed_ms,
        ),
        json_entry("last_timeout_height", snap.commit_inflight.last_timeout_height),
        json_entry("last_timeout_view", snap.commit_inflight.last_timeout_view),
        json_entry(
            "last_timeout_block_hash",
            snap.commit_inflight
                .last_timeout_block_hash
                .map(|hash| Value::from(format!("{hash}")))
                .unwrap_or(Value::Null),
        ),
        json_entry("pause_total", snap.commit_inflight.pause_total),
        json_entry("resume_total", snap.commit_inflight.resume_total),
        json_entry("paused_since_ms", snap.commit_inflight.paused_since_ms),
        json_entry("pause_queue_depths", commit_pause_queue_depths),
        json_entry("resume_queue_depths", commit_resume_queue_depths),
    ]);
    let kura_store = json_object(vec![
        json_entry("failures_total", snap.kura_store.failures_total),
        json_entry("abort_total", snap.kura_store.abort_total),
        json_entry("stage_total", snap.kura_store.stage_total),
        json_entry("rollback_total", snap.kura_store.rollback_total),
        json_entry("stage_last_height", snap.kura_store.stage_last_height),
        json_entry("stage_last_view", snap.kura_store.stage_last_view),
        json_entry(
            "stage_last_hash",
            snap.kura_store
                .stage_last_hash
                .map(|hash| Value::from(format!("{hash}")))
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "rollback_last_height",
            snap.kura_store.rollback_last_height,
        ),
        json_entry("rollback_last_view", snap.kura_store.rollback_last_view),
        json_entry(
            "rollback_last_hash",
            snap.kura_store
                .rollback_last_hash
                .map(|hash| Value::from(format!("{hash}")))
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "rollback_last_reason",
            snap.kura_store
                .rollback_last_reason
                .map(Value::from)
                .unwrap_or(Value::Null),
        ),
        json_entry("lock_reset_total", snap.kura_store.lock_reset_total),
        json_entry(
            "lock_reset_last_height",
            snap.kura_store.lock_reset_last_height,
        ),
        json_entry(
            "lock_reset_last_view",
            snap.kura_store.lock_reset_last_view,
        ),
        json_entry(
            "lock_reset_last_hash",
            snap.kura_store
                .lock_reset_last_hash
                .map(|hash| Value::from(format!("{hash}")))
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "lock_reset_last_reason",
            snap.kura_store
                .lock_reset_last_reason
                .map(Value::from)
                .unwrap_or(Value::Null),
        ),
        json_entry("last_retry_attempt", snap.kura_store.last_retry_attempt),
        json_entry(
            "last_retry_backoff_ms",
            snap.kura_store.last_retry_backoff_ms,
        ),
        json_entry("last_height", snap.kura_store.last_height),
        json_entry("last_view", snap.kura_store.last_view),
        json_entry(
            "last_hash",
            snap.kura_store
                .last_hash
                .map(|hash| Value::from(format!("{hash}")))
                .unwrap_or(Value::Null),
        ),
    ]);
    let da_gate = json_object(vec![
        json_entry("reason", snap.da_gate.reason.as_str()),
        json_entry("last_satisfied", snap.da_gate.last_satisfied.as_str()),
        json_entry(
            "missing_availability_total",
            snap.da_gate.missing_availability_total,
        ),
        json_entry("manifest_guard_total", snap.da_gate.manifest_guard_total),
    ]);
    let missing_block_fetch = json_object(vec![
        json_entry("total", snap.missing_block_fetch_total),
        json_entry("last_targets", snap.missing_block_fetch_last_targets),
        json_entry("last_dwell_ms", snap.missing_block_fetch_last_dwell_ms),
    ]);
    let view_change_causes = json_object(vec![
        json_entry(
            "commit_failure_total",
            snap.view_change_causes.commit_failure_total,
        ),
        json_entry(
            "quorum_timeout_total",
            snap.view_change_causes.quorum_timeout_total,
        ),
        json_entry("da_gate_total", snap.view_change_causes.da_gate_total),
        json_entry(
            "missing_payload_total",
            snap.view_change_causes.missing_payload_total,
        ),
        json_entry("missing_qc_total", snap.view_change_causes.missing_qc_total),
        json_entry(
            "validation_reject_total",
            snap.view_change_causes.validation_reject_total,
        ),
        json_entry("last_cause", snap.view_change_causes.last_cause.clone()),
        json_entry(
            "last_cause_timestamp_ms",
            snap.view_change_causes.last_cause_timestamp_ms,
        ),
    ]);
    let block_sync_roster = json_object(vec![
        json_entry(
            "commit_certificate_hint_total",
            snap.block_sync_roster.commit_certificate_hint_total,
        ),
        json_entry(
            "checkpoint_hint_total",
            snap.block_sync_roster.checkpoint_hint_total,
        ),
        json_entry(
            "commit_certificate_history_total",
            snap.block_sync_roster.commit_certificate_history_total,
        ),
        json_entry(
            "checkpoint_history_total",
            snap.block_sync_roster.checkpoint_history_total,
        ),
        json_entry(
            "roster_sidecar_total",
            snap.block_sync_roster.roster_sidecar_total,
        ),
        json_entry(
            "commit_roster_journal_total",
            snap.block_sync_roster.commit_roster_journal_total,
        ),
        json_entry(
            "drop_missing_total",
            snap.block_sync_roster.drop_missing_total,
        ),
    ]);
    let block_sync = json_object(vec![
        json_entry(
            "drop_invalid_signatures_total",
            snap.block_sync_drop_invalid_signatures_total,
        ),
        json_entry("qc_replaced_total", snap.block_sync_qc_replaced_total),
        json_entry(
            "qc_derive_failed_total",
            snap.block_sync_qc_derive_failed_total,
        ),
        json_entry("roster", block_sync_roster),
    ]);
    let epoch = json_object(vec![
        json_entry("length_blocks", snap.epoch_length_blocks),
        json_entry("commit_deadline_offset", snap.epoch_commit_deadline_offset),
        json_entry("reveal_deadline_offset", snap.epoch_reveal_deadline_offset),
    ]);
    let prf = json_object(vec![
        json_entry("height", snap.prf_height),
        json_entry("view", snap.prf_view),
        json_entry("epoch_seed", snap.prf_epoch_seed.map(hex::encode)),
    ]);
    let membership = json_object(vec![
        json_entry("height", snap.membership_height),
        json_entry("view", snap.membership_view),
        json_entry("epoch", snap.membership_epoch),
        json_entry(
            "view_hash",
            snap.membership_view_hash
                .map(|bytes| Value::from(hex::encode(bytes)))
                .unwrap_or(Value::Null),
        ),
    ]);
    let membership_mismatch = json_object(vec![
        json_entry(
            "active_peers",
            Value::Array(
                snap.membership_mismatch
                    .active_peers
                    .iter()
                    .map(|peer| Value::from(peer.to_string()))
                    .collect(),
            ),
        ),
        json_entry(
            "last_peer",
            snap.membership_mismatch
                .last_peer
                .as_ref()
                .map(|peer| Value::from(peer.to_string()))
                .unwrap_or(Value::Null),
        ),
        json_entry("last_height", snap.membership_mismatch.last_height),
        json_entry("last_view", snap.membership_mismatch.last_view),
        json_entry("last_epoch", snap.membership_mismatch.last_epoch),
        json_entry(
            "last_local_hash",
            snap.membership_mismatch
                .last_local_hash
                .map(|bytes| Value::from(hex::encode(bytes)))
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "last_remote_hash",
            snap.membership_mismatch
                .last_remote_hash
                .map(|bytes| Value::from(hex::encode(bytes)))
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "last_timestamp_ms",
            snap.membership_mismatch.last_timestamp_ms,
        ),
    ]);
    let lane_commitments = Value::Array(
        snap.lane_commitments
            .iter()
            .map(|entry| {
                json_object(vec![
                    json_entry("block_height", entry.block_height),
                    json_entry("lane_id", entry.lane_id),
                    json_entry("tx_count", entry.tx_count),
                    json_entry("total_chunks", entry.total_chunks),
                    json_entry("rbc_bytes_total", entry.rbc_bytes_total),
                    json_entry("teu_total", entry.teu_total),
                    json_entry("block_hash", format!("{}", entry.block_hash)),
                ])
            })
            .collect(),
    );
    let dataspace_commitments = Value::Array(
        snap.dataspace_commitments
            .iter()
            .map(|entry| {
                json_object(vec![
                    json_entry("block_height", entry.block_height),
                    json_entry("lane_id", entry.lane_id),
                    json_entry("dataspace_id", entry.dataspace_id),
                    json_entry("tx_count", entry.tx_count),
                    json_entry("total_chunks", entry.total_chunks),
                    json_entry("rbc_bytes_total", entry.rbc_bytes_total),
                    json_entry("teu_total", entry.teu_total),
                    json_entry("block_hash", format!("{}", entry.block_hash)),
                ])
            })
            .collect(),
    );
    let lane_governance = Value::Array(
        snap.lane_governance
            .iter()
            .map(|entry| {
                let runtime_upgrade = entry.runtime_upgrade.as_ref().map(|hook| {
                    json_object(vec![
                        json_entry("allow", hook.allow),
                        json_entry("require_metadata", hook.require_metadata),
                        json_entry(
                            "metadata_key",
                            hook.metadata_key
                                .clone()
                                .map(Value::from)
                                .unwrap_or(Value::Null),
                        ),
                        json_entry(
                            "allowed_ids",
                            Value::Array(
                                hook.allowed_ids.iter().cloned().map(Value::from).collect(),
                            ),
                        ),
                    ])
                });
                let privacy_commitments = Value::Array(
                    entry
                        .privacy_commitments
                        .iter()
                        .map(|commitment| {
                            let (scheme, merkle, snark) = match &commitment.scheme {
                                sumeragi::status::LanePrivacyCommitmentSchemeSnapshot::Merkle {
                                    root,
                                    max_depth,
                                } => (
                                    Value::from("merkle"),
                                    json_object(vec![
                                        json_entry("root", format!("0x{}", hex::encode(root))),
                                        json_entry("max_depth", u64::from(*max_depth)),
                                    ]),
                                    Value::Null,
                                ),
                                sumeragi::status::LanePrivacyCommitmentSchemeSnapshot::Snark {
                                    circuit_id,
                                    verifying_key_digest,
                                    statement_hash,
                                    proof_hash,
                                } => (
                                    Value::from("snark"),
                                    Value::Null,
                                    json_object(vec![
                                        json_entry("circuit_id", u64::from(*circuit_id)),
                                        json_entry(
                                            "verifying_key_digest",
                                            format!("0x{}", hex::encode(verifying_key_digest)),
                                        ),
                                        json_entry(
                                            "statement_hash",
                                            format!("0x{}", hex::encode(statement_hash)),
                                        ),
                                        json_entry(
                                            "proof_hash",
                                            format!("0x{}", hex::encode(proof_hash)),
                                        ),
                                    ]),
                                ),
                            };
                            json_object(vec![
                                json_entry("id", u64::from(commitment.id)),
                                json_entry("scheme", scheme),
                                json_entry("merkle", merkle),
                                json_entry("snark", snark),
                            ])
                        })
                        .collect(),
                );
                json_object(vec![
                    json_entry("lane_id", entry.lane_id),
                    json_entry("alias", entry.alias.clone()),
                    json_entry(
                        "governance",
                        entry
                            .governance
                            .clone()
                            .map(Value::from)
                            .unwrap_or(Value::Null),
                    ),
                    json_entry("manifest_required", entry.manifest_required),
                    json_entry("manifest_ready", entry.manifest_ready),
                    json_entry(
                        "manifest_path",
                        entry
                            .manifest_path
                            .clone()
                            .map(Value::from)
                            .unwrap_or(Value::Null),
                    ),
                    json_entry(
                        "validator_ids",
                        Value::Array(
                            entry
                                .validator_ids
                                .iter()
                                .cloned()
                                .map(Value::from)
                                .collect(),
                        ),
                    ),
                    json_entry(
                        "quorum",
                        entry
                            .quorum
                            .map(|value| Value::from(u64::from(value)))
                            .unwrap_or(Value::Null),
                    ),
                    json_entry(
                        "protected_namespaces",
                        Value::Array(
                            entry
                                .protected_namespaces
                                .iter()
                                .cloned()
                                .map(Value::from)
                                .collect(),
                        ),
                    ),
                    json_entry("runtime_upgrade", runtime_upgrade.unwrap_or(Value::Null)),
                    json_entry("privacy_commitments", privacy_commitments),
                ])
            })
            .collect(),
    );
    let access_set_sources = json_object(vec![
        json_entry(
            "manifest_hints",
            snap.access_set_sources.manifest_hints,
        ),
        json_entry(
            "entrypoint_hints",
            snap.access_set_sources.entrypoint_hints,
        ),
        json_entry("prepass_merge", snap.access_set_sources.prepass_merge),
        json_entry(
            "conservative_fallback",
            snap.access_set_sources.conservative_fallback,
        ),
    ]);
    let recent_evictions = Value::Array(
        snap.rbc_store_recent_evictions
            .iter()
            .map(|ev| {
                json_object(vec![
                    json_entry("block_hash", Value::from(hex::encode(ev.block_hash))),
                    json_entry("height", ev.height),
                    json_entry("view", ev.view),
                ])
            })
            .collect(),
    );
    let rbc_store = json_object(vec![
        json_entry("sessions", snap.rbc_store_sessions),
        json_entry("bytes", snap.rbc_store_bytes),
        json_entry("pressure_level", snap.rbc_store_pressure_level),
        json_entry(
            "backpressure_deferrals_total",
            snap.rbc_store_backpressure_deferrals_total,
        ),
        json_entry("evictions_total", snap.rbc_store_evictions_total),
        json_entry("recent_evictions", recent_evictions),
    ]);
    let npos_election = snap
        .npos_election
        .as_ref()
        .map(|election| {
            let validator_set = Value::Array(
                election
                    .validator_set
                    .iter()
                    .map(|peer| Value::from(peer.public_key().to_string()))
                    .collect(),
            );
            let params = json_object(vec![
                json_entry("max_validators", election.params.max_validators),
                json_entry("min_self_bond", election.params.min_self_bond),
                json_entry("min_nomination_bond", election.params.min_nomination_bond),
                json_entry(
                    "max_nominator_concentration_pct",
                    election.params.max_nominator_concentration_pct,
                ),
                json_entry("seat_band_pct", election.params.seat_band_pct),
                json_entry(
                    "max_entity_correlation_pct",
                    election.params.max_entity_correlation_pct,
                ),
                json_entry(
                    "finality_margin_blocks",
                    election.params.finality_margin_blocks,
                ),
            ]);
            let tie_break = Value::Array(
                election
                    .tie_break
                    .iter()
                    .map(|entry| {
                        json_object(vec![
                            json_entry("peer_id", entry.peer_id.public_key().to_string()),
                            json_entry("score", hex::encode(entry.score)),
                        ])
                    })
                    .collect(),
            );
            json_object(vec![
                json_entry("epoch", election.epoch),
                json_entry("snapshot_height", election.snapshot_height),
                json_entry("seed", hex::encode(election.seed)),
                json_entry("candidates_total", election.candidates_total),
                json_entry(
                    "validator_set_hash",
                    format!("{}", election.validator_set_hash),
                ),
                json_entry("validator_set", validator_set),
                json_entry("params", params),
                json_entry(
                    "rejection_reason",
                    election
                        .rejection_reason
                        .as_ref()
                        .map(|s| Value::from(s.clone()))
                        .unwrap_or(Value::Null),
                ),
                json_entry("tie_break", tie_break),
            ])
        })
        .unwrap_or(Value::Null);
    let consensus_caps = snap
        .consensus_caps
        .as_ref()
        .map(|caps| {
            json_object(vec![
                json_entry("collectors_k", caps.collectors_k),
                json_entry("redundant_send_r", caps.redundant_send_r),
                json_entry("da_enabled", caps.da_enabled),
                json_entry("require_execution_qc", caps.require_execution_qc),
                json_entry("require_wsv_exec_qc", caps.require_wsv_exec_qc),
                json_entry("rbc_chunk_max_bytes", caps.rbc_chunk_max_bytes),
                json_entry("rbc_session_ttl_ms", caps.rbc_session_ttl_ms),
                json_entry("rbc_store_max_sessions", caps.rbc_store_max_sessions),
                json_entry("rbc_store_soft_sessions", caps.rbc_store_soft_sessions),
                json_entry("rbc_store_max_bytes", caps.rbc_store_max_bytes),
                json_entry("rbc_store_soft_bytes", caps.rbc_store_soft_bytes),
            ])
        })
        .unwrap_or(Value::Null);
    crate::json_object(vec![
        json_entry("mode_tag", &snap.mode_tag),
        json_entry(
            "staged_mode_tag",
            snap.staged_mode_tag
                .clone()
                .map(Value::from)
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "staged_mode_activation_height",
            snap.staged_mode_activation_height
                .map(Value::from)
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "mode_activation_lag_blocks",
            snap.mode_activation_lag_blocks
                .map(Value::from)
                .unwrap_or(Value::Null),
        ),
        json_entry("mode_flip_kill_switch", snap.mode_flip_kill_switch),
        json_entry("mode_flip_blocked", snap.mode_flip_blocked),
        json_entry("mode_flip_success_total", snap.mode_flip_success_total),
        json_entry("mode_flip_fail_total", snap.mode_flip_fail_total),
        json_entry("mode_flip_blocked_total", snap.mode_flip_blocked_total),
        json_entry(
            "last_mode_flip_timestamp_ms",
            snap.last_mode_flip_timestamp_ms
                .map(Value::from)
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "last_mode_flip_error",
            snap.last_mode_flip_error
                .clone()
                .map(Value::from)
                .unwrap_or(Value::Null),
        ),
        json_entry("consensus_caps", consensus_caps),
        json_entry("leader_index", snap.leader_index),
        json_entry("view_change_index", snap.view_change_index),
        json_entry("view_change_causes", view_change_causes),
        json_entry("highest_qc", highest_qc),
        json_entry("locked_qc", locked_qc),
        json_entry("tx_queue", tx_queue),
        json_entry("worker_loop", worker_loop),
        json_entry("commit_inflight", commit_inflight),
        json_entry("missing_block_fetch", missing_block_fetch),
        json_entry("block_sync", block_sync),
        json_entry("kura_store", kura_store),
        json_entry("epoch", epoch),
        json_entry("gossip_fallback_total", snap.gossip_fallback_total),
        json_entry("dedup_evictions", dedup_evictions),
        json_entry("bg_post_drop_post_total", snap.bg_post_drop_post_total),
        json_entry(
            "bg_post_drop_broadcast_total",
            snap.bg_post_drop_broadcast_total,
        ),
        json_entry(
            "block_created_dropped_by_lock_total",
            snap.block_created_dropped_by_lock_total,
        ),
        json_entry(
            "block_created_hint_mismatch_total",
            snap.block_created_hint_mismatch_total,
        ),
        json_entry(
            "block_created_proposal_mismatch_total",
            snap.block_created_proposal_mismatch_total,
        ),
        json_entry("view_change_causes", view_change_causes),
        json_entry("validation_reject_total", snap.validation_reject_total),
        json_entry(
            "validation_reject_reason",
            snap.validation_reject_reason
                .map(Value::from)
                .unwrap_or(Value::Null),
        ),
        json_entry("validation_rejects", validation_rejects),
        json_entry("da_gate", da_gate),
        json_entry("settlement", settlement),
        json_entry(
            "pacemaker_backpressure_deferrals_total",
            snap.pacemaker_backpressure_deferrals_total,
        ),
        json_entry(
            "commit_pipeline_tick_total",
            snap.commit_pipeline_tick_total,
        ),
        json_entry("rbc_store", rbc_store),
        json_entry("qc_rebuild_attempts_total", snap.qc_rebuild_attempts_total),
        json_entry(
            "qc_rebuild_successes_total",
            snap.qc_rebuild_successes_total,
        ),
        json_entry(
            "qc_quorum_without_qc_total",
            snap.qc_quorum_without_qc_total,
        ),
        json_entry(
            "collectors_targeted_current",
            snap.collectors_targeted_current,
        ),
        json_entry(
            "collectors_targeted_last_per_block",
            snap.collectors_targeted_last_per_block,
        ),
        json_entry("redundant_sends_total", snap.redundant_sends_total),
        json_entry("prf", prf),
        json_entry("membership", membership),
        json_entry("membership_mismatch", membership_mismatch),
        json_entry(
            "lane_governance_sealed_total",
            snap.lane_governance_sealed_total,
        ),
        json_entry(
            "lane_governance_sealed_aliases",
            Value::Array(
                snap.lane_governance_sealed_aliases
                    .iter()
                    .cloned()
                    .map(Value::from)
                    .collect(),
            ),
        ),
        json_entry("lane_governance", lane_governance),
        json_entry("pipeline_conflict_rate_bps", snap.pipeline_conflict_rate_bps),
        json_entry("access_set_sources", access_set_sources),
        json_entry("nexus_fee", nexus_fee_snapshot_value(&snap.nexus_fee)),
        json_entry(
            "nexus_staking",
            nexus_staking_snapshot_value(&snap.nexus_staking),
        ),
        json_entry("vrf_penalty_epoch", snap.vrf_penalty_epoch),
        json_entry("vrf_committed_no_reveal_total", snap.vrf_non_reveal_total),
        json_entry(
            "vrf_no_participation_total",
            snap.vrf_no_participation_total,
        ),
        json_entry("vrf_late_reveals_total", snap.vrf_late_reveals_total),
        json_entry(
            "consensus_penalties_applied_total",
            snap.consensus_penalties_applied_total,
        ),
        json_entry(
            "consensus_penalties_pending",
            snap.consensus_penalties_pending,
        ),
        json_entry(
            "vrf_penalties_applied_total",
            snap.vrf_penalties_applied_total,
        ),
        json_entry("vrf_penalties_pending", snap.vrf_penalties_pending),
        json_entry("npos_election", npos_election),
    ])
}

#[cfg(test)]
mod status_tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, PublicKey};
    use iroha_data_model::{
        consensus::{ValidatorElectionOutcome, ValidatorElectionParameters, ValidatorTieBreak},
        nexus::LaneId,
    };
    use iroha_primitives::numeric::Numeric;

    #[test]
    fn status_snapshot_json_includes_vrf_fields() {
        let snap = sumeragi::StatusSnapshot {
            vrf_penalty_epoch: 7,
            vrf_non_reveal_total: 2,
            vrf_no_participation_total: 1,
            vrf_late_reveals_total: 4,
            consensus_penalties_applied_total: 5,
            consensus_penalties_pending: 2,
            vrf_penalties_applied_total: 3,
            vrf_penalties_pending: 1,
            epoch_length_blocks: 3600,
            epoch_commit_deadline_offset: 120,
            epoch_reveal_deadline_offset: 160,
            ..Default::default()
        };
        let payload = status_snapshot_json(&snap);
        assert_eq!(
            payload
                .get("vrf_penalty_epoch")
                .and_then(Value::as_u64)
                .unwrap(),
            7
        );
        assert_eq!(
            payload
                .get("vrf_committed_no_reveal_total")
                .and_then(Value::as_u64)
                .unwrap(),
            2
        );
        assert_eq!(
            payload
                .get("vrf_no_participation_total")
                .and_then(Value::as_u64)
                .unwrap(),
            1
        );
        assert_eq!(
            payload
                .get("vrf_late_reveals_total")
                .and_then(Value::as_u64)
                .unwrap(),
            4
        );
        assert_eq!(
            payload
                .get("consensus_penalties_applied_total")
                .and_then(Value::as_u64)
                .unwrap(),
            5
        );
        assert_eq!(
            payload
                .get("consensus_penalties_pending")
                .and_then(Value::as_u64)
                .unwrap(),
            2
        );
        assert_eq!(
            payload
                .get("vrf_penalties_applied_total")
                .and_then(Value::as_u64)
                .unwrap(),
            3
        );
        assert_eq!(
            payload
                .get("vrf_penalties_pending")
                .and_then(Value::as_u64)
                .unwrap(),
            1
        );
        let epoch = payload
            .get("epoch")
            .and_then(Value::as_object)
            .expect("epoch object");
        assert_eq!(
            epoch.get("length_blocks").and_then(Value::as_u64).unwrap(),
            3600
        );
        assert_eq!(
            epoch
                .get("commit_deadline_offset")
                .and_then(Value::as_u64)
                .unwrap(),
            120
        );
        assert_eq!(
            epoch
                .get("reveal_deadline_offset")
                .and_then(Value::as_u64)
                .unwrap(),
            160
        );
        assert_eq!(
            payload
                .get("lane_governance_sealed_total")
                .and_then(Value::as_u64)
                .unwrap(),
            0
        );
        assert!(
            payload
                .get("lane_governance_sealed_aliases")
                .and_then(Value::as_array)
                .is_some()
        );
        let membership = payload
            .get("membership")
            .and_then(Value::as_object)
            .expect("membership object present");
        assert_eq!(membership.get("height").and_then(Value::as_u64).unwrap(), 0);
        assert_eq!(membership.get("view").and_then(Value::as_u64).unwrap(), 0);
        assert_eq!(membership.get("epoch").and_then(Value::as_u64).unwrap(), 0);
        assert!(
            membership
                .get("view_hash")
                .map(|value| value.is_null())
                .unwrap_or(false)
        );
        let membership_mismatch = payload
            .get("membership_mismatch")
            .and_then(Value::as_object)
            .expect("membership_mismatch object present");
        assert!(
            membership_mismatch
                .get("active_peers")
                .and_then(Value::as_array)
                .map(|peers| peers.is_empty())
                .unwrap_or(false)
        );
        assert!(
            membership_mismatch
                .get("last_peer")
                .map(|value| value.is_null())
                .unwrap_or(false)
        );
        assert_eq!(
            membership_mismatch
                .get("last_height")
                .and_then(Value::as_u64)
                .unwrap(),
            0
        );
        assert_eq!(
            membership_mismatch
                .get("last_view")
                .and_then(Value::as_u64)
                .unwrap(),
            0
        );
        assert_eq!(
            membership_mismatch
                .get("last_epoch")
                .and_then(Value::as_u64)
                .unwrap(),
            0
        );
        assert!(
            membership_mismatch
                .get("last_local_hash")
                .map(|value| value.is_null())
                .unwrap_or(false)
        );
        assert!(
            membership_mismatch
                .get("last_remote_hash")
                .map(|value| value.is_null())
                .unwrap_or(false)
        );
        assert_eq!(
            membership_mismatch
                .get("last_timestamp_ms")
                .and_then(Value::as_u64)
                .unwrap(),
            0
        );
        assert!(
            payload
                .get("lane_governance")
                .and_then(Value::as_array)
                .map(|entries| entries.is_empty())
                .unwrap_or(false),
            "lane governance array missing"
        );
    }

    #[test]
    fn status_snapshot_json_includes_npos_election() {
        let peer_pk = PublicKey::from_hex(
            Algorithm::Ed25519,
            "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4",
        )
        .expect("peer pk parses");
        let peer = PeerId::from(peer_pk);
        let params = ValidatorElectionParameters {
            max_validators: 8,
            min_self_bond: 1,
            min_nomination_bond: 1,
            max_nominator_concentration_pct: 25,
            seat_band_pct: 5,
            max_entity_correlation_pct: 10,
            finality_margin_blocks: 8,
        };
        let election = ValidatorElectionOutcome {
            epoch: 9,
            snapshot_height: 12,
            seed: [0xAB; 32],
            candidates_total: 1,
            validator_set_hash: HashOf::new(&vec![peer.clone()]),
            validator_set: vec![peer.clone()],
            params,
            rejection_reason: None,
            tie_break: vec![ValidatorTieBreak {
                peer_id: peer.clone(),
                score: [0u8; 32],
            }],
        };
        let snap = sumeragi::StatusSnapshot {
            npos_election: Some(election),
            ..Default::default()
        };

        let payload = status_snapshot_json(&snap);
        let election_json = payload
            .get("npos_election")
            .expect("npos election present")
            .as_object()
            .expect("npos election is object");
        assert_eq!(
            election_json
                .get("epoch")
                .and_then(Value::as_u64)
                .expect("epoch field"),
            9
        );
        assert_eq!(
            election_json
                .get("validator_set")
                .and_then(Value::as_array)
                .expect("validator_set field")
                .len(),
            1
        );
    }

    #[test]
    fn status_snapshot_json_includes_nexus_economics() {
        let fee = sumeragi::status::NexusFeeSnapshot {
            charged_total: 2,
            charged_via_payer_total: 1,
            charged_via_sponsor_total: 1,
            sponsor_disabled_total: 1,
            sponsor_cap_exceeded_total: 1,
            config_errors_total: 1,
            transfer_failures_total: 1,
            last_amount: Some(42),
            last_asset_id: Some("xor#sora".to_owned()),
            last_payer: Some(sumeragi::status::NexusFeePayer::Sponsor),
            last_payer_id: Some("sponsor@test".to_owned()),
            last_error: Some("denied".to_owned()),
        };
        let lane_id = LaneId::new(7);
        let staking = sumeragi::status::NexusStakingSnapshot {
            lanes: vec![sumeragi::status::NexusStakingLaneSnapshot {
                lane_id,
                bonded: Numeric::new(1_000, 0),
                pending_unbond: Numeric::new(25, 0),
                slash_total: 3,
            }],
        };
        let snap = sumeragi::StatusSnapshot {
            nexus_fee: fee,
            nexus_staking: staking,
            ..Default::default()
        };
        let payload = status_snapshot_json(&snap);
        let fee_json = payload
            .get("nexus_fee")
            .and_then(Value::as_object)
            .expect("nexus_fee");
        assert_eq!(
            fee_json
                .get("charged_via_sponsor_total")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            fee_json.get("last_payer").and_then(Value::as_str),
            Some("sponsor")
        );
        assert_eq!(
            fee_json.get("last_amount").and_then(Value::as_str),
            Some("42")
        );

        let staking_json = payload
            .get("nexus_staking")
            .and_then(Value::as_object)
            .expect("nexus_staking");
        let lanes = staking_json
            .get("lanes")
            .and_then(Value::as_array)
            .expect("lanes");
        assert_eq!(lanes.len(), 1);
        let lane = lanes[0].as_object().expect("lane object");
        assert_eq!(
            lane.get("lane_id").and_then(Value::as_u64),
            Some(u64::from(lane_id.as_u32()))
        );
        assert_eq!(lane.get("bonded").and_then(Value::as_str), Some("1000"));
        assert_eq!(
            lane.get("pending_unbond").and_then(Value::as_str),
            Some("25")
        );
        assert_eq!(lane.get("slash_total").and_then(Value::as_u64), Some(3));
    }

    #[test]
    fn status_snapshot_json_includes_recent_rbc_evictions() {
        let evicted = status::RbcEvictedSession {
            block_hash: [0xAB; 32],
            height: 12,
            view: 3,
        };
        let snap = sumeragi::StatusSnapshot {
            rbc_store_sessions: 1,
            rbc_store_bytes: 512,
            rbc_store_pressure_level: 2,
            rbc_store_evictions_total: 1,
            rbc_store_recent_evictions: vec![evicted.clone()],
            ..Default::default()
        };
        let payload = status_snapshot_json(&snap);
        let rbc = payload
            .get("rbc_store")
            .and_then(Value::as_object)
            .expect("rbc_store object");
        let recent = rbc
            .get("recent_evictions")
            .and_then(Value::as_array)
            .expect("recent evictions array");
        assert_eq!(recent.len(), 1);
        let entry = recent[0].as_object().expect("eviction entry object");
        assert_eq!(
            entry.get("block_hash").and_then(Value::as_str).unwrap(),
            hex::encode(evicted.block_hash)
        );
        assert_eq!(
            entry.get("height").and_then(Value::as_u64).unwrap(),
            evicted.height
        );
        assert_eq!(
            entry.get("view").and_then(Value::as_u64).unwrap(),
            evicted.view
        );
    }

    #[test]
    fn status_snapshot_json_includes_da_gate_and_kura_store() {
        let last_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xCD; 32]));
        let snap = sumeragi::StatusSnapshot {
            da_gate: status::DaGateSnapshot {
                reason: status::DaGateReasonSnapshot::MissingAvailabilityQc,
                last_satisfied: status::DaGateSatisfactionSnapshot::AvailabilityQc,
                missing_availability_total: 2,
                manifest_guard_total: 4,
            },
            missing_block_fetch_total: 5,
            missing_block_fetch_last_targets: 3,
            missing_block_fetch_last_dwell_ms: 11,
            kura_store: status::KuraStoreSnapshot {
                failures_total: 1,
                abort_total: 2,
                last_retry_attempt: 3,
                last_retry_backoff_ms: 7,
                last_height: 9,
                last_view: 4,
                last_hash: Some(last_hash),
                ..Default::default()
            },
            ..Default::default()
        };

        let payload = status_snapshot_json(&snap);
        let gate = payload
            .get("da_gate")
            .and_then(Value::as_object)
            .expect("da_gate object");
        assert_eq!(
            gate.get("reason").and_then(Value::as_str),
            Some("missing_availability_qc")
        );
        assert_eq!(
            gate.get("last_satisfied").and_then(Value::as_str),
            Some("availability_qc")
        );
        assert_eq!(
            gate.get("missing_availability_total")
                .and_then(Value::as_u64),
            Some(2)
        );
        assert_eq!(
            gate.get("manifest_guard_total").and_then(Value::as_u64),
            Some(4)
        );
        assert!(!gate.contains_key("missing_rbc_total"));
        assert!(!gate.contains_key("last_missing_rbc_height"));

        let fetch = payload
            .get("missing_block_fetch")
            .and_then(Value::as_object)
            .expect("missing_block_fetch object");
        assert_eq!(fetch.get("total").and_then(Value::as_u64), Some(5));
        assert_eq!(fetch.get("last_targets").and_then(Value::as_u64), Some(3));
        assert_eq!(fetch.get("last_dwell_ms").and_then(Value::as_u64), Some(11));

        let kura = payload
            .get("kura_store")
            .and_then(Value::as_object)
            .expect("kura_store object");
        assert_eq!(kura.get("failures_total").and_then(Value::as_u64), Some(1));
        assert_eq!(kura.get("abort_total").and_then(Value::as_u64), Some(2));
        assert_eq!(kura.get("stage_total").and_then(Value::as_u64), Some(0));
        assert_eq!(
            kura.get("rollback_total").and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            kura.get("lock_reset_total").and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            kura.get("last_retry_attempt").and_then(Value::as_u64),
            Some(3)
        );
        assert_eq!(
            kura.get("last_retry_backoff_ms").and_then(Value::as_u64),
            Some(7)
        );
        assert_eq!(kura.get("last_height").and_then(Value::as_u64), Some(9));
        assert_eq!(kura.get("last_view").and_then(Value::as_u64), Some(4));
        assert_eq!(
            kura.get("last_hash")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            Some(format!("{last_hash}"))
        );
        assert_eq!(
            payload
                .get("validation_reject_total")
                .and_then(Value::as_u64),
            Some(0)
        );
        assert!(
            payload
                .get("validation_reject_reason")
                .map(|reason| reason.is_null())
                .unwrap_or(false)
        );
        let view_change_causes = payload
            .get("view_change_causes")
            .and_then(Value::as_object)
            .expect("view_change_causes object");
        assert_eq!(
            view_change_causes
                .get("commit_failure_total")
                .and_then(Value::as_u64),
            Some(0)
        );
        assert!(
            view_change_causes
                .get("last_cause")
                .map(|cause| cause.is_null())
                .unwrap_or(false)
        );
        assert_eq!(
            view_change_causes
                .get("validation_reject_total")
                .and_then(Value::as_u64),
            Some(0)
        );

        let roster = payload
            .get("block_sync_roster")
            .and_then(Value::as_object)
            .expect("block_sync_roster object");
        assert_eq!(
            roster
                .get("commit_roster_journal_total")
                .and_then(Value::as_u64),
            Some(0)
        );
        assert_eq!(
            roster.get("drop_missing_total").and_then(Value::as_u64),
            Some(0)
        );
    }
}

/// GET /v1/sumeragi/status — latest consensus status snapshot
/// Returns leader index and HighestQC (height, view).
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_status(
    accept: Option<axum::http::HeaderValue>,
    nexus_enabled: bool,
) -> Result<Response> {
    let mut snap = sumeragi::status_snapshot();
    if !nexus_enabled {
        snap = snap.strip_lane_details();
    }
    let format = match crate::utils::negotiate_response_format(accept.as_ref()) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };

    if matches!(format, crate::utils::ResponseFormat::Norito) {
        let wire = SumeragiStatusWire {
            mode_tag: snap.mode_tag.clone(),
            staged_mode_tag: snap.staged_mode_tag.clone(),
            staged_mode_activation_height: snap.staged_mode_activation_height,
            mode_activation_lag_blocks: snap.mode_activation_lag_blocks,
            mode_flip_kill_switch: snap.mode_flip_kill_switch,
            mode_flip_blocked: snap.mode_flip_blocked,
            mode_flip_success_total: snap.mode_flip_success_total,
            mode_flip_fail_total: snap.mode_flip_fail_total,
            mode_flip_blocked_total: snap.mode_flip_blocked_total,
            last_mode_flip_timestamp_ms: snap.last_mode_flip_timestamp_ms,
            last_mode_flip_error: snap.last_mode_flip_error.clone(),
            consensus_caps: snap
                .consensus_caps
                .as_ref()
                .map(|caps| SumeragiConsensusCapsStatus {
                    collectors_k: caps.collectors_k,
                    redundant_send_r: caps.redundant_send_r,
                    da_enabled: caps.da_enabled,
                    require_execution_qc: caps.require_execution_qc,
                    require_wsv_exec_qc: caps.require_wsv_exec_qc,
                    rbc_chunk_max_bytes: caps.rbc_chunk_max_bytes,
                    rbc_session_ttl_ms: caps.rbc_session_ttl_ms,
                    rbc_store_max_sessions: caps.rbc_store_max_sessions,
                    rbc_store_soft_sessions: caps.rbc_store_soft_sessions,
                    rbc_store_max_bytes: caps.rbc_store_max_bytes,
                    rbc_store_soft_bytes: caps.rbc_store_soft_bytes,
                }),
            leader_index: snap.leader_index,
            highest_qc_height: snap.highest_qc_height,
            highest_qc_view: snap.highest_qc_view,
            highest_qc_subject: snap.highest_qc_subject,
            locked_qc_height: snap.locked_qc_height,
            locked_qc_view: snap.locked_qc_view,
            locked_qc_subject: snap.locked_qc_subject,
            commit_certificate: SumeragiCommitCertificateStatus {
                height: snap.commit_certificate.height,
                view: snap.commit_certificate.view,
                epoch: snap.commit_certificate.epoch,
                block_hash: snap.commit_certificate.block_hash,
                validator_set_hash: snap.commit_certificate.validator_set_hash,
                validator_set_len: snap.commit_certificate.validator_set_len,
                signatures_total: snap.commit_certificate.signatures_total,
            },
            commit_quorum: SumeragiCommitQuorumStatus {
                height: snap.commit_quorum.height,
                view: snap.commit_quorum.view,
                block_hash: snap.commit_quorum.block_hash,
                signatures_present: snap.commit_quorum.signatures_present,
                signatures_counted: snap.commit_quorum.signatures_counted,
                signatures_set_b: snap.commit_quorum.signatures_set_b,
                signatures_required: snap.commit_quorum.signatures_required,
                last_updated_ms: snap.commit_quorum.last_updated_ms,
            },
            view_change_proof_accepted_total: snap.view_change_proof_accepted_total,
            view_change_proof_stale_total: snap.view_change_proof_stale_total,
            view_change_proof_rejected_total: snap.view_change_proof_rejected_total,
            view_change_suggest_total: snap.view_change_suggest_total,
            view_change_install_total: snap.view_change_install_total,
            view_change_causes: SumeragiViewChangeCauseStatus {
                commit_failure_total: snap.view_change_causes.commit_failure_total,
                quorum_timeout_total: snap.view_change_causes.quorum_timeout_total,
                da_gate_total: snap.view_change_causes.da_gate_total,
                missing_payload_total: snap.view_change_causes.missing_payload_total,
                missing_qc_total: snap.view_change_causes.missing_qc_total,
                validation_reject_total: snap.view_change_causes.validation_reject_total,
                last_cause: snap.view_change_causes.last_cause.clone(),
                last_cause_timestamp_ms: snap.view_change_causes.last_cause_timestamp_ms,
                last_commit_failure_timestamp_ms: snap
                    .view_change_causes
                    .last_commit_failure_timestamp_ms,
                last_quorum_timeout_timestamp_ms: snap
                    .view_change_causes
                    .last_quorum_timeout_timestamp_ms,
                last_da_gate_timestamp_ms: snap.view_change_causes.last_da_gate_timestamp_ms,
                last_missing_payload_timestamp_ms: snap
                    .view_change_causes
                    .last_missing_payload_timestamp_ms,
                last_missing_qc_timestamp_ms: snap.view_change_causes.last_missing_qc_timestamp_ms,
                last_validation_reject_timestamp_ms: snap
                    .view_change_causes
                    .last_validation_reject_timestamp_ms,
            },
            gossip_fallback_total: snap.gossip_fallback_total,
            block_created_dropped_by_lock_total: snap.block_created_dropped_by_lock_total,
            block_created_hint_mismatch_total: snap.block_created_hint_mismatch_total,
            block_created_proposal_mismatch_total: snap.block_created_proposal_mismatch_total,
            validation_reject_total: snap.validation_reject_total,
            validation_reject_reason: snap.validation_reject_reason.map(ToOwned::to_owned),
            validation_rejects: SumeragiValidationRejectStatus {
                total: snap.validation_rejects.total,
                stateless_total: snap.validation_rejects.stateless_total,
                execution_total: snap.validation_rejects.execution_total,
                prev_hash_total: snap.validation_rejects.prev_hash_total,
                prev_height_total: snap.validation_rejects.prev_height_total,
                topology_total: snap.validation_rejects.topology_total,
                last_reason: snap.validation_rejects.last_reason.map(ToOwned::to_owned),
                last_height: snap.validation_rejects.last_height,
                last_view: snap.validation_rejects.last_view,
                last_block: snap.validation_rejects.last_block,
                last_timestamp_ms: snap.validation_rejects.last_timestamp_ms,
            },
            peer_key_policy: SumeragiPeerKeyPolicyStatus {
                total: snap.peer_key_policy.total,
                missing_hsm_total: snap.peer_key_policy.missing_hsm_total,
                disallowed_algorithm_total: snap.peer_key_policy.disallowed_algorithm_total,
                disallowed_provider_total: snap.peer_key_policy.disallowed_provider_total,
                lead_time_violation_total: snap.peer_key_policy.lead_time_violation_total,
                activation_in_past_total: snap.peer_key_policy.activation_in_past_total,
                expiry_before_activation_total: snap.peer_key_policy.expiry_before_activation_total,
                identifier_collision_total: snap.peer_key_policy.identifier_collision_total,
                last_reason: snap.peer_key_policy.last_reason.map(ToOwned::to_owned),
                last_timestamp_ms: snap.peer_key_policy.last_timestamp_ms,
            },
            block_sync_roster: SumeragiBlockSyncRosterStatus {
                commit_certificate_hint_total: snap.block_sync_roster.commit_certificate_hint_total,
                checkpoint_hint_total: snap.block_sync_roster.checkpoint_hint_total,
                commit_certificate_history_total: snap
                    .block_sync_roster
                    .commit_certificate_history_total,
                checkpoint_history_total: snap.block_sync_roster.checkpoint_history_total,
                roster_sidecar_total: snap.block_sync_roster.roster_sidecar_total,
                commit_roster_journal_total: snap.block_sync_roster.commit_roster_journal_total,
                drop_missing_total: snap.block_sync_roster.drop_missing_total,
            },
            pacemaker_backpressure_deferrals_total: snap.pacemaker_backpressure_deferrals_total,
            commit_pipeline_tick_total: snap.commit_pipeline_tick_total,
            da_reschedule_total: snap.da_reschedule_total,
            missing_block_fetch: SumeragiMissingBlockFetchStatus {
                total: snap.missing_block_fetch_total,
                last_targets: snap.missing_block_fetch_last_targets,
                last_dwell_ms: snap.missing_block_fetch_last_dwell_ms,
            },
            da_gate: SumeragiDaGateStatus {
                reason: match snap.da_gate.reason {
                    sumeragi::status::DaGateReasonSnapshot::MissingAvailabilityQc => {
                        SumeragiDaGateReason::MissingAvailabilityQc
                    }
                    sumeragi::status::DaGateReasonSnapshot::ManifestMissing => {
                        SumeragiDaGateReason::ManifestMissing
                    }
                    sumeragi::status::DaGateReasonSnapshot::ManifestHashMismatch => {
                        SumeragiDaGateReason::ManifestHashMismatch
                    }
                    sumeragi::status::DaGateReasonSnapshot::ManifestReadFailed => {
                        SumeragiDaGateReason::ManifestReadFailed
                    }
                    sumeragi::status::DaGateReasonSnapshot::ManifestSpoolScan => {
                        SumeragiDaGateReason::ManifestSpoolScan
                    }
                    sumeragi::status::DaGateReasonSnapshot::None => SumeragiDaGateReason::None,
                },
                last_satisfied: match snap.da_gate.last_satisfied {
                    sumeragi::status::DaGateSatisfactionSnapshot::None => {
                        SumeragiDaGateSatisfaction::None
                    }
                    sumeragi::status::DaGateSatisfactionSnapshot::AvailabilityQc => {
                        SumeragiDaGateSatisfaction::AvailabilityQc
                    }
                },
                missing_availability_total: snap.da_gate.missing_availability_total,
                manifest_guard_total: snap.da_gate.manifest_guard_total,
            },
            kura_store: SumeragiKuraStoreStatus {
                failures_total: snap.kura_store.failures_total,
                abort_total: snap.kura_store.abort_total,
                stage_total: snap.kura_store.stage_total,
                rollback_total: snap.kura_store.rollback_total,
                stage_last_height: snap.kura_store.stage_last_height,
                stage_last_view: snap.kura_store.stage_last_view,
                stage_last_hash: snap.kura_store.stage_last_hash,
                rollback_last_height: snap.kura_store.rollback_last_height,
                rollback_last_view: snap.kura_store.rollback_last_view,
                rollback_last_hash: snap.kura_store.rollback_last_hash,
                rollback_last_reason: snap
                    .kura_store
                    .rollback_last_reason
                    .map(str::to_string),
                lock_reset_total: snap.kura_store.lock_reset_total,
                lock_reset_last_height: snap.kura_store.lock_reset_last_height,
                lock_reset_last_view: snap.kura_store.lock_reset_last_view,
                lock_reset_last_hash: snap.kura_store.lock_reset_last_hash,
                lock_reset_last_reason: snap
                    .kura_store
                    .lock_reset_last_reason
                    .map(str::to_string),
                last_retry_attempt: snap.kura_store.last_retry_attempt,
                last_retry_backoff_ms: snap.kura_store.last_retry_backoff_ms,
                last_height: snap.kura_store.last_height,
                last_view: snap.kura_store.last_view,
                last_hash: snap.kura_store.last_hash,
            },
            rbc_store: SumeragiRbcStoreStatus {
                sessions: snap.rbc_store_sessions,
                bytes: snap.rbc_store_bytes,
                pressure_level: snap.rbc_store_pressure_level,
                backpressure_deferrals_total: snap.rbc_store_backpressure_deferrals_total,
                evictions_total: snap.rbc_store_evictions_total,
                recent_evictions: snap
                    .rbc_store_recent_evictions
                    .iter()
                    .map(|entry| SumeragiRbcEvictedSession {
                        block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                            entry.block_hash,
                        )),
                        height: entry.height,
                        view: entry.view,
                    })
                    .collect(),
            },
            pending_rbc: SumeragiPendingRbcStatus {
                sessions: snap.pending_rbc.sessions,
                session_cap: snap.pending_rbc.session_cap,
                chunks: snap.pending_rbc.chunks,
                bytes: snap.pending_rbc.bytes,
                max_chunks_per_session: snap.pending_rbc.max_chunks_per_session,
                max_bytes_per_session: snap.pending_rbc.max_bytes_per_session,
                ttl_ms: snap.pending_rbc.ttl_ms,
                drops_total: snap.pending_rbc.drops_total,
                drops_cap_total: snap.pending_rbc.drops_cap_total,
                drops_cap_bytes_total: snap.pending_rbc.drops_cap_bytes_total,
                drops_ttl_total: snap.pending_rbc.drops_ttl_total,
                drops_ttl_bytes_total: snap.pending_rbc.drops_ttl_bytes_total,
                drops_bytes_total: snap.pending_rbc.drops_bytes_total,
                evicted_total: snap.pending_rbc.evicted_total,
                entries: snap
                    .pending_rbc
                    .entries
                    .iter()
                    .map(|entry| SumeragiPendingRbcEntry {
                        block_hash: entry.block_hash,
                        height: entry.height,
                        view: entry.view,
                        chunks: entry.chunks,
                        bytes: entry.bytes,
                        ready: entry.ready,
                        deliver: entry.deliver,
                        dropped_chunks: entry.dropped_chunks,
                        dropped_bytes: entry.dropped_bytes,
                        dropped_ready: entry.dropped_ready,
                        dropped_deliver: entry.dropped_deliver,
                        age_ms: entry.age_ms,
                    })
                    .collect(),
            },
            tx_queue_depth: snap.tx_queue_depth,
            tx_queue_capacity: snap.tx_queue_capacity,
            tx_queue_saturated: snap.tx_queue_saturated,
            epoch_length_blocks: snap.epoch_length_blocks,
            epoch_commit_deadline_offset: snap.epoch_commit_deadline_offset,
            epoch_reveal_deadline_offset: snap.epoch_reveal_deadline_offset,
            prf_epoch_seed: snap.prf_epoch_seed,
            prf_height: snap.prf_height,
            prf_view: snap.prf_view,
            vrf_penalty_epoch: snap.vrf_penalty_epoch,
            vrf_committed_no_reveal_total: snap.vrf_non_reveal_total,
            vrf_no_participation_total: snap.vrf_no_participation_total,
            vrf_late_reveals_total: snap.vrf_late_reveals_total,
            consensus_penalties_applied_total: snap.consensus_penalties_applied_total,
            consensus_penalties_pending: snap.consensus_penalties_pending,
            vrf_penalties_applied_total: snap.vrf_penalties_applied_total,
            vrf_penalties_pending: snap.vrf_penalties_pending,
            membership: SumeragiMembershipStatus {
                height: snap.membership_height,
                view: snap.membership_view,
                epoch: snap.membership_epoch,
                view_hash: snap.membership_view_hash,
            },
            membership_mismatch: SumeragiMembershipMismatchStatus {
                active_peers: snap.membership_mismatch.active_peers.clone(),
                last_peer: snap.membership_mismatch.last_peer.clone(),
                last_height: snap.membership_mismatch.last_height,
                last_view: snap.membership_mismatch.last_view,
                last_epoch: snap.membership_mismatch.last_epoch,
                last_local_hash: snap.membership_mismatch.last_local_hash,
                last_remote_hash: snap.membership_mismatch.last_remote_hash,
                last_timestamp_ms: snap.membership_mismatch.last_timestamp_ms,
            },
            lane_commitments: snap
                .lane_commitments
                .iter()
                .map(|entry| SumeragiLaneCommitment {
                    block_height: entry.block_height,
                    lane_id: LaneId::new(entry.lane_id),
                    tx_count: entry.tx_count,
                    total_chunks: entry.total_chunks,
                    rbc_bytes_total: entry.rbc_bytes_total,
                    teu_total: entry.teu_total,
                    block_hash: entry.block_hash,
                })
                .collect(),
            dataspace_commitments: snap
                .dataspace_commitments
                .iter()
                .map(|entry| SumeragiDataspaceCommitment {
                    block_height: entry.block_height,
                    lane_id: LaneId::new(entry.lane_id),
                    dataspace_id: DataSpaceId::new(entry.dataspace_id),
                    tx_count: entry.tx_count,
                    total_chunks: entry.total_chunks,
                    rbc_bytes_total: entry.rbc_bytes_total,
                    teu_total: entry.teu_total,
                    block_hash: entry.block_hash,
                })
                .collect(),
            lane_settlement_commitments: snap.lane_settlement_commitments.clone(),
            lane_relay_envelopes: snap.lane_relay_envelopes.clone(),
            lane_governance_sealed_total: snap.lane_governance_sealed_total,
            lane_governance_sealed_aliases: snap.lane_governance_sealed_aliases.clone(),
            lane_governance: snap
                .lane_governance
                .iter()
                .map(|entry| SumeragiLaneGovernance {
                    lane_id: LaneId::new(entry.lane_id),
                    alias: entry.alias.clone(),
                    governance: entry.governance.clone(),
                    manifest_required: entry.manifest_required,
                    manifest_ready: entry.manifest_ready,
                    manifest_path: entry.manifest_path.clone(),
                    validator_ids: entry.validator_ids.clone(),
                    quorum: entry.quorum,
                    protected_namespaces: entry.protected_namespaces.clone(),
                    runtime_upgrade: entry.runtime_upgrade.as_ref().map(|hook| {
                        SumeragiRuntimeUpgradeHook {
                            allow: hook.allow,
                            require_metadata: hook.require_metadata,
                            metadata_key: hook.metadata_key.clone(),
                            allowed_ids: hook.allowed_ids.clone(),
                        }
                    }),
                })
                .collect(),
            worker_loop: SumeragiWorkerLoopStatus {
                stage: snap.worker_loop.stage.as_str().to_owned(),
                stage_started_ms: snap.worker_loop.stage_started_ms,
                last_iteration_ms: snap.worker_loop.last_iteration_ms,
                queue_depths: SumeragiWorkerQueueDepths {
                    vote_rx: snap.worker_loop.queue_depths.vote_rx,
                    block_payload_rx: snap.worker_loop.queue_depths.block_payload_rx,
                    rbc_chunk_rx: snap.worker_loop.queue_depths.rbc_chunk_rx,
                    block_rx: snap.worker_loop.queue_depths.block_rx,
                    consensus_rx: snap.worker_loop.queue_depths.consensus_rx,
                    lane_relay_rx: snap.worker_loop.queue_depths.lane_relay_rx,
                    background_rx: snap.worker_loop.queue_depths.background_rx,
                },
                queue_diagnostics: SumeragiWorkerQueueDiagnostics {
                    blocked_total: SumeragiWorkerQueueTotals {
                        vote_rx: snap.worker_loop.queue_diagnostics.blocked_total.vote_rx,
                        block_payload_rx: snap
                            .worker_loop
                            .queue_diagnostics
                            .blocked_total
                            .block_payload_rx,
                        rbc_chunk_rx: snap.worker_loop.queue_diagnostics.blocked_total.rbc_chunk_rx,
                        block_rx: snap.worker_loop.queue_diagnostics.blocked_total.block_rx,
                        consensus_rx: snap.worker_loop.queue_diagnostics.blocked_total.consensus_rx,
                        lane_relay_rx: snap.worker_loop.queue_diagnostics.blocked_total.lane_relay_rx,
                        background_rx: snap.worker_loop.queue_diagnostics.blocked_total.background_rx,
                    },
                    blocked_ms_total: SumeragiWorkerQueueTotals {
                        vote_rx: snap.worker_loop.queue_diagnostics.blocked_ms_total.vote_rx,
                        block_payload_rx: snap
                            .worker_loop
                            .queue_diagnostics
                            .blocked_ms_total
                            .block_payload_rx,
                        rbc_chunk_rx: snap
                            .worker_loop
                            .queue_diagnostics
                            .blocked_ms_total
                            .rbc_chunk_rx,
                        block_rx: snap.worker_loop.queue_diagnostics.blocked_ms_total.block_rx,
                        consensus_rx: snap
                            .worker_loop
                            .queue_diagnostics
                            .blocked_ms_total
                            .consensus_rx,
                        lane_relay_rx: snap
                            .worker_loop
                            .queue_diagnostics
                            .blocked_ms_total
                            .lane_relay_rx,
                        background_rx: snap
                            .worker_loop
                            .queue_diagnostics
                            .blocked_ms_total
                            .background_rx,
                    },
                    blocked_max_ms: SumeragiWorkerQueueTotals {
                        vote_rx: snap.worker_loop.queue_diagnostics.blocked_max_ms.vote_rx,
                        block_payload_rx: snap
                            .worker_loop
                            .queue_diagnostics
                            .blocked_max_ms
                            .block_payload_rx,
                        rbc_chunk_rx: snap
                            .worker_loop
                            .queue_diagnostics
                            .blocked_max_ms
                            .rbc_chunk_rx,
                        block_rx: snap.worker_loop.queue_diagnostics.blocked_max_ms.block_rx,
                        consensus_rx: snap
                            .worker_loop
                            .queue_diagnostics
                            .blocked_max_ms
                            .consensus_rx,
                        lane_relay_rx: snap
                            .worker_loop
                            .queue_diagnostics
                            .blocked_max_ms
                            .lane_relay_rx,
                        background_rx: snap
                            .worker_loop
                            .queue_diagnostics
                            .blocked_max_ms
                            .background_rx,
                    },
                    dropped_total: SumeragiWorkerQueueTotals {
                        vote_rx: snap.worker_loop.queue_diagnostics.dropped_total.vote_rx,
                        block_payload_rx: snap
                            .worker_loop
                            .queue_diagnostics
                            .dropped_total
                            .block_payload_rx,
                        rbc_chunk_rx: snap.worker_loop.queue_diagnostics.dropped_total.rbc_chunk_rx,
                        block_rx: snap.worker_loop.queue_diagnostics.dropped_total.block_rx,
                        consensus_rx: snap.worker_loop.queue_diagnostics.dropped_total.consensus_rx,
                        lane_relay_rx: snap.worker_loop.queue_diagnostics.dropped_total.lane_relay_rx,
                        background_rx: snap.worker_loop.queue_diagnostics.dropped_total.background_rx,
                    },
                },
            },
            commit_inflight: SumeragiCommitInflightStatus {
                active: snap.commit_inflight.active,
                id: snap.commit_inflight.id,
                height: snap.commit_inflight.height,
                view: snap.commit_inflight.view,
                block_hash: snap.commit_inflight.block_hash,
                started_ms: snap.commit_inflight.started_ms,
                elapsed_ms: snap.commit_inflight.elapsed_ms,
                timeout_ms: snap.commit_inflight.timeout_ms,
                timeout_total: snap.commit_inflight.timeout_total,
                last_timeout_timestamp_ms: snap.commit_inflight.last_timeout_timestamp_ms,
                last_timeout_elapsed_ms: snap.commit_inflight.last_timeout_elapsed_ms,
                last_timeout_height: snap.commit_inflight.last_timeout_height,
                last_timeout_view: snap.commit_inflight.last_timeout_view,
                last_timeout_block_hash: snap.commit_inflight.last_timeout_block_hash,
                pause_total: snap.commit_inflight.pause_total,
                resume_total: snap.commit_inflight.resume_total,
                paused_since_ms: snap.commit_inflight.paused_since_ms,
                pause_queue_depths: SumeragiWorkerQueueDepths {
                    vote_rx: snap.commit_inflight.pause_queue_depths.vote_rx,
                    block_payload_rx: snap.commit_inflight.pause_queue_depths.block_payload_rx,
                    rbc_chunk_rx: snap.commit_inflight.pause_queue_depths.rbc_chunk_rx,
                    block_rx: snap.commit_inflight.pause_queue_depths.block_rx,
                    consensus_rx: snap.commit_inflight.pause_queue_depths.consensus_rx,
                    lane_relay_rx: snap.commit_inflight.pause_queue_depths.lane_relay_rx,
                    background_rx: snap.commit_inflight.pause_queue_depths.background_rx,
                },
                resume_queue_depths: SumeragiWorkerQueueDepths {
                    vote_rx: snap.commit_inflight.resume_queue_depths.vote_rx,
                    block_payload_rx: snap.commit_inflight.resume_queue_depths.block_payload_rx,
                    rbc_chunk_rx: snap.commit_inflight.resume_queue_depths.rbc_chunk_rx,
                    block_rx: snap.commit_inflight.resume_queue_depths.block_rx,
                    consensus_rx: snap.commit_inflight.resume_queue_depths.consensus_rx,
                    lane_relay_rx: snap.commit_inflight.resume_queue_depths.lane_relay_rx,
                    background_rx: snap.commit_inflight.resume_queue_depths.background_rx,
                },
            },
        };
        return Ok(crate::NoritoBody(wire).into_response());
    }
    let payload = status_snapshot_json(&snap);
    let body = norito::json::to_json_pretty(&payload).map_err(|e| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(
            e.to_string(),
        ))
    })?;
    let mut resp = axum::response::Response::new(axum::body::Body::from(body));
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    Ok(resp)
}

/// SSE stream for `/v1/sumeragi/status/sse`, emitting the same payload as the JSON snapshot.
pub fn handle_v1_sumeragi_status_sse(
    poll_ms: u64,
    nexus_enabled: bool,
) -> Sse<impl futures::Stream<Item = Result<SseEvent, Infallible>>> {
    let interval = Duration::from_millis(poll_ms.max(100));
    let ticker = tokio::time::interval(interval);
    let stream = stream::unfold(ticker, move |mut ticker| async move {
        ticker.tick().await;
        let snapshot = sumeragi::status_snapshot();
        let filtered = if nexus_enabled {
            snapshot
        } else {
            snapshot.strip_lane_details()
        };
        let payload = status_snapshot_json(&filtered);
        let body = norito::json::to_json(&payload).unwrap_or_else(|_| "{}".to_owned());
        let ev = SseEvent::default().data(body);
        Some((Ok(ev), ticker))
    });
    Sse::new(stream)
}

fn vrf_summary_json(record: &iroha_data_model::consensus::VrfEpochRecord) -> norito::json::Value {
    let participants_total = u64::try_from(record.participants.len()).unwrap_or(0);
    let commitments_total = u64::try_from(
        record
            .participants
            .iter()
            .filter(|p| p.commitment.is_some())
            .count(),
    )
    .unwrap_or(0);
    let reveals_total = u64::try_from(
        record
            .participants
            .iter()
            .filter(|p| p.reveal.is_some())
            .count(),
    )
    .unwrap_or(0);
    let late_reveals_total = u64::try_from(record.late_reveals.len()).unwrap_or(0);
    let late_reveals: Vec<norito::json::Value> = record
        .late_reveals
        .iter()
        .map(|entry| {
            json_object(vec![
                json_entry("signer", entry.signer),
                json_entry("noted_at_height", entry.noted_at_height),
            ])
        })
        .collect();
    json_object(vec![
        json_entry("found", true),
        json_entry("epoch", record.epoch),
        json_entry("finalized", record.finalized),
        json_entry("seed_hex", hex::encode(record.seed)),
        json_entry("epoch_length", record.epoch_length),
        json_entry("commit_deadline_offset", record.commit_deadline_offset),
        json_entry("reveal_deadline_offset", record.reveal_deadline_offset),
        json_entry("roster_len", record.roster_len),
        json_entry("updated_at_height", record.updated_at_height),
        json_entry("participants_total", participants_total),
        json_entry("commitments_total", commitments_total),
        json_entry("reveals_total", reveals_total),
        json_entry("late_reveals_total", late_reveals_total),
        json_entry(
            "committed_no_reveal",
            json_array::<u32, _>(record.committed_no_reveal.clone()),
        ),
        json_entry(
            "no_participation",
            json_array::<u32, _>(record.no_participation.clone()),
        ),
        json_entry("late_reveals", Value::Array(late_reveals)),
    ])
}

fn vrf_summary_not_found_json(epoch: u64) -> norito::json::Value {
    json_object(vec![
        json_entry("found", false),
        json_entry("epoch", epoch),
        json_entry("finalized", false),
        json_entry("seed_hex", Option::<String>::None),
        json_entry("epoch_length", 0u64),
        json_entry("commit_deadline_offset", 0u64),
        json_entry("reveal_deadline_offset", 0u64),
        json_entry("roster_len", 0u32),
        json_entry("updated_at_height", 0u64),
        json_entry("participants_total", 0u64),
        json_entry("commitments_total", 0u64),
        json_entry("reveals_total", 0u64),
        json_entry("late_reveals_total", 0u64),
        json_entry(
            "committed_no_reveal",
            json_array::<u32, _>(Vec::<u32>::new()),
        ),
        json_entry("no_participation", json_array::<u32, _>(Vec::<u32>::new())),
        json_entry("late_reveals", Value::Array(Vec::new())),
    ])
}

/// GET /v1/sumeragi/telemetry — aggregated collector/QC/RBC metrics snapshot.
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_telemetry(state: Arc<CoreState>) -> Result<impl IntoResponse> {
    let availability = status::availability_snapshot();
    let collectors: Vec<norito::json::Value> = availability
        .collectors
        .into_iter()
        .map(|entry| {
            crate::json_object(vec![
                json_entry("collector_idx", entry.collector_idx),
                json_entry("peer_id", entry.peer.to_string()),
                json_entry("votes_ingested", entry.votes_ingested),
            ])
        })
        .collect();
    let qc_latency = status::qc_latency_snapshot();
    let qc_entries: Vec<norito::json::Value> = qc_latency
        .into_iter()
        .map(|(kind, ms)| {
            crate::json_object(vec![json_entry("kind", kind), json_entry("last_ms", ms)])
        })
        .collect();
    let backlog = status::rbc_backlog_snapshot();
    let pending = status::pending_rbc_snapshot();
    let vrf_snapshot = {
        let view = state.view();
        let mut active: Option<(u64, iroha_data_model::consensus::VrfEpochRecord)> = None;
        let mut latest_final: Option<(u64, iroha_data_model::consensus::VrfEpochRecord)> = None;
        for (epoch, record) in view.world().vrf_epochs().iter() {
            if record.finalized {
                latest_final = Some((*epoch, record.clone()));
            } else {
                active = Some((*epoch, record.clone()));
            }
        }
        active.or(latest_final).map(|(_, record)| record)
    };
    let vrf_json = vrf_snapshot
        .as_ref()
        .map(|record| vrf_summary_json(record))
        .unwrap_or_else(|| vrf_summary_not_found_json(0));
    let payload = crate::json_object(vec![
        json_entry(
            "availability",
            crate::json_object(vec![
                json_entry("total_votes_ingested", availability.total),
                json_entry("collectors", collectors),
            ]),
        ),
        json_entry("qc_latency_ms", qc_entries),
        json_entry(
            "rbc_backlog",
            crate::json_object(vec![
                json_entry("pending_sessions", backlog.pending_sessions),
                json_entry("total_missing_chunks", backlog.total_missing_chunks),
                json_entry("max_missing_chunks", backlog.max_missing_chunks),
            ]),
        ),
        json_entry(
            "rbc_pending",
            crate::json_object(vec![
                json_entry("sessions", pending.sessions),
                json_entry("chunks", pending.chunks),
                json_entry("bytes", pending.bytes),
                json_entry("drops_total", pending.drops_total),
                json_entry("drops_cap_total", pending.drops_cap_total),
                json_entry("drops_cap_bytes_total", pending.drops_cap_bytes_total),
                json_entry("drops_ttl_total", pending.drops_ttl_total),
                json_entry("drops_ttl_bytes_total", pending.drops_ttl_bytes_total),
                json_entry("drops_bytes_total", pending.drops_bytes_total),
                json_entry("evicted_total", pending.evicted_total),
                json_entry("session_cap", pending.session_cap),
                json_entry("max_chunks_per_session", pending.max_chunks_per_session),
                json_entry("max_bytes_per_session", pending.max_bytes_per_session),
                json_entry("ttl_ms", pending.ttl_ms),
            ]),
        ),
        json_entry("vrf", vrf_json),
    ]);
    let body = norito::json::to_json_pretty(&payload).map_err(|e| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(
            e.to_string(),
        ))
    })?;
    let mut resp = axum::response::Response::new(axum::body::Body::from(body));
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    Ok(resp)
}

/// GET /v1/sumeragi/vrf/penalties/{epoch} — epoch VRF penalties snapshot
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_vrf_penalties(
    epoch: axum::extract::Path<String>,
) -> Result<impl IntoResponse> {
    // Parse epoch string as u64 (accept decimal or hex with 0x prefix)
    let ep_str = epoch.0;
    let ep = if let Some(rest) = ep_str.strip_prefix("0x") {
        u64::from_str_radix(rest, 16).map_err(|_| {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "invalid epoch".into(),
                ),
            ))
        })?
    } else {
        ep_str.parse::<u64>().map_err(|_| {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "invalid epoch".into(),
                ),
            ))
        })?
    };
    if let Some(r) = iroha_core::sumeragi::epoch_report::get(ep) {
        let payload = crate::json_object(vec![
            json_entry("epoch", r.epoch),
            json_entry("roster_len", r.roster_len),
            json_entry("committed_no_reveal", r.committed_no_reveal),
            json_entry("no_participation", r.no_participation),
        ]);
        let body = norito::json::to_json_pretty(&payload).map_err(|e| {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(e.to_string()),
            ))
        })?;
        let mut resp = axum::response::Response::new(axum::body::Body::from(body));
        resp.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        Ok(resp)
    } else {
        // 404-like empty JSON (stable shape)
        let payload = crate::json_object(vec![
            json_entry("epoch", ep),
            json_entry("roster_len", 0u64),
            json_entry("committed_no_reveal", Vec::<String>::new()),
            json_entry("no_participation", Vec::<String>::new()),
        ]);
        let body = norito::json::to_json_pretty(&payload).map_err(|e| {
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(e.to_string()),
            ))
        })?;
        let mut resp = axum::response::Response::new(axum::body::Body::from(body));
        resp.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        Ok(resp)
    }
}

/// GET /v1/sumeragi/vrf/epoch/{epoch} — persisted VRF epoch snapshot
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_vrf_epoch(
    state: Arc<CoreState>,
    epoch: u64,
) -> Result<impl IntoResponse> {
    let record_opt = {
        let view = state.view();
        view.world()
            .vrf_epochs()
            .iter()
            .find(|entry| *entry.0 == epoch)
            .map(|(_, rec)| rec.clone())
    };

    let payload = if let Some(record) = record_opt {
        let participants: Vec<Value> = record
            .participants
            .iter()
            .map(|p| {
                let mut entries = vec![
                    json_entry("signer", p.signer),
                    json_entry("last_updated_height", p.last_updated_height),
                ];
                if let Some(commitment) = p.commitment {
                    entries.push(json_entry("commitment", hex::encode(commitment)));
                }
                if let Some(reveal) = p.reveal {
                    entries.push(json_entry("reveal", hex::encode(reveal)));
                }
                json_object(entries)
            })
            .collect();
        crate::json_object(vec![
            json_entry("epoch", record.epoch),
            json_entry("found", true),
            json_entry("seed_hex", hex::encode(record.seed)),
            json_entry("epoch_length", record.epoch_length),
            json_entry("commit_deadline_offset", record.commit_deadline_offset),
            json_entry("reveal_deadline_offset", record.reveal_deadline_offset),
            json_entry("roster_len", record.roster_len),
            json_entry("finalized", record.finalized),
            json_entry("updated_at_height", record.updated_at_height),
            json_entry("participants", Value::Array(participants)),
            json_entry("committed_no_reveal", record.committed_no_reveal.clone()),
            json_entry("no_participation", record.no_participation.clone()),
        ])
    } else {
        crate::json_object(vec![
            json_entry("epoch", epoch),
            json_entry("found", false),
            json_entry("seed_hex", Option::<String>::None),
            json_entry("epoch_length", 0u64),
            json_entry("commit_deadline_offset", 0u64),
            json_entry("reveal_deadline_offset", 0u64),
            json_entry("roster_len", 0u32),
            json_entry("finalized", false),
            json_entry("updated_at_height", 0u64),
            json_entry("participants", Value::Array(Vec::new())),
            json_entry("committed_no_reveal", Vec::<u32>::new()),
            json_entry("no_participation", Vec::<u32>::new()),
        ])
    };

    let body = norito::json::to_json_pretty(&payload).map_err(|e| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(
            e.to_string(),
        ))
    })?;
    let mut resp = axum::response::Response::new(axum::body::Body::from(body));
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    Ok(resp)
}

struct VrfCommitRequestDto {
    pub epoch: u64,
    pub signer: u32,
    pub commitment_hex: String,
}

struct VrfRevealRequestDto {
    pub epoch: u64,
    pub signer: u32,
    pub reveal_hex: String,
}

pub fn handle_post_sumeragi_vrf_commit(
    sumeragi: SumeragiHandle,
    request: VrfCommitRequestDto,
) -> Result<axum::response::Response, Error> {
    let commitment = parse_hex32(&request.commitment_hex, "commitment_hex")?;
    let commit = iroha_data_model::block::consensus::VrfCommit {
        epoch: request.epoch,
        commitment,
        signer: request.signer,
    };
    sumeragi.incoming_block_message(BlockMessage::VrfCommit(commit));
    Ok(StatusCode::ACCEPTED.into_response())
}

pub fn handle_post_sumeragi_vrf_reveal(
    sumeragi: SumeragiHandle,
    request: VrfRevealRequestDto,
) -> Result<axum::response::Response, Error> {
    let reveal = parse_hex32(&request.reveal_hex, "reveal_hex")?;
    let msg = iroha_data_model::block::consensus::VrfReveal {
        epoch: request.epoch,
        reveal,
        signer: request.signer,
    };
    sumeragi.incoming_block_message(BlockMessage::VrfReveal(msg));
    Ok(StatusCode::ACCEPTED.into_response())
}

/// GET /v1/sumeragi/rbc/sessions — RBC session snapshot
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_rbc_sessions() -> Result<impl IntoResponse> {
    let items = rbc_status::snapshot();
    let arr: Vec<norito::json::Value> = items
        .into_iter()
        .map(|s| {
            crate::json_object(vec![
                json_entry("block_hash", hash_to_hex(s.block_hash)),
                json_entry("height", s.height),
                json_entry("view", s.view),
                json_entry("total_chunks", s.total_chunks),
                json_entry("received_chunks", s.received_chunks),
                json_entry("ready_count", s.ready_count),
                json_entry("delivered", s.delivered),
                json_entry(
                    "payload_hash",
                    s.payload_hash.map(|h| hex::encode(h.as_ref())),
                ),
                json_entry("recovered", s.recovered_from_disk),
                json_entry("invalid", s.invalid),
                json_entry(
                    "lane_backlog",
                    norito::json::Value::Array(
                        s.lane_backlog
                            .iter()
                            .map(|entry| {
                                crate::json_object(vec![
                                    json_entry("lane_id", u64::from(entry.lane_id)),
                                    json_entry("tx_count", entry.tx_count),
                                    json_entry("total_chunks", entry.total_chunks),
                                    json_entry("pending_chunks", entry.pending_chunks),
                                    json_entry("rbc_bytes_total", entry.rbc_bytes_total),
                                ])
                            })
                            .collect(),
                    ),
                ),
                json_entry(
                    "dataspace_backlog",
                    norito::json::Value::Array(
                        s.dataspace_backlog
                            .iter()
                            .map(|entry| {
                                crate::json_object(vec![
                                    json_entry("lane_id", u64::from(entry.lane_id)),
                                    json_entry("dataspace_id", entry.dataspace_id),
                                    json_entry("tx_count", entry.tx_count),
                                    json_entry("total_chunks", entry.total_chunks),
                                    json_entry("pending_chunks", entry.pending_chunks),
                                    json_entry("rbc_bytes_total", entry.rbc_bytes_total),
                                ])
                            })
                            .collect(),
                    ),
                ),
            ])
        })
        .collect();
    let payload = crate::json_object(vec![
        json_entry("sessions_active", rbc_status::sessions_active()),
        json_entry("items", arr),
    ]);
    let body = norito::json::to_json_pretty(&payload).map_err(|e| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(
            e.to_string(),
        ))
    })?;
    let mut resp = axum::response::Response::new(axum::body::Body::from(body));
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    Ok(resp)
}

/// GET /v1/sumeragi/rbc — RBC session/throughput counters
#[cfg(feature = "telemetry")]
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_rbc_status(
    telemetry: &MaybeTelemetry,
) -> Result<impl IntoResponse + use<'_>> {
    if !telemetry.allows_developer_outputs() {
        return Err(Error::telemetry_profile_forbidden(
            "sumeragi_rbc_status",
            telemetry.profile(),
        ));
    }

    let m = telemetry.metrics().await;
    let payload = crate::json_object(vec![
        json_entry("sessions_active", m.sumeragi_rbc_sessions_active.get()),
        json_entry(
            "sessions_pruned_total",
            m.sumeragi_rbc_sessions_pruned_total.get(),
        ),
        json_entry(
            "ready_broadcasts_total",
            m.sumeragi_rbc_ready_broadcasts_total.get(),
        ),
        json_entry(
            "deliver_broadcasts_total",
            m.sumeragi_rbc_deliver_broadcasts_total.get(),
        ),
        json_entry(
            "payload_bytes_delivered_total",
            m.sumeragi_rbc_payload_bytes_delivered_total.get(),
        ),
    ]);
    let body = norito::json::to_json_pretty(&payload).map_err(|e| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(
            e.to_string(),
        ))
    })?;
    let mut resp = axum::response::Response::new(axum::body::Body::from(body));
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    Ok(resp)
}

/// GET /v1/sumeragi/rbc/delivered/{height}/{view} — delivery status for a specific (height, view)
/// Returns a compact JSON with `delivered` boolean and a minimal summary when a session exists.
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_rbc_delivered_height_view(
    height_view: axum::extract::Path<(u64, u64)>,
) -> Result<impl IntoResponse> {
    let (height, view) = height_view.0;
    let items = rbc_status::snapshot();
    let mut matches: Vec<_> = items
        .into_iter()
        .filter(|s| s.height == height && s.view == view)
        .collect();
    // Default payload when no session is present
    if matches.is_empty() {
        let payload = crate::json_object(vec![
            json_entry("height", height),
            json_entry("view", view),
            json_entry("delivered", false),
            json_entry("present", false),
            json_entry("block_hash", Value::Null),
            json_entry("ready_count", 0u64),
            json_entry("received_chunks", 0u64),
            json_entry("total_chunks", 0u64),
        ]);
        let body = norito::json::to_json_pretty(&payload).map_err(|e| {
            Error::Query(iroha_data_model::ValidationFail::InternalError(
                e.to_string(),
            ))
        })?;
        let mut resp = axum::response::Response::new(axum::body::Body::from(body));
        resp.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_static("application/json"),
        );
        return Ok(resp);
    }

    // If multiple sessions exist (conflicting proposals), report delivery as true if any reached DELIVER
    let delivered_any = matches.iter().any(|s| s.delivered);
    // Prefer a delivered session to report details; otherwise the first entry
    matches.sort_by_key(|s| (!s.delivered, s.total_chunks));
    let pick = &matches[0];
    let payload = crate::json_object(vec![
        json_entry("height", height),
        json_entry("view", view),
        json_entry("delivered", delivered_any),
        json_entry("present", true),
        json_entry("block_hash", hash_to_hex(pick.block_hash)),
        json_entry("ready_count", pick.ready_count),
        json_entry("received_chunks", pick.received_chunks),
        json_entry("total_chunks", pick.total_chunks),
    ]);
    let body = norito::json::to_json_pretty(&payload).map_err(|e| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(
            e.to_string(),
        ))
    })?;
    let mut resp = axum::response::Response::new(axum::body::Body::from(body));
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    Ok(resp)
}

/// GET /v1/sumeragi/exec_root/{hash} — return execution post-state root for a block hash (if present)
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_exec_root(
    State(state): State<std::sync::Arc<CoreState>>,
    axum::extract::Path(hash_hex): axum::extract::Path<String>,
    accept: Option<axum::http::HeaderValue>,
) -> Result<Response> {
    use core::str::FromStr as _;
    let parsed = iroha_crypto::Hash::from_str(&hash_hex).map_err(|e| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(format!(
            "invalid hash: {}",
            e
        )))
    })?;
    let typed = iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(parsed);
    let view = state.view();
    let root_opt = view.world.exec_roots().get(&typed);
    let exec_root_opt = root_opt.copied();
    let format = match crate::utils::negotiate_response_format(accept.as_ref()) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };

    if matches!(format, crate::utils::ResponseFormat::Norito) {
        let wire = ExecRootWire {
            block_hash: typed,
            exec_root: exec_root_opt.clone(),
        };
        return Ok(crate::NoritoBody(wire).into_response());
    }
    let payload = match exec_root_opt {
        Some(r) => crate::json_object(vec![
            json_entry("block_hash", hash_hex.clone()),
            json_entry("exec_root", format!("{r}")),
        ]),
        None => crate::json_object(vec![
            json_entry("block_hash", hash_hex.clone()),
            json_entry("exec_root", Value::Null),
        ]),
    };
    let body = norito::json::to_json_pretty(&payload).map_err(|e| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(
            e.to_string(),
        ))
    })?;
    let mut resp = axum::response::Response::new(axum::body::Body::from(body));
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    Ok(resp)
}

/// GET /v1/sumeragi/exec_qc/{hash} — return full ExecutionQC record for a block hash (if present)
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_exec_qc(
    State(state): State<std::sync::Arc<CoreState>>,
    axum::extract::Path(hash_hex): axum::extract::Path<String>,
    accept: Option<axum::http::HeaderValue>,
) -> Result<Response> {
    use core::str::FromStr as _;
    let parsed = iroha_crypto::Hash::from_str(&hash_hex).map_err(|e| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(format!(
            "invalid hash: {}",
            e
        )))
    })?;
    let typed = iroha_crypto::HashOf::<BlockHeader>::from_untyped_unchecked(parsed);
    let view = state.view();
    let rec_opt = view.world.exec_qcs().get(&typed);
    let record_opt = rec_opt.cloned();
    let format = match crate::utils::negotiate_response_format(accept.as_ref()) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };

    if matches!(format, crate::utils::ResponseFormat::Norito) {
        return Ok(crate::NoritoBody(record_opt).into_response());
    }
    let payload = match record_opt.as_ref() {
        Some(r) => crate::json_object(vec![
            json_entry("subject_block_hash", hash_hex.clone()),
            json_entry("post_state_root", format!("{}", r.post_state_root)),
            json_entry("height", r.height),
            json_entry("view", r.view),
            json_entry("epoch", r.epoch),
            json_entry("signers_bitmap", hex::encode(&r.signers_bitmap)),
            json_entry(
                "bls_aggregate_signature",
                hex::encode(&r.bls_aggregate_signature),
            ),
        ]),
        None => crate::json_object(vec![
            json_entry("subject_block_hash", hash_hex.clone()),
            json_entry("exec_qc", Value::Null),
        ]),
    };
    let body = norito::json::to_json_pretty(&payload).map_err(|e| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(
            e.to_string(),
        ))
    })?;
    let mut resp = axum::response::Response::new(axum::body::Body::from(body));
    resp.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    Ok(resp)
}
