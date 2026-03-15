//! Data availability ingest handlers for Torii.

#![allow(clippy::redundant_pub_crate)]

use std::{
    borrow::{Cow, ToOwned},
    io::{ErrorKind, Read},
    str::FromStr,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use axum::{
    extract::{Path as AxumPath, State},
    http::{HeaderMap, HeaderName, HeaderValue, StatusCode},
    response::Response,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use blake3::{Hasher as Blake3Hasher, hash as blake3_hash};
use flate2::read::{DeflateDecoder, GzDecoder};
use iroha_config::parameters::actual::LaneConfig as ConfigLaneConfig;
use iroha_core::da::{LaneEpoch, ReplayFingerprint, ReplayInsertOutcome, ReplayKey};
use iroha_crypto::{
    Hash, KeyPair, Signature,
    encryption::{ChaCha20Poly1305, SymmetricEncryptor},
};
use iroha_data_model::{
    account::AccountId,
    da::{
        commitment::{DaCommitmentRecord, DaProofScheme, KzgCommitment},
        manifest::ChunkRole,
        pin_intent::DaPinIntent,
        prelude::*,
    },
    nexus::LaneId,
    sorafs::{
        capacity::ProviderId,
        pin_registry::{ManifestDigest, StorageClass},
    },
};
use iroha_logger::{error, warn};
use iroha_torii_shared::da::sampling::{
    build_sampling_plan, compute_sample_window, sampling_plan_to_value,
};
use iroha_zkp_halo2::pallas::{
    Params as IpaCurveParams, Polynomial as IpaPolynomial, Scalar as IpaScalar,
};
use norito::{
    core::NoritoDeserialize,
    decode_from_bytes, from_bytes,
    json::{self, JsonSerialize, Map, Value},
    to_bytes,
};
use sorafs_car::{ChunkStore, build_plan_from_da_manifest, fetch_plan::chunk_fetch_specs_to_json};
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, ChunkingProfileV1,
    deal::XorAmount,
    pdp::{HashAlgorithmV1, PDP_COMMITMENT_VERSION_V1, PdpCommitmentV1},
};
use zstd::stream::decode_all as zstd_decode_all;

use super::persistence::{RECEIPT_SIGNATURE_PLACEHOLDER, ReceiptInsertOutcome};
use super::rs16::build_chunk_commitments;
use super::{persistence, storage_class_label, taikai, taikai::taikai_ingest};
use crate::{
    NoritoQuery, SharedAppState,
    routing::MaybeTelemetry,
    sorafs::api::ResponseError,
    utils::{self, ResponseFormat},
};

const HEADER_SORA_PDP_COMMITMENT: &str = "sora-pdp-commitment";
const META_DA_REGISTRY_ALIAS: &str = "da.registry.alias";
const META_DA_REGISTRY_OWNER: &str = "da.registry.owner";
const BYTES_PER_GIB: u64 = 1024 * 1024 * 1024;
const SECS_PER_MONTH: u64 = 30 * 24 * 60 * 60;

#[derive(Debug)]
struct CanonicalPayload<'a> {
    bytes: Cow<'a, [u8]>,
}

impl CanonicalPayload<'_> {
    fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    fn len(&self) -> usize {
        self.bytes.len()
    }

    fn into_vec(self) -> Vec<u8> {
        self.bytes.into_owned()
    }
}

