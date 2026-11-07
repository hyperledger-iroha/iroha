//! Publisher-authenticated initial source staging under exact native finalized assignments.

use crate::SharedAppState;
use iroha_allocation::{AllocationBudget, AllocationReservation};
use iroha_crypto::HashOf;
use iroha_data_model::{
    account::AccountId,
    sumeragi_finality::{
        CertifiedResultScope, CertifiedResultV1, ScheduleOutcome, SumeragiCommitCertificateV1,
    },
    transaction::TransactionEntrypoint,
};
use std::{num::NonZeroU64, time::Instant};
use axum::{
    body::Bytes,
    extract::{Extension, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use iroha_core::query::provider_ingest_source::{
    PublisherSourceBindingV1, authorize_publisher_source_v1,
};
use iroha_core::state::WorldReadOnly;
use iroha_data_model::sorafs::{
    capacity::ProviderId,
    pin_registry::{ManifestDigest, ReplicationOrderId},
};
use mv::storage::StorageReadOnly;
use sorafs_car::publisher::PublisherSourceRequestV1;

fn publication_now() -> Result<u64, StatusCode> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)
}

/// Stage one authenticated metadata reservation or one chunk; never admit or complete a pin.
pub async fn stage_source(
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
            let request =
                PublisherSourceRequestV1::decode(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
            let (binding, manifest) = match &request {
                PublisherSourceRequestV1::Metadata(header) => {
                    let (manifest, _) = header.verify().map_err(|_| StatusCode::BAD_REQUEST)?;
                    let binding = PublisherSourceBindingV1 {
                        provider_id: ProviderId::new(header.provider_id),
                        order_id: ReplicationOrderId::new(header.order_id),
                        assignment_revision: header.assignment_revision,
                        manifest_digest: ManifestDigest::from_manifest(&manifest)
                            .map_err(|_| StatusCode::BAD_REQUEST)?,
                    };
                    (binding, Some(manifest))
                }
                PublisherSourceRequestV1::Chunk(chunk) => (
                    PublisherSourceBindingV1 {
                        provider_id: ProviderId::new(chunk.provider_id),
                        order_id: ReplicationOrderId::new(chunk.order_id),
                        assignment_revision: chunk.assignment_revision,
                        manifest_digest: ManifestDigest::new(chunk.manifest_digest),
                    },
                    None,
                ),
            };
            if state.sorafs_node.capacity_provider_id() != Some(binding.provider_id) {
                return Err(StatusCode::CONFLICT);
            }
            let authorize = || {
                let view = state.state.view();
                let authority = authorize_publisher_source_v1(
                    &view,
                    &verified.account,
                    &binding,
                    publication_now()?,
                )
                .map_err(|_| StatusCode::FORBIDDEN)?;
                if let Some(manifest) = &manifest {
                    let pin = view
                        .world()
                        .pin_manifests()
                        .get(&binding.manifest_digest)
                        .ok_or(StatusCode::NOT_FOUND)?;
                    if pin.root_cid.as_bytes().as_slice() != manifest.root_cid.as_slice()
                        || pin.chunk_digest_sha3_256 != manifest.chunk_digest_sha3_256
                        || pin.por_root != manifest.por_root
                        || pin.content_length != manifest.content_length
                    {
                        return Err(StatusCode::FORBIDDEN);
                    }
                }
                Ok::<_, StatusCode>(authority)
            };
            let authority = authorize()?;
            let storage = state
                .sorafs_node
                .storage()
                .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
            match &request {
                PublisherSourceRequestV1::Metadata(header) => {
                    storage
                        .stage_publisher_source(
                            header,
                            authority.deadline_epoch,
                            authority.finalized_epoch,
                        )
                        .map_err(|_| StatusCode::CONFLICT)?;
                }
                PublisherSourceRequestV1::Chunk(chunk) => {
                    storage
                        .stage_publisher_source_chunk(chunk, authority.finalized_epoch)
                        .map_err(|_| StatusCode::CONFLICT)?;
                }
            }
            authorize()?;
            Ok::<_, StatusCode>(())
        }),
    )
    .await;
    match result {
        Ok(Ok(())) => StatusCode::NO_CONTENT.into_response(),
        Ok(Err(status)) => (status, "publisher source staging rejected").into_response(),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "publisher source staging unavailable",
        )
            .into_response(),
    }
}

