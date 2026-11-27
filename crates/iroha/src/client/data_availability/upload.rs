//! Blocking DA upload transport shared by the Rust client and CLI.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use eyre::{Result, WrapErr as _, ensure};
use iroha_data_model::da::{ingest::DaIngestRequest, types::BlobDigest};
use iroha_torii_shared::da::upload::*;
use url::Url;

/// Send one signed ingest phase using account-authenticated HTTP exchanges.
/// Retries resume the contiguous prefix already acknowledged by Torii.
///
/// # Errors
/// Returns an error on an invalid length, transport failure or inconsistent progress.
pub fn submit_ingest_phase<T>(
    endpoint: &Url,
    request: &DaIngestRequest,
    mut post: impl FnMut(&Url, Vec<u8>) -> Result<(Vec<u8>, T)>,
) -> Result<(Vec<u8>, T)> {
    let length = request.payload.len();
    if length <= DA_UPLOAD_PART_BYTES {
        return post(endpoint, norito::json::to_vec(request)?);
    }
    ensure!(
        length <= DA_UPLOAD_MAX_BYTES,
        "DA transmitted payload exceeds the upload limit"
    );
    let mut parts_url = endpoint.clone();
    parts_url
        .path_segments_mut()
        .map_err(|_| eyre::eyre!("DA ingest endpoint requires an HTTP path"))?
        .pop_if_empty()
        .push("parts");
    let upload = DaUploadDescriptor {
        network_id: request.network_id,
        lane_id: request.lane_id,
        epoch: request.epoch,
        wire_bytes: length as u64,
        wire_hash: BlobDigest::from_hash(blake3::hash(&request.payload)),
    };
    let mut exchange = |action| {
        let body = norito::json::to_vec(&DaUploadRequest { upload, action })?;
        ensure!(
            body.len() <= DA_UPLOAD_REQUEST_MAX_BYTES,
            "DA upload envelope exceeds the request limit"
        );
        post(&parts_url, body)
    };
    let (bytes, _) = exchange(DaUploadAction::Begin)?;
    let progress: DaUploadProgress =
        norito::json::from_slice(&bytes).wrap_err("invalid DA begin response")?;
    let count = length.div_ceil(DA_UPLOAD_PART_BYTES);
    ensure!(
        progress.next_part as usize <= count,
        "Torii returned invalid DA upload progress"
    );
    for (index, data) in request
        .payload
        .chunks(DA_UPLOAD_PART_BYTES)
        .enumerate()
        .skip(progress.next_part as usize)
    {
        let (bytes, _) = exchange(DaUploadAction::Put(DaUploadPart {
            index: index as u32,
            bytes_b64: STANDARD.encode(data),
        }))?;
        let progress: DaUploadProgress =
            norito::json::from_slice(&bytes).wrap_err("invalid DA part acknowledgment")?;
        ensure!(
            progress.next_part as usize == index + 1,
            "Torii returned inconsistent DA part acknowledgment"
        );
    }
    // Only metadata crosses the wire on Finish. Keep a single copy of the large body.
    let metadata = DaIngestRequest {
        network_id: request.network_id,
        owner: request.owner.clone(),
        client_blob_id: request.client_blob_id,
        lane_id: request.lane_id,
        epoch: request.epoch,
        sequence: request.sequence,
        blob_class: request.blob_class,
        codec: request.codec.clone(),
        erasure_profile: request.erasure_profile,
        retention_policy: request.retention_policy.clone(),
        chunk_size: request.chunk_size,
        total_size: request.total_size,
        payload_hash: request.payload_hash,
        compression: request.compression,
        norito_manifest: request.norito_manifest.clone(),
        payload: Vec::new(),
        metadata: request.metadata.clone(),
        signatures: request.signatures.clone(),
        pin_scope_signatures: request.pin_scope_signatures.clone(),
    };
    exchange(DaUploadAction::Finish(Box::new(metadata)))
}