/// HTTP handler for `/v1/da/ingest`.
pub async fn handler_post_da_ingest(
    State(app): State<SharedAppState>,
    headers: HeaderMap,
    utils::extractors::JsonOnly(request): utils::extractors::JsonOnly<DaIngestRequest>,
) -> Result<Response, ResponseError> {
    let telemetry = app.telemetry_handle();
    let cluster_label = app
        .da_ingest
        .telemetry_cluster_label
        .as_deref()
        .unwrap_or("default");
    let nexus = app.state.nexus_snapshot();
    let format = utils::negotiate_response_format(headers.get(axum::http::header::ACCEPT))
        .map_err(ResponseError::from)?;

    if !nexus.enabled {
        return Err(ResponseError::from(build_error_response(
            StatusCode::BAD_REQUEST,
            "/v1/da/ingest requires nexus.enabled=true; lanes are unavailable in Iroha 2 mode",
            format,
        )));
    }

    let canonical = normalize_payload(&request).map_err(|(status, message)| {
        ResponseError::from(build_error_response(status, &message, format))
    })?;

    validate_request(&request, canonical.len()).map_err(|(status, message)| {
        ResponseError::from(build_error_response(status, message, format))
    })?;

    let proof_scheme =
        lane_proof_scheme(&nexus.lane_config, request.lane_id).map_err(|(status, message)| {
            ResponseError::from(build_error_response(status, &message, format))
        })?;

    let mut metadata = encrypt_governance_metadata(
        &request.metadata,
        app.da_ingest.governance_metadata_key.as_ref(),
        app.da_ingest.governance_metadata_key_label.as_deref(),
    )
    .map_err(|(status, message)| {
        ResponseError::from(build_error_response(status, &message, format))
    })?;

    let mut taikai_ssm_payload =
        taikai_ingest::take_ssm_entry(&mut metadata).map_err(|(status, message)| {
            ResponseError::from(build_error_response(status, &message, format))
        })?;
    let mut taikai_trm_payload =
        taikai_ingest::take_trm_entry(&mut metadata).map_err(|(status, message)| {
            ResponseError::from(build_error_response(status, &message, format))
        })?;

    let taikai_availability = if matches!(request.blob_class, BlobClass::TaikaiSegment) {
        taikai::taikai_availability_from_metadata(&request.metadata, taikai_trm_payload.as_deref())
            .map_err(|(status, message)| {
                ResponseError::from(build_error_response(status, &message, format))
            })?
    } else {
        None
    };

    let (expected_retention, retention_mismatch) = app.da_ingest.replication_policy.enforce(
        request.blob_class,
        taikai_availability,
        &request.retention_policy,
    );
    let enforced_retention = expected_retention.clone();
    if retention_mismatch {
        warn!(
            blob_class = ?request.blob_class,
            submitted = ?request.retention_policy,
            expected = ?enforced_retention,
            "overriding DA retention policy to match configured network baseline"
        );
    }

    if matches!(request.blob_class, BlobClass::TaikaiSegment) {
        let payload_digest = BlobDigest::from_hash(blake3_hash(canonical.as_slice()));
        taikai::apply_taikai_ingest_tags(
            &mut metadata,
            taikai_availability,
            &enforced_retention,
            payload_digest,
            request.total_size,
        )
        .map_err(|(status, message)| {
            ResponseError::from(build_error_response(status, &message, format))
        })?;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);
    let queued_at_secs = now.as_secs();
    let chunk_store = build_chunk_store(&request, canonical.as_slice());
    let chunking_observer = |elapsed: Duration| {
        record_da_chunking_metrics(&telemetry, elapsed);
    };
    let manifest = resolve_manifest_with_observer(
        &request,
        &chunk_store,
        canonical.as_slice(),
        &metadata,
        &enforced_retention,
        queued_at_secs,
        &app.da_ingest.rent_policy,
        Some(&chunking_observer),
    )
    .map_err(|(status, message)| {
        ResponseError::from(build_error_response(status, &message, format))
    })?;

    let fingerprint = manifest.fingerprint;
    let lane_epoch = LaneEpoch::new(request.lane_id, request.epoch);
    let replay_key = ReplayKey::new(lane_epoch, request.sequence, fingerprint);

    let outcome = app.da_replay_cache.insert(replay_key, Instant::now());

    match outcome {
        ReplayInsertOutcome::Fresh { .. } | ReplayInsertOutcome::Duplicate { .. } => {
            if matches!(outcome, ReplayInsertOutcome::Fresh { .. }) {
                if let Err(err) = app.da_replay_store.record(lane_epoch, request.sequence) {
                    warn!(?err, "failed to persist DA replay cursor");
                }
            }
            let duplicate = matches!(outcome, ReplayInsertOutcome::Duplicate { .. });
            let (rent_gib, rent_months) =
                rent_usage_from_request(request.total_size, &enforced_retention);
            record_da_rent_quote_metrics(
                &telemetry,
                cluster_label,
                enforced_retention.storage_class,
                rent_gib,
                rent_months,
                &manifest.manifest.rent_quote,
            );

            if let Err(err) = persistence::persist_manifest_for_sorafs(
                &app.da_ingest.manifest_store_dir,
                &manifest.encoded,
                request.lane_id,
                request.epoch,
                request.sequence,
                &manifest.storage_ticket,
                &fingerprint,
            ) {
                error!(
                    ?err,
                    spool_dir = ?app.da_ingest.manifest_store_dir,
                    ticket = %hex::encode(manifest.storage_ticket.as_ref()),
                    "failed to enqueue DA manifest for SoraFS orchestration"
                );
            }

            let pdp_commitment = compute_pdp_commitment(
                &manifest.manifest_hash,
                &manifest.manifest,
                &chunk_store,
                queued_at_secs,
            )
            .map_err(|(status, message)| {
                ResponseError::from(build_error_response(status, &message, format))
            })?;
            let pdp_commitment_bytes =
                encode_pdp_commitment_bytes(&pdp_commitment).map_err(|(status, message)| {
                    ResponseError::from(build_error_response(status, &message, format))
                })?;
            let pdp_header_value = pdp_commitment_header_value(&pdp_commitment_bytes).map_err(
                |(status, message)| {
                    ResponseError::from(build_error_response(status, &message, format))
                },
            )?;

            if let Err(err) = persistence::persist_pdp_commitment(
                &app.da_ingest.manifest_store_dir,
                &pdp_commitment,
                request.lane_id,
                request.epoch,
                request.sequence,
                &manifest.storage_ticket,
                &fingerprint,
            ) {
                error!(
                    ?err,
                    spool_dir = ?app.da_ingest.manifest_store_dir,
                    ticket = %hex::encode(manifest.storage_ticket.as_ref()),
                    "failed to enqueue PDP commitment for SoraFS orchestration"
                );
            }

            let stripe_layout = stripe_layout_from_manifest(&manifest.manifest);
            let receipt = build_receipt(
                &app.da_receipt_signer,
                &request,
                queued_at_secs,
                manifest.blob_hash,
                manifest.chunk_root,
                manifest.manifest_hash,
                manifest.storage_ticket,
                pdp_commitment_bytes.clone(),
                manifest.manifest.rent_quote,
                stripe_layout,
            );
            let commitment_record = build_da_commitment_record(
                &request,
                &manifest,
                &enforced_retention,
                &receipt.operator_signature,
                &pdp_commitment_bytes,
                proof_scheme,
            );
            if let Err(err) = persistence::persist_da_commitment_record(
                &app.da_ingest.manifest_store_dir,
                &commitment_record,
                request.lane_id,
                request.epoch,
                request.sequence,
                &manifest.storage_ticket,
                &fingerprint,
            ) {
                error!(
                    ?err,
                    spool_dir = ?app.da_ingest.manifest_store_dir,
                    ticket = %hex::encode(manifest.storage_ticket.as_ref()),
                    "failed to enqueue DA commitment record for bundle ingestion"
                );
            }
            if let Err(err) = persistence::persist_da_commitment_schedule_entry(
                &app.da_ingest.manifest_store_dir,
                &commitment_record,
                &pdp_commitment_bytes,
                request.lane_id,
                request.epoch,
                request.sequence,
                &manifest.storage_ticket,
                &fingerprint,
            ) {
                error!(
                    ?err,
                    spool_dir = ?app.da_ingest.manifest_store_dir,
                    ticket = %hex::encode(manifest.storage_ticket.as_ref()),
                    "failed to enqueue DA commitment schedule entry"
                );
            }
            let pin_alias =
                registry_alias_from_metadata(&request.metadata).map_err(|(status, message)| {
                    ResponseError::from(build_error_response(status, &message, format))
                })?;
            let pin_owner =
                registry_owner_from_metadata(&request.metadata).map_err(|(status, message)| {
                    ResponseError::from(build_error_response(status, &message, format))
                })?;
            let mut pin_intent = DaPinIntent::new(
                request.lane_id,
                request.epoch,
                request.sequence,
                manifest.storage_ticket,
                ManifestDigest::new(*manifest.manifest_hash.as_bytes()),
            );
            pin_intent.alias = pin_alias;
            pin_intent.owner = pin_owner;
            if let Err(err) = persistence::persist_da_pin_intent(
                &app.da_ingest.manifest_store_dir,
                &pin_intent,
                request.lane_id,
                request.epoch,
                request.sequence,
                &manifest.storage_ticket,
                &fingerprint,
            ) {
                error!(
                    ?err,
                    spool_dir = ?app.da_ingest.manifest_store_dir,
                    ticket = %hex::encode(manifest.storage_ticket.as_ref()),
                    "failed to enqueue DA pin intent for registry ingestion"
                );
            }

            if let Err(err) = persistence::persist_da_receipt(
                &app.da_ingest.manifest_store_dir,
                &receipt,
                request.sequence,
                &fingerprint,
            ) {
                error!(
                    ?err,
                    spool_dir = ?app.da_ingest.manifest_store_dir,
                    lane = request.lane_id.as_u32(),
                    epoch = request.epoch,
                    sequence = request.sequence,
                    "failed to enqueue DA receipt for downstream fanout"
                );
            }
            match app.da_receipt_log.append(
                lane_epoch,
                request.sequence,
                receipt.clone(),
                fingerprint,
            ) {
                Ok(outcome) => {
                    record_da_receipt_metrics(&telemetry, lane_epoch, request.sequence, &outcome);
                }
                Err(err) => {
                    warn!(
                        ?err,
                        ?lane_epoch,
                        sequence = request.sequence,
                        "failed to record DA receipt in durable log"
                    );
                    record_da_receipt_error_metrics(&telemetry, lane_epoch, request.sequence);
                }
            }

            if matches!(request.blob_class, BlobClass::TaikaiSegment) {
                let taikai = match taikai_ingest::build_envelope(
                    &request,
                    &manifest,
                    &chunk_store,
                    canonical.as_slice(),
                    Some(&chunking_observer),
                ) {
                    Ok(value) => value,
                    Err((status, message)) => {
                        let stream_label = taikai::stream_label_from_metadata(&request.metadata)
                            .unwrap_or_else(|| taikai_ingest::STREAM_LABEL_FALLBACK.to_string());
                        taikai::record_taikai_ingest_error(
                            &telemetry,
                            cluster_label,
                            &stream_label,
                            status,
                        );
                        return Err(ResponseError::from(build_error_response(
                            status, &message, format,
                        )));
                    }
                };

                if let Err(err) = taikai_ingest::persist_envelope(
                    &app.da_ingest.manifest_store_dir,
                    request.lane_id,
                    request.epoch,
                    request.sequence,
                    &manifest.storage_ticket,
                    &fingerprint,
                    &taikai.envelope_bytes,
                ) {
                    error!(
                        ?err,
                        spool_dir = ?app.da_ingest.manifest_store_dir,
                        "failed to enqueue Taikai envelope for anchoring"
                    );
                }

                if let Err(err) = taikai_ingest::persist_indexes(
                    &app.da_ingest.manifest_store_dir,
                    request.lane_id,
                    request.epoch,
                    request.sequence,
                    &manifest.storage_ticket,
                    &fingerprint,
                    &taikai.indexes_json,
                ) {
                    error!(
                        ?err,
                        spool_dir = ?app.da_ingest.manifest_store_dir,
                        "failed to enqueue Taikai index bundle for anchoring"
                    );
                }

                let ssm_bytes = taikai_ssm_payload.take().ok_or_else(|| {
                    build_error_response(
                        StatusCode::BAD_REQUEST,
                        "metadata entry `taikai.ssm` is required for Taikai segments",
                        format,
                    )
                })?;

                let ssm_outcome = taikai::validate_taikai_ssm(
                    &ssm_bytes,
                    &manifest.manifest_hash,
                    &taikai.car_digest,
                    &taikai.envelope_bytes,
                    taikai.telemetry.segment_sequence,
                    &app.sorafs_alias_cache_policy,
                    &telemetry,
                )
                .map_err(|(status, message)| {
                    ResponseError::from(build_error_response(status, &message, format))
                })?;

                if let Err(err) = taikai_ingest::persist_ssm(
                    &app.da_ingest.manifest_store_dir,
                    request.lane_id,
                    request.epoch,
                    request.sequence,
                    &manifest.storage_ticket,
                    &fingerprint,
                    &ssm_bytes,
                ) {
                    error!(
                        ?err,
                        spool_dir = ?app.da_ingest.manifest_store_dir,
                        "failed to enqueue Taikai signing manifest for anchoring"
                    );
                }

                iroha_logger::info!(
                    manifest_hash = %hex::encode(manifest.manifest_hash.as_ref()),
                    alias = %ssm_outcome.alias_label,
                    ssm_digest = %hex::encode(ssm_outcome.ssm_digest.as_ref()),
                    "accepted Taikai signing manifest"
                );

                if let Some(trm_bytes) = taikai_trm_payload.take() {
                    let routing_manifest = taikai::validate_taikai_trm(&trm_bytes, &taikai)
                        .map_err(|(status, message): (StatusCode, String)| {
                            ResponseError::from(build_error_response(status, &message, format))
                        })?;
                    let manifest_digest_hex = hex::encode(blake3_hash(&trm_bytes).as_bytes());
                    let mut lineage_guard = taikai_ingest::TrmLineageGuard::new(
                        &app.da_ingest.manifest_store_dir,
                        &routing_manifest.alias_binding,
                    )
                    .map_err(|(status, message): (StatusCode, String)| {
                        ResponseError::from(build_error_response(status, &message, format))
                    })?;
                    if let Some(guard) = lineage_guard.as_mut() {
                        guard
                            .validate(&routing_manifest, &manifest_digest_hex)
                            .map_err(|(status, message): (StatusCode, String)| {
                                ResponseError::from(build_error_response(status, &message, format))
                            })?;
                    }

                    match taikai_ingest::persist_trm(
                        &app.da_ingest.manifest_store_dir,
                        request.lane_id,
                        request.epoch,
                        request.sequence,
                        &manifest.storage_ticket,
                        &fingerprint,
                        &trm_bytes,
                    ) {
                        Ok(persisted) => {
                            if persisted.is_some() {
                                if let Some(guard) = lineage_guard.as_mut() {
                                    guard
                                        .persist_lineage_hint(
                                            request.lane_id,
                                            request.epoch,
                                            request.sequence,
                                            &manifest.storage_ticket,
                                            &fingerprint,
                                        )
                                        .map_err(|(status, message): (StatusCode, String)| {
                                            ResponseError::from(build_error_response(
                                                status, &message, format,
                                            ))
                                        })?;
                                    guard
                                        .commit(
                                            routing_manifest.segment_window,
                                            &manifest_digest_hex,
                                        )
                                        .map_err(|(status, message): (StatusCode, String)| {
                                            ResponseError::from(build_error_response(
                                                status, &message, format,
                                            ))
                                        })?;
                                }
                            }
                            taikai::record_taikai_alias_rotation_event(
                                &telemetry,
                                cluster_label,
                                &routing_manifest,
                                &manifest_digest_hex,
                            );
                        }
                        Err(err) => {
                            error!(
                                ?err,
                                spool_dir = ?app.da_ingest.manifest_store_dir,
                                "failed to enqueue Taikai routing manifest for anchoring"
                            );
                        }
                    }
                }

                taikai::record_taikai_ingest_metrics(&telemetry, cluster_label, &taikai.telemetry);
            }

            let response = DaIngestResponse {
                status: "accepted",
                duplicate,
                receipt: Some(receipt),
            };
            let mut http_response = utils::respond_with_format(response, format);
            http_response.headers_mut().insert(
                HeaderName::from_static(HEADER_SORA_PDP_COMMITMENT),
                pdp_header_value,
            );
            Ok(with_status(http_response, StatusCode::ACCEPTED))
        }
        ReplayInsertOutcome::StaleSequence { highest_observed } => {
            let message = format!(
                "sequence {} is too far behind; highest observed is {}",
                request.sequence, highest_observed
            );
            Ok(build_error_response(StatusCode::CONFLICT, &message, format))
        }
        ReplayInsertOutcome::ConflictingFingerprint { .. } => Ok(build_error_response(
            StatusCode::CONFLICT,
            "sequence already used for a different manifest",
            format,
        )),
    }
}