fn canonical_response<T: norito::NoritoSerialize + norito::NoritoSchema>(
    value: &T,
    maximum: usize,
) -> Result<Response, StatusCode> {
    if norito::core::encoded_frame_len(value).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
        > maximum
    {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    let bytes = norito::to_bytes(value).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    if bytes.len() > maximum {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "application/x-norito"),
            (axum::http::header::CACHE_CONTROL, "private, no-store"),
        ],
        bytes,
    )
        .into_response())
}

/// Return informational exact native rows; the client must challenge them through the assertion.
pub async fn prepare_publication(
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
            let digest: ManifestDigest = norito::decode_canonical_with_limits(
                &body,
                norito::DecodeLimits::new(4096, 4096, 4096, 16384, 16),
            )
            .map_err(|_| StatusCode::BAD_REQUEST)?;
            let view = state.state.view();
            let hash = view
                .latest_block_hash()
                .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
            iroha_core::query::signer_finality::verify_signer_finality_v1(
                &view,
                view.height() as u64,
                *hash.as_ref(),
            )
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
            let pin = view
                .world()
                .pin_manifests()
                .get(&digest)
                .ok_or(StatusCode::NOT_FOUND)?;
            if pin.submitted_by != verified.account {
                return Err(StatusCode::FORBIDDEN);
            }
            let order_id =
                iroha_data_model::sorafs::pin_registry::derive_sorafs_auto_replication_order_id_v1(
                    &digest,
                );
            let order = view
                .world()
                .replication_orders()
                .get(&order_id)
                .ok_or(StatusCode::ACCEPTED)?;
            canonical_response(
                &iroha_data_model::sorafs::publication::SorafsPublicationPreparationV1 {
                    pin: pin.clone(),
                    order: order.clone(),
                },
                2 * 1024 * 1024,
            )
        }),
    )
    .await;
    match result {
        Ok(Ok(response)) => response,
        Ok(Err(status)) => status.into_response(),
        Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

/// Return compact native execution evidence of the requester's own publication assertion.
///
/// The response carries quorum certificates and Merkle paths, never a block body, so any
/// account can publish and verify without a ledger-wide read permission. Only the
/// assertion's signer receives its inclusion; any other account is refused before a path is
/// built or an output row is copied.
pub async fn publication_proof(
    State(state): State<SharedAppState>,
    Extension(verified): Extension<crate::app_auth::VerifiedCanonicalRequest>,
    body: Bytes,
) -> Response {
    use iroha_core::state::{StateReadOnlyWithTransactions, TransactionsReadOnly};
    use iroha_data_model::sorafs::publication::{
        PUBLICATION_PROOF_MAX_BYTES_V1, SorafsPublicationProofRequestV1,
    };

    let permit = match crate::acquire_query_admission(state.as_ref(), true).await {
        Ok(permit) => permit,
        Err(error) => return error.into_response(),
    };
    let timeout = state.proof_limits.request_timeout;
    let result = crate::panic_recovery::join_recoverable(
        crate::panic_recovery::spawn_blocking_recoverable(move || {
            let _permit = permit;
            let request: SorafsPublicationProofRequestV1 = norito::decode_canonical_with_limits(
                &body,
                norito::DecodeLimits::new(4096, 4096, 4096, 16384, 16),
            )
            .map_err(|_| StatusCode::BAD_REQUEST)?;
            if request.floor_height == 0
                || request.floor_block_hash == [0; 32]
                || request.entry_hash == [0; 32]
            {
                return Err(StatusCode::BAD_REQUEST);
            }
            let entry_hash: HashOf<TransactionEntrypoint> =
                HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed(request.entry_hash));
            let height = state
                .state
                .view()
                .transactions()
                .get(&entry_hash)
                .and_then(|height| u64::try_from(height.get()).ok())
                .and_then(NonZeroU64::new)
                .ok_or(StatusCode::ACCEPTED)?;
            let deadline = Instant::now()
                .checked_add(timeout)
                .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
            let budget = AllocationBudget::new(PUBLICATION_PROOF_BUDGET_BYTES);
            // The certificate charges stay held until the response bytes exist.
            let (proof, _charges) = publication_evidence(
                &state.state,
                &verified.account,
                &request,
                entry_hash,
                height,
                &budget,
                deadline,
            )?;
            canonical_response(&proof, PUBLICATION_PROOF_MAX_BYTES_V1)
        }),
    )
    .await;
    match result {
        Ok(Ok(response)) => response,
        Ok(Err(status)) => status.into_response(),
        Err(_) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

/// Allocation ceiling for one publication proof: certificates, inclusion and decoder work.
const PUBLICATION_PROOF_BUDGET_BYTES: usize =
    2 * iroha_data_model::sorafs::publication::PUBLICATION_PROOF_MAX_BYTES_V1;

/// Build compact evidence for one committed assertion signed by `requester`.
///
/// Returns the epoch-boundary certificates from the floor's epoch to the executing block's
/// epoch, that block's commit certificate and the assertion's selective inclusion, together
/// with the allocation charges of the certificate reads. Every component is DATA: the client
/// authenticates it under its own independently selected checkpoint.
fn publication_evidence(
    state: &iroha_core::state::State,
    requester: &AccountId,
    request: &iroha_data_model::sorafs::publication::SorafsPublicationProofRequestV1,
    entry_hash: HashOf<TransactionEntrypoint>,
    height: NonZeroU64,
    budget: &AllocationBudget,
    deadline: Instant,
) -> Result<
    (
        iroha_data_model::sorafs::publication::SorafsPublicationProofV1,
        Vec<AllocationReservation>,
    ),
    StatusCode,
> {
    use iroha_core::state::StateReadOnly as _;
    use iroha_data_model::sorafs::publication::{
        PUBLICATION_PROOF_MAX_BYTES_V1, PUBLICATION_PROOF_MAX_EPOCH_BOUNDARIES_V1,
        SorafsPublicationProofV1,
    };

    let view = state.view();
    let floor_index = usize::try_from(request.floor_height)
        .ok()
        .and_then(|floor| floor.checked_sub(1))
        .ok_or(StatusCode::BAD_REQUEST)?;
    if view
        .block_hashes()
        .get(floor_index)
        .map(|hash| *hash.as_ref())
        != Some(request.floor_block_hash)
        || height.get() <= request.floor_height
    {
        return Err(StatusCode::CONFLICT);
    }
    let limit = u64::try_from(PUBLICATION_PROOF_MAX_BYTES_V1)
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let history = iroha_core::state::CanonicalHistoryReadBudget::new(
        budget.clone(),
        norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            PUBLICATION_PROOF_MAX_BYTES_V1,
            PUBLICATION_PROOF_MAX_BYTES_V1,
            PUBLICATION_PROOF_MAX_BYTES_V1,
            PUBLICATION_PROOF_MAX_BYTES_V1,
            norito::core::MAX_VALUE_NESTING_DEPTH,
        )),
    );
    // The signer rule runs on the owner projection, before any Merkle path is built.
    let inclusion = state
        .kagemusha_ledger_inclusion_for_entry(
            height,
            entry_hash,
            iroha_core::state::BlockProofLimits {
                max_response_bytes: limit,
                ..crate::block_proof_limits(state)
            },
            &history,
            |owner| owner.authority == *requester,
        )
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
        .ok_or(StatusCode::FORBIDDEN)?
        .inclusion;
    let scope: CertifiedResultScope = view
        .native_execution_tip()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?
        .root_scope()
        .into();
    let mut charges = Vec::new();
    let mut read = |at: u64| -> Result<(SumeragiCommitCertificateV1, ScheduleOutcome), StatusCode> {
        let at = NonZeroU64::new(at).ok_or(StatusCode::CONFLICT)?;
        let (certificate, charge) =
            iroha_core::sumeragi::finality::read_commit_certificate(&view, at, budget, deadline)
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
                .into_parts();
        charges.push(charge);
        let schedule = CertifiedResultV1::decode(&certificate.result_preimage, scope)
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?
            .commitment()
            .schedule
            .clone();
        Ok((certificate, schedule))
    };
    // Genesis has no commit certificate; height two carries the first epoch's schedule.
    let mut epoch_end = read(request.floor_height.max(2))?
        .1
        .current
        .authorization
        .last_height;
    let mut epoch_boundaries = Vec::new();
    while epoch_end < height.get() {
        if epoch_boundaries.len() == PUBLICATION_PROOF_MAX_EPOCH_BOUNDARIES_V1 {
            return Err(StatusCode::PAYLOAD_TOO_LARGE);
        }
        let (boundary, schedule) = read(epoch_end)?;
        let next = schedule
            .boundary
            .as_ref()
            .map(|boundary| boundary.next.authorization.last_height)
            .filter(|next| *next > epoch_end)
            .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;
        epoch_boundaries.push(boundary);
        epoch_end = next;
    }
    let (certificate, _) = read(height.get())?;
    Ok((
        SorafsPublicationProofV1 {
            epoch_boundaries,
            certificate,
            inclusion,
        },
        charges,
    ))
}

#[cfg(test)]
#[path = "publisher/proof_tests.rs"]
mod proof_tests;
