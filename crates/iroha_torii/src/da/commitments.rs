//! Torii handlers for DA commitments (DA-3).
//!
//! These endpoints operate on the in-memory commitment index populated during
//! block application. Durable WSV plumbing can replace the backing store once
//! available without changing the handler surface.

use std::num::{NonZeroU64, NonZeroUsize};

use axum::extract::State;
use iroha_config::parameters::actual::LaneConfig as ConfigLaneConfig;
use iroha_core::da::{
    build_da_commitment_proof, commitment_store::DaCommitmentStore, proof_policy_bundle,
    verify_da_commitment_proof,
};
use iroha_data_model::{
    da::commitment::{
        DaCommitmentProof, DaCommitmentWithLocation, DaProofPolicyBundle, DaProofScheme,
    },
    query::parameters::Pagination,
    sorafs::pin_registry::ManifestDigest,
};

use crate::{Error, JsonBody, NoritoJson, SharedAppState};

const ENDPOINT_DA_COMMITMENTS: &str = "/v2/da/commitments";
const ENDPOINT_DA_COMMITMENTS_PROVE: &str = "/v2/da/commitments/prove";
const ENDPOINT_DA_COMMITMENTS_VERIFY: &str = "/v2/da/commitments/verify";
const ENDPOINT_DA_PROOF_POLICIES: &str = "/v2/da/proof_policies";
const ENDPOINT_DA_PROOF_POLICY_SNAPSHOT: &str = "/v2/da/proof_policy_snapshot";

/// Request payload for DA commitment queries and proof generation.
#[derive(
    Debug,
    Default,
    Clone,
    crate::json_macros::JsonDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
)]
pub struct DaCommitmentProofRequest {
    #[norito(default)]
    pub manifest_hash: Option<ManifestDigest>,
    #[norito(default)]
    pub lane_id: Option<u32>,
    #[norito(default)]
    pub epoch: Option<u64>,
    #[norito(default)]
    pub sequence: Option<u64>,
    #[norito(default)]
    pub pagination: Option<Pagination>,
}

/// Response surface for DA commitment listings.
#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
)]
pub struct DaCommitmentListResponse {
    pub policies: DaProofPolicyBundle,
    pub commitments: Vec<DaCommitmentWithLocation>,
}

