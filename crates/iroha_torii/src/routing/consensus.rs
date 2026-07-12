//! Consensus-related Torii handlers split out from the main routing module.

use super::*;
use iroha_data_model::prelude::ChainId;
use iroha_data_model::{
    block::consensus::{
        NativeAmxAttestationBodyV2, NativeAmxAttestationQcV2, NativeAmxLegRecordV2, NativeAmxPhase,
        NativeAmxReceipt, SumeragiCommittedLaneBlock, SumeragiProposalGateStatus, SumeragiQcEntry,
        SumeragiV1StatusWire,
    },
    nexus::{DataSpaceId, LaneId},
};

#[derive(Clone, Debug, Encode, Decode)]
struct EvidenceListWire {
    total: u64,
    items: Vec<EvidenceRecord>,
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
    commit_ms: u64,
    pipeline_total_ms: u64,
}

#[derive(Debug, crate::json_macros::JsonSerialize, norito::derive::NoritoSerialize)]
#[allow(clippy::struct_field_names)]
struct SumeragiPhasesMax {
    propose_ms: u64,
    collect_da_ms: u64,
    collect_prevote_ms: u64,
    collect_precommit_ms: u64,
    collect_aggregator_ms: u64,
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
    commit_ms: u64,
    pipeline_total_ms: u64,
    collect_aggregator_gossip_total: u64,
    block_created_dropped_by_lock_total: u64,
    block_created_hint_mismatch_total: u64,
    block_created_proposal_mismatch_total: u64,
    max_ms: SumeragiPhasesMax,
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
    let commit_qc = json_object(vec![
        json_entry("height", snap.commit_qc.height),
        json_entry("view", snap.commit_qc.view),
        json_entry("epoch", snap.commit_qc.epoch),
        json_entry(
            "block_hash",
            snap.commit_qc
                .block_hash
                .map(|hash| Value::from(format!("{hash}")))
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "validator_set_hash",
            snap.commit_qc
                .validator_set_hash
                .map(|hash| Value::from(format!("{hash}")))
                .unwrap_or(Value::Null),
        ),
        json_entry("validator_set_len", snap.commit_qc.validator_set_len),
        json_entry("signatures_total", snap.commit_qc.signatures_total),
    ]);
    let commit_quorum = json_object(vec![
        json_entry("height", snap.commit_quorum.height),
        json_entry("view", snap.commit_quorum.view),
        json_entry(
            "block_hash",
            snap.commit_quorum
                .block_hash
                .map(|hash| Value::from(format!("{hash}")))
                .unwrap_or(Value::Null),
        ),
        json_entry("signatures_present", snap.commit_quorum.signatures_present),
        json_entry("signatures_counted", snap.commit_quorum.signatures_counted),
        json_entry("signatures_set_b", snap.commit_quorum.signatures_set_b),
        json_entry(
            "signatures_required",
            snap.commit_quorum.signatures_required,
        ),
        json_entry("last_updated_ms", snap.commit_quorum.last_updated_ms),
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
        commit_ms: snap.commit_ms,
        pipeline_total_ms: snap.pipeline_total_ms,
        collect_aggregator_gossip_total: snap.gossip_fallback_total,
        block_created_dropped_by_lock_total: snap.block_created_dropped_by_lock_total,
        block_created_hint_mismatch_total: snap.block_created_hint_mismatch_total,
        block_created_proposal_mismatch_total: snap.block_created_proposal_mismatch_total,
        max_ms: SumeragiPhasesMax {
            propose_ms: snap.propose_max_ms,
            collect_da_ms: snap.collect_da_max_ms,
            collect_prevote_ms: snap.collect_prevote_max_ms,
            collect_precommit_ms: snap.collect_precommit_max_ms,
            collect_aggregator_ms: snap.collect_aggregator_max_ms,
            commit_ms: snap.commit_max_ms,
            pipeline_total_ms: snap.pipeline_total_max_ms,
        },
        ema_ms: SumeragiPhasesEma {
            propose_ms: snap.propose_ema_ms,
            collect_da_ms: snap.collect_da_ema_ms,
            collect_prevote_ms: snap.collect_prevote_ema_ms,
            collect_precommit_ms: snap.collect_precommit_ema_ms,
            collect_aggregator_ms: snap.collect_aggregator_ema_ms,
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

/// GET /v1/sumeragi/bls-keys — map of network public keys -> BLS public keys (hex strings)
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_bls_keys(
    State(state): State<Arc<CoreState>>,
    accept: Option<axum::http::HeaderValue>,
) -> Result<Response> {
    // Debug/operator endpoint; non-consensus. Build mapping from world peers where identity key is BLS-normal.
    let world = state.world_view();
    let peers = world.peers().clone();
    let mut obj: std::collections::BTreeMap<String, Option<String>> =
        std::collections::BTreeMap::new();
    for p in peers {
        let net_pk = p.public_key().to_string();
        let bls_pk_val = if matches!(
            p.public_key().try_algorithm(),
            Ok(iroha_crypto::Algorithm::BlsNormal)
        ) {
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
    let world = state.world_view();
    let snap = sumeragi::status_snapshot();
    let peers = state.commit_topology_snapshot();
    let chain_height = u64::try_from(state.committed_height()).unwrap_or(u64::MAX);
    let n = peers.len();
    let fallback_mode = match snap.mode_tag.as_str() {
        iroha_core::sumeragi::consensus::NPOS_TAG | "Npos" => ConsensusMode::Npos,
        iroha_core::sumeragi::consensus::PERMISSIONED_TAG | "Permissioned" => {
            ConsensusMode::Permissioned
        }
        _ => {
            if world.sumeragi_npos_parameters().is_some() {
                ConsensusMode::Npos
            } else {
                ConsensusMode::Permissioned
            }
        }
    };
    let mode = sumeragi::effective_consensus_mode_for_height_from_world(
        &world,
        chain_height,
        fallback_mode,
    );
    let (plan_height, plan_view) =
        collector_plan_context(snap.prf_height, snap.prf_view, chain_height);
    let npos_collector_config = if matches!(mode, ConsensusMode::Npos) {
        sumeragi::load_npos_collector_config_from_world(&world, state.chain_id_ref())
    } else {
        None
    };
    let npos_param_seed = if matches!(mode, ConsensusMode::Npos) {
        world
            .sumeragi_npos_parameters()
            .map(|params| params.epoch_seed())
    } else {
        None
    };
    let seed_from_mode = npos_collector_config
        .map(|cfg| cfg.seed)
        .or(npos_param_seed);
    let prefer_snapshot_seed = snap.prf_height >= chain_height;
    let epoch_seed = match mode {
        ConsensusMode::Permissioned => None,
        ConsensusMode::Npos => {
            if prefer_snapshot_seed {
                snap.prf_epoch_seed.or(seed_from_mode)
            } else {
                seed_from_mode.or(snap.prf_epoch_seed)
            }
        }
    };
    if peers.is_empty() {
        let consensus_mode_label = match mode {
            ConsensusMode::Permissioned => "Permissioned",
            ConsensusMode::Npos => "Npos",
        };
        let epoch_seed_hex = epoch_seed.map(hex::encode);
        let payload = CollectorsResponse {
            consensus_mode: consensus_mode_label,
            mode: consensus_mode_label,
            topology_len: 0,
            min_votes_for_commit: 0,
            proxy_tail_index: 0,
            height: plan_height,
            view: plan_view,
            collectors_k: 0,
            redundant_send_r: 0,
            epoch_seed: epoch_seed_hex.clone(),
            collectors: Vec::new(),
            prf: PrfContext {
                height: plan_height,
                view: plan_view,
                epoch_seed: epoch_seed_hex,
            },
        };
        let format = match crate::utils::negotiate_response_format(accept.as_ref()) {
            Ok(fmt) => fmt,
            Err(resp) => return Ok(resp),
        };
        return Ok(crate::utils::respond_with_format(payload, format));
    }
    let topology = iroha_core::sumeragi::network_topology::Topology::new(peers.clone());
    let min_votes = topology.min_votes_for_commit();
    let tail = topology.proxy_tail_index();
    let available = n.saturating_sub(tail);
    let (mut k_raw, redundant_send_r) = match mode {
        ConsensusMode::Permissioned => {
            let params = world.parameters().sumeragi();
            (
                params.collectors_k as usize,
                params.collectors_redundant_send_r,
            )
        }
        ConsensusMode::Npos => {
            if let Some(cfg) = npos_collector_config {
                (cfg.k, cfg.redundant_send_r)
            } else {
                let params = world.parameters().sumeragi();
                iroha_logger::warn!(
                    "Missing sumeragi_npos_parameters payload; falling back to permissioned collector settings"
                );
                (
                    params.collectors_k as usize,
                    params.collectors_redundant_send_r,
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
    let format = match crate::utils::negotiate_response_format(accept.as_ref()) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };
    Ok(crate::utils::respond_with_format(payload, format))
}

fn collector_plan_context(
    snapshot_height: u64,
    snapshot_view: u64,
    chain_height: u64,
) -> (u64, u64) {
    if snapshot_height >= chain_height {
        (snapshot_height, snapshot_view)
    } else {
        (chain_height, 0)
    }
}

/// GET /v1/sumeragi/params — snapshot of on-chain Sumeragi parameters
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_params(
    State(state): State<std::sync::Arc<CoreState>>,
    accept: Option<axum::http::HeaderValue>,
) -> Result<Response> {
    let world = state.world_view();
    let sp = world.parameters().sumeragi();
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
        chain_height: u64::try_from(state.committed_height()).unwrap_or(u64::MAX),
    };
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
    let world = state.world_view();
    let n = iroha_core::query::evidence_count_from_world(&world) as u64;
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
    /// Optional filter by kind: one of DoublePrepare, DoubleCommit, InvalidQc, InvalidProposal
    pub kind: Option<String>,
}

/// GET /v1/sumeragi/evidence — list recent evidence entries (in-memory audit snapshot).
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_evidence_list(
    State(state): State<std::sync::Arc<CoreState>>,
    crate::NoritoQuery(q): crate::NoritoQuery<EvidenceListQuery>,
    accept: Option<axum::http::HeaderValue>,
) -> Result<Response> {
    let world = state.world_view();
    let mut records = iroha_core::query::evidence_list_snapshot_from_world(&world);
    // Optional kind filter
    if let Some(kind_s) = q.kind.as_deref() {
        use iroha_core::sumeragi::consensus::EvidenceKind;
        let kind_opt = match kind_s {
            "DoublePrepare" | "DoublePrevote" => Some(EvidenceKind::DoublePrepare),
            "DoubleCommit" | "DoublePrecommit" => Some(EvidenceKind::DoubleCommit),
            "InvalidQc" | "InvalidQC" => Some(EvidenceKind::InvalidQc),
            "InvalidProposal" => Some(EvidenceKind::InvalidProposal),
            "Censorship" => Some(EvidenceKind::Censorship),
            "SumeragiV2Equivocation" => Some(EvidenceKind::SumeragiV2Equivocation),
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

/// Handle POST `/v1/sumeragi/evidence`, validating and forwarding consensus evidence.
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
    let cleaned: String = value.chars().filter(|ch| !ch.is_whitespace()).collect();
    let body = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
        .unwrap_or(cleaned.as_str());
    let bytes = hex::decode(body).map_err(|err| {
        Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                "evidence_hex: {err}"
            )),
        ))
    })?;
    norito::decode_from_bytes::<ConsensusEvidence>(&bytes).map_err(|err| {
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
    let topology_peers = state.commit_topology_snapshot();
    let (subject_height, _) = iroha_core::sumeragi::evidence_subject_height_view(&evidence);
    let world = state.world_view();
    let height =
        subject_height.unwrap_or_else(|| u64::try_from(state.committed_height()).unwrap_or(0));
    let prf_seed = Some(iroha_core::sumeragi::npos_seed_for_height_from_world(
        &world,
        state.chain_id_ref(),
        height,
    ));
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
            prf_seed,
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
        block::BlockHeader, consensus::VrfEpochRecord, parameter::system::SumeragiNposParameters,
        peer::PeerId, prelude::ChainId,
    };
    use norito::codec::Encode as _;

    fn checked_consensus_bls_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
            .expect("test consensus BLS fixture key derivation should succeed")
    }

    #[test]
    fn checked_consensus_bls_keypair_uses_fallible_seed_derivation() {
        let first = checked_consensus_bls_keypair(0x30);
        let repeat = checked_consensus_bls_keypair(0x30);
        let second = checked_consensus_bls_keypair(0x31);

        assert_eq!(first.algorithm(), Algorithm::BlsNormal);
        assert_eq!(first.public_key(), repeat.public_key());
        assert_ne!(first.public_key(), second.public_key());
    }

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
            phase: Phase::Prepare,
            block_hash: HashOf::from_untyped_unchecked(hash),
            parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            height,
            view,
            epoch: 0,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let preimage = iroha_core::sumeragi::consensus::vote_preimage(chain_id, mode_tag, &vote);
        let signature = Signature::try_new(keypair.private_key(), &preimage)
            .expect("test fixture signing should succeed");
        let payload = signature.payload().to_vec();
        vote.bls_sig = payload;
        vote
    }

    fn sample_evidence(chain_id: &ChainId, keypair: &KeyPair) -> Evidence {
        let mode_tag = iroha_core::sumeragi::consensus::PERMISSIONED_TAG;
        let v1 = make_vote(chain_id, mode_tag, keypair, 10, 3, 0x11);
        let v2 = make_vote(chain_id, mode_tag, keypair, 10, 3, 0x22);
        Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        }
    }

    #[test]
    fn decode_evidence_hex_accepts_plain_and_prefixed() {
        let chain_id: ChainId = "torii-evidence".parse().expect("chain id parses");
        let keypair = checked_consensus_bls_keypair(0x32);
        let ev = sample_evidence(&chain_id, &keypair);
        let encoded = norito::to_bytes(&ev).expect("encode evidence");
        let plain = hex::encode(&encoded);
        let prefixed = format!("0x{plain}");

        let decoded_plain = decode_evidence_hex(&plain).expect("decode plain hex");
        let decoded_prefixed = decode_evidence_hex(&prefixed).expect("decode 0x hex");

        assert_eq!(decoded_plain.kind, EvidenceKind::DoublePrepare);
        assert_eq!(decoded_prefixed.kind, EvidenceKind::DoublePrepare);
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
        let chain_id: ChainId = "torii-evidence".parse().expect("chain id parses");
        let keypair = checked_consensus_bls_keypair(0x33);
        let ev = sample_evidence(&chain_id, &keypair);
        let mut encoded = norito::to_bytes(&ev).expect("encode evidence");
        encoded.pop();
        let truncated = hex::encode(&encoded);
        let err = decode_evidence_hex(&truncated).expect_err("expect decode failure");
        assert!(matches!(
            err,
            Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(_)
            ))
        ));
    }

    #[test]
    fn decode_evidence_hex_ignores_whitespace() {
        let chain_id: ChainId = "torii-evidence".parse().expect("chain id parses");
        let keypair = checked_consensus_bls_keypair(0x34);
        let ev = sample_evidence(&chain_id, &keypair);
        let encoded = norito::to_bytes(&ev).expect("encode evidence");
        let hex = hex::encode(&encoded);
        let mut spaced = String::from("0x");
        for (idx, chunk) in hex.as_bytes().chunks(4).enumerate() {
            if idx > 0 {
                if idx % 2 == 0 {
                    spaced.push('\n');
                } else {
                    spaced.push(' ');
                }
            }
            spaced.push_str(std::str::from_utf8(chunk).expect("hex chunk"));
        }

        let decoded = decode_evidence_hex(&spaced).expect("decode spaced hex");
        assert_eq!(decoded.kind, EvidenceKind::DoublePrepare);
    }

    #[test]
    fn decode_and_validate_evidence_rejects_structurally_invalid_payload() {
        let chain_id: ChainId = "torii-evidence".parse().expect("chain id parses");
        let keypair = checked_consensus_bls_keypair(0x35);
        let state = test_state_with_peer(PeerId::new(keypair.public_key().clone()));
        let mode_tag = iroha_core::sumeragi::consensus::PERMISSIONED_TAG;
        let vote = make_vote(&chain_id, mode_tag, &keypair, 42, 7, 0xAB);
        let forged = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote {
                v1: vote.clone(),
                v2: vote,
            },
        };
        let encoded = hex::encode(forged.encode());
        let err = decode_and_validate_evidence(&encoded, &state, &chain_id)
            .expect_err("invalid evidence must fail");
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
        let keypair0 = checked_consensus_bls_keypair(0x36);
        let keypair1 = checked_consensus_bls_keypair(0x37);
        let peer0 = PeerId::new(keypair0.public_key().clone());
        let peer1 = PeerId::new(keypair1.public_key().clone());
        let mut peers = vec![peer0.clone(), peer1.clone()];
        peers.sort();
        let topology = iroha_core::sumeragi::network_topology::Topology::new(peers.clone());
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
        assert_ne!(
            leader_epoch0, leader_epoch1,
            "seed search must pick distinct leaders"
        );

        let signer_peer = peers
            .get(leader_epoch0)
            .expect("leader index should be in range");
        let signer_keypair = if signer_peer == &peer0 {
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
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        let encoded = hex::encode(ev.encode());
        let decoded = decode_and_validate_evidence(&encoded, &state, &chain_id)
            .expect("evidence should validate with subject-height seed");
        assert_eq!(decoded.kind, EvidenceKind::DoublePrepare);
    }

    #[test]
    fn decode_and_validate_evidence_permissioned_uses_prf_seed() {
        let chain_id: ChainId = "torii-evidence".parse().expect("chain id parses");
        let keypair0 = checked_consensus_bls_keypair(0x38);
        let keypair1 = checked_consensus_bls_keypair(0x39);
        let peer0 = PeerId::new(keypair0.public_key().clone());
        let peer1 = PeerId::new(keypair1.public_key().clone());
        let mut peers = vec![peer0.clone(), peer1.clone()];
        peers.sort();
        let topology = iroha_core::sumeragi::network_topology::Topology::new(peers.clone());
        let height = 1_u64;
        let view = 0_u64;

        let canonical_leader = topology
            .as_ref()
            .first()
            .expect("topology should have at least one peer")
            .clone();
        let mut seed_epoch1 = None;
        for byte in 0u8..=u8::MAX {
            let seed = [byte; 32];
            let mut rotated = topology.clone();
            rotated.shuffle_prf(seed, height);
            rotated.nth_rotation(view);
            let leader = rotated
                .as_ref()
                .first()
                .expect("rotated topology should have at least one peer");
            if leader != &canonical_leader {
                seed_epoch1 = Some(seed);
                break;
            }
        }
        let seed_epoch1 = seed_epoch1.expect("must find a seed that changes permissioned leader");
        let mut rotated = topology.clone();
        rotated.shuffle_prf(seed_epoch1, height);
        rotated.nth_rotation(view);
        let signer_peer = rotated
            .as_ref()
            .first()
            .expect("rotated topology should have at least one peer");
        let signer_keypair = if signer_peer == &peer0 {
            &keypair0
        } else {
            &keypair1
        };

        let world = iroha_core::state::World::default();
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
                    seed: seed_epoch1,
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
                    seed: [0x00; 32],
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

        let mode_tag = iroha_core::sumeragi::consensus::PERMISSIONED_TAG;
        let mut v1 = make_vote(&chain_id, mode_tag, signer_keypair, height, view, 0x11);
        v1.signer = 0;
        let mut v2 = make_vote(&chain_id, mode_tag, signer_keypair, height, view, 0x22);
        v2.signer = 0;
        let ev = Evidence {
            kind: EvidenceKind::DoublePrepare,
            payload: EvidencePayload::DoubleVote { v1, v2 },
        };
        let encoded = hex::encode(ev.encode());
        let decoded = decode_and_validate_evidence(&encoded, &state, &chain_id)
            .expect("permissioned evidence should validate with PRF-seeded topology");
        assert_eq!(decoded.kind, EvidenceKind::DoublePrepare);
    }
}

#[cfg(feature = "app_api")]
/// GET /v1/sumeragi/new-view/sse — SSE stream of NEW_VIEW counts polled periodically.
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

fn hash_with_prefix<H>(hash: H) -> String
where
    H: core::fmt::Display,
{
    format!("{hash}")
}

fn native_amx_phase_label(phase: NativeAmxPhase) -> &'static str {
    match phase {
        NativeAmxPhase::Prepare => "prepare",
        NativeAmxPhase::Commit => "commit",
    }
}

fn native_amx_phase_json(phase: NativeAmxPhase) -> Value {
    json_object(vec![
        json_entry("phase", native_amx_phase_label(phase)),
        json_entry("detail", Value::Null),
    ])
}

fn native_amx_attestation_body_json(body: &NativeAmxAttestationBodyV2) -> Value {
    json_object(vec![
        json_entry(
            "round",
            json_object(vec![
                json_entry("context_id", hash_with_prefix(body.round.context_id.0)),
                json_entry("height", body.round.height),
                json_entry("view", body.round.view),
            ]),
        ),
        json_entry("epoch", body.epoch),
        json_entry("source_id", hex::encode(body.source_id)),
        json_entry(
            "tx_entrypoint_hash",
            hash_with_prefix(body.tx_entrypoint_hash),
        ),
        json_entry("plan_digest", hash_with_prefix(body.plan_digest)),
        json_entry("phase", native_amx_phase_json(body.phase)),
        json_entry("coordinator_lane_id", body.coordinator_lane_id),
        json_entry("coordinator_dataspace_id", body.coordinator_dataspace_id),
        json_entry("participant_lane_id", body.participant_lane_id),
        json_entry("participant_dataspace_id", body.participant_dataspace_id),
        json_entry(
            "participant_previous_block_height",
            body.participant_previous_block_height,
        ),
        json_entry(
            "participant_previous_block_descriptor_hash",
            body.participant_previous_block_descriptor_hash
                .map(hash_with_prefix),
        ),
        json_entry(
            "participant_lane_block_height",
            body.participant_lane_block_height,
        ),
        json_entry(
            "participant_lane_block_view",
            body.participant_lane_block_view,
        ),
        json_entry(
            "participant_proposal_hash",
            hash_with_prefix(body.participant_proposal_hash),
        ),
        json_entry(
            "participant_settlement_commitment",
            hash_with_prefix(body.participant_settlement_commitment),
        ),
        json_entry(
            "planned_coordinator_block_height",
            body.planned_coordinator_block_height,
        ),
    ])
}

fn native_amx_attestation_qc_json(qc: &NativeAmxAttestationQcV2) -> Value {
    json_object(vec![
        json_entry("body", native_amx_attestation_body_json(&qc.body)),
        json_entry("validator_set_hash_version", qc.validator_set_hash_version),
        json_entry(
            "validator_set_hash",
            hash_with_prefix(qc.validator_set_hash),
        ),
        json_entry(
            "validator_set",
            Value::Array(
                qc.validator_set
                    .iter()
                    .map(|peer| Value::from(peer.to_string()))
                    .collect(),
            ),
        ),
        json_entry(
            "signers_bitmap",
            Value::Array(qc.signers_bitmap.iter().copied().map(Value::from).collect()),
        ),
        json_entry(
            "bls_aggregate_signature",
            hex::encode(&qc.bls_aggregate_signature),
        ),
    ])
}

fn lane_block_qc_signer_count(qc: &iroha_data_model::block::consensus::LaneBlockQcV1) -> u32 {
    qc.signers_bitmap
        .iter()
        .map(|byte| byte.count_ones())
        .sum::<u32>()
}

fn lane_block_qc_summary_json(qc: &iroha_data_model::block::consensus::LaneBlockQcV1) -> Value {
    let signer_count = lane_block_qc_signer_count(qc);
    json_object(vec![
        json_entry("phase", format!("{:?}", qc.body.phase).to_ascii_lowercase()),
        json_entry("validator_set_hash_version", qc.validator_set_hash_version),
        json_entry(
            "validator_set_hash",
            hash_with_prefix(qc.validator_set_hash),
        ),
        json_entry("validator_count", u64::from(qc.body.validator_count)),
        json_entry("min_quorum", u64::from(qc.body.min_quorum)),
        json_entry("signer_count", u64::from(signer_count)),
    ])
}

fn committed_lane_block_wire(
    entry: &sumeragi::status::CommittedLaneBlockSnapshot,
) -> SumeragiCommittedLaneBlock {
    SumeragiCommittedLaneBlock {
        lane_id: entry.lane_id,
        dataspace_id: entry.dataspace_id,
        lane_incarnation: entry.proposal.descriptor.lane_incarnation,
        lane_block_height: entry.lane_block_height,
        lane_block_view: entry.lane_block_view,
        descriptor_hash: entry.descriptor_hash,
        proposal_hash: entry.proposal_hash,
        execution_status: entry.execution_status.as_str().to_owned(),
        executable_payload_available: entry.executable_payload_available(),
        subject_hash: entry.proposal.descriptor.subject_hash,
        payload_ownership_hash: entry.proposal.descriptor.payload_ownership_hash,
        rbc_instance_hash: entry.proposal.descriptor.rbc_instance_hash,
        qc_mode_tag: entry.proposal.descriptor.qc_mode_tag.clone(),
        validator_count: entry.proposal.descriptor.validator_count,
        min_quorum: entry.proposal.descriptor.min_quorum,
        prepare_qc_signer_count: lane_block_qc_signer_count(&entry.prepare_qc),
        commit_qc_signer_count: lane_block_qc_signer_count(&entry.commit_qc),
    }
}

fn committed_lane_block_json(entry: &sumeragi::status::CommittedLaneBlockSnapshot) -> Value {
    json_object(vec![
        json_entry("lane_id", Value::from(u64::from(entry.lane_id.as_u32()))),
        json_entry("dataspace_id", Value::from(entry.dataspace_id.as_u64())),
        json_entry(
            "lane_incarnation",
            hash_with_prefix(entry.proposal.descriptor.lane_incarnation),
        ),
        json_entry("lane_block_height", entry.lane_block_height),
        json_entry("lane_block_view", entry.lane_block_view),
        json_entry("descriptor_hash", hash_with_prefix(entry.descriptor_hash)),
        json_entry("proposal_hash", hash_with_prefix(entry.proposal_hash)),
        json_entry("execution_status", entry.execution_status.as_str()),
        json_entry(
            "executable_payload_available",
            entry.executable_payload_available(),
        ),
        json_entry(
            "subject_hash",
            hash_with_prefix(entry.proposal.descriptor.subject_hash),
        ),
        json_entry(
            "payload_ownership_hash",
            hash_with_prefix(entry.proposal.descriptor.payload_ownership_hash),
        ),
        json_entry(
            "rbc_instance_hash",
            hash_with_prefix(entry.proposal.descriptor.rbc_instance_hash),
        ),
        json_entry("qc_mode_tag", entry.proposal.descriptor.qc_mode_tag.clone()),
        json_entry("prepare_qc", lane_block_qc_summary_json(&entry.prepare_qc)),
        json_entry("commit_qc", lane_block_qc_summary_json(&entry.commit_qc)),
    ])
}

fn native_amx_leg_json(leg: &NativeAmxLegRecordV2) -> Value {
    json_object(vec![
        json_entry("lane_id", leg.lane_id),
        json_entry("dataspace_id", leg.dataspace_id),
        json_entry(
            "participant_proposal",
            json_value(&leg.participant_proposal),
        ),
        json_entry(
            "participant_settlement",
            json_value(&leg.participant_settlement),
        ),
        json_entry(
            "participant_settlement_hash",
            hash_with_prefix(leg.participant_settlement_hash),
        ),
        json_entry(
            "prepare_qc",
            native_amx_attestation_qc_json(&leg.prepare_qc),
        ),
        json_entry("commit_qc", native_amx_attestation_qc_json(&leg.commit_qc)),
    ])
}

fn native_amx_receipt_json(receipt: &NativeAmxReceipt) -> Value {
    json_object(vec![
        json_entry("version", receipt.version),
        json_entry("source_id", hex::encode(receipt.source_id)),
        json_entry("chain_id_hash", hash_with_prefix(receipt.chain_id_hash)),
        json_entry("plan_digest", hash_with_prefix(receipt.plan_digest)),
        json_entry("lane_id", receipt.lane_id),
        json_entry("dataspace_id", receipt.dataspace_id),
        json_entry(
            "lane_incarnation",
            hash_with_prefix(receipt.lane_incarnation),
        ),
        json_entry("authority_context_height", receipt.authority_context_height),
        json_entry("lane_block_height", receipt.lane_block_height),
        json_entry("lane_block_view", receipt.lane_block_view),
        json_entry(
            "coordinator_proposal_hash",
            hash_with_prefix(receipt.coordinator_proposal_hash),
        ),
        json_entry(
            "legs",
            Value::Array(receipt.legs.iter().map(native_amx_leg_json).collect()),
        ),
    ])
}

fn nexus_fee_snapshot_value(fee: &sumeragi::status::NexusFeeSnapshot) -> Value {
    json_object(vec![
        json_entry("charged_total", fee.charged_total),
        json_entry("charged_via_payer_total", fee.charged_via_payer_total),
        json_entry("charged_via_sponsor_total", fee.charged_via_sponsor_total),
        json_entry("sponsor_disabled_total", fee.sponsor_disabled_total),
        json_entry("sponsor_unauthorized_total", fee.sponsor_unauthorized_total),
        json_entry("sponsor_cap_exceeded_total", fee.sponsor_cap_exceeded_total),
        json_entry("config_errors_total", fee.config_errors_total),
        json_entry("transfer_failures_total", fee.transfer_failures_total),
        json_entry(
            "last_amount",
            fee.last_amount
                .as_ref()
                .map(|amount| Value::from(amount.to_string()))
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

fn sumeragi_v1_pending_finality(snap: &sumeragi::StatusSnapshot) -> Option<HashOf<BlockHeader>> {
    if let Some(block_hash) = snap.canonical_pending_finality {
        return Some(block_hash);
    }
    let settled = snap
        .qc_deferred_resolved_total
        .saturating_add(snap.qc_deferred_expired_total);
    if snap.qc_deferred_missing_payload_total <= settled {
        return None;
    }
    let quorum_complete = snap.commit_quorum.signatures_required > 0
        && snap.commit_quorum.signatures_counted >= snap.commit_quorum.signatures_required;
    (quorum_complete && snap.commit_quorum.height > snap.commit_qc.height)
        .then_some(snap.commit_quorum.block_hash)
        .flatten()
}

fn sumeragi_v1_phase(snap: &sumeragi::StatusSnapshot) -> &'static str {
    if sumeragi_v1_pending_finality(snap).is_some() {
        "pending_finality"
    } else if snap.commit_inflight.active {
        "commit"
    } else if snap.commit_quorum.signatures_present > 0 || snap.highest_qc_height > 0 {
        "prepare"
    } else {
        "proposal"
    }
}

fn sumeragi_v1_quorum_policy(
    snap: &sumeragi::StatusSnapshot,
) -> Option<iroha_data_model::block::consensus::QuorumPolicy> {
    if snap.mode_tag == iroha_data_model::block::consensus::NPOS_TAG {
        return None;
    }
    let validators = u32::try_from(snap.commit_qc.validator_set_len).ok()?;
    (validators > 0)
        .then_some(iroha_data_model::block::consensus::QuorumPolicy::PermissionedCount(validators))
}

fn sumeragi_v1_payload_status(snap: &sumeragi::StatusSnapshot) -> &'static str {
    let settled = snap
        .qc_deferred_resolved_total
        .saturating_add(snap.qc_deferred_expired_total);
    if snap.qc_deferred_missing_payload_total > settled
        || matches!(
            snap.da_gate.reason,
            sumeragi::status::DaGateReasonSnapshot::MissingLocalData
        )
    {
        "missing_local_payload"
    } else {
        "available"
    }
}

fn sumeragi_v1_rbc_status(snap: &sumeragi::StatusSnapshot) -> &'static str {
    if snap.pending_rbc.sessions > 0 {
        "pending"
    } else if snap
        .consensus_caps
        .as_ref()
        .is_some_and(|caps| caps.da_enabled)
    {
        "advisory"
    } else {
        "disabled"
    }
}

fn sumeragi_v1_height_view(snap: &sumeragi::StatusSnapshot) -> (u64, u64) {
    if snap.membership_height > 0 || snap.membership_view > 0 {
        return (snap.membership_height, snap.membership_view);
    }
    let height = snap
        .highest_qc_height
        .max(snap.locked_qc_height)
        .max(snap.commit_qc.height)
        .max(snap.commit_quorum.height);
    let view = snap
        .highest_qc_view
        .max(snap.locked_qc_view)
        .max(snap.commit_qc.view)
        .max(snap.commit_quorum.view);
    (height, view)
}

fn sumeragi_v1_status_wire(snap: &sumeragi::StatusSnapshot) -> SumeragiV1StatusWire {
    let (height, view) = sumeragi_v1_height_view(snap);
    SumeragiV1StatusWire {
        height,
        view,
        phase: sumeragi_v1_phase(snap).to_owned(),
        leader_index: snap.leader_index,
        highest_qc: SumeragiQcEntry {
            height: snap.highest_qc_height,
            view: snap.highest_qc_view,
            subject_block_hash: snap.highest_qc_subject,
        },
        locked_qc: SumeragiQcEntry {
            height: snap.locked_qc_height,
            view: snap.locked_qc_view,
            subject_block_hash: snap.locked_qc_subject,
        },
        pending_finality: sumeragi_v1_pending_finality(snap),
        validator_set_id: snap
            .commit_qc
            .validator_set_hash
            .map(|hash| iroha_data_model::block::consensus::ValidatorSetId { hash }),
        quorum_policy: sumeragi_v1_quorum_policy(snap),
        payload_status: sumeragi_v1_payload_status(snap).to_owned(),
        rbc_status: sumeragi_v1_rbc_status(snap).to_owned(),
    }
}

fn sumeragi_v1_status_json(snap: &sumeragi::StatusSnapshot) -> norito::json::Value {
    let wire = sumeragi_v1_status_wire(snap);
    json_object(vec![
        json_entry("height", wire.height),
        json_entry("view", wire.view),
        json_entry("phase", wire.phase),
        json_entry("leader_index", wire.leader_index),
        json_entry(
            "highest_qc",
            json_object(vec![
                json_entry("height", wire.highest_qc.height),
                json_entry("view", wire.highest_qc.view),
                json_entry(
                    "subject_block_hash",
                    wire.highest_qc
                        .subject_block_hash
                        .map(|hash| Value::from(format!("{hash}")))
                        .unwrap_or(Value::Null),
                ),
            ]),
        ),
        json_entry(
            "locked_qc",
            json_object(vec![
                json_entry("height", wire.locked_qc.height),
                json_entry("view", wire.locked_qc.view),
                json_entry(
                    "subject_block_hash",
                    wire.locked_qc
                        .subject_block_hash
                        .map(|hash| Value::from(format!("{hash}")))
                        .unwrap_or(Value::Null),
                ),
            ]),
        ),
        json_entry(
            "pending_finality",
            wire.pending_finality
                .map(|hash| Value::from(format!("{hash}")))
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "validator_set_id",
            wire.validator_set_id
                .map(|id| Value::from(format!("{}", id.hash)))
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "quorum_policy",
            wire.quorum_policy
                .map(|policy| match policy {
                    iroha_data_model::block::consensus::QuorumPolicy::PermissionedCount(
                        validators,
                    ) => json_object(vec![
                        json_entry("kind", "permissioned_count"),
                        json_entry("validators", validators),
                    ]),
                    iroha_data_model::block::consensus::QuorumPolicy::NposStake(total) => {
                        json_object(vec![
                            json_entry("kind", "npos_stake"),
                            json_entry("total_stake", total.to_string()),
                        ])
                    }
                })
                .unwrap_or(Value::Null),
        ),
        json_entry("payload_status", wire.payload_status),
        json_entry("rbc_status", wire.rbc_status),
    ])
}

fn proposal_gate_status(
    gate: sumeragi::status::ProposalGateSnapshot,
) -> SumeragiProposalGateStatus {
    SumeragiProposalGateStatus {
        height: gate.height,
        view: gate.view,
        queue_len: gate.queue_len,
        pending_blocks_total: gate.pending_blocks_total,
        pending_blocks_blocking: gate.pending_blocks_blocking,
        active_pending_for_tip: gate.active_pending_for_tip,
        queue_saturated: gate.queue_saturated,
        active_pending: gate.active_pending,
        rbc_backlog: gate.rbc_backlog,
        relay_backpressure: gate.relay_backpressure,
        consensus_queue_backpressure: gate.consensus_queue_backpressure,
        should_defer: gate.should_defer,
        only_pacing_backpressure: gate.only_pacing_backpressure,
        commit_inflight_active: gate.commit_inflight_active,
        cached_proposal_present: gate.cached_proposal_present,
        cached_proposal_hint_present: gate.cached_proposal_hint_present,
        round_liveness_present: gate.round_liveness_present,
        frontier_owner_present: gate.frontier_owner_present,
        missing_qc_liveness_active: gate.missing_qc_liveness_active,
        last_pacemaker_attempt_age_ms: gate.last_pacemaker_attempt_age_ms,
        last_successful_proposal_age_ms: gate.last_successful_proposal_age_ms,
    }
}

fn proposal_gate_json(gate: sumeragi::status::ProposalGateSnapshot) -> norito::json::Value {
    json_object(vec![
        json_entry("height", gate.height),
        json_entry("view", gate.view),
        json_entry("queue_len", gate.queue_len),
        json_entry("pending_blocks_total", gate.pending_blocks_total),
        json_entry("pending_blocks_blocking", gate.pending_blocks_blocking),
        json_entry("active_pending_for_tip", gate.active_pending_for_tip),
        json_entry("queue_saturated", gate.queue_saturated),
        json_entry("active_pending", gate.active_pending),
        json_entry("rbc_backlog", gate.rbc_backlog),
        json_entry("relay_backpressure", gate.relay_backpressure),
        json_entry(
            "consensus_queue_backpressure",
            gate.consensus_queue_backpressure,
        ),
        json_entry("should_defer", gate.should_defer),
        json_entry("only_pacing_backpressure", gate.only_pacing_backpressure),
        json_entry("commit_inflight_active", gate.commit_inflight_active),
        json_entry("cached_proposal_present", gate.cached_proposal_present),
        json_entry(
            "cached_proposal_hint_present",
            gate.cached_proposal_hint_present,
        ),
        json_entry("round_liveness_present", gate.round_liveness_present),
        json_entry("frontier_owner_present", gate.frontier_owner_present),
        json_entry(
            "missing_qc_liveness_active",
            gate.missing_qc_liveness_active,
        ),
        json_entry(
            "last_pacemaker_attempt_age_ms",
            gate.last_pacemaker_attempt_age_ms,
        ),
        json_entry(
            "last_successful_proposal_age_ms",
            gate.last_successful_proposal_age_ms,
        ),
    ])
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
        json_entry(
            "stake_quorum_timeout_total",
            snap.view_change_causes.stake_quorum_timeout_total,
        ),
        json_entry(
            "roster_unavailable_total",
            snap.view_change_causes.roster_unavailable_total,
        ),
        json_entry("da_gate_total", snap.view_change_causes.da_gate_total),
        json_entry(
            "censorship_evidence_total",
            snap.view_change_causes.censorship_evidence_total,
        ),
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
            "last_stake_quorum_timeout_timestamp_ms",
            snap.view_change_causes
                .last_stake_quorum_timeout_timestamp_ms,
        ),
        json_entry(
            "last_roster_unavailable_timestamp_ms",
            snap.view_change_causes.last_roster_unavailable_timestamp_ms,
        ),
        json_entry(
            "last_da_gate_timestamp_ms",
            snap.view_change_causes.last_da_gate_timestamp_ms,
        ),
        json_entry(
            "last_censorship_evidence_timestamp_ms",
            snap.view_change_causes
                .last_censorship_evidence_timestamp_ms,
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
        json_entry(
            "vote_expired_total",
            snap.dedup_evictions.vote_expired_total,
        ),
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
            "block_sync_update_capacity_total",
            snap.dedup_evictions.block_sync_update_capacity_total,
        ),
        json_entry(
            "block_sync_update_expired_total",
            snap.dedup_evictions.block_sync_update_expired_total,
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
        json_entry(
            "rbc_chunk_capacity_total",
            snap.dedup_evictions.rbc_chunk_capacity_total,
        ),
        json_entry(
            "rbc_chunk_expired_total",
            snap.dedup_evictions.rbc_chunk_expired_total,
        ),
        json_entry(
            "lane_block_artifact_capacity_total",
            snap.dedup_evictions.lane_block_artifact_capacity_total,
        ),
        json_entry(
            "lane_block_artifact_expired_total",
            snap.dedup_evictions.lane_block_artifact_expired_total,
        ),
    ]);
    let consensus_message_handling_entries = Value::Array(
        snap.consensus_message_handling
            .entries
            .iter()
            .map(|entry| {
                json_object(vec![
                    json_entry("kind", entry.kind.as_str()),
                    json_entry("outcome", entry.outcome.as_str()),
                    json_entry("reason", entry.reason.as_str()),
                    json_entry("total", entry.total),
                ])
            })
            .collect(),
    );
    let consensus_message_handling = json_object(vec![json_entry(
        "entries",
        consensus_message_handling_entries,
    )]);
    let vote_validation_drop_entries = Value::Array(
        snap.vote_validation_drops
            .entries
            .iter()
            .map(|entry| {
                json_object(vec![
                    json_entry("reason", entry.reason.as_str()),
                    json_entry("height", entry.height),
                    json_entry("view", entry.view),
                    json_entry("epoch", entry.epoch),
                    json_entry("signer_index", entry.signer_index),
                    json_entry(
                        "peer_id",
                        entry
                            .peer_id
                            .as_ref()
                            .map(|peer| Value::from(format!("{peer}")))
                            .unwrap_or(Value::Null),
                    ),
                    json_entry(
                        "roster_hash",
                        entry
                            .roster_hash
                            .map(|hash| Value::from(format!("{hash}")))
                            .unwrap_or(Value::Null),
                    ),
                    json_entry("roster_len", entry.roster_len),
                    json_entry("block_hash", Value::from(format!("{}", entry.block_hash))),
                    json_entry("timestamp_ms", entry.timestamp_ms),
                ])
            })
            .collect(),
    );
    let vote_validation_drop_peer_entries = Value::Array(
        snap.vote_validation_drops
            .peer_entries
            .iter()
            .map(|entry| {
                let reasons = Value::Array(
                    entry
                        .reasons
                        .iter()
                        .map(|reason| {
                            json_object(vec![
                                json_entry("reason", reason.reason.as_str()),
                                json_entry("total", reason.total),
                            ])
                        })
                        .collect(),
                );
                json_object(vec![
                    json_entry("peer_id", Value::from(format!("{}", entry.peer_id))),
                    json_entry(
                        "roster_hash",
                        entry
                            .roster_hash
                            .map(|hash| Value::from(format!("{hash}")))
                            .unwrap_or(Value::Null),
                    ),
                    json_entry("roster_len", entry.roster_len),
                    json_entry("total", entry.total),
                    json_entry("reasons", reasons),
                    json_entry("last_height", entry.last_height),
                    json_entry("last_view", entry.last_view),
                    json_entry("last_epoch", entry.last_epoch),
                    json_entry("last_timestamp_ms", entry.last_timestamp_ms),
                ])
            })
            .collect(),
    );
    let vote_validation_drops = json_object(vec![
        json_entry("total", snap.vote_validation_drops.total),
        json_entry("entries", vote_validation_drop_entries),
        json_entry("peer_entries", vote_validation_drop_peer_entries),
    ]);
    let tx_queue = json_object(vec![
        json_entry("depth", snap.tx_queue_depth),
        json_entry("capacity", snap.tx_queue_capacity),
        json_entry("retained_bytes", snap.tx_queue_retained_bytes),
        json_entry("max_retained_bytes", snap.tx_queue_max_retained_bytes),
        json_entry("saturated", snap.tx_queue_saturated),
        json_entry("saturated_by_count", snap.tx_queue_saturated_by_count),
        json_entry("saturated_by_bytes", snap.tx_queue_saturated_by_bytes),
        json_entry("saturated_by_age", snap.tx_queue_saturated_by_age),
        json_entry("oldest_queued_age_ms", snap.tx_queue_oldest_queued_age_ms),
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
    let commit_pause_queue_depths = queue_depths_value(&snap.commit_inflight.pause_queue_depths);
    let commit_resume_queue_depths = queue_depths_value(&snap.commit_inflight.resume_queue_depths);
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
        json_entry(
            "last_timeout_height",
            snap.commit_inflight.last_timeout_height,
        ),
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
    let commit_pipeline = json_object(vec![
        json_entry("last_total_ms", snap.commit_pipeline.last_total_ms),
        json_entry(
            "last_validation_ms",
            snap.commit_pipeline.last_validation_ms,
        ),
        json_entry(
            "last_qc_rebuild_ms",
            snap.commit_pipeline.last_qc_rebuild_ms,
        ),
        json_entry("last_gate_ms", snap.commit_pipeline.last_gate_ms),
        json_entry("last_finalize_ms", snap.commit_pipeline.last_finalize_ms),
        json_entry(
            "last_drain_results_ms",
            snap.commit_pipeline.last_drain_results_ms,
        ),
        json_entry(
            "last_drain_qc_verify_ms",
            snap.commit_pipeline.last_drain_qc_verify_ms,
        ),
        json_entry(
            "last_drain_persist_ms",
            snap.commit_pipeline.last_drain_persist_ms,
        ),
        json_entry(
            "last_drain_kura_store_ms",
            snap.commit_pipeline.last_drain_kura_store_ms,
        ),
        json_entry(
            "last_drain_state_apply_ms",
            snap.commit_pipeline.last_drain_state_apply_ms,
        ),
        json_entry(
            "last_drain_state_commit_ms",
            snap.commit_pipeline.last_drain_state_commit_ms,
        ),
        json_entry("ema_total_ms", snap.commit_pipeline.ema_total_ms),
        json_entry("ema_validation_ms", snap.commit_pipeline.ema_validation_ms),
        json_entry("ema_gate_ms", snap.commit_pipeline.ema_gate_ms),
        json_entry("ema_finalize_ms", snap.commit_pipeline.ema_finalize_ms),
    ]);
    let round_gap = json_object(vec![
        json_entry(
            "last_deliver_to_state_commit_ms",
            snap.round_gap.last_deliver_to_state_commit_ms,
        ),
        json_entry(
            "last_state_commit_to_next_propose_ms",
            snap.round_gap.last_state_commit_to_next_propose_ms,
        ),
        json_entry(
            "last_deliver_to_next_propose_ms",
            snap.round_gap.last_deliver_to_next_propose_ms,
        ),
        json_entry(
            "ema_deliver_to_state_commit_ms",
            snap.round_gap.ema_deliver_to_state_commit_ms,
        ),
        json_entry(
            "ema_state_commit_to_next_propose_ms",
            snap.round_gap.ema_state_commit_to_next_propose_ms,
        ),
        json_entry(
            "ema_deliver_to_next_propose_ms",
            snap.round_gap.ema_deliver_to_next_propose_ms,
        ),
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
        json_entry("rollback_last_height", snap.kura_store.rollback_last_height),
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
        json_entry("lock_reset_last_view", snap.kura_store.lock_reset_last_view),
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
            "missing_local_data_total",
            snap.da_gate.missing_local_data_total,
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
        json_entry(
            "stake_quorum_timeout_total",
            snap.view_change_causes.stake_quorum_timeout_total,
        ),
        json_entry(
            "roster_unavailable_total",
            snap.view_change_causes.roster_unavailable_total,
        ),
        json_entry("da_gate_total", snap.view_change_causes.da_gate_total),
        json_entry(
            "censorship_evidence_total",
            snap.view_change_causes.censorship_evidence_total,
        ),
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
        json_entry(
            "last_commit_failure_timestamp_ms",
            snap.view_change_causes.last_commit_failure_timestamp_ms,
        ),
        json_entry(
            "last_quorum_timeout_timestamp_ms",
            snap.view_change_causes.last_quorum_timeout_timestamp_ms,
        ),
        json_entry(
            "last_stake_quorum_timeout_timestamp_ms",
            snap.view_change_causes
                .last_stake_quorum_timeout_timestamp_ms,
        ),
        json_entry(
            "last_roster_unavailable_timestamp_ms",
            snap.view_change_causes.last_roster_unavailable_timestamp_ms,
        ),
        json_entry(
            "last_da_gate_timestamp_ms",
            snap.view_change_causes.last_da_gate_timestamp_ms,
        ),
        json_entry(
            "last_censorship_evidence_timestamp_ms",
            snap.view_change_causes
                .last_censorship_evidence_timestamp_ms,
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
    let block_sync_roster = json_object(vec![
        json_entry(
            "commit_qc_hint_total",
            snap.block_sync_roster.commit_qc_hint_total,
        ),
        json_entry(
            "checkpoint_hint_total",
            snap.block_sync_roster.checkpoint_hint_total,
        ),
        json_entry(
            "commit_qc_history_total",
            snap.block_sync_roster.commit_qc_history_total,
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
        json_entry(
            "drop_unsolicited_share_blocks_total",
            snap.block_sync_roster.drop_unsolicited_share_blocks_total,
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
    let lane_settlement_commitments = Value::Array(
        snap.lane_settlement_commitments
            .iter()
            .map(|entry| {
                let receipts = Value::Array(
                    entry
                        .receipts
                        .iter()
                        .map(|receipt| {
                            json_object(vec![
                                json_entry("source_id", hex::encode(receipt.source_id)),
                                json_entry(
                                    "local_amount_micro",
                                    receipt.local_amount_micro.to_string(),
                                ),
                                json_entry("xor_due_micro", receipt.xor_due_micro.to_string()),
                                json_entry(
                                    "xor_after_haircut_micro",
                                    receipt.xor_after_haircut_micro.to_string(),
                                ),
                                json_entry(
                                    "xor_variance_micro",
                                    receipt.xor_variance_micro.to_string(),
                                ),
                                json_entry("timestamp_ms", receipt.timestamp_ms),
                            ])
                        })
                        .collect(),
                );
                let native_amx_receipts = Value::Array(
                    entry
                        .native_amx_receipts
                        .iter()
                        .map(native_amx_receipt_json)
                        .collect(),
                );
                let swap_metadata = entry
                    .swap_metadata
                    .as_ref()
                    .map(|meta| {
                        json_object(vec![
                            json_entry("epsilon_bps", meta.epsilon_bps),
                            json_entry("twap_window_seconds", meta.twap_window_seconds),
                            json_entry(
                                "liquidity_profile",
                                Value::from(format!("{:?}", meta.liquidity_profile)),
                            ),
                            json_entry("twap_local_per_xor", meta.twap_local_per_xor.clone()),
                            json_entry(
                                "volatility_class",
                                Value::from(format!("{:?}", meta.volatility_class)),
                            ),
                        ])
                    })
                    .unwrap_or(Value::Null);
                json_object(vec![
                    json_entry("block_height", entry.block_height),
                    json_entry("lane_id", entry.lane_id),
                    json_entry("dataspace_id", entry.dataspace_id),
                    json_entry("tx_count", entry.tx_count),
                    json_entry("total_local_micro", entry.total_local_micro.to_string()),
                    json_entry("total_xor_due_micro", entry.total_xor_due_micro.to_string()),
                    json_entry(
                        "total_xor_after_haircut_micro",
                        entry.total_xor_after_haircut_micro.to_string(),
                    ),
                    json_entry(
                        "total_xor_variance_micro",
                        entry.total_xor_variance_micro.to_string(),
                    ),
                    json_entry("swap_metadata", swap_metadata),
                    json_entry("receipts", receipts),
                    json_entry(
                        "nexus_fee_receipts",
                        crate::json_value(&entry.nexus_fee_receipts),
                    ),
                    json_entry("native_amx_receipts", native_amx_receipts),
                ])
            })
            .collect(),
    );
    let committed_lane_blocks = Value::Array(
        snap.committed_lane_blocks
            .iter()
            .map(committed_lane_block_json)
            .collect(),
    );
    let lane_block_sessions = Value::Array(
        snap.lane_block_sessions
            .iter()
            .map(|entry| {
                json::to_value(entry).expect("serialize lane-block session status for status")
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
        json_entry("manifest_hints", snap.access_set_sources.manifest_hints),
        json_entry("entrypoint_hints", snap.access_set_sources.entrypoint_hints),
        json_entry("prepass_merge", snap.access_set_sources.prepass_merge),
        json_entry(
            "conservative_fallback",
            snap.access_set_sources.conservative_fallback,
        ),
    ]);
    let pipeline_execution = json_object(vec![
        json_entry(
            "tx_vertices_total",
            snap.pipeline_execution.tx_vertices_total,
        ),
        json_entry("tx_edges_total", snap.pipeline_execution.tx_edges_total),
        json_entry(
            "overlay_count_total",
            snap.pipeline_execution.overlay_count_total,
        ),
        json_entry(
            "overlay_instr_total",
            snap.pipeline_execution.overlay_instr_total,
        ),
        json_entry(
            "overlay_bytes_total",
            snap.pipeline_execution.overlay_bytes_total,
        ),
        json_entry("rbc_chunks_total", snap.pipeline_execution.rbc_chunks_total),
        json_entry("rbc_bytes_total", snap.pipeline_execution.rbc_bytes_total),
        json_entry(
            "detached_prepared_total",
            snap.pipeline_execution.detached_prepared_total,
        ),
        json_entry(
            "detached_merged_total",
            snap.pipeline_execution.detached_merged_total,
        ),
        json_entry(
            "detached_fallback_total",
            snap.pipeline_execution.detached_fallback_total,
        ),
        json_entry(
            "quarantine_executed_total",
            snap.pipeline_execution.quarantine_executed_total,
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
        json_entry("persist_drops_total", snap.rbc_store_persist_drops_total),
        json_entry("evictions_total", snap.rbc_store_evictions_total),
        json_entry("recent_evictions", recent_evictions),
    ]);
    let rbc_mismatch_entries = Value::Array(
        snap.rbc_mismatch
            .entries
            .iter()
            .map(|entry| {
                json_object(vec![
                    json_entry("peer_id", Value::from(entry.peer_id.to_string())),
                    json_entry(
                        "chunk_digest_mismatch_total",
                        entry.chunk_digest_mismatch_total,
                    ),
                    json_entry(
                        "payload_hash_mismatch_total",
                        entry.payload_hash_mismatch_total,
                    ),
                    json_entry("chunk_root_mismatch_total", entry.chunk_root_mismatch_total),
                    json_entry("last_timestamp_ms", entry.last_timestamp_ms),
                ])
            })
            .collect(),
    );
    let rbc_mismatch = json_object(vec![json_entry("entries", rbc_mismatch_entries)]);
    let pending_rbc_entries = Value::Array(
        snap.pending_rbc
            .entries
            .iter()
            .map(|entry| {
                json_object(vec![
                    json_entry("block_hash", Value::from(format!("{}", entry.block_hash))),
                    json_entry("height", entry.height),
                    json_entry("view", entry.view),
                    json_entry("chunks", entry.chunks),
                    json_entry("bytes", entry.bytes),
                    json_entry("ready", entry.ready),
                    json_entry("deliver", entry.deliver),
                    json_entry("dropped_chunks", entry.dropped_chunks),
                    json_entry("dropped_bytes", entry.dropped_bytes),
                    json_entry("dropped_ready", entry.dropped_ready),
                    json_entry("dropped_deliver", entry.dropped_deliver),
                    json_entry("age_ms", entry.age_ms),
                ])
            })
            .collect(),
    );
    let pending_rbc = json_object(vec![
        json_entry("sessions", snap.pending_rbc.sessions),
        json_entry("session_cap", snap.pending_rbc.session_cap),
        json_entry("chunks", snap.pending_rbc.chunks),
        json_entry("bytes", snap.pending_rbc.bytes),
        json_entry(
            "max_chunks_per_session",
            snap.pending_rbc.max_chunks_per_session,
        ),
        json_entry(
            "max_bytes_per_session",
            snap.pending_rbc.max_bytes_per_session,
        ),
        json_entry("ttl_ms", snap.pending_rbc.ttl_ms),
        json_entry("drops_total", snap.pending_rbc.drops_total),
        json_entry("drops_cap_total", snap.pending_rbc.drops_cap_total),
        json_entry(
            "drops_cap_bytes_total",
            snap.pending_rbc.drops_cap_bytes_total,
        ),
        json_entry("drops_ttl_total", snap.pending_rbc.drops_ttl_total),
        json_entry(
            "drops_ttl_bytes_total",
            snap.pending_rbc.drops_ttl_bytes_total,
        ),
        json_entry("drops_bytes_total", snap.pending_rbc.drops_bytes_total),
        json_entry("evicted_total", snap.pending_rbc.evicted_total),
        json_entry("stash_ready_total", snap.pending_rbc.stash_ready_total),
        json_entry(
            "stash_ready_init_missing_total",
            snap.pending_rbc.stash_ready_init_missing_total,
        ),
        json_entry(
            "stash_ready_roster_missing_total",
            snap.pending_rbc.stash_ready_roster_missing_total,
        ),
        json_entry(
            "stash_ready_roster_hash_mismatch_total",
            snap.pending_rbc.stash_ready_roster_hash_mismatch_total,
        ),
        json_entry(
            "stash_ready_roster_unverified_total",
            snap.pending_rbc.stash_ready_roster_unverified_total,
        ),
        json_entry("stash_deliver_total", snap.pending_rbc.stash_deliver_total),
        json_entry(
            "stash_deliver_init_missing_total",
            snap.pending_rbc.stash_deliver_init_missing_total,
        ),
        json_entry(
            "stash_deliver_roster_missing_total",
            snap.pending_rbc.stash_deliver_roster_missing_total,
        ),
        json_entry(
            "stash_deliver_roster_hash_mismatch_total",
            snap.pending_rbc.stash_deliver_roster_hash_mismatch_total,
        ),
        json_entry(
            "stash_deliver_roster_unverified_total",
            snap.pending_rbc.stash_deliver_roster_unverified_total,
        ),
        json_entry("stash_chunk_total", snap.pending_rbc.stash_chunk_total),
        json_entry("entries", pending_rbc_entries),
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
                json_entry("rbc_chunk_max_bytes", caps.rbc_chunk_max_bytes),
                json_entry("rbc_session_ttl_ms", caps.rbc_session_ttl_ms),
                json_entry("rbc_store_max_sessions", caps.rbc_store_max_sessions),
                json_entry("rbc_store_soft_sessions", caps.rbc_store_soft_sessions),
                json_entry("rbc_store_max_bytes", caps.rbc_store_max_bytes),
                json_entry("rbc_store_soft_bytes", caps.rbc_store_soft_bytes),
            ])
        })
        .unwrap_or(Value::Null);
    let effective_npos_timeouts = snap
        .effective_npos_timeouts
        .as_ref()
        .map(|timeouts| {
            json_object(vec![
                json_entry("propose_ms", timeouts.propose_ms),
                json_entry("prevote_ms", timeouts.prevote_ms),
                json_entry("precommit_ms", timeouts.precommit_ms),
                json_entry("commit_ms", timeouts.commit_ms),
                json_entry("da_ms", timeouts.da_ms),
                json_entry("aggregator_ms", timeouts.aggregator_ms),
                json_entry("exec_ms", timeouts.exec_ms),
                json_entry("witness_ms", timeouts.witness_ms),
            ])
        })
        .unwrap_or(Value::Null);
    let npos_repair_coverage = snap
        .npos_repair_coverage
        .as_ref()
        .map(|coverage| {
            json_object(vec![
                json_entry("last_repair_height", coverage.last_repair_height),
                json_entry("last_repair_view", coverage.last_repair_view),
                json_entry("reason", coverage.reason.clone()),
                json_entry(
                    "selected_repair_peer_count",
                    coverage.selected_repair_peer_count,
                ),
                json_entry(
                    "required_stake_quorum_bps",
                    coverage.required_stake_quorum_bps,
                ),
                json_entry(
                    "selected_stake_coverage_bps",
                    coverage.selected_stake_coverage_bps,
                ),
                json_entry(
                    "reached_stake_quorum_coverage",
                    coverage.reached_stake_quorum_coverage,
                ),
            ])
        })
        .unwrap_or(Value::Null);
    crate::json_object(vec![
        json_entry("canonical", sumeragi_v1_status_json(snap)),
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
        json_entry("effective_min_finality_ms", snap.effective_min_finality_ms),
        json_entry("effective_block_time_ms", snap.effective_block_time_ms),
        json_entry("effective_commit_time_ms", snap.effective_commit_time_ms),
        json_entry(
            "effective_pacing_factor_bps",
            snap.effective_pacing_factor_bps,
        ),
        json_entry(
            "effective_commit_quorum_timeout_ms",
            snap.effective_commit_quorum_timeout_ms,
        ),
        json_entry(
            "effective_availability_timeout_ms",
            snap.effective_availability_timeout_ms,
        ),
        json_entry(
            "effective_pacemaker_interval_ms",
            snap.effective_pacemaker_interval_ms,
        ),
        json_entry("effective_npos_timeouts", effective_npos_timeouts),
        json_entry("npos_repair_coverage", npos_repair_coverage),
        json_entry("effective_collectors_k", snap.effective_collectors_k),
        json_entry(
            "effective_redundant_send_r",
            snap.effective_redundant_send_r,
        ),
        json_entry("leader_index", snap.leader_index),
        json_entry("view_change_index", snap.view_change_index),
        json_entry("view_change_causes", view_change_causes),
        json_entry("highest_qc", highest_qc),
        json_entry("locked_qc", locked_qc),
        json_entry("commit_qc", commit_qc),
        json_entry("commit_quorum", commit_quorum),
        json_entry("tx_queue", tx_queue),
        json_entry("worker_loop", worker_loop),
        json_entry("commit_inflight", commit_inflight),
        json_entry("commit_pipeline", commit_pipeline),
        json_entry("round_gap", round_gap),
        json_entry("missing_block_fetch", missing_block_fetch),
        json_entry(
            "committed_edge_conflict_obsolete_total",
            snap.committed_edge_conflict_obsolete_total,
        ),
        json_entry(
            "roster_sidecar_mismatch_obsolete_total",
            snap.roster_sidecar_mismatch_obsolete_total,
        ),
        json_entry("block_sync", block_sync),
        json_entry("kura_store", kura_store),
        json_entry("epoch", epoch),
        json_entry("gossip_fallback_total", snap.gossip_fallback_total),
        json_entry(
            "gossip_duplicate_known_skipped_total",
            snap.gossip_duplicate_known_skipped_total,
        ),
        json_entry(
            "quorum_stall_age_escalation_total",
            snap.quorum_stall_age_escalation_total,
        ),
        json_entry(
            "retransmit_target_set_last",
            snap.retransmit_target_set_last,
        ),
        json_entry(
            "retransmit_target_set_total",
            snap.retransmit_target_set_total,
        ),
        json_entry(
            "retransmit_target_set_samples",
            snap.retransmit_target_set_samples,
        ),
        json_entry(
            "retransmit_skip_relay_backpressure_total",
            snap.retransmit_skip_relay_backpressure_total,
        ),
        json_entry(
            "retransmit_skip_backlog_pacing_total",
            snap.retransmit_skip_backlog_pacing_total,
        ),
        json_entry(
            "retransmit_skip_no_targets_total",
            snap.retransmit_skip_no_targets_total,
        ),
        json_entry(
            "retransmit_skip_cooldown_total",
            snap.retransmit_skip_cooldown_total,
        ),
        json_entry("dedup_evictions", dedup_evictions),
        json_entry("consensus_message_handling", consensus_message_handling),
        json_entry("vote_validation_drops", vote_validation_drops),
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
        json_entry("proposal_gate", proposal_gate_json(snap.proposal_gate)),
        json_entry(
            "commit_pipeline_tick_total",
            snap.commit_pipeline_tick_total,
        ),
        json_entry("rbc_store", rbc_store),
        json_entry("rbc_mismatch", rbc_mismatch),
        json_entry("pending_rbc", pending_rbc),
        json_entry("qc_rebuild_attempts_total", snap.qc_rebuild_attempts_total),
        json_entry(
            "qc_rebuild_successes_total",
            snap.qc_rebuild_successes_total,
        ),
        json_entry(
            "consensus_missing_block_height_progress_deferred_total",
            snap.consensus_missing_block_height_progress_deferred_total,
        ),
        json_entry(
            "qc_deferred_missing_payload_total",
            snap.qc_deferred_missing_payload_total,
        ),
        json_entry(
            "qc_deferred_resolved_total",
            snap.qc_deferred_resolved_total,
        ),
        json_entry("qc_deferred_expired_total", snap.qc_deferred_expired_total),
        json_entry(
            "consensus_missing_qc_reacquire_attempt_total",
            snap.consensus_missing_qc_reacquire_attempt_total,
        ),
        json_entry(
            "consensus_missing_qc_reacquire_success_total",
            snap.consensus_missing_qc_reacquire_success_total,
        ),
        json_entry(
            "consensus_missing_qc_reacquire_exhausted_total",
            snap.consensus_missing_qc_reacquire_exhausted_total,
        ),
        json_entry(
            "consensus_missing_qc_rotation_deferred_total",
            snap.consensus_missing_qc_rotation_deferred_total,
        ),
        json_entry(
            "consensus_forced_proposal_attempt_total",
            snap.consensus_forced_proposal_attempt_total,
        ),
        json_entry(
            "consensus_forced_proposal_success_total",
            snap.consensus_forced_proposal_success_total,
        ),
        json_entry(
            "consensus_roster_unavailable_detected_total",
            snap.consensus_roster_unavailable_detected_total,
        ),
        json_entry(
            "consensus_roster_unavailable_election_attempt_total",
            snap.consensus_roster_unavailable_election_attempt_total,
        ),
        json_entry(
            "consensus_roster_unavailable_election_success_total",
            snap.consensus_roster_unavailable_election_success_total,
        ),
        json_entry(
            "consensus_roster_unavailable_wait_candidates_total",
            snap.consensus_roster_unavailable_wait_candidates_total,
        ),
        json_entry(
            "consensus_roster_recovery_state",
            snap.consensus_roster_recovery_state
                .map(Value::from)
                .unwrap_or(Value::Null),
        ),
        json_entry(
            "consensus_roster_recovery_dwell_ms",
            json_object(
                snap.consensus_roster_recovery_dwell_ms
                    .iter()
                    .map(|(state, ms)| json_entry(*state, *ms))
                    .collect::<Vec<_>>(),
            ),
        ),
        json_entry(
            "blocksync_range_pull_escalation_total",
            snap.blocksync_range_pull_escalation_total,
        ),
        json_entry(
            "blocksync_range_pull_success_total",
            snap.blocksync_range_pull_success_total,
        ),
        json_entry(
            "blocksync_range_pull_failure_total",
            snap.blocksync_range_pull_failure_total,
        ),
        json_entry(
            "blocksync_range_pull_candidate_exhausted_total",
            snap.blocksync_range_pull_candidate_exhausted_total,
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
        json_entry("lane_commitments", lane_commitments),
        json_entry("lane_settlement_commitments", lane_settlement_commitments),
        json_entry("committed_lane_blocks", committed_lane_blocks),
        json_entry("lane_block_sessions", lane_block_sessions),
        json_entry("dataspace_commitments", dataspace_commitments),
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
        json_entry(
            "pipeline_conflict_rate_bps",
            snap.pipeline_conflict_rate_bps,
        ),
        json_entry("pipeline_execution", pipeline_execution),
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
        block::consensus::{
            CertPhase, LaneBlockCommitment, LaneBlockDescriptorV1, LaneBlockProposalV1,
            LaneBlockQcV1, LaneLiquidityProfile, LaneSettlementReceipt, LaneSwapMetadata,
            LaneVolatilityClass, NativeAmxAttestationBodyV2, NativeAmxAttestationQcV2,
            NativeAmxLegRecordV2, NativeAmxPhase, NativeAmxReceipt,
        },
        consensus::VALIDATOR_SET_HASH_VERSION_V1,
        consensus::{ValidatorElectionOutcome, ValidatorElectionParameters, ValidatorTieBreak},
        nexus::{DataSpaceId, LaneId},
        peer::PeerId,
        transaction::TransactionEntrypoint,
    };
    use iroha_primitives::numeric::Numeric;

    fn checked_status_peer(seed: u8, context: &'static str) -> PeerId {
        PeerId::new(
            super::super::checked_routing_fixture_keypair(seed, Algorithm::Ed25519, context)
                .public_key()
                .clone(),
        )
    }

    fn committed_lane_block_status_fixture() -> status::CommittedLaneBlockSnapshot {
        let validator = checked_status_peer(91, "committed-lane-block-status");
        let validator_set = vec![validator];
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(11),
            lane_incarnation: Hash::new(b"torii-consensus-lane-incarnation"),
            proposal_height: 13,
            previous_lane_block_height: 12,
            previous_lane_block_descriptor_hash: Some(Hash::prehashed([0x61; Hash::LENGTH])),
            lane_block_height: 13,
            lane_block_view: 2,
            subject_hash: Hash::prehashed([0x62; Hash::LENGTH]),
            payload_ownership_hash: Hash::prehashed([0x63; Hash::LENGTH]),
            rbc_instance_hash: Hash::prehashed([0x64; Hash::LENGTH]),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::prehashed([0x65; Hash::LENGTH])],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set: validator_set.clone(),
            validator_count: 1,
            min_quorum: 1,
            qc_mode_tag: "permissioned:lane:7:dataspace:11".to_string(),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        proposal.proposal_hash = proposal.computed_proposal_hash();
        let prepare_qc = LaneBlockQcV1 {
            body: proposal.vote_body(CertPhase::Prepare),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set: validator_set.clone(),
            signers_bitmap: vec![0b0000_0001],
            bls_aggregate_signature: vec![0xA1],
            payload_availability_qc: None,
        };
        let commit_qc = LaneBlockQcV1 {
            body: proposal.vote_body(CertPhase::Commit),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set,
            signers_bitmap: vec![0b0000_0001],
            bls_aggregate_signature: vec![0xA2],
            payload_availability_qc: None,
        };
        status::CommittedLaneBlockSnapshot {
            lane_id: proposal.descriptor.lane_id,
            dataspace_id: proposal.descriptor.dataspace_id,
            lane_block_height: proposal.descriptor.lane_block_height,
            lane_block_view: proposal.descriptor.lane_block_view,
            descriptor_hash: proposal.descriptor.descriptor_hash,
            proposal_hash: proposal.proposal_hash,
            execution_status: status::CommittedLaneBlockExecutionStatus::AwaitingExecutablePayload,
            proposal,
            prepare_qc,
            commit_qc,
        }
    }

    fn authoritative_v2_status_fixture() -> iroha_data_model::block::consensus_v2::SumeragiV2Status
    {
        use iroha_data_model::block::consensus_v2 as wire;

        wire::SumeragiV2Status {
            protocol_version: wire::PROTOCOL_VERSION,
            node_fingerprint: Hash::new(b"torii-v2-status-node"),
            build_fingerprint: Hash::new(b"torii-v2-status-build"),
            config_fingerprint: Hash::new(b"torii-v2-status-config"),
            height_context_id: wire::HeightContextId(
                HashOf::<wire::HeightContext>::from_untyped_unchecked(Hash::new(
                    b"torii-v2-status-context",
                )),
            ),
            height: 7,
            view: 2,
            phase: wire::SumeragiV2StatusPhase::Prepare,
            leader: 1,
            locked_prepare_qc: None,
            highest_prepare_qc: None,
            last_timeout_certificate: None,
            body_state: wire::SumeragiV2BodyState::Validated,
            pending_persistence_id: None,
            last_committed_height: 0,
            last_committed_subject: None,
            height_context: wire::SumeragiV2HeightContextStatus {
                epoch: 1,
                epoch_end_height: 10,
                mode: wire::ConsensusMode::Permissioned,
                epoch_seed: [0xA5; 32],
                validator_count: 4,
                quorum: wire::DualQuorum {
                    min_signers: 3,
                    total_power: 4,
                },
            },
            last_commit_qc: None,
        }
    }

    fn lane_payload_ownership_status_fixture()
    -> iroha_data_model::block::consensus::SumeragiLanePayloadOwnership {
        iroha_data_model::block::consensus::SumeragiLanePayloadOwnership {
            proposal_height: 13,
            proposal_view: 2,
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(11),
            lane_incarnation: Hash::new(b"torii-v2-status-lane-incarnation"),
            lane_block_height: 13,
            lane_block_view: 2,
            subject_hash: Hash::prehashed([0x71; Hash::LENGTH]),
            qc_mode_tag: "permissioned:lane:7:dataspace:11".to_owned(),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::prehashed([0x72; Hash::LENGTH])],
            previous_lane_block_height: 12,
            previous_lane_block_descriptor_hash: Some(Hash::prehashed([0x73; Hash::LENGTH])),
            lane_block_descriptor_hash: Some(Hash::prehashed([0x74; Hash::LENGTH])),
            lane_block_descriptor_validator_set: Vec::new(),
            lane_block_descriptor_validator_count: 0,
            lane_block_descriptor_min_quorum: 0,
            payload_ownership_hash: Hash::prehashed([0x75; Hash::LENGTH]),
            rbc_instance_hash: Hash::prehashed([0x76; Hash::LENGTH]),
        }
    }

    fn lane_block_session_status_fixture()
    -> iroha_data_model::block::consensus::SumeragiLaneBlockSessionStatus {
        iroha_data_model::block::consensus::SumeragiLaneBlockSessionStatus {
            lane_id: LaneId::new(7),
            dataspace_id: DataSpaceId::new(11),
            lane_incarnation: Hash::new(b"torii-v2-status-lane-incarnation"),
            lane_block_height: 13,
            lane_block_view: 2,
            proposal_hash: Hash::prehashed([0x77; Hash::LENGTH]),
            has_proposal: true,
            prepare_vote_count: 3,
            commit_vote_count: 2,
            has_prepare_qc: true,
            has_commit_qc: false,
            pending_commit_vote_request: true,
            pending_committed_session_drain: false,
            committed_session_drained: false,
            validator_count: 4,
            min_quorum: 3,
        }
    }

    #[test]
    fn v2_status_json_preserves_lane_observability_and_strips_when_nexus_disabled() {
        let ownership = lane_payload_ownership_status_fixture();
        let committed = committed_lane_block_status_fixture();
        let committed_wire = committed_lane_block_wire(&committed);
        let session = lane_block_session_status_fixture();
        let snapshot = sumeragi::StatusSnapshot {
            lane_payload_ownerships: vec![ownership.clone()],
            committed_lane_blocks: vec![committed],
            lane_block_sessions: vec![session],
            ..Default::default()
        };

        let enabled = sumeragi_v2_status_json_from_snapshot(
            authoritative_v2_status_fixture(),
            snapshot.clone(),
            true,
            true,
        );
        assert_eq!(enabled.lane_payload_ownerships, vec![ownership]);
        assert_eq!(enabled.committed_lane_blocks, vec![committed_wire]);
        assert_eq!(enabled.lane_block_sessions, vec![session]);
        assert!(enabled.local_peer_removed);

        let enabled_json =
            norito::json::to_value(&enabled).expect("serialize authoritative v2 status JSON");
        let enabled_object = enabled_json
            .as_object()
            .expect("authoritative v2 status JSON object");
        for key in [
            "lane_payload_ownerships",
            "committed_lane_blocks",
            "lane_block_sessions",
        ] {
            assert_eq!(
                enabled_object
                    .get(key)
                    .and_then(Value::as_array)
                    .map(Vec::len),
                Some(1),
                "missing v2 status observability field {key}"
            );
        }
        assert_eq!(
            enabled_object
                .get("local_peer_removed")
                .and_then(Value::as_bool),
            Some(true)
        );

        let disabled = sumeragi_v2_status_json_from_snapshot(
            authoritative_v2_status_fixture(),
            snapshot,
            false,
            true,
        );
        assert!(disabled.lane_payload_ownerships.is_empty());
        assert!(disabled.committed_lane_blocks.is_empty());
        assert!(disabled.lane_block_sessions.is_empty());
        assert!(disabled.local_peer_removed);

        let disabled_json =
            norito::json::to_value(&disabled).expect("serialize stripped v2 status JSON");
        let disabled_object = disabled_json
            .as_object()
            .expect("stripped v2 status JSON object");
        for key in [
            "lane_payload_ownerships",
            "committed_lane_blocks",
            "lane_block_sessions",
        ] {
            assert!(
                disabled_object
                    .get(key)
                    .and_then(Value::as_array)
                    .is_some_and(Vec::is_empty),
                "Nexus-disabled v2 status leaked {key}"
            );
        }
        assert_eq!(
            disabled_object
                .get("local_peer_removed")
                .and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn v2_status_json_projects_settlement_u128_values_as_decimal_strings_everywhere() {
        let receipt = LaneSettlementReceipt {
            source_id: [0xAB; 32],
            local_amount_micro: u128::MAX,
            xor_due_micro: u128::MAX - 1,
            xor_after_haircut_micro: u128::MAX - 2,
            xor_variance_micro: u128::MAX - 3,
            timestamp_ms: 42,
        };
        let commitment = LaneBlockCommitment {
            block_height: 1,
            lane_id: LaneId::new(3),
            lane_incarnation: Hash::new(b"v2-status-u128-lane-incarnation"),
            dataspace_id: DataSpaceId::new(9),
            tx_count: 1,
            total_local_micro: u128::MAX,
            total_xor_due_micro: u128::MAX - 1,
            total_xor_after_haircut_micro: u128::MAX - 2,
            total_xor_variance_micro: u128::MAX - 3,
            swap_metadata: None,
            receipts: vec![receipt],
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        };
        let header = iroha_data_model::block::BlockHeader::new(
            core::num::NonZeroU64::new(1).expect("nonzero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let relay = iroha_data_model::nexus::LaneRelayEnvelope::new(
            header,
            None,
            None,
            commitment.clone(),
            0,
        )
        .expect("construct relay fixture");
        let snapshot = sumeragi::StatusSnapshot {
            lane_settlement_commitments: vec![commitment],
            lane_relay_envelopes: vec![relay],
            ..Default::default()
        };

        let payload = norito::json::to_value(&sumeragi_v2_status_json_from_snapshot(
            authoritative_v2_status_fixture(),
            snapshot,
            true,
            false,
        ))
        .expect("serialize v2 status projection");
        let object = payload.as_object().expect("v2 status object");
        let top_level = object["lane_settlement_commitments"]
            .as_array()
            .and_then(|entries| entries.first())
            .and_then(Value::as_object)
            .expect("top-level settlement commitment");
        assert_eq!(
            top_level.get("total_local_micro").and_then(Value::as_str),
            Some(u128::MAX.to_string().as_str())
        );
        let receipt = top_level["receipts"]
            .as_array()
            .and_then(|entries| entries.first())
            .and_then(Value::as_object)
            .expect("top-level settlement receipt");
        assert_eq!(
            receipt.get("local_amount_micro").and_then(Value::as_str),
            Some(u128::MAX.to_string().as_str())
        );
        let embedded = object["lane_relay_envelopes"]
            .as_array()
            .and_then(|entries| entries.first())
            .and_then(Value::as_object)
            .and_then(|relay| relay.get("settlement_commitment"))
            .and_then(Value::as_object)
            .expect("relay settlement commitment");
        assert_eq!(
            embedded.get("total_local_micro").and_then(Value::as_str),
            Some(u128::MAX.to_string().as_str())
        );
        let embedded_receipt = embedded["receipts"]
            .as_array()
            .and_then(|entries| entries.first())
            .and_then(Value::as_object)
            .expect("relay settlement receipt");
        assert_eq!(
            embedded_receipt
                .get("local_amount_micro")
                .and_then(Value::as_str),
            Some(u128::MAX.to_string().as_str())
        );
    }

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
            effective_min_finality_ms: 150,
            effective_block_time_ms: 1_000,
            effective_commit_time_ms: 1_500,
            effective_pacing_factor_bps: 12_500,
            effective_commit_quorum_timeout_ms: 3_000,
            effective_availability_timeout_ms: 2_500,
            effective_pacemaker_interval_ms: 750,
            effective_npos_timeouts: Some(sumeragi::status::NposTimeoutsSnapshot {
                propose_ms: 200,
                prevote_ms: 210,
                precommit_ms: 220,
                commit_ms: 230,
                da_ms: 240,
                aggregator_ms: 250,
                exec_ms: 260,
                witness_ms: 270,
            }),
            effective_collectors_k: 4,
            effective_redundant_send_r: 2,
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
                .get("effective_min_finality_ms")
                .and_then(Value::as_u64)
                .unwrap(),
            150
        );
        assert_eq!(
            payload
                .get("effective_block_time_ms")
                .and_then(Value::as_u64)
                .unwrap(),
            1_000
        );
        assert_eq!(
            payload
                .get("effective_commit_time_ms")
                .and_then(Value::as_u64)
                .unwrap(),
            1_500
        );
        let npos_timeouts = payload
            .get("effective_npos_timeouts")
            .and_then(Value::as_object)
            .expect("effective_npos_timeouts object");
        assert_eq!(
            npos_timeouts.get("propose_ms").and_then(Value::as_u64),
            Some(200)
        );
        assert_eq!(
            npos_timeouts.get("witness_ms").and_then(Value::as_u64),
            Some(270)
        );
        assert_eq!(
            payload
                .get("effective_collectors_k")
                .and_then(Value::as_u64)
                .unwrap(),
            4
        );
        assert_eq!(
            payload
                .get("effective_redundant_send_r")
                .and_then(Value::as_u64)
                .unwrap(),
            2
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
    fn status_snapshot_json_serializes_lane_settlement_commitments() {
        let receipt = LaneSettlementReceipt {
            source_id: [0xAB; 32],
            local_amount_micro: 1_500u128,
            xor_due_micro: 75_000u128,
            xor_after_haircut_micro: 70_000u128,
            xor_variance_micro: 5_000u128,
            timestamp_ms: 1_700_000_000_000,
        };
        let commitment = LaneBlockCommitment {
            block_height: 42,
            lane_id: LaneId::new(3),
            lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
            dataspace_id: DataSpaceId::new(9),
            tx_count: 2,
            total_local_micro: 1_500u128,
            total_xor_due_micro: 75_000u128,
            total_xor_after_haircut_micro: 70_000u128,
            total_xor_variance_micro: 5_000u128,
            swap_metadata: Some(LaneSwapMetadata {
                epsilon_bps: 25,
                twap_window_seconds: 60,
                liquidity_profile: LaneLiquidityProfile::Tier2,
                twap_local_per_xor: "12.5".to_owned(),
                volatility_class: LaneVolatilityClass::Stable,
            }),
            receipts: vec![receipt.clone()],
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        };
        let snap = sumeragi::StatusSnapshot {
            lane_settlement_commitments: vec![commitment.clone()],
            ..Default::default()
        };

        let payload = status_snapshot_json(&snap);
        let entries = payload
            .get("lane_settlement_commitments")
            .and_then(Value::as_array)
            .expect("lane settlement commitments array");
        assert_eq!(entries.len(), 1);
        let entry = entries[0]
            .as_object()
            .expect("lane settlement commitment object");
        assert_eq!(
            entry.get("block_height").and_then(Value::as_u64),
            Some(commitment.block_height)
        );
        assert_eq!(entry.get("lane_id").and_then(Value::as_u64), Some(3));
        assert_eq!(entry.get("dataspace_id").and_then(Value::as_u64), Some(9));
        assert_eq!(
            entry
                .get("total_xor_variance_micro")
                .and_then(Value::as_str),
            Some(commitment.total_xor_variance_micro.to_string().as_str())
        );

        let metadata = entry
            .get("swap_metadata")
            .and_then(Value::as_object)
            .expect("swap metadata object");
        assert_eq!(
            metadata.get("epsilon_bps").and_then(Value::as_u64),
            Some(25)
        );
        assert_eq!(
            metadata.get("liquidity_profile").and_then(Value::as_str),
            Some("Tier2")
        );

        let receipts = entry
            .get("receipts")
            .and_then(Value::as_array)
            .expect("receipts array");
        assert_eq!(receipts.len(), 1);
        let receipt_entry = receipts[0].as_object().expect("receipt object");
        assert_eq!(
            receipt_entry.get("source_id").and_then(Value::as_str),
            Some(hex::encode(receipt.source_id).as_str())
        );
        assert_eq!(
            receipt_entry
                .get("xor_after_haircut_micro")
                .and_then(Value::as_str),
            Some(receipt.xor_after_haircut_micro.to_string().as_str())
        );
        assert!(
            entry
                .get("nexus_fee_receipts")
                .and_then(Value::as_array)
                .expect("nexus fee receipts array")
                .is_empty()
        );
        assert!(
            entry
                .get("native_amx_receipts")
                .and_then(Value::as_array)
                .expect("native AMX receipts array")
                .is_empty()
        );
    }

    #[test]
    fn status_snapshot_json_serializes_committed_lane_blocks() {
        let committed = committed_lane_block_status_fixture();
        let mut payload_available = committed.clone();
        payload_available.execution_status =
            status::CommittedLaneBlockExecutionStatus::PayloadAvailableAwaitingExecutor;
        let mut payload_recovered = committed.clone();
        payload_recovered.execution_status =
            status::CommittedLaneBlockExecutionStatus::PayloadRecoveredAwaitingStateApplication;
        let mut payload_preflighted = committed.clone();
        payload_preflighted.execution_status =
            status::CommittedLaneBlockExecutionStatus::PayloadPreflightedAwaitingStateApplication;
        let mut payload_preflight_rejected = committed.clone();
        payload_preflight_rejected.execution_status = status::CommittedLaneBlockExecutionStatus::PayloadPreflightRejectedAwaitingStateApplication;
        let mut receipt_conflict = committed.clone();
        receipt_conflict.execution_status =
            status::CommittedLaneBlockExecutionStatus::ApplicationReceiptConflictsWithPreflight;
        let mut predecessor_blocked = committed.clone();
        predecessor_blocked.execution_status =
            status::CommittedLaneBlockExecutionStatus::AwaitingPredecessorApplication;
        let mut state_applied = committed.clone();
        state_applied.execution_status =
            status::CommittedLaneBlockExecutionStatus::StateAppliedByCanonicalBlock;
        let mut direct_applied = committed.clone();
        direct_applied.execution_status =
            status::CommittedLaneBlockExecutionStatus::StateAppliedByDirectExecution;
        let snap = sumeragi::StatusSnapshot {
            committed_lane_blocks: vec![
                committed.clone(),
                payload_available.clone(),
                payload_recovered.clone(),
                payload_preflighted.clone(),
                payload_preflight_rejected.clone(),
                receipt_conflict.clone(),
                predecessor_blocked.clone(),
                state_applied.clone(),
                direct_applied.clone(),
            ],
            ..Default::default()
        };

        let payload = status_snapshot_json(&snap);
        let entries = payload
            .get("committed_lane_blocks")
            .and_then(Value::as_array)
            .expect("committed lane block array");
        assert_eq!(entries.len(), 9);
        let entry = entries[0].as_object().expect("committed lane block object");
        assert_eq!(entry.get("lane_id").and_then(Value::as_u64), Some(7));
        assert_eq!(entry.get("dataspace_id").and_then(Value::as_u64), Some(11));
        assert_eq!(
            entry.get("lane_block_height").and_then(Value::as_u64),
            Some(committed.lane_block_height)
        );
        assert_eq!(
            entry.get("descriptor_hash").and_then(Value::as_str),
            Some(hash_with_prefix(committed.descriptor_hash).as_str())
        );
        assert_eq!(
            entry.get("proposal_hash").and_then(Value::as_str),
            Some(hash_with_prefix(committed.proposal_hash).as_str())
        );
        assert_eq!(
            entry.get("execution_status").and_then(Value::as_str),
            Some(committed.execution_status.as_str())
        );
        assert_eq!(
            entry
                .get("executable_payload_available")
                .and_then(Value::as_bool),
            Some(false)
        );
        let available_entry = entries[1]
            .as_object()
            .expect("payload-available committed lane block object");
        assert_eq!(
            available_entry
                .get("execution_status")
                .and_then(Value::as_str),
            Some(payload_available.execution_status.as_str())
        );
        assert_eq!(
            available_entry
                .get("executable_payload_available")
                .and_then(Value::as_bool),
            Some(true)
        );
        let recovered_entry = entries[2]
            .as_object()
            .expect("payload-recovered committed lane block object");
        assert_eq!(
            recovered_entry
                .get("execution_status")
                .and_then(Value::as_str),
            Some(payload_recovered.execution_status.as_str())
        );
        assert_eq!(
            recovered_entry
                .get("executable_payload_available")
                .and_then(Value::as_bool),
            Some(true)
        );
        let preflighted_entry = entries[3]
            .as_object()
            .expect("preflighted committed lane block object");
        assert_eq!(
            preflighted_entry
                .get("execution_status")
                .and_then(Value::as_str),
            Some(payload_preflighted.execution_status.as_str())
        );
        assert_eq!(
            preflighted_entry
                .get("executable_payload_available")
                .and_then(Value::as_bool),
            Some(true)
        );
        let preflight_rejected_entry = entries[4]
            .as_object()
            .expect("preflight-rejected committed lane block object");
        assert_eq!(
            preflight_rejected_entry
                .get("execution_status")
                .and_then(Value::as_str),
            Some(payload_preflight_rejected.execution_status.as_str())
        );
        assert_eq!(
            preflight_rejected_entry
                .get("executable_payload_available")
                .and_then(Value::as_bool),
            Some(false)
        );
        let receipt_conflict_entry = entries[5]
            .as_object()
            .expect("receipt-conflict committed lane block object");
        assert_eq!(
            receipt_conflict_entry
                .get("execution_status")
                .and_then(Value::as_str),
            Some(receipt_conflict.execution_status.as_str())
        );
        assert_eq!(
            receipt_conflict_entry
                .get("executable_payload_available")
                .and_then(Value::as_bool),
            Some(false)
        );
        let predecessor_blocked_entry = entries[6]
            .as_object()
            .expect("predecessor-blocked committed lane block object");
        assert_eq!(
            predecessor_blocked_entry
                .get("execution_status")
                .and_then(Value::as_str),
            Some(predecessor_blocked.execution_status.as_str())
        );
        assert_eq!(
            predecessor_blocked_entry
                .get("executable_payload_available")
                .and_then(Value::as_bool),
            Some(false)
        );
        let applied_entry = entries[7]
            .as_object()
            .expect("state-applied committed lane block object");
        assert_eq!(
            applied_entry
                .get("execution_status")
                .and_then(Value::as_str),
            Some(state_applied.execution_status.as_str())
        );
        assert_eq!(
            applied_entry
                .get("executable_payload_available")
                .and_then(Value::as_bool),
            Some(true)
        );
        let direct_applied_entry = entries[8]
            .as_object()
            .expect("direct-applied committed lane block object");
        assert_eq!(
            direct_applied_entry
                .get("execution_status")
                .and_then(Value::as_str),
            Some(direct_applied.execution_status.as_str())
        );
        assert_eq!(
            direct_applied_entry
                .get("executable_payload_available")
                .and_then(Value::as_bool),
            Some(true)
        );
        let prepare_qc = entry
            .get("prepare_qc")
            .and_then(Value::as_object)
            .expect("prepare QC summary");
        let commit_qc = entry
            .get("commit_qc")
            .and_then(Value::as_object)
            .expect("commit QC summary");
        assert_eq!(
            prepare_qc.get("phase").and_then(Value::as_str),
            Some("prepare")
        );
        assert_eq!(
            commit_qc.get("phase").and_then(Value::as_str),
            Some("commit")
        );
        assert_eq!(
            prepare_qc.get("signer_count").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            commit_qc.get("signer_count").and_then(Value::as_u64),
            Some(1)
        );
    }

    #[test]
    fn status_snapshot_json_serializes_native_amx_receipts_in_lane_settlement_commitments() {
        let source_id = [0xCE; 32];
        let plan_digest = Hash::new(b"consensus-status-native-amx-plan");
        let tx_entrypoint_hash =
            HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::prehashed(source_id));
        let coordinator_lane_id = LaneId::new(4);
        let coordinator_dataspace_id = DataSpaceId::new(11);
        let participant_lane_id = LaneId::new(5);
        let participant_dataspace_id = DataSpaceId::new(12);
        let chain_id_hash = Hash::new(b"consensus-status-native-amx-chain");
        let coordinator_lane_incarnation =
            Hash::new(b"consensus-status-native-amx-coordinator-incarnation");
        let participant_lane_incarnation =
            Hash::new(b"consensus-status-native-amx-participant-incarnation");
        let coordinator_proposal_hash =
            Hash::new(b"consensus-status-native-amx-coordinator-proposal");
        let validators = vec![
            checked_status_peer(0xA1, "derive native AMX status fixture peer key 1"),
            checked_status_peer(0xA2, "derive native AMX status fixture peer key 2"),
        ];
        let validator_set_hash = HashOf::new(&validators);
        let validator_count = u32::try_from(validators.len()).expect("fixture validator count");
        let participant_min_quorum = u32::try_from(validators.len().saturating_mul(2) / 3 + 1)
            .expect("fixture validator quorum");
        let participant_previous_block_height = 76;
        let participant_previous_block_descriptor_hash =
            Some(Hash::new(b"consensus-status-native-amx-participant-parent"));
        let native_amx_qc = |phase: NativeAmxPhase| {
            let mut body = NativeAmxAttestationBodyV2 {
                round: iroha_data_model::block::consensus_v2::ConsensusRound {
                    context_id: iroha_data_model::block::consensus_v2::HeightContextId(
                        HashOf::<
                            iroha_data_model::block::consensus_v2::HeightContext,
                        >::from_untyped_unchecked(Hash::new(
                            b"torii-consensus-native-amx-context",
                        )),
                    ),
                    height: 70,
                    view: 3,
                },
                epoch: 7,
                chain_id_hash,
                source_id,
                tx_entrypoint_hash,
                plan_digest,
                phase,
                coordinator_lane_id,
                coordinator_dataspace_id,
                coordinator_lane_incarnation,
                participant_lane_id,
                participant_dataspace_id,
                participant_lane_incarnation,
                participant_previous_block_height,
                participant_previous_block_descriptor_hash,
                participant_lane_block_height: 77,
                participant_lane_block_view: 0,
                participant_proposal_hash: Hash::new(
                    b"consensus-status-native-amx-participant-proposal",
                ),
                participant_settlement_commitment: Hash::prehashed([0; Hash::LENGTH]),
                participant_validator_set_hash: validator_set_hash,
                participant_validator_count: validator_count,
                participant_min_quorum,
                authority_context_height: 70,
                planned_coordinator_block_height: 77,
                coordinator_lane_block_view: 3,
                coordinator_proposal_hash,
            };
            body.participant_settlement_commitment =
                body.computed_participant_settlement_commitment();
            NativeAmxAttestationQcV2 {
                body,
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash,
                validator_set: validators.clone(),
                validator_set_pops: vec![vec![0x5A; 96]; validators.len()],
                signers_bitmap: vec![0b0000_0011],
                bls_aggregate_signature: vec![0xA5; 96],
            }
        };
        let mut prepare_qc = native_amx_qc(NativeAmxPhase::Prepare);
        let mut commit_qc = native_amx_qc(NativeAmxPhase::Commit);
        let body = prepare_qc.body;
        let mut descriptor = LaneBlockDescriptorV1 {
            lane_id: body.participant_lane_id,
            dataspace_id: body.participant_dataspace_id,
            lane_incarnation: body.participant_lane_incarnation,
            proposal_height: body.authority_context_height,
            previous_lane_block_height: body.participant_previous_block_height,
            previous_lane_block_descriptor_hash: body.participant_previous_block_descriptor_hash,
            lane_block_height: body.participant_lane_block_height,
            lane_block_view: body.participant_lane_block_view,
            subject_hash: Hash::new(b"consensus-status-native-amx-participant-subject"),
            payload_ownership_hash: Hash::new(b"consensus-status-native-amx-participant-ownership"),
            rbc_instance_hash: Hash::new(b"consensus-status-native-amx-participant-rbc"),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::from(body.tx_entrypoint_hash)],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: body.participant_validator_set_hash,
            validator_set: validators.clone(),
            validator_count: body.participant_validator_count,
            min_quorum: body.participant_min_quorum,
            qc_mode_tag: "permissioned:native-amx-consensus-status".to_owned(),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut participant_proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        participant_proposal.proposal_hash = participant_proposal.computed_proposal_hash();
        prepare_qc.body.participant_proposal_hash = participant_proposal.proposal_hash;
        commit_qc.body.participant_proposal_hash = participant_proposal.proposal_hash;
        let participant_settlement = prepare_qc.body.computed_participant_settlement();
        let participant_settlement_hash =
            iroha_data_model::nexus::compute_settlement_hash(&participant_settlement)
                .expect("fixture participant settlement hashes");
        let receipt = NativeAmxReceipt {
            version: 2,
            source_id,
            chain_id_hash,
            plan_digest,
            lane_id: coordinator_lane_id,
            dataspace_id: coordinator_dataspace_id,
            lane_incarnation: coordinator_lane_incarnation,
            authority_context_height: 70,
            lane_block_height: 77,
            lane_block_view: 3,
            coordinator_proposal_hash,
            legs: vec![NativeAmxLegRecordV2 {
                lane_id: participant_lane_id,
                dataspace_id: participant_dataspace_id,
                participant_proposal,
                participant_settlement,
                participant_settlement_hash,
                prepare_qc,
                commit_qc,
            }],
        };
        let commitment = LaneBlockCommitment {
            block_height: 77,
            lane_id: coordinator_lane_id,
            lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
            dataspace_id: coordinator_dataspace_id,
            tx_count: 1,
            total_local_micro: 0,
            total_xor_due_micro: 0,
            total_xor_after_haircut_micro: 0,
            total_xor_variance_micro: 0,
            swap_metadata: None,
            receipts: Vec::new(),
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: vec![receipt],
        };
        let snap = sumeragi::StatusSnapshot {
            lane_settlement_commitments: vec![commitment],
            ..Default::default()
        };

        let payload = status_snapshot_json(&snap);
        let native = payload
            .get("lane_settlement_commitments")
            .and_then(Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(Value::as_object)
            .and_then(|entry| entry.get("native_amx_receipts"))
            .and_then(Value::as_array)
            .and_then(|receipts| receipts.first())
            .and_then(Value::as_object)
            .expect("native AMX receipt object");
        assert_eq!(native.get("version").and_then(Value::as_u64), Some(2));
        assert_eq!(
            native.get("source_id").and_then(Value::as_str),
            Some(hex::encode(source_id).as_str())
        );
        assert_eq!(
            native.get("plan_digest").and_then(Value::as_str),
            Some(hash_with_prefix(plan_digest).as_str())
        );
        assert_eq!(
            native.get("lane_id").and_then(Value::as_u64),
            Some(u64::from(coordinator_lane_id))
        );
        assert_eq!(
            native.get("dataspace_id").and_then(Value::as_u64),
            Some(u64::from(coordinator_dataspace_id))
        );

        let leg = native
            .get("legs")
            .and_then(Value::as_array)
            .and_then(|legs| legs.first())
            .and_then(Value::as_object)
            .expect("native AMX leg object");
        assert_eq!(
            leg.get("lane_id").and_then(Value::as_u64),
            Some(u64::from(participant_lane_id))
        );
        let prepare_qc = leg
            .get("prepare_qc")
            .and_then(Value::as_object)
            .expect("prepare QC object");
        let prepare_body = prepare_qc
            .get("body")
            .and_then(Value::as_object)
            .expect("prepare body object");
        let prepare_phase = prepare_body
            .get("phase")
            .and_then(Value::as_object)
            .expect("tagged prepare phase object");
        assert_eq!(prepare_phase.len(), 2);
        assert_eq!(
            prepare_phase.get("phase").and_then(Value::as_str),
            Some("prepare")
        );
        assert!(prepare_phase.get("detail").is_some_and(Value::is_null));
        assert_eq!(prepare_body.get("epoch").and_then(Value::as_u64), Some(7));
        assert_eq!(
            prepare_body
                .get("round")
                .and_then(Value::as_object)
                .and_then(|round| round.get("view"))
                .and_then(Value::as_u64),
            Some(3)
        );
        assert_eq!(
            prepare_body
                .get("tx_entrypoint_hash")
                .and_then(Value::as_str),
            Some(hash_with_prefix(tx_entrypoint_hash).as_str())
        );
        assert_eq!(
            prepare_qc.get("validator_set_hash").and_then(Value::as_str),
            Some(hash_with_prefix(validator_set_hash).as_str())
        );
        assert_eq!(
            prepare_qc
                .get("signers_bitmap")
                .and_then(Value::as_array)
                .and_then(|values| values.first())
                .and_then(Value::as_u64),
            Some(0b0000_0011)
        );
        let commit_phase = leg
            .get("commit_qc")
            .and_then(Value::as_object)
            .and_then(|qc| qc.get("body"))
            .and_then(Value::as_object)
            .and_then(|body| body.get("phase"))
            .and_then(Value::as_object)
            .expect("tagged commit phase object");
        assert_eq!(commit_phase.len(), 2);
        assert_eq!(
            commit_phase.get("phase").and_then(Value::as_str),
            Some("commit")
        );
        assert!(commit_phase.get("detail").is_some_and(Value::is_null));
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
            sponsor_unauthorized_total: 1,
            sponsor_cap_exceeded_total: 1,
            config_errors_total: 1,
            transfer_failures_total: 1,
            last_amount: Some(Numeric::from(42_u32)),
            last_asset_id: Some("61CtjvNd9T3THAR65GsMVHr82Bjc".to_owned()),
            last_payer: Some(sumeragi::status::NexusFeePayer::Sponsor),
            last_payer_id: Some("sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76".to_owned()),
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
            fee_json
                .get("sponsor_unauthorized_total")
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
            rbc_store_persist_drops_total: 7,
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
        assert_eq!(
            rbc.get("persist_drops_total").and_then(Value::as_u64),
            Some(7)
        );
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
    fn status_snapshot_json_includes_rbc_mismatch() {
        let peer_pk = PublicKey::from_hex(
            Algorithm::Ed25519,
            "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4",
        )
        .expect("peer pk parses");
        let peer = PeerId::from(peer_pk);
        let peer_id = peer.to_string();
        let entry = status::RbcMismatchEntry {
            peer_id: peer.clone(),
            chunk_digest_mismatch_total: 3,
            payload_hash_mismatch_total: 2,
            chunk_root_mismatch_total: 1,
            last_timestamp_ms: 1_724_000_000_123,
        };
        let snap = sumeragi::StatusSnapshot {
            rbc_mismatch: status::RbcMismatchSnapshot {
                entries: vec![entry],
            },
            ..Default::default()
        };
        let payload = status_snapshot_json(&snap);
        let mismatch = payload
            .get("rbc_mismatch")
            .and_then(Value::as_object)
            .expect("rbc_mismatch object");
        let entries = mismatch
            .get("entries")
            .and_then(Value::as_array)
            .expect("rbc_mismatch entries");
        assert_eq!(entries.len(), 1);
        let entry = entries[0].as_object().expect("rbc_mismatch entry object");
        assert_eq!(
            entry.get("peer_id").and_then(Value::as_str),
            Some(peer_id.as_str())
        );
        assert_eq!(
            entry
                .get("chunk_digest_mismatch_total")
                .and_then(Value::as_u64),
            Some(3)
        );
        assert_eq!(
            entry
                .get("payload_hash_mismatch_total")
                .and_then(Value::as_u64),
            Some(2)
        );
        assert_eq!(
            entry
                .get("chunk_root_mismatch_total")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            entry.get("last_timestamp_ms").and_then(Value::as_u64),
            Some(1_724_000_000_123)
        );
    }

    #[test]
    fn status_snapshot_json_includes_pending_rbc_stash_counters() {
        let hash = Hash::prehashed([0x11; Hash::LENGTH]);
        let hash_typed = HashOf::from_untyped_unchecked(hash);
        let hash_str = format!("{hash_typed}");
        let mut entry = status::PendingRbcEntrySnapshot::default();
        entry.block_hash = hash_typed;
        entry.height = 9;
        entry.view = 1;
        entry.ready = 2;
        entry.deliver = 1;
        entry.age_ms = 42;

        let pending_rbc = status::PendingRbcSnapshot {
            sessions: 1,
            session_cap: 8,
            chunks: 3,
            bytes: 1024,
            max_chunks_per_session: 10,
            max_bytes_per_session: 2048,
            ttl_ms: 1000,
            drops_total: 1,
            drops_cap_total: 1,
            drops_cap_bytes_total: 12,
            drops_ttl_total: 0,
            drops_ttl_bytes_total: 0,
            drops_bytes_total: 12,
            evicted_total: 0,
            stash_ready_total: 2,
            stash_ready_init_missing_total: 1,
            stash_ready_roster_missing_total: 0,
            stash_ready_roster_hash_mismatch_total: 0,
            stash_ready_roster_unverified_total: 1,
            stash_deliver_total: 1,
            stash_deliver_init_missing_total: 1,
            stash_deliver_roster_missing_total: 0,
            stash_deliver_roster_hash_mismatch_total: 0,
            stash_deliver_roster_unverified_total: 0,
            stash_chunk_total: 3,
            entries: vec![entry],
        };
        let snap = sumeragi::StatusSnapshot {
            pending_rbc,
            ..Default::default()
        };
        let payload = status_snapshot_json(&snap);
        let pending = payload
            .get("pending_rbc")
            .and_then(Value::as_object)
            .expect("pending_rbc object");
        assert_eq!(
            pending.get("stash_ready_total").and_then(Value::as_u64),
            Some(2)
        );
        assert_eq!(
            pending
                .get("stash_ready_init_missing_total")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            pending
                .get("stash_ready_roster_unverified_total")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            pending.get("stash_deliver_total").and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            pending.get("stash_chunk_total").and_then(Value::as_u64),
            Some(3)
        );

        let entries = pending
            .get("entries")
            .and_then(Value::as_array)
            .expect("pending entries array");
        assert_eq!(entries.len(), 1);
        let entry = entries[0].as_object().expect("pending entry object");
        assert_eq!(
            entry.get("block_hash").and_then(Value::as_str),
            Some(hash_str.as_str())
        );
        assert_eq!(entry.get("height").and_then(Value::as_u64), Some(9));
        assert_eq!(entry.get("view").and_then(Value::as_u64), Some(1));
        assert_eq!(entry.get("ready").and_then(Value::as_u64), Some(2));
        assert_eq!(entry.get("deliver").and_then(Value::as_u64), Some(1));
        assert_eq!(entry.get("age_ms").and_then(Value::as_u64), Some(42));
    }

    #[test]
    fn status_snapshot_json_includes_consensus_message_handling() {
        let snap = sumeragi::StatusSnapshot {
            consensus_message_handling: status::ConsensusMessageHandlingSnapshot {
                entries: vec![status::ConsensusMessageHandlingEntry {
                    kind: status::ConsensusMessageKind::BlockCreated,
                    outcome: status::ConsensusMessageOutcome::Dropped,
                    reason: status::ConsensusMessageReason::HintMismatch,
                    total: 4,
                }],
            },
            ..Default::default()
        };
        let payload = status_snapshot_json(&snap);
        let handling = payload
            .get("consensus_message_handling")
            .and_then(Value::as_object)
            .expect("consensus_message_handling object");
        let entries = handling
            .get("entries")
            .and_then(Value::as_array)
            .expect("consensus_message_handling entries");
        assert_eq!(entries.len(), 1);
        let entry = entries[0].as_object().expect("entry object");
        assert_eq!(
            entry.get("kind").and_then(Value::as_str),
            Some("block_created")
        );
        assert_eq!(
            entry.get("outcome").and_then(Value::as_str),
            Some("dropped")
        );
        assert_eq!(
            entry.get("reason").and_then(Value::as_str),
            Some("hint_mismatch")
        );
        assert_eq!(entry.get("total").and_then(Value::as_u64), Some(4));
    }

    #[test]
    fn status_snapshot_json_includes_recovery_reacquire_fields() {
        let snap = sumeragi::StatusSnapshot {
            consensus_missing_block_height_progress_deferred_total: 10,
            consensus_missing_qc_reacquire_attempt_total: 2,
            consensus_missing_qc_reacquire_success_total: 1,
            consensus_missing_qc_reacquire_exhausted_total: 3,
            consensus_missing_qc_rotation_deferred_total: 4,
            consensus_forced_proposal_attempt_total: 5,
            consensus_forced_proposal_success_total: 2,
            consensus_roster_unavailable_detected_total: 9,
            consensus_roster_unavailable_election_attempt_total: 6,
            consensus_roster_unavailable_election_success_total: 3,
            consensus_roster_unavailable_wait_candidates_total: 1,
            consensus_roster_recovery_state: Some("wait_candidates"),
            consensus_roster_recovery_dwell_ms: std::collections::BTreeMap::from([
                ("steady", 10_u64),
                ("wait_candidates", 20_u64),
            ]),
            blocksync_range_pull_candidate_exhausted_total: 7,
            ..Default::default()
        };
        let payload = status_snapshot_json(&snap);
        assert_eq!(
            payload
                .get("consensus_missing_block_height_progress_deferred_total")
                .and_then(Value::as_u64),
            Some(10)
        );
        assert_eq!(
            payload
                .get("consensus_missing_qc_reacquire_attempt_total")
                .and_then(Value::as_u64),
            Some(2)
        );
        assert_eq!(
            payload
                .get("consensus_missing_qc_reacquire_success_total")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            payload
                .get("consensus_missing_qc_reacquire_exhausted_total")
                .and_then(Value::as_u64),
            Some(3)
        );
        assert_eq!(
            payload
                .get("consensus_missing_qc_rotation_deferred_total")
                .and_then(Value::as_u64),
            Some(4)
        );
        assert_eq!(
            payload
                .get("consensus_forced_proposal_attempt_total")
                .and_then(Value::as_u64),
            Some(5)
        );
        assert_eq!(
            payload
                .get("consensus_forced_proposal_success_total")
                .and_then(Value::as_u64),
            Some(2)
        );
        assert!(
            payload.get("consensus_no_roster_fallback_total").is_none(),
            "fallback telemetry field removed after fail-closed-only no-roster cutover"
        );
        assert!(
            payload
                .get("consensus_no_roster_fallback_allowed_total")
                .is_none(),
            "fallback alias field removed after fail-closed-only no-roster cutover"
        );
        assert_eq!(
            payload
                .get("consensus_roster_unavailable_detected_total")
                .and_then(Value::as_u64),
            Some(9)
        );
        assert_eq!(
            payload
                .get("consensus_roster_unavailable_election_attempt_total")
                .and_then(Value::as_u64),
            Some(6)
        );
        assert_eq!(
            payload
                .get("consensus_roster_unavailable_election_success_total")
                .and_then(Value::as_u64),
            Some(3)
        );
        assert_eq!(
            payload
                .get("consensus_roster_unavailable_wait_candidates_total")
                .and_then(Value::as_u64),
            Some(1)
        );
        assert_eq!(
            payload
                .get("consensus_roster_recovery_state")
                .and_then(Value::as_str),
            Some("wait_candidates")
        );
        assert_eq!(
            payload
                .get("blocksync_range_pull_candidate_exhausted_total")
                .and_then(Value::as_u64),
            Some(7)
        );
    }

    #[test]
    fn status_snapshot_json_includes_da_gate_and_kura_store() {
        let last_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xCD; 32]));
        let snap = sumeragi::StatusSnapshot {
            da_gate: status::DaGateSnapshot {
                reason: status::DaGateReasonSnapshot::MissingLocalData,
                last_satisfied: status::DaGateSatisfactionSnapshot::ManifestGuardRecovered,
                missing_local_data_total: 2,
                manifest_guard_total: 4,
            },
            missing_block_fetch_total: 5,
            missing_block_fetch_last_targets: 3,
            missing_block_fetch_last_dwell_ms: 11,
            committed_edge_conflict_obsolete_total: 2,
            roster_sidecar_mismatch_obsolete_total: 6,
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
            Some("missing_local_data")
        );
        assert_eq!(
            gate.get("last_satisfied").and_then(Value::as_str),
            Some("manifest_guard_recovered")
        );
        assert_eq!(
            gate.get("missing_local_data_total").and_then(Value::as_u64),
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
        assert_eq!(
            payload
                .get("committed_edge_conflict_obsolete_total")
                .and_then(Value::as_u64),
            Some(2)
        );
        assert_eq!(
            payload
                .get("roster_sidecar_mismatch_obsolete_total")
                .and_then(Value::as_u64),
            Some(6)
        );

        let kura = payload
            .get("kura_store")
            .and_then(Value::as_object)
            .expect("kura_store object");
        assert_eq!(kura.get("failures_total").and_then(Value::as_u64), Some(1));
        assert_eq!(kura.get("abort_total").and_then(Value::as_u64), Some(2));
        assert_eq!(kura.get("stage_total").and_then(Value::as_u64), Some(0));
        assert_eq!(kura.get("rollback_total").and_then(Value::as_u64), Some(0));
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

        let block_sync = payload
            .get("block_sync")
            .and_then(Value::as_object)
            .expect("block_sync object");
        let roster = block_sync
            .get("roster")
            .and_then(Value::as_object)
            .expect("block_sync roster object");
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
        assert_eq!(
            roster
                .get("drop_unsolicited_share_blocks_total")
                .and_then(Value::as_u64),
            Some(0)
        );
    }

    #[test]
    fn status_snapshot_json_includes_canonical_v1_state() {
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xAB; 32]));
        let snap = sumeragi::StatusSnapshot {
            membership_height: 12,
            membership_view: 3,
            leader_index: 2,
            highest_qc_height: 11,
            highest_qc_view: 4,
            highest_qc_subject: Some(block_hash),
            locked_qc_height: 10,
            locked_qc_view: 1,
            locked_qc_subject: Some(block_hash),
            qc_deferred_missing_payload_total: 2,
            qc_deferred_resolved_total: 1,
            commit_qc: status::QcSnapshot {
                height: 12,
                view: 3,
                block_hash: Some(block_hash),
                validator_set_len: 4,
                ..Default::default()
            },
            ..Default::default()
        };

        let payload = status_snapshot_json(&snap);
        let canonical = payload
            .get("canonical")
            .and_then(Value::as_object)
            .expect("canonical v1 status");
        assert_eq!(canonical.get("height").and_then(Value::as_u64), Some(12));
        assert_eq!(canonical.get("view").and_then(Value::as_u64), Some(3));
        assert_eq!(
            canonical.get("phase").and_then(Value::as_str),
            Some("prepare")
        );
        assert_eq!(
            canonical.get("payload_status").and_then(Value::as_str),
            Some("missing_local_payload")
        );
        assert!(
            canonical
                .get("pending_finality")
                .and_then(Value::as_str)
                .is_none()
        );
        let quorum = canonical
            .get("quorum_policy")
            .and_then(Value::as_object)
            .expect("quorum policy");
        assert_eq!(
            quorum.get("kind").and_then(Value::as_str),
            Some("permissioned_count")
        );
        assert_eq!(quorum.get("validators").and_then(Value::as_u64), Some(4));
    }

    #[test]
    fn status_snapshot_json_uses_next_height_commit_quorum_for_missing_payload_pending_finality() {
        let committed_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xAD; 32]));
        let quorum_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xAE; 32]));
        let snap = sumeragi::StatusSnapshot {
            membership_height: 13,
            membership_view: 5,
            highest_qc_height: 12,
            highest_qc_view: 4,
            qc_deferred_missing_payload_total: 2,
            qc_deferred_resolved_total: 1,
            commit_qc: status::QcSnapshot {
                height: 12,
                view: 4,
                block_hash: Some(committed_hash),
                validator_set_len: 4,
                ..Default::default()
            },
            commit_quorum: status::CommitQuorumSnapshot {
                height: 13,
                view: 5,
                block_hash: Some(quorum_hash),
                signatures_counted: 3,
                signatures_required: 3,
                ..Default::default()
            },
            ..Default::default()
        };

        let payload = status_snapshot_json(&snap);
        let canonical = payload
            .get("canonical")
            .and_then(Value::as_object)
            .expect("canonical v1 status");
        assert_eq!(
            canonical.get("phase").and_then(Value::as_str),
            Some("pending_finality")
        );
        assert_eq!(
            canonical.get("payload_status").and_then(Value::as_str),
            Some("missing_local_payload")
        );
        let expected_block_hash = format!("{quorum_hash}");
        assert_eq!(
            canonical.get("pending_finality").and_then(Value::as_str),
            Some(expected_block_hash.as_str())
        );
    }

    #[test]
    fn status_snapshot_json_uses_canonical_pending_finality_snapshot() {
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xAC; 32]));
        let snap = sumeragi::StatusSnapshot {
            membership_height: 13,
            membership_view: 5,
            canonical_pending_finality: Some(block_hash),
            commit_qc: status::QcSnapshot {
                height: 12,
                view: 4,
                validator_set_len: 4,
                ..Default::default()
            },
            ..Default::default()
        };

