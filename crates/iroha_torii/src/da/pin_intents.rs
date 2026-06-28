//! Torii handlers for DA pin intents.
//!
//! These endpoints operate on the in-memory pin intent index populated during
//! block application. Durable WSV plumbing can replace the backing store once
//! available without changing the handler surface.

use std::num::NonZeroU64;

use axum::extract::State;
use iroha_config::parameters::actual::Nexus;
use iroha_core::{da::pin_store::DaPinStore, state::WorldStateSnapshot};
use iroha_data_model::{
    da::{pin_intent::DaPinIntentWithLocation, types::StorageTicketId},
    query::parameters::Pagination,
    sorafs::pin_registry::ManifestDigest,
};

use crate::{Error, JsonBody, NoritoJson, SharedAppState};

const ENDPOINT_DA_PIN_INTENTS: &str = "/v1/da/pin_intents";
const ENDPOINT_DA_PIN_INTENTS_PROVE: &str = "/v1/da/pin_intents/prove";
const ENDPOINT_DA_PIN_INTENTS_VERIFY: &str = "/v1/da/pin_intents/verify";

/// Request payload for DA pin intent queries and proof generation.
#[derive(
    Debug,
    Default,
    Clone,
    crate::json_macros::JsonDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
)]
pub struct DaPinIntentQueryRequest {
    #[norito(default)]
    pub manifest_hash: Option<ManifestDigest>,
    #[norito(default)]
    pub storage_ticket: Option<StorageTicketId>,
    #[norito(default)]
    pub alias: Option<String>,
    #[norito(default)]
    pub lane_id: Option<u32>,
    #[norito(default)]
    pub epoch: Option<u64>,
    #[norito(default)]
    pub sequence: Option<u64>,
    #[norito(default)]
    pub pagination: Option<Pagination>,
}

/// Verification response for indexed DA pin intent location data.
#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
)]
pub struct DaPinIntentVerifyResponse {
    pub valid: bool,
}

/// HTTP handler for `/v1/da/pin_intents`.
pub async fn handler_list_pin_intents(
    State(app): State<SharedAppState>,
    NoritoJson(request): NoritoJson<DaPinIntentQueryRequest>,
) -> Result<JsonBody<Vec<DaPinIntentWithLocation>>, Error> {
    let nexus = app.state.nexus_snapshot();
    crate::ensure_nexus_lanes_enabled(nexus.enabled, ENDPOINT_DA_PIN_INTENTS)?;
    let store = app.state.da_pin_intents();
    let items = list_active_from_store(&store, &request, &nexus);
    Ok(JsonBody(items))
}

/// HTTP handler for `/v1/da/pin_intents/prove`.
pub async fn handler_prove_pin_intent(
    State(app): State<SharedAppState>,
    NoritoJson(request): NoritoJson<DaPinIntentQueryRequest>,
) -> Result<JsonBody<Option<DaPinIntentWithLocation>>, Error> {
    let nexus = app.state.nexus_snapshot();
    crate::ensure_nexus_lanes_enabled(nexus.enabled, ENDPOINT_DA_PIN_INTENTS_PROVE)?;
    let store = app.state.da_pin_intents();
    let proof = find_active_in_store(&store, &request, &nexus);
    Ok(JsonBody(proof))
}

/// HTTP handler for `/v1/da/pin_intents/verify`.
pub async fn handler_verify_pin_intent(
    State(app): State<SharedAppState>,
    NoritoJson(proof): NoritoJson<DaPinIntentWithLocation>,
) -> Result<JsonBody<DaPinIntentVerifyResponse>, Error> {
    let nexus = app.state.nexus_snapshot();
    crate::ensure_nexus_lanes_enabled(nexus.enabled, ENDPOINT_DA_PIN_INTENTS_VERIFY)?;
    let store = app.state.da_pin_intents();
    let valid = verify_against_store(&store, &nexus, &proof);
    Ok(JsonBody(DaPinIntentVerifyResponse { valid }))
}

fn list_active_from_store(
    store: &DaPinStore,
    request: &DaPinIntentQueryRequest,
    nexus: &Nexus,
) -> Vec<DaPinIntentWithLocation> {
    let pagination = request.pagination.clone().unwrap_or_default();
    let limit = pagination
        .limit
        .map(NonZeroU64::get)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or_else(|| store.len());
    let Ok(offset) = usize::try_from(pagination.offset) else {
        return Vec::new();
    };
    if limit == 0 {
        return Vec::new();
    }

    if let Some(target) = find_active_in_store(store, request, nexus) {
        return if offset == 0 {
            vec![target]
        } else {
            Vec::new()
        };
    }
    if request_targets_pin_intent(request) {
        return Vec::new();
    }

    store
        .all_sorted()
        .filter(|entry| pin_intent_lane_is_active(nexus, entry))
        .skip(offset)
        .take(limit)
        .cloned()
        .collect()
}