#[derive(
    Debug, Default, Clone, crate::json_macros::JsonDeserialize, norito::derive::NoritoDeserialize,
)]
pub struct DaManifestQuery {
    block_hash: Option<String>,
}

/// HTTP handler for `/v1/da/manifests/{ticket}`.
pub async fn handler_get_da_manifest(
    State(app): State<SharedAppState>,
    AxumPath(ticket_hex): AxumPath<String>,
    NoritoQuery(params): NoritoQuery<DaManifestQuery>,
    headers: HeaderMap,
) -> Result<Response, ResponseError> {
    let format = utils::negotiate_response_format(headers.get(axum::http::header::ACCEPT))
        .map_err(ResponseError::from)?;

    let nexus_enabled = app.state.nexus_snapshot().enabled;
    if !nexus_enabled {
        return Err(ResponseError::from(build_error_response(
            StatusCode::BAD_REQUEST,
            "/v1/da/manifests requires nexus.enabled=true; lanes are unavailable in Iroha 2 mode",
            format,
        )));
    }

    let ticket_bytes = match parse_storage_ticket_hex(ticket_hex.trim()) {
        Ok(bytes) => bytes,
        Err(message) => {
            return Err(ResponseError::from(build_error_response(
                StatusCode::BAD_REQUEST,
                &message,
                format,
            )));
        }
    };
    let ticket = StorageTicketId::new(ticket_bytes);

    let manifest_bytes =
        match persistence::load_manifest_from_spool(&app.da_ingest.manifest_store_dir, &ticket) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                return Err(ResponseError::from(build_error_response(
                    StatusCode::NOT_FOUND,
                    "manifest not found for storage ticket",
                    format,
                )));
            }
            Err(err) => {
                return Err(ResponseError::from(build_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("failed to read manifest from spool: {err}"),
                    format,
                )));
            }
        };

    let manifest: DaManifestV1 = match decode_from_bytes(&manifest_bytes) {
        Ok(manifest) => manifest,
        Err(err) => {
            return Err(ResponseError::from(build_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("failed to decode stored manifest: {err}"),
                format,
            )));
        }
    };

    let plan = match build_plan_from_da_manifest(&manifest) {
        Ok(plan) => plan,
        Err(err) => {
            return Err(ResponseError::from(build_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("failed to derive chunk plan from manifest: {err}"),
                format,
            )));
        }
    };

    let chunk_plan = chunk_fetch_specs_to_json(&plan);
    let manifest_json = match json::to_value(&manifest) {
        Ok(value) => value,
        Err(err) => {
            return Err(ResponseError::from(build_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("failed to render manifest JSON: {err}"),
                format,
            )));
        }
    };
    let manifest_hash = BlobDigest::from_hash(blake3_hash(&manifest_bytes));

    let sampling_plan = if let Some(block_hex) = params.block_hash.as_deref() {
        let block_hash = match parse_block_hash_hex(block_hex) {
            Ok(hash) => hash,
            Err(message) => {
                return Err(ResponseError::from(build_error_response(
                    StatusCode::BAD_REQUEST,
                    &message,
                    format,
                )));
            }
        };
        Some(build_sampling_plan(&manifest, &block_hash))
    } else {
        None
    };

    let mut body = Map::new();
    body.insert(
        "storage_ticket".into(),
        Value::from(hex::encode(ticket.as_bytes())),
    );
    body.insert(
        "client_blob_id".into(),
        Value::from(hex::encode(manifest.client_blob_id.as_bytes())),
    );
    body.insert(
        "blob_hash".into(),
        Value::from(hex::encode(manifest.blob_hash.as_bytes())),
    );
    body.insert(
        "chunk_root".into(),
        Value::from(hex::encode(manifest.chunk_root.as_bytes())),
    );
    body.insert(
        "manifest_hash".into(),
        Value::from(hex::encode(manifest_hash.as_bytes())),
    );
    body.insert("lane_id".into(), Value::from(manifest.lane_id.as_u32()));
    body.insert("epoch".into(), Value::from(manifest.epoch));
    body.insert("manifest".into(), manifest_json);
    body.insert(
        "manifest_norito".into(),
        Value::from(BASE64.encode(&manifest_bytes)),
    );
    body.insert(
        "manifest_len".into(),
        Value::from(manifest_bytes.len() as u64),
    );
    body.insert("chunk_plan".into(), chunk_plan);
    if let Some(plan) = sampling_plan {
        body.insert("sampling_plan".into(), sampling_plan_to_value(&plan));
    }

    let mut response = utils::respond_value_with_format(Value::Object(body), format);
    match persistence::load_pdp_commitment_from_spool(&app.da_ingest.manifest_store_dir, &ticket) {
        Ok(commitment) => match pdp_commitment_header_value(&commitment) {
            Ok(value) => {
                response
                    .headers_mut()
                    .insert(HeaderName::from_static(HEADER_SORA_PDP_COMMITMENT), value);
            }
            Err((_, message)) => {
                warn!(
                    ticket = %hex::encode(ticket.as_bytes()),
                    "failed to encode PDP commitment header: {message}"
                );
            }
        },
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => {
            warn!(
                ?err,
                ticket = %hex::encode(ticket.as_bytes()),
                "failed to load PDP commitment for manifest fetch"
            );
        }
    }
    Ok(response)
}