        let payload = status_snapshot_json(&snap);
        let canonical = payload
            .get("canonical")
            .and_then(Value::as_object)
            .expect("canonical v1 status");
        assert_eq!(canonical.get("height").and_then(Value::as_u64), Some(13));
        assert_eq!(canonical.get("view").and_then(Value::as_u64), Some(5));
        assert_eq!(
            canonical.get("phase").and_then(Value::as_str),
            Some("pending_finality")
        );
        let expected_block_hash = format!("{block_hash}");
        assert_eq!(
            canonical.get("pending_finality").and_then(Value::as_str),
            Some(expected_block_hash.as_str())
        );
    }

    #[test]
    fn status_snapshot_json_includes_commit_qc_and_quorum() {
        let block_hash = HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([0x11; 32]),
        );
        let validator_set_hash =
            HashOf::<Vec<iroha_data_model::peer::PeerId>>::from_untyped_unchecked(Hash::prehashed(
                [0x22; 32],
            ));
        let snap = sumeragi::StatusSnapshot {
            commit_qc: status::QcSnapshot {
                height: 12,
                view: 3,
                epoch: 1,
                block_hash: Some(block_hash),
                validator_set_hash: Some(validator_set_hash),
                validator_set_len: 4,
                signatures_total: 3,
            },
            commit_quorum: status::CommitQuorumSnapshot {
                height: 12,
                view: 3,
                block_hash: Some(block_hash),
                signatures_present: 4,
                signatures_counted: 3,
                signatures_set_b: 2,
                signatures_required: 3,
                last_updated_ms: 1234,
            },
            commit_pipeline: status::CommitPipelineSnapshot {
                last_total_ms: 84,
                last_validation_ms: 21,
                last_qc_rebuild_ms: 8,
                last_gate_ms: 9,
                last_finalize_ms: 17,
                last_drain_results_ms: 12,
                last_drain_qc_verify_ms: 1,
                last_drain_persist_ms: 2,
                last_drain_kura_store_ms: 3,
                last_drain_state_apply_ms: 4,
                last_drain_state_commit_ms: 5,
                ema_total_ms: 80,
                ema_validation_ms: 19,
                ema_gate_ms: 8,
                ema_finalize_ms: 16,
            },
            round_gap: status::RoundGapSnapshot {
                last_deliver_to_state_commit_ms: 31,
                last_state_commit_to_next_propose_ms: 12,
                last_deliver_to_next_propose_ms: 43,
                ema_deliver_to_state_commit_ms: 29,
                ema_state_commit_to_next_propose_ms: 10,
                ema_deliver_to_next_propose_ms: 39,
            },
            pipeline_execution: status::PipelineExecutionSnapshot {
                tx_vertices_total: 10,
                detached_merged_total: 4,
                detached_fallback_total: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        let payload = status_snapshot_json(&snap);
        let commit_qc = payload
            .get("commit_qc")
            .and_then(Value::as_object)
            .expect("commit_qc object");
        let block_hash_str = format!("{block_hash}");
        let validator_set_hash_str = format!("{validator_set_hash}");
        assert_eq!(commit_qc.get("height").and_then(Value::as_u64), Some(12));
        assert_eq!(commit_qc.get("view").and_then(Value::as_u64), Some(3));
        assert_eq!(commit_qc.get("epoch").and_then(Value::as_u64), Some(1));
        assert_eq!(
            commit_qc.get("block_hash").and_then(Value::as_str),
            Some(block_hash_str.as_str())
        );
        assert_eq!(
            commit_qc.get("validator_set_hash").and_then(Value::as_str),
            Some(validator_set_hash_str.as_str())
        );
        assert_eq!(
            commit_qc.get("validator_set_len").and_then(Value::as_u64),
            Some(4)
        );
        assert_eq!(
            commit_qc.get("signatures_total").and_then(Value::as_u64),
            Some(3)
        );
        let commit_quorum = payload
            .get("commit_quorum")
            .and_then(Value::as_object)
            .expect("commit_quorum object");
        assert_eq!(
            commit_quorum.get("height").and_then(Value::as_u64),
            Some(12)
        );
        assert_eq!(commit_quorum.get("view").and_then(Value::as_u64), Some(3));
        assert_eq!(
            commit_quorum.get("block_hash").and_then(Value::as_str),
            Some(block_hash_str.as_str())
        );
        assert_eq!(
            commit_quorum
                .get("signatures_present")
                .and_then(Value::as_u64),
            Some(4)
        );
        assert_eq!(
            commit_quorum
                .get("signatures_counted")
                .and_then(Value::as_u64),
            Some(3)
        );
        assert_eq!(
            commit_quorum
                .get("signatures_set_b")
                .and_then(Value::as_u64),
            Some(2)
        );
        assert_eq!(
            commit_quorum
                .get("signatures_required")
                .and_then(Value::as_u64),
            Some(3)
        );
        assert_eq!(
            commit_quorum.get("last_updated_ms").and_then(Value::as_u64),
            Some(1234)
        );
        let commit_pipeline = payload
            .get("commit_pipeline")
            .and_then(Value::as_object)
            .expect("commit_pipeline object");
        assert_eq!(
            commit_pipeline.get("last_total_ms").and_then(Value::as_u64),
            Some(84)
        );
        assert_eq!(
            commit_pipeline
                .get("last_drain_state_commit_ms")
                .and_then(Value::as_u64),
            Some(5)
        );
        assert_eq!(
            commit_pipeline
                .get("ema_finalize_ms")
                .and_then(Value::as_u64),
            Some(16)
        );
        let round_gap = payload
            .get("round_gap")
            .and_then(Value::as_object)
            .expect("round_gap object");
        assert_eq!(
            round_gap
                .get("last_deliver_to_state_commit_ms")
                .and_then(Value::as_u64),
            Some(31)
        );
        assert_eq!(
            round_gap
                .get("last_state_commit_to_next_propose_ms")
                .and_then(Value::as_u64),
            Some(12)
        );
        assert_eq!(
            round_gap
                .get("ema_deliver_to_next_propose_ms")
                .and_then(Value::as_u64),
            Some(39)
        );
        let pipeline_execution = payload
            .get("pipeline_execution")
            .and_then(Value::as_object)
            .expect("pipeline execution object");
        assert_eq!(
            pipeline_execution
                .get("tx_vertices_total")
                .and_then(Value::as_u64),
            Some(10)
        );
        assert_eq!(
            pipeline_execution
                .get("detached_merged_total")
                .and_then(Value::as_u64),
            Some(4)
        );
        assert_eq!(
            pipeline_execution
                .get("detached_fallback_total")
                .and_then(Value::as_u64),
            Some(1)
        );
    }

    #[test]
    fn status_snapshot_json_includes_tx_queue_pressure_reasons() {
        let snap = sumeragi::StatusSnapshot {
            tx_queue_depth: 4,
            tx_queue_capacity: 20_000,
            tx_queue_retained_bytes: 1_024,
            tx_queue_max_retained_bytes: 65_536,
            tx_queue_saturated: false,
            tx_queue_saturated_by_count: false,
            tx_queue_saturated_by_bytes: false,
            tx_queue_saturated_by_age: true,
            tx_queue_oldest_queued_age_ms: 7_500,
            ..Default::default()
        };

        let payload = status_snapshot_json(&snap);
        let tx_queue = payload
            .get("tx_queue")
            .and_then(Value::as_object)
            .expect("tx_queue object");
        assert_eq!(tx_queue.get("depth").and_then(Value::as_u64), Some(4));
        assert_eq!(
            tx_queue.get("capacity").and_then(Value::as_u64),
            Some(20_000)
        );
        assert_eq!(
            tx_queue.get("retained_bytes").and_then(Value::as_u64),
            Some(1_024)
        );
        assert_eq!(
            tx_queue.get("max_retained_bytes").and_then(Value::as_u64),
            Some(65_536)
        );
        assert_eq!(
            tx_queue.get("saturated").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            tx_queue.get("saturated_by_count").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            tx_queue.get("saturated_by_bytes").and_then(Value::as_bool),
            Some(false)
        );
        assert_eq!(
            tx_queue.get("saturated_by_age").and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            tx_queue.get("oldest_queued_age_ms").and_then(Value::as_u64),
            Some(7_500)
        );
    }

    #[test]
    fn status_reconciliation_uses_committed_npos_epoch_parameters() {
        let world = iroha_core::state::World::default();
        {
            let mut block = world.block();
            let parameters = block.parameters.get_mut();
            parameters.sumeragi.next_mode =
                Some(iroha_data_model::parameter::system::SumeragiConsensusMode::Npos);
            parameters.sumeragi.mode_activation_height = Some(0);
            let npos_params = iroha_data_model::parameter::system::SumeragiNposParameters {
                epoch_length_blocks: 6,
                vrf_commit_window_blocks: 2,
                vrf_reveal_window_blocks: 4,
                ..iroha_data_model::parameter::system::SumeragiNposParameters::default()
                    .with_epoch_seed([0x42; 32])
            };
            parameters.custom.insert(
                iroha_data_model::parameter::system::SumeragiNposParameters::parameter_id(),
                npos_params.into_custom_parameter(),
            );
            block.commit();
        }
        let state = CoreState::new_for_testing(
            world,
            iroha_core::kura::Kura::blank_kura_for_testing(),
            iroha_core::query::store::LiveQueryStore::start_test(),
        );
        let stale_snapshot = sumeragi::StatusSnapshot {
            mode_tag: iroha_core::sumeragi::consensus::PERMISSIONED_TAG.to_owned(),
            epoch_length_blocks: 0,
            epoch_commit_deadline_offset: 0,
            epoch_reveal_deadline_offset: 0,
            prf_epoch_seed: None,
            ..Default::default()
        };

        let reconciled = reconcile_sumeragi_status_snapshot_with_world(stale_snapshot, &state);

        assert_eq!(
            reconciled.mode_tag,
            iroha_core::sumeragi::consensus::NPOS_TAG
        );
        assert_eq!(
            reconciled.staged_mode_tag.as_deref(),
            Some(iroha_core::sumeragi::consensus::NPOS_TAG)
        );
        assert_eq!(reconciled.staged_mode_activation_height, Some(0));
        assert_eq!(reconciled.epoch_length_blocks, 6);
        assert_eq!(reconciled.epoch_commit_deadline_offset, 2);
        assert_eq!(reconciled.epoch_reveal_deadline_offset, 6);
        assert_eq!(reconciled.prf_epoch_seed, Some([0x42; 32]));
    }

    #[test]
    fn status_reconciliation_preserves_permissioned_prf_seed() {
        let state = CoreState::new_for_testing(
            iroha_core::state::World::default(),
            iroha_core::kura::Kura::blank_kura_for_testing(),
            iroha_core::query::store::LiveQueryStore::start_test(),
        );
        let snapshot = sumeragi::StatusSnapshot {
            mode_tag: iroha_core::sumeragi::consensus::PERMISSIONED_TAG.to_owned(),
            epoch_length_blocks: 99,
            epoch_commit_deadline_offset: 12,
            epoch_reveal_deadline_offset: 34,
            prf_epoch_seed: Some([0x7A; 32]),
            ..Default::default()
        };

        let reconciled = reconcile_sumeragi_status_snapshot_with_world(snapshot, &state);

        assert_eq!(
            reconciled.mode_tag,
            iroha_core::sumeragi::consensus::PERMISSIONED_TAG
        );
        assert_eq!(reconciled.epoch_length_blocks, 0);
        assert_eq!(reconciled.epoch_commit_deadline_offset, 0);
        assert_eq!(reconciled.epoch_reveal_deadline_offset, 0);
        assert_eq!(reconciled.prf_epoch_seed, Some([0x7A; 32]));
    }
}

