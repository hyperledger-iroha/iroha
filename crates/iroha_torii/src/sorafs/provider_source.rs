//! Initial replication source transport authorized by the exact live native assignment.
use super::authority_auth::{AuthorityRequestRejection, require_controller_authority};
use crate::{SharedAppState, app_auth::VerifiedCanonicalRequest};
use axum::{
    body::Bytes,
    extract::{Extension, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use iroha_core::query::provider_ingest_source::authorize_provider_source_v1;
use iroha_core::state::{StateView, WorldReadOnly as _};
use iroha_data_model::sorafs::publication::SorafsAssignedSourceRequestV1;
use sorafs_car::publisher::{
    PROVIDER_SOURCE_RESPONSE_MAX_BYTES_V1, ProviderSourceResponseV1, PublisherSourceHeaderV1,
    PublisherSourceUploadV1,
};

fn now_secs() -> Result<u64, StatusCode> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}

/// Selection and exact native provider policy are read together, before and after source I/O.
fn authorize_source_at_cut(
    view: &StateView<'_>,
    verified: &VerifiedCanonicalRequest,
    request: &SorafsAssignedSourceRequestV1,
    now_secs: u64,
) -> Result<(), StatusCode> {
    require_controller_authority(view.world().runtime_selection(), verified).map_err(|error| {
        match error {
            AuthorityRequestRejection::UnsupportedSelection => StatusCode::SERVICE_UNAVAILABLE,
            AuthorityRequestRejection::DelegatedSigningKey => StatusCode::FORBIDDEN,
        }
    })?;
    authorize_provider_source_v1(view, &verified.account, request, now_secs)
        .map_err(|_| StatusCode::FORBIDDEN)
}

pub async fn read_source(
    State(state): State<SharedAppState>,
    Extension(verified): Extension<crate::app_auth::VerifiedCanonicalRequest>,
    body: Bytes,
) -> Response {
    let permit = match crate::acquire_query_admission(state.as_ref(), true).await {
        Ok(permit) => permit,
        Err(error) => return error.into_response(),
    };
    let result = crate::panic_recovery::join_recoverable(
        crate::panic_recovery::spawn_blocking_recoverable(move || {
            let _permit = permit;
            let request: SorafsAssignedSourceRequestV1 = norito::decode_canonical_with_limits(
                &body,
                norito::DecodeLimits::new(4096, 4096, 4096, 16384, 16),
            )
            .map_err(|_| StatusCode::BAD_REQUEST)?;
            if state
                .sorafs_node
                .capacity_provider_id()
                .map(|provider| *provider.as_bytes())
                != Some(request.source_provider)
            {
                return Err(StatusCode::FORBIDDEN);
            }
            authorize_source_at_cut(&state.state.view(), &verified, &request, now_secs()?)?;
            let storage = state
                .sorafs_node
                .storage()
                .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
            let id = hex::encode(request.manifest_digest);
            let response = if let Some(index) = request.chunk_index {
                let bytes = storage
                    .read_chunk_at(&id, index as usize)
                    .map_err(|error| match error {
                        sorafs_node::store::StorageError::ManifestNotFound { .. } => {
                            StatusCode::NOT_FOUND
                        }
                        _ => StatusCode::SERVICE_UNAVAILABLE,
                    })?
                    .ok_or(StatusCode::BAD_REQUEST)?;
                ProviderSourceResponseV1::Chunk(PublisherSourceUploadV1 { index, bytes })
            } else {
                let stored = storage.manifest(&id).ok_or(StatusCode::NOT_FOUND)?;
                if !stored.payload_available() {
                    return Err(StatusCode::SERVICE_UNAVAILABLE);
                }
                let manifest = stored
                    .load_manifest()
                    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
                let profile =
                    sorafs_manifest::validate_registered_chunker_profile(&manifest.chunking)
                        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
                        .profile;
                let plan = stored
                    .try_to_car_plan(profile)
                    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
                let header = PublisherSourceHeaderV1::new(
                    request.target_provider,
                    request.order_id,
                    request.assignment_revision,
                    &manifest,
                    &plan,
                )
                .map_err(|_| StatusCode::PAYLOAD_TOO_LARGE)?;
                if !stored.payload_available() {
                    return Err(StatusCode::SERVICE_UNAVAILABLE);
                }
                ProviderSourceResponseV1::Metadata(header)
            };
            authorize_source_at_cut(&state.state.view(), &verified, &request, now_secs()?)?;
            if norito::core::encoded_frame_len(&response)
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
                > PROVIDER_SOURCE_RESPONSE_MAX_BYTES_V1
            {
                return Err(StatusCode::PAYLOAD_TOO_LARGE);
            }
            norito::encode_canonical(&response).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
        }),
    )
    .await;
    match result {
        Ok(Ok(bytes)) => (
            [
                (axum::http::header::CONTENT_TYPE, "application/x-norito"),
                (axum::http::header::CACHE_CONTROL, "private, no-store"),
            ],
            bytes,
        )
            .into_response(),
        Ok(Err(status)) => (status, "native provider source rejected").into_response(),
        Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };

    #[test]
    fn provider_source_controller_authentication_does_not_replace_native_source_policy() {
        let signer = iroha_test_samples::ALICE_KEYPAIR.public_key().clone();
        let mut verified = VerifiedCanonicalRequest {
            account: iroha_test_samples::ALICE_ID.clone(),
            signer: signer.clone(),
            verified_signers: vec![signer],
            offchain_signing_key: false,
        };
        let request = SorafsAssignedSourceRequestV1 {
            target_provider: [1; 32],
            source_provider: [2; 32],
            order_id: [3; 32],
            assignment_revision: 1,
            manifest_digest: [4; 32],
            chunk_index: None,
            floor_height: 1,
            floor_block_hash: [5; 32],
        };
        for selected in [false, true] {
            let world = World::new();
            if selected {
                iroha_core::runtime::activate_additions_for_testing(
                    &world,
                    "iroha.sorafs.authority_governance.v1",
                )
                .unwrap();
            }
            let state = State::new_for_testing(
                world,
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            let view = state.view();
            verified.offchain_signing_key = false;
            require_controller_authority(view.world().runtime_selection(), &verified).unwrap();
            assert_eq!(
                authorize_source_at_cut(&view, &verified, &request, 1),
                Err(StatusCode::FORBIDDEN),
                "a controller request still needs native history, assignment, scoped permission and exact provider completion policy"
            );
            verified.offchain_signing_key = true;
            assert_eq!(
                authorize_source_at_cut(&view, &verified, &request, 1),
                Err(StatusCode::FORBIDDEN)
            );
        }
    }
}
