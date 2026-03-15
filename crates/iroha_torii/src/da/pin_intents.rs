//! Torii handlers for DA pin intents.
//!
//! These endpoints operate on the in-memory pin intent index populated during
//! block application. Durable WSV plumbing can replace the backing store once
//! available without changing the handler surface.

use std::num::NonZeroU64;

use axum::extract::State;
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

/// Stateless verification response placeholder.
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
    let items = list_from_store(&store, &request);
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
    let proof = find_in_store(&store, &request);
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
    let valid = verify_against_store(&store, &proof);
    Ok(JsonBody(DaPinIntentVerifyResponse { valid }))
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
        .unwrap_or(usize::MAX);
    let offset = usize::try_from(pagination.offset).unwrap_or(usize::MAX);

    if let Some(target) = find_in_store(store, request) {
        return if offset == 0 && limit > 0 {
            vec![target]
        } else {
            Vec::new()
        };
    }

    store
        .all_sorted()
        .skip(offset)
        .take(limit)
        .cloned()
        .collect()
}

fn find_in_store(
    store: &DaPinStore,
    request: &DaPinIntentQueryRequest,
) -> Option<DaPinIntentWithLocation> {
    if let Some(ticket) = request.storage_ticket {
        return store.get_by_ticket(&ticket).cloned();
    }

    if let Some(alias) = &request.alias {
        return store.get_by_alias(alias).map(|(_, record)| record.clone());
    }

    if let Some(manifest) = request.manifest_hash {
        return store.get_by_manifest(&manifest).cloned();
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

fn verify_against_store(store: &DaPinStore, proof: &DaPinIntentWithLocation) -> bool {
    let ticket = &proof.intent.storage_ticket;
    store
        .get_by_ticket(ticket)
        .map(|stored| stored.intent == proof.intent && stored.location == proof.location)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use iroha_data_model::{
        da::{commitment::DaCommitmentLocation, pin_intent::DaPinIntent, types::StorageTicketId},
        nexus::LaneId,
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

    fn enable_nexus(app: &mut crate::SharedAppState) {
        let app = std::sync::Arc::get_mut(app).expect("unique app state");
        let state = std::sync::Arc::get_mut(&mut app.state).expect("unique core state");
        let mut nexus_cfg = state.nexus_snapshot();
        nexus_cfg.enabled = true;
        state
            .set_nexus(nexus_cfg)
            .expect("enable Nexus lane catalog for tests");
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
    fn verify_rejects_mismatched_ticket() {
        let store = store_with_records();
        let mut proof = find_in_store(
            &store,
            &DaPinIntentQueryRequest {
                manifest_hash: Some(ManifestDigest::new([1; 32])),
                ..DaPinIntentQueryRequest::default()
            },
        )
        .expect("proof should exist");

        proof.intent.storage_ticket = StorageTicketId::new([9; 32]);
        assert!(!verify_against_store(&store, &proof));
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
}