#[derive(Debug, crate::json_macros::JsonSerialize)]
struct LaneSettlementReceiptJson {
    source_id: [u8; 32],
    local_amount_micro: String,
    xor_due_micro: String,
    xor_after_haircut_micro: String,
    xor_variance_micro: String,
    timestamp_ms: u64,
}

impl From<iroha_data_model::block::consensus::LaneSettlementReceipt> for LaneSettlementReceiptJson {
    fn from(receipt: iroha_data_model::block::consensus::LaneSettlementReceipt) -> Self {
        Self {
            source_id: receipt.source_id,
            local_amount_micro: receipt.local_amount_micro.to_string(),
            xor_due_micro: receipt.xor_due_micro.to_string(),
            xor_after_haircut_micro: receipt.xor_after_haircut_micro.to_string(),
            xor_variance_micro: receipt.xor_variance_micro.to_string(),
            timestamp_ms: receipt.timestamp_ms,
        }
    }
}

#[derive(Debug, crate::json_macros::JsonSerialize)]
struct LaneBlockCommitmentJson {
    block_height: u64,
    lane_id: iroha_data_model::nexus::LaneId,
    lane_incarnation: iroha_crypto::Hash,
    dataspace_id: iroha_data_model::nexus::DataSpaceId,
    tx_count: u64,
    total_local_micro: String,
    total_xor_due_micro: String,
    total_xor_after_haircut_micro: String,
    total_xor_variance_micro: String,
    #[norito(default)]
    swap_metadata: Option<iroha_data_model::block::consensus::LaneSwapMetadata>,
    #[norito(default)]
    receipts: Vec<LaneSettlementReceiptJson>,
    #[norito(default)]
    nexus_fee_receipts: Vec<iroha_data_model::block::consensus::NexusFeeReceipt>,
    #[norito(default)]
    native_amx_receipts: Vec<iroha_data_model::block::consensus::NativeAmxReceipt>,
}