fn normalize_payload(
    request: &DaIngestRequest,
) -> Result<CanonicalPayload<'_>, (StatusCode, String)> {
    match request.compression {
        Compression::Identity => Ok(CanonicalPayload {
            bytes: Cow::Borrowed(&request.payload),
        }),
        Compression::Gzip | Compression::Deflate | Compression::Zstd => {
            let expected_len = usize::try_from(request.total_size).map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    format!(
                        "total_size {} exceeds this node's supported payload length",
                        request.total_size
                    ),
                )
            })?;
            let decompressed = match request.compression {
                Compression::Identity => unreachable!("handled above"),
                Compression::Gzip => decompress_reader(
                    GzDecoder::new(request.payload.as_slice()),
                    expected_len,
                    "gzip",
                )?,
                Compression::Deflate => decompress_reader(
                    DeflateDecoder::new(request.payload.as_slice()),
                    expected_len,
                    "deflate",
                )?,
                Compression::Zstd => decompress_zstd(request.payload.as_slice(), expected_len)?,
            };
            Ok(CanonicalPayload {
                bytes: Cow::Owned(decompressed),
            })
        }
    }
}

fn parse_block_hash_hex(input: &str) -> Result<Hash, String> {
    let trimmed = input
        .trim()
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    if trimmed.is_empty() {
        return Err("block_hash must not be empty when provided".into());
    }
    Hash::from_str(trimmed).map_err(|err| format!("invalid block_hash: {err}"))
}

fn parse_storage_ticket_hex(input: &str) -> Result<[u8; 32], String> {
    if input.is_empty() {
        return Err("storage ticket must be provided".into());
    }
    let trimmed = input.trim_start_matches("0x").trim_start_matches("0X");
    let bytes = hex::decode(trimmed)
        .map_err(|_| "storage ticket must be a 64-character hex string".to_owned())?;
    if bytes.len() != 32 {
        return Err(format!(
            "storage ticket must decode to 32 bytes (got {})",
            bytes.len()
        ));
    }
    let mut array = [0u8; 32];
    array.copy_from_slice(&bytes);
    Ok(array)
}

fn encode_pdp_commitment_bytes(
    commitment: &PdpCommitmentV1,
) -> Result<Vec<u8>, (StatusCode, String)> {
    to_bytes(commitment).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to encode PDP commitment: {err}"),
        )
    })
}

fn pdp_commitment_header_value(bytes: &[u8]) -> Result<HeaderValue, (StatusCode, String)> {
    let encoded = BASE64.encode(bytes);
    HeaderValue::from_str(&encoded).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to encode Sora-PDP-Commitment header: {err}"),
        )
    })
}