/// Response surface for DA commitment proofs.
#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoDeserialize,
    norito::derive::NoritoSerialize,
)]
pub struct DaCommitmentProofResponse {
    pub policies: DaProofPolicyBundle,
    pub proof: DaCommitmentProof,
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
pub struct DaCommitmentVerifyResponse {
    pub valid: bool,
    #[norito(default)]
    pub error: Option<String>,
}

/// HTTP handler for `/v2/da/commitments`.
pub async fn handler_list_commitments(
    State(app): State<SharedAppState>,
    NoritoJson(request): NoritoJson<DaCommitmentProofRequest>,
) -> Result<JsonBody<DaCommitmentListResponse>, Error> {
    let nexus = app.state.nexus_snapshot();
    crate::ensure_nexus_lanes_enabled(nexus.enabled, ENDPOINT_DA_COMMITMENTS)?;
    let store = app.state.da_commitments();
    let items = list_from_store(&store, &request);
    let policies = proof_policy_bundle(&nexus.lane_config);
    Ok(JsonBody(DaCommitmentListResponse {
        policies,
        commitments: items,
    }))
}

/// HTTP handler for `/v2/da/commitments/prove`.
pub async fn handler_prove_commitment(
    State(app): State<SharedAppState>,
    NoritoJson(request): NoritoJson<DaCommitmentProofRequest>,
) -> Result<JsonBody<Option<DaCommitmentProofResponse>>, Error> {
    let nexus = app.state.nexus_snapshot();
    crate::ensure_nexus_lanes_enabled(nexus.enabled, ENDPOINT_DA_COMMITMENTS_PROVE)?;
    let store = app.state.da_commitments();
    let proof = build_proof_from_store(&store, &request);
    proof.map_or_else(
        || Ok(JsonBody(None)),
        |proof| {
            let policies = proof_policy_bundle(&nexus.lane_config);
            Ok(JsonBody(Some(DaCommitmentProofResponse {
                policies,
                proof,
            })))
        },
    )
}

/// HTTP handler for `/v2/da/commitments/verify`.
pub async fn handler_verify_commitment(
    State(app): State<SharedAppState>,
    NoritoJson(proof): NoritoJson<DaCommitmentProof>,
) -> Result<JsonBody<DaCommitmentVerifyResponse>, Error> {
    let nexus = app.state.nexus_snapshot();
    crate::ensure_nexus_lanes_enabled(nexus.enabled, ENDPOINT_DA_COMMITMENTS_VERIFY)?;
    let response = verify_against_store(
        &app.state.da_commitments(),
        &nexus.lane_config,
        &proof,
        app.state.as_ref(),
    );
    Ok(JsonBody(response))
}

/// HTTP handler for `/v2/da/proof_policies`.
pub async fn handler_list_proof_policies(
    State(app): State<SharedAppState>,
) -> Result<JsonBody<DaProofPolicyBundle>, Error> {
    let nexus = app.state.nexus_snapshot();
    crate::ensure_nexus_lanes_enabled(nexus.enabled, ENDPOINT_DA_PROOF_POLICIES)?;
    let policies = proof_policy_bundle(&nexus.lane_config);
    Ok(JsonBody(policies))
}

/// HTTP handler for `/v2/da/proof_policy_snapshot`.
pub async fn handler_proof_policy_bundle(
    State(app): State<SharedAppState>,
) -> Result<JsonBody<DaProofPolicyBundle>, Error> {
    let nexus = app.state.nexus_snapshot();
    crate::ensure_nexus_lanes_enabled(nexus.enabled, ENDPOINT_DA_PROOF_POLICY_SNAPSHOT)?;
    let bundle = proof_policy_bundle(&nexus.lane_config);
    Ok(JsonBody(bundle))
}

fn list_from_store(
    store: &DaCommitmentStore,
    request: &DaCommitmentProofRequest,
) -> Vec<DaCommitmentWithLocation> {
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
        .enumerate()
        .skip(offset)
        .take(limit)
        .map(|(_, record)| record.clone())
        .collect()
}

fn find_in_store(
    store: &DaCommitmentStore,
    request: &DaCommitmentProofRequest,
) -> Option<DaCommitmentWithLocation> {
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

fn build_proof_from_store(
    store: &DaCommitmentStore,
    request: &DaCommitmentProofRequest,
) -> Option<DaCommitmentProof> {
    let target = find_in_store(store, request)?;
    let bundle = store.bundle_at(target.location.block_height)?;
    let index = usize::try_from(target.location.index_in_bundle).ok()?;
    build_da_commitment_proof(bundle, target.location.block_height, index)
}

fn verify_against_store(
    store: &DaCommitmentStore,
    lane_config: &ConfigLaneConfig,
    proof: &DaCommitmentProof,
    state: &iroha_core::state::State,
) -> DaCommitmentVerifyResponse {
    let Some(bundle) = store.bundle_at(proof.location.block_height) else {
        return DaCommitmentVerifyResponse {
            valid: false,
            error: Some(format!(
                "no DA commitment bundle stored for block {}",
                proof.location.block_height
            )),
        };
    };

    let Ok(block_height) = usize::try_from(proof.location.block_height) else {
        return DaCommitmentVerifyResponse {
            valid: false,
            error: Some(format!(
                "block height {} does not fit into usize for lookup",
                proof.location.block_height
            )),
        };
    };
    let Some(nonzero_height) = NonZeroUsize::new(block_height) else {
        return DaCommitmentVerifyResponse {
            valid: false,
            error: Some("proof references block height 0".to_string()),
        };
    };
    let Some(block) = state.block_by_height(nonzero_height) else {
        return DaCommitmentVerifyResponse {
            valid: false,
            error: Some(format!(
                "block {} not available in Kura",
                proof.location.block_height
            )),
        };
    };

    let header = block.header();
    match verify_da_commitment_proof(proof, bundle, &header, lane_config) {
        Ok(()) => DaCommitmentVerifyResponse {
            valid: true,
            error: None,
        },
        Err(err) => DaCommitmentVerifyResponse {
            valid: false,
            error: Some(err.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU32, sync::Arc};

    use iroha_crypto::{Hash, Signature};
    use iroha_data_model::{
        block::BlockHeader,
        da::{
            commitment::{DaCommitmentRecord, DaProofScheme, RetentionClass},
            types::{BlobDigest, StorageTicketId},
        },
        nexus::{DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId},
    };

    use super::*;
    use crate::{NoritoJson, mk_app_state_for_tests};

    fn sample_record(lane: u32, epoch: u64, sequence: u64) -> DaCommitmentRecord {
        DaCommitmentRecord::new(
            LaneId::new(lane),
            epoch,
            sequence,
            BlobDigest::new([lane as u8; 32]),
            ManifestDigest::new([epoch as u8; 32]),
            DaProofScheme::MerkleSha256,
            Hash::prehashed([sequence as u8; 32]),
            None,
            None,
            RetentionClass::default(),
            StorageTicketId::new([0x22; 32]),
            Signature::from_bytes(&[0x33; 64]),
        )
    }

    fn lane_config_with_entries(entries: &[(LaneId, DaProofScheme)]) -> ConfigLaneConfig {
        let max_lane = entries
            .iter()
            .map(|(lane, _)| lane.as_u32())
            .max()
            .unwrap_or(0);
        let lane_count = NonZeroU32::new(max_lane.saturating_add(1)).expect("lane count");
        let lanes: Vec<ModelLaneConfig> = entries
            .iter()
            .map(|(lane_id, scheme)| ModelLaneConfig {
                id: *lane_id,
                dataspace_id: DataSpaceId::new(u64::from(lane_id.as_u32())),
                alias: format!("lane-{}", lane_id.as_u32()),
                proof_scheme: *scheme,
                ..ModelLaneConfig::default()
            })
            .collect();
        let catalog = LaneCatalog::new(lane_count, lanes).expect("lane catalog");
        ConfigLaneConfig::from_catalog(&catalog)
    }

    fn store_with_records() -> DaCommitmentStore {
        let records = vec![
            sample_record(1, 1, 1),
            sample_record(1, 2, 0),
            sample_record(2, 1, 5),
        ];
        DaCommitmentStore::from_bundle_at_height(&records, 9)
    }

    fn pagination(limit: Option<u64>, offset: u64) -> Pagination {
        Pagination::new(limit.and_then(NonZeroU64::new), offset)
    }

    fn enable_nexus(app: &mut crate::SharedAppState) {
        let app = Arc::get_mut(app).expect("unique app state");
        let state = Arc::get_mut(&mut app.state).expect("unique core state");
        let mut nexus_cfg = state.nexus_snapshot();
        nexus_cfg.enabled = true;
        state
            .set_nexus(nexus_cfg)
            .expect("enable Nexus lane catalog for tests");
    }

    #[test]
    fn list_respects_pagination() {
        let store = store_with_records();
        let request = DaCommitmentProofRequest {
            pagination: Some(pagination(Some(2), 1)),
            ..DaCommitmentProofRequest::default()
        };

        let items = list_from_store(&store, &request);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].location.index_in_bundle, 1);
        assert_eq!(items[1].location.index_in_bundle, 2);
        assert_eq!(items[0].location.block_height, 9);
    }

    #[test]
    fn list_with_manifest_filters_correct_record() {
        let store = store_with_records();
        let manifest = ManifestDigest::new([2; 32]);
        let request = DaCommitmentProofRequest {
            manifest_hash: Some(manifest),
            ..DaCommitmentProofRequest::default()
        };

        let items = list_from_store(&store, &request);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].commitment.manifest_hash, manifest);
        assert_eq!(items[0].location.index_in_bundle, 1);
    }