impl From<iroha_data_model::block::consensus::LaneBlockCommitment> for LaneBlockCommitmentJson {
    fn from(commitment: iroha_data_model::block::consensus::LaneBlockCommitment) -> Self {
        Self {
            block_height: commitment.block_height,
            lane_id: commitment.lane_id,
            lane_incarnation: commitment.lane_incarnation,
            dataspace_id: commitment.dataspace_id,
            tx_count: commitment.tx_count,
            total_local_micro: commitment.total_local_micro.to_string(),
            total_xor_due_micro: commitment.total_xor_due_micro.to_string(),
            total_xor_after_haircut_micro: commitment.total_xor_after_haircut_micro.to_string(),
            total_xor_variance_micro: commitment.total_xor_variance_micro.to_string(),
            swap_metadata: commitment.swap_metadata,
            receipts: commitment.receipts.into_iter().map(Into::into).collect(),
            nexus_fee_receipts: commitment.nexus_fee_receipts,
            native_amx_receipts: commitment.native_amx_receipts,
        }
    }
}

#[derive(Debug, crate::json_macros::JsonSerialize)]
struct LaneRelayEnvelopeJson {
    lane_id: iroha_data_model::nexus::LaneId,
    lane_incarnation: iroha_crypto::Hash,
    dataspace_id: iroha_data_model::nexus::DataSpaceId,
    block_height: u64,
    block_header: iroha_data_model::block::BlockHeader,
    #[norito(default)]
    qc: Option<iroha_data_model::consensus::Qc>,
    #[norito(default)]
    da_commitment_hash:
        Option<iroha_crypto::HashOf<iroha_data_model::da::commitment::DaCommitmentBundle>>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    lane_block_descriptor_hash: Option<iroha_crypto::Hash>,
    settlement_commitment: LaneBlockCommitmentJson,
    settlement_hash: iroha_crypto::HashOf<iroha_data_model::block::consensus::LaneBlockCommitment>,
    #[norito(default)]
    rbc_bytes_total: u64,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    manifest_root: Option<[u8; 32]>,
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    fastpq_proof: Option<iroha_data_model::nexus::LaneFastpqProofMaterial>,
}