fn decompress_reader<R>(
    mut reader: R,
    expected_len: usize,
    algorithm: &'static str,
) -> Result<Vec<u8>, (StatusCode, String)>
where
    R: Read,
{
    let mut buffer = Vec::with_capacity(expected_len.min(16 * 1024));
    reader.read_to_end(&mut buffer).map_err(|err| {
        (
            StatusCode::BAD_REQUEST,
            format!("failed to decompress {algorithm} payload: {err}"),
        )
    })?;
    verify_decompressed_len(buffer, expected_len, algorithm)
}

fn decompress_zstd(payload: &[u8], expected_len: usize) -> Result<Vec<u8>, (StatusCode, String)> {
    let bytes = zstd_decode_all(payload).map_err(|err| {
        (
            StatusCode::BAD_REQUEST,
            format!("failed to decompress zstd payload: {err}"),
        )
    })?;
    verify_decompressed_len(bytes, expected_len, "zstd")
}

fn verify_decompressed_len(
    buffer: Vec<u8>,
    expected_len: usize,
    algorithm: &'static str,
) -> Result<Vec<u8>, (StatusCode, String)> {
    if buffer.len() != expected_len {
        Err((
            StatusCode::BAD_REQUEST,
            format!(
                "{algorithm} payload decompressed to {} bytes but total_size advertises {} bytes",
                buffer.len(),
                expected_len
            ),
        ))
    } else {
        Ok(buffer)
    }
}

fn validate_request(
    request: &DaIngestRequest,
    canonical_payload_len: usize,
) -> Result<(), (StatusCode, &'static str)> {
    if request.total_size != canonical_payload_len as u64 {
        return Err((
            StatusCode::BAD_REQUEST,
            "payload length does not match total_size",
        ));
    }

    if request.chunk_size == 0 || !request.chunk_size.is_power_of_two() {
        return Err((
            StatusCode::BAD_REQUEST,
            "chunk_size must be a non-zero power of two",
        ));
    }

    if request.chunk_size < 2 {
        return Err((
            StatusCode::BAD_REQUEST,
            "chunk_size must be at least 2 bytes for parity encoding",
        ));
    }

    const MAX_CHUNK_SIZE: u32 = 2 * 1024 * 1024;
    if request.chunk_size > MAX_CHUNK_SIZE {
        return Err((
            StatusCode::BAD_REQUEST,
            "chunk_size exceeds supported maximum (2 MiB)",
        ));
    }

    if request.erasure_profile.data_shards == 0 && request.erasure_profile.parity_shards == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "erasure profile must include at least one data or parity shard",
        ));
    }

    if request.erasure_profile.parity_shards < 2 {
        return Err((
            StatusCode::BAD_REQUEST,
            "erasure profile requires at least 2 parity shards",
        ));
    }

    Ok(())
}

fn lane_proof_scheme(
    lane_config: &ConfigLaneConfig,
    lane_id: LaneId,
) -> Result<DaProofScheme, (StatusCode, String)> {
    let Some(entry) = lane_config.entry(lane_id) else {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("lane {} not present in lane catalog", lane_id.as_u32()),
        ));
    };

    match entry.proof_scheme {
        DaProofScheme::MerkleSha256 | DaProofScheme::KzgBls12_381 => Ok(entry.proof_scheme),
    }
}

fn manifest_fingerprint(
    manifest: &DaManifestV1,
) -> Result<ReplayFingerprint, (StatusCode, String)> {
    let encoded = to_bytes(manifest).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to encode DA manifest for fingerprint: {err}"),
        )
    })?;
    Ok(ReplayFingerprint::from_hash(blake3_hash(&encoded)))
}

#[allow(clippy::too_many_arguments)]
fn build_receipt(
    signer: &KeyPair,
    request: &DaIngestRequest,
    queued_at: u64,
    blob_hash: BlobDigest,
    chunk_root: BlobDigest,
    manifest_hash: BlobDigest,
    storage_ticket: StorageTicketId,
    pdp_commitment: Vec<u8>,
    rent_quote: DaRentQuote,
    stripe_layout: DaStripeLayout,
) -> DaIngestReceipt {
    let mut receipt = DaIngestReceipt {
        client_blob_id: request.client_blob_id.clone(),
        lane_id: request.lane_id,
        epoch: request.epoch,
        blob_hash,
        chunk_root,
        manifest_hash,
        storage_ticket,
        pdp_commitment: Some(pdp_commitment),
        stripe_layout,
        queued_at_unix: queued_at,
        rent_quote,
        operator_signature: Signature::from_bytes(&RECEIPT_SIGNATURE_PLACEHOLDER),
    };
    let unsigned_bytes =
        to_bytes(&receipt).expect("DA receipt is Norito-serializable before signing");
    receipt.operator_signature = Signature::new(signer.private_key(), &unsigned_bytes);
    receipt
}

fn stripe_layout_from_manifest(manifest: &DaManifestV1) -> DaStripeLayout {
    DaStripeLayout {
        total_stripes: manifest.total_stripes,
        shards_per_stripe: manifest.shards_per_stripe,
        row_parity_stripes: manifest.erasure_profile.row_parity_stripes,
    }
}

fn chunk_profile_for_request(chunk_size: u32) -> ChunkProfile {
    let size = usize::try_from(chunk_size.max(1)).unwrap_or(usize::MAX);
    ChunkProfile {
        min_size: size,
        target_size: size,
        max_size: size,
        break_mask: 1,
    }
}

fn build_chunk_store(request: &DaIngestRequest, canonical_payload: &[u8]) -> ChunkStore {
    let mut store = ChunkStore::with_profile(chunk_profile_for_request(request.chunk_size));
    store.ingest_bytes(canonical_payload);
    store
}

fn encrypt_governance_metadata(
    metadata: &ExtraMetadata,
    key: Option<&[u8; 32]>,
    key_label: Option<&str>,
) -> Result<ExtraMetadata, (StatusCode, String)> {
    if metadata.items.is_empty() {
        return Ok(metadata.clone());
    }

    let mut encryptor: Option<SymmetricEncryptor<ChaCha20Poly1305>> = None;
    let mut processed = Vec::with_capacity(metadata.items.len());

    for entry in &metadata.items {
        let mut entry = entry.clone();
        match entry.visibility {
            MetadataVisibility::Public => {
                if entry.encryption != MetadataEncryption::None {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        format!(
                            "metadata entry `{}` is public but declares encryption {:?}",
                            entry.key, entry.encryption
                        ),
                    ));
                }
            }
            MetadataVisibility::GovernanceOnly => {
                let key_bytes = key.ok_or_else(|| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Torii governance metadata encryption key is not configured".into(),
                    )
                })?;
                let expected_label = key_label;

                if encryptor.is_none() {
                    let enc = SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(key_bytes)
                        .map_err(|err| {
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                format!(
                                    "failed to initialise governance metadata encryptor: {err}"
                                ),
                            )
                        })?;
                    encryptor = Some(enc);
                }
                let encryptor = encryptor.as_ref().expect("initialised above");

                match entry.encryption {
                    MetadataEncryption::None => {
                        let ciphertext = encryptor
                            .encrypt_easy(entry.key.as_bytes(), &entry.value)
                            .map_err(|err| {
                                (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    format!(
                                        "failed to encrypt governance metadata entry `{}`: {err}",
                                        entry.key
                                    ),
                                )
                            })?;
                        entry.value = ciphertext;
                        entry.encryption = MetadataEncryption::chacha20poly1305_with_label(
                            expected_label.map(ToOwned::to_owned),
                        );
                    }
                    MetadataEncryption::ChaCha20Poly1305(ref envelope) => {
                        if let Some(label) = expected_label {
                            match envelope.key_label.as_deref() {
                                Some(observed) if observed == label => {}
                                Some(other) => {
                                    return Err((
                                        StatusCode::BAD_REQUEST,
                                        format!(
                                            "governance metadata entry `{}` encrypted with unexpected key `{other}` (expected `{label}`)",
                                            entry.key
                                        ),
                                    ));
                                }
                                None => {
                                    return Err((
                                        StatusCode::BAD_REQUEST,
                                        format!(
                                            "governance metadata entry `{}` missing key label (expected `{label}`)",
                                            entry.key
                                        ),
                                    ));
                                }
                            }
                        }

                        encryptor
                            .decrypt_easy(entry.key.as_bytes(), &entry.value)
                            .map_err(|_| {
                                (
                                    StatusCode::BAD_REQUEST,
                                    format!(
                                        "governance metadata entry `{}` has invalid ciphertext",
                                        entry.key
                                    ),
                                )
                            })?;
                    }
                }
            }
        }
        processed.push(entry);
    }

    Ok(ExtraMetadata { items: processed })
}

