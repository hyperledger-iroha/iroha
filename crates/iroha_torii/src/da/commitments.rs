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

const ENDPOINT_DA_COMMITMENTS: &str = "/v1/da/commitments";
const ENDPOINT_DA_COMMITMENTS_PROVE: &str = "/v1/da/commitments/prove";
const ENDPOINT_DA_COMMITMENTS_VERIFY: &str = "/v1/da/commitments/verify";
const ENDPOINT_DA_PROOF_POLICIES: &str = "/v1/da/proof_policies";
const ENDPOINT_DA_PROOF_POLICY_SNAPSHOT: &str = "/v1/da/proof_policy_snapshot";

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

/// Verification response for a DA commitment Merkle proof.
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

/// HTTP handler for `/v1/da/commitments`.
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

/// HTTP handler for `/v1/da/commitments/prove`.
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

/// HTTP handler for `/v1/da/commitments/verify`.
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

/// HTTP handler for `/v1/da/proof_policies`.
pub async fn handler_list_proof_policies(
    State(app): State<SharedAppState>,
) -> Result<JsonBody<DaProofPolicyBundle>, Error> {
    let nexus = app.state.nexus_snapshot();
    crate::ensure_nexus_lanes_enabled(nexus.enabled, ENDPOINT_DA_PROOF_POLICIES)?;
    let policies = proof_policy_bundle(&nexus.lane_config);
    Ok(JsonBody(policies))
}