impl From<iroha_data_model::nexus::LaneRelayEnvelope> for LaneRelayEnvelopeJson {
    fn from(envelope: iroha_data_model::nexus::LaneRelayEnvelope) -> Self {
        Self {
            lane_id: envelope.lane_id,
            lane_incarnation: envelope.lane_incarnation,
            dataspace_id: envelope.dataspace_id,
            block_height: envelope.block_height,
            block_header: envelope.block_header,
            qc: envelope.qc,
            da_commitment_hash: envelope.da_commitment_hash,
            lane_block_descriptor_hash: envelope.lane_block_descriptor_hash,
            settlement_commitment: envelope.settlement_commitment.into(),
            settlement_hash: envelope.settlement_hash,
            rbc_bytes_total: envelope.rbc_bytes_total,
            manifest_root: envelope.manifest_root,
            fastpq_proof: envelope.fastpq_proof,
        }
    }
}

#[derive(Debug, crate::json_macros::JsonSerialize)]
struct SumeragiV2StatusJson {
    #[norito(flatten)]
    authoritative: iroha_data_model::block::consensus_v2::SumeragiV2Status,
    lane_settlement_commitments: Vec<LaneBlockCommitmentJson>,
    lane_relay_envelopes: Vec<LaneRelayEnvelopeJson>,
    lane_payload_ownerships: Vec<iroha_data_model::block::consensus::SumeragiLanePayloadOwnership>,
    committed_lane_blocks: Vec<iroha_data_model::block::consensus::SumeragiCommittedLaneBlock>,
    lane_block_sessions: Vec<iroha_data_model::block::consensus::SumeragiLaneBlockSessionStatus>,
    local_peer_removed: bool,
    operator: iroha_data_model::block::consensus_v2::SumeragiV2OperatorStatus,
}