fn list_from_store(
    store: &DaPinStore,
    request: &DaPinIntentQueryRequest,
) -> Vec<DaPinIntentWithLocation> {
    let pagination = request.pagination.clone().unwrap_or_default();
    let limit = pagination
        .limit
        .map(NonZeroU64::get)
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or_else(|| store.len());
    let Ok(offset) = usize::try_from(pagination.offset) else {
        return Vec::new();
    };
    if limit == 0 {
        return Vec::new();
    }

    if let Some(target) = find_in_store(store, request) {
        return if offset == 0 {
            vec![target]
        } else {
            Vec::new()
        };
    }
    if request_targets_pin_intent(request) {
        return Vec::new();
    }
    if offset >= store.len() {
        return Vec::new();
    }

    store
        .all_sorted()
        .skip(offset)
        .take(limit)
        .cloned()
        .collect()
}

fn find_active_in_store(
    store: &DaPinStore,
    request: &DaPinIntentQueryRequest,
    nexus: &Nexus,
) -> Option<DaPinIntentWithLocation> {
    find_in_store(store, request).filter(|entry| pin_intent_lane_is_active(nexus, entry))
}

fn find_in_store(
    store: &DaPinStore,
    request: &DaPinIntentQueryRequest,
) -> Option<DaPinIntentWithLocation> {
    if let Some(ticket) = request.storage_ticket {
        let target = store.get_by_ticket(&ticket)?.clone();
        return request_matches_pin_intent(&target, request).then_some(target);
    }

    if let Some(alias) = &request.alias {
        let target = store
            .get_by_alias(alias)
            .map(|(_, record)| record.clone())?;
        return request_matches_pin_intent(&target, request).then_some(target);
    }

    if let Some(manifest) = request.manifest_hash {
        let target = store.get_by_manifest(&manifest)?.clone();
        return request_matches_pin_intent(&target, request).then_some(target);
    }

    let (Some(lane_id), Some(epoch), Some(sequence)) =
        (request.lane_id, request.epoch, request.sequence)
    else {
        return None;
    };

    store
        .get_by_lane_epoch_sequence(lane_id, epoch, sequence)
        .cloned()
}

fn request_targets_pin_intent(request: &DaPinIntentQueryRequest) -> bool {
    request.manifest_hash.is_some()
        || request.storage_ticket.is_some()
        || request.alias.is_some()
        || request.lane_id.is_some()
        || request.epoch.is_some()
        || request.sequence.is_some()
}

fn request_matches_pin_intent(
    target: &DaPinIntentWithLocation,
    request: &DaPinIntentQueryRequest,
) -> bool {
    if request
        .storage_ticket
        .is_some_and(|ticket| target.intent.storage_ticket != ticket)
    {
        return false;
    }
    if request
        .alias
        .as_ref()
        .is_some_and(|alias| target.intent.alias.as_deref() != Some(alias.as_str()))
    {
        return false;
    }
    if request
        .manifest_hash
        .is_some_and(|manifest| target.intent.manifest_hash != manifest)
    {
        return false;
    }
    if request
        .lane_id
        .is_some_and(|lane_id| target.intent.lane_id.as_u32() != lane_id)
    {
        return false;
    }
    if request
        .epoch
        .is_some_and(|epoch| target.intent.epoch != epoch)
    {
        return false;
    }
    if request
        .sequence
        .is_some_and(|sequence| target.intent.sequence != sequence)
    {
        return false;
    }
    true
}

fn pin_intent_lane_is_active(nexus: &Nexus, proof: &DaPinIntentWithLocation) -> bool {
    iroha_core::da::active_lane_proof_policy(nexus, proof.intent.lane_id).is_ok()
}

fn verify_against_store(
    store: &DaPinStore,
    nexus: &Nexus,
    proof: &DaPinIntentWithLocation,
) -> bool {
    if !pin_intent_lane_is_active(nexus, proof) {
        return false;
    }

    let ticket = &proof.intent.storage_ticket;
    store
        .get_by_ticket(ticket)
        .map(|stored| stored.intent == proof.intent && stored.location == proof.location)
        .unwrap_or(false)
}

#[cfg(all(test, feature = "app_api"))]
mod tests {
    use std::num::NonZeroU32;