fn role_tag(role: ChunkRole) -> u8 {
    match role {
        ChunkRole::Data => 0,
        ChunkRole::LocalParity => 1,
        ChunkRole::GlobalParity => 2,
        ChunkRole::StripeParity => 3,
    }
}

fn effective_chunk_role(commitment: &ChunkCommitment) -> ChunkRole {
    if commitment.parity && matches!(commitment.role, ChunkRole::Data) {
        ChunkRole::GlobalParity
    } else {
        commitment.role
    }
}

fn ipa_scalar_from_chunk(commitment: &ChunkCommitment) -> IpaScalar {
    let mut hasher = Blake3Hasher::new();
    hasher.update(&commitment.index.to_le_bytes());
    hasher.update(&commitment.offset.to_le_bytes());
    hasher.update(&commitment.length.to_le_bytes());
    hasher.update(commitment.commitment.as_bytes());
    hasher.update(&[commitment.parity as u8, role_tag(commitment.role)]);
    hasher.update(&commitment.group_id.to_le_bytes());
    let mut wide = [0u8; 64];
    hasher.finalize_xof().fill(&mut wide);
    IpaScalar::from_uniform(&wide)
}

pub fn ipa_commitment_from_chunks(
    commitments: &[ChunkCommitment],
) -> Result<BlobDigest, (StatusCode, String)> {
    if commitments.is_empty() {
        return Ok(BlobDigest::default());
    }
    let params_len = commitments.len().next_power_of_two().max(1);
    let params = IpaCurveParams::new(params_len).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to derive IPA parameters: {err}"),
        )
    })?;
    let mut scalars: Vec<IpaScalar> = commitments.iter().map(ipa_scalar_from_chunk).collect();
    while scalars.len() < params_len {
        scalars.push(IpaScalar::zero());
    }
    let poly = IpaPolynomial::from_coeffs(scalars);
    let commitment = poly.commit(&params).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to commit IPA vector: {err}"),
        )
    })?;
    Ok(BlobDigest::new(commitment.to_bytes()))
}

fn compute_tree_height(count: usize) -> u16 {
    if count <= 1 {
        return 1;
    }
    let bits = usize::BITS - (count - 1).leading_zeros();
    bits as u16
}

fn compute_pdp_commitment(
    manifest_digest: &BlobDigest,
    manifest: &DaManifestV1,
    chunk_store: &ChunkStore,
    sealed_at_unix: u64,
) -> Result<PdpCommitmentV1, (StatusCode, String)> {
    let hot_root = chunk_store.pdp_hot_root().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "chunking did not produce PDP hot-leaf commitments".to_owned(),
        )
    })?;
    let segment_root = chunk_store.pdp_segment_root().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "chunking did not produce PDP segment commitments".to_owned(),
        )
    })?;

    let hot_leaf_count = chunk_store.pdp_hot_leaf_count();
    if hot_leaf_count == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "payload produced zero PDP hot leaves".to_owned(),
        ));
    }
    let segment_count = chunk_store.pdp_segment_count();
    if segment_count == 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "payload produced zero PDP segments".to_owned(),
        ));
    }

    let commitment = PdpCommitmentV1 {
        version: PDP_COMMITMENT_VERSION_V1,
        manifest_digest: *manifest_digest.as_ref(),
        chunk_profile: ChunkingProfileV1::from_profile(
            chunk_store.profile(),
            BLAKE3_256_MULTIHASH_CODE,
        ),
        commitment_root_hot: hot_root,
        commitment_root_segment: segment_root,
        hash_algorithm: HashAlgorithmV1::Blake3_256,
        hot_tree_height: compute_tree_height(hot_leaf_count),
        segment_tree_height: compute_tree_height(segment_count),
        sample_window: compute_sample_window(manifest.total_size),
        sealed_at: sealed_at_unix,
    };

    commitment
        .validate()
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(commitment)
}

#[derive(Debug)]
/// Manifest data captured during DA ingest for downstream spool writers.
pub(crate) struct ManifestArtifacts {
    pub(super) manifest: DaManifestV1,
    pub(super) encoded: Vec<u8>,
    pub(super) manifest_hash: BlobDigest,
    pub(super) blob_hash: BlobDigest,
    pub(super) chunk_root: BlobDigest,
    pub(super) storage_ticket: StorageTicketId,
    pub(super) fingerprint: ReplayFingerprint,
}