impl From<iroha_data_model::block::consensus_v2::SumeragiV2StatusResponse>
    for SumeragiV2StatusJson
{
    fn from(response: iroha_data_model::block::consensus_v2::SumeragiV2StatusResponse) -> Self {
        Self {
            authoritative: response.authoritative,
            lane_settlement_commitments: response
                .lane_settlement_commitments
                .into_iter()
                .map(Into::into)
                .collect(),
            lane_relay_envelopes: response
                .lane_relay_envelopes
                .into_iter()
                .map(Into::into)
                .collect(),
            lane_payload_ownerships: response.lane_payload_ownerships,
            committed_lane_blocks: response.committed_lane_blocks,
            lane_block_sessions: response.lane_block_sessions,
            local_peer_removed: response.local_peer_removed,
            operator: response.operator,
        }
    }
}

fn sumeragi_v2_status_json(
    authoritative: iroha_data_model::block::consensus_v2::SumeragiV2Status,
    state: &CoreState,
    nexus_enabled: bool,
) -> SumeragiV2StatusJson {
    sumeragi_v2_status_response(authoritative, state, nexus_enabled).into()
}

fn sumeragi_v2_status_response(
    authoritative: iroha_data_model::block::consensus_v2::SumeragiV2Status,
    state: &CoreState,
    nexus_enabled: bool,
) -> iroha_data_model::block::consensus_v2::SumeragiV2StatusResponse {
    let snapshot =
        reconcile_sumeragi_status_snapshot_with_world(sumeragi::status_snapshot(), state);
    sumeragi_v2_status_response_from_snapshot(
        authoritative,
        snapshot,
        nexus_enabled,
        sumeragi::status::local_peer_removed(),
    )
}