    use iroha_data_model::{
        da::{commitment::DaCommitmentLocation, pin_intent::DaPinIntent, types::StorageTicketId},
        nexus::{LaneCatalog, LaneConfig as ModelLaneConfig, LaneId},
    };

    use super::*;

    fn sample_intent(lane: u32, epoch: u64, sequence: u64) -> DaPinIntent {
        DaPinIntent::new(
            LaneId::new(lane),
            epoch,
            sequence,
            StorageTicketId::new([lane as u8; 32]),
            ManifestDigest::new([sequence as u8; 32]),
        )
    }

    fn store_with_records() -> DaPinStore {
        let intents = vec![
            DaPinIntentWithLocation {
                intent: sample_intent(1, 1, 1),
                location: DaCommitmentLocation {
                    block_height: 5,
                    index_in_bundle: 0,
                },
            },
            DaPinIntentWithLocation {
                intent: sample_intent(2, 2, 0),
                location: DaCommitmentLocation {
                    block_height: 6,
                    index_in_bundle: 1,
                },
            },
            DaPinIntentWithLocation {
                intent: sample_intent(3, 1, 5),
                location: DaCommitmentLocation {
                    block_height: 7,
                    index_in_bundle: 2,
                },
            },
        ];
        DaPinStore::from_intents(&intents)
    }

    fn pagination(limit: Option<u64>, offset: u64) -> Pagination {
        Pagination::new(limit.and_then(NonZeroU64::new), offset)
    }

    fn lane_catalog_with_lane_ids(lane_ids: &[u32]) -> LaneCatalog {
        let max_lane = lane_ids.iter().copied().max().unwrap_or(0);
        let lane_count = NonZeroU32::new(max_lane.saturating_add(1)).expect("lane count");
        let mut lanes = lane_ids
            .iter()
            .copied()
            .map(|lane_id| ModelLaneConfig {
                id: LaneId::new(lane_id),
                alias: format!("lane-{lane_id}"),
                ..ModelLaneConfig::default()
            })
            .collect::<Vec<_>>();
        lanes.sort_by_key(|lane| lane.id.as_u32());
        lanes.dedup_by_key(|lane| lane.id.as_u32());
        LaneCatalog::new(lane_count, lanes).expect("lane catalog")
    }

    fn nexus_with_lane_ids(lane_ids: &[u32]) -> Nexus {
        let lane_catalog = lane_catalog_with_lane_ids(lane_ids);
        Nexus {
            enabled: true,
            lane_config: iroha_config::parameters::actual::LaneConfig::from_catalog(&lane_catalog),
            lane_catalog,
            ..Nexus::default()
        }
    }

    fn enable_nexus_with_lane_ids(app: &mut crate::SharedAppState, lane_ids: &[u32]) {
        let app = std::sync::Arc::get_mut(app).expect("unique app state");
        let state = std::sync::Arc::get_mut(&mut app.state).expect("unique core state");
        let nexus_cfg = nexus_with_lane_ids(lane_ids);
        state
            .set_nexus(nexus_cfg)
            .expect("enable Nexus lane catalog for tests");
    }

    fn enable_nexus(app: &mut crate::SharedAppState) {
        enable_nexus_with_lane_ids(app, &[0]);
    }

    fn install_stale_runtime_lane_geometry(app: &crate::SharedAppState, stale_lane: LaneId) {
        let authoritative_catalog = lane_catalog_with_lane_ids(&[0]);
        let stale_geometry_catalog = lane_catalog_with_lane_ids(&[0, stale_lane.as_u32()]);
        let mut nexus = app.state.nexus.write();
        nexus.enabled = true;
        nexus.lane_catalog = authoritative_catalog;
        nexus.lane_config =
            iroha_config::parameters::actual::LaneConfig::from_catalog(&stale_geometry_catalog);
        assert!(
            nexus.lane_config.entry(stale_lane).is_some(),
            "fixture must retain stale runtime geometry for the removed lane"
        );
    }

    fn seed_pin_store(app: &mut crate::SharedAppState, store: DaPinStore) {
        let app = std::sync::Arc::get_mut(app).expect("unique app state");
        let state = std::sync::Arc::get_mut(&mut app.state).expect("unique core state");
        drop(state.da_pin_intents());
        *state.da_pin_intents.write() = store;
    }