#[allow(clippy::too_many_arguments)]
fn resolve_manifest(
    request: &DaIngestRequest,
    chunk_store: &ChunkStore,
    canonical_payload: &[u8],
    metadata: &ExtraMetadata,
    enforced_retention: &RetentionPolicy,
    queued_at_unix: u64,
    rent_policy: &DaRentPolicyV1,
) -> Result<ManifestArtifacts, (StatusCode, String)> {
    resolve_manifest_with_observer(
        request,
        chunk_store,
        canonical_payload,
        metadata,
        enforced_retention,
        queued_at_unix,
        rent_policy,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn resolve_manifest_with_observer(
    request: &DaIngestRequest,
    chunk_store: &ChunkStore,
    canonical_payload: &[u8],
    metadata: &ExtraMetadata,
    enforced_retention: &RetentionPolicy,
    queued_at_unix: u64,
    rent_policy: &DaRentPolicyV1,
    chunking_observer: Option<&dyn Fn(Duration)>,
) -> Result<ManifestArtifacts, (StatusCode, String)> {
    let blob_hash = BlobDigest::from_hash(*chunk_store.payload_digest());
    let chunk_root = BlobDigest::new(*chunk_store.por_tree().root());
    let total_stripes = (chunk_store.chunks().len() as u32)
        .div_ceil(u32::from(request.erasure_profile.data_shards));
    let shards_per_stripe = u32::from(request.erasure_profile.data_shards)
        .saturating_add(u32::from(request.erasure_profile.parity_shards));
    let total_stripes_full =
        total_stripes.saturating_add(u32::from(request.erasure_profile.row_parity_stripes));

    let chunking_started = Instant::now();
    let chunk_commitments = build_chunk_commitments(request, chunk_store, canonical_payload)?;
    if let Some(observer) = chunking_observer {
        observer(chunking_started.elapsed());
    }

    let (rent_gib, rent_months) = rent_usage_from_request(request.total_size, enforced_retention);
    let rent_quote = rent_policy.quote(rent_gib, rent_months).map_err(|err| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to compute DA rent quote: {err}"),
        )
    })?;

    let manifest_template = if let Some(bytes) = &request.norito_manifest {
        let archived = from_bytes::<DaManifestV1>(bytes).map_err(|err| {
            warn!(?err, "failed to decode DA manifest");
            (
                StatusCode::BAD_REQUEST,
                format!("failed to decode DA manifest: {err}"),
            )
        })?;
        let manifest = NoritoDeserialize::try_deserialize(archived).map_err(|err| {
            warn!(?err, "failed to deserialize DA manifest");
            (
                StatusCode::BAD_REQUEST,
                format!("failed to deserialize DA manifest: {err}"),
            )
        })?;
        let expected_ipa = ipa_commitment_from_chunks(&chunk_commitments)?;
        let ipa_commitment = if manifest.ipa_commitment.is_zero() {
            expected_ipa
        } else if manifest.ipa_commitment == expected_ipa {
            manifest.ipa_commitment
        } else {
            return Err((
                StatusCode::BAD_REQUEST,
                "manifest ipa_commitment does not match computed value".into(),
            ));
        };

        verify_manifest_against_request(
            request,
            &manifest,
            enforced_retention,
            metadata,
            &chunk_commitments,
            blob_hash,
            chunk_root,
            &rent_quote,
        )?;

        DaManifestV1 {
            version: manifest.version,
            storage_ticket: StorageTicketId::default(),
            total_stripes: total_stripes_full,
            shards_per_stripe,
            metadata: metadata.clone(),
            rent_quote,
            ipa_commitment,
            issued_at_unix: 0,
            ..manifest
        }
    } else {
        let ipa_commitment = ipa_commitment_from_chunks(&chunk_commitments)?;
        DaManifestV1 {
            version: DaManifestV1::VERSION,
            client_blob_id: request.client_blob_id.clone(),
            lane_id: request.lane_id,
            epoch: request.epoch,
            blob_class: request.blob_class,
            codec: request.codec.clone(),
            blob_hash,
            chunk_root,
            storage_ticket: StorageTicketId::default(),
            total_size: request.total_size,
            chunk_size: request.chunk_size,
            total_stripes: total_stripes_full,
            shards_per_stripe,
            erasure_profile: request.erasure_profile,
            retention_policy: enforced_retention.clone(),
            rent_quote,
            chunks: chunk_commitments.clone(),
            ipa_commitment,
            metadata: metadata.clone(),
            issued_at_unix: 0,
        }
    };

    let fingerprint = manifest_fingerprint(&manifest_template)?;
    let storage_ticket = StorageTicketId::new(*fingerprint.as_bytes());
    let manifest = DaManifestV1 {
        storage_ticket,
        issued_at_unix: queued_at_unix,
        ..manifest_template
    };
    let encoded =
        to_bytes(&manifest).map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    let manifest_hash = BlobDigest::from_hash(blake3_hash(&encoded));

    Ok(ManifestArtifacts {
        manifest,
        encoded,
        manifest_hash,
        blob_hash,
        chunk_root,
        storage_ticket,
        fingerprint,
    })
}

fn derive_kzg_commitment(
    chunk_root: &BlobDigest,
    storage_ticket: &StorageTicketId,
) -> KzgCommitment {
    let mut hasher = Blake3Hasher::new();
    hasher.update(chunk_root.as_ref());
    hasher.update(storage_ticket.as_ref());

    let mut bytes = [0u8; 48];
    hasher.finalize_xof().fill(&mut bytes);
    KzgCommitment::new(bytes)
}

fn build_da_commitment_record(
    request: &DaIngestRequest,
    manifest: &ManifestArtifacts,
    retention: &RetentionPolicy,
    operator_signature: &Signature,
    pdp_commitment_bytes: &[u8],
    proof_scheme: DaProofScheme,
) -> DaCommitmentRecord {
    let manifest_digest = ManifestDigest::new(*manifest.manifest_hash.as_bytes());
    let chunk_root = Hash::prehashed(*manifest.chunk_root.as_bytes());
    let proof_digest = Hash::new(pdp_commitment_bytes);
    let kzg_commitment = match proof_scheme {
        DaProofScheme::MerkleSha256 => None,
        DaProofScheme::KzgBls12_381 => Some(derive_kzg_commitment(
            &manifest.chunk_root,
            &manifest.storage_ticket,
        )),
    };
    DaCommitmentRecord::new(
        request.lane_id,
        request.epoch,
        request.sequence,
        request.client_blob_id.clone(),
        manifest_digest,
        proof_scheme,
        chunk_root,
        kzg_commitment,
        Some(proof_digest),
        retention.clone(),
        manifest.storage_ticket,
        operator_signature.clone(),
    )
}

fn record_da_rent_quote_metrics(
    telemetry: &MaybeTelemetry,
    cluster_label: &str,
    storage_class: StorageClass,
    rent_gib: u64,
    rent_months: u32,
    rent_quote: &DaRentQuote,
) {
    if !telemetry.is_enabled() {
        return;
    }
    let months_u64 = u64::from(rent_months);
    let gib_months = rent_gib.saturating_mul(months_u64);
    let storage_label = storage_class_label(storage_class);
    telemetry.with_metrics(|handle| {
        handle.record_da_rent_quote(cluster_label, storage_label, gib_months, rent_quote);
    });
}

fn record_da_chunking_metrics(telemetry: &MaybeTelemetry, elapsed: Duration) {
    if !telemetry.is_enabled() {
        return;
    }
    telemetry.with_metrics(|handle| {
        handle.observe_da_chunking_seconds(elapsed.as_secs_f64());
    });
}

fn record_da_receipt_metrics(
    telemetry: &MaybeTelemetry,
    lane_epoch: LaneEpoch,
    sequence: u64,
    outcome: &ReceiptInsertOutcome,
) {
    if !telemetry.is_enabled() {
        return;
    }
    let (outcome_label, cursor_advanced) = match outcome {
        ReceiptInsertOutcome::Stored { cursor_advanced } => ("stored", *cursor_advanced),
        ReceiptInsertOutcome::Duplicate { .. } => ("duplicate", false),
        ReceiptInsertOutcome::ManifestConflict { .. } => ("manifest_conflict", false),
        ReceiptInsertOutcome::StaleSequence { .. } => ("stale_sequence", false),
    };
    telemetry.with_metrics(|handle| {
        handle.record_da_receipt_outcome(
            lane_epoch.lane_id.as_u32(),
            lane_epoch.epoch,
            sequence,
            outcome_label,
            cursor_advanced,
        );
    });
}

fn record_da_receipt_error_metrics(
    telemetry: &MaybeTelemetry,
    lane_epoch: LaneEpoch,
    sequence: u64,
) {
    if !telemetry.is_enabled() {
        return;
    }
    telemetry.with_metrics(|handle| {
        handle.record_da_receipt_outcome(
            lane_epoch.lane_id.as_u32(),
            lane_epoch.epoch,
            sequence,
            "error",
            false,
        );
    });
}