    #[test]
    fn prove_builds_merkle_proof() {
        let store = store_with_records();
        let request = DaCommitmentProofRequest {
            lane_id: Some(2),
            epoch: Some(1),
            sequence: Some(5),
            ..DaCommitmentProofRequest::default()
        };

        let proof = build_proof_from_store(&store, &request).expect("proof should exist");
        let bundle = store
            .bundle_at(proof.location.block_height)
            .expect("bundle present");
        let mut header = BlockHeader::new(
            NonZeroU64::new(proof.location.block_height).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        header.set_da_commitments_hash(Some(bundle.canonical_hash()));

        let config = lane_config_with_entries(&[
            (LaneId::new(1), DaProofScheme::MerkleSha256),
            (LaneId::new(2), DaProofScheme::MerkleSha256),
        ]);

        assert!(verify_da_commitment_proof(&proof, bundle, &header, &config).is_ok());
    }

    #[tokio::test]
    async fn list_handler_includes_policy_bundle() {
        let mut app = mk_app_state_for_tests();
        enable_nexus(&mut app);
        let request = DaCommitmentProofRequest::default();
        let JsonBody(_response) =
            super::handler_list_commitments(State(app.clone()), NoritoJson(request))
                .await
                .expect("handler should succeed");
        let expected = proof_policy_bundle(&app.state.nexus_snapshot().lane_config);

        assert_eq!(expected.version, DaProofPolicyBundle::VERSION_V1);
    }

    #[tokio::test]
    async fn proof_policy_handler_reports_lane_metadata() {
        let mut app = mk_app_state_for_tests();
        enable_nexus(&mut app);
        let JsonBody(bundle) = super::handler_list_proof_policies(State(app.clone()))
            .await
            .expect("handler should succeed");

        assert_eq!(bundle.version, DaProofPolicyBundle::VERSION_V1);
        assert!(
            !bundle.policies.is_empty(),
            "expected policies derived from lane configuration"
        );
        let nexus_snapshot = app.state.nexus_snapshot();
        let primary = nexus_snapshot.lane_config.primary();
        let first = &bundle.policies[0];
        assert_eq!(first.lane_id, primary.lane_id);
        assert_eq!(first.dataspace_id, primary.dataspace_id);
        assert_eq!(first.alias, primary.alias);
        assert_eq!(first.proof_scheme, primary.proof_scheme);
    }

    #[tokio::test]
    async fn proof_policy_bundle_handler_exposes_hash() {
        let mut app = mk_app_state_for_tests();
        enable_nexus(&mut app);
        let JsonBody(bundle) = super::handler_proof_policy_bundle(State(app.clone()))
            .await
            .expect("handler should succeed");

        assert!(
            !bundle.policies.is_empty(),
            "expected proof policies in bundle response"
        );
        assert_eq!(bundle.version, DaProofPolicyBundle::VERSION_V1);
        assert_ne!(bundle.policy_hash, Hash::prehashed([0; 32]));
    }

    #[tokio::test]
    async fn handlers_reject_when_nexus_disabled() {
        let app = mk_app_state_for_tests();
        let err = super::handler_list_commitments(
            State(app),
            NoritoJson(DaCommitmentProofRequest::default()),
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
}