    #[test]
    fn list_respects_pagination() {
        let store = store_with_records();
        let request = DaPinIntentQueryRequest {
            pagination: Some(pagination(Some(2), 1)),
            ..DaPinIntentQueryRequest::default()
        };

        let items = list_from_store(&store, &request);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].location.index_in_bundle, 1);
        assert_eq!(items[1].location.index_in_bundle, 2);
        assert_eq!(items[0].location.block_height, 6);
        assert_eq!(items[1].location.block_height, 7);
    }

    #[test]
    fn list_with_manifest_filters_correct_record() {
        let store = store_with_records();
        let manifest = ManifestDigest::new([5; 32]);
        let request = DaPinIntentQueryRequest {
            manifest_hash: Some(manifest),
            ..DaPinIntentQueryRequest::default()
        };

        let items = list_from_store(&store, &request);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].intent.manifest_hash, manifest);
        assert_eq!(items[0].location.index_in_bundle, 2);
        assert_eq!(items[0].location.block_height, 7);
    }

    #[test]
    fn list_manifest_filter_rejects_conflicting_tuple() {
        let store = store_with_records();
        let request = DaPinIntentQueryRequest {
            manifest_hash: Some(ManifestDigest::new([5; 32])),
            lane_id: Some(2),
            epoch: Some(1),
            sequence: Some(5),
            ..DaPinIntentQueryRequest::default()
        };

        let items = list_from_store(&store, &request);
        assert!(items.is_empty());
    }

    #[test]
    fn list_partial_tuple_filter_does_not_fall_back_to_full_list() {
        let store = store_with_records();
        let request = DaPinIntentQueryRequest {
            lane_id: Some(3),
            ..DaPinIntentQueryRequest::default()
        };

        let items = list_from_store(&store, &request);
        assert!(items.is_empty());
    }

    #[test]
    fn list_over_offset_returns_empty_page() {
        let store = store_with_records();
        let request = DaPinIntentQueryRequest {
            pagination: Some(pagination(Some(1), u64::MAX)),
            ..DaPinIntentQueryRequest::default()
        };

        let items = list_from_store(&store, &request);
        assert!(items.is_empty());
    }

    #[test]
    fn list_overlarge_limit_is_bounded_to_store_length() {
        let store = store_with_records();
        let request = DaPinIntentQueryRequest {
            pagination: Some(pagination(Some(u64::MAX), 0)),
            ..DaPinIntentQueryRequest::default()
        };

        let items = list_from_store(&store, &request);
        assert_eq!(items.len(), store.len());
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn prove_uses_lane_epoch_sequence() {
        let store = store_with_records();
        let request = DaPinIntentQueryRequest {
            lane_id: Some(3),
            epoch: Some(1),
            sequence: Some(5),
            ..DaPinIntentQueryRequest::default()
        };

        let proof = find_in_store(&store, &request).expect("proof should exist");
        assert_eq!(proof.location.index_in_bundle, 2);
        assert_eq!(proof.location.block_height, 7);
    }

    #[test]
    fn prove_rejects_conflicting_ticket_and_tuple_selectors() {
        let store = store_with_records();
        let request = DaPinIntentQueryRequest {
            storage_ticket: Some(StorageTicketId::new([3; 32])),
            lane_id: Some(2),
            epoch: Some(1),
            sequence: Some(5),
            ..DaPinIntentQueryRequest::default()
        };

        assert!(find_in_store(&store, &request).is_none());
    }

    #[test]
    fn verify_rejects_mismatched_ticket() {
        let store = store_with_records();
        let nexus = nexus_with_lane_ids(&[0, 1, 2, 3]);
        let mut proof = find_in_store(
            &store,
            &DaPinIntentQueryRequest {
                manifest_hash: Some(ManifestDigest::new([1; 32])),
                ..DaPinIntentQueryRequest::default()
            },
        )
        .expect("proof should exist");

        proof.intent.storage_ticket = StorageTicketId::new([9; 32]);
        assert!(!verify_against_store(&store, &nexus, &proof));
    }

    #[tokio::test]
    async fn handlers_reject_when_nexus_disabled() {
        let app = crate::mk_app_state_for_tests();
        let err = super::handler_list_pin_intents(
            State(app),
            NoritoJson(DaPinIntentQueryRequest::default()),
        )
        .await
        .expect_err("DA endpoints should reject when Nexus is disabled");
        match err {
            Error::AppQueryValidation { code, message } => {
                assert_eq!(code, "nexus_disabled");
                assert!(
                    message.contains("nexus.enabled=true"),
                    "message should explain required flag: {message}"
                );
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[tokio::test]
    async fn handler_succeeds_when_nexus_enabled() {
        let mut app = crate::mk_app_state_for_tests();
        enable_nexus(&mut app);
        let JsonBody(items) = super::handler_list_pin_intents(
            State(app),
            NoritoJson(DaPinIntentQueryRequest::default()),
        )
        .await
        .expect("handler should succeed");
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn handler_prove_and_verify_roundtrip_indexed_pin_intent() {
        let mut app = crate::mk_app_state_for_tests();
        enable_nexus_with_lane_ids(&mut app, &[0, 1, 2, 3]);
        seed_pin_store(&mut app, store_with_records());

        let JsonBody(proof) = super::handler_prove_pin_intent(
            State(app.clone()),
            NoritoJson(DaPinIntentQueryRequest {
                lane_id: Some(3),
                epoch: Some(1),
                sequence: Some(5),
                ..DaPinIntentQueryRequest::default()
            }),
        )
        .await
        .expect("pin intent proof lookup should succeed");
        let proof = proof.expect("indexed pin intent should be present");

        assert_eq!(proof.intent.manifest_hash, ManifestDigest::new([5; 32]));
        assert_eq!(proof.location.block_height, 7);
        assert_eq!(proof.location.index_in_bundle, 2);

        let JsonBody(response) = super::handler_verify_pin_intent(State(app), NoritoJson(proof))
            .await
            .expect("pin intent verification should succeed");
        assert!(response.valid);
    }

    #[tokio::test]
    async fn handler_verify_rejects_tampered_indexed_pin_intent() {
        let mut app = crate::mk_app_state_for_tests();
        enable_nexus_with_lane_ids(&mut app, &[0, 1, 2, 3]);
        seed_pin_store(&mut app, store_with_records());

        let JsonBody(proof) = super::handler_prove_pin_intent(
            State(app.clone()),
            NoritoJson(DaPinIntentQueryRequest {
                manifest_hash: Some(ManifestDigest::new([5; 32])),
                ..DaPinIntentQueryRequest::default()
            }),
        )
        .await
        .expect("pin intent proof lookup should succeed");
        let mut proof = proof.expect("indexed pin intent should be present");
        proof.location.index_in_bundle += 1;

        let JsonBody(response) = super::handler_verify_pin_intent(State(app), NoritoJson(proof))
            .await
            .expect("pin intent verification should succeed");
        assert!(!response.valid);
    }

    #[tokio::test]
    async fn handler_verify_rejects_stale_runtime_lane_geometry() {
        let mut app = crate::mk_app_state_for_tests();
        enable_nexus_with_lane_ids(&mut app, &[0, 1]);
        let stale = DaPinIntentWithLocation {
            intent: sample_intent(1, 3, 7),
            location: DaCommitmentLocation {
                block_height: 8,
                index_in_bundle: 0,
            },
        };
        seed_pin_store(
            &mut app,
            DaPinStore::from_intents(std::slice::from_ref(&stale)),
        );

        let JsonBody(proof) = super::handler_prove_pin_intent(
            State(app.clone()),
            NoritoJson(DaPinIntentQueryRequest {
                storage_ticket: Some(stale.intent.storage_ticket),
                ..DaPinIntentQueryRequest::default()
            }),
        )
        .await
        .expect("pin intent proof lookup should succeed");
        let proof = proof.expect("indexed pin intent should be present before lane removal");

        install_stale_runtime_lane_geometry(&app, stale.intent.lane_id);

        let JsonBody(response) = super::handler_verify_pin_intent(State(app), NoritoJson(proof))
            .await
            .expect("pin intent verification should succeed");
        assert!(!response.valid);
    }

    #[tokio::test]
    async fn handler_list_and_prove_ignore_stale_runtime_lane_geometry() {
        let mut app = crate::mk_app_state_for_tests();
        enable_nexus_with_lane_ids(&mut app, &[0, 1]);
        let stale = DaPinIntentWithLocation {
            intent: sample_intent(1, 4, 8),
            location: DaCommitmentLocation {
                block_height: 9,
                index_in_bundle: 1,
            },
        };
        seed_pin_store(
            &mut app,
            DaPinStore::from_intents(std::slice::from_ref(&stale)),
        );
        install_stale_runtime_lane_geometry(&app, stale.intent.lane_id);

        let JsonBody(items) = super::handler_list_pin_intents(
            State(app.clone()),
            NoritoJson(DaPinIntentQueryRequest::default()),
        )
        .await
        .expect("pin intent list should succeed");
        assert!(
            items.is_empty(),
            "stale runtime-only lane pin intents must not be listed"
        );

        let JsonBody(proof) = super::handler_prove_pin_intent(
            State(app),
            NoritoJson(DaPinIntentQueryRequest {
                storage_ticket: Some(stale.intent.storage_ticket),
                ..DaPinIntentQueryRequest::default()
            }),
        )
        .await
        .expect("pin intent proof lookup should succeed");
        assert!(
            proof.is_none(),
            "stale runtime-only lane pin intents must not produce proofs"
        );
    }
}