fn sumeragi_v2_status_json_from_snapshot(
    authoritative: iroha_data_model::block::consensus_v2::SumeragiV2Status,
    snapshot: sumeragi::StatusSnapshot,
    nexus_enabled: bool,
    local_peer_removed: bool,
) -> SumeragiV2StatusJson {
    sumeragi_v2_status_response_from_snapshot(
        authoritative,
        snapshot,
        nexus_enabled,
        local_peer_removed,
    )
    .into()
}

fn sumeragi_v2_status_response_from_snapshot(
    authoritative: iroha_data_model::block::consensus_v2::SumeragiV2Status,
    snapshot: sumeragi::StatusSnapshot,
    nexus_enabled: bool,
    local_peer_removed: bool,
) -> iroha_data_model::block::consensus_v2::SumeragiV2StatusResponse {
    let snapshot = if nexus_enabled {
        snapshot
    } else {
        snapshot.strip_lane_details()
    };
    let committed_lane_blocks = snapshot
        .committed_lane_blocks
        .iter()
        .map(committed_lane_block_wire)
        .collect();
    iroha_data_model::block::consensus_v2::SumeragiV2StatusResponse {
        authoritative,
        lane_settlement_commitments: snapshot.lane_settlement_commitments,
        lane_relay_envelopes: snapshot.lane_relay_envelopes,
        lane_payload_ownerships: snapshot.lane_payload_ownerships,
        committed_lane_blocks,
        lane_block_sessions: snapshot.lane_block_sessions,
        local_peer_removed,
        operator: sumeragi::status::v2_operator_status(),
    }
}