fn da_metadata_error(key: &str, message: impl Into<String>) -> (StatusCode, String) {
    (
        StatusCode::BAD_REQUEST,
        format!("invalid DA metadata `{key}`: {}", message.into()),
    )
}

fn validate_public_metadata_entry(
    entry: &MetadataEntry,
    key: &str,
) -> Result<(), (StatusCode, String)> {
    if entry.visibility != MetadataVisibility::Public {
        return Err(da_metadata_error(key, "must use public visibility"));
    }
    if !matches!(entry.encryption, MetadataEncryption::None) {
        return Err(da_metadata_error(key, "must not be encrypted"));
    }
    Ok(())
}

fn registry_alias_from_metadata(
    metadata: &ExtraMetadata,
) -> Result<Option<String>, (StatusCode, String)> {
    let Some(entry) = metadata
        .items
        .iter()
        .find(|entry| entry.key == META_DA_REGISTRY_ALIAS)
    else {
        return Ok(None);
    };
    validate_public_metadata_entry(entry, META_DA_REGISTRY_ALIAS)?;
    let value = std::str::from_utf8(&entry.value)
        .map_err(|_| da_metadata_error(META_DA_REGISTRY_ALIAS, "value must be valid UTF-8"))?
        .trim();
    if value.is_empty() {
        return Err(da_metadata_error(
            META_DA_REGISTRY_ALIAS,
            "alias must not be empty",
        ));
    }
    Ok(Some(value.to_owned()))
}

fn registry_owner_from_metadata(
    metadata: &ExtraMetadata,
) -> Result<Option<AccountId>, (StatusCode, String)> {
    let Some(entry) = metadata
        .items
        .iter()
        .find(|entry| entry.key == META_DA_REGISTRY_OWNER)
    else {
        return Ok(None);
    };
    validate_public_metadata_entry(entry, META_DA_REGISTRY_OWNER)?;
    let value = std::str::from_utf8(&entry.value)
        .map_err(|_| da_metadata_error(META_DA_REGISTRY_OWNER, "value must be valid UTF-8"))?
        .trim();
    if value.is_empty() {
        return Err(da_metadata_error(
            META_DA_REGISTRY_OWNER,
            "owner must not be empty",
        ));
    }
    let owner = AccountId::parse_encoded(value)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .map_err(|err| {
            da_metadata_error(
                META_DA_REGISTRY_OWNER,
                format!("invalid AccountId `{value}`: {err}"),
            )
        })?;
    Ok(Some(owner))
}

#[allow(clippy::too_many_arguments)]
fn verify_manifest_against_request(
    request: &DaIngestRequest,
    manifest: &DaManifestV1,
    expected_retention: &RetentionPolicy,
    expected_metadata: &ExtraMetadata,
    computed_chunks: &[ChunkCommitment],
    blob_hash: BlobDigest,
    chunk_root: BlobDigest,
    expected_rent: &DaRentQuote,
) -> Result<(), (StatusCode, String)> {
    if manifest.version != DaManifestV1::VERSION {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "unsupported manifest version {}; expected {}",
                manifest.version,
                DaManifestV1::VERSION
            ),
        ));
    }
    if manifest.client_blob_id != request.client_blob_id {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest client_blob_id does not match ingest request".into(),
        ));
    }
    if manifest.lane_id != request.lane_id || manifest.epoch != request.epoch {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest lane/epoch do not match ingest request".into(),
        ));
    }
    if manifest.blob_class != request.blob_class || manifest.codec != request.codec {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest blob classification does not match ingest request".into(),
        ));
    }
    if manifest.total_size != request.total_size || manifest.chunk_size != request.chunk_size {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest total_size or chunk_size does not match ingest request".into(),
        ));
    }
    if manifest.erasure_profile != request.erasure_profile {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest erasure profile does not match ingest request".into(),
        ));
    }
    if manifest.retention_policy != *expected_retention {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest retention policy does not match ingest request".into(),
        ));
    }
    if manifest.metadata != *expected_metadata {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest metadata does not match ingest request".into(),
        ));
    }
    if manifest.rent_quote != *expected_rent {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest rent quote does not match configured policy".into(),
        ));
    }
    if manifest.blob_hash != blob_hash {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest blob_hash does not match canonical payload digest".into(),
        ));
    }
    if manifest.chunk_root != chunk_root {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest chunk_root does not match recomputed chunk root".into(),
        ));
    }
    if manifest.chunks.len() != computed_chunks.len() {
        return Err((
            StatusCode::BAD_REQUEST,
            "manifest chunk count does not match chunker output".into(),
        ));
    }

    if manifest.blob_class == BlobClass::TaikaiSegment {
        taikai::validate_taikai_cache_hint(expected_metadata, &blob_hash, manifest.total_size)?;
        taikai::validate_da_proof_tier(expected_metadata, manifest.retention_policy.storage_class)?;
    }

    for (expected, actual) in computed_chunks.iter().zip(manifest.chunks.iter()) {
        if expected.index != actual.index
            || expected.offset != actual.offset
            || expected.length != actual.length
            || expected.commitment != actual.commitment
        {
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "manifest chunk commitment mismatch at index {}",
                    expected.index
                ),
            ));
        }
        if expected.parity != actual.parity {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("manifest parity flag mismatch at index {}", expected.index),
            ));
        }
        if effective_chunk_role(expected) != effective_chunk_role(actual) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("manifest role mismatch at index {}", expected.index),
            ));
        }
        if actual.group_id != 0 && expected.group_id != actual.group_id {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("manifest group_id mismatch at index {}", expected.index),
            ));
        }
    }

    Ok(())
}

fn build_error_response(status: StatusCode, message: &str, format: ResponseFormat) -> Response {
    let mut map = Map::new();
    map.insert("status".into(), Value::from(status.as_str()));
    map.insert("error".into(), Value::from(message));
    let body = Value::Object(map);
    let mut response = utils::respond_value_with_format(body, format);
    *response.status_mut() = status;
    response
}

fn ceil_div_u64(value: u64, divisor: u64) -> u64 {
    if divisor == 0 {
        return 0;
    }
    if value == 0 {
        return 0;
    }
    value.div_ceil(divisor)
}

fn rent_usage_from_request(total_size: u64, retention: &RetentionPolicy) -> (u64, u32) {
    let adjusted_size = total_size.max(1);
    let gib = ceil_div_u64(adjusted_size, BYTES_PER_GIB).max(1);
    let retention_secs = retention
        .hot_retention_secs
        .max(retention.cold_retention_secs)
        .max(1);
    let months_u64 = ceil_div_u64(retention_secs, SECS_PER_MONTH).max(1);
    let months_u32 = u32::try_from(months_u64).unwrap_or(u32::MAX);
    (gib, months_u32)
}

fn with_status(mut response: Response, status: StatusCode) -> Response {
    *response.status_mut() = status;
    response
}

#[derive(JsonSerialize, norito::derive::NoritoSerialize)]
struct DaIngestResponse {
    status: &'static str,
    duplicate: bool,
    receipt: Option<DaIngestReceipt>,
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