/// HTTP handler for `/v1/da/proof_policy_snapshot`.
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
    if request_targets_commitment(request) {
        return Vec::new();
    }
    if offset >= store.len() {
        return Vec::new();
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
        let target = store.get_by_manifest(&manifest)?.clone();
        return request_matches_commitment(&target, request).then_some(target);
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

fn request_targets_commitment(request: &DaCommitmentProofRequest) -> bool {
    request.manifest_hash.is_some()
        || request.lane_id.is_some()
        || request.epoch.is_some()
        || request.sequence.is_some()
}

fn request_matches_commitment(
    target: &DaCommitmentWithLocation,
    request: &DaCommitmentProofRequest,
) -> bool {
    if request
        .lane_id
        .is_some_and(|lane_id| target.commitment.lane_id.as_u32() != lane_id)
    {
        return false;
    }
    if request
        .epoch
        .is_some_and(|epoch| target.commitment.epoch != epoch)
    {
        return false;
    }
    if request
        .sequence
        .is_some_and(|sequence| target.commitment.sequence != sequence)
    {
        return false;
    }
    true
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

#[cfg(all(test, feature = "app_api"))]
mod tests {
    use std::{num::NonZeroU32, sync::Arc};

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::{
        block::{BlockHeader, builder::BlockBuilder},
        da::{
            commitment::{DaCommitmentBundle, DaCommitmentRecord, DaProofScheme, RetentionClass},
            types::{BlobDigest, StorageTicketId},
        },
        nexus::{DataSpaceId, LaneCatalog, LaneConfig as ModelLaneConfig, LaneId},
    };

    use super::*;
    use crate::{NoritoJson, mk_app_state_for_tests};

    fn checked_random_keypair_with_algorithm(algorithm: Algorithm, context: &str) -> KeyPair {
        KeyPair::try_random_with_algorithm(algorithm).unwrap_or_else(|err| {
            panic!("{context}: checked random {algorithm:?} key generation failed: {err}")
        })
    }

    fn sample_record(lane: u32, epoch: u64, sequence: u64) -> DaCommitmentRecord {
        let mut storage_ticket = [0x22; 32];
        storage_ticket[..4].copy_from_slice(&lane.to_be_bytes());
        storage_ticket[4..12].copy_from_slice(&epoch.to_be_bytes());
        storage_ticket[12..20].copy_from_slice(&sequence.to_be_bytes());

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
            StorageTicketId::new(storage_ticket),
            Signature::from_bytes(&[0x33; 64]),
        )
    }

    fn lane_config_with_entries(entries: &[(LaneId, DaProofScheme)]) -> ConfigLaneConfig {
        ConfigLaneConfig::from_catalog(&lane_catalog_with_entries(entries))
    }

    fn lane_catalog_with_entries(entries: &[(LaneId, DaProofScheme)]) -> LaneCatalog {
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
                dataspace_id: DataSpaceId::UNIVERSAL,
                alias: format!("lane-{}", lane_id.as_u32()),
                proof_scheme: *scheme,
                ..ModelLaneConfig::default()
            })
            .collect();
        LaneCatalog::new(lane_count, lanes).expect("lane catalog")
    }

    fn store_with_records() -> DaCommitmentStore {
        let records = vec![
            sample_record(1, 1, 1),
            sample_record(1, 2, 0),
            sample_record(2, 3, 5),
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

    fn app_with_da_commitment_bundle(records: Vec<DaCommitmentRecord>) -> crate::SharedAppState {
        let mut app = mk_app_state_for_tests();
        enable_nexus(&mut app);

        let mut lane_entries: Vec<_> = records
            .iter()
            .map(|record| (record.lane_id, record.proof_scheme))
            .collect();
        lane_entries.sort_by_key(|(lane_id, _)| lane_id.as_u32());
        lane_entries.dedup_by_key(|(lane_id, _)| lane_id.as_u32());
        {
            let app = Arc::get_mut(&mut app).expect("unique app state");
            let state = Arc::get_mut(&mut app.state).expect("unique core state");
            let mut nexus_cfg = state.nexus_snapshot();
            nexus_cfg.lane_catalog = lane_catalog_with_entries(&lane_entries);
            state
                .set_nexus(nexus_cfg)
                .expect("seed Nexus DA lane policy for tests");
        }

        let bundle = DaCommitmentBundle::new(records);
        let bundle_for_store = bundle.clone();
        let keypair = checked_random_keypair_with_algorithm(
            Algorithm::BlsNormal,
            "DA commitment block fixture",
        );
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut builder = BlockBuilder::new(header);
        builder.set_da_commitments(Some(bundle));
        let block = builder.build_with_signature(0, keypair.private_key());
        let header = block.header();
        let block_height = header.height().get();
        let block_hash = block.hash();

        app.kura
            .store_block(Arc::new(block))
            .expect("store DA commitment block");
        let mut block_hashes = app.state.block_hashes.block();
        block_hashes.push_for_tests(block_hash);
        block_hashes.commit_for_tests();
        app.state.update_latest_block_header_cache_for_tests(header);
        drop(app.state.da_commitments());
        {
            let mut store = app.state.da_commitments.write();
            store.insert_bundle(block_height, bundle_for_store);
            assert!(
                store.bundle_at(block_height).is_some(),
                "DA commitment fixture must seed handler store"
            );
        }
        app
    }

    async fn prove_for_manifest(
        app: crate::SharedAppState,
        manifest: ManifestDigest,
    ) -> DaCommitmentProof {
        let JsonBody(response) = super::handler_prove_commitment(
            State(app),
            NoritoJson(DaCommitmentProofRequest {
                manifest_hash: Some(manifest),
                ..DaCommitmentProofRequest::default()
            }),
        )
        .await
        .expect("proof handler should succeed");
        response.expect("proof should be returned").proof
    }

    async fn verify_invalid(
        app: crate::SharedAppState,
        proof: DaCommitmentProof,
        expected_error: &str,
    ) {
        let JsonBody(verification) =
            super::handler_verify_commitment(State(app), NoritoJson(proof))
                .await
                .expect("verify handler should succeed");
        assert!(!verification.valid);
        assert!(
            verification
                .error
                .as_deref()
                .is_some_and(|message| message.contains(expected_error)),
            "unexpected verification error: {verification:?}"
        );
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
    fn list_manifest_filter_rejects_conflicting_lane_tuple() {
        let store = store_with_records();
        let manifest = ManifestDigest::new([2; 32]);
        let request = DaCommitmentProofRequest {
            manifest_hash: Some(manifest),
            lane_id: Some(2),
            epoch: Some(1),
            sequence: Some(5),
            ..DaCommitmentProofRequest::default()
        };

        let items = list_from_store(&store, &request);
        assert!(items.is_empty());
    }

    #[test]
    fn list_partial_tuple_filter_does_not_fall_back_to_full_list() {
        let store = store_with_records();
        let request = DaCommitmentProofRequest {
            lane_id: Some(1),
            ..DaCommitmentProofRequest::default()
        };

        let items = list_from_store(&store, &request);
        assert!(items.is_empty());
    }

    #[test]
    fn list_targeted_record_respects_offset_as_empty_page() {
        let store = store_with_records();
        let request = DaCommitmentProofRequest {
            manifest_hash: Some(ManifestDigest::new([2; 32])),
            pagination: Some(pagination(Some(1), 1)),
            ..DaCommitmentProofRequest::default()
        };

        let items = list_from_store(&store, &request);
        assert!(items.is_empty());
    }

    #[test]
    fn list_over_offset_returns_empty_page() {
        let store = store_with_records();
        let request = DaCommitmentProofRequest {
            pagination: Some(pagination(Some(1), u64::MAX)),
            ..DaCommitmentProofRequest::default()
        };

        let items = list_from_store(&store, &request);
        assert!(items.is_empty());
    }

    #[test]
    fn prove_builds_merkle_proof() {
        let store = store_with_records();
        let request = DaCommitmentProofRequest {
            lane_id: Some(2),
            epoch: Some(3),
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

    #[test]
    fn prove_builds_merkle_proof_when_stale_duplicate_is_filtered_from_index() {
        let mut store = DaCommitmentStore::default();
        let first = sample_record(1, 1, 1);
        let mut stale_duplicate = first.clone();
        stale_duplicate.manifest_hash = ManifestDigest::new([0x55; 32]);
        stale_duplicate.storage_ticket = StorageTicketId::new([0x66; 32]);
        let later = sample_record(2, 3, 4);

        store.insert_bundle(7, DaCommitmentBundle::new(vec![first]));
        store.insert_bundle(
            8,
            DaCommitmentBundle::new(vec![stale_duplicate.clone(), later.clone()]),
        );

        let request = DaCommitmentProofRequest {
            lane_id: Some(2),
            epoch: Some(3),
            sequence: Some(4),
            ..DaCommitmentProofRequest::default()
        };
        let proof = build_proof_from_store(&store, &request)
            .expect("proof should use the raw committed bundle");
        assert_eq!(proof.location.block_height, 8);
        assert_eq!(proof.location.index_in_bundle, 1);
        assert_eq!(proof.commitment, later);

        let bundle = store
            .bundle_at(proof.location.block_height)
            .expect("committed bundle present");
        assert_eq!(bundle.commitments.as_slice(), &[stale_duplicate, later]);
        let mut header = BlockHeader::new(
            NonZeroU64::new(proof.location.block_height).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        header.set_da_commitments_hash(Some(bundle.canonical_hash()));
        let config = lane_config_with_entries(&[(LaneId::new(2), DaProofScheme::MerkleSha256)]);

        assert!(verify_da_commitment_proof(&proof, bundle, &header, &config).is_ok());
    }

    #[test]
    fn prove_returns_none_for_absent_and_partial_lookup_keys() {
        let store = store_with_records();
        let absent_manifest = DaCommitmentProofRequest {
            manifest_hash: Some(ManifestDigest::new([0x99; 32])),
            ..DaCommitmentProofRequest::default()
        };
        assert!(build_proof_from_store(&store, &absent_manifest).is_none());

        let partial_lane_tuple = DaCommitmentProofRequest {
            lane_id: Some(1),
            epoch: Some(1),
            sequence: None,
            ..DaCommitmentProofRequest::default()
        };
        assert!(build_proof_from_store(&store, &partial_lane_tuple).is_none());

        let wrong_sequence = DaCommitmentProofRequest {
            lane_id: Some(1),
            epoch: Some(1),
            sequence: Some(999),
            ..DaCommitmentProofRequest::default()
        };
        assert!(build_proof_from_store(&store, &wrong_sequence).is_none());
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
    async fn prove_and_verify_handlers_roundtrip_merkle_proof_from_committed_block() {
        let records = vec![
            sample_record(1, 1, 1),
            sample_record(1, 2, 0),
            sample_record(2, 3, 5),
        ];
        let manifest = records[2].manifest_hash;
        let app = app_with_da_commitment_bundle(records);

        let JsonBody(response) = super::handler_prove_commitment(
            State(app.clone()),
            NoritoJson(DaCommitmentProofRequest {
                manifest_hash: Some(manifest),
                ..DaCommitmentProofRequest::default()
            }),
        )
        .await
        .expect("proof handler should succeed");
        let proof_response = response.expect("proof should be returned for committed bundle");
        assert_eq!(proof_response.proof.location.block_height, 1);
        assert_eq!(proof_response.proof.location.index_in_bundle, 2);
        assert!(
            !proof_response.proof.path.is_empty(),
            "multi-record bundle should return a non-empty Merkle path"
        );

        let JsonBody(verification) =
            super::handler_verify_commitment(State(app), NoritoJson(proof_response.proof))
                .await
                .expect("verify handler should succeed");
        assert!(verification.valid, "proof should verify: {verification:?}");
        assert!(verification.error.is_none());
    }

    #[tokio::test]
    async fn prove_handler_returns_none_for_unknown_commitment_keys() {
        let records = vec![sample_record(1, 1, 1), sample_record(1, 2, 0)];
        let app = app_with_da_commitment_bundle(records);

        let JsonBody(response) = super::handler_prove_commitment(
            State(app.clone()),
            NoritoJson(DaCommitmentProofRequest {
                manifest_hash: Some(ManifestDigest::new([0xFA; 32])),
                ..DaCommitmentProofRequest::default()
            }),
        )
        .await
        .expect("proof handler should succeed");
        assert!(response.is_none());

        let JsonBody(response) = super::handler_prove_commitment(
            State(app),
            NoritoJson(DaCommitmentProofRequest {
                lane_id: Some(1),
                epoch: Some(1),
                sequence: Some(999),
                ..DaCommitmentProofRequest::default()
            }),
        )
        .await
        .expect("proof handler should succeed");
        assert!(response.is_none());
    }

    #[tokio::test]
    async fn prove_handler_returns_none_for_conflicting_commitment_selectors() {
        let records = vec![sample_record(1, 1, 1), sample_record(2, 2, 2)];
        let manifest = records[0].manifest_hash;
        let app = app_with_da_commitment_bundle(records);

        let JsonBody(response) = super::handler_prove_commitment(
            State(app.clone()),
            NoritoJson(DaCommitmentProofRequest {
                manifest_hash: Some(manifest),
                lane_id: Some(2),
                epoch: Some(1),
                sequence: Some(1),
                ..DaCommitmentProofRequest::default()
            }),
        )
        .await
        .expect("proof handler should succeed");
        assert!(
            response.is_none(),
            "manifest must not override a conflicting lane selector"
        );

        let JsonBody(response) = super::handler_prove_commitment(
            State(app),
            NoritoJson(DaCommitmentProofRequest {
                manifest_hash: Some(manifest),
                lane_id: Some(1),
                epoch: Some(9),
                sequence: Some(1),
                ..DaCommitmentProofRequest::default()
            }),
        )
        .await
        .expect("proof handler should succeed");
        assert!(
            response.is_none(),
            "manifest must not override a conflicting epoch selector"
        );
    }

    #[tokio::test]
    async fn verify_handler_rejects_tampered_merkle_root() {
        let records = vec![sample_record(1, 1, 1), sample_record(1, 2, 2)];
        let manifest = records[1].manifest_hash;
        let app = app_with_da_commitment_bundle(records);

        let JsonBody(response) = super::handler_prove_commitment(
            State(app.clone()),
            NoritoJson(DaCommitmentProofRequest {
                manifest_hash: Some(manifest),
                ..DaCommitmentProofRequest::default()
            }),
        )
        .await
        .expect("proof handler should succeed");
        let mut proof = response.expect("proof should be returned").proof;
        proof.root = Hash::prehashed([0xEE; 32]);

        let JsonBody(verification) =
            super::handler_verify_commitment(State(app), NoritoJson(proof))
                .await
                .expect("verify handler should succeed");
        assert!(!verification.valid);
        assert!(
            verification
                .error
                .as_deref()
                .is_some_and(|message| message.contains("Merkle path")),
            "unexpected verification error: {verification:?}"
        );
    }

    #[tokio::test]
    async fn verify_handler_rejects_missing_bundle_reference() {
        let records = vec![sample_record(1, 1, 1), sample_record(1, 2, 2)];
        let manifest = records[1].manifest_hash;
        let app = app_with_da_commitment_bundle(records);
        let mut proof = prove_for_manifest(app.clone(), manifest).await;
        proof.location.block_height = 2;

        verify_invalid(app, proof, "no DA commitment bundle stored for block 2").await;
    }

    #[tokio::test]
    async fn verify_handler_rejects_bundle_without_backing_kura_block() {
        let app = app_with_da_commitment_bundle(vec![sample_record(1, 1, 1)]);
        let missing_block_bundle =
            DaCommitmentBundle::new(vec![sample_record(1, 2, 2), sample_record(1, 3, 3)]);
        let proof = build_da_commitment_proof(&missing_block_bundle, 2, 1)
            .expect("proof for bundle without Kura block");
        app.state
            .da_commitments
            .write()
            .insert_bundle(2, missing_block_bundle);

        verify_invalid(app, proof, "block 2 not available in Kura").await;
    }

    #[tokio::test]
    async fn verify_handler_rejects_out_of_bounds_index() {
        let records = vec![sample_record(1, 1, 1), sample_record(1, 2, 2)];
        let manifest = records[1].manifest_hash;
        let app = app_with_da_commitment_bundle(records);
        let mut proof = prove_for_manifest(app.clone(), manifest).await;
        proof.location.index_in_bundle = u32::MAX;

        verify_invalid(app, proof, "out of bounds").await;
    }

    #[tokio::test]
    async fn verify_handler_rejects_bundle_len_mismatch() {
        let records = vec![sample_record(1, 1, 1), sample_record(1, 2, 2)];
        let manifest = records[1].manifest_hash;
        let app = app_with_da_commitment_bundle(records);
        let mut proof = prove_for_manifest(app.clone(), manifest).await;
        proof.bundle_len = proof.bundle_len.saturating_add(1);

        verify_invalid(app, proof, "bundle length").await;
    }

    #[tokio::test]
    async fn verify_handler_rejects_commitment_payload_mismatch() {
        let records = vec![sample_record(1, 1, 1), sample_record(1, 2, 2)];
        let manifest = records[1].manifest_hash;
        let app = app_with_da_commitment_bundle(records);
        let mut proof = prove_for_manifest(app.clone(), manifest).await;
        proof.commitment = sample_record(1, 9, 9);

        verify_invalid(app, proof, "commitment payload differs").await;
    }

    #[tokio::test]
    async fn verify_handler_rejects_tampered_bundle_hash() {
        let records = vec![sample_record(1, 1, 1), sample_record(1, 2, 2)];
        let manifest = records[1].manifest_hash;
        let app = app_with_da_commitment_bundle(records);
        let mut proof = prove_for_manifest(app.clone(), manifest).await;
        proof.bundle_hash =
            HashOf::<DaCommitmentBundle>::from_untyped_unchecked(Hash::prehashed([0xAB; 32]));

        verify_invalid(app, proof, "DA commitment bundle hash mismatch").await;
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