/// GET /v1/sumeragi/status — latest authoritative Sumeragi v2 snapshot.
///
/// The legacy status shape is archival-only. Until the v2 runner publishes a
/// replayed reducer snapshot, fail closed instead of exposing v1/RBC state as
/// though it described the live consensus protocol.
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_status(
    State(state): State<std::sync::Arc<CoreState>>,
    accept: Option<axum::http::HeaderValue>,
    nexus_enabled: bool,
) -> Result<Response> {
    let format = match crate::utils::negotiate_response_format(accept.as_ref()) {
        Ok(format) => format,
        Err(response) => return Ok(response),
    };
    let Some(status) = sumeragi::status::v2_status() else {
        return Ok(StatusCode::SERVICE_UNAVAILABLE.into_response());
    };
    let response = sumeragi_v2_status_response(status, state.as_ref(), nexus_enabled);
    response.validate().map_err(|error| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(format!(
            "refusing to serve invalid authoritative Sumeragi v2 status: {error}"
        )))
    })?;
    if matches!(format, crate::utils::ResponseFormat::Norito) {
        return Ok(crate::utils::respond_with_format(response, format));
    }

    let payload = SumeragiV2StatusJson::from(response);
    let body = norito::json::to_json_pretty(&payload).map_err(|error| {
        Error::Query(iroha_data_model::ValidationFail::InternalError(
            error.to_string(),
        ))
    })?;
    let mut response = axum::response::Response::new(axum::body::Body::from(body));
    response.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("application/json"),
    );
    Ok(response)
}

fn sumeragi_mode_tag(mode: ConsensusMode) -> &'static str {
    match mode {
        ConsensusMode::Permissioned => iroha_core::sumeragi::consensus::PERMISSIONED_TAG,
        ConsensusMode::Npos => iroha_core::sumeragi::consensus::NPOS_TAG,
    }
}

fn staged_sumeragi_mode_tag(
    mode: iroha_data_model::parameter::system::SumeragiConsensusMode,
) -> &'static str {
    match mode {
        iroha_data_model::parameter::system::SumeragiConsensusMode::Permissioned => {
            iroha_core::sumeragi::consensus::PERMISSIONED_TAG
        }
        iroha_data_model::parameter::system::SumeragiConsensusMode::Npos => {
            iroha_core::sumeragi::consensus::NPOS_TAG
        }
    }
}

fn status_snapshot_fallback_mode(
    snap: &sumeragi::StatusSnapshot,
    world_has_npos_params: bool,
) -> ConsensusMode {
    match snap.mode_tag.as_str() {
        iroha_core::sumeragi::consensus::NPOS_TAG | "Npos" => ConsensusMode::Npos,
        iroha_core::sumeragi::consensus::PERMISSIONED_TAG | "Permissioned" => {
            ConsensusMode::Permissioned
        }
        _ if world_has_npos_params => ConsensusMode::Npos,
        _ => ConsensusMode::Permissioned,
    }
}

fn reconcile_sumeragi_status_snapshot_with_world(
    mut snap: sumeragi::StatusSnapshot,
    state: &CoreState,
) -> sumeragi::StatusSnapshot {
    let world = state.world_view();
    let sumeragi_params = world.parameters().sumeragi();
    snap.staged_mode_tag = sumeragi_params
        .next_mode
        .map(staged_sumeragi_mode_tag)
        .map(ToOwned::to_owned);
    snap.staged_mode_activation_height = sumeragi_params.mode_activation_height;

    let npos_params = world.sumeragi_npos_parameters();
    let chain_height = u64::try_from(state.committed_height()).unwrap_or(u64::MAX);
    let fallback_mode = status_snapshot_fallback_mode(&snap, npos_params.is_some());
    let effective_mode = sumeragi::effective_consensus_mode_for_height_from_world(
        &world,
        chain_height,
        fallback_mode,
    );
    snap.mode_tag = sumeragi_mode_tag(effective_mode).to_owned();

    match effective_mode {
        ConsensusMode::Permissioned => {
            snap.epoch_length_blocks = 0;
            snap.epoch_commit_deadline_offset = 0;
            snap.epoch_reveal_deadline_offset = 0;
            snap.npos_repair_coverage = None;
        }
        ConsensusMode::Npos => {
            if let Some(params) = npos_params {
                let commit_offset = params.vrf_commit_window_blocks();
                snap.epoch_length_blocks = params.epoch_length_blocks();
                snap.epoch_commit_deadline_offset = commit_offset;
                snap.epoch_reveal_deadline_offset =
                    commit_offset.saturating_add(params.vrf_reveal_window_blocks());
                snap.prf_epoch_seed = snap.prf_epoch_seed.or(Some(params.epoch_seed()));
            }
        }
    }

    snap
}

/// SSE stream for `/v1/sumeragi/status/sse` using only authoritative v2 snapshots.
///
/// Before reducer replay completes the stream remains silent instead of
/// emitting the archival v1/RBC status shape.
pub fn handle_v1_sumeragi_status_sse(
    state: std::sync::Arc<CoreState>,
    poll_ms: u64,
    nexus_enabled: bool,
) -> Sse<impl futures::Stream<Item = Result<SseEvent, Infallible>>> {
    let interval = Duration::from_millis(poll_ms.max(100));
    let ticker = tokio::time::interval(interval);
    let stream = stream::unfold((ticker, state), move |(mut ticker, state)| async move {
        loop {
            ticker.tick().await;
            if let Some(status) = sumeragi::status::v2_status() {
                let response = sumeragi_v2_status_response(status, state.as_ref(), nexus_enabled);
                if let Err(error) = response.validate() {
                    iroha_logger::error!(
                        ?error,
                        "refusing to stream invalid authoritative Sumeragi v2 status"
                    );
                    continue;
                }
                let payload = SumeragiV2StatusJson::from(response);
                match norito::json::to_json(&payload) {
                    Ok(body) => {
                        let event = SseEvent::default().data(body);
                        break Some((Ok(event), (ticker, state)));
                    }
                    Err(error) => {
                        iroha_logger::error!(
                            ?error,
                            "failed to serialize authoritative Sumeragi v2 status"
                        );
                    }
                }
            }
        }
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
    let world = state.world_view();
    let mut active: Option<(u64, iroha_data_model::consensus::VrfEpochRecord)> = None;
    let mut latest_final: Option<(u64, iroha_data_model::consensus::VrfEpochRecord)> = None;
    for (epoch, record) in world.vrf_epochs().iter() {
        if record.finalized {
            latest_final = Some((*epoch, record.clone()));
        } else {
            active = Some((*epoch, record.clone()));
        }
    }
    let vrf_snapshot = active.or(latest_final).map(|(_, record)| record);
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
                json_entry("stash_ready_total", pending.stash_ready_total),
                json_entry(
                    "stash_ready_init_missing_total",
                    pending.stash_ready_init_missing_total,
                ),
                json_entry(
                    "stash_ready_roster_missing_total",
                    pending.stash_ready_roster_missing_total,
                ),
                json_entry(
                    "stash_ready_roster_hash_mismatch_total",
                    pending.stash_ready_roster_hash_mismatch_total,
                ),
                json_entry(
                    "stash_ready_roster_unverified_total",
                    pending.stash_ready_roster_unverified_total,
                ),
                json_entry("stash_deliver_total", pending.stash_deliver_total),
                json_entry(
                    "stash_deliver_init_missing_total",
                    pending.stash_deliver_init_missing_total,
                ),
                json_entry(
                    "stash_deliver_roster_missing_total",
                    pending.stash_deliver_roster_missing_total,
                ),
                json_entry(
                    "stash_deliver_roster_hash_mismatch_total",
                    pending.stash_deliver_roster_hash_mismatch_total,
                ),
                json_entry(
                    "stash_deliver_roster_unverified_total",
                    pending.stash_deliver_roster_unverified_total,
                ),
                json_entry("stash_chunk_total", pending.stash_chunk_total),
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
    let (penalty_epoch, committed_total, no_participation_total, late_reveals_total) =
        status::vrf_penalty_snapshot();
    if let Some(r) = iroha_core::sumeragi::epoch_report::get(ep) {
        let payload = crate::json_object(vec![
            json_entry("epoch", r.epoch),
            json_entry("roster_len", r.roster_len),
            json_entry("committed_no_reveal", r.committed_no_reveal),
            json_entry("no_participation", r.no_participation),
            json_entry("vrf_penalty_epoch", penalty_epoch),
            json_entry("vrf_committed_no_reveal_total", committed_total),
            json_entry("vrf_no_participation_total", no_participation_total),
            json_entry("vrf_late_reveals_total", late_reveals_total),
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
            json_entry("committed_no_reveal", Vec::<u32>::new()),
            json_entry("no_participation", Vec::<u32>::new()),
            json_entry("vrf_penalty_epoch", penalty_epoch),
            json_entry("vrf_committed_no_reveal_total", committed_total),
            json_entry("vrf_no_participation_total", no_participation_total),
            json_entry("vrf_late_reveals_total", late_reveals_total),
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
    let world = state.world_view();
    let record_opt = world
        .vrf_epochs()
        .iter()
        .find(|entry| *entry.0 == epoch)
        .map(|(_, rec)| rec.clone());

    let payload = if let Some(record) = record_opt {
        let late_reveals_total = u64::try_from(record.late_reveals.len()).unwrap_or(0);
        let late_reveals: Vec<Value> = record
            .late_reveals
            .iter()
            .map(|entry| {
                json_object(vec![
                    json_entry("signer", entry.signer),
                    json_entry("noted_at_height", entry.noted_at_height),
                ])
            })
            .collect();
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
            json_entry("late_reveals_total", late_reveals_total),
            json_entry("late_reveals", Value::Array(late_reveals)),
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
            json_entry("late_reveals_total", 0u64),
            json_entry("late_reveals", Value::Array(Vec::new())),
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
    pub bls_sig_hex: String,
}

struct VrfRevealRequestDto {
    pub epoch: u64,
    pub signer: u32,
    pub reveal_hex: String,
    pub bls_sig_hex: String,
}

pub fn handle_post_sumeragi_vrf_commit(
    sumeragi: SumeragiHandle,
    request: VrfCommitRequestDto,
) -> Result<axum::response::Response, Error> {
    let commitment = parse_hex32(&request.commitment_hex, "commitment_hex")?;
    let bls_sig = parse_hex_bytes(&request.bls_sig_hex, "bls_sig_hex")?;
    let commit = iroha_data_model::block::consensus::VrfCommit {
        epoch: request.epoch,
        commitment,
        signer: request.signer,
        bls_sig,
    };
    if !sumeragi.incoming_block_message(BlockMessage::VrfCommit(commit)) {
        return Ok(StatusCode::SERVICE_UNAVAILABLE.into_response());
    }
    Ok(StatusCode::ACCEPTED.into_response())
}

pub fn handle_post_sumeragi_vrf_reveal(
    sumeragi: SumeragiHandle,
    request: VrfRevealRequestDto,
) -> Result<axum::response::Response, Error> {
    let reveal = parse_hex32(&request.reveal_hex, "reveal_hex")?;
    let bls_sig = parse_hex_bytes(&request.bls_sig_hex, "bls_sig_hex")?;
    let msg = iroha_data_model::block::consensus::VrfReveal {
        epoch: request.epoch,
        reveal,
        signer: request.signer,
        bls_sig,
    };
    if !sumeragi.incoming_block_message(BlockMessage::VrfReveal(msg)) {
        return Ok(StatusCode::SERVICE_UNAVAILABLE.into_response());
    }
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
                    "complete_delivery",
                    rbc_status_summary_has_complete_delivery(&s),
                ),
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
            "init_requests_total",
            m.sumeragi_rbc_init_requests_total.get(),
        ),
        json_entry(
            "chunk_requests_total",
            m.sumeragi_rbc_chunk_requests_total.get(),
        ),
        json_entry(
            "requested_chunks_total",
            m.sumeragi_rbc_requested_chunks_total.get(),
        ),
        json_entry(
            "init_repair_fallback_total",
            m.sumeragi_rbc_repair_fallback_total
                .with_label_values(&["init"])
                .get(),
        ),
        json_entry(
            "chunk_repair_fallback_total",
            m.sumeragi_rbc_repair_fallback_total
                .with_label_values(&["chunk"])
                .get(),
        ),
        json_entry(
            "ready_broadcasts_total",
            m.sumeragi_rbc_ready_broadcasts_total.get(),
        ),
        json_entry(
            "ready_rebroadcasts_skipped_total",
            m.sumeragi_rbc_rebroadcast_skipped_total
                .with_label_values(&["ready"])
                .get(),
        ),
        json_entry(
            "deliver_broadcasts_total",
            m.sumeragi_rbc_deliver_broadcasts_total.get(),
        ),
        json_entry(
            "payload_bytes_delivered_total",
            m.sumeragi_rbc_payload_bytes_delivered_total.get(),
        ),
        json_entry(
            "reconstructed_stripes_total",
            m.sumeragi_rbc_reconstructed_stripes_total.get(),
        ),
        json_entry(
            "seed_latency_count",
            m.sumeragi_rbc_seed_latency_ms.get_sample_count(),
        ),
        json_entry(
            "payload_rebroadcasts_skipped_total",
            m.sumeragi_rbc_rebroadcast_skipped_total
                .with_label_values(&["payload"])
                .get(),
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
/// Returns compact JSON with `delivered=true` only for non-invalid positive complete chunks.
/// Matching incomplete or invalid sessions remain visible through the summary fields.
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

    // If multiple sessions exist (conflicting proposals), report delivery only when
    // DELIVER is backed by internally consistent positive chunk accounting.
    let delivered_any = matches.iter().any(rbc_status_summary_has_complete_delivery);
    // Prefer a complete delivered session to report details; otherwise keep the
    // most complete available diagnostic entry.
    matches.sort_by_key(|s| {
        (
            !rbc_status_summary_has_complete_delivery(s),
            std::cmp::Reverse(u64::from(s.received_chunks)),
            std::cmp::Reverse(u64::from(s.total_chunks)),
        )
    });
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

fn rbc_status_summary_has_complete_delivery(summary: &rbc_status::Summary) -> bool {
    summary.delivered
        && !summary.invalid
        && summary.total_chunks != 0
        && summary.received_chunks == summary.total_chunks
}

/// GET /v1/sumeragi/commit-qcs/{block_hash} — return the full commit QC record for a block hash.
#[iroha_futures::telemetry_future]
pub async fn handle_v1_sumeragi_commit_qc(
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
    let world = state.world_view();
    let qc_opt = world.commit_qcs().get(&typed).cloned();
    let format = match crate::utils::negotiate_response_format(accept.as_ref()) {
        Ok(fmt) => fmt,
        Err(resp) => return Ok(resp),
    };

    if matches!(format, crate::utils::ResponseFormat::Norito) {
        return Ok(crate::NoritoBody(qc_opt).into_response());
    }
    let payload = match qc_opt.as_ref() {
        Some(qc) => {
            let validator_set = norito::json::Value::Array(
                qc.validator_set
                    .iter()
                    .map(|peer| norito::json::Value::from(peer.to_string()))
                    .collect(),
            );
            let commit_qc = crate::json_object(vec![
                json_entry("phase", format!("{:?}", qc.phase)),
                json_entry("parent_state_root", format!("{}", qc.parent_state_root)),
                json_entry("post_state_root", format!("{}", qc.post_state_root)),
                json_entry("height", qc.height),
                json_entry("view", qc.view),
                json_entry("epoch", qc.epoch),
                json_entry("mode_tag", qc.mode_tag.clone()),
                json_entry("validator_set_hash", format!("{}", qc.validator_set_hash)),
                json_entry("validator_set_hash_version", qc.validator_set_hash_version),
                json_entry("validator_set", validator_set),
                json_entry("signers_bitmap", hex::encode(&qc.aggregate.signers_bitmap)),
                json_entry(
                    "bls_aggregate_signature",
                    hex::encode(&qc.aggregate.bls_aggregate_signature),
                ),
            ]);
            crate::json_object(vec![
                json_entry("subject_block_hash", hash_hex.clone()),
                json_entry("commit_qc", commit_qc),
            ])
        }
        None => crate::json_object(vec![
            json_entry("subject_block_hash", hash_hex.clone()),
            json_entry("commit_qc", norito::json::Value::Null),
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
